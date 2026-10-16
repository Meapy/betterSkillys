//! The night sky behind the screens outside the game, the same as the website's hero
//! (`website/site.js`): banded sky, twinkling stars, a moon, the odd shooting star, a castle
//! skyline with lit windows, and embers rising from below. Everything is drawn in cells of a few
//! screen pixels, so it looks like pixel art at any size. Also the game's name in a pixel font.

use crate::hud::Ui;
use crate::theme::color;

/// A small seeded random generator (the site's), so the skyline is the same every time.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0 as f32 / 4294967296.0
    }
}

struct Star {
    x: f32,
    y: f32,
    big: bool,
    phase: f32,
    speed: f32,
    tint: u32,
}

/// A lit window (or the castle gate), flickering.
struct Light {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    phase: f32,
    flick: bool,
    hot: bool,
    gate: bool,
}

struct Ember {
    x: f32,
    y: f32,
    v: f32,
    sway: f32,
    life: f32,
}

struct Meteor {
    x: f32,
    y: f32,
    len: f32,
    t: f32,
}

#[derive(Default)]
pub struct Sky {
    /// The window size it was built for.
    size: (u32, u32),
    /// Screen pixels per cell, and the size in cells.
    px: f32,
    w: f32,
    h: f32,
    stars: Vec<Star>,
    /// The skyline's shapes, in cells: x, y, width, height, colour.
    land: Vec<[f32; 4]>,
    land_color: Vec<u32>,
    lights: Vec<Light>,
    embers: Vec<Ember>,
    meteor: Option<Meteor>,
    last: Option<i32>,
    /// Random numbers for what moves (embers, shooting stars).
    rng: Option<Lcg>,
}

/// The sky from the top down to 80% of the height, in flat steps like an old palette, then the
/// glow on the horizon.
const SKY: [u32; 8] = [0x06040C, 0x0A0716, 0x0F0A1F, 0x150D29, 0x1C1033, 0x26123B, 0x33143F, 0x45183F];
const HORIZON: [u32; 2] = [0x5A1D3E, 0x6E2438];

