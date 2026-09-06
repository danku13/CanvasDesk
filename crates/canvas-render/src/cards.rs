//! Карточки нод: чистая раскладка/цвета + instanced SDF-пайплайн (T4).
//!
//! Скруглённый прямоугольник, тень и рамка выделения рисуются SDF во фрагментном
//! шейдере (shaders/cards.wgsl); все карточки кадра — один instanced draw.

use canvas_core::Node;

use crate::camera::Camera;

/// Высота заголовка карточки в world-пикселях.
pub const HEADER_HEIGHT: f32 = 28.0;
/// Радиус скругления в world-пикселях.
pub const CORNER_RADIUS: f32 = 8.0;

/// Цвет заливки карточки по умолчанию (#26262c).
const DEFAULT_FILL: [f32; 4] = [0.149, 0.149, 0.173, 1.0];
/// Рамка выделения (акцент).
pub const SELECTION_BORDER: [f32; 4] = [0.396, 0.612, 0.969, 1.0];
/// Рамка битой ссылки (brokenLink) — серая.
pub const BROKEN_BORDER: [f32; 4] = [0.45, 0.45, 0.45, 1.0];

/// Пресеты цветов JSON Canvas ("1".."6"), приглушённые тона поверх тёмного фона.
const PRESET_COLORS: [(&str, [f32; 4]); 6] = [
    ("1", [0.42, 0.24, 0.24, 1.0]), // red
    ("2", [0.45, 0.33, 0.20, 1.0]), // orange
    ("3", [0.45, 0.41, 0.20, 1.0]), // yellow
    ("4", [0.24, 0.40, 0.26, 1.0]), // green
    ("5", [0.20, 0.38, 0.40, 1.0]), // cyan
    ("6", [0.36, 0.27, 0.45, 1.0]), // purple
];

/// Парсинг `#RRGGBB` в RGBA 0..1.
fn parse_hex(color: &str) -> Option<[f32; 4]> {
    let hex = color.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |i: usize| -> Option<f32> {
        Some(u8::from_str_radix(&hex[i..i + 2], 16).ok()? as f32 / 255.0)
    };
    Some([channel(0)?, channel(2)?, channel(4)?, 1.0])
}

/// Цвет заливки карточки: пресет "1".."6" или "#RRGGBB" по JSON Canvas spec, иначе дефолт.
pub fn card_color(node: &Node) -> [f32; 4] {
    match node.color.as_deref() {
        Some(preset) => PRESET_COLORS
            .iter()
            .find(|(key, _)| *key == preset)
            .map(|(_, rgba)| *rgba)
            .or_else(|| parse_hex(preset))
            .unwrap_or(DEFAULT_FILL),
        None => DEFAULT_FILL,
    }
}

/// Заголовок карточки: имя файла из пути / первая строка текста / label группы.
pub fn title_for(node: &Node) -> String {
    if let Some(file) = &node.file {
        let name = file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(file.as_str());
        return name.to_owned();
    }
    if let Some(text) = &node.text {
        if let Some(line) = text.lines().next().filter(|line| !line.is_empty()) {
            return line.to_owned();
        }
    }
    if let Some(label) = &node.label {
        if !label.is_empty() {
            return label.clone();
        }
    }
    "—".to_owned()
}

/// Буква-заглушка иконки по расширению файла ("rs" → 'R').
pub fn extension_letter(node: &Node) -> Option<char> {
    let file = node.file.as_deref()?;
    let name = file.rsplit(['/', '\\']).next()?;
    let ext = name.rsplit_once('.')?.1;
    ext.chars().next().map(|c| c.to_ascii_uppercase())
}

/// Инстанс карточки для GPU (layout — attributes в cards.wgsl).
#[derive(Debug, Clone, Copy)]
pub struct CardInstance {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub fill: [f32; 4],
    pub border: [f32; 4],
    /// x — радиус (world px), y — selected (0/1), z — broken (0/1).
    pub params: [f32; 4],
}

impl CardInstance {
    const FLOATS: usize = 16;

    fn write_to(&self, out: &mut Vec<u8>) {
        for group in [
            &self.pos[..],
            &self.size[..],
            &self.fill[..],
            &self.border[..],
            &self.params[..],
        ] {
            for value in group {
                out.extend_from_slice(&value.to_ne_bytes());
            }
        }
    }
}

