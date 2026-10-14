//! The screens outside the game (the AIR client's `TitleView`/`AccountScreen`,
//! `CharacterSelectionAndNewsScreen`, `NewCharacterScreen`): sign in or register, then pick,
//! make or delete a character and choose a server. They're drawn over the title art in the
//! shared theme, scaled to the window and laid out from its shape each frame. Also the small
//! menu Escape opens in the game.

use crate::hud::Ui;
use crate::redraw::Dye;
use crate::theme::{self, color, Kind};
use bsk_data::{ObjectClass, ObjectDesc, TextureRef};
use bsk_net::account::{CharList, CharSummary};
use bsk_render::{AtlasRect, Quad, Tint};
use winit::keyboard::KeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Login,
    Register,
    /// Waiting on the account server.
    Busy,
    Chars,
    NewChar,
}

/// The text fields, by index.
pub const EMAIL: usize = 0;
pub const PASSWORD: usize = 1;
pub const NAME: usize = 2;
pub const CONFIRM: usize = 3;

/// What can be clicked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Field(usize),
    SignIn,
    ToRegister,
    Register,
    ToLogin,
    Remember,
    Char(i32),
    Delete(i32),
    ConfirmDelete,
    CancelDelete,
    NewChar,
    Class(u16),
    Back,
    ServerPrev,
    ServerNext,
    Logout,
}

/// What the player asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    SignIn,
    Register,
    Play(i32),
    Create(u16),
    Delete(i32),
    Logout,
}

pub struct Front {
    pub screen: Screen,
    /// Email, password, name, confirm password.
    pub fields: [String; 4],
    focus: usize,
    pub remember: bool,
    /// A line under the form or list: (text, is it an error).
    pub message: Option<(String, bool)>,
    /// What the Busy screen says.
    pub busy: String,
    pub list: Option<CharList>,
    /// Which of the list's servers to play on.
    pub server: usize,
    /// The character whose deletion is being confirmed.
    deleting: Option<i32>,
    /// Clickable areas from the last frame.
    targets: Vec<(Target, [f32; 4])>,
    /// Where the account server is, shown small on the login screen.
    pub endpoint_label: String,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            screen: Screen::Login,
            fields: Default::default(),
            focus: EMAIL,
            remember: true,
            message: None,
            busy: String::new(),
            list: None,
            server: 0,
            deleting: None,
            targets: Vec::new(),
            endpoint_label: String::new(),
        }
    }
}

/// The screens' scale: their design is about 1000×720.
pub fn scale(w: f32, h: f32) -> f32 {
    (h / 720.0).min(w / 1000.0).clamp(0.5, 3.0)
}

fn inside(r: [f32; 4], (x, y): (f64, f64)) -> bool {
    let (x, y) = (x as f32, y as f32);
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
}

/// The classes a new character can be (`ObjectLibrary.playerChars_`), in type order.
pub fn player_classes(data: &bsk_data::GameData) -> Vec<&ObjectDesc> {
    let mut classes: Vec<_> = data.objects.values().filter(|d| d.player && d.class == ObjectClass::Player).collect();
    classes.sort_by_key(|d| d.object_type);
    classes
}

/// `<UnlockLevel level="5" type="0x0307">`: whether the account may play the class, and the
/// first requirement it lacks.
fn unlock(desc: &ObjectDesc, list: &CharList) -> Option<(u16, i32)> {
    desc.xml
        .children_named("UnlockLevel")
        .map(|u| (bsk_data::xml::as3_int(u.attr("type").unwrap_or("0")) as u16, u.attr_int("level", 0)))
        .find(|(t, level)| list.class_stats.get(t).is_none_or(|s| s.0 < *level))
}

/// A tier colour for an item's slot edge: white tiers, gold for the top ones, purple for
/// untiered, orange for sets.
pub fn tier_edge(data: &bsk_data::GameData, item: i32) -> Option<u32> {
    let d = data.objects.get(&(item as u16))?;
    let xml = &d.xml;
    if ["Admin", "Ethereal", "Ascended", "Legendary"].iter().any(|t| xml.has(t)) {
        return Some(0xFFC800);
    }
    if xml.has("Consumable") || xml.has("Treasure") {
        return None;
    }
    if let Some(t) = xml.text_of("Tier") {
        let tier: i32 = t.trim().parse().unwrap_or(0);
        return Some(match tier {
            0..=5 => 0x8A93A6,
            6..=9 => 0xC9D2E3,
            10..=11 => 0x7FB2FF,
            _ => 0xFFC800,
        });
    }
    if xml.attr("setType").is_some() {
        return Some(0xFF9900);
    }
    xml.has("SlotType").then_some(0xB070FF)
}

impl Front {
    fn field_is_secret(i: usize) -> bool {
        i == PASSWORD || i == CONFIRM
    }

