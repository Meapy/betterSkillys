//! Which sprite an object or tile shows (`TextureDataConcrete.as`).

use crate::xml::{as3_int, Element};
use std::collections::HashMap;

/// One image source.
#[derive(Debug, Clone, PartialEq)]
pub enum TextureRef {
    /// `<Texture><File>set</File><Index>i</Index></Texture>`: tile `i` of an image set.
    Image { set: String, index: u32 },
    /// `<AnimatedTexture>`: character `index` of an animated sheet (walk/attack frames).
    Animated { set: String, index: u32 },
    /// `<RandomTexture>`: one of these, chosen per tile or object.
    Random(Vec<TextureData>),
    /// `<RemoteTexture>`: fetched from Kabam's servers; not supported (draws nothing).
    Remote,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextureData {
    pub main: TextureRef,
    /// Dye mask (set, index).
    pub mask: Option<(String, u32)>,
    /// `<AltTexture id="n">`: chosen by the object's ALT_TEXTURE stat.
    pub alt: HashMap<i32, TextureData>,
}

impl TextureData {
    /// Like `new TextureDataConcrete(xml)`.
    pub fn from_xml(xml: &Element) -> Self {
        let mut data = TextureData { main: TextureRef::None, mask: None, alt: HashMap::new() };
        let main = ["Texture", "AnimatedTexture", "RemoteTexture", "RandomTexture"].iter().find_map(|n| xml.child(n));
        data.parse(main.unwrap_or(xml));
        for alt in xml.children_named("AltTexture") {
            data.alt.insert(alt.attr_int("id", 0), TextureData::from_xml(alt));
        }
        if let Some(mask) = xml.child("Mask") {
            data.mask = Some(image(mask));
        }
        data
    }

    fn parse(&mut self, xml: &Element) {
        self.main = match xml.name.as_str() {
            "Texture" => {
                let (set, index) = image(xml);
                TextureRef::Image { set, index }
            }
            "AnimatedTexture" => {
                let (set, index) = image(xml);
                TextureRef::Animated { set, index }
            }
            "RandomTexture" => TextureRef::Random(xml.children.iter().map(TextureData::from_xml).collect()),
            "RemoteTexture" => TextureRef::Remote,
            _ => TextureRef::None,
        };
    }

    /// `getTexture(n)`: for random textures, choice `n` (wrapping).
    pub fn pick(&self, n: i32) -> &TextureData {
        match &self.main {
            TextureRef::Random(choices) if !choices.is_empty() => {
                choices[n.rem_euclid(choices.len() as i32) as usize].pick(n)
            }
            _ => self,
        }
    }
}

fn image(xml: &Element) -> (String, u32) {
    (xml.text_of("File").unwrap_or("").to_owned(), xml.child("Index").map_or(0, |i| as3_int(&i.text)) as u32)
}
