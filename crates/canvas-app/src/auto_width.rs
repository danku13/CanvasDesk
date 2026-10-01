//! FR-080 (auto-width node by content): расчёт ширины ноды по её тексту.
//!
//! Контракт: «10 средних слов в одну строку» — целевая ширина контента
//! ~420 лог. px (10 слов × ~6 chars × ~7 px/char при TYPE_BODY=14px).
//! Измерение — через [`canvas_ui::measure::TextMeasurer`] с тем же
//! шейпером cosmic-text, что у рендера (CR-015 паритет). Для пустого
//! текста — целевая ширина (нода создаётся «на вырост», чтобы при
//! вводе 10 слов не дёргалась).
//!
//! Стыковка с моделью: [`App::create_note_at`] создаёт новую заметку
//! с `width = auto_width_for_text(None)` вместо хардкод-дефолта 260.
//! [`App::finish_editing`] после коммита текста пересчитывает ширину.
//! Batch-применение к выделению — пункт меню `CanvasMenuItem::AutoWidth`
//! (FR-080 §3).
//!
//! Габариты согласованы с рендером:
//! - шапка: `CARD_HEADER_HEIGHT` (34px)
//! - тело: `BODY_PADDING` (10) + `BODY_TOP_GAP` (4) + N строк ×
//!   `TYPE_BODY_LINE` (20) + `BODY_PADDING` (10)
//! - целевая ширина контента = `TARGET_CONTENT_WIDTH` (см. ниже)
//! - суммарная ширина = контент + `BODY_PADDING × 2`

use canvas_core::tokens::TYPE_BODY;
use canvas_render::text::{measure_font_system, SANS_FAMILY};
use canvas_ui::measure::TextMeasurer;

/// Целевая ширина контента ноды: 10 средних слов в одну строку.
/// ~6 chars/слово × 10 слов × ~7 px/char при TYPE_BODY=14px → 420px.
/// Не кегль-зависимая эвристика, а продуктовая константа — владелец
/// говорит «10 слов», движок переводит в px. Идёт в `MIN`/`MAX` ниже.
pub const TARGET_CONTENT_WIDTH: f32 = 420.0;

/// Минимальная ширина ноды (с padding). Меньше — нода визуально
/// «сжимается» и текст переносится по одному слову на строку.
pub const MIN_NODE_WIDTH: f32 = 200.0;

/// Максимальная ширина ноды (с padding). Больше — текст в одной
/// строке перестаёт читаться (глаз застревает на 12+ словах без
/// переноса). Длинные слова (> MAX) рвёт рендер (ellipsis).
pub const MAX_NODE_WIDTH: f32 = 600.0;

/// Внутренние поля ноды (сумма левого + правого). Согласовано с
/// `BODY_PADDING` (10) × 2 = 20px в рендере.
pub const HORIZONTAL_PADDING: f32 = 20.0;

/// Рассчитать ширину ноды по её тексту.
///
/// Алгоритм:
/// 1. Пустой / None текст → `TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING`
///    (нода создаётся «на вырост» — при вводе до 10 слов не дёргается).
/// 2. Непустой текст: измеряем однострочную ширину через TextMeasurer.
///    Если текст влезает в TARGET (≤ 420) → ширина = измеренная +
///    padding (но не меньше MIN). Если не влезает (> 420) →
///    `TARGET + padding` (текст врапится в N строк; высота ноды не
///    трогается — рендер обрежет по высоте).
/// 3. Кламп к [MIN, MAX].
///
/// Замер дорогостоящий (cosmic-text шейпинг) — кэш TextMeasurer
/// делает повторные вызовы дешёвыми. Создание FontSystem на каждый
/// вызов — ~1мс (бенчмарки docs_ui); для batch-применения к N нодам
/// переиспользуем один FontSystem (см. [`auto_width_for_text_with`]).
pub fn auto_width_for_text(text: Option<&str>) -> f32 {
    let mut fs = measure_font_system();
    let mut m = TextMeasurer::new();
    auto_width_for_text_with(text, &mut m, &mut fs)
}

