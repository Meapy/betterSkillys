//! The HUD (`HUDView`) and chat (`TextBox`), laid out for whatever size the window is.
//!
//! The AIR client's HUD is a 200×600 panel beside a 600×600 view. Here the panel keeps its
//! design, scaled by one factor (`Layout::s`) chosen from the window's width and height, and
//! sits at the right edge; the world fills the rest. Everything is worked out each frame from
//! the current size, so resizing or going fullscreen just works.

use crate::redraw::Redrawer;
use crate::sprites::{SrcRect, Sprites};
use crate::text::{Fonts, Style, Text};
use bsk_data::{GameData, ObjectClass, TextureData, TextureRef};
use bsk_render::{Quad, Renderer, Tint};
use bsk_world::{GameObj, World};
use std::collections::HashMap;

/// Where things go for one window size.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub w: f32,
    pub h: f32,
    /// HUD units (the AIR client's pixels) to screen pixels.
    pub s: f32,
    /// The panel's left edge; the world is drawn left of it.
    pub panel_x: f32,
}

/// The panel's design size.
const PANEL_W: f32 = 200.0;
const PANEL_H: f32 = 600.0;

impl Layout {
    pub fn new(w: u32, h: u32) -> Self {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        // The whole panel fits the height, and takes at most 28% of the width.
        let s = (h / (PANEL_H + 10.0)).min(w * 0.28 / PANEL_W).clamp(0.45, 4.0);
        Layout { w, h, s, panel_x: (w - PANEL_W * s).round() }
    }

    /// Width of the world's part of the window.
    pub fn view_w(&self) -> f32 {
        self.panel_x
    }

    /// Pixels per tile: about 50 (the AIR client's) for a 750-pixel view, more or less with the
    /// view's smaller side.
    pub fn map_scale(&self) -> f64 {
        50.0 * (self.view_w().min(self.h) as f64 / 750.0).clamp(0.6, 4.0)
    }

    /// Where the player stands: centred across the view, a little below the middle.
    pub fn camera_point(&self) -> (f64, f64) {
        (self.view_w() as f64 / 2.0, self.h as f64 * 13.0 / 24.0)
    }

    /// A point on the panel, from HUD units.
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.panel_x + x * self.s, y * self.s)
    }

    /// A font size in HUD units, on the screen (never unreadably small).
    pub fn font(&self, size: f32) -> f32 {
        (size * self.s).round().max(9.0)
    }

    /// Whether a screen point is on the panel.
    pub fn on_panel(&self, x: f64, y: f64) -> bool {
        x as f32 >= self.panel_x && (y as f32) < self.h
    }
}

/// What drawing the HUD needs.
pub struct Ui<'a> {
    pub renderer: &'a mut Renderer,
    pub sprites: &'a Sprites,
    pub data: &'a GameData,
    pub redrawer: &'a mut Redrawer,
    pub fonts: &'a Fonts,
    pub text: &'a mut Text,
}

impl Ui<'_> {
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: u32, alpha: f32) {
        let white = self.renderer.white();
        self.renderer.sprite(white, Quad::rect(x, y, w, h), Tint::color(rgb, alpha));
    }

    fn label(&mut self, s: &str, x: f32, y: f32, size: f32, bold: bool, color: u32) {
        self.text.draw(self.fonts, self.renderer, s, x, y, Style { size, bold, color });
    }

    fn measure(&mut self, s: &str, size: f32, bold: bool) -> f32 {
        self.text.measure(self.fonts, self.renderer, s, Style { size, bold, color: 0 })
    }

    fn src(&self, tex: &TextureData) -> Option<SrcRect> {
        match &tex.pick(0).main {
            TextureRef::Image { set, index } => self.sprites.tile_src(set, *index),
            TextureRef::Animated { set, index } => self.sprites.char_src(set, *index, 0, 0),
            _ => None,
        }
    }

    /// `getRedrawnTextureFromType(type, 80, true)`, centred on (cx, cy) at `scale`.
    fn item_icon(&mut self, item: i32, cx: f32, cy: f32, scale: f32) {
        let Some(desc) = self.data.objects.get(&(item as u16)) else { return };
        let Some(src) = self.src(&desc.texture) else { return };
        let size = if src.h == 16 { 40 } else { 80 };
        let Some(icon) = self.redrawer.get(self.sprites, self.renderer, src, false, size, None) else { return };
        let (w, h) = (icon.w as f32 * scale, icon.h as f32 * scale);
        self.renderer.sprite(icon.rect, Quad::rect((cx - w / 2.0).round(), (cy - h / 2.0).round(), w, h), Tint::NONE);
    }
}

