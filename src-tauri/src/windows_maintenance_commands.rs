use crate::commands::{ApiResult, AppState, controlled, worker};
use anyhow::ensure;
use dustaway_core::*;
use dustaway_windows::{
    ipc::Request,
    system_maintenance::{WindowsReport, WindowsSnapshot},
};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowsPlanRef {
    pub scope: Scope,
    pub plan_id: String,
}

#[tauri::command]
pub async fn scan_windows_maintenance(
    state: AppState<'_>,
    request: CommandRequest,
    scope: Scope,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "scan_windows_maintenance", move |state| {
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "请先等待文件扫描或清理完成"
        );
        crate::require_scope(scope)?;
        match scope {
            Scope::User => state.windows_maintenance.scan(
                dustaway_windows::system_maintenance::settings_exclusions(&state.user.settings()),
            ),
            Scope::System => {
                state.launch_system()?;
                state.system(Request::WindowsScan)
            }
        }
    })
    .await
}
#[tauri::command]
pub async fn get_windows_maintenance(
    state: AppState<'_>,
    scope: Scope,
) -> ApiResult<Option<WindowsSnapshot>> {
    let state = state.inner().clone();
    worker(move || match scope {
        Scope::User => Ok(Some(state.windows_maintenance.snapshot())),
        Scope::System => {
            crate::require_scope(scope)?;
            if state.helper.lock().unwrap().is_none() {
                Ok(None)
            } else {
                state.system(Request::WindowsSnapshot).map(Some)
            }
        }
    })
    .await
}
#[tauri::command]
pub async fn prepare_windows_maintenance(
    state: AppState<'_>,
    request: CommandRequest,
    scope: Scope,
    session: String,
    ids: Vec<String>,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(
        state,
        request,
        "prepare_windows_maintenance",
        move |state| {
            ensure!(
                !state.user.is_busy() && !state.batch_active(),
                "清理任务进行中"
            );
            match scope {
                Scope::User => state.windows_maintenance.prepare(
                    session,
                    ids,
                    dustaway_windows::system_maintenance::settings_exclusions(
                        &state.user.settings(),
                    ),
                ),
                Scope::System => state.system(Request::WindowsPrepare { session, ids }),
            }
        },
    )
    .await
}
#[tauri::command]
pub async fn cancel_windows_maintenance(
    state: AppState<'_>,
    request: CommandRequest,
) -> ApiResult<ControlInfo> {
    let state = state.inner().clone();
    controlled(state, request, "cancel_windows_maintenance", move |state| {
        state.cancel_all();
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn get_windows_maintenance_history(
    state: AppState<'_>,
    scope: Scope,
) -> ApiResult<Vec<WindowsReport>> {
    let state = state.inner().clone();
    worker(move || match scope {
        Scope::User => state.windows_maintenance.history(),
        Scope::System => state.system(Request::WindowsHistory),
    })
    .await
}
