use super::*;
#[path = "app_cache_scan_tests.rs"]
mod application_cache;
#[path = "coverage_scan_tests.rs"]
mod coverage_observed;
#[path = "developer_tests.rs"]
mod developer_tests;
#[path = "discovered_cache_tests.rs"]
mod discovered_cache_tests;
#[path = "scan_tests_netdisk.rs"]
mod netdisk;
use dustaway_core::{Engine, ScanPhase, Selection};
use std::{
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

fn fixture() -> (tempfile::TempDir, WindowsPlatform) {
    let temp = tempfile::tempdir().unwrap();
    let platform = WindowsPlatform {
        inventory: std::sync::Mutex::new(Some(crate::app_cache::Inventory {
            entries: vec![],
            complete: true,
        })),
        tool_roots: Default::default(),
        tool_probe: std::sync::Arc::new(crate::tool_discovery::UnavailableProbe),
        real_app_acceptance: None,
        log_test_acceptance: None,
        discovery_drives: vec![],
        app_test_identity: None,
        app_test_acceptance: None,
        scope: Scope::User,
        local: extended(&temp.path().join("local")).unwrap(),
        roaming: temp.path().join("roaming"),
        system: temp.path().join("system"),
    };
    (temp, platform)
}

#[test]
fn tool_discovery_uses_injected_production_orchestration_and_retains_standard_remnants() {
    use crate::tool_discovery::{Discovered, Probe};
    struct Configured(PathBuf);
    impl Probe for Configured {
        fn query(&self, _: &str, _: &AtomicBool) -> Result<Discovered> {
            Ok(Discovered {
                tool: dustaway_core::ToolDiscovery {
                    executable: Some("C:\\tool.exe".into()),
                    entry: None,
                    version: Some("1".into()),
                    target: Some(display_path(&self.0)),
                    ready: true,
                    reason: None,
                    message: "ready".into(),
                    identity: Some("pinned".into()),
                },
            })
        }
    }
    let (temp, mut platform) = fixture();
    let target = extended(&temp.path().join("configured/cache")).unwrap();
    platform.tool_probe = Arc::new(Configured(target.clone()));
    let rule = rule("dev.uv");
    let cancel = AtomicBool::new(false);
    let roots = platform
        .discover_roots(&rule, Some(&cancel), &mut |_| {})
        .unwrap();
    assert!(
        roots
            .iter()
            .any(|p| display_path(p).eq_ignore_ascii_case(&display_path(&target)))
    );
    assert!(roots.iter().any(|p| p.ends_with("uv/cache")));
    assert!(platform.tool_discovery(&rule.id).unwrap().ready);
    assert_eq!(platform.roots(&rule).unwrap().len(), roots.len());
    platform.tool_probe = Arc::new(crate::tool_discovery::UnavailableProbe);
    let fallback = platform
        .discover_roots(&rule, Some(&cancel), &mut |_| {})
        .unwrap();
    assert!(fallback.iter().any(|p| p.ends_with("uv/cache")));
    let state = platform.tool_discovery(&rule.id).unwrap();
    assert!(!state.ready && state.target.is_none());
    platform.invalidate_tool_discovery(Some(&rule.id));
    assert!(platform.tool_discovery(&rule.id).is_none());
}
fn rule(id: &str) -> RuleDescriptor {
    dustaway_core::rules::catalog()
        .unwrap()
        .into_iter()
        .find(|r| r.id == id)
        .unwrap()
}

#[test]
#[ignore = "Explicit isolated test PC; only selects files created by this acceptance test"]
fn remote_standard_temp_acceptance() {
    // A real user Temp can contain tens of thousands of files; the short fixture
    // deadline used by unit tests does not apply to this explicit remote test.
    fn wait(engine: &Engine) {
        let deadline = Instant::now() + Duration::from_secs(300);
        while engine.is_busy() {
            assert!(Instant::now() < deadline, "Remote Temp scan did not finish");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    assert_eq!(std::env::var("COMPUTERNAME").unwrap(), "DESKTOP-24NJTT6");
    let evidence =
        PathBuf::from(r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261003-system-cleanup");
    assert_eq!(
        std::fs::read_to_string(evidence.join(".dustaway-test")).unwrap(),
        "DustAway.SystemCleanup.20261003"
    );
    let scope = if security::elevated().unwrap() {
        Scope::System
    } else {
        Scope::User
    };
    let platform = WindowsPlatform::new(scope).unwrap();
    let rule_id = if scope == Scope::System {
        "windows.system_temp"
    } else {
        "windows.user_temp"
    };
    let descriptor = rule(rule_id);
    let root = platform.roots(&descriptor).unwrap().remove(0);
    let fixture = tempfile::Builder::new()
        .prefix("DustAway-24h-acceptance-")
        .tempdir_in(&root)
        .unwrap();
    let old = fixture.path().join("older-than-24h.msi");
    let recent = fixture.path().join("recent.zip");
    let future = fixture.path().join("future.vsix");
    write(&old, true);
    filetime::set_file_mtime(&old, filetime::FileTime::from_unix_time(now() - 86460, 0)).unwrap();
    write(&recent, false);
    write(&future, false);
    filetime::set_file_mtime(
        &future,
        filetime::FileTime::from_unix_time(now() + 86400, 0),
    )
    .unwrap();
    let engine = Engine::open(
        Arc::new(platform),
        scope,
        &evidence.join(if scope == Scope::System {
            "system-temp-journal"
        } else {
            "user-temp-journal"
        }),
    )
    .unwrap();
    let session = engine.start_scan(vec![rule_id.into()]).unwrap();
    wait(&engine);
    let mut chosen = None;
    let mut offset = 0;
    loop {
        let page = engine.candidates(&session, rule_id, offset, 200).unwrap();
        for row in page.items {
            if row.path == display_path(&old) {
                chosen = Some(row.id)
            }
        }
        offset += 200;
        if offset >= page.total {
            break;
        }
    }
    let candidate = chosen.expect("Owned file older than 24 hours should be a candidate");
    let plan = engine
        .prepare(
            &session,
            Selection {
                included_ids: vec![candidate],
                reviewed_rule_ids: vec![rule_id.into()],
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(plan.files, 1);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    let report = engine.report().unwrap();
    assert_eq!(report.deleted, 1);
    assert!(!old.exists());
    assert!(recent.exists() && future.exists());
    std::fs::write(
        evidence.join(if scope == Scope::System {
            "system-temp-report.json"
        } else {
            "user-temp-report.json"
        }),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
fn write(path: &Path, old: bool) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"protected fixture contents").unwrap();
    if old {
        filetime::set_file_mtime(
            path,
            filetime::FileTime::from_unix_time(now() - 30 * 86400, 0),
        )
        .unwrap();
    }
}
fn wait(engine: &Engine) {
    let until = Instant::now() + Duration::from_secs(10);
    while engine.is_busy() {
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn scan(engine: &Arc<Engine>) -> String {
    let id = engine.start_scan(vec!["windows.user_temp".into()]).unwrap();
    wait(engine);
    id
}
fn selection() -> Selection {
    Selection {
        rule_ids: vec!["windows.user_temp".into()],
        excluded_ids: vec![],
        ..Selection::default()
    }
}

#[test]
fn selected_volumes_limit_enumeration_and_reject_empty_scope() {
    let (temp, platform) = fixture();
    let file = platform.local.join("Temp/old.tmp");
    write(&file, true);
    let actual = volume_target(&file).unwrap();
    let other = dustaway_core::VolumeTarget {
        key: "not-this-volume".into(),
        label: "Z:\\".into(),
        opaque: String::new(),
    };
    let rule = rule("windows.user_temp");
    let mut checked = 0;
    let mut files = 0;
    platform
        .scan_on_volumes(
            &rule,
            &[],
            Some(std::slice::from_ref(&other)),
            &AtomicBool::new(false),
            &mut |event| {
                if matches!(event, ScanEvent::Checked) {
                    checked += 1;
                }
                if matches!(event, ScanEvent::File(_)) {
                    files += 1;
                }
            },
        )
        .unwrap();
    assert_eq!((checked, files), (0, 0));
    assert!(
        platform
            .scan_on_volumes(&rule, &[], Some(&[]), &AtomicBool::new(false), &mut |_| {})
            .is_err()
    );
    let engine = Engine::open(
        Arc::new(platform),
        Scope::User,
        &temp.path().join("journal.sqlite"),
    )
    .unwrap();
    assert!(
        engine
            .start_scan_on_volumes(vec![rule.id.clone()], Some(vec![]))
            .is_err()
    );
    engine
        .start_scan_on_volumes(vec![rule.id.clone()], Some(vec![actual.clone()]))
        .unwrap();
    wait(&engine);
    assert_eq!(engine.summary().rules[0].files, 1);
    assert_eq!(
        engine.status().scan_volumes,
        Some(vec![actual.label.clone()])
    );
    engine
        .start_scan_on_volumes(vec![rule.id.clone()], Some(vec![other, actual]))
        .unwrap();
    wait(&engine);
    assert_eq!(engine.summary().rules[0].files, 1);
    engine.start_scan(vec![rule.id]).unwrap();
    wait(&engine);
    assert!(engine.status().scan_volumes.is_none());
    assert_eq!(engine.summary().rules[0].files, 1);
    assert_eq!(std::fs::read(&file).unwrap(), b"protected fixture contents");
}

#[test]
fn old_crash_dumps_remain_readonly_without_real_acceptance() {
    let (_temp, platform) = fixture();
    let root = platform.local.join("CrashDumps");
    let old = root.join("old.dmp");
    write(&old, true);
    let mut dump = vec![0; 48];
    dump[..4].copy_from_slice(b"MDMP");
    dump[4..8].copy_from_slice(&0xa793u32.to_le_bytes());
    dump[8..12].copy_from_slice(&1u32.to_le_bytes());
    dump[12..16].copy_from_slice(&32u32.to_le_bytes());
    dump[36..40].copy_from_slice(&4u32.to_le_bytes());
    dump[40..44].copy_from_slice(&44u32.to_le_bytes());
    std::fs::write(&old, &dump).unwrap();
    filetime::set_file_mtime(
        &old,
        filetime::FileTime::from_unix_time(now() - 31 * 86400, 0),
    )
    .unwrap();
    std::fs::write(root.join("recent.dmp"), &dump).unwrap();
    write(&root.join("invalid.dmp"), true);
    write(&root.join("nested/keep.dmp"), true);
    let descriptor = rule("windows.crash_dumps");
    let mut files = vec![];
    platform
        .scan(&descriptor, &[], &AtomicBool::new(false), &mut |event| {
            if let ScanEvent::File(file) = event {
                files.push(file);
            }
        })
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        platform
            .delete(&descriptor, &files[0], &[display_path(&old)])
            .is_err()
    );
    assert!(old.exists());
    assert_eq!(
        error_reason(&platform.delete(&descriptor, &files[0], &[]).unwrap_err()),
        ReasonCode::ValidationPending
    );
    assert!(old.exists());
    for path in ["recent.dmp", "invalid.dmp", "nested/keep.dmp"] {
        assert!(root.join(path).exists());
    }
}

#[test]
fn age_cutoff_keeps_one_second_newer_and_future_files() {
    let (_temp, platform) = fixture();
    let root = platform.local.join("age-fixture");
    let path = root.join("candidate");
    write(&path, true);
    let cutoff = now() - 30 * 86400;
    for (time, accepted) in [(cutoff, true), (cutoff + 1, false), (now() + 86400, false)] {
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(time, 0)).unwrap();
        assert_eq!(snapshot_at(&root, &path, cutoff, None).is_ok(), accepted);
    }
}

#[test]
fn temp_24_hours_counts_recent_packages_without_making_observations_deletable() {
    let (temp, platform) = fixture();
    let root = platform.local.join("Temp");
    for n in 0..269 {
        let file = root.join(format!("old-{n:03}.tmp"));
        write(&file, true);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&file)
            .unwrap()
            .set_len(if n == 0 {
                40_996_396 - 268 * 1024
            } else {
                1024
            })
            .unwrap();
    }
    for (name, size, age) in [
        ("installer.msi", 400_000, 2 * 86400),
        ("recent.zip", 900_000, 12 * 3600),
        ("recent.vsix", 800_000, 6 * 3600),
        ("future.tmp", 700_000, -86400),
    ] {
        let path = root.join(name);
        write(&path, false);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(size)
            .unwrap();
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(now() - age, 0))
            .unwrap();
    }
    // set_len updates timestamps: restore the age of the 269 old fixture files.
    for n in 0..269 {
        filetime::set_file_mtime(
            root.join(format!("old-{n:03}.tmp")),
            filetime::FileTime::from_unix_time(now() - 8 * 86400, 0),
        )
        .unwrap();
    }
    let engine = Engine::open(
        Arc::new(platform),
        Scope::User,
        &temp.path().join("journal"),
    )
    .unwrap();
    let session = scan(&engine);
    let summary = engine.summary();
    let row = &summary.rules[0];
    assert_eq!(row.files, 270);
    assert_eq!(row.bytes, "41396396");
    let observation = row.observation.as_ref().unwrap();
    assert_eq!(observation.recent_files, 3);
    assert_eq!(observation.retained_bytes, "2400000");
    assert_eq!(observation.reasons[0].code, ReasonCode::Recent);
    let first = engine
        .observations(&session, "windows.user_temp", 0, 2)
        .unwrap();
    assert_eq!(first.total, 3);
    assert!(first.items[0].path.ends_with("recent.zip"));
    assert!(first.items[1].path.ends_with("recent.vsix"));
    assert_eq!(
        engine
            .observations(&session, "windows.user_temp", 2, 2)
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(
        engine
            .observations("expired", "windows.user_temp", 0, 50)
            .is_err()
    );
    assert!(engine.observations(&session, "not-a-rule", 0, 50).is_err());
    let injected = Selection {
        included_ids: vec![first.items[0].path.clone()],
        ..Default::default()
    };
    assert!(engine.prepare(&session, injected).is_err());
    let plan = engine.prepare(&session, selection()).unwrap();
    assert_eq!(plan.files, 270);
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(engine.report().unwrap().deleted, 270);
    for name in ["recent.zip", "recent.vsix", "future.tmp"] {
        assert!(root.join(name).is_file());
    }
    let next = scan(&engine);
    assert_ne!(next, session);
    assert!(
        engine
            .observations(&session, "windows.user_temp", 0, 50)
            .is_err()
    );
}

#[test]
fn temp_policy_uses_continuous_86400_seconds_at_scan_prepare_and_delete() {
    let (_temp, platform) = fixture();
    let root = platform.local.join("Temp");
    let path = root.join("package.vsix");
    write(&path, true);
    let descriptor = rule("windows.user_temp");
    assert_eq!((descriptor.version, descriptor.min_age_days), (2, 1));
    let cutoff = now() - 86400;
    for (when, accepted) in [
        (cutoff - 1, true),
        (cutoff, true),
        (cutoff + 1, false),
        (now() + 86400, false),
    ] {
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(when, 0)).unwrap();
        assert_eq!(snapshot_at(&root, &path, cutoff, None).is_ok(), accepted);
    }
    filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(cutoff - 10, 0)).unwrap();
    let record = snapshot_at(&root, &path, cutoff, None).unwrap();
    platform.validate_candidate(&descriptor, &record).unwrap();
    std::fs::write(&path, b"modified after scan").unwrap();
    assert!(platform.validate_candidate(&descriptor, &record).is_err());
    assert!(platform.delete(&descriptor, &record, &[]).is_err());
    assert!(path.exists());
}

#[test]
fn iis_history_is_limited_to_standard_sites_named_logs_and_thirty_days() {
    let (temp, mut platform) = fixture();
    platform.scope = Scope::System;
    write(
        &platform
            .system
            .join("System32/inetsrv/config/applicationHost.config"),
        false,
    );
    let root = temp.path().join("inetpub/logs/LogFiles/W3SVC1");
    for name in [
        "u_ex250101.log",
        "u_ex250102.log",
        "not-iis.log",
        "nested/u_ex250103.log",
    ] {
        let path = root.join(name);
        write(&path, true);
        std::fs::write(&path,b"#Software: Microsoft Internet Information Services 10.0\r\n#Fields: date time\r\n2025-01-01 00:00:00").unwrap();
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_unix_time(now() - 31 * 86400, 0),
        )
        .unwrap();
    }
    filetime::set_file_mtime(
        root.join("u_ex250102.log"),
        filetime::FileTime::from_unix_time(now() - 29 * 86400, 0),
    )
    .unwrap();
    write(
        &temp
            .path()
            .join("inetpub/logs/LogFiles/custom/u_ex250101.log"),
        true,
    );
    let descriptor = rule("windows.iis_logs");
    let mut files = vec![];
    platform
        .scan(&descriptor, &[], &AtomicBool::new(false), &mut |event| {
            if let ScanEvent::File(file) = event {
                files.push(file)
            }
        })
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].path_display.ends_with("u_ex250101.log"));
    platform.validate_candidate(&descriptor, &files[0]).unwrap();
    platform.delete(&descriptor, &files[0], &[]).unwrap();
    assert!(root.join("u_ex250102.log").exists());
    assert!(root.join("not-iis.log").exists());
    assert!(root.join("nested/u_ex250103.log").exists());
}

