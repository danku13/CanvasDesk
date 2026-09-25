//! FR-ICONS: screen-space wgpu-пайплайн SVG-иконок.
//!
//! Атлас собирается один раз при init из растеризованных байт (`icon_data.rs`):
//! 4 набора × 16 иконок × 32×32 px (bootstrap 24×24 дополнен до 32×32)
//! = 16×4 сетка = 512×128 px атлас. Все 4 набора в одном атласе — переключение
//! набора не требует ребинда bind-группы (выбор набора = выбор UV в атласе).
//!
//! Инстанс = pos/size в экранных px + UV в атласе + tint (RGBA). Shader —
//! screen-space (без camera uniform), рисуется в общем render pass ПОВЕРХ
//! карточек/полос/текстов (как виджет-снапшоты и направляющие снапа).
//!
//! Tint — цвет слота `icon` темы (или `text` для инлайн-иконок). Атлас хранит
//! белый силуэт (rgb=1, alpha — форма) — tint в шейдере даёт монохромный цвет.
//!
//! wasm-gate (ADR-0011): байты вшиты в бинарник, ноль runtime FS-доступа.

use crate::icon_data::{icon_rgba, icon_set_px, ICON_NAMES, ICON_SETS};

/// Сторона ячейки иконки в атласе, px (максимальный размер растра — 32 для
/// lucide/material/feather; bootstrap 24 дополнен до 32).
pub const ICON_CELL_PX: u32 = 32;
/// Иконок в строке атласа (= число имён).
pub const ICONS_PER_ROW: u32 = ICON_NAMES.len() as u32;
/// Сторона атласа: ICONS_PER_ROW × ICON_SETS.len() ячеек ICON_CELL_PX².
pub const ATLAS_W: u32 = ICONS_PER_ROW * ICON_CELL_PX; // 13 * 32 = 416
pub const ATLAS_H: u32 = ICON_SETS.len() as u32 * ICON_CELL_PX; // 4 * 32 = 128

/// Инстанс иконки для GPU (layout — attributes в icons.wgsl).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconInstance {
    /// Левый верх квада, физ. px (screen-space).
    pub pos: [f32; 2],
    /// Размер квада, физ. px (квадратный, но храним как vec2 для общности).
    pub size: [f32; 2],
    /// UV левого верха в атласе.
    pub uv_min: [f32; 2],
    /// UV правого низа в атласе.
    pub uv_max: [f32; 2],
    /// RGBA tint (премультипликации нет; alpha умножается на alpha силуэта).
    pub tint: [f32; 4],
}

impl IconInstance {
    pub(crate) const FLOATS: usize = 12; // 2+2+2+2+4

    pub(crate) fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
            0 => Float32x2, // pos
            1 => Float32x2, // size
            2 => Float32x2, // uv_min
            3 => Float32x2, // uv_max
            4 => Float32x4, // tint
        ];
        wgpu::VertexBufferLayout {
            array_stride: (Self::FLOATS * 4) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &ATTRS,
        }
    }

    pub(crate) fn write_to(&self, out: &mut Vec<u8>) {
        for group in [
            &self.pos[..],
            &self.size[..],
            &self.uv_min[..],
            &self.uv_max[..],
            &self.tint[..],
        ] {
            for value in group {
                out.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
}

/// UV-координаты иконки `(set, name)` в атласе: левый верх + правый низ.
/// Возвращает `None` для неизвестной пары — фолбэк на глиф (в app.rs).
///
/// Чистая функция (без GPU) — тестируется без устройства.
pub fn icon_uv(set: &str, name: &str) -> Option<([f32; 2], [f32; 2])> {
    let set_idx = ICON_SETS.iter().position(|s| *s == set)?;
    let name_idx = ICON_NAMES.iter().position(|n| *n == name)?;
    let cell_w = ICON_CELL_PX as f32 / ATLAS_W as f32;
    let cell_h = ICON_CELL_PX as f32 / ATLAS_H as f32;
    let min = [(name_idx as f32) * cell_w, (set_idx as f32) * cell_h];
    let max = [min[0] + cell_w, min[1] + cell_h];
    Some((min, max))
}

/// Упаковка viewport-юниформа иконок: `[f32; 2]` → 16 байт
/// (`vec2<f32>` на смещении 0 + 2 пад-флоата — раскладка `ViewportUniform`
/// в shaders/icons.wgsl).
///
/// Чистая функция (без GPU) — тестируется без устройства. История: здесь
/// была инлайн-упаковка с 8-байтными срезами под 4-байтный `to_ne_bytes()`
/// — безусловная паника `copy_from_slice` на ПЕРВОМ кадре любого рантайма
/// (wasm — чёрный экран после трапа; натив не поймала CI без GPU).
pub fn pack_viewport_uniform(viewport: [f32; 2]) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&viewport[0].to_ne_bytes());
    bytes[4..8].copy_from_slice(&viewport[1].to_ne_bytes());
    bytes // 8..16 — пад, остаётся нулевым
}

