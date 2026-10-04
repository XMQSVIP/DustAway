//! Read-only scan of the installed uv cache. Never prepares or executes maintenance.
use anyhow::Result;
use dustaway_core::{Platform, ScanEvent, Scope};
use std::sync::atomic::AtomicBool;

fn main() -> Result<()> {
    let platform = dustaway_windows::WindowsPlatform::new(Scope::User)?;
    let rule = dustaway_core::rules::catalog()?
        .into_iter()
        .find(|rule| rule.id == "dev.uv")
        .unwrap();
    let mut files = 0;
    platform.scan(&rule, &[], &AtomicBool::new(false), &mut |event| {
        if matches!(event, ScanEvent::File(_)) {
            files += 1;
        }
    })?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "files": files,
            "tool": platform.tool_discovery("dev.uv"),
            "deletion_attempted": false,
        }))?
    );
    Ok(())
}
