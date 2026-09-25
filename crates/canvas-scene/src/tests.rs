//! MCP-тесты инструментов канваса (FR-037 MW1/ADR-0012, перенос из
//! canvas-app/src/main.rs без изменений ассертов): диспетчер 27
//! инструментов, undo-инварианты MCP-мутаций, формулы/поток, эталоны CP1
//! (mcp_fr029_instagram_mvp_reference), CP3 (graph_apply мини-эталон №1) и
//! CP5 (analyze_bottlenecks). Отличается от исходника только конструкция
//! viewport (`Viewport::default()` вместо `Camera::default()` — те же
//! значения 0/0/1) и пути импортов. Исполняются нативно и под
//! wasm32-wasip1 (wasmtime, RUST_TEST_THREADS=1) — гейт FR-037.

use std::path::PathBuf;

use canvas_core::analyze;
use canvas_core::expr::ExprOutcome;
use canvas_core::flow::FlowKind;
use canvas_core::{Canvas, CanvasdeskExt, Edge, Node, NodeKind, Side, SpatialIndex};

use crate::mcp::{
    mcp_dispatch, mcp_flow_v2, mcp_unwrap_call, whatif_delta_rows, DEFAULT_FILE_CARD_H,
    DEFAULT_FILE_CARD_W,
};
use crate::scene::{display_body_text, SceneState, MAX_ZOOM, UNDO_LIMIT};

/// Тестовая сцена: заметка, файл, группа со связью (MCP-тесты).
fn mcp_scene() -> SceneState {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::text("n1", "Привет Мир", 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("f1", "docs/SPEC.md", 500.0, 100.0, 320.0, 220.0));
    let mut group = Node::group("g1", 0.0, 0.0, 900.0, 600.0);
    group.label = Some("Зона работы".to_owned());
    canvas.nodes.push(group);
    canvas.add_edge(Edge::new("edge-1", "n1", None, "f1", Some(Side::Right)));
    SceneState::new(canvas, PathBuf::from("target/tmp/mcp.canvas"))
}

fn dispatch(
    scene: &mut SceneState,
    method: &str,
    params: &str,
) -> Result<serde_json::Value, String> {
    let params: serde_json::Value = serde_json::from_str(params).expect("params — JSON");
    let registry = canvas_core::templates::TemplateRegistry::builtin();
    mcp_dispatch(scene, &registry, method, &params)
}

/// canvas_info: счётчики нод/связей и путь к файлу.
#[test]
fn mcp_canvas_info_counts() {
    let mut scene = mcp_scene();
    let info = dispatch(&mut scene, "canvas_info", "{}").expect("canvas_info");
    assert_eq!(info["nodes"], 3);
    assert_eq!(info["edges"], 1);
    assert_eq!(info["path"], "target/tmp/mcp.canvas");
}

