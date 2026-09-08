//! GPU-проход миникарты (T13-B, SPEC §6.1): загрузка `MinimapImage` в текстуру
//! и один квад в правом нижнем углу окна (screen-space, физические px).
//!
//! Зона T13-B: этот файл + `renderer.rs`. Паттерн GPU-части — `thumbs.rs`
//! (instanced quad + выборка текстуры; шейдер рядом в `shaders/`), но вместо
//! атласа — единственная текстура RGBA8 и один квад. Пайплайн рисуется
//! последним (поверх канваса и оверлеев): HUD — левый верхний угол, миникарта —
//! правый нижний, визуального конфликта нет.
//!
//! Чистая математика размещения квада покрыта юнит-тестами здесь (без GPU);
//! сам wgpu-пайплайн — по образцу thumbs.rs, компиляция на windows-таргете
//! обязательна (`cargo check -p canvas-render --target x86_64-pc-windows-msvc
//! --all-targets`).

// MINIMAP_W/Н и MINIMAP_MARGIN используются реализацией quad_rect (T13-B);
// импорт оставлен здесь, чтобы сигнатуры модуля были единым контрактом.
use crate::minimap::{MinimapImage, MINIMAP_H, MINIMAP_MARGIN, MINIMAP_W};

/// Допуск при сравнении окна с минимальным размером: деление phys/scale при
/// дробном scale_factor может дать 251.999… вместо 252 (окно 252 логических).
const SIZE_EPS: f32 = 1e-3;

/// Размещение квада миникарты, физические px: `[x0, y0, x1, y1]`.
///
/// * источник координат — верхний левый угол окна;
/// * логический размер `MINIMAP_W × MINIMAP_H`, отступ `MINIMAP_MARGIN` от
///   правого нижнего угла, пересчёт через `scale_factor`;
/// * окно меньше `(MINIMAP_W + 2·margin) × (MINIMAP_H + 2·margin)` логических
///   px → `None` (миникарта скрыта, TASKS T13 §7 плана).
pub fn quad_rect(window_phys_w: u32, window_phys_h: u32, scale_factor: f32) -> Option<[f32; 4]> {
    // Вырожденный scale (0, отрицательный, NaN, бесконечность) — корректного
    // соответствия физ↔лог нет, миникарта скрывается
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return None;
    }
    let logical = quad_rect_logical(
        window_phys_w as f32 / scale_factor,
        window_phys_h as f32 / scale_factor,
    )?;
    Some([
        logical[0] * scale_factor,
        logical[1] * scale_factor,
        logical[2] * scale_factor,
        logical[3] * scale_factor,
    ])
}

/// Прямоугольник миникарты в ЛОГИЧЕСКИХ px — для hit-test в приложении
/// (координаты курсора winit — логические). Тот же too-small-фильтр, что и
/// `quad_rect`.
pub fn quad_rect_logical(window_logical_w: f32, window_logical_h: f32) -> Option<[f32; 4]> {
    // Минимальное окно: квад с отступом с обеих сторон каждой оси
    let min_w = MINIMAP_W as f32 + 2.0 * MINIMAP_MARGIN;
    let min_h = MINIMAP_H as f32 + 2.0 * MINIMAP_MARGIN;
    if !window_logical_w.is_finite() || !window_logical_h.is_finite() {
        return None;
    }
    if window_logical_w + SIZE_EPS < min_w || window_logical_h + SIZE_EPS < min_h {
        return None;
    }
    let x1 = window_logical_w - MINIMAP_MARGIN;
    let y1 = window_logical_h - MINIMAP_MARGIN;
    Some([x1 - MINIMAP_W as f32, y1 - MINIMAP_H as f32, x1, y1])
}

/// Uniform квада миникарты: экранный прямоугольник + физический размер окна.
/// Layout — зеркало `QuadUniform` в shaders/minimap.wgsl (32 байта).
#[derive(Debug, Clone, Copy)]
struct QuadUniform {
    /// `[x0, y0, x1, y1]` в физических px от левого верхнего угла окна.
    rect: [f32; 4],
    /// Физический размер окна — ортографическая проекция (идиома thumbs).
    viewport: [f32; 2],
    _pad: [f32; 2],
}

