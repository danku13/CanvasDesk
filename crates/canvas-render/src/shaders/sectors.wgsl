// Donut/pie-сектора (FR-022, рестайл 2026-09-16): instanced кольцевые сектора
// с центральным отверстием — wheel-меню шаблонов в стиле circular-menu
// (https://www.npmjs.com/package/circular-menu — ориентир по форме, код свой).
// Форма задаётся SDF annular-wedge (кольцевой клин): радиальные границы r0/r1
// + радиальные отрезки по углам a0/a1. Нулевой угол — 12 часов, направление —
// по часовой (экранные координаты, y вниз: угол atan2(dy, dx), позиция
// (cos, sin)). Сглаживание — smoothstep по fwidth(SDF) в физических px.

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
    @location(0) center: vec2<f32>,  // центр окружности, world
    @location(1) radii: vec2<f32>,   // x: r0 (внутренний), y: r1 (внешний), world
    @location(2) angles: vec2<f32>,  // x: a0, y: a1, радианы (12 часов, по часовой)
    @location(3) fill: vec4<f32>,    // RGBA
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,   // позиция фрагмента, world (для полярного SDF)
    @location(1) center: vec2<f32>,
    @location(2) radii: vec2<f32>,
    @location(3) angles: vec2<f32>,
    @location(4) fill: vec4<f32>,
};

fn world_to_screen(world: vec2<f32>) -> vec2<f32> {
    return (world - camera.position) * camera.effective_zoom + camera.viewport * 0.5;
}

// Точки дуг r0/r1 под углом a: (min_x, min_y, max_x, max_y) относительно центра.
fn arc_points(a: f32, r0: f32, r1: f32) -> vec4<f32> {
    let p0 = vec2<f32>(r0 * cos(a), r0 * sin(a));
    let p1 = vec2<f32>(r1 * cos(a), r1 * sin(a));
    return vec4<f32>(min(p0, p1), max(p0, p1));
}

fn merge_bbox(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(min(a.xy, b.xy), max(a.zw, b.zw));
}

// Bounding-квад сектора (world): min/max по точкам дуг r0/r1 на концах углов
// a0/a1 плюс точкам в кардинальных направлениях (0, π/2, π, 3π/2 — там
// экстремумы cos/sin), попадающим строго внутрь дугового интервала. Для
// полного круга (span ≈ 2π) кардиналы входят все. Результат: (min_x, min_y,
// max_x, max_y) вокруг center.
fn sector_bbox(center: vec2<f32>, r0: f32, r1: f32, a0: f32, a1: f32) -> vec4<f32> {
    let tau = 6.283185307179586;
    let span = a1 - a0;
    let full = span >= tau - 0.001;
    // Концы дуги — всегда; кардиналы — только если строго внутри дуги
    // (развёрнуто вручную: динамическая индексация массива в WGSL запрещена).
    var acc = arc_points(a0, r0, r1);
    acc = merge_bbox(acc, arc_points(a1, r0, r1));
    if full || (0.0 > a0 && 0.0 < a1) {
        acc = merge_bbox(acc, arc_points(0.0, r0, r1));
    }
    if full || (1.5707963267948966 > a0 && 1.5707963267948966 < a1) {
        acc = merge_bbox(acc, arc_points(1.5707963267948966, r0, r1));
    }
    if full || (3.141592653589793 > a0 && 3.141592653589793 < a1) {
        acc = merge_bbox(acc, arc_points(3.141592653589793, r0, r1));
    }
    if full || (4.71238898038469 > a0 && 4.71238898038469 < a1) {
        acc = merge_bbox(acc, arc_points(4.71238898038469, r0, r1));
    }
    return vec4<f32>(center + acc.xy, center + acc.zw);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // Единичный квад, 6 вершин на инстанс
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0),
    );
    let corner = corners[in.vertex_index % 6u];

    // Запас под AA-край; квад в world, чтобы SDF во фрагменте был линеен
    let margin = 2.0 / camera.effective_zoom;
    let bbox = sector_bbox(in.center, in.radii.x, in.radii.y, in.angles.x, in.angles.y);
    let world = vec2<f32>(
        mix(bbox.x - margin, bbox.z + margin, corner.x),
        mix(bbox.y - margin, bbox.w + margin, corner.y),
    );
    let screen = world_to_screen(world);

    var out: VertexOutput;
    out.clip = vec4<f32>(
        screen / camera.viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0),
        0.0,
        1.0,
    );
    out.world = world;
    out.center = in.center;
    out.radii = in.radii;
    out.angles = in.angles;
    out.fill = in.fill;
    return out;
}

// Знаковое угловое расстояние от theta до дугового интервала [a0, a1]
// (радианы, окружная метрика): отрицательное — theta строго внутри дуги
// (по модулю — расстояние до ближайшего края), 0 — на крае, положительное —
// снаружи. Зеркало чистой функции `sectors::angle_gap` (Rust) — единый
// источник формулы: hit-test wheel-меню (template_ui) использует Rust-
// версию, шейдер — эту; инвариант WYSIWYG тестами обеих сторон.
fn angle_gap(theta: f32, a0: f32, a1: f32) -> f32 {
    let tau = 6.283185307179586;
    let span = a1 - a0;
    if span >= tau - 0.001 {
        return -1.0;
    }
    // d — theta, приведённое в [a0, a0 + tau): окружная позиция от начала дуги
    var d = theta - a0;
    d = d - tau * floor(d / tau);
    if d <= span {
        return -min(d, span - d);
    }
    return min(d - span, tau - d);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Полярные координаты фрагмента в физических px: при равномерном зуме
    // масштаб по обеим осям одинаков, углы зум не искажает — только radii
    // пересчитываются в px.
    let zoom = camera.effective_zoom;
    let p = (in.world - in.center) * zoom;
    let r = length(p);
    let r0 = in.radii.x * zoom;
    let r1 = in.radii.y * zoom;

    // SDF annular-wedge (кольцевой клин), пиксели:
    // 1) радиальная часть: отрицательна между r0 и r1 (кольцо);
    // 2) угловая часть: знаковое окружное расстояние до дуги [a0, a1]
    //    (отрицательно внутри), умноженное на r — локальная px-метрика;
    // 3) пересечение (max) полуплоскостей — клин сектора. Обе части знаковые,
    //    иначе max() с нулевой внутренней частью «съел» бы радиальную.
    let d_radial = max(r - r1, r0 - r);
    var th = atan2(p.y, p.x);
    if th < 0.0 {
        th = th + 6.283185307179586;
    }
    let d_ang = angle_gap(th, in.angles.x, in.angles.y) * r;
    let sd = max(d_radial, d_ang);

    // AA: сглаживание по экранной производной SDF (не тоньше 0.75 px)
    let aa = max(fwidth(sd), 0.75);
    let alpha = (1.0 - smoothstep(-aa, aa, sd)) * in.fill.a;
    if alpha <= 0.004 {
        discard;
    }
    return vec4<f32>(in.fill.rgb, alpha);
}
