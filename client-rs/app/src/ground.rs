//! Ground tiles as the AIR client draws them (`Square.rebuild3D`, `SquareFace`, `TileRedrawer`):
//! a tile whose neighbours have a higher blend priority gets their texture blended into its
//! corners through the edge masks; animated tiles (water, lava) scroll their texture.

use crate::sprites::Sprites;
use bsk_data::{AnimateKind, GameData, TextureData, TextureRef};
use bsk_render::{AtlasRect, Quad, Renderer, Tint};
use bsk_world::World;
use image::RgbaImage;
use std::collections::HashMap;

const INNER: usize = 0;
const SIDE0: usize = 1;
const SIDE1: usize = 2;
const OUTER: usize = 3;
const INNER_P1: usize = 4;
const INNER_P2: usize = 5;

/// What to draw for one square.
pub struct TileLayers {
    /// The tile's own texture (scrolling if animated), unless the blended image replaces it.
    pub base: Option<(AtlasRect, AnimateKind, f64, f64)>,
    /// The blended corners, over the base.
    pub overlay: Option<AtlasRect>,
    /// How far objects standing here sink, in sprite pixels (`Square.sink_`).
    pub sink: i32,
}

#[derive(Default)]
pub struct Ground {
    /// Per quadrant (0..4): the six mask sets, each a list of 4×4 alpha images.
    masks: Option<Vec<[Vec<RgbaImage>; 6]>>,
    cache: HashMap<([u16; 9], bool), Option<AtlasRect>>,
}

/// `BitmapUtil.rotateBitmapData` for square images: `turns` quarter turns clockwise.
fn rotate(img: &RgbaImage, turns: i32) -> RgbaImage {
    let n = img.width();
    RgbaImage::from_fn(n, n, |x, y| {
        let (sx, sy) = match turns.rem_euclid(4) {
            0 => (x, y),
            1 => (y, n - 1 - x),
            2 => (n - 1 - x, n - 1 - y),
            _ => (n - 1 - y, x),
        };
        *img.get_pixel(sx, sy)
    })
}

fn texture0(data: &GameData, sprites: &Sprites, tile: u16) -> Option<RgbaImage> {
    // GroundLibrary.getBitmapData(type): texture 0, even for random textures.
    let ground = data.grounds.get(&tile)?;
    match &ground.texture.pick(0).main {
        TextureRef::Image { set, index } => sprites.tile_src(set, *index).map(|r| sprites.pixels(r)),
        _ => None,
    }
}

fn texture_rect(sprites: &Sprites, tex: &TextureData, pick: i32) -> Option<AtlasRect> {
    match &tex.pick(pick).main {
        TextureRef::Image { set, index } => sprites.tile(set, *index),
        _ => None,
    }
}

impl Ground {
    fn masks(&mut self, sprites: &Sprites) -> &Vec<[Vec<RgbaImage>; 6]> {
        self.masks.get_or_insert_with(|| {
            let set = |name: &str| -> Vec<RgbaImage> {
                let count = sprites.set(name).map_or(0, |s| s.count);
                (0..count).filter_map(|i| sprites.tile_src(name, i)).map(|r| sprites.pixels(r)).collect()
            };
            let (inner, side, outer, p1, p2) =
                (set("inner_mask"), set("sides_mask"), set("outer_mask"), set("innerP1_mask"), set("innerP2_mask"));
            let rot = |v: &Vec<RgbaImage>, t: i32| v.iter().map(|m| rotate(m, t)).collect::<Vec<_>>();
            [-1, 0, 2, 1]
                .iter()
                .map(|&i| [rot(&inner, i), rot(&side, i - 1), rot(&side, i), rot(&outer, i), rot(&p1, i), rot(&p2, i)])
                .collect()
        })
    }

    /// The layers for square (x, y).
    pub fn layers(&mut self, world: &World, data: &GameData, sprites: &Sprites, renderer: &mut Renderer, x: i32, y: i32) -> Option<TileLayers> {
        let tile = world.tile(x, y)?;
        let ground = data.grounds.get(&tile)?;
        let animated = ground.animate.kind != AnimateKind::None;
        let base_rect = texture_rect(sprites, &ground.texture, bsk_data::ground::tile_hash(x, y));
        let overlay = self.blended(world, data, sprites, renderer, x, y, !animated);
        let sink = if ground.sink { if overlay.is_some() { 6 } else { 12 } } else { 0 };
        let base = if animated {
            base_rect.map(|r| (r, ground.animate.kind, ground.animate.dx, ground.animate.dy))
        } else if overlay.is_some() {
            // The blended image already holds the tile's own texture underneath.
            None
        } else {
            base_rect.map(|r| (r, AnimateKind::None, 0.0, 0.0))
        };
        Some(TileLayers { base, overlay, sink })
    }

