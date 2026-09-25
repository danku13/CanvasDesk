//! Perf-гейт reflow 1000 узлов на `FlexLayoutEngine` (FR-068 §W2; контракт
//! §8 FR-068, docs/change-requests/fr-068-ui-refactoring-long-term.md;
//! ADR-0015 — собственный layout-движок 0 deps).
//!
//! # Что измеряется
//!
//! Один полный immediate-mode reflow синтетического UI-графа от корня
//! ПОЛНОСТЬЮ через ЯВНЫЙ [`FlexLayoutEngine`] (каждый вызов — через
//! backend-метод, без [`super::default_backend`]-диспетчеризации):
//! `stack` → `constrain` → `pad` → `Column::lay_out_with` (10
//! контейнерных строк) → 9 × `Row::lay_out_with` + `grid_cells_with`.
//! Метрика — МЕДИАННАЯ длительность одного reflow в μs по `ITERATIONS`
//! замерам (`Instant`), после `WARMUP` прогревочных прогонов (не
//! замеряются). canvas-ui — нативный тест: `std::time` доступен.
//!
//! # Определение синтетического графа (зафиксировано, детерминировано)
//!
//! ТОТ ЖЕ граф, что у `perf_baseline.rs` (FR-068 W0), — сравнение
//! backend'ов на идентичном дереве (1000 узлов): viewport 1280×800
//! (canonical G4-окно), stage 1200×760 (`constrain` + центрирование
//! `stack`), внутренний слот = stage − pad 16 px. «Узел» = ЛИСТОВОЙ
//! rect, вычисленный за один reflow; итого РОВНО 1000 узлов (`RowCase`
//! × 10 строк × 100 листьев):
//!
//! - строки 0–4: `RowPolicy::Fit`, 100 Child (каждый 4-й — `flexible`
//!   с grow = 1; малые ширины 2–6 px — слот не переполняется,
//!   распределение свободного места exercised); main/cross варьируются
//!   (Start/SpaceBetween/End × Start/Center/End от индекса строки);
//! - строки 5–6: `RowPolicy::Wrap`, 100 Child шириной 40–130 px —
//!   жадная упаковка ≈ 9 строк переноса (строки ниже слота НЕ
//!   маскируются — контракт F-15, для замера осознанно);
//! - строки 7–8: `RowPolicy::SqueezeTail`, 100 Child шириной 60–180 px —
//!   сумма ≫ слота, хвост сжимается до нулевой ширины. В графе
//!   ОСОЗНАННО есть (в отличие от `perf_taffy.rs`): `FlexLayoutEngine`
//!   реализует `SqueezeTail` ДОСЛОВНО (§Контракт-4 FR-068 — «Flex
//!   решает»), расхождение C3 с taffy `flex_shrink` на этот гейт не
//!   влияет;
//! - строка 9: `grid_cells` — 10 явных колонок × 10 строк = 100 ячеек
//!   (F-16; вертикально выходит за высоту строкового слота —
//!   grid-ячейки не маскируются, для замера осознанно).
//!
//! 10 контейнерных rect'ов строк (выход `Column::lay_out_with`), stage
//! и внутренний слот в 1000 НЕ входят (это контейнеры, не узлы). Все
//! размеры — формула от глобального индекса (без RNG; seeded-RNG в std
//! нет, формулы дают тот же детерминизм) — см. константы в
//! `fit_row`/`wrap_row`/`squeeze_row`/`grid_row`.
//!
//! # W2-стаб (ВАЖНО — интерпретация числа)
//!
//! На момент волны W2 `FlexLayoutEngine` — СТАБ (агент 2-a заменяет
//! собственной реализацией flexbox + round-layout в этой же волне):
//! V-5-методы делегируют семантике `NativeBackend` 1:1 (байт-в-байт).
//! Пока стаб активен, число ниже может замерять Native-делегацию, а не
//! финальный собственный flexbox. ФИНАЛЬНЫЙ baseline перегенерируется
//! лидом на интеграции волны W2
//! (`CANVAS_UI_UPDATE_PERF_FLEX=1 cargo test -p canvas-ui --test perf_flex -- --ignored`).
//! Методология (граф/warmup/итерации/медиана/бюджеты) зафиксирована —
//! числа сравнимы между стабом и финалом на одной машине.
//!
//! # Машинно-зависимость (осознанная)
//!
//! Абсолютное значение зависит от машины и профиля (`cargo test` —
//! debug). Baseline-файл `tests/perf_flex_baseline.txt` — референс
//! dev-машины и коммитится. Сравнение при запуске: регрессия > 20% от
//! записанного — fail (контракт §8 FR-068), улучшение > 20% —
//! предупреждение в stdout (не fail; baseline можно обновить осознанно).
//!
//! # Бюджеты
//!
//! - Абсолютный (любая машина): медиана < 1000 μs = 1 мс на 1000 узлов
//!   (§W2 гейт FR-068: «reflow 1000 узлов < 1 мс на FlexLayoutEngine»;
//!   тот же §8-бюджет, что у `perf_baseline`/`perf_taffy`). Движок —
//!   чистая f32-арифметика без внешних крейтов (0 deps, G7): медленных
//!   dep-веток, как у taffy в dev-профиле, нет — бюджет ассертится в
//!   обоих профилях.
//! - Относительный (baseline dev-машины): дрейф > 20% — сигнал.
//!
//! # Запуск
//!
//! ```text
//! cargo test -p canvas-ui --test perf_flex -- --ignored
//! cargo test -p canvas-ui -- --ignored perf_flex          # как в FR-068
//! CANVAS_UI_UPDATE_PERF_FLEX=1 cargo test -p canvas-ui --test perf_flex -- --ignored
//! ```

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use canvas_ui::layout::{
    constrain, grid_cells_with, pad, stack, Child, Column, CrossAlign, FlexLayoutEngine, HAlign,
    MainAlign, Row, RowPolicy, VAlign,
};
use canvas_ui::{EdgeInsets, UiRect, UiVec2};

