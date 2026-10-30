use super::*;

const IDS: [&str; 4] = [
    "appcache.dingtalk_updates",
    "appcache.dingtalk_custom_logs",
    "appcache.gameviewer_logs",
    "appcache.hbuilder_backup",
];
const UPDATE: &str = "DingTalkUpdate_v4_8.3.45.260720005-8.5.0.260817002";

fn setup() -> (tempfile::TempDir, WindowsPlatform, PathBuf) {
    let (temp, mut platform) = fixture();
    let drive = temp.path().join("disk");
    std::fs::create_dir_all(&drive).unwrap();
    platform.discovery_drives = vec![drive.clone()];
    (temp, platform, drive)
}
fn update(base: &Path) -> PathBuf {
    let root = base.join(UPDATE);
    write(&root.join("upgrade_manifest.json"), true);
    write(&root.join("bin/component.dll.patch"), true);
    root
}
fn logs(base: &Path) {
    std::fs::create_dir_all(base.join("config")).unwrap();
    std::fs::create_dir_all(base.join("defEmotion")).unwrap();
    write(&base.join("log/history.xlog"), false);
}
fn scan(
    platform: &WindowsPlatform,
    id: &str,
    volumes: Option<&[dustaway_core::VolumeTarget]>,
    exclusions: &[String],
    cancel: &AtomicBool,
) -> (Result<()>, Vec<ScanEvent>) {
    let mut events = vec![];
    let result = platform.scan_on_volumes(&rule(id), exclusions, volumes, cancel, &mut |e| {
        events.push(e)
    });
    (result, events)
}
fn files(events: &[ScanEvent]) -> Vec<FileRecord> {
    events
        .iter()
        .filter_map(|e| {
            if let ScanEvent::File(f) = e {
                Some(f.clone())
            } else {
                None
            }
        })
        .collect()
}
fn register(platform: &WindowsPlatform, name: &str, path: &Path) {
    platform.inventory.lock().unwrap().as_mut().unwrap().entries.push(serde_json::from_value(serde_json::json!({
        "id":name,"name":name,"publisher":"fixture","version":"1","source":"desktop","scope":"user",
        "estimated_bytes":null,"installed_on":null,"install_location":display_path(path),
        "can_uninstall":false,"unavailable_reason":null,"residual_supported":false,"residual_note":"fixture"
    })).unwrap());
}

#[test]
fn discovered_cache_covers_relocated_updates_logs_and_backups_without_neighbors() {
    let (temp, platform, drive) = setup();
    for parent in ["dingcache", "DingTalkAppData/DingTalk"] {
        let base = drive.join(parent);
        update(&base);
        logs(&base);
        for file in [
            "users/chat.db",
            "plugins/plugin.dll",
            "Download/important.zip",
            "config/settings.json",
        ] {
            write(&base.join(file), true);
        }
    }
    write(&drive.join("GameViewer/bin/GameViewer.exe"), true);
    write(&drive.join("GameViewer/log/server/log/history.slog"), false);
    write(&drive.join("GameViewer/Download/document.zip"), true);
    write(&drive.join("HBuilderX/HBuilderX.exe"), true);
    write(&drive.join("HBuilderX/update/backup/old.exe"), true);
    write(&drive.join("HBuilderX/projects/source.txt"), true);
    let volume = volume_target(&drive).unwrap();
    for (id, expected) in IDS.into_iter().zip([4, 2, 1, 1]) {
        let (result, events) = scan(
            &platform,
            id,
            Some(std::slice::from_ref(&volume)),
            &[],
            &AtomicBool::new(false),
        );
        result.unwrap();
        let found = files(&events);
        assert_eq!(found.len(), expected, "{id}");
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ScanEvent::CoverageIssue(_)))
        );
        for file in found {
            assert_eq!(
                error_reason(&platform.delete(&rule(id), &file, &[]).unwrap_err()),
                ReasonCode::ValidationPending
            );
            assert_eq!(
                std::fs::read(&file.path_display).unwrap(),
                b"protected fixture contents"
            );
        }
    }
    let engine = Engine::open(
        Arc::new(platform),
        Scope::User,
        &temp.path().join("journal"),
    )
    .unwrap();
    let session = engine.start_scan(IDS.map(str::to_owned).to_vec()).unwrap();
    wait(&engine);
    for id in IDS {
        assert!(
            engine
                .prepare(
                    &session,
                    Selection {
                        rule_ids: vec![id.into()],
                        reviewed_rule_ids: vec![id.into()],
                        ..Default::default()
                    }
                )
                .is_err()
        );
    }
}