/// Minimap colours, worked out once per type (`GroundLibrary.getColor`,
/// `GameObject.getColor`: the `<Color>`, else the texture's most common colour).
#[derive(Default)]
pub struct MapColors {
    tiles: HashMap<u16, u32>,
    objects: HashMap<u16, u32>,
}

fn most_common_color(img: &image::RgbaImage) -> u32 {
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for p in img.pixels() {
        if p[3] != 0 {
            *counts.entry(u32::from_be_bytes([p[3], p[0], p[1], p[2]])).or_default() += 1;
        }
    }
    // Ties go to the larger ARGB value, as BitmapUtil.mostCommonColor does.
    counts.into_iter().max_by_key(|&(c, n)| (n, c)).map_or(0, |(c, _)| c & 0xFF_FFFF)
}

impl MapColors {
    fn tile(&mut self, ui: &Ui, tile: u16) -> u32 {
        *self.tiles.entry(tile).or_insert_with(|| {
            let Some(g) = ui.data.grounds.get(&tile) else { return 0 };
            g.color.unwrap_or_else(|| ui.src(&g.texture).map_or(0, |r| most_common_color(&ui.sprites.pixels(r))))
        })
    }

    fn object(&mut self, ui: &Ui, object_type: u16) -> u32 {
        *self.objects.entry(object_type).or_insert_with(|| {
            let Some(d) = ui.data.objects.get(&object_type) else { return 0 };
            // Walls show their top (Wall.getColor).
            let tex = if d.class.is_wall() { d.top.as_ref().unwrap_or(&d.texture) } else { &d.texture };
            d.color.unwrap_or_else(|| ui.src(tex).map_or(0, |r| most_common_color(&ui.sprites.pixels(r))))
        })
    }
}

/// Stat ids (`StatData`).
mod stat {
    pub const MAX_HP: u8 = 0;
    pub const HP: u8 = 1;
    pub const MAX_MP: u8 = 3;
    pub const MP: u8 = 4;
    pub const NEXT_LEVEL_EXP: u8 = 5;
    pub const EXP: u8 = 6;
    pub const LEVEL: u8 = 7;
    pub const INVENTORY_0: u8 = 8;
    pub const ATTACK: u8 = 20;
    pub const DEFENSE: u8 = 21;
    pub const SPEED: u8 = 22;
    pub const VITALITY: u8 = 26;
    pub const WISDOM: u8 = 27;
    pub const DEXTERITY: u8 = 28;
    pub const ATTACK_BOOST: u8 = 48;
    pub const DEFENSE_BOOST: u8 = 49;
    pub const SPEED_BOOST: u8 = 50;
    pub const VITALITY_BOOST: u8 = 51;
    pub const WISDOM_BOOST: u8 = 52;
    pub const DEXTERITY_BOOST: u8 = 53;
    pub const CURR_FAME: u8 = 57;
    pub const NEXT_CLASS_QUEST_FAME: u8 = 58;
}

/// `Slot` colours.
const SLOT_FILL: u32 = 0x545454;
const PANEL_FILL: u32 = 0x3A3A3A;
const MINIMAP_FILL: u32 = 0x1B1B1B;