/// Backend замера: ЯВНЫЙ `FlexLayoutEngine` (FR-068 §W2) — гейт
/// собственного движка, без диспетчеризации `default_backend`.
const BACKEND: FlexLayoutEngine = FlexLayoutEngine;

/// Каноническое G4-окно (FR-068 W0): 1280×800.
const VIEWPORT: UiRect = UiRect::new(0.0, 0.0, 1280.0, 800.0);
/// Stage-панель (desired-размер, клампится `constrain` в viewport).
const STAGE: UiVec2 = UiVec2::new(1200.0, 760.0);
/// Отступ stage → корневой слот графа.
const PAD: f32 = 16.0;
/// Базовый зазор всех строк/сеток (ui px).
const GAP: f32 = 8.0;
/// Строк корневого Column (9 линейных Row + 1 grid-строка).
const ROWS: usize = 10;
/// Листьев в строке.
const ITEMS_PER_ROW: usize = 100;
/// РОВНО 1000 листовых rect'ов за reflow (определение «1000 узлов» §8).
const NODES: usize = ROWS * ITEMS_PER_ROW;
/// Бюджет контракта §8 FR-068: reflow 1000 узлов < 1 мс.
const BUDGET_US: f64 = 1000.0;
/// Порог дрейфа против baseline (§8: регрессия > 20% — fail).
const DRIFT: f64 = 0.20;
/// Прогревочные прогоны (не замеряются).
const WARMUP: usize = 20;
/// Замеряемые итерации reflow.
const ITERATIONS: usize = 200;

/// Одна строка графа: линейная раскладка (`Row`) или явная сетка.
enum RowCase {
    Linear {
        row: Row,
        items: Vec<Child>,
    },
    Grid {
        cols: Vec<f32>,
        rows: usize,
        row_h: f32,
        gap: UiVec2,
    },
}

/// Синтетический граф: корень-Column + 10 строковых случаев.
struct Graph {
    column: Column,
    column_children: Vec<Child>,
    rows: Vec<RowCase>,
}