    fn fields_on_screen(&self) -> &'static [usize] {
        match self.screen {
            Screen::Login => &[EMAIL, PASSWORD],
            Screen::Register => &[NAME, EMAIL, PASSWORD, CONFIRM],
            _ => &[],
        }
    }

    /// Show a screen, focusing its first field.
    pub fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.deleting = None;
        if let Some(&first) = self.fields_on_screen().first() {
            // Signing in with an email already typed: start at the password.
            self.focus = if screen == Screen::Login && !self.fields[EMAIL].is_empty() { PASSWORD } else { first };
        }
    }

    /// A key pressed (with the text it types, if any).
    pub fn key(&mut self, code: KeyCode, text: Option<&str>) -> Option<Action> {
        if self.deleting.is_some() {
            match code {
                KeyCode::Escape => self.deleting = None,
                KeyCode::Enter | KeyCode::NumpadEnter => return self.deleting.take().map(Action::Delete),
                _ => {}
            }
            return None;
        }
        let fields = self.fields_on_screen();
        match code {
            KeyCode::Tab if !fields.is_empty() => {
                let i = fields.iter().position(|&f| f == self.focus).unwrap_or(0);
                self.focus = fields[(i + 1) % fields.len()];
            }
            KeyCode::Enter | KeyCode::NumpadEnter => match self.screen {
                Screen::Login => return Some(Action::SignIn),
                Screen::Register => return Some(Action::Register),
                _ => {}
            },
            KeyCode::Escape => match self.screen {
                Screen::Register => self.show(Screen::Login),
                Screen::NewChar => self.show(Screen::Chars),
                _ => {}
            },
            KeyCode::Backspace if fields.contains(&self.focus) => {
                self.fields[self.focus].pop();
            }
            _ => {
                if let (Some(t), true) = (text, fields.contains(&self.focus)) {
                    let field = &mut self.fields[self.focus];
                    field.extend(t.chars().filter(|c| !c.is_control()));
                    while field.chars().count() > 128 {
                        field.pop();
                    }
                }
            }
        }
        None
    }

    /// A left click.
    pub fn click(&mut self, pos: (f64, f64)) -> Option<Action> {
        let target = self.targets.iter().rev().find(|(_, r)| inside(*r, pos)).map(|(t, _)| *t)?;
        let servers = self.list.as_ref().map_or(0, |l| l.servers.len());
        match target {
            Target::Field(i) => self.focus = i,
            Target::SignIn => return Some(Action::SignIn),
            Target::Register => return Some(Action::Register),
            Target::ToRegister => {
                self.message = None;
                self.show(Screen::Register);
            }
            Target::ToLogin => {
                self.message = None;
                self.show(Screen::Login);
            }
            Target::Back => self.show(Screen::Chars),
            Target::Remember => self.remember = !self.remember,
            Target::Char(id) => return Some(Action::Play(id)),
            Target::Delete(id) => self.deleting = Some(id),
            Target::ConfirmDelete => return self.deleting.take().map(Action::Delete),
            Target::CancelDelete => self.deleting = None,
            Target::NewChar => self.show(Screen::NewChar),
            Target::Class(t) => return Some(Action::Create(t)),
            Target::ServerPrev if servers > 0 => self.server = (self.server + servers - 1) % servers,
            Target::ServerNext if servers > 0 => self.server = (self.server + 1) % servers,
            Target::ServerPrev | Target::ServerNext => {}
            Target::Logout => return Some(Action::Logout),
        }
        None
    }

    /// Draw the current screen. `title` is the background art and its size.
    pub fn draw(&mut self, ui: &mut Ui, w: f32, h: f32, title: Option<(AtlasRect, u32, u32)>, mouse: (f64, f64), now: i32) {
        self.targets.clear();
        let s = scale(w, h);
        draw_backdrop(ui, w, h, title, self.screen, now);
        match self.screen {
            Screen::Login => self.draw_login(ui, w, h, s, mouse, now),
            Screen::Register => self.draw_register(ui, w, h, s, mouse, now),
            Screen::Busy => {
                let (cx, cy) = (w / 2.0, h * 0.74);
                spinner(ui, cx, cy, 18.0 * s, now, s);
                let size = (18.0 * s).round();
                let tw = ui.measure(&self.busy, size, true);
                ui.label(&self.busy, cx - tw / 2.0, cy + 30.0 * s, size, true, color::TEXT);
            }
            Screen::Chars => self.draw_chars(ui, w, h, s, mouse),
            Screen::NewChar => self.draw_new_char(ui, w, h, s, mouse),
        }
        if self.deleting.is_some() {
            self.draw_confirm(ui, w, h, s, mouse);
        }
    }

    fn button(&mut self, ui: &mut Ui, target: Target, r: [f32; 4], label: &str, kind: Kind, s: f32, mouse: (f64, f64)) {
        ui.button(r, label, kind, inside(r, mouse), s);
        self.targets.push((target, r));
    }

    /// A text link: `lead` in grey, then `label` in gold.
    fn link(&mut self, ui: &mut Ui, target: Target, cx: f32, y: f32, lead: &str, label: &str, size: f32, mouse: (f64, f64)) {
        let lw = ui.measure(lead, size, false);
        let tw = ui.measure(label, size, true);
        let x = cx - (lw + tw) / 2.0;
        let r = [x + lw, y, tw, size * 1.3];
        ui.label(lead, x, y, size, false, color::TEXT_DIM);
        ui.label(label, x + lw, y, size, true, if inside(r, mouse) { color::ACCENT_HI } else { color::ACCENT });
        self.targets.push((target, r));
    }

    #[allow(clippy::too_many_arguments)]
    fn field(&mut self, ui: &mut Ui, i: usize, label: &str, placeholder: &str, x: f32, y: f32, width: f32, s: f32, now: i32) -> f32 {
        let size = (13.0 * s).round().max(9.0);
        ui.label(label, x, y, size, true, color::TEXT_DIM);
        let r = [x, y + 20.0 * s, width, 38.0 * s];
        let focused = self.focus == i;
        ui.field_box(r, focused, s);
        let value = &self.fields[i];
        let fsize = (16.0 * s).round().max(10.0);
        let ty = r[1] + (r[3] - fsize * 1.22) / 2.0;
        if value.is_empty() && !focused {
            ui.label(placeholder, r[0] + 12.0 * s, ty, fsize, false, color::TEXT_MUTED);
        } else {
            let mut shown = if Self::field_is_secret(i) { "•".repeat(value.chars().count()) } else { value.clone() };
            if focused && (now / 530) % 2 == 0 {
                shown.push('|');
            }
            // Long text scrolls: show its end.
            let mut start = 0;
            while start < shown.len() && ui.measure(&shown[start..], fsize, false) > width - 24.0 * s {
                start += shown[start..].chars().next().map_or(1, |c| c.len_utf8());
            }
            ui.label(&shown[start..], r[0] + 12.0 * s, ty, fsize, false, color::TEXT);
        }
        self.targets.push((Target::Field(i), r));
        y + 72.0 * s
    }

    fn message_line(&self, ui: &mut Ui, cx: f32, y: f32, max_w: f32, s: f32) {
        if let Some((text, error)) = &self.message {
            let size = (14.0 * s).round().max(9.0);
            let tw = ui.measure(text, size, true).min(max_w);
            let c = if *error { color::DANGER } else { color::GOOD };
            theme::fill(ui.renderer, [cx - tw / 2.0 - 12.0 * s, y - 4.0 * s, tw + 24.0 * s, size * 1.25 + 8.0 * s], 6.0 * s, c, 0.16);
            ui.label(text, cx - tw / 2.0, y, size, true, theme::shade(c, 0.35));
        }
    }

    /// Where the sign-in and register cards go: right of the art's middle on wide windows,
    /// below the logo on tall ones.
    fn card_rect(w: f32, h: f32, bw: f32, bh: f32) -> [f32; 4] {
        if w >= 1.3 * h {
            let x = (w * 0.79 - bw / 2.0).min(w - bw - 24.0);
            [x, ((h - bh) / 2.0).max(16.0), bw, bh]
        } else {
            [(w - bw) / 2.0, (h - bh - 28.0).max(h * 0.45).min(h - bh - 8.0), bw, bh]
        }
    }

    fn draw_login(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, mouse: (f64, f64), now: i32) {
        let r = Self::card_rect(w, h, 380.0 * s, 420.0 * s);
        let [x, y, bw, bh] = r;
        theme::panel(ui.renderer, r, s);
        let pad = 28.0 * s;
        ui.label("Welcome back", x + pad, y + 24.0 * s, (26.0 * s).round(), true, color::TEXT);
        ui.label("Sign in to continue your adventure", x + pad, y + 60.0 * s, (14.0 * s).round(), false, color::TEXT_DIM);
        let mut fy = y + 96.0 * s;
        fy = self.field(ui, EMAIL, "Email", "you@example.com", x + pad, fy, bw - 2.0 * pad, s, now);
        fy = self.field(ui, PASSWORD, "Password", "Your password", x + pad, fy, bw - 2.0 * pad, s, now);
        // Remember me, as a switch.
        let size = (14.0 * s).round().max(9.0);
        let sw = [x + pad, fy + 2.0 * s, 36.0 * s, 20.0 * s];
        theme::fill(ui.renderer, sw, sw[3] / 2.0, if self.remember { color::ACCENT } else { 0x3A4050 }, 1.0);
        let knob_x = if self.remember { sw[0] + sw[2] - sw[3] } else { sw[0] };
        theme::fill(ui.renderer, [knob_x + 2.0 * s, sw[1] + 2.0 * s, sw[3] - 4.0 * s, sw[3] - 4.0 * s], sw[3], 0xFFFFFF, 1.0);
        ui.label("Remember me", sw[0] + sw[2] + 10.0 * s, sw[1] + 1.0 * s, size, false, color::TEXT_DIM);
        self.targets.push((Target::Remember, [sw[0], sw[1], 160.0 * s, sw[3]]));
        self.button(ui, Target::SignIn, [x + pad, fy + 40.0 * s, bw - 2.0 * pad, 44.0 * s], "Sign in", Kind::Primary, s, mouse);
        theme::fill(ui.renderer, [x + pad, y + bh - 54.0 * s, bw - 2.0 * pad, s.max(1.0)], 0.0, color::BORDER, 1.0);
        self.link(ui, Target::ToRegister, x + bw / 2.0, y + bh - 40.0 * s, "New here?  ", "Create an account", size, mouse);
        self.message_line(ui, x + bw / 2.0, y + bh + 16.0 * s, bw, s);
        let small = (12.0 * s).round().max(9.0);
        ui.label(&self.endpoint_label, 12.0 * s, h - small * 1.7, small, false, color::TEXT_MUTED);
    }

    fn draw_register(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, mouse: (f64, f64), now: i32) {
        let r = Self::card_rect(w, h, 380.0 * s, 520.0 * s);
        let [x, y, bw, bh] = r;
        theme::panel(ui.renderer, r, s);
        let pad = 28.0 * s;
        ui.label("Create your account", x + pad, y + 24.0 * s, (24.0 * s).round(), true, color::TEXT);
        let mut fy = y + 66.0 * s;
        for (i, label, hint) in [
            (NAME, "Name", "Up to 10 letters"),
            (EMAIL, "Email", "you@example.com"),
            (PASSWORD, "Password", "Choose a password"),
            (CONFIRM, "Confirm password", "Type it again"),
        ] {
            fy = self.field(ui, i, label, hint, x + pad, fy, bw - 2.0 * pad, s, now);
        }
        self.button(ui, Target::Register, [x + pad, fy + 4.0 * s, bw - 2.0 * pad, 44.0 * s], "Create account", Kind::Primary, s, mouse);
        let size = (14.0 * s).round().max(9.0);
        self.link(ui, Target::ToLogin, x + bw / 2.0, y + bh - 36.0 * s, "Have an account?  ", "Sign in", size, mouse);
        self.message_line(ui, x + bw / 2.0, y + bh + 16.0 * s, bw, s);
    }

    fn draw_chars(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, mouse: (f64, f64)) {
        let Some(list) = self.list.clone() else { return };
        // The top bar: who, their fame and gold, and Log out.
        let bar = [0.0, 0.0, w, 68.0 * s];
        theme::rounded(ui.renderer, bar, 0.0, (color::PANEL_TOP, 0.92), (color::PANEL_BOTTOM, 0.92));
        theme::fill(ui.renderer, [0.0, bar[3] - s.max(1.0), w, s.max(1.0)], 0.0, color::BORDER, 1.0);
        let pad = 24.0 * s;
        ui.label(&list.name, pad, 12.0 * s, (24.0 * s).round(), true, color::TEXT);
        let size = (14.0 * s).round();
        let mut x = pad;
        for (label, value, c) in [("Fame", list.fame, color::ACCENT), ("Gold", list.credits, 0xFFE27A)] {
            ui.label(label, x, 42.0 * s, size, false, color::TEXT_DIM);
            x += ui.measure(label, size, false) + 6.0 * s;
            let v = value.to_string();
            ui.label(&v, x, 42.0 * s, size, true, c);
            x += ui.measure(&v, size, true) + 18.0 * s;
        }
        self.button(ui, Target::Logout, [w - pad - 110.0 * s, 16.0 * s, 110.0 * s, 36.0 * s], "Log out", Kind::Secondary, s, mouse);

        // Characters, then a card for a new one while there's room.
        let (cw, ch, gap) = (440.0 * s, 96.0 * s, 14.0 * s);
        let room = (list.chars.len() as i32) < list.max_num_chars;
        let count = list.chars.len() + usize::from(room);
        let cols = (((w - 2.0 * pad + gap) / (cw + gap)).floor() as usize).clamp(1, 3).min(count.max(1));
        let rows = count.div_ceil(cols);
        let total_w = cols as f32 * (cw + gap) - gap;
        let x0 = (w - total_w) / 2.0;
        let y0 = (bar[3] + 70.0 * s).max((h - rows as f32 * (ch + gap)) / 2.0);
        let title = "Choose your character";
        let ts = (24.0 * s).round();
        let tw = ui.measure(title, ts, true);
        ui.label(title, (w - tw) / 2.0, y0 - 46.0 * s, ts, true, color::TEXT);
        let cell = |i: usize| [x0 + (i % cols) as f32 * (cw + gap), y0 + (i / cols) as f32 * (ch + gap), cw, ch];
        for (i, c) in list.chars.iter().enumerate() {
            self.char_card(ui, c, cell(i), s, mouse);
        }
        if room {
            let r = cell(list.chars.len());
            let over = inside(r, mouse) && self.deleting.is_none();
            let edge = if over { color::ACCENT } else { color::BORDER };
            theme::frame(ui.renderer, r, 10.0 * s, s.max(1.0), (edge, 0.9), ((0x1A1D24, 0.6), (0x14161B, 0.6)));
            let plus = (34.0 * s).round();
            let label = "New character";
            let size = (18.0 * s).round();
            let lw = ui.measure(label, size, true);
            let pw = ui.measure("+", plus, true);
            let total = pw + 12.0 * s + lw;
            let cx = r[0] + (r[2] - total) / 2.0;
            let c = if over { color::ACCENT_HI } else { color::TEXT_DIM };
            ui.label("+", cx, r[1] + (r[3] - plus * 1.22) / 2.0, plus, true, c);
            ui.label(label, cx + pw + 12.0 * s, r[1] + (r[3] - size * 1.22) / 2.0, size, true, c);
            self.targets.push((Target::NewChar, r));
        }
        let bottom = y0 + rows as f32 * (ch + gap);
        // The server: arrows to switch when there's a choice.
        if let Some(server) = list.servers.get(self.server.min(list.servers.len().saturating_sub(1))) {
            let size = (15.0 * s).round();
            let label = format!("Server: {}", server.name);
            let lw = ui.measure(&label, size, true);
            let pill_w = lw + if list.servers.len() > 1 { 90.0 * s } else { 36.0 * s };
            let pill = [(w - pill_w) / 2.0, bottom + 10.0 * s, pill_w, 34.0 * s];
            theme::frame(ui.renderer, pill, pill[3] / 2.0, s.max(1.0), (color::BORDER, 1.0), ((color::PANEL_TOP, 0.9), (color::PANEL_BOTTOM, 0.9)));
            ui.label(&label, pill[0] + (pill[2] - lw) / 2.0, pill[1] + (pill[3] - size * 1.22) / 2.0, size, true, color::TEXT);
            if list.servers.len() > 1 {
                for (t, arrow, ax) in [(Target::ServerPrev, "‹", pill[0] + 10.0 * s), (Target::ServerNext, "›", pill[0] + pill[2] - 26.0 * s)] {
                    let r = [ax - 6.0 * s, pill[1], 28.0 * s, pill[3]];
                    let big = (22.0 * s).round();
                    ui.label(arrow, ax, pill[1] + (pill[3] - big * 1.22) / 2.0, big, true, if inside(r, mouse) { color::ACCENT_HI } else { color::ACCENT });
                    self.targets.push((t, r));
                }
            }
        }
        self.message_line(ui, w / 2.0, bottom + 60.0 * s, w * 0.8, s);
        let hint = "Click a character to play";
        let hs = (13.0 * s).round();
        let hw = ui.measure(hint, hs, false);
        ui.label(hint, (w - hw) / 2.0, h - hs * 2.2, hs, false, color::TEXT_MUTED);
    }

    fn char_card(&mut self, ui: &mut Ui, c: &CharSummary, r: [f32; 4], s: f32, mouse: (f64, f64)) {
        let over = inside(r, mouse) && self.deleting.is_none();
        let r = if over { [r[0], r[1] - 2.0 * s, r[2], r[3]] } else { r };
        theme::shadow(ui.renderer, r, 10.0 * s, if over { 16.0 * s } else { 9.0 * s }, if over { 0.55 } else { 0.4 });
        let edge = if over { color::ACCENT } else { color::BORDER_HI };
        theme::frame(ui.renderer, r, 10.0 * s, s.max(1.0), (edge, 1.0), ((color::PANEL_TOP, 0.95), (color::PANEL_BOTTOM, 0.97)));
        self.targets.push((Target::Char(c.id), r));
        // The portrait in a dark well.
        let well = [r[0] + 12.0 * s, r[1] + 12.0 * s, r[3] - 24.0 * s, r[3] - 24.0 * s];
        theme::rounded(ui.renderer, well, 8.0 * s, (0x0E1015, 0.9), (0x1A1E26, 0.9));
        let class = ui.data.objects.get(&c.object_type);
        let skin = (c.skin != 0).then(|| ui.data.objects.get(&(c.skin as u16))).flatten();
        if let Some(d) = skin.or(class) {
            portrait(ui, d, c.tex1, c.tex2, well[0] + well[2] / 2.0, well[1] + well[3] / 2.0, s * 0.95);
        }
        let tx = well[0] + well[2] + 14.0 * s;
        let name = class.map_or("Unknown", |d| d.display_id.as_str());
        ui.label(name, tx, r[1] + 16.0 * s, (20.0 * s).round(), true, color::TEXT);
        let size = (14.0 * s).round();
        let mut x = tx;
        for (label, value, col) in [("Level", c.level, color::TEXT), ("Fame", c.fame, color::ACCENT)] {
            ui.label(label, x, r[1] + 50.0 * s, size, false, color::TEXT_DIM);
            x += ui.measure(label, size, false) + 5.0 * s;
            let v = value.to_string();
            ui.label(&v, x, r[1] + 50.0 * s, size, true, col);
            x += ui.measure(&v, size, true) + 14.0 * s;
        }
        // What it wears, edged by tier.
        let side = 38.0 * s;
        for (k, &item) in c.equipment.iter().take(4).enumerate() {
            let sx = r[0] + r[2] - 14.0 * s - (4 - k) as f32 * (side + 6.0 * s) + 6.0 * s;
            let sr = [sx, r[1] + r[3] - side - 12.0 * s, side, side];
            let edge = if item > 0 { tier_edge(ui.data, item) } else { None };
            ui.slot_box(sr, edge, false, s);
            if item > 0 {
                ui.item_icon(item, 80, sr[0] + side / 2.0, sr[1] + side / 2.0, s * 0.8);
            }
        }
        // Delete, top right, only under the mouse.
        let d = [r[0] + r[2] - 30.0 * s, r[1] + 6.0 * s, 24.0 * s, 24.0 * s];
        if over || inside(d, mouse) {
            let over_d = inside(d, mouse);
            theme::fill(ui.renderer, d, 12.0 * s, if over_d { color::DANGER } else { 0x3A4050 }, 0.95);
            let size = (13.0 * s).round();
            let xw = ui.measure("✕", size, true);
            ui.label("✕", d[0] + (d[2] - xw) / 2.0, d[1] + (d[3] - size * 1.22) / 2.0, size, true, 0xFFFFFF);
            self.targets.push((Target::Delete(c.id), d));
        }
    }

    fn draw_new_char(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, mouse: (f64, f64)) {
        let Some(list) = self.list.clone() else { return };
        let classes = player_classes(ui.data);
        let pad = 24.0 * s;
        let title = "Choose a class";
        let ts = (26.0 * s).round();
        let tw = ui.measure(title, ts, true);
        ui.label(title, (w - tw) / 2.0, pad, ts, true, color::TEXT);
        let (tw_, th_, gap) = (128.0 * s, 156.0 * s, 12.0 * s);
        let cols = (((w - 2.0 * pad + gap) / (tw_ + gap)).floor() as usize).clamp(1, 9);
        let rows = classes.len().div_ceil(cols);
        let x0 = (w - (cols as f32 * (tw_ + gap) - gap)) / 2.0;
        let y0 = (pad + 56.0 * s).max((h - rows as f32 * (th_ + gap)) / 2.0);
        for (i, d) in classes.iter().enumerate() {
            let r = [x0 + (i % cols) as f32 * (tw_ + gap), y0 + (i / cols) as f32 * (th_ + gap), tw_, th_];
            let need = unlock(d, &list);
            let over = inside(r, mouse) && need.is_none();
            let r = if over { [r[0], r[1] - 3.0 * s, r[2], r[3]] } else { r };
            theme::shadow(ui.renderer, r, 10.0 * s, if over { 14.0 * s } else { 8.0 * s }, 0.45);
            let edge = if over { color::ACCENT } else { color::BORDER_HI };
            theme::frame(ui.renderer, r, 10.0 * s, s.max(1.0), (edge, 1.0), ((color::PANEL_TOP, 0.95), (color::PANEL_BOTTOM, 0.97)));
            let well = [r[0] + 14.0 * s, r[1] + 12.0 * s, r[2] - 28.0 * s, r[2] - 28.0 * s];
            theme::rounded(ui.renderer, well, 8.0 * s, (0x0E1015, 0.85), (0x1A1E26, 0.85));
            portrait(ui, d, 0, 0, well[0] + well[2] / 2.0, well[1] + well[3] / 2.0, s * 1.15);
            let size = (15.0 * s).round();
            let nw = ui.measure(&d.display_id, size, true);
            ui.label(&d.display_id, r[0] + (tw_ - nw) / 2.0, well[1] + well[3] + 8.0 * s, size, true, if need.is_none() { color::TEXT } else { color::TEXT_MUTED });
            match need {
                None => self.targets.push((Target::Class(d.object_type), r)),
                Some((t, level)) => {
                    theme::fill(ui.renderer, r, 10.0 * s, 0x000000, 0.5);
                    let req = ui.data.objects.get(&t).map_or("?", |c| c.display_id.as_str());
                    let text = format!("Level {level} {req}");
                    let small = (12.0 * s).round().max(8.0);
                    let lw = ui.measure(&text, small, true);
                    let by = r[1] + r[3] - 26.0 * s;
                    theme::fill(ui.renderer, [r[0] + (tw_ - lw) / 2.0 - 8.0 * s, by - 2.0 * s, lw + 16.0 * s, small * 1.3 + 4.0 * s], 6.0 * s, 0x000000, 0.6);
                    ui.label(&text, r[0] + (tw_ - lw) / 2.0, by, small, true, 0xFFB070);
                }
            }
        }
        let bottom = y0 + rows as f32 * (th_ + gap);
        self.button(ui, Target::Back, [(w - 140.0 * s) / 2.0, (bottom + 8.0 * s).min(h - 52.0 * s), 140.0 * s, 40.0 * s], "Back", Kind::Secondary, s, mouse);
        self.message_line(ui, w / 2.0, (bottom + 60.0 * s).min(h - 24.0 * s), w * 0.8, s);
    }

    fn draw_confirm(&mut self, ui: &mut Ui, w: f32, h: f32, s: f32, mouse: (f64, f64)) {
        let Some(id) = self.deleting else { return };
        let what = self
            .list
            .as_ref()
            .and_then(|l| l.chars.iter().find(|c| c.id == id))
            .map(|c| format!("{} (level {})", ui.data.objects.get(&c.object_type).map_or("character", |d| d.display_id.as_str()), c.level))
            .unwrap_or_else(|| "this character".into());
        // Only the dialog's buttons answer while it's up.
        self.targets.clear();
        ui.rect(0.0, 0.0, w, h, 0x000000, 0.55);
        let r = [(w - 440.0 * s) / 2.0, (h - 170.0 * s) / 2.0, 440.0 * s, 170.0 * s];
        theme::panel(ui.renderer, r, s);
        ui.label(&format!("Delete {what}?"), r[0] + 26.0 * s, r[1] + 24.0 * s, (20.0 * s).round(), true, color::TEXT);
        ui.label("Its items and fame are gone for good.", r[0] + 26.0 * s, r[1] + 58.0 * s, (14.0 * s).round(), false, color::TEXT_DIM);
        let by = r[1] + r[3] - 62.0 * s;
        self.button(ui, Target::CancelDelete, [r[0] + r[2] - 290.0 * s, by, 128.0 * s, 40.0 * s], "Keep it", Kind::Secondary, s, mouse);
        self.button(ui, Target::ConfirmDelete, [r[0] + r[2] - 152.0 * s, by, 128.0 * s, 40.0 * s], "Delete", Kind::Danger, s, mouse);
    }
}

