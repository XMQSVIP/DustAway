//! Explicit real-product checks, restricted to the dedicated marked Brave installation.
//! Diagnostic reports never contain a replayable uninstall or residual context.
mod independent_rules;
use super::*;
use dustaway_core::software::residual_rules::{self as rules, Target};
use std::{fs::OpenOptions, io::Write};

const VERSION: &str = "154.1.96.60";
const MARKER: &str = "DustAwayBrave20261001";

struct Fixture {
    root: PathBuf,
    base: PathBuf,
    profile: PathBuf,
    prepared: PreparedSoftware,
    context: ContextData,
}

fn fixture() -> Result<Fixture> {
    fixture_with_seed("brave-evidence-v3-seed-verification.json")
}

fn fixture_with_seed(seed_name: &str) -> Result<Fixture> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6" && !security::elevated()?,
        "dedicated ordinary-user host required"
    );
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let root = local.join("DustAwayAcceptance/20260930-software");
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path())
            && std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
                == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong test root"
    );
    let base = local.join("BraveSoftware");
    let user_data = base.join("Brave-Browser/User Data");
    let profile = user_data.join("Default");
    for path in [&base, &user_data, &profile] {
        let _pin = PinnedPath::open(path, true, false)?;
        ensure!(
            std::fs::read_to_string(path.join(".dustaway-test-only"))? == MARKER,
            "test marker missing"
        );
    }
    ensure!(
        !security::process_running("brave.exe")?
            && !security::process_running("BraveBrowserStandaloneSilentSetup.exe")?,
        "vendor process remains; inspect original handle"
    );
    let inventory = WindowsSoftware.inventory()?;
    ensure!(inventory.warnings.is_empty(), "incomplete inventory");
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .filter(|entry| entry.identity.family == "brave")
        .collect();
    ensure!(entries.len() == 1, "expected only the dedicated Brave");
    let prepared = WindowsSoftware.prepare(entries[0])?;
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    let install = base.join("Brave-Browser/Application");
    ensure!(
        context.file.as_ref() == Some(&install.join(VERSION).join("Installer/setup.exe"))
            && context.verified_version.as_deref() == Some(VERSION)
            && context.rule_version == RESIDUAL_RULE_VERSION,
        "unexpected signed installation identity or version"
    );
    ensure!(
        context.warnings.is_empty(),
        "scope warnings: {:?}",
        context.warnings
    );
    let seed: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(seed_name))?)?;
    ensure!(
        seed["passed"] == true
            && seed["signedVersion"] == VERSION
            && seed["twoGracefulExits"] == true
            && seed["scopes"]
                .as_array()
                .is_some_and(|items| items.len() == 11),
        "real browser data generation incomplete"
    );
    Ok(Fixture {
        root,
        base,
        profile,
        prepared,
        context,
    })
}

#[test]
#[ignore = "one-shot read-only real Brave preflight on the marked acceptance PC"]
fn real_brave_scope_preflight() -> Result<()> {
    let fixture = fixture()?;
    let mut warnings = vec![];
    let classifier =
        residual_scope::Classifier::new(&fixture.context.residual_scopes, &mut warnings);
    ensure!(warnings.is_empty(), "captured scope changed");
    let captured = serde_json::to_value(&fixture.context.residual_scopes)?;
    let captured = captured.as_array().context("scope array missing")?;
    let specs: Vec<_> = rules::catalog()
        .into_iter()
        .filter(|spec| spec.family == "brave")
        .collect();
    ensure!(
        captured.len() == 11 && specs.len() == 11,
        "missing or extra profile scopes"
    );
    let mut observed = vec![];
    for spec in &specs {
        let scope = fixture.profile.join(&spec.relative);
        let matching: Vec<_> = captured
            .iter()
            .filter(|entry| entry["rule_id"] == spec.id)
            .collect();
        ensure!(matching.len() == 1, "ambiguous scope: {}", spec.id);
        let path: PathBuf = serde_json::from_value(matching[0]["path"].clone())?;
        ensure!(
            path == scope && matching[0]["scope_sha256"] == spec.scope_sha256(),
            "scope differs"
        );
        ensure!(
            classifier
                .classify(&scope, spec.target == Target::Tree)
                .is_some_and(|(_, found)| found == spec),
            "independent classification failed: {}",
            spec.id
        );
        let _pin = PinnedPath::open(&scope, spec.target == Target::Tree, false)?;
        ensure!(
            !rules::accepted(spec, "brave", VERSION),
            "preflight cannot approve deletion"
        );
        observed.push(serde_json::json!({
            "rule_id":spec.id, "rule_version":spec.version, "scope_sha256":spec.scope_sha256(),
            "kind":spec.kind, "relative":spec.relative, "target":spec.target,
            "independently_captured":true, "production_deletion_approved":false,
        }));
    }
    for (path, directory) in [
        (fixture.base.clone(), true),
        (fixture.profile.clone(), true),
        (fixture.base.join("dustaway-shared-witness.txt"), false),
        (fixture.profile.join("unrecognized/Cache/entry"), false),
    ] {
        ensure!(
            classifier.classify(&path, directory).is_none(),
            "shared or unknown path classified"
        );
    }
    ensure!(
        WindowsSoftware
            .scan(&fixture.prepared, &[], &AtomicBool::new(false))
            .is_err(),
        "installed software admitted residual scan"
    );
    let report = serde_json::json!({
        "passed":true, "ordinary_user":true, "stage":"pre_uninstall_scope_preflight",
        "signed_executable_version":VERSION, "rules":observed,
        "shared_parent_and_whole_profile_excluded":true, "nested_cache_name_excluded":true,
        "installed_software_scan_refused":true, "uninstall_executed":false,
        "residual_deletion_executed":false, "production_deletion_approved":false,
        "authority_persisted":false,
    });
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(fixture.root.join("brave-scope-preflight-20261001.json"))?
        .write_all(&serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
