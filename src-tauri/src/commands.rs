use crate::Service;
use anyhow::{Result, ensure};
use dustaway_core::*;
use dustaway_windows::ipc::Request;
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

pub(crate) type AppState<'a> = State<'a, Arc<Service>>;
pub(crate) type ApiResult<T> = std::result::Result<T, ApiError>;
#[tauri::command]
pub async fn get_application_support() -> ApiResult<Vec<ApplicationSupport>> {
    worker(|| Ok(dustaway_windows::applications::support())).await
}
fn api_error(error: anyhow::Error) -> ApiError {
    classify_api_error(error)
}

#[tauri::command]
pub async fn read_analysis_page(
    state: AppState<'_>,
    query: AnalysisReadQuery,
) -> ApiResult<AnalysisReadPage> {
    let state = state.inner().clone();
    let cancel = state
        .analysis
        .begin_query(&query.request_id)
        .map_err(api_error)?;
    worker(move || state.analysis.read_registered_page(query, cancel)).await
}
#[tauri::command]
pub async fn cancel_analysis_query(state: AppState<'_>, request_id: String) -> ApiResult<()> {
    state.analysis.cancel_query(&request_id);
    Ok(())
}
#[tauri::command]
pub async fn get_analysis_count(
    state: AppState<'_>,
    request_id: String,
) -> ApiResult<AnalysisCount> {
    state.analysis.query_count(&request_id).map_err(api_error)
}
#[tauri::command]
pub async fn get_analysis_storage(state: AppState<'_>) -> ApiResult<Option<String>> {
    Ok(state.analysis.storage_location())
}
#[tauri::command]
pub async fn pick_analysis_storage(
    app: tauri::AppHandle,
    state: AppState<'_>,
) -> ApiResult<Option<String>> {
    let state = state.inner().clone();
    worker(move || {
        let selected = app
            .dialog()
            .file()
            .set_title("选择临时索引所在的本地磁盘或文件夹")
            .blocking_pick_folder();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let parent = selected.into_path()?;
        dustaway_windows::fs::is_fixed_ntfs(&parent)?;
        let _control = state.control.lock().unwrap();
        ensure!(
            !state.user.is_busy()
                && !state.analysis.is_busy()
                && !state.batch_active()
                && !state.software.is_busy(),
            "请先结束当前任务"
        );
        let path = parent.join("DustAway-analysis-cache");
        state.user.invalidate_plan()?;
        state
            .analysis
            .configure_storage(AnalysisStorage::new(path.clone()))?;
        let data = dustaway_windows::platform::data_dir()?;
        std::fs::write(
            data.join("analysis-storage.json"),
            serde_json::to_vec(&path)?,
        )?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
}

#[tauri::command]
pub async fn get_analysis_locations(state: AppState<'_>) -> ApiResult<Vec<AnalysisLocation>> {
    Ok(state.analysis.locations())
}
#[tauri::command]
pub async fn get_manual_selection_preview(
    state: AppState<'_>,
    selection: ManualSelection,
    request_id: String,
) -> ApiResult<ManualPreview> {
    let state = state.inner().clone();
    let cancel = state
        .analysis
        .begin_manual_preview(&request_id)
        .map_err(api_error)?;
    worker(move || {
        state
            .analysis
            .preview_manual(&selection, request_id, cancel)
    })
    .await
}
#[tauri::command]
pub async fn cancel_manual_selection_preview(
    state: AppState<'_>,
    request_id: String,
) -> ApiResult<()> {
    state.analysis.cancel_manual_preview(&request_id);
    Ok(())
}
#[tauri::command]
pub async fn get_analysis_context(
    state: AppState<'_>,
    location_token: String,
) -> ApiResult<AnalysisContext> {
    let state = state.inner().clone();
    worker(move || state.analysis.context(&location_token)).await
}
#[tauri::command]
pub async fn refine_analysis_location(
    state: AppState<'_>,
    session: String,
    node: String,
) -> ApiResult<AnalysisLocation> {
    let state = state.inner().clone();
    worker(move || state.analysis.refine(&session, &node)).await
}
#[tauri::command]
pub async fn list_analysis_discovery(
    state: AppState<'_>,
    session: String,
    query: AnalysisDiscoveryQuery,
) -> ApiResult<AnalysisFilesPage> {
    let state = state.inner().clone();
    worker(move || state.analysis.discovery(&session, &query)).await
}

#[tauri::command]
pub async fn list_analysis_files(
    state: AppState<'_>,
    session: String,
    search: String,
    offset: u64,
) -> ApiResult<AnalysisFilesPage> {
    let state = state.inner().clone();
    worker(move || state.analysis.files(&session, &search, offset)).await
}
#[tauri::command]
pub async fn pick_analysis_location(
    app: tauri::AppHandle,
    state: AppState<'_>,
) -> ApiResult<Option<AnalysisLocation>> {
    let state = state.inner().clone();
    worker(move || {
        let selected = app
            .dialog()
            .file()
            .set_title("选择只读分析的目录")
            .blocking_pick_folder();
        selected
            .map(|p| {
                let path = p.into_path()?;
                state.analysis.register(
                    path.to_str()
                        .ok_or_else(|| ApiError::new("invalid_location", "目录名称无法显示"))?,
                )
            })
            .transpose()
    })
    .await
}
#[tauri::command]
pub async fn start_analysis(
    state: AppState<'_>,
    request: CommandRequest,
    token: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "start_analysis", move |state| {
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        if state.helper.lock().unwrap().is_some() {
            ensure!(
                !matches!(
                    state.status(Scope::System)?.state,
                    JobState::Scanning | JobState::Cleaning
                ),
                ApiError::new("busy", "系统任务进行中")
            );
        }
        state.user.invalidate_plan()?;
        state
            .analysis
            .start(&token, state.user.settings().exclusions)
    })
    .await
}
#[tauri::command]
pub async fn cancel_analysis(
    state: AppState<'_>,
    request: CommandRequest,
    session: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "cancel_analysis", move |state| {
        state.analysis.cancel(&session)
    })
    .await
}
#[tauri::command]
pub async fn list_analysis_children(
    state: AppState<'_>,
    session: String,
    node: String,
    search: String,
    offset: u64,
    descending: bool,
) -> ApiResult<AnalysisPage> {
    let state = state.inner().clone();
    worker(move || {
        state
            .analysis
            .children(&session, &node, &search, offset, descending)
    })
    .await
}
#[tauri::command]
pub async fn reveal_analysis_entry(
    state: AppState<'_>,
    session: String,
    node: String,
) -> ApiResult<()> {
    let state = state.inner().clone();
    worker(move || state.analysis.reveal(&session, &node)).await
}
pub(crate) async fn worker<T: Send + 'static>(
    f: impl FnOnce() -> Result<T> + Send + 'static,
) -> ApiResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| ApiError::new("worker_failed", e.to_string()))?
        .map_err(api_error)
}

