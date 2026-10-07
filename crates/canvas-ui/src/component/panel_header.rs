//! FR-UI-PANEL-HEADER: panel header with multiple aligned buttons.
//!
//! Closes TODO K3/K4 — `kit_ui::gallery_layout` and
//! `admin_ui::admin_hit_slots` need close + theme + reset in one
//! coherent row (the kit's [`stage_close_button`](crate::component::button::stage_close_button)
//! puts close alone in the corner; theme/reset are wide toggle buttons
//! that must stay aligned with close on the same y).
//!
//! Канон (соответствует `kit_ui.rs::gallery_layout` ~398 и
//! `admin_ui.rs::admin_hit_slots` ~167):
//! - Кнопки выстраиваются справа-налево от `slot.right() - SPACING_SM`;
//! - Зазор между кнопками = `GAP_CONTROLS`;
//! - Icon-кнопки — квадрат `ICON_BUTTON_SIZE × ICON_BUTTON_SIZE`,
//!   вертикально центрированы в полосе `BUTTON_HEIGHT` (слоте шапки);
//! - Toggle-кнопки — `label_w × BUTTON_HEIGHT` (широкие toggle-кнопки
//!   темы/сброса — не квадратные icon-button);
//! - Сепаратор — горизонтальная линия 1px под шапкой (на `slot.y +
//!   BUTTON_HEIGHT`), цвет = `palette.panel_border`.
//!
//! Контракт F-8 (как у остальных kit-компонентов): кит НЕ выбирает
//! цвет/текст — только геометрия. Стиль кнопок — [`crate::component::button::button_style`]
//! (вариант + состояние) от потребителя; глиф иконки — [`crate::component::button::icon_glyph`].

use crate::component::{KitPalette, BUTTON_HEIGHT, GAP_CONTROLS, ICON_BUTTON_SIZE};
use crate::geometry::UiRect;
use crate::paint::PaintItem;

/// Header button spec: either an icon button (×, gear) or a wide toggle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderButton {
    /// Square icon button (close, settings) — uses [`ICON_BUTTON_SIZE`].
    Icon {
        /// Semantic kind — consumer maps to glyph via
        /// [`crate::component::button::icon_glyph`] (или SVG-атлас).
        kind: IconKind,
        /// Stable string id (e.g. "close", "theme") — consumer maps
        /// rect back to click handler by id (как у `kit_ui::HitSlot`).
        id: &'static str,
    },
    /// Wide toggle button (theme, reset) — fixed width, label text
    /// (label не входит в геометрию: потребитель измеряет подпись
    /// `TextMeasurer`-ом отдельно и передаёт итоговую `label_w`).
    Toggle {
        /// Измеренная ширина подписи + `2·BUTTON_PAD_H` (потребитель
        /// измеряет через [`crate::component::button::button_size`] и
        /// передаёт сюда `w`).
        label_w: f32,
        /// Stable string id (e.g. "theme", "reset").
        id: &'static str,
    },
}

impl HeaderButton {
    /// Ширина кнопки (расчёт — единая точка для layout-цикла).
    pub fn width(&self) -> f32 {
        match self {
            HeaderButton::Icon { .. } => ICON_BUTTON_SIZE,
            HeaderButton::Toggle { label_w, .. } => *label_w,
        }
    }
    /// Высота кнопки (Icon — `ICON_BUTTON_SIZE`, Toggle — `BUTTON_HEIGHT`).
    pub fn height(&self) -> f32 {
        match self {
            HeaderButton::Icon { .. } => ICON_BUTTON_SIZE,
            HeaderButton::Toggle { .. } => BUTTON_HEIGHT,
        }
    }
    /// Stable id (для маппинга rect → обработчик клика у потребителя).
    pub fn id(&self) -> &'static str {
        match self {
            HeaderButton::Icon { id, .. } | HeaderButton::Toggle { id, .. } => id,
        }
    }
}

/// Семантический вид иконки в шапке — потребитель маппит в глиф/SVG
/// (соответствует [`crate::component::button::Icon`] по набору базовых
/// иконок хедера; `Custom` — для специфичных иконок, не входящих в kit
/// enum — потребитель рисует глиф отдельно через `Painter::icon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    /// Закрыть (×) — обычно в правом краю шапки.
    Close,
    /// Настройки/шестерёнка (⚙).
    Settings,
    /// Поиск (⌕).
    Search,
    /// Custom icon — consumer draws glyph separately via
    /// [`crate::paint::Painter::icon`].
    Custom,
}

