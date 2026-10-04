use super::*;
use filetime::{FileTime, set_file_mtime};

pub(crate) fn gpu_fixture(root: &Path) -> PathBuf {
    std::fs::create_dir_all(root).unwrap();
    for (n, size) in [(0u32, 36u32), (1, 256), (2, 1024), (3, 4096)] {
        let mut bytes = vec![0u8; 8192 + size as usize];
        bytes[..4].copy_from_slice(&0xc104cac3u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&0x20000u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&n.to_le_bytes());
        bytes[12..16].copy_from_slice(&size.to_le_bytes());
        std::fs::write(root.join(format!("data_{n}")), bytes).unwrap();
    }
    let mut index = vec![0u8; 368 + 65536 * 4];
    index[..4].copy_from_slice(&0xc103cac3u32.to_le_bytes());
    index[4..8].copy_from_slice(&0x30000u32.to_le_bytes());
    std::fs::write(root.join("index"), index).unwrap();
    for file in std::fs::read_dir(root).unwrap() {
        set_file_mtime(
            file.unwrap().path(),
            FileTime::from_unix_time(now() - 30 * 86400, 0),
        )
        .unwrap();
    }
    root.join("data_1")
}
pub(crate) fn simple_code_fixture(root: &Path) -> PathBuf {
    std::fs::create_dir_all(root.join("js")).unwrap();
    let path = root.join("js/0123456789abcdef_0");
    std::fs::write(&path, cache_bytes()).unwrap();
    let mut marker = vec![0; 24];
    marker[..8].copy_from_slice(&0xfcfb6d1ba7725c30u64.to_le_bytes());
    marker[8..12].copy_from_slice(&9u32.to_le_bytes());
    std::fs::write(root.join("js/index"), marker).unwrap();
    path
}

pub(crate) fn zip_bytes(name: &str, payload: &[u8]) -> Vec<u8> {
    let mut crc = !0u32;
    for &b in payload {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    let crc = !crc;
    let n = name.len() as u16;
    let size = payload.len() as u32;
    let mut b = Vec::new();
    b.extend(b"PK\x03\x04");
    b.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    b.extend(crc.to_le_bytes());
    b.extend(size.to_le_bytes());
    b.extend(size.to_le_bytes());
    b.extend(n.to_le_bytes());
    b.extend([0, 0]);
    b.extend(name.as_bytes());
    b.extend(payload);
    let offset = b.len() as u32;
    b.extend(b"PK\x01\x02");
    b.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    b.extend(crc.to_le_bytes());
    b.extend(size.to_le_bytes());
    b.extend(size.to_le_bytes());
    b.extend(n.to_le_bytes());
    b.extend([0; 16]);
    b.extend(name.as_bytes());
    let length = b.len() as u32 - offset;
    b.extend(b"PK\x05\x06");
    b.extend([0, 0, 0, 0, 1, 0, 1, 0]);
    b.extend(length.to_le_bytes());
    b.extend(offset.to_le_bytes());
    b.extend([0, 0]);
    b
}
#[test]
fn vsix_checks_manifest_crc_structure_and_refuses_disguised_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("publisher.tool-1.0.0");
    let bytes = zip_bytes(
        "extension/package.json",
        br#"{"publisher":"publisher","name":"tool","version":"1.0.0"}"#,
    );
    std::fs::write(&path, &bytes).unwrap();
    assert!(valid_vsix(&path, None).unwrap());
    for invalid in [
        b"PK fake.zip".to_vec(),
        bytes[..bytes.len() - 1].to_vec(),
        zip_bytes("../extension/package.json", b"{}"),
        zip_bytes("extension/package.json", b"{}"),
        zip_bytes("report.txt", b"important"),
    ] {
        std::fs::write(&path, invalid).unwrap();
        assert!(!valid_vsix(&path, None).unwrap());
    }
    let mut corrupt = bytes;
    corrupt[60] ^= 1;
    std::fs::write(&path, corrupt).unwrap();
    assert!(!valid_vsix(&path, None).unwrap());
}
#[test]
fn code_cache_protects_current_build_future_and_boundary_directories() {
    let dir = tempfile::tempdir().unwrap();
    let spec = dustaway_core::app_cache::rule("appcache.vscode_code").unwrap();
    let current = "a".repeat(40);
    let old = "b".repeat(40);
    let identity = CodeIdentity {
        _pins: vec![],
        commit: current.clone(),
        fingerprint: "fixture".into(),
        profile: None,
    };
    for name in [&current, &old] {
        std::fs::create_dir_all(dir.path().join(name).join("chrome/js")).unwrap();
        std::fs::write(
            dir.path().join(name).join("chrome/js/0123456789abcdef_0"),
            cache_bytes(),
        )
        .unwrap();
    }
    let old_file = dir.path().join(&old).join("chrome/js/0123456789abcdef_0");
    let cutoff = now() - 90 * 86400;
    set_file_mtime(
        dir.path().join(&old),
        FileTime::from_unix_time(cutoff - 2, 0),
    )
    .unwrap();
    assert_eq!(
        reason(spec, dir.path(), &old_file, Some(&identity)).unwrap(),
        None
    );
    assert_eq!(
        reason(
            spec,
            dir.path(),
            &dir.path()
                .join(&current)
                .join("chrome/js/0123456789abcdef_0"),
            Some(&identity)
        )
        .unwrap(),
        Some(ReasonCode::Protected)
    );
    assert_eq!(
        reason(spec, dir.path(), &old_file, None).unwrap(),
        Some(ReasonCode::ApplicationUnverified)
    );
    for time in [cutoff + 1, now() + 86400] {
        set_file_mtime(dir.path().join(&old), FileTime::from_unix_time(time, 0)).unwrap();
        assert_eq!(
            reason(spec, dir.path(), &old_file, Some(&identity)).unwrap(),
            Some(ReasonCode::Recent)
        );
    }
    assert_eq!(
        reason(
            spec,
            dir.path(),
            &dir.path().join("../personal.txt"),
            Some(&identity)
        )
        .unwrap(),
        Some(ReasonCode::Protected)
    );
}
#[test]
fn discovery_preserves_uninstalled_data_without_entering_personal_storage() {
    let dir = tempfile::tempdir().unwrap();
    let local = dir.path().join("local");
    let roaming = dir.path().join("roaming");
    let spec = dustaway_core::app_cache::rule("appcache.cherry").unwrap();
    for leaf in [
        "Cache",
        "Code Cache",
        "Partitions/webview/Cache",
        "data",
        "IndexedDB",
        "Local Storage",
    ] {
        std::fs::create_dir_all(roaming.join("CherryStudio").join(leaf)).unwrap();
    }
    let mut issues = vec![];
    let found = roots(&local, &roaming, spec, None, &mut |r| issues.push(r)).unwrap();
    assert_eq!(found.len(), 3);
    assert!(issues.is_empty());
    assert!(
        !found
            .iter()
            .any(|p| p.ends_with("data") || p.ends_with("IndexedDB"))
    );
    let observation = Inventory {
        entries: vec![],
        complete: true,
    }
    .observation(spec, &found);
    assert_eq!(observation.identity_status, "unverified_data");
    assert_eq!(observation.locations.len(), 3);
    assert!(blocked(spec).unwrap().is_some());
    let cancel = AtomicBool::new(true);
    assert!(
        Inventory::read(Some(&cancel))
            .err()
            .unwrap()
            .is::<ScanCancelled>()
    );
    assert!(
        roots(&local, &roaming, spec, Some(&cancel), &mut |_| {})
            .unwrap_err()
            .is::<ScanCancelled>()
    );
}
#[test]
fn update_directories_are_always_read_only_even_when_old() {
    for spec in dustaway_core::app_cache::catalog()
        .iter()
        .filter(|s| s.kind == "updates")
    {
        assert!(!spec.cleanable());
        assert!(!dustaway_core::cleanup_acceptance::release_enabled(
            &spec.descriptor()
        ));
        assert_eq!(
            blocked(spec).unwrap().unwrap().code,
            ReasonCode::ValidationPending
        );
    }
}

