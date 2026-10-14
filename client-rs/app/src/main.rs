//! betterSkillys client: the desktop executable and, built for wasm32, the browser version.

mod sprites;

use bsk_assets::AssetSource;
use bsk_render::{Quad, Renderer, Tint};
use sprites::Sprites;
use std::sync::Arc;
use web_time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

/// Not "betterSkillys": that's the AIR client's window, which tools must never touch.
const TITLE: &str = "betterSkillys-rs";

struct Loaded {
    renderer: Renderer,
    sprites: Sprites,
}

enum UserEvent {
    Ready(Box<Loaded>),
    Failed(String),
}

struct App {
    proxy: EventLoopProxy<UserEvent>,
    window: Option<Arc<Window>>,
    loaded: Option<Box<Loaded>>,
    started: Instant,
    fps_frames: u32,
    fps_since: Instant,
}

async fn load(window: Arc<Window>) -> Result<Box<Loaded>, String> {
    let size = window.inner_size();
    let mut renderer = Renderer::new(window, size.width, size.height).await?;
    let source = AssetSource::locate().map_err(|e| e.to_string())?;
    log::info!("assets: {}", source.describe());
    let manifest = source.manifest().await.map_err(|e| e.to_string())?;
    let t = Instant::now();
    let sprites = Sprites::load(&source, &manifest, &mut renderer).await?;
    log::info!(
        "{} sheets loaded into {} atlas pages in {} ms",
        manifest.sheet_files().len(),
        renderer.atlas_pages(),
        t.elapsed().as_millis()
    );
    Ok(Box::new(Loaded { renderer, sprites }))
}

impl App {
    fn draw(&mut self) {
        let Some(loaded) = self.loaded.as_mut() else { return };
        let Loaded { renderer, sprites } = &mut **loaded;
        let t = self.started.elapsed().as_secs_f32();
        let (w, h) = renderer.size();
        renderer.begin();

        // Phase 0 test scene: a grass field and the player classes walking.
        let scale = 5.0;
        let tile = 8.0 * scale;
        if let Some(grass) = sprites.tile("lofiEnvironment2", 0x31) {
            let (cols, rows) = ((w as f32 / tile) as i32 + 1, (h as f32 / tile) as i32 + 1);
            for ty in 0..rows {
                for tx in 0..cols {
                    renderer.sprite(grass, Quad::rect(tx as f32 * tile, ty as f32 * tile, tile, tile), Tint::NONE);
                }
            }
        }
        let frame = ((t * 4.0) as u32) % 2 + 1;
        if let Some(players) = sprites.char_sheet("players") {
            for i in 0..players.count {
                let x = 40.0 + (i % 12) as f32 * 80.0;
                let y = 40.0 + (i / 12) as f32 * 80.0;
                if let Some(rect) = sprites.char_frame("players", i, 0, frame) {
                    renderer.sprite(rect, Quad::rect(x, y, tile, tile), Tint::NONE);
                }
            }
        }
        if let Some(rect) = sprites.tile("lofiObj3", 0x1a) {
            renderer.sprite(rect, Quad::rotated(w as f32 / 2.0, h as f32 / 2.0, tile * 2.0, tile * 2.0, t), Tint::NONE);
        }
        renderer.fill(0.0, h as f32 - 24.0, w as f32, 24.0, 0x000000, 0.6);

        if let Err(e) = renderer.present(0x000000) {
            log::warn!("present: {e}");
        }

        self.fps_frames += 1;
        let elapsed = self.fps_since.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            if let Some(window) = &self.window {
                window.set_title(&format!(
                    "{TITLE} - {:.0} fps, {} draws",
                    self.fps_frames as f32 / elapsed,
                    renderer.last_draws
                ));
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
        #[allow(unused_mut)]
        let mut attributes = Window::default_attributes()
            .with_title(TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(1024.0, 768.0));
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes = attributes.with_append(true);
        }
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
            }
            UserEvent::Failed(e) => {
                log::error!("startup failed: {e}");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(loaded) = &mut self.loaded {
                    loaded.renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                self.draw();
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
        started: Instant::now(),
        fps_frames: 0,
        fps_since: Instant::now(),
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
