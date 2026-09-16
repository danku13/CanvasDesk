//! FR-018: UI шаблонов — боковая палитра (`Ctrl+P`) и радиальное wheel-меню
//! (`Shift+клик` по пустому месту).
//!
//! Модуль ЧИСТЫЙ (без winit/wgpu): состояния, геометрия, hit-test'ы,
//! фильтрация реестра — всё тестируется без GPU/окна. Рендер выполняет
//! приложение через `FrameOverlay` (квады [`CardInstance`] + screen-тексты —
//! сборка в `main.rs`, паттерн `search_overlay`).
//!
//! Решения владельца FR-018:
//! - панель `Ctrl+P` (поиск + категории) И радиальный wheel по `Shift+клику`
//!   (внешнее кольцо — категории, внутренние — шаблоны категории; при
//!   переполнении кольца шаблоны уходят в концентрические под-кольца —
//!   правка владельца 2026-09-16: раскладка отталкивается от размера
//!   плашек-мини-карточек, зазор между ними гарантирован);
//! - иконки — квад-иконки (как в палитре действий, без SVG/resvg) —
//!   геометрия в [`crate::cards`-пайплайне]; здесь — [`icon_key`].

use canvas_core::templates::{TemplateManifest, TemplateRegistry};

use crate::Vec2;

// --- Иконки ---

/// Ключ квад-иконки шаблона: известные ключи манифестов (`lb`, `db`,
/// `cache`, `http`, `queue`), прочее — `custom` (рамка с ядром). Геометрия
/// самих иконок — `canvas_render::cards::template_icon_quads` (общая с
/// шапкой карточки).
pub fn icon_key(manifest: &TemplateManifest) -> &str {
    match manifest.icon.as_str() {
        "lb" | "db" | "cache" | "http" | "queue" | "gateway" | "worker" | "storage" | "auth"
        | "grpc" | "graphql" => manifest.icon.as_str(),
        _ => "custom",
    }
}

// --- Панель шаблонов (Ctrl+P) ---

/// Ширина панели, логические px (клампится к окну).
pub const PANEL_WIDTH: f32 = 340.0;
/// Боковой отступ панели от правого края окна.
pub const PANEL_MARGIN: f32 = 12.0;
/// Отступ от верхнего края окна.
pub const PANEL_TOP_MARGIN: f32 = 12.0;
/// Внутренний отступ содержимого.
pub const PANEL_PADDING: f32 = 8.0;
/// Высота поля поиска.
pub const INPUT_HEIGHT: f32 = 32.0;
/// Высота строки категории (чипы-фильтры).
pub const CATEGORY_ROW_H: f32 = 26.0;
/// Высота строки шаблона: имя + описание.
pub const ROW_HEIGHT: f32 = 46.0;
/// Максимум видимых строк (далее — прокрутка колесом/стрелками).
pub const MAX_VISIBLE_ROWS: usize = 9;
/// Сторона квад-иконки в строке панели (логические px).
pub const TEMPLATE_ROW_ICON: f32 = 18.0;

/// Состояние боковой палитры шаблонов (FR-018, `Ctrl+P`). Поле ввода —
/// своя лёгкая модель (однострочная, как `SearchInput`), НЕ `EditingSession`.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatePanel {
    pub open: bool,
    /// Строка фильтра (подстрока без учёта регистра по имени/описанию/id).
    pub filter: String,
    /// Байтовая позиция каретки в `filter`.
    pub cursor: usize,
    /// Фильтр категории (клик по чипу); None — все категории.
    pub category: Option<String>,
    /// Индекс выбранной строки в отфильтрованном списке (клавиатура).
    pub selected: usize,
    /// Верхняя видимая строка (прокрутка).
    pub scroll_top: usize,
}

impl TemplatePanel {
    pub fn new() -> Self {
        Self {
            open: false,
            filter: String::new(),
            cursor: 0,
            category: None,
            selected: 0,
            scroll_top: 0,
        }
    }