#[test]
fn discovered_cache_uses_registration_and_deduplicates_known_locations() {
    let (_temp, platform, drive) = setup();
    let custom = drive.join("My Tools/Remote Control");
    write(&custom.join("bin/GameViewer.exe"), true);
    write(&custom.join("log/client/test.slog"), true);
    register(&platform, "GameViewer", &custom);
    register(&platform, "UU 远程", &custom.join("bin"));
    let known = drive.join("GameViewer");
    write(&known.join("GameViewer.exe"), true);
    write(&known.join("log/test.slog"), true);
    register(&platform, "GameViewer", &known);
    // A registry name alone does not turn an unrelated log directory into a product.
    let false_product = drive.join("Personal");
    write(&false_product.join("log/document.txt"), true);
    register(&platform, "GameViewer", &false_product);
    let (result, events) = scan(&platform, IDS[2], None, &[], &AtomicBool::new(false));
    result.unwrap();
    assert_eq!(files(&events).len(), 2);
    assert!(
        files(&events)
            .iter()
            .all(|f| !f.path_display.contains("Personal"))
    );
}

#[test]
fn discovered_cache_requires_update_manifest_and_strict_version_names() {
    let (_temp, platform, drive) = setup();
    let base = drive.join("dingcache");
    update(&base);
    for name in [
        "DingTalkUpdate_notes",
        "DingTalkUpdate_v4_8.5.0-personal",
        "DingTalkUpdate_v4_8..5",
        "DingTalkUpdate_v4_8.5.0-8.6.0-backup",
    ] {
        write(&base.join(name).join("upgrade_manifest.json"), true);
    }
    write(
        &base.join("DingTalkUpdate_v5_8.5.0-8.6.0/private.txt"),
        true,
    );
    let (result, events) = scan(&platform, IDS[0], None, &[], &AtomicBool::new(false));
    result.unwrap();
    assert_eq!(files(&events).len(), 2);
}

#[test]
fn discovered_cache_respects_disk_identity_exclusions_and_cancellation() {
    let (_temp, platform, drive) = setup();
    let root = update(&drive.join("dingcache"));
    let mut other = volume_target(&drive).unwrap();
    other.key = "stale-volume-identity".into();
    let (_, events) = scan(
        &platform,
        IDS[0],
        Some(&[other]),
        &[],
        &AtomicBool::new(false),
    );
    assert!(files(&events).is_empty());
    for exclusion in [&drive, &drive.join("dingcache"), &root] {
        let (_, events) = scan(
            &platform,
            IDS[0],
            None,
            &[display_path(exclusion)],
            &AtomicBool::new(false),
        );
        assert!(files(&events).is_empty());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ScanEvent::Retained(ReasonCode::Excluded)))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ScanEvent::NoticeCode(ReasonCode::LocationMissing)))
        );
    }
    let (_, events) = scan(&platform, IDS[0], None, &[], &AtomicBool::new(true));
    assert!(files(&events).is_empty());
    let cancel = AtomicBool::new(false);
    let mut count = 0;
    platform
        .scan(&rule(IDS[0]), &[], &cancel, &mut |event| {
            if matches!(event, ScanEvent::File(_)) {
                count += 1;
                cancel.store(true, Ordering::Release);
            }
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn discovered_cache_reports_partial_access_and_skips_linked_roots() {
    let (temp, platform, drive) = setup();
    let good = update(&drive.join("dingcache"));
    let denied = update(&drive.join("DingTalkAppData/DingTalk"));
    let guard = Denied::new(&denied);
    let (result, events) = scan(&platform, IDS[0], None, &[], &AtomicBool::new(false));
    result.unwrap();
    assert_eq!(files(&events).len(), 2);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ScanEvent::CoverageIssue(_)))
    );
    drop(guard);
    let outside = temp.path().join("outside");
    update(&outside);
    let link = drive.join("DingTalk");
    assert!(
        std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .creation_flags(0x08000000)
            .output()
            .unwrap()
            .status
            .success()
    );
    let (_, events) = scan(&platform, IDS[0], None, &[], &AtomicBool::new(false));
    assert_eq!(files(&events).len(), 4);
    assert!(
        files(&events)
            .iter()
            .all(|f| !f.path_display.contains("outside"))
    );
    std::fs::remove_dir(&link).unwrap();
    assert!(good.join("upgrade_manifest.json").exists());
}

#[test]
fn discovered_cache_adds_standard_updates_but_never_duplicates_standard_logs() {
    let (_temp, mut platform, drive) = setup();
    platform.local = drive.join("User/Local");
    platform.roaming = drive.join("User/Roaming");
    update(&platform.local.join("DingTalk"));
    update(&platform.roaming.join("DingTalk"));
    logs(&platform.roaming.join("DingTalk"));
    register(&platform, "DingTalk", &platform.roaming.join("DingTalk"));
    let (result, events) = scan(&platform, IDS[0], None, &[], &AtomicBool::new(false));
    result.unwrap();
    assert_eq!(files(&events).len(), 4);
    let (result, events) = scan(&platform, IDS[1], None, &[], &AtomicBool::new(false));
    result.unwrap();
    assert!(files(&events).is_empty());
    let (result, events) = scan(
        &platform,
        "appcache.dingtalk_logs",
        None,
        &[],
        &AtomicBool::new(false),
    );
    result.unwrap();
    assert_eq!(files(&events).len(), 1);
}
