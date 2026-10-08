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

/// Высота строки header bubble (кегль 9px · SCREEN_LINE_FACTOR 1.3 ≈ 11.7 →
/// 12). Источник: `agent_panel.rs` bot bubble — подпись «AI Агент» рисуется
/// моноширинно-приглушённой строкой над текстом; до Q2 (Task Q) высота
/// header-rect была 14 (магическое число, равно `CHAT_LINE_H`), что
/// визуально растягивало строку header'а под текстовую. Q2 канонизирует
/// header как отдельную метрику: 12px — реальный line-height кегля 9.
pub const CHAT_HEADER_H: f32 = 12.0;

// --- Раскладка/стиль ------------------------------------------------------

/// Раскладка bubble: rect пузыря + text_area + (опц.) header_area +
/// tool_call_rows.
///
/// `header_area` — строка-подпись над text_area (например, «AI Агент» в bot
/// bubble `agent_panel.rs`); `None` если `header=false` (нет header'а —
/// обычный bubble без подписи). Семантика: header — не заголовок ОКНА
/// панели (его рисует `panel_header`), а подпись ОТПРАВИТЕЛЯ внутри bubble.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatBubbleLayout {
    /// Rect пузыря (весь слот).
    pub rect: UiRect,
    /// Text area внутри пузыря (после пада `SPACING_SM`, а при `header=true`
    /// — ниже `header_area.bottom()`).
    pub text_area: UiRect,
    /// Header area (опц.) — строка-подпись над text_area. `None` при
    /// `header=false`. Высота `CHAT_HEADER_H`; x/w — text_area-выравнены.
    pub header_area: Option<UiRect>,
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

// tint() извлечена в `crate::paint::tint` (аудит 2026-10-06: была дублирована 3×).
use crate::paint::tint;

