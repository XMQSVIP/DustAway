use anyhow::{Result, bail, ensure};
use dustaway_core::software::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

struct Fake {
    installed: AtomicBool,
    unknown: AtomicBool,
    restarted: AtomicBool,
    boot_unknown: AtomicBool,
    calls: AtomicUsize,
    deletes: AtomicUsize,
    scans: AtomicUsize,
    fresh_ids: AtomicBool,
    readonly: AtomicBool,
    scan_failure: AtomicBool,
    prepare_failure: AtomicBool,
    inventory_failure: AtomicBool,
    inventory_calls: AtomicUsize,
    hold_first_inventory: AtomicBool,
    inventory_fresh_ids: AtomicBool,
    hold_delete: AtomicBool,
    deleting: AtomicBool,
    hold_launch: AtomicBool,
    hold_scan: AtomicBool,
    panic_scan: AtomicBool,
    panic_delete: AtomicBool,
    record_count: AtomicUsize,
    mixed_separators: AtomicBool,
    store_local_cache: AtomicBool,
    mode: u8,
}
impl Fake {
    fn new(mode: u8) -> Arc<Self> {
        Arc::new(Self {
            installed: AtomicBool::new(true),
            unknown: AtomicBool::new(false),
            restarted: AtomicBool::new(false),
            boot_unknown: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
            deletes: AtomicUsize::new(0),
            scans: AtomicUsize::new(0),
            fresh_ids: AtomicBool::new(false),
            readonly: AtomicBool::new(false),
            scan_failure: AtomicBool::new(false),
            prepare_failure: AtomicBool::new(false),
            inventory_failure: AtomicBool::new(false),
            inventory_calls: AtomicUsize::new(0),
            hold_first_inventory: AtomicBool::new(false),
            inventory_fresh_ids: AtomicBool::new(false),
            hold_delete: AtomicBool::new(false),
            deleting: AtomicBool::new(false),
            hold_launch: AtomicBool::new(false),
            hold_scan: AtomicBool::new(false),
            panic_scan: AtomicBool::new(false),
            panic_delete: AtomicBool::new(false),
            record_count: AtomicUsize::new(1),
            mixed_separators: AtomicBool::new(false),
            store_local_cache: AtomicBool::new(false),
            mode,
        })
    }
}
fn entry() -> SoftwareEntry {
    SoftwareEntry {
        software: InstalledSoftware {
            id: "app-1".into(),
            name: "隔离测试软件".into(),
            publisher: "Fixture".into(),
            version: "1.0".into(),
            source: SoftwareSource::Desktop,
            scope: "current_user".into(),
            estimated_bytes: Some("9999".into()),
            installed_on: None,
            install_location: Some("C:\\private-test-location".into()),
            icon_data_url: None,
            can_uninstall: true,
            unavailable_reason: None,
            residual_supported: true,
            residual_note: "Fixture".into(),
            residual_capabilities: vec![],
            missing_information: vec![],
        }
        .with_missing_information(),
        identity: SoftwareIdentity {
            source: SoftwareSource::Desktop,
            key: "fixture-id".into(),
            family: "fixture".into(),
        },
    }
}
#[test]
fn missing_inventory_information_does_not_change_uninstall_authority_or_old_record_compatibility() {
    let original = entry().software;
    let mut incomplete = original.clone();
    incomplete.publisher = "  ".into();
    incomplete.version.clear();
    incomplete.estimated_bytes = Some("0".into());
    incomplete.installed_on = Some("".into());
    incomplete.install_location = Some(" \t ".into());
    incomplete = incomplete.with_missing_information();
    assert_eq!(
        incomplete.missing_information,
        vec![
            SoftwareMissingField::Publisher,
            SoftwareMissingField::Version,
            SoftwareMissingField::InstalledOn,
            SoftwareMissingField::InstallLocation
        ]
    );
    assert_eq!(incomplete.can_uninstall, original.can_uninstall);
    assert_eq!(incomplete.unavailable_reason, original.unavailable_reason);
    incomplete.estimated_bytes = None;
    assert!(
        incomplete
            .with_missing_information()
            .missing_information
            .contains(&SoftwareMissingField::EstimatedBytes)
    );
    let mut old = serde_json::to_value(&original).unwrap();
    old.as_object_mut().unwrap().remove("residual_capabilities");
    old.as_object_mut().unwrap().remove("missing_information");
    let restored: InstalledSoftware = serde_json::from_value(old).unwrap();
    assert!(restored.residual_capabilities.is_empty());
    assert!(restored.missing_information.is_empty());
    assert!(restored.icon_data_url.is_none());
    assert_eq!(restored.residual_note, original.residual_note);
}
impl SoftwarePlatform for Fake {
    fn restarted_since(&self, _: i64) -> Result<bool> {
        ensure!(
            !self.boot_unknown.load(Ordering::SeqCst),
            "boot state unavailable"
        );
        Ok(self.restarted.load(Ordering::SeqCst))
    }
    fn inventory(&self) -> Result<SoftwareInventory> {
        let failed = self.inventory_failure.load(Ordering::SeqCst);
        let call = self.inventory_calls.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(3);
        while call == 0 && self.hold_first_inventory.load(Ordering::SeqCst) {
            ensure!(Instant::now() < deadline, "fixture inventory timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
        ensure!(!failed, "fixture inventory failure");
        let mut item = entry();
        if self.store_local_cache.load(Ordering::SeqCst) {
            item.software.source = SoftwareSource::Store;
            item.software.scope = "current_user".into();
            item.identity = SoftwareIdentity {
                source: SoftwareSource::Store,
                key: "DedicatedFixture_1.0.0.0_x64__publisher".into(),
                family: "DedicatedFixture_publisher".into(),
            };
        }
        if self.inventory_fresh_ids.load(Ordering::SeqCst) {
            item.software.id = format!("inventory-{call}");
            item.identity.key = format!("identity-{call}");
        }
        Ok(SoftwareInventory {
            entries: vec![item],
            warnings: vec![],
        })
    }
    fn prepare(&self, e: &SoftwareEntry) -> Result<PreparedSoftware> {
        ensure!(
            !self.prepare_failure.load(Ordering::SeqCst),
            "fixture preparation failure"
        );
        Ok(PreparedSoftware {
            entry: e.clone(),
            fingerprint: "frozen".into(),
            context: "private context, never journaled".into(),
        })
    }
    fn launch(&self, _: &PreparedSoftware, progress: &mut dyn FnMut(u32)) -> Result<LaunchOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        while self.hold_launch.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        progress(50);
        match self.mode {
            1 => Ok(LaunchOutcome::Cancelled),
            2 => Ok(LaunchOutcome::RestartRequired),
            3 => Ok(LaunchOutcome::Untracked),
            4 => bail!("fixture failure"),
            5 => Ok(LaunchOutcome::Completed),
            6 => panic!("fixture launch panic"),
            _ => {
                self.installed.store(false, Ordering::SeqCst);
                Ok(LaunchOutcome::Completed)
            }
        }
    }
    fn installed(&self, _: &SoftwareIdentity) -> Result<bool> {
        ensure!(!self.unknown.load(Ordering::SeqCst), "source unavailable");
        Ok(self.installed.load(Ordering::SeqCst))
    }
    fn scan(
        &self,
        _: &PreparedSoftware,
        _: &[String],
        cancel: &AtomicBool,
    ) -> Result<ResidualScanData> {
        ensure!(
            !self.scan_failure.load(Ordering::SeqCst),
            "fixture scan failure"
        );
        let generation = self.scans.fetch_add(1, Ordering::SeqCst);
        while self.hold_scan.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !self.panic_scan.load(Ordering::SeqCst),
            "fixture scan panic"
        );
        let mut data = ResidualScanData {
            records: vec![
                ResidualRecord {
                    candidate: ResidualCandidate {
                        id: if self.fresh_ids.load(Ordering::SeqCst) {
                            format!("allowed-{generation}")
                        } else {
                            "allowed".into()
                        },
                        path: if self.mixed_separators.load(Ordering::SeqCst) {
                            "C:/fixture\\settings.json".into()
                        } else {
                            "C:\\fixture\\settings.json".into()
                        },
                        category: "data".into(),
                        bytes: "12".into(),
                        directory: false,
                        selectable: !self.readonly.load(Ordering::SeqCst),
                        reason: None,
                    },
                    opaque: "private snapshot".into(),
                },
                ResidualRecord {
                    candidate: ResidualCandidate {
                        id: "protected".into(),
                        path: "C:\\fixture\\shared".into(),
                        category: "data".into(),
                        bytes: "100".into(),
                        directory: false,
                        selectable: false,
                        reason: Some("shared".into()),
                    },
                    opaque: "".into(),
                },
            ],
            warnings: if cancel.load(Ordering::SeqCst) {
                vec!["stopped".into()]
            } else {
                vec![]
            },
            supported: true,
        };
        if self.store_local_cache.load(Ordering::SeqCst) {
            // Exercise the engine's data confirmation with the real catalog classification.
            // This fake candidate never authorizes any production package or file.
            let rule = residual_rules::rule("uninstall.store.local_cache").unwrap();
            data.records[0].candidate.path =
                r"C:\fixture\Packages\DedicatedFixture_publisher\LocalCache\document.db".into();
            data.records[0].candidate.category = rule.kind.category().into();
        }
        for index in 1..self.record_count.load(Ordering::SeqCst) {
            let mut record = data.records[0].clone();
            record.candidate.id = format!("extra-{index}");
            record.candidate.path = format!("C:\\fixture\\settings-{index}.json");
            data.records.push(record);
        }
        Ok(data)
    }
    fn delete(
        &self,
        _: &PreparedSoftware,
        _: &ResidualRecord,
        exclusions: &[String],
    ) -> Result<()> {
        self.deleting.store(true, Ordering::SeqCst);
        while self.hold_delete.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !self.panic_delete.load(Ordering::SeqCst),
            "fixture delete panic"
        );
        ensure!(
            !self.installed.load(Ordering::SeqCst),
            dustaway_core::Refusal(dustaway_core::ReasonCode::Protected)
        );
        ensure!(
            exclusions.is_empty(),
            dustaway_core::Refusal(dustaway_core::ReasonCode::Excluded)
        );
        self.deletes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn wait(engine: &SoftwareEngine) -> UninstallTask {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let task = engine.snapshot().unwrap();
        if !task.phase.active() {
            return task;
        }
        assert!(Instant::now() < deadline, "task timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn start(engine: &Arc<SoftwareEngine>) -> String {
    let id = engine.list().unwrap().items[0].id.clone();
    let plan = engine.prepare(&id).unwrap();
    engine.start(&plan.id).unwrap()
}

fn wait_until(condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "fixture did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn stopped_uninstall_ignores_late_progress_and_completion() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.hold_launch.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
    let id = start(&engine);
    wait_until(|| fake.calls.load(Ordering::SeqCst) == 1);
    engine.stop_tracking();
    fake.hold_launch.store(false, Ordering::SeqCst);
    wait_until(|| Arc::strong_count(&engine) == 1);
    let task = engine.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert!(task.progress.is_none());
    assert!(!task.can_scan_residuals);
    assert_eq!(engine.history().unwrap()[0].phase, UninstallPhase::Unknown);
    assert!(engine.list().is_err());
    assert!(engine.verify(&id).is_err());
    assert!(engine.scan(&id, vec![]).is_err());
    assert!(engine.clear_history().is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn stopped_scan_cannot_publish_a_late_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
    let id = start(&engine);
    wait(&engine);
    fake.hold_scan.store(true, Ordering::SeqCst);
    engine.scan(&id, vec![]).unwrap();
    wait_until(|| fake.scans.load(Ordering::SeqCst) == 1);
    engine.stop_tracking();
    fake.hold_scan.store(false, Ordering::SeqCst);
    wait_until(|| Arc::strong_count(&engine) == 1);
    let task = engine.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert!(!task.can_scan_residuals);
    assert!(task.residual_scan_id.is_none());
    assert!(engine.residuals(&id, 0, "").is_err());
    assert_eq!(engine.history().unwrap()[0].phase, UninstallPhase::Unknown);
}

#[test]
fn native_worker_panics_preserve_uncertainty_and_revoke_residual_authority() {
    for stage in ["launch", "scan", "delete"] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("h.sqlite");
        let fake = Fake::new(if stage == "launch" { 6 } else { 0 });
        let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
        let id = start(&engine);
        if stage != "launch" {
            wait(&engine);
            fake.panic_scan.store(stage == "scan", Ordering::SeqCst);
            engine.scan(&id, vec![]).unwrap();
            if stage == "delete" {
                wait(&engine);
                let plan = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
                fake.panic_delete.store(true, Ordering::SeqCst);
                engine.clean(&plan.id, vec![]).unwrap();
            }
        }
        wait_until(|| Arc::strong_count(&engine) == 1);
        let task = engine.snapshot().unwrap();
        assert_eq!(task.phase, UninstallPhase::Unknown, "{stage}");
        assert!(!engine.is_busy());
        assert!(!task.can_scan_residuals);
        assert!(task.residual_scan_id.is_none());
        assert_eq!(engine.history().unwrap()[0].phase, UninstallPhase::Unknown);
        assert_eq!(task.residual.unknown, u64::from(stage == "delete"));
        fake.panic_scan.store(false, Ordering::SeqCst);
        engine.verify(&id).unwrap();
        wait(&engine);
        assert!(!engine.snapshot().unwrap().can_scan_residuals);
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn parses_only_unambiguous_absolute_executables() {
    let (file, args) =
        executable_command(r#""C:\Program Files\Vendor\uninstall.exe" /remove "x y""#).unwrap();
    assert_eq!(file, r"C:\Program Files\Vendor\uninstall.exe");
    assert_eq!(args, r#"/remove "x y""#);
    for command in [
        r"uninstall.exe /u",
        r"C:\Program Files\a.exe /u",
        r"\\server\share\u.exe",
        r"C:\Windows\cmd.exe /c erase",
        r"C:\Windows\powershell.exe -c x",
        r"C:\Windows\rundll32.exe abc",
        r"C:\a\..\u.exe",
        r"C:\a.exe:stream.exe",
        "C:\\a.exe\n/c",
        r#""C:\unclosed.exe"#,
    ] {
        assert!(executable_command(command).is_err(), "{command}");
    }
}
#[test]
fn plan_is_single_use_and_refresh_invalidates_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let e = SoftwareEngine::open(fake.clone(), &temp.path().join("history.sqlite")).unwrap();
    e.list().unwrap();
    let old = e.prepare("app-1").unwrap();
    e.list().unwrap();
    assert!(e.start(&old.id).is_err());
    let p = e.prepare("app-1").unwrap();
    e.start(&p.id).unwrap();
    assert!(e.start(&p.id).is_err());
    assert_eq!(wait(&e).phase, UninstallPhase::Removed);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn returned_success_never_substitutes_for_installation_verification() {
    for (mode, expected) in [
        (1, UninstallPhase::Cancelled),
        (2, UninstallPhase::RestartRequired),
        (3, UninstallPhase::Unknown),
        (4, UninstallPhase::Failed),
        (5, UninstallPhase::StillInstalled),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let e = SoftwareEngine::open(Fake::new(mode), &temp.path().join("h.sqlite")).unwrap();
        let id = start(&e);
        assert_eq!(wait(&e).phase, expected);
        assert!(e.scan(&id, vec![]).is_err());
    }
}
#[test]
fn restart_recovery_requires_boot_and_installation_checks_without_replay() {
    for (installed, unknown, expected) in [
        (false, false, UninstallPhase::Removed),
        (true, false, UninstallPhase::StillInstalled),
        (false, true, UninstallPhase::Unknown),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("restart.sqlite");
        let fake = Fake::new(2);
        let engine = SoftwareEngine::open(fake.clone(), &db).unwrap();
        let id = start(&engine);
        assert_eq!(wait(&engine).phase, UninstallPhase::RestartRequired);
        drop(engine);
        let recovered = SoftwareEngine::open(fake.clone(), &db).unwrap();
        assert!(recovered.verify(&id).is_err());
        fake.boot_unknown.store(true, Ordering::SeqCst);
        assert!(recovered.verify(&id).is_err());
        assert_eq!(
            recovered.snapshot().unwrap().phase,
            UninstallPhase::RestartRequired
        );
        assert!(recovered.scan(&id, vec![]).is_err());
        fake.boot_unknown.store(false, Ordering::SeqCst);
        fake.restarted.store(true, Ordering::SeqCst);
        fake.installed.store(installed, Ordering::SeqCst);
        fake.unknown.store(unknown, Ordering::SeqCst);
        recovered.verify(&id).unwrap();
        let task = wait(&recovered);
        assert_eq!(task.phase, expected);
        assert_eq!(task.id, id);
        assert_eq!(task.residual.deleted_bytes, "0");
        assert_eq!(recovered.history().unwrap().len(), 1);
        assert!(recovered.scan(&id, vec![]).is_err());
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn residual_path_search_accepts_windows_separators_without_changing_authority() {
    for mixed in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let fake = Fake::new(0);
        fake.mixed_separators.store(mixed, Ordering::SeqCst);
        let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
        let id = start(&engine);
        wait(&engine);
        engine.scan(&id, vec![]).unwrap();
        wait(&engine);
        let all = engine.residuals(&id, 0, "").unwrap();
        let original = all.items.iter().find(|item| item.id == "allowed").unwrap();
        for query in [
            r"C:\fixture\settings.json",
            "c:/FIXTURE/settings.json",
            r"fixture/settings",
        ] {
            let found = engine.residuals(&id, 0, query).unwrap();
            assert_eq!(found.total, 1, "query {query}, mixed={mixed}");
            assert_eq!(found.scan_id, all.scan_id);
            assert_eq!(found.items[0].id, original.id);
            assert_eq!(found.items[0].path, original.path);
            assert!(found.items[0].selectable);
            let plan = engine
                .prepare_cleanup(&id, vec![found.items[0].id.clone()])
                .unwrap();
            assert_eq!(plan.files, 1);
            assert_eq!(plan.bytes, "12");
        }
        let retained = engine.residuals(&id, 0, "c:/fixture/shared").unwrap();
        assert_eq!(retained.total, 1);
        assert!(!retained.items[0].selectable);
        assert!(
            engine
                .prepare_cleanup(&id, vec![retained.items[0].id.clone()])
                .is_err()
        );
        assert_eq!(engine.residuals(&id, 0, "c:/other").unwrap().total, 0);
        assert_eq!(engine.residuals(&id, 1, "settings").unwrap().items.len(), 0);
        assert_eq!(engine.residuals(&id, 0, "").unwrap().total, all.total);
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn residual_manifest_is_explicit_deduplicated_and_preserves_readonly() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let e = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
    let id = start(&e);
    wait(&e);
    e.scan(&id, vec![]).unwrap();
    wait(&e);
    assert_eq!(e.residuals(&id, 0, "settings").unwrap().total, 1);
    assert!(e.prepare_cleanup(&id, vec![]).is_err());
    assert!(e.prepare_cleanup(&id, vec!["protected".into()]).is_err());
    assert!(
        e.prepare_cleanup(&id, vec!["C:\\arbitrary".into()])
            .is_err()
    );
    let p = e
        .prepare_cleanup(&id, vec!["allowed".into(), "allowed".into()])
        .unwrap();
    assert_eq!(p.files, 1);
    assert_eq!(p.bytes, "12");
    assert!(p.includes_data);
    assert_eq!(p.preview.len(), 1);
    assert_eq!(p.preview[0].id, "allowed");
    e.clean(&p.id, vec![]).unwrap();
    let t = wait(&e);
    assert_eq!(t.residual.deleted_bytes, "12");
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 1);
    assert!(e.clean(&p.id, vec![]).is_err());
}
#[test]
fn store_local_cache_selection_requires_the_data_deletion_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.store_local_cache.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
    let id = start(&engine);
    assert_eq!(wait(&engine).software.source, SoftwareSource::Store);
    engine.scan(&id, vec![]).unwrap();
    wait(&engine);
    let manifest = engine.residuals(&id, 0, "LocalCache").unwrap();
    assert_eq!(manifest.total, 1);
    assert_eq!(manifest.items[0].category, "data");
    assert!(engine.prepare_cleanup(&id, vec![]).is_err());
    let plan = engine
        .prepare_cleanup(&id, vec![manifest.items[0].id.clone()])
        .unwrap();
    assert!(plan.includes_data);
    assert_eq!(plan.preview[0].category, "data");
    assert_eq!(plan.files, 1);
    assert_eq!(plan.bytes, "12");
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
}
#[test]
fn reinstall_and_new_exclusions_prevent_deletion() {
    for reinstall in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let fake = Fake::new(0);
        let e = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
        let id = start(&e);
        wait(&e);
        e.scan(&id, vec![]).unwrap();
        wait(&e);
        let p = e.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
        fake.installed.store(reinstall, Ordering::SeqCst);
        e.clean(
            &p.id,
            if reinstall {
                vec![]
            } else {
                vec!["protected root".into()]
            },
        )
        .unwrap();
        let t = wait(&e);
        assert_eq!(t.residual.skipped, 1);
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn closing_during_an_inflight_delete_records_uncertainty_without_replaying() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("h.sqlite");
    let fake = Fake::new(0);
    let e = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let id = start(&e);
    wait(&e);
    e.scan(&id, vec![]).unwrap();
    wait(&e);
    let p = e.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
    fake.hold_delete.store(true, Ordering::SeqCst);
    e.clean(&p.id, vec![]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !fake.deleting.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    e.stop_tracking();
    let resumed = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let restored = resumed.snapshot().unwrap();
    assert_eq!(restored.phase, UninstallPhase::Unknown);
    assert_eq!(restored.uninstall_status, Some(UninstallStatus::Removed));
    assert_eq!(restored.residual.unknown, 1);
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert!(resumed.clean(&p.id, vec![]).is_err());
    fake.hold_delete.store(false, Ordering::SeqCst);
    wait_until(|| Arc::strong_count(&e) == 1);
    assert_eq!(e.snapshot().unwrap().phase, UninstallPhase::Unknown);
    let report = resumed.history().unwrap().remove(0);
    assert_eq!(report.phase, UninstallPhase::Unknown);
    assert_eq!(report.uninstall_status, Some(UninstallStatus::Removed));
    assert_eq!(report.residual.unknown, 1);
    assert_eq!(report.residual.deleted_files, 0);
}
#[test]
fn unavailable_source_never_means_removed() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.unknown.store(true, Ordering::SeqCst);
    let e = SoftwareEngine::open(fake, &temp.path().join("h.sqlite")).unwrap();
    start(&e);
    let task = wait(&e);
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert_eq!(task.uninstall_status, Some(UninstallStatus::Unknown));
}

#[test]
fn legacy_and_current_journals_preserve_only_supported_standard_uninstall_results() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("status.sqlite");
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    start(&engine);
    assert_eq!(
        wait(&engine).uninstall_status,
        Some(UninstallStatus::Removed)
    );
    drop(engine);
    let db = rusqlite::Connection::open(&path).unwrap();
    let original: String = db
        .query_row("SELECT data FROM uninstall_reports", [], |r| r.get(0))
        .unwrap();
    for (phase, saved_status, expected) in [
        ("uninstalling", None, UninstallStatus::Unknown),
        ("verifying", Some("verifying"), UninstallStatus::Unknown),
        ("scanning_residuals", None, UninstallStatus::Removed),
        ("cleaning_residuals", None, UninstallStatus::Removed),
        ("unknown", None, UninstallStatus::Unknown),
        ("unknown", Some("removed"), UninstallStatus::Removed),
        ("restart_required", None, UninstallStatus::RestartRequired),
        ("still_installed", None, UninstallStatus::StillInstalled),
    ] {
        let mut saved: serde_json::Value = serde_json::from_str(&original).unwrap();
        saved["phase"] = phase.into();
        if let Some(status) = saved_status {
            saved["uninstall_status"] = status.into();
        } else {
            saved.as_object_mut().unwrap().remove("uninstall_status");
        }
        db.execute("UPDATE uninstall_reports SET data=?1", [saved.to_string()])
            .unwrap();
        let recovered = SoftwareEngine::open(fake.clone(), &path).unwrap();
        let task = recovered.snapshot().unwrap();
        assert_eq!(task.uninstall_status, Some(expected), "{phase}");
        assert_eq!(
            recovered.history().unwrap()[0].uninstall_status,
            Some(expected)
        );
        assert!(!task.can_scan_residuals);
        assert!(recovered.scan(&task.id, vec![]).is_err());
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn recovery_preserves_unknown_without_relaunching_or_path_leak() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("h.sqlite");
    let fake = Fake::new(0);
    let e = SoftwareEngine::open(fake.clone(), &path).unwrap();
    start(&e);
    wait(&e);
    let report = e.history().unwrap().remove(0);
    assert!(report.software.install_location.is_none());
    drop(e);
    let db = rusqlite::Connection::open(&path).unwrap();
    let data: String = db
        .query_row("SELECT data FROM uninstall_reports", [], |r| r.get(0))
        .unwrap();
    assert!(!data.contains("private-test-location"));
    assert!(!data.contains("private context"));
    let mut value: serde_json::Value = serde_json::from_str(&data).unwrap();
    value["phase"] = "cleaning_residuals".into();
    db.execute("UPDATE uninstall_reports SET data=?1", [value.to_string()])
        .unwrap();
    drop(db);
    let recovered = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let task = recovered.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert_eq!(task.residual.unknown, 1);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert!(!task.can_scan_residuals);
    assert!(task.residual_scan_id.is_none());
    recovered.verify(&task.id).unwrap();
    let verified = wait(&recovered);
    assert_eq!(verified.phase, UninstallPhase::Removed);
    assert!(!verified.can_scan_residuals);
    assert!(verified.message.contains("本次不能检查或清理残留"));
    assert!(recovered.scan(&task.id, vec![]).is_err());
}

#[test]
fn launcher_registration_pending_survives_restart_without_restoring_authority() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("pending.sqlite");
    let fake = Fake::new(5);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let id = start(&engine);
    assert_eq!(wait(&engine).phase, UninstallPhase::StillInstalled);
    drop(engine);
    // Simulate a journal from the previous version without live-capability fields.
    let db = rusqlite::Connection::open(&path).unwrap();
    let data: String = db
        .query_row("SELECT data FROM uninstall_reports", [], |r| r.get(0))
        .unwrap();
    let mut old: serde_json::Value = serde_json::from_str(&data).unwrap();
    old.as_object_mut().unwrap().remove("can_scan_residuals");
    old.as_object_mut().unwrap().remove("residual_scan_id");
    db.execute("UPDATE uninstall_reports SET data=?1", [old.to_string()])
        .unwrap();
    drop(db);
    let resumed = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let pending = resumed.snapshot().unwrap();
    assert_eq!(pending.id, id);
    assert_eq!(pending.phase, UninstallPhase::StillInstalled);
    assert!(!pending.can_scan_residuals);
    fake.installed.store(false, Ordering::SeqCst);
    resumed.verify(&id).unwrap();
    let verified = wait(&resumed);
    assert_eq!(verified.phase, UninstallPhase::Removed);
    assert!(!verified.can_scan_residuals);
    assert!(resumed.scan(&id, vec![]).is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fake.scans.load(Ordering::SeqCst), 0);
}

#[test]
fn each_rescan_replaces_manifest_and_invalidates_prior_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.fresh_ids.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("rescan.sqlite")).unwrap();
    let id = start(&engine);
    assert!(wait(&engine).can_scan_residuals);
    engine.scan(&id, vec![]).unwrap();
    let first = wait(&engine);
    let manifest = engine.residuals(&id, 0, "").unwrap();
    assert_eq!(
        first.residual_scan_id.as_deref(),
        Some(manifest.scan_id.as_str())
    );
    let old_plan = engine
        .prepare_cleanup(&id, vec!["allowed-0".into()])
        .unwrap();
    engine.scan(&id, vec![]).unwrap();
    let second = wait(&engine);
    assert_eq!(first.phase, second.phase);
    assert_ne!(first.residual_scan_id, second.residual_scan_id);
    assert!(engine.clean(&old_plan.id, vec![]).is_err());
    assert!(
        engine
            .prepare_cleanup(&id, vec!["allowed-0".into()])
            .is_err()
    );
    assert!(
        engine
            .prepare_cleanup(&id, vec!["allowed-1".into()])
            .is_ok()
    );
    let history = engine.history().unwrap();
    assert!(!history[0].can_scan_residuals);
    assert!(history[0].residual_scan_id.is_none());
    fake.scan_failure.store(true, Ordering::SeqCst);
    engine.scan(&id, vec![]).unwrap();
    let failed = wait(&engine);
    assert_eq!(failed.phase, UninstallPhase::Removed);
    assert!(failed.can_scan_residuals);
    assert!(failed.residual_scan_id.is_none());
    assert!(engine.residuals(&id, 0, "").is_err());
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
}

#[test]
fn readonly_results_do_not_invite_selection_or_grant_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.readonly.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake, &temp.path().join("readonly.sqlite")).unwrap();
    let id = start(&engine);
    wait(&engine);
    engine.scan(&id, vec![]).unwrap();
    let task = wait(&engine);
    assert!(task.message.contains("仅支持只读检查"));
    assert!(!task.message.contains("请选择"));
    assert!(engine.prepare_cleanup(&id, vec!["allowed".into()]).is_err());
}

#[test]
fn failed_uninstall_preparation_revokes_the_previous_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("prepare.sqlite")).unwrap();
    engine.list().unwrap();
    let old = engine.prepare("app-1").unwrap();
    assert!(engine.prepare("unknown-app").is_err());
    assert!(engine.start(&old.id).is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);

    let old = engine.prepare("app-1").unwrap();
    fake.prepare_failure.store(true, Ordering::SeqCst);
    assert!(engine.prepare("app-1").is_err());
    assert!(engine.start(&old.id).is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);

    fake.prepare_failure.store(false, Ordering::SeqCst);
    let fresh = engine.prepare("app-1").unwrap();
    engine.start(&fresh.id).unwrap();
    assert_eq!(wait(&engine).phase, UninstallPhase::Removed);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_inventory_refresh_revokes_old_software_and_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("inventory.sqlite")).unwrap();
    engine.list().unwrap();
    let old = engine.prepare("app-1").unwrap();
    fake.inventory_failure.store(true, Ordering::SeqCst);
    assert!(engine.list().is_err());
    assert!(engine.start(&old.id).is_err());
    assert!(engine.prepare("app-1").is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    fake.inventory_failure.store(false, Ordering::SeqCst);
    engine.list().unwrap();
    let fresh = engine.prepare("app-1").unwrap();
    engine.start(&fresh.id).unwrap();
    wait(&engine);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_inventory_cannot_replace_latest_list_or_revoke_its_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.hold_first_inventory.store(true, Ordering::SeqCst);
    fake.inventory_fresh_ids.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("list.sqlite")).unwrap();
    let old_engine = engine.clone();
    let old = std::thread::spawn(move || old_engine.list());
    wait_until(|| fake.inventory_calls.load(Ordering::SeqCst) == 1);
    let latest = engine.list().unwrap();
    assert_eq!(latest.items[0].id, "inventory-1");
    let plan = engine.prepare(&latest.items[0].id).unwrap();
    fake.hold_first_inventory.store(false, Ordering::SeqCst);
    assert!(old.join().unwrap().is_err(), "stale response was accepted");
    engine.start(&plan.id).unwrap();
    assert_eq!(wait(&engine).phase, UninstallPhase::Removed);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_inventory_cannot_restore_authority_after_newer_refresh_failed() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    fake.hold_first_inventory.store(true, Ordering::SeqCst);
    fake.inventory_fresh_ids.store(true, Ordering::SeqCst);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("list.sqlite")).unwrap();
    let old_engine = engine.clone();
    let old = std::thread::spawn(move || old_engine.list());
    wait_until(|| fake.inventory_calls.load(Ordering::SeqCst) == 1);
    fake.inventory_failure.store(true, Ordering::SeqCst);
    assert!(engine.list().is_err());
    fake.hold_first_inventory.store(false, Ordering::SeqCst);
    assert!(
        old.join().unwrap().is_err(),
        "stale response revived software IDs"
    );
    assert!(engine.prepare("inventory-0").is_err());
    assert!(engine.prepare("inventory-1").is_err());
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    fake.inventory_failure.store(false, Ordering::SeqCst);
    let latest = engine.list().unwrap();
    assert_eq!(latest.items[0].id, "inventory-2");
    assert!(engine.prepare(&latest.items[0].id).is_ok());
}