/// nodes_list: сводки без text по умолчанию, с text по флагу; node_get —
/// полная нода.
#[test]
fn mcp_nodes_list_and_get() {
    let mut scene = mcp_scene();
    let list = dispatch(&mut scene, "nodes_list", "{}").expect("nodes_list");
    let first = &list[0];
    assert_eq!(first["id"], "n1");
    assert_eq!(first["type"], "text");
    assert!(
        !first.as_object().unwrap().contains_key("text"),
        "text скрыт"
    );
    let list = dispatch(&mut scene, "nodes_list", r#"{"text":true}"#).expect("nodes_list с text");
    assert_eq!(list[0]["text"], "Привет Мир");

    let node = dispatch(&mut scene, "node_get", r#"{"id":"f1"}"#).expect("node_get");
    assert_eq!(node["file"], "docs/SPEC.md");
    assert_eq!(node["width"], 320.0);
    let err = dispatch(&mut scene, "node_get", r#"{"id":"ghost"}"#).expect_err("нет такой ноды");
    assert!(err.contains("не найдена"));
}

/// nodes_search: подстрока без учёта регистра по text/label/file.
#[test]
fn mcp_nodes_search_case_insensitive() {
    let mut scene = mcp_scene();
    let hits = dispatch(&mut scene, "nodes_search", r#"{"query":"привет"}"#).expect("search");
    assert_eq!(hits.as_array().expect("массив").len(), 1);
    assert_eq!(hits[0]["id"], "n1");
    // по label группы
    let hits =
        dispatch(&mut scene, "nodes_search", r#"{"query":"ЗОНА"}"#).expect("search по label");
    assert_eq!(hits[0]["id"], "g1");
    // по file
    let hits =
        dispatch(&mut scene, "nodes_search", r#"{"query":"spec.md"}"#).expect("search по file");
    assert_eq!(hits[0]["id"], "f1");
    // мимо
    let hits = dispatch(&mut scene, "nodes_search", r#"{"query":"zzz"}"#).expect("search пусто");
    assert!(hits.as_array().expect("массив").is_empty());
}

/// node_create_note: дефолтные размеры 260×120, переопределение, id
/// со свободным суффиксом, spatial index обновлён, канвас грязный.
#[test]
fn mcp_node_create_note() {
    let mut scene = mcp_scene();
    let created =
        dispatch(&mut scene, "node_create_note", r#"{"x":50.0,"y":900.0}"#).expect("create_note");
    assert_eq!(created["id"], "note-1");
    let index = scene.canvas.nodes.len() - 1;
    let node = &scene.canvas.nodes[index];
    assert_eq!((node.x, node.y), (50.0, 900.0));
    assert_eq!((node.width, node.height), (260.0, 120.0));
    assert!(scene.dirty_since.is_some(), "канвас грязный");
    // spatial видит новую ноду
    let hits = scene.spatial.query_rect([50.0, 900.0, 60.0, 910.0]);
    assert!(hits.contains(&index), "новая нода в spatial");

    let created = dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0.0,"y":0.0,"text":"abc","width":400.0,"height":300.0}"#,
    )
    .expect("create_note с размерами");
    assert_eq!(created["id"], "note-2");
    let node = scene.canvas.nodes.last().expect("нода");
    assert_eq!((node.width, node.height), (400.0, 300.0));
    assert_eq!(node.text.as_deref(), Some("abc"));

    // x обязателен
    assert!(dispatch(&mut scene, "node_create_note", r#"{"y":1.0}"#).is_err());
}

/// node_create_file: карточка по пути, файл на диске НЕ создаётся,
/// дефолтные размеры DROP_CARD.
#[test]
fn mcp_node_create_file_no_disk_write() {
    let mut scene = mcp_scene();
    let disk_path = PathBuf::from("target/tmp/mcp_never_created.txt");
    let _ = std::fs::remove_file(&disk_path);
    let params = format!(r#"{{"path":"{}","x":10.0,"y":20.0}}"#, disk_path.display());
    let created = dispatch(&mut scene, "node_create_file", &params).expect("create_file");
    assert_eq!(created["id"], "file-1");
    let node = scene.canvas.nodes.last().expect("нода");
    assert_eq!(node.kind(), NodeKind::File);
    assert_eq!(
        (node.width, node.height),
        (DEFAULT_FILE_CARD_W, DEFAULT_FILE_CARD_H)
    );
    assert!(!disk_path.exists(), "MCP не создаёт файл на диске");
}

/// node_update_text / node_move / node_resize: модель + spatial + dirty.
#[test]
fn mcp_node_update_move_resize() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"Новый текст"}"#,
    )
    .expect("update_text");
    assert_eq!(scene.canvas.nodes[0].text.as_deref(), Some("Новый текст"));

    dispatch(&mut scene, "node_move", r#"{"id":"n1","x":-50.0,"y":42.0}"#).expect("move");
    assert_eq!(
        (scene.canvas.nodes[0].x, scene.canvas.nodes[0].y),
        (-50.0, 42.0)
    );
    let hits = scene.spatial.query_rect([-50.0, 42.0, -40.0, 52.0]);
    assert!(hits.contains(&0), "spatial обновлён после move");

    dispatch(
        &mut scene,
        "node_resize",
        r#"{"id":"n1","width":500.0,"height":400.0}"#,
    )
    .expect("resize");
    assert_eq!(
        (scene.canvas.nodes[0].width, scene.canvas.nodes[0].height),
        (500.0, 400.0)
    );

    assert!(dispatch(&mut scene, "node_move", r#"{"id":"ghost","x":0.0,"y":0.0}"#).is_err());
    assert!(dispatch(&mut scene, "node_resize", r#"{"id":"n1","width":1.0}"#).is_err());
}

/// FR-005 node_edit: обновляются ТОЛЬКО переданные поля; label/color
/// null — сброс; геометрия — с обновлением spatial; ответ — сводка.
#[test]
fn mcp_node_edit_updates_only_given_fields() {
    let mut scene = mcp_scene();
    // Только text: координаты/размеры/подпись не тронуты
    let summary = dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","text":"Отредактировано"}"#,
    )
    .expect("node_edit text");
    assert_eq!(summary["id"], "n1");
    assert_eq!(summary["text"], "Отредактировано");
    assert_eq!(
        (scene.canvas.nodes[0].x, scene.canvas.nodes[0].y),
        (100.0, 100.0)
    );
    assert_eq!(scene.canvas.nodes[0].width, 260.0);
    // FR-061 T9: prose-фолбэк описания убран — у обычной заметки зоны
    // описания нет. FR-069 (этап F): growth-only refit подгоняет высоту
    // для ВСЕХ нод (ранний выход по node_shows_result_footer снят),
    // резерв футера — по флагу (у заметки без итогов футера нет — без
    // «пустого хвоста»). Правка поля text высоту НЕ меняет сама
    // (геометрия не передана) — это отдельный документированный refit.
    assert_eq!(scene.canvas.nodes[0].height, 120.0);
    assert_eq!(scene.canvas.nodes[0].label, None);
    // Только геометрия: text не тронут
    dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","x":500.0,"y":600.0,"width":300.0,"height":200.0}"#,
    )
    .expect("node_edit geometry");
    assert_eq!(
        scene.canvas.nodes[0].text.as_deref(),
        Some("Отредактировано")
    );
    let hits = scene.spatial.query_rect([500.0, 600.0, 510.0, 610.0]);
    assert!(hits.contains(&0), "spatial обновлён после node_edit");
    // label: строка — задан, null — сброс
    dispatch(&mut scene, "node_edit", r#"{"id":"g1","label":"Моя зона"}"#)
        .expect("node_edit label");
    assert_eq!(scene.canvas.nodes[2].label.as_deref(), Some("Моя зона"));
    dispatch(&mut scene, "node_edit", r#"{"id":"g1","label":null}"#).expect("node_edit label null");
    assert_eq!(scene.canvas.nodes[2].label, None);
    // color: пресет и сброс
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","color":"3"}"#).expect("node_edit color");
    assert_eq!(scene.canvas.nodes[0].color.as_deref(), Some("3"));
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","color":null}"#).expect("node_edit color null");
    assert_eq!(scene.canvas.nodes[0].color, None);
}

/// FR-005 node_edit: валидация — несуществующая нода, плохой color,
/// неположительные размеры, label не-строкой; сцена при ошибках
/// не меняется.
#[test]
fn mcp_node_edit_validation() {
    let mut scene = mcp_scene();
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"ghost"}"#).is_err());
    let before = scene.canvas.nodes[0].clone();
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","color":"9"}"#).is_err());
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","width":-5.0}"#).is_err());
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","height":0.0}"#).is_err());
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","label":42}"#).is_err());
    assert_eq!(scene.canvas.nodes[0], before, "ошибки не меняют ноду");
    // Пустой вызов (только id) — валиден: ничего не изменилось, но
    // сводка возвращена (дешёвая «проверка связи»)
    let summary = dispatch(&mut scene, "node_edit", r#"{"id":"n1"}"#).expect("no-op");
    assert_eq!(summary["id"], "n1");
    assert_eq!(scene.canvas.nodes[0], before);
}

/// node_delete: каскад связей, spatial перестроен, дети группы живы.
#[test]
fn mcp_node_delete_cascades_edges() {
    let mut scene = mcp_scene();
    // n1 связана с f1 — удаление n1 рвёт edge-1; дети группы g1 остаются
    dispatch(&mut scene, "node_delete", r#"{"id":"n1"}"#).expect("delete");
    assert_eq!(scene.canvas.nodes.len(), 2);
    assert!(scene.canvas.edges.is_empty(), "связь каскадно удалена");
    assert!(scene.canvas.node("g1").is_some(), "группа на месте");
    assert!(scene.canvas.node("f1").is_some(), "дети не удалены");
    // spatial консистентен с моделью: индексы пересчитаны (f1=0, g1=1)
    assert_eq!(
        scene
            .spatial
            .query_rect([-1000.0, -1000.0, 1000.0, 1000.0])
            .len(),
        2
    );
    // бывшее место n1 теперь покрывает группа (дети остались внутри)
    assert_eq!(scene.spatial.hit_test([110.0, 110.0]), Some(1));
    assert!(
        dispatch(&mut scene, "node_delete", r#"{"id":"n1"}"#).is_err(),
        "повторное удаление — ошибка"
    );
}

/// node_set_color: пресет, сброс null, отказ на мусоре.
#[test]
fn mcp_node_set_color_validation() {
    let mut scene = mcp_scene();
    dispatch(&mut scene, "node_set_color", r#"{"id":"n1","color":"4"}"#).expect("set_color");
    assert_eq!(scene.canvas.nodes[0].color.as_deref(), Some("4"));
    dispatch(&mut scene, "node_set_color", r#"{"id":"n1","color":null}"#).expect("сброс цвета");
    assert_eq!(scene.canvas.nodes[0].color, None);
    let err = dispatch(&mut scene, "node_set_color", r#"{"id":"n1","color":"red"}"#)
        .expect_err("не пресет");
    assert!(err.contains("\"1\"..\"6\""));
}

/// edge_create: id вида edge-N, стороны any→None / явные, валидация нод
/// и сторон; edge_delete по id и ошибка на отсутствующую связь.
#[test]
fn mcp_edge_create_delete() {
    let mut scene = mcp_scene();
    // дефолт any → стороны не заданы
    let created =
        dispatch(&mut scene, "edge_create", r#"{"from":"n1","to":"g1"}"#).expect("edge_create");
    assert_eq!(created["id"], "edge-2");
    let edge = scene.canvas.edges.last().expect("связь");
    assert_eq!(edge.from_side, None);
    assert_eq!(edge.to_side, None);
    // явные стороны
    let created = dispatch(
        &mut scene,
        "edge_create",
        r#"{"from":"f1","to":"n1","fromSide":"left","toSide":"bottom"}"#,
    )
    .expect("edge_create со сторонами");
    assert_eq!(created["id"], "edge-3");
    let edge = scene.canvas.edges.last().expect("связь");
    assert_eq!(edge.from_side, Some(Side::Left));
    assert_eq!(edge.to_side, Some(Side::Bottom));

    // несуществующая нода — ошибка, связь не создана
    assert!(dispatch(&mut scene, "edge_create", r#"{"from":"n1","to":"ghost"}"#).is_err());
    // мусорная сторона — ошибка
    assert!(dispatch(
        &mut scene,
        "edge_create",
        r#"{"from":"n1","to":"f1","fromSide":"diagonal"}"#
    )
    .is_err());
    assert_eq!(scene.canvas.edges.len(), 3, "валидные связи остались");

    dispatch(&mut scene, "edge_delete", r#"{"id":"edge-1"}"#).expect("edge_delete");
    assert_eq!(scene.canvas.edges.len(), 2);
    assert!(
        dispatch(&mut scene, "edge_delete", r#"{"id":"edge-1"}"#).is_err(),
        "повторное удаление — ошибка"
    );
}

/// FR-032: edges_list/edge_get — каноническая схема (id/from/to/kind/
/// fromLine?/fromSide/toSide), kind отражает тип потока, edge_get
/// неизвестного id — ошибка.
#[test]
fn mcp_edges_list_and_get() {
    let mut scene = mcp_scene();
    // value-ребро с построчным истоком (FR-014/FR-025)
    let mut value_edge = Edge::new("ve-1", "n1", None, "f1", None);
    value_edge.set_flow_kind(canvas_core::FlowKind::Value);
    value_edge.from_line = Some(2);
    scene.canvas.add_edge(value_edge);

    let list = dispatch(&mut scene, "edges_list", "{}").expect("edges_list");
    let edges = list.as_array().expect("массив рёбер");
    assert_eq!(edges.len(), 2);
    let control = &edges[0];
    assert_eq!(control["id"], "edge-1");
    assert_eq!(control["from"], "n1");
    assert_eq!(control["to"], "f1");
    assert_eq!(control["kind"], "control");
    assert_eq!(control["toSide"], "right");
    assert!(
        !control
            .as_object()
            .expect("объект")
            .contains_key("fromLine"),
        "fromLine опционален"
    );
    let value = &edges[1];
    assert_eq!(value["id"], "ve-1");
    assert_eq!(value["kind"], "value");
    assert_eq!(value["fromLine"], 2);

    let one = dispatch(&mut scene, "edge_get", r#"{"id":"ve-1"}"#).expect("edge_get");
    assert_eq!(one["id"], "ve-1");
    assert_eq!(one["kind"], "value");
    assert_eq!(one["fromLine"], 2);

    let err = dispatch(&mut scene, "edge_get", r#"{"id":"ghost"}"#).expect_err("нет такой связи");
    assert!(err.contains("не найдена"));
}

/// FR-032: graph_validate на чистом графе — valid:true, issues:[]; на
/// цикле — valid:false, ровно один E-CYCLE с участниками (топология
/// строится напрямую: MCP-пути цикл запрещают by design).
#[test]
fn mcp_graph_validate_clean_and_cycle() {
    let mut scene = mcp_scene();
    let report = dispatch(&mut scene, "graph_validate", "{}").expect("validate");
    assert_eq!(report["valid"], true);
    assert_eq!(report["issues"], serde_json::json!([]));

    // Цикл a→b→a (value): граф ломается — отчёт с кодом и участниками
    let mut canvas = Canvas::default();
    let mut a = Node::text("a", "a", 0.0, 0.0);
    a.set_expr(Some("1".to_owned()));
    let mut b = Node::text("b", "b", 220.0, 0.0);
    b.set_expr(Some("$in".to_owned()));
    canvas.nodes.push(a);
    canvas.nodes.push(b);
    let mut e1 = Edge::new("e1", "a", None, "b", None);
    e1.set_flow_kind(canvas_core::FlowKind::Value);
    let mut e2 = Edge::new("e2", "b", None, "a", None);
    e2.set_flow_kind(canvas_core::FlowKind::Value);
    canvas.add_edge(e1);
    canvas.add_edge(e2);
    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/mcp-validate.canvas"));
    let report = dispatch(&mut scene, "graph_validate", "{}").expect("validate");
    assert_eq!(report["valid"], false);
    let issues = report["issues"].as_array().expect("issues");
    assert_eq!(issues.len(), 1, "цикл — единственный issue: {issues:?}");
    assert_eq!(issues[0]["code"], "E-CYCLE");
    assert_eq!(issues[0]["severity"], "error");
    let message = issues[0]["message"].as_str().expect("message");
    assert!(
        message.contains('a') && message.contains('b'),
        "участники: {message}"
    );
    // Чтение: undo-стек и dirty не затронуты (валидация не мутирует)
    assert!(scene.dirty_since.is_none(), "канвас не помечен грязным");
}

/// FR-032: graph_validate ловит E-OVERLOAD и W-UNUSED-SLOT с точными
/// node_id/edge_id (нечитаемый — именно второй слот); warning не делает
/// модель невалидной.
#[test]
fn mcp_graph_validate_overload_and_unused_slot() {
    let mut canvas = Canvas::default();
    let mut mm1 = Node::text("mm1", "mm1", 0.0, 0.0);
    mm1.set_expr(Some("mm1(1200 rps, 1000 rps)".to_owned()));
    let mut a = Node::text("a", "a", 220.0, 0.0);
    a.set_expr(Some("10".to_owned()));
    let mut b = Node::text("b", "b", 220.0, 180.0);
    b.set_expr(Some("20".to_owned()));
    let mut sum = Node::text("sum", "sum", 440.0, 0.0);
    sum.set_expr(Some("$1 + 100".to_owned()));
    canvas.nodes.push(mm1);
    canvas.nodes.push(a);
    canvas.nodes.push(b);
    canvas.nodes.push(sum);
    let mut e1 = Edge::new("e-a", "a", None, "sum", None);
    e1.set_flow_kind(canvas_core::FlowKind::Value);
    let mut e2 = Edge::new("e-b", "b", None, "sum", None);
    e2.set_flow_kind(canvas_core::FlowKind::Value);
    canvas.add_edge(e1);
    canvas.add_edge(e2);
    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/mcp-validate2.canvas"));
    let report = dispatch(&mut scene, "graph_validate", "{}").expect("validate");
    assert_eq!(report["valid"], false, "E-OVERLOAD — ошибка");
    let issues = report["issues"].as_array().expect("issues");
    assert_eq!(issues.len(), 2, "перегрузка + нечитаемый слот: {issues:?}");
    assert_eq!(issues[0]["code"], "E-OVERLOAD");
    assert_eq!(issues[0]["node_id"], "mm1");
    assert_eq!(issues[1]["code"], "W-UNUSED-SLOT");
    assert_eq!(issues[1]["node_id"], "sum");
    // Формула читает только $1 — предупреждение о ребре второго слота
    assert_eq!(issues[1]["edge_id"], "e-b");
}

/// viewport_get/set: центр и зум, кламп зума камерой.
#[test]
fn mcp_viewport_get_set() {
    let mut scene = mcp_scene();
    let view = dispatch(&mut scene, "viewport_get", "{}").expect("viewport_get");
    assert_eq!(view["x"], 0.0);
    assert_eq!(view["zoom"], 1.0);

    let view = dispatch(
        &mut scene,
        "viewport_set",
        r#"{"x":100.0,"y":-50.0,"zoom":2.5}"#,
    )
    .expect("viewport_set");
    assert_eq!(view["x"].as_f64().expect("x"), 100.0);
    assert_eq!(view["y"].as_f64().expect("y"), -50.0);
    assert_eq!(view["zoom"], 2.5);
    // зум клампится
    let view = dispatch(
        &mut scene,
        "viewport_set",
        r#"{"x":0.0,"y":0.0,"zoom":100.0}"#,
    )
    .expect("viewport_set зум");
    assert_eq!(view["zoom"], MAX_ZOOM);
    // zoom опционален
    let view = dispatch(&mut scene, "viewport_set", r#"{"x":1.0,"y":2.0}"#)
        .expect("viewport_set без зума");
    assert_eq!(view["zoom"], MAX_ZOOM, "зум не задет");
}

/// mcp_unwrap_call: tools/call → (name, arguments); прочие методы как есть.
#[test]
fn mcp_unwrap_call_passthrough_and_tool_name() {
    let params: serde_json::Value =
        serde_json::json!({"name": "node_move", "arguments": {"id": "n1", "x": 1.0, "y": 2.0}});
    let (method, args) = mcp_unwrap_call("tools/call", &params);
    assert_eq!(method, "node_move");
    assert_eq!(args, serde_json::json!({"id": "n1", "x": 1.0, "y": 2.0}));

    // метод напрямую (тесты dispatch) — без изменений
    let params = serde_json::json!({"id": "n1"});
    let (method, args) = mcp_unwrap_call("node_get", &params);
    assert_eq!(method, "node_get");
    assert_eq!(args, params);

    // arguments отсутствует → Null, не паника
    let params = serde_json::json!({"name": "canvas_info"});
    let (method, args) = mcp_unwrap_call("tools/call", &params);
    assert_eq!(method, "canvas_info");
    assert_eq!(args, serde_json::Value::Null);
}

/// Неизвестный инструмент и кривые параметры — Err (посредник сделает isError).
#[test]
fn mcp_unknown_tool_and_bad_params() {
    let mut scene = mcp_scene();
    assert!(dispatch(&mut scene, "canvas_destroy", "{}").is_err());
    assert!(dispatch(&mut scene, "node_get", "{}").is_err());
    assert!(dispatch(&mut scene, "nodes_search", r#"{"query":42}"#).is_err());
}

/// Лимит истории — ровно 50 (запрос «не менее 50»): 55 шагов → 50,
/// старейший вытеснен, 50-й отменяем.
#[test]
fn undo_stack_limit_is_fifty() {
    let mut scene = mcp_scene();
    for i in 0..55 {
        dispatch(
            &mut scene,
            "node_create_note",
            &format!(r#"{{"x": {i}.0, "y": 0.0}}"#),
        )
        .expect("node_create_note");
    }
    assert_eq!(scene.undo_stack.len(), 55.min(UNDO_LIMIT));
    assert_eq!(scene.undo_stack.len(), 50, "глубина ровно 50");
    // 55 созданий, отменяем 50: первые 5 созданий вне истории
    // (вытеснены) — в сцене 3 исходных + 5 = 8 нод
    for _ in 0..50 {
        let Some(before) = scene.take_undo() else {
            panic!("история не должна кончиться раньше 50 шагов");
        };
        scene.canvas = before;
        scene.spatial = SpatialIndex::build(&scene.canvas);
    }
    assert_eq!(
        scene.canvas.nodes.len(),
        8,
        "3 исходных + 5 вытеснённых из истории созданий"
    );
}

/// Удаление ноды через MCP → undo восстанавливает ноду И каскадную
/// связь; redo возвращает удаление.
#[test]
fn mcp_delete_undo_redo_roundtrip() {
    let mut scene = mcp_scene();
    let before = scene.canvas.clone();
    dispatch(&mut scene, "node_delete", r#"{"id":"n1"}"#).expect("node_delete");
    // n1 удалена, edge-1 оборвана каскадом
    assert_eq!(scene.canvas.nodes.len(), 2);
    assert!(scene.canvas.edges.is_empty());
    // undo: сцена «до» возвращается целиком
    let snapshot = scene.take_undo().expect("шаг undo есть");
    assert_eq!(snapshot, before, "снапшот — состояние до удаления");
    scene.canvas = snapshot;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    assert_eq!(scene.canvas.nodes.len(), 3);
    assert_eq!(scene.canvas.edges.len(), 1);
    // redo: удаление возвращается
    let after = scene.take_redo().expect("шаг redo есть");
    assert_eq!(after.nodes.len(), 2);
    assert!(after.edges.is_empty());
}

/// push нового шага обнуляет ветку redo (стандарт undo-модели).
#[test]
fn new_action_clears_redo_branch() {
    let mut scene = mcp_scene();
    dispatch(&mut scene, "node_move", r#"{"id":"n1","x":10.0,"y":10.0}"#).expect("node_move");
    let _ = scene.take_undo().expect("undo доступен");
    assert_eq!(scene.redo_stack.len(), 1);
    // новое действие после undo — redo ветка сброшена
    dispatch(&mut scene, "node_set_color", r#"{"id":"n1","color":"3"}"#).expect("node_set_color");
    assert!(scene.redo_stack.is_empty(), "redo обнулён новым шагом");
    assert_eq!(scene.undo_stack.len(), 1, "в истории только новый шаг");
}

/// Валидационные ошибки MCP не оставляют пустых шагов: node_get /
/// неизвестный id / кривой color — история пуста.
#[test]
fn mcp_validation_errors_leave_no_steps() {
    let mut scene = mcp_scene();
    // чтение — не мутация
    dispatch(&mut scene, "nodes_list", "{}").expect("nodes_list");
    assert!(scene.undo_stack.is_empty(), "чтение не шаг");
    // несуществующий id — Err до мутации, шага нет
    assert!(dispatch(&mut scene, "node_delete", r#"{"id":"нет"}"#).is_err());
    assert!(scene.undo_stack.is_empty(), "ошибка валидации не шаг");
    // node_edit с невалидным width — Err
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","width":-5.0}"#).is_err());
    assert_eq!(scene.undo_stack.len(), 1, "node_edit пушит до мутаций");
    // этот шаг откатывает частично применённые поля (text/label)
    let snapshot = scene.take_undo().expect("шаг есть");
    assert_eq!(snapshot, mcp_scene().canvas, "снапшот — исходная сцена");
}

/// edge_delete отсутствующей связи — Err без шага (сравнение после).
#[test]
fn mcp_edge_delete_missing_no_step() {
    let mut scene = mcp_scene();
    assert!(dispatch(&mut scene, "edge_delete", r#"{"id":"нет"}"#).is_err());
    assert!(scene.undo_stack.is_empty(), "no-op удаления — не шаг");
    // существующая связь — шаг есть
    dispatch(&mut scene, "edge_delete", r#"{"id":"edge-1"}"#).expect("edge_delete");
    assert_eq!(scene.undo_stack.len(), 1);
}

/// MCP node_edit { expr } — формула сохранена, результат пересчитан
/// в expr_results (инвариант 4: runtime, не в .canvas).
#[test]
fn mcp_node_edit_expr_computes_result() {
    let mut scene = mcp_scene();
    let summary = dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","expr":"5 ms × 200 req/s"}"#,
    )
    .expect("node_edit expr");
    assert_eq!(summary["expr"], "5 ms × 200 req/s", "формула в сводке");
    // Результат — runtime-кэш, в модели его нет
    let value = scene.expr_results.get("n1").expect("результат есть");
    match value {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 ms·req/s"),
        other => panic!("ожидался результат, получено: {other:?}"),
    }
    let json = scene.canvas.to_json().expect("сериализация");
    assert!(
        !json.contains("1\u{a0}000 ms"),
        "результат не сериализуется"
    );
    assert!(json.contains("5 ms × 200 req/s"), "формула сериализуется");
}

/// MCP node_edit { expr: null } — сброс calc-режима; невалидная
/// формула — Err с диагностикой, нода не менялась и шага undo нет
/// (валидация ДО push_undo).
#[test]
fn mcp_node_edit_expr_null_and_invalid() {
    let mut scene = mcp_scene();
    // Сброс отсутствующей формулы — no-op без ошибки
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","expr":null}"#).expect("expr null");
    assert!(!scene.expr_results.contains_key("n1"));

    // Установка, затем сброс через null
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","expr":"1k rps"}"#).expect("expr set");
    assert!(scene.expr_results.contains_key("n1"));
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","expr":null}"#).expect("expr reset");
    assert!(
        scene.canvas.node("n1").and_then(Node::expr).is_none(),
        "формула удалена из модели"
    );
    assert!(!scene.expr_results.contains_key("n1"), "результат удалён");

    // Невалидная формула: Err, нода не тронута, undo-шага нет
    let steps_before = scene.undo_stack.len();
    let err = dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","expr":"= invalid @#$"}"#,
    )
    .expect_err("парсинг формулы");
    assert!(err.contains("expr"), "диагностика с префиксом поля: {err}");
    assert_eq!(
        scene.undo_stack.len(),
        steps_before,
        "невалидный expr не пушит шаг"
    );
    // Кривой тип expr — тоже Err
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","expr":42}"#).is_err());
}

/// Формула с ошибкой вычисления сохраняется (парсинг ок), результат —
/// красная диагностика в expr_results (MCP отвергает только синтаксис).
#[test]
fn mcp_node_edit_expr_eval_error_is_stored() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","expr":"5 ms + 3 rps"}"#,
    )
    .expect("синтаксически корректная формула сохраняется");
    match scene.expr_results.get("n1").expect("запись есть") {
        ExprOutcome::Err(msg) => {
            assert!(msg.contains("не совместимы"), "диагностика: {msg}")
        }
        other => panic!("ожидалась ошибка вычисления: {other:?}"),
    }
}

/// Интеграционный сценарий верификации FR-013: node_update_text со
/// строками «= …» выводит формулу; построчный результат — Numi-стиль
/// (строки сценария); undo восстанавливает пустой expr и убирает
/// результаты; загрузка с canvasdesk.expr без формульных строк в
/// тексте — программный итог в футере.
#[test]
fn expr_undo_redo_restores_formula() {
    let mut scene = mcp_scene();
    // Заметка с формулой: текст с «=»-строками → формула + построчный
    // результат (Numi-стиль)
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"Параметры\n= 1 sec + 500 ms"}"#,
    )
    .expect("node_update_text");
    assert_eq!(
        scene.canvas.node("n1").and_then(Node::expr),
        Some("1 sec + 500 ms"),
        "формула выведена из текста"
    );
    // FR-014: expr_results — карта потока значений (запись есть для
    // любой expr-ноды); вытеснение футера построчными результатами —
    // правило РЕНДЕРА (text.rs), а не отсутствие записи
    assert!(
        scene.expr_results.contains_key("n1"),
        "результат формулы в карте потока"
    );
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты есть");
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], None, "проза без результата");
    match lines[1].as_ref().expect("результат «=»-строки") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1.5 sec"),
        other => panic!("ожидалось значение: {other:?}"),
    }

    // Undo: expr пуст, результатов нет
    let before = scene.take_undo().expect("шаг есть");
    scene.canvas = before;
    scene.recompute_all_expr();
    assert_eq!(
        scene.canvas.node("n1").and_then(Node::expr),
        None,
        "после undo формула из правки исчезла"
    );
    assert!(!scene.expr_results.contains_key("n1"), "итога нет");
    assert!(
        !scene.expr_line_results.contains_key("n1"),
        "построчных результатов нет"
    );

    // Загрузка с формулой (текст без формульных строк): recompute_all_expr
    // при SceneState::new даёт программный итог в футере
    let mut canvas = Canvas::default();
    let mut note = Node::text("calc", "Gateway", 0.0, 0.0);
    note.set_expr(Some("1k rps".to_owned()));
    canvas.nodes.push(note);
    let scene = SceneState::new(canvas, PathBuf::from("target/tmp/expr.canvas"));
    match scene
        .expr_results
        .get("calc")
        .expect("результат при загрузке")
    {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 rps"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// FR-013 (правка 2, Numi-стиль): каждая формульная строка текста —
/// свой результат; переменные протекают между строками; проза и
/// пустые строки без результата; canvasdesk.expr не материализуется.
/// FR-014: n1 входит в карту потока (expr_results — итог = последняя
/// формульная строка), но футер на карточке не рисуется — построчные
/// результаты вытесняют программный итог (правило рендера).
#[test]
fn per_line_results_numi_sheet() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"Gateway\nrps = 1000\n\nlatency = 50 ms\nlatency × rps"}"#,
    )
    .expect("node_update_text");
    assert_eq!(
        scene.canvas.node("n1").and_then(Node::expr),
        None,
        "авто-формулы не материализуются в canvasdesk.expr"
    );
    // FR-014: значение ноды в карте потока есть (последняя формульная
    // строка «latency × rps»); показ футера гасится построчными
    // результатами — правило рендера, не карты
    match scene.expr_results.get("n1").expect("итог в карте потока") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "50\u{a0}000 ms"),
        other => panic!("ожидалось значение: {other:?}"),
    };
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты есть");
    assert_eq!(lines.len(), 5, "Vec выровнен по строкам текста");
    assert_eq!(lines[0], None, "проза");
    match lines[1].as_ref().expect("присваивание — результат") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000"),
        other => panic!("ожидалось значение: {other:?}"),
    }
    assert_eq!(lines[2], None, "пустая строка");
    match lines[4]
        .as_ref()
        .expect("выражение с переменными — результат")
    {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "50\u{a0}000 ms"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// MCP node_edit { expr } при тексте без формульных строк — программный
/// итог в expr_results (футер карточки), построчных результатов нет.
#[test]
fn mcp_program_result_fallback_for_prose_text() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","expr":"5 ms × 200 req/s"}"#,
    )
    .expect("node_edit expr");
    assert!(
        !scene.expr_line_results.contains_key("n1"),
        "в прозе формульных строк нет — построчных результатов нет"
    );
    match scene.expr_results.get("n1").expect("программный итог") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 ms·req/s"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// FR-013 (правка 2): выражения без «=» считаются построчно при
/// node_update_text и при загрузке канваса; проза результата не создаёт.
#[test]
fn auto_lines_compute_without_equal_prefix() {
    let mut scene = mcp_scene();
    // Явной формулы нет, последняя строка — выражение: результат на ней
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"Пропускная способность\n1000 rps * 2"}"#,
    )
    .expect("node_update_text");
    assert_eq!(
        scene.canvas.node("n1").and_then(Node::expr),
        None,
        "авто-формула не материализуется в canvasdesk.expr"
    );
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты есть");
    assert_eq!(lines[0], None, "проза — не формула");
    match lines[1].as_ref().expect("результат авто-строки") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "2\u{a0}000 rps"),
        other => panic!("ожидалось значение: {other:?}"),
    }

    // Проза: цифры в строке есть, но выражение не парсится — результата нет
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"План на 15:00"}"#,
    )
    .expect("node_update_text проза");
    assert!(
        !scene.expr_line_results.contains_key("n1"),
        "проза — не формула"
    );
    assert!(!scene.expr_results.contains_key("n1"), "итога тоже нет");

    // Загрузка канваса с авто-формулой: результат пересчитывается
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::text("auto", "5 ms × 200 req/s", 0.0, 0.0));
    let scene = SceneState::new(canvas, PathBuf::from("target/tmp/auto.canvas"));
    let lines = scene
        .expr_line_results
        .get("auto")
        .expect("построчные результаты при загрузке");
    match lines[0].as_ref().expect("результат") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 ms·req/s"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// FR-013 (правка 4): ТОЧНЫЕ листы владельца из фидбека — присваивания
/// (обе формы), ссылки на переменные, кумулятивное окружение. Каждая
/// строка показывает результат, присваивания наполняют Env.
#[test]
fn per_line_results_owner_sheets_v4() {
    let mut scene = mcp_scene();
    // Нода 1 владельца
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"123 + 5123 = a\n235 + 2323 = b\nx = 200\nc = a + b\n200 + x"}"#,
    )
    .expect("node_update_text нода 1");
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты ноды 1");
    assert_eq!(lines.len(), 5);
    let texts: Vec<String> = lines
        .iter()
        .map(|line| match line {
            Some(ExprOutcome::Ok(value)) => value.to_string(),
            other => panic!("ожидалось значение: {other:?}"),
        })
        .collect();
    assert_eq!(texts[0], "5\u{a0}246", "хвостовое присваивание a");
    assert_eq!(texts[1], "2\u{a0}558", "хвостовое присваивание b");
    assert_eq!(texts[2], "200", "чистое присваивание x");
    assert_eq!(texts[3], "7\u{a0}804", "ссылки на переменные c = a + b");
    assert_eq!(texts[4], "400", "ссылка на x");

    // Нода 2 владельца
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"x = 200\n250 + x"}"#,
    )
    .expect("node_update_text нода 2");
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты ноды 2");
    assert_eq!(lines.len(), 2);
    match lines[0].as_ref().expect("x = 200") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "200"),
        other => panic!("ожидалось значение: {other:?}"),
    }
    match lines[1].as_ref().expect("250 + x") {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "450"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// FR-013 (правка 4): ошибки строк доходят до expr_line_results как
/// ExprOutcome::Err (для красного бейджа и тултипа); ссылки на
/// объявленную, но не вычислившуюся переменную тоже помечены.
#[test]
fn per_line_error_outcomes_reach_scene() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_update_text",
        r#"{"id":"n1","text":"x = 1 sec + 2 req\nx + 1\nитог = 2 + 2"}"#,
    )
    .expect("node_update_text ошибки");
    let lines = scene
        .expr_line_results
        .get("n1")
        .expect("построчные результаты есть");
    assert_eq!(lines.len(), 3);
    assert!(
        matches!(&lines[0], Some(ExprOutcome::Err(_))),
        "несовместимость единиц видна"
    );
    assert!(
        matches!(&lines[1], Some(ExprOutcome::Err(_))),
        "ссылка на объявленную, но не вычисленную x видна"
    );
    assert!(
        matches!(&lines[2], Some(ExprOutcome::Ok(_))),
        "строка ниже по-прежнему вычисляется"
    );
}

/// flow_set_kind: тогл control → value → control; round-trip в extra.
#[test]
fn mcp_flow_set_kind_toggles_flow() {
    let mut scene = mcp_scene();
    let out = dispatch(
        &mut scene,
        "flow_set_kind",
        r#"{"id":"edge-1","kind":"value"}"#,
    )
    .expect("flow_set_kind value");
    assert_eq!(out["kind"], "value");
    assert_eq!(
        scene.canvas.edges[0].flow_kind(),
        canvas_core::flow::FlowKind::Value
    );
    // Тогл обратно — поле удаляется
    let out = dispatch(
        &mut scene,
        "flow_set_kind",
        r#"{"id":"edge-1","kind":"control"}"#,
    )
    .expect("flow_set_kind control");
    assert_eq!(out["kind"], "control");
    assert!(
        scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
        "control удаляет расширение целиком"
    );
    // Некорректный kind — ошибка
    let err = dispatch(
        &mut scene,
        "flow_set_kind",
        r#"{"id":"edge-1","kind":"поток"}"#,
    )
    .expect_err("kind валидируется");
    assert!(err.contains("kind"), "{err}");
}

/// flow_set_kind при цикле — isError с участниками (DAG-инвариант MCP).
#[test]
fn mcp_flow_set_kind_rejects_cycle() {
    let mut scene = mcp_scene();
    // A → B → A из новых нод и value-рёбер
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fa","x":0,"y":0,"text":"A"}"#,
    )
    .expect("fa");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fb","x":300,"y":0,"text":"B"}"#,
    )
    .expect("fb");
    // id создаются автоматически (note-N) — найдём по тексту
    let id_a = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("A"))
        .map(|n| n.id.clone())
        .expect("нода A");
    let id_b = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("B"))
        .map(|n| n.id.clone())
        .expect("нода B");
    let e1 = dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
    )
    .expect("edge A→B")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{e1}","kind":"value"}}"#),
    )
    .expect("A→B value");
    let e2 = dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from":"{id_b}","to":"{id_a}"}}"#),
    )
    .expect("edge B→A")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let err = dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{e2}","kind":"value"}}"#),
    )
    .expect_err("цикл B→A→B отклонён");
    assert!(err.contains("цикл"), "{err}");
    // А control — пожалуйста
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{e2}","kind":"control"}}"#),
    )
    .expect("control допустим");
}

/// flow_recalc: живой пересчёт цепочки A→B→C; правка формулы A меняет
/// downstream; удаление ребра — «вход отсутствует».
#[test]
fn mcp_flow_recalc_chain_live_reval() {
    let mut scene = mcp_scene();
    // A=5, B=$in × 2, C=$in + 1
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fa","x":0,"y":0,"text":"A\n= 5"}"#,
    )
    .expect("fa");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fb","x":300,"y":0,"text":"B\n= $in × 2"}"#,
    )
    .expect("fb");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fc","x":600,"y":0,"text":"C\n= $in + 1"}"#,
    )
    .expect("fc");
    let id = |scene: &SceneState, text: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| {
                n.text
                    .as_deref()
                    .map(|t| t.starts_with(text))
                    .unwrap_or(false)
            })
            .map(|n| n.id.clone())
            .expect("нода сценария")
    };
    let (id_a, id_b, id_c) = (id(&scene, "A"), id(&scene, "B"), id(&scene, "C"));
    for (from, to) in [(&id_a, &id_b), (&id_b, &id_c)] {
        let edge_id = dispatch(
            &mut scene,
            "edge_create",
            &format!(r#"{{"from":"{from}","to":"{to}"}}"#),
        )
        .expect("edge")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        dispatch(
            &mut scene,
            "flow_set_kind",
            &format!(r#"{{"id":"{edge_id}","kind":"value"}}"#),
        )
        .expect("value-ребро");
    }
    // Верификация FR-014: {A: 5, B: 10, C: 11}
    let map = dispatch(&mut scene, "flow_recalc", "{}").expect("flow_recalc");
    assert_eq!(map[&id_a]["value"], 5.0);
    assert_eq!(map[&id_b]["value"], 10.0);
    assert_eq!(map[&id_c]["value"], 11.0);
    assert_eq!(map[&id_a]["unit"], "");
    // Правка A → downstream пересчитан: {A: 7, B: 14, C: 15}
    dispatch(
        &mut scene,
        "node_edit",
        &format!(r#"{{"id":"{id_a}","expr":"7"}}"#),
    )
    .expect("node_edit expr");
    let map = dispatch(&mut scene, "flow_recalc", "{}").expect("flow_recalc");
    assert_eq!(map[&id_b]["value"], 14.0, "downstream пересчитан живьём");
    assert_eq!(map[&id_c]["value"], 15.0);
    // Удаление value-ребра A→B — у B «вход отсутствует», C тоже
    let e_ab = scene
        .canvas
        .edges
        .iter()
        .find(|e| e.from_node == id_a && e.to_node == id_b)
        .map(|e| e.id.clone())
        .expect("ребро A→B");
    dispatch(&mut scene, "edge_delete", &format!(r#"{{"id":"{e_ab}"}}"#)).expect("edge_delete");
    let map = dispatch(&mut scene, "flow_recalc", "{}").expect("flow_recalc");
    assert!(map[&id_b]["error"]
        .as_str()
        .expect("ошибка входа")
        .contains("вход"));
    assert!(map[&id_c]["error"].as_str().is_some(), "downstream тоже");
}

/// flow_cycle_check: без value-циклов — []; после value-цикла — участники.
#[test]
fn mcp_flow_cycle_check_reports_participants() {
    let mut scene = mcp_scene();
    assert_eq!(
        dispatch(&mut scene, "flow_cycle_check", "{}").expect("[]"),
        serde_json::json!([])
    );
    // Контрольный цикл (edge-1 n1→f1 + обратный f1→n1) — НЕ значение
    dispatch(&mut scene, "edge_create", r#"{"from":"f1","to":"n1"}"#).expect("обратное ребро");
    dispatch(
        &mut scene,
        "flow_set_kind",
        r#"{"id":"edge-1","kind":"value"}"#,
    )
    .expect("прямое value");
    assert_eq!(
        dispatch(&mut scene, "flow_cycle_check", "{}").expect("[]"),
        serde_json::json!([]),
        "одно value-ребро цикла не создаёт"
    );
    // Обратное тоже value — цикл n1→f1→n1. Через MCP такой тогл
    // отклоняется (см. mcp_flow_set_kind_rejects_cycle), поэтому строим
    // чужой-файл сценарий прямой мутацией extra
    let back_index = scene
        .canvas
        .edges
        .iter()
        .position(|e| e.from_node == "f1" && e.to_node == "n1")
        .expect("обратное ребро");
    scene.canvas.edges[back_index].set_flow_kind(canvas_core::flow::FlowKind::Value);
    let participants = dispatch(&mut scene, "flow_cycle_check", "{}").expect("участники");
    // Участники отсортированы по id (лексикографически)
    assert_eq!(participants, serde_json::json!(["f1", "n1"]));
}

/// FR-014 + FR-006: undo тогла value → control восстанавливает поток
/// (формула downstream снова получает вход).
#[test]
fn mcp_flow_toggle_undo_restores_downstream() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fa","x":0,"y":0,"text":"A\n= 5"}"#,
    )
    .expect("fa");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"id":"fb","x":300,"y":0,"text":"B\n= $in × 2"}"#,
    )
    .expect("fb");
    let id_a = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("A\n= 5"))
        .map(|n| n.id.clone())
        .expect("A");
    let id_b = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.text.as_deref() == Some("B\n= $in × 2"))
        .map(|n| n.id.clone())
        .expect("B");
    let e1 = dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
    )
    .expect("edge")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{e1}","kind":"value"}}"#),
    )
    .expect("value");
    // B = 10
    assert_eq!(
        scene.expr_results.get(&id_b),
        Some(&ExprOutcome::Ok(canvas_core::expr::Value::scalar(10.0)))
    );
    // Тогл в control — у B «вход отсутствует»
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{e1}","kind":"control"}}"#),
    )
    .expect("control");
    match scene.expr_results.get(&id_b).expect("запись") {
        ExprOutcome::Err(msg) => assert!(msg.contains("вход"), "{msg}"),
        other => panic!("ожидалась ошибка входа: {other:?}"),
    }
    // Undo — снапшот «до тогла» возвращает value-ребро и пересчёт
    let before = scene.take_undo().expect("шаг undo");
    scene.canvas = before;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    scene.recompute_flow();
    assert_eq!(
        scene.expr_results.get(&id_b),
        Some(&ExprOutcome::Ok(canvas_core::expr::Value::scalar(10.0))),
        "после undo поток восстановлен"
    );
}

