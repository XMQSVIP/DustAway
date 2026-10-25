use crate::{fs::*, security};
use anyhow::{Context, Result, ensure};
use dustaway_core::{
    FileRecord, Platform, ReasonCode, Refusal, RuleDescriptor, ScanEvent, Scope, now,
};
use std::{
    collections::HashSet,
    ffi::OsString,
    os::windows::{ffi::OsStringExt, fs::MetadataExt},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
use windows::{
    Win32::{
        System::{Com::CoTaskMemFree, SystemInformation::GetWindowsDirectoryW},
        UI::Shell::{
            FOLDERID_LocalAppData, FOLDERID_ProgramData, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT,
            SHGetKnownFolderPath,
        },
    },
    core::GUID,
};

#[cfg(test)]
#[path = "browser_acceptance.rs"]
mod browser_acceptance;

#[cfg(test)]
#[path = "scan_benchmark.rs"]
mod scan_benchmark;

#[cfg(test)]
#[path = "scan_tests.rs"]
mod scan_tests;

#[cfg(test)]
#[path = "application_acceptance_tests.rs"]
mod application_acceptance_tests;

#[cfg(test)]
#[path = "real_application_acceptance.rs"]
mod real_application_acceptance;

pub fn known_folder(id: &GUID) -> Result<PathBuf> {
    unsafe {
        let value = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)?;
        let path = PathBuf::from(OsString::from_wide(value.as_wide()));
        CoTaskMemFree(Some(value.0.cast()));
        Ok(path)
    }
}
pub fn data_dir() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_LocalAppData)?.join("DustAway"))
}
pub fn system_data_dir() -> Result<PathBuf> {
    Ok(known_folder(&FOLDERID_ProgramData)?.join("DustAway"))
}
pub(crate) fn windows_dir() -> Result<PathBuf> {
    let mut buf = [0u16; 32768];
    let len = unsafe { GetWindowsDirectoryW(Some(&mut buf)) } as usize;
    ensure!(len > 0 && len < buf.len(), "无法读取 Windows 目录");
    Ok(PathBuf::from(OsString::from_wide(&buf[..len])))
}

pub struct WindowsPlatform {
    inventory: std::sync::Mutex<Option<crate::app_cache::Inventory>>,
    tool_roots: std::sync::Mutex<std::collections::HashMap<String, dustaway_core::ToolDiscovery>>,
    tool_probe: std::sync::Arc<dyn crate::tool_discovery::Probe>,
    scope: Scope,
    local: PathBuf,
    roaming: PathBuf,
    system: PathBuf,
    #[cfg(test)]
    app_test_identity: Option<String>,
    #[cfg(test)]
    app_test_acceptance: Option<std::sync::Arc<application_acceptance_tests::SyntheticAcceptance>>,
    #[cfg(test)]
    real_app_acceptance: Option<std::sync::Arc<real_application_acceptance::RealAcceptance>>,
}
impl WindowsPlatform {
    fn cache_identity(
        &self,
        spec: &dustaway_core::app_cache::CacheSpec,
    ) -> Result<crate::app_cache::CodeIdentity> {
        #[cfg(test)]
        if let Some(identity) = &self.app_test_identity {
            return Ok(crate::app_cache::CodeIdentity::fixture(identity.clone()));
        }
        crate::app_cache::code_identity(spec)
    }
    pub fn new(scope: Scope) -> Result<Self> {
        Ok(Self {
            inventory: Default::default(),
            tool_roots: Default::default(),
            tool_probe: std::sync::Arc::new(crate::tool_discovery::InstalledProbe),
            #[cfg(test)]
            app_test_identity: None,
            #[cfg(test)]
            app_test_acceptance: None,
            #[cfg(test)]
            real_app_acceptance: None,
            scope,
            local: extended(&known_folder(&FOLDERID_LocalAppData)?)?,
            roaming: extended(&known_folder(&FOLDERID_RoamingAppData)?)?,
            system: extended(&windows_dir()?)?,
        })
    }
    pub(crate) fn roots(&self, rule: &RuleDescriptor) -> Result<Vec<PathBuf>> {
        self.discover_roots(rule, None, &mut |_| {})
    }
    fn discover_roots(
        &self,
        rule: &RuleDescriptor,
        cancel: Option<&AtomicBool>,
        issue: &mut dyn FnMut(ReasonCode),
    ) -> Result<Vec<PathBuf>> {
        check_cancel(cancel)?;
        ensure!(
            rule.requires_admin == (self.scope == Scope::System),
            "不允许跨权限域操作"
        );
        dustaway_core::rules::validate(rule)?;
        if let Some(spec) = dustaway_core::app_cache::rule(&rule.id) {
            return crate::app_cache::roots(&self.local, &self.roaming, spec, cancel, issue);
        }
        if let Some(spec) = dustaway_core::developer::rule(&rule.id) {
            #[cfg(not(test))]
            let home = known_folder(&windows::Win32::UI::Shell::FOLDERID_Profile)?;
            #[cfg(test)]
            let home = self.local.parent().unwrap().join("home");
            let mut found =
                crate::developer::roots(&self.local, &self.roaming, &home, spec, cancel, issue)?;
            if crate::tool_discovery::supported(&rule.id) {
                if let Some(cancel) = cancel {
                    self.tool_roots.lock().unwrap().remove(&rule.id);
                    let discovered =
                        crate::tool_discovery::inspect(self.tool_probe.as_ref(), &rule.id, cancel)?;
                    self.tool_roots
                        .lock()
                        .unwrap()
                        .insert(rule.id.clone(), discovered);
                }
                if let Some(target) = self
                    .tool_roots
                    .lock()
                    .unwrap()
                    .get(&rule.id)
                    .and_then(|t| t.target.as_ref())
                {
                    let root = extended(Path::new(target))?;
                    if !found
                        .iter()
                        .any(|p| display_path(p).eq_ignore_ascii_case(&display_path(&root)))
                    {
                        found.push(root);
                    }
                }
            }
            return Ok(found);
        }
        if let Some(spec) = dustaway_core::residue::rule(&rule.id) {
            return crate::residue::roots(&self.local, spec, cancel, issue);
        }
        if let Some(spec) = dustaway_core::applications::rule(&rule.id) {
            // Installation proof only gates deletion; recognized remnants remain visible.
            return crate::application_layouts::inspect(
                &self.local,
                &self.roaming,
                spec,
                cancel,
                issue,
            )
            .map(|found| found.roots);
        }
        match rule.root.as_str() {
            "user_temp" => Ok(vec![extended(&self.local.join("Temp"))?]),
            "system_temp" => Ok(vec![extended(&self.system.join("Temp"))?]),
            "iis_logs" => {
                if !self
                    .system
                    .join("System32/inetsrv/config/applicationHost.config")
                    .is_file()
                {
                    return Ok(vec![]);
                }
                let base = self
                    .system
                    .parent()
                    .context("无效系统目录")?
                    .join("inetpub/logs/LogFiles");
                if !base.try_exists()? {
                    return Ok(vec![]);
                }
                let _pin = PinnedPath::open_cancellable(&base, true, false, cancel)?;
                let mut roots = vec![];
                for entry in std::fs::read_dir(base)? {
                    check_cancel(cancel)?;
                    let entry = entry?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry.file_type()?.is_dir()
                        && name
                            .strip_prefix("W3SVC")
                            .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
                    {
                        roots.push(extended(&entry.path())?);
                    }
                }
                Ok(roots)
            }
            root if dustaway_core::browsers::chromium(root).is_some() => {
                crate::profile_locations::chromium(&self.local, root, cancel, issue).map(
                    |profiles| {
                        profiles
                            .into_iter()
                            .map(|profile| profile.join(&rule.relative))
                            .collect()
                    },
                )
            }
            "firefox" => {
                crate::profile_locations::firefox(&self.roaming, cancel, issue).map(|profiles| {
                    profiles
                        .into_iter()
                        .map(|profile| {
                            self.local
                                .join("Mozilla/Firefox/Profiles")
                                .join(profile.file_name().expect("verified standard profile"))
                                .join("cache2")
                        })
                        .collect()
                })
            }
            _ => anyhow::bail!("未识别的清理规则"),
        }
    }

