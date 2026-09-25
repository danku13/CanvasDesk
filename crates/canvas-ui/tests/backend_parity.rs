//! Интеграционная матрица паритета `NativeBackend` ↔ `TaffyBackend`
//! (FR-068 W1, ADR-0014; Контракт-3 «G4-линты × N backend'ов» и
//! §«Parity-оракул» FR-068: детерминированные сцены — побитовое ==
//! rect'ов кортежем `(x, y, w, h)`).
//!
//! # Что здесь есть, чего нет в smoke-тестах `taffy_backend.rs`
//!
//! Юнит-smoke (`src/layout/taffy_backend.rs`, 14 тестов) пинает базовые
//! случаи; этот файл — РАСШИРЕННАЯ матрица на интеграционном уровне:
//! декартовы произведения gap × align × наборы детей, edge-случаи grid
//! (1 строка / 1 колонка / отрицательный трек), measured-дети с РЕАЛЬНЫМ
//! `TextMeasurer` (шейпинг cosmic-text), пины задокументированных
//! расхождений и маски `features()`.
//!
//! # Детерминизм и «целые пиксели»
//!
//! Все размеры/зазоры/треки в побитовых секциях — ЦЕЛЫЕ ui px: taffy
//! округляет итоговые layout-координаты к целой px-сетке (CSS rounding
//! on freeze / tree rounding) — на целых входах округление тождественно,
//! на дробных даёт документированное расхождение ≤ 0.5 ui px (см. пины
//! `*_documented_divergence` ниже). Дробные входы в побитовые секции
//! сознательно не допускаются — для них есть пины.
//!
//! # Запуск
//!
//! ```text
//! cargo test -p canvas-ui --features taffy --test backend_parity
//! ```
//! Без фичи `taffy` файл компилируется в пустой тестовый бинарник
//! (`#![cfg(feature = "taffy")]` на весь файл) — default-сборка не тянет
//! taffy (zero-dep инвариант G7, §Контракт-2 FR-068).
#![cfg(feature = "taffy")]

use canvas_ui::layout::{
    grid_cells_with, Child, Column, CrossAlign, LayoutBackend, LayoutFeatures, MainAlign,
    MeasuredItem, NativeBackend, Row, RowPolicy, TaffyBackend,
};
use canvas_ui::measure::TextMeasurer;
use canvas_ui::{UiRect, UiVec2};

const NATIVE: NativeBackend = NativeBackend;
const TAFFY: TaffyBackend = TaffyBackend;

/// Слот с ненулевым началом координат (ловит перепутывание абсолютных и
/// относительных координат в адаптере).
fn slot() -> UiRect {
    UiRect::new(100.0, 50.0, 300.0, 200.0)
}

/// Побитовое сравнение `Vec<UiRect>` кортежем `(x, y, w, h)` —
/// parity-оракул FR-068 W1 (NaN/±0 паттернов нет: входы целые).
fn assert_same(a: &[UiRect], b: &[UiRect], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: разное число rect'ов");
    for (i, (ra, rb)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            (ra.x, ra.y, ra.w, ra.h),
            (rb.x, rb.y, rb.w, rb.h),
            "{what}[{i}]: native {ra:?} vs taffy {rb:?}"
        );
    }
}

// --- (a) Row Fit × gap × cross × наборы детей --------------------------------

