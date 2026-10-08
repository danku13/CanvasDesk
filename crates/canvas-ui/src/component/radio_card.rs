//! FR-UI-RADIO: radio-card компонент (label + desc + selected ring).
//!
//! Извлекает паттерн из `ai_onboarding` (3×) и `graph_builder_ui` (3×) —
//! радио-карточка с радио-индикатором (◉/○), заголовком и описанием.
//! Канон геометрии: карточка = весь слот; индикатор 12×12 (диаметр кружка)
//! слева по центру вертикали; label — справа от индикатора с зазором
//! SPACING_SM; description — ниже label с зазором SPACING_S.
//!
//! Контракт F-8 (PRD-0009 §8): цвет — только слоты [`KitPalette`];
//! отступы/радиусы — только шкала `canvas_core::tokens::SPACING_*`/`RADIUS_*`;
//! текст — через `TextMeasurer` у потребителя (кит отдаёт rect'ы для подписи).
//!
//! Состояние `selected` (boolean) и `KitState` (Normal/Hovered/Disabled)
//! управляют выбором слотов: selected → акцент-тинт + акцент-рамка;
//! hovered → hover_fill; normal → panel_fill. Tint (alpha-overlay над
//! rgb слота) — именованный паттерн из `agent_panel.rs:443-444` /
//! `graph_builder_ui.rs:197-202`: rgb сохраняется, меняется только alpha
//! (документированное отклонение от «никакой арифметики над цветами» —
//! tint это не новый цвет, а прозрачность существующего слота).

use crate::component::{KitPalette, KitState};
use crate::geometry::UiRect;
use crate::paint::PaintItem;

// --- Метрики radio-card ----------------------------------------------------

/// Размер индикатора radio-card (кружок ◉/○ слева, по центру вертикали).
/// 12 px — диаметр кружка прототипа (graph_builder_ui/ai_onboarding):
/// визуально соразмерен с подписью 13 px икон-кнопки 26 px (той же шапки).
/// Не выводится из `RADIUS_PILL` (токен радиуса, не размера) — explicit
/// константа kit-сущности.
pub const RADIO_INDICATOR_SIZE: f32 = 12.0;

/// Высота строки label radio-card (FONT_BODY · SCREEN_LINE_FACTOR ≈ 16.9 →
/// 16). Источник: `graph_builder_ui.rs` rect подписи режима (16 px height).
pub const RADIO_LABEL_LINE_H: f32 = 16.0;

/// Высота строки description radio-card (FONT_CAPTION · SCREEN_LINE_FACTOR ≈
/// 14.3 → 14). Источник: `graph_builder_ui.rs` rect описания (14 px height).
pub const RADIO_DESC_LINE_H: f32 = 14.0;

// --- Раскладка/стиль -------------------------------------------------------

/// Раскладка radio-card: внешний rect + индикатор + rect'ы подписи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioCardLayout {
    /// Card background rect (весь слот).
    pub rect: UiRect,
    /// Selected indicator (radio dot 12×12) — слева по центру вертикали.
    pub indicator: UiRect,
    /// Label text rect (верхняя область справа от индикатора).
    pub label: UiRect,
    /// Description text rect (ниже label, опционально). `None` — без desc.
    pub desc: Option<UiRect>,
}

/// Стиль radio-card: заливка/рамка карточки + цвета подписи + цвет
/// индикатора. Все цвета — слоты [`KitPalette`] (контракт F-8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadioCardStyle {
    /// Card background fill (selected → accent tint, unselected → panel_fill).
    pub card_fill: [f32; 4],
    /// Card border (selected → accent, unselected → panel_border).
    pub card_border: [f32; 4],
    /// Card corner radius — `RADIUS_PANEL` (карточный радиус кита).
    pub radius: f32,
    /// Label text color — `text_title` (жирная подпись).
    pub label_color: [f32; 4],
    /// Description text color (muted) — `text_muted`.
    pub desc_color: [f32; 4],
    /// Indicator fill (selected → accent, unselected → text_muted).
    pub indicator_fill: [f32; 4],
}

