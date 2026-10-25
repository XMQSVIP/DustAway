//! Production read-only scan. Redacted output; no cleanup execution or application launch.
#[path = "../src/acceptance_wait.rs"]
mod acceptance_wait;
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::sync::Arc;

fn run() -> Result<serde_json::Value> {
    let run_id = std::env::var("DUSTAWAY_COVERAGE_RUN")?;
    ensure!(
        (8..=80).contains(&run_id.len())
            && run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "Invalid run"
    );
    let host = std::env::var("COMPUTERNAME")?;
    let ordinary = !dustaway_windows::security::elevated()?;
    if host == "DESKTOP-24NJTT6" {
        ensure!(ordinary, "Ordinary user required");
        let root = std::path::PathBuf::from(std::env::var("LOCALAPPDATA")?)
            .join("DustAwayAcceptance/coverage-024")
            .join(&run_id);
        ensure!(
            std::env::current_exe()?.parent() == Some(root.as_path()),
            "Wrong run folder"
        );
        ensure!(
            std::fs::read_to_string(root.join(".dustaway-test"))?
                == format!("DustAway.Coverage.0.24.0:{run_id}"),
            "Wrong marker"
        );
    } else {
        ensure!(host == "SK-20201215SSBP", "Wrong audit host");
    }
    let platform = Arc::new(dustaway_windows::WindowsPlatform::new(Scope::User)?);
    let engine = Engine::open(platform, Scope::User, std::path::Path::new(":memory:"))?;
    let ids: Vec<String> = [
        "appcache.codebuddy_extension_logs",
        "appcache.workbuddy_extension_logs",
        "appcache.todesk_logs",
        "appcache.todesk_web",
        "appcache.dingtalk_logs",
        "appcache.webtoapi_web",
        "appcache.webtoapi_logs",
        "appcache.dbx_web",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    ensure!(
        ids.iter().all(|id| app_cache::rule(id)
            .is_some_and(|s| s.kind == "readonly_observed" && !s.cleanable())),
        "Wrong scope"
    );
    let session = engine.start_scan(ids.clone())?;
    let finished = acceptance_wait::wait(|| engine.is_busy(), || engine.cancel());
    let state = match finished {
        acceptance_wait::WaitOutcome::Finished => "completed",
        acceptance_wait::WaitOutcome::Cancelled => "cancelled",
        acceptance_wait::WaitOutcome::Uncertain => "uncertain",
    };
    let mut total = 0u64;
    let rows:Vec<_>=engine.summary().rules.into_iter().map(|row| -> Result<_> {
        let known=row.bytes.parse::<u64>()?+row.observation.as_ref().map(|o|o.retained_bytes.parse::<u64>()).transpose()?.unwrap_or(0);
        total+=known;
        let mut reasons=row.observation.as_ref().map(|o|o.reasons.clone()).unwrap_or_default();
        if row.files>0 {reasons.push(ObservedReason{code:ReasonCode::ValidationPending,files:row.files,bytes:row.bytes.clone()});}
        Ok(serde_json::json!({"rule":row.rule_id,"phase":row.phase,"knownBytes":known.to_string(),"retainedBytes":known.to_string(),"candidateFiles":row.files,"candidateBytes":row.bytes,"retainedFiles":row.observation.as_ref().map(|o|o.retained_files+row.files),"selectableBytes":"0","unknownCapacity":row.coverage.as_ref().is_none_or(|c|!c.complete)||row.observation.as_ref().is_none_or(|o|row.skipped>o.retained_files),"readFailed":row.scan_failed,"scanIssue":row.scan_issue,"availability":row.availability,"reasons":reasons,"application":row.observation.and_then(|o|o.application).map(|a|serde_json::json!({"identityStatus":a.identity_status,"inventoryComplete":a.inventory_complete}))}))
    }).collect::<Result<Vec<_>>>()?;
    let mut gates = vec![];
    if finished == acceptance_wait::WaitOutcome::Finished {
        for id in &ids {
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
            ensure!(rejected, "Read-only rule accepted plan");
            gates.push(id);
        }
    }
    Ok(
        serde_json::json!({"schema":3,"version":env!("CARGO_PKG_VERSION"),"runId":run_id,"host":host,"ordinaryUser":ordinary,"mode":"read_only","status":state,"rules":rows,"newlyRecognizedKnownBytes":total.to_string(),"retainedKnownBytes":total.to_string(),"selectableDeltaBytes":"0","rejectedRulePlans":gates,"deletionAttempted":false,"webpageRecoveryVerified":false}),
    )
}
fn main() {
    match run() {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            if value["status"] != "completed" {
                std::process::exit(2);
            }
        }
        Err(_) => {
            eprintln!("Read-only coverage probe failed; no cleanup executed.");
            std::process::exit(1);
        }
    }
}
