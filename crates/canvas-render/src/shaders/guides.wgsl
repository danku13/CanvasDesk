// Направляющие магнитной раскладки (FR-038, T-038.3): instanced-проход
// тонких линий и ghost-рамки, рисуемый ПОВЕРХ нод (после карточек и
// снапшотов виджетов, до wheel-меню FR-022). Геометрия — world-прямоугольник
// инстанса (для линии вытянут на весь viewport, для ghost — snapped-bbox);
// форма (линия/рамка) — SDF во фрагменте. Толщина и период штриха — в
// физических px (через effective_zoom = zoom * scale_factor), поэтому вид
// константный при любом зуме; фаза штриха якорится к world-координатам —
// не «ползёт» при панорамировании. Стили (params.x):
//   0 — сплошная линия (guide-источник: совпадение с соседом);
//   1 — пунктирная линия (grid-источник: привязка к сетке);
//   2 — пунктирная рамка (ghost-предпросмотр snapped-позиции, п.2 v2).
// Производные (fwidth) считаются ВНЕ условных веток — требование
// uniform control flow WGSL: маски обоих стилей считаются всегда и
// смешиваются шаговой функцией по style.

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
    @location(0) rect: vec4<f32>,    // world [x0, y0, x1, y1]
    @location(1) color: vec4<f32>,   // RGBA (a уже включает интенсивность п.8)
    @location(2) params: vec4<f32>,  // [style, half_px, dash_period_px, 0]
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,   // позиция фрагмента, world
    @location(1) rect: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) params: vec4<f32>,
};

fn world_to_screen(world: vec2<f32>) -> vec2<f32> {
    return (world - camera.position) * camera.effective_zoom + camera.viewport * 0.5;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Единичный квад, 6 вершин на инстанс
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.vertex_index % 6u];

    // Запас под AA-край (в world), чтобы SDF имел место на сглаживание
    let margin = 1.5 / camera.effective_zoom;
    let world = vec2<f32>(
        mix(in.rect.x - margin, in.rect.z + margin, corner.x),
        mix(in.rect.y - margin, in.rect.w + margin, corner.y),
    );
    let screen = world_to_screen(world);

    var out: VertexOutput;
    out.clip = vec4<f32>(
        screen / camera.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.world = world;
    out.rect = in.rect;
    out.color = in.color;
    out.params = in.params;
    return out;
}

// Маска пунктира: фаза в физических px, период — px; ON на первой половине
// периода (duty 0.5). Граница штриха сглажена по производной фазы (кламп —
// чтобы разрыв fract на границе периода не размывал маску на весь период).
fn dash_mask(phase_px: f32, period_px: f32) -> f32 {
    let t = fract(phase_px / period_px);
    let aa = clamp(fwidth(t), 0.02, 0.3);
    return 1.0 - smoothstep(0.5 - aa, 0.5 + aa, t);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let zoom = camera.effective_zoom;
    let center = vec2<f32>(
        (in.rect.x + in.rect.z) * 0.5,
        (in.rect.y + in.rect.w) * 0.5,
    );
    let half_size = vec2<f32>(in.rect.z - in.rect.x, in.rect.w - in.rect.y) * 0.5;
    let local = in.world - center;

    let style = in.params.x;
    let half_px = in.params.y;
    let period = max(in.params.z, 1.0);

    // Линия (стили 0/1): тонкая ось — направление толщины, длинная — фаза
    let vertical = half_size.x <= half_size.y;
    let across = select(local.y, local.x, vertical) * zoom;
    let along = select(local.x, local.y, vertical) * zoom;
    let aa_line = max(fwidth(across), 0.5);
    let line = 1.0 - smoothstep(half_px - aa_line, half_px + aa_line, abs(across));
    let line_mask = line
        * mix(1.0, dash_mask(along, period), step(0.5, style))
        * step(style, 1.5);

    // Рамка ghost (стиль 2): |sdBox| в px; фаза штриха — по оси, чья граница
    // ближе (на вертикальных рёбрах — y, на горизонтальных — x)
    let q = abs(local) - half_size;
    let sd = (length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0)) * zoom;
    let aa_border = max(fwidth(sd), 0.5);
    let border = 1.0 - smoothstep(half_px - aa_border, half_px + aa_border, abs(sd));
    let edge_x = half_size.x - abs(local.x);
    let edge_y = half_size.y - abs(local.y);
    let phase = select(local.x, local.y, edge_x < edge_y) * zoom;
    let border_mask = border * dash_mask(phase, period) * step(1.5, style);

    let mask = max(line_mask, border_mask);
    let alpha = mask * in.color.a;
    if alpha <= 0.004 {
        discard;
    }
    return vec4<f32>(in.color.rgb, alpha);
}
