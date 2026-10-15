//! The game world as the client sees it: the map's tiles, the objects on it, and our player's
//! movement. Ported from the AIR client's `Map`, `Square`, `GameObject` and `Player`, keeping
//! their arithmetic (positions are `f64`, as AS3's `Number`).
//!
//! Times are milliseconds since the game started (`GameSprite.lastUpdate_`).

pub mod combat;
pub mod condition;
mod movement;
pub mod random;

use bsk_data::GameData;
use bsk_proto::{stat, NewTick, ObjectData, StatData, StatValue, Update};
use std::collections::HashMap;

pub use combat::{Hit, Impact, Projectile};
pub use movement::{move_speed, MoveInput};
pub use random::Random;

/// The tile type of a square nothing has been sent for (`Square.tileType_` default).
pub const NO_TILE: u16 = 255;

pub struct GameObj {
    pub id: i32,
    pub object_type: u16,
    pub x: f64,
    pub y: f64,
    tick_x: f64,
    tick_y: f64,
    pos_at_tick_x: f64,
    pos_at_tick_y: f64,
    move_x: f64,
    move_y: f64,
    last_tick_update_time: i32,
    my_last_tick_id: i32,
    /// Every stat the server has sent, latest value.
    pub stats: HashMap<u8, StatValue>,
    /// Whether it moved during the last update (for walk animation).
    pub moving: bool,
    /// The direction it last moved in, radians (for facing).
    pub facing: f64,
    /// When it last attacked and toward which world angle (for the attack frames).
    pub attack_start: i32,
    pub attack_angle: f64,
    /// Killed by one of our shots, before the server says so.
    pub dead: bool,
    /// Its `<Animation>` state, and the (animation, frame) showing now.
    animations: bsk_data::Animations,
    pub anim_frame: Option<(usize, usize)>,
}

