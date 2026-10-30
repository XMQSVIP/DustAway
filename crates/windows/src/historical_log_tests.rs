//! Only self-created, identity-bound temporary directories can receive this permit.
use super::*;
use crate::historical_log_probe::fixture::FixtureProbe;
use crate::historical_logs::{AGE_DAYS, Policy, RULE_IDS};
use dustaway_core::{BlockReason, Engine, JobState, Selection, Settings};
use sha2::{Digest, Sha256};
use std::{
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    sync::{Arc, Mutex, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

const MARKER: &str = "dustaway-historical-log-synthetic-only";
#[path = "historical_log_hardening_tests.rs"]
mod hardening;
pub(super) struct LogPermit {
    directory: tempfile::TempDir,
    root_identity: Identity,
    marker_stamp: Stamp,
    rule_id: &'static str,
    pub(super) probe: FixtureProbe,
    enabled: AtomicBool,
}
impl LogPermit {
    pub(super) fn require(&self, platform: &WindowsPlatform, rule: &RuleDescriptor) -> Result<()> {
        ensure!(
            self.enabled.load(Ordering::Acquire),
            Refusal(ReasonCode::ValidationPending)
        );
        ensure!(
            rule.id == self.rule_id && RULE_IDS.contains(&self.rule_id),
            Refusal(ReasonCode::ValidationPending)
        );
        dustaway_core::rules::validate(rule)?;
        ensure!(
            platform.scope == Scope::User,
            Refusal(ReasonCode::ValidationPending)
        );
        let base = self.directory.path();
        ensure!(
            base.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dustaway-log-acceptance-")
        );
        ensure!(extended(base.parent().unwrap())? == extended(&std::env::temp_dir())?);
        for (actual, name) in [
            (&platform.local, "local"),
            (&platform.roaming, "roaming"),
            (&platform.system, "system"),
        ] {
            ensure!(extended(actual)? == extended(&base.join(name))?);
        }
        let root = PinnedPath::open(base, true, false)?;
        ensure!(root.stamp.identity == self.root_identity);
        let marker = PinnedPath::open(&base.join(".dustaway-test-only"), false, false)?;
        ensure!(marker.stamp == self.marker_stamp);
        ensure!(std::fs::read_to_string(&marker.path)? == MARKER);
        Ok(())
    }
}
struct Fixture {
    platform: Arc<WindowsPlatform>,
    permit: Arc<LogPermit>,
    files: Vec<PathBuf>,
    protected: Vec<PathBuf>,
    journal: PathBuf,
}
fn write(path: &Path, age: i64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"self-created diagnostic fixture").unwrap();
    filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(now() - age, 0)).unwrap();
}
fn rule(id: &str) -> RuleDescriptor {
    dustaway_core::app_cache::rule(id).unwrap().descriptor()
}
fn fixture(id: &'static str, enabled: bool) -> Fixture {
    let directory = tempfile::Builder::new()
        .prefix("dustaway-log-acceptance-")
        .tempdir()
        .unwrap();
    let base = directory.path();
    let marker = base.join(".dustaway-test-only");
    std::fs::write(&marker, MARKER).unwrap();
    let marker_stamp = PinnedPath::open(&marker, false, false).unwrap().stamp;
    let root_identity = PinnedPath::open(base, true, false).unwrap().stamp.identity;
    let local = extended(&base.join("local")).unwrap();
    let roaming = extended(&base.join("roaming")).unwrap();
    let system = extended(&base.join("system")).unwrap();
    for p in [&local, &roaming, &system] {
        std::fs::create_dir_all(p).unwrap();
    }
    let policy = Policy::for_rule(id).unwrap();
    let mut files = vec![];
    let mut protected = vec![];
    for root in policy.roots(&local, &roaming).unwrap() {
        for name in ["old.log", "old.xlog", "older.LOG"] {
            let p = root.join(name);
            write(&p, 8 * 86400);
            files.push(p);
        }
        for name in [
            "buffer.mmap",
            "unknown.bin",
            "chat.db",
            "backup.log.bak",
            "cache/nested.log",
        ] {
            let p = root.join(name);
            write(&p, 60 * 86400);
            protected.push(p);
        }
        for (name, age) in [("recent.log", 1), ("future.xlog", -86400)] {
            let p = root.join(name);
            write(&p, age);
            protected.push(p);
        }
    }
    let app_base = if id == RULE_IDS[0] {
        roaming.join("DingTalk")
    } else {
        local.join("ToDesk")
    };
    for name in [
        "recordLog/old.log",
        "updaterlogs/old.log",
        "chat/messages.db",
        "accounts/user.json",
        "config.ini",
        "documents/work.docx",
        "EBWebView/Cache/old.log",
    ] {
        let p = app_base.join(name);
        write(&p, 60 * 86400);
        protected.push(p);
    }
    let journal = base.join("journal.sqlite");
    let probe = FixtureProbe::new(base, id == RULE_IDS[0]);
    let permit = Arc::new(LogPermit {
        directory,
        root_identity,
        marker_stamp,
        rule_id: id,
        probe,
        enabled: AtomicBool::new(true),
    });
    let platform = Arc::new(WindowsPlatform {
        inventory: Mutex::new(Some(crate::app_cache::Inventory {
            entries: vec![],
            complete: true,
        })),
        tool_roots: Default::default(),
        tool_probe: Arc::new(crate::tool_discovery::UnavailableProbe),
        scope: Scope::User,
        local,
        roaming,
        system,
        app_test_identity: None,
        app_test_acceptance: None,
        real_app_acceptance: None,
        log_test_acceptance: enabled.then(|| permit.clone()),
        discovery_drives: vec![],
    });
    permit.require(&platform, &rule(id)).unwrap();
    Fixture {
        platform,
        permit,
        files,
        protected,
        journal,
    }
}
fn wait(engine: &Engine) {
    let until = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn scan(engine: &Arc<Engine>, id: &str) -> String {
    let session = engine.start_scan(vec![id.into()]).unwrap();
    wait(engine);
    assert_eq!(engine.status().state, JobState::Ready);
    session
}
fn selection(id: &str) -> Selection {
    Selection {
        rule_ids: vec![id.into()],
        reviewed_rule_ids: vec![id.into()],
        ..Default::default()
    }
}
fn records(f: &Fixture, id: &str) -> Vec<FileRecord> {
    let mut files = vec![];
    f.platform
        .scan(&rule(id), &[], &AtomicBool::new(false), &mut |event| {
            if let ScanEvent::File(file) = event {
                files.push(file);
            }
        })
        .unwrap();
    files
}
fn hashes(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| format!("{:x}", Sha256::digest(std::fs::read(p).unwrap())))
        .collect()
}

