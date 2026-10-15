//! Particles (`objects/particles`): the sparks of projectile hits and deaths, the server's
//! effects (`ShowEffect`: heals, novas, thrown grenades, lightning...) and area attacks (`Aoe`).
//!
//! The AIR client adds each effect to the map as an object that spawns particles; here an
//! effect spawns its particles at once into one list. A particle is a small square of colour
//! with a dark outline (`TextureRedrawer.redrawSolidSquare`), standing `z` tiles above its spot.
//! Sizes are the AIR client's (`size / 100 × 5` pixels at 50 pixels a tile) and scale with the map.

use crate::sprites::Sprites;
use crate::view::Camera;
use bsk_data::{GameData, TextureData, TextureRef};
use bsk_proto::ShowEffect;
use bsk_render::{Renderer, Tint};
use bsk_world::{Impact, World};
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Clone, Debug)]
enum Motion {
    /// Moves by its velocity.
    Plain,
    /// Moves, shrinking as it ages (`SparkParticle`).
    Spark,
    /// Moves, leaving sparks behind (`SparkerParticle`).
    Sparker,
    /// Arcs up and down while it flies, leaving sparks; not drawn itself (`ThrowParticle`).
    Throw,
    /// Stays beside an object as it rises (`HealParticle`).
    Follow { id: i32, angle: f64, dist: f64 },
    /// Drifts along a line, swaying (`StreamParticle`).
    Stream { px: f64, py: f64, xd: f64, yd: f64, period: f64 },
    /// Speeds toward an object until it reaches it (`FlowParticle`).
    Flow { id: i32, max_dist: f64, speed: f64 },
}

#[derive(Clone, Debug)]
struct Particle {
    x: f64,
    y: f64,
    z: f64,
    /// Tiles a millisecond.
    vx: f64,
    vy: f64,
    vz: f64,
    color: u32,
    /// The AIR client's size: `size / 100 × 5` pixels.
    size: f64,
    life: i32,
    left: i32,
    motion: Motion,
}

impl Particle {
    fn new(x: f64, y: f64, z: f64, color: u32, size: f64, life: i32, motion: Motion) -> Self {
        Particle { x, y, z, vx: 0.0, vy: 0.0, vz: 0.0, color, size, life, left: life, motion }
    }

    fn moving(mut self, vx: f64, vy: f64, vz: f64) -> Self {
        (self.vx, self.vy, self.vz) = (vx, vy, vz);
        self
    }
}

/// `FlashDescription`: an object tinted toward a colour, pulsing.
#[derive(Clone, Copy, Debug)]
struct Flash {
    start: i32,
    color: u32,
    period: i32,
    repeats: i32,
}

/// A hit whose colours come from a texture, worked out when the sprites are at hand.
#[derive(Clone, Debug)]
struct Pending {
    impact: Impact,
    /// The struck object's type and size, if it was an object.
    target: Option<(u16, i32)>,
}

/// Every live particle, and the effects on objects and the camera.
#[derive(Default)]
pub struct Particles {
    list: Vec<Particle>,
    pending: Vec<Pending>,
    /// Texture colours by object type (`BloodComposition`), and by projectile look.
    colors: HashMap<u16, Vec<u32>>,
    flashes: HashMap<i32, Flash>,
    /// `Camera.startJitter`: when the shaking started.
    jitter_since: Option<i32>,
    rng: u64,
}

/// At most this many particles at once, so a busy fight stays smooth.
const MAX_PARTICLES: usize = 6000;

