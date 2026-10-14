//! Object sprites as the AIR client draws them (`TextureRedrawer.redraw` with
//! `GlowRedrawer.outlineGlow`): scaled up ×5 at size 100 on a padded canvas, with a soft black
//! outline. Results are cached in the atlas.

use crate::sprites::{Sprites, SrcRect};
use bsk_render::{AtlasRect, Renderer};
use image::{GenericImageView, Rgba, RgbaImage};
use std::collections::HashMap;

/// `TextureRedrawer.magic`: padding left, right and above the sprite, for the outline.
pub const PAD: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    src: SrcRect,
    mirror: bool,
    size: i32,
}

/// A redrawn sprite: where it is in the atlas, and its canvas size in pixels.
#[derive(Debug, Clone, Copy)]
pub struct Redrawn {
    pub rect: AtlasRect,
    pub w: u32,
    pub h: u32,
}

#[derive(Default)]
pub struct Redrawer {
    cache: HashMap<Key, Option<Redrawn>>,
    shadow: Option<AtlasRect>,
}

impl Redrawer {
    /// The shadow under objects (`GameObject.drawShadow`): a radial gradient from 50% to 0%
    /// alpha, white, to be tinted with the shadow colour.
    pub fn shadow(&mut self, renderer: &mut Renderer) -> Option<AtlasRect> {
        if self.shadow.is_none() {
            const N: u32 = 64;
            let img = RgbaImage::from_fn(N, N, |x, y| {
                let (dx, dy) = ((x as f32 + 0.5) / N as f32 * 2.0 - 1.0, (y as f32 + 0.5) / N as f32 * 2.0 - 1.0);
                let r = (dx * dx + dy * dy).sqrt();
                Rgba([255, 255, 255, (0.5 * (1.0 - r).max(0.0) * 255.0).round() as u8])
            });
            self.shadow = renderer.add_image(&img);
        }
        self.shadow
    }

    /// `src` scaled for `size` (100 = normal), mirrored left-right if asked, with its outline.
    pub fn get(
        &mut self,
        sprites: &Sprites,
        renderer: &mut Renderer,
        src: SrcRect,
        mirror: bool,
        size: i32,
    ) -> Option<Redrawn> {
        let key = Key { src, mirror, size };
        *self.cache.entry(key).or_insert_with(|| {
            let image = redraw(&sprites.pixels(src), mirror, size, 5.0);
            let (w, h) = image.dimensions();
            renderer.add_image(&image).map(|rect| Redrawn { rect, w, h })
        })
    }
}

/// `TextureRedrawer.resize` then `GlowRedrawer.outlineGlow(texture, 0)`.
pub fn redraw(src: &impl GenericImageView<Pixel = Rgba<u8>>, mirror: bool, size: i32, scale: f64) -> RgbaImage {
    let (w, h) = src.dimensions();
    let factor = scale * (size as f64 / 100.0);
    let sw = (factor * w as f64) as u32;
    let sh = (factor * h as f64) as u32;
    let mut out = RgbaImage::new(sw + 2 * PAD, sh + 1 + PAD);
    if sw > 0 && sh > 0 {
        for y in 0..sh {
            // Nearest neighbour, sampling each target pixel's centre, as Flash's draw does.
            let sy = (((y as f64 + 0.5) * h as f64 / sh as f64) as u32).min(h - 1);
            for x in 0..sw {
                let sx = (((x as f64 + 0.5) * w as f64 / sw as f64) as u32).min(w - 1);
                let sx = if mirror { w - 1 - sx } else { sx };
                out.put_pixel(x + PAD, y + PAD, src.get_pixel(sx, sy));
            }
        }
    }
    outline(&mut out);
    out
}

/// The outline: `GlowFilter(black, 0.8 alpha, blur 1.4, strength 255)` saturates every pixel
/// next to a visible one (diagonals included) to 80% black, behind the sprite.
fn outline(img: &mut RgbaImage) {
    const GLOW: f32 = 0.8;
    let (w, h) = img.dimensions();
    let alpha: Vec<u8> = img.pixels().map(|p| p[3]).collect();
    let at = |x: i32, y: i32| -> u8 {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 { 0 } else { alpha[(y as u32 * w + x as u32) as usize] }
    };
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut near = 0u8;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    near = near.max(at(x + dx, y + dy));
                }
            }
            if near == 0 {
                continue;
            }
            // Sprite over glow: out = src + glow * (1 - src alpha).
            let glow_a = GLOW * (near as f32 / 255.0).min(1.0).ceil();
            let p = img.get_pixel_mut(x as u32, y as u32);
            let sa = p[3] as f32 / 255.0;
            let oa = sa + glow_a * (1.0 - sa);
            if oa <= 0.0 {
                continue;
            }
            for c in 0..3 {
                // The glow is black, so only the sprite's colour remains, rescaled.
                p[c] = ((p[c] as f32 * sa) / oa).round().min(255.0) as u8;
            }
            p[3] = (oa * 255.0).round() as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_and_outline() {
        // A single opaque red pixel in an 8×8 sprite at size 100: 40×40 scaled, on a
        // (40 + 24) × (40 + 13) canvas, with the red block at (12, 12) and black around it.
        let mut src = RgbaImage::new(8, 8);
        src.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        let out = redraw(&src, false, 100, 5.0);
        assert_eq!(out.dimensions(), (64, 53));
        assert_eq!(*out.get_pixel(12, 12), Rgba([255, 0, 0, 255]));
        assert_eq!(*out.get_pixel(16, 16), Rgba([255, 0, 0, 255]));
        assert_eq!(out.get_pixel(17, 12)[3], 204, "outline right of the block");
        assert_eq!(out.get_pixel(11, 11)[3], 204, "outline diagonal");
        assert_eq!(out.get_pixel(18, 12)[3], 0, "only one pixel wide");
        // Mirrored, the block moves to the right edge of the sprite.
        let out = redraw(&src, true, 100, 5.0);
        assert_eq!(*out.get_pixel(12 + 39, 12), Rgba([255, 0, 0, 255]));
        // Size 50 halves it.
        assert_eq!(redraw(&src, false, 50, 5.0).dimensions(), (20 + 24, 20 + 13));
    }
}
