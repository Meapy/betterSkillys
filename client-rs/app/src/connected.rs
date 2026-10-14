//! Walls that join their neighbours (`ConnectedObject.as`, `ConnectedWall.as`, `CaveWall.as`).
//! The server sends which sides connect (the CONNECT stat); that picks one of six shapes and a
//! quarter turn. Faces are built in the object's own space, the tile centred on 0.
//!
//! The AIR client draws ConnectedWall with Stage3D vertex buffers (GPU rendering is its
//! default): every face is a quad whose corners carry the texture's corners, and the small
//! corner pieces ("bits") exist only for the L shape. CaveWall faces are drawn the software way,
//! polygons with the texture laid on by their first, second and last corner.

pub type V3 = [f64; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Dot,
    ShortLine,
    L,
    Line,
    T,
    Cross,
}

impl Shape {
    /// ConnectedWall's own top texture for the shape, if the XML gives one.
    pub fn top_texture_tag(self) -> &'static str {
        match self {
            Shape::Dot => "DotTexture",
            Shape::ShortLine => "ShortLineTexture",
            Shape::L => "LTexture",
            Shape::Line => "LineTexture",
            Shape::T => "TTexture",
            Shape::Cross => "CrossTexture",
        }
    }
}

/// `getConnectedResults`: the shape and quarter turns for a CONNECT stat. One byte per side
/// (north, east, south, west from the top byte), 1 where it connects.
pub fn shape(connect: i32) -> Option<(Shape, i32)> {
    let pattern = connect as u32 & 0x0F0F_0F0F;
    let shapes = [
        (0x0202_0202u32, Shape::Dot),
        (0x0102_0202, Shape::ShortLine),
        (0x0101_0202, Shape::L),
        (0x0102_0102, Shape::Line),
        (0x0101_0201, Shape::T),
        (0x0101_0101, Shape::Cross),
    ];
    for (mut value, shape) in shapes {
        // initHelper: the first rotation to produce a pattern keeps it.
        for rotation in 0..4 {
            if value == pattern {
                return Some((shape, rotation));
            }
            value = value.rotate_right(8);
        }
    }
    None
}

/// One face, in the object's space before its turn.
#[derive(Debug, Clone)]
pub struct Face {
    pub verts: Vec<V3>,
    /// Texture coordinates per vertex (unused when `basis_uv` is set).
    pub uvs: Vec<[f64; 2]>,
    pub tris: Vec<[usize; 3]>,
    /// The first, second and last corners of the face as built: they decide which way it faces
    /// (back faces aren't drawn), its shade, and which neighbour it looks at.
    pub basis: [V3; 3],
    /// Software-drawn faces: the texture coordinates at the basis corners, the rest following.
    pub basis_uv: Option<[[f64; 2]; 3]>,
    /// `normalL_`: the neighbour the face looks at, when not its own normal.
    pub normal: Option<V3>,
    /// Uses the shape's top texture.
    pub top: bool,
}

const N0: V3 = [-1.0, -1.0, 0.0];
const N2: V3 = [1.0, -1.0, 0.0];
const N3: V3 = [1.0, 0.0, 0.0];
const N4: V3 = [1.0, 1.0, 0.0];
const N6: V3 = [-1.0, 1.0, 0.0];
const N7: V3 = [-1.0, 0.0, 0.0];

/// The affine map taking texture coordinates `uv[i]` to the points `p[i]`.
fn affine(uv: [[f64; 2]; 3], p: [V3; 3]) -> impl Fn(f64, f64) -> V3 {
    let (e1, e2) = ([uv[1][0] - uv[0][0], uv[1][1] - uv[0][1]], [uv[2][0] - uv[0][0], uv[2][1] - uv[0][1]]);
    let det = e1[0] * e2[1] - e1[1] * e2[0];
    move |u, v| {
        let (du, dv) = (u - uv[0][0], v - uv[0][1]);
        let (s, t) = if det == 0.0 { (0.0, 0.0) } else { ((du * e2[1] - dv * e2[0]) / det, (e1[0] * dv - e1[1] * du) / det) };
        std::array::from_fn(|i| p[0][i] + s * (p[1][i] - p[0][i]) + t * (p[2][i] - p[0][i]))
    }
}