/// (a) `RowPolicy::Fit`: декартово произведение {gap 0, 8} × {cross Start,
/// Center, End} × 4 набора детей (2–7 детей, разные высоты) — побитовый
/// паритет. Все размеры целые и высоты чётные: при `Center` y =
/// slot.y + (slot.h − h)/2 без дробей (см. модульную доку про px-сетку).
#[test]
fn row_fit_gap_cross_matrix_is_bitwise_equal() {
    // Наборы детей: 2/3/5/7 детей, разные высоты (все чётные).
    let child_sets: [&[Child]; 4] = [
        &[Child::fixed(40.0, 20.0), Child::fixed(60.0, 30.0)],
        &[
            Child::fixed(80.0, 30.0),
            Child::fixed(40.0, 10.0),
            Child::fixed(10.0, 24.0),
        ],
        &[
            Child::fixed(20.0, 12.0),
            Child::fixed(30.0, 28.0),
            Child::fixed(50.0, 8.0),
            Child::fixed(10.0, 16.0),
            Child::fixed(40.0, 20.0),
        ],
        &[
            Child::fixed(12.0, 10.0),
            Child::fixed(8.0, 20.0),
            Child::fixed(24.0, 14.0),
            Child::fixed(16.0, 6.0),
            Child::fixed(30.0, 22.0),
            Child::fixed(6.0, 18.0),
            Child::fixed(20.0, 12.0),
        ],
    ];
    for gap in [0.0, 8.0] {
        for cross in [CrossAlign::Start, CrossAlign::Center, CrossAlign::End] {
            for (set_i, items) in child_sets.iter().enumerate() {
                let row = Row {
                    gap,
                    cross,
                    ..Row::default()
                };
                let what = format!("row fit gap={gap} cross={cross:?} set#{set_i}");
                assert_same(
                    &NATIVE.lay_out_row(row, slot(), items),
                    &TAFFY.lay_out_row(row, slot(), items),
                    &what,
                );
            }
        }
    }
}

// --- (b) Row SpaceBetween / End (без переполнения) ----------------------------

/// (b) `MainAlign::SpaceBetween` и `End` без переполнения — побитовый
/// паритет. `SpaceBetween`: свободное место = слот − Σ детей − Σ зазоров,
/// делится на (n − 1) — наборы подобраны так, что деление нацело
/// (дробный эффективный зазор → дробные позиции → px-сетка taffy, пин).
/// Включая вырожденный n=1 (SpaceBetween вырождается в Start).
#[test]
fn row_aligns_space_between_and_end_without_overflow_are_bitwise_equal() {
    let cases: [(f32, &[Child]); 5] = [
        // n=1: SpaceBetween → Start (edge)
        (8.0, &[Child::fixed(40.0, 20.0)]),
        // n=2: extra = 300−100−8 = 192 → /1 нацело
        (8.0, &[Child::fixed(40.0, 20.0), Child::fixed(60.0, 30.0)]),
        // n=3: extra = 300−150−16 = 134 → /2 = 67 нацело
        (
            8.0,
            &[
                Child::fixed(40.0, 20.0),
                Child::fixed(60.0, 30.0),
                Child::fixed(50.0, 10.0),
            ],
        ),
        // n=5, gap 0: extra = 300−116 = 184 → /4 = 46 нацело
        (
            0.0,
            &[
                Child::fixed(20.0, 12.0),
                Child::fixed(30.0, 28.0),
                Child::fixed(10.0, 8.0),
                Child::fixed(40.0, 16.0),
                Child::fixed(16.0, 20.0),
            ],
        ),
        // End: любой набор без переполнения
        (
            8.0,
            &[
                Child::fixed(30.0, 20.0),
                Child::fixed(24.0, 14.0),
                Child::fixed(12.0, 6.0),
                Child::fixed(6.0, 18.0),
                Child::fixed(48.0, 10.0),
            ],
        ),
    ];
    for main in [MainAlign::SpaceBetween, MainAlign::End] {
        for (case_i, (gap, items)) in cases.iter().enumerate() {
            let row = Row {
                gap: *gap,
                main,
                cross: CrossAlign::Center,
                ..Row::default()
            };
            let what = format!("row {main:?} case#{case_i} gap={gap}");
            assert_same(
                &NATIVE.lay_out_row(row, slot(), items),
                &TAFFY.lay_out_row(row, slot(), items),
                &what,
            );
        }
    }
}

// --- (c) Row grow: целые доли --------------------------------------------------

