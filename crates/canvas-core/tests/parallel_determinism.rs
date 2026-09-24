//! FR-065 P1/P2: тесты детерминизма и flatten-эквивалентности
//! поярусного параллелизма пересчёта DAG.
//!
//! Контракты (`docs/plans/adr-0008-wave-s-plan.md` §5.1/§5.2/§5.7):
//! - **§5.2 — `topo_levels` НОВАЯ, `topo_sort` стабильна.**
//!   Инвариант: `topo_levels(canvas)?.into_iter().flatten().collect::<Vec<_>>()`
//!   == `topo_sort(canvas)?` (побитово, на эталонах ADR-0005/0006 и
//!   случайных графах 10–1000 нод).
//! - **§5.7.3 — collect-then-reduce.** Параллельные агрегаты собираются в
//!   фиксированном порядке (sort by `index` ascending); `par_iter().reduce()`
//!   ЗАПРЕЩЁН (недетерминированный порядок float-операций, архдок §5.6).
//!   Гарантия: повторные вызовы `propagate_with_lines` дают побитово
//!   идентичные `FlowSolutions` (детерминизм float-агрегатов).
//!
//! Тесты запускаются на ОБОИХ путях (однопоточный flatten + параллельный
//! `rayon` `par_iter` за `--features parallel`) — `cargo test -p canvas-core`
//! и `cargo test -p canvas-core --features parallel` оба обязаны быть
//! зелёными. Cross-path эквивалентность чисел эталонов ADR-0005/0006
//! проверяется в `crates/canvas-scene/src/tests.rs` (golden-тесты
//! `mcp_fr029_instagram_mvp_reference`, `graph_apply_assembles_mini_reference_with_oracle`,
//! `analyze_bottlenecks_reference_and_growth`) — они проходят на обеих
//! сборках (поведение `propagate_with_lines_data` идентично).

use canvas_core::{
    propagate_with_lines, topo_levels, topo_sort, Canvas, CycleError, Edge, FlowKind,
    FlowSolutions, Node, WhatIfOverrides,
};

// --- Хелперы сцены (зеркало flow.rs::tests) ---

fn node_with_expr(canvas: &mut Canvas, id: &str, formula: &str, x: f32) {
    let mut node = Node::text(id, id, x, 0.0);
    node.set_expr(Some(formula.to_owned()));
    canvas.nodes.push(node);
}

fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
    let mut edge = Edge::new(id, from, None, to, None);
    edge.set_flow_kind(FlowKind::Value);
    canvas.add_edge(edge);
}

/// Случайный DAG: `n_levels` ярусов, на каждом `width` нод, рёбра идут
/// из яруса `k` в ярус `k+1` (randomized pairs). Размер графа =
/// `n_levels × width` нод, рёбра = `width × fanout` на ярус.
fn random_dag(n_levels: usize, width: usize, seed: u64) -> Canvas {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut canvas = Canvas::default();
    let mut prev_level: Vec<String> = Vec::new();
    for level in 0..n_levels {
        let mut current: Vec<String> = Vec::new();
        for i in 0..width {
            let id = format!("L{level}_N{i}");
            let formula = if level == 0 {
                format!("{i}")
            } else {
                // Сумма 1–2 случайных входов предыдущего яруса
                let mut h = DefaultHasher::new();
                (seed, level, i).hash(&mut h);
                let pick1 = (h.finish() as usize) % width;
                let mut h2 = DefaultHasher::new();
                (seed, level, i, pick1).hash(&mut h2);
                let pick2 = (h2.finish() as usize) % width;
                if pick1 == pick2 {
                    "$in × 2".to_string()
                } else {
                    "$1 + $2".to_string()
                }
            };
            node_with_expr(&mut canvas, &id, &formula, level as f32);
            current.push(id);
        }
        // Рёбра из previous level в current
        if !prev_level.is_empty() {
            for (i, id) in current.iter().enumerate() {
                let mut h = DefaultHasher::new();
                (seed, level, i).hash(&mut h);
                let parent1 = &prev_level[(h.finish() as usize) % prev_level.len()];
                value_edge(&mut canvas, &format!("e{level}_{i}_a"), parent1, id);
                let mut h2 = DefaultHasher::new();
                (seed, level, i, 1).hash(&mut h2);
                let parent2 = &prev_level[(h2.finish() as usize) % prev_level.len()];
                if parent1 != parent2 {
                    value_edge(&mut canvas, &format!("e{level}_{i}_b"), parent2, id);
                }
            }
        }
        prev_level = current;
    }
    canvas
}