impl Particles {
    /// A random number in [0, 1).
    fn random(&mut self) -> f64 {
        if self.rng == 0 {
            self.rng = 0x9E37_79B9_7F4A_7C15;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// `RandomUtil.plusMinus`.
    fn pm(&mut self, range: f64) -> f64 {
        self.random() * range * 2.0 - range
    }

    fn add(&mut self, p: Particle) {
        if self.list.len() < MAX_PARTICLES {
            self.list.push(p);
        }
    }

    /// A new world: nothing carries over.
    pub fn clear(&mut self) {
        self.list.clear();
        self.pending.clear();
        self.flashes.clear();
        self.jitter_since = None;
    }

    /// `SparkerParticle`s from `start` to `end`.
    fn sparker(&mut self, start: (f64, f64), end: (f64, f64), color: u32, size: f64, life: i32) {
        let life = life.max(1);
        let (vx, vy) = ((end.0 - start.0) / life as f64, (end.1 - start.1) / life as f64);
        self.add(Particle::new(start.0, start.1, 0.0, color, size, life, Motion::Sparker).moving(vx, vy, 0.0));
    }

    #[allow(clippy::too_many_arguments)]
    /// `SparkParticle`: drifting `dx`, `dy` tiles a second, shrinking.
    fn spark(&mut self, x: f64, y: f64, z: f64, color: u32, size: f64, life: i32, dx: f64, dy: f64) {
        self.add(Particle::new(x, y, z, color, size, life, Motion::Spark).moving(dx / 1000.0, dy / 1000.0, 0.0));
    }

    /// `AOEEffect` (and `NovaEffect`): sparkers out to the edge of a circle.
    fn nova(&mut self, x: f64, y: f64, radius: f64, color: u32, size: f64) {
        let n = (4.0 + radius * 2.0) as i32;
        for i in 0..n {
            let a = i as f64 * 2.0 * PI / n as f64;
            self.sparker((x, y), (x + radius * a.cos(), y + radius * a.sin()), color, size, 200);
        }
    }

    /// `Aoe`: the blast of an area attack.
    pub fn aoe(&mut self, x: f64, y: f64, radius: f64, color: u32) {
        self.nova(x, y, radius, color & 0xFF_FFFF, 200.0);
    }

    /// Projectiles struck something (the world's `impacts`): sparks once the colours are known.
    pub fn impacts(&mut self, world: &mut World) {
        for impact in world.impacts.drain(..) {
            let target = impact.target.and_then(|id| world.objects.get(&id)).map(|o| (o.object_type, o.stat_int(2).unwrap_or(100)));
            if impact.target.is_some() && target.is_none() {
                continue;
            }
            self.pending.push(Pending { impact, target });
        }
    }

    /// `Damage` from the server for someone else's hit: an explosion of the target's colours.
    pub fn damaged(&mut self, world: &World, target_id: i32, kill: bool) {
        let Some(o) = world.objects.get(&target_id) else { return };
        let impact = Impact { x: o.x, y: o.y, target: Some(target_id), kill, look: String::new(), angle: 0.0, speed: 0 };
        self.pending.push(Pending { impact, target: Some((o.object_type, o.stat_int(2).unwrap_or(100))) });
    }

    /// Turn the hits waiting for their colours into sparks (`HitEffect`, `ExplosionEffect`).
    pub fn realize(&mut self, sprites: &Sprites, data: &GameData) {
        for Pending { impact, target } in std::mem::take(&mut self.pending) {
            let (colors, size, count) = match target {
                Some((t, size)) => (self.blood(sprites, data, t), size as f64, if impact.kill { 30 } else { 10 }),
                None => {
                    let Some(&t) = data.object_ids.get(&impact.look) else { continue };
                    (self.texture_colors(sprites, data, t), 100.0, 3)
                }
            };
            if colors.is_empty() {
                continue;
            }
            let explode = impact.kill || impact.speed == 0;
            for _ in 0..count {
                let color = colors[(self.random() * colors.len() as f64) as usize % colors.len()];
                let life = 200 + (self.random() * 100.0) as i32;
                let (vx, vy) = if explode {
                    (self.random() - 0.5, self.random() - 0.5)
                } else {
                    // Back the way the projectile came (`HitEffect`).
                    let s = impact.speed as f64 / 600.0;
                    (s * (impact.angle + PI).cos() + (self.random() - 0.5) * 0.4, s * (impact.angle + PI).sin() + (self.random() - 0.5) * 0.4)
                };
                let p = Particle::new(impact.x, impact.y, 0.5, color, size, life, Motion::Plain).moving(vx * 0.008, vy * 0.008, 0.0);
                self.add(p);
            }
        }
    }

    /// `BloodComposition.getBloodComposition`: the texture's colours, some swapped for blood.
    fn blood(&mut self, sprites: &Sprites, data: &GameData, object_type: u16) -> Vec<u32> {
        let colors = self.texture_colors(sprites, data, object_type);
        let Some(d) = data.objects.get(&object_type) else { return colors };
        let prob: f64 = d.xml.text_of("BloodProb").and_then(|p| p.trim().parse().ok()).unwrap_or(0.0);
        if prob <= 0.0 {
            return colors;
        }
        let blood = d.xml.text_of("BloodColor").map_or(0xFF0000, |c| bsk_data::xml::as3_int(c) as u32) & 0xFF_FFFF;
        (0..colors.len()).map(|i| if self.random() < prob { blood } else { colors[i] }).collect()
    }

    /// Every opaque pixel's colour in an object's texture (`BloodComposition.getColors`).
    fn texture_colors(&mut self, sprites: &Sprites, data: &GameData, object_type: u16) -> Vec<u32> {
        if let Some(c) = self.colors.get(&object_type) {
            return c.clone();
        }
        let colors = data.objects.get(&object_type).map_or_else(Vec::new, |d| pixels(sprites, &d.texture));
        self.colors.insert(object_type, colors.clone());
        colors
    }

    /// `GameServerConnection.onShowEffect`.
    pub fn show_effect(&mut self, e: &ShowEffect, world: &World, now: i32) {
        let color = e.color & 0xFF_FFFF;
        let at = |id: i32| world.objects.get(&id).map(|o| (o.x, o.y));
        let (p1, p2) = ((e.pos1.x as f64, e.pos1.y as f64), (e.pos2.x as f64, e.pos2.y as f64));
        let go = at(e.target_object_id);
        match e.effect_type {
            // HealEffect: ten rising squares around the object.
            1 => {
                let Some(_) = go else { return };
                for i in 0..10 {
                    let angle = 2.0 * PI * i as f64 / 10.0;
                    let size = (3 + (self.random() * 5.0) as i32) as f64 * 20.0;
                    let dist = 0.3 + 0.4 * self.random();
                    let (z, vz) = (self.random() * 0.3, 0.1 + self.random() * 0.1);
                    let motion = Motion::Follow { id: e.target_object_id, angle, dist };
                    self.add(Particle::new(0.0, 0.0, z, color, size, 1000, motion).moving(0.0, 0.0, vz * 0.008));
                }
            }
            // TeleportEffect: blue squares rising where it happened.
            2 => {
                for _ in 0..20 {
                    let (angle, dist) = (2.0 * PI * self.random(), 0.7 * self.random());
                    let life = 500 + (1000.0 * self.random()) as i32;
                    let p = Particle::new(p1.0 + dist * angle.cos(), p1.1 + dist * angle.sin(), 0.0, 0x0000FF, 50.0, life, Motion::Plain);
                    self.add(p.moving(0.0, 0.0, 0.1 * 0.008));
                }
            }
            // StreamEffect: squares drifting from pos1 to pos2.
            3 => {
                for _ in 0..5 {
                    let size = (3 + (self.random() * 5.0) as i32) as f64 * 20.0;
                    let life = 1500 + (self.random() * 3000.0) as i32;
                    let (dx, dy) = ((p2.0 - p1.0) / life as f64, (p2.1 - p1.1) / life as f64);
                    let speed = (dx * dx + dy * dy).sqrt().max(1e-9);
                    let motion = Motion::Stream { px: p1.0, py: p1.1, xd: dy / speed * 0.25, yd: -dx / speed * 0.25, period: 0.25 + self.random() * 0.5 };
                    self.add(Particle::new(p1.0, p1.1, 1.85, color, size, life, motion).moving(dx, dy, 0.0));
                }
            }
            // ThrowEffect: a grenade's arc from the thrower to where it lands.
            4 => {
                let start = go.unwrap_or(p2);
                let life = if e.duration == 0 { 1500 } else { e.duration * 1000 };
                let (vx, vy) = ((p1.0 - start.0) / life as f64, (p1.1 - start.1) / life as f64);
                self.add(Particle::new(start.0, start.1, 0.0, color, 0.0, life, Motion::Throw).moving(vx, vy, 0.0));
            }
            // NovaEffect: radius in pos1.x.
            5 => {
                if let Some((x, y)) = go {
                    self.nova(x, y, p1.0, color, 10.0);
                }
            }
            // PoisonEffect: sparks bursting off the object.
            6 => {
                if let Some((x, y)) = go {
                    for _ in 0..10 {
                        let (dx, dy) = (self.pm(4.0), self.pm(4.0));
                        self.spark(x, y, 0.75, color, 100.0, 400, dx, dy);
                    }
                }
            }
            // LineEffect: sparks along the line from the object to pos1.
            7 => {
                if let Some((x, y)) = go {
                    for i in 0..30 {
                        let t = i as f64 / 30.0;
                        let (dx, dy) = (self.pm(1.0), self.pm(1.0));
                        self.spark(x + (p1.0 - x) * t, y + (p1.1 - y) * t, 0.5, color, 100.0, 700, dx, dy);
                    }
                }
            }
            // BurstEffect: from pos1 out to the circle through pos2.
            8 => {
                if go.is_some() {
                    let r = (p2.0 - p1.0).hypot(p2.1 - p1.1);
                    for i in 0..24 {
                        let a = i as f64 * 2.0 * PI / 24.0;
                        let life = 100 + (self.random() * 200.0) as i32;
                        self.sparker(p1, (p1.0 + r * a.cos(), p1.1 + r * a.sin()), color, 100.0, life);
                    }
                }
            }
            // FlowEffect: squares speeding from pos1 to the object.
            9 => {
                if let Some((x, y)) = go {
                    for _ in 0..5 {
                        let size = (3 + (self.random() * 5.0) as i32) as f64 * 20.0;
                        let max_dist = (x - p1.0).hypot(y - p1.1);
                        let motion = Motion::Flow { id: e.target_object_id, max_dist, speed: self.random() * 5.0 };
                        self.add(Particle::new(p1.0, p1.1, 0.5, color, size, 10_000, motion));
                    }
                }
            }
            // RingEffect: a thin ring of radius pos1.x.
            10 => {
                if let Some((x, y)) = go {
                    let r = p1.0;
                    for i in 0..12 {
                        let a = i as f64 * 2.0 * PI / 12.0;
                        let (c, s) = (a.cos(), a.sin());
                        self.sparker((x + r * 0.9 * c, y + r * 0.9 * s), (x + r * c, y + r * s), color, 0.0, 200);
                    }
                }
            }
            // LightningEffect: a jagged line of sparks to pos1, pos2.x their size.
            11 => {
                if let Some((x, y)) = go {
                    let dist = (p1.0 - x).hypot(p1.1 - y);
                    let n = ((dist * 3.0) as i32).max(1);
                    for i in 0..n {
                        let t = i as f64 / n as f64;
                        let k = i.min(n - i) as f64;
                        let (jx, jy) = (self.pm(dist / 200.0 * k), self.pm(dist / 200.0 * k));
                        let life = (1000.0 - t * 900.0) as i32;
                        self.spark(x + (p1.0 - x) * t + jx, y + (p1.1 - y) * t + jy, 0.5, color, p2.0, life, 0.0, 0.0);
                    }
                }
            }
            // CollapseEffect: from the circle through pos2 into pos1.
            12 => {
                if go.is_some() {
                    let r = (p2.0 - p1.0).hypot(p2.1 - p1.1);
                    for i in 0..24 {
                        let a = i as f64 * 2.0 * PI / 24.0;
                        self.sparker((p1.0 + r * a.cos(), p1.1 + r * a.sin()), p1, color, 300.0, 200);
                    }
                }
            }
            // ConeBlastEffect: a 60° fan toward pos1, radius pos2.x.
            13 => {
                if let Some((x, y)) = go {
                    let aim = (p1.1 - y).atan2(p1.0 - x);
                    let arc = PI / 3.0;
                    for i in 0..7 {
                        let a = aim - arc / 2.0 + i as f64 * arc / 7.0;
                        self.sparker((x, y), (x + p2.0 * a.cos(), y + p2.0 * a.sin()), color, 200.0, 100);
                    }
                }
            }
            // The camera shakes (`Camera.startJitter`).
            14 => {
                self.jitter_since.get_or_insert(now);
            }
            // FlashDescription: period pos1.x seconds, pos1.y times.
            15 if go.is_some() => {
                let flash = Flash { start: now, color, period: ((p1.0 * 1000.0) as i32).max(1), repeats: p1.1 as i32 };
                self.flashes.insert(e.target_object_id, flash);
            }
            // ThrowProjectileEffect: an object thrown in an arc; drawn as a throw.
            16 => {
                let life = if e.duration > 0 { e.duration } else { 1000 };
                let (vx, vy) = ((p1.0 - p2.0) / life as f64, (p1.1 - p2.1) / life as f64);
                self.add(Particle::new(p2.0, p2.1, 0.0, 0xFFFFFF, 0.0, life, Motion::Throw).moving(vx, vy, 0.0));
            }
            // InspireEffect: the object flashes gold.
            17 if go.is_some() => {
                let flash = Flash { start: now, color, period: ((p2.0 * 1000.0) as i32).max(1), repeats: (p2.1 as i32).max(1) };
                self.flashes.insert(e.target_object_id, flash);
            }
            _ => {}
        }
    }

    /// `--effect <type>` (tests): show that effect on us (0: an area attack beside us).
    pub fn test_effect(&mut self, effect_type: u8, world: &World, now: i32) {
        let Some(me) = world.player() else { return };
        let (x, y) = (me.x as f32, me.y as f32);
        if effect_type == 0 {
            self.aoe(me.x + 2.0, me.y, 2.0, 0xFF0000);
            return;
        }
        let e = ShowEffect {
            effect_type,
            target_object_id: me.id,
            pos1: bsk_proto::WorldPos { x: x + 3.0, y: y - 2.0 },
            pos2: bsk_proto::WorldPos { x: if matches!(effect_type, 11 | 13) { 3.0 } else { x + 1.5 }, y },
            color: 0xFF40C0FF,
            duration: 0,
            object_type: 0,
        };
        let e = match effect_type {
            5 | 10 => ShowEffect { pos1: bsk_proto::WorldPos { x: 3.0, y: 0.0 }, ..e },
            8 | 12 => ShowEffect { pos1: bsk_proto::WorldPos { x, y }, pos2: bsk_proto::WorldPos { x: x + 2.5, y }, ..e },
            15 => ShowEffect { pos1: bsk_proto::WorldPos { x: 0.5, y: 4.0 }, ..e },
            _ => e,
        };
        self.show_effect(&e, world, now);
    }

    /// Move every particle `dt` ms on; drop the finished ones.
    pub fn update(&mut self, world: &World, now: i32, dt: i32) {
        let dt = dt.max(0);
        let mut spawned = Vec::new();
        let mut list = std::mem::take(&mut self.list);
        list.retain_mut(|p| {
            p.left -= dt;
            if p.left <= 0 {
                return false;
            }
            let d = dt as f64;
            match p.motion {
                Motion::Plain | Motion::Spark => {
                    p.x += p.vx * d;
                    p.y += p.vy * d;
                    p.z += p.vz * d;
                }
                Motion::Sparker => {
                    p.x += p.vx * d;
                    p.y += p.vy * d;
                    spawned.push((p.x, p.y, p.z, p.color, 100.0 * (p.z + 1.0), 600));
                }
                Motion::Throw => {
                    p.z = (p.left as f64 / p.life as f64 * PI).sin() * 2.0;
                    p.x += p.vx * d;
                    p.y += p.vy * d;
                    spawned.push((p.x, p.y, p.z, p.color, 100.0 * (p.z + 1.0), 400));
                }
                Motion::Follow { id, angle, dist } => {
                    let Some(o) = world.objects.get(&id) else { return false };
                    p.x = o.x + dist * angle.cos();
                    p.y = o.y + dist * angle.sin();
                    p.z += p.vz * d;
                }
                Motion::Stream { ref mut px, ref mut py, xd, yd, period } => {
                    *px += p.vx * d;
                    *py += p.vy * d;
                    let f = (p.left as f64 / 1000.0 / period).sin();
                    p.x = *px + xd * f;
                    p.y = *py + yd * f;
                }
                Motion::Flow { id, ref mut max_dist, ref mut speed } => {
                    let Some(o) = world.objects.get(&id) else { return false };
                    let to = (o.x - p.x).hypot(o.y - p.y);
                    if to < 0.5 {
                        return false;
                    }
                    *speed += 8.0 * d / 1000.0;
                    *max_dist -= *speed * d / 1000.0;
                    let target = (to - *speed * d / 1000.0).min(*max_dist).max(0.0);
                    let k = target / to;
                    p.x = o.x - (o.x - p.x) * k;
                    p.y = o.y - (o.y - p.y) * k;
                }
            }
            true
        });
        self.list = list;
        for (x, y, z, color, size, life) in spawned {
            let (dx, dy) = (self.pm(1.0), self.pm(1.0));
            self.spark(x, y, z, color, size, life, dx, dy);
        }
        self.flashes.retain(|_, f| now <= f.start + f.period * f.repeats);
    }

    /// `Camera.update`'s shake: up to half a tile, building up over ten seconds.
    pub fn jitter(&mut self, now: i32) -> (f64, f64) {
        let Some(since) = self.jitter_since else { return (0.0, 0.0) };
        let j = ((now - since) as f64 * 0.5 / 10_000.0).min(0.5);
        (self.pm(j), self.pm(j))
    }

    /// `FlashDescription.apply`: the tint an object wears now, if it flashes.
    pub fn flash_tint(&self, object_id: i32, now: i32) -> Option<Tint> {
        let f = self.flashes.get(&object_id)?;
        let t = (now - f.start).rem_euclid(f.period) as f32 / f.period as f32;
        let k = (t * std::f32::consts::PI).sin() * 0.5;
        let [r, g, b] = bsk_render::rgb_f32(f.color);
        Some(Tint { mul: [1.0 - k, 1.0 - k, 1.0 - k, 1.0], add: [k * r, k * g, k * b] })
    }

    /// Draw every particle: a square of colour with a dark edge.
    pub fn draw(&self, cam: &Camera, renderer: &mut Renderer) {
        let k = cam.scale / 50.0;
        for p in &self.list {
            let size = match p.motion {
                Motion::Throw => continue,
                Motion::Spark => p.size * p.left as f64 / p.life as f64,
                _ => p.size,
            };
            let side = (size / 100.0 * 5.0).floor() * k;
            let [sx, sy] = cam.to_screen(p.x, p.y, p.z);
            let edge = (1.0 * k).max(1.0) as f32;
            let half = (side / 2.0) as f32;
            renderer.fill(sx - half - edge, sy - half - edge, 2.0 * (half + edge), 2.0 * (half + edge), 0x000000, 0.6);
            if side > 0.0 {
                renderer.fill(sx - half, sy - half, 2.0 * half, 2.0 * half, p.color, 1.0);
            }
        }
    }
}

/// The colours of every opaque pixel of a texture's first frame.
fn pixels(sprites: &Sprites, tex: &TextureData) -> Vec<u32> {
    let src = match &tex.pick(0).main {
        TextureRef::Image { set, index } => sprites.tile_src(set, *index),
        TextureRef::Animated { set, index } => sprites.char_src(set, *index, 0, 0),
        _ => None,
    };
    let Some(src) = src else { return Vec::new() };
    sprites.pixels(src).pixels().filter(|p| p[3] != 0).map(|p| u32::from_be_bytes([0, p[0], p[1], p[2]])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsk_proto::WorldPos;

    fn effect(effect_type: u8, target: i32, pos1: (f32, f32), pos2: (f32, f32)) -> ShowEffect {
        ShowEffect {
            effect_type,
            target_object_id: target,
            pos1: WorldPos { x: pos1.0, y: pos1.1 },
            pos2: WorldPos { x: pos2.0, y: pos2.1 },
            color: 0xFFFF0000,
            duration: 0,
            object_type: 0,
        }
    }

    #[test]
    fn effects_spawn_and_fade() {
        let world = World::new(10, 10, "test", 1);
        let mut p = Particles::default();
        // A grenade (no thrower known: from pos2), and an area attack.
        p.show_effect(&effect(4, 99, (5.0, 5.0), (1.0, 1.0)), &world, 0);
        p.aoe(5.0, 5.0, 2.0, 0xFFFF0000);
        assert_eq!(p.list.len(), 1 + 8);
        p.update(&world, 100, 100);
        // The throw and the sparkers leave sparks behind.
        assert!(p.list.len() > 9);
        p.update(&world, 2100, 2000);
        assert!(p.list.is_empty());
        // Effects on unknown objects do nothing.
        p.show_effect(&effect(5, 99, (3.0, 0.0), (0.0, 0.0)), &world, 0);
        assert!(p.list.is_empty());
    }

    #[test]
    fn flash_and_jitter() {
        let world = World::new(10, 10, "test", 1);
        let mut p = Particles::default();
        p.show_effect(&effect(14, 0, (0.0, 0.0), (0.0, 0.0)), &world, 1000);
        let (jx, jy) = p.jitter(21_000);
        assert!(jx.abs() <= 0.5 && jy.abs() <= 0.5);
        assert!(p.flash_tint(7, 0).is_none());
    }
}
