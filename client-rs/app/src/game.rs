//! A game in progress: the world server connection, the session that keeps it alive, and the
//! world it describes.

use bsk_data::GameData;
use bsk_net::{GameSocket, SocketEvent, Target};
use bsk_proto::{ClientMessage, ServerMessage, WorldPos};
use bsk_session::{Event, HelloParams, Join, Session};
use bsk_world::{MoveInput, World};

/// The AIR client's default camera angle (`Parameters`: 7π/4), which Z resets to.
pub const DEFAULT_CAMERA_ANGLE: f64 = 7.0 * std::f64::consts::PI / 4.0;
/// `Parameters.PLAYER_ROTATE_SPEED`, radians per millisecond.
const ROTATE_SPEED: f64 = 0.003;

pub struct Game {
    socket: GameSocket,
    session: Session,
    pub world: Option<World>,
    /// What to show while there's no world yet, or why the game ended.
    pub status: String,
    pub ended: bool,
    pub camera_angle: f64,
    out: Vec<ClientMessage>,
}

impl Game {
    pub fn start(target: Target, hello: HelloParams, join: Join) -> Self {
        let mut socket = GameSocket::connect(&target);
        let (session, first) = Session::start(hello, join);
        socket.send(&first);
        Self {
            socket,
            session,
            world: None,
            status: format!("Connecting to {}:{}…", target.host, target.port),
            ended: false,
            camera_angle: DEFAULT_CAMERA_ANGLE,
            out: Vec::new(),
        }
    }

    /// Handle everything the server sent since the last frame.
    pub fn poll(&mut self, now: i32, data: &GameData) {
        while let Some(event) = self.socket.poll() {
            match event {
                SocketEvent::Connected => self.status = "Connected, entering the game…".into(),
                SocketEvent::Closed(why) => {
                    self.status = format!("Disconnected: {why}");
                    self.ended = true;
                }
                SocketEvent::Message(msg) => self.handle(&msg, now, data),
            }
        }
    }

    fn handle(&mut self, msg: &ServerMessage, now: i32, data: &GameData) {
        // The session answers first (a NewTick's Move goes out before its statuses apply, as
        // GameServerConnection.onNewTick does).
        let event = self.session.handle(msg, now, &mut self.out);
        let sent_move = self.out.iter().any(|m| matches!(m, ClientMessage::Move(_)));
        for m in self.out.drain(..) {
            self.socket.send(&m);
        }
        if let Some(world) = &mut self.world {
            if sent_move {
                world.on_move(data);
            }
        }
        match event {
            Some(Event::MapInfo(info)) => {
                self.world = Some(World::new(info.width as i32, info.height as i32, &info.display_name));
                self.status = format!("Entering {}…", info.display_name);
            }
            Some(Event::Joined { object_id, .. }) => {
                if let Some(w) = &mut self.world {
                    w.me = Some(object_id);
                }
                self.status.clear();
            }
            Some(Event::Failure { description, .. }) => self.status = description,
            Some(Event::Reconnect(r)) => {
                // Changing worlds comes later; for now say where the server wanted us to go.
                self.status = format!("The server sent us to {} ({}:{}), which isn't supported yet", r.name, r.host, r.port);
                self.ended = true;
            }
            None => {}
        }
        let Some(world) = &mut self.world else { return };
        match msg {
            ServerMessage::Update(u) => world.apply_update(u, data, now),
            ServerMessage::NewTick(t) => world.apply_new_tick(t, now),
            ServerMessage::Goto(g) => world.apply_goto(g.object_id, g.pos.x as f64, g.pos.y as f64, now, data),
            _ => {}
        }
    }

    /// Advance the world by `dt` ms with the keys held; `rotate` is -1, 0 or 1.
    pub fn update(&mut self, now: i32, dt: i32, keys: (f64, f64), rotate: f64, data: &GameData) {
        self.camera_angle += dt as f64 * ROTATE_SPEED * rotate;
        let Some(world) = &mut self.world else { return };
        let input = MoveInput { x: keys.0, y: keys.1, camera_angle: self.camera_angle };
        if let Some((x, y)) = world.update(now, dt, &input, data) {
            self.session.set_position(WorldPos { x: x as f32, y: y as f32 }, now);
        }
    }

    pub fn close(&mut self) {
        self.socket.close();
    }
}
