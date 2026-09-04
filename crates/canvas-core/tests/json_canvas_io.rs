//! T3: парсинг и lossless round-trip `.canvas` (JSON Canvas 1.0, SPEC §5.1).

use std::str::FromStr;

use canvas_core::{Canvas, NodeKind, PreviewState, Side};

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
    assert_eq!(widget.kind(), NodeKind::Unknown);
    assert_eq!(widget.broken_link, Some(true));
    let ext = widget.canvasdesk.as_ref().expect("объект canvasdesk");
    assert_eq!(ext.widget_id, "com.example.clock");
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
