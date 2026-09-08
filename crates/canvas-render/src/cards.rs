//! Карточки нод: чистая раскладка/цвета + instanced SDF-пайплайн (T4).
//!
//! Скруглённый прямоугольник, тень и рамка выделения рисуются SDF во фрагментном
//! шейдере (shaders/cards.wgsl); все карточки кадра — один instanced draw.

use canvas_core::Node;

use crate::camera::Camera;
use crate::markdown;

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

/// Цвет по JSON Canvas spec: пресет "1".."6" или "#RRGGBB".
/// None — цвет не задан или не парсится (вызывающий подставляет свой дефолт).
pub fn named_color(color: Option<&str>) -> Option<[f32; 4]> {
    let value = color?;
    PRESET_COLORS
        .iter()
        .find(|(key, _)| *key == value)
        .map(|(_, rgba)| *rgba)
        .or_else(|| parse_hex(value))
}

/// Цвет заливки карточки: пресет "1".."6" или "#RRGGBB" по JSON Canvas spec, иначе дефолт.
pub fn card_color(node: &Node) -> [f32; 4] {
    named_color(node.color.as_deref()).unwrap_or(DEFAULT_FILL)
}

/// Цвет пресета палитры JSON Canvas ("1".."6") — для меню выбора цвета (T7).
pub fn preset_color(preset: &str) -> Option<[f32; 4]> {
    PRESET_COLORS
        .iter()
        .find(|(key, _)| *key == preset)
        .map(|(_, rgba)| *rgba)
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
            // Маркеры форматирования (**...**, ==...==) в заголовке не показываем
            return markdown::strip(line);
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
    /// x — радиус (world px), y — selected (0/1), z — broken (0/1),
    /// w — без тени (0/1, мелкие квады связей/портов, T8).
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

/// Инстанс карточки одной ноды: заливка по цвету, рамка выделения/битой
/// ссылки, параметры SDF. Чистая функция для z-прохода рендера.
pub fn card_instance(node: &Node, selected: bool) -> CardInstance {
    let broken = node.broken_link == Some(true);
    let border = if selected {
        SELECTION_BORDER
    } else if broken {
        BROKEN_BORDER
    } else {
        [0.0; 4]
    };
    CardInstance {
        pos: [node.x, node.y],
        size: [node.width, node.height],
        fill: card_color(node),
        border,
        params: [CORNER_RADIUS, f32::from(selected), f32::from(broken), 0.0],
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
            Some(card_instance(node, selected == Some(index)))
        })
        .collect()
}

// --- Связи (T8) ---
//
// Поворотов в пайплайне нет, поэтому кривые и стрелки рисуются цепочками
// маленьких кружков (квад d×d с radius = d/2): при плотной тесселяции
// соседние кружки перекрываются и дают гладкую линию без полигонов.

/// Точек тесселяции кривой связи при рендере (плотнее hit-test'а — гладкость).
pub const EDGE_RENDER_SEGMENTS: usize = 48;
/// Диаметр кружка линии связи в world-px.
pub const EDGE_DOT: f32 = 2.5;
/// Диаметр кружка выделенной связи (толще, T8).
pub const EDGE_DOT_SELECTED: f32 = 3.5;
/// Диаметр кружка порта ноды при hover в world-px.
pub const PORT_DOT: f32 = 10.0;
/// Цвет связи по умолчанию — нейтральный серо-голубой.
pub const EDGE_COLOR: [f32; 4] = [0.52, 0.58, 0.66, 1.0];
/// Цвет резиновой линии (drag новой связи) — акцент с прозрачностью.
const DRAFT_COLOR: [f32; 4] = [0.396, 0.612, 0.969, 0.7];
/// Длина уса стрелки в world-px.
const ARROW_LEN: f32 = 10.0;
/// Угол уса стрелки от обратного направления касательной.
const ARROW_ANGLE: f32 = std::f32::consts::FRAC_PI_6; // 30°
/// Кружков на ус стрелки.
const ARROW_DOTS: usize = 4;

/// Кружок диаметром `d` с центром в `center` (params.w = 1 — без тени).
fn dot(center: [f32; 2], d: f32, fill: [f32; 4]) -> CardInstance {
    CardInstance {
        pos: [center[0] - d / 2.0, center[1] - d / 2.0],
        size: [d, d],
        fill,
        border: [0.0; 4],
        params: [d / 2.0, 0.0, 0.0, 1.0],
    }
}

