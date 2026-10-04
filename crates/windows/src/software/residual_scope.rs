//! Scope capture happens before uninstall. No new authority is inferred during residual scanning.
use super::*;
use dustaway_core::software::residual_rules::{self as rules, Locator, ResidualRule, Target};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
pub(super) struct CapturedScope {
    rule_id: String,
    scope_sha256: String,
    path: PathBuf,
    stamp: Stamp,
    parents: Vec<Identity>,
}

fn profiles(
    entry: &SoftwareEntry,
    rule: &ResidualRule,
    local: &Path,
    roaming: &Path,
    issue: &mut dyn FnMut(ReasonCode),
) -> Result<Vec<PathBuf>> {
    match &rule.locator {
        Locator::ChromiumProfile => {
            crate::profile_locations::chromium(local, &entry.identity.family, None, issue)
        }
        Locator::FirefoxRoamingProfile => crate::profile_locations::firefox(roaming, None, issue),
        Locator::FirefoxLocalProfile => crate::profile_locations::firefox(roaming, None, issue)
            .map(|profiles| {
                profiles
                    .into_iter()
                    .map(|profile| {
                        local
                            .join("Mozilla/Firefox/Profiles")
                            .join(profile.file_name().expect("verified profile"))
                    })
                    .collect()
            }),
        Locator::ApplicationProfile(id) => {
            let basis = dustaway_core::applications::rule(id).context("未知配置布局")?;
            ensure!(basis.app == entry.identity.family, "配置布局不属于该软件");
            let location_only = dustaway_core::applications::ApplicationRule {
                relative: "",
                ..*basis
            };
            let found =
                crate::application_layouts::inspect(local, roaming, &location_only, None, issue)?;
            if found.unavailable == Some(ReasonCode::UnsupportedLayout) {
                issue(ReasonCode::UnsupportedLayout);
            }
            Ok(found.roots)
        }
        Locator::WpsOfficeData | Locator::WpsAddins => {
            ensure!(
                entry.identity.source == SoftwareSource::Desktop && entry.identity.family == "wps",
                "WPS 数据位置不属于该软件"
            );
            Ok(vec![
                roaming.join(
                    rule.locator
                        .fixed_roaming_base()
                        .context("未知 WPS 数据位置")?,
                ),
            ])
        }
        Locator::StorePackage => {
            let family = &entry.identity.family;
            ensure!(
                entry.identity.source == SoftwareSource::Store
                    && !family.is_empty()
                    && family
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')),
                "无效应用包身份"
            );
            Ok(vec![local.join("Packages").join(family)])
        }
    }
}

pub(super) fn capture(
    entry: &SoftwareEntry,
    roots: &[Root],
    local: &Path,
    roaming: &Path,
) -> (Vec<CapturedScope>, Vec<String>) {
    let family = if entry.identity.source == SoftwareSource::Store {
        "store"
    } else {
        &entry.identity.family
    };
    let mut scopes = vec![];
    let mut warnings = vec![];
    let mut locations = HashMap::new();
    for spec in rules::catalog().into_iter().filter(|r| r.family == family) {
        let key = serde_json::to_string(&spec.locator).expect("fixed locator");
        let profiles = locations.entry(key).or_insert_with(|| {
            profiles(entry, &spec, local, roaming, &mut |_| {
                warnings.push("部分配置目录未能独立核验，相关残留仅保留".into());
            })
            .unwrap_or_else(|_| {
                warnings.push("部分配置目录读取失败，相关残留仅保留".into());
                vec![]
            })
        });
        for profile in profiles.iter() {
            let path = profile.join(&spec.relative);
            if !roots.iter().any(|root| path.starts_with(&root.path)) {
                continue;
            }
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(meta) => meta,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => {
                    warnings.push("部分规则位置无法读取，已保留".into());
                    continue;
                }
            };
            if meta.is_dir() != (spec.target == Target::Tree) {
                warnings.push("部分规则位置类型异常，已保留".into());
                continue;
            }
            match PinnedPath::open(&path, meta.is_dir(), false) {
                Ok(pin) => scopes.push(CapturedScope {
                    rule_id: spec.id.clone(),
                    scope_sha256: spec.scope_sha256(),
                    path,
                    stamp: pin.stamp,
                    parents: pin.parent_ids,
                }),
                Err(_) => warnings.push("部分规则位置被占用或包含链接，已保留".into()),
            }
        }
    }
    warnings.sort();
    warnings.dedup();
    (scopes, warnings)
}

