use dustaway_core::*;
use dustaway_windows::{WindowsAnalysis, fs::*};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
fn analyze(path: &Path) -> (Arc<AnalysisEngine>, String, AnalysisSnapshot) {
    let engine = AnalysisEngine::new(Arc::new(WindowsAnalysis::for_directory(path))).unwrap();
    let id = engine.start(&engine.locations()[0].token, vec![]).unwrap();
    let end = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(5));
    }
    let snapshot = engine.snapshot().unwrap();
    (engine, id, snapshot)
}
#[test]
fn occupied_and_hardlinked_files_are_counted_without_modification() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("中文 空格.txt");
    std::fs::write(&path, b"protected contents").unwrap();
    std::fs::hard_link(&path, temp.path().join("shared.txt")).unwrap();
    let before = std::fs::metadata(&path).unwrap().modified().unwrap();
    let name = wide(&path);
    let _writer = Handle(
        unsafe {
            CreateFileW(
                PCWSTR(name.as_ptr()),
                FILE_GENERIC_WRITE.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .unwrap(),
    );
    let (engine, id, s) = analyze(temp.path());
    assert_eq!(s.state, "completed");
    assert_eq!(s.files, 2);
    assert_eq!(s.logical_bytes, "36");
    assert_eq!(s.unique_bytes, "18");
    let page = engine.children(&id, "0", "", 0, true).unwrap();
    assert!(page.items.iter().all(|e| e.shared));
    let files = engine.files(&id, "", 0).unwrap();
    assert_eq!(files.total, 2);
    assert!(files.items.iter().all(|e| e.shared));
    let context = engine.context(&engine.locations()[0].token).unwrap();
    assert!(context.error.is_none());
    assert!(context.available_bytes.unwrap().parse::<u64>().unwrap() > 0);
    assert_eq!(std::fs::read(&path).unwrap(), b"protected contents");
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        before
    );
    assert!(engine.reveal(&id, "../outside").is_err());
}
#[test]
fn long_unicode_readonly_file_is_counted_and_preserved() {
    let temp = tempfile::tempdir().unwrap();
    let mut folder = extended(temp.path()).unwrap();
    for i in 0..9 {
        folder.push(format!("中文长目录-{i}-abcdefghijklmnopqrstuvwxyz"));
        std::fs::create_dir(&folder).unwrap();
    }
    let file = folder.join("只读资料.txt");
    std::fs::write(&file, b"keep").unwrap();
    let mut permissions = std::fs::metadata(&file).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&file, permissions).unwrap();
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();
    let (engine, id, s) = analyze(temp.path());
    assert_eq!(s.state, "completed");
    assert_eq!(s.unique_bytes, "4");
    assert_eq!(
        engine.files(&id, "只读资料", 0).unwrap().items[0].bytes,
        "4"
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"keep");
    let after = std::fs::metadata(&file).unwrap();
    assert_eq!(after.modified().unwrap(), before);
    assert!(after.permissions().readonly());
}
#[test]
fn sparse_logical_size_is_counted_and_offline_attributes_are_skipped() {
    use std::os::windows::process::CommandExt;
    let temp = tempfile::tempdir().unwrap();
    let sparse = temp.path().join("sparse.bin");
    std::fs::write(&sparse, b"x").unwrap();
    let result = std::process::Command::new("fsutil.exe")
        .args(["sparse", "setflag"])
        .arg(&sparse)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(result.status.success());
    std::fs::OpenOptions::new()
        .write(true)
        .open(&sparse)
        .unwrap()
        .set_len(1 << 30)
        .unwrap();
    let before = std::fs::metadata(&sparse).unwrap().modified().unwrap();
    let offline = temp.path().join("offline.bin");
    std::fs::write(&offline, b"protected").unwrap();
    let name = wide(&offline);
    unsafe {
        SetFileAttributesW(PCWSTR(name.as_ptr()), FILE_ATTRIBUTE_OFFLINE).unwrap();
    }
    let (_, _, s) = analyze(temp.path());
    unsafe {
        SetFileAttributesW(PCWSTR(name.as_ptr()), FILE_ATTRIBUTE_NORMAL).unwrap();
    }
    assert_eq!(s.state, "partial");
    assert_eq!(s.unique_bytes, (1u64 << 30).to_string());
    assert_eq!(s.files, 1);
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide(&sparse).as_ptr()), Some(&mut high)) };
    assert_ne!(low, u32::MAX);
    let expected = (u64::from(high) << 32) | u64::from(low);
    assert_eq!(s.allocated_bytes, expected.to_string());
    assert!(expected < 1u64 << 30);
    assert!(s.issues.iter().any(|i| i.code == "link_or_cloud"));
    assert_eq!(
        std::fs::metadata(&sparse).unwrap().modified().unwrap(),
        before
    );
    assert_eq!(std::fs::read(&offline).unwrap(), b"protected");
}
#[test]
fn access_denied_directory_is_incomplete_and_depth_limit_stops_enumeration() {
    use std::os::windows::process::CommandExt;
    let temp = tempfile::tempdir().unwrap();
    let denied = temp.path().join("denied");
    std::fs::create_dir(&denied).unwrap();
    std::fs::write(denied.join("protected"), b"keep").unwrap();
    let deny = std::process::Command::new("icacls.exe")
        .arg(&denied)
        .args(["/deny", "*S-1-1-0:(R)"])
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(deny.status.success());
    let (_, _, s) = analyze(temp.path());
    // Restore only the DACL entry installed on this dedicated temporary fixture.
    let restore = std::process::Command::new("icacls.exe")
        .arg(&denied)
        .args(["/remove:d", "*S-1-1-0"])
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(restore.status.success());
    assert_eq!(s.state, "partial");
    assert!(s.issues.iter().any(|i| i.code == "directory_inaccessible"));
    assert_eq!(std::fs::read(denied.join("protected")).unwrap(), b"keep");
    let mut deep = extended(temp.path()).unwrap();
    for _ in 0..=ANALYSIS_DEPTH {
        deep.push("层");
        std::fs::create_dir(&deep).unwrap();
    }
    std::fs::write(deep.join("keep"), b"keep").unwrap();
    let (_, _, s) = analyze(temp.path());
    assert_eq!(s.state, "partial");
    assert!(s.issues.iter().any(|i| i.code == "depth_limit"));
    assert_eq!(std::fs::read(deep.join("keep")).unwrap(), b"keep");
}
#[test]
fn junction_is_recorded_but_never_traversed() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), b"secret").unwrap();
    // Junctions do not need developer mode or symlink privilege.
    let junction = temp.path().join("junction");
    let status = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(status.status.success());
    let (_, _, s) = analyze(temp.path());
    assert_eq!(s.state, "partial");
    assert_eq!(s.files, 0);
    assert!(s.issues.iter().any(|i| i.code == "link_or_cloud"));
    std::fs::remove_dir(&junction).unwrap();
}
#[test]
fn replaced_root_or_file_cannot_be_revealed() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("data");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("file");
    std::fs::write(&file, b"original").unwrap();
    let platform = WindowsAnalysis::for_directory(&root);
    let prepared = platform.validate_root(root.to_str().unwrap()).unwrap();
    let mut records = vec![];
    platform
        .scan(&prepared, &AtomicBool::new(false), &mut |event| {
            if let AnalysisEvent::Entry(r) = event {
                records.push(r);
            }
            Ok(true)
        })
        .unwrap();
    std::fs::rename(&file, root.join("old")).unwrap();
    std::fs::write(&file, b"replacement").unwrap();
    assert!(
        platform
            .reveal(
                &prepared,
                &[records[0].opaque.clone(), records[1].opaque.clone()]
            )
            .is_err()
    );
    std::fs::rename(&root, temp.path().join("old-root")).unwrap();
    std::fs::create_dir(&root).unwrap();
    assert!(
        platform
            .scan(&prepared, &AtomicBool::new(false), &mut |_| Ok(true))
            .is_err()
    );
}
#[test]
fn compressed_allocation_matches_independent_windows_query() {
    use std::os::windows::process::CommandExt;
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("compressed.bin");
    std::fs::write(&file, vec![b'x'; 1024 * 1024]).unwrap();
    let output = std::process::Command::new("compact.exe")
        .args(["/C", "/I", "/Q", "/F"])
        .arg(&file)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(output.status.success());
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();
    let (_, _, snapshot) = analyze(temp.path());
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(wide(&file).as_ptr()), Some(&mut high)) };
    assert_ne!(low, u32::MAX);
    let expected = (u64::from(high) << 32) | u64::from(low);
    assert_eq!(snapshot.state, "completed");
    assert_eq!(snapshot.logical_bytes, "1048576");
    assert_eq!(snapshot.allocated_bytes, expected.to_string());
    assert!(expected < 1048576);
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        before
    );
}