/// (c) Flex-grow (FR-062 F-14) с ЦЕЛЫМИ долями свободного места — побитовый
/// паритет: 1:1, 1:1:2 и смешанный ряд с фиксированными детьми. Свободное
/// место делится нацело (дробные доли — документированное расхождение,
/// пин ниже).
#[test]
fn row_grow_integral_shares_is_bitwise_equal() {
    let cases: [(f32, &[Child]); 3] = [
        // 1:1: свободное = 300 − 40 − 10 = 250 → по 125
        (
            10.0,
            &[
                Child::flexible(20.0, 20.0, 1.0),
                Child::flexible(20.0, 20.0, 1.0),
            ],
        ),
        // 1:1:2: свободное = 300 − 60 − 20 = 220 → 55/55/110
        (
            10.0,
            &[
                Child::flexible(20.0, 20.0, 1.0),
                Child::flexible(20.0, 20.0, 1.0),
                Child::flexible(20.0, 20.0, 2.0),
            ],
        ),
        // смешанный: свободное = 300 − 130 − 30 = 140 → фиксированные не
        // растут, flex: 35/105
        (
            10.0,
            &[
                Child::fixed(50.0, 12.0),
                Child::flexible(20.0, 20.0, 1.0),
                Child::flexible(20.0, 20.0, 3.0),
                Child::fixed(40.0, 28.0),
            ],
        ),
    ];
    for (case_i, (gap, items)) in cases.iter().enumerate() {
        let row = Row {
            gap: *gap,
            cross: CrossAlign::Center,
            ..Row::default()
        };
        let what = format!("row grow integral case#{case_i}");
        assert_same(
            &NATIVE.lay_out_row(row, slot(), items),
            &TAFFY.lay_out_row(row, slot(), items),
            &what,
        );
    }
}

// --- (d) Row Wrap ---------------------------------------------------------------

/// (d) `RowPolicy::Wrap` (FR-062 F-15): 2–3 строки, поперечное
/// выравнивание ВНУТРИ строки (Center/End по высоте строки), ребёнок
/// шире слота — собственная строка, строки ниже слота выходят за нижний
/// край (не маскируются — контракт F-15) — побитовый паритет.
#[test]
fn row_wrap_lines_cross_align_overflow_below_is_bitwise_equal() {
    let cases: [(f32, CrossAlign, f32, &[Child]); 5] = [
        // 2 строки: [120+8+120=248 ≤ 300], [80] — cross Start
        (
            8.0,
            CrossAlign::Start,
            200.0,
            &[
                Child::fixed(120.0, 20.0),
                Child::fixed(120.0, 40.0),
                Child::fixed(80.0, 10.0),
            ],
        ),
        // 3 строки: 100+8+100=208, +8+100=316>300 → [2, 2, 1]; Center
        // внутри строки (высоты чётные — y без дробей)
        (
            8.0,
            CrossAlign::Center,
            200.0,
            &[
                Child::fixed(100.0, 20.0),
                Child::fixed(100.0, 40.0),
                Child::fixed(100.0, 10.0),
                Child::fixed(100.0, 30.0),
                Child::fixed(100.0, 20.0),
            ],
        ),
        // тот же набор, cross End внутри строки
        (
            8.0,
            CrossAlign::End,
            200.0,
            &[
                Child::fixed(100.0, 20.0),
                Child::fixed(100.0, 40.0),
                Child::fixed(100.0, 10.0),
                Child::fixed(100.0, 30.0),
                Child::fixed(100.0, 20.0),
            ],
        ),
        // ребёнок шире слота (350 > 300) — собственная строка; сосед —
        // следующая; переполнение вправо видно (как Fit)
        (
            8.0,
            CrossAlign::Start,
            200.0,
            &[Child::fixed(350.0, 20.0), Child::fixed(50.0, 10.0)],
        ),
        // строки НИЖЕ слота: слот высотой 40, две строки по 20 + gap 8 →
        // вторая строка y=28..48 — за нижним краем (виден, не маскируется)
        (
            8.0,
            CrossAlign::Start,
            40.0,
            &[
                Child::fixed(120.0, 20.0),
                Child::fixed(120.0, 20.0),
                Child::fixed(80.0, 20.0),
                Child::fixed(80.0, 20.0),
            ],
        ),
    ];
    for (case_i, (gap, cross, slot_h, items)) in cases.iter().enumerate() {
        let row = Row {
            gap: *gap,
            cross: *cross,
            policy: RowPolicy::Wrap,
            ..Row::default()
        };
        let s = UiRect::new(100.0, 50.0, 300.0, *slot_h);
        let what = format!("row wrap case#{case_i} cross={cross:?} slot_h={slot_h}");
        assert_same(
            &NATIVE.lay_out_row(row, s, items),
            &TAFFY.lay_out_row(row, s, items),
            &what,
        );
    }
}

