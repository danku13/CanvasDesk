//! FR-053 (U3 PRD-0009, F-6): TextMeasurer — измеренный текст для
//! раскладки (PRD-0009 §7.4 V-5, AC-4.1).
//!
//! Ширина/высота строки берутся из РЕАЛЬНОГО шейпинга cosmic-text — те же
//! метрики, что у screen-конвейера рендера (`canvas-render/src/text.rs`:
//! `Metrics::new(size, size·1.3)`, `Wrap::None`, семейство/вес —
//! `sans_attrs`), поэтому раскладка и отрисовка живут в одних единицах
//! (устраняет класс дефекта «эвристика 0.62·кегль занижала ширину» —
//! CR-015).
//!
//! `FontSystem` — аргументом (владелец инстанса — `canvas-render::text`,
//! PRD-0009 §14/Q6); `TextMeasurer` владеет только кэшем по ключу
//! (текст, семейство, кегль, макс-ширина) — Q6-a «измерение + кэш».
//! Кэш ограничен по ёмкости: переполнение → полная очистка (профиль
//! «просадка после всплеска уникальных строк» дешевле LRU и
//! детерминирован).

use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping, Weight, Wrap};
use std::collections::HashMap;

/// Фактор высоты строки screen-текстов рендера (зеркало
/// `canvas-render/src/text.rs`: `line_height = font_size * 1.3`).
pub const SCREEN_LINE_FACTOR: f32 = 1.3;

/// FR-067 (этап F, шаг 3 плана): вес шейпинга по семейству — паритет
/// атрибутам рендера (`canvas-render/src/text.rs`): sans
/// («Noto Sans Display») → [`Weight::MEDIUM`] — `sans_attrs`; моно
/// («Noto Sans Mono») и наклонное моно («CanvasDesk Mono Oblique») →
/// [`Weight::NORMAL`] (400) — `mono_attrs`/`mono_oblique_attrs`.
/// Прежний жёсткий MEDIUM компенсировался отсутствием Medium-лица у
/// Noto Sans Mono (CSS-подбор давал Regular 400) — паритет сделан
/// явным, чтобы смена шрифтовой базы не меняла раскладку. Имена
/// семейств — те же строковые данные, что в `Family::Name` рендера;
/// живой паритет залочен тестом `ui_measure_weight_matches_render_attrs`
/// (canvas-render).
pub fn family_weight(family: &str) -> Weight {
    match family {
        "Noto Sans Mono" | "CanvasDesk Mono Oblique" => Weight::NORMAL,
        _ => Weight::MEDIUM,
    }
}

/// Суб-пиксельный допуск «помещается»: раскладка гоняет ширины через
/// арифметику rect'ов (w+pad−pad) — отмена разрядов f32 даёт расхождение
/// порядка 1e-5; допуск отсекает ложные ellipsis на точной подгонке.
pub const FIT_EPS: f32 = 0.05;

/// Спецификация измерения текста.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSpec<'a> {
    pub text: &'a str,
    /// Семейство cosmic-text (например, «Noto Sans Display» — то же имя,
    /// что в `Family::Name` рендера).
    pub family: &'a str,
    /// Кегль в логических px.
    pub size: f32,
    /// Ограничение ширины для переносимых измерений; `f32::INFINITY` —
    /// без ограничения (однострочная ширина).
    pub max_width: f32,
}

/// Результат измерения.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// Ширина самой широкой строки (ui px).
    pub width: f32,
    /// Суммарная высота строк: `lines · size · SCREEN_LINE_FACTOR`.
    pub height: f32,
    /// Число строк после раскладки.
    pub lines: usize,
}

/// Ключ кэша: (текст, семейство, кегль, макс-ширина) — PRD-0009 AC-4.1.
/// Плавающие значения включаются битовым представлением (детерминизм
/// NaN-паттернов не важен: кегли/ширины конечны).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MeasureKey {
    text: Box<str>,
    family: Box<str>,
    size_bits: u32,
    max_bits: u32,
}

/// Измеритель текста с кэшем. Дешёвое создание на кадр (пул строк кадра
/// короткий; шейпинг dominates), кэш переиспользует ширины внутри кадра
/// (ellipsis-поиск префиксов).
#[derive(Debug, Clone)]
pub struct TextMeasurer {
    cache: HashMap<MeasureKey, Measured>,
    cap: usize,
}

