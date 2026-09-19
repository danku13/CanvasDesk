//! Слой направляющих магнитной раскладки (FR-038, T-038.3): линии smart
//! guides + ghost-предпросмотр snapped-позиции, instanced-проход по образцу
//! `sectors.rs` (FR-022) — свой uniform камеры, instanced vertex buffer,
//! alpha-blend, без depth. Форма (линия/пунктир/рамка) — SDF во фрагментном
//! шейдере (`shaders/guides.wgsl`); геометрия — world-прямоугольники.
//!
//! Данные кадра приходят от приложения (T-038.4) из `SnapOutcome` снап-движка
//! (`canvas-app/src/snap.rs`, T-038.2): оси направляющих в world-координатах
//! (`guides_x`/`guides_y`), источник каждой — `GuideSource` (зеркало
//! `SnapSource`: рендер не зависит от app — слои core → scene → render → app,
//! ADR-0012, а app собирается ПОСЛЕ рендера; конвертация — один `match` в
//! T-038.4). Ghost-рамка — snapped-bbox = текущий bbox + дельта снапа.
//!
//! «Нелинейное усиление» (п.8 v2): альфа и толщина линии — чистые функции
//! от интенсивности `guide_intensity(|дельта| / допуск)`; интенсивность
//! считается ПО ОСЯМ (дельта снапа по X — для вертикальных направляющих,
//! по Y — для горизонтальных).
//!
//! Z-порядок: после карточек/тамбнейлов/текста и снапшотов виджетов,
//! ДО wheel-меню (сектора FR-022) — см. порядок проходов в `renderer.rs`.
//!
//! GPU-часть минимальна: вся логика (интенсивность, альфа, толщина, сборка
//! инстансов) — чистые тестируемые функции этого модуля (правило AGENTS.md).

use crate::camera::Camera;
use crate::cards::CameraUniform;
use crate::theme::ThemeColors;

// ---------------------------------------------------------------------------
// Чистая логика (без GPU)
// ---------------------------------------------------------------------------

/// Источник привязки направляющей (FR-038, п.18; зеркало `SnapSource` из
/// `canvas-app/src/snap.rs` — см. доку модуля). Определяет стиль линии:
/// `Guide` — сплошная маджента (совпадение с соседом), `Grid` — пунктир
/// приглушённого тона (привязка к сетке): «однозначно показывает источник
/// привязки» двумя независимыми кодировками (штрих + яркость).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideSource {
    /// Ось от фоновой сетки (snap-to-grid).
    Grid,
    /// Ось от соседней ноды (smart guides: край/центр/середина).
    Guide,
}

/// Ось направляющей: world-координата + источник привязки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideLine {
    /// World-координата оси: x — для вертикальной (в `guides_x`),
    /// y — для горизонтальной (в `guides_y`).
    pub pos: f32,
    /// Источник привязки (стиль линии).
    pub source: GuideSource,
}

impl GuideLine {
    /// Ось с заданным источником (удобство при конвертации из `SnapOutcome`).
    pub fn new(pos: f32, source: GuideSource) -> Self {
        Self { pos, source }
    }
}

/// Данные слоя направляющих кадра (FR-038, T-038.3). Владеет данными —
/// приложение пересобирает на каждый кадр драга и передаёт в
/// [`crate::Renderer::set_guides`]; пустой кадр (`Default`) — слой не рисуется.
///
/// Поля `delta`/`tolerance` питают нелинейное усиление (п.8): интенсивность
/// линии оси = `guide_intensity(|дельта оси| / tolerance)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuidesFrame {
    /// Вертикальные направляющие (world x).
    pub guides_x: Vec<GuideLine>,
    /// Горизонтальные направляющие (world y).
    pub guides_y: Vec<GuideLine>,
    /// Ghost-предпросмотр: рамка bbox в snapped-позиции (п.2 v2).
    /// None — не рисуется (снапа нет). Считается приложением из текущего
    /// bbox перемещаемого + дельты снапа (см. [`GuidesFrame::from_snap`]).
    pub ghost: Option<[f32; 4]>,
    /// Модуль дельты снапа по осям в world px: `[|dx|, |dy|]`.
    pub delta: [f32; 2],
    /// Допуск снапа в world px (настройка `snap_tolerance_px`, T-038.4).
    pub tolerance: f32,
}

