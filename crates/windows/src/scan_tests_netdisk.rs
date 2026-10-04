//! Only owned tempfile fixtures are mutated. Application identity is a cfg(test) stub.
use super::*;
use dustaway_core::applications::{APPLICATION_RULES, ProfileLayout};

fn files(
    platform: &WindowsPlatform,
    descriptor: &RuleDescriptor,
) -> (Vec<FileRecord>, Vec<ReasonCode>) {
    let mut found = vec![];
    let mut issues = vec![];
    let result = platform.scan(descriptor, &[], &AtomicBool::new(false), &mut |e| match e {
        ScanEvent::File(file) => found.push(file),
        ScanEvent::CoverageIssue(code) | ScanEvent::NoticeCode(code) => issues.push(code),
        _ => {}
    });
    if result.is_err() {
        assert!(!issues.is_empty(), "scan failed without a coverage issue");
    }
    (found, issues)
}

#[test]
fn all_seven_netdisk_rules_require_markers_exclude_persistent_data_and_refuse_delete() {
    let specs: Vec<_> = APPLICATION_RULES
        .iter()
        .filter(|s| s.profiles == ProfileLayout::ChromiumRoot)
        .collect();
    assert_eq!(specs.len(), 7);
    for spec in specs {
        let (_temp, mut platform) = fixture();
        platform.app_test_identity = Some("netdisk-fixture-v1".into());
        let descriptor = rule(spec.id);
        let profile = platform.roaming.join(spec.base);
        let target = profile.join(spec.relative).join("candidate");
        write(&target, false); // No age limit.
        assert_eq!(
            files(&platform, &descriptor).1,
            vec![ReasonCode::UnsupportedLayout]
        );
        let marker = profile.join("Preferences");
        std::fs::create_dir(&marker).unwrap();
        assert!(files(&platform, &descriptor).0.is_empty());
        std::fs::remove_dir(&marker).unwrap();
        write(&marker, false);
        let protected: Vec<_> = [
            "Cache/outside-data",
            "Local Storage/state",
            "IndexedDB/state",
            "databases/state",
            "Network/Cookies",
            "Session Storage/state",
            "Storage/offline",
            "downloads/document",
            "download.xltd",
            "download.cfg",
            "backup/document",
            "TaskDb.dat",
        ]
        .iter()
        .map(|p| profile.join(p))
        .collect();
        for p in &protected {
            write(p, false);
        }
        let (found, issues) = files(&platform, &descriptor);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(found.len(), 1, "{}", spec.id);
        platform.validate_candidate(&descriptor, &found[0]).unwrap();
        for forged in [false, true] {
            let mut request = descriptor.clone();
            request.cleanup_validated = forged;
            assert_eq!(
                error_reason(&platform.delete(&request, &found[0], &[]).unwrap_err()),
                ReasonCode::ValidationPending
            );
        }
        let mut candidates = 0;
        let mut retained = 0;
        platform
            .scan(
                &descriptor,
                &[display_path(&profile)],
                &AtomicBool::new(false),
                &mut |e| match e {
                    ScanEvent::File(_) => candidates += 1,
                    ScanEvent::Retained(ReasonCode::Excluded) => retained += 1,
                    _ => {}
                },
            )
            .unwrap();
        assert_eq!(candidates, 0);
        assert!(retained > 0);
        platform
            .scan(&descriptor, &[], &AtomicBool::new(true), &mut |_| {
                panic!("cancelled scan emitted")
            })
            .unwrap();
        platform.app_test_identity = Some("netdisk-fixture-v2".into());
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&descriptor, &found[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
        platform.app_test_identity = Some("netdisk-fixture-v1".into());
        std::fs::rename(&marker, profile.join("Preferences.saved")).unwrap();
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&descriptor, &found[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
        for p in protected {
            assert_eq!(std::fs::read(p).unwrap(), b"protected fixture contents");
        }
        assert!(target.exists());
    }
}

#[test]
fn netdisk_inaccessible_markers_and_hardlinks_do_not_become_complete_zero_results() {
    for app in ["baidu_netdisk", "quark_netdisk", "thunder"] {
        let (_temp, mut platform) = fixture();
        platform.app_test_identity = Some("netdisk-fixture-v1".into());
        let descriptor = rule(&format!("{app}.code"));
        let spec = dustaway_core::applications::rule(&descriptor.id).unwrap();
        let profile = platform.roaming.join(spec.base);
        let marker = profile.join("Preferences");
        let target = profile.join("Code Cache/candidate");
        write(&marker, false);
        write(&target, false);
        let denied = Denied::new(&marker);
        let (found, issues) = files(&platform, &descriptor);
        assert!(found.is_empty());
        assert!(issues.contains(&ReasonCode::AccessDenied));
        drop(denied);
        unsafe {
            use windows::Win32::Storage::FileSystem::*;
            SetFileAttributesW(
                windows::core::PCWSTR(wide(&target).as_ptr()),
                FILE_ATTRIBUTE_OFFLINE,
            )
            .unwrap();
        }
        let offline = files(&platform, &descriptor).0;
        unsafe {
            use windows::Win32::Storage::FileSystem::*;
            SetFileAttributesW(
                windows::core::PCWSTR(wide(&target).as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
            )
            .unwrap();
        }
        assert!(offline.is_empty());
        assert_eq!(files(&platform, &descriptor).0.len(), 1);
        std::fs::hard_link(&target, profile.join("retained-shared-copy")).unwrap();
        assert!(files(&platform, &descriptor).0.is_empty());
        assert!(target.exists());
    }
}

#[test]
fn netdisk_profile_replacement_and_junction_invalidate_old_candidates() {
    for app in ["baidu_netdisk", "quark_netdisk", "thunder"] {
        let (temp, mut platform) = fixture();
        platform.app_test_identity = Some("netdisk-fixture-v1".into());
        let descriptor = rule(&format!("{app}.code"));
        let spec = dustaway_core::applications::rule(&descriptor.id).unwrap();
        let profile = platform.roaming.join(spec.base);
        write(&profile.join("Preferences"), false);
        write(&profile.join("Code Cache/candidate"), false);
        let found = files(&platform, &descriptor).0;
        assert_eq!(found.len(), 1);
        let saved = profile.with_extension("saved");
        let parked = profile.with_extension("parked");
        for path in [&profile, &saved, &parked] {
            assert!(
                extended(path)
                    .unwrap()
                    .starts_with(extended(temp.path()).unwrap())
            );
        }
        std::fs::rename(&profile, &saved).unwrap();
        write(&profile.join("Preferences"), false);
        std::fs::rename(saved.join("Code Cache"), profile.join("Code Cache")).unwrap();
        assert_eq!(
            error_reason(
                &platform
                    .validate_candidate(&descriptor, &found[0])
                    .unwrap_err()
            ),
            ReasonCode::Changed
        );
        std::fs::rename(&profile, &parked).unwrap();
        let output = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(display_path(&profile).replace('/', "\\"))
            .arg(display_path(&parked).replace('/', "\\"))
            .creation_flags(0x08000000)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(platform.validate_candidate(&descriptor, &found[0]).is_err());
        // A junction at the fixed configuration root is refused, not traversed.
        assert!(
            platform
                .scan(&descriptor, &[], &AtomicBool::new(false), &mut |e| {
                    assert!(!matches!(e, ScanEvent::File(_)));
                })
                .is_err()
        );
        std::fs::remove_dir(&profile).unwrap(); // Remove only this fixture junction.
        assert!(parked.join("Code Cache/candidate").exists());
    }
}
