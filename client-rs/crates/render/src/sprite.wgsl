// One instanced quad per sprite. The quad is the unit square mapped to the screen by an affine
// transform; colour is `texel * mul + add * texel.a` (premultiplied alpha, like Flash's
// ColorTransform with the offset scaled by coverage), after an optional colour filter.

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

struct Instance {
    @location(0) axes: vec4<f32>,   // x axis (a, b), y axis (c, d)
    @location(1) origin: vec2<f32>, // where the quad's (0, 0) corner lands, in pixels
    @location(2) uv: vec4<f32>,     // u0, v0, u1, v1
    @location(3) mul: vec4<f32>,
    @location(4) add: vec4<f32>,
    @location(5) page: u32,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) mul: vec4<f32>,
    @location(2) add: vec4<f32>,
    @location(3) @interpolate(flat) page: u32,
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
    out.page = inst.page;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let t = page_texel(in.page, in.uv);
    // add.w picks a colour filter first: 1 grey (greyscaleFilterMatrix), 2 red (redFilterMatrix).
    let l = dot(t.rgb, vec3<f32>(0.3, 0.59, 0.11));
    var c = t.rgb;
    if (in.add.w > 1.5) {
        c = vec3<f32>(l, 0.0, 0.0);
    } else if (in.add.w > 0.5) {
        c = vec3<f32>(l, l, l);
    }
    let rgb = c * in.mul.rgb + in.add.rgb * t.a;
    let a = t.a * in.mul.a;
    return vec4<f32>(rgb * in.mul.a, a);
}
