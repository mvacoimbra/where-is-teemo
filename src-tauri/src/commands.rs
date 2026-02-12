use tauri::State;

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