/// Раскладка шапки панели: rect шапки + упорядоченный (слева-направо)
/// список rect'ов кнопок с их id.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelHeaderLayout {
    /// Rect шапки (slot по X, `BUTTON_HEIGHT` по Y) — для хит-теста и
    /// отрисовки фона шапки потребителем.
    pub rect: UiRect,
    /// Button rects in left-to-right order, with their IDs
    /// (для потребителей с двусторонним порядком — reverse на 1 строку).
    pub buttons: Vec<(UiRect, &'static str)>,
}

/// Стиль сепаратора шапки: 1px горизонтальная линия под шапкой,
/// цвет = `palette.panel_border`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelHeaderStyle {
    /// Цвет сепаратора — слот `panel_border` (та же рамка панели).
    pub separator_color: [f32; 4],
    /// Y-координата сепаратора (`slot.y + BUTTON_HEIGHT`).
    pub separator_y: f32,
}

/// Раскладка шапки панели в `slot` с рядом кнопок, прижатых к правому
/// краю.
///
/// Кнопки выстраиваются справа-налево от `slot.right() - SPACING_SM` с
/// зазором `GAP_CONTROLS` между соседями. Icon-кнопки вертикально
/// центрированы в полосе `BUTTON_HEIGHT` (как `kit_ui`/`admin_ui`:
/// `(30 - 26) / 2 = 2px` отступ сверху — square icon visual-центрирован
/// в taller header bar). Toggle-кнопки занимают полную высоту
/// `BUTTON_HEIGHT`.
///
/// Сепаратор — горизонтальная линия 1px на `slot.y + BUTTON_HEIGHT`,
/// цвет = `palette.panel_border`.
///
/// Возвращает `(layout, style)` — потребитель рисует кнопки через
/// `Painter + button_style` отдельно (как у [`crate::component::footer::footer_buttons`]),
/// сепаратор — через [`paint_panel_separator`].
///
/// Переполнение НЕ маскируется: если сумма ширин кнопок + зазоров +
/// inset'ов превышает `slot.w`, левые кнопки выходят за `slot.x` (как у
/// [`crate::component::footer::footer_buttons`]) — ловится G4-линтом,
/// потребитель обязан обеспечить достаточную ширину слота.
pub fn panel_header(
    slot: UiRect,
    buttons: &[HeaderButton],
    palette: &KitPalette,
) -> (PanelHeaderLayout, PanelHeaderStyle) {
    let inset = canvas_core::tokens::SPACING_SM;
    let gap = GAP_CONTROLS;
    let header_h = BUTTON_HEIGHT;
    let n = buttons.len();
    let mut ordered: Vec<(UiRect, &'static str)> = Vec::with_capacity(n);
    // Справа-налево: последняя кнопка — flush к `slot.right() - inset`;
    // каждая предыдущая — на `gap` левее предыдущей.
    let mut x_right = slot.right() - inset;
    for b in buttons.iter().rev() {
        let w = b.width();
        let h_i = b.height();
        // Вертикально центрированы в полосе BUTTON_HEIGHT: Icon — (header_h - icon_size)/2,
        // Toggle — 0 (h_i == header_h).
        let y = slot.y + (header_h - h_i).max(0.0) / 2.0;
        let x = x_right - w;
        ordered.push((UiRect::new(x, y, w, h_i), b.id()));
        x_right = x - gap;
    }
    // Перевернуть в left-to-right порядок (как требует контракт PanelHeaderLayout).
    ordered.reverse();
    let layout = PanelHeaderLayout {
        rect: UiRect::new(slot.x, slot.y, slot.w, header_h),
        buttons: ordered,
    };
    let style = PanelHeaderStyle {
        separator_color: palette.panel_border,
        separator_y: slot.y + header_h,
    };
    (layout, style)
}

/// Нарисовать сепаратор шапки — 1px горизонтальную линию под шапкой
/// (цвет = `style.separator_color` = слот `palette.panel_border`).
///
/// Возвращает `Vec<PaintItem>` (1 элемент — `PaintItem::Rect` высотой 1px
/// на всю ширину шапки) для прямого push в `Painter`-items потребителя,
/// либо `Painter::rect` — оба пути эквивалентны (контракт G7 — Painter
/// копит данные, не исполняет).
pub fn paint_panel_separator(
    layout: &PanelHeaderLayout,
    style: &PanelHeaderStyle,
) -> Vec<PaintItem> {
    vec![PaintItem::Rect {
        rect: UiRect::new(layout.rect.x, style.separator_y, layout.rect.w, 1.0),
        fill: style.separator_color,
        border: [0.0; 4],
        radius: 0.0,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::palette_a;

    /// 3 кнопки (Icon close + Toggle theme + Toggle reset) —
    /// правый край close flush к `slot.right - inset`; theme на `gap`
    /// левее close; reset на `gap` левее theme; высота Icon = `ICON_BUTTON_SIZE`,
    /// вертикально центрирован в `BUTTON_HEIGHT`.
    #[test]
    fn three_buttons_right_aligned_with_correct_gap_and_centers() {
        let slot = UiRect::new(0.0, 0.0, 500.0, 30.0);
        let p = palette_a();
        // Визуальный порядок (left-to-right): reset, theme, close (как kit_ui/admin_ui).
        let buttons = [
            HeaderButton::Toggle {
                label_w: 96.0,
                id: "reset",
            },
            HeaderButton::Toggle {
                label_w: 170.0,
                id: "theme",
            },
            HeaderButton::Icon {
                kind: IconKind::Close,
                id: "close",
            },
        ];
        let (layout, style) = panel_header(slot, &buttons, &p);
        assert_eq!(layout.buttons.len(), 3);
        // Порядок сохранён (left-to-right): reset, theme, close.
        assert_eq!(layout.buttons[0].1, "reset");
        assert_eq!(layout.buttons[1].1, "theme");
        assert_eq!(layout.buttons[2].1, "close");
        // Правый край close = slot.right - inset (SPACING_SM = 8).
        let inset = canvas_core::tokens::SPACING_SM;
        let (close_rect, _) = layout.buttons[2];
        assert!(
            (close_rect.right() - (slot.right() - inset)).abs() < 0.01,
            "close.right = slot.right - inset (SPACING_SM)"
        );
        // Icon-кнопка: квадрат ICON_BUTTON_SIZE, вертикально центрирована в BUTTON_HEIGHT.
        assert!((close_rect.w - ICON_BUTTON_SIZE).abs() < 0.01);
        assert!((close_rect.h - ICON_BUTTON_SIZE).abs() < 0.01);
        let y_center_expected = slot.y + (BUTTON_HEIGHT - ICON_BUTTON_SIZE) / 2.0;
        assert!((close_rect.y - y_center_expected).abs() < 0.01);
        // Theme: x = close.x - gap - label_w; высота = BUTTON_HEIGHT.
        let (theme_rect, _) = layout.buttons[1];
        assert!(
            (theme_rect.right() - (close_rect.x - GAP_CONTROLS)).abs() < 0.01,
            "theme.right = close.x - GAP_CONTROLS"
        );
        assert!((theme_rect.w - 170.0).abs() < 0.01);
        assert!((theme_rect.h - BUTTON_HEIGHT).abs() < 0.01);
        assert!(
            (theme_rect.y - slot.y).abs() < 0.01,
            "Toggle — full BUTTON_HEIGHT"
        );
        // Reset: x = theme.x - gap - label_w.
        let (reset_rect, _) = layout.buttons[0];
        assert!(
            (reset_rect.right() - (theme_rect.x - GAP_CONTROLS)).abs() < 0.01,
            "reset.right = theme.x - GAP_CONTROLS"
        );
        // Сепаратор: y = slot.y + BUTTON_HEIGHT, цвет = palette.panel_border.
        assert!((style.separator_y - (slot.y + BUTTON_HEIGHT)).abs() < 0.01);
        assert_eq!(style.separator_color, p.panel_border);
        // Rect шапки: slot по X, BUTTON_HEIGHT по Y.
        assert!((layout.rect.x - slot.x).abs() < 0.01);
        assert!((layout.rect.w - slot.w).abs() < 0.01);
        assert!((layout.rect.h - BUTTON_HEIGHT).abs() < 0.01);
    }

    /// Сепаратор: paint_panel_separator возвращает один `PaintItem::Rect`
    /// 1px высотой на всю ширину шапки, цвет = `palette.panel_border`,
    /// рамка прозрачная, радиус 0.
    #[test]
    fn paint_separator_emits_one_pixel_line_with_palette_border_slot() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 30.0);
        let p = palette_a();
        let (layout, style) = panel_header(slot, &[], &p);
        let items = paint_panel_separator(&layout, &style);
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0],
            PaintItem::Rect {
                rect: UiRect::new(slot.x, slot.y + BUTTON_HEIGHT, slot.w, 1.0),
                fill: p.panel_border,
                border: [0.0; 4],
                radius: 0.0,
            }
        );
    }
}
