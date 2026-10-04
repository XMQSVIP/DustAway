//! Read-only installation/layout checks. Does not scan, prepare or delete files.
use anyhow::Result;
use dustaway_core::{Platform, Scope};

fn main() -> Result<()> {
    let platform = dustaway_windows::WindowsPlatform::new(Scope::User)?;
    let rules = dustaway_core::rules::catalog()?;
    let mut checks = Vec::new();
    for id in [
        "dingtalk.http",
        "dingtalk.code",
        "appcache.vscode_vsix",
        "appcache.vscode_code",
        "appcache.cherry_code",
        "appcache.pycharm_code",
    ] {
        let rule = rules.iter().find(|r| r.id == id).unwrap();
        let result = platform.blocked(rule);
        checks.push(serde_json::json!({
            "rule": id,
            "licensed": platform.cleanup_validated(rule),
            "blocked": result.as_ref().ok(),
            "error": result.err().map(|error| error.to_string()),
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "applications": dustaway_windows::applications::support(),
            "checks": checks,
            "deletion_attempted": false,
        }))?
    );
    Ok(())
}
