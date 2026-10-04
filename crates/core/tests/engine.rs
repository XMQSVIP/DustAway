use anyhow::Result;
use dustaway_core::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Default)]
struct Fake {
    files: Mutex<Vec<FileRecord>>,
    deleted: Mutex<Vec<String>>,
    blocked: AtomicBool,
    block_reason: Mutex<Option<ReasonCode>>,
    changed_identity: AtomicBool,
    slow: AtomicBool,
    fail_delete: AtomicBool,
    fail_scan: AtomicBool,
    recent: AtomicBool,
    panic_scan: AtomicBool,
    cleanup_gate: Mutex<Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>>,
    process_error: AtomicBool,
    partial: AtomicBool,
    scan_gate: Mutex<Option<(Arc<std::sync::Barrier>, Arc<std::sync::Barrier>)>>,
}

#[test]
fn application_block_reasons_survive_refresh_and_do_not_authorize_deletion() {
    for code in [
        ReasonCode::ApplicationRunning,
        ReasonCode::ApplicationUnverified,
        ReasonCode::UnsupportedVersion,
        ReasonCode::ValidationPending,
        ReasonCode::ProcessUnknown,
    ] {
        let (_temp, fake, engine) = fixture(2);
        *fake.block_reason.lock().unwrap() = Some(code);
        let id = engine.start_scan(vec!["weixin.logs".into()]).unwrap();
        wait(&engine);
        let summary = engine.refresh_availability(&id).unwrap();
        assert_eq!(summary.rules[0].availability, Some(code));
        assert!(
            engine
                .prepare(
                    &id,
                    Selection {
                        rule_ids: vec!["weixin.logs".into()],
                        excluded_ids: vec![],
                        ..Selection::default()
                    }
                )
                .is_err()
        );
        assert!(fake.deleted.lock().unwrap().is_empty());
    }
}

#[test]
fn application_identity_change_invalidates_preparation() {
    let (_temp, fake, engine) = fixture(1);
    let id = engine.start_scan(vec!["weixin.logs".into()]).unwrap();
    wait(&engine);
    fake.changed_identity.store(true, Ordering::Release);
    assert!(
        engine
            .prepare(
                &id,
                Selection {
                    rule_ids: vec!["weixin.logs".into()],
                    excluded_ids: vec![],
                    ..Selection::default()
                }
            )
            .is_err()
    );
    assert!(fake.deleted.lock().unwrap().is_empty());
}

#[test]
fn process_detail_survives_scan_and_refresh() {
    let (_temp, fake, engine) = fixture(1);
    fake.blocked.store(true, Ordering::Release);
    let id = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    wait(&engine);
    assert_eq!(
        engine.summary().rules[0].note.as_deref(),
        Some("fixture running")
    );
    assert_eq!(
        engine.refresh_availability(&id).unwrap().rules[0]
            .note
            .as_deref(),
        Some("fixture running")
    );
}

#[test]
fn application_restart_after_plan_is_reported_as_a_skip() {
    let (_temp, fake, engine) = fixture(2);
    let id = engine.start_scan(vec!["qq.http".into()]).unwrap();
    wait(&engine);
    let plan = engine
        .prepare(
            &id,
            Selection {
                rule_ids: vec!["qq.http".into()],
                reviewed_rule_ids: vec!["qq.http".into()],
                excluded_ids: vec![],
                ..Selection::default()
            },
        )
        .unwrap();
    *fake.block_reason.lock().unwrap() = Some(ReasonCode::ApplicationRunning);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert!(fake.deleted.lock().unwrap().is_empty());
    let report = engine.report().unwrap();
    assert_eq!(report.skipped, 2);
    assert!(
        report.categories[0]
            .reasons
            .iter()
            .any(|r| r.code == ReasonCode::ApplicationRunning)
    );
}
impl Platform for Fake {
    // This in-memory platform models an accepted application for engine state-machine tests.
    // Production acceptance is tested separately against the ordinary Windows library.
    fn cleanup_validated(&self, rule: &RuleDescriptor) -> bool {
        rule.cleanup_validated
    }
    fn validate_candidate(&self, _: &RuleDescriptor, _: &FileRecord) -> Result<()> {
        anyhow::ensure!(
            !self.changed_identity.load(Ordering::Acquire),
            Refusal(ReasonCode::Changed)
        );
        Ok(())
    }
    fn file_volume(&self, _: &FileRecord) -> Result<VolumeTarget> {
        Ok(VolumeTarget {
            key: "fixture".into(),
            label: "T:".into(),
            opaque: String::new(),
        })
    }
    fn volume_available(&self, _: &VolumeTarget) -> Result<u64> {
        self.free_space()
    }
    fn candidate_action(
        &self,
        _: &RuleDescriptor,
        file: &FileRecord,
        kind: ExclusionKind,
        _: bool,
    ) -> Result<String> {
        Ok(match kind {
            ExclusionKind::File => file.path_display.clone(),
            ExclusionKind::Parent => "fixture".into(),
        })
    }
    fn scan(
        &self,
        _: &RuleDescriptor,
        _: &[String],
        cancel: &AtomicBool,
        emit: &mut dyn FnMut(ScanEvent),
    ) -> Result<()> {
        assert!(
            !self.panic_scan.load(Ordering::Acquire),
            "fixture scan panic"
        );
        if self.fail_scan.load(Ordering::Acquire) {
            return Err(Refusal(ReasonCode::DirectoryInaccessible).into());
        }
        if self.partial.load(Ordering::Acquire) {
            emit(ScanEvent::CoverageIssue(ReasonCode::AccessDenied));
        }
        if let Some((entered, release)) = self.scan_gate.lock().unwrap().take() {
            entered.wait();
            release.wait();
        }
        for file in self.files.lock().unwrap().clone() {
            if cancel.load(Ordering::Acquire) {
                break;
            }
            if self.slow.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
            }
            emit(ScanEvent::Checked);
            if self.recent.load(Ordering::Acquire) {
                emit(ScanEvent::Retained(ReasonCode::Recent));
            } else {
                emit(ScanEvent::File(file));
            }
        }
        Ok(())
    }
    fn blocked(&self, _: &RuleDescriptor) -> Result<Option<BlockReason>> {
        if let Some(code) = *self.block_reason.lock().unwrap() {
            return Ok(Some(BlockReason::new(code, code.message())));
        }
        if self.process_error.load(Ordering::Acquire) {
            anyhow::bail!("fixture process query failed");
        }
        Ok(self
            .blocked
            .load(Ordering::Acquire)
            .then(|| BlockReason::new(ReasonCode::BrowserRunning, "fixture running")))
    }
    fn delete(&self, _: &RuleDescriptor, file: &FileRecord, _: &[String]) -> Result<()> {
        if self.slow.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.fail_delete.load(Ordering::Acquire) {
            return Err(DeleteFailure("fixture I/O error".into()).into());
        }
        self.deleted.lock().unwrap().push(file.path_display.clone());
        Ok(())
    }
    fn free_space(&self) -> Result<u64> {
        if let Some((entered, release)) = self.cleanup_gate.lock().unwrap().take() {
            entered.wait();
            release.wait();
        }
        Ok(1024)
    }
}

