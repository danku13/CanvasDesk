//! Миникарта (T13, SPEC §6.1): снимок сцены, подгонка масштаба и CPU-растеризация
//! обзорного кадра + математика world↔minimap для навигации кликом/драгом.
//!
//! Модуль чистой математики: без wgpu/winit — покрывается юнит-тестами.
//! Логический размер миникарты константен (220×140, отступ 16 — SPEC/TASKS T13);
//! растеризация выполняется в физических пикселях — размер буфера задаёт
//! приложение (`width_px/height_px`, обычно 220×scale_factor).
//!
//! Отступление от SPEC §6.1 (offscreen GPU-проход): контент миникарты — плоские
//! одноцветные прямоугольники и линии 1px; буфер собирается на CPU и загружается
//! одной текстурой в T13-B (`minimap_pass.rs`). Обоснование — план T13 §3/§8.

use std::collections::HashMap;

use canvas_core::Canvas;

/// Ширина миникарты в логических px (SPEC §6.1 / TASKS T13).
pub const MINIMAP_W: u32 = 220;
/// Высота миникарты в логических px.
pub const MINIMAP_H: u32 = 140;
/// Отступ от правого нижнего угла окна в логических px.
pub const MINIMAP_MARGIN: f32 = 16.0;
/// Радиус скругления углов фона миникарты, px.
pub const CORNER_RADIUS: u32 = 8;
/// Рамка текущего viewport, px.
pub const VIEWPORT_FRAME_PX: u32 = 2;
/// Максимальное число нод, при котором ещё рисуются edges (TASKS T13).
pub const MAX_NODES_FOR_EDGES: usize = 500;
/// Внутренний отступ контента от края миникарты, px.
pub const CONTENT_PADDING_PX: f32 = 8.0;
/// Минимальный размер прямоугольника ноды на миникарте, px.
pub const NODE_MIN_PX: u32 = 2;

/// Цвет ноды-файла (RGBA, непрозрачный).
pub const NODE_COLOR_FILE: [u8; 4] = [96, 148, 228, 255];
/// Цвет текстовой ноды (жёлтая палитра заметок).
pub const NODE_COLOR_TEXT: [u8; 4] = [228, 196, 96, 255];
/// Цвет ноды-группы.
pub const NODE_COLOR_GROUP: [u8; 4] = [150, 150, 158, 255];
/// Цвет ноды с битой ссылкой (file, brokenLink).
pub const NODE_COLOR_BROKEN: [u8; 4] = [214, 92, 92, 255];
/// Цвет рамки viewport (белый, непрозрачный).
pub const VIEWPORT_COLOR: [u8; 4] = [255, 255, 255, 255];
/// Цвет линий edges.
pub const EDGE_COLOR: [u8; 4] = [170, 176, 188, 255];
/// Фон миникарты: тёмный, полупрозрачный.
pub const BG_COLOR: [u8; 4] = [30, 32, 38, 184];

/// Максимальная сторона буфера миникарты, px: реальный физический размер
/// (логические 220×140 × scale_factor) не превышает размеры экрана с большим
/// запасом; порог защищает от гигантской аллокации при некорректном вызове.
const MAX_BUFFER_PX: u32 = 4096;
/// Запас за границей буфера для клампа концов линий edges: искажается только
/// невидимая часть линии, зато исключены многосекундные проходы Брезенхэма по
/// гигантским map-координатам (вырожденный масштаб точечной сцены).
const EDGE_CLAMP_MARGIN_PX: i32 = 64;
/// Минимальная половина контента по оси (world): защита от деления на ноль
/// при вычислении масштаба (точечный/линейный контент).
const MIN_CONTENT_HALF: f32 = 1e-3;
/// Минимальный равномерный масштаб (px/world): обратное преобразование
/// не делит на ноль даже при испорченных вручную полях.
const MIN_SCALE: f32 = 1e-6;

/// Тип ноды на миникарте — цвет прямоугольника (TASKS T13: «цвет по типу»).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapNodeKind {
    /// Файловая нода.
    File,
    /// Файловая нода с brokenLink (T10) — выделяется красным.
    FileBroken,
    /// Текстовая заметка.
    Text,
    /// Группа.
    Group,
}

impl MinimapNodeKind {
    /// Цвет заливки прямоугольника ноды.
    pub const fn color(self) -> [u8; 4] {
        match self {
            Self::File => NODE_COLOR_FILE,
            Self::FileBroken => NODE_COLOR_BROKEN,
            Self::Text => NODE_COLOR_TEXT,
            Self::Group => NODE_COLOR_GROUP,
        }
    }
}

/// Упрощённая нода миникарты: AABB в world-координатах + тип.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapNode {
    /// `[min_x, min_y, max_x, max_y]` в world-координатах.
    pub rect: [f32; 4],
    /// Тип (цвет) ноды.
    pub kind: MinimapNodeKind,
}

impl MinimapNode {
    /// Центр AABB ноды.
    pub const fn center(&self) -> [f32; 2] {
        [
            (self.rect[0] + self.rect[2]) / 2.0,
            (self.rect[1] + self.rect[3]) / 2.0,
        ]
    }
}

/// Снимок сцены для миникарты: ноды (AABB+тип), edges (пары центров нод в world).
/// Владеет данными (собирается из `Canvas` один раз на изменение сцены).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MinimapInput {
    /// Упрощённые ноды.
    pub nodes: Vec<MinimapNode>,
    /// Edge как отрезок центр→центр (упрощение: side-геометрия не нужна).
    pub edges: Vec<([f32; 2], [f32; 2])>,
    /// Видимый прямоугольник камеры: `[min_x, min_y, max_x, max_y]`, world.
    pub viewport_world: [f32; 4],
}