impl Sky {
    fn build(&mut self, w: f32, h: f32, s: f32) {
        self.size = (w as u32, h as u32);
        self.px = (3.0 * s).round().max(2.0);
        let (cw, ch) = ((w / self.px).ceil(), (h / self.px).ceil());
        self.w = cw;
        self.h = ch;
        let mut r = Lcg(7);
        self.stars.clear();
        for _ in 0..((cw * ch / 90.0).round() as usize) {
            let x = (r.next() * cw).floor();
            let y = (r.next().powf(1.4) * ch * 0.72).floor();
            let big = r.next() < 0.08;
            let phase = r.next() * std::f32::consts::TAU;
            let speed = 0.6 + r.next() * 1.8;
            let t = r.next();
            let tint = if t < 0.15 { 0xFFD99A } else if t < 0.3 { 0xC6B4FF } else { 0xFFFFFF };
            self.stars.push(Star { x, y, big, phase, speed, tint });
        }

        self.land.clear();
        self.land_color.clear();
        self.lights.clear();
        let base = ch - 1.0;
        let add = |land: &mut Vec<[f32; 4]>, colors: &mut Vec<u32>, r: [f32; 4], c: u32| {
            land.push(r);
            colors.push(c);
        };

        // far hills
        for x in 0..cw as i32 {
            let xf = x as f32;
            let hgt = ch * 0.2 + (xf / 23.0).sin() * 6.0 + (xf / 9.0 + 1.0).sin() * 3.0 + (xf / 61.0 + 2.0).sin() * 10.0;
            add(&mut self.land, &mut self.land_color, [xf, (base - hgt).round(), 1.0, hgt.round() + 1.0], 0x24163A);
        }

        // the town: blocks with battlements and the odd spire
        let mut r = Lcg(11);
        let cx = (cw * 0.5).round();
        let castle_w = (cw * 0.3).min(120.0).round().max(46.0);
        let town = 0x170E26;
        let mut x = -4.0;
        while x < cw {
            let bw = 6.0 + (r.next() * 12.0).floor();
            let mut bh = (ch * (0.08 + r.next() * 0.14)).round();
            if x + bw > cx - castle_w / 2.0 - 4.0 && x < cx + castle_w / 2.0 + 4.0 {
                bh = (bh * 0.7).round();
            }
            let top = base - bh;
            add(&mut self.land, &mut self.land_color, [x, top, bw, bh + 1.0], town);
            let mut k = 0.0;
            while k < bw {
                add(&mut self.land, &mut self.land_color, [x + k, top - 2.0, 1.0, 2.0], town);
                k += 2.0;
            }
            if r.next() < 0.3 {
                let sx = x + (bw / 2.0).floor();
                for st in 0..8 {
                    let half = ((8 - st) / 3) as f32;
                    add(&mut self.land, &mut self.land_color, [sx - half, top - 10.0 + st as f32, 1.0 + 2.0 * half, 1.0], town);
                }
                add(&mut self.land, &mut self.land_color, [sx, top - 13.0, 1.0, 3.0], town);
            }
            let mut wy = top + 3.0;
            while wy < base - 3.0 {
                let mut wx = x + 2.0;
                while wx < x + bw - 2.0 {
                    if r.next() < 0.16 {
                        let phase = r.next() * std::f32::consts::TAU;
                        let flick = r.next() < 0.3;
                        self.lights.push(Light { x: wx, y: wy, w: 1.0, h: 2.0, phase, flick, hot: false, gate: false });
                    }
                    wx += 3.0;
                }
                wy += 4.0;
            }
            x += bw + (r.next() * 3.0).floor();
        }

        // the castle in the middle: a keep, two towers and a spire, a gate with a glow
        let keep = 0x100A1C;
        let keep_h = (ch * 0.2).min(70.0).round();
        let keep_top = base - keep_h;
        let kx = cx - (castle_w / 2.0).round();
        add(&mut self.land, &mut self.land_color, [kx, keep_top + 10.0, castle_w, keep_h], keep);
        let mut k = 0.0;
        while k < castle_w {
            add(&mut self.land, &mut self.land_color, [kx + k, keep_top + 6.0, 2.0, 4.0], keep);
            k += 4.0;
        }
        let mut k = 6.0;
        while k < castle_w - 6.0 {
            self.lights.push(Light { x: kx + k, y: keep_top + 18.0, w: 2.0, h: 3.0, phase: k, flick: true, hot: false, gate: false });
            k += 7.0;
        }
        let tw = (castle_w * 0.16).round().max(8.0);
        for (i, tx) in [kx - tw + 2.0, kx + castle_w - 2.0, cx - (tw / 2.0).round()].into_iter().enumerate() {
            let th = if i == 2 { keep_h + 18.0 } else { keep_h + 6.0 };
            let tt = base - th;
            add(&mut self.land, &mut self.land_color, [tx, tt, tw, th + 1.0], keep);
            let mid = tx + (tw / 2.0).floor();
            for y in 0..tw as i32 {
                let half = ((y as f32 / tw) * (tw / 2.0 + 1.0)).round().max(0.0);
                add(&mut self.land, &mut self.land_color, [mid - half, tt - tw + y as f32, half * 2.0, 1.0], keep);
            }
            add(&mut self.land, &mut self.land_color, [mid - 1.0, tt - tw - 4.0, 1.0, 4.0], keep);
            self.lights.push(Light { x: mid - 1.0, y: tt + 6.0, w: 2.0, h: 3.0, phase: i as f32 * 2.0, flick: true, hot: true, gate: false });
        }
        self.lights.push(Light { x: cx - 4.0, y: base - 12.0, w: 8.0, h: 12.0, phase: 0.0, flick: true, hot: true, gate: true });

        // the near ground, the screens' own colour
        for x in 0..cw as i32 {
            let xf = x as f32;
            let gh = 3.0 + ((xf / 13.0).sin() * 1.5 + (xf / 5.0).sin() * 0.8).round();
            add(&mut self.land, &mut self.land_color, [xf, base - gh, 1.0, gh + 1.0], color::BACKGROUND);
        }

        let mut rng = self.rng.take().unwrap_or(Lcg(0x5EED));
        self.embers.clear();
        for _ in 0..((cw / 14.0).round() as usize) {
            let e = Self::ember(&mut rng, cw, ch, true);
            self.embers.push(e);
        }
        self.rng = Some(rng);
        self.meteor = None;
    }

    fn ember(r: &mut Lcg, w: f32, h: f32, anywhere: bool) -> Ember {
        Ember {
            x: r.next() * w,
            y: if anywhere { h * (0.5 + r.next() * 0.5) } else { h + 2.0 },
            v: 0.08 + r.next() * 0.22,
            sway: r.next() * std::f32::consts::TAU,
            life: 1.0,
        }
    }

