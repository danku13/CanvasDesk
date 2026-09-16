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
//!   (donut-сектора в стиле circular-menu: кольца от центрального отверстия
//!   наружу — шаблоны выбранной категории, внешние кольца — категории;
//!   при переполнении кольца сектора уходят в концентрические под-кольца —
//!   рестайл FR-022 2026-09-16 по скриншоту пользователя);
//! - иконки — квад-иконки (как в палитре действий, без SVG/resvg) —
//!   геометрия в [`crate::cards`-пайплайне]; здесь — [`icon_key`].

use canvas_core::templates::{TemplateManifest, TemplateRegistry};
use canvas_render::sectors::{angle_gap, norm_angle};

use std::time::{Duration, Instant};

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
/// постоянный левый док; ревизия FR-025 2026-09-16 — палитра ПРИМАРНО
/// свёрнута: вертикальная полоса категорий по центру слева, hover раскрывает
/// flyout справа). Поле ввода — своя лёгкая модель (однострочная,
/// как `SearchInput`), НЕ `EditingSession`.
///
/// Ревизия FR-025: `open` — развёрнут ли док (по умолчанию false — свёрнута
/// в полосу категорий); `focused` — принимает ли панель клавиатуру (фокус в
/// поиске: Ctrl+P или клик по полю фильтра). Клик по канвасу мимо панели
/// фокус снимает, док не закрывает.
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
    /// Новая панель — СВЁРНУТАЯ (ревизия FR-025: палитра примарно свёрнута,
    /// полоса категорий по центру слева; развёрнутый док — по Ctrl+P),
    /// клавиатурный фокус снят.
    pub fn new() -> Self {
        Self {
            open: false,
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

/// Default для совместимости (`TemplatePanel::new` — свёрнутая полоса).
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

/// Ширина полосы категорий свёрнутой палитры (ревизия FR-025, 2026-09-16),
/// логические px. Ширина полосы — максимум ширин чипов категорий, но не
/// уже этой границы (впритык под короткие имена не сжимаем).
pub const STRIP_MIN_W: f32 = 64.0;
/// Вертикальный внутренний паддинг полосы (над первой строкой и под шевроном).
pub const STRIP_PAD_V: f32 = 6.0;
/// Горизонтальный внутренний паддинг полосы.
pub const STRIP_PAD_H: f32 = 4.0;
/// Запас ширины полосы под счётчик шаблонов («backend · 10») рядом с именем.
pub const STRIP_COUNT_SLACK: f32 = 28.0;
/// Зазор между полосой категорий и flyout, логические px.
pub const FLYOUT_GAP: f32 = 6.0;
/// Минимальная ширина flyout, логические px.
pub const FLYOUT_MIN_W: f32 = 260.0;
/// Вертикальный внутренний паддинг flyout (над первой/под последней строкой).
pub const FLYOUT_PAD_V: f32 = 6.0;
/// Горизонтальный внутренний паддинг flyout.
pub const FLYOUT_PAD_H: f32 = 8.0;

/// Ширина flyout: уже дока на 40 px, но не уже [`FLYOUT_MIN_W`].
pub fn flyout_width() -> f32 {
    (PANEL_WIDTH - 40.0).max(FLYOUT_MIN_W)
}

/// Геометрия свёрнутой палитры (ревизия FR-025): вертикальная полоса
/// категорий у левого края, центрированная по вертикали окна. Все rect'ы —
/// `[x, y, w, h]` (конвенция `point_in_rect`). Единый источник для рендера
/// и hit-test: что нарисовано, по тому и клик/наведение.
#[derive(Debug, Clone, PartialEq)]
pub struct StripLayout {
    /// Прямоугольник полосы целиком (включая шеврон).
    pub rect: [f32; 4],
    /// Строки категорий: (rect, имя категории), порядок — порядок реестра.
    pub rows: Vec<([f32; 4], String)>,
    /// Шеврон «развернуть док» — строка внизу полосы.
    pub chevron_rect: [f32; 4],
}

/// Раскладка свёрнутой полосы категорий: ширина — по самому длинному чипу
/// (+ запас под счётчик), строки по [`CATEGORY_ROW_H`], полоса центрирована
/// по вертикали окна и клампится отступом [`PANEL_TOP_MARGIN`] сверху/снизу
/// (в маленьком окне не уходит за края выше верхнего отступа).
pub fn dock_strip_layout(categories: &[String], window_h: f32) -> StripLayout {
    let width = categories
        .iter()
        .map(|name| category_chip_width(name) + STRIP_COUNT_SLACK)
        .fold(STRIP_MIN_W, f32::max);
    let rows_h = categories.len() as f32 * CATEGORY_ROW_H;
    // + строка шеврона внизу полосы
    let height = rows_h + STRIP_PAD_V * 2.0 + CATEGORY_ROW_H;
    let y = ((window_h - height) / 2.0).clamp(
        PANEL_TOP_MARGIN,
        (window_h - height - PANEL_TOP_MARGIN).max(PANEL_TOP_MARGIN),
    );
    let x = PANEL_MARGIN;
    let rows = categories
        .iter()
        .enumerate()
        .map(|(i, name)| {
            (
                [
                    x + STRIP_PAD_H,
                    y + STRIP_PAD_V + i as f32 * CATEGORY_ROW_H,
                    width - STRIP_PAD_H * 2.0,
                    CATEGORY_ROW_H,
                ],
                name.clone(),
            )
        })
        .collect();
    StripLayout {
        rect: [x, y, width, height],
        rows,
        chevron_rect: [
            x + STRIP_PAD_H,
            y + STRIP_PAD_V + rows_h,
            width - STRIP_PAD_H * 2.0,
            CATEGORY_ROW_H,
        ],
    }
}

/// Геометрия flyout — выпадающего списка шаблонов категории справа от
/// полосы. Rect'ы — `[x, y, w, h]`. `row_rects` — только видимое окно строк
/// (обрезка по скроллу как у строк дока); `scroll_top` в раскладке уже
/// клампнут к `max_scroll`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyoutLayout {
    /// Прямоугольник flyout.
    pub rect: [f32; 4],
    /// Видимые строки (начиная с клампнутого `scroll_top`).
    pub row_rects: Vec<[f32; 4]>,
    /// Клампнутая первая видимая строка.
    pub scroll_top: usize,
    /// Максимум прокрутки в строках.
    pub max_scroll: usize,
}

/// Раскладка flyout справа от строки-категории (`strip_row_rect`, xywh):
/// ширина — [`flyout_width`] (кламп к правому краю окна с отступом
/// [`PANEL_MARGIN`]), высота — по числу items, но не больше окна минус
/// отступы; вертикально центрирована относительно строки-категории и
/// клампится в `[PANEL_TOP_MARGIN, window_h − h − PANEL_TOP_MARGIN]`, так что
/// строки никогда не вылезают за область видимости.
pub fn flyout_layout(
    strip_row_rect: [f32; 4],
    item_count: usize,
    window_w: f32,
    window_h: f32,
    scroll_top: usize,
) -> FlyoutLayout {
    let x = strip_row_rect[0] + strip_row_rect[2] + FLYOUT_GAP;
    let width = flyout_width().min((window_w - x - PANEL_MARGIN).max(FLYOUT_MIN_W));
    let natural_h = item_count as f32 * ROW_HEIGHT + FLYOUT_PAD_V * 2.0;
    let max_h = (window_h - PANEL_TOP_MARGIN * 2.0).max(ROW_HEIGHT + FLYOUT_PAD_V * 2.0);
    let height = natural_h.min(max_h);
    let visible_count = if item_count == 0 {
        0
    } else {
        (((height - FLYOUT_PAD_V * 2.0) / ROW_HEIGHT).floor() as usize)
            .max(1)
            .min(item_count)
    };
    let max_scroll = item_count.saturating_sub(visible_count);
    let top = scroll_top.min(max_scroll);
    let row_cy = strip_row_rect[1] + strip_row_rect[3] / 2.0;
    let y = (row_cy - height / 2.0).clamp(
        PANEL_TOP_MARGIN,
        (window_h - PANEL_TOP_MARGIN - height).max(PANEL_TOP_MARGIN),
    );
    let row_rects = (0..visible_count.min(item_count - top))
        .map(|v| {
            [
                x + FLYOUT_PAD_H,
                y + FLYOUT_PAD_V + v as f32 * ROW_HEIGHT,
                width - FLYOUT_PAD_H * 2.0,
                ROW_HEIGHT - 4.0,
            ]
        })
        .collect();
    FlyoutLayout {
        rect: [x, y, width, height],
        row_rects,
        scroll_top: top,
        max_scroll,
    }
}

/// Задержка открытия flyout по наведению (hover-intent, как у
/// `PaletteHover`): провод курсора через полосу к канвасу не мигает списками.
pub const STRIP_OPEN_DELAY_MS: u64 = 150;
/// Отсрочка закрытия после ухода курсора с полосы/flyout (grace period).
pub const STRIP_CLOSE_DELAY_MS: u64 = 300;

/// Состояние hover-раскрытия категорий свёрнутой полосы палитры (ревизия
/// FR-025). Механика — как у палитры выделения (`PaletteHover`, FR-009/010):
/// hover-intent 150 мс на открытие, grace 300 мс на закрытие, пин по клику
/// (WAI-ARIA menu button), удержание пока курсор внутри flyout. Скролл
/// flyout — колесом мыши, кламп к `max_scroll` (max_scroll зависит от числа
/// шаблонов категории — передаётся снаружи).
#[derive(Debug, Clone)]
pub struct StripHover {
    /// Раскрытая категория (индекс строки в `StripLayout::rows`).
    pub open: Option<usize>,
    /// Пин по клику: раскрытие держится после ухода курсора до повторного
    /// клика/Esc.
    pub pinned: bool,
    /// Первая видимая строка flyout (прокрутка колесом).
    pub scroll_top: usize,
    /// Строка полосы под курсором (None — мимо полосы) — для подсветки;
    /// смена строки — тоже повод для перерисовки.
    pub hovered_row: Option<usize>,
    /// (категория, момент входа курсора) — накопление hover-intent.
    trigger_since: Option<(usize, Instant)>,
    /// Момент ухода курсора с открытой зоны — отсрочка закрытия.
    left_since: Option<Instant>,
}

impl Default for StripHover {
    fn default() -> Self {
        Self::new()
    }
}

impl StripHover {
    pub fn new() -> Self {
        Self {
            open: None,
            pinned: false,
            scroll_top: 0,
            hovered_row: None,
            trigger_since: None,
            left_since: None,
        }
    }

    /// Идут кадры ожидания (hover-intent / grace) — `about_to_wait` держит
    /// цикл перерисовки, иначе задержки не сработают при неподвижном курсоре.
    pub fn pending(&self) -> bool {
        self.trigger_since.is_some() || self.left_since.is_some()
    }

    /// Кадровое обновление: `hovered` — строка категории под курсором,
    /// `in_flyout` — курсор внутри rect'а flyout раскрытой категории.
    /// true — раскрытие или строка под курсором изменились (нужна
    /// перерисовка); при неподвижном курсоре стабильно false — без
    /// самоподдерживающегося цикла кадров.
    pub fn update_at(&mut self, hovered: Option<usize>, in_flyout: bool, now: Instant) -> bool {
        let open_delay = Duration::from_millis(STRIP_OPEN_DELAY_MS);
        let close_delay = Duration::from_millis(STRIP_CLOSE_DELAY_MS);
        let row_changed = self.hovered_row != hovered;
        self.hovered_row = hovered;
        let mut changed = false;
        match hovered {
            Some(cat) => {
                self.left_since = None;
                match self.open {
                    Some(open) if open == cat => self.trigger_since = None,
                    _ => {
                        // Курсор на другой/закрытой категории — копим intent
                        match self.trigger_since {
                            Some((target, since)) if target == cat => {
                                if now.duration_since(since) >= open_delay {
                                    self.open = Some(cat);
                                    self.scroll_top = 0;
                                    self.trigger_since = None;
                                    changed = true;
                                }
                            }
                            _ => self.trigger_since = Some((cat, now)),
                        }
                    }
                }
            }
            None => {
                self.trigger_since = None;
                if in_flyout || self.pinned {
                    // Внутри flyout или закреплено кликом — держим открытым
                    self.left_since = None;
                } else if self.open.is_some() {
                    match self.left_since {
                        None => self.left_since = Some(now),
                        Some(since) => {
                            if now.duration_since(since) >= close_delay {
                                self.reset();
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        changed || row_changed
    }

    /// Клик по строке категории: пин-переключение раскрытия (открывает без
    /// задержки — намерение явное; повторный клик — закрыть).
    pub fn toggle_trigger(&mut self, category: usize) {
        if self.open == Some(category) && self.pinned {
            self.reset();
        } else {
            self.open = Some(category);
            self.pinned = true;
            self.scroll_top = 0;
            self.trigger_since = None;
            self.left_since = None;
        }
    }

    /// Прокрутка flyout на `delta` строк с клампом к `[0, max_scroll]`.
    pub fn scroll_by(&mut self, delta: i32, max_scroll: usize) {
        let cur = self.scroll_top as i32;
        self.scroll_top = (cur + delta).clamp(0, max_scroll as i32) as usize;
    }

    /// Сброс: flyout закрыт, пин и прокрутка сняты.
    pub fn reset(&mut self) {
        self.open = None;
        self.pinned = false;
        self.scroll_top = 0;
        self.hovered_row = None;
        self.trigger_since = None;
        self.left_since = None;
    }
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

// Рестайл FR-022 (2026-09-16) по скриншоту пользователя: «прямоугольные
// плашки на концентрических кольцах» заменены donut-секторами в стиле
// circular-menu (https://www.npmjs.com/package/circular-menu — ориентир
// по форме; пакет JS, реализация своя): кольцевые сектора с центральным
// отверстием, в каждом секторе иконка + короткая подпись, между секторами
// угловые зазоры. Рендер — instanced SDF-проход `canvas_render::sectors`
// (`sectors.wgsl`); форма сектора и hit-test считаются одной и той же
// полярной математикой (`sectors::angle_gap` — Rust-зеркало WGSL), поэтому
// расхождения «клик мимо нарисованного» нет (раньше угловой тест расходился
// с AABB плашек — заменён осознанно).

/// Толщина кольца wheel (радиальная ширина секторов), лог. px.
pub const WHEEL_RING_THICKNESS: f32 = 68.0;
/// Межкольцевой зазор (радиальный), лог. px.
pub const WHEEL_RING_GAP: f32 = 8.0;
/// Угловой зазор между соседними секторами кольца, радианы (≈2.5°).
pub const WHEEL_SECTOR_GAP: f32 = 0.043_633_23;
/// Минимальная дуга сектора на среднем радиусе кольца, лог. px — из неё
/// выводится вместимость кольца (окружность / (дуга + угловой зазор в px)).
pub const WHEEL_MIN_SECTOR_ARC: f32 = 60.0;
/// Максимум секторов в одном кольце; большее — концентрические под-кольца
/// (внешние вместительнее — окружность больше).
pub const WHEEL_RING_CAP: usize = 6;
/// Радиус центрального отверстия (дырка donut — глотает клик мимо секторов).
pub const WHEEL_HUB_R: f32 = 24.0;
/// FR-022: диаметр кнопки-хаба (клик = «назад»/«закрыть»). Крупная цель
/// ≥ 44 лог. px — гайдлайн сенсорных целей (Big Medium); совпадает с
/// удвоенным [`WHEEL_HUB_R`] — зона хаба и кнопка — одно и то же.
pub const WHEEL_HUB_D: f32 = 48.0;
/// Отступ wheel от краёв окна при клампе центра.
pub const WHEEL_SCREEN_MARGIN: f32 = 8.0;
/// Предел символов строки имени в подписи сектора (длиннее — перенос).
pub const WHEEL_TPL_TEXT_CHARS: usize = 15;

/// Состояние радиального меню шаблонов (FR-018, `Shift+клик` по пустому
/// месту). `screen` — центр в логических px (для рендера/hit-test),
/// `world` — точка канваса (куда инстанцируется выбранный шаблон).
#[derive(Debug, Clone, PartialEq)]
pub struct WheelMenu {
    pub screen: Vec2,
    pub world: Vec2,
    /// Выбранная категория (внутренние кольца — её шаблоны); None — только
    /// кольца категорий.
    pub category: Option<String>,
}

/// Цель попадания в wheel.
#[derive(Debug, Clone, PartialEq)]
pub enum WheelHit {
    /// Сектор категории (индекс в `registry.categories()`).
    Category(usize),
    /// Сектор шаблона (индекс ВНУТРИ категории — `registry.by_category`;
    /// сквозной по кольцам, кольца — только план раскладки).
    Template(usize),
}

/// Donut-сектор wheel: дуговой интервал [a0, a1] (радианы, 12 часов = -π/2,
/// по часовой) на кольце [r0, r1] (лог. px от центра). Единый источник
/// правды рендера (`SectorInstance` в `wheel_overlay`) и hit-test
/// ([`WheelGeometry::hit`]) — инвариант WYSIWYG.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelSector {
    pub a0: f32,
    pub a1: f32,
    pub r0: f32,
    pub r1: f32,
    pub hit: WheelHit,
}

impl WheelSector {
    /// Середина дуги сектора (для иконки/подписи и hit-пробы тестов).
    pub fn mid_angle(&self) -> f32 {
        (self.a0 + self.a1) / 2.0
    }
    /// Средний радиус кольца сектора (для иконки/подписи).
    pub fn mid_radius(&self) -> f32 {
        (self.r0 + self.r1) / 2.0
    }
}

/// Геометрия wheel на кадр: центр (с клампом к окну), сектора (шаблоны —
/// внутренние кольца от дырки наружу, категории — внешние), внешний радиус,
/// квадрат кнопки-хаба. Строится чистой функцией [`wheel_geometry`] — и
/// рендер (`wheel_overlay`), и клики ([`WheelGeometry::hit`]) вызывают её с
/// теми же аргументами, поэтому расхождений быть не может.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelGeometry {
    /// Центр меню: точка клика, сдвинутая клампом внутрь окна, если wheel
    /// целиком не влезает.
    pub center: Vec2,
    /// Сектора: шаблоны (изнутри наружу), затем категории.
    pub sectors: Vec<WheelSector>,
    /// Внешний радиус (максимальный r1 секторов; для «клик заметно дальше —
    /// закрыть» и клампа центра).
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

    /// Сектор под курсором (screen px): полярный hit-test — кольцо по
    /// радиусу (r0 ≤ r ≤ r1), угловая принадлежность — знаковое угловое
    /// расстояние `sectors::angle_gap` (тот же код, что в WGSL-шейдере
    /// рендера, поэтому клик всегда попадает в нарисованное). В угловом
    /// зазоре, в дырке (r < WHEEL_HUB_R) и за внешним радиусом — None.
    pub fn hit(&self, cursor: Vec2) -> Option<WheelHit> {
        let dx = cursor[0] - self.center[0];
        let dy = cursor[1] - self.center[1];
        let r = dx.hypot(dy);
        let theta = norm_angle(dy.atan2(dx));
        self.sectors
            .iter()
            .find(|s| r >= s.r0 && r <= s.r1 && angle_gap(theta, s.a0, s.a1) <= 0.0)
            .map(|s| s.hit.clone())
    }
}

/// Радиусы кольца `k` (0 — кольцо у дырки): r0 = HUB_R + k·(толщина+зазор),
/// r1 = r0 + толщина.
fn ring_radii(k: usize) -> (f32, f32) {
    let r0 = WHEEL_HUB_R + k as f32 * (WHEEL_RING_THICKNESS + WHEEL_RING_GAP);
    (r0, r0 + WHEEL_RING_THICKNESS)
}

/// Вместимость кольца `k`: число секторов, чья дуга на среднем радиусе
/// ≥ [`WHEEL_MIN_SECTOR_ARC`] с учётом углового зазора между соседями.
/// Гарантированно ≥ 2 (при HUB_R=24/толщине 68 кольцо 0 вмещает 5).
pub fn ring_capacity(k: usize) -> usize {
    let (r0, r1) = ring_radii(k);
    let r_mid = (r0 + r1) / 2.0;
    let arc_gap = WHEEL_SECTOR_GAP * r_mid;
    ((std::f32::consts::TAU * r_mid / (WHEEL_MIN_SECTOR_ARC + arc_gap)).floor() as usize)
        .clamp(2, WHEEL_RING_CAP)
}

/// План колец: `count` секторов по кольцам вместимости [`ring_capacity`]
/// (у внешних колец она больше), сбалансированно: 10 → [5, 5], 11 → [5, 6],
/// 7 → [3, 4], 15 → [5, 5, 5]. Каждая часть ≤ вместимости своего кольца.
pub fn wheel_ring_plan(count: usize) -> Vec<usize> {
    if count == 0 {
        return Vec::new();
    }
    // Минимальное число колец, чья суммарная вместимость покрывает count
    let mut rings = 0_usize;
    let mut total = 0_usize;
    while total < count {
        total += ring_capacity(rings);
        rings += 1;
    }
    let base = count / rings;
    let extra = count % rings;
    (0..rings)
        .map(|i| base + usize::from(i + extra >= rings))
        .collect()
}

/// Сектора одного кольца: `count` секторов на кольце `k`, цель каждого —
/// через `hit_of` (нарастающий сквозной индекс — за пределами функции).
/// Старт — 12 часов, по часовой; между соседними секторами угловой зазор
/// [`WHEEL_SECTOR_GAP`].
fn ring_sectors(k: usize, count: usize, hit_of: impl FnMut(usize) -> WheelHit) -> Vec<WheelSector> {
    let (r0, r1) = ring_radii(k);
    let step = std::f32::consts::TAU / count as f32;
    let start = -std::f32::consts::FRAC_PI_2;
    let mut hit_of = hit_of;
    (0..count)
        .map(|i| {
            let a0 = start + step * i as f32 + WHEEL_SECTOR_GAP / 2.0;
            let a1 = start + step * (i + 1) as f32 - WHEEL_SECTOR_GAP / 2.0;
            WheelSector {
                a0,
                a1,
                r0,
                r1,
                hit: hit_of(i),
            }
        })
        .collect()
}

/// Перенос имени шаблона в подписи сектора: имя длиннее `max_chars`
/// символов делится на две строки по самому позднему подходящему разделителю
/// (пробел или дефис) так, чтобы обе части влезали; не получилось — одна
/// строка (хвост клипается рендером по ширине подписи).
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

/// Геометрия wheel (рестайл FR-022, donut-сектора): кольца фиксированной
/// толщины [`WHEEL_RING_THICKNESS`] с межкольцевым зазором
/// [`WHEEL_RING_GAP`]; вместимость каждого кольца выводится из минимальной
/// дуги сектора ([`ring_capacity`]), переполнение уходит во внешние
/// под-кольца ([`wheel_ring_plan`]). Шаблоны — кольца от дырки наружу,
/// категории — внешние кольца. Угловые зазоры между секторами —
/// [`WHEEL_SECTOR_GAP`], они же формируют визуальные промежутки рендера.
/// `screen` — точка клика; `window_w/h` — размер окна для клампа центра
/// (клик у края не должен обрезать меню).
pub fn wheel_geometry(
    screen: Vec2,
    window_w: f32,
    window_h: f32,
    category_count: usize,
    template_count: usize,
) -> WheelGeometry {
    // 1. Кольца шаблонов — от дырки наружу. Индекс шаблона — сквозной по
    //    категории (кольца — только план раскладки).
    let plan = wheel_ring_plan(template_count);
    let mut sectors: Vec<WheelSector> = Vec::new();
    let mut template_index = 0_usize;
    for (k, count) in plan.iter().enumerate() {
        sectors.extend(ring_sectors(k, *count, |_| {
            let hit = WheelHit::Template(template_index);
            template_index += 1;
            hit
        }));
    }
    // 2. Кольца категорий — снаружи шаблонных (индексы по порядку реестра).
    let cat_plan = wheel_ring_plan(category_count);
    let mut category_index = 0_usize;
    for (j, count) in cat_plan.iter().enumerate() {
        let k = plan.len() + j;
        sectors.extend(ring_sectors(k, *count, |_| {
            let hit = WheelHit::Category(category_index);
            category_index += 1;
            hit
        }));
    }

    // 3. Внешний радиус, кнопка-хаб (FR-022) и кламп центра к окну.
    let extent = sectors.iter().map(|s| s.r1).fold(WHEEL_HUB_R, f32::max);
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
    WheelGeometry {
        center,
        sectors,
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
        // Ревизия FR-025 (2026-09-16): палитра ПРИМАРНО свёрнута — полоса
        // категорий по центру слева; развёрнутый док — по Ctrl+P/клику.
        // Фокус — только по Ctrl+P или клику в поиск; Esc сворачивает док;
        // клик по канвасу мимо панели снимает фокус, не закрывая док
        let mut panel = TemplatePanel::new();
        assert!(!panel.open, "ревизия FR-025: по умолчанию свёрнута");
        assert!(!panel.focused);
        // Ctrl+P: развернуть + фокус + чистый фильтр
        panel.open();
        assert!(panel.open && panel.focused);
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
        // FR-025 п.3: Esc по такому (развёрнутому, но БЕЗ фокуса) доку тоже
        // должен сворачивать — снятие open обязано не зависеть от focused
        // (ветка общей Esc-цепочки в main.rs, здесь — контракт close())
        panel.close();
        assert!(!panel.open && !panel.focused);
    }

    #[test]
    fn panel_collapse_button_and_strip_geometry() {
        let registry = registry();
        let mut panel = TemplatePanel::new();
        panel.open = true;
        let rows = panel_rows(&registry, &panel);
        let lay = panel_layout(1280.0, 800.0, &registry, &panel, &rows);
        // Кнопка сворачивания — в правой части шапки, внутри панели
        let c = lay.collapse_rect;
        assert!(c[0] >= lay.header_rect[0]);
        assert!(c[0] + c[2] <= lay.panel_rect[0] + lay.panel_rect[2] - PANEL_PADDING + 0.01);
        assert!(c[1] >= lay.header_rect[1]);
        assert!(c[1] + c[3] <= lay.header_rect[1] + lay.header_rect[3] + 0.01);
        // Ревизия FR-025: свёрнутая палитра — вертикальная полоса категорий,
        // центрированная по вертикали; строки категорий + шеврон внизу
        let categories: Vec<String> = registry
            .categories()
            .into_iter()
            .map(|c| c.to_owned())
            .collect();
        let strip = dock_strip_layout(&categories, 800.0);
        assert_eq!(strip.rect[0], PANEL_MARGIN);
        assert!(strip.rect[1] >= PANEL_TOP_MARGIN);
        let h = strip.rect[3];
        assert!(
            (strip.rect[1] - (800.0 - h) / 2.0).abs() < 0.01,
            "полоса центрирована по вертикали"
        );
        assert!(strip.rect[2] >= STRIP_MIN_W);
        assert_eq!(strip.rows.len(), categories.len());
        for (i, (rect, name)) in strip.rows.iter().enumerate() {
            assert_eq!(name, &categories[i]);
            assert!(rect[1] >= strip.rect[1]);
            assert!(rect[1] + rect[3] <= strip.rect[1] + strip.rect[3] + 0.01);
        }
        // Шеврон — ниже последней строки категории, внутри полосы
        let last = strip.rows.last().expect("строки есть");
        assert!(strip.chevron_rect[1] > last.0[1]);
        assert!(
            strip.chevron_rect[1] + strip.chevron_rect[3] <= strip.rect[1] + strip.rect[3] + 0.01
        );
        // Малое окно: полоса клампится к верхнему отступу, не уходит в минус
        let small = dock_strip_layout(&categories, 60.0);
        assert_eq!(small.rect[1], PANEL_TOP_MARGIN);
    }

    #[test]
    fn strip_layout_clamps_to_window_bottom() {
        // Окно, где центровка упирается в нижний отступ — полоса не вылезает
        let categories: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
        let h = (categories.len() + 1) as f32 * CATEGORY_ROW_H + STRIP_PAD_V * 2.0;
        let window_h = h + PANEL_TOP_MARGIN * 2.0 + 10.0;
        let strip = dock_strip_layout(&categories, window_h);
        assert!(strip.rect[1] >= PANEL_TOP_MARGIN - 0.01);
        assert!(
            strip.rect[1] + strip.rect[3] <= window_h - PANEL_TOP_MARGIN + 0.01,
            "низ полосы — не ниже нижнего отступа"
        );
    }

    #[test]
    fn flyout_layout_clamps_height_and_scroll() {
        // 40 шаблонов в окне 600 px высотой: высота клампится к окну, строки
        // не вылезают ни за прямоугольник flyout, ни за окно
        let strip_row = [12.0, 300.0, 80.0, CATEGORY_ROW_H];
        let fly = flyout_layout(strip_row, 40, 1280.0, 600.0, 0);
        let max_h = 600.0 - PANEL_TOP_MARGIN * 2.0;
        assert!(fly.rect[3] <= max_h + 0.01, "высота клампится к окну");
        assert!(fly.rect[1] >= PANEL_TOP_MARGIN - 0.01);
        assert!(fly.rect[1] + fly.rect[3] <= 600.0 - PANEL_TOP_MARGIN + 0.01);
        // Справа от строки полосы
        assert!(fly.rect[0] >= strip_row[0] + strip_row[2] + FLYOUT_GAP - 0.01);
        let visible = ((fly.rect[3] - FLYOUT_PAD_V * 2.0) / ROW_HEIGHT).floor() as usize;
        assert_eq!(fly.row_rects.len(), visible);
        assert_eq!(fly.max_scroll, 40 - visible);
        for rect in &fly.row_rects {
            assert!(rect[1] >= fly.rect[1] - 0.01);
            assert!(rect[1] + rect[3] <= fly.rect[1] + fly.rect[3] + 0.01);
            assert!(rect[1] + rect[3] <= 600.0 - PANEL_TOP_MARGIN + 0.01);
            assert!(rect[0] >= fly.rect[0] - 0.01);
            assert!(rect[0] + rect[2] <= fly.rect[0] + fly.rect[2] + 0.01);
        }
        // Скролл клампится к max_scroll
        let scrolled = flyout_layout(strip_row, 40, 1280.0, 600.0, 100);
        assert_eq!(scrolled.scroll_top, fly.max_scroll);
        assert!(!scrolled.row_rects.is_empty());
        assert!((scrolled.row_rects[0][1] - (scrolled.rect[1] + FLYOUT_PAD_V)).abs() < 0.01);
    }

    #[test]
    fn flyout_layout_few_items_centers_on_row() {
        // Мало items — высота по содержимому, вертикальный центр flyout —
        // центр строки-категории; скролла нет
        let strip_row = [12.0, 200.0, 80.0, CATEGORY_ROW_H];
        let fly = flyout_layout(strip_row, 3, 1280.0, 800.0, 0);
        let expected_h = 3.0 * ROW_HEIGHT + FLYOUT_PAD_V * 2.0;
        assert!((fly.rect[3] - expected_h).abs() < 0.01);
        let row_cy = strip_row[1] + strip_row[3] / 2.0;
        assert!(
            (fly.rect[1] + fly.rect[3] / 2.0 - row_cy).abs() < 0.01,
            "flyout центрирован относительно строки категории"
        );
        assert_eq!(fly.max_scroll, 0);
        assert_eq!(fly.row_rects.len(), 3);
    }

    #[test]
    fn strip_hover_opens_after_intent_and_closes_with_grace() {
        // Hover-intent: открытие только после 150 мс наведения; grace 300 мс
        // держит flyout после ухода курсора. true — смена строки/раскрытия.
        let t0 = Instant::now();
        let mut hover = StripHover::new();
        assert!(
            hover.update_at(Some(0), false, t0),
            "смена строки под курсором"
        );
        assert_eq!(hover.open, None);
        assert!(!hover.update_at(Some(0), false, t0 + Duration::from_millis(149)));
        assert_eq!(hover.open, None, "до intent-задержки не открывается");
        assert!(hover.update_at(Some(0), false, t0 + Duration::from_millis(150)));
        assert_eq!(hover.open, Some(0));
        // Уход курсора: в пределах grace — ещё открыто (строка сменилась —
        // перерисовка нужна)
        assert!(hover.update_at(None, false, t0 + Duration::from_millis(200)));
        assert_eq!(hover.open, Some(0));
        assert!(hover.update_at(None, false, t0 + Duration::from_millis(200 + 300)));
        assert_eq!(hover.open, None, "после grace-задержки закрывается");
        assert!(!hover.pending());
    }

    #[test]
    fn strip_hover_switches_category_and_resets_scroll() {
        let t0 = Instant::now();
        let mut hover = StripHover::new();
        hover.update_at(Some(0), false, t0);
        hover.update_at(Some(0), false, t0 + Duration::from_millis(150));
        assert_eq!(hover.open, Some(0));
        hover.scroll_by(3, 10);
        assert_eq!(hover.scroll_top, 3);
        // Переход на другую категорию — тоже с intent-задержкой, скролл
        // новой категории сброшен
        hover.update_at(Some(1), false, t0 + Duration::from_millis(200));
        assert!(
            hover.update_at(Some(1), false, t0 + Duration::from_millis(200 + 150)),
            "переключение категории меняет состояние"
        );
        assert_eq!(hover.open, Some(1));
        assert_eq!(hover.scroll_top, 0);
    }

    #[test]
    fn strip_hover_pin_survives_cursor_leave() {
        // Пин по клику: открыто без задержки и держится после ухода курсора;
        // повторный клик — закрыть (строка под курсором не менялась — false)
        let t0 = Instant::now();
        let mut hover = StripHover::new();
        hover.toggle_trigger(2);
        assert_eq!(hover.open, Some(2));
        assert!(hover.pinned);
        assert!(!hover.update_at(None, false, t0 + Duration::from_secs(5)));
        assert_eq!(hover.open, Some(2), "pinned не закрывается по grace");
        hover.toggle_trigger(2);
        assert_eq!(hover.open, None);
        assert!(!hover.pinned);
    }

    #[test]
    fn strip_hover_flyout_holds_open_without_pin() {
        // Курсор со строки ушёл, но внутри flyout — открытие держится
        // (grace не тикает); смена строки — повод для перерисовки (true)
        let t0 = Instant::now();
        let mut hover = StripHover::new();
        hover.update_at(Some(0), false, t0);
        hover.update_at(Some(0), false, t0 + Duration::from_millis(150));
        assert!(hover.update_at(None, true, t0 + Duration::from_secs(10)));
        assert_eq!(hover.open, Some(0));
        assert!(!hover.pending(), "внутри flyout grace не копится");
    }

    #[test]
    fn strip_hover_scroll_clamps() {
        let mut hover = StripHover::new();
        hover.scroll_by(7, 5);
        assert_eq!(hover.scroll_top, 5, "верхняя граница max_scroll");
        hover.scroll_by(-99, 5);
        assert_eq!(hover.scroll_top, 0, "нижняя граница 0");
        hover.scroll_by(2, 0);
        assert_eq!(hover.scroll_top, 0, "max_scroll=0 — скролла нет");
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

    // --- Wheel: геометрия donut-секторов (рестайл FR-022, 2026-09-16) ---

    /// Проба hit-test в mid-углу/радиусе сектора (экранные px).
    fn sector_probe(geo: &WheelGeometry, sector: &WheelSector) -> Vec2 {
        sector_point(geo.center, sector.mid_angle(), sector.mid_radius())
    }

    #[test]
    fn wheel_ring_plan_splits_balanced() {
        // План — как раньше (балансированное дробление), но вместимость колец
        // выводится из минимальной дуги сектора (ring_capacity)
        assert_eq!(wheel_ring_plan(0), Vec::<usize>::new());
        assert_eq!(wheel_ring_plan(1), vec![1]);
        // 6 > вместимости кольца 0 (=5) — балансированное дробление [3, 3]
        assert_eq!(wheel_ring_plan(6), vec![3, 3]);
        assert_eq!(wheel_ring_plan(7), vec![3, 4]);
        assert_eq!(wheel_ring_plan(10), vec![5, 5]);
        assert_eq!(wheel_ring_plan(11), vec![5, 6]);
        assert_eq!(wheel_ring_plan(13), vec![4, 4, 5]);
        // 15 шаблонов при thickness 68 — минимум 2 кольца (кольцо 0 вмещает 5)
        let plan = wheel_ring_plan(15);
        assert!(plan.len() >= 2, "15 шаблонов не влезают в одно кольцо");
        assert_eq!(plan, vec![5, 5, 5]);
        // Каждая часть — не больше вместимости своего кольца
        for (k, count) in plan.iter().enumerate() {
            assert!(
                *count <= ring_capacity(k),
                "кольцо {k}: {count} > вместимости {}",
                ring_capacity(k)
            );
        }
    }

    #[test]
    fn wheel_ring_capacity_grows_outward() {
        // Вместимость кольца 0 при HUB_R=24/thickness=68 — 5 (окружность
        // 364 px / дугу 62.5 px), дальше не убывает; потолок — WHEEL_RING_CAP
        assert_eq!(ring_capacity(0), 5);
        assert!(ring_capacity(1) >= ring_capacity(0));
        assert!(ring_capacity(2) >= ring_capacity(1));
        for k in 0..6 {
            assert!(ring_capacity(k) >= 2, "кольцо {k} вмещает минимум 2");
            assert!(ring_capacity(k) <= WHEEL_RING_CAP);
        }
    }

    #[test]
    fn wheel_sector_ring_radii_and_angular_gaps() {
        // Радиусы колец: r0 = HUB_R + k·(thickness+gap), r1 = r0 + thickness
        let geo = wheel_geometry([400.0, 300.0], 1280.0, 800.0, 4, 10);
        assert_eq!(geo.sectors.len(), 14);
        let (r0_0, r1_0) = ring_radii(0);
        let (r0_1, r1_1) = ring_radii(1);
        let (r0_2, r1_2) = ring_radii(2);
        for s in &geo.sectors {
            let (er0, er1) = match s.hit {
                WheelHit::Template(_) => {
                    // 10 шаблонов → план [5, 5]: кольца 0 и 1
                    if s.r0 < r0_1 - 0.01 {
                        (r0_0, r1_0)
                    } else {
                        (r0_1, r1_1)
                    }
                }
                WheelHit::Category(_) => (r0_2, r1_2),
            };
            assert!((s.r0 - er0).abs() < 0.01, "r0 кольца: {} vs {er0}", s.r0);
            assert!((s.r1 - er1).abs() < 0.01, "r1 кольца: {} vs {er1}", s.r1);
        }
        // Угловые зазоры: между концом сектора и началом следующего в кольце
        // — ровно WHEEL_SECTOR_GAP (последний → первый через wrap 0/2π)
        let mut by_ring: Vec<Vec<&WheelSector>> = Vec::new();
        for s in &geo.sectors {
            let ring =
                ((s.r0 - WHEEL_HUB_R) / (WHEEL_RING_THICKNESS + WHEEL_RING_GAP)).round() as usize;
            while by_ring.len() <= ring {
                by_ring.push(Vec::new());
            }
            by_ring[ring].push(s);
        }
        for (ring, ring_secs) in by_ring.iter().enumerate() {
            let n = ring_secs.len();
            assert!(n >= 1 && n <= ring_capacity(ring));
            let mut ordered = ring_secs.clone();
            ordered.sort_by(|a, b| a.a0.partial_cmp(&b.a0).expect("углы"));
            for i in 0..n {
                let cur = ordered[i];
                let next = ordered[(i + 1) % n];
                // Развёрнутый угол следующего начала (wrap-around через 0/2π)
                let next_a0 = if next.a0 <= cur.a0 {
                    next.a0 + std::f32::consts::TAU
                } else {
                    next.a0
                };
                let gap = next_a0 - cur.a1;
                assert!(
                    (gap - WHEEL_SECTOR_GAP).abs() < 1e-3,
                    "кольцо {ring}: зазор {gap} между соседями"
                );
            }
        }
    }

    #[test]
    fn wheel_hub_button_and_center_hole() {
        // Хаб — квадрат WHEEL_HUB_D вокруг центра, рендер и клик видят один
        // и тот же прямоугольник; дырка donut (r < WHEEL_HUB_R) — не сектор
        let geo = wheel_geometry([400.0, 300.0], 1280.0, 800.0, 4, 10);
        assert!((geo.hub[2] - WHEEL_HUB_D).abs() < 0.01);
        assert!((geo.hub[0] + WHEEL_HUB_D / 2.0 - geo.center[0]).abs() < 0.01);
        assert!((geo.hub[1] + WHEEL_HUB_D / 2.0 - geo.center[1]).abs() < 0.01);
        assert!(geo.hub_hit(geo.center));
        assert!(!geo.hub_hit([geo.center[0], geo.center[1] - geo.extent]));
        // Клик в центр — дырка, не сектор (хаб обрабатывается отдельно)
        assert_eq!(geo.hit(geo.center), None);
        // Ни один сектор не заходит в дырку
        for s in &geo.sectors {
            assert!(s.r0 >= WHEEL_HUB_R, "сектор залез в дырку: r0={}", s.r0);
        }
    }

    #[test]
    fn wheel_hit_polar_mid_angles() {
        // Каждый сектор отвечает по точке в mid-углу на среднем радиусе;
        // в угловом зазоре, за r1 и в дырке — None
        let registry = registry(); // mock: 4 категории
        let geo = wheel_geometry(
            [400.0, 300.0],
            1280.0,
            800.0,
            registry.categories().len(),
            0,
        );
        assert_eq!(geo.sectors.len(), 4);
        for s in &geo.sectors {
            let probe = sector_probe(&geo, s);
            assert_eq!(geo.hit(probe), Some(s.hit.clone()));
        }
        // Угловой зазор между первым и вторым сектором кольца категорий
        let mut ordered: Vec<&WheelSector> = geo.sectors.iter().collect();
        ordered.sort_by(|a, b| a.a0.partial_cmp(&b.a0).expect("углы"));
        let gap_angle = (ordered[0].a1 + ordered[1].a0) / 2.0;
        let r_mid = ordered[0].mid_radius();
        let gap_point = sector_point(geo.center, gap_angle, r_mid);
        assert_eq!(geo.hit(gap_point), None, "угловой зазор — мимо секторов");
        // За внешним радиусом и в дырке — None
        let far = sector_point(geo.center, ordered[0].mid_angle(), geo.extent + 20.0);
        assert_eq!(geo.hit(far), None);
        assert_eq!(geo.hit(geo.center), None);
        // Без выбранной категории шаблонных секторов нет
        assert!(geo
            .sectors
            .iter()
            .all(|s| !matches!(s.hit, WheelHit::Template(_))));
    }

    #[test]
    fn wheel_hit_templates_flat_index() {
        let registry = registry();
        let geo = wheel_geometry(
            [400.0, 300.0],
            1280.0,
            800.0,
            registry.categories().len(),
            registry.by_category("backend").len(),
        );
        // Каждый шаблонный сектор отвечает своим плоским индексом
        for s in &geo.sectors {
            let probe = sector_probe(&geo, s);
            assert_eq!(geo.hit(probe), Some(s.hit.clone()));
        }
        assert!(geo.sectors.iter().any(|s| s.hit == WheelHit::Template(0)));
        assert!(geo.sectors.iter().any(|s| s.hit == WheelHit::Template(1)));
        // Категории остаются кликабельными при выбранной категории
        assert!(geo
            .sectors
            .iter()
            .any(|s| matches!(s.hit, WheelHit::Category(_))));
    }

    #[test]
    fn wheel_geometry_clamps_to_window() {
        // Клик у правого края: центр сдвигается, wheel целиком в окне
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
    fn wheel_geometry_15_templates_multiple_rings() {
        // Переполнение кольца: 15 шаблонов уходят в под-кольца, сквозные
        // индексы 0..15 все представлены
        let geo = wheel_geometry([400.0, 300.0], 1600.0, 1200.0, 4, 15);
        let templates: Vec<usize> = geo
            .sectors
            .iter()
            .filter_map(|s| match s.hit {
                WheelHit::Template(i) => Some(i),
                _ => None,
            })
            .collect();
        assert_eq!(templates.len(), 15);
        let mut sorted = templates.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..15).collect::<Vec<_>>());
        // Кольца шаблонов — минимум два (вместимость кольца 0 — 5)
        let mut template_r0: Vec<f32> = geo
            .sectors
            .iter()
            .filter_map(|s| matches!(s.hit, WheelHit::Template(_)).then_some(s.r0))
            .collect();
        template_r0.sort_by(|a, b| a.partial_cmp(b).expect("радиусы"));
        template_r0.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        assert!(template_r0.len() >= 2, "15 шаблонов — минимум 2 кольца");
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
        // Сценарий: клик по сектору категории выбирает категорию
        // (появляются шаблонные кольца), клик по сектору шаблона — цель
        // для инстанциации
        let registry = registry();
        let categories = registry.categories();
        let cache_index = categories
            .iter()
            .position(|c| *c == "cache")
            .expect("cache");
        let geo = wheel_geometry([500.0, 400.0], 1280.0, 800.0, categories.len(), 0);
        let cat = geo
            .sectors
            .iter()
            .find(|s| s.hit == WheelHit::Category(cache_index))
            .expect("сектор cache");
        assert_eq!(
            geo.hit(sector_probe(&geo, cat)),
            Some(WheelHit::Category(cache_index))
        );
        // После выбора категории: 1 шаблон cache на кольце у дырки
        let geo2 = wheel_geometry(
            [500.0, 400.0],
            1280.0,
            800.0,
            categories.len(),
            registry.by_category("cache").len(),
        );
        let tpl = geo2
            .sectors
            .iter()
            .find(|s| s.hit == WheelHit::Template(0))
            .expect("сектор шаблона");
        assert!(
            (tpl.r0 - WHEEL_HUB_R).abs() < 0.01,
            "шаблоны — кольцо у дырки"
        );
        assert_eq!(
            geo2.hit(sector_probe(&geo2, tpl)),
            Some(WheelHit::Template(0))
        );
    }
}
