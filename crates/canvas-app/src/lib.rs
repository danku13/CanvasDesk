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

/// Менеджер виджетов (M5 T20-F): реестр + LOD + host-обёртки. Модуль
/// кроссплатформен (host — cfg(windows) внутри), юнит-тесты — на Linux.
pub mod widgets;

/// Палитра выделения (FR-009/FR-010): screen-space тулбар под выделением —
/// группы настроек с выпадающими перечнями и иконками. Заменяет текстовые
/// контекстные меню ноды/связи.
pub mod palette;

/// UI шаблонов (FR-018): боковая палитра (Ctrl+P) и радиальное wheel-меню
/// (Shift+клик по пустому месту). Чистая логика — рендер в main.rs.
pub mod template_ui;

/// FR-021: контекстные подсказки при Numi-вводе — чистая модель
/// (токен/фильтрация каталога движка/popup). Рендер и перехват клавиш —
/// в main.rs, паттерн wheel-оверлея.
pub mod hints_ui;

/// FR-026: панель настроек — группы и выпадающие меню — чистая модель
/// (группы/род строки/перечень значений/геометрия с клампом/hit-тесты).
/// Рендер и ввод — в main.rs; `ui::panel_rect` переиспользует высоту.
pub mod settings_ui;

/// Чистая UI-логика приложения: геометрия оверлеев (контекстное меню,
/// панель настроек), hit-тесты, генератор id заметок, детектор двойного
/// клика. Не зависит от окна и GPU — используется бинарём и тестами.
pub mod ui {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    /// Ширина контекстного меню в логических px (T7; screen-space —
    /// константный размер при любом зуме).
    pub const MENU_WIDTH: f32 = 170.0;
    /// Высота пункта меню в логических px.
    pub const MENU_ITEM_HEIGHT: f32 = 26.0;
    /// Внутренний отступ меню в логических px.
    pub const MENU_PADDING: f32 = 6.0;
    /// Сдвиг подписи пункта: слева место под образец цвета.
    pub const MENU_LABEL_X: f32 = 26.0;
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

    /// Rect панели настроек: прижата к кнопке (с зазором), в том же углу.
    /// Высота — из `settings_ui::panel_height` (FR-026: группы + отступы).
    pub fn panel_rect(corner: Corner, viewport: Vec2) -> [f32; 4] {
        let height = crate::settings_ui::panel_height();
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

    /// Точка в зоне resize (правый нижний угол ноды)? Чистая функция для тестов.
    pub fn in_resize_corner(node: &Node, point: Vec2) -> bool {
        let right = node.x + node.width;
        let bottom = node.y + node.height;
        point[0] >= right - RESIZE_HANDLE
            && point[0] <= right
            && point[1] >= bottom - RESIZE_HANDLE
            && point[1] <= bottom
    }

    /// Контекстное меню пустого канваса (ПКМ мимо нод/связей): создание
    /// группы, фокус, хоткеи, подменю «Виджеты ▸». Screen-space: origin —
    /// логические px от угла окна, размер константен при любом зуме
    /// (уточнение владельца). Меню ноды/связи заменены палитрой выделения
    /// (модуль `palette`).
    pub struct ContextMenu {
        /// Позиция (логические px) левого верхнего угла меню.
        pub origin: Vec2,
        /// Подменю «Виджеты ▸» (T20-F): открывается кликом по пункту Widgets
        /// базового меню; пусто/None — подменю не открыто.
        pub submenu: Option<Submenu>,
    }

    /// Подменю контекстного меню (T20-F): список пакетов виджетов.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Submenu {
        /// Позиция (логические px) левого верхнего угла колонки подменю.
        pub origin: Vec2,
        /// Пункты: установка ноды виджета или удаление пакета (T21-C).
        pub entries: Vec<SubmenuEntry>,
    }

