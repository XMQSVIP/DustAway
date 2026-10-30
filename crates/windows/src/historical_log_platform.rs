//! Prepared adapter uses the production Engine and file-handle deletion boundary.
use super::*;
use crate::historical_log_probe::{Probe, WindowsProbe, observed};
use crate::historical_logs::{AGE_DAYS, LogIdentity, Policy};
use dustaway_core::BlockReason;

impl WindowsPlatform {
    pub(super) fn prepared_log_policy(&self, rule: &RuleDescriptor) -> Result<Option<Policy>> {
        let Some(policy) = Policy::for_rule(&rule.id) else {
            return Ok(None);
        };
        dustaway_core::rules::validate(rule)?;
        #[cfg(test)]
        if let Some(permit) = &self.log_test_acceptance {
            permit.require(self, rule)?;
            return Ok(Some(policy));
        }
        // Deliberately no release/feature/environment switch: real acceptance is outstanding.
        let _ = policy;
        Ok(None)
    }

    fn log_probe(&self) -> &dyn Probe {
        #[cfg(test)]
        if let Some(permit) = &self.log_test_acceptance {
            return &permit.probe;
        }
        &WindowsProbe
    }
    fn log_identity(
        &self,
        rule: &RuleDescriptor,
        policy: Policy,
        cancel: Option<&AtomicBool>,
    ) -> Result<LogIdentity> {
        ensure!(
            observed(cancel, || self.prepared_log_policy(rule))?.is_some(),
            Refusal(ReasonCode::ValidationPending)
        );
        policy.identify(self.log_probe(), cancel)
    }

    pub(super) fn logs_blocked(
        &self,
        rule: &RuleDescriptor,
        policy: Policy,
    ) -> Result<Option<BlockReason>> {
        let result = (|| -> Result<()> {
            let identity = self.log_identity(rule, policy, None)?;
            identity.require_stopped(self.log_probe(), None)
        })();
        Ok(result.err().map(|e| {
            let code = error_reason(&e);
            BlockReason::new(code, code.message())
        }))
    }

    pub(super) fn validate_log(
        &self,
        rule: &RuleDescriptor,
        policy: Policy,
        file: &FileRecord,
    ) -> Result<LogIdentity> {
        let identity = self.log_identity(rule, policy, None)?;
        let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
        ensure!(
            snapshot.application_identity == Some(identity.fingerprint()?),
            Refusal(ReasonCode::Changed)
        );
        let root = from_native(&snapshot.root);
        let path = from_native(&snapshot.path);
        ensure!(
            policy.roots(&self.local, &self.roaming)?.contains(&root),
            Refusal(ReasonCode::Changed)
        );
        ensure!(policy.accepts(&root, &path), Refusal(ReasonCode::Protected));
        candidate_action_snapshot(&snapshot, dustaway_core::ExclusionKind::File, false)?;
        policy.require_age(snapshot.stamp.modified_unix(), now())?;
        Ok(identity)
    }

    pub(super) fn delete_log(
        &self,
        rule: &RuleDescriptor,
        policy: Policy,
        file: &FileRecord,
        exclusions: &[String],
    ) -> Result<()> {
        // Keep the verified executable pinned until the final file-handle operation finishes.
        let identity = self.validate_log(rule, policy, file)?;
        identity.require_stopped(self.log_probe(), None)?;
        let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
        delete_snapshot(&snapshot, AGE_DAYS, exclusions)
    }

