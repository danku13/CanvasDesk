// Бесконечная сетка в world-space (T2).
// Линии рисуются во фрагментном шейдере через fwidth — толщина ~1 физический px
// независимо от зума, без мерцания при масштабировании.

struct GridUniform {
    position: vec2<f32>,        // мировая точка в центре viewport
    viewport: vec2<f32>,        // размер viewport в физических px
    effective_zoom: f32,        // zoom * scale_factor (world -> физические px)
    minor_alpha: f32,
    major_alpha: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> grid: GridUniform;

const MINOR_STEP: f32 = 20.0;   // world px, синхронизировано с grid.rs
const MAJOR_STEP: f32 = 100.0;  // world px

const MINOR_COLOR: vec3<f32> = vec3<f32>(0.165, 0.165, 0.188); // #2a2a30
const MAJOR_COLOR: vec3<f32> = vec3<f32>(0.220, 0.220, 0.247); // #38383f

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

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let world = grid.position
        + (frag.xy - grid.viewport * 0.5) / grid.effective_zoom;

    let minor = max(
        grid_line(world.x, MINOR_STEP),
        grid_line(world.y, MINOR_STEP),
    ) * grid.minor_alpha;
    let major = max(
        grid_line(world.x, MAJOR_STEP),
        grid_line(world.y, MAJOR_STEP),
    ) * grid.major_alpha;

    // Крупная линия перекрывает мелкую в точках совпадения
    let color = MINOR_COLOR * minor + MAJOR_COLOR * major * (1.0 - minor);
    let alpha = max(minor, major);
    return vec4<f32>(color, alpha);
}
