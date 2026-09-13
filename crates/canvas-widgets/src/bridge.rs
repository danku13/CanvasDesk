//! Мост host↔widget (SPEC §7.6, план M5 §4.6): двусторонний JSON-RPC
//! (newline-free одиночные JSON-объекты) поверх WebView2 postMessage.
//! Все сообщения валидируются схемами; невалидное — drop + warn, не паника.

use crate::WidgetProps;
use serde::Deserialize;
use serde_json::{json, Value};

/// Тема для `init`/`themeChanged` (план M5 П8): минимум — dark + accent.
/// Сериализация — через `to_value` (в JSON-конверте сообщений).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeInfo {
    pub dark: bool,
    /// Акцент канваса, `#RRGGBB`.
    pub accent: String,
}

impl ThemeInfo {
    pub fn new(dark: bool, accent: impl Into<String>) -> Self {
        Self {
            dark,
            accent: accent.into(),
        }
    }

    fn to_value(&self) -> Value {
        json!({ "dark": self.dark, "accent": self.accent })
    }
}

/// Сообщения хоста виджету (уведомления, без `id`).
#[derive(Debug, Clone, PartialEq)]
pub enum HostToWidget {
    /// После готовности контроллера: идентификация + стартовые данные.
    Init {
        node_id: String,
        props: WidgetProps,
        theme: ThemeInfo,
        zoom: f32,
    },
    /// props изменились хостом (undo/MCP/повторная инициализация).
    PropsChanged {
        props: WidgetProps,
    },
    /// Переход live↔snapshot: виджет может экономить таймеры.
    Visibility {
        visible: bool,
    },
    ThemeChanged {
        theme: ThemeInfo,
    },
}

/// Сообщения виджета хосту. `ReadDir`/`StateGet`/`StateSet` — запросы
/// (отправляются с `id`, хост отвечает `reply` — см. `Reply`).
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetToHost {
    /// Виджет загрузился и готов принимать init-данные.
    Ready,
    /// Запрос размера ноды (кламп 160..2000 на стороне хоста).
    Resize { w: f32, h: f32 },
    /// Персистенция props в `.canvas` (undo-шаг, автосейв).
    SetProps { props: WidgetProps },
    /// Открыть файл в ассоциированном приложении (permission `shell:open`).
    OpenFile { path: String },
    /// Листинг allowlist-директории (permission `fs:read`, запрос).
    ReadDir { path: String },
    /// Уведомление (HUD-строка + лог).
    Toast { text: String },
    /// Объёмное состояние в SQLite `widget_state` (запрос).
    StateGet { key: String },
    /// Объёмное состояние в SQLite `widget_state` (запрос).
    StateSet { key: String, value: String },
}

/// Ответ хоста на запрос виджета (с тем же `id`, что был в запросе).
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub id: Value,
    pub result: Value,
}

impl Reply {
    /// Успешный ответ (readDir → entries; stateGet → value и т.п.).
    pub fn ok(id: Value, result: Value) -> Self {
        Self { id, result }
    }

    /// Ошибка (отказ permission, путь вне allowlist…): виджет обязан показать
    /// внятную деградацию, не падать.
    pub fn err(id: Value, message: impl Into<String>) -> Self {
        Self {
            id,
            result: json!({ "error": message.into() }),
        }
    }

    /// JSON-RPC-ответ целиком (одна строка для PostWebMessageAsJson).
    pub fn to_json(&self) -> String {
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": self.id,
            "result": self.result,
        }))
        .unwrap_or_default()
    }
}

impl WidgetToHost {
    /// Имя метода JSON-RPC (camelCase, как в SPEC §7.6).
    pub fn method(&self) -> &'static str {
        match self {
            WidgetToHost::Ready => "ready",
            WidgetToHost::Resize { .. } => "resize",
            WidgetToHost::SetProps { .. } => "setProps",
            WidgetToHost::OpenFile { .. } => "openFile",
            WidgetToHost::ReadDir { .. } => "readDir",
            WidgetToHost::Toast { .. } => "toast",
            WidgetToHost::StateGet { .. } => "stateGet",
            WidgetToHost::StateSet { .. } => "stateSet",
        }
    }
}

impl HostToWidget {
    /// Имя метода JSON-RPC.
    pub fn method(&self) -> &'static str {
        match self {
            HostToWidget::Init { .. } => "init",
            HostToWidget::PropsChanged { .. } => "propsChanged",
            HostToWidget::Visibility { .. } => "visibility",
            HostToWidget::ThemeChanged { .. } => "themeChanged",
        }
    }

    fn params(&self) -> Value {
        match self {
            HostToWidget::Init {
                node_id,
                props,
                theme,
                zoom,
            } => json!({
                "nodeId": node_id,
                "props": props,
                "theme": theme.to_value(),
                "zoom": zoom,
            }),
            HostToWidget::PropsChanged { props } => json!({ "props": props }),
            HostToWidget::Visibility { visible } => json!({ "visible": visible }),
            HostToWidget::ThemeChanged { theme } => json!({ "theme": theme.to_value() }),
        }
    }

    /// Сериализация в строку для `PostWebMessageAsJson` (host → widget).
    pub fn to_json(&self) -> String {
        serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "method": self.method(),
            "params": self.params(),
        }))
        .unwrap_or_default()
    }
}

