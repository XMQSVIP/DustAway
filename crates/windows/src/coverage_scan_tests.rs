//! Fixed read-only observations (0.22, 0.23 and 0.24): no real application data and no acceptance override.
use super::*;

fn specs() -> impl Iterator<Item = &'static dustaway_core::app_cache::CacheSpec> {
    dustaway_core::app_cache::catalog()
        .iter()
        .filter(|s| s.kind == "readonly_observed" && s.folder != "discovered")
}
fn roots(platform: &WindowsPlatform, spec: &dustaway_core::app_cache::CacheSpec) -> Vec<PathBuf> {
    let folder = if spec.folder == "local" {
        &platform.local
    } else {
        &platform.roaming
    };
    spec.paths
        .iter()
        .map(|p| folder.join(&spec.base).join(p))
        .collect()
}
fn events(
    platform: &WindowsPlatform,
    id: &str,
    excluded: &[String],
    cancel: &AtomicBool,
) -> (Result<()>, Vec<ScanEvent>) {
    let mut items = Vec::new();
    let result = platform.scan(&rule(id), excluded, cancel, &mut |e| items.push(e));
    (result, items)
}
fn count(items: &[ScanEvent]) -> (usize, u64) {
    items.iter().fold((0, 0), |(n, b), event| match event {
        ScanEvent::File(f) => (n + 1, b + f.bytes),
        ScanEvent::RetainedFile { bytes, .. } => (n + 1, b + bytes),
        _ => (n, b),
    })
}

#[test]
fn observed_rules_cover_only_fixed_leaves_and_refuse_all_plans() {
    assert_eq!(specs().count(), 26);
    for spec in specs() {
        let (temp, platform) = fixture();
        let paths = roots(&platform, spec);
        let mut protected = vec![];
        for root in &paths {
            write(&root.join("data_0"), false);
            write(&root.join("not-a-cache-format.private"), true);
            write(&root.join("recent.log"), false);
            let near = root
                .with_file_name(format!(
                    "{}-personal",
                    root.file_name().unwrap().to_string_lossy()
                ))
                .join("data_0");
            write(&near, true);
            protected.push(near);
        }
        let folder = if spec.folder == "local" {
            &platform.local
        } else {
            &platform.roaming
        };
        for relative in [
            "Local Storage/chat.db",
            "artifact-sessions/work",
            "backup/document",
            "tmp/backup/restore",
            "users/account",
            "update/package.zip",
            "projects/source",
            "Data/private",
            "attachments/private",
            "backups/private",
            "generated-images/private",
            "extension/private",
            "Profile 1/Cache/data_0",
            "Default/Service Worker/private",
            "Default/Network/Cookies",
            "Default/Local Storage/chat.db",
            "Default/IndexedDB/chat",
            "Default/Extensions/extension",
            "Downloads/file",
            "runtimes/bin",
        ] {
            let path = folder.join(&spec.base).join(relative);
            write(&path, true);
            protected.push(path);
        }
        if matches!(
            spec.id.as_str(),
            "appcache.todesk_web" | "appcache.webtoapi_web" | "appcache.dbx_web"
        ) {
            let sibling = folder.join(&spec.base).join("GraphiteDawnCache/private");
            write(&sibling, true);
            protected.push(sibling);
        }
        let expected = paths.len() * 3 * b"protected fixture contents".len();
        let (result, found) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        result.unwrap();
        assert_eq!(
            count(&found),
            (paths.len() * 3, expected as u64),
            "{}",
            spec.id
        );
        assert!(found.iter().any(|e| matches!(e,ScanEvent::ApplicationObserved(a) if a.identity_status=="unverified_data")));
        let files: Vec<_> = found
            .iter()
            .filter_map(|e| {
                if let ScanEvent::File(f) = e {
                    Some(f.clone())
                } else {
                    None
                }
            })
            .collect();
        assert!(!files.is_empty());
        for file in &files {
            assert!(platform.delete(&spec.descriptor(), file, &[]).is_err());
        }
        let engine = Engine::open(
            Arc::new(platform),
            Scope::User,
            &temp.path().join("journal"),
        )
        .unwrap();
        let session = engine.start_scan(vec![spec.id.clone()]).unwrap();
        wait(&engine);
        for selection in [
            Selection {
                rule_ids: vec![spec.id.clone()],
                reviewed_rule_ids: vec![spec.id.clone()],
                ..Default::default()
            },
            Selection {
                included_ids: engine
                    .candidates(&session, &spec.id, 0, 200)
                    .unwrap()
                    .items
                    .iter()
                    .map(|f| f.id.clone())
                    .collect(),
                reviewed_rule_ids: vec![spec.id.clone()],
                ..Default::default()
            },
        ] {
            assert!(engine.prepare(&session, selection).is_err());
        }
        for path in protected {
            assert_eq!(std::fs::read(path).unwrap(), b"protected fixture contents");
        }
    }
}