    pub(super) fn scan_logs(
        &self,
        rule: &RuleDescriptor,
        policy: Policy,
        exclusions: &[String],
        volumes: Option<&[dustaway_core::VolumeTarget]>,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        let mut had_error = false;
        let result = (|| -> Result<()> {
            check_cancel(Some(cancel))?;
            let identity = self.log_identity(rule, policy, Some(cancel))?;
            let probe = self.log_probe();
            let fingerprint = identity.fingerprint()?;
            let exclusions = CompiledExclusions::new(exclusions);
            let at = now();
            let cutoff = at - i64::from(AGE_DAYS) * 86400;
            let mut seen = HashSet::new();
            let mut missing = 0;
            let mut selected = 0;
            let mut usable = false;
            for root in policy.roots(&self.local, &self.roaming)? {
                check_cancel(Some(cancel))?;
                if let Some(volumes) = volumes {
                    let target = match observed(Some(cancel), || {
                        volume_target_cancellable(&root, Some(cancel))
                    }) {
                        Ok(target) => target,
                        Err(e) if e.is::<ScanCancelled>() => return Err(e),
                        Err(e) => {
                            log_coverage_issue(emit, &mut had_error, &e, None);
                            continue;
                        }
                    };
                    if !volumes.iter().any(|v| v.key == target.key) {
                        continue;
                    }
                }
                selected += 1;
                if exclusions.contains(&root) {
                    emit(ScanEvent::Retained(ReasonCode::Excluded));
                    continue;
                }
                match observed(Some(cancel), || Ok(root.try_exists()?)) {
                    Ok(false) => {
                        missing += 1;
                        continue;
                    }
                    Ok(true) => {}
                    Err(e) if e.is::<ScanCancelled>() => return Err(e),
                    Err(e) => {
                        log_coverage_issue(emit, &mut had_error, &e, None);
                        continue;
                    }
                }
                let root_result = observed(Some(cancel), || {
                    is_fixed_ntfs_cancellable(&root, Some(cancel))?;
                    PinnedPath::open_cancellable(&root, true, false, Some(cancel))
                });
                let _root_pin = match root_result {
                    Ok(pin) => pin,
                    Err(e) if e.is::<ScanCancelled>() => return Err(e),
                    Err(e) if log_retention(error_reason(&e)) => {
                        emit(ScanEvent::Retained(error_reason(&e)));
                        continue;
                    }
                    Err(e) => {
                        log_coverage_issue(emit, &mut had_error, &e, None);
                        continue;
                    }
                };
                let mut entries = match observed(Some(cancel), || probe.read_dir(&root)) {
                    Ok(entries) => entries,
                    Err(e) if e.is::<ScanCancelled>() => return Err(e),
                    Err(e) => {
                        log_coverage_issue(emit, &mut had_error, &e, None);
                        continue;
                    }
                };
                let mut root_error = false;
                loop {
                    check_cancel(Some(cancel))?;
                    let entry = entries.next();
                    check_cancel(Some(cancel))?;
                    let Some(entry) = entry else {
                        if !root_error {
                            usable = true;
                        }
                        break;
                    };
                    let path = match entry {
                        Ok(path) => path,
                        Err(e) if e.is::<ScanCancelled>() => return Err(e),
                        Err(e) => {
                            root_error = true;
                            log_coverage_issue(emit, &mut had_error, &e, None);
                            continue;
                        }
                    };
                    emit(ScanEvent::Checked);
                    check_cancel(Some(cancel))?;
                    // A probe cannot widen a fixed root or supply a sibling path.
                    ensure!(
                        path.parent() == Some(root.as_path()),
                        Refusal(ReasonCode::Protected)
                    );
                    if exclusions.contains(&path) {
                        retain_observed(emit, &path, ReasonCode::Excluded);
                        usable = true;
                        continue;
                    }
                    let metadata = match observed(Some(cancel), || probe.metadata(&path)) {
                        Ok(meta) => meta,
                        Err(e) if e.is::<ScanCancelled>() => return Err(e),
                        Err(e) => {
                            root_error = true;
                            log_coverage_issue(emit, &mut had_error, &e, Some(&path));
                            continue;
                        }
                    };
                    let kind = metadata.file_type();
                    if kind.is_symlink() || metadata.file_attributes() & 0x400 != 0 {
                        retain_observed(emit, &path, ReasonCode::LinkOrCloud);
                        usable = true;
                        continue;
                    }
                    if kind.is_dir() {
                        emit(ScanEvent::Retained(ReasonCode::Protected));
                        usable = true;
                        continue;
                    }
                    if !policy.accepts(&root, &path) {
                        retain_observed(emit, &path, ReasonCode::Protected);
                        usable = true;
                        continue;
                    }
                    match observed(Some(cancel), || {
                        snapshot_at(&root, &path, cutoff, Some(cancel))
                    }) {
                        Ok(mut record) => {
                            policy.require_age(record.modified_unix, at)?;
                            let mut snapshot: Snapshot = serde_json::from_str(&record.opaque)?;
                            if !seen.insert((
                                snapshot.stamp.identity.volume,
                                snapshot.stamp.identity.index,
                            )) {
                                continue;
                            }
                            snapshot.application_identity = Some(fingerprint.clone());
                            record.opaque = serde_json::to_string(&snapshot)?;
                            check_cancel(Some(cancel))?;
                            usable = true;
                            emit(ScanEvent::File(record));
                        }
                        Err(e) if e.is::<ScanCancelled>() => return Err(e),
                        Err(e) if log_retention(error_reason(&e)) => {
                            usable = true;
                            retain_observed(emit, &path, error_reason(&e));
                        }
                        Err(e) => {
                            root_error = true;
                            log_coverage_issue(emit, &mut had_error, &e, Some(&path));
                        }
                    }
                }
            }
            check_cancel(Some(cancel))?;
            ensure!(
                !had_error || usable,
                Refusal(ReasonCode::DirectoryInaccessible)
            );
            if selected == 0 {
                emit(ScanEvent::Notice("所选磁盘中没有此类日志位置".into()));
            } else if missing == selected {
                emit(ScanEvent::NoticeCode(ReasonCode::LocationMissing));
            }
            Ok(())
        })();
        match result {
            Err(e) if e.is::<ScanCancelled>() => Ok(()),
            Err(e) => {
                if !had_error {
                    emit(ScanEvent::CoverageIssue(error_reason(&e)));
                }
                Err(e)
            }
            Ok(()) => Ok(()),
        }
    }
}

fn log_retention(reason: ReasonCode) -> bool {
    matches!(
        reason,
        ReasonCode::Recent
            | ReasonCode::InUse
            | ReasonCode::LinkOrCloud
            | ReasonCode::Protected
            | ReasonCode::Excluded
    )
}
fn log_coverage_issue(
    emit: &mut dyn FnMut(ScanEvent),
    failed: &mut bool,
    error: &anyhow::Error,
    path: Option<&Path>,
) {
    *failed = true;
    let reason = match error_reason(error) {
        ReasonCode::Unknown => ReasonCode::ScanFailed,
        known => known,
    };
    emit(ScanEvent::CoverageIssue(reason));
    if let Some(path) = path {
        emit(ScanEvent::Observation(dustaway_core::ObservedFile {
            path: display_path(path),
            bytes: None,
            modified_unix: None,
            reason,
        }));
    }
}
