//! Интеграционные тесты PRD-0007 X6 (закрытие PoC, FR-048):
//!
//! 1. G7 — объяснение уходит/возвращается без следов: сборка дерева
//!    (Ready/чтение) не двигает модель и ревизию; чип AC-3.3 по ревизии;
//!    после undo модель и дерево совпадают с исходными (F-5).
//! 2. F-9/MCP-паритет — `explain_number` текстом следует за активным
//!    what-if (преамбула + новые значения) и возвращает базу без следов.
//! 3. F-12/G2 — индикатор покрытия «Цепочки: N%» на эталоне владельца
//!    (5 нод-таблиц юнит-экономики): 100% на связанной модели, падение
//!    после разрыва поставки параметра.
//! 4. G4 — автосвязь на эталоне «5 таблиц»: precision = 1.0 (каждое
//!    предложение — настоящая связь) ≥ 0.9 и recall = 1.0 ≥ 0.8.

use std::path::PathBuf;

use canvas_app::{Canvas, Edge, Node};
use canvas_core::flow::FlowKind;
use canvas_core::templates::TemplateRegistry;
use canvas_core::{
    build_lineage, chain_coverage, find_proposals, DataSnapshots, LineageFlow, LineageNodeId,
    LineageTree,
};
use canvas_scene::{mcp_dispatch, SceneState};

/// Заметка-calc: текст — Numi-лист.
fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
    canvas.nodes.push(Node::text(id, id, x, 0.0));
    canvas.nodes.last_mut().expect("нода").text = Some(text.to_owned());
}

/// Проливание из именованной переменной истока в параметр приёмника
/// (то же, что создаёт undo-бат автосвязи X4: fromOutput = toParam = имя).
fn spill_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str, param: &str) {
    let mut edge = Edge::new(id, from, None, to, None);
    edge.set_flow_kind(FlowKind::Value);
    edge.from_output = Some(param.to_owned());
    edge.to_param = Some(param.to_owned());
    canvas.add_edge(edge);
}

/// Эталон владельца (PRD-0007 §2, §15): юнит-экономика «Аренда устройства»
/// — 5 нод-таблиц: Трафик, Монетизация, Себестоимость, Риски и расчётная
/// Юнит-экономика (переменные листа + параметры от таблиц).
fn demo_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    calc_node(&mut canvas, "traffic", "Трафик\nusers = 1000", 0.0);
    calc_node(&mut canvas, "monet", "Монетизация\narpu = 810", 300.0);
    calc_node(
        &mut canvas,
        "cost",
        "Себестоимость\nrent = 620\nother = -190",
        600.0,
    );
    calc_node(&mut canvas, "risk", "Риски\nnpl = 0.12", 900.0);
    calc_node(
        &mut canvas,
        "unit",
        "Юнит-экономика\nrevenue = $users × $arpu\ncost = $rent + $other\nrisk = revenue × $npl\nprofit = revenue - cost - risk",
        1200.0,
    );
    canvas
}

/// Связать таблицы демо-модели (как undo-бат автосвязи после ревью).
fn link_demo_tables(canvas: &mut Canvas) {
    spill_edge(canvas, "e-users", "traffic", "unit", "users");
    spill_edge(canvas, "e-arpu", "monet", "unit", "arpu");
    spill_edge(canvas, "e-rent", "cost", "unit", "rent");
    spill_edge(canvas, "e-other", "cost", "unit", "other");
    spill_edge(canvas, "e-npl", "risk", "unit", "npl");
}

fn scene_of(canvas: Canvas, name: &str) -> SceneState {
    SceneState::new(
        canvas,
        PathBuf::from(format!("target/tmp/x6-{name}.canvas")),
    )
}

fn dispatch(
    scene: &mut SceneState,
    method: &str,
    params: &str,
) -> Result<serde_json::Value, String> {
    let params: serde_json::Value = serde_json::from_str(params).expect("params — JSON");
    let registry = TemplateRegistry::builtin();
    mcp_dispatch(scene, &registry, method, &params)
}

/// Дерево происхождения итога `root` по активным решениям сцены.
fn tree_of(scene: &SceneState, root: LineageNodeId) -> LineageTree {
    let data = DataSnapshots::new();
    build_lineage(
        &scene.canvas,
        LineageFlow::Ready {
            solutions: &canvas_scene::read_flow(&scene.flow_active),
            data: &data,
        },
        root,
    )
    .expect("дерево построено")
}