#[test]
fn windows_residue_scan_is_narrow_readonly_and_revalidates_changed_files() {
    for spec in dustaway_core::residue::RULES {
        let (_temp, platform) = fixture();
        let base = platform.local.join(spec.relative);
        let root = if spec.id == "windows.crash_dumps" {
            base.clone()
        } else {
            base.join("fixture-report")
        };
        let extension = if spec.id == "windows.directx_cache" {
            "idx"
        } else {
            "dmp"
        };
        let old = root.join(format!("old.{extension}"));
        write(&old, true);
        write(&root.join(format!("recent.{extension}")), false);
        write(&root.join("personal.docx"), true);
        write(&root.join("nested/protected.dmp"), true);
        if spec.id == "windows.wer_archive" {
            std::fs::write(root.join("Report.wer"), b"Version=1\nEventType=APPCRASH\n").unwrap();
            write(&base.join("unmarked/old.dmp"), true);
            write(&base.parent().unwrap().join("ReportQueue/queued.dmp"), true);
        }
        if spec.id == "windows.directx_cache" {
            write(&root.join("old.val"), true);
            write(&root.join("recent.val"), false);
            write(&root.join("unpaired.idx"), true);
            write(&base.join("unknown/arbitrary.bin"), true);
        }
        if spec.id == "windows.crash_dumps" {
            let mut dump = vec![0; 48];
            dump[..4].copy_from_slice(b"MDMP");
            dump[4..8].copy_from_slice(&0xa793u32.to_le_bytes());
            dump[8..12].copy_from_slice(&1u32.to_le_bytes());
            dump[12..16].copy_from_slice(&32u32.to_le_bytes());
            dump[36..40].copy_from_slice(&4u32.to_le_bytes());
            dump[40..44].copy_from_slice(&44u32.to_le_bytes());
            std::fs::write(&old, &dump).unwrap();
            std::fs::write(root.join("recent.dmp"), &dump).unwrap();
            filetime::set_file_mtime(
                &old,
                filetime::FileTime::from_unix_time(now() - 31 * 86400, 0),
            )
            .unwrap();
            write(&root.join("fake.dmp"), true);
        }
        let mut files = vec![];
        platform
            .scan(&rule(spec.id), &[], &AtomicBool::new(false), &mut |e| {
                if let ScanEvent::File(f) = e {
                    files.push(f);
                }
            })
            .unwrap();
        assert_eq!(
            files.len(),
            if spec.id == "windows.directx_cache" {
                2
            } else {
                1
            },
            "{}",
            spec.id
        );
        {
            assert_eq!(
                platform.blocked(&rule(spec.id)).unwrap().unwrap().code,
                ReasonCode::ValidationPending
            );
            for file in &files {
                platform.validate_candidate(&rule(spec.id), file).unwrap();
                assert_eq!(
                    error_reason(&platform.delete(&rule(spec.id), file, &[]).unwrap_err()),
                    ReasonCode::ValidationPending
                );
            }
        }
        assert!(old.exists());
        let selected = files
            .iter()
            .find(|f| f.path_display.ends_with(&format!("old.{extension}")))
            .unwrap();
        std::fs::write(&old, b"changed after scan").unwrap();
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&rule(spec.id), selected)
                    .unwrap_err()
            ),
            if spec.id == "windows.crash_dumps" {
                ReasonCode::Protected
            } else {
                ReasonCode::Changed
            }
        );
    }
}

