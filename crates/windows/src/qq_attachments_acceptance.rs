//! Dedicated-machine, explicit ignored tests. Never compiled into application builds.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{System::WindowsProgramming::GetComputerNameW, UI::Shell::FOLDERID_LocalAppData},
    core::PWSTR,
};

const ACCOUNT: &str = r"C:\Users\MSN\Documents\Tencent Files\1776866992";
const TEST_ROOT: &str = r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261002-qq";
const VERSION: &str = "9.7.25.29417";
const SAMPLES: &[(&str, &str)] = &[
    (
        "DustAway-QQ-test-image.jpg",
        "7CEB11557EAF0558CC134155884857AEB9383E3D4BA8042E0584072D5D1FC2BA",
    ),
    (
        "DustAway-QQ-test-video.mp4",
        "F3D94BD078E0AF76D53B11654ADFF72CE2CCBFE5AB72C2A9305A21B07F3478F0",
    ),
    (
        "DustAway-QQ-test-file.zip",
        "4694CAD977F0A1053BC42F53CF338E13D32B0B26C3BBD32242AF5E88896587C8",
    ),
    (
        "DustAway-QQ-test-space.zip",
        "4694CAD977F0A1053BC42F53CF338E13D32B0B26C3BBD32242AF5E88896587C8",
    ),
];
static ENABLED: AtomicBool = AtomicBool::new(false);
struct Permit;
impl Drop for Permit {
    fn drop(&mut self) {
        ENABLED.store(false, Ordering::Release);
    }
}
pub(super) fn permitted(account: &QqAccount, path: Option<&Path>) -> bool {
    if !ENABLED.load(Ordering::Acquire)
        || account.provider != AttachmentProvider::QqClassic
        || account.version != VERSION
        || account.layout != "qq-classic-account-v1"
        || extended(Path::new(ACCOUNT)).ok().as_ref() != Some(&from_native(&account.path))
    {
        return false;
    }
    let Some(path) = path else { return true };
    SAMPLES.iter().any(|(name, hash)| {
        path == from_native(&account.path)
            .join("FileRecv/MobileFile")
            .join(name)
            && sample_hash(path).is_ok_and(|value| value == *hash)
    })
}
pub(super) fn sample_hash(path: &Path) -> Result<String> {
    // Scan/prepare/delete already hold the restrictive identity pin. A second
    // non-delete-sharing pin would conflict with our own DELETE handle.
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= 1024 * 1024,
        "Unexpected sample size"
    );
    let mut bytes = vec![];
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "Unexpected sample growth");
    Ok(format!("{:X}", Sha256::digest(bytes)))
}
#[test]
fn sample_hash_works_while_own_delete_handle_is_pinned() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("sample.zip");
    std::fs::write(&path, b"PK\x03\x04test")?;
    let expected = sample_hash(&path)?;
    let _pin = PinnedPath::open_attachment(&path, true)?;
    ensure!(sample_hash(&path)? == expected, "Pinned sample changed");
    Ok(())
}
fn environment(samples: &[(&str, &str)]) -> Result<(PathBuf, Permit)> {
    let mut name = [0u16; 256];
    let mut length = name.len() as u32;
    unsafe {
        GetComputerNameW(Some(PWSTR(name.as_mut_ptr())), &mut length)?;
    }
    ensure!(
        String::from_utf16(&name[..length as usize])? == "DESKTOP-24NJTT6",
        "Dedicated computer required"
    );
    ensure!(!security::elevated()?, "Ordinary interactive user required");
    let root = PathBuf::from(TEST_ROOT);
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "Wrong executable directory"
    );
    ensure!(
        known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20261002-qq") == root,
        "Wrong user"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-qq-classic-test-only"))?
            == "DustAway.QqClassic.20261003",
        "Wrong test marker"
    );
    let (account, _) = inspect(AttachmentProvider::QqClassic, Path::new(ACCOUNT))?;
    ensure!(account.version == VERSION, "Wrong classic QQ version");
    for (name, hash) in samples {
        let _pin = PinnedPath::observe(
            &Path::new(ACCOUNT).join("FileRecv/MobileFile").join(name),
            false,
            None,
        )?;
        ensure!(
            sample_hash(&Path::new(ACCOUNT).join("FileRecv/MobileFile").join(name))? == *hash,
            "Unexpected sample contents"
        );
    }
    ENABLED.store(true, Ordering::Release);
    Ok((root, Permit))
}
fn scanned(
    samples: &[(&str, &str)],
) -> Result<(Arc<AttachmentEngine>, String, Vec<AttachmentEntry>)> {
    let engine = AttachmentEngine::new(Arc::new(WindowsAttachments));
    let location = engine.register(AttachmentProvider::QqClassic, ACCOUNT)?;
    let session = engine.start(&location.token, vec![])?;
    let started = Instant::now();
    while engine.is_busy() {
        ensure!(started.elapsed() < Duration::from_secs(90), "Scan timeout");
        std::thread::sleep(Duration::from_millis(50));
    }
    let snapshot = engine.snapshot().context("Missing snapshot")?;
    ensure!(
        snapshot.state == "complete" && snapshot.issues == 0,
        "Incomplete scan"
    );
    let page = engine.page(AttachmentQuery {
        session: session.clone(),
        kind: None,
        min_age_days: 0,
        search: "DustAway-QQ-test-".into(),
        offset: 0,
        selected_ids: None,
    })?;
    let mut items = vec![];
    for (name, _) in samples {
        let path = Path::new(ACCOUNT).join("FileRecv/MobileFile").join(name);
        let item = page
            .entries
            .iter()
            .find(|e| Path::new(&e.path) == path)
            .context("Missing received sample")?;
        ensure!(item.cleanup_block.is_none(), "Sample was blocked");
        items.push(item.clone());
    }
    ensure!(
        page.entries
            .iter()
            .filter(|e| !items.iter().any(|i| i.id == e.id))
            .all(|e| e.cleanup_block.is_some()),
        "Nonselected sample gained permission"
    );
    Ok((engine, session, items))
}
#[test]
#[ignore = "authorized ordinary-user classic QQ account with received synthetic samples"]
fn classic_running_guard() -> Result<()> {
    let (root, _permit) = environment(&SAMPLES[..3])?;
    let (engine, session, items) = scanned(&SAMPLES[..3])?;
    let (_, found) = inspect(AttachmentProvider::QqClassic, Path::new(ACCOUNT))?;
    let running = security::application_processes(
        &process_names(AttachmentProvider::QqClassic)?,
        &found.install,
    )?;
    ensure!(running.iter().any(|p| p == "QQ.exe"), "QQ must be running");
    let selection = AttachmentSelection {
        session: session.clone(),
        selected_ids: items.iter().map(|e| e.id.clone()).collect(),
    };
    let error = match engine.prepare(&selection, &[], &AtomicBool::new(false)) {
        Err(e) => e,
        Ok(_) => anyhow::bail!("Running guard failed"),
    };
    ensure!(
        reason_for(&error) == ReasonCode::ApplicationRunning,
        "Unexpected refusal: {error}"
    );
    let previews = items
        .iter()
        .filter(|e| e.preview_available)
        .map(|e| {
            engine
                .thumbnail(&session, &e.id)
                .map(|s| (e.name.clone(), s.len()))
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(previews.len() == 2, "Image and video previews required");
    std::fs::write(
        root.join("classic-running-guard.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":VERSION,"running_components":running,"refused":"application_running","deletion_attempted":false,"previews":previews}),
        )?,
    )?;
    Ok(())
}
#[test]
#[ignore = "single-use deletion of three exact synthetic classic QQ received files; QQ must be exited"]
fn classic_delete_once() -> Result<()> {
    delete_samples(&SAMPLES[..3], "classic")
}
#[test]
#[ignore = "single-use deletion of one newly received synthetic ZIP; records actual disk space"]
fn classic_space_delete_once() -> Result<()> {
    delete_samples(&SAMPLES[3..], "classic-space")
}
fn delete_samples(samples: &[(&str, &str)], batch: &str) -> Result<()> {
    let (root, _permit) = environment(samples)?;
    ensure!(
        !root.join(format!("{batch}-delete.started")).exists(),
        "Never replay this deletion"
    );
    let (engine, session, items) = scanned(samples)?;
    let selected = AttachmentSelection {
        session,
        selected_ids: items.iter().map(|e| e.id.clone()).collect(),
    };
    let manifest = engine.prepare(&selected, &[], &AtomicBool::new(false))?;
    let paths: Vec<_> = items.iter().map(|e| e.path.clone()).collect();
    let protected_root = Path::new(ACCOUNT)
        .parent()
        .context("Missing account parent")?;
    let before = super::super::acceptance::protected(protected_root, &paths)?;
    ensure!(
        before.len() > 20,
        "Missing protected database and attachment samples"
    );
    for name in ["DustAway-QQ-test-audio.wav", "DustAway-QQ-test-readme.txt"] {
        ensure!(
            before
                .keys()
                .any(|key| Path::new(key)
                    == Path::new(ACCOUNT).join("FileRecv/MobileFile").join(name)),
            "Missing retained received file"
        );
    }
    std::fs::write(
        root.join(format!("{batch}-protected-before.json")),
        serde_json::to_vec_pretty(&before)?,
    )?;
    let cleanup = Engine::open(
        Arc::new(crate::WindowsPlatform::new(Scope::User)?),
        Scope::User,
        &root.join(format!("{batch}-attachment-test-history.sqlite")),
    )?;
    let plan = cleanup.attach_manual(manifest)?;
    let mut marker = std::fs::File::create_new(root.join(format!("{batch}-delete.started")))?;
    marker.write_all(plan.id.as_bytes())?;
    marker.sync_all()?;
    cleanup.execute(&plan.id)?;
    let started = Instant::now();
    while cleanup.is_busy() {
        ensure!(
            started.elapsed() < Duration::from_secs(90),
            "Cleanup timeout; do not replay"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let report = cleanup.report().context("Missing report")?;
    std::fs::write(
        root.join(format!("{batch}-delete-report.json")),
        serde_json::to_vec_pretty(&report)?,
    )?;
    ensure!(
        report.deleted == items.len() as u64
            && report.failed == 0
            && report.skipped == 0
            && report.unknown == 0,
        "Cleanup incomplete"
    );
    let expected_bytes = items
        .iter()
        .map(|e| e.bytes.parse::<u64>())
        .collect::<std::result::Result<Vec<_>, _>>()?
        .iter()
        .sum::<u64>()
        .to_string();
    ensure!(
        report.deleted_bytes == expected_bytes && report.candidate_bytes == expected_bytes,
        "Incorrect byte accounting"
    );
    ensure!(
        report
            .manual
            .as_ref()
            .is_some_and(|r| r.rule_id == "qq.attachments"),
        "Incorrect receipt source"
    );
    ensure!(
        report.volume_space_changes.len() == 1
            && report.volume_space_changes.iter().all(|v| v.error.is_none()
                && v.before_bytes.is_some()
                && v.after_bytes.is_some()
                && v.change_bytes.is_some()),
        "Missing disk space measurements"
    );
    ensure!(cleanup.execute(&plan.id).is_err(), "Plan replayed");
    ensure!(
        items.iter().all(|e| !Path::new(&e.path).exists()),
        "Selected file remains"
    );
    let after = super::super::acceptance::protected(protected_root, &paths)?;
    std::fs::write(
        root.join(format!("{batch}-protected-after.json")),
        serde_json::to_vec_pretty(&after)?,
    )?;
    ensure!(before == after, "Protected files changed");
    let result = serde_json::json!({"version":VERSION,"layout":"qq-classic-account-v1","relative_area":"FileRecv/MobileFile","formats":samples.iter().filter_map(|(n,_)| Path::new(n).extension().and_then(|v| v.to_str())).collect::<Vec<_>>(),"files":items,"report":report,"protected_files":before.len(),"protected_unchanged":true,"human_functional_verification":"pending"});
    std::fs::write(
        root.join(format!("{batch}-acceptance.json")),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}