    /// Открыть (сброс фильтров — каждый вызов с чистого листа).
    pub fn open(&mut self) {
        self.open = true;
        self.filter.clear();
        self.cursor = 0;
        self.category = None;
        self.selected = 0;
        self.scroll_top = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Вставка строки в каретку (печать символа, IME).
    pub fn insert_str(&mut self, text: &str) {
        let byte = self.cursor.min(self.filter.len());
        self.filter.insert_str(byte, text);
        self.cursor += text.len();
    }

    /// Backspace: удалить символ перед кареткой (true — было изменение).
    pub fn backspace(&mut self) -> bool {
        if self.cursor == 0 {
            return false;
        }
        let byte = self.cursor.min(self.filter.len());
        let prev = self.filter[..byte]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.filter.replace_range(prev..byte, "");
        self.cursor = prev;
        true
    }

    pub fn move_left(&mut self) {
        let byte = self.cursor.min(self.filter.len());
        if let Some(i) = self.filter[..byte]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
        {
            self.cursor = i;
        }
    }

    pub fn move_right(&mut self) {
        let byte = self.cursor.min(self.filter.len());
        if let Some(i) = self.filter[byte..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| byte + i)
        {
            self.cursor = i;
        }
    }

    /// Сдвиг выделения списка (стрелки); true — было изменение.
    pub fn move_selection(&mut self, delta: i32, total: usize) -> bool {
        if total == 0 {
            return false;
        }
        let current = self.selected as i32;
        let next = (current + delta).clamp(0, total as i32 - 1);
        if next == current {
            return false;
        }
        self.selected = next as usize;
        // Прокрутка следует за выделением
        if self.selected < self.scroll_top {
            self.scroll_top = self.selected;
        } else if self.selected >= self.scroll_top + MAX_VISIBLE_ROWS {
            self.scroll_top = self.selected + 1 - MAX_VISIBLE_ROWS;
        }
        true
    }
}

impl Default for TemplatePanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Отфильтрованные индексы реестра для панели: категория-фильтр + подстрока
/// (имя/описание/id, регистр не важен). Порядок — порядок реестра.
pub fn panel_rows(registry: &TemplateRegistry, panel: &TemplatePanel) -> Vec<usize> {
    let query = panel.filter.to_lowercase();
    registry
        .list()
        .iter()
        .enumerate()
        .filter(|(_, manifest)| {
            if let Some(category) = &panel.category {
                if &manifest.category != category {
                    return false;
                }
            }
            query.is_empty()
                || manifest.name.to_lowercase().contains(&query)
                || manifest.description.to_lowercase().contains(&query)
                || manifest.id.to_lowercase().contains(&query)
        })
        .map(|(index, _)| index)
        .collect()
}

/// Геометрия панели на кадр (логические px). `rows` — параллельно
/// `row_rects` (индексы реестра из [`panel_rows`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    pub panel_rect: [f32; 4],
    pub input_rect: [f32; 4],
    /// Чипы категорий: (rect, имя категории, активен).
    pub category_rects: Vec<([f32; 4], String, bool)>,
    /// Rect'ы строк (параллельно rows); только видимые.
    pub row_rects: Vec<[f32; 4]>,
}

pub fn panel_layout(
    window_w: f32,
    window_h: f32,
    registry: &TemplateRegistry,
    panel: &TemplatePanel,
    rows: &[usize],
) -> PanelLayout {
    let width = PANEL_WIDTH.min((window_w - PANEL_MARGIN * 2.0).max(0.0));
    // Правый край (как Miro/Figma — решение владельца)
    let x = (window_w - width - PANEL_MARGIN).max(PANEL_MARGIN);
    let y = PANEL_TOP_MARGIN;

    // Чипы категорий: одна строка, ширина по имени (+ паддинг), перенос
    // не делаем — в v1 категорий ≤ 6
    let categories = registry.categories();
    let mut category_rects = Vec::with_capacity(categories.len());
    let mut cx = x + PANEL_PADDING;
    for category in &categories {
        let w = category.len() as f32 * 7.5 + 20.0;
        if cx + w > x + width - PANEL_PADDING {
            break; // не влезли — остальные доступны прокруткой фильтра
        }
        let active = panel.category.as_deref() == Some(*category);
        category_rects.push((
            [
                cx,
                y + PANEL_PADDING + INPUT_HEIGHT + 6.0,
                w,
                CATEGORY_ROW_H,
            ],
            (*category).to_owned(),
            active,
        ));
        cx += w + 6.0;
    }

    let rows_top = y + PANEL_PADDING + INPUT_HEIGHT + 6.0 + CATEGORY_ROW_H + 6.0;
    let visible = rows
        .len()
        .saturating_sub(panel.scroll_top)
        .min(MAX_VISIBLE_ROWS);
    let mut row_rects = Vec::with_capacity(visible);
    for i in 0..visible {
        let row_y = rows_top + i as f32 * ROW_HEIGHT;
        row_rects.push([
            x + PANEL_PADDING,
            row_y,
            width - PANEL_PADDING * 2.0,
            ROW_HEIGHT - 4.0,
        ]);
    }
    // Высота панели — по контенту, клампится к окну
    let content_bottom = rows_top + visible as f32 * ROW_HEIGHT + PANEL_PADDING;
    let height = (content_bottom - y).min((window_h - PANEL_TOP_MARGIN * 2.0).max(0.0));
    PanelLayout {
        panel_rect: [x, y, width, height],
        input_rect: [
            x + PANEL_PADDING,
            y + PANEL_PADDING,
            width - PANEL_PADDING * 2.0,
            INPUT_HEIGHT,
        ],
        category_rects,
        row_rects,
    }
}

// --- Wheel-меню (Shift+клик) ---

