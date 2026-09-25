//! FR-068 W2: шейпинг текста за trait boundary — [`Shaper`].
//!
//! До W2 пайплайн шейпинга cosmic-text был вшит в `measure.rs` как
//! приватная `shape_measure`: реализация замера и кэш были одной
//! единицей. W2 выделяет операцию «шейпить строку → [`Measured`]» в
//! объектно-безопасный трейт: [`crate::measure::TextMeasurer`] владеет
//! `Box<dyn Shaper>`, cosmic-text — реализация по умолчанию
//! ([`CosmicShaper`], пайплайн перенесён бит-в-бит — метрики неизменны),
//! тестовые сценарии — детерминированный [`MockShaper`] (фича
//! `mock-shaper`, не для продакшна).
//!
//! Зачем граница (FR-068 §W2): подмена шейпера делает геометрию тестов
//! шрифто-независимой (мок без шрифтов), а сам шейпинг — заменяемым без
//! правок потребителей (сигнатуры `TextMeasurer` не меняются).

use crate::measure::{Measured, TextSpec, SCREEN_LINE_FACTOR};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Wrap};

/// Шейпинг текста → [`Measured`]. `fs` — шрифтовой пул ПОТРЕБИТЕЛЯ
/// (владелец — canvas-render::text, PRD-0009 §14/Q6): инвариант
/// «замер тем же пулом, что рендер» (CR-015). Аргумент `text` —
/// источник истины по строке (дублирует spec.text по контракту FR-068 §W2).
///
/// Объектно-безопасен сознательно: `TextMeasurer` хранит `Box<dyn Shaper>`
/// (выбор реализации — в рантайме, цена — vtable-вызов на замер, не на
/// глиф; шейпинг сам по себе на порядки дороже).
pub trait Shaper {
    /// Шейпинг текста → Measured. `fs` — шрифтовой пул ПОТРЕБИТЕЛЯ
    /// (владелец — canvas-render::text, PRD-0009 §14/Q6): инвариант
    /// «замер тем же пулом, что рендер» (CR-015). Аргумент `text` —
    /// источник истины по строке (дублирует spec.text по контракту
    /// FR-068 §W2).
    fn shape(&mut self, fs: &mut cosmic_text::FontSystem, text: &str, spec: &TextSpec) -> Measured;

    /// Собственный пул шейпера (lazy; standalone-сценарии/тесты).
    fn font_system(&mut self) -> &mut cosmic_text::FontSystem;
}

/// Реализация [`Shaper`] по умолчанию: реальный шейпинг cosmic-text.
///
/// Пайплайн 1:1 перенесён из `measure.rs` (`shape_measure`, FR-053 U3)
/// бит-в-бит: `Buffer` с `Metrics(size, size·SCREEN_LINE_FACTOR)` и
/// `Wrap::None`, `set_size`, `Attrs` family/weight, `Shaping::Advanced`,
/// `shape_until_scroll`, `layout_runs` → [`Measured`]; те же метрики,
/// что у screen-конвейера рендера (`canvas-render/src/text.rs`), CR-015.
///
/// # Почему `fs` — аргумент [`Shaper::shape`], а не поле шейпера
///
/// Пул [`FontSystem`] принадлежит потребителю (`canvas-render::text`,
/// PRD-0009 §14/Q6): замер обязан шейпить тем же пулом, что и рендер
/// (CR-015 — класс дефекта «замер чужим шрифтом»), иначе раскладка и
/// отрисовка расходятся в px. Передача пула аргументом даёт нулевой churn
/// потребителей: сигнатуры `TextMeasurer::measure/width_of/ellipsis` не
/// менялись (пул и раньше приходил извне), шейпер не заводит второй пул
/// на крейт.
///
/// # Почему собственный пул — lazy
///
/// `FontSystem::new()` дорогой (скан системных шрифтов), а
/// `TextMeasurer::new()` создаётся на кадр — дежурный пул в поле делал бы
/// каждое создание измерителя тяжёлым. Поле нужно только
/// standalone-сценариям/тестам (некому передать `fs`), поэтому оно
/// лениво создаётся при первом [`Shaper::font_system`]; горячий путь
/// ([`Shaper::shape`] с переданным пулом) его никогда не трогает.
pub struct CosmicShaper {
    /// Ленивый собственный пул (standalone-сценарии/тесты). `None`, пока
    /// `font_system()` не вызван; `shape` его не использует.
    lazy_fs: Option<FontSystem>,
}

impl Default for CosmicShaper {
    fn default() -> Self {
        Self::new()
    }
}

impl CosmicShaper {
    /// Шейпер без собственного пула (пул придёт аргументом `shape`).
    pub fn new() -> Self {
        Self { lazy_fs: None }
    }
}

impl Shaper for CosmicShaper {
    fn shape(&mut self, fs: &mut cosmic_text::FontSystem, text: &str, spec: &TextSpec) -> Measured {
        cosmic_shape(fs, text, spec)
    }

    fn font_system(&mut self) -> &mut cosmic_text::FontSystem {
        lazy_font_system(&mut self.lazy_fs)
    }
}

