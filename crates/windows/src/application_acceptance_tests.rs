//! Synthetic authorization exists only in rustc's test build. No environment variable can enable it.
use super::*;
use dustaway_core::{
    BlockReason, CandidateQuery, Engine, JobState, Selection, Settings,
    applications::{ApplicationFolder, ProfileLayout},
    cleanup_acceptance::{CleanupAcceptance, PREPARATION_RULES, versions_in},
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

const VERSION: &str = "0.0.0.1";
const MARKER: &str = "synthetic-application-acceptance-only";

pub(super) struct SyntheticAcceptance {
    directory: tempfile::TempDir,
    identity: Identity,
    records: Vec<CleanupAcceptance>,
    version: Mutex<String>,
    process: Mutex<Option<ReasonCode>>,
    generation: std::sync::atomic::AtomicUsize,
}

impl SyntheticAcceptance {
    pub(super) fn check_boundary(&self, platform: &WindowsPlatform) -> Result<()> {
        let root = self.directory.path();
        ensure!(
            root.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("dustaway-acceptance-")
        );
        ensure!(extended(root.parent().unwrap())? == extended(&std::env::temp_dir())?);
        ensure!(platform.scope == Scope::User);
        for (actual, name) in [
            (&platform.local, "local"),
            (&platform.roaming, "roaming"),
            (&platform.system, "system"),
        ] {
            ensure!(extended(actual)? == extended(&root.join(name))?);
        }
        let pin = PinnedPath::open(root, true, false)?;
        ensure!(pin.stamp.identity == self.identity);
        let marker = root.join(".dustaway-test-only");
        let _marker_pin = PinnedPath::open(&marker, false, false)?;
        ensure!(std::fs::read_to_string(marker)? == MARKER);
        Ok(())
    }

    pub(super) fn identity(&self) -> String {
        format!(
            "synthetic-identity:{}:{}",
            self.version.lock().unwrap(),
            self.generation.load(std::sync::atomic::Ordering::Acquire)
        )
    }

    pub(super) fn enabled(&self, platform: &WindowsPlatform, rule: &RuleDescriptor) -> bool {
        self.check_boundary(platform).is_ok()
            && PREPARATION_RULES.contains(&rule.id.as_str())
            && !versions_in(rule, &self.records).is_empty()
    }

    pub(super) fn blocked(
        &self,
        platform: &WindowsPlatform,
        rule: &RuleDescriptor,
    ) -> Result<Option<BlockReason>> {
        self.check_boundary(platform)?;
        let code = if !self.enabled(platform, rule) {
            Some(ReasonCode::ValidationPending)
        } else if platform.roots(rule)?.is_empty() {
            Some(ReasonCode::UnsupportedLayout)
        } else {
            *self.process.lock().unwrap()
        };
        Ok(code.map(|code| BlockReason::new(code, code.message())))
    }
}

struct Fixture {
    platform: Arc<WindowsPlatform>,
    state: Arc<SyntheticAcceptance>,
    files: Vec<PathBuf>,
    protected: Vec<PathBuf>,
    journal: PathBuf,
}

fn write(path: &Path, age: i64) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"synthetic-data-not-real-application-content").unwrap();
    filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(now() - age, 0)).unwrap();
}

fn descriptor(id: &str) -> RuleDescriptor {
    dustaway_core::rules::catalog()
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap()
}

