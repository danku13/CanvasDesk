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

// --- Панель шаблонов (Ctrl+P, FR-024 — в стиле Miro) ---

/// Ширина панели, логические px (клампится к окну).
pub const PANEL_WIDTH: f32 = 340.0;
/// Боковой отступ панели от ЛЕВОГО края окна (FR-024: док слева —
/// паттерн Miro Template picker).
pub const PANEL_MARGIN: f32 = 12.0;
/// Отступ от верхнего и нижнего края окна (панель — во всю высоту).
pub const PANEL_TOP_MARGIN: f32 = 12.0;
/// Внутренний отступ содержимого.
pub const PANEL_PADDING: f32 = 10.0;
/// Высота шапки панели («Шаблоны» + счётчик).
pub const PANEL_HEADER_H: f32 = 30.0;
/// Высота поля поиска.
pub const INPUT_HEIGHT: f32 = 32.0;
/// Высота строки категории (чипы-фильтры).
pub const CATEGORY_ROW_H: f32 = 26.0;
/// Шаг строки шаблона: карточка + зазор.
pub const ROW_HEIGHT: f32 = 46.0;
/// Высота заголовка секции категории (Miro-стиль группировки).
pub const SECTION_HEIGHT: f32 = 24.0;
/// Максимум видимых строк-шаблонов (далее — прокрутка стрелками).
pub const MAX_VISIBLE_ROWS: usize = 12;
/// Сторона квад-иконки в строке панели (логические px).
pub const TEMPLATE_ROW_ICON: f32 = 18.0;
/// Сторона плитки под иконкой (Miro-стиль: иконка на скруглённом квадрате).
pub const TEMPLATE_ROW_TILE: f32 = 28.0;
/// Окно прокрутки в строках (секции+шаблоны вперемешку) — для
/// следования выделения при клавиатурной навигации.
pub const SCROLL_WINDOW: usize = 14;
/// Высота футера-подсказки панели (CR-011): резервируется в геометрии,
/// строки списка под неё не заходят.
pub const PANEL_FOOTER_H: f32 = 24.0;

/// Ширина чипа категории по имени (CR-011): считается по СИМВОЛАМ
/// (`chars().count()`), не по байтам UTF-8 — иначе кириллические категории
/// получали чип вдвое шире текста и вылезали за панель.
pub fn category_chip_width(name: &str) -> f32 {
    name.chars().count() as f32 * 7.5 + 20.0
}

/// Строка панели (FR-024): заголовок секции категории или строка шаблона.
/// Секции — группировка реестра «как в Miro»; выделение (клавиатура) и
/// клик-вставка цели только строки шаблонов.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelRow {
    /// Заголовок секции — имя категории.
    Section(String),
    /// Строка шаблона — индекс в `registry.list()`.
    Template(usize),
}

/// Состояние боковой палитры шаблонов (FR-018, `Ctrl+P`; FR-025 —
/// постоянный левый док). Поле ввода — своя лёгкая модель (однострочная,
/// как `SearchInput`), НЕ `EditingSession`.
///
/// FR-025: `open` — развёрнут ли док (по умолчанию true — палитра
/// доступна постоянно, как в Miro); `focused` — принимает ли панель
/// клавиатуру (фокус в поиске: Ctrl+P или клик по полю фильтра). Клик
/// по канвасу мимо панели фокус снимает, док не закрывает.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplatePanel {
    pub open: bool,
    /// Клавиатурный фокус: true — клавиши уходят в панель (фильтр, стрелки,
    /// Enter, Esc), false — в канвас (панель видна, но не перехватывает).
    pub focused: bool,
    /// Строка фильтра (подстрока без учёта регистра по имени/описанию/id).
    pub filter: String,
    /// Байтовая позиция каретки в `filter`.
    pub cursor: usize,
    /// Фильтр категории (клик по чипу); None — все категории.
    pub category: Option<String>,
    /// Выбранная строка-шаблон (ординал среди [`PanelRow::Template`]).
    pub selected: usize,
    /// Первая видимая строка (индекс в векторе [`PanelRow`], прокрутка).
    pub scroll_top: usize,
}

impl TemplatePanel {
    /// Новая панель — развёрнутый док (FR-025: палитра доступна постоянно),
    /// клавиатурный фокус снят.
    pub fn new() -> Self {
        Self {
            open: true,
            focused: false,
            filter: String::new(),
            cursor: 0,
            category: None,
            selected: 0,
            scroll_top: 0,
        }
    }

