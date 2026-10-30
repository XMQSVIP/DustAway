use super::*;
use crate::historical_log_probe::{
    Hive, ProcessObservation, Processes, discover_todesk,
    fixture::{APP_PATH, UNINSTALL},
};
use dustaway_core::ScanPhase;

fn identify(f: &Fixture, id: &str) -> Result<crate::historical_logs::LogIdentity> {
    Policy::for_rule(id)
        .unwrap()
        .identify(&f.permit.probe, None)
}
fn identity_refused(f: &Fixture, id: &str) {
    let error = identify(f, id).err().expect("identity must be refused");
    assert_eq!(error_reason(&error), ReasonCode::ApplicationUnverified);
}

#[test]
fn historical_logs_discovery_filters_registrations_and_deduplicates_physical_programs() {
    let f = fixture(RULE_IDS[1], true);
    let p = &f.permit.probe;
    let alias = display_path(&p.exe.with_file_name("todesk.EXE"));
    p.value(Hive::User, APP_PATH, "", Ok(Some(alias)));
    p.children.lock().unwrap().insert(
        (Hive::Machine, UNINSTALL.into()),
        Ok(vec!["ours".into(), "other".into()]),
    );
    let ours = format!(r"{UNINSTALL}\ours");
    let other = format!(r"{UNINSTALL}\other");
    p.value(
        Hive::Machine,
        &ours,
        "DisplayName",
        Ok(Some("ToDesk".into())),
    );
    p.value(
        Hive::Machine,
        &ours,
        "InstallLocation",
        Ok(Some(display_path(p.exe.parent().unwrap()))),
    );
    p.value(
        Hive::Machine,
        &ours,
        "DisplayIcon",
        Ok(Some(format!("\"{}\",0", display_path(&p.exe)))),
    );
    p.value(
        Hive::Machine,
        &other,
        "DisplayName",
        Ok(Some("Other software".into())),
    );
    // Querying an unrelated installation's paths would make discovery incomplete.
    p.value(
        Hive::Machine,
        &other,
        "InstallLocation",
        Err(ReasonCode::AccessDenied),
    );
    let discovery = discover_todesk(p, None).unwrap();
    assert!(discovery.complete);
    // DOS, extended and case variants remain observations until file handles prove sameness.
    assert!(discovery.paths.len() >= 2);
    let identity = identify(&f, RULE_IDS[1]).unwrap();
    let crate::historical_logs::LogIdentity::ToDesk { pin, .. } = &identity else {
        panic!("expected a pinned ToDesk installation");
    };
    assert_eq!(
        pin.stamp.identity,
        PinnedPath::open(&p.exe, false, false)
            .unwrap()
            .stamp
            .identity
    );
    assert_eq!(p.signature_calls.load(Ordering::Relaxed), 1);
    assert_eq!(p.product_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn historical_logs_incomplete_missing_conflicting_or_invalid_installs_never_verify() {
    for mode in [
        "missing",
        "registry-value",
        "registry-enumeration",
        "registered-missing",
        "registration-without-path",
        "conflict",
        "invalid-competing",
        "relative",
        "wrong-executable",
    ] {
        let f = fixture(RULE_IDS[1], true);
        let p = &f.permit.probe;
        match mode {
            "missing" => p.candidates.lock().unwrap().clear(),
            "registry-value" => p.value(Hive::User, APP_PATH, "", Err(ReasonCode::AccessDenied)),
            "registry-enumeration" => {
                p.children.lock().unwrap().insert(
                    (Hive::Machine, UNINSTALL.into()),
                    Err(ReasonCode::AccessDenied),
                );
            }
            "registered-missing" => p.value(
                Hive::User,
                APP_PATH,
                "",
                Ok(Some(display_path(
                    &p.exe.parent().unwrap().join("absent/ToDesk.exe"),
                ))),
            ),
            "registration-without-path" => {
                p.children
                    .lock()
                    .unwrap()
                    .insert((Hive::User, UNINSTALL.into()), Ok(vec!["ours".into()]));
                p.value(
                    Hive::User,
                    &format!(r"{UNINSTALL}\ours"),
                    "DisplayName",
                    Ok(Some("ToDesk".into())),
                );
            }
            "relative" => p.value(
                Hive::User,
                APP_PATH,
                "",
                Ok(Some("relative/ToDesk.exe".into())),
            ),
            "wrong-executable" => p.value(
                Hive::User,
                APP_PATH,
                "",
                Ok(Some(display_path(&p.exe.with_file_name("Other.exe")))),
            ),
            _ => {
                let other = extended(&f.permit.directory.path().join("second/ToDesk.exe")).unwrap();
                assert!(other.starts_with(extended(f.permit.directory.path()).unwrap()));
                std::fs::create_dir_all(other.parent().unwrap()).unwrap();
                std::fs::copy(&p.exe, &other).unwrap();
                let mut facts = p.facts.lock().unwrap().get(&p.exe).unwrap().clone();
                if mode == "invalid-competing" {
                    facts.publisher = "untrusted".into();
                }
                p.facts.lock().unwrap().insert(other.clone(), facts);
                p.candidates.lock().unwrap().push(other);
            }
        }
        identity_refused(&f, RULE_IDS[1]);
        if matches!(
            mode,
            "registry-value" | "registry-enumeration" | "relative" | "wrong-executable"
        ) {
            assert_eq!(p.signature_calls.load(Ordering::Relaxed), 0);
        }
        assert!(f.files.iter().all(|p| p.exists()));
    }
}

#[test]
fn historical_logs_signature_precedes_resources_and_missing_metadata_blocks() {
    for mode in [
        "signature",
        "publisher",
        "product",
        "missing-product",
        "missing-version",
    ] {
        let f = fixture(RULE_IDS[1], true);
        let p = &f.permit.probe;
        {
            let mut facts = p.facts.lock().unwrap();
            let facts = facts.get_mut(&p.exe).unwrap();
            match mode {
                "signature" => facts.signature_valid = false,
                "publisher" => facts.publisher = "untrusted".into(),
                "product" => facts.product = Some("Other application".into()),
                "missing-product" => facts.product = None,
                _ => facts.version = None,
            }
        }
        identity_refused(&f, RULE_IDS[1]);
        assert_eq!(
            p.product_calls.load(Ordering::Relaxed),
            usize::from(!matches!(mode, "signature" | "publisher"))
        );
        assert_eq!(
            p.version_calls.load(Ordering::Relaxed),
            usize::from(mode == "missing-version")
        );
    }
    // Missing registration is normal when a single standard installation verifies.
    let f = fixture(RULE_IDS[1], true);
    let before = identify(&f, RULE_IDS[1]).unwrap().fingerprint().unwrap();
    f.permit.probe.replace_program();
    assert_ne!(
        before,
        identify(&f, RULE_IDS[1]).unwrap().fingerprint().unwrap()
    );
}

#[test]
fn historical_logs_process_decision_uses_one_snapshot_and_preserves_running_priority() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let p = &f.permit.probe;
        let identity = identify(&f, id).unwrap();
        let own = p.exe.parent().unwrap().join("background.exe");
        let other = p
            .exe
            .parent()
            .unwrap()
            .with_file_name("installation-other")
            .join("updater.exe");
        let main = p.exe.file_name().unwrap().to_str().unwrap().to_owned();
        let cases = [
            (vec![], true, Some(1), None),
            (
                vec![ProcessObservation {
                    name: main.clone(),
                    session: Some(2),
                    image: None,
                }],
                true,
                Some(1),
                Some(ReasonCode::ApplicationRunning),
            ),
            (
                vec![ProcessObservation {
                    name: "background.exe".into(),
                    session: Some(1),
                    image: Some(own.clone()),
                }],
                true,
                Some(1),
                Some(ReasonCode::ApplicationRunning),
            ),
            (
                vec![ProcessObservation {
                    name: "background.exe".into(),
                    session: Some(1),
                    image: None,
                }],
                true,
                Some(1),
                Some(ReasonCode::ProcessUnknown),
            ),
            (
                vec![ProcessObservation {
                    name: "updater.exe".into(),
                    session: Some(1),
                    image: Some(other.clone()),
                }],
                true,
                Some(1),
                None,
            ),
            (vec![], false, Some(1), Some(ReasonCode::ProcessUnknown)),
            (vec![], true, None, Some(ReasonCode::ProcessUnknown)),
            (
                vec![ProcessObservation {
                    name: "background.exe".into(),
                    session: None,
                    image: None,
                }],
                true,
                Some(1),
                Some(ReasonCode::ProcessUnknown),
            ),
            (
                vec![ProcessObservation {
                    name: main,
                    session: None,
                    image: None,
                }],
                false,
                None,
                Some(ReasonCode::ApplicationRunning),
            ),
        ];
        for (entries, complete, current_session, expected) in cases {
            *p.process_state.lock().unwrap() = Processes {
                entries,
                complete,
                current_session,
            };
            let calls = p.process_calls.load(Ordering::Relaxed);
            let result = identity.require_stopped(p, None);
            assert_eq!(result.err().map(|e| error_reason(&e)), expected);
            assert_eq!(p.process_calls.load(Ordering::Relaxed), calls + 1);
        }
        if id == RULE_IDS[1] {
            for (image, expected) in [
                (Some(own), Some(ReasonCode::ApplicationRunning)),
                (Some(other), None),
                (None, Some(ReasonCode::ProcessUnknown)),
            ] {
                *p.process_state.lock().unwrap() = Processes {
                    current_session: Some(1),
                    complete: true,
                    entries: vec![ProcessObservation {
                        name: "updater.exe".into(),
                        session: Some(2),
                        image,
                    }],
                };
                assert_eq!(
                    identity
                        .require_stopped(p, None)
                        .err()
                        .map(|e| error_reason(&e)),
                    expected
                );
            }
        }
    }
}

