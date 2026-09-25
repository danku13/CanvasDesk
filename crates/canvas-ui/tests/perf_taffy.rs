//! Perf-гейт reflow 1000 узлов на `TaffyBackend` (FR-068 W1; контракт §8
//! FR-068, docs/change-requests/fr-068-ui-refactoring-long-term.md;
//! ADR-0014 — taffy opt-in за фичей `taffy`).
//!
//! # Что измеряется
//!
//! Один полный immediate-mode reflow синтетического UI-графа от корня
//! ПОЛНОСТЬЮ через [`TaffyBackend`] (каждый вызов `lay_out_*` строит
//! одноразовое `TaffyTree` → `compute_layout` → чтение rect'ов — D2
//! immediate-mode ADR-0014): `stack` → `constrain` → `pad` →
//! `Column::lay_out_with` (10 контейнерных строк) → 7 × `Row::lay_out_with`,
//! 2 × `Column::lay_out_with` и `grid_cells_with`. Метрика — МЕДИАННАЯ
//! длительность одного reflow в μs по `ITERATIONS` замерам (`Instant`),
//! после `WARMUP` прогревочных прогонов (не замеряются). canvas-ui —
//! нативный тест: `std::time` доступен.
//!
//! # Определение синтетического графа (зафиксировано, детерминировано)
//!
//! Аналог `perf_baseline.rs` (W0), НО БЕЗ `SqueezeTail`: политика
//! мапится в taffy на `flex_shrink` — документированное расхождение C3
//! (§Контракт-4 FR-068), замерять её в parity-гейте осознанно не
//! выносим: граф держится только на побитово-паритетных политиках
//! (Fit/Wrap/grid/Column). Viewport 1280×800, stage 1200×760, внутренний
//! слот = stage − pad 16 px. «Узел» = ЛИСТОВОЙ rect; итого РОВНО 1000
//! узлов (10 случаев × 100 листьев):
//!
//! - случаи 0–4: `RowPolicy::Fit`, 100 Child (каждый 4-й — `flexible`,
//!   grow = 1; малые ширины — слот не переполняется), main/cross
//!   варьируются от индекса;
//! - случаи 5–6: `RowPolicy::Wrap`, 100 Child шириной 40–130 px —
//!   жадная упаковка ≈ 9 строк переноса (строки ниже слота не
//!   маскируются — F-15, для замера осознанно);
//! - случаи 7–8: `Column` (вертикальный flex, FR-062 F-14), 100 Child
//!   высотой 2–6 px — сумма ≫ высоты слота: переполнение по главной оси
//!   видно (Fit-политика колонки, без сжатия — flex_shrink у детей 0);
//! - случай 9: `grid_cells` — 10 явных колонок × 10 строк = 100 ячеек.
//!
//! Контейнерные rect'ы строк в 1000 НЕ входят (определение «узла» —
//! как в `perf_baseline.rs`). Все размеры — формулы от глобального
//! индекса (без RNG).
//!
//! # Профиль замера (ВАЖНО — отличие от `perf_baseline`)
//!
//! Абсолютный бюджет §8 (< 1 мс) валидируется в RELEASE-профиле: в dev
//! (дефолт `cargo test`) вся dep-дерево (taffy и его транзитивные)
//! компилируется без оптимизаций (opt-level 0), что даёт ~10× к CPU
//! taffy: замер dev-профиля на dev-машине — ~2.7 мс (профиль, не движок;
//! вердикт движка — 262 мкс в release, запас ×3.8 от бюджета). Порог НЕ
//! ослаблен: в dev-профиле тест печатает INFO с числом и командой
//! release-прогона и держит ОТНОСИТЕЛЬНЫЙ гейт дрейфа ±20% против
//! baseline-строки СВОЕГО профиля; в release — полный ассерт бюджета.
//! (Опция фикса dev-замера — `[profile.dev.package.taffy] opt-level=3`
//! в корневом Cargo.toml — решение владельца, вне скоупа этой задачи.)
//!
//! # Машинно-зависимость (осознанная)
//!
//! Baseline-файл `tests/perf_taffy_baseline.txt` — референс dev-машины,
//! хранит ОБЕ строки — `median_us_debug=` и `median_us_release=` — и
//! коммитится. Сравнение — против строки СВОЕГО профиля: регрессия >
//! 20% — fail, улучшение > 20% — предупреждение (не fail).
//!
//! # Бюджеты
//!
//! - Абсолютный (release, любая машина): медиана < 1_000_000 нс = 1 мс
//!   на 1000 узлов (§8 FR-068 — тот же контракт, что у `perf_baseline`).
//! - Относительный (baseline dev-машины, оба профиля): дрейф > 20% —
//!   сигнал. W3 FR-068 сравнит release-цифру с retained-кэшем дерева
//!   (taffy пересоздаётся на кадр — осознанная цена W1, ADR-0014).
//!
//! # Запуск
//!
//! ```text
//! # гейт (абсолютный бюджет §8):
//! cargo test -p canvas-ui --features taffy --release --test perf_taffy -- --ignored --nocapture
//! # дрейф-гейт dev-профиля (дефолтный cargo test):
//! cargo test -p canvas-ui --features taffy --test perf_taffy -- --ignored --nocapture
//! # регенерация baseline (запусти В ОБОИХ профилях — файл хранит обе строки):
//! CANVAS_UI_UPDATE_PERF_TAFFY=1 cargo test -p canvas-ui --features taffy --test perf_taffy -- --ignored
//! CANVAS_UI_UPDATE_PERF_TAFFY=1 cargo test -p canvas-ui --features taffy --release --test perf_taffy -- --ignored
//! ```
//! Без фичи `taffy` файл компилируется в пустой тестовый бинарник
//! (zero-dep инвариант G7, §Контракт-2 FR-068).
#![cfg(feature = "taffy")]