/// Результат разбора входящего сообщения (drop + warn делает вызывающий).
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    /// Распознанное сообщение (без `id` — уведомление).
    Notification(WidgetToHost),
    /// Запрос: сообщение + идентификатор для ответа.
    Request { id: Value, call: WidgetToHost },
}

/// Ошибка разбора: невалидный JSON, не объект, нет method, params не по схеме.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BridgeError {
    #[error("не JSON / не объект: {0}")]
    NotJsonObject(String),
    #[error("нет поля method")]
    NoMethod,
    #[error("неизвестный метод: {0}")]
    UnknownMethod(String),
    #[error("params не соответствуют схеме метода {method}: {message}")]
    BadParams { method: String, message: String },
}

/// Разбор сообщения виджета → хост (строка из `WebMessageReceived`).
/// Неизвестный метод — `Err(UnknownMethod)` (вызывающий делает drop + warn,
/// не паникует — AGENTS). Запросы распознаются по наличию `id`.
pub fn parse_widget_message(raw: &str) -> Result<Parsed, BridgeError> {
    let value: Value =
        serde_json::from_str(raw).map_err(|e| BridgeError::NotJsonObject(e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| BridgeError::NotJsonObject("не объект".into()))?;
    let method = obj
        .get("method")
        .and_then(Value::as_str)
        .ok_or(BridgeError::NoMethod)?;
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    let id = obj.get("id").cloned();

    let call = widget_call_from(method, params)?;
    Ok(match id {
        Some(id) => Parsed::Request { id, call },
        None => Parsed::Notification(call),
    })
}

/// Строгая типизация params по методу.
fn widget_call_from(method: &str, params: Value) -> Result<WidgetToHost, BridgeError> {
    let bad = |message: String| BridgeError::BadParams {
        method: method.to_owned(),
        message,
    };
    let type_err = |e: serde_json::Error| bad(e.to_string());

    match method {
        "ready" => Ok(WidgetToHost::Ready),
        "resize" => {
            #[derive(Deserialize)]
            struct P {
                w: f32,
                h: f32,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::Resize { w: p.w, h: p.h })
        }
        "setProps" => {
            #[derive(Deserialize)]
            struct P {
                props: WidgetProps,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::SetProps { props: p.props })
        }
        "openFile" => {
            #[derive(Deserialize)]
            struct P {
                path: String,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::OpenFile { path: p.path })
        }
        "readDir" => {
            #[derive(Deserialize)]
            struct P {
                path: String,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::ReadDir { path: p.path })
        }
        "toast" => {
            #[derive(Deserialize)]
            struct P {
                text: String,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::Toast { text: p.text })
        }
        "stateGet" => {
            #[derive(Deserialize)]
            struct P {
                key: String,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::StateGet { key: p.key })
        }
        "stateSet" => {
            #[derive(Deserialize)]
            struct P {
                key: String,
                value: String,
            }
            let p: P = serde_json::from_value(params).map_err(type_err)?;
            Ok(WidgetToHost::StateSet {
                key: p.key,
                value: p.value,
            })
        }
        other => Err(BridgeError::UnknownMethod(other.to_owned())),
    }
}

/// Листинг readDir для ответа: только имена + is_dir, без атрибутов
/// (минимальная поверхность утечки; allowlist проверяется ДО вызова).
pub fn read_dir_entries(dir: &std::path::Path) -> Result<Value, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut names: Vec<Value> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        names.push(json!({ "name": name, "isDir": is_dir }));
    }
    names.sort_by(|a, b| {
        a.get("name")
            .and_then(Value::as_str)
            .cmp(&b.get("name").and_then(Value::as_str))
    });
    Ok(json!({ "entries": names }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> ThemeInfo {
        ThemeInfo::new(true, "#3B82F6")
    }

    #[test]
    fn host_to_widget_serialization_round_trip_shape() {
        let mut props = WidgetProps::new();
        props.insert("k".into(), Value::from(7));
        let msg = HostToWidget::Init {
            node_id: "n1".into(),
            props: props.clone(),
            theme: theme(),
            zoom: 0.5,
        };
        let json = msg.to_json();
        assert!(json.contains(r#""method":"init""#), "{json}");
        assert!(json.contains(r#""nodeId":"n1""#), "{json}");
        assert!(json.contains(r#""zoom":0.5"#), "{json}");
        assert!(json.contains("#3B82F6"), "{json}");
        // Разбор обратно не требуется (направление host→widget), но JSON валиден
        let v: Value = serde_json::from_str(&json).expect("валидный JSON");
        assert_eq!(v["jsonrpc"], "2.0");
    }

    #[test]
    fn visibility_and_theme_messages() {
        let msg = HostToWidget::Visibility { visible: false };
        assert!(msg.to_json().contains(r#""visible":false"#));
        let msg = HostToWidget::ThemeChanged { theme: theme() };
        assert!(msg.to_json().contains(r#""method":"themeChanged""#));
    }

    #[test]
    fn parse_all_widget_notifications() {
        let mut props = WidgetProps::new();
        props.insert("text".into(), Value::String("привет".into()));

        let cases: Vec<(String, WidgetToHost)> = vec![
            (
                r#"{"jsonrpc":"2.0","method":"ready"}"#.into(),
                WidgetToHost::Ready,
            ),
            (
                r#"{"jsonrpc":"2.0","method":"resize","params":{"w":320,"h":200}}"#.into(),
                WidgetToHost::Resize { w: 320.0, h: 200.0 },
            ),
            (
                format!(
                    r#"{{"jsonrpc":"2.0","method":"setProps","params":{{"props":{}}}}}"#,
                    serde_json::to_string(&props).unwrap()
                ),
                WidgetToHost::SetProps { props },
            ),
            (
                r#"{"jsonrpc":"2.0","method":"openFile","params":{"path":"C:/x.txt"}}"#.into(),
                WidgetToHost::OpenFile {
                    path: "C:/x.txt".into(),
                },
            ),
            (
                r#"{"jsonrpc":"2.0","method":"toast","params":{"text":"ой"}}"#.into(),
                WidgetToHost::Toast {
                    text: "ой".into()
                },
            ),
        ];
        for (raw, expected) in cases {
            let parsed = parse_widget_message(&raw).expect("парсинг");
            match parsed {
                Parsed::Notification(call) => assert_eq!(call, expected, "{raw}"),
                Parsed::Request { .. } => panic!("уведомление распознано как запрос: {raw}"),
            }
        }
    }

    #[test]
    fn parse_requests_with_id() {
        let parsed = parse_widget_message(
            r#"{"jsonrpc":"2.0","id":42,"method":"readDir","params":{"path":"docs"}}"#,
        )
        .expect("запрос readDir");
        match parsed {
            Parsed::Request { id, call } => {
                assert_eq!(id, Value::from(42));
                assert_eq!(
                    call,
                    WidgetToHost::ReadDir {
                        path: "docs".into()
                    }
                );
            }
            Parsed::Notification(_) => panic!("должен быть запрос"),
        }

        let parsed = parse_widget_message(
            r#"{"jsonrpc":"2.0","id":"abc","method":"stateGet","params":{"key":"draft"}}"#,
        )
        .expect("запрос stateGet");
        match parsed {
            Parsed::Request { id, call } => {
                assert_eq!(id, Value::String("abc".into()));
                assert_eq!(
                    call,
                    WidgetToHost::StateGet {
                        key: "draft".into()
                    }
                );
            }
            Parsed::Notification(_) => panic!("должен быть запрос"),
        }
    }

    #[test]
    fn unknown_method_is_error_not_panic() {
        let err = parse_widget_message(r#"{"jsonrpc":"2.0","method":"haxor","params":{}}"#)
            .expect_err("неизвестный метод");
        assert!(matches!(err, BridgeError::UnknownMethod(m) if m == "haxor"));
        // Вызывающий: drop + warn, без паники (AGENTS)
    }

    #[test]
    fn malformed_and_mismatched_messages() {
        assert!(parse_widget_message("не json").is_err());
        assert!(parse_widget_message("[1,2]").is_err());
        assert!(parse_widget_message(r#"{"params":{}}"#).is_err()); // нет method
                                                                    // params не по схеме: resize без чисел
        let err = parse_widget_message(r#"{"method":"resize","params":{"w":"широко","h":2}}"#)
            .expect_err("тип w неверен");
        assert!(matches!(err, BridgeError::BadParams { .. }), "{err}");
        // setProps с props не-объектом
        assert!(parse_widget_message(r#"{"method":"setProps","params":{"props":5}}"#).is_err());
        // toast без params вообще — ошибка схемы (строгий разбор)
        assert!(parse_widget_message(r#"{"method":"toast"}"#).is_err());
    }

    #[test]
    fn reply_serialization() {
        let ok = Reply::ok(Value::from(7), json!({ "entries": [] })).to_json();
        assert!(ok.contains(r#""id":7"#), "{ok}");
        assert!(ok.contains(r#""result""#), "{ok}");
        let err = Reply::err(Value::String("q".into()), "нет permission fs:read").to_json();
        assert!(err.contains("нет permission fs:read"), "{err}");
        assert!(err.contains(r#""id":"q""#), "{err}");
    }

    #[test]
    fn read_dir_entries_lists_sorted() {
        let dir = std::env::temp_dir().join("canvasdesk_bridge_test_entries");
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.txt"), b"").unwrap();
        std::fs::create_dir_all(dir.join("a-sub")).unwrap();
        let value = read_dir_entries(&dir).expect("листинг");
        let entries = value["entries"].as_array().expect("массив");
        assert_eq!(entries.len(), 2);
        // Сортировка по имени: a-sub, b.txt
        assert_eq!(entries[0]["name"], "a-sub");
        assert_eq!(entries[0]["isDir"], true);
        assert_eq!(entries[1]["name"], "b.txt");
        assert_eq!(entries[1]["isDir"], false);
        std::fs::remove_dir_all(&dir).ok();
    }
}
