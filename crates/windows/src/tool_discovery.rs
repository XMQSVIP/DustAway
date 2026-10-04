//! Shared, injectable installed-tool discovery. Only fixed read-only probes are permitted.
use crate::{
    fs::*,
    maintenance_tools::{Action, Tool},
    platform::{known_folder, volume_target},
    uv_cache,
};
use anyhow::{Result, ensure};
use dustaway_core::{ToolDiscovery, VolumeTarget};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, FOLDERID_Profile};

pub(crate) fn supported(id: &str) -> bool {
    matches!(id, "dev.uv" | "dev.go_build" | "dev.yarn" | "dev.ccache")
}
pub(crate) trait Probe: Send + Sync {
    fn query(&self, rule: &str, cancel: &AtomicBool) -> Result<Discovered>;
}
pub(crate) struct InstalledProbe;
pub(crate) struct Discovered {
    pub tool: ToolDiscovery,
}
pub(crate) fn identity(exe: &Path, entry: Option<&Path>) -> Result<String> {
    let exe = PinnedPath::open(exe, false, false)?;
    let entry = entry
        .map(|p| PinnedPath::open(p, false, false))
        .transpose()?;
    let bytes = serde_json::to_vec(&(
        exe.stamp,
        exe.parent_ids,
        entry.map(|p| (p.stamp, p.parent_ids)),
    ))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn target_identity(exe: &Path, entry: Option<&Path>, root: &Path) -> Result<String> {
    let executable = identity(exe, entry)?;
    let directory = PinnedPath::open(root, true, false)
        .ok()
        .map(|p| (p.stamp.identity, p.parent_ids));
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(executable, directory))?)
    ))
}
impl Probe for InstalledProbe {
    fn query(&self, rule: &str, cancel: &AtomicBool) -> Result<Discovered> {
        let home = known_folder(&FOLDERID_Profile)?;
        let (exe, entry, version, root, layout) = if rule == "dev.uv" {
            let (exe, version, root) =
                uv_cache::discover(&home, &known_folder(&FOLDERID_LocalAppData)?, cancel)?;
            let layout = uv_cache::validate_root(&root).map(|_| ());
            (exe, None, version, root, layout)
        } else {
            let tool = Tool::discover(Action::parse(rule)?, &home, cancel)?;
            let root = tool.target(&home, cancel)?;
            let layout = tool.validate_layout(&root, cancel);
            (tool.exe, tool.entry, tool.version, root, layout)
        };
        check_cancel(Some(cancel))?;
        let mut tool = ToolDiscovery {
            executable: Some(display_path(&exe)),
            entry: entry.as_deref().map(display_path),
            version: Some(version),
            target: Some(display_path(&root)),
            identity: Some(target_identity(&exe, entry.as_deref(), &root)?),
            ready: false,
            reason: None,
            message: String::new(),
        };
        let result = layout
            .and_then(|_| crate::tool_process::guard(rule, Some(&exe), entry.as_deref(), cancel));
        match result {
            Ok(()) => {
                tool.ready = true;
                tool.message = "已确认工具与处理位置；执行前仍会重新核对".into();
            }
            Err(error) => {
                let cache_missing = root.try_exists().is_ok_and(|exists| !exists);
                tool.reason = Some(
                    error
                        .downcast_ref::<dustaway_core::ApiError>()
                        .map(|e| e.code)
                        .unwrap_or(if cache_missing {
                            "cache_missing"
                        } else {
                            "unsupported_layout"
                        })
                        .into(),
                );
                tool.message = if cache_missing {
                    "实际处理位置当前没有缓存；标准位置发现的残留仍保留，可重新检查这一项".into()
                } else {
                    format!("{error:#}")
                };
            }
        }
        Ok(Discovered { tool })
    }
}

#[cfg(test)]
pub(crate) struct UnavailableProbe;
#[cfg(test)]
impl Probe for UnavailableProbe {
    fn query(&self, _: &str, _: &AtomicBool) -> Result<Discovered> {
        anyhow::bail!("未安装测试工具")
    }
}
pub(crate) fn inspect(probe: &dyn Probe, rule: &str, cancel: &AtomicBool) -> Result<ToolDiscovery> {
    ensure!(supported(rule), "此规则没有官方操作");
    let result = probe.query(rule, cancel);
    check_cancel(Some(cancel))?;
    Ok(match result {
        Ok(found) => found.tool,
        Err(error) => ToolDiscovery {
            executable: None,
            entry: None,
            version: None,
            target: None,
            identity: None,
            ready: false,
            reason: Some(
                error
                    .downcast_ref::<dustaway_core::ApiError>()
                    .map(|e| e.code)
                    .unwrap_or("target_unknown")
                    .into(),
            ),
            message: format!(
                "实际处理位置未确认：{error:#}。已保留标准位置中的文件，可重新检查这一项"
            ),
        },
    })
}
pub(crate) fn constrain(
    tool: &mut ToolDiscovery,
    volumes: Option<&[VolumeTarget]>,
    exclusions: &[String],
) {
    let Some(root) = tool.target.as_ref().map(PathBuf::from) else {
        return;
    };
    let result = (|| -> Result<()> {
        if let Some(volumes) = volumes {
            let target = volume_target(&root)?;
            ensure!(
                volumes.iter().any(|v| v.key == target.key),
                dustaway_core::ApiError::new(
                    "outside_scan",
                    "实际处理位置不在本次所选磁盘；调整磁盘后完整扫描"
                )
            );
        }
        uv_cache::validate_exclusions(&root, exclusions)
    })();
    if let Err(error) = result {
        tool.ready = false;
        tool.reason = Some(
            error
                .downcast_ref::<dustaway_core::ApiError>()
                .map(|e| e.code)
                .unwrap_or("excluded")
                .into(),
        );
        tool.message = format!("{error:#}");
    }
}

