//! Drawing the world (`Map.draw`): ground tiles, then shadows, then objects and walls sorted by
//! their screen depth.
//!
//! The camera (`Camera.configure`, without perspective) maps a world point (x, y, z) to the
//! screen as `scale × (r·d, u·d − z)`, d being the offset from the camera, r = (cos a, sin a) and
//! u = (−sin a, cos a). That's affine, so every tile, wall face and sprite is one quad.

use crate::redraw::{Redrawer, PAD};
use crate::sprites::{SrcRect, Sprites};
use crate::text::{Fonts, Style, Text};
use bsk_assets::Facing;
use bsk_data::{GameData, ObjectDesc, TextureData, TextureRef};
use bsk_render::{Quad, Renderer, Tint};
use bsk_world::{GameObj, World};

/// Pixels per tile at map scale 1, as in the AIR client.
pub const TILE_PX: f64 = 50.0;

#[derive(Debug, Clone, Copy)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    /// Pixels per tile.
    pub scale: f64,
    /// Where the camera point is on the screen.
    pub cx: f64,
    pub cy: f64,
}

impl Camera {
    pub fn to_screen(&self, x: f64, y: f64, z: f64) -> [f32; 2] {
        let (s, c) = self.angle.sin_cos();
        let (dx, dy) = (x - self.x, y - self.y);
        let sx = dx * c + dy * s;
        let sy = -dx * s + dy * c - z;
        [(self.cx + sx * self.scale) as f32, (self.cy + sy * self.scale) as f32]
    }

    /// The quad through world points p0, p1 (along the texture's x) and p3 (along its y).
    fn quad(&self, p0: [f64; 3], p1: [f64; 3], p3: [f64; 3]) -> Quad {
        let o = self.to_screen(p0[0], p0[1], p0[2]);
        let a = self.to_screen(p1[0], p1[1], p1[2]);
        let b = self.to_screen(p3[0], p3[1], p3[2]);
        Quad { origin: o, x_axis: [a[0] - o[0], a[1] - o[1]], y_axis: [b[0] - o[0], b[1] - o[1]] }
    }

    /// The world area (in tiles from the camera) that can be on screen.
    fn radius(&self, w: f64, h: f64) -> f64 {
        let far_x = self.cx.max(w - self.cx);
        let far_y = self.cy.max(h - self.cy) + 2.0 * self.scale;
        (far_x * far_x + far_y * far_y).sqrt() / self.scale + 1.0
    }
}

/// `Lighting3D.shadeValue`: 0.75 ambient plus the rest from a light along (1, 3, 2).
fn shade(normal: [f64; 3]) -> f32 {
    let len = (1.0f64 + 9.0 + 4.0).sqrt();
    let light = [1.0 / len, 3.0 / len, 2.0 / len];
    let n_len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let dot = (normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2]) / n_len;
    (0.75 + 0.25 * dot.max(0.0)) as f32
}

/// `Plane3D.computeNormalVec` for a face given by its first three corners.
fn normal(v0: [f64; 3], v1: [f64; 3], v2: [f64; 3]) -> [f64; 3] {
    let u = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let v = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];
    [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]
}

pub struct Scene<'a> {
    pub sprites: &'a Sprites,
    pub data: &'a GameData,
    pub world: &'a World,
    pub camera: Camera,
    pub time: i32,
}

/// The image an object shows now, before redrawing.
struct Look {
    src: SrcRect,
    mirror: bool,
}

const SEC_TO_DIRS: [[Dir; 3]; 8] = {
    use Dir::*;
    [[Left, Up, Down], [Up, Left, Down], [Up, Right, Down], [Right, Up, Down], [Right, Down, Down], [Down, Right, Right], [Down, Left, Left], [Left, Down, Down]]
};

#[derive(Clone, Copy, PartialEq)]
enum Dir {
    Right,
    Left,
    Down,
    Up,
}