fn fixture(id: &'static str) -> Fixture {
    assert!(PREPARATION_RULES.contains(&id));
    let directory = tempfile::Builder::new()
        .prefix("dustaway-acceptance-")
        .tempdir()
        .unwrap();
    let base = directory.path();
    write(&base.join(".dustaway-test-only"), 0);
    std::fs::write(base.join(".dustaway-test-only"), MARKER).unwrap();
    let spec = dustaway_core::applications::rule(id).unwrap();
    let folder = match spec.folder {
        ApplicationFolder::Local => "local",
        ApplicationFolder::Roaming => "roaming",
    };
    let profiles = match spec.profiles {
        ProfileLayout::DingTalkNumbered => vec![
            base.join(folder).join("DingTalk_123/Default"),
            base.join(folder).join("DingTalk_456/Default"),
        ],
        _ => vec![base.join(folder).join(spec.base)],
    };
    let mut files = vec![];
    let mut protected = vec![];
    for profile in profiles {
        if spec.profiles != ProfileLayout::Direct {
            let marker = profile.join("Preferences");
            write(&marker, 0);
            protected.push(marker);
        }
        let root = profile.join(spec.relative);
        for n in 0..3 {
            let path = root.join(format!(
                "candidate-{n}.{}",
                spec.extension.unwrap_or("cache")
            ));
            write(&path, (i64::from(spec.age) + 1) * 86400);
            files.push(path);
        }
        for name in [
            "Local Storage/data",
            "IndexedDB/data",
            "Network/Cookies",
            "Session Storage/data",
            "downloads/unfinished.xltd",
            "downloads/complete.zip",
            "offline/document.docx",
            "tasks.db",
            "chat/attachment.png",
        ] {
            let path = profile.join(name);
            write(&path, 60 * 86400);
            protected.push(path);
        }
        if id == "weixin.logs" {
            for name in [
                "recent.xlog",
                "buffer.mmap",
                "buffer.mmap3",
                "history.db",
                "wrong.xlog.bak",
            ] {
                let path = root.join(name);
                write(&path, 0);
                protected.push(path);
            }
            let future = root.join("future.xlog");
            write(&future, -86400);
            protected.push(future);
        }
    }
    let journal = base.join("history.sqlite");
    let local = extended(&base.join("local")).unwrap();
    let roaming = extended(&base.join("roaming")).unwrap();
    let system = extended(&base.join("system")).unwrap();
    let identity = PinnedPath::open(base, true, false).unwrap().stamp.identity;
    let state = Arc::new(SyntheticAcceptance {
        directory,
        identity,
        records: vec![CleanupAcceptance {
            rule_id: id,
            rule_version: 1,
            layout_id: spec.layout_id,
            application_version: VERSION,
            evidence_id: "synthetic-test-only-not-release-evidence",
        }],
        version: Mutex::new(VERSION.into()),
        process: Mutex::new(None),
        generation: std::sync::atomic::AtomicUsize::new(0),
    });
    let platform = Arc::new(WindowsPlatform {
        inventory: Default::default(),
        tool_roots: Default::default(),
        tool_probe: std::sync::Arc::new(crate::tool_discovery::UnavailableProbe),
        real_app_acceptance: None,
        scope: Scope::User,
        local,
        roaming,
        system,
        app_test_identity: None,
        app_test_acceptance: Some(state.clone()),
    });
    state.check_boundary(&platform).unwrap();
    Fixture {
        platform,
        state,
        files,
        protected,
        journal,
    }
}

fn wait(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < deadline, "synthetic task timed out");
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
        ..Selection::default()
    }
}

fn protection(paths: &[PathBuf]) -> Vec<(String, String)> {
    paths
        .iter()
        .map(|path| {
            let pin = PinnedPath::open(path, false, false).unwrap();
            (
                format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
                pin.stamp.modified.to_string(),
            )
        })
        .collect()
}

#[test]
fn six_rule_cleanup_receipts() {
    let mut receipts = vec![];
    for &id in PREPARATION_RULES {
        let fixture = fixture(id);
        let before = protection(&fixture.protected);
        let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        let session = scan(&engine, id);
        assert!(fixture.files.iter().all(|p| p.exists()));
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        rule_ids: vec![id.into()],
                        ..Selection::default()
                    }
                )
                .is_err()
        );
        let page = engine.candidates(&session, id, 0, 200).unwrap();
        assert_eq!(page.total as usize, fixture.files.len());
        let mut chosen = selection(id);
        chosen.excluded_ids.push(page.items[0].id.clone());
        let plan = engine.prepare(&session, chosen).unwrap();
        assert_eq!(plan.files, page.total - 1);
        assert!(engine.execute("forged-plan").is_err());
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert!(engine.execute(&plan.id).is_err());
        let report = engine.report().unwrap();
        assert_eq!(report.deleted, plan.files);
        assert_eq!(report.failed + report.skipped + report.unknown, 0);
        assert_eq!(fixture.files.iter().filter(|p| p.exists()).count(), 1);
        assert_eq!(protection(&fixture.protected), before);
        assert_eq!(
            engine
                .query_candidates(
                    &session,
                    id,
                    0,
                    200,
                    CandidateQuery {
                        view: "cleanup".into(),
                        status: Some("deleted".into()),
                        ..Default::default()
                    }
                )
                .unwrap()
                .total,
            plan.files
        );
        receipts.push(serde_json::json!({
            "rule_id": id, "rule_version": 1,
            "layout_id": dustaway_core::applications::rule(id).unwrap().layout_id,
            "application_version": VERSION, "candidate_count": page.total,
            "selected_count": plan.files, "deleted_count": report.deleted,
            "deleted_bytes": report.deleted_bytes, "retained_count": 1,
            "failed_count": 0, "unknown_count": 0,
            "protected_count": before.len(),
            "protection_before": before, "protection_after": protection(&fixture.protected),
            "single_use_plan": true, "result_status": report.status,
        }));
    }
    println!(
        "DUSTAWAY_SYNTHETIC_RECEIPTS={}",
        serde_json::json!(receipts)
    );
}