impl GameObj {
    pub fn stat_int(&self, id: u8) -> Option<i32> {
        match self.stats.get(&id) {
            Some(StatValue::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn stat_str(&self, id: u8) -> Option<&str> {
        match self.stats.get(&id) {
            Some(StatValue::Str(s)) => Some(s),
            _ => None,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.stat_str(stat::NAME).filter(|n| !n.is_empty())
    }

    /// Condition effects, first and second batch.
    pub fn conditions(&self) -> [u32; 2] {
        [self.stat_int(29).unwrap_or(0) as u32, self.stat_int(120).unwrap_or(0) as u32]
    }

    fn apply_stats(&mut self, stats: &[StatData]) {
        for s in stats {
            self.stats.insert(s.id, s.value.clone());
        }
    }
}

/// Numbers each World made, so caches can tell a new map from the old one.
static NEXT_WORLD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct World {
    pub width: i32,
    pub height: i32,
    pub name: String,
    /// Different for every World made.
    id: u64,
    tiles: Vec<u16>,
    /// The squares whose tile was set or changed, in order (for caches of what's drawn there).
    tile_log: Vec<u32>,
    /// Squares a tile has been set on (`Map.squares_` entries that exist).
    known: Vec<bool>,
    /// The static object standing on each square (`Square.obj_`).
    occupant: rustc_hash::FxHashMap<(i32, i32), i32>,
    pub objects: rustc_hash::FxHashMap<i32, GameObj>,
    /// Our player's object id.
    pub me: Option<i32>,
    pub last_tick_id: i32,
    /// The ground's effect on our speed (`Player.moveMultiplier_`), and sinking.
    move_multiplier: f64,
    sink_level: i32,
    pub projectiles: Vec<Projectile>,
    /// Where projectiles struck since the app last took them (for the sparks).
    pub impacts: Vec<combat::Impact>,
    /// Seeded with the map's seed: shot and ground damage rolls, in step with the server's.
    pub random: Random,
    /// Our next shot's bullet id (`GameObject.getBulletId`, from 1).
    pub next_bullet_id: i32,
    anim_seed: u32,
    last_ground_damage: HashMap<(i32, i32), i32>,
}

impl World {
    pub fn new(width: i32, height: i32, name: &str, seed: u32) -> Self {
        let n = (width.max(0) * height.max(0)) as usize;
        Self {
            width,
            height,
            name: name.to_owned(),
            id: NEXT_WORLD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            tiles: vec![NO_TILE; n],
            tile_log: Vec::new(),
            known: vec![false; n],
            occupant: Default::default(),
            objects: Default::default(),
            me: None,
            last_tick_id: -1,
            move_multiplier: 1.0,
            sink_level: 0,
            projectiles: Vec::new(),
            impacts: Vec::new(),
            random: Random::new(seed),
            next_bullet_id: 1,
            anim_seed: 0x9E37_79B9,
            last_ground_damage: HashMap::new(),
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| (x + y * self.width) as usize)
    }

    /// The tile type at a square, if one has been sent.
    pub fn tile(&self, x: i32, y: i32) -> Option<u16> {
        self.index(x, y).filter(|&i| self.known[i]).map(|i| self.tiles[i])
    }

    pub fn set_tile(&mut self, x: i32, y: i32, tile_type: u16) {
        if let Some(i) = self.index(x, y) {
            if !self.known[i] || self.tiles[i] != tile_type {
                self.tile_log.push(i as u32);
            }
            self.tiles[i] = tile_type;
            self.known[i] = true;
        }
    }

    /// Different for every World made.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The squares (`x + y * width`) whose tile was set or changed, oldest first.
    pub fn tile_log(&self) -> &[u32] {
        &self.tile_log
    }

    pub fn occupant(&self, x: i32, y: i32) -> Option<&GameObj> {
        self.occupant.get(&(x, y)).and_then(|id| self.objects.get(id))
    }

    pub fn player(&self) -> Option<&GameObj> {
        self.me.and_then(|id| self.objects.get(&id))
    }

    /// `Update`: new tiles, new objects, removed objects.
    pub fn apply_update(&mut self, update: &Update, data: &GameData, time: i32) {
        for t in update.tiles.iter() {
            self.set_tile(t.x as i32, t.y as i32, t.tile_type as u16);
        }
        for o in update.new_objs.iter() {
            self.add_object(o, data, time);
        }
        for id in update.drops.iter() {
            self.remove_object(*id);
        }
    }

    fn add_object(&mut self, o: &ObjectData, data: &GameData, time: i32) {
        let (x, y) = (o.status.pos.x as f64, o.status.pos.y as f64);
        let id = o.status.object_id;
        self.remove_object(id);
        let mut obj = GameObj {
            id,
            object_type: o.object_type as u16,
            x,
            y,
            tick_x: x,
            tick_y: y,
            pos_at_tick_x: x,
            pos_at_tick_y: y,
            move_x: 0.0,
            move_y: 0.0,
            last_tick_update_time: time,
            my_last_tick_id: self.last_tick_id,
            stats: HashMap::new(),
            moving: false,
            facing: 0.0,
            attack_start: i32::MIN / 2,
            attack_angle: 0.0,
            dead: false,
            animations: Default::default(),
            anim_frame: None,
        };
        obj.apply_stats(&o.status.stats);
        self.objects.insert(id, obj);
        self.place(id, x, y, data);
    }

    pub fn remove_object(&mut self, id: i32) {
        if let Some(obj) = self.objects.remove(&id) {
            let key = (obj.x as i32, obj.y as i32);
            if self.occupant.get(&key) == Some(&id) {
                self.occupant.remove(&key);
            }
        }
    }

    /// `GameObject.moveTo`: set the position, and for static objects claim the square.
    fn place(&mut self, id: i32, x: f64, y: f64, data: &GameData) -> bool {
        if x < 0.0 || y < 0.0 || x >= self.width as f64 || y >= self.height as f64 {
            return false;
        }
        let Some(obj) = self.objects.get_mut(&id) else { return false };
        let old = (obj.x as i32, obj.y as i32);
        obj.x = x;
        obj.y = y;
        let is_static = data.objects.get(&obj.object_type).is_some_and(|d| d.is_static);
        if is_static {
            if self.occupant.get(&old) == Some(&id) {
                self.occupant.remove(&old);
            }
            self.occupant.insert((x as i32, y as i32), id);
        }
        true
    }

    /// `NewTick`: other objects head for their new positions over the tick; stats change.
    pub fn apply_new_tick(&mut self, tick: &NewTick, time: i32) {
        for s in tick.statuses.iter() {
            let is_me = Some(s.object_id) == self.me;
            let last_tick_id = self.last_tick_id;
            let Some(obj) = self.objects.get_mut(&s.object_id) else { continue };
            if tick.tick_time != 0 && !is_me {
                // GameObject.onTickPos
                if obj.my_last_tick_id < last_tick_id {
                    obj.x = obj.tick_x;
                    obj.y = obj.tick_y;
                }
                obj.last_tick_update_time = time;
                obj.tick_x = s.pos.x as f64;
                obj.tick_y = s.pos.y as f64;
                obj.pos_at_tick_x = obj.x;
                obj.pos_at_tick_y = obj.y;
                obj.move_x = (obj.tick_x - obj.pos_at_tick_x) / tick.tick_time as f64;
                obj.move_y = (obj.tick_y - obj.pos_at_tick_y) / tick.tick_time as f64;
                obj.my_last_tick_id = tick.tick_id;
            }
            obj.apply_stats(&s.stats);
        }
        self.last_tick_id = tick.tick_id;
    }

    /// `Goto`: an object (often us) is moved at once.
    pub fn apply_goto(&mut self, id: i32, x: f64, y: f64, time: i32, data: &GameData) {
        if self.place(id, x, y, data) {
            if let Some(o) = self.objects.get_mut(&id) {
                o.last_tick_update_time = time;
                (o.tick_x, o.tick_y, o.pos_at_tick_x, o.pos_at_tick_y) = (x, y, x, y);
                (o.move_x, o.move_y) = (0.0, 0.0);
            }
        }
    }

    /// Advance to `time`: other objects glide toward their tick positions; our player walks.
    /// Returns our new position if it changed.
    pub fn update(&mut self, time: i32, dt: i32, input: &MoveInput, data: &GameData) -> Option<(f64, f64)> {
        self.animate(time, data);
        let ids: Vec<i32> = self.objects.keys().copied().filter(|&id| Some(id) != self.me).collect();
        for id in ids {
            let last_tick_id = self.last_tick_id;
            let Some(o) = self.objects.get_mut(&id) else { continue };
            o.moving = false;
            if o.move_x == 0.0 && o.move_y == 0.0 {
                continue;
            }
            let (nx, ny) = if o.my_last_tick_id < last_tick_id {
                (o.move_x, o.move_y) = (0.0, 0.0);
                (o.tick_x, o.tick_y)
            } else {
                let tick_dt = (time - o.last_tick_update_time) as f64;
                o.moving = true;
                o.facing = o.move_y.atan2(o.move_x);
                (o.pos_at_tick_x + tick_dt * o.move_x, o.pos_at_tick_y + tick_dt * o.move_y)
            };
            self.place(id, nx, ny, data);
        }
        self.walk(dt, input, data)
    }
}

impl World {
    /// Steps `<Animation>`s (`Animations.getTexture`); characters with walk frames have none.
    fn animate(&mut self, time: i32, data: &GameData) {
        let seed = &mut self.anim_seed;
        let mut random = || {
            // xorshift32: the AIR client uses Math.random here, so any source will do.
            *seed ^= *seed << 13;
            *seed ^= *seed >> 17;
            *seed ^= *seed << 5;
            (*seed >> 8) as f64 / (1u32 << 24) as f64
        };
        for o in self.objects.values_mut() {
            let Some(desc) = data.objects.get(&o.object_type) else { continue };
            if !desc.animations.is_empty() {
                o.anim_frame = o.animations.frame(&desc.animations, time, &mut random);
            }
        }
    }
}

#[cfg(test)]
mod tests;