impl Scene<'_> {
    fn texture_src(&self, tex: &TextureData, pick: i32) -> Option<SrcRect> {
        match &tex.pick(pick).main {
            TextureRef::Image { set, index } => self.sprites.tile_src(set, *index),
            TextureRef::Animated { set, index } => self.sprites.char_src(set, *index, 0, 0),
            _ => None,
        }
    }

    /// `GameObject.getTexture` for animated characters: the frame for the facing (relative to
    /// the camera), walking or standing.
    fn look(&self, obj: &GameObj, desc: &ObjectDesc) -> Option<Look> {
        let tex = desc.texture.pick(obj.id);
        let TextureRef::Animated { set, index } = &tex.main else {
            return self.texture_src(tex, obj.id).map(|src| Look { src, mirror: false });
        };
        let sheet = self.sprites.char_sheet(set)?;
        let rows = sheet.rows();
        // Which row holds which direction, and which directions exist (AnimatedChar's dict_).
        let row_of = |d: Dir| -> Option<(u32, bool)> {
            match (sheet.facing, d) {
                (Facing::Right, Dir::Right) => Some((0, false)),
                (Facing::Right, Dir::Left) => Some((0, true)),
                (Facing::Right, Dir::Down) => (rows >= 2).then_some((1, false)),
                (Facing::Down, Dir::Down) => Some((0, false)),
                (Facing::Down, Dir::Right) => (rows >= 2).then_some((1, false)),
                (Facing::Down, Dir::Left) => (rows >= 2).then_some((1, true)),
                (_, Dir::Up) => (rows >= 3).then_some((2, false)),
            }
        };
        let rel = bound_to_pi(obj.facing - self.camera.angle);
        let sec = ((rel / std::f64::consts::FRAC_PI_4 + 4.0) as i32).rem_euclid(8) as usize;
        let dirs = SEC_TO_DIRS[sec];
        let (dir, (row, mirror)) = dirs.iter().find_map(|&d| row_of(d).map(|r| (d, r)))?;
        let symmetric = matches!(dir, Dir::Down | Dir::Up);

        let walking = obj.moving;
        let frame = if walking {
            // walkPer: 400 ms for any normal speed; two frames.
            let p = (self.time.rem_euclid(400)) as f64 / 400.0;
            let walk2 = self.sprites.char_src(set, *index, row, 2)?;
            if p < 0.5 || !self.sprites.is_blank(walk2) {
                if p < 0.5 { 1 } else { 2 }
            } else if symmetric {
                // walk1 mirrored stands in for a blank walk2.
                let src = self.sprites.char_src(set, *index, row, 1)?;
                return Some(Look { src, mirror: !mirror });
            } else {
                0
            }
        } else {
            0
        };
        let src = self.sprites.char_src(set, *index, row, frame)?;
        Some(Look { src, mirror })
    }
}

fn bound_to_pi(a: f64) -> f64 {
    let mut a = a;
    while a < -std::f64::consts::PI {
        a += std::f64::consts::TAU;
    }
    while a > std::f64::consts::PI {
        a -= std::f64::consts::TAU;
    }
    a
}

enum Item<'a> {
    Sprite { obj: &'a GameObj, desc: &'a ObjectDesc },
    Wall { obj: &'a GameObj, desc: &'a ObjectDesc },
}

pub fn draw(scene: &Scene, renderer: &mut Renderer, redrawer: &mut Redrawer, fonts: &Fonts, text: &mut Text) {
    let (w, h) = renderer.size();
    let cam = scene.camera;
    let radius = cam.radius(w as f64, h as f64);
    let world = scene.world;
    let (x0, x1) = ((cam.x - radius).floor() as i32, (cam.x + radius).ceil() as i32);
    let (y0, y1) = ((cam.y - radius).floor() as i32, (cam.y + radius).ceil() as i32);

    // Ground.
    for ty in y0.max(0)..=y1.min(world.height - 1) {
        for tx in x0.max(0)..=x1.min(world.width - 1) {
            let Some(tile) = world.tile(tx, ty) else { continue };
            let Some(ground) = scene.data.grounds.get(&tile) else { continue };
            let pick = bsk_data::ground::tile_hash(tx, ty);
            let Some(src) = scene.texture_src(&ground.texture, pick) else { continue };
            let Some(rect) = scene.sprites.atlas(src) else { continue };
            let (x, y) = (tx as f64, ty as f64);
            renderer.sprite(rect, cam.quad([x, y, 0.0], [x + 1.0, y, 0.0], [x, y + 1.0, 0.0]), Tint::NONE);
        }
    }

    // Objects and walls, back to front by the screen y of their ground point.
    let mut items: Vec<(f32, Item)> = Vec::new();
    for obj in world.objects.values() {
        if (obj.x - cam.x).abs() > radius || (obj.y - cam.y).abs() > radius {
            continue;
        }
        let Some(desc) = scene.data.objects.get(&obj.object_type) else { continue };
        let depth = cam.to_screen(obj.x, obj.y, 0.0)[1];
        if desc.class.is_wall() {
            items.push((depth, Item::Wall { obj, desc }));
        } else {
            items.push((depth, Item::Sprite { obj, desc }));
        }
    }
    items.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (_, item) in &items {
        match item {
            Item::Wall { obj, desc } => draw_wall(scene, renderer, obj, desc),
            Item::Sprite { obj, desc } => {
                let Some(look) = scene.look(obj, desc) else { continue };
                let size = obj.stat_int(2).unwrap_or(desc.min_size);
                let Some(sprite) = redrawer.get(scene.sprites, renderer, look.src, look.mirror, size) else { continue };
                if desc.draw_on_ground {
                    // Flat on its square, like a tile.
                    let Some(rect) = scene.sprites.atlas(look.src) else { continue };
                    let (x, y) = (obj.x.floor(), obj.y.floor());
                    renderer.sprite(rect, cam.quad([x, y, 0.0], [x + 1.0, y, 0.0], [x, y + 1.0, 0.0]), Tint::NONE);
                    continue;
                }
                // The canvas's bottom centre stands on the object (raised by its z); sprites
                // scale with the map.
                let k = (cam.scale / TILE_PX) as f32;
                let [px, py] = cam.to_screen(obj.x, obj.y, desc.z);
                let (sw, sh) = (sprite.w as f32 * k, sprite.h as f32 * k);
                renderer.sprite(sprite.rect, Quad::rect(px - sw / 2.0, py - sh, sw, sh), Tint::NONE);
            }
        }
    }

    // Names (`GameObject.drawName`): bold 16 px white, centred under the object.
    let style = Style { size: 16.0, bold: true, color: 0xFFFFFF };
    for (_, item) in &items {
        let Item::Sprite { obj, desc } = item else { continue };
        if !desc.show_name {
            continue;
        }
        let Some(name) = obj.name() else { continue };
        let [px, py] = cam.to_screen(obj.x, obj.y, 0.0);
        text.draw_centered(fonts, renderer, name, px, py, style);
    }
    let _ = PAD;
}

