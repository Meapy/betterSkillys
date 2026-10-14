//! A game in progress: the world server connection, the session that keeps it alive, and the
//! world it describes. Changing worlds (portals, escaping to the nexus) is a Reconnect: a new
//! connection with the key the server handed out.

use bsk_data::{GameData, ObjectClass};
use bsk_net::{GameSocket, SocketEvent, Target};
use bsk_proto::{
    ClientMessage, EnemyHit, Escape, GroundDamage, InvDrop, InvSwap, OtherHit, PlayerHit, PlayerShoot, PlayerText,
    Reconnect, ServerMessage, SlotObject, SquareHit, UseItem, UsePortal, ViewRadius, WorldPos,
};
use bsk_session::{Event, HelloParams, Join, Session};
use bsk_world::{Hit, MoveInput, Projectile, World};

/// The AIR client's default camera angle (`Parameters`: 7π/4), which Z resets to.
pub const DEFAULT_CAMERA_ANGLE: f64 = 7.0 * std::f64::consts::PI / 4.0;
/// `Parameters.PLAYER_ROTATE_SPEED`, radians per millisecond.
const ROTATE_SPEED: f64 = 0.003;
/// `Map.MIN_VIEW_RADIUS` (the server's default sight) and `MAX_VIEW_RADIUS`, in tiles.
const MIN_VIEW_RADIUS: i32 = 15;
const MAX_VIEW_RADIUS: i32 = 40;

pub struct Game {
    socket: GameSocket,
    session: Session,
    target: Target,
    /// What Hello needs again on a reconnect.
    hello: HelloParams,
    char_id: i32,
    pub world: Option<World>,
    /// What to show while there's no world yet, or why the game ended.
    pub status: String,
    pub ended: bool,
    pub camera_angle: f64,
    out: Vec<ClientMessage>,
    /// The sight radius the server uses for us, and when we last changed it.
    sent_view_radius: i32,
    view_radius_sent_at: i32,
    /// Text rising over objects: damage numbers and the server's notifications.
    pub floaters: Vec<crate::view::Floater>,
    /// Speech balloons, one per speaker.
    pub bubbles: Vec<crate::view::Bubble>,
    /// When the ability (Space) can be used again (`Player.nextAltAttack_`).
    next_ability: i32,
    /// What we sent, by message name (logged when the game closes).
    pub counts: std::collections::BTreeMap<&'static str, u32>,
    /// Chat, oldest first (kept across worlds, as the AIR client's TextBox is).
    pub chat: Vec<crate::hud::ChatLine>,
}

/// How many chat lines to keep.
const CHAT_KEEP: usize = 200;

impl Game {
    /// Connect and load a character, or create one (`Join::Create`).
    pub fn start(target: Target, hello: HelloParams, join: Join) -> Self {
        let socket = GameSocket::connect(&target);
        let char_id = match join {
            Join::Load { char_id } => char_id,
            Join::Create { .. } => -1,
        };
        let (session, first) = Session::start(clone_hello(&hello), join);
        let mut game = Self {
            socket,
            session,
            status: format!("Connecting to {}:{}…", target.host, target.port),
            target,
            hello,
            char_id,
            world: None,
            ended: false,
            camera_angle: DEFAULT_CAMERA_ANGLE,
            out: Vec::new(),
            sent_view_radius: MIN_VIEW_RADIUS,
            view_radius_sent_at: i32::MIN / 2,
            floaters: Vec::new(),
            bubbles: Vec::new(),
            next_ability: 0,
            counts: Default::default(),
            chat: Vec::new(),
        };
        game.socket.send(&first);
        game
    }

