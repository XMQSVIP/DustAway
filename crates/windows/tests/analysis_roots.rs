use dustaway_core::{AnalysisEvent, AnalysisPlatform};
use dustaway_windows::{WindowsAnalysis, fs::extended};
use std::sync::atomic::AtomicBool;

#[test]
fn compact_chain_preserves_native_names_and_rejects_replaced_ancestor() {
    use std::os::windows::ffi::OsStringExt;
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("祖先");
    let mut dir = first.clone();
    for _ in 0..20 {
        dir.push("长目录和空格 名称");
    }
    // Windows permits non-Unicode UTF-16 names; display text must not be used to reconstruct paths.
    dir.push(std::ffi::OsString::from_wide(&[0x78, 0xd800, 0x79]));
    std::fs::create_dir_all(extended(&dir).unwrap()).unwrap();
    let source = WindowsAnalysis::for_directory(temp.path());
    let root = source.validate_root(temp.path().to_str().unwrap()).unwrap();
    let mut records = vec![];
    source
        .scan(&root, &AtomicBool::new(false), &mut |e| {
            if let AnalysisEvent::Entry(r) = e {
                records.push(r.opaque);
            }
            Ok(true)
        })
        .unwrap();
    assert_eq!(records.len(), 23);
    assert!(source.refine(&root, &records).is_ok());
    std::fs::rename(&first, temp.path().join("保留原目录")).unwrap();
    std::fs::create_dir(&first).unwrap();
    assert!(source.refine(&root, &records).is_err());
}

#[test]
fn enumerates_root_children_before_descending_and_refines_only_verified_directories() {
    let temp = tempfile::tempdir().unwrap();
    for name in ["first", "second"] {
        std::fs::create_dir(temp.path().join(name)).unwrap();
        std::fs::write(temp.path().join(name).join("keep.txt"), "protected fixture").unwrap();
    }
    let source = WindowsAnalysis::for_directory(temp.path());
    let root = source.validate_root(temp.path().to_str().unwrap()).unwrap();
    let mut records = vec![];
    source
        .scan(&root, &AtomicBool::new(false), &mut |e| {
            if let AnalysisEvent::Entry(r) = e {
                records.push(r);
            }
            Ok(true)
        })
        .unwrap();
    assert_eq!(records[1].entry.parent.as_deref(), Some("0"));
    assert_eq!(records[2].entry.parent.as_deref(), Some("0"));
    assert!(records[1].entry.directory && records[2].entry.directory);
    let refined = source
        .refine(
            &root,
            &[records[0].opaque.clone(), records[1].opaque.clone()],
        )
        .unwrap();
    assert!(refined.ends_with(&records[1].entry.name));
    let path = temp.path().join(&records[1].entry.name);
    let moved = temp.path().join("preserved-original");
    std::fs::rename(&path, &moved).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(
        source
            .refine(
                &root,
                &[records[0].opaque.clone(), records[1].opaque.clone()]
            )
            .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(moved.join("keep.txt")).unwrap(),
        "protected fixture"
    );
}
#[test]
fn depth_limit_preserves_other_branches_and_marks_incomplete() {
    let temp = tempfile::tempdir().unwrap();
    let mut deep = extended(&temp.path().join("a-deep")).unwrap();
    for _ in 0..130 {
        deep.push("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("beyond.txt"), "keep").unwrap();
    std::fs::create_dir(temp.path().join("z-next")).unwrap();
    std::fs::write(temp.path().join("z-next/reachable.txt"), "keep").unwrap();
    let source = WindowsAnalysis::for_directory(temp.path());
    let root = source.validate_root(temp.path().to_str().unwrap()).unwrap();
    let (mut found, mut limited) = (false, false);
    source
        .scan(&root, &AtomicBool::new(false), &mut |e| {
            match e {
                AnalysisEvent::Entry(r) => found |= r.entry.name == "reachable.txt",
                AnalysisEvent::Issue { code, .. } => limited |= code == "depth_limit",
                _ => {}
            }
            Ok(true)
        })
        .unwrap();
    assert!(found && limited);
}
#[test]
fn only_registered_fixed_ntfs_volume_roots_are_accepted_without_reading_contents() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().ancestors().last().unwrap();
    assert!(
        WindowsAnalysis::for_directory(temp.path())
            .validate_root(path.to_str().unwrap())
            .is_err()
    );
    let source = WindowsAnalysis::new();
    let root = source.validate_root(path.to_str().unwrap()).unwrap();
    assert!(!root.opaque.is_empty());
    assert_eq!(source.location_info(path.to_str().unwrap()).kind, "volume");
}

#[test]
#[cfg(debug_assertions)]
fn fixture_disk_registers_only_the_marked_directory_and_keeps_host_root_unavailable() {
    use dustaway_core::AnalysisEngine;
    use std::sync::Arc;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("analysis");
    std::fs::create_dir(&path).unwrap();
    assert!(WindowsAnalysis::for_fixture_volume(&path).is_err());
    std::fs::write(temp.path().join(".dustaway-test-only"), "fixture-only").unwrap();
    let source = WindowsAnalysis::for_fixture_volume(&path).unwrap();
    assert!(
        source
            .validate_root(path.ancestors().last().unwrap().to_str().unwrap())
            .is_err()
    );
    let engine = AnalysisEngine::new(Arc::new(source)).unwrap();
    let locations = engine.locations();
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].path, path.to_str().unwrap());
    assert_eq!(locations[0].info.kind, "volume");
    assert!(locations[0].info.supported);
    let volumes = engine.scan_volumes(&[locations[0].token.clone()]).unwrap();
    assert_eq!(
        volumes[0].key,
        dustaway_windows::platform::volume_target(&path)
            .unwrap()
            .key
    );
    assert!(
        engine
            .scan_volumes(&[path.to_string_lossy().into_owned()])
            .is_err()
    );
}
