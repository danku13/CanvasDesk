//! Ошибки canvas-core: thiserror на библиотечной границе (AGENTS.md).

/// Ошибка ядра: I/O, формат `.canvas`, платформенные сервисы.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
    #[error("ошибка формата .canvas: {0}")]
    Json(#[from] serde_json::Error),
    #[error("платформенная ошибка: {0}")]
    Platform(String),
}