    /// Развернуть док с клавиатурным фокусом в поиске (сброс фильтров —
    /// каждый вызов с чистого листа). Вызывается по Ctrl+P.
    pub fn open(&mut self) {
        self.open = true;
        self.focused = true;
        self.filter.clear();
        self.cursor = 0;
        self.category = None;
        self.selected = 0;
        self.scroll_top = 0;
    }

    /// Свернуть док в полосу-ручку (FR-025; Esc). Фокус снимается.
    pub fn close(&mut self) {
        self.open = false;
        self.focused = false;
    }

    /// Развернуть док без сброса фильтров и без клавиатурного фокуса
    /// (клик по полосе-ручке свёрнутого дока).
    pub fn expand(&mut self) {
        self.open = true;
    }

    /// Клавиатурный фокус в поиск (Ctrl+P по развёрнутому доку: фильтр
    /// сохраняется, каретка — в конец строки).
    pub fn focus_search(&mut self) {
        self.focused = true;
        self.cursor = self.filter.len();
    }

    /// Снять клавиатурный фокус (клик по канвасу): док остаётся развёрнут.
    pub fn unfocus(&mut self) {
        self.focused = false;
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

    /// Сдвиг выделения (стрелки): нумеруются ТОЛЬКО строки-шаблоны
    /// (секции — заголовки, не цели); true — было изменение. Прокрутка
    /// следует за выделением: окно [`SCROLL_WINDOW`] строк, над первой
    /// строкой категории показывается её заголовок.
    pub fn move_selection(&mut self, delta: i32, rows: &[PanelRow]) -> bool {
        let total = template_row_count(rows);
        if total == 0 {
            return false;
        }
        let next = (self.selected as i32 + delta).clamp(0, total as i32 - 1);
        if next == self.selected as i32 {
            return false;
        }
        self.selected = next as usize;
        let Some(row_idx) = row_of_ordinal(rows, self.selected) else {
            return true;
        };
        if row_idx < self.scroll_top {
            // Заголовок секции над строкой — показать и его
            self.scroll_top = if row_idx > 0 && matches!(rows[row_idx - 1], PanelRow::Section(_)) {
                row_idx - 1
            } else {
                row_idx
            };
        } else if row_idx >= self.scroll_top + SCROLL_WINDOW {
            self.scroll_top = row_idx + 1 - SCROLL_WINDOW;
        }
        true
    }
}

/// Default для совместимости (`TemplatePanel::new` — развёрнутый док).
impl Default for TemplatePanel {
    fn default() -> Self {
        Self::new()
    }
}

/// FR-025: нажатие на строку шаблона палитры — кандидат в drag: без
/// движения порога это клик (вставка в центр viewport), с движением —
/// drag с ghost-превью (вставка в точку курсора). Отпускание решает.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelDrag {
    /// Индекс шаблона в `registry.list()`.
    pub index: usize,
    /// Точка нажатия (screen, логические px).
    pub press: Vec2,
    /// Порог пройден — идёт drag (ghost-превью, вставка в точку курсора).
    pub active: bool,
}

impl PanelDrag {
    /// Порог перевода нажатия в drag, логические px (как рамка выделения).
    pub const THRESHOLD: f32 = 4.0;

    /// Обновление по позиции курсора: true — только что перешло в drag.
    pub fn update(&mut self, cursor: Vec2) -> bool {
        if self.active {
            return false;
        }
        let moved = (cursor[0] - self.press[0]).abs() > Self::THRESHOLD
            || (cursor[1] - self.press[1]).abs() > Self::THRESHOLD;
        if moved {
            self.active = true;
        }
        moved
    }
}

/// Число строк-шаблонов в наборе строк панели (секции не считаются).
pub fn template_row_count(rows: &[PanelRow]) -> usize {
    rows.iter()
        .filter(|row| matches!(row, PanelRow::Template(_)))
        .count()
}

/// Позиция строки-шаблона по ординалу выделения (k-я строка-шаблон →
/// индекс в `rows`). Секции пропускаются.
pub fn row_of_ordinal(rows: &[PanelRow], ordinal: usize) -> Option<usize> {
    let mut seen = 0_usize;
    for (index, row) in rows.iter().enumerate() {
        if matches!(row, PanelRow::Template(_)) {
            if seen == ordinal {
                return Some(index);
            }
            seen += 1;
        }
    }
    None
}

