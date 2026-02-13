use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio_rustls::{TlsAcceptor, TlsConnector};

use crate::proxy::presence;
use crate::state::StealthMode;

pub struct ProxyConfig {
    pub listen_addr: String,
    pub remote_port: u16,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    #[allow(dead_code)]
    pub ca_cert_pem: String,
}

/// Start the XMPP TLS proxy. Blocks until the shutdown signal is received.
pub async fn run_proxy(
    config: ProxyConfig,
    host_rx: watch::Receiver<String>,
    mode_rx: watch::Receiver<StealthMode>,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<(), String> {
    let tls_acceptor = build_tls_acceptor(&config)?;
    let tls_connector = build_tls_connector(&config)?;
    let remote_port = config.remote_port;

    let listener = TcpListener::bind(&config.listen_addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {e}", config.listen_addr))?;

    log::info!("XMPP proxy listening on {}", config.listen_addr);

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (tcp_stream, peer_addr) = match accept_result {
                    Ok(v) => v,
                    Err(e) => {
                        log::error!("Accept failed: {e}");
                        continue;
                    }
                };

                log::info!("New connection from {peer_addr}");

                let acceptor = tls_acceptor.clone();
                let connector = tls_connector.clone();
                let host = host_rx.borrow().clone();
                let mode = mode_rx.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(
                        tcp_stream, acceptor, connector, &host, remote_port, mode,
                    ).await {
                        log::error!("Connection from {peer_addr} ended with error: {e}");
                    } else {
                        log::info!("Connection from {peer_addr} closed cleanly");
                    }
                });
            }
            _ = shutdown_rx.changed() => {
                log::info!("Proxy received shutdown signal");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_connection(
    tcp_stream: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    connector: TlsConnector,
    remote_host: &str,
    remote_port: u16,
    mut mode_rx: watch::Receiver<StealthMode>,
) -> Result<(), String> {
    // Accept TLS from Riot client
    let client_tls = acceptor
        .accept(tcp_stream)
        .await
        .map_err(|e| format!("TLS accept failed: {e}"))?;

    // Connect to real Riot chat server
    let remote_addr = format!("{remote_host}:{remote_port}");
    let remote_tcp = tokio::net::TcpStream::connect(&remote_addr)
        .await
        .map_err(|e| format!("Failed to connect to {remote_addr}: {e}"))?;

    let server_name = ServerName::try_from(remote_host.to_string())
        .map_err(|e| format!("Invalid server name '{remote_host}': {e}"))?;

    let server_tls = connector
        .connect(server_name, remote_tcp)
        .await
        .map_err(|e| format!("TLS connect to {remote_addr} failed: {e}"))?;

    log::info!("TLS tunnel established to {remote_addr}");

    // Split both connections for bidirectional forwarding
    let (mut client_read, mut client_write) = tokio::io::split(client_tls);
    let (mut server_read, mut server_write) = tokio::io::split(server_tls);

    // Server → Client: pass through unmodified
    let server_to_client = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = match server_read.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    log::error!("Read from server failed: {e}");
                    break;
                }
            };
            let preview: String = String::from_utf8_lossy(&buf[..n]).chars().take(120).collect();
            log::debug!("S→C: {preview}");
            if let Err(e) = client_write.write_all(&buf[..n]).await {
                log::error!("Write to client failed: {e}");
                break;
            }
        }
    });

    // Client → Server: filter presence stanzas + inject on mode toggle
    let client_to_server = tokio::spawn(async move {
        let mut buf = vec![0u8; 8192];
        let mut stanza_buf = String::new();
        let mut last_presence = String::new();
        let mut watch_mode = true;

        loop {
            tokio::select! {
                result = client_read.read(&mut buf) => {
                    let n = match result {
                        Ok(0) => break,
                        Ok(n) => n,
                        Err(e) => {
                            log::error!("Read from client failed: {e}");
                            break;
                        }
                    };

                    stanza_buf.push_str(&String::from_utf8_lossy(&buf[..n]));

                    while let Some(end) = presence::find_stanza_end(&stanza_buf) {
                        let stanza: String = stanza_buf.drain(..end).collect();

                        // Cache raw presence before filtering (skip unavailable ones)
                        if stanza.trim_start().starts_with("<presence")
                            && !stanza.contains("type=\"unavailable\"")
                        {
                            last_presence = stanza.clone();
                        }

                        let mode = mode_rx.borrow().clone();
                        let filtered = presence::filter_outgoing(&stanza, &mode);

                        let preview: String = filtered.chars().take(120).collect();
                        log::debug!("C→S: {preview}");

                        if let Err(e) = server_write.write_all(filtered.as_bytes()).await {
                            log::error!("Write to server failed: {e}");
                            return;
                        }
                    }
                }
                result = mode_rx.changed(), if watch_mode => {
                    if result.is_err() {
                        watch_mode = false;
                        continue;
                    }

                    let mode = mode_rx.borrow().clone();
                    let inject = match mode {
                        StealthMode::Offline => {
                            log::info!("Mode → Offline: injecting unavailable presence");
                            r#"<presence type="unavailable"/>"#.to_string()
                        }
                        StealthMode::Online => {
                            if last_presence.is_empty() {
                                log::info!("Mode → Online: injecting basic available presence");
                                "<presence/>".to_string()
                            } else {
                                log::info!("Mode → Online: re-sending last cached presence");
                                last_presence.clone()
                            }
                        }
                    };

                    log::debug!("Injected: {}", inject.chars().take(120).collect::<String>());

                    if let Err(e) = server_write.write_all(inject.as_bytes()).await {
                        log::error!("Write to server (inject) failed: {e}");
                        return;
                    }
                }
            }
        }

        // Flush remaining buffer (partial data at disconnect)
        if !stanza_buf.is_empty() {
            let _ = server_write.write_all(stanza_buf.as_bytes()).await;
        }
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = server_to_client => {},
        _ = client_to_server => {},
    }

    Ok(())
}

fn build_tls_acceptor(config: &ProxyConfig) -> Result<TlsAcceptor, String> {
    let certs = load_certs_from_pem(&config.server_cert_pem)?;
    let key = load_key_from_pem(&config.server_key_pem)?;

    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("Failed to build TLS server config: {e}"))?;

    Ok(TlsAcceptor::from(Arc::new(server_config)))
}

fn build_tls_connector(_config: &ProxyConfig) -> Result<TlsConnector, String> {
    // We connect to the real Riot server — use system roots
    let mut root_store = RootCertStore::empty();

    // Add system root certificates
    let native = rustls_native_certs::load_native_certs();
    for cert in native.certs {
        root_store.add(cert).ok();
    }

    let client_config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(TlsConnector::from(Arc::new(client_config)))
}

fn load_certs_from_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let mut reader = std::io::Cursor::new(pem);
    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to parse certificate PEM: {e}"))
}

fn load_key_from_pem(pem: &str) -> Result<PrivateKeyDer<'static>, String> {
    let mut reader = std::io::Cursor::new(pem);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| format!("Failed to parse key PEM: {e}"))?
        .ok_or_else(|| "No private key found in PEM".to_string())
}