/// The title art covering the window, slowly drifting, with a vignette; darker behind lists.
fn draw_backdrop(ui: &mut Ui, w: f32, h: f32, title: Option<(AtlasRect, u32, u32)>, screen: Screen, now: i32) {
    ui.rect(0.0, 0.0, w, h, 0x07080A, 1.0);
    if let Some((rect, iw, ih)) = title {
        // A slow zoom in and out keeps the screen alive.
        let breathe = 1.03 + 0.02 * ((now as f32) / 9000.0).sin();
        let k = (w / iw as f32).max(h / ih as f32) * breathe;
        let (dw, dh) = (iw as f32 * k, ih as f32 * k);
        ui.renderer.sprite(rect, Quad::rect((w - dw) / 2.0, (h - dh) / 2.0, dw, dh), Tint::NONE);
    }
    let dark = match screen {
        Screen::Login | Screen::Register | Screen::Busy => 0.15,
        _ => 0.62,
    };
    ui.rect(0.0, 0.0, w, h, 0x000000, dark);
    // Vignette: darker toward the top and bottom edges.
    let band = h * 0.28;
    theme::rounded(ui.renderer, [0.0, 0.0, w, band], 0.0, (0x000000, 0.55), (0x000000, 0.0));
    theme::rounded(ui.renderer, [0.0, h - band, w, band], 0.0, (0x000000, 0.0), (0x000000, 0.6));
}

