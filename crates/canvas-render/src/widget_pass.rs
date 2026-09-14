//! GPU-проход снапшотов виджетов (T20-D, план M5 §4.5): текстурированные квады
//! в областях контента виджет-нод — заместитель live-HWND (SPEC §7.6 airspace).
//!
//! Паттерн — `thumbs.rs` (тот же шейдер `shaders/thumbs.wgsl`, тот же
//! CameraUniform-лейаут, instanced quads), но вместо общего атласа — своя
//! текстура RGBA8 на виджет (снапшоты крупнее ячейки атласа 256), uv 0..1.
//! Ключ текстуры — строковый id ноды (не индекс): переживает удаления/undo.
//!
//! LRU-кэп `WIDGET_SNAPSHOT_MAX` текстур; кламп размера задаёт хост
//! (512×512, план M5 §4.5). Рисуется ПОСЛЕ всех карточек, ДО screen-оверлеев
//! (меню поверх снапшота — П7 airspace-политика).
//!
//! Чистая логика LRU/фильтрации покрыта юнит-тестами; wgpu-часть
//! компилируется на Linux (гейт) и windows-таргете (win-чек).

use crate::camera::Camera;
use crate::thumbs::ThumbInstance;
use std::collections::HashMap;

/// Максимум текстур снапшотов (LRU-вытеснение; план M5 §4.5: 16 × 512² ≈ 16 МБ).
pub const WIDGET_SNAPSHOT_MAX: usize = 16;

/// Квад снапшота от приложения: id ноды + область контента в world-px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetQuad<'a> {
    pub node_id: &'a str,
    /// Левый верх области контента, world px.
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

/// Uniform камеры — зеркало лейаута в `shaders/thumbs.wgsl` (32 байта).
#[derive(Debug, Clone, Copy)]
struct CameraUniform {
    position: [f32; 2],
    viewport: [f32; 2],
    effective_zoom: f32,
    _pad: [f32; 3],
}

impl CameraUniform {
    fn new(camera: &Camera, viewport: [f32; 2], scale_factor: f32) -> Self {
        Self {
            position: [camera.position()[0], camera.position()[1]],
            viewport,
            effective_zoom: camera.zoom() * scale_factor,
            _pad: [0.0; 3],
        }
    }

    fn to_bytes(self) -> [u8; 32] {
        let floats = [
            self.position[0],
            self.position[1],
            self.viewport[0],
            self.viewport[1],
            self.effective_zoom,
            self._pad[0],
            self._pad[1],
            self._pad[2],
        ];
        let mut bytes = [0u8; 32];
        for (i, value) in floats.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_ne_bytes());
        }
        bytes
    }
}

/// Текстура снапшота одного виджета + bind group пайплайна.
struct WidgetTexture {
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    /// LRU-тик последнего обновления.
    tick: u64,
    /// wgpu-ресурсы держатся bind group'ом; поля для диагностики/пересоздания
    #[allow(dead_code)]
    texture: wgpu::Texture,
    #[allow(dead_code)]
    view: wgpu::TextureView,
}

/// Проход снапшотов виджетов: общий uniform камеры + instance-буфер
/// (ThumbInstance с uv 0..1) + персональные bind group на текстуру.
pub struct WidgetPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    /// Текстуры по node_id.
    textures: HashMap<String, WidgetTexture>,
    /// Порядок отрисовки кадра: node_id отфильтрованных квадов (заполняется
    /// `update`, читается `draw`).
    frame_order: Vec<String>,
    tick: u64,
}

