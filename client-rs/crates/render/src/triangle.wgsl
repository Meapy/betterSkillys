// Textured triangles (3D models, walls): positions already on the screen, colour multiplied by
// `mul`. `uv` runs 0..1 across the source rectangle `rect`, repeating if `wrap` is 1.

struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var page: texture_2d<f32>;
@group(1) @binding(1) var page_sampler: sampler;

struct Vertex {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) mul: vec4<f32>,
    @location(4) wrap: f32,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) mul: vec4<f32>,
    @location(2) rect: vec4<f32>,
    @location(3) wrap: f32,
};

@vertex
fn vs_main(v: Vertex) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(v.pos.x / globals.screen.x * 2.0 - 1.0, 1.0 - v.pos.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = v.uv;
    out.mul = v.mul;
    out.rect = v.rect;
    out.wrap = v.wrap;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Stay inside the rectangle: the next texel over belongs to another image.
    let local = select(clamp(in.uv, vec2<f32>(0.0), vec2<f32>(0.99999)), fract(in.uv), in.wrap > 0.5);
    let t = textureSample(page, page_sampler, in.rect.xy + local * in.rect.zw);
    return vec4<f32>(t.rgb * in.mul.rgb * in.mul.a, t.a * in.mul.a);
}