// Правка владельца FR-018 (2026-09-16): «отталкивайся от размера карточки
// и сделай адекватные отступы карточек друг от друга» — раньше 10 плашек
// 60×60 стояли на кольце радиуса 41 и налезали друг на друга. Теперь
// радиусы колец ВЫЧИСЛЯЮТСЯ из габаритов плашек-мини-карточек и
// гарантированного зазора, переполнение уходит в под-кольца, а итоговая
// раскладка проверяется попарным AABB-тестом — налезание невозможно.

/// Ширина плашки шаблона (мини-карточка: квад-иконка + имя), лог. px.
pub const WHEEL_TPL_W: f32 = 124.0;
/// Высота плашки шаблона (1–2 строки имени).
pub const WHEEL_TPL_H: f32 = 46.0;
/// Ширина плашки категории.
pub const WHEEL_CAT_W: f32 = 96.0;
/// Высота плашки категории.
pub const WHEEL_CAT_H: f32 = 30.0;
/// Гарантированный зазор между соседними плашками (по обеим осям).
pub const WHEEL_GAP: f32 = 12.0;
/// Максимум плашек в одном кольце; большее — концентрические под-кольца
/// (внешние вместительнее — окружность больше).
pub const WHEEL_RING_CAP: usize = 6;
/// Радиус центрального хаба (пустая зона — глотает клик).
pub const WHEEL_HUB_R: f32 = 24.0;
/// FR-022: диаметр кнопки-хаба (клик = «назад»/«закрыть»). Крупная цель
/// ≥ 44 лог. px — гайдлайн сенсорных целей (Big Medium); совпадает с
/// удвоенным [`WHEEL_HUB_R`] — зона хаба и кнопка — одно и то же.
pub const WHEEL_HUB_D: f32 = 48.0;
/// Отступ wheel от краёв окна при клампе центра.
pub const WHEEL_SCREEN_MARGIN: f32 = 8.0;
/// Предел символов строки имени на плашке (длиннее — перенос по пробелу).
pub const WHEEL_TPL_TEXT_CHARS: usize = 15;
/// Шаг роста радиуса в корректирующем цикле раскладки.
const WHEEL_GROW_STEP: f32 = 4.0;
/// Предел итераций корректирующего цикла (детерминизм).
const WHEEL_GROW_ITERS: usize = 256;

/// Состояние радиального меню шаблонов (FR-018, `Shift+клик` по пустому
/// месту). `screen` — центр в логических px (для рендера/hit-test),
/// `world` — точка канваса (куда инстанцируется выбранный шаблон).
#[derive(Debug, Clone, PartialEq)]
pub struct WheelMenu {
    pub screen: Vec2,
    pub world: Vec2,
    /// Выбранная категория (внутреннее кольцо); None — только внешнее.
    pub category: Option<String>,
}

/// Цель попадания в wheel.
#[derive(Debug, Clone, PartialEq)]
pub enum WheelHit {
    /// Плашка категории (индекс в `registry.categories()`).
    Category(usize),
    /// Плашка шаблона (индекс ВНУТРИ категории — `registry.by_category`;
    /// сквозной по кольцам, кольца — только план раскладки).
    Template(usize),
}

/// Плашка wheel: прямоугольник (screen px) + цель попадания. Единый
/// источник правды рендера и hit-test: что нарисовано — по тому и клик.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelPlate {
    pub rect: [f32; 4],
    pub hit: WheelHit,
}

/// Геометрия wheel на кадр: центр (с клампом к окну), плашки, внешний
/// радиус. Строится чистой функцией [`wheel_geometry`] — и рендер
/// (`wheel_overlay`), и клики ([`WheelGeometry::hit`]) вызывают её с теми
/// же аргументами, поэтому расхождений быть не может.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelGeometry {
    /// Центр меню: точка клика, сдвинутая клампом внутрь окна, если wheel
    /// целиком не влезает.
    pub center: Vec2,
    /// Плашки: шаблоны (изнутри наружу), затем категории.
    pub plates: Vec<WheelPlate>,
    /// Радиус описанной окружности плашек (для «клик заметно дальше —
    /// закрыть»).
    pub extent: f32,
    /// FR-022: квадрат кнопки-хаба (`WHEEL_HUB_D × WHEEL_HUB_D`) вокруг
    /// центра — единый источник для рендера круга и клика «назад/закрыть».
    pub hub: [f32; 4],
}

impl WheelGeometry {
    /// FR-022: курсор на кнопке-хабе? Тест по квадрату хаба (визуальный
    /// круг вписан в него; углы квадрата прощаются — Fitts).
    pub fn hub_hit(&self, cursor: Vec2) -> bool {
        let [x, y, w, h] = self.hub;
        cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
    }