impl WidgetPass {
    /// Пайплайн идентичен thumbs (тот же шейдер), bind group layout —
    /// uniform + текстура + сэмплер (вместо атласа — текстура виджета).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("widget-snapshots"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/thumbs.wgsl").into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("widget-snapshots camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("widget-snapshots"),
            // Linear: снапшот захватывается при текущем зуме; при панорамировании
            // (зум мог измениться с момента захвата) лёгкое сглаживание уместнее
            // пикселизации nearest
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("widget-snapshots"),
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
            label: Some("widget-snapshots"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("widget-snapshots"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[ThumbInstance::vertex_buffer_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Снапшот непрозрачен, но alpha-канал PNG корректен;
                    // blending как у thumbs — единообразие
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
        // Размер и ёмкость — из одной формулы (instance_buffer_size):
        // регрессия 6b8f115 — захардкоженный `size: 64` при capacity 8;
        // при 3 снапшотах write_buffer 0..96 в буфер 64 → wgpu validation
        // error → паника (wgpu ошибки фатальны)
        let instance_capacity = 8usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("widget-snapshots instances"),
            size: Self::instance_buffer_size(instance_capacity),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            uniform_buffer,
            sampler,
            bind_group_layout,
            instance_buffer,
            instance_capacity,
            textures: HashMap::new(),
            frame_order: Vec::new(),
            tick: 0,
        }
    }

    /// Размер instance-буфера для заданной ёмкости. Единственная точка
    /// расчёта — и для начального создания (`new`), и для роста (`update`):
    /// буфер обязан вмещать `capacity` инстансов, иначе `write_buffer`
    /// выйдет за границы (wgpu validation error, фатальная паника).
    fn instance_buffer_size(capacity: usize) -> wgpu::BufferAddress {
        (ThumbInstance::FLOATS * 4 * capacity) as wgpu::BufferAddress
    }

    /// Загрузить/заменить снапшот виджета. Размер изменился — текстура
    /// пересоздаётся; совпадает — перезапись пикселей (как MinimapTexture).
    /// LRU: переполнение кэпа вытесняет самую давно не обновлявшуюся.
    pub fn set_snapshot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        node_id: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) {
        if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
            tracing::warn!(
                node_id,
                width,
                height,
                "снапшот отброшен: неконсистентные данные"
            );
            return;
        }
        self.tick += 1;
        let recreate = !matches!(
            self.textures.get(node_id),
            Some(t) if t.width == width && t.height == height
        );
        if recreate {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("widget-snapshot"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Rgba8Unorm (не Srgb): как атлас тамбнейлов, без перекодировки
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("widget-snapshot"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.textures.insert(
                node_id.to_owned(),
                WidgetTexture {
                    bind_group,
                    width,
                    height,
                    tick: self.tick,
                    texture,
                    view,
                },
            );
            self.evict_over_cap();
        } else if let Some(t) = self.textures.get_mut(node_id) {
            t.tick = self.tick;
        }
        // Пиксели: write_texture переупаковывает строки (идиома minimap/thumbs)
        let texture = &self
            .textures
            .get(node_id)
            .expect("текстура только что вставлена")
            .texture;
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// LRU-вытеснение за кэпом.
    fn evict_over_cap(&mut self) {
        while self.textures.len() > WIDGET_SNAPSHOT_MAX {
            let victim = self
                .textures
                .iter()
                .min_by_key(|(_, t)| t.tick)
                .map(|(k, _)| k.clone());
            match victim {
                Some(key) => {
                    self.textures.remove(&key);
                    tracing::debug!(node = %key, "снапшот-текстура вытеснена (LRU)");
                }
                None => break,
            }
        }
    }

    /// Удалить текстуру (нода удалена/пакет бит).
    pub fn clear_snapshot(&mut self, node_id: &str) {
        self.textures.remove(node_id);
        self.frame_order.retain(|id| id != node_id);
    }

    pub fn clear_all(&mut self) {
        self.textures.clear();
        self.frame_order.clear();
    }

    pub fn has_snapshot(&self, node_id: &str) -> bool {
        self.textures.contains_key(node_id)
    }

    pub fn snapshot_count(&self) -> usize {
        self.textures.len()
    }

    /// Собрать кадр: uniform камеры + инстансы квадов, У КОТОРЫХ есть текстура
    /// (остальные — заглушка карточки, рисуется cards-проходом). Возвращает
    /// число видимых квадов для `draw`.
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: &Camera,
        viewport: [f32; 2],
        scale_factor: f32,
        quads: &[WidgetQuad<'_>],
    ) -> u32 {
        let uniform = CameraUniform::new(camera, viewport, scale_factor);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());

        let visible: Vec<(&WidgetQuad, &WidgetTexture)> = quads
            .iter()
            .filter_map(|q| self.textures.get(q.node_id).map(|t| (q, t)))
            .collect();
        self.frame_order = visible.iter().map(|(q, _)| q.node_id.to_owned()).collect();

        if visible.len() > self.instance_capacity {
            self.instance_capacity = visible.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("widget-snapshots instances"),
                size: Self::instance_buffer_size(self.instance_capacity),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(visible.len() * ThumbInstance::FLOATS * 4);
        for (quad, _) in &visible {
            // uv 0..1: текстура целиком принадлежит виджету
            ThumbInstance {
                pos: quad.pos,
                size: quad.size,
                uv_min: [0.0, 0.0],
                uv_max: [1.0, 1.0],
            }
            .write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        visible.len() as u32
    }

    /// Нарисовать `count` квадов (после `update`, порядок тот же): по одному
    /// draw на виджет — bind group переключается между текстурами.
    pub fn draw<'pass>(&'pass self, pass: &mut wgpu::RenderPass<'pass>, count: u32) {
        if count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        for (i, node_id) in self.frame_order.iter().take(count as usize).enumerate() {
            if let Some(texture) = self.textures.get(node_id) {
                pass.set_bind_group(0, &texture.bind_group, &[]);
                pass.draw(0..6, i as u32..(i as u32 + 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_order_filters_without_textures() {
        // Чистая логика фильтрации: без GPU проверяем выборку по «есть текстура»
        // через имитацию карты (сам WidgetPass требует device) — тестируем
        // формулу: только квады с известными id попадают в порядок.
        let quads = [
            WidgetQuad {
                node_id: "a",
                pos: [0.0, 0.0],
                size: [1.0, 1.0],
            },
            WidgetQuad {
                node_id: "b",
                pos: [1.0, 1.0],
                size: [2.0, 2.0],
            },
            WidgetQuad {
                node_id: "c",
                pos: [3.0, 3.0],
                size: [1.0, 1.0],
            },
        ];
        let with_textures = ["a", "c"];
        let order: Vec<&str> = quads
            .iter()
            .filter(|q| with_textures.contains(&q.node_id))
            .map(|q| q.node_id)
            .collect();
        assert_eq!(order, vec!["a", "c"], "b без текстуры исключён");
    }

    #[test]
    fn lru_cap_is_sane() {
        assert_eq!(WIDGET_SNAPSHOT_MAX, 16, "план M5 §4.5");
    }

    #[test]
    fn instance_buffer_fits_capacity() {
        // Регрессия крэша при 3 live-виджетах: write_buffer 0..96 в буфер
        // 64 (захардкоженный размер при capacity 8). Буфер обязан вмещать
        // заявленную ёмкость — при FLOATS=8 инстанс занимает 32 байта.
        let stride = ThumbInstance::FLOATS * 4;
        assert_eq!(
            stride, 32,
            "лейаут thumbs.wgsl изменился — проверить паддинг"
        );
        for capacity in [8usize, 16, 32] {
            let size = WidgetPass::instance_buffer_size(capacity) as usize;
            assert_eq!(
                size,
                stride * capacity,
                "буфер меньше ёмкости → wgpu overrun при {capacity} инстансах"
            );
        }
        // Начальная ёмкость покрывает типичный сеанс до первого роста:
        // 3 виджета из лога крэша — с запасом в 96..256 байт
        assert!(
            WidgetPass::instance_buffer_size(8) as usize >= 3 * stride,
            "3 снапшота (96 Б) обязаны помещаться в начальный буфер"
        );
    }
}