impl MinimapInput {
    /// Собрать снимок канваса: file-ноды (brokenLink → FileBroken), text, group;
    /// edges — центры соединяемых нод. Некорректные rect нод (min>max, NaN)
    /// пропускаются, а не ломают снимок.
    pub fn from_canvas(canvas: &Canvas, viewport_world: [f32; 4]) -> Self {
        let mut nodes = Vec::with_capacity(canvas.nodes.len());
        // Центры валидных нод по id: edges резолвятся за O(1) на связь
        // (линейный поиск при 5000 нод давал бы O(n·edges)).
        let mut centers: HashMap<&str, [f32; 2]> = HashMap::with_capacity(canvas.nodes.len());
        for node in &canvas.nodes {
            let kind = match node.node_type.as_str() {
                "file" if node.broken_link == Some(true) => MinimapNodeKind::FileBroken,
                "file" => MinimapNodeKind::File,
                "text" => MinimapNodeKind::Text,
                "group" => MinimapNodeKind::Group,
                // Неизвестные типы (link, widget из M5, будущие) не имеют
                // устоявшегося цвета на миникарте — пропускаются (план T13 §5.1
                // описывает file/text/group). При появлении новых типов решение
                // пересматривается на уровне контракта.
                _ => continue,
            };
            let rect = [node.x, node.y, node.x + node.width, node.y + node.height];
            let Some(rect) = sanitize_rect(rect) else {
                continue;
            };
            let minimap_node = MinimapNode { rect, kind };
            centers.insert(node.id.as_str(), minimap_node.center());
            nodes.push(minimap_node);
        }
        let mut edges = Vec::with_capacity(canvas.edges.len());
        for edge in &canvas.edges {
            // Связь на отсутствующую или пропущенную ноду в снимок не попадает.
            if let (Some(from), Some(to)) = (
                centers.get(edge.from_node.as_str()),
                centers.get(edge.to_node.as_str()),
            ) {
                edges.push((*from, *to));
            }
        }
        Self {
            nodes,
            edges,
            viewport_world,
        }
    }
}

/// Растеризованный кадр миникарты: RGBA8, `width × height`, без mipmap.
/// Расположение в памяти — построчно, начиная с верхнего левого угла.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimapImage {
    /// Ширина буфера, px (физические).
    pub width: u32,
    /// Высота буфера, px (физические).
    pub height: u32,
    /// Пиксели RGBA8, длина = width × height × 4.
    pub rgba: Vec<u8>,
}

impl MinimapImage {
    /// Пустое изображение 0×0 (нет данных).
    pub const EMPTY: Self = Self {
        width: 0,
        height: 0,
        rgba: Vec::new(),
    };
}

/// Подгонка world→мини-карта: content bounds (ноды ∪ viewport) с padding,
/// равномерный масштаб, центрирование. Вырожденные случаи (пусто, точка,
/// нулевая ширина/высота) дают безопасное отображение без паники и NaN:
/// пустой контент — тождественный масштаб 1:1 с началом world в центре буфера.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimapMapping {
    /// Центр контента в world.
    pub content_center: [f32; 2],
    /// Половина размера контента в world (не меньше 1e-3 по каждой оси).
    pub content_half: [f32; 2],
    /// Размер буфера, px.
    pub size_px: [f32; 2],
    /// Отступ контента внутри буфера, px.
    pub padding_px: f32,
}

impl MinimapMapping {
    /// Подогнать content bounds (union AABB нод ∪ viewport, + padding) в буфер
    /// `width_px × height_px`: равномерный масштаб и центрирование. Viewport
    /// включается в контент только при корректном невырожденном rect (нулевой
    /// viewport свёрнутого окна контент не растягивает); пустой контент —
    /// масштаб 1:1 (см. доку структуры).
    pub fn fit(
        nodes: &[MinimapNode],
        viewport_world: [f32; 4],
        width_px: u32,
        height_px: u32,
    ) -> Self {
        let size_px = [width_px as f32, height_px as f32];
        let mut bounds: Option<[f32; 4]> = None;
        for node in nodes {
            if let Some(rect) = sanitize_rect(node.rect) {
                bounds = Some(union_rect(bounds, rect));
            }
        }
        if let Some(rect) = sanitize_rect(viewport_world) {
            if rect[2] > rect[0] && rect[3] > rect[1] {
                bounds = Some(union_rect(bounds, rect));
            }
        }
        let Some([min_x, min_y, max_x, max_y]) = bounds else {
            // Пустой контент: content_half равен половине доступной области —
            // масштаб 1:1, начало world — в центре буфера; round-trip точен.
            let avail = avail_px(size_px, CONTENT_PADDING_PX);
            return Self {
                content_center: [0.0, 0.0],
                content_half: [avail[0] * 0.5, avail[1] * 0.5],
                size_px,
                padding_px: CONTENT_PADDING_PX,
            };
        };
        Self {
            content_center: [(min_x + max_x) * 0.5, (min_y + max_y) * 0.5],
            // Вырожденная ось (точка/линия контента) — минимум MIN_CONTENT_HALF,
            // чтобы масштаб в `scale` оставался конечным.
            content_half: [
                ((max_x - min_x) * 0.5).max(MIN_CONTENT_HALF),
                ((max_y - min_y) * 0.5).max(MIN_CONTENT_HALF),
            ],
            size_px,
            padding_px: CONTENT_PADDING_PX,
        }
    }

    /// Равномерный масштаб world→map (px на world-единицу), вычисляется из
    /// полей. Поля публичны и могут быть изменены извне: NaN-компоненты
    /// поглощаются `max`/`min` (операнд NaN отбрасывается), результат всегда
    /// конечен и не меньше MIN_SCALE — деление в map_to_world безопасно.
    fn scale(&self) -> f32 {
        let half = [
            self.content_half[0].max(MIN_CONTENT_HALF),
            self.content_half[1].max(MIN_CONTENT_HALF),
        ];
        let avail = avail_px(self.size_px, self.padding_px);
        let scale_x = avail[0] / (2.0 * half[0]);
        let scale_y = avail[1] / (2.0 * half[1]);
        scale_x.min(scale_y).max(MIN_SCALE)
    }

    /// world → px миникарты (верхний левый угол = [0, 0]). Не-конечные
    /// компоненты входа заменяются центром контента — выход всегда конечен.
    pub fn world_to_map(&self, world: [f32; 2]) -> [f32; 2] {
        let world = [
            finite_or(world[0], self.content_center[0]),
            finite_or(world[1], self.content_center[1]),
        ];
        let scale = self.scale();
        [
            self.size_px[0] * 0.5 + (world[0] - self.content_center[0]) * scale,
            self.size_px[1] * 0.5 + (world[1] - self.content_center[1]) * scale,
        ]
    }