    /// Плашка под курсором (screen px): точный hit-test по прямоугольникам
    /// плашек (раньше был угловой тест по секторам, не совпадавший с
    /// квадами рендера).
    pub fn hit(&self, cursor: Vec2) -> Option<WheelHit> {
        self.plates
            .iter()
            .find(|p| {
                let [x, y, w, h] = p.rect;
                cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
            })
            .map(|p| p.hit.clone())
    }
}

/// Сектора кольца: центр угла i-го из count секторов (от 12 часов, по
/// часовой). Чистая функция — детерминизм секторов для hit-test и рендера.
pub fn sector_center_angle(count: usize, index: usize) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let step = std::f32::consts::TAU / count as f32;
    -std::f32::consts::FRAC_PI_2 + step * (index as f32 + 0.5)
}

/// План колец шаблонов: `count` плашек по кольцам вместимостью
/// [`WHEEL_RING_CAP`], сбалансированно, внешние кольца вместительнее
/// (их окружность больше): 10 → [5, 5], 11 → [5, 6], 7 → [3, 4].
pub fn wheel_ring_plan(count: usize) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    let rings = count.div_ceil(WHEEL_RING_CAP);
    let base = count / rings;
    let extra = count % rings;
    (0..rings)
        .map(|i| base + usize::from(i + extra >= rings))
        .collect()
}

/// Минимальный радиус кольца из `count` плашек шириной `side_w`
/// (ось-выровненные прямоугольники): для каждой пары соседей
/// max(|Δx|, |Δy|) ≥ side_w + [`WHEEL_GAP`] — гарантия отсутствия
/// налезания при любой ориентации хорды. `min_r` — нижняя граница
/// (зазор до хаба / соседнего кольца).
pub fn ring_min_radius(count: usize, side_w: f32, min_r: f32) -> f32 {
    let mut r = min_r;
    if count >= 2 {
        let need = side_w + WHEEL_GAP;
        for i in 0..count {
            let a = sector_center_angle(count, i);
            let b = sector_center_angle(count, (i + 1) % count);
            let reach = (b.cos() - a.cos()).abs().max((b.sin() - a.sin()).abs());
            if reach > 1e-6 {
                r = r.max(need / reach);
            }
        }
    }
    r
}

/// Прямоугольники пересекаются ПО ПЛОЩАДИ (касание рёбер — не пересечение).
fn rects_overlap(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Есть ли хоть одна пересекающаяся пара в наборе плашек.
fn any_overlap(rects: &[[f32; 4]]) -> bool {
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            if rects_overlap(rects[i], rects[j]) {
                return true;
            }
        }
    }
    false
}