// --- (e) Column -----------------------------------------------------------------

/// (e) `Column` (политика Fit, FR-062 F-14): фиксированные дети
/// (Start/Center/End поперёк), grow-целые доли, `MainAlign::End` и
/// `SpaceBetween` (деление свободного нацело) — побитовый паритет.
#[test]
fn column_fit_grow_end_space_between_is_bitwise_equal() {
    // Fit с поперечными выравниваниями (ширины чётные — Center без дробей)
    for cross in [CrossAlign::Start, CrossAlign::Center, CrossAlign::End] {
        let column = Column {
            gap: 6.0,
            cross,
            ..Column::default()
        };
        let items = [
            Child::fixed(40.0, 50.0),
            Child::fixed(80.0, 30.0),
            Child::fixed(20.0, 10.0),
        ];
        assert_same(
            &NATIVE.lay_out_column(column, slot(), &items),
            &TAFFY.lay_out_column(column, slot(), &items),
            &format!("column fit cross={cross:?}"),
        );
    }
    // grow 1:1: свободное = 200 − 60 − 8 = 132 → по 66
    let column = Column {
        gap: 8.0,
        ..Column::default()
    };
    let grow_items = [
        Child::flexible(40.0, 30.0, 1.0),
        Child::flexible(40.0, 30.0, 1.0),
    ];
    assert_same(
        &NATIVE.lay_out_column(column, slot(), &grow_items),
        &TAFFY.lay_out_column(column, slot(), &grow_items),
        "column grow 1:1",
    );
    // grow 1:1:2: свободное = 200 − 60 − 16 = 124 → 31/31/62
    let grow_items3 = [
        Child::flexible(40.0, 20.0, 1.0),
        Child::flexible(40.0, 20.0, 1.0),
        Child::flexible(40.0, 20.0, 2.0),
    ];
    assert_same(
        &NATIVE.lay_out_column(column, slot(), &grow_items3),
        &TAFFY.lay_out_column(column, slot(), &grow_items3),
        "column grow 1:1:2",
    );
    // End: прижаты к нижнему краю (без переполнения)
    let end_column = Column {
        gap: 8.0,
        main: MainAlign::End,
        ..Column::default()
    };
    let items = [Child::fixed(40.0, 40.0), Child::fixed(80.0, 60.0)];
    assert_same(
        &NATIVE.lay_out_column(end_column, slot(), &items),
        &TAFFY.lay_out_column(end_column, slot(), &items),
        "column End",
    );
    // SpaceBetween: extra = 200 − 150 − 16 = 34 → /2 = 17 нацело
    let sb_column = Column {
        gap: 8.0,
        main: MainAlign::SpaceBetween,
        ..Column::default()
    };
    let items = [
        Child::fixed(40.0, 40.0),
        Child::fixed(60.0, 60.0),
        Child::fixed(50.0, 50.0),
    ];
    assert_same(
        &NATIVE.lay_out_column(sb_column, slot(), &items),
        &TAFFY.lay_out_column(sb_column, slot(), &items),
        "column SpaceBetween",
    );
}

// --- (f) grid --------------------------------------------------------------------

