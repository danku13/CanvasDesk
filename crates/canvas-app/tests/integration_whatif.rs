//! Интеграционные тесты FR-017 (CP6): what-if сценарии.
//!
//! Сценарий владельца: 3 calc-ноды A→B→C (value-рёбра); подмена строки A
//! пересчитывает каскад; подмены runtime-only — `.canvas` без Apply не
//! мутируется (инвариант 2); сценарии персистентны в `canvasdesk.whatif`.
//!
//! Инварианты FR-017 на уровне модели:
//! 1. каскад A→B→C: подмена строки 0 ноды A → outputs {A: 20, B: 40, C: 41}
//!    (эталон «Проверка» FR-017: {A: 5→20, B: 10→40, C: 11→41});
//! 2. хеш `.canvas` до/после what-if сессии без Apply — идентичен;
//! 3. `Scenario` round-trip через `canvas.extra` (`canvasdesk.whatif`),
//!    пустой список не оставляет поля;
//! 4. `validate_scenario` — протухшие подмены (удалённая нода, строка вне
//!    текста, проза) помечены; валидные работают.
//!
//! MCP-сценарии (whatif_set_override/apply/activate/undo) — в `main.rs`
//! mod tests (mcp_dispatch приватен для бинарного крейта, паттерн FR-014).

use std::collections::HashMap;

use canvas_app::{Canvas, Edge, Node};
use canvas_core::expr::Value;
use canvas_core::flow::{self, WhatIfOverrides};
use canvas_core::whatif::{self, Scenario};

/// Заметка-calc: текст — Numi-лист (построчные формулы видят `$in`).
fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
    canvas.nodes.push(Node::text(id, id, x, 0.0));
    canvas.nodes.last_mut().expect("нода").text = Some(text.to_owned());
}

/// Value-ребро (`canvasdesk.flow.kind = "value"`).
fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
    let mut edge = Edge::new(id, from, None, to, None);
    edge.set_flow_kind(flow::FlowKind::Value);
    canvas.add_edge(edge);
}

/// Ландшафт из эталона «Проверка»: A (`a = 5`) → B (`b = $in × 2`) →
/// C (`c = $in + 1`) по value-рёбрам.
fn abc_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "a = 5", 0.0);
    calc_node(&mut canvas, "B", "b = $in × 2", 300.0);
    calc_node(&mut canvas, "C", "c = $in + 1", 600.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    canvas
}

/// Инвариант 1 (FR-017 «Проверка»): подмена строки 0 ноды A (`a = 20`)
/// пересчитывает весь каскад — {A: 5→20, B: 10→40, C: 11→41}. База
/// (`WhatIfOverrides::default`) остаётся {A: 5, B: 10, C: 11}.
#[test]
fn override_recalculates_downstream_cascade() {
    let canvas = abc_canvas();
    // База — прежняя семантика propagate
    let base = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert_eq!(base["A"].as_ref().unwrap(), &Value::scalar(5.0));
    assert_eq!(base["B"].as_ref().unwrap(), &Value::scalar(10.0));
    assert_eq!(base["C"].as_ref().unwrap(), &Value::scalar(11.0));

    // What-if: подмена строки 0 ноды A
    let mut line_exprs = HashMap::new();
    line_exprs.insert(("A".to_owned(), 0usize), "a = 20".to_owned());
    let whatif = WhatIfOverrides {
        line_exprs,
        ..WhatIfOverrides::default()
    };
    let solutions = flow::propagate_with_lines(&canvas, &whatif).expect("DAG");
    assert_eq!(
        solutions.outputs["A"].as_ref().unwrap(),
        &Value::scalar(20.0),
        "A: 5 → 20"
    );
    assert_eq!(
        solutions.outputs["B"].as_ref().unwrap(),
        &Value::scalar(40.0),
        "B: 10 → 40"
    );
    assert_eq!(
        solutions.outputs["C"].as_ref().unwrap(),
        &Value::scalar(41.0),
        "C: 11 → 41"
    );
    // Построчные выходы виртуального исходника — строка 0 ноды A
    assert_eq!(
        solutions.lines.get(&("A".to_owned(), 0)).unwrap(),
        &Value::scalar(20.0),
        "построчный выход подменённой строки — новое значение"
    );
}

