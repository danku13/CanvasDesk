//! Карточки нод: чистая раскладка/цвета + instanced SDF-пайплайн (T4).
//!
//! Скруглённый прямоугольник, тень и рамка выделения рисуются SDF во фрагментном
//! шейдере (shaders/cards.wgsl); все карточки кадра — один instanced draw.

use canvas_core::Node;
use canvas_core::NodeKind;

use crate::camera::Camera;
use crate::camera::Vec2;
use crate::markdown;
use crate::theme::ThemeColors;
use crate::Color;

/// Высота заголовка карточки в world-пикселях.
pub const HEADER_HEIGHT: f32 = 28.0;
/// Радиус скругления в world-пикселях.
pub const CORNER_RADIUS: f32 = 8.0;

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

/// Цвет заливки карточки: пресет "1".."6" или "#RRGGBB" по JSON Canvas spec,
/// иначе заливка по умолчанию из темы.
pub fn card_color(node: &Node, theme: &ThemeColors) -> [f32; 4] {
    named_color(node.color.as_deref()).unwrap_or(theme.card_fill)
}

/// Цвет пресета палитры JSON Canvas ("1".."6") — для меню выбора цвета (T7).
pub fn preset_color(preset: &str) -> Option<[f32; 4]> {
    PRESET_COLORS
        .iter()
        .find(|(key, _)| *key == preset)
        .map(|(_, rgba)| *rgba)
}

/// Заголовок карточки: имя файла из пути / подпись группы / первая строка
/// текста / label. Подпись группы — как есть: markdown-стриппинг к label
/// не применяется (это заголовок, а не тело заметки).
pub fn title_for(node: &Node) -> String {
    if let Some(file) = &node.file {
        let name = file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(file.as_str());
        return name.to_owned();
    }
    // Группа: подпись в label; без неё — нейтральный дефолт
    if node.kind() == NodeKind::Group {
        return node
            .label
            .clone()
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| "Группа".to_owned());
    }
    if let Some(text) = &node.text {
        if let Some(line) = text.lines().next().filter(|line| !line.is_empty()) {
            // Маркеры форматирования (**...**, ==...==) и ATX-заголовок (# ...)
            // в заголовке карточки не показываем
            return markdown::strip(crate::gfm::strip_atx(line));
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
/// Группа рисуется рамкой с полупрозрачной заливкой (theme.group_*) —
/// карточкой-контейнером, а не содержимым.
pub fn card_instance(node: &Node, selected: bool, theme: &ThemeColors) -> CardInstance {
    let broken = node.broken_link == Some(true);
    let group = node.kind() == NodeKind::Group;
    let border = if selected {
        SELECTION_BORDER
    } else if broken {
        BROKEN_BORDER
    } else if group {
        theme.group_border
    } else {
        [0.0; 4]
    };
    CardInstance {
        pos: [node.x, node.y],
        size: [node.width, node.height],
        fill: if group {
            theme.group_fill
        } else {
            card_color(node, theme)
        },
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
    theme: &ThemeColors,
) -> Vec<CardInstance> {
    indices
        .iter()
        .filter_map(|&index| {
            let node = canvas.nodes.get(index)?;
            Some(card_instance(node, selected == Some(index), theme))
        })
        .collect()
}

// --- Виджет-ноды (CR-004) ---

/// Подсветка хрома виджет-ноды при hover (CR-004 v1): лёгкая акцентная
/// полоса заголовка — единственный видимый след «карточки» (заливка и
/// тень прозрачны); рамка остаётся drag-зоной без визуального хрома.
pub const WIDGET_CHROME_HOVER_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.10];

/// Сделать инстанс карточки виджет-ноды полностью прозрачным (CR-004):
/// заливка — нулевая альфа, тень — выключена (params.w). Рамка выделения
/// (params.y) и битой ссылки сохраняются — согласовано с CR-006
/// (выделение = рамка по контуру, без заливки). Вызывается приложением
/// ТОЛЬКО для виджетов с видимым контентом (live-HWND или снапшот):
/// placeholder битого пакета остаётся серой карточкой (WIDGETS.md §10).
pub fn make_widget_transparent(inst: &mut CardInstance) {
    inst.fill = [0.0, 0.0, 0.0, 0.0];
    inst.params[3] = 1.0;
}

/// Инстанс подсветки полосы заголовка виджет-ноды при hover (CR-004 v1):
/// квад высотой HEADER_HEIGHT от верха ноды, без тени, радиус карточки.
/// Рисуется сразу после прозрачной карточки — под контентом виджета.
pub fn widget_header_hover_instance(node: &Node) -> CardInstance {
    CardInstance {
        pos: [node.x, node.y],
        size: [node.width, HEADER_HEIGHT.min(node.height)],
        fill: WIDGET_CHROME_HOVER_FILL,
        border: [0.0; 4],
        params: [CORNER_RADIUS, 0.0, 0.0, 1.0],
    }
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
/// Диаметр кружка порта ноды при hover в world-px (минимум).
pub const PORT_DOT: f32 = 10.0;
/// Диаметр кружка порта по зоне захвата (CR-003): зона больше — кружок
/// заметно больше (визуальный отклик настройки), но не гигантский.
pub const PORT_DOT_MAX: f32 = 26.0;

/// Диаметр кружка порта для заданной зоны захвата (CR-003), world-px.
/// Зона в экранных px == world-px при zoom 1 — соответствие наглядно;
/// сверху кламп, чтобы крупные зоны не рисовали монетки.
pub fn port_dot_diameter(zone_px: f32) -> f32 {
    zone_px.clamp(PORT_DOT, PORT_DOT_MAX)
}
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
/// Период пунктира в единицах диаметра кружка (черта + пропуск).
const DASH_PERIOD: f32 = 8.0;
/// Доля периода пунктира, занятая чертой.
const DASH_DUTY: f32 = 0.6;
/// Шаг одиночных точек (стиль «точки») в единицах диаметра.
const DOT_SPACING: f32 = 3.0;

// --- Режим фокуса (T23, brainstorm-focus) ---

/// Цвет подсвеченной фокусом связи — акцент (един для тёмной/светлой
/// темы, как рамка выделения и группы; альфа модулируется пульсом).
pub const FOCUS_EDGE_COLOR: [f32; 4] = [0.396, 0.612, 0.969, 1.0];
/// Доля яркости, остающаяся у НЕ-фокусных элементов при dim = 1
/// (план T23 §1: «~35% яркости»).
pub const FOCUS_DIM_FLOOR: f32 = 0.35;
/// Прибавка толщины фокусной связи в world-px (без пульса).
pub const FOCUS_EDGE_BOOST: f32 = 1.2;
/// Амплитуда «дыхания» толщины фокусной связи в world-px.
pub const FOCUS_EDGE_PULSE_BOOST: f32 = 0.8;

/// Вид фокуса для кадра (T23): подсвеченные ноды/связи (отсортированные
/// индексы из `canvas_core::FocusSet`), степень затемнения прочего и фаза
/// «дыхания» подсвеченных связей. Данные живёт в приложении — рендер
/// получает только срезы; вычисляется на каждый кадр (динамика без
/// инвалидаций: драги/удаления подхватываются сами).
///
/// `dim = 0` (EMPTY) — режим выключен: кадр идентичен прежнему поведению.
#[derive(Debug, Clone, Copy)]
pub struct FocusView<'a> {
    /// Подсвеченные ноды (семя + соседи, может включать выделенную —
    /// её добавляет приложение, приоритет выделения над фокусом).
    pub nodes: &'a [usize],
    /// Подсвеченные связи (инцидентные семени).
    pub edges: &'a [usize],
    /// Степень затемнения прочих элементов: 0 — выключено, 1 — полное.
    pub dim: f32,
    /// Фаза «дыхания» подсвеченных связей 0..1 (0 — нет).
    pub pulse: f32,
}

impl FocusView<'_> {
    /// Выключенный фокус: пустые срезы, dim = 0 — ничего не меняется.
    pub const EMPTY: FocusView<'static> = FocusView {
        nodes: &[],
        edges: &[],
        dim: 0.0,
        pulse: 0.0,
    };

    /// Нода подсвечена? (бинарный поиск — срез отсортирован)
    pub fn has_node(&self, index: usize) -> bool {
        self.nodes.binary_search(&index).is_ok()
    }

    /// Связь подсвечена?
    pub fn has_edge(&self, index: usize) -> bool {
        self.edges.binary_search(&index).is_ok()
    }

    /// Множитель альфы НЕ-фокусных элементов при текущем dim
    /// (1.0 — не трогать; dim=1 → FOCUS_DIM_FLOOR).
    pub fn dim_factor(&self) -> f32 {
        1.0 - self.dim * (1.0 - FOCUS_DIM_FLOOR)
    }
}

