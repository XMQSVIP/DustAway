//! Acceptance harness only; never shipped. Deletes only its newly-created, recorded fixtures.
use anyhow::{Context, Result, ensure};
use dustaway_core::{Engine, Scope, Selection};
use dustaway_windows::{WindowsPlatform, security};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
struct Sample {
    rule: String,
    path: PathBuf,
    sha256: String,
    bytes: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    candidates: Vec<Sample>,
    protected: Vec<Sample>,
}
fn sha(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}
fn seed(base: &Path, relative: &str, rule: &str, old: bool) -> Result<Sample> {
    let path = base.join(relative);
    fs::create_dir_all(path.parent().context("parent")?)?;
    // Pin every parent before writing. Refuse junctions, cloud paths and redirected fixtures.
    let _pin = dustaway_windows::fs::PinnedPath::open(path.parent().unwrap(), true, false)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(
        format!("DustAway developer acceptance 20261001: {rule} {relative}\n").as_bytes(),
    )?;
    file.sync_all()?;
    if old {
        filetime::set_file_mtime(
            &path,
            filetime::FileTime::from_unix_time(dustaway_core::now() - 40 * 86400, 0),
        )?;
    }
    Ok(Sample {
        rule: rule.into(),
        bytes: fs::metadata(&path)?.len(),
        sha256: sha(&path)?,
        path,
    })
}
fn wait(engine: &Engine) -> Result<()> {
    let start = Instant::now();
    while engine.is_busy() {
        ensure!(
            start.elapsed() < Duration::from_secs(180),
            "Timeout; do not resume deletion"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
fn main() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "Wrong host"
    );
    ensure!(
        std::env::var("USERNAME")?.eq_ignore_ascii_case("MSN"),
        "Wrong user"
    );
    ensure!(!security::elevated()?, "Limited token required");
    let local = PathBuf::from(std::env::var("LOCALAPPDATA")?);
    let roaming = PathBuf::from(std::env::var("APPDATA")?);
    let root = local.join("DustAwayAcceptance/20261001-developer-cleanup");
    ensure!(
        fs::read_to_string(root.join(".dustaway-developer"))? == "DustAway.Developer.20261001",
        "Marker"
    );
    let evidence = root.join("evidence");
    let manifest_path = evidence.join("fixtures.json");
    match std::env::args().nth(1).as_deref() {
        Some("seed") => {
            ensure!(
                !manifest_path.exists(),
                "Already seeded; do not overwrite evidence"
            );
            let fixtures = [
                (
                    &local,
                    format!(
                        "npm-cache/_cacache/content-v2/sha512/da/18/{}",
                        "1".repeat(124)
                    ),
                    "dev.npm_cache",
                ),
                (
                    &local,
                    "npm-cache/_logs/2026-08-01-dustaway-20261001.log".into(),
                    "dev.npm_logs",
                ),
                (
                    &local,
                    format!("pip/Cache/http-v2/d/a/1/8/0/{}.body", "1".repeat(56)),
                    "dev.pip_http",
                ),
                (
                    &local,
                    format!(
                        "pip/Cache/wheels/da/18/00/{}/dustaway_acceptance-0.0.0-py3-none-any.whl",
                        "1".repeat(50)
                    ),
                    "dev.pip_wheels",
                ),
                (
                    &local,
                    format!(
                        "NuGet/v3-cache/{}$dustaway-20261001/list_acceptance.dat",
                        "1".repeat(40)
                    ),
                    "dev.nuget_http",
                ),
                (
                    &roaming,
                    "Code/Code Cache/js/da18000100000000_0".into(),
                    "dev.vscode_cache",
                ),
                (
                    &roaming,
                    "Code/logs/20260801T001801/dustaway-20261001.log".into(),
                    "dev.vscode_logs",
                ),
                (
                    &local,
                    "JetBrains/IdeaIC2026.2/log/dustaway-20261001.log".into(),
                    "dev.jetbrains_logs",
                ),
            ];
            let mut manifest = Manifest {
                candidates: vec![],
                protected: vec![],
            };
            for (base, relative, rule) in fixtures {
                manifest.candidates.push(seed(base, &relative, rule, true)?);
            }
            for (base, relative) in [
                (&local, "npm-cache/_cacache/tmp/dustaway-20261001.txt"),
                (&local, "pip/Cache/selfcheck/dustaway-20261001.json"),
                (&local, "NuGet/v3-cache/dustaway-20261001.lock"),
                (&roaming, "Code/User/dustaway-20261001.json"),
                (&roaming, "Code/Backups/dustaway-20261001/unsaved.txt"),
                (
                    &local,
                    "JetBrains/IdeaIC2026.2/LocalHistory/dustaway-20261001.bin",
                ),
                (
                    &local,
                    "JetBrains/PersonalProject/log/dustaway-20261001.log",
                ),
                (
                    &local,
                    "npm-cache/_logs/2026-10-01-dustaway-recent-20261001.log",
                ),
            ] {
                manifest
                    .protected
                    .push(seed(base, relative, "protected", false)?);
            }
            save(&manifest_path, &manifest)?;
        }
        Some("clean") => {
            ensure!(
                !evidence.join("fixture-clean-started.json").exists(),
                "Deletion already requested; never replay"
            );
            let manifest: Manifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
            for sample in manifest.candidates.iter().chain(&manifest.protected) {
                ensure!(sha(&sample.path)? == sample.sha256, "Fixture changed");
            }
            let engine = Engine::open(
                Arc::new(WindowsPlatform::new(Scope::User)?),
                Scope::User,
                &evidence.join("fixture-journal.sqlite"),
            )?;
            let ids: Vec<_> = dustaway_core::developer::RULES
                .iter()
                .map(|s| s.id.to_owned())
                .collect();
            let session = engine.start_scan(ids.clone())?;
            wait(&engine)?;
            save(&evidence.join("fixture-scan.json"), &engine.summary())?;
            let mut chosen = vec![];
            for sample in &manifest.candidates {
                let mut matches = vec![];
                let mut offset = 0;
                loop {
                    let page = engine.candidates(&session, &sample.rule, offset, 200)?;
                    offset += page.items.len() as u64;
                    for item in page.items {
                        if Path::new(&item.path) == sample.path {
                            matches.push(item);
                        }
                    }
                    if offset >= page.total {
                        break;
                    }
                }
                ensure!(
                    matches.len() == 1,
                    "Expected one fixture for {}: {}",
                    sample.rule,
                    matches.len()
                );
                chosen.push(matches.remove(0));
            }
            save(&evidence.join("fixture-delete-list.json"), &chosen)?;
            let plan = engine.prepare(
                &session,
                Selection {
                    included_ids: chosen.iter().map(|c| c.id.clone()).collect(),
                    reviewed_rule_ids: ids,
                    ..Default::default()
                },
            )?;
            ensure!(
                plan.files == 8
                    && plan.bytes.parse::<u64>()?
                        == manifest.candidates.iter().map(|s| s.bytes).sum::<u64>(),
                "Wrong selection"
            );
            save(&evidence.join("fixture-clean-started.json"), &plan)?;
            engine.execute(&plan.id)?;
            wait(&engine)?;
            let report = engine.report().context("report")?;
            save(&evidence.join("fixture-report.json"), &report)?;
            ensure!(
                report.deleted == 8 && report.failed + report.unknown + report.skipped == 0,
                "Incomplete cleanup"
            );
            ensure!(engine.execute(&plan.id).is_err(), "Replay accepted");
            for sample in &manifest.candidates {
                ensure!(!sample.path.try_exists()?, "Fixture retained");
            }
            for sample in &manifest.protected {
                ensure!(sha(&sample.path)? == sample.sha256, "Protection changed");
            }
            save(
                &evidence.join("fixture-success.json"),
                &serde_json::json!({"passed": true, "deleted": 8, "protected": manifest.protected.len(), "replay_refused": true, "ordinary_user": true}),
            )?;
        }
        _ => anyhow::bail!("seed or clean required"),
    }
    Ok(())
}