    /// px миникарты → world (обратное преобразование; hit-test клика/драга).
    /// Не-конечные компоненты входа заменяются центром карты.
    pub fn map_to_world(&self, px: [f32; 2]) -> [f32; 2] {
        let px = [
            finite_or(px[0], self.size_px[0] * 0.5),
            finite_or(px[1], self.size_px[1] * 0.5),
        ];
        let scale = self.scale();
        [
            self.content_center[0] + (px[0] - self.size_px[0] * 0.5) / scale,
            self.content_center[1] + (px[1] - self.size_px[1] * 0.5) / scale,
        ]
    }
}

/// Снимок + подгонка + растеризация миникарты (T13-A).
///
/// С lifecycle приложения: `capture` — на изменение сцены/размера буфера;
/// `render` — на изменение камеры (рамка viewport) или после `capture`;
/// `map_to_world` — на события мыши (клик/драг по миникарте).
#[derive(Debug, Clone, PartialEq)]
pub struct Minimap {
    input: MinimapInput,
    mapping: MinimapMapping,
}

impl Minimap {
    /// Снять сцену и подогнать масштаб под размер буфера `width_px × height_px`
    /// (физические px; обычно логические 220×140 × scale_factor).
    pub fn capture(
        canvas: &Canvas,
        viewport_world: [f32; 4],
        width_px: u32,
        height_px: u32,
    ) -> Self {
        let input = MinimapInput::from_canvas(canvas, viewport_world);
        let mapping = MinimapMapping::fit(&input.nodes, viewport_world, width_px, height_px);
        Self { input, mapping }
    }

    /// Обновить только рамку viewport (камера двигается, сцена — нет):
    /// дешевле, чем полная пересборка `capture`.
    pub fn set_viewport(&mut self, viewport_world: [f32; 4]) {
        // Content bounds зависит от viewport: пересобираем fit по сохранённым
        // нодам с прежним размером буфера (size_px текущего mapping).
        let width = self.mapping.size_px[0].max(0.0) as u32;
        let height = self.mapping.size_px[1].max(0.0) as u32;
        self.input.viewport_world = viewport_world;
        self.mapping = MinimapMapping::fit(&self.input.nodes, viewport_world, width, height);
    }

    /// Растеризация в RGBA-буфер (фон+скругление, ноды, edges, рамка viewport).
    /// Чистая функция без мутации состояния: может вызываться каждый кадр
    /// при изменении камеры (порог 2 мс на 5000 нод — план T13 §1).
    ///
    /// Сложность: O(пикселей фона + сумма площадей нод + длина edges);
    /// аллокаций на ноду нет (единственная аллокация — сам буфер).
    pub fn render(&self) -> MinimapImage {
        // Сатурирующее приведение: NaN/отрицательный размер → 0 (as u32).
        let width = self.mapping.size_px[0].max(0.0) as u32;
        let height = self.mapping.size_px[1].max(0.0) as u32;
        if width == 0 || height == 0 || width > MAX_BUFFER_PX || height > MAX_BUFFER_PX {
            return MinimapImage::EMPTY;
        }
        // Стороны уже ≤ MAX_BUFFER_PX — произведение не переполняет usize.
        let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
        {
            let mut raster = Raster {
                rgba: &mut rgba,
                w: width as i32,
                h: height as i32,
            };
            // 1. Фон: полупрозрачная заливка на весь буфер.
            raster.fill_rect(0, 0, width as i32, height as i32, BG_COLOR);
            // 2. Ноды: map-прямоугольники с клампом и минимальным размером.
            for node in &self.input.nodes {
                let Some(rect) = sanitize_rect(node.rect) else {
                    continue;
                };
                let p_min = self.mapping.world_to_map([rect[0], rect[1]]);
                let p_max = self.mapping.world_to_map([rect[2], rect[3]]);
                raster.fill_node_rect(p_min, p_max, node.kind.color());
            }
            // 3. Edges: линии 1 px между map-центрами нод — только на читаемых
            //    сценах (TASKS T13: при 500+ нодах линии не рисуются).
            if self.input.nodes.len() < MAX_NODES_FOR_EDGES {
                for (from, to) in &self.input.edges {
                    let a = self.mapping.world_to_map(*from);
                    let b = self.mapping.world_to_map(*to);
                    raster.draw_line(a, b, EDGE_COLOR);
                }
            }
            // 4. Рамка viewport: белый контур толщиной VIEWPORT_FRAME_PX
            //    (невалидный viewport — без рамки).
            if sanitize_rect(self.input.viewport_world).is_some() {
                let frame = self.viewport_frame_px();
                raster.fill_frame(frame, VIEWPORT_COLOR);
            }
            // 5. Скругление углов: за пределами радиуса alpha 0 — маска
            //    поверх всего контента (углы миникарты всегда прозрачны).
            raster.round_corners(CORNER_RADIUS as i32);
        }
        MinimapImage {
            width,
            height,
            rgba,
        }
    }

    /// Экран миникарты (px) → world-точка. Клик = центрирование камеры.
    pub fn map_to_world(&self, px: [f32; 2]) -> [f32; 2] {
        self.mapping.map_to_world(px)
    }

    /// world → px миникарты (симметрия для тестов hit-test).
    pub fn world_to_map(&self, world: [f32; 2]) -> [f32; 2] {
        self.mapping.world_to_map(world)
    }

    /// Прямоугольник viewport на миникарте, px: `[x0, y0, x1, y1]` — рамка.
    /// Не-конечные углы невалидного viewport'а отображаются в центр
    /// (см. `MinimapMapping::world_to_map`).
    pub fn viewport_frame_px(&self) -> [f32; 4] {
        let vp = self.input.viewport_world;
        let p_min = self.mapping.world_to_map([vp[0], vp[1]]);
        let p_max = self.mapping.world_to_map([vp[2], vp[3]]);
        [p_min[0], p_min[1], p_max[0], p_max[1]]
    }

    /// Снимок сцены (для отладки/тестов).
    pub fn input(&self) -> &MinimapInput {
        &self.input
    }
}

/// Конечное значение или `fallback` (NaN/±inf заменяются нейтральной точкой,
/// чтобы не-конечные входы не давали NaN на выходе преобразований).
fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

