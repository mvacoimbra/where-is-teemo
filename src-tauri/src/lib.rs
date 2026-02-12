mod commands;
mod proxy;
mod riot;
mod state;

use state::AppState;
use tauri::Manager;

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
            commands::stop_proxy,
            commands::get_cert_status,
            commands::install_ca,
            commands::get_regions,
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            setup_certs(&data_dir);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_certs(data_dir: &std::path::Path) {
    match proxy::certs::ensure_ca(data_dir) {
        Ok(ca) => {
            log::info!("CA certificate ready");
            if let Err(e) = proxy::certs::generate_server_cert(&ca, data_dir) {
                log::error!("Failed to generate server cert: {e}");
            }
        }
        Err(e) => {
            log::error!("Failed to ensure CA: {e}");
        }
    }
}
