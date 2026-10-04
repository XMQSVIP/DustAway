use crate::commands::{ApiResult, AppState, controlled, worker};
use anyhow::ensure;
use dustaway_core::*;
use dustaway_windows::uv_cache::UvReport;

#[tauri::command]
pub async fn prepare_uv_cache(
    state: AppState<'_>,
    request: CommandRequest,
    session: String,
    preparation_id: Option<String>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "prepare_uv_cache", move |state| {
        let scanned = state.user.tool_scan(&session, "dev.uv")?;
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "清理任务进行中"
        );
        ensure!(
            !state
                .user
                .settings()
                .excluded_rules
                .iter()
                .any(|id| id == "dev.uv"),
            "uv 缓存已按设置排除"
        );
        let summary = state.user.summary();
        ensure!(summary.session_id == session, "扫描已变化，请重新扫描");
        let row = summary
            .rules
            .iter()
            .find(|r| r.rule_id == "dev.uv")
            .ok_or_else(|| anyhow::anyhow!("本次扫描未检查 uv"))?;
        ensure!(
            row.coverage.as_ref().is_some_and(|c| c.complete)
                && !row.scan_failed
                && row.phase == ScanPhase::Completed,
            "缓存读取不完整，请重新扫描"
        );
        ensure!(row.files > 0, "没有已发现的 uv 缓存");
        let root = scanned
            .tool
            .as_ref()
            .and_then(|t| t.target.as_deref())
            .ok_or_else(|| anyhow::anyhow!("无法确认实际缓存位置"))?;
        state.user.invalidate_plan()?;
        let plan = state.uv.prepare_scanned_cancellable(
            &preparation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            &session,
            root,
            state.user.settings().exclusions,
            scanned.tool.as_ref().unwrap(),
        )?;
        let mut bindings = state.tool_plans.lock().unwrap();
        bindings.retain(|_, (_, rule, _)| rule != "dev.uv");
        bindings.insert(plan.id.clone(), (session, "dev.uv".into(), scanned));
        Ok(plan)
    })
    .await
}
#[tauri::command]
pub async fn execute_uv_cache(
    state: AppState<'_>,
    request: CommandRequest,
    plan_id: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "execute_uv_cache", move |state| {
        let (session, rule, scanned) =
            crate::maintenance_commands::take_tool_binding(state, &plan_id)?;
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "清理任务进行中"
        );
        ensure!(
            !state
                .user
                .settings()
                .excluded_rules
                .iter()
                .any(|id| id == "dev.uv"),
            "uv 缓存已按设置排除"
        );
        state.user.invalidate_plan()?;
        state.analysis.mark_stale();
        let task = state.uv.start(
            &plan_id,
            &state.user.summary().session_id,
            &state.user.settings().exclusions,
        )?;
        state
            .user
            .mark_tool_started(&session, &rule, scanned.revision, &task)?;
        Ok(task)
    })
    .await
}
#[tauri::command]
pub async fn cancel_uv_cache(
    state: AppState<'_>,
    request: CommandRequest,
) -> ApiResult<ControlInfo> {
    controlled(state.inner().clone(), request, "cancel_uv_cache", |state| {
        state.uv.cancel();
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn get_uv_task(state: AppState<'_>) -> ApiResult<Option<serde_json::Value>> {
    Ok(state.uv.task().map(|report| {
        let mut value = serde_json::to_value(report).expect("uv report serialization");
        value["session"] = state.uv.task_session().into();
        value
    }))
}
#[tauri::command]
pub async fn get_uv_history(state: AppState<'_>) -> ApiResult<Vec<UvReport>> {
    let state = state.inner().clone();
    worker(move || state.uv.history()).await
}

#[tauri::command]
pub async fn cancel_uv_preparation(state: AppState<'_>, preparation_id: String) -> ApiResult<()> {
    state
        .uv
        .cancel_preparation(&preparation_id)
        .map_err(|e| ApiError::new("invalid_preparation", e.to_string()))
}