/// Строки панели (FR-024): при пустом фильтре и без чипа категории —
/// группировка по категориям с заголовками секций (порядок реестра,
/// паттерн Miro Template picker); при поиске/фильтре — плоский список
/// совпадений (секции не имеют смысла в результатах поиска).
pub fn panel_rows(registry: &TemplateRegistry, panel: &TemplatePanel) -> Vec<PanelRow> {
    let query = panel.filter.to_lowercase();
    let matches = |manifest: &TemplateManifest| -> bool {
        if let Some(category) = &panel.category {
            if &manifest.category != category {
                return false;
            }
        }
        query.is_empty()
            || manifest.name.to_lowercase().contains(&query)
            || manifest.description.to_lowercase().contains(&query)
            || manifest.id.to_lowercase().contains(&query)
    };
    let grouped = panel.filter.is_empty() && panel.category.is_none();
    if !grouped {
        return registry
            .list()
            .iter()
            .enumerate()
            .filter(|(_, manifest)| matches(manifest))
            .map(|(index, _)| PanelRow::Template(index))
            .collect();
    }
    let mut rows = Vec::new();
    for category in registry.categories() {
        let indexes: Vec<usize> = registry
            .list()
            .iter()
            .enumerate()
            .filter(|(_, manifest)| manifest.category == *category)
            .map(|(index, _)| index)
            .collect();
        if indexes.is_empty() {
            continue;
        }
        rows.push(PanelRow::Section((*category).to_owned()));
        rows.extend(indexes.into_iter().map(PanelRow::Template));
    }
    rows
}

/// Геометрия панели на кадр (логические px) — левый док во всю высоту
/// окна (FR-024). `rows` — результат [`panel_rows`].
#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    pub panel_rect: [f32; 4],
    /// Шапка панели («Шаблоны» + счётчик).
    pub header_rect: [f32; 4],
    pub input_rect: [f32; 4],
    /// Чипы категорий: (rect, имя категории, активен).
    pub category_rects: Vec<([f32; 4], String, bool)>,
    /// Rect'ы видимых строк — параллельно [`PanelLayout::rows`]
    /// (секции и шаблоны в одном списке).
    pub row_rects: Vec<[f32; 4]>,
    /// Видимые строки (параллельно row_rects).
    pub rows: Vec<PanelRow>,
    /// Футер-подсказка (CR-011): единый rect для рендера и hit-test —
    /// строки списка в него не заходят (резерв в `panel_layout`).
    pub footer_rect: [f32; 4],
    /// Кнопка сворачивания дока в шапке (FR-025; 22×22 у правого края
    /// шапки) — единый rect для рендера и hit-test.
    pub collapse_rect: [f32; 4],
}

/// Ширина полосы-ручки свёрнутого дока палитры (FR-025), логические px.
pub const COLLAPSED_STRIP_W: f32 = 28.0;
/// Высота полосы-ручки свёрнутого дока (кнопка «развернуть»), логические px.
pub const COLLAPSED_STRIP_H: f32 = 44.0;

/// Полоса-ручка свёрнутого дока палитры (FR-025): у левого края вверху;
/// клик по ней разворачивает док. Единый источник для рендера и hit-test.
pub fn collapsed_strip_rect(_window_h: f32) -> [f32; 4] {
    [
        PANEL_MARGIN,
        PANEL_TOP_MARGIN,
        COLLAPSED_STRIP_W,
        COLLAPSED_STRIP_H,
    ]
}

