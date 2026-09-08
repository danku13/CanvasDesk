// Миникарта (T13-B, SPEC §6.1): один текстурированный квад в правом нижнем
// углу окна. Позиция задаётся uniform-прямоугольником в ФИЗИЧЕСКИХ px;
// ортографическая проекция — та же идиома, что у thumbs.wgsl
// (screen / viewport * vec2(2, -2) + vec2(-1, 1)). Вершинного буфера нет:
// вершины разворачиваются из vertex_index, uv 0..1 → выборка текстуры RGBA8
// (CPU-растр T13-A). Квад рисуется последним проходом кадра.

struct QuadUniform {
    rect: vec4<f32>,      // [x0, y0, x1, y1] квада в физических px
    viewport: vec2<f32>,  // размер окна в физических px
    _pad0: f32,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> quad: QuadUniform;
@group(0) @binding(1) var minimap: texture_2d<f32>;
@group(0) @binding(2) var minimap_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Два треугольника квада (раскладка как в thumbs.wgsl)
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[vertex_index % 6u];
    let screen = mix(quad.rect.xy, quad.rect.zw, corner);

    var out: VertexOutput;
    out.clip = vec4<f32>(
        screen / quad.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.uv = corner;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Прямой вывод rgba: смешивание с кадром — blending-этап пайплайна
    // (straight alpha, полупрозрачный фон миникарты)
    return textureSample(minimap, minimap_sampler, in.uv);
}
