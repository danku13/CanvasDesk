//! FR-UI-FOOTER: 3-button right-aligned footer pattern.
//!
//! Извлекает паттерн из `autolink_ui`, `onboarding`, `settings`,
//! `graph_builder_ui` (3-кнопочный правый футер: Cancel/OK, Prev/Next,
//! Generate/Cancel). Канон: правый край последней кнопки = правый край слота
//! (потребитель inset'ит слот сам — кит не добавляет дополнительный inset,
//! API остаётся минимальным); зазор между кнопками = `GAP_CONTROLS`.
//!
//! Контракт F-8 (PRD-0009 §8): кит НЕ выбирает цвет/текст — только
//! геометрия. Стиль кнопок — `button_style(variant, state, palette)` от
//! потребителя (он же передаёт варианты Primary/Secondary/Danger по
//! индексу). Текст — через `TextMeasurer`.
//!
//! FR-UI-FOOTER-R: удалён простой вариант `footer_buttons(slot, n)` с
//! фиксированной `BUTTON_WIDTH` (0 потребителей — все consumer'ы
//! используют измеряемый `footer_buttons_measured` с переменными ширинами
//! или `split_footer_buttons` для split-паттерна «одна кнопка слева,
//! другая справа»). Измеряемый вариант строго лучше: переменные ширины
//! учитывать реальные подписи кнопок (CR-015: без эвристик «длина × 0.62»).

use crate::component::{BUTTON_HEIGHT, GAP_CONTROLS};
use crate::geometry::UiRect;
use crate::measure::TextMeasurer;

/// Вариант с измеренными ширинами кнопок (для подписей переменной длины).
///
/// `widths[i]` — измеренная ширина кнопки `i` (текст + `2·BUTTON_PAD_H`,
/// потребитель измеряет через `TextMeasurer::width_of` + `BUTTON_PAD_H·2`
/// или [`crate::component::button::button_size`] которая возвращает готовую
/// ширину). Высота каждой кнопки = [`BUTTON_HEIGHT`]; зазор = [`GAP_CONTROLS`].
///
/// Правый край последней кнопки = `slot.right()` (inset = 0 в каноне кита,
/// как у бывшей `footer_buttons` — простой вариант удалён за отсутствием
/// потребителей). Возвращает `Vec<(rect, index)>` в порядке слева-направо.
/// Если кнопки не помещаются в слот — переполнение НЕ маскируется (G4-линт).
///
/// `_m` зарезервирован для будущей версии с встроенным замером (сегодня
/// потребитель измеряет сам и передаёт `widths`); сохранён в сигнатуре
/// для симметрии с `Row::lay_out_measured` и однозначного API.
#[allow(clippy::unused_self)] // _m зарезервирован (см. doc)
pub fn footer_buttons_measured(
    slot: UiRect,
    widths: &[f32],
    _m: &mut TextMeasurer,
) -> Vec<(UiRect, usize)> {
    let h = BUTTON_HEIGHT;
    let gap = GAP_CONTROLS;
    let y = slot.y + (slot.h - h).max(0.0) / 2.0;
    let total_w: f32 = widths.iter().sum::<f32>() + (widths.len().saturating_sub(1) as f32) * gap;
    let start_x = slot.right() - total_w;
    let mut x = start_x;
    widths
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let r = UiRect::new(x, y, w.max(0.0), h);
            x += w + gap;
            (r, i)
        })
        .collect()
}

// === FR-UI-SPLIT-FOOTER: split footer (left-aligned + right-aligned) =======

/// Группа кнопок в split-footer (см. [`split_footer_buttons`]).
///
/// `Left` — левая группа (выровнена по левому краю слота + inset);
/// `Right` — правая группа (выровнена по правому краю слота - inset).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterGroup {
    Left,
    Right,
}

