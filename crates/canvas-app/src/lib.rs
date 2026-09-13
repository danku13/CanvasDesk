//! canvas-app library — общая логика приложения.
//!
//! Переэкспортирует типы и функции, нужные интеграционным тестам без окна
//! winit, и содержит модуль `ui` — чистые функции интерфейса (геометрия
//! оверлеев, hit-тесты, палитра контекстного меню, детектор двойного
//! клика), общий для бинаря (`main.rs`) и тестов. Раньше эти функции
//! дублировались в `main.rs` копипастой — теперь источник один.

pub use canvas_core::{
    edge_at, focus_set, nearest_side, next_port_zone, port_at, port_point, Canvas, Corner, Edge,
    EdgeLineStyle, EdgeThickness, FocusSeed, FocusSet, Node, NodeKind, Settings, Side,
    SpatialIndex,
};
pub use canvas_render::camera::Vec2;
pub use canvas_render::cards::{preset_color, CardInstance, HEADER_HEIGHT};
pub use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand, Marker,
    EDGE_EDIT_HEIGHT, EDGE_EDIT_WIDTH,
};
pub use canvas_render::text::{
    body_area, OverlayText, ScreenText, BODY_FONT_SIZE, BODY_LINE_HEIGHT, BODY_PADDING,
    BODY_TOP_GAP,
};
pub use canvas_render::{
    Camera, Color, FrameMeter, FrameOverlay, FrameStats, SceneView, Selection,
};
pub use winit::keyboard::{Key, ModifiersState, NamedKey};

/// Чистая UI-логика приложения: геометрия оверлеев (контекстное меню,
/// панель настроек), hit-тесты, генератор id заметок, детектор двойного
/// клика. Не зависит от окна и GPU — используется бинарём и тестами.
pub mod ui {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// Ширина контекстного меню в world-px (T7).
    pub const MENU_WIDTH: f32 = 170.0;
    /// Высота пункта меню в world-px.
    pub const MENU_ITEM_HEIGHT: f32 = 26.0;
    /// Внутренний отступ меню в world-px.
    pub const MENU_PADDING: f32 = 6.0;
    /// Сдвиг подписи пункта: слева место под образец цвета.
    pub const MENU_LABEL_X: f32 = 26.0;
    /// Пункты палитры (T7): пресеты "1".."6" + None — сброс цвета.
    pub const MENU_ITEMS: [Option<&str>; 7] = [
        Some("1"),
        Some("2"),
        Some("3"),
        Some("4"),
        Some("5"),
        Some("6"),
        None,
    ];
    /// Фон меню — тёмный, почти непрозрачный.
    pub const MENU_FILL: [f32; 4] = [0.11, 0.11, 0.13, 0.97];

    /// Минимальные размеры ноды (ручной resize, T7).
    pub const MIN_NODE_WIDTH: f32 = 160.0;
    pub const MIN_NODE_HEIGHT: f32 = 64.0;
    /// Потолок автороста ширины заметки под контент (T7).
    pub const MAX_NOTE_WIDTH: f32 = 600.0;
    /// Зона захвата в правом нижнем углу ноды для ручного resize (world-px, T7).
    pub const RESIZE_HANDLE: f32 = 16.0;

    // --- Drag-drop из Explorer (T9, план docs/plans/T9-drag-drop.md) ---

    /// Ширина карточки дропа в world-px (как seed-карточки файлов).
    pub const DROP_CARD_W: f32 = 320.0;
    /// Высота карточки дропа в world-px.
    pub const DROP_CARD_H: f32 = 220.0;
    /// Зазор сетки дропа (шаг = карточка + зазор, критерий T9).
    pub const DROP_GRID_GAP: f32 = 24.0;
    /// Колонок в ряду сетки дропа (перенос строки после 5 карточек).
    pub const DROP_GRID_COLS: usize = 5;
    /// Призраков на превью зоны дропа не больше (дёшево рисовать, план §5).
    pub const DROP_PREVIEW_MAX: usize = 50;

    /// Сторона летающей кнопки настроек (логические px).
    pub const SETTINGS_BUTTON: f32 = 36.0;
    /// Отступ кнопки и панели настроек от краёв окна (логические px).
    pub const SETTINGS_MARGIN: f32 = 12.0;
    /// Зазор между кнопкой и панелью настроек.
    pub const SETTINGS_GAP: f32 = 8.0;
    /// Ширина панели настроек.
    pub const PANEL_WIDTH: f32 = 300.0;
    /// Высота строки настройки.
    pub const PANEL_ROW_HEIGHT: f32 = 28.0;
    /// Высота заголовка панели.
    pub const PANEL_HEADER_HEIGHT: f32 = 30.0;
    /// Высота строки-подсказки внизу панели.
    pub const PANEL_HINT_HEIGHT: f32 = 24.0;
    /// Внутренний отступ панели.
    pub const PANEL_PADDING: f32 = 10.0;

    /// Строки панели настроек (порядок = порядок отображения). Тема вынесена
    /// в отдельную кнопку-переключатель рядом с кнопкой настроек.
    pub const SETTINGS_ROWS: [SettingsRow; 8] = [
        SettingsRow::ButtonCorner,
        SettingsRow::Grid,
        SettingsRow::GridStyle,
        SettingsRow::GridDensity,
        SettingsRow::EdgesAvoid,
        SettingsRow::PortZone,
        SettingsRow::FocusMode,
        SettingsRow::HudOnStart,
    ];

