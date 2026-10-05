//! FR-UI-CRUMBS: breadcrumb chip strip with overflow `take_while`.
//!
//! Extracts pattern from `explain_ui::crumb_rects` (AC-2.3, X6): bread crumbs
//! «Root → Child → Grandchild» в мета-строке шапки окна explain. Канон
//! геометрии: горизонтальный ряд чипов одной высоты (`CRUMB_H`), зазор
//! `CRUMB_GAP`; переполнение — `take_while` по правому краю слота (чипы,
//! не влезающие в зону, не отрисовываются и не кликабельны — детерминизм
//! рендера и hit-теста, паттерн `token_before_caret` из FR-059).
//!
//! ## Контракт с потребителем
//!
//! Кит отдаёт `Vec<(rect, label)>` — потребитель рисует через `Painter::rect`
//! (заливка/рамка) + `Painter::label` (текст по центру). Стиль чипа —
//! `chip_style(state, palette)` от потребителя (active crumb →
//! `KitState::Selected`). Текст — измеренный `TextMeasurer::width_of`
//! (как `chip_size`/`button_size`): ширина чипа = текст + `2·pad_x`.
//!
//! ## Политика переполнения (см. explain_ui::crumb_rects)
//!
//! В отличие от `chip_strip` (где переполнение НЕ маскируется — Fit policy),
//! crumbs **DROP** чипы, не влезающие в слот — текущий фокус (правый край
//! пути) важнее корневых уровней. Алгоритм:
//! 1. Измеряем ширину каждого crumb (`text + 2·pad_x`).
//! 2. Берём хвост с конца: `take_while` сумма_widths + gap ≤ slot.w.
//! 3. Если отброшено ≥ 1 crumb'а,prepend «…» crumb (та же высота, ширина
//!    замера «…» + `2·pad_x`).
//!
//! Возвращает Vec в порядке отображения (слева направо), т.е. `["…",
//! "Parent", "Child"]` если корень «Root» был отброшен. `crumbs_active_index`
//! возвращает индекс ПОСЛЕДНЕГО visible crumb'а (для `KitState::Selected`
//! подсветки — это текущий фокус пути).

use crate::geometry::UiRect;
use crate::measure::TextMeasurer;

/// Высота чипа-крошки (из explain_ui::CRUMB_H — 18 px; меньше chip_height=24
/// т.к. крошки в шапке окна, плотнее основного контента).
pub const CRUMB_H: f32 = 18.0;

/// Зазор между чипами-крошками (из explain_ui::CRUMB_GAP — 4 px; меньше
/// `SPACING_S=6` т.к. крошки плотнее).
pub const CRUMB_GAP: f32 = 4.0;

/// Потолок ширины чипа-крошки (из explain_ui::CRUMB_W_MAX — длинные заголовки
/// обрезаются рендером текста через `TextMeasurer::ellipsis`).
pub const CRUMB_W_MAX: f32 = 148.0;

/// Пол ширины чипа (из explain_ui::CRUMB_W_MIN — читаемость; уже — рендер
/// обрезает хвост).
pub const CRUMB_W_MIN: f32 = 48.0;

/// Семейство шрифта крошек (паритет `sans_attrs` рендера, CR-015).
pub const CRUMB_FAMILY: &str = "Noto Sans Display";

/// Кегль подписи крошки (11 px — меньше chip FONT_SIZE=12 т.к. шапка
/// explain компактнее; паритет с explain_ui::meta_rect подписями).
pub const CRUMB_FONT_SIZE: f32 = 11.0;