/// Вариант с переиспользуемыми TextMeasurer/FontSystem — для batch
/// (apply ко всем выделенным нодам). Избегает перевыделения FontSystem
/// на каждый узел (паттерн `measure_font_system()` возвращает
/// `MutexGuard<'static, FontSystem>` — переиспользуем).
pub fn auto_width_for_text_with(
    text: Option<&str>,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> f32 {
    let content = match text {
        Some(s) if !s.is_empty() => s,
        _ => {
            return (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING)
                .clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
        }
    };
    // Берём первую строку — она определяет минимальную ширину
    // (рендер переносит по словам, но не рвёт одно слово). Если
    // первая строка длиннее TARGET — она определит, в сколько строк
    // разложится текст; берём min(измерянная, TARGET) и clamp.
    let first_line = content.lines().next().unwrap_or("");
    let measured = if first_line.is_empty() {
        // Пустая первая строка (text = "\nfoo") — измеряем вторую
        content
            .lines()
            .nth(1)
            .filter(|s| !s.is_empty())
            .map(|s| m.width_of(fs, s, SANS_FAMILY, TYPE_BODY))
            .unwrap_or(0.0)
    } else {
        m.width_of(fs, first_line, SANS_FAMILY, TYPE_BODY)
    };
    let content_width = measured.clamp(0.0, TARGET_CONTENT_WIDTH);
    (content_width + HORIZONTAL_PADDING).clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str) -> f32 {
        let mut fs = measure_font_system();
        let mut m = TextMeasurer::new();
        auto_width_for_text_with(Some(text), &mut m, &mut fs)
    }

    #[test]
    fn empty_text_returns_target_width() {
        let w = auto_width_for_text(None);
        assert!((w - (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING)).abs() < 0.01);
        assert!(w >= MIN_NODE_WIDTH && w <= MAX_NODE_WIDTH);
    }

    #[test]
    fn empty_string_returns_target_width() {
        let w = auto_width_for_text(Some(""));
        assert!((w - (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING)).abs() < 0.01);
    }

    #[test]
    fn short_text_returns_measured_plus_padding() {
        // "abc" — короткое слово; измерянная ширина + padding.
        let w = measure("abc");
        assert!(
            w > HORIZONTAL_PADDING,
            "w={}, padding={}",
            w,
            HORIZONTAL_PADDING
        );
        assert!(w >= MIN_NODE_WIDTH, "w={} < min={}", w, MIN_NODE_WIDTH);
    }

    #[test]
    fn long_text_capped_at_target_plus_padding() {
        // Очень длинная однострочная "палка" — cap к TARGET + padding.
        // (NOT MAX — MAX is a safety net for super-long single words,
        // not the standard rule. Standard rule: target wraps to N lines.)
        let w = measure(&"a".repeat(500));
        // measured of "aaa...500" exceeds TARGET, so content_width=TARGET=420.
        // width = 420 + 20 = 440. Cap clamps to [200, 600] — 440 is in range.
        assert!(
            (w - (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING)).abs() < 1.0,
            "w={} expected ~{} (target+padding)",
            w,
            TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING
        );
    }

    #[test]
    fn medium_text_capped_at_target_when_exceeds() {
        // 30 слов по 5 букв = 150 chars — измерянная ширина > TARGET.
        let long = "abcde ".repeat(30);
        let w = measure(&long);
        // Должна быть в районе TARGET + padding (текст врапится).
        assert!(w <= MAX_NODE_WIDTH, "w={} > max={}", w, MAX_NODE_WIDTH);
        assert!(
            (w - (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING)).abs() < 5.0,
            "w={} expected ~{} (target+padding)",
            w,
            TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING
        );
    }

    #[test]
    fn multilinetext_uses_first_line() {
        // Первая строка длиннее второй — ширина по измерению первой.
        // 48 'a' chars — measured ширина меньше TARGET (420), потому что
        // 'a' узкий глиф. Ширина = measured + padding, clamped к min.
        let w = measure("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nb");
        // measured of 48 'a's < TARGET, так что content_width = measured.
        // width = measured + 20, clamped to [200, 600].
        assert!(
            w >= MIN_NODE_WIDTH && w <= MAX_NODE_WIDTH,
            "w={} out of bounds",
            w
        );
        // Width should NOT exceed TARGET + padding (48 'a' chars is
        // shorter than 60-char target line).
        assert!(
            w <= TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING + 1.0,
            "w={} > target+padding={}",
            w,
            TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING
        );
    }

    #[test]
    fn width_within_min_max_bounds() {
        for text in &["x", "hello", &"word ".repeat(20), &"a".repeat(1000)] {
            let w = measure(text);
            assert!(
                w >= MIN_NODE_WIDTH && w <= MAX_NODE_WIDTH,
                "text={} (len={}) → w={}, out of [{}, {}]",
                &text[..text.len().min(20)],
                text.len(),
                w,
                MIN_NODE_WIDTH,
                MAX_NODE_WIDTH
            );
        }
    }
}