// tint() извлечена в `crate::paint::tint` (аудит 2026-10-06: была дублирована 3×).
use crate::paint::tint;

/// Раскладка radio-card в слоте `slot` с измеренными ширинами подписей.
///
/// `label_w` — измеренная ширина label (через `TextMeasurer::width_of`,
/// как `chip_size`/`button_size` — ширина текста без пада, кит не
/// дублирует замер).
/// `desc_w` — измеренная ширина desc; `None` — без description (карточка
/// с одним label, desc rect не возвращается).
/// `selected` — выбрана ли карточка (определяет слот заливки/рамки).
/// `state` — `KitState` (Hovered/Disabled управляют хромом; Pressed
/// трактуется как Hovered — стиль radio-card не различает нажатие).
///
/// Геометрия: card = весь слот; indicator 12×12 слева, по центру вертикали;
/// label — справа от индикатора с зазором `SPACING_SM`, высота
/// `RADIO_LABEL_LINE_H`; desc (если есть) — ниже label с зазором
/// `SPACING_S`, высота `RADIO_DESC_LINE_H`. Все пад'и — `SPACING_SM`.
pub fn radio_card(
    slot: UiRect,
    label_w: f32,
    desc_w: Option<f32>,
    selected: bool,
    state: KitState,
    palette: &KitPalette,
) -> (RadioCardLayout, RadioCardStyle) {
    let pad = canvas_core::tokens::SPACING_SM; // 8
    let indicator = RADIO_INDICATOR_SIZE; // 12
                                          // X-координата правого блока: pad + indicator + gap (SPACING_SM).
    let text_x = slot.x + pad + indicator + pad;
    // Label — вверху с отступом pad, высота = RADIO_LABEL_LINE_H.
    let label_y = slot.y + pad;
    let label = UiRect::new(text_x, label_y, label_w.max(0.0), RADIO_LABEL_LINE_H);
    // Description — ниже label с зазором SPACING_S; высота = RADIO_DESC_LINE_H.
    let desc = desc_w.map(|w| {
        let desc_y = label_y + RADIO_LABEL_LINE_H + canvas_core::tokens::SPACING_S;
        UiRect::new(text_x, desc_y, w.max(0.0), RADIO_DESC_LINE_H)
    });
    // Индикатор — слева, по центру вертикали карточки.
    let indicator_y = slot.y + (slot.h - indicator).max(0.0) / 2.0;
    let indicator_rect = UiRect::new(slot.x + pad, indicator_y, indicator, indicator);

    let layout = RadioCardLayout {
        rect: slot,
        indicator: indicator_rect,
        label,
        desc,
    };

    // Стиль: (selected, state) → слоты palette. Tint-альфа для selected
    // (0.10 — историческое значение graph_builder_ui.rs:201).
    let (card_fill, card_border, indicator_fill) = match (selected, state) {
        (true, KitState::Disabled) => (
            tint(palette.accent, 0.10),
            palette.control_border,
            palette.disabled_text,
        ),
        (true, _) => (tint(palette.accent, 0.10), palette.accent, palette.accent),
        (false, KitState::Hovered | KitState::Pressed) => {
            (palette.hover_fill, palette.panel_border, palette.text_muted)
        }
        (false, KitState::Disabled) => (
            palette.panel_fill,
            palette.panel_border,
            palette.disabled_text,
        ),
        (false, _) => (palette.panel_fill, palette.panel_border, palette.text_muted),
    };
    let label_color = if state == KitState::Disabled {
        palette.disabled_text
    } else {
        palette.text_title
    };
    let desc_color = if state == KitState::Disabled {
        palette.disabled_text
    } else {
        palette.text_muted
    };
    let style = RadioCardStyle {
        card_fill,
        card_border,
        radius: canvas_core::tokens::RADIUS_PANEL,
        label_color,
        desc_color,
        indicator_fill,
    };
    (layout, style)
}

