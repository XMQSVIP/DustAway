//! Isolated filesystem acceptance through the production scanner and deletion boundary.
//! Only installation identity is injected; this module is absent from release programs.
use super::*;
use crate::app_cache::tests::{cache_bytes, zip_bytes};

fn sample(platform: &WindowsPlatform, id: &str) -> PathBuf {
    let spec = dustaway_core::app_cache::rule(id).unwrap();
    let root = platform.roaming.join(&spec.base).join(&spec.paths[0]);
    let (path, bytes) = if spec.kind == "vsix" {
        (
            root.join("fixture.tool-1.0.0"),
            zip_bytes(
                "extension/package.json",
                br#"{"publisher":"fixture","name":"tool","version":"1.0.0"}"#,
            ),
        )
    } else {
        (
            root.join("b".repeat(40))
                .join("chrome/js/0123456789abcdef_0"),
            cache_bytes(),
        )
    };
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    let old = filetime::FileTime::from_unix_time(now() - 120 * 86400, 0);
    filetime::set_file_mtime(&path, old).unwrap();
    if spec.kind != "vsix" {
        filetime::set_file_mtime(root.join("b".repeat(40)), old).unwrap();
    }
    path
}
fn collect(platform: &WindowsPlatform, id: &str) -> Vec<FileRecord> {
    let mut found = vec![];
    platform
        .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| {
            if let ScanEvent::File(f) = e {
                found.push(f);
            }
        })
        .unwrap();
    found
}
#[test]
fn four_vscode_rules_complete_one_plan_protect_personal_data_and_refuse_replay() {
    for id in [
        "appcache.vscode_vsix",
        "appcache.vscode_code",
        "appcache.vscode_insiders_vsix",
        "appcache.vscode_insiders_code",
    ] {
        let (temp, mut platform) = fixture();
        platform.app_test_identity = Some("isolated-cache-build".into());
        *platform.inventory.lock().unwrap() = Some(crate::app_cache::Inventory {
            entries: vec![],
            complete: true,
        });
        let path = sample(&platform, id);
        let protected = path.parent().unwrap().join("chat.db");
        write(&protected, true);
        let user = platform
            .roaming
            .join("Code/User/globalStorage/conversations.db");
        write(&user, true);
        let recovery = platform.roaming.join("Code/Backups/work.txt");
        write(&recovery, true);
        let bytes = std::fs::read(&protected).unwrap();
        let engine = Engine::open(
            Arc::new(platform),
            Scope::User,
            &temp.path().join("journal"),
        )
        .unwrap();
        let session = engine.start_scan(vec![id.into()]).unwrap();
        wait(&engine);
        assert_eq!(engine.summary().rules[0].files, 1, "{id}");
        let plan = engine
            .prepare(
                &session,
                Selection {
                    rule_ids: vec![id.into()],
                    reviewed_rule_ids: vec![id.into()],
                    ..Selection::default()
                },
            )
            .unwrap();
        assert!(path.exists(), "preparation must not delete");
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert_eq!(engine.report().unwrap().deleted, 1, "{id}");
        assert!(!path.exists());
        assert!(engine.execute(&plan.id).is_err());
        assert_eq!(std::fs::read(&protected).unwrap(), bytes);
        assert!(user.exists() && recovery.exists());
    }
}
#[test]
fn cache_scan_revalidates_installation_file_and_build_directory_changes() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("before".into());
    let id = "appcache.vscode_code";
    let path = sample(&platform, id);
    let files = collect(&platform, id);
    assert_eq!(files.len(), 1);
    platform.app_test_identity = Some("upgraded".into());
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    platform.app_test_identity = Some("before".into());
    let commit = path.parent().unwrap().parent().unwrap().parent().unwrap();
    filetime::set_file_mtime(commit, filetime::FileTime::from_unix_time(now(), 0)).unwrap();
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    assert!(path.exists());
    let id = "appcache.vscode_vsix";
    let path = sample(&platform, id);
    let files = collect(&platform, id);
    std::fs::write(&path, b"changed after scan").unwrap();
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    assert!(path.exists());
}
#[test]
fn new_cache_discovery_respects_volumes_exclusions_and_partial_profiles() {
    let (_temp, platform) = fixture();
    let base = platform.roaming.join("ziniaobrowser/instances");
    let good = base.join("userdata1/Cache/data_0");
    write(&good, true);
    let denied = base.join("userdata2/Cache/data_0");
    write(&denied, true);
    let protected = base.join("userdata1/Local Storage/chat.db");
    write(&protected, true);
    let fake = base.join("personal/Cache/data_0");
    write(&fake, true);
    let guard = Denied::new(denied.parent().unwrap());
    let mut files = vec![];
    let mut issues = 0;
    platform
        .scan(
            &rule("appcache.ziniao"),
            &[],
            &AtomicBool::new(false),
            &mut |e| match e {
                ScanEvent::File(f) => files.push(f),
                ScanEvent::CoverageIssue(_) => issues += 1,
                _ => {}
            },
        )
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(issues > 0);
    drop(guard);
    let excluded = display_path(&base);
    let mut count = 0;
    platform
        .scan(
            &rule("appcache.ziniao"),
            &[excluded],
            &AtomicBool::new(false),
            &mut |e| {
                if matches!(e, ScanEvent::File(_)) {
                    count += 1
                }
            },
        )
        .unwrap();
    assert_eq!(count, 0);
    let other = dustaway_core::VolumeTarget {
        key: "another-disk".into(),
        label: "Z:\\".into(),
        opaque: String::new(),
    };
    platform
        .scan_on_volumes(
            &rule("appcache.ziniao"),
            &[],
            Some(&[other]),
            &AtomicBool::new(false),
            &mut |e| {
                if matches!(e, ScanEvent::File(_)) {
                    count += 1
                }
            },
        )
        .unwrap();
    assert_eq!(count, 0);
    assert!(protected.exists() && fake.exists());
}
#[test]
fn recent_vsix_bytes_are_measured_but_not_candidates_and_links_are_preserved() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("fixture".into());
    let id = "appcache.vscode_vsix";
    let path = sample(&platform, id);
    filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(now(), 0)).unwrap();
    let mut retained = 0;
    let mut candidates = 0;
    platform
        .scan(&rule(id), &[], &AtomicBool::new(false), &mut |e| match e {
            ScanEvent::RetainedFile {
                reason: ReasonCode::Recent,
                bytes,
            } => retained += bytes,
            ScanEvent::File(_) => candidates += 1,
            _ => {}
        })
        .unwrap();
    assert_eq!(candidates, 0);
    assert_eq!(retained, std::fs::metadata(&path).unwrap().len());
    std::fs::hard_link(&path, platform.roaming.join("protected-linked-package")).unwrap();
    assert!(collect(&platform, id).is_empty());
    assert!(path.exists());
}

