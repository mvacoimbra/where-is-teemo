use tauri::{AppHandle, Manager, State};

use crate::proxy;
use crate::proxy::certs;
use crate::proxy::config_proxy;
use crate::riot;
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
    log::info!("Stealth mode changed: {:?} → {:?}", inner.stealth_mode, new_mode);
    inner.stealth_mode = new_mode.clone();

    if let Some(tx) = &inner.mode_tx {
        let _ = tx.send(new_mode);
    } else {
        log::warn!("No mode channel — proxy not running, mode change won't take effect until next launch");
    }

    StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    }
}

/// Full launch flow: kill existing → start config proxy → start XMPP proxy → launch game.
#[tauri::command]
pub async fn launch_game(
    game: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StatusInfo, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {e}"))?;

    // 1. Kill existing Riot processes
    if riot::process::is_riot_running() {
        log::info!("Killing existing Riot processes");
        riot::process::kill_riot_processes()?;
    }

    // 2. Ensure certs are ready
    let ca = certs::ensure_ca(&data_dir)?;
    let server = certs::generate_server_cert(&ca, &data_dir)?;

    // 3. Start config proxy (intercepts Riot config, redirects chat to localhost)
    let config_handle = config_proxy::start_config_proxy(5223).await?;
    let config_port = config_handle.port;
    let chat_host_rx = config_handle.chat_host_rx;

    // 4. Start XMPP proxy (we'll use a default host, updated when config is fetched)
    let initial_mode = {
        let inner = state.inner.lock().unwrap();
        inner.stealth_mode.clone()
    };

    // Use selected region's chat host, or default
    let chat_host = {
        let inner = state.inner.lock().unwrap();
        inner.detected_chat_host.clone()
    }
    .unwrap_or_else(|| "na2.chat.si.riotgames.com".to_string());

    log::info!("Using chat host: {chat_host}");

    let proxy_handle = proxy::start_proxy(
        chat_host,
        5223,
        server.cert_pem,
        server.key_pem,
        ca.cert_pem,
        initial_mode,
    )
    .await?;

    // 5. Launch the game with our config proxy
    log::info!("Launching game '{game}' via config proxy on port {config_port}");
    if let Err(e) = riot::process::launch_riot_client(&game, config_port) {
        log::error!("Failed to launch game: {e}");
        // Clean up proxies since launch failed
        let _ = proxy_handle.shutdown_tx.send(true);
        let _ = config_handle.shutdown_tx.send(true);
        return Err(e);
    }

    // 6. Update state
    {
        let mut inner = state.inner.lock().unwrap();
        inner.proxy_status = ProxyStatus::Running;
        inner.connected_game = Some(game);
        inner.mode_tx = Some(proxy_handle.mode_tx);
        inner.shutdown_tx = Some(proxy_handle.shutdown_tx);
        inner.config_shutdown_tx = Some(config_handle.shutdown_tx);
    }

    // 7. Spawn a task to update XMPP proxy target once real chat host is discovered
    let host_tx = proxy_handle.host_tx;
    tokio::spawn(async move {
        let mut rx = chat_host_rx;
        while rx.changed().await.is_ok() {
            if let Some(host) = rx.borrow().clone() {
                log::info!("Real chat host discovered: {host} — updating XMPP proxy target");
                let _ = host_tx.send(host);
                break;
            }
        }
    });

    let inner = state.inner.lock().unwrap();
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
    if let Some(tx) = inner.config_shutdown_tx.take() {
        let _ = tx.send(true);
    }
    inner.mode_tx = None;
    inner.proxy_status = ProxyStatus::Idle;
    inner.connected_game = None;

    StatusInfo {
        stealth_mode: inner.stealth_mode.clone(),
        proxy_status: inner.proxy_status.clone(),
        connected_game: inner.connected_game.clone(),
    }
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

#[tauri::command]
pub fn get_regions() -> Vec<RegionInfo> {
    riot::config::REGIONS
        .iter()
        .map(|(code, name)| RegionInfo {
            code: code.to_string(),
            name: name.to_string(),
        })
        .collect()
}

#[tauri::command]
pub fn set_region(region: String, state: State<'_, AppState>) -> Result<(), String> {
    let chat_host = riot::config::chat_server_for_region(&region)
        .ok_or_else(|| format!("Unknown region: {region}"))?;

    let mut inner = state.inner.lock().unwrap();
    inner.detected_region = Some(region);
    inner.detected_chat_host = Some(chat_host.to_string());
    Ok(())
}

#[derive(serde::Serialize)]
pub struct CertStatus {
    pub ca_generated: bool,
    pub server_generated: bool,
    pub ca_trusted: bool,
}

#[derive(serde::Serialize)]
pub struct RegionInfo {
    pub code: String,
    pub name: String,
}
