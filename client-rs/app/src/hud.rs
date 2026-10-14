//! The HUD (`HUDView`) and chat (`TextBox`), laid out for whatever size the window is.
//!
//! The AIR client's HUD is a 200×600 panel beside a 600×600 view. Here the panel keeps its
//! look, with room for stats, potions and the interact area (bags, portals) below the
//! inventory, scaled by one factor (`Layout::s`) chosen from the window's width and height. It
//! sits at the right edge and the world fills the rest. Everything is worked out each frame
//! from the current size, so resizing or going fullscreen just works.
//!
//! Items move the AIR client's way (`ItemGridMediator`): drag between slots to swap, onto the
//! world to drop, double-click to equip or use (or take from a bag), shift-click to use.

use crate::redraw::{Dye, Redrawer};
use crate::sprites::{SrcRect, Sprites};
use crate::text::{Fonts, Style, Text};
use bsk_data::{GameData, ObjectClass, ObjectDesc, TextureData, TextureRef};
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
const PANEL_H: f32 = 690.0;

impl Layout {
    pub fn new(w: u32, h: u32) -> Self {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        // The whole panel fits the height, and takes at most 28% of the width.
        let s = (h / PANEL_H).min(w * 0.28 / PANEL_W).clamp(0.45, 4.0);
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

    /// A rectangle on the panel (x, y, w, h), from HUD units.
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        let (px, py) = self.at(x, y);
        [px, py, w * self.s, h * self.s]
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

fn inside(r: [f32; 4], (x, y): (f64, f64)) -> bool {
    let (x, y) = (x as f32, y as f32);
    x >= r[0] && y >= r[1] && x < r[0] + r[2] && y < r[1] + r[3]
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

    fn cut_rect(&mut self, r: [f32; 4], cut: f32, cuts: [bool; 4], rgb: u32, alpha: f32) {
        cut_rect(self.renderer, r, cut, cuts, rgb, alpha);
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

    /// `getRedrawnTextureFromType(type, size, true)`, centred on (cx, cy) at `scale`.
    fn item_icon(&mut self, item: i32, size: i32, cx: f32, cy: f32, scale: f32) {
        let Some(desc) = self.data.objects.get(&(item as u16)) else { return };
        let Some(src) = self.src(&desc.texture) else { return };
        let size = if src.h == 16 { size / 2 } else { size };
        let Some(icon) = self.redrawer.get(self.sprites, self.renderer, src, false, size, None) else { return };
        let (w, h) = (icon.w as f32 * scale, icon.h as f32 * scale);
        self.renderer.sprite(icon.rect, Quad::rect((cx - w / 2.0).round(), (cy - h / 2.0).round(), w, h), Tint::NONE);
    }

    /// A tile drawn as one flat colour (`greyColorFilter`), keeping its shape.
    fn silhouette(&mut self, set: &str, index: u32, x: f32, y: f32, side: f32, rgb: u32) {
        if let Some(rect) = self.sprites.tile(set, index) {
            let [r, g, b] = bsk_render::rgb_f32(rgb);
            // Colour multiplied to zero, then the flat colour added: the shape in one colour.
            self.renderer.sprite(rect, Quad::rect(x, y, side, side), Tint { mul: [0.0, 0.0, 0.0, 1.0], add: [r, g, b] });
        }
    }
}

/// `GraphicsUtil.drawCutEdgeRect`: a rectangle with the chosen corners (top left, top right,
/// bottom right, bottom left) cut off at 45°.
pub fn cut_rect(renderer: &mut Renderer, r: [f32; 4], cut: f32, cuts: [bool; 4], rgb: u32, alpha: f32) {
    let [x, y, w, h] = r;
    let mut pts: Vec<[f32; 2]> = Vec::with_capacity(8);
    let corners = [
        ([x, y + cut], [x + cut, y], [x, y]),
        ([x + w - cut, y], [x + w, y + cut], [x + w, y]),
        ([x + w, y + h - cut], [x + w - cut, y + h], [x + w, y + h]),
        ([x + cut, y + h], [x, y + h - cut], [x, y + h]),
    ];
    for (i, (a, b, c)) in corners.into_iter().enumerate() {
        if cuts[i] {
            pts.push(a);
            pts.push(b);
        } else {
            pts.push(c);
        }
    }
    let white = renderer.white();
    let tint = Tint::color(rgb, alpha);
    for k in 1..pts.len() - 1 {
        renderer.triangle(white, [pts[0], pts[k], pts[k + 1]], [[0.5, 0.5]; 3], tint);
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
pub mod stat {
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
    pub const TEX1: u8 = 32;
    pub const TEX2: u8 = 33;
    pub const ATTACK_BOOST: u8 = 48;
    pub const DEFENSE_BOOST: u8 = 49;
    pub const SPEED_BOOST: u8 = 50;
    pub const VITALITY_BOOST: u8 = 51;
    pub const WISDOM_BOOST: u8 = 52;
    pub const DEXTERITY_BOOST: u8 = 53;
    pub const CURR_FAME: u8 = 57;
    pub const NEXT_CLASS_QUEST_FAME: u8 = 58;
    pub const HEALTH_POTION_STACK: u8 = 65;
    pub const MAGIC_POTION_STACK: u8 = 66;
    pub const BACKPACK_0: u8 = 67;
    pub const HAS_BACKPACK: u8 = 75;
    pub const TEXTURE: u8 = 76;
}

/// The stat holding inventory slot `slot` (0-11 the inventory, 12-19 the backpack).
pub fn slot_stat(slot: u8) -> u8 {
    if slot < 12 { stat::INVENTORY_0 + slot } else { stat::BACKPACK_0 + slot - 12 }
}

/// The health and magic potions kept in stacks (`PotionInventoryModel`): item, slot id.
pub const POTIONS: [(i32, u8); 2] = [(2594, 254), (2595, 255)];

/// Colours (`Slot`, `HUDView`, `CharacterWindowBackground`).
const SLOT_FILL: u32 = 0x545454;
const SLOT_ICON: u32 = 0x363636;
const PANEL_FILL: u32 = 0x3A3A3A;
const MINIMAP_FILL: u32 = 0x1B1B1B;
const BOX_OUTER: u32 = 0x676767;
const BOX_INNER: u32 = 0x454545;

/// What a slot holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    /// One of the four equipment slots, with the class's slot type.
    Equip(i32),
    /// Inventory or backpack, with its number key (1-8).
    Inventory(u8),
    /// A bag's.
    Container,
    /// A potion stack (F, V), holding `count`.
    Potion(i32),
}

/// One slot on the panel.
#[derive(Clone, Copy, Debug)]
pub struct Slot {
    pub owner: i32,
    pub index: u8,
    pub item: i32,
    pub kind: SlotKind,
    rect: [f32; 4],
    /// Corners cut, as ItemGrid gives them.
    cuts: [bool; 4],
}

impl Slot {
    pub fn is(&self, other: &Slot) -> bool {
        self.owner == other.owner && self.index == other.index
    }
}

/// `Slot` types (`SlotType` in item XML).
pub fn slot_type_name(t: i32) -> &'static str {
    match t {
        0 => "Any",
        1 => "Sword",
        2 => "Dagger",
        3 => "Bow",
        4 => "Tome",
        5 => "Shield",
        6 => "Leather Armor",
        7 => "Armor",
        8 => "Wand",
        9 => "Accessory",
        10 => "Potion",
        11 => "Spell",
        12 => "Holy Seal",
        13 => "Cloak",
        14 => "Robe",
        15 => "Quiver",
        16 => "Helm",
        17 => "Staff",
        18 => "Poison",
        19 => "Skull",
        20 => "Trap",
        21 => "Orb",
        22 => "Prism",
        23 => "Scepter",
        24 => "Shuriken",
        _ => "Item",
    }
}

/// `Slot.drawBackground`: the silhouette an empty equipment slot shows.
fn slot_type_icon(t: i32) -> Option<(&'static str, u32)> {
    Some(match t {
        1 => ("lofiObj5", 48),
        2 => ("lofiObj5", 96),
        3 => ("lofiObj5", 80),
        4 => ("lofiObj6", 80),
        5 => ("lofiObj6", 112),
        6 => ("lofiObj5", 0),
        7 => ("lofiObj5", 32),
        8 => ("lofiObj5", 64),
        9 => ("lofiObj", 44),
        11 => ("lofiObj6", 64),
        12 => ("lofiObj6", 160),
        13 => ("lofiObj6", 32),
        14 => ("lofiObj5", 16),
        15 => ("lofiObj6", 48),
        16 => ("lofiObj6", 96),
        17 => ("lofiObj5", 112),
        18 => ("lofiObj6", 128),
        19 => ("lofiObj6", 0),
        20 => ("lofiObj6", 16),
        21 => ("lofiObj6", 144),
        22 => ("lofiObj6", 176),
        23 => ("lofiObj6", 192),
        _ => return None,
    })
}

/// The class's `<SlotTypes>` (the first four are the equipment slots).
pub fn class_slot_types(class: Option<&ObjectDesc>) -> Vec<i32> {
    class
        .and_then(|c| c.xml.text_of("SlotTypes"))
        .map(|s| s.split(',').map(|t| t.trim().parse().unwrap_or(0)).collect())
        .unwrap_or_default()
}

/// An item's `<SlotType>`.
pub fn item_slot_type(data: &GameData, item: i32) -> i32 {
    data.objects.get(&(item as u16)).map_or(0, |d| d.xml.int("SlotType", 0))
}

/// Whether a slot of kind `kind` can take `item` (`canHoldItem`): equipment only its type.
pub fn can_hold(data: &GameData, kind: SlotKind, item: i32) -> bool {
    match kind {
        _ if item <= 0 => true,
        SlotKind::Equip(t) => t == 0 || item_slot_type(data, item) == t,
        SlotKind::Potion(_) => false,
        _ => true,
    }
}

/// The inventory page showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Inventory,
    Backpack,
}

/// What can be clicked that isn't a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Nexus,
    Page(Page),
    /// Enter the portal.
    Enter(i32),
}

/// The thing beside us the interact area shows (`InteractPanel`): a bag or a portal.
#[derive(Clone, Debug)]
pub struct Interact {
    pub id: i32,
    pub name: String,
    pub container: bool,
}

/// `GameSprite.updateNearestInteractive`: the closest bag or portal within a tile.
pub fn interact_target(world: &World, data: &GameData) -> Option<Interact> {
    let me = world.player()?;
    world
        .objects
        .values()
        .filter_map(|o| {
            let d = data.objects.get(&o.object_type)?;
            let container = matches!(d.class, ObjectClass::Container | ObjectClass::OneWayContainer);
            let portal = matches!(d.class, ObjectClass::Portal | ObjectClass::GuildHallPortal);
            let dist = (o.x - me.x).hypot(o.y - me.y);
            ((container || portal) && dist < 1.0).then(|| (dist, o, d, container))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, o, d, container)| Interact {
            id: o.id,
            // Bags by their type's name (their name stat is internal); portals by their name.
            name: if container { d.display_id.clone() } else { o.name().map(str::to_owned).unwrap_or_else(|| d.display_id.clone()) },
            container,
        })
}

/// HUD state kept between frames.
#[derive(Default)]
pub struct HudState {
    pub page: Page,
    /// A slot pressed but not yet dragged, and where the press was.
    press: Option<(Slot, (f64, f64))>,
    /// The slot whose item is being dragged.
    pub drag: Option<Slot>,
    /// The last click on a slot, for double clicks.
    last_click: Option<(i32, u8, i32)>,
}

/// What the player did on the HUD.
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Swap(Slot, Slot),
    /// Double click: equip it, or use it (`equipOrUseInventory`); from a bag, take it.
    EquipOrUse(Slot),
    /// Shift-click: use it.
    Use(Slot),
    /// Dragged onto the world.
    Drop(Slot),
    /// Dragged from a bag onto the panel: into the first free slot.
    Take(Slot),
    Button(Button),
}

