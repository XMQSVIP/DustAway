//! Closed, compiled application-cache locations. Recognition is not deletion permission.
use crate::{CleanupPolicy, DataKind, RuleDescriptor};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheSpec {
    #[serde(default = "initial_version")]
    pub version: u32,
    pub id: String,
    pub app: String,
    pub title: String,
    pub folder: String,
    pub base: String,
    pub paths: Vec<String>,
    pub kind: String,
    pub age: u32,
    pub processes: Vec<String>,
    pub aliases: Vec<String>,
    pub impact: String,
}
fn initial_version() -> u32 {
    1
}
impl CacheSpec {
    pub fn cleanable(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "vsix" | "vscode_old_code" | "documented_gpu" | "documented_code"
        )
    }
    pub fn descriptor(&self) -> RuleDescriptor {
        RuleDescriptor {
            id: self.id.clone(),
            version: self.version,
            title: self.title.clone(),
            description: self.impact.clone(),
            impact: self.impact.clone(),
            root: "application_cache".into(),
            relative: self.base.clone(),
            min_age_days: self.age,
            process: self.processes.first().cloned(),
            requires_admin: false,
            default_selected: false,
            cleanup_policy: CleanupPolicy::Review,
            cleanup_validated: self.cleanable(),
            app_id: None,
            data_kind: DataKind::Cache,
        }
    }
}
pub fn catalog() -> &'static [CacheSpec] {
    static CATALOG: OnceLock<Vec<CacheSpec>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../../../rules/application-cache.json"))
            .expect("compiled cache catalog")
    })
}
pub fn rule(id: &str) -> Option<&'static CacheSpec> {
    catalog().iter().find(|s| s.id == id)
}
/// Registration names often omit spaces (for example "123浜戠洏"). This remains
/// a display hint, never installation verification or deletion authorization.
pub fn registered_name_matches(name: &str, aliases: &[&str]) -> bool {
    let normalize = |s: &str| {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    };
    let name = normalize(name);
    aliases
        .iter()
        .any(|a| !a.is_empty() && name.contains(&normalize(a)))
}
pub fn valid_descriptor(r: &RuleDescriptor) -> bool {
    rule(&r.id)
        .is_some_and(|s| serde_json::to_value(r).ok() == serde_json::to_value(s.descriptor()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_closed_locations_and_no_implicit_permissions() {
        assert!(registered_name_matches("123浜戠洏 3.2.3", &["123 浜戠洏"]));
        assert!(!registered_name_matches("Other product", &["123 浜戠洏"]));
        let mut ids = std::collections::HashSet::new();
        for s in catalog() {
            assert!(ids.insert(&s.id));
            assert!(matches!(s.folder.as_str(), "local" | "roaming"));
            for p in std::iter::once(&s.base).chain(s.paths.iter()) {
                assert!(
                    !p.starts_with('/')
                        && !p.contains(':')
                        && !p.contains("..")
                        && !p.contains('*')
                );
            }
            assert!(valid_descriptor(&s.descriptor()));
            let mut forged = s.descriptor();
            forged.relative = "Documents".into();
            assert!(!valid_descriptor(&forged));
            forged = s.descriptor();
            forged.cleanup_validated = !forged.cleanup_validated;
            assert!(!valid_descriptor(&forged));
            assert!(!s.descriptor().default_selected);
            if s.cleanable() {
                assert!(matches!(
                    s.app.as_str(),
                    "vscode" | "vscode_insiders" | "cherry" | "pycharm"
                ));
            }
        }
    }
}
