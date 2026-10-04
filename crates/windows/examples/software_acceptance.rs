//! Real, opt-in acceptance driver. Hard-locked to dedicated fixtures on the test computer.
//! It does not change any production residue gate or accept arbitrary removal targets.
use anyhow::{Context, Result, ensure};
use dustaway_core::software::*;
use dustaway_windows::software::WindowsSoftware;
use serde_json::json;
use std::{
    path::Path,
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};

const MARKER: &str = "DustAway.Acceptance.20260930.7c7e25e8";

fn fixture(root: &Path, args: &[&str]) -> Result<()> {
    ensure!(
        Command::new(root.join("DustAwayAcceptanceFixture.exe"))
            .args(args)
            .status()?
            .success(),
        "fixture failed"
    );
    Ok(())
}
fn wait(engine: &SoftwareEngine) -> Result<UninstallTask> {
    wait_for(engine, Duration::from_secs(90))
}
fn wait_for(engine: &SoftwareEngine, timeout: Duration) -> Result<UninstallTask> {
    let start = Instant::now();
    loop {
        let task = engine.snapshot().context("missing task")?;
        if !task.phase.active() {
            return Ok(task);
        }
        ensure!(
            start.elapsed() < timeout,
            "task timed out (no process is force-killed)"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn select(engine: &SoftwareEngine, name: &str) -> Result<String> {
    let list = engine.list()?;
    let matching: Vec<_> = list
        .items
        .iter()
        .filter(|s| {
            s.name == format!("DustAway Acceptance {name}")
                && s.publisher == "DustAway Acceptance Fixtures"
        })
        .collect();
    ensure!(matching.len() == 1, "fixture identity must be unique");
    Ok(matching[0].id.clone())
}
fn prepare(engine: &SoftwareEngine, name: &str, root: &Path) -> Result<UninstallPlan> {
    let id = select(engine, name)?;
    // Validate both the real registry identity and the immutable executable path before planning.
    let inventory = WindowsSoftware.inventory()?;
    let entry = inventory
        .entries
        .iter()
        .find(|e| e.software.id == id)
        .context("entry vanished")?;
    let (hive, _, key): (String, u32, String) = serde_json::from_str(&entry.identity.key)?;
    ensure!(
        hive == "user"
            && key
                == format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{MARKER}.{name}"),
        "non-fixture identity rejected"
    );
    ensure!(
        entry.software.install_location.as_deref() == root.join("installed").to_str(),
        "non-fixture location rejected"
    );
    engine.prepare(&id)
}
fn launch_count(root: &Path, name: &str) -> usize {
    std::fs::read_to_string(root.join("launches.log"))
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == name)
        .count()
}
fn check(checks: &mut Vec<serde_json::Value>, name: &str, condition: bool) -> Result<()> {
    ensure!(condition, "acceptance failed: {name}");
    println!("PASS {name}");
    checks.push(json!({"check": name, "result": "passed"}));
    Ok(())
}
fn run(root: &Path) -> Result<()> {
    let mut checks = vec![];
    fixture(root, &["install"])?;
    let engine = SoftwareEngine::open(Arc::new(WindowsSoftware), &root.join("acceptance.sqlite"))?;
    let list = engine.list()?;
    check(
        &mut checks,
        "inventory_sources_read_completely",
        list.warnings.is_empty(),
    )?;
    let items: Vec<_> = list
        .items
        .iter()
        .filter(|s| s.name.starts_with("DustAway Acceptance "))
        .collect();
    check(
        &mut checks,
        "same_name_different_installations_remain_distinct",
        items
            .iter()
            .filter(|s| s.name == "DustAway Acceptance Same Name")
            .count()
            == 2,
    )?;
    check(
        &mut checks,
        "hidden_and_update_entries_filtered",
        !items
            .iter()
            .any(|s| s.name.ends_with(" Hidden") || s.name.ends_with(" Update")),
    )?;
    for suffix in ["Missing", "Wrapper", "Ambiguous", "NoRemove"] {
        let item = items
            .iter()
            .find(|s| s.name == format!("DustAway Acceptance {suffix}"))
            .context("expected disabled fixture")?;
        check(
            &mut checks,
            &format!("{suffix}_blocked_with_reason"),
            !item.can_uninstall && item.unavailable_reason.is_some(),
        )?;
    }
    let missing = items.iter().find(|s| s.name.ends_with(" Missing")).unwrap();
    check(
        &mut checks,
        "missing_size_and_metadata_preserved_as_unknown",
        missing.estimated_bytes.is_none()
            && missing.publisher.is_empty()
            && missing.version.is_empty(),
    )?;

    for name in ["Normal", "Delayed", "Early", "ZeroExit", "Reinstall"] {
        let plan = prepare(&engine, name, root)?;
        let task_id = engine.start(&plan.id)?;
        check(
            &mut checks,
            &format!("{name}_duplicate_plan_rejected"),
            engine.start(&plan.id).is_err(),
        )?;
        if name == "Delayed" {
            check(
                &mut checks,
                "list_refresh_blocked_while_running",
                engine.list().is_err(),
            )?;
            check(
                &mut checks,
                "residue_scan_blocked_while_running",
                engine.scan(&task_id, vec![]).is_err(),
            )?;
        }
        let task = wait(&engine)?;
        check(
            &mut checks,
            &format!("{name}_launched_once"),
            launch_count(root, name) == 1,
        )?;
        check(
            &mut checks,
            &format!("{name}_no_fake_exe_percentage"),
            task.progress.is_none(),
        )?;
        if name == "Early" || name == "ZeroExit" {
            check(
                &mut checks,
                &format!("{name}_exit_zero_not_reported_as_removed"),
                task.phase == UninstallPhase::StillInstalled,
            )?;
            check(
                &mut checks,
                &format!("{name}_residue_scan_refused"),
                engine.scan(&task_id, vec![]).is_err(),
            )?;
            if name == "Early" {
                std::thread::sleep(Duration::from_secs(5));
                engine.verify(&task_id)?;
                check(
                    &mut checks,
                    "early_launcher_later_verified_removed",
                    wait(&engine)?.phase == UninstallPhase::Removed,
                )?;
            }
        } else {
            check(
                &mut checks,
                &format!("{name}_registration_absence_verified"),
                task.phase == UninstallPhase::Removed,
            )?;
            check(
                &mut checks,
                &format!("{name}_only_installer_payload_removed"),
                !root
                    .join("installed")
                    .join(format!("payload-{name}.txt"))
                    .exists()
                    && root.join(format!("retained-{name}.txt")).exists(),
            )?;
            if name == "Reinstall" {
                fixture(root, &["install-one", name])?;
                engine.scan(&task_id, vec![])?;
                let denied = wait(&engine)?;
                check(
                    &mut checks,
                    "reinstall_prevents_residue_scan",
                    denied.phase == UninstallPhase::Removed
                        && engine.residuals(&task_id, 0, "").is_err(),
                )?;
            } else {
                engine.scan(&task_id, vec![])?;
                check(
                    &mut checks,
                    &format!("{name}_unsupported_residue_explicit"),
                    wait(&engine)?.phase == UninstallPhase::ResidualsReady
                        && !engine.residuals(&task_id, 0, "")?.supported,
                )?;
            }
        }
    }
    // A second actual process exits during a live uninstaller. Recovery must not replay it.
    ensure!(
        Command::new(std::env::current_exe()?)
            .arg("--recovery-start")
            .status()?
            .success(),
        "recovery starter failed"
    );
    let recovery = SoftwareEngine::open(Arc::new(WindowsSoftware), &root.join("recovery.sqlite"))?;
    let recovered = recovery.snapshot().context("missing recovered task")?;
    check(
        &mut checks,
        "process_exit_recovers_as_unknown",
        recovered.phase == UninstallPhase::Unknown,
    )?;
    check(
        &mut checks,
        "recovery_does_not_replay_uninstaller",
        launch_count(root, "Recovery") == 1,
    )?;
    std::thread::sleep(Duration::from_secs(5));
    recovery.verify(&recovered.id)?;
    check(
        &mut checks,
        "recovered_result_verified_without_replay",
        wait(&recovery)?.phase == UninstallPhase::Removed && launch_count(root, "Recovery") == 1,
    )?;
    check(
        &mut checks,
        "recovery_does_not_recreate_residue_authority",
        recovery.scan(&recovered.id, vec![]).is_err(),
    )?;
    check(
        &mut checks,
        "estimated_size_never_counted_as_freed",
        engine
            .history()?
            .iter()
            .all(|t| t.residual.deleted_bytes == "0"),
    )?;
    fixture(root, &["cleanup"])?;
    let left = WindowsSoftware
        .inventory()?
        .entries
        .into_iter()
        .filter(|e| e.identity.key.contains(MARKER))
        .count();
    check(
        &mut checks,
        "all_exe_fixture_registrations_removed",
        left == 0,
    )?;
    std::fs::write(
        root.join("exe-acceptance.json"),
        serde_json::to_vec_pretty(
            &json!({"kind":"real-exe-backend-acceptance", "computer":"DESKTOP-24NJTT6", "production_residue_deletion_enabled":false, "checks":checks, "uninstall_history":engine.history()?, "scope":"Dedicated fixture EXEs only; verify token/session in execution-context.json. Does not validate UAC or native UI."}),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")
            .unwrap_or_default()
            .eq_ignore_ascii_case("DESKTOP-24NJTT6"),
        "only the authorized acceptance computer is allowed"
    );
    let root = std::env::current_exe()?
        .parent()
        .context("no executable directory")?
        .to_owned();
    ensure!(
        root == Path::new(&std::env::var("LOCALAPPDATA")?)
            .join(r"DustAwayAcceptance\20260930-software"),
        "wrong fixture directory"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))? == MARKER,
        "missing acceptance marker"
    );
    if std::env::args().nth(1).as_deref() == Some("--uac-cancel") {
        let db = root.join("uac-acceptance.sqlite");
        ensure!(
            !db.exists(),
            "UAC acceptance already started; do not replay"
        );
        fixture(&root, &["install-one", "Uac"])?;
        let engine = SoftwareEngine::open(Arc::new(WindowsSoftware), &db)?;
        let plan = prepare(&engine, "Uac", &root)?;
        let task_id = engine.start(&plan.id)?;
        let task = wait_for(&engine, Duration::from_secs(600))?;
        std::fs::write(
            root.join("uac-terminal-task.json"),
            serde_json::to_vec_pretty(&task)?,
        )?;
        ensure!(
            task.phase == UninstallPhase::Cancelled,
            "UAC was not cancelled: {:?}",
            task.phase
        );
        ensure!(
            launch_count(&root, "Uac") == 0,
            "uninstaller ran despite cancellation"
        );
        ensure!(
            engine.scan(&task_id, vec![]).is_err(),
            "cancelled task allowed residue scan"
        );
        ensure!(
            select(&engine, "Uac").is_ok(),
            "cancelled software vanished"
        );
        fixture(&root, &["cleanup"])?;
        std::fs::write(
            root.join("uac-acceptance.json"),
            serde_json::to_vec_pretty(
                &json!({"result":"passed","phase":task.phase,"uninstaller_started":false,"residue_scan_refused":true,"registration_preserved_until_fixture_teardown":true}),
            )?,
        )?;
        println!("PASS UAC cancellation, registration preservation and residue refusal");
        return Ok(());
    }
    if std::env::args().nth(1).as_deref() == Some("--recovery-start") {
        let engine =
            SoftwareEngine::open(Arc::new(WindowsSoftware), &root.join("recovery.sqlite"))?;
        let plan = prepare(&engine, "Recovery", &root)?;
        engine.start(&plan.id)?;
        let started = Instant::now();
        while launch_count(&root, "Recovery") == 0 {
            ensure!(
                started.elapsed() < Duration::from_secs(10),
                "uninstaller did not start"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        std::process::exit(0);
    }
    ensure!(
        !root.join("acceptance.sqlite").exists(),
        "fresh acceptance run required; never replay an old run"
    );
    if let Err(error) = run(&root) {
        std::fs::write(root.join("exe-acceptance-error.txt"), format!("{error:#}"))?;
        return Err(error);
    }
    Ok(())
}