/// edge_ports: закрепление обоих концов, авто сбрасывает пины;
/// неизвестный id / невалидный pin — ошибка.
#[test]
fn mcp_edge_ports_pins_and_auto() {
    let mut scene = mcp_scene();
    let out =
        dispatch(&mut scene, "edge_ports", r#"{"id":"edge-1","pin":"both"}"#).expect("pin both");
    assert_eq!(out["pins"]["from"], true);
    assert_eq!(out["pins"]["to"], true);
    assert_eq!(scene.canvas.edges[0].port_pins(), (true, true));

    // auto — снятие всех закреплений
    let out = dispatch(&mut scene, "edge_ports", r#"{"id":"edge-1","pin":"auto"}"#).expect("auto");
    assert_eq!(out["pins"]["from"], false);
    assert_eq!(out["pins"]["to"], false);
    assert!(!scene.canvas.edges[0].ports_pinned());
    assert!(
        scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
        "пустое расширение удалено"
    );

    // Ошибки: неизвестный pin и неизвестный id
    assert!(dispatch(
        &mut scene,
        "edge_ports",
        r#"{"id":"edge-1","pin":"diagonal"}"#
    )
    .is_err());
    assert!(dispatch(&mut scene, "edge_ports", r#"{"id":"ghost","pin":"auto"}"#).is_err());
}

/// FR-019: template_list — built-in реестр отдаёт 61 шаблон с полной
/// схемой (FR-019: 15 + FR-027: 30 + audit-2026-09: 16; инвариант 4:
/// MCP-видимость эквивалентна UI; двуязычные имена).
#[test]
fn mcp_template_list_builtin_registry() {
    let mut scene = mcp_scene();
    let list = dispatch(&mut scene, "template_list", "{}").expect("list");
    let templates = list.as_array().expect("массив");
    assert_eq!(
        templates.len(),
        61,
        "все built-in шаблоны (FR-019: 15 + FR-027: 30 + audit-2026-09: 16)"
    );
    let lb = templates
        .iter()
        .find(|t| t["id"] == "com.canvasdesk.lb")
        .expect("com.canvasdesk.lb");
    assert_eq!(lb["name_en"], "Load Balancer");
    assert_eq!(lb["name_ru"], "Балансировщик нагрузки");
    // FR-029: секция outputs — версия схемы 1.1; FR-016 (CP5):
    // utilization-выход — минорный подъём до 1.2
    assert_eq!(lb["version"], "1.2.1");
    let lb_outputs = lb["outputs"].as_array().expect("outputs у lb (FR-029)");
    assert!(lb_outputs.iter().any(|o| o["name"] == "next_hop_rps"));
    // FR-016: named-выход utilization — источник ρ для анализатора
    assert!(
        lb_outputs.iter().any(|o| o["name"] == "utilization"),
        "outputs lb: {lb_outputs:?}"
    );
    assert_eq!(lb["category"], "backend");
    assert_eq!(lb["expr"], "mm1($rps, $service_rate, $servers)");
    assert_eq!(lb["params"]["rps"]["type"], "rate");
    assert_eq!(lb["params"]["rps"]["default"], 1000.0);
    assert_eq!(lb["params"]["rps"]["unit"], "rps");
    // Схема для UI: иконка и цвет категории в списке
    assert_eq!(lb["icon"], "lb");
    assert_eq!(lb["color"], "#4A90E2");
    // FR-020: источник каждого шаблона в списке
    assert_eq!(lb["source"], "builtin");
    // Категории: 14 backend + 6 network (+ product-analytics/unit-economics
    // из аудита 2026-09 — не проверяются здесь, см. реестр).
    let by_cat = |cat: &str| templates.iter().filter(|t| t["category"] == cat).count();
    assert_eq!(by_cat("backend"), 14);
    assert_eq!(by_cat("network"), 6);
}

/// FR-018: template_instantiate — text-нода с Numi-листом параметров
/// и снимком canvasdesk.template; переопределение параметра учитывается
/// формулой (пересчёт в потоке); undo возвращает состояние до создания.
#[test]
fn mcp_template_instantiate_creates_linked_node() {
    let mut scene = mcp_scene();
    let out = dispatch(
        &mut scene,
        "template_instantiate",
        r#"{"id":"com.canvasdesk.lb","x":150,"y":250,"params":{"rps":2000}}"#,
    )
    .expect("instantiate");
    let id = out["id"].as_str().expect("id").to_owned();
    let node = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.id == id)
        .expect("нода создана");
    assert_eq!(node.kind(), NodeKind::Text);
    assert_eq!(node.x, 150.0);
    // Numi-лист параметров с переопределением
    let text = node.text.as_deref().expect("текст");
    assert!(text.contains("rps = 2000 rps"), "текст: {text}");
    assert!(text.contains("servers = 2"));
    // Снимок template-ссылки
    let template = node.template().expect("template");
    assert_eq!(template.id, "com.canvasdesk.lb");
    assert_eq!(template.version, "1.2.1");
    assert_eq!(template.expr, "mm1($rps, $service_rate, $servers)");
    assert_eq!(template.params["rps"].num, 2000.0);
    assert_eq!(template.icon, "lb");
    // Формула в потоке (FR-014-стык): результат пересчитан
    assert!(
        scene.expr_results.contains_key(&id),
        "результат формулы шаблона в потоке"
    );
    // Undo: нода исчезает (undo-шаг при создании)
    let before = scene.canvas.nodes.len();
    let restored = scene.take_undo().expect("undo-шаг есть");
    scene.canvas = restored;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    assert_eq!(scene.canvas.nodes.len(), before - 1);
    assert!(scene.canvas.nodes.iter().all(|n| n.id != id));
}

/// FR-018: template_instantiate — ошибки: неизвестный id, параметр вне
/// границ манифеста (min/max), неизвестное имя параметра.
#[test]
fn mcp_template_instantiate_rejects_bad_input() {
    let mut scene = mcp_scene();
    // Неизвестный шаблон
    assert!(dispatch(
        &mut scene,
        "template_instantiate",
        r#"{"id":"ghost","x":0,"y":0}"#
    )
    .is_err());
    // rps < min 0
    assert!(dispatch(
        &mut scene,
        "template_instantiate",
        r#"{"id":"com.canvasdesk.lb","x":0,"y":0,"params":{"rps":-5}}"#
    )
    .is_err());
    // Неизвестное имя параметра
    assert!(dispatch(
        &mut scene,
        "template_instantiate",
        r#"{"id":"com.canvasdesk.lb","x":0,"y":0,"params":{"ghost":1}}"#
    )
    .is_err());
    // Ни одна ошибочная ветка не мутировала модель
    assert!(scene.undo_stack.is_empty());
}

/// edge_ports "from": WYSIWYG — в fromSide фиксируется текущая
/// эффективная сторона; свободный конец следует геометрии после
/// переноса ноды. Undo возвращает состояние до пина.
#[test]
fn mcp_edge_ports_pin_freezes_effective_side() {
    let mut scene = mcp_scene();
    // n1(100,100) → f1(500,100): кратчайшая пара Right → Left
    let out =
        dispatch(&mut scene, "edge_ports", r#"{"id":"edge-1","pin":"from"}"#).expect("pin from");
    assert_eq!(out["pins"]["from"], true);
    assert_eq!(out["pins"]["to"], false);
    assert_eq!(scene.canvas.edges[0].from_side, Some(Side::Right));
    assert_eq!(scene.canvas.edges[0].port_pins(), (true, false));

    // Перенос f1 влево за n1: закреплённый исток остаётся Right,
    // свободный сток переходит на кратчайший порт
    scene.canvas.nodes[1].x = -500.0;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    let curve = canvas_core::edge_curve(&scene.canvas, &scene.canvas.edges[0]).expect("кривая");
    assert_eq!(
        curve.p0,
        canvas_core::port_point(&scene.canvas.nodes[0], Side::Right),
        "right порт n1 закреплён"
    );
    assert_eq!(curve.p1, [-180.0, 210.0], "сток f1 — правый порт (авто)");

    // Undo: пин снят, снапшот до мутации
    let before = scene.take_undo().expect("шаг undo");
    scene.canvas = before;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    assert!(!scene.canvas.edges[0].ports_pinned());
}

/// Хелпер: значение в допуске ±1% (гейт эталона ADR-0005).
fn close_1pct(actual: f64, oracle: f64) -> bool {
    (actual - oracle).abs() <= oracle.abs() * 0.01
}

/// CP1 (FR-029, ADR-0005): эталонный сценарий «Instagram MVP» —
/// 12 нод, 10 value-рёбер с адресацией портов (fromOutput/toParam).
/// Сборка ТОЛЬКО через MCP (агентный путь), числа сходятся с таблицей
/// §Эталонные значения в допуске ±1%; правка DAU одним node_edit
/// удваивает всю цепочку без правки связей/формул (демо-критерий CP1).
#[test]
fn mcp_fr029_instagram_mvp_reference() {
    let mut scene = SceneState::new(
        Canvas::default(),
        PathBuf::from("target/tmp/instagram.canvas"),
    );

    // 1. Текстовая нода «Трафик-профиль» (Numi-лист)
    dispatch(
            &mut scene,
            "node_create_note",
            r#"{"x": 0, "y": 0, "width": 300, "text": "dau = 1000000\nsess = 4\nreq = 12\npeak = 2.5\nbudget = 45 ms\navg_rps = dau × sess × req × 1 rps / 86400\npeak_rps = avg_rps × peak"}"#,
        )
        .expect("traffic");
    // id ноды — из nodes_list (создана последней)
    let traffic = scene.canvas.nodes.last().expect("нода").id.clone();

    // 2. Инстанциация 11 шаблонных нод (ADR-0005 §Состав)
    let mut node_id = |template: &str, x: f32, y: f32, params: &str| {
        let result = dispatch(
            &mut scene,
            "template_instantiate",
            &format!(r#"{{"id": "{template}", "x": {x}, "y": {y}, "params": {params}}}"#),
        )
        .expect(template);
        result["id"].as_str().expect("id").to_owned()
    };
    // Параметры подобраны так, чтобы пик-нагрузка не перегружала сервисы
    // (E-OVERLOAD — отдельный сценарий; эталон A2 стартует чистым)
    let cdn = node_id(
        "com.canvasdesk.cdn",
        400.0,
        0.0,
        r#"{"cache_hit": 0.6, "origin_latency": 1}"#,
    );
    let lb = node_id("com.canvasdesk.tcp-lb", 800.0, 0.0, "{}");
    let gateway = node_id(
        "com.canvasdesk.api-gateway",
        1200.0,
        -200.0,
        r#"{"latency_budget": 2.5, "auth_overhead": 1}"#,
    );
    let auth = node_id("com.canvasdesk.auth-service", 1600.0, -400.0, "{}");
    let feed = node_id(
        "com.canvasdesk.graphql",
        1600.0,
        0.0,
        r#"{"resolver_time": 0.5}"#,
    );
    let media = node_id("com.canvasdesk.grpc-service", 1600.0, 400.0, "{}");
    let cache = node_id("com.canvasdesk.cache-redis", 1200.0, 400.0, "{}");
    let db = node_id("com.canvasdesk.db-sql-master", 2000.0, -100.0, "{}");
    let replica = node_id("com.canvasdesk.db-sql-replica", 2400.0, -100.0, "{}");
    let queue = node_id("com.canvasdesk.queue-kafka", 2000.0, 300.0, "{}");
    let workers = node_id(
        "com.canvasdesk.worker",
        2400.0,
        300.0,
        r#"{"processing_time": 2}"#,
    );

    // 3. Value-рёбра с адресацией портов (ADR-0005 §Цепочки проливания)
    let mut link = |from: &str, to: &str, from_output: &str, to_param: &str| {
        dispatch(
                &mut scene,
                "edge_create",
                &format!(
                    r#"{{"from": "{from}", "to": "{to}", "fromOutput": "{from_output}", "toParam": "{to_param}", "kind": "value"}}"#
                ),
            )
            .unwrap_or_else(|err| panic!("ребро {from}.{from_output} → {to}.{to_param}: {err}"));
    };
    link(&traffic, &cdn, "peak_rps", "rps");
    link(&cdn, &lb, "origin_rps", "connections_per_sec");
    link(&lb, &gateway, "out_gateway", "rps");
    link(&lb, &auth, "out_auth", "rps");
    link(&lb, &feed, "out_feed", "rps");
    link(&lb, &media, "out_media", "rps");
    link(&feed, &db, "db_qps", "qps");
    link(&db, &replica, "replica_load", "qps");
    link(&feed, &queue, "events", "produce_rate");
    link(&queue, &workers, "consume_rate", "tasks_per_sec");

    // 4. edges_list: агент видит адресацию (CR-013 G4)
    let edges = dispatch(&mut scene, "edges_list", "{}").expect("edges_list");
    let edges = edges.as_array().expect("массив");
    assert_eq!(edges.len(), 10, "10 value-рёбер");
    let first = &edges[0];
    assert_eq!(first["fromOutput"], "peak_rps");
    assert_eq!(first["toParam"], "rps");
    assert_eq!(first["kind"], "value");

    // 5. flow_recalc v2: значения против oracle ADR-0005 (±1%)
    let values = dispatch(&mut scene, "flow_recalc", "{}").expect("flow_recalc");
    let oracle = |node: &str, output: &str, expected: f64, what: &str| {
        let actual = values[node]["outputs"][output]["value"]
            .as_f64()
            .unwrap_or_else(|| panic!("{what}: нет значения — {values}"));
        assert!(
            close_1pct(actual, expected),
            "{what}: {actual} vs oracle {expected} (±1%)"
        );
    };
    oracle(&traffic, "avg_rps", 555.6, "avg_rps = 1e6·4·12/86400");
    oracle(&traffic, "peak_rps", 1388.9, "peak_rps = 555.6·2.5");
    oracle(&cdn, "origin_rps", 555.6, "origin = 1389·(1−0.6)");
    oracle(&lb, "out_gateway", 555.6, "gateway = весь origin");
    oracle(&lb, "out_auth", 83.3, "auth = 556·0.15");
    oracle(&lb, "out_feed", 333.3, "feed = 556·0.60");
    oracle(&lb, "out_media", 138.9, "media = 556·0.25");
    oracle(&feed, "db_qps", 80.0, "db = 333·0.8·0.3");
    oracle(&feed, "events", 333.3, "events = feed rps");
    oracle(&db, "replica_load", 40.0, "replica = 80·0.5");
    oracle(&queue, "consume_rate", 333.3, "consume = min(333, 2400)");

    // Проливание подтверждено транситивно: db_qps зависит от пролито-
    // го в feed.rps; upstream_rps шлюза — от пролитого out_gateway
    oracle(
        &gateway,
        "upstream_rps",
        555.6,
        "gateway.upstream_rps = пролитый rps",
    );

    // 6. Гейт A2 (FR-032 × FR-029, интеграция CP1): чистый эталон —
    // valid; 3 подсаженные ошибки (единица, дубль-вход, несуществующий
    // порт) → ровно 3 issue с кодом/нодой/ребром; исправление → чисто.
    // Подсадка — прямой мутацией рёбер (как hand-edited .canvas): MCP
    // edge_create такие рёбра отклоняет валидацией имён.
    {
        let report = dispatch(&mut scene, "graph_validate", "{}").expect("graph_validate");
        assert_eq!(report["valid"], true, "чистый эталон: {report}");
        assert_eq!(
            report["issues"].as_array().map(Vec::len),
            Some(0),
            "базовая линия без проблем: {report}"
        );

        // подсадка 1: E-UNIT — время (budget, ms) в Rate-параметр qps
        // кэша (у кэша нет других входов — ровно одна проблема)
        let mut unit_edge = Edge::new("planted-unit", &traffic, None, &cache, None);
        unit_edge.set_flow_kind(FlowKind::Value);
        unit_edge.from_output = Some("budget".to_owned());
        unit_edge.to_param = Some("qps".to_owned());
        scene.canvas.add_edge(unit_edge);
        // подсадка 2: E-DOUBLE-INPUT — второе ребро в gateway.rps
        let mut dbl_edge = Edge::new("planted-dbl", &lb, None, &gateway, None);
        dbl_edge.set_flow_kind(FlowKind::Value);
        dbl_edge.from_output = Some("out_feed".to_owned());
        dbl_edge.to_param = Some("rps".to_owned());
        scene.canvas.add_edge(dbl_edge);
        // подсадка 3: E-PORT-UNKNOWN — несуществующее имя выхода в
        // свободный параметр token_verify (без дубль-входа)
        let mut port_edge = Edge::new("planted-port", &traffic, None, &auth, None);
        port_edge.set_flow_kind(FlowKind::Value);
        port_edge.from_output = Some("no_such_output".to_owned());
        port_edge.to_param = Some("token_verify".to_owned());
        scene.canvas.add_edge(port_edge);

        let report =
            dispatch(&mut scene, "graph_validate", "{}").expect("graph_validate с подсадками");
        let issues = report["issues"].as_array().expect("issues");
        assert_eq!(issues.len(), 3, "ровно 3 issue: {report}");
        let by_code = |code: &str| {
            issues
                .iter()
                .find(|i| i["code"] == code)
                .unwrap_or_else(|| panic!("нет {code}: {report}"))
        };
        let unit_issue = by_code("E-UNIT");
        assert_eq!(unit_issue["edge_id"], "planted-unit");
        assert_eq!(unit_issue["node_id"], cache);
        let dbl_issue = by_code("E-DOUBLE-INPUT");
        assert_eq!(dbl_issue["edge_id"], "planted-dbl");
        assert_eq!(dbl_issue["node_id"], gateway);
        let port_issue = by_code("E-PORT-UNKNOWN");
        assert_eq!(port_issue["edge_id"], "planted-port");

        // исправление: снять все три подсадки — эталон снова чист
        scene
            .canvas
            .edges
            .retain(|edge| !edge.id.starts_with("planted-"));
        let report =
            dispatch(&mut scene, "graph_validate", "{}").expect("graph_validate после исправления");
        assert_eq!(report["valid"], true, "после исправления: {report}");
        assert_eq!(report["issues"].as_array().map(Vec::len), Some(0));
    }

    // 7. Демо-критерий CP1: правка DAU одним node_edit удваивает цепочку
    dispatch(
            &mut scene,
            "node_edit",
            &format!(
                r#"{{"id": "{traffic}", "text": "dau = 2000000\nsess = 4\nreq = 12\npeak = 2.5\nbudget = 45 ms\navg_rps = dau × sess × req × 1 rps / 86400\npeak_rps = avg_rps × peak"}}"#
            ),
        )
        .expect("node_edit dau");
    let doubled = dispatch(&mut scene, "flow_recalc", "{}").expect("recalc");
    for (node, output, expected, what) in [
        (&traffic, "peak_rps", 2777.8, "peak_rps ×2"),
        (&cdn, "origin_rps", 1111.1, "origin ×2"),
        (&feed, "db_qps", 160.0, "db_qps ×2"),
        (&queue, "consume_rate", 666.7, "consume ×2"),
    ] {
        let actual = doubled[node]["outputs"][output]["value"]
            .as_f64()
            .unwrap_or_else(|| panic!("{what}: нет значения — {doubled}"));
        assert!(
            close_1pct(actual, expected),
            "{what}: {actual} vs oracle {expected}"
        );
    }

    // 7. Валидация имён: неизвестный выход/параметр — ошибка вызова
    let err = dispatch(
            &mut scene,
            "edge_create",
            &format!(
                r#"{{"from": "{cdn}", "to": "{gateway}", "fromOutput": "no_such_output", "toParam": "rps", "kind": "value"}}"#
            ),
        )
        .expect_err("неизвестный выход — ошибка");
    assert!(err.contains("no_such_output"), "ошибка называет имя: {err}");
    let err = dispatch(
            &mut scene,
            "edge_create",
            &format!(
                r#"{{"from": "{cdn}", "to": "{gateway}", "fromOutput": "origin_rps", "toParam": "no_such_param", "kind": "value"}}"#
            ),
        )
        .expect_err("неизвестный параметр — ошибка");
    assert!(
        err.contains("no_such_param"),
        "ошибка называет параметр: {err}"
    );
    // взаимное исключение fromLine/fromOutput
    let err = dispatch(
            &mut scene,
            "edge_create",
            &format!(
                r#"{{"from": "{cdn}", "to": "{gateway}", "fromLine": 0, "fromOutput": "origin_rps", "kind": "value"}}"#
            ),
        )
        .expect_err("fromLine + fromOutput — ошибка схемы");
    assert!(err.contains("взаимно исключаются"), "{err}");
    // toParam на текстовой ноде-приёмнике — ошибка (нет параметров)
    let err = dispatch(
            &mut scene,
            "edge_create",
            &format!(
                r#"{{"from": "{cdn}", "to": "{traffic}", "fromOutput": "origin_rps", "toParam": "rps", "kind": "value"}}"#
            ),
        )
        .expect_err("toParam на текстовой ноде — ошибка");
    assert!(err.contains("текстовая"), "{err}");
}

fn graph_apply(scene: &mut SceneState, ops: &str) -> Result<serde_json::Value, String> {
    let params = format!(r#"{{"operations": {ops}}}"#);
    let params: serde_json::Value = serde_json::from_str(&params).expect("params — JSON");
    let registry = canvas_core::templates::TemplateRegistry::builtin();
    mcp_dispatch(scene, &registry, "graph_apply", &params)
}

/// Oracle-числа эталона №1 (ADR-0006): peak_rps ≈ 208.33 rps;
/// CDN W ≈ 34.29 ms (ρ 0.417); CDN origin ≈ 20.83 rps;
/// Gateway W ≈ 3.20 ms (latency_budget 5 − auth_overhead 2).
fn assert_close(actual: f64, expected: f64, what: &str) {
    let tolerance = (expected * 0.01).abs().max(1e-9);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} != {expected} (±1 %)"
    );
}

/// MCP e2e (FR-033 п.6а): один вызов graph_apply собирает мини-эталон
/// «Нагрузка → CDN → Gateway» — ноды, параметризация, value-рёбра с
/// адресацией портов (toParam/fromOutput); flow в ответе = oracle ±1 %.
#[test]
fn graph_apply_assembles_mini_reference_with_oracle() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga.canvas"));
    let undo_before = scene.undo_stack.len();

    let ops = r#"[
            {"op":"node_create_note","ref":"traffic","x":0,"y":0,"width":280,"text":"dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\navg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × peak"},
            {"op":"template_instantiate","ref":"cdn","template":"com.canvasdesk.cdn","x":360,"y":0,"params":{"cache_hit":0.9,"origin_latency":20}},
            {"op":"template_instantiate","ref":"gw","template":"com.canvasdesk.api-gateway","x":720,"y":0,"params":{"latency_budget":5,"auth_overhead":2}},
            {"op":"param_set","ref":"traffic","param":"dau","value":200000},
            {"op":"edge_create","fromRef":"traffic","toRef":"cdn","kind":"value","toParam":"rps"},
            {"op":"edge_create","fromRef":"cdn","toRef":"gw","kind":"value","fromOutput":"origin_rps","toParam":"rps"},
            {"op":"node_move","ref":"cdn","x":400,"y":40},
            {"op":"node_move","ref":"gw","x":760,"y":40},
            {"op":"node_create_note","ref":"cost","x":0,"y":320,"text":"cdn_cost = 50 $\n gw_cost = 36 $\n total = cdn_cost + gw_cost"},
            {"op":"edge_create","fromRef":"cost","toRef":"cdn"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("батч собирается");
    assert_eq!(out["ok"], true, "ответ: {out}");
    assert_eq!(
        out["created"].as_array().expect("created").len(),
        7,
        "4 ноды + 3 ребра"
    );
    assert_eq!(out["report"].as_array().expect("report").len(), 10);

    // ref-резолв: created содержит имена ref-ов и реальные id
    let created: Vec<&serde_json::Value> = out["created"].as_array().unwrap().iter().collect();
    assert!(created
        .iter()
        .any(|e| e["ref"] == "traffic" && e["node_id"].is_string()));
    assert!(created
        .iter()
        .any(|e| e["ref"] == "cdn" && e["node_id"].is_string()));
    assert!(created
        .iter()
        .any(|e| e["ref"] == "gw" && e["node_id"].is_string()));
    let edge_ops = created.iter().filter(|e| e["edge_id"].is_string()).count();
    assert_eq!(edge_ops, 3, "три ребра (2 value + 1 control)");

    // Undo = ровно ОДИН шаг на весь батч
    assert_eq!(
        scene.undo_stack.len(),
        undo_before + 1,
        "успешный батч — один undo-шаг"
    );

    // flow в ответе = oracle эталона №1 (ADR-0006) ±1 %
    let flow = &out["flow"];
    let traffic_node_id = created
        .iter()
        .find(|e| e["ref"] == "traffic")
        .and_then(|e| e["node_id"].as_str())
        .expect("traffic id");
    let cdn_node_id = created
        .iter()
        .find(|e| e["ref"] == "cdn")
        .and_then(|e| e["node_id"].as_str())
        .expect("cdn id");
    let gw_node_id = created
        .iter()
        .find(|e| e["ref"] == "gw")
        .and_then(|e| e["node_id"].as_str())
        .expect("gw id");

    assert_close(
        flow[traffic_node_id]["value"].as_f64().expect("peak_rps"),
        208.3333,
        "peak_rps ноды «Нагрузка»",
    );
    assert_eq!(flow[traffic_node_id]["unit"], "req/s");

    assert_close(
        flow[cdn_node_id]["value"].as_f64().expect("cdn W"),
        0.0342857,
        "CDN W (Erlang-C, ρ 0.417)",
    );
    assert_eq!(flow[cdn_node_id]["unit"], "sec");
    assert_close(
        flow[cdn_node_id]["outputs"]["origin_rps"]["value"]
            .as_f64()
            .expect("origin_rps"),
        20.8333,
        "именованный выход CDN.origin_rps (проливание rps = 208.33)",
    );

    assert_close(
        flow[gw_node_id]["value"].as_f64().expect("gw W"),
        0.0032,
        "Gateway W (rps пролито через fromOutput=origin)",
    );
    assert_eq!(flow[gw_node_id]["unit"], "sec");

    // Смета тоже в flow (last formula line)
    let cost_node_id = created
        .iter()
        .find(|e| e["ref"] == "cost")
        .and_then(|e| e["node_id"].as_str())
        .expect("cost id");
    assert_close(
        flow[cost_node_id]["value"].as_f64().expect("cost"),
        86.0,
        "смета мини-эталона",
    );

    // live-ревал: правка DAU одним node_edit меняет весь downstream
    // без правки связей/формул (инвариант ADR-0007)
    let dau_edit = serde_json::json!({
        "id": traffic_node_id,
        "text": TRAFFIC_TEXT.replacen("200000", "400000", 1),
    });
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_edit",
        &dau_edit,
    )
    .expect("node_edit dau");
    let flow = mcp_flow_v2(&scene.canvas);
    assert_close(
        flow[traffic_node_id]["value"].as_f64().expect("peak ×2"),
        416.6667,
        "peak_rps после удвоения DAU",
    );
    assert_close(
        flow[cdn_node_id]["outputs"]["origin_rps"]["value"]
            .as_f64()
            .expect("origin_rps ×2"),
        41.6667,
        "CDN.origin_rps после удвоения DAU — проливание живое",
    );
}

/// Атомарность (FR-033 п.5/п.6б): ошибка операции в середине батча →
/// {ok:false, op_index, code} и сериализация канваса байт-в-байт прежняя.
#[test]
fn graph_apply_mid_batch_error_is_atomic() {
    let mut scene = mcp_scene();
    let before = serde_json::to_string(&scene.canvas).expect("сериализация до");
    let undo_before = scene.undo_stack.len();

    // 3-я операция (index 2) ломается: edge на несуществующую ноду
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"первая"},
            {"op":"node_create_note","ref":"b","x":300,"y":0,"text":"вторая"},
            {"op":"edge_create","fromRef":"a","toRef":"ghost"},
            {"op":"node_create_note","ref":"c","x":600,"y":0,"text":"третья"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false, "ответ: {out}");
    assert_eq!(out["op_index"], 2, "сломанная операция — index 2");
    assert_eq!(out["code"], "E-NOT-FOUND");
    assert!(out["message"].as_str().expect("message").contains("ghost"));

    let after = serde_json::to_string(&scene.canvas).expect("сериализация после");
    assert_eq!(before, after, "канвас байт-в-байт прежний");
    assert_eq!(
        scene.undo_stack.len(),
        undo_before,
        "неудачный батч — ни одного undo-шага"
    );

    // Валидация портов тоже атомарна: неизвестный toParam
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"x"},
            {"op":"template_instantiate","ref":"cdn","template":"com.canvasdesk.cdn","x":300,"y":0},
            {"op":"edge_create","fromRef":"a","toRef":"cdn","kind":"value","toParam":"nope"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false);
    assert_eq!(out["code"], "E-PORT-UNKNOWN", "ответ: {out}");
    assert_eq!(
        serde_json::to_string(&scene.canvas).unwrap(),
        before,
        "канвас по-прежнему прежний"
    );
}

/// Ref-резолв (FR-033 п.5): edge на ref ноды, созданной РАНЬШЕ в этом же
/// батче, работает; ref будущей ноды — ошибка; дубликат ref — ошибка.
#[test]
fn graph_apply_ref_resolution_rules() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga2.canvas"));

    // Forward-ref: c ещё не создана на момент ребра
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"a"},
            {"op":"edge_create","fromRef":"a","toRef":"c"},
            {"op":"node_create_note","ref":"c","x":300,"y":0,"text":"c"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false, "ответ: {out}");
    assert_eq!(out["op_index"], 1);
    assert_eq!(out["code"], "E-NOT-FOUND");

    // Дубликат ref
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"a"},
            {"op":"node_create_note","ref":"a","x":300,"y":0,"text":"вторая a"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false, "ответ: {out}");
    assert_eq!(out["code"], "E-BAD-OP");

    // Смешанная адресация: ref для созданных, id — для существующих
    scene = mcp_scene();
    let ops = r#"[
            {"op":"node_create_note","ref":"new1","x":0,"y":0,"text":"новая"},
            {"op":"edge_create","fromRef":"new1","to":"n1","kind":"value"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], true, "ответ: {out}");
    assert_eq!(scene.canvas.nodes.len(), 4, "3 сцены + 1 новая");
    assert_eq!(scene.canvas.edges.len(), 2, "edge-1 + новое ребро");
    let new_edge = scene.canvas.edges.last().expect("ребро");
    assert_eq!(new_edge.to_node, "n1", "id существующей ноды зарезолвен");
    assert_eq!(new_edge.flow_kind(), canvas_core::flow::FlowKind::Value);
}

