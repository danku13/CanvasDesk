//! Бесконечная сетка в world-space (T2): GPU-пайплайн + чистая функция внешнего вида.
//!
//! Толщина линий — в screen-space через производные (`fwidth`) в шейдере,
//! поэтому линии не мерцают и не меняют толщину при зуме.
//! Zoom-адаптивность (FR-038, п.3 v2): выше sub-порога — дополнительные
//! sub-линии полушага, ниже coarse-порога — только major-шаг
//! (`adaptive_grid_steps`; рендер передаёт результат в uniform как шаги).

use crate::camera::Camera;

/// Видимость линий сетки при данном зуме (альфы для шейдера).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridAppearance {
    /// Альфа мелкой сетки (0..1).
    pub minor_alpha: f32,
    /// Альфа крупной сетки (0..1).
    pub major_alpha: f32,
}

/// Плавный переход 0→1 на отрезке [edge0, edge1] (аналог GLSL smoothstep).
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Порог sub-сетки по умолчанию (FR-038, п.3 v2, ~150%): выше — рисуются
/// дополнительные sub-линии полушага. Вынесен в настройки Snap (T-038.4).
pub const DEFAULT_SUB_ZOOM: f32 = 1.5;

/// Порог coarse-сетки по умолчанию (FR-038, п.3 v2, ~50%): ниже — вместо
/// minor рисуется только major-шаг. Вынесен в настройки Snap (T-038.4).
pub const DEFAULT_COARSE_ZOOM: f32 = 0.5;

/// Zoom-адаптивные эффективные шаги сетки (FR-038, п.3 v2): (minor, major).
///
/// * `zoom > sub_zoom` — дополнительные sub-линии полушага для точной
///   раскладки при приближении (эффективный minor = minor/2);
/// * `zoom < coarse_zoom` — шаг укрупняется до major: minor-линии сливаются
///   с major и остаётся только крупный шаг (coarse-grid при отдалении);
/// * между порогами — шаги из настроек без изменений.
///
/// Чистая функция без состояния: рендер каждый кадр передаёт БАЗОВЫЕ шаги
/// из `GridDensity` (настройки), поэтому результат стабилен покадрово.
/// Порядок веток (sub → coarse → база) и строгие неравенства ЗЕРКАЛЬНЫ
/// `canvas-app::snap::effective_grid_step` (T-038.2): рендер не зависит от
/// app (ADR-0012 — слои core → scene → render → app), поэтому семантика
/// продублирована и закреплена тестами — шаг снапа и линии сетки на экране
/// совпадают при ЛЮБЫХ порогах, включая вырожденные `sub_zoom <=
/// coarse_zoom` (их валидация — забота приложения, T-038.4). Клампы:
/// неположительные шаги возвращаются как есть (альфа `grid_appearance`
/// погасит вырожденный шаг), нечисловой зум — без изменений.
pub fn adaptive_grid_steps(
    minor: f32,
    major: f32,
    zoom: f32,
    sub_zoom: f32,
    coarse_zoom: f32,
) -> (f32, f32) {
    // NaN-шаг не пройдёт проверку — вернётся как есть (без изменений)
    let steps_valid = minor > 0.0 && major > 0.0;
    if !steps_valid || !zoom.is_finite() {
        return (minor, major);
    }
    // Зеркало effective_grid_step снап-движка: sub первым, coarse вторым
    if zoom > sub_zoom {
        (minor / 2.0, major)
    } else if zoom < coarse_zoom {
        (major, major)
    } else {
        (minor, major)
    }
}

/// Альфы линий сетки по зуму и шагам (`minor_step`/`major_step` — в world-px).
///
/// Мелкая сетка (screen-шаг = minor_step * zoom) гасится, когда линии сливаются
/// (порог ~10 screen-px между линиями) — иначе муар и мерцание при отдалении.
/// Крупная видна всегда, но при экстремальном отдалении приглушается.
pub fn grid_appearance(zoom: f32, minor_step: f32, major_step: f32) -> GridAppearance {
    let minor_screen_step = minor_step * zoom;
    // Полная видимость при шаге >= 10 screen-px (zoom >= 0.5 при 20 world-px),
    // гашение к ~4 px
    let minor_alpha = smoothstep(4.0, 10.0, minor_screen_step);
    let major_screen_step = major_step * zoom;
    let major_alpha = smoothstep(4.0, 12.0, major_screen_step).clamp(0.15, 1.0);
    GridAppearance {
        minor_alpha,
        major_alpha,
    }
}

