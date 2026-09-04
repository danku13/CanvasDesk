// Карточки нод (T4): SDF скруглённого прямоугольника — заливка, мягкая тень,
// рамка выделения / brokenLink. Один instanced draw на все карточки кадра.

struct CameraUniform {
    position: vec2<f32>,        // мировая точка в центре viewport
    viewport: vec2<f32>,        // размер viewport в физических px
    effective_zoom: f32,        // zoom * scale_factor (world -> физические px)
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) fill: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) params: vec4<f32>, // x: radius (world), y: selected, z: broken
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) origin_screen: vec2<f32>, // левый верх карточки, физ. px
    @location(1) size_screen: vec2<f32>,   // размер карточки, физ. px
    @location(2) fill: vec4<f32>,
    @location(3) border: vec4<f32>,
    @location(4) params: vec4<f32>,
};

fn world_to_screen(world: vec2<f32>) -> vec2<f32> {
    return (world - camera.position) * camera.effective_zoom + camera.viewport * 0.5;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Единичный квадрат, 6 вершин на инстанс
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.vertex_index % 6u];

    // Тень выступает за край карточки — расширяем квадрат на запас
    let margin = 12.0 / camera.effective_zoom; // world px
    let world = in.pos + (corner * (in.size + 2.0 * margin) - margin);
    let screen = world_to_screen(world);

    var out: VertexOutput;
    out.clip = vec4<f32>(
        screen / camera.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.origin_screen = world_to_screen(in.pos);
    out.size_screen = in.size * camera.effective_zoom;
    out.fill = in.fill;
    out.border = in.border;
    out.params = in.params;
    return out;
}

// SDF скруглённого прямоугольника: p относительно центра, b — полуразмеры.
fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // @builtin(position) во фрагментном шейдере — координата фрагмента в физ. px
    let frag = in.clip.xy;
    let half_size = in.size_screen * 0.5;
    let center = in.origin_screen + half_size;
    let p = frag - center;
    let radius = in.params.x * camera.effective_zoom;

    // Тело карточки
    let sd = sd_rounded_box(p, half_size, radius);
    let aa = max(fwidth(sd), 1.0);
    let fill_alpha = 1.0 - smoothstep(-aa, aa, sd);

    // Мягкая тень: тот же SDF со смещением и размытием
    let shadow_offset = vec2<f32>(0.0, 4.0);
    let shadow_sd = sd_rounded_box(p - shadow_offset, half_size, radius) - 6.0;
    let shadow_alpha = (1.0 - smoothstep(-4.0, 6.0, shadow_sd)) * 0.35;

    // Рамка: выделение (2px) или broken (1px серая)
    var border_alpha = 0.0;
    if in.params.y > 0.5 {
        border_alpha = (1.0 - smoothstep(-aa, aa, abs(sd) - 1.5)) * 1.0;
    } else if in.params.z > 0.5 {
        border_alpha = (1.0 - smoothstep(-aa, aa, abs(sd) - 0.75)) * 1.0;
    }

    // Слои: тень -> заливка -> рамка
    var color = vec3<f32>(0.0);
    var alpha = shadow_alpha;
    color = mix(color, in.fill.rgb, fill_alpha);
    alpha = alpha + fill_alpha * (1.0 - alpha);
    color = mix(color, in.border.rgb, border_alpha * step(0.001, in.border.a));
    alpha = alpha + border_alpha * (1.0 - alpha);

    return vec4<f32>(color, alpha);
}