    fn application_identity(&self, rule: &RuleDescriptor) -> Result<Option<String>> {
        let Some(spec) = dustaway_core::applications::rule(&rule.id) else {
            return Ok(None);
        };
        #[cfg(test)]
        if let Some(acceptance) = &self.app_test_acceptance {
            acceptance.check_boundary(self)?;
            return Ok(Some(acceptance.identity()));
        }
        #[cfg(test)]
        if let Some(identity) = &self.app_test_identity {
            return Ok(Some(identity.clone()));
        }
        crate::applications::identify(dustaway_core::applications::application(spec.app).unwrap())?
            .fingerprint()
            .map(Some)
    }

    fn cleanup_application_pin(
        &self,
        rule: &RuleDescriptor,
    ) -> Result<Option<crate::applications::VerifiedApplication>> {
        #[cfg(test)]
        if let Some(acceptance) = &self.real_app_acceptance {
            return acceptance.require(self, rule).map(Some);
        }
        #[cfg(test)]
        if let Some(acceptance) = &self.app_test_acceptance {
            acceptance.check_boundary(self)?;
            ensure!(
                acceptance.enabled(self, rule),
                Refusal(ReasonCode::ValidationPending)
            );
            return Ok(None);
        }
        crate::applications::require_supported(rule).map(Some)
    }
}

impl Platform for WindowsPlatform {
    fn tool_discovery(&self, rule: &str) -> Option<dustaway_core::ToolDiscovery> {
        self.tool_roots.lock().unwrap().get(rule).cloned()
    }
    fn invalidate_tool_discovery(&self, rule: Option<&str>) {
        let mut cache = self.tool_roots.lock().unwrap();
        if let Some(rule) = rule {
            cache.remove(rule);
        } else {
            cache.clear();
            *self.inventory.lock().unwrap() = None;
        }
    }

