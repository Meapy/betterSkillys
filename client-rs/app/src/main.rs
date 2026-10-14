//! betterSkillys client: the desktop executable and, built for wasm32, the browser version.

mod connected;
mod game;
mod ground;
mod login;
mod redraw;
#[allow(dead_code)] // lookups the game screens will use
mod sprites;
mod text;
mod view;

use bsk_assets::AssetSource;
use bsk_data::GameData;
use bsk_net::Pending;
use bsk_render::Renderer;
use bsk_session::{HelloParams, NEXUS_GAME_ID};
use game::Game;
use login::LoginConfig;
use sprites::Sprites;
use std::collections::HashSet;
use std::sync::Arc;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// Not "betterSkillys": that's the AIR client's window, which tools must never touch.
const TITLE: &str = "betterSkillys-rs";

struct Loaded {
    renderer: Renderer,
    sprites: Sprites,
    data: GameData,
    redrawer: redraw::Redrawer,
    fonts: text::Fonts,
    text: text::Text,
    ground: ground::Ground,
}

enum UserEvent {
    Ready(Box<Loaded>),
    Failed(String),
}

/// Where we are between start-up and playing.
enum Phase {
    /// No login given: nothing to connect to.
    Idle(String),
    SigningIn(LoginConfig, Pending<bsk_net::http::Response>),
    Playing(Game),
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    loaded: Option<Box<Loaded>>,
    phase: Phase,
    started: Instant,
    last_frame: i32,
    keys: HashSet<KeyCode>,
    fps_frames: u32,
    fps_since: Instant,
    last_title: String,
    /// `--shot <file> --shot-after <seconds>`: save one frame as PNG, then quit (desktop).
    shot: Option<(String, f32)>,
    quit: bool,
    /// Scripted key holds (`--hold`), and when the first one started.
    holds: Vec<(KeyCode, i32)>,
    holds_from: Option<i32>,
    last_scripted: Option<KeyCode>,
    walk_to: Option<(f64, f64)>,
    /// The mouse on the window, and whether the left button is down.
    mouse: (f64, f64),
    mouse_down: bool,
    /// The I key: keep firing toward the mouse.
    autofire: bool,
    /// `--aim <degrees>`: where scripted fire (`--hold f:<ms>`) aims, on the screen; `None`
    /// (`--aim auto`) aims at the nearest enemy.
    script_aim: Option<f64>,
    /// Keys pressed since the last frame (for one-shot actions).
    pressed: Vec<KeyCode>,
}

#[cfg(not(target_arch = "wasm32"))]
fn shot_args() -> Option<(String, f32)> {
    let args: Vec<String> = std::env::args().collect();
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    Some((value("--shot")?, value("--shot-after").and_then(|s| s.parse().ok()).unwrap_or(5.0)))
}

#[cfg(target_arch = "wasm32")]
fn shot_args() -> Option<(String, f32)> {
    None
}

/// `--walk-to <x>,<y>`: walk straight to a world point once in the game (desktop tests).
fn walk_to_arg() -> Option<(f64, f64)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        let v = args.iter().position(|a| a == "--walk-to").and_then(|i| args.get(i + 1))?;
        let (x, y) = v.split_once(',')?;
        Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
    }
    #[cfg(target_arch = "wasm32")]
    None
}

/// `--aim <degrees>` (desktop tests): the screen direction scripted fire aims at, or `auto` for
/// the nearest enemy.
fn aim_arg() -> Option<f64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        match args.iter().position(|a| a == "--aim").and_then(|i| args.get(i + 1)) {
            Some(v) if v == "auto" => None,
            v => Some(v.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).to_radians()),
        }
    }
    #[cfg(target_arch = "wasm32")]
    Some(0.0)
}

/// For `--aim auto`: the world angle from the player to the nearest live enemy, if any.
fn nearest_enemy_angle(world: &bsk_world::World, data: &bsk_data::GameData) -> Option<f64> {
    let me = world.player()?;
    world
        .objects
        .values()
        .filter(|o| !o.dead && data.objects.get(&o.object_type).is_some_and(|d| d.enemy))
        .map(|o| (o.x - me.x, o.y - me.y))
        .min_by(|a, b| a.0.hypot(a.1).total_cmp(&b.0.hypot(b.1)))
        .map(|(dx, dy)| dy.atan2(dx))
}