/// Everything clickable on the panel this frame.
pub struct Panel {
    pub slots: Vec<Slot>,
    buttons: Vec<(Button, [f32; 4])>,
    pub interact: Option<Interact>,
    has_backpack: bool,
}

const DOUBLE_CLICK_MS: i32 = 350;

impl Panel {
    pub fn new(lay: &Layout, world: &World, data: &GameData, page: Page) -> Self {
        let mut panel = Panel { slots: Vec::new(), buttons: Vec::new(), interact: None, has_backpack: false };
        let Some(me) = world.player() else { return panel };
        let item = |o: &GameObj, slot: u8| o.stat_int(slot_stat(slot)).unwrap_or(-1);
        let types = class_slot_types(data.objects.get(&me.object_type));
        // ItemGrid's cuts for one row of four, and for two rows.
        let row_cuts = |i: usize, rows: usize| -> [bool; 4] {
            let (col, row) = (i % 4, i / 4);
            let top = row == 0;
            let bottom = row + 1 == rows;
            [col == 0 && top, col == 3 && top, col == 3 && bottom, col == 0 && bottom]
        };
        for i in 0..4u8 {
            panel.slots.push(Slot {
                owner: me.id,
                index: i,
                item: item(me, i),
                kind: SlotKind::Equip(types.get(i as usize).copied().unwrap_or(0)),
                rect: lay.rect(14.0 + 44.0 * i as f32, 306.0, 40.0, 40.0),
                cuts: row_cuts(i as usize, 1),
            });
        }
        panel.has_backpack = me.stat_int(stat::HAS_BACKPACK).unwrap_or(0) != 0;
        let page = if panel.has_backpack { page } else { Page::Inventory };
        let first = if page == Page::Backpack { 12 } else { 4 };
        for i in 0..8u8 {
            panel.slots.push(Slot {
                owner: me.id,
                index: first + i,
                item: item(me, first + i),
                kind: SlotKind::Inventory(i + 1),
                rect: lay.rect(14.0 + 44.0 * (i % 4) as f32, 376.0 + 44.0 * (i / 4) as f32, 40.0, 40.0),
                cuts: row_cuts(i as usize, 2),
            });
        }
        if panel.has_backpack {
            panel.buttons.push((Button::Page(Page::Inventory), lay.rect(14.0, 352.0, 84.0, 20.0)));
            panel.buttons.push((Button::Page(Page::Backpack), lay.rect(102.0, 352.0, 84.0, 20.0)));
        }
        for (k, (potion, slot_id)) in POTIONS.iter().enumerate() {
            let count = me.stat_int(if k == 0 { stat::HEALTH_POTION_STACK } else { stat::MAGIC_POTION_STACK }).unwrap_or(0);
            panel.slots.push(Slot {
                owner: me.id,
                index: *slot_id,
                item: *potion,
                kind: SlotKind::Potion(count),
                rect: lay.rect(14.0 + 44.0 * k as f32, 528.0, 40.0, 40.0),
                cuts: [k == 0, k == 1, k == 1, k == 0],
            });
        }
        panel.buttons.push((Button::Nexus, lay.rect(108.0, 536.0, 80.0, 24.0)));
        // InteractPanel: a bag's eight slots, or a portal's Enter button.
        panel.interact = interact_target(world, data);
        if let Some(target) = &panel.interact {
            if target.container {
                if let Some(bag) = world.objects.get(&target.id) {
                    for i in 0..8u8 {
                        panel.slots.push(Slot {
                            owner: bag.id,
                            index: i,
                            item: item(bag, i),
                            kind: SlotKind::Container,
                            rect: lay.rect(14.0 + 44.0 * (i % 4) as f32, 600.0 + 44.0 * (i / 4) as f32, 40.0, 40.0),
                            cuts: row_cuts(i as usize, 2),
                        });
                    }
                }
            } else {
                panel.buttons.push((Button::Enter(target.id), lay.rect(50.0, 620.0, 100.0, 30.0)));
            }
        }
        panel
    }

