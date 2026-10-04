//! Read-only developer evidence. Never exports filenames, locations or raw errors.
use dustaway_core::{
    ApplicationSupport, Platform, ReasonCode, ScanEvent, Scope, applications::APPLICATION_RULES,
};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

pub fn inventory(apps: &[ApplicationSupport]) -> Value {
    json!({
        "schema_version": 3, "build_version": env!("CARGO_PKG_VERSION"),
        "catalog_id": dustaway_core::applications::evidence_catalog_id(),
        "sampled_at": dustaway_core::now(), "mode": "read_only",
        "applications": apps.iter().map(|app| json!({
            "app_id": app.app_id, "version": app.version,
            "supported_version": app.supported_version,
            "identity_verified": app.identity_verified, "scan_supported": app.scan_supported,
            "status": app.status, "running_component_count": app.running_components.len(),
            // Freeze v1-v3 wire shape: new IPC capability fields are not legacy evidence.
            "rules": app.rules.iter().map(|r| json!({
                "rule_id": r.rule_id, "cleanup_validated": r.cleanup_validated,
                "scan_basis": r.scan_basis, "layout_id": r.layout_id,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "deletion_performed": false, "acceptance_status": "not_run",
    })
}

pub fn scan(platform: &dyn Platform, apps: &[ApplicationSupport]) -> anyhow::Result<Value> {
    let mut result = inventory(apps);
    let mut rows = Vec::new();
    for rule in dustaway_core::rules::for_scope(Scope::User)?
        .into_iter()
        .filter(|r| r.app_id.is_some())
    {
        let mut files = 0u64;
        let mut bytes = 0u64;
        let mut checked = 0u64;
        let mut retained = vec![];
        let mut issues = vec![];
        let mut notices = vec![];
        let start = std::time::Instant::now();
        let scanned = platform.scan(
            &rule,
            &[],
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::Checked => checked += 1,
                ScanEvent::File(file) => {
                    files += 1;
                    bytes += file.bytes;
                }
                ScanEvent::Retained(code) => dustaway_core::count_reason(&mut retained, code),
                ScanEvent::CoverageIssue(code) => dustaway_core::count_reason(&mut issues, code),
                ScanEvent::NoticeCode(code) => dustaway_core::count_reason(&mut notices, code),
                // Free-text events can contain private paths. Never serialize them.
                _ => {}
            },
        );
        let error = scanned.err().map(|e| crate::fs::error_reason(&e));
        let blocked = match platform.blocked(&rule) {
            Ok(reason) => reason.map(|r| r.code),
            Err(_) => Some(ReasonCode::ProcessUnknown),
        };
        let spec = APPLICATION_RULES
            .iter()
            .find(|r| r.id == rule.id)
            .expect("registered application rule");
        let support = apps
            .iter()
            .find(|a| a.app_id == spec.app)
            .and_then(|a| a.rules.iter().find(|r| r.rule_id == rule.id));
        rows.push(json!({
            "rule_id": rule.id, "rule_version": rule.version, "app_id": spec.app,
            "min_age_days": spec.age, "data_kind": spec.kind,
            "scan_basis": support.map(|r| r.scan_basis).unwrap_or_default(),
            "layout_id": support.and_then(|r| r.layout_id.as_deref()),
            "checked": checked, "files": files, "logical_bytes": bytes.to_string(),
            "retained": retained, "issues": issues, "notices": notices,
            "coverage_complete": error.is_none() && issues.is_empty(),
            "error_code": error, "blocked": blocked, "elapsed_ms": start.elapsed().as_millis(),
            "inspection_status": if error.is_some() { "failed" } else { "read_only" },
            "acceptance_status": "not_run", "deletion_performed": false,
        }));
    }
    result["rules"] = json!(rows);
    result["scan_scope"] = json!("builtin_application_rules_without_user_exclusions");
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dustaway_core::{FileRecord, RuleDescriptor};
    struct ReadOnly;
    impl Platform for ReadOnly {
        fn scan(
            &self,
            _: &RuleDescriptor,
            _: &[String],
            _: &AtomicBool,
            emit: &mut dyn FnMut(ScanEvent),
        ) -> anyhow::Result<()> {
            emit(ScanEvent::Notice("C:\\Users\\PRIVATE\\account".into()));
            emit(ScanEvent::Skipped("private account text".into()));
            emit(ScanEvent::File(FileRecord {
                path_display: "PRIVATE file".into(),
                bytes: 42,
                modified_unix: 0,
                opaque: "PRIVATE".into(),
            }));
            emit(ScanEvent::CoverageIssue(ReasonCode::DirectoryInaccessible));
            anyhow::bail!("C:\\Users\\PRIVATE\\cannot read")
        }
        fn blocked(
            &self,
            _: &RuleDescriptor,
        ) -> anyhow::Result<Option<dustaway_core::BlockReason>> {
            anyhow::bail!("PRIVATE")
        }
        fn delete(&self, _: &RuleDescriptor, _: &FileRecord, _: &[String]) -> anyhow::Result<()> {
            panic!("evidence must never delete")
        }
        fn free_space(&self) -> anyhow::Result<u64> {
            panic!("not needed")
        }
    }
    #[test]
    fn evidence_redacts_text_and_never_claims_acceptance() {
        let app = ApplicationSupport {
            app_id: "weixin".into(),
            title: "PRIVATE title".into(),
            version: Some("4.1.13.65".into()),
            supported_version: "4.1.13.65".into(),
            identity_verified: true,
            scan_supported: true,
            cleanup_validated: false,
            rules: vec![dustaway_core::ApplicationRuleSupport {
                rule_id: "weixin.logs".into(),
                cleanup_validated: false,
                scan_basis: dustaway_core::ScanBasis::VerifiedVersion,
                layout_id: Some("weixin-diagnostics-v1".into()),
                cleanup_versions: Some(vec![]),
                cleanup_block: Some(ReasonCode::ValidationPending),
            }],
            status: Some(ReasonCode::ApplicationRunning),
            message: "PRIVATE error".into(),
            running_components: vec!["PRIVATE component".into()],
            locations: vec!["C:\\Users\\PRIVATE".into()],
        };
        let result = scan(&ReadOnly, &[app]).unwrap();
        let text = result.to_string();
        assert!(!text.contains("PRIVATE") && !text.contains("private"));
        assert!(!text.contains("cleanup_versions") && !text.contains("cleanup_block"));
        assert_eq!(
            result["rules"].as_array().unwrap().len(),
            APPLICATION_RULES.len()
        );
        for row in result["rules"].as_array().unwrap() {
            assert_eq!(row["logical_bytes"], "42");
            assert_eq!(row["coverage_complete"], false);
            assert_eq!(row["inspection_status"], "failed");
            assert_eq!(row["acceptance_status"], "not_run");
            assert_eq!(row["deletion_performed"], false);
        }
    }
    #[test]
    fn checker_catalog_matches_compiled_rules_and_versions() {
        let catalog: Value =
            serde_json::from_str(include_str!("../../../scripts/application-catalog-v3.json"))
                .unwrap();
        assert_eq!(
            catalog["catalog_id"],
            dustaway_core::applications::evidence_catalog_id()
        );
        assert_eq!(
            catalog["rules"],
            dustaway_core::applications::evidence_catalog()
        );
        let catalog = &catalog["rules"];
        assert_eq!(catalog.as_array().unwrap().len(), APPLICATION_RULES.len());
        for spec in APPLICATION_RULES {
            let item = catalog
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["rule_id"] == spec.id)
                .unwrap();
            assert_eq!(item["app_id"], spec.app);
            assert_eq!(item["min_age_days"], spec.age);
            assert_eq!(
                item["supported_version"],
                dustaway_core::applications::application(spec.app)
                    .unwrap()
                    .version
            );
            assert_eq!(
                spec.validated(),
                matches!(spec.id, "dingtalk.http" | "dingtalk.code")
            );
        }
    }
}
