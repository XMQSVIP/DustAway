#![cfg(windows)]
use dustaway_windows::{
    fs::*,
    platform::{delete_snapshot, snapshot},
};
use std::{fs, os::windows::fs::OpenOptionsExt};

fn setup() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new()
        .prefix("dustaway-safety-test-")
        .tempdir()
        .unwrap();
    let file = dir.path().join("测试缓存.tmp");
    fs::write(&file, b"fixture-only").unwrap();
    filetime::set_file_mtime(
        &file,
        filetime::FileTime::from_unix_time(dustaway_core::now() - 86400 * 8, 0),
    )
    .unwrap();
    (dir, file)
}
fn snap(root: &std::path::Path, path: &std::path::Path) -> Snapshot {
    serde_json::from_str(&snapshot(root, path, 7).unwrap().opaque).unwrap()
}
#[test]
fn deletes_only_verified_fixture_file() {
    let (dir, file) = setup();
    let protected = dir.path().join("keep.txt");
    fs::write(&protected, b"keep").unwrap();
    let s = snap(dir.path(), &file);
    delete_snapshot(&s, 7, &[]).unwrap();
    assert!(!file.exists());
    assert!(protected.exists());
    assert!(dir.path().exists());
}
#[test]
fn modified_file_is_preserved() {
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    fs::write(&file, b"changed after preview").unwrap();
    assert!(delete_snapshot(&s, 7, &[]).is_err());
    assert!(file.exists());
}
#[test]
fn replaced_same_size_file_is_preserved() {
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    let old = dir.path().join("original.tmp");
    fs::rename(&file, &old).unwrap();
    fs::write(&file, b"fixture-only").unwrap();
    filetime::set_file_mtime(
        &file,
        filetime::FileTime::from_unix_time(s.stamp.modified_unix(), 0),
    )
    .unwrap();
    assert!(delete_snapshot(&s, 7, &[]).is_err());
    assert!(file.exists());
}
#[test]
fn replaced_parent_is_preserved() {
    let (dir, file) = setup();
    let folder = dir.path().join("parent");
    fs::create_dir(&folder).unwrap();
    let moved = folder.join("cache.tmp");
    fs::rename(&file, &moved).unwrap();
    let s = snap(dir.path(), &moved);
    fs::rename(&folder, dir.path().join("old-parent")).unwrap();
    fs::create_dir(&folder).unwrap();
    fs::rename(dir.path().join("old-parent/cache.tmp"), &moved).unwrap();
    assert!(delete_snapshot(&s, 7, &[]).is_err());
    assert!(moved.exists());
}
#[test]
fn hardlinks_are_preserved() {
    let (dir, file) = setup();
    fs::hard_link(&file, dir.path().join("link.tmp")).unwrap();
    assert!(snapshot(dir.path(), &file, 7).is_err());
    assert!(file.exists());
}
#[test]
fn newly_hardlinked_file_is_preserved() {
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    fs::hard_link(&file, dir.path().join("link.tmp")).unwrap();
    assert!(delete_snapshot(&s, 7, &[]).is_err());
}
#[test]
fn locked_file_is_preserved() {
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&file)
        .unwrap();
    assert!(delete_snapshot(&s, 7, &[]).is_err());
}
#[test]
fn recent_files_and_exclusions_are_preserved() {
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    assert!(delete_snapshot(&s, 7, &[file.to_string_lossy().into_owned()]).is_err());
    filetime::set_file_mtime(&file, filetime::FileTime::now()).unwrap();
    assert!(snapshot(dir.path(), &file, 7).is_err());
}
#[test]
fn root_and_prefix_siblings_are_rejected() {
    let (dir, file) = setup();
    assert!(snapshot(&file, &file, 0).is_err());
    let other = dir.path().join("different-root");
    assert!(snapshot(&other, &file, 0).is_err());
    assert!(extended(std::path::Path::new("C:\\test\\file:stream")).is_err());
    assert!(extended(std::path::Path::new("\\\\server\\share\\file")).is_err());
    assert!(extended(std::path::Path::new("C:relative")).is_err());
    assert!(excluded(
        std::path::Path::new("C:\\safe\\file"),
        &["c:\\SAFE".into()]
    ));
    assert!(!excluded(
        std::path::Path::new("C:\\safe-other\\file"),
        &["C:\\safe".into()]
    ));
}
#[test]
fn junction_is_not_traversed() {
    use std::os::windows::process::CommandExt;
    let (dir, file) = setup();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::rename(&file, target.join("cache.tmp")).unwrap();
    let link = dir.path().join("junction");
    // Only create a test junction; never invoke a shell to delete/move it.
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(PinnedPath::open(&link, true, false).is_err());
    assert!(snapshot(dir.path(), &link.join("cache.tmp"), 0).is_err());
    fs::remove_dir(&link).unwrap();
    assert!(target.join("cache.tmp").exists());
}
#[test]
fn long_unicode_path_can_be_verified() {
    let (dir, file) = setup();
    let mut folder = extended(dir.path()).unwrap();
    for i in 0..9 {
        folder.push(format!("长目录名称-测试-{i}-abcdefghijklmnopqrstuvwxyz"));
        fs::create_dir(&folder).unwrap();
    }
    let path = folder.join("缓存.tmp");
    fs::rename(&file, &path).unwrap();
    let s = snap(dir.path(), &path);
    delete_snapshot(&s, 7, &[]).unwrap();
    assert!(!path.exists());
}

#[test]
fn candidate_actions_are_read_only_and_reject_changed_missing_or_outside_files() {
    use dustaway_core::ExclusionKind;
    use dustaway_windows::platform::candidate_action_snapshot;
    let (dir, file) = setup();
    let s = snap(dir.path(), &file);
    assert_eq!(
        candidate_action_snapshot(&s, ExclusionKind::Parent, false).unwrap(),
        display_path(dir.path())
    );
    assert_eq!(
        candidate_action_snapshot(&s, ExclusionKind::File, false).unwrap(),
        display_path(&file)
    );
    assert!(file.exists());
    let mut forged = s.clone();
    forged.root = native(&dir.path().join("outside"));
    assert!(candidate_action_snapshot(&forged, ExclusionKind::Parent, false).is_err());
    fs::write(&file, b"changed").unwrap();
    assert!(candidate_action_snapshot(&s, ExclusionKind::File, false).is_err());
    fs::remove_file(&file).unwrap();
    assert!(candidate_action_snapshot(&s, ExclusionKind::File, false).is_err());
}
#[test]
fn candidate_actions_refuse_replaced_parent_and_new_hardlink() {
    use dustaway_core::ExclusionKind;
    use dustaway_windows::platform::candidate_action_snapshot;
    let (dir, file) = setup();
    let parent = dir.path().join("中文 有空格");
    fs::create_dir(&parent).unwrap();
    let moved = parent.join("缓存.tmp");
    fs::rename(&file, &moved).unwrap();
    let s = snap(dir.path(), &moved);
    fs::rename(&parent, dir.path().join("old")).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::rename(dir.path().join("old/缓存.tmp"), &moved).unwrap();
    assert!(candidate_action_snapshot(&s, ExclusionKind::Parent, false).is_err());
    let s = snap(dir.path(), &moved);
    fs::hard_link(&moved, dir.path().join("link")).unwrap();
    assert!(candidate_action_snapshot(&s, ExclusionKind::File, false).is_err());
    assert!(moved.exists());
}
