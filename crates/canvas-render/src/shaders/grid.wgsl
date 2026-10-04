// Бесконечная сетка в world-space (T2).
// Линии рисуются во фрагментном шейдере: ядро ~1.5px + AA в screen-space —
// толщина не меняется при зуме, без мерцания при масштабировании. Шаги, режим
// (линии/точки) и альфы приходят из uniform — плотность, вид настраиваются.
// ЦВЕТА — через текстуру 1×2 RGBA8Unorm (binding 1): texel (0,0) — minor,
// (1,0) — major. Почему не в uniform: на WebGL2 (wgpu 22 GL-бэкенд) цветовые
// поля в хвосте uniform-блока до шейдера не доезжают (точки/линии рисовались
// почти чёрным при верной геометрии/альфах — замер 2026-10-05); texel-загрузка
// идёт тем же путём, что атласы текста/иконок — там байты доходят 1:1.
// RGBA8Unorm: sample = байт/255 без гамма-кривой (raw passthrough).

struct GridUniform {
    position: vec2<f32>,        // мировая точка в центре viewport
    viewport: vec2<f32>,        // размер viewport в физических px
    effective_zoom: f32,        // zoom * scale_factor (world -> физические px)
    minor_alpha: f32,
    major_alpha: f32,
    minor_step: f32,            // шаг мелкой сетки, world px
    major_step: f32,            // шаг крупной сетки, world px
    mode: f32,                  // 0 — линии, 1 — точки
    _pad0: vec2<f32>,           // выравнивание до 64 байт
    _pad1: vec4<f32>,
};

@group(0) @binding(0) var<uniform> grid: GridUniform;
@group(0) @binding(1) var grid_colors: texture_2d<f32>;

/// Радиус точки в физических px (режим «точки»). 2.0 (было 1.5): при радиусе
/// 1.5 центральный пиксель узла получал ~50-70% альфы — точки выглядели
/// невидимыми (жалоба 2026-10-05); 2.0 + клампнутый хвост дают яркое ядро 2×2.
const DOT_RADIUS: f32 = 2.0;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
};

// Fullscreen-треугольник на 3 вершины без буферов.
@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VertexOutput;
    out.clip = vec4<f32>(positions[idx], 0.0, 1.0);
    return out;
}

// Интенсивность линии сетки с шагом step в точке coord (в world).
// ГЕОМЕТРИЯ В ПИКСЕЛЬНОМ ПРОСТРАНСТВЕ (фикс «невидимой сетки» 2026-10-05):
// прежний профиль `1 − min(dist/fwidth, 1)` давал ядру линии лишь ~50%
// покрытия (линия падала между пикселями; на SwiftShader/WebGL2 — до ~15%)
// — сетка и точки выглядели невидимыми при любых цветах темы. Теперь —
// расстояние до линии в ФИЗИЧЕСКИХ px, ядро 0.75px + клампнутый AA-хвост
// (тот же паттерн, что в cards.wgsl: aa = max(fwidth, 1.0)) — на любой
// DPR и любом производном-бэке линия держит полную альфу в ядре.
fn grid_line(coord: f32, step: f32, zoom: f32) -> f32 {
    let scaled = coord / step;
    let dist_px = abs(fract(scaled - 0.5) - 0.5) * step * zoom;
    let aa = max(fwidth(dist_px), 1.0);
    return 1.0 - smoothstep(0.75 - aa, 0.75 + aa, dist_px);
}

// Расстояние в физических px до ближайшего узла сетки по одной оси.
fn node_dist_px(coord: f32, step: f32, zoom: f32) -> f32 {
    let scaled = coord / step;
    return abs(fract(scaled - 0.5) - 0.5) * step * zoom;
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let world = grid.position
        + (frag.xy - grid.viewport * 0.5) / grid.effective_zoom;

    // Цвета из текстуры: texel (0,0) — minor, (1,0) — major (RGBA8Unorm).
    let minor_color = textureLoad(grid_colors, vec2<i32>(0, 0), 0).rgb;
    let major_color = textureLoad(grid_colors, vec2<i32>(1, 0), 0).rgb;

    var minor: f32;
    var major: f32;
    if (grid.mode > 0.5) {
        // Точки в узлах мелкой сетки; крупная в этом режиме не рисуется.
        // Профиль точки — тот же клампнутый хвост, что у линий (aa ≥ 1px):
        // яркое ядро до R−0.75px, плавный спад к R+0.75px.
        let dx = node_dist_px(world.x, grid.minor_step, grid.effective_zoom);
        let dy = node_dist_px(world.y, grid.minor_step, grid.effective_zoom);
        let r = length(vec2<f32>(dx, dy));
        minor = (1.0 - smoothstep(DOT_RADIUS - 0.75, DOT_RADIUS + 0.75, r))
            * grid.minor_alpha;
        major = 0.0;
    } else {
        minor = max(
            grid_line(world.x, grid.minor_step, grid.effective_zoom),
            grid_line(world.y, grid.minor_step, grid.effective_zoom),
        ) * grid.minor_alpha;
        major = max(
            grid_line(world.x, grid.major_step, grid.effective_zoom),
            grid_line(world.y, grid.major_step, grid.effective_zoom),
        ) * grid.major_alpha;
    }

    // Крупная линия перекрывает мелкую в точках совпадения
    let color = minor_color * minor + major_color * major * (1.0 - minor);
    let alpha = max(minor, major);
    return vec4<f32>(color, alpha);
}