/// Dots chasing round a circle.
fn spinner(ui: &mut Ui, cx: f32, cy: f32, radius: f32, now: i32, s: f32) {
    const DOTS: i32 = 10;
    let head = (now / 90) % DOTS;
    for i in 0..DOTS {
        let a = i as f32 / DOTS as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let age = (head - i).rem_euclid(DOTS) as f32 / DOTS as f32;
        let d = 5.0 * s;
        let (x, y) = (cx + radius * a.cos(), cy + radius * a.sin());
        theme::fill(ui.renderer, [x - d / 2.0, y - d / 2.0, d, d], d / 2.0, color::ACCENT, 1.0 - age * 0.85);
    }
}

/// A class's standing frame, dyed, centred at (cx, cy).
fn portrait(ui: &mut Ui, desc: &ObjectDesc, tex1: i32, tex2: i32, cx: f32, cy: f32, s: f32) {
    let TextureRef::Animated { set, index } = &desc.texture.pick(0).main else { return };
    let Some(src) = ui.sprites.char_src(set, *index, 0, 0) else { return };
    let dye = ui.sprites.char_mask_src(set, *index, 0, 0).map(|mask| Dye { mask, tex1, tex2 });
    let Some(img) = ui.redrawer.get(ui.sprites, ui.renderer, src, false, 100, dye) else { return };
    let (iw, ih) = (img.w as f32 * s, img.h as f32 * s);
    // The redrawn image has room for the outline above and beside it: centre the sprite.
    let pad = crate::redraw::PAD as f32 * s;
    ui.renderer.sprite(img.rect, Quad::rect((cx - iw / 2.0).round(), (cy - ih / 2.0 + pad / 2.0).round(), iw, ih), Tint::NONE);
}