/// `--hold <key>:<ms>` (repeatable): keys to hold one after another once in the game, for
/// testing movement without touching the window (desktop).
fn hold_args() -> Vec<(KeyCode, i32)> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .filter(|w| w[0] == "--hold")
            .filter_map(|w| {
                let (key, ms) = w[1].split_once(':')?;
                let code = match key {
                    "w" => KeyCode::KeyW,
                    "a" => KeyCode::KeyA,
                    "s" => KeyCode::KeyS,
                    "d" => KeyCode::KeyD,
                    "q" => KeyCode::KeyQ,
                    "e" => KeyCode::KeyE,
                    "r" => KeyCode::KeyR,
                    "f" => KeyCode::F23, // fire toward --aim
                    "0" => KeyCode::Digit0,
                    "-" => KeyCode::F24, // a pause: holds nothing
                    _ => return None,
                };
                Some((code, ms.parse().ok()?))
            })
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    Vec::new()
}

async fn load(window: Arc<Window>) -> Result<Box<Loaded>, String> {
    let size = window.inner_size();
    let mut renderer = Renderer::new(window, size.width, size.height).await?;
    let source = AssetSource::locate().map_err(|e| e.to_string())?;
    log::info!("assets: {}", source.describe());
    let manifest = source.manifest().await.map_err(|e| e.to_string())?;
    let t = Instant::now();
    let sprites = Sprites::load(&source, &manifest, &mut renderer).await?;
    let data = GameData::load(&source, &manifest).await?;
    log::info!(
        "loaded {} sheets ({} atlas pages), {} grounds, {} objects in {} ms",
        manifest.sheet_files().len(),
        renderer.atlas_pages(),
        data.grounds.len(),
        data.objects.len(),
        t.elapsed().as_millis()
    );
    let (regular, bold) = futures::join!(source.load(&manifest.fonts.regular), source.load(&manifest.fonts.bold));
    let fonts = text::Fonts::new(regular.map_err(|e| e.to_string())?, bold.map_err(|e| e.to_string())?)?;
    Ok(Box::new(Loaded { renderer, sprites, data, redrawer: Default::default(), fonts, text: Default::default(), ground: Default::default() }))
}

impl App {
    fn now(&self) -> i32 {
        self.started.elapsed().as_millis() as i32
    }

    fn start_login(&mut self) {
        self.phase = match login::config() {
            Ok(Some(cfg)) => {
                let pending = cfg.char_list();
                Phase::SigningIn(cfg, pending)
            }
            Ok(None) => Phase::Idle("no login given (see client-rs/app/src/login.rs)".into()),
            Err(e) => Phase::Idle(e),
        };
    }

    /// Move through the sign-in steps as their answers arrive.
    fn advance(&mut self) {
        let Phase::SigningIn(cfg, pending) = &self.phase else { return };
        let Some(response) = pending.take() else { return };
        self.phase = match login::parse_char_list(response) {
            Err(e) => Phase::Idle(format!("sign-in failed: {e}")),
            Ok(list) => match (list.chars.iter().find(|c| !c.dead), list.servers.first()) {
                (Some(ch), Some(server)) => {
                    log::info!("character {} on {} ({}:{})", ch.id, server.name, server.host, server.port);
                    let hello = HelloParams {
                        build_version: bsk_net::login::BUILD_VERSION.into(),
                        game_id: NEXUS_GAME_ID,
                        guid: bsk_net::login::encrypt(&cfg.email),
                        password: bsk_net::login::encrypt(&cfg.password),
                        key_time: -1,
                        key: vec![],
                    };
                    Phase::Playing(Game::start(cfg.target(&server.host, server.port), hello, ch.id))
                }
                (None, _) => Phase::Idle("this account has no living character".into()),
                (_, None) => Phase::Idle("the account server lists no world servers".into()),
            },
        };
    }

    fn held(&self, codes: &[KeyCode]) -> bool {
        codes.iter().any(|c| self.keys.contains(c) || self.scripted_key(self.now()) == Some(*c))
    }

    /// The key a `--hold` script holds at `now`, counted from when we joined the world.
    fn scripted_key(&self, now: i32) -> Option<KeyCode> {
        let mut t = now - self.holds_from?;
        for &(code, ms) in &self.holds {
            if t < ms {
                return Some(code);
            }
            t -= ms;
        }
        None
    }

