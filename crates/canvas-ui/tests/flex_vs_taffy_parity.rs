#![cfg(feature = "taffy")]
//! Паритет-тест `FlexLayoutEngine` ↔ `TaffyBackend` (FR-068 §W2, гейт
//! «1000 случайных деревьев, побитовая идентичность ≥ 80% на совместимых
//! политиках»; ADR-0015 §Решение п.3 — собственный 0-dep движок вместо
//! taffy; файл-таблица FR-068 §W2: `test(ui): flex_vs_taffy_parity`).
//!
//! # Методология
//!
//! - 1000 детерминированных случайных деревьев трёх категорий:
//!   (а) `Row` — политики `Fit`/`Wrap`, main ∈ {Start, SpaceBetween, End},
//!   cross ∈ {Start, Center, End}, gap ∈ {0, 2, 4, 6, 8, 12}, 1..=24 детей
//!   fixed/flexible (400 деревьев); (б) `Column` — аналогично, без политик
//!   (у колонки их нет; 300 деревьев); (в) `lay_out_grid` — равные колонки
//!   (3..=8 одинаковой ширины — целой или X.5), строки 1..=6, высота строки
//!   целая (300 деревьев). Слот каждого дерева — случайный ЦЕЛОЧИСЛЕННЫЙ
//!   rect (дробный слот — территория round-layout, покрыта оракулом ниже).
//! - Сравнение ПОБИТОВОЕ: `f32::to_bits` по x/y/w/h каждого rect (стиль
//!   parity-оракула W1 `backend_parity.rs`; NaN/±0 паттернов нет — входы
//!   целые или X.5 положительные). Дерево «совпало», если совпали ВСЕ его
//!   rect'ы; гейт — доля совпавших деревьев ≥ 0.80.
//! - PRNG — splitmix64 (Steele/Lemire), zero-dep (std only, инвариант G7 —
//!   `rand` в dev-deps не тянем), фиксированные seed-константы на категорию:
//!   прогон полностью воспроизводим (детерминизм F-18).
//!
//! # Исключения из случайной выборки (документированные расхождения)
//!
//! Известные семантические расхождения backend'ов ИСКЛЮЧЕНЫ из случайной
//! выборки и покрыты отдельными тестами — они НЕ считаются против гейта:
//!
//! 1. **Дробный grow → round-on-freeze** (CSS flexbox §9.7): taffy
//!    округляет целевые main-размеры flex-детей до целой px-сетки, поэтому
//!    неокруглённая арифметика даёт расхождение ≤ 0.5 ui px. В выборке —
//!    ТОЛЬКО целые grow-доли {0.0, 1.0, 2.0} (0.5 из палитры спеки исключён)
//!    и целые размеры ОСНОВНОЙ оси; X.5 допущены в ПОПЕРЕЧНОЙ оси (приведён
//!    к единой px-сетке round-layout'ом — финальный `FlexLayoutEngine`
//!    зеркалит `taffy::compute::round_layout`, см. доку `flex.rs`) и в треках
//!    равной сетки (категория (в) — оракул round_layout taffy). Гэпы — целые
//!    по спеке (набор выше).
//! 2. **`SqueezeTail` ≠ `flex_shrink`** (расхождение C3, §Контракт-4
//!    FR-068): `FlexLayoutEngine` реализует политику ДОСЛОВНО (каждому —
//!    `min(desired, остаток)`, хвост сжимается до нулевой ширины, half-open
//!    hit — семантика what-if бара FR-017/CR-015, «FlexLayoutEngine
//!    решает»), taffy мапит её на CSS flex_shrink (сжатие распределяется по
//!    ВСЕМ детям пропорционально shrink×basis). Политика исключена из
//!    выборки и покрыта отдельным пином
//!    [`squeeze_tail_c3_divergence_documented`]: расхождение ЗАДАНО
//!    контрактом, а не дефект паритета.
//!
//! # Запуск
//!
//! ```text
//! cargo test -p canvas-ui --test flex_vs_taffy_parity --features taffy
//! ```
//! Без фичи `taffy` файл компилируется в пустой бинарник — default-сборка
//! не тянет taffy (zero-dep инвариант G7, §Контракт-2 FR-068).

use canvas_ui::layout::{
    grid_cells_with, Child, Column, CrossAlign, FlexLayoutEngine, LayoutBackend, MainAlign, Row,
    RowPolicy, TaffyBackend,
};
use canvas_ui::{UiRect, UiVec2};