/// Валидный прямоугольник `[min_x, min_y, max_x, max_y]`: все значения конечны
/// и min ≤ max. Некорректные rect (NaN/±inf, вывернутые min>max — отрицательные
/// width/height) отбрасываются вызывающей стороной.
fn sanitize_rect(rect: [f32; 4]) -> Option<[f32; 4]> {
    if rect.iter().all(|v| v.is_finite()) && rect[0] <= rect[2] && rect[1] <= rect[3] {
        Some(rect)
    } else {
        None
    }
}

/// Объединение AABB: `bounds` = None — просто `rect`
/// (оба аргумента — `[min_x, min_y, max_x, max_y]`).
fn union_rect(bounds: Option<[f32; 4]>, rect: [f32; 4]) -> [f32; 4] {
    match bounds {
        None => rect,
        Some(b) => [
            b[0].min(rect[0]),
            b[1].min(rect[1]),
            b[2].max(rect[2]),
            b[3].max(rect[3]),
        ],
    }
}

/// Доступная область контента в буфере (px): размер минус двойной padding.
/// Вырожденный буфер (меньше 2·padding) клампится к 1 px — масштаб остаётся
/// конечным и положительным.
fn avail_px(size_px: [f32; 2], padding_px: f32) -> [f32; 2] {
    [
        (size_px[0] - 2.0 * padding_px).max(1.0),
        (size_px[1] - 2.0 * padding_px).max(1.0),
    ]
}

/// Полуинтервал пикселей `[a, b)`, растянутый до `min_px` от центра `center`
/// (крошечные/точечные примитивы остаются видимыми).
fn min_size_span(a: i32, b: i32, center: i32, min_px: i32) -> (i32, i32) {
    if b - a >= min_px {
        (a, b)
    } else {
        let start = center - min_px / 2;
        (start, start + min_px)
    }
}

/// CPU-растеризатор буфера миникарты: кламп координат в границы и заливка
/// примитивов (внутренняя структура `render`, вне публичного контракта).
struct Raster<'a> {
    rgba: &'a mut [u8],
    /// Ширина буфера, px.
    w: i32,
    /// Высота буфера, px.
    h: i32,
}

