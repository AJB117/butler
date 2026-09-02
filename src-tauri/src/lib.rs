mod cluster;
mod commands;
mod config;
mod models;
mod ssh;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            app.manage(cluster::ClusterState::load(config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_status,
            commands::list_sessions,
            commands::open_session,
            commands::close_session,
            commands::kill_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Butler");
}
