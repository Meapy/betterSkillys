// One instanced quad per sprite. The quad is the unit square mapped to the screen by an affine
// transform; colour is `texel * mul + add * texel.a` (premultiplied alpha, like Flash's
// ColorTransform with the offset scaled by coverage).

struct Globals {
    screen: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var page: texture_2d<f32>;
@group(1) @binding(1) var page_sampler: sampler;

struct Instance {
    @location(0) axes: vec4<f32>,   // x axis (a, b), y axis (c, d)
    @location(1) origin: vec2<f32>, // where the quad's (0, 0) corner lands, in pixels
    @location(2) uv: vec4<f32>,     // u0, v0, u1, v1
    @location(3) mul: vec4<f32>,
    @location(4) add: vec4<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) mul: vec4<f32>,
    @location(2) add: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VertexOut {
    // Two triangles: (0,0) (1,0) (0,1), (0,1) (1,0) (1,1).
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let c = corners[vi];
    let p = inst.origin + inst.axes.xy * c.x + inst.axes.zw * c.y;
    var out: VertexOut;
    out.position = vec4<f32>(p.x / globals.screen.x * 2.0 - 1.0, 1.0 - p.y / globals.screen.y * 2.0, 0.0, 1.0);
    out.uv = mix(inst.uv.xy, inst.uv.zw, c);
    out.mul = inst.mul;
    out.add = inst.add;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let t = textureSample(page, page_sampler, in.uv);
    let rgb = t.rgb * in.mul.rgb + in.add.rgb * t.a;
    let a = t.a * in.mul.a;
    return vec4<f32>(rgb * in.mul.a, a);
}
