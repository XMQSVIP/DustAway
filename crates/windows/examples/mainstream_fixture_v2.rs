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
    let home = PathBuf::from(std::env::var("USERPROFILE")?);
    let root = local.join("DustAwayAcceptance/20261001-developer-mainstream-v2");
    ensure!(
        fs::read_to_string(root.join(".dustaway-developer"))? == "DustAway.Mainstream.20261001.V2",
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
                (&home, ".cargo/registry/cache/dustaway-aaaaaaaaaaaaaaaa/dustaway-mainstream-v2-20261001-0.0.0.crate".to_owned(), "dev.cargo_archives"),
                (&home, ".rustup/downloads/dbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb".to_owned(), "dev.rustup_downloads"),
                (&home, ".conda/pkgs/dustaway-mainstream-v2-20261001-0.0.0-0.conda".to_owned(), "dev.conda_archives"),
                (&local, "Composer/files/dustaway/mainstream/dbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb.zip".to_owned(), "dev.composer_archives"),
                (&home, ".gem/ruby/3.3.0/cache/dustaway-mainstream-v2-20261001-0.0.0.gem".to_owned(), "dev.gem_archives"),
                (&local, "vcpkg/archives/da/dbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdb.zip".to_owned(), "dev.vcpkg_archives"),
                (&home, ".gradle/daemon/8.14.3/daemon-20261002.out.log".to_owned(), "dev.gradle_logs"),
                (&local, "Google/AndroidStudio2026.1/log/dustaway-mainstream-v2-20261001.log".to_owned(), "dev.android_logs"),
                (&roaming, "Cursor/Code Cache/js/db18000120261001_0".to_owned(), "dev.cursor_cache"),
                (&roaming, "Cursor/logs/20260801T002901/dustaway-mainstream-v2-20261001.log".to_owned(), "dev.cursor_logs"),
            ];
            let mut manifest = Manifest {
                candidates: vec![],
                protected: vec![],
            };
            for (base, relative, rule) in fixtures {
                manifest.candidates.push(seed(base, &relative, rule, true)?);
            }
            for (base, relative) in [
                (&home, ".cargo/bin/dustaway-mainstream-v2-protected.txt"),
                (
                    &home,
                    ".cargo/registry/src/dustaway-mainstream-v2/source.rs",
                ),
                (
                    &home,
                    ".rustup/toolchains/dustaway-mainstream-v2/protected.txt",
                ),
                (
                    &home,
                    ".conda/pkgs/dustaway-mainstream-v2-extracted/info/index.json",
                ),
                (
                    &home,
                    ".gem/ruby/3.3.0/gems/dustaway-mainstream-v2/source.rb",
                ),
                (&home, ".gradle/daemon/8.14.3/dustaway-mainstream-v2.lock"),
                (&local, "Composer/repo/dustaway-mainstream-v2.json"),
                (&local, "vcpkg/installed/dustaway-mainstream-v2.txt"),
                (
                    &local,
                    "Google/AndroidStudio2026.1/LocalHistory/dustaway-mainstream-v2.bin",
                ),
                (&roaming, "Cursor/User/History/dustaway-mainstream-v2.txt"),
                (&roaming, "Cursor/Backups/dustaway-mainstream-v2.txt"),
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
                plan.files == 10
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
                report.deleted == 10 && report.failed + report.unknown + report.skipped == 0,
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
                &serde_json::json!({"passed": true, "deleted": 10, "protected": manifest.protected.len(), "replay_refused": true, "ordinary_user": true}),
            )?;
        }
        _ => anyhow::bail!("seed or clean required"),
    }
    Ok(())
}