impl Default for TextMeasurer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextMeasurer {
    /// Ёмкость кэша по умолчанию: строка кадра UI — десятки, всплеск
    /// ellipsis-префиксов — сотни; 4096 покрывает с запасом.
    pub const DEFAULT_CAP: usize = 4096;

    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            cap: Self::DEFAULT_CAP,
        }
    }

    /// Кэш с нестандартной ёмкостью (тесты переполнения).
    pub fn with_cap(cap: usize) -> Self {
        Self {
            cache: HashMap::new(),
            cap: cap.max(1),
        }
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Измерить текст реальным шейпингом cosmic-text (кэш: hit — без
    /// шейпинга).
    pub fn measure(&mut self, fs: &mut cosmic_text::FontSystem, spec: &TextSpec) -> Measured {
        let key = MeasureKey {
            text: spec.text.into(),
            family: spec.family.into(),
            size_bits: spec.size.to_bits(),
            max_bits: spec.max_width.to_bits(),
        };
        if let Some(hit) = self.cache.get(&key) {
            return *hit;
        }
        let measured = shape_measure(fs, spec);
        // Переполнение ёмкости → очистка (кэш — ускоритель, не источник
        // истины: очистка не меняет результаты измерения).
        if self.cache.len() >= self.cap {
            self.cache.clear();
        }
        self.cache.insert(key, measured);
        measured
    }

    /// Однострочная ширина текста (замена эвристик `0.62·кегль`).
    pub fn width_of(
        &mut self,
        fs: &mut cosmic_text::FontSystem,
        text: &str,
        family: &str,
        size: f32,
    ) -> f32 {
        self.measure(
            fs,
            &TextSpec {
                text,
                family,
                size,
                max_width: f32::INFINITY,
            },
        )
        .width
    }

    /// Политика Ellipsis (AC-4.2): самая длинная граница символов
    /// префикса, чья ширина с хвостом «…» укладывается в `max_width`;
    /// текст целиком, если помещается. Бинарный поиск по префиксам —
    /// ширины кэшируются.
    pub fn ellipsis(
        &mut self,
        fs: &mut cosmic_text::FontSystem,
        text: &str,
        family: &str,
        size: f32,
        max_width: f32,
    ) -> String {
        if max_width <= 0.0 {
            return String::new();
        }
        let full = self.width_of(fs, text, family, size);
        if full <= max_width + FIT_EPS {
            return text.to_owned();
        }
        let ell = '\u{2026}';
        if self.width_of(fs, &ell.to_string(), family, size) > max_width + FIT_EPS {
            return String::new();
        }
        let chars: Vec<char> = text.chars().collect();
        // Наибольший префикс длины k, где width(prefix) + width(…) <= max.
        // Инвариант: lo укладывается, hi — нет.
        let mut lo = 0usize; // точно помещается (пустой префикс)
        let mut hi = chars.len(); // точно не помещается (full > max)
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let prefix: String = chars[..mid].iter().collect();
            let w = self.width_of(fs, &prefix, family, size)
                + self.width_of(fs, &ell.to_string(), family, size);
            if w <= max_width + FIT_EPS {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut out: String = chars[..lo].iter().collect();
        out.push(ell);
        out
    }
}

/// Реальное измерение: тот же пайплайн, что screen-тексты рендера
/// (`text.rs`: Buffer + Metrics(size, size·1.3) + Wrap::None + shape),
/// семейство/вес — по [`family_weight`] (паритет `sans_attrs`/`mono_attrs`
/// рендера — FR-067).
fn shape_measure(fs: &mut cosmic_text::FontSystem, spec: &TextSpec) -> Measured {
    let size = spec.size.max(0.0);
    let line_height = size * SCREEN_LINE_FACTOR;
    let mut buffer = Buffer::new(fs, Metrics::new(size, line_height));
    buffer.set_wrap(fs, Wrap::None);
    // Ограничение ширины: для Wrap::None строки не переносятся, ширина
    // задаёт только область (бесконечность — без ограничения).
    buffer.set_size(fs, Some(spec.max_width), Some(line_height));
    let attrs = Attrs::new()
        .family(Family::Name(spec.family))
        .weight(family_weight(spec.family));
    buffer.set_text(fs, spec.text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(fs, false);
    let mut width = 0.0f32;
    let mut lines = 0usize;
    for run in buffer.layout_runs() {
        width = width.max(run.line_w);
        lines += 1;
    }
    Measured {
        width,
        height: lines as f32 * line_height,
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Детерминированный FontSystem: только вшитый рендером шрифт
    /// (тот же файл `NotoSansDisplay-Medium.ttf`, что FONT_DATA
    /// canvas-render) — метрики тестов = метрикам рендера и одинаковы
    /// на всех платформах CI.
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    const FAMILY: &str = "Noto Sans Display";

    #[test]
    fn empty_text_has_zero_width() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let r = m.measure(
            &mut fs,
            &TextSpec {
                text: "",
                family: FAMILY,
                size: 13.0,
                max_width: f32::INFINITY,
            },
        );
        assert_eq!(r.width, 0.0);
        assert_eq!(r.lines, 1, "пустая строка — одна линия раскладки");
        assert_eq!(m.len(), 1, "результат в кэше");
    }

    /// Ширина монотонна по длине текста и растёт с кеглем (санити
    /// реального шейпинга; точные px не фиксируются — метрики шрифта
    /// не часть контракта).
    #[test]
    fn width_monotonic_by_length_and_size() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let short = m.width_of(&mut fs, "База", FAMILY, 13.0);
        let long = m.width_of(&mut fs, "База + сценарий", FAMILY, 13.0);
        let bigger = m.width_of(&mut fs, "База", FAMILY, 20.0);
        assert!(short > 0.0);
        assert!(long > short, "длинный текст шире короткого");
        assert!(bigger > short, "больший кегль шире");
        // Многосимвольный текст с запасом уже эвристики 0.62·кегль —
        // проверяется отношением, не точным значением.
        let digits = m.width_of(&mut fs, "1111111111", FAMILY, 13.0);
        let wide = m.width_of(&mut fs, "ШШШШШШШШШШ", FAMILY, 13.0);
        assert!(wide > digits, "моноширинно-узкие цифры уже широких глифов");
    }

    #[test]
    fn cache_reuses_measurements() {
        let mut fs = font_system();
        let mut m = TextMeasurer::with_cap(2);
        let a = m.width_of(&mut fs, "Сценарий 1", FAMILY, 13.0);
        let again = m.width_of(&mut fs, "Сценарий 1", FAMILY, 13.0);
        assert_eq!(a, again);
        assert_eq!(m.len(), 1, "один ключ, повтор — из кэша");
        // Другой кегль — другой ключ.
        let _ = m.width_of(&mut fs, "Сценарий 1", FAMILY, 15.0);
        assert_eq!(m.len(), 2);
        // Третий уникальный ключ при ёмкости 2 → очистка.
        let _ = m.width_of(&mut fs, "Сценарий 2", FAMILY, 13.0);
        assert!(m.len() <= 2, "ёмкость соблюдена");
    }

    /// Политика Ellipsis: помещающийся текст возвращается целиком;
    /// не помещающийся — префикс + «…», чья ширина в границе; монотонен
    /// по границе.
    #[test]
    fn ellipsis_respects_max_width() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let text = "Очень длинное имя сценария с деталями эксперимента";
        let full = m.width_of(&mut fs, text, FAMILY, 13.0);
        // Помещается — целиком.
        assert_eq!(m.ellipsis(&mut fs, text, FAMILY, 13.0, full + 1.0), text);
        // Не помещается — усечён и реально помещается.
        let cut = m.ellipsis(&mut fs, text, FAMILY, 13.0, full * 0.5);
        assert!(cut.ends_with('\u{2026}'), "хвост — символ многоточия");
        assert!(cut.chars().count() < text.chars().count());
        let cut_w = m.width_of(&mut fs, &cut, FAMILY, 13.0);
        assert!(
            cut_w <= full * 0.5 + FIT_EPS + f32::EPSILON,
            "усечённый текст ({cut_w}) укладывается в границу {}",
            full * 0.5
        );
        // Монотонность: шире граница — длиннее (по ширине) результат.
        let wider = m.ellipsis(&mut fs, text, FAMILY, 13.0, full * 0.75);
        assert!(m.width_of(&mut fs, &wider, FAMILY, 13.0) >= cut_w);
        // Нулевая/отрицательная граница — пустая строка.
        assert_eq!(m.ellipsis(&mut fs, text, FAMILY, 13.0, 0.0), "");
    }

    /// Многострочность: при конечной ширине и Wrap::None строки не
    /// переносятся, но спецификация max_width не рвёт одиночную строку.
    #[test]
    fn nowrap_single_line_height() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let r = m.measure(
            &mut fs,
            &TextSpec {
                text: "строка",
                family: FAMILY,
                size: 13.0,
                max_width: 50.0,
            },
        );
        assert_eq!(r.lines, 1, "Wrap::None — без переноса");
        assert!((r.height - 13.0 * SCREEN_LINE_FACTOR).abs() < 1e-3);
    }
}