    /// Строка-переключатель панели настроек.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SettingsRow {
        /// Угол летающей кнопки (цикл по 4 углам).
        ButtonCorner,
        /// Сетка канваса вкл/выкл.
        Grid,
        /// Вид сетки: линии или точки.
        GridStyle,
        /// Плотность сетки (цикл по 3 вариантам).
        GridDensity,
        /// Связи огибают посторонние ноды.
        EdgesAvoid,
        /// Зона захвата портов для drag связи (CR-003): цикл по пресетам.
        PortZone,
        /// Режим фокуса связей (T23, brainstorm-focus) вкл/выкл.
        FocusMode,
        /// HUD (F3) включён при старте.
        HudOnStart,
    }

    impl SettingsRow {
        /// Подпись строки с текущим значением.
        pub fn label(self, settings: &Settings) -> String {
            let on_off = |v: bool| if v { "вкл" } else { "выкл" };
            match self {
                SettingsRow::ButtonCorner => {
                    format!("Угол кнопки: {}", settings.button_corner.label())
                }
                SettingsRow::Grid => format!("Сетка: {}", on_off(settings.grid_visible)),
                SettingsRow::GridStyle => format!("Вид сетки: {}", settings.grid_style.label()),
                SettingsRow::GridDensity => {
                    format!("Плотность сетки: {}", settings.grid_density.label())
                }
                SettingsRow::EdgesAvoid => {
                    format!("Связи огибают ноды: {}", on_off(settings.edges_avoid_nodes))
                }
                SettingsRow::PortZone => {
                    format!("Зона портов: {} px", settings.port_zone_px as i32)
                }
                SettingsRow::FocusMode => {
                    format!("Фокус на связях: {}", on_off(settings.focus_mode))
                }
                SettingsRow::HudOnStart => {
                    format!("HUD при запуске: {}", on_off(settings.hud_on_start))
                }
            }
        }
    }

    /// Точка в rect [x, y, w, h]? (логические px, границы включительны)
    pub fn point_in_rect(rect: [f32; 4], point: Vec2) -> bool {
        point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
    }

    /// Эффективный scale factor для экранного ввода/раскладки (R10):
    /// в desktop-режиме после репарентинга `window.scale_factor()` врёт
    /// (winit не получает корректный DPI на детях Progman/WorkerW) — берём
    /// DPI из GetDpiForWindow (поллинг монитора T15-D + снятие сразу после
    /// attach); в оконном режиме — scale_factor окна.
    pub fn effective_scale(window_scale: f32, desktop_dpi: Option<u32>) -> f32 {
        (match desktop_dpi {
            Some(dpi) => dpi as f64 / 96.0,
            None => f64::from(window_scale),
        }) as f32
    }

    /// Rect летающей кнопки настроек в логических px от угла окна.
    pub fn button_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let x = match corner {
            Corner::TopLeft | Corner::BottomLeft => SETTINGS_MARGIN,
            _ => viewport[0] - SETTINGS_MARGIN - SETTINGS_BUTTON,
        };
        let y = match corner {
            Corner::TopLeft | Corner::TopRight => SETTINGS_MARGIN,
            _ => viewport[1] - SETTINGS_MARGIN - SETTINGS_BUTTON,
        };
        [x, y, SETTINGS_BUTTON, SETTINGS_BUTTON]
    }

    /// Rect кнопки переключения темы: тот же угол, вплотную к кнопке
    /// настроек (внутрь экрана по горизонтали через SETTINGS_GAP).
    pub fn theme_button_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let button = button_rect(corner, viewport);
        let x = match corner {
            Corner::TopLeft | Corner::BottomLeft => button[0] + SETTINGS_BUTTON + SETTINGS_GAP,
            _ => button[0] - SETTINGS_GAP - SETTINGS_BUTTON,
        };
        [x, button[1], SETTINGS_BUTTON, SETTINGS_BUTTON]
    }

    /// Высота панели настроек: паддинги + заголовок + строки + подсказка.
    pub fn panel_height() -> f32 {
        PANEL_PADDING * 2.0
            + PANEL_HEADER_HEIGHT
            + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT
            + PANEL_HINT_HEIGHT
    }

    /// Rect панели настроек: прижата к кнопке (с зазором), в том же углу.
    pub fn panel_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let height = panel_height();
        let x = match corner {
            Corner::TopLeft | Corner::BottomLeft => SETTINGS_MARGIN,
            _ => viewport[0] - SETTINGS_MARGIN - PANEL_WIDTH,
        };
        let y = match corner {
            Corner::TopLeft | Corner::TopRight => SETTINGS_MARGIN + SETTINGS_BUTTON + SETTINGS_GAP,
            _ => viewport[1] - SETTINGS_MARGIN - SETTINGS_BUTTON - SETTINGS_GAP - height,
        };
        [x, y, PANEL_WIDTH, height]
    }

    /// Hit-test строки панели: индекс в SETTINGS_ROWS или None
    /// (заголовок/подсказка/паддинги не кликабельны).
    pub fn panel_row_at(panel: [f32; 4], point: Vec2) -> Option<usize> {
        let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
        if point[0] < panel[0]
            || point[0] > panel[0] + panel[2]
            || point[1] < rows_top
            || point[1] > rows_top + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT
        {
            return None;
        }
        let i = ((point[1] - rows_top) / PANEL_ROW_HEIGHT) as usize;
        (i < SETTINGS_ROWS.len()).then_some(i)
    }

    /// Точка в зоне resize (правый нижний угол ноды)? Чистая функция для тестов.
    pub fn in_resize_corner(node: &Node, point: Vec2) -> bool {
        let right = node.x + node.width;
        let bottom = node.y + node.height;
        point[0] >= right - RESIZE_HANDLE
            && point[0] <= right
            && point[1] >= bottom - RESIZE_HANDLE
            && point[1] <= bottom
    }

    /// Контекстное меню ноды (T7): палитра цветов в world-точке клика ПКМ.
    /// Цель — нода (палитра) или связь (стиль/толщина/цвет линии).
    pub struct ContextMenu {
        pub target: MenuTarget,
        pub origin: Vec2,
    }

    /// Цель контекстного меню (ПКМ по канвасу).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MenuTarget {
        /// Нода: палитра цветов + действия (индекс в canvas.nodes).
        Node(usize),
        /// Связь: стиль линии, толщина, цвет (индекс в canvas.edges).
        Edge(usize),
        /// Пустое место: действия канваса (создание группы).
        Canvas,
    }

    /// Активный drag резиновой линии (T8 + CR-002): от порта ноды к курсору —
    /// новая связь; или перепривязка конца существующей связи.
    pub enum EdgeDrag {
        /// Новая связь: тянем от порта `from_node` (T8).
        New { from_node: String, from_side: Side },
        /// Перепривязка конца существующей связи (CR-002): тянем хэндл
        /// конца `end` связи `edge_index` к новой ноде — без удаления.
        Rebind {
            edge_index: usize,
            end: canvas_core::EdgeEnd,
        },
    }

    impl EdgeDrag {
        /// Исток резиновой линии в world-координатах и его сторона:
        /// для новой связи — порт ноды-истока; для перепривязки —
        /// НЕПОДВИЖНЫЙ (противоположный) конец связи. None — данных нет
        /// (нода удалена/связь висячая) — линию не рисуем.
        pub fn draft_origin(&self, canvas: &Canvas) -> Option<([f32; 2], Side)> {
            match self {
                EdgeDrag::New {
                    from_node,
                    from_side,
                } => {
                    let node = canvas.node(from_node)?;
                    Some((port_point(node, *from_side), *from_side))
                }
                EdgeDrag::Rebind { edge_index, end } => {
                    let opposite = match end {
                        canvas_core::EdgeEnd::From => canvas_core::EdgeEnd::To,
                        canvas_core::EdgeEnd::To => canvas_core::EdgeEnd::From,
                    };
                    let (side, point) = canvas_core::edge_endpoint(canvas, *edge_index, opposite)?;
                    Some((point, side))
                }
            }
        }
    }

    /// Максимальный интервал между кликами двойного клика (winit его не даёт, T7).
    const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
    /// Максимальный сдвиг курсора между кликами двойного клика (логические px).
    const DOUBLE_CLICK_DIST: f64 = 5.0;

    /// Детектор двойного клика (T7): интервал и сдвиг между нажатиями ЛКМ.
    pub struct DoubleClick {
        last: Option<(Instant, Vec2)>,
    }

    impl Default for DoubleClick {
        fn default() -> Self {
            Self::new()
        }
    }

    impl DoubleClick {
        pub fn new() -> Self {
            Self { last: None }
        }

        /// Зарегистрировать нажатие; true — это второй клик пары.
        pub fn register(&mut self, at: Instant, pos: Vec2) -> bool {
            let double = self.last.is_some_and(|(time, prev)| {
                at.duration_since(time) <= DOUBLE_CLICK_INTERVAL
                    && (pos[0] as f64 - prev[0] as f64).abs() <= DOUBLE_CLICK_DIST
                    && (pos[1] as f64 - prev[1] as f64).abs() <= DOUBLE_CLICK_DIST
            });
            self.last = Some((at, pos));
            double
        }
    }

    /// Первый свободный id вида `{prefix}-N` (T9): N от 1, занятые в канвасе
    /// пропускаются. Обобщение генератора id заметок на `file-N`/`note-N`
    /// (вызовы с "note" — заметки, с "file" — ноды дропа).
    pub fn next_free_id(canvas: &Canvas, prefix: &str) -> String {
        let mut n = 1u32;
        while canvas
            .nodes
            .iter()
            .any(|node| node.id == format!("{prefix}-{n}"))
        {
            n += 1;
        }
        format!("{prefix}-{n}")
    }

    // --- Оверлей горячих клавиш (FR-004) ---

    /// Ширина панели хоткеев (логические px).
    pub const HOTKEYS_PANEL_WIDTH: f32 = 340.0;
    /// Высота строки хоткея (логические px).
    pub const HOTKEYS_ROW_HEIGHT: f32 = 22.0;
    /// Внутренний отступ панели хоткеев.
    pub const HOTKEYS_PADDING: f32 = 10.0;
    /// Высота заголовка панели хоткеев.
    pub const HOTKEYS_HEADER_HEIGHT: f32 = 30.0;
    /// Ширина колонки клавиши (выравнивание описаний).
    pub const HOTKEYS_KEY_COLUMN: f32 = 118.0;

    /// Список горячих клавиш (FR-004): (клавиша, описание) — единый
    /// источник для оверлея F1. Порядок = порядок отображения; обновлять
    /// при изменении хоткеев (ввод — main.rs on_key).
    pub const HOTKEYS: &[(&str, &str)] = &[
        ("F1", "список горячих клавиш"),
        ("Ctrl+F", "поиск по канвасу"),
        ("F3", "HUD / следующий результат"),
        ("Esc", "закрыть меню и панели"),
        ("Del", "удалить выделенное"),
        ("Ctrl+C", "копировать ноды"),
        ("Ctrl+V", "вставить ноды"),
        ("Ctrl+D", "дублировать ноды"),
        ("Ctrl+клик", "добавить к выделению"),
        ("ЛКМ + drag", "рамка выделения"),
        ("ЛКМ от порта", "протянуть связь"),
        ("ЛКМ за хэндл", "перепривязать связь"),
        ("2× клик", "заметка / открыть файл"),
        ("ПКМ", "меню объекта"),
        ("Space+drag", "панорамирование"),
        ("Ctrl+колесо", "масштаб"),
        ("Ctrl+Enter", "зафиксировать заметку"),
        ("Ctrl+,", "настройки"),
        ("F", "фокус на связях"),
    ];

    /// Полная высота панели хоткеев (FR-004): паддинги + заголовок +
    /// строки. Клампится к высоте окна в `hotkeys_panel_rect`.
    pub fn hotkeys_panel_height() -> f32 {
        HOTKEYS_PADDING * 2.0 + HOTKEYS_HEADER_HEIGHT + HOTKEYS.len() as f32 * HOTKEYS_ROW_HEIGHT
    }

    /// Rect панели хоткеев (FR-004): у ЛЕВОГО края окна, вертикально по
    /// центру (запрос пользователя: «посередине слева экрана»). Высота
    /// клампится к окну (низ не вылезает), минимум отступа сверху.
    pub fn hotkeys_panel_rect(viewport: Vec2) -> [f32; 4] {
        let max_h = (viewport[1] - SETTINGS_MARGIN * 2.0).max(0.0);
        let height = hotkeys_panel_height().min(max_h);
        let y = ((viewport[1] - height) / 2.0).max(SETTINGS_MARGIN);
        [
            SETTINGS_MARGIN,
            y,
            HOTKEYS_PANEL_WIDTH.min(viewport[0]),
            height,
        ]
    }

    // --- Множественное выделение (CR-001) ---

    /// Порог «клик vs drag» рамки выделения: логические px (CR-001).
    pub const SELECT_DRAG_THRESHOLD: f32 = 4.0;
    /// Заливка рамки выделения (CR-001): акцент, полупрозрачная (стиль
    /// призраков зоны дропа T9).
    pub const SELECT_RECT_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.08];
    /// Рамка рамки выделения (CR-001): акцент заметнее заливки.
    pub const SELECT_RECT_BORDER: [f32; 4] = [0.396, 0.612, 0.969, 0.6];

    /// Прямоугольник рамки выделения по двум углам (world, CR-001):
    /// нормализация min/max — тянуть можно в любую сторону.
    pub fn rubber_band_rect(a: Vec2, b: Vec2) -> [f32; 4] {
        let x = a[0].min(b[0]);
        let y = a[1].min(b[1]);
        [x, y, (a[0] - b[0]).abs(), (a[1] - b[1]).abs()]
    }

    /// Ноды, пересекающие прямоугольник рамки (CR-001): AABB-пересечение
    /// (частичное вхождение считается — как в Obsidian/Miro), включая
    /// группы. Порядок — индексы модели (стабильный). Доказательство
    /// простоты: полный перебор — рамка редка (одна на жест), O(V) не
    /// мешает кадру (поиск стартует на отпускании ЛКМ).
    pub fn nodes_in_rect(canvas: &Canvas, rect: [f32; 4]) -> Vec<usize> {
        let (x0, y0, x1, y1) = (rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]);
        canvas
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| {
                node.x < x1 && node.x + node.width > x0 && node.y < y1 && node.y + node.height > y0
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Тогл ноды в наборе выделения (CR-001): нет — добавить (в конец,
    /// порядок добавления = порядок кликов), есть — убрать.
    /// Возвращает true, если нода оказалась в наборе после вызова.
    pub fn toggle_selected_node(selected: &mut Vec<usize>, index: usize) -> bool {
        if let Some(pos) = selected.iter().position(|&i| i == index) {
            selected.remove(pos);
            false
        } else {
            selected.push(index);
            true
        }
    }

    /// Ctrl/Shift+клик по ноде (CR-001.3, ревью 2026-09-14): уже выделенное
    /// ОСТАЁТСЯ выделенным, клик-нутая нода тоглится. Одиночное выделение
    /// (якорь `primary`, набор пуст) сначала переносится в набор — иначе
    /// прежняя нода теряла подсветку при тогле. Клик по уже выбранной
    /// (якорем или в наборе) — снятие выделения с неё. Возвращает новый
    /// якорь: `Some(index)` — нода добавлена; `None` — снята (набор без
    /// якоря, как после рамки).
    pub fn toggle_selection_with_primary(
        primary: Option<usize>,
        selected_nodes: &mut Vec<usize>,
        index: usize,
    ) -> Option<usize> {
        // Якорь без дубля входит в набор: якорь == index → тогл ниже снимет
        // его (клик по уже выбранной); якорь != index → сохранится
        if let Some(prev) = primary {
            if !selected_nodes.contains(&prev) {
                selected_nodes.push(prev);
            }
        }
        if toggle_selected_node(selected_nodes, index) {
            Some(index)
        } else {
            None
        }
    }

    /// Исходные позиции нод для drag (CR-001): тянем захваченную ноду;
    /// если она в наборе выделения — тянем весь набор. Дети выделенных
    /// ГРУПП включаются автоматически (паттерн translate_group: каждая
    /// нода сдвигается ровно один раз, дубликаты схлопываются).
    /// Возвращает пары (индекс, исходная позиция) по возрастанию индексов.
    pub fn drag_origins(canvas: &Canvas, grabbed: usize, selected: &[usize]) -> Vec<(usize, Vec2)> {
        // База: набор (если захваченная в нём) или одна нода
        let base: Vec<usize> = if selected.contains(&grabbed) {
            selected.to_vec()
        } else {
            vec![grabbed]
        };
        // Дети групп из базы; вложенность — без рекурсии: дитя-группа уже
        // в базе как группа, её дети добавляются этим же проходом (v1)
        let mut indices: Vec<usize> = base.clone();
        for index in &base {
            if canvas
                .nodes
                .get(*index)
                .is_some_and(|node| node.kind() == NodeKind::Group)
            {
                indices.extend(canvas_core::group_children(canvas, *index));
            }
        }
        // Каждая нода ровно один раз, порядок стабильный
        indices.sort_unstable();
        indices.dedup();
        indices
            .into_iter()
            .filter_map(|index| {
                canvas
                    .nodes
                    .get(index)
                    .map(|node| (index, [node.x, node.y]))
            })
            .collect()
    }

    /// Состояние drag ноды (CR-001): захваченная нода (primary, как раньше),
    /// world-якорь курсора в момент захвата и исходные позиции всех
    /// перемещаемых нод (выделение или одна + дети групп). На движении
    /// каждая нода ставится в origin + delta — ровно один сдвиг за кадр.
    #[derive(Debug, Clone)]
    pub struct DragState {
        /// Нода, за которую захватили (выделение рамкой не меняет её).
        pub primary: usize,
        /// World-позиция курсора при захвате.
        pub grab_world: Vec2,
        /// Исходные позиции перемещаемых нод (drag_origins).
        pub origins: Vec<(usize, Vec2)>,
    }

    // --- Буфер нодов: дублирование и копипаст (FR-003) ---

    /// Сдвиг дубликата от оригинала (FR-003, Ctrl+D): world-px по обеим осям.
    pub const DUPLICATE_OFFSET: f32 = 32.0;

    /// Префикс для нового id копии ноды (FR-003): по типу ноды —
    /// `next_free_id` даст уникальный `note-N`/`file-N`/….
    pub fn node_prefix(node: &Node) -> &'static str {
        match node.kind() {
            NodeKind::Text => "note",
            NodeKind::File => "file",
            NodeKind::Link => "link",
            NodeKind::Group | NodeKind::Unknown => "node",
        }
    }

    /// Копии нодов с новыми уникальными id (FR-003): содержимое (текст,
    /// файл, размеры, цвет, extra) переносится как есть; id — как у
    /// свежесозданных. Уникальность — не только против канваса, но и
    /// против уже назначенных в этом вызове (два `note-*` в буфере не
    /// должны получить один id: копии ещё не в канвасе).
    pub fn reassign_ids(canvas: &Canvas, nodes: &[Node]) -> Vec<Node> {
        let mut taken: Vec<String> = Vec::with_capacity(nodes.len());
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            let prefix = node_prefix(node);
            let id = std::iter::successors(Some(1u32), |n| Some(n + 1))
                .map(|n| format!("{prefix}-{n}"))
                .find(|id| !canvas.nodes.iter().any(|node| node.id == *id) && !taken.contains(id))
                .expect("счётчик найдёт свободный id");
            taken.push(id.clone());
            let mut copy = node.clone();
            copy.id = id;
            out.push(copy);
        }
        out
    }

    /// bbox набора нодов (FR-003): [x, y, w, h] по extremes; пустой
    /// набор — нулевой квад в [0,0].
    pub fn nodes_bbox(nodes: &[Node]) -> [f32; 4] {
        if nodes.is_empty() {
            return [0.0; 4];
        }
        let mut x0 = f32::MAX;
        let mut y0 = f32::MAX;
        let mut x1 = f32::MIN;
        let mut y1 = f32::MIN;
        for node in nodes {
            x0 = x0.min(node.x);
            y0 = y0.min(node.y);
            x1 = x1.max(node.x + node.width);
            y1 = y1.max(node.y + node.height);
        }
        [x0, y0, x1 - x0, y1 - y0]
    }

    /// Размещение вставки буфера нодов (FR-003).
    pub enum PastePlacement {
        /// Центр bbox набора — в world-точку курсора: взаимное расположение
        /// копий сохраняется (Ctrl+V).
        AtCursor(Vec2),
        /// Сдвиг всех копий на (dx, dy) от оригиналов (Ctrl+D).
        Offset(Vec2),
    }

    /// Вставить копии буфера с размещением (FR-003): id уже переназначены
    /// (`reassign_ids` — ДО вызова), позиции — по placement. Чистая
    /// функция: возвращает готовые к push ноды, модель не трогает.
    pub fn paste_nodes(nodes: &[Node], placement: PastePlacement) -> Vec<Node> {
        let delta = match placement {
            PastePlacement::AtCursor(cursor) => {
                let bbox = nodes_bbox(nodes);
                [
                    cursor[0] - bbox[0] - bbox[2] / 2.0,
                    cursor[1] - bbox[1] - bbox[3] / 2.0,
                ]
            }
            PastePlacement::Offset(delta) => delta,
        };
        nodes
            .iter()
            .map(|node| {
                let mut copy = node.clone();
                copy.x += delta[0];
                copy.y += delta[1];
                copy
            })
            .collect()
    }

    // --- Парсинг CF_HDROP и раскладка дропа (T9) ---

    /// Разобрать содержимое CF_HDROP ЦЕЛИКОМ: DROPFILES-заголовок (20 байт:
    /// pFiles-офсет LE, pt, fNC, fWide) + UTF-16 null-terminated строки +
    /// DOUBLE null в конце. Чистая функция от байтов — тестируется синтетикой
    /// на любой ОС (снимает shell байты с IDataObject как есть).
    ///
    /// Толерантность: нечётный хвостовой байт игнорируется; без терминатора
    /// отдаём что накопили. ANSI-вариант (fWide=0) не поддерживаем — Explorer
    /// всегда кладёт UTF-16.
    pub fn parse_hdrop_bytes(bytes: &[u8]) -> Vec<PathBuf> {
        // DROPFILES-заголовок = 20 байт; меньше — битый формат
        if bytes.len() < 20 {
            return Vec::new();
        }
        let p_files = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if p_files > bytes.len() {
            return Vec::new();
        }
        let f_wide = i32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) != 0;
        if !f_wide {
            return Vec::new();
        }
        let mut paths = Vec::new();
        let mut segment: Vec<u16> = Vec::new();
        // Пары u16; хвостовый нечётный байт (remainder) игнорируем
        for chunk in bytes[p_files..].chunks_exact(2) {
            let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
            if unit == 0 {
                if segment.is_empty() {
                    // Пустой сегмент = терминатор списка (DOUBLE null)
                    return paths;
                }
                paths.push(PathBuf::from(String::from_utf16_lossy(&segment)));
                segment.clear();
            } else {
                segment.push(unit);
            }
        }
        // Терминатора не было — отдаём что накопили
        if !segment.is_empty() {
            paths.push(PathBuf::from(String::from_utf16_lossy(&segment)));
        }
        paths
    }

    /// Разворачивает пути дропа: каталог — его дети (глубина 1, подпапки-дети
    /// НЕ разворачиваются — сами станут нодами); симлинки пропускаются (не
    /// следуем — защита от циклов); обычные файлы — как есть. Детей каталога
    /// сортируем по имени (предсказуемость сетки), скрытые/системные — мимо.
    pub fn expand_drop_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for path in paths {
            // symlink_metadata не следует по ссылке: симлинки видны сразу
            let Ok(meta) = std::fs::symlink_metadata(path) else {
                continue; // путь исчез/недоступен — пропускаем
            };
            let file_type = meta.file_type();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let Ok(entries) = std::fs::read_dir(path) else {
                    continue; // нечитаемый каталог — пропускаем целиком
                };
                let mut children: Vec<(std::ffi::OsString, PathBuf)> = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if !child_is_hidden(&name, &entry) {
                        children.push((name, entry.path()));
                    }
                }
                // Сортировка по имени: порядок сетки не зависит от выдачи FS
                children.sort_by(|a, b| a.0.cmp(&b.0));
                out.extend(children.into_iter().map(|(_, child)| child));
            } else {
                out.push(path.clone()); // обычный файл — порядок входа сохраняем
            }
        }
        out
    }

    /// Скрытый/системный ребёнок каталога? Unix — имя с ведущей точкой;
    /// Windows — FILE_ATTRIBUTE_HIDDEN (0x2) | FILE_ATTRIBUTE_SYSTEM (0x4).
    #[cfg(not(windows))]
    fn child_is_hidden(name: &std::ffi::OsStr, _entry: &std::fs::DirEntry) -> bool {
        name.to_string_lossy().starts_with('.')
    }

    /// Скрытый/системный ребёнок каталога (Windows): читаем атрибуты
    /// метаданных записи каталога, битые — считаем скрытыми (не показываем).
    #[cfg(windows)]
    fn child_is_hidden(_name: &std::ffi::OsStr, entry: &std::fs::DirEntry) -> bool {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        entry
            .metadata()
            .map(|meta| {
                meta.file_attributes() & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
            })
            .unwrap_or(true)
    }

    /// Позиции сетки дропа от origin (row-major): `count` карточек,
    /// перенос строки после DROP_GRID_COLS колонок, шаг = карточка + зазор.
    pub fn drop_grid(origin: Vec2, count: usize) -> Vec<Vec2> {
        (0..count)
            .map(|i| {
                let col = i % DROP_GRID_COLS;
                let row = i / DROP_GRID_COLS;
                [
                    origin[0] + col as f32 * (DROP_CARD_W + DROP_GRID_GAP),
                    origin[1] + row as f32 * (DROP_CARD_H + DROP_GRID_GAP),
                ]
            })
            .collect()
    }

    /// Вид текста из CF_UNICODETEXT: URL или обычный текст.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DropTextKind {
        /// Начинается (после trim) с `http://`/`https://` — без учёта регистра.
        Url,
        /// Всё остальное.
        Plain,
    }

    /// Классификация текста дропа: префикс `http://`/`https://` (case-
    /// insensitive, после trim) — Url, иначе Plain.
    pub fn drop_text_kind(text: &str) -> DropTextKind {
        let trimmed = text.trim();
        let head: String = trimmed
            .chars()
            .take("https://".len())
            .flat_map(char::to_lowercase)
            .collect();
        if head.starts_with("http://") || head == "https://" {
            DropTextKind::Url
        } else {
            DropTextKind::Plain
        }
    }

    /// Тип вставки из дропа (T9): файловая нода или заметка с текстом.
    #[derive(Debug, Clone, PartialEq)]
    pub enum DropInsertKind {
        /// Файловая нода (путь как дал Explorer, абсолютный).
        File(PathBuf),
        /// Текстовая нода: URL или произвольный текст (критерий T9 — URL
        /// становится заметкой с текстом ссылки; Plain-текст — бонус).
        Note(String),
    }

    /// Одна вставка дропа: id, тип и позиция в world-координатах.
    #[derive(Debug, Clone, PartialEq)]
    pub struct DropInsert {
        pub id: String,
        pub kind: DropInsertKind,
        pub pos: Vec2,
    }

    /// Спланировать вставку дропа (T9): сырые данные из shell -> готовые
    /// ноды с id и позициями. CF_HDROP разбирается и разворачивается
    /// (каталоги — глубина 1), позиции даёт сетка от origin; текст — единая
    /// заметка в origin. Канвас не мутируется — вставку делает приложение.
    pub fn plan_drop(
        canvas: &Canvas,
        data: &canvas_shell::dragdrop::DragData,
        origin: Vec2,
    ) -> Vec<DropInsert> {
        let occupied: HashSet<&str> = canvas.nodes.iter().map(|node| node.id.as_str()).collect();
        let mut issued: HashSet<String> = HashSet::new();
        match data {
            canvas_shell::dragdrop::DragData::HdropBytes(bytes) => {
                let paths = expand_drop_paths(&parse_hdrop_bytes(bytes));
                let positions = drop_grid(origin, paths.len());
                paths
                    .into_iter()
                    .zip(positions)
                    .map(|(path, pos)| DropInsert {
                        id: next_free_plan_id(&occupied, &mut issued, "file"),
                        kind: DropInsertKind::File(path),
                        pos,
                    })
                    .collect()
            }
            canvas_shell::dragdrop::DragData::Text(text) => {
                // И Url, и Plain -> единая заметка с полным текстом
                let kind = match drop_text_kind(text) {
                    DropTextKind::Url | DropTextKind::Plain => DropInsertKind::Note(text.clone()),
                };
                vec![DropInsert {
                    id: next_free_plan_id(&occupied, &mut issued, "note"),
                    kind,
                    pos: origin,
                }]
            }
            canvas_shell::dragdrop::DragData::None => Vec::new(),
        }
    }

    /// Следующий свободный `{prefix}-N` внутри плана: избегаем и занятых в
    /// канвасе, и уже выданных в этом плане (несколько файлов подряд).
    fn next_free_plan_id(
        occupied: &HashSet<&str>,
        issued: &mut HashSet<String>,
        prefix: &str,
    ) -> String {
        let mut n = 1u32;
        loop {
            let id = format!("{prefix}-{n}");
            if !occupied.contains(id.as_str()) && !issued.contains(&id) {
                issued.insert(id.clone());
                return id;
            }
            n += 1;
        }
    }

    /// Максимальная длина подписи призрака дропа (в символах) — длинные имена
    /// файлов обрезаются многоточием, чтобы не вылезать за призрак карточки.
    pub const DROP_GHOST_LABEL_MAX: usize = 40;

    /// Подпись призрака дропа (Т9): имя файла из пути / первая строка заметки.
    /// Чистая функция — тестируется без GPU; вызывается из оверлей-прохода
    /// для каждого пункта плана при перетаскивании.
    pub fn drop_ghost_label(kind: &DropInsertKind) -> String {
        let raw = match kind {
            DropInsertKind::File(path) => path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned()),
            DropInsertKind::Note(text) => text.lines().next().unwrap_or("").to_owned(),
        };
        let mut chars = raw.chars();
        let head: String = chars.by_ref().take(DROP_GHOST_LABEL_MAX).collect();
        if chars.next().is_some() {
            format!("{head}…")
        } else {
            head
        }
    }

    /// Ширина подписи призрака дропа в world-px: ширина карточки призрака
    /// минус боковые отступы.
    pub const DROP_GHOST_LABEL_PAD: f32 = 12.0;

    /// Rect пункта меню в world-координатах: [x, y, w, h].
    pub fn menu_item_rect(origin: Vec2, i: usize) -> [f32; 4] {
        [
            origin[0] + MENU_PADDING,
            origin[1] + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT,
            MENU_WIDTH - MENU_PADDING * 2.0,
            MENU_ITEM_HEIGHT,
        ]
    }

    /// Полный rect меню с `items` пунктами: [x, y, w, h].
    pub fn menu_rect_for(origin: Vec2, items: usize) -> [f32; 4] {
        [
            origin[0],
            origin[1],
            MENU_WIDTH,
            MENU_PADDING * 2.0 + items as f32 * MENU_ITEM_HEIGHT,
        ]
    }

    /// Полный rect меню ноды (палитра): [x, y, w, h].
    pub fn menu_rect(origin: Vec2) -> [f32; 4] {
        menu_rect_for(origin, MENU_ITEMS.len())
    }

    /// Hit-test пункта меню из `items` по world-точке.
    pub fn menu_item_at_for(origin: Vec2, point: Vec2, items: usize) -> Option<usize> {
        let [x, y, w, h] = menu_rect_for(origin, items);
        if point[0] < x
            || point[0] > x + w
            || point[1] < y + MENU_PADDING
            || point[1] > y + h - MENU_PADDING
        {
            return None;
        }
        let i = ((point[1] - y - MENU_PADDING) / MENU_ITEM_HEIGHT) as usize;
        (i < items).then_some(i)
    }

    /// Hit-test пункта меню ноды по world-точке (T7).
    pub fn menu_item_at(origin: Vec2, point: Vec2) -> Option<usize> {
        menu_item_at_for(origin, point, MENU_ITEMS.len())
    }

    /// Подпись пункта меню.
    pub fn menu_label(item: Option<&str>) -> String {
        match item {
            Some(preset) => format!("Цвет {preset}"),
            None => "Без цвета".to_owned(),
        }
    }

    /// Пункт контекстного меню связи: стиль линии, толщина или цвет
    /// (None — сброс цвета на дефолтный).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum EdgeMenuItem {
        /// Стиль линии (сплошная/пунктир/точки).
        Style(EdgeLineStyle),
        /// Толщина линии (тонкая/средняя/толстая).
        Thickness(EdgeThickness),
        /// Цвет: пресет "1".."6" или None — сброс.
        Color(Option<&'static str>),
    }

    /// Пункты меню связи (ПКМ по линии): 3 стиля, 3 толщины, 6 цветов + сброс.
    pub const EDGE_MENU_ITEMS: [EdgeMenuItem; 13] = [
        EdgeMenuItem::Style(EdgeLineStyle::Solid),
        EdgeMenuItem::Style(EdgeLineStyle::Dashed),
        EdgeMenuItem::Style(EdgeLineStyle::Dotted),
        EdgeMenuItem::Thickness(EdgeThickness::Thin),
        EdgeMenuItem::Thickness(EdgeThickness::Medium),
        EdgeMenuItem::Thickness(EdgeThickness::Thick),
        EdgeMenuItem::Color(Some("1")),
        EdgeMenuItem::Color(Some("2")),
        EdgeMenuItem::Color(Some("3")),
        EdgeMenuItem::Color(Some("4")),
        EdgeMenuItem::Color(Some("5")),
        EdgeMenuItem::Color(Some("6")),
        EdgeMenuItem::Color(None),
    ];

    /// Пункт составного меню ноды (T7 + группы): цвет из палитры,
    /// разделитель (не кликабелен) или действие.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NodeMenuItem {
        /// Цвет: пресет "1".."6" или None — сброс.
        Color(Option<&'static str>),
        /// Разделитель палитры и действий (клик игнорируется).
        Separator,
        /// «Сгруппировать»: обернуть ноду в группу (bbox = нода + padding).
        Group,
    }

    /// Меню ноды: палитра (7 пунктов) + разделитель + действия.
    pub const NODE_MENU_ITEMS: [NodeMenuItem; 9] = [
        NodeMenuItem::Color(Some("1")),
        NodeMenuItem::Color(Some("2")),
        NodeMenuItem::Color(Some("3")),
        NodeMenuItem::Color(Some("4")),
        NodeMenuItem::Color(Some("5")),
        NodeMenuItem::Color(Some("6")),
        NodeMenuItem::Color(None),
        NodeMenuItem::Separator,
        NodeMenuItem::Group,
    ];

    /// Пункт меню пустого канваса (ПКМ мимо нод и связей).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CanvasMenuItem {
        /// «Создать группу» в центре текущего viewport.
        NewGroup,
        /// T23: «Фокус на связях» — переключатель режима brainstorm-focus
        /// (галочка отражает текущее состояние).
        FocusMode,
        /// FR-004.1: «Горячие клавиши (F1)» — переключатель видимости
        /// оверлея хоткеев (галочка — панель открыта).
        Hotkeys,
    }

    /// Меню пустого канваса.
    pub const CANVAS_MENU_ITEMS: [CanvasMenuItem; 3] = [
        CanvasMenuItem::NewGroup,
        CanvasMenuItem::FocusMode,
        CanvasMenuItem::Hotkeys,
    ];

    /// Подпись пункта меню ноды. Разделитель подписи не имеет.
    pub fn node_menu_label(item: NodeMenuItem) -> Option<String> {
        match item {
            NodeMenuItem::Color(color) => Some(menu_label(color)),
            NodeMenuItem::Separator => None,
            NodeMenuItem::Group => Some("Сгруппировать".to_owned()),
        }
    }

    /// Подпись пункта меню пустого канваса. `focus_on` — состояние режима
    /// фокуса, `hotkeys_open` — состояние оверлея хоткеев: для пунктов-
    /// переключателей рисуется ✓-галочка (паттерн edge_menu_label,
    /// FR-004.1 — Hotkeys).
    pub fn canvas_menu_label(item: CanvasMenuItem, focus_on: bool, hotkeys_open: bool) -> String {
        match item {
            CanvasMenuItem::NewGroup => "Создать группу".to_owned(),
            CanvasMenuItem::FocusMode => {
                format!("{}Фокус на связях", if focus_on { "✓ " } else { "" })
            }
            CanvasMenuItem::Hotkeys => format!(
                "{}Горячие клавиши (F1)",
                if hotkeys_open { "✓ " } else { "" }
            ),
        }
    }

    /// Семя фокуса (T23) из интерактивных состояний: приоритет — нода под
    /// курсором (живое «прощупывание» графа), затем выделенная нода (фокус
    /// держится после ухода курсора), затем выделенная связь (линия + оба
    /// конца). Ничего нет — None (затемнение плавно уходит).
    pub fn focus_seed_of(hovered: Option<usize>, selected: Option<Selection>) -> Option<FocusSeed> {
        if let Some(index) = hovered {
            return Some(FocusSeed::Node(index));
        }
        match selected {
            Some(Selection::Node(index)) => Some(FocusSeed::Node(index)),
            Some(Selection::Edge(index)) => Some(FocusSeed::Edge(index)),
            None => None,
        }
    }

    // --- Группы нод (v1) ---

    /// Отступ группы от оборачиваемой ноды по всем сторонам (world-px).
    pub const GROUP_PADDING: f32 = 40.0;
    /// Ширина новой группы «Создать группу» (world-px).
    pub const GROUP_WIDTH: f32 = 400.0;
    /// Высота новой группы «Создать группу» (world-px).
    pub const GROUP_HEIGHT: f32 = 300.0;
    /// Подпись новой группы по умолчанию.
    pub const GROUP_DEFAULT_LABEL: &str = "Группа";

    /// План «Сгруппировать»: группа с bbox = rect ноды + padding по всем
    /// сторонам, подпись по умолчанию, id `group-N`. Канвас не мутируется —
    /// вставку делает приложение (паттерн plan_drop, T9).
    pub fn plan_group_around(canvas: &Canvas, index: usize, padding: f32) -> Option<Node> {
        let node = canvas.nodes.get(index)?;
        let mut group = Node::group(
            next_free_id(canvas, "group"),
            node.x - padding,
            node.y - padding,
            node.width + padding * 2.0,
            node.height + padding * 2.0,
        );
        group.label = Some(GROUP_DEFAULT_LABEL.to_owned());
        Some(group)
    }

    /// План «Создать группу»: группа GROUP_WIDTH × GROUP_HEIGHT с центром
    /// в world-точке (центр viewport), подпись по умолчанию, id `group-N`.
    pub fn plan_group_at(canvas: &Canvas, center: Vec2) -> Node {
        let mut group = Node::group(
            next_free_id(canvas, "group"),
            center[0] - GROUP_WIDTH / 2.0,
            center[1] - GROUP_HEIGHT / 2.0,
            GROUP_WIDTH,
            GROUP_HEIGHT,
        );
        group.label = Some(GROUP_DEFAULT_LABEL.to_owned());
        group
    }

    /// Выборочный hit-test среди кандидатов (выдача spatial index под
    /// точкой): сначала не-group ноды — с меньшей площадью в приоритете
    /// (ребёнок группы выбирается раньше самой группы), затем группы —
    /// верхняя по z (последняя в массиве). None — кандидатов нет.
    pub fn select_node_hit(canvas: &Canvas, candidates: &[usize]) -> Option<usize> {
        let area = |index: usize| {
            canvas
                .nodes
                .get(index)
                .map(|node| node.width * node.height)
                .unwrap_or(f32::MAX)
        };
        let mut plain: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&index| {
                canvas
                    .nodes
                    .get(index)
                    .is_some_and(|node| node.kind() != NodeKind::Group)
            })
            .collect();
        if !plain.is_empty() {
            // Меньшая площадь; при равенстве — выше по z (больший индекс)
            plain.sort_by(|a, b| area(*a).total_cmp(&area(*b)).then(b.cmp(a)));
            return plain.first().copied();
        }
        candidates
            .iter()
            .copied()
            .filter(|&index| {
                canvas
                    .nodes
                    .get(index)
                    .is_some_and(|node| node.kind() == NodeKind::Group)
            })
            .max()
    }

    /// Подпись пункта меню связи с отметкой текущего значения (`✓`).
    pub fn edge_menu_label(item: EdgeMenuItem, edge: &Edge) -> String {
        let current = match item {
            EdgeMenuItem::Style(style) => edge.style.unwrap_or(EdgeLineStyle::Solid) == style,
            EdgeMenuItem::Thickness(thickness) => edge.thickness.unwrap_or_default() == thickness,
            EdgeMenuItem::Color(color) => edge.color.as_deref() == color,
        };
        let mark = if current { "✓ " } else { "" };
        match item {
            EdgeMenuItem::Style(style) => format!("{mark}Линия: {}", style.label()),
            EdgeMenuItem::Thickness(thickness) => {
                format!("{mark}Толщина: {}", thickness.label())
            }
            EdgeMenuItem::Color(Some(preset)) => format!("{mark}Цвет {preset}"),
            EdgeMenuItem::Color(None) => {
                if edge.color.is_none() {
                    "✓ Без цвета".to_owned()
                } else {
                    "Без цвета".to_owned()
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Детектор двойного клика (T7): пара кликов в интервале — double,
        /// далёкие по времени или позиции — нет.
        #[test]
        fn double_click_detection() {
            let t0 = Instant::now();
            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [100.0, 100.0]), "первый клик");
            assert!(detector.register(t0 + Duration::from_millis(200), [102.0, 99.0]));
            // Третий клик сразу после — тоже double (считаем парами)
            assert!(detector.register(t0 + Duration::from_millis(300), [100.0, 100.0]));

            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [0.0, 0.0]));
            // Интервал превышен
            assert!(!detector.register(t0 + Duration::from_millis(600), [0.0, 0.0]));

            let mut detector = DoubleClick::new();
            assert!(!detector.register(t0, [0.0, 0.0]));
            // Курсор ушёл дальше порога
            assert!(!detector.register(t0 + Duration::from_millis(100), [50.0, 0.0]));
        }

        /// Генератор id (T7/T9): первый свободный по префиксу.
        #[test]
        fn note_id_first_free() {
            let canvas = Canvas::default();
            assert_eq!(next_free_id(&canvas, "note"), "note-1");
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-2");
            canvas.nodes.push(Node::text("note-2", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-3");
        }

        // --- CR-002: перепривязка связей ---

        /// Сцена: a(0,0,100,100) → b(400,0,100,100), явные стороны
        /// Right/Left, порты [100,50] и [400,50].
        fn rebind_canvas() -> Canvas {
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
            canvas
                .nodes
                .push(Node::file("b", "C:/b.png", 400.0, 0.0, 100.0, 100.0));
            canvas.add_edge(Edge::new(
                "e1",
                "a",
                Some(Side::Right),
                "b",
                Some(Side::Left),
            ));
            canvas
        }

        fn approx(a: [f32; 2], b: [f32; 2]) {
            assert!(
                (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4,
                "ожидалось {b:?}, получено {a:?}"
            );
        }

        /// Исток резиновой линии: новая связь — порт истока; перепривязка —
        /// НЕПОДВИЖНЫЙ (противоположный тянущемуся) конец; висячая — None.
        #[test]
        fn edge_drag_draft_origin() {
            let canvas = rebind_canvas();
            // Новая связь от right-порта a
            let drag = EdgeDrag::New {
                from_node: "a".to_owned(),
                from_side: Side::Right,
            };
            let (point, side) = drag.draft_origin(&canvas).expect("порт истока");
            assert_eq!(side, Side::Right);
            approx(point, [100.0, 50.0]);
            // Перепривязка ИСТОКА: резинка от СТОКА (left-порт b)
            let drag = EdgeDrag::Rebind {
                edge_index: 0,
                end: canvas_core::EdgeEnd::From,
            };
            let (point, side) = drag.draft_origin(&canvas).expect("неподвижный конец");
            assert_eq!(side, Side::Left, "неподвижный — противоположный конец");
            approx(point, [400.0, 50.0]);
            // Перепривязка СТОКА: резинка от ИСТОКА (right-порт a)
            let drag = EdgeDrag::Rebind {
                edge_index: 0,
                end: canvas_core::EdgeEnd::To,
            };
            let (point, side) = drag.draft_origin(&canvas).expect("неподвижный конец");
            assert_eq!(side, Side::Right);
            approx(point, [100.0, 50.0]);
            // Несуществующая нода/индекс — None (линию не рисуем)
            let drag = EdgeDrag::New {
                from_node: "ghost".to_owned(),
                from_side: Side::Right,
            };
            assert!(drag.draft_origin(&canvas).is_none());
            let drag = EdgeDrag::Rebind {
                edge_index: 9,
                end: canvas_core::EdgeEnd::To,
            };
            assert!(drag.draft_origin(&canvas).is_none());
        }

        /// Хэндлы концов выделенной связи (CR-002, рендер): кружки в портах
        /// обоих концов; висячая связь — пусто.
        #[test]
        fn edge_handle_instances_at_endpoints() {
            let canvas = rebind_canvas();
            let handles = canvas_render::cards::build_edge_handle_instances(&canvas, 0, 14.0);
            assert_eq!(handles.len(), 2, "хэндлы обоих концов");
            let centers: Vec<[f32; 2]> = handles
                .iter()
                .map(|inst| {
                    [
                        inst.pos[0] + inst.size[0] / 2.0,
                        inst.pos[1] + inst.size[1] / 2.0,
                    ]
                })
                .collect();
            approx(centers[0], [100.0, 50.0]);
            approx(centers[1], [400.0, 50.0]);
            // Висячая связь — пусто
            let mut dangling = rebind_canvas();
            dangling.nodes.remove(1);
            assert!(
                canvas_render::cards::build_edge_handle_instances(&dangling, 0, 14.0).is_empty()
            );
        }

        // --- CR-001: множественное выделение ---

        /// Сцена: две ноды и группа с ребёнком (индексы 0, 1 — ноды,
        /// 2 — группа, 3 — дитя группы).
        fn selection_canvas() -> Canvas {
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
            canvas
                .nodes
                .push(Node::file("b", "C:/b.png", 300.0, 0.0, 100.0, 100.0));
            // Группа 200..600 × 200..500, дитя — центром внутри
            canvas
                .nodes
                .push(Node::group("g", 200.0, 200.0, 400.0, 300.0));
            canvas
                .nodes
                .push(Node::file("c", "C:/c.png", 350.0, 300.0, 50.0, 50.0));
            canvas
        }

        /// Рамка: нормализация углов (тянуть в любую сторону); nodes_in_rect —
        /// AABB-пересечение с частичным вхождением, мимо — пусто.
        #[test]
        fn rubber_band_selects_intersecting_nodes() {
            let canvas = selection_canvas();
            // Прямое направление
            let rect = rubber_band_rect([0.0, 0.0], [200.0, 100.0]);
            assert_eq!(rect, [0.0, 0.0, 200.0, 100.0]);
            // Обратное направление — нормализация
            let rect = rubber_band_rect([200.0, 100.0], [0.0, 0.0]);
            assert_eq!(rect, [0.0, 0.0, 200.0, 100.0]);
            // Рамка 0..200 × 0..100: a целиком; b (300..400) — мимо;
            // группа (200..600 × 200..500) — краем не заходит
            assert_eq!(nodes_in_rect(&canvas, rect), vec![0]);
            // Широкая рамка: a целиком, b частично, группа частично
            let rect = rubber_band_rect([-50.0, -50.0], [350.0, 250.0]);
            assert_eq!(nodes_in_rect(&canvas, rect), vec![0, 1, 2]);
            // Рамка только по дитяти (внутри группы): c и сама группа —
            // обе пересекают прямоугольник
            let rect = rubber_band_rect([300.0, 250.0], [400.0, 400.0]);
            assert_eq!(nodes_in_rect(&canvas, rect), vec![2, 3]);
            // Вырожденная (клик) — пусто
            let rect = rubber_band_rect([1000.0, 1000.0], [1000.0, 1000.0]);
            assert!(nodes_in_rect(&canvas, rect).is_empty());
        }

        /// Тогл: добавление в конец, снятие, повторное добавление.
        #[test]
        fn toggle_selected_node_add_remove() {
            let mut selected = Vec::new();
            assert!(toggle_selected_node(&mut selected, 3), "добавили 3");
            assert!(toggle_selected_node(&mut selected, 7), "добавили 7");
            assert_eq!(selected, vec![3, 7], "порядок добавления");
            assert!(!toggle_selected_node(&mut selected, 3), "сняли 3");
            assert_eq!(selected, vec![7]);
            assert!(toggle_selected_node(&mut selected, 3), "снова 3");
            assert_eq!(selected, vec![7, 3]);
        }

        /// CR-001.3 (ревью 2026-09-14): Ctrl+клик при одиночном якоре —
        /// якорь сохраняется, клик-нутая добавляется; клик по уже
        /// выбранной (якорю/в наборе) — снятие; якорь = Edge не участвует.
        #[test]
        fn toggle_with_primary_promotes_and_toggles() {
            // Одиночный якорь 0, набор пуст, Ctrl+клик по 1: обе выделены
            let mut selected = Vec::new();
            let anchor = toggle_selection_with_primary(Some(0), &mut selected, 1);
            assert_eq!(selected, vec![0, 1], "якорь вошёл в набор, 1 добавлена");
            assert_eq!(anchor, Some(1));

            // Ctrl+клик по якорю 0 (уже в наборе после прошлого шага):
            // снятие, якорь None
            let anchor = toggle_selection_with_primary(Some(0), &mut selected, 0);
            assert_eq!(selected, vec![1], "0 снята");
            assert_eq!(anchor, None, "якорь снят");

            // Одиночный якорь 5, набор пуст, Ctrl+клик по 5: пуш и тогл —
            // выделение снято (клик по уже выбранной)
            let mut selected = Vec::new();
            let anchor = toggle_selection_with_primary(Some(5), &mut selected, 5);
            assert!(selected.is_empty(), "5 добавлена и тут же снята");
            assert_eq!(anchor, None);

            // Якорь 2 при наборе {2, 9}: дубликата нет, клик по 4 — набор
            let mut selected = vec![2, 9];
            let anchor = toggle_selection_with_primary(Some(2), &mut selected, 4);
            assert_eq!(selected, vec![2, 9, 4]);
            assert_eq!(anchor, Some(4));

            // Якорь None (Edge/ничего), набор {1}: прежнее поведение — тогл
            let mut selected = vec![1];
            let anchor = toggle_selection_with_primary(None, &mut selected, 1);
            assert!(selected.is_empty());
            assert_eq!(anchor, None);
        }

        /// drag_origins: одна нода; нода из набора — весь набор; дети
        /// выделенных групп — автоматически, дубликаты схлопываются.
        #[test]
        fn drag_origins_set_children_and_dedup() {
            let canvas = selection_canvas();
            // Захват вне набора — только захваченная нода
            let origins = drag_origins(&canvas, 0, &[1]);
            assert_eq!(origins, vec![(0, [0.0, 0.0])]);
            // Захват группы (2) без набора — группа + дитя (3)
            let origins = drag_origins(&canvas, 2, &[]);
            assert_eq!(
                origins,
                vec![(2, [200.0, 200.0]), (3, [350.0, 300.0])],
                "дети группы тянутся вместе"
            );
            // Захват b(1) из набора {1, 2}: весь набор + дети группы,
            // по возрастанию индексов, без дубликатов
            let origins = drag_origins(&canvas, 1, &[1, 2]);
            assert_eq!(
                origins,
                vec![(1, [300.0, 0.0]), (2, [200.0, 200.0]), (3, [350.0, 300.0])],
                "набор + дети групп, по возрастанию"
            );
            // Дитя (3) в наборе И группа (2) в наборе: 3 один раз
            let origins = drag_origins(&canvas, 3, &[3, 2]);
            assert_eq!(
                origins.len(),
                2,
                "дитя в наборе и от группы — ровно один раз"
            );
            // Невалидный индекс — пусто (filter_map)
            assert!(drag_origins(&canvas, 9, &[]).is_empty());
        }

        // --- Буфер нодов (FR-003) ---

        /// reassign_ids: уникальные id даже для нескольких нодов одного
        /// типа (копии ещё не в канвасе), префиксы по типу, контент как есть.
        #[test]
        fn reassign_ids_unique_per_type() {
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "текст", 0.0, 0.0));
            // Две заметки в буфере: note-2 и note-3, не два одинаковых
            let buffer = vec![
                Node::text("note-1", "текст", 0.0, 0.0),
                Node::text("x", "другая", 100.0, 100.0),
            ];
            let copies = reassign_ids(&canvas, &buffer);
            assert_eq!(
                copies
                    .iter()
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["note-2", "note-3"],
                "разные id для нодов одного типа"
            );
            assert_eq!(copies[0].text.as_deref(), Some("текст"), "контент как есть");
            // Префиксы по типу
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            let buffer = vec![
                Node::text("a", "", 0.0, 0.0),
                Node::file("b", "C:/x.png", 0.0, 0.0, 100.0, 100.0),
                Node::group("g", 0.0, 0.0, 100.0, 100.0),
            ];
            let copies = reassign_ids(&canvas, &buffer);
            assert_eq!(node_prefix(&copies[0]), "note");
            assert_eq!(node_prefix(&copies[1]), "file");
            assert_eq!(node_prefix(&copies[2]), "node", "группа — префикс node");
            assert_eq!(copies[1].id, "file-1");
            assert_eq!(copies[2].id, "node-1");
            // Пустой буфер — пусто
            assert!(reassign_ids(&canvas, &[]).is_empty());
        }

        /// nodes_bbox + paste_nodes: AtCursor — центр bbox на курсор с
        /// сохранением взаимного расположения; Offset — сдвиг всех.
        #[test]
        fn paste_nodes_placement_and_bbox() {
            // Две ноды: (0,0,100,100) и (300,0,100,100) → bbox [0,0,400,100]
            let buffer = vec![
                Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0),
                Node::file("b", "C:/b.png", 300.0, 0.0, 100.0, 100.0),
            ];
            assert_eq!(nodes_bbox(&buffer), [0.0, 0.0, 400.0, 100.0]);
            assert_eq!(nodes_bbox(&[]), [0.0; 4]);
            // Вставка центром на (1000, 500): центр bbox (200, 50) →
            // сдвиг (800, 450); взаимные позиции сохранены
            let pasted = paste_nodes(&buffer, PastePlacement::AtCursor([1000.0, 500.0]));
            assert_eq!((pasted[0].x, pasted[0].y), (800.0, 450.0));
            assert_eq!((pasted[1].x, pasted[1].y), (1100.0, 450.0));
            assert_eq!(pasted[1].x - pasted[0].x, 300.0, "взаимное расположение");
            // Offset — простой сдвиг
            let pasted = paste_nodes(&buffer, PastePlacement::Offset([32.0, 32.0]));
            assert_eq!((pasted[0].x, pasted[0].y), (32.0, 32.0));
            assert_eq!((pasted[1].x, pasted[1].y), (332.0, 32.0));
            // Контент не меняется (id/файлы при переносе — как заданы)
            assert_eq!(pasted[0].file.as_deref(), Some("C:/a.png"));
        }

        // --- Оверлей горячих клавиш (FR-004) ---

        /// Панель хоткеев: слева, вертикально по центру; высота клампится
        /// к окну (малые экраны), ширина — к окну.
        #[test]
        fn hotkeys_panel_rect_centered_left() {
            // Высокое окно: полная высота, центр по вертикали
            let full = hotkeys_panel_height();
            let rect = hotkeys_panel_rect([1600.0, 900.0]);
            assert_eq!(rect[0], SETTINGS_MARGIN, "у левого края");
            assert_eq!(rect[2], HOTKEYS_PANEL_WIDTH);
            assert_eq!(rect[3], full, "высокое окно — без клампа");
            assert!(
                (rect[1] - (900.0 - full) / 2.0).abs() < 1e-3,
                "вертикальный центр: {}",
                rect[1]
            );
            // Низ не вылезает
            assert!(rect[1] + rect[3] <= 900.0 - SETTINGS_MARGIN + 1e-3);
            // Малое окно: высота клампнута, отступ сверху сохранён
            let rect = hotkeys_panel_rect([1600.0, 300.0]);
            assert_eq!(rect[3], 300.0 - SETTINGS_MARGIN * 2.0, "кламп к окну");
            assert!(rect[1] >= SETTINGS_MARGIN);
            assert!(rect[1] + rect[3] <= 300.0 - SETTINGS_MARGIN + 1e-3);
            // Узкое окно: ширина клампнута
            let rect = hotkeys_panel_rect([200.0, 900.0]);
            assert!(rect[2] <= 200.0);
            // Данные хоткеев: непустые пары, колонка клавиш влезает
            assert!(!HOTKEYS.is_empty());
            for (key, description) in HOTKEYS {
                assert!(!key.is_empty(), "пустая клавиша");
                assert!(!description.is_empty(), "пустое описание: {key}");
            }
        }

        // --- Drag-drop (T9) ---

        /// Синтетический CF_HDROP: DROPFILES-заголовок {pFiles=20, pt=0,
        /// fNC=0, fWide=1} + UTF-16 строки с \0 каждая + финальный \0.
        fn hdrop(paths: &[&str]) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&20u32.to_le_bytes()); // pFiles — офсет строк
            bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.x
            bytes.extend_from_slice(&0i32.to_le_bytes()); // pt.y
            bytes.extend_from_slice(&0i32.to_le_bytes()); // fNC
            bytes.extend_from_slice(&1i32.to_le_bytes()); // fWide
            for path in paths {
                for unit in path.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes.extend_from_slice(&0u16.to_le_bytes());
            }
            bytes.extend_from_slice(&0u16.to_le_bytes()); // DOUBLE null — конец списка
            bytes
        }

        /// CF_HDROP: два пути, пустой список.
        #[test]
        fn parse_hdrop_paths_and_empty() {
            let paths = parse_hdrop_bytes(&hdrop(&["C:/a.txt", "C:/dir/b.jpg"]));
            assert_eq!(
                paths,
                vec![PathBuf::from("C:/a.txt"), PathBuf::from("C:/dir/b.jpg")]
            );
            assert!(parse_hdrop_bytes(&hdrop(&[])).is_empty());
        }

        /// Битый CF_HDROP: короткий буфер, pFiles больше длины, ANSI-вариант.
        #[test]
        fn parse_hdrop_malformed() {
            // len < 20 — вообще не DROPFILES
            assert!(parse_hdrop_bytes(&[0u8; 19]).is_empty());
            // pFiles указывает за конец буфера
            let mut bytes = hdrop(&["C:/a.txt"]);
            bytes[0..4].copy_from_slice(&100u32.to_le_bytes());
            assert!(parse_hdrop_bytes(&bytes).is_empty());
            // fWide = 0 — ANSI не поддерживаем (Explorer всегда UTF-16)
            let mut bytes = hdrop(&["C:/a.txt"]);
            bytes[16..20].copy_from_slice(&0i32.to_le_bytes());
            assert!(parse_hdrop_bytes(&bytes).is_empty());
        }

        /// Нет терминатора списка — отдаём что накопили; нечётный хвост — мимо.
        #[test]
        fn parse_hdrop_without_terminator() {
            // header + "a.txt\0" + "b.txt" (без \0 и без DOUBLE null) + мусорный байт
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&20u32.to_le_bytes());
            bytes.extend_from_slice(&[0u8; 12]);
            bytes.extend_from_slice(&1i32.to_le_bytes());
            for path in ["a.txt", "b.txt"] {
                for unit in path.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                if path == "a.txt" {
                    bytes.extend_from_slice(&0u16.to_le_bytes());
                }
            }
            bytes.push(0xff); // нечётный хвостовой байт — игнорируется
            let paths = parse_hdrop_bytes(&bytes);
            assert_eq!(paths, vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")]);
        }

        /// Уникальный temp-каталог теста (без новых зависимостей).
        fn temp_dir(name: &str) -> PathBuf {
            let dir =
                std::env::temp_dir().join(format!("canvasdesk-t9-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("tempdir");
            dir
        }

        /// Каталог -> дети глубины 1 (подпапка сама нода), скрытый skip,
        /// сортировка по имени.
        #[test]
        fn expand_drop_folder_depth_one() {
            let dir = temp_dir("folder");
            std::fs::write(dir.join("b.txt"), b"1").unwrap();
            std::fs::write(dir.join("a.txt"), b"2").unwrap();
            std::fs::write(dir.join(".hidden"), b"3").unwrap();
            // На Windows «скрытый» — атрибут файла, а не точка в имени:
            // выставляем attrib +h (встроенная команда), иначе фильтр
            // FILE_ATTRIBUTE_HIDDEN не имеет что фильтровать (урок CI fa1ca0b).
            // На Unix достаточно имени с ведущей точкой.
            #[cfg(windows)]
            {
                let status = std::process::Command::new("attrib")
                    .arg("+h")
                    .arg(dir.join(".hidden").as_os_str())
                    .status()
                    .expect("запуск attrib");
                assert!(status.success(), "attrib +h не смог скрыть файл");
            }
            std::fs::create_dir_all(dir.join("sub")).unwrap();
            let paths = expand_drop_paths(std::slice::from_ref(&dir));
            assert_eq!(
                paths,
                vec![dir.join("a.txt"), dir.join("b.txt"), dir.join("sub")]
            );
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Файл -> сам; порядок входа сохраняется (не сортируется).
        #[test]
        fn expand_drop_files_keep_order() {
            let dir = temp_dir("files");
            let z = dir.join("z.txt");
            let a = dir.join("a.txt");
            std::fs::write(&z, b"1").unwrap();
            std::fs::write(&a, b"2").unwrap();
            let paths = expand_drop_paths(&[z.clone(), a.clone()]);
            assert_eq!(paths, vec![z, a]);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Симлинк пропускается (не следуем — защита от циклов). Unix-only.
        #[cfg(unix)]
        #[test]
        fn expand_drop_symlink_skipped() {
            let dir = temp_dir("symlink");
            std::fs::write(dir.join("real.txt"), b"x").unwrap();
            std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();
            let paths = expand_drop_paths(&[dir.join("link.txt")]);
            assert!(paths.is_empty(), "симлинк должен быть пропущен: {paths:?}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// Сетка дропа: 0/1/5/6/11 позиций, шаг карточка+зазор, перенос
        /// строки после 5 колонок (критерий T9 — предсказуемость).
        #[test]
        fn drop_grid_layout() {
            assert!(drop_grid([0.0, 0.0], 0).is_empty());
            assert_eq!(drop_grid([10.0, 20.0], 1), vec![[10.0, 20.0]]);
            let five = drop_grid([0.0, 0.0], 5);
            assert_eq!(five.len(), 5);
            assert_eq!(five[1][0], DROP_CARD_W + DROP_GRID_GAP);
            assert_eq!(five[4][0], 4.0 * (DROP_CARD_W + DROP_GRID_GAP));
            // 6-я — вторая строка, с origin.x
            let six = drop_grid([7.0, 11.0], 6);
            assert_eq!(six[5], [7.0, 11.0 + DROP_CARD_H + DROP_GRID_GAP]);
            // 11-я — третья строка
            let eleven = drop_grid([0.0, 0.0], 11);
            assert_eq!(eleven[10][1], 2.0 * (DROP_CARD_H + DROP_GRID_GAP));
        }

        /// next_free_id: первый свободный по префиксу, чужие префиксы не мешают.
        #[test]
        fn next_free_id_prefixes() {
            let canvas = Canvas::default();
            assert_eq!(next_free_id(&canvas, "note"), "note-1");
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            assert_eq!(next_free_id(&canvas, "note"), "note-2");
            canvas
                .nodes
                .push(Node::file("file-1", "a", 0.0, 0.0, 1.0, 1.0));
            canvas
                .nodes
                .push(Node::file("file-2", "b", 0.0, 0.0, 1.0, 1.0));
            assert_eq!(next_free_id(&canvas, "file"), "file-3");
            // Чистый префикс — с 1
            assert_eq!(next_free_id(&canvas, "link"), "link-1");
        }

        /// Классификация текста: http/https без учёта регистра — Url.
        #[test]
        fn drop_text_kind_detection() {
            assert_eq!(drop_text_kind("http://example.com"), DropTextKind::Url);
            assert_eq!(drop_text_kind("https://example.com"), DropTextKind::Url);
            assert_eq!(drop_text_kind("  HTTPs://Example.COM "), DropTextKind::Url);
            assert_eq!(drop_text_kind("привет"), DropTextKind::Plain);
            assert_eq!(drop_text_kind(""), DropTextKind::Plain);
        }

        /// План дропа файлов: свободные id с учётом занятых, сетка от origin,
        /// канвас не мутируется. Пути — реальные temp-файлы: expand их
        /// проверяет на ФС (несуществующие пропускаются).
        #[test]
        fn plan_drop_files_grid_and_ids() {
            let dir = temp_dir("plan");
            std::fs::write(dir.join("a.png"), b"1").unwrap();
            std::fs::write(dir.join("b.png"), b"2").unwrap();
            std::fs::write(dir.join("c.png"), b"3").unwrap();
            let a = dir.join("a.png").to_string_lossy().into_owned();
            let b = dir.join("b.png").to_string_lossy().into_owned();
            let c = dir.join("c.png").to_string_lossy().into_owned();
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("file-1", "old.png", 0.0, 0.0, 10.0, 10.0));
            let plan = plan_drop(
                &canvas,
                &canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[&a, &b, &c])),
                [100.0, 200.0],
            );
            assert_eq!(plan.len(), 3);
            // file-1 занят в канвасе — нумерация со свободных
            assert_eq!(plan[0].id, "file-2");
            assert_eq!(plan[1].id, "file-3");
            assert_eq!(plan[2].id, "file-4");
            assert_eq!(plan[0].pos, [100.0, 200.0]);
            assert_eq!(plan[1].pos, [100.0 + DROP_CARD_W + DROP_GRID_GAP, 200.0]);
            assert_eq!(plan[0].kind, DropInsertKind::File(PathBuf::from(&a)));
            // Канвас не мутирован
            assert_eq!(canvas.nodes.len(), 1);
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// План дропа текста: одна заметка в origin с полным текстом.
        #[test]
        fn plan_drop_text_single_note() {
            let plan = plan_drop(
                &Canvas::default(),
                &canvas_shell::dragdrop::DragData::Text("https://example.com".into()),
                [5.0, 6.0],
            );
            assert_eq!(plan.len(), 1);
            assert_eq!(plan[0].id, "note-1");
            assert_eq!(plan[0].pos, [5.0, 6.0]);
            assert_eq!(
                plan[0].kind,
                DropInsertKind::Note("https://example.com".into())
            );
        }

        /// Нет поддерживаемых форматов — план пуст.
        #[test]
        fn plan_drop_none_is_empty() {
            assert!(plan_drop(
                &Canvas::default(),
                &canvas_shell::dragdrop::DragData::None,
                [0.0, 0.0]
            )
            .is_empty());
        }

        /// Подпись призрака дропа (Т9): имя файла из пути (Windows-разделитель).
        #[test]
        fn drop_ghost_label_file_name() {
            let kind = DropInsertKind::File(PathBuf::from("C:\\Users\\danku\\Downloads\\SPEC.md"));
            assert_eq!(drop_ghost_label(&kind), "SPEC.md");
            // Unix-путь для кроссплатформенности теста
            let kind = DropInsertKind::File(PathBuf::from("docs/TASKS.md"));
            assert_eq!(drop_ghost_label(&kind), "TASKS.md");
        }

        /// Подпись заметки — первая строка; длинные строки обрезаются.
        #[test]
        fn drop_ghost_label_note_first_line_and_truncation() {
            let kind = DropInsertKind::Note("первая строка\nвторая".into());
            assert_eq!(drop_ghost_label(&kind), "первая строка");
            let long = "а".repeat(DROP_GHOST_LABEL_MAX + 10);
            let kind = DropInsertKind::Note(long.clone());
            let label = drop_ghost_label(&kind);
            assert!(label.ends_with('…'), "обрезка с многоточием: {label}");
            assert_eq!(label.chars().count(), DROP_GHOST_LABEL_MAX + 1);
            // Ровно лимит — без многоточия
            let exact = "б".repeat(DROP_GHOST_LABEL_MAX);
            let kind = DropInsertKind::Note(exact.clone());
            assert_eq!(drop_ghost_label(&kind), exact);
        }

        /// Hit-test меню (T7): пункты палитры, края, промахи.
        #[test]
        fn menu_hit_test() {
            let origin = [100.0, 50.0];
            // Первый пункт (цвет "1")
            assert_eq!(
                menu_item_at(origin, [110.0, 50.0 + MENU_PADDING + 3.0]),
                Some(0)
            );
            // Последний пункт (сброс цвета)
            let last_y = 50.0 + MENU_PADDING + 6.0 * MENU_ITEM_HEIGHT + 3.0;
            assert_eq!(menu_item_at(origin, [110.0, last_y]), Some(6));
            // Правее меню, выше, ниже — промах
            assert_eq!(menu_item_at(origin, [100.0 + MENU_WIDTH + 1.0, 60.0]), None);
            assert_eq!(menu_item_at(origin, [110.0, 49.0]), None);
            assert_eq!(
                menu_item_at(origin, [110.0, 50.0 + menu_rect(origin)[3] + 1.0]),
                None
            );
            // Вертикальный паддинг между рамкой и первым пунктом — промах
            assert_eq!(menu_item_at(origin, [110.0, 51.0]), None);
        }

        /// Hit-test меню связи: 13 пунктов (3 стиля, 3 толщины, 7 цветов),
        /// границы групп различимы.
        #[test]
        fn edge_menu_hit_test() {
            let origin = [100.0, 50.0];
            let n = EDGE_MENU_ITEMS.len();
            assert_eq!(n, 13);
            // Первый пункт (стиль «сплошная»)
            assert_eq!(
                menu_item_at_for(origin, [110.0, 50.0 + MENU_PADDING + 3.0], n),
                Some(0)
            );
            // Граница групп: толщина «тонкая» (индекс 3) и цвет "1" (индекс 6)
            let y = |i: usize| 50.0 + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT + 3.0;
            assert_eq!(menu_item_at_for(origin, [110.0, y(3)], n), Some(3));
            assert_eq!(menu_item_at_for(origin, [110.0, y(6)], n), Some(6));
            // Последний пункт — сброс цвета
            assert_eq!(menu_item_at_for(origin, [110.0, y(12)], n), Some(12));
            // Промахи: правее, выше, ниже
            assert_eq!(
                menu_item_at_for(origin, [100.0 + MENU_WIDTH + 1.0, 60.0], n),
                None
            );
            assert_eq!(menu_item_at_for(origin, [110.0, 49.0], n), None);
            assert_eq!(
                menu_item_at_for(origin, [110.0, 50.0 + menu_rect_for(origin, n)[3] + 1.0], n),
                None
            );
        }

        /// Подписи меню связи: отметка `✓` только у текущих значений.
        #[test]
        fn edge_menu_labels_mark_current() {
            let mut edge = Edge::new("e1", "a", None, "b", None);
            edge.style = Some(EdgeLineStyle::Dashed);
            edge.thickness = Some(EdgeThickness::Thick);
            edge.color = Some("3".into());

            let label = |item| edge_menu_label(item, &edge);
            assert!(!label(EDGE_MENU_ITEMS[0]).starts_with('✓'));
            assert!(label(EDGE_MENU_ITEMS[1]).starts_with("✓"), "dashed текущий");
            assert!(!label(EDGE_MENU_ITEMS[2]).starts_with('✓'));
            assert!(!label(EDGE_MENU_ITEMS[3]).starts_with('✓'));
            assert!(!label(EDGE_MENU_ITEMS[4]).starts_with('✓'));
            assert!(label(EDGE_MENU_ITEMS[5]).starts_with('✓'), "thick текущий");
            assert!(!label(EDGE_MENU_ITEMS[6]).starts_with('✓'));
            assert!(label(EDGE_MENU_ITEMS[8]).starts_with("✓"), "цвет 3 текущий");
            assert!(!label(EDGE_MENU_ITEMS[12]).starts_with('✓'));

            // Без стилей: ✓ у дефолтов (solid/medium/без цвета)
            let plain = Edge::new("e2", "a", None, "b", None);
            assert!(edge_menu_label(EDGE_MENU_ITEMS[0], &plain).starts_with('✓'));
            assert!(edge_menu_label(EDGE_MENU_ITEMS[4], &plain).starts_with('✓'));
            assert!(edge_menu_label(EDGE_MENU_ITEMS[12], &plain).starts_with('✓'));
        }

        /// Составное меню ноды: 9 пунктов (7 палитра + разделитель +
        /// «Сгруппировать»), hit-test по длине, разделитель без подписи.
        #[test]
        fn node_menu_items_and_hit_test() {
            let origin = [100.0, 50.0];
            let n = NODE_MENU_ITEMS.len();
            assert_eq!(n, 9);
            assert_eq!(NODE_MENU_ITEMS[7], NodeMenuItem::Separator);
            assert_eq!(NODE_MENU_ITEMS[8], NodeMenuItem::Group);
            // Подписи: палитра + действие; у разделителя подписи нет
            assert_eq!(
                node_menu_label(NODE_MENU_ITEMS[0]),
                Some("Цвет 1".to_owned())
            );
            assert_eq!(
                node_menu_label(NODE_MENU_ITEMS[6]),
                Some("Без цвета".to_owned())
            );
            assert_eq!(node_menu_label(NODE_MENU_ITEMS[7]), None);
            assert_eq!(
                node_menu_label(NODE_MENU_ITEMS[8]),
                Some("Сгруппировать".to_owned())
            );
            // Hit-test: первый пункт, разделитель (индекс 7), действие (8)
            let y = |i: usize| 50.0 + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT + 3.0;
            assert_eq!(menu_item_at_for(origin, [110.0, y(0)], n), Some(0));
            assert_eq!(menu_item_at_for(origin, [110.0, y(7)], n), Some(7));
            assert_eq!(menu_item_at_for(origin, [110.0, y(8)], n), Some(8));
            // Промахи: правее, выше, ниже
            assert_eq!(
                menu_item_at_for(origin, [100.0 + MENU_WIDTH + 1.0, 60.0], n),
                None
            );
            assert_eq!(menu_item_at_for(origin, [110.0, 49.0], n), None);
            assert_eq!(
                menu_item_at_for(origin, [110.0, 50.0 + menu_rect_for(origin, n)[3] + 1.0], n),
                None
            );
        }

        /// Меню пустого канваса: «Создать группу» + «Фокус на связях» (T23)
        /// + «Горячие клавиши (F1)» (FR-004.1).
        #[test]
        fn canvas_menu_single_item() {
            let origin = [100.0, 50.0];
            let n = CANVAS_MENU_ITEMS.len();
            assert_eq!(n, 3);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[0], false, false),
                "Создать группу"
            );
            // T23: второй пункт — переключатель фокуса с ✓-галочкой
            assert_eq!(CANVAS_MENU_ITEMS[1], CanvasMenuItem::FocusMode);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[1], true, false),
                "✓ Фокус на связях"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[1], false, false),
                "Фокус на связях"
            );
            // FR-004.1: третий пункт — переключатель оверлея хоткеев
            assert_eq!(CANVAS_MENU_ITEMS[2], CanvasMenuItem::Hotkeys);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[2], false, true),
                "✓ Горячие клавиши (F1)"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[2], false, false),
                "Горячие клавиши (F1)"
            );
            let y = 50.0 + MENU_PADDING + 3.0;
            assert_eq!(menu_item_at_for(origin, [110.0, y], n), Some(0));
            // Хит-test второго пункта
            let y1 = 50.0 + MENU_PADDING + MENU_ITEM_HEIGHT + 3.0;
            assert_eq!(menu_item_at_for(origin, [110.0, y1], n), Some(1));
            assert_eq!(
                menu_item_at_for(origin, [110.0, 50.0 + menu_rect_for(origin, n)[3] + 1.0], n),
                None
            );
        }

        /// T23: приоритет семени фокуса — hover > выделенная нода >
        /// выделенная связь > ничего.
        #[test]
        fn focus_seed_priority() {
            assert_eq!(focus_seed_of(None, None), None);
            assert_eq!(
                focus_seed_of(None, Some(Selection::Node(3))),
                Some(FocusSeed::Node(3))
            );
            assert_eq!(
                focus_seed_of(None, Some(Selection::Edge(7))),
                Some(FocusSeed::Edge(7))
            );
            // Hover побеждает выделение — «живое» прошупывание графа
            assert_eq!(
                focus_seed_of(Some(1), Some(Selection::Node(3))),
                Some(FocusSeed::Node(1))
            );
            assert_eq!(
                focus_seed_of(Some(2), Some(Selection::Edge(9))),
                Some(FocusSeed::Node(2))
            );
        }

        /// План «Сгруппировать»: bbox = нода + padding 40, label по умолчанию,
        /// id group-N; невалидный индекс — None.
        #[test]
        fn plan_group_around_bbox() {
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("a", "C:/a.png", 100.0, 200.0, 260.0, 120.0));
            let group = plan_group_around(&canvas, 0, GROUP_PADDING).expect("нода есть");
            assert_eq!(group.kind(), NodeKind::Group);
            assert_eq!(group.id, "group-1");
            assert_eq!(group.label.as_deref(), Some(GROUP_DEFAULT_LABEL));
            assert_eq!(
                (group.x, group.y, group.width, group.height),
                (
                    100.0 - GROUP_PADDING,
                    200.0 - GROUP_PADDING,
                    260.0 + GROUP_PADDING * 2.0,
                    120.0 + GROUP_PADDING * 2.0
                )
            );
            // Занятый id пропускается
            canvas.nodes.push(group);
            let second = plan_group_around(&canvas, 0, GROUP_PADDING).expect("нода есть");
            assert_eq!(second.id, "group-2");
            // Канвас не мутирован планом
            assert_eq!(canvas.nodes.len(), 2);
            // Невалидный индекс
            assert!(plan_group_around(&canvas, 99, GROUP_PADDING).is_none());
        }

        /// План «Создать группу»: 400×300 с центром в точке, label по умолчанию.
        #[test]
        fn plan_group_at_viewport_center() {
            let canvas = Canvas::default();
            let group = plan_group_at(&canvas, [1000.0, 600.0]);
            assert_eq!(group.kind(), NodeKind::Group);
            assert_eq!(group.id, "group-1");
            assert_eq!(group.label.as_deref(), Some(GROUP_DEFAULT_LABEL));
            assert_eq!(group.width, GROUP_WIDTH);
            assert_eq!(group.height, GROUP_HEIGHT);
            // Центр бокса — в заданной точке
            assert_eq!(
                (group.x + group.width / 2.0, group.y + group.height / 2.0),
                (1000.0, 600.0)
            );
        }

        /// Выборочный hit-test: ребёнок (не-group) выбирается раньше группы;
        /// вне детей — сама группа; пустые кандидаты — None.
        #[test]
        fn select_node_hit_prefers_children() {
            let mut canvas = Canvas::default();
            // Группа 0..400 × 0..300, затем ребёнок, затем соседняя нода
            canvas.nodes.push(Node::group("g", 0.0, 0.0, 400.0, 300.0));
            canvas
                .nodes
                .push(Node::file("child", "C:/c.png", 50.0, 50.0, 100.0, 80.0));
            canvas
                .nodes
                .push(Node::file("big", "C:/b.png", 0.0, 400.0, 500.0, 200.0));

            let spatial = SpatialIndex::build(&canvas);
            // Точка внутри ребёнка: группа тоже содержит её, но выбор — ребёнок
            let point = [100.0, 90.0];
            let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
            assert!(candidates.contains(&0) && candidates.contains(&1));
            assert_eq!(select_node_hit(&canvas, &candidates), Some(1));
            // Свободная часть группы (вне ребёнка): выбор — группа
            let point = [300.0, 250.0];
            let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
            assert_eq!(select_node_hit(&canvas, &candidates), Some(0));
            // Не-group без группы — как раньше (верхняя по z из попавших)
            let point = [200.0, 500.0];
            let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
            assert_eq!(candidates, vec![2]);
            assert_eq!(select_node_hit(&canvas, &candidates), Some(2));
            // Пустые кандидаты
            assert_eq!(select_node_hit(&canvas, &[]), None);
        }

        /// Зона resize (T7): правый нижний угол ноды, границы включительны.
        #[test]
        fn resize_corner_hit_zone() {
            let mut node = Node::text("n", "t", 100.0, 100.0);
            node.width = 260.0;
            node.height = 120.0;
            // Угол (360, 220): внутри зоны
            assert!(in_resize_corner(&node, [355.0, 215.0]));
            assert!(in_resize_corner(&node, [360.0, 220.0]));
            // Снаружи: левее/выше зоны, за пределами ноды
            assert!(!in_resize_corner(&node, [340.0, 215.0]));
            assert!(!in_resize_corner(&node, [355.0, 200.0]));
            assert!(!in_resize_corner(&node, [365.0, 220.0]));
            // Противоположный угол — не resize
            assert!(!in_resize_corner(&node, [105.0, 105.0]));
        }

        /// effective_scale (R10): desktop DPI побеждает враньё scale_factor
        /// окна после репарентинга; без desktop DPI — scale_factor окна.
        #[test]
        fn effective_scale_prefers_desktop_dpi() {
            // dpi 120 = 125% — desktop-режим: берём его, а не 1.0 из winit
            let s = effective_scale(1.0, Some(120));
            assert!((s - 1.25).abs() < 1e-6);
            // dpi 96 = 100% — даже если winit врёт 1.25
            let s = effective_scale(1.25, Some(96));
            assert!((s - 1.0).abs() < 1e-6);
            // нет desktop DPI — оконный режим: scale_factor окна как есть
            let s = effective_scale(1.5, None);
            assert!((s - 1.5).abs() < 1e-6);
        }

        /// Кнопка настроек: rect в каждом из 4 углов viewport (панель настроек).
        #[test]
        fn settings_button_corners() {
            let viewport = [1600.0, 900.0];
            let tl = button_rect(Corner::TopLeft, viewport);
            assert_eq!(
                tl,
                [
                    SETTINGS_MARGIN,
                    SETTINGS_MARGIN,
                    SETTINGS_BUTTON,
                    SETTINGS_BUTTON
                ]
            );
            let tr = button_rect(Corner::TopRight, viewport);
            assert_eq!(tr[0], 1600.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            assert_eq!(tr[1], SETTINGS_MARGIN);
            let br = button_rect(Corner::BottomRight, viewport);
            assert_eq!(br[0], 1600.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            assert_eq!(br[1], 900.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            let bl = button_rect(Corner::BottomLeft, viewport);
            assert_eq!(bl[0], SETTINGS_MARGIN);
            assert_eq!(bl[1], 900.0 - SETTINGS_MARGIN - SETTINGS_BUTTON);
            // Точка кнопки попадает в hit-test, соседняя — нет
            assert!(point_in_rect(tr, [tr[0] + 2.0, tr[1] + 2.0]));
            assert!(!point_in_rect(tr, [tr[0] - 1.0, tr[1] + 2.0]));
        }

        /// Кнопка темы: тот же угол/ряд, что кнопка настроек, зазор
        /// SETTINGS_GAP, вся кнопка в viewport, пересечений с кнопкой нет.
        #[test]
        fn theme_button_next_to_settings_button() {
            let viewport = [1600.0, 900.0];
            for corner in [
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ] {
                let button = button_rect(corner, viewport);
                let theme = theme_button_rect(corner, viewport);
                // Та же строка по вертикали
                assert_eq!(theme[1], button[1]);
                assert_eq!(theme[3], SETTINGS_BUTTON);
                // Вся кнопка в viewport
                assert!(theme[0] >= 0.0 && theme[0] + theme[2] <= viewport[0]);
                // Зазор ровно SETTINGS_GAP, пересечения нет
                let gap = (theme[0] - (button[0] + button[2])).abs();
                let gap_left = (button[0] - (theme[0] + theme[2])).abs();
                assert!(
                    (gap - SETTINGS_GAP).abs() < 1e-3 || (gap_left - SETTINGS_GAP).abs() < 1e-3,
                    "зазор SETTINGS_GAP: theme={theme:?} button={button:?}"
                );
            }
            // Правый угол: тема левее настроек; левый — правее
            let tr_theme = theme_button_rect(Corner::TopRight, viewport);
            let tr = button_rect(Corner::TopRight, viewport);
            assert!(tr_theme[0] + tr_theme[2] < tr[0]);
            let tl_theme = theme_button_rect(Corner::TopLeft, viewport);
            let tl = button_rect(Corner::TopLeft, viewport);
            assert!(tl_theme[0] > tl[0] + tl[2]);
        }

        /// Панель настроек: прижата к углу кнопки, целиком в viewport.
        #[test]
        fn settings_panel_placement() {
            let viewport = [1600.0, 900.0];
            for corner in [
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ] {
                let panel = panel_rect(corner, viewport);
                assert!(
                    panel[0] >= 0.0 && panel[0] + panel[2] <= viewport[0],
                    "{corner:?}"
                );
                assert!(
                    panel[1] >= 0.0 && panel[1] + panel[3] <= viewport[1],
                    "{corner:?}"
                );
                let button = button_rect(corner, viewport);
                // Панель по горизонтали на той же стороне, что и кнопка
                let same_side = (panel[0] - button[0]).abs() < 1.0
                    || ((panel[0] + panel[2]) - (button[0] + button[2])).abs() < 1.0;
                assert!(same_side, "{corner:?}: панель не под кнопкой");
            }
        }

        /// Hit-test строк панели: строки кликабельны, заголовок/подсказка/паддинги — нет.
        #[test]
        fn settings_panel_row_hit_test() {
            let viewport = [1600.0, 900.0];
            let panel = panel_rect(Corner::TopRight, viewport);
            let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
            // Первая и последняя строки
            assert_eq!(
                panel_row_at(panel, [panel[0] + 20.0, rows_top + 3.0]),
                Some(0)
            );
            let last = SETTINGS_ROWS.len() - 1;
            let last_y = rows_top + last as f32 * PANEL_ROW_HEIGHT + 3.0;
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, last_y]), Some(last));
            // Заголовок и подсказка не кликабельны
            assert_eq!(
                panel_row_at(panel, [panel[0] + 20.0, panel[1] + PANEL_PADDING + 3.0]),
                None
            );
            let hint_y = rows_top + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT + 3.0;
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, hint_y]), None);
            // Мимо панели
            assert_eq!(panel_row_at(panel, [panel[0] - 5.0, last_y]), None);
            assert_eq!(panel_row_at(panel, [panel[0] + 20.0, panel[1] - 5.0]), None);
        }
    }
}
