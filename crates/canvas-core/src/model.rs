//! Модель данных `.canvas` (JSON Canvas 1.0, SPEC §5.1).
//!
//! Совместимость с jsoncanvas.org: неизвестные поля нод, связей и корня
//! сохраняются в `extra` (serde flatten) и не теряются при round-trip;
//! неизвестные типы нод не ломают парсинг (`node_type` — строка).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Сторона ноды для привязки связи (JSON Canvas: `top`/`right`/`bottom`/`left`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

/// Тип ноды, выведенный из строки `type` (JSON Canvas 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Text,
    Link,
    Group,
    /// Неизвестный тип (например, `widget` из M5) — нода сохраняется как есть.
    Unknown,
}

/// Последний уровень детализации ноды — расширение `previewState` (SPEC §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewState {
    Thumbnail,
    Live,
    None,
}

/// Расширение `canvasdesk` ноды-виджета (M5, SPEC §5.1/§7.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasdeskExt {
    #[serde(rename = "widgetId")]
    pub widget_id: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
}

/// Нода канваса. `node_type` — строкой, чтобы неизвестные типы (widget и будущие)
/// не ломали парсинг (SPEC §5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    /// Путь к файлу. Отклонение от spec: допускаются абсолютные пути Windows (SPEC §5.1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Цвет по JSON Canvas spec: пресет "1".."6" или "#RRGGBB".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Расширение: файл недоступен — карточка с серой рамкой (SPEC §5.1).
    #[serde(rename = "brokenLink", skip_serializing_if = "Option::is_none")]
    pub broken_link: Option<bool>,
    /// Расширение: последний уровень детализации ноды (SPEC §5.1).
    #[serde(rename = "previewState", skip_serializing_if = "Option::is_none")]
    pub preview_state: Option<PreviewState>,
    /// Расширение: данные виджет-ноды (M5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvasdesk: Option<CanvasdeskExt>,
    /// Неизвестные поля — сохраняются при round-trip (совместимость с Obsidian).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Node {
    /// Тип ноды по строке `type`.
    pub fn kind(&self) -> NodeKind {
        match self.node_type.as_str() {
            "file" => NodeKind::File,
            "text" => NodeKind::Text,
            "link" => NodeKind::Link,
            "group" => NodeKind::Group,
            _ => NodeKind::Unknown,
        }
    }

    /// Текстовая нода-заметка.
    pub fn text(id: impl Into<String>, text: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            id: id.into(),
            node_type: "text".to_owned(),
            file: None,
            text: Some(text.into()),
            label: None,
            color: None,
            x,
            y,
            width: 260.0,
            height: 120.0,
            broken_link: None,
            preview_state: None,
            canvasdesk: None,
            extra: Map::new(),
        }
    }

    /// Файловая нода.
    pub fn file(
        id: impl Into<String>,
        file: impl Into<String>,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            id: id.into(),
            node_type: "file".to_owned(),
            file: Some(file.into()),
            text: None,
            label: None,
            color: None,
            x,
            y,
            width,
            height,
            broken_link: None,
            preview_state: None,
            canvasdesk: None,
            extra: Map::new(),
        }
    }
}

/// Связь между нодами.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide", skip_serializing_if = "Option::is_none")]
    pub from_side: Option<Side>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide", skip_serializing_if = "Option::is_none")]
    pub to_side: Option<Side>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Неизвестные поля (fromEnd/toEnd и пр.) — сохраняются при round-trip.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// Корень `.canvas`-файла.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    /// Неизвестные поля верхнего уровня — сохраняются при round-trip.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Canvas {
    /// Найти ноду по id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }
}
