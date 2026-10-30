//! Read-only production discovery probe. Usage: cargo run ... -- D:\
use dustaway_core::{Platform, ScanEvent, Scope};
use dustaway_windows::{WindowsPlatform, platform::volume_target};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool, time::Instant};

fn main() -> anyhow::Result<()> {
    let drive = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("请指定磁盘根目录，例如 D:\\"))?;
    let volume = volume_target(Path::new(&drive))?;
    anyhow::ensure!(
        volume.label.eq_ignore_ascii_case(&drive),
        "请指定完整磁盘根目录"
    );
    let platform = WindowsPlatform::new(Scope::User)?;
    let mut results = vec![];
    for spec in dustaway_core::app_cache::catalog()
        .iter()
        .filter(|s| s.folder == "discovered")
    {
        let started = Instant::now();
        let rule = spec.descriptor();
        let mut roots = BTreeMap::<String, (u64, u64)>::new();
        let mut issues = vec![];
        let mut retained = 0u64;
        let mut retained_bytes = 0u64;
        let result = platform.scan_on_volumes(
            &rule,
            &[],
            Some(std::slice::from_ref(&volume)),
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::File(file) => {
                    let entry = roots
                        .entry(platform.candidate_root(&file).unwrap_or_default())
                        .or_default();
                    entry.0 += 1;
                    entry.1 += file.bytes;
                }
                ScanEvent::CoverageIssue(code) => issues.push(format!("{code:?}")),
                ScanEvent::RetainedFile { bytes, .. } => {
                    retained += 1;
                    retained_bytes += bytes;
                }
                ScanEvent::Retained(_) => retained += 1,
                _ => {}
            },
        );
        results.push(serde_json::json!({"rule":spec.id,"roots":roots,"elapsed_ms":started.elapsed().as_millis(),"success":result.is_ok(),"error":result.err().map(|e|e.to_string()),"issues":issues,"retained":retained,"retained_bytes":retained_bytes,"cleanable":platform.cleanup_validated(&rule)}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"volume":volume.label,"results":results})
        )?
    );
    Ok(())
}
