//! Export the compiled registry; never inspects the host or enables cleanup.
fn main() -> anyhow::Result<()> {
    use dustaway_core::{applications::*, rules};
    let user = rules::for_scope(dustaway_core::Scope::User)?;
    let files = user
        .iter()
        .filter(|r| dustaway_core::cleanup_acceptance::release_enabled(r))
        .count();
    let tools = user
        .iter()
        .filter(|r| r.id == "dev.uv" || dustaway_core::cleanup_evidence::documented_tool(&r.id))
        .count();
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 3, "catalog_id": evidence_catalog_id(),
            "rules": evidence_catalog(), "preview_rules": rules::catalog()?,
            "support_summary": {"version": env!("CARGO_PKG_VERSION"), "user_rules":user.len(), "file_cleanup_rules":files, "official_tool_rules":tools, "readonly_rules":user.len()-files-tools},
        }))?
    );
    Ok(())
}
