//! Residual authority is independent of ordinary cleanup, including its acceptance records.
use crate::{applications, browsers};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualKind {
    Cache,
    Settings,
    LocalData,
}
impl ResidualKind {
    pub fn category(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::Settings | Self::LocalData => "data",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualInspectionSupport {
    FixedLocations,
    Unsupported,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualCleanupSupport {
    Unsupported,
    NotValidated,
    /// Some scopes have evidence; this installation and each actual file still need verification.
    RequiresVerification,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualCapability {
    pub kind: ResidualKind,
    pub inspection: ResidualInspectionSupport,
    pub cleanup: ResidualCleanupSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locator {
    ChromiumProfile,
    FirefoxLocalProfile,
    FirefoxRoamingProfile,
    /// Reuses profile recognition only. It does not inherit cleanup authority.
    ApplicationProfile(String),
    /// Independently documented WPS locations; not derived from the diagnostics rule.
    WpsOfficeData,
    WpsAddins,
    StorePackage,
}
impl Locator {
    pub fn fixed_roaming_base(&self) -> Option<&'static str> {
        match self {
            Self::WpsOfficeData => Some("Kingsoft/office6"),
            Self::WpsAddins => Some("Kingsoft/wps/jsaddons"),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    File,
    Tree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResidualRule {
    pub id: String,
    pub family: String,
    pub version: u32,
    pub kind: ResidualKind,
    pub locator: Locator,
    pub relative: String,
    pub target: Target,
    /// Diagnostic rules retain their positive file-extension boundary after uninstall.
    pub extension: Option<String>,
}
impl ResidualRule {
    /// A changed scope cannot reuse old evidence merely by retaining its rule ID/version.
    pub fn scope_sha256(&self) -> String {
        let layout = match &self.locator {
            Locator::ApplicationProfile(id) => applications::rule(id).map(|spec| {
                serde_json::json!({
                    "app": spec.app, "folder": spec.folder, "profiles": spec.profiles,
                    "base": spec.base, "layout_id": spec.layout_id,
                })
            }),
            Locator::ChromiumProfile => browsers::chromium(&self.family)
                .map(|product| serde_json::json!({"base": product.user_data})),
            Locator::WpsOfficeData | Locator::WpsAddins => {
                Some(serde_json::json!({"roaming_base": self.locator.fixed_roaming_base()}))
            }
            Locator::StorePackage => Some(serde_json::json!({
                "ownership_contract": "current-user-exclusive-store-v2",
            })),
            _ => None,
        };
        let bytes = serde_json::json!({
            "rule": self, "layout": layout, "discovery_contract": "residual-profiles-v1",
        })
        .to_string();
        format!("{:x}", Sha256::digest(bytes.as_bytes()))
    }

    pub fn accepts_entry(&self, directory: bool, extension: Option<&str>) -> bool {
        if self.target == Target::File && directory {
            return false;
        }
        match &self.extension {
            Some(expected) => {
                !directory && extension.is_some_and(|value| value.eq_ignore_ascii_case(expected))
            }
            None => true,
        }
    }
}

fn add(
    rules: &mut Vec<ResidualRule>,
    family: &str,
    suffix: &str,
    kind: ResidualKind,
    locator: Locator,
    relative: &str,
    target: Target,
) {
    rules.push(ResidualRule {
        id: format!("uninstall.{family}.{suffix}"),
        family: family.into(),
        version: 1,
        kind,
        locator,
        relative: relative.into(),
        target,
        extension: None,
    });
}

/// Positive fixed scopes only. Unmatched content stays visible through read-only inspection.
pub fn catalog() -> Vec<ResidualRule> {
    use ResidualKind::{Cache, LocalData, Settings};
    use Target::{File, Tree};
    let mut rules = vec![];
    for product in browsers::CHROMIUM_PRODUCTS {
        for (suffix, relative) in browsers::CHROMIUM_CACHES {
            add(
                &mut rules,
                product.id,
                suffix,
                Cache,
                Locator::ChromiumProfile,
                relative,
                Tree,
            );
        }
        for (suffix, relative, kind, target) in [
            ("preferences", "Preferences", Settings, File),
            ("secure_preferences", "Secure Preferences", Settings, File),
            ("bookmarks", "Bookmarks", LocalData, File),
            ("history", "History", LocalData, File),
            ("cookies", "Network/Cookies", LocalData, File),
            ("local_storage", "Local Storage", LocalData, Tree),
            ("indexed_db", "IndexedDB", LocalData, Tree),
            ("sessions", "Sessions", LocalData, Tree),
        ] {
            add(
                &mut rules,
                product.id,
                suffix,
                kind,
                Locator::ChromiumProfile,
                relative,
                target,
            );
        }
    }
    add(
        &mut rules,
        "firefox",
        "http",
        Cache,
        Locator::FirefoxLocalProfile,
        "cache2",
        Tree,
    );
    for (suffix, relative, kind, target) in [
        ("preferences", "prefs.js", Settings, File),
        ("user_preferences", "user.js", Settings, File),
        ("cookies", "cookies.sqlite", LocalData, File),
        ("places", "places.sqlite", LocalData, File),
        ("storage", "storage", LocalData, Tree),
        ("bookmarks", "bookmarkbackups", LocalData, Tree),
        ("sessions", "sessionstore-backups", LocalData, Tree),
    ] {
        add(
            &mut rules,
            "firefox",
            suffix,
            kind,
            Locator::FirefoxRoamingProfile,
            relative,
            target,
        );
    }
    for spec in applications::APPLICATION_RULES {
        // Ordinary Weixin web-cache discovery does not authorize an uninstall
        // scope. Its profile Preferences and storage remain undeclared here.
        if spec.profiles == applications::ProfileLayout::WeixinWeb {
            continue;
        }
        let suffix = spec.id.strip_prefix(&format!("{}.", spec.app)).unwrap();
        add(
            &mut rules,
            spec.app,
            suffix,
            Cache,
            Locator::ApplicationProfile(spec.id.into()),
            spec.relative,
            Tree,
        );
        rules.last_mut().unwrap().extension = spec.extension.map(str::to_owned);
    }
    for app in applications::APPLICATIONS {
        let Some(basis) = applications::APPLICATION_RULES.iter().find(|spec| {
            spec.app == app.id
                && !matches!(
                    spec.profiles,
                    applications::ProfileLayout::Direct | applications::ProfileLayout::WeixinWeb
                )
        }) else {
            // Direct diagnostic layouts do not imply settings/data ownership.
            continue;
        };
        let preferences = if basis.profiles == applications::ProfileLayout::QqNumeric {
            "LocalPrefs.json"
        } else {
            "Preferences"
        };
        for (suffix, relative, kind, target) in [
            ("preferences", preferences, Settings, File),
            ("local_storage", "Local Storage", LocalData, Tree),
            ("indexed_db", "IndexedDB", LocalData, Tree),
        ] {
            add(
                &mut rules,
                app.id,
                suffix,
                kind,
                Locator::ApplicationProfile(basis.id.into()),
                relative,
                target,
            );
        }
    }
    // Independent vendor-described purposes; approval is still required for each scope.
    // Custom backup paths, add-in code and the shared Kingsoft parent are not included.
    for (suffix, locator, relative, kind, target) in [
        ("backups", Locator::WpsOfficeData, "backup", LocalData, Tree),
        (
            "published_addins",
            Locator::WpsAddins,
            "publish.xml",
            Settings,
            File,
        ),
        (
            "legacy_addins",
            Locator::WpsAddins,
            "jsplugins.xml",
            Settings,
            File,
        ),
    ] {
        add(&mut rules, "wps", suffix, kind, locator, relative, target);
    }
    for (suffix, leaf, kind) in [
        // LocalCache means excluded from backup, not disposable. MSIX can redirect
        // persistent settings and user data here; use the data confirmation category.
        ("local_cache", "LocalCache", LocalData),
        ("temporary", "TempState", Cache),
        ("settings", "Settings", Settings),
        ("local_data", "LocalState", LocalData),
        ("roaming_data", "RoamingState", LocalData),
    ] {
        add(
            &mut rules,
            "store",
            suffix,
            kind,
            Locator::StorePackage,
            leaf,
            Tree,
        );
        rules.last_mut().expect("Store rule").version = if suffix == "local_cache" { 4 } else { 3 };
    }
    rules
}

pub fn rule(id: &str) -> Option<ResidualRule> {
    catalog().into_iter().find(|rule| rule.id == id)
}

#[derive(Clone, Copy)]
pub struct ResidualAcceptance<'a> {
    pub rule_id: &'a str,
    pub rule_version: u32,
    pub scope_sha256: &'a str,
    /// Desktop family, or the exact Store package family (never every package of a version).
    pub application_identity: &'a str,
    pub application_version: &'a str,
    pub evidence_sha256: &'a str,
}

// Only independently reviewed real-software residue evidence belongs here.
// Neither ordinary cleanup approval nor a successful read-only scan populates this table.
pub const ACCEPTANCES: &[ResidualAcceptance<'static>] = &[
    firefox_evidence(
        "uninstall.firefox.http",
        "d6c10a700f1b67ad855451337f8bf815a73347bbe9b48cf98fd1996ba6d7642e",
    ),
    firefox_evidence(
        "uninstall.firefox.preferences",
        "bcda48ed02bae8d8656998e8e4ced1e8902f907217627558659c80fa2e76218e",
    ),
    firefox_evidence(
        "uninstall.firefox.user_preferences",
        "64d0a3d941fca016c4c43ac91a157a097468dc89f9bc9f657d71eb57dfaece3f",
    ),
    firefox_evidence(
        "uninstall.firefox.cookies",
        "3bb28c2dbb0b82d5cdc3f80c54cf32375fc9e79d0d2afa28ddfd389ffaed039d",
    ),
    firefox_evidence(
        "uninstall.firefox.places",
        "8e90fdcb72ad0fc696b7905dbf80782f4ed31d9e7484c209a8b2cc8574404226",
    ),
    firefox_evidence(
        "uninstall.firefox.storage",
        "35edd327db86eb6c09f8bbe4d9c2f7a442d8de969d5cf06cc2215e0e6bdbbd4f",
    ),
    firefox_history_evidence(
        "uninstall.firefox.bookmarks",
        "849fdcad1b334ad8659d7e0c19c3c83cd7c55fcae7d6535c87f8ea8565ffd46f",
    ),
    firefox_history_evidence(
        "uninstall.firefox.sessions",
        "5270742c132e325af3e94acdd9626f850a8eb61af83eb5be28b76291c6b3b090",
    ),
];

const fn firefox_evidence(
    rule_id: &'static str,
    scope_sha256: &'static str,
) -> ResidualAcceptance<'static> {
    // docs/evidence/software/firefox-rules-20261001.json records six completed
    // rule checks and the later empty-bookmark failure; no permission for that
    // unexercised rule, sessions or a whole profile is implied. Versions are evidence metadata.
    ResidualAcceptance {
        rule_id,
        rule_version: 1,
        scope_sha256,
        application_identity: "firefox",
        application_version: "140.17.0.1158",
        evidence_sha256: "7054e8e1188039ebeafad2b8d5c126a449c340a35f1e3f42c95459ca7fae1f71",
    }
}

const fn firefox_history_evidence(
    rule_id: &'static str,
    scope_sha256: &'static str,
) -> ResidualAcceptance<'static> {
    // Separate real browser-generated backups and native deletion evidence.
    // The original six-rule evidence is unchanged and cannot authorize these scopes.
    ResidualAcceptance {
        rule_id,
        rule_version: 1,
        scope_sha256,
        application_identity: "firefox",
        application_version: "140.17.0.1158",
        evidence_sha256: "8419dfa2ff643de5a073783f63853d25539e664351c574cab6cfcf3271a5a048",
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit())
}
fn valid_version(value: &str) -> bool {
    value.split('.').count() == 4
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && part.parse::<u16>().is_ok()
        })
}
fn accepted_in(
    spec: &ResidualRule,
    identity: &str,
    version: &str,
    records: &[ResidualAcceptance<'_>],
) -> bool {
    if records.is_empty()
        || rule(&spec.id).as_ref() != Some(spec)
        || identity.is_empty()
        || !valid_version(version)
        || (spec.family != "store" && identity != spec.family)
    {
        return false;
    }
    let scope = spec.scope_sha256();
    records.iter().any(|record| {
        record.rule_id == spec.id
            && record.rule_version == spec.version
            && record.scope_sha256 == scope
            && valid_hash(record.scope_sha256)
            && record.application_identity == identity
            && valid_version(record.application_version)
            && valid_hash(record.evidence_sha256)
    })
}
pub fn accepted(spec: &ResidualRule, identity: &str, version: &str) -> bool {
    accepted_in(spec, identity, version, ACCEPTANCES)
}

/// Describes declared fixed locations without filesystem access or trusting a display version.
/// A positive description must never substitute for `accepted` or a captured scope.
pub fn capabilities(identity: &super::SoftwareIdentity) -> Vec<ResidualCapability> {
    capabilities_in(identity, ACCEPTANCES)
}
fn capabilities_in(
    identity: &super::SoftwareIdentity,
    records: &[ResidualAcceptance<'_>],
) -> Vec<ResidualCapability> {
    let family = match identity.source {
        super::SoftwareSource::Store if !identity.family.is_empty() => "store",
        super::SoftwareSource::Desktop => identity.family.as_str(),
        _ => "",
    };
    let rules = catalog();
    [
        ResidualKind::Cache,
        ResidualKind::Settings,
        ResidualKind::LocalData,
    ]
    .into_iter()
    .map(|kind| {
        let mut matches = rules
            .iter()
            .filter(|rule| rule.family == family && rule.kind == kind)
            .peekable();
        let supported = matches.peek().is_some();
        let has_evidence = matches.any(|rule| {
            records.iter().any(|record| {
                accepted_in(rule, &identity.family, record.application_version, records)
            })
        });
        ResidualCapability {
            kind,
            inspection: if supported {
                ResidualInspectionSupport::FixedLocations
            } else {
                ResidualInspectionSupport::Unsupported
            },
            cleanup: if !supported {
                ResidualCleanupSupport::Unsupported
            } else if has_evidence {
                ResidualCleanupSupport::RequiresVerification
            } else {
                ResidualCleanupSupport::NotValidated
            },
        }
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inventory_identity(
        family: &str,
        source: super::super::SoftwareSource,
    ) -> super::super::SoftwareIdentity {
        super::super::SoftwareIdentity {
            source,
            key: "inventory-only".into(),
            family: family.into(),
        }
    }
    #[test]
    fn inventory_capabilities_separate_cache_from_undeclared_settings_and_data() {
        use super::super::SoftwareSource;
        for family in super::super::RESIDUAL_FAMILIES {
            let capabilities = capabilities(&inventory_identity(family, SoftwareSource::Desktop));
            assert_eq!(capabilities.len(), 3);
            for capability in capabilities {
                let supported = capability.kind == ResidualKind::Cache || *family != "weixin";
                assert_eq!(
                    capability.inspection == ResidualInspectionSupport::FixedLocations,
                    supported,
                    "{family}"
                );
                assert_eq!(
                    capability.cleanup,
                    if *family == "firefox" {
                        ResidualCleanupSupport::RequiresVerification
                    } else if supported {
                        ResidualCleanupSupport::NotValidated
                    } else {
                        ResidualCleanupSupport::Unsupported
                    }
                );
            }
        }
        let store = capabilities(&inventory_identity(
            "Example.Package_publisher",
            SoftwareSource::Store,
        ));
        assert!(store.iter().all(
            |c| c.inspection == ResidualInspectionSupport::FixedLocations
                && c.cleanup == ResidualCleanupSupport::NotValidated
        ));
        for identity in [
            inventory_identity("", SoftwareSource::Desktop),
            inventory_identity("other", SoftwareSource::Desktop),
            inventory_identity("", SoftwareSource::Store),
        ] {
            assert!(
                capabilities(&identity)
                    .iter()
                    .all(|c| c.inspection == ResidualInspectionSupport::Unsupported
                        && c.cleanup == ResidualCleanupSupport::Unsupported)
            );
        }
    }
    #[test]
    fn compiled_firefox_evidence_only_opens_eight_independently_versioned_scopes() {
        let original =
            include_bytes!("../../../../docs/evidence/software/firefox-rules-20261001.json");
        let history = include_bytes!(
            "../../../../docs/evidence/software/firefox-history-rules-20261001.json"
        );
        assert_eq!(ACCEPTANCES.len(), 8);
        let mut ids = std::collections::HashSet::new();
        for (evidence, approvals, whole_run_passed) in [
            (original.as_slice(), &ACCEPTANCES[..6], false),
            (history.as_slice(), &ACCEPTANCES[6..], true),
        ] {
            let digest = format!("{:x}", Sha256::digest(evidence));
            let records: serde_json::Value = serde_json::from_slice(evidence).unwrap();
            assert_eq!(records["whole_run_passed"], whole_run_passed);
            let results = records["per_rule_results"].as_array().unwrap();
            assert_eq!(results.len(), approvals.len());
            for record in approvals {
                assert!(ids.insert(record.rule_id));
                assert_eq!(digest, record.evidence_sha256);
                let spec = rule(record.rule_id).unwrap();
                assert!(accepted(&spec, "firefox", "140.17.0.1158"));
                assert!(accepted(&spec, "firefox", "140.17.0.0"));
                assert!(accepted(&spec, "firefox", "141.0.0.0"));
                assert!(!accepted(&spec, "chrome", "140.17.0.1158"));
                assert!(results.iter().any(|result| {
                    result["rule_id"] == record.rule_id
                        && result["rule_version"] == record.rule_version
                        && result["scope_sha256"] == record.scope_sha256
                        && result["deleted_files"].as_u64().unwrap_or(0) > 0
                        && result["unselected_hashes_unchanged"] == true
                        && result["excluded_occupied_changed_hardlinked_replaced_files_refused"]
                            == true
                }));
            }
        }
        for spec in catalog().iter().filter(|spec| spec.family != "firefox") {
            assert!(!accepted(spec, &spec.family, "140.17.0.1158"));
        }
    }
    #[test]
    fn capability_evidence_is_category_and_package_specific_and_still_requires_verification() {
        use super::super::SoftwareSource;
        for (rule_id, family, source) in [
            ("uninstall.firefox.http", "firefox", SoftwareSource::Desktop),
            (
                "uninstall.store.local_cache",
                "Example.Package_publisher",
                SoftwareSource::Store,
            ),
        ] {
            let spec = rule(rule_id).unwrap();
            let scope = spec.scope_sha256();
            let record = ResidualAcceptance {
                rule_id,
                rule_version: spec.version,
                scope_sha256: &scope,
                application_identity: family,
                application_version: "1.2.3.4",
                evidence_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            };
            let identity = inventory_identity(family, source.clone());
            let described = capabilities_in(&identity, &[record]);
            for category in described {
                assert_eq!(
                    category.cleanup,
                    if category.kind == spec.kind {
                        ResidualCleanupSupport::RequiresVerification
                    } else {
                        ResidualCleanupSupport::NotValidated
                    }
                );
            }
            // Firefox has its own compiled evidence; the synthetic Store package does not.
            assert_eq!(accepted(&spec, family, "1.2.3.4"), family == "firefox");
            let other = inventory_identity("Another.Package_publisher", source);
            assert!(
                capabilities_in(&other, &[record])
                    .iter()
                    .all(|c| c.cleanup != ResidualCleanupSupport::RequiresVerification)
            );
            for invalid in [
                ResidualAcceptance {
                    rule_version: spec.version + 1,
                    ..record
                },
                ResidualAcceptance {
                    scope_sha256: "incorrect",
                    ..record
                },
                ResidualAcceptance {
                    evidence_sha256: "missing",
                    ..record
                },
                ResidualAcceptance {
                    application_version: "display-version",
                    ..record
                },
            ] {
                assert!(
                    capabilities_in(&identity, &[invalid])
                        .iter()
                        .all(|category| category.cleanup == ResidualCleanupSupport::NotValidated)
                );
            }
        }
    }
    #[test]
    fn every_declared_family_has_separate_residual_scopes_and_no_parent_catchall() {
        let rules = catalog();
        let mut ids = std::collections::HashSet::new();
        for family in super::super::RESIDUAL_FAMILIES
            .iter()
            .copied()
            .chain(["store"])
        {
            assert!(
                rules
                    .iter()
                    .any(|rule| rule.family == family && rule.kind == ResidualKind::Cache)
            );
        }
        for spec in &rules {
            assert!(ids.insert(&spec.id), "duplicate {}", spec.id);
            assert!(spec.id.starts_with("uninstall."));
            assert!(!spec.relative.is_empty());
            assert!(!spec.relative.split('/').any(|p| p == ".." || p.is_empty()));
            assert_eq!(
                accepted(spec, &spec.family, "1.2.3.4"),
                ACCEPTANCES.iter().any(|record| record.rule_id == spec.id)
            );
        }
        assert!(
            !rules
                .iter()
                .any(|rule| rule.family == "weixin" && rule.kind != ResidualKind::Cache)
        );
    }
    #[test]
    fn evidence_for_one_cache_allows_new_versions_but_not_other_scopes() {
        let http = rule("uninstall.firefox.http").unwrap();
        let hash = http.scope_sha256();
        // Test-only values are deliberately never added to ACCEPTANCES.
        let record = ResidualAcceptance {
            rule_id: "uninstall.firefox.http",
            rule_version: 1,
            scope_sha256: &hash,
            application_identity: "firefox",
            application_version: "140.17.0.0",
            evidence_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        };
        assert!(accepted_in(&http, "firefox", "140.17.0.0", &[record]));
        assert!(accepted(&http, "firefox", "140.17.0.0"));
        assert!(accepted_in(&http, "firefox", "150.0.0.1", &[record]));
        for other in [
            "uninstall.firefox.preferences",
            "uninstall.firefox.storage",
            "uninstall.chrome.http",
        ] {
            let other = rule(other).unwrap();
            assert!(!accepted_in(&other, &other.family, "140.17.0.0", &[record]));
        }
        assert!(!accepted_in(&http, "firefox", "141.0", &[record]));
        assert!(!accepted_in(
            &http,
            "another-family",
            "140.17.0.0",
            &[record]
        ));
        let mut changed = http.clone();
        changed.relative = String::new();
        assert!(!accepted_in(&changed, "firefox", "140.17.0.0", &[record]));
        for invalid in [
            ResidualAcceptance {
                rule_version: 2,
                ..record
            },
            ResidualAcceptance {
                scope_sha256: "",
                ..record
            },
            ResidualAcceptance {
                evidence_sha256: "",
                ..record
            },
        ] {
            assert!(!accepted_in(&http, "firefox", "140.17.0.0", &[invalid]));
        }
    }
    #[test]
    fn store_evidence_is_bound_to_one_package_family_and_one_data_scope() {
        let data = rule("uninstall.store.local_cache").unwrap();
        let hash = data.scope_sha256();
        let record = ResidualAcceptance {
            rule_id: "uninstall.store.local_cache",
            rule_version: data.version,
            scope_sha256: &hash,
            application_identity: "DedicatedFixture_publisher",
            application_version: "1.0.0.0",
            evidence_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        };
        assert!(accepted_in(
            &data,
            "DedicatedFixture_publisher",
            "1.0.0.0",
            &[record]
        ));
        assert!(!accepted_in(
            &data,
            "AnotherPackage_publisher",
            "1.0.0.0",
            &[record]
        ));
        assert!(!accepted_in(
            &rule("uninstall.store.local_data").unwrap(),
            "DedicatedFixture_publisher",
            "1.0.0.0",
            &[record]
        ));
    }
    #[test]
    fn store_local_cache_is_persistent_data_and_cannot_reuse_cache_classification_evidence() {
        let data = rule("uninstall.store.local_cache").unwrap();
        assert_eq!(data.kind, ResidualKind::LocalData);
        assert_eq!(data.kind.category(), "data");
        assert_eq!(data.version, 4);
        assert_eq!(data.relative, "LocalCache");
        assert_eq!(
            rule("uninstall.store.temporary").unwrap().kind,
            ResidualKind::Cache
        );
        let mut old = data.clone();
        old.kind = ResidualKind::Cache;
        old.version = 3;
        let old_scope = old.scope_sha256();
        assert_ne!(data.scope_sha256(), old_scope);
        let old_evidence = ResidualAcceptance {
            rule_id: "uninstall.store.local_cache",
            rule_version: 3,
            scope_sha256: &old_scope,
            application_identity: "DedicatedFixture_publisher",
            application_version: "1.0.0.0",
            evidence_sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        };
        assert!(!accepted_in(
            &data,
            "DedicatedFixture_publisher",
            "1.0.0.0",
            &[old_evidence]
        ));
        assert!(!accepted_in(
            &old,
            "DedicatedFixture_publisher",
            "1.0.0.0",
            &[old_evidence]
        ));
        assert!(!accepted(&data, "DedicatedFixture_publisher", "1.0.0.0"));
    }
    #[test]
    fn diagnostics_extension_boundaries_cannot_select_a_directory_or_arbitrary_data() {
        let logs = rule("uninstall.weixin.logs").unwrap();
        assert!(logs.accepts_entry(false, Some("XLOG")));
        assert!(!logs.accepts_entry(false, Some("db")));
        assert!(!logs.accepts_entry(true, None));
        let settings = rule("uninstall.firefox.preferences").unwrap();
        assert!(!settings.accepts_entry(true, None));
    }
}
