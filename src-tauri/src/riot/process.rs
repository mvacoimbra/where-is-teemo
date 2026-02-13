use std::path::PathBuf;
use sysinfo::System;

const RIOT_PROCESS_NAMES: &[&str] = &[
    "RiotClientServices",
    "LeagueClient",
    "VALORANT-Win64-Shipping",
    "Riot Client",
];

/// Check if any Riot-related process is currently running.
pub fn is_riot_running() -> bool {
    let s = System::new_all();
    s.processes().values().any(|p| {
        let name = p.name().to_string_lossy();
        RIOT_PROCESS_NAMES
            .iter()
            .any(|rn| name.contains(rn))
    })
}

/// Kill all running Riot client processes.
pub fn kill_riot_processes() -> Result<(), String> {
    let s = System::new_all();
    let mut killed = 0;

    for process in s.processes().values() {
        let name = process.name().to_string_lossy();
        if RIOT_PROCESS_NAMES.iter().any(|rn| name.contains(rn)) {
            log::info!("Killing process: {} (PID {})", name, process.pid());
            process.kill();
            killed += 1;
        }
    }

    if killed > 0 {
        log::info!("Killed {killed} Riot process(es)");
        // Give processes time to clean up
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(())
}

/// Find the Riot Client executable path.
pub fn find_riot_client() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        find_riot_client_macos()
    }

    #[cfg(target_os = "windows")]
    {
        find_riot_client_windows()
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn find_riot_client_macos() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("/Applications/Riot Client.app/Contents/MacOS/RiotClientServices"),
        PathBuf::from("/Users/Shared/Riot Games/Riot Client.app/Contents/MacOS/RiotClientServices"),
    ];

    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join("Applications/Riot Client.app/Contents/MacOS/RiotClientServices"),
        );
    }

    for path in &candidates {
        log::debug!("Checking Riot Client path: {}", path.display());
        if path.exists() {
            log::info!("Found Riot Client at: {}", path.display());
            return Some(path.clone());
        }
    }

    // Try to find via RiotClientInstalls.json
    log::debug!("Checking RiotClientInstalls.json");
    find_from_installs_json()
}

#[cfg(target_os = "windows")]
fn find_riot_client_windows() -> Option<PathBuf> {
    // Check RiotClientInstalls.json first (most reliable)
    if let Some(path) = find_from_installs_json() {
        return Some(path);
    }

    // Fallback: check common install paths
    let candidates = [
        "C:\\Riot Games\\Riot Client\\RiotClientServices.exe",
        "D:\\Riot Games\\Riot Client\\RiotClientServices.exe",
    ];

    for path_str in &candidates {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn find_from_installs_json() -> Option<PathBuf> {
    let installs_path = get_installs_json_path()?;
    let content = std::fs::read_to_string(&installs_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try keys in priority order
    for key in &["rc_live", "rc_default", "rc_beta"] {
        if let Some(path_str) = json.get(key).and_then(|v| v.as_str()) {
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

fn get_installs_json_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // %ProgramData%\Riot Games\RiotClientInstalls.json
        std::env::var("ProgramData").ok().map(|pd| {
            PathBuf::from(pd)
                .join("Riot Games")
                .join("RiotClientInstalls.json")
        })
    }

    #[cfg(target_os = "macos")]
    {
        // ~/Library/Application Support/Riot Games/RiotClientInstalls.json
        dirs::home_dir().map(|h| {
            h.join("Library/Application Support/Riot Games/RiotClientInstalls.json")
        })
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Launch the Riot Client with a specific game and config proxy URL.
pub fn launch_riot_client(
    game: &str,
    config_proxy_port: u16,
) -> Result<(), String> {
    let client_path = find_riot_client().ok_or_else(|| {
        log::error!("Riot Client not found at any known path");
        "Riot Client not found. Is it installed?".to_string()
    })?;

    let config_url = format!("http://127.0.0.1:{config_proxy_port}");

    let launch_product = match game {
        "league_of_legends" => "--launch-product=league_of_legends",
        "valorant" => "--launch-product=valorant",
        _ => return Err(format!("Unknown game: {game}")),
    };

    log::info!(
        "Launching Riot Client: {:?} --client-config-url=\"{config_url}\" {launch_product}",
        client_path
    );

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args([
                "-a",
                client_path.to_str().unwrap_or_default(),
                "--args",
                &format!("--client-config-url={config_url}"),
                launch_product,
                "--launch-patchline=live",
            ])
            .spawn()
            .map_err(|e| format!("Failed to launch Riot Client: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new(&client_path)
            .args([
                &format!("--client-config-url=\"{config_url}\""),
                launch_product,
                "--launch-patchline=live",
            ])
            .spawn()
            .map_err(|e| format!("Failed to launch Riot Client: {e}"))?;
    }

    Ok(())
}

/// Add the `dirs` crate dependency for home_dir
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }

        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok().map(PathBuf::from)
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            std::env::var("HOME").ok().map(PathBuf::from)
        }
    }
}