impl Raster<'_> {
    /// Заливка пикселей `[x0, x1) × [y0, y1)` с клампом в буфер.
    fn fill_rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: [u8; 4]) {
        let x0 = x0.clamp(0, self.w);
        let x1 = x1.clamp(0, self.w);
        let y0 = y0.clamp(0, self.h);
        let y1 = y1.clamp(0, self.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let row = y as usize * self.w as usize;
            for x in x0..x1 {
                let offset = (row + x as usize) * 4;
                self.rgba[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }

    /// Прямоугольник ноды по map-координатам: floor/ceil до пикселей, кламп
    /// в буфер, минимальный размер NODE_MIN_PX растягивается от центра ноды.
    /// Нода целиком вне буфера минимальным размером внутрь не вытягивается.
    fn fill_node_rect(&mut self, p_min: [f32; 2], p_max: [f32; 2], color: [u8; 4]) {
        let x0 = p_min[0].floor() as i32;
        let y0 = p_min[1].floor() as i32;
        let x1 = p_max[0].ceil() as i32;
        let y1 = p_max[1].ceil() as i32;
        if x1 <= 0 || y1 <= 0 || x0 >= self.w || y0 >= self.h {
            return;
        }
        let cx = ((p_min[0] + p_max[0]) * 0.5).round() as i32;
        let cy = ((p_min[1] + p_max[1]) * 0.5).round() as i32;
        let (x0, x1) = min_size_span(x0, x1, cx, NODE_MIN_PX as i32);
        let (y0, y1) = min_size_span(y0, y1, cy, NODE_MIN_PX as i32);
        self.fill_rect(x0, y0, x1, y1, color);
    }

    /// Контур рамки viewport по map-координатам: полосы толщиной
    /// VIEWPORT_FRAME_PX (верх/низ — во всю ширину, лево/право — между ними,
    /// углы не дублируются); вырожденная рамка растягивается до минимального
    /// видимого размера (нулевая площадь — камера свёрнутого окна).
    fn fill_frame(&mut self, frame: [f32; 4], color: [u8; 4]) {
        let x0 = frame[0].floor() as i32;
        let y0 = frame[1].floor() as i32;
        let x1 = frame[2].ceil() as i32;
        let y1 = frame[3].ceil() as i32;
        let cx = ((frame[0] + frame[2]) * 0.5).round() as i32;
        let cy = ((frame[1] + frame[3]) * 0.5).round() as i32;
        let (x0, x1) = min_size_span(x0, x1, cx, VIEWPORT_FRAME_PX as i32);
        let (y0, y1) = min_size_span(y0, y1, cy, VIEWPORT_FRAME_PX as i32);
        let t = VIEWPORT_FRAME_PX as i32;
        self.fill_rect(x0, y0, x1, y0 + t, color);
        self.fill_rect(x0, y1 - t, x1, y1, color);
        self.fill_rect(x0, y0 + t, x0 + t, y1 - t, color);
        self.fill_rect(x1 - t, y0 + t, x1, y1 - t, color);
    }

    /// Линия 1 px (алгоритм Брезенхэма) между map-точками. Концы клампятся
    /// в буфер с запасом EDGE_CLAMP_MARGIN_PX (видимой части искажение не
    /// видно), пиксели вне буфера пропускаются.
    fn draw_line(&mut self, a: [f32; 2], b: [f32; 2], color: [u8; 4]) {
        let x_min = -EDGE_CLAMP_MARGIN_PX;
        let y_min = -EDGE_CLAMP_MARGIN_PX;
        let x_max = self.w + EDGE_CLAMP_MARGIN_PX;
        let y_max = self.h + EDGE_CLAMP_MARGIN_PX;
        let ax = (a[0].round() as i32).clamp(x_min, x_max);
        let ay = (a[1].round() as i32).clamp(y_min, y_max);
        let bx = (b[0].round() as i32).clamp(x_min, x_max);
        let by = (b[1].round() as i32).clamp(y_min, y_max);
        let step_x = if bx >= ax { 1 } else { -1 };
        let step_y = if by >= ay { 1 } else { -1 };
        let dx = (bx - ax).abs();
        let dy = -(by - ay).abs();
        let mut x = ax;
        let mut y = ay;
        let mut err = dx + dy;
        loop {
            self.put_pixel(x, y, color);
            if x == bx && y == by {
                break;
            }
            let doubled = 2 * err;
            if doubled >= dy {
                err += dy;
                x += step_x;
            }
            if doubled <= dx {
                err += dx;
                y += step_y;
            }
        }
    }

    /// Одиночный пиксель с проверкой границ (линии выходят за края буфера).
    fn put_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            let offset = (y as usize * self.w as usize + x as usize) * 4;
            self.rgba[offset..offset + 4].copy_from_slice(&color);
        }
    }

    /// Скругление углов: пиксели, у которых квадрат расстояния от центра
    /// скругления больше r², получают alpha 0 (четыре угла зеркальны).
    fn round_corners(&mut self, radius: i32) {
        let r = radius.min(self.w).min(self.h);
        let r2 = r * r;
        for y in 0..r {
            for x in 0..r {
                let dx = r - x;
                let dy = r - y;
                if dx * dx + dy * dy > r2 {
                    let x = x as usize;
                    let y = y as usize;
                    let w = self.w as usize;
                    let h = self.h as usize;
                    self.clear_alpha(x, y);
                    self.clear_alpha(w - 1 - x, y);
                    self.clear_alpha(x, h - 1 - y);
                    self.clear_alpha(w - 1 - x, h - 1 - y);
                }
            }
        }
    }

    /// alpha = 0: пиксель за маской скругления (координаты в границах буфера).
    fn clear_alpha(&mut self, x: usize, y: usize) {
        let offset = (y * self.w as usize + x) * 4;
        self.rgba[offset + 3] = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::{Edge, Node, Side};
    use std::time::Instant;

    /// Допуск для точных аффинных проверок (суб-пиксель).
    const EPS: f32 = 1e-2;

    fn assert_close(actual: [f32; 2], expected: [f32; 2], eps: f32, what: &str) {
        assert!(
            (actual[0] - expected[0]).abs() <= eps && (actual[1] - expected[1]).abs() <= eps,
            "{what}: ожидалось {expected:?}, получено {actual:?}"
        );
    }

    /// Нода произвольного типа: Node::text + мутация типа/размера
    /// (у модели нет конструкторов group/broken — поля публичные).
    fn node_of_type(node_type: &str, id: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
        let mut node = Node::text(id, "заметка", x, y);
        node.node_type = node_type.to_owned();
        node.width = w;
        node.height = h;
        node
    }

    fn canvas(nodes: Vec<Node>, edges: Vec<Edge>) -> Canvas {
        Canvas {
            nodes,
            edges,
            ..Canvas::default()
        }
    }

    /// Сетка file-нод со шагом 300 (генератор сцен на пороге edges).
    fn grid_nodes(count: usize) -> Vec<Node> {
        (0..count)
            .map(|i| {
                node_of_type(
                    "file",
                    &format!("n{i}"),
                    (i % 50) as f32 * 300.0,
                    (i / 50) as f32 * 300.0,
                    100.0,
                    100.0,
                )
            })
            .collect()
    }

    fn pixel(image: &MinimapImage, x: u32, y: u32) -> [u8; 4] {
        let offset = (y as usize * image.width as usize + x as usize) * 4;
        [
            image.rgba[offset],
            image.rgba[offset + 1],
            image.rgba[offset + 2],
            image.rgba[offset + 3],
        ]
    }

    /// Пиксель под map-точкой (floor координат).
    fn pixel_at(image: &MinimapImage, map: [f32; 2]) -> [u8; 4] {
        pixel(image, map[0] as u32, map[1] as u32)
    }

    fn contains_color(image: &MinimapImage, color: [u8; 4]) -> bool {
        image.rgba.chunks_exact(4).any(|px| px == color.as_slice())
    }

    /// Контент 100×100 в буфере 220×140: ограничивает ось Y (124/100 < 204/100),
    /// масштаб 1.24 — по Y контент прижат к padding, по X центрирован.
    #[test]
    fn fit_scales_uniformly_and_centers_content() {
        let mapping = MinimapMapping::fit(&[], [0.0, 0.0, 100.0, 100.0], 220, 140);
        assert_close(
            mapping.world_to_map([50.0, 50.0]),
            [110.0, 70.0],
            EPS,
            "центр контента в центре буфера",
        );
        assert_close(
            mapping.world_to_map([0.0, 0.0]),
            [48.0, 8.0],
            EPS,
            "min-угол: y прижат к padding",
        );
        assert_close(
            mapping.world_to_map([100.0, 100.0]),
            [172.0, 132.0],
            EPS,
            "max-угол: y у нижнего padding",
        );
    }

    /// Round-trip world↔map на краях контента, в центре и в произвольных
    /// точках: погрешность ≤ 1 px карты (план T13 §5.1).
    #[test]
    fn world_map_round_trip_edges_and_center() {
        let mapping = MinimapMapping::fit(&[], [0.0, 0.0, 100.0, 100.0], 220, 140);
        for world in [
            [0.0, 0.0],
            [100.0, 100.0],
            [50.0, 50.0],
            [13.7, 87.3],
            [100.0, 0.0],
        ] {
            let map = mapping.world_to_map(world);
            let back = mapping.map_to_world(map);
            let map2 = mapping.world_to_map(back);
            assert_close(map2, map, 1.0, "round-trip world→map→world→map");
        }
        for map in [[8.0, 8.0], [212.0, 132.0], [110.0, 70.0], [5.0, 130.0]] {
            let world = mapping.map_to_world(map);
            assert!(
                world[0].is_finite() && world[1].is_finite(),
                "world конечен для {map:?}"
            );
            assert_close(
                mapping.world_to_map(world),
                map,
                1.0,
                "round-trip map→world→map",
            );
        }
    }

    /// Пустой контент (нет ни нод, ни валидного viewport): масштаб 1:1,
    /// начало world — в центре буфера, без паники и NaN (тождественный
    /// масштаб — интерпретация «тождественного отображения» контракта).
    #[test]
    fn fit_empty_content_identity_scale() {
        for bad_viewport in [
            [f32::NAN; 4],
            [0.0, 0.0, 0.0, 0.0],
            [f32::INFINITY; 4],
            [100.0, 0.0, 0.0, 100.0], // вывернутый min>max
        ] {
            let mapping = MinimapMapping::fit(&[], bad_viewport, 220, 140);
            assert_close(
                mapping.world_to_map([0.0, 0.0]),
                [110.0, 70.0],
                EPS,
                "origin в центре буфера",
            );
            assert_close(
                mapping.world_to_map([10.0, -4.0]),
                [120.0, 66.0],
                EPS,
                "масштаб 1:1",
            );
            assert_close(
                mapping.map_to_world([137.0, 71.0]),
                [27.0, 1.0],
                EPS,
                "обратное преобразование 1:1",
            );
        }
    }

    /// Одна нода-точка (width=height=0): контент-точка попадает в центр буфера,
    /// масштаб конечен (защита 1e-3), round-trip без NaN.
    #[test]
    fn fit_single_point_node_centers() {
        let nodes = [MinimapNode {
            rect: [100.0, 200.0, 100.0, 200.0],
            kind: MinimapNodeKind::Text,
        }];
        let mapping = MinimapMapping::fit(&nodes, [f32::NAN; 4], 220, 140);
        assert_close(
            mapping.world_to_map([100.0, 200.0]),
            [110.0, 70.0],
            0.5,
            "точка в центре буфера",
        );
        let back = mapping.map_to_world([110.0, 70.0]);
        assert_close(back, [100.0, 200.0], 0.5, "центр карты → точка контента");
        assert!(back[0].is_finite() && back[1].is_finite());
    }

    /// Сцена на ±1e6: края контента попадают в padding ограничивающей оси,
    /// round-trip ≤ 1 px, все значения конечны (огромные координаты не дают NaN).
    #[test]
    fn fit_far_scene_no_nan() {
        let nodes = [
            MinimapNode {
                rect: [-1.0e6, -1.0e6, -1.0e6 + 50.0, -1.0e6 + 50.0],
                kind: MinimapNodeKind::File,
            },
            MinimapNode {
                rect: [1.0e6, 1.0e6, 1.0e6 + 50.0, 1.0e6 + 50.0],
                kind: MinimapNodeKind::File,
            },
        ];
        let mapping = MinimapMapping::fit(&nodes, [f32::NAN; 4], 220, 140);
        assert_close(
            mapping.world_to_map([-1.0e6, -1.0e6]),
            [48.0, 8.0],
            0.05,
            "min-угол далёкой сцены",
        );
        assert_close(
            mapping.world_to_map([1.0e6 + 50.0, 1.0e6 + 50.0]),
            [172.0, 132.0],
            0.05,
            "max-угол далёкой сцены",
        );
        for world in [[-1.0e6, -1.0e6], [1.0e6 + 50.0, 1.0e6 + 50.0], [25.0, 25.0]] {
            let map = mapping.world_to_map(world);
            assert!(
                map[0].is_finite() && map[1].is_finite(),
                "map конечен для {world:?}"
            );
            assert_close(
                mapping.world_to_map(mapping.map_to_world(map)),
                map,
                1.0,
                "round-trip на ±1e6",
            );
        }
    }

    /// NaN/±inf на входах fit и преобразований не дают NaN на выходе.
    #[test]
    fn nan_inputs_never_produce_nan() {
        let bad_nodes = [
            MinimapNode {
                rect: [f32::NAN; 4],
                kind: MinimapNodeKind::File,
            },
            MinimapNode {
                rect: [f32::INFINITY, 0.0, 100.0, f32::NAN],
                kind: MinimapNodeKind::Text,
            },
            MinimapNode {
                rect: [0.0, 0.0, f32::NEG_INFINITY, 10.0],
                kind: MinimapNodeKind::Group,
            },
        ];
        let mapping = MinimapMapping::fit(&bad_nodes, [f32::NAN, 0.0, 100.0, f32::NAN], 220, 140);
        for world in [
            [0.0, 0.0],
            [500.0, -500.0],
            [f32::NAN, 10.0],
            [f32::INFINITY, f32::NEG_INFINITY],
        ] {
            let map = mapping.world_to_map(world);
            assert!(
                map[0].is_finite() && map[1].is_finite(),
                "world_to_map({world:?}) = {map:?}"
            );
        }
        for map in [
            [0.0, 0.0],
            [110.0, 70.0],
            [f32::NAN, f32::NAN],
            [f32::INFINITY, -3.0],
        ] {
            let world = mapping.map_to_world(map);
            assert!(
                world[0].is_finite() && world[1].is_finite(),
                "map_to_world({map:?}) = {world:?}"
            );
        }
    }

    /// from_canvas: file (brokenLink → FileBroken, иначе File), text, group;
    /// link/widget (неизвестные типы) пропускаются; rect = [x, y, x+w, y+h].
    #[test]
    fn from_canvas_classifies_node_kinds() {
        let mut nodes = vec![
            node_of_type("file", "f1", 0.0, 0.0, 100.0, 100.0),
            node_of_type("file", "f2", 400.0, 0.0, 100.0, 100.0),
            node_of_type("text", "t1", 0.0, 200.0, 100.0, 100.0),
            node_of_type("group", "g1", 200.0, 200.0, 100.0, 100.0),
            node_of_type("link", "l1", 600.0, 0.0, 100.0, 100.0),
            node_of_type("widget", "w1", 600.0, 200.0, 100.0, 100.0),
        ];
        nodes[0].broken_link = Some(true);
        let input = MinimapInput::from_canvas(&canvas(nodes, vec![]), [0.0, 0.0, 500.0, 500.0]);
        assert_eq!(input.nodes.len(), 4, "link и widget пропущены");
        assert_eq!(input.nodes[0].kind, MinimapNodeKind::FileBroken);
        assert_eq!(input.nodes[0].rect, [0.0, 0.0, 100.0, 100.0]);
        assert_eq!(input.nodes[1].kind, MinimapNodeKind::File);
        assert_eq!(input.nodes[2].kind, MinimapNodeKind::Text);
        assert_eq!(input.nodes[3].kind, MinimapNodeKind::Group);
        assert_eq!(input.viewport_world, [0.0, 0.0, 500.0, 500.0]);
    }

    /// from_canvas: edges — пары центров валидных нод; связи на отсутствующие
    /// ноды пропускаются.
    #[test]
    fn from_canvas_edges_use_centers_and_skip_missing() {
        let input = MinimapInput::from_canvas(
            &canvas(
                vec![
                    node_of_type("file", "a", 0.0, 0.0, 20.0, 20.0),
                    node_of_type("text", "b", 100.0, 0.0, 20.0, 20.0),
                ],
                vec![
                    Edge::new("e1", "a", Some(Side::Top), "b", Some(Side::Top)),
                    Edge::new("e2", "a", None, "ghost", None),
                    Edge::new("e3", "ghost", None, "b", None),
                ],
            ),
            [0.0, 0.0, 150.0, 50.0],
        );
        assert_eq!(input.edges.len(), 1, "edges на ghost-ноды пропущены");
        assert_eq!(input.edges[0], ([10.0, 10.0], [110.0, 10.0]));
    }

    /// from_canvas: ноды с NaN-координатами и отрицательной шириной
    /// (min>max) пропускаются; точечная нода (min==max) — валидна.
    #[test]
    fn from_canvas_skips_invalid_rects() {
        let mut nan_node = node_of_type("text", "n1", 0.0, 0.0, 10.0, 10.0);
        nan_node.x = f32::NAN;
        let inverted = node_of_type("text", "n2", 0.0, 0.0, -50.0, 10.0);
        let point = node_of_type("text", "n3", 5.0, 5.0, 0.0, 0.0);
        let input = MinimapInput::from_canvas(
            &canvas(vec![nan_node, inverted, point], vec![]),
            [f32::NAN; 4],
        );
        assert_eq!(input.nodes.len(), 1);
        assert_eq!(input.nodes[0].rect, [5.0, 5.0, 5.0, 5.0]);
    }

    /// Растеризация пустой сцены с viewport: фон, белая рамка контента,
    /// прозрачные углы за радиусом скругления (план §6: «фон+рамка без нод»).
    #[test]
    fn render_background_frame_and_corners() {
        let minimap = Minimap::capture(&Canvas::default(), [0.0, 0.0, 100.0, 100.0], 220, 140);
        let image = minimap.render();
        assert_eq!(image.width, 220);
        assert_eq!(image.height, 140);
        assert_eq!(image.rgba.len(), 220 * 140 * 4);
        assert_eq!(pixel(&image, 110, 70), BG_COLOR, "центр — фон");
        // Рамка = map-прямоугольник контента [48, 8, 172, 132] (± округление)
        assert_eq!(
            pixel(&image, 110, 8),
            VIEWPORT_COLOR,
            "верхняя полоса рамки"
        );
        assert_eq!(
            pixel(&image, 110, 131),
            VIEWPORT_COLOR,
            "нижняя полоса рамки"
        );
        assert_eq!(pixel(&image, 48, 70), VIEWPORT_COLOR, "левая полоса рамки");
        for (x, y) in [(0, 0), (219, 0), (0, 139), (219, 139)] {
            assert_eq!(
                pixel(&image, x, y)[3],
                0,
                "угол ({x},{y}) за радиусом скругления — alpha 0"
            );
        }
    }

    /// Пиксель в центре каждой ноды = цвет типа; brokenLink — NODE_COLOR_BROKEN.
    #[test]
    fn render_node_center_pixel_by_kind() {
        let mut nodes = vec![
            node_of_type("file", "f", 0.0, 0.0, 100.0, 100.0),
            node_of_type("file", "fb", 200.0, 0.0, 100.0, 100.0),
            node_of_type("text", "t", 0.0, 200.0, 100.0, 100.0),
            node_of_type("group", "g", 200.0, 200.0, 100.0, 100.0),
        ];
        nodes[1].broken_link = Some(true);
        let minimap = Minimap::capture(&canvas(nodes, vec![]), [0.0, 0.0, 300.0, 300.0], 220, 140);
        let image = minimap.render();
        for (world, color) in [
            ([50.0, 50.0], NODE_COLOR_FILE),
            ([250.0, 50.0], NODE_COLOR_BROKEN),
            ([50.0, 250.0], NODE_COLOR_TEXT),
            ([250.0, 250.0], NODE_COLOR_GROUP),
        ] {
            assert_eq!(
                pixel_at(&image, minimap.world_to_map(world)),
                color,
                "центр ноды {world:?}"
            );
        }
    }

    /// Точечная нода растягивается до NODE_MIN_PX=2 от центра карты;
    /// соседние пиксели — фон.
    #[test]
    fn render_point_node_minimum_two_pixels() {
        let minimap = Minimap::capture(
            &canvas(
                vec![node_of_type("text", "p", 50.0, 50.0, 0.0, 0.0)],
                vec![],
            ),
            [0.0, 0.0, 100.0, 100.0],
            220,
            140,
        );
        let image = minimap.render();
        // Центр ноды (50,50) → map [110,70] → пиксели {109,110}×{69,70}
        assert_eq!(pixel(&image, 110, 70), NODE_COLOR_TEXT);
        assert_eq!(pixel(&image, 109, 69), NODE_COLOR_TEXT);
        assert_eq!(pixel(&image, 108, 70), BG_COLOR, "слева от 2×2 — фон");
        assert_eq!(pixel(&image, 112, 70), BG_COLOR, "справа от 2×2 — фон");
    }

    /// Edge между центрами нод виден при небольшой сцене (пиксель на линии
    /// между нодами, вне нод и вне рамки viewport).
    #[test]
    fn render_edges_visible_below_threshold() {
        let minimap = Minimap::capture(
            &canvas(
                vec![
                    node_of_type("file", "a", 0.0, 0.0, 20.0, 20.0),
                    node_of_type("file", "b", 100.0, 0.0, 20.0, 20.0),
                    node_of_type("text", "c", 50.0, 100.0, 20.0, 20.0),
                ],
                vec![Edge::new("e1", "a", None, "b", None)],
            ),
            [-20.0, -20.0, 140.0, 140.0],
            220,
            140,
        );
        let image = minimap.render();
        // Центры a=(10,10) и b=(110,10) → map y≈31, линия между нодами
        let map = minimap.world_to_map([60.0, 10.0]);
        assert_eq!(
            pixel_at(&image, map),
            EDGE_COLOR,
            "пиксель на линии между центрами a и b"
        );
        assert!(contains_color(&image, EDGE_COLOR));
    }

    /// Порог edges (план §6): при 499 нодах линии рисуются, при 500 — нет.
    /// Edge между нодами середины сетки — линия вдали от рамки viewport
    /// (верхний ряд совпал бы с её верхней полосой).
    #[test]
    fn render_edges_hidden_at_threshold() {
        // Ноды 45-й колонки строк 4 и 5: центры (13550,1250) и (13550,1550) —
        // вертикальная линия в map ≈ x=192, y 67..71, пиксель (192,69) вне нод.
        let edges = vec![Edge::new("e1", "n245", None, "n295", None)];
        let viewport = [0.0, 0.0, 15_000.0, 3_000.0];
        let below = Minimap::capture(&canvas(grid_nodes(499), edges.clone()), viewport, 220, 140);
        let below_image = below.render();
        assert_eq!(
            pixel(&below_image, 192, 69),
            EDGE_COLOR,
            "пиксель на линии между n245 и n295"
        );
        assert!(
            contains_color(&below_image, EDGE_COLOR),
            "при 499 нодах edges видны"
        );
        let above = Minimap::capture(&canvas(grid_nodes(500), edges), viewport, 220, 140);
        assert!(
            !contains_color(&above.render(), EDGE_COLOR),
            "при 500 нодах edges скрыты"
        );
    }

    /// Вырожденный буфер (нулевая сторона) и нереально большой — EMPTY;
    /// fit на нулевом буфере не паникует и даёт конечные значения.
    #[test]
    fn render_degenerate_buffer_returns_empty() {
        let scene = canvas(
            vec![node_of_type("text", "t", 0.0, 0.0, 100.0, 100.0)],
            vec![],
        );
        for (w, h) in [(0, 0), (0, 140), (220, 0), (99_999, 140)] {
            let minimap = Minimap::capture(&scene, [0.0, 0.0, 100.0, 100.0], w, h);
            assert_eq!(minimap.render(), MinimapImage::EMPTY, "буфер {w}×{h}");
        }
        let mapping = MinimapMapping::fit(&[], [0.0, 0.0, 100.0, 100.0], 0, 0);
        let map = mapping.world_to_map([50.0, 50.0]);
        assert!(
            map[0].is_finite() && map[1].is_finite(),
            "нулевой буфер: {map:?}"
        );
    }

    /// Невалидный viewport (NaN): рамка не рисуется — только фон.
    #[test]
    fn render_invalid_viewport_no_frame() {
        let minimap = Minimap::capture(&Canvas::default(), [f32::NAN; 4], 220, 140);
        let image = minimap.render();
        assert_eq!(pixel(&image, 110, 70), BG_COLOR);
        assert!(!contains_color(&image, VIEWPORT_COLOR));
    }

    /// Нулевой viewport (свёрнутое окно): без паники, рамка вырождается
    /// в видимую точку минимального размера (как ноды).
    #[test]
    fn render_zero_viewport_frame_is_point() {
        let minimap = Minimap::capture(&Canvas::default(), [0.0, 0.0, 0.0, 0.0], 220, 140);
        let image = minimap.render();
        // Identity-масштаб: map([0,0]) = центр буфера [110,70], рамка 2×2
        assert_eq!(pixel(&image, 110, 70), VIEWPORT_COLOR);
    }

    /// Hit-test: клик в центр миникарты → world-центр контента;
    /// клик в угол контента — соответствующий world-угол.
    #[test]
    fn hit_test_center_click_returns_content_center() {
        let minimap = Minimap::capture(
            &canvas(
                vec![node_of_type("file", "f", 0.0, 0.0, 100.0, 100.0)],
                vec![],
            ),
            [0.0, 0.0, 100.0, 100.0],
            220,
            140,
        );
        assert_close(
            minimap.map_to_world([110.0, 70.0]),
            [50.0, 50.0],
            EPS,
            "клик в центр миникарты",
        );
        assert_close(
            minimap.map_to_world([48.0, 8.0]),
            [0.0, 0.0],
            0.5,
            "клик в min-угол контента",
        );
    }

    /// set_viewport: input обновляется, mapping пересобирается по нодам ∪
    /// новый viewport; hit-test и round-trip согласованы с новой геометрией.
    #[test]
    fn set_viewport_rebuilds_mapping() {
        let scene = canvas(
            vec![node_of_type("file", "f", 0.0, 0.0, 100.0, 100.0)],
            vec![],
        );
        let mut minimap = Minimap::capture(&scene, [0.0, 0.0, 100.0, 100.0], 220, 140);
        // Расширяем viewport: контент = [-100,-100, 200,200], центр (50,50)
        minimap.set_viewport([-100.0, -100.0, 200.0, 200.0]);
        assert_eq!(
            minimap.input().viewport_world,
            [-100.0, -100.0, 200.0, 200.0]
        );
        assert_close(
            minimap.map_to_world([110.0, 70.0]),
            [50.0, 50.0],
            EPS,
            "центр карты → новый центр контента",
        );
        // half = 150 → scale = 124/300; min-угол: x центрирован (48), y = 8
        assert_close(
            minimap.world_to_map([-100.0, -100.0]),
            [48.0, 8.0],
            EPS,
            "новый min-угол контента",
        );
        let map = minimap.world_to_map([37.5, -12.25]);
        assert_close(
            minimap.world_to_map(minimap.map_to_world(map)),
            map,
            1.0,
            "round-trip после set_viewport",
        );
    }

    /// Критерий производительности (SPEC/TASKS T13: 5000 нод — единицы мс):
    /// порог теста мягкий (debug/CI), ловит только грубые O(w·h) на ноду;
    /// при 5000 ≥ 500 edges не рисуются.
    #[test]
    fn render_5000_nodes_stays_fast() {
        let nodes: Vec<Node> = (0..5000)
            .map(|i| {
                node_of_type(
                    "file",
                    &format!("n{i}"),
                    (i % 100) as f32 * 260.0,
                    (i / 100) as f32 * 260.0,
                    240.0,
                    240.0,
                )
            })
            .collect();
        let minimap = Minimap::capture(
            &canvas(nodes, vec![]),
            [0.0, 0.0, 26_000.0, 13_000.0],
            220,
            140,
        );
        let started = Instant::now();
        let image = minimap.render();
        let elapsed = started.elapsed();
        assert_eq!(image.rgba.len(), 220 * 140 * 4);
        assert!(
            elapsed.as_millis() < 100,
            "render 5000 нод занял {elapsed:?}"
        );
        assert!(!contains_color(&image, EDGE_COLOR), "5000 ≥ порога edges");
    }
}