#[test]
fn deletion_failure_is_reported_separately_from_safety_skip() {
    let (_temp, fake, engine) = fixture(1);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    fake.fail_delete.store(true, Ordering::Release);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let report = engine.report().unwrap();
    assert_eq!((report.deleted, report.skipped, report.failed), (0, 0, 1));
    assert_eq!(
        engine
            .candidates(&session, "windows.user_temp", 0, 10)
            .unwrap()
            .items[0]
            .status,
        "failed"
    );
}

#[test]
fn partial_coverage_survives_refresh_plan_report_history_and_redaction() {
    let (_temp, fake, engine) = fixture(2);
    fake.partial.store(true, Ordering::Release);
    let session = scan(&engine);
    let summary = engine.summary();
    assert_eq!(summary.rules[0].phase, ScanPhase::Partial);
    assert!(!summary.rules[0].blocked);
    let refreshed = engine.refresh_availability(&session).unwrap();
    let coverage = refreshed.rules[0].coverage.as_ref().unwrap();
    assert!(!coverage.complete);
    assert_eq!(coverage.issues[0].count, 1);
    assert_eq!(coverage.issues[0].code, ReasonCode::AccessDenied);
    let plan = engine.prepare(&session, select()).unwrap();
    assert_eq!(plan.files, 2);
    assert!(!plan.categories[0].coverage.as_ref().unwrap().complete);
    assert!(engine.execute("forged-plan").is_err());
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let history = engine.history().unwrap();
    assert!(!history[0].categories[0].coverage.as_ref().unwrap().complete);
    let json = serde_json::to_string(&history).unwrap();
    for file in fake.files.lock().unwrap().iter() {
        assert!(!json.contains(&file.path_display));
    }
    assert_eq!(history[0].deleted, 2);
}

#[test]
fn cancellation_waits_for_blocked_operation_and_cannot_prepare_partial_results() {
    let (_temp, fake, engine) = fixture(2);
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    *fake.scan_gate.lock().unwrap() = Some((entered.clone(), release.clone()));
    let session = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    entered.wait();
    engine.cancel();
    assert!(engine.is_busy());
    assert_eq!(engine.status().state, JobState::Scanning);
    assert!(engine.prepare(&session, select()).is_err());
    release.wait();
    wait(&engine);
    assert_eq!(engine.status().state, JobState::Cancelled);
    assert!(
        !engine.summary().rules[0]
            .coverage
            .as_ref()
            .unwrap()
            .complete
    );
    assert!(engine.prepare(&session, select()).is_err());
}

#[test]
fn empty_complete_partial_and_failed_scans_are_distinct_and_legacy_coverage_is_unknown() {
    let (_temp, fake, engine) = fixture(0);
    scan(&engine);
    assert!(
        engine.summary().rules[0]
            .coverage
            .as_ref()
            .unwrap()
            .complete
    );
    fake.partial.store(true, Ordering::Release);
    scan(&engine);
    assert_eq!(engine.summary().rules[0].phase, ScanPhase::Partial);
    fake.fail_scan.store(true, Ordering::Release);
    let session = scan(&engine);
    assert_eq!(engine.summary().rules[0].phase, ScanPhase::Failed);
    assert!(engine.prepare(&session, select()).is_err());
    let mut old = serde_json::to_value(&engine.summary().rules[0]).unwrap();
    old.as_object_mut().unwrap().remove("coverage");
    assert!(
        serde_json::from_value::<RuleSummary>(old)
            .unwrap()
            .coverage
            .is_none()
    );
    let old: RuleOutcome = serde_json::from_value(serde_json::json!({"rule_id":"old","total":1,"deleted":1,"skipped":0,"failed":0,"unknown":0,"deleted_bytes":"1","reasons":[],"retained":[]})).unwrap();
    assert!(old.coverage.is_none());
}

