//! Headless test client: logs in to an account server, joins the nexus with the first living
//! character and keeps the connection alive, answering the server as the real client does.
//! Use it against the isolated test servers only.
//!
//!     bsk-bot --account http://127.0.0.1:8082 --credentials <file> [--seconds 60] [--char <id>]
//!             [--gateway ws://127.0.0.1:8095/ws]
//!
//! The credentials file has `email: ...` and `password: ...` lines (test-account.local.txt).
//! With `--gateway`, the game connection goes through the gateway's WebSocket bridge, as the
//! browser client's does. The exit code is 0 if it stayed connected for the whole time.


use bsk_net::{account, http, login, GameSocket, SocketEvent, Target};
use bsk_proto::{ClientMessage, ServerMessage};
use bsk_session::{Event, HelloParams, Join, Phase, Session, NEXUS_GAME_ID};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

struct Args {
    account: String,
    credentials: String,
    seconds: u64,
    char_id: Option<i32>,
    gateway: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut args =
        Args { account: String::new(), credentials: String::new(), seconds: 60, char_id: None, gateway: None };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--account" => args.account = value()?,
            "--credentials" => args.credentials = value()?,
            "--seconds" => args.seconds = value()?.parse().map_err(|e| format!("--seconds: {e}"))?,
            "--char" => args.char_id = Some(value()?.parse().map_err(|e| format!("--char: {e}"))?),
            "--gateway" => args.gateway = Some(value()?),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if args.account.is_empty() || args.credentials.is_empty() {
        return Err("usage: bsk-bot --account <url> --credentials <file> [--seconds N] [--char ID]".into());
    }
    Ok(args)
}

fn read_credentials(path: &str) -> Result<(String, String), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(name).and_then(|r| r.strip_prefix(':')).map(|v| v.trim().to_owned()))
            .ok_or(format!("{path}: no '{name}:' line"))
    };
    Ok((field("email")?, field("password")?))
}

fn wait<T>(pending: bsk_net::Pending<T>) -> T {
    loop {
        if let Some(v) = pending.take() {
            return v;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run(args: Args) -> Result<(), String> {
    let (email, password) = read_credentials(&args.credentials)?;
    let base = args.account.trim_end_matches('/');

    let xml = wait(http::post_form(&format!("{base}/char/list"), &[("guid", &email), ("password", &password)]))?;
    let list = account::parse_char_list(&xml)?;
    let ch = match args.char_id {
        Some(id) => list.chars.iter().find(|c| c.id == id).ok_or(format!("no character {id}"))?,
        None => list.chars.iter().find(|c| !c.dead).ok_or("no living character")?,
    };
    let server = list.servers.first().ok_or("the account server lists no world servers")?;
    log::info!(
        "character {} (type 0x{:04x}, level {}), server {} at {}:{}",
        ch.id, ch.object_type, ch.level, server.name, server.host, server.port
    );

    // Through the gateway the same way the desktop client goes (wss with BSK_CA, as in its tests).
    let mut socket =
        GameSocket::connect(&Target { host: server.host.clone(), port: server.port, gateway: args.gateway.clone() });
    let started = Instant::now();
    let now = || started.elapsed().as_millis() as i32;
    let hello = HelloParams {
        build_version: login::BUILD_VERSION.into(),
        game_id: NEXUS_GAME_ID,
        guid: login::encrypt(&email),
        password: login::encrypt(&password),
        key_time: -1,
        key: vec![],
    };
    let (mut session, first) = Session::start(hello, Join::Load { char_id: ch.id });
    socket.send(&first);

    let mut received: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut sent: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut objects = std::collections::HashSet::new();
    let mut out: Vec<ClientMessage> = Vec::new();
    let deadline = started + Duration::from_secs(args.seconds);
    let mut next_report = started + Duration::from_secs(10);
    // The round trips the server measured (each Ping carries its recent average), this report.
    let mut rtts: Vec<i32> = Vec::new();

    let result = loop {
        if Instant::now() >= deadline {
            break Ok(());
        }
        let Some(event) = socket.poll() else {
            std::thread::sleep(Duration::from_millis(2));
            if Instant::now() >= next_report {
                next_report += Duration::from_secs(10);
                let ping = match (rtts.iter().min(), rtts.iter().max()) {
                    (Some(lo), Some(hi)) => {
                        format!(", ping {lo}-{hi} ms (avg {})", rtts.iter().sum::<i32>() / rtts.len() as i32)
                    }
                    _ => String::new(),
                };
                rtts.clear();
                log::info!(
                    "{:>3} s: {} ticks, {} objects seen{ping}",
                    started.elapsed().as_secs(),
                    received.get("NewTick").unwrap_or(&0),
                    objects.len()
                );
            }
            continue;
        };
        match event {
            SocketEvent::Connected => log::info!("connected"),
            SocketEvent::Closed(why) => break Err(format!("disconnected after {} ms: {why}", now())),
            SocketEvent::Message(msg) => {
                *received.entry(msg.name()).or_default() += 1;
                match &*msg {
                    ServerMessage::Ping(p) if p.round_trip_time >= 0 => rtts.push(p.round_trip_time),
                    ServerMessage::Update(u) => {
                        objects.extend(u.new_objs.iter().map(|o| o.status.object_id));
                        for o in u.new_objs.iter() {
                            log::trace!("object {} type 0x{:04x} at ({}, {})", o.status.object_id, o.object_type, o.status.pos.x, o.status.pos.y);
                        }
                        if let Some(me) = u.new_objs.iter().find(|o| Some(o.status.object_id) == session.object_id) {
                            let stats: Vec<String> =
                                me.status.stats.iter().map(|s| format!("{}={:?}", s.id, s.value)).collect();
                            log::debug!("our stats: {}", stats.join(" "));
                        }
                    }
                    ServerMessage::Text(t) if !t.text.is_empty() => {
                        log::info!("chat <{}> {}", t.name, t.text)
                    }
                    _ => {}
                }
                match session.handle(&msg, now(), &mut out) {
                    Some(Event::MapInfo(m)) => log::info!("map {} ({}×{})", m.name, m.width, m.height),
                    Some(Event::Joined { object_id, .. }) => log::info!("joined as object {object_id}"),
                    Some(Event::Failure { error_id, description }) => {
                        break Err(format!("failure {error_id}: {description}"))
                    }
                    Some(Event::Reconnect(r)) => break Err(format!("asked to reconnect to {}:{}", r.host, r.port)),
                    None => {}
                }
                for m in out.drain(..) {
                    *sent.entry(m.name()).or_default() += 1;
                    socket.send(&m);
                }
            }
        }
    };

    log::info!("received: {received:?}");
    log::info!("sent: {sent:?}");
    if session.phase != Phase::InGame {
        return Err(result.err().unwrap_or_else(|| format!("never got into the game ({:?})", session.phase)));
    }
    result?;
    log::info!("stayed connected for {} s at {:?}", args.seconds, session.position);
    socket.close();
    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let code = match parse_args().and_then(run) {
        Ok(()) => 0,
        Err(e) => {
            log::error!("{e}");
            1
        }
    };
    std::process::exit(code);
}