#[test]
fn historical_logs_engine_selects_one_file_preserves_neighbors_and_never_replays() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
        let session = scan(&engine, id);
        let page = engine.candidates(&session, id, 0, 200).unwrap();
        assert_eq!(page.total as usize, f.files.len());
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        rule_ids: vec![id.into()],
                        ..Default::default()
                    }
                )
                .is_err()
        );
        let chosen = &page.items[0];
        let plan = engine
            .prepare(
                &session,
                Selection {
                    included_ids: vec![chosen.id.clone()],
                    reviewed_rule_ids: vec![id.into()],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(plan.files, 1);
        let late = f.files[0].parent().unwrap().join("new-after-plan.log");
        write(&late, 30 * 86400);
        let mut protected = f.protected.clone();
        protected.push(late);
        protected.extend(
            f.files
                .iter()
                .filter(|p| display_path(p) != chosen.path)
                .cloned(),
        );
        let before = hashes(&protected);
        assert!(engine.execute("forged").is_err());
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        let report = engine.report().unwrap();
        assert_eq!(
            (
                report.deleted,
                report.skipped,
                report.failed,
                report.unknown
            ),
            (1, 0, 0, 0)
        );
        assert_eq!(report.deleted_bytes, chosen.bytes);
        assert!(!Path::new(&chosen.path).exists());
        assert_eq!(before, hashes(&protected));
        assert!(engine.execute(&plan.id).is_err());
        println!(
            "HISTORICAL_LOG_SYNTHETIC rule={id} selected=1 deleted=1 protected={}",
            protected.len()
        );
    }
}

