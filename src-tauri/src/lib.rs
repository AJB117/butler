mod cluster;
mod commands;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(cluster::ClusterState::demo())
        .invoke_handler(tauri::generate_handler![
            commands::list_sessions,
            commands::kill_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Butler");
}