    fn slot_at(&self, pos: (f64, f64)) -> Option<Slot> {
        self.slots.iter().find(|s| inside(s.rect, pos)).copied()
    }

    fn button_at(&self, pos: (f64, f64)) -> Option<Button> {
        self.buttons.iter().find(|(_, r)| inside(*r, pos)).map(|(b, _)| *b)
    }

    /// The left button went down at `pos`. True if the HUD took it (so the game doesn't fire).
    pub fn press(&self, state: &mut HudState, lay: &Layout, pos: (f64, f64), shift: bool, now: i32, actions: &mut Vec<Action>) -> bool {
        if let Some(slot) = self.slot_at(pos) {
            if slot.item > 0 {
                if shift {
                    actions.push(Action::Use(slot));
                } else if state.last_click.is_some_and(|(o, i, t)| o == slot.owner && i == slot.index && now - t < DOUBLE_CLICK_MS) {
                    actions.push(Action::EquipOrUse(slot));
                    state.last_click = None;
                } else {
                    state.last_click = Some((slot.owner, slot.index, now));
                    state.press = Some((slot, pos));
                }
            }
            return true;
        }
        if let Some(button) = self.button_at(pos) {
            if let Button::Page(p) = button {
                state.page = p;
            }
            actions.push(Action::Button(button));
            return true;
        }
        lay.on_panel(pos.0, pos.1)
    }