impl QuadUniform {
    fn to_bytes(self) -> [u8; 32] {
        let floats = [
            self.rect[0],
            self.rect[1],
            self.rect[2],
            self.rect[3],
            self.viewport[0],
            self.viewport[1],
            self._pad[0],
            self._pad[1],
        ];
        let mut bytes = [0u8; 32];
        for (i, value) in floats.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
        bytes
    }
}

/// Загруженный кадр миникарты: текстура RGBA8 + bind group пайплайна.
/// Пересоздаётся при смене размера буфера (`set_minimap` в renderer.rs).
pub struct MinimapTexture {
    texture: wgpu::Texture,
    // view хранится по контракту T13-B: явное владение ресурсом рядом с
    // текстурой (bind group wgpu и сам удерживает view; поле не читается
    // после создания — подавление dead_code осознанное)
    #[allow(dead_code)]
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    /// Размер текстуры, px.
    size: [u32; 2],
}

impl MinimapTexture {
    /// Создать текстуру под кадр, bind group и загрузить пиксели.
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pipeline: &MinimapPipeline,
        image: &MinimapImage,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("minimap"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Rgba8Unorm (не Srgb): значения проходят в кадр без перекодировки,
            // как у атласа тамбнейлов
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("minimap"),
            layout: &pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pipeline.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
                },
            ],
        });
        let this = Self {
            texture,
            view,
            bind_group,
            size: [image.width, image.height],
        };
        this.write_pixels(queue, image);
        this
    }

    /// Перезаписать содержимое текстуры (размер совпадает с кадром).
    fn write_pixels(&self, queue: &wgpu::Queue, image: &MinimapImage) {
        // write_texture в wgpu 22 идёт через staging-буфер и переупаковывает
        // строки сам (проверено по wgpu-core 22.1: need_copy_aligned_rows =
        // false), поэтому bytes_per_row = width·4 без выравнивания на 256 —
        // в отличие от copy_buffer_to_texture
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &image.rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(image.width * 4),
                rows_per_image: Some(image.height),
            },
            wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// GPU-пайплайн миникарты: один текстурированный квад поверх всей сцены.