/// `Wall.draw`: a textured cube. Sides that face away aren't drawn; sides against another wall
/// or unknown ground are black.
fn draw_wall(scene: &Scene, renderer: &mut Renderer, obj: &GameObj, desc: &ObjectDesc) {
    let cam = scene.camera;
    let (xi, yi) = (obj.x.floor(), obj.y.floor());
    let side = scene.texture_src(&desc.texture, obj.id).and_then(|s| scene.sprites.atlas(s));
    let top = desc
        .top
        .as_ref()
        .and_then(|t| scene.texture_src(t, obj.id))
        .and_then(|s| scene.sprites.atlas(s))
        .or(side);
    let Some(side) = side else { return };

    // Sides, in Wall.addWall order: north, east, south, west; neighbours at sqX/sqY.
    let corners = [
        ([xi, yi], [xi + 1.0, yi], (0, -1)),
        ([xi + 1.0, yi], [xi + 1.0, yi + 1.0], (1, 0)),
        ([xi + 1.0, yi + 1.0], [xi, yi + 1.0], (0, 1)),
        ([xi, yi + 1.0], [xi, yi], (-1, 0)),
    ];
    for (a, b, (nx, ny)) in corners {
        let v0 = [a[0], a[1], 1.0];
        let v1 = [b[0], b[1], 1.0];
        let v3 = [a[0], a[1], 0.0];
        let quad = cam.quad(v0, v1, v3);
        // Back faces: the projected corners wind the other way.
        let cross = quad.x_axis[0] * quad.y_axis[1] - quad.x_axis[1] * quad.y_axis[0];
        if cross >= 0.0 {
            continue;
        }
        let (sx, sy) = (xi as i32 + nx, yi as i32 + ny);
        let neighbour_wall = scene
            .world
            .occupant(sx, sy)
            .and_then(|o| scene.data.objects.get(&o.object_type))
            .is_some_and(|d| d.class.is_wall());
        if scene.world.tile(sx, sy).is_none() || neighbour_wall {
            renderer.sprite(renderer.white(), quad, Tint::color(0x000000, 1.0));
            continue;
        }
        let s = shade(normal(v0, v1, [b[0], b[1], 0.0]));
        renderer.sprite(side, quad, Tint { mul: [s, s, s, 1.0], add: [0.0; 3] });
    }
    if let Some(top) = top {
        let v0 = [xi, yi, 1.0];
        let s = shade(normal(v0, [xi + 1.0, yi, 1.0], [xi + 1.0, yi + 1.0, 1.0]));
        let quad = cam.quad(v0, [xi + 1.0, yi, 1.0], [xi, yi + 1.0, 1.0]);
        renderer.sprite(top, quad, Tint { mul: [s, s, s, 1.0], add: [0.0; 3] });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_projection() {
        let cam = Camera { x: 10.0, y: 10.0, angle: 0.0, scale: 50.0, cx: 400.0, cy: 300.0 };
        assert_eq!(cam.to_screen(10.0, 10.0, 0.0), [400.0, 300.0]);
        assert_eq!(cam.to_screen(11.0, 10.0, 0.0), [450.0, 300.0]);
        assert_eq!(cam.to_screen(10.0, 11.0, 1.0), [400.0, 300.0], "one tile south, one up");
        // Turned a quarter: east goes up the screen.
        let cam = Camera { angle: std::f64::consts::FRAC_PI_2, ..cam };
        let p = cam.to_screen(11.0, 10.0, 0.0);
        assert!((p[0] - 400.0).abs() < 1e-3 && (p[1] - 250.0).abs() < 1e-3, "{p:?}");
    }

    #[test]
    fn wall_shading() {
        // Top face: normal up, 0.75 + 0.25 · 2/√14.
        let top = shade(normal([0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0]));
        assert!((top - (0.75 + 0.25 * 2.0 / 14f32.sqrt())).abs() < 1e-6);
    }
}