#[test]
fn historical_logs_late_process_start_skips_remaining_frozen_files() {
    for id in RULE_IDS {
        let f = fixture(id, true);
        let before = hashes(&f.protected);
        let (sent, received) = mpsc::channel();
        let (resume, resumed) = mpsc::channel();
        let controlled = Arc::new(Controlled {
            platform: f.platform.clone(),
            pause: Mutex::new(Some((sent, resumed))),
            fail: false,
        });
        let engine = Engine::open(controlled, Scope::User, &f.journal).unwrap();
        let session = scan(&engine, id);
        let plan = engine.prepare(&session, selection(id)).unwrap();
        engine.execute(&plan.id).unwrap();
        received.recv_timeout(Duration::from_secs(20)).unwrap();
        f.permit.probe.set_running(
            f.permit.probe.exe.file_name().unwrap().to_str().unwrap(),
            Some(2),
            None,
        );
        resume.send(()).unwrap();
        wait(&engine);
        let report = engine.report().unwrap();
        assert_eq!(
            (report.deleted, report.skipped, report.failed),
            (1, plan.files - 1, 0)
        );
        assert!(
            report.categories[0]
                .reasons
                .iter()
                .any(|r| r.code == ReasonCode::ApplicationRunning)
        );
        assert_eq!(hashes(&f.protected), before);
    }
}