    fn frame(&mut self) {
        let now = self.now();
        let dt = (now - self.last_frame).clamp(0, 200);
        self.last_frame = now;
        self.advance();

        let mut keys = (0.0, 0.0);
        if self.held(&[KeyCode::KeyA, KeyCode::ArrowLeft]) {
            keys.0 -= 1.0;
        }
        if self.held(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
            keys.0 += 1.0;
        }
        if self.held(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
            keys.1 -= 1.0;
        }
        if self.held(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
            keys.1 += 1.0;
        }
        let rotate = self.held(&[KeyCode::KeyE]) as i32 as f64 - self.held(&[KeyCode::KeyQ]) as i32 as f64;
        let reset_angle = self.held(&[KeyCode::KeyZ]);

        // Scripted walking (--walk-to), then the --hold script.
        if let (Some((tx, ty)), Phase::Playing(game)) = (self.walk_to, &self.phase) {
            if let Some(me) = game.world.as_ref().and_then(|w| w.player()) {
                let (dx, dy) = (tx - me.x, ty - me.y);
                if dx.hypot(dy) < 0.15 {
                    self.walk_to = None;
                } else {
                    let rel = dy.atan2(dx) - game.camera_angle;
                    keys = (rel.cos(), rel.sin());
                }
            }
        }
        if self.holds_from.is_none() && !self.holds.is_empty() && self.walk_to.is_none() {
            if let Phase::Playing(game) = &self.phase {
                if game.world.as_ref().is_some_and(|w| w.player().is_some()) {
                    self.holds_from = Some(now);
                }
            }
        }
        // A scripted key starting counts as a press.
        let scripted = self.scripted_key(now);
        if scripted != self.last_scripted {
            if let Some(k) = scripted {
                self.pressed.push(k);
            }
            self.last_scripted = scripted;
        }
        let pressed = std::mem::take(&mut self.pressed);
        let scripted_fire = self.scripted_key(now) == Some(KeyCode::F23);
        let wants_fire = self.mouse_down || self.autofire || scripted_fire;
        let (mouse, script_aim) = (self.mouse, self.script_aim);
        let Some(loaded) = self.loaded.as_mut() else { return };
        let Loaded { renderer, sprites, data, redrawer, fonts, text, ground } = &mut **loaded;
        if let Phase::Playing(game) = &mut self.phase {
            for key in pressed {
                match key {
                    KeyCode::Digit0 | KeyCode::Numpad0 => {
                        if let Some((id, _)) = game.nearest_portal(data) {
                            game.use_portal(id);
                        }
                    }
                    KeyCode::KeyR => game.escape(),
                    KeyCode::KeyI => self.autofire = !self.autofire,
                    _ => {}
                }
            }
            game.poll(now, data);
            if reset_angle {
                game.camera_angle = game::DEFAULT_CAMERA_ANGLE;
            }
            game.update(now, dt, keys, rotate, data);
        }

        renderer.begin();
        let (w, h) = renderer.size();
        let status = match &self.phase {
            Phase::Idle(why) => why.clone(),
            Phase::SigningIn(..) => "Signing in…".into(),
            Phase::Playing(game) => game.status.clone(),
        };
        let mut view_radius = None;
        let mut fire = None;
        match &self.phase {
            Phase::Idle(_) | Phase::SigningIn(..) => {}
            Phase::Playing(game) => {
                if let Some(world) = &game.world {
                    if let Some(me) = world.player() {
                        // Freer layout than the AIR client's 600×600 view beside its HUD: the
                        // player is centred, a little below the middle, and the map scales
                        // with the window height.
                        let scale = view::TILE_PX * (h as f64 / 800.0).max(1.0);
                        let camera = view::Camera {
                            x: me.x,
                            y: me.y,
                            angle: game.camera_angle,
                            scale,
                            cx: w as f64 / 2.0,
                            cy: h as f64 * 13.0 / 24.0,
                        };
                        // The farthest screen corner from us, in tiles.
                        let far = [(0.0, 0.0), (w as f64, 0.0), (0.0, h as f64), (w as f64, h as f64)]
                            .iter()
                            .map(|(x, y)| (x - camera.cx).hypot(y - camera.cy) / scale)
                            .fold(0.0, f64::max);
                        view_radius = Some(far);
                        // Firing: toward the mouse from where we stand on the screen (MapUserInput),
                        // or toward --aim for scripted fire.
                        if wants_fire {
                            fire = match (scripted_fire, script_aim) {
                                (true, Some(screen_angle)) => Some(game.camera_angle + screen_angle),
                                (true, None) => nearest_enemy_angle(world, data),
                                _ => Some(game.camera_angle + (mouse.1 - camera.cy).atan2(mouse.0 - camera.cx)),
                            };
                        }
                        let scene = view::Scene { sprites, data, world, camera, time: now, damage_texts: &game.damage_texts };
                        view::draw(&scene, renderer, redrawer, ground, fonts, text);
                    }
                }
            }
        }
        if let (Some(far), Phase::Playing(game)) = (view_radius, &mut self.phase) {
            game.want_view_radius(far, now);
            if let Some(angle) = fire {
                game.shoot(now, angle, data);
            }
        }
        if !status.is_empty() {
            let style = text::Style { size: 18.0, bold: true, color: 0xFFFFFF };
            text.draw_centered(fonts, renderer, &status, w as f32 / 2.0, h as f32 / 2.0 + 40.0, style);
        }
        if let Phase::Playing(game) = &self.phase {
            if let Some((_, name)) = game.nearest_portal(data) {
                let style = text::Style { size: 18.0, bold: true, color: 0xFFFFFF };
                let hint = format!("{name}  (0 to enter)");
                text.draw_centered(fonts, renderer, &hint, w as f32 / 2.0, h as f32 - 60.0, style);
            }
        }
        if let Some((_, after)) = &self.shot {
            if now as f32 >= after * 1000.0 && !renderer.capture_next_frame() {
                log::error!("--shot: this surface can't be read back");
                self.quit = true;
            }
        }
        if let Err(e) = renderer.present(0x000000) {
            log::warn!("present: {e}");
        }
        if let Some(image) = renderer.take_capture() {
            let path = self.shot.take().map(|s| s.0).unwrap_or_default();
            match image.save(&path) {
                Ok(()) => log::info!("saved {path}"),
                Err(e) => log::error!("{path}: {e}"),
            }
            self.quit = true;
        }

        self.fps_frames += 1;
        let elapsed = self.fps_since.elapsed().as_secs_f32();
        if elapsed >= 0.5 {
            let mut title = format!("{TITLE} - {:.0} fps, {} draws", self.fps_frames as f32 / elapsed, renderer.last_draws);
            if !status.is_empty() {
                title += &format!(" - {status}");
            }
            if title != self.last_title {
                if let Some(window) = &self.window {
                    window.set_title(&title);
                }
                if !status.is_empty() && !self.last_title.ends_with(&status) {
                    log::info!("{status}");
                }
                self.last_title = title;
            }
            self.fps_frames = 0;
            self.fps_since = Instant::now();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes().with_title(TITLE);
        #[cfg(not(target_arch = "wasm32"))]
        let attributes = attributes.with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0));
        // In a browser the page's CSS sizes the canvas.
        #[cfg(target_arch = "wasm32")]
        let attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes.with_append(true)
        };
        let window = Arc::new(event_loop.create_window(attributes).expect("create window"));
        self.window = Some(window.clone());

        let proxy = self.proxy.clone();
        let task = async move {
            let event = match load(window).await {
                Ok(loaded) => UserEvent::Ready(loaded),
                Err(e) => UserEvent::Failed(e),
            };
            let _ = proxy.send_event(event);
        };
        #[cfg(not(target_arch = "wasm32"))]
        pollster::block_on(task);
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(task);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Ready(loaded) => {
                self.loaded = Some(loaded);
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    self.loaded.as_mut().unwrap().renderer.resize(size.width, size.height);
                    window.request_redraw();
                }
                self.start_login();
            }
            UserEvent::Failed(e) => {
                log::error!("startup failed: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if let Phase::Playing(game) = &mut self.phase {
                    game.close();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat {
                                self.pressed.push(code);
                            }
                            self.keys.insert(code);
                        }
                        ElementState::Released => {
                            self.keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::Focused(false) => {
                self.keys.clear();
                self.mouse_down = false;
            }
            WindowEvent::CursorMoved { position, .. } => self.mouse = (position.x, position.y),
            WindowEvent::MouseInput { state, button: winit::event::MouseButton::Left, .. } => {
                self.mouse_down = state == ElementState::Pressed;
            }
            WindowEvent::RedrawRequested => {
                self.frame();
                if self.quit {
                    if let Phase::Playing(game) = &mut self.phase {
                        if let Some(me) = game.world.as_ref().and_then(|w| w.player()) {
                            log::info!("player at ({:.2}, {:.2}); {}", me.x, me.y, if game.ended { &game.status } else { "connected" });
                        }
                        game.close();
                    }
                    event_loop.exit();
                    return;
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"))
        .init();
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        let _ = console_log::init_with_level(log::Level::Info);
    }

    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("event loop");
    let app = App {
        proxy: event_loop.create_proxy(),
        window: None,
        loaded: None,
        phase: Phase::Idle("loading".into()),
        started: Instant::now(),
        last_frame: 0,
        keys: HashSet::new(),
        fps_frames: 0,
        fps_since: Instant::now(),
        last_title: String::new(),
        shot: shot_args(),
        quit: false,
        holds: hold_args(),
        holds_from: None,
        last_scripted: None,
        pressed: Vec::new(),
        walk_to: walk_to_arg(),
        mouse: (0.0, 0.0),
        mouse_down: false,
        autofire: false,
        script_aim: aim_arg(),
    };
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = app;
        event_loop.run_app(&mut app).expect("event loop");
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn_app(app);
    }
}
