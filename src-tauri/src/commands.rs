use tauri::State;

use crate::{
    cluster::ClusterState,
    models::{BackendStatus, OpenSessionResult, ProjectSnapshot, Session},
    projects::ProjectState,
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

#[tauri::command]
pub async fn project_snapshot(
    state: State<'_, ProjectState>,
) -> Result<ProjectSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.snapshot())
        .await
        .map_err(|error| format!("The project metadata task failed: {error}"))?
}

#[tauri::command]
pub async fn create_project(
    name: String,
    remote_path: Option<String>,
    state: State<'_, ProjectState>,
) -> Result<ProjectSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.create_project(name, remote_path))
        .await
        .map_err(|error| format!("The project creation task failed: {error}"))?
}

#[tauri::command]
pub async fn update_project(
    project_id: i64,
    name: String,
    remote_path: Option<String>,
    state: State<'_, ProjectState>,
) -> Result<ProjectSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.update_project(project_id, name, remote_path)
    })
    .await
    .map_err(|error| format!("The project update task failed: {error}"))?
}

#[tauri::command]
pub async fn delete_project(
    project_id: i64,
    state: State<'_, ProjectState>,
) -> Result<ProjectSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_project(project_id))
        .await
        .map_err(|error| format!("The project deletion task failed: {error}"))?
}

#[tauri::command]
pub async fn assign_session_project(
    session_id: String,
    project_id: i64,
    state: State<'_, ProjectState>,
) -> Result<ProjectSnapshot, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.assign_session(session_id, project_id))
        .await
        .map_err(|error| format!("The session assignment task failed: {error}"))?
}