#[test]
fn windows_residue_rejects_invalid_report_markers_and_preserves_exclusions() {
    let (_temp, platform) = fixture();
    let root = platform
        .local
        .join("Microsoft/Windows/WER/ReportArchive/untrusted");
    write(&root.join("old.dmp"), true);
    std::fs::write(root.join("Report.wer"), b"not an error report").unwrap();
    assert!(
        platform
            .roots(&rule("windows.wer_archive"))
            .unwrap()
            .is_empty()
    );
    let crash = platform.local.join("CrashDumps");
    write(&crash.join("old.dmp"), true);
    let mut files = 0;
    platform
        .scan(
            &rule("windows.crash_dumps"),
            &[display_path(&crash)],
            &AtomicBool::new(false),
            &mut |e| {
                if matches!(e, ScanEvent::File(_)) {
                    files += 1;
                }
            },
        )
        .unwrap();
    assert_eq!(files, 0);
}

#[test]
fn domestic_scan_filters_diagnostics_and_never_opens_the_release_gate() {
    for id in ["weixin.logs", "weixin.dumps", "wps.logs"] {
        let (_temp, mut platform) = fixture();
        platform.app_test_identity = Some("isolated-test-identity".into());
        let spec = dustaway_core::applications::rule(id).unwrap();
        let root = platform.roaming.join(spec.base).join(spec.relative);
        let extension = spec.extension.unwrap();
        let old = root.join(format!("archive.{extension}"));
        let recent = root.join(format!("recent.{extension}"));
        let future = root.join(format!("future.{extension}"));
        write(&old, false);
        write(&recent, false);
        write(&future, false);
        filetime::set_file_mtime(
            &old,
            filetime::FileTime::from_unix_time(now() - (i64::from(spec.age) + 1) * 86400, 0),
        )
        .unwrap();
        filetime::set_file_mtime(
            &future,
            filetime::FileTime::from_unix_time(now() + 86400, 0),
        )
        .unwrap();
        for name in [
            "buffer.mmap",
            "buffer.mmap3",
            "config.ini",
            "state.dat",
            "document.docx",
            "history.db",
        ] {
            write(&root.join(name), true);
        }
        let mut files = vec![];
        let mut reasons = vec![];
        platform
            .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| match e {
                ScanEvent::File(f) => files.push(f),
                ScanEvent::Retained(r) | ScanEvent::RetainedFile { reason: r, .. } => {
                    reasons.push(r)
                }
                _ => {}
            })
            .unwrap();
        assert_eq!(files.len(), 1, "{id}");
        assert!(
            files[0]
                .path_display
                .ends_with(&format!("archive.{extension}"))
        );
        assert_eq!(
            reasons
                .iter()
                .filter(|r| **r == ReasonCode::Protected)
                .count(),
            6
        );
        assert_eq!(
            reasons.iter().filter(|r| **r == ReasonCode::Recent).count(),
            2
        );
        platform.validate_candidate(&rule(id), &files[0]).unwrap();
        assert_eq!(
            error_reason(&platform.delete(&rule(id), &files[0], &[]).unwrap_err()),
            ReasonCode::ValidationPending
        );
        assert!(old.exists());
        platform.app_test_identity = Some("upgraded-executable".into());
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&rule(id), &files[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
        platform.app_test_identity = Some("isolated-test-identity".into());
        std::fs::write(&old, b"changed since scanning").unwrap();
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&rule(id), &files[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
    }
}

#[test]
fn qq_requires_numeric_profile_marker_and_only_three_cache_leaves() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("isolated-qq".into());
    let base = platform.roaming.join("Tencent/QQ/webkitex_cache");
    for profile in ["12345", "67890", "not-an-account"] {
        if profile != "67890" {
            write(&base.join(profile).join("LocalPrefs.json"), false);
        }
        for leaf in [
            "Cache/Cache_Data",
            "Code Cache",
            "GPUCache",
            "Local Storage",
            "Network",
            "Session Storage",
        ] {
            write(&base.join(profile).join(leaf).join("test-data"), true);
        }
    }
    for id in ["qq.http", "qq.code", "qq.gpu"] {
        let mut files = vec![];
        platform
            .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| {
                if let ScanEvent::File(f) = e {
                    files.push(f);
                }
            })
            .unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].path_display.contains("12345"));
        platform.validate_candidate(&rule(id), &files[0]).unwrap();
        assert_eq!(
            error_reason(&platform.delete(&rule(id), &files[0], &[]).unwrap_err()),
            ReasonCode::ValidationPending
        );
        let marker = base.join("12345/LocalPrefs.json");
        std::fs::rename(&marker, marker.with_extension("saved")).unwrap();
        assert!(platform.validate_candidate(&rule(id), &files[0]).is_err());
        std::fs::rename(marker.with_extension("saved"), &marker).unwrap();
    }
    let denied = Denied::new(&base.join("12345/LocalPrefs.json"));
    let mut issues = vec![];
    let roots = crate::application_layouts::inspect(
        &platform.local,
        &platform.roaming,
        dustaway_core::applications::rule("qq.http").unwrap(),
        None,
        &mut |r| issues.push(r),
    )
    .unwrap();
    assert!(roots.roots.is_empty());
    assert!(issues.contains(&ReasonCode::AccessDenied));
    drop(denied);
}