pub(crate) async fn controlled<T: Serialize + Send + 'static>(
    state: Arc<Service>,
    request: CommandRequest,
    command: &'static str,
    work: impl FnOnce(&Arc<Service>) -> Result<T> + Send + 'static,
) -> ApiResult<ControlInfo> {
    worker(move || {
        let mut control = state.control.lock().unwrap();
        ensure!(
            !state.close_requested.load(Ordering::Acquire),
            ApiError::new("closing", "程序正在退出")
        );
        control.run(&request, command, || {
            if !matches!(command, "cancel_windows_maintenance" | "cancel_job") {
                ensure!(
                    !state.windows_maintenance.is_busy(),
                    "Windows 维护任务进行中，请等待或停止"
                );
                if state.helper.lock().unwrap().is_some() {
                    let status: dustaway_windows::system_maintenance::WindowsSnapshot =
                        state.system(Request::WindowsSnapshot)?;
                    ensure!(
                        !matches!(
                            status.status.as_str(),
                            "scanning" | "preparing" | "running" | "stopping"
                        ),
                        "系统维护任务进行中，请等待或停止"
                    );
                }
            }
            ensure!(
                command == "cancel_job"
                    || !state
                        .user
                        .status()
                        .rule_refresh
                        .as_ref()
                        .is_some_and(|r| r.state == JobState::Scanning),
                ApiError::new("busy", "单项检查进行中，请等待或取消检查")
            );
            ensure!(
                !state.maintenance.is_busy() || command == "cancel_tool_cache",
                ApiError::new("busy", "官方缓存整理进行中，请等待或停止任务")
            );
            ensure!(
                !state.uv.is_busy() || command == "cancel_uv_cache",
                ApiError::new("busy", "uv 缓存整理进行中，请等待或停止任务")
            );
            ensure!(
                !state.software.is_busy() || command == "cancel_uninstall_residual_task",
                ApiError::new("busy", "软件卸载或残留处理进行中，请等待完成")
            );
            ensure!(
                !state.analysis.is_busy()
                    || matches!(
                        command,
                        "cancel_analysis" | "save_settings" | "clear_history"
                    ),
                ApiError::new("busy", "空间分析进行中，请等待或停止任务")
            );
            ensure!(
                !state.attachments.is_busy() || command == "cancel_job",
                ApiError::new("busy", "附件扫描进行中，请等待或停止扫描")
            );
            work(&state)
        })
    })
    .await
}
fn snapshot_locked(
    state: &Service,
    scope: Scope,
    control: &crate::control::ControlState,
) -> Result<RuntimeSnapshot> {
    crate::require_scope(scope)?;
    let batch = state.batch.lock().unwrap();
    let mut snapshot = match scope {
        Scope::User => state.user.runtime_snapshot(),
        Scope::System => state.system(Request::Snapshot)?,
    };
    if batch.contains(&scope) && snapshot.status.state == JobState::Ready {
        snapshot.status.state = JobState::Cleaning;
        snapshot.summary.state = JobState::Cleaning;
        snapshot.status.current_rule = "正在准备清理".into();
    }
    if let Some(error) = state.batch_error.lock().unwrap().clone() {
        snapshot.status.error = Some(error);
    }
    snapshot.control = Some(control.info());
    if scope == Scope::User {
        snapshot.analysis = state.analysis.snapshot();
        snapshot.uninstall = state.software.snapshot();
    }
    Ok(snapshot)
}
#[tauri::command]
pub async fn reconcile_task_request(
    state: AppState<'_>,
    request: Option<CommandRequest>,
) -> ApiResult<RuntimeSnapshot> {
    let state = state.inner().clone();
    worker(move || {
        let mut control = state.control.lock().unwrap();
        control.reconcile(request.as_ref())?;
        snapshot_locked(&state, Scope::User, &control)
    })
    .await
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionRef {
    scope: Scope,
    id: String,
}
#[derive(Serialize)]
pub struct ScanStart {
    sessions: Vec<SessionRef>,
    warning: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct PlanRequest {
    scope: Scope,
    session: String,
    selection: Selection,
}
#[derive(Serialize, Deserialize)]
pub struct ScopedPlan {
    scope: Scope,
    plan: CleanupPlan,
}
#[derive(Serialize)]
pub struct ExecuteStart {
    scopes: Vec<Scope>,
    warning: Option<String>,
}

#[tauri::command]
pub async fn bootstrap(state: AppState<'_>) -> ApiResult<Bootstrap> {
    let state = state.inner().clone();
    worker(move || {
        #[cfg(all(debug_assertions, feature = "e2e-fixture"))]
        let version = concat!(env!("CARGO_PKG_VERSION"), "+e2e-fixture");
        #[cfg(not(all(debug_assertions, feature = "e2e-fixture")))]
        let version = env!("CARGO_PKG_VERSION");
        Ok(Bootstrap {
            preferences: state.user.preferences()?,
            rules: crate::available_rules()?,
            settings: state.user.settings(),
            version: version.into(),
            capabilities: Capabilities {
                system_cleanup: cfg!(feature = "system-cleanup"),
            },
        })
    })
    .await
}
#[tauri::command]
pub async fn start_scan(
    state: AppState<'_>,
    request: CommandRequest,
    rule_ids: Vec<String>,
    volume_tokens: Option<Vec<String>>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "start_scan", move |state| {
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        *state.batch_error.lock().unwrap() = None;
        let all = crate::validate_scan_rules(&rule_ids)?;
        ensure!(
            volume_tokens.is_none()
                || all
                    .iter()
                    .filter(|r| rule_ids.contains(&r.id))
                    .all(|r| !r.requires_admin),
            "指定磁盘扫描仅支持普通用户范围"
        );
        let volumes = volume_tokens
            .as_ref()
            .map(|tokens| state.analysis.scan_volumes(tokens))
            .transpose()?;
        ensure!(
            !state.user.is_busy(),
            ApiError::new("busy", "已有任务正在运行")
        );
        let connected = state.helper.lock().unwrap().is_some();
        if connected {
            match state.status(Scope::System) {
                Ok(status) => ensure!(
                    !matches!(status.state, JobState::Scanning | JobState::Cleaning),
                    "系统任务进行中"
                ),
                Err(_) => {
                    state.helper.lock().unwrap().take();
                }
            }
        }
        let user_ids: Vec<_> = rule_ids
            .iter()
            .filter(|id| all.iter().any(|r| r.id == **id && !r.requires_admin))
            .cloned()
            .collect();
        let mut result = ScanStart {
            sessions: vec![],
            warning: None,
        };
        if !user_ids.is_empty() {
            let id = state.user.start_scan_on_volumes(user_ids, volumes)?;
            if state.user.remember_scan(&id).is_err() {
                result.warning = Some("扫描已开始，但本次类别选择未保存".into());
            }
            result.sessions.push(SessionRef {
                scope: Scope::User,
                id,
            });
        }
        if rule_ids
            .iter()
            .any(|id| all.iter().any(|r| r.id == *id && r.requires_admin))
        {
            let system = (|| -> Result<String> {
                state.launch_system()?;
                state.system(Request::Scan {
                    rule_ids: rule_ids
                        .iter()
                        .filter(|id| all.iter().any(|r| r.id == **id && r.requires_admin))
                        .cloned()
                        .collect(),
                })
            })();
            match system {
                Ok(id) => result.sessions.push(SessionRef {
                    scope: Scope::System,
                    id,
                }),
                Err(e) => result.warning = Some(format!("{e:#}")),
            }
        }
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn cancel_job(
    state: AppState<'_>,
    request: CommandRequest,
    targets: Vec<SessionRef>,
    refresh_task_id: Option<String>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "cancel_job", move |state| {
        ensure!(
            !targets.is_empty(),
            ApiError::new("invalid_session", "缺少目标任务")
        );
        if let Some(task) = refresh_task_id {
            ensure!(
                targets.len() == 1 && targets[0].scope == Scope::User,
                "单项检查目标无效"
            );
            state.user.cancel_rule_refresh(&targets[0].id, &task)?;
        } else {
            for target in &targets {
                crate::require_scope(target.scope)?;
                ensure!(
                    state.status(target.scope)?.id == target.id && !target.id.is_empty(),
                    ApiError::new("invalid_session", "停止请求的目标任务已变化")
                );
            }

            ensure!(
                !state
                    .user
                    .status()
                    .rule_refresh
                    .as_ref()
                    .is_some_and(|r| r.state == JobState::Scanning),
                "取消单项检查需要对应任务标识"
            );
            state.cancel_all();
        }
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn get_job_status(state: AppState<'_>, scope: Scope) -> ApiResult<JobStatus> {
    let state = state.inner().clone();
    worker(move || state.status(scope)).await
}
#[tauri::command]
pub async fn get_runtime_snapshot(state: AppState<'_>, scope: Scope) -> ApiResult<RuntimeSnapshot> {
    let state = state.inner().clone();
    worker(move || {
        let control = state.control.lock().unwrap();
        snapshot_locked(&state, scope, &control)
    })
    .await
}

#[tauri::command]
pub async fn reveal_candidate(
    state: AppState<'_>,
    scope: Scope,
    session: String,
    candidate_id: String,
) -> ApiResult<()> {
    let state = state.inner().clone();
    worker(move || {
        let _control = state.control.lock().unwrap();
        ensure!(
            scope == Scope::User,
            ApiError::new("capability_unavailable", "此操作仅支持用户清理")
        );
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        state.user.reveal_candidate(&session, &candidate_id)
    })
    .await
}
#[tauri::command]
pub async fn add_candidate_exclusions(
    state: AppState<'_>,
    request: CommandRequest,
    scope: Scope,
    session: String,
    items: Vec<CandidateExclusion>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "add_candidate_exclusions", move |state| {
        ensure!(
            scope == Scope::User,
            ApiError::new("capability_unavailable", "此操作仅支持用户清理")
        );
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        let result = state.user.add_candidate_exclusions(&session, items)?;
        if result.scan_invalidated {
            state.maintenance.invalidate_plan();
            state.analysis.mark_stale();
        }
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn get_scan_summary(state: AppState<'_>, scope: Scope) -> ApiResult<ScanSummary> {
    let state = state.inner().clone();
    worker(move || match scope {
        Scope::User => Ok(state.user.summary()),
        Scope::System => state.system(Request::Summary),
    })
    .await
}
#[tauri::command]
pub async fn list_scan_observations(
    state: AppState<'_>,
    scope: Scope,
    session: String,
    rule: String,
    offset: u64,
    limit: u32,
) -> ApiResult<ObservationPage> {
    let state = state.inner().clone();
    worker(move || match scope {
        Scope::User => state.user.observations(&session, &rule, offset, limit),
        Scope::System => state.system(Request::Observations {
            session,
            rule,
            offset,
            limit,
        }),
    })
    .await
}

#[tauri::command]
pub async fn list_candidates(
    state: AppState<'_>,
    scope: Scope,
    session: String,
    rule: String,
    offset: u64,
    limit: u32,
    query: Option<CandidateQuery>,
) -> ApiResult<CandidatePage> {
    let state = state.inner().clone();
    worker(move || match scope {
        Scope::User => {
            state
                .user
                .query_candidates(&session, &rule, offset, limit, query.unwrap_or_default())
        }
        Scope::System => state.system(Request::Candidates {
            session,
            rule,
            offset,
            limit,
            query: query.unwrap_or_default(),
        }),
    })
    .await
}
#[tauri::command]
pub async fn refresh_rule_availability(
    state: AppState<'_>,
    request: CommandRequest,
    scope: Scope,
    session: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "refresh_rule_availability", move |state| {
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        match scope {
            Scope::User => state.user.refresh_availability(&session),
            Scope::System => state.system(Request::Refresh { session }),
        }
    })
    .await
}
#[tauri::command]
pub async fn prepare_cleanup(
    state: AppState<'_>,
    request: CommandRequest,
    requests: Vec<PlanRequest>,
    manual: Option<ManualSelection>,
    attachments: Option<AttachmentSelection>,
    preparation_id: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    let operation = state
        .analysis
        .begin_manual_preparation(&preparation_id)
        .map_err(api_error)?;
    controlled(state, request, "prepare_cleanup", move |state| {
        operation.check()?;
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        ensure!(
            (!requests.is_empty() || manual.is_some() || attachments.is_some())
                && requests.len() <= 2,
            "清理请求无效"
        );
        for req in &requests {
            crate::require_scope(req.scope)?;
            if req
                .selection
                .rule_ids
                .iter()
                .any(|id| id == "windows.system_temp")
            {
                crate::require_scope(Scope::System)?;
            }
        }
        ensure!(
            requests.len() <= 1 || requests[0].scope != requests[1].scope,
            "清理范围重复"
        );
        state.user.invalidate_plan()?;
        if let Some(selection) = attachments {
            ensure!(
                requests.is_empty() && manual.is_none(),
                "附件不能与常规垃圾或自选文件混合清理"
            );
            let manifest = state.attachments.prepare(
                &selection,
                &state.user.settings().exclusions,
                operation.cancellation(),
            )?;
            operation.check()?;
            let plan = state.user.attach_manual(manifest)?;
            if let Err(error) = operation.check() {
                state.user.invalidate_plan()?;
                return Err(error);
            }
            return Ok(vec![ScopedPlan {
                scope: Scope::User,
                plan,
            }]);
        }
        // Preparation failure must not leave an executable old plan.
        let manifest = manual
            .as_ref()
            .map(|selection| {
                state
                    .analysis
                    .prepare_manual_cancellable(selection, operation.cancellation())
            })
            .transpose()?;
        let mut result = vec![];
        for req in requests {
            operation.check()?;
            let plan = match req.scope {
                Scope::User => state.user.prepare(&req.session, req.selection)?,
                Scope::System => state.system(Request::Prepare {
                    session: req.session,
                    selection: req.selection,
                })?,
            };
            result.push(ScopedPlan {
                scope: req.scope,
                plan,
            });
        }
        if let Some(manifest) = manifest {
            operation.check()?;
            state
                .analysis
                .validate_manual_session(&manifest.summary.session)?;
            let plan = state.user.attach_manual(manifest)?;
            result.retain(|p| p.scope != Scope::User);
            result.push(ScopedPlan {
                scope: Scope::User,
                plan,
            });
        }
        if let Err(error) = operation.check().and_then(|_| {
            manual
                .as_ref()
                .map(|m| state.analysis.validate_manual_session(&m.session))
                .transpose()
                .map(|_| ())
        }) {
            state.user.invalidate_plan()?;
            return Err(error);
        }
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn cancel_cleanup_preparation(
    state: AppState<'_>,
    preparation_id: String,
) -> ApiResult<()> {
    // Deliberately does not wait for the command mutex held by preparation.
    state
        .analysis
        .cancel_manual_preparation(&preparation_id)
        .map_err(api_error)
}
#[tauri::command]
pub async fn exclude_analysis_entry(
    state: AppState<'_>,
    request: CommandRequest,
    session: String,
    node: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "exclude_analysis_entry", move |state| {
        ensure!(!state.user.is_busy() && !state.batch_active(), "任务进行中");
        let path = state.analysis.manual_exclusion(&session, &node)?;
        let mut settings = state.user.settings();
        if !settings
            .exclusions
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&path))
        {
            settings.exclusions.push(path);
        }
        let result = state.user.save_settings(settings)?;
        state.analysis.mark_stale();
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn execute_cleanup(
    state: AppState<'_>,
    request: CommandRequest,
    plans: Vec<ScopedPlan>,
    windows_plans: Option<Vec<crate::windows_maintenance_commands::WindowsPlanRef>>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "execute_cleanup", move |state| {
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "已有清理任务正在运行")
        );
        let windows_plans = windows_plans.unwrap_or_default();
        ensure!(
            (!plans.is_empty() || !windows_plans.is_empty())
                && plans.len() <= 2
                && windows_plans.len() <= 2,
            "清理计划无效"
        );
        ensure!(
            windows_plans.len() < 2 || windows_plans[0].scope != windows_plans[1].scope,
            "维护范围重复"
        );
        for item in &windows_plans {
            crate::require_scope(item.scope)?;
            match item.scope {
                Scope::User => state.windows_maintenance.validate_plan(
                    &item.plan_id,
                    &dustaway_windows::system_maintenance::settings_exclusions(
                        &state.user.settings(),
                    ),
                )?,
                Scope::System => state
                    .system::<serde_json::Value>(Request::WindowsValidate {
                        plan_id: item.plan_id.clone(),
                    })
                    .map(|_| ())?,
            }
        }
        for item in &plans {
            crate::require_scope(item.scope)?;
        }
        ensure!(
            plans.len() < 2 || plans[0].scope != plans[1].scope,
            "清理范围重复"
        );
        for item in &plans {
            match item.scope {
                Scope::User => state.user.validate_plan(&item.plan.id)?,
                Scope::System => {
                    state.system::<serde_json::Value>(Request::Validate {
                        plan_id: item.plan.id.clone(),
                    })?;
                }
            }
        }
        let result = ExecuteStart {
            scopes: [Scope::User, Scope::System]
                .into_iter()
                .filter(|s| {
                    plans.iter().any(|p| p.scope == *s)
                        || windows_plans.iter().any(|p| p.scope == *s)
                })
                .collect(),
            warning: None,
        };
        // Initialize user execution under control, before a stop can be accepted.
        if windows_plans.is_empty() && plans.len() == 1 && plans[0].scope == Scope::User {
            *state.batch_error.lock().unwrap() = None;
            state.user.execute(&plans[0].plan.id)?;
            state.analysis.mark_stale();
            return Ok(result);
        }
        *state.batch.lock().unwrap() = result.scopes.clone();
        state.analysis.mark_stale();
        state.batch_cancel.store(false, Ordering::Release);
        *state.batch_error.lock().unwrap() = None;
        let runner = state.clone();
        std::thread::spawn(move || {
            let outcome = (|| -> Result<()> {
                for item in plans {
                    let startup = runner.control.lock().unwrap();
                    if runner.batch_cancel.load(Ordering::Acquire) {
                        break;
                    }
                    match item.scope {
                        Scope::User => {
                            runner.user.execute(&item.plan.id)?;
                        }
                        Scope::System => {
                            runner.system::<String>(Request::Execute {
                                plan_id: item.plan.id,
                            })?;
                        }
                    }
                    drop(startup);
                    loop {
                        if runner.batch_cancel.load(Ordering::Acquire) {
                            runner.user.cancel();
                            if runner.helper.lock().unwrap().is_some() {
                                let _ = runner.system::<serde_json::Value>(Request::Cancel);
                            }
                        }
                        let status = match item.scope {
                            Scope::User => runner.user.status(),
                            Scope::System => runner.system(Request::Status)?,
                        };
                        if !matches!(status.state, JobState::Scanning | JobState::Cleaning) {
                            ensure!(
                                status.state != JobState::Failed,
                                "{}",
                                status.error.unwrap_or_else(|| "清理异常中断".into())
                            );
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    if item.scope == Scope::System
                        && let Some(report) =
                            runner.system::<Option<CleanupReport>>(Request::Report)?
                    {
                        runner.user.record_external_report(&report)?;
                    }
                    if windows_plans.is_empty() {
                        runner.batch.lock().unwrap().retain(|s| *s != item.scope);
                    }
                }
                for item in windows_plans {
                    if runner.batch_cancel.load(Ordering::Acquire) {
                        break;
                    }
                    {
                        let _control = runner.control.lock().unwrap();
                        match item.scope {
                            Scope::User => {
                                runner.windows_maintenance.execute(
                                    item.plan_id,
                                    dustaway_windows::system_maintenance::settings_exclusions(
                                        &runner.user.settings(),
                                    ),
                                )?;
                            }
                            Scope::System => {
                                runner.system::<String>(Request::WindowsExecute {
                                    plan_id: item.plan_id,
                                })?;
                            }
                        }
                    }
                    loop {
                        if runner.batch_cancel.load(Ordering::Acquire) {
                            runner.cancel_all();
                        }
                        let status: dustaway_windows::system_maintenance::WindowsSnapshot =
                            match item.scope {
                                Scope::User => runner.windows_maintenance.snapshot(),
                                Scope::System => runner.system(Request::WindowsSnapshot)?,
                            };
                        if !matches!(status.status.as_str(), "running" | "stopping") {
                            ensure!(status.status != "uncertain", "Windows 维护结果待核实");
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                Ok(())
            })();
            if let Err(error) = outcome {
                *runner.batch_error.lock().unwrap() = Some(format!("清理已中断：{error:#}"));
            }
            runner.batch.lock().unwrap().clear();
        });
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn get_cleanup_report(
    state: AppState<'_>,
    scope: Scope,
) -> ApiResult<Option<CleanupReport>> {
    let state = state.inner().clone();
    worker(move || {
        let report: Option<CleanupReport> = match scope {
            Scope::User => state.user.report(),
            Scope::System => state.system(Request::Report)?,
        };
        if let Some(report) = &report
            && scope == Scope::System
        {
            state.user.record_external_report(report)?;
        }
        Ok(report)
    })
    .await
}
#[tauri::command]
pub async fn get_history(state: AppState<'_>) -> ApiResult<Vec<CleanupReport>> {
    let state = state.inner().clone();
    worker(move || state.user.history()).await
}
#[tauri::command]
pub async fn clear_history(state: AppState<'_>, request: CommandRequest) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "clear_history", |state| {
        ensure!(
            !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        state.software.clear_history()?;
        state.uv.clear_history()?;
        state.maintenance.clear_history()?;
        state.user.clear_history()
    })
    .await
}
#[tauri::command]
pub async fn save_settings(
    state: AppState<'_>,
    request: CommandRequest,
    patch: SettingsPatch,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "save_settings", move |state| {
        let old = state.user.settings();
        let settings = patch.apply(old.clone());
        let scope_changed =
            old.exclusions != settings.exclusions || old.excluded_rules != settings.excluded_rules;
        ensure!(
            !scope_changed || !state.batch_active(),
            ApiError::new("busy", "清理任务进行中")
        );
        for path in &settings.exclusions {
            dustaway_windows::fs::extended(std::path::Path::new(path))?;
        }
        ensure!(
            !scope_changed || (!state.user.is_busy() && !state.analysis.is_busy()),
            ApiError::new("busy", "任务进行中")
        );
        if state.helper.lock().unwrap().is_some() {
            state.system::<serde_json::Value>(Request::Settings {
                settings: settings.clone(),
            })?;
        }
        let result = state.user.save_settings(settings)?;
        if result.scan_invalidated {
            state.maintenance.invalidate_plan();
            state.analysis.mark_stale();
        }
        Ok(result)
    })
    .await
}
#[tauri::command]
pub async fn pick_exclusion(app: tauri::AppHandle, kind: String) -> ApiResult<Option<String>> {
    worker(move || {
        ensure!(kind == "file" || kind == "folder", "无效选择类型");
        let dialog = app.dialog().file().set_title("选择不需要清理的文件或目录");
        let selected = if kind == "folder" {
            dialog.blocking_pick_folder()
        } else {
            dialog.blocking_pick_file()
        };
        selected
            .map(|p| {
                p.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(anyhow::Error::from)
            })
            .transpose()
    })
    .await
}
#[tauri::command]
pub async fn export_diagnostics(app: tauri::AppHandle, state: AppState<'_>) -> ApiResult<bool> {
    let state = state.inner().clone();
    worker(move||{
    let selected=app.dialog().file().set_title("导出诊断摘要（不包含个人路径）").set_file_name("dustaway-diagnostics.json").add_filter("JSON",&["json"]).blocking_save_file();
    let Some(selected)=selected else{return Ok(false)};
    let data=serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"platform":"windows-x64","rules":dustaway_core::rules::catalog()?.iter().map(|r|(&r.id,r.version)).collect::<Vec<_>>(),"reports":state.user.history()?,"paths_redacted":true});
    std::fs::write(selected.into_path()?,serde_json::to_vec_pretty(&data)?)?;Ok(true)
}).await
}
#[tauri::command]
pub async fn open_storage_settings() -> ApiResult<()> {
    worker(|| open_fixed_settings(false)).await
}
#[tauri::command]
pub async fn open_apps_settings() -> ApiResult<()> {
    worker(|| open_fixed_settings(true)).await
}
pub(crate) fn open_fixed_settings(apps: bool) -> Result<()> {
    use windows::{
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        core::{PCWSTR, w},
    };
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            if apps {
                w!("ms-settings:appsfeatures")
            } else {
                w!("ms-settings:storagesense")
            },
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    ensure!(
        result.0 as isize > 32,
        "无法打开 Windows 设置，请从开始菜单打开设置后重试"
    );
    Ok(())
}