impl GuidesFrame {
    /// Есть ли что рисовать (без GPU-проверки — быстрая оценка для приложения).
    pub fn is_empty(&self) -> bool {
        self.guides_x.is_empty() && self.guides_y.is_empty() && self.ghost.is_none()
    }

    /// Собрать кадр из выхода снап-движка (T-038.2/T-038.4): текущий bbox
    /// перемещаемого (world), коррекция снапа (dx, dy), оси направляющих
    /// (`SnapOutcome.guides_x/guides_y`), единый источник результата
    /// (`SnapOutcome.source` → `GuideSource`) и допуск.
    /// Ghost = bbox + дельта; delta = [|dx|, |dy|] для усиления п.8.
    pub fn from_snap(
        bbox: [f32; 4],
        dx: f32,
        dy: f32,
        guides_x: Vec<f32>,
        guides_y: Vec<f32>,
        source: GuideSource,
        tolerance: f32,
    ) -> Self {
        Self {
            guides_x: guides_x
                .into_iter()
                .map(|pos| GuideLine::new(pos, source))
                .collect(),
            guides_y: guides_y
                .into_iter()
                .map(|pos| GuideLine::new(pos, source))
                .collect(),
            ghost: Some([bbox[0] + dx, bbox[1] + dy, bbox[2] + dx, bbox[3] + dy]),
            delta: [dx.abs(), dy.abs()],
            tolerance,
        }
    }
}

/// Нелинейное усиление направляющей (FR-038, п.8 v2): интенсивность 0..1
/// из отношения `ratio = |дельта снапа| / допуск`.
///
/// `1 - ratio²` с клампом [0, 1] (вариант из задачи): при дельте 0 — максимум
/// (линия «прилипла», яркая и толстая), у края допуска — 0; рост при
/// приближении к нулю нелинеен (квадрат): половина допуска даёт 0.75, а не 0.5.
/// Вне [0, ∞) — клампы: ratio < 0 трактуется как 0 (max), нечисловое/бесконечное
/// (в т.ч. допуск 0 при ненулевой дельте) — 0 (линия не рисуется ярче фона).
pub fn guide_intensity(ratio: f32) -> f32 {
    if !ratio.is_finite() {
        return 0.0;
    }
    let r = ratio.max(0.0); // отрицательный ratio трактуется как 0 (max)
    (1.0 - r * r).clamp(0.0, 1.0)
}

/// Альфа линии из интенсивности (п.8): плато 0.25 у края допуска — направляющая
/// видна сразу при входе в допуск и «усиливается» к нулю дельты. Кламп [0, 1].
pub fn guide_alpha(intensity: f32) -> f32 {
    let intensity = intensity.clamp(0.0, 1.0);
    0.25 + 0.75 * intensity
}

/// Толщина линии в физических px из интенсивности (п.8): 1 px у края допуска
/// → 2.5 px при точном совпадении. Кламп через интенсивность.
pub fn guide_thickness(intensity: f32) -> f32 {
    let intensity = intensity.clamp(0.0, 1.0);
    1.0 + 1.5 * intensity
}

/// Интенсивность одной оси кадра: дельта/допуск → `guide_intensity`.
/// Допуск ≤ 0 — вырожденный конфиг: дельта 0 трактуется как точное совпадение
/// (max), иначе линия не усиливается вовсе.
fn axis_intensity(delta: f32, tolerance: f32) -> f32 {
    let ratio = if tolerance > 0.0 {
        delta.abs() / tolerance
    } else if delta == 0.0 {
        0.0
    } else {
        f32::INFINITY
    };
    guide_intensity(ratio)
}