/// (f) `grid_cells` (FR-062 F-16): равные/неравные треки, gap x≠y,
/// вырожденные 1 строка / 1 колонка, отрицательная ширина колонки
/// (кламп в 0 у обоих backend'ов) — побитовый паритет.
#[test]
fn grid_tracks_gaps_edge_cases_are_bitwise_equal() {
    let cases: [(&[f32], usize, f32, UiVec2); 6] = [
        // равные треки
        (&[60.0, 60.0, 60.0], 2, 24.0, UiVec2::new(8.0, 6.0)),
        // неравные треки (T2-триггер ADR-0013)
        (&[60.0, 120.0, 30.0], 2, 24.0, UiVec2::new(8.0, 6.0)),
        // gap x ≠ y
        (&[80.0, 80.0, 80.0], 2, 20.0, UiVec2::new(12.0, 4.0)),
        // 1 строка
        (&[100.0, 100.0], 1, 30.0, UiVec2::new(8.0, 8.0)),
        // 1 колонка
        (&[100.0], 3, 30.0, UiVec2::new(8.0, 8.0)),
        // отрицательная ширина колонки → кламп в 0 (ячейка вырожденная)
        (&[-50.0, 80.0], 2, 20.0, UiVec2::new(4.0, 4.0)),
    ];
    for (case_i, (cols, rows, row_h, gap)) in cases.iter().enumerate() {
        let what = format!("grid case#{case_i} cols={cols:?} rows={rows}");
        assert_same(
            &NATIVE.lay_out_grid(slot(), cols, *rows, *row_h, *gap),
            &TAFFY.lay_out_grid(slot(), cols, *rows, *row_h, *gap),
            &what,
        );
    }
}

// --- (g) MeasuredItem с реальным TextMeasurer ------------------------------------

/// Детерминированный FontSystem: только вшитый рендером шрифт (тот же
/// файл `NotoSansDisplay-Medium.ttf`, что FONT_DATA canvas-render) —
/// паттерн `row_guides.rs`/`measure.rs` тестов: метрики одинаковы на всех
/// платформах CI.
fn font_system() -> cosmic_text::FontSystem {
    let mut fs = cosmic_text::FontSystem::new();
    const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
    fs.db_mut().load_font_data(FONT.to_vec());
    fs
}

const FAMILY: &str = "Noto Sans Display";
/// Кегль measured-теста: высота строки = size·1.3 = 13 ui px — ЦЕЛОЕ
/// (f32-произведение 10·1.3 даёт ровно 13.0), иначе px-сетка taffy дала
/// бы документированное расхождение по высоте (см. пин ниже).
const MEASURE_SIZE: f32 = 10.0;

/// (g) `MeasuredItem` (FR-062 F-13) через `Row::lay_out_measured_with`:
/// Fixed / Text(max_w — кламп сверху) / Text(min_w — подъём снизу) /
/// Spacer, реальный шейпинг cosmic-text. Побитовый паритет ПО
/// ПОСТРОЕНИЮ: `MeasuredItem::resolve` (единая точка замера) выполняется
/// ДО адаптера у обоих backend'ов, а клампы max_w/min_w дают ЦЕЛЫЕ
/// ширины — px-сетка taffy тождественна (см. модульную доку).
#[test]
fn measured_items_with_real_text_measurer_is_bitwise_equal() {
    let mut m = TextMeasurer::new();
    let mut fs = font_system();
    // Длинный текст гарантированно шире max_w=120 (кламп сверху → 120);
    // «ок» гарантированно уже min_w=64 (подъём снизу → 64).
    let long_text = "Очень длинная строка сценария что-если с деталями";
    let items = [
        MeasuredItem::Fixed { w: 80.0, h: 20.0 },
        MeasuredItem::Text {
            text: long_text,
            max_w: Some(120.0),
            min_w: 0.0,
        },
        MeasuredItem::Text {
            text: "ок",
            max_w: None,
            min_w: 64.0,
        },
        MeasuredItem::Spacer(24.0),
    ];
    // Пре-условие побитовой секции: resolve дал целые ширины/высоты
    // (клампы целые; высота = 1 линия · 10 · 1.3 = 13). Если тонет —
    // изменились данные теста, а не паритет: исправь пре-условие осознанно.
    for (i, item) in items.iter().enumerate() {
        let c = item.resolve(&mut m, &mut fs, FAMILY, MEASURE_SIZE);
        assert_eq!(
            c.w.trunc(),
            c.w,
            "пре-условие: resolved ширина ребёнка #{i} должна быть целой (w={})",
            c.w
        );
        assert_eq!(
            c.h.trunc(),
            c.h,
            "пре-условие: resolved высота ребёнка #{i} должна быть целой (h={})",
            c.h
        );
    }
    assert_eq!(
        items[1].resolve(&mut m, &mut fs, FAMILY, MEASURE_SIZE).w,
        120.0,
        "кламп max_w сработал (текст шире лимита)"
    );
    assert_eq!(
        items[2].resolve(&mut m, &mut fs, FAMILY, MEASURE_SIZE).w,
        64.0,
        "подъём min_w сработал (текст уже пола)"
    );

    for policy in [RowPolicy::Fit, RowPolicy::Wrap] {
        let row = Row {
            gap: 8.0,
            cross: CrossAlign::Start,
            policy,
            ..Row::default()
        };
        // Wrap: слот уже суммы → 2–3 строки переноса (те же resolved
        // размеры — паритет переносов тоже побитовый).
        let s = UiRect::new(
            100.0,
            50.0,
            if policy == RowPolicy::Wrap {
                160.0
            } else {
                300.0
            },
            200.0,
        );
        let native = NATIVE.lay_out_measured(row, s, &items, &mut m, &mut fs, FAMILY, MEASURE_SIZE);
        let taffy = TAFFY.lay_out_measured(row, s, &items, &mut m, &mut fs, FAMILY, MEASURE_SIZE);
        assert_same(&native, &taffy, &format!("measured {policy:?}"));
    }
}