/// Инвариант 2 (FR-017): what-if сессия без Apply не мутирует `.canvas` —
/// сериализованный JSON байт-в-байт идентичен до и после подмен.
#[test]
fn session_without_apply_leaves_canvas_bytes_identical() {
    let canvas = abc_canvas();
    let hash_before = canvas.to_json().expect("сериализация");

    // Сессия: подмены + полный пересчёт (propagator чистый — &Canvas)
    let mut line_exprs = HashMap::new();
    line_exprs.insert(("A".to_owned(), 0usize), "a = 20".to_owned());
    let whatif = WhatIfOverrides {
        line_exprs,
        ..WhatIfOverrides::default()
    };
    let solutions = flow::propagate_with_lines(&canvas, &whatif).expect("DAG");
    assert_eq!(
        solutions.outputs["C"].as_ref().unwrap(),
        &Value::scalar(41.0),
        "пересчёт по подмене"
    );

    let hash_after = canvas.to_json().expect("сериализация");
    assert_eq!(
        hash_before, hash_after,
        "подмены runtime-only — `.canvas` не изменился (инвариант 2)"
    );
}

/// Инвариант 3 (FR-017): `Scenario` round-trip через `canvas.extra`
/// (`canvasdesk.whatif`); пустой список УДАЛЯЕТ поле — файл без сценариев
/// байт-в-байт как раньше.
#[test]
fn scenario_round_trip_through_canvas_extra() {
    let mut canvas = abc_canvas();
    let pristine = canvas.to_json().expect("сериализация");

    // Без сценариев — поля нет
    assert!(whatif::scenarios_from_canvas(&canvas).is_empty());
    assert_eq!(canvas.to_json().expect("сериализация"), pristine);

    // Пишем два сценария — переживают round-trip
    let mut s1 = Scenario {
        name: "Рост ×2".to_owned(),
        ..Scenario::default()
    };
    s1.line_exprs
        .insert(("A".to_owned(), 0usize), "a = 20".to_owned());
    let s2 = Scenario {
        name: "Отказ реплики".to_owned(),
        ..Scenario::default()
    };
    whatif::scenarios_to_canvas(&mut canvas, &[s1.clone(), s2.clone()]);
    assert_eq!(
        whatif::scenarios_from_canvas(&canvas),
        vec![s1, s2],
        "сценарии восстановлены из extra"
    );

    // Очистка — поле удаляется, JSON идентичен исходному
    whatif::scenarios_to_canvas(&mut canvas, &[]);
    assert!(whatif::scenarios_from_canvas(&canvas).is_empty());
    assert_eq!(
        canvas.to_json().expect("сериализация"),
        pristine,
        "пустой список сценариев не оставляет следов в файле"
    );
}

/// Инвариант 4 (FR-017, Q5c): `validate_scenario` помечает протухшие
/// подмены (нода удалена / строка вне текста / строка стала прозой);
/// валидные подмены не возвращаются и работают в пересчёте.
#[test]
fn validate_scenario_marks_stale_overrides() {
    let mut canvas = abc_canvas();
    // Прозаическая нода: подмена её строки — протухшая (значений нет)
    calc_node(&mut canvas, "D", "обычная заметка", 900.0);
    let scenario = Scenario {
        name: "S".to_owned(),
        line_exprs: HashMap::from([
            (("A".to_owned(), 0usize), "a = 20".to_owned()),
            (("A".to_owned(), 5usize), "a = 99".to_owned()),
            (("ghost".to_owned(), 0usize), "x = 1".to_owned()),
            (("D".to_owned(), 0usize), "d = 1".to_owned()),
        ]),
    };
    let stale = whatif::validate_scenario(&canvas, &scenario);
    assert_eq!(stale.len(), 3, "три протухших подмены");
    assert!(
        stale
            .iter()
            .any(|s| s.node == "ghost" && s.reason.contains("удалена")),
        "нода удалена: {stale:?}"
    );
    assert!(
        stale
            .iter()
            .any(|s| s.node == "A" && s.line == 5 && s.reason.contains("удалена")),
        "строка удалена: {stale:?}"
    );
    assert!(
        stale
            .iter()
            .any(|s| s.node == "D" && s.reason.contains("прозой")),
        "строка стала прозой: {stale:?}"
    );

    // Активные подмены — только валидная; протухшие тихо пропускаются
    let active = whatif::active_line_exprs(&canvas, &scenario);
    assert_eq!(active.len(), 1);
    assert_eq!(
        active.get(&("A".to_owned(), 0usize)).map(String::as_str),
        Some("a = 20")
    );
    let solutions = flow::propagate_with_lines(
        &canvas,
        &WhatIfOverrides {
            line_exprs: active,
            ..WhatIfOverrides::default()
        },
    )
    .expect("DAG");
    assert_eq!(
        solutions.outputs["B"].as_ref().unwrap(),
        &Value::scalar(40.0),
        "валидная подмена работает, протухшие не мешают"
    );
}
