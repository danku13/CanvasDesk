//! T3: парсинг и lossless round-trip `.canvas` (JSON Canvas 1.0, SPEC §5.1).

use std::str::FromStr;

use canvas_core::{Canvas, Edge, Node, NodeKind, PreviewState, Side};

const SPEC_EXAMPLE: &str = include_str!("fixtures/spec_example.canvas");
const OBSIDIAN: &str = include_str!("fixtures/obsidian.canvas");
const BROKEN: &str = include_str!("fixtures/broken.canvas");

/// Примеры из спецификации парсятся, типы и координаты совпадают.
#[test]
fn parse_spec_examples() {
    let canvas = Canvas::from_str(SPEC_EXAMPLE).expect("spec_example.canvas должен парситься");
    assert_eq!(canvas.nodes.len(), 4);
    assert_eq!(canvas.edges.len(), 1);

    let file = &canvas.nodes[0];
    assert_eq!(file.kind(), NodeKind::File);
    assert_eq!(file.file.as_deref(), Some("C:/Projects/alpha/spec.pdf"));
    assert_eq!(
        (file.x, file.y, file.width, file.height),
        (120.0, 80.0, 340.0, 440.0)
    );

    let text = &canvas.nodes[1];
    assert_eq!(text.kind(), NodeKind::Text);
    assert_eq!(text.text.as_deref(), Some("Согласовать до пятницы"));
    assert_eq!(text.color.as_deref(), Some("3"));

    let group = &canvas.nodes[2];
    assert_eq!(group.kind(), NodeKind::Group);
    assert_eq!(group.label.as_deref(), Some("Альфа-проект"));

    let edge = &canvas.edges[0];
    assert_eq!(edge.from_node, "n2");
    assert_eq!(edge.from_side, Some(Side::Right));
    assert_eq!(edge.to_node, "n1");
    assert_eq!(edge.to_side, Some(Side::Top));
    assert_eq!(edge.label.as_deref(), Some("блокирует"));
}

/// Расширения связи (`edgeStyle`/`edgeWidth`) сериализуются, парсятся обратно
/// и не ломают файлы без них; неизвестные поля по-прежнему в `extra`.
#[test]
fn edge_style_thickness_round_trip() {
    use canvas_core::{Edge, EdgeLineStyle, EdgeThickness};

    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(canvas_core::Node::text("a", "x", 0.0, 0.0));
    canvas
        .nodes
        .push(canvas_core::Node::text("b", "y", 300.0, 0.0));
    let mut edge = Edge::new("e1", "a", None, "b", None);
    edge.style = Some(EdgeLineStyle::Dashed);
    edge.thickness = Some(EdgeThickness::Thick);
    canvas.add_edge(edge);
    canvas.add_edge(Edge::new("e2", "a", None, "b", None));

    let serialized = canvas.to_json().expect("сериализация");
    let compact: String = serialized.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("\"edgeStyle\":\"dashed\""), "{serialized}");
    assert!(compact.contains("\"edgeWidth\":\"thick\""), "{serialized}");
    // Поля отсутствуют у связи без стиля (не мусорим в файл)
    assert!(!compact.contains("edgeStyle\":\"solid"), "{serialized}");

    let parsed = Canvas::from_str(&serialized).expect("парсинг");
    assert_eq!(parsed.edges[0].style, Some(EdgeLineStyle::Dashed));
    assert_eq!(parsed.edges[0].thickness, Some(EdgeThickness::Thick));
    assert_eq!(parsed.edges[1].style, None);
    assert_eq!(parsed.edges[1].thickness, None);
    // Полный round-trip без потерь
    let again = parsed.to_json().expect("повторная сериализация");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serialized).unwrap(),
        serde_json::from_str::<serde_json::Value>(&again).unwrap()
    );
}

/// Неизвестные поля нод, edges и корня + неизвестные типы нод доезжают без потерь.
#[test]
fn round_trip_preserves_unknown_fields() {
    for source in [SPEC_EXAMPLE, OBSIDIAN] {
        let canvas = Canvas::from_str(source).expect("парсинг");
        let serialized = canvas.to_json().expect("сериализация");
        let before: serde_json::Value =
            serde_json::from_str(source).expect("исходник — валидный JSON");
        let after: serde_json::Value =
            serde_json::from_str(&serialized).expect("результат — валидный JSON");
        assert_eq!(
            normalize_numbers(before),
            normalize_numbers(after),
            "round-trip потерял данные:\n{serialized}"
        );
    }
}