    /// `GameServerConnection.onReconnect`: connect again, to another server if the message
    /// names one, with its game id and key.
    fn reconnect(&mut self, r: &Reconnect) {
        self.socket.close();
        if !r.host.is_empty() {
            self.target.host = r.host.clone();
            self.target.port = r.port as u16;
        }
        self.hello.game_id = r.game_id;
        self.hello.key_time = r.key_time;
        self.hello.key = r.key.0.clone();
        self.socket = GameSocket::connect(&self.target);
        let (session, first) = Session::start(clone_hello(&self.hello), Join::Load { char_id: self.char_id });
        self.session = session;
        self.socket.send(&first);
        self.world = None;
        self.sent_view_radius = MIN_VIEW_RADIUS;
        self.status = format!("Entering {}…", r.name);
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
            if self.ended {
                break;
            }
        }
    }

    fn handle(&mut self, msg: &ServerMessage, now: i32, data: &GameData) {
        // The session answers first (a NewTick's Move goes out before its statuses apply, as
        // GameServerConnection.onNewTick does).
        let event = self.session.handle(msg, now, &mut self.out);
        // Shots from an owner we don't know (or that died) are acknowledged with -1, as
        // GameServerConnection.onEnemyShoot / onServerPlayerShoot do.
        let unknown_owner = |id: i32| self.world.as_ref().and_then(|w| w.objects.get(&id)).is_none_or(|o| o.dead);
        let owner = match msg {
            ServerMessage::EnemyShoot(e) => Some(e.owner_id),
            ServerMessage::ServerPlayerShoot(s) => Some(s.owner_id),
            _ => None,
        };
        if owner.is_some_and(unknown_owner) {
            for m in self.out.iter_mut() {
                if let ClientMessage::ShootAck(ack) = m {
                    ack.time = -1;
                }
            }
        }
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
                self.world = Some(World::new(info.width as i32, info.height as i32, &info.display_name, info.seed));
                self.status = format!("Entering {}…", info.display_name);
            }
            Some(Event::Joined { object_id, char_id }) => {
                // A new character has its id now; reconnects load it.
                self.char_id = char_id;
                if let Some(w) = &mut self.world {
                    w.me = Some(object_id);
                }
                self.status.clear();
            }
            Some(Event::Failure { description, .. }) => self.status = description,
            Some(Event::Reconnect(r)) => {
                self.reconnect(&r);
                return;
            }
            None => {}
        }
        if let ServerMessage::Death(d) = msg {
            if d.char_id == self.char_id {
                self.status = format!("You died, killed by {}.", d.killed_by);
                self.ended = true;
                return;
            }
        }
        if let ServerMessage::Text(t) = msg {
            self.add_chat(&t.name, &t.recipient, &t.text, t.name_color, t.text_color, now);
            // GameServerConnection.onText: a balloon over the speaker, coloured by who it is.
            let speaker = self.world.as_ref().and_then(|w| w.objects.get(&t.object_id));
            if let (Some(o), true) = (speaker, t.object_id >= 0 && t.bubble_time > 0) {
                let enemy = data.objects.get(&o.object_type).is_some_and(|d| d.enemy);
                let colors = if enemy {
                    [0x561F1C, 0xFC8642, 0xCDC0BF]
                } else if t.recipient == "*Guild*" {
                    [0x3E8A00, 0xA6FF5D, 0xD3F8CC]
                } else if t.recipient == "*Party*" {
                    [0x78A0FF, 0x5A7DD1, 0xD9E4FF]
                } else if !t.recipient.is_empty() {
                    [0x260AB6, 0x00F0FF, 0xD3CCF7]
                } else {
                    [0xE1DEDC, 0xFFFFFF, 0x545454]
                };
                self.bubbles.retain(|b| b.object_id != t.object_id);
                self.bubbles.push(crate::view::Bubble {
                    object_id: t.object_id,
                    text: t.text.clone(),
                    colors,
                    at: now,
                    life: t.bubble_time as i32 * 1000,
                });
            }
        }
        let Some(world) = &mut self.world else { return };
        match msg {
            ServerMessage::Update(u) => world.apply_update(u, data, now),
            ServerMessage::NewTick(t) => world.apply_new_tick(t, now),
            ServerMessage::Goto(g) => world.apply_goto(g.object_id, g.pos.x as f64, g.pos.y as f64, now, data),
            ServerMessage::EnemyShoot(e) => {
                let Some(owner) = world.objects.get(&e.owner_id).filter(|o| !o.dead) else { return };
                let container = owner.object_type;
                let (num, inc) = e.spread.map_or((1, 0.0), |s| (s.num_shots as i32, s.angle_inc as f64));
                for i in 0..num {
                    let angle = e.angle as f64 + inc * i as f64;
                    let (x, y) = (e.starting_pos.x as f64, e.starting_pos.y as f64);
                    let p = Projectile::new(e.owner_id, e.bullet_id + i, container, e.bullet_type as i32, angle, now, x, y, 1.0, e.damage, true);
                    world.add_projectile(p);
                }
                if let Some(owner) = world.objects.get_mut(&e.owner_id) {
                    owner.attack_start = now;
                    owner.attack_angle = e.angle as f64 + inc * (num - 1) as f64 / 2.0;
                }
            }
            ServerMessage::ServerPlayerShoot(s) => {
                if world.objects.get(&s.owner_id).is_some_and(|o| !o.dead) {
                    let (x, y) = (s.starting_pos.x as f64, s.starting_pos.y as f64);
                    let p = Projectile::new(s.owner_id, s.bullet_id, s.container_type as u16, 0, s.angle as f64, now, x, y, 1.0, s.damage, false);
                    world.add_projectile(p);
                }
            }
            ServerMessage::AllyShoot(a) => {
                let Some(owner) = world.objects.get_mut(&a.owner_id).filter(|o| !o.dead) else { return };
                owner.attack_start = now;
                owner.attack_angle = a.angle as f64;
                let (x, y) = (owner.x, owner.y);
                let p = Projectile::new(a.owner_id, a.bullet_id, a.container_type as u16, 0, a.angle as f64, now, x, y, 1.0, 0, false);
                world.add_projectile(p);
            }
            ServerMessage::Notification(n) => {
                // CharacterStatusText: two seconds over the object.
                self.floaters.push(crate::view::Floater {
                    object_id: n.object_id,
                    text: n.text.clone(),
                    color: n.color & 0xFF_FFFF,
                    at: now,
                    life: 2000,
                });
            }
            ServerMessage::Damage(d) => {
                // Someone was hit (not by our shots): show the number.
                if Some(d.object_id) != world.me {
                    self.floaters.push(crate::view::Floater::damage(d.target_id, d.damage_amount, now));
                }
            }
            _ => {}
        }
    }

    pub fn add_chat(&mut self, name: &str, recipient: &str, text: &str, name_color: i32, text_color: i32, now: i32) {
        self.chat.push(crate::hud::ChatLine {
            name: name.to_owned(),
            recipient: recipient.to_owned(),
            text: text.to_owned(),
            name_color,
            text_color,
            at: now,
        });
        if self.chat.len() > CHAT_KEEP {
            self.chat.drain(..self.chat.len() - CHAT_KEEP);
        }
    }

    /// Send a chat line or command (`PlayerText`); the server echoes what others should see.
    pub fn say(&mut self, text: &str) {
        let text = text.trim();
        if !text.is_empty() {
            self.socket.send(&PlayerText { text: text.to_owned() }.into());
        }
    }

    /// Fire toward the world angle `angle` (mouse held). Sends a PlayerShoot per projectile.
    pub fn shoot(&mut self, now: i32, angle: f64, data: &GameData) {
        if self.session.map.as_ref().is_some_and(|m| m.disable_shooting) {
            return;
        }
        let last_move = self.session.last_move_time().unwrap_or(now);
        let Some(world) = &mut self.world else { return };
        for p in world.shoot(now, angle, last_move, data) {
            let msg = PlayerShoot {
                time: now,
                bullet_id: p.bullet_id,
                container_type: p.container_type as i32,
                starting_pos: WorldPos { x: p.start_x as f32, y: p.start_y as f32 },
                angle: p.angle as f32,
            };
            *self.counts.entry("PlayerShoot").or_default() += 1;
            self.socket.send(&msg.into());
        }
    }

    /// Move the projectiles and tell the server what they hit.
    fn update_projectiles(&mut self, now: i32, data: &GameData) {
        let Some(world) = &mut self.world else { return };
        for hit in world.update_projectiles(now, data) {
            let msg: ClientMessage = match hit {
                Hit::Player { bullet_id, owner_id, damage } => {
                    if let Some(me) = world.me {
                        self.floaters.push(crate::view::Floater::damage(me, damage, now));
                    }
                    PlayerHit { bullet_id, object_id: owner_id }.into()
                }
                Hit::Enemy { time, bullet_id, target_id, kill, damage } => {
                    self.floaters.push(crate::view::Floater::damage(target_id, damage, now));
                    EnemyHit { time, bullet_id, target_id, kill }.into()
                }
                Hit::Other { time, bullet_id, owner_id, target_id } => {
                    OtherHit { time, bullet_id, object_id: owner_id, target_id }.into()
                }
                Hit::Square { time, bullet_id, owner_id } => SquareHit { time, bullet_id, object_id: owner_id }.into(),
                Hit::Ground { time, x, y, damage } => {
                    if let Some(me) = world.me {
                        self.floaters.push(crate::view::Floater::damage(me, damage, now));
                    }
                    GroundDamage { time, position: WorldPos { x: x as f32, y: y as f32 } }.into()
                }
            };
            *self.counts.entry(msg.name()).or_default() += 1;
            self.socket.send(&msg);
        }
        self.floaters.retain(|f| now - f.at < f.life);
        self.bubbles.retain(|b| now - b.at < b.life);
    }

    /// Advance the world by `dt` ms with the keys held; `rotate` is -1, 0 or 1.
    pub fn update(&mut self, now: i32, dt: i32, keys: (f64, f64), rotate: f64, data: &GameData) {
        self.camera_angle += dt as f64 * ROTATE_SPEED * rotate;
        let Some(world) = &mut self.world else { return };
        let input = MoveInput { x: keys.0, y: keys.1, camera_angle: self.camera_angle };
        if let Some((x, y)) = world.update(now, dt, &input, data) {
            self.session.set_position(WorldPos { x: x as f32, y: y as f32 }, now);
        }
        self.update_projectiles(now, data);
    }

    /// `GameSprite`'s VIEW_RADIUS: ask the server to send as far as the screen reaches
    /// (`far` tiles from us), 15 to 40, at most once a second when it changes.
    pub fn want_view_radius(&mut self, far: f64, now: i32) {
        let radius = (far.ceil() as i32).clamp(MIN_VIEW_RADIUS, MAX_VIEW_RADIUS);
        let in_game = self.world.as_ref().is_some_and(|w| w.player().is_some());
        if in_game && radius != self.sent_view_radius && now - self.view_radius_sent_at >= 1000 {
            self.socket.send(&ViewRadius { radius }.into());
            self.sent_view_radius = radius;
            self.view_radius_sent_at = now;
        }
    }

    /// The portal within a tile of us, if any, with what to call it.
    pub fn nearest_portal<'a>(&'a self, data: &'a GameData) -> Option<(i32, &'a str)> {
        let world = self.world.as_ref()?;
        let me = world.player()?;
        world
            .objects
            .values()
            .filter_map(|o| {
                let desc = data.objects.get(&o.object_type)?;
                matches!(desc.class, ObjectClass::Portal | ObjectClass::GuildHallPortal).then_some((o, desc))
            })
            .map(|(o, desc)| ((o.x - me.x).hypot(o.y - me.y), o, desc))
            .filter(|(d, _, _)| *d < 1.0)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, o, desc)| (o.id, o.name().unwrap_or(&desc.display_id)))
    }

    fn send(&mut self, msg: ClientMessage) {
        *self.counts.entry(msg.name()).or_default() += 1;
        self.socket.send(&msg);
    }

    /// Set an inventory slot locally until the server's update arrives (the AIR client changes
    /// `equipment_` as it sends).
    fn predict(&mut self, owner: i32, slot: u8, item: i32) {
        if let Some(o) = self.world.as_mut().and_then(|w| w.objects.get_mut(&owner)) {
            o.stats.insert(crate::hud::slot_stat(slot), bsk_proto::StatValue::Int(item));
        }
    }

    /// `GameServerConnection.invSwap`.
    pub fn inv_swap(&mut self, now: i32, a: (i32, u8, i32), b: (i32, u8, i32)) {
        let Some(me) = self.world.as_ref().and_then(|w| w.player()) else { return };
        let position = WorldPos { x: me.x as f32, y: me.y as f32 };
        let msg = InvSwap {
            time: now,
            position,
            slot_object1: SlotObject { object_id: a.0, slot_id: a.1, object_type: a.2 },
            slot_object2: SlotObject { object_id: b.0, slot_id: b.1, object_type: b.2 },
        };
        self.send(msg.into());
        self.predict(a.0, a.1, b.2);
        self.predict(b.0, b.1, a.2);
    }

    /// `useItem_new`: use a consumable (or InvUse item) in a slot. False if it can't be used.
    pub fn use_item(&mut self, now: i32, owner: i32, slot: u8, item: i32, data: &GameData) -> bool {
        let Some(xml) = data.objects.get(&(item as u16)).map(|d| &d.xml) else { return false };
        if !(xml.has("Consumable") || xml.has("InvUse")) {
            return false;
        }
        let consumable = xml.has("Consumable");
        let msg = UseItem {
            time: now,
            slot_object: SlotObject { object_id: owner, slot_id: slot, object_type: item },
            item_use_pos: WorldPos { x: 0.0, y: 0.0 },
            use_type: 0,
        };
        self.send(msg.into());
        if consumable {
            self.predict(owner, slot, -1);
        }
        true
    }

    /// `invDrop`: put it on the ground.
    pub fn inv_drop(&mut self, owner: i32, slot: u8, item: i32) {
        self.send(InvDrop { slot_object: SlotObject { object_id: owner, slot_id: slot, object_type: item } }.into());
        self.predict(owner, slot, -1);
    }

    /// `Player.useAltWeapon` (Space): the ability in slot 1, aimed at a world point; `angle`
    /// is the world direction of the mouse, for abilities that shoot.
    pub fn use_ability(&mut self, now: i32, x: f64, y: f64, angle: f64, data: &GameData) {
        if self.session.map.as_ref().is_some_and(|m| m.disable_abilities) {
            return;
        }
        let Some(me) = self.world.as_ref().and_then(|w| w.player()) else { return };
        let item = me.stat_int(crate::hud::slot_stat(1)).unwrap_or(-1);
        let mp = me.stat_int(crate::hud::stat::MP).unwrap_or(0);
        let (id, paused) = (me.id, bsk_world::condition::has(me.conditions(), bsk_world::condition::PAUSED));
        let Some(xml) = data.objects.get(&(item as u16)).map(|d| &d.xml) else { return };
        if paused || !xml.has("Usable") || now < self.next_ability || xml.int("MpCost", 0) > mp {
            return;
        }
        self.next_ability = now + (xml.number("Cooldown", 0.5) * 1000.0) as i32;
        let shoots = xml.children_named("Activate").any(|a| a.text.trim() == "Shoot");
        let msg = UseItem {
            time: now,
            slot_object: SlotObject { object_id: id, slot_id: 1, object_type: item },
            item_use_pos: WorldPos { x: x as f32, y: y as f32 },
            use_type: 1,
        };
        self.send(msg.into());
        if shoots {
            let last_move = self.session.last_move_time().unwrap_or(now);
            let Some(world) = &mut self.world else { return };
            for p in world.shoot_ability(now, item, angle, last_move, data) {
                let msg = PlayerShoot {
                    time: now,
                    bullet_id: p.bullet_id,
                    container_type: p.container_type as i32,
                    starting_pos: WorldPos { x: p.start_x as f32, y: p.start_y as f32 },
                    angle: p.angle as f32,
                };
                self.send(msg.into());
            }
        }
    }

    /// F and V: a health or magic potion from the stacks (`UseBuyPotionCommand`).
    pub fn use_potion(&mut self, now: i32, which: usize) {
        let Some(me) = self.world.as_ref().and_then(|w| w.player()) else { return };
        let stack = if which == 0 { crate::hud::stat::HEALTH_POTION_STACK } else { crate::hud::stat::MAGIC_POTION_STACK };
        if me.stat_int(stack).unwrap_or(0) <= 0 {
            return;
        }
        let (item, slot) = crate::hud::POTIONS[which];
        let msg = UseItem {
            time: now,
            slot_object: SlotObject { object_id: me.id, slot_id: slot, object_type: item },
            item_use_pos: WorldPos { x: me.x as f32, y: me.y as f32 },
            use_type: 1,
        };
        self.send(msg.into());
    }

    pub fn use_portal(&mut self, object_id: i32) {
        self.socket.send(&UsePortal { object_id }.into());
    }

    /// Back to the nexus (the R key). Not from the nexus itself: the server disconnects for that.
    pub fn escape(&mut self) {
        if self.session.map.as_ref().is_some_and(|m| m.name != "Nexus") {
            self.socket.send(&Escape {}.into());
        }
    }

    pub fn close(&mut self) {
        if !self.counts.is_empty() {
            log::info!("sent: {:?}", self.counts);
        }
        self.socket.close();
    }
}

fn clone_hello(h: &HelloParams) -> HelloParams {
    HelloParams {
        build_version: h.build_version.clone(),
        game_id: h.game_id,
        guid: h.guid.clone(),
        password: h.password.clone(),
        key_time: h.key_time,
        key: h.key.clone(),
    }
}