/// Затемнение инстанса карточки (T23): альфа заливки и рамки × фактор.
/// Выделенные/фокусные инстансы не затемняются — вызов только для прочих
/// (приоритет выделения над фокусом, план T23 §7).
pub fn dim_instance(inst: &mut CardInstance, factor: f32) {
    if factor >= 1.0 {
        return;
    }
    inst.fill[3] *= factor;
    inst.border[3] *= factor;
}

/// Затемнение цвета текста glyphon (T23): альфа × фактор с клампом.
/// Применяется к `TextArea::default_color` заголовков/иконок/тел/лейблов
/// не-фокусных элементов.
pub fn dim_color(color: Color, factor: f32) -> Color {
    if factor >= 1.0 {
        return color;
    }
    let alpha = (color.a() as f32 * factor).round().clamp(0.0, 255.0) as u8;
    Color::rgba(color.r(), color.g(), color.b(), alpha)
}

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

/// Общая длина полилинии по длине дуги.
fn polyline_length(points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum()
}

/// Точка полилинии на дистанции `s` от начала (по дуге).
fn polyline_point_at(points: &[[f32; 2]], s: f32) -> Option<[f32; 2]> {
    let mut acc = 0.0f32;
    for w in points.windows(2) {
        let seg = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
        let len = seg[0].hypot(seg[1]);
        if len < f32::EPSILON {
            continue;
        }
        if acc + len >= s {
            let t = ((s - acc) / len).clamp(0.0, 1.0);
            return Some([w[0][0] + seg[0] * t, w[0][1] + seg[1] * t]);
        }
        acc += len;
    }
    points.last().copied()
}

