//! Интеграционные тесты FR-014: поток значений по рёбрам — DAG-движок.
//!
//! Сценарий владельца: несколько calc-нод связаны value-рёбрами; формула
//! downstream ссылается на значение upstream через `$in` / `$1..$N`;
//! правка upstream живьём пересчитывает downstream (инвариант live).
//!
//! Инварианты FR-014 на уровне библиотеки:
//! 1. `topo_sort` — чистая функция над `Canvas` (DAG-инвариант, CycleError
//!    с участниками);
//! 2. `propagate` — чистая функция (цепочка, слияние, missing inbound);
//! 3. round-trip `canvasdesk.flow.kind` через `extra` (`.canvas` хранит
//!    только формулы и топологию — результаты не сериализуются, инвариант 4);
//! 4. юнит-покрытие движка — `canvas-core/src/flow.rs`; сценарии с
//!    MCP-диспетчером (flow_set_kind/flow_recalc/flow_cycle_check + undo) —
//!    в `main.rs` mod tests (mcp_dispatch приватен для бинарного крейта).

use std::collections::HashMap;
use std::str::FromStr;

use canvas_core::expr::Value;
use canvas_core::flow::{self, FlowKind};
use canvas_core::{Canvas, Edge, Node};

/// Нода с формулой (`canvasdesk.expr` — как после UI `=`-строк или MCP).
fn calc_node(canvas: &mut Canvas, id: &str, formula: &str, x: f32) {
    let mut node = Node::text(id, id, x, 0.0);
    node.set_expr(Some(formula.to_owned()));
    canvas.nodes.push(node);
}

/// Value-ребро (`canvasdesk.flow.kind = "value"`).
fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
    let mut edge = Edge::new(id, from, None, to, None);
    edge.set_flow_kind(FlowKind::Value);
    canvas.add_edge(edge);
}

/// Верификация FR-014 (ручная приёмка, уровень модели): 3 calc-ноды,
/// формулы `A=5`, `B=$in × 2`, `C=$in + 1`; value-рёбра A→B→C —
/// propagate даёт {A: 5, B: 10, C: 11}.
#[test]
fn service_chain_propagates_values() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "5", 0.0);
    calc_node(&mut canvas, "B", "$in × 2", 300.0);
    calc_node(&mut canvas, "C", "$in + 1", 600.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");

    let outputs = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(5.0));
    assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(10.0));
    assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(11.0));
}

/// Live-ревал (инвариант FR-014): правка формулы A (7) — downstream
/// пересчитан в пределах того же вызова propagator'а ({A: 7, B: 14, C: 15});
/// модель (`Canvas`) — единственный источник, результаты runtime-only.
#[test]
fn formula_edit_revalues_downstream() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "5", 0.0);
    calc_node(&mut canvas, "B", "$in × 2", 300.0);
    calc_node(&mut canvas, "C", "$in + 1", 600.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");

    // Правка формулы A: set_expr + propagate (то же, что делает приложение)
    let index = canvas.nodes.iter().position(|n| n.id == "A").expect("A");
    canvas.nodes[index].set_expr(Some("7".to_owned()));
    let outputs = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(14.0));
    assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(15.0));

    // Удаление ребра A→B — downstream «вход отсутствует», не падение
    canvas.remove_edge("e1");
    let outputs = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert!(outputs["B"].is_err(), "вход отсутствует");
    assert!(outputs["C"].is_err(), "каскад по downstream");
}

/// Слияние потоков (diamond с SUM): $1/$2 по порядку входящих value-рёбер.
#[test]
fn merged_flows_indexed_by_edge_order() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "in1", "1000 rps", 0.0);
    calc_node(&mut canvas, "in2", "250 rps", 0.0);
    calc_node(&mut canvas, "sum", "$1 + $2", 600.0);
    // Порядок рёбер задаёт нумерацию: e1 → $1 (in1), e2 → $2 (in2)
    value_edge(&mut canvas, "e1", "in1", "sum");
    value_edge(&mut canvas, "e2", "in2", "sum");
    let outputs = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert_eq!(outputs["sum"].as_ref().unwrap().to_string(), "1250 rps");

    // in2 без формулы (значения нет) — слот Some(None): ошибка входа,
    // индексация $1/$2 не схлопывается
    let index = canvas.nodes.iter().position(|n| n.id == "in2").unwrap();
    canvas.nodes[index].set_expr(None);
    let outputs = flow::propagate(&canvas, &HashMap::new()).expect("DAG");
    assert!(outputs["sum"].is_err(), "второй вход отсутствует");
}

/// DAG-инвариант сквозной: value-цикл — Err(CycleError) с участниками;
/// проверка через MCP-семантику flow_cycle_check (topo_sort).
#[test]
fn value_cycle_is_rejected_with_participants() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "1", 0.0);
    calc_node(&mut canvas, "B", "2", 300.0);
    calc_node(&mut canvas, "C", "3", 600.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    // UI: попытка замкнуть C→A — предикат блокирует создание value-ребра
    assert!(flow::creates_value_cycle(&canvas, "C", "A"));
    // MCP flow_set_kind тоже отвергает; чужой файл мог бы содержать цикл —
    // propagator возвращает ту же ошибку с участниками
    value_edge(&mut canvas, "e3", "C", "A");
    let err = flow::propagate(&canvas, &HashMap::new()).expect_err("цикл");
    assert_eq!(
        err.nodes,
        vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]
    );
    // flow_cycle_check: участники — по id, отсортированы
    let err = flow::topo_sort(&canvas).expect_err("цикл");
    assert_eq!(
        err.nodes,
        vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]
    );
}

/// Инвариант 4 (сквозной): `.canvas` хранит формулы и `canvasdesk.flow.kind`,
/// но НЕ результаты — после round-trip propagator восстанавливает значения
/// из формул + топологии.
#[test]
fn round_trip_keeps_topology_restores_values() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "1000 rps", 0.0);
    calc_node(&mut canvas, "B", "$in / 4", 300.0);
    value_edge(&mut canvas, "e1", "A", "B");

    let json = canvas.to_json().expect("сериализация");
    assert!(
        !json.contains("250"),
        "результат propagate не сериализуется: {json}"
    );
    let restored = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(
        restored.edges[0].flow_kind(),
        FlowKind::Value,
        "тип потока пережил round-trip"
    );
    let outputs = flow::propagate(&restored, &HashMap::new()).expect("DAG");
    assert_eq!(
        outputs["B"].as_ref().unwrap().to_string(),
        "250 rps",
        "значения восстановлены из формул и топологии"
    );
}

/// What-if (FR-017, точка расширения propagator'а): override значения
/// ноды меняет её и downstream без правки формул.
#[test]
fn overrides_flow_downstream_what_if() {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "1000 rps", 0.0);
    calc_node(&mut canvas, "B", "$in / 4", 300.0);
    value_edge(&mut canvas, "e1", "A", "B");
    // Override 800 rps: единица — та же размерность, что в формуле A
    let rps = canvas_core::expr::eval(
        &canvas_core::expr::parse("1 rps").unwrap(),
        &canvas_core::expr::Env::empty(),
    )
    .unwrap()
    .unit;
    let mut overrides = HashMap::new();
    overrides.insert("A".to_owned(), Value::with_unit(800.0, rps));
    let outputs = flow::propagate(&canvas, &overrides).expect("DAG");
    assert_eq!(
        outputs["B"].as_ref().unwrap().to_string(),
        "200 rps",
        "what-if: 800 rps / 4"
    );
}