use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use canvas_ui::layout::{
    constrain, grid_cells_with, pad, stack, Child, Column, CrossAlign, HAlign, MainAlign, Row,
    RowPolicy, TaffyBackend, VAlign,
};
use canvas_ui::{EdgeInsets, UiRect, UiVec2};

/// Backend замера: полный taffy-путь (каждый контейнер — одноразовое
/// TaffyTree, см. модульную доку).
const BACKEND: TaffyBackend = TaffyBackend;

/// Каноническое G4-окно (FR-068 W0): 1280×800.
const VIEWPORT: UiRect = UiRect::new(0.0, 0.0, 1280.0, 800.0);
/// Stage-панель (desired-размер, клампится `constrain` в viewport).
const STAGE: UiVec2 = UiVec2::new(1200.0, 760.0);
/// Отступ stage → корневой слот графа.
const PAD: f32 = 16.0;
/// Базовый зазор всех строк/колонок/сеток (ui px).
const GAP: f32 = 8.0;
/// Строк корневого Column (7 Row + 2 Column + 1 grid-строка).
const ROWS: usize = 10;
/// Листьев в строке/колонке.
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

/// Один слот корневого Column: линейная раскладка (`Row`/`Column`) или
/// явная сетка.
enum RowCase {
    Row {
        row: Row,
        items: Vec<Child>,
    },
    Column {
        column: Column,
        items: Vec<Child>,
    },
    Grid {
        cols: Vec<f32>,
        rows: usize,
        row_h: f32,
        gap: UiVec2,
    },
}

/// Синтетический граф: корень-Column + 10 случаев.
struct Graph {
    column: Column,
    column_children: Vec<Child>,
    cases: Vec<RowCase>,
}

/// Случаи 0–4: `Fit`, малые листья (слот не переполняется), каждый
/// 4-й — flex (grow = 1); main/cross циклируются от индекса случая.
fn fit_case(i: usize) -> RowCase {
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
    RowCase::Row { row, items }
}

/// Случаи 5–6: `Wrap` — ширины 40–130 px, жадная упаковка ≈ 9 строк.
fn wrap_case(i: usize) -> RowCase {
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
    RowCase::Row { row, items }
}

/// Случаи 7–8: `Column` (F-14) — низкие листья (2–6 px), сумма высот ≫
/// слота: переполнение вниз видно (без сжатия — политика Fit, гибкие
/// дети не активируются: свободного места нет). Каждый 4-й — flexible
/// для однородности дерева с Fit-строками.
fn column_case(i: usize) -> RowCase {
    let column = Column {
        gap: GAP,
        main: MainAlign::Start,
        cross: CrossAlign::Start,
    };
    let items = (0..ITEMS_PER_ROW)
        .map(|j| {
            let g = i * ITEMS_PER_ROW + j;
            let w = 40.0 + (g * 7 % 60) as f32;
            let h = 2.0 + (g * 3 % 4) as f32;
            if j % 4 == 0 {
                Child::flexible(w, h, 1.0)
            } else {
                Child::fixed(w, h)
            }
        })
        .collect();
    RowCase::Column { column, items }
}

/// Случай 9: явная сетка 10 колонок × 10 строк = 100 ячеек (F-16).
fn grid_case(i: usize) -> RowCase {
    let cols = (0..10).map(|c| 80.0 + (c * 23 % 60) as f32).collect();
    RowCase::Grid {
        cols,
        rows: 10,
        row_h: 24.0 + (i * 3 % 16) as f32,
        gap: UiVec2::new(GAP, GAP),
    }
}