fn office_profiles(platform: &WindowsPlatform) -> Vec<PathBuf> {
    let mut profiles = vec![];
    for account in ["fixture-one", "fixture-two"] {
        for kind in ["profile_main", "profile_explorer"] {
            profiles.push(
                platform
                    .roaming
                    .join("LarkShell/aha/users")
                    .join(account)
                    .join(kind),
            );
        }
    }
    for suffix in ["133", "134"] {
        profiles.push(platform.local.join(format!("DingTalk_{suffix}/Default")));
    }
    for profile in &profiles {
        write(&profile.join("Preferences"), false);
        for leaf in [
            "Cache/Cache_Data",
            "Code Cache",
            "GPUCache",
            "Local Storage",
            "IndexedDB",
            "Network",
            "Session Storage",
            "mail",
            "documents",
            "backup",
        ] {
            write(&profile.join(leaf).join("fixture-data"), true);
        }
    }
    // No marker, an unknown profile name, a nested account and a nonnumeric DingTalk suffix.
    for profile in [
        platform
            .roaming
            .join("LarkShell/aha/users/no-marker/profile_main"),
        platform
            .roaming
            .join("LarkShell/aha/users/fixture-one/profile_other"),
        platform
            .roaming
            .join("LarkShell/aha/users/nested/account/profile_main"),
        platform.local.join("DingTalk_invalid/Default"),
    ] {
        write(&profile.join("Cache/Cache_Data/fixture-data"), true);
    }
    profiles
}

