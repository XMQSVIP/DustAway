use super::*;
#[test]
fn release_is_bound_to_format_and_monthly_layout_not_application_version() {
    let root = Path::new(r"C:\xwechat_files\wxid_fixture");
    for path in [
        "msg/video/2026-09/sample.mp4",
        "msg/video/2026-09/sample.jpg",
        "msg/file/2026-10/received.zip",
        "msg/file/2026-10/received.pdf",
        "msg/file/2026-10/received.txt",
        "msg/file/2026-10/received.docx",
        "msg/file/2026-10/photo.png",
    ] {
        assert!(format_released(LAYOUT, root, &root.join(path)));
        assert!(!format_released("unknown-layout", root, &root.join(path)));
    }
    for path in [
        "msg/attach/2026-09/sample.jpg",
        "msg/video/sample.mp4",
        "msg/video/2026-13/sample.mp4",
        "msg/video/2026-09/deeper/sample.mp4",
        "msg/file/2026-10/received.exe",
        "msg/file/2026-10/received.dat",
        "msg/file/2026-10/received.db",
        "msg/file/2026-10/received.pdf.exe",
        "msg/audio/2026-10/voice.mp3",
        "db_storage/message/message_0.db",
    ] {
        assert!(!format_released(LAYOUT, root, &root.join(path)), "{path}");
    }
}

#[test]
fn received_file_formats_still_require_a_valid_header_and_pinned_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    let folder = root.join("msg/file/2026-10");
    std::fs::create_dir_all(&folder).unwrap();
    for (name, content) in [
        ("test.pdf", b"%PDF-1.7\nfixture".as_slice()),
        ("test.txt", b"owned test fixture".as_slice()),
        ("test.docx", b"PK\x03\x04fixture".as_slice()),
    ] {
        let path = folder.join(name);
        std::fs::write(&path, content).unwrap();
        assert!(format_released(LAYOUT, &root, &path));
        assert_eq!(classify(&root, &path).unwrap(), AttachmentKind::File);
        let stamp = snapshot(&root, &path);
        assert!(file_pin(&stamp, &[], false).is_ok());
        assert!(file_pin(&stamp, &[display_path(&folder)], true).is_err());
        std::fs::write(&path, b"\x00unknown bytes").unwrap();
        assert_eq!(classify(&root, &path).unwrap(), AttachmentKind::Unknown);
        assert!(file_pin(&stamp, &[], true).is_err());
    }
}
#[test]
fn readonly_attachment_delete_is_narrow_and_preserves_attributes_on_refusal() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("readonly.txt");
    std::fs::write(&path, b"fixture").unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    assert!(PinnedPath::open(&path, false, true).is_err());
    let pin = PinnedPath::open_attachment(&path, false).unwrap();
    assert!(pin.mark_attachment_delete().is_err()); // no DELETE access
    assert!(std::fs::metadata(&path).unwrap().permissions().readonly());
    PinnedPath::open_attachment(&path, true)
        .unwrap()
        .mark_attachment_delete()
        .unwrap();
    assert!(!path.exists());
}
fn snapshot(root: &Path, path: &Path) -> AttachmentStamp {
    let pin = PinnedPath::observe(path, false, None).unwrap();
    let account = Account {
        path: native(root),
        identity: pin.stamp.identity.clone(),
        parents: vec![],
        database: pin.stamp.identity.clone(),
        messages: pin.stamp.identity.clone(),
        message_db: pin.stamp.identity.clone(),
        owner_sid: "fixture".into(),
        application: "fixture".into(),
        version: "4.fixture".into(),
        layout: LAYOUT.into(),
    };
    AttachmentStamp {
        file: Snapshot {
            path: native(path),
            root: native(root),
            parents: pin.parent_ids.clone(),
            stamp: pin.stamp,
            application_identity: None,
        },
        account,
    }
}
#[test]
fn frozen_files_refuse_changes_exclusions_links_and_parent_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(&dir.path().join("xwechat_files/wxid_fixture")).unwrap();
    let parent = root.join("msg/file");
    std::fs::create_dir_all(&parent).unwrap();
    let path = parent.join("sample.txt");
    std::fs::write(&path, b"original").unwrap();
    let old = snapshot(&root, &path);
    assert!(crate::manual::protected(&path, false));
    assert!(file_pin(&old, &[], false).is_ok());
    assert!(file_pin(&old, &[display_path(&parent)], false).is_err());
    let link = parent.join("linked.txt");
    std::fs::hard_link(&path, &link).unwrap();
    assert!(file_pin(&old, &[], false).is_err());
    std::fs::remove_file(&link).unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert!(file_pin(&old, &[], false).is_err());
    let fresh = snapshot(&root, &path);
    std::fs::rename(&parent, root.join("msg/old-file")).unwrap();
    std::fs::create_dir(&parent).unwrap();
    std::fs::write(&path, b"changed").unwrap();
    assert!(file_pin(&fresh, &[], true).is_err());
    assert!(path.exists());
}
#[test]
fn unknown_formats_and_non_message_files_stay_protected() {
    let dir = tempfile::tempdir().unwrap();
    let root = extended(dir.path()).unwrap();
    std::fs::create_dir_all(root.join("msg/file")).unwrap();
    let unknown = root.join("msg/file/image.dat");
    std::fs::write(&unknown, b"encrypted").unwrap();
    assert_eq!(classify(&root, &unknown).unwrap(), AttachmentKind::Unknown);
    assert!(file_pin(&snapshot(&root, &unknown), &[], true).is_err());
    std::fs::create_dir_all(root.join("db_storage")).unwrap();
    let database = root.join("db_storage/message.db");
    std::fs::write(&database, b"chat database").unwrap();
    assert!(file_pin(&snapshot(&root, &database), &[], true).is_err());
    assert!(unknown.exists() && database.exists());
    assert!(!layout_supported("unknown-layout"));
}
