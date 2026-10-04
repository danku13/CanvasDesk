//! FR-UI-CHAT: chat message bubble (normal/error/success + tool calls).
//!
//! Извлекает паттерн из `agent_panel.rs` (сообщение чата: kind=Normal/
//! Error/Success + строки tool_calls). Канон геометрии: bubble = весь слот;
//! padding = SPACING_SM; text_area = bubble inset; tool_call_rows —
//! ниже text_area с зазором SPACING_S, по `CHAT_LINE_H` высотой каждый.
//!
//! Контракт F-8 (PRD-0009 §8): цвет — только слоты [`KitPalette`];
//! отступы/радиусы — только шкала `canvas_core::tokens::SPACING_*`/`RADIUS_*`;
//! текст — через `TextMeasurer` у потребителя. Tint (alpha-overlay над rgb
//! слота для error/success фона) — именованный паттерн `agent_panel.rs:
//! 443-451`: rgb сохраняется, меняется только alpha (прозрачность фона/
//! рамки под семантику error/success — не новый цвет, а прозрачность
//! существующего слота `control_danger`/`control_success`).

use crate::component::KitPalette;
use crate::geometry::UiRect;
use crate::paint::PaintItem;

// --- Вид сообщения --------------------------------------------------------

/// Семантика сообщения чата — определяет слоты заливки/рамки/текста.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatBubbleKind {
    /// Обычное сообщение (фон `panel_fill`, текст `text`).
    Normal,
    /// Сообщение об ошибке (фон `control_danger` tint, текст `control_danger`).
    Error,
    /// Сообщение об успехе (фон `control_success` tint, текст `control_success`).
    Success,
}

// --- Метрики bubble -------------------------------------------------------

/// Высота строки текста bubble (FONT_CAPTION · SCREEN_LINE_FACTOR ≈ 14.3 →
/// 14). Источник: `agent_panel.rs` (n_lines · 14.0 px) — кегль 11px подписи
/// пузыря, line-height 14.
pub const CHAT_LINE_H: f32 = 14.0;

/// Высота строки tool_call (моноширинная подпись, 14 px line — та же
/// `CHAT_LINE_H`, отдельная константа для семантики).
pub const CHAT_TOOL_CALL_H: f32 = CHAT_LINE_H;

// --- Раскладка/стиль ------------------------------------------------------

/// Раскладка bubble: rect пузыря + text_area + tool_call_rows.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatBubbleLayout {
    /// Rect пузыря (весь слот).
    pub rect: UiRect,
    /// Text area внутри пузыря (после пада `SPACING_SM`).
    pub text_area: UiRect,
    /// Rect'ы строк tool_call (по `CHAT_TOOL_CALL_H` высотой, ниже
    /// text_area с зазором `SPACING_S`).
    pub tool_call_rows: Vec<UiRect>,
}

/// Стиль bubble: заливка/рамка/радиус/цвет текста — слоты [`KitPalette`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChatBubbleStyle {
    /// Bubble fill (Normal → `panel_fill`, Error → `control_danger` tint,
    /// Success → `control_success` tint).
    pub fill: [f32; 4],
    /// Bubble border (Normal → `panel_border`, Error/Success →
    /// `control_danger`/`control_success` tint 0.38).
    pub border: [f32; 4],
    /// Bubble corner radius — `RADIUS_PANEL` (карточный радиус кита).
    pub radius: f32,
    /// Text color (Normal → `text`, Error → `control_danger`,
    /// Success → `control_success`).
    pub text_color: [f32; 4],
}

/// Alpha-tint (rgb сохраняется, alpha заменяется) — именованный паттерн
/// `agent_panel.rs:443-451`: tinted-фон под error/success семантику.
fn tint(slot: [f32; 4], alpha: f32) -> [f32; 4] {
    [slot[0], slot[1], slot[2], alpha]
}

