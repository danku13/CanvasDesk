//! canvas-widgets — движок виджетов (M5): манифесты, bridge, LOD-планировщик,
//! реестр пакетов и WebView2-хост (Windows). Архитектура — docs/plans/M5-widgets.md.
//!
//! Правила размещения (план M5 §4.1): вся чистая логика кроссплатформенна и
//! тестируется на Linux; Win32/WebView2 — `host/` под `cfg(windows)`.
//! Зависимость от rusqlite запрещена (см. Cargo.toml).

pub mod bridge;
pub mod host_types;
pub mod layout;
pub mod lod;
pub mod manifest;
pub mod permissions;
pub mod registry;
pub mod snapshot;

#[cfg(windows)]
pub mod host;

pub use bridge::{HostToWidget, Parsed, Reply, ThemeInfo, WidgetToHost};
pub use host_types::{FrameApplication, HostError, LiveRequest, WidgetEventSender};

use serde_json::Map;

/// Событие из мира виджетов в приложение (по образцу `AppEvent`).
/// Кроссплатформенный тип: на Linux события генерируются только тестами
/// (host-реализация Windows-only), менеджер приложения обрабатывает единообразно.
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetEvent {
    /// Создание среды WebView2 завершено (ок/ошибка; при ошибке виджеты
    /// деградируют в placeholder с понятной ошибкой — SPEC §9).
    EnvironmentReady { ok: bool },
    /// Асинхронное создание WebView2-контроллера завершено (ок/ошибка).
    ControllerReady { node_id: String, ok: bool },
    /// Снапшот виджета готов (CapturePreview → PNG → RGBA).
    SnapshotReady {
        node_id: String,
        snapshot: snapshot::WidgetSnapshot,
    },
    /// Сообщение виджета по мосту (WebMessageReceived).
    Message {
        node_id: String,
        message: bridge::WidgetToHost,
    },
    /// Тик таймера (1 c): хост проверяет, кому пора освежить снапшот.
    Tick,
}

/// Props виджета — JSON-объект из `canvasdesk.props` (SPEC §5.1).
pub type WidgetProps = Map<String, serde_json::Value>;