fn reason(
    spec: &CacheSpec,
    root: &Path,
    path: &Path,
    identity: Option<&CodeIdentity>,
) -> Result<Option<ReasonCode>> {
    super::file_reason(spec, root, path, identity, None)
}
pub(crate) fn cache_bytes() -> Vec<u8> {
    let mut b = vec![0; 49];
    b[..8].copy_from_slice(&0xfcfb6d1ba7725c30u64.to_le_bytes());
    b[8..12].copy_from_slice(&5u32.to_le_bytes());
    b[12..16].copy_from_slice(&1u32.to_le_bytes());
    b[25..33].copy_from_slice(&0xf4fa6f45970d41d8u64.to_le_bytes());
    b
}
#[test]
fn versions_and_binary_formats_cannot_be_inferred_from_names() {
    let at = 2_000_000_000;
    for days in [7, 90] {
        let boundary = at - i64::from(days) * 86400;
        assert!(!old_build_directory(boundary, at, days));
        assert!(!old_build_directory(boundary + 1, at, days));
        assert!(!old_build_directory(at + 1, at, days));
        assert!(old_build_directory(boundary - 1, at, days));
    }
    assert!(same_version("1.134.0.0", "1.134.0"));
    assert!(same_version("1.134.0", "1.134.0-insider"));
    for (a, b) in [
        ("1.1.0", "1.10.0"),
        ("1.0", "1.0.0"),
        ("1.134.0.1", "1.134.0"),
        ("garbage", "garbage"),
    ] {
        assert!(!same_version(a, b));
    }
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("0123456789abcdef_0");
    std::fs::write(&p, cache_bytes()).unwrap();
    assert!(valid_code_format(&p).unwrap());
    std::fs::write(&p, b"personal source code, not a cache").unwrap();
    assert!(!valid_code_format(&p).unwrap());
    let cancel = AtomicBool::new(true);
    assert!(
        valid_vsix(&p, Some(&cancel))
            .unwrap_err()
            .is::<ScanCancelled>()
    );
}
#[test]
fn vsix_temporary_mismatched_names_and_zip_flags_are_protected() {
    let dir = tempfile::tempdir().unwrap();
    let good = zip_bytes(
        "extension/package.json",
        br#"{"publisher":"p","name":"n","version":"1.0.0"}"#,
    );
    for name in [
        ".download",
        "p.n-1.0.0.tmp",
        "other.name-1.0.0",
        "p.n-1.0.0.sigzip",
    ] {
        let p = dir.path().join(name);
        std::fs::write(&p, &good).unwrap();
        assert!(!valid_vsix(&p, None).unwrap());
    }
    let p = dir.path().join("p.n-1.0.0");
    let mut bad = good.clone();
    bad[6] = 1;
    std::fs::write(&p, bad).unwrap();
    assert!(!valid_vsix(&p, None).unwrap());
    let mut bad = good;
    bad[8] = 8;
    std::fs::write(&p, bad).unwrap();
    assert!(!valid_vsix(&p, None).unwrap());
}
