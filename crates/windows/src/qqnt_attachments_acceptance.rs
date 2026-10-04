//! Separate QQNT real-account evidence. No code in this module ships in the app.
use super::acceptance::sample_hash;
use super::*;
use std::{
    io::Write,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{System::WindowsProgramming::GetComputerNameW, UI::Shell::FOLDERID_LocalAppData},
    core::PWSTR,
};
const ACCOUNT: &str = r"C:\Users\MSN\Documents\Tencent Files\1730932123";
const TEST_ROOT: &str = r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261002-qq";
const VERSION: &str = "9.9.33.52230";
const AREA: &str = "nt_qq/nt_data/File";
const SAMPLES: &[(&str, &str)] = &[
    (
        "DustAway-QQNT-test-image.jpg",
        "7CEB11557EAF0558CC134155884857AEB9383E3D4BA8042E0584072D5D1FC2BA",
    ),
    (
        "DustAway-QQNT-test-video.mp4",
        "F3D94BD078E0AF76D53B11654ADFF72CE2CCBFE5AB72C2A9305A21B07F3478F0",
    ),
    (
        "DustAway-QQNT-test-file.zip",
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
        || account.provider != AttachmentProvider::Qqnt
        || account.version != VERSION
        || account.layout != "qqnt-account-v1"
        || extended(Path::new(ACCOUNT)).ok().as_ref() != Some(&from_native(&account.path))
    {
        return false;
    }
    let Some(path) = path else { return true };
    SAMPLES.iter().any(|(name, hash)| {
        path == from_native(&account.path).join(AREA).join(name)
            && sample_hash(path).is_ok_and(|value| value == *hash)
    })
}
fn environment() -> Result<(PathBuf, Permit)> {
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
        std::fs::read_to_string(root.join(".dustaway-qqnt-test-only"))? == "DustAway.Qqnt.20261003",
        "Wrong test marker"
    );
    let (account, _) = inspect(AttachmentProvider::Qqnt, Path::new(ACCOUNT))?;
    ensure!(account.version == VERSION, "Wrong QQNT version");
    for (name, hash) in SAMPLES {
        let path = Path::new(ACCOUNT).join(AREA).join(name);
        let _pin = PinnedPath::observe(&path, false, None)?;
        ensure!(
            sample_hash(&path)? == *hash,
            "Unexpected QQNT sample contents"
        );
    }
    ENABLED.store(true, Ordering::Release);
    Ok((root, Permit))
}
fn scanned() -> Result<(Arc<AttachmentEngine>, String, Vec<AttachmentEntry>)> {
    let engine = AttachmentEngine::new(Arc::new(WindowsAttachments));
    let location = engine.register(AttachmentProvider::Qqnt, ACCOUNT)?;
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
        search: "DustAway-QQNT-test-".into(),
        offset: 0,
        selected_ids: None,
    })?;
    let mut items = vec![];
    for (name, _) in SAMPLES {
        let path = Path::new(ACCOUNT).join(AREA).join(name);
        let item = page
            .entries
            .iter()
            .find(|e| Path::new(&e.path) == path)
            .context("Missing received QQNT sample")?;
        ensure!(item.cleanup_block.is_none(), "Sample was blocked");
        items.push(item.clone());
    }
    ensure!(
        page.entries
            .iter()
            .filter(|e| !items.iter().any(|i| i.id == e.id))
            .all(|e| e.cleanup_block.is_some()),
        "Unverified sample gained permission"
    );
    Ok((engine, session, items))
}
#[test]
#[ignore = "authorized ordinary-user QQNT account with received synthetic samples"]
fn qqnt_running_guard() -> Result<()> {
    let (root, _permit) = environment()?;
    let (engine, session, items) = scanned()?;
    let (_, found) = inspect(AttachmentProvider::Qqnt, Path::new(ACCOUNT))?;
    let running = running_processes(AttachmentProvider::Qqnt, &found.install);
    ensure!(
        match &running {
            Ok(names) => names.iter().any(|p| p == "QQ.exe"),
            Err(error) => reason_for(error) == ReasonCode::ProcessUnknown,
        },
        "QQNT must be running or its process state explicitly unverifiable"
    );
    let selection = AttachmentSelection {
        session: session.clone(),
        selected_ids: items.iter().map(|e| e.id.clone()).collect(),
    };
    let error = match engine.prepare(&selection, &[], &AtomicBool::new(false)) {
        Err(e) => e,
        Ok(_) => anyhow::bail!("Running guard failed"),
    };
    ensure!(
        matches!(
            reason_for(&error),
            ReasonCode::ApplicationRunning | ReasonCode::ProcessUnknown
        ),
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
        root.join("nt-running-guard.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":VERSION,"running_components":running.as_ref().ok(),"process_error":running.as_ref().err().map(|e| e.to_string()),"refused":reason_for(&error),"deletion_attempted":false,"previews":previews}),
        )?,
    )?;
    Ok(())
}
fn downloads() -> Result<Vec<(String, String)>> {
    SAMPLES
        .iter()
        .map(|(name, _)| {
            Ok((
                (*name).into(),
                sample_hash(&Path::new(r"C:\Users\MSN\Downloads").join(name))?,
            ))
        })
        .collect()
}
#[test]
#[ignore = "single-use deletion of three exact received QQNT files; QQNT must be exited"]
fn qqnt_delete_once() -> Result<()> {
    let (root, _permit) = environment()?;
    ensure!(
        !root.join("nt-delete.started").exists(),
        "Never replay this deletion"
    );
    let (engine, session, items) = scanned()?;
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
    let downloads_before = downloads()?;
    ensure!(before.len() > 100, "Missing protected samples");
    for name in [
        "DustAway-QQNT-test-audio.wav",
        "DustAway-QQNT-test-readme.txt",
    ] {
        ensure!(
            before
                .keys()
                .any(|key| Path::new(key) == Path::new(ACCOUNT).join(AREA).join(name)),
            "Missing retained file"
        );
    }
    std::fs::write(
        root.join("nt-protected-before.json"),
        serde_json::to_vec_pretty(&before)?,
    )?;
    let cleanup = Engine::open(
        Arc::new(crate::WindowsPlatform::new(Scope::User)?),
        Scope::User,
        &root.join("nt-attachment-test-history.sqlite"),
    )?;
    let plan = cleanup.attach_manual(manifest)?;
    let mut marker = std::fs::File::create_new(root.join("nt-delete.started"))?;
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
        root.join("nt-delete-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    ensure!(
        report.deleted == 3 && report.failed == 0 && report.skipped == 0 && report.unknown == 0,
        "Cleanup incomplete"
    );
    ensure!(
        report.deleted_bytes == "4375" && report.candidate_bytes == "4375",
        "Incorrect byte accounting"
    );
    ensure!(
        report
            .manual
            .as_ref()
            .is_some_and(|r| r.rule_id == "qqnt.attachments"),
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
        root.join("nt-protected-after.json"),
        serde_json::to_vec_pretty(&after)?,
    )?;
    ensure!(before == after, "Protected files changed");
    ensure!(
        downloads_before == downloads()?,
        "Original Downloads copies changed"
    );
    let result = serde_json::json!({"version":VERSION,"layout":"qqnt-account-v1","relative_area":AREA,"formats":["jpg","mp4","zip"],"files":items,"report":report,"protected_files":before.len(),"protected_unchanged":true,"downloads_copies_unchanged":true,"human_functional_verification":"pending"});
    std::fs::write(
        root.join("nt-acceptance.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}