/// param_set (FR-033 п.4/п.5): правит ровно одну строку «param = value
/// unit» (текст + снапшот шаблона), остальные строки нетронуты;
/// несуществующий параметр — ошибка операции.
#[test]
fn graph_apply_param_set_edits_single_line() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga4.canvas"));
    let ops = r#"[
            {"op":"template_instantiate","ref":"gw","template":"com.canvasdesk.api-gateway","x":0,"y":0},
            {"op":"param_set","ref":"gw","param":"latency_budget","value":5}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("батч");
    assert_eq!(out["ok"], true, "ответ: {out}");
    let node = &scene.canvas.nodes[0];
    let text = node.text.as_deref().expect("текст");
    let lines: Vec<&str> = text.split('\n').collect();
    assert!(
        lines.contains(&"latency_budget = 5 ms"),
        "строка заменена с единицей снапшота: {text:?}"
    );
    assert!(
        lines.contains(&"rps = 20 rps"),
        "соседние строки нетронуты: {text:?}"
    );
    // Снапшот синхронизирован — расчёт видит новое значение
    let tpl = node.template().expect("шаблон");
    assert_eq!(tpl.params.get("latency_budget").expect("param").num, 5.0);
    // ref живёт только внутри батча: следующие батчи адресуют по id
    let gw_id = out["created"][0]["node_id"]
        .as_str()
        .expect("id")
        .to_owned();

    // Единица из операции переопределяет снапшот
    let ops = format!(
        r#"[{{"op":"param_set","id":"{gw_id}","param":"latency_budget","value":0.005,"unit":"sec"}}]"#
    );
    let out = graph_apply(&mut scene, &ops).expect("батч");
    assert_eq!(out["ok"], true, "ответ: {out}");
    let text = scene.canvas.nodes[0].text.as_deref().expect("текст");
    assert!(
        text.contains("latency_budget = 0.005 sec"),
        "единица из операции: {text:?}"
    );

    // Параметра нет в тексте — ошибка (консервативно, без append)
    let ops = format!(r#"[{{"op":"param_set","id":"{gw_id}","param":"servers","value":4}}]"#);
    let out = graph_apply(&mut scene, &ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false, "ответ: {out}");
    assert_eq!(out["code"], "E-PARAM-UNKNOWN");
}

