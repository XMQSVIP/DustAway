use crate::{
    Service,
    commands::{ApiResult, AppState, controlled, worker},
};
use anyhow::{Result, ensure};
use dustaway_core::{CommandRequest, ControlInfo, software::*};

fn idle(state: &Service) -> Result<()> {
    ensure!(
        !state.user.is_busy() && !state.batch_active() && !state.analysis.is_busy(),
        "已有扫描、分析或清理任务，请等待完成"
    );
    if state.helper.lock().unwrap().is_some() {
        let s = state.status(dustaway_core::Scope::System)?;
        ensure!(
            !matches!(
                s.state,
                dustaway_core::JobState::Scanning | dustaway_core::JobState::Cleaning
            ),
            "系统清理进行中"
        );
    }
    Ok(())
}
#[tauri::command]
pub async fn list_installed_software(state: AppState<'_>) -> ApiResult<SoftwareList> {
    let engine = state.software.clone();
    worker(move || engine.list()).await
}
#[tauri::command]
pub async fn prepare_uninstall(
    state: AppState<'_>,
    request: CommandRequest,
    software_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "prepare_uninstall",
        move |s| {
            idle(s)?;
            s.software.prepare(&software_id)
        },
    )
    .await
}
#[tauri::command]
pub async fn execute_uninstall(
    state: AppState<'_>,
    request: CommandRequest,
    plan_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "execute_uninstall",
        move |s| {
            idle(s)?;
            s.user.invalidate_for_software()?;
            s.analysis.mark_stale();
            s.software.start(&plan_id)
        },
    )
    .await
}
#[tauri::command]
pub async fn get_uninstall_task(state: AppState<'_>) -> ApiResult<Option<UninstallTask>> {
    Ok(state.software.snapshot())
}
#[tauri::command]
pub async fn verify_uninstall(
    state: AppState<'_>,
    request: CommandRequest,
    task_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "verify_uninstall",
        move |s| {
            idle(s)?;
            s.software.verify(&task_id)
        },
    )
    .await
}
#[tauri::command]
pub async fn scan_uninstall_residuals(
    state: AppState<'_>,
    request: CommandRequest,
    task_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "scan_uninstall_residuals",
        move |s| {
            idle(s)?;
            s.software.scan(&task_id, s.user.settings().exclusions)
        },
    )
    .await
}
#[tauri::command]
pub async fn list_uninstall_residuals(
    state: AppState<'_>,
    task_id: String,
    offset: usize,
    search: String,
) -> ApiResult<ResidualScan> {
    let engine = state.software.clone();
    worker(move || engine.residuals(&task_id, offset, &search)).await
}
#[tauri::command]
pub async fn prepare_uninstall_residual_cleanup(
    state: AppState<'_>,
    request: CommandRequest,
    task_id: String,
    candidate_ids: Vec<String>,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "prepare_uninstall_residual_cleanup",
        move |s| {
            idle(s)?;
            s.software.prepare_cleanup(&task_id, candidate_ids)
        },
    )
    .await
}
#[tauri::command]
pub async fn execute_uninstall_residual_cleanup(
    state: AppState<'_>,
    request: CommandRequest,
    plan_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "execute_uninstall_residual_cleanup",
        move |s| {
            idle(s)?;
            s.software.clean(&plan_id, s.user.settings().exclusions)
        },
    )
    .await
}
#[tauri::command]
pub async fn cancel_uninstall_residual_task(
    state: AppState<'_>,
    request: CommandRequest,
    task_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "cancel_uninstall_residual_task",
        move |s| s.software.cancel_residuals(&task_id),
    )
    .await
}
#[tauri::command]
pub async fn get_uninstall_history(state: AppState<'_>) -> ApiResult<Vec<UninstallReport>> {
    let engine = state.software.clone();
    worker(move || engine.history()).await
}