/// G7 (F-10/F-5): объяснение уходит/возвращается без следов — чтение
/// (панель + подсветка = сборка дерева) не меняет модель и ревизию;
/// чип «Данные изменены» — по ревизии; после undo модель и дерево
/// возвращаются к исходным.
#[test]
fn explain_panel_lifecycle_leaves_model_untouched() {
    let mut scene = scene_of(demo_canvas(), "lifecycle");
    link_demo_tables(&mut scene.canvas);
    scene.recompute_flow();

    let before_canvas = scene.canvas.clone();
    let before_rev = scene.revision;

    // Открытие панели (X2 open_explain): сборка дерева итога profit —
    // чистое чтение: канвас байт-в-байт, ревизия не сдвинулась (AC-3.3).
    let tree_base = tree_of(&scene, LineageNodeId::total("unit"));
    assert_eq!(scene.canvas, before_canvas, "сборка дерева не мутирует");
    assert_eq!(scene.revision, before_rev, "чтение не двигает ревизию");
    assert_eq!(
        tree_base.nodes[0]
            .value
            .as_ref()
            .and_then(|v| v.as_ref().ok()),
        canvas_scene::read_flow(&scene.flow_active).outputs["unit"]
            .as_ref()
            .ok(),
        "корень дерева — то же значение, что полоса D"
    );

    // Возмущение модели (правка текста листа как пользователь/канвас) —
    // ревизия растёт: чип «Данные изменены» появляется (AC-3.3), снапшот
    // панели НЕ перестраивается сам.
    let snapshot_rev = before_rev;
    let index = scene
        .canvas
        .nodes
        .iter()
        .position(|node| node.id == "traffic")
        .expect("нода Трафик");
    let new_text = "Трафик\nusers = 1200".to_owned();
    let expr = canvas_scene::split_formula_lines(&new_text);
    scene.canvas.nodes[index].text = Some(new_text);
    scene.canvas.nodes[index].set_expr(expr);
    scene.recompute_flow();
    assert!(scene.revision > snapshot_rev, "ревизия выросла");

    // Клик по чипу: перестройка из нового снапшота — новое значение в
    // корне (profit = 1200 × 810 − cost − risk).
    let tree_after = tree_of(&scene, LineageNodeId::total("unit"));
    let base = tree_base.nodes[0]
        .value
        .as_ref()
        .and_then(|v| v.as_ref().ok())
        .expect("значение базы");
    let after = tree_after.nodes[0]
        .value
        .as_ref()
        .and_then(|v| v.as_ref().ok())
        .expect("значение после правки");
    assert_ne!(base, after, "дерево перестроено из нового снапшота");

    // Возврат модели (App отдаёт снапшот «до» из undo — на уровне модели
    // просто реставрируем) → дерево совпадает с исходным: объяснение
    // вернулось без следов (F-5).
    scene.canvas = before_canvas.clone();
    scene.recompute_flow();
    assert_eq!(scene.canvas.nodes, before_canvas.nodes, "модель возвращена");
    let tree_restored = tree_of(&scene, LineageNodeId::total("unit"));
    assert_eq!(tree_restored, tree_base, "дерево совпало с исходным (F-5)");
}