/// Позиции центров кружков линии связи по полилинии с учётом стиля
/// (чистая функция — тестируется без GPU). Сплошная — плотная цепочка,
/// пунктир — черта/пропуск по периоду, точки — одиночные кружки с шагом.
pub fn line_pattern_dots(
    points: &[[f32; 2]],
    style: canvas_core::EdgeLineStyle,
    d: f32,
) -> Vec<[f32; 2]> {
    let total = polyline_length(points);
    if total <= f32::EPSILON {
        return points.first().copied().into_iter().collect();
    }
    let step = (d * 0.8).max(0.5);
    let mut out = Vec::new();
    let push_at = |s: f32, out: &mut Vec<[f32; 2]>| {
        if let Some(p) = polyline_point_at(points, s) {
            out.push(p);
        }
    };
    match style {
        canvas_core::EdgeLineStyle::Solid => {
            let mut s = 0.0;
            while s <= total {
                push_at(s, &mut out);
                s += step;
            }
        }
        canvas_core::EdgeLineStyle::Dashed => {
            let period = d * DASH_PERIOD;
            let on = period * DASH_DUTY;
            let mut start = 0.0;
            while start <= total {
                let end = (start + on).min(total);
                let mut s = start;
                while s <= end {
                    push_at(s, &mut out);
                    s += step;
                }
                start += period;
            }
        }
        canvas_core::EdgeLineStyle::Dotted => {
            let spacing = (d * DOT_SPACING).max(1.0);
            let mut s = 0.0;
            while s <= total {
                push_at(s, &mut out);
                s += spacing;
            }
        }
    }
    out
}

/// Кружки вдоль полилинии + опционально стрелка на конце по направлению
/// последнего сегмента (применяется и к огибающим маршрутам).
fn polyline_dots(
    points: &[[f32; 2]],
    style: canvas_core::EdgeLineStyle,
    d: f32,
    fill: [f32; 4],
    arrow: bool,
    out: &mut Vec<CardInstance>,
) {
    if points.is_empty() {
        return;
    }
    for point in line_pattern_dots(points, style, d) {
        out.push(dot(point, d, fill));
    }
    if !arrow {
        return;
    }
    let Some(last) = points.last() else { return };
    let Some(prev) = points.get(points.len().saturating_sub(2)) else {
        return;
    };
    let tangent = [last[0] - prev[0], last[1] - prev[1]];
    let len = tangent[0].hypot(tangent[1]);
    if len < f32::EPSILON {
        return;
    }
    let back = [-tangent[0] / len, -tangent[1] / len];
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
                [last[0] + dir[0] * dist, last[1] + dir[1] * dist],
                d,
                fill,
            ));
        }
    }
}

/// Инстансы всех связей канваса (T8): кривые-«чётки» и стрелки.
/// Выделенная связь (`selected` — индекс в `canvas.edges`) ярче и толще.
/// Стиль/толщина — из полей связи `edgeStyle`/`edgeWidth` (дефолты: сплошная,
/// средняя). Висячие связи (без ноды) пропускаются. `avoid` — обход
/// посторонних нод (глобальная настройка), рендер идёт по огибающей
/// полилинии. Добавлять ПЕРЕД инстансами карточек — связи под нодами
/// (порядок в буфере = порядок рисования).
///
/// T23: связи из `focus.edges` — акцентным цветом (альфа дышит пульсом)
/// и толще (`FOCUS_EDGE_BOOST` + пульс); прочие при dim > 0 — затемнены.
/// Выделенная связь рисуется как раньше (приоритет выделения).
pub fn build_edge_instances(
    canvas: &canvas_core::Canvas,
    selected: Option<usize>,
    avoid: bool,
    focus: &FocusView,
    hidden_edge: Option<usize>,
    hidden_ids: &std::collections::HashSet<&str>,
) -> Vec<CardInstance> {
    let mut out = Vec::new();
    for (index, edge) in canvas.edges.iter().enumerate() {
        // CR-002: перепривязываемая связь скрыта — её место занимает
        // резиновая линия от неподвижного конца
        if hidden_edge == Some(index) {
            continue;
        }
        // FR-011: связи инцидентные скрытым нодам (свернутые поддеревья)
        // не рисуются
        if hidden_ids.contains(edge.from_node.as_str())
            || hidden_ids.contains(edge.to_node.as_str())
        {
            continue;
        }
        let Some(points) = canvas_core::edge_polyline(canvas, edge, avoid, EDGE_RENDER_SEGMENTS)
        else {
            continue;
        };
        let is_selected = selected == Some(index);
        let in_focus = focus.has_edge(index);
        let (fill, d) = if is_selected {
            (
                SELECTION_BORDER,
                edge.thickness.unwrap_or_default().dot() + (EDGE_DOT_SELECTED - EDGE_DOT),
            )
        } else if in_focus {
            // Альфа дышит вместе с толщиной: статика 0.75, пик 1.0
            let mut fill = FOCUS_EDGE_COLOR;
            fill[3] = 0.75 + 0.25 * focus.pulse;
            let d = edge.thickness.unwrap_or_default().dot()
                + FOCUS_EDGE_BOOST
                + focus.pulse * FOCUS_EDGE_PULSE_BOOST;
            (fill, d)
        } else {
            let mut fill = named_color(edge.color.as_deref()).unwrap_or(EDGE_COLOR);
            if focus.dim > 0.0 {
                fill[3] *= focus.dim_factor();
            }
            (fill, edge.thickness.unwrap_or_default().dot())
        };
        let style = edge.style.unwrap_or(canvas_core::EdgeLineStyle::Solid);
        polyline_dots(&points, style, d, fill, true, &mut out);
    }
    out
}