/// Строки 0–4: `Fit`, малые листья (слот не переполняется), каждый
/// 4-й — flex (grow = 1); main/cross циклируются от индекса строки.
fn fit_row(i: usize) -> RowCase {
    let row = Row {
        gap: GAP,
        main: [MainAlign::Start, MainAlign::SpaceBetween, MainAlign::End][i % 3],
        cross: [CrossAlign::Start, CrossAlign::Center, CrossAlign::End][i % 3],
        policy: RowPolicy::Fit,
    };
    let items = (0..ITEMS_PER_ROW)
        .map(|j| {
            let g = i * ITEMS_PER_ROW + j;
            let w = 2.0 + (g * 3 % 4) as f32;
            let h = 16.0 + (g * 13 % 24) as f32;
            if j % 4 == 0 {
                Child::flexible(w, h, 1.0)
            } else {
                Child::fixed(w, h)
            }
        })
        .collect();
    RowCase::Linear { row, items }
}

/// Строки 5–6: `Wrap` — ширины 40–130 px, жадная упаковка ≈ 9 строк.
fn wrap_row(i: usize) -> RowCase {
    let row = Row {
        gap: GAP,
        main: MainAlign::Start,
        cross: CrossAlign::Start,
        policy: RowPolicy::Wrap,
    };
    let items = (0..ITEMS_PER_ROW)
        .map(|j| {
            let g = i * ITEMS_PER_ROW + j;
            Child::fixed(40.0 + (g * 11 % 90) as f32, 18.0 + (g * 7 % 22) as f32)
        })
        .collect();
    RowCase::Linear { row, items }
}

/// Строки 7–8: `SqueezeTail` — ширины 60–180 px (Σ ≫ слота), хвост до 0.
/// В графе осознанно есть: FlexLayoutEngine реализует политику дословно
/// (§Контракт-4 FR-068), в отличие от perf_taffy.rs (расхождение C3).
fn squeeze_row(i: usize) -> RowCase {
    let row = Row {
        gap: GAP,
        main: MainAlign::Start,
        cross: CrossAlign::Center,
        policy: RowPolicy::SqueezeTail,
    };
    let items = (0..ITEMS_PER_ROW)
        .map(|j| {
            let g = i * ITEMS_PER_ROW + j;
            Child::fixed(60.0 + (g * 17 % 120) as f32, 20.0 + (g * 5 % 28) as f32)
        })
        .collect();
    RowCase::Linear { row, items }
}

/// Строка 9: явная сетка 10 колонок × 10 строк = 100 ячеек (F-16).
fn grid_row(i: usize) -> RowCase {
    let cols = (0..10).map(|c| 80.0 + (c * 23 % 60) as f32).collect();
    RowCase::Grid {
        cols,
        rows: 10,
        row_h: 24.0 + (i * 3 % 16) as f32,
        gap: UiVec2::new(GAP, GAP),
    }
}

/// Детерминированная сборка графа (без RNG, чистая функция констант).
fn build_graph() -> Graph {
    let column = Column {
        gap: GAP,
        main: MainAlign::Start,
        cross: CrossAlign::Start,
    };
    let inner_w = STAGE.x - 2.0 * PAD; // ширина корневого слота после pad
    let column_children = (0..ROWS)
        .map(|i| Child::fixed(inner_w, 48.0 + (i * 7 % 32) as f32))
        .collect();
    let rows = (0..5)
        .map(fit_row)
        .chain((5..7).map(wrap_row))
        .chain((7..9).map(squeeze_row))
        .chain(std::iter::once(grid_row(9)))
        .collect();
    Graph {
        column,
        column_children,
        rows,
    }
}

