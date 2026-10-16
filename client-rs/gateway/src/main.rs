//! The browser client's server. Browsers can't open TCP sockets or (without CORS) call another
//! origin, so one port does it all:
//!
//! - `/`: the web build (`tools/build_client_rs.py --web`);
//! - `/api/<path>`: forwarded to the account server as `<account>/<path>` (POST and GET);
//! - `/ws/<host>:<port>`: a WebSocket whose binary messages are the bytes of a TCP connection to
//!   that world server. Only targets given with `--allow` are accepted, so it isn't an open proxy.
//!   `--allow <host:port>=<host:port>` accepts the first (the address the server list gives
//!   players) and connects to the second, as on a VPS where the gateway and the world server share
//!   the machine: `--allow play.example.com:2051=127.0.0.1:2051`.
//!
//!     bsk-gateway --web build/client-rs/web --account http://127.0.0.1:8080 --allow 127.0.0.1:2051
//!                 [--bind 127.0.0.1:8095] [--forward-address]
//!
//! It binds to 127.0.0.1 unless `--bind` says otherwise. The desktop client comes this way too
//! when its account address is a site's `/api` (https and wss through Caddy).
//!
//! `--forward-address`: each bridged connection starts with a PROXY_INFO message (250) carrying
//! the player's address (from `X-Forwarded-For` when the request comes from this machine, i.e.
//! Caddy), so the world server doesn't see everyone as 127.0.0.1 (IP bans, mutes). Only for a
//! world proxy (`tools/world_proxy.py`) that takes it in place of its own: an older one routes
//! such a connection to the active world server whatever its Hello asks for.

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use futures::{SinkExt, StreamExt};
use axum::extract::ConnectInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

struct Config {
    account: String,
    /// Target asked for -> address connected to.
    allowed: HashMap<String, String>,
    forward_address: bool,
    http: reqwest::Client,
}

/// Largest account request accepted from a browser.
const MAX_API_BODY: usize = 64 * 1024;

fn parse_args() -> Result<(String, String, Config), String> {
    let mut web = None;
    let mut bind = "127.0.0.1:8095".to_owned();
    let mut account = None;
    let mut allowed = HashMap::new();
    let mut forward_address = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--web" => web = Some(value()?),
            "--bind" => bind = value()?,
            "--account" => account = Some(value()?.trim_end_matches('/').to_owned()),
            "--allow" => {
                let v = value()?;
                let (target, to) = v.split_once('=').unwrap_or((&v, &v));
                allowed.insert(target.to_owned(), to.to_owned());
            }
            "--forward-address" => forward_address = true,
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let usage = "usage: bsk-gateway --web <dir> --account <url> --allow <host:port>[=<host:port>]... [--bind <addr:port>] [--forward-address]";
    let (Some(web), Some(account)) = (web, account) else { return Err(usage.into()) };
    if allowed.is_empty() {
        return Err(format!("no --allow world server given\n{usage}"));
    }
    Ok((web, bind, Config { account, allowed, forward_address, http: reqwest::Client::new() }))
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let (web, bind, config) = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    log::info!("serving {web} on http://{bind}, account server {}, world servers {:?}", config.account, config.allowed);
    let app = Router::new()
        .route("/ws/:target", get(bridge))
        .route("/api/*path", any(api))
        // The build's .gz copies when the browser takes gzip (build_client_rs.py --web makes them).
        .fallback_service(tower_http::services::ServeDir::new(web).precompressed_gzip())
        .layer(axum::middleware::from_fn(cache_headers))
        .with_state(Arc::new(config));
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| {
        eprintln!("{bind}: {e}");
        std::process::exit(1);
    });
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.expect("server");
}

/// Files asked for with the build in the query (`?v=`, every file the page and the game load)
/// never change: browsers keep them for good. Everything else (the page itself) is checked each
/// time, so a new build is picked up at once.
async fn cache_headers(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let path = req.uri().path();
    let skip = path.starts_with("/api/") || path.starts_with("/ws/");
    let versioned = req.uri().query().is_some_and(|q| q.split('&').any(|p| p.starts_with("v=")));
    let mut response = next.run(req).await;
    if !skip && response.status().is_success() {
        let value = if versioned { "public, max-age=31536000, immutable" } else { "no-cache" };
        response.headers_mut().insert(axum::http::header::CACHE_CONTROL, axum::http::HeaderValue::from_static(value));
    }
    response
}

