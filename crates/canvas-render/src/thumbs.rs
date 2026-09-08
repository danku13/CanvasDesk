//! Пайплайн тамбнейлов (T6): instanced quads с выборкой из атласа 2048².
//!
//! Карточка сначала рисуется с иконкой-заглушкой; когда тамбнейл готов
//! (канал из ThumbService → `insert`), поверх тела карточки рисуется
//! текстурированный квад. LOD: тамбнейлы — только при zoom ≥ 0.25 (SPEC §6.2).

use canvas_core::{Canvas, Thumbnail};

use crate::atlas::{Slot, ThumbSlots, ATLAS_SIZE, CELL_SIZE};
use crate::camera::Camera;
use crate::cards::{CameraUniform, HEADER_HEIGHT};

/// LOD-порог тамбнейлов по SPEC §6.2: ниже — только прямоугольник + иконка.
pub const THUMB_MIN_ZOOM: f32 = 0.25;

/// Инстанс тамбнейла для GPU (layout — attributes в thumbs.wgsl).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThumbInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

impl ThumbInstance {
    const FLOATS: usize = 8;

    fn write_to(&self, out: &mut Vec<u8>) {
        for group in [
            &self.pos[..],
            &self.size[..],
            &self.uv_min[..],
            &self.uv_max[..],
        ] {
            for value in group {
                out.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
}

/// Собрать инстансы кадра: только file-ноды с тамбнейлом в атласе,
/// при zoom ≥ THUMB_MIN_ZOOM. Обращение к слоту обновляет его LRU-тик.
pub fn build_thumb_instances(
    canvas: &Canvas,
    indices: &[usize],
    slots: &mut ThumbSlots,
    zoom: f32,
) -> Vec<ThumbInstance> {
    if zoom < THUMB_MIN_ZOOM {
        return Vec::new();
    }
    indices
        .iter()
        .filter_map(|&index| {
            let node = canvas.nodes.get(index)?;
            node.file.as_ref()?;
            let slot = slots.touch(index)?;
            Some(fit_instance(node.x, node.y, node.width, node.height, slot))
        })
        .collect()
}

/// Вписать тамбнейл в тело карточки (rect минус заголовок) с сохранением
/// пропорций и центрированием. Чистая функция — тестируется без GPU.
fn fit_instance(x: f32, y: f32, width: f32, height: f32, slot: Slot) -> ThumbInstance {
    let body = [width, (height - HEADER_HEIGHT).max(0.0)];
    let (tw, th) = (slot.width as f32, slot.height as f32);
    let scale = if tw > 0.0 && th > 0.0 {
        (body[0] / tw).min(body[1] / th)
    } else {
        0.0
    };
    let size = [tw * scale, th * scale];
    let pos = [
        x + (body[0] - size[0]) * 0.5,
        y + HEADER_HEIGHT + (body[1] - size[1]) * 0.5,
    ];
    let (uv_min, uv_max) = slot.uv();
    ThumbInstance {
        pos,
        size,
        uv_min,
        uv_max,
    }
}

/// GPU-пайплайн тамбнейлов: атлас-текстура + instanced draw.
pub struct ThumbsPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    atlas_texture: wgpu::Texture,
    /// Размещение ячеек (чистая логика, см. atlas.rs).
    slots: ThumbSlots,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl ThumbsPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("thumbs"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/thumbs.wgsl").into()),
        });

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("thumb atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Rgba8Unorm (не Srgb): значения проходят в кадр без перекодировки,
            // как и цвета карточек
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("thumb atlas"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("thumbs camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("thumbs"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("thumbs"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("thumbs"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: (ThumbInstance::FLOATS * 4) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x2, // pos
                1 => Float32x2, // size
                2 => Float32x2, // uv_min
                3 => Float32x2, // uv_max
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("thumbs"),
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

        let instance_capacity = 64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("thumbs instances"),
            size: (ThumbInstance::FLOATS * 4 * instance_capacity) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            atlas_texture,
            slots: ThumbSlots::new(),
            instance_buffer,
            instance_capacity,
        }
    }

    /// Загрузить тамбнейл ноды в атлас (LRU-вытеснение при переполнении).
    /// Растр больше ячейки отбрасывается — сервис обязан вписать в CELL_SIZE.
    pub fn insert(&mut self, queue: &wgpu::Queue, node: usize, thumb: &Thumbnail) {
        if thumb.width > CELL_SIZE || thumb.height > CELL_SIZE || thumb.width == 0 {
            tracing::warn!(
                node,
                w = thumb.width,
                h = thumb.height,
                "тамбнейл не влезает в ячейку атласа — пропущен"
            );
            return;
        }
        let slot = self.slots.alloc(node, thumb.width, thumb.height).0;
        let [origin_x, origin_y] = slot.origin_px();
        // wgpu требует bytes_per_row кратным 256 — выравниваем строки в копию
        let row_bytes = thumb.width as usize * 4;
        let aligned = row_bytes.div_ceil(256) * 256;
        let data;
        let bytes_per_row = if aligned == row_bytes {
            data = None;
            Some(row_bytes as u32)
        } else {
            let mut padded = vec![0u8; aligned * thumb.height as usize];
            for y in 0..thumb.height as usize {
                padded[y * aligned..y * aligned + row_bytes]
                    .copy_from_slice(&thumb.rgba[y * row_bytes..(y + 1) * row_bytes]);
            }
            data = Some(padded);
            Some(aligned as u32)
        };
        let source = data.as_deref().unwrap_or(&thumb.rgba);
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: origin_x,
                    y: origin_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            source,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row,
                rows_per_image: Some(thumb.height),
            },
            wgpu::Extent3d {
                width: thumb.width,
                height: thumb.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Есть ли у ноды тамбнейл в атласе (дедупликация заказов на стороне app).
    pub fn contains(&self, node: usize) -> bool {
        self.slots.contains(node)
    }

    /// Число тамбнейлов в атласе (HUD).
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Атлас пуст (ни одного тамбнейла).
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Полный сброс слотов (T8): индексы нод сдвинулись после удаления;
    /// тамбнейлы перезакажутся лениво из ThumbService/SQLite.
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Доступ к размещению для сборки инстансов кадра.
    pub fn slots_mut(&mut self) -> &mut ThumbSlots {
        &mut self.slots
    }

    /// Загрузить камеру и инстансы кадра; вернуть число инстансов.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport: [f32; 2],
        scale_factor: f32,
        instances: &[ThumbInstance],
    ) -> u32 {
        let uniform = CameraUniform::new(camera, viewport, scale_factor);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("thumbs instances"),
                size: (ThumbInstance::FLOATS * 4 * self.instance_capacity) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(instances.len() * ThumbInstance::FLOATS * 4);
        for instance in instances {
            instance.write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        instances.len() as u32
    }

    /// Нарисовать `count` тамбнейлов в активном render pass.
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
    use canvas_core::Node;

    fn slot(width: u32, height: u32) -> Slot {
        let mut slots = ThumbSlots::new();
        slots.alloc(0, width, height).0
    }

    /// Fit: квадратный тамбнейл в широкой карточке — по высоте тела, по центру.
    #[test]
    fn fit_centers_and_preserves_aspect() {
        // Карточка 200×100, тело 200×72, тамбнейл 256×256 → 72×72 по центру
        let inst = fit_instance(10.0, 20.0, 200.0, 100.0, slot(256, 256));
        assert!((inst.size[0] - 72.0).abs() < 1e-4);
        assert!((inst.size[1] - 72.0).abs() < 1e-4);
        assert!((inst.pos[0] - (10.0 + 64.0)).abs() < 1e-4);
        assert!((inst.pos[1] - (20.0 + HEADER_HEIGHT)).abs() < 1e-4);
    }

    /// Fit: широкий тамбнейл ограничен шириной тела.
    #[test]
    fn fit_wide_thumbnail() {
        // Карточка 100×300, тело 100×272, тамбнейл 256×128 → 100×50
        let inst = fit_instance(0.0, 0.0, 100.0, 300.0, slot(256, 128));
        assert!((inst.size[0] - 100.0).abs() < 1e-4);
        assert!((inst.size[1] - 50.0).abs() < 1e-4);
        // Центрирование по вертикали тела
        assert!((inst.pos[1] - (HEADER_HEIGHT + (272.0 - 50.0) * 0.5)).abs() < 1e-4);
    }

    /// Нулевая высота тела не паникует и даёт нулевой размер.
    #[test]
    fn fit_zero_body() {
        let inst = fit_instance(0.0, 0.0, 100.0, HEADER_HEIGHT, slot(256, 256));
        assert_eq!(inst.size, [0.0, 0.0]);
    }

    /// Сборка инстансов: только file-ноды со слотом, LOD-фильтр по zoom.
    #[test]
    fn build_filters_nodes() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("b", "C:/b.png", 200.0, 0.0, 100.0, 100.0));
        canvas.nodes.push(Node::text("c", "t", 400.0, 0.0));

        let mut slots = ThumbSlots::new();
        slots.alloc(0, 256, 256); // тамбнейл есть только у ноды 0

        // zoom 1.0: один инстанс (нода 1 без тамбнейла, нода 2 — текстовая)
        let insts = build_thumb_instances(&canvas, &[0, 1, 2], &mut slots, 1.0);
        assert_eq!(insts.len(), 1);
        // zoom 0.1 < 0.25: LOD — без тамбнейлов
        assert!(build_thumb_instances(&canvas, &[0], &mut slots, 0.1).is_empty());
        // touch обновил LRU — повторный alloc не вытеснит ноду 0 первой
        assert!(slots.contains(0));
    }

    /// Страйд сериализации совпадает с vertex layout.
    #[test]
    fn instance_stride_matches_serialization() {
        let inst = ThumbInstance {
            pos: [1.0, 2.0],
            size: [3.0, 4.0],
            uv_min: [0.1, 0.2],
            uv_max: [0.9, 0.8],
        };
        let mut bytes = Vec::new();
        inst.write_to(&mut bytes);
        assert_eq!(bytes.len(), ThumbInstance::FLOATS * 4);
    }
}
