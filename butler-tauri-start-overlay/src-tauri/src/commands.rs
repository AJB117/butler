use tauri::State;

use crate::{cluster::ClusterState, models::Session};

#[tauri::command]
pub fn list_sessions(state: State<'_, ClusterState>) -> Result<Vec<Session>, String> {
    state.list_sessions()
}

#[tauri::command]
pub fn kill_session(
    session_id: String,
    state: State<'_, ClusterState>,
) -> Result<Vec<Session>, String> {
    state.kill_session(&session_id)?;
    state.list_sessions()
}
