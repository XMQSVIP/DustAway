//! Explicit personal-file deletion. This policy can only restrict, never unlock a rule.
use crate::{fs::*, platform::known_folder};
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};
use windows::Win32::{Storage::FileSystem::*, UI::Shell::*};

struct Protection {
    trees: Vec<String>,
    containers: Vec<String>,
}
static PROTECTION: LazyLock<Protection> = LazyLock::new(|| {
    let mut trees: Vec<_> = [
        FOLDERID_Windows,
        FOLDERID_ProgramFiles,
        FOLDERID_ProgramFilesX86,
        FOLDERID_ProgramData,
        FOLDERID_LocalAppData,
        FOLDERID_LocalAppDataLow,
        FOLDERID_RoamingAppData,
    ]
    .iter()
    .filter_map(|id| known_folder(id).ok())
    .collect();
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        trees.push(parent.to_owned());
    }
    // Verified installations can live on any drive. Their entire trees remain protected.
    for app in dustaway_core::applications::APPLICATIONS {
        if let Ok(found) = crate::applications::identify(app) {
            trees.push(found.install);
        }
    }
    let containers: Vec<PathBuf> = [
        FOLDERID_Profile,
        FOLDERID_Desktop,
        FOLDERID_Documents,
        FOLDERID_Downloads,
        FOLDERID_Pictures,
        FOLDERID_Music,
        FOLDERID_Videos,
    ]
    .iter()
    .filter_map(|id| known_folder(id).ok())
    .collect();
    trees.extend(crate::applications::installed_directories());
    if let Ok(platform) = crate::WindowsPlatform::new(Scope::User)
        && let Ok(rules) = dustaway_core::rules::for_scope(Scope::User)
    {
        for rule in rules {
            if let Ok(roots) = platform.roots(&rule) {
                trees.extend(roots);
            }
        }
    }
    Protection {
        trees: trees.iter().map(|p| key(p)).collect(),
        containers: containers.iter().map(|p| key(p)).collect(),
    }
});
fn key(path: &Path) -> String {
    display_path(path).trim_end_matches('\\').to_lowercase()
}
fn within(path: &str, root: &str) -> bool {
    path == root || path.strip_prefix(root).is_some_and(|s| s.starts_with('\\'))
}
pub(crate) fn protected(path: &Path, directory: bool) -> bool {
    let name = key(path);
    if path.parent().is_none() || name.len() <= 2 {
        return true;
    }
    if PROTECTION.trees.iter().any(|p| within(&name, p)) {
        return true;
    }
    if directory && PROTECTION.containers.contains(&name) {
        return true;
    }
    // Core app data outside Known Folders, including user-selected messaging locations.
    let parts: Vec<_> = name.split('\\').collect();
    if parts.iter().any(|p| {
        matches!(
            *p,
            "program files"
                | "program files (x86)"
                | "programdata"
                | "$recycle.bin"
                | "system volume information"
                | "windowsapps"
                | "wpsystem"
                | "appdata"
                | "wechat files"
                | "xwechat_files"
                | "weixin"
                | "dingtalk"
                | "baidunetdisk"
                | "tencent files"
        )
    }) {
        return true;
    }
    // System trees on any drive stay protected, including their descendants. A project
    // subdirectory named "windows" does not by itself identify an operating-system tree.
    if parts.len() >= 2
        && matches!(
            parts[1],
            "windows" | "recovery" | "config.msi" | "$windows.~bt" | "$windows.~ws" | "boot"
        )
    {
        return true;
    }
    if parts.len() == 2
        && matches!(
            parts[1],
            "users" | "bootmgr" | "pagefile.sys" | "swapfile.sys" | "hiberfil.sys"
        )
    {
        return true;
    }
    false
}

pub(crate) fn capability(entry: &AnalysisEntry) -> ManualCapability {
    let reason = if entry.excluded {
        Some(ReasonCode::Excluded)
    } else if entry.shared && !entry.directory {
        Some(ReasonCode::LinkOrCloud)
    } else if entry.incomplete || entry.pending || entry.issue.is_some() {
        Some(ReasonCode::ScanFailed)
    } else if !entry.rule_ids.is_empty() || protected(Path::new(&entry.path), entry.directory) {
        Some(ReasonCode::Protected)
    } else {
        None
    };
    ManualCapability {
        allowed: reason.is_none(),
        reason,
    }
}
pub(crate) fn delete(record: &ManualRecord, exclusions: &[String]) -> Result<()> {
    let snapshot: Snapshot = serde_json::from_str(&record.file.opaque)?;
    let path = extended(&from_native(&snapshot.path))?;
    let root = extended(&from_native(&snapshot.root))?;
    ensure!(
        path.starts_with(&root) && path != root,
        Refusal(ReasonCode::Protected)
    );
    ensure!(
        !protected(&path, record.directory),
        Refusal(ReasonCode::Protected)
    );
    ensure!(!excluded(&path, exclusions), Refusal(ReasonCode::Excluded));
    is_fixed_ntfs(&path)?;
    let pin = PinnedPath::open(&path, record.directory, true)?;
    ensure!(
        pin.parent_ids == snapshot.parents,
        Refusal(ReasonCode::Changed)
    );
    if record.directory {
        // Our child deletions change directory times. Identity/creation/attributes must
        // still agree; Windows disposition refuses a directory with any remaining child.
        ensure!(
            pin.stamp.identity == snapshot.stamp.identity
                && pin.stamp.created == snapshot.stamp.created
                && pin.stamp.attributes == snapshot.stamp.attributes,
            Refusal(ReasonCode::Changed)
        );
        pin.mark_empty_directory_delete()
    } else {
        ensure!(pin.stamp == snapshot.stamp, Refusal(ReasonCode::Changed));
        pin.mark_delete()
    }
}
pub(crate) fn allowed_stamp(stamp: &Stamp) -> Result<()> {
    ensure!(
        stamp.attributes
            & (FILE_ATTRIBUTE_REPARSE_POINT.0
                | FILE_ATTRIBUTE_OFFLINE.0
                | FILE_ATTRIBUTE_SYSTEM.0
                | FILE_ATTRIBUTE_ENCRYPTED.0
                | 0x40000
                | 0x400000)
            == 0,
        Refusal(ReasonCode::Protected)
    );
    if stamp.attributes & FILE_ATTRIBUTE_DIRECTORY.0 == 0 {
        ensure!(stamp.links == 1, Refusal(ReasonCode::LinkOrCloud));
        ensure!(
            stamp.attributes & FILE_ATTRIBUTE_READONLY.0 == 0,
            Refusal(ReasonCode::Protected)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_trees_protect_descendants_without_blocking_named_project_subdirectories() {
        assert!(protected(
            Path::new(r"Z:\Windows\System32\kernel32.dll"),
            false
        ));
        assert!(protected(Path::new(r"Z:\Recovery\settings.bin"), false));
        assert!(protected(Path::new(r"Z:\Boot\BCD"), false));
        assert!(protected(Path::new(r"Z:\Users"), true));
        assert!(!protected(
            Path::new(r"Z:\Projects\windows\notes.txt"),
            false
        ));
    }
}