fn matches(scope: &CapturedScope, spec: &ResidualRule, path: &Path, directory: bool) -> bool {
    (match spec.target {
        Target::File => path == scope.path,
        Target::Tree => path.starts_with(&scope.path),
    }) && spec.accepts_entry(directory, path.extension().and_then(|s| s.to_str()))
}

fn unchanged(scope: &CapturedScope, spec: &ResidualRule) -> Result<()> {
    let pin = PinnedPath::open(&scope.path, spec.target == Target::Tree, false)?;
    ensure!(
        pin.stamp.identity == scope.stamp.identity
            && pin.stamp.created == scope.stamp.created
            && pin.stamp.attributes == scope.stamp.attributes
            && pin.parent_ids == scope.parents,
        Refusal(ReasonCode::Changed)
    );
    Ok(())
}

pub(super) struct Classifier<'a> {
    scopes: Vec<(usize, &'a CapturedScope, ResidualRule)>,
}
impl<'a> Classifier<'a> {
    pub fn new(scopes: &'a [CapturedScope], warnings: &mut Vec<String>) -> Self {
        let catalog: HashMap<_, _> = rules::catalog()
            .into_iter()
            .map(|rule| (rule.id.clone(), rule))
            .collect();
        let scopes = scopes
            .iter()
            .enumerate()
            .filter_map(|(index, scope)| {
                let spec = catalog.get(&scope.rule_id)?;
                if scope.scope_sha256 != spec.scope_sha256() || unchanged(scope, spec).is_err() {
                    if scope.path.try_exists().unwrap_or(true) {
                        warnings.push("部分独立规则范围已变化或无法读取，相关项目已保留".into());
                    }
                    return None;
                }
                Some((index, scope, spec.clone()))
            })
            .collect();
        Self { scopes }
    }
    pub fn classify(&self, path: &Path, directory: bool) -> Option<(usize, &ResidualRule)> {
        let mut matched = self.scopes.iter().filter_map(|(index, scope, spec)| {
            matches(scope, spec, path, directory).then_some((*index, spec))
        });
        let first = matched.next()?;
        // Ambiguous ownership is retained, never resolved by name, prefix or iteration order.
        matched.next().is_none().then_some(first)
    }
}

pub(super) fn authorized(
    context: &ContextData,
    prepared: &PreparedSoftware,
    record: &ResidualRecord,
) -> Result<()> {
    authorized_with(context, prepared, record, rules::accepted)
}

// The production entry above always uses the compiled evidence table. Keeping the
// other checks in one body lets the opt-in native test exercise the exact guards
// while gathering evidence, without adding a runtime or IPC approval override.
fn authorized_with(
    context: &ContextData,
    prepared: &PreparedSoftware,
    record: &ResidualRecord,
    accepted: impl FnOnce(&ResidualRule, &str, &str) -> bool,
) -> Result<()> {
    let snapshot: ResidualStamp = serde_json::from_str(&record.opaque)?;
    let scope = snapshot
        .scope
        .and_then(|index| context.residual_scopes.get(index))
        .ok_or(Refusal(ReasonCode::Protected))?;
    let spec = rules::rule(&scope.rule_id).ok_or(Refusal(ReasonCode::Protected))?;
    ensure!(
        scope.scope_sha256 == spec.scope_sha256()
            && matches(scope, &spec, &snapshot.path, record.candidate.directory),
        Refusal(ReasonCode::Protected)
    );
    ensure!(
        (spec.family == "store" && prepared.entry.identity.source == SoftwareSource::Store)
            || (prepared.entry.identity.source == SoftwareSource::Desktop
                && spec.family == prepared.entry.identity.family),
        Refusal(ReasonCode::Protected)
    );
    ensure!(
        accepted(
            &spec,
            &prepared.entry.identity.family,
            context.verified_version.as_deref().unwrap_or("")
        ),
        Refusal(ReasonCode::ValidationPending)
    );
    // Recheck the narrow scope itself, in addition to the observation root and frozen file.
    unchanged(scope, &spec)
}

