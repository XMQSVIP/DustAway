//! Real, one-shot evidence on the marked spare PC. The standard launcher keeps
//! its interactive wizard. The evidence predicate exists only in test builds.
use super::*;
use std::{collections::BTreeMap, os::windows::fs::OpenOptionsExt};

const PREFIX: &str = "brave-rules-v2-20261001";

fn status(root: &Path, phase: &str) -> Result<()> {
    std::fs::write(
        root.join(format!("{PREFIX}-status.json")),
        serde_json::to_vec(&serde_json::json!({"phase":phase}))?,
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

fn execute(p: &PreparedSoftware, r: &ResidualRecord, exclusions: &[String]) -> Result<()> {
    delete_residual(p, r, exclusions, |context, prepared, record| {
        residual_scope::authorize_native_evidence(context, prepared, record, |spec, id, version| {
            id == "brave" && version == VERSION && spec.family == "brave" && spec.version == 1
        })
    })
}

// Diagnostics only; never deserialize these hashes as deletion authority.
fn hashes(base: &Path) -> Result<BTreeMap<String, String>> {
    use std::os::windows::fs::MetadataExt;
    let mut out = BTreeMap::new();
    let mut pending = vec![base.to_path_buf()];
    let mut total = 0u64;
    let mut visited = 0;
    while let Some(path) = pending.pop() {
        visited += 1;
        ensure!(visited <= 30_000, "unexpected test profile size");
        let meta = std::fs::symlink_metadata(&path)?;
        ensure!(
            meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0,
            "linked test path"
        );
        if meta.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        } else {
            total += meta.len();
            ensure!(
                meta.len() <= 64 * 1024 * 1024 && total <= 512 * 1024 * 1024,
                "unexpected test data size"
            );
            out.insert(
                path.strip_prefix(base)?.to_string_lossy().into_owned(),
                format!("{:x}", Sha256::digest(std::fs::read(&path)?)),
            );
        }
    }
    Ok(out)
}

fn new_file(path: &Path, bytes: &[u8]) -> Result<()> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn verify_real_seed(f: &Fixture) -> Result<()> {
    let bytes = std::fs::read(f.root.join("brave-evidence-v4-seed-verification.json"))?;
    ensure!(
        format!("{:x}", Sha256::digest(&bytes))
            == "f4cb0f83712b0b5d507336ec314ead306a43018554c06533bef5edf6c86ba853",
        "original browser seed evidence changed"
    );
    let seed: serde_json::Value = serde_json::from_slice(&bytes)?;
    let scopes = seed["scopes"].as_array().context("missing seed scopes")?;
    for spec in rules::catalog().into_iter().filter(|s| s.family == "brave") {
        let scope = scopes
            .iter()
            .find(|s| s["scope"] == spec.relative)
            .context("missing real scope")?;
        let files = scope["files"].as_array().context("missing real files")?;
        ensure!(!files.is_empty(), "empty real scope");
        for file in files {
            let name = file["name"].as_str().context("missing relative name")?;
            ensure!(
                Path::new(name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
                "invalid relative seed name"
            );
            let mut path = f.profile.join(&spec.relative);
            if !name.is_empty() {
                path.push(name);
            }
            let pin = PinnedPath::open(&path, false, false)?;
            drop(pin);
            let content = std::fs::read(&path)?;
            ensure!(
                file["bytes"].as_u64() == Some(content.len() as u64)
                    && file["sha256"].as_str()
                        == Some(format!("{:x}", Sha256::digest(&content)).as_str()),
                "real browser data changed before uninstall"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "one-shot interactive Brave uninstall and real residual evidence on the marked spare PC"]
fn real_brave_interactive_rules() -> Result<()> {
    let f = fixture_with_seed("brave-evidence-v4-seed-verification.json")?;
    new_file(&f.root.join(format!("{PREFIX}.started")), b"one-shot")?;
    let result = run(&f);
    if let Err(error) = &result {
        std::fs::write(
            f.root.join(format!("{PREFIX}-error.txt")),
            format!("{error:#}"),
        )?;
        status(&f.root, "failed_do_not_relaunch")?;
    }
    result
}

fn run(f: &Fixture) -> Result<()> {
    verify_real_seed(f)?;
    ensure!(
        f.context.args == "--uninstall --verbose-logging",
        "unexpected vendor arguments"
    );
    let specs: Vec<_> = rules::catalog()
        .into_iter()
        .filter(|s| s.family == "brave")
        .collect();
    ensure!(
        specs.len() == 11 && f.context.residual_scopes.len() == 11,
        "ambiguous scope count"
    );
    let mut warnings = vec![];
    let classifier = residual_scope::Classifier::new(&f.context.residual_scopes, &mut warnings);
    ensure!(warnings.is_empty(), "captured scope changed");
    for spec in &specs {
        let scope = f.profile.join(&spec.relative);
        ensure!(
            classifier
                .classify(&scope, spec.target == Target::Tree)
                .is_some_and(|(_, s)| s == spec),
            "rule scope differs"
        );
        ensure!(
            !rules::accepted(spec, "brave", VERSION),
            "production permission already open"
        );
    }
    drop(classifier);
    let before_inventory = WindowsSoftware.inventory()?;
    ensure!(before_inventory.warnings.is_empty(), "incomplete inventory");
    let other_ids: HashSet<_> = before_inventory
        .entries
        .iter()
        .filter(|e| e.identity != f.prepared.entry.identity)
        .map(|e| e.software.id.clone())
        .collect();
    let shared = f.base.join("dustaway-brave-shared-witness-v2-20261001.txt");
    new_file(&shared, b"dedicated shared-parent witness")?;
    let unknown = f.profile.join("DustAwayUnknownScopeV220261001");
    std::fs::create_dir(&unknown)?;
    new_file(
        &unknown.join("keep.txt"),
        b"new unknown scope, no deletion authority",
    )?;
    let before = hashes(&f.profile)?;
    status(&f.root, "waiting_for_brave_uninstall_wizard")?;
    ensure!(
        matches!(
            WindowsSoftware.launch(&f.prepared, &mut |_| {})?,
            LaunchOutcome::Completed
        ),
        "vendor launcher not completed; no deletion"
    );
    // Keep the SAME captured context while observing an early-exiting launcher.
    // No retry, command replay, process termination, or persisted authorization.
    let deadline = std::time::Instant::now() + Duration::from_secs(900);
    while WindowsSoftware.installed(&f.prepared.entry.identity)?
        || security::process_running("brave.exe")?
        || security::process_running("setup.exe")?
    {
        ensure!(
            std::time::Instant::now() < deadline,
            "vendor state unresolved; no deletion or replay"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
    ensure!(
        !f.base
            .join("Brave-Browser/Application/brave.exe")
            .try_exists()?,
        "browser binary remains"
    );
    let after_inventory = WindowsSoftware.inventory()?;
    ensure!(
        after_inventory.warnings.is_empty()
            && after_inventory
                .entries
                .iter()
                .map(|e| e.software.id.clone())
                .collect::<HashSet<_>>()
                == other_ids,
        "unrelated installation identity changed"
    );
    let after_vendor = hashes(&f.profile)?;
    let vendor_changes: std::collections::BTreeSet<_> = before
        .keys()
        .chain(after_vendor.keys())
        .filter(|k| before.get(*k) != after_vendor.get(*k))
        .cloned()
        .collect();
    new_file(
        &f.root.join(format!("{PREFIX}-vendor-diff.json")),
        &serde_json::to_vec_pretty(
            &serde_json::json!({"changed_relative_paths":vendor_changes,"after":after_vendor}),
        )?,
    )?;
    ensure!(
        std::fs::read(&shared)? == b"dedicated shared-parent witness",
        "shared witness changed"
    );
    status(&f.root, "uninstall_verified_checking_residual_protection")?;
    // Cancellation before the process guard returns an error; cancellation
    // during enumeration returns a partial scan. Neither is a complete scan.
    match WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(true)) {
        Err(error) => ensure!(
            error.to_string() == "检查已停止，结果不完整",
            "unexpected pre-cancelled scan error: {error:#}"
        ),
        Ok(_) => bail!("pre-cancelled scan passed the process guard"),
    }
    let scan = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
    ensure!(
        scan.records.iter().all(|r| !r.candidate.selectable),
        "production gate opened early"
    );
    ensure!(
        execute(&f.prepared, record(&scan, &f.profile)?, &[]).is_err(),
        "whole profile acquired authority"
    );
    ensure!(
        execute(&f.prepared, record(&scan, &unknown.join("keep.txt"))?, &[]).is_err(),
        "unknown scope acquired authority"
    );
    let mut results = vec![];
    for spec in &specs {
        status(&f.root, &format!("validating_{}", spec.id))?;
        let scope = f.profile.join(&spec.relative);
        let scan = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
        let classifier = residual_scope::Classifier::new(&f.context.residual_scopes, &mut vec![]);
        if classifier
            .classify(&scope, spec.target == Target::Tree)
            .is_none()
        {
            // An interactive vendor may replace a preferences file. Keep the
            // original boundary; do not adopt the replacement or call it a
            // successful deletion scenario for this rule.
            let retained = record(&scan, &scope)?;
            ensure!(
                !retained.candidate.selectable
                    && !scan.warnings.is_empty()
                    && execute(&f.prepared, retained, &[]).is_err(),
                "changed vendor scope not retained"
            );
            results.push(serde_json::json!({
                "rule_id":spec.id,"rule_version":spec.version,"scope_sha256":spec.scope_sha256(),"kind":spec.kind,
                "outcome":"retained_scope_unverifiable_after_vendor","deletion_verified":false,
                "deleted_files":0,"deleted_bytes":0,"scope_refused":true,
            }));
            std::fs::write(
                f.root.join(format!("{PREFIX}-partial.json")),
                serde_json::to_vec_pretty(&results)?,
            )?;
            continue;
        }
        let actual = scan
            .records
            .iter()
            .find(|r| {
                !r.candidate.directory
                    && Path::new(&r.candidate.path).starts_with(&scope)
                    && classifier
                        .classify(Path::new(&r.candidate.path), false)
                        .is_some_and(|(_, s)| s.id == spec.id)
                    && r.candidate.bytes.parse::<u64>().is_ok_and(|n| n > 0)
            })
            .context("no nonempty actual browser file remains for rule")?;
        let path = PathBuf::from(&actual.candidate.path);
        ensure!(
            path.starts_with(&f.profile),
            "file escaped dedicated profile"
        );
        ensure!(
            WindowsSoftware.delete(&f.prepared, actual, &[]).is_err(),
            "production gate opened early"
        );
        let exclusion = display_path(&path);
        ensure!(
            execute(&f.prepared, actual, std::slice::from_ref(&exclusion)).is_err(),
            "exclusion ignored"
        );
        let excluded = WindowsSoftware.scan(&f.prepared, &[exclusion], &AtomicBool::new(false))?;
        ensure!(
            record(&excluded, &path).is_err() && !excluded.warnings.is_empty(),
            "scan exclusion ignored"
        );
        let original = std::fs::read(&path)?;
        ensure!(original.len() <= 64 * 1024 * 1024, "unexpected file size");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(&path)?;
        ensure!(
            execute(&f.prepared, actual, &[]).is_err(),
            "occupied file deleted"
        );
        drop(lock);
        let mut changed = original.clone();
        changed.extend_from_slice(b"dustaway-modified");
        std::fs::write(&path, &changed)?;
        ensure!(
            execute(&f.prepared, actual, &[]).is_err(),
            "modified file deleted"
        );
        std::fs::write(&path, &original)?;
        let fresh = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
        let actual = record(&fresh, &path)?;
        let peer = f.profile.join(format!(".dustaway-hardlink-{}", spec.id));
        ensure!(!peer.try_exists()?, "prior hardlink remains");
        std::fs::hard_link(&path, &peer)?;
        ensure!(
            execute(&f.prepared, actual, &[]).is_err(),
            "hardlinked file deleted"
        );
        let partial = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
        ensure!(
            !partial.warnings.is_empty() && record(&partial, &path).is_err(),
            "partial scan not reported"
        );
        ensure!(
            std::fs::read(&peer)? == original,
            "hardlink witness changed"
        );
        std::fs::remove_file(&peer)?; // Exact peer created above, never its source.
        let fresh = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
        let actual = record(&fresh, &path)?;
        let saved = path.with_file_name(format!(".dustaway-original-{}", spec.id));
        ensure!(!saved.try_exists()?, "prior replacement remains");
        std::fs::rename(&path, &saved)?;
        new_file(&path, b"dustaway-path-replacement")?;
        ensure!(
            execute(&f.prepared, actual, &[]).is_err(),
            "replacement file deleted"
        );
        ensure!(
            std::fs::read(&path)? == b"dustaway-path-replacement",
            "replacement witness changed"
        );
        std::fs::remove_file(&path)?; // Only our verified replacement, not recursive.
        std::fs::rename(&saved, &path)?;
        ensure!(std::fs::read(&path)? == original, "original not restored");
        let frozen = WindowsSoftware.scan(&f.prepared, &[], &AtomicBool::new(false))?;
        let mut selected: Vec<_> = frozen
            .records
            .iter()
            .filter(|r| {
                Path::new(&r.candidate.path).starts_with(&scope)
                    && classifier
                        .classify(Path::new(&r.candidate.path), r.candidate.directory)
                        .is_some_and(|(_, s)| s.id == spec.id)
            })
            .collect();
        selected
            .sort_by_key(|r| std::cmp::Reverse(Path::new(&r.candidate.path).components().count()));
        let new_path = scope.join(format!(".dustaway-created-after-freeze-{PREFIX}"));
        if spec.target == Target::Tree {
            new_file(&new_path, b"new file must survive frozen deletion")?;
        }
        let mut expected = hashes(&f.profile)?;
        let (mut files, mut bytes, mut directories, mut nonempty) = (0usize, 0u64, 0usize, 0usize);
        for item in selected {
            match execute(&f.prepared, item, &[]) {
                Ok(()) if item.candidate.directory => directories += 1,
                Ok(()) => {
                    let path = Path::new(&item.candidate.path);
                    ensure!(!path.try_exists()?, "deleted file remains");
                    ensure!(
                        expected
                            .remove(
                                &path
                                    .strip_prefix(&f.profile)?
                                    .to_string_lossy()
                                    .into_owned()
                            )
                            .is_some(),
                        "unexpected deleted file"
                    );
                    files += 1;
                    bytes += item.candidate.bytes.parse::<u64>()?;
                }
                Err(_)
                    if item.candidate.directory
                        && std::fs::read_dir(&item.candidate.path)?.next().is_some() =>
                {
                    nonempty += 1
                }
                Err(error) => {
                    return Err(error.context(format!("unexpected refusal in {}", spec.id)));
                }
            }
        }
        ensure!(files > 0 && bytes > 0, "no real browser bytes deleted");
        ensure!(
            hashes(&f.profile)? == expected
                && std::fs::read(&shared)? == b"dedicated shared-parent witness",
            "unselected files changed"
        );
        if spec.target == Target::Tree {
            ensure!(
                new_path.is_file() && nonempty > 0,
                "new file or nonempty root deleted"
            );
        }
        results.push(serde_json::json!({
            "rule_id":spec.id,"rule_version":spec.version,"scope_sha256":spec.scope_sha256(),"kind":spec.kind,
            "outcome":"test_only_deletion_and_protection_verified","deletion_verified":true,
            "deleted_files":files,"deleted_bytes":bytes,"deleted_empty_directories":directories,"retained_nonempty_directories":nonempty,
            "excluded_occupied_changed_hardlinked_replaced_files_refused":true,"partial_scan_reported":true,
            "unselected_hashes_unchanged":true,"new_files_preserved":spec.target==Target::Tree,
        }));
        std::fs::write(
            f.root.join(format!("{PREFIX}-partial.json")),
            serde_json::to_vec_pretty(&results)?,
        )?;
    }
    new_file(
        &f.root.join(format!("{PREFIX}-report.json")),
        &serde_json::to_vec_pretty(&serde_json::json!({
            "passed":true,"ordinary_user":true,"signed_executable_version":VERSION,
            "same_in_memory_pre_uninstall_context":true,"context_persisted":false,
            "standard_windows_software_launcher":true,"interactive_vendor_uninstall":true,"silent_uninstall":false,
            "production_deletion_entry":false,"production_approval_added":false,
            "authority":"test-only evidence predicate; shared production scope and deletion guards",
            "original_identity_absent":true,"other_installation_identities_unchanged":true,
            "cancelled_scan_reported":true,"unknown_scope_refused":true,"whole_profile_refused":true,
            "shared_parent_unchanged":true,
            "deletion_rules_verified":results.iter().filter(|r|r["deletion_verified"]==true).count(),
            "rules":results,
            "not_covered":["Tauri confirmation and selection UI","actual reinstallation refusal","cloud-provider integration","other versions"],
        }))?,
    )?;
    status(&f.root, "passed")
}