#[test]
fn office_rules_discover_multiple_profiles_but_never_protected_siblings_or_cleanup() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("isolated-office-build".into());
    let profiles = office_profiles(&platform);
    for id in [
        "feishu.http",
        "feishu.code",
        "feishu.gpu",
        "dingtalk.http",
        "dingtalk.code",
    ] {
        let descriptor = rule(id);
        let mut files = vec![];
        platform
            .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
                if let ScanEvent::File(file) = e {
                    files.push(file);
                }
            })
            .unwrap();
        assert_eq!(files.len(), if id.starts_with("feishu") { 4 } else { 2 });
        for file in &files {
            assert!(file.path_display.ends_with("fixture-data"));
            assert!(
                file.path_display
                    .contains(&descriptor.relative.replace('/', "\\"))
            );
            platform.validate_candidate(&descriptor, file).unwrap();
            // A disabled descriptor stays read-only even after an independent
            // real-version acceptance is added to the compiled registry.
            let mut readonly = descriptor.clone();
            readonly.cleanup_validated = false;
            assert_eq!(
                error_reason(&platform.delete(&readonly, file, &[]).unwrap_err()),
                ReasonCode::ValidationPending
            );
            let mut forged = descriptor.clone();
            forged.cleanup_validated = true;
            if matches!(id, "dingtalk.http" | "dingtalk.code") {
                assert_eq!(
                    dustaway_core::cleanup_acceptance::release_block(&forged),
                    None
                );
            } else {
                assert_eq!(
                    error_reason(&platform.delete(&forged, file, &[]).unwrap_err()),
                    ReasonCode::ValidationPending
                );
            }
        }
        let excluded = platform.candidate_root(&files[0]).unwrap();
        let mut kept = 0;
        let mut remaining = 0;
        platform
            .scan(
                &descriptor,
                &[excluded],
                &AtomicBool::new(false),
                &mut |e| match e {
                    ScanEvent::File(_) => remaining += 1,
                    ScanEvent::Retained(ReasonCode::Excluded) => kept += 1,
                    _ => {}
                },
            )
            .unwrap();
        assert_eq!(remaining + 1, files.len());
        assert_eq!(kept, 1);
        platform.app_test_identity = Some("upgraded-office-build".into());
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&descriptor, &files[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
        platform.app_test_identity = Some("isolated-office-build".into());
    }
    for profile in profiles {
        for leaf in [
            "Local Storage",
            "IndexedDB",
            "Network",
            "Session Storage",
            "mail",
            "documents",
            "backup",
        ] {
            assert_eq!(
                std::fs::read(profile.join(leaf).join("fixture-data")).unwrap(),
                b"protected fixture contents"
            );
        }
    }
}

