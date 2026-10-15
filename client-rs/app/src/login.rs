//! Talking to the account server, and remembering the login.
//!
//! Desktop: the account server is `--account <url>`, else the `BSK_ACCOUNT` environment
//! variable, else the address built in (`BSK_ACCOUNT` at build time, or this PC's
//! `http://127.0.0.1:8080`). `--credentials <file>` (with `email:` and `password:` lines, like
//! test-account.local.txt) signs in straight away, for tests.
//! Browser: everything goes through the gateway that serves the page (`api/` and `ws/`); a page
//! may set `window.bskLogin = {email, password}` to sign in straight away.

use bsk_net::account::CharList;
use bsk_net::{account, http, Pending, Target};

/// Where the account server and world servers are reached.
#[derive(Clone, Debug)]
pub struct Endpoint {
    /// Account server base URL, without a trailing slash.
    pub account: String,
    /// Browsers: the gateway's WebSocket base.
    pub gateway: Option<String>,
}

impl Endpoint {
    pub fn char_list(&self, email: &str, password: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/char/list", self.account), &[("guid", email), ("password", password)])
    }

    /// `/account/register`: a new account with this email, password and name.
    pub fn register(&self, email: &str, password: &str, name: &str) -> Pending<http::Response> {
        http::post_form(
            &format!("{}/account/register", self.account),
            &[("guid", email), ("newGuid", email), ("newPassword", password), ("name", name)],
        )
    }

    /// `/char/delete`.
    pub fn delete_char(&self, email: &str, password: &str, char_id: i32) -> Pending<http::Response> {
        let id = char_id.to_string();
        http::post_form(
            &format!("{}/char/delete", self.account),
            &[("guid", email), ("password", password), ("charId", &id)],
        )
    }

    /// `/account/purchaseCharSlot`.
    pub fn buy_slot(&self, email: &str, password: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/account/purchaseCharSlot", self.account), &[("guid", email), ("password", password)])
    }

    /// `/account/purchaseSkin`.
    pub fn buy_skin(&self, email: &str, password: &str, skin: u16) -> Pending<http::Response> {
        let t = skin.to_string();
        http::post_form(&format!("{}/account/purchaseSkin", self.account), &[("guid", email), ("password", password), ("skinType", &t)])
    }

    /// `/account/changePassword`.
    pub fn change_password(&self, email: &str, password: &str, new_password: &str) -> Pending<http::Response> {
        http::post_form(
            &format!("{}/account/changePassword", self.account),
            &[("guid", email), ("password", password), ("newPassword", new_password)],
        )
    }

    /// `/fame/list`: the legends for "week", "month" or "all".
    pub fn legends(&self, timespan: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/fame/list", self.account), &[("timespan", timespan)])
    }

    /// `/char/fame`: a dead character's fame summary.
    pub fn char_fame(&self, account_id: i32, char_id: i32) -> Pending<http::Response> {
        let (a, c) = (account_id.to_string(), char_id.to_string());
        http::post_form(&format!("{}/char/fame", self.account), &[("accountId", &a), ("charId", &c)])
    }

    pub fn guild_board(&self, email: &str, password: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/guild/getBoard", self.account), &[("guid", email), ("password", password)])
    }

    pub fn set_guild_board(&self, email: &str, password: &str, board: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/guild/setBoard", self.account), &[("guid", email), ("password", password), ("board", board)])
    }

    pub fn guild_members(&self, email: &str, password: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/guild/listMembers", self.account), &[("guid", email), ("password", password)])
    }

    pub fn potions(&self, email: &str, password: &str) -> Pending<http::Response> {
        http::post_form(&format!("{}/account/grabPotions", self.account), &[("guid", email), ("password", password)])
    }

    pub fn target(&self, host: &str, port: u16) -> Target {
        Target { host: host.to_owned(), port, gateway: self.gateway.clone() }
    }
}

pub fn parse_char_list(response: http::Response) -> Result<CharList, String> {
    account::parse_char_list(&response?)
}

pub fn parse_success(response: http::Response) -> Result<(), String> {
    account::success(&response?)
}

