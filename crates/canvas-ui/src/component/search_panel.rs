//! FR-UI-SEARCH-PANEL: top-center search panel with input + results + scroll.
//!
//! Извлекает паттерн из `search_ui` (аудит §6.1: LOW-priority kit gap):
//! панель поиска в верхней-центральной части вьюпорта, сверху —
//! однострочное поле ввода, под ним — список результатов с прокруткой.
//! Канон: ширина панели клампится к `viewport.w − 2·SIDE_MARGIN`, высота =
//! `pad + input_h + (pad + results_h)? + pad` (нет результатов — только
//! поле), верх панели = `TOP_MARGIN`, по горизонтали — центр вьюпорта.
//!
//! Контракт F-8 (PRD-0009 §8): кит НЕ выбирает цвет/содержимое — только
//! геометрия. Потребитель рисует поле/строки слотами своей темы (sample:
//! `search_ui::layout` → `panel_rect`/`input_rect`/`row_rects` — те же
//! rect'ы, что возвращает эта функция). Скролл-бар — отдельный вызов
//! `kit::scroll_bar` потребителем (если `scroll.needs_scroll()`).

use crate::component::list::{list_rows, ScrollState};
use crate::component::LIST_ROW_H;
use crate::geometry::{EdgeInsets, UiRect, UiVec2};
use crate::layout::{constrain, stack, HAlign, VAlign};

// Token re-exports (I-1: ноль визуального скачка — значения = прежним
// литералам search_ui::layout_with, файл-источник `search_ui.rs`).
use canvas_core::tokens::SPACING_LG as PANEL_SIDE_MARGIN;
use canvas_core::tokens::SPACING_LG as PANEL_TOP_MARGIN;
use canvas_core::tokens::SPACING_SM as PANEL_PADDING;

/// Желаемая ширина панели поиска (логические px; клампится к
/// `viewport.w − 2·PANEL_SIDE_MARGIN` — узкие окна не рвут раскладку).
/// Значение = `search_ui::PANEL_WIDTH` (460) — I-1: ноль скачка.
pub const SEARCH_PANEL_WIDTH: f32 = 460.0;

/// Высота поля ввода поиска. Крупнее стандартного `TEXT_FIELD_HEIGHT`
/// (= 30) — акцентная панель поиска (значение = `search_ui::INPUT_HEIGHT`
/// = 36; I-1: ноль скачка).
pub const SEARCH_PANEL_INPUT_HEIGHT: f32 = 36.0;

/// Высота строки результата по умолчанию (равна `LIST_ROW_H` = 26 —
/// унификация со списком подсказок/flowmap/calc_panel; потребитель может
/// передать свой `row_h` параметром).
pub const SEARCH_PANEL_ROW_H: f32 = LIST_ROW_H;

/// Раскладка панели поиска: rect панели + rect поля ввода + rect
/// области результатов + список rect'ов видимых строк (с учётом
/// скролла).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPanelLayout {
    /// Rect панели (топ-центр вьюпорта, ширина клампится, высота =
    /// контент + 2·PANEL_PADDING).
    pub panel: UiRect,
    /// Rect поля ввода (верх панели внутри отступа, полная ширина
    /// inner, высота = `SEARCH_PANEL_INPUT_HEIGHT`).
    pub input_field: UiRect,
    /// Rect области результатов (под полем ввода + PANEL_PADDING,
    /// высота = `min(n_results, max_visible) · row_h`; 0 строк — пустой).
    pub results_area: UiRect,
    /// Rect'ы видимых строк результата (из `list_rows` — со смещением
    /// `scroll.offset` и частичными строками на краях).
    pub result_rows: Vec<UiRect>,
}

