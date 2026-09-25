//! FR-068 W2: тесты `MockShaper` и проводки `Shaper` → `TextMeasurer`.
//!
//! `MockShaper` (фича `mock-shaper`) — детерминированный шейпер БЕЗ
//! шрифтов: ширина = `chars · 0.6 · size`, высота = `lines · size ·
//! SCREEN_LINE_FACTOR`. На нём проверяется сама граница
//! (`TextMeasurer::with_shaper`) и шрифто-независимая геометрия
//! measured-раскладки: точные ширины/rect'ы фиксируются формулой, а не
//! метриками шрифта — тесты одинаковы на всех платформах CI.
//!
//! # Плавающая точка
//!
//! Ожидания считаются ЗЕРКАЛОМ формулы мока (тот же порядок операций
//! [`mock_w`]) — значения бит-в-бит, `assert_eq!` на f32 легален.
//! Границы ellipsis/строк выбраны с запасом против FIT_EPS (0.05),
//! чтобы ulp-шум f32 (~1e-5) не влиял на исход.
//!
//! # Запуск
//!
//! ```text
//! cargo test -p canvas-ui --test shaper_mock --features mock-shaper
//! ```
//! Без фичи файл компилируется в пустой тестовый бинарник
//! (`#![cfg(feature = "mock-shaper")]` на весь файл — default-сборка
//! без мока).
#![cfg(feature = "mock-shaper")]

use canvas_ui::layout::{FlexLayoutEngine, MeasuredItem, NativeBackend, Row};
use canvas_ui::measure::{TextMeasurer, TextSpec, FIT_EPS, SCREEN_LINE_FACTOR};
use canvas_ui::shaper::{MockShaper, Shaper};
use canvas_ui::UiRect;
use cosmic_text::Weight;

const FAMILY: &str = "Noto Sans Display";

/// Зеркало формулы мока (FR-068 §W2): `n · 0.6 · size` — тот же порядок
/// операций, что в `MockShaper::shape`, поэтому значения совпадают
/// бит-в-бит (сравнение `assert_eq!` на f32 корректно).
fn mock_w(n: usize, size: f32) -> f32 {
    n as f32 * 0.6 * size
}

/// Детерминированный измеритель на моке (шрифты в замере не участвуют).
fn mock_measurer() -> TextMeasurer {
    TextMeasurer::with_shaper(Box::new(MockShaper::default()))
}

/// Точная формула ширины мока — напрямую через [`Shaper`] и через
/// [`TextMeasurer`] (проводка `measure` → `shaper.shape(spec.text)`)
/// дают одно значение; высота/строки однострочного замера.
#[test]
fn mock_width_exact_formula() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut shaper = MockShaper::default();
    let spec = TextSpec::single_line("abcd", FAMILY, 10.0);
    let direct = shaper.shape(&mut fs, "abcd", &spec);
    assert_eq!(direct.width, mock_w(4, 10.0), "4 символа · 0.6 · кегль 10");
    assert_eq!(direct.width, 24.0, "санити: ~24 ui px");
    assert_eq!(direct.lines, 1, "max_width = ∞ — одна строка");
    assert_eq!(direct.height, 1.0f32 * 10.0 * SCREEN_LINE_FACTOR);

    let mut m = mock_measurer();
    let via_measurer = m.width_of(&mut fs, "abcd", FAMILY, 10.0);
    assert_eq!(via_measurer, direct.width, "граница Shaper прозрачна");
    // Пустая строка — нулевая ширина, но одна линия раскладки.
    let empty = shaper.shape(&mut fs, "", &spec);
    assert_eq!(empty.width, 0.0);
    assert_eq!(empty.lines, 1);
}

/// Контракт FR-068 §W2: аргумент `text` — источник истины по строке
/// (не `spec.text`): шейпер обязан шейпить то, что передано аргументом.
#[test]
fn mock_shape_uses_text_argument_not_spec_text() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut shaper = MockShaper::default();
    let spec = TextSpec::single_line("xxxxxxxx", FAMILY, 10.0);
    let measured = shaper.shape(&mut fs, "ab", &spec);
    assert_eq!(
        measured.width,
        mock_w(2, 10.0),
        "источник истины — аргумент text, не spec.text"
    );
}

