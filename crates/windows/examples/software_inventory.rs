//! Read-only discovery smoke test. Never prepares or executes removal.
use dustaway_core::software::{SoftwarePlatform, SoftwareSource};
fn main() -> anyhow::Result<()> {
    let list = dustaway_windows::software::WindowsSoftware.inventory()?;
    let desktop = list
        .entries
        .iter()
        .filter(|e| e.software.source == SoftwareSource::Desktop)
        .count();
    println!(
        "desktop={desktop}, store={}, warnings={:?}",
        list.entries.len() - desktop,
        list.warnings
    );
    let icons = list
        .entries
        .iter()
        .filter(|entry| entry.software.icon_data_url.is_some())
        .count();
    println!("icons={icons}/{}", list.entries.len());
    Ok(())
}
