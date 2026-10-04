use dustaway_core::{Platform, ScanEvent, Scope};
use dustaway_windows::WindowsPlatform;
use std::sync::atomic::AtomicBool;
fn main() -> anyhow::Result<()> {
    let platform = WindowsPlatform::new(Scope::User)?;
    for rule in dustaway_core::rules::for_scope(Scope::User)? {
        let (mut files, mut skipped, mut bytes) = (0u64, 0u64, 0u64);
        let result = platform.scan(
            &rule,
            &[],
            &AtomicBool::new(false),
            &mut |event| match event {
                ScanEvent::File(file) => {
                    files += 1;
                    bytes += file.bytes;
                }
                ScanEvent::Skipped(_) | ScanEvent::Retained(_) | ScanEvent::RetainedFile { .. } => {
                    skipped += 1
                }
                ScanEvent::Observation(_)
                | ScanEvent::Notice(_)
                | ScanEvent::NoticeCode(_)
                | ScanEvent::Checked
                | ScanEvent::CoverageIssue(_)
                | ScanEvent::ApplicationObserved(_) => {}
            },
        );
        println!(
            "{}: candidates={files}, skipped={skipped}, bytes={bytes}, success={}, blocked={}",
            rule.id,
            result.is_ok(),
            platform.blocked(&rule)?.is_some()
        );
    }
    Ok(())
}
