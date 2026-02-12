use tauri::{AppHandle, Manager, State};

use crate::proxy;
use crate::proxy::certs;
use crate::state::{AppState, ProxyStatus, StatusInfo, StealthMode};

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
    let new_mode = match mode.as_str() {
        "online" => StealthMode::Online,
        _ => StealthMode::Offline,
    };
    inner.stealth_mode = new_mode.clone();

    // Update the proxy's mode channel in real-time
    if let Some(tx) = &inner.mode_tx {
        let _ = tx.send(new_mode);
    }

    StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    }
}

#[tauri::command]
pub async fn start_proxy(
    remote_host: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StatusInfo, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    let ca = certs::ensure_ca(&data_dir)?;
    let server = certs::generate_server_cert(&ca, &data_dir)?;

    let initial_mode = {
        let inner = state.inner.lock().unwrap();
        inner.stealth_mode.clone()
    };

    let handle = proxy::start_proxy(
        remote_host,
        5223,
        server.cert_pem,
        server.key_pem,
        ca.cert_pem,
        initial_mode,
    )
    .await?;

    let mut inner = state.inner.lock().unwrap();
    inner.proxy_status = ProxyStatus::Running;
    inner.mode_tx = Some(handle.mode_tx);
    inner.shutdown_tx = Some(handle.shutdown_tx);

    Ok(StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    })
}

#[tauri::command]
pub fn stop_proxy(state: State<'_, AppState>) -> StatusInfo {
    let mut inner = state.inner.lock().unwrap();

    if let Some(tx) = inner.shutdown_tx.take() {
        let _ = tx.send(true);
    }
    inner.mode_tx = None;
    inner.proxy_status = ProxyStatus::Idle;

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