/// What the command line or page asked for.
pub struct Startup {
    pub endpoint: Endpoint,
    /// Sign in with these straight away (tests, or a page that collected them).
    pub login: Option<(String, String)>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn startup() -> Result<Startup, String> {
    let mut account = None;
    let mut credentials = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--account" => account = it.next(),
            // Read by main: sign in with --credentials but stay on the character list.
            "--stay" | "--bench" => {}
            "--credentials" => credentials = it.next(),
            // Read by main (screenshots, scripted input).
            "--shot" | "--shot-after" | "--hold" | "--walk-to" | "--aim" | "--say" | "--type" | "--npc" | "--effect" | "--size" | "--mouse" | "--mouse-script" => {
                it.next();
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let account = account
        .or_else(|| std::env::var("BSK_ACCOUNT").ok())
        .unwrap_or_else(|| option_env!("BSK_ACCOUNT").unwrap_or("http://127.0.0.1:8080").to_owned());
    let login = match credentials {
        Some(path) => {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            let field = |name: &str| {
                text.lines()
                    .find_map(|l| l.strip_prefix(name).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_owned()))
                    .ok_or(format!("{path}: no '{name}:' line"))
            };
            Some((field("email")?, field("password")?))
        }
        None => None,
    };
    Ok(Startup { endpoint: Endpoint { account: account.trim_end_matches('/').to_owned(), gateway: None }, login })
}

#[cfg(target_arch = "wasm32")]
pub fn startup() -> Result<Startup, String> {
    use wasm_bindgen::JsValue;
    let window = web_sys::window().ok_or("no window")?;
    let location = window.location();
    let host = location.host().map_err(|e| format!("{e:?}"))?;
    let ws = if location.protocol().map_err(|e| format!("{e:?}"))? == "https:" { "wss" } else { "ws" };
    let login = js_sys::Reflect::get(&window, &JsValue::from_str("bskLogin")).ok().filter(|v| !v.is_undefined() && !v.is_null());
    let login = login.map(|l| {
        let get = |k: &str| js_sys::Reflect::get(&l, &JsValue::from_str(k)).ok().and_then(|v| v.as_string()).unwrap_or_default();
        (get("email"), get("password"))
    });
    Ok(Startup { endpoint: Endpoint { account: "api".into(), gateway: Some(format!("{ws}://{host}/ws")) }, login })
}

/// What's kept on this computer (or in this browser) between runs, by name: the login
/// ("Remember me") and the settings, as the AIR client keeps them in its shared object.
#[cfg(not(target_arch = "wasm32"))]
pub mod store {
    use std::path::PathBuf;

    fn path(name: &str) -> Option<PathBuf> {
        let base = if cfg!(windows) {
            std::env::var_os("APPDATA").map(PathBuf::from)
        } else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        };
        base.map(|b| b.join("betterSkillys-rs").join(format!("{name}.txt")))
    }

    pub fn load(name: &str) -> Option<String> {
        std::fs::read_to_string(path(name)?).ok()
    }

    pub fn save(name: &str, text: Option<&str>) {
        let Some(path) = path(name) else { return };
        match text {
            Some(t) => {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                // Written beside it and moved over it, so a crash never leaves half a file.
                let tmp = path.with_extension("tmp");
                if let Err(e) = std::fs::write(&tmp, t).and_then(|_| std::fs::rename(&tmp, &path)) {
                    log::warn!("{}: {e}", path.display());
                }
            }
            None => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub mod store {
    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load(name: &str) -> Option<String> {
        storage()?.get_item(&format!("betterSkillys-rs.{name}")).ok()?
    }

    pub fn save(name: &str, text: Option<&str>) {
        let Some(s) = storage() else { return };
        let key = format!("betterSkillys-rs.{name}");
        let _ = match text {
            Some(t) => s.set_item(&key, t),
            None => s.remove_item(&key),
        };
    }
}

/// The remembered (email, password), if any.
pub fn remembered() -> Option<(String, String)> {
    let text = store::load("login")?;
    let mut lines = text.lines();
    let email = lines.next()?.to_owned();
    let password = lines.next().unwrap_or("").to_owned();
    (!email.is_empty()).then_some((email, password))
}

/// Keep this login for next time, or forget it (`None`).
pub fn remember(login: Option<(&str, &str)>) {
    match login {
        Some((email, password)) => store::save("login", Some(&format!("{email}\n{password}\n"))),
        None => store::save("login", None),
    }
}
