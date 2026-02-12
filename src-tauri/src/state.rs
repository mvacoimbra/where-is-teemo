use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StealthMode {
    Online,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProxyStatus {
    Idle,
    Running,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub stealth_mode: StealthMode,
    pub proxy_status: ProxyStatus,
    pub connected_game: Option<String>,
}

pub struct AppState {
    pub inner: Mutex<AppStateInner>,
}

pub struct AppStateInner {
    pub stealth_mode: StealthMode,
    pub proxy_status: ProxyStatus,
    pub connected_game: Option<String>,
    pub detected_region: Option<String>,
    pub detected_chat_host: Option<String>,
    pub mode_tx: Option<watch::Sender<StealthMode>>,
    pub shutdown_tx: Option<watch::Sender<bool>>,
    pub config_shutdown_tx: Option<watch::Sender<bool>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(AppStateInner {
                stealth_mode: StealthMode::Offline,
                proxy_status: ProxyStatus::Idle,
                connected_game: None,
                detected_region: None,
                detected_chat_host: None,
                mode_tx: None,
                shutdown_tx: None,
                config_shutdown_tx: None,
            }),
        }
    }
}
