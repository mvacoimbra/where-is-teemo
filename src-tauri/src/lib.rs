mod commands;
mod state;
mod proxy;
mod riot;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = AppState::default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_stealth_mode,
            commands::launch_game,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
