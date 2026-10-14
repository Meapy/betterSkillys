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

/// `<Projectile id="n">` of a weapon or monster (`ProjectileProperties.as`). AS3 reads Speed,
/// LifetimeMS and Size as ints.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectileDesc {
    pub bullet_type: i32,
    /// The object (by id) whose texture the projectile shows.
    pub object_id: String,
    pub lifetime: i32,
    pub speed: i32,
    /// -1: use the container's size.
    pub size: i32,
    pub min_damage: i32,
    pub max_damage: i32,
    /// Condition effect names it inflicts.
    pub effects: Vec<String>,
    pub multi_hit: bool,
    pub passes_cover: bool,
    pub armor_piercing: bool,
    pub wavy: bool,
    pub parametric: bool,
    pub boomerang: bool,
    pub amplitude: f64,
    pub frequency: f64,
    pub magnitude: f64,
}

impl ProjectileDesc {
    fn from_xml(xml: &Element) -> Self {
        let (min_damage, max_damage) = if xml.has("Damage") {
            (xml.int("Damage", 0), xml.int("Damage", 0))
        } else {
            (xml.int("MinDamage", 0), xml.int("MaxDamage", 0))
        };
        ProjectileDesc {
            bullet_type: xml.attr_int("id", 0),
            object_id: xml.text_of("ObjectId").unwrap_or("").to_owned(),
            lifetime: xml.int("LifetimeMS", 0),
            speed: xml.int("Speed", 0),
            size: if xml.has("Size") { crate::xml::to_int32(xml.number("Size", 0.0)) } else { -1 },
            min_damage,
            max_damage,
            effects: xml.children_named("ConditionEffect").map(|c| c.text.clone()).collect(),
            multi_hit: xml.has("MultiHit"),
            passes_cover: xml.has("PassesCover"),
            armor_piercing: xml.has("ArmorPiercing"),
            wavy: xml.has("Wavy"),
            parametric: xml.has("Parametric"),
            boomerang: xml.has("Boomerang"),
            amplitude: xml.number("Amplitude", 0.0),
            frequency: xml.number("Frequency", 1.0),
            magnitude: xml.number("Magnitude", 3.0),
        }
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
    pub projectiles: std::collections::HashMap<i32, ProjectileDesc>,
    /// `<Animation>`s, played now and then over the texture.
    pub animations: Vec<crate::animation::AnimationData>,
    pub xml: Element,
}

impl ObjectDesc {
    /// Weapons: `<NumProjectiles>` (1), `<ArcGap>` in degrees (11.25), `<RateOfFire>` (1).
    pub fn num_projectiles(&self) -> i32 {
        self.xml.int("NumProjectiles", 1)
    }

    pub fn arc_gap_degrees(&self) -> f64 {
        self.xml.number("ArcGap", 11.25)
    }

    pub fn rate_of_fire(&self) -> f64 {
        self.xml.number("RateOfFire", 1.0)
    }

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
            projectiles: xml
                .children_named("Projectile")
                .map(|p| {
                    let d = ProjectileDesc::from_xml(p);
                    (d.bullet_type, d)
                })
                .collect(),
            animations: xml.children_named("Animation").map(crate::animation::AnimationData::from_xml).collect(),
            xml: xml.clone(),
        }
    }
}