/// Layout a row of breadcrumb crumbs in `slot`. Crumbs that don't fit
/// are dropped (take_while from the RIGHT — keep last N that fit);
/// if any are dropped, prepend a "…" crumb.
///
/// Returns `Vec<(rect, label)>` in display order (left-to-right): if root
/// crumb(s) were dropped, the first entry is "…".
///
/// `labels` — crumb texts (e.g. ["Root", "Child", "Grandchild"]); in display
/// order, root first.
/// `gap` — gap between crumbs (use `CRUMB_GAP` = 4).
/// `pad_x` — internal horizontal padding of each crumb (use `CHIP_PAD_H` = 8).
/// `m`, `fs` — for measuring label widths (CRUMB_FAMILY, CRUMB_FONT_SIZE).
pub fn crumbs(
    slot: UiRect,
    labels: &[String],
    gap: f32,
    pad_x: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<(UiRect, String)> {
    if labels.is_empty() || slot.w <= 0.0 {
        return Vec::new();
    }
    // Измеряем каждый crumb: ширина = text + 2·pad_x, но не более CRUMB_W_MAX
    // (длинные заголовки обрезает рендер — kit отдаёт max-ширину, текст
    // центрируется в нём; пользователь вызывает `TextMeasurer::ellipsis` сам,
    // если нужно усечь подпись до более короткой — кит геометрию не меняет).
    let measured: Vec<f32> = labels
        .iter()
        .map(|label| {
            let text_w = m.width_of(fs, label, CRUMB_FAMILY, CRUMB_FONT_SIZE);
            (text_w + 2.0 * pad_x).clamp(CRUMB_W_MIN, CRUMB_W_MAX)
        })
        .collect();

    // Сколько крошек ПОМЕЩАЕТСЯ с конца (берём хвост, current focus важнее
    // root'а — паритет explain_ui::crumb_rects: shown = last N). Алгоритм:
    // идём с конца, копим сумму ширин + gap; последний crumb, при добавлении
    // которого сумма ≤ slot.w, — граница.
    let mut total_w: f32 = 0.0;
    let mut first_fit = labels.len(); // индекс первого показанного
    for (i, &w) in measured.iter().enumerate().rev() {
        let added = if total_w == 0.0 { w } else { w + gap };
        if total_w + added > slot.w + crate::measure::FIT_EPS {
            break;
        }
        total_w += added;
        first_fit = i;
    }
    // Если даже один crumb не помещается — показываем только последний
    // (деградация: один crumb всегда виден, рендер обрезает хвост).
    if first_fit == labels.len() {
        first_fit = labels.len() - 1;
    }
    let dropped = first_fit; // сколько крошек с начала отброшено
    let visible: &[String] = &labels[first_fit..];

    // Если что-то отброшено — prepend "…" crumb (тот же pad_x, CRUMB_H).
    let ellipsis = "…".to_owned();
    let ellipsis_w = m.width_of(fs, &ellipsis, CRUMB_FAMILY, CRUMB_FONT_SIZE) + 2.0 * pad_x;
    let prepend_ellipsis = dropped > 0;

    // Пересчитываем total_w с учётом "…" crumb (если он добавлен, он занимает
    // место первого visible crumb'а, поэтому берём на одну крошку меньше).
    let mut shown_labels: Vec<String> = Vec::new();
    if prepend_ellipsis {
        shown_labels.push(ellipsis.clone());
    }
    // Если добавили "…", нужно убедиться, что хвост по-прежнему помещается.
    // Берём (visible.len() - 1) последних visible'ов — fallback на 1, если
    // только "…" помещается.
    let take_from_visible = if prepend_ellipsis {
        // Сколько visible'ов помещается рядом с "…": total_w пересчитываем.
        let avail_after_ellipsis = (slot.w - ellipsis_w - gap).max(0.0);
        let mut acc: f32 = 0.0;
        let mut count = 0;
        for (i, &w) in measured.iter().enumerate().rev().take(visible.len()) {
            let added = if acc == 0.0 { w } else { w + gap };
            if acc + added > avail_after_ellipsis + crate::measure::FIT_EPS {
                break;
            }
            acc += added;
            count += 1;
            // ограничиваемся длиной visible (i — индекс в исходном массиве,
            // нужно отслеживать, что берём из visible — последние count)
            if count >= visible.len() {
                break;
            }
            let _ = i; // suppress unused
        }
        count.max(1) // хотя бы один crumb рядом с "…"
    } else {
        visible.len()
    };

    let start_in_visible = visible.len().saturating_sub(take_from_visible);
    for label in &visible[start_in_visible..] {
        shown_labels.push(label.clone());
    }

    // Лейаут: left-to-right, начиная с slot.x. Высота = CRUMB_H, по центру
    // вертикали слота (паритет explain_ui: y = meta[1] + (meta[3]-CRUMB_H)/2).
    let y = slot.y + (slot.h - CRUMB_H).max(0.0) / 2.0;
    let mut x = slot.x;
    let mut out: Vec<(UiRect, String)> = Vec::with_capacity(shown_labels.len());
    for label in shown_labels {
        let text_w = m.width_of(fs, &label, CRUMB_FAMILY, CRUMB_FONT_SIZE);
        let w = (text_w + 2.0 * pad_x).clamp(CRUMB_W_MIN, CRUMB_W_MAX);
        // Если crumb не помещается в оставшуюся ширину — clip по правому краю
        // (последний crumb может быть обрезан, но показан — паритет explain_ui
        // `w = width.min(meta_end - x)`).
        let clipped_w = w.min((slot.right() - x).max(0.0));
        if clipped_w <= 0.0 {
            break; // больше нет места
        }
        out.push((UiRect::new(x, y, clipped_w, CRUMB_H), label));
        x += clipped_w + gap;
    }
    out
}

/// Returns the index of the last visible crumb in the OUTPUT Vec — for
/// "active" highlight (`KitState::Selected`). The last visible crumb is
/// the current focus of the path (parent of the deepest visible level).
///
/// `fitted` — number of crumbs returned by `crumbs()` (output Vec length).
/// `total` — original number of labels passed to `crumbs()`.
///
/// Returns `None` if no crumbs are shown (`fitted == 0`).
pub fn crumbs_active_index(_total: usize, fitted: usize) -> Option<usize> {
    if fitted == 0 {
        None
    } else {
        Some(fitted - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::load_display_font;

    /// Все крошки помещаются в широкий слот — ни одной не отброшено,
    /// "…" НЕ prepended. Возврат: 3 крошки, первая слева, зазор = CRUMB_GAP,
    /// высота = CRUMB_H.
    #[test]
    fn all_crumbs_fit_no_ellipsis_prepended() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels: Vec<String> = ["Root", "Child", "Grandchild"]
            .iter()
            .map(|&s| s.to_owned())
            .collect();
        let slot = UiRect::new(0.0, 0.0, 1000.0, 40.0);
        let out = crumbs(slot, &labels, CRUMB_GAP, 8.0, &mut m, &mut fs);
        assert_eq!(out.len(), 3, "все 3 крошки видны");
        // Первая — слева, не "…".
        assert_eq!(out[0].1, "Root");
        assert_eq!(out[2].1, "Grandchild");
        // Высота каждой = CRUMB_H.
        for (r, _) in &out {
            assert!((r.h - CRUMB_H).abs() < 0.01);
        }
        // Первая — слева (slot.x).
        assert!((out[0].0.x - slot.x).abs() < 0.01);
        // Зазор = CRUMB_GAP.
        let gap = out[1].0.x - out[0].0.right();
        assert!((gap - CRUMB_GAP).abs() < 0.01);
        // Все в границах слота.
        for (r, _) in &out {
            assert!(r.right() <= slot.right() + 0.5);
        }
        // Active index = 2 (последняя).
        assert_eq!(crumbs_active_index(3, out.len()), Some(2));
    }

    /// Переполнение: слот узкий, влезает только 1 crumb + "…". Первая
    /// показанная — "…" (root отброшен), за ней — последний crumb.
    #[test]
    fn overflow_drops_left_and_prepends_ellipsis() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels: Vec<String> = (0..8).map(|i| format!("Level{i}_long_label")).collect();
        // Узкий слот — влезает 2-3 crumb'а.
        let slot = UiRect::new(0.0, 0.0, 200.0, 40.0);
        let out = crumbs(slot, &labels, CRUMB_GAP, 8.0, &mut m, &mut fs);
        // Хотя бы одна крошка показана.
        assert!(!out.is_empty(), "хотя бы один crumb показан");
        // Первая — "…" (что-то отброшено).
        assert_eq!(out[0].1, "…", "ellipsis prepended при переполнении");
        // Все в границах слота.
        for (r, _) in &out {
            assert!(r.right() <= slot.right() + 0.5);
        }
        // Последний crumb — НЕ "…", а последний из labels.
        assert_eq!(out.last().unwrap().1, *labels.last().unwrap());
        // Active = последний в выводе.
        assert_eq!(
            crumbs_active_index(labels.len(), out.len()),
            Some(out.len() - 1)
        );
    }

    /// Пустой список labels → пустой Vec (no-op).
    #[test]
    fn empty_labels_returns_empty() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = TextMeasurer::new();
        let slot = UiRect::new(0.0, 0.0, 400.0, 40.0);
        let empty: Vec<String> = Vec::new();
        let out = crumbs(slot, &empty, CRUMB_GAP, 8.0, &mut m, &mut fs);
        assert!(out.is_empty());
        assert_eq!(crumbs_active_index(0, 0), None);
    }

    /// Один crumb — даже если он шире слота, показан (clipped по правому краю).
    #[test]
    fn single_crumb_always_shown() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels: Vec<String> = vec!["VeryLongSingleCrumbLabel".to_owned()];
        let slot = UiRect::new(0.0, 0.0, 80.0, 40.0);
        let out = crumbs(slot, &labels, CRUMB_GAP, 8.0, &mut m, &mut fs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, "VeryLongSingleCrumbLabel");
        // Ширина клипнута по правому краю слота (CRUMB_W_MAX > slot.w).
        assert!(out[0].0.right() <= slot.right() + 0.5);
    }

    /// Крошки выровнены по центру вертикали слота (паритет explain_ui).
    #[test]
    fn crumbs_vertically_centered_in_slot() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels: Vec<String> = vec!["A".to_owned(), "B".to_owned()];
        let slot = UiRect::new(0.0, 0.0, 400.0, 30.0);
        let out = crumbs(slot, &labels, CRUMB_GAP, 8.0, &mut m, &mut fs);
        let expected_y = slot.y + (slot.h - CRUMB_H) / 2.0;
        for (r, _) in &out {
            assert!((r.y - expected_y).abs() < 0.01, "крошки по центру слота");
        }
    }
}