#[test]
fn cancelling_cleanup_keeps_completed_results_and_does_not_resume() {
    let (_temp, fake, engine) = fixture(100);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    fake.slow.store(true, Ordering::Release);
    engine.execute(&plan.id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while fake.deleted.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    engine.cancel();
    wait(&engine);
    let report = engine.report().unwrap();
    assert_eq!(report.status, "cancelled");
    assert!(report.deleted > 0 && report.deleted < 100);
    assert_eq!(report.deleted as usize, fake.deleted.lock().unwrap().len());
    assert!(engine.execute(&plan.id).is_err());
}
fn fixture(count: usize) -> (tempfile::TempDir, Arc<Fake>, Arc<Engine>) {
    let temp = tempfile::tempdir().unwrap();
    let fake = Arc::new(Fake::default());
    *fake.files.lock().unwrap() = (0..count)
        .map(|i| FileRecord {
            path_display: format!("fixture/{i}"),
            bytes: 10,
            modified_unix: 0,
            opaque: "fixture-only".into(),
        })
        .collect();
    let engine = Engine::open(
        fake.clone(),
        Scope::User,
        &temp.path().join("history.sqlite"),
    )
    .unwrap();
    (temp, fake, engine)
}
fn wait(engine: &Engine) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < deadline, "job timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn scan(engine: &Arc<Engine>) -> String {
    let id = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    wait(engine);
    assert_eq!(engine.status().state, JobState::Ready);
    id
}
fn select() -> Selection {
    Selection {
        rule_ids: vec!["windows.user_temp".into()],
        excluded_ids: vec![],
        ..Selection::default()
    }
}

#[test]
fn scan_is_read_only_and_plan_is_single_use() {
    let (_temp, fake, engine) = fixture(4);
    let session = scan(&engine);
    assert!(fake.deleted.lock().unwrap().is_empty());
    let plan = engine.prepare(&session, select()).unwrap();
    assert_eq!(plan.files, 4);
    assert_eq!(plan.bytes, "40");
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert!(engine.execute(&plan.id).is_err());
    assert_eq!(fake.deleted.lock().unwrap().len(), 4);
    assert_eq!(engine.report().unwrap().deleted, 4);
}
#[test]
fn excluded_ids_are_not_deleted() {
    let (_temp, fake, engine) = fixture(3);
    let session = scan(&engine);
    let page = engine
        .candidates(&session, "windows.user_temp", 0, 200)
        .unwrap();
    let mut selection = select();
    selection.excluded_ids.push(page.items[0].id.clone());
    let plan = engine.prepare(&session, selection).unwrap();
    assert_eq!(plan.files, 2);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert!(!fake.deleted.lock().unwrap().contains(&page.items[0].path));
}

#[test]
fn existing_fourteen_conditional_rules_keep_selection_and_single_use_plans() {
    let rules: Vec<_> = rules::for_scope(Scope::User)
        .unwrap()
        .into_iter()
        .filter(|r| {
            r.cleanup_validated
                && r.app_id.is_none()
                && r.root != "developer"
                && r.root != "application_cache"
                && r.id != "windows.crash_dumps"
        })
        .collect();
    assert_eq!(rules.len(), 14);
    for rule in rules {
        let (_temp, fake, engine) = fixture(2);
        let session = engine.start_scan(vec![rule.id.clone()]).unwrap();
        wait(&engine);
        let files = engine.candidates(&session, &rule.id, 0, 200).unwrap();
        assert_eq!(files.items.len(), 2, "{}", rule.id);
        let plan = engine
            .prepare(
                &session,
                Selection {
                    rule_ids: vec![rule.id.clone()],
                    excluded_ids: vec![files.items[0].id.clone()],
                    ..Selection::default()
                },
            )
            .unwrap();
        assert_eq!(plan.files, 1, "{}", rule.id);
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert!(engine.execute(&plan.id).is_err());
        assert_eq!(
            *fake.deleted.lock().unwrap(),
            vec![files.items[1].path.clone()]
        );
    }
}

#[test]
fn crash_dump_selection_remains_readonly_until_real_acceptance() {
    let (_temp, fake, engine) = fixture(2);
    let id = "windows.crash_dumps";
    let session = engine.start_scan(vec![id.into()]).unwrap();
    wait(&engine);
    let files = engine.candidates(&session, id, 0, 200).unwrap();
    let mut selection = Selection {
        included_ids: vec![files.items[0].id.clone()],
        ..Default::default()
    };
    assert!(engine.prepare(&session, selection.clone()).is_err());
    selection.reviewed_rule_ids.push(id.into());
    assert!(engine.prepare(&session, selection).is_err());
    assert!(fake.deleted.lock().unwrap().is_empty());
}

#[test]
fn developer_rules_require_explicit_review_and_delete_only_selected_files_once() {
    for spec in developer::RULES.iter().filter(|s| s.cleanable) {
        let (_temp, fake, engine) = fixture(2);
        let session = engine.start_scan(vec![spec.id.into()]).unwrap();
        wait(&engine);
        let files = engine.candidates(&session, spec.id, 0, 200).unwrap();
        let mut selection = Selection {
            included_ids: vec![files.items[0].id.clone()],
            ..Default::default()
        };
        assert!(engine.prepare(&session, selection.clone()).is_err());
        selection.reviewed_rule_ids.push(spec.id.into());
        let plan = engine.prepare(&session, selection).unwrap();
        assert_eq!(plan.files, 1);
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert!(engine.execute(&plan.id).is_err());
        assert_eq!(
            *fake.deleted.lock().unwrap(),
            vec![files.items[0].path.clone()]
        );
    }
}

