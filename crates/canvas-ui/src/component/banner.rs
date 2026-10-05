//! FR-UI-BANNER: banner with label + action button.
//!
//! Extracts pattern from `autolink_ui.rs` (rejected items banner — У8):
//! горизонтальный баннер с цветной рамкой (Info/Warning/Error), текстом-
//! сообщением слева и кнопкой действия справа («Восстановить», «Retry»).
//! Канон геометрии из autolink_ui.rs/explain.rs:
//! - высота баннера = `max(BUTTON_HEIGHT, label_h + 2·SPACING_SM)` (по
//!   умолчанию `BANNER_H = 34` — паритет `autolink_ui::BANNER_H - 8 = 34`);
//! - `action_button`: правый край = `slot.right()`, ширина =
//!   `action_label_w + 2·BUTTON_PAD_H`, высота = `BUTTON_HEIGHT`;
//! - `label_area`: слева с падом `SPACING_SM`, ширина = оставшаяся ширина
//!   минус `action_button.w + 2·SPACING_SM` (зазор для кнопки).
//!
//! ## Контракт с потребителем
//!
//! Цвета — только слоты [`KitPalette`] (контракт F-8). Tint (alpha-overlay
//! над rgb слота) — именованный паттерн из `radio_card.rs`/`chat_bubble.rs`
//! (rgb сохраняется, alpha заменяется — не новый цвет, а прозрачность
//! существующего слота). `BannerKind` выбирает слот:
//! - **Info** → `control_fill` (нейтральный баннер, цвет текста `text`);
//! - **Warning** → `control_warning` (tint 0.10 fill, full border, текст
//!   `control_warning`);
//! - **Error** → `control_danger` (tint 0.10 fill, full border, текст
//!   `control_danger`).
//!
//! `paint_banner` paints ТОЛЬКО фон баннера (1 PaintItem::Rect — паритет
//! `paint_chat_bubble`, который тоже не рисует текст). Подпись label и
//! action button рисуются потребителем через `Painter::label` (label_area,
//! label_color) и `kit::button_style` + `Painter::control` (action_button,
//! Secondary/Ghost variant).

use crate::component::{KitPalette, KitState, BUTTON_HEIGHT, BUTTON_PAD_H};
use crate::geometry::UiRect;
use crate::paint::PaintItem;

/// Высота баннера по умолчанию (паритет `autolink_ui::BANNER_H - 8 = 34`):
/// баннер под шапкой autolink занимает 42 px слота, сам баннер высотой 34 px.
pub const BANNER_H: f32 = 34.0;

/// Внутренний пад баннера: `SPACING_SM` = 8 (паритет explain.rs:208 —
/// `banner[0] + 10.0` и `banner[1] + 6.0`).
pub const BANNER_PAD: f32 = canvas_core::tokens::SPACING_SM;

/// Радиус баннера — `RADIUS_PANEL` (карточный радиус кита; паритет
/// explain.rs:204 `radius = 8.0` — близко к `RADIUS_PANEL = 10.0`).
pub const BANNER_RADIUS: f32 = canvas_core::tokens::RADIUS_PANEL;

/// Высота строки текста баннера (FONT_CAPTION · SCREEN_LINE_FACTOR ≈ 14.3 →
/// 14; паритет explain.rs:211 `height = 16.0`).
pub const BANNER_LABEL_LINE_H: f32 = 16.0;

/// Tint alpha для фона баннера (rgb слота сохраняется, alpha = 0.10 —
/// мягкая подложка под текстом; паритет `radio_card.rs`/`chat_bubble.rs`).
pub const BANNER_TINT_ALPHA: f32 = 0.10;

/// Семантика баннера — выбирает слот заливки/рамки/текста.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    /// Информационный (нейтральный): `control_fill` tint, `text` подпись.
    Info,
    /// Предупреждение: `control_warning` tint, `control_warning` подпись.
    Warning,
    /// Ошибка: `control_danger` tint, `control_danger` подпись.
    Error,
}

