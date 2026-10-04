mod maintenance_commands;
mod windows_maintenance_commands;
use maintenance_commands::*;
use windows_maintenance_commands::*;
mod attachment_commands;
mod commands;
use attachment_commands::*;
mod uv_commands;
use uv_commands::*;
mod software_commands;
use software_commands::*;
mod control;
#[cfg(all(debug_assertions, feature = "e2e-fixture"))]
mod fixture;
#[cfg(all(debug_assertions, feature = "e2e-fixture"))]
mod fixture_attachments;
use commands::*;
use dustaway_core::*;
use dustaway_windows::{
    ipc::{HelperClient, Request},
    platform, security,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{Emitter, Manager};

pub(crate) struct Service {
    pub windows_maintenance: Arc<dustaway_windows::system_maintenance::WindowsMaintenance>,
    pub tool_plans: Mutex<std::collections::HashMap<String, (String, String, RuleSummary)>>,
    pub maintenance: Arc<dustaway_windows::maintenance::Maintenance>,
    pub uv: Arc<dustaway_windows::uv_cache::UvCache>,
    pub software: Arc<software::SoftwareEngine>,
    pub analysis: Arc<AnalysisEngine>,
    pub attachments: Arc<AttachmentEngine>,
    pub user: Arc<Engine>,
    pub helper: Mutex<Option<HelperClient>>,
    pub control: Mutex<control::ControlState>,
    pub batch: Mutex<Vec<Scope>>,
    pub batch_cancel: AtomicBool,
    pub close_requested: AtomicBool,
    pub batch_error: Mutex<Option<String>>,
}
impl Service {
    pub fn system<T: serde::de::DeserializeOwned>(&self, request: Request) -> anyhow::Result<T> {
        require_scope(Scope::System)?;
        use anyhow::Context;
        let helper = self.helper.lock().unwrap();
        helper
            .as_ref()
            .context("系统清理组件未连接，请重新扫描并授权")?
            .call(request)
    }
    pub fn launch_system(&self) -> anyhow::Result<()> {
        require_scope(Scope::System)?;
        use anyhow::Context;
        let mut helper = self.helper.lock().unwrap();
        if let Some(current) = helper.as_ref()
            && current.call::<JobStatus>(Request::Status).is_ok()
        {
            return Ok(());
        }
        *helper = None;
        let path = std::env::current_exe()?
            .parent()
            .context("无效程序目录")?
            .join("dustaway-helper.exe");
        let client = HelperClient::launch(&path, env!("DUSTAWAY_HELPER_SHA256"))?;
        client.call::<serde_json::Value>(Request::Settings {
            settings: self.user.settings(),
        })?;
        for report in client.call::<Vec<CleanupReport>>(Request::History)? {
            self.user.record_external_report(&report)?;
        }
        *helper = Some(client);
        Ok(())
    }
    pub fn status(&self, scope: Scope) -> anyhow::Result<JobStatus> {
        let mut status = match scope {
            Scope::User => self.user.status(),
            Scope::System => self.system(Request::Status)?,
        };
        if let Some(error) = self.batch_error.lock().unwrap().clone() {
            status.error = Some(error);
        }
        if self.batch.lock().unwrap().contains(&scope) && status.state == JobState::Ready {
            status.state = JobState::Cleaning;
            status.current_rule = "等待前一个任务完成".into();
        }
        Ok(status)
    }
    pub fn batch_active(&self) -> bool {
        !self.batch.lock().unwrap().is_empty()
    }
    pub fn cancel_all(&self) {
        self.windows_maintenance.cancel();
        self.uv.cancel();
        self.maintenance.cancel();
        self.analysis.cancel_all();
        self.attachments.cancel();
        self.batch_cancel.store(true, Ordering::Release);
        self.user.cancel();
        if self.helper.lock().unwrap().is_some() {
            let _ = self.system::<serde_json::Value>(Request::Cancel);
        }
    }
}

pub fn run() {
    if !startup_preflight() {
        return;
    }
    let context = tauri::generate_context!();
    #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
    let context = {
        let mut context = context;
        context.config_mut().identifier = "io.dustaway.desktop.fixture".into();
        context
    };
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let state = window.state::<Arc<Service>>().inner().clone();
                if state.software.is_busy() {
                    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_YESNO, MB_ICONWARNING, IDYES};
                    use windows::core::w;
                    let answer = unsafe { MessageBoxW(None, w!("退出将停止跟踪软件卸载和残留处理。已删除内容不会恢复，卸载向导可能继续运行；下次打开需重新核对。是否退出？"), w!("退出拂尘"), MB_YESNO | MB_ICONWARNING) };
                    if answer != IDYES { return; }
                }
                if state.close_requested.swap(true, Ordering::AcqRel) {
                    return;
                }
                let app = window.app_handle().clone();
                let _ = app.emit("closing", ());
                // Never block the native UI thread on a command or filesystem lock.
                std::thread::spawn(move || {
                    let mut control = state.control.lock().unwrap();
                    control.closing = true;
                    let system_active = state.helper.lock().unwrap().is_some();
                    state.software.stop_tracking();
                    state.cancel_all();
                    drop(control);
                    while state.user.is_busy() || state.batch_active() || state.analysis.is_busy() || state.attachments.is_busy() || state.uv.is_busy() || state.maintenance.is_busy() || state.windows_maintenance.is_busy() {
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    if system_active {
                        while state.status(Scope::System).is_ok_and(|s| {
                            matches!(s.state, JobState::Scanning | JobState::Cleaning)
                        }) || state.system::<dustaway_windows::system_maintenance::WindowsSnapshot>(Request::WindowsSnapshot).is_ok_and(|s| matches!(s.status.as_str(), "scanning"|"preparing"|"running"|"stopping")) {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                    state.helper.lock().unwrap().take();
                    let _ = state.analysis.close_session();
                    app.exit(0);
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            scan_windows_maintenance,
            get_windows_maintenance,
            prepare_windows_maintenance,
            cancel_windows_maintenance,
            get_windows_maintenance_history,

            list_scan_observations,
            cancel_tool_preparation,
            refresh_cleanup_rule, prepare_tool_cache, execute_tool_cache, cancel_tool_cache, get_tool_task, get_tool_history,
            prepare_uv_cache,
            cancel_uv_preparation,
            execute_uv_cache,
            cancel_uv_cache,
            get_uv_task,
            get_uv_history,
            list_installed_software,
            prepare_uninstall,
            execute_uninstall,
            get_uninstall_task,
            verify_uninstall,
            scan_uninstall_residuals,
            list_uninstall_residuals,
            prepare_uninstall_residual_cleanup,
            execute_uninstall_residual_cleanup,
            cancel_uninstall_residual_task,
            get_uninstall_history,
            get_application_support,
            get_analysis_locations,
            get_attachment_locations,pick_attachment_location,start_attachment_scan,get_attachment_snapshot,
            list_attachments,get_attachment_thumbnail,reveal_attachment,cancel_attachment_scan,close_attachment_session,cancel_attachment_plan,
            read_analysis_page,cancel_analysis_query,get_analysis_count,get_analysis_storage,pick_analysis_storage,
            get_analysis_context,
            get_manual_selection_preview,
            cancel_manual_selection_preview,
            refine_analysis_location,
            list_analysis_files,
            list_analysis_discovery,
            open_apps_settings,
            pick_analysis_location,
            start_analysis,
            cancel_analysis,
            list_analysis_children,
            reveal_analysis_entry,
            bootstrap,
            start_scan,
            cancel_job,
            get_job_status,
            get_runtime_snapshot,
            reconcile_task_request,
            reveal_candidate,
            add_candidate_exclusions,
            get_scan_summary,
            list_candidates,
            refresh_rule_availability,
            prepare_cleanup,
            cancel_cleanup_preparation,
            exclude_analysis_entry,
            execute_cleanup,
            get_cleanup_report,
            get_history,
            save_settings,
            pick_exclusion,
            export_diagnostics,
            clear_history,
            open_storage_settings
        ])
        .build(context);
    let app = match app {
        Ok(app) => app,
        Err(error) => {
            show_startup_failure(&error.to_string(), false);
            return;
        }
    };
    // build() initializes the single-instance plugin. Only the owning instance may
    // open/recover the journal. Initialize before run() creates any webview, because
    // a failing setup hook panics inside the event loop rather than returning Err.
    let data_failed = AtomicBool::new(false);
    match initialize_service(&data_failed) {
        Ok(service) => {
            app.manage(service);
        }
        Err(error) => {
            show_startup_failure(&format!("{error:#}"), data_failed.load(Ordering::Acquire));
            return;
        }
    }
    app.run(|_, _| {});
}

fn initialize_service(data_failure: &AtomicBool) -> anyhow::Result<Arc<Service>> {
    anyhow::ensure!(
        !security::elevated()?,
        "请以普通用户身份启动拂尘。本版本日常清理不需要管理员权限。"
    );
    let data = platform::data_dir().inspect_err(|_| {
        data_failure.store(true, Ordering::Release);
    })?;
    std::fs::create_dir_all(&data).inspect_err(|_| {
        data_failure.store(true, Ordering::Release);
    })?;
    #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
    let (provider, journal): (Arc<dyn Platform>, std::path::PathBuf) = fixture::provider()?;
    #[cfg(not(all(debug_assertions, feature = "e2e-fixture")))]
    let (provider, journal): (Arc<dyn Platform>, std::path::PathBuf) = (
        Arc::new(dustaway_windows::WindowsPlatform::new(Scope::User)?),
        data.join("history.sqlite"),
    );
    let user = Engine::open(provider, Scope::User, &journal).inspect_err(|_| {
        data_failure.store(true, Ordering::Release);
    })?;
    #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
    let analysis_provider: Arc<dyn AnalysisPlatform> =
        Arc::new(dustaway_windows::WindowsAnalysis::for_fixture_volume(
            &journal.parent().unwrap().join("analysis"),
        )?);
    #[cfg(not(all(debug_assertions, feature = "e2e-fixture")))]
    let analysis_provider: Arc<dyn AnalysisPlatform> =
        Arc::new(dustaway_windows::WindowsAnalysis::new());
    let cache_path = std::fs::read(data.join("analysis-storage.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<std::path::PathBuf>(&b).ok())
        .filter(|p| dustaway_windows::fs::is_fixed_ntfs(p).is_ok())
        .unwrap_or_else(|| data.join("analysis-cache"));
    let default_cache = data.join("analysis-cache");
    let analysis = AnalysisEngine::with_storage(
        analysis_provider.clone(),
        AnalysisStorage::new(cache_path.clone()),
    )
    .or_else(|error| {
        if cache_path == default_cache {
            Err(error)
        } else {
            AnalysisEngine::with_storage(analysis_provider, AnalysisStorage::new(default_cache))
        }
    })?;
    #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
    let software_platform: Arc<dyn software::SoftwarePlatform> =
        Arc::new(fixture::FixtureSoftware::default());
    #[cfg(not(all(debug_assertions, feature = "e2e-fixture")))]
    let software_platform: Arc<dyn software::SoftwarePlatform> =
        Arc::new(dustaway_windows::software::WindowsSoftware);
    let software = software::SoftwareEngine::open(software_platform, &journal)?;
    #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
    let attachment_platform: Arc<dyn AttachmentPlatform> =
        Arc::new(fixture_attachments::FixtureAttachments);
    #[cfg(not(all(debug_assertions, feature = "e2e-fixture")))]
    let attachment_platform: Arc<dyn AttachmentPlatform> =
        Arc::new(dustaway_windows::attachments::WindowsAttachments);
    Ok(Arc::new(Service {
        windows_maintenance: dustaway_windows::system_maintenance::WindowsMaintenance::new(
            Scope::User,
            &data,
        )?,
        tool_plans: Mutex::new(Default::default()),
        maintenance: Arc::new(dustaway_windows::maintenance::Maintenance::new(&data)),
        uv: Arc::new(dustaway_windows::uv_cache::UvCache::new(&data)),
        software,
        analysis,
        attachments: AttachmentEngine::new(attachment_platform),
        user,
        helper: Mutex::new(None),
        control: Mutex::new(control::ControlState::default()),
        batch: Mutex::new(vec![]),
        batch_cancel: AtomicBool::new(false),
        close_requested: AtomicBool::new(false),
        batch_error: Mutex::new(None),
    }))
}

fn show_startup_failure(error: &str, data_failed: bool) {
    use windows::{
        Win32::UI::WindowsAndMessaging::{IDYES, MB_ICONERROR, MB_OK, MB_YESNO, MessageBoxW},
        core::{PCWSTR, w},
    };
    let message = dustaway_windows::fs::text_wide(&startup_failure_message(error, data_failed));
    unsafe {
        let answer = MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            w!("拂尘"),
            if data_failed {
                MB_YESNO | MB_ICONERROR
            } else {
                MB_OK | MB_ICONERROR
            },
        );
        if data_failed && answer == IDYES {
            let _ = commands::open_fixed_settings(false);
        }
    }
}

fn startup_failure_message(error: &str, data_failed: bool) -> String {
    if data_failed {
        format!(
            "拂尘无法读取或保存启动数据，尚未启动清理。请保留原有设置与历史数据库。\n\n{error}\n\n是否打开 Windows 存储设置检查可用空间？处理后可重新启动拂尘。"
        )
    } else {
        format!("拂尘无法启动。请根据以下原因检查运行环境。\n\n{error}")
    }
}

pub(crate) fn require_scope(scope: Scope) -> anyhow::Result<()> {
    anyhow::ensure!(
        scope != Scope::System || cfg!(feature = "system-cleanup"),
        ApiError::new("capability_unavailable", "当前免安装版本不提供系统提权清理")
    );
    Ok(())
}
pub(crate) fn available_rules() -> anyhow::Result<Vec<RuleDescriptor>> {
    Ok(dustaway_core::rules::catalog()?
        .into_iter()
        .filter(|r| !r.requires_admin || cfg!(feature = "system-cleanup"))
        .collect())
}
pub(crate) fn validate_scan_rules(ids: &[String]) -> anyhow::Result<Vec<RuleDescriptor>> {
    if ids.iter().any(|id| id == "windows.system_temp") {
        require_scope(Scope::System)?;
    }
    let rules = available_rules()?;
    anyhow::ensure!(
        !ids.is_empty() && ids.iter().all(|id| rules.iter().any(|r| &r.id == id)),
        ApiError::new("invalid_request", "请选择有效清理类别")
    );
    Ok(rules)
}

fn startup_preflight() -> bool {
    if tauri::webview_version().is_ok() {
        return true;
    }
    use windows::{
        Win32::UI::{
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                IDYES, MB_ICONINFORMATION, MB_YESNO, MessageBoxW, SW_SHOWNORMAL,
            },
        },
        core::{PCWSTR, w},
    };
    unsafe {
        let answer = MessageBoxW(
            None,
            w!(
                "未检测到可用的 Microsoft WebView2 Runtime。拂尘需要此组件显示界面。\n\n是否打开微软下载页？下载和安装由你自行完成；安装后重新启动拂尘。"
            ),
            w!("拂尘 · 需要 WebView2"),
            MB_YESNO | MB_ICONINFORMATION,
        );
        if answer == IDYES {
            ShellExecuteW(
                None,
                w!("open"),
                w!("https://developer.microsoft.com/microsoft-edge/webview2/#download-section"),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registered_business_commands_have_local_window_permissions() {
        let source = include_str!("lib.rs");
        let commands = source
            .split_once("tauri::generate_handler![")
            .unwrap()
            .1
            .split_once("])")
            .unwrap()
            .0;
        let manifest = include_str!("../build.rs");
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!(["main"]));
        assert!(capability.get("remote").is_none());
        for command in commands.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            assert!(
                manifest.contains(&format!("\"{command}\"")),
                "Command missing from manifest: {command}"
            );
            let permission = format!("allow-{}", command.replace('_', "-"));
            assert!(
                capability["permissions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|p| p.as_str() == Some(&permission)),
                "Window permission missing: {command}"
            );
        }
    }
    #[test]
    fn startup_data_failure_preserves_cause_and_offers_native_storage_recovery() {
        let message = startup_failure_message("database or disk is full", true);
        assert!(message.contains("database or disk is full"));
        assert!(message.contains("尚未启动清理"));
        assert!(message.contains("保留原有设置与历史数据库"));
        assert!(message.contains("Windows 存储设置"));
        assert!(!startup_failure_message("webview failed", false).contains("Windows 存储设置"));
    }
    #[test]
    fn portable_capabilities_reject_system_and_mixed_requests() {
        if cfg!(feature = "system-cleanup") {
            return;
        }
        assert_eq!(available_rules().unwrap().len(), 115);
        assert!(require_scope(Scope::System).is_err());
        for ids in [
            vec!["windows.system_temp".into()],
            vec!["windows.user_temp".into(), "windows.system_temp".into()],
        ] {
            let error = validate_scan_rules(&ids).unwrap_err();
            assert_eq!(
                error.downcast_ref::<ApiError>().unwrap().code,
                "capability_unavailable"
            );
        }
        assert!(validate_scan_rules(&["windows.user_temp".into()]).is_ok());
        assert!(validate_scan_rules(&["invented".into()]).is_err());
    }
}