/// Максимальное расстояние угла прямоугольника от начала координат
/// (координаты центр-относительные) — вклад плашки во внешний радиус.
fn rect_max_extent(rect: [f32; 4]) -> f32 {
    let [x, y, w, h] = rect;
    [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
        .iter()
        .map(|(cx, cy)| cx.hypot(*cy))
        .fold(0.0_f32, f32::max)
}

/// Прямоугольники одного кольца (относительно центра wheel): `count`
/// плашек `side_w`×`side_h` на радиусе `r`, центры по углам
/// [`sector_center_angle`] (старт с 12 часов, по часовой).
fn ring_rects(count: usize, r: f32, side_w: f32, side_h: f32) -> Vec<[f32; 4]> {
    (0..count)
        .map(|i| {
            let [x, y] = sector_point([0.0, 0.0], sector_center_angle(count, i), r);
            [x - side_w / 2.0, y - side_h / 2.0, side_w, side_h]
        })
        .collect()
}

/// Разместить кольцо: стартовый радиус — от ширины плашки и зазора
/// ([`ring_min_radius`]), затем радиус растёт шагом [`WHEEL_GROW_STEP`],
/// пока хоть одна пара плашек (новых или с уже размещёнными) пересекается
/// по площади. Гарантия отсутствия налезания — по построению. Возвращает
/// (радиус, прямоугольники).
fn place_ring(
    existing: &[[f32; 4]],
    count: usize,
    side_w: f32,
    side_h: f32,
    min_r: f32,
) -> (f32, Vec<[f32; 4]>) {
    if count == 0 {
        return (min_r, Vec::new());
    }
    let mut r = ring_min_radius(count, side_w, min_r);
    for _ in 0..WHEEL_GROW_ITERS {
        let rects = ring_rects(count, r, side_w, side_h);
        let mut all = existing.to_vec();
        all.extend_from_slice(&rects);
        if !any_overlap(&all) {
            return (r, rects);
        }
        r += WHEEL_GROW_STEP;
    }
    (r, ring_rects(count, r, side_w, side_h))
}

/// Перенос имени шаблона на плашке: имя длиннее `max_chars` символов
/// делится на две строки по самому позднему подходящему разделителю
/// (пробел или дефис) так, чтобы обе части влезали; не получилось — одна
/// строка (хвост клипается рендером по ширине плашки).
pub fn split_two_lines(name: &str, max_chars: usize) -> (String, Option<String>) {
    let chars = |s: &str| s.chars().count();
    if chars(name) <= max_chars {
        return (name.to_owned(), None);
    }
    let mut best: Option<usize> = None; // байт-позиция начала второй строки
    for (byte, ch) in name.char_indices() {
        if ch != ' ' && ch != '-' {
            continue;
        }
        let split = byte + ch.len_utf8();
        let (n1, n2) = (chars(&name[..split]), chars(&name[split..]));
        if n1 <= max_chars
            && n2 <= max_chars
            && n1 >= 2
            && n2 >= 2
            && best.map_or(true, |b| split > b)
        {
            best = Some(split);
        }
    }
    match best {
        Some(split) => (
            name[..split].trim_end().to_owned(),
            Some(name[split..].to_owned()),
        ),
        None => (name.to_owned(), None),
    }
}

/// Геометрия wheel (правка владельца FR-018): раскладка ОТТАЛКИВАЕТСЯ ОТ
/// РАЗМЕРА ПЛАШЕК — радиусы вычисляются из габаритов мини-карточек и
/// гарантированного зазора [`WHEEL_GAP`]; переполнение кольца
/// (>[`WHEEL_RING_CAP`]) уходит в концентрические под-кольца; итог
/// проверяется попарным AABB-тестом ([`place_ring`]) — налезание плашек
/// невозможно. `screen` — точка клика; `window_w/h` — размер окна для
/// клампа центра (клик у края не должен обрезать меню).
pub fn wheel_geometry(
    screen: Vec2,
    window_w: f32,
    window_h: f32,
    category_count: usize,
    template_count: usize,
) -> WheelGeometry {
    // 1. Кольца шаблонов — от хаба наружу. Межкольцевой зазор стартует от
    //    радиуса предыдущего кольца + высоты плашки; угловые налезания
    //    добирает корректирующий цикл place_ring.
    let plan = wheel_ring_plan(template_count);
    let hub_min_r = WHEEL_HUB_R + WHEEL_TPL_H / 2.0 + WHEEL_GAP;
    let mut rects: Vec<[f32; 4]> = Vec::new();
    let mut placed_extent = 0.0_f32;
    let mut prev_r = 0.0_f32;
    for (k, count) in plan.iter().enumerate() {
        let min_r = if k == 0 {
            hub_min_r
        } else {
            prev_r + WHEEL_TPL_H + WHEEL_GAP
        };
        let (r, placed) = place_ring(&rects, *count, WHEEL_TPL_W, WHEEL_TPL_H, min_r);
        prev_r = r;
        for rect in &placed {
            placed_extent = placed_extent.max(rect_max_extent(*rect));
        }
        rects.extend(placed);
    }
    // 2. Кольцо категорий — снаружи; без шаблонов — своё расстояние до хаба.
    let cat_min_r = if prev_r > 0.0 {
        prev_r + WHEEL_TPL_H / 2.0 + WHEEL_GAP + WHEEL_CAT_H / 2.0
    } else {
        WHEEL_HUB_R + WHEEL_CAT_H / 2.0 + WHEEL_GAP
    };
    let (_, cat_rects) = place_ring(&rects, category_count, WHEEL_CAT_W, WHEEL_CAT_H, cat_min_r);

    // 3. Плашки: индекс шаблона — сквозной по категории (кольца — только
    //    план раскладки), категории — по порядку реестра.
    let mut plates: Vec<WheelPlate> = Vec::with_capacity(rects.len() + cat_rects.len());
    let mut template_index = 0_usize;
    let mut ring_iter = rects.iter();
    for count in &plan {
        for _ in 0..*count {
            let rect = *ring_iter.next().expect("кольцо размещено");
            plates.push(WheelPlate {
                rect,
                hit: WheelHit::Template(template_index),
            });
            template_index += 1;
        }
    }
    for (i, rect) in cat_rects.iter().enumerate() {
        plates.push(WheelPlate {
            rect: *rect,
            hit: WheelHit::Category(i),
        });
    }

    // 4. Внешний радиус, кнопка-хаб (FR-022) и кламп центра к окну.
    let extent = cat_rects
        .iter()
        .fold(placed_extent, |acc, rect| acc.max(rect_max_extent(*rect)));
    let lo = extent + WHEEL_SCREEN_MARGIN;
    let center: Vec2 = if window_w >= lo * 2.0 && window_h >= lo * 2.0 {
        [
            screen[0].clamp(lo, window_w - lo),
            screen[1].clamp(lo, window_h - lo),
        ]
    } else {
        [window_w / 2.0, window_h / 2.0]
    };
    let hub = [
        center[0] - WHEEL_HUB_D / 2.0,
        center[1] - WHEEL_HUB_D / 2.0,
        WHEEL_HUB_D,
        WHEEL_HUB_D,
    ];
    for plate in &mut plates {
        plate.rect[0] += center[0];
        plate.rect[1] += center[1];
    }
    WheelGeometry {
        center,
        plates,
        extent,
        hub,
    }
}

/// Точка на кольце для рендера сектора (центр сектора на среднем радиусе).
pub fn sector_point(center: Vec2, angle: f32, radius: f32) -> Vec2 {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TemplateRegistry {
        TemplateRegistry::mock()
    }

    // --- Панель ---

    #[test]
    fn panel_filter_by_name_and_id() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        // Пустой фильтр — все 5
        assert_eq!(panel_rows(&registry, &panel).len(), 5);
        // По имени (регистр не важен)
        panel.insert_str("load");
        assert_eq!(panel_rows(&registry, &panel), vec![0]);
        // По id
        panel.filter.clear();
        panel.cursor = 0;
        panel.insert_str("mock.db");
        assert_eq!(panel_rows(&registry, &panel), vec![1]);
        // По описанию
        panel.filter.clear();
        panel.cursor = 0;
        panel.insert_str("партиции");
        assert_eq!(panel_rows(&registry, &panel), vec![4]);
        // Мимо — пусто
        panel.filter = "ghost".to_owned();
        assert!(panel_rows(&registry, &panel).is_empty());
    }

    #[test]
    fn panel_category_filter() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        panel.category = Some("backend".to_owned());
        let rows = panel_rows(&registry, &panel);
        assert_eq!(rows.len(), 2);
        assert_eq!(registry.list()[rows[0]].category, "backend");
        assert_eq!(registry.list()[rows[1]].category, "backend");
    }

    #[test]
    fn panel_input_caret_edits() {
        let mut panel = TemplatePanel::new();
        panel.insert_str("lb");
        panel.insert_str(" x");
        assert_eq!(panel.filter, "lb x");
        assert_eq!(panel.cursor, 4);
        // Backspace в середине
        panel.move_left();
        panel.move_left();
        panel.backspace(); // удаляет 'b' → "l x"
        assert_eq!(panel.filter, "l x");
        assert_eq!(panel.cursor, 1);
        // Backspace в начале — no-op
        panel.cursor = 0;
        assert!(!panel.backspace());
        assert_eq!(panel.filter, "l x");
    }

    #[test]
    fn panel_selection_scroll_follows() {
        let mut panel = TemplatePanel::new();
        panel.open = true;
        assert!(panel.move_selection(1, 20));
        assert_eq!(panel.selected, 1);
        panel.move_selection(10, 20);
        assert_eq!(panel.selected, 11);
        // Прокрутка догоняет выделение (MAX_VISIBLE_ROWS = 9)
        assert!(panel.scroll_top + MAX_VISIBLE_ROWS > panel.selected);
        // Границы
        panel.move_selection(100, 20);
        assert_eq!(panel.selected, 19);
        panel.move_selection(-100, 20);
        assert_eq!(panel.selected, 0);
        assert_eq!(panel.scroll_top, 0);
        // Пустой список — no-op
        assert!(!panel.move_selection(1, 0));
    }

    #[test]
    fn panel_layout_geometry() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        let rows = panel_rows(&registry, &panel);
        let lay = panel_layout(1280.0, 800.0, &registry, &panel, &rows);
        // Панель у правого края
        assert!((lay.panel_rect[0] + lay.panel_rect[2] - (1280.0 - PANEL_MARGIN)).abs() < 0.01);
        // Поле ввода внутри панели
        assert!(lay.input_rect[0] > lay.panel_rect[0]);
        // Строки не вылезают за панель; все 5 видны
        assert_eq!(lay.row_rects.len(), 5);
        for rect in &lay.row_rects {
            assert!(rect[0] >= lay.panel_rect[0]);
            assert!(rect[0] + rect[2] <= lay.panel_rect[0] + lay.panel_rect[2] + 0.01);
            assert!(rect[1] + rect[3] <= lay.panel_rect[1] + lay.panel_rect[3] + 0.01);
        }
        // Чипы категорий — 4 (backend/cache/network/queue)
        assert_eq!(lay.category_rects.len(), 4);
    }

    // --- Wheel: геометрия (правка владельца — раскладка от плашек) ---

    #[test]
    fn wheel_ring_plan_splits_balanced() {
        assert_eq!(wheel_ring_plan(0), Vec::<usize>::new());
        assert_eq!(wheel_ring_plan(1), vec![1]);
        assert_eq!(wheel_ring_plan(6), vec![6]);
        assert_eq!(wheel_ring_plan(7), vec![3, 4]);
        assert_eq!(wheel_ring_plan(10), vec![5, 5]);
        assert_eq!(wheel_ring_plan(11), vec![5, 6]);
        assert_eq!(wheel_ring_plan(13), vec![4, 4, 5]);
        assert_eq!(wheel_ring_plan(15), vec![5, 5, 5]);
    }

    #[test]
    fn wheel_hub_button_geometry() {
        // FR-022: хаб — квадрат WHEEL_HUB_D вокруг центра, рендер и клик
        // видят один и тот же прямоугольник; плашки в хаб не заходят
        let geo = wheel_geometry([400.0, 300.0], 1280.0, 800.0, 4, 10);
        assert!((geo.hub[2] - WHEEL_HUB_D).abs() < 0.01);
        assert!((geo.hub[0] + WHEEL_HUB_D / 2.0 - geo.center[0]).abs() < 0.01);
        assert!((geo.hub[1] + WHEEL_HUB_D / 2.0 - geo.center[1]).abs() < 0.01);
        // Клик по центру — хаб; на первой плашке — не хаб
        assert!(geo.hub_hit(geo.center));
        assert!(!geo.hub_hit([geo.center[0], geo.center[1] - geo.extent]));
        for plate in &geo.plates {
            let [x, y, w, h] = plate.rect;
            let overlap = geo.hub[0] < x + w
                && x < geo.hub[0] + geo.hub[2]
                && geo.hub[1] < y + h
                && y < geo.hub[1] + geo.hub[3];
            assert!(!overlap, "плашка налезла на кнопку-хаб");
        }
    }

    #[test]
    fn wheel_geometry_plates_never_overlap() {
        // ГЛАВНЫЙ инвариант правки: никакая пара плашек не налезает друг
        // на друга — ни при 10 шаблонах (backend), ни при 15, ни без
        // шаблонов (категории одни)
        for tpl in [0usize, 1, 2, 5, 6, 7, 10, 11, 15] {
            for cat in [0usize, 2, 4, 6] {
                let geo = wheel_geometry([400.0, 300.0], 1280.0, 800.0, cat, tpl);
                let rects: Vec<[f32; 4]> = geo.plates.iter().map(|p| p.rect).collect();
                assert!(
                    !any_overlap(&rects),
                    "налезание плашек: категорий={cat}, шаблонов={tpl}"
                );
            }
        }
    }

    #[test]
    fn wheel_geometry_sizes_and_hub_clearance() {
        let geo = wheel_geometry([400.0, 300.0], 1280.0, 800.0, 2, 10);
        assert_eq!(geo.plates.len(), 12);
        for plate in &geo.plates {
            let [x, y, w, h] = plate.rect;
            match plate.hit {
                WheelHit::Template(_) => {
                    assert!((w - WHEEL_TPL_W).abs() < 0.01 && (h - WHEEL_TPL_H).abs() < 0.01);
                }
                WheelHit::Category(_) => {
                    assert!((w - WHEEL_CAT_W).abs() < 0.01 && (h - WHEEL_CAT_H).abs() < 0.01);
                }
            }
            // Плашка не заходит в хаб: ближайшая точка прямоугольника к
            // центру — дальше радиуса хаба
            let nx = geo.center[0].clamp(x, x + w);
            let ny = geo.center[1].clamp(y, y + h);
            let dist = (nx - geo.center[0]).hypot(ny - geo.center[1]);
            assert!(dist >= WHEEL_HUB_R, "плашка залезла в хаб: dist={dist}");
        }
    }

    #[test]
    fn wheel_hit_by_plate_rects() {
        let registry = registry(); // mock: 4 категории, в backend 2 шаблона
        let menu = WheelMenu {
            screen: [400.0, 300.0],
            world: [10.0, 20.0],
            category: None,
        };
        let geo = wheel_geometry(menu.screen, 1280.0, 800.0, registry.categories().len(), 0);
        // Центр плашки категории 0 → Category(0)
        let plate = geo
            .plates
            .iter()
            .find(|p| p.hit == WheelHit::Category(0))
            .expect("плашка категории");
        let center = [
            plate.rect[0] + plate.rect[2] / 2.0,
            plate.rect[1] + plate.rect[3] / 2.0,
        ];
        assert_eq!(geo.hit(center), Some(WheelHit::Category(0)));
        // Хаб и дальняя точка — None
        assert_eq!(geo.hit(geo.center), None);
        assert_eq!(geo.hit([900.0, 300.0]), None);
        // Без категории шаблонных плашек нет
        assert!(geo
            .plates
            .iter()
            .all(|p| !matches!(p.hit, WheelHit::Template(_))));
    }

    #[test]
    fn wheel_hit_templates_flat_index() {
        let registry = registry();
        let menu = WheelMenu {
            screen: [400.0, 300.0],
            world: [10.0, 20.0],
            category: Some("backend".to_owned()),
        };
        let geo = wheel_geometry(
            menu.screen,
            1280.0,
            800.0,
            registry.categories().len(),
            registry.by_category("backend").len(),
        );
        // Каждая шаблонная плашка отвечает своим плоским индексом
        for plate in &geo.plates {
            let center = [
                plate.rect[0] + plate.rect[2] / 2.0,
                plate.rect[1] + plate.rect[3] / 2.0,
            ];
            assert_eq!(geo.hit(center), Some(plate.hit.clone()));
        }
        assert!(geo.plates.iter().any(|p| p.hit == WheelHit::Template(0)));
        assert!(geo.plates.iter().any(|p| p.hit == WheelHit::Template(1)));
        // Категории остаются кликабельными при выбранной категории
        assert!(geo
            .plates
            .iter()
            .any(|p| matches!(p.hit, WheelHit::Category(_))));
    }

    #[test]
    fn wheel_geometry_clamps_to_window() {
        // Клик у правого края: центр сдвигается, wheel целиком в окне
        // (высота 1000 — с запасом под большой wheel 10 шаблонов + 4
        // категорий)
        let geo = wheel_geometry([1270.0, 500.0], 1280.0, 1000.0, 4, 10);
        assert!(geo.center[0] + geo.extent <= 1280.0 - WHEEL_SCREEN_MARGIN + 0.01);
        assert!(geo.center[0] - geo.extent >= WHEEL_SCREEN_MARGIN - 0.01);
        assert!(geo.center[1] + geo.extent <= 1000.0 - WHEEL_SCREEN_MARGIN + 0.01);
        assert!(geo.center[1] - geo.extent >= WHEEL_SCREEN_MARGIN - 0.01);
        // Клик в центре окна — центр не двигается
        let mid = wheel_geometry([640.0, 500.0], 1280.0, 1000.0, 4, 10);
        assert!((mid.center[0] - 640.0).abs() < 0.01);
        // Крошечное окно (wheel не влезает) — центр в середине, без паники
        let small = wheel_geometry([50.0, 50.0], 200.0, 150.0, 4, 10);
        assert!((small.center[0] - 100.0).abs() < 0.01);
        assert!((small.center[1] - 75.0).abs() < 0.01);
    }

    #[test]
    fn wheel_split_two_lines_names() {
        // Короткие — одна строка
        assert_eq!(split_two_lines("Воркер", 15), ("Воркер".to_owned(), None));
        assert_eq!(
            split_two_lines("API-шлюз", 15),
            ("API-шлюз".to_owned(), None)
        );
        // Длинные — перенос по пробелу, обе части влезают
        assert_eq!(
            split_two_lines("Балансировщик нагрузки", 15),
            ("Балансировщик".to_owned(), Some("нагрузки".to_owned()))
        );
        assert_eq!(
            split_two_lines("БД SQL (реплика)", 15),
            ("БД SQL".to_owned(), Some("(реплика)".to_owned()))
        );
        assert_eq!(
            split_two_lines("Сервис аутентификации", 15),
            ("Сервис".to_owned(), Some("аутентификации".to_owned()))
        );
        // Нет пробела — дефис
        assert_eq!(
            split_two_lines("TCP-балансировщик", 15),
            ("TCP-".to_owned(), Some("балансировщик".to_owned()))
        );
        // Развалить нельзя — одна строка (клип рендером)
        assert_eq!(
            split_two_lines("оченьдлинноеслово безпробелов", 8),
            ("оченьдлинноеслово безпробелов".to_owned(), None)
        );
    }

    #[test]
    fn wheel_categories_then_templates_flow() {
        // Сценарий: клик по плашке категории выбирает категорию
        // (появляются шаблонные плашки), клик по плашке шаблона — цель
        // для инстанциации
        let registry = registry();
        let menu = WheelMenu {
            screen: [500.0, 400.0],
            world: [10.0, 20.0],
            category: None,
        };
        let categories = registry.categories();
        let cache_index = categories
            .iter()
            .position(|c| *c == "cache")
            .expect("cache");
        let geo = wheel_geometry(menu.screen, 1280.0, 800.0, categories.len(), 0);
        let plate = geo
            .plates
            .iter()
            .find(|p| p.hit == WheelHit::Category(cache_index))
            .expect("плашка cache");
        let center = [
            plate.rect[0] + plate.rect[2] / 2.0,
            plate.rect[1] + plate.rect[3] / 2.0,
        ];
        assert_eq!(geo.hit(center), Some(WheelHit::Category(cache_index)));
        // После выбора категории: 1 шаблон cache на нижнем кольце
        let menu2 = WheelMenu {
            screen: menu.screen,
            world: menu.world,
            category: Some("cache".to_owned()),
        };
        let geo2 = wheel_geometry(
            menu2.screen,
            1280.0,
            800.0,
            categories.len(),
            registry.by_category("cache").len(),
        );
        let tpl = geo2
            .plates
            .iter()
            .find(|p| p.hit == WheelHit::Template(0))
            .expect("плашка шаблона");
        let center = [
            tpl.rect[0] + tpl.rect[2] / 2.0,
            tpl.rect[1] + tpl.rect[3] / 2.0,
        ];
        assert_eq!(geo2.hit(center), Some(WheelHit::Template(0)));
    }
}