/// Лимиты схемы (FR-033 п.1/п.5): 257-я операция и > 128 нод — ошибки
/// уровня вызова (isError), а не {ok:false}.
#[test]
fn graph_apply_enforces_limits() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga5.canvas"));

    // 257 node_move-операций по существующей ноде n1
    let moves: Vec<String> = (0..257)
        .map(|i| format!(r#"{{"op":"node_move","id":"n1","x":{i},"y":0}}"#))
        .collect();
    let ops = format!("[{}]", moves.join(","));
    let err = graph_apply(&mut scene, &ops).expect_err("лимит операций");
    assert!(err.contains("256"), "{err}");

    // 129 node_create_note — сверх лимита нод
    let notes: Vec<String> = (0..129)
        .map(|i| format!(r#"{{"op":"node_create_note","x":0,"y":{i},"text":"n{i}"}}"#))
        .collect();
    let ops = format!("[{}]", notes.join(","));
    let err = graph_apply(&mut scene, &ops).expect_err("лимит нод");
    assert!(err.contains("128"), "{err}");
    assert!(
        scene.canvas.nodes.is_empty(),
        "канвас не изменился при ошибке лимита"
    );
}

/// Undo/redo после успешного батча (FR-033 п.6в): Ctrl+Z откатывает всю
/// сборку одним шагом, Ctrl+Y возвращает.
#[test]
fn graph_apply_undo_reverts_whole_batch() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga6.canvas"));
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"= 5"},
            {"op":"template_instantiate","ref":"gw","template":"com.canvasdesk.api-gateway","x":300,"y":0},
            {"op":"edge_create","fromRef":"a","toRef":"gw","kind":"value","toParam":"rps"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("батч");
    assert_eq!(out["ok"], true);
    assert_eq!(scene.canvas.nodes.len(), 2);
    assert_eq!(scene.canvas.edges.len(), 1);

    // Ctrl+Z: вся сборка исчезла одним шагом
    let before = scene.take_undo().expect("undo-шаг есть");
    scene.canvas = before;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    assert!(scene.canvas.nodes.is_empty(), "сборка откатилась целиком");
    assert!(scene.canvas.edges.is_empty(), "рёбра ушли вместе с нодами");

    // Ctrl+Y: сборка вернулась целиком
    let after = scene.take_redo().expect("redo-шаг есть");
    scene.canvas = after;
    scene.spatial = SpatialIndex::build(&scene.canvas);
    assert_eq!(scene.canvas.nodes.len(), 2);
    assert_eq!(scene.canvas.edges.len(), 1);
}

/// value-цикл внутри батча — ошибка операции E-CYCLE с участниками;
/// канвас не меняется.
#[test]
fn graph_apply_rejects_value_cycle() {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ga7.canvas"));
    let ops = r#"[
            {"op":"node_create_note","ref":"a","x":0,"y":0,"text":"= 1"},
            {"op":"node_create_note","ref":"b","x":300,"y":0,"text":"= 2"},
            {"op":"edge_create","fromRef":"a","toRef":"b","kind":"value"},
            {"op":"edge_create","fromRef":"b","toRef":"a","kind":"value"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("инструмент отвечает");
    assert_eq!(out["ok"], false, "ответ: {out}");
    assert_eq!(out["op_index"], 3);
    assert_eq!(out["code"], "E-CYCLE");
    assert!(
        out["message"].as_str().expect("msg").contains("→"),
        "участники цикла в сообщении: {out}"
    );
    assert!(scene.canvas.nodes.is_empty(), "канвас не изменился");
}

/// Сборка мини-эталона №1 (ADR-0006: Нагрузка → CDN → Gateway) и карта
/// id нод по ref-ам батча.
fn reference_scene() -> (SceneState, String, String) {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/ab1.canvas"));
    let ops = r#"[
            {"op":"node_create_note","ref":"traffic","x":0,"y":0,"width":280,"text":"dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\navg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × peak"},
            {"op":"template_instantiate","ref":"cdn","template":"com.canvasdesk.cdn","x":360,"y":0,"params":{"cache_hit":0.9,"origin_latency":20}},
            {"op":"template_instantiate","ref":"gw","template":"com.canvasdesk.api-gateway","x":720,"y":0,"params":{"latency_budget":5,"auth_overhead":2}},
            {"op":"edge_create","fromRef":"traffic","toRef":"cdn","kind":"value","toParam":"rps"},
            {"op":"edge_create","fromRef":"cdn","toRef":"gw","kind":"value","fromOutput":"origin_rps","toParam":"rps"}
        ]"#;
    let out = graph_apply(&mut scene, ops).expect("батч эталона");
    assert_eq!(out["ok"], true, "ответ: {out}");
    let created = out["created"].as_array().expect("created").clone();
    let id_of = |reference: &str| -> String {
        created
            .iter()
            .find(|e| e["ref"] == reference)
            .and_then(|e| e["node_id"].as_str())
            .expect("id ноды эталона")
            .to_owned()
    };
    let cdn = id_of("cdn");
    let gw = id_of("gw");
    (scene, cdn, gw)
}

/// Запись узла из отчёта analyze_bottlenecks по id.
fn node_report(report: &serde_json::Value, node_id: &str) -> serde_json::Value {
    report["nodes"]
        .as_array()
        .expect("массив nodes")
        .iter()
        .find(|entry| entry["id"] == node_id)
        .cloned()
        .unwrap_or_else(|| panic!("нода {node_id} в отчёте: {report}"))
}

/// CP5 (FR-016, гейт B1): эталон №1 под нагрузкой — узкие места видны
/// БЕЗ чтения чисел в нодах: анализ отдаёт те же флаги/бейджи, что
/// рисует канвас. Базовая линия — здоровый ландшафт (CDN ρ 0.417 —
/// узкое место №1, но ниже warn 0.7); рост DAU ×2 → Warn (ρ 0.833);
/// рост ×5.35 → Overload (ρ 2.23 — ветка C эталона №2 ADR-0006).
/// Инструмент — чтение: канвас и undo-история не затрагиваются.
#[test]
fn analyze_bottlenecks_reference_and_growth() {
    let (mut scene, cdn, gw) = reference_scene();

    // --- 1. Базовая линия: здоровый ландшафт (ADR-0006 эталон №1) ---
    let undo_before = scene.undo_stack.len();
    let canvas_before = serde_json::to_string(&scene.canvas).expect("сериализация до");
    let report = dispatch(&mut scene, "analyze_bottlenecks", "{}").expect("analyze_bottlenecks");
    // Чтение: канвас байт-в-байт и undo не тронуты
    assert_eq!(
        serde_json::to_string(&scene.canvas).expect("сериализация после"),
        canvas_before,
        "анализ не мутирует канвас"
    );
    assert_eq!(scene.undo_stack.len(), undo_before, "undo не растёт");

    // Только queue-ноды в отчёте: «Нагрузка» (Rate) анализа не даёт
    let nodes = report["nodes"].as_array().expect("массив");
    assert_eq!(nodes.len(), 2, "cdn + gw: {report}");
    // Пороги — в ответе (контракт для агента)
    assert_eq!(report["thresholds"]["warn_util"], 0.7);
    assert_eq!(report["thresholds"]["critical_util"], 0.9);
    assert_eq!(report["thresholds"]["warn_wait_sec"], 0.1);

    let cdn_report = node_report(&report, &cdn);
    assert_eq!(
        cdn_report["severity"], "none",
        "ρ 0.417 < 0.7: {cdn_report}"
    );
    assert_close(
        cdn_report["utilization"].as_f64().expect("ρ cdn"),
        0.4167,
        "CDN ρ (named-выход utilization)",
    );
    assert_close(
        cdn_report["wait_sec"].as_f64().expect("W cdn"),
        0.0342857,
        "CDN W = 34.29 ms (ниже warn 100 ms)",
    );
    assert_eq!(
        cdn_report["badge"], "42% · W: 34 ms",
        "бейдж канваса: {cdn_report}"
    );
    let gw_report = node_report(&report, &gw);
    assert_eq!(gw_report["severity"], "none");
    assert_close(
        gw_report["utilization"].as_f64().expect("ρ gw"),
        0.0625,
        "GW ρ = 20.83/333.3",
    );

    // --- 2. Рост DAU ×2 (400k): CDN ρ = 0.833 → Warn (жёлтая рамка) ---
    // node_update_text — мутирующий путь с полным пересчётом
    // (node_edit с text без expr пересчёт не поднимает — ленивость
    // футера CR-012; агенту достаточно свежего отчёта инструмента).
    let dau_edit = serde_json::json!({
        "id": node_id_of(&scene, "dau = 200000").expect("нода «Нагрузка»"),
        "text": TRAFFIC_TEXT.replacen("200000", "400000", 1),
    });
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_update_text",
        &dau_edit,
    )
    .expect("node_update_text dau ×2");
    let report = dispatch(&mut scene, "analyze_bottlenecks", "{}").expect("analyze_bottlenecks ×2");
    let cdn_report = node_report(&report, &cdn);
    assert_eq!(
        cdn_report["severity"], "warn",
        "ρ 0.833 ∈ [0.7, 0.9): {cdn_report}"
    );
    assert_close(
        cdn_report["utilization"].as_f64().expect("ρ cdn ×2"),
        0.8333,
        "CDN ρ после ×2",
    );
    assert_eq!(
        cdn_report["badge"], "83% · W: 120 ms",
        "бейдж Warn: {cdn_report}"
    );

    // --- 3. Рост DAU ×5.35 (ветка C эталона №2): CDN ρ = 2.23 → Overload ---
    let dau_edit = serde_json::json!({
        "id": node_id_of(&scene, "dau = 400000").expect("нода «Нагрузка»"),
        "text": TRAFFIC_TEXT.replacen("200000", "1070000", 1),
    });
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_update_text",
        &dau_edit,
    )
    .expect("node_update_text dau ×5.35");
    let report =
        dispatch(&mut scene, "analyze_bottlenecks", "{}").expect("analyze_bottlenecks ×5.35");
    let cdn_report = node_report(&report, &cdn);
    assert_eq!(
        cdn_report["severity"], "overload",
        "ρ 2.23 ≥ 1 — тёмно-красная рамка + OVERLOAD: {cdn_report}"
    );
    assert_close(
        cdn_report["utilization"].as_f64().expect("ρ cdn ×5.35"),
        2.2292,
        "CDN ρ = 2.23 (ADR-0006 ветка C)",
    );
    assert_eq!(cdn_report["badge"], "OVERLOAD 223%", "бейдж: {cdn_report}");
    // W при перегрузке аналитически ∞ — в отчёте его нет
    assert!(cdn_report.get("wait_sec").is_none(), "W нет: {cdn_report}");
    // GW остаётся здоровым: origin 111.46 rps при μ 333 rps
    let gw_report = node_report(&report, &gw);
    assert_eq!(gw_report["severity"], "none", "GW здоров: {gw_report}");
    assert_close(
        gw_report["utilization"].as_f64().expect("ρ gw"),
        0.3344,
        "GW ρ = 111.46/333.3",
    );

    // --- 4. Runtime-кэш сцены синхронен с отчётом (рендер читает его) ---
    let cdn_flags = scene.analysis.get(&cdn).expect("флаги CDN в сцене");
    assert_eq!(cdn_flags.severity, canvas_core::AnalysisSeverity::Overload);
    assert!((cdn_flags.utilization.unwrap() - 2.2292).abs() < 2.2292 * 0.01);
    assert!(analyze::has_risk(&scene.analysis), "оверлей авто-включится");
}

/// Поиск id ноды по подстроке текста (нода «Нагрузка» эталона).
fn node_id_of(scene: &SceneState, needle: &str) -> Option<String> {
    scene
        .canvas
        .nodes
        .iter()
        .find(|node| {
            node.text
                .as_deref()
                .is_some_and(|text| text.contains(needle))
        })
        .map(|node| node.id.clone())
}

// --- FR-037 MW1 (ребейз): тесты upstream CP6 (what-if) и FR-029
// (проливания), перенесённые сюда вместе с MCP-слоем (имена/ассерты
// без изменений; как и у остальных — камера ушла из диспетчера) ---
/// MCP-текст: literal `\n` (два символа — двойное экранирование от
/// ИИ-агентов) нормализуется в реальные переводы строк: нода получает
/// построчный Numi-лист, а не одну строку с видимым эскейпом.
#[test]
fn mcp_text_normalizes_literal_newlines() {
    let mut scene = SceneState::new(
        Canvas::default(),
        PathBuf::from("target/tmp/mcp_text_nl.canvas"),
    );
    let created = dispatch(
        &mut scene,
        "node_create_note",
        &serde_json::json!({ "x": 0, "y": 0, "text": "rps = 1389 rps\\ncache_hit = 0.6" })
            .to_string(),
    )
    .expect("node_create_note");
    let id = created["id"].as_str().expect("id ноды");
    let node = scene.canvas.node(id).expect("нода");
    assert_eq!(
        node.text.as_deref(),
        Some("rps = 1389 rps\ncache_hit = 0.6"),
        "literal \\n стал реальным переводом строки"
    );
    let lines = scene
        .expr_line_results
        .get(id)
        .expect("построчные результаты есть");
    assert_eq!(lines.len(), 2, "две формульные строки после нормализации");
    // node_update_text — тот же путь нормализации.
    dispatch(
        &mut scene,
        "node_update_text",
        &serde_json::json!({ "id": id, "text": "a = 1\\nb = 2" }).to_string(),
    )
    .expect("node_update_text");
    assert_eq!(
        scene.canvas.node(id).expect("нода").text.as_deref(),
        Some("a = 1\nb = 2")
    );
}

/// FR-029 (визуализация проливания): recompute_flow заполняет
/// param_spills — параметр, строка присваивания, заголовок источника и
/// значение РЕБРА (пролитое), а не локальный литерал строки; текст «как
/// на карточке» подменяет присваивание подписью источника; MCP
/// flow_recalc отдаёт spilled-инфо агенту.
#[test]
fn recompute_fills_param_spills_with_edge_value() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text(
        "traffic",
        "Traffic Profile\npeak_rps = 1388.89 rps",
        0.0,
        0.0,
    ));
    canvas.nodes.push(Node::text(
        "cdn",
        "rps = 100 rps\ncache_hit = 0.6",
        300.0,
        0.0,
    ));
    let mut edge = Edge::new("e1", "traffic", None, "cdn", None);
    edge.set_flow_kind(FlowKind::Value);
    edge.to_param = Some("rps".to_owned());
    edge.from_output = Some("peak_rps".to_owned());
    canvas.add_edge(edge);
    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/spills.canvas"));
    let spills = scene.param_spills.get("cdn").expect("spills CDN");
    assert_eq!(spills.len(), 1);
    let spill = &spills[0];
    assert_eq!(spill.param, "rps");
    assert_eq!(spill.line, Some(0));
    assert_eq!(spill.from_label, "Traffic Profile");
    assert_eq!(spill.from_output, Some("peak_rps".to_owned()));
    assert_eq!(
        spill.value,
        Some("1\u{a0}388.89 rps".to_owned()),
        "бейдж — пролитое значение ребра, а не локальный литерал 100 rps"
    );
    // Текст как на карточке: присваивание заменено подписью источника.
    let display = display_body_text(scene.canvas.node("cdn").expect("cdn"), &scene.param_spills);
    assert_eq!(display, "rps ← Traffic Profile · peak_rps\ncache_hit = 0.6");
    // Без проливания — исходный текст.
    let plain = display_body_text(
        scene.canvas.node("traffic").expect("traffic"),
        &scene.param_spills,
    );
    assert_eq!(plain, "Traffic Profile\npeak_rps = 1388.89 rps");
    // MCP flow_recalc: spilled — источник, адресация и значение.
    let result = dispatch(&mut scene, "flow_recalc", "{}").expect("flow_recalc");
    let spilled = &result["cdn"]["spilled"];
    assert_eq!(spilled["rps"]["from"], "traffic");
    assert_eq!(spilled["rps"]["fromOutput"], "peak_rps");
    assert_eq!(spilled["rps"]["value"], 1388.89);
    assert_eq!(spilled["rps"]["unit"], "rps");
}