    fn cleanup_validated(&self, rule: &RuleDescriptor) -> bool {
        #[cfg(test)]
        if let Some(acceptance) = &self.real_app_acceptance {
            return acceptance.require(self, rule).is_ok();
        }
        #[cfg(test)]
        if let Some(acceptance) = &self.app_test_acceptance {
            return acceptance.enabled(self, rule);
        }
        dustaway_core::cleanup_acceptance::release_enabled(rule)
    }
    fn validate_candidate(&self, rule: &RuleDescriptor, file: &FileRecord) -> Result<()> {
        dustaway_core::rules::validate(rule)?;
        if matches!(rule.root.as_str(), "user_temp" | "system_temp" | "iis_logs") {
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            let root = from_native(&snapshot.root);
            let path = from_native(&snapshot.path);
            ensure!(
                self.roots(rule)?.contains(&root),
                Refusal(ReasonCode::Changed)
            );
            if rule.root == "iis_logs" {
                ensure!(
                    path.parent() == Some(root.as_path()) && iis_log_file(&path),
                    Refusal(ReasonCode::Protected)
                );
            }
            candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
            ensure!(
                snapshot.stamp.modified_unix() <= now() - i64::from(rule.min_age_days) * 86400,
                Refusal(ReasonCode::Recent)
            );
        }
        if let Some(spec) = dustaway_core::app_cache::rule(&rule.id) {
            ensure!(spec.cleanable(), Refusal(ReasonCode::ValidationPending));
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            let identity = self.cache_identity(spec)?;
            ensure!(
                snapshot.application_identity.as_ref() == Some(&identity.fingerprint),
                Refusal(ReasonCode::Changed)
            );
            let root = from_native(&snapshot.root);
            ensure!(
                self.roots(rule)?.contains(&root),
                Refusal(ReasonCode::Changed)
            );
            if let Some(reason) = crate::app_cache::file_reason(
                spec,
                &root,
                &from_native(&snapshot.path),
                Some(&identity),
                None,
            )? {
                return Err(Refusal(reason).into());
            }
            candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
            ensure!(
                snapshot.stamp.modified_unix() <= now() - i64::from(rule.min_age_days) * 86400,
                Refusal(ReasonCode::Recent)
            );
        }
        if dustaway_core::developer::rule(&rule.id).is_some() {
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            let root = from_native(&snapshot.root);
            ensure!(
                self.roots(rule)?.contains(&root),
                Refusal(ReasonCode::Changed)
            );
            ensure!(
                crate::developer::accepts_file(&rule.id, &root, &from_native(&snapshot.path)),
                Refusal(ReasonCode::Protected)
            );
            candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
            ensure!(
                snapshot.stamp.modified_unix() <= now() - i64::from(rule.min_age_days) * 86400,
                Refusal(ReasonCode::Recent)
            );
        }
        if dustaway_core::residue::rule(&rule.id).is_some() {
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            let root = from_native(&snapshot.root);
            ensure!(
                self.roots(rule)?.contains(&root),
                Refusal(ReasonCode::Changed)
            );
            ensure!(
                crate::residue::accepts_file(&rule.id, &root, &from_native(&snapshot.path)),
                Refusal(ReasonCode::Protected)
            );
            candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
            ensure!(
                snapshot.stamp.modified_unix() <= now() - i64::from(rule.min_age_days) * 86400,
                Refusal(ReasonCode::Recent)
            );
        }
        if dustaway_core::applications::rule(&rule.id).is_some() {
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            ensure!(
                self.application_identity(rule)? == snapshot.application_identity
                    && snapshot.application_identity.is_some(),
                Refusal(ReasonCode::Changed)
            );
            // Report changed identity/metadata before interpreting a changed file's bytes.
            candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
            ensure!(
                dustaway_core::applications::accepts_file(&rule.id, &from_native(&snapshot.path)),
                Refusal(ReasonCode::Protected)
            );
            let root = extended(&from_native(&snapshot.root))?;
            ensure!(
                crate::weixin_cache::accepts(&rule.id, &root, &from_native(&snapshot.path))?,
                Refusal(ReasonCode::Protected)
            );
            ensure!(
                self.roots(rule)?
                    .iter()
                    .any(|p| extended(p).is_ok_and(|p| p == root)),
                Refusal(ReasonCode::Changed)
            );
            ensure!(
                snapshot.stamp.modified_unix() <= now() - i64::from(rule.min_age_days) * 86400,
                Refusal(ReasonCode::Recent)
            );
        }
        Ok(())
    }
    fn file_volume(&self, file: &FileRecord) -> Result<dustaway_core::VolumeTarget> {
        let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
        volume_target(&from_native(&snapshot.path))
    }
    fn volume_available(&self, volume: &dustaway_core::VolumeTarget) -> Result<u64> {
        free_bytes(Path::new(&volume.opaque))
    }
    fn candidate_root(&self, file: &FileRecord) -> Option<String> {
        serde_json::from_str::<Snapshot>(&file.opaque)
            .ok()
            .map(|s| display_path(&from_native(&s.root)))
    }
    fn candidate_action(
        &self,
        rule: &RuleDescriptor,
        file: &FileRecord,
        kind: dustaway_core::ExclusionKind,
        reveal: bool,
    ) -> Result<String> {
        let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
        ensure!(
            self.roots(rule)?.contains(&from_native(&snapshot.root)),
            Refusal(ReasonCode::Changed)
        );
        candidate_action_snapshot(&snapshot, kind, reveal)
    }
    fn scan(
        &self,
        rule: &RuleDescriptor,
        exclusions: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        self.scan_on_volumes(rule, exclusions, None, cancel, emit)
    }
    fn scan_on_volumes(
        &self,
        rule: &RuleDescriptor,
        exclusions: &[String],
        volumes: Option<&[dustaway_core::VolumeTarget]>,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        ensure!(volumes.is_none_or(|v| !v.is_empty()), "请选择至少一个磁盘");
        let cache_spec = dustaway_core::app_cache::rule(&rule.id);
        let code_identity = cache_spec
            .filter(|s| s.cleanable())
            .and_then(|s| self.cache_identity(s).ok());
        let application_identity = if let Some(identity) = &code_identity {
            Some(identity.fingerprint.clone())
        } else {
            self.application_identity(rule).ok().flatten()
        };
        let cutoff = now() - i64::from(rule.min_age_days) * 86400;
        let raw_exclusions = exclusions;
        let exclusions = CompiledExclusions::new(exclusions);
        let outcome = (|| -> Result<()> {
            let mut issues = 0;
            let mut application_notice = None;
            let mut coverage = |code| {
                issues += 1;
                emit(ScanEvent::CoverageIssue(code));
            };
            let discovery = if let Some(spec) = dustaway_core::applications::rule(&rule.id) {
                dustaway_core::rules::validate(rule)?;
                ensure!(self.scope == Scope::User, "不允许跨权限域操作");
                crate::application_layouts::inspect(
                    &self.local,
                    &self.roaming,
                    spec,
                    Some(cancel),
                    &mut coverage,
                )
                .map(|found| {
                    application_notice = found.unavailable;
                    found.roots
                })
            } else {
                self.discover_roots(rule, Some(cancel), &mut coverage)
            };
            let roots = match discovery {
                Ok(roots) => roots,
                Err(error) => {
                    if !error.is::<ScanCancelled>() {
                        emit(ScanEvent::CoverageIssue(error_reason(&error)));
                    }
                    return Err(error);
                }
            };
            if let Some(spec) = cache_spec {
                let mut inventory = self.inventory.lock().unwrap();
                if inventory.is_none() {
                    *inventory = Some(crate::app_cache::Inventory::read(Some(cancel))?);
                }
                let mut observed = inventory.as_ref().unwrap().observation(spec, &roots);
                if code_identity.is_some() {
                    observed.identity_status = "verified".into();
                }
                emit(ScanEvent::ApplicationObserved(observed));
            } else if let Some(spec) = dustaway_core::applications::rule(&rule.id) {
                let mut inventory = self.inventory.lock().unwrap();
                if inventory.is_none() {
                    *inventory = Some(crate::app_cache::Inventory::read(Some(cancel))?);
                }
                if let Some(app) = dustaway_core::applications::APPLICATIONS
                    .iter()
                    .find(|a| a.id == spec.app)
                {
                    let mut observed =
                        inventory
                            .as_ref()
                            .unwrap()
                            .observe(app.title, &[app.title], &roots);
                    if application_identity.is_some() {
                        observed.identity_status = "verified".into();
                    }
                    emit(ScanEvent::ApplicationObserved(observed));
                }
            }
            check_cancel(Some(cancel))?;
            if let Some(tool) = self.tool_roots.lock().unwrap().get_mut(&rule.id) {
                crate::tool_discovery::constrain(tool, volumes, raw_exclusions);
            }
            ensure!(
                !roots.is_empty() || issues == 0,
                Refusal(ReasonCode::DirectoryInaccessible)
            );
            if roots.is_empty() {
                let base = if let Some(product) = dustaway_core::browsers::chromium(&rule.root) {
                    Some(self.local.join(product.user_data))
                } else if rule.root == "firefox" {
                    Some(self.roaming.join("Mozilla/Firefox/profiles.ini"))
                } else {
                    None
                };
                let missing = cache_spec.is_some_and(|s| s.kind == "readonly_observed")
                    || base.is_some_and(|p| p.try_exists().is_ok_and(|exists| !exists));
                emit(ScanEvent::NoticeCode(application_notice.unwrap_or(
                    if missing {
                        ReasonCode::LocationMissing
                    } else {
                        ReasonCode::UnsupportedLayout
                    },
                )));
            }
            let mut seen = HashSet::new();
            let mut observed_seen = HashSet::new();
            let mut covered = roots.is_empty();
            let mut selected_roots = 0;
            let mut missing_roots = 0;
            for root in roots {
                check_cancel(Some(cancel))?;
                if let Some(volumes) = volumes {
                    let target = volume_target(&root)?;
                    if !volumes.iter().any(|v| v.key == target.key) {
                        covered = true;
                        continue;
                    }
                }
                selected_roots += 1;
                if exclusions.contains(&root) {
                    covered = true;
                    emit(ScanEvent::Retained(ReasonCode::Excluded));
                    continue;
                }
                match root.try_exists() {
                    Ok(false) => {
                        covered = true;
                        missing_roots += 1;
                        continue;
                    }
                    Ok(true) => {}
                    Err(error) => {
                        emit(ScanEvent::CoverageIssue(error_reason(&error.into())));
                        continue;
                    }
                }
                is_fixed_ntfs(&root)?;
                let _root_pin = match PinnedPath::open_cancellable(&root, true, false, Some(cancel))
                {
                    Ok(p) => p,
                    Err(error) if error.is::<ScanCancelled>() => return Err(error),
                    Err(error) => {
                        let code = error_reason(&error);
                        if intentional_skip(code) {
                            covered = true;
                            emit(ScanEvent::Retained(code));
                        } else {
                            emit(ScanEvent::CoverageIssue(code));
                        }
                        continue;
                    }
                };
                let mut stack = vec![root.clone()];
                while let Some(dir) = stack.pop() {
                    check_cancel(Some(cancel))?;
                    if exclusions.contains(&dir) {
                        emit(ScanEvent::Retained(ReasonCode::Excluded));
                        continue;
                    }
                    let _pin = match PinnedPath::open_cancellable(&dir, true, false, Some(cancel)) {
                        Ok(p) => p,
                        Err(error) if error.is::<ScanCancelled>() => return Err(error),
                        Err(e) => {
                            let code = error_reason(&e);
                            emit(if intentional_skip(code) {
                                ScanEvent::Retained(code)
                            } else {
                                ScanEvent::CoverageIssue(code)
                            });
                            continue;
                        }
                    };
                    let entries = match std::fs::read_dir(&dir) {
                        Ok(entries) => entries,
                        Err(e) => {
                            emit(ScanEvent::CoverageIssue(error_reason(&e.into())));
                            continue;
                        }
                    };
                    let mut entry_failed = false;
                    let mut entry_read = false;
                    for entry in entries {
                        check_cancel(Some(cancel))?;
                        let entry = match entry {
                            Ok(e) => e,
                            Err(e) => {
                                entry_failed = true;
                                emit(ScanEvent::CoverageIssue(error_reason(&e.into())));
                                continue;
                            }
                        };
                        entry_read = true;
                        emit(ScanEvent::Checked);
                        let path = entry.path();
                        // Count a read-only observed file once even when multiple fixed
                        // roots or hard-link names reach it, including retained files.
                        if cache_spec.is_some_and(|s| s.kind == "readonly_observed")
                            && entry.file_type().is_ok_and(|t| t.is_file())
                            && let Ok(pin) = PinnedPath::observe(&path, false, Some(cancel))
                            && !observed_seen
                                .insert((pin.stamp.identity.volume, pin.stamp.identity.index))
                        {
                            continue;
                        }
                        if exclusions.contains(&path) {
                            retain_observed(emit, &path, ReasonCode::Excluded);
                            continue;
                        }
                        let file_type = match entry.file_type() {
                            Ok(t) => t,
                            Err(e) => {
                                emit(ScanEvent::CoverageIssue(error_reason(&e.into())));
                                continue;
                            }
                        };
                        if file_type.is_symlink() {
                            retain_observed(emit, &path, ReasonCode::LinkOrCloud);
                            continue;
                        }
                        if file_type.is_dir() {
                            if rule.root == "iis_logs"
                                || dustaway_core::residue::rule(&rule.id).is_some()
                            {
                                emit(ScanEvent::Retained(ReasonCode::Protected));
                                continue;
                            }
                            if let Ok(metadata) = entry.metadata()
                                && let Some(reason) =
                                    attribute_refusal(metadata.file_attributes(), true)
                            {
                                emit(ScanEvent::Retained(reason));
                                continue;
                            }
                            if crate::developer::descend(&rule.id, &root, &path) {
                                stack.push(path);
                            }
                            continue;
                        }
                        if !file_type.is_file() {
                            continue;
                        }
                        if rule.root == "iis_logs" && !iis_log_file(&path) {
                            retain_observed(emit, &path, ReasonCode::Protected);
                            continue;
                        }
                        let weixin_file = match crate::weixin_cache::accepts(&rule.id, &root, &path)
                        {
                            Ok(accepted) => accepted,
                            Err(error) => {
                                retain_observed(emit, &path, error_reason(&error));
                                emit(ScanEvent::CoverageIssue(error_reason(&error)));
                                continue;
                            }
                        };
                        if !dustaway_core::applications::accepts_file(&rule.id, &path)
                            || !weixin_file
                            || !crate::residue::accepts_file(&rule.id, &root, &path)
                            || !crate::developer::accepts_file(&rule.id, &root, &path)
                        {
                            if rule.root == "weixin" {
                                retain_observed(emit, &path, ReasonCode::Protected);
                            } else {
                                emit(ScanEvent::Retained(ReasonCode::Protected));
                            }
                            continue;
                        }
                        if let Some(spec) = cache_spec {
                            let reason = match crate::app_cache::file_reason(
                                spec,
                                &root,
                                &path,
                                code_identity.as_ref(),
                                Some(cancel),
                            ) {
                                Ok(reason) => reason,
                                Err(error) if error.is::<ScanCancelled>() => return Err(error),
                                Err(error) => {
                                    emit(ScanEvent::CoverageIssue(error_reason(&error)));
                                    continue;
                                }
                            };
                            if let Some(reason) = reason {
                                if let Ok(meta) = entry.metadata() {
                                    if spec.kind == "vscode_old_code"
                                        && crate::app_cache::recognized_code_file(&root, &path)
                                        && crate::app_cache::valid_code_format(&path)
                                            .unwrap_or(false)
                                        && attribute_refusal(meta.file_attributes(), false)
                                            .is_none()
                                    {
                                        emit(ScanEvent::RetainedFile {
                                            reason,
                                            bytes: meta.len(),
                                        });
                                    } else if spec.kind == "readonly_observed" {
                                        retain_observed(emit, &path, reason);
                                    } else {
                                        emit(ScanEvent::Retained(reason));
                                    }
                                } else {
                                    emit(ScanEvent::CoverageIssue(ReasonCode::AccessDenied));
                                }
                                continue;
                            }
                        }
                        let observe = cache_spec.is_some_and(|s| !s.cleanable())
                            || dustaway_core::developer::rule(&rule.id)
                                .is_some_and(|s| !s.cleanable);
                        if let Ok(metadata) = entry.metadata()
                            && let Some(reason) = prefilter(
                                if observe {
                                    metadata.file_attributes() & !(1 | 4 | 0x4000)
                                } else {
                                    metadata.file_attributes()
                                },
                                metadata.last_write_time(),
                                rule.min_age_days,
                                cutoff,
                            )
                        {
                            retain_observed(emit, &path, reason);
                            continue;
                        }
                        match snapshot_mode(&root, &path, cutoff, Some(cancel), observe) {
                            Ok(mut record) => {
                                let mut snap: Snapshot = serde_json::from_str(&record.opaque)?;
                                if application_identity.is_some() {
                                    snap.application_identity = application_identity.clone();
                                    record.opaque = serde_json::to_string(&snap)?;
                                }
                                if let Some(volumes) = volumes {
                                    let target = volume_target(&path)?;
                                    ensure!(
                                        volumes.iter().any(|v| v.key == target.key),
                                        Refusal(ReasonCode::Changed)
                                    );
                                }
                                if seen
                                    .insert((snap.stamp.identity.volume, snap.stamp.identity.index))
                                {
                                    emit(ScanEvent::File(record));
                                }
                            }
                            Err(e) if e.is::<ScanCancelled>() => return Err(e),
                            Err(e) => retain_observed(emit, &path, error_reason(&e)),
                        }
                    }
                    covered |= entry_read || !entry_failed;
                }
            }
            if volumes.is_some() && selected_roots == 0 {
                emit(ScanEvent::Notice("所选磁盘中没有此类垃圾位置".into()));
            }
            // One profile without a cache must not hide another profile's
            // verified candidates behind a rule-wide missing-location status.
            if selected_roots > 0 && missing_roots == selected_roots {
                emit(ScanEvent::NoticeCode(ReasonCode::LocationMissing));
            }
            ensure!(covered, Refusal(ReasonCode::DirectoryInaccessible));
            Ok(())
        })();
        match outcome {
            Err(error) if error.is::<ScanCancelled>() => Ok(()),
            other => other,
        }
    }
    fn blocked(&self, rule: &RuleDescriptor) -> Result<Option<dustaway_core::BlockReason>> {
        dustaway_core::rules::validate(rule)?;
        #[cfg(test)]
        if let Some(acceptance) = &self.real_app_acceptance {
            return match acceptance.require(self, rule) {
                Ok(found) => crate::applications::blocked_for_verified(rule, &found),
                Err(error) => {
                    let code = error_reason(&error);
                    Ok(Some(dustaway_core::BlockReason::new(code, code.message())))
                }
            };
        }
        #[cfg(test)]
        if let Some(acceptance) = &self.app_test_acceptance {
            return acceptance.blocked(self, rule);
        }
        if crate::tool_discovery::supported(&rule.id) {
            let cached = self.tool_roots.lock().unwrap().get(&rule.id).cloned();
            if let Some(mut tool) = cached {
                if tool.ready
                    || matches!(
                        tool.reason.as_deref(),
                        Some("application_running" | "process_unknown")
                    )
                {
                    let blocked = crate::tool_process::blocked(
                        &rule.id,
                        tool.executable.as_deref().map(Path::new),
                        tool.entry.as_deref().map(Path::new),
                        &AtomicBool::new(false),
                    )
                    .unwrap_or_else(|_| {
                        Some(dustaway_core::BlockReason::new(
                            ReasonCode::ProcessUnknown,
                            "运行状态无法确认，请重新检查",
                        ))
                    });
                    tool.ready = blocked.is_none();
                    tool.reason = blocked.as_ref().map(|b| {
                        if b.code == ReasonCode::ApplicationRunning {
                            "application_running"
                        } else {
                            "process_unknown"
                        }
                        .into()
                    });
                    tool.message = blocked
                        .as_ref()
                        .map(|b| b.message.clone())
                        .unwrap_or_else(|| "运行条件已复查，未查找新增文件".into());
                    self.tool_roots
                        .lock()
                        .unwrap()
                        .insert(rule.id.clone(), tool);
                    return Ok(blocked);
                }
                return Ok(Some(dustaway_core::BlockReason::new(
                    ReasonCode::Unverifiable,
                    tool.message,
                )));
            }
        }
        if let Some(spec) = dustaway_core::app_cache::rule(&rule.id) {
            #[cfg(test)]
            if self.app_test_identity.is_some() && spec.cleanable() {
                return Ok(None);
            }
            return crate::app_cache::blocked(spec);
        }
        if !rule.cleanup_validated && rule.app_id.is_none() {
            return Ok(Some(dustaway_core::BlockReason::new(
                ReasonCode::ValidationPending,
                "本规则尚待隔离 Windows 真实删除验收；仅可扫描和查看，不开放删除",
            )));
        }
        if dustaway_core::applications::rule(&rule.id).is_some() {
            return crate::applications::blocked(rule);
        }
        if let Some(spec) = dustaway_core::developer::rule(&rule.id) {
            return crate::developer::blocked(spec);
        }
        if let Some(process) = &rule.process
            && security::process_running(process)?
        {
            return Ok(Some(dustaway_core::BlockReason::new(
                ReasonCode::BrowserRunning,
                format!(
                    "{} 正在运行，请完全退出（包括后台进程）后重新检查",
                    process.trim_end_matches(".exe")
                ),
            )));
        }
        Ok(None)
    }
    fn delete(
        &self,
        rule: &RuleDescriptor,
        file: &FileRecord,
        exclusions: &[String],
    ) -> Result<()> {
        let result = (|| -> Result<()> {
            dustaway_core::rules::validate(rule)?;
            ensure!(
                self.cleanup_validated(rule),
                Refusal(ReasonCode::ValidationPending)
            );
            let _cache_identity = dustaway_core::app_cache::rule(&rule.id)
                .map(|spec| self.cache_identity(spec))
                .transpose()?;
            self.validate_candidate(rule, file)?;
            if (dustaway_core::developer::rule(&rule.id).is_some()
                || dustaway_core::app_cache::rule(&rule.id).is_some())
                && let Some(reason) = self.blocked(rule)?
            {
                return Err(Refusal(reason.code).into());
            }
            let _application_pin;
            if dustaway_core::applications::rule(&rule.id).is_some() {
                // Enforced again at the filesystem boundary, even for a forged plan.
                ensure!(
                    self.cleanup_validated(rule),
                    Refusal(ReasonCode::ValidationPending)
                );
                if let Some(reason) = self.blocked(rule)? {
                    return Err(Refusal(reason.code).into());
                }
                _application_pin = self.cleanup_application_pin(rule)?;
                self.validate_candidate(rule, file)?;
            }
            let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
            let root = from_native(&snapshot.root);
            // Re-derive roots from the built-in rule. Never trust a snapshot-supplied root.
            ensure!(
                self.roots(rule)?.contains(&root),
                Refusal(ReasonCode::Changed)
            );
            delete_snapshot(&snapshot, rule.min_age_days, exclusions)
        })();
        result.map_err(|e| {
            if e.downcast_ref::<dustaway_core::DeleteFailure>().is_some() {
                e
            } else {
                let code = error_reason(&e);
                Refusal(if code == ReasonCode::Unknown {
                    ReasonCode::Unverifiable
                } else {
                    code
                })
                .into()
            }
        })
    }
    fn free_space(&self) -> Result<u64> {
        free_bytes(&self.system)
    }
}

