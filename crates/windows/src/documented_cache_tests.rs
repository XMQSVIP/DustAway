use super::*;
use crate::app_cache::tests::{gpu_fixture, simple_code_fixture};

#[test]
fn documented_gpu_refuses_foreign_files_and_unknown_block_geometry() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let path = gpu_fixture(root);
    for file in std::fs::read_dir(root).unwrap() {
        assert!(gpu_file(root, &file.unwrap().path()).unwrap());
    }
    let original = std::fs::read(&path).unwrap();
    for at in [0, 4, 8, 12] {
        let mut changed = original.clone();
        changed[at] ^= 1;
        std::fs::write(&path, changed).unwrap();
        assert!(!gpu_file(root, &path).unwrap());
    }
    std::fs::write(&path, &original).unwrap();
    let foreign = root.join("conversations.db");
    std::fs::write(&foreign, &original).unwrap();
    assert!(!gpu_file(root, &foreign).unwrap());
    let response = root.join("f_000001");
    std::fs::write(&response, &original).unwrap();
    assert!(!gpu_file(root, &response).unwrap());
    let spec = dustaway_core::app_cache::rule("appcache.cherry_gpu").unwrap();
    assert!(
        accepts(spec, root, &path, Some(&AtomicBool::new(true)))
            .unwrap_err()
            .is::<ScanCancelled>()
    );
    let malformed = root.join("data_2");
    std::fs::write(&malformed, b"not cache data").unwrap();
    assert!(!gpu_file(root, &malformed).unwrap());
}

#[test]
fn documented_code_requires_fixed_layout_and_preserves_shared_index() {
    let temp = tempfile::tempdir().unwrap();
    let spec = dustaway_core::app_cache::rule("appcache.pycharm_code").unwrap();
    let path = simple_code_fixture(temp.path());
    assert!(accepts(spec, temp.path(), &path, None).unwrap());
    assert!(!accepts(spec, temp.path(), &temp.path().join("js/index"), None).unwrap());
    let forged = temp.path().join("js/report.docx");
    std::fs::copy(&path, &forged).unwrap();
    assert!(!accepts(spec, temp.path(), &forged, None).unwrap());
    std::fs::write(temp.path().join("js/index"), b"unrecognized index").unwrap();
    assert!(!accepts(spec, temp.path(), &path, None).unwrap());
}

#[test]
fn documented_identity_accepts_consistent_new_versions_and_invalidates_digest_on_change() {
    let product = serde_json::json!({"name":"PyCharm","version":"2026.1","buildNumber":"261.22158.340","productCode":"PY","productVendor":"JetBrains","dataDirectoryName":"PyCharm2026.1","launch":[{"os":"Windows","arch":"amd64","launcherPath":"bin/pycharm64.exe"}]});
    assert!(pycharm_product(&product));
    assert!(pycharm_build_matches(&product, "PY-261.22158.340\n"));
    let mut updated = product.clone();
    updated["version"] = "2027.2.1".into();
    updated["buildNumber"] = "272.123.4".into();
    updated["dataDirectoryName"] = "PyCharm2027.2".into();
    assert!(pycharm_build_matches(&updated, "PY-272.123.4"));
    assert!(!pycharm_build_matches(&updated, "PY-261.22158.340"));
    assert!(profile_matches(
        Some("PyCharm2027.2"),
        Path::new(
            "C:/Users/Test/AppData/Local/JetBrains/PyCharm2027.2/jcef_cache/Default/Code Cache"
        )
    ));
    assert!(!profile_matches(
        Some("PyCharm2026.1"),
        Path::new(
            "C:/Users/Test/AppData/Local/JetBrains/PyCharm2027.2/jcef_cache/Default/Code Cache"
        )
    ));
    assert!(!profile_matches(
        Some("PyCharm2027.2"),
        Path::new("C:/Unrelated/PyCharm2027.2/jcef_cache")
    ));
    for malformed in [
        "../PyCharm2027.2",
        "PyCharm",
        "IntelliJ2027.2",
        "PyCharm2027.2/other",
    ] {
        assert!(!pycharm_profile(malformed));
    }
    assert!(jcef_matches(
        "JCEF_VERSION_DETAILED=150.1.2-gabc-chromium-150.0.1",
        "150.1.2.0"
    ));
    assert!(!jcef_matches(
        "JCEF_VERSION_DETAILED=150.1.2-gabc-chromium-150.0.1",
        "149.1.2.0"
    ));
    assert!(!jcef_matches("JCEF_VERSION_DETAILED=garbage", "150.1.2.0"));
    assert!(!jcef_matches(
        "JCEF_VERSION_DETAILED=150.1.2-gabc\nJCEF_VERSION_DETAILED=150.1.2-gabc",
        "150.1.2.0"
    ));
    for field in [
        "version",
        "buildNumber",
        "productCode",
        "dataDirectoryName",
        "productVendor",
    ] {
        let mut other = product.clone();
        other[field] = "unexpected".into();
        assert!(!pycharm_product(&other));
    }
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("installed-tool");
    std::fs::write(&file, b"one").unwrap();
    let expected = format!("{:x}", Sha256::digest(b"one"));
    assert!(exact_hash(&PinnedPath::open(&file, false, false).unwrap(), &expected).unwrap());
    std::fs::write(&file, b"two").unwrap();
    assert!(!exact_hash(&PinnedPath::open(&file, false, false).unwrap(), &expected).unwrap());
}