/// Раскладка баннера: внешний rect + область подписи + rect кнопки действия.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BannerLayout {
    /// Rect баннера (весь слот).
    pub rect: UiRect,
    /// Text label area (слева, с падом `BANNER_PAD`).
    pub label_area: UiRect,
    /// Action button rect (правый край = slot.right(), высота = `BUTTON_HEIGHT`).
    pub action_button: UiRect,
}

/// Стиль баннера: заливка/рамка/радиус + цвета подписи и кнопки.
/// Все цвета — слоты [`KitPalette`] (контракт F-8); alpha-tint — для фона
/// (kind-слот с alpha = `BANNER_TINT_ALPHA`), full alpha — для рамки и
/// текста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BannerStyle {
    /// Banner background fill (kind slot tinted with `BANNER_TINT_ALPHA`).
    pub fill: [f32; 4],
    /// Banner border (kind slot, full alpha).
    pub border: [f32; 4],
    /// Banner corner radius — `BANNER_RADIUS` (`RADIUS_PANEL`).
    pub radius: f32,
    /// Label text color (kind slot for Warning/Error; `text` for Info).
    pub label_color: [f32; 4],
    /// Action button text color (`text_title` — для контраста на кнопке).
    pub action_color: [f32; 4],
}

/// Alpha-tint (rgb сохраняется, alpha заменяется) — именованный паттерн
/// `radio_card.rs`/`chat_bubble.rs`/`agent_panel.rs:443-451`.
fn tint(slot: [f32; 4], alpha: f32) -> [f32; 4] {
    [slot[0], slot[1], slot[2], alpha]
}

/// Layout a banner in `slot` with label + action button.
///
/// `label_w` — measured width of label text (consumer measures via
/// `TextMeasurer::width_of`; kit does NOT measure to keep API pure).
/// `action_label_w` — measured width of action button text.
/// `kind` — controls color (Info → `control_fill`, Warning → `control_warning`,
/// Error → `control_danger`).
/// `state` — `KitState` for action button text color (Disabled → `disabled_text`;
/// другие — `text_title`).
///
/// ## Геометрия
///
/// - rect = `slot`;
/// - `action_button`: ширина = `action_label_w + 2·BUTTON_PAD_H`, высота =
///   `BUTTON_HEIGHT`, правый край = `slot.right()`, по центру вертикали
///   слота;
/// - `label_area`: `x = slot.x + BANNER_PAD`, `y = slot.y + BANNER_PAD`,
///   ширина = `slot.w - 2·BANNER_PAD - action_button.w - BANNER_PAD`
///   (остаток после кнопки + зазор), высота = `BANNER_LABEL_LINE_H`.
pub fn banner(
    slot: UiRect,
    label_w: f32,
    action_label_w: f32,
    kind: BannerKind,
    state: KitState,
    palette: &KitPalette,
) -> (BannerLayout, BannerStyle) {
    let pad = BANNER_PAD;
    // Action button: width = action_label_w + 2·BUTTON_PAD_H, высота = BUTTON_HEIGHT,
    // правый край = slot.right(), центр по вертикали.
    let action_w = action_label_w + 2.0 * BUTTON_PAD_H;
    let action_x = slot.right() - action_w;
    let action_y = slot.y + (slot.h - BUTTON_HEIGHT).max(0.0) / 2.0;
    let action_button = UiRect::new(action_x, action_y, action_w.max(0.0), BUTTON_HEIGHT);

    // Label area: слева с падом `pad`, ширина = остаток после кнопки + зазор.
    // Зазор между label и action_button = pad (как пад label слева).
    let label_x = slot.x + pad;
    let label_y = slot.y + pad;
    let label_w_avail = (action_x - pad - label_x).max(0.0);
    // Реальная ширина label = min(measured_w, available) — kit не обрезает
    // (рендер обрезает через ellipsis), но не выходит за границу action_button.
    let label_w_actual = label_w.min(label_w_avail);
    let label_area = UiRect::new(label_x, label_y, label_w_actual, BANNER_LABEL_LINE_H);

    let layout = BannerLayout {
        rect: slot,
        label_area,
        action_button,
    };

    // Style: kind → slot colors. Tint (alpha-overlay над rgb) для фона;
    // full alpha для рамки/текста.
    let (kind_slot, label_color) = match kind {
        BannerKind::Info => (palette.control_fill, palette.text),
        BannerKind::Warning => (palette.control_warning, palette.control_warning),
        BannerKind::Error => (palette.control_danger, palette.control_danger),
    };
    let action_color = if state == KitState::Disabled {
        palette.disabled_text
    } else {
        palette.text_title
    };
    let style = BannerStyle {
        fill: tint(kind_slot, BANNER_TINT_ALPHA),
        border: kind_slot,
        radius: BANNER_RADIUS,
        label_color,
        action_color,
    };
    (layout, style)
}