/// Uniform камеры для шейдера сетки (80 байт, layout по правилам WGSL):
/// позиция/зум/альфы + шаги линий (плотность), режим (линии/точки) и цвета.
#[derive(Debug, Clone, Copy)]
struct GridUniform {
    position: [f32; 2],
    viewport: [f32; 2],
    effective_zoom: f32,
    minor_alpha: f32,
    major_alpha: f32,
    minor_step: f32,
    major_step: f32,
    /// 0 — линии, 1 — точки.
    mode: f32,
    _pad: [f32; 2],
    /// Цвета сетки (sRGB 0..1, w — не используется).
    minor_color: [f32; 4],
    major_color: [f32; 4],
}

impl GridUniform {
    fn to_bytes(self) -> [u8; 80] {
        let floats = [
            self.position[0],
            self.position[1],
            self.viewport[0],
            self.viewport[1],
            self.effective_zoom,
            self.minor_alpha,
            self.major_alpha,
            self.minor_step,
            self.major_step,
            self.mode,
            0.0,
            0.0,
            self.minor_color[0],
            self.minor_color[1],
            self.minor_color[2],
            self.minor_color[3],
            self.major_color[0],
            self.major_color[1],
            self.major_color[2],
            self.major_color[3],
        ];
        let mut bytes = [0u8; 80];
        for (i, value) in floats.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
        bytes
    }
}

/// Внешний вид сетки кадра (плотность, режим, цвета) — параметр update_camera.
#[derive(Debug, Clone, Copy)]
pub struct GridLook {
    /// (мелкий, крупный) шаг сетки в world-px (плотность).
    pub steps: (f32, f32),
    /// Режим «точки» вместо линий.
    pub dots: bool,
    /// (мелкий, крупный) цвета линий (тема).
    pub colors: ([f32; 3], [f32; 3]),
}

/// Пайплайн сетки: fullscreen-треугольник, линии считаются во фрагментном шейдере.
pub struct GridPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl GridPipeline {
    /// Создать пайплайн под формат surface.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("grid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/grid.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("grid"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("grid"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("grid"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
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
            label: Some("grid camera"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("grid"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
        }
    }