/// Собрать инстансы кадра по видимым нодам (culling, T5): `indices` —
/// результат `SpatialIndex::query_rect` по viewport, отсортирован по возрастанию
/// (z-порядок = порядок нод в массиве).
pub fn build_instances(
    canvas: &canvas_core::Canvas,
    indices: &[usize],
    selected: Option<usize>,
) -> Vec<CardInstance> {
    indices
        .iter()
        .filter_map(|&index| {
            let node = canvas.nodes.get(index)?;
            let selected = selected == Some(index);
            let broken = node.broken_link == Some(true);
            let border = if selected {
                SELECTION_BORDER
            } else if broken {
                BROKEN_BORDER
            } else {
                [0.0; 4]
            };
            Some(CardInstance {
                pos: [node.x, node.y],
                size: [node.width, node.height],
                fill: card_color(node),
                border,
                params: [CORNER_RADIUS, f32::from(selected), f32::from(broken), 0.0],
            })
        })
        .collect()
}

/// Uniform камеры — тот же layout, что у сетки (grid.rs).
#[derive(Debug, Clone, Copy)]
pub struct CameraUniform {
    pub position: [f32; 2],
    pub viewport: [f32; 2],
    pub effective_zoom: f32,
    pub _pad: [f32; 3],
}

impl CameraUniform {
    pub fn new(camera: &Camera, viewport: [f32; 2], scale_factor: f32) -> Self {
        Self {
            position: camera.position(),
            viewport,
            effective_zoom: camera.zoom() * scale_factor,
            _pad: [0.0; 3],
        }
    }