    /// `TileRedrawer.redraw` for plain tiles (`getSig` and `redrawRect`).
    fn blended(&mut self, world: &World, data: &GameData, sprites: &Sprites, renderer: &mut Renderer, x: i32, y: i32, with_background: bool) -> Option<AtlasRect> {
        let tile = world.tile(x, y)?;
        let priority = data.grounds.get(&tile).map_or(-1, |g| g.blend_priority);
        let mut sig = [tile; 9];
        let mut i = 0;
        for ny in y - 1..=y + 1 {
            for nx in x - 1..=x + 1 {
                if (nx, ny) != (x, y) {
                    if let Some(n) = world.tile(nx, ny) {
                        if data.grounds.get(&n).map_or(-1, |g| g.blend_priority) > priority {
                            sig[i] = n;
                        }
                    }
                }
                i += 1;
            }
        }
        if sig.iter().all(|&t| t == tile) {
            return None;
        }
        if let Some(cached) = self.cache.get(&(sig, with_background)) {
            return *cached;
        }
        let result = self.build(data, sprites, &sig, with_background).and_then(|img| renderer.add_image(&img));
        self.cache.insert((sig, with_background), result);
        result
    }

    fn build(&mut self, data: &GameData, sprites: &Sprites, sig: &[u16; 9], with_background: bool) -> Option<RgbaImage> {
        let base = sig[4];
        let s = |i: usize| sig[i] != base;
        let mut redraw = [false; 4];
        if s(1) {
            redraw[0] = true;
            redraw[1] = true;
        }
        if s(3) {
            redraw[0] = true;
            redraw[2] = true;
        }
        if s(5) {
            redraw[1] = true;
            redraw[3] = true;
        }
        if s(7) {
            redraw[2] = true;
            redraw[3] = true;
        }
        redraw[0] |= s(0);
        redraw[1] |= s(2);
        redraw[2] |= s(6);
        redraw[3] |= s(8);
        if !redraw.iter().any(|&r| r) {
            return None;
        }
        let orig = texture0(data, sprites, base)?;
        let mut out = if with_background { orig.clone() } else { RgbaImage::new(orig.width(), orig.height()) };
        // Quadrant, its corner, and the neighbours (n0, n1, n2) around it.
        let quads = [(0, (0, 0), (3, 0, 1)), (1, (4, 0), (1, 2, 5)), (2, (0, 4), (7, 6, 3)), (3, (4, 4), (5, 8, 7))];
        let seed = sig.iter().fold(0u32, |h, &t| h.wrapping_mul(31).wrapping_add(t as u32));
        for (q, (px, py), (a, b, c)) in quads {
            if redraw[q] {
                let pick = (seed.wrapping_add(q as u32 * 7919)) as usize;
                self.redraw_rect(data, sprites, &mut out, q, px, py, base, sig[a], sig[b], sig[c], pick);
            }
        }
        Some(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn redraw_rect(&mut self, data: &GameData, sprites: &Sprites, out: &mut RgbaImage, q: usize, px: u32, py: u32, base: u16, n0: u16, n1: u16, n2: u16, pick: usize) {
        let masks = self.masks(sprites)[q].clone();
        let choose = |kind: usize| masks[kind].get(pick % masks[kind].len().max(1)).cloned();
        let (blend, mask) = if base == n0 && base == n2 {
            (n1, choose(OUTER))
        } else if base != n0 && base != n2 {
            if n0 != n2 {
                if let (Some(t0), Some(m0)) = (texture0(data, sprites, n0), choose(INNER_P1)) {
                    copy_masked(out, &t0, &m0, px, py);
                }
                if let (Some(t2), Some(m2)) = (texture0(data, sprites, n2), choose(INNER_P2)) {
                    copy_masked(out, &t2, &m2, px, py);
                }
                return;
            }
            (n0, choose(INNER))
        } else if base != n0 {
            (n0, choose(SIDE0))
        } else {
            (n2, choose(SIDE1))
        };
        if let (Some(tex), Some(mask)) = (texture0(data, sprites, blend), mask) {
            copy_masked(out, &tex, &mask, px, py);
        }
    }
}

/// `copyPixels(source, rect at (px, py), (px, py), mask, (0, 0), mergeAlpha = true)`: the 4×4
/// block of `src` at (px, py), its alpha multiplied by the mask's, laid over `out`.
fn copy_masked(out: &mut RgbaImage, src: &RgbaImage, mask: &RgbaImage, px: u32, py: u32) {
    for my in 0..mask.height().min(4) {
        for mx in 0..mask.width().min(4) {
            let (x, y) = (px + mx, py + my);
            if x >= src.width() || y >= src.height() || x >= out.width() || y >= out.height() {
                continue;
            }
            let s = src.get_pixel(x, y);
            let a = (s[3] as f32 / 255.0) * (mask.get_pixel(mx, my)[3] as f32 / 255.0);
            let d = out.get_pixel_mut(x, y);
            let da = d[3] as f32 / 255.0;
            let oa = a + da * (1.0 - a);
            if oa > 0.0 {
                for c in 0..3 {
                    d[c] = ((s[c] as f32 * a + d[c] as f32 * da * (1.0 - a)) / oa).round() as u8;
                }
            }
            d[3] = (oa * 255.0).round() as u8;
        }
    }
}

/// Draw a tile texture over `quad`, its texture shifted by (ox, oy) tiles and wrapping, as
/// `SquareFace` does for animated ground. Up to four pieces.
pub fn draw_scrolled(renderer: &mut Renderer, rect: AtlasRect, quad: Quad, ox: f64, oy: f64, tint: Tint) {
    let ox = ox.rem_euclid(1.0) as f32;
    let oy = oy.rem_euclid(1.0) as f32;
    // Along each axis: (start, end) on the tile, and where that piece starts in the texture.
    let spans = |o: f32| -> Vec<(f32, f32, f32)> {
        if o == 0.0 { vec![(0.0, 1.0, 0.0)] } else { vec![(0.0, 1.0 - o, o), (1.0 - o, 1.0, 0.0)] }
    };
    for &(x0, x1, u0) in &spans(ox) {
        for &(y0, y1, v0) in &spans(oy) {
            let origin = [
                quad.origin[0] + quad.x_axis[0] * x0 + quad.y_axis[0] * y0,
                quad.origin[1] + quad.x_axis[1] * x0 + quad.y_axis[1] * y0,
            ];
            let piece = Quad {
                origin,
                x_axis: [quad.x_axis[0] * (x1 - x0), quad.x_axis[1] * (x1 - x0)],
                y_axis: [quad.y_axis[0] * (y1 - y0), quad.y_axis[1] * (y1 - y0)],
            };
            renderer.sprite_part(rect, [u0, v0, u0 + (x1 - x0), v0 + (y1 - y0)], piece, tint);
        }
    }
}

/// `SquareFace.draw`'s texture offset for an animated tile at `time` ms.
pub fn scroll(kind: AnimateKind, dx: f64, dy: f64, time: i32) -> (f64, f64) {
    let t = time as f64 / 1000.0;
    match kind {
        AnimateKind::Wave => ((dx * t).sin(), (dy * t).sin()),
        AnimateKind::Flow => (dx * t, dy * t),
        AnimateKind::None => (0.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_is_clockwise() {
        // A pixel at the top right goes to the bottom right after one clockwise turn.
        let mut img = RgbaImage::new(4, 4);
        img.put_pixel(3, 0, image::Rgba([1, 2, 3, 255]));
        assert_eq!(rotate(&img, 1).get_pixel(3, 3)[3], 255);
        assert_eq!(rotate(&img, -1).get_pixel(0, 0)[3], 255);
        assert_eq!(rotate(&img, 2).get_pixel(0, 3)[3], 255);
    }

    #[test]
    fn masked_copy() {
        let mut out = RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]));
        let src = RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        let mut mask = RgbaImage::new(4, 4);
        mask.put_pixel(0, 0, image::Rgba([0, 0, 0, 255]));
        copy_masked(&mut out, &src, &mask, 4, 4);
        assert_eq!(*out.get_pixel(4, 4), image::Rgba([255, 0, 0, 255]), "masked in");
        assert_eq!(*out.get_pixel(5, 4), image::Rgba([0, 0, 255, 255]), "masked out");
        assert_eq!(*out.get_pixel(0, 0), image::Rgba([0, 0, 255, 255]), "other quadrant untouched");
    }
}
