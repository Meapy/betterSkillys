//! Sprite sheets in the atlas, looked up the way the AIR client's `AssetLibrary` and
//! `AnimatedChars` do: by set name and tile index.

use bsk_assets::{AssetSource, Facing, Manifest};
use bsk_render::{AtlasRect, Renderer};
use std::collections::HashMap;

/// One sheet file in the atlas. A sheet bigger than an atlas page is stored as a grid of pieces,
/// cut on column and row boundaries that every tile size registered for the file shares.
pub struct Sheet {
    pub width: u32,
    pub height: u32,
    piece_w: u32,
    piece_h: u32,
    columns: u32,
    /// Row by row.
    pieces: Vec<AtlasRect>,
}

impl Sheet {
    /// The atlas rectangle for a pixel rectangle of the sheet, or `None` if it's outside the
    /// sheet or crosses a piece edge.
    pub fn rect(&self, x: u32, y: u32, w: u32, h: u32) -> Option<AtlasRect> {
        if x + w > self.width || y + h > self.height {
            return None;
        }
        let (px, py) = (x / self.piece_w, y / self.piece_h);
        let piece = self.pieces.get((py * self.columns + px) as usize)?;
        let (left, top) = (x - px * self.piece_w, y - py * self.piece_h);
        (left + w <= piece.w as u32 && top + h <= piece.h as u32).then(|| piece.sub(left, top, w, h))
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
        let t = web_time::Instant::now();
        let fetched = futures::future::join_all(files.iter().map(|f| source.load(f))).await;
        let fetch_ms = t.elapsed().as_millis();
        let (mut decode, mut upload) = (std::time::Duration::ZERO, std::time::Duration::ZERO);

        // Each file is cut into pieces whose sides are multiples of every tile and block size
        // registered for it, so no tile straddles two pieces.
        let mut sizes: Vec<(&str, u32, u32)> = Vec::new();
        for s in &manifest.image_sets {
            if let Some(f) = &s.file {
                sizes.push((f, s.w, s.h));
            }
        }
        for c in &manifest.animated_chars {
            for f in std::iter::once(&c.file).chain(&c.mask) {
                sizes.extend([(f.as_str(), c.sheet_w, c.sheet_h), (f.as_str(), c.w, c.h)]);
            }
        }
        let mut unit: HashMap<&str, (u32, u32)> = HashMap::new();
        for (file, w, h) in sizes {
            let u = unit.entry(file).or_insert((1, 1));
            *u = (lcm(u.0, w.max(1)), lcm(u.1, h.max(1)));
        }

        let mut sprites = Sprites::default();
        let mut index_of: HashMap<&str, usize> = HashMap::new();
        let max = renderer.max_image_size();
        for (file, bytes) in files.iter().zip(fetched) {
            let bytes = bytes.map_err(|e| e.to_string())?;
            let t = web_time::Instant::now();
            let image = bsk_assets::decode_image(file, &bytes).map_err(|e| e.to_string())?;
            decode += t.elapsed();
            let t = web_time::Instant::now();
            let (width, height) = image.dimensions();
            let (uw, uh) = unit[file];
            if uw > max || uh > max {
                return Err(format!("{file}: a {uw}×{uh} tile doesn't fit an atlas page"));
            }
            let (piece_w, piece_h) = ((max / uw) * uw, (max / uh) * uh);
            let columns = width.div_ceil(piece_w);
            let mut pieces = Vec::new();
            for y in (0..height).step_by(piece_h as usize) {
                for x in (0..width).step_by(piece_w as usize) {
                    let (w, h) = (piece_w.min(width - x), piece_h.min(height - y));
                    let piece = image::imageops::crop_imm(&image, x, y, w, h).to_image();
                    pieces.push(renderer.add_image(&piece).ok_or_else(|| format!("{file}: atlas full"))?);
                }
            }
            upload += t.elapsed();
            index_of.insert(file, sprites.sheets.len());
            sprites.sheets.push(Sheet { width, height, piece_w, piece_h, columns, pieces });
        }
        log::info!(
            "sheets: fetch {fetch_ms} ms, decode {} ms, atlas {} ms",
            decode.as_millis(),
            upload.as_millis()
        );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_finds_the_piece_holding_a_tile() {
        // A 300×250 sheet cut into 128×96 pieces: 3 columns, 3 rows.
        let mut pieces = Vec::new();
        for row in 0..3u16 {
            for col in 0..3u16 {
                let w = if col == 2 { 44 } else { 128 };
                let h = if row == 2 { 58 } else { 96 };
                pieces.push(AtlasRect { page: row * 3 + col, x: 100, y: 200, w, h });
            }
        }
        let sheet = Sheet { width: 300, height: 250, piece_w: 128, piece_h: 96, columns: 3, pieces };
        assert_eq!(sheet.rect(0, 0, 16, 16), Some(AtlasRect { page: 0, x: 100, y: 200, w: 16, h: 16 }));
        assert_eq!(sheet.rect(144, 112, 16, 16), Some(AtlasRect { page: 4, x: 116, y: 216, w: 16, h: 16 }));
        assert_eq!(sheet.rect(256, 192, 32, 32), Some(AtlasRect { page: 8, x: 100, y: 200, w: 32, h: 32 }));
        assert_eq!(sheet.rect(120, 0, 16, 16), None, "crosses a column edge");
        assert_eq!(sheet.rect(0, 240, 16, 16), None, "outside the sheet");
    }

    #[test]
    fn lcm_of_tile_sizes() {
        assert_eq!(lcm(8, 16), 16);
        assert_eq!(lcm(16, 24), 48);
        assert_eq!(lcm(40, 8), 40);
    }
}