    /// Draw the sky over the whole window (`w` × `h` pixels, at the screens' scale `s`), `now`
    /// milliseconds into the run.
    pub fn draw(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, now: i32) {
        if self.size != (w as u32, h as u32) || self.px == 0.0 {
            self.build(w, h, s);
        }
        let dt = self.last.map_or(0, |l| (now - l).clamp(0, 64)) as f32;
        self.last = Some(now);
        let t = now as f32 / 1000.0;
        let (px, cw, ch) = (self.px, self.w, self.h);
        let cell = |ui: &mut Ui, x: f32, y: f32, cw: f32, chh: f32, c: u32, a: f32| ui.rect(x * px, y * px, cw * px, chh * px, c, a);

        ui.rect(0.0, 0.0, w, h, color::BACKGROUND, 1.0);
        let band = |ui: &mut Ui, y0: f32, y1: f32, colours: &[u32]| {
            let step = (y1 - y0) / colours.len() as f32;
            for (i, &c) in colours.iter().enumerate() {
                cell(ui, 0.0, (y0 + i as f32 * step).floor(), cw, step.ceil() + 1.0, c, 1.0);
            }
        };
        band(ui, 0.0, ch * 0.8, &SKY);
        band(ui, ch * 0.8, ch, &HORIZON);

        for st in &self.stars {
            let a = 0.45 + 0.55 * (t * st.speed + st.phase).sin().max(0.0);
            cell(ui, st.x, st.y, 1.0, 1.0, st.tint, a);
            if st.big && a > 0.8 {
                cell(ui, st.x - 1.0, st.y, 3.0, 1.0, st.tint, a * 0.5);
                cell(ui, st.x, st.y - 1.0, 1.0, 3.0, st.tint, a * 0.5);
            }
        }

        // the moon, lit from the top left, with a dotted halo and a few craters: high up on a wide
        // window (clear of the sign-in card), lower on a tall one (clear of the name)
        let my = if cw >= 1.3 * ch { 0.11 } else { 0.3 };
        let (mx, my) = ((cw * 0.82).round(), (ch * my).round());
        let rad = (cw.min(ch) * 0.045).round().max(5.0) as i32;
        for y in -rad - 3..=rad + 3 {
            for x in -rad - 3..=rad + 3 {
                let d = ((x * x + y * y) as f32).sqrt();
                let (xf, yf) = (x as f32, y as f32);
                if d <= rad as f32 {
                    let lit = xf + yf * 0.4 < rad as f32 * 0.5;
                    cell(ui, mx + xf, my + yf, 1.0, 1.0, if lit { 0xFBEED2 } else { 0xD9C39C }, 1.0);
                } else if d <= rad as f32 + 3.0 && (x + y).rem_euclid(2) == 0 {
                    cell(ui, mx + xf, my + yf, 1.0, 1.0, 0xFFDCA0, 0.14);
                }
            }
        }
        for [x, y, cwid, chei] in [[-2.0, -3.0, 2.0, 2.0], [2.0, 1.0, 3.0, 2.0], [-4.0, 3.0, 2.0, 1.0]] {
            cell(ui, mx + x, my + y, cwid, chei, 0xCDB48A, 1.0);
        }

        // a shooting star now and then
        let rng = self.rng.get_or_insert(Lcg(0x5EED));
        if self.meteor.is_none() && rng.next() < dt / 7000.0 {
            self.meteor = Some(Meteor { x: cw * (0.1 + rng.next() * 0.6), y: ch * rng.next() * 0.25, len: 10.0 + rng.next() * 10.0, t: 0.0 });
        }
        if let Some(m) = &mut self.meteor {
            m.t += dt / 1000.0;
            let (hx, hy) = (m.x + m.t * cw * 0.5, m.y + m.t * cw * 0.22);
            let fade = (1.0 - m.t * 1.4).max(0.0);
            for k in 0..m.len as i32 {
                let kf = k as f32;
                let a = (1.0 - kf / m.len) * fade;
                cell(ui, (hx - kf * 2.2).round(), (hy - kf).round(), 1.0, 1.0, if k < 2 { 0xFFFFFF } else { 0xFFD99A }, a);
            }
            if m.t > 0.75 {
                self.meteor = None;
            }
        }

        for (r, &c) in self.land.iter().zip(&self.land_color) {
            cell(ui, r[0], r[1], r[2], r[3], c, 1.0);
        }

        for l in &self.lights {
            let glow = if l.flick { 0.7 + 0.3 * (t * 3.0 + l.phase).sin() * (t * 7.3 + l.phase).sin() } else { 1.0 };
            let c = if l.gate { color::ACCENT } else if l.hot { 0xFF9A4D } else { color::GOLD };
            if l.gate {
                cell(ui, l.x + 2.0, l.y, l.w - 4.0, 1.0, c, glow);
                cell(ui, l.x + 1.0, l.y + 1.0, l.w - 2.0, 1.0, c, glow);
                cell(ui, l.x, l.y + 2.0, l.w, l.h - 2.0, c, glow);
                cell(ui, l.x - 3.0, l.y - 2.0, l.w + 6.0, l.h + 2.0, c, glow * 0.25);
            } else {
                cell(ui, l.x, l.y, l.w, l.h, c, glow);
            }
        }

        let rng = self.rng.as_mut().expect("seeded above");
        for e in &mut self.embers {
            e.y -= e.v * dt / 16.0;
            e.sway += dt / 900.0;
            e.life = ((e.y / ch - 0.35) * 2.2).min(1.0);
            if e.life <= 0.0 {
                *e = Self::ember(rng, cw, ch, false);
            }
            let c = if e.life > 0.6 { 0xFFB066 } else { 0xFF5A2A };
            cell(ui, (e.x + e.sway.sin() * 3.0).round(), e.y.round(), 1.0, 1.0, c, e.life.max(0.0));
        }
    }
}