#[test]
fn rejected_residual_selection_revokes_the_previous_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("selection.sqlite")).unwrap();
    let id = start(&engine);
    wait(&engine);
    engine.scan(&id, vec![]).unwrap();
    wait(&engine);
    for (task, selection) in [
        (id.as_str(), vec![]),
        (id.as_str(), vec!["protected".into()]),
        (id.as_str(), vec!["foreign-candidate".into()]),
        ("previous-task", vec!["allowed".into()]),
    ] {
        let old = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
        assert!(engine.prepare_cleanup(task, selection).is_err());
        assert!(engine.clean(&old.id, vec![]).is_err());
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
        assert_eq!(
            engine.snapshot().unwrap().phase,
            UninstallPhase::ResidualsReady
        );
    }
    let fresh = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
    engine.clean(&fresh.id, vec![]).unwrap();
    assert_eq!(wait(&engine).residual.deleted_bytes, "12");
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 1);
}

fn refuse_journal_writes(path: &std::path::Path) -> rusqlite::Connection {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_uninstall_report BEFORE INSERT ON uninstall_reports BEGIN SELECT RAISE(FAIL,'fixture journal failure'); END;").unwrap();
    db
}

#[test]
fn uninstall_journal_failure_keeps_prior_task_and_consumes_confirmation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("uninstall-write.sqlite");
    let fake = Fake::new(5);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let original = start(&engine);
    assert_eq!(wait(&engine).phase, UninstallPhase::StillInstalled);
    let plan = engine.prepare("app-1").unwrap();
    let db = refuse_journal_writes(&path);
    assert!(engine.start(&plan.id).is_err());
    assert!(!engine.is_busy());
    let retained = engine.snapshot().expect("prior task must not disappear");
    assert_eq!(retained.id, original);
    assert_eq!(retained.phase, UninstallPhase::StillInstalled);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    db.execute_batch("DROP TRIGGER refuse_uninstall_report;")
        .unwrap();
    assert!(engine.start(&plan.id).is_err());
    let fresh = engine.prepare("app-1").unwrap();
    let next = engine.start(&fresh.id).unwrap();
    assert_ne!(next, original);
    wait(&engine);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn scan_journal_failure_never_sticks_busy_or_preserves_old_manifest() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("scan-write.sqlite");
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let id = start(&engine);
    wait(&engine);
    engine.scan(&id, vec![]).unwrap();
    wait(&engine);
    let old = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
    let db = refuse_journal_writes(&path);
    assert!(engine.scan(&id, vec![]).is_err());
    assert!(!engine.is_busy());
    let task = engine.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Removed);
    assert!(task.message.contains("历史记录保存失败"));
    assert!(task.can_scan_residuals);
    assert!(task.residual_scan_id.is_none());
    assert!(engine.residuals(&id, 0, "").is_err());
    assert!(engine.clean(&old.id, vec![]).is_err());
    assert_eq!(fake.scans.load(Ordering::SeqCst), 1);
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    db.execute_batch("DROP TRIGGER refuse_uninstall_report;")
        .unwrap();
    engine.scan(&id, vec![]).unwrap();
    assert_eq!(wait(&engine).phase, UninstallPhase::ResidualsReady);
    assert_eq!(fake.scans.load(Ordering::SeqCst), 2);
}