/// Палитра направляющих кадра — цвета из темы (CR-007: контраст на обеих
/// темах проверяется тестами `theme.rs`, WCAG 1.4.11 ≥ 3:1 к фону).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidePalette {
    /// Линия-«сосед» (guide): маджента темы.
    pub align: [f32; 4],
    /// Линия-сетка (grid): приглушённый тон того же семейства.
    pub grid: [f32; 4],
    /// Ghost-рамка snapped-позиции: маджента с низкой альфой.
    pub ghost: [f32; 4],
}

impl GuidePalette {
    /// Палитра из темы: ghost — цвет align с фиксированной низкой альфой.
    pub fn from_theme(theme: &ThemeColors) -> Self {
        let align = theme.guide_align;
        Self {
            align,
            grid: theme.guide_grid,
            ghost: [align[0], align[1], align[2], GHOST_ALPHA],
        }
    }
}

/// Стиль инстанса в `params.x`: сплошная линия (guide-источник).
pub const STYLE_GUIDE: f32 = 0.0;
/// Стиль инстанса: пунктирная линия (grid-источник).
pub const STYLE_GRID: f32 = 1.0;
/// Стиль инстанса: пунктирная рамка (ghost-предпросмотр).
pub const STYLE_GHOST: f32 = 2.0;

/// Период штриха grid-линий в физических px (стиль `STYLE_GRID`).
pub const DASH_PERIOD_PX: f32 = 9.0;
/// Период штриха ghost-рамки в физических px (крупнее линии — отличим).
pub const GHOST_DASH_PERIOD_PX: f32 = 12.0;
/// Толщина рамки ghost в физических px.
pub const GHOST_BORDER_PX: f32 = 2.0;
/// Альфа ghost-рамки (полупрозрачный предпросмотр, п.2 v2).
pub const GHOST_ALPHA: f32 = 0.45;
/// Множитель альфы grid-линий к guide-линиям (вторая кодировка источника).
pub const GRID_ALPHA_FACTOR: f32 = 0.85;
/// Запас квада под AA-край SDF, физических px (синхронно с шейдером).
const AA_MARGIN_PX: f32 = 1.5;

/// Один инстанс прохода: world-прямоугольник + цвет + параметры стиля
/// (12 floats = 48 байт, layout вершинного буфера фиксирован).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideInstance {
    /// World-прямоугольник [x0, y0, x1, y1] (линия вытянута на viewport).
    pub rect: [f32; 4],
    /// RGBA (альфа уже включает интенсивность п.8).
    pub color: [f32; 4],
    /// [style, half_px (полутолщина), dash_period_px, 0].
    pub params: [f32; 4],
}

impl GuideInstance {
    /// Число float на инстанс.
    pub const FLOATS: usize = 12;

