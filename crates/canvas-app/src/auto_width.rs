//! FR-080 (auto-width node by content): расчёт ширины ноды по её тексту.
//!
//! Контракт: «10 средних слов в одну строку» — целевая ширина контента.
//! CR-015: прежний вывод «10 слов × ~6 chars × ~7 px/char → 420px»
//! (символьно-арифметическая эвристика) заменён ИЗМЕРЕНИЕМ эталонной
//! строки [`REFERENCE_TEN_WORDS`] реальным шейпингом при кегле тела
//! (TYPE_BODY/SANS_FAMILY) — метрики шрифта вместо «7 px/char».
//! Ширина ВСЕГДА равна `измеренный эталон + HORIZONTAL_PADDING`, кламп
//! к [MIN, MAX]. Не адаптируется к фактической длине текста — нода с
//! 1 словом и нода с 100 словами обе будут одной ширины (длинный текст
//! врапится в N строк). Это продуктовое решение владельца: «чтобы в
//! одну строку влезало до 10 слов средней длинны» — значит ширина под
//! 10 слов, не под контент; измеряется только ЭТАЛОН, не текст ноды.
//!
//! Параметры `text`, `m`, `fs` сохранены в API для будущих расширений
//! (например, если для 1-словных заметок захотим shrink-to-fit), но
//! сейчас НЕ используются — measured_width текста ноды игнорируется.
//! Тесты проверяют, что вызов возвращает одну и ту же (измеренную)
//! константу независимо от текста.
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
//! - целевая ширина контента = измеренный эталон «10 слов» (см. ниже)
//! - суммарная ширина = контент + `BODY_PADDING × 2`

use canvas_core::tokens::TYPE_BODY;
use canvas_render::text::{measure_font_system, SANS_FAMILY};
use canvas_ui::measure::TextMeasurer;

/// Эталонная строка «10 средних слов» — формализация продуктового решения
/// владельца FR-080 («чтобы в одну строку влезало до 10 слов средней
/// длинны»): 10 × «слово » — «слово» (5 букв + пробел) как усреднённое
/// слово RU-текста. Целевая ширина контента — измеренная ширина этой
/// строки при кегле тела ([`TYPE_BODY`]/SANS_FAMILY); это же семейство и
/// кегль, каким рендерится тело заметки — раскладка и рисование в одних
/// единицах (FR-053). Сам текст ноды НЕ измеряется (решение владельца).
pub const REFERENCE_TEN_WORDS: &str = "слово слово слово слово слово слово слово слово слово слово";

/// Целевая ширина контента ноды: ИЗМЕРЕННАЯ ширина эталона
/// [`REFERENCE_TEN_WORDS`] («10 средних слов») при кегле тела.
/// Замена прежней константы `TARGET_CONTENT_WIDTH = 420` (CR-015:
/// «10 слов × ~6 chars × ~7 px/char» — символьно-арифметическая
/// эвристика); число px теперь происходит из метрик шрифта.
pub fn target_content_width(m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
    m.width_of(fs, REFERENCE_TEN_WORDS, SANS_FAMILY, TYPE_BODY)
}

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
/// Всегда возвращает `измеренный эталон + HORIZONTAL_PADDING`, кламп
/// к [MIN, MAX]. Текст ноды не измеряется — целевая ширина одна для
/// всех нод (решение владельца FR-080: «чтобы в одну строку влезало
/// до 10 слов средней длинны»); измеряется только эталонная строка
/// [`REFERENCE_TEN_WORDS`] (см. [`target_content_width`]).
///
/// Параметр `text` сохранён в API для будущих расширений (например,
/// shrink для очень коротких заметок), но сейчас не используется.
pub fn auto_width_for_text(_text: Option<&str>) -> f32 {
    // Не draw-путь (создание/правка ноды по действию пользователя) —
    // внешний пул шейпинга приложения (FR-094), как у соседних хендлеров.
    let mut fs = measure_font_system();
    let mut m = TextMeasurer::new();
    auto_width_for_text_with(_text, &mut m, &mut fs)
}

