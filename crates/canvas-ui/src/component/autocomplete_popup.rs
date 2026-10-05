//! FR-UI-AUTOCOMPLETE: popup с anchor + flip + list_rows + ellipsis.
//!
//! Извлекает паттерн из `hints_ui` (аудит §6.1: LOW-priority kit gap):
//! всплывающее меню автодополнения, привязанное к якорю (строка каретки
//! или поле ввода), с flip при нехватке места снизу (через
//! [`dropdown_menu`]), списком строк через [`list_rows`] и ellipsis-обрезкой
//! длинных лейблов через [`TextMeasurer::ellipsis`].
//!
//! Контракт F-8 (PRD-0009 §8): кит НЕ выбирает цвет/содержимое — только
//! геометрия + подготовка лейблов (ellipsis). Потребитель рисует фон панели
//! и строки слотами своей темы (sample: `hints_ui::popup_rect`/`hint_rows` —
//! те же rect'ы, что возвращает эта функция; текст — лейблы из второго
//! возвращаемого значения). Выделение/hover — `WidgetState` потребителя.

use crate::component::dropdown::{dropdown_menu, DropdownLayout};
use crate::component::list::{list_rows, ScrollState};
use crate::geometry::{UiRect, UiVec2};
use crate::measure::TextMeasurer;

// Token re-exports (I-1: ноль визуального скачка — значения = прежним
// литералам hints_ui::popup_rect/hint_rows, файл-источник `hints_ui.rs`).
use canvas_core::tokens::SPACING_S as POPUP_PAD;
use canvas_core::tokens::SPACING_SM as POPUP_ROW_INSET_H;

/// Раскладка popup автодополнения: rect панели (clamped/flipped), rect'ы
/// строк внутри (для подсветки hover/selected), флаг flip (для анимации
/// разворота вверх).
#[derive(Debug, Clone, PartialEq)]
pub struct AutocompleteLayout {
    /// Rect панели popup (после flip+clamp через [`dropdown_menu`]).
    pub popup: UiRect,
    /// Rect'ы строк внутри popup (для подсветки hover/selected; один
    /// rect на видимую строку, без скролла — высота popup = n_visible ·
    /// row_h + 2·POPUP_PAD, контент = высоте popup).
    pub rows: Vec<UiRect>,
    /// Popup развернут НАД якорем (мало места снизу — flip кита).
    pub flipped: bool,
}

/// Раскладка popup автодополнения с привязкой к `anchor`.
///
/// `anchor` — rect контрола, к которому привязан popup (например, строка
/// каретки или поле ввода; popup раскрывается снизу от якоря, при нехватке
/// места — flip вверх — см. [`dropdown_menu`]).
///
/// `items` — лейблы строк (могут быть ellipsized, если шире popup). Все
/// `items` учитываются при вычислении popup-высоты до `max_visible`; свыше
/// `max_visible` строки НЕ показываются (потребитель ведёт скролл отдельно
/// — sample: `hints_ui::HintPopup::scroll_top`).
///
/// `max_visible` — максимум видимых строк (высота popup = min(items.len,
/// max_visible) · row_h + 2·POPUP_PAD).
/// `row_h` — высота строки (типовой = `LIST_ROW_H` = 26).
/// `popup_w` — желаемая ширина popup (минимум `anchor.w` — гарантирует
/// [`dropdown_menu`]; клампится к `viewport.w`).
/// `viewport` — для flip + clamp (потребитель кодирует поля: `viewport.x/y`
/// — внешние отступы, `viewport.right()/bottom()` — внутренние края
/// клампов; sample: `hints_ui::popup_rect` использует viewport с полями
/// `HINT_MARGIN`).
/// `family`/`size` — для измерения и ellipsis лейблов (F-6: те же метрики,
/// что у рендера; sample: `search_ui::layout_with` — `SANS_FAMILY`/13).
///
/// Возвращает `(layout, ellipsized_labels)`: consumer рисует popup фон
/// (`layout.popup`) и строки (text — `ellipsized_labels[i]` в `layout.rows[i]`).
#[allow(clippy::too_many_arguments)] // FR-UI-AUTOCOMPLETE: плоский контракт (frozen
                                     // сигнатура из спецификации Task I; группировка в
                                     // структуру лишь переносит 3 поля в `AutocompleteOpts`
                                     // без сокращения реальных параметров вызова).
