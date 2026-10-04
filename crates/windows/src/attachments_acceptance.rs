//! Explicit ignored tests on the authorized test computer only. Never in application builds.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::Arc,
    time::{Duration, Instant},
};
use windows::{
    Win32::{System::WindowsProgramming::GetComputerNameW, UI::Shell::FOLDERID_LocalAppData},
    core::PWSTR,
};
const ACCOUNT_PATH: &str = r"C:\Users\MSN\Documents\xwechat_files\wxid_d2d8w6wc7gdo22_59a6";
const TEST_PATH: &str = r"C:\Users\MSN\AppData\Local\DustAwayAcceptance\20261002-attachments";
fn environment() -> Result<PathBuf> {
    let mut name = [0u16; 256];
    let mut n = name.len() as u32;
    unsafe {
        GetComputerNameW(Some(PWSTR(name.as_mut_ptr())), &mut n)?;
    }
    ensure!(
        String::from_utf16(&name[..n as usize])? == "DESKTOP-24NJTT6",
        "Dedicated computer required"
    );
    ensure!(
        !security::elevated()?,
        "Ordinary interactive account required"
    );
    let root = PathBuf::from(TEST_PATH);
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "Wrong executable location"
    );
    ensure!(
        known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20261002-attachments")
            == root,
        "Wrong user"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-test-only"))?
            == "DustAway.Attachments.20261002",
        "Wrong test marker"
    );
    let (account, _) = account(Path::new(ACCOUNT_PATH))?;
    ensure!(account.version == "4.1.15.13", "Wrong Weixin version");
    Ok(root)
}
fn scanned() -> Result<(Arc<AttachmentEngine>, String, Vec<AttachmentEntry>)> {
    let engine = AttachmentEngine::new(Arc::new(WindowsAttachments));
    let location = engine.register(AttachmentProvider::Weixin, ACCOUNT_PATH)?;
    let session = engine.start(&location.token, vec![])?;
    let start = Instant::now();
    while engine.is_busy() {
        ensure!(start.elapsed() < Duration::from_secs(90), "scan timeout");
        std::thread::sleep(Duration::from_millis(50));
    }
    ensure!(
        engine.snapshot().unwrap().state == "complete",
        "scan incomplete"
    );
    let page = engine.page(AttachmentQuery {
        session: session.clone(),
        kind: None,
        min_age_days: 0,
        search: String::new(),
        offset: 0,
        selected_ids: None,
    })?;
    let mut selected = vec![];
    for kind in [
        AttachmentKind::Video,
        AttachmentKind::Image,
        AttachmentKind::File,
    ] {
        let item = page
            .entries
            .iter()
            .find(|e| e.kind == kind && e.cleanup_block.is_none())
            .context("missing actual sample")?;
        ensure!(
            item.bytes.parse::<u64>()? <= 16 * 1024 * 1024,
            "sample too large"
        );
        selected.push(item.clone());
    }
    Ok((engine, session, selected))
}
pub(super) fn protected(
    root: &Path,
    excluded: &[String],
) -> Result<BTreeMap<String, (u64, String)>> {
    let mut pending = vec![root.to_path_buf()];
    let mut output = BTreeMap::new();
    while let Some(dir) = pending.pop() {
        let _pin = PinnedPath::observe(&dir, true, None)?;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let ty = entry.file_type()?;
            ensure!(!ty.is_symlink(), "linked protection sample");
            if ty.is_dir() {
                pending.push(path);
                continue;
            }
            if excluded.contains(&display_path(&path)) {
                continue;
            }
            let pin = PinnedPath::observe(&path, false, None)?;
            let mut file = std::fs::File::open(&path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0u8; 65536];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
            output.insert(
                display_path(&path),
                (pin.stamp.size, format!("{:x}", hash.finalize())),
            );
        }
    }
    Ok(output)
}
#[test]
#[ignore = "authorized Windows 11 test computer; running Weixin required"]
fn real_running_guard() -> Result<()> {
    let root = environment()?;
    let (engine, session, items) = scanned()?;
    let selection = AttachmentSelection {
        session,
        selected_ids: items.iter().map(|e| e.id.clone()).collect(),
    };
    let error = match engine.prepare(&selection, &[], &AtomicBool::new(false)) {
        Err(e) => e,
        Ok(_) => anyhow::bail!("running guard did not refuse"),
    };
    ensure!(
        reason_for(&error) == ReasonCode::ApplicationRunning,
        "wrong refusal: {error}"
    );
    std::fs::write(
        root.join("running-guard.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":"4.1.15.13","refused":"application_running","deletion_attempted":false}),
        )?,
    )?;
    Ok(())
}
#[test]
#[ignore = "single-use real attachment deletion on authorized computer; Weixin must be exited"]
fn real_delete_once() -> Result<()> {
    let root = environment()?;
    ensure!(
        !root.join("delete.started").exists(),
        "Never replay this deletion"
    );
    let (engine, session, items) = scanned()?;
    let selected = AttachmentSelection {
        session: session.clone(),
        selected_ids: items.iter().map(|e| e.id.clone()).collect(),
    };
    let manifest = engine.prepare(&selected, &[], &AtomicBool::new(false))?;
    let paths: Vec<_> = items.iter().map(|e| e.path.clone()).collect();
    let before = protected(Path::new(ACCOUNT_PATH), &paths)?;
    ensure!(!before.is_empty(), "No protection samples");
    let previews: Vec<_> = items
        .iter()
        .filter(|e| e.preview_available)
        .map(|e| engine.thumbnail(&session, &e.id).map(|s| s.len()))
        .collect::<Result<_>>()?;
    let platform = Arc::new(crate::WindowsPlatform::new(Scope::User)?);
    let cleanup = Engine::open(
        platform,
        Scope::User,
        &root.join("attachment-test-history.sqlite"),
    )?;
    let plan = cleanup.attach_manual(manifest)?;
    let mut marker = std::fs::File::create_new(root.join("delete.started"))?;
    marker.write_all(plan.id.as_bytes())?;
    marker.sync_all()?;
    cleanup.execute(&plan.id)?;
    let start = Instant::now();
    while cleanup.is_busy() {
        ensure!(
            start.elapsed() < Duration::from_secs(90),
            "cleanup timeout; do not replay"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let report = cleanup.report().context("report missing")?;
    std::fs::write(
        root.join("delete-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    ensure!(
        report.deleted == items.len() as u64
            && report.failed == 0
            && report.skipped == 0
            && report.unknown == 0,
        "cleanup incomplete"
    );
    ensure!(cleanup.execute(&plan.id).is_err(), "replayed plan");
    ensure!(
        items.iter().all(|e| !Path::new(&e.path).exists()),
        "selected file remains"
    );
    let after = protected(Path::new(ACCOUNT_PATH), &paths)?;
    ensure!(before == after, "protected files changed");
    let result = serde_json::json!({"version":"4.1.15.13","files":items,"report":report,"protected_files":before.len(),"protected_unchanged":true,"previews":previews,"human_functional_verification":"pending"});
    std::fs::write(
        root.join("acceptance.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(())
}
