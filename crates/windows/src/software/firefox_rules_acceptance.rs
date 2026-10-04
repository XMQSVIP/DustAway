//! One-shot real residual-rule evidence. Vendor silent removal only prepares the
//! data scenario; it is explicitly not evidence of the interactive uninstall UI.
use super::*;
use dustaway_core::software::residual_rules::{self as rules, Locator, Target};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    process::{Command, Stdio},
};

const VERSION: &str = "140.17.0.1158";
const IDS: &[&str] = &[
    "uninstall.firefox.http",
    "uninstall.firefox.preferences",
    "uninstall.firefox.user_preferences",
    "uninstall.firefox.cookies",
    "uninstall.firefox.places",
    "uninstall.firefox.storage",
    "uninstall.firefox.bookmarks",
];
const HISTORY_IDS: &[&str] = &["uninstall.firefox.bookmarks", "uninstall.firefox.sessions"];

fn status(root: &Path, prefix: &str, phase: &str) -> Result<()> {
    std::fs::write(
        root.join(format!("{prefix}-status.json")),
        serde_json::to_vec_pretty(&serde_json::json!({"phase":phase}))?,
    )?;
    println!("{phase}");
    std::io::stdout().flush()?;
    Ok(())
}

fn record<'a>(scan: &'a ResidualScanData, path: &Path) -> Result<&'a ResidualRecord> {
    scan.records
        .iter()
        .find(|r| Path::new(&r.candidate.path) == path)
        .context("expected frozen file missing")
}

fn evidence_delete(
    p: &PreparedSoftware,
    r: &ResidualRecord,
    exclusions: &[String],
    history: bool,
) -> Result<()> {
    delete_residual(p, r, exclusions, |context, prepared, record| {
        residual_scope::authorize_native_evidence(
            context,
            prepared,
            record,
            |spec, identity, version| {
                identity == "firefox"
                    && version == VERSION
                    && spec.version == 1
                    && (if history { HISTORY_IDS } else { IDS }).contains(&spec.id.as_str())
            },
        )
    })
}

// Evidence hashes only. These values never become a persisted deletion authority.
// Hardlinks in old test fixtures may be read here; the production scanner still
// refuses them. Reparse points are never traversed.
fn hashes(bases: &[&Path]) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (index, base) in bases.iter().enumerate() {
        let mut pending = vec![base.to_path_buf()];
        while let Some(path) = pending.pop() {
            let meta = std::fs::symlink_metadata(&path)?;
            use std::os::windows::fs::MetadataExt;
            ensure!(
                meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0,
                "linked test path"
            );
            if meta.is_dir() {
                for entry in std::fs::read_dir(&path)? {
                    pending.push(entry?.path());
                }
            } else {
                result.insert(
                    format!("{index}/{}", path.strip_prefix(base)?.to_string_lossy()),
                    format!("{:x}", Sha256::digest(std::fs::read(&path)?)),
                );
            }
        }
    }
    Ok(result)
}

fn key(path: &Path, bases: &[&Path]) -> Result<String> {
    for (index, base) in bases.iter().enumerate() {
        if let Ok(relative) = path.strip_prefix(base) {
            return Ok(format!("{index}/{}", relative.to_string_lossy()));
        }
    }
    bail!("file escaped dedicated profiles")
}

#[test]
#[ignore = "one-shot marked Firefox residual deletion on the dedicated ordinary-user PC"]
fn real_firefox_independent_rules() -> Result<()> {
    run(false)
}

#[test]
#[ignore = "production deletion plus reinstall protection in the marked Firefox fixture"]
fn real_firefox_production_rules() -> Result<()> {
    run(true)
}

#[test]
#[ignore = "one-shot real bookmark/session evidence in the marked ordinary-user Firefox profile"]
fn real_firefox_history_rules() -> Result<()> {
    run_scenario(false, true)
}

#[test]
#[ignore = "compiled bookmark/session permissions and reinstall protection on the dedicated PC"]
fn real_firefox_history_production_rules() -> Result<()> {
    run_scenario(true, true)
}

fn run(production: bool) -> Result<()> {
    run_scenario(production, false)
}