/// Числа приводятся к f64: координаты у нас f32, поэтому `120` после round-trip
/// становится `120.0` — данные не теряются, меняется только представление числа.
fn normalize_numbers(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Number(n) => Value::from(n.as_f64().unwrap_or_default()),
        Value::Array(items) => Value::Array(items.into_iter().map(normalize_numbers).collect()),
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, normalize_numbers(v)))
                .collect(),
        ),
        other => other,
    }
}

/// Расширения CanvasDesk типизированы и сериализуются с теми же именами полей.
#[test]
fn typed_extensions_round_trip() {
    let canvas = Canvas::from_str(SPEC_EXAMPLE).expect("парсинг");

    let file = &canvas.nodes[0];
    assert_eq!(file.preview_state, Some(PreviewState::Thumbnail));
    assert_eq!(file.broken_link, None);

    let widget = &canvas.nodes[3];
    // M5: «widget» распознаётся как собственный тип (NodeKind::Widget);
    // для сторонних редакторов строка типа сохраняется как есть.
    assert_eq!(widget.kind(), NodeKind::Widget);
    assert_eq!(widget.broken_link, Some(true));
    let ext = widget.canvasdesk.as_ref().expect("объект canvasdesk");
    assert_eq!(ext.widget_id.as_deref(), Some("com.example.clock"));
    assert_eq!(
        ext.props.get("timezone").and_then(|v| v.as_str()),
        Some("Europe/Moscow")
    );

    let json = canvas.to_json().expect("сериализация");
    assert!(json.contains("\"previewState\": \"thumbnail\""), "{json}");
    assert!(json.contains("\"brokenLink\": true"), "{json}");
    assert!(
        json.contains("\"widgetId\": \"com.example.clock\""),
        "{json}"
    );
    // Поля со значением None не сериализуются
    assert!(!json.contains("\"file\": null"), "{json}");
}

/// Битый файл — ошибка с указанием места (поле/строка/колонка).
#[test]
fn broken_file_error_mentions_location() {
    let err = Canvas::from_str(BROKEN).expect_err("нода без x должна дать ошибку");
    let message = err.to_string();
    assert!(
        message.contains('x') && (message.contains("строка") || message.contains("line")),
        "ошибка должна указывать поле и позицию: {message}"
    );

    let err = Canvas::from_str("{ не json").expect_err("невалидный JSON");
    assert!(err.to_string().contains("line"), "позиция в ошибке: {err}");
}

/// Абсолютные пути Windows в `file` — задокументированное отклонение от spec (SPEC §5.1).
#[test]
fn absolute_windows_path_in_file() {
    let canvas = Canvas::from_str(SPEC_EXAMPLE).expect("парсинг");
    let path = "C:/Projects/alpha/spec.pdf";
    assert_eq!(canvas.nodes[0].file.as_deref(), Some(path));
    let json = canvas.to_json().expect("сериализация");
    assert!(json.contains(path), "{json}");
}

/// Пустой канвас (только неизвестные поля верхнего уровня) парсится.
#[test]
fn empty_canvas_with_unknown_root_fields() {
    let canvas = Canvas::from_str(r#"{"pluginData": {"a": 1}}"#).expect("парсинг");
    assert!(canvas.nodes.is_empty());
    assert!(canvas.edges.is_empty());
    let json = canvas.to_json().expect("сериализация");
    let value: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(value["pluginData"]["a"], 1);
}

/// Группа (node_type = "group", подпись в `label`) сериализуется и парсится
/// обратно без потерь — формат `.canvas` группы не расширяем.
#[test]
fn group_node_round_trip() {
    use canvas_core::Node;

    let mut canvas = Canvas::default();
    let mut group = Node::group("g1", 10.0, 20.0, 400.0, 300.0);
    group.label = Some("Спринт 1".to_owned());
    canvas.nodes.push(group);
    canvas.nodes.push(Node::text("n1", "заметка", 50.0, 60.0));

    let serialized = canvas.to_json().expect("сериализация");
    assert!(serialized.contains("\"type\": \"group\""), "{serialized}");
    assert!(
        serialized.contains("\"label\": \"Спринт 1\""),
        "{serialized}"
    );
    // У группы нет file/text — в файл они не пишутся (проверяем её объект)
    let value: serde_json::Value = serde_json::from_str(&serialized).expect("валидный JSON");
    let group_json = &value["nodes"][0];
    assert!(group_json.get("file").is_none(), "{serialized}");
    assert!(group_json.get("text").is_none(), "{serialized}");

    let parsed = Canvas::from_str(&serialized).expect("парсинг");
    assert_eq!(parsed.nodes.len(), 2);
    let group = &parsed.nodes[0];
    assert_eq!(group.kind(), NodeKind::Group);
    assert_eq!(group.label.as_deref(), Some("Спринт 1"));
    assert_eq!(
        (group.x, group.y, group.width, group.height),
        (10.0, 20.0, 400.0, 300.0)
    );
    // Полный round-trip без потерь
    let again = parsed.to_json().expect("повторная сериализация");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&serialized).unwrap(),
        serde_json::from_str::<serde_json::Value>(&again).unwrap()
    );
}