/// Детерминированная сборка графа (без RNG, чистая функция констант):
/// 5×Fit + 2×Wrap + 2×Column + grid (БЕЗ SqueezeTail — C3, см. доку).
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
    let cases = (0..5)
        .map(fit_case)
        .chain((5..7).map(wrap_case))
        .chain((7..9).map(column_case))
        .chain(std::iter::once(grid_case(9)))
        .collect();
    Graph {
        column,
        column_children,
        cases,
    }
}

/// Один полный reflow от корня ЧЕРЕЗ TaffyBackend (то, что замеряется):
/// stack → constrain → pad → Column → случаи. Кладёт в `out` РОВНО 1000
/// листовых rect'ов.
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
    for (case, &slot) in graph.cases.iter().zip(&row_slots) {
        match case {
            RowCase::Row { row, items } => out.extend(row.lay_out_with(&BACKEND, slot, items)),
            RowCase::Column { column, items } => {
                out.extend(column.lay_out_with(&BACKEND, slot, items))
            }
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

/// Активный профиль: dev (`cargo test`) или release (`--release`);
/// определяет строку baseline и режим абсолютного бюджета (см. доку
/// «Профиль замера»).
const RELEASE_PROFILE: bool = !cfg!(debug_assertions);

/// Ключ строки baseline активного профиля.
fn baseline_key() -> &'static str {
    if RELEASE_PROFILE {
        "median_us_release="
    } else {
        "median_us_debug="
    }
}

/// Строка baseline АКТИВНОГО профиля из текста файла (первое вхождение;
/// файл хранит обе строки — debug и release).
fn parse_baseline(text: &str) -> Option<f64> {
    let key = baseline_key();
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.trim().parse().ok())
}

/// Строка baseline ДРУГОГО профиля (сохраняется при регенерации; None —
/// ещё не записана).
fn parse_baseline_other_profile(text: &str) -> Option<f64> {
    let other = if RELEASE_PROFILE {
        "median_us_debug="
    } else {
        "median_us_release="
    };
    text.lines()
        .find_map(|l| l.strip_prefix(other)?.trim().parse().ok())
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
/// машинно-зависимость) + ДВЕ строки — `median_us_debug=` и
/// `median_us_release=`. Обе обязательны: гейт дрейфа сравнивает замер
/// со строкой СВОЕГО профиля (см. доку «Профиль замера»). Существующая
/// строка другого профиля (`previous`) сохраняется: регенерация одного
/// профиля не затирает другой.
fn baseline_text(median_us: f64, previous: Option<&str>) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let keep_other =
        |other: Option<f64>, hint: &str| other.map(|v| format!("median_us_{hint}{v:.3}"));
    let (debug_line, release_line) = if RELEASE_PROFILE {
        (
            keep_other(
                previous.and_then(parse_baseline_other_profile),
                "debug=",
            )
            .unwrap_or_else(|| {
                "# median_us_debug= — запусти ещё и в dev-профиле с CANVAS_UI_UPDATE_PERF_TAFFY=1".to_string()
            }),
            format!("median_us_release={median_us:.3}"),
        )
    } else {
        (
            format!("median_us_debug={median_us:.3}"),
            keep_other(
                previous.and_then(parse_baseline_other_profile),
                "release=",
            )
            .unwrap_or_else(|| {
                "# median_us_release= — запусти ещё и в release (--release) с CANVAS_UI_UPDATE_PERF_TAFFY=1".to_string()
            }),
        )
    };
    format!(
        "# CanvasDesk FR-068 W1 — perf baseline reflow на TaffyBackend (crates/canvas-ui/tests/perf_taffy.rs)\n\
         # Граф: viewport 1280×800 → constrain/stack (stage 1200×760) → pad(16) → Column(10) через TaffyBackend →\n\
         # 5×Row Fit+flex + 2×Row Wrap + 2×Column (по 100 Child) + grid_cells 10×10 = РОВНО 1000 листовых rect'ов за reflow.\n\
         # БЕЗ SqueezeTail: маппинг в flex_shrink — документированное расхождение C3 (§Контракт-4 FR-068);\n\
         # каждый контейнер = одноразовое TaffyTree (immediate-mode D2 ADR-0014). Размеры детерминированы формулами от индекса.\n\
         # Метрика: медиана {ITERATIONS} reflow-прогонов после {WARMUP} прогревочных, μs.\n\
         # Абсолютный бюджет §8 (< 1 мс) валидируется в RELEASE-профиле (dev: deps без оптимизаций — см. доку теста);\n\
         # гейт дрейфа: регрессия > 20% против строки СВОЕГО профиля — fail, улучшение > 20% — warn.\n\
         # Baseline МАШИННО-ЗАВИСИМ — референс dev-машины.\n\
         # Дата записи: {date}\n\
         {debug_line}\n\
         {release_line}\n",
        date = iso_date_from_unix(secs),
    )
}

