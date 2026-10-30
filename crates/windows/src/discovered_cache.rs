//! Bounded, metadata-only discovery beyond AppData. Never grants deletion permission.
use crate::{app_cache::Inventory, fs::*, platform::volume_target};
use anyhow::{Result, ensure};
use dustaway_core::{ReasonCode, VolumeTarget, app_cache::CacheSpec};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

#[cfg(not(test))]
pub(crate) fn fixed_drives(cancel: Option<&AtomicBool>) -> Result<Vec<PathBuf>> {
    use windows::{
        Win32::{
            Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives},
            System::WindowsProgramming::DRIVE_FIXED,
        },
        core::PCWSTR,
    };
    check_cancel(cancel)?;
    let mask = unsafe { GetLogicalDrives() };
    ensure!(mask != 0, "无法枚举本地磁盘");
    let mut drives = vec![];
    for n in 0..26 {
        check_cancel(cancel)?;
        if mask & (1 << n) != 0 {
            let path = PathBuf::from(format!("{}:\\", char::from(b'A' + n)));
            if unsafe { GetDriveTypeW(PCWSTR(wide(&path).as_ptr())) } == DRIVE_FIXED {
                drives.push(path);
            }
        }
    }
    Ok(drives)
}

/// Accept versioned update directories, never generic names containing "update".
pub(crate) fn dingtalk_update_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let Some((generation, versions)) = name
        .strip_prefix("dingtalkupdate_v")
        .and_then(|s| s.split_once('_'))
    else {
        return false;
    };
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !numeric(generation) {
        return false;
    }
    let versions: Vec<_> = versions.split('-').collect();
    (1..=2).contains(&versions.len())
        && versions.iter().all(|v| {
            let parts: Vec<_> = v.split('.').collect();
            parts.len() >= 2 && parts.iter().all(|p| numeric(p))
        })
}