#[test]
fn rules_recheck_version_process_identity_marker_and_changed_files() {
    for &id in PREPARATION_RULES {
        for condition in [
            ReasonCode::ApplicationRunning,
            ReasonCode::ProcessUnknown,
            ReasonCode::UnsupportedVersion,
            ReasonCode::Changed,
        ] {
            let fixture = fixture(id);
            let engine =
                Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
            let session = scan(&engine, id);
            let plan = engine.prepare(&session, selection(id)).unwrap();
            match condition {
                ReasonCode::UnsupportedVersion => {
                    *fixture.state.version.lock().unwrap() = "0.0.0.2".into()
                }
                ReasonCode::Changed => {
                    for path in &fixture.files {
                        std::fs::write(path, b"changed after confirmation").unwrap();
                    }
                }
                other => *fixture.state.process.lock().unwrap() = Some(other),
            }
            engine.execute(&plan.id).unwrap();
            wait(&engine);
            let report = engine.report().unwrap();
            assert_eq!(report.deleted, 0, "{id} {condition:?}");
            assert_eq!(report.skipped, plan.files);
            assert!(report.categories[0].reasons.iter().any(|r| r.code
                == if condition == ReasonCode::UnsupportedVersion {
                    ReasonCode::Changed
                } else {
                    condition
                }));
            assert!(fixture.files.iter().all(|p| p.exists()));
        }
        let fixture = fixture(id);
        let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        let session = scan(&engine, id);
        *fixture.state.version.lock().unwrap() = "0.0.0.2".into();
        assert!(engine.prepare(&session, selection(id)).is_err());
        let updated_session = scan(&engine, id);
        assert!(engine.prepare(&updated_session, selection(id)).is_ok());
        *fixture.state.version.lock().unwrap() = VERSION.into();
        for path in fixture
            .protected
            .iter()
            .filter(|p| p.file_name().unwrap() == "Preferences")
        {
            std::fs::remove_file(path).unwrap();
        }
        if id != "weixin.logs" {
            assert!(engine.prepare(&session, selection(id)).is_err());
        }
        std::fs::remove_file(fixture.state.directory.path().join(".dustaway-test-only")).unwrap();
        assert!(!fixture.platform.cleanup_validated(&descriptor(id)));
    }
}

struct Paused {
    platform: Arc<WindowsPlatform>,
    after_first: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
}
impl Platform for Paused {
    fn cleanup_validated(&self, r: &RuleDescriptor) -> bool {
        self.platform.cleanup_validated(r)
    }
    fn scan(
        &self,
        r: &RuleDescriptor,
        e: &[String],
        c: &AtomicBool,
        emit: &mut dyn FnMut(dustaway_core::ScanEvent),
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
        self.platform.delete(r, f, e)?;
        if let Some((sent, resume)) = self.after_first.lock().unwrap().take() {
            sent.send(()).unwrap();
            resume.recv_timeout(Duration::from_secs(20)).unwrap();
        }
        Ok(())
    }
    fn free_space(&self) -> Result<u64> {
        self.platform.free_space()
    }
}

#[test]
fn each_rule_cancels_remaining_files_and_recovery_never_resumes_deletion() {
    for &id in PREPARATION_RULES {
        let fixture = fixture(id);
        let (sent, received) = mpsc::channel();
        let (resume, resumed) = mpsc::channel();
        let platform = Arc::new(Paused {
            platform: fixture.platform.clone(),
            after_first: Mutex::new(Some((sent, resumed))),
        });
        let engine = Engine::open(platform, Scope::User, &fixture.journal).unwrap();
        let session = scan(&engine, id);
        let plan = engine.prepare(&session, selection(id)).unwrap();
        engine.execute(&plan.id).unwrap();
        received.recv_timeout(Duration::from_secs(20)).unwrap();
        engine.cancel();
        resume.send(()).unwrap();
        wait(&engine);
        let report = engine.report().unwrap();
        assert_eq!(report.deleted, 1);
        assert_eq!(report.status, "cancelled");
        drop(engine);
        let restored =
            Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        assert!(!restored.is_busy());
        assert!(restored.execute(&plan.id).is_err());
        assert_eq!(
            fixture.files.iter().filter(|p| p.exists()).count(),
            fixture.files.len() - 1
        );
    }
}