    /// Сериализация в байты вершинного буфера (native-endian f32).
    pub fn write_to(&self, bytes: &mut Vec<u8>) {
        for value in self
            .rect
            .iter()
            .chain(self.color.iter())
            .chain(self.params.iter())
        {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
}

/// Чистая сборка инстансов кадра направляющих (world → геометрия прохода).
///
/// * `effective_zoom` — `zoom * scale_factor` (world → физические px);
/// * `viewport_world` — видимый world-прямоугольник камеры
///   (`Camera::visible_world_rect`), линии вытянуты на него;
/// * ghost рисуется ПЕРВЫМ (линии направляющих поверх его рамки при
///   совпадении осей); интенсивность линий — по осям дельты снапа.
pub fn build_instances(
    effective_zoom: f32,
    viewport_world: [f32; 4],
    frame: &GuidesFrame,
    palette: &GuidePalette,
) -> Vec<GuideInstance> {
    let mut out = Vec::new();
    if !effective_zoom.is_finite() || effective_zoom <= 0.0 {
        return out;
    }
    if let Some(ghost) = frame.ghost {
        // Вырожденный bbox не рисуем (нулевая площадь). Рамка = ТОЧНЫЙ
        // snapped-bbox: запас под AA-край добавляет вершинный шейдер
        // (расширяет квад инстанса на 1.5 px) — расширение здесь сдвинуло бы
        // SDF-рамку наружу, т.к. для рамки rect и есть форма.
        if ghost[0] < ghost[2] && ghost[1] < ghost[3] {
            out.push(GuideInstance {
                rect: ghost,
                color: palette.ghost,
                params: [
                    STYLE_GHOST,
                    GHOST_BORDER_PX * 0.5,
                    GHOST_DASH_PERIOD_PX,
                    0.0,
                ],
            });
        }
    }
    let intensity_x = axis_intensity(frame.delta[0], frame.tolerance);
    let intensity_y = axis_intensity(frame.delta[1], frame.tolerance);
    let ctx = LineContext {
        effective_zoom,
        viewport_world,
        palette,
    };
    for line in &frame.guides_x {
        push_line(&mut out, &ctx, line.pos, true, line.source, intensity_x);
    }
    for line in &frame.guides_y {
        push_line(&mut out, &ctx, line.pos, false, line.source, intensity_y);
    }
    out
}

/// Контекст сборки линии: зум, границы viewport и палитра кадра.
struct LineContext<'a> {
    effective_zoom: f32,
    viewport_world: [f32; 4],
    palette: &'a GuidePalette,
}

/// Добавить инстанс линии во весь viewport (`vertical` — линия x = pos,
/// иначе горизонтальная y = pos). Нечисловая ось пропускается (защита).
fn push_line(
    out: &mut Vec<GuideInstance>,
    ctx: &LineContext,
    pos: f32,
    vertical: bool,
    source: GuideSource,
    intensity: f32,
) {
    if !pos.is_finite() {
        return;
    }
    let (color, style, period) = match source {
        GuideSource::Guide => (ctx.palette.align, STYLE_GUIDE, 0.0),
        GuideSource::Grid => (ctx.palette.grid, STYLE_GRID, DASH_PERIOD_PX),
    };
    let thickness = guide_thickness(intensity);
    // Полутолщина в world + запас под AA-край
    let half = (thickness * 0.5 + AA_MARGIN_PX) / ctx.effective_zoom;
    let mut alpha = color[3] * guide_alpha(intensity);
    if source == GuideSource::Grid {
        alpha *= GRID_ALPHA_FACTOR;
    }
    let vp = ctx.viewport_world;
    let rect = if vertical {
        [pos - half, vp[1], pos + half, vp[3]]
    } else {
        [vp[0], pos - half, vp[2], pos + half]
    };
    out.push(GuideInstance {
        rect,
        color: [color[0], color[1], color[2], alpha],
        params: [style, thickness * 0.5, period, 0.0],
    });
}

// ---------------------------------------------------------------------------
// GPU-проход (по образцу sectors.rs)
// ---------------------------------------------------------------------------

/// Пайплайн направляющих: instanced quad + SDF линии/рамки во фрагменте.
pub struct GuidesPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl GuidesPipeline {
    /// Создать пайплайн под формат surface.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("guides"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/guides.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("guides"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("guides"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: (GuideInstance::FLOATS * 4) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x4, // rect
                1 => Float32x4, // color
                2 => Float32x4, // params
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("guides"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[instance_layout],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("guides camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("guides"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let instance_capacity = 64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("guides instances"),
            size: (GuideInstance::FLOATS * 4 * instance_capacity) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            instance_buffer,
            instance_capacity,
        }
    }

