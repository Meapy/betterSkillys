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
//!                 [--bind 127.0.0.1:8095]
//!
//! It binds to 127.0.0.1 unless `--bind` says otherwise.

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use axum::Router;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

struct Config {
    account: String,
    /// Target asked for -> address connected to.
    allowed: HashMap<String, String>,
    http: reqwest::Client,
}

/// Largest account request accepted from a browser.
const MAX_API_BODY: usize = 64 * 1024;

fn parse_args() -> Result<(String, String, Config), String> {
    let mut web = None;
    let mut bind = "127.0.0.1:8095".to_owned();
    let mut account = None;
    let mut allowed = HashMap::new();
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
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let usage = "usage: bsk-gateway --web <dir> --account <url> --allow <host:port>[=<host:port>]... [--bind <addr:port>]";
    let (Some(web), Some(account)) = (web, account) else { return Err(usage.into()) };
    if allowed.is_empty() {
        return Err(format!("no --allow world server given\n{usage}"));
    }
    Ok((web, bind, Config { account, allowed, http: reqwest::Client::new() }))
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
        .fallback_service(tower_http::services::ServeDir::new(web))
        .with_state(Arc::new(config));
    let listener = tokio::net::TcpListener::bind(&bind).await.unwrap_or_else(|e| {
        eprintln!("{bind}: {e}");
        std::process::exit(1);
    });
    axum::serve(listener, app).await.expect("server");
}

async fn api(
    State(config): State<Arc<Config>>,
    Path(path): Path<String>,
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

async fn bridge(State(config): State<Arc<Config>>, Path(target): Path<String>, ws: WebSocketUpgrade) -> Response {
    let Some(to) = config.allowed.get(&target).cloned() else {
        log::warn!("refused WebSocket to {target}");
        return StatusCode::FORBIDDEN.into_response();
    };
    ws.on_upgrade(move |socket| relay(socket, to))
}

async fn relay(socket: WebSocket, target: String) {
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
    log::info!("bridge open to {target}");
    let (mut tcp_read, mut tcp_write) = tcp.into_split();
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