/// Один полный reflow от корня ЧЕРЕЗ FlexLayoutEngine (то, что
/// замеряется): stack → constrain → pad → Column → строки/grid. Кладёт
/// в `out` РОВНО 1000 листовых rect'ов (контейнерные rect'ы строк
/// существуют внутри, но не добавляются — определение «узла» см. в
/// модульной доке).
fn reflow(graph: &Graph, out: &mut Vec<UiRect>) {
    out.clear();
    let viewport = black_box(VIEWPORT);
    let desired = constrain(
        UiVec2::new(320.0, 240.0),
        UiVec2::new(viewport.w, viewport.h),
        STAGE,
    );
    let stage = stack(viewport, desired, HAlign::Center, VAlign::Center);
    let root_slot = pad(stage, EdgeInsets::uniform(PAD));
    let row_slots = graph
        .column
        .lay_out_with(&BACKEND, root_slot, &graph.column_children);
    for (case, &slot) in graph.rows.iter().zip(&row_slots) {
        match case {
            RowCase::Linear { row, items } => out.extend(row.lay_out_with(&BACKEND, slot, items)),
            RowCase::Grid {
                cols,
                rows,
                row_h,
                gap,
            } => out.extend(grid_cells_with(&BACKEND, slot, cols, *rows, *row_h, *gap)),
        }
    }
}

/// Медиана (чётное N — среднее двух центральных значений).
fn median(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    if n % 2 == 1 {
        samples[n / 2]
    } else {
        (samples[n / 2 - 1] + samples[n / 2]) / 2.0
    }
}

/// Строка `median_us=` из текста baseline (первое вхождение).
fn parse_baseline(text: &str) -> Option<f64> {
    text.lines()
        .find_map(|l| l.strip_prefix("median_us=")?.trim().parse().ok())
}

/// ISO-дата из unix-секунд без внешних крейтов (civil-from-days,
/// Howard Hinnant; алгоритм public domain, детерминирован).
fn iso_date_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Текст baseline-файла: комментарий (дата/определение графа/
/// машинно-зависимость/W2-стаб) + одна строка `median_us=<число>`.
fn baseline_text(median_us: f64) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "# CanvasDesk FR-068 W2 — perf baseline reflow на FlexLayoutEngine (crates/canvas-ui/tests/perf_flex.rs)\n\
         # Граф (идентичен perf_baseline.rs): viewport 1280×800 → constrain/stack (stage 1200×760) → pad(16) →\n\
         # Column(10) через ЯВНЫЙ FlexLayoutEngine → 5×Row Fit + 2×Row Wrap + 2×Row SqueezeTail (по 100 Child) +\n\
         # grid_cells 10×10 = РОВНО 1000 листовых rect'ов за reflow; размеры детерминированы формулами от индекса (без RNG).\n\
         # Контейнерные rect'ы строк в 1000 не входят. SqueezeTail в графе осознанно: Flex реализует политику дословно (§Контракт-4).\n\
         # Метрика: медиана {ITERATIONS} reflow-прогонов после {WARMUP} прогревочных, μs; профиль cargo test (debug).\n\
         # Baseline МАШИННО-ЗАВИСИМ — референс dev-машины; сравнение: регрессия > 20% — fail, улучшение > 20% — warn (§8 FR-068).\n\
         # ⚠️ W2-стаб: FlexLayoutEngine может делегировать NativeBackend — финальный baseline перегенерируется лидом\n\
         # на интеграции волны W2 (CANVAS_UI_UPDATE_PERF_FLEX=1).\n\
         # Дата записи: {date}\n\
         median_us={median_us:.3}\n",
        date = iso_date_from_unix(secs),
    )
}