/// The menu Escape opens in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuChoice {
    Resume,
    CharSelect,
    Logout,
}

/// Draw the in-game menu; returns the buttons for clicks.
pub fn draw_game_menu(ui: &mut Ui, w: f32, h: f32, mouse: (f64, f64)) -> Vec<(MenuChoice, [f32; 4])> {
    let s = scale(w, h);
    ui.rect(0.0, 0.0, w, h, 0x000000, 0.5);
    let r = [(w - 320.0 * s) / 2.0, (h - 258.0 * s) / 2.0, 320.0 * s, 258.0 * s];
    theme::panel(ui.renderer, r, s);
    let size = (22.0 * s).round();
    let tw = ui.measure("Paused", size, true);
    ui.label("Paused", r[0] + (r[2] - tw) / 2.0, r[1] + 20.0 * s, size, true, color::TEXT);
    let hint = "Esc to close";
    let hs = (12.0 * s).round();
    let hw = ui.measure(hint, hs, false);
    ui.label(hint, r[0] + (r[2] - hw) / 2.0, r[1] + 50.0 * s, hs, false, color::TEXT_MUTED);
    let mut out = Vec::new();
    for (i, (choice, label, kind)) in [
        (MenuChoice::Resume, "Back to the game", Kind::Primary),
        (MenuChoice::CharSelect, "Character select", Kind::Secondary),
        (MenuChoice::Logout, "Log out", Kind::Danger),
    ]
    .into_iter()
    .enumerate()
    {
        let b = [r[0] + 30.0 * s, r[1] + 80.0 * s + i as f32 * 54.0 * s, r[2] - 60.0 * s, 42.0 * s];
        ui.button(b, label, kind, inside(b, mouse), s);
        out.push((choice, b));
    }
    out
}

