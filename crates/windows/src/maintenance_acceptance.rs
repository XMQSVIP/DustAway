//! Explicit opt-in acceptance using real tools against isolated development fixtures only.
use super::*;
use sha2::{Digest, Sha256};

fn official_fixture(kind: &str, action: Action) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/rule-refresh-20261002/tool-fixtures")
        .canonicalize()
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("manifest.json")).unwrap()).unwrap();
    let row = manifest
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == kind)
        .unwrap();
    let root = PathBuf::from(row["root"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert!(root.starts_with(&base));
    let result_path = base.join(format!("{kind}-result.json"));
    let mut result = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(result_path)
        .expect("Never replay fixture cleanup; preserve its first receipt");
    let mut tool = Tool {
        action,
        exe: row["exe"].as_str().unwrap().into(),
        entry: row["entry"].as_str().map(PathBuf::from),
        version: String::new(),
    };
    let cancel = AtomicBool::new(false);
    tool.version = tool
        .probe(
            &known_folder(&FOLDERID_Profile).unwrap(),
            if action == Action::Go {
                &["version"]
            } else {
                &["--version"]
            },
            &cancel,
        )
        .unwrap();
    assert!(supported(action, &tool.version));
    tool.validate_layout(&root, &cancel).unwrap();
    let before = crate::uv_cache::inventory(&root, &cancel, Duration::from_secs(30)).unwrap();
    assert!(!before.is_empty());
    let check_protected = || {
        for item in row["protected"].as_array().unwrap() {
            let path = Path::new(item["path"].as_str().unwrap());
            let actual = format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()));
            assert_eq!(
                actual,
                item["sha256"].as_str().unwrap(),
                "protected: {}",
                path.display()
            );
        }
    };
    check_protected();
    let root_pin = PinnedPath::open(&root, true, false).unwrap();
    let (ok, output) =
        run_bounded(&mut tool.cleanup(&root), &cancel, Duration::from_secs(120)).unwrap();
    let after = crate::uv_cache::inventory(&root, &cancel, Duration::from_secs(30)).ok();
    use std::io::Write;
    result.write_all(&serde_json::to_vec_pretty(&serde_json::json!({"kind":kind,"version":tool.version,"ok":ok,"output":output,"before_files":before.len(),"after_files":after.as_ref().map(|a|a.len()),"protection_checked_after_receipt":true})).unwrap()).unwrap();
    check_protected();
    if let Some(removed) = row["removed"].as_array() {
        for file in removed {
            assert!(
                !Path::new(file.as_str().unwrap()).exists(),
                "old cache retained"
            );
        }
    }
    if action == Action::Go || action == Action::Yarn {
        assert!(
            after.as_ref().unwrap().len() < before.len(),
            "cache did not shrink"
        );
    }
    drop(root_pin);
    assert!(ok, "official tool failed: {output}");
}
#[test]
#[ignore = "Requires fresh isolated fixtures from scripts/rule-refresh/prepare-tools.mjs"]
fn official_pnpm_preserves_project_dependencies() {
    official_fixture("pnpm", Action::Pnpm);
}
#[test]
#[ignore = "Requires fresh isolated fixtures from scripts/rule-refresh/prepare-tools.mjs"]
fn official_go_preserves_modules_tools_and_fuzz() {
    official_fixture("go", Action::Go);
}
#[test]
#[ignore = "Requires fresh isolated fixtures from scripts/rule-refresh/prepare-tools.mjs"]
fn official_yarn_preserves_projects_and_pnp() {
    official_fixture("yarn", Action::Yarn);
}
#[test]
#[ignore = "Requires fresh isolated fixtures from scripts/rule-refresh/prepare-tools.mjs"]
fn official_ccache_preserves_recent_and_configuration() {
    official_fixture("ccache", Action::Ccache);
}