/// Perf-гейт FR-068 §W2: reflow 1000 узлов на `FlexLayoutEngine` —
/// медиана < 1 мс; против baseline `tests/perf_flex_baseline.txt` —
/// регрессия > 20% — fail, улучшение > 20% — предупреждение. Замер
/// машинно-зависим: baseline — референс dev-машины; на другой машине
/// возможен ложный fail по дрейфу — тогда обнови baseline осознанно
/// (`CANVAS_UI_UPDATE_PERF_FLEX=1`). W2-стаб может замерять
/// Native-делегацию (см. доку «W2-стаб») — финальный baseline
/// перегенерируется лидом на интеграции.
#[test]
#[ignore = "perf-замер (машинно-зависим): cargo test -p canvas-ui --test perf_flex -- --ignored"]
fn perf_flex_reflow_1000() {
    let graph = build_graph();
    black_box(&graph);
    let mut out = Vec::with_capacity(NODES);

    // Санити определения графа: РОВНО 1000 листовых rect'ов за reflow.
    reflow(&graph, &mut out);
    assert_eq!(
        out.len(),
        NODES,
        "число листовых узлов сместилось — обнови doc-комментарий и baseline осознанно"
    );
    // Детерминизм: чистые функции — два reflow побитово совпадают.
    let mut again = Vec::with_capacity(out.len());
    reflow(&graph, &mut again);
    assert_eq!(out, again, "reflow через FlexLayoutEngine недетерминирован");
    black_box(&out);

    // Прогрев (не замеряется): аллокатор, ветвления, страничные промахи.
    for _ in 0..WARMUP {
        reflow(&graph, &mut out);
    }
    black_box(&out);

    // Замер: N итераций полного reflow от корня, медиана в μs.
    let mut samples = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let t0 = Instant::now();
        reflow(&graph, &mut out);
        let dt = t0.elapsed();
        black_box(&out);
        samples.push(dt.as_nanos() as f64 / 1_000.0);
    }
    let med = median(&mut samples);
    black_box(med);
    println!(
        "reflow {NODES} узлов через FlexLayoutEngine: median = {med:.1} μs ({ITERATIONS} итераций после {WARMUP} прогревочных; бюджет §W2 < {BUDGET_US:.0} μs)"
    );

    // Абсолютный бюджет контракта §8 FR-068 — на любой машине (движок —
    // чистая f32-арифметика 0 deps: медленных dep-веток dev-профиля,
    // как у taffy, нет — см. доку «Бюджеты»).
    assert!(
        med < BUDGET_US,
        "reflow 1000 узлов (FlexLayoutEngine): median {med:.1} μs ≥ бюджет {BUDGET_US:.0} μs \
         (FR-068 §W2: reflow < 1 мс на FlexLayoutEngine). \
         Замер машинно-зависим: проверь профиль (debug/release) и машину, прежде чем менять движок."
    );

    // Baseline: обновление по флагу, иначе сравнение с дрейфом ±20%.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("perf_flex_baseline.txt");
    if std::env::var("CANVAS_UI_UPDATE_PERF_FLEX").is_ok_and(|v| v == "1") {
        fs::write(&path, baseline_text(med)).expect("запись tests/perf_flex_baseline.txt");
        println!("baseline обновлён: {} (median_us={med:.3})", path.display());
        return;
    }
    let recorded = fs::read_to_string(&path)
        .ok()
        .and_then(|t| parse_baseline(&t));
    match recorded {
        Some(baseline) => {
            let drift_pct = (med / baseline - 1.0) * 100.0;
            assert!(
                med <= baseline * (1.0 + DRIFT),
                "регрессия perf-flex: baseline {baseline:.1} μs → текущая {med:.1} μs \
                 (+{drift_pct:.0}% > +20%). FR-068 §W2: регрессия > 20% — фиксируй осознанно: \
                 либо исправь деградацию, либо обнови baseline отдельным коммитом с пояснением \
                 (CANVAS_UI_UPDATE_PERF_FLEX=1 cargo test -p canvas-ui --test perf_flex -- --ignored). \
                 Baseline машинно-зависим — на другой машине/в другом профиле обнови его осознанно."
            );
            if med < baseline * (1.0 - DRIFT) {
                println!(
                    "WARN: улучшение > 20% против baseline ({baseline:.1} μs → {med:.1} μs, {drift_pct:.0}%) — не fail; \
                     рассмотри осознанное обновление baseline: CANVAS_UI_UPDATE_PERF_FLEX=1"
                );
            }
            println!("baseline {baseline:.1} μs — дрейф {drift_pct:+.1}% в пределах ±20%: ок");
        }
        None => println!(
            "WARN: {} отсутствует или без строки median_us= — сравнение с baseline пропущено \
             (абсолютный бюджет §W2 проверен). Создай baseline: \
             CANVAS_UI_UPDATE_PERF_FLEX=1 cargo test -p canvas-ui --test perf_flex -- --ignored",
            path.display()
        ),
    }
}
