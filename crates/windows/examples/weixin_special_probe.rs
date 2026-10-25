//! Read-only production adapters; exports counts and reasons, never account names or paths.
#[path = "../src/acceptance_wait.rs"]
mod acceptance_wait;
use acceptance_wait::{WaitOutcome, wait};
use anyhow::{Result, ensure};
use dustaway_core::*;
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};

fn probe() -> Result<serde_json::Value> {
    let run = std::env::var("DUSTAWAY_WEIXIN_RUN")?;
    ensure!(
        (8..=80).contains(&run.len())
            && run.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'),
        "Invalid run"
    );
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6"
            && !dustaway_windows::security::elevated()?,
        "Ordinary test user required"
    );
    let root = PathBuf::from(std::env::var("LOCALAPPDATA")?)
        .join("DustAwayAcceptance/weixin-0211")
        .join(&run);
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "Wrong test location"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-test"))?
            == format!("DustAway.Weixin.0.21.1:{run}"),
        "Wrong marker"
    );
    let platform = Arc::new(dustaway_windows::WindowsPlatform::new(Scope::User)?);
    let engine = Engine::open(platform.clone(), Scope::User, &root.join("probe.sqlite"))?;
    let ids: Vec<_> = rules::for_scope(Scope::User)?
        .into_iter()
        .filter(|r| r.root == "weixin")
        .map(|r| r.id)
        .collect();
    engine.start_scan(ids)?;
    let outcome = wait(|| engine.is_busy(), || engine.cancel());
    let support = dustaway_windows::applications::support()
        .into_iter()
        .find(|a| a.app_id == "weixin");
    let checks: Vec<_>=engine.summary().rules.into_iter().map(|row| {
        let unknown = row.coverage.as_ref().is_none_or(|c| !c.complete) || row.observation.as_ref().is_none_or(|o| row.skipped>o.retained_files);
        serde_json::json!({"rule":row.rule_id,"phase":row.phase,"candidateFiles":row.files,"candidateBytes":row.bytes,
            "retainedFiles":row.observation.as_ref().map(|o|o.retained_files),"retainedBytes":row.observation.as_ref().map(|o|&o.retained_bytes),
            "unknownCapacity":unknown,"readFailed":row.scan_failed,"reason":row.scan_issue,"blocked":row.availability,
            "reasons":row.reasons,"retainedReasons":row.observation.map(|o|o.reasons)})
    }).collect();
    let attachments =
        AttachmentEngine::new(Arc::new(dustaway_windows::attachments::WindowsAttachments));
    let found = attachments.discover(AttachmentProvider::Weixin);
    let mut accounts = Vec::new();
    let mut state = match outcome {
        WaitOutcome::Finished => "completed",
        WaitOutcome::Cancelled => "cancelled",
        WaitOutcome::Uncertain => "uncertain",
    };
    if outcome == WaitOutcome::Finished {
        for location in &found.locations {
            let session = attachments.start(&location.token, vec![])?;
            let done = wait(|| attachments.is_busy(), || attachments.cancel());
            if done != WaitOutcome::Finished {
                state = if done == WaitOutcome::Uncertain {
                    "uncertain"
                } else {
                    "cancelled"
                };
                break;
            }
            let page = attachments.page(AttachmentQuery {
                session,
                kind: None,
                min_age_days: 0,
                search: String::new(),
                offset: 0,
                selected_ids: None,
            })?;
            ensure!(
                page.categories.iter().map(|c| c.files).sum::<usize>() == page.total
                    && page.filtered_bytes == page.snapshot.bytes,
                "Accounting mismatch"
            );
            accounts.push(serde_json::json!({"accountId":format!("{:x}",Sha256::digest(location.path.as_bytes())),"version":location.version,
                "state":page.snapshot.state,"files":page.total,"bytes":page.filtered_bytes,"issues":page.snapshot.issues,
                "unknownCapacity":page.snapshot.state!="complete","selectableFiles":page.selectable_files,"selectableBytes":page.selectable_bytes,"categories":page.categories}));
            attachments.close()?;
        }
    }
    Ok(
        serde_json::json!({"schema":2,"runId":run,"version":env!("CARGO_PKG_VERSION"),"mode":"read_only","ordinaryUser":true,
        "status":state,"checks":checks,"discovery":found.state,"discoveryIssues":found.issues.len(),"accounts":accounts,
        "application":support.map(|a|serde_json::json!({"version":a.version,"identityVerified":a.identity_verified,"status":a.status,"runningComponents":a.running_components.len()})),
        "deletionAttempted":false,"webpageRecoveryVerified":false}),
    )
}
fn main() {
    match probe() {
        Ok(value) => {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            if value["status"] != "completed" {
                std::process::exit(2);
            }
        }
        Err(_) => {
            eprintln!("Read-only probe failed; no cleanup was requested.");
            std::process::exit(1);
        }
    }
}