/// Отрисовать radio-card: возвращает `Vec<PaintItem>` для фона карточки +
/// индикатора. Подписи (label/desc) — забота потребителя: текст рисуется
/// через `Painter::label`/`TextMeasurer` (кит цвет от геометрии не отделяет,
/// F-8). Порядок items: фон карточки → индикатор (индикатор поверх фона).
pub fn paint_radio_card(layout: &RadioCardLayout, style: &RadioCardStyle) -> Vec<PaintItem> {
    // Selected: индикатор — залитый кружок (fill=accent, радиус=di/2 → круг).
    // Unselected: индикатор — кольцо (fill=прозрачный, border=text_muted).
    // Класс radio-card хранит «selected» через card_fill/card_border (выбор
    // слота в `radio_card`); индикатор здесь всегда рисуется как залитый
    // кружок цветом `indicator_fill` (selected=accent, unselected=muted) —
    // вариант «кольцо» для unselected — ответственность потребителя (через
    // `Painter::rect` с прозрачным fill) — кит отдаёт канонический залитый
    // кружок, согласованый с glyph-фолбэком `◉`/`○` прототипа.
    let indicator_radius = layout.indicator.w / 2.0;
    vec![
        // 1. Card background (залитый rect с рамкой).
        PaintItem::Rect {
            rect: layout.rect,
            fill: style.card_fill,
            border: style.card_border,
            radius: style.radius,
        },
        // 2. Indicator (залитый кружок поверх фона).
        PaintItem::Rect {
            rect: layout.indicator,
            fill: style.indicator_fill,
            border: [0.0; 4],
            radius: indicator_radius,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{palette_a, palette_b};

    /// Selected vs unselected — разные стили (контракт F-8: смена слота
    /// меняет стиль). Selected → accent fill/border; unselected → panel_*
    /// fill/border.
    #[test]
    fn selected_vs_unselected_produce_different_styles() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 200.0, 54.0);
        let (_, sel) = radio_card(slot, 80.0, Some(120.0), true, KitState::Normal, &p);
        let (_, uns) = radio_card(slot, 80.0, Some(120.0), false, KitState::Normal, &p);
        // Fill: selected = accent tint (alpha 0.10); unselected = panel_fill.
        assert_ne!(sel.card_fill, uns.card_fill, "fill различается");
        // Border: selected = accent; unselected = panel_border.
        assert_eq!(sel.card_border, p.accent);
        assert_eq!(uns.card_border, p.panel_border);
        // Indicator: selected = accent; unselected = text_muted.
        assert_eq!(sel.indicator_fill, p.accent);
        assert_eq!(uns.indicator_fill, p.text_muted);
        // Радиус — RADIUS_PANEL (карточный).
        assert_eq!(sel.radius, canvas_core::tokens::RADIUS_PANEL);
        assert_eq!(uns.radius, canvas_core::tokens::RADIUS_PANEL);
    }

    /// Раскладка уважает границы слота: rect = slot, индикатор внутри слева,
    /// label/desc — справа от индикатора и не выходят за пределы карточки.
    #[test]
    fn layout_respects_slot_bounds() {
        let p = palette_a();
        let slot = UiRect::new(20.0, 30.0, 300.0, 54.0);
        let (lay, _) = radio_card(slot, 100.0, Some(200.0), true, KitState::Normal, &p);
        // rect = slot
        assert_eq!(lay.rect, slot);
        let pad = canvas_core::tokens::SPACING_SM; // 8
                                                   // Индикатор: 12×12, слева (slot.x + pad), по центру вертикали.
        assert_eq!(lay.indicator.w, RADIO_INDICATOR_SIZE);
        assert_eq!(lay.indicator.h, RADIO_INDICATOR_SIZE);
        assert!((lay.indicator.x - (slot.x + pad)).abs() < 0.01);
        let indicator_y_center = lay.indicator.y + lay.indicator.h / 2.0;
        let slot_y_center = slot.y + slot.h / 2.0;
        assert!(
            (indicator_y_center - slot_y_center).abs() < 0.01,
            "индикатор по центру вертикали"
        );
        // Индикатор внутри слота.
        assert!(lay.indicator.x >= slot.x && lay.indicator.right() <= slot.right() + 0.01);
        assert!(lay.indicator.y >= slot.y && lay.indicator.bottom() <= slot.bottom() + 0.01);
        // Label: справа от индикатора (text_x = slot.x + pad + 12 + pad),
        // высота RADIO_LABEL_LINE_H (16).
        let text_x = slot.x + pad + RADIO_INDICATOR_SIZE + pad;
        assert!((lay.label.x - text_x).abs() < 0.01);
        assert!((lay.label.y - (slot.y + pad)).abs() < 0.01);
        assert!((lay.label.w - 100.0).abs() < 0.01);
        assert!((lay.label.h - RADIO_LABEL_LINE_H).abs() < 0.01);
        // Desc: ниже label с зазором SPACING_S.
        let desc = lay.desc.expect("desc есть");
        let desc_y = slot.y + pad + RADIO_LABEL_LINE_H + canvas_core::tokens::SPACING_S;
        assert!((desc.y - desc_y).abs() < 0.01);
        assert!((desc.w - 200.0).abs() < 0.01);
        assert!((desc.h - RADIO_DESC_LINE_H).abs() < 0.01);
        // Без desc_w — desc rect нет.
        let (lay_no_desc, _) = radio_card(slot, 100.0, None, false, KitState::Normal, &p);
        assert!(lay_no_desc.desc.is_none());
    }

    /// Смена палитры меняет стиль — контракт F-8 «цвета только из слотов».
    #[test]
    fn style_uses_palette_slots_only() {
        let a = palette_a();
        let b = palette_b();
        let slot = UiRect::new(0.0, 0.0, 200.0, 54.0);
        let (_, sa) = radio_card(slot, 80.0, Some(120.0), true, KitState::Normal, &a);
        let (_, sb) = radio_card(slot, 80.0, Some(120.0), true, KitState::Normal, &b);
        assert_ne!(sa.card_fill, sb.card_fill);
        assert_ne!(sa.card_border, sb.card_border);
        assert_ne!(sa.indicator_fill, sb.indicator_fill);
        assert_ne!(sa.label_color, sb.label_color);
        assert_ne!(sa.desc_color, sb.desc_color);
    }

    /// Hovered state — слот hover_fill (а не panel_fill).
    #[test]
    fn hovered_state_uses_hover_slot() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 200.0, 54.0);
        let (_, hover) = radio_card(slot, 80.0, Some(120.0), false, KitState::Hovered, &p);
        assert_eq!(hover.card_fill, p.hover_fill);
    }

    /// paint_radio_card — ровно 2 items (фон + индикатор), индикатор
    /// поверх фона; радиус индикатора = side/2 (кружок).
    #[test]
    fn paint_emits_two_items_background_then_indicator() {
        let p = palette_a();
        let slot = UiRect::new(10.0, 20.0, 300.0, 54.0);
        let (lay, style) = radio_card(slot, 80.0, Some(120.0), true, KitState::Normal, &p);
        let items = paint_radio_card(&lay, &style);
        assert_eq!(items.len(), 2, "фон + индикатор");
        // 0: card background.
        assert_eq!(
            items[0],
            PaintItem::Rect {
                rect: lay.rect,
                fill: style.card_fill,
                border: style.card_border,
                radius: style.radius,
            }
        );
        // 1: indicator (radius = side/2 → circle).
        assert_eq!(
            items[1],
            PaintItem::Rect {
                rect: lay.indicator,
                fill: style.indicator_fill,
                border: [0.0; 4],
                radius: lay.indicator.w / 2.0,
            }
        );
    }
}
