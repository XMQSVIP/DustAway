//! Explicit debug-only backend for destructive WebDriver tests. Never in release builds.
use anyhow::{Result, ensure};
use dustaway_core::*;
use dustaway_windows::{
    fs::*,
    platform::{delete_snapshot, snapshot},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
struct Fixture {
    root: PathBuf,
}

/// The E2E build never delegates software operations to the host's installation registry.
#[derive(Default)]
pub struct FixtureSoftware {
    removed: AtomicBool,
    deleted_residuals: Mutex<std::collections::HashSet<String>>,
}
impl FixtureSoftware {
    /// Purely in-memory entries. Their display paths are never passed to a filesystem API.
    fn residual_records() -> Vec<software::ResidualRecord> {
        if std::env::var("DUSTAWAY_E2E_SOFTWARE_RESIDUALS").as_deref() != Ok("1") {
            return vec![];
        }
        [
            (
                "cache-dir",
                r"C:\DustAway-Synthetic\Cache",
                "cache",
                "0",
                true,
                true,
            ),
            (
                "cache-file",
                r"C:\DustAway-Synthetic\Cache\entry.bin",
                "cache",
                "1024",
                false,
                true,
            ),
            (
                "data-file",
                r"C:\DustAway-Synthetic\Preferences",
                "data",
                "256",
                false,
                true,
            ),
            (
                "shared",
                r"C:\DustAway-Synthetic\Shared\keep.txt",
                "data",
                "512",
                false,
                false,
            ),
        ]
        .into_iter()
        .map(
            |(id, path, category, bytes, directory, selectable)| software::ResidualRecord {
                candidate: software::ResidualCandidate {
                    id: id.into(),
                    path: path.into(),
                    category: category.into(),
                    bytes: bytes.into(),
                    directory,
                    selectable,
                    reason: (!selectable).then(|| "模拟共享资料，始终保留".into()),
                },
                opaque: format!("synthetic-residual:{id}"),
            },
        )
        .collect()
    }
}
impl software::SoftwarePlatform for FixtureSoftware {
    fn inventory(&self) -> Result<software::SoftwareInventory> {
        let entry = software::SoftwareEntry {
            software: software::InstalledSoftware {
                id: "fixture-software".into(),
                name: "隔离测试软件".into(),
                publisher: "DustAway Fixture".into(),
                version: "1.0".into(),
                source: software::SoftwareSource::Desktop,
                scope: "current_user".into(),
                estimated_bytes: Some("10240".into()),
                installed_on: None,
                install_location: None,
                icon_data_url: None,
                can_uninstall: true,
                unavailable_reason: None,
                residual_supported: true,
                residual_note: "仅测试状态流转，不调用本机卸载程序".into(),
                residual_capabilities: [
                    software::residual_rules::ResidualKind::Cache,
                    software::residual_rules::ResidualKind::Settings,
                    software::residual_rules::ResidualKind::LocalData,
                ]
                .into_iter()
                .map(|kind| software::residual_rules::ResidualCapability {
                    kind,
                    inspection: software::residual_rules::ResidualInspectionSupport::FixedLocations,
                    cleanup: software::residual_rules::ResidualCleanupSupport::RequiresVerification,
                })
                .collect(),
                missing_information: vec![],
            }
            .with_missing_information(),
            identity: software::SoftwareIdentity {
                source: software::SoftwareSource::Desktop,
                key: "fixture-software".into(),
                family: "fixture".into(),
            },
        };
        Ok(software::SoftwareInventory {
            entries: if self.removed.load(Ordering::Acquire) {
                vec![]
            } else {
                vec![entry]
            },
            warnings: vec![],
        })
    }
    fn prepare(&self, entry: &software::SoftwareEntry) -> Result<software::PreparedSoftware> {
        Ok(software::PreparedSoftware {
            entry: entry.clone(),
            fingerprint: "fixture".into(),
            context: String::new(),
        })
    }
    fn launch(
        &self,
        _: &software::PreparedSoftware,
        _: &mut dyn FnMut(u32),
    ) -> Result<software::LaunchOutcome> {
        // Only the debug fixture can delay a synthetic uninstall for IPC recovery tests.
        let delay = std::env::var("DUSTAWAY_E2E_SOFTWARE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        ensure!(delay <= 180000, "fixture delay exceeds test limit");
        std::thread::sleep(std::time::Duration::from_millis(delay));
        self.removed.store(true, Ordering::Release);
        Ok(software::LaunchOutcome::Completed)
    }
    fn installed(&self, _: &software::SoftwareIdentity) -> Result<bool> {
        Ok(!self.removed.load(Ordering::Acquire))
    }
    fn scan(
        &self,
        _: &software::PreparedSoftware,
        _: &[String],
        _: &AtomicBool,
    ) -> Result<software::ResidualScanData> {
        Ok(software::ResidualScanData {
            records: Self::residual_records()
                .into_iter()
                .filter(|r| {
                    !self
                        .deleted_residuals
                        .lock()
                        .unwrap()
                        .contains(&r.candidate.id)
                })
                .collect(),
            warnings: vec![],
            supported: true,
        })
    }
    fn delete(
        &self,
        _: &software::PreparedSoftware,
        record: &software::ResidualRecord,
        _: &[String],
    ) -> Result<()> {
        ensure!(
            self.removed.load(Ordering::Acquire),
            "fixture is still installed"
        );
        let candidate = Self::residual_records()
            .into_iter()
            .find(|r| {
                r.candidate.id == record.candidate.id
                    && r.candidate.selectable
                    && r.opaque == record.opaque
            })
            .ok_or_else(|| anyhow::anyhow!("unknown or readonly fixture residue"))?;
        let mut deleted = self.deleted_residuals.lock().unwrap();
        ensure!(
            candidate.candidate.id != "cache-dir" || deleted.contains("cache-file"),
            "fixture directory is not empty"
        );
        ensure!(
            deleted.insert(candidate.candidate.id),
            "fixture residue already removed"
        );
        Ok(())
    }
}
impl Fixture {
    fn rule_root(&self, rule: &RuleDescriptor) -> Result<PathBuf> {
        if rule.id == "windows.user_temp" {
            return Ok(self.root.clone());
        }
        ensure!(
            applications::rule(&rule.id).is_some(),
            "invalid fixture rule"
        );
        Ok(self
            .root
            .parent()
            .unwrap()
            .join("applications")
            .join(&rule.id))
    }
}
pub fn provider() -> Result<(Arc<dyn Platform>, PathBuf)> {
    let root = PathBuf::from(std::env::var("DUSTAWAY_E2E_ROOT")?);
    ensure!(
        root.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("dustaway-e2e-")),
        "invalid fixture root"
    );
    ensure!(
        root.parent() == Some(std::env::temp_dir().as_path()),
        "fixture must be a direct temporary directory child"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-test-only"))? == "fixture-only",
        "fixture marker absent"
    );
    let journal = root.join("history.sqlite");
    Ok((
        Arc::new(Fixture {
            root: root.join("cache"),
        }),
        journal,
    ))
}
impl Platform for Fixture {
    fn file_volume(&self, _: &FileRecord) -> Result<VolumeTarget> {
        dustaway_windows::platform::volume_target(&self.root)
    }
    fn volume_available(&self, volume: &VolumeTarget) -> Result<u64> {
        free_bytes(std::path::Path::new(&volume.opaque))
    }
    fn candidate_root(&self, _: &FileRecord) -> Option<String> {
        Some(display_path(&self.root))
    }
    fn candidate_action(
        &self,
        rule: &RuleDescriptor,
        file: &FileRecord,
        kind: ExclusionKind,
        reveal: bool,
    ) -> Result<String> {
        let root = self.rule_root(rule)?;
        let snap: Snapshot = serde_json::from_str(&file.opaque)?;
        ensure!(
            extended(&from_native(&snap.root))? == extended(&root)?,
            "fixture scope mismatch"
        );
        dustaway_windows::platform::candidate_action_snapshot(&snap, kind, reveal)
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
        volumes: Option<&[VolumeTarget]>,
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        if rule.id != "windows.user_temp" && applications::rule(&rule.id).is_none() {
            return Ok(());
        }
        let root = self.rule_root(rule)?;
        if let Some(volumes) = volumes {
            ensure!(!volumes.is_empty(), "请选择至少一个磁盘");
            let target = dustaway_windows::platform::volume_target(&root)?;
            if !volumes.iter().any(|v| v.key == target.key) {
                return Ok(());
            }
        }
        if !root.exists() {
            return Ok(());
        }
        if self
            .root
            .parent()
            .unwrap()
            .join(".simulate-unreadable")
            .exists()
        {
            emit(ScanEvent::CoverageIssue(ReasonCode::AccessDenied));
        }
        for entry in std::fs::read_dir(&root)? {
            if cancel.load(Ordering::Acquire) {
                break;
            }
            let path = entry?.path();
            emit(ScanEvent::Checked);
            if excluded(&path, exclusions) {
                emit(ScanEvent::Retained(ReasonCode::Excluded));
                continue;
            }
            if !applications::accepts_file(&rule.id, &path) {
                emit(ScanEvent::Retained(ReasonCode::Protected));
                continue;
            }
            match snapshot(&root, &path, rule.min_age_days) {
                Ok(file) => emit(ScanEvent::File(file)),
                Err(e) => {
                    let reason = error_reason(&e);
                    let metadata = std::fs::symlink_metadata(&path)?;
                    emit(ScanEvent::RetainedFile {
                        reason,
                        bytes: metadata.len(),
                    });
                    emit(ScanEvent::Observation(ObservedFile {
                        path: display_path(&path),
                        bytes: Some(metadata.len().to_string()),
                        modified_unix: metadata
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|t| t.as_secs() as i64),
                        reason,
                    }));
                }
            }
        }
        Ok(())
    }
    fn blocked(&self, rule: &RuleDescriptor) -> Result<Option<dustaway_core::BlockReason>> {
        if applications::rule(&rule.id).is_some() {
            let code = if self
                .root
                .parent()
                .unwrap()
                .join(".application-running")
                .exists()
            {
                ReasonCode::ApplicationRunning
            } else {
                ReasonCode::ValidationPending
            };
            return Ok(Some(BlockReason::new(code, code.message())));
        }
        Ok(None)
    }
    fn delete(
        &self,
        rule: &RuleDescriptor,
        file: &FileRecord,
        exclusions: &[String],
    ) -> Result<()> {
        ensure!(rule.id == "windows.user_temp", "invalid fixture rule");
        let snap: Snapshot = serde_json::from_str(&file.opaque)?;
        ensure!(
            extended(&from_native(&snap.root))? == extended(&self.root)?,
            "fixture scope mismatch"
        );
        delete_snapshot(&snap, rule.min_age_days, exclusions)
    }
    fn free_space(&self) -> Result<u64> {
        free_bytes(&self.root)
    }
}