#[test]
fn cleanup_journal_failure_requires_fresh_confirmation_without_deleting() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cleanup-write.sqlite");
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let id = start(&engine);
    wait(&engine);
    engine.scan(&id, vec![]).unwrap();
    let ready = wait(&engine);
    let old = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
    let db = refuse_journal_writes(&path);
    assert!(engine.clean(&old.id, vec![]).is_err());
    assert!(!engine.is_busy());
    let task = engine.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::ResidualsReady);
    assert_eq!(task.residual_scan_id, ready.residual_scan_id);
    assert!(task.message.contains("历史记录保存失败"));
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    db.execute_batch("DROP TRIGGER refuse_uninstall_report;")
        .unwrap();
    assert!(engine.clean(&old.id, vec![]).is_err());
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
    let fresh = engine.prepare_cleanup(&id, vec!["allowed".into()]).unwrap();
    engine.clean(&fresh.id, vec![]).unwrap();
    assert_eq!(wait(&engine).residual.deleted_bytes, "12");
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 1);
}

fn prepare_three_residuals(engine: &Arc<SoftwareEngine>, fake: &Fake) -> ResidualCleanupPlan {
    fake.record_count.store(3, Ordering::SeqCst);
    let id = start(engine);
    wait(engine);
    engine.scan(&id, vec![]).unwrap();
    wait(engine);
    engine
        .prepare_cleanup(
            &id,
            vec!["allowed".into(), "extra-1".into(), "extra-2".into()],
        )
        .unwrap()
}