/// FR-UI-SPLIT-FOOTER: left-aligned + right-aligned buttons in one slot.
///
/// Closes TODO J/FR-UI-FOOTER — onboarding carousel footer (Prev left,
/// Next right) и AI-onboarding footer (Privacy left, Continue right).
/// `footer_buttons` раскладывает N кнопок right-aligned (последняя flush
/// к `slot.right`) — НЕ подходит для split-паттерна «одна кнопка слева,
/// другая справа» (см. комментарий J в `onboarding_ui.rs:355` и
/// `onboarding_ui.rs:699`).
///
/// Раскладка:
/// - **Left group**: кнопки слева-направо от `slot.x + inset` с зазором
///   `gap` между соседями (первая кнопка `.x = slot.x + inset`);
/// - **Right group**: кнопки справа-налево от `slot.right - inset` с
///   тем же зазором (первая правая кнопка `.right = slot.right - inset`,
///   следующая — на `gap` левее);
/// - **Vertical**: все кнопки высотой `BUTTON_HEIGHT`, центрированы по
///   вертикали в слоте (как [`footer_buttons`]).
///
/// Возвращает `Vec<(rect, group, index)>` в порядке left-group → right-group
/// (внутри группы — left-to-right / right-to-left соответственно,
/// `index` — позиция внутри группы). Переполнение НЕ маскируется: если
/// сумма ширин двух групп + 2·inset + пересечение больше `slot.w` —
/// правая и левая группы накладываются (ловится G4-линтом, потребитель
/// обязан обеспечить достаточную ширину слота).
pub fn split_footer_buttons(
    slot: UiRect,
    left_button_widths: &[f32],
    right_button_widths: &[f32],
    gap: f32,
    inset: f32,
) -> Vec<(UiRect, FooterGroup, usize)> {
    let h = BUTTON_HEIGHT;
    let y = slot.y + (slot.h - h).max(0.0) / 2.0;
    let mut out = Vec::with_capacity(left_button_widths.len() + right_button_widths.len());
    // Left group: слева-направо от slot.x + inset.
    let mut x_left = slot.x + inset;
    for (i, &w) in left_button_widths.iter().enumerate() {
        let r = UiRect::new(x_left, y, w.max(0.0), h);
        out.push((r, FooterGroup::Left, i));
        x_left += w + gap;
    }
    // Right group: справа-налево от slot.right - inset.
    let mut x_right = slot.right() - inset;
    for (i, &w) in right_button_widths.iter().enumerate() {
        let x = x_right - w;
        let r = UiRect::new(x, y, w.max(0.0), h);
        out.push((r, FooterGroup::Right, i));
        x_right = x - gap;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// footer_buttons_measured: переменные ширины — кнопки right-aligned,
    /// правый край последней = slot.right().
    #[test]
    fn measured_widths_right_aligned() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 50.0);
        let mut m = TextMeasurer::new();
        let widths = [80.0, 120.0, 60.0];
        let buttons = footer_buttons_measured(slot, &widths, &mut m);
        assert_eq!(buttons.len(), 3);
        // Ширины сохранены.
        for (i, &(r, _)) in buttons.iter().enumerate() {
            assert!((r.w - widths[i]).abs() < 0.01, "btn[{i}].w");
        }
        // Правый край последней = slot.right().
        let last = buttons[2].0;
        assert!((last.right() - slot.right()).abs() < 0.01);
        // Зазор между кнопками = GAP_CONTROLS.
        let gap1 = buttons[1].0.x - buttons[0].0.right();
        assert!((gap1 - GAP_CONTROLS).abs() < 0.01);
        let gap2 = buttons[2].0.x - buttons[1].0.right();
        assert!((gap2 - GAP_CONTROLS).abs() < 0.01);
    }

    /// n = 0: пустой Vec (no-op).
    #[test]
    fn zero_buttons_returns_empty() {
        let slot = UiRect::new(0.0, 0.0, 300.0, 50.0);
        let mut m = TextMeasurer::new();
        assert!(footer_buttons_measured(slot, &[], &mut m).is_empty());
    }

    // === FR-UI-SPLIT-FOOTER: split footer (left-aligned + right-aligned) ===

    /// 1 left + 1 right: левая кнопка `.x = slot.x + inset`, правая кнопка
    /// `.right = slot.right - inset`; высота = `BUTTON_HEIGHT`,
    /// вертикально центрированы в слоте. Паритет с `onboarding_ui::button_rect`
    /// (Prev: `card.x + ONBOARDING_PAD`, Next: `card.x + card.w - ONBOARDING_PAD - W`).
    #[test]
    fn split_footer_one_left_one_right_at_opposite_edges() {
        let slot = UiRect::new(0.0, 0.0, 500.0, 60.0);
        let inset = 12.0;
        let gap = GAP_CONTROLS;
        let left_w = 80.0; // Prev
        let right_w = 100.0; // Next
        let buttons = split_footer_buttons(slot, &[left_w], &[right_w], gap, inset);
        assert_eq!(buttons.len(), 2);
        // Первая запись — left group, index 0.
        assert_eq!(buttons[0].1, FooterGroup::Left);
        assert_eq!(buttons[0].2, 0);
        // Вторая — right group, index 0.
        assert_eq!(buttons[1].1, FooterGroup::Right);
        assert_eq!(buttons[1].2, 0);
        // Левая: x = slot.x + inset.
        let (left_rect, _, _) = buttons[0];
        assert!(
            (left_rect.x - (slot.x + inset)).abs() < 0.01,
            "Prev у левого края + inset"
        );
        assert!((left_rect.w - left_w).abs() < 0.01);
        assert!((left_rect.h - BUTTON_HEIGHT).abs() < 0.01);
        // Y — центр слота.
        let y_expected = slot.y + (slot.h - BUTTON_HEIGHT) / 2.0;
        assert!((left_rect.y - y_expected).abs() < 0.01);
        // Правая: right = slot.right - inset.
        let (right_rect, _, _) = buttons[1];
        assert!(
            (right_rect.right() - (slot.right() - inset)).abs() < 0.01,
            "Next у правого края - inset"
        );
        assert!((right_rect.w - right_w).abs() < 0.01);
        assert!((right_rect.y - y_expected).abs() < 0.01);
    }
}