/// Раскладка панели поиска в верхней-центральной части `viewport`.
///
/// `viewport` — видимый rect (например, окно) для центрирования панели и
/// клампа ширины (панель ≤ `viewport.w − 2·PANEL_SIDE_MARGIN`).
/// `n_results` — полное число строк модели (для вычисления `content_h`
/// скролла). `max_visible` — максимум видимых строк (далее — прокрутка).
/// `scroll` — состояние скролла (потребитель ведёт `offset`/`content_h`/
/// `viewport_h` — контрактом `scroll.viewport_h == results_area.h`).
///
/// Возвращает раскладку — потребитель рисует input/rows слотами своей
/// темы (F-8: цвет — только слоты, кит не знает).
pub fn search_panel(
    viewport: UiRect,
    n_results: usize,
    max_visible: usize,
    scroll: &ScrollState,
) -> SearchPanelLayout {
    let row_h = SEARCH_PANEL_ROW_H;
    let visible_count = n_results.min(max_visible);
    let pad_v = PANEL_PADDING;
    let input_h = SEARCH_PANEL_INPUT_HEIGHT;

    // Ширина панели: желаемая SEARCH_PANEL_WIDTH, потолок — доступная
    // ширина вьюпорта (минус 2·SIDE_MARGIN — отступы от боковых краёв).
    let available_w = (viewport.w - 2.0 * PANEL_SIDE_MARGIN).max(0.0);
    let panel_w = constrain(
        UiVec2::new(0.0, 0.0),
        UiVec2::new(available_w, f32::INFINITY),
        UiVec2::new(SEARCH_PANEL_WIDTH, 0.0),
    )
    .x;

    // Высота панели: pad + input + (pad + results_h, если есть строки) + pad.
    let results_h = visible_count as f32 * row_h;
    let panel_h = pad_v
        + input_h
        + (if visible_count > 0 {
            pad_v + results_h
        } else {
            0.0
        })
        + pad_v;

    // Панель: top-center вьюпорта + верхний отступ TOP_MARGIN.
    let panel = stack(
        UiRect::new(viewport.x, viewport.y, viewport.w, viewport.h.max(0.0)),
        UiVec2::new(panel_w, panel_h),
        HAlign::Center,
        VAlign::Start,
    );
    let panel = UiRect::new(panel.x, viewport.y + PANEL_TOP_MARGIN, panel_w, panel_h);

    // Внутренний слот (inset на PANEL_PADDING со всех сторон).
    let inner = panel.inset(&EdgeInsets::uniform(pad_v));

    // Поле ввода: верх inner, полная ширина, высота = INPUT_HEIGHT.
    let input_field = UiRect::new(inner.x, inner.y, inner.w, input_h);

    // Область результатов: под полем (через PANEL_PADDING), высота = visible·row_h.
    let results_y = input_field.bottom() + pad_v;
    let results_area = UiRect::new(inner.x, results_y, inner.w, results_h);

    // Видимые строки: list_rows (со смещением scroll.offset, частичные
    // строки на краях включаются). Контракт scroll.viewport_h == results_area.h
    // ведёт потребитель; здесь — берём как есть.
    let result_rows: Vec<UiRect> = list_rows(results_area, scroll, row_h, 0.0, n_results)
        .into_iter()
        .map(|(_, rect)| rect)
        .collect();

    SearchPanelLayout {
        panel,
        input_field,
        results_area,
        result_rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::list::ScrollState;

    /// 5 результатов, max_visible 3 → results_area содержит 3 строки
    /// (прокрутка; panel высота = 2·pad + input + pad + 3·row_h).
    #[test]
    fn five_results_max_visible_three_yields_three_rows() {
        let viewport = UiRect::new(0.0, 0.0, 1024.0, 768.0);
        // 5 строк, max_visible 3 — окно прокрутки показывает 3.
        let scroll = ScrollState {
            offset: 0.0,
            content_h: 5.0 * SEARCH_PANEL_ROW_H,
            viewport_h: 3.0 * SEARCH_PANEL_ROW_H,
        };
        let layout = search_panel(viewport, 5, 3, &scroll);

        // Панель — top-center: ширина = SEARCH_PANEL_WIDTH (влезает в 1024).
        assert!((layout.panel.w - SEARCH_PANEL_WIDTH).abs() < 0.01);
        // Центр по горизонтали: x = (viewport.w - panel.w) / 2.
        let expected_x = (viewport.w - SEARCH_PANEL_WIDTH) / 2.0;
        assert!((layout.panel.x - expected_x).abs() < 0.01);
        assert!((layout.panel.y - PANEL_TOP_MARGIN).abs() < 0.01);
        // Высота панели = pad + input + pad + 3·row_h + pad.
        let expected_h = PANEL_PADDING
            + SEARCH_PANEL_INPUT_HEIGHT
            + PANEL_PADDING
            + 3.0 * SEARCH_PANEL_ROW_H
            + PANEL_PADDING;
        assert!((layout.panel.h - expected_h).abs() < 0.01);

        // Поле ввода: верх inner, ширина = inner.w, высота = INPUT_HEIGHT.
        assert!((layout.input_field.y - (layout.panel.y + PANEL_PADDING)).abs() < 0.01);
        assert!((layout.input_field.h - SEARCH_PANEL_INPUT_HEIGHT).abs() < 0.01);
        assert!((layout.input_field.x - (layout.panel.x + PANEL_PADDING)).abs() < 0.01);

        // Область результатов: 3 строки по SEARCH_PANEL_ROW_H.
        assert!((layout.results_area.h - 3.0 * SEARCH_PANEL_ROW_H).abs() < 0.01);
        assert!(
            (layout.results_area.y - (layout.input_field.bottom() + PANEL_PADDING)).abs() < 0.01
        );

        // 3 видимые строки (offset 0, viewport 3·row_h, 5 строк в модели).
        assert_eq!(layout.result_rows.len(), 3);
        // Строки стакаются сверху вниз внутри results_area.
        for (i, r) in layout.result_rows.iter().enumerate() {
            let expected_y = layout.results_area.y + i as f32 * SEARCH_PANEL_ROW_H;
            assert!((r.y - expected_y).abs() < 0.01, "row {i}: y={}", r.y);
            assert!((r.h - SEARCH_PANEL_ROW_H).abs() < 0.01);
            assert!((r.w - layout.results_area.w).abs() < 0.01);
        }
    }

    /// Узкое окно (viewport.w < 2·SIDE_MARGIN + SEARCH_PANEL_WIDTH) →
    /// ширина панели клампится к доступной; 0 результатов → панель
    /// содержит только поле ввода (results_area пустой, result_rows пусто).
    #[test]
    fn narrow_viewport_clamps_width_zero_results_only_input() {
        // viewport.w = 200, available = 200 - 2·12 = 176 < 460.
        let viewport = UiRect::new(0.0, 0.0, 200.0, 600.0);
        let scroll = ScrollState {
            offset: 0.0,
            content_h: 0.0,
            viewport_h: 0.0,
        };
        let layout = search_panel(viewport, 0, 8, &scroll);
        // Ширина = доступная (176), не SEARCH_PANEL_WIDTH.
        assert!((layout.panel.w - (200.0 - 2.0 * PANEL_SIDE_MARGIN)).abs() < 0.01);
        // 0 результатов → results_area высотой 0, result_rows пусто.
        assert_eq!(layout.results_area.h, 0.0);
        assert!(layout.result_rows.is_empty());
        // Панель = pad + input + pad (без блока результатов).
        let expected_h = PANEL_PADDING + SEARCH_PANEL_INPUT_HEIGHT + PANEL_PADDING;
        assert!((layout.panel.h - expected_h).abs() < 0.01);
    }
}
