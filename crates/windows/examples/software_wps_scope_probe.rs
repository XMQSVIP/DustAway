//! Read-only WPS preparation probe. No launch, residual scan, deletion or backup/settings reads.
//! Its JSON contains no paths, file identities, commands or recoverable preparation context.
use anyhow::{Context, Result, ensure};
use dustaway_core::software::{SoftwarePlatform, residual_rules};
use dustaway_windows::software::WindowsSoftware;

fn main() -> Result<()> {
    ensure!(
        std::env::args_os().len() == 1,
        "this read-only probe takes no arguments"
    );
    let inventory = WindowsSoftware.inventory()?;
    ensure!(
        inventory.warnings.is_empty(),
        "inventory incomplete; no conclusion recorded"
    );
    let entries: Vec<_> = inventory
        .entries
        .iter()
        .filter(|e| e.identity.family == "wps")
        .collect();
    ensure!(
        entries.len() == 1,
        "expected exactly one WPS installation; no conclusion recorded"
    );
    if !entries[0].software.can_uninstall {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "evidence_kind": "read_only_preparation_observation", "family": "wps",
                "preparation_ok": false, "unavailable_reason": entries[0].software.unavailable_reason,
                "personal_data_contents_read": false, "uninstall_executed": false,
                "residual_deletion_executed": false, "real_rule_acceptance": false,
            }))?
        );
        anyhow::bail!("WPS preparation is unavailable; no scope conclusion recorded");
    }
    let prepared = WindowsSoftware.prepare(entries[0])?;
    let context: serde_json::Value = serde_json::from_str(&prepared.context)?;
    let scopes = context["residual_scopes"]
        .as_array()
        .context("missing scope array")?;
    let details: Vec<_> = ["uninstall.wps.backups", "uninstall.wps.published_addins", "uninstall.wps.legacy_addins"]
        .into_iter().map(|id| {
            let spec = residual_rules::rule(id).expect("built-in WPS rule");
            serde_json::json!({
                "rule_id": id, "scope_sha256": spec.scope_sha256(), "kind": spec.kind,
                "target": spec.target,
                "captured": scopes.iter().any(|s| s["rule_id"] == id && s["scope_sha256"] == spec.scope_sha256()),
                "production_deletion_approved": residual_rules::accepted(&spec, "wps", context["verified_version"].as_str().unwrap_or("")),
            })
        }).collect();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
        "evidence_kind": "read_only_preparation_observation",
        "preparation_ok": true,
            "family": "wps", "verified_version": context["verified_version"],
            "uninstall_context_version": context["rule_version"],
            "preparation_warning_count": context["warnings"].as_array().map(Vec::len),
            "scopes": details,
        "personal_data_contents_read": false, "uninstall_executed": false, "residual_deletion_executed": false,
            "real_rule_acceptance": false,
        }))?
    );
    Ok(())
}