    /// Загрузить параметры камеры кадра.
    ///
    /// `viewport` — в физических пикселях (шейдер работает в frag coord);
    /// камера хранит логические координаты, поэтому зум домножается на scale_factor.
    /// `look` — внешний вид сетки (шаги/плотность, режим точек, цвета темы).
    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport: [f32; 2],
        scale_factor: f32,
        look: GridLook,
    ) {
        let appearance = grid_appearance(camera.zoom(), look.steps.0, look.steps.1);
        let uniform = GridUniform {
            position: camera.position(),
            viewport,
            effective_zoom: camera.zoom() * scale_factor,
            minor_alpha: appearance.minor_alpha,
            major_alpha: appearance.major_alpha,
            minor_step: look.steps.0,
            major_step: look.steps.1,
            mode: if look.dots { 1.0 } else { 0.0 },
            _pad: [0.0; 2],
            minor_color: [look.colors.0[0], look.colors.0[1], look.colors.0[2], 1.0],
            major_color: [look.colors.1[0], look.colors.1[1], look.colors.1[2], 1.0],
        };
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());
    }

    /// Нарисовать сетку в активном render pass (3 вершины, без буферов).
    pub fn draw<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{MAX_ZOOM, MIN_ZOOM};

    /// Базовые шаги (средняя плотность, SPEC T2).
    const MINOR: f32 = 20.0;
    const MAJOR: f32 = 100.0;

    /// При zoom = 1.0 мелкая сетка полностью видна, при MIN_ZOOM — погашена.
    #[test]
    fn minor_grid_fades_out_when_zoomed_out() {
        let near = grid_appearance(1.0, MINOR, MAJOR);
        assert!(
            (near.minor_alpha - 1.0).abs() < 1e-6,
            "при zoom 1.0 мелкая сетка должна быть видна полностью: {}",
            near.minor_alpha
        );
        let far = grid_appearance(MIN_ZOOM, MINOR, MAJOR);
        assert!(
            far.minor_alpha.abs() < 1e-6,
            "при zoom {MIN_ZOOM} мелкая сетка должна быть погашена: {}",
            far.minor_alpha
        );
    }

    /// Крупная сетка видна при любом зуме (ориентир при отдалении).
    #[test]
    fn major_grid_always_visible() {
        for zoom in [MIN_ZOOM, 0.1, 0.25, 1.0, 4.0] {
            let appearance = grid_appearance(zoom, MINOR, MAJOR);
            assert!(
                appearance.major_alpha >= 0.15,
                "крупная сетка должна оставаться видимой при zoom {zoom}: {}",
                appearance.major_alpha
            );
        }
    }

    /// Альфа мелкой сетки монотонно не убывает с ростом зума (нет дёргания границ).
    #[test]
    fn minor_alpha_monotonic_in_zoom() {
        let mut previous = grid_appearance(MIN_ZOOM, MINOR, MAJOR).minor_alpha;
        let mut zoom = MIN_ZOOM;
        while zoom < 4.0 {
            zoom *= 1.05;
            let current = grid_appearance(zoom, MINOR, MAJOR).minor_alpha;
            assert!(
                current >= previous - 1e-6,
                "альфа упала с {previous} до {current} при zoom {zoom}"
            );
            previous = current;
        }
    }

    /// Альфы зависят от screen-шага (шаг × зум): одинаковый screen-шаг —
    /// одинаковая альфа независимо от плотности; при отдалении частая сетка
    /// гаснет раньше редкой (иначе муар).
    #[test]
    fn alpha_depends_on_screen_step() {
        // Эквивалентность screen-шага: 10 world-px при zoom 0.6 = 20 при 0.3
        let dense = grid_appearance(0.6, 10.0, 50.0).minor_alpha;
        let medium = grid_appearance(0.3, 20.0, 100.0).minor_alpha;
        assert!(
            (dense - medium).abs() < 1e-6,
            "одинаковый screen-шаг → одинаковая альфа: {dense} vs {medium}"
        );
        // При отдалении частая гаснет раньше
        let zoom = 0.3;
        let dense = grid_appearance(zoom, 10.0, 50.0).minor_alpha;
        let sparse = grid_appearance(zoom, 40.0, 200.0).minor_alpha;
        assert!(
            sparse > dense,
            "при zoom {zoom} редкая ({sparse}) виднее частой ({dense})"
        );
    }

    /// FR-038 п.3 v2: выше sub-порога — sub-линии полушага (minor/2),
    /// major не меняется; на границе (zoom == sub_zoom) — ещё базовые шаги.
    #[test]
    fn adaptive_steps_add_sub_lines_above_threshold() {
        // Средняя плотность (SPEC T2): 20/100
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 1.51, 1.5, 0.5),
            (10.0, 100.0)
        );
        // Ровно на пороге — без sub (строгое неравенство)
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 1.5, 1.5, 0.5),
            (20.0, 100.0)
        );
        // Плотная сетка: sub-полушаг 5
        assert_eq!(adaptive_grid_steps(10.0, 50.0, 4.0, 1.5, 0.5), (5.0, 50.0));
        // MAX_ZOOM камеры — тоже sub
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, MAX_ZOOM, 1.5, 0.5),
            (10.0, 100.0)
        );
    }

    /// FR-038 п.3 v2: ниже coarse-порога — только major-шаг (minor = major);
    /// на границе (zoom == coarse_zoom) — ещё базовые шаги.
    #[test]
    fn adaptive_steps_coarse_below_threshold() {
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 0.49, 1.5, 0.5),
            (100.0, 100.0)
        );
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 0.5, 1.5, 0.5),
            (20.0, 100.0)
        );
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, MIN_ZOOM, 1.5, 0.5),
            (100.0, 100.0)
        );
    }

    /// Между порогами шаги из настроек без изменений; sub/coarse не
    /// совпадают ни с одним из них при корректных порогах.
    #[test]
    fn adaptive_steps_unchanged_between_thresholds() {
        for zoom in [0.51_f32, 0.75, 1.0, 1.49] {
            assert_eq!(
                adaptive_grid_steps(20.0, 100.0, zoom, 1.5, 0.5),
                (20.0, 100.0),
                "zoom {zoom} — базовые шаги"
            );
        }
    }

    /// Клампы: неположительные шаги и нечисловой зум возвращаются как есть
    /// (шейдерная альфа гасит вырожденный шаг); coarse-ветка — неподвижная
    /// точка (повторный вызов с её выходом как входом не меняет результат),
    /// функция детерминирована.
    #[test]
    fn adaptive_steps_clamps_and_fixed_point() {
        assert_eq!(adaptive_grid_steps(0.0, 100.0, 2.0, 1.5, 0.5), (0.0, 100.0));
        assert_eq!(
            adaptive_grid_steps(-5.0, 100.0, 2.0, 1.5, 0.5),
            (-5.0, 100.0)
        );
        assert_eq!(adaptive_grid_steps(20.0, 0.0, 2.0, 1.5, 0.5), (20.0, 0.0));
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, f32::NAN, 1.5, 0.5),
            (20.0, 100.0)
        );
        // Coarse — fixed point: (major, major) при zoom < coarse стабильно
        assert_eq!(
            adaptive_grid_steps(100.0, 100.0, 0.4, 1.5, 0.5),
            (100.0, 100.0)
        );
        // Детерминизм: те же аргументы — тот же результат
        let first = adaptive_grid_steps(20.0, 100.0, 2.0, 1.5, 0.5);
        let second = adaptive_grid_steps(20.0, 100.0, 2.0, 1.5, 0.5);
        assert_eq!(first, second);
    }

    /// Вырожденные пороги — семантика ЗЕРКАЛЬНА
    /// `canvas-app::snap::effective_grid_step` (порядок sub → coarse): шаг
    /// снапа и линии сетки совпадают при любых настройках, включая
    /// противоречивые (их валидация — забота приложения, T-038.4).
    #[test]
    fn adaptive_steps_thresholds_mirror_snap_engine() {
        // sub_zoom == coarse_zoom: при зуме выше порога работает sub-ветка
        // (zoom > sub_zoom проверяется первым — как в снап-движке)
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 2.0, 0.5, 0.5),
            (10.0, 100.0)
        );
        // sub_zoom < coarse_zoom: в зоне перекрытия побеждает sub (первая ветка)
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 0.6, 0.3, 0.5),
            (10.0, 100.0)
        );
        // ... ниже sub_zoom (< coarse_zoom) — coarse-ветка
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 0.25, 0.3, 0.5),
            (100.0, 100.0)
        );
        // NaN-порог — сравнение ложно, обе ветки не срабатывают: базовые шаги
        assert_eq!(
            adaptive_grid_steps(20.0, 100.0, 2.0, f32::NAN, 0.5),
            (20.0, 100.0)
        );
    }

    /// Оракул паритета с снап-движком: формула `effective_grid_step`
    /// (canvas-app/src/snap.rs, T-038.2) продублирована в тесте как эталон —
    /// любой дрейф эффективного minor-шага сетки от шага снапа ловится
    /// регрессом по матрице плотностей × зумов (п.3: snap к видимым линиям).
    #[test]
    fn adaptive_steps_minor_matches_snap_oracle() {
        fn snap_oracle(minor: f32, major: f32, zoom: f32, sub: f32, coarse: f32) -> f32 {
            if zoom > sub {
                minor / 2.0
            } else if zoom < coarse {
                major
            } else {
                minor
            }
        }
        // Все пресеты GridDensity (Dense 10/50, Medium 20/100, Sparse 40/200)
        for (minor, major) in [(10.0, 50.0), (20.0, 100.0), (40.0, 200.0)] {
            let mut zoom = MIN_ZOOM;
            while zoom <= MAX_ZOOM {
                let (m, mj) = adaptive_grid_steps(minor, major, zoom, 1.5, 0.5);
                assert_eq!(
                    m,
                    snap_oracle(minor, major, zoom, 1.5, 0.5),
                    "minor при zoom {zoom}"
                );
                assert_eq!(mj, major, "major не меняется: {zoom}");
                zoom += 0.01;
            }
        }
    }

    /// Uniform сериализуется в 80 байт — layout WGSL-структуры
    /// (позиция, viewport, зум, альфы, шаги, режим, выравнивание, цвета).
    #[test]
    fn uniform_layout_is_80_bytes() {
        let uniform = GridUniform {
            position: [1.5, -2.5],
            viewport: [1920.0, 1080.0],
            effective_zoom: 2.0,
            minor_alpha: 0.7,
            major_alpha: 1.0,
            minor_step: 20.0,
            major_step: 100.0,
            mode: 1.0,
            _pad: [0.0; 2],
            minor_color: [0.1, 0.2, 0.3, 1.0],
            major_color: [0.4, 0.5, 0.6, 1.0],
        };
        let bytes = uniform.to_bytes();
        assert_eq!(bytes.len(), 80);
        assert_eq!(&bytes[0..4], &1.5f32.to_ne_bytes());
        assert_eq!(&bytes[16..20], &2.0f32.to_ne_bytes());
        assert_eq!(&bytes[24..28], &1.0f32.to_ne_bytes());
        // Шаги: offsets 28 и 32; режим: offset 36
        assert_eq!(&bytes[28..32], &20.0f32.to_ne_bytes());
        assert_eq!(&bytes[32..36], &100.0f32.to_ne_bytes());
        assert_eq!(&bytes[36..40], &1.0f32.to_ne_bytes());
        // Цвета: minor 48..64, major 64..80
        assert_eq!(&bytes[48..52], &0.1f32.to_ne_bytes());
        assert_eq!(&bytes[64..68], &0.4f32.to_ne_bytes());
    }
}