/// Кружки вдоль кривой (полилиния тесселяции) + стрелка на конце.
/// Стрелка — два «уса» из кружков от конца кривой назад по касательной ±30°.
fn curve_dots(
    curve: &canvas_core::CubicBezier,
    d: f32,
    fill: [f32; 4],
    out: &mut Vec<CardInstance>,
) {
    for point in canvas_core::tessellate(curve, EDGE_RENDER_SEGMENTS) {
        out.push(dot(point, d, fill));
    }
    let tangent = canvas_core::curve_tangent(curve, 1.0);
    let back = [-tangent[0], -tangent[1]];
    let (sin, cos) = ARROW_ANGLE.sin_cos();
    for sign in [1.0f32, -1.0] {
        // Поворот вектора back на ±ARROW_ANGLE
        let dir = [
            back[0] * cos - back[1] * sin * sign,
            back[0] * sin * sign + back[1] * cos,
        ];
        for i in 1..=ARROW_DOTS {
            let dist = ARROW_LEN * i as f32 / ARROW_DOTS as f32;
            out.push(dot(
                [curve.p1[0] + dir[0] * dist, curve.p1[1] + dir[1] * dist],
                d,
                fill,
            ));
        }
    }
}

/// Инстансы всех связей канваса (T8): кривые-«чётки» и стрелки.
/// Выделенная связь (`selected` — индекс в `canvas.edges`) ярче и толще.
/// Висячие связи (без ноды) пропускаются. Добавлять ПЕРЕД инстансами
/// карточек — связи под нодами (порядок в буфере = порядок рисования).
pub fn build_edge_instances(
    canvas: &canvas_core::Canvas,
    selected: Option<usize>,
) -> Vec<CardInstance> {
    let mut out = Vec::new();
    for (index, edge) in canvas.edges.iter().enumerate() {
        let Some(curve) = canvas_core::edge_curve(canvas, edge) else {
            continue;
        };
        let is_selected = selected == Some(index);
        let fill = if is_selected {
            SELECTION_BORDER
        } else {
            named_color(edge.color.as_deref()).unwrap_or(EDGE_COLOR)
        };
        let d = if is_selected {
            EDGE_DOT_SELECTED
        } else {
            EDGE_DOT
        };
        curve_dots(&curve, d, fill, &mut out);
    }
    out
}

/// Порты ноды при hover (T8): 4 кружка по центрам сторон, поверх карточек.
pub fn build_port_instances(canvas: &canvas_core::Canvas, node: usize) -> Vec<CardInstance> {
    let mut out = Vec::with_capacity(4);
    if let Some(node) = canvas.nodes.get(node) {
        for side in [
            canvas_core::Side::Top,
            canvas_core::Side::Right,
            canvas_core::Side::Bottom,
            canvas_core::Side::Left,
        ] {
            out.push(dot(
                canvas_core::port_point(node, side),
                PORT_DOT,
                SELECTION_BORDER,
            ));
        }
    }
    out
}