const CORNER_UVS: [[f64; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
/// Stage3D's index buffer over a 4-corner vertex buffer.
const QUAD_TRIS: [[usize; 3]; 2] = [[0, 1, 2], [2, 1, 3]];

/// `<BaseIndent>`, `<TopIndent>` and `<Height>` of a ConnectedWall.
#[derive(Debug, Clone, Copy)]
pub struct Indents {
    pub base: f64,
    pub top: f64,
    pub height: f64,
}

impl Indents {
    pub fn from_xml(xml: &bsk_data::xml::Element) -> Self {
        Indents {
            base: if xml.has("BaseIndent") { 0.5 - xml.number("BaseIndent", 0.0) } else { 0.5 },
            top: if xml.has("TopIndent") { 0.5 - xml.number("TopIndent", 0.0) } else { 0.25 },
            height: xml.number("Height", 1.0),
        }
    }
}

struct Walls {
    b: f64,
    t: f64,
    faces: Vec<Face>,
}

impl Walls {
    /// `addQuad`: top edge v0 to v1, bottom v3 to v2; an indented side slants in.
    fn quad(&mut self, v0: V3, v1: V3, _v2: V3, v3: V3, left: bool, right: bool) {
        let d = self.b - self.t;
        let n = if left { -d / (1.0 - d - if right { d } else { 0.0 }) } else { 0.0 };
        let n1 = if left { n } else { 0.0 };
        let mut n2 = if right { -n } else { 0.0 };
        if n1 == 0.0 && n2 == 0.0 && right && v3[0] == -0.5 {
            n2 = 0.34;
        }
        let p = affine([[0.0, 0.0], [1.0, 0.0], [n, 1.0]], [v0, v1, v3]);
        self.faces.push(Face {
            verts: vec![p(0.0, 0.0), p(1.0, 0.0), p(n1, 1.0), p(1.0 + n2, 1.0)],
            uvs: CORNER_UVS.to_vec(),
            tris: QUAD_TRIS.to_vec(),
            basis: [v0, v1, v3],
            basis_uv: None,
            normal: None,
            top: false,
        });
    }

    /// `addBit` with forceStage3D: the corner pieces of the L.
    fn bit(&mut self, v0: V3, v1: V3, v3: V3, normal: V3, indent: bool, lean_left: bool) {
        let uv = if indent {
            [[-0.5 + self.t, 0.0], [0.0, 0.0], [-0.5 + self.b, 1.0]]
        } else {
            [[1.0, 0.0], [1.5 - self.t, 0.0], [1.0, 1.0]]
        };
        let p = affine(uv, [v0, v1, v3]);
        let frame: [[f64; 2]; 4] = if lean_left {
            [[-0.25, 0.0], [0.0, 0.0], [-0.25, 1.0], [0.0, 1.0]]
        } else {
            [[1.0, 0.0], [1.25, 0.0], [1.0, 1.0], [1.25, 1.0]]
        };
        self.faces.push(Face {
            verts: frame.iter().map(|f| p(f[0], f[1])).collect(),
            uvs: CORNER_UVS.to_vec(),
            tris: QUAD_TRIS.to_vec(),
            basis: [v0, v1, v3],
            basis_uv: None,
            normal: Some(normal),
            top: false,
        });
    }

    /// `addTop`: `uvs` are the texture coordinates of the first, second and last corner.
    fn top(&mut self, verts: &[V3], uvs: [f64; 6]) {
        let last = verts.len() - 1;
        let uvt = |i: usize| -> [f64; 2] {
            match i {
                0 => [uvs[0], uvs[1]],
                1 => [uvs[2], uvs[3]],
                i if i == last => [uvs[4], uvs[5]],
                _ => [0.0, 0.0],
            }
        };
        let basis = [verts[0], verts[1], verts[last]];
        let p = affine([uvt(0), uvt(1), uvt(last)], basis);
        // Vertex buffer entries place each corner by its own position, read as texture space.
        let at = |i: usize| p(verts[i][0] + 0.5, verts[i][1] + 0.5);
        let tex = |t: usize| [uvt(if t != 2 { t } else { t - 1 })[0], uvt(if t != 2 { t } else { t + 1 })[1]];
        let mut face = |order: [usize; 4], tex_of: [usize; 4]| {
            self.faces.push(Face {
                verts: order.iter().map(|&i| at(i)).collect(),
                uvs: tex_of.iter().map(|&t| tex(t)).collect(),
                tris: QUAD_TRIS.to_vec(),
                basis,
                basis_uv: None,
                normal: None,
                top: true,
            });
        };
        match verts.len() {
            4 => face([0, 1, 3, 2], [0, 1, 3, 2]),
            6 => {
                face([0, 1, 5, 2], [0, 1, 5, 2]);
                face([2, 3, 5, 4], [5, 0, 2, 1]);
            }
            _ => {
                // No vertex buffer: the whole texture on its square.
                self.faces.push(Face {
                    verts: CORNER_UVS.iter().map(|f| p(f[0], f[1])).collect(),
                    uvs: CORNER_UVS.to_vec(),
                    tris: QUAD_TRIS.to_vec(),
                    basis,
                    basis_uv: None,
                    normal: None,
                    top: true,
                });
            }
        }
    }
}

/// `ConnectedWall.build*`.
pub fn connected_wall(shape: Shape, ind: Indents) -> Vec<Face> {
    let (b, t, h) = (ind.base, ind.top, ind.height);
    let mut w = Walls { b, t, faces: Vec::new() };
    let v = |x: f64, y: f64, z: f64| -> V3 { [x, y, z] };
    match shape {
        Shape::Dot => {
            let (v0, v1, v2, v3) = (v(-b, -b, 0.0), v(b, -b, 0.0), v(b, b, 0.0), v(-b, b, 0.0));
            let (v4, v5, v6, v7) = (v(-t, -t, h), v(t, -t, h), v(t, t, h), v(-t, t, h));
            w.quad(v5, v4, v0, v1, true, true);
            w.quad(v6, v5, v1, v2, true, true);
            w.quad(v4, v7, v3, v0, true, true);
            w.quad(v7, v6, v2, v3, true, true);
            w.top(&[v4, v5, v6, v7], [0.25, 0.25, 0.75, 0.25, 0.25, 0.75]);
        }
        Shape::ShortLine => {
            let (v0, v1, v2, v3) = (v(-b, -0.5, 0.0), v(b, -0.5, 0.0), v(b, b, 0.0), v(-b, b, 0.0));
            let (v4, v5, v6, v7) = (v(-t, -0.5, h), v(t, -0.5, h), v(t, t, h), v(-t, t, h));
            w.quad(v6, v5, v1, v2, true, false);
            w.quad(v4, v7, v3, v0, false, true);
            w.quad(v7, v6, v2, v3, true, true);
            w.top(&[v4, v5, v6, v7], [0.25, 0.0, 0.75, 0.0, 0.25, 0.75]);
        }
        Shape::L => {
            let (v0, v1, v2) = (v(-b, -0.5, 0.0), v(b, -0.5, 0.0), v(b, -b, 0.0));
            let (v3, v4, v5) = (v(0.5, -b, 0.0), v(0.5, b, 0.0), v(-b, b, 0.0));
            let (v6, v7, v8) = (v(-t, -0.5, h), v(t, -0.5, h), v(t, -t, h));
            let (v9, va, vb) = (v(0.5, -t, h), v(0.5, t, h), v(-t, t, h));
            w.bit(v8, v7, v2, N2, true, true);
            w.bit(v9, v8, v3, N2, false, false);
            w.quad(vb, va, v4, v5, true, false);
            w.quad(v6, vb, v5, v0, false, true);
            w.top(&[v6, v7, v8, v9, va, vb], [0.25, 0.0, 0.75, 0.0, 0.25, 0.75]);
            let _ = v1;
        }
        Shape::Line => {
            let (v0, v1, v2, v3) = (v(-b, -0.5, 0.0), v(b, -0.5, 0.0), v(b, 0.5, 0.0), v(-b, 0.5, 0.0));
            let (v4, v5, v6, v7) = (v(-t, -0.5, h), v(t, -0.5, h), v(t, 0.5, h), v(-t, 0.5, h));
            w.quad(v6, v5, v1, v2, false, false);
            w.quad(v4, v7, v3, v0, false, false);
            w.top(&[v4, v5, v6, v7], [0.25, 0.0, 0.75, 0.0, 0.25, 1.0]);
        }
        Shape::T => {
            // The corner bits aren't drawn on the GPU path; the far wall and the top are.
            let (v4, v5) = (v(0.5, b, 0.0), v(-0.5, b, 0.0));
            let (v8, v9, va, vb) = (v(-t, -0.5, h), v(t, -0.5, h), v(t, -t, h), v(0.5, -t, h));
            let (vc, vd, ve, vf) = (v(0.5, t, h), v(-0.5, t, h), v(-0.5, -t, h), v(-t, -t, h));
            w.quad(vd, vc, v4, v5, false, false);
            w.top(&[v8, v9, va, vb, vc, vd, ve, vf], [0.25, 0.0, 0.75, 0.0, 0.25, 0.25]);
        }
        Shape::Cross => {
            let top = [
                v(-t, -0.5, h),
                v(t, -0.5, h),
                v(t, -t, h),
                v(0.5, -t, h),
                v(0.5, t, h),
                v(t, t, h),
                v(t, 0.5, h),
                v(-t, 0.5, h),
                v(-t, t, h),
                v(-0.5, t, h),
                v(-0.5, -t, h),
                v(-t, -t, h),
            ];
            w.top(&top, [0.25, 0.0, 0.75, 0.0, 0.25, 0.25]);
        }
    }
    w.faces
}

/// `CaveWall.build*`: rough rock, its corners jittered by `random` (Math.random there) and by
/// the tile position, so neighbouring pieces meet.
pub fn cave_wall(shape: Shape, rotation: i32, x: i32, y: i32, random: &mut impl FnMut() -> f64) -> Vec<Face> {
    let gv = |side: i32, id: i32| cave_vertex(side, id, rotation, x, y);
    let mut faces = Vec::new();
    let mut r = || random();
    let mut add = |normal: Option<V3>, verts: &[V3], r: &mut dyn FnMut() -> f64| cave_face(&mut faces, normal, verts, r);
    match shape {
        Shape::Dot => {
            let v0 = [-0.25 - r() * 0.25, -0.25 - r() * 0.25, 0.0];
            let v1 = [0.25 + r() * 0.25, -0.25 - r() * 0.25, 0.0];
            let v2 = [0.25 + r() * 0.25, 0.25 + r() * 0.25, 0.0];
            let v3 = [-0.25 - r() * 0.25, 0.25 + r() * 0.25, 0.0];
            let v4 = [-0.25 + r() * 0.5, -0.25 + r() * 0.5, 1.0];
            add(None, &[v4, v0, v1], &mut r);
            add(None, &[v4, v1, v2], &mut r);
            add(None, &[v4, v2, v3], &mut r);
            add(None, &[v4, v3, v0], &mut r);
        }
        Shape::ShortLine => {
            let (v0, v1) = (gv(0, 0), gv(0, 3));
            let v2 = [0.25 + r() * 0.25, 0.25 + r() * 0.25, 0.0];
            let v3 = [-0.25 - r() * 0.25, 0.25 + r() * 0.25, 0.0];
            let (v4, v5) = (gv(0, 1), gv(0, 2));
            let v6 = [r() * 0.25, r() * 0.25, 0.5];
            let v7 = [r() * -0.25, r() * 0.25, 0.5];
            add(None, &[v4, v7, v3, v0], &mut r);
            add(None, &[v7, v6, v2, v3], &mut r);
            add(None, &[v6, v5, v1, v2], &mut r);
            add(None, &[v4, v5, v6, v7], &mut r);
        }
        Shape::L => {
            let (v0, v1, v2, v3) = (gv(0, 0), gv(0, 3), gv(1, 0), gv(1, 3));
            let v4 = [-r() * 0.25, r() * 0.25, 0.0];
            let (v5, v6, v7, v8) = (gv(0, 1), gv(0, 2), gv(1, 1), gv(1, 2));
            let v9 = [r() * 0.25, -r() * 0.25, 1.0];
            add(None, &[v5, v9, v4, v0], &mut r);
            add(None, &[v9, v8, v3, v4], &mut r);
            add(Some(N2), &[v7, v6, v1, v2], &mut r);
            add(None, &[v5, v6, v7, v8, v9], &mut r);
        }
        Shape::Line => {
            let (v0, v1, v2, v3) = (gv(0, 0), gv(0, 3), gv(2, 3), gv(2, 0));
            let (v4, v5, v6, v7) = (gv(0, 1), gv(0, 2), gv(2, 2), gv(2, 1));
            add(Some(N7), &[v4, v7, v3, v0], &mut r);
            add(Some(N3), &[v6, v5, v1, v2], &mut r);
            add(None, &[v4, v5, v6, v7], &mut r);
        }
        Shape::T => {
            let (v0, v1, v2, v3, v4, v5) = (gv(0, 0), gv(0, 3), gv(1, 0), gv(1, 3), gv(3, 3), gv(3, 0));
            let (v6, v7, v8, v9, va, vb) = (gv(0, 1), gv(0, 2), gv(1, 1), gv(1, 2), gv(3, 2), gv(3, 1));
            add(Some(N2), &[v8, v7, v1, v2], &mut r);
            add(None, &[va, v9, v3, v4], &mut r);
            add(Some(N0), &[v6, vb, v5, v0], &mut r);
            add(None, &[v6, v7, v8, v9, va, vb], &mut r);
        }
        Shape::Cross => {
            let (v0, v1, v2, v3) = (gv(0, 0), gv(0, 3), gv(1, 0), gv(1, 3));
            let (v4, v5, v6, v7) = (gv(2, 3), gv(2, 0), gv(3, 3), gv(3, 0));
            let (v8, v9, va, vb) = (gv(0, 1), gv(0, 2), gv(1, 1), gv(1, 2));
            let (vc, vd, ve, vf) = (gv(2, 2), gv(2, 1), gv(3, 2), gv(3, 1));
            add(Some(N2), &[va, v9, v1, v2], &mut r);
            add(Some(N4), &[vc, vb, v3, v4], &mut r);
            add(Some(N6), &[ve, vd, v5, v6], &mut r);
            add(Some(N0), &[v8, vf, v7, v0], &mut r);
            add(None, &[v8, v9, va, vb, vc, vd, ve, vf], &mut r);
        }
    }
    faces
}

/// `CaveWall.getVertex`: a corner on a side of the tile, pushed in or out by a hash of the tile
/// it shares with the neighbour on that side.
fn cave_vertex(side: i32, id: i32, rotation: i32, mut x: i32, mut y: i32) -> V3 {
    match (side + rotation) % 4 {
        1 => x += 1,
        2 => y += 1,
        _ => {}
    }
    let r = match id {
        0 | 3 => 15 + (x.wrapping_mul(1259) ^ y.wrapping_mul(2957)) % 35,
        _ => 3 + (x.wrapping_mul(2179) ^ y.wrapping_mul(1237)) % 35,
    } as f64;
    let (v, h) = match id {
        0 => (-r / 100.0, 0.0),
        1 => (-r / 100.0, 1.0),
        2 => (r / 100.0, 1.0),
        _ => (r / 100.0, 0.0),
    };
    match side {
        0 => [v, -0.5, h],
        1 => [0.5, v, h],
        2 => [v, 0.5, h],
        _ => [-0.5, v, h],
    }
}

/// `CaveWall.faceHelper`: quads split into two triangles one way or the other at random.
fn cave_face(faces: &mut Vec<Face>, normal: Option<V3>, verts: &[V3], random: &mut dyn FnMut() -> f64) {
    let uvs: &[[f64; 2]] = match verts.len() {
        3 => &[[0.0, 0.0], [0.0, 1.0], [1.0, 1.0]],
        4 => &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        5 => &[[0.2, 0.0], [0.8, 0.0], [1.0, 0.2], [1.0, 0.8], [0.0, 0.8]],
        6 => &[[0.0, 0.0], [0.2, 0.0], [1.0, 0.2], [1.0, 0.8], [0.0, 0.8], [0.0, 0.2]],
        _ => &[[0.0, 0.0], [0.2, 0.0], [1.0, 0.2], [1.0, 0.8], [0.8, 1.0], [0.2, 1.0], [0.0, 0.8], [0.0, 0.2]],
    };
    let mut polygon = |ix: &[usize]| {
        let last = ix[ix.len() - 1];
        faces.push(Face {
            verts: ix.iter().map(|&i| verts[i]).collect(),
            uvs: Vec::new(),
            tris: (1..ix.len() - 1).map(|k| [0, k, k + 1]).collect(),
            basis: [verts[ix[0]], verts[ix[1]], verts[last]],
            basis_uv: Some([uvs[ix[0]], uvs[ix[1]], uvs[last]]),
            normal,
            top: false,
        });
    };
    if verts.len() == 4 {
        if random() < 0.5 {
            polygon(&[0, 1, 3]);
            polygon(&[1, 2, 3]);
        } else {
            polygon(&[0, 2, 3]);
            polygon(&[0, 1, 2]);
        }
    } else {
        let all: Vec<usize> = (0..verts.len()).collect();
        polygon(&all);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_and_turns() {
        assert_eq!(shape(0x0202_0202), Some((Shape::Dot, 0)));
        // North only, then east only: a short line turned a quarter.
        assert_eq!(shape(0x0102_0202), Some((Shape::ShortLine, 0)));
        assert_eq!(shape(0x0201_0202), Some((Shape::ShortLine, 1)));
        // East and west: the line turned once; north and south is the same line unturned.
        assert_eq!(shape(0x0201_0201), Some((Shape::Line, 1)));
        assert_eq!(shape(0x0102_0102), Some((Shape::Line, 0)));
        assert_eq!(shape(0x0101_0101), Some((Shape::Cross, 0)));
        assert_eq!(shape(0), None);
    }

    #[test]
    fn dot_wall_slants_in() {
        // The default dot: a 1×1 base, a 0.5×0.5 top at height 1. The north face's bottom
        // corners are the base corners, its top corners the top's.
        let faces = connected_wall(Shape::Dot, Indents { base: 0.5, top: 0.25, height: 1.0 });
        assert_eq!(faces.len(), 5);
        let north = &faces[0];
        let near = |a: V3, b: V3| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(near(north.verts[0], [0.25, -0.25, 1.0]));
        assert!(near(north.verts[1], [-0.25, -0.25, 1.0]));
        assert!(near(north.verts[2], [0.5, -0.5, 0.0]), "{:?}", north.verts[2]);
        assert!(near(north.verts[3], [-0.5, -0.5, 0.0]), "{:?}", north.verts[3]);
        // The top keeps its corners and shows the middle of its texture.
        let top = &faces[4];
        assert!(near(top.verts[0], [-0.25, -0.25, 1.0]));
        assert_eq!(top.uvs[0], [0.25, 0.25]);
        assert_eq!(top.uvs[3], [0.75, 0.75]);
    }
}
