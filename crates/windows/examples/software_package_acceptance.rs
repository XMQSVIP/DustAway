//! Explicit MSI/MSIX fixture identities only. Run on the dedicated acceptance computer.
use anyhow::{Context, Result, ensure};
use dustaway_core::software::*;
use dustaway_windows::software::WindowsSoftware;
use serde_json::json;
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
fn main() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")?.eq_ignore_ascii_case("DESKTOP-24NJTT6"),
        "wrong computer"
    );
    let root = std::env::current_exe()?
        .parent()
        .context("no root")?
        .to_owned();
    ensure!(
        root == Path::new(&std::env::var("LOCALAPPDATA")?)
            .join(r"DustAwayAcceptance\20260930-software"),
        "wrong directory"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "missing marker"
    );
    let kind = std::env::args()
        .nth(1)
        .context("explicit msi or msix required")?;
    let (product, display, expected) = match kind.as_str() {
        "msi" => (
            "{7C7E25E8-73B7-4D77-B8E7-7B4BB5650930}",
            "DustAway Acceptance MSI",
            UninstallPhase::Removed,
        ),
        "msi-completion" => (
            "{7C7E25E8-73B7-4D77-B8E7-7B4BB5650931}",
            "DustAway Acceptance MSI Completion",
            UninstallPhase::Removed,
        ),
        "msi-restart" => (
            "{7C7E25E8-73B7-4D77-B8E7-7B4BB5650932}",
            "DustAway Acceptance MSI Restart",
            UninstallPhase::RestartRequired,
        ),
        "msix" => ("", "DustAway Acceptance MSIX", UninstallPhase::Removed),
        _ => anyhow::bail!("unsupported kind"),
    };
    let db = root.join(format!("{kind}-acceptance.sqlite"));
    if db.exists() {
        // Preparing can fail before any task is persisted (for example a rejected MSI host).
        // A recorded task, including a recovered unknown result, always forbids a retry.
        let previous = SoftwareEngine::open(Arc::new(WindowsSoftware), &db)?;
        ensure!(
            previous.history()?.is_empty() && previous.snapshot().is_none(),
            "recorded uninstall exists; never replay it"
        );
    }
    let inventory = WindowsSoftware.inventory()?;
    ensure!(inventory.warnings.is_empty(), "incomplete inventory");
    let matching: Vec<_> = inventory.entries.iter().filter(|e| {
        if !product.is_empty() {
            e.identity.source == SoftwareSource::Desktop
                && serde_json::from_str::<(String, u32, String)>(&e.identity.key).is_ok_and(|(hive, _, key)| {
                    matches!(hive.as_str(), "user" | "machine")
                        && key == format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{product}")
                })
        } else {
            e.identity.source == SoftwareSource::Store
                && e.identity.key.starts_with("DustAway.Acceptance.7c7e25e8_1.0.0.0_x64__")
        }
    }).collect();
    ensure!(
        matching.len() == 1,
        "expected exactly one approved fixture, got {}",
        matching.len()
    );
    let entry = matching[0];
    ensure!(
        entry.software.publisher == "DustAway Acceptance Fixtures"
            && entry.software.name == display,
        "wrong fixture metadata"
    );
    let before: std::collections::BTreeSet<_> = inventory
        .entries
        .iter()
        .filter(|e| e.software.id != entry.software.id)
        .map(|e| e.software.id.clone())
        .collect();
    let engine = SoftwareEngine::open(Arc::new(WindowsSoftware), &db)?;
    let list = engine.list()?;
    ensure!(
        list.items
            .iter()
            .any(|s| s.id == entry.software.id && s.can_uninstall),
        "fixture cannot uninstall"
    );
    let plan = engine.prepare(&entry.software.id)?;
    let task = engine.start(&plan.id)?;
    ensure!(engine.start(&plan.id).is_err(), "duplicate start accepted");
    let deadline = Instant::now() + Duration::from_secs(if product.is_empty() { 90 } else { 600 });
    let mut progress = vec![];
    let mut previous_phase = None;
    loop {
        let status = engine.snapshot().context("missing task")?;
        if previous_phase != Some(status.phase) {
            std::fs::write(
                root.join(format!("{kind}-live-task.json")),
                serde_json::to_vec_pretty(&status)?,
            )?;
            previous_phase = Some(status.phase);
        }
        if let Some(value) = status.progress {
            progress.push(value);
        }
        if !status.phase.active() {
            std::fs::write(
                root.join(format!("{kind}-terminal-task.json")),
                serde_json::to_vec_pretty(&status)?,
            )?;
            ensure!(
                status.phase == expected,
                "{kind} unexpected result: {:?} {}",
                status.phase,
                status.message
            );
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "system installer still active; tracking stopped without killing it"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    ensure!(
        !WindowsSoftware.installed(&entry.identity)?,
        "fixture still installed"
    );
    let scan = if expected == UninstallPhase::RestartRequired {
        ensure!(
            engine.scan(&task, vec![]).is_err(),
            "restart task allowed scan"
        );
        ensure!(
            engine.verify(&task).is_err(),
            "restart task verified without restart"
        );
        let recovered = SoftwareEngine::open(Arc::new(WindowsSoftware), &db)?;
        ensure!(
            recovered.snapshot().context("lost restart task")?.phase
                == UninstallPhase::RestartRequired,
            "recovery lost restart requirement"
        );
        ensure!(
            recovered.verify(&task).is_err() && recovered.scan(&task, vec![]).is_err(),
            "recovery bypassed restart requirement"
        );
        None
    } else {
        engine.scan(&task, vec![])?;
        while engine.is_busy() {
            ensure!(Instant::now() < deadline, "residue scan timeout");
            std::thread::sleep(Duration::from_millis(50));
        }
        let scan = engine.residuals(&task, 0, "")?;
        ensure!(
            scan.items.iter().all(|c| !c.selectable),
            "production residue gate unexpectedly enabled"
        );
        Some(scan)
    };
    let after_inventory = WindowsSoftware.inventory()?;
    ensure!(
        after_inventory.warnings.is_empty(),
        "incomplete verification inventory"
    );
    let after: std::collections::BTreeSet<_> = after_inventory
        .entries
        .iter()
        .map(|e| e.software.id.clone())
        .collect();
    ensure!(
        before == after,
        "unrelated software inventory changed; review before continuing"
    );
    ensure!(
        engine.snapshot().unwrap().residual.deleted_bytes == "0",
        "estimate counted as release"
    );
    std::fs::write(
        root.join(format!("{kind}-acceptance.json")),
        serde_json::to_vec_pretty(
            &json!({"kind":kind,"result":"passed","ordinary_user":true,"duplicate_start_rejected":true,"registration_absence_verified":true,"unrelated_installation_identities_unchanged":true,"production_residue_deletion_enabled":false,"system_progress_samples":progress,"restart_required_and_recovery_guard_verified":expected == UninstallPhase::RestartRequired,"residue_scan":scan,"task":engine.snapshot()}),
        )?,
    )?;
    println!("PASS real {kind}: expected {expected:?}, identity and residue safety checks passed");
    Ok(())
}