#[test]
fn office_layout_marker_changes_permissions_links_and_cancellation_are_respected() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("isolated-office-build".into());
    let profiles = office_profiles(&platform);
    let descriptor = rule("feishu.http");
    let mut files = vec![];
    platform
        .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
            if let ScanEvent::File(file) = e {
                files.push(file);
            }
        })
        .unwrap();
    let marker = profiles[0].join("Preferences");
    std::fs::rename(&marker, marker.with_extension("saved")).unwrap();
    let affected = files
        .iter()
        .find(|f| f.path_display.contains("fixture-one\\profile_main"))
        .unwrap();
    assert_eq!(
        error_reason(
            &platform
                .validate_candidate(&descriptor, affected)
                .unwrap_err()
        ),
        ReasonCode::Changed
    );
    std::fs::rename(marker.with_extension("saved"), &marker).unwrap();
    let denied = Denied::new(&marker);
    let mut issues = vec![];
    let mut count = 0;
    platform
        .scan(
            &descriptor,
            &[],
            &AtomicBool::new(false),
            &mut |e| match e {
                ScanEvent::CoverageIssue(code) => issues.push(code),
                ScanEvent::File(_) => count += 1,
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(count, 3);
    assert!(issues.contains(&ReasonCode::AccessDenied));
    drop(denied);
    let cached = profiles[0].join("Cache/Cache_Data/fixture-data");
    std::fs::hard_link(&cached, profiles[0].join("shared-copy")).unwrap();
    let mut retained = vec![];
    platform
        .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
            if let ScanEvent::Retained(code) | ScanEvent::RetainedFile { reason: code, .. } = e {
                retained.push(code);
            }
        })
        .unwrap();
    assert!(retained.contains(&ReasonCode::LinkOrCloud));
    let mut emitted = 0;
    platform
        .scan(&descriptor, &[], &AtomicBool::new(true), &mut |_| {
            emitted += 1
        })
        .unwrap();
    assert_eq!(emitted, 0);
}

#[test]
fn office_missing_location_is_distinct_from_unsupported_profile_and_full_zero_scan() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("isolated-office-build".into());
    let mut notices = vec![];
    platform
        .scan(
            &rule("feishu.http"),
            &[],
            &AtomicBool::new(false),
            &mut |e| {
                if let ScanEvent::NoticeCode(code) = e {
                    notices.push(code);
                }
            },
        )
        .unwrap();
    assert_eq!(notices, vec![ReasonCode::LocationMissing]);
    write(
        &platform
            .roaming
            .join("LarkShell/aha/users/unmarked/profile_main/Cache/Cache_Data/data"),
        true,
    );
    notices.clear();
    platform
        .scan(
            &rule("feishu.http"),
            &[],
            &AtomicBool::new(false),
            &mut |e| {
                if let ScanEvent::NoticeCode(code) = e {
                    notices.push(code);
                }
            },
        )
        .unwrap();
    assert_eq!(notices, vec![ReasonCode::UnsupportedLayout]);
}

#[test]
fn office_profile_parent_replacement_and_junction_cannot_reuse_candidates() {
    for id in ["feishu.http", "dingtalk.http"] {
        let (temp, mut platform) = fixture();
        platform.app_test_identity = Some("isolated-office-build".into());
        let profiles = office_profiles(&platform);
        let profile = if id.starts_with("feishu") {
            &profiles[0]
        } else {
            &profiles[4]
        };
        let descriptor = rule(id);
        let mut files = vec![];
        platform
            .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
                if let ScanEvent::File(file) = e {
                    files.push(file);
                }
            })
            .unwrap();
        let affected = files
            .iter()
            .find(|file| {
                file.path_display.replace('/', "\\")
                    == display_path(&profile.join("Cache/Cache_Data/fixture-data"))
                        .replace('/', "\\")
            })
            .unwrap();
        let saved = profile.with_extension("saved");
        // All moves are within the owned tempfile tree. Preserve the file/root identity,
        // but replace their profile ancestor so only chain revalidation can reject it.
        assert!(
            extended(profile)
                .unwrap()
                .starts_with(extended(temp.path()).unwrap())
        );
        assert!(
            extended(&saved)
                .unwrap()
                .starts_with(extended(temp.path()).unwrap())
        );
        std::fs::rename(profile, &saved).unwrap();
        write(&profile.join("Preferences"), false);
        std::fs::rename(saved.join("Cache"), profile.join("Cache")).unwrap();
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&descriptor, affected)
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );

        let parked = profile.with_extension("replaced");
        assert!(
            extended(&parked)
                .unwrap()
                .starts_with(extended(temp.path()).unwrap())
        );
        std::fs::rename(profile, &parked).unwrap();
        write(&saved.join("Cache/Cache_Data/outside-candidate"), true);
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(display_path(profile).replace('/', "\\"))
            .arg(display_path(&saved).replace('/', "\\"))
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut after = vec![];
        platform
            .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
                if let ScanEvent::File(file) = e {
                    after.push(file);
                }
            })
            .unwrap();
        assert_eq!(after.len() + 1, files.len());
        assert!(
            after
                .iter()
                .all(|f| !f.path_display.contains("outside-candidate"))
        );
        assert!(platform.validate_candidate(&descriptor, affected).is_err());
        std::fs::remove_dir(profile).unwrap(); // Remove only the fixture junction, never its target.
        assert!(saved.join("Cache/Cache_Data/outside-candidate").exists());
    }
}