/// Раскладка chat bubble в слоте `slot` с `n_lines` строками текста и
/// `n_tool_calls` строками tool_call.
///
/// Геометрия: bubble = весь слот; text_area = bubble inset `SPACING_SM`;
/// высота text_area = `n_lines · CHAT_LINE_H`; tool_call_rows начинаются
/// ниже text_area с зазором `SPACING_S`, по `CHAT_TOOL_CALL_H` высотой
/// каждый.
///
/// Слот `slot` должен быть достаточно высоким — кит НЕ маскирует
/// переполнение (деградация видна потребителю, ловится G4-линтом). Ширина
/// bubble = slot.w (как в `agent_panel.rs`).
pub fn chat_bubble(
    slot: UiRect,
    n_lines: usize,
    n_tool_calls: usize,
    kind: ChatBubbleKind,
    palette: &KitPalette,
) -> (ChatBubbleLayout, ChatBubbleStyle) {
    let pad = canvas_core::tokens::SPACING_SM; // 8
                                               // text_area: bubble inset pad.
    let text_area_w = (slot.w - 2.0 * pad).max(0.0);
    let text_area_h = (n_lines as f32) * CHAT_LINE_H;
    let text_area = UiRect::new(slot.x + pad, slot.y + pad, text_area_w, text_area_h);
    // tool_call_rows: ниже text_area с зазором SPACING_S.
    let mut tool_call_rows = Vec::with_capacity(n_tool_calls);
    let tc_start_y = text_area.bottom() + canvas_core::tokens::SPACING_S;
    let tc_w = text_area_w;
    for i in 0..n_tool_calls {
        let y = tc_start_y + (i as f32) * CHAT_TOOL_CALL_H;
        tool_call_rows.push(UiRect::new(text_area.x, y, tc_w, CHAT_TOOL_CALL_H));
    }

    // Стиль: kind → слоты palette. Tint-альфы — исторические значения
    // agent_panel.rs:443-451 (0.08 фон, 0.38 рамка).
    let (fill, border, text_color) = match kind {
        ChatBubbleKind::Normal => (palette.panel_fill, palette.panel_border, palette.text),
        ChatBubbleKind::Error => (
            tint(palette.control_danger, 0.08),
            tint(palette.control_danger, 0.38),
            palette.control_danger,
        ),
        ChatBubbleKind::Success => (
            tint(palette.control_success, 0.08),
            tint(palette.control_success, 0.38),
            palette.control_success,
        ),
    };
    let style = ChatBubbleStyle {
        fill,
        border,
        radius: canvas_core::tokens::RADIUS_PANEL,
        text_color,
    };

    let layout = ChatBubbleLayout {
        rect: slot,
        text_area,
        tool_call_rows,
    };
    (layout, style)
}

