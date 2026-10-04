//! Package identity is not proof of exclusive data ownership. These read-only checks
//! exclude shared containers and app relationships before granting a residual scope.
use super::*;
use windows::{
    ApplicationModel::{FindRelatedPackagesOptions, Package, PackageRelationship},
    Management::Deployment::SharedPackageContainerManager,
};

const CONTRACT_VERSION: u32 = 2;
const LIMIT: u32 = 10_000;
pub(super) const RETAINED: &str =
    "商店应用的数据独占性未能核验或存在共享关系，残留位置已保留；标准卸载能力不受影响";

/// In-memory preparation context only. Never stored in recovery/history or accepted via IPC.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Proof {
    contract_version: u32,
    user_sid: String,
    full_name: String,
    family: String,
    #[serde(default)]
    install_path: Option<PathBuf>,
}

pub(super) fn valid_family(family: &str) -> bool {
    !family.is_empty()
        && family.len() <= 255
        && family
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// Includes definitions whose members are not currently installed. Do not derive
/// membership from the installed Main package list or treat an unavailable API as empty.
fn shared_families(cancel: Option<&AtomicBool>) -> Result<HashSet<String>> {
    store_processes::check_cancel(cancel)?;
    let containers = SharedPackageContainerManager::GetDefault()?.FindContainers()?;
    let count = containers.Size()?;
    ensure!(count <= LIMIT, "共享应用容器数量超过检查上限");
    let mut total = 0u32;
    let mut families = HashSet::new();
    for index in 0..count {
        store_processes::check_cancel(cancel)?;
        let members = containers.GetAt(index)?.GetMembers()?;
        let count = members.Size()?;
        total = total.checked_add(count).context("共享应用成员数量无效")?;
        ensure!(total <= LIMIT, "共享应用成员数量超过检查上限");
        for member in 0..count {
            store_processes::check_cancel(cancel)?;
            let family = members.GetAt(member)?.PackageFamilyName()?.to_string();
            ensure!(valid_family(&family), "共享应用包身份无效");
            families.insert(family.to_ascii_lowercase());
        }
    }
    Ok(families)
}

fn relationship_options() -> Result<FindRelatedPackagesOptions> {
    // Dependencies is not the supported API for an unpackaged query caller.
    // An older OS lacking FindRelatedPackages keeps residuals read-only.
    let options = FindRelatedPackagesOptions::CreateInstance(PackageRelationship::All)?;
    options.SetIncludeFrameworks(false)?;
    options.SetIncludeResources(false)?;
    options.SetIncludeOptionals(true)?;
    options.SetIncludeHostRuntimes(true)?;
    Ok(options)
}

fn related_families(
    package: &Package,
    options: &FindRelatedPackagesOptions,
    remaining: &mut u32,
    cancel: Option<&AtomicBool>,
) -> Result<HashSet<String>> {
    let related = package.FindRelatedPackages(options)?;
    let count = related.Size()?;
    *remaining = remaining
        .checked_sub(count)
        .context("关联应用包数量超过检查上限")?;
    let mut families = HashSet::new();
    for index in 0..count {
        store_processes::check_cancel(cancel)?;
        let family = related.GetAt(index)?.Id()?.FamilyName()?.to_string();
        ensure!(valid_family(&family), "关联应用包身份无效");
        families.insert(family.to_ascii_lowercase());
    }
    Ok(families)
}

struct Observation {
    registrations: Vec<String>,
    shared: bool,
    related: bool,
}
fn validate(observation: Observation, identity: &SoftwareIdentity, installed: bool) -> Result<()> {
    ensure!(
        identity.source == SoftwareSource::Store && valid_family(&identity.family),
        "无效商店应用身份"
    );
    ensure!(
        !observation.shared && !observation.related,
        "应用数据可能被其他包共用"
    );
    if installed {
        ensure!(
            observation.registrations == [identity.key.as_str()],
            "同包族的安装身份已变化"
        );
    } else {
        ensure!(observation.registrations.is_empty(), "应用包已重新安装");
    }
    Ok(())
}

fn observe(
    identity: &SoftwareIdentity,
    installed: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Observation> {
    store_processes::check_cancel(cancel)?;
    ensure!(
        identity.source == SoftwareSource::Store && valid_family(&identity.family),
        "无效商店应用身份"
    );
    let shared = shared_families(cancel)?.contains(&identity.family.to_ascii_lowercase());
    let manager = PackageManager::new()?;
    let iterator = manager
        .FindPackagesByUserSecurityIdPackageFamilyName(
            &HSTRING::new(),
            &HSTRING::from(&identity.family),
        )?
        .First()?;
    let mut registrations = vec![];
    while iterator.HasCurrent()? {
        store_processes::check_cancel(cancel)?;
        registrations.push(iterator.Current()?.Id()?.FullName()?.to_string());
        ensure!(
            registrations.len() <= LIMIT as usize,
            "应用包数量超过检查上限"
        );
        iterator.MoveNext()?;
    }
    // Fail before querying a vanished package or needlessly reading unrelated metadata.
    validate(
        Observation {
            registrations: registrations.clone(),
            shared,
            related: false,
        },
        identity,
        installed,
    )?;
    let options = relationship_options()?;
    let mut remaining = LIMIT;
    let related = if installed {
        let package = manager.FindPackageByUserSecurityIdPackageFullName(
            &HSTRING::new(),
            &HSTRING::from(&identity.key),
        )?;
        ensure!(
            package.Id()?.FamilyName()? == identity.family
                && package.Id()?.FullName()? == identity.key
                && !package.IsFramework()?
                && !package.IsResourcePackage()?
                && !package.IsBundle()?
                && !package.IsOptional()?,
            "不是已核验的当前用户主应用包"
        );
        !related_families(&package, &options, &mut remaining, cancel)?.is_empty()
    } else {
        // The removed package can no longer be queried. Check both Main and Optional
        // packages for a remaining/new relationship; a partial query never proves absence.
        let iterator = manager
            .FindPackagesByUserSecurityIdWithPackageTypes(
                &HSTRING::new(),
                PackageTypes::Main | PackageTypes::Optional,
            )?
            .First()?;
        let mut count = 0;
        let mut found = false;
        while iterator.HasCurrent()? {
            store_processes::check_cancel(cancel)?;
            count += 1;
            ensure!(count <= LIMIT, "应用包数量超过检查上限");
            if related_families(&iterator.Current()?, &options, &mut remaining, cancel)?
                .contains(&identity.family.to_ascii_lowercase())
            {
                found = true;
                break;
            }
            iterator.MoveNext()?;
        }
        found
    };
    Ok(Observation {
        registrations,
        shared,
        related,
    })
}

pub(super) fn capture(identity: &SoftwareIdentity) -> Result<Proof> {
    let _com = Com::new()?;
    validate(observe(identity, true, None)?, identity, true)?;
    let package = PackageManager::new()?.FindPackageByUserSecurityIdPackageFullName(
        &HSTRING::new(),
        &HSTRING::from(&identity.key),
    )?;
    let install = extended(Path::new(&package.InstalledPath()?.to_string()))?;
    ensure!(install.components().count() > 2, "应用安装位置无法核验");
    Ok(Proof {
        contract_version: CONTRACT_VERSION,
        user_sid: security::current_sid()?,
        full_name: identity.key.clone(),
        family: identity.family.clone(),
        install_path: Some(install),
    })
}

fn bound(proof: &Proof, identity: &SoftwareIdentity, user_sid: &str) -> Result<()> {
    ensure!(
        proof.contract_version == CONTRACT_VERSION
            && identity.source == SoftwareSource::Store
            && proof.full_name == identity.key
            && proof.family == identity.family
            && valid_family(&proof.family)
            && !proof.user_sid.is_empty()
            && proof.user_sid == user_sid,
        "应用数据归属上下文已失效"
    );
    ensure!(
        proof
            .install_path
            .as_ref()
            .is_some_and(|path| extended(path).is_ok_and(|path| path.components().count() > 2)),
        "应用进程位置上下文已失效"
    );
    Ok(())
}

pub(super) fn recheck(
    proof: Option<&Proof>,
    identity: &SoftwareIdentity,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    store_processes::check_cancel(cancel)?;
    let proof = proof.context("缺少卸载前的应用数据独占性核验")?;
    bound(proof, identity, &security::current_sid()?)?;
    let _com = Com::new()?;
    validate(observe(identity, false, cancel)?, identity, false)?;
    store_processes::ensure_stopped(
        &proof.family,
        proof.install_path.as_deref().context("缺少应用进程位置")?,
        &proof.user_sid,
        cancel,
    )
}

pub(super) fn warning(error: &anyhow::Error, cancel: &AtomicBool) -> String {
    if cancel.load(Ordering::Acquire) {
        return "检查已停止，结果不完整".into();
    }
    if let Some(Refusal(code @ (ReasonCode::ApplicationRunning | ReasonCode::ProcessUnknown))) =
        error.downcast_ref::<Refusal>()
    {
        return format!("{}；商店应用残留已保留", code.message());
    }
    RETAINED.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> SoftwareIdentity {
        SoftwareIdentity {
            source: SoftwareSource::Store,
            key: "DustAway.Test_1.0.0.0_x64__publisher".into(),
            family: "DustAway.Test_publisher".into(),
        }
    }
    fn observation(names: &[&str], shared: bool, related: bool) -> Observation {
        Observation {
            registrations: names.iter().map(|s| (*s).into()).collect(),
            shared,
            related,
        }
    }
    #[test]
    fn uninstall_scope_rejects_shared_optional_and_changed_package_ownership() {
        let identity = identity();
        assert!(validate(observation(&[&identity.key], false, false), &identity, true).is_ok());
        for facts in [
            observation(&[&identity.key], true, false),
            observation(&[&identity.key], false, true),
            observation(&[], false, false),
            observation(&["DustAway.Test_2.0.0.0_x64__publisher"], false, false),
            observation(&[&identity.key, &identity.key], false, false),
        ] {
            assert!(validate(facts, &identity, true).is_err());
        }
        // Uninstalled members in a container and new optional relationships still retain data.
        for facts in [
            observation(&[], true, false),
            observation(&[], false, true),
            observation(&["DustAway.Test_2.0.0.0_x64__publisher"], false, false),
        ] {
            assert!(validate(facts, &identity, false).is_err());
        }
        assert!(validate(observation(&[], false, false), &identity, false).is_ok());
    }
    #[test]
    fn isolation_context_is_bound_to_user_package_and_contract() {
        let identity = identity();
        let mut proof = Proof {
            contract_version: CONTRACT_VERSION,
            user_sid: "user-a".into(),
            full_name: identity.key.clone(),
            family: identity.family.clone(),
            install_path: Some(PathBuf::from("C:/WindowsApps/Test")),
        };
        assert!(bound(&proof, &identity, "user-a").is_ok());
        assert!(bound(&proof, &identity, "user-b").is_err());
        proof.contract_version += 1;
        assert!(bound(&proof, &identity, "user-a").is_err());
        proof.contract_version = CONTRACT_VERSION;
        proof.full_name.push_str("-changed");
        assert!(bound(&proof, &identity, "user-a").is_err());
        proof.full_name = identity.key.clone();
        proof.family.push_str("-neighbor");
        assert!(bound(&proof, &identity, "user-a").is_err());
        assert!(recheck(None, &identity, None).is_err());
        assert!(!valid_family("../another-package"));
    }

    #[test]
    fn legacy_process_context_and_scan_warnings_do_not_imply_empty_residuals() -> Result<()> {
        let identity = identity();
        let proof: Proof = serde_json::from_value(serde_json::json!({
            "contract_version":CONTRACT_VERSION,"user_sid":"user-a",
            "full_name":identity.key,"family":identity.family,
        }))?;
        assert!(bound(&proof, &identity, "user-a").is_err());
        let cancel = AtomicBool::new(false);
        for code in [ReasonCode::ApplicationRunning, ReasonCode::ProcessUnknown] {
            let message = warning(&anyhow::anyhow!(Refusal(code)), &cancel);
            assert!(message.contains(code.message()) && message.contains("保留"));
        }
        assert_eq!(
            warning(&anyhow::anyhow!("partial package query"), &cancel),
            RETAINED
        );
        cancel.store(true, Ordering::Release);
        assert_eq!(
            warning(&anyhow::anyhow!("partial package query"), &cancel),
            "检查已停止，结果不完整"
        );
        assert_eq!(
            observe(&identity, false, Some(&cancel))
                .err()
                .unwrap()
                .to_string(),
            "检查已停止，结果不完整"
        );
        Ok(())
    }

    #[test]
    fn legacy_store_context_does_not_scan_frozen_paths_without_isolation_proof() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("LocalState");
        std::fs::create_dir(&path)?;
        std::fs::write(path.join("must-be-retained"), "fixture")?;
        let root_pin = PinnedPath::open(&path, true, false)?;
        let identity = identity();
        let mut entry = Registration {
            hive: "user".into(),
            view: KEY_WOW64_64KEY.0,
            key: "unused".into(),
            name: "Isolated scope regression".into(),
            publisher: "fixture".into(),
            version: "1.0.0.0".into(),
            command: String::new(),
            location: None,
            size: None,
            date: None,
            msi: false,
            no_remove: true,
        }
        .entry();
        entry.identity = identity;
        entry.software.source = SoftwareSource::Store;
        entry.software.residual_supported = true;
        // Deliberately old JSON has no new field; it must deserialize but confer no authority.
        let context = serde_json::json!({
            "rule_version": RESIDUAL_RULE_VERSION, "file":null, "args":"", "stamp":null,
            "roots":[{"path":path,"stamp":root_pin.stamp,"parents":root_pin.parent_ids}],
            "processes":[], "warnings":[], "residual_scopes":[], "verified_version":"1.0.0.0",
        })
        .to_string();
        drop(root_pin);
        let prepared = PreparedSoftware {
            entry,
            fingerprint: "test-only".into(),
            context,
        };
        let result = WindowsSoftware.scan(&prepared, &[], &AtomicBool::new(false))?;
        assert!(result.supported);
        assert!(result.records.is_empty());
        assert_eq!(result.warnings, [RETAINED]);
        assert_eq!(
            std::fs::read_to_string(path.join("must-be-retained"))?,
            "fixture"
        );
        Ok(())
    }

    #[test]
    #[ignore = "read-only ordinary-token Store API probe on the dedicated acceptance PC"]
    fn real_store_isolation_queries() -> Result<()> {
        ensure!(
            std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
            "wrong host"
        );
        ensure!(!security::elevated()?, "ordinary token required");
        let root =
            known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20260930-software");
        ensure!(
            std::env::current_exe()?.parent() == Some(root.as_path()),
            "wrong executable scope"
        );
        ensure!(
            std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
                == "DustAway.Acceptance.20260930.7c7e25e8",
            "wrong marker"
        );
        let output = root.join("store-isolation-query-20261001.json");
        ensure!(
            !output.try_exists()?,
            "query evidence already exists; do not overwrite"
        );
        let _com = Com::new()?;
        let shared = shared_families(None);
        let shared_result = match &shared {
            Ok(families) => serde_json::json!({"ok":true,"member_count":families.len()}),
            Err(error) => serde_json::json!({"ok":false,"error":error.to_string()}),
        };
        let inventory = store_inventory()?;
        ensure!(inventory.warnings.is_empty(), "partial Store inventory");
        let before: HashSet<_> = inventory
            .entries
            .iter()
            .map(|e| e.identity.key.clone())
            .collect();
        let mut observations = vec![];
        let mut tested_fallback = false;
        for entry in inventory
            .entries
            .iter()
            .filter(|e| e.software.can_uninstall)
        {
            let proof = capture(&entry.identity);
            let isolated = proof.is_ok();
            let preparation = if !isolated && !tested_fallback {
                let prepared = WindowsSoftware.prepare(entry)?;
                let context: ContextData = serde_json::from_str(&prepared.context)?;
                ensure!(
                    context.store_isolation.is_none()
                        && context.roots.is_empty()
                        && context.residual_scopes.is_empty()
                        && context.warnings.iter().any(|warning| warning == RETAINED),
                    "failed isolation query did not retain all scopes"
                );
                ensure!(
                    prepared.entry.software.can_uninstall,
                    "isolation blocks standard uninstall"
                );
                tested_fallback = true;
                true
            } else {
                false
            };
            observations.push(serde_json::json!({
                "package_identity_sha256":format!("{:x}", Sha256::digest(entry.identity.key.as_bytes())),
                "isolated":isolated, "error":proof.err().map(|e|e.to_string()),
                "standard_prepare_retains_scopes":preparation,
            }));
        }
        let after = store_inventory()?;
        ensure!(after.warnings.is_empty(), "partial verification inventory");
        ensure!(
            before
                == after
                    .entries
                    .iter()
                    .map(|e| e.identity.key.clone())
                    .collect(),
            "package identities changed during read-only probe"
        );
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version":1, "contract_version":CONTRACT_VERSION,
                "ordinary_user":true, "shared_containers":shared_result,
                "current_user_main_count":before.len(), "observations":observations,
                "fallback_verified":tested_fallback, "inventory_unchanged":true,
                "real_uninstall":false, "real_file_deletion":false,
                "production_approval_added":false,
            }))?,
        )?;
        Ok(())
    }

    #[test]
    #[ignore = "requires the dedicated empty shared-container fixture; queries only"]
    fn real_store_uninstalled_shared_members_are_retained() -> Result<()> {
        ensure!(
            std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
            "wrong host"
        );
        ensure!(!security::elevated()?, "ordinary token required");
        let root =
            known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20260930-software");
        ensure!(
            std::env::current_exe()?.parent() == Some(root.as_path()),
            "wrong executable scope"
        );
        ensure!(
            std::fs::read_to_string(root.join(".dustaway-isolated-acceptance"))?
                == "DustAway.Acceptance.20260930.7c7e25e8",
            "wrong marker"
        );
        let phase = std::env::var("DUSTAWAY_STORE_SHARED_PHASE")?;
        ensure!(
            matches!(phase.as_str(), "shared" | "restored"),
            "wrong phase"
        );
        let output = root.join(format!("store-shared-{phase}-20261001.json"));
        ensure!(!output.try_exists()?, "evidence exists; no replay");
        let _com = Com::new()?;
        let families = shared_families(None)?;
        let identity = SoftwareIdentity {
            source: SoftwareSource::Store,
            key: "DustAway.Acceptance.7c7e25e8_1.0.0.0_x64__768b01p0s2m14".into(),
            family: "DustAway.Acceptance.7c7e25e8_768b01p0s2m14".into(),
        };
        let peer = SoftwareIdentity {
            source: SoftwareSource::Store,
            key: "DustAway.Acceptance.SharedPeer.7c7e25e8_1.0.0.0_x64__768b01p0s2m14".into(),
            family: "DustAway.Acceptance.SharedPeer.7c7e25e8_768b01p0s2m14".into(),
        };
        for identity in [&identity, &peer] {
            ensure!(
                !WindowsSoftware.installed(identity)?,
                "test member must remain uninstalled"
            );
            let present = families.contains(&identity.family.to_ascii_lowercase());
            ensure!(
                present == (phase == "shared"),
                "unexpected fixture membership"
            );
            let result =
                observe(identity, false, None).and_then(|facts| validate(facts, identity, false));
            if phase == "shared" {
                ensure!(
                    result.is_err_and(|e| e.to_string() == "应用数据可能被其他包共用"),
                    "shared member not refused by production guard"
                );
            } else {
                result?;
            }
        }
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&serde_json::json!({
                "phase":phase, "ordinary_user":true, "uninstalled_members":2,
                "shared_members_visible":phase == "shared", "guard_refused":phase == "shared",
                "post_removal_queries_ok":phase == "restored", "result":"passed",
                "read_only_query":true, "real_uninstall":false, "real_file_deletion":false,
                "production_approval_added":false,
            }))?,
        )?;
        Ok(())
    }
}