fn run_scenario(production: bool, history: bool) -> Result<()> {
    let prefix = if history && production {
        "firefox-history-production-20261001"
    } else if history {
        "firefox-history-rules-20261001"
    } else if production {
        "firefox-production-ini-20261001"
    } else {
        "firefox-rules-20261001"
    };
    let execute = |p: &PreparedSoftware, r: &ResidualRecord, exclusions: &[String]| {
        if production {
            WindowsSoftware.delete(p, r, exclusions)
        } else {
            evidence_delete(p, r, exclusions, history)
        }
    };
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
        "wrong root marker"
    );
    let ready: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("firefox-real-ready.json"))?)?;
    let leaf = ready["ProfileDirectory"]
        .as_str()
        .context("profile missing")?;
    ensure!(
        leaf.ends_with(".DustAwayAcceptance20260930")
            && leaf
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
        "wrong profile leaf"
    );
    let profile = roaming.join("Mozilla/Firefox/Profiles").join(leaf);
    let local_profile = local.join("Mozilla/Firefox/Profiles").join(leaf);
    ensure!(
        std::fs::read_to_string(profile.join(".dustaway-test-only"))? == "DustAwayFirefox20260930",
        "wrong profile marker"
    );
    for name in ["firefox.exe", "Un_A.exe", "FirefoxSetup.exe"] {
        ensure!(
            !security::process_running(name)?,
            "prior vendor process active"
        );
    }
    let inventory = WindowsSoftware.inventory()?;
    ensure!(inventory.warnings.is_empty(), "incomplete inventory");
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .filter(|e| e.identity.family == "firefox")
        .collect();
    ensure!(
        entries.len() == 1,
        "dedicated Firefox must be the only Firefox installation"
    );
    let prepared = WindowsSoftware.prepare(entries[0])?;
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    let install = root.join("firefox-real/application");
    let helper = install.join("uninstall/helper.exe");
    ensure!(
        context.file.as_ref() == Some(&helper)
            && context.warnings.is_empty()
            && context.verified_version.as_deref() == Some(VERSION),
        "unexpected signed installation"
    );
    ensure!(
        if history {
            (HISTORY_IDS.len()..=8).contains(&context.residual_scopes.len())
        } else {
            context.residual_scopes.len() == IDS.len()
        },
        "unexpected captured scopes"
    );
    let mut warnings = vec![];
    let classifier = residual_scope::Classifier::new(&context.residual_scopes, &mut warnings);
    ensure!(warnings.is_empty(), "scope changed");
    let specs: Vec<_> = (if history { HISTORY_IDS } else { IDS })
        .iter()
        .filter(|id| history || !production || **id != "uninstall.firefox.bookmarks")
        .map(|id| rules::rule(id).context("rule missing"))
        .collect::<Result<_>>()?;
    let scopes: Vec<_> = specs
        .iter()
        .map(|spec| {
            let base = match spec.locator {
                Locator::FirefoxLocalProfile => &local_profile,
                Locator::FirefoxRoamingProfile => &profile,
                _ => unreachable!(),
            };
            base.join(&spec.relative)
        })
        .collect();
    for (spec, path) in specs.iter().zip(&scopes) {
        ensure!(
            classifier
                .classify(path, spec.target == Target::Tree)
                .is_some_and(|(_, rule)| rule == spec),
            "independent scope not captured"
        );
        ensure!(
            rules::accepted(spec, "firefox", VERSION) == production,
            "unexpected production gate for this round"
        );
    }
    drop(classifier);
    let absent_scope = if history {
        profile.join(format!("unprepared-scope-{prefix}"))
    } else {
        profile.join("sessionstore-backups")
    };
    ensure!(
        !absent_scope.try_exists()?,
        "unexpected session scope; inspect before running"
    );
    let shared = roaming.join("Mozilla/dustaway-shared-witness.txt");
    let shared_before = std::fs::read(&shared)?;
    let before_ids: HashSet<_> = inventory
        .entries
        .iter()
        .filter(|e| e.identity != prepared.entry.identity)
        .map(|e| e.software.id.clone())
        .collect();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("{prefix}.started")))?;
    status(&root, prefix, "captured_before_vendor_removal")?;
    // New rule scope in the original fixture; an unrecognized new scope in the
    // history fixture, whose eight declared Firefox scopes already exist.
    std::fs::create_dir(&absent_scope)?;
    std::fs::write(
        absent_scope.join(".dustaway-new-scope"),
        b"test-only scope created after preparation",
    )?;
    let pin = UninstallerPin::open(&helper, false)?;
    ensure!(
        Some(pin.stamp()) == context.stamp.as_ref(),
        "uninstaller changed"
    );
    drop(pin);
    // Official documented CLI for data-test setup, NOT the production launcher or UI test.
    let mut child = Command::new(&helper)
        .arg("/S")
        .current_dir(&install)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    loop {
        if let Some(exit) = child.try_wait()? {
            ensure!(exit.success(), "vendor helper failed");
            break;
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "vendor process remains; never replay or terminate"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    while WindowsSoftware.installed(&prepared.entry.identity)?
        || security::process_running("Un_A.exe")?
    {
        ensure!(
            std::time::Instant::now() < deadline,
            "vendor removal not verified; no deletion"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    ensure!(
        !install.join("firefox.exe").try_exists()?,
        "browser remains"
    );
    let after = WindowsSoftware.inventory()?;
    ensure!(
        after.warnings.is_empty()
            && after
                .entries
                .iter()
                .map(|e| e.software.id.clone())
                .collect::<HashSet<_>>()
                == before_ids,
        "unrelated installation identities changed"
    );
    ensure!(
        std::fs::read(&shared)? == shared_before,
        "shared witness changed by vendor"
    );
    status(
        &root,
        prefix,
        "official_silent_removal_verified_residual_checks_starting",
    )?;
    let bases = [profile.as_path(), local_profile.as_path()];
    let stopped = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(true))?;
    ensure!(
        !stopped.warnings.is_empty(),
        "cancelled scan reported complete"
    );
    let scan = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
    let new_record = record(&scan, &absent_scope.join(".dustaway-new-scope"))?;
    ensure!(
        execute(&prepared, new_record, &[]).is_err(),
        "new rule scope acquired authority"
    );
    let retained_root = record(&scan, &profile)?;
    ensure!(
        execute(&prepared, retained_root, &[]).is_err(),
        "whole profile acquired authority"
    );
    if production {
        status(&root, prefix, "checking_actual_reinstallation_refusal")?;
        let actual = scan
            .records
            .iter()
            .find(|r| {
                r.candidate.selectable
                    && !r.candidate.directory
                    && (!history
                        || scopes
                            .iter()
                            .any(|scope| Path::new(&r.candidate.path).starts_with(scope)))
            })
            .context("no approved production candidate")?;
        check_reinstallation(&root, &install, &prepared, actual, prefix)?;
    }
    let mut results = vec![];
    for (spec, scope) in specs.iter().zip(&scopes) {
        status(&root, prefix, &format!("validating_{}", spec.id))?;
        let scan = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        let context: ContextData = serde_json::from_str(&prepared.context)?;
        let classifier = residual_scope::Classifier::new(&context.residual_scopes, &mut vec![]);
        let actual = scan
            .records
            .iter()
            .find(|r| {
                !r.candidate.directory
                    && Path::new(&r.candidate.path).starts_with(scope)
                    && classifier
                        .classify(Path::new(&r.candidate.path), false)
                        .is_some_and(|(_, s)| s.id == spec.id)
                    && !r.candidate.path.contains("dustaway-validation")
                    && (!history
                        || Path::new(&r.candidate.path)
                            .extension()
                            .is_some_and(|s| s == "jsonlz4"))
            })
            .context("no actual vendor file for rule")?;
        let path = PathBuf::from(&actual.candidate.path);
        ensure!(
            bases.iter().any(|base| path.starts_with(base)),
            "file escaped test profile"
        );
        ensure!(
            actual.candidate.selectable == production,
            "candidate capability differs"
        );
        if !production {
            ensure!(
                WindowsSoftware.delete(&prepared, actual, &[]).is_err(),
                "production gate opened early"
            );
        }
        let exclusion = display_path(&path);
        ensure!(
            execute(&prepared, actual, std::slice::from_ref(&exclusion)).is_err(),
            "exclusion ignored"
        );
        let excluded = WindowsSoftware.scan(&prepared, &[exclusion], &AtomicBool::new(false))?;
        ensure!(
            record(&excluded, &path).is_err() && !excluded.warnings.is_empty(),
            "scan exclusion ignored"
        );
        let original = std::fs::read(&path)?;
        ensure!(
            original.len() <= 64 * 1024 * 1024,
            "unexpected fixture file size"
        );
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&path)?;
        ensure!(
            execute(&prepared, actual, &[]).is_err(),
            "occupied file deleted"
        );
        drop(lock);
        let mut changed = original.clone();
        changed.extend_from_slice(b"dustaway-modified");
        std::fs::write(&path, &changed)?;
        ensure!(
            execute(&prepared, actual, &[]).is_err(),
            "changed file deleted"
        );
        std::fs::write(&path, &original)?;
        // Renew only the frozen scan, retaining the SAME pre-uninstall scope context.
        let fresh = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        let actual = record(&fresh, &path)?;
        let peer = profile.join(format!(".dustaway-hardlink-{}", spec.id));
        ensure!(!peer.try_exists()?, "prior hardlink witness remains");
        std::fs::hard_link(&path, &peer)?;
        ensure!(
            execute(&prepared, actual, &[]).is_err(),
            "hardlinked file deleted"
        );
        let partial = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        ensure!(
            !partial.warnings.is_empty() && record(&partial, &path).is_err(),
            "hardlink scan was not partial"
        );
        ensure!(
            std::fs::read(&peer)? == original,
            "hardlink witness changed"
        );
        std::fs::remove_file(&peer)?; // Only the exact newly created test peer, never its source.
        let fresh = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        let actual = record(&fresh, &path)?;
        let saved = path.with_file_name(format!(".dustaway-original-{}", spec.id));
        ensure!(!saved.try_exists()?, "prior replacement witness remains");
        std::fs::rename(&path, &saved)?;
        std::fs::write(&path, b"dustaway-path-replacement")?;
        ensure!(
            execute(&prepared, actual, &[]).is_err(),
            "replacement file deleted"
        );
        ensure!(
            std::fs::read(&path)? == b"dustaway-path-replacement",
            "replacement witness changed"
        );
        std::fs::remove_file(&path)?; // Restore only this verified test replacement; no recursive removal.
        std::fs::rename(&saved, &path)?;
        ensure!(std::fs::read(&path)? == original, "original not restored");
        let frozen = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        let mut selected: Vec<_> = frozen
            .records
            .iter()
            .filter(|r| {
                Path::new(&r.candidate.path).starts_with(scope)
                    && classifier
                        .classify(Path::new(&r.candidate.path), r.candidate.directory)
                        .is_some_and(|(_, s)| s.id == spec.id)
            })
            .collect();
        selected
            .sort_by_key(|r| std::cmp::Reverse(Path::new(&r.candidate.path).components().count()));
        let new_path = scope.join(format!(".dustaway-created-after-freeze-{prefix}"));
        if spec.target == Target::Tree {
            ensure!(!new_path.try_exists()?, "new-file witness exists");
            std::fs::write(&new_path, b"new file must survive the frozen deletion list")?;
        }
        let mut expected = hashes(&bases)?;
        let mut files = 0usize;
        let mut bytes = 0u64;
        let mut empty_directories = 0usize;
        let mut retained_nonempty = 0usize;
        for item in selected {
            match execute(&prepared, item, &[]) {
                Ok(()) if item.candidate.directory => empty_directories += 1,
                Ok(()) => {
                    let path = Path::new(&item.candidate.path);
                    ensure!(!path.try_exists()?, "deleted file still present");
                    ensure!(
                        expected.remove(&key(path, &bases)?).is_some(),
                        "unexpected deleted file"
                    );
                    files += 1;
                    bytes += item.candidate.bytes.parse::<u64>()?;
                }
                Err(_)
                    if item.candidate.directory
                        && std::fs::read_dir(&item.candidate.path)?.next().is_some() =>
                {
                    retained_nonempty += 1;
                }
                Err(error) => {
                    return Err(error.context(format!("unexpected refusal in {}", spec.id)));
                }
            }
        }
        ensure!(files > 0, "no actual files deleted for rule");
        ensure!(
            hashes(&bases)? == expected && std::fs::read(&shared)? == shared_before,
            "unselected data or new files changed"
        );
        if spec.target == Target::Tree {
            ensure!(
                new_path.is_file() && retained_nonempty > 0,
                "new file or nonempty root deleted"
            );
        }
        results.push(serde_json::json!({
            "rule_id":spec.id, "rule_version":spec.version, "scope_sha256":spec.scope_sha256(),
            "kind":spec.kind, "deleted_files":files, "deleted_bytes":bytes,
            "deleted_empty_directories":empty_directories, "retained_nonempty_directories":retained_nonempty,
            "excluded_occupied_changed_hardlinked_replaced_files_refused":true,
            "partial_scan_reported":true, "unselected_hashes_unchanged":true,
            "new_files_preserved":spec.target == Target::Tree,
        }));
        std::fs::write(
            root.join(format!("{prefix}-partial.json")),
            serde_json::to_vec_pretty(&results)?,
        )?;
    }
    let report = serde_json::json!({
        "passed":true, "ordinary_user":true, "signed_executable_version":VERSION,
        "same_in_memory_pre_uninstall_context":true, "context_persisted":false,
        "official_silent_uninstall":true, "interactive_launcher_test":false,
        "production_deletion_entry":production, "reinstalled_application_refused":production,
        "authority":if production {"compiled per-rule approval and WindowsSoftware.delete"} else {"test-only evidence predicate; shared production scope and deletion guards"},
        "original_identity_absent":true, "other_installation_identities_unchanged":true,
        "cancelled_scan_reported":true, "new_scope_refused":true, "whole_profile_refused":true,
        "shared_parent_unchanged":true, "rules":results,
        "missing_real_rules":if history {vec![]} else {vec!["uninstall.firefox.bookmarks","uninstall.firefox.sessions"]},
        "not_covered":["interactive UI", "cloud synchronization provider", "other versions"],
    });
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("{prefix}-report.json")))?
        .write_all(&serde_json::to_vec_pretty(&report)?)?;
    status(&root, prefix, "passed")?;
    Ok(())
}