// --- (h) Пины задокументированных расхождений ------------------------------------

/// Пин расхождения C3 (§Контракт-4 FR-068): `SqueezeTail` ≠ CSS
/// flex_shrink. Native сжимает ТОЛЬКО хвост (до нуля при переполнении,
/// голова не тронута); taffy распределяет сжатие по ВСЕМ детям
/// пропорционально basis. Это ФИКСАЦИЯ различия, не fail: поведение
/// должно быть видимым, а не случайным.
#[test]
fn pin_squeeze_tail_divergence_c3() {
    let row = Row {
        gap: 4.0,
        policy: RowPolicy::SqueezeTail,
        ..Row::default()
    };
    let items = [
        Child::fixed(150.0, 20.0),
        Child::fixed(150.0, 20.0),
        Child::fixed(150.0, 20.0),
    ];
    let native = NATIVE.lay_out_row(row, slot(), &items);
    let taffy = TAFFY.lay_out_row(row, slot(), &items);
    // Native: хвост сжат ДО 0, голова — desired (150)
    assert_eq!(native[2].w, 0.0, "native SqueezeTail: хвост == 0.0");
    assert_eq!(native[0].w, 150.0, "native SqueezeTail: голова не сжата");
    // taffy (flex_shrink по всем): голова сжата, хвост НЕ обнулён
    assert!(
        taffy[0].w < 150.0,
        "taffy flex_shrink сжимает голову (C3): got {}",
        taffy[0].w
    );
    assert!(
        taffy[2].w > 0.0,
        "taffy flex_shrink: хвост имеет ненулевую долю сжатия (got {})",
        taffy[2].w
    );
    // Общий инвариант обоих: сумма + зазоры не вылезает за слот
    let total_t = taffy.iter().map(|r| r.w).sum::<f32>() + 2.0 * 4.0;
    assert!(
        total_t <= slot().w + 0.01,
        "taffy SqueezeTail-путь: сумма детей ≤ слота (got {total_t})"
    );
}

