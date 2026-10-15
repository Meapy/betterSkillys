//! Emotes (`Emotes`, `EmoteGraphicHelper`): words in chat and speech balloons, like `:)` or
//! `:kappa:`, drawn as pictures. An emote is 16 pixels a side times its scale at the AIR
//! client's 14-pixel text, and grows with the text; animated ones show a frame every 100 ms.

use crate::sprites::Sprites;
use crate::text::{Fonts, Style, Text};
use bsk_render::{Quad, Renderer, Tint};

/// The picture `word` stands for: its frame now, and its side at this text size.
fn emote(sprites: &Sprites, word: &str, size: f32, now: i32) -> Option<(bsk_render::AtlasRect, f32, f32)> {
    let e = sprites.emotes.get(word)?;
    let (set, index) = e.frames.get((now.max(0) / 100) as usize % e.frames.len().max(1))?;
    let rect = sprites.tile(set, *index)?;
    let src = sprites.tile_src(set, *index)?;
    let k = size / 14.0;
    Some((rect, src.w as f32 * e.scale * k, src.h as f32 * e.scale * k))
}

/// Whether any word of `s` is an emote.
pub fn has_emote(sprites: &Sprites, s: &str) -> bool {
    !sprites.emotes.is_empty() && s.split(' ').any(|w| sprites.emotes.contains_key(w))
}

/// How wide `s` is, emotes drawn as pictures.
pub fn measure(sprites: &Sprites, fonts: &Fonts, text: &mut Text, renderer: &mut Renderer, s: &str, style: Style) -> f32 {
    if !has_emote(sprites, s) {
        return text.measure(fonts, renderer, s, style);
    }
    let space = text.measure(fonts, renderer, " ", style);
    let mut w = 0.0;
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            w += space;
        }
        w += match emote(sprites, word, style.size, 0) {
            Some((_, ew, _)) => ew + 2.0,
            None => text.measure(fonts, renderer, word, style),
        };
    }
    w
}

/// Draw `s` with its top left at (x, y), emotes as pictures.
#[allow(clippy::too_many_arguments)]
pub fn draw(sprites: &Sprites, fonts: &Fonts, text: &mut Text, renderer: &mut Renderer, s: &str, x: f32, y: f32, style: Style, now: i32) {
    if !has_emote(sprites, s) {
        text.draw(fonts, renderer, s, x, y, style);
        return;
    }
    let space = text.measure(fonts, renderer, " ", style);
    let mut x = x;
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            x += space;
        }
        match emote(sprites, word, style.size, now) {
            Some((rect, ew, eh)) => {
                // Centred on the line of text.
                let top = y + (style.size * 1.22 - eh) / 2.0;
                renderer.sprite(rect, Quad::rect(x.round(), top.round(), ew, eh), Tint::NONE);
                x += ew + 2.0;
            }
            None => {
                text.draw(fonts, renderer, word, x, y, style);
                x += text.measure(fonts, renderer, word, style);
            }
        }
    }
}
