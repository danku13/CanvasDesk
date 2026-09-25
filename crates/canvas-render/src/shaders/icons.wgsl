// FR-ICONS: SVG-иконки UI (screen-space instanced quads с tint).
//
// Атлас собирается один раз при init из растеризованных байт (icon_data.rs):
// 4 набора × 13 иконок × 32×32 px (bootstrap 24×24 дополнен до 32×32).
//
// Shader — screen-space (без camera uniform): pos/size уже в физических px.
// Tint — цвет слота `icon` темы (умножается на RGB силуэта; alpha — из атласа).

struct ViewportUniform {
    viewport: vec2<f32>,  // физические px
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,      // левый верх, физ. px
    @location(1) size: vec2<f32>,     // размер, физ. px
    @location(2) uv_min: vec2<f32>,   // uv левого верха в атласе
    @location(3) uv_max: vec2<f32>,   // uv правого низа в атласе
    @location(4) tint: vec4<f32>,     // RGBA tint (премультипликации НЕТ)
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.vertex_index % 6u];
    let screen = in.pos + corner * in.size;

    var out: VertexOutput;
    // Screen px → clip space: Y инвертируется (screen Y сверху вниз).
    out.clip = vec4<f32>(
        screen / viewport.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.uv = mix(in.uv_min, in.uv_max, corner);
    out.tint = in.tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(atlas, atlas_sampler, in.uv);
    // Атлас — белый силуэт (rgb=1) на прозрачном фоне. Tint умножается на
    // alpha силуэта — получаем монохромную иконку цвета tint.
    return vec4<f32>(in.tint.rgb, tex.a * in.tint.a);
}