/// Пин расхождения rounding (CSS spec §9.7 rounding on freeze / px-сетка
/// taffy): дробные доли grow — taffy округляет main-размер к целому ui px,
/// Native оставляет дробь. Расхождение ≤ 0.5 ui px на координату —
/// фиксация, не fail.
#[test]
fn pin_fractional_grow_rounding_divergence() {
    let row = Row {
        gap: 10.0,
        ..Row::default()
    };
    let items = [
        Child::fixed(50.0, 20.0),
        Child::flexible(20.0, 20.0, 1.0),
        Child::flexible(20.0, 20.0, 3.0),
    ];
    let native = NATIVE.lay_out_row(row, slot(), &items);
    let taffy = TAFFY.lay_out_row(row, slot(), &items);
    // Native: 20 + 190·0.25 = 67.5 (без округления)
    assert_eq!(native[1].w, 67.5, "native: дробная доля без округления");
    // taffy: округление к целому px-сетке
    assert_eq!(
        taffy[1].w, 68.0,
        "taffy: rounding on freeze — доля к целому px"
    );
    for (i, (n, t)) in native.iter().zip(taffy.iter()).enumerate() {
        let dw = (n.w - t.w).abs();
        let dx = (n.x - t.x).abs();
        assert!(
            dw <= 0.5 + 1e-4 && dx <= 0.5 + 1e-4,
            "расхождение дробного grow [{i}] выше 0.5 ui px: dx={dx} dw={dw}"
        );
    }
}

/// Пин расхождения `MainAlign::End` + переполнение: Native клампит левый
/// край (`x ≥ slot.x`, переполнение уходит вправо), CSS flex-end при
/// переполнении выталкивает детей ЗА ЛЕВЫЙ край (unsafe alignment,
/// x < slot.x). Фиксация, не fail (parity — только без переполнения).
#[test]
fn pin_end_with_overflow_divergence() {
    let row = Row {
        main: MainAlign::End,
        ..Row::default()
    };
    let items = [Child::fixed(200.0, 20.0), Child::fixed(200.0, 20.0)];
    let s = slot();
    let native = NATIVE.lay_out_row(row, s, &items);
    let taffy = TAFFY.lay_out_row(row, s, &items);
    // Native: левый край клампнут к началу слота
    assert!(
        native[0].x >= s.x,
        "native End+overflow: x ≥ slot.x (got {})",
        native[0].x
    );
    // taffy: CSS flex-end уводит детей за левый край слота
    assert!(
        taffy[0].x < s.x,
        "taffy End+overflow: x < slot.x — выход за левый край (got {})",
        taffy[0].x
    );
}

// --- (i) Маски features() ---------------------------------------------------------

/// (i) `features()`: биты backend'ов соответствуют документированным маскам
/// (§Контракт-2 FR-068, доки `LayoutFeatures`): Native = GROW | BASIS |
/// WRAP | GRID_2D | AUTO_SIZE (без SHRINK/CLIP/PERCENT/ASPECT/STICKY);
/// Taffy = все 10 битов (STICKY — эмуляция в `lay_out_scene`).
#[test]
fn features_masks_match_documented_contract() {
    let native = NATIVE.features();
    let expected_native = LayoutFeatures::FLEX_GROW
        .union(LayoutFeatures::FLEX_BASIS)
        .union(LayoutFeatures::FLEX_WRAP)
        .union(LayoutFeatures::GRID_2D)
        .union(LayoutFeatures::AUTO_SIZE);
    assert_eq!(
        native.bits(),
        expected_native.bits(),
        "NativeBackend: маска должна быть ровно GROW|BASIS|WRAP|GRID_2D|AUTO_SIZE"
    );
    assert!(
        !native.contains(LayoutFeatures::FLEX_SHRINK),
        "Native: CSS flex_shrink нет (SqueezeTail — именованная деградация, C3)"
    );
    assert!(
        !native.contains(LayoutFeatures::OVERFLOW_CLIP),
        "Native: overflow-клипа нет"
    );
    assert!(
        !native.contains(LayoutFeatures::PERCENT),
        "Native: percent нет"
    );
    assert!(
        !native.contains(LayoutFeatures::ASPECT_RATIO),
        "Native: aspect-ratio нет"
    );
    assert!(
        !native.contains(LayoutFeatures::STICKY),
        "Native: sticky нет"
    );

    let taffy = TAFFY.features();
    let expected_taffy = expected_native
        .union(LayoutFeatures::FLEX_SHRINK)
        .union(LayoutFeatures::OVERFLOW_CLIP)
        .union(LayoutFeatures::PERCENT)
        .union(LayoutFeatures::ASPECT_RATIO)
        .union(LayoutFeatures::STICKY);
    assert_eq!(
        taffy.bits(),
        expected_taffy.bits(),
        "TaffyBackend: маска должна быть ровно все 10 битов"
    );
    assert!(
        taffy.contains(expected_native),
        "TaffyBackend: надмножество возможностей Native"
    );
    // FR-068 W2: `default_backend()` под фичей taffy — TaffyBackend
    // (файловая таблица W2); без фичи — FlexLayoutEngine (собственный
    // движок, 0 deps). `pilot_backend()` — TaffyBackend (W1-поведение).
    assert_eq!(
        canvas_ui::layout::default_backend().features().bits(),
        taffy.bits(),
        "default_backend() под фичей taffy — TaffyBackend (FR-068 W2)"
    );
    assert_eq!(
        canvas_ui::layout::pilot_backend().features().bits(),
        taffy.bits(),
        "pilot_backend() под фичей taffy — TaffyBackend"
    );
}