// --- §5.2: Flatten-эквивалентность topo_levels и topo_sort ---

#[test]
fn topo_levels_flatten_equals_topo_sort_empty() {
    let canvas = Canvas::default();
    let levels = topo_levels(&canvas).expect("empty DAG");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("empty DAG"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_no_edges() {
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "a", "1", 0.0);
    node_with_expr(&mut canvas, "b", "2", 1.0);
    node_with_expr(&mut canvas, "c", "3", 2.0);
    let levels = topo_levels(&canvas).expect("no edges");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("no edges"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_chain() {
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
    node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
    node_with_expr(&mut canvas, "D", "$in - 3", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    value_edge(&mut canvas, "e3", "C", "D");
    let levels = topo_levels(&canvas).expect("chain");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("chain"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_diamond() {
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
    node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
    node_with_expr(&mut canvas, "D", "$1 + $2", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "A", "C");
    value_edge(&mut canvas, "e3", "B", "D");
    value_edge(&mut canvas, "e4", "C", "D");
    let levels = topo_levels(&canvas).expect("diamond");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("diamond"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_interleaved() {
    // Граф, где topo_sort выдаёт «непоследовательный» порядок из-за
    // FIFO queue: A→B (B на ярусе 1), C изолирована (на ярусе 0).
    // topo_levels должен побитово совпасть с topo_sort (drain-фронтир).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "C", "3", 1.0);
    node_with_expr(&mut canvas, "D", "5", 2.0);
    node_with_expr(&mut canvas, "B", "$in × 2", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "C", "D");
    let levels = topo_levels(&canvas).expect("interleaved");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("interleaved"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_random_100() {
    // Случайный граф 10 ярусов × 10 нод (100 нод, ~180 рёбер) —
    // проверка на нетривиальной топологии.
    let canvas = random_dag(10, 10, 42);
    let levels = topo_levels(&canvas).expect("random 100");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("random 100"));
}

#[test]
fn topo_levels_flatten_equals_topo_sort_random_1000() {
    // Случайный граф 20 ярусов × 50 нод (1000 нод, ~1800 рёбер) —
    // проверка на тяжёлом графе (бенчмарк-класс архдок §9 M4).
    let canvas = random_dag(20, 50, 7);
    let levels = topo_levels(&canvas).expect("random 1000");
    let flat: Vec<usize> = levels.into_iter().flatten().collect();
    assert_eq!(flat, topo_sort(&canvas).expect("random 1000"));
}

#[test]
fn topo_levels_cycle_returns_error() {
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "2", 1.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "A");
    let err = topo_levels(&canvas).expect_err("цикл");
    assert_eq!(err.nodes, vec!["A".to_owned(), "B".to_owned()]);
}

#[test]
fn topo_levels_cycle_matches_topo_sort_error() {
    // §5.2: при цикле topo_levels и topo_sort возвращают ОДИНАКОВЫЙ
    // CycleError (тот же cycle_participants, тот же порядок сортировки).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "2", 1.0);
    node_with_expr(&mut canvas, "C", "3", 2.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    value_edge(&mut canvas, "e3", "C", "A");
    let err_levels = topo_levels(&canvas).expect_err("цикл levels");
    let err_sort = topo_sort(&canvas).expect_err("цикл sort");
    assert_eq!(err_levels, err_sort);
}

#[test]
fn topo_levels_levels_are_independent() {
    // Контракт фронтира Кана: внутри одного яруса нет value-рёбер между
    // узлами этого яруса (иначе нарушается независимость для параллельного
    // обхода). Проверяем инвариант на ромбе: ярус 0 = [A], ярус 1 = [B, C]
    // (независимы), ярус 2 = [D].
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
    node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
    node_with_expr(&mut canvas, "D", "$1 + $2", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "A", "C");
    value_edge(&mut canvas, "e3", "B", "D");
    value_edge(&mut canvas, "e4", "C", "D");
    let levels = topo_levels(&canvas).expect("DAG");
    // 3 яруса: [A], [B, C], [D]
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0], vec![0]); // A
                                    // B и C — на одном ярусе (порядок — FIFO queue Кана)
    let level1_set: std::collections::HashSet<usize> = levels[1].iter().copied().collect();
    assert_eq!(level1_set, [1, 2].into_iter().collect());
    assert_eq!(levels[2], vec![3]); // D
                                    // Инвариант: внутри яруса нет value-рёбер между узлами этого яруса
    let n = canvas.nodes.len();
    let mut index_of: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (i, node) in canvas.nodes.iter().enumerate() {
        index_of.insert(node.id.as_str(), i);
    }
    for level in &levels {
        let level_set: std::collections::HashSet<usize> = level.iter().copied().collect();
        for edge in &canvas.edges {
            if edge.flow_kind() != FlowKind::Value {
                continue;
            }
            let (Some(&from), Some(&to)) = (
                index_of.get(edge.from_node.as_str()),
                index_of.get(edge.to_node.as_str()),
            ) else {
                continue;
            };
            // Ни одно value-ребро не должно иметь ОБА конца в одном ярусе
            assert!(
                !(level_set.contains(&from) && level_set.contains(&to)),
                "value-ребро {}→{} внутри одного яруса — нарушен контракт Кана",
                edge.from_node,
                edge.to_node
            );
        }
    }
    let _ = n;
}

// --- §5.7.3: Детерминизм параллельного пересчёта ---

#[test]
fn propagate_deterministic_chain_repeated() {
    // Повторные вызовы propagate_with_lines дают побитово идентичные
    // FlowSolutions (детерминизм float-агрегатов, контракт §5.7.3).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "0.1", 0.0);
    node_with_expr(&mut canvas, "B", "$in + 0.2", 1.0);
    node_with_expr(&mut canvas, "C", "$in × 1.5", 2.0);
    node_with_expr(&mut canvas, "D", "$in - 0.7", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    value_edge(&mut canvas, "e3", "C", "D");
    let whatif = WhatIfOverrides::default();
    let first = propagate_with_lines(&canvas, &whatif).expect("DAG");
    for _ in 0..10 {
        let next = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq_solutions(&first, &next);
    }
}

#[test]
fn propagate_deterministic_diamond_repeated() {
    // Ромб: A→B, A→C, B→D, C→D — ярус [B, C] вычисляется параллельно;
    // D агрегирует ($1 + $2). Сортировка float-операций детерминирована
    // (sort by index ascending перед merge — контракт §5.7.3).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "100", 0.0);
    node_with_expr(&mut canvas, "B", "$in × 0.5", 1.0);
    node_with_expr(&mut canvas, "C", "$in × 0.3", 2.0);
    node_with_expr(&mut canvas, "D", "$1 + $2", 3.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "A", "C");
    value_edge(&mut canvas, "e3", "B", "D");
    value_edge(&mut canvas, "e4", "C", "D");
    let whatif = WhatIfOverrides::default();
    let first = propagate_with_lines(&canvas, &whatif).expect("DAG");
    for _ in 0..10 {
        let next = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq_solutions(&first, &next);
    }
}

#[test]
fn propagate_deterministic_wide_level_repeated() {
    // Широкий ярус: A → [B1, B2, B3, B4, B5] → C (агрегация $1 + $2 + $3 + $4 + $5).
    // Параллельный путь должен дать побитово идентичный результат при
    // повторных вызовах (контракт §5.7.3 — collect → sort by index → merge).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    for i in 1..=5 {
        let formula = format!("$in × {i}.0");
        node_with_expr(&mut canvas, &format!("B{i}"), &formula, i as f32);
        value_edge(&mut canvas, &format!("e_b{i}"), "A", &format!("B{i}"));
    }
    // C: $1 + $2 + $3 + $4 + $5 — порядок float-операций важен
    node_with_expr(&mut canvas, "C", "$1 + $2 + $3 + $4 + $5", 10.0);
    for i in 1..=5 {
        value_edge(&mut canvas, &format!("e_c{i}"), &format!("B{i}"), "C");
    }
    let whatif = WhatIfOverrides::default();
    let first = propagate_with_lines(&canvas, &whatif).expect("DAG");
    for _ in 0..20 {
        let next = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq_solutions(&first, &next);
    }
}

#[test]
fn propagate_deterministic_random_1000_repeated() {
    // Тяжёлый граф (1000 нод, ~1800 рёбер) — детерминизм на масштабе
    // бенчмарка архдока §9 M4. Параллельный путь (rayon par_iter, bounded
    // thread pool) обязан давать побитово идентичные результаты при
    // повторных вызовах (контракт §5.7.3 — sort by index ascending).
    let canvas = random_dag(20, 50, 13);
    let whatif = WhatIfOverrides::default();
    let first = propagate_with_lines(&canvas, &whatif).expect("DAG");
    for _ in 0..5 {
        let next = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq_solutions(&first, &next);
    }
}

#[test]
fn propagate_with_override_deterministic() {
    // WhatIf override (value-level подмена) — детерминирован на обоих путях.
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "100", 0.0);
    node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
    node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    let mut whatif = WhatIfOverrides::default();
    whatif
        .node_values
        .insert("A".to_owned(), canvas_core::Value::scalar(50.0));
    let first = propagate_with_lines(&canvas, &whatif).expect("DAG");
    for _ in 0..10 {
        let next = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq_solutions(&first, &next);
    }
    // Sanity: A=50, B=100, C=101
    let a = first
        .outputs
        .get("A")
        .and_then(|r| r.as_ref().ok())
        .map(|v| v.num);
    let b = first
        .outputs
        .get("B")
        .and_then(|r| r.as_ref().ok())
        .map(|v| v.num);
    let c = first
        .outputs
        .get("C")
        .and_then(|r| r.as_ref().ok())
        .map(|v| v.num);
    assert_eq!(a, Some(50.0));
    assert_eq!(b, Some(100.0));
    assert_eq!(c, Some(101.0));
}

// --- Хелперы сравнения ---

/// Побитовое сравнение FlowSolutions (HashMap — порядок не важен,
/// PartialEq для Value f64 — побитовый, не tolerance-based).
fn assert_eq_solutions(a: &FlowSolutions, b: &FlowSolutions) {
    assert_eq!(a.outputs, b.outputs, "outputs drift — недетерминизм");
    assert_eq!(a.lines, b.lines, "lines drift — недетерминизм");
    assert_eq!(a.named, b.named, "named drift — недетерминизм");
    assert_eq!(a.warnings, b.warnings, "warnings drift — недетерминизм");
}

// --- Утилиты для тестов (заглушки ниже не нужны — flow самодостаточен) ---

#[test]
fn topo_levels_and_sort_handle_self_loop() {
    // Петля A→A — CycleError с участником ["A"] (поведение идентично topo_sort).
    let mut canvas = Canvas::default();
    node_with_expr(&mut canvas, "A", "1", 0.0);
    node_with_expr(&mut canvas, "B", "2", 1.0);
    value_edge(&mut canvas, "e1", "A", "A");
    value_edge(&mut canvas, "e2", "A", "B");
    let err_levels: CycleError = topo_levels(&canvas).expect_err("петля");
    let err_sort: CycleError = topo_sort(&canvas).expect_err("петля");
    assert_eq!(err_levels, err_sort);
    assert_eq!(err_levels.nodes, vec!["A".to_owned()]);
}
