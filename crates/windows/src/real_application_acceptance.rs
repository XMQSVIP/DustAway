//! Ignored acceptance tests only. This module and its permit are absent from application builds.
//! A permit keeps the real signed executable, real process checks, roots and file deletion guards.
use super::*;
use anyhow::Context;
use dustaway_core::{Engine, JobState, Selection};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{Win32::System::WindowsProgramming::GetComputerNameW, core::PWSTR};

const ROOT: &str = r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261001-home-cleanup";
const MARKER: &str = "DustAway.HomeCleanup.20261001";
const RULES: &[&str] = &[
    "dingtalk.http",
    "dingtalk.code",
    "baidu_netdisk.http",
    "baidu_netdisk.code",
    "baidu_netdisk.gpu",
];

fn test_root() -> Result<PathBuf> {
    let mut name = [0u16; 256];
    let mut size = name.len() as u32;
    unsafe {
        GetComputerNameW(Some(PWSTR(name.as_mut_ptr())), &mut size)?;
    }
    ensure!(
        String::from_utf16(&name[..size as usize])? == "DESKTOP-24NJTT6",
        "dedicated computer required"
    );
    ensure!(!security::elevated()?, "ordinary interactive user required");
    let root = PathBuf::from(ROOT);
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "test executable must be in dedicated directory"
    );
    ensure!(
        known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20261001-home-cleanup")
            == root,
        "wrong Windows user"
    );
    let _root = PinnedPath::open(&root, true, false)?;
    let _marker = PinnedPath::open(&root.join(".dustaway-test-only"), false, false)?;
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-test-only"))? == MARKER,
        "test marker missing"
    );
    Ok(root)
}

fn descriptor(id: &str) -> Result<RuleDescriptor> {
    ensure!(RULES.contains(&id), "outside acceptance scope");
    dustaway_core::rules::catalog()?
        .into_iter()
        .find(|r| r.id == id)
        .context("unknown rule")
}

// Records are pinned to locally verified, signed binaries. Adding one is not release approval.
fn expected_binary(app: &str) -> Result<(&'static str, &'static str)> {
    match app {
        "baidu_netdisk" => Ok((
            "8.8.8.101",
            "ce1a67e04777524e4020ee4fe22004567e03695c41afc4f2d00b7345ce98b065",
        )),
        "dingtalk" => Ok((
            "8.2.0.119",
            "5705de940c8bf048792af1e99b55c19f914fc1ca89b6ddb7ab3c4ac0595f69f4",
        )),
        _ => anyhow::bail!("installed binary has not yet been verified for this acceptance run"),
    }
}

pub(super) struct RealAcceptance {
    rule: RuleDescriptor,
    root_identity: Identity,
    executable_identity: String,
    roots: Vec<(PathBuf, Identity)>,
}