/// Вершины разворачиваются из `vertex_index` по uniform-прямоугольнику —
/// вершинного/инстансного буфера нет (один инстанс на кадр).
pub struct MinimapPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl MinimapPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("minimap"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/minimap.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("minimap quad"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Linear: буфер растеризуется в физических px, равных размеру квада —
        // сэмплинг 1:1, выбор фильтра не критичен; linear сглаживает
        // полупиксельные смещения краёв при дробном scale_factor
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("minimap"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("minimap"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("minimap"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("minimap"),
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
                    // Straight alpha как у thumbs: полупрозрачный фон миникарты
                    // смешивается с кадром blending-этапом, альфой surface
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

        Self {
            pipeline,
            uniform_buffer,
            sampler,
            bind_group_layout,
        }
    }

    /// Загрузить кадр миникарты: размер не изменился — перезапись пикселей
    /// существующей текстуры; изменился — новая текстура + bind group
    /// (старая освобождается). Ошибок wgpu здесь нет: write_texture не падает
    /// при корректном layout (валидация длины буфера — на стороне Renderer).
    pub fn upload(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        current: Option<MinimapTexture>,
        image: &MinimapImage,
    ) -> MinimapTexture {
        if let Some(texture) = current {
            if texture.size == [image.width, image.height] {
                texture.write_pixels(queue, image);
                return texture;
            }
        }
        MinimapTexture::new(device, queue, self, image)
    }

    /// Обновить uniform квада: прямоугольник в физических px + размер окна.
    /// Вызывается до кодирования кадра: write_buffer упорядочен раньше
    /// команд кодировщика, отправляемых в submit.
    pub fn update_quad(&self, queue: &wgpu::Queue, rect: [f32; 4], viewport: [f32; 2]) {
        let uniform = QuadUniform {
            rect,
            viewport,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());
    }

    /// Нарисовать квад миникарты в активном render pass — последним,
    /// после карточек, тамбнейлов и всех текст-групп (см. Renderer::render).
    pub fn draw<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        texture: &'pass MinimapTexture,
    ) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &texture.bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Классическое окно 1280×720, scale 1.0: правый нижний угол с отступом
    /// 16 и размером 220×140 → [1044, 564, 1264, 704].
    #[test]
    fn quad_rect_default_dpi() {
        let rect = quad_rect(1280, 720, 1.0).expect("окно достаточно велико");
        assert_eq!(rect, [1044.0, 564.0, 1264.0, 704.0]);
    }

    /// Scale 2.0 (физ. 2560×1440 = лог. 1280×720): тот же прямоугольник,
    /// пересчитанный в физические px.
    #[test]
    fn quad_rect_hidpi() {
        let rect = quad_rect(2560, 1440, 2.0).expect("окно достаточно велико");
        assert_eq!(rect, [2088.0, 1128.0, 2528.0, 1408.0]);
    }

    /// Пропорциональность: quad_rect по физическим px = логический
    /// прямоугольник × scale (метрики не плывут при дробном DPI).
    #[test]
    fn quad_rect_scales_with_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let (logical_w, logical_h) = (1280.0, 720.0);
            let physical = quad_rect(
                (logical_w * scale) as u32,
                (logical_h * scale) as u32,
                scale,
            )
            .expect("окно достаточно велико");
            let logical = quad_rect_logical(logical_w, logical_h).expect("окно велико");
            assert_eq!(
                physical,
                [
                    logical[0] * scale,
                    logical[1] * scale,
                    logical[2] * scale,
                    logical[3] * scale,
                ],
                "scale {scale}"
            );
        }
    }

    /// Окно 250×150 логических — меньше (220+32)×(140+32): миникарта скрыта.
    #[test]
    fn quad_rect_too_small() {
        assert!(quad_rect(250, 150, 1.0).is_none());
        assert!(quad_rect(250, 150, 2.0).is_none());
    }

    /// Ровно 252×172 логических — влезает (квад с отступом 16 с обеих сторон);
    /// 251×172 и 252×171 — нет.
    #[test]
    fn quad_rect_min_boundary() {
        let rect = quad_rect(252, 172, 1.0).expect("ровно минимальное окно");
        assert_eq!(rect, [16.0, 16.0, 236.0, 156.0]);
        assert!(quad_rect(251, 172, 1.0).is_none());
        assert!(quad_rect(252, 171, 1.0).is_none());
    }

    /// Вырожденный scale (0, отрицательный, NaN) — None без деления на ноль.
    #[test]
    fn quad_rect_bad_scale() {
        assert!(quad_rect(1280, 720, 0.0).is_none());
        assert!(quad_rect(1280, 720, -1.0).is_none());
        assert!(quad_rect(1280, 720, f32::NAN).is_none());
    }

    /// Логический вариант: та же раскладка в логических px (hit-test).
    #[test]
    fn quad_rect_logical_layout() {
        let rect = quad_rect_logical(1280.0, 720.0).expect("окно достаточно велико");
        assert_eq!(rect, [1044.0, 564.0, 1264.0, 704.0]);
    }

    /// Логический too-small: 250×150 и 251×172 — None.
    #[test]
    fn quad_rect_logical_too_small() {
        assert!(quad_rect_logical(250.0, 150.0).is_none());
        assert!(quad_rect_logical(251.0, 172.0).is_none());
    }

    /// Логическая граница ровно 252×172 — Some с отступом 16 от краёв.
    #[test]
    fn quad_rect_logical_min_boundary() {
        let rect = quad_rect_logical(252.0, 172.0).expect("ровно минимальное окно");
        assert_eq!(rect, [16.0, 16.0, 236.0, 156.0]);
    }
}
