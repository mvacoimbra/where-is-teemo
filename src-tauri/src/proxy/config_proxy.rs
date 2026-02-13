use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::watch;

const RIOT_CONFIG_URL: &str = "https://clientconfig.rpg.riotgames.com";

pub struct ConfigProxyHandle {
    pub port: u16,
    pub shutdown_tx: watch::Sender<bool>,
    /// The real chat host extracted from the Riot config.
    pub chat_host_rx: watch::Receiver<Option<String>>,
}

struct ProxyState {
    chat_port: u16,
    chat_host_tx: watch::Sender<Option<String>>,
    http_client: reqwest::Client,
}

/// Start a local HTTP server that proxies Riot client config requests.
/// Replaces chat.host with 127.0.0.1 and chat.port with our proxy port.
pub async fn start_config_proxy(chat_port: u16) -> Result<ConfigProxyHandle, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("Failed to bind config proxy: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("Failed to get local addr: {e}"))?
        .port();

    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let (chat_host_tx, chat_host_rx) = watch::channel(None);

    let http_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(15))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .unwrap();

    let state = Arc::new(ProxyState {
        chat_port,
        chat_host_tx,
        http_client,
    });

    tokio::spawn(async move {
        log::info!("Config proxy listening on 127.0.0.1:{port}");

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    let (stream, _addr) = match accept_result {
                        Ok(v) => v,
                        Err(e) => {
                            log::error!("Config proxy accept failed: {e}");
                            continue;
                        }
                    };

                    let state = state.clone();
                    let io = TokioIo::new(stream);

                    tokio::spawn(async move {
                        let svc = service_fn(move |req| {
                            let state = state.clone();
                            async move { handle_request(req, &state).await }
                        });

                        if let Err(e) = http1::Builder::new()
                            .serve_connection(io, svc)
                            .await
                        {
                            log::error!("Config proxy connection error: {e}");
                        }
                    });
                }
                _ = shutdown_rx.changed() => {
                    log::info!("Config proxy shutting down");
                    break;
                }
            }
        }
    });

    Ok(ConfigProxyHandle {
        port,
        shutdown_tx,
        chat_host_rx,
    })
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
    state: &ProxyState,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // Build upstream URL preserving path AND query string
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let upstream_url = format!("{RIOT_CONFIG_URL}{path_and_query}");

    log::info!("Config proxy: {} {path_and_query}", req.method());

    let mut upstream_req = state.http_client.get(&upstream_url);

    // Forward only the headers Riot needs (matching Deceive's behavior)
    for header in ["user-agent", "x-riot-entitlements-jwt", "authorization"] {
        if let Some(val) = req.headers().get(header) {
            upstream_req = upstream_req.header(header, val);
        }
    }

    let response = match upstream_req.send().await {
        Ok(resp) => resp,
        Err(e) => {
            log::error!("Config proxy upstream failed: {e}");
            return Ok(Response::builder()
                .status(502)
                .body(Full::new(Bytes::from(format!("Upstream error: {e}"))))
                .unwrap());
        }
    };

    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    log::debug!("Config proxy upstream response: {status} ({content_type})");

    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            log::error!("Config proxy failed to read upstream body: {e}");
            return Ok(Response::builder()
                .status(502)
                .body(Full::new(Bytes::from(format!("Body read error: {e}"))))
                .unwrap());
        }
    };

    // Only patch JSON responses that contain chat config keys
    let final_body = if content_type.contains("json") {
        match patch_config(&body, state) {
            Some(patched) => patched,
            None => body,
        }
    } else {
        body
    };

    Ok(Response::builder()
        .status(status.as_u16())
        .header("content-type", &content_type)
        .body(Full::new(Bytes::from(final_body)))
        .unwrap())
}

fn patch_config(body: &str, state: &ProxyState) -> Option<String> {
    let mut config: serde_json::Value = serde_json::from_str(body).ok()?;
    let obj = config.as_object_mut()?;

    // Only patch if this response actually has chat config
    let has_chat_config = obj.contains_key("chat.host")
        || obj.contains_key("chat.port")
        || obj.contains_key("chat.affinities");

    if !has_chat_config {
        return None;
    }

    // Extract and replace chat.host
    if let Some(host_val) = obj.get("chat.host") {
        if let Some(host) = host_val.as_str() {
            let real_host = host.to_string();
            log::info!("Detected real chat host: {real_host}");
            let _ = state.chat_host_tx.send(Some(real_host));
        }
        obj.insert(
            "chat.host".to_string(),
            serde_json::Value::String("127.0.0.1".to_string()),
        );
    }

    // Replace chat.port
    if obj.contains_key("chat.port") {
        obj.insert(
            "chat.port".to_string(),
            serde_json::Value::Number(state.chat_port.into()),
        );
    }

    // Replace all chat.affinities with localhost
    if let Some(affinities) = obj.get_mut("chat.affinities") {
        if let Some(aff_obj) = affinities.as_object_mut() {
            for (_key, val) in aff_obj.iter_mut() {
                *val = serde_json::Value::String("127.0.0.1".to_string());
            }
        }
    }

    // Allow bad certs (only when we're patching chat config)
    obj.insert(
        "chat.allow_bad_cert.enabled".to_string(),
        serde_json::Value::Bool(true),
    );

    serde_json::to_string(&config).ok()
}