// --- Детерминированный PRNG (zero-dep) ---------------------------------------

/// splitmix64 (Steele, Lea, Flood — «Fast splittable pseudorandom number
/// generators»): детерминированный PRNG без внешних зависимостей (std only;
/// инвариант G7 FR-068 — `rand` не входит в deps/dev-deps крейта).
/// Качества splitmix64 достаточно для равномерной выборки параметров
/// деревьев; фиксированные seed-константы делают прогон воспроизводимым.
struct SplitMix64(u64);

impl SplitMix64 {
    /// Новый генератор от фиксированной seed-константы.
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Равномерное u64 в [0, n) (n == 0 → 0).
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }

    /// Целое в [lo, hi] включительно.
    fn int_in(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    /// Размер в ui px: целое в [lo, hi] либо то же + 0.5 с вероятностью
    /// `half_pct`% (X.5 — см. доку: только поперечная ось / треки сетки).
    fn dim_in(&mut self, lo: i64, hi: i64, half_pct: u32) -> f32 {
        let v = self.int_in(lo, hi) as f32;
        if self.below(100) < u64::from(half_pct) {
            v + 0.5
        } else {
            v
        }
    }

    /// Случайный элемент набора (палитры параметров).
    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }

    /// Событие с вероятностью `pct`%.
    fn chance(&mut self, pct: u32) -> bool {
        self.below(100) < u64::from(pct)
    }
}

// --- Палитры параметров выборки ----------------------------------------------

/// Seed-константы категорий (произвольные фиксированные 64-бит значения;
/// смена любого из них меняет выборку — осознанная регенерация гейта).
const SEED_ROW: u64 = 0x9E6C_1D2C_5A5A_0001;
const SEED_COLUMN: u64 = 0x9E6C_1D2C_A5A5_0002;
const SEED_GRID: u64 = 0x9E6C_1D2C_0F0F_0003;

/// Объём выборки FR-068 §W2 (гейт: 1000 деревьев).
const TOTAL_TREES: usize = 1000;
/// Доли категорий: 40% row (Fit/Wrap), 30% column, 30% grid.
const ROW_TREES: usize = 400;
const COLUMN_TREES: usize = 300;
const GRID_TREES: usize = 300; // ROW + COLUMN + GRID == TOTAL_TREES

/// Гэпы по спеке §W2 (целые; X.5-гэпы сознательно не вводим — см. доку).
const GAPS: [f32; 6] = [0.0, 2.0, 4.0, 6.0, 8.0, 12.0];
const MAINS: [MainAlign; 3] = [MainAlign::Start, MainAlign::SpaceBetween, MainAlign::End];
const CROSSES: [CrossAlign; 3] = [CrossAlign::Start, CrossAlign::Center, CrossAlign::End];
/// ЦЕЛЫЕ grow-доли (исключение 1 модульной доки: дробный grow →
/// round-on-freeze taffy — вне случайной выборки).
const GROW_SHARES: [f32; 3] = [0.0, 1.0, 2.0];

/// Случайный целочисленный слот (x/y — сдвиг, ловит путаницу относительных
/// и абсолютных координат; w/h — рабочие размеры, часто меньше контента —
/// переполнение попадает в выборку, как и свободное место).
fn random_slot(rng: &mut SplitMix64) -> UiRect {
    UiRect::new(
        rng.int_in(0, 160) as f32,
        rng.int_in(0, 160) as f32,
        rng.int_in(200, 1200) as f32,
        rng.int_in(60, 400) as f32,
    )
}

/// Дети для Row: базовый размер ГЛАВНОЙ оси (ширина) — целый; ПОПЕРЕЧНОЙ
/// (высота) — целый или X.5 (исключение 1 модульной доки).
fn row_children(rng: &mut SplitMix64, n: usize) -> Vec<Child> {
    (0..n)
        .map(|_| {
            let basis = rng.int_in(8, 96) as f32;
            let cross = rng.dim_in(8, 48, 50);
            if rng.chance(50) {
                Child::fixed(basis, cross)
            } else {
                Child::flexible(basis, cross, rng.pick(&GROW_SHARES))
            }
        })
        .collect()
}