#[test]
fn historical_logs_partial_acl_failure_keeps_other_roots_and_partial_selection() {
    let id = RULE_IDS[0];
    let f = fixture(id, true);
    let before = hashes(&f.protected);
    let roots = f.platform.roots(&rule(id)).unwrap();
    let guard = super::super::scan_tests::Denied::new(&roots[0]);
    let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
    let session = scan(&engine, id);
    let row = &engine.summary().rules[0];
    assert_eq!(row.phase, ScanPhase::Partial);
    assert!(!row.scan_failed && !row.blocked);
    assert!(!row.coverage.as_ref().unwrap().complete);
    assert_eq!(row.files, 3);
    let plan = engine.prepare(&session, selection(id)).unwrap();
    assert_eq!(plan.files, 3);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(engine.report().unwrap().deleted, 3);
    drop(guard);
    assert!(
        f.files
            .iter()
            .filter(|p| p.parent() == Some(roots[0].as_path()))
            .all(|p| p.exists())
    );
    assert_eq!(hashes(&f.protected), before);
}

#[test]
fn historical_logs_all_unreadable_fails_but_empty_readable_root_is_partial() {
    for empty_other in [false, true] {
        let id = RULE_IDS[0];
        let f = fixture(id, true);
        let roots = f.platform.roots(&rule(id)).unwrap();
        let first = super::super::scan_tests::Denied::new(&roots[0]);
        let second = if empty_other {
            let saved = extended(&f.permit.directory.path().join("saved-second-root")).unwrap();
            let base = extended(f.permit.directory.path()).unwrap();
            assert!(roots[1].starts_with(&base) && saved.starts_with(&base));
            std::fs::rename(&roots[1], saved).unwrap();
            std::fs::create_dir(&roots[1]).unwrap();
            None
        } else {
            Some(super::super::scan_tests::Denied::new(&roots[1]))
        };
        let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
        let session = scan(&engine, id);
        let row = &engine.summary().rules[0];
        assert_eq!(
            row.phase,
            if empty_other {
                ScanPhase::Partial
            } else {
                ScanPhase::Failed
            }
        );
        assert_eq!(row.scan_failed, !empty_other);
        assert_eq!(row.files, 0);
        assert!(!row.coverage.as_ref().unwrap().complete);
        assert!(engine.prepare(&session, selection(id)).is_err());
        drop(second);
        drop(first);
    }
}

