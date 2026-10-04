//! Offline declaration export. Does not inspect installed software or grant runtime authority.
fn main() {
    use dustaway_core::software::{RESIDUAL_RULE_VERSION, residual_rules};
    let rules: Vec<_> = residual_rules::catalog()
        .into_iter()
        .map(|spec| serde_json::json!({"rule": spec, "scope_sha256": spec.scope_sha256()}))
        .collect();
    let evidence: Vec<_> = residual_rules::ACCEPTANCES.iter().map(|record| {
        serde_json::json!({
            "rule_id": record.rule_id, "rule_version": record.rule_version,
            "scope_sha256": record.scope_sha256, "application_identity": record.application_identity,
            "application_version": record.application_version, "evidence_sha256": record.evidence_sha256,
        })
    }).collect();
    println!(
        "{}",
        serde_json::json!({
            "schema_version": 1, "uninstall_context_version": RESIDUAL_RULE_VERSION,
            "rules": rules, "compiled_acceptances": evidence,
        })
    );
}
