// Textured triangles (3D models): positions already on the screen, colour multiplied by `mul`.

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
    @location(2) mul: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) mul: vec4<f32>,
};

@vertex
fn vs_main(v: Vertex) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(v.pos.x / globals.screen.x * 2.0 - 1.0, 1.0 - v.pos.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = v.uv;
    out.mul = v.mul;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let t = textureSample(page, page_sampler, in.uv);
    return vec4<f32>(t.rgb * in.mul.rgb * in.mul.a, t.a * in.mul.a);
}