/// Монотонность формулы: больше символов — шире, больше кегль — шире
/// (линейно по size: кегль ×2 → ширина ×2 с точностью f32).
#[test]
fn mock_width_monotonic_by_chars_and_size() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let w2 = m.width_of(&mut fs, "ab", FAMILY, 10.0);
    let w5 = m.width_of(&mut fs, "abcde", FAMILY, 10.0);
    let w10 = m.width_of(&mut fs, "abcdefghij", FAMILY, 10.0);
    assert!(w2 > 0.0);
    assert!(w2 < w5 && w5 < w10, "длиннее — шире");
    let bigger = m.width_of(&mut fs, "ab", FAMILY, 20.0);
    assert!(
        (bigger - 2.0 * w2).abs() < 1e-3,
        "кегль ×2 → ширина ×2 (линейность формулы)"
    );
}

/// Число строк: конечная граница уже ширины → `ceil(ширина / граница)`;
/// ширина ровно в границе и бесконечная граница — одна строка.
#[test]
fn mock_lines_ceil_by_max_width() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut shaper = MockShaper::default();
    // Ширина "abcdefghij" = 10·0.6·10 ≈ 60 ui px; граница 25 → ceil(2.4) = 3.
    let spec = TextSpec {
        text: "abcdefghij",
        family: FAMILY,
        size: 10.0,
        max_width: 25.0,
        weight: Weight::MEDIUM,
    };
    let m3 = shaper.shape(&mut fs, "abcdefghij", &spec);
    assert_eq!(m3.lines, 3, "ceil(60 / 25) = 3");
    assert_eq!(m3.height, 3.0f32 * 10.0 * SCREEN_LINE_FACTOR);
    // Граница = точной ширине — помещается, одна строка (width ≤ max_width).
    let width = mock_w(10, 10.0);
    let fits = shaper.shape(
        &mut fs,
        "abcdefghij",
        &TextSpec {
            text: "abcdefghij",
            family: FAMILY,
            size: 10.0,
            max_width: width,
            weight: Weight::MEDIUM,
        },
    );
    assert_eq!(fits.lines, 1, "ширина ≤ границы — перенос не нужен");
    // Бесконечная граница — всегда одна строка (см. single_line-тест).
    // Высота = lines · size · SCREEN_LINE_FACTOR — проверено выше на 1 и 3 строках.
}

/// Проводка через [`TextMeasurer::with_shaper`]: кэш работает на моке
/// (повтор — hit, шейпер не дёргается; len() растёт только на новых
/// ключах). Значение из кэша идентично первому замеру.
#[test]
fn mock_via_measurer_is_cached() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    assert!(m.is_empty());
    let a = m.width_of(&mut fs, "кэш", FAMILY, 13.0);
    assert_eq!(a, mock_w(3, 13.0), "через границу приходит формула мока");
    assert_eq!(m.len(), 1, "результат в кэше");
    let b = m.width_of(&mut fs, "кэш", FAMILY, 13.0);
    assert_eq!(a, b, "повтор — из кэша, значение то же");
    assert_eq!(m.len(), 1, "нового ключа нет — шейпер не дёргался");
    let _ = m.width_of(&mut fs, "другой", FAMILY, 13.0);
    assert_eq!(m.len(), 2, "новая строка — новый ключ");
}

