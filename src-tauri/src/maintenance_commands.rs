use crate::commands::{ApiResult, AppState, controlled, worker};
use anyhow::ensure;
use dustaway_core::*;
use dustaway_windows::maintenance::MaintenanceReport;

#[tauri::command]
pub async fn prepare_tool_cache(
    state: AppState<'_>,
    request: CommandRequest,
    session: String,
    rule_id: String,
    preparation_id: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "prepare_tool_cache", move |state| {
        let scanned = state.user.tool_scan(&session, &rule_id)?;
        state.maintenance.invalidate_plan();
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
                .any(|id| id == &rule_id),
            "工具缓存已按设置排除"
        );
        let summary = state.user.summary();
        ensure!(summary.session_id == session, "扫描已变化，请重新扫描");
        let row = summary
            .rules
            .iter()
            .find(|r| r.rule_id == rule_id)
            .ok_or_else(|| anyhow::anyhow!("本次扫描未检查该工具"))?;
        ensure!(
            row.coverage.as_ref().is_some_and(|c| c.complete)
                && !row.scan_failed
                && row.phase == ScanPhase::Completed,
            "缓存读取不完整，请重新扫描"
        );
        ensure!(row.files > 0, "没有已发现的 工具缓存");
        let mut roots = std::collections::BTreeSet::new();
        for offset in (0..row.files).step_by(200) {
            ensure!(
                !state.maintenance.preparation_cancelled(&preparation_id),
                ApiError::new("preparation_cancelled", "检查已取消")
            );
            for candidate in state
                .user
                .candidates(&session, &rule_id, offset, 200)?
                .items
            {
                if let Some(root) = candidate.root {
                    roots.insert(root);
                }
            }
        }
        let roots: Vec<_> = roots.into_iter().collect();
        state.user.invalidate_plan()?;
        let plan = state.maintenance.prepare_scanned_cancellable(
            &preparation_id,
            &session,
            &rule_id,
            &roots,
            state.user.settings().exclusions,
            scanned.tool.as_ref(),
        )?;
        let mut bindings = state.tool_plans.lock().unwrap();
        bindings.retain(|_, (_, rule, _)| rule != &rule_id);
        bindings.insert(plan.id.clone(), (session, rule_id, scanned));
        Ok(plan)
    })
    .await
}
#[tauri::command]
pub async fn execute_tool_cache(
    state: AppState<'_>,
    request: CommandRequest,
    plan_id: String,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "execute_tool_cache", move |state| {
        let (session, rule, scanned) = take_tool_binding(state, &plan_id)?;
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "清理任务进行中"
        );
        state.user.invalidate_plan()?;
        state.analysis.mark_stale();
        let task = state.maintenance.start(
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

pub(crate) fn take_tool_binding(
    state: &crate::Service,
    id: &str,
) -> anyhow::Result<(String, String, RuleSummary)> {
    let binding = state
        .tool_plans
        .lock()
        .unwrap()
        .remove(id)
        .ok_or_else(|| anyhow::anyhow!("确认已失效，请重新检查"))?;
    let current = state.user.tool_scan(&binding.0, &binding.1)?;
    ensure!(
        current.revision == binding.2.revision,
        "此项扫描已变化，确认已失效"
    );
    Ok(binding)
}

#[tauri::command]
pub async fn refresh_cleanup_rule(
    state: AppState<'_>,
    request: CommandRequest,
    session: String,
    rule_id: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "refresh_cleanup_rule",
        move |state| {
            ensure!(
                !state.user.is_busy() && !state.batch_active(),
                "已有任务正在运行"
            );
            state.maintenance.invalidate_plan();
            state.uv.invalidate_plan();
            state.tool_plans.lock().unwrap().clear();
            state.user.start_rule_refresh(&session, &rule_id)
        },
    )
    .await
}

#[tauri::command]
pub async fn cancel_tool_cache(
    state: AppState<'_>,
    request: CommandRequest,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "cancel_tool_cache",
        |state| {
            state.maintenance.cancel();
            Ok(())
        },
    )
    .await
}
#[tauri::command]
pub async fn get_tool_task(state: AppState<'_>) -> ApiResult<Option<MaintenanceReport>> {
    Ok(state.maintenance.task())
}
#[tauri::command]
pub async fn get_tool_history(state: AppState<'_>) -> ApiResult<Vec<MaintenanceReport>> {
    let state = state.inner().clone();
    worker(move || state.maintenance.history()).await
}

#[tauri::command]
pub async fn cancel_tool_preparation(state: AppState<'_>, preparation_id: String) -> ApiResult<()> {
    state
        .maintenance
        .cancel_preparation(&preparation_id)
        .map_err(|e| ApiError::new("invalid_preparation", e.to_string()))
}