    /// The mouse moved: a press becomes a drag once it has gone a few pixels.
    pub fn moved(&self, state: &mut HudState, pos: (f64, f64)) {
        if let Some((slot, from)) = state.press {
            if (pos.0 - from.0).hypot(pos.1 - from.1) > 4.0 {
                state.drag = Some(slot);
                state.press = None;
            }
        }
    }

    /// The left button came up at `pos`.
    pub fn release(&self, state: &mut HudState, lay: &Layout, data: &GameData, pos: (f64, f64), actions: &mut Vec<Action>) {
        state.press = None;
        let Some(from) = state.drag.take() else { return };
        if let Some(to) = self.slot_at(pos) {
            if to.is(&from) {
                return;
            }
            let fits = can_hold(data, to.kind, from.item) && can_hold(data, from.kind, to.item);
            if fits && !matches!(from.kind, SlotKind::Potion(_)) {
                actions.push(Action::Swap(from, to));
            }
        } else if !lay.on_panel(pos.0, pos.1) {
            if from.kind != SlotKind::Container {
                actions.push(Action::Drop(from));
            }
        } else if from.kind == SlotKind::Container {
            actions.push(Action::Take(from));
        }
    }
}

/// The panel: minimap, portrait and name, bars, equipment, inventory, stats, potions, the
/// interact area.
pub fn draw_panel(ui: &mut Ui, lay: &Layout, world: &World, camera_angle: f64, colors: &mut MapColors, panel: &Panel, state: &HudState, mouse: (f64, f64)) {
    let s = lay.s;
    ui.rect(lay.panel_x, 0.0, lay.w - lay.panel_x, lay.h, PANEL_FILL, 1.0);
    let Some(me) = world.player() else { return };
    draw_minimap(ui, lay, world, me, camera_angle, colors);

    // CharacterDetailsView: portrait and name.
    draw_portrait(ui, lay, me);
    let (x, y) = lay.at(42.0, 204.0);
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
        let (bx, by) = lay.at(12.0, 232.0 + 24.0 * i as f32);
        status_bar(ui, lay, bx, by, &label, value, max, color);
    }

    // The equipment's box (HUDView's equippedGridBG).
    ui.cut_rect(lay.rect(10.0, 302.0, 180.0, 48.0), 6.0 * s, [true; 4], BOX_OUTER, 1.0);
    ui.cut_rect(lay.rect(11.0, 303.0, 178.0, 46.0), 6.0 * s, [true; 4], BOX_INNER, 1.0);

    // Page tabs, when there's a backpack.
    for (button, r) in &panel.buttons {
        let Button::Page(p) = button else { continue };
        let on = *p == state.page;
        ui.cut_rect(*r, 4.0 * s, [true, true, false, false], if on { 0x5F5F5F } else { 0x2E2E2E }, 1.0);
        let label = if *p == Page::Inventory { "Inventory" } else { "Backpack" };
        let size = lay.font(12.0);
        let w = ui.measure(label, size, true);
        ui.label(label, r[0] + (r[2] - w) / 2.0, r[1] + (r[3] - size * 1.2) / 2.0, size, true, if on { 0xFFFFFF } else { 0x9B9B9B });
    }

    let hover = if state.drag.is_some() { panel.slot_at(mouse) } else { None };
    for slot in &panel.slots {
        let dragged = state.drag.is_some_and(|d| d.is(slot));
        let target = hover.is_some_and(|h| h.is(slot));
        draw_slot(ui, lay, slot, dragged, target);
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
    for (i, (label, id, boost_id, tag)) in stats.into_iter().enumerate() {
        let (value, boost) = (stat(id), stat(boost_id));
        let (tx, ty) = lay.at(14.0 + 92.0 * (i % 2) as f32, 466.0 + 18.0 * (i / 2) as f32);
        let size = lay.font(13.0);
        ui.label(label, tx, ty, size, true, 0xB3B3B3);
        let color = if max_of(tag).is_some_and(|m| value - boost >= m) {
            0xFCDF00
        } else if boost > 0 {
            0x6EFF6E
        } else {
            0xFFFFFF
        };
        // The value, then any boost smaller; both shrink if the column is too narrow.
        let room = 58.0 * s;
        let value_s = value.to_string();
        let boost_s = if boost != 0 { format!(" {boost:+}") } else { String::new() };
        let mut vs = size;
        let mut bs = lay.font(10.0);
        let mut width = ui.measure(&value_s, vs, false) + ui.measure(&boost_s, bs, false);
        if width > room {
            let k = room / width;
            vs = (vs * k).floor().max(8.0);
            bs = (bs * k).floor().max(7.0);
            width = ui.measure(&value_s, vs, false);
        } else {
            width = ui.measure(&value_s, vs, false);
        }
        let vx = tx + 32.0 * s;
        ui.label(&value_s, vx, ty + (size - vs) * 0.8, vs, false, color);
        if boost != 0 {
            let bc = if boost > 0 { 0x6EFF6E } else { 0xFF6E6E };
            ui.label(&boost_s, vx + width, ty + (size - bs) * 0.8, bs, false, bc);
        }
    }

    // Buttons.
    for (button, r) in &panel.buttons {
        let label = match button {
            Button::Nexus => "Nexus (R)".to_owned(),
            Button::Enter(_) => "Enter (0)".to_owned(),
            Button::Page(_) => continue,
        };
        let over = inside(*r, mouse);
        ui.cut_rect(*r, 4.0 * s, [true; 4], if over { 0x7A7A7A } else { 0x5F5F5F }, 1.0);
        let size = lay.font(13.0);
        let w = ui.measure(&label, size, true);
        ui.label(&label, r[0] + (r[2] - w) / 2.0, r[1] + (r[3] - size * 1.25) / 2.0, size, true, 0xFFFFFF);
    }

    // InteractPanel's title.
    if let Some(target) = &panel.interact {
        ui.rect(lay.panel_x + 8.0 * s, 578.0 * s, 184.0 * s, 1.0_f32.max(s), 0x545454, 1.0);
        let size = lay.font(15.0);
        let w = ui.measure(&target.name, size, true);
        ui.label(&target.name, lay.panel_x + (PANEL_W * s - w) / 2.0, 582.0 * s, size, true, 0xFFFFFF);
    }
}