/// Политика Ellipsis на моке: помещающийся текст — целиком (в т.ч.
/// точная подгонка — FIT_EPS); не помещающийся — префикс + «…».
/// Ширины символов у мока ОДИНАКОВЫ (0.6·size), поэтому ожидаемый
/// префикс считается аналитически; границы выбраны с запасом против
/// FIT_EPS и ulp-шума (~1e-5 при кегле 10).
#[test]
fn mock_ellipsis_policy() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let text = "abcdefghij"; // 10 одинаковых (по моку) символов
    let size = 10.0;
    let full = m.width_of(&mut fs, text, FAMILY, size); // ≈ 60 ui px
                                                        // Помещается — целиком; точная подгонка — тоже (допуск FIT_EPS).
    assert_eq!(m.ellipsis(&mut fs, text, FAMILY, size, full + 1.0), text);
    assert_eq!(m.ellipsis(&mut fs, text, FAMILY, size, full), text);
    // Нулевая/отрицательная граница — пустая строка (политика measure.rs).
    assert_eq!(m.ellipsis(&mut fs, text, FAMILY, size, 0.0), "");

    // max = 0.55·full ≈ 5.5·c: влезает 4 символа + «…» (5·c), 5 символов
    // + «…» (6·c) — нет; запас от границы — ~0.5·c ≈ 3 ui px >> FIT_EPS.
    let cut = m.ellipsis(&mut fs, text, FAMILY, size, full * 0.55);
    assert_eq!(cut, "abcd…", "равные ширины символов → точный префикс");
    assert!(cut.ends_with('\u{2026}'));
    assert_eq!(cut.chars().count(), 5, "4 символа + многоточие");
    let cut_w = m.width_of(&mut fs, &cut, FAMILY, size); // 5·c
    assert!(
        cut_w <= full * 0.55 + FIT_EPS + f32::EPSILON,
        "усечённый текст ({cut_w}) укладывается в границу {}",
        full * 0.55
    );
    // Монотонность: шире граница — длиннее (по ширине) результат.
    let wider = m.ellipsis(&mut fs, text, FAMILY, size, full * 0.75); // ≈ 7.5·c → 6 символов + «…»
    let wider_w = m.width_of(&mut fs, &wider, FAMILY, size);
    assert!(wider_w > cut_w, "{wider} ({wider_w}) шире {cut} ({cut_w})");
    assert!(wider_w <= full * 0.75 + FIT_EPS + f32::EPSILON);
}

/// Мок игнорирует семейство и вес (задокументированное упрощение):
/// одна строка — одна ширина при любых атрибутах шрифта.
#[test]
fn mock_ignores_family_and_weight() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let sans = m.width_of_weighted(&mut fs, "тест", FAMILY, 13.0, Weight::MEDIUM);
    let mono = m.width_of_weighted(&mut fs, "тест", "Noto Sans Mono", 13.0, Weight::NORMAL);
    let bold = m.width_of_weighted(&mut fs, "тест", "Прочее семейство", 13.0, Weight::BOLD);
    assert_eq!(sans, mono, "семейство не влияет на формулу мока");
    assert_eq!(sans, bold, "вес не влияет на формулу мока");
    assert_eq!(sans, mock_w(4, 13.0));
}

/// [`MeasuredItem::Text::resolve`] с моком: Child получает ТОЧНО
/// формульную ширину/высоту; min_w поднимает, max_w клампит.
#[test]
fn mock_measured_item_text_resolve_exact_width() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let size = 12.0;
    // Без границ: w = 4·0.6·12, h = size·SCREEN_LINE_FACTOR.
    let child = MeasuredItem::Text {
        text: "abcd",
        max_w: None,
        min_w: 0.0,
        pad_x: 0.0,
    }
    .resolve(&mut m, &mut fs, FAMILY, size);
    assert_eq!(child.w, mock_w(4, size), "мок-ширина без искажений");
    assert_eq!(child.h, size * SCREEN_LINE_FACTOR, "высота однострочного");
    assert_eq!(child.grow, 0.0, "measured-ребёнок фиксированный");
    // min_w шире текста — ширина поднята до min_w.
    let child = MeasuredItem::Text {
        text: "abcd",
        max_w: None,
        min_w: 100.0,
        pad_x: 0.0,
    }
    .resolve(&mut m, &mut fs, FAMILY, size);
    assert_eq!(child.w, 100.0);
    // max_w уже текста — кламп (решение об ellipsis — за потребителем).
    let child = MeasuredItem::Text {
        text: "abcd",
        max_w: Some(20.0),
        min_w: 0.0,
        pad_x: 0.0,
    }
    .resolve(&mut m, &mut fs, FAMILY, size);
    assert_eq!(child.w, 20.0);
}