/// Раскладка chat bubble в слоте `slot` с `n_lines` строками текста и
/// `n_tool_calls` строками tool_call.
///
/// Геометрия: bubble = весь слот; `header_area` (если `header=true`) —
/// строка-подпись над text_area (половина `SPACING_SM` от верха bubble,
/// высотой `CHAT_HEADER_H`); text_area — ниже header'а (без доп. зазора:
/// header.bottom() = text_area.y) либо, при `header=false`, на `SPACING_SM`
/// от верха; высота text_area = `n_lines · CHAT_LINE_H`; tool_call_rows
/// начинаются ниже text_area с зазором `SPACING_S`, по `CHAT_TOOL_CALL_H`
/// высотой каждый.
///
/// Слот `slot` должен быть достаточно высоким — кит НЕ маскирует
/// переполнение (деградация видна потребителю, ловится G4-линтом). Ширина
/// bubble = slot.w (как в `agent_panel.rs`).
///
/// `header` — флаг наличия header-строки над text_area (например, подпись
/// «AI Агент» в bot bubble `agent_panel.rs`); `false` — обычный bubble без
/// header'а (историческое поведение, parity с прототипом чата: user bubble,
/// error-тосты и т.п.).
pub fn chat_bubble(
    slot: UiRect,
    n_lines: usize,
    n_tool_calls: usize,
    kind: ChatBubbleKind,
    palette: &KitPalette,
    header: bool,
) -> (ChatBubbleLayout, ChatBubbleStyle) {
    let pad = canvas_core::tokens::SPACING_SM; // 8
                                               // text_area: bubble inset pad.
    let text_area_w = (slot.w - 2.0 * pad).max(0.0);
    // Header area (опц.) — половина pad от верха bubble, высотой CHAT_HEADER_H.
    // Половина pad (=4) — историческое смещение «AI Агент» в agent_panel.rs
    // (`msg_y + 4.0`); полный pad (=8) растягивал бы header под текстовую
    // строку визуально — header меньше (кегль 9 vs 11).
    let header_area = if header {
        Some(UiRect::new(
            slot.x + pad,
            slot.y + pad * 0.5,
            text_area_w,
            CHAT_HEADER_H,
        ))
    } else {
        None
    };
    // text_area.y: slot.y + pad (без header) ИЛИ header.bottom() (с header).
    // Без доп. зазора header.bottom() = text_area.y — header плавно перетекает
    // в текст (визуальный gap создаётся line-height'ом самого header'а,
    // CHAT_HEADER_H=12 при кегле 9 → ~3px визуального gap до текста).
    let text_area_y = match &header_area {
        Some(h) => h.bottom(),
        None => slot.y + pad,
    };
    let text_area_h = (n_lines as f32) * CHAT_LINE_H;
    let text_area = UiRect::new(slot.x + pad, text_area_y, text_area_w, text_area_h);
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
        header_area,
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
        let (_, style) = chat_bubble(slot, 2, 0, ChatBubbleKind::Normal, &p, false);
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
        let (_, style) = chat_bubble(slot, 1, 1, ChatBubbleKind::Error, &p, false);
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
        let (_, style) = chat_bubble(slot, 1, 0, ChatBubbleKind::Success, &p, false);
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
            let (_, sa) = chat_bubble(slot, 1, 0, kind, &a, false);
            let (_, sb) = chat_bubble(slot, 1, 0, kind, &b, false);
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
    /// высотой каждый. `header=false` → `header_area = None` (историческое
    /// поведение bubble без подписи).
    #[test]
    fn layout_respects_slot_bounds_and_tool_call_stacking() {
        let p = palette_a();
        let slot = UiRect::new(10.0, 20.0, 280.0, 120.0);
        let n_lines = 2;
        let n_tool_calls = 3;
        let (lay, _) = chat_bubble(
            slot,
            n_lines,
            n_tool_calls,
            ChatBubbleKind::Normal,
            &p,
            false,
        );
        // rect = slot.
        assert_eq!(lay.rect, slot);
        let pad = canvas_core::tokens::SPACING_SM; // 8
                                                   // text_area: внутри пада, высота = n_lines · CHAT_LINE_H.
        assert!((lay.text_area.x - (slot.x + pad)).abs() < 0.01);
        assert!((lay.text_area.y - (slot.y + pad)).abs() < 0.01);
        assert!((lay.text_area.w - (slot.w - 2.0 * pad)).abs() < 0.01);
        assert!((lay.text_area.h - (n_lines as f32 * CHAT_LINE_H)).abs() < 0.01);
        // header_area: None (header=false).
        assert!(lay.header_area.is_none());
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
        let (lay_no_tc, _) = chat_bubble(slot, 2, 0, ChatBubbleKind::Normal, &p, false);
        assert!(lay_no_tc.tool_call_rows.is_empty());
    }

    /// Q2 (Task Q): `header=true` → `header_area` над text_area, половина
    /// `SPACING_SM` от верха bubble, высотой `CHAT_HEADER_H`; text_area
    /// начинается с `header_area.bottom()` (без доп. зазора — header
    /// плавно перетекает в текст). tool_call_rows — как обычно, ниже
    /// text_area.bottom()+SPACING_S.
    #[test]
    fn header_area_stacks_above_text_area() {
        let p = palette_a();
        let slot = UiRect::new(10.0, 20.0, 280.0, 120.0);
        let n_lines = 2;
        let n_tool_calls = 1;
        let (lay, _) = chat_bubble(
            slot,
            n_lines,
            n_tool_calls,
            ChatBubbleKind::Normal,
            &p,
            true,
        );
        let pad = canvas_core::tokens::SPACING_SM; // 8
        let header = lay.header_area.expect("header=true → Some(header_area)");
        // header_area.x/w — выравнены с text_area (pad от bubble).
        assert!((header.x - (slot.x + pad)).abs() < 0.01, "header.x");
        assert!((header.w - (slot.w - 2.0 * pad)).abs() < 0.01, "header.w");
        // header_area.y: slot.y + pad/2 (половина pad от верха — историческое
        // смещение «AI Агент» в agent_panel.rs).
        assert!((header.y - (slot.y + pad * 0.5)).abs() < 0.01, "header.y");
        // header_area.h: CHAT_HEADER_H.
        assert!((header.h - CHAT_HEADER_H).abs() < 0.01, "header.h");
        // text_area.y = header.bottom() (без доп. зазора).
        assert!(
            (lay.text_area.y - header.bottom()).abs() < 0.01,
            "text_area.y"
        );
        // text_area.h: n_lines · CHAT_LINE_H.
        assert!(
            (lay.text_area.h - (n_lines as f32 * CHAT_LINE_H)).abs() < 0.01,
            "text_area.h"
        );
        // tool_call_rows: ниже text_area.bottom()+SPACING_S, как обычно.
        let tc_start_y = lay.text_area.bottom() + canvas_core::tokens::SPACING_S;
        assert_eq!(lay.tool_call_rows.len(), n_tool_calls);
        assert!(
            (lay.tool_call_rows[0].y - tc_start_y).abs() < 0.01,
            "tc[0].y"
        );
    }

    /// Q2: `header=true` и `n_tool_calls=0` → tool_call_rows пустой, но
    /// header_area + text_area остаются валидными (no panic, no overflow).
    #[test]
    fn header_with_no_tool_calls() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (lay, _) = chat_bubble(slot, 3, 0, ChatBubbleKind::Normal, &p, true);
        assert!(lay.header_area.is_some());
        assert!(lay.tool_call_rows.is_empty());
        // text_area ниже header.bottom().
        let header = lay.header_area.unwrap();
        assert!(lay.text_area.y >= header.bottom() - 0.01);
    }

    /// paint_chat_bubble — ровно 1 item (фон пузыря).
    #[test]
    fn paint_emits_single_background_rect() {
        let p = palette_a();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        let (lay, style) = chat_bubble(slot, 2, 1, ChatBubbleKind::Normal, &p, false);
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
