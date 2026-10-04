//! Shared profile recognition only. Returning a location never grants deletion permission.
use crate::{
    fs::*,
    platform::{intentional_skip, profile_marker},
};
use anyhow::{Result, ensure};
use dustaway_core::{ReasonCode, Refusal};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub(crate) fn chromium(
    local: &Path,
    family: &str,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    let product =
        dustaway_core::browsers::chromium(family).ok_or(Refusal(ReasonCode::Protected))?;
    let base = local.join(product.user_data);
    if !base
        .try_exists()
        .map_err(|_| Refusal(ReasonCode::DirectoryInaccessible))?
    {
        return Ok(vec![]);
    }
    let _pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
    let mut profiles = vec![];
    for entry in std::fs::read_dir(&base)? {
        check_cancel(cancel)?;
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                issue(error_reason(&error.into()));
                continue;
            }
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        if !dustaway_core::browsers::standard_profile(&name) {
            continue;
        }
        let profile = entry.path();
        let _profile_pin = match PinnedPath::open_cancellable(&profile, true, false, cancel) {
            Ok(pin) => pin,
            Err(error) if error.is::<ScanCancelled>() => return Err(error),
            Err(error) => {
                let code = error_reason(&error);
                if !intentional_skip(code) {
                    issue(code);
                }
                continue;
            }
        };
        if profile_marker(&profile.join("Preferences"), cancel, issue)? {
            profiles.push(profile);
        }
    }
    Ok(profiles)
}

pub(crate) fn firefox(
    roaming: &Path,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    let base = roaming.join("Mozilla/Firefox");
    let config = base.join("profiles.ini");
    if !config
        .try_exists()
        .map_err(|_| Refusal(ReasonCode::DirectoryInaccessible))?
    {
        return Ok(vec![]);
    }
    let pin = PinnedPath::open_cancellable(&config, false, false, cancel)?;
    ensure!(pin.stamp.size < 1024 * 1024, "Firefox 配置异常，已跳过");
    let content = std::fs::read_to_string(&config)?;
    let mut relative = false;
    let mut path = None;
    let mut is_profile = false;
    let mut profiles = vec![];
    let mut flush = |relative: bool, path: &Option<String>, is_profile: bool| -> Result<()> {
        check_cancel(cancel)?;
        if !relative || !is_profile {
            return Ok(());
        }
        let Some(path) = path else { return Ok(()) };
        let normalized = path.replace('\\', "/");
        let parts: Vec<_> = normalized.split('/').collect();
        if parts.len() != 2
            || parts[0] != "Profiles"
            || parts[1].is_empty()
            || parts[1] == "."
            || parts[1] == ".."
            || parts[1].contains(':')
        {
            return Ok(());
        }
        let profile = base.join("Profiles").join(parts[1]);
        if profile_marker(&profile.join("prefs.js"), cancel, issue)? {
            profiles.push(profile);
        }
        Ok(())
    };
    for line in content.trim_start_matches('\u{feff}').lines() {
        check_cancel(cancel)?;
        let line = line.trim();
        if line.starts_with('[') {
            flush(relative, &path, is_profile)?;
            relative = false;
            path = None;
            is_profile = line
                .strip_prefix("[Profile")
                .and_then(|name| name.strip_suffix(']'))
                .is_some_and(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()));
        } else if let Some(value) = line.strip_prefix("IsRelative=") {
            relative = value == "1";
        } else if let Some(value) = line.strip_prefix("Path=") {
            path = Some(value.to_owned());
        }
    }
    flush(relative, &path, is_profile)?;
    profiles.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    profiles.dedup_by(|a, b| {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    });
    Ok(profiles)
}