// --- (g2) Пин дробного measured-текста --------------------------------------------

/// Пин (расширение rounding-расхождения на measured-детей): НЕЗАЖАТЫЙ
/// (`max_w = None`, `min_w = 0`) текст имеет дробную измеренную ширину —
/// Native передаёт дробь как есть, taffy округляет к целой px-сетке
/// (тот же rounding on freeze). Расхождение ≤ 0.5 ui px на координату —
/// фиксация, не fail. Побитовый паритет measured-детей — на зажатых
/// (`max_w`/`min_w`) размерах, см. `measured_items_with_real_text_measurer`.
#[test]
fn pin_measured_fractional_text_rounding_divergence() {
    let mut m = TextMeasurer::new();
    let mut fs = font_system();
    let items = [MeasuredItem::Text {
        text: "Дробная ширина без зажимов",
        max_w: None,
        min_w: 0.0,
    }];
    let resolved = items[0].resolve(&mut m, &mut fs, FAMILY, 13.0);
    // Пре-условие пина: ширина дробная (иначе пин вырожден — обнови
    // текст/кегль осознанно).
    assert_ne!(
        resolved.w.trunc(),
        resolved.w,
        "пре-условие: измеренная ширина должна быть дробной (got {})",
        resolved.w
    );
    let row = Row::default();
    let native = NATIVE.lay_out_measured(row, slot(), &items, &mut m, &mut fs, FAMILY, 13.0);
    let taffy = TAFFY.lay_out_measured(row, slot(), &items, &mut m, &mut fs, FAMILY, 13.0);
    // taffy: размер на целой px-сетке
    assert_eq!(
        taffy[0].w.trunc(),
        taffy[0].w,
        "taffy: measured-ширина на px-сетке (got {})",
        taffy[0].w
    );
    assert_eq!(
        taffy[0].h.trunc(),
        taffy[0].h,
        "taffy: measured-высота на px-сетке (got {})",
        taffy[0].h
    );
    // Граница расхождения: ≤ 0.5 ui px на координату
    let dx = (native[0].x - taffy[0].x).abs();
    let dw = (native[0].w - taffy[0].w).abs();
    let dh = (native[0].h - taffy[0].h).abs();
    assert!(
        dx <= 0.5 + 1e-4 && dw <= 0.5 + 1e-4 && dh <= 0.5 + 1e-4,
        "расхождение дробного measured-текста выше 0.5 ui px: dx={dx} dw={dw} dh={dh}"
    );
}

/// Санити-страж утилиты `grid_cells_with`: делегирование в выбранный
/// backend совпадает с прямым вызовом `lay_out_grid` (проводка
/// `grid_cells_with` ↔ `LayoutBackend::lay_out_grid` не перепутана).
#[test]
fn grid_cells_with_dispatches_to_selected_backend() {
    let cols = [60.0, 120.0, 30.0];
    let gap = UiVec2::new(8.0, 6.0);
    let via_helper = grid_cells_with(&TAFFY, slot(), &cols, 2, 24.0, gap);
    let direct = TAFFY.lay_out_grid(slot(), &cols, 2, 24.0, gap);
    assert_same(&via_helper, &direct, "grid_cells_with → TaffyBackend");
}
