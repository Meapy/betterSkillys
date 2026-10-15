// Textured triangles (3D models, walls): positions already on the screen, colour multiplied by
// `mul`. `uv` runs 0..1 across the source rectangle `rect`, repeating if `wrap` is 1.

struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var p0: texture_2d<f32>;
@group(1) @binding(1) var p1: texture_2d<f32>;
@group(1) @binding(2) var p2: texture_2d<f32>;
@group(1) @binding(3) var p3: texture_2d<f32>;
@group(1) @binding(4) var p4: texture_2d<f32>;
@group(1) @binding(5) var p5: texture_2d<f32>;
@group(1) @binding(6) var p6: texture_2d<f32>;
@group(1) @binding(7) var p7: texture_2d<f32>;
@group(1) @binding(8) var page_sampler: sampler;

// A texel of page `page` of the bound group (nearest, no mipmaps).
fn page_texel(page: u32, uv: vec2<f32>) -> vec4<f32> {
    switch page {
        case 0u: { return textureSampleLevel(p0, page_sampler, uv, 0.0); }
        case 1u: { return textureSampleLevel(p1, page_sampler, uv, 0.0); }
        case 2u: { return textureSampleLevel(p2, page_sampler, uv, 0.0); }
        case 3u: { return textureSampleLevel(p3, page_sampler, uv, 0.0); }
        case 4u: { return textureSampleLevel(p4, page_sampler, uv, 0.0); }
        case 5u: { return textureSampleLevel(p5, page_sampler, uv, 0.0); }
        case 6u: { return textureSampleLevel(p6, page_sampler, uv, 0.0); }
        default: { return textureSampleLevel(p7, page_sampler, uv, 0.0); }
    }
}

struct Vertex {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) mul: vec4<f32>,
    @location(4) wrap: f32,
    @location(5) page: u32,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) mul: vec4<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) wrap: f32,
    @location(4) @interpolate(flat) page: u32,
};

@vertex
fn vs_main(v: Vertex) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(v.pos.x / globals.screen.x * 2.0 - 1.0, 1.0 - v.pos.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = v.uv;
    out.mul = v.mul;
    out.rect = v.rect;
    out.wrap = v.wrap;
    out.page = v.page;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Stay inside the rectangle: the next texel over belongs to another image.
    let local = select(clamp(in.uv, vec2<f32>(0.0), vec2<f32>(0.99999)), fract(in.uv), in.wrap > 0.5);
    let t = page_texel(in.page, in.rect.xy + local * in.rect.zw);
    return vec4<f32>(t.rgb * in.mul.rgb * in.mul.a, t.a * in.mul.a);
}