/// Отрисовать chat bubble: возвращает `Vec<PaintItem>` — фон пузыря.
/// Подписи (текст, tool_call строки) — забота потребителя: текст рисуется
/// через `Painter::label`/`TextMeasurer` слотом `style.text_color`/
/// `palette.text_muted` (tool_call — приглушённый текст). Контракт F-8:
/// кит цвет от геометрии не отделяет — здесь только rect пузыря.
pub fn paint_chat_bubble(layout: &ChatBubbleLayout, style: &ChatBubbleStyle) -> Vec<PaintItem> {
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

    /// Normal → panel_fill/panel_border/text.
    #[test]
    fn kind_normal_uses_panel_slots() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (_, style) = chat_bubble(slot, 2, 0, ChatBubbleKind::Normal, &p);
        assert_eq!(style.fill, p.panel_fill);
        assert_eq!(style.border, p.panel_border);
        assert_eq!(style.text_color, p.text);
        assert_eq!(style.radius, canvas_core::tokens::RADIUS_PANEL);
    }

    /// Error → control_danger tint (0.08 fill, 0.38 border, full text).
    #[test]
    fn kind_error_uses_danger_slot_tint() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (_, style) = chat_bubble(slot, 1, 1, ChatBubbleKind::Error, &p);
        assert_eq!(
            style.fill,
            [
                p.control_danger[0],
                p.control_danger[1],
                p.control_danger[2],
                0.08
            ]
        );
        assert_eq!(
            style.border,
            [
                p.control_danger[0],
                p.control_danger[1],
                p.control_danger[2],
                0.38
            ]
        );
        assert_eq!(style.text_color, p.control_danger);
    }

    /// Success → control_success tint (0.08 fill, 0.38 border, full text).
    #[test]
    fn kind_success_uses_success_slot_tint() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (_, style) = chat_bubble(slot, 1, 0, ChatBubbleKind::Success, &p);
        assert_eq!(
            style.fill,
            [
                p.control_success[0],
                p.control_success[1],
                p.control_success[2],
                0.08
            ]
        );
        assert_eq!(
            style.border,
            [
                p.control_success[0],
                p.control_success[1],
                p.control_success[2],
                0.38
            ]
        );
        assert_eq!(style.text_color, p.control_success);
    }

    /// Смена палитры меняет fill — контракт F-8 (цвета только из слотов).
    #[test]
    fn style_uses_palette_slots_only() {
        let a = palette_a();
        let b = palette_b();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        for kind in [
            ChatBubbleKind::Normal,
            ChatBubbleKind::Error,
            ChatBubbleKind::Success,
        ] {
            let (_, sa) = chat_bubble(slot, 1, 0, kind, &a);
            let (_, sb) = chat_bubble(slot, 1, 0, kind, &b);
            assert_ne!(sa.fill, sb.fill, "{kind:?}: fill не из слота");
            assert_ne!(sa.border, sb.border, "{kind:?}: border не из слота");
            assert_ne!(
                sa.text_color, sb.text_color,
                "{kind:?}: text_color не из слота"
            );
        }
    }

    /// Раскладка уважает границы слота: rect = slot; text_area внутри пада;
    /// tool_call_rows ниже text_area с зазором SPACING_S, по CHAT_TOOL_CALL_H
    /// высотой каждый.
    #[test]
    fn layout_respects_slot_bounds_and_tool_call_stacking() {
        let p = palette_a();
        let slot = UiRect::new(10.0, 20.0, 280.0, 120.0);
        let n_lines = 2;
        let n_tool_calls = 3;
        let (lay, _) = chat_bubble(slot, n_lines, n_tool_calls, ChatBubbleKind::Normal, &p);
        // rect = slot.
        assert_eq!(lay.rect, slot);
        let pad = canvas_core::tokens::SPACING_SM; // 8
                                                   // text_area: внутри пада, высота = n_lines · CHAT_LINE_H.
        assert!((lay.text_area.x - (slot.x + pad)).abs() < 0.01);
        assert!((lay.text_area.y - (slot.y + pad)).abs() < 0.01);
        assert!((lay.text_area.w - (slot.w - 2.0 * pad)).abs() < 0.01);
        assert!((lay.text_area.h - (n_lines as f32 * CHAT_LINE_H)).abs() < 0.01);
        // tool_call_rows: n_tool_calls rect'ов.
        assert_eq!(lay.tool_call_rows.len(), n_tool_calls);
        let tc_start_y = lay.text_area.bottom() + canvas_core::tokens::SPACING_S;
        for (i, r) in lay.tool_call_rows.iter().enumerate() {
            let expected_y = tc_start_y + (i as f32) * CHAT_TOOL_CALL_H;
            assert!((r.y - expected_y).abs() < 0.01, "tc[{i}].y");
            assert!((r.h - CHAT_TOOL_CALL_H).abs() < 0.01, "tc[{i}].h");
            assert!((r.x - lay.text_area.x).abs() < 0.01, "tc[{i}].x");
            assert!((r.w - lay.text_area.w).abs() < 0.01, "tc[{i}].w");
        }
        // Без tool_calls — пустой Vec.
        let (lay_no_tc, _) = chat_bubble(slot, 2, 0, ChatBubbleKind::Normal, &p);
        assert!(lay_no_tc.tool_call_rows.is_empty());
    }

    /// paint_chat_bubble — ровно 1 item (фон пузыря).
    #[test]
    fn paint_emits_single_background_rect() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (lay, style) = chat_bubble(slot, 2, 1, ChatBubbleKind::Normal, &p);
        let items = paint_chat_bubble(&lay, &style);
        assert_eq!(items.len(), 1, "только фон пузыря");
        assert_eq!(
            items[0],
            PaintItem::Rect {
                rect: lay.rect,
                fill: style.fill,
                border: style.border,
                radius: style.radius,
            }
        );
    }
}