#[cfg(test)]
pub(super) fn authorize_native_evidence(
    context: &ContextData,
    prepared: &PreparedSoftware,
    record: &ResidualRecord,
    accepted: impl FnOnce(&ResidualRule, &str, &str) -> bool,
) -> Result<()> {
    authorized_with(context, prepared, record, accepted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dustaway_core::software::residual_rules::ResidualKind;

    fn entry(family: &str, source: SoftwareSource) -> SoftwareEntry {
        let mut entry = Registration {
            hive: "user".into(),
            view: KEY_WOW64_64KEY.0,
            key: "Software\\DustAway\\Acceptance\\ResidualScopesOnly".into(),
            name: "Scope fixture".into(),
            publisher: "fixture".into(),
            version: "1.0".into(),
            command: String::new(),
            location: None,
            size: None,
            date: None,
            msi: false,
            no_remove: true,
        }
        .entry();
        entry.identity.family = family.into();
        entry.identity.source = source.clone();
        entry.software.source = source;
        entry
    }
    fn write(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "synthetic metadata only").unwrap();
    }
    fn captured(entry: &SoftwareEntry, local: &Path, roaming: &Path) -> Vec<CapturedScope> {
        let (roots, warnings) = roots_at(entry, local, roaming).unwrap();
        assert!(warnings.is_empty());
        let (scopes, warnings) = capture(entry, &roots, local, roaming);
        assert!(warnings.is_empty(), "{warnings:?}");
        scopes
    }
    fn kind(classifier: &Classifier<'_>, path: &Path) -> Option<ResidualKind> {
        classifier
            .classify(path, path.is_dir())
            .map(|(_, spec)| spec.kind)
    }

    #[test]
    fn chromium_fixed_leaves_do_not_claim_nested_cache_names_unmarked_or_new_scopes() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let base = local.join("Google/Chrome/User Data");
        let profile = base.join("Default");
        for relative in [
            "Preferences",
            "Cache/Cache_Data/entry",
            "Local Storage/data",
            "saved/Cache/Cache_Data/keep",
        ] {
            write(&profile.join(relative));
        }
        write(&base.join("Profile 2/Cache/Cache_Data/unmarked"));
        write(&base.join("Custom/Preferences"));
        write(&base.join("Custom/Cache/Cache_Data/keep"));
        let scopes = captured(&entry("chrome", SoftwareSource::Desktop), &local, &roaming);
        write(&profile.join("GPUCache/new-scope-after-prepare"));
        let mut warnings = vec![];
        let classifier = Classifier::new(&scopes, &mut warnings);
        assert!(warnings.is_empty());
        assert_eq!(
            kind(&classifier, &profile.join("Cache/Cache_Data/entry")),
            Some(ResidualKind::Cache)
        );
        assert_eq!(
            kind(&classifier, &profile.join("Preferences")),
            Some(ResidualKind::Settings)
        );
        assert_eq!(
            kind(&classifier, &profile.join("Local Storage/data")),
            Some(ResidualKind::LocalData)
        );
        for path in [
            profile.clone(),
            profile.join("saved/Cache/Cache_Data/keep"),
            profile.join("GPUCache/new-scope-after-prepare"),
            base.join("Profile 2/Cache/Cache_Data/unmarked"),
            base.join("Custom/Cache/Cache_Data/keep"),
        ] {
            assert!(kind(&classifier, &path).is_none(), "{}", path.display());
        }
    }

    #[test]
    fn firefox_requires_registered_relative_profile_and_keeps_custom_and_unknown_content() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let base = roaming.join("Mozilla/Firefox");
        for name in ["standard", "unregistered"] {
            write(&base.join(format!("Profiles/{name}/prefs.js")));
            write(&local.join(format!("Mozilla/Firefox/Profiles/{name}/cache2/entry")));
        }
        write(&base.join("Profiles/standard/storage/data"));
        write(&base.join("Profiles/standard/downloads/Cache/keep"));
        std::fs::write(base.join("profiles.ini"),
            "[Profile0]\nIsRelative=1\nPath=Profiles/standard\n[Profile1]\nIsRelative=0\nPath=C:/custom\n[Profile2]\nIsRelative=1\nPath=Profiles/../outside\n").unwrap();
        let scopes = captured(&entry("firefox", SoftwareSource::Desktop), &local, &roaming);
        let classifier = Classifier::new(&scopes, &mut vec![]);
        assert_eq!(
            kind(&classifier, &base.join("Profiles/standard/prefs.js")),
            Some(ResidualKind::Settings)
        );
        assert_eq!(
            kind(&classifier, &base.join("Profiles/standard/storage/data")),
            Some(ResidualKind::LocalData)
        );
        assert_eq!(
            kind(
                &classifier,
                &local.join("Mozilla/Firefox/Profiles/standard/cache2/entry")
            ),
            Some(ResidualKind::Cache)
        );
        for path in [
            base.join("profiles.ini"),
            base.join("Profiles/unregistered/prefs.js"),
            base.join("Profiles/standard/downloads/Cache/keep"),
            local.join("Mozilla/Firefox/Profiles/unregistered/cache2/entry"),
        ] {
            assert!(kind(&classifier, &path).is_none());
        }
    }

    #[test]
    fn diagnostics_preserve_extensions_and_dingtalk_uses_local_numbered_profiles() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let logs = roaming.join("Tencent/xwechat/log");
        write(&logs.join("one.xlog"));
        write(&logs.join("private.db"));
        write(&roaming.join("Tencent/shared.xlog"));
        let scopes = captured(&entry("weixin", SoftwareSource::Desktop), &local, &roaming);
        let classifier = Classifier::new(&scopes, &mut vec![]);
        assert_eq!(
            kind(&classifier, &logs.join("one.xlog")),
            Some(ResidualKind::Cache)
        );
        assert!(kind(&classifier, &logs).is_none());
        assert!(kind(&classifier, &logs.join("private.db")).is_none());
        assert!(kind(&classifier, &roaming.join("Tencent/shared.xlog")).is_none());
        for base in [&local, &roaming] {
            write(&base.join("DingTalk_123/Default/Preferences"));
            write(&base.join("DingTalk_123/Default/Cache/Cache_Data/data"));
        }
        write(&local.join("DingTalk_other/Default/Preferences"));
        write(&local.join("DingTalk_other/Default/Cache/Cache_Data/data"));
        let scopes = captured(
            &entry("dingtalk", SoftwareSource::Desktop),
            &local,
            &roaming,
        );
        let classifier = Classifier::new(&scopes, &mut vec![]);
        assert_eq!(
            kind(
                &classifier,
                &local.join("DingTalk_123/Default/Cache/Cache_Data/data")
            ),
            Some(ResidualKind::Cache)
        );
        assert!(
            kind(
                &classifier,
                &roaming.join("DingTalk_123/Default/Cache/Cache_Data/data")
            )
            .is_none()
        );
        assert!(
            kind(
                &classifier,
                &local.join("DingTalk_other/Default/Cache/Cache_Data/data")
            )
            .is_none()
        );
    }

    #[test]
    fn wps_settings_and_backups_are_independent_of_logs_and_keep_shared_custom_and_addin_code() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let office = roaming.join("Kingsoft/office6");
        let addins = roaming.join("Kingsoft/wps/jsaddons");
        write(&office.join("backup/recovery.bak"));
        for name in [
            "publish.xml",
            "jsplugins.xml",
            "publish.xml.bak",
            "plugin/code.js",
            "nested/publish.xml",
        ] {
            write(&addins.join(name));
        }
        assert!(!office.join("log").exists());
        let scopes = captured(&entry("wps", SoftwareSource::Desktop), &local, &roaming);
        assert_eq!(scopes.len(), 3);
        let classifier = Classifier::new(&scopes, &mut vec![]);
        assert_eq!(
            kind(&classifier, &office.join("backup")),
            Some(ResidualKind::LocalData)
        );
        assert_eq!(
            kind(&classifier, &office.join("backup/recovery.bak")),
            Some(ResidualKind::LocalData)
        );
        for name in ["publish.xml", "jsplugins.xml"] {
            assert_eq!(
                kind(&classifier, &addins.join(name)),
                Some(ResidualKind::Settings)
            );
        }
        for path in [
            roaming.join("Kingsoft"),
            office.clone(),
            office.join("backup-other/data"),
            addins.clone(),
            addins.join("publish.xml.bak"),
            addins.join("plugin/code.js"),
            addins.join("nested/publish.xml"),
            temp.path().join("custom-backup/recovery.bak"),
            local.join("Kingsoft/office6/backup/data"),
        ] {
            assert!(kind(&classifier, &path).is_none(), "{}", path.display());
        }
        // Replacing just the settings file must not be mistaken for the captured original.
        std::fs::rename(
            addins.join("publish.xml"),
            addins.join("saved-original.xml"),
        )
        .unwrap();
        write(&addins.join("publish.xml"));
        let mut warnings = vec![];
        let classifier = Classifier::new(&scopes, &mut warnings);
        assert!(kind(&classifier, &addins.join("publish.xml")).is_none());
        assert!(!warnings.is_empty());
        assert_eq!(
            kind(&classifier, &addins.join("jsplugins.xml")),
            Some(ResidualKind::Settings)
        );
    }

    #[test]
    fn wps_data_discovery_rejects_other_owners_links_and_new_locations() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let office = roaming.join("Kingsoft/office6");
        let addins = roaming.join("Kingsoft/wps/jsaddons");
        write(&addins.join("publish.xml"));
        std::fs::hard_link(
            addins.join("publish.xml"),
            temp.path().join("linked-setting"),
        )
        .unwrap();
        let app = entry("wps", SoftwareSource::Desktop);
        let (roots, warnings) = roots_at(&app, &local, &roaming).unwrap();
        assert!(warnings.is_empty());
        let (scopes, warnings) = capture(&app, &roots, &local, &roaming);
        assert!(scopes.is_empty());
        assert!(!warnings.is_empty());
        write(&office.join("backup/new-after-prepare"));
        assert!(
            kind(
                &Classifier::new(&scopes, &mut vec![]),
                &office.join("backup/new-after-prepare")
            )
            .is_none()
        );
        let rule = rules::rule("uninstall.wps.backups").unwrap();
        for other in [
            entry("weixin", SoftwareSource::Desktop),
            entry("wps", SoftwareSource::Store),
        ] {
            assert!(profiles(&other, &rule, &local, &roaming, &mut |_| {}).is_err());
        }
    }

    #[test]
    fn store_scopes_are_disjoint_and_replacing_a_scope_revokes_its_classification() {
        let temp = tempfile::tempdir().unwrap();
        let local = temp.path().join("local");
        let roaming = temp.path().join("roaming");
        let package = local.join("Packages/DedicatedFixture_publisher");
        for leaf in ["LocalCache", "TempState", "Settings", "LocalState"] {
            write(&package.join(leaf).join("data"));
        }
        write(&local.join("Packages/OtherPackage_publisher/LocalCache/data"));
        let scopes = captured(
            &entry("DedicatedFixture_publisher", SoftwareSource::Store),
            &local,
            &roaming,
        );
        let classifier = Classifier::new(&scopes, &mut vec![]);
        assert_eq!(
            kind(&classifier, &package.join("LocalCache/data")),
            Some(ResidualKind::LocalData)
        );
        assert_eq!(
            kind(&classifier, &package.join("TempState/data")),
            Some(ResidualKind::Cache)
        );
        assert_eq!(
            kind(&classifier, &package.join("Settings/data")),
            Some(ResidualKind::Settings)
        );
        assert_eq!(
            kind(&classifier, &package.join("LocalState/data")),
            Some(ResidualKind::LocalData)
        );
        assert!(kind(&classifier, &package).is_none());
        assert!(
            kind(
                &classifier,
                &local.join("Packages/OtherPackage_publisher/LocalCache/data")
            )
            .is_none()
        );
        std::fs::rename(package.join("LocalCache"), package.join("saved-original")).unwrap();
        write(&package.join("LocalCache/data"));
        let mut warnings = vec![];
        let classifier = Classifier::new(&scopes, &mut warnings);
        assert!(kind(&classifier, &package.join("LocalCache/data")).is_none());
        assert!(!warnings.is_empty());
        assert_eq!(
            kind(&classifier, &package.join("Settings/data")),
            Some(ResidualKind::Settings)
        );
    }
}