/// Rechecked by the command boundary before preparing a plan, using this same adapter.
pub fn verify(rule: &str, expected: &ToolDiscovery, cancel: &AtomicBool) -> Result<()> {
    verify_with(&InstalledProbe, rule, expected, cancel)
}
fn verify_with(
    probe: &dyn Probe,
    rule: &str,
    expected: &ToolDiscovery,
    cancel: &AtomicBool,
) -> Result<()> {
    let found = inspect(probe, rule, cancel)?;
    ensure!(found.ready, "{}", found.message);
    ensure!(
        found.identity == expected.identity
            && found.target == expected.target
            && found.version == expected.version,
        "工具或实际处理位置已变化，请重新检查这一项；不会更换处理目标"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Failing;
    impl Probe for Failing {
        fn query(&self, _: &str, _: &AtomicBool) -> Result<Discovered> {
            anyhow::bail!(dustaway_core::ApiError::new(
                "unsupported_version",
                "版本未适配"
            ))
        }
    }
    #[test]
    fn query_failure_is_structured_and_cancel_never_falls_back_to_ready() {
        let cancel = AtomicBool::new(false);
        let result = inspect(&Failing, "dev.go_build", &cancel).unwrap();
        assert!(!result.ready);
        assert_eq!(result.reason.as_deref(), Some("unsupported_version"));
        assert!(result.message.contains("实际处理位置未确认"));
        assert!(result.target.is_none());
        assert!(inspect(&Failing, "dev.pnpm", &cancel).is_err());
        cancel.store(true, std::sync::atomic::Ordering::Release);
        assert!(inspect(&Failing, "dev.uv", &cancel).is_err());
    }
    #[test]
    fn target_and_identity_changes_cannot_reuse_a_scan_and_scope_is_enforced() {
        struct Fixed(ToolDiscovery);
        impl Probe for Fixed {
            fn query(&self, _: &str, _: &AtomicBool) -> Result<Discovered> {
                Ok(Discovered {
                    tool: self.0.clone(),
                })
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("cache");
        std::fs::create_dir(&root).unwrap();
        let expected = ToolDiscovery {
            executable: Some("C:\\tool.exe".into()),
            entry: None,
            version: Some("1".into()),
            target: Some(display_path(&root)),
            ready: true,
            reason: None,
            message: "ready".into(),
            identity: Some("one".into()),
        };
        let cancel = AtomicBool::new(false);
        assert!(verify_with(&Fixed(expected.clone()), "dev.go_build", &expected, &cancel).is_ok());
        let mut changed = expected.clone();
        changed.identity = Some("replacement".into());
        assert!(verify_with(&Fixed(changed), "dev.go_build", &expected, &cancel).is_err());
        let mut changed = expected.clone();
        changed.target = Some("D:\\other-cache".into());
        assert!(verify_with(&Fixed(changed), "dev.go_build", &expected, &cancel).is_err());
        let mut outside = expected.clone();
        constrain(
            &mut outside,
            Some(&[VolumeTarget {
                key: "other-volume".into(),
                label: "D:".into(),
                opaque: String::new(),
            }]),
            &[],
        );
        assert!(!outside.ready);
        assert_eq!(outside.reason.as_deref(), Some("outside_scan"));
        let mut excluded = expected.clone();
        constrain(&mut excluded, None, &[display_path(&root)]);
        assert!(!excluded.ready);
        assert_eq!(excluded.target, expected.target);
    }
    #[test]
    fn same_path_directory_replacement_changes_the_discovery_identity() {
        let temp = tempfile::tempdir().unwrap();
        let exe = temp.path().join("tool.exe");
        std::fs::write(&exe, b"identity fixture, never executed").unwrap();
        let root = temp.path().join("cache");
        std::fs::create_dir(&root).unwrap();
        let before = target_identity(&exe, None, &root).unwrap();
        // All renamed directories are dedicated children of this TempDir.
        std::fs::rename(&root, temp.path().join("old-cache")).unwrap();
        std::fs::create_dir(&root).unwrap();
        assert_ne!(target_identity(&exe, None, &root).unwrap(), before);
    }
}
