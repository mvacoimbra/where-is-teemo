use serde::{Deserialize, Serialize};
use std::sync::Mutex;

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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(AppStateInner {
                stealth_mode: StealthMode::Offline,
                proxy_status: ProxyStatus::Idle,
                connected_game: None,
            }),
        }
    }
}