    /// Загрузить камеру и инстансы кадра; вернуть число инстансов.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport: [f32; 2],
        scale_factor: f32,
        instances: &[GuideInstance],
    ) -> u32 {
        let uniform = CameraUniform::new(camera, viewport, scale_factor);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("guides instances"),
                size: (GuideInstance::FLOATS * 4 * self.instance_capacity) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(instances.len() * GuideInstance::FLOATS * 4);
        for instance in instances {
            instance.write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        instances.len() as u32
    }

    /// Нарисовать `count` инстансов направляющих в активном render pass.
    pub fn draw<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>, count: u32) {
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..6, 0..count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    /// guide_intensity: границы и клампы (п.8).
    #[test]
    fn intensity_boundaries() {
        assert!(near(guide_intensity(0.0), 1.0), "дельта 0 — максимум");
        assert!(near(guide_intensity(1.0), 0.0), "край допуска — ноль");
        assert!(near(guide_intensity(0.5), 0.75), "квадрат: 1 - 0.25");
        assert!(near(guide_intensity(2.0), 0.0), "за допуском — кламп 0");
        assert!(
            near(guide_intensity(-0.5), 1.0),
            "отрицательный ratio — кламп к 0"
        );
        assert!(
            near(guide_intensity(f32::INFINITY), 0.0),
            "бесконечность — 0"
        );
        assert!(near(guide_intensity(f32::NAN), 0.0), "NaN — 0 (без паники)");
        assert!(guide_intensity(0.25) <= 1.0 && guide_intensity(0.25) >= 0.0);
    }

    /// guide_intensity: монотонное не-возрастание по ratio — «дёргания» нет.
    #[test]
    fn intensity_monotonic() {
        let mut previous = guide_intensity(0.0);
        let mut ratio = 0.0_f32;
        while ratio < 1.0 {
            ratio += 0.01;
            let current = guide_intensity(ratio);
            assert!(
                current <= previous + 1e-6,
                "интенсивность выросла с {previous} до {current} при ratio {ratio}"
            );
            previous = current;
        }
    }

    /// guide_intensity: нелинейность — половина допуска даёт 0.75 (не 0.5,
    /// как у линейной), усиление к нулю дельты идёт быстрее в дальней зоне.
    #[test]
    fn intensity_nonlinear() {
        let mid = guide_intensity(0.5);
        assert!(
            mid - 0.5 > 0.2,
            "квадратичная кривая выше линейной в середине: {mid}"
        );
        // Прирост от 0.75→1 больше, чем от 0→0.25: кривая «прижимается» к 1
        let near_zero = guide_intensity(0.0) - guide_intensity(0.25);
        let near_edge = guide_intensity(0.75) - guide_intensity(1.0);
        assert!(
            near_edge > near_zero,
            "падение у края допуска круче: {near_edge} vs {near_zero}"
        );
    }

    /// Альфа и толщина — растут по интенсивности, в границах диапазонов.
    #[test]
    fn alpha_and_thickness_ranges() {
        let mut intensity = 0.0;
        while intensity <= 1.0 {
            let alpha = guide_alpha(intensity);
            assert!((0.25..=1.0).contains(&alpha), "альфа {alpha} вне [0.25, 1]");
            let thickness = guide_thickness(intensity);
            assert!(
                (1.0..=2.5).contains(&thickness),
                "толщина {thickness} вне [1, 2.5] px"
            );
            intensity += 0.05;
        }
        assert!(near(guide_alpha(1.0), 1.0));
        assert!(near(guide_thickness(1.0), 2.5));
        // Вне диапазона — кламп
        assert!(near(guide_alpha(2.0), 1.0));
        assert!(near(guide_thickness(-1.0), 1.0));
    }

    /// axis_intensity: дельты осей с допуском и вырожденный допуск.
    #[test]
    fn axis_intensity_scales_by_tolerance() {
        assert!(near(axis_intensity(0.0, 10.0), 1.0));
        assert!(near(axis_intensity(5.0, 10.0), 0.75));
        assert!(near(axis_intensity(-5.0, 10.0), 0.75), "модуль дельты");
        assert!(near(axis_intensity(10.0, 10.0), 0.0));
        // Вырожденный допуск: дельта 0 — точное совпадение, иначе — ноль
        assert!(near(axis_intensity(0.0, 0.0), 1.0));
        assert!(near(axis_intensity(3.0, 0.0), 0.0));
        assert!(near(axis_intensity(3.0, -1.0), 0.0));
    }

    /// Геометрия: вертикальная guide-линия — узкий квад во всю высоту
    /// viewport на world-координате; горизонтальная — во всю ширину.
    #[test]
    fn line_instances_span_viewport() {
        let frame = GuidesFrame {
            guides_x: vec![GuideLine::new(100.0, GuideSource::Guide)],
            guides_y: vec![GuideLine::new(-50.0, GuideSource::Guide)],
            ghost: None,
            delta: [0.0, 0.0],
            tolerance: 10.0,
        };
        let palette = GuidePalette {
            align: [1.0, 0.0, 1.0, 1.0],
            grid: [0.5, 0.0, 0.5, 1.0],
            ghost: [1.0, 0.0, 1.0, 0.45],
        };
        let vp = [-500.0, -300.0, 500.0, 300.0];
        let instances = build_instances(2.0, vp, &frame, &palette);
        assert_eq!(instances.len(), 2, "две линии, без ghost");
        // Вертикальная: x-центр = 100, y — границы viewport
        let v = instances[0];
        let v_half = (v.rect[2] - v.rect[0]) * 0.5;
        assert!(near((v.rect[0] + v.rect[2]) * 0.5, 100.0), "ось x = 100");
        assert!(
            near(v.rect[1], -300.0) && near(v.rect[3], 300.0),
            "высота = viewport"
        );
        // Полутолщина = (2.5/2 + 1.5 margin)/zoom 2 = 1.375 world
        assert!(near(v_half, 1.375), "полутолщина квада: {v_half}");
        assert_eq!(v.params[0], STYLE_GUIDE, "guide — сплошная");
        assert!(near(v.params[2], 0.0), "без штриха");
        assert!(near(v.color[3], 1.0), "альфа максимум при дельте 0");
        // Горизонтальная: y-центр = -50, x — границы viewport
        let h = instances[1];
        assert!(near((h.rect[1] + h.rect[3]) * 0.5, -50.0), "ось y = -50");
        assert!(
            near(h.rect[0], -500.0) && near(h.rect[2], 500.0),
            "ширина = viewport"
        );
        // Интенсивность по ОСЯМ: delta.x → вертикальные, delta.y → горизонтальные
        let frame = GuidesFrame {
            guides_x: vec![GuideLine::new(0.0, GuideSource::Guide)],
            guides_y: vec![GuideLine::new(0.0, GuideSource::Guide)],
            ghost: None,
            delta: [0.0, 10.0],
            tolerance: 10.0,
        };
        let instances = build_instances(1.0, vp, &frame, &palette);
        assert!(
            near(instances[0].color[3], 1.0),
            "x-линия: дельта 0 — максимум"
        );
        assert!(
            near(instances[1].color[3], 0.25),
            "y-линия: дельта = допуск — плато"
        );
    }

    /// Стили источников: guide — сплошная полная альфа, grid — пунктир
    /// (period > 0) и приглушение; обоим — интенсивность от общей дельты.
    #[test]
    fn grid_source_is_dashed_and_dimmer() {
        let frame = GuidesFrame {
            guides_x: vec![
                GuideLine::new(0.0, GuideSource::Guide),
                GuideLine::new(20.0, GuideSource::Grid),
            ],
            guides_y: Vec::new(),
            ghost: None,
            delta: [2.0, 0.0],
            tolerance: 10.0,
        };
        let palette = GuidePalette {
            align: [1.0, 0.18, 0.83, 1.0],
            grid: [0.86, 0.16, 0.72, 1.0],
            ghost: [1.0, 0.18, 0.83, 0.45],
        };
        let instances = build_instances(1.0, [-100.0, -100.0, 100.0, 100.0], &frame, &palette);
        assert_eq!(instances.len(), 2);
        let solid = &instances[0];
        let dashed = &instances[1];
        assert_eq!(solid.params[0], STYLE_GUIDE);
        assert_eq!(dashed.params[0], STYLE_GRID, "grid — пунктир");
        assert!(near(dashed.params[2], DASH_PERIOD_PX), "период штриха");
        assert!(
            dashed.color[3] < solid.color[3],
            "grid-линия приглушена: {} < {}",
            dashed.color[3],
            solid.color[3]
        );
        // Интенсивность: дельта 2 из допуска 10 → 1 - 0.04 = 0.96
        let expected_alpha = 0.25 + 0.75 * 0.96;
        assert!(near(solid.color[3], expected_alpha));
    }

    /// Ghost-рамка: квад = ТОЧНЫЙ snapped-bbox (AA-запас даёт вершинный
    /// шейдер), стиль пунктирной рамки, рисуется первым (линии поверх при
    /// совпадении осей).
    #[test]
    fn ghost_rect_instance() {
        let frame = GuidesFrame::from_snap(
            [10.0, 20.0, 110.0, 70.0],
            5.0,
            -4.0,
            vec![15.0],
            Vec::new(),
            GuideSource::Guide,
            8.0,
        );
        let palette = GuidePalette {
            align: [1.0, 0.18, 0.83, 1.0],
            grid: [0.86, 0.16, 0.72, 1.0],
            ghost: [1.0, 0.18, 0.83, 0.45],
        };
        let instances = build_instances(1.0, [-100.0, -100.0, 200.0, 200.0], &frame, &palette);
        assert_eq!(instances.len(), 2, "ghost + одна линия");
        let ghost = &instances[0];
        assert_eq!(ghost.params[0], STYLE_GHOST);
        assert!(
            near(ghost.params[1], GHOST_BORDER_PX * 0.5),
            "полутолщина рамки"
        );
        assert!(near(ghost.params[2], GHOST_DASH_PERIOD_PX), "штрих рамки");
        // Snapped = bbox + дельта: [15, 16, 115, 66] — рамка точно по bbox,
        // запас под AA добавляет vs_main (расширение квада, не формы)
        assert!(near(ghost.rect[0], 15.0));
        assert!(near(ghost.rect[1], 16.0));
        assert!(near(ghost.rect[2], 115.0));
        assert!(near(ghost.rect[3], 66.0));
        assert!(near(ghost.color[3], GHOST_ALPHA), "полупрозрачная рамка");
    }

    /// Пустой/вырожденный кадр и защита: нечисловые оси пропускаются,
    /// вырожденный ghost не рисуется, нулевой зум — пусто.
    #[test]
    fn degenerate_inputs_are_safe() {
        let palette = GuidePalette {
            align: [1.0, 0.0, 1.0, 1.0],
            grid: [0.5, 0.0, 0.5, 1.0],
            ghost: [1.0, 0.0, 1.0, 0.45],
        };
        // Пустой кадр
        assert!(build_instances(
            1.0,
            [-1.0, -1.0, 1.0, 1.0],
            &GuidesFrame::default(),
            &palette
        )
        .is_empty());
        // Нечисловая ось пропущена, конечная осталась
        let frame = GuidesFrame {
            guides_x: vec![
                GuideLine::new(f32::NAN, GuideSource::Guide),
                GuideLine::new(5.0, GuideSource::Guide),
            ],
            guides_y: vec![GuideLine::new(f32::INFINITY, GuideSource::Grid)],
            ghost: None,
            delta: [0.0, 0.0],
            tolerance: 1.0,
        };
        let instances = build_instances(1.0, [-10.0, -10.0, 10.0, 10.0], &frame, &palette);
        assert_eq!(instances.len(), 1, "NaN/∞ оси отброшены");
        // Вырожденный ghost (нулевая ширина) не рисуется
        let frame = GuidesFrame {
            ghost: Some([5.0, 5.0, 5.0, 10.0]),
            ..GuidesFrame::default()
        };
        assert!(
            build_instances(1.0, [-10.0, -10.0, 10.0, 10.0], &frame, &palette).is_empty(),
            "нулевая ширина ghost"
        );
        // Нечисловой/нулевой зум — пусто (нет смысла считать px→world)
        let frame = GuidesFrame {
            guides_x: vec![GuideLine::new(0.0, GuideSource::Guide)],
            ..GuidesFrame::default()
        };
        assert!(build_instances(0.0, [-10.0, -10.0, 10.0, 10.0], &frame, &palette).is_empty());
        assert!(build_instances(f32::NAN, [-10.0, -10.0, 10.0, 10.0], &frame, &palette).is_empty());
    }

    /// from_snap: маппинг SnapOutcome → кадр (ghost = bbox + дельта,
    /// delta = модули, оси помечены единым источником).
    #[test]
    fn from_snap_maps_outcome() {
        let frame = GuidesFrame::from_snap(
            [0.0, 0.0, 40.0, 30.0],
            -3.5,
            2.0,
            vec![-1.75, 18.25],
            vec![16.0],
            GuideSource::Grid,
            12.0,
        );
        assert_eq!(frame.guides_x.len(), 2);
        assert_eq!(frame.guides_y.len(), 1);
        assert!(frame.guides_x.iter().all(|l| l.source == GuideSource::Grid));
        assert_eq!(frame.guides_x[1].pos, 18.25);
        assert_eq!(frame.ghost, Some([-3.5, 2.0, 36.5, 32.0]), "bbox + дельта");
        assert_eq!(frame.delta, [3.5, 2.0], "модули дельт");
        assert_eq!(frame.tolerance, 12.0);
        assert!(!frame.is_empty());
        // Snap-to-grid без осей направляющих: ghost остаётся — предпросмотр
        // рисуется и без линий (оси появятся при snap-to-guides, п.6)
        let no_axes = GuidesFrame::from_snap(
            [0.0, 0.0, 1.0, 1.0],
            2.0,
            0.0,
            Vec::new(),
            Vec::new(),
            GuideSource::Grid,
            5.0,
        );
        assert!(no_axes.guides_x.is_empty() && no_axes.guides_y.is_empty());
        assert!(no_axes.ghost.is_some(), "snap-to-grid: ghost без осей");
        assert!(!no_axes.is_empty());
        // Полностью пустой кадр — Default (слой молчит)
        assert!(GuidesFrame::default().is_empty());
    }

    /// Сериализация инстанса: 12 float = 48 байт, порядок полей фиксирован.
    #[test]
    fn instance_layout_is_12_floats() {
        let inst = GuideInstance {
            rect: [1.0, 2.0, 3.0, 4.0],
            color: [0.5, 0.6, 0.7, 0.8],
            params: [STYLE_GRID, 1.25, DASH_PERIOD_PX, 0.0],
        };
        assert_eq!(GuideInstance::FLOATS, 12);
        let mut bytes = Vec::new();
        inst.write_to(&mut bytes);
        assert_eq!(bytes.len(), 48);
        let floats: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes(c.try_into().expect("4 байта")))
            .collect();
        assert_eq!(
            floats,
            vec![1.0, 2.0, 3.0, 4.0, 0.5, 0.6, 0.7, 0.8, 1.0, 1.25, 9.0, 0.0]
        );
    }

    /// Палитра из темы: ghost — цвет align с альфой GHOST_ALPHA.
    #[test]
    fn palette_from_theme() {
        for theme in [ThemeColors::dark(), ThemeColors::light()] {
            let palette = GuidePalette::from_theme(&theme);
            assert_eq!(palette.align, theme.guide_align);
            assert_eq!(palette.grid, theme.guide_grid);
            assert!(near(palette.ghost[0], theme.guide_align[0]));
            assert!(near(palette.ghost[3], GHOST_ALPHA));
        }
    }
}