fn draw_slot(ui: &mut Ui, lay: &Layout, slot: &Slot, dragged: bool, target: bool) {
    let s = lay.s;
    let [x, y, w, h] = slot.rect;
    ui.cut_rect(slot.rect, 4.0 * s, slot.cuts, if target { 0x7A7A7A } else { SLOT_FILL }, 1.0);
    let empty = slot.item <= 0 || dragged || matches!(slot.kind, SlotKind::Potion(0));
    if empty {
        match slot.kind {
            SlotKind::Equip(t) => {
                if let Some((set, index)) = slot_type_icon(t) {
                    ui.silhouette(set, index, x + 4.0 * s, y + 4.0 * s, 32.0 * s, SLOT_ICON);
                }
            }
            SlotKind::Inventory(key) => {
                let size = lay.font(26.0);
                let label = key.to_string();
                let lw = ui.measure(&label, size, true);
                ui.label(&label, x + (w - lw) / 2.0, y + (h - size * 1.2) / 2.0, size, true, SLOT_ICON);
            }
            _ => {}
        }
    }
    if !empty || matches!(slot.kind, SlotKind::Potion(_)) {
        if matches!(slot.kind, SlotKind::Potion(0)) {
            // An empty stack shows its potion greyed out.
            if let Some(src) = ui.data.objects.get(&(slot.item as u16)).and_then(|d| ui.src(&d.texture)) {
                if let Some(rect) = ui.sprites.atlas(src) {
                    ui.renderer.sprite(rect, Quad::rect(x + 4.0 * s, y + 4.0 * s, 32.0 * s, 32.0 * s), Tint { mul: [0.0, 0.0, 0.0, 1.0], add: bsk_render::rgb_f32(SLOT_ICON) });
                }
            }
        } else {
            ui.item_icon(slot.item, 80, x + w / 2.0, y + h / 2.0, s);
        }
    }
    // Stack counts and potion keys (ItemTileSprite's DOSE_MATRIX corner).
    let small = lay.font(12.0);
    if let SlotKind::Potion(count) = slot.kind {
        ui.label(&count.to_string(), x + 3.0 * s, y + h - small * 1.25, small, true, 0xFFFFFF);
        let key = if slot.index == POTIONS[0].1 { "F" } else { "V" };
        let kw = ui.measure(key, small, true);
        ui.label(key, x + w - kw - 3.0 * s, y + 1.0 * s, small, true, 0xB3B3B3);
    } else if !empty {
        let doses = ui.data.objects.get(&(slot.item as u16)).and_then(|d| d.xml.text_of("Doses").or(d.xml.text_of("Quantity")).map(str::to_owned));
        if let Some(n) = doses {
            ui.label(&n, x + 3.0 * s, y + h - small * 1.25, small, true, 0xFFFFFF);
        }
    }
}