/// Порты ноды при hover (T8): 4 кружка по центрам сторон, поверх карточек.
/// `zone_px` — зона захвата из настроек (CR-003): диаметр кружка следует
/// за зоной (`port_dot_diameter`). У групп портов нет: edge-drag с группы
/// не начинается (группа — контейнер, не конечная точка связи).
pub fn build_port_instances(
    canvas: &canvas_core::Canvas,
    node: usize,
    zone_px: f32,
) -> Vec<CardInstance> {
    let dot_d = port_dot_diameter(zone_px);
    let mut out = Vec::with_capacity(4);
    if let Some(node) = canvas.nodes.get(node) {
        if node.kind() == NodeKind::Group {
            return out;
        }
        for side in [
            canvas_core::Side::Top,
            canvas_core::Side::Right,
            canvas_core::Side::Bottom,
            canvas_core::Side::Left,
        ] {
            out.push(dot(
                canvas_core::port_point(node, side),
                dot_d,
                SELECTION_BORDER,
            ));
        }
    }
    out
}

/// Хэндлы концов ВЫДЕЛЕННОЙ связи (CR-002): кружки на обоих концах —
/// захват хэндла начинает drag перепривязки. Положения — резолв
/// `edge_endpoint` (та же геометрия, что у линии). Диаметр — как у портов
/// (`port_dot_diameter`); цвет — рамка выделения. Висячая/невалидная — пусто.
pub fn build_edge_handle_instances(
    canvas: &canvas_core::Canvas,
    edge_index: usize,
    zone_px: f32,
) -> Vec<CardInstance> {
    let dot_d = port_dot_diameter(zone_px);
    let mut out = Vec::with_capacity(2);
    for end in [canvas_core::EdgeEnd::From, canvas_core::EdgeEnd::To] {
        if let Some((_, point)) = canvas_core::edge_endpoint(canvas, edge_index, end) {
            out.push(dot(point, dot_d, SELECTION_BORDER));
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

// --- Призраки зоны дропа (T9) ---
//
// Квады-«призраки» сетки вставки на время DragOver: полупрозрачный
// акцент (та же гамма, что SELECTION_BORDER/DRAFT_COLOR), без тени —
// это мелкие overlay-квады в FrameOverlay.instances (world-space).

/// Заливка призрака карточки дропа — акцент, полупрозрачный.
pub const DROP_GHOST_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.10];
/// Рамка призрака карточки дропа — акцент заметнее заливки.
pub const DROP_GHOST_BORDER: [f32; 4] = [0.396, 0.612, 0.969, 0.7];
/// Рамка зоны дропа (bbox сетки) — акцент, средняя прозрачность.
pub const DROP_ZONE_BORDER: [f32; 4] = [0.396, 0.612, 0.969, 0.5];

/// Призрак одной карточки дропа: pos/size заданы сеткой, заливка и рамка
/// дропа, без тени (params.w = 1 — малые квады не отбрасывают).
pub fn drop_ghost(pos: Vec2, card_size: [f32; 2]) -> CardInstance {
    CardInstance {
        pos,
        size: card_size,
        fill: DROP_GHOST_FILL,
        border: DROP_GHOST_BORDER,
        params: [CORNER_RADIUS, 0.0, 0.0, 1.0],
    }
}

/// Призраки сетки дропа по позициям (не длиннее `cap` — превью дешёвое,
/// DROP_PREVIEW_MAX обрезает гигантские выборки из Explorer).
pub fn drop_ghosts(positions: &[Vec2], card_size: [f32; 2], cap: usize) -> Vec<CardInstance> {
    positions
        .iter()
        .take(cap)
        .map(|&pos| drop_ghost(pos, card_size))
        .collect()
}

/// Рамка зоны дропа: bbox сетки призраков (pos_i..pos_i+card_size),
/// расширенный на gap/2 со всех сторон; заливка полностью прозрачная
/// (только рамка), без тени. Пустые позиции — None (рисовать нечего).
pub fn drop_zone_frame(positions: &[Vec2], card_size: [f32; 2], gap: f32) -> Option<CardInstance> {
    let first = *positions.first()?;
    let margin = gap / 2.0;
    let mut min = [first[0], first[1]];
    let mut max = [first[0] + card_size[0], first[1] + card_size[1]];
    for &pos in &positions[1..] {
        min[0] = min[0].min(pos[0]);
        min[1] = min[1].min(pos[1]);
        max[0] = max[0].max(pos[0] + card_size[0]);
        max[1] = max[1].max(pos[1] + card_size[1]);
    }
    Some(CardInstance {
        pos: [min[0] - margin, min[1] - margin],
        size: [
            (max[0] - min[0]) + margin * 2.0,
            (max[1] - min[1]) + margin * 2.0,
        ],
        fill: [0.0; 4],
        border: DROP_ZONE_BORDER,
        params: [CORNER_RADIUS, 0.0, 0.0, 1.0],
    })
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

    /// FR-011: пустой набор скрытых нод для вызовов build_edge_instances.
    fn no_hidden() -> std::collections::HashSet<&'static str> {
        std::collections::HashSet::new()
    }

    /// Пресеты "1".."6" отличаются от дефолта и друг от друга.
    #[test]
    fn color_presets() {
        let theme = ThemeColors::dark();
        let mut node = Node::text("n", "t", 0.0, 0.0);
        assert_eq!(card_color(&node, &theme), theme.card_fill);
        node.color = Some("3".into());
        assert_eq!(card_color(&node, &theme), PRESET_COLORS[2].1);
        node.color = Some("6".into());
        assert_eq!(card_color(&node, &theme), PRESET_COLORS[5].1);
        node.color = Some("9".into());
        assert_eq!(card_color(&node, &theme), theme.card_fill);
    }

    /// Hex-цвет "#RRGGBB" парсится; битый — дефолт темы.
    #[test]
    fn color_hex() {
        let theme = ThemeColors::dark();
        let mut node = Node::text("n", "t", 0.0, 0.0);
        node.color = Some("#ff8000".into());
        let rgba = card_color(&node, &theme);
        assert!((rgba[0] - 1.0).abs() < 1e-3);
        assert!((rgba[1] - 128.0 / 255.0).abs() < 1e-3);
        assert!((rgba[2] - 0.0).abs() < 1e-3);
        node.color = Some("#zzz".into());
        assert_eq!(card_color(&node, &theme), theme.card_fill);
        node.color = Some("#12345".into());
        assert_eq!(card_color(&node, &theme), theme.card_fill);
    }

    /// Заливка по умолчанию зависит от темы (светлая ≠ тёмная).
    #[test]
    fn default_fill_follows_theme() {
        let node = Node::text("n", "t", 0.0, 0.0);
        let dark = card_color(&node, &ThemeColors::dark());
        let light = card_color(&node, &ThemeColors::light());
        assert_eq!(dark, ThemeColors::dark().card_fill);
        assert_ne!(dark, light);
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
        // ATX-маркер заголовка не показываем буквально
        let heading = Node::text("n", "## Заголовок заметки\nтело", 0.0, 0.0);
        assert_eq!(title_for(&heading), "Заголовок заметки");
        let no_space = Node::text("n", "#заголовок", 0.0, 0.0);
        assert_eq!(title_for(&no_space), "#заголовок");
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

    /// CR-004: прозрачность виджет-ноды — заливка нулевая, тень выключена;
    /// рамка выделения сохраняется; обычные ноды функция не трогает.
    #[test]
    fn widget_transparency() {
        let theme = ThemeColors::dark();
        let widget = Node::widget(
            "w",
            canvas_core::CanvasdeskExt {
                widget_id: "com.canvasdesk.clock".into(),
                props: Default::default(),
            },
            "Clock",
            10.0,
            20.0,
            320.0,
            200.0,
        );
        // Выделенная виджет-нода: рамка есть, заливки/тени нет
        let mut inst = card_instance(&widget, true, &theme);
        assert_eq!(inst.border, SELECTION_BORDER, "рамка выделения сохранена");
        make_widget_transparent(&mut inst);
        assert_eq!(inst.fill, [0.0; 4], "заливка полностью прозрачна");
        assert_eq!(inst.params[3], 1.0, "тень выключена");
        assert_eq!(inst.params[1], 1.0, "признак выделения не тронут");
        // Не выделенная: рамки нет, остальное так же
        let mut inst = card_instance(&widget, false, &theme);
        make_widget_transparent(&mut inst);
        assert_eq!(inst.fill, [0.0; 4]);
        assert_eq!(inst.params[3], 1.0);
        // Обычная текст-нода: прозрачность не применяется (защита от
        // случайного вызова не на том типе — просто инвариант функции)
        let mut text_inst = card_instance(&Node::text("t", "x", 0.0, 0.0), false, &theme);
        make_widget_transparent(&mut text_inst);
        assert_eq!(text_inst.fill, [0.0; 4]);
        // Хром-подсветка hover: полоса заголовка, без тени
        let hover = widget_header_hover_instance(&widget);
        assert_eq!(hover.pos, [10.0, 20.0]);
        assert_eq!(hover.size, [320.0, HEADER_HEIGHT]);
        assert_eq!(hover.fill, WIDGET_CHROME_HOVER_FILL);
        assert_eq!(hover.params[3], 1.0, "без тени");
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

    /// Инстансы связей (T8): кружки вдоль полилинии + усы стрелки; выделенная —
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

        let instances = build_edge_instances(&canvas, None, false, &FocusView::EMPTY, None, &no_hidden());
        assert!(
            instances.len() > ARROW_DOTS * 2,
            "кружки линии + стрелка: {}",
            instances.len()
        );
        // Все инстансы — кружки без тени цвета пресета "2"
        let expected = named_color(Some("2")).expect("пресет");
        for inst in &instances {
            assert_eq!(inst.params[0], inst.size[0] / 2.0, "круг: radius = d/2");
            assert_eq!(inst.params[3], 1.0, "без тени");
            assert_eq!(inst.fill, expected);
            assert_eq!(inst.size[0], EDGE_DOT);
        }
        // Первая точка — в порту from (ресэмплинг начинается с p0)
        assert_eq!(
            instances[0].pos,
            [100.0 - EDGE_DOT / 2.0, 50.0 - EDGE_DOT / 2.0]
        );

        // Выделенная связь — акцент и толще (шаг ресэмплинга зависит от d,
        // поэтому число кружков иное — сравниваем только атрибуты)
        let selected = build_edge_instances(&canvas, Some(0), false, &FocusView::EMPTY, None, &no_hidden());
        assert!(selected.len() > ARROW_DOTS * 2);
        assert_eq!(selected[0].fill, SELECTION_BORDER);
        assert_eq!(selected[0].size[0], EDGE_DOT_SELECTED);
    }

    /// T23: фокусная связь — акцентный цвет (альфа дышит пульсом), толщина
    /// больше базовой на FOCUS_EDGE_BOOST (+пульс); при dim>0 прочие связи
    /// затемнены до dim_factor; выделенная связь приоритетнее фокуса.
    #[test]
    fn edge_instances_focus_highlight_and_dim() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("b", "C:/b.png", 500.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("c", "C:/c.png", 0.0, 500.0, 100.0, 100.0));
        canvas.add_edge(canvas_core::Edge::new("e1", "a", None, "b", None));
        canvas.add_edge(canvas_core::Edge::new("e2", "b", None, "c", None));

        let dim_view = FocusView {
            nodes: &[0, 1],
            edges: &[0],
            dim: 1.0,
            pulse: 0.5,
        };
        let inst = build_edge_instances(&canvas, None, false, &dim_view, None, &no_hidden());
        // Фокусная e1 (первая в буфере): акцент, альфа 0.75 + 0.25·пульс,
        // толщина базовая + буст + пульс
        let base_d = canvas_core::EdgeThickness::Medium.dot();
        assert_eq!(
            inst[0].fill,
            [
                FOCUS_EDGE_COLOR[0],
                FOCUS_EDGE_COLOR[1],
                FOCUS_EDGE_COLOR[2],
                0.875
            ]
        );
        assert!(
            (inst[0].size[0] - (base_d + FOCUS_EDGE_BOOST + 0.5 * FOCUS_EDGE_PULSE_BOOST)).abs()
                < 1e-3,
            "толщина фокусной: {}",
            inst[0].size[0]
        );
        // Не-фокусная e2: цвет дефолтный, альфа × dim_factor, толщина базовая
        // (фактор — через ту же формулу dim_factor: бит-точное сравнение)
        let factor = dim_view.dim_factor();
        let dimmed_color = [
            EDGE_COLOR[0],
            EDGE_COLOR[1],
            EDGE_COLOR[2],
            EDGE_COLOR[3] * factor,
        ];
        let e2_first = inst
            .iter()
            .position(|i| i.fill == dimmed_color)
            .expect("e2 затемнена");
        assert!((inst[e2_first].size[0] - base_d).abs() < 1e-3);

        // Выделенная e2 при том же фокусе — как раньше: акцент выделения,
        // НЕ затемнена
        let sel = build_edge_instances(&canvas, Some(1), false, &dim_view, None, &no_hidden());
        let sel_e2 = sel
            .iter()
            .find(|i| i.fill == SELECTION_BORDER)
            .expect("выделенная не затемнена");
        assert_eq!(sel_e2.size[0], base_d + (EDGE_DOT_SELECTED - EDGE_DOT));

        // dim = 0 — выключено: все связи обычной яркости
        let off = FocusView {
            nodes: &[0],
            edges: &[],
            dim: 0.0,
            pulse: 0.0,
        };
        let plain = build_edge_instances(&canvas, None, false, &off, None, &no_hidden());
        assert!(plain.iter().any(|i| i.fill == EDGE_COLOR));
    }

    /// T23: FocusView::dim_factor линейно мапит dim в [1, FLOOR];
    /// dim_instance/dim_color гасят альфу, фактор ≥ 1 — нетронуто.
    #[test]
    fn focus_dim_helpers() {
        let mut view = FocusView::EMPTY;
        assert_eq!(view.dim_factor(), 1.0);
        view.dim = 1.0;
        assert!((view.dim_factor() - FOCUS_DIM_FLOOR).abs() < 1e-4);
        view.dim = 0.5;
        assert!((view.dim_factor() - (1.0 + FOCUS_DIM_FLOOR) / 2.0).abs() < 1e-4);

        let mut inst = CardInstance {
            pos: [0.0; 2],
            size: [10.0; 2],
            fill: [1.0, 0.5, 0.25, 1.0],
            border: [1.0, 1.0, 1.0, 0.5],
            params: [5.0, 0.0, 0.0, 1.0],
        };
        dim_instance(&mut inst, 1.0);
        assert_eq!(inst.fill[3], 1.0, "фактор 1 — нет изменений");
        dim_instance(&mut inst, 0.5);
        assert!((inst.fill[3] - 0.5).abs() < 1e-4);
        assert!((inst.border[3] - 0.25).abs() < 1e-4);

        let color = Color::rgba(0xe6, 0xe6, 0xe6, 200);
        let dimmed = dim_color(color, 0.35);
        assert_eq!(dimmed.a(), 70, "200 × 0.35 = 70");
        assert_eq!(dimmed.r(), 0xe6);
        assert_eq!(dim_color(color, 1.0), color);
        // Кламп: альфа не уходит за 255
        let bright = dim_color(Color::rgba(1, 1, 1, 255), 0.999);
        assert_eq!(bright.a(), 255);
    }

    /// T23: has_node/has_edge — бинарный поиск по отсортированным срезам;
    /// EMPTY всё возвращает false.
    #[test]
    fn focus_view_membership() {
        let view = FocusView {
            nodes: &[1, 3, 5],
            edges: &[2, 4],
            dim: 1.0,
            pulse: 0.0,
        };
        assert!(view.has_node(1) && view.has_node(3) && view.has_node(5));
        assert!(!view.has_node(0) && !view.has_node(2) && !view.has_node(4));
        assert!(view.has_edge(2) && view.has_edge(4));
        assert!(!view.has_edge(0) && !view.has_edge(3));
        assert!(!FocusView::EMPTY.has_node(0));
        assert!(!FocusView::EMPTY.has_edge(0));
    }

    /// Обход нод: при avoid=true инстансы строятся по огибающей полилинии
    /// (их число отличается от прямой Безье, пропусков нет).
    #[test]
    fn edge_instances_avoid_route() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("b", "C:/b.png", 500.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("wall", "C:/w.png", 230.0, 0.0, 140.0, 100.0));
        canvas.add_edge(canvas_core::Edge::new("e1", "a", None, "b", None));
        let plain = build_edge_instances(&canvas, None, false, &FocusView::EMPTY, None, &no_hidden());
        let avoided = build_edge_instances(&canvas, None, true, &FocusView::EMPTY, None, &no_hidden());
        assert!(
            avoided.len() > plain.len(),
            "огибающий маршрут длиннее прямой: {} vs {}",
            avoided.len(),
            plain.len()
        );
        assert!(
            avoided.len() != plain.len() || avoided.iter().zip(&plain).any(|(a, b)| a.pos != b.pos),
            "инстансы огибающего маршрута отличаются от прямой"
        );
    }

    /// Порты hover-ноды: 4 кружка по центрам сторон, акцентный цвет.
    /// CR-003: диаметр кружков следует за зоной захвата (кламп 10..26).
    #[test]
    fn port_instances_at_side_centers() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 100.0, 200.0, 300.0, 120.0));
        let zone = 14.0;
        let ports = build_port_instances(&canvas, 0, zone);
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
        assert!(ports
            .iter()
            .all(|inst| inst.size == [port_dot_diameter(zone); 2]));
        // Дефолтная зона — прежний размер, крупная зона — кламп сверху,
        // зона ниже минимума — кламп снизу (CR-003)
        assert_eq!(port_dot_diameter(10.0), PORT_DOT);
        assert_eq!(port_dot_diameter(40.0), PORT_DOT_MAX);
        assert_eq!(port_dot_diameter(2.0), PORT_DOT);
        // Невалидный индекс — пусто
        assert!(build_port_instances(&canvas, 9, zone).is_empty());
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

    /// Призраки зоны дропа (T9): пустые позиции, cap обрезает гигантскую выборку.
    #[test]
    fn drop_ghosts_empty_and_capped() {
        assert!(drop_ghosts(&[], [320.0, 220.0], 50).is_empty());
        assert!(drop_zone_frame(&[], [320.0, 220.0], 24.0).is_none());
        // 10 позиций, cap 3 — только 3 призрака (превью дешёвое)
        let positions: Vec<[f32; 2]> = (0..10).map(|i| [i as f32 * 100.0, 0.0]).collect();
        assert_eq!(drop_ghosts(&positions, [320.0, 220.0], 3).len(), 3);
    }

    /// Призрак карточки дропа: pos/size, полупрозрачные цвета, без тени.
    #[test]
    fn drop_ghost_instance_shape() {
        let ghost = drop_ghost([7.0, 11.0], [320.0, 220.0]);
        assert_eq!(ghost.pos, [7.0, 11.0]);
        assert_eq!(ghost.size, [320.0, 220.0]);
        assert_eq!(ghost.fill, DROP_GHOST_FILL);
        assert_eq!(ghost.border, DROP_GHOST_BORDER);
        assert!(
            ghost.fill[3] > 0.0 && ghost.fill[3] < 1.0,
            "заливка полупрозрачна"
        );
        assert!(
            ghost.border[3] > 0.0 && ghost.border[3] < 1.0,
            "рамка полупрозрачна"
        );
        assert_eq!(ghost.params, [CORNER_RADIUS, 0.0, 0.0, 1.0]);
    }

    /// Рамка зоны дропа: bbox сетки + gap/2 со всех сторон, заливка пустая.
    #[test]
    fn drop_zone_frame_bbox() {
        let card = [320.0, 220.0];
        let gap = 24.0;
        let m = gap / 2.0;
        // Одна позиция: x = pos - m, y = pos - m, size = card + 2m
        let frame = drop_zone_frame(&[[10.0, 20.0]], card, gap).expect("рамка одной позиции");
        assert_eq!(frame.pos, [10.0 - m, 20.0 - m]);
        assert_eq!(frame.size, [card[0] + 2.0 * m, card[1] + 2.0 * m]);
        assert_eq!(frame.fill, [0.0; 4], "заливка полностью прозрачна");
        assert_eq!(frame.border, DROP_ZONE_BORDER);
        assert_eq!(frame.params[3], 1.0, "без тени");
        // Две позиции в ряд: ширина = 2*card + gap + 2m
        let step = card[0] + gap;
        let frame =
            drop_zone_frame(&[[0.0, 0.0], [step, 0.0]], card, gap).expect("рамка двух позиций");
        assert_eq!(frame.size[0], 2.0 * card[0] + gap + 2.0 * m);
        assert_eq!(frame.size[1], card[1] + 2.0 * m);
        assert_eq!(frame.pos, [-m, -m]);
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

        let instances = build_instances(&canvas, &[0, 1], Some(1), &ThemeColors::dark());
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
        let instances = build_instances(&canvas, &[1, 3], None, &ThemeColors::dark());
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].pos, [100.0, 0.0]);
        assert_eq!(instances[1].pos, [300.0, 0.0]);
        // Пустой список — пустой батч
        assert!(build_instances(&canvas, &[], None, &ThemeColors::dark()).is_empty());
        // Невалидный индекс пропускается без паники
        assert_eq!(
            build_instances(&canvas, &[99], None, &ThemeColors::dark()).len(),
            0
        );
    }

    /// Группа: полупрозрачная заливка и рамка из темы; выделенная —
    /// акцентной рамкой (как обычные ноды). Битая группа — серой.
    #[test]
    fn group_instance_uses_theme_frame() {
        let dark = ThemeColors::dark();
        let mut group = Node::group("g", 10.0, 20.0, 400.0, 300.0);
        let plain = card_instance(&group, false, &dark);
        assert_eq!(plain.fill, dark.group_fill, "заливка группы из темы");
        assert_eq!(plain.border, dark.group_border, "рамка группы из темы");
        assert!(
            plain.fill[3] > 0.0 && plain.fill[3] < 1.0,
            "заливка полупрозрачна"
        );
        assert_eq!(plain.params[1], 0.0, "не выделена");
        // Выделенная — акцентной рамкой выделения
        let selected = card_instance(&group, true, &dark);
        assert_eq!(selected.border, SELECTION_BORDER);
        assert_eq!(selected.params[1], 1.0);
        // Битая — серой рамкой (приоритет над рамкой группы)
        group.broken_link = Some(true);
        let broken = card_instance(&group, false, &dark);
        assert_eq!(broken.border, BROKEN_BORDER);
        assert_eq!(broken.params[2], 1.0);
        // Светлая тема даёт свои цвета
        let light = ThemeColors::light();
        assert_ne!(card_instance(&group, false, &light).fill, dark.group_fill);
    }

    /// Заголовок группы: label как есть (markdown не трогаем), без label —
    /// «Группа»; пустой label — тоже «Группа».
    #[test]
    fn group_title_defaults() {
        let mut group = Node::group("g", 0.0, 0.0, 400.0, 300.0);
        assert_eq!(title_for(&group), "Группа", "без label — дефолт");
        group.label = Some("**Спринт**".to_owned());
        assert_eq!(
            title_for(&group),
            "**Спринт**",
            "markdown к label не применяется"
        );
        group.label = Some(String::new());
        assert_eq!(title_for(&group), "Группа", "пустой label — дефолт");
    }

    /// Порты hover-ноды: у групп пусто (edge-drag с группы не начинается).
    #[test]
    fn port_instances_skip_groups() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::group("g", 0.0, 0.0, 400.0, 300.0));
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 100.0, 200.0, 300.0, 120.0));
        assert!(
            build_port_instances(&canvas, 0, 10.0).is_empty(),
            "у группы портов нет"
        );
        assert_eq!(build_port_instances(&canvas, 1, 10.0).len(), 4);
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

    /// Паттерн «сплошная»: плотная цепочка от начала до конца с шагом 0.8d.
    #[test]
    fn pattern_solid_covers_whole_line() {
        let line = [[0.0, 0.0], [100.0, 0.0]];
        let dots = line_pattern_dots(&line, canvas_core::EdgeLineStyle::Solid, 2.5);
        assert_eq!(dots.len(), 51, "шаг 2.0: 0..=100");
        assert_eq!(dots[0], [0.0, 0.0]);
        assert_eq!(dots[50], [100.0, 0.0]);
    }

    /// Паттерн «пунктир»: черта/пропуск по периоду; в пропуске точек нет.
    #[test]
    fn pattern_dashed_alternates_runs_and_gaps() {
        let line = [[0.0, 0.0], [100.0, 0.0]];
        let dots = line_pattern_dots(&line, canvas_core::EdgeLineStyle::Dashed, 2.5);
        // Период 20, черта 12: x ∈ {0..=12}, {20..=32}, ...
        for x in [0.0, 10.0, 12.0, 20.0, 32.0, 80.0, 92.0] {
            assert!(
                dots.iter().any(|p| (p[0] - x).abs() < 1e-3),
                "точка на черте x={x}"
            );
        }
        for x in [14.0, 16.0, 18.0, 34.0, 78.0] {
            assert!(
                dots.iter().all(|p| (p[0] - x).abs() > 1e-3),
                "пропуск без точек x={x}"
            );
        }
    }

    /// Паттерн «точки»: изолированные кружки с шагом 3d.
    #[test]
    fn pattern_dotted_spaces_points() {
        let line = [[0.0, 0.0], [100.0, 0.0]];
        let dots = line_pattern_dots(&line, canvas_core::EdgeLineStyle::Dotted, 2.5);
        assert_eq!(dots.len(), 14, "шаг 7.5: 0..97.5");
        for pair in dots.windows(2) {
            let dist = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            assert!((dist - 7.5).abs() < 1e-3, "равный шаг между точками");
        }
    }

    /// Стиль и толщина из полей связи попадают в инстансы.
    #[test]
    fn edge_instances_use_style_and_thickness() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("b", "C:/b.png", 500.0, 0.0, 100.0, 100.0));
        let mut edge = canvas_core::Edge::new("e1", "a", None, "b", None);
        edge.style = Some(canvas_core::EdgeLineStyle::Dotted);
        edge.thickness = Some(canvas_core::EdgeThickness::Thin);
        canvas.add_edge(edge);

        let instances = build_edge_instances(&canvas, None, false, &FocusView::EMPTY, None, &no_hidden());
        assert!(!instances.is_empty());
        assert!(
            instances.iter().all(|inst| inst.size[0] == 1.8),
            "тонкая линия — кружки 1.8px"
        );
    }
}