    /// Действие пункта подменю виджетов (T21-C: управление пакетами).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SubmenuAction {
        /// Добавить ноду виджета на канвас (id пакета).
        Insert(String),
        /// Удалить пакет (id пакета; с диалогом подтверждения П11).
        Remove(String),
    }

    /// Пункт подменю виджетов.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct SubmenuEntry {
        /// Действие пункта.
        pub action: SubmenuAction,
        /// Подпись (имя пакета из манифеста).
        pub label: String,
    }

    /// FR-009: настройка/действие ноды из палитры выделения (модуль
    /// `palette`). Состав зависит от типа ноды; авто-раскладка FR-010 —
    /// отдельное действие палитры (`PaletteAction::Layout`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum NodeSetting {
        // FR-009: общие
        Rename,
        Duplicate,
        // FR-011: mindmap
        AddChild,
        AddSibling,
        CollapseBranch,
        ExpandBranch,
        // FR-009: по типам
        OpenFile,
        OpenFolder,
        CopyPath,
        ClearText,
        Ungroup,
        WidgetReload,
        WidgetPermissions,
    }

    /// Активный drag резиновой линии (T8 + CR-002): от порта ноды к курсору —
    /// новая связь; или перепривязка конца существующей связи.
    pub enum EdgeDrag {
        /// Новая связь: тянем от порта `from_node` (T8). FR-014: `value_flow`
        /// — Shift+drag создаёт value-ребро (поток значений); обычный drag —
        /// контрольную связь (дефолт, обратная совместимость).
        New {
            from_node: String,
            from_side: Side,
            value_flow: bool,
        },
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
                    ..
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
        ("Ctrl+P", "палитра шаблонов"),
        ("Shift+клик (пусто)", "wheel-меню шаблонов"),
        ("F3", "HUD / следующий результат"),
        ("Esc", "закрыть меню и панели"),
        ("Del", "удалить выделенное"),
        ("Ctrl+Z", "отменить действие"),
        ("Ctrl+Y", "вернуть отменённое"),
        ("Ctrl+C", "копировать ноды"),
        ("Ctrl+X", "вырезать ноды"),
        ("Ctrl+V", "вставить ноды"),
        ("Ctrl+D", "дублировать ноды"),
        ("Ctrl+G", "сгруппировать выделенное"),
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
            // Виджет-копия (M5): общий префикс «widget-» — как у созданных
            // из меню (менеджер ведёт свой счётчик next_node_id)
            NodeKind::Widget => "widget",
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
    /// T21-B добавляет установку виджет-пакета drag-ом папки.
    #[derive(Debug, Clone, PartialEq)]
    pub enum DropInsertKind {
        /// Файловая нода (путь как дал Explorer, абсолютный).
        File(PathBuf),
        /// Текстовая нода: URL или произвольный текст (критерий T9 — URL
        /// становится заметкой с текстом ссылки; Plain-текст — бонус).
        Note(String),
        /// Установка виджет-пакета (T21-B): папка с валидным widget.json;
        /// после Drop — диалог подтверждения, затем install + нода
        /// (источник-папка, имя пакета из манифеста).
        InstallWidget(PathBuf, String),
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

    /// T21-B: классификация дропа — папка виджет-пакета? Дроп считается
    /// установкой пакета, когда в нём РОВНО одна папка с `widget.json`
    /// в корне (мультидроп с папками/файлами идёт прежним путём — файлы).
    /// Чистая функция (только fs-проверки), вызывается до plan_drop.
    pub fn dropped_widget_package(
        data: &canvas_shell::dragdrop::DragData,
    ) -> Option<std::path::PathBuf> {
        let canvas_shell::dragdrop::DragData::HdropBytes(bytes) = data else {
            return None;
        };
        let paths = parse_hdrop_bytes(bytes);
        let [single] = paths.as_slice() else {
            return None;
        };
        (single.is_dir() && single.join("widget.json").is_file()).then(|| single.to_path_buf())
    }

    /// Максимальная длина подписи призрака дропа (в символах) — длинные имена
    /// файлов обрезаются многоточием, чтобы не вылезать за призрак карточки.
    pub const DROP_GHOST_LABEL_MAX: usize = 40;

    /// Подпись призрака дропа (Т9): имя файла из пути / первая строка заметки.
    /// Чистая функция — тестируется без GPU; вызывается из оверлей-прохода
    /// для каждого пункта плана при перетаскивании.
    pub fn drop_ghost_label(kind: &DropInsertKind) -> String {
        let raw = match kind {
            DropInsertKind::File(path) => {
                // Имя файла из пути с разделителями любого стиля: CF_HDROP
                // даёт Windows-пути (обратный слеш), а функция работает и на
                // Linux-сборке (тесты, планировщик дропа) — Path::file_name()
                // распознаёт только нативный разделитель (на Linux вернул бы
                // весь «C:\…\SPEC.md» как один компонент), поэтому делим
                // строку по обоим разделителям и берём последний непустой.
                let s = path.to_string_lossy();
                s.rsplit(['\\', '/'])
                    .find(|part| !part.is_empty())
                    .unwrap_or_default()
                    .to_string()
            }
            DropInsertKind::Note(text) => text.lines().next().unwrap_or("").to_owned(),
            DropInsertKind::InstallWidget(_, name) => format!("Установить виджет: {name}"),
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

    /// Rect пункта меню в логических px (screen-space): [x, y, w, h].
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

    /// Hit-test пункта меню из `items` по точке в логических px.
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
        /// M5 (T20-F): «Виджеты ▸» — подменю установки виджет-нод
        /// (список пакетов реестра).
        Widgets,
        /// T15: «Режим десктопа» — переключатель встройки канваса в рабочий
        /// стол (WorkerW/Progman). На не-Windows — пункт скрыт. Галочка ✓ —
        /// встройка активна; выбор снимает встройку (detach + восстановление
        /// иконок + очистка состояния монитора).
        DesktopMode,
    }

    /// Меню пустого канваса.
    pub const CANVAS_MENU_ITEMS: [CanvasMenuItem; 5] = [
        CanvasMenuItem::NewGroup,
        CanvasMenuItem::FocusMode,
        CanvasMenuItem::Hotkeys,
        CanvasMenuItem::Widgets,
        CanvasMenuItem::DesktopMode,
    ];

    /// Подпись пункта меню пустого канваса. `focus_on` — состояние режима
    /// фокуса, `hotkeys_open` — состояние оверлея хоткеев, `desktop_on` —
    /// состояние desktop-встройки: для пунктов-переключателей рисуется
    /// ✓-галочка (FR-004.1 — Hotkeys, T15 — DesktopMode).
    pub fn canvas_menu_label(
        item: CanvasMenuItem,
        focus_on: bool,
        hotkeys_open: bool,
        desktop_on: bool,
    ) -> String {
        match item {
            CanvasMenuItem::NewGroup => "Создать группу".to_owned(),
            CanvasMenuItem::FocusMode => {
                format!("{}Фокус на связях", if focus_on { "✓ " } else { "" })
            }
            CanvasMenuItem::Hotkeys => format!(
                "{}Горячие клавиши (F1)",
                if hotkeys_open { "✓ " } else { "" }
            ),
            CanvasMenuItem::Widgets => "Виджеты ▸…".to_owned(),
            CanvasMenuItem::DesktopMode => {
                format!("{}Режим десктопа", if desktop_on { "✓ " } else { "" })
            }
        }
    }

    /// Rect колонки подменю (логические px): [x, y, w, h]. Высота — по числу
    /// пунктов (пустой список — 1 строка «(нет установленных)»).
    pub fn submenu_rect(submenu: &Submenu) -> [f32; 4] {
        menu_rect_for(submenu.origin, submenu.entries.len().max(1))
    }

    /// Hit-test пункта подменю по точке в логических px.
    pub fn submenu_item_at(submenu: &Submenu, point: Vec2) -> Option<usize> {
        menu_item_at_for(submenu.origin, point, submenu.entries.len().max(1))
            .filter(|&i| i < submenu.entries.len())
    }

    /// Origin подменю справа от базового меню: колонка со сдвигом на ширину.
    pub fn submenu_origin_next_to(menu_origin: Vec2) -> Vec2 {
        [menu_origin[0] + MENU_WIDTH + 2.0, menu_origin[1]]
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
    /// сторонам, подпись по умолчанию, id `group-N`. FR-012: группа создаётся
    /// с ЯВНЫМ списком детей (оборачиваемая нода) — случайное перекрытие
    /// после создания membership не меняет. Канвас не мутируется —
    /// вставку делает приложение (паттерн plan_drop, T9).
    pub fn plan_group_around(canvas: &Canvas, index: usize, padding: f32) -> Option<Node> {
        plan_group_around_nodes(canvas, &[index], padding)
    }

    /// План группы вокруг НЕСКОЛЬКИХ нод (Ctrl+G): bbox = объединение rect'ов
    /// всех указанных нод + padding по всем сторонам, подпись по умолчанию,
    /// id `group-N`. FR-012: группа создаётся с ЯВНЫМ списком детей (id всех
    /// обёрнутых нод, порядок = порядок индексов) — случайное перекрытие
    /// после создания membership не меняет. Несуществующие индексы
    /// пропускаются; пустой набор (все мимо) — None. Канвас не мутируется —
    /// вставку делает приложение (паттерн plan_drop, T9).
    pub fn plan_group_around_nodes(
        canvas: &Canvas,
        indices: &[usize],
        padding: f32,
    ) -> Option<Node> {
        let wrapped: Vec<Node> = indices
            .iter()
            .filter_map(|&index| canvas.nodes.get(index))
            .cloned()
            .collect();
        if wrapped.is_empty() {
            return None;
        }
        let [bx, by, bw, bh] = nodes_bbox(&wrapped);
        let mut group = Node::group(
            next_free_id(canvas, "group"),
            bx - padding,
            by - padding,
            bw + padding * 2.0,
            bh + padding * 2.0,
        );
        group.label = Some(GROUP_DEFAULT_LABEL.to_owned());
        group.children = Some(wrapped.iter().map(|node| node.id.clone()).collect());
        Some(group)
    }

    /// План «Создать группу»: группа GROUP_WIDTH × GROUP_HEIGHT с центром
    /// в world-точке (центр viewport), подпись по умолчанию, id `group-N`.
    /// FR-012: явный (пустой) список детей — «случайное» перекрытие не
    /// подвязывает ноды.
    pub fn plan_group_at(canvas: &Canvas, center: Vec2) -> Node {
        let mut group = Node::group(
            next_free_id(canvas, "group"),
            center[0] - GROUP_WIDTH / 2.0,
            center[1] - GROUP_HEIGHT / 2.0,
            GROUP_WIDTH,
            GROUP_HEIGHT,
        );
        group.label = Some(GROUP_DEFAULT_LABEL.to_owned());
        group.children = Some(Vec::new());
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
            // Новая связь от right-порта a (контрольная — дефолт drag)
            let drag = EdgeDrag::New {
                from_node: "a".to_owned(),
                from_side: Side::Right,
                value_flow: false,
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
                value_flow: true,
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
            // FR-006/FR-007: новые операции в списке (undo/redo/cut)
            let keys: Vec<&str> = HOTKEYS.iter().map(|(key, _)| *key).collect();
            for required in ["Ctrl+Z", "Ctrl+Y", "Ctrl+X"] {
                assert!(keys.contains(&required), "в списке нет {required}");
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

        /// T21-B: классификация дропа — только одиночная папка с widget.json
        /// в корне считается пакетом виджета; файлы/мультидроп/текст — нет.
        #[test]
        fn dropped_widget_package_classification() {
            let dir = temp_dir("pkgdrop");
            // Папка-пакет с манифестом
            let pkg = dir.join("my-widget");
            std::fs::create_dir_all(&pkg).unwrap();
            std::fs::write(
                pkg.join("widget.json"),
                r#"{"id":"x.test","name":"T","version":"1","entry":"index.html"}"#,
            )
            .unwrap();
            let pkg_s = pkg.to_string_lossy().into_owned();
            assert_eq!(
                dropped_widget_package(&canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[
                    &pkg_s
                ]))),
                Some(pkg.clone())
            );
            // Папка БЕЗ манифеста — не пакет (обычная папка файлов)
            let plain = dir.join("plain-dir");
            std::fs::create_dir_all(&plain).unwrap();
            let plain_s = plain.to_string_lossy().into_owned();
            assert_eq!(
                dropped_widget_package(&canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[
                    &plain_s
                ]))),
                None
            );
            // Файл (даже widget.json напрямую) — не пакет
            let file = pkg.join("widget.json");
            let file_s = file.to_string_lossy().into_owned();
            assert_eq!(
                dropped_widget_package(&canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[
                    &file_s
                ]))),
                None
            );
            // Две папки с манифестами — не установка (неоднозначно)
            let pkg2 = dir.join("my-widget-2");
            std::fs::create_dir_all(&pkg2).unwrap();
            std::fs::write(
                pkg2.join("widget.json"),
                r#"{"id":"y.test","name":"T","version":"1","entry":"index.html"}"#,
            )
            .unwrap();
            let pkg2_s = pkg2.to_string_lossy().into_owned();
            assert_eq!(
                dropped_widget_package(&canvas_shell::dragdrop::DragData::HdropBytes(hdrop(&[
                    &pkg_s, &pkg2_s
                ]))),
                None
            );
            // Текст — не пакет
            assert_eq!(
                dropped_widget_package(&canvas_shell::dragdrop::DragData::Text(
                    "привет".to_owned()
                )),
                None
            );
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

        /// M5 (T20-F): подменю «Виджеты ▸» — геометрия колонки справа от
        /// базового меню, hit-test пунктов, пустой список, origin-хелпер.
        #[test]
        fn widgets_submenu_geometry_and_hit_test() {
            let menu_origin: Vec2 = [100.0, 50.0];
            let sub_origin = submenu_origin_next_to(menu_origin);
            // Колонка начинается за шириной меню (+2 px зазор)
            assert_eq!(sub_origin[0], menu_origin[0] + MENU_WIDTH + 2.0);
            assert_eq!(sub_origin[1], menu_origin[1]);

            let entries = vec![
                SubmenuEntry {
                    action: SubmenuAction::Insert("com.canvasdesk.clock".to_owned()),
                    label: "Clock".to_owned(),
                },
                SubmenuEntry {
                    action: SubmenuAction::Insert("com.canvasdesk.calendar".to_owned()),
                    label: "Calendar".to_owned(),
                },
                SubmenuEntry {
                    action: SubmenuAction::Remove("com.canvasdesk.clock".to_owned()),
                    label: "— Удалить: Clock".to_owned(),
                },
            ];
            let submenu = Submenu {
                origin: sub_origin,
                entries,
            };
            let [_, y, w, h] = submenu_rect(&submenu);
            assert_eq!(w, MENU_WIDTH);
            // 3 пункта (вставка ×2 + удаление T21-C) — высота по всем
            assert_eq!(h, MENU_PADDING * 2.0 + 3.0 * MENU_ITEM_HEIGHT);
            // Пункт 0/1 накликиваются, мимо — None
            assert_eq!(
                submenu_item_at(&submenu, [sub_origin[0] + 10.0, y + MENU_PADDING + 3.0]),
                Some(0)
            );
            assert_eq!(
                submenu_item_at(
                    &submenu,
                    [
                        sub_origin[0] + 10.0,
                        y + MENU_PADDING + MENU_ITEM_HEIGHT + 3.0
                    ]
                ),
                Some(1)
            );
            assert_eq!(
                submenu_item_at(
                    &submenu,
                    [
                        sub_origin[0] + 10.0,
                        y + MENU_PADDING + 2.0 * MENU_ITEM_HEIGHT + 3.0
                    ]
                ),
                Some(2)
            );
            assert_eq!(submenu_item_at(&submenu, [500.0, 500.0]), None);
            // Пустое подменю: rect не вырожден (1 строка), хит-тест — None
            let empty = Submenu {
                origin: sub_origin,
                entries: Vec::new(),
            };
            assert_eq!(
                submenu_rect(&empty)[3],
                MENU_PADDING * 2.0 + MENU_ITEM_HEIGHT
            );
            assert_eq!(
                submenu_item_at(&empty, [sub_origin[0] + 10.0, y + 20.0]),
                None
            );
            // Клик по колонке подменю НЕ попадает в базовое меню (правее)
            assert_eq!(
                menu_item_at_for(menu_origin, [sub_origin[0] + 10.0, y + 20.0], 4),
                None
            );
        }

        /// M5: подменю в ContextMenu — тогл через Widgets-пункт открывает,
        /// повторный ПКМ закрывает всё меню (submenu не переживает take).
        #[test]
        fn context_menu_with_submenu_composes() {
            let menu = ContextMenu {
                origin: [0.0, 0.0],
                submenu: Some(Submenu {
                    origin: submenu_origin_next_to([0.0, 0.0]),
                    entries: vec![SubmenuEntry {
                        action: SubmenuAction::Insert("com.canvasdesk.clock".to_owned()),
                        label: "Clock".to_owned(),
                    }],
                }),
            };
            assert!(menu.submenu.is_some());
            assert_eq!(menu.submenu.as_ref().unwrap().entries.len(), 1);
            // Дефолтное меню — без подменю
            let plain = ContextMenu {
                origin: [0.0, 0.0],
                submenu: None,
            };
            assert!(plain.submenu.is_none());
        }

        /// Меню пустого канваса: «Создать группу» + «Фокус на связях» (T23)
        /// + «Горячие клавиши (F1)» (FR-004.1).
        #[test]
        fn canvas_menu_single_item() {
            let origin = [100.0, 50.0];
            let n = CANVAS_MENU_ITEMS.len();
            assert_eq!(n, 5);
            // M5 (T20-F): четвёртый пункт — вход в подменю виджетов
            assert_eq!(CANVAS_MENU_ITEMS[3], CanvasMenuItem::Widgets);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[3], false, false, false),
                "Виджеты ▸…"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[0], false, false, false),
                "Создать группу"
            );
            // T23: второй пункт — переключатель фокуса с ✓-галочкой
            assert_eq!(CANVAS_MENU_ITEMS[1], CanvasMenuItem::FocusMode);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[1], true, false, false),
                "✓ Фокус на связях"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[1], false, false, false),
                "Фокус на связях"
            );
            // FR-004.1: третий пункт — переключатель оверлея хоткеев
            assert_eq!(CANVAS_MENU_ITEMS[2], CanvasMenuItem::Hotkeys);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[2], false, true, false),
                "✓ Горячие клавиши (F1)"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[2], false, false, false),
                "Горячие клавиши (F1)"
            );
            // T15: пятый пункт — переключатель desktop-режима с ✓-галочкой
            assert_eq!(CANVAS_MENU_ITEMS[4], CanvasMenuItem::DesktopMode);
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[4], false, false, true),
                "✓ Режим десктопа"
            );
            assert_eq!(
                canvas_menu_label(CANVAS_MENU_ITEMS[4], false, false, false),
                "Режим десктопа"
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

        /// Ctrl+G: план группы вокруг набора нод — bbox по всем + padding,
        /// дети — явный список id выделенных (FR-012).
        #[test]
        fn plan_group_around_nodes_bbox() {
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 80.0));
            canvas
                .nodes
                .push(Node::file("b", "C:/b.png", 300.0, 200.0, 100.0, 100.0));
            // Третья нода — вне выделения
            canvas
                .nodes
                .push(Node::file("c", "C:/c.png", 1000.0, 1000.0, 50.0, 50.0));

            let group =
                plan_group_around_nodes(&canvas, &[0, 1], GROUP_PADDING).expect("ноды есть");
            assert_eq!(group.kind(), NodeKind::Group);
            assert_eq!(group.id, "group-1");
            assert_eq!(group.label.as_deref(), Some(GROUP_DEFAULT_LABEL));
            // bbox набора [0,0..400,300] + padding 40 по всем сторонам
            assert_eq!(
                (group.x, group.y, group.width, group.height),
                (
                    0.0 - GROUP_PADDING,
                    0.0 - GROUP_PADDING,
                    400.0 + GROUP_PADDING * 2.0,
                    300.0 + GROUP_PADDING * 2.0
                )
            );
            // FR-012: дети — ЯВНЫЙ список id выделенных, порядок = индексы
            assert_eq!(group.children, Some(vec!["a".to_owned(), "b".to_owned()]));
            // Канвас не мутирован планом
            assert_eq!(canvas.nodes.len(), 3);
        }

        /// Ctrl+G: пустой набор и индексы мимо — None; невалидные индексы
        /// пропускаются; один индекс — эквивалент plan_group_around.
        #[test]
        fn plan_group_around_nodes_empty_and_gaps() {
            let mut canvas = Canvas::default();
            canvas
                .nodes
                .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 80.0));
            // Пустой набор — None
            assert!(plan_group_around_nodes(&canvas, &[], GROUP_PADDING).is_none());
            // Все индексы мимо — None
            assert!(plan_group_around_nodes(&canvas, &[7, 9], GROUP_PADDING).is_none());
            // Несуществующие индексы пропускаются: [5, 0] → группа вокруг «a»
            let group =
                plan_group_around_nodes(&canvas, &[5, 0], GROUP_PADDING).expect("валидный есть");
            assert_eq!(group.children, Some(vec!["a".to_owned()]));
            // Один индекс — эквивалент plan_group_around
            let single = plan_group_around(&canvas, 0, GROUP_PADDING).expect("нода есть");
            assert_eq!(
                (group.x, group.y, group.width, group.height),
                (single.x, single.y, single.width, single.height)
            );
            assert_eq!(group.id, single.id);
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

        // FR-026: hit-тесты строк панели настроек переехали в
        // settings_ui (row_at по layout с группами)
    }
}