#[test]
fn eligibility_never_spreads_and_long_term_exclusions_prevent_discovery() {
    let fixture = fixture("dingtalk.http");
    assert!(
        fixture
            .platform
            .cleanup_validated(&descriptor("dingtalk.http"))
    );
    assert!(
        !fixture
            .platform
            .cleanup_validated(&descriptor("dingtalk.code"))
    );
    let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
    engine
        .save_settings(Settings {
            theme: "system".into(),
            exclusions: fixture.files.iter().map(|p| display_path(p)).collect(),
            ..Default::default()
        })
        .unwrap();
    let session = scan(&engine, "dingtalk.http");
    assert_eq!(
        engine
            .candidates(&session, "dingtalk.http", 0, 200)
            .unwrap()
            .total,
        0
    );
    assert!(
        engine
            .prepare(&session, selection("dingtalk.http"))
            .is_err()
    );
}

#[test]
fn each_rule_rechecks_changes_between_two_deletions() {
    for &id in PREPARATION_RULES {
        for restart in [false, true] {
            let fixture = fixture(id);
            let before = protection(&fixture.protected);
            let (sent, received) = mpsc::channel();
            let (resume, resumed) = mpsc::channel();
            let platform = Arc::new(Paused {
                platform: fixture.platform.clone(),
                after_first: Mutex::new(Some((sent, resumed))),
            });
            let engine = Engine::open(platform, Scope::User, &fixture.journal).unwrap();
            let session = scan(&engine, id);
            let plan = engine.prepare(&session, selection(id)).unwrap();
            engine.execute(&plan.id).unwrap();
            received.recv_timeout(Duration::from_secs(20)).unwrap();
            if restart {
                *fixture.state.process.lock().unwrap() = Some(ReasonCode::ApplicationRunning);
            } else {
                for file in fixture.files.iter().filter(|p| p.exists()) {
                    std::fs::write(file, b"changed while cleaning").unwrap();
                }
            }
            resume.send(()).unwrap();
            wait(&engine);
            let report = engine.report().unwrap();
            assert_eq!(report.deleted, 1);
            assert_eq!(report.skipped, plan.files - 1);
            let expected = if restart {
                ReasonCode::ApplicationRunning
            } else {
                ReasonCode::Changed
            };
            assert!(
                report.categories[0]
                    .reasons
                    .iter()
                    .any(|r| r.code == expected)
            );
            assert_eq!(protection(&fixture.protected), before);
        }
    }
}

#[test]
fn weixin_age_boundary_uses_full_snapshot_validation() {
    let fixture = fixture("weixin.logs");
    let path = &fixture.files[0];
    let root = path.parent().unwrap();
    let cutoff = now() - 7 * 86400;
    filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(cutoff, 0)).unwrap();
    assert!(snapshot_at(root, path, cutoff, None).is_ok());
    filetime::set_file_mtime(path, filetime::FileTime::from_unix_time(cutoff + 1, 0)).unwrap();
    assert_eq!(
        error_reason(&snapshot_at(root, path, cutoff, None).unwrap_err()),
        ReasonCode::Recent
    );
}

#[test]
fn all_six_rules_reject_unsafe_files_and_same_version_identity_replacement() {
    use windows::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, SetFileAttributesW,
    };
    use windows::core::PCWSTR;
    for &id in PREPARATION_RULES {
        let fixture = fixture(id);
        let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        let session = scan(&engine, id);
        fixture
            .state
            .generation
            .store(1, std::sync::atomic::Ordering::Release);
        assert!(
            engine.prepare(&session, selection(id)).is_err(),
            "changed binary with same version: {id}"
        );
        fixture
            .state
            .generation
            .store(0, std::sync::atomic::Ordering::Release);
        let linked = &fixture.files[0];
        let external = fixture.state.directory.path().join("protected-hardlink");
        std::fs::hard_link(linked, &external).unwrap();
        let cloud = &fixture.files[1];
        unsafe {
            SetFileAttributesW(PCWSTR(wide(cloud).as_ptr()), FILE_ATTRIBUTE_OFFLINE).unwrap();
        }
        let new_session = scan(&engine, id);
        let page = engine.candidates(&new_session, id, 0, 200).unwrap();
        assert_eq!(page.total as usize, fixture.files.len() - 2);
        let plan = engine.prepare(&new_session, selection(id)).unwrap();
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert!(linked.exists() && external.exists() && cloud.exists());
        unsafe {
            SetFileAttributesW(PCWSTR(wide(cloud).as_ptr()), FILE_ATTRIBUTE_NORMAL).unwrap();
        }
    }
}