/// The panel: minimap, name, bars, equipment, inventory and stats.
pub fn draw_panel(ui: &mut Ui, lay: &Layout, world: &World, camera_angle: f64, colors: &mut MapColors) {
    let s = lay.s;
    ui.rect(lay.panel_x, 0.0, lay.w - lay.panel_x, lay.h, PANEL_FILL, 1.0);
    let Some(me) = world.player() else { return };
    draw_minimap(ui, lay, world, me, camera_angle, colors);

    // CharacterDetailsView: the name.
    let (x, y) = lay.at(10.0, 204.0);
    let name = me.name().unwrap_or("").to_owned();
    ui.label(&name, x, y, lay.font(18.0), true, 0xFFFFFF);

    // StatMetersView: experience (fame at level 20), HP, MP.
    let stat = |id: u8| me.stat_int(id).unwrap_or(0);
    let level = stat(stat::LEVEL);
    let bars = [
        if level < 20 {
            (format!("Lvl {level}"), stat(stat::EXP), stat(stat::NEXT_LEVEL_EXP), 0x5A8025)
        } else {
            ("Fame".to_owned(), stat(stat::CURR_FAME), stat(stat::NEXT_CLASS_QUEST_FAME), 0xE25F00)
        },
        ("HP".to_owned(), stat(stat::HP), stat(stat::MAX_HP), 0xE03434),
        ("MP".to_owned(), stat(stat::MP), stat(stat::MAX_MP), 0x6084E0),
    ];
    for (i, (label, value, max, color)) in bars.into_iter().enumerate() {
        let (bx, by) = lay.at(12.0, 230.0 + 24.0 * i as f32);
        status_bar(ui, lay, bx, by, &label, value, max, color);
    }

    // EquippedGrid (slots 0-3) and the inventory (4-11), 44 units apart.
    let item = |slot: u8| me.stat_int(stat::INVENTORY_0 + slot).unwrap_or(-1);
    for i in 0..4u8 {
        let (sx, sy) = lay.at(14.0 + 44.0 * i as f32, 304.0);
        slot(ui, lay, sx, sy, item(i));
    }
    for i in 0..8u8 {
        let (sx, sy) = lay.at(14.0 + 44.0 * (i % 4) as f32, 356.0 + 44.0 * (i / 4) as f32);
        slot(ui, lay, sx, sy, item(4 + i));
    }

    // StatsView: two columns of three. Gold at the class maximum, green while boosted.
    let class = ui.data.objects.get(&me.object_type);
    let max_of = |tag: &str| class.and_then(|c| c.xml.child(tag)).and_then(|e| e.attr("max")).and_then(|m| m.parse::<i32>().ok());
    let stats = [
        ("ATT", stat::ATTACK, stat::ATTACK_BOOST, "Attack"),
        ("DEF", stat::DEFENSE, stat::DEFENSE_BOOST, "Defense"),
        ("SPD", stat::SPEED, stat::SPEED_BOOST, "Speed"),
        ("DEX", stat::DEXTERITY, stat::DEXTERITY_BOOST, "Dexterity"),
        ("VIT", stat::VITALITY, stat::VITALITY_BOOST, "HpRegen"),
        ("WIS", stat::WISDOM, stat::WISDOM_BOOST, "MpRegen"),
    ];
    let size = lay.font(13.0);
    for (i, (label, id, boost_id, tag)) in stats.into_iter().enumerate() {
        let (value, boost) = (stat(id), stat(boost_id));
        let (tx, ty) = lay.at(16.0 + 88.0 * (i % 2) as f32, 454.0 + 18.0 * (i / 2) as f32);
        ui.label(label, tx, ty, size, true, 0xB3B3B3);
        let color = if max_of(tag).is_some_and(|m| value - boost >= m) {
            0xFCDF00
        } else if boost > 0 {
            0x6EFF6E
        } else {
            0xFFFFFF
        };
        let text = if boost != 0 { format!("{value} ({boost:+})") } else { value.to_string() };
        ui.label(&text, tx + 34.0 * s, ty, size, false, color);
    }
}

