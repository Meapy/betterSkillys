//! Objects: players, monsters, items, walls, portals... (`ObjectProperties.as`, `ObjectLibrary.as`).

use crate::texture::TextureData;
use crate::xml::Element;

/// The game object class an entry is drawn and handled as (`ObjectLibrary.TYPE_MAP`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectClass {
    GameObject,
    Player,
    Character,
    Wall,
    ConnectedWall,
    CaveWall,
    DoubleWall,
    Container,
    OneWayContainer,
    Portal,
    GuildHallPortal,
    Projectile,
    Equipment,
    Merchant,
    Sign,
    SpiderWeb,
    Stalagmite,
    /// Everything else, by name (shops, NPCs, boards...): drawn as a plain object for now.
    Other(String),
}

impl ObjectClass {
    fn parse(s: &str) -> Self {
        match s {
            "" | "GameObject" => Self::GameObject,
            "Player" => Self::Player,
            "Character" => Self::Character,
            "Wall" => Self::Wall,
            "ConnectedWall" => Self::ConnectedWall,
            "CaveWall" => Self::CaveWall,
            "DoubleWall" => Self::DoubleWall,
            "Container" => Self::Container,
            "OneWayContainer" => Self::OneWayContainer,
            "Portal" => Self::Portal,
            "GuildHallPortal" => Self::GuildHallPortal,
            "Projectile" => Self::Projectile,
            "Equipment" => Self::Equipment,
            "Merchant" => Self::Merchant,
            "Sign" => Self::Sign,
            "SpiderWeb" => Self::SpiderWeb,
            "Stalagmite" => Self::Stalagmite,
            other => Self::Other(other.to_owned()),
        }
    }

    /// Drawn as a 3D block (walls) rather than a sprite.
    pub fn is_wall(&self) -> bool {
        matches!(self, Self::Wall | Self::ConnectedWall | Self::CaveWall | Self::DoubleWall)
    }
}

#[derive(Debug, Clone)]
pub struct ObjectDesc {
    pub object_type: u16,
    pub id: String,
    pub display_id: String,
    pub class: ObjectClass,
    pub texture: TextureData,
    /// `<Top>`: a wall's top face.
    pub top: Option<TextureData>,
    /// `<Model>`: the name of a 3D model.
    pub model: Option<String>,
    pub shadow_size: i32,
    pub player: bool,
    pub enemy: bool,
    pub draw_on_ground: bool,
    pub draw_under: bool,
    pub occupy_square: bool,
    pub full_occupy: bool,
    pub enemy_occupy_square: bool,
    pub is_static: bool,
    pub no_mini_map: bool,
    pub z: f64,
    pub flying: bool,
    pub color: Option<u32>,
    pub show_name: bool,
    pub dont_face_attacks: bool,
    pub dont_face_movement: bool,
    pub min_size: i32,
    pub max_size: i32,
    pub size_step: i32,
    pub angle_correction: f64,
    pub rotation: f64,
    pub shadow_color: u32,
    pub xml: Element,
}

impl ObjectDesc {
    /// `<ProtectFromSink>`: doesn't sink into water.
    pub fn protect_from_sink(&self) -> bool {
        self.xml.has("ProtectFromSink")
    }

    pub fn from_xml(xml: &Element) -> Self {
        let (mut min_size, mut max_size, mut size_step) = (100, 100, 5);
        if xml.has("Size") {
            min_size = xml.int("Size", 100);
            max_size = min_size;
            if max_size == -1 {
                (min_size, max_size) = (0, 0);
            }
        } else {
            min_size = xml.int("MinSize", min_size);
            max_size = xml.int("MaxSize", max_size);
            size_step = xml.int("SizeStep", size_step);
        }
        let draw_on_ground = xml.has("DrawOnGround");
        ObjectDesc {
            object_type: xml.attr_int("type", 0) as u16,
            id: xml.attr("id").unwrap_or("").to_owned(),
            display_id: xml.text_of("DisplayId").or(xml.attr("id")).unwrap_or("").to_owned(),
            class: ObjectClass::parse(xml.text_of("Class").unwrap_or("")),
            texture: TextureData::from_xml(xml),
            top: xml.child("Top").map(TextureData::from_xml),
            model: xml.text_of("Model").map(str::to_owned),
            shadow_size: xml.int("ShadowSize", 100),
            player: xml.has("Player"),
            enemy: xml.has("Enemy"),
            draw_on_ground,
            draw_under: draw_on_ground || xml.has("DrawUnder"),
            occupy_square: xml.has("OccupySquare"),
            full_occupy: xml.has("FullOccupy"),
            enemy_occupy_square: xml.has("EnemyOccupySquare"),
            is_static: xml.has("Static"),
            no_mini_map: xml.has("NoMiniMap"),
            z: xml.number("Z", 0.0),
            flying: xml.has("Flying"),
            color: xml.child("Color").map(|c| crate::xml::as3_int(&c.text) as u32),
            show_name: xml.has("ShowName"),
            dont_face_attacks: xml.has("DontFaceAttacks"),
            dont_face_movement: xml.has("DontFaceMovement"),
            min_size,
            max_size,
            size_step: size_step.max(1),
            angle_correction: xml.number("AngleCorrection", 0.0) * std::f64::consts::PI / 4.0,
            rotation: xml.number("Rotation", 0.0),
            shadow_color: xml.int("ShadowColor", 0) as u32,
            xml: xml.clone(),
        }
    }
}