/// FR-017 (CP6) MCP-сценарий «Проверка»: 3 calc-ноды A→B→C;
/// `whatif_set_override(A, 0, "a = 20")` → дельты {A: 5→20 (+15),
/// B: 10→40 (+30), C: 11→41 (+30)}; сессия без Apply не мутирует
/// `.canvas`; `whatif_apply()` → строка A в тексте = `20`; undo (один
/// шаг) — база восстановлена.
#[test]
fn mcp_whatif_override_apply_undo() {
    let mut scene = mcp_scene();
    for (x, text) in [
        (0.0, "a = 5"),
        (300.0, "b = $in × 2"),
        (600.0, "c = $in + 1"),
    ] {
        dispatch(
            &mut scene,
            "node_create_note",
            &format!(r#"{{"x":{x},"y":0,"text":"{text}"}}"#),
        )
        .expect("нода");
    }
    // id генерируются автоматически (note-N) — найдём по тексту
    let id = |scene: &SceneState, prefix: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| {
                n.text
                    .as_deref()
                    .map(|t| t.starts_with(prefix))
                    .unwrap_or(false)
            })
            .map(|n| n.id.clone())
            .expect("нода сценария")
    };
    let (id_a, id_b, id_c) = (id(&scene, "a = "), id(&scene, "b = "), id(&scene, "c = "));
    for (from, to) in [(&id_a, &id_b), (&id_b, &id_c)] {
        let edge_id = dispatch(
            &mut scene,
            "edge_create",
            &format!(r#"{{"from":"{from}","to":"{to}"}}"#),
        )
        .expect("edge")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        dispatch(
            &mut scene,
            "flow_set_kind",
            &format!(r#"{{"id":"{edge_id}","kind":"value"}}"#),
        )
        .expect("value-ребро");
    }
    // Именованный сценарий (персистентен — мутация canvas, undo-шаг)
    dispatch(&mut scene, "whatif_scenario_create", r#"{"name":"S1"}"#).expect("сценарий");
    let hash_before = scene.canvas.to_json().expect("сериализация");

    // Подмена строки 0 ноды A — runtime: canvas не мутируется
    let out = dispatch(
        &mut scene,
        "whatif_set_override",
        &format!(r#"{{"node_id":"{id_a}","line":0,"expr":"a = 20"}}"#),
    )
    .expect("set_override");
    assert_eq!(out["scenario"], "S1");
    assert_eq!(
        scene.canvas.to_json().expect("сериализация"),
        hash_before,
        "подмена runtime-only (инвариант 2)"
    );

    // Дельты — эталон «Проверка» FR-017
    let deltas = dispatch(&mut scene, "whatif_deltas", "{}").expect("deltas");
    let key_a = format!("{id_a}:0");
    let key_b = format!("{id_b}:value");
    let key_c = format!("{id_c}:value");
    assert_eq!(deltas["deltas"][key_a.as_str()]["base"], "5");
    assert_eq!(deltas["deltas"][key_a.as_str()]["whatif"], "20");
    assert_eq!(deltas["deltas"][key_a.as_str()]["delta"], "+15");
    assert_eq!(deltas["deltas"][key_b.as_str()]["base"], "10");
    assert_eq!(deltas["deltas"][key_b.as_str()]["whatif"], "40");
    assert_eq!(deltas["deltas"][key_b.as_str()]["delta"], "+30");
    assert_eq!(deltas["deltas"][key_c.as_str()]["base"], "11");
    assert_eq!(deltas["deltas"][key_c.as_str()]["whatif"], "41");
    assert_eq!(deltas["deltas"][key_c.as_str()]["delta"], "+30");

    // Переключение База ↔ S1 — runtime, дельты появляются/исчезают
    dispatch(&mut scene, "whatif_scenario_activate", r#"{"name":"База"}"#).expect("база");
    let deltas = dispatch(&mut scene, "whatif_deltas", "{}").expect("deltas");
    assert!(
        deltas["deltas"].as_object().expect("объект").is_empty(),
        "на базе дельт нет: {deltas}"
    );
    dispatch(&mut scene, "whatif_scenario_activate", r#"{"name":"S1"}"#).expect("S1");
    assert_eq!(
        scene.canvas.to_json().expect("сериализация"),
        hash_before,
        "переключение сценариев файл не трогает (инвариант 2)"
    );

    // Apply: подмена уходит в persisted-текст, сценарий удалён (Q6b)
    let out = dispatch(&mut scene, "whatif_apply", "{}").expect("apply");
    assert_eq!(out["applied"], 1);
    let text = scene.canvas.node(&id_a).expect("A").text.clone().unwrap();
    assert_eq!(text.lines().next().expect("строка 0"), "a = 20");
    assert!(
        scene
            .canvas
            .extra
            .get("canvasdesk")
            .and_then(|ext| ext.get("whatif"))
            .is_none(),
        "применённый сценарий удалён из canvasdesk.whatif"
    );
    // Undo (один шаг, FR-006) — база восстановлена
    let before = scene.take_undo().expect("undo-шаг apply");
    scene.canvas = before;
    scene.scenarios = canvas_core::whatif::scenarios_from_canvas(&scene.canvas);
    if scene
        .active_scenario
        .is_some_and(|i| i >= scene.scenarios.len())
    {
        scene.active_scenario = None;
    }
    scene.recompute_flow();
    let text = scene.canvas.node(&id_a).expect("A").text.clone().unwrap();
    assert_eq!(
        text.lines().next().expect("строка 0"),
        "a = 5",
        "undo → база"
    );
    assert_eq!(
        whatif_delta_rows(&scene).len(),
        0,
        "после undo активных дельт нет"
    );
}

/// Текст эталонной ноды «Нагрузка» (эталон №1 ADR-0006: dau=200000,
/// 3 сессии × 10 req → 69.44 rps avg, peak ×3 → 208.33 rps).
const TRAFFIC_TEXT: &str = "dau = 200000\nsess = 3\nreq = 10 req\npeak = 3\navg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × peak";

/// M8/W3 (wasm-port §6): SceneState.save_now пишет через инъектированное
/// хранилище (натив — FsCanvasStorage, тест — MemStorage); содержимое
/// читается обратно тем же хранилищем.
#[test]
fn save_now_goes_through_injected_storage() {
    use canvas_core::CanvasStorage;
    use std::sync::Arc;

    let storage = Arc::new(canvas_core::MemStorage::new());
    let path = PathBuf::from("mem://w3-storage-test.canvas");
    let canvas = Canvas::default();
    let mut scene = crate::SceneState::with_storage(canvas, path.clone(), storage.clone());
    assert!(scene.save_now(), "сохранение в MemStorage успешно");
    assert_eq!(storage.len(), 1, "хранилище получило файл");
    let loaded = storage.load(&path).expect("файл читается из хранилища");
    assert!(loaded.nodes.is_empty(), "roundtrip пустой сцены");
    // Повторный сейв: dirty_since сброшен — autosave_if_due не пишет.
    assert!(!scene.autosave_if_due(), "не dirty — записи нет");
}

/// CR-016: автоимя сценария (пустое имя) — первый свободный номер, а не
/// len+1: при непоследовательных именах («Сценарий 2», «Сценарий 3») len+1
/// коллидирует с существующим и чип «+» падает с «уже существует».
#[test]
fn whatif_autoname_picks_first_free_slot() {
    let mut scene = mcp_scene();
    scene
        .whatif_create_scenario("Сценарий 2")
        .expect("явное имя");
    // Автоимя занимает первый свободный номер, а не len+1.
    let index = scene
        .whatif_create_scenario("")
        .expect("автоимя без коллизии");
    assert_eq!(scene.scenarios[index].name, "Сценарий 1");
    // Явный дубль — по-прежнему ошибка (контракт MCP).
    assert!(
        scene.whatif_create_scenario("Сценарий 2").is_err(),
        "явная коллизия имени — Err"
    );
    // После удаления автоимя занимает освободившийся номер.
    scene.whatif_delete_scenario(index);
    let again = scene.whatif_create_scenario("").expect("повторное автоимя");
    assert_eq!(scene.scenarios[again].name, "Сценарий 1");
}

// --- FR-050 этап A: Н4 fail-fast дубль-входов; Р-4 кэш авто-строк ---

/// FR-050 Н4: fail-fast дубль-входов — прямое `edge_create` со вторым
/// value-ребром в занятый `toParam` отклоняется немедленно с кодом
/// E-DOUBLE-INPUT; канвас не изменился (undo-шаг не добавлен).
#[test]
fn mcp_edge_create_double_input_fail_fast() {
    let mut scene = mcp_scene();
    // Сборка: заметки-источники + шаблонный приёмник + первое ребро в rps
    // (value-ребро без fromOutput — узловое значение заметки).
    let ops = r#"[
        {"op": "node_create_note", "ref": "src", "x": 0, "y": 0, "text": "v = 100 rps"},
        {"op": "node_create_note", "ref": "alt", "x": 0, "y": 200, "text": "w = 200 rps"},
        {"op": "template_instantiate", "ref": "gw", "template": "com.canvasdesk.api-gateway",
         "x": 400, "y": 0, "params": {"latency_budget": 5, "auth_overhead": 2}},
        {"op": "edge_create", "fromRef": "src", "toRef": "gw", "kind": "value",
         "toParam": "rps"}
    ]"#;
    let report = graph_apply(&mut scene, ops).expect("сборка");
    assert_eq!(report["ok"], true, "сборка чистая: {report}");
    let gw = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.template().is_some())
        .map(|n| n.id.clone())
        .expect("шаблонная нода");
    let alt = scene
        .canvas
        .nodes
        .iter()
        .find(|n| {
            n.text
                .as_deref()
                .map(|t| t.contains("w = 200"))
                .unwrap_or(false)
        })
        .map(|n| n.id.clone())
        .expect("нода alt");
    let undo_len = scene.undo_stack.len();
    // Второе ребро в тот же gw.rps — fail-fast E-DOUBLE-INPUT
    let err = dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{alt}", "to": "{gw}", "fromOutput": "w", "toParam": "rps", "kind": "value"}}"#
        ),
    )
    .expect_err("дубль-вход отклонён");
    assert!(err.contains("E-DOUBLE-INPUT"), "ошибка называет код: {err}");
    assert!(err.contains("rps"), "ошибка называет параметр: {err}");
    // Канвас не изменился: одно ребро в rps, undo не рос
    let into_rps = scene
        .canvas
        .edges
        .iter()
        .filter(|e| e.to_param.as_deref() == Some("rps") && e.to_node == gw)
        .count();
    assert_eq!(into_rps, 1, "в параметре осталось одно ребро");
    assert_eq!(
        scene.undo_stack.len(),
        undo_len,
        "неудачный вызов не оставил undo-шаг"
    );
    // Другой параметр того же приёмника — допустимо (не дубль)
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{alt}", "to": "{gw}", "toParam": "auth_overhead", "kind": "value"}}"#
        ),
    )
    .expect("другой параметр — не дубль");
}

/// FR-050 Н4: graph_apply[edge_create] — дубль-вход (два edge_create в
/// один toParam в ОДНОМ батче) отклоняется операцией с кодом
/// E-DOUBLE-INPUT, батч атомарно откатывается; замена источника — явная
/// пара edge_delete + edge_create в одном батче (проходит: ровно одно
/// ребро в параметре); edge_delete несуществующего ребра — E-NOT-FOUND.
#[test]
fn graph_apply_double_input_atomic_and_replacement() {
    let mut scene = mcp_scene();
    // Два edge_create в один toParam в ОДНОМ батче — пятая операция
    // падает E-DOUBLE-INPUT, весь батч откатывается (атомарность FR-033)
    let dup = r#"[
        {"op": "node_create_note", "ref": "src", "x": 0, "y": 0, "text": "v = 100 rps"},
        {"op": "node_create_note", "ref": "alt", "x": 0, "y": 200, "text": "w = 200 rps"},
        {"op": "template_instantiate", "ref": "gw", "template": "com.canvasdesk.api-gateway",
         "x": 400, "y": 0, "params": {"latency_budget": 5, "auth_overhead": 2}},
        {"op": "edge_create", "fromRef": "src", "toRef": "gw", "kind": "value",
         "toParam": "rps", "ref": "e_src"},
        {"op": "edge_create", "fromRef": "alt", "toRef": "gw", "kind": "value",
         "toParam": "rps", "ref": "e_alt"}
    ]"#;
    let report = graph_apply(&mut scene, dup).expect("отчёт об ошибке операции");
    assert_eq!(report["ok"], false, "батч отклонён: {report}");
    assert_eq!(report["op_index"], 4, "падает операция дубля: {report}");
    assert_eq!(report["code"], "E-DOUBLE-INPUT", "код устойчив: {report}");
    assert!(
        report["message"].as_str().expect("message").contains("rps"),
        "ошибка называет параметр: {report}"
    );
    assert!(
        scene.canvas.edges.iter().all(|e| e.to_param.is_none()),
        "атомарный откат: ни одного адресованного ребра"
    );
    assert_eq!(scene.canvas.nodes.len(), 3, "созданные ноды откатились");
    // Замена источника: create + delete + create в одном батче — проходит
    let replace = r#"[
        {"op": "node_create_note", "ref": "src", "x": 0, "y": 0, "text": "v = 100 rps"},
        {"op": "node_create_note", "ref": "alt", "x": 0, "y": 200, "text": "w = 200 rps"},
        {"op": "template_instantiate", "ref": "gw", "template": "com.canvasdesk.api-gateway",
         "x": 400, "y": 0, "params": {"latency_budget": 5, "auth_overhead": 2}},
        {"op": "edge_create", "fromRef": "src", "toRef": "gw", "kind": "value",
         "toParam": "rps", "ref": "e_first"},
        {"op": "edge_delete", "id": "e_first"},
        {"op": "edge_create", "fromRef": "alt", "toRef": "gw", "kind": "value",
         "toParam": "rps"}
    ]"#;
    let report = graph_apply(&mut scene, replace).expect("замена источника");
    assert_eq!(report["ok"], true, "явная замена допустима: {report}");
    let gw = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.template().is_some())
        .map(|n| n.id.clone())
        .expect("шаблонная нода");
    assert_eq!(
        scene
            .canvas
            .edges
            .iter()
            .filter(|e| e.to_param.as_deref() == Some("rps") && e.to_node == gw)
            .count(),
        1,
        "источник заменён — ровно одно ребро в параметре"
    );
    // edge_delete несуществующего ребра — операция падает E-NOT-FOUND
    let missing = r#"[{"op": "edge_delete", "id": "no-such-edge"}]"#;
    let report = graph_apply(&mut scene, missing).expect("отчёт");
    assert_eq!(report["ok"], false, "несуществующее ребро: {report}");
    assert_eq!(report["code"], "E-NOT-FOUND", "{report}");
}

/// FR-050 Р-4 (Н10-а): recompute_flow заполняет кэш авто-строк —
/// производные данные пересчёта для рендера (этап D): путь «Объект.Поле»,
/// значение из активного пересчёта, слот не читается формулой приёмника.
/// Прямое edge_create (текстовый исток с fromOutput — валидация имён
/// прямого инструмента знает переменные Numi-листа).
#[test]
fn scene_auto_rows_cache_populated() {
    let mut scene = mcp_scene();
    let ops = r#"[
        {"op": "node_create_note", "ref": "traffic", "x": 0, "y": 0,
         "text": "Трафик\npeak_rps = 1389 rps"},
        {"op": "node_create_note", "ref": "gateway", "x": 400, "y": 0,
         "text": "заметка без формулы"}
    ]"#;
    let report = graph_apply(&mut scene, ops).expect("сборка");
    assert_eq!(report["ok"], true, "сборка чистая: {report}");
    let find = |scene: &SceneState, text: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref().map(|t| t.contains(text)).unwrap_or(false))
            .map(|n| n.id.clone())
            .expect(text)
    };
    let traffic = find(&scene, "Трафик");
    let gateway = find(&scene, "заметка без формулы");
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{traffic}", "to": "{gateway}", "fromOutput": "peak_rps", "kind": "value"}}"#
        ),
    )
    .expect("value-ребро");
    let rows = scene.auto_rows.get(&gateway).expect("кэш заполнен");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "Трафик.peak_rps");
    assert_eq!(rows[0].field, "peak_rps");
    assert_eq!(rows[0].slot, 0);
    let value = rows[0].value.as_ref().expect("значение пролито");
    assert!(
        value.to_string().contains("1\u{a0}389"),
        "значение: {value}"
    );
    // Удаление ребра → пересчёт → строка исчезла
    let edge_id = scene
        .canvas
        .edges
        .iter()
        .find(|e| e.to_node == gateway)
        .map(|e| e.id.clone())
        .expect("ребро");
    dispatch(
        &mut scene,
        "edge_delete",
        &format!(r#"{{"id": "{edge_id}"}}"#),
    )
    .expect("edge_delete");
    assert!(
        !scene.auto_rows.contains_key(&gateway),
        "ребра нет — авто-строка исчезла"
    );
}

/// PRD-0008 (Q5 v2 — «подтянуть MCP под обновления», запрос владельца
/// 2026-09-22): schemes_list — реестр галереи виден агенту: те же пакеты,
/// что в галерее (Ctrl+T), с размерами графа; чтение — канвас
/// и undo не тронуты. Расширение каталога (аудит 2026-09-25): 6 → 10.
#[test]
fn mcp_schemes_list_embedded_registry() {
    let mut scene = mcp_scene();
    let undo_before = scene.undo_stack.len();
    let list = dispatch(&mut scene, "schemes_list", "{}").expect("schemes_list");
    let schemes = list.as_array().expect("массив схем");
    assert_eq!(schemes.len(), 10, "10 пакетов PRD-0008 §7.2: {list}");
    let ids: Vec<&str> = schemes.iter().filter_map(|s| s["id"].as_str()).collect();
    assert!(
        ids.contains(&"com.canvasdesk.scheme.intro-calculations"),
        "intro-схема в реестре: {ids:?}"
    );
    for scheme in schemes {
        assert!(!scheme["name"].as_str().expect("имя RU").is_empty());
        assert!(!scheme["name_en"].as_str().expect("имя EN").is_empty());
        assert!(scheme["nodes"].as_u64().expect("nodes") >= 4);
        assert!(scheme["edges"].as_u64().expect("edges") >= 3);
        assert!(!scheme["category"].as_str().expect("категория").is_empty());
    }
    assert_eq!(
        scene.undo_stack.len(),
        undo_before,
        "чтение: undo не растёт"
    );
    assert_eq!(scene.canvas.nodes.len(), 3, "канвас не мутирован");
}

/// PRD-0008 (Q5 v2): schemes_apply — вставка как «Открыть» в галерее:
/// ремап id без коллизий с занятым канвасом, ровно один undo-шаг,
/// полный пересчёт с оракулами G2 (load 5000 / share 0.625), bbox для
/// zoom-to-fit; вторая вставка не конфликтует; неизвестный id — ошибка
/// БЕЗ undo-шага (fail-fast до мутации).
#[test]
fn mcp_schemes_apply_inserts_flow_and_undo() {
    let mut scene = mcp_scene();
    let undo_before = scene.undo_stack.len();
    let out = dispatch(
        &mut scene,
        "schemes_apply",
        r#"{"id": "com.canvasdesk.scheme.intro-calculations", "x": 500.0, "y": 300.0}"#,
    )
    .expect("schemes_apply");
    assert_eq!(out["applied"], "com.canvasdesk.scheme.intro-calculations");
    assert!(!out["name"].as_str().expect("имя схемы").is_empty());
    let nodes = out["nodes"].as_array().expect("созданные ноды").clone();
    let edges = out["edges"].as_array().expect("созданные рёбра").clone();
    assert_eq!(nodes.len(), 6, "6 нод intro-схемы (PRD §7.2): {out}");
    assert_eq!(edges.len(), 4, "4 ребра intro-схемы");
    // Ремап: свежие note-N id (канвас занят note-1/f1/g1 из mcp_scene)
    assert!(nodes
        .iter()
        .all(|id| id.as_str().is_some_and(|id| id.starts_with("note-"))));
    // value-рёбра схемы видны с адресацией (контракт edges_list)
    assert!(edges.iter().any(|e| e["kind"] == "value"));
    assert!(edges.iter().all(|e| e["id"].as_str().is_some()));
    // Оракул G2: поток после вставки — контрольные числа PRD-0008
    let flow = out["flow"].as_object().expect("карта flow");
    let values: Vec<f64> = flow.values().filter_map(|e| e["value"].as_f64()).collect();
    assert!(values.contains(&5000.0), "оракул load 5000: {out}");
    assert!(values.contains(&0.625), "оракул share 0.625: {out}");
    // Один undo-шаг, канвас помечен грязным (автосейв)
    assert_eq!(
        scene.undo_stack.len(),
        undo_before + 1,
        "ровно один undo-шаг"
    );
    assert!(
        scene.dirty_since.is_some(),
        "вставка помечает канвас грязным"
    );
    // bbox — для viewport_set/zoom-to-fit
    let bbox = out["bbox"].as_array().expect("bbox");
    assert_eq!(bbox.len(), 4);

    // Вторая вставка: ремап без коллизий с первой
    let second = dispatch(
        &mut scene,
        "schemes_apply",
        r#"{"id": "com.canvasdesk.scheme.intro-calculations"}"#,
    )
    .expect("повторная вставка");
    let second_nodes = second["nodes"]
        .as_array()
        .expect("ноды 2-й вставки")
        .clone();
    assert_eq!(second_nodes.len(), 6);
    let first_ids: Vec<String> = nodes
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    assert!(
        second_nodes.iter().all(|id| id
            .as_str()
            .is_some_and(|id| !first_ids.contains(&id.to_owned()))),
        "id второй вставки не пересекаются с первой"
    );

    // Неизвестная схема: ошибка до мутации — undo не растёт
    let undo_after = scene.undo_stack.len();
    let err = dispatch(
        &mut scene,
        "schemes_apply",
        r#"{"id": "com.canvasdesk.scheme.no-such"}"#,
    );
    assert!(err.is_err(), "неизвестная схема — ошибка");
    assert_eq!(
        scene.undo_stack.len(),
        undo_after,
        "fail-fast без undo-шага"
    );
}