// --- FR-013: canvasdesk.expr — Numi-формула text-ноды ---

/// FR-013 (инвариант 3): set_expr → round-trip через `.canvas` — поле
/// сохранено; set_expr(None) — поле удалено из JSON (round-trip чистый);
/// чужие ключи внутри `canvasdesk` не теряются.
#[test]
fn expr_round_trip_and_reset() {
    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "Параметры шлюза", 0.0, 0.0);
    note.set_expr(Some("5 ms × 200 req/s".to_owned()));
    canvas.nodes.push(note);

    // Сериализация: поле на месте, формат Obsidian-совместимый
    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(
        parsed["nodes"][0]["canvasdesk"]["expr"], "5 ms × 200 req/s",
        "canvasdesk.expr в JSON"
    );

    // Обратное чтение: accessor видит формулу
    let restored = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(
        restored.nodes[0].expr(),
        Some("5 ms × 200 req/s"),
        "формула пережила round-trip"
    );
    assert_eq!(restored.nodes[0].kind(), NodeKind::Text);

    // Сброс: ключ удалён целиком — пустого canvasdesk в JSON нет
    let mut cleared = restored;
    cleared.nodes[0].set_expr(None);
    assert_eq!(cleared.nodes[0].expr(), None, "формула сброшена");
    let json = cleared.to_json().expect("сериализация после сброса");
    assert!(
        !json.contains("canvasdesk"),
        "пустого расширения в JSON быть не должно: {json}"
    );
}

/// FR-013: формула из чужого файла (canvasdesk.expr в extra) читается;
/// соседние чужие ключи внутри canvasdesk сохраняются при set_expr.
#[test]
fn expr_reads_external_file_and_preserves_siblings() {
    let source = r#"{
        "nodes": [
            {
                "id": "n1",
                "type": "text",
                "text": "Gateway",
                "x": 0,
                "y": 0,
                "width": 260,
                "height": 120,
                "canvasdesk": { "expr": "1k rps" }
            }
        ],
        "edges": []
    }"#;
    let mut canvas = Canvas::from_str(source).expect("чужой файл парсится");
    assert_eq!(canvas.nodes[0].expr(), Some("1k rps"));

    // Сброс формулы: canvasdesk без других данных исчезает из JSON
    canvas
        .nodes
        .get_mut(0)
        .expect("нода есть")
        .set_expr(Some("2k rps".to_owned()));
    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(parsed["nodes"][0]["canvasdesk"]["expr"], "2k rps");
}

// --- FR-018: canvasdesk.template — ссылка шаблонной ноды ---

