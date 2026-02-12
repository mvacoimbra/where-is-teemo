mod commands;
mod proxy;
mod riot;
mod state;

use state::AppState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    log::info!("Where Is Teemo starting");

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
            commands::set_region,
        ])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            setup_certs(&data_dir);
            setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
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

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let offline_item = MenuItem::with_id(app, "offline", "Invisible", true, None::<&str>)?;
    let online_item = MenuItem::with_id(app, "online", "Online", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let show_item = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &offline_item,
            &online_item,
            &separator,
            &show_item,
            &quit_item,
        ],
    )?;

    TrayIconBuilder::new()
        .tooltip("Where Is Teemo")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "offline" => {
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                inner.stealth_mode = state::StealthMode::Offline;
                if let Some(tx) = &inner.mode_tx {
                    let _ = tx.send(state::StealthMode::Offline);
                }
                log::info!("Stealth mode: Invisible (via tray)");
            }
            "online" => {
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                inner.stealth_mode = state::StealthMode::Online;
                if let Some(tx) = &inner.mode_tx {
                    let _ = tx.send(state::StealthMode::Online);
                }
                log::info!("Stealth mode: Online (via tray)");
            }
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                log::info!("Quit requested — cleaning up");
                let state = app.state::<AppState>();
                let mut inner = state.inner.lock().unwrap();
                if let Some(tx) = inner.shutdown_tx.take() {
                    let _ = tx.send(true);
                }
                if let Some(tx) = inner.config_shutdown_tx.take() {
                    let _ = tx.send(true);
                }
                drop(inner);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