#[test]
fn historical_logs_production_observes_full_scope_but_rejects_prepare_and_direct_delete() {
    for id in RULE_IDS {
        let f = fixture(id, false);
        let before = hashes(&f.files);
        let files = records(&f, id);
        assert!(files.iter().any(|r| r.path_display.ends_with("nested.log")));
        if id == RULE_IDS[0] {
            assert!(files.iter().any(|r| r.path_display.contains("recordLog")));
        }
        assert!(!f.platform.cleanup_validated(&rule(id)));
        for file in &files {
            assert_eq!(
                error_reason(&f.platform.delete(&rule(id), file, &[]).unwrap_err()),
                ReasonCode::ValidationPending
            );
        }
        let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
        let session = scan(&engine, id);
        assert!(engine.prepare(&session, selection(id)).is_err());
        assert_eq!(before, hashes(&f.files));
        let rules = dustaway_core::rules::for_scope(Scope::User).unwrap();
        assert_eq!(rules.len(), 145);
        assert_eq!(
            rules
                .iter()
                .filter(|r| dustaway_core::cleanup_acceptance::release_enabled(r))
                .count(),
            42
        );
    }
}

#[test]
fn historical_logs_recheck_installation_process_age_and_file_changes_at_prepare_and_delete() {
    for id in RULE_IDS {
        for (change, before_prepare) in [
            "identity",
            "unverified",
            "running",
            "unknown",
            "modified",
            "recent",
            "revoked",
        ]
        .into_iter()
        .flat_map(|change| [(change, true), (change, false)])
        {
            let f = fixture(id, true);
            let files = records(&f, id);
            let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
            let session = scan(&engine, id);
            let plan = engine.prepare(&session, selection(id)).unwrap();
            match change {
                "identity" => {
                    f.permit.probe.replace_program();
                }
                "unverified" => {
                    f.permit
                        .probe
                        .facts
                        .lock()
                        .unwrap()
                        .get_mut(&f.permit.probe.exe)
                        .unwrap()
                        .publisher = "untrusted".into();
                }
                "running" => {
                    f.permit.probe.set_running(
                        f.permit.probe.exe.file_name().unwrap().to_str().unwrap(),
                        Some(2),
                        None,
                    );
                }
                "unknown" => f.permit.probe.process_state.lock().unwrap().complete = false,
                "revoked" => f.permit.enabled.store(false, Ordering::Release),
                other => {
                    for file in &f.files {
                        if other == "modified" {
                            std::fs::write(file, b"changed contents").unwrap();
                        } else {
                            filetime::set_file_mtime(
                                file,
                                filetime::FileTime::from_unix_time(now(), 0),
                            )
                            .unwrap();
                        }
                    }
                }
            }
            for file in &files {
                assert!(f.platform.delete(&rule(id), file, &[]).is_err());
            }
            if before_prepare {
                assert!(
                    engine.prepare(&session, selection(id)).is_err(),
                    "{id}: {change}"
                );
                assert!(f.files.iter().all(|p| p.exists()));
                continue;
            }
            engine.execute(&plan.id).unwrap();
            wait(&engine);
            let report = engine.report().unwrap();
            assert_eq!(report.deleted, 0, "{id}: {change}");
            assert_eq!(report.skipped, plan.files);
            assert!(f.files.iter().all(|p| p.exists()));
        }
    }
}