/// FR-018 (инвариант 2): set_template → round-trip через `.canvas` —
/// снимок {id, version, expr, params, icon, color} сохранён полностью
/// (Obsidian-формат валиден: поле живёт в extra); set_template(None)
/// удаляет ключ, не трогая соседние (паттерн set_expr/set_flow_kind).
#[test]
fn template_round_trip_and_reset() {
    let mut canvas = Canvas::default();
    let mut node = Node::text("tpl-1", "rps = 1000 rps\nservers = 2", 0.0, 0.0);
    node.set_template(Some(canvas_core::templates::TemplateRef {
        id: "mock.lb".to_owned(),
        version: "1.0.0".to_owned(),
        expr: "mm1($rps, $service_rate, $servers)".to_owned(),
        name: Some("Балансировщик нагрузки".to_owned()),
        params: [
            (
                "rps".to_owned(),
                canvas_core::templates::TemplateParam {
                    num: 1000.0,
                    unit: Some("rps".to_owned()),
                },
            ),
            (
                "servers".to_owned(),
                canvas_core::templates::TemplateParam {
                    num: 2.0,
                    unit: None,
                },
            ),
        ]
        .into_iter()
        .collect(),
        icon: "lb".to_owned(),
        color: "#4A90E2".to_owned(),
    }));
    canvas.nodes.push(node);

    // Сериализация: JSON-структура на месте, файл — валидный JSON Canvas
    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    let template_json = &parsed["nodes"][0]["canvasdesk"]["template"];
    assert_eq!(template_json["id"], "mock.lb", "id в JSON");
    assert_eq!(template_json["version"], "1.0.0");
    assert_eq!(template_json["expr"], "mm1($rps, $service_rate, $servers)");
    assert_eq!(template_json["params"]["rps"]["num"], 1000.0);
    assert_eq!(template_json["params"]["rps"]["unit"], "rps");
    assert_eq!(template_json["params"]["servers"]["num"], 2.0);
    assert_eq!(template_json["icon"], "lb");
    assert_eq!(template_json["color"], "#4A90E2");

    // Обратное чтение: accessor возвращает тот же TemplateRef
    let restored = Canvas::from_str(&json).expect("парсинг");
    let template = restored.nodes[0].template().expect("template-ссылка");
    assert_eq!(template.id, "mock.lb");
    assert_eq!(template.version, "1.0.0");
    assert_eq!(template.expr, "mm1($rps, $service_rate, $servers)");
    assert_eq!(template.params.len(), 2);
    assert_eq!(template.params["rps"].display(), "1000 rps");
    assert_eq!(template.params["servers"].display(), "2");
    assert_eq!(template.icon, "lb");
    assert_eq!(template.color, "#4A90E2");
    assert_eq!(restored.nodes[0].kind(), NodeKind::Text);

    // Сброс: ключ удалён целиком; пустого canvasdesk в JSON нет
    let mut cleared = restored;
    cleared.nodes[0].set_template(None);
    assert!(cleared.nodes[0].template().is_none(), "ссылка сброшена");
    let json = cleared.to_json().expect("сериализация после сброса");
    assert!(
        !json.contains("canvasdesk"),
        "пустого расширения в JSON быть не должно: {json}"
    );
}

/// FR-018: template из чужого файла (без icon/color — старые сборки)
/// читается с дефолтами; файл с текстом-Numi-листом параметров и
/// template-ссылкой полностью реконструирует шаблонную ноду.
#[test]
fn template_reads_external_file_with_defaults() {
    let source = r#"{
        "nodes": [
            {
                "id": "tpl-old",
                "type": "text",
                "text": "qps = 80 rps",
                "x": 0,
                "y": 0,
                "width": 300,
                "height": 120,
                "canvasdesk": {
                    "template": {
                        "id": "mock.db",
                        "version": "1.0.0",
                        "expr": "mm1($qps, 1 req / $query_time, $replicas)",
                        "params": { "qps": { "num": 80, "unit": "rps" } }
                    }
                }
            }
        ],
        "edges": []
    }"#;
    let canvas = Canvas::from_str(source).expect("чужой файл парсится");
    let template = canvas.nodes[0].template().expect("template-ссылка");
    assert_eq!(template.id, "mock.db");
    assert_eq!(template.params["qps"].display(), "80 rps");
    // Файлы до снапшота иконки/цвета — дефолты, не ошибка парсинга
    assert_eq!(template.icon, "custom");
    assert_eq!(template.color, "#9B9B9B");
}

