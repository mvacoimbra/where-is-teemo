use tauri::{AppHandle, Manager, State};

use crate::proxy::certs;
use crate::state::{AppState, StatusInfo, StealthMode};

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> StatusInfo {
    let inner = state.inner.lock().unwrap();
    StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    }
}

#[tauri::command]
pub fn set_stealth_mode(mode: String, state: State<'_, AppState>) -> StatusInfo {
    let mut inner = state.inner.lock().unwrap();
    inner.stealth_mode = match mode.as_str() {
        "online" => StealthMode::Online,
        _ => StealthMode::Offline,
    };
    StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    }
}

#[tauri::command]
pub fn launch_game(game: String, _state: State<'_, AppState>) -> Result<String, String> {
    // TODO: Phase 4 — actually launch the game with proxy
    Ok(format!("Would launch: {game}"))
}

#[tauri::command]
pub fn get_cert_status(app: AppHandle) -> Result<CertStatus, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    let ca_exists = data_dir.join("certs").join("ca.pem").exists();
    let server_exists = data_dir.join("certs").join("server.pem").exists();
    let ca_trusted = certs::is_ca_installed(&data_dir);

    Ok(CertStatus {
        ca_generated: ca_exists,
        server_generated: server_exists,
        ca_trusted,
    })
}

#[tauri::command]
pub fn install_ca(app: AppHandle) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    certs::install_ca_system(&data_dir)
}

#[derive(serde::Serialize)]
pub struct CertStatus {
    pub ca_generated: bool,
    pub server_generated: bool,
    pub ca_trusted: bool,
}