pub(crate) fn intentional_skip(code: ReasonCode) -> bool {
    matches!(
        code,
        ReasonCode::LinkOrCloud | ReasonCode::Protected | ReasonCode::Excluded
    )
}

pub(crate) fn profile_marker(
    path: &Path,
    cancel: Option<&AtomicBool>,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<bool> {
    check_cancel(cancel)?;
    // Missing markers indicate unsupported layouts; access failures do not.
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            issue(error_reason(&error.into()));
            return Ok(false);
        }
        Ok(_) => {}
    }
    match PinnedPath::open_cancellable(path, false, false, cancel) {
        Ok(_) => Ok(true),
        Err(error) if error.is::<ScanCancelled>() => Err(error),
        Err(error) => {
            let code = error_reason(&error);
            if !intentional_skip(code) {
                issue(code);
            }
            Ok(false)
        }
    }
}

fn retain_observed(emit: &mut dyn FnMut(ScanEvent), path: &Path, reason: ReasonCode) {
    let metadata = std::fs::symlink_metadata(path)
        .ok()
        .filter(|m| m.is_file() && m.file_attributes() & 0x400 == 0);
    if let Some(meta) = &metadata {
        emit(ScanEvent::RetainedFile {
            reason,
            bytes: meta.len(),
        });
    } else {
        emit(ScanEvent::Retained(reason));
    }
    emit(ScanEvent::Observation(dustaway_core::ObservedFile {
        path: display_path(path),
        bytes: metadata.as_ref().map(|m| m.len().to_string()),
        modified_unix: metadata
            .as_ref()
            .map(|m| (m.last_write_time() / 10_000_000) as i64 - 11_644_473_600),
        reason,
    }));
}

