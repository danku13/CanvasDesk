//! Приёмочные тесты публичного API canvas-core (TDD, задача T0).
//! Спецификация поведения: docs/SPEC.md §5.1 (JSON Canvas 1.0) и §4 (трейты).

use std::path::Path;

use canvas_core::{
    Canvas, CoreError, Edge, Node, PreviewProvider, ShellIntegration, Side, Thumbnail,
    ThumbnailProvider,
};

/// Canvas с нодами и связями переживает round-trip через serde_json без потерь.
#[test]
fn canvas_round_trip() {
    let canvas = Canvas {
        nodes: vec![Node::file(
            "n1",
            "C:/Projects/alpha/spec.pdf",
            120.0,
            80.0,
            340.0,
            440.0,
        )],
        edges: vec![Edge {
            id: "e1".into(),
            from_node: "n1".into(),
            from_side: Some(Side::Right),
            to_node: "n2".into(),
            to_side: Some(Side::Top),
            label: Some("блокирует".into()),
            color: None,
            style: None,
            thickness: None,
            from_line: None,
            from_output: None,
            to_param: None,
            extra: Default::default(),
        }],
        extra: Default::default(),
    };

    let json = serde_json::to_string_pretty(&canvas).expect("сериализация");
    let restored: Canvas = serde_json::from_str(&json).expect("десериализация");
    assert_eq!(canvas, restored);
}

/// Имена полей в JSON соответствуют JSON Canvas spec, а не rust-идентификаторам.
#[test]
fn uses_json_canvas_field_names() {
    let mut node = Node::text("n1", "заметка", 0.0, 0.0);
    node.width = 100.0;
    node.height = 50.0;
    node.color = Some("3".into());
    let value = serde_json::to_value(&node).expect("сериализация ноды");
    let obj = value.as_object().expect("нода — JSON-объект");
    for key in ["id", "type", "x", "y", "width", "height", "text", "color"] {
        assert!(obj.contains_key(key), "нет ключа {key}");
    }
    assert!(!obj.contains_key("node_type"), "rust-имя утекло в JSON");

    let edge = Edge {
        id: "e1".into(),
        from_node: "n1".into(),
        from_side: Some(Side::Right),
        to_node: "n2".into(),
        to_side: None,
        label: None,
        color: None,
        style: None,
        thickness: None,
        from_line: None,
        from_output: None,
        to_param: None,
        extra: Default::default(),
    };
    let value = serde_json::to_value(&edge).expect("сериализация связи");
    let obj = value.as_object().expect("связь — JSON-объект");
    assert!(obj.contains_key("fromNode"));
    assert!(obj.contains_key("fromSide"));
    assert!(obj.contains_key("toNode"));
    assert_eq!(obj["fromSide"], serde_json::json!("right"));
}

/// Пустые Option-поля не сериализуются — файл остаётся чистым.
#[test]
fn optional_fields_omitted_when_none() {
    let node = Node {
        id: "n1".into(),
        node_type: "group".into(),
        file: None,
        text: None,
        label: None,
        color: None,
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
        broken_link: None,
        preview_state: None,
        canvasdesk: None,
        collapsed: None,
        children: None,
        extra: Default::default(),
    };
    let json = serde_json::to_string(&node).expect("сериализация");
    for key in ["file", "text", "label", "color"] {
        assert!(!json.contains(key), "пустое поле {key} попало в JSON");
    }
}

struct MockThumbs;
impl ThumbnailProvider for MockThumbs {
    fn thumbnail(&self, _path: &Path, _max_size: u32) -> Result<Thumbnail, CoreError> {
        Ok(Thumbnail {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        })
    }
}

struct MockPreview;
impl PreviewProvider for MockPreview {
    fn preview(&self, _path: &Path, _w: u32, _h: u32) -> Result<Thumbnail, CoreError> {
        Err(CoreError::Platform("нет handler'а".into()))
    }
}

struct MockShell;
impl ShellIntegration for MockShell {
    fn open_file(&self, _path: &Path) -> Result<(), CoreError> {
        Ok(())
    }
}

/// Трейты платформенных сервисов object-safe: работают через Box<dyn ...>.
#[test]
fn provider_traits_are_object_safe() {
    let thumbs: Box<dyn ThumbnailProvider> = Box::new(MockThumbs);
    let thumb = thumbs.thumbnail(Path::new("a.png"), 256).expect("тамбнейл");
    assert_eq!(thumb.rgba.len(), 4);

    let preview: Box<dyn PreviewProvider> = Box::new(MockPreview);
    assert!(preview.preview(Path::new("a.docx"), 100, 100).is_err());

    let shell: Box<dyn ShellIntegration> = Box::new(MockShell);
    shell.open_file(Path::new("a.txt")).expect("open_file");
}

// --- FR-072: canvasdesk.title — явный заголовок ноды ---

/// FR-072: приоритет имени Σ-строки — явный заголовок → имя шаблона →
/// первая строка текста (заголовок и legacy-фолбэк не конфликтуют).
#[test]
fn sigma_row_name_prefers_explicit_title() {
    let mut note = canvas_core::Node::text("n1", "vm = 40 $\ndb = 25 $", 0.0, 0.0);
    assert_eq!(note.sigma_row_name(), "vm = 40 $", "legacy: первая строка");

    note.set_title(Some("Смета".to_owned()));
    assert_eq!(note.sigma_row_name(), "Смета", "явный заголовок выигрывает");

    note.set_title(Some(String::new()));
    assert_eq!(
        note.sigma_row_name(),
        "vm = 40 $",
        "пустой явный заголовок — фолбэк к первой строке"
    );
}

/// FR-072: remove_first_line — перенос первой строки тела в заголовок.
#[test]
fn remove_first_line_variants() {
    let mut note = canvas_core::Node::text("n1", "Смета\nvm = 40 $", 0.0, 0.0);
    assert_eq!(note.remove_first_line(), Some("Смета".to_owned()));
    assert_eq!(note.text.as_deref(), Some("vm = 40 $"));

    // Единственная строка — текст становится пустым
    let mut single = canvas_core::Node::text("n2", "только заголовок", 0.0, 0.0);
    assert_eq!(
        single.remove_first_line(),
        Some("только заголовок".to_owned())
    );
    assert_eq!(single.text.as_deref(), Some(""));

    // Пустой/отсутствующий текст — ничего не удаляется
    let mut empty = canvas_core::Node::text("n3", "", 0.0, 0.0);
    assert_eq!(empty.remove_first_line(), None);
    let mut none = canvas_core::Node::text("n4", "", 0.0, 0.0);
    none.text = None;
    assert_eq!(none.remove_first_line(), None);
}
