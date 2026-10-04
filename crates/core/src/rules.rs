use crate::browsers::{CHROMIUM_CACHES, chromium};
use crate::model::{RuleDescriptor, Scope};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    rule: Vec<RuleDescriptor>,
}

pub fn validate(rule: &RuleDescriptor) -> Result<()> {
    let valid = if let Some(product) = chromium(&rule.root) {
        CHROMIUM_CACHES.iter().any(|(kind, relative)| {
            rule.id == format!("{}.{}", product.id, kind) && rule.relative == *relative
        }) && rule.process.as_deref() == Some(product.process)
            && rule.min_age_days == 0
            && !rule.requires_admin
    } else {
        match rule.root.as_str() {
            "iis_logs" => {
                rule.id == "windows.iis_logs"
                    && rule.version == 1
                    && rule.relative.is_empty()
                    && rule.process.is_none()
                    && rule.min_age_days == 30
                    && rule.requires_admin
                    && !rule.default_selected
                    && rule.cleanup_policy == crate::CleanupPolicy::Review
                    && rule.data_kind == crate::DataKind::Diagnostics
                    && rule.app_id.is_none()
            }
            "application_cache" => crate::app_cache::valid_descriptor(rule),
            "developer" => crate::developer::valid_descriptor(rule),
            "windows_residue" => crate::residue::valid_descriptor(rule),
            "weixin" | "qq" | "wps" | "feishu" | "dingtalk" | "baidu_netdisk" | "quark_netdisk"
            | "thunder" => crate::applications::valid_descriptor(rule),
            "user_temp" | "system_temp" => {
                rule.id == format!("windows.{}", rule.root)
                    && rule.relative.is_empty()
                    && rule.process.is_none()
                    && rule.min_age_days == 1
                    && rule.version == 2
                    && rule.requires_admin == (rule.root == "system_temp")
            }
            "firefox" => {
                rule.id == "firefox.http"
                    && rule.relative == "cache2"
                    && rule.process.as_deref() == Some("firefox.exe")
                    && rule.min_age_days == 0
                    && !rule.requires_admin
            }
            _ => false,
        }
    };
    let metadata_valid = rule.root == "iis_logs"
        || crate::app_cache::rule(&rule.id).is_some()
        || crate::residue::rule(&rule.id).is_some()
        || crate::developer::rule(&rule.id).is_some()
        || crate::applications::rule(&rule.id).is_some()
        || (rule.app_id.is_none() && rule.data_kind == crate::DataKind::Cache);
    ensure!(
        valid && metadata_valid && rule.version > 0,
        "规则身份、目录或进程边界不匹配"
    );
    Ok(())
}

pub fn catalog() -> Result<Vec<RuleDescriptor>> {
    let mut catalog: Catalog = toml::from_str(include_str!("../../../rules/builtin.toml"))?;
    catalog
        .rule
        .extend(crate::app_cache::catalog().iter().map(|s| s.descriptor()));
    let mut ids = HashSet::new();
    for rule in &catalog.rule {
        ensure!(ids.insert(&rule.id), "重复的规则标识");
        validate(rule)?;
    }
    Ok(catalog.rule)
}
pub fn for_scope(scope: Scope) -> Result<Vec<RuleDescriptor>> {
    Ok(catalog()?
        .into_iter()
        .filter(|r| r.requires_admin == (scope == Scope::System))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_rule_metadata_and_browser_reason_remain_readable() {
        let mut old = serde_json::to_value(&catalog().unwrap()[0]).unwrap();
        old.as_object_mut().unwrap().remove("app_id");
        old.as_object_mut().unwrap().remove("data_kind");
        let restored: RuleDescriptor = serde_json::from_value(old).unwrap();
        assert!(restored.app_id.is_none());
        assert_eq!(restored.data_kind, crate::DataKind::Cache);
        validate(&restored).unwrap();
        assert_eq!(
            serde_json::from_str::<crate::ReasonCode>("\"browser_running\"").unwrap(),
            crate::ReasonCode::BrowserRunning
        );
    }
    #[test]
    fn builtins_are_narrow_and_partitioned() {
        assert_eq!(
            catalog().unwrap().len(),
            77 + crate::app_cache::catalog().len()
        );
        assert_eq!(for_scope(Scope::System).unwrap().len(), 2);
        assert!(
            for_scope(Scope::User)
                .unwrap()
                .iter()
                .all(|r| !r.requires_admin)
        );
    }

    #[test]
    fn new_rules_are_opt_in_and_reject_mixed_boundaries() {
        let rules = catalog().unwrap();
        let new: Vec<_> = rules
            .iter()
            .filter(|r| matches!(r.root.as_str(), "brave" | "vivaldi"))
            .collect();
        assert_eq!(new.len(), 6);
        for rule in new {
            assert!(!rule.default_selected);
            for relative in [
                "",
                "..",
                "Service Worker/CacheStorage",
                "Local Storage",
                "Preferences",
            ] {
                let mut bad = rule.clone();
                bad.relative = relative.into();
                assert!(validate(&bad).is_err());
            }
            let mut bad = rule.clone();
            bad.process = None;
            assert!(validate(&bad).is_err());
            bad = rule.clone();
            bad.root = "user_temp".into();
            assert!(validate(&bad).is_err());
            bad = rule.clone();
            bad.requires_admin = true;
            assert!(validate(&bad).is_err());
        }
    }
}
