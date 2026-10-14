//! One connection to a world server, as a state machine without I/O: feed it the server's
//! messages, send what it returns. It does what `GameServerConnection.as` does to stay connected:
//!
//! - Hello first; on MapInfo, Load the character (or Create a new one);
//! - one Move per NewTick, with that tick's id (the server disconnects on a missing or extra one);
//! - UpdateAck per Update, GotoAck per Goto, Pong per Ping, ShootAck per EnemyShoot and per own
//!   ServerPlayerShoot, AoeAck per Aoe.
//!
//! Times sent to the server are milliseconds since the connection started, like Flash's
//! `getTimer()`; the caller passes the current one in.

use bsk_proto::*;

/// What the session is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Hello sent; waiting for MapInfo.
    Handshake,
    /// Load or Create sent; waiting for CreateSuccess.
    Joining,
    /// In the world.
    InGame,
    /// Disconnected by a Failure (or the server closed).
    Ended,
}

/// How to enter the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    Load { char_id: i32 },
    Create { class_type: u16, skin_type: u16 },
}

/// What the game layer may care about, beyond the acks the session sends itself.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    MapInfo(MapInfo),
    Joined { object_id: i32, char_id: i32 },
    Failure { error_id: i32, description: String },
    Reconnect(Reconnect),
}

pub struct Session {
    pub phase: Phase,
    join: Join,
    /// Our player's object id, after CreateSuccess.
    pub object_id: Option<i32>,
    /// Our player's position: set by the server (Update, Goto), then by the game when it moves.
    pub position: Option<WorldPos>,
    last_tick: Option<i32>,
    move_records: Vec<MoveRecord>,
    last_move_time: Option<i32>,
    pub map: Option<MapInfo>,
}

pub struct HelloParams {
    pub build_version: String,
    pub game_id: i32,
    /// Already RSA-encrypted and base64-encoded.
    pub guid: String,
    pub password: String,
    pub key_time: i32,
    pub key: Vec<u8>,
}

/// The nexus. Other negative ids are special worlds; positive ones come from Reconnect.
pub const NEXUS_GAME_ID: i32 = -2;

impl Session {
    /// Start a session: returns it with the Hello to send first.
    pub fn start(hello: HelloParams, join: Join) -> (Self, ClientMessage) {
        let msg = Hello {
            build_version: hello.build_version,
            game_id: hello.game_id,
            guid: hello.guid,
            password: hello.password,
            key_time: hello.key_time,
            key: hello.key.into(),
            map_json: LongString::default(),
        };
        let session = Self {
            phase: Phase::Handshake,
            join,
            object_id: None,
            position: None,
            last_tick: None,
            move_records: Vec::new(),
            last_move_time: None,
            map: None,
        };
        (session, msg.into())
    }

    /// Record where our player is now (the game calls this as the player walks). Sent with the
    /// next Move, as `GameSprite`'s move records.
    pub fn set_position(&mut self, pos: WorldPos, time: i32) {
        self.position = Some(pos);
        if self.move_records.len() < 10 {
            self.move_records.push(MoveRecord { time, x: pos.x, y: pos.y });
        }
    }

    /// Handle a message from the server. Replies go to `out`; anything else the game should know
    /// about is returned.
    pub fn handle(&mut self, msg: &ServerMessage, now: i32, out: &mut Vec<ClientMessage>) -> Option<Event> {
        match msg {
            ServerMessage::MapInfo(info) => {
                self.map = Some(info.clone());
                if self.phase == Phase::Handshake {
                    out.push(match self.join {
                        Join::Load { char_id } => Load { char_id }.into(),
                        Join::Create { class_type, skin_type } => Create { class_type, skin_type }.into(),
                    });
                    self.phase = Phase::Joining;
                }
                return Some(Event::MapInfo(info.clone()));
            }
            ServerMessage::CreateSuccess(c) => {
                self.object_id = Some(c.object_id);
                self.phase = Phase::InGame;
                return Some(Event::Joined { object_id: c.object_id, char_id: c.char_id });
            }
            ServerMessage::Failure(f) => {
                return Some(Event::Failure { error_id: f.error_id, description: f.description.clone() });
            }
            ServerMessage::Reconnect(r) => {
                self.phase = Phase::Ended;
                return Some(Event::Reconnect(r.clone()));
            }
            ServerMessage::Update(u) => {
                if let Some(me) = self.object_id {
                    if let Some(obj) = u.new_objs.iter().find(|o| o.status.object_id == me) {
                        self.position.get_or_insert(obj.status.pos);
                    }
                }
                out.push(UpdateAck {}.into());
            }
            ServerMessage::NewTick(t) => {
                out.push(self.make_move(t.tick_id, now).into());
                self.last_tick = Some(t.tick_id);
            }
            ServerMessage::Ping(p) => out.push(Pong { serial: p.serial, time: now }.into()),
            ServerMessage::Goto(g) => {
                if Some(g.object_id) == self.object_id {
                    self.position = Some(g.pos);
                }
                out.push(GotoAck { time: now }.into());
            }
            ServerMessage::EnemyShoot(_) => out.push(ShootAck { time: now }.into()),
            ServerMessage::ServerPlayerShoot(s) if Some(s.owner_id) == self.object_id => {
                out.push(ShootAck { time: now }.into());
            }
            ServerMessage::Aoe(a) => out.push(AoeAck { time: now, position: self.position.unwrap_or(a.pos) }.into()),
            _ => {}
        }
        None
    }

