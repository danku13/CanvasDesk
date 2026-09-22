//! Интеграционный тест PRD-0007 X3 (F-6, AC-4.1–AC-4.3): what-if из
//! explain-дерева на уровне модели.
//!
//! Сценарий: подмена листа (то же, что делает `commit_explain_edit` из
//! окна проверки) → живой пересчёт сцены (`recompute_flow`) → what-if
//! дерево из `flow_active` → дельты против `flow_baseline`
//! (`lineage_deltas`, формат FR-017). Инварианты:
//! 1. дельта на корне и на затронутых узлах («было → стало (+Δ)»);
//! 2. базовая модель не мутируется (`.canvas` без Apply, инвариант 2
//!    FR-017);
//! 3. сценарий персистентен — round-trip через `canvasdesk.whatif`
//!    (AC-4.3: переживает перезагрузку);
//! 4. возврат к базе одним действием (`whatif_activate(None)`) — дельт
//!    больше нет.

use std::path::PathBuf;

use canvas_app::{Canvas, Edge, Node};
use canvas_core::flow::FlowKind;
use canvas_core::{
    build_lineage, lineage_deltas, DataSnapshots, LineageFlow, LineageNodeId, LineageTree,
};
use canvas_scene::SceneState;

/// Заметка-calc: текст — Numi-лист (построчные формулы видят `$in`).
fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
    canvas.nodes.push(Node::text(id, id, x, 0.0));
    canvas.nodes.last_mut().expect("нода").text = Some(text.to_owned());
}

/// Value-ребро (`canvasdesk.flow.kind = "value"`).
fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
    let mut edge = Edge::new(id, from, None, to, None);
    edge.set_flow_kind(FlowKind::Value);
    canvas.add_edge(edge);
}

/// Ландшафт: A (`a = 5`) → B (`b = $in × 2`) → C (`c = $in + 1`).
fn abc_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "A", "a = 5", 0.0);
    calc_node(&mut canvas, "B", "b = $in × 2", 300.0);
    calc_node(&mut canvas, "C", "c = $in + 1", 600.0);
    value_edge(&mut canvas, "e1", "A", "B");
    value_edge(&mut canvas, "e2", "B", "C");
    canvas
}

/// Дерево происхождения итога `root` по активным решениям сцены.
fn tree_of(scene: &SceneState, root: LineageNodeId) -> LineageTree {
    let data = DataSnapshots::new();
    build_lineage(
        &scene.canvas,
        LineageFlow::Ready {
            solutions: &scene.flow_active,
            data: &data,
        },
        root,
    )
    .expect("дерево построено")
}

/// AC-4.1 + AC-4.2: подмена листа из дерева — живой пересчёт и дельты.
#[test]
fn explain_edit_produces_deltas_and_keeps_base() {
    let scene = SceneState::new(abc_canvas(), PathBuf::from("target/tmp/x3-explain.canvas"));
    let base_tree = tree_of(&scene, LineageNodeId::total("C"));
    let snapshot_canvas = scene.canvas.clone();

    // Действие «Изменить» из explain-окна: подмена листа (A, строка 0)
    // через рантайм FR-017 — сценарий автосоздаётся, база не мутируется.
    let mut scene = scene;
    scene.whatif_active = true;
    let index = scene.whatif_create_scenario("").expect("сценарий создан");
    scene.active_scenario = Some(index);
    canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);
    if let Some(scenario) = scene.scenarios.get_mut(index) {
        scenario
            .line_exprs
            .insert(("A".to_owned(), 0usize), "a = 7".to_owned());
    }
    scene.recompute_flow();

    // Живая модель: пересчитанные значения на канвасе (полоса D).
    assert_eq!(
        scene.flow_active.outputs["C"].as_ref().unwrap().to_string(),
        "15",
        "C: 11 → 15 (живая модель)"
    );
    // База не мутировалась (инвариант 2 FR-017).
    assert_eq!(scene.canvas.nodes, snapshot_canvas.nodes, "база цела");

    // What-if дерево — из flow_active; дельты — против flow_baseline.
    let whatif_tree = tree_of(&scene, LineageNodeId::total("C"));
    let deltas = lineage_deltas(&base_tree, &whatif_tree);
    let root_delta = deltas
        .get(&("C".to_owned(), None))
        .expect("дельта итога корня");
    assert_eq!(root_delta.base.to_string(), "11");
    assert_eq!(root_delta.whatif.to_string(), "15");
    assert_eq!(root_delta.delta, "+4", "формат дельты FR-017");
    assert!(
        deltas.contains_key(&("A".to_owned(), Some(0))),
        "лист подмены тоже с дельтой: {deltas:?}"
    );
}