#[test]
fn historical_logs_iterator_and_metadata_errors_preserve_known_capacity_and_selection() {
    for id in RULE_IDS {
        for metadata in [false, true] {
            let f = fixture(id, true);
            let roots = f.platform.roots(&rule(id)).unwrap();
            if metadata {
                f.permit
                    .probe
                    .metadata_errors
                    .lock()
                    .unwrap()
                    .insert(f.files[0].clone(), ReasonCode::AccessDenied);
            } else {
                f.permit
                    .probe
                    .entry_errors
                    .lock()
                    .unwrap()
                    .insert(roots[0].clone(), 1);
            }
            let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
            let session = scan(&engine, id);
            let summary = engine.summary();
            let row = &summary.rules[0];
            assert_eq!(row.phase, ScanPhase::Partial);
            assert!(!row.scan_failed && !row.blocked);
            let page = engine.candidates(&session, id, 0, 200).unwrap();
            assert_eq!(page.total as usize, f.files.len() - usize::from(metadata));
            assert_eq!(
                row.bytes.parse::<u64>().unwrap(),
                page.items
                    .iter()
                    .map(|f| f.bytes.parse::<u64>().unwrap())
                    .sum::<u64>()
            );
            if metadata {
                let mut events = vec![];
                f.platform
                    .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| {
                        events.push(e)
                    })
                    .unwrap();
                assert!(events.iter().any(|e| matches!(e, ScanEvent::Observation(o) if o.path == display_path(&f.files[0]) && o.bytes.is_none())));
            }
            let plan = engine.prepare(&session, selection(id)).unwrap();
            assert_eq!(plan.files, page.total);
            assert!(f.files.iter().all(|p| p.exists()));
        }
    }
}

#[test]
fn historical_logs_cancel_discards_late_probe_results_and_emits_no_false_failures() {
    for id in RULE_IDS {
        let mut points = vec!["registry", "signature", "version", "read_dir", "metadata"];
        if id == RULE_IDS[1] {
            points.push("product");
        }
        for point in points {
            let f = fixture(id, true);
            let cancel = Arc::new(AtomicBool::new(false));
            *f.permit.probe.cancel_at.lock().unwrap() = Some((point, cancel.clone()));
            let mut events = vec![];
            f.platform
                .scan(&rule(id), &[], &cancel, &mut |e| events.push(e))
                .unwrap();
            assert!(cancel.load(Ordering::Acquire), "{id}: {point}");
            assert!(
                !events
                    .iter()
                    .any(|e| matches!(e, ScanEvent::File(_) | ScanEvent::CoverageIssue(_)))
            );
        }
        let f = fixture(id, true);
        let cancel = Arc::new(AtomicBool::new(false));
        *f.permit.probe.cancel_entry.lock().unwrap() = Some((2, cancel.clone()));
        f.platform
            .scan(&rule(id), &[], &cancel, &mut |e| {
                if matches!(e, ScanEvent::File(_)) {
                    assert!(!cancel.load(Ordering::Acquire));
                }
                assert!(!matches!(e, ScanEvent::CoverageIssue(_)));
            })
            .unwrap();
        assert!(cancel.load(Ordering::Acquire));
        let identity = identify(&f, id).unwrap();
        cancel.store(false, Ordering::Release);
        *f.permit.probe.cancel_at.lock().unwrap() = Some(("process", cancel.clone()));
        assert!(
            identity
                .require_stopped(&f.permit.probe, Some(&cancel))
                .unwrap_err()
                .is::<ScanCancelled>()
        );
    }
}

#[test]
fn historical_logs_engine_cancellation_stops_post_scan_identity_and_process_checks() {
    for id in RULE_IDS {
        for point in ["registry", "signature", "read_dir", "metadata"] {
            let f = fixture(id, true);
            let (sent, received) = mpsc::channel();
            let (resume, resumed) = mpsc::channel();
            *f.permit.probe.pause_at.lock().unwrap() = Some((point, sent, resumed));
            let engine = Engine::open(f.platform.clone(), Scope::User, &f.journal).unwrap();
            let session = engine.start_scan(vec![id.into()]).unwrap();
            received.recv_timeout(Duration::from_secs(20)).unwrap();
            engine.cancel();
            resume.send(()).unwrap();
            wait(&engine);
            assert_eq!(engine.status().state, JobState::Cancelled);
            let row = &engine.summary().rules[0];
            assert_eq!(row.phase, ScanPhase::Interrupted);
            assert!(!row.scan_failed);
            assert_eq!((row.files, row.bytes.as_str()), (0, "0"));
            assert!(row.coverage.as_ref().unwrap().issues.is_empty());
            assert_eq!(f.permit.probe.process_calls.load(Ordering::Relaxed), 0);
            assert!(engine.prepare(&session, selection(id)).is_err());
            assert!(f.files.iter().all(|p| p.exists()));
        }
    }
}