#[test]
fn observed_rules_distinguish_empty_missing_denied_cancelled_and_excluded() {
    for spec in specs() {
        let (_temp, platform) = fixture();
        let root = roots(&platform, spec).remove(0);
        let (_, missing) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        assert!(
            missing
                .iter()
                .any(|e| matches!(e, ScanEvent::NoticeCode(ReasonCode::LocationMissing)))
        );
        std::fs::create_dir_all(&root).unwrap();
        let (result, empty) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        result.unwrap();
        assert_eq!(count(&empty), (0, 0));
        assert!(
            !empty
                .iter()
                .any(|e| matches!(e, ScanEvent::NoticeCode(ReasonCode::LocationMissing)))
        );
        let file = root.join("data_0");
        write(&file, false);
        let (_, cancelled) = events(&platform, &spec.id, &[], &AtomicBool::new(true));
        assert_eq!(count(&cancelled), (0, 0));
        let (_, excluded) = events(
            &platform,
            &spec.id,
            &[display_path(&root)],
            &AtomicBool::new(false),
        );
        assert!(
            excluded
                .iter()
                .any(|e| matches!(e, ScanEvent::Retained(ReasonCode::Excluded)))
        );
        assert!(!excluded.iter().any(|e| matches!(e, ScanEvent::File(_))));
        let guard = Denied::new(&root);
        let (result, denied) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        assert!(result.is_err());
        assert!(
            denied
                .iter()
                .any(|e| matches!(e, ScanEvent::CoverageIssue(_)))
        );
        drop(guard);
        assert!(file.exists());
    }
}

#[test]
fn observed_files_count_hard_links_once_and_do_not_follow_junctions() {
    for spec in specs() {
        let (temp, platform) = fixture();
        let paths = roots(&platform, spec);
        let root = &paths[0];
        let original = root.join("data_0");
        write(&original, false);
        std::fs::hard_link(&original, root.join("data_1")).unwrap();
        if paths.len() > 1 {
            std::fs::create_dir_all(&paths[1]).unwrap();
            std::fs::hard_link(&original, paths[1].join("data_0")).unwrap();
        }
        let (result, found) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        result.unwrap();
        assert_eq!(
            count(&found),
            (1, b"protected fixture contents".len() as u64)
        );
        let outside = temp.path().join("outside");
        write(&outside.join("data_0"), true);
        let link = root.join("linked");
        assert!(
            std::process::Command::new("cmd.exe")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(display_path(&link).replace('/', "\\"))
                .arg(display_path(&outside).replace('/', "\\"))
                .creation_flags(0x08000000)
                .output()
                .unwrap()
                .status
                .success()
        );
        let (result, found) = events(&platform, &spec.id, &[], &AtomicBool::new(false));
        result.unwrap();
        // The junction entry itself may have unknown metadata; its target is never read.
        assert_eq!(
            found
                .iter()
                .filter(|e| matches!(e, ScanEvent::File(_)))
                .count(),
            1
        );
        std::fs::remove_dir(&link).unwrap();
        assert!(outside.join("data_0").exists());
    }
}