impl RealAcceptance {
    fn new(platform: &WindowsPlatform, rule: RuleDescriptor) -> Result<Self> {
        let root = test_root()?;
        let app = dustaway_core::applications::application(
            rule.app_id.as_deref().context("application required")?,
        )
        .context("unknown application")?;
        let (version, hash) = expected_binary(app.id)?;
        let found = crate::applications::identify(app)?;
        ensure!(found.version == version, "unverified version");
        ensure!(
            format!("{:x}", Sha256::digest(std::fs::read(&found.pin.path)?)) == hash,
            "unverified binary"
        );
        let started = PinnedPath::open(&root.join(".dustaway-test-only"), false, false)?
            .stamp
            .created;
        let (product, installer_hash) = match app.id {
            "baidu_netdisk" => (
                "BaiduNetdisk",
                "1A388C5D0A8B90D04BD2FB83652F8DECC19310E020A74A3587F955A90259922B",
            ),
            "dingtalk" => (
                "DingTalk",
                "C4664B74F7B688FCBE30B951632EB6B0ABB820703908835695B5F82F8431CF56",
            ),
            _ => anyhow::bail!("profile admission not yet verified"),
        };
        let preflight: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(root.join(format!("{product}.install-preflight.json")))?
                .trim_start_matches('\u{feff}'),
        )?;
        ensure!(
            preflight["existing_data_absent"] == true
                && preflight["ordinary_user"] == true
                && preflight["installer_sha256"] == installer_hash,
            "installation preflight missing"
        );
        let roots = platform
            .roots(&rule)?
            .into_iter()
            .map(|path| {
                let identity = PinnedPath::open(&path, true, false)?.stamp.identity;
                Ok((path, identity))
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(!roots.is_empty(), "no verified cache root");
        let spec =
            dustaway_core::applications::rule(&rule.id).context("unknown application rule")?;
        for (cache, _) in &roots {
            let mut profile = cache.clone();
            for _ in Path::new(spec.relative).components() {
                ensure!(profile.pop());
            }
            ensure!(
                PinnedPath::open(&profile, true, false)?.stamp.created >= started,
                "profile predates dedicated test run"
            );
        }
        let permit = Self {
            rule,
            root_identity: PinnedPath::open(&root, true, false)?.stamp.identity,
            executable_identity: found.fingerprint()?,
            roots,
        };
        permit.require(platform, &permit.rule)?;
        Ok(permit)
    }

    pub(super) fn require(
        &self,
        platform: &WindowsPlatform,
        rule: &RuleDescriptor,
    ) -> Result<crate::applications::VerifiedApplication> {
        let root = test_root()?;
        ensure!(
            platform.scope == Scope::User
                && platform.app_test_identity.is_none()
                && platform.app_test_acceptance.is_none(),
            "real and synthetic scopes cannot mix"
        );
        ensure!(
            platform.local == extended(&known_folder(&FOLDERID_LocalAppData)?)?
                && platform.roaming == extended(&known_folder(&FOLDERID_RoamingAppData)?)?,
            "redirected profile refused"
        );
        ensure!(
            PinnedPath::open(&root, true, false)?.stamp.identity == self.root_identity,
            Refusal(ReasonCode::Changed)
        );
        ensure!(
            serde_json::to_value(rule)? == serde_json::to_value(&self.rule)?
                && RULES.contains(&rule.id.as_str()),
            Refusal(ReasonCode::ValidationPending)
        );
        dustaway_core::rules::validate(rule)?;
        let app = dustaway_core::applications::application(
            rule.app_id.as_deref().context("missing application")?,
        )
        .context("unknown application")?;
        let found = crate::applications::identify(app)?;
        ensure!(
            found.version == expected_binary(app.id)?.0
                && found.fingerprint()? == self.executable_identity,
            Refusal(ReasonCode::Changed)
        );
        let current = platform.roots(rule)?;
        ensure!(
            current.len() == self.roots.len(),
            Refusal(ReasonCode::Changed)
        );
        for (path, identity) in &self.roots {
            ensure!(
                current.contains(path)
                    && PinnedPath::open(path, true, false)?.stamp.identity == *identity,
                Refusal(ReasonCode::Changed)
            );
        }
        Ok(found)
    }
}

fn wait(engine: &Engine) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(180);
    while engine.is_busy() {
        if Instant::now() > deadline {
            engine.cancel();
            anyhow::bail!("acceptance operation timed out; stop requested");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn write_receipt(root: &Path, action: &str, value: &serde_json::Value) -> Result<()> {
    let path = root.join(format!("real-{action}-{}.json", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}

type Protection = BTreeMap<String, (Stamp, String)>;

fn protect(roots: &[PathBuf], omitted: &[PathBuf]) -> Result<Protection> {
    let mut result = BTreeMap::new();
    let mut pending = roots.to_vec();
    while let Some(path) = pending.pop() {
        let path = extended(&path)?;
        if omitted.iter().any(|root| path.starts_with(root)) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)?;
        let pin = PinnedPath::open(&path, metadata.is_dir(), false)?;
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        } else {
            let mut input = std::fs::File::open(&path)?;
            let mut digest = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                digest.update(&buffer[..count]);
            }
            ensure!(
                PinnedPath::open(&path, false, false)?.stamp == pin.stamp,
                "protection sample changed while hashing"
            );
            // No account path or contents enter the evidence output.
            let key = format!("{:x}", Sha256::digest(serde_json::to_vec(&native(&path))?));
            result.insert(key, (pin.stamp, format!("{:x}", digest.finalize())));
        }
    }
    Ok(result)
}

fn protection_roots(
    platform: &WindowsPlatform,
    rule: &RuleDescriptor,
    install: &Path,
) -> Result<Vec<PathBuf>> {
    let mut paths = vec![install.to_owned()];
    match rule.app_id.as_deref() {
        Some("baidu_netdisk") => {
            paths.push(platform.roaming.join("baidunetdisk"));
            // Explicitly identified by the user in this conversation; never enumerate other downloads.
            paths.push(PathBuf::from(
                r"D:\BaiduNetdiskDownload\amazon_twister_parser.py",
            ));
            paths.push(PathBuf::from(
                r"D:\BaiduNetdiskDownload\py_project260907.zip.baiduyun.p.downloading",
            ));
        }
        Some("dingtalk") => {
            paths.push(platform.roaming.join("DingTalk"));
            paths.push(platform.local.join("DingTalk"));
            let spec = dustaway_core::applications::rule(&rule.id).context("unknown rule")?;
            for mut path in platform.roots(rule)? {
                for _ in Path::new(spec.relative).components() {
                    ensure!(path.pop());
                }
                ensure!(path.pop()); // Entire numbered profile, not only Default.
                paths.push(path);
            }
        }
        _ => anyhow::bail!("unverified protection scope"),
    }
    Ok(paths)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct OrdinaryProtection {
    time: i64,
    phase: String,
    rule_id: String,
    version: String,
    application_sha256: String,
    protected_file_count: usize,
    protection_sha256: String,
    deletion_attempted: bool,
}

fn ordinary_protection_matches(before: &OrdinaryProtection, after: &OrdinaryProtection) -> bool {
    before.phase == "before"
        && after.phase == "after"
        && before.rule_id == "dingtalk.http"
        && after.rule_id == before.rule_id
        && before.version == "8.2.0.119"
        && after.version == before.version
        && after.time >= before.time
        && before.protected_file_count > 0
        && before.protected_file_count == after.protected_file_count
        && before.application_sha256.len() == 64
        && before.application_sha256 == after.application_sha256
        && before.protection_sha256.len() == 64
        && before.protection_sha256 == after.protection_sha256
        && !before.deletion_attempted
        && !after.deletion_attempted
}

#[test]
#[ignore = "dedicated computer only; read-only protection checkpoint for ordinary homepage acceptance"]
fn ordinary_home_protection() -> Result<()> {
    let root = test_root()?;
    let phase = std::env::var("DUSTAWAY_HOME_PROTECTION_PHASE")?;
    ensure!(
        matches!(phase.as_str(), "before" | "after"),
        "unknown phase"
    );
    let rule = descriptor("dingtalk.http")?;
    let platform = WindowsPlatform::new(Scope::User)?;
    // Verify the same dedicated installation, but never attach a test deletion
    // permit to the platform and never create an Engine or a cleanup plan.
    let admission = RealAcceptance::new(&platform, rule.clone())?;
    let found = admission.require(&platform, &rule)?;
    ensure!(
        platform.blocked(&rule)?.is_none(),
        "application must be closed"
    );
    let before_path = root.join("ordinary-http-protection-before.json");
    let after_path = root.join("ordinary-http-protection-after.json");
    ensure!(
        !after_path.exists(),
        "completed checkpoint cannot be repeated"
    );
    ensure!(
        phase != "before" || !before_path.exists(),
        "baseline already recorded"
    );
    let before = if phase == "after" {
        Some(serde_json::from_slice::<OrdinaryProtection>(
            &std::fs::read(&before_path)?,
        )?)
    } else {
        None
    };
    let protected = protect(
        &protection_roots(&platform, &rule, &found.install)?,
        &platform.roots(&rule)?,
    )?;
    ensure!(!protected.is_empty(), "no protected samples");
    admission.require(&platform, &rule)?;
    ensure!(
        platform.blocked(&rule)?.is_none(),
        "application reopened while hashing"
    );
    let checkpoint = OrdinaryProtection {
        time: now(),
        phase: phase.clone(),
        rule_id: rule.id,
        version: found.version.clone(),
        application_sha256: format!("{:x}", Sha256::digest(found.fingerprint()?.as_bytes())),
        protected_file_count: protected.len(),
        protection_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(&protected)?)),
        deletion_attempted: false,
    };
    let unchanged = before
        .as_ref()
        .map(|b| ordinary_protection_matches(b, &checkpoint));
    let mut evidence = serde_json::to_value(&checkpoint)?;
    evidence["protection_unchanged"] = serde_json::json!(unchanged);
    evidence["functional_check"] = serde_json::json!("not_run");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(if phase == "before" {
            before_path
        } else {
            after_path
        })?;
    file.write_all(&serde_json::to_vec_pretty(&evidence)?)?;
    file.sync_all()?;
    ensure!(
        unchanged != Some(false),
        "protection mismatch; preserve failed receipt"
    );
    Ok(())
}

#[test]
fn ordinary_checkpoints_require_same_identity_files_and_order() -> Result<()> {
    let make = |phase: &str| OrdinaryProtection {
        time: 10,
        phase: phase.to_owned(),
        rule_id: "dingtalk.http".to_owned(),
        version: "8.2.0.119".to_owned(),
        application_sha256: "a".repeat(64),
        protected_file_count: 4,
        protection_sha256: "b".repeat(64),
        deletion_attempted: false,
    };
    let before = make("before");
    assert!(ordinary_protection_matches(&before, &make("after")));
    for field in [
        "phase",
        "rule_id",
        "version",
        "application_sha256",
        "protection_sha256",
    ] {
        let mut changed = serde_json::to_value(make("after"))?;
        changed[field] = serde_json::json!("different");
        assert!(!ordinary_protection_matches(
            &before,
            &serde_json::from_value(changed)?
        ));
    }
    for (field, value) in [
        ("time", serde_json::json!(9)),
        ("protected_file_count", serde_json::json!(0)),
        ("protected_file_count", serde_json::json!(5)),
        ("deletion_attempted", serde_json::json!(true)),
    ] {
        let mut changed = serde_json::to_value(make("after"))?;
        changed[field] = value;
        assert!(!ordinary_protection_matches(
            &before,
            &serde_json::from_value(changed)?
        ));
    }
    Ok(())
}

#[test]
#[ignore = "dedicated computer only; one real cache cleanup, requires application to be fully closed"]
fn real_application_cleanup() -> Result<()> {
    let root = test_root()?;
    let rule = descriptor(&std::env::var("DUSTAWAY_HOME_RULE")?)?;
    let mut platform = WindowsPlatform::new(Scope::User)?;
    let permit = Arc::new(RealAcceptance::new(&platform, rule.clone())?);
    let found = permit.require(&platform, &rule)?;
    platform.real_app_acceptance = Some(permit);
    ensure!(
        platform.blocked(&rule)?.is_none(),
        "save work and exit the application, including background components"
    );
    ensure!(!root.join("stop-real-cleanup").exists(), "stop requested");
    let protected_roots = protection_roots(&platform, &rule, &found.install)?;
    let omitted = platform.roots(&rule)?;
    let before = protect(&protected_roots, &omitted)?;
    ensure!(!before.is_empty(), "no protected samples");
    let platform = Arc::new(platform);
    let engine = Engine::open(
        platform.clone(),
        Scope::User,
        &root.join(format!("clean-{}.sqlite", uuid::Uuid::new_v4())),
    )?;
    let session = engine.start_scan(vec![rule.id.clone()])?;
    wait(&engine)?;
    let summary = engine.summary();
    ensure!(
        summary.rules.len() == 1
            && summary.rules[0].files > 0
            && !summary.rules[0].scan_failed
            && summary.rules[0]
                .coverage
                .as_ref()
                .is_some_and(|coverage| coverage.complete),
        "complete nonzero real scan required"
    );
    let plan = engine.prepare(
        &session,
        Selection {
            rule_ids: vec![rule.id.clone()],
            reviewed_rule_ids: vec![rule.id.clone()],
            ..Default::default()
        },
    )?;
    ensure!(
        plan.files > 0 && protect(&protected_roots, &omitted)? == before,
        "protection samples changed before execution"
    );
    ensure!(
        platform.blocked(&rule)?.is_none() && !root.join("stop-real-cleanup").exists(),
        "application restarted or stop requested"
    );
    let mut once = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join(format!("real-clean-{}.started", rule.id)))?;
    once.write_all(&serde_json::to_vec(
        &serde_json::json!({"time":now(),"rule_id":rule.id,"plan":plan,"automatic_replay":false}),
    )?)?;
    once.sync_all()?;
    let execution = (|| -> Result<()> {
        engine.execute(&plan.id)?;
        let deadline = Instant::now() + Duration::from_secs(300);
        while engine.is_busy() {
            if root.join("stop-real-cleanup").exists() || Instant::now() > deadline {
                engine.cancel();
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Ok(())
    })();
    let after = protect(&protected_roots, &omitted);
    let unchanged = after.as_ref().is_ok_and(|value| value == &before);
    let report = engine.report();
    write_receipt(
        &root,
        "cleanup",
        &serde_json::json!({
            "time":now(), "rule_id":rule.id, "version":found.version,
            "scan":summary, "plan":plan, "report":report,
            "execution_returned_ok":execution.is_ok(), "protected_file_count":before.len(),
            "protection_unchanged":unchanged, "protection_before_sha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&before)?)),
            "protection_after_sha256":after.as_ref().ok().map(|value|format!("{:x}",Sha256::digest(serde_json::to_vec(value).unwrap()))),
            "relaunch_functional_check":"not_run", "cache_regeneration":"not_run", "release_approved":false
        }),
    )?;
    execution?;
    ensure!(
        unchanged,
        "protected samples differ; preserve evidence and do not approve release"
    );
    let report = report.context("missing cleanup result")?;
    ensure!(
        report.deleted == plan.files && report.failed + report.skipped + report.unknown == 0,
        "partial cleanup is not successful acceptance"
    );
    ensure!(engine.execute(&plan.id).is_err(), "plan must be single-use");
    Ok(())
}

#[test]
fn protection_detects_changed_and_added_files_and_refuses_links() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let file = temp.path().join("test-document");
    std::fs::write(&file, b"before")?;
    let roots = vec![temp.path().to_owned()];
    let first = protect(&roots, &[])?;
    std::fs::write(&file, b"after!")?;
    assert_ne!(protect(&roots, &[])?, first);
    let second = protect(&roots, &[])?;
    std::fs::write(temp.path().join("new-document"), b"added")?;
    assert_ne!(protect(&roots, &[])?, second);
    std::fs::hard_link(&file, temp.path().join("linked-document"))?;
    assert!(protect(&roots, &[]).is_err());
    Ok(())
}

#[test]
#[ignore = "dedicated real-application computer; metadata scan only"]
fn real_application_scan() -> Result<()> {
    let root = test_root()?;
    let platform = Arc::new(WindowsPlatform::new(Scope::User)?);
    let engine = Engine::open(
        platform,
        Scope::User,
        &root.join(format!("scan-{}.sqlite", uuid::Uuid::new_v4())),
    )?;
    let start = Instant::now();
    engine.start_scan(RULES.iter().map(|s| (*s).to_owned()).collect())?;
    wait(&engine)?;
    ensure!(engine.status().state == JobState::Ready);
    write_receipt(
        &root,
        "scan",
        &serde_json::json!({"time":now(), "scan_ms":start.elapsed().as_millis(), "summary":engine.summary(), "report":engine.report(), "deletion_attempted":false, "production_permissions":true}),
    )
}

#[test]
#[ignore = "dedicated real-application computer; verifies running guard without deleting"]
fn real_application_running_guard() -> Result<()> {
    let root = test_root()?;
    let rule = descriptor(&std::env::var("DUSTAWAY_HOME_RULE")?)?;
    let mut platform = WindowsPlatform::new(Scope::User)?;
    platform.real_app_acceptance = Some(Arc::new(RealAcceptance::new(&platform, rule.clone())?));
    let engine = Engine::open(
        Arc::new(platform),
        Scope::User,
        &root.join(format!("blocked-{}.sqlite", uuid::Uuid::new_v4())),
    )?;
    let session = engine.start_scan(vec![rule.id.clone()])?;
    wait(&engine)?;
    let summary = engine.summary();
    let row = summary.rules.first().context("missing rule result")?;
    ensure!(
        row.files > 0 && row.availability == Some(ReasonCode::ApplicationRunning),
        "nonzero running-app candidate evidence required"
    );
    ensure!(
        engine
            .prepare(
                &session,
                Selection {
                    rule_ids: vec![rule.id.clone()],
                    reviewed_rule_ids: vec![rule.id.clone()],
                    ..Default::default()
                }
            )
            .is_err(),
        "running application must prevent preparing deletion"
    );
    ensure!(engine.report().is_none());
    write_receipt(
        &root,
        "running-guard",
        &serde_json::json!({"time":now(), "rule_id":rule.id, "summary":summary, "deletion_attempted":false, "running_guard_passed":true, "release_approved":false}),
    )
}

#[test]
fn real_permit_scope_does_not_accept_sibling_or_unverified_product() {
    assert!(descriptor("weixin.logs").is_err());
    assert!(descriptor("windows.user_temp").is_err());
    assert!(expected_binary("qq").is_err());
    assert!(expected_binary("weixin").is_err());
    for id in RULES {
        assert_eq!(
            dustaway_core::cleanup_acceptance::release_enabled(&descriptor(id).unwrap()),
            matches!(*id, "dingtalk.http" | "dingtalk.code")
        );
    }
}