fn run_vendor(path: &Path, args: &[String]) -> Result<()> {
    let pin = PinnedPath::open(path, false, false)?;
    ensure!(
        crate::applications::signed_by(&pin, "Mozilla Corporation"),
        "vendor signature mismatch"
    );
    drop(pin);
    let mut child = Command::new(path)
        .args(args)
        .current_dir(path.parent().context("missing parent")?)
        .creation_flags(CREATE_NO_WINDOW.0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    loop {
        if let Some(exit) = child.try_wait()? {
            ensure!(exit.success(), "vendor command failed");
            return Ok(());
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "vendor still running; no retry or termination"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn check_reinstallation(
    root: &Path,
    install: &Path,
    prepared: &PreparedSoftware,
    record: &ResidualRecord,
    prefix: &str,
) -> Result<()> {
    let installer = root.join("FirefoxSetup.exe");
    ensure!(
        format!("{:x}", Sha256::digest(std::fs::read(&installer)?))
            == "df6f7046d53ff17403e49092c0fe830e24cabd8dacbfad3225870955908fccf9",
        "installer hash mismatch"
    );
    let original = std::fs::read(&record.candidate.path)?;
    let ini = root.join(format!("{prefix}-reinstallation.ini"));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&ini)?
        .write_all(installer_configuration(install)?.as_bytes())?;
    run_vendor(
        &installer,
        &[format!("/INI={}", display_path(&extended(&ini)?))],
    )?;
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    while !WindowsSoftware.installed(&prepared.entry.identity)?
        || !install.join("firefox.exe").is_file()
    {
        ensure!(
            std::time::Instant::now() < deadline,
            "reinstallation not verified; inspect before retry"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    ensure!(
        WindowsSoftware
            .scan(prepared, &[], &AtomicBool::new(false))
            .is_err(),
        "reinstalled scan allowed"
    );
    let context: ContextData = serde_json::from_str(&prepared.context)?;
    residual_scope::authorized(&context, prepared, record)?;
    let refused = WindowsSoftware.delete(prepared, record, &[]).unwrap_err();
    ensure!(
        refused
            .downcast_ref::<Refusal>()
            .is_some_and(|reason| reason.0 == ReasonCode::Protected),
        "reinstallation was not the reason for refusal"
    );
    ensure!(
        std::fs::read(&record.candidate.path)? == original,
        "reinstalled application data changed"
    );
    run_vendor(&install.join("uninstall/helper.exe"), &["/S".into()])?;
    let deadline = std::time::Instant::now() + Duration::from_secs(180);
    while WindowsSoftware.installed(&prepared.entry.identity)?
        || security::process_running("Un_A.exe")?
    {
        ensure!(
            std::time::Instant::now() < deadline,
            "test reinstall removal not verified"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    ensure!(
        !install.join("firefox.exe").try_exists()?,
        "test reinstall binary remains"
    );
    Ok(())
}

fn installer_configuration(install: &Path) -> Result<String> {
    let install = display_path(&extended(install)?);
    ensure!(
        !install.contains(['\r', '\n', '"', '/']),
        "invalid test install configuration"
    );
    Ok(format!(
        "[Install]\r\nInstallDirectoryPath={install}\r\nMaintenanceService=false\r\nRegisterDefaultAgent=false\r\nDesktopShortcut=false\r\nStartMenuShortcuts=false\r\nTaskbarShortcut=false\r\nPreventRebootRequired=true\r\n"
    ))
}

#[test]
fn vendor_ini_normalizes_mixed_separators_without_command_line_truncation() {
    let ini = installer_configuration(Path::new(
        r"C:\Users\Fixture\DustAwayAcceptance/20260930-software/firefox-real/application",
    ))
    .unwrap();
    assert!(ini.contains("InstallDirectoryPath=C:\\Users\\Fixture\\DustAwayAcceptance\\20260930-software\\firefox-real\\application\r\n"));
    assert!(!ini.contains('/'));
    assert!(installer_configuration(Path::new("C:\\bad\nInstallDirectoryPath=x")).is_err());
}