async fn api(
    State(config): State<Arc<Config>>,
    Path(path): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if body.len() > MAX_API_BODY {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let query = uri.query().map(|q| format!("?{q}")).unwrap_or_default();
    let url = format!("{}/{}{}", config.account, path, query);
    let mut request = config.http.request(method, &url).body(body);
    if let Some(ct) = headers.get("content-type") {
        request = request.header("content-type", ct);
    }
    // The player's address, for the account server's limits: the one the proxy in front (Caddy)
    // forwarded, else the peer's own, so a player reaching the gateway directly can't pick one.
    request = request.header("x-forwarded-for", player_address(peer, &headers));
    match request.send().await {
        Ok(r) => {
            let status = StatusCode::from_u16(r.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let content_type = r.headers().get("content-type").cloned();
            let bytes = r.bytes().await.unwrap_or_default();
            let mut response = (status, bytes).into_response();
            if let Some(ct) = content_type.and_then(|v| v.to_str().ok().map(str::to_owned)) {
                if let Ok(v) = ct.parse() {
                    response.headers_mut().insert("content-type", v);
                }
            }
            response
        }
        Err(e) => {
            log::warn!("{url}: {e}");
            (StatusCode::BAD_GATEWAY, "account server unreachable").into_response()
        }
    }
}

async fn bridge(
    State(config): State<Arc<Config>>,
    Path(target): Path<String>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let Some(to) = config.allowed.get(&target).cloned() else {
        log::warn!("refused WebSocket to {target}");
        return StatusCode::FORBIDDEN.into_response();
    };
    let address = config.forward_address.then(|| player_address(peer, &headers));
    ws.on_upgrade(move |socket| relay(socket, to, address))
}

/// The player's address: the one Caddy (on this machine) forwarded, else the peer's.
fn player_address(peer: SocketAddr, headers: &HeaderMap) -> String {
    let forwarded = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok());
    // The nearest proxy adds its client's address last.
    match forwarded.and_then(|f| f.rsplit(',').next()).map(str::trim).filter(|a| !a.is_empty()) {
        Some(a) if peer.ip().is_loopback() => a.to_owned(),
        _ => peer.ip().to_canonical().to_string(),
    }
}

/// PROXY_INFO (250): the world proxy's message telling the world server the player's address.
fn proxy_info(address: &str) -> Vec<u8> {
    let data = address.as_bytes();
    let mut frame = Vec::with_capacity(7 + data.len());
    frame.extend_from_slice(&(7 + data.len() as i32).to_be_bytes());
    frame.push(250);
    frame.extend_from_slice(&(data.len() as i16).to_be_bytes());
    frame.extend_from_slice(data);
    frame
}

async fn relay(socket: WebSocket, target: String, address: Option<String>) {
    let tcp = match TcpStream::connect(&target).await {
        Ok(t) => t,
        Err(e) => {
            log::warn!("{target}: {e}");
            let mut socket = socket;
            let _ = socket.send(Message::Close(None)).await;
            return;
        }
    };
    let _ = tcp.set_nodelay(true);
    log::info!("bridge open to {target}{}", address.as_deref().map(|a| format!(" for {a}")).unwrap_or_default());
    let (mut tcp_read, mut tcp_write) = tcp.into_split();
    if let Some(a) = &address {
        if tcp_write.write_all(&proxy_info(a)).await.is_err() {
            return;
        }
    }
    let (mut ws_send, mut ws_recv) = socket.split();

    let up = async {
        while let Some(Ok(msg)) = ws_recv.next().await {
            match msg {
                Message::Binary(data) => {
                    if tcp_write.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = tcp_write.shutdown().await;
    };
    let down = async {
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            match tcp_read.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if ws_send.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = ws_send.send(Message::Close(None)).await;
    };
    tokio::select! {
        _ = up => {}
        _ = down => {}
    }
    log::info!("bridge closed to {target}");
}