fn iis_log_file(path: &Path) -> bool {
    use std::io::Read;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let Some(date) = name
        .strip_prefix("u_ex")
        .and_then(|n| n.strip_suffix(".log"))
    else {
        return false;
    };
    if !matches!(date.len(), 6 | 8) || !date.bytes().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let Ok(_pin) = PinnedPath::open(path, false, false) else {
        return false;
    };
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut header = [0u8; 512];
    file.read(&mut header).is_ok_and(|n| {
        header[..n].starts_with(b"#Software: Microsoft Internet Information Services")
    })
}

fn attribute_refusal(attributes: u32, directory: bool) -> Option<ReasonCode> {
    use windows::Win32::Storage::FileSystem::*;
    if attributes & (FILE_ATTRIBUTE_REPARSE_POINT.0 | FILE_ATTRIBUTE_OFFLINE.0 | 0x40000 | 0x400000)
        != 0
    {
        return Some(ReasonCode::LinkOrCloud);
    }
    if !directory
        && attributes
            & (FILE_ATTRIBUTE_READONLY.0 | FILE_ATTRIBUTE_SYSTEM.0 | FILE_ATTRIBUTE_ENCRYPTED.0)
            != 0
    {
        return Some(ReasonCode::Protected);
    }
    None
}

fn prefilter(attributes: u32, modified: u64, age: u32, cutoff: i64) -> Option<ReasonCode> {
    if let Some(reason) = attribute_refusal(attributes, false) {
        return Some(reason);
    }
    if age > 0 && modified != 0 {
        let unix = i64::try_from(modified / 10_000_000)
            .ok()?
            .checked_sub(11_644_473_600)?;
        if unix > cutoff {
            return Some(ReasonCode::Recent);
        }
    }
    None
}