#[test]
fn historical_logs_age_cutoff_matches_snapshot_and_final_validation() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let p = &f.files[0];
        let root = p.parent().unwrap();
        let at = now();
        let cutoff = at - i64::from(AGE_DAYS) * 86400;
        filetime::set_file_mtime(p, filetime::FileTime::from_unix_time(cutoff, 0)).unwrap();
        assert!(snapshot_at(root, p, cutoff, None).is_ok());
        let file = records(&f, id)
            .into_iter()
            .find(|r| r.path_display == display_path(p))
            .unwrap();
        f.platform.validate_candidate(&rule(id), &file).unwrap();
        f.platform.delete(&rule(id), &file, &[]).unwrap();
        write(p, 8 * 86400);
        for time in [cutoff + 1, at + 86400] {
            filetime::set_file_mtime(p, filetime::FileTime::from_unix_time(time, 0)).unwrap();
            assert_eq!(
                error_reason(&snapshot_at(root, p, cutoff, None).unwrap_err()),
                ReasonCode::Recent
            );
        }
    }
}

#[test]
fn historical_logs_exclusions_busy_files_and_forged_scopes_stay_protected() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let original = records(&f, id).remove(0);
        assert!(
            f.platform
                .delete(
                    &rule(id),
                    &original,
                    std::slice::from_ref(&original.path_display)
                )
                .is_err()
        );
        for path in [&f.protected[0], &f.protected[4]] {
            let mut forged = original.clone();
            let mut snap: Snapshot = serde_json::from_str(&forged.opaque).unwrap();
            snap.path = native(path);
            forged.opaque = serde_json::to_string(&snap).unwrap();
            assert!(f.platform.validate_candidate(&rule(id), &forged).is_err());
        }
        let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
        engine
            .save_settings(Settings {
                theme: "system".into(),
                exclusions: vec![display_path(&f.files[0])],
                ..Default::default()
            })
            .unwrap();
        let session = scan(&engine, id);
        let plan = engine.prepare(&session, selection(id)).unwrap();
        assert_eq!(plan.files as usize, f.files.len() - 1);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&f.files[1])
            .unwrap();
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        drop(lock);
        assert_eq!(engine.report().unwrap().skipped, 1);
        assert!(f.files[0].exists() && f.files[1].exists());
    }
}

#[test]
fn historical_logs_permit_cannot_escape_its_rule_root_marker_or_scope() {
    for id in RULE_IDS {
        let mut f = fixture(id, true);
        let other = if id == RULE_IDS[0] {
            RULE_IDS[1]
        } else {
            RULE_IDS[0]
        };
        assert!(!f.platform.cleanup_validated(&rule(other)));
        let mut forged = rule(id);
        forged.cleanup_validated = true;
        assert!(!f.platform.cleanup_validated(&forged));
        let original = records(&f, id).remove(0);
        let platform = Arc::get_mut(&mut f.platform).unwrap();
        let saved = platform.local.clone();
        platform.local = platform.roaming.clone();
        assert!(!platform.cleanup_validated(&rule(id)));
        platform.local = saved;
        platform.scope = Scope::System;
        assert!(!platform.cleanup_validated(&rule(id)));
        platform.scope = Scope::User;
        std::fs::write(
            f.permit.directory.path().join(".dustaway-test-only"),
            "changed marker",
        )
        .unwrap();
        assert!(!platform.cleanup_validated(&rule(id)));
        assert!(platform.delete(&rule(id), &original, &[]).is_err());
        assert!(f.files.iter().all(|p| p.exists()));
    }
}

#[test]
fn historical_logs_hardlinks_and_cloud_files_are_rejected_before_and_after_scan() {
    use windows::{
        Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, SetFileAttributesW,
        },
        core::PCWSTR,
    };
    for id in RULE_IDS {
        let f = fixture(id, true);
        let originals = records(&f, id);
        let linked = &f.files[0];
        let cloud = &f.files[1];
        let outside = f.permit.directory.path().join("protected-hardlink");
        std::fs::hard_link(linked, &outside).unwrap();
        unsafe {
            SetFileAttributesW(PCWSTR(wide(cloud).as_ptr()), FILE_ATTRIBUTE_OFFLINE).unwrap();
        }
        let remaining = records(&f, id);
        assert_eq!(remaining.len(), f.files.len() - 2);
        for file in originals
            .iter()
            .filter(|r| [display_path(linked), display_path(cloud)].contains(&r.path_display))
        {
            assert!(f.platform.delete(&rule(id), file, &[]).is_err());
        }
        unsafe {
            SetFileAttributesW(PCWSTR(wide(cloud).as_ptr()), FILE_ATTRIBUTE_NORMAL).unwrap();
        }
        assert!(linked.exists() && outside.exists() && cloud.exists());
    }
}