/// [`Row::lay_out_measured_with`] с моком на [`FlexLayoutEngine`] и
/// [`NativeBackend`]: детерминированная геометрия БЕЗ реального шрифта —
/// точные rect'ы из формулы мока; оба backend'а совпадают бит-в-бит
/// (W2-стаб Flex ≡ Native, §W2 «V-5 методы 1:1»), замер текста един
/// (кэш measurer'а — 1 ключ на обе раскладки).
#[test]
fn mock_row_layout_deterministic_on_both_backends() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let size = 10.0;
    let items = [
        MeasuredItem::Text {
            text: "abcd", // 4·0.6·10 ≈ 24 ui px, h = 13
            max_w: None,
            min_w: 0.0,
            pad_x: 0.0,
        },
        MeasuredItem::Fixed { w: 50.0, h: 20.0 },
        MeasuredItem::Spacer(10.0),
    ];
    let row = Row {
        gap: 8.0,
        ..Default::default()
    };
    let slot = UiRect::new(0.0, 0.0, 400.0, 100.0);
    let flex = row.lay_out_measured_with(
        &FlexLayoutEngine,
        slot,
        &items,
        &mut m,
        &mut fs,
        FAMILY,
        size,
    );
    let native =
        row.lay_out_measured_with(&NativeBackend, slot, &items, &mut m, &mut fs, FAMILY, size);
    assert_eq!(flex.len(), 3);
    for (i, (rf, rn)) in flex.iter().zip(native.iter()).enumerate() {
        assert_eq!(
            (rf.x, rf.y, rf.w, rf.h),
            (rn.x, rn.y, rn.w, rn.h),
            "Flex[{i}] ≢ Native[{i}] (W2-стаб: 1:1 семантика Native)"
        );
    }
    // Точная геометрия из формулы мока (Fit: Σ = 24+50+10+2·8 < 400 —
    // свободное место не распределено, старт слота, зазор = gap).
    let tw = mock_w(4, size);
    assert_eq!(
        flex[0],
        UiRect::new(0.0, 0.0, tw, size * SCREEN_LINE_FACTOR)
    );
    assert_eq!(flex[1], UiRect::new(tw + 8.0, 0.0, 50.0, 20.0));
    assert_eq!(flex[2], UiRect::new(tw + 58.0 + 8.0, 0.0, 10.0, 0.0));
    // Замер един: один уникальный ключ на обе раскладки (шейпинг ДО
    // адаптера — контракт FR-068 W1/W2).
    assert_eq!(m.len(), 1, "обе раскладки — один и тот же замер из кэша");
}

/// Кламп max_w внутри measured-раскладки: текст уже границы → ширина
/// ребёнка = max_w (ellipsis — решение потребителя), ряд съезжает
/// детерминированно.
#[test]
fn mock_row_layout_text_max_w_clamp() {
    let mut fs = cosmic_text::FontSystem::new();
    let mut m = mock_measurer();
    let size = 10.0;
    let items = [
        MeasuredItem::Text {
            text: "abcdefghij", // ≈ 60 ui px
            max_w: Some(20.0),
            min_w: 0.0,
            pad_x: 0.0,
        },
        MeasuredItem::Fixed { w: 30.0, h: 10.0 },
    ];
    let row = Row::default();
    let slot = UiRect::new(0.0, 0.0, 400.0, 100.0);
    let rects = row.lay_out_measured_with(
        &FlexLayoutEngine,
        slot,
        &items,
        &mut m,
        &mut fs,
        FAMILY,
        size,
    );
    assert_eq!(
        rects[0],
        UiRect::new(0.0, 0.0, 20.0, size * SCREEN_LINE_FACTOR)
    );
    assert_eq!(rects[1], UiRect::new(20.0, 0.0, 30.0, 10.0));
}

/// [`Shaper::font_system`] у мока: ленивый пул существует и выдаётся
/// повторно (standalone-контракт трейта; сам шейпинг пул игнорирует).
#[test]
fn mock_font_system_is_lazy_and_usable() {
    let mut shaper = MockShaper::default();
    let first = shaper.font_system() as *mut cosmic_text::FontSystem;
    let second = shaper.font_system() as *mut cosmic_text::FontSystem;
    assert_eq!(first, second, "ленивый пул создаётся один раз");
}
