//! Compile-time release evidence, independent of versions observed during read-only scanning.
use crate::{ReasonCode, RuleDescriptor, applications};

#[derive(Debug, Clone, Copy)]
pub struct CleanupAcceptance {
    pub rule_id: &'static str,
    pub rule_version: u32,
    pub layout_id: &'static str,
    pub application_version: &'static str,
    pub evidence_id: &'static str,
}

// Real cleanup, unchanged protected files, regenerated cache and human functional
// confirmation are linked in docs/verification-home-cleanup.md. Synthetic tests
// alone never authorize a new rule or layout. Observed application versions are
// evidence metadata, not an allowlist; runtime identity/layout checks still apply.
pub const ACCEPTANCES: &[CleanupAcceptance] = &[
    CleanupAcceptance {
        rule_id: "dingtalk.http",
        rule_version: 1,
        layout_id: "dingtalk-web-profile-v1",
        application_version: "8.2.0.119",
        evidence_id: "home-cleanup-20261001-dingtalk-http",
    },
    CleanupAcceptance {
        rule_id: "dingtalk.code",
        rule_version: 1,
        layout_id: "dingtalk-web-profile-v1",
        application_version: "8.2.0.119",
        evidence_id: "home-cleanup-20261001-dingtalk-code",
    },
];
pub const PREPARATION_RULES: &[&str] = &[
    "weixin.logs",
    "dingtalk.http",
    "dingtalk.code",
    "baidu_netdisk.http",
    "baidu_netdisk.code",
    "baidu_netdisk.gpu",
];

/// Pure matcher; passing records here does not change the compiled release registry.
pub fn versions_in(rule: &RuleDescriptor, records: &[CleanupAcceptance]) -> Vec<String> {
    let Some(spec) = applications::rule(&rule.id) else {
        return vec![];
    };
    if !applications::valid_descriptor(rule) {
        return vec![];
    }
    versions_for_spec(spec, rule.version, records)
}

pub fn versions_for_spec(
    spec: &applications::ApplicationRule,
    rule_version: u32,
    records: &[CleanupAcceptance],
) -> Vec<String> {
    let mut versions: Vec<_> = records
        .iter()
        .filter(|record| {
            record.rule_id == spec.id
                && record.rule_version == rule_version
                && record.layout_id == spec.layout_id
                && !record.evidence_id.trim().is_empty()
                && record.application_version.split('.').count() == 4
                && record
                    .application_version
                    .split('.')
                    .all(|v| !v.is_empty() && v.bytes().all(|c| c.is_ascii_digit()))
        })
        .map(|record| record.application_version.to_owned())
        .collect();
    versions.sort();
    versions.dedup();
    versions
}

pub fn versions(rule: &RuleDescriptor) -> Vec<String> {
    versions_in(rule, ACCEPTANCES)
}

pub fn release_enabled(rule: &RuleDescriptor) -> bool {
    crate::rules::validate(rule).is_ok()
        && rule.cleanup_validated
        && (rule.id != "windows.crash_dumps" || crate::cleanup_evidence::documented_files(rule))
        && (rule.root != "application_cache" || crate::cleanup_evidence::documented_app_cache(rule))
        && (rule.app_id.is_none() || !versions(rule).is_empty())
}

pub fn release_block(rule: &RuleDescriptor) -> Option<ReasonCode> {
    (!release_enabled(rule)).then_some(ReasonCode::ValidationPending)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_is_bound_to_one_rule_layout_and_version_not_scan_observations() {
        let rules = crate::rules::catalog().unwrap();
        let http = rules.iter().find(|r| r.id == "dingtalk.http").unwrap();
        let code = rules.iter().find(|r| r.id == "dingtalk.code").unwrap();
        let record = CleanupAcceptance {
            rule_id: "dingtalk.http",
            rule_version: 1,
            layout_id: "dingtalk-web-profile-v1",
            application_version: "1.2.3.4",
            evidence_id: "synthetic-unit-test-not-release-evidence",
        };
        assert_eq!(versions_in(http, &[record]), ["1.2.3.4"]);
        assert!(versions_in(code, &[record]).is_empty());
        for invalid in [
            CleanupAcceptance {
                rule_version: 2,
                ..record
            },
            CleanupAcceptance {
                layout_id: "another-layout",
                ..record
            },
            CleanupAcceptance {
                evidence_id: "",
                ..record
            },
            CleanupAcceptance {
                application_version: "*",
                ..record
            },
        ] {
            assert!(versions_in(http, &[invalid]).is_empty());
        }
        assert_ne!(
            applications::application("dingtalk").unwrap().version,
            "1.2.3.4"
        );
        assert_eq!(versions(http), ["8.2.0.119"]);
        assert_eq!(versions(code), ["8.2.0.119"]);
        let mut forged = rules
            .iter()
            .find(|r| r.id == "baidu_netdisk.code")
            .unwrap()
            .clone();
        forged.cleanup_validated = true;
        assert!(!release_enabled(&forged));
        let mut stripped = forged.clone();
        stripped.app_id = None;
        assert!(!release_enabled(&stripped));
        assert_eq!(release_block(&forged), Some(ReasonCode::ValidationPending));
        assert_eq!(
            rules
                .iter()
                .filter(|r| !r.requires_admin && release_enabled(r))
                .count(),
            42
        );
    }

    #[test]
    fn real_acceptance_authorizes_the_reviewed_rule_and_layout_independent_of_app_version() {
        let rules = crate::rules::catalog().unwrap();
        let accepted = ["dingtalk.http", "dingtalk.code"];
        for id in accepted {
            let rule = rules.iter().find(|r| r.id == id).unwrap();
            assert!(release_enabled(rule));
            assert!(!rule.default_selected);
            assert_eq!(rule.cleanup_policy, crate::CleanupPolicy::Review);
            assert_eq!(release_block(rule), None);
            let mut disabled = rule.clone();
            disabled.cleanup_validated = false;
            assert!(!release_enabled(&disabled));
            assert_eq!(
                release_block(&disabled),
                Some(ReasonCode::ValidationPending)
            );
        }
        assert_ne!(ACCEPTANCES[0].evidence_id, ACCEPTANCES[1].evidence_id);
        for rule in rules
            .iter()
            .filter(|r| r.app_id.is_some() && !accepted.contains(&r.id.as_str()))
        {
            assert!(!release_enabled(rule));
            assert_eq!(release_block(rule), Some(ReasonCode::ValidationPending));
        }
    }
}