#[test]
fn historical_logs_replaced_files_parent_directories_and_junctions_refuse_old_snapshots() {
    for id in RULE_IDS {
        for junction in [false, true] {
            let f = fixture(id, true);
            let originals = records(&f, id);
            let root = f.files[0].parent().unwrap();
            let saved = extended(&f.permit.directory.path().join("saved-log-root")).unwrap();
            assert!(root.starts_with(extended(f.permit.directory.path()).unwrap()));
            assert!(saved.starts_with(extended(f.permit.directory.path()).unwrap()));
            std::fs::rename(root, &saved).unwrap();
            if junction {
                let output = std::process::Command::new("cmd.exe")
                    .args(["/d", "/c", "mklink", "/J"])
                    .arg(display_path(root))
                    .arg(display_path(&saved))
                    .creation_flags(0x08000000)
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            } else {
                write(&root.join("old.log"), 8 * 86400);
            }
            let old = originals
                .iter()
                .find(|r| r.path_display == display_path(&root.join("old.log")))
                .unwrap();
            assert!(f.platform.validate_candidate(&rule(id), old).is_err());
            assert!(f.platform.delete(&rule(id), old, &[]).is_err());
            assert!(saved.join("old.log").is_file());
            if junction {
                std::fs::remove_dir(root).unwrap();
            }
        }
        let f = fixture(id, true);
        let old = records(&f, id).remove(0);
        let path = PathBuf::from(&old.path_display);
        let saved = f.permit.directory.path().join("saved-file");
        let time =
            filetime::FileTime::from_last_modification_time(&std::fs::metadata(&path).unwrap());
        std::fs::rename(&path, &saved).unwrap();
        std::fs::copy(&saved, &path).unwrap();
        filetime::set_file_mtime(&path, time).unwrap();
        assert!(f.platform.delete(&rule(id), &old, &[]).is_err());
        assert!(saved.exists() && path.exists());
    }
}

type Pause = (mpsc::Sender<()>, mpsc::Receiver<()>);

#[test]
fn historical_logs_scan_cancellation_exclusions_missing_and_empty_are_distinct() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let roots = f.platform.roots(&rule(id)).unwrap();
        let excluded: Vec<_> = roots.iter().map(|r| display_path(r)).collect();
        let mut events = vec![];
        f.platform
            .scan(&rule(id), &excluded, &AtomicBool::new(false), &mut |e| {
                events.push(e)
            })
            .unwrap();
        assert!(!events.iter().any(|e| matches!(
            e,
            ScanEvent::File(_) | ScanEvent::NoticeCode(ReasonCode::LocationMissing)
        )));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ScanEvent::Retained(ReasonCode::Excluded)))
        );
        let cancel = AtomicBool::new(false);
        events.clear();
        f.platform
            .scan(&rule(id), &[], &cancel, &mut |e| {
                if matches!(e, ScanEvent::Checked) {
                    cancel.store(true, Ordering::Release);
                }
                events.push(e);
            })
            .unwrap();
        assert!(cancel.load(Ordering::Acquire));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ScanEvent::File(_) | ScanEvent::CoverageIssue(_)))
        );
        assert_eq!(records(&f, id).len(), f.files.len());
        for (i, root) in roots.iter().enumerate() {
            let saved = extended(&f.permit.directory.path().join(format!("saved-{i}"))).unwrap();
            let base = extended(f.permit.directory.path()).unwrap();
            assert!(root.starts_with(&base) && saved.starts_with(&base));
            std::fs::rename(root, saved).unwrap();
        }
        events.clear();
        f.platform
            .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| {
                events.push(e)
            })
            .unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ScanEvent::NoticeCode(ReasonCode::LocationMissing)))
        );
        for root in &roots {
            std::fs::create_dir(root).unwrap();
        }
        events.clear();
        f.platform
            .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| {
                events.push(e)
            })
            .unwrap();
        assert!(!events.iter().any(|e| matches!(
            e,
            ScanEvent::File(_)
                | ScanEvent::NoticeCode(ReasonCode::LocationMissing)
                | ScanEvent::CoverageIssue(_)
        )));
        f.permit
            .probe
            .facts
            .lock()
            .unwrap()
            .get_mut(&f.permit.probe.exe)
            .unwrap()
            .publisher = "untrusted".into();
        events.clear();
        assert!(
            f.platform
                .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| events
                    .push(e))
                .is_err()
        );
        assert!(events.iter().any(|e| matches!(
            e,
            ScanEvent::CoverageIssue(ReasonCode::ApplicationUnverified)
        )));
    }
}