/// GPU-пайплайн иконок: атлас-текстура + instanced draw (screen-space).
pub struct IconPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    atlas_texture: wgpu::Texture,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl IconPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("icons"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/icons.wgsl").into()),
        });

        // Атлас: 416×128 Rgba8Unorm, заполняется один раз при init.
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("icon atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_W,
                height: ATLAS_H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("icon atlas"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("icons viewport"),
            size: 16, // vec2 + 2 pad = 16 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("icons"),
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
            label: Some("icons"),
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
            label: Some("icons"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("icons"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[IconInstance::vertex_buffer_layout()],
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

        let instance_capacity = 256;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("icons instances"),
            size: (IconInstance::FLOATS * 4 * instance_capacity) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            atlas_texture,
            instance_buffer,
            instance_capacity,
        }
    }

    /// Загрузить атлас из вшитых байт. Вызывается один раз при init рендера
    /// (после `new()`). Загружает все 4 набора × 16 иконок в общий атлас.
    pub fn upload_atlas(&self, queue: &wgpu::Queue) {
        for (set_idx, set_name) in ICON_SETS.iter().enumerate() {
            for (name_idx, icon_name) in ICON_NAMES.iter().enumerate() {
                let Some(rgba) = icon_rgba(set_name, icon_name) else {
                    tracing::warn!(
                        set = set_name,
                        icon = icon_name,
                        "иконка отсутствует в реестре"
                    );
                    continue;
                };
                let px = icon_set_px(set_name) as usize;
                debug_assert_eq!(
                    rgba.len(),
                    px * px * 4,
                    "размер растра не совпадает с заявленным"
                );
                // Bootstrap (24×24) дополняем до 32×32 прозрачными полями
                // по центру; остальные наборы уже 32×32.
                let target_px = ICON_CELL_PX as usize;
                let (data, bytes_per_row) = if px == target_px {
                    (rgba.to_vec(), target_px * 4)
                } else {
                    let mut padded = vec![0u8; target_px * target_px * 4];
                    let offset = (target_px - px) / 2;
                    for y in 0..px {
                        let src_row = &rgba[y * px * 4..(y + 1) * px * 4];
                        let dst_y = y + offset;
                        let dst_x = offset;
                        let dst_start = (dst_y * target_px + dst_x) * 4;
                        padded[dst_start..dst_start + src_row.len()].copy_from_slice(src_row);
                    }
                    (padded, target_px * 4)
                };
                let origin_x = (name_idx as u32) * ICON_CELL_PX;
                let origin_y = (set_idx as u32) * ICON_CELL_PX;
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
                    &data,
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row as u32),
                        rows_per_image: Some(ICON_CELL_PX),
                    },
                    wgpu::Extent3d {
                        width: ICON_CELL_PX,
                        height: ICON_CELL_PX,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
    }

    /// Загрузить viewport и инстансы кадра; вернуть число инстансов.
    /// Инстансы — в экранных физических px (как квады полос kit_ui).
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        viewport: [f32; 2],
        instances: &[IconInstance],
    ) -> u32 {
        let uniform_bytes = pack_viewport_uniform(viewport);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform_bytes);

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("icons instances"),
                size: (IconInstance::FLOATS * 4 * self.instance_capacity) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(instances.len() * IconInstance::FLOATS * 4);
        for instance in instances {
            instance.write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        instances.len() as u32
    }

    /// Нарисовать `count` иконок в активном render pass (поверх всех полос).
    pub fn draw<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>, count: u32) {
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.set_vertex_buffer(
            1,
            self.instance_buffer.slice(..), // уголки квада берём из vertex_index
        );
        // 6 вершин на инстанс (два треугольника), count инстансов
        pass.draw(0..6, 0..count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UV-координаты первой иконки первого набора — верхний левый угол атласа.
    #[test]
    fn uv_first_icon_is_origin() {
        let (min, max) = icon_uv("lucide", "close").expect("lucide/close есть в реестре");
        assert_eq!(min, [0.0, 0.0]);
        let cell_w = ICON_CELL_PX as f32 / ATLAS_W as f32;
        let cell_h = ICON_CELL_PX as f32 / ATLAS_H as f32;
        assert!((max[0] - cell_w).abs() < 1e-6);
        assert!((max[1] - cell_h).abs() < 1e-6);
    }

    /// UV-координаты иконки в конце атласа — правый нижний угол.
    #[test]
    fn uv_last_icon_is_corner() {
        let (min, max) = icon_uv("bootstrap", "chevron_right")
            .expect("bootstrap/chevron_right есть в реестре");
        let cell_w = ICON_CELL_PX as f32 / ATLAS_W as f32;
        let cell_h = ICON_CELL_PX as f32 / ATLAS_H as f32;
        // Последняя колонка + последняя строка
        let expected_min_x = (ICONS_PER_ROW - 1) as f32 * cell_w;
        let expected_min_y = (ICON_SETS.len() - 1) as f32 * cell_h;
        assert!((min[0] - expected_min_x).abs() < 1e-6, "min.x: {min:?}");
        assert!((min[1] - expected_min_y).abs() < 1e-6, "min.y: {min:?}");
        assert!((max[0] - 1.0).abs() < 1e-6, "max.x: {max:?}");
        assert!((max[1] - 1.0).abs() < 1e-6, "max.y: {max:?}");
    }

    /// Неизвестная пара — None (фолбэк на глиф в app.rs).
    #[test]
    fn unknown_icon_returns_none() {
        assert!(icon_uv("unknown", "close").is_none());
        assert!(icon_uv("lucide", "unknown").is_none());
        assert!(icon_uv("unknown", "unknown").is_none());
    }

    /// Все пары (set, name) из реестров имеют UV.
    #[test]
    fn all_registered_icons_have_uv() {
        for set in ICON_SETS {
            for name in ICON_NAMES {
                assert!(icon_uv(set, name).is_some(), "{set}/{name} нет UV");
            }
        }
    }

    /// Инстанс сериализуется в 12 float (48 байт).
    #[test]
    fn instance_writes_12_floats() {
        let inst = IconInstance {
            pos: [1.0, 2.0],
            size: [3.0, 4.0],
            uv_min: [0.1, 0.2],
            uv_max: [0.3, 0.4],
            tint: [0.5, 0.6, 0.7, 0.8],
        };
        let mut bytes = Vec::new();
        inst.write_to(&mut bytes);
        assert_eq!(bytes.len(), 48, "12 float × 4 bytes");
    }

    /// Атлас вмещает все 4 набора × 16 иконок = 64 ячейки 32×32.
    #[test]
    fn atlas_size_matches_repositories() {
        let total_cells = ICONS_PER_ROW * ICON_SETS.len() as u32;
        assert_eq!(total_cells, 64, "4 набора × 16 иконок");
        assert_eq!(ATLAS_W, 16 * 32);
        assert_eq!(ATLAS_H, 4 * 32);
    }

    /// Регрессия wasm-чёрного экрана: упаковка юниформа — vec2 на смещении 0
    /// (два f32 по 4 байта), пад 8..16 нулевой; раскладка ViewportUniform
    /// в shaders/icons.wgsl. Прежняя инлайн-версия писала 4 байта в
    /// 8-байтный срез — паника на первом кадре.
    #[test]
    fn viewport_uniform_packs_vec2_plus_pad() {
        let bytes = pack_viewport_uniform([1280.0, 800.0]);
        assert_eq!(bytes.len(), 16, "vec2 + 2 pad = 16 байт");
        let w = f32::from_ne_bytes(bytes[0..4].try_into().unwrap());
        let h = f32::from_ne_bytes(bytes[4..8].try_into().unwrap());
        assert_eq!((w, h), (1280.0, 800.0), "viewport в первых 8 байтах");
        assert!(bytes[8..16].iter().all(|&b| b == 0), "пад 8..16 нулевой");
    }
}
