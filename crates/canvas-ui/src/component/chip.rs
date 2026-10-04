//! FR-UI-CHIP-STRIP: Row из Fit-чипов с опциональным «All» preset.
//!
//! Извлекает паттерн из `scheme_gallery_ui`, `template_ui`, `settings_ui`
//! (фильтр-чипы по категориям / preset-чипы настроек). Канон: каждый чип —
//! измеренный текст + `CHIP_PAD_H·2` пад по горизонтали, высота
//! `CHIP_HEIGHT`; зазор = `SPACING_S` (бывший `BAR_GAP` whatif_ui.rs);
//! политика `RowPolicy::Fit` (по умолчанию — переполнение НЕ маскируется,
//! ловится G4-линтом) или `RowPolicy::SqueezeTail` (если `squeeze=true` —
//! хвост сжимается до 0, как бывший молчаливый `break`-кламп чипов галереи,
//! но теперь именованный и явный).
//!
//! Контракт F-8 (PRD-0009 §8): кит НЕ выбирает цвет — только геометрия.
//! Стиль чипа — `chip_style(state, palette)` от потребителя (передаёт
//! `KitState::Selected` для активного чипа и т.д.). Текст — через
//! `TextMeasurer` (здесь — измерение ширины для раскладки).

use crate::component::{CHIP_HEIGHT, CHIP_PAD_H};
use crate::geometry::UiRect;
use crate::layout::{CrossAlign, MeasuredItem, Row, RowPolicy};
use crate::measure::TextMeasurer;

/// Семейство шрифта чипов по умолчанию (то же имя, что `Family::Name`
/// рендера — паритет `sans_attrs`, CR-015: замер тем же лицом, что и
/// отрисовка). Совпадает с `BUTTON_FAMILY` из `button.rs`.
pub const CHIP_FAMILY: &str = "Noto Sans Display";

/// Кегль подписи чипа (12 px — прежний кегль чипов галереи/whatif;
/// `BAR_FONT_SIZE` whatif_ui.rs).
pub const CHIP_FONT_SIZE: f32 = 12.0;

/// Зазор между чипами в полосе (`SPACING_S` = 6 — бывший `BAR_GAP`
/// whatif_ui.rs / зазор чипов галереи).
pub const CHIP_GAP: f32 = canvas_core::tokens::SPACING_S;

