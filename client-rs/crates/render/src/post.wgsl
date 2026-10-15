// The world drawn through a filter, as the AIR client's Stage3D post effects do: grey while
// paused, nearly black while blind, blurred (swaying) while drunk. One triangle covers the screen.

struct Post {
    // 1 grey, 2 blind, 3 blur.
    mode: f32,
    // The blur's radius, in pixels.
    radius: f32,
    texel: vec2<f32>,
};

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> post: Post;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    let x = f32((vi << 1u) & 2u);
    let y = f32(vi & 2u);
    var out: VertexOut;
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if (post.mode > 2.5) {
        // A 7 × 7 spread of samples over the radius.
        var sum = vec4<f32>(0.0);
        for (var i = -3; i <= 3; i++) {
            for (var j = -3; j <= 3; j++) {
                let d = vec2<f32>(f32(i), f32(j)) / 3.0 * post.radius * post.texel;
                sum += textureSampleLevel(scene, scene_sampler, in.uv + d, 0.0);
            }
        }
        return vec4<f32>((sum / 49.0).rgb, 1.0);
    }
    let c = textureSampleLevel(scene, scene_sampler, in.uv, 0.0);
    if (post.mode > 1.5) {
        // Map.BLIND_FILTER: each channel 0.05 × (r + g + b).
        let v = 0.05 * (c.r + c.g + c.b);
        return vec4<f32>(v, v, v, 1.0);
    }
    // MoreColorUtil.greyscaleFilterMatrix.
    let l = dot(c.rgb, vec3<f32>(0.3, 0.59, 0.11));
    return vec4<f32>(l, l, l, 1.0);
}