/// Дети для Column: зеркально — главная ось Y (высота) целая, поперечная
/// X (ширина) — целая или X.5.
fn column_children(rng: &mut SplitMix64, n: usize) -> Vec<Child> {
    (0..n)
        .map(|_| {
            let basis = rng.int_in(8, 96) as f32;
            let cross = rng.dim_in(8, 48, 50);
            if rng.chance(50) {
                Child::fixed(cross, basis)
            } else {
                Child::flexible(cross, basis, rng.pick(&GROW_SHARES))
            }
        })
        .collect()
}

/// Сгенерированное дерево (данные) + единая точка раскладки любым backend'ом.
enum Case {
    Row {
        row: Row,
        slot: UiRect,
        items: Vec<Child>,
    },
    Column {
        column: Column,
        slot: UiRect,
        items: Vec<Child>,
    },
    Grid {
        slot: UiRect,
        cols: Vec<f32>,
        rows: usize,
        row_h: f32,
        gap: UiVec2,
    },
}

fn lay_out_case(backend: &dyn LayoutBackend, case: &Case) -> Vec<UiRect> {
    match case {
        Case::Row { row, slot, items } => backend.lay_out_row(*row, *slot, items),
        Case::Column {
            column,
            slot,
            items,
        } => backend.lay_out_column(*column, *slot, items),
        Case::Grid {
            slot,
            cols,
            rows,
            row_h,
            gap,
        } => grid_cells_with(backend, *slot, cols, *rows, *row_h, *gap),
    }
}

// --- Учёт совпадений / примеры расхождений ------------------------------------

/// Статистика категории (гейт печатает её по всем трём).
struct CategoryStat {
    name: &'static str,
    total: usize,
    matched: usize,
}

/// Детальный пример расхождения (для сообщения assert — до 5 штук).
struct DiffExample {
    category: &'static str,
    tree: usize,
    rect: usize,
    field: &'static str,
    flex: f32,
    taffy: f32,
}

/// Максимальное число сохранённых примеров (в assert попадают первые 5).
const EXAMPLE_CAP: usize = 8;
/// Порог гейта FR-068 §W2: ≥ 80% деревьев побитово.
const GATE_RATIO: f64 = 0.80;

/// Побитовое сравнение одного дерева: совпадение = все rect'ы равны
/// `f32::to_bits` по всем координатам (и длины равны).
fn compare_tree(
    category: &'static str,
    tree_idx: usize,
    flex: &[UiRect],
    taffy: &[UiRect],
    stat: &mut CategoryStat,
    examples: &mut Vec<DiffExample>,
) {
    stat.total += 1;
    if flex.len() != taffy.len() {
        if examples.len() < EXAMPLE_CAP {
            examples.push(DiffExample {
                category,
                tree: tree_idx,
                rect: usize::MAX,
                field: "len",
                flex: flex.len() as f32,
                taffy: taffy.len() as f32,
            });
        }
        return;
    }
    let mut same = true;
    'rects: for (ri, (a, b)) in flex.iter().zip(taffy.iter()).enumerate() {
        for (field, va, vb) in [
            ("x", a.x, b.x),
            ("y", a.y, b.y),
            ("w", a.w, b.w),
            ("h", a.h, b.h),
        ] {
            if va.to_bits() != vb.to_bits() {
                same = false;
                if examples.len() < EXAMPLE_CAP {
                    examples.push(DiffExample {
                        category,
                        tree: tree_idx,
                        rect: ri,
                        field,
                        flex: va,
                        taffy: vb,
                    });
                }
                break 'rects;
            }
        }
    }
    if same {
        stat.matched += 1;
    }
}

/// Рендер примеров расхождений (индекс дерева, rect, биты — как требует
/// спека §W2).
fn render_examples(examples: &[DiffExample]) -> String {
    let mut out = String::new();
    for (i, e) in examples.iter().take(5).enumerate() {
        if e.field == "len" {
            out.push_str(&format!(
                "  {i}) дерево #{} [{}]: число rect'ов flex {} vs taffy {}\n",
                e.tree, e.category, e.flex, e.taffy
            ));
        } else {
            out.push_str(&format!(
                "  {i}) дерево #{} [{}] rect[{}].{}: flex {} (bits {:#010x}) vs taffy {} (bits {:#010x})\n",
                e.tree,
                e.category,
                e.rect,
                e.field,
                e.flex,
                e.flex.to_bits(),
                e.taffy,
                e.taffy.to_bits(),
            ));
        }
    }
    out
}