/// AC-4.3: сценарий из explain-подмены переживает перезагрузку
/// (round-trip через `canvasdesk.whatif`), подмена листа-константы
/// («5» без «=») не протухает.
#[test]
fn explain_scenario_survives_reload() {
    let mut canvas = Canvas::default();
    // Лист-константа БЕЗ «=»: строка 0 «5» — как в дереве explain.
    calc_node(&mut canvas, "A", "5", 0.0);
    calc_node(&mut canvas, "B", "b = $in × 2", 300.0);
    value_edge(&mut canvas, "e1", "A", "B");

    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/x3-reload.canvas"));
    scene.whatif_active = true;
    let index = scene.whatif_create_scenario("NPL 90+").expect("сценарий");
    scene.active_scenario = Some(index);
    if let Some(scenario) = scene.scenarios.get_mut(index) {
        scenario
            .line_exprs
            .insert(("A".to_owned(), 0usize), "7".to_owned());
    }
    canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);

    // Round-trip: serialize → deserialize (перезагрузка файла).
    let json = serde_json::to_string(&scene.canvas).expect("сериализация");
    let restored: Canvas = serde_json::from_str(&json).expect("десериализация");
    let scenarios = canvas_core::whatif::scenarios_from_canvas(&restored);
    assert_eq!(scenarios.len(), 1, "сценарий восстановлен");
    assert_eq!(scenarios[0].name, "NPL 90+");
    assert!(scenarios[0]
        .line_exprs
        .contains_key(&("A".to_owned(), 0usize)));
    // Подмена листа-константы «5» (без «=») — живая (validate X3).
    let stale = canvas_core::whatif::validate_scenario(&restored, &scenarios[0]);
    assert!(stale.is_empty(), "константа не протухает: {stale:?}");

    // И значения сходятся: пересчёт с восстановленными подменами.
    let mut scene2 = SceneState::new(restored, PathBuf::from("target/tmp/x3-reload2.canvas"));
    scene2.whatif_active = true;
    scene2.scenarios = scenarios;
    scene2.active_scenario = Some(0);
    scene2.recompute_flow();
    assert_eq!(
        scene2.flow_active.outputs["B"]
            .as_ref()
            .unwrap()
            .to_string(),
        "14",
        "B: 10 → 14 после перезагрузки"
    );
}

/// AC-4.3 (возврат к базе одним действием): whatif_activate(None) —
/// значения вернулись к базе, дельт против нового базового дерева нет.
#[test]
fn back_to_base_clears_deltas() {
    let mut scene = SceneState::new(
        abc_canvas(),
        PathBuf::from("target/tmp/x3-back-to-base.canvas"),
    );
    let base_tree = tree_of(&scene, LineageNodeId::total("C"));

    // Включаем what-if (как из explain-окна).
    scene.whatif_active = true;
    let index = scene.whatif_create_scenario("").expect("сценарий создан");
    scene.active_scenario = Some(index);
    if let Some(scenario) = scene.scenarios.get_mut(index) {
        scenario
            .line_exprs
            .insert(("A".to_owned(), 0usize), "a = 7".to_owned());
    }
    scene.recompute_flow();
    let whatif_tree = tree_of(&scene, LineageNodeId::total("C"));
    assert!(!lineage_deltas(&base_tree, &whatif_tree).is_empty());

    // Возврат к базе одним действием (пилюля «База» бара FR-017).
    scene.whatif_activate(None);
    assert_eq!(
        scene.flow_active.outputs["C"].as_ref().unwrap().to_string(),
        "11",
        "значение вернулось к базе"
    );
    let back_to_base = tree_of(&scene, LineageNodeId::total("C"));
    assert!(
        lineage_deltas(&base_tree, &back_to_base).is_empty(),
        "дельт после возврата нет"
    );
}
