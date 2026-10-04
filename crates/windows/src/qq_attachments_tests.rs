use super::*;
#[test]
fn discovery_separates_families_but_keeps_partial_layouts_for_diagnostics() -> Result<()> {
    let dir = tempfile::tempdir()?;
    assert!(!family_hint(AttachmentProvider::Qqnt, dir.path())?);
    assert!(!family_hint(AttachmentProvider::QqClassic, dir.path())?);
    std::fs::create_dir(dir.path().join("nt_qq"))?;
    assert!(family_hint(AttachmentProvider::Qqnt, dir.path())?);
    assert!(!family_hint(AttachmentProvider::QqClassic, dir.path())?);
    std::fs::write(dir.path().join("FileInfo.db"), b"partial")?;
    assert!(family_hint(AttachmentProvider::QqClassic, dir.path())?);
    assert!(
        layout(AttachmentProvider::Qqnt)?
            .1
            .contains(&"nt_qq/nt_db/nt_msg.db")
    );
    assert!(
        layout(AttachmentProvider::Qqnt)?
            .1
            .contains(&"nt_qq/nt_db/profile_info.db")
    );
    Ok(())
}
#[test]
fn attachment_guard_includes_external_qqguild_without_extending_cache_rules() {
    for provider in [AttachmentProvider::QqClassic, AttachmentProvider::Qqnt] {
        let names = process_names(provider).unwrap();
        assert!(names.contains(&"QQGuild.exe"));
        assert!(names.contains(&"QQ.exe"));
        assert!(names.contains(&"TXPlatform.exe"));
    }
    assert!(process_names(AttachmentProvider::Weixin).is_err());
    assert!(
        !dustaway_core::applications::application("qq")
            .unwrap()
            .processes
            .contains(&"QQGuild.exe")
    );
}
fn account_fixture(root: &Path, provider: AttachmentProvider) -> QqAccount {
    let pin = PinnedPath::observe(root, true, None).unwrap();
    QqAccount {
        provider,
        path: native(root),
        identity: pin.stamp.identity,
        parents: pin.parent_ids,
        markers: vec![],
        owner_sid: "test".into(),
        application: "test".into(),
        version: "9.7.25.29417".into(),
        layout: layout(provider).unwrap().0.into(),
    }
}
fn record(account: &QqAccount, path: &Path) -> ManualRecord {
    let pin = PinnedPath::observe(path, false, None).unwrap();
    ManualRecord {
        directory: false,
        identity: "test".into(),
        file: FileRecord {
            path_display: display_path(path),
            bytes: pin.stamp.size,
            modified_unix: pin.stamp.modified_unix(),
            opaque: serde_json::to_string(&QqStamp {
                provider: account.provider,
                account: account.clone(),
                file: Snapshot {
                    path: native(path),
                    root: account.path.clone(),
                    parents: pin.parent_ids,
                    stamp: pin.stamp,
                    application_identity: Some(account.application.clone()),
                },
            })
            .unwrap(),
        },
    }
}
#[test]
fn both_qq_sources_keep_the_common_snapshot_for_volume_accounting() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = extended(dir.path())?;
    let path = root.join("sample.zip");
    std::fs::write(&path, b"PK\x03\x04test")?;
    let platform = crate::WindowsPlatform::new(Scope::User)?;
    for provider in [AttachmentProvider::QqClassic, AttachmentProvider::Qqnt] {
        let item = record(&account_fixture(&root, provider), &path);
        let snapshot: Snapshot = serde_json::from_str(&item.file.opaque)?;
        assert_eq!(snapshot.path, native(&path));
        let target = Platform::file_volume(&platform, &item.file)?;
        assert_eq!(target.key, crate::platform::volume_target(&path)?.key);
        assert!(Platform::volume_available(&platform, &target)? > 0);
    }
    Ok(())
}
#[test]
fn families_protected_areas_and_unaccepted_formats_never_inherit_permission() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    let classic = account_fixture(&root, AttachmentProvider::QqClassic);
    let nt = account_fixture(&root, AttachmentProvider::Qqnt);
    for path in ["Video/test.mp4", "Image/test.jpg", "FileRecv/test.zip"] {
        assert!(attachment_area(&classic, &root.join(path)).is_ok());
        assert!(attachment_area(&nt, &root.join(path)).is_err());
        assert!(!released(&classic, &root.join(path)));
    }
    for path in ["nt_qq/nt_data/Video/test.mp4", "nt_qq/nt_data/Pic/test.png"] {
        assert!(attachment_area(&nt, &root.join(path)).is_ok());
        assert!(attachment_area(&classic, &root.join(path)).is_err());
        assert!(!released(&nt, &root.join(path)));
    }
    for path in [
        "MyCollection/Video/test.mp4",
        "GroupCollection/Image/test.jpg",
        "Video/Backup/test.mp4",
        "nt_qq/nt_db/nt_msg.db",
        "QQGuild/nt_data/Video/test.mp4",
        "../Video/test.mp4",
    ] {
        assert!(
            attachment_area(&classic, &root.join(path)).is_err(),
            "{path}"
        );
        assert!(attachment_area(&nt, &root.join(path)).is_err(), "{path}");
    }
    for name in [
        "index.db",
        "index.db-wal",
        "login.json",
        "shared.idx",
        "settings.ini",
    ] {
        assert!(protected_name(Path::new(name)));
    }
    assert!(!account_name("All Users"));
    assert!(!account_name("12345/../../"));
}
#[test]
fn classic_release_is_bound_to_layout_and_received_formats_not_app_version() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    let mut account = account_fixture(&root, AttachmentProvider::QqClassic);
    for name in ["photo.jpg", "video.mp4", "file.zip"] {
        assert!(released(
            &account,
            &root.join("FileRecv/MobileFile").join(name)
        ));
    }
    for path in [
        "Image/photo.jpg",
        "Video/video.mp4",
        "Audio/voice.wav",
        "FileRecv/file.zip",
        "FileRecv/MobileFile/nested/file.zip",
        "FileRecv/MobileFile/photo.png",
        "FileRecv/MobileFile/voice.wav",
        "FileRecv/MobileFile/shared.db",
        "FileRecv/MobileFile/image.dat",
        "FileRecv/MobileFile/../other.zip",
    ] {
        assert!(!released(&account, &root.join(path)), "{path}");
    }
    let path = root.join("FileRecv/MobileFile/file.zip");
    account.version = "9.7.25.29418".into();
    assert!(released(&account, &path));
    account.version = "9.7.25.29417".into();
    account.layout = "unverified-layout".into();
    assert!(!released(&account, &path));
    account.layout = "qq-classic-account-v1".into();
    account.provider = AttachmentProvider::Qqnt;
    assert!(!released(&account, &path));
}
#[test]
fn nt_release_is_bound_to_family_layout_and_format_not_app_version() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    let mut account = account_fixture(&root, AttachmentProvider::Qqnt);
    account.version = "9.9.33.52230".into();
    for name in ["photo.jpg", "video.mp4", "file.zip"] {
        assert!(released(
            &account,
            &root.join("nt_qq/nt_data/File").join(name)
        ));
    }
    for path in [
        "nt_qq/nt_data/Pic/photo.jpg",
        "nt_qq/nt_data/Video/video.mp4",
        "nt_qq/nt_data/Audio/voice.wav",
        "nt_qq/nt_data/File/Thumb/photo.jpg",
        "nt_qq/nt_data/File/nested/file.zip",
        "nt_qq/nt_data/File/photo.png",
        "nt_qq/nt_data/File/voice.wav",
        "nt_qq/nt_data/File/notes.txt",
        "nt_qq/nt_data/File/shared.db",
        "nt_qq/nt_data/File/encrypted.dat",
        "nt_qq/nt_data/File/../other.zip",
        "FileRecv/MobileFile/file.zip",
        "Downloads/file.zip",
        "nt_qq/nt_db/profile_info.db",
    ] {
        assert!(!released(&account, &root.join(path)), "{path}");
    }
    let path = root.join("nt_qq/nt_data/File/file.zip");
    for version in ["9.9.33.52231", "9.9.36.0", "9.7.25.29417", "12.0.1.20"] {
        account.version = version.into();
        assert!(released(&account, &path));
    }
    account.version = "9.9.33.52230".into();
    account.layout = "unverified-layout".into();
    assert!(!released(&account, &path));
    account.layout = "qqnt-account-v1".into();
    account.provider = AttachmentProvider::QqClassic;
    assert!(!released(&account, &path));
}
#[test]
fn pinned_qq_files_refuse_changes_links_exclusions_and_foreign_accounts() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    std::fs::create_dir(root.join("FileRecv")).unwrap();
    let path = root.join("FileRecv/test.zip");
    std::fs::write(&path, b"PK\x03\x04test").unwrap();
    let account = account_fixture(&root, AttachmentProvider::QqClassic);
    let original = record(&account, &path);
    assert!(pin_record(&original, &[], false).is_ok());
    assert!(pin_record(&original, &[display_path(&path)], false).is_err());
    let other = register(AttachmentProvider::Qqnt, &display_path(&root));
    assert!(other.is_err());
    let mut forged = original.clone();
    let mut stamp: QqStamp = serde_json::from_str(&forged.file.opaque).unwrap();
    stamp.provider = AttachmentProvider::Qqnt;
    forged.file.opaque = serde_json::to_string(&stamp).unwrap();
    assert!(pin_record(&forged, &[], false).is_err());
    std::fs::write(&path, b"changed").unwrap();
    assert!(pin_record(&original, &[], false).is_err());
    let changed = record(&account, &path);
    std::fs::hard_link(&path, root.join("another.zip")).unwrap();
    assert!(pin_record(&changed, &[], false).is_err());
    assert!(path.exists());
}