/// Вариант с переиспользуемыми TextMeasurer/FontSystem — для batch
/// (apply ко всем выделенным нодам). Возвращает ту же ширину, что и
/// [`auto_width_for_text`]. Параметры `text` не используются (решение
/// владельца — константная ширина), `m`/`fs` — измерение эталона.
pub fn auto_width_for_text_with(
    _text: Option<&str>,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> f32 {
    (target_content_width(m, fs) + HORIZONTAL_PADDING).clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(text: &str) -> f32 {
        let mut fs = measure_font_system();
        let mut m = TextMeasurer::new();
        auto_width_for_text_with(Some(text), &mut m, &mut fs)
    }

    /// Ожидаемая ширина: измеренный эталон «10 средних слов» + пад,
    /// кламп [MIN, MAX] (CR-015: число px — из метрик шрифта, не из
    /// «7 px/char»).
    fn expected() -> f32 {
        let mut fs = measure_font_system();
        let mut m = TextMeasurer::new();
        (target_content_width(&mut m, &mut fs) + HORIZONTAL_PADDING)
            .clamp(MIN_NODE_WIDTH, MAX_NODE_WIDTH)
    }

    #[test]
    fn empty_text_returns_target_width() {
        let w = auto_width_for_text(None);
        assert!((w - expected()).abs() < 0.01);
    }

    #[test]
    fn empty_string_returns_target_width() {
        let w = auto_width_for_text(Some(""));
        assert!((w - expected()).abs() < 0.01);
    }

    #[test]
    fn short_text_returns_target_width() {
        // Раньше "abc" давал measured + padding (мало). Сейчас —
        // измеренный эталон + padding, как просил владелец:
        // «чтобы в одну строку влезало до 10 слов средней длинны» —
        // не под фактический контент, а под целевую ширину.
        let w = measure("abc");
        assert!(
            (w - expected()).abs() < 0.01,
            "w={} expected {}",
            w,
            expected()
        );
    }

    #[test]
    fn long_text_returns_target_width() {
        // Длинный текст — тоже ширина эталона (врапится в N строк).
        let w = measure(&"a".repeat(500));
        assert!(
            (w - expected()).abs() < 0.01,
            "w={} expected {}",
            w,
            expected()
        );
    }

    #[test]
    fn medium_text_returns_target_width() {
        // 30 слов — тоже ширина эталона (врапится в ~3 строки).
        let long = "abcde ".repeat(30);
        let w = measure(&long);
        assert!(
            (w - expected()).abs() < 0.01,
            "w={} expected {}",
            w,
            expected()
        );
    }

    #[test]
    fn multilinetext_returns_target_width() {
        // Многострочный текст — тоже ширина эталона. Раньше мерили первую
        // строку, для расчётных нод с короткими строками это давало
        // 150-200px (3-4 слова) — баг FR-080 (жалоба владельца).
        let w = measure("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nb");
        assert!(
            (w - expected()).abs() < 0.01,
            "w={} expected {}",
            w,
            expected()
        );
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
        // Инвариант: текст не влияет на результат — измеренная константа.
        let empty = auto_width_for_text(None);
        let one_word = measure("x");
        let long = measure(&"word ".repeat(100));
        assert!((empty - one_word).abs() < 0.01);
        assert!((empty - long).abs() < 0.01);
        assert!((empty - expected()).abs() < 0.01);
    }

    #[test]
    fn reference_target_from_font_metrics_within_design_bounds() {
        // CR-015: целевая ширина — из метрик шрифта (эталон «10 средних
        // слов» при TYPE_BODY), не из «7 px/char»; клампы не должны
        // «выезжать»: сырой эталон остаётся в дизайновом коридоре
        // [MIN − пад, MAX − пад] (кламп в [auto_width_for_text] не срабатывает).
        let mut fs = measure_font_system();
        let mut m = TextMeasurer::new();
        let raw = target_content_width(&mut m, &mut fs);
        assert!(raw > 0.0, "эталон измеряется (не 0)");
        assert!(
            (MIN_NODE_WIDTH - HORIZONTAL_PADDING..=MAX_NODE_WIDTH - HORIZONTAL_PADDING)
                .contains(&raw),
            "сырой эталон {raw} вне дизайнового коридора"
        );
        // Повторный замер стабилен (кэш/детерминизм шейпинга).
        let again = target_content_width(&mut m, &mut fs);
        assert!((raw - again).abs() < 0.01);
    }
}
