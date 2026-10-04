//! Recheck the already-started MSI fixture. This tool cannot launch an uninstaller.
use anyhow::{Context, Result, ensure};
use dustaway_core::software::{
    SoftwareEngine, SoftwareIdentity, SoftwarePlatform, SoftwareSource, UninstallPhase,
    UninstallTask,
};
use dustaway_windows::software::WindowsSoftware;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::System::ApplicationInstallationAndServicing::{
        INSTALLSTATE_UNKNOWN, MsiQueryProductStateW,
    },
    core::w,
};

fn main() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")?.eq_ignore_ascii_case("DESKTOP-24NJTT6"),
        "wrong computer"
    );
    let root = std::env::current_exe()?
        .parent()
        .context("missing root")?
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
    let mode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "interrupted".into());
    ensure!(
        matches!(
            mode.as_str(),
            "interrupted" | "restart-preflight" | "after-restart"
        ) && std::env::args().len() <= 2,
        "expected interrupted, restart-preflight or after-restart"
    );
    let restart = mode != "interrupted";
    let (journal_name, display, product) = if restart {
        (
            "msi-restart-acceptance.sqlite",
            "DustAway Acceptance MSI Restart",
            "{7C7E25E8-73B7-4D77-B8E7-7B4BB5650932}",
        )
    } else {
        (
            "msi-acceptance.sqlite",
            "DustAway Acceptance MSI",
            "{7C7E25E8-73B7-4D77-B8E7-7B4BB5650930}",
        )
    };
    let report_name = match mode.as_str() {
        "restart-preflight" => "msi-restart-preflight-verification.json",
        "after-restart" => "msi-after-restart-verification.json",
        _ => "msi-recovery-verification.json",
    };
    ensure!(
        !root.join(report_name).exists(),
        "report exists; preserve prior evidence"
    );
    let db = root.join(journal_name);
    ensure!(db.is_file(), "existing journal required");
    let identity = SoftwareIdentity {
        source: SoftwareSource::Desktop,
        key: serde_json::to_string(&(
            "machine",
            512u32,
            format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{product}"),
        ))?,
        family: String::new(),
    };
    let expected_id = format!("desktop:{:x}", Sha256::digest(identity.key.as_bytes()));
    let engine = SoftwareEngine::open(Arc::new(WindowsSoftware), &db)?;
    let before = engine
        .snapshot()
        .context("no interrupted task to recheck; no action taken")?;
    ensure!(
        before.software.id == expected_id
            && before.software.name == display
            && before.software.publisher == "DustAway Acceptance Fixtures",
        "wrong recovered task"
    );
    ensure!(
        before.phase
            == if restart {
                UninstallPhase::RestartRequired
            } else {
                UninstallPhase::Unknown
            },
        "unexpected recovery phase"
    );
    ensure!(engine.history()?.len() == 1, "expected one original task");
    if restart {
        let original: UninstallTask =
            serde_json::from_slice(&std::fs::read(root.join("msi-restart-terminal-task.json"))?)?;
        ensure!(
            original.id == before.id
                && original.updated_at == before.updated_at
                && original.phase == UninstallPhase::RestartRequired,
            "task differs from the recorded MSI restart outcome"
        );
    }
    // Verify the product as well as the original ARP identity before consuming the recheck.
    ensure!(
        !WindowsSoftware.installed(&identity)?,
        "fixture registration still exists; no recheck consumed"
    );
    let installer_state = unsafe {
        MsiQueryProductStateW(if restart {
            w!("{7C7E25E8-73B7-4D77-B8E7-7B4BB5650932}")
        } else {
            w!("{7C7E25E8-73B7-4D77-B8E7-7B4BB5650930}")
        })
    };
    ensure!(
        installer_state == INSTALLSTATE_UNKNOWN,
        "MSI product still registered: {:?}",
        installer_state
    );
    if mode == "restart-preflight" {
        ensure!(
            !WindowsSoftware.restarted_since(before.updated_at)?,
            "already restarted"
        );
        ensure!(
            engine.verify(&before.id).is_err(),
            "recheck bypassed restart guard"
        );
        ensure!(
            engine.scan(&before.id, vec![]).is_err(),
            "scan bypassed restart guard"
        );
        let after = engine.snapshot().context("lost pending restart task")?;
        ensure!(
            after.phase == UninstallPhase::RestartRequired && after.id == before.id,
            "task changed"
        );
        std::fs::write(
            root.join(report_name),
            serde_json::to_vec_pretty(&json!({
                "result":"passed_before_restart", "uninstaller_relaunched":false,
                "boot_guard_enforced":true, "scan_refused":true, "task":after
            }))?,
        )?;
        println!(
            "PASS preflight: reboot and residue guards remain enforced; no uninstaller launched"
        );
        return Ok(());
    }
    if restart {
        let preflight: serde_json::Value = serde_json::from_slice(&std::fs::read(
            root.join("msi-restart-preflight-verification.json"),
        )?)?;
        ensure!(
            preflight["result"] == "passed_before_restart" && preflight["task"]["id"] == before.id,
            "missing matching preflight evidence"
        );
        ensure!(
            WindowsSoftware.restarted_since(before.updated_at)?,
            "Windows has not restarted; nothing verified"
        );
    }
    engine.verify(&before.id)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while engine.is_busy() {
        ensure!(Instant::now() < deadline, "recheck did not finish");
        std::thread::sleep(Duration::from_millis(50));
    }
    let after = engine.snapshot().context("missing rechecked task")?;
    ensure!(
        after.id == before.id && engine.history()?.len() == 1,
        "task identity or history changed"
    );
    ensure!(
        after.phase == UninstallPhase::Removed,
        "removal not verified"
    );
    ensure!(
        after.residual.deleted_bytes == "0",
        "estimated size counted as freed space"
    );
    let residue_refused = engine.scan(&after.id, vec![]).is_err();
    ensure!(residue_refused, "recovery recreated lost residue authority");
    std::fs::write(
        root.join(report_name),
        serde_json::to_vec_pretty(&json!({
            "result":if restart { "passed_after_actual_restart" } else { "passed_after_interrupted_tracking" },
            "uninstaller_relaunched":false,
            "actual_restart_verified":restart,
            "before":before,"after":after,
            "registry_absent":true,"msi_product_state":"INSTALLSTATE_UNKNOWN",
            "lost_residue_authority_not_recreated":residue_refused,
            "scope":"Only verifies the existing dedicated MSI removal. No uninstall, restart or residual deletion is performed by this tool."
        }))?,
    )?;
    println!(
        "PASS existing MSI removal verified after recovery, without launching another uninstall"
    );
    Ok(())
}
