//! canvas-app library — публичный API для интеграционных тестов.
//!
//! Реэкспортирует типы и функции, нужные для тестирования логики приложения
//! без создания окна winit.

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

/// Внутренние константы и функции для тестов (не часть публичного API приложения).
pub mod test_helpers {
    use super::*;
    use std::time::{Duration, Instant};

    pub const ZOOM_STEP_PER_LINE: f32 = 1.1;
    pub const PAN_PX_PER_LINE: f32 = 40.0;
    pub const AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(2);
    pub const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(500);
    pub const DOUBLE_CLICK_DIST: f64 = 5.0;

    pub const MENU_WIDTH: f32 = 170.0;
    pub const MENU_ITEM_HEIGHT: f32 = 26.0;
    pub const MENU_PADDING: f32 = 6.0;
    pub const MENU_LABEL_X: f32 = 26.0;
    pub const MENU_ITEMS: [Option<&str>; 7] = [
        Some("1"),
        Some("2"),
        Some("3"),
        Some("4"),
        Some("5"),
        Some("6"),
        None,
    ];
    pub const MENU_FILL: [f32; 4] = [0.11, 0.11, 0.13, 0.97];

    pub const MIN_NODE_WIDTH: f32 = 160.0;
    pub const MIN_NODE_HEIGHT: f32 = 64.0;
    pub const MAX_NOTE_WIDTH: f32 = 600.0;
    pub const RESIZE_HANDLE: f32 = 16.0;

    pub const SETTINGS_BUTTON: f32 = 36.0;
    pub const SETTINGS_MARGIN: f32 = 12.0;
    pub const SETTINGS_GAP: f32 = 8.0;
    pub const PANEL_WIDTH: f32 = 300.0;
    pub const PANEL_ROW_HEIGHT: f32 = 28.0;
    pub const PANEL_HEADER_HEIGHT: f32 = 30.0;
    pub const PANEL_HINT_HEIGHT: f32 = 24.0;
    pub const PANEL_PADDING: f32 = 10.0;

    pub const SETTINGS_ROWS: [SettingsRow; 3] = [
        SettingsRow::ButtonCorner,
        SettingsRow::Grid,
        SettingsRow::HudOnStart,
    ];

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SettingsRow {
        ButtonCorner,
        Grid,
        HudOnStart,
    }

    impl SettingsRow {
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

    pub fn point_in_rect(rect: [f32; 4], point: Vec2) -> bool {
        point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
    }

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

    pub fn panel_height() -> f32 {
        PANEL_PADDING * 2.0
            + PANEL_HEADER_HEIGHT
            + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT
            + PANEL_HINT_HEIGHT
    }

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

    pub fn in_resize_corner(node: &Node, point: Vec2) -> bool {
        let right = node.x + node.width;
        let bottom = node.y + node.height;
        point[0] >= right - RESIZE_HANDLE
            && point[0] <= right
            && point[1] >= bottom - RESIZE_HANDLE
            && point[1] <= bottom
    }

    pub struct ContextMenu {
        pub node: usize,
        pub origin: Vec2,
    }

    pub struct EdgeDrag {
        pub from_node: String,
        pub from_side: Side,
    }

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

    pub fn menu_item_rect(origin: Vec2, i: usize) -> [f32; 4] {
        [
            origin[0] + MENU_PADDING,
            origin[1] + MENU_PADDING + i as f32 * MENU_ITEM_HEIGHT,
            MENU_WIDTH - MENU_PADDING * 2.0,
            MENU_ITEM_HEIGHT,
        ]
    }

    pub fn menu_rect(origin: Vec2) -> [f32; 4] {
        [
            origin[0],
            origin[1],
            MENU_WIDTH,
            MENU_PADDING * 2.0 + MENU_ITEMS.len() as f32 * MENU_ITEM_HEIGHT,
        ]
    }

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

    pub fn menu_label(item: Option<&str>) -> String {
        match item {
            Some(preset) => format!("Цвет {preset}"),
            None => "Без цвета".to_owned(),
        }
    }
}
