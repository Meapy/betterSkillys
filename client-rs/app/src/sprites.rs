//! Sprite sheets in the atlas, looked up the way the AIR client's `AssetLibrary` and
//! `AnimatedChars` do: by set name and tile index.

use bsk_assets::{AssetSource, Facing, Manifest};
use bsk_render::{AtlasRect, Renderer};
use std::collections::HashMap;

/// One sheet file in the atlas. A sheet taller than an atlas page is stored as several bands,
/// cut on a row boundary that every tile size registered for the file shares.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    band_height: u32,
    bands: Vec<AtlasRect>,
}

impl Sheet {
    /// The atlas rectangle for a pixel rectangle of the sheet, or `None` if it's outside the
    /// sheet or crosses a band edge.
    pub fn rect(&self, x: u32, y: u32, w: u32, h: u32) -> Option<AtlasRect> {
        if x + w > self.width || y + h > self.height {
            return None;
        }
        let band = (y / self.band_height) as usize;
        let top = y - band as u32 * self.band_height;
        let rect = self.bands.get(band)?;
        (top + h <= rect.h as u32).then(|| rect.sub(x, top, w, h))
    }
}

pub struct ImageSet {
    sheet: Option<usize>,
    pub tile_w: u32,
    pub tile_h: u32,
    cols: u32,
    pub count: u32,
}

pub struct CharSheet {
    sheet: usize,
    mask: Option<usize>,
    pub tile_w: u32,
    pub tile_h: u32,
    pub block_w: u32,
    pub block_h: u32,
    blocks_per_row: u32,
    pub count: u32,
    pub facing: Facing,
}

#[derive(Default)]
pub struct Sprites {
    sheets: Vec<Sheet>,
    sets: HashMap<String, ImageSet>,
    chars: HashMap<String, CharSheet>,
}

impl Sprites {
    /// Tile `index` of image set `set`.
    pub fn tile(&self, set: &str, index: u32) -> Option<AtlasRect> {
        let s = self.sets.get(set)?;
        let sheet = &self.sheets[s.sheet?];
        if index >= s.count {
            return None;
        }
        sheet.rect((index % s.cols) * s.tile_w, (index / s.cols) * s.tile_h, s.tile_w, s.tile_h)
    }

    pub fn set(&self, name: &str) -> Option<&ImageSet> {
        self.sets.get(name)
    }

    /// Frame `frame` (0..7 within the character's block, row `row`) of character `index`.
    /// Row 0 faces right (or down), the next rows are the other directions for sheets that have them.
    pub fn char_frame(&self, name: &str, index: u32, row: u32, frame: u32) -> Option<AtlasRect> {
        let c = self.chars.get(name)?;
        if index >= c.count {
            return None;
        }
        let bx = (index % c.blocks_per_row) * c.block_w;
        let by = (index / c.blocks_per_row) * c.block_h;
        self.sheets[c.sheet].rect(bx + frame * c.tile_w, by + row * c.tile_h, c.tile_w, c.tile_h)
    }

    pub fn char_sheet(&self, name: &str) -> Option<&CharSheet> {
        self.chars.get(name)
    }

    pub fn char_mask_frame(&self, name: &str, index: u32, row: u32, frame: u32) -> Option<AtlasRect> {
        let c = self.chars.get(name)?;
        let mask = &self.sheets[c.mask?];
        let bx = (index % c.blocks_per_row) * c.block_w;
        let by = (index / c.blocks_per_row) * c.block_h;
        mask.rect(bx + frame * c.tile_w, by + row * c.tile_h, c.tile_w, c.tile_h)
    }

    pub fn set_names(&self) -> impl Iterator<Item = &str> {
        self.sets.keys().map(String::as_str)
    }

    /// Fetch every sheet in the manifest (all at once, which matters in a browser) and put it in
    /// the atlas.
    pub async fn load(source: &AssetSource, manifest: &Manifest, renderer: &mut Renderer) -> Result<Self, String> {
        let files = manifest.sheet_files();
        let fetched = futures::future::join_all(files.iter().map(|f| source.load(f))).await;

        // Each file is cut into bands on a multiple of every row height registered for it.
        let mut heights: Vec<(&str, u32)> = Vec::new();
        for s in &manifest.image_sets {
            if let Some(f) = &s.file {
                heights.push((f, s.h));
            }
        }
        for c in &manifest.animated_chars {
            for f in std::iter::once(&c.file).chain(&c.mask) {
                heights.extend([(f.as_str(), c.sheet_h), (f.as_str(), c.h)]);
            }
        }
        let mut unit: HashMap<&str, u32> = HashMap::new();
        for (file, h) in heights {
            let u = unit.entry(file).or_insert(1);
            *u = lcm(*u, h.max(1));
        }

        let mut sprites = Sprites::default();
        let mut index_of: HashMap<&str, usize> = HashMap::new();
        let max = renderer.max_image_size();
        for (file, bytes) in files.iter().zip(fetched) {
            let bytes = bytes.map_err(|e| e.to_string())?;
            let image = bsk_assets::decode_image(file, &bytes).map_err(|e| e.to_string())?;
            let (width, height) = image.dimensions();
            let u = unit[file];
            if width > max || u > max {
                return Err(format!("{file}: {width}×{height} doesn't fit an atlas page"));
            }
            let band_height = (max / u) * u;
            let mut bands = Vec::new();
            let mut y = 0;
            while y < height {
                let h = band_height.min(height - y);
                let band = image::imageops::crop_imm(&image, 0, y, width, h).to_image();
                bands.push(renderer.add_image(&band).ok_or_else(|| format!("{file}: atlas full"))?);
                y += h;
            }
            index_of.insert(file, sprites.sheets.len());
            sprites.sheets.push(Sheet { width, height, band_height, bands });
        }

        for s in &manifest.image_sets {
            let sheet = s.file.as_deref().map(|f| index_of[f]);
            let (cols, count) = match sheet {
                Some(i) => {
                    let sh = &sprites.sheets[i];
                    let cols = sh.width / s.w;
                    (cols, cols * (sh.height / s.h))
                }
                None => (1, 1),
            };
            sprites.sets.insert(s.name.clone(), ImageSet { sheet, tile_w: s.w, tile_h: s.h, cols, count });
        }
        for c in &manifest.animated_chars {
            let sheet = index_of[c.file.as_str()];
            let sh = &sprites.sheets[sheet];
            let blocks_per_row = (sh.width / c.sheet_w).max(1);
            let count = blocks_per_row * (sh.height / c.sheet_h);
            sprites.chars.insert(
                c.name.clone(),
                CharSheet {
                    sheet,
                    mask: c.mask.as_deref().map(|m| index_of[m]),
                    tile_w: c.w,
                    tile_h: c.h,
                    block_w: c.sheet_w,
                    block_h: c.sheet_h,
                    blocks_per_row,
                    count,
                    facing: c.dir,
                },
            );
        }
        Ok(sprites)
    }
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn lcm(a: u32, b: u32) -> u32 {
    a / gcd(a, b) * b
}
