mod cluster;
mod commands;
mod config;
mod models;
mod projects;
mod ssh;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let project_state = projects::ProjectState::load(&config_dir).map_err(|message| {
                std::io::Error::new(std::io::ErrorKind::Other, message)
            })?;
            app.manage(project_state);
            app.manage(cluster::ClusterState::load(config_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::backend_status,
            commands::list_sessions,
            commands::open_session,
            commands::close_session,
            commands::kill_session,
            commands::project_snapshot,
            commands::create_project,
            commands::update_project,
            commands::delete_project,
            commands::assign_session_project,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Butler");
}