/// PRD-0007 (X2, FR-048): lineage — дерево происхождения для агента,
/// та же модель, что окно проверки: итог B (= $in × 2) — calc-корень со
/// значением и формулой, вход — leaf через via-ребро; построчный корень
/// работает; неизвестная нода / отрицательный line / проза — ошибки.
#[test]
fn mcp_lineage_tree_total_line_and_errors() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0,"y":0,"text":"A\n= 5"}"#,
    )
    .expect("нода A");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":300,"y":0,"text":"B\n= $in × 2"}"#,
    )
    .expect("нода B");
    let id = |scene: &SceneState, prefix: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref().is_some_and(|t| t.starts_with(prefix)))
            .map(|n| n.id.clone())
            .expect("нода сценария")
    };
    let (id_a, id_b) = (id(&scene, "A"), id(&scene, "B"));
    let edge_id = dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
    )
    .expect("edge")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{edge_id}","kind":"value"}}"#),
    )
    .expect("value-ребро");

    // Итог ноды B: root = calc 10 с формулой, вход A — leaf 5 через via
    let out = dispatch(&mut scene, "lineage", &format!(r#"{{"node_id":"{id_b}"}}"#))
        .expect("lineage итога B");
    assert_eq!(out["root"]["node_id"], id_b.as_str());
    assert_eq!(
        out["root"]["line"],
        serde_json::Value::Null,
        "null — итог ноды"
    );
    let nodes = out["nodes"].as_array().expect("массив узлов");
    assert_eq!(nodes.len(), 2, "итог B + его вход: {out}");
    let root = &nodes[0];
    assert_eq!(root["node_id"], id_b.as_str());
    assert_eq!(root["kind"], "calc", "формула с входом — расчётный узел");
    assert_eq!(root["value"], 10.0, "B = 5 × 2");
    assert!(root["formula"].as_str().expect("формула").contains("$in"));
    assert!(!root["title"].as_str().expect("заголовок").is_empty());
    let children = root["children"].as_array().expect("дети корня");
    assert_eq!(children.len(), 1);
    assert_eq!(
        children[0]["via"]["edge_id"].as_str().expect("via-ребро"),
        edge_id,
        "via — ребро для подсветки цепочки"
    );
    let leaf = &nodes[children[0]["child"].as_u64().expect("индекс ребёнка") as usize];
    assert_eq!(leaf["node_id"], id_a.as_str());
    assert_eq!(leaf["kind"], "leaf", "константа без входов");
    assert_eq!(leaf["value"], 5.0);

    // Построчный корень (строка 1 ноды B — формула) и корень листа A
    let line_root = dispatch(
        &mut scene,
        "lineage",
        &format!(r#"{{"node_id":"{id_b}", "line": 1}}"#),
    )
    .expect("lineage строки B");
    assert_eq!(line_root["root"]["line"], 1);
    let a_root = dispatch(&mut scene, "lineage", &format!(r#"{{"node_id":"{id_a}"}}"#))
        .expect("lineage листа A");
    assert_eq!(
        a_root["nodes"].as_array().expect("узлы").len(),
        1,
        "лист без детей"
    );

    // Негативные ветки: неизвестная нода, line < 0, проза
    assert!(dispatch(&mut scene, "lineage", r#"{"node_id": "no-such"}"#).is_err());
    assert!(dispatch(
        &mut scene,
        "lineage",
        &format!(r#"{{"node_id":"{id_b}", "line": -1}}"#)
    )
    .is_err());
    assert!(
        dispatch(
            &mut scene,
            "lineage",
            &format!(r#"{{"node_id":"{id_b}", "line": 0}}"#)
        )
        .is_err(),
        "проза не может быть корнем"
    );
    // line: null — то же, что итог (сахар для агентов)
    let null_root = dispatch(
        &mut scene,
        "lineage",
        &format!(r#"{{"node_id":"{id_b}", "line": null}}"#),
    )
    .expect("line null");
    assert_eq!(null_root["root"]["line"], serde_json::Value::Null);
}

/// PRD-0007 (X6, FR-048, F-9): explain_number — текстовая линейная
/// развёртка того же дерева: render:"text", в text — заголовок с
/// значением корня, узлы с адресами [id:строка], канал прихода (ребро),
/// статистика; при активном what-if — преамбула; негативы — как у lineage.
#[test]
fn mcp_explain_number_text_deployment() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0,"y":0,"text":"A\n= 5"}"#,
    )
    .expect("нода A");
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":300,"y":0,"text":"B\n= $in × 2"}"#,
    )
    .expect("нода B");
    let id = |scene: &SceneState, prefix: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref().is_some_and(|t| t.starts_with(prefix)))
            .map(|n| n.id.clone())
            .expect("нода сценария")
    };
    let (id_a, id_b) = (id(&scene, "A"), id(&scene, "B"));
    let edge_id = dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
    )
    .expect("edge")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    dispatch(
        &mut scene,
        "flow_set_kind",
        &format!(r#"{{"id":"{edge_id}","kind":"value"}}"#),
    )
    .expect("value-ребро");

    let out = dispatch(
        &mut scene,
        "explain_number",
        &format!(r#"{{"node_id":"{id_b}"}}"#),
    )
    .expect("explain_number итога B");
    assert_eq!(out["render"], "text", "маркер text-first результата");
    let text = out["text"].as_str().expect("текст объяснения");
    assert!(
        !text.contains("Режим what-if"),
        "без активного сценария преамбулы нет: {text}"
    );
    // Заголовок: корень со значением (B = 5 × 2 = 10).
    let first_line = text.lines().next().expect("первая строка");
    assert!(first_line.starts_with("Цепочка расчёта: "), "{first_line}");
    assert!(first_line.contains("= 10"), "{first_line}");
    // Узлы: адрес [id:строка] корня и листа, канал с ребром, статистика.
    // Лист-константа Numi-строки несёт формулу — панель тоже показывает её
    // (паритет UI: «формула» приоритетнее пометки «исходное значение»).
    assert!(text.contains(&format!("[{id_b}]")), "{text}");
    assert!(text.contains(&format!("[{id_a}]")), "{text}");
    assert!(text.contains("· формула: 5"), "{text}");
    assert!(text.contains(&format!("(ребро {edge_id})")), "{text}");
    assert!(text.ends_with("Всего узлов: 2 (листьев: 1)"), "{text}");
    assert_eq!(out["root"]["node_id"], id_b.as_str());
    assert_eq!(out["nodes"], 2);
    assert_eq!(out["truncated"], false);

    // Негативы — паритет с lineage: неизвестная нода / проза-корень.
    assert!(dispatch(&mut scene, "explain_number", r#"{"node_id": "no-such"}"#).is_err());
    assert!(dispatch(
        &mut scene,
        "explain_number",
        &format!(r#"{{"node_id":"{id_b}", "line": 0}}"#)
    )
    .is_err());
}

/// MCP-parity (запрос владельца 2026-09-22): flow_recalc и
/// analyze_bottlenecks следуют за АКТИВНЫМ what-if сценарием — агент
/// видит те же числа/флаги, что пользователь на канвасе (инвариант
/// «MCP-видимость = UI»); авто-строки FR-050 Р-4 в ответе flow_recalc
/// несут значения активного сценария; graph_apply flow — тот же источник.
#[test]
fn mcp_flow_and_analysis_follow_active_whatif() {
    let (mut scene, cdn, _gw) = reference_scene();
    let traffic = node_id_of(&scene, "dau = 200000").expect("нода «Нагрузка»");

    // База: CDN ρ 0.417 — none; peak_rps 208.33
    let report = dispatch(&mut scene, "analyze_bottlenecks", "{}").expect("analyze база");
    assert_eq!(node_report(&report, &cdn)["severity"], "none");
    let base = dispatch(&mut scene, "flow_recalc", "{}").expect("flow база");
    assert_close(
        base[&traffic]["value"].as_f64().expect("peak_rps"),
        208.3333,
        "база: peak_rps",
    );

    // Нода-наблюдатель с value-ребром БЕЗ toParam → авто-строка (FR-050 Р-4)
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0,"y":600,"text":"наблюдатель"}"#,
    )
    .expect("наблюдатель");
    let observer = node_id_of(&scene, "наблюдатель").expect("id наблюдателя");
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{traffic}", "to": "{observer}", "kind": "value", "fromOutput": "peak_rps"}}"#
        ),
    )
    .expect("value-ребро без toParam");

    // What-if: DAU ×2 — подмена строки 0 (runtime, файл не меняется)
    dispatch(
        &mut scene,
        "whatif_set_override",
        &format!(r#"{{"node_id":"{traffic}","line":0,"expr":"dau = 400000"}}"#),
    )
    .expect("подмена DAU ×2");

    // Активное состояние: peak_rps 416.67 (не база!), ρ 0.833 → warn
    let active = dispatch(&mut scene, "flow_recalc", "{}").expect("flow активный");
    assert_close(
        active[&traffic]["value"].as_f64().expect("peak_rps ×2"),
        416.6667,
        "flow_recalc показывает ПОДМЕНУ, как канвас",
    );
    let report = dispatch(&mut scene, "analyze_bottlenecks", "{}").expect("analyze активный");
    assert_eq!(
        node_report(&report, &cdn)["severity"],
        "warn",
        "анализ следует за подменой: ρ 0.833"
    );
    // Авто-строка наблюдателя — значение АКТИВНОГО сценария
    let auto = active[&observer]["autoRows"]
        .as_array()
        .expect("авто-строки");
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0]["field"], "peak_rps");
    assert!(auto[0]["path"].as_str().expect("путь").contains("peak_rps"));
    assert_close(
        auto[0]["value"].as_f64().expect("значение авто-строки"),
        416.6667,
        "авто-строка — активное значение",
    );

    // graph_apply flow — тот же активный источник (node_move не меняет
    // модель, но пересчёт и карта в ответе — активные)
    let moved = graph_apply(
        &mut scene,
        &format!(r#"[{{"op": "node_move", "id": "{observer}", "x": 40, "y": 640}}]"#),
    )
    .expect("node_move");
    assert_close(
        moved["flow"][&traffic]["value"]
            .as_f64()
            .expect("peak_rps в ответе батча"),
        416.6667,
        "graph_apply flow — активное состояние",
    );

    // Возврат на «Базу» — значения возвращаются (канвас не мутировал)
    dispatch(
        &mut scene,
        "whatif_scenario_activate",
        r#"{"name": "База"}"#,
    )
    .expect("активация Базы");
    let returned = dispatch(&mut scene, "flow_recalc", "{}").expect("flow после возврата");
    assert_close(
        returned[&traffic]["value"].as_f64().expect("peak_rps база"),
        208.3333,
        "база восстановлена",
    );
}

/// FR-050 Р-3 (этап C): кэш unmapped-рёбер в `SceneState` — ребро есть,
/// значения нет (строка-источник стала прозой) → id ребра в кэше (пунктир
/// янтарным + тултип «проблема + решение»); возврат значения пересчётом
/// снимает состояние (инвариант 4 FR-045).
#[test]
fn scene_unmapped_edges_cache_set_and_unset() {
    let mut scene = mcp_scene();
    let ops = r#"[
        {"op": "node_create_note", "ref": "traffic", "x": 0, "y": 0,
         "text": "Трафик\npeak_rps = 1389 rps"},
        {"op": "node_create_note", "ref": "gateway", "x": 400, "y": 0,
         "text": "заметка без формулы"}
    ]"#;
    let report = graph_apply(&mut scene, ops).expect("сборка");
    assert_eq!(report["ok"], true, "сборка чистая: {report}");
    let find = |scene: &SceneState, text: &str| {
        scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref().map(|t| t.contains(text)).unwrap_or(false))
            .map(|n| n.id.clone())
            .expect(text)
    };
    let traffic = find(&scene, "Трафик");
    let gateway = find(&scene, "заметка без формулы");
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{traffic}", "to": "{gateway}", "fromOutput": "peak_rps", "kind": "value"}}"#
        ),
    )
    .expect("value-ребро");
    let edge_id = scene
        .canvas
        .edges
        .iter()
        .find(|e| e.to_node == gateway)
        .map(|e| e.id.clone())
        .expect("ребро");
    // Значение пролито — unmapped пуст
    assert!(
        !scene.unmapped_edges.contains(&edge_id),
        "значение есть — ребро НЕ unmapped"
    );
    // Источник стал прозой: строка-присваивание удалена — значения нет
    // (node_update_text — полный пересчёт; node_edit с text ленив — CR-012)
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_update_text",
        &serde_json::json!({
            "id": traffic,
            "text": "Трафик",
        }),
    )
    .expect("правка источника");
    assert!(
        scene.unmapped_edges.contains(&edge_id),
        "связь есть, значения нет — ребро unmapped"
    );
    // Возврат значения пересчётом снимает состояние
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_update_text",
        &serde_json::json!({
            "id": traffic,
            "text": "Трафик\npeak_rps = 1389 rps",
        }),
    )
    .expect("возврат строки-источника");
    assert!(
        !scene.unmapped_edges.contains(&edge_id),
        "значение вернулось — состояние снято"
    );
}

/// FR-050 Н9-2 (этап D): представление проливания несёт квалифицированный
/// путь источника «Объект.Поле» и локальный литерал RHS — данные тултипа
/// «пролито: Трафик.peak_rps = 1389 rps (локально было: 50 rps)».
/// Приёмник — шаблонная нода (toParam адресует параметры шаблонов).
#[test]
fn spill_view_carries_path_and_local() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "text": "Трафик\npeak_rps = 1389 rps"}"#,
    )
    .expect("traffic");
    let traffic = scene.canvas.nodes.last().expect("нода").id.clone();
    let cdn = dispatch(
        &mut scene,
        "template_instantiate",
        r#"{"id": "com.canvasdesk.cdn", "x": 400, "y": 0, "params": {}}"#,
    )
    .expect("CDN")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{traffic}", "to": "{cdn}", "fromOutput": "peak_rps", "toParam": "rps", "kind": "value"}}"#
        ),
    )
    .expect("toParam-ребро");
    let spills = scene.param_spills.get(&cdn).expect("проливание в кэше");
    assert_eq!(spills.len(), 1);
    let view = &spills[0];
    assert_eq!(view.param, "rps");
    assert_eq!(view.line, Some(0), "первая строка листа CDN — rps");
    assert_eq!(view.path, "Трафик.peak_rps", "квалифицированный путь Н9-2");
    assert_eq!(
        view.value.as_deref(),
        Some("1\u{a0}389 rps"),
        "значение ребра"
    );
    assert_eq!(
        view.local.as_deref(),
        Some("50 rps"),
        "локальный литерал RHS"
    );
}

/// FR-050 Р-4 (этап D): рост высоты под авто-строки при подключении связи
/// (growth-only, как CR-012): карточка обязана вместить строку-проекцию —
/// иначе она обрежется клипом тела; достаточная высота не трогается.
#[test]
fn auto_rows_grow_node_height() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "text": "Трафик\npeak_rps = 1389 rps"}"#,
    )
    .expect("traffic");
    let traffic = scene.canvas.nodes.last().expect("нода").id.clone();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 400, "y": 0, "text": "Смета\nитог := 100 + 5"}"#,
    )
    .expect("note");
    let note = scene.canvas.nodes.last().expect("нода").id.clone();
    // Зажим высоты ЗАРАНЕЕ: строка-проекция не вместится → карточка растёт
    for node in scene.canvas.nodes.iter_mut() {
        if node.id == note {
            node.height = 90.0;
        }
    }
    dispatch(
        &mut scene,
        "edge_create",
        &format!(
            r#"{{"from": "{traffic}", "to": "{note}", "fromOutput": "peak_rps", "kind": "value"}}"#
        ),
    )
    .expect("позиционное value-ребро (живой пересчёт)");
    // Авто-строка: формула ноды не читает $1 (и нет toParam)
    let rows = scene.auto_rows.get(&note).expect("авто-строка в кэше");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].path, "Трафик.peak_rps");
    assert_eq!(rows[0].slot, 0);
    assert_eq!(
        rows[0].value.clone().map(|v| v.to_string()),
        Some("1\u{a0}389 rps".to_owned())
    );
    let after = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.id == note)
        .map(|n| n.height)
        .expect("нода");
    assert!(after > 90.0, "высота выросла под авто-строку: 90 → {after}");
    // Достаточная высота не трогается (growth-only, без осцилляций):
    // пересчёт без мутаций (node_update_text тем же текстом) — высота та же
    mcp_dispatch(
        &mut scene,
        &canvas_core::templates::TemplateRegistry::builtin(),
        "node_update_text",
        &serde_json::json!({
            "id": note,
            "text": "Смета\nитог := 100 + 5",
        }),
    )
    .expect("повторный пересчёт");
    let stable = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.id == note)
        .map(|n| n.height)
        .expect("нода");
    assert_eq!(after, stable, "повторный пересчёт — без осцилляций");
}

// --- FR-050 этап E: Н9-1 детект изменений для волны каскада ---

/// FR-050 Н9-1: первый пересчёт (загрузка) — без волны (сравнивать не с
/// чем); мутация, не меняющая итогов (сдвиг позиции ноды) — тоже пусто.
#[test]
fn flow_changed_nodes_empty_on_load_and_positional_move() {
    let mut scene = mcp_scene();
    assert!(
        scene.flow_changed_nodes.is_empty(),
        "первый пересчёт — без волны"
    );
    // Сдвиг позиции ноды: итоги формул от позиции не зависят — волны нет
    scene.canvas.nodes[0].x += 64.0;
    scene.recompute_flow();
    assert!(
        scene.flow_changed_nodes.is_empty(),
        "позиционная мутация не меняет значений"
    );
}

/// FR-050 Н9-1: правка upstream (текст истока) → итоги истока и приёмника
/// изменились — обе ноды в списке seeds; повторный пересчёт без мутаций —
/// список пуст (волна одноразовая на событие изменения).
#[test]
fn flow_changed_nodes_detects_upstream_edit() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "text": "Трафик\npeak_rps = 1389 rps"}"#,
    )
    .expect("traffic");
    let traffic = scene.canvas.nodes.last().expect("нода").id.clone();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 400, "y": 0, "text": "$in × 2"}"#,
    )
    .expect("receiver");
    let receiver = scene.canvas.nodes.last().expect("нода").id.clone();
    dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from": "{traffic}", "to": "{receiver}", "fromLine": 1, "kind": "value"}}"#),
    )
    .expect("value-ребро");
    // Подключение ребра — итог приёмника сменил состояние (ошибка «вход
    // отсутствует» → значение): волна уместна, приёмник в seeds;
    // повторный пересчёт — пусто (волна одноразовая на событие)
    assert_eq!(
        scene.flow_changed_nodes,
        vec![receiver.clone()],
        "подключение ребра — значение подтянулось"
    );
    scene.recompute_flow();
    assert!(
        scene.flow_changed_nodes.is_empty(),
        "повторный пересчёт — без изменений"
    );
    // Правка upstream: 1389 → 2000 — строка-значение истока (присваивающий
    // лист без узлового итога — lines-детект) И итог приёмника: обе ноды
    // в seeds (волна бежит от истока вниз, приёмник продолжает каскад)
    dispatch(
        &mut scene,
        "node_update_text",
        &serde_json::json!({
            "id": traffic,
            "text": "Трафик\npeak_rps = 2000 rps",
        })
        .to_string(),
    )
    .expect("правка истока");
    let changed = scene.flow_changed_nodes.clone();
    assert_eq!(changed.len(), 2, "исток и приёмник: {changed:?}");
    assert!(
        changed.contains(&traffic),
        "исток (строка-значение): {changed:?}"
    );
    assert!(changed.contains(&receiver), "приёмник (итог): {changed:?}");
    // Повторный пересчёт без мутаций — волна погасла
    scene.recompute_flow();
    assert!(
        scene.flow_changed_nodes.is_empty(),
        "повторный пересчёт — без изменений"
    );
}

/// FR-050 этап F (Н6/Р-6): все 6 схем FR-049 мигрированы на именованные
/// ссылки «Объект.Поле» — формулы не используют позиционные `$N`/`$in`,
/// каждая позиционная value-связь адресуется qualified-путём (fromOutput
/// на ребре). Оракулы чисел — ДО миграции (значения не меняются,
/// детерминизм резолва имён); без ошибок вычисления и предупреждений
/// W-UNUSED-SLOT (именованный путь читает слот своего ребра).
#[test]
fn schemes_named_refs_keep_oracles() {
    let oracles: &[(&str, &[(&str, f64)])] = &[
        (
            "com.canvasdesk.scheme.intro-calculations",
            &[("total", 5000.0), ("share", 0.625)],
        ),
        // rps = 5000/30 ≈ 166.667; peak = ×3 = 500; util = 166.67/200 ≈ 0.8333;
        // peak_util = 500/200 = 2.5
        (
            "com.canvasdesk.scheme.capacity-service",
            &[
                ("rps", 166.666_67),
                ("peak_rps", 500.0),
                ("util", 0.833_333_3),
                ("peak_util", 2.5),
            ],
        ),
        // total = 1200 + 300 = 1500; annual = 1500 × 12 = 18000
        (
            "com.canvasdesk.scheme.intro-whatif",
            &[("total", 1500.0), ("annual", 18_000.0)],
        ),
        // team 13500 — построчная переменная (не узловой выход);
        // узловые: infra 1400 (последняя строка), reserve_sum 2025,
        // total 16925, final_sum 16925; advance_sum 10155 — строка
        (
            "com.canvasdesk.scheme.project-budget",
            &[
                ("infra", 1_400.0),
                ("reserve_sum", 2_025.0),
                ("total", 16_925.0),
                ("final_sum", 16_925.0),
            ],
        ),
        // total_area 30, living_cost 450, bedroom_cost 300, total 1450,
        // final 1305
        (
            "com.canvasdesk.scheme.renovation-estimate",
            &[
                ("total_area", 30.0),
                ("living_cost", 450.0),
                ("bedroom_cost", 300.0),
                ("total", 1_450.0),
                ("final", 1_305.0),
            ],
        ),
        // margin 6, ltv 216, ratio 1.8, payback 20
        (
            "com.canvasdesk.scheme.unit-economics",
            &[
                ("margin", 6.0),
                ("ltv", 216.0),
                ("ratio", 1.8),
                ("payback", 20.0),
            ],
        ),
    ];
    for (scheme_id, checks) in oracles {
        let mut scene = mcp_scene();
        let out = mcp_dispatch(
            &mut scene,
            &canvas_core::templates::TemplateRegistry::builtin(),
            "schemes_apply",
            &serde_json::json!({ "id": scheme_id, "x": 0.0, "y": 0.0 }),
        )
        .unwrap_or_else(|err| panic!("{scheme_id}: schemes_apply: {err}"));
        let flow = out["flow"]
            .as_object()
            .unwrap_or_else(|| panic!("{scheme_id}: нет flow: {out}"));
        // Значения по ИМЕНИ переменной (текст ноды: заголовок / присваивание)
        let values: Vec<f64> = flow
            .values()
            .filter_map(|entry| entry["value"].as_f64())
            .collect();
        for (name, expected) in *checks {
            let found = values.iter().any(|v| {
                close_1pct(*v, *expected) && (v - expected).abs() < expected.abs().max(1.0) * 0.01
            });
            assert!(
                found,
                "{scheme_id}: оракул {name}={expected} не найден среди {values:?}"
            );
        }
        // Ни одной ошибки вычисления (красные строки) — резолв имён полный
        for (node_id, entry) in flow {
            if entry["error"].is_string() {
                panic!("{scheme_id}: ошибка в {node_id}: {}", entry["error"]);
            }
        }
    }
}