pub fn volume_target(path: &Path) -> Result<dustaway_core::VolumeTarget> {
    use windows::{Win32::Storage::FileSystem::GetVolumeNameForVolumeMountPointW, core::PCWSTR};
    let path = extended(path)?;
    let drive = path
        .ancestors()
        .last()
        .ok_or(Refusal(ReasonCode::UnsupportedVolume))?;
    is_fixed_ntfs(&path)?;
    let name = wide(drive);
    let mut guid = [0u16; 128];
    unsafe {
        GetVolumeNameForVolumeMountPointW(PCWSTR(name.as_ptr()), &mut guid)?;
    }
    let len = guid
        .iter()
        .position(|c| *c == 0)
        .ok_or(Refusal(ReasonCode::Unverifiable))?;
    let volume = String::from_utf16(&guid[..len])?;
    Ok(dustaway_core::VolumeTarget {
        key: volume.clone(),
        label: display_path(drive),
        opaque: volume,
    })
}
pub fn snapshot(root: &Path, path: &Path, min_age_days: u32) -> Result<FileRecord> {
    snapshot_at(root, path, now() - i64::from(min_age_days) * 86400, None)
}
fn snapshot_at(
    root: &Path,
    path: &Path,
    cutoff: i64,
    cancel: Option<&AtomicBool>,
) -> Result<FileRecord> {
    snapshot_mode(root, path, cutoff, cancel, false)
}
fn snapshot_mode(
    root: &Path,
    path: &Path,
    cutoff: i64,
    cancel: Option<&AtomicBool>,
    observe: bool,
) -> Result<FileRecord> {
    let root = extended(root)?;
    let path = extended(path)?;
    ensure!(
        path.starts_with(&root) && path != root,
        "文件不在允许目录内"
    );
    let pinned = if observe {
        PinnedPath::observe(path.as_path(), false, cancel)?
    } else {
        PinnedPath::open_cancellable(&path, false, false, cancel)?
    };
    let modified = pinned.stamp.modified_unix();
    ensure!(modified <= cutoff, Refusal(ReasonCode::Recent));
    let record = FileRecord {
        path_display: display_path(&path),
        bytes: pinned.stamp.size,
        modified_unix: modified,
        opaque: serde_json::to_string(&Snapshot {
            application_identity: None,
            path: native(&path),
            root: native(&root),
            parents: pinned.parent_ids,
            stamp: pinned.stamp,
        })?,
    };
    Ok(record)
}

