// Тамбнейлы нод (T6): instanced quads с выборкой из атласа 2048² (SPEC §6.4).

struct CameraUniform {
    position: vec2<f32>,        // мировая точка в центре viewport
    viewport: vec2<f32>,        // размер viewport в физических px
    effective_zoom: f32,        // zoom * scale_factor (world -> физические px)
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,     // левый верх квада, world px
    @location(1) size: vec2<f32>,    // размер квада, world px
    @location(2) uv_min: vec2<f32>,  // uv левого верха в атласе
    @location(3) uv_max: vec2<f32>,  // uv правого низа в атласе
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn world_to_screen(world: vec2<f32>) -> vec2<f32> {
    return (world - camera.position) * camera.effective_zoom + camera.viewport * 0.5;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.vertex_index % 6u];
    let screen = world_to_screen(in.pos + corner * in.size);

    var out: VertexOutput;
    out.clip = vec4<f32>(
        screen / camera.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.uv = mix(in.uv_min, in.uv_max, corner);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(atlas, atlas_sampler, in.uv);
}