#[test]
fn managed_developer_caches_remain_readonly_even_after_review_and_refresh() {
    for spec in developer::RULES.iter().filter(|s| !s.cleanable) {
        let (_temp, fake, engine) = fixture(1);
        let session = engine.start_scan(vec![spec.id.into()]).unwrap();
        wait(&engine);
        let files = engine.candidates(&session, spec.id, 0, 1).unwrap();
        engine.refresh_availability(&session).unwrap();
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        included_ids: vec![files.items[0].id.clone()],
                        reviewed_rule_ids: vec![spec.id.into()],
                        ..Default::default()
                    }
                )
                .is_err(),
            "{}",
            spec.id
        );
        assert!(fake.deleted.lock().unwrap().is_empty());
    }
}

#[test]
fn explicit_files_do_not_select_the_rest_of_the_category() {
    let (_temp, fake, engine) = fixture(3);
    let session = scan(&engine);
    let files = engine
        .candidates(&session, "windows.user_temp", 0, 200)
        .unwrap();
    let plan = engine
        .prepare(
            &session,
            Selection {
                included_ids: vec![files.items[1].id.clone(), files.items[1].id.clone()],
                ..Selection::default()
            },
        )
        .unwrap();
    assert_eq!(plan.files, 1);
    assert_eq!(plan.bytes, "10");
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(
        *fake.deleted.lock().unwrap(),
        vec![files.items[1].path.clone()]
    );
}

#[test]
fn review_selection_requires_acknowledgement_and_rejects_old_or_foreign_files() {
    let (_temp, _, engine) = fixture(3);
    let session = engine.start_scan(vec!["qq.http".into()]).unwrap();
    wait(&engine);
    let files = engine.candidates(&session, "qq.http", 0, 200).unwrap();
    let mut selection = Selection {
        included_ids: vec![files.items[0].id.clone()],
        ..Selection::default()
    };
    assert!(engine.prepare(&session, selection.clone()).is_err());
    selection.reviewed_rule_ids.push("qq.http".into());
    let plan = engine.prepare(&session, selection.clone()).unwrap();
    assert_eq!(plan.files, 1);
    selection.included_ids.push("not-in-this-session".into());
    assert!(engine.prepare(&session, selection).is_err());
    assert!(engine.execute(&plan.id).is_err());
    let _next = engine.start_scan(vec!["qq.http".into()]).unwrap();
    wait(&engine);
    assert!(
        engine
            .prepare(
                &session,
                Selection {
                    included_ids: vec![files.items[0].id.clone()],
                    reviewed_rule_ids: vec!["qq.http".into()],
                    ..Selection::default()
                }
            )
            .is_err()
    );
}

#[test]
fn empty_selection_invalidates_plan_and_fully_excluded_rules_leave_no_plan_category() {
    let (_temp, _, engine) = fixture(3);
    let session = engine
        .start_scan(vec!["windows.user_temp".into(), "qq.http".into()])
        .unwrap();
    wait(&engine);
    let excluded = engine.candidates(&session, "qq.http", 0, 200).unwrap();
    assert!(!excluded.items.is_empty());
    let plan = engine
        .prepare(
            &session,
            Selection {
                rule_ids: vec!["windows.user_temp".into(), "qq.http".into()],
                reviewed_rule_ids: vec!["qq.http".into()],
                excluded_ids: excluded.items.into_iter().map(|f| f.id).collect(),
                ..Selection::default()
            },
        )
        .unwrap();
    assert_eq!(plan.rule_ids, vec!["windows.user_temp"]);
    assert_eq!(plan.categories.len(), 1);
    assert_eq!(plan.impacts.len(), 1);
    assert!(engine.prepare(&session, Selection::default()).is_err());
    assert!(engine.execute(&plan.id).is_err());
}