/// Paint banner background (1 `PaintItem::Rect` — паритет `paint_chat_bubble`).
///
/// Тext label и action button — забота потребителя:
/// - label: `Painter::label(layout.label_area, ..., style.label_color, ...)`;
/// - action button: `kit::button_style(Secondary, state, palette)` +
///   `Painter::control(layout.action_button, &style)`, подпись —
///   `Painter::label(action_button, ..., style.action_color, ..., Center)`.
pub fn paint_banner(layout: &BannerLayout, style: &BannerStyle) -> Vec<PaintItem> {
    vec![PaintItem::Rect {
        rect: layout.rect,
        fill: style.fill,
        border: style.border,
        radius: style.radius,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{palette_a, palette_b};

    /// Info баннер: layout — label слева с падом, action_button справа
    /// (правый край = slot.right()), высота = BUTTON_HEIGHT, по центру
    /// вертикали. Style — Info → control_fill tint.
    #[test]
    fn info_banner_layout_label_left_button_right() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 500.0, BANNER_H);
        let (lay, style) = banner(
            slot,
            120.0, // label_w
            60.0,  // action_label_w
            BannerKind::Info,
            KitState::Normal,
            &p,
        );
        // rect = slot.
        assert_eq!(lay.rect, slot);
        // action_button: правый край = slot.right(), высота = BUTTON_HEIGHT.
        assert!(
            (lay.action_button.right() - slot.right()).abs() < 0.01,
            "правый край кнопки = slot.right()"
        );
        assert!((lay.action_button.h - BUTTON_HEIGHT).abs() < 0.01);
        // action_button.w = action_label_w + 2·BUTTON_PAD_H.
        let expected_w = 60.0 + 2.0 * BUTTON_PAD_H;
        assert!((lay.action_button.w - expected_w).abs() < 0.01);
        // action_button по центру вертикали.
        let cy = slot.y + (slot.h - BUTTON_HEIGHT) / 2.0;
        assert!((lay.action_button.y - cy).abs() < 0.01);
        // label_area: слева с падом BANNER_PAD.
        assert!((lay.label_area.x - (slot.x + BANNER_PAD)).abs() < 0.01);
        // label_area не выходит за action_button (зазор = pad).
        assert!(
            lay.label_area.right() <= lay.action_button.x - BANNER_PAD + 0.01,
            "label не наезжает на action_button"
        );
        // Style: Info → fill = control_fill tint, border = control_fill,
        // label_color = text, action_color = text_title.
        assert_eq!(style.fill, tint(p.control_fill, BANNER_TINT_ALPHA));
        assert_eq!(style.border, p.control_fill);
        assert_eq!(style.label_color, p.text);
        assert_eq!(style.action_color, p.text_title);
        assert_eq!(style.radius, BANNER_RADIUS);
    }

    /// Warning vs Error vs Info — разные стили (контракт F-8: смена слота
    /// меняет стиль). Warning → control_warning slot; Error → control_danger.
    #[test]
    fn kind_changes_style_slots() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 500.0, BANNER_H);
        let (_, info_s) = banner(slot, 120.0, 60.0, BannerKind::Info, KitState::Normal, &p);
        let (_, warn_s) = banner(slot, 120.0, 60.0, BannerKind::Warning, KitState::Normal, &p);
        let (_, err_s) = banner(slot, 120.0, 60.0, BannerKind::Error, KitState::Normal, &p);
        // Fill отличается (Info vs Warning vs Error).
        assert_ne!(info_s.fill, warn_s.fill);
        assert_ne!(warn_s.fill, err_s.fill);
        assert_ne!(info_s.fill, err_s.fill);
        // Border = kind slot (full alpha).
        assert_eq!(warn_s.border, p.control_warning);
        assert_eq!(err_s.border, p.control_danger);
        // label_color: Info → text; Warning → control_warning; Error → control_danger.
        assert_eq!(info_s.label_color, p.text);
        assert_eq!(warn_s.label_color, p.control_warning);
        assert_eq!(err_s.label_color, p.control_danger);
    }

    /// Disabled state → action_color = disabled_text (контракт F-8).
    #[test]
    fn disabled_state_uses_disabled_text_for_action() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 500.0, BANNER_H);
        let (_, normal_s) = banner(slot, 120.0, 60.0, BannerKind::Info, KitState::Normal, &p);
        let (_, disabled_s) = banner(slot, 120.0, 60.0, BannerKind::Info, KitState::Disabled, &p);
        assert_eq!(normal_s.action_color, p.text_title);
        assert_eq!(disabled_s.action_color, p.disabled_text);
    }

    /// `paint_banner` возвращает 1 PaintItem::Rect (фон баннера).
    #[test]
    fn paint_banner_returns_single_rect_item() {
        let p = palette_a();
        let slot = UiRect::new(10.0, 20.0, 400.0, BANNER_H);
        let (lay, style) = banner(slot, 100.0, 50.0, BannerKind::Error, KitState::Normal, &p);
        let items = paint_banner(&lay, &style);
        assert_eq!(items.len(), 1, "только фон баннера");
        match &items[0] {
            PaintItem::Rect {
                rect,
                fill,
                border,
                radius,
            } => {
                assert_eq!(*rect, lay.rect);
                assert_eq!(*fill, style.fill);
                assert_eq!(*border, style.border);
                assert!((radius - style.radius).abs() < 0.01);
            }
            other => panic!("ожидался Rect, got {other:?}"),
        }
    }

    /// Смена палитры меняет стиль (контракт F-8: стиль выбирает слот, не
    /// вычисляет цвет). palette_a vs palette_b — все слоты отличаются.
    #[test]
    fn style_changes_with_palette() {
        let a = palette_a();
        let b = palette_b();
        let slot = UiRect::new(0.0, 0.0, 500.0, BANNER_H);
        let (_, sa) = banner(slot, 120.0, 60.0, BannerKind::Warning, KitState::Normal, &a);
        let (_, sb) = banner(slot, 120.0, 60.0, BannerKind::Warning, KitState::Normal, &b);
        assert_ne!(sa.fill, sb.fill, "fill следует за палитрой");
        assert_ne!(sa.border, sb.border);
        assert_ne!(sa.label_color, sb.label_color);
        assert_ne!(sa.action_color, sb.action_color);
    }

    /// Узкий слот: action_button выходит за слот → label_area_w = 0 (clip),
    /// action_button всё равно рисуется (деградация видна — G4-линт ловит).
    #[test]
    fn narrow_slot_label_clipped_but_button_drawn() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 100.0, BANNER_H);
        let (lay, _) = banner(slot, 80.0, 60.0, BannerKind::Info, KitState::Normal, &p);
        // action_button всё равно рисуется с правого края.
        assert!(
            (lay.action_button.right() - slot.right()).abs() < 0.01,
            "action_button прижат к правому краю"
        );
        // label_area.width clip'нут до 0 (или почти) — label не наезжает на кнопку.
        assert!(
            lay.label_area.right() <= lay.action_button.x + 0.5,
            "label clipнут, не наезжает на action_button"
        );
    }
}