pub fn menu_click(buttons: &[(MenuChoice, [f32; 4])], pos: (f64, f64)) -> Option<MenuChoice> {
    buttons.iter().find(|(_, r)| inside(*r, pos)).map(|(c, _)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_tab() {
        let mut f = Front::default();
        f.show(Screen::Login);
        f.key(KeyCode::KeyA, Some("a@b"));
        f.key(KeyCode::Tab, None);
        f.key(KeyCode::KeyX, Some("pw"));
        f.key(KeyCode::Backspace, None);
        assert_eq!(f.fields[EMAIL], "a@b");
        assert_eq!(f.fields[PASSWORD], "p");
        assert_eq!(f.key(KeyCode::Enter, None), Some(Action::SignIn));
        // Escape from Register goes back to Login.
        f.show(Screen::Register);
        f.key(KeyCode::Escape, None);
        assert_eq!(f.screen, Screen::Login);
    }

    #[test]
    fn cards_fit_wide_and_tall_windows() {
        for (w, h) in [(1920.0, 1080.0), (800.0, 1000.0), (640.0, 480.0)] {
            let s = scale(w, h);
            let r = Front::card_rect(w, h, 380.0 * s, 420.0 * s);
            assert!(r[0] >= 0.0 && r[0] + r[2] <= w + 0.5, "{w}×{h}: {r:?}");
            assert!(r[1] >= 0.0, "{w}×{h}: {r:?}");
        }
    }
}