#[test]
fn each_rule_preserves_excluded_new_and_busy_files_outside_the_frozen_plan() {
    use std::os::windows::fs::OpenOptionsExt;
    for &id in PREPARATION_RULES {
        let fixture = fixture(id);
        let before = protection(&fixture.protected);
        let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        engine
            .save_settings(Settings {
                theme: "system".into(),
                exclusions: vec![display_path(&fixture.files[0])],
                ..Default::default()
            })
            .unwrap();
        let session = scan(&engine, id);
        let plan = engine.prepare(&session, selection(id)).unwrap();
        assert_eq!(plan.files as usize, fixture.files.len() - 1, "{id}");
        let new_file = fixture.files[0].parent().unwrap().join(format!(
            "after-plan.{}",
            dustaway_core::applications::rule(id)
                .unwrap()
                .extension
                .unwrap_or("cache")
        ));
        write(&new_file, 60 * 86400);
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&fixture.files[1])
            .unwrap();
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        let report = engine.report().unwrap();
        assert_eq!(report.deleted, plan.files - 1, "{id}");
        assert_eq!(report.skipped, 1, "{id}");
        drop(lock);
        assert!(fixture.files[0].exists() && fixture.files[1].exists() && new_file.exists());
        assert_eq!(protection(&fixture.protected), before, "{id}");
    }
}

#[test]
fn five_cache_rules_refuse_replaced_roots_and_junctions_after_confirmation() {
    use std::os::windows::process::CommandExt;
    for id in PREPARATION_RULES
        .iter()
        .copied()
        .filter(|id| *id != "weixin.logs")
    {
        for junction in [false, true] {
            let fixture = fixture(id);
            let before = protection(&fixture.protected);
            let engine =
                Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
            let session = scan(&engine, id);
            let plan = engine.prepare(&session, selection(id)).unwrap();
            let roots = fixture.platform.roots(&descriptor(id)).unwrap();
            let mut originals = vec![];
            for (index, root) in roots.iter().enumerate() {
                let saved = fixture
                    .state
                    .directory
                    .path()
                    .join(format!("saved-cache-{index}"));
                assert!(
                    root.starts_with(&fixture.platform.local)
                        || root.starts_with(&fixture.platform.roaming)
                );
                std::fs::rename(root, &saved).unwrap();
                originals.push(saved.clone());
                if junction {
                    let output = std::process::Command::new("cmd.exe")
                        .args(["/d", "/c", "mklink", "/J"])
                        .arg(display_path(root))
                        .arg(&saved)
                        .creation_flags(0x08000000)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "{id}: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                } else {
                    std::fs::create_dir(root).unwrap();
                }
            }
            engine.execute(&plan.id).unwrap();
            wait(&engine);
            assert_eq!(
                engine.report().unwrap().deleted,
                0,
                "{id} junction={junction}"
            );
            assert_eq!(
                originals
                    .iter()
                    .map(|root| std::fs::read_dir(root).unwrap().count())
                    .sum::<usize>(),
                fixture.files.len()
            );
            assert_eq!(protection(&fixture.protected), before);
            drop(engine);
            for root in roots {
                // Empty directory or fixture junction only; never recursively remove its target.
                std::fs::remove_dir(root).unwrap();
            }
        }
    }
}

#[test]
fn each_rule_reports_partial_reads_and_never_cleans_unreadable_subtrees() {
    use super::scan_tests::Denied;
    for &id in PREPARATION_RULES {
        let fixture = fixture(id);
        let protected = fixture.files[0]
            .parent()
            .unwrap()
            .join("denied/protected.cache");
        write(&protected, 60 * 86400);
        let before = protection(std::slice::from_ref(&protected));
        let denied = Denied::new(protected.parent().unwrap());
        let engine = Engine::open(fixture.platform.clone(), Scope::User, &fixture.journal).unwrap();
        let session = scan(&engine, id);
        let summary = engine.summary();
        assert_eq!(
            summary.rules[0].phase,
            dustaway_core::ScanPhase::Partial,
            "{id}"
        );
        assert!(!summary.rules[0].coverage.as_ref().unwrap().complete);
        let plan = engine.prepare(&session, selection(id)).unwrap();
        assert_eq!(plan.files as usize, fixture.files.len());
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert_eq!(engine.report().unwrap().deleted, plan.files);
        drop(denied);
        assert_eq!(protection(std::slice::from_ref(&protected)), before);
        let roots = fixture.platform.roots(&descriptor(id)).unwrap();
        let denied_roots: Vec<_> = roots.iter().map(|root| Denied::new(root)).collect();
        let session = scan(&engine, id);
        assert_eq!(engine.summary().rules[0].files, 0);
        assert!(engine.prepare(&session, selection(id)).is_err());
        drop(denied_roots);
    }
}