// --- Гейт: 1000 деревьев -------------------------------------------------------

/// Гейт FR-068 §W2: 1000 случайных деревьев, побитовая идентичность
/// `FlexLayoutEngine` ↔ `TaffyBackend` ≥ 80% (совпавшие деревья / все).
/// Статистика по категориям печатается в stdout (`--nocapture`).
#[test]
fn flex_vs_taffy_1000_trees_bitwise_parity_gate() {
    assert_eq!(
        ROW_TREES + COLUMN_TREES + GRID_TREES,
        TOTAL_TREES,
        "выборка: 1000 деревьев (row + column + grid)"
    );
    let flex = FlexLayoutEngine;
    let taffy = TaffyBackend;

    let mut stats = [
        CategoryStat {
            name: "row Fit/Wrap",
            total: 0,
            matched: 0,
        },
        CategoryStat {
            name: "column",
            total: 0,
            matched: 0,
        },
        CategoryStat {
            name: "grid",
            total: 0,
            matched: 0,
        },
    ];
    let mut examples: Vec<DiffExample> = Vec::new();

    // (а) Row: Fit/Wrap × main × cross × gap, 1..=24 детей fixed/flexible.
    let mut rng = SplitMix64::new(SEED_ROW);
    for i in 0..ROW_TREES {
        let policy = if rng.chance(50) {
            RowPolicy::Fit
        } else {
            RowPolicy::Wrap
        };
        let row = Row {
            gap: rng.pick(&GAPS),
            main: rng.pick(&MAINS),
            cross: rng.pick(&CROSSES),
            policy,
        };
        let n = rng.int_in(1, 24) as usize;
        let slot = random_slot(&mut rng);
        let items = row_children(&mut rng, n);
        let case = Case::Row { row, slot, items };
        let a = lay_out_case(&flex, &case);
        let b = lay_out_case(&taffy, &case);
        compare_tree("row", i, &a, &b, &mut stats[0], &mut examples);
    }

    // (б) Column: аналогично, без политик (у колонки их нет).
    let mut rng = SplitMix64::new(SEED_COLUMN);
    for i in 0..COLUMN_TREES {
        let column = Column {
            gap: rng.pick(&GAPS),
            main: rng.pick(&MAINS),
            cross: rng.pick(&CROSSES),
        };
        let n = rng.int_in(1, 24) as usize;
        let slot = random_slot(&mut rng);
        let items = column_children(&mut rng, n);
        let case = Case::Column {
            column,
            slot,
            items,
        };
        let a = lay_out_case(&flex, &case);
        let b = lay_out_case(&taffy, &case);
        compare_tree("column", i, &a, &b, &mut stats[1], &mut examples);
    }

    // (в) lay_out_grid: равные колонки 3..=8 (целые или X.5 — оракул
    // round_layout), строки 1..=6, целая высота строки, целые гэпы.
    let mut rng = SplitMix64::new(SEED_GRID);
    for i in 0..GRID_TREES {
        let ncols = rng.int_in(3, 8) as usize;
        let col_w = rng.dim_in(20, 120, 50);
        let rows = rng.int_in(1, 6) as usize;
        let row_h = rng.int_in(20, 80) as f32;
        let gap = UiVec2::new(rng.pick(&GAPS), rng.pick(&GAPS));
        let case = Case::Grid {
            slot: random_slot(&mut rng),
            cols: vec![col_w; ncols],
            rows,
            row_h,
            gap,
        };
        let a = lay_out_case(&flex, &case);
        let b = lay_out_case(&taffy, &case);
        compare_tree("grid", i, &a, &b, &mut stats[2], &mut examples);
    }

    // Статистика по категориям + итог.
    let matched: usize = stats.iter().map(|s| s.matched).sum();
    let total: usize = stats.iter().map(|s| s.total).sum();
    for s in &stats {
        let pct = if s.total == 0 {
            0.0
        } else {
            100.0 * s.matched as f64 / s.total as f64
        };
        println!(
            "  {:<14}: {}/{} побитово ({pct:.1}%)",
            s.name, s.matched, s.total
        );
    }
    let ratio = matched as f64 / total as f64;
    println!(
        "  ИТОГО          : {matched}/{total} побитово ({:.1}%) — гейт ≥ {}%",
        ratio * 100.0,
        GATE_RATIO * 100.0
    );
    assert!(
        ratio >= GATE_RATIO,
        "гейт FR-068 §W2 НЕ пройден: побитовый паритет FlexLayoutEngine ↔ \
         TaffyBackend {:.1}% < {}% ({matched}/{total} деревьев).\n\
         Известные исключённые расхождения (не против гейта): дробный grow \
         (round-on-freeze) и SqueezeTail (C3) — см. модульную доку теста.\n\
         Примеры расхождений (до 5):\n{}",
        ratio * 100.0,
        GATE_RATIO * 100.0,
        render_examples(&examples),
    );
}