/// Раскладка полосы чипов: возвращает `Vec<(chip_rect, &str)>` в порядке
/// `items`. Каждый чип измерен через `TextMeasurer::width_of` (text +
/// `2·CHIP_PAD_H`), высота `CHIP_HEIGHT`. Политика:
/// - `squeeze = false` (по умолчанию) → `RowPolicy::Fit`: чипы идут подряд
///   от левого края слота; переполнение НЕ маскируется (выходит за правый
///   край, ловится G4-линтом — как было в `scheme_gallery_ui` до
///   миграции `break`-кламп).
/// - `squeeze = true` → `RowPolicy::SqueezeTail`: хвост сжимается до 0 при
///   нехватке места (именованная деградация — явная замена бывшего
///   молчаливого `break`-кламп).
///
/// «All» preset — это просто первый item (label «Все»/«All»); кит не
/// различает «All» и обычные чипы — потребитель решает семантику
/// (chip_style(Selected) для All при category=None).
///
/// Порядок items сохранён: `result[i]` соответствует `items[i]`.
pub fn chip_strip<'a>(
    slot: UiRect,
    items: &'a [&str],
    squeeze: bool,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<(UiRect, &'a str)> {
    if items.is_empty() {
        return Vec::new();
    }
    // Измеряем каждого ребёнка как MeasuredItem::Text с pad_x = 2·CHIP_PAD_H
    // (бит-в-бит эквивалент `chip_size`: text + CHIP_PAD_H·2) и явной
    // высотой `CHIP_HEIGHT` (измеренная высота 12·1.3 ≈ 15.6 ≠ CHIP_HEIGHT=24
    // — дизайнерская константа, не измеренная).
    let measured: Vec<MeasuredItem> = items
        .iter()
        .map(|&label| MeasuredItem::Text {
            text: label,
            max_w: None, // без клампа — Fit policy surfaces overflow (G5)
            min_w: 0.0,
            pad_x: CHIP_PAD_H * 2.0,
            h: Some(CHIP_HEIGHT),
        })
        .collect();
    let row = Row {
        gap: CHIP_GAP,
        cross: CrossAlign::Start,
        policy: if squeeze {
            RowPolicy::SqueezeTail
        } else {
            RowPolicy::Fit
        },
        ..Row::default()
    };
    let rects = row.lay_out_measured(slot, &measured, m, fs, CHIP_FAMILY, CHIP_FONT_SIZE);
    rects
        .into_iter()
        .zip(items.iter())
        .map(|(r, &label)| (r, label))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::load_display_font;

    /// 3 чипа в широком слоте помещаются: первый слева, зазор = SPACING_S,
    /// высота = CHIP_HEIGHT, ширина = text + 2·CHIP_PAD_H.
    #[test]
    fn three_chips_fit_in_wide_slot() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels = ["Все", "API", "DB"];
        let slot = UiRect::new(0.0, 0.0, 400.0, 40.0);
        let chips = chip_strip(slot, &labels, false, &mut m, &mut fs);
        assert_eq!(chips.len(), 3);
        // Первый чип — слева (slot.x).
        assert!((chips[0].0.x - slot.x).abs() < 0.01, "первый слева");
        // Высота каждого = CHIP_HEIGHT.
        for &(r, _) in &chips {
            assert!((r.h - CHIP_HEIGHT).abs() < 0.01);
        }
        // Ширина первого = text_width + 2·CHIP_PAD_H.
        let text_w = m.width_of(&mut fs, "Все", CHIP_FAMILY, CHIP_FONT_SIZE);
        let expected_w = text_w + 2.0 * CHIP_PAD_H;
        assert!(
            (chips[0].0.w - expected_w).abs() < 0.5,
            "ширина чипа = text + 2·pad (допуск round_layout 0.5)"
        );
        // Зазор между чипами = SPACING_S (CHIP_GAP).
        let gap = chips[1].0.x - chips[0].0.right();
        assert!((gap - CHIP_GAP).abs() < 0.01, "зазор = SPACING_S");
        // Все чипы в границах слота.
        for &(r, _) in &chips {
            assert!(
                r.right() <= slot.right() + 0.01,
                "чипы в слоте (нет переполнения)"
            );
        }
    }

    /// Переполнение НЕ маскируется при `squeeze=false` (RowPolicy::Fit):
    /// последний чип выходит за правый край слота (ловится G4-линтом).
    #[test]
    fn fit_policy_does_not_mask_overflow() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        // Узкий слот + много чипов → переполнение.
        let labels = ["Очень длинная категория", "API", "DB", "Cache", "Network"];
        let slot = UiRect::new(0.0, 0.0, 100.0, 40.0);
        let chips = chip_strip(slot, &labels, false, &mut m, &mut fs);
        assert_eq!(chips.len(), labels.len(), "все чипы возвращены");
        // Хотя бы один чип выходит за правый край слота.
        let overflow = chips.iter().any(|(r, _)| r.right() > slot.right() + 0.01);
        assert!(overflow, "переполнение НЕ маскируется — ловится G4-линтом");
    }

    /// SqueezeTail: хвост сжимается до 0 при нехватке места — ни один чип
    /// не выходит за правый край слота.
    #[test]
    fn squeeze_tail_compresses_overflow() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels = ["Очень длинная категория", "API", "DB", "Cache", "Network"];
        let slot = UiRect::new(0.0, 0.0, 100.0, 40.0);
        let chips = chip_strip(slot, &labels, true, &mut m, &mut fs);
        assert_eq!(
            chips.len(),
            labels.len(),
            "все чипы возвращены (SqueezeTail)"
        );
        // Ни один чип не выходит за правый край слота.
        for (i, (r, _)) in chips.iter().enumerate() {
            assert!(
                r.right() <= slot.right() + 0.01,
                "чип[{i}] не выходит за слот (SqueezeTail): right={} > slot.right={}",
                r.right(),
                slot.right()
            );
        }
        // Хвостовые чипы — нулевой ширины (сжались).
        let zero_w = chips.iter().any(|(r, _)| r.w <= 0.01);
        assert!(zero_w, "хвост сжат до 0 ширины (SqueezeTail)");
    }

    /// Пустой список items → пустой Vec (no-op).
    #[test]
    fn empty_items_returns_empty() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = TextMeasurer::new();
        let slot = UiRect::new(0.0, 0.0, 400.0, 40.0);
        assert!(chip_strip(slot, &[], false, &mut m, &mut fs).is_empty());
    }

    /// Порядок labels сохранён (result[i] ↔ items[i]).
    #[test]
    fn order_preserved() {
        let mut fs = cosmic_text::FontSystem::new();
        load_display_font(&mut fs);
        let mut m = TextMeasurer::new();
        let labels = ["Все", "API", "DB", "Cache"];
        let slot = UiRect::new(0.0, 0.0, 600.0, 40.0);
        let chips = chip_strip(slot, &labels, false, &mut m, &mut fs);
        for (i, (_, label)) in chips.iter().enumerate() {
            assert_eq!(*label, labels[i], "порядок labels сохранён");
        }
    }
}
