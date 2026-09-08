//! canvas-app library — общая логика приложения.
//!
//! Переэкспортирует типы и функции, нужные интеграционным тестам без окна
//! winit, и содержит модуль `ui` — чистые функции интерфейса (геометрия
//! оверлеев, hit-тесты, палитра контекстного меню, детектор двойного
//! клика), общий для бинаря (`main.rs`) и тестов. Раньше эти функции
//! дублировались в `main.rs` копипастой — теперь источник один.

pub use canvas_core::{
    edge_at, nearest_side, port_at, port_point, Canvas, Corner, Edge, Node, NodeKind, Settings,
    Side, SpatialIndex,
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

    /// Строки панели настроек (порядок = порядок отображения).
    pub const SETTINGS_ROWS: [SettingsRow; 3] = [
        SettingsRow::ButtonCorner,
        SettingsRow::Grid,
        SettingsRow::HudOnStart,
    ];

    /// Строка-переключатель панели настроек.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SettingsRow {
        /// Угол летающей кнопки (цикл по 4 углам).
        ButtonCorner,
        /// Сетка канваса вкл/выкл.
        Grid,
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
    pub struct ContextMenu {
        pub node: usize,
        pub origin: Vec2,
    }

    /// Активный drag резиновой линии новой связи (T8): от порта ноды к курсору.
    pub struct EdgeDrag {
        pub from_node: String,
        pub from_side: Side,
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

    /// Первый свободный id заметки вида `note-N` (T7).
    pub fn next_note_id(canvas: &Canvas) -> String {
        let mut n = 1u32;
        while canvas
            .nodes
            .iter()
            .any(|node| node.id == format!("note-{n}"))
        {
            n += 1;
        }
        format!("note-{n}")
    }

    /// Rect пункта меню в world-координатах: [x, y, w, h].
    pub fn menu_item_rect(origin: Vec2, i: usize) -> [f32; 4] {
        [
            origin[0] + MENU_PADDING,
            origin[1] + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT,
            MENU_WIDTH - MENU_PADDING * 2.0,
            MENU_ITEM_HEIGHT,
        ]
    }

    /// Полный rect меню: [x, y, w, h].
    pub fn menu_rect(origin: Vec2) -> [f32; 4] {
        [
            origin[0],
            origin[1],
            MENU_WIDTH,
            MENU_PADDING * 2.0 + MENU_ITEMS.len() as f32 * MENU_ITEM_HEIGHT,
        ]
    }

    /// Hit-test пункта меню по world-точке (T7).
    pub fn menu_item_at(origin: Vec2, point: Vec2) -> Option<usize> {
        let [x, y, w, h] = menu_rect(origin);
        if point[0] < x
            || point[0] > x + w
            || point[1] < y + MENU_PADDING
            || point[1] > y + h - MENU_PADDING
        {
            return None;
        }
        let i = ((point[1] - y - MENU_PADDING) / MENU_ITEM_HEIGHT) as usize;
        (i < MENU_ITEMS.len()).then_some(i)
    }

    /// Подпись пункта меню.
    pub fn menu_label(item: Option<&str>) -> String {
        match item {
            Some(preset) => format!("Цвет {preset}"),
            None => "Без цвета".to_owned(),
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

        /// Генератор id заметок (T7): первый свободный note-N.
        #[test]
        fn note_id_first_free() {
            let canvas = Canvas::default();
            assert_eq!(next_note_id(&canvas), "note-1");
            let mut canvas = Canvas::default();
            canvas.nodes.push(Node::text("note-1", "", 0.0, 0.0));
            assert_eq!(next_note_id(&canvas), "note-2");
            canvas.nodes.push(Node::text("note-2", "", 0.0, 0.0));
            assert_eq!(next_note_id(&canvas), "note-3");
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
