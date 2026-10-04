//! Opt-in real Firefox acceptance, limited to a newly created, marked profile on the spare PC.
//! This module is absent from production. It never opens a production deletion gate.
use super::*;
use std::{collections::BTreeMap, fs::OpenOptions, os::windows::fs::OpenOptionsExt};

fn hashes(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        let meta = std::fs::symlink_metadata(&path)?;
        let _pin = PinnedPath::open(&path, meta.is_dir(), false)?;
        if meta.is_dir() {
            for child in std::fs::read_dir(path)? {
                pending.push(child?.path());
            }
        } else {
            out.insert(
                path.strip_prefix(root)?.to_string_lossy().into_owned(),
                format!("{:x}", Sha256::digest(std::fs::read(&path)?)),
            );
        }
    }
    Ok(out)
}

#[test]
#[ignore = "requires the dedicated acceptance PC, signed Firefox and a fresh marked profile"]
fn real_firefox_uninstall_residual_protection() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    ensure!(!security::elevated()?, "ordinary token required");
    let root = std::env::current_exe()?
        .parent()
        .context("no root")?
        .to_owned();
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    ensure!(
        root == local.join(r"DustAwayAcceptance\20260930-software"),
        "wrong root"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("firefox-real-ready.json"))?)?;
    let profile_name = ready["ProfileDirectory"]
        .as_str()
        .context("missing profile")?;
    ensure!(
        profile_name.ends_with(".DustAwayAcceptance20260930")
            && !profile_name.contains(['/', '\\']),
        "wrong profile"
    );
    let profile = roaming.join("Mozilla/Firefox/Profiles").join(profile_name);
    let cache = local
        .join("Mozilla/Firefox/Profiles")
        .join(profile_name)
        .join("cache2");
    ensure!(
        std::fs::read_to_string(profile.join(".dustaway-test-only"))? == "DustAwayFirefox20260930",
        "profile not marked"
    );
    ensure!(
        cache.is_dir() && !security::process_running("firefox.exe")?,
        "browser not ready"
    );
    let expected_install = root.join("firefox-real/application");
    let inventory = WindowsSoftware.inventory()?;
    ensure!(inventory.warnings.is_empty(), "incomplete inventory");
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .filter(|e| e.identity.family == "firefox")
        .collect();
    ensure!(
        entries.len() == 1,
        "expected exactly one test Firefox installation"
    );
    let entry = entries[0];
    let prepared = WindowsSoftware.prepare(entry)?;
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    ensure!(
        context.file.as_ref() == Some(&expected_install.join("uninstall/helper.exe")),
        "uninstaller outside test installation"
    );
    ensure!(
        !context.args.to_lowercase().contains("/s"),
        "acceptance must preserve interactive uninstall"
    );
    ensure!(
        context.warnings.is_empty() && context.roots.len() == 2,
        "ownership or roots were not verified"
    );
    let expected_roots: HashSet<_> = [
        local.join("Mozilla/Firefox/Profiles"),
        roaming.join("Mozilla/Firefox"),
    ]
    .into_iter()
    .collect();
    ensure!(
        context
            .roots
            .iter()
            .all(|r| expected_roots.contains(&r.path)),
        "unexpected residual root"
    );
    let before_ids: HashSet<_> = inventory
        .entries
        .iter()
        .filter(|e| e.software.id != entry.software.id)
        .map(|e| e.software.id.clone())
        .collect();
    let data_before = hashes(&roaming.join("Mozilla/Firefox"))?;
    ensure!(
        data_before.keys().any(|p| p.ends_with("cookies.sqlite")),
        "no actual cookie data"
    );
    let shared = roaming.join("Mozilla/dustaway-shared-witness.txt");
    let shared_before = std::fs::read(&shared)?;
    let witness = cache.join("dustaway-validation");
    let retry = std::env::var("DUSTAWAY_FIREFOX_RETRY").is_ok_and(|v| v == "launcher-check");
    let second_cycle = std::env::var("DUSTAWAY_FIREFOX_RETRY").is_ok_and(|v| v == "second-cycle");
    if retry || second_cycle {
        ensure!(
            std::fs::read_to_string(root.join(if second_cycle {
                "firefox-residual-retry-error.log"
            } else {
                "firefox-residual-first-error.log"
            }))?
            .trim()
                == if second_cycle {
                    "Error: official uninstall changed protected profile data"
                } else {
                    "Error: installation registration remains"
                },
            "unexpected prior failure; do not retry"
        );
        if second_cycle {
            let retained: serde_json::Value = serde_json::from_slice(&std::fs::read(
                root.join("firefox-retained-verification.json"),
            )?)?;
            ensure!(
                retained["passed"] == true,
                "reinstalled profile was not verified"
            );
        }
        for name in ["modified", "excluded", "locked", "hardlinked"] {
            ensure!(
                std::fs::read(witness.join(name))? == b"test-only frozen witness",
                "prior test progressed or witness changed"
            );
        }
        ensure!(
            !witness.join("new-after-freeze").exists() && !witness.join("hardlink-peer").exists(),
            "prior deletion phase already began"
        );
    } else {
        std::fs::create_dir(&witness)?;
        for name in ["modified", "excluded", "locked", "hardlinked"] {
            std::fs::write(witness.join(name), b"test-only frozen witness")?;
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(if second_cycle {
            "firefox-residual-second-cycle.started"
        } else if retry {
            "firefox-residual-retry.started"
        } else {
            "firefox-residual.started"
        }))?;
    std::fs::write(
        root.join("firefox-before-official-uninstall.json"),
        serde_json::to_vec_pretty(&data_before)?,
    )?;
    std::fs::write(
        root.join("firefox-residual-status.json"),
        br#"{"phase":"waiting_for_firefox_uninstall_wizard"}"#,
    )?;
    if !retry {
        ensure!(
            matches!(
                WindowsSoftware.launch(&prepared, &mut |_| {})?,
                LaunchOutcome::Completed
            ),
            "uninstall did not complete"
        );
    }
    // Vendor launchers can return before the actual uninstaller finishes. Keep the
    // test session alive and observe the original identity; never relaunch in this loop.
    let deadline = std::time::Instant::now() + Duration::from_secs(600);
    while WindowsSoftware.installed(&entry.identity)? {
        ensure!(
            std::time::Instant::now() < deadline,
            "installation registration remains; no retry or forced stop"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    while security::process_running("Un_A.exe")? {
        ensure!(
            std::time::Instant::now() < deadline,
            "official uninstaller still running; no forced stop"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    ensure!(
        !expected_install.join("firefox.exe").exists(),
        "browser binary still installed"
    );
    let after_inventory = WindowsSoftware.inventory()?;
    ensure!(
        after_inventory.warnings.is_empty(),
        "incomplete post-uninstall inventory"
    );
    ensure!(
        before_ids
            == after_inventory
                .entries
                .iter()
                .map(|e| e.software.id.clone())
                .collect(),
        "unrelated software identities changed"
    );
    let data_after_vendor = hashes(&roaming.join("Mozilla/Firefox"))?;
    let mut vendor_changes = std::collections::BTreeSet::new();
    for path in data_before.keys().chain(data_after_vendor.keys()) {
        if data_before.get(path) != data_after_vendor.get(path) {
            vendor_changes.insert(path.clone());
        }
    }
    std::fs::write(
        root.join("firefox-official-uninstall-diff.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"changed_paths":vendor_changes,"after":data_after_vendor}),
        )?,
    )?;
    ensure!(
        profile.join("cookies.sqlite").is_file() && profile.join("prefs.js").is_file(),
        "official uninstall removed the test profile; no cache deletion"
    );
    ensure!(
        std::fs::read(&shared)? == shared_before,
        "shared parent changed"
    );
    check_residuals(
        &root,
        &prepared,
        profile_name,
        data_after_vendor,
        serde_json::to_value(vendor_changes)?,
        serde_json::json!({
            "real_official_uninstall":true,
            "continued_existing_uninstaller":retry,
            "new_cycle_after_verified_reinstall":second_cycle,
            "unrelated_installations_unchanged":true,
            "authority":"captured before the tracked uninstall"
        }),
    )
}

fn check_residuals(
    root: &Path,
    prepared: &PreparedSoftware,
    profile_name: &str,
    data_after_vendor: BTreeMap<String, String>,
    vendor_changes: serde_json::Value,
    provenance: serde_json::Value,
) -> Result<()> {
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    let cache = local
        .join("Mozilla/Firefox/Profiles")
        .join(profile_name)
        .join("cache2");
    let witness = cache.join("dustaway-validation");
    let shared = roaming.join("Mozilla/dustaway-shared-witness.txt");
    let shared_before = std::fs::read(&shared)?;
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    ensure!(
        hashes(&roaming.join("Mozilla/Firefox"))? == data_after_vendor,
        "profile changed before scan"
    );
    let scan = WindowsSoftware.scan(prepared, &[], &AtomicBool::new(false))?;
    ensure!(
        scan.supported && scan.warnings.is_empty() && !scan.records.is_empty(),
        "residual scan incomplete"
    );
    ensure!(
        scan.records.iter().all(|r| !r.candidate.selectable),
        "production selection gate opened"
    );
    ensure!(
        !scan
            .records
            .iter()
            .any(|r| Path::new(&r.candidate.path) == shared),
        "shared Mozilla parent entered scan"
    );
    let record = |path: &Path| -> Result<&ResidualRecord> {
        scan.records
            .iter()
            .find(|r| Path::new(&r.candidate.path) == path)
            .context("missing frozen record")
    };
    let actual_cache = scan
        .records
        .iter()
        .find(|r| {
            !r.candidate.directory
                && Path::new(&r.candidate.path).starts_with(cache.join("entries"))
        })
        .context("no real browser cache entries")?;
    let actual_path = PathBuf::from(&actual_cache.candidate.path);
    ensure!(
        WindowsSoftware.delete(prepared, actual_cache, &[]).is_err(),
        "production deletion gate opened"
    );
    let excluded_path = witness.join("excluded");
    let exclusion = display_path(&excluded_path);
    let filtered = WindowsSoftware.scan(
        prepared,
        std::slice::from_ref(&exclusion),
        &AtomicBool::new(false),
    )?;
    ensure!(
        !filtered
            .records
            .iter()
            .any(|r| r.candidate.path == exclusion)
            && !filtered.warnings.is_empty(),
        "exclusion not preserved"
    );
    ensure!(
        delete_captured(&context, record(&excluded_path)?, &[exclusion]).is_err(),
        "excluded file deleted"
    );
    let mut changed = std::fs::read(witness.join("modified"))?;
    changed.extend_from_slice(b"\nchanged after the frozen manifest");
    std::fs::write(witness.join("modified"), changed)?;
    ensure!(
        delete_captured(&context, record(&witness.join("modified"))?, &[]).is_err(),
        "changed file deleted"
    );
    let locked = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(witness.join("locked"))?;
    ensure!(
        delete_captured(&context, record(&witness.join("locked"))?, &[]).is_err(),
        "occupied file deleted"
    );
    // An exclusive content handle need not deny FILE_READ_ATTRIBUTES. Its
    // delete refusal above is the contract; do not demand a metadata-scan failure.
    drop(locked);
    std::fs::hard_link(witness.join("hardlinked"), witness.join("hardlink-peer"))?;
    ensure!(
        delete_captured(&context, record(&witness.join("hardlinked"))?, &[]).is_err(),
        "hardlinked file deleted"
    );
    let partial = WindowsSoftware.scan(prepared, &[], &AtomicBool::new(false))?;
    ensure!(
        !partial.warnings.is_empty()
            && !partial.records.iter().any(|r| {
                [witness.join("hardlinked"), witness.join("hardlink-peer")]
                    .iter()
                    .any(|p| Path::new(&r.candidate.path) == p)
            }),
        "partial scan did not preserve hardlinked files"
    );
    std::fs::write(witness.join("new-after-freeze"), b"must not be deleted")?;
    ensure!(
        delete_captured(&context, record(&witness)?, &[]).is_err(),
        "nonempty directory deleted"
    );
    let stopped = WindowsSoftware.scan(prepared, &[], &AtomicBool::new(true))?;
    ensure!(
        !stopped.warnings.is_empty(),
        "stopped scan reported complete"
    );
    // Test-only authority: exactly one frozen cache entry from this new marked profile.
    // Settings, profile roots and other installations never reach this private executor.
    ensure!(
        actual_path.starts_with(cache.join("entries")),
        "cache boundary changed"
    );
    ensure!(
        !WindowsSoftware.installed(&prepared.entry.identity)?
            && !security::process_running("firefox.exe")?,
        "Firefox was reinstalled or restarted before the test-only deletion"
    );
    ensure!(
        !desktop_inventory()?
            .iter()
            .any(|r| family(&r.name) == Some("firefox")),
        "Firefox family was reinstalled"
    );
    delete_captured(&context, actual_cache, &[])?;
    ensure!(!actual_path.exists(), "frozen cache file was not removed");
    ensure!(
        hashes(&roaming.join("Mozilla/Firefox"))? == data_after_vendor
            && std::fs::read(&shared)? == shared_before,
        "protected data changed"
    );
    for name in [
        "modified",
        "excluded",
        "locked",
        "hardlinked",
        "new-after-freeze",
    ] {
        ensure!(witness.join(name).exists(), "witness disappeared");
    }
    let report = serde_json::json!({
        "result":"passed", "version":prepared.entry.software.version, "ordinary_user":true,
        "registration_absent":true, "provenance":provenance,
        "production_deletion_enabled":false, "scope":"one frozen real cache entry via test-only access to the private executor; no production rule approval",
        "deleted_cache_files":1, "deleted_cache_bytes":actual_cache.candidate.bytes,
        "scanned_records":scan.records.len(), "profile_data_unchanged":true, "profile_preservation_baseline":"after official uninstall, before DustAway cache deletion", "shared_parent_preserved":true,
        "official_uninstaller_changed_paths":vendor_changes,
        "exclusion_preserved":true, "changed_file_preserved":true, "occupied_file_preserved":true,
        "hardlink_preserved":true, "new_file_preserved":true, "nonempty_directory_preserved":true,
        "partial_scan_reported":true, "partial_scan_trigger":"hardlinks; exclusive content lock does not require metadata scan failure", "stopped_scan_reported":true,
        "protected_hashes":data_after_vendor
    });
    std::fs::write(
        root.join("firefox-residual-verification.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    std::fs::write(
        root.join("firefox-residual-status.json"),
        br#"{"phase":"passed"}"#,
    )?;
    Ok(())
}

#[test]
#[ignore = "one-shot supplement on the dedicated PC after the recorded second-cycle assertion failure"]
fn real_firefox_residual_supplement() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    ensure!(!security::elevated()?, "ordinary token required");
    let root = std::env::current_exe()?
        .parent()
        .context("no root")?
        .to_owned();
    let local = known_folder(&FOLDERID_LocalAppData)?;
    let roaming = known_folder(&FOLDERID_RoamingAppData)?;
    ensure!(
        root == local.join(r"DustAwayAcceptance\20260930-software"),
        "wrong root"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    ensure!(
        std::fs::read_to_string(root.join("firefox-residual-second-error.log"))?.trim()
            == "Error: partial scan did not report occupied file",
        "unexpected prior failure"
    );
    let prior_text = std::fs::read_to_string(root.join("firefox-residual-second-result.json"))?;
    let prior: serde_json::Value = serde_json::from_str(prior_text.trim_start_matches('\u{feff}'))?;
    ensure!(prior["ExitCode"] == 101, "unexpected prior result");
    ensure!(
        !root.join("firefox-real/application/firefox.exe").exists()
            && !root.join("firefox-residual-verification.json").exists(),
        "prior phase already progressed"
    );
    ensure!(
        !security::process_running("firefox.exe")? && !security::process_running("Un_A.exe")?,
        "vendor process remains"
    );
    let inventory = WindowsSoftware.inventory()?;
    ensure!(
        inventory.warnings.is_empty()
            && !inventory
                .entries
                .iter()
                .any(|e| e.identity.family == "firefox"),
        "Firefox exists or inventory incomplete"
    );
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("firefox-real-ready.json"))?)?;
    let profile_name = ready["ProfileDirectory"]
        .as_str()
        .context("missing profile")?;
    ensure!(
        profile_name == "q27dwpy5.DustAwayAcceptance20260930",
        "wrong dedicated profile"
    );
    let profile = roaming.join("Mozilla/Firefox/Profiles").join(profile_name);
    ensure!(
        std::fs::read_to_string(profile.join(".dustaway-test-only"))? == "DustAwayFirefox20260930",
        "profile not marked"
    );
    let witness = local
        .join("Mozilla/Firefox/Profiles")
        .join(profile_name)
        .join("cache2/dustaway-validation");
    ensure!(
        std::fs::read(witness.join("modified"))? == b"changed after the frozen manifest",
        "unexpected modified witness"
    );
    for name in ["excluded", "locked", "hardlinked"] {
        ensure!(
            std::fs::read(witness.join(name))? == b"test-only frozen witness",
            "witness changed"
        );
    }
    ensure!(
        !witness.join("new-after-freeze").exists() && !witness.join("hardlink-peer").exists(),
        "prior deletion phase progressed"
    );
    ensure!(
        std::fs::read_to_string(roaming.join("Mozilla/dustaway-shared-witness.txt"))?
            == "must remain outside Firefox residual roots",
        "shared witness changed"
    );
    let diff: serde_json::Value = serde_json::from_slice(&std::fs::read(
        root.join("firefox-official-uninstall-diff.json"),
    )?)?;
    let baseline: BTreeMap<String, String> = serde_json::from_value(diff["after"].clone())?;
    ensure!(
        hashes(&roaming.join("Mozilla/Firefox"))? == baseline,
        "profile changed since official uninstall"
    );

    // This is a new, test-only scan context, not recovery of the prior production
    // authorization. No registry entry is written, and no executable can be launched.
    let entry = Registration {
        hive: "user".into(),
        view: KEY_WOW64_64KEY.0,
        key: r"Software\DustAway\Acceptance\NeverInstalledFirefoxResidualSupplement20260930".into(),
        name: "Mozilla Firefox acceptance scan supplement".into(),
        publisher: "test-only synthetic scan identity".into(),
        version: "140.17.0esr".into(),
        command: String::new(),
        location: None,
        size: None,
        date: None,
        msi: false,
        no_remove: true,
    }
    .entry();
    let (captured_roots, warnings) = roots(&entry)?;
    ensure!(
        captured_roots.len() == 2 && warnings.is_empty(),
        "cannot capture standard roots"
    );
    let context = ContextData {
        rule_version: RESIDUAL_RULE_VERSION,
        file: None,
        args: String::new(),
        stamp: None,
        roots: captured_roots,
        processes: vec!["firefox.exe".into()],
        warnings,
        residual_scopes: vec![],
        verified_version: None,
        store_isolation: None,
    };
    let prepared = PreparedSoftware {
        entry,
        fingerprint: "test-only; no executable".into(),
        context: serde_json::to_string(&context)?,
    };
    ensure!(
        !WindowsSoftware.installed(&prepared.entry.identity)?,
        "synthetic identity unexpectedly exists"
    );
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("firefox-residual-supplement.started"))?;
    check_residuals(
        &root,
        &prepared,
        profile_name,
        baseline,
        diff["changed_paths"].clone(),
        serde_json::json!({
            "supplement_only":true,
            "original_end_to_end_result":"failed; firefox-residual-second-result.json retained unchanged",
            "uninstall_launched":false,
            "authority":"fresh test-only scan context after verified uninstall; original production authorization not recovered",
            "real_official_uninstall_evidence":"separate second-cycle run before the occupied-file scan assertion",
        }),
    )
}
