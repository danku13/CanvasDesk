//! Бесконечная сетка в world-space (T2): GPU-пайплайн + чистая функция внешнего вида.
//!
//! Толщина линий — в screen-space через производные (`fwidth`) в шейдере,
//! поэтому линии не мерцают и не меняют толщину при зуме.

use crate::camera::Camera;

/// Шаг мелкой сетки в world-пикселях (SPEC, T2).
pub const MINOR_STEP: f32 = 20.0;
/// Шаг крупной сетки в world-пикселях (SPEC, T2).
pub const MAJOR_STEP: f32 = 100.0;

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

/// Альфы линий сетки по зуму.
///
/// Мелкая сетка (screen-шаг = MINOR_STEP * zoom) гасится, когда линии сливаются
/// (порог ~10 screen-px между линиями) — иначе муар и мерцание при отдалении.
/// Крупная видна всегда, но при экстремальном отдалении приглушается.
pub fn grid_appearance(zoom: f32) -> GridAppearance {
    let minor_screen_step = MINOR_STEP * zoom;
    // Полная видимость при шаге >= 10 screen-px (zoom >= 0.5), гашение к ~4 px
    let minor_alpha = smoothstep(4.0, 10.0, minor_screen_step);
    let major_screen_step = MAJOR_STEP * zoom;
    let major_alpha = smoothstep(4.0, 12.0, major_screen_step).clamp(0.15, 1.0);
    GridAppearance {
        minor_alpha,
        major_alpha,
    }
}

/// Uniform камеры для шейдера сетки (32 байта, layout по правилам WGSL).
#[derive(Debug, Clone, Copy)]
struct GridUniform {
    position: [f32; 2],
    viewport: [f32; 2],
    effective_zoom: f32,
    minor_alpha: f32,
    major_alpha: f32,
    _pad: f32,
}

impl GridUniform {
    fn to_bytes(self) -> [u8; 32] {
        let floats = [
            self.position[0],
            self.position[1],
            self.viewport[0],
            self.viewport[1],
            self.effective_zoom,
            self.minor_alpha,
            self.major_alpha,
            self._pad,
        ];
        let mut bytes = [0u8; 32];
        for (i, value) in floats.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
        bytes
    }
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
            size: 32,
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
    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport: [f32; 2],
        scale_factor: f32,
    ) {
        let appearance = grid_appearance(camera.zoom());
        let uniform = GridUniform {
            position: camera.position(),
            viewport,
            effective_zoom: camera.zoom() * scale_factor,
            minor_alpha: appearance.minor_alpha,
            major_alpha: appearance.major_alpha,
            _pad: 0.0,
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
    use crate::camera::MIN_ZOOM;

    /// При zoom = 1.0 мелкая сетка полностью видна, при MIN_ZOOM — погашена.
    #[test]
    fn minor_grid_fades_out_when_zoomed_out() {
        let near = grid_appearance(1.0);
        assert!(
            (near.minor_alpha - 1.0).abs() < 1e-6,
            "при zoom 1.0 мелкая сетка должна быть видна полностью: {}",
            near.minor_alpha
        );
        let far = grid_appearance(MIN_ZOOM);
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
            let appearance = grid_appearance(zoom);
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
        let mut previous = grid_appearance(MIN_ZOOM).minor_alpha;
        let mut zoom = MIN_ZOOM;
        while zoom < 4.0 {
            zoom *= 1.05;
            let current = grid_appearance(zoom).minor_alpha;
            assert!(
                current >= previous - 1e-6,
                "альфа упала с {previous} до {current} при zoom {zoom}"
            );
            previous = current;
        }
    }

    /// Uniform сериализуется в 32 байта — layout WGSL-структуры.
    #[test]
    fn uniform_layout_is_32_bytes() {
        let uniform = GridUniform {
            position: [1.5, -2.5],
            viewport: [1920.0, 1080.0],
            effective_zoom: 2.0,
            minor_alpha: 0.7,
            major_alpha: 1.0,
            _pad: 0.0,
        };
        let bytes = uniform.to_bytes();
        assert_eq!(bytes.len(), 32);
        assert_eq!(&bytes[0..4], &1.5f32.to_ne_bytes());
        assert_eq!(&bytes[16..20], &2.0f32.to_ne_bytes());
        assert_eq!(&bytes[24..28], &1.0f32.to_ne_bytes());
    }
}