/// `StatusBar`: a filled bar with its label at the left and "value/max" centred.
#[allow(clippy::too_many_arguments)]
fn status_bar(ui: &mut Ui, lay: &Layout, x: f32, y: f32, label: &str, value: i32, max: i32, color: u32) {
    let (w, h) = (176.0 * lay.s, 16.0 * lay.s);
    ui.rect(x, y, w, h, 0x545454, 1.0);
    let frac = if max > 0 { (value as f32 / max as f32).clamp(0.0, 1.0) } else { 0.0 };
    ui.rect(x, y, (w * frac).round(), h, color, 1.0);
    let size = lay.font(12.0);
    let ty = y + (h - size * 1.2) / 2.0;
    ui.label(label, x + 4.0 * lay.s, ty, size, true, 0xFFFFFF);
    let v = format!("{value}/{max}");
    let vw = ui.measure(&v, size, true);
    ui.label(&v, x + (w - vw) / 2.0, ty, size, true, 0xFFFFFF);
}

fn slot(ui: &mut Ui, lay: &Layout, x: f32, y: f32, item: i32) {
    let side = 40.0 * lay.s;
    ui.rect(x, y, side, side, SLOT_FILL, 1.0);
    if item > 0 {
        ui.item_icon(item, x + side / 2.0, y + side / 2.0, lay.s);
    }
}

/// `MiniMap.draw` at its first zoom (4 units a tile), north up, the player's arrow turned with
/// the camera. Static objects that fill their square show their colour over the ground.
fn draw_minimap(ui: &mut Ui, lay: &Layout, world: &World, me: &GameObj, camera_angle: f64, colors: &mut MapColors) {
    const SIDE: f32 = 192.0;
    let (x0, y0) = lay.at(4.0, 4.0);
    let side = SIDE * lay.s;
    ui.rect(x0, y0, side, side, MINIMAP_FILL, 1.0);
    let zoom = 4.0 * lay.s;
    let half = (SIDE / 2.0 / 4.0).ceil() as i32 + 1;
    // Keep the map's edge from leaving a gap: the view stops at the map's sides.
    let span = SIDE as f64 / 4.0;
    let clamp = |c: f64, size: i32| if size as f64 <= span { size as f64 / 2.0 } else { c.clamp(span / 2.0, size as f64 - span / 2.0) };
    let (cx, cy) = (clamp(me.x, world.width), clamp(me.y, world.height));
    let to_map = |x: f64, y: f64| -> (f32, f32) {
        (x0 + side / 2.0 + ((x - cx) as f32) * zoom, y0 + side / 2.0 + ((y - cy) as f32) * zoom)
    };
    let mut marks: HashMap<(i32, i32), u32> = HashMap::new();
    for o in world.objects.values() {
        if let Some(d) = ui.data.objects.get(&o.object_type) {
            if d.is_static && d.occupy_square && !d.no_mini_map {
                marks.insert((o.x.floor() as i32, o.y.floor() as i32), colors.object(ui, o.object_type));
            }
        }
    }
    let (tx, ty) = (cx.floor() as i32, cy.floor() as i32);
    for y in ty - half..=ty + half {
        for x in tx - half..=tx + half {
            let Some(tile) = world.tile(x, y) else { continue };
            let (px, py) = to_map(x as f64, y as f64);
            // Whole tiles only, clipped to the box.
            let (l, t) = (px.max(x0), py.max(y0));
            let (r, b) = ((px + zoom).min(x0 + side), (py + zoom).min(y0 + side));
            if r <= l || b <= t {
                continue;
            }
            let color = match marks.get(&(x, y)) {
                Some(&c) => c,
                None => colors.tile(ui, tile),
            };
            ui.rect(l, t, r - l, b - t, color, 1.0);
        }
    }
    // Players yellow, enemies red, other characters their colour, portals blue; at the box's
    // edge when out of it.
    for o in world.objects.values() {
        if Some(o.id) == world.me {
            continue;
        }
        let Some(d) = ui.data.objects.get(&o.object_type) else { continue };
        if d.no_mini_map {
            continue;
        }
        let color = if d.player {
            0xFFFF00
        } else if d.class == ObjectClass::Character {
            if d.enemy { 0xFF0000 } else { colors.object(ui, o.object_type) }
        } else if matches!(d.class, ObjectClass::Portal | ObjectClass::GuildHallPortal) {
            0x0000FF
        } else {
            continue;
        };
        let (px, py) = to_map(o.x, o.y);
        let d2 = 2.0 * lay.s;
        let (px, py) = (px.clamp(x0 + d2, x0 + side - d2), py.clamp(y0 + d2, y0 + side - d2));
        ui.rect(px - d2, py - d2, 2.0 * d2, 2.0 * d2, color, 1.0);
    }
    // The player: lofiInterface 54 tinted blue, pointing the way the camera faces.
    let (px, py) = to_map(me.x, me.y);
    if let Some(arrow) = ui.sprites.tile("lofiInterface", 54) {
        let (w, h) = (8.0 * lay.s, 32.0 * lay.s);
        let quad = Quad::rotated(px, py, w, h, camera_angle as f32);
        ui.renderer.sprite(arrow, quad, Tint { mul: [0.0, 0.0, 1.0, 1.0], add: [0.0; 3] });
    } else {
        ui.rect(px - 2.0 * lay.s, py - 2.0 * lay.s, 4.0 * lay.s, 4.0 * lay.s, 0x0000FF, 1.0);
    }
}

