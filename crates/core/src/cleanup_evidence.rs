//! Offline evidence metadata. It never authorizes an arbitrary path or command.
use crate::RuleDescriptor;
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Evidence {
    pub rule_id: String,
    pub rule_version: u32,
    pub root: String,
    pub relative: String,
    pub method: String,
    pub real_acceptance: String,
}
pub fn catalog() -> Vec<Evidence> {
    serde_json::from_str(include_str!("../../../rules/cleanup-evidence.json"))
        .expect("compiled evidence")
}
pub fn documented_files(rule: &RuleDescriptor) -> bool {
    rule.id == "windows.crash_dumps"
        && catalog().iter().any(|e| {
            e.rule_id == rule.id
                && e.rule_version == rule.version
                && e.root == rule.root
                && e.relative == rule.relative
                && e.method == "files"
                && e.real_acceptance == "passed"
        })
}
pub fn documented_app_cache(rule: &RuleDescriptor) -> bool {
    crate::app_cache::rule(&rule.id).is_some_and(|s| s.cleanable())
        && catalog().iter().any(|e| {
            e.rule_id == rule.id
                && e.rule_version == rule.version
                && e.root == rule.root
                && e.relative == rule.relative
                && e.method == "files"
        })
}
pub fn documented_tool(rule_id: &str) -> bool {
    let Ok(rules) = crate::rules::catalog() else {
        return false;
    };
    let Some(rule) = rules.iter().find(|r| r.id == rule_id) else {
        return false;
    };
    catalog().iter().any(|e| {
        e.rule_id == rule.id
            && e.rule_version == rule.version
            && e.root == rule.root
            && e.relative == rule.relative
            && e.method == "official_tool"
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_matches_fixed_rules_and_covers_retained_items() {
        assert!(!documented_tool("dev.pnpm"));
        assert!(!documented_tool("dev.bun"));
        for id in ["dev.go_build", "dev.yarn", "dev.ccache"] {
            assert!(documented_tool(id));
        }
        let rules = crate::rules::catalog().unwrap();
        let evidence = catalog();
        let mut ids = std::collections::HashSet::new();
        for e in &evidence {
            assert!(ids.insert(&e.rule_id));
            let r = rules.iter().find(|r| r.id == e.rule_id).unwrap();
            assert_eq!(
                (r.version, &r.root, &r.relative),
                (e.rule_version, &e.root, &e.relative)
            );
        }
        for r in rules
            .iter()
            .filter(|r| !r.requires_admin && !crate::cleanup_acceptance::release_enabled(r))
        {
            assert!(
                evidence.iter().any(|e| e.rule_id == r.id),
                "missing evidence {}",
                r.id
            );
        }
        let mut dump = rules
            .iter()
            .find(|r| r.id == "windows.crash_dumps")
            .unwrap()
            .clone();
        assert!(!documented_files(&dump));
        assert!(!crate::cleanup_acceptance::release_enabled(&dump));
        dump.version = 1;
        assert!(!documented_files(&dump));
    }
}