/// F-9 (MCP-паритет): explain_number — текст с преамбулой при активном
/// what-if и новыми значениями; после сброса подмен и удаления сценария
/// текст и модель возвращаются к базе без следов.
#[test]
fn explain_number_follows_whatif_and_returns() {
    let mut scene = scene_of(demo_canvas(), "mcp");
    link_demo_tables(&mut scene.canvas);
    scene.recompute_flow();

    let before_nodes = scene.canvas.nodes.clone();
    let before_edges = scene.canvas.edges.clone();

    // База: текст без преамбулы, корень = 712370
    // (810000 − 430 − 97200).
    let out =
        dispatch(&mut scene, "explain_number", r#"{"node_id":"unit"}"#).expect("explain_number");
    assert_eq!(out["render"], "text");
    let text = out["text"].as_str().expect("текст");
    assert!(!text.contains("Режим what-if"), "{text}");
    assert!(text.contains("= 712370"), "{text}");
    assert_eq!(scene.canvas.nodes, before_nodes, "чтение MCP — без следов");

    // Подмена листа через MCP (тот же WhatIfOverrides, что UI): режим
    // поднимается, текст несёт преамбулу и новое значение корня.
    dispatch(
        &mut scene,
        "whatif_set_override",
        r#"{"node_id":"traffic","line":1,"expr":"users = 2000"}"#,
    )
    .expect("подмена");
    scene.recompute_flow();
    let out = dispatch(&mut scene, "explain_number", r#"{"node_id":"unit"}"#)
        .expect("explain_number what-if");
    let text = out["text"].as_str().expect("текст");
    assert!(text.starts_with("Режим what-if"), "{text}");
    assert!(
        text.contains("= 1425170"),
        "2000·810 − 430 − 2000·810·0.12: {text}"
    );

    // Возврат к базе одним действием (whatif_reset — runtime): значения
    // и текст совпали с базой; сценарий удалён — модель без следов.
    dispatch(&mut scene, "whatif_reset", "{}").expect("сброс подмен");
    dispatch(
        &mut scene,
        "whatif_scenario_delete",
        r#"{"name":"Сценарий MCP"}"#,
    )
    .expect("сценарий удалён");
    scene.recompute_flow();
    let out = dispatch(&mut scene, "explain_number", r#"{"node_id":"unit"}"#)
        .expect("explain_number база");
    let text = out["text"].as_str().expect("текст");
    assert!(!text.contains("Режим what-if"), "{text}");
    assert!(text.contains("= 712370"), "{text}");
    assert_eq!(scene.canvas.nodes, before_nodes, "ноды возвращены");
    assert_eq!(scene.canvas.edges, before_edges, "связи возвращены");
}

/// F-12/G2: покрытие цепочками на эталоне — 100% на связанной модели;
/// разрыв поставки arpu (удаление ребра) рвёт цепочки downstream —
/// покрытие падает; цифр нет — процент не определён.
#[test]
fn coverage_indicator_on_demo_model() {
    let mut scene = scene_of(demo_canvas(), "coverage");
    link_demo_tables(&mut scene.canvas);
    scene.recompute_flow();

    let data = DataSnapshots::new();
    let stat = chain_coverage(
        &scene.canvas,
        LineageFlow::Ready {
            solutions: &canvas_scene::read_flow(&scene.flow_active),
            data: &data,
        },
    );
    assert!(stat.total >= 5, "итог + построчные: {}", stat.total);
    assert_eq!(stat.covered, stat.total, "все цепочки доходят до листьев");
    assert_eq!(stat.percent(), Some(100));

    // Разрыв: ребро arpu удалено — $arpu не подставлен. Цифры с ошибкой
    // (revenue/risk/profit/итог unit) остаются в знаменателе
    // («вычисляемые», но не покрытые) — покрытие падает ниже 100%.
    scene.canvas.edges.retain(|edge| edge.id != "e-arpu");
    scene.recompute_flow();
    let stat = chain_coverage(
        &scene.canvas,
        LineageFlow::Ready {
            solutions: &canvas_scene::read_flow(&scene.flow_active),
            data: &data,
        },
    );
    assert_eq!(
        stat.total, 14,
        "знаменатель не сжался (Err-цифры считаются)"
    );
    assert!(
        stat.covered < stat.total,
        "после разрыва покрытие падает: {stat:?}"
    );
    assert!(
        stat.percent().is_some_and(|p| p < 100),
        "после разрыва покрытие < 100%: {stat:?}"
    );

    // Канвас без расчётных нод — цифр нет, процент не определён.
    let empty = scene_of(Canvas::default(), "coverage-empty");
    let stat = chain_coverage(
        &empty.canvas,
        LineageFlow::Ready {
            solutions: &canvas_scene::read_flow(&empty.flow_active),
            data: &data,
        },
    );
    assert_eq!(stat.total, 0);
    assert_eq!(stat.percent(), None);
}

/// G4: автосвязь на эталоне «5 таблиц» — precision ≥ 0.9 (каждое
/// предложение — настоящая связь ground truth) и recall ≥ 0.8 (все
/// связи найдены). На эталоне детектор даёт ровно 5/5 — 1.0/1.0.
#[test]
fn autolink_precision_recall_on_demo_tables() {
    let mut canvas = demo_canvas();
    // Шум: лишние присваивания без потребителей и совпадающее имя,
    // которое НИКТО не ждёт (не параметр и не $-ссылка) — предложений
    // дать не должно.
    calc_node(&mut canvas, "noise", "ctr = 0.05\nитог = 42", 1500.0);

    let proposals = find_proposals(&canvas);
    // Ground truth: users, arpu, rent, other, npl → unit.
    let mut truth: Vec<(String, String, String)> = ["users", "arpu", "rent", "other", "npl"]
        .iter()
        .map(|param| ("unit".to_owned(), param.to_string(), param.to_string()))
        .collect();
    truth.sort();

    let mut found: Vec<(String, String, String)> = proposals
        .iter()
        .map(|p| (p.to_node.clone(), p.param.clone(), p.param.clone()))
        .collect();
    found.sort();

    let true_positives = found.iter().filter(|item| truth.contains(item)).count();
    // precision = TP / (TP + FP) — ложных предложений нет.
    let precision = true_positives as f64 / found.len() as f64;
    // recall = TP / (TP + FN) — все связи ground truth найдены.
    let recall = true_positives as f64 / truth.len() as f64;
    assert_eq!(found.len(), truth.len(), "ровно 5 предложений: {found:?}");
    assert!(precision >= 0.9, "precision {precision} < 0.9");
    assert!(recall >= 0.8, "recall {recall} < 0.8");

    // Проверка паритета: созданные по предложениям рёбра питают модель —
    // итог юнит-экономики сходится с оракулом (810000 − 430 − 97200).
    for proposal in &proposals {
        spill_edge(
            &mut canvas,
            &format!("e-{}", proposal.param),
            &proposal.from_node,
            &proposal.to_node,
            &proposal.param,
        );
    }
    let mut scene = scene_of(canvas, "g4-flow");
    scene.recompute_flow();
    let tree = tree_of(&scene, LineageNodeId::total("unit"));
    let profit = tree.nodes[0]
        .value
        .as_ref()
        .and_then(|v| v.as_ref().ok())
        .expect("итог вычислен");
    assert_eq!(profit.to_string(), "712370", "оракул юнит-экономики");
}