/// One chat line (`TextBoxLine`).
#[derive(Debug, Clone)]
pub struct ChatLine {
    pub name: String,
    pub recipient: String,
    pub text: String,
    pub name_color: i32,
    pub text_color: i32,
    /// When it arrived, ms.
    pub at: i32,
}

impl ChatLine {
    /// `getTextBlock`: the pieces and their colours. `me` is our name (for "To:" lines).
    pub fn pieces(&self, me: &str) -> Vec<(String, u32)> {
        let mut name = self.name.clone();
        let mut name_color = if self.name_color == 0x12_3456 {
            0x00FE00
        } else if self.name_color != 0 {
            self.name_color as u32
        } else {
            0x00FF00
        };
        let mut text_color = if self.text_color != 0 { self.text_color as u32 } else { 0xFFFFFF };
        match self.name.as_str() {
            "" => text_color = 0xFFFF00,
            "&" => text_color = 0xCCBB00,
            "*Client*" => text_color = 0x0000FF,
            "*Help*" => text_color = 0xFF5B05,
            "*Error*" => text_color = 0xFF0000,
            _ => {}
        }
        if matches!(self.name.as_str(), "" | "&" | "*Client*" | "*Help*" | "*Error*") {
            name.clear();
        }
        if let Some(rest) = self.name.strip_prefix('#') {
            name = rest.to_owned();
            name_color = 0xFFA800;
        }
        if let Some(rest) = self.name.strip_prefix('@') {
            name = rest.to_owned();
            (name_color, text_color) = (0xFFFF00, 0xFFFF00);
        }
        let mut out = Vec::new();
        match self.recipient.as_str() {
            "" => {}
            "*Guild*" => (name_color, text_color) = (0xA6FF5D, 0xA6FF5D),
            "*Party*" => (name_color, text_color) = (0x78A0FF, 0x78A0FF),
            to => {
                (name_color, text_color) = (0x00F0FF, 0x00F0FF);
                if self.name == me {
                    out.push(("To: ".to_owned(), 0x00F0FF));
                    name = to.to_owned();
                }
            }
        }
        if !name.is_empty() {
            out.push((format!("<{name}> "), name_color));
        }
        out.push((self.text.clone(), text_color));
        out
    }
}

/// How long a chat line stays up while the input is closed, ms.
const CHAT_SHOW_MS: i32 = 30_000;

