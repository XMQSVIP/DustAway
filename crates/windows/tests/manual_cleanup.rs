//! Ordinary production library, disposable fixtures on the project volume only.
use dustaway_core::*;
use dustaway_windows::{WindowsAnalysis, WindowsPlatform};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

fn fixture() -> tempfile::TempDir {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("artifacts");
    std::fs::create_dir_all(&workspace).unwrap();
    tempfile::Builder::new()
        .prefix("manual-fixture-")
        .tempdir_in(workspace)
        .unwrap()
}
fn analyze(path: &Path, excluded: Vec<String>) -> (Arc<AnalysisEngine>, String) {
    let analysis = AnalysisEngine::new(Arc::new(WindowsAnalysis::for_directory(path))).unwrap();
    let session = analysis
        .start(&analysis.locations()[0].token, excluded)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while analysis.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(analysis.snapshot().unwrap().state, "completed");
    (analysis, session)
}
fn engine(path: &Path) -> Arc<Engine> {
    Engine::open(
        Arc::new(WindowsPlatform::new(Scope::User).unwrap()),
        Scope::User,
        &path.join("journal.sqlite"),
    )
    .unwrap()
}
fn execute(engine: &Arc<Engine>, plan: &CleanupPlan) -> CleanupReport {
    engine.execute(&plan.id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while engine.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(engine.execute(&plan.id).is_err());
    engine.report().unwrap()
}
fn selection(session: &str, ids: &[String], excluded: &[String]) -> ManualSelection {
    ManualSelection {
        session: session.into(),
        selected_ids: ids.to_vec(),
        excluded_ids: excluded.to_vec(),
    }
}
#[test]
fn cloud_attributes_and_parent_junction_added_after_confirmation_are_retained() {
    use dustaway_windows::fs::wide;
    use windows::{Win32::Storage::FileSystem::*, core::PCWSTR};
    let temp = fixture();
    let root = temp.path().join("content");
    let parent = root.join("parent");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let cloud = root.join("cloud.txt");
    std::fs::write(&cloud, b"preserve cloud").unwrap();
    std::fs::write(parent.join("file.txt"), b"original").unwrap();
    std::fs::write(outside.join("file.txt"), b"protected target").unwrap();
    let before = std::fs::metadata(outside.join("file.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let ids: Vec<_> = analysis
        .files(&session, "", 0)
        .unwrap()
        .items
        .into_iter()
        .map(|e| e.id)
        .collect();
    let manifest = analysis
        .prepare_manual(&selection(&session, &ids, &[]))
        .unwrap();
    assert_eq!(manifest.summary.files, 2);
    let engine = engine(temp.path());
    let plan = engine.attach_manual(manifest).unwrap();
    std::fs::rename(&parent, root.join("moved")).unwrap();
    let linked = std::process::Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(&parent)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(linked.status.success());
    unsafe {
        SetFileAttributesW(PCWSTR(wide(&cloud).as_ptr()), FILE_ATTRIBUTE_OFFLINE).unwrap();
    }
    let report = execute(&engine, &plan);
    unsafe {
        SetFileAttributesW(PCWSTR(wide(&cloud).as_ptr()), FILE_ATTRIBUTE_NORMAL).unwrap();
    }
    // Remove this exact test junction without following or recursively deleting its target.
    std::fs::remove_dir(&parent).unwrap();
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 2);
    assert_eq!(std::fs::read(&cloud).unwrap(), b"preserve cloud");
    assert_eq!(
        std::fs::read(outside.join("file.txt")).unwrap(),
        b"protected target"
    );
    assert_eq!(
        std::fs::metadata(outside.join("file.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
}
#[test]
fn selected_tree_deduplicates_children_preserves_exclusions_and_new_files() {
    let temp = fixture();
    let root = temp.path().join("content");
    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("empty")).unwrap();
    std::fs::write(folder.join("delete.txt"), b"delete me").unwrap();
    std::fs::write(folder.join("keep.txt"), b"keep me").unwrap();
    let before = std::fs::metadata(folder.join("keep.txt"))
        .unwrap()
        .modified()
        .unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let dir = analysis
        .children(&session, "0", "", 0, true)
        .unwrap()
        .items
        .remove(0);
    assert!(dir.manual.allowed, "{:?}", dir.manual);
    let files = analysis.files(&session, "", 0).unwrap().items;
    let keep = files.iter().find(|e| e.name == "keep.txt").unwrap();
    let delete = files.iter().find(|e| e.name == "delete.txt").unwrap();
    let manifest = analysis
        .prepare_manual(&selection(
            &session,
            &[dir.id, delete.id.clone()],
            std::slice::from_ref(&keep.id),
        ))
        .unwrap();
    assert_eq!(manifest.summary.files, 1);
    assert_eq!(manifest.summary.directories, 2);
    let engine = engine(temp.path());
    let plan = engine.attach_manual(manifest).unwrap();
    assert!(plan.rule_ids.is_empty());
    assert_eq!(plan.bytes, "9");
    std::fs::write(folder.join("new.txt"), b"arrived after confirmation").unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 1);
    assert_eq!(report.manual.unwrap().deleted, 1);
    assert_eq!(report.directories.deleted, 1);
    assert_eq!(report.directories.skipped, 1);
    assert!(!folder.join("delete.txt").exists());
    assert!(!folder.join("empty").exists());
    assert_eq!(std::fs::read(folder.join("keep.txt")).unwrap(), b"keep me");
    assert_eq!(
        std::fs::metadata(folder.join("keep.txt"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    assert!(folder.join("new.txt").exists());
    let journal = std::fs::read(temp.path().join("journal.sqlite")).unwrap();
    assert!(!String::from_utf8_lossy(&journal).contains("delete.txt"));
}
#[test]
fn hardlink_outside_scan_root_is_rechecked_before_a_plan_can_delete_it() {
    let temp = fixture();
    let root = temp.path().join("content");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("ordinary.bin");
    let outside = temp.path().join("outside.bin");
    std::fs::write(&file, b"keep shared content").unwrap();
    std::fs::hard_link(&file, &outside).unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let row = analysis.files(&session, "", 0).unwrap().items.remove(0);
    // Directory batches cannot reveal links outside the scanned scope.
    assert!(row.verification_pending || row.shared);
    assert!(
        analysis
            .prepare_manual(&selection(&session, &[row.id], &[]))
            .is_err()
    );
    assert_eq!(std::fs::read(&file).unwrap(), b"keep shared content");
    assert_eq!(std::fs::read(&outside).unwrap(), b"keep shared content");
}
#[test]
fn changes_before_and_after_confirmation_never_delete_replacements() {
    let temp = fixture();
    let root = temp.path().join("content");
    std::fs::create_dir(&root).unwrap();
    let file = root.join("ordinary.bin");
    std::fs::write(&file, b"original").unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let id = analysis.files(&session, "", 0).unwrap().items[0].id.clone();
    std::fs::write(&file, b"changed").unwrap();
    assert!(
        analysis
            .prepare_manual(&selection(&session, &[id], &[]))
            .is_err()
    );
    let (analysis, session) = analyze(&root, vec![]);
    let id = analysis.files(&session, "", 0).unwrap().items[0].id.clone();
    let manifest = analysis
        .prepare_manual(&selection(&session, &[id], &[]))
        .unwrap();
    let engine = engine(temp.path());
    let plan = engine.attach_manual(manifest).unwrap();
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, b"replacement").unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 1);
    assert_eq!(std::fs::read(&file).unwrap(), b"replacement");
}
#[test]
fn permanent_exclusions_hardlinks_and_app_data_remain_inside_selected_folder() {
    let temp = fixture();
    let root = temp.path().join("content");
    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("xwechat_files")).unwrap();
    for name in [
        "keep.txt",
        "shared.txt",
        "delete.txt",
        "xwechat_files/chat.db",
    ] {
        std::fs::write(folder.join(name), b"fixture").unwrap();
    }
    std::fs::hard_link(folder.join("shared.txt"), folder.join("shared-copy.txt")).unwrap();
    let (analysis, session) = analyze(
        &root,
        vec![folder.join("keep.txt").to_string_lossy().into_owned()],
    );
    let dir = analysis
        .children(&session, "0", "", 0, true)
        .unwrap()
        .items
        .remove(0);
    let manifest = analysis
        .prepare_manual(&selection(&session, &[dir.id], &[]))
        .unwrap();
    assert_eq!(manifest.summary.files, 1);
    assert!(manifest.summary.retained >= 4);
    let engine = engine(temp.path());
    let plan = engine.attach_manual(manifest).unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 1);
    for name in [
        "keep.txt",
        "shared.txt",
        "shared-copy.txt",
        "xwechat_files/chat.db",
    ] {
        assert_eq!(std::fs::read(folder.join(name)).unwrap(), b"fixture");
    }
}
#[test]
fn forged_stale_and_duplicate_selections_are_rejected() {
    let temp = fixture();
    std::fs::write(temp.path().join("file.txt"), b"fixture").unwrap();
    let (analysis, session) = analyze(temp.path(), vec![]);
    let id = analysis.files(&session, "", 0).unwrap().items[0].id.clone();
    assert!(
        analysis
            .prepare_manual(&selection("forged", std::slice::from_ref(&id), &[]))
            .is_err()
    );
    assert!(
        analysis
            .prepare_manual(&selection(&session, &["../../outside".into()], &[]))
            .is_err()
    );
    assert!(
        analysis
            .prepare_manual(&selection(
                &session,
                std::slice::from_ref(&id),
                std::slice::from_ref(&id)
            ))
            .is_err()
    );
    analysis.mark_stale();
    assert!(
        analysis
            .prepare_manual(&selection(&session, &[id], &[]))
            .is_err()
    );
}
#[test]
fn empty_tree_is_deleted_bottom_up_and_parent_replacement_is_retained() {
    let temp = fixture();
    let root = temp.path().join("content");
    let folder = root.join("folder");
    std::fs::create_dir_all(folder.join("child")).unwrap();
    std::fs::write(folder.join("child/file.txt"), b"fixture").unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let id = analysis.children(&session, "0", "", 0, true).unwrap().items[0]
        .id
        .clone();
    let engine = engine(temp.path());
    let plan = engine
        .attach_manual(
            analysis
                .prepare_manual(&selection(&session, &[id], &[]))
                .unwrap(),
        )
        .unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 1);
    assert_eq!(report.directories.deleted, 2);
    assert!(!folder.exists());
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("file.txt"), b"original").unwrap();
    let (analysis, session) = analyze(&root, vec![]);
    let id = analysis.children(&session, "0", "", 0, true).unwrap().items[0]
        .id
        .clone();
    engine.invalidate_plan().unwrap();
    let plan = engine
        .attach_manual(
            analysis
                .prepare_manual(&selection(&session, &[id], &[]))
                .unwrap(),
        )
        .unwrap();
    std::fs::rename(&folder, root.join("moved")).unwrap();
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(folder.join("file.txt"), b"replacement").unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.directories.deleted, 0);
    assert_eq!(
        std::fs::read(folder.join("file.txt")).unwrap(),
        b"replacement"
    );
    assert!(root.join("moved/file.txt").exists());
}
#[test]
fn cancellation_stops_remaining_work_and_a_locked_file_is_preserved() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = fixture();
    let root = temp.path().join("content");
    let folder = root.join("folder");
    std::fs::create_dir_all(&folder).unwrap();
    for i in 0..200 {
        std::fs::write(folder.join(format!("{i}.txt")), b"fixture").unwrap();
    }
    let (analysis, session) = analyze(&root, vec![]);
    let id = analysis.children(&session, "0", "", 0, true).unwrap().items[0]
        .id
        .clone();
    let engine = engine(temp.path());
    let plan = engine
        .attach_manual(
            analysis
                .prepare_manual(&selection(&session, &[id], &[]))
                .unwrap(),
        )
        .unwrap();
    engine.execute(&plan.id).unwrap();
    engine.cancel();
    let deadline = Instant::now() + Duration::from_secs(10);
    while engine.is_busy() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let report = engine.report().unwrap();
    assert_eq!(report.status, "cancelled");
    assert!(report.deleted < 200);
    assert_eq!(
        std::fs::read_dir(&folder).unwrap().count() as u64,
        200 - report.deleted
    );
    assert!(engine.execute(&plan.id).is_err());
    let (analysis, session) = analyze(&root, vec![]);
    let file = analysis.files(&session, "", 0).unwrap().items.remove(0);
    engine.invalidate_plan().unwrap();
    let plan = engine
        .attach_manual(
            analysis
                .prepare_manual(&selection(&session, &[file.id], &[]))
                .unwrap(),
        )
        .unwrap();
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(1)
        .open(&file.path)
        .unwrap();
    let report = execute(&engine, &plan);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 1);
    assert!(Path::new(&file.path).exists());
}