#[test]
fn stopping_cleanup_counts_unattempted_selection_separately() {
    let temp = tempfile::tempdir().unwrap();
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &temp.path().join("h.sqlite")).unwrap();
    let plan = prepare_three_residuals(&engine, &fake);
    fake.hold_delete.store(true, Ordering::SeqCst);
    engine.clean(&plan.id, vec![]).unwrap();
    wait_until(|| fake.deleting.load(Ordering::SeqCst));
    engine.cancel_residuals(&plan.task_id).unwrap();
    fake.hold_delete.store(false, Ordering::SeqCst);
    let task = wait(&engine);
    assert_eq!(task.phase, UninstallPhase::Finished);
    assert_eq!(task.residual.deleted_files, 1);
    assert_eq!(task.residual.deleted_bytes, "12");
    assert_eq!(task.residual.unprocessed, 2);
    assert_eq!(task.residual.unknown, 0);
    assert_eq!(task.residual.skipped + task.residual.failed, 0);
    assert_eq!(engine.history().unwrap()[0].residual.unprocessed, 2);
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 1);
    assert!(engine.clean(&plan.id, vec![]).is_err());
}

#[test]
fn failed_delete_intent_never_reaches_the_filesystem() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("intent.sqlite");
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let plan = prepare_three_residuals(&engine, &fake);
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER refuse_intent BEFORE INSERT ON uninstall_reports WHEN json_extract(NEW.data, '$.cleanup_progress.in_flight') = 1 BEGIN SELECT RAISE(FAIL,'fixture intent failure'); END;").unwrap();
    engine.clean(&plan.id, vec![]).unwrap();
    let task = wait(&engine);
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert_eq!(task.residual.unprocessed, 3);
    assert_eq!(task.residual.unknown, 0);
    assert!(!fake.deleting.load(Ordering::SeqCst));
    assert!(!task.can_scan_residuals);
    assert!(task.residual_scan_id.is_none());
    assert_eq!(engine.history().unwrap()[0].residual.unprocessed, 3);
}