/// The chat box at the bottom left of the view: recent lines, wrapped to fit, and the input
/// line when open.
pub fn draw_chat(ui: &mut Ui, lay: &Layout, lines: &[ChatLine], me: &str, input: Option<&str>, now: i32) {
    let size = lay.font(14.0);
    let line_h = (size * 1.25).round();
    let margin = 8.0 * lay.s;
    let max_w = (lay.view_w() - 2.0 * margin).min(620.0 * lay.s).max(120.0);
    let max_rows = ((lay.h * 0.4) / line_h).floor().max(3.0) as usize;
    let mut bottom = lay.h - margin;
    if let Some(input) = input {
        let shown = format!("{input}_");
        ui.rect(margin - 3.0, bottom - line_h - 2.0, max_w + 6.0, line_h + 4.0, 0x000000, 0.5);
        // Long input scrolls: show its end.
        let mut start = 0;
        while start < shown.len() && ui.measure(&shown[start..], size, false) > max_w {
            start += shown[start..].chars().next().map_or(1, |c| c.len_utf8());
        }
        ui.label(&shown[start..], margin, bottom - line_h, size, false, 0xFFFFFF);
        bottom -= line_h + 6.0;
    }
    // Wrap the newest lines into rows, newest at the bottom.
    let mut rows: Vec<Vec<(String, u32)>> = Vec::new();
    for line in lines.iter().rev() {
        if input.is_none() && now - line.at > CHAT_SHOW_MS {
            break;
        }
        let mut wrapped = wrap(ui, &line.pieces(me), size, max_w);
        while let Some(row) = wrapped.pop() {
            rows.push(row);
        }
        if rows.len() >= max_rows {
            break;
        }
    }
    rows.truncate(max_rows);
    for (i, row) in rows.iter().enumerate() {
        let y = bottom - line_h * (i as f32 + 1.0);
        let mut x = margin;
        for (piece, color) in row {
            ui.label(piece, x, y, size, false, *color);
            x += ui.measure(piece, size, false);
        }
    }
}

/// Break coloured pieces into rows no wider than `max_w`, at spaces where possible.
fn wrap(ui: &mut Ui, pieces: &[(String, u32)], size: f32, max_w: f32) -> Vec<Vec<(String, u32)>> {
    let mut rows = vec![Vec::new()];
    let mut x = 0.0;
    for (piece, color) in pieces {
        for word in piece.split_inclusive(' ') {
            let w = ui.measure(word, size, false);
            if x + w > max_w && x > 0.0 {
                rows.push(Vec::new());
                x = 0.0;
            }
            // A word longer than a row is cut wherever it runs out.
            let mut rest = word;
            while ui.measure(rest, size, false) > max_w {
                let mut cut = 0;
                for (i, c) in rest.char_indices() {
                    if ui.measure(&rest[..i + c.len_utf8()], size, false) > max_w {
                        break;
                    }
                    cut = i + c.len_utf8();
                }
                let cut = cut.max(rest.chars().next().map_or(1, |c| c.len_utf8()));
                rows.last_mut().unwrap().push((rest[..cut].to_owned(), *color));
                rows.push(Vec::new());
                rest = &rest[cut..];
                x = 0.0;
            }
            if !rest.is_empty() {
                rows.last_mut().unwrap().push((rest.to_owned(), *color));
                x += ui.measure(rest, size, false);
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_fits_any_window() {
        for (w, h) in [(800, 600), (1920, 1080), (3840, 2160), (640, 1200), (1200, 400), (320, 240)] {
            let l = Layout::new(w, h);
            let panel_w = l.w - l.panel_x;
            assert!(PANEL_H * l.s <= h as f32 || l.s == 0.45, "{w}×{h}: panel too tall");
            assert!(panel_w <= w as f32 * 0.28 + 1.0 || l.s == 0.45, "{w}×{h}: panel too wide");
            assert!(l.view_w() > 0.0);
        }
        // A typical window keeps the AIR client's look: about 1:1 at 600 high.
        assert!((Layout::new(800, 610).s - 1.0).abs() < 1e-3);
    }

    #[test]
    fn chat_colours() {
        let line = |name: &str, recipient: &str| ChatLine {
            name: name.into(),
            recipient: recipient.into(),
            text: "hi".into(),
            name_color: 0,
            text_color: 0,
            at: 0,
        };
        assert_eq!(line("Bob", "").pieces("Me"), vec![("<Bob> ".into(), 0x00FF00), ("hi".into(), 0xFFFFFF)]);
        assert_eq!(line("", "").pieces("Me"), vec![("hi".into(), 0xFFFF00)]);
        assert_eq!(line("#Oryx", "").pieces("Me")[0], ("<Oryx> ".into(), 0xFFA800));
        // A tell we sent shows who it went to.
        assert_eq!(line("Me", "Bob").pieces("Me")[0], ("To: ".into(), 0x00F0FF));
    }
}