pub fn panel_layout(
    window_w: f32,
    window_h: f32,
    registry: &TemplateRegistry,
    panel: &TemplatePanel,
    rows: &[PanelRow],
) -> PanelLayout {
    let width = PANEL_WIDTH.min((window_w - PANEL_MARGIN * 2.0).max(0.0));
    // FR-024: док у ЛЕВОГО края, во всю высоту окна (как Miro)
    let x = PANEL_MARGIN;
    let y = PANEL_TOP_MARGIN;
    let height = (window_h - PANEL_TOP_MARGIN * 2.0).max(0.0);
    let inner_w = width - PANEL_PADDING * 2.0;

    let header_rect = [
        x + PANEL_PADDING,
        y + PANEL_PADDING,
        inner_w,
        PANEL_HEADER_H,
    ];
    let input_rect = [
        x + PANEL_PADDING,
        y + PANEL_PADDING + PANEL_HEADER_H + 6.0,
        inner_w,
        INPUT_HEIGHT,
    ];

    // Чипы категорий: одна строка, ширина по имени (+ паддинг), перенос
    // не делаем — в v1 категорий ≤ 6
    let categories = registry.categories();
    let mut category_rects = Vec::with_capacity(categories.len());
    let mut cx = x + PANEL_PADDING;
    let chips_y = input_rect[1] + INPUT_HEIGHT + 6.0;
    for category in &categories {
        let w = category_chip_width(category);
        if cx + w > x + width - PANEL_PADDING {
            break; // не влезли — остальные доступны прокруткой фильтра
        }
        let active = panel.category.as_deref() == Some(*category);
        category_rects.push((
            [cx, chips_y, w, CATEGORY_ROW_H],
            (*category).to_owned(),
            active,
        ));
        cx += w + 6.0;
    }

    let rows_top = chips_y + CATEGORY_ROW_H + 6.0;
    // CR-011: резерв под футер-подсказку — строки в неё не заходят
    let footer_rect = [
        x + PANEL_PADDING,
        y + height - PANEL_PADDING - PANEL_FOOTER_H,
        inner_w,
        PANEL_FOOTER_H,
    ];
    let bottom_limit = footer_rect[1];
    let mut row_rects = Vec::new();
    let mut visible_rows = Vec::new();
    let mut cursor_y = rows_top;
    let mut shown_templates = 0_usize;
    for row in rows.iter().skip(panel.scroll_top) {
        if matches!(row, PanelRow::Template(_)) && shown_templates >= MAX_VISIBLE_ROWS {
            break;
        }
        let (step, card_h) = match row {
            PanelRow::Section(_) => (SECTION_HEIGHT, SECTION_HEIGHT),
            PanelRow::Template(_) => (ROW_HEIGHT, ROW_HEIGHT - 4.0),
        };
        if cursor_y + card_h > bottom_limit {
            break; // строка не влезает в панель — прокрутка
        }
        row_rects.push([x + PANEL_PADDING, cursor_y, inner_w, card_h]);
        visible_rows.push(row.clone());
        cursor_y += step;
        if matches!(row, PanelRow::Template(_)) {
            shown_templates += 1;
        }
    }
    PanelLayout {
        panel_rect: [x, y, width, height],
        header_rect,
        input_rect,
        category_rects,
        row_rects,
        rows: visible_rows,
        footer_rect,
        collapse_rect: [
            x + width - PANEL_PADDING - 22.0,
            y + PANEL_PADDING + 4.0,
            22.0,
            22.0,
        ],
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

    // --- Панель (FR-024: секции, левый док) ---

    fn template_indexes(rows: &[PanelRow]) -> Vec<usize> {
        rows.iter()
            .filter_map(|row| match row {
                PanelRow::Template(index) => Some(*index),
                PanelRow::Section(_) => None,
            })
            .collect()
    }

    #[test]
    fn panel_filter_by_name_and_id() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        // Пустой фильтр — секции по категориям + все 5 шаблонов
        let rows = panel_rows(&registry, &panel);
        assert_eq!(template_indexes(&rows).len(), 5);
        assert_eq!(
            rows.iter()
                .filter(|r| matches!(r, PanelRow::Section(_)))
                .count(),
            4
        );
        // По имени (регистр не важен) — плоский список без секций
        panel.insert_str("load");
        let rows = panel_rows(&registry, &panel);
        assert_eq!(template_indexes(&rows), vec![0]);
        assert!(rows.iter().all(|r| matches!(r, PanelRow::Template(_))));
        // По id
        panel.filter.clear();
        panel.cursor = 0;
        panel.insert_str("mock.db");
        assert_eq!(template_indexes(&panel_rows(&registry, &panel)), vec![1]);
        // По описанию
        panel.filter.clear();
        panel.cursor = 0;
        panel.insert_str("партиции");
        assert_eq!(template_indexes(&panel_rows(&registry, &panel)), vec![4]);
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
        let indexes = template_indexes(&rows);
        assert_eq!(indexes.len(), 2);
        assert_eq!(registry.list()[indexes[0]].category, "backend");
        assert_eq!(registry.list()[indexes[1]].category, "backend");
    }

    #[test]
    fn panel_grouping_follows_registry_order() {
        // Секции — в порядке реестра; шаблоны внутри — свои индексы
        let registry = registry();
        let panel = TemplatePanel::new();
        let rows = panel_rows(&registry, &panel);
        let categories: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                PanelRow::Section(name) => Some(name.as_str()),
                PanelRow::Template(_) => None,
            })
            .collect();
        assert_eq!(categories, registry.categories());
        // Каждый Section предшествует своим Template
        for (pos, row) in rows.iter().enumerate() {
            if let PanelRow::Section(name) = row {
                let next = rows.get(pos + 1).expect("секция не пустая");
                match next {
                    PanelRow::Template(index) => {
                        assert_eq!(registry.list()[*index].category, name.as_str())
                    }
                    PanelRow::Section(_) => panic!("секция без шаблонов"),
                }
            }
        }
    }

    #[test]
    fn panel_row_of_ordinal_skips_sections() {
        let rows = vec![
            PanelRow::Section("backend".to_owned()),
            PanelRow::Template(0),
            PanelRow::Template(1),
            PanelRow::Section("cache".to_owned()),
            PanelRow::Template(2),
        ];
        assert_eq!(template_row_count(&rows), 3);
        assert_eq!(row_of_ordinal(&rows, 0), Some(1));
        assert_eq!(row_of_ordinal(&rows, 1), Some(2));
        assert_eq!(row_of_ordinal(&rows, 2), Some(4));
        assert_eq!(row_of_ordinal(&rows, 3), None);
    }

    #[test]
    fn panel_dock_mode_focus_lifecycle() {
        // FR-025: док развёрнут по умолчанию, фокус — только по Ctrl+P
        // или клику в поиск; Esc сворачивает док; клик по канвасу мимо
        // панели снимает фокус, не закрывая док
        let mut panel = TemplatePanel::new();
        assert!(panel.open, "FR-025: палитра доступна постоянно");
        assert!(!panel.focused);
        panel.focus_search();
        assert!(panel.focused);
        panel.unfocus();
        assert!(panel.open);
        assert!(!panel.focused);
        // Ctrl+P по свёрнутому: развернуть + фокус + чистый фильтр
        panel.close();
        assert!(!panel.open && !panel.focused);
        panel.open();
        assert!(panel.open && panel.focused);
        panel.insert_str("lb");
        // Ctrl+P по развёрнутому: фокус в поиск, фильтр сохраняется
        panel.focus_search();
        assert_eq!(panel.filter, "lb");
        assert!(panel.focused);
        assert_eq!(panel.cursor, panel.filter.len());
        // Esc: свернуть док (фокус снят)
        panel.close();
        assert!(!panel.open && !panel.focused);
        // Клик по ручке: развернуть без фокуса и без сброса фильтра
        panel.expand();
        assert!(panel.open && !panel.focused && panel.filter == "lb");
    }

    #[test]
    fn panel_collapse_button_and_strip_geometry() {
        let registry = registry();
        let panel = TemplatePanel::new();
        let rows = panel_rows(&registry, &panel);
        let lay = panel_layout(1280.0, 800.0, &registry, &panel, &rows);
        // Кнопка сворачивания — в правой части шапки, внутри панели
        let c = lay.collapse_rect;
        assert!(c[0] >= lay.header_rect[0]);
        assert!(c[0] + c[2] <= lay.panel_rect[0] + lay.panel_rect[2] - PANEL_PADDING + 0.01);
        assert!(c[1] >= lay.header_rect[1]);
        assert!(c[1] + c[3] <= lay.header_rect[1] + lay.header_rect[3] + 0.01);
        // Полоса-ручка свёрнутого дока — у левого края, внутри высоты окна
        let strip = collapsed_strip_rect(800.0);
        assert_eq!(strip[0], PANEL_MARGIN);
        assert_eq!(strip[1], PANEL_TOP_MARGIN);
        assert_eq!(strip[2], COLLAPSED_STRIP_W);
        assert_eq!(strip[3], COLLAPSED_STRIP_H);
    }

    #[test]
    fn panel_drag_threshold() {
        // FR-025: до порога — клик, после — drag
        let mut drag = PanelDrag {
            index: 0,
            press: [100.0, 100.0],
            active: false,
        };
        assert!(!drag.update([103.0, 101.0]));
        assert!(!drag.active, "в пределах порога — это клик");
        assert!(drag.update([105.0, 100.0]));
        assert!(drag.active, "порог пройден — drag");
        // Повторные обновления не переключают состояние
        assert!(!drag.update([200.0, 200.0]));
        assert!(drag.active);
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
    fn panel_selection_scroll_follows_with_sections() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        let rows = panel_rows(&registry, &panel);
        let total = template_row_count(&rows);
        assert!(panel.move_selection(1, &rows));
        assert_eq!(panel.selected, 1);
        // Границы: вниз до последнего шаблона, вверх до первого
        panel.move_selection(100, &rows);
        assert_eq!(panel.selected, total - 1);
        panel.move_selection(-100, &rows);
        assert_eq!(panel.selected, 0);
        assert_eq!(panel.scroll_top, 0);
        // Прокрутка догоняет выделение: выделенная строка в окне
        panel.move_selection(total as i32 - 1, &rows);
        let row_idx = row_of_ordinal(&rows, panel.selected).expect("строка");
        assert!(row_idx >= panel.scroll_top);
        assert!(row_idx < panel.scroll_top + SCROLL_WINDOW);
        // Пустой список — no-op
        assert!(!panel.move_selection(1, &[]));
    }

    #[test]
    fn panel_layout_geometry() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        let rows = panel_rows(&registry, &panel);
        let lay = panel_layout(1280.0, 800.0, &registry, &panel, &rows);
        // FR-024: док у ЛЕВОГО края, во всю высоту окна
        assert!((lay.panel_rect[0] - PANEL_MARGIN).abs() < 0.01);
        assert!((lay.panel_rect[1] - PANEL_TOP_MARGIN).abs() < 0.01);
        assert!((lay.panel_rect[3] - (800.0 - PANEL_TOP_MARGIN * 2.0)).abs() < 0.01);
        // Шапка и поле ввода внутри панели
        assert!(lay.header_rect[0] > lay.panel_rect[0]);
        assert!(lay.input_rect[0] > lay.panel_rect[0]);
        assert!(lay.input_rect[1] > lay.header_rect[1]);
        // Строки не вылезают за панель (группировка: секции + 5 шаблонов)
        assert_eq!(lay.rows.len(), rows.len());
        for rect in &lay.row_rects {
            assert!(rect[0] >= lay.panel_rect[0]);
            assert!(rect[0] + rect[2] <= lay.panel_rect[0] + lay.panel_rect[2] + 0.01);
            assert!(rect[1] + rect[3] <= lay.panel_rect[1] + lay.panel_rect[3] + 0.01);
        }
        // Чипы категорий — 4 (backend/cache/network/queue)
        assert_eq!(lay.category_rects.len(), 4);
        // CR-011: резерв под футер — ни одна строка не пересекает footer_rect
        let footer = lay.footer_rect;
        assert!(footer[1] + footer[3] <= lay.panel_rect[1] + lay.panel_rect[3] + 0.01);
        for rect in &lay.row_rects {
            let overlap = footer[0] < rect[0] + rect[2]
                && rect[0] < footer[0] + footer[2]
                && footer[1] < rect[1] + rect[3]
                && rect[1] < footer[1] + footer[3];
            assert!(!overlap, "строка палитры налезла на футер");
        }
        // Малое окно: строки обрезаются по высоте панели, без паники
        let small = panel_layout(400.0, 300.0, &registry, &panel, &rows);
        for rect in &small.row_rects {
            assert!(rect[1] + rect[3] <= small.panel_rect[1] + small.panel_rect[3] + 0.01);
        }
    }

    #[test]
    fn panel_chip_width_counts_chars_not_bytes() {
        // CR-011: ширина чипа — по символам, не по байтам UTF-8: кириллица
        // (2 байта/символ) давала чип вдвое шире текста и чипы вылезали
        // за панель, остальные категории молча отбрасывались
        assert!((category_chip_width("db") - (2.0 * 7.5 + 20.0)).abs() < 0.01);
        assert!((category_chip_width("БД") - (2.0 * 7.5 + 20.0)).abs() < 0.01);
        assert!((category_chip_width("Очереди") - (7.0 * 7.5 + 20.0)).abs() < 0.01);
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