/// Perf-гейт FR-068 W1 (контракт §8) на `TaffyBackend`: reflow 1000
/// узлов — медиана < 1 мс (абсолютный ассерт — в RELEASE-профиле: в dev
/// деп-дерево компилируется без оптимизаций, см. доку «Профиль
/// замера»); против baseline `tests/perf_taffy_baseline.txt` регрессия
/// сверх 20% (против строки СВОЕГО профиля) — fail, улучшение сверх
/// 20% — предупреждение. Замер машинно-зависим: baseline — референс
/// dev-машины; на другой машине возможен ложный fail по дрейфу — тогда
/// обнови baseline осознанно (`CANVAS_UI_UPDATE_PERF_TAFFY=1`).
#[test]
#[ignore = "perf-замер (машинно-зависим): cargo test -p canvas-ui --features taffy --release --test perf_taffy -- --ignored"]
fn perf_taffy_reflow_1000() {
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
    assert_eq!(out, again, "reflow через TaffyBackend недетерминирован");
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
        "reflow {NODES} узлов через TaffyBackend: median = {med:.1} μs ({ITERATIONS} итераций после {WARMUP} прогревочных; бюджет §8 < {BUDGET_US:.0} μs)"
    );

    // Абсолютный бюджет контракта §8 FR-068 — в release-профиле (в dev
    // деп-дерево taffy компилируется без оптимизаций: замер ~10× к
    // движку — профиль, не регрессия; см. доку «Профиль замера»). Порог
    // НЕ ослаблен: в dev гейтит дрейф против debug-строки baseline.
    if RELEASE_PROFILE {
        assert!(
            med < BUDGET_US,
            "reflow 1000 узлов (TaffyBackend): median {med:.1} μs ≥ бюджет {BUDGET_US:.0} μs \
             (§8 FR-068: reflow < 1 мс). Замер машинно-зависим: проверь машину; \
             W3 FR-068 добавит retained-кэш дерева — см. ADR-0014 (taffy пересоздаётся на кадр в W1)."
        );
    } else {
        println!(
            "INFO: dev-профиль (deps без оптимизаций) — абсолютный бюджет §8 (< {BUDGET_US:.0} μs) НЕ ассертится здесь; \
             вердикт движка: cargo test -p canvas-ui --features taffy --release --test perf_taffy -- --ignored \
             (текущий dev-замер: {med:.1} μs)"
        );
    }

    // Baseline: обновление по флагу (с сохранением строки другого
    // профиля), иначе сравнение с дрейфом ±20% против строки СВОЕГО
    // профиля.
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("perf_taffy_baseline.txt");
    if std::env::var("CANVAS_UI_UPDATE_PERF_TAFFY").is_ok_and(|v| v == "1") {
        let previous = fs::read_to_string(&path).ok();
        fs::write(&path, baseline_text(med, previous.as_deref()))
            .expect("запись tests/perf_taffy_baseline.txt");
        println!(
            "baseline обновлён: {} ({}{med:.3})",
            path.display(),
            baseline_key()
        );
        return;
    }
    let file_text = fs::read_to_string(&path).ok();
    let recorded = file_text.as_deref().and_then(parse_baseline);
    match recorded {
        Some(baseline) => {
            let drift_pct = (med / baseline - 1.0) * 100.0;
            assert!(
                med <= baseline * (1.0 + DRIFT),
                "регрессия perf-taffy: baseline {baseline:.1} μs → текущая {med:.1} μs \
                 (+{drift_pct:.0}% > +20%). §8 FR-068: регрессия > 20% — фиксируй осознанно: \
                 либо исправь деградацию, либо обнови baseline отдельным коммитом с пояснением \
                 (CANVAS_UI_UPDATE_PERF_TAFFY=1 cargo test -p canvas-ui --features taffy \
                 [--release] --test perf_taffy -- --ignored). Baseline машинно-зависим — на другой машине/\
                 в другом профиле обнови его осознанно."
            );
            if med < baseline * (1.0 - DRIFT) {
                println!(
                    "WARN: улучшение > 20% против baseline ({baseline:.1} μs → {med:.1} μs, {drift_pct:.0}%) — не fail; \
                     рассмотри осознанное обновление baseline: CANVAS_UI_UPDATE_PERF_TAFFY=1"
                );
            }
            println!("baseline {baseline:.1} μs — дрейф {drift_pct:+.1}% в пределах ±20%: ок");
        }
        None => println!(
            "WARN: {} отсутствует или без строки {} — сравнение с baseline пропущено \
             (гейт дрейфа не активен). Создай baseline в ОБОИХ профилях: \
             CANVAS_UI_UPDATE_PERF_TAFFY=1 cargo test -p canvas-ui --features taffy [--release] --test perf_taffy -- --ignored",
            path.display(),
            baseline_key()
        ),
    }
}
