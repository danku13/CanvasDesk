//! Ошибки canvas-core: thiserror на библиотечной границе (AGENTS.md).

/// Ошибка ядра: I/O, формат `.canvas`, платформенные сервисы.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),
    #[error("ошибка формата .canvas (line {line}, column {column}): {message}")]
    Parse {
        line: usize,
        column: usize,
        message: String,
    },
    #[error("платформенная ошибка: {0}")]
    Platform(String),
}

impl CoreError {
    /// Обернуть ошибку serde_json с позицией (строка/колонка).
    pub(crate) fn from_parse_error(err: serde_json::Error) -> Self {
        CoreError::Parse {
            line: err.line(),
            column: err.column(),
            message: err.to_string(),
        }
    }
}