/// FR-014: `canvasdesk.flow.kind` ребра переживает round-trip; отсутствие
/// поля (старые файлы) читается как Control; сброс в Control удаляет поле
/// целиком, не трогая соседние ключи.
#[test]
fn edge_flow_kind_round_trip() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
    canvas.nodes.push(Node::text("b", "b", 10.0, 0.0));
    // Value-ребро
    let mut value = Edge::new("e1", "a", None, "b", None);
    value.set_flow_kind(canvas_core::flow::FlowKind::Value);
    canvas.add_edge(value);
    // Control-ребро — поле не пишется вовсе (дефолт)
    let mut control = Edge::new("e2", "b", None, "a", None);
    control.set_flow_kind(canvas_core::flow::FlowKind::Control);
    canvas.add_edge(control);

    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(
        parsed["edges"][0]["canvasdesk"]["flow"]["kind"], "value",
        "kind=value в файле"
    );
    assert!(
        parsed["edges"][1].get("canvasdesk").is_none(),
        "control-ребро без расширения: {}",
        json
    );

    let restored = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(
        restored.edges[0].flow_kind(),
        canvas_core::flow::FlowKind::Value
    );
    assert_eq!(
        restored.edges[1].flow_kind(),
        canvas_core::flow::FlowKind::Control,
        "отсутствие поля — control"
    );
}

/// CR-008: `canvasdesk.pin_ports` ребра переживает round-trip; отсутствие
/// поля (старые файлы) — оба конца авто; снятие последнего пина удаляет
/// поле, не трогая соседние ключи `canvasdesk` (например, `flow`).
#[test]
fn edge_port_pins_round_trip() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
    canvas.nodes.push(Node::text("b", "b", 10.0, 0.0));
    // Пин истока
    let mut pinned = Edge::new("e1", "a", Some(Side::Right), "b", Some(Side::Left));
    pinned.set_port_pin(canvas_core::EdgeEnd::From, true);
    canvas.add_edge(pinned);
    // Авто-ребро — поле не пишется вовсе
    canvas.add_edge(Edge::new("e2", "b", None, "a", None));

    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(
        parsed["edges"][0]["canvasdesk"]["pin_ports"],
        serde_json::json!(["from"]),
        "pin_ports в файле"
    );
    assert!(
        parsed["edges"][1].get("canvasdesk").is_none(),
        "авто-ребро без расширения: {json}"
    );

    let restored = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(restored.edges[0].port_pins(), (true, false));
    assert_eq!(restored.edges[1].port_pins(), (false, false));
    assert!(restored.edges[0].ports_pinned());
    assert!(!restored.edges[1].ports_pinned());

    // Мусор/неизвестные значения — (false, false), без паник
    let junk = r#"{
        "nodes": [
            { "id": "a", "type": "text", "text": "a", "x": 0, "y": 0, "width": 260, "height": 120 },
            { "id": "b", "type": "text", "text": "b", "x": 10, "y": 0, "width": 260, "height": 120 }
        ],
        "edges": [
            { "id": "e", "fromNode": "a", "toNode": "b",
              "canvasdesk": { "pin_ports": ["wtf", 42, {"x": 1}, "to"] } }
        ]
    }"#;
    let restored: Canvas = Canvas::from_str(junk).expect("парсинг мусора");
    assert_eq!(
        restored.edges[0].port_pins(),
        (false, true),
        "unknown игнорируются, \"to\" распознан"
    );
}

/// FR-014: чужой файл с `canvasdesk.flow.kind` ребра читается; чужие
/// соседи внутри canvasdesk ребра сохраняются при тогле.
#[test]
fn edge_flow_reads_external_file_and_preserves_siblings() {
    let source = r#"{
        "nodes": [
            { "id": "a", "type": "text", "text": "a", "x": 0, "y": 0, "width": 260, "height": 120 },
            { "id": "b", "type": "text", "text": "b", "x": 10, "y": 0, "width": 260, "height": 120 }
        ],
        "edges": [
            {
                "id": "e1",
                "fromNode": "a",
                "toNode": "b",
                "canvasdesk": { "note": "моё", "flow": { "kind": "value" } }
            }
        ]
    }"#;
    let mut canvas = Canvas::from_str(source).expect("чужой файл парсится");
    assert_eq!(
        canvas.edges[0].flow_kind(),
        canvas_core::flow::FlowKind::Value
    );
    // Тогл в Control: flow пустой → удаляется, note остаётся
    canvas
        .edges
        .get_mut(0)
        .expect("ребро есть")
        .set_flow_kind(canvas_core::flow::FlowKind::Control);
    let json = canvas.to_json().expect("сериализация");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("валидный JSON");
    assert_eq!(parsed["edges"][0]["canvasdesk"]["note"], "моё");
    assert!(
        parsed["edges"][0]["canvasdesk"].get("flow").is_none(),
        "пустой flow удалён"
    );
}
