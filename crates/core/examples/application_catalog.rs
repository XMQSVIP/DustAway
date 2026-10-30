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
            "file_cleanup_rule_ids": user.iter().filter(|r| dustaway_core::cleanup_acceptance::release_enabled(r)).map(|r| &r.id).collect::<Vec<_>>(),
            "official_tool_rule_ids": user.iter().filter(|r| r.id == "dev.uv" || dustaway_core::cleanup_evidence::documented_tool(&r.id)).map(|r| &r.id).collect::<Vec<_>>(),
            "release_evidence": dustaway_core::cleanup_acceptance::ACCEPTANCES.iter().map(|e| serde_json::json!({"rule_id": e.rule_id, "rule_version": e.rule_version, "layout_id": e.layout_id, "application_version": e.application_version, "evidence_id": e.evidence_id})).collect::<Vec<_>>(),
            "support_summary": {"version": env!("CARGO_PKG_VERSION"), "user_rules":user.len(), "file_cleanup_rules":files, "official_tool_rules":tools, "readonly_rules":user.len()-files-tools},
        }))?
    );
    Ok(())
}
