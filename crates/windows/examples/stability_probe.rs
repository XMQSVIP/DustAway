//! Ordinary-user production preflight for one 0.25 candidate. Never executes deletion.
#[path = "../src/acceptance_wait.rs"]
mod acceptance_wait;
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::sync::Arc;

fn run() -> Result<serde_json::Value> {
    ensure!(
        !dustaway_windows::security::elevated()?,
        "Ordinary user required"
    );
    let id = std::env::args().nth(1).unwrap_or_default();
    ensure!(
        matches!(
            id.as_str(),
            "weixin.code" | "weixin.gpu" | "appcache.dingtalk_logs" | "appcache.todesk_logs"
        ),
        "Outside candidate scope"
    );
    let platform = Arc::new(dustaway_windows::WindowsPlatform::new(Scope::User)?);
    let rule = rules::for_scope(Scope::User)?
        .into_iter()
        .find(|r| r.id == id)
        .unwrap();
    ensure!(
        !platform.cleanup_validated(&rule),
        "Candidate now enabled; use its release acceptance workflow"
    );
    let engine = Engine::open(platform, Scope::User, std::path::Path::new(":memory:"))?;
    let session = engine.start_scan(vec![id.clone()])?;
    let outcome = acceptance_wait::wait(|| engine.is_busy(), || engine.cancel());
    ensure!(
        outcome == acceptance_wait::WaitOutcome::Finished,
        "Scan cancelled or stop unconfirmed"
    );
    let summary = engine.summary();
    let row = summary.rules.iter().find(|r| r.rule_id == id).unwrap();
    let rejected = engine
        .prepare(
            &session,
            Selection {
                rule_ids: vec![id.clone()],
                reviewed_rule_ids: vec![id.clone()],
                ..Default::default()
            },
        )
        .is_err();
    ensure!(rejected, "Readonly candidate accepted a plan");
    Ok(serde_json::json!({
        "schema_version":1, "version":env!("CARGO_PKG_VERSION"), "rule_id":id,
        "ordinary_user":true, "synthetic":false, "mode":"production_readonly",
        "phase":row.phase, "candidate_files":row.files, "candidate_bytes":row.bytes,
        "retained_files":row.observation.as_ref().map(|o|o.retained_files),
        "retained_bytes":row.observation.as_ref().map(|o|&o.retained_bytes),
        "scan_issue":row.scan_issue, "availability":row.availability,
        "coverage_complete":row.coverage.as_ref().map(|c|c.complete),
        "plan_rejected":rejected, "deletion_attempted":false, "human_recovery_verified":false
    }))
}
fn main() {
    match run() {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap()),
        Err(error) => {
            eprintln!("Preflight failed: {error:#}");
            std::process::exit(1);
        }
    }
}
