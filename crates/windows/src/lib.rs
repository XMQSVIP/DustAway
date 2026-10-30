#[cfg(test)]
mod acceptance_snapshot;
#[cfg(test)]
mod acceptance_wait;
mod app_cache;
pub mod application_evidence;
mod application_layouts;
pub mod applications;
pub mod attachments;
mod developer;
mod discovered_cache;
pub mod fs;
mod historical_log_probe;
mod historical_logs;
pub mod ipc;
pub mod maintenance;
mod maintenance_tools;
pub mod platform;
mod profile_locations;
mod residue;
pub mod security;
pub mod software;
pub mod system_maintenance;
pub mod tool_discovery;
mod tool_process;
pub mod uv_cache;
mod weixin_cache;
mod windows_cleanup;
mod windows_signature;
pub use platform::WindowsPlatform;
pub mod analysis;
mod analysis_listing;
mod analysis_purpose;
mod manual;
pub use analysis::WindowsAnalysis;
pub(crate) fn rules_for_analysis() -> anyhow::Result<Vec<dustaway_core::RuleDescriptor>> {
    Ok(dustaway_core::rules::for_scope(dustaway_core::Scope::User)?
        .into_iter()
        .filter(|r| r.root != "firefox" && r.app_id.is_none())
        .collect())
}