#[test]
fn failed_delete_result_save_recovers_only_the_durable_intent_without_replay() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("result.sqlite");
    let fake = Fake::new(0);
    let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let plan = prepare_three_residuals(&engine, &fake);
    fake.hold_delete.store(true, Ordering::SeqCst);
    engine.clean(&plan.id, vec![]).unwrap();
    wait_until(|| fake.deleting.load(Ordering::SeqCst));
    let db = refuse_journal_writes(&path);
    let saved: String = db
        .query_row("SELECT data FROM uninstall_reports", [], |r| r.get(0))
        .unwrap();
    assert!(!saved.contains("settings"));
    assert!(!saved.contains("private snapshot"));
    let value: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert_eq!(value["cleanup_progress"]["remaining"], 2);
    assert_eq!(value["cleanup_progress"]["in_flight"], true);
    fake.hold_delete.store(false, Ordering::SeqCst);
    wait_until(|| Arc::strong_count(&engine) == 1);
    let task = engine.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert_eq!(task.residual.deleted_files, 1);
    assert_eq!(task.residual.unprocessed, 2);
    assert_eq!(task.residual.unknown, 0);
    assert!(!task.can_scan_residuals);
    drop(engine);
    db.execute_batch("DROP TRIGGER refuse_uninstall_report;")
        .unwrap();
    let resumed = SoftwareEngine::open(fake.clone(), &path).unwrap();
    let task = resumed.snapshot().unwrap();
    assert_eq!(task.phase, UninstallPhase::Unknown);
    assert_eq!(task.residual.deleted_files, 0);
    assert_eq!(task.residual.unknown, 1);
    assert_eq!(task.residual.unprocessed, 2);
    assert!(resumed.clean(&plan.id, vec![]).is_err());
    assert_eq!(fake.deletes.load(Ordering::SeqCst), 1);
    assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn recovery_before_or_between_deletions_does_not_invent_an_inflight_file() {
    for (completed, remaining) in [(0, 3), (1, 2), (3, 0)] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("progress.sqlite");
        let fake = Fake::new(0);
        let engine = SoftwareEngine::open(fake.clone(), &path).unwrap();
        start(&engine);
        wait(&engine);
        wait_until(|| Arc::strong_count(&engine) == 1);
        drop(engine);
        let db = rusqlite::Connection::open(&path).unwrap();
        let saved: String = db
            .query_row("SELECT data FROM uninstall_reports", [], |r| r.get(0))
            .unwrap();
        let mut data: serde_json::Value = serde_json::from_str(&saved).unwrap();
        data["phase"] = "cleaning_residuals".into();
        data["residual"]["deleted_files"] = completed.into();
        data["cleanup_progress"] = serde_json::json!({"remaining": remaining, "in_flight": false});
        db.execute("UPDATE uninstall_reports SET data=?1", [data.to_string()])
            .unwrap();
        let resumed = SoftwareEngine::open(fake.clone(), &path).unwrap();
        let task = resumed.snapshot().unwrap();
        assert_eq!(task.phase, UninstallPhase::Unknown);
        assert_eq!(task.residual.deleted_files, completed);
        assert_eq!(task.residual.unprocessed, remaining);
        assert_eq!(task.residual.unknown, 0);
        assert!(!task.can_scan_residuals);
        assert_eq!(fake.deletes.load(Ordering::SeqCst), 0);
        drop(resumed);
        let again = SoftwareEngine::open(fake.clone(), &path).unwrap();
        assert_eq!(again.snapshot().unwrap().residual.unprocessed, remaining);
        assert_eq!(again.snapshot().unwrap().residual.unknown, 0);
    }
}