/// The player's portrait (`Player.getPortrait`): the class (or skin) standing, dyed.
fn draw_portrait(ui: &mut Ui, lay: &Layout, me: &GameObj) {
    let skin = me.stat_int(stat::TEXTURE).filter(|&t| t != 0).and_then(|t| ui.data.objects.get(&(t as u16)));
    let Some(desc) = skin.or_else(|| ui.data.objects.get(&me.object_type)) else { return };
    let TextureRef::Animated { set, index } = &desc.texture.pick(0).main else { return };
    let Some(src) = ui.sprites.char_src(set, *index, 0, 0) else { return };
    let dye = ui.sprites.char_mask_src(set, *index, 0, 0).map(|mask| Dye {
        mask,
        tex1: me.stat_int(stat::TEX1).unwrap_or(0),
        tex2: me.stat_int(stat::TEX2).unwrap_or(0),
    });
    let Some(icon) = ui.redrawer.get(ui.sprites, ui.renderer, src, false, 60, dye) else { return };
    let (cx, cy) = lay.at(22.0, 214.0);
    let k = lay.s * 0.8;
    let (w, h) = (icon.w as f32 * k, icon.h as f32 * k);
    ui.renderer.sprite(icon.rect, Quad::rect((cx - w / 2.0).round(), (cy - h / 2.0).round(), w, h), Tint::NONE);
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

/// `MiniMap.draw` at its first zoom (4 units a tile), north up, the player's arrow turned with
/// the camera. Static objects that fill their square show their colour over the ground.
fn draw_minimap(ui: &mut Ui, lay: &Layout, world: &World, me: &GameObj, camera_angle: f64, colors: &mut MapColors) {
    const SIDE: f32 = 192.0;
    let (x0, y0) = lay.at(4.0, 4.0);
    let side = SIDE * lay.s;
    ui.rect(x0 - lay.s, y0 - lay.s, side + 2.0 * lay.s, side + 2.0 * lay.s, 0x262626, 1.0);
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

/// Drawn over everything: the item being dragged, and the tooltip for what's under the mouse.
pub fn draw_overlay(ui: &mut Ui, lay: &Layout, world: &World, panel: &Panel, state: &HudState, mouse: (f64, f64)) {
    let (mx, my) = (mouse.0 as f32, mouse.1 as f32);
    if let Some(drag) = state.drag {
        ui.item_icon(drag.item, 80, mx, my, lay.s);
        return;
    }
    let Some(slot) = panel.slot_at(mouse) else { return };
    let me = world.player();
    let lines = if slot.item > 0 {
        item_tooltip(ui.data, slot.item, me, slot.kind)
    } else {
        let what = match slot.kind {
            SlotKind::Equip(t) => slot_type_name(t),
            _ => "item",
        };
        vec![Line::new(&format!("Empty {what} Slot"), 14.0, false, 0x9B9B9B)]
    };
    draw_tooltip(ui, lay, &lines, if slot.item > 0 { Some(slot.item) } else { None }, mouse);
}

/// One line of a tooltip.
pub struct Line {
    text: String,
    size: f32,
    bold: bool,
    color: u32,
}

impl Line {
    fn new(text: &str, size: f32, bold: bool, color: u32) -> Self {
        Line { text: text.to_owned(), size, bold, color }
    }
}

const TOOLTIP_W: f32 = 230.0;
const ATTR: u32 = 0xB3B3B3;
const VALUE: u32 = 0xFFFF8F;

fn tier_tag(xml: &bsk_data::xml::Element) -> Option<(String, u32)> {
    let hidden = ["Consumable", "NoTierTag", "Treasure", "PetFood"];
    if hidden.iter().any(|t| xml.has(t)) {
        return None;
    }
    let color = [("Admin", 0xFC65D0), ("Ethereal", 0xF61D1D), ("Ascended", 0x1BF7DD), ("Legendary", 0xFFC800)]
        .iter()
        .find(|(t, _)| xml.has(t))
        .map(|(_, c)| *c);
    if let Some(tier) = xml.text_of("Tier") {
        Some((format!("T{}", tier.trim()), color.unwrap_or(0xFFFFFF)))
    } else if xml.attr("setType").is_some() {
        Some(("ST".into(), color.unwrap_or(0xF2FF00)))
    } else {
        Some(("UT".into(), color.unwrap_or(0xE22BC9)))
    }
}

fn stat_name(id: i32) -> &'static str {
    match id {
        0 => "Maximum HP",
        3 => "Maximum MP",
        20 => "Attack",
        21 => "Defense",
        22 => "Speed",
        26 => "Vitality",
        27 => "Wisdom",
        28 => "Dexterity",
        _ => "Stat",
    }
}

/// `EquipmentToolTip`, the parts that matter: title and tier, description, weapon numbers,
/// ability costs, stat bonuses, and how to use it.
pub fn item_tooltip(data: &GameData, item: i32, me: Option<&GameObj>, kind: SlotKind) -> Vec<Line> {
    let Some(desc) = data.objects.get(&(item as u16)) else { return vec![Line::new("Unknown item", 14.0, true, 0xFFFFFF)] };
    let xml = &desc.xml;
    let mut out = Vec::new();
    let title_color = [("Admin", 0xFF8BDE), ("Ethereal", 0xFF5A5A), ("Ascended", 0x66FFF0), ("Legendary", 0xFFE066)]
        .iter()
        .find(|(t, _)| xml.has(t))
        .map_or(0xFFFFFF, |(_, c)| *c);
    out.push(Line::new(&desc.display_id, 16.0, true, title_color));
    if let Some((tag, color)) = tier_tag(xml) {
        out.push(Line::new(&tag, 14.0, true, color));
    }
    if let Some(d) = xml.text_of("Description") {
        out.push(Line::new(d.trim(), 13.0, false, ATTR));
    }
    let mut attr = |label: &str, value: String| out.push(Line::new(&format!("{label}: {value}"), 13.0, false, VALUE));
    if let Some(p) = desc.projectiles.get(&0) {
        attr("Shots", desc.num_projectiles().to_string());
        let dmg = if p.min_damage == p.max_damage { p.min_damage.to_string() } else { format!("{} - {}", p.min_damage, p.max_damage) };
        attr("Damage", dmg);
        let range = p.speed as f64 * p.lifetime as f64 / 10000.0;
        attr("Range", format!("{}", (range * 100.0).round() / 100.0));
        if [1, 2, 3, 8, 17, 24].contains(&xml.int("SlotType", 0)) {
            attr("Rate of Fire", format!("{:.0}%", desc.rate_of_fire() * 100.0));
        }
        for e in &p.effects {
            attr("Shot effect", e.clone());
        }
    }
    if xml.has("Usable") {
        if let Some(cost) = xml.text_of("MpCost") {
            attr("MP Cost", cost.trim().to_owned());
        }
        attr("Cooldown", format!("{} secs", xml.number("Cooldown", 0.5)));
    }
    if let Some(doses) = xml.text_of("Doses") {
        attr("Doses", doses.trim().to_owned());
    }
    if let Some(fame) = xml.text_of("FameBonus") {
        attr("Fame Bonus", format!("{}%", fame.trim()));
    }
    let boosts: Vec<_> = xml.children_named("ActivateOnEquip").filter(|a| a.text.trim() == "IncrementStat").collect();
    if !boosts.is_empty() {
        out.push(Line::new("On Equip:", 13.0, false, ATTR));
        for b in boosts {
            let amount = b.attr_int("amount", 0);
            let color = if amount < 0 { 0xFF0000 } else { 0x00FF00 };
            out.push(Line::new(&format!("    {amount:+} {}", stat_name(b.attr_int("stat", -1))), 13.0, false, color));
        }
    }
    for a in xml.children_named("Activate") {
        let effect = a.text.trim();
        let detail = match effect {
            "Heal" | "HealNova" => a.attr("amount").map(|v| format!("{effect}: {v} HP")),
            "Magic" | "MagicNova" => a.attr("amount").map(|v| format!("{effect}: {v} MP")),
            "IncrementStat" => Some(format!("Permanently +{} {}", a.attr_int("amount", 0), stat_name(a.attr_int("stat", -1)))),
            "ConditionEffectSelf" | "ConditionEffectAura" => a.attr("effect").map(|e| {
                let secs = a.attr_number("duration", 0.0);
                format!("{e} for {secs} secs")
            }),
            _ => None,
        };
        out.push(Line::new(&detail.unwrap_or_else(|| effect.to_owned()), 13.0, false, VALUE));
    }
    // Restrictions.
    let mut note = |t: &str, c: u32| out.push(Line::new(t, 13.0, false, c));
    if xml.has("Consumable") {
        note("Consumed with use", ATTR);
    }
    if xml.has("Usable") && matches!(kind, SlotKind::Equip(_)) {
        note("Press [Space] in the world to use", 0xFFFFFF);
    } else if xml.has("Consumable") || xml.has("InvUse") {
        note("Double-click or shift-click to use", 0xFFFFFF);
    } else if kind == SlotKind::Container {
        note("Double-click to take", ATTR);
    } else if !matches!(kind, SlotKind::Equip(_)) && xml.has("SlotType") && xml.int("SlotType", 0) != 10 {
        note("Double-click to equip", ATTR);
    }
    if xml.has("Soulbound") {
        note("Soulbound", ATTR);
    }
    if let Some(me) = me {
        let slot_type = xml.int("SlotType", 0);
        let equips = class_slot_types(data.objects.get(&me.object_type));
        if (1..=24).contains(&slot_type) && slot_type != 10 && !equips.iter().take(4).any(|&t| t == slot_type) {
            let class = data.objects.get(&me.object_type).map_or("you", |c| c.display_id.as_str());
            note(&format!("Not usable by {class}"), 0xFC8642);
        }
    }
    out
}

/// A tooltip box beside the mouse, kept inside the window: the item's icon and title on top,
/// then the lines wrapped to the box.
fn draw_tooltip(ui: &mut Ui, lay: &Layout, lines: &[Line], icon: Option<i32>, mouse: (f64, f64)) {
    let s = lay.s;
    let w = TOOLTIP_W * s;
    let pad = 8.0 * s;
    let inner = w - 2.0 * pad;
    let icon_w = if icon.is_some() { 44.0 * s } else { 0.0 };
    // Wrap every line; the first ones (title, tier) sit beside the icon.
    let mut rows: Vec<(String, f32, bool, u32, f32)> = Vec::new();
    let mut y: f32 = 0.0;
    for (i, line) in lines.iter().enumerate() {
        let size = lay.font(line.size);
        let indent = if i < 2 { icon_w } else { 0.0 };
        if i == 2 {
            y = y.max(icon_w) + 4.0 * s;
        }
        for part in wrap(ui, &[(line.text.clone(), line.color)], size, inner - indent) {
            let text: String = part.into_iter().map(|(t, _)| t).collect();
            rows.push((text, size, line.bold, line.color, y));
            y += size * 1.25;
        }
    }
    let h = y.max(icon_w) + 2.0 * pad;
    let (mut x0, mut y0) = (mouse.0 as f32 + 16.0 * s, mouse.1 as f32 + 8.0 * s);
    if x0 + w > lay.w {
        x0 = mouse.0 as f32 - w - 8.0 * s;
    }
    if y0 + h > lay.h {
        y0 = (lay.h - h).max(0.0);
    }
    x0 = x0.max(0.0);
    ui.rect(x0 - s, y0 - s, w + 2.0 * s, h + 2.0 * s, 0x9B9B9B, 1.0);
    ui.rect(x0, y0, w, h, 0x363636, 0.95);
    if let Some(item) = icon {
        ui.item_icon(item, 60, x0 + pad + icon_w / 2.0 - 2.0 * s, y0 + pad + icon_w / 2.0 - 2.0 * s, s * 0.9);
    }
    for (i, (text, size, bold, color, ry)) in rows.iter().enumerate() {
        let indent = if i < 2 && icon.is_some() && *ry < icon_w { icon_w } else { 0.0 };
        ui.label(text, x0 + pad + indent, y0 + pad + ry, *size, *bold, *color);
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
            assert!(PANEL_H * l.s <= h as f32 + 0.5 || l.s == 0.45, "{w}×{h}: panel too tall");
            assert!(panel_w <= w as f32 * 0.28 + 1.0 || l.s == 0.45, "{w}×{h}: panel too wide");
            assert!(l.view_w() > 0.0);
        }
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

    #[test]
    fn slot_stats() {
        assert_eq!(slot_stat(0), 8);
        assert_eq!(slot_stat(11), 19);
        assert_eq!(slot_stat(12), 67);
        assert_eq!(slot_stat(19), 74);
    }
}
