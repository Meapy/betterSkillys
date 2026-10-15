//! Texture atlas: every image the renderer draws lives in a few large pages, so a frame's sprites
//! go out in very few draw calls (the AIR client's `TextureAtlas`, without its size limit).

/// A rectangle of an atlas page, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtlasRect {
    pub page: u16,
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl AtlasRect {
    /// The part of this rectangle at (`x`, `y`) of size `w`×`h`, relative to its top left.
    pub fn sub(&self, x: u32, y: u32, w: u32, h: u32) -> AtlasRect {
        debug_assert!(x + w <= self.w as u32 && y + h <= self.h as u32);
        AtlasRect { page: self.page, x: self.x + x as u16, y: self.y + y as u16, w: w as u16, h: h as u16 }
    }
}

/// Gap between images, so linear filtering and rounding never pick up a neighbour's pixels.
const PADDING: u32 = 1;

struct Shelf {
    y: u32,
    height: u32,
    next_x: u32,
}

pub(crate) struct Page {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    shelves: Vec<Shelf>,
    next_y: u32,
}

pub(crate) struct Atlas {
    pub size: u32,
    pub pages: Vec<Page>,
}

impl Atlas {
    pub fn new(size: u32) -> Self {
        Self { size, pages: Vec::new() }
    }

    /// Room for a `w`×`h` image, in an existing page or a new one. `None` if it can never fit.
    pub fn allocate(
        &mut self,
        w: u32,
        h: u32,
        mut new_page: impl FnMut(u32) -> Page,
    ) -> Option<AtlasRect> {
        if w + PADDING > self.size || h + PADDING > self.size {
            return None;
        }
        for (i, page) in self.pages.iter_mut().enumerate() {
            if let Some((x, y)) = page.place(w, h, self.size) {
                return Some(rect(i, x, y, w, h));
            }
        }
        let mut page = new_page(self.size);
        let (x, y) = page.place(w, h, self.size).expect("an empty page holds anything that fits");
        self.pages.push(page);
        Some(rect(self.pages.len() - 1, x, y, w, h))
    }
}

fn rect(page: usize, x: u32, y: u32, w: u32, h: u32) -> AtlasRect {
    AtlasRect { page: page as u16, x: x as u16, y: y as u16, w: w as u16, h: h as u16 }
}

impl Page {
    pub fn new(texture: wgpu::Texture) -> Self {
        let view = texture.create_view(&Default::default());
        Self { texture, view, shelves: Vec::new(), next_y: 0 }
    }

    /// Shelf packing: the first shelf tall enough (but not wastefully tall) with room left.
    fn place(&mut self, w: u32, h: u32, size: u32) -> Option<(u32, u32)> {
        let (pw, ph) = (w + PADDING, h + PADDING);
        for shelf in &mut self.shelves {
            if ph <= shelf.height && ph * 2 > shelf.height && shelf.next_x + pw <= size {
                let x = shelf.next_x;
                shelf.next_x += pw;
                return Some((x, shelf.y));
            }
        }
        if self.next_y + ph > size {
            return None;
        }
        let shelf = Shelf { y: self.next_y, height: ph, next_x: pw };
        self.next_y += ph;
        let y = shelf.y;
        self.shelves.push(shelf);
        Some((0, y))
    }
}