/// FR-050 этап F (фикс): инвариант «строка и узел видят одно окружение» —
/// построчная формула приёмника резолвит qualified-ссылку «Объект.Поле»
/// так же, как узловой итог: `line_eval_env` — зеркало `inbound_values`
/// (слоты + проливание в параметры + qualified-карта). До фикса
/// окружение строк было slots-only (`inbound_slots_with_lines`): строка
/// краснела «вход не найден» при верном узловом итоге (найдено
/// миграцией схем FR-049 на именованные ссылки).
#[test]
fn line_eval_env_resolves_named_refs() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "width": 300, "text": "Заявки\nusers = 10"}"#,
    )
    .expect("исток");
    let src = scene.canvas.nodes.last().expect("нода").id.clone();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 400, "y": 0, "width": 300, "text": "Отчёт\nx = Заявки.users * 2"}"#,
    )
    .expect("приёмник");
    let dst = scene.canvas.nodes.last().expect("нода").id.clone();
    dispatch(
        &mut scene,
        "edge_create",
        &format!(r#"{{"from": "{src}", "to": "{dst}", "kind": "value", "fromOutput": "users"}}"#),
    )
    .expect("ребро fromOutput");
    scene.recompute_flow();

    // Узловой итог приёмника верен: Заявки.users × 2 = 20
    match scene.expr_results.get(&dst) {
        Some(ExprOutcome::Ok(value)) => assert_eq!(value.to_string(), "20"),
        other => panic!("узловой итог: {other:?}"),
    }
    // Инвариант: построчный результат той же строки — тоже 20 (строка
    // не краснеет); до фикса здесь был None при верном узловом итоге
    let lines = scene
        .expr_line_results
        .get(&dst)
        .expect("построчные результаты");
    assert_eq!(lines.len(), 2, "титул + формула");
    assert_eq!(lines[0], None, "проза без результата");
    match lines[1]
        .as_ref()
        .expect("строка с qualified-ссылкой резолвится")
    {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "20"),
        other => panic!("построчный итог: {other:?}"),
    }
}

// --- FR-061 хвосты (D-7/D-8 runtime v1): тогглы блока/описания, Q3-проза ---

/// FR-061 хвосты (D-7 runtime v1, Q4): тоггл свёрнутости — runtime-состояние,
/// пустое по умолчанию (дефолт — развёрнутый блок), в .canvas не пишется.
#[test]
fn block_collapsed_toggle_is_runtime_state() {
    let canvas = Canvas::default();
    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/fr061-toggle.canvas"));
    assert!(scene.block_collapsed.is_empty(), "дефолт — развёрнут");

    let node = Node::text("n1", "текст", 0.0, 0.0);
    scene.canvas.nodes.push(node);

    assert!(scene.toggle_block_collapsed("n1"), "первый тоггл — свёрнут");
    assert!(scene.block_collapsed.contains("n1"));
    assert!(
        !scene.toggle_block_collapsed("n1"),
        "второй тоггл — развёрнут"
    );
    assert!(scene.block_collapsed.is_empty());
}

/// FR-061 хвосты (D-8, «Раскрыть+авто»): тоггл раскрытости описания и
/// автосворачивание (клик вне ноды — collapse_descs_except(None)).
#[test]
fn desc_expanded_toggle_and_auto_collapse() {
    let canvas = Canvas::default();
    let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/fr061-desc.canvas"));
    scene.canvas.nodes.push(Node::text("n1", "текст", 0.0, 0.0));
    scene.canvas.nodes.push(Node::text("n2", "текст", 0.0, 0.0));

    assert!(scene.toggle_desc_expanded("n1"));
    assert!(scene.toggle_desc_expanded("n2"));
    assert_eq!(scene.desc_expanded.len(), 2);

    // Клик по телу n1 — сохраняется только n1
    scene.collapse_descs_except(Some("n1"));
    assert_eq!(scene.desc_expanded.len(), 1);
    assert!(scene.desc_expanded.contains("n1"));

    // Клик по фону — сворачиваются все
    scene.collapse_descs_except(None);
    assert!(scene.desc_expanded.is_empty());
}

/// FR-061 приёмка T9 (решение по фидбэку владельца 2026-09-24): prose-
/// фолбэк описания УБРАН — он рисовал первый абзац тела ДВАЖДЫ (зона
/// описания + тело). Зона описания — только явные источники:
/// `canvasdesk.desc` → манифест шаблона; у обычной заметки зоны нет.
#[test]
fn node_desc_without_prose_fallback() {
    // Случай 1: явный desc — зона есть
    let mut canvas = Canvas::default();
    let mut node = Node::text("n1", "Проза текста.\n\n800 rps", 0.0, 0.0);
    node.canvasdesk = Some(CanvasdeskExt {
        desc: Some("Явное описание".to_owned()),
        props: Default::default(),
        widget_id: None,
        expr: None,
        template: None,
        data: None,
        title: None,
    });
    canvas.nodes.push(node);
    let scene = SceneState::new(canvas, PathBuf::from("target/tmp/fr061-q3a.canvas"));
    assert_eq!(
        scene.node_desc_text(0).as_deref(),
        Some("Явное описание"),
        "явный desc — источник зоны"
    );

    // Случай 2: обычная заметка с прозой — зоны НЕТ (первый абзац не
    // дублируется в зоне описания, он живёт только в теле)
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text(
        "n2",
        "800 rps\n\nОписание нагрузки шлюза.",
        0.0,
        0.0,
    ));
    let scene = SceneState::new(canvas, PathBuf::from("target/tmp/fr061-q3b.canvas"));
    assert!(
        scene.node_desc_text(0).is_none(),
        "проза-фолбэк убран: без desc/манифеста зоны нет"
    );
}

// --- FR-066 (M5/S3): monte_carlo_run — MC/QMC-прогон ---------------------------

/// Хелпер: эталон №5 ADR-0006 (unit economics) одним Numi-листом —
/// сборка через MCP (агентный путь), как эталон Instagram MVP.
#[cfg(feature = "qmc")]
fn fr066_etalon5_scene() -> (SceneState, String) {
    let mut scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/fr066.canvas"));
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "width": 340, "text": "spend = 60000\nnew_customers = 3000\ncac = spend / new_customers\narpu = 12\nmargin = 0.8\nchurn = 0.1\nltv = arpu * margin / churn\nratio = ltv / cac\npayback = cac / (arpu * margin)"}"#,
    )
    .expect("unit-economics лист");
    let id = scene.canvas.nodes.last().expect("нода").id.clone();
    (scene, id)
}

/// FR-066 e2e: monte_carlo_run на эталоне №5 — квантили против oracle ±1 %
/// (cac = 60000/N(3000, 100): медиана 20, P90 20.895, P99 21.684),
/// сид-воспроизводимость (повтор — побитово те же квантили),
/// engine-метаданные §5.7.4, анализ на P90.
#[cfg(feature = "qmc")]
#[test]
fn mcp_fr066_monte_carlo_run_etalon5_reference() {
    let (mut scene, ue) = fr066_etalon5_scene();
    let request = format!(
        r#"{{"runs": 4096, "mode": "qmc", "seed": 7, "params": {{"{ue}:new_customers": {{"dist": "normal", "mean": 3000, "sd": 100}}}}}}"#
    );
    let out = dispatch(&mut scene, "monte_carlo_run", &request).expect("monte_carlo_run");
    assert_eq!(out["runs"], 4096);
    assert_eq!(out["failed_runs"], 0);
    assert_eq!(out["mode"], "qmc");
    assert_eq!(out["seed"], 7);
    assert_eq!(out["stale"], false, "первый прогон — не протух");
    assert!(out["duration_ms"].as_f64().is_some_and(|ms| ms > 0.0));
    assert_eq!(
        out["quantiles"],
        serde_json::json!([0.5, 0.9, 0.99]),
        "дефолтные квантили"
    );

    // квантили cac (переменная листа — именованный выход) против oracle
    let close_1pct = |actual: f64, oracle: f64| (actual - oracle).abs() <= oracle.abs() * 0.01;
    let cac = out["named"][format!("{ue}:cac")]
        .as_object()
        .expect("named-серия cac");
    let p50 = cac["P50"]["value"].as_f64().expect("P50");
    let p90 = cac["P90"]["value"].as_f64().expect("P90");
    let p99 = cac["P99"]["value"].as_f64().expect("P99");
    assert!(close_1pct(p50, 20.0), "P50(cac) = {p50} vs 20.0");
    assert!(close_1pct(p90, 20.895), "P90(cac) = {p90} vs 20.895");
    assert!(close_1pct(p99, 21.684), "P99(cac) = {p99} vs 21.684");
    assert!(p50 < p90 && p90 < p99, "монотонность квантилей");
    // построчная серия: строка 2 — «cac = spend / new_customers»
    assert!(
        out["lines"][format!("{ue}:2")]["P90"]["value"]
            .as_f64()
            .is_some_and(|v| close_1pct(v, 20.895)),
        "P90 строки cac"
    );

    // сид-воспроизводимость: повторный вызов с тем же seed — идентичные
    // квантили (визуально дельт нет, diff снимков = 0)
    let out2 = dispatch(&mut scene, "monte_carlo_run", &request).expect("повтор");
    assert_eq!(out["named"], out2["named"], "квантили идентичны");
    assert_eq!(out["outputs"], out2["outputs"]);
    assert_eq!(out["lines"], out2["lines"]);

    // engine-метаданные §5.7.4: raw JSON в canvasdesk.engine
    assert_eq!(
        scene.canvas.extra["canvasdesk"]["engine"]["version"],
        "M5.0"
    );
    assert_eq!(
        scene.canvas.extra["canvasdesk"]["engine"]["seed"].as_str(),
        Some("7"),
        "seed канонически строкой (JS-safe)"
    );
    assert_eq!(scene.canvas.extra["canvasdesk"]["engine"]["qmc"], true);

    // анализ на хвостовом квантиле (P90): утилизаций нет — severity none
    assert_eq!(out["severity"], "none");
    assert!((out["analysis"]["quantile"].as_f64().unwrap_or(0.0) - 0.9).abs() < 1e-12);
    assert!(
        out["analysis"]["thresholds"].is_object(),
        "пороги как у analyze_bottlenecks"
    );

    // undo: engine-метаданные — один undo-шаг на ПЕРВОМ прогоне
    // (второй не менял канвас), снимок до мутации
    let before = scene.take_undo().expect("undo-шаг engine-метаданных");
    assert!(
        before
            .extra
            .get("canvasdesk")
            .and_then(|v| v.get("engine"))
            .is_none(),
        "снимок — до записи engine"
    );
}

/// FR-066: режим mc — работает и детерминирован; кастомные квантили
/// (P10 «runway»-кейс) и именованные серии ответа.
#[cfg(feature = "qmc")]
#[test]
fn mcp_fr066_monte_carlo_run_mc_mode_and_custom_quantiles() {
    let (mut scene, ue) = fr066_etalon5_scene();
    let request = format!(
        r#"{{"runs": 2048, "mode": "mc", "seed": 3, "quantiles": [0.1, 0.5], "params": {{"{ue}:new_customers": {{"dist": "normal", "mean": 3000, "sd": 100}}}}}}"#
    );
    let out = dispatch(&mut scene, "monte_carlo_run", &request).expect("mc-режим");
    assert_eq!(out["mode"], "mc");
    assert_eq!(out["quantiles"], serde_json::json!([0.1, 0.5]));
    // P10(cac) = 60000/(3000 − 100·Φ⁻¹(0.1)) = 60000/3128.2 = 19.182
    let p10 = out["named"][format!("{ue}:cac")]["P10"]["value"]
        .as_f64()
        .expect("P10");
    assert!(
        (p10 - 19.182).abs() <= 19.182 * 0.01,
        "P10(cac) = {p10} vs 19.182"
    );
    // детерминизм MC-режима: тот же seed — те же квантили
    let out2 = dispatch(&mut scene, "monte_carlo_run", &request).expect("mc-повтор");
    assert_eq!(out["named"], out2["named"], "MC: сид-воспроизводимость");
}

/// FR-066: строгая валидация MCP — неизвестная нода/параметр, мусорный
/// mode/dist/quantiles, QMC за 2^16 — ошибки ДО прогонов (канвас не
/// помечается грязным).
#[cfg(feature = "qmc")]
#[test]
fn mcp_fr066_monte_carlo_run_strict_validation() {
    let (mut scene, ue) = fr066_etalon5_scene();
    let before = scene.canvas.clone();
    for (label, request) in [
        (
            "ghost-нода",
            r#"{"runs": 100, "params": {"ghost:x": {"dist": "exp", "lambda": 1}}}"#,
        ),
        (
            "параметр без строки",
            &format!(
                r#"{{"runs": 100, "params": {{"{ue}:no_such": {{"dist": "exp", "lambda": 1}}}}}}"#
            ),
        ),
        (
            "мусорный dist",
            &format!(
                r#"{{"runs": 100, "params": {{"{ue}:churn": {{"dist": "weibull", "lambda": 1}}}}}}"#
            ),
        ),
        (
            "ключ без двоеточия",
            r#"{"runs": 100, "params": {"ue churn": {"dist": "exp", "lambda": 1}}}"#,
        ),
        ("runs = 0", r#"{"runs": 0, "params": {}}"#),
        ("QMC за 2^16", r#"{"runs": 65537, "params": {}}"#),
        (
            "мусорный mode",
            r#"{"runs": 100, "mode": "rng", "params": {}}"#,
        ),
        (
            "квантиль вне (0,1)",
            r#"{"runs": 100, "quantiles": [1.5], "params": {}}"#,
        ),
        (
            "σ ≤ 0",
            &format!(
                r#"{{"runs": 100, "params": {{"{ue}:churn": {{"dist": "normal", "mean": 1, "sd": 0}}}}}}"#
            ),
        ),
    ] {
        let err = dispatch(&mut scene, "monte_carlo_run", request)
            .unwrap_err()
            .to_lowercase();
        assert!(!err.is_empty(), "{label}: ошибка обязана быть");
    }
    assert_eq!(scene.canvas, before, "невалидные вызовы канвас не меняют");
    // без обязательного params — ошибка
    assert!(dispatch(&mut scene, "monte_carlo_run", r#"{"runs": 100}"#).is_err());
    // без runs — ошибка
    assert!(dispatch(&mut scene, "monte_carlo_run", r#"{"params": {}}"#).is_err());
}

/// FR-066 (P3): анализ узких мест на P90 — severity эскалирует на хвосте
/// (P50 → warn, P90 → critical), FR-016 без правок: ρ-канвас
/// «load = 0.85 / rho = load × 1 %», load ~ N(0.85, 0.05).
#[cfg(feature = "qmc")]
#[test]
fn mcp_fr066_monte_carlo_run_severity_escalates_on_p90() {
    let mut scene = SceneState::new(
        Canvas::default(),
        PathBuf::from("target/tmp/fr066-rho.canvas"),
    );
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x": 0, "y": 0, "text": "load = 0.85\nrho = load × 1 %"}"#,
    )
    .expect("ρ-лист");
    let srv = scene.canvas.nodes.last().expect("нода").id.clone();

    // P50-снимок: severity none/warn — но инструмент анализирует ХВОСТ,
    // поэтому проверяем эскалацию через сам ответ: P90 ρ ≈ 0.914 → critical
    let request = format!(
        r#"{{"runs": 4096, "seed": 11, "params": {{"{srv}:load": {{"dist": "normal", "mean": 0.85, "sd": 0.05}}}}}}"#
    );
    let out = dispatch(&mut scene, "monte_carlo_run", &request).expect("monte_carlo_run");
    assert_eq!(out["severity"], "critical", "P90 ρ ≈ 0.914 — critical");
    let node = out["analysis"]["nodes"]
        .as_array()
        .expect("узлы анализа")
        .iter()
        .find(|n| n["id"] == srv.as_str())
        .expect("флаги srv")
        .clone();
    assert_eq!(node["severity"], "critical");
    assert!(
        node["utilization"]
            .as_f64()
            .is_some_and(|u| u > 0.9 && u < 1.0),
        "ρ из P90-снимка: {node}"
    );
    // квантили значения ноды (ρ): P50 < 0.9 ≤ P90
    let rho = out["outputs"][&srv].as_object().expect("серия ρ");
    let p50 = rho["P50"]["value"].as_f64().expect("P50 ρ");
    let p90 = rho["P90"]["value"].as_f64().expect("P90 ρ");
    assert!(
        (p50 - 0.85).abs() < 0.02 && (p90 - 0.914).abs() < 0.02,
        "P50 ρ = {p50}, P90 ρ = {p90}"
    );
    assert!(p50 < 0.9 && p90 >= 0.9, "эскалация именно на хвосте");
}

/// FR-066 §5.7.4: рассинхрон версии движка в extra → stale = true.
#[cfg(feature = "qmc")]
#[test]
fn mcp_fr066_monte_carlo_run_stale_on_version_mismatch() {
    let (mut scene, ue) = fr066_etalon5_scene();
    let request = format!(
        r#"{{"runs": 256, "seed": 0, "params": {{"{ue}:new_customers": {{"dist": "normal", "mean": 3000, "sd": 100}}}}}}"#
    );
    // протухшие метаданные версии M4.9 (симуляция прогона старой версией)
    scene.canvas.extra.insert(
        "canvasdesk".to_owned(),
        serde_json::json!({"engine": {"version": "M4.9", "seed": "1", "stats": true, "parallel": true, "qmc": false}}),
    );
    let out = dispatch(&mut scene, "monte_carlo_run", &request).expect("прогон");
    assert_eq!(out["stale"], true, "версия M4.9 ≠ M5.0 — протух");
    // после прогона метаданные обновлены актуальной версией
    assert_eq!(
        scene.canvas.extra["canvasdesk"]["engine"]["version"],
        "M5.0"
    );
    let out2 = dispatch(&mut scene, "monte_carlo_run", &request).expect("повтор");
    assert_eq!(out2["stale"], false, "метаданные актуальны");
}

// --- FR-072: title в MCP-инструментах ---

/// FR-072: node_create_note с title — явный заголовок в canvasdesk;
/// без title — legacy-поведение (поле не создаётся).
#[test]
fn mcp_node_create_note_with_title() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0.0,"y":0.0,"text":"vm = 40 $","title":"Смета"}"#,
    )
    .expect("create_note с title");
    let node = scene.canvas.nodes.last().expect("нода");
    assert_eq!(node.title(), Some("Смета"), "явный заголовок записан");
    assert_eq!(node.text.as_deref(), Some("vm = 40 $"), "тело не тронуто");

    // Без title — поле не создаётся (legacy-фолбэк живёт)
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":10.0,"y":10.0,"text":"заметка"}"#,
    )
    .expect("create_note без title");
    let node = scene.canvas.nodes.last().expect("нода");
    assert_eq!(node.title(), None, "без title поле не создаётся");
}

/// FR-072: node_edit — title меняет заголовок, не трогая text;
/// title = null — сброс к legacy; другие поля обновляются независимо.
#[test]
fn mcp_node_edit_title_field() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","title":"Итоговая смета"}"#,
    )
    .expect("node_edit title");
    let node = &scene.canvas.nodes[0];
    assert_eq!(node.title(), Some("Итоговая смета"));
    assert_ne!(
        node.text.as_deref().map(str::trim),
        Some("Итоговая смета"),
        "текст не затронут правкой заголовка"
    );

    // Совместно с text: оба поля применяются
    dispatch(
        &mut scene,
        "node_edit",
        r#"{"id":"n1","text":"новое тело","title":"Новый заголовок"}"#,
    )
    .expect("node_edit text+title");
    let node = &scene.canvas.nodes[0];
    assert_eq!(node.text.as_deref(), Some("новое тело"));
    assert_eq!(node.title(), Some("Новый заголовок"));

    // title = null — сброс к legacy-фолбэку
    dispatch(&mut scene, "node_edit", r#"{"id":"n1","title":null}"#).expect("сброс title");
    assert_eq!(scene.canvas.nodes[0].title(), None, "title сброшен");

    // Неверный тип — ошибка вызова
    assert!(dispatch(&mut scene, "node_edit", r#"{"id":"n1","title":42}"#).is_err());
}

/// FR-072: nodes_search находит ноду по явному заголовку.
#[test]
fn mcp_nodes_search_by_title() {
    let mut scene = mcp_scene();
    dispatch(
        &mut scene,
        "node_create_note",
        r#"{"x":0.0,"y":0.0,"text":"vm = 40 $","title":"Смета на инфраструктуру"}"#,
    )
    .expect("create_note");
    let found = dispatch(&mut scene, "nodes_search", r#"{"query":"смета"}"#).expect("поиск");
    assert_eq!(
        found.as_array().map(Vec::len),
        Some(1),
        "поиск по заголовку (регистр не важен): {found}"
    );
}
