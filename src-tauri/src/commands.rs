use tauri::State;

use crate::{
    cluster::ClusterState,
    models::{BackendStatus, OpenSessionResult, Session},
};

#[tauri::command]
pub async fn backend_status(
    state: State<'_, ClusterState>,
) -> Result<BackendStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.status())
        .await
        .map_err(|error| format!("The backend status task failed: {error}"))
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, ClusterState>,
) -> Result<Vec<Session>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_sessions())
        .await
        .map_err(|error| format!("The session discovery task failed: {error}"))?
}

#[tauri::command]
pub async fn open_session(
    session_id: String,
    state: State<'_, ClusterState>,
) -> Result<OpenSessionResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.open_session(&session_id))
        .await
        .map_err(|error| format!("The session opening task failed: {error}"))?
}

#[tauri::command]
pub async fn close_session(
    session_id: String,
    state: State<'_, ClusterState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.close_session(&session_id))
        .await
        .map_err(|error| format!("The session closing task failed: {error}"))?
}

#[tauri::command]
pub async fn kill_session(
    session_id: String,
    state: State<'_, ClusterState>,
) -> Result<Vec<Session>, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.kill_session(&session_id))
        .await
        .map_err(|error| format!("The session cancellation task failed: {error}"))?
}