pub fn delete_snapshot(
    snapshot: &Snapshot,
    min_age_days: u32,
    exclusions: &[String],
) -> Result<()> {
    let root = extended(&from_native(&snapshot.root))?;
    let path = extended(&from_native(&snapshot.path))?;
    ensure!(
        path.starts_with(&root) && path != root,
        "文件不在允许目录内"
    );
    ensure!(!excluded(&path, exclusions), Refusal(ReasonCode::Excluded));
    is_fixed_ntfs(&path)?;
    let pinned = PinnedPath::open(&path, false, true)?;
    ensure!(
        pinned.parent_ids == snapshot.parents,
        Refusal(ReasonCode::Changed)
    );
    ensure!(pinned.stamp == snapshot.stamp, Refusal(ReasonCode::Changed));
    ensure!(
        pinned.stamp.modified_unix() <= now() - i64::from(min_age_days) * 86400,
        Refusal(ReasonCode::Recent)
    );
    pinned.mark_delete()
}

/// Read-only action; keep the validated file and parent chain pinned through Shell selection.
pub fn candidate_action_snapshot(
    snapshot: &Snapshot,
    kind: dustaway_core::ExclusionKind,
    reveal: bool,
) -> Result<String> {
    use dustaway_core::ExclusionKind;
    let root = extended(&from_native(&snapshot.root))?;
    let path = extended(&from_native(&snapshot.path))?;
    ensure!(
        path.starts_with(&root) && path != root,
        Refusal(ReasonCode::Changed)
    );
    is_fixed_ntfs(&path)?;
    let pinned = PinnedPath::observe(&path, false, None)?;
    ensure!(
        pinned.parent_ids == snapshot.parents && pinned.stamp == snapshot.stamp,
        Refusal(ReasonCode::Changed)
    );
    let target = match kind {
        ExclusionKind::File => path.as_path(),
        ExclusionKind::Parent => path.parent().ok_or(Refusal(ReasonCode::Changed))?,
    };
    ensure!(target.starts_with(&root), Refusal(ReasonCode::Changed));
    if reveal {
        reveal_pinned_path(&path)?;
    }
    Ok(display_path(target))
}

