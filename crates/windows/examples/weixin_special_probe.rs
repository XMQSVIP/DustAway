//! Read-only production adapters; exports counts, never attachment names/content.
use anyhow::{Result, ensure};
use dustaway_core::*;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let platform = dustaway_windows::WindowsPlatform::new(Scope::User)?;
    let rules = rules::for_scope(Scope::User)?;
    let mut checks = Vec::new();
    for rule in rules.iter().filter(|r| r.root == "weixin") {
        checks.push(serde_json::json!({
            "rule": rule.id,
            "licensed": platform.cleanup_validated(rule),
            "blocked": platform.blocked(rule)?.map(|r| r.code),
        }));
    }
    let engine = AttachmentEngine::new(Arc::new(dustaway_windows::attachments::WindowsAttachments));
    let found = engine.discover(AttachmentProvider::Weixin);
    let mut accounts = Vec::new();
    for location in &found.locations {
        let session = engine.start(&location.token, vec![])?;
        let started = Instant::now();
        while engine.is_busy() && started.elapsed() < Duration::from_secs(90) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if engine.is_busy() {
            engine.cancel();
            anyhow::bail!("Read-only scan timed out");
        }
        let page = engine.page(AttachmentQuery {
            session: session.clone(),
            kind: None,
            min_age_days: 0,
            search: String::new(),
            offset: 0,
            selected_ids: None,
        })?;
        ensure!(
            page.categories.iter().map(|c| c.files).sum::<usize>() == page.total,
            "Classification does not reconcile"
        );
        ensure!(
            page.filtered_bytes == page.snapshot.bytes,
            "Byte totals do not reconcile"
        );
        accounts.push(serde_json::json!({
            "version": location.version,
            "state": page.snapshot.state,
            "files": page.total,
            "bytes": page.filtered_bytes,
            "selectable_files": page.selectable_files,
            "selectable_bytes": page.selectable_bytes,
            "categories": page.categories,
        }));
        engine.close()?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "checks": checks, "discovery": found.state,
            "accounts": accounts, "deletion_attempted": false,
        }))?
    );
    Ok(())
}
