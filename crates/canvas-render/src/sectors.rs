//! Donut/pie-сектора (FR-022, рестайл wheel-меню 2026-09-16): instanced-проход
//! кольцевых секторов с центральным отверстием — форма радиального меню по
//! ориентиру circular-menu (https://www.npmjs.com/package/circular-menu;
//! реализация своя: JS-пакет не переносится).
//!
//! Геометрия сектора — annular-wedge (кольцевой клин): центр окружности,
//! внутренний/внешний радиусы r0/r1 и углы a0/a1 (нулевой угол — 12 часов,
//! направление — по часовой; экранные координаты, y вниз: точка дуги —
//! `(center + r·cos a, center + r·sin a)`). Форма считается SDF во
//! фрагментном шейдере (`shaders/sectors.wgsl`) — поворотов/дуг у
//! карточного пайплайна нет, сектора им не нарисовать. Vertex-шейдер
//! разворачивает bounding-квад сектора из тех же параметров (чистая
//! математика, см. `sector_bbox` в WGSL).
//!
//! Инстанс — 10 floats (40 байт), world-space. Пайплайн — по образцу
//! `cards.rs`: свой uniform буфер камеры, instanced vertex buffer,
//! alpha-blend, без depth.

use crate::camera::Camera;
use crate::cards::CameraUniform;

/// Один кольцевой сектор (world-space).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectorInstance {
    /// Центр окружности, world.
    pub center: [f32; 2],
    /// Внутренний радиус (отверстие donut), world px.
    pub r0: f32,
    /// Внешний радиус, world px.
    pub r1: f32,
    /// Начальный угол дуги, радианы (12 часов = `-π/2`, по часовой).
    pub a0: f32,
    /// Конечный угол дуги, радианы (a1 > a0, развёрнутый интервал).
    pub a1: f32,
    /// Заливка RGBA.
    pub fill: [f32; 4],
}

impl SectorInstance {
    /// Число float на инстанс (layout вершинного буфера фиксирован).
    pub const FLOATS: usize = 10;