struct Controlled {
    platform: Arc<WindowsPlatform>,
    pause: Mutex<Option<Pause>>,
    fail: bool,
}
impl Platform for Controlled {
    fn cleanup_validated(&self, r: &RuleDescriptor) -> bool {
        self.platform.cleanup_validated(r)
    }
    fn scan(
        &self,
        r: &RuleDescriptor,
        e: &[String],
        c: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        self.platform.scan(r, e, c, emit)
    }
    fn validate_candidate(&self, r: &RuleDescriptor, f: &FileRecord) -> Result<()> {
        self.platform.validate_candidate(r, f)
    }
    fn blocked(&self, r: &RuleDescriptor) -> Result<Option<BlockReason>> {
        self.platform.blocked(r)
    }
    fn delete(&self, r: &RuleDescriptor, f: &FileRecord, e: &[String]) -> Result<()> {
        if self.fail {
            return Err(dustaway_core::DeleteFailure(
                "injected fixture-only deletion failure".into(),
            )
            .into());
        }
        self.platform.delete(r, f, e)?;
        if let Some((sent, resumed)) = self.pause.lock().unwrap().take() {
            sent.send(()).unwrap();
            resumed.recv_timeout(Duration::from_secs(20)).unwrap();
        }
        Ok(())
    }
    fn free_space(&self) -> Result<u64> {
        self.platform.free_space()
    }
}
#[test]
fn historical_logs_engine_cancel_and_failure_receipts_do_not_claim_undeleted_bytes() {
    for id in RULE_IDS {
        for fail in [false, true] {
            let f = fixture(id, true);
            let (sent, received) = mpsc::channel();
            let (resume, resumed) = mpsc::channel();
            let controlled = Arc::new(Controlled {
                platform: f.platform.clone(),
                pause: Mutex::new(Some((sent, resumed))),
                fail,
            });
            let engine = Engine::open(controlled, Scope::User, &f.journal).unwrap();
            let session = scan(&engine, id);
            let plan = engine.prepare(&session, selection(id)).unwrap();
            engine.execute(&plan.id).unwrap();
            if !fail {
                received.recv_timeout(Duration::from_secs(20)).unwrap();
                engine.cancel();
                resume.send(()).unwrap();
            }
            wait(&engine);
            let report = engine.report().unwrap();
            if fail {
                assert_eq!(
                    (report.deleted, report.failed, report.deleted_bytes.as_str()),
                    (0, plan.files, "0")
                );
            } else {
                assert_eq!(report.deleted, 1);
                assert_eq!(report.status, "cancelled");
                assert_eq!(
                    f.files.iter().filter(|p| p.exists()).count(),
                    f.files.len() - 1
                );
            }
            drop(engine);
            let recovered = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
            assert!(!recovered.is_busy());
            assert!(recovered.execute(&plan.id).is_err());
        }
    }
}
