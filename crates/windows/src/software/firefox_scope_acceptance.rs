//! Opt-in preparation evidence only. No launcher or deletion API is called here.
use super::*;
use dustaway_core::software::residual_rules::{self as rules, Locator, Target};

#[test]
#[ignore = "requires the marked Firefox installation on the dedicated acceptance PC"]
fn real_firefox_v2_scope_preflight() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    ensure!(!security::elevated()?, "ordinary token required");
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    let root = local.join("DustAwayAcceptance/20260930-software");
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "wrong executable scope"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("firefox-real-ready.json"))?)?;
    let leaf = ready["ProfileDirectory"]
        .as_str()
        .context("profile leaf missing")?;
    ensure!(
        leaf.ends_with(".DustAwayAcceptance20260930")
            && leaf
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
        "wrong profile leaf"
    );
    let profile = roaming.join("Mozilla/Firefox/Profiles").join(leaf);
    let cache_profile = local.join("Mozilla/Firefox/Profiles").join(leaf);
    ensure!(
        std::fs::read_to_string(profile.join(".dustaway-test-only"))? == "DustAwayFirefox20260930",
        "profile marker missing"
    );
    for process in ["firefox.exe", "Un_A.exe", "FirefoxSetup.exe"] {
        ensure!(
            !security::process_running(process)?,
            "Firefox process remains active"
        );
    }
    let inventory = WindowsSoftware.inventory()?;
    ensure!(
        inventory.warnings.is_empty(),
        "incomplete software inventory"
    );
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .filter(|entry| entry.identity.family == "firefox")
        .collect();
    ensure!(
        entries.len() == 1,
        "expected only the dedicated Firefox installation"
    );
    let prepared = WindowsSoftware.prepare(entries[0])?;
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    ensure!(
        context.file.as_ref() == Some(&root.join("firefox-real/application/uninstall/helper.exe")),
        "uninstaller is outside the test installation"
    );
    ensure!(context.rule_version == 2, "unexpected context version");
    ensure!(
        context.warnings.is_empty(),
        "scope preparation warnings: {:?}",
        context.warnings
    );
    let version = context
        .verified_version
        .as_deref()
        .context("signed executable version unavailable")?;
    ensure!(
        version == "140.17.0.1158",
        "unexpected Firefox version: {version}"
    );
    let captured = serde_json::to_value(&context.residual_scopes)?;
    let captured = captured.as_array().context("scope array missing")?;
    let mut warnings = vec![];
    let classifier = residual_scope::Classifier::new(&context.residual_scopes, &mut warnings);
    ensure!(
        warnings.is_empty(),
        "captured scopes changed before verification"
    );
    let mut observed = vec![];
    for rule in rules::catalog()
        .into_iter()
        .filter(|rule| rule.family == "firefox")
    {
        let base = match rule.locator {
            Locator::FirefoxLocalProfile => &cache_profile,
            Locator::FirefoxRoamingProfile => &profile,
            _ => bail!("unexpected Firefox locator"),
        };
        let path = base.join(&rule.relative);
        let matching: Vec<_> = captured
            .iter()
            .filter(|scope| scope["rule_id"] == rule.id)
            .collect();
        let present = path.try_exists()?;
        if present {
            ensure!(
                matching.len() == 1,
                "scope capture count differs: {}",
                rule.id
            );
            let stored_path: PathBuf = serde_json::from_value(matching[0]["path"].clone())?;
            ensure!(
                stored_path == path && matching[0]["scope_sha256"] == rule.scope_sha256(),
                "captured scope differs from independent rule"
            );
            ensure!(
                classifier
                    .classify(&path, rule.target == Target::Tree)
                    .is_some_and(|(_, matched)| matched.id == rule.id),
                "independent classification failed: {}",
                rule.id
            );
            let _pin = PinnedPath::open(&path, rule.target == Target::Tree, false)?;
        } else {
            ensure!(matching.is_empty(), "missing scope was captured");
        }
        ensure!(
            !rules::accepted(&rule, "firefox", version),
            "preflight must not enable production deletion"
        );
        observed.push(serde_json::json!({
            "rule_id":rule.id, "rule_version":rule.version, "scope_sha256":rule.scope_sha256(),
            "kind":rule.kind, "relative":rule.relative, "target":rule.target,
            "present":present, "independently_captured":present, "production_deletion_approved":false,
        }));
    }
    ensure!(
        captured.len()
            == observed
                .iter()
                .filter(|rule| rule["present"] == true)
                .count(),
        "unexpected extra profile scope captured"
    );
    for required in [
        "uninstall.firefox.http",
        "uninstall.firefox.preferences",
        "uninstall.firefox.cookies",
        "uninstall.firefox.places",
        "uninstall.firefox.storage",
    ] {
        ensure!(
            observed
                .iter()
                .any(|rule| rule["rule_id"] == required && rule["present"] == true),
            "required real scope absent: {required}"
        );
    }
    ensure!(
        classifier
            .classify(&roaming.join("Mozilla/dustaway-shared-witness.txt"), false)
            .is_none(),
        "shared parent classified"
    );
    ensure!(
        classifier
            .classify(&profile.join("not-a-rule/Cache/entry"), false)
            .is_none(),
        "arbitrary Cache name classified"
    );
    ensure!(
        WindowsSoftware
            .scan(&prepared, &[], &AtomicBool::new(false))
            .is_err(),
        "installed software incorrectly admitted residual scan"
    );
    // No PreparedSoftware/context, executable command or file-identity authority is persisted.
    let report = serde_json::json!({
        "passed":true, "stage":"pre_uninstall_scope_preflight", "ordinary_token":true,
        "context_version":context.rule_version, "signed_executable_version":version,
        "rules":observed, "shared_parent_excluded":true, "nested_cache_name_excluded":true,
        "installed_software_scan_refused":true, "uninstall_executed":false,
        "residual_deletion_executed":false, "production_deletion_approved":false,
        "authority_persisted":false,
    });
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("firefox-v2-scope-preflight.json"))?;
    output.write_all(&serde_json::to_vec_pretty(&report)?)?;
    Ok(())
}
