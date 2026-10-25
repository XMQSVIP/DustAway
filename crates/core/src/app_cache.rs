//! Closed, compiled application-cache locations. Recognition is not deletion permission.
use crate::{CleanupPolicy, DataKind, RuleDescriptor};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheSpec {
    #[serde(default = "initial_version")]
    pub version: u32,
    pub id: String,
    pub app: String,
    pub title: String,
    pub folder: String,
    pub base: String,
    pub paths: Vec<String>,
    pub kind: String,
    #[serde(default)]
    pub data_kind: DataKind,
    pub age: u32,
    pub processes: Vec<String>,
    pub aliases: Vec<String>,
    pub impact: String,
}
fn initial_version() -> u32 {
    1
}
impl CacheSpec {
    pub fn cleanable(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "vsix" | "vscode_old_code" | "documented_gpu" | "documented_code"
        )
    }
    pub fn descriptor(&self) -> RuleDescriptor {
        RuleDescriptor {
            id: self.id.clone(),
            version: self.version,
            title: self.title.clone(),
            description: self.impact.clone(),
            impact: self.impact.clone(),
            root: "application_cache".into(),
            relative: self.base.clone(),
            min_age_days: self.age,
            process: self.processes.first().cloned(),
            requires_admin: false,
            default_selected: false,
            cleanup_policy: CleanupPolicy::Review,
            cleanup_validated: self.cleanable(),
            app_id: None,
            data_kind: self.data_kind,
        }
    }
}
pub fn catalog() -> &'static [CacheSpec] {
    static CATALOG: OnceLock<Vec<CacheSpec>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../rules/application-cache.json"))
            .expect("compiled cache catalog")
    })
}
pub fn rule(id: &str) -> Option<&'static CacheSpec> {
    catalog().iter().find(|s| s.id == id)
}
/// Registration names often omit spaces (for example "123浜戠洏"). This remains
/// a display hint, never installation verification or deletion authorization.
pub fn registered_name_matches(name: &str, aliases: &[&str]) -> bool {
    let normalize = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    };
    let name = normalize(name);
    aliases
        .iter()
        .any(|a| !a.is_empty() && name.contains(&normalize(a)))
}
pub fn valid_descriptor(r: &RuleDescriptor) -> bool {
    rule(&r.id)
        .is_some_and(|s| serde_json::to_value(r).ok() == serde_json::to_value(s.descriptor()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_closed_locations_and_no_implicit_permissions() {
        assert!(registered_name_matches("123浜戠洏 3.2.3", &["123 浜戠洏"]));
        assert!(!registered_name_matches("Other product", &["123 浜戠洏"]));
        let mut ids = std::collections::HashSet::new();
        for s in catalog() {
            assert!(ids.insert(&s.id));
            assert!(matches!(s.folder.as_str(), "local" | "roaming"));
            for p in std::iter::once(&s.base).chain(s.paths.iter()) {
                assert!(
                    !p.starts_with('/')
                        && !p.contains(':')
                        && !p.contains("..")
                        && !p.contains('*')
                );
            }
            assert!(valid_descriptor(&s.descriptor()));
            let mut forged = s.descriptor();
            forged.relative = "Documents".into();
            assert!(!valid_descriptor(&forged));
            forged = s.descriptor();
            forged.cleanup_validated = !forged.cleanup_validated;
            assert!(!valid_descriptor(&forged));
            assert!(!s.descriptor().default_selected);
            if s.cleanable() {
                assert!(matches!(
                    s.app.as_str(),
                    "vscode" | "vscode_insiders" | "cherry" | "pycharm"
                ));
            }
        }
    }

    #[test]
    fn diagnostics_metadata_is_optional_and_cannot_enable_cleanup() {
        let old: CacheSpec = serde_json::from_value(serde_json::json!({
            "id":"legacy", "app":"legacy", "title":"legacy", "folder":"local",
            "base":"legacy", "paths":["Cache"], "kind":"readonly", "age":0,
            "processes":[], "aliases":[], "impact":"readonly"
        }))
        .unwrap();
        assert_eq!(old.descriptor().data_kind, DataKind::Cache);
        for id in [
            "appcache.shadowbot_logs",
            "appcache.docker_logs",
            "appcache.reqable_logs",
        ] {
            let spec = rule(id).unwrap();
            assert_eq!(spec.descriptor().data_kind, DataKind::Diagnostics);
            assert_eq!(spec.age, 0);
            assert!(!spec.cleanable());
            assert!(!crate::cleanup_acceptance::release_enabled(
                &spec.descriptor()
            ));
        }
    }
}

#[cfg(test)]
mod coverage_024_tests {
    use super::*;

    #[test]
    fn eight_observed_scopes_pin_locations_types_and_no_permission() {
        let common = vec![
            "Default/Cache",
            "Default/Code Cache",
            "Default/GPUCache",
            "Default/DawnGraphiteCache",
            "Default/DawnWebGPUCache",
            "ShaderCache",
            "GrShaderCache",
        ];
        let scopes = [
            (
                "codebuddy_extension_logs",
                "local",
                "CodeBuddyExtension",
                vec!["Logs"],
                DataKind::Diagnostics,
            ),
            (
                "workbuddy_extension_logs",
                "local",
                "WorkBuddyExtension",
                vec!["Logs"],
                DataKind::Diagnostics,
            ),
            (
                "todesk_logs",
                "local",
                "ToDesk",
                vec!["Logs"],
                DataKind::Diagnostics,
            ),
            (
                "todesk_web",
                "local",
                "ToDesk/EBWebView",
                common.clone(),
                DataKind::Cache,
            ),
            (
                "dingtalk_logs",
                "roaming",
                "DingTalk",
                vec!["log", "holmeslogs", "recordLog", "updaterlogs"],
                DataKind::Diagnostics,
            ),
            (
                "webtoapi_web",
                "local",
                "WebToApi/chrome-profile",
                common.clone(),
                DataKind::Cache,
            ),
            (
                "webtoapi_logs",
                "local",
                "WebToApi",
                vec!["logs"],
                DataKind::Diagnostics,
            ),
            (
                "dbx_web",
                "local",
                "com.dbx.app/EBWebView",
                common,
                DataKind::Cache,
            ),
        ];
        for (id, folder, base, paths, kind) in scopes {
            let spec = rule(&format!("appcache.{id}")).unwrap();
            assert_eq!((spec.folder.as_str(), spec.base.as_str()), (folder, base));
            assert_eq!(spec.paths, paths);
            assert_eq!(spec.data_kind, kind);
            assert_eq!(spec.kind, "readonly_observed");
            assert_eq!(spec.age, 0);
            assert!(!spec.descriptor().default_selected);
            assert!(!spec.cleanable());
            assert!(!crate::cleanup_acceptance::release_enabled(
                &spec.descriptor()
            ));
        }
    }
}

#[cfg(test)]
mod coverage_023_tests {
    use super::*;
    #[test]
    fn ten_observed_scopes_preserve_exact_boundaries_and_no_deletion_authority() {
        let common = [
            "Default/Cache",
            "Default/Code Cache",
            "Default/GPUCache",
            "Default/DawnGraphiteCache",
            "Default/DawnWebGPUCache",
            "ShaderCache",
            "GrShaderCache",
        ];
        for (id, folder, base, leaf) in [
            ("codex_logs", "local", "Codex", "Logs"),
            ("trae_logs", "roaming", "Trae", "logs"),
            ("trae_cn_logs", "roaming", "Trae CN", "logs"),
            ("kiro_logs", "roaming", "Kiro", "logs"),
            ("workbuddy_logs", "roaming", "WorkBuddy", "logs"),
        ] {
            let spec = rule(&format!("appcache.{id}")).unwrap();
            assert_eq!((spec.folder.as_str(), spec.base.as_str()), (folder, base));
            assert_eq!(spec.paths, [leaf]);
            assert_eq!(spec.data_kind, DataKind::Diagnostics);
            assert_eq!(spec.age, 0);
            assert!(!crate::cleanup_acceptance::release_enabled(
                &spec.descriptor()
            ));
        }
        for (id, folder, base, graphite) in [
            ("doubao_web", "local", "Doubao/User Data", true),
            (
                "gameviewer_web",
                "local",
                "GameViewer/webviewcache/EBWebView",
                true,
            ),
            (
                "ccswitch_web",
                "local",
                "com.ccswitch.desktop/EBWebView",
                false,
            ),
            ("ima_web", "local", "ima.copilot/User Data", true),
            ("easyspider_web", "roaming", "EasySpider", false),
        ] {
            let spec = rule(&format!("appcache.{id}")).unwrap();
            let mut paths = if id == "easyspider_web" {
                vec!["Code Cache", "GPUCache", "DawnCache"]
            } else {
                common.to_vec()
            };
            if graphite {
                paths.push("GraphiteDawnCache");
            }
            assert_eq!((spec.folder.as_str(), spec.base.as_str()), (folder, base));
            assert_eq!(spec.paths, paths);
            assert_eq!(spec.data_kind, DataKind::Cache);
            assert_eq!(spec.age, 0);
            assert!(!crate::cleanup_acceptance::release_enabled(
                &spec.descriptor()
            ));
        }
    }
}