/// Реальный шейпинг cosmic-text — ПАЙПЛАЙН ПЕРЕНЕСЁН БИТ-В-БИТ из
/// `measure.rs` (функция `shape_measure` удалена в W2): тот же порядок
/// вызовов, те же константы — метрики измерений не меняются (контракт
/// FR-068 §W2).
///
/// Тот же пайплайн, что screen-тексты рендера (`text.rs`: Buffer +
/// Metrics(size, size·1.3) + Wrap::None + shape), семейство/вес — из
/// спеки (паритет с шейпингом потребителя); FR-069: вес ячеек по
/// семейству задают вызывающие хелперы (моно — NORMAL, sans — MEDIUM) —
/// см. width_of / MEASURE_WEIGHT row_grid.
fn cosmic_shape(fs: &mut cosmic_text::FontSystem, text: &str, spec: &TextSpec) -> Measured {
    let size = spec.size.max(0.0);
    let line_height = size * SCREEN_LINE_FACTOR;
    let mut buffer = Buffer::new(fs, Metrics::new(size, line_height));
    buffer.set_wrap(fs, Wrap::None);
    // Ограничение ширины: для Wrap::None строки не переносятся, ширина
    // задаёт только область (бесконечность — без ограничения).
    buffer.set_size(fs, Some(spec.max_width), Some(line_height));
    // Вес — из спеки (паритет с рендером потребителя; см. TextSpec::weight:
    // cosmic-text ищет лицо семейства только среди лиц точного веса).
    let attrs = Attrs::new()
        .family(Family::Name(spec.family))
        .weight(spec.weight);
    // `text` — источник истины по строке (контракт FR-068 §W2;
    // `TextMeasurer::measure` передаёт spec.text — данные те же).
    buffer.set_text(fs, text, attrs, Shaping::Advanced);
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

/// Ленивая инициализация собственного пула шейпера (общая для
/// [`CosmicShaper`] и [`MockShaper`]): `FontSystem::new()` — только при
/// первом обращении (см. доку [`CosmicShaper`] «почему lazy»).
fn lazy_font_system(slot: &mut Option<FontSystem>) -> &mut FontSystem {
    slot.get_or_insert_with(FontSystem::new)
}

/// Детерминированный [`Shaper`] для тестов (фича `mock-shaper`):
/// шрифтов НЕТ — ширина считается формулой, геометрия тестов не зависит
/// от шрифтов платформы/CI. **НЕ ДЛЯ ПРОДАКШНА**: метрики не совпадают с
/// рендером cosmic-text (инвариант CR-015 нарушается по построению) —
/// только через `TextMeasurer::with_shaper` в тестах/офлайн-инструментах.
///
/// Формула (зафиксирована лидом, FR-068 §W2):
/// - ширина = `chars(text) · 0.6 · size`;
/// - строки = 1, если `max_width` бесконечен или ширина ≤ `max_width`,
///   иначе `ceil(ширина / max_width)`;
/// - высота = `строки · size · SCREEN_LINE_FACTOR`.
///
/// Семейство и вес игнорируются (задокументированное упрощение): мок не
/// подбирает лица, ширина не зависит от атрибутов шрифта — тестам важна
/// детерминированность, не паритет с рендером. Вырожденные входы
/// (`max_width ≤ 0`, кроме бесконечности; отрицательный `size`) — вне
/// контракта мока (тесты используют положительные значения).
///
/// `font_system()` — такой же lazy, как у [`CosmicShaper`] (мок игнорирует
/// пул, но trait-контракт требует предоставить валидный `&mut FontSystem`
/// standalone-вызовам).
#[cfg(feature = "mock-shaper")]
pub struct MockShaper {
    /// Ленивый собственный пул (заглушка контракта [`Shaper::font_system`]).
    lazy_fs: Option<FontSystem>,
}

#[cfg(feature = "mock-shaper")]
impl Default for MockShaper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "mock-shaper")]
impl MockShaper {
    /// Мок без собственного пула (пул не используется шейпингом).
    pub fn new() -> Self {
        Self { lazy_fs: None }
    }
}

#[cfg(feature = "mock-shaper")]
impl Shaper for MockShaper {
    fn shape(
        &mut self,
        _fs: &mut cosmic_text::FontSystem,
        text: &str,
        spec: &TextSpec,
    ) -> Measured {
        // Формула мока (см. доку структуры): ширина от числа символов
        // (не байт/графем — тесты на ASCII/кириллице дают одно и то же).
        let width = text.chars().count() as f32 * 0.6 * spec.size;
        let lines = if spec.max_width.is_infinite() || width <= spec.max_width {
            1
        } else {
            (width / spec.max_width).ceil() as usize
        };
        Measured {
            width,
            height: lines as f32 * spec.size * SCREEN_LINE_FACTOR,
            lines,
        }
    }

    fn font_system(&mut self) -> &mut cosmic_text::FontSystem {
        lazy_font_system(&mut self.lazy_fs)
    }
}
