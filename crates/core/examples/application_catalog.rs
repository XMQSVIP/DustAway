//! Export the compiled registry; never inspects the host or enables cleanup.
fn main() -> anyhow::Result<()> {
    use dustaway_core::{applications::*, rules};
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": 3, "catalog_id": evidence_catalog_id(),
            "rules": evidence_catalog(), "preview_rules": rules::catalog()?,
        }))?
    );
    Ok(())
}
