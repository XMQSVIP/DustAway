//! The signed Weixin installation and Preferences-marked Radium profiles bind
//! these scopes. Login, Local Storage, mini-app data and account msg are excluded.
use crate::{app_cache, fs::*};
use anyhow::Result;
use std::path::Path;

pub(crate) fn accepts(id: &str, root: &Path, path: &Path) -> Result<bool> {
    if !matches!(id, "weixin.http" | "weixin.code" | "weixin.gpu") {
        return Ok(true);
    }
    let Ok(relative) = path.strip_prefix(root) else {
        return Ok(false);
    };
    let parts: Vec<_> = relative.iter().filter_map(|p| p.to_str()).collect();
    match id {
        "weixin.gpu" => app_cache::valid_gpu_file(root, path),
        "weixin.code" => {
            if parts.len() != 2 || !matches!(parts[0], "js" | "wasm") {
                return Ok(false);
            }
            let name = parts[1];
            if !name
                .strip_suffix("_0")
                .is_some_and(|s| s.len() == 16 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Ok(false);
            }
            let marker = path.parent().unwrap().join("index");
            Ok(marker.try_exists()?
                && app_cache::valid_code_format(&marker)?
                && app_cache::valid_code_format(path)?)
        }
        // Read-only inventory of external HTTP entries. Shared HTTP storage
        // requires category-wide invalidation; this rule has no release permit.
        "weixin.http" => {
            if parts.len() != 1 {
                return Ok(false);
            }
            let name = parts[0];
            if !name
                .strip_prefix("f_")
                .is_some_and(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Ok(false);
            }
            let _pin = PinnedPath::observe(path, false, None)?;
            app_cache::valid_gpu_file(root, &root.join("index"))
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
#[path = "weixin_acceptance.rs"]
mod acceptance;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_cache::tests::{gpu_fixture, simple_code_fixture},
        application_layouts::weixin_web_profile,
    };
    #[test]
    fn cache_formats_and_profile_names_cannot_expand_into_chat_or_storage() {
        assert!(weixin_web_profile("webview"));
        assert!(weixin_web_profile(
            "web_shell_0123456789abcdef0123456789abcdef"
        ));
        for name in [
            "Default",
            "wxid_sample",
            "webview_backup",
            "webview_../",
            "game_123",
        ] {
            assert!(!weixin_web_profile(name), "{name}");
        }
        let dir = tempfile::tempdir().unwrap();
        let root = extended(dir.path()).unwrap();
        let gpu = root.join("GPUCache");
        let data = gpu_fixture(&gpu);
        assert!(accepts("weixin.gpu", &gpu, &data).unwrap());
        assert!(!accepts("weixin.gpu", &root, &data).unwrap());
        for name in ["Cookies", "message.db", "important.zip", "f_000001"] {
            let path = gpu.join(name);
            std::fs::write(&path, "retain").unwrap();
            assert!(!accepts("weixin.gpu", &gpu, &path).unwrap());
        }
        std::fs::write(&data, "not a cache").unwrap();
        assert!(!accepts("weixin.gpu", &gpu, &data).unwrap());
        let code = root.join("Code Cache");
        let entry = simple_code_fixture(&code);
        assert!(accepts("weixin.code", &code, &entry).unwrap());
        assert!(!accepts("weixin.code", &code, &code.join("js/index")).unwrap());
        std::fs::write(code.join("js/index"), "wrong marker").unwrap();
        assert!(!accepts("weixin.code", &code, &entry).unwrap());
    }
    #[test]
    fn web_layout_requires_preferences_and_refuses_reparse_profiles() {
        use dustaway_core::applications::rule;
        let dir = tempfile::tempdir().unwrap();
        let root = extended(dir.path()).unwrap();
        let spec = rule("weixin.gpu").unwrap();
        let profile = root.join(spec.base).join("webview");
        gpu_fixture(&profile.join(spec.relative));
        let scan =
            || crate::application_layouts::inspect(&root, &root, spec, None, &mut |_| {}).unwrap();
        assert!(scan().roots.is_empty());
        std::fs::write(profile.join("Preferences"), "{}").unwrap();
        assert_eq!(scan().roots, vec![profile.join(spec.relative)]);
        let unknown = root.join(spec.base).join("webview_backup");
        gpu_fixture(&unknown.join(spec.relative));
        std::fs::write(unknown.join("Preferences"), "{}").unwrap();
        assert_eq!(scan().roots.len(), 1);
        std::fs::rename(&profile, root.join("moved-profile")).unwrap();
        // Directory junctions are tested without requiring symlink privileges.
        let result = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&profile)
            .arg(root.join("moved-profile"))
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(scan().roots.is_empty());
        std::fs::remove_dir(&profile).unwrap();
    }
}