    /// `GameServerConnection.move`: the position, plus the records since the last Move if more
    /// than 125 ms passed (at most 10, none from the last 25 ms).
    fn make_move(&mut self, tick_id: i32, now: i32) -> Move {
        let new_position = self.position.unwrap_or(WorldPos { x: -1.0, y: -1.0 });
        let mut records = Vec::new();
        if let Some(last) = self.last_move_time {
            if now - last > 125 {
                records.extend(self.move_records.iter().take_while(|r| r.time < now - 25).take(10).copied());
            }
        }
        self.move_records.clear();
        self.last_move_time = Some(now);
        Move { tick_id, time: now, new_position, records: records.into() }
    }

    pub fn last_tick(&self) -> Option<i32> {
        self.last_tick
    }

    /// When the last Move went out (`MoveRecords.lastClearTime_`).
    pub fn last_move_time(&self) -> Option<i32> {
        self.last_move_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let hello = HelloParams {
            build_version: "1.0.0".into(),
            game_id: NEXUS_GAME_ID,
            guid: "g".into(),
            password: "p".into(),
            key_time: -1,
            key: vec![],
        };
        let (s, first) = Session::start(hello, Join::Load { char_id: 3 });
        assert_eq!(first.id(), 16);
        s
    }

    #[test]
    fn handshake_loads_the_character() {
        let mut s = session();
        let mut out = Vec::new();
        s.handle(&MapInfo::default().into(), 10, &mut out);
        assert_eq!(out, vec![ClientMessage::Load(Load { char_id: 3 })]);
        assert_eq!(s.phase, Phase::Joining);
        let ev = s.handle(&CreateSuccess { object_id: 55, char_id: 3 }.into(), 20, &mut out);
        assert_eq!(ev, Some(Event::Joined { object_id: 55, char_id: 3 }));
        assert_eq!(s.phase, Phase::InGame);
    }

    #[test]
    fn every_new_tick_gets_one_move_with_its_id() {
        let mut s = session();
        let mut out = Vec::new();
        s.handle(&CreateSuccess { object_id: 55, char_id: 3 }.into(), 0, &mut out);
        let me = ObjectData {
            object_type: 0x300,
            status: ObjectStatus { object_id: 55, pos: WorldPos { x: 5.5, y: 6.5 }, stats: Default::default() },
        };
        s.handle(&Update { new_objs: vec![me].into(), ..Default::default() }.into(), 0, &mut out);
        out.clear();
        for tick in 1..=3 {
            s.handle(&NewTick { tick_id: tick, ..Default::default() }.into(), tick * 200, &mut out);
        }
        let moves: Vec<_> = out
            .iter()
            .map(|m| match m {
                ClientMessage::Move(m) => (m.tick_id, m.new_position),
                other => panic!("{other:?}"),
            })
            .collect();
        let at = WorldPos { x: 5.5, y: 6.5 };
        assert_eq!(moves, vec![(1, at), (2, at), (3, at)]);
    }

    #[test]
    fn move_before_spawn_is_minus_one() {
        let mut s = session();
        let mut out = Vec::new();
        s.handle(&NewTick { tick_id: 9, ..Default::default() }.into(), 0, &mut out);
        let ClientMessage::Move(m) = &out[0] else { panic!() };
        assert_eq!(m.new_position, WorldPos { x: -1.0, y: -1.0 });
    }

    #[test]
    fn move_records_follow_the_air_client_rules() {
        let mut s = session();
        let mut out = Vec::new();
        s.handle(&NewTick { tick_id: 1, ..Default::default() }.into(), 1000, &mut out);
        s.set_position(WorldPos { x: 1.0, y: 1.0 }, 1050);
        s.set_position(WorldPos { x: 1.1, y: 1.0 }, 1190); // within 25 ms of the move: left out
        out.clear();
        s.handle(&NewTick { tick_id: 2, ..Default::default() }.into(), 1200, &mut out);
        let ClientMessage::Move(m) = &out[0] else { panic!() };
        assert_eq!(m.records.len(), 1);
        assert_eq!(m.records[0].time, 1050);
    }

    #[test]
    fn acks() {
        let mut s = session();
        let mut out = Vec::new();
        s.handle(&CreateSuccess { object_id: 55, char_id: 3 }.into(), 0, &mut out);
        out.clear();
        s.handle(&Ping { serial: 77, round_trip_time: 0 }.into(), 500, &mut out);
        s.handle(&Goto { object_id: 55, pos: WorldPos { x: 2.0, y: 3.0 } }.into(), 600, &mut out);
        s.handle(&EnemyShoot::default().into(), 700, &mut out);
        s.handle(&ServerPlayerShoot { owner_id: 99, ..Default::default() }.into(), 710, &mut out);
        s.handle(&ServerPlayerShoot { owner_id: 55, ..Default::default() }.into(), 720, &mut out);
        assert_eq!(
            out,
            vec![
                ClientMessage::Pong(Pong { serial: 77, time: 500 }),
                ClientMessage::GotoAck(GotoAck { time: 600 }),
                ClientMessage::ShootAck(ShootAck { time: 700 }),
                ClientMessage::ShootAck(ShootAck { time: 720 }),
            ]
        );
        assert_eq!(s.position, Some(WorldPos { x: 2.0, y: 3.0 }));
    }
}