#[test]
fn unvalidated_rules_cannot_be_unlocked_by_selection_or_explicit_files() {
    let (_temp, fake, engine) = fixture(1);
    for spec in rules::catalog()
        .unwrap()
        .iter()
        .filter(|r| !r.cleanup_validated)
    {
        let session = engine.start_scan(vec![spec.id.clone()]).unwrap();
        wait(&engine);
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        rule_ids: vec![spec.id.clone()],
                        reviewed_rule_ids: vec![spec.id.clone()],
                        ..Selection::default()
                    }
                )
                .is_err()
        );
        let candidate = engine
            .candidates(&session, &spec.id, 0, 200)
            .unwrap()
            .items
            .remove(0);
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        included_ids: vec![candidate.id],
                        reviewed_rule_ids: vec![spec.id.clone()],
                        ..Selection::default()
                    }
                )
                .is_err()
        );
    }
    assert!(fake.deleted.lock().unwrap().is_empty());
}
#[test]
fn old_session_and_unknown_ids_are_rejected() {
    let (_temp, _, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    scan(&engine);
    assert!(engine.execute(&plan.id).is_err());
    assert!(engine.prepare(&session, select()).is_err());
    let mut selection = select();
    selection.excluded_ids.push("fabricated".into());
    assert!(engine.prepare(&engine.status().id, selection).is_err());
    assert!(engine.start_scan(vec!["arbitrary.path".into()]).is_err());
}
#[test]
fn changing_cleanup_exclusions_invalidates_scan() {
    let (_temp, _, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    engine
        .save_settings(Settings {
            theme: "dark".into(),
            exclusions: vec!["C:/fixture/keep".into()],
            ..Default::default()
        })
        .unwrap();
    assert!(engine.execute(&plan.id).is_err());
    assert!(
        engine
            .candidates(&session, "windows.user_temp", 0, 20)
            .is_err()
    );
}
#[test]
fn browser_restarting_after_confirmation_is_skipped() {
    let (_temp, fake, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    fake.blocked.store(true, Ordering::Release);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(engine.report().unwrap().skipped, 2);
    assert!(fake.deleted.lock().unwrap().is_empty());
}
#[test]
fn files_created_after_scan_are_not_added_to_plan() {
    let (_temp, fake, engine) = fixture(1);
    let session = scan(&engine);
    fake.files.lock().unwrap().push(FileRecord {
        path_display: "new".into(),
        bytes: 100,
        modified_unix: 0,
        opaque: "new".into(),
    });
    let plan = engine.prepare(&session, select()).unwrap();
    assert_eq!(plan.files, 1);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(fake.deleted.lock().unwrap().as_slice(), ["fixture/0"]);
}
#[test]
fn cancelled_scan_cannot_be_cleaned() {
    let (_temp, fake, engine) = fixture(1000);
    fake.slow.store(true, Ordering::Release);
    let id = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    engine.cancel();
    wait(&engine);
    assert_eq!(engine.status().state, JobState::Cancelled);
    assert!(engine.prepare(&id, select()).is_err());
}
#[test]
fn summary_persists_but_candidates_do_not() {
    let (temp, fake, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    drop(engine);
    let reopened = Engine::open(fake, Scope::User, &temp.path().join("history.sqlite")).unwrap();
    assert_eq!(reopened.history().unwrap().len(), 1);
    assert!(
        reopened
            .candidates(&session, "windows.user_temp", 0, 200)
            .is_err()
    );
    assert_eq!(reopened.status().state, JobState::Idle);
}
#[test]
fn interrupted_intent_is_never_replayed() {
    let (temp, fake, engine) = fixture(0);
    drop(engine);
    let db = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    let report = CleanupReport {
        manual: None,
        directories: Default::default(),
        volume_space_changes: vec![],
        id: "crash".into(),
        started_at: now(),
        finished_at: None,
        status: "running".into(),
        total: 1,
        deleted: 0,
        skipped: 0,
        failed: 0,
        unknown: 0,
        candidate_bytes: "10".into(),
        deleted_bytes: "0".into(),
        free_space_change: None,
        scope: Scope::User,
        categories: vec![RuleOutcome {
            rule_id: "windows.user_temp".into(),
            total: 1,
            deleted_bytes: "0".into(),
            ..Default::default()
        }],
    };
    db.execute(
        "INSERT INTO reports VALUES('crash',?1,?2)",
        rusqlite::params![now(), serde_json::to_string(&report).unwrap()],
    )
    .unwrap();
    db.execute("INSERT INTO intents(job,item,outcome,rule) VALUES('crash','file','pending','windows.user_temp')", [])
        .unwrap();
    drop(db);
    let reopened = Engine::open(
        fake.clone(),
        Scope::User,
        &temp.path().join("history.sqlite"),
    )
    .unwrap();
    let report = &reopened.history().unwrap()[0];
    assert_eq!(report.status, "interrupted");
    assert_eq!(report.unknown, 1);
    assert_eq!(report.categories[0].unknown, 1);
    assert!(fake.deleted.lock().unwrap().is_empty());
}
#[test]
fn hundred_thousand_candidates_are_paged() {
    let (_temp, _, engine) = fixture(100_000);
    let session = scan(&engine);
    let page = engine
        .candidates(&session, "windows.user_temp", 99_900, 1000)
        .unwrap();
    assert_eq!(page.total, 100_000);
    assert_eq!(page.items.len(), 100);
    assert_eq!(engine.summary().rules[0].bytes, "1000000");
    let filtered = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                search: "fixture/999".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(filtered.total, 111);
    assert_eq!(filtered.items.len(), 111);
}

#[test]
fn theme_preserves_plan_but_exclusions_invalidate_it() {
    let (_temp, _, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    let mut settings = engine.settings();
    settings.theme = "dark".into();
    assert!(
        !engine
            .save_settings(settings.clone())
            .unwrap()
            .scan_invalidated
    );
    engine.validate_plan(&plan.id).unwrap();
    assert_eq!(engine.status().id, session);
    settings.excluded_rules.push("windows.system_temp".into());
    assert!(engine.save_settings(settings).unwrap().scan_invalidated);
    assert!(engine.validate_plan(&plan.id).is_err());
}

#[test]
fn theme_can_change_during_scan_without_resetting_worker() {
    let (_temp, fake, engine) = fixture(100);
    fake.slow.store(true, Ordering::Release);
    let session = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    let mut settings = engine.settings();
    settings.theme = "dark".into();
    assert!(
        !engine
            .save_settings(settings.clone())
            .unwrap()
            .scan_invalidated
    );
    settings.excluded_rules.push("windows.user_temp".into());
    assert!(engine.save_settings(settings).is_err());
    wait(&engine);
    assert_eq!(engine.status().id, session);
}

#[test]
fn availability_refresh_preserves_files_and_never_unblocks_scan_failure() {
    let (_temp, fake, engine) = fixture(3);
    fake.blocked.store(true, Ordering::Release);
    let session = scan(&engine);
    assert!(engine.summary().rules[0].blocked);
    fake.files.lock().unwrap().push(FileRecord {
        path_display: "new-after-scan".into(),
        bytes: 50,
        modified_unix: 0,
        opaque: String::new(),
    });
    fake.blocked.store(false, Ordering::Release);
    assert!(!engine.refresh_availability(&session).unwrap().rules[0].blocked);
    assert_eq!(engine.prepare(&session, select()).unwrap().files, 3);
    fake.fail_scan.store(true, Ordering::Release);
    let failed = scan(&engine);
    assert!(engine.refresh_availability(&failed).unwrap().rules[0].scan_failed);
    assert!(engine.prepare(&failed, select()).is_err());
}

#[test]
fn unknown_process_state_and_browser_restart_are_safe_skips() {
    let (_temp, fake, engine) = fixture(2);
    let session = scan(&engine);
    fake.process_error.store(true, Ordering::Release);
    assert_eq!(
        engine.refresh_availability(&session).unwrap().rules[0].availability,
        Some(ReasonCode::ProcessUnknown)
    );
    fake.process_error.store(false, Ordering::Release);
    engine.refresh_availability(&session).unwrap();
    let plan = engine.prepare(&session, select()).unwrap();
    fake.blocked.store(true, Ordering::Release);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let r = engine.report().unwrap();
    assert_eq!((r.deleted, r.skipped, r.categories[0].skipped), (0, 2, 2));
    assert_eq!(r.categories[0].reasons[0].code, ReasonCode::BrowserRunning);
    assert!(fake.deleted.lock().unwrap().is_empty());
}

#[test]
fn query_is_literal_stable_and_sorts_full_u64_without_precision_loss() {
    let (_temp, fake, engine) = fixture(0);
    *fake.files.lock().unwrap() = vec![
        FileRecord {
            path_display: "C:/中文/a%_file".into(),
            bytes: 9007199254740993,
            modified_unix: 1,
            opaque: String::new(),
        },
        FileRecord {
            path_display: "C:/中文/AOTHER".into(),
            bytes: 9007199254740992,
            modified_unix: 2,
            opaque: String::new(),
        },
        FileRecord {
            path_display: "C:/中文/b".into(),
            bytes: 0,
            modified_unix: 3,
            opaque: String::new(),
        },
    ];
    let session = scan(&engine);
    let first = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            1,
            CandidateQuery::default(),
        )
        .unwrap();
    assert_eq!(first.items[0].bytes, "9007199254740993");
    let literal = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                search: "%_".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(literal.total, 1);
    let case = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                search: "aother".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(case.total, 1);
    assert!(
        engine
            .query_candidates(
                &session,
                "windows.user_temp",
                0,
                200,
                CandidateQuery {
                    sort: "size; DROP TABLE candidates".into(),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        engine
            .query_candidates(
                &session,
                "windows.user_temp",
                0,
                200,
                CandidateQuery {
                    search: "字".repeat(257),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        engine
            .query_candidates(&session, "unscanned", 0, 200, CandidateQuery::default())
            .is_err()
    );
}

#[test]
fn cleanup_view_only_contains_plan_and_counts_match() {
    let (_temp, _, engine) = fixture(3);
    let session = scan(&engine);
    let candidates = engine
        .candidates(&session, "windows.user_temp", 0, 200)
        .unwrap();
    let mut selection = select();
    selection.excluded_ids.push(candidates.items[0].id.clone());
    let plan = engine.prepare(&session, selection).unwrap();
    assert_eq!(plan.categories[0].files, 2);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let done = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                view: "cleanup".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(done.total, 2);
    let remaining = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                view: "cleanup".into(),
                status: Some("candidate".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(remaining.total, 0);
    let r = engine.report().unwrap();
    assert_eq!(r.categories[0].deleted, r.deleted);
    assert_eq!(r.categories[0].deleted_bytes, r.deleted_bytes);
    assert!(!serde_json::to_string(&r).unwrap().contains("fixture/"));
}

#[test]
fn old_history_reads_and_retention_never_removes_running_report() {
    let (temp, _, engine) = fixture(0);
    let db = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    let old = serde_json::json!({"id":"old","started_at":now(),"finished_at":now(),"status":"finished","total":0,"deleted":0,"skipped":0,"failed":0,"unknown":0,"candidate_bytes":"0","deleted_bytes":"0","free_space_change":null,"scope":"system"});
    db.execute(
        "INSERT INTO reports VALUES('old',?1,?2)",
        rusqlite::params![now(), old.to_string()],
    )
    .unwrap();
    assert!(engine.history().unwrap()[0].categories.is_empty());
    db.execute(
        "INSERT INTO reports VALUES('active',?1,?2)",
        rusqlite::params![
            now() - 40 * 86400,
            old.to_string().replace("finished", "running")
        ],
    )
    .unwrap();
    db.execute(
        "UPDATE reports SET started=?1 WHERE id='old'",
        [now() - 40 * 86400],
    )
    .unwrap();
    assert_eq!(engine.history().unwrap().len(), 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM reports WHERE id='active'", [], |r| {
            r.get::<_, i64>(0)
        })
        .unwrap(),
        1
    );
}

#[test]
fn journal_failure_stops_before_any_deletion() {
    let (temp, fake, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    let db = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER no_intents BEFORE INSERT ON intents BEGIN SELECT RAISE(FAIL,'fixture disk failure'); END;").unwrap();
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(engine.status().state, JobState::Failed);
    assert!(fake.deleted.lock().unwrap().is_empty());
    assert_eq!(engine.history().unwrap()[0].status, "interrupted");
}

#[test]
fn result_write_failure_after_deletion_stays_unknown_and_never_replays() {
    let (temp, fake, engine) = fixture(2);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    let db = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    db.execute_batch("CREATE TRIGGER no_result BEFORE UPDATE ON reports BEGIN SELECT RAISE(FAIL,'fixture result write failure'); END;").unwrap();
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(fake.deleted.lock().unwrap().len(), 1);
    let r = engine.report().unwrap();
    assert_eq!(r.status, "interrupted");
    assert_eq!((r.deleted, r.unknown), (0, 1));
    assert_eq!(r.categories[0].unknown, 1);
    let page = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            200,
            CandidateQuery {
                view: "cleanup".into(),
                status: Some("unknown".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(page.total, 1);
    assert!(engine.execute(&plan.id).is_err());
}
proptest::proptest! {
    #[test]
    fn unknown_sessions_never_authorize_deletion(session in "[a-z]{1,50}"){
        let(_temp,fake,engine)=fixture(1);scan(&engine);
        proptest::prop_assert!(engine.prepare(&session,select()).is_err());
        proptest::prop_assert!(fake.deleted.lock().unwrap().is_empty());
    }
}

#[test]
fn snapshot_is_consistent_and_cancelled_categories_are_not_empty_successes() {
    let (_temp, fake, engine) = fixture(100);
    fake.slow.store(true, Ordering::Release);
    engine
        .start_scan(vec!["windows.user_temp".into(), "edge.http".into()])
        .unwrap();
    let snapshot = engine.runtime_snapshot();
    assert_eq!(snapshot.status.id, snapshot.summary.session_id);
    assert_eq!(snapshot.status.state, snapshot.summary.state);
    engine.cancel();
    wait(&engine);
    let snapshot = engine.runtime_snapshot();
    assert_eq!(snapshot.status.state, JobState::Cancelled);
    assert!(
        snapshot
            .summary
            .rules
            .iter()
            .any(|r| r.phase == ScanPhase::Interrupted)
    );
    assert!(engine.prepare(&snapshot.status.id, select()).is_err());
}
#[test]
fn preference_is_separate_persistent_and_records_actual_scan_rules() {
    let (temp, fake, engine) = fixture(1);
    assert!(engine.preferences().unwrap().last_scan_rule_ids.is_none());
    engine
        .patch_settings(SettingsPatch {
            excluded_rules: Some(vec!["edge.http".into()]),
            ..Default::default()
        })
        .unwrap();
    let id = engine
        .start_scan(vec!["windows.user_temp".into(), "edge.http".into()])
        .unwrap();
    engine.remember_scan(&id).unwrap();
    wait(&engine);
    engine
        .patch_settings(SettingsPatch {
            theme: Some("dark".into()),
            ..Default::default()
        })
        .unwrap();
    assert!(engine.start_scan(vec!["fake".into()]).is_err());
    let reopen = Engine::open(fake, Scope::User, &temp.path().join("history.sqlite")).unwrap();
    assert_eq!(
        reopen.preferences().unwrap().last_scan_rule_ids.unwrap(),
        vec!["windows.user_temp"]
    );
    assert_eq!(reopen.settings().theme, "dark");
    assert_eq!(reopen.status().state, JobState::Idle);
}
#[test]
fn exclusion_batch_is_atomic_deduplicated_and_invalidates_old_plan() {
    let (_temp, _, engine) = fixture(2);
    let id = scan(&engine);
    let plan = engine.prepare(&id, select()).unwrap();
    let file = engine
        .candidates(&id, "windows.user_temp", 0, 1)
        .unwrap()
        .items
        .remove(0);
    let item = CandidateExclusion {
        candidate_id: file.id.clone(),
        kind: ExclusionKind::File,
    };
    assert!(
        engine
            .add_candidate_exclusions(
                &id,
                vec![
                    item.clone(),
                    CandidateExclusion {
                        candidate_id: "fake".into(),
                        kind: ExclusionKind::Parent
                    }
                ]
            )
            .is_err()
    );
    assert!(engine.settings().exclusions.is_empty());
    assert!(engine.validate_plan(&plan.id).is_ok());
    engine
        .patch_settings(SettingsPatch {
            theme: Some("dark".into()),
            ..Default::default()
        })
        .unwrap();
    let result = engine
        .add_candidate_exclusions(&id, vec![item.clone(), item.clone()])
        .unwrap();
    assert_eq!(result.settings.exclusions.len(), 1);
    assert_eq!(result.settings.theme, "dark");
    assert!(result.scan_invalidated);
    assert!(engine.validate_plan(&plan.id).is_err());
    assert!(engine.add_candidate_exclusions(&id, vec![item]).is_err());
}
#[test]
fn candidate_exclusion_storage_failure_preserves_settings_and_scan() {
    let (temp, _, engine) = fixture(1);
    let id = scan(&engine);
    let file = engine
        .candidates(&id, "windows.user_temp", 0, 1)
        .unwrap()
        .items
        .remove(0);
    let journal = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    journal.execute_batch("CREATE TRIGGER refuse_settings BEFORE INSERT ON settings BEGIN SELECT RAISE(FAIL,'fixture disk failure'); END;").unwrap();
    assert!(
        engine
            .add_candidate_exclusions(
                &id,
                vec![CandidateExclusion {
                    candidate_id: file.id,
                    kind: ExclusionKind::Parent
                }]
            )
            .is_err()
    );
    assert!(engine.settings().exclusions.is_empty());
    assert_eq!(engine.status().id, id);
    assert_eq!(engine.summary().rules[0].phase, ScanPhase::Completed);
}
#[test]
fn exclusion_limit_and_reveal_ids_cannot_bypass_validation() {
    let (_temp, _, engine) = fixture(1);
    let id = scan(&engine);
    let file = engine
        .candidates(&id, "windows.user_temp", 0, 1)
        .unwrap()
        .items
        .remove(0);
    assert!(engine.reveal_candidate("stale", &file.id).is_err());
    assert!(engine.reveal_candidate(&id, "fake").is_err());
    engine
        .patch_settings(SettingsPatch {
            exclusions: Some((0..500).map(|n| format!("fixture/keep{n}")).collect()),
            ..Default::default()
        })
        .unwrap();
    let id = scan(&engine);
    let file = engine
        .candidates(&id, "windows.user_temp", 0, 1)
        .unwrap()
        .items
        .remove(0);
    assert!(
        engine
            .add_candidate_exclusions(
                &id,
                vec![CandidateExclusion {
                    candidate_id: file.id,
                    kind: ExclusionKind::File
                }]
            )
            .is_err()
    );
    assert_eq!(engine.settings().exclusions.len(), 500);
    assert_eq!(engine.status().id, id);
}

#[test]
fn retained_only_scan_publishes_real_progress_before_completion() {
    let (_temp, fake, engine) = fixture(500);
    fake.recent.store(true, Ordering::Release);
    fake.slow.store(true, Ordering::Release);
    engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = engine.runtime_snapshot();
        if snapshot.status.scan_progress.checked > 0 {
            assert_eq!(snapshot.status.visited, 0);
            assert!(
                snapshot.summary.rules[0]
                    .reasons
                    .iter()
                    .any(|r| r.code == ReasonCode::Recent)
            );
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    engine.cancel();
    wait(&engine);
    let snapshot = engine.runtime_snapshot();
    assert_eq!(snapshot.status.state, JobState::Cancelled);
    assert!(snapshot.status.scan_progress.checked > 0);
    assert_eq!(snapshot.summary.rules[0].files, 0);
    assert_eq!(snapshot.summary.rules[0].phase, ScanPhase::Interrupted);
}
#[test]
fn worker_failure_marks_waiting_categories_interrupted() {
    let (_temp, fake, engine) = fixture(0);
    fake.panic_scan.store(true, Ordering::Release);
    engine
        .start_scan(vec!["windows.user_temp".into(), "edge.http".into()])
        .unwrap();
    wait(&engine);
    assert_eq!(engine.status().state, JobState::Failed);
    assert!(
        engine
            .summary()
            .rules
            .iter()
            .all(|r| !matches!(r.phase, ScanPhase::Pending | ScanPhase::Scanning))
    );
}
#[test]
fn stop_after_acceptance_before_first_file_performs_no_deletion() {
    use std::sync::Barrier;
    let (_temp, fake, engine) = fixture(4);
    let session = scan(&engine);
    let plan = engine.prepare(&session, select()).unwrap();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    *fake.cleanup_gate.lock().unwrap() = Some((entered.clone(), release.clone()));
    engine.execute(&plan.id).unwrap();
    entered.wait();
    engine.cancel();
    release.wait();
    wait(&engine);
    let report = engine.report().unwrap();
    assert_eq!(report.status, "cancelled");
    assert_eq!(report.deleted, 0);
    assert_eq!(report.total, 4);
    assert!(fake.deleted.lock().unwrap().is_empty());
    assert!(engine.execute(&plan.id).is_err());
}
#[test]
fn unplanned_view_is_disjoint_from_cleanup_results() {
    let (_temp, _fake, engine) = fixture(4);
    let session = scan(&engine);
    let file = engine
        .candidates(&session, "windows.user_temp", 0, 1)
        .unwrap()
        .items[0]
        .id
        .clone();
    let mut selection = select();
    selection.excluded_ids.push(file.clone());
    let plan = engine.prepare(&session, selection).unwrap();
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let outside = engine
        .query_candidates(
            &session,
            "windows.user_temp",
            0,
            999,
            CandidateQuery {
                view: "unplanned".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(outside.total, 1);
    assert_eq!(outside.items[0].id, file);
    assert_eq!(engine.report().unwrap().total, 3);
    assert!(
        engine
            .query_candidates(
                "forged",
                "windows.user_temp",
                0,
                1,
                CandidateQuery {
                    view: "unplanned".into(),
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn interrupted_manual_intents_keep_file_and_directory_counts_separate() {
    let (temp, fake, engine) = fixture(0);
    drop(engine);
    let db = rusqlite::Connection::open(temp.path().join("history.sqlite")).unwrap();
    let report = CleanupReport {
        manual: Some(RuleOutcome {
            total: 1,
            deleted_bytes: "0".into(),
            ..Default::default()
        }),
        directories: DirectoryOutcome {
            total: 1,
            ..Default::default()
        },
        volume_space_changes: vec![],
        id: "crash".into(),
        started_at: now(),
        finished_at: None,
        status: "running".into(),
        total: 1,
        deleted: 0,
        skipped: 0,
        failed: 0,
        unknown: 0,
        candidate_bytes: "10".into(),
        deleted_bytes: "0".into(),
        free_space_change: None,
        scope: Scope::User,
        categories: vec![],
    };
    db.execute(
        "INSERT INTO reports VALUES('crash',?1,?2)",
        rusqlite::params![now(), serde_json::to_string(&report).unwrap()],
    )
    .unwrap();
    db.execute("INSERT INTO intents(job,item,outcome,rule) VALUES('crash','manual:file:node','pending',''),('crash','manual:directory:parent','pending','')", [])
        .unwrap();
    drop(db);
    let reopened = Engine::open(
        fake.clone(),
        Scope::User,
        &temp.path().join("history.sqlite"),
    )
    .unwrap();
    let report = &reopened.history().unwrap()[0];
    assert_eq!(report.status, "interrupted");
    assert_eq!(report.unknown, 1);
    assert_eq!(report.manual.as_ref().unwrap().unknown, 1);
    assert_eq!(report.directories.unknown, 1);
    assert!(fake.deleted.lock().unwrap().is_empty());
}