pub(crate) struct Discovery<'a> {
    pub drives: &'a [PathBuf],
    pub local: &'a Path,
    pub roaming: &'a Path,
    pub inventory: &'a Inventory,
    pub volumes: Option<&'a [VolumeTarget]>,
    pub exclusions: &'a CompiledExclusions,
    pub cancel: Option<&'a AtomicBool>,
}
impl Discovery<'_> {
    fn pin(&self, path: &Path, directory: bool) -> Result<Option<PinnedPath>> {
        check_cancel(self.cancel)?;
        if self.exclusions.contains(path) || !path.try_exists()? {
            return Ok(None);
        }
        PinnedPath::observe(path, directory, self.cancel).map(Some)
    }
    fn marker(&self, base: &Path, name: &str, directory: bool) -> Result<bool> {
        Ok(self.pin(&base.join(name), directory)?.is_some())
    }
    fn product(&self, spec: &CacheSpec, base: &Path) -> Result<bool> {
        match spec.app.as_str() {
            "dingtalk" => Ok(self.marker(base, "DingTalk.exe", false)?
                || (self.marker(base, "config", true)?
                    && (self.marker(base, "globalStorage", true)?
                        || self.marker(base, "defEmotion", true)?))),
            "gameviewer" => Ok(self.marker(base, "GameViewer.exe", false)?
                || self.marker(base, "bin/GameViewer.exe", false)?),
            "hbuilder" => self.marker(base, "HBuilderX.exe", false),
            _ => Ok(false),
        }
    }
    pub(crate) fn roots(
        &self,
        spec: &CacheSpec,
        issue: &mut dyn FnMut(ReasonCode),
    ) -> Result<Vec<PathBuf>> {
        ensure!(
            spec.folder == "discovered" && !spec.cleanable(),
            "仅支持已编译的只读识别"
        );
        check_cancel(self.cancel)?;
        let mut selected = vec![];
        for drive in self.drives {
            check_cancel(self.cancel)?;
            match volume_target(drive) {
                Ok(v)
                    if self
                        .volumes
                        .is_none_or(|vs| vs.iter().any(|s| s.key == v.key)) =>
                {
                    selected.push(extended(drive)?)
                }
                Ok(_) => {}
                Err(e) => {
                    if self.volumes.is_none_or(|vs| {
                        vs.iter()
                            .any(|s| s.label.eq_ignore_ascii_case(&display_path(drive)))
                    }) {
                        issue(error_reason(&e));
                    }
                }
            }
        }
        let in_scope = |p: &Path| {
            let path = display_path(p).to_lowercase();
            selected.iter().any(|drive| {
                let drive = display_path(drive).trim_end_matches('\\').to_lowercase();
                path == drive
                    || path
                        .strip_prefix(&drive)
                        .is_some_and(|tail| tail.starts_with('\\'))
            })
        };
        let mut bases: Vec<PathBuf> = selected
            .iter()
            .flat_map(|d| spec.paths.iter().map(move |p| d.join(p)))
            .collect();
        // Standard update locations are additional discovery seeds; standard DingTalk logs
        // remain owned by their existing rule and must not be counted a second time.
        if spec.id == "appcache.dingtalk_updates" {
            bases.extend([self.local.join("DingTalk"), self.roaming.join("DingTalk")]);
        }
        for entry in &self.inventory.entries {
            check_cancel(self.cancel)?;
            if dustaway_core::app_cache::registered_name_matches(
                &entry.name,
                &spec.aliases.iter().map(String::as_str).collect::<Vec<_>>(),
            ) && let Some(location) = &entry.install_location
                && let Ok(base) = extended(Path::new(location))
                && in_scope(&base)
            {
                bases.push(base.clone());
                // Some registrations point to the executable's bin directory.
                if spec.app == "gameviewer"
                    && base
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
                    && let Some(parent) = base.parent()
                {
                    bases.push(parent.to_path_buf());
                }
            }
        }
        let mut seen_bases = HashSet::new();
        let mut seen_roots = HashSet::new();
        let mut roots = vec![];
        let standard_logs = extended(&self.roaming.join("DingTalk"))?;
        for base in bases {
            check_cancel(self.cancel)?;
            let base = extended(&base)?;
            if !in_scope(&base) || !seen_bases.insert(display_path(&base).to_lowercase()) {
                continue;
            }
            if self.exclusions.contains(&base) {
                issue(ReasonCode::Excluded);
                continue;
            }
            // A registered location may coincide with the existing AppData log scope.
            if spec.id == "appcache.dingtalk_custom_logs"
                && display_path(&base)
                    .trim_end_matches('\\')
                    .eq_ignore_ascii_case(&display_path(&standard_logs))
            {
                continue;
            }
            let result = (|| -> Result<()> {
                let Some(_base_pin) = self.pin(&base, true)? else {
                    return Ok(());
                };
                let mut add = |path: PathBuf| -> Result<()> {
                    if let Some(pin) = self.pin(&path, true)?
                        && seen_roots.insert((pin.stamp.identity.volume, pin.stamp.identity.index))
                    {
                        roots.push(pin.path);
                    }
                    Ok(())
                };
                if spec.id == "appcache.dingtalk_updates" {
                    for entry in std::fs::read_dir(&base)? {
                        check_cancel(self.cancel)?;
                        let result = (|| -> Result<()> {
                            let entry = entry?;
                            if !dingtalk_update_name(&entry.file_name().to_string_lossy()) {
                                return Ok(());
                            }
                            let path = entry.path();
                            if self.exclusions.contains(&path)
                                || self
                                    .exclusions
                                    .contains(&path.join("upgrade_manifest.json"))
                            {
                                issue(ReasonCode::Excluded);
                                return Ok(());
                            }
                            let Some(_pin) = self.pin(&path, true)? else {
                                return Ok(());
                            };
                            // Read metadata of the manifest, never its contents or chat files.
                            if self.marker(&path, "upgrade_manifest.json", false)? {
                                add(path)?;
                            }
                            Ok(())
                        })();
                        if let Err(e) = result {
                            if e.is::<ScanCancelled>() {
                                return Err(e);
                            }
                            issue(error_reason(&e));
                        }
                    }
                } else if self.product(spec, &base)? {
                    let leaves: &[&str] = match spec.id.as_str() {
                        "appcache.dingtalk_custom_logs" => {
                            &["log", "holmeslogs", "recordLog", "updaterlogs"]
                        }
                        "appcache.gameviewer_logs" => &["log"],
                        "appcache.hbuilder_backup" => &["update/backup"],
                        _ => &[],
                    };
                    for leaf in leaves {
                        if self.exclusions.contains(&base.join(leaf)) {
                            issue(ReasonCode::Excluded);
                            continue;
                        }
                        if let Err(e) = add(base.join(leaf)) {
                            if e.is::<ScanCancelled>() {
                                return Err(e);
                            }
                            issue(error_reason(&e));
                        }
                    }
                }
                Ok(())
            })();
            if let Err(e) = result {
                if e.is::<ScanCancelled>() {
                    return Err(e);
                }
                issue(error_reason(&e));
            }
        }
        roots.sort();
        Ok(roots)
    }
}
