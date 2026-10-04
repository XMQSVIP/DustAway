//! Explicit test-computer acceptance; absent from release builds.
use super::*;
use crate::{
    WindowsPlatform, applications,
    platform::{delete_snapshot, known_folder},
    security,
};
use anyhow::{Context, ensure};
use dustaway_core::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf, sync::atomic::AtomicBool};
use windows::Win32::UI::Shell::{
    FOLDERID_Documents, FOLDERID_LocalAppData, FOLDERID_RoamingAppData,
};

fn environment() -> Result<PathBuf> {
    ensure!(
        std::env::var("COMPUTERNAME")? == "DESKTOP-24NJTT6",
        "Wrong host"
    );
    ensure!(!security::elevated()?, "Ordinary user required");
    let root = known_folder(&FOLDERID_LocalAppData)?.join("DustAwayAcceptance/20261003-weixin");
    ensure!(
        std::env::current_exe()?.parent() == Some(root.as_path()),
        "Wrong executable location"
    );
    ensure!(
        std::fs::read_to_string(root.join(".dustaway-test"))? == "DustAway.Weixin.20261003",
        "Wrong marker"
    );
    Ok(root)
}

fn protection(omitted: &[PathBuf]) -> Result<BTreeMap<String, String>> {
    let mut pending = vec![
        known_folder(&FOLDERID_RoamingAppData)?.join("Tencent/xwechat"),
        known_folder(&FOLDERID_Documents)?.join("xwechat_files"),
    ];
    let mut hashes = BTreeMap::new();
    while let Some(path) = pending.pop() {
        let path = extended(&path)?;
        if omitted.contains(&path) {
            continue;
        }
        let meta = std::fs::symlink_metadata(&path)?;
        let _pin = PinnedPath::observe(&path, meta.is_dir(), None)?;
        if meta.is_dir() {
            for entry in std::fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        } else {
            use std::io::Read;
            let mut file = std::fs::File::open(&path)?;
            let mut digest = Sha256::new();
            let mut chunk = [0u8; 65536];
            loop {
                let n = file.read(&mut chunk)?;
                if n == 0 {
                    break;
                }
                digest.update(&chunk[..n]);
            }
            hashes.insert(
                format!("{:x}", Sha256::digest(serde_json::to_vec(&native(&path))?)),
                format!("{:x}", digest.finalize()),
            );
        }
    }
    Ok(hashes)
}

#[test]
#[ignore = "authorized Windows 11 test computer; explicit cache-only acceptance"]
fn real_weixin_cache_acceptance() -> Result<()> {
    println!("stage: checking test environment");
    let root = environment()?;
    println!("stage: checking signed Weixin installation");
    let app = dustaway_core::applications::application("weixin").unwrap();
    let found = applications::identify(app)?;
    ensure!(found.version == "4.1.15.13", "Test installation changed");
    let platform = WindowsPlatform::new(Scope::User)?;
    let mode = std::env::var("DUSTAWAY_WEIXIN_TEST_MODE")?;
    ensure!(
        matches!(mode.as_str(), "running" | "delete" | "after"),
        "Unknown mode"
    );
    let mut output = vec![];
    for id in ["weixin.http", "weixin.code", "weixin.gpu"] {
        println!("stage: scanning {id}");
        let rule = rules::catalog()?
            .into_iter()
            .find(|r| r.id == id)
            .context("Missing rule")?;
        let mut files = vec![];
        let mut retained = 0;
        platform.scan(
            &rule,
            &[],
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::File(file) => files.push(file),
                ScanEvent::Retained(_) => retained += 1,
                _ => (),
            },
        )?;
        let blocked = applications::blocked_for_verified(&rule, &found)?;
        println!(
            "stage: {id}: {} candidates; blocked {:?}",
            files.len(),
            blocked.as_ref().map(|r| r.code)
        );
        if mode == "running" {
            ensure!(
                blocked
                    .as_ref()
                    .is_some_and(|r| r.code == ReasonCode::ApplicationRunning),
                "Running guard did not refuse"
            );
        }
        let bytes: u64 = files.iter().map(|f| f.bytes).sum();
        let mut deleted = 0;
        let mut unchanged = None;
        if mode == "delete" && matches!(id, "weixin.code" | "weixin.gpu") && !files.is_empty() {
            ensure!(blocked.is_none(), "Exit Weixin before acceptance");
            let omitted = files
                .iter()
                .map(|f| serde_json::from_str::<Snapshot>(&f.opaque).map(|s| from_native(&s.path)))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let before = protection(&omitted)?;
            for file in &files {
                let fresh = applications::identify(app)?;
                ensure!(
                    fresh.fingerprint()? == found.fingerprint()?,
                    "Application changed"
                );
                ensure!(
                    applications::blocked_for_verified(&rule, &fresh)?.is_none(),
                    "Application restarted"
                );
                platform.validate_candidate(&rule, file)?;
                let snapshot: Snapshot = serde_json::from_str(&file.opaque)?;
                // Same pinned-file deletion boundary as WindowsPlatform::delete.
                delete_snapshot(&snapshot, rule.min_age_days, &[])?;
                ensure!(!from_native(&snapshot.path).exists(), "File remains");
                deleted += 1;
            }
            let after = protection(&omitted)?;
            ensure!(before == after, "Protected Weixin files changed");
            unchanged = Some(before.len());
        }
        output.push(serde_json::json!({"rule":id,"files":files.len(),"bytes":bytes.to_string(),"retained":retained,"running_block":blocked.as_ref().map(|r|r.code),"deleted":deleted,"protected_unchanged":unchanged}));
    }
    std::fs::write(
        root.join(format!("{mode}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"mode":mode,"version":found.version,"rules":output}),
        )?,
    )?;
    Ok(())
}