/// Резиновая линия новой связи (T8): кривая от порта до курсора,
/// полупрозрачная, без стрелки.
pub fn build_draft_instances(
    port: [f32; 2],
    side: canvas_core::Side,
    cursor: [f32; 2],
) -> Vec<CardInstance> {
    let curve = canvas_core::draft_curve(port, side, cursor);
    canvas_core::tessellate(&curve, EDGE_RENDER_SEGMENTS)
        .into_iter()
        .map(|point| dot(point, EDGE_DOT, DRAFT_COLOR))
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
        self.draw_range(pass, 0..count);
    }

    /// Нарисовать инстансы карточек диапазона `range` (z-порядок: сегменты
    /// кадра рисуют свои диапазоны, тамбнейлы и текст между ними).
    pub fn draw_range<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        range: std::ops::Range<u32>,
    ) {
        if range.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        pass.draw(0..6, range);
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
        // Маркеры форматирования в заголовке стрипятся
        let styled = Node::text("n", "**Важно** и ==срочно==", 0.0, 0.0);
        assert_eq!(title_for(&styled), "Важно и срочно");
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

    /// named_color: пресеты и hex парсятся, мусор и None — None (дефолт на вызывающем).
    #[test]
    fn named_color_parsing() {
        assert_eq!(named_color(None), None);
        assert_eq!(named_color(Some("1")), Some(PRESET_COLORS[0].1));
        assert_eq!(named_color(Some("#ff8000")).map(|c| c[0]), Some(1.0));
        assert_eq!(named_color(Some("9")), None);
        assert_eq!(named_color(Some("#zzz")), None);
    }

    /// Инстансы связей (T8): кружки тесселяции + усы стрелки; выделенная —
    /// акцентом и толще; цвет из edge.color; висячая связь пропускается.
    #[test]
    fn edge_instances_chain_and_arrow() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("b", "C:/b.png", 500.0, 0.0, 100.0, 100.0));
        let mut edge = canvas_core::Edge::new("e1", "a", None, "b", None);
        edge.color = Some("2".into());
        canvas.add_edge(edge);
        canvas.add_edge(canvas_core::Edge::new("e2", "a", None, "missing", None));

        let per_edge = EDGE_RENDER_SEGMENTS + 1 + ARROW_DOTS * 2;
        let instances = build_edge_instances(&canvas, None);
        assert_eq!(instances.len(), per_edge, "висячая e2 пропущена");
        // Все инстансы — кружки без тени цвета пресета "2"
        let expected = named_color(Some("2")).expect("пресет");
        for inst in &instances {
            assert_eq!(inst.params[0], inst.size[0] / 2.0, "круг: radius = d/2");
            assert_eq!(inst.params[3], 1.0, "без тени");
            assert_eq!(inst.fill, expected);
            assert_eq!(inst.size[0], EDGE_DOT);
        }
        // Первая и последняя точки кривой — в портах
        assert_eq!(
            instances[0].pos,
            [100.0 - EDGE_DOT / 2.0, 50.0 - EDGE_DOT / 2.0]
        );

        // Выделенная связь — акцент и толще
        let selected = build_edge_instances(&canvas, Some(0));
        assert_eq!(selected.len(), per_edge);
        assert_eq!(selected[0].fill, SELECTION_BORDER);
        assert_eq!(selected[0].size[0], EDGE_DOT_SELECTED);
    }

    /// Порты hover-ноды: 4 кружка по центрам сторон, акцентный цвет.
    #[test]
    fn port_instances_at_side_centers() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 100.0, 200.0, 300.0, 120.0));
        let ports = build_port_instances(&canvas, 0);
        assert_eq!(ports.len(), 4);
        let centers: Vec<[f32; 2]> = ports
            .iter()
            .map(|inst| {
                [
                    inst.pos[0] + inst.size[0] / 2.0,
                    inst.pos[1] + inst.size[1] / 2.0,
                ]
            })
            .collect();
        assert_eq!(centers[0], [250.0, 200.0]); // top
        assert_eq!(centers[1], [400.0, 260.0]); // right
        assert_eq!(centers[2], [250.0, 320.0]); // bottom
        assert_eq!(centers[3], [100.0, 260.0]); // left
        assert!(ports.iter().all(|inst| inst.size == [PORT_DOT, PORT_DOT]));
        // Невалидный индекс — пусто
        assert!(build_port_instances(&canvas, 9).is_empty());
    }

    /// Резиновая линия (T8): цепочка кружков от порта к курсору.
    #[test]
    fn draft_instances_from_port_to_cursor() {
        let instances =
            build_draft_instances([100.0, 50.0], canvas_core::Side::Right, [400.0, 200.0]);
        assert_eq!(instances.len(), EDGE_RENDER_SEGMENTS + 1);
        // Первый кружок — в порте, последний — у курсора
        let first = instances[0];
        assert_eq!(
            [
                first.pos[0] + first.size[0] / 2.0,
                first.pos[1] + first.size[1] / 2.0
            ],
            [100.0, 50.0]
        );
        let last = instances[instances.len() - 1];
        assert_eq!(
            [
                last.pos[0] + last.size[0] / 2.0,
                last.pos[1] + last.size[1] / 2.0
            ],
            [400.0, 200.0]
        );
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