    /// Сериализация в байты вершинного буфера (native-endian f32).
    pub fn write_to(&self, bytes: &mut Vec<u8>) {
        let floats = [
            self.center[0],
            self.center[1],
            self.r0,
            self.r1,
            self.a0,
            self.a1,
            self.fill[0],
            self.fill[1],
            self.fill[2],
            self.fill[3],
        ];
        for value in floats {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
}

/// Нормализация угла в `[0, TAU)`.
pub fn norm_angle(a: f32) -> f32 {
    a.rem_euclid(std::f32::consts::TAU)
}

/// Знаковое угловое расстояние от `theta` до дугового интервала `[a0, a1]`
/// (радианы, окружная метрика): отрицательное — theta строго внутри дуги,
/// 0 — на краю, положительное — снаружи (до ближайшего края). Развёрнутый
/// интервал (a1 > a0, может выходить за `[0, TAU)`) приводится окружно.
/// Зеркало `angle_gap` в `shaders/sectors.wgsl` — hit-test wheel-меню
/// (`template_ui`) использует эту Rust-версию, рендер — WGSL; обе тестируются
/// (инвариант WYSIWYG). Полный круг (span ≥ TAU) — всегда «внутри» (-1).
pub fn angle_gap(theta: f32, a0: f32, a1: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let span = a1 - a0;
    if span >= tau {
        return -1.0;
    }
    // d — theta, приведённый в [a0, a0 + tau): окружная позиция от начала дуги
    let d = (theta - a0).rem_euclid(tau);
    if d <= span {
        -d.min(span - d)
    } else {
        (d - span).min(tau - d)
    }
}

/// Пайплайн donut-секторов: instanced quad + SDF annular-wedge во фрагменте.
pub struct SectorsPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl SectorsPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sectors"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sectors.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sectors"),
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
            label: Some("sectors"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: (SectorInstance::FLOATS * 4) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x2, // center
                1 => Float32x2, // r0, r1
                2 => Float32x2, // a0, a1
                3 => Float32x4, // fill
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sectors"),
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
            label: Some("sectors camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sectors"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let instance_capacity = 64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sectors instances"),
            size: (SectorInstance::FLOATS * 4 * instance_capacity) as wgpu::BufferAddress,
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
        instances: &[SectorInstance],
    ) -> u32 {
        let uniform = CameraUniform::new(camera, viewport, scale_factor);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sectors instances"),
                size: (SectorInstance::FLOATS * 4 * self.instance_capacity) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(instances.len() * SectorInstance::FLOATS * 4);
        for instance in instances {
            instance.write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        instances.len() as u32
    }

    /// Нарисовать `count` секторов в активном render pass.
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
        (a - b).abs() < 1e-4
    }

    /// Сериализация: 12 float = 48 байт, порядок полей фиксирован.
    #[test]
    fn instance_layout_is_12_floats() {
        let inst = SectorInstance {
            center: [1.0, 2.0],
            r0: 3.0,
            r1: 4.0,
            a0: 5.0,
            a1: 6.0,
            fill: [7.0, 8.0, 9.0, 0.5],
        };
        assert_eq!(SectorInstance::FLOATS, 10);
        let mut bytes = Vec::new();
        inst.write_to(&mut bytes);
        assert_eq!(bytes.len(), 40);
        let floats: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes(c.try_into().expect("4 байта")))
            .collect();
        assert_eq!(
            floats,
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 0.5]
        );
    }

    /// Угловое расстояние: внутри дуги — отрицательное (по модулю — до
    /// ближайшего края), на краю — 0, вне — положительное окружное расстояние
    /// до ближайшего края (в т.ч. wrap-around через 0/2π и развёрнутые
    /// интервалы).
    #[test]
    fn angle_gap_annular_wedge() {
        let tau = std::f32::consts::TAU;
        // Дуга [-π/2, 0] (12 → 3 часов): середина -π/4 — строго внутри
        assert!(
            angle_gap(
                -std::f32::consts::FRAC_PI_4,
                -std::f32::consts::FRAC_PI_2,
                0.0
            ) < 0.0
        );
        // Нормализованный эквивалент середины: 7π/4 в [0, 2π)
        assert!(near(
            angle_gap(7.0 * tau / 8.0, -std::f32::consts::FRAC_PI_2, 0.0),
            -std::f32::consts::FRAC_PI_4
        ));
        // Ровно на крае a1=0 — 0
        assert!(near(angle_gap(0.0, -std::f32::consts::FRAC_PI_2, 0.0), 0.0));
        // За концом a1=0 на 0.1 рад — расстояние 0.1
        assert!(near(angle_gap(0.1, -std::f32::consts::FRAC_PI_2, 0.0), 0.1));
        // До начала a0=-π/2 на 0.1 рад (wrap через 0/2π нет) — 0.1
        assert!(near(
            angle_gap(
                -std::f32::consts::FRAC_PI_2 - 0.1,
                -std::f32::consts::FRAC_PI_2,
                0.0
            ),
            0.1
        ));
        // Полный круг — всегда внутри
        assert!(angle_gap(3.3, 0.0, tau) < 0.0);
        // Развёрнутый интервал за 2π: [3π/2, 2π+π/2] — середина 0 (≡ 2π) внутри
        assert!(angle_gap(0.0, 1.5 * tau / 2.0, tau + std::f32::consts::FRAC_PI_2) < 0.0);
        // Wrap-around: дуга [357.5°, 362.5°] (через 0): 0° — внутри
        let gap_deg = 2.5_f32.to_radians();
        let a0 = tau - gap_deg; // 357.5°
        let a1 = tau + gap_deg; // 362.5° ≡ 2.5°
        assert!(angle_gap(0.0, a0, a1) < 0.0, "0° внутри дуги через wrap");
        assert!(angle_gap(tau - 0.01, a0, a1) < 0.0);
        assert!(angle_gap(0.02, a0, a1) < 0.0);
        // 180° — до ближайшего края 177.5° в радианах
        assert!(near(angle_gap(tau / 2.0, a0, a1), tau / 2.0 - gap_deg));
    }

    /// Нормализация угла в [0, TAU).
    #[test]
    fn norm_angle_wraps() {
        let tau = std::f32::consts::TAU;
        assert!(near(norm_angle(0.0), 0.0));
        assert!(near(norm_angle(-0.5), tau - 0.5));
        assert!(near(norm_angle(tau + 0.25), 0.25));
        assert!(near(norm_angle(3.0 * tau + 1.0), 1.0));
    }
}