fn documented_sample(platform: &WindowsPlatform, id: &str) -> PathBuf {
    let spec = dustaway_core::app_cache::rule(id).unwrap();
    let folder = if spec.folder == "local" {
        &platform.local
    } else {
        &platform.roaming
    };
    let base = folder.join(&spec.base);
    let base = if spec.app == "pycharm" {
        base.join("PyCharm2027.2")
    } else {
        base
    };
    let root = base.join(&spec.paths[0]);
    let path = if spec.kind == "documented_gpu" {
        crate::app_cache::tests::gpu_fixture(&root)
    } else {
        crate::app_cache::tests::simple_code_fixture(&root)
    };
    filetime::set_file_mtime(
        &path,
        filetime::FileTime::from_unix_time(now() - 30 * 86400, 0),
    )
    .unwrap();
    path
}

#[test]
fn documented_vendor_caches_complete_cleanup_without_deleting_shared_or_personal_data() {
    for id in [
        "appcache.cherry_gpu",
        "appcache.cherry_code",
        "appcache.pycharm_gpu",
        "appcache.pycharm_code",
    ] {
        let (temp, mut platform) = fixture();
        platform.app_test_identity = Some("verified-fixture-vendor".into());
        let path = documented_sample(&platform, id);
        let spec = dustaway_core::app_cache::rule(id).unwrap();
        let folder = if spec.folder == "local" {
            &platform.local
        } else {
            &platform.roaming
        };
        let base = folder.join(&spec.base);
        let mut protected = vec![
            path.parent().unwrap().join("personal.json"),
            path.parent().unwrap().join("chat.db"),
        ];
        for name in [
            "Data/conversations.db",
            "Local Storage/accounts.db",
            "LocalHistory/history.storageData",
            "caches/content.dat",
            "projects/important.py",
            "Backups/recovery.txt",
        ] {
            protected.push(base.join(name));
        }
        if spec.kind == "documented_gpu" {
            protected.push(path.parent().unwrap().join("f_000001"));
        }
        for file in &protected {
            if !file.exists() {
                write(file, true);
            }
        }
        let before: Vec<_> = protected
            .iter()
            .map(|p| std::fs::read(p).unwrap())
            .collect();
        let other = if spec.app == "cherry" {
            "appcache.cherry"
        } else {
            "appcache.jetbrains"
        };
        let other_candidates = collect(&platform, other);
        assert!(
            !other_candidates
                .iter()
                .any(|f| f.path_display == display_path(&path)),
            "no duplicate capacity: {id}"
        );
        let engine = Engine::open(
            Arc::new(platform),
            Scope::User,
            &temp.path().join("journal"),
        )
        .unwrap();
        let session = engine.start_scan(vec![id.into()]).unwrap();
        wait(&engine);
        assert_eq!(
            engine.summary().rules[0].files,
            if spec.kind == "documented_gpu" { 5 } else { 1 },
            "{id}"
        );
        let selection = Selection {
            rule_ids: vec![id.into()],
            reviewed_rule_ids: vec![id.into()],
            ..Selection::default()
        };
        let plan = engine.prepare(&session, selection).unwrap();
        assert!(path.exists());
        engine.execute(&plan.id).unwrap();
        wait(&engine);
        assert_eq!(
            engine.report().unwrap().deleted,
            if spec.kind == "documented_gpu" { 5 } else { 1 },
            "{id}"
        );
        assert!(!path.exists());
        assert!(engine.execute(&plan.id).is_err());
        for (file, bytes) in protected.iter().zip(before) {
            assert_eq!(std::fs::read(file).unwrap(), bytes);
        }
    }
}