    pub fn to_bytes(self) -> [u8; 32] {
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

/// Пайплайн карточек: instanced quad + SDF в фрагментном шейдере.
pub struct CardsPipeline {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
}

impl CardsPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cards"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/cards.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("cards"),
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
            label: Some("cards"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: (CardInstance::FLOATS * 4) as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![
                0 => Float32x2, // pos
                1 => Float32x2, // size
                2 => Float32x4, // fill
                3 => Float32x4, // border
                4 => Float32x4, // params
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cards"),
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
            label: Some("cards camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cards"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let instance_capacity = 256;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cards instances"),
            size: (CardInstance::FLOATS * 4 * instance_capacity) as wgpu::BufferAddress,
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
        instances: &[CardInstance],
    ) -> u32 {
        let uniform = CameraUniform::new(camera, viewport, scale_factor);
        queue.write_buffer(&self.uniform_buffer, 0, &uniform.to_bytes());

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cards instances"),
                size: (CardInstance::FLOATS * 4 * self.instance_capacity) as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        let mut bytes = Vec::with_capacity(instances.len() * CardInstance::FLOATS * 4);
        for instance in instances {
            instance.write_to(&mut bytes);
        }
        queue.write_buffer(&self.instance_buffer, 0, &bytes);
        instances.len() as u32
    }

    /// Нарисовать `count` карточек в активном render pass.
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
    use canvas_core::{Canvas, Node};

    /// Пресеты "1".."6" отличаются от дефолта и друг от друга.
    #[test]
    fn color_presets() {
        let mut node = Node::text("n", "t", 0.0, 0.0);
        assert_eq!(card_color(&node), DEFAULT_FILL);
        node.color = Some("3".into());
        assert_eq!(card_color(&node), PRESET_COLORS[2].1);
        node.color = Some("6".into());
        assert_eq!(card_color(&node), PRESET_COLORS[5].1);
        node.color = Some("9".into());
        assert_eq!(card_color(&node), DEFAULT_FILL);
    }

    /// Hex-цвет "#RRGGBB" парсится; битый — дефолт.
    #[test]
    fn color_hex() {
        let mut node = Node::text("n", "t", 0.0, 0.0);
        node.color = Some("#ff8000".into());
        let rgba = card_color(&node);
        assert!((rgba[0] - 1.0).abs() < 1e-3);
        assert!((rgba[1] - 128.0 / 255.0).abs() < 1e-3);
        assert!((rgba[2] - 0.0).abs() < 1e-3);
        node.color = Some("#zzz".into());
        assert_eq!(card_color(&node), DEFAULT_FILL);
        node.color = Some("#12345".into());
        assert_eq!(card_color(&node), DEFAULT_FILL);
    }

    /// Заголовок: имя файла из Windows/Unix-пути, первая строка текста, label группы.
    #[test]
    fn title_extraction() {
        let file = Node::file(
            "n",
            "C:\\Projects\\альфа\\Спецификация.pdf",
            0.0,
            0.0,
            10.0,
            10.0,
        );
        assert_eq!(title_for(&file), "Спецификация.pdf");
        let unix = Node::file("n", "docs/SPEC.md", 0.0, 0.0, 10.0, 10.0);
        assert_eq!(title_for(&unix), "SPEC.md");
        let text = Node::text("n", "Первая строка\nвторая", 0.0, 0.0);
        assert_eq!(title_for(&text), "Первая строка");
        let mut group = Node::text("n", "", 0.0, 0.0);
        group.text = None;
        group.label = Some("Группа".into());
        assert_eq!(title_for(&group), "Группа");
        let empty = Node::text("n", "", 0.0, 0.0);
        let mut empty = empty;
        empty.text = None;
        assert_eq!(title_for(&empty), "—");
    }

    /// Буква иконки по расширению; без расширения/файла — None.
    #[test]
    fn icon_letter() {
        let file = Node::file("n", "C:/a/report.XLSX", 0.0, 0.0, 10.0, 10.0);
        assert_eq!(extension_letter(&file), Some('X'));
        let no_ext = Node::file("n", "C:/a/README", 0.0, 0.0, 10.0, 10.0);
        assert_eq!(extension_letter(&no_ext), None);
        let text = Node::text("n", "t", 0.0, 0.0);
        assert_eq!(extension_letter(&text), None);
    }

    /// Инстансы: рамка выделения/битой ссылки, z-порядок = порядок индексов.
    #[test]
    fn instances_reflect_selection_and_broken() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 10.0, 10.0));
        let mut broken = Node::file("b", "C:/b.png", 20.0, 0.0, 10.0, 10.0);
        broken.broken_link = Some(true);
        canvas.nodes.push(broken);

        let instances = build_instances(&canvas, &[0, 1], Some(1));
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].params[1], 0.0);
        assert_eq!(instances[1].params[1], 1.0);
        assert_eq!(instances[1].params[2], 1.0);
        assert_eq!(instances[1].border, SELECTION_BORDER);
    }

    /// Culling (T5): ноды вне переданных индексов не попадают в батч,
    /// порядок инстансов следует порядку индексов.
    #[test]
    fn instances_only_for_given_indices() {
        let mut canvas = Canvas::default();
        for i in 0..5 {
            canvas.nodes.push(Node::file(
                format!("n{i}"),
                "C:/f.png",
                i as f32 * 100.0,
                0.0,
                10.0,
                10.0,
            ));
        }
        // Видимы только ноды 1 и 3 (выдача spatial index отсортирована)
        let instances = build_instances(&canvas, &[1, 3], None);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].pos, [100.0, 0.0]);
        assert_eq!(instances[1].pos, [300.0, 0.0]);
        // Пустой список — пустой батч
        assert!(build_instances(&canvas, &[], None).is_empty());
        // Невалидный индекс пропускается без паники
        assert_eq!(build_instances(&canvas, &[99], None).len(), 0);
    }

    /// Сериализация инстанса совпадает с vertex buffer stride (FLOATS * 4 байта).
    #[test]
    fn instance_stride_matches_serialization() {
        let instance = CardInstance {
            pos: [1.0, 2.0],
            size: [3.0, 4.0],
            fill: [0.1, 0.2, 0.3, 1.0],
            border: [0.0; 4],
            params: [8.0, 1.0, 0.0, 0.0],
        };
        let mut bytes = Vec::new();
        instance.write_to(&mut bytes);
        assert_eq!(bytes.len(), CardInstance::FLOATS * 4);
        assert_eq!(&bytes[0..4], &1.0f32.to_ne_bytes());
        assert_eq!(&bytes[48..52], &8.0f32.to_ne_bytes());
    }
}
