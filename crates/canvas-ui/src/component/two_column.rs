//! FR-UI-TWO-COLUMN: [sidebar | content] layout.
//!
//! Извлекает паттерн из `calc_panel_ui`, `admin_ui`, `settings_ui`
//! (аудит §6.1: MEDIUM-priority kit gap): двух-колоночная раскладка с
//! фиксированной боковой колонкой и заполняющим контентом. Канон:
//! `sidebar` прижат к левому (или правому) краю `slot`, `content`
//! заполняет остаток с зазором `gap` (типовой зазор — `GAP_CONTROLS`
//! /`SPACING_MD` = 8, потребитель передаёт свой).
//!
//! Контракт F-8 (PRD-0009 §8): кит НЕ выбирает цвет/содержимое —
//! только геометрия. Стили панелей — `panel_style(palette)` от
//! потребителя; hover/selected строк sidebar'а — `WidgetState`'ы
//! потребителя (кит не ведёт; образец `settings_ui`/`admin_ui`).

use crate::geometry::UiRect;

/// Раскладка двухколоночной панели: [sidebar | content] (или
/// [content | sidebar] у [`two_column_right`]).
///
/// Инвариант: `sidebar.right() < content.x` (или `content.right() <
/// sidebar.x` у правого варианта) — колонки НЕ перекрываются; зазор
/// `gap` между ними. Ширина `content` = `slot.w − sidebar_w − gap`
/// (если `sidebar_w + gap > slot.w` — content вырождается в пустой
/// rect `w = 0`: класс переполнения, детектируется G4-линтом, не
/// маскируется).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoColumnLayout {
    /// Боковая колонка (sidebar).
    pub sidebar: UiRect,
    /// Основной контент.
    pub content: UiRect,
}

/// Раскладка [sidebar | content] — sidebar слева, content справа.
///
/// `slot` — внешняя область (родительский слот панели). `sidebar_w` —
/// фиксированная ширина боковой колонки. `gap` — зазор между sidebar и
/// content (передавайте `GAP_CONTROLS` = 8 или `SPACING_MD` = 8).
///
/// Высоты обеих колонок = `slot.h` (наследуется). Если `sidebar_w +
/// gap > slot.w`, content вырождается в пустой rect (класс
/// переполнения — G4-линт, не маскируется).
pub fn two_column(slot: UiRect, sidebar_w: f32, gap: f32) -> TwoColumnLayout {
    let sidebar = UiRect::new(slot.x, slot.y, sidebar_w, slot.h);
    let content = UiRect::new(
        slot.x + sidebar_w + gap,
        slot.y,
        (slot.w - sidebar_w - gap).max(0.0),
        slot.h,
    );
    TwoColumnLayout { sidebar, content }
}

/// Вариант [content | sidebar] — sidebar справа, content слева.
///
/// Симметричен [`two_column`]: sidebar прижат к правому краю `slot`
/// (`sidebar.right() == slot.right()`), content заполняет левую часть.
/// Тот же инвариант «колонки не перекрываются» (зазор `gap` между
/// ними).
pub fn two_column_right(slot: UiRect, sidebar_w: f32, gap: f32) -> TwoColumnLayout {
    let content = UiRect::new(slot.x, slot.y, (slot.w - sidebar_w - gap).max(0.0), slot.h);
    let sidebar = UiRect::new(slot.right() - sidebar_w, slot.y, sidebar_w, slot.h);
    TwoColumnLayout { sidebar, content }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Левый sidebar: sidebar прижат к левому краю slot; content —
    /// правее с зазором gap; ширина content = slot.w − sidebar_w − gap;
    /// колонки не перекрываются.
    #[test]
    fn left_sidebar_layout_no_overlap_correct_widths() {
        let slot = UiRect::new(10.0, 20.0, 500.0, 400.0);
        let layout = two_column(slot, 200.0, 8.0);
        // Sidebar прижат к левому краю slot, ширина = 200, высота = slot.h.
        assert!((layout.sidebar.x - slot.x).abs() < 0.01);
        assert!((layout.sidebar.y - slot.y).abs() < 0.01);
        assert!((layout.sidebar.w - 200.0).abs() < 0.01);
        assert!((layout.sidebar.h - slot.h).abs() < 0.01);
        // Content правее sidebar с зазором gap; ширина = остаток.
        assert!((layout.content.x - (slot.x + 200.0 + 8.0)).abs() < 0.01);
        assert!((layout.content.w - (500.0 - 200.0 - 8.0)).abs() < 0.01);
        // Ключевое: колонки НЕ перекрываются (sidebar.right() + gap == content.x).
        assert!(
            (layout.sidebar.right() + 8.0 - layout.content.x).abs() < 0.01,
            "gap = 8 между sidebar и content"
        );
        // Content доходит до правого края slot.
        assert!((layout.content.right() - slot.right()).abs() < 0.01);
    }

    /// Правый sidebar: sidebar прижат к правому краю slot; content —
    /// левее с зазором gap; ширина content = slot.w − sidebar_w − gap;
    /// колонки не перекрываются.
    #[test]
    fn right_sidebar_layout_no_overlap_correct_widths() {
        let slot = UiRect::new(0.0, 0.0, 600.0, 500.0);
        let layout = two_column_right(slot, 180.0, 8.0);
        // Sidebar прижат к правому краю slot.
        assert!((layout.sidebar.right() - slot.right()).abs() < 0.01);
        assert!((layout.sidebar.w - 180.0).abs() < 0.01);
        // Content левее sidebar с зазором gap.
        assert!((layout.content.x - slot.x).abs() < 0.01);
        assert!((layout.content.right() + 8.0 - layout.sidebar.x).abs() < 0.01);
        assert!((layout.content.w - (600.0 - 180.0 - 8.0)).abs() < 0.01);
        // Ключевое: колонки НЕ перекрываются.
        assert!(layout.content.right() < layout.sidebar.x);
    }

    /// Переполнение: sidebar_w + gap > slot.w — content вырождается в
    /// пустой rect (w = 0), sidebar остаётся с полной шириной; класс
    /// переполнения НЕ маскируется (G4-линт).
    #[test]
    fn overflow_content_degenerates_to_empty_rect() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 200.0);
        let layout = two_column(slot, 120.0, 8.0);
        // sidebar_w (120) > slot.w (100) → sidebar шире slot (UiRect
        // не нормализует это; валидация — на потребителе).
        assert!((layout.sidebar.w - 120.0).abs() < 0.01);
        // Content — пустой (w = 0), не отрицательный.
        assert_eq!(layout.content.w, 0.0);
        assert!(
            !layout.content.is_empty() || layout.content.h > 0.0,
            "content — пустой по ширине, но высота сохранена"
        );
    }
}
