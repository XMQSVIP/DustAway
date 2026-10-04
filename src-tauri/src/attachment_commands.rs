use crate::commands::{ApiResult, AppState, controlled, worker};
use anyhow::ensure;
use dustaway_core::*;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn get_attachment_locations(
    state: AppState<'_>,
    provider: AttachmentProvider,
) -> ApiResult<AttachmentDiscovery> {
    let state = state.inner().clone();
    worker(move || Ok(state.attachments.discover(provider))).await
}
#[tauri::command]
pub async fn pick_attachment_location(
    app: tauri::AppHandle,
    state: AppState<'_>,
    provider: AttachmentProvider,
) -> ApiResult<Option<AttachmentLocation>> {
    let state = state.inner().clone();
    worker(move || {
        app.dialog()
            .file()
            .set_title(format!("选择{}账号资料目录", provider.title()))
            .blocking_pick_folder()
            .map(|p| {
                state
                    .attachments
                    .register(provider, &p.into_path()?.to_string_lossy())
            })
            .transpose()
    })
    .await
}
#[tauri::command]
pub async fn start_attachment_scan(
    state: AppState<'_>,
    request: CommandRequest,
    token: String,
) -> ApiResult<ControlInfo> {
    controlled(
        state.inner().clone(),
        request,
        "start_attachment_scan",
        move |state| {
            ensure!(
                !state.user.is_busy() && !state.batch_active(),
                "清理任务进行中"
            );
            state.user.invalidate_plan()?;
            state
                .attachments
                .start(&token, state.user.settings().exclusions)
        },
    )
    .await
}
#[tauri::command]
pub async fn get_attachment_snapshot(state: AppState<'_>) -> ApiResult<Option<AttachmentSnapshot>> {
    Ok(state.attachments.snapshot())
}
#[tauri::command]
pub async fn list_attachments(
    state: AppState<'_>,
    query: AttachmentQuery,
) -> ApiResult<AttachmentPage> {
    let state = state.inner().clone();
    worker(move || state.attachments.page(query)).await
}
#[tauri::command]
pub async fn get_attachment_thumbnail(
    state: AppState<'_>,
    session: String,
    id: String,
) -> ApiResult<String> {
    let state = state.inner().clone();
    worker(move || state.attachments.thumbnail(&session, &id)).await
}
#[tauri::command]
pub async fn reveal_attachment(state: AppState<'_>, session: String, id: String) -> ApiResult<()> {
    let state = state.inner().clone();
    worker(move || state.attachments.reveal(&session, &id)).await
}
#[tauri::command]
pub async fn cancel_attachment_scan(state: AppState<'_>) -> ApiResult<()> {
    state.attachments.cancel();
    Ok(())
}
#[tauri::command]
pub async fn close_attachment_session(state: AppState<'_>) -> ApiResult<()> {
    let state = state.inner().clone();
    worker(move || {
        let _control = state.control.lock().unwrap();
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "请先等待清理结束"
        );
        state.user.invalidate_plan()?;
        state.attachments.close()
    })
    .await
}

#[tauri::command]
pub async fn cancel_attachment_plan(state: AppState<'_>) -> ApiResult<()> {
    let state = state.inner().clone();
    worker(move || {
        let _control = state.control.lock().unwrap();
        ensure!(
            !state.user.is_busy() && !state.batch_active(),
            "清理正在进行"
        );
        state.user.invalidate_plan()
    })
    .await
}