/// The game's name in a 5×7 pixel font, as the website's heading: gold, with an ember drop
/// and a dark shadow under it. `cell` is the size of one font pixel; centred on `cx`, top at
/// `y`. Returns the height drawn.
pub fn logo(ui: &mut Ui, text: &str, cx: f32, y: f32, cell: f32) -> f32 {
    let cell = cell.round().max(1.0);
    let upper = text.to_uppercase();
    let advance = 6.0 * cell;
    let width = upper.chars().count() as f32 * advance - cell;
    let x0 = (cx - width / 2.0).round();
    let layers: [(f32, u32, f32); 3] = [(2.0, 0x08060F, 0.6), (1.0, 0xC2410C, 1.0), (0.0, color::GOLD, 1.0)];
    for (drop, c, a) in layers {
        for (i, ch) in upper.chars().enumerate() {
            let Some(rows) = glyph(ch) else { continue };
            let gx = x0 + i as f32 * advance;
            for (row, line) in rows.iter().enumerate() {
                let rc = if drop == 0.0 && row < 2 { 0xFFE27A } else { c };
                for (col, b) in line.bytes().enumerate() {
                    if b == b'#' {
                        ui.rect(gx + col as f32 * cell, y + (row as f32 + drop) * cell, cell, cell, rc, a);
                    }
                }
            }
        }
    }
    9.0 * cell
}

/// The width of `text` in the pixel font at `cell`.
pub fn logo_width(text: &str, cell: f32) -> f32 {
    text.chars().count() as f32 * 6.0 * cell - cell
}

fn glyph(c: char) -> Option<[&'static str; 7]> {
    Some(match c {
        'A' => [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'B' => ["####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."],
        'C' => [".####", "#....", "#....", "#....", "#....", "#....", ".####"],
        'D' => ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
        'E' => ["#####", "#....", "#....", "####.", "#....", "#....", "#####"],
        'F' => ["#####", "#....", "#....", "####.", "#....", "#....", "#...."],
        'G' => [".####", "#....", "#....", "#.###", "#...#", "#...#", ".###."],
        'H' => ["#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        'I' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"],
        'J' => ["..###", "...#.", "...#.", "...#.", "...#.", "#..#.", ".##.."],
        'K' => ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
        'L' => ["#....", "#....", "#....", "#....", "#....", "#....", "#####"],
        'M' => ["#...#", "##.##", "#.#.#", "#.#.#", "#...#", "#...#", "#...#"],
        'N' => ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
        'O' => [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'P' => ["####.", "#...#", "#...#", "####.", "#....", "#....", "#...."],
        'Q' => [".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#"],
        'R' => ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
        'S' => [".####", "#....", "#....", ".###.", "....#", "....#", "####."],
        'T' => ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
        'U' => ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
        'V' => ["#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#.."],
        'W' => ["#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#"],
        'X' => ["#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#"],
        'Y' => ["#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#.."],
        'Z' => ["#####", "....#", "...#.", "..#..", ".#...", "#....", "#####"],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font() {
        for c in 'A'..='Z' {
            let g = glyph(c).unwrap();
            assert!(g.iter().all(|row| row.len() == 5), "{c}");
        }
        assert!(glyph(' ').is_none());
        assert_eq!(logo_width("DO", 2.0), 22.0);
    }
}