pub fn autocomplete_popup(
    anchor: UiRect,
    items: &[String],
    max_visible: usize,
    row_h: f32,
    popup_w: f32,
    viewport: UiRect,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> (AutocompleteLayout, Vec<String>) {
    // 1. Число видимых строк = min(items.len, max_visible); высота popup =
    //    n_visible · row_h + 2 · POPUP_PAD (вертикальный пад сверху/снизу).
    let n_visible = items.len().min(max_visible);
    let popup_h = n_visible as f32 * row_h + 2.0 * POPUP_PAD;
    let content = UiVec2::new(popup_w, popup_h);

    // 2. flip+clamp через dropdown_menu (1:1 повтор hints_ui::popup_rect):
    //    popup снизу от якоря, при нехватке места — flip вверх, по горизонтали
    //    зажат во вьюпорт.
    let DropdownLayout {
        menu: popup,
        flipped,
    } = dropdown_menu(anchor, viewport, content);

    // 3. Область строк: popup inset по горизонтали на POPUP_ROW_INSET_H,
    //    по вертикали — на POPUP_PAD (один rect на строку высотой row_h).
    let row_area = UiRect::new(
        popup.x + POPUP_ROW_INSET_H,
        popup.y + POPUP_PAD,
        (popup.w - 2.0 * POPUP_ROW_INSET_H).max(0.0),
        (popup.h - 2.0 * POPUP_PAD).max(0.0),
    );
    // list_rows — чистая функция; без скролла (offset 0, viewport = content).
    // Частичные строки на краях НЕ включаются (content = viewport).
    let scroll = ScrollState {
        offset: 0.0,
        content_h: n_visible as f32 * row_h,
        viewport_h: row_area.h,
    };
    let rows: Vec<UiRect> = list_rows(row_area, &scroll, row_h, 0.0, n_visible)
        .into_iter()
        .map(|(_, rect)| rect)
        .collect();

    // 4. Ellipsis каждого лейбла до ширины row_area.w (popup.w - 2·inset).
    //    Текст шире row_area.w — префикс + «…» (контракт kit `ellipsis`,
    //    F-6: метрики рендера, не эвристика 0.62·кегль).
    let max_text_w = row_area.w;
    let labels: Vec<String> = items
        .iter()
        .take(n_visible)
        .map(|label| m.ellipsis(fs, label, family, size, max_text_w))
        .collect();

    (
        AutocompleteLayout {
            popup,
            rows,
            flipped,
        },
        labels,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный FontSystem: только вшитый рендером шрифт
    /// (`NotoSansDisplay-Medium.ttf`) — метрики тестов = метрикам рендера
    /// и одинаковы на всех платформах CI (паритет `measure.rs::tests`).
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    const FAMILY: &str = "Noto Sans Display";
    const SIZE: f32 = 13.0;

    /// Popup помещается ниже якоря — без flip; высота = n·row_h + 2·pad;
    /// строки стакаются сверху вниз внутри popup.
    #[test]
    fn popup_fits_below_anchor_no_flip() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = UiRect::new(100.0, 100.0, 200.0, 120.0); // строка каретки
        let items = vec!["mm1".to_owned(), "mmc".to_owned(), "mms".to_owned()];
        let (layout, labels) = autocomplete_popup(
            anchor, &items, 8, // max_visible
            24.0, 300.0, viewport, &mut m, &mut fs, FAMILY, SIZE,
        );
        // Без flip — popup снизу от якоря, через DROPDOWN_GAP (4.0).
        assert!(!layout.flipped);
        let gap = crate::component::DROPDOWN_GAP;
        assert!((layout.popup.y - (anchor.bottom() + gap)).abs() < 0.01);
        // Высота popup = 3 · 24 + 2 · POPUP_PAD.
        let expected_h = 3.0 * 24.0 + 2.0 * POPUP_PAD;
        assert!((layout.popup.h - expected_h).abs() < 0.01);
        // 3 строки в popup.
        assert_eq!(layout.rows.len(), 3);
        assert_eq!(labels.len(), 3);
        // Строки стакаются сверху вниз внутри row_area.
        let row_area_y = layout.popup.y + POPUP_PAD;
        for (i, r) in layout.rows.iter().enumerate() {
            let expected_y = row_area_y + i as f32 * 24.0;
            assert!((r.y - expected_y).abs() < 0.01, "row {i}: y={}", r.y);
            assert!((r.h - 24.0).abs() < 0.01);
        }
        // Лейблы — без ellipsis (короткие, помещаются).
        assert_eq!(labels[0], "mm1");
        assert_eq!(labels[1], "mmc");
        assert_eq!(labels[2], "mms");
    }

    /// Popup у нижнего края вьюпорта — flip вверх (над якорем); высота
    /// сохраняется, низ popup у `anchor.y - DROPDOWN_GAP`.
    #[test]
    fn popup_flips_above_when_no_space_below() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // Якорь у самого низа: popup (3·24 + 2·6 = 84) снизу не влезает.
        let anchor = UiRect::new(100.0, 560.0, 200.0, 580.0);
        let items = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
        let (layout, _labels) = autocomplete_popup(
            anchor, &items, 8, 24.0, 300.0, viewport, &mut m, &mut fs, FAMILY, SIZE,
        );
        // Flip — popup НАД якорем.
        assert!(layout.flipped, "должен flip: снизу нет места");
        let gap = crate::component::DROPDOWN_GAP;
        // Низ popup = anchor.y - DROPDOWN_GAP (flip кита: dropdown_menu).
        assert!((layout.popup.bottom() - (anchor.y - gap)).abs() < 0.01);
        // Высота та же — flip сохраняет размер (position-clamp, не обрезка).
        let expected_h = 3.0 * 24.0 + 2.0 * POPUP_PAD;
        assert!((layout.popup.h - expected_h).abs() < 0.01);
        // 3 строки всё ещё видны.
        assert_eq!(layout.rows.len(), 3);
    }

    /// Длинный лейбл ellipsized до ширины popup; лейбл, помещающийся
    /// целиком — возвращается как есть (без «…»).
    #[test]
    fn long_label_ellipsized_short_label_kept() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // Узкий anchor (caret line w=1) → popup.w = max(popup_w, anchor.w)
        // = max(120, 1) = 120 (контракт dropdown_menu: ширина ≥ якоря).
        let anchor = UiRect::new(100.0, 100.0, 1.0, 16.0);
        // Узкий popup (120 px) — длинный лейбл не помещается.
        let items = vec![
            "ok".to_owned(),
            "very_long_function_name_with_many_chars_exceeding_budget".to_owned(),
        ];
        let (layout, labels) = autocomplete_popup(
            anchor, &items, 8, 24.0, 120.0, // узкий popup
            viewport, &mut m, &mut fs, FAMILY, SIZE,
        );
        // row_area.w = popup.w - 2·POPUP_ROW_INSET_H = 120 - 16 = 104
        // (контракт dropdown_menu: popup.w = max(popup_w, anchor.w) = 120).
        let row_w = layout.rows.first().map(|r| r.w).unwrap_or(0.0);
        let max_text_w = 120.0 - 2.0 * POPUP_ROW_INSET_H;
        assert!(
            (row_w - max_text_w).abs() < 0.01,
            "row_w ({row_w}) = max_text_w ({max_text_w})"
        );
        // «ok» — короткий, без изменений.
        assert_eq!(labels[0], "ok");
        // Длинный — ellipsized (хвост «…»).
        assert!(
            labels[1].ends_with('\u{2026}'),
            "длинный лейбл ellipsized: {}",
            labels[1]
        );
        // Ширина ellipsized лейбла ≤ max_text_w (контракт kit `ellipsis`).
        let w = m.width_of(&mut fs, &labels[1], FAMILY, SIZE);
        assert!(
            w <= max_text_w + 0.5,
            "ellipsized ширина ({w}) ≤ бюджета ({max_text_w})"
        );
        // Ширина «ok» тоже ≤ бюджета.
        let w_short = m.width_of(&mut fs, &labels[0], FAMILY, SIZE);
        assert!(w_short <= max_text_w);
        // Число строк = 2 (оба видны).
        assert_eq!(layout.rows.len(), 2);
    }
}
