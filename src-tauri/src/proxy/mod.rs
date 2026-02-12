pub mod certs;
pub mod config_proxy;
pub mod presence;
pub mod xmpp_proxy;

use tokio::sync::watch;

use crate::state::StealthMode;

pub struct ProxyHandle {
    pub shutdown_tx: watch::Sender<bool>,
    pub mode_tx: watch::Sender<StealthMode>,
}

/// Start the XMPP proxy with the given certs and remote server.
/// Returns a handle to control the proxy (shutdown, toggle stealth).
pub async fn start_proxy(
    remote_host: String,
    remote_port: u16,
    server_cert_pem: String,
    server_key_pem: String,
    ca_cert_pem: String,
    initial_mode: StealthMode,
) -> Result<ProxyHandle, String> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (mode_tx, mode_rx) = watch::channel(initial_mode);

    let config = xmpp_proxy::ProxyConfig {
        listen_addr: "127.0.0.1:5223".to_string(),
        remote_host,
        remote_port,
        server_cert_pem,
        server_key_pem,
        ca_cert_pem,
    };

    tokio::spawn(async move {
        if let Err(e) = xmpp_proxy::run_proxy(config, mode_rx, shutdown_rx).await {
            log::error!("Proxy exited with error: {e}");
        }
    });

    Ok(ProxyHandle {
        shutdown_tx,
        mode_tx,
    })
}