pub(crate) fn reveal_pinned_path(path: &Path) -> Result<()> {
    use windows::{
        Win32::{
            System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            UI::Shell::{Common::ITEMIDLIST, SHOpenFolderAndSelectItems, SHParseDisplayName},
        },
        core::PCWSTR,
    };
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    struct Pidl(*mut ITEMIDLIST);
    impl Drop for Pidl {
        fn drop(&mut self) {
            unsafe {
                CoTaskMemFree(Some(self.0.cast()));
            }
        }
    }
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let _com = Com;
        let mut pidl = Pidl(std::ptr::null_mut());
        // Shell parsing names use DOS paths, unlike the extended filesystem paths used by handles.
        // Refuse non-Unicode names instead of selecting a lossy replacement path.
        let shell_path = path
            .to_str()
            .ok_or(Refusal(ReasonCode::Unverifiable))?
            .trim_start_matches("\\\\?\\");
        let name = text_wide(shell_path);
        SHParseDisplayName(PCWSTR(name.as_ptr()), None, &mut pidl.0, 0, None)?;
        SHOpenFolderAndSelectItems(pidl.0, None, 0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    fn fixture() -> (tempfile::TempDir, WindowsPlatform) {
        let temp = tempfile::tempdir().unwrap();
        let platform = WindowsPlatform {
            inventory: Default::default(),
            tool_roots: Default::default(),
            tool_probe: std::sync::Arc::new(crate::tool_discovery::UnavailableProbe),
            real_app_acceptance: None,
            app_test_identity: None,
            app_test_acceptance: None,
            scope: Scope::User,
            local: extended(&temp.path().join("local")).unwrap(),
            roaming: extended(&temp.path().join("roaming")).unwrap(),
            system: extended(temp.path()).unwrap(),
        };
        (temp, platform)
    }
    #[test]
    fn chromium_requires_standard_profile_markers_and_narrow_cache_roots() {
        let (_temp, platform) = fixture();
        for product in dustaway_core::browsers::CHROMIUM_PRODUCTS {
            let base = platform.local.join(product.user_data);
            for name in [
                "Default",
                "Profile 2",
                "Portable",
                "Profile bad",
                "Guest Profile",
                "System Profile",
            ] {
                write(&base.join(name).join("Preferences"));
                write(&base.join(name).join("History"));
            }
            std::fs::create_dir_all(base.join("Profile 3")).unwrap();
            let rule = dustaway_core::rules::catalog()
                .unwrap()
                .into_iter()
                .find(|r| r.id == format!("{}.http", product.id))
                .unwrap();
            let roots = platform.roots(&rule).unwrap();
            assert_eq!(roots.len(), 2);
            assert!(roots.contains(&base.join("Default/Cache/Cache_Data")));
            assert!(roots.contains(&base.join("Profile 2/Cache/Cache_Data")));
            assert!(base.join("Default/History").exists());
        }
    }

    #[test]
    fn new_browser_rules_preserve_profile_data_and_refuse_changed_candidates() {
        let (_temp, platform) = fixture();
        let rules = dustaway_core::rules::catalog().unwrap();
        for app in ["brave", "vivaldi"] {
            let base = platform
                .local
                .join(dustaway_core::browsers::chromium(app).unwrap().user_data);
            let profile = base.join("Default");
            let protected = [
                "Preferences",
                "Cookies",
                "Network/Cookies",
                "Login Data",
                "History",
                "Sessions/tab",
                "Extensions/settings",
                "Local Storage/data",
                "IndexedDB/data",
                "Service Worker/CacheStorage/data",
                "BraveWallet/data",
                "Rewards/data",
                "Notes",
                "Mail/message",
                "VivaldiThumbnails/image",
            ];
            for path in protected {
                write(&profile.join(path));
            }
            let stamps: Vec<_> = protected
                .iter()
                .map(|p| {
                    std::fs::metadata(profile.join(p))
                        .unwrap()
                        .modified()
                        .unwrap()
                })
                .collect();
            for rule in rules.iter().filter(|r| r.root == app) {
                let cache = profile.join(&rule.relative);
                write(&cache.join("delete.cache"));
                write(&cache.join("retained.cache"));
                write(&cache.join("changed.cache"));
                write(&cache.join("linked.cache"));
                std::fs::hard_link(
                    cache.join("linked.cache"),
                    profile.join(format!("{}.link", rule.id)),
                )
                .unwrap();
                let exclusions = vec![display_path(&cache.join("retained.cache"))];
                let mut candidates = vec![];
                platform
                    .scan(rule, &exclusions, &AtomicBool::new(false), &mut |event| {
                        if let ScanEvent::File(file) = event {
                            candidates.push(file);
                        }
                    })
                    .unwrap();
                assert_eq!(candidates.len(), 2);
                for file in candidates {
                    if file.path_display.ends_with("changed.cache") {
                        std::fs::write(cache.join("changed.cache"), b"changed content").unwrap();
                        assert!(platform.delete(rule, &file, &exclusions).is_err());
                    } else {
                        platform.delete(rule, &file, &exclusions).unwrap();
                    }
                }
                assert!(!cache.join("delete.cache").exists());
                for name in ["retained.cache", "changed.cache", "linked.cache"] {
                    assert!(cache.join(name).exists());
                }
            }
            for (index, path) in protected.iter().enumerate() {
                assert_eq!(std::fs::read(profile.join(path)).unwrap(), b"fixture");
                assert_eq!(
                    std::fs::metadata(profile.join(path))
                        .unwrap()
                        .modified()
                        .unwrap(),
                    stamps[index]
                );
            }
        }
    }
    #[test]
    fn firefox_requires_relative_standard_layout_and_profile_marker() {
        let (_temp, platform) = fixture();
        let base = platform.roaming.join("Mozilla/Firefox");
        write(&base.join("Profiles/standard/prefs.js"));
        write(&base.join("Profiles/second/prefs.js"));
        write(&base.join("profiles.ini"));
        std::fs::write(base.join("profiles.ini"), "[Profile0]\nIsRelative=1\nPath=Profiles/standard\n[Profile1]\nIsRelative=1\nPath=Profiles/second\n[Profile2]\nIsRelative=0\nPath=C:/private\n[Profile3]\nIsRelative=1\nPath=Profiles/../outside\n[Profile4]\nIsRelative=1\nPath=Profiles/missing\n").unwrap();
        let rule = dustaway_core::rules::catalog()
            .unwrap()
            .into_iter()
            .find(|r| r.root == "firefox")
            .unwrap();
        let roots = platform.roots(&rule).unwrap();
        assert_eq!(roots.len(), 2);
        assert!(
            roots.contains(
                &platform
                    .local
                    .join("Mozilla/Firefox/Profiles/standard/cache2")
            )
        );
        assert!(
            roots.contains(
                &platform
                    .local
                    .join("Mozilla/Firefox/Profiles/second/cache2")
            )
        );
    }
}
