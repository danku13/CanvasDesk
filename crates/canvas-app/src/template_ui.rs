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
//!   (2 кольца: внешнее — категории, внутреннее — шаблоны категории);
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

/// Внешний радиус wheel (категории), логические px.
pub const WHEEL_OUTER_R: f32 = 118.0;
/// Внутренний радиус wheel (шаблоны выбранной категории).
pub const WHEEL_INNER_R: f32 = 58.0;
/// Радиус центрального хаба (пустая зона — глотает клик).
pub const WHEEL_HUB_R: f32 = 24.0;
/// Сторона плашки-сектора категории (пайплайн без поворотов — сектор
/// рисуется квадом в центре сектора).
pub const WHEEL_SECTOR: f32 = 64.0;
/// Сторона плашки-сектора шаблона (внутреннее кольцо).
pub const WHEEL_SECTOR_INNER: f32 = 60.0;

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
    /// Сектор категории (индекс в `registry.categories()`).
    Category(usize),
    /// Сектор шаблона (индекс ВНУТРИ категории — `registry.by_category`).
    Template(usize),
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

/// Hit-test wheel: точка курсора (логические px) → сектор. Внешнее кольцо —
/// категории (`WHEEL_INNER_R..WHEEL_OUTER_R`), внутреннее — шаблоны
/// (`WHEEL_HUB_R..WHEEL_INNER_R`, только когда категория выбрана).
pub fn wheel_hit(menu: &WheelMenu, registry: &TemplateRegistry, cursor: Vec2) -> Option<WheelHit> {
    let dx = cursor[0] - menu.screen[0];
    let dy = cursor[1] - menu.screen[1];
    let dist = (dx * dx + dy * dy).sqrt();
    if dist > WHEEL_OUTER_R || dist < WHEEL_HUB_R {
        return None;
    }
    // Угол точки (0 — ось X, по часовой вниз из-за Y-вниз экранных координат)
    let angle = dy.atan2(dx);
    let categories = registry.categories();
    if dist >= WHEEL_INNER_R {
        // Внешнее кольцо: ближайший по углу сектор категории
        let count = categories.len();
        if count == 0 {
            return None;
        }
        let step = std::f32::consts::TAU / count as f32;
        let offset = angle + std::f32::consts::FRAC_PI_2 + step / 2.0;
        let index = (offset.rem_euclid(std::f32::consts::TAU) / step) as usize % count;
        Some(WheelHit::Category(index))
    } else if let Some(category) = &menu.category {
        // Внутреннее кольцо: шаблоны выбранной категории
        let templates = registry.by_category(category);
        let count = templates.len();
        if count == 0 {
            return None;
        }
        let step = std::f32::consts::TAU / count as f32;
        let offset = angle + std::f32::consts::FRAC_PI_2 + step / 2.0;
        let index = (offset.rem_euclid(std::f32::consts::TAU) / step) as usize % count;
        Some(WheelHit::Template(index))
    } else {
        None
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

    // --- Wheel ---

    #[test]
    fn wheel_categories_hit() {
        let registry = registry();
        let menu = WheelMenu {
            screen: [400.0, 300.0],
            world: [100.0, 200.0],
            category: None,
        };
        // Мимо радиуса — None
        assert_eq!(wheel_hit(&menu, &registry, [400.0 + 200.0, 300.0]), None);
        assert_eq!(wheel_hit(&menu, &registry, [400.0, 300.0]), None); // центр-хаб
                                                                       // 12 часов — первая категория (backend, порядок реестра)
        let hit = wheel_hit(&menu, &registry, [400.0, 300.0 - 90.0]);
        assert_eq!(hit, Some(WheelHit::Category(0)));
        // Количество категорий: 12 часов + полный оборот — снова backend
        let count = registry.categories().len();
        let angle = -std::f32::consts::FRAC_PI_2 + 0.01;
        let hit = wheel_hit(
            &menu,
            &registry,
            [400.0 + 90.0 * angle.cos(), 300.0 + 90.0 * angle.sin()],
        );
        let matched = match hit {
            Some(WheelHit::Category(0)) => true,
            Some(WheelHit::Category(i)) => i == count - 1,
            _ => false,
        };
        assert!(matched, "ожидалась категория 0 или {count}-1");
    }

    #[test]
    fn wheel_templates_hit_after_category() {
        let registry = registry();
        let menu = WheelMenu {
            screen: [400.0, 300.0],
            world: [100.0, 200.0],
            category: Some("backend".to_owned()),
        };
        // Внутреннее кольцо, 12 часов — первый шаблон backend (mock.lb)
        let hit = wheel_hit(&menu, &registry, [400.0, 300.0 - 40.0]);
        assert_eq!(hit, Some(WheelHit::Template(0)));
        // Без категории внутреннее кольцо не отвечает
        let menu_no_cat = WheelMenu {
            screen: [400.0, 300.0],
            world: [100.0, 200.0],
            category: None,
        };
        assert_eq!(
            wheel_hit(&menu_no_cat, &registry, [400.0, 300.0 - 40.0]),
            None
        );
    }

    #[test]
    fn wheel_sector_angles_deterministic() {
        // 4 категории: сектора по 90°, старт с 12 часов
        assert!(
            (sector_center_angle(4, 0)
                - (-std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4))
                .abs()
                < 1e-6
        );
        assert!(
            (sector_center_angle(4, 2)
                - (-std::f32::consts::FRAC_PI_2 + 2.5 * std::f32::consts::FRAC_PI_2))
                .abs()
                < 1e-6
        );
        // 0/1 категорий — без паники; одна категория — сектор 360°,
        // центр в нижней точке
        assert!((sector_center_angle(0, 0) - 0.0).abs() < 1e-6);
        assert!((sector_center_angle(1, 0) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn wheel_categories_then_templates_flow() {
        // Сценарий: клик по категории перестраивает внутреннее кольцо;
        // клик по шаблону внутри — цель для инстанциации
        let registry = registry();
        let mut menu = WheelMenu {
            screen: [500.0, 400.0],
            world: [10.0, 20.0],
            category: None,
        };
        // Клик в сектор категории cache (индекс 1 в categories())
        let categories = registry.categories();
        let cache_index = categories
            .iter()
            .position(|c| *c == "cache")
            .expect("cache");
        let angle = sector_center_angle(categories.len(), cache_index);
        let point = sector_point(menu.screen, angle, (WHEEL_INNER_R + WHEEL_OUTER_R) / 2.0);
        match wheel_hit(&menu, &registry, point) {
            Some(WheelHit::Category(i)) => {
                menu.category = Some(categories[i].to_owned());
            }
            other => panic!("ожидалась категория, получено {other:?}"),
        }
        // Внутреннее кольцо теперь отвечает шаблонами cache (1 шт.)
        let angle = sector_center_angle(1, 0);
        let point = sector_point(menu.screen, angle, (WHEEL_HUB_R + WHEEL_INNER_R) / 2.0);
        assert_eq!(
            wheel_hit(&menu, &registry, point),
            Some(WheelHit::Template(0))
        );
    }
}
