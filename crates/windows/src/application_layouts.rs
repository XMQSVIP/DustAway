//! Closed, metadata-only application layouts. Never reads Preferences or account contents.
use crate::{fs::*, platform::profile_marker};
use anyhow::Result;
use dustaway_core::{
    ReasonCode,
    applications::{ApplicationFolder, ApplicationRule, ProfileLayout},
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub(crate) struct LayoutInspection {
    pub roots: Vec<PathBuf>,
    pub unavailable: Option<ReasonCode>,
}

pub(crate) fn description(local: &Path, roaming: &Path, spec: &ApplicationRule) -> String {
    let base = match spec.folder {
        ApplicationFolder::Local => local,
        ApplicationFolder::Roaming => roaming,
    }
    .join(spec.base);
    let profile = match spec.profiles {
        ProfileLayout::Direct | ProfileLayout::ChromiumRoot => base,
        ProfileLayout::QqNumeric => base.join("<数字配置目录>"),
        ProfileLayout::FeishuProfiles => base.join("<账号>/profile_main 或 profile_explorer"),
        ProfileLayout::DingTalkNumbered => base.join("DingTalk_<数字>/Default"),
        ProfileLayout::WeixinWeb => base.join("<已识别网页配置>"),
    };
    display_path(&profile.join(spec.relative))
}

fn exists(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn inspect(
    local: &Path,
    roaming: &Path,
    spec: &ApplicationRule,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<LayoutInspection> {
    check_cancel(cancel)?;
    let base = match spec.folder {
        ApplicationFolder::Local => local,
        ApplicationFolder::Roaming => roaming,
    }
    .join(spec.base);
    if !exists(&base)? {
        return Ok(LayoutInspection {
            roots: vec![],
            unavailable: Some(ReasonCode::LocationMissing),
        });
    }
    let _base_pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
    let mut profiles = vec![];
    let mut named_profiles = 0;
    if matches!(
        spec.profiles,
        ProfileLayout::Direct | ProfileLayout::ChromiumRoot
    ) {
        profiles.push(base.clone());
    } else {
        for entry in std::fs::read_dir(&base)? {
            check_cancel(cancel)?;
            let found = (|| -> Result<Vec<PathBuf>> {
                let entry = entry?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let relevant = match spec.profiles {
                    ProfileLayout::QqNumeric => {
                        !name.is_empty() && name.bytes().all(|c| c.is_ascii_digit())
                    }
                    ProfileLayout::DingTalkNumbered => name
                        .strip_prefix("DingTalk_")
                        .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())),
                    ProfileLayout::FeishuProfiles => true,
                    ProfileLayout::WeixinWeb => weixin_web_profile(&name),
                    ProfileLayout::Direct | ProfileLayout::ChromiumRoot => unreachable!(),
                };
                if !relevant {
                    return Ok(vec![]);
                }
                // A reparse point is refused by PinnedPath, including directory junctions.
                if !entry.file_type()?.is_dir() {
                    return Ok(vec![]);
                }
                named_profiles += 1;
                let path = entry.path();
                let _pin = PinnedPath::open_cancellable(&path, true, false, cancel)?;
                Ok(match spec.profiles {
                    ProfileLayout::QqNumeric => vec![path],
                    ProfileLayout::WeixinWeb => vec![path],
                    ProfileLayout::FeishuProfiles => {
                        vec![path.join("profile_main"), path.join("profile_explorer")]
                    }
                    ProfileLayout::DingTalkNumbered => vec![path.join("Default")],
                    ProfileLayout::Direct | ProfileLayout::ChromiumRoot => unreachable!(),
                })
            })();
            match found {
                Ok(paths) => profiles.extend(paths),
                Err(e) if e.is::<ScanCancelled>() => return Err(e),
                Err(e) => issue(error_reason(&e)),
            }
        }
    }
    let mut matched_profiles = 0;
    let mut roots = vec![];
    for profile in profiles {
        check_cancel(cancel)?;
        let found = (|| -> Result<Option<PathBuf>> {
            if !exists(&profile)? {
                return Ok(None);
            }
            let _pin = PinnedPath::open_cancellable(&profile, true, false, cancel)?;
            let marker = match spec.profiles {
                ProfileLayout::Direct => None,
                ProfileLayout::QqNumeric => Some("LocalPrefs.json"),
                _ => Some("Preferences"),
            };
            if let Some(marker) = marker
                && !profile_marker(&profile.join(marker), cancel, issue)?
            {
                return Ok(None);
            }
            matched_profiles += 1;
            let root = profile.join(spec.relative);
            if !exists(&root)? {
                return Ok(None);
            }
            let _root_pin = PinnedPath::open_cancellable(&root, true, false, cancel)?;
            Ok(Some(root))
        })();
        match found {
            Ok(Some(root)) => roots.push(root),
            Ok(None) => {}
            Err(e) if e.is::<ScanCancelled>() => return Err(e),
            Err(e) => issue(error_reason(&e)),
        }
    }
    roots.sort();
    roots.dedup();
    let unavailable = roots.is_empty().then_some(
        if matched_profiles > 0
            || (spec.profiles == ProfileLayout::DingTalkNumbered && named_profiles == 0)
        {
            ReasonCode::LocationMissing
        } else {
            ReasonCode::UnsupportedLayout
        },
    );
    Ok(LayoutInspection { roots, unavailable })
}

pub(crate) fn weixin_web_profile(name: &str) -> bool {
    ["game", "multitab", "webview", "web_shell"]
        .iter()
        .any(|prefix| {
            name == *prefix
                || name
                    .strip_prefix(&format!("{prefix}_"))
                    .is_some_and(|suffix| {
                        suffix.len() == 32 && suffix.bytes().all(|b| b.is_ascii_hexdigit())
                    })
        })
}