// --- Пин документированного расхождения C3 (SqueezeTail) ----------------------

/// Пин C3 (§Контракт-4 FR-068): `SqueezeTail` — ИМЕНОВАННАЯ деградация
/// узкого слота с дословной семантикой what-if бара (FR-017/CR-015):
/// каждый ребёнок получает `min(desired, остаток)`, хвост сжимается до
/// нулевой ширины, за слот ничего не выходит. `FlexLayoutEngine`
/// реализует её ДОСЛОВНО («FlexLayoutEngine решает» — решение владельца
/// по §Контракту-4: деградация — часть продукта, а не CSS), taffy мапит
/// политику на CSS flex_shrink — тот сжимает ВСЕХ детей пропорционально
/// shrink×basis, поэтому результаты РАЗНЫЕ. Это документированное
/// расхождение C3: оно задано контрактом, покрыто здесь и НЕ считается
/// против гейта паритета (политика исключена из случайной выборки).
///
/// Фиксированный случай: слот 100, gap 6, три ребёнка по 60 (переполнение
/// 180 + 2·6 − 100 = 92). Flex: (60, 34, 0) — дословный `min(остаток)`;
/// taffy (flex_shrink): распределяет 92 по всем трём (~29.33 каждому).
#[test]
fn squeeze_tail_c3_divergence_documented() {
    let row = Row {
        gap: 6.0,
        policy: RowPolicy::SqueezeTail,
        ..Row::default()
    };
    let slot = UiRect::new(0.0, 0.0, 100.0, 40.0);
    let items = [Child::fixed(60.0, 40.0); 3];

    let flex = FlexLayoutEngine.lay_out_row(row, slot, &items);
    assert_eq!(flex.len(), 3, "число rect'ов = числу детей");
    // Дословная семантика: (60, 34, 0) — хвост сжат до нуля.
    assert_eq!(
        (flex[0].x, flex[0].w),
        (0.0, 60.0),
        "Flex[0]: полный размер 60 (остаток 100)"
    );
    assert_eq!(
        (flex[1].x, flex[1].w),
        (66.0, 34.0),
        "Flex[1]: min(60, остаток 34)"
    );
    assert_eq!(
        flex[2].w, 0.0,
        "пин: Flex-хвост == 0 (дословная деградация)"
    );
    for r in &flex {
        assert!(
            r.x >= slot.x - f32::EPSILON && r.right() <= slot.right() + f32::EPSILON,
            "пин: Flex-результат внутри слота ({r:?})"
        );
    }

    let taffy = TaffyBackend.lay_out_row(row, slot, &items);
    assert_eq!(taffy.len(), 3, "число rect'ов = числу детей");
    for r in &taffy {
        assert!(
            r.x >= slot.x - f32::EPSILON && r.right() <= slot.right() + f32::EPSILON,
            "пин: taffy-результат внутри слота ({r:?}) — flex_shrink сжимает линию до слота"
        );
    }
    println!("Flex  (дословный SqueezeTail): {flex:?}");
    println!("taffy (flex_shrink, C3):       {taffy:?}");

    let bitwise_same = flex.len() == taffy.len()
        && flex.iter().zip(taffy.iter()).all(|(a, b)| {
            a.x.to_bits() == b.x.to_bits()
                && a.y.to_bits() == b.y.to_bits()
                && a.w.to_bits() == b.w.to_bits()
                && a.h.to_bits() == b.h.to_bits()
        });
    assert!(
        !bitwise_same,
        "пин C3: на переполнении taffy (flex_shrink) обязан расходиться с \
         Flex (дословный SqueezeTail) — см. §Контракт-4 FR-068; совпадение \
         означает потерю дословной семантики what-if бара"
    );
}
