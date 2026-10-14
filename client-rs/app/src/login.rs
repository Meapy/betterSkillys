//! Signing in, until the real login screens exist (phase 5): the account and credentials come
//! from the command line on the desktop, and from the page in a browser.
//!
//! Desktop: `betterskillys --account http://127.0.0.1:8082 --credentials <file>`, the file having
//! `email:` and `password:` lines (like test-account.local.txt).
//! Browser: the page sets `window.bskLogin = {email, password}` before starting the client; the
//! account server and world servers are reached through the gateway that serves the page.

use bsk_net::account::CharList;
use bsk_net::{account, http, Pending, Target};

#[derive(Clone)]
pub struct LoginConfig {
    /// Account server base URL, without a trailing slash.
    pub account: String,
    pub email: String,
    pub password: String,
    /// Browsers: the gateway's WebSocket base.
    pub gateway: Option<String>,
}

impl LoginConfig {
    pub fn char_list(&self) -> Pending<http::Response> {
        http::post_form(&format!("{}/char/list", self.account), &[("guid", &self.email), ("password", &self.password)])
    }

    pub fn target(&self, host: &str, port: u16) -> Target {
        Target { host: host.to_owned(), port, gateway: self.gateway.clone() }
    }
}

pub fn parse_char_list(response: http::Response) -> Result<CharList, String> {
    account::parse_char_list(&response?)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn config() -> Result<Option<LoginConfig>, String> {
    let mut account = None;
    let mut credentials = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--account" => account = it.next(),
            "--credentials" => credentials = it.next(),
            // Read by main (screenshots, scripted keys).
            "--shot" | "--shot-after" | "--hold" | "--walk-to" | "--aim" => {
                it.next();
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let (Some(account), Some(credentials)) = (account, credentials) else { return Ok(None) };
    let text = std::fs::read_to_string(&credentials).map_err(|e| format!("{credentials}: {e}"))?;
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_owned()))
            .ok_or(format!("{credentials}: no '{name}:' line"))
    };
    Ok(Some(LoginConfig {
        account: account.trim_end_matches('/').to_owned(),
        email: field("email")?,
        password: field("password")?,
        gateway: None,
    }))
}

#[cfg(target_arch = "wasm32")]
pub fn config() -> Result<Option<LoginConfig>, String> {
    use wasm_bindgen::JsValue;
    let window = web_sys::window().ok_or("no window")?;
    let login = js_sys::Reflect::get(&window, &JsValue::from_str("bskLogin")).map_err(|e| format!("{e:?}"))?;
    if login.is_undefined() || login.is_null() {
        return Ok(None);
    }
    let get = |k: &str| {
        js_sys::Reflect::get(&login, &JsValue::from_str(k)).ok().and_then(|v| v.as_string()).unwrap_or_default()
    };
    let location = window.location();
    let host = location.host().map_err(|e| format!("{e:?}"))?;
    let ws = if location.protocol().map_err(|e| format!("{e:?}"))? == "https:" { "wss" } else { "ws" };
    Ok(Some(LoginConfig {
        account: "api".into(),
        email: get("email"),
        password: get("password"),
        gateway: Some(format!("{ws}://{host}/ws")),
    }))
}
