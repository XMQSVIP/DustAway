//! Read-only comparison with the OS protection snapshot; never launches an uninstaller.
use anyhow::{Context, Result, ensure};
use dustaway_core::software::{SoftwarePlatform, SoftwareSource};
use dustaway_windows::software::WindowsSoftware;
use serde::Deserialize;
use serde_json::json;
use std::path::Path;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Expected {
    package_full_name: String,
    non_removable: bool,
    signature_kind: String,
}

fn main() -> Result<()> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "wrong host"
    );
    let root = std::env::current_exe()?
        .parent()
        .context("no root")?
        .to_owned();
    ensure!(
        root == Path::new(&std::env::var("LOCALAPPDATA")?)
            .join(r"DustAwayAcceptance\20260930-software"),
        "wrong root"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
            == "DustAway.Acceptance.20260930.7c7e25e8",
        "wrong marker"
    );
    let input = std::fs::read_to_string(root.join("store-policy-expected.json"))?;
    let expected: Vec<Expected> = serde_json::from_str(input.trim_start_matches('\u{feff}'))?;
    let inventory = WindowsSoftware.inventory()?;
    ensure!(
        inventory.warnings.is_empty(),
        "incomplete inventory: {:?}",
        inventory.warnings
    );
    let mut checked = 0;
    let mut system = 0;
    let mut policy = 0;
    for item in expected {
        let entry = inventory
            .entries
            .iter()
            .find(|e| {
                e.identity.source == SoftwareSource::Store
                    && e.identity.key == item.package_full_name
            })
            .context("package absent from native inventory")?;
        if item.non_removable || item.signature_kind == "System" {
            ensure!(
                !entry.software.can_uninstall && entry.software.unavailable_reason.is_some(),
                "protected package was allowed"
            );
            ensure!(
                WindowsSoftware.prepare(entry).is_err(),
                "protected package received plan"
            );
            if item.signature_kind == "System" {
                system += 1;
            } else {
                policy += 1;
            }
        } else {
            ensure!(
                entry.software.can_uninstall,
                "removable package was unexpectedly blocked"
            );
        }
        checked += 1;
    }
    ensure!(
        system > 0 && policy > 0,
        "test needs both system and non-system protection cases"
    );
    let report = json!({"result":"passed","checked_packages":checked,"system_protected":system,"non_system_policy_protected":policy,"protected_prepare_refused":true,"uninstall_invoked":false});
    std::fs::write(
        root.join("store-policy-acceptance.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{report}");
    Ok(())
}