#[test]
#[ignore = "authorized test computer; deletes only test-created received files"]
fn real_weixin_received_files() -> Result<()> {
    use crate::attachments::WindowsAttachments;
    let evidence_root = environment()?;
    let app = dustaway_core::applications::application("weixin").unwrap();
    let found = applications::identify(app)?;
    ensure!(
        security::application_processes(app.processes, &found.install)?.is_empty(),
        "Exit Weixin first"
    );
    let locations = WindowsAttachments.candidate_locations(AttachmentProvider::Weixin)?;
    ensure!(locations.len() == 1, "Exactly one test account required");
    let account = WindowsAttachments.account(AttachmentProvider::Weixin, &locations[0])?;
    let folder = Path::new(&locations[0]).join("msg/file/2026-10");
    let _folder = PinnedPath::observe(&folder, true, None)?;
    let stem = format!("dustaway-owned-{}", uuid::Uuid::new_v4());
    let mut owned = Vec::new();
    for (ext, bytes) in [
        ("txt", b"DustAway owned acceptance fixture.\n".to_vec()),
        (
            "pdf",
            b"%PDF-1.4\n1 0 obj << /Type /Catalog >> endobj\n%%EOF\n".to_vec(),
        ),
        (
            "docx",
            crate::app_cache::tests::zip_bytes(
                "word/document.xml",
                b"<document>DustAway fixture</document>",
            ),
        ),
    ] {
        use std::io::Write;
        let path = folder.join(format!("{stem}.{ext}"));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        owned.push(display_path(&extended(&path)?));
    }
    std::fs::write(
        evidence_root.join("received-owned.json"),
        serde_json::to_vec_pretty(&owned)?,
    )?;
    let mut records = vec![];
    WindowsAttachments.scan_attachments(&account, &[], &AtomicBool::new(false), &mut |item| {
        if let Ok(item) = item
            && owned.contains(&item.entry.path)
        {
            records.push(item);
        }
        Ok(())
    })?;
    ensure!(
        records.len() == owned.len(),
        "Owned files were not all recognized"
    );
    let protected = protection(
        &records
            .iter()
            .map(|r| extended(Path::new(&r.entry.path)))
            .collect::<Result<Vec<_>>>()?,
    )?;
    for item in &records {
        ensure!(item.entry.cleanup_block.is_none(), "Owned file was blocked");
        ensure!(
            WindowsAttachments
                .verify_attachment(&account, &item.record, &[display_path(&folder)])
                .is_err(),
            "Exclusion did not protect file"
        );
        WindowsAttachments.verify_attachment(&account, &item.record, &[])?;
        WindowsAttachments.delete_manual(&item.record, &[])?;
        ensure!(!Path::new(&item.entry.path).exists(), "Owned file remains");
    }
    ensure!(
        protected == protection(&[])?,
        "Unselected Weixin files changed"
    );
    std::fs::write(
        evidence_root.join("received-files.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"version":found.version,"test_created_only":true,"deleted":records.len(),"formats":["txt","pdf","docx"],"protected_unchanged":protected.len(),"exclusion_refused":true}),
        )?,
    )?;
    Ok(())
}
