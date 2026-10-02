//! FR-080 (auto-width node by content): расчёт ширины ноды по её тексту.
//!
//! Контракт: «10 средних слов в одну строку» — целевая ширина контента
//! ~420 лог. px (10 слов × ~6 chars × ~7 px/char при TYPE_BODY=14px).
//! Ширина ВСЕГДА равна `TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING`
//! (440px), кламп к [MIN, MAX]. Не адаптируется к фактической длине
//! текста — нода с 1 словом и нода с 100 словами обе будут 440px wide
//! (длинный текст врапится в N строк). Это产品的ое решение владельца:
//! «чтобы в одну строку влезало до 10 слов средней длинны» — значит
//! ширина под 10 слов, не под контент.
//!
//! Параметры `text`, `m`, `fs` сохранены в API для будущих расширений
//! (например, если для 1-словных заметок захотим shrink-to-fit), но
//! сейчас НЕ используются — measured_width игнорируется. Тесты
//! проверяют, что вызов возвращает константу независимо от текста.
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

// Импорты сохранены для будущей поддержки измерения (сейчас не нужны
// — функция возвращает константу). Не убираем, чтобы не ломать API
// callers, которые уже передают TextMeasurer/FontSystem.
#![allow(unused_imports)]
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
/// Всегда возвращает `TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING`
/// (440px), кламп к [MIN, MAX]. Текст не измеряется — константа
/// «10 слов в одну строку» одна для всех нод (см. владелец FR-080:
/// «чтобы в одну строку влезало до 10 слов средней длинны»).
///
/// Параметр `text` сохранён в API для будущих расширений (например,
/// shrink для очень коротких заметок), но сейчас не используется.
pub fn auto_width_for_text(_text: Option<&str>) -> f32 {
    (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING).clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
}

/// Вариант с переиспользуемыми TextMeasurer/FontSystem — для batch
/// (apply ко всем выделенным нодам). Параметры НЕ используются —
/// функция возвращает ту же константу, что и [`auto_width_for_text`].
/// Сохранены в сигнатуре для будущих расширений и обратной совместимости.
pub fn auto_width_for_text_with(
    _text: Option<&str>,
    _m: &mut TextMeasurer,
    _fs: &mut cosmic_text::FontSystem,
) -> f32 {
    auto_width_for_text(_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str) -> f32 {
        let mut fs = measure_font_system();
        let mut m = TextMeasurer::new();
        auto_width_for_text_with(Some(text), &mut m, &mut fs)
    }

    const EXPECTED: f32 =
        (TARGET_CONTENT_WIDTH + HORIZONTAL_PADDING).clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH);

    #[test]
    fn empty_text_returns_target_width() {
        let w = auto_width_for_text(None);
        assert!((w - EXPECTED).abs() < 0.01);
    }

    #[test]
    fn empty_string_returns_target_width() {
        let w = auto_width_for_text(Some(""));
        assert!((w - EXPECTED).abs() < 0.01);
    }

    #[test]
    fn short_text_returns_target_width() {
        // Раньше "abc" давал measured + padding (мало). Сейчас —
        // константа TARGET+padding (440px), как просил владелец:
        // «чтобы в одну строку влезало до 10 слов средней длинны» —
        // не под фактический контент, а под целевую ширину.
        let w = measure("abc");
        assert!((w - EXPECTED).abs() < 0.01, "w={} expected {}", w, EXPECTED);
    }

    #[test]
    fn long_text_returns_target_width() {
        // Длинный текст тоже 440px (врапится в N строк).
        let w = measure(&"a".repeat(500));
        assert!((w - EXPECTED).abs() < 0.01, "w={} expected {}", w, EXPECTED);
    }

    #[test]
    fn medium_text_returns_target_width() {
        // 30 слов — тоже 440px (врапится в 3 строки).
        let long = "abcde ".repeat(30);
        let w = measure(&long);
        assert!((w - EXPECTED).abs() < 0.01, "w={} expected {}", w, EXPECTED);
    }

    #[test]
    fn multilinetext_returns_target_width() {
        // Многострочный текст — тоже 440px. Раньше мерили первую
        // строку, для расчётных нод с короткими строками это давало
        // 150-200px (3-4 слова) — баг FR-080 (жалоба владельца).
        let w = measure("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nb");
        assert!((w - EXPECTED).abs() < 0.01, "w={} expected {}", w, EXPECTED);
    }

    #[test]
    fn width_within_min_max_bounds() {
        for text in &["x", "hello", &"word ".repeat(20), &"a".repeat(1000)] {
            let w = measure(text);
            assert!(
                (MIN_NODE_WIDTH..=MAX_NODE_WIDTH).contains(&w),
                "text={} (len={}) → w={}, out of [{}, {}]",
                &text[..text.len().min(20)],
                text.len(),
                w,
                MIN_NODE_WIDTH,
                MAX_NODE_WIDTH
            );
        }
    }

    #[test]
    fn all_text_variants_return_same_constant() {
        // Инвариант: текст не влияет на результат — константа.
        let empty = auto_width_for_text(None);
        let one_word = measure("x");
        let long = measure(&"word ".repeat(100));
        assert!((empty - one_word).abs() < 0.01);
        assert!((empty - long).abs() < 0.01);
        assert!((empty - EXPECTED).abs() < 0.01);
    }
}
