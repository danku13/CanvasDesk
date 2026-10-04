// Бесконечная сетка в world-space (T2).
// Линии рисуются во фрагментном шейдере через fwidth — толщина ~1 физический px
// независимо от зума, без мерцания при масштабировании. Шаги, режим
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

/// Радиус точки в физических px (режим «точки»).
const DOT_RADIUS: f32 = 1.5;

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
// Толщина линии ~1 px в screen-space за счёт fwidth.
fn grid_line(coord: f32, step: f32) -> f32 {
    let scaled = coord / step;
    let dist = abs(fract(scaled - 0.5) - 0.5) / fwidth(scaled);
    return 1.0 - min(dist, 1.0);
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
        // Точки в узлах мелкой сетки; крупная в этом режиме не рисуется
        let dx = node_dist_px(world.x, grid.minor_step, grid.effective_zoom);
        let dy = node_dist_px(world.y, grid.minor_step, grid.effective_zoom);
        let r = length(vec2<f32>(dx, dy));
        minor = (1.0 - min(r / DOT_RADIUS, 1.0)) * grid.minor_alpha;
        major = 0.0;
    } else {
        minor = max(
            grid_line(world.x, grid.minor_step),
            grid_line(world.y, grid.minor_step),
        ) * grid.minor_alpha;
        major = max(
            grid_line(world.x, grid.major_step),
            grid_line(world.y, grid.major_step),
        ) * grid.major_alpha;
    }

    // Крупная линия перекрывает мелкую в точках совпадения
    let color = minor_color * minor + major_color * major * (1.0 - minor);
    let alpha = max(minor, major);
    return vec4<f32>(color, alpha);
}