#[test]
fn documented_vendor_cache_rechecks_membership_identity_age_exclusions_and_links() {
    let (_temp, mut platform) = fixture();
    platform.app_test_identity = Some("before".into());
    let id = "appcache.cherry_gpu";
    let path = documented_sample(&platform, id);
    let files = collect(&platform, id);
    assert_eq!(files.len(), 5);
    let files: Vec<_> = files
        .into_iter()
        .filter(|f| f.path_display == display_path(&path))
        .collect();
    platform.app_test_identity = Some("updated".into());
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    platform.app_test_identity = Some("before".into());
    assert!(
        platform
            .delete(
                &rule(id),
                &files[0],
                &[display_path(path.parent().unwrap())]
            )
            .is_err()
    );
    let index = path.clone();
    let bytes = std::fs::read(&index).unwrap();
    std::fs::write(&index, b"changed index after scan").unwrap();
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    std::fs::write(&index, bytes).unwrap();
    filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(now() + 86400, 0)).unwrap();
    assert!(
        !collect(&platform, id)
            .iter()
            .any(|f| f.path_display == display_path(&path))
    );
    assert!(platform.delete(&rule(id), &files[0], &[]).is_err());
    filetime::set_file_mtime(
        &path,
        filetime::FileTime::from_unix_time(now() - 30 * 86400, 0),
    )
    .unwrap();
    std::fs::hard_link(&path, platform.roaming.join("linked-personal-data")).unwrap();
    assert!(
        !collect(&platform, id)
            .iter()
            .any(|f| f.path_display == display_path(&path))
    );
    assert!(path.exists());
}