#[test]
fn thirty_day_cutoff_keeps_recent_and_future_dump_files() {
    let (temp, _) = fixture();
    let path = temp.path().join("crash.dmp");
    write(&path, false);
    let cutoff = now() - 30 * 86400;
    for (time, accepted) in [
        (cutoff - 1, true),
        (cutoff, true),
        (cutoff + 1, false),
        (now() + 86400, false),
    ] {
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(time, 0)).unwrap();
        assert_eq!(
            snapshot_at(temp.path(), &path, cutoff, None).is_ok(),
            accepted
        );
    }
}
pub(super) struct Denied(PathBuf);
impl Denied {
    pub(super) fn new(path: &Path) -> Self {
        assert!(
            std::process::Command::new("icacls.exe")
                .arg(path)
                .args(["/deny", "*S-1-1-0:(R)"])
                .creation_flags(0x08000000)
                .output()
                .unwrap()
                .status
                .success()
        );
        Self(path.into())
    }
}
impl Drop for Denied {
    fn drop(&mut self) {
        assert!(
            std::process::Command::new("icacls.exe")
                .arg(&self.0)
                .args(["/remove:d", "*S-1-1-0"])
                .creation_flags(0x08000000)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}

#[test]
fn prefilter_is_retention_only_and_uses_full_validation_at_the_age_boundary() {
    let cutoff = now() - 7 * 86400;
    let ticks = |time: i64| (time + 11_644_473_600) as u64 * 10_000_000;
    assert_eq!(prefilter(0, ticks(cutoff), 7, cutoff), None);
    assert_eq!(prefilter(0, ticks(cutoff) + 9_999_999, 7, cutoff), None);
    assert_eq!(
        prefilter(0, ticks(cutoff + 1), 7, cutoff),
        Some(ReasonCode::Recent)
    );
    assert_eq!(
        prefilter(0, ticks(now() + 86400), 7, cutoff),
        Some(ReasonCode::Recent)
    );
    assert_eq!(prefilter(0, 0, 7, cutoff), None);
    assert_eq!(prefilter(0, ticks(now() + 86400), 0, now()), None);
    assert_eq!(
        prefilter(0x1000, ticks(now()), 7, cutoff),
        Some(ReasonCode::LinkOrCloud)
    );
    assert_eq!(
        prefilter(1, ticks(now()), 7, cutoff),
        Some(ReasonCode::Protected)
    );
    let (temp, _) = fixture();
    let file = temp.path().join("stale-metadata");
    write(&file, false);
    // A stale enumeration says old; current handle metadata must still refuse it.
    assert_eq!(prefilter(0, ticks(cutoff - 1), 7, cutoff), None);
    assert_eq!(
        error_reason(&snapshot_at(temp.path(), &file, cutoff, None).unwrap_err()),
        ReasonCode::Recent
    );
    filetime::set_file_mtime(&file, filetime::FileTime::from_unix_time(cutoff, 0)).unwrap();
    assert!(snapshot_at(temp.path(), &file, cutoff, None).is_ok());
}

#[test]
fn recent_locked_files_are_retained_without_relaxing_old_file_validation() {
    let (_temp, platform) = fixture();
    let cache = platform.local.join("Temp");
    for (name, old) in [
        ("近期.tmp", false),
        ("旧占用.tmp", true),
        ("old.tmp", true),
        ("linked.tmp", true),
        ("offline.tmp", true),
    ] {
        write(&cache.join(name), old);
    }
    std::fs::hard_link(
        cache.join("linked.tmp"),
        platform.local.join("personal-link"),
    )
    .unwrap();
    let recent = std::fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(cache.join("近期.tmp"))
        .unwrap();
    let old = std::fs::OpenOptions::new()
        .write(true)
        .share_mode(0)
        .open(cache.join("旧占用.tmp"))
        .unwrap();
    let offline = cache.join("offline.tmp");
    let before = std::fs::metadata(&offline).unwrap().modified().unwrap();
    unsafe {
        windows::Win32::Storage::FileSystem::SetFileAttributesW(
            windows::core::PCWSTR(wide(&offline).as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_OFFLINE,
        )
        .unwrap();
    }
    let mut files = vec![];
    let mut reasons = vec![];
    let result = platform.scan(
        &rule("windows.user_temp"),
        &[],
        &AtomicBool::new(false),
        &mut |e| match e {
            ScanEvent::File(f) => files.push(f),
            ScanEvent::Retained(r) | ScanEvent::RetainedFile { reason: r, .. } => reasons.push(r),
            _ => {}
        },
    );
    unsafe {
        windows::Win32::Storage::FileSystem::SetFileAttributesW(
            windows::core::PCWSTR(wide(&offline).as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        )
        .unwrap();
    }
    result.unwrap();
    assert_eq!(files.len(), 2);
    assert!(files.iter().any(|f| f.path_display.ends_with("old.tmp")));
    assert!(reasons.contains(&ReasonCode::Recent));
    // FILE_READ_ATTRIBUTES can succeed while a data writer holds the file.
    // Deletion must still refuse that writer; prefilter never authorizes deletion.
    let occupied = files
        .iter()
        .find(|f| f.path_display.ends_with("旧占用.tmp"))
        .unwrap();
    let saved: Snapshot = serde_json::from_str(&occupied.opaque).unwrap();
    assert_eq!(
        error_reason(&delete_snapshot(&saved, 7, &[]).unwrap_err()),
        ReasonCode::InUse
    );
    assert_eq!(
        reasons
            .iter()
            .filter(|r| **r == ReasonCode::LinkOrCloud)
            .count(),
        2
    );
    drop((recent, old));
    assert_eq!(
        std::fs::metadata(&offline).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(
        std::fs::read(&offline).unwrap(),
        b"protected fixture contents"
    );
}

#[test]
fn inaccessible_subtree_is_partial_and_all_inaccessible_roots_are_blocked() {
    let (temp, platform) = fixture();
    let cache = platform.local.join("Temp");
    let protected = cache.join("denied/保留.txt");
    write(&protected, true);
    write(&cache.join("old.tmp"), true);
    let before = std::fs::metadata(&protected).unwrap().modified().unwrap();
    let denied = Denied::new(protected.parent().unwrap());
    let engine = Engine::open(
        Arc::new(platform),
        Scope::User,
        &temp.path().join("journal"),
    )
    .unwrap();
    let session = scan(&engine);
    assert_eq!(engine.summary().rules[0].phase, ScanPhase::Partial);
    assert_eq!(engine.summary().rules[0].files, 1);
    let plan = engine.prepare(&session, selection()).unwrap();
    engine.execute(&plan.id).unwrap();
    wait(&engine);
    assert_eq!(engine.report().unwrap().deleted, 1);
    drop(denied);
    assert_eq!(
        std::fs::metadata(&protected).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(
        std::fs::read(&protected).unwrap(),
        b"protected fixture contents"
    );
    let denied_root = Denied::new(&cache);
    let session = scan(&engine);
    assert_eq!(engine.summary().rules[0].phase, ScanPhase::Failed);
    assert!(engine.prepare(&session, selection()).is_err());
    drop(denied_root);
}

#[test]
fn unreadable_profile_marker_is_not_reported_as_unsupported_layout() {
    let (_temp, platform) = fixture();
    let base = platform.local.join(
        dustaway_core::browsers::chromium("brave")
            .unwrap()
            .user_data,
    );
    write(&base.join("Default/Preferences"), false);
    write(&base.join("Default/Cache/Cache_Data/cache"), false);
    let denied_marker = Denied::new(&base.join("Default/Preferences"));
    let mut issues = vec![];
    let mut unsupported = false;
    let outcome = platform.scan(
        &rule("brave.http"),
        &[],
        &AtomicBool::new(false),
        &mut |e| match e {
            ScanEvent::CoverageIssue(code) => issues.push(code),
            ScanEvent::NoticeCode(ReasonCode::UnsupportedLayout) => unsupported = true,
            _ => {}
        },
    );
    assert!(outcome.is_err());
    assert!(!unsupported);
    assert_eq!(issues, vec![ReasonCode::AccessDenied]);
    write(&base.join("Profile 2/Preferences"), false);
    write(&base.join("Profile 2/Cache/Cache_Data/cache"), false);
    let mut count = 0;
    platform
        .scan(
            &rule("brave.http"),
            &[],
            &AtomicBool::new(false),
            &mut |e| {
                if matches!(e, ScanEvent::File(_)) {
                    count += 1;
                }
            },
        )
        .unwrap();
    assert_eq!(count, 1);
    drop(denied_marker);
}

#[test]
fn chromium_profiles_keep_readable_caches_when_other_profiles_are_missing_or_denied() {
    for family in ["edge", "chrome", "brave", "vivaldi"] {
        let (_temp, platform) = fixture();
        let base = platform
            .local
            .join(dustaway_core::browsers::chromium(family).unwrap().user_data);
        for profile in ["Default", "Profile 1", "Profile 2"] {
            write(&base.join(profile).join("Preferences"), false);
        }
        write(&base.join("Default/Cache/Cache_Data/cache"), false);
        write(&base.join("Profile 2/Cache/Cache_Data/denied-cache"), false);
        let denied = Denied::new(&base.join("Profile 2/Preferences"));
        let mut files = vec![];
        let mut issues = vec![];
        let mut notices = vec![];
        platform
            .scan(
                &rule(&format!("{family}.http")),
                &[],
                &AtomicBool::new(false),
                &mut |event| match event {
                    ScanEvent::File(file) => files.push(file),
                    ScanEvent::CoverageIssue(code) => issues.push(code),
                    ScanEvent::NoticeCode(code) => notices.push(code),
                    _ => {}
                },
            )
            .unwrap();
        assert_eq!(files.len(), 1, "{family}");
        assert_eq!(issues, vec![ReasonCode::AccessDenied], "{family}");
        assert!(
            !notices.contains(&ReasonCode::LocationMissing),
            "{family}: {notices:?}"
        );
        drop(denied);
    }
}

#[test]
fn firefox_bom_duplicate_and_partial_profiles_preserve_valid_cache() {
    let (_temp, platform) = fixture();
    let base = platform.roaming.join("Mozilla/Firefox");
    write(&base.join("Profiles/good/prefs.js"), false);
    write(&base.join("Profiles/denied/prefs.js"), false);
    write(&base.join("Profiles/empty/prefs.js"), false);
    write(&base.join("Profiles/not-a-profile/prefs.js"), false);
    std::fs::write(base.join("profiles.ini"), "\u{feff}[Profile0]\nIsRelative=1\nPath=Profiles/good\n[Profile1]\nIsRelative=1\nPath=Profiles/GOOD\n[Profile2]\nIsRelative=1\nPath=Profiles/denied\n[Profile3]\nIsRelative=1\nPath=Profiles/empty\n[ProfileFake]\nIsRelative=1\nPath=Profiles/not-a-profile\n[Profile4]\nIsRelative=0\nPath=C:/outside\n").unwrap();
    let cache = platform
        .local
        .join("Mozilla/Firefox/Profiles/good/cache2/cache");
    write(&cache, false);
    let denied = Denied::new(&base.join("Profiles/denied/prefs.js"));
    let mut issues = vec![];
    let profiles =
        crate::profile_locations::firefox(&platform.roaming, None, &mut |code| issues.push(code))
            .unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(issues, vec![ReasonCode::AccessDenied]);
    let mut files = 0;
    let mut missing = false;
    platform
        .scan(
            &rule("firefox.http"),
            &[],
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::File(_) => files += 1,
                ScanEvent::NoticeCode(ReasonCode::LocationMissing) => missing = true,
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(files, 1);
    assert!(!missing);
    drop(denied);
}

#[test]
fn compiled_exclusions_preserve_component_boundaries_and_cancel_releases_handles() {
    let (temp, platform) = fixture();
    let cache = platform.local.join("Temp");
    write(&cache.join("保留/file"), true);
    write(&cache.join("保留-other/file"), true);
    let exclusions = vec![
        display_path(&cache.join("保留")).to_uppercase(),
        "relative-invalid".into(),
    ];
    let compiled = CompiledExclusions::new(&exclusions);
    assert!(compiled.contains(&cache.join("保留/file")));
    assert!(!compiled.contains(&cache.join("保留-other/file")));
    let cancel = AtomicBool::new(false);
    let mut candidates = 0;
    platform
        .scan(&rule("windows.user_temp"), &exclusions, &cancel, &mut |e| {
            if matches!(e, ScanEvent::Checked) {
                cancel.store(true, Ordering::Release);
            }
            if matches!(e, ScanEvent::File(_)) {
                candidates += 1;
            }
        })
        .unwrap();
    assert_eq!(candidates, 0);
    assert!(
        PinnedPath::open_cancellable(&cache, true, false, Some(&cancel))
            .err()
            .unwrap()
            .is::<ScanCancelled>()
    );
    std::fs::rename(&cache, temp.path().join("renamed")).unwrap();
}
