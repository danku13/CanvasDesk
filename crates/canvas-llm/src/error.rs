//! FR-LLM-A / ADR-0016 §4.5 — ошибки LLM-слоя.
//!
//! Единый тип ошибок для всех `LlmProvider`-реализаций. Варианты
//! покрывают все классы отказов, описанные в PRD-0010 NF-5 (fallback):
//! транспортные, протокольные, аутентификация, rate limits и
//! «операция не поддерживается данным провайдером» (например, `embed()`
//! у Anthropic — см. `crates/canvas-llm/src/anthropic.rs`).
//!
//! Клонируемость (`Clone`) нужна воркеру suggest (FR-079): ошибка
//! возвращается через канал из worker-потока в UI-тред, где
//! складывается в suggest-log — без клонирования модель владения
//! ломает `Sender<Result<...>>`.

// FR-LLM-A: без thiserror (нет зависимостей в дефолтной сборке) —
// вручную реализуем Display + std::error::Error, сигнатура та же.
// thiserror сэкономил бы boilerplate, но он тянет proc-macro2/syn/quote
// в zero-dep дерево, что нарушает ADR-0011 (wasm-гейт).

/// Ошибка вызова `LlmProvider`. Все варианты ведут к fallback
/// (suggest — на lex, graph-builder/agent — к toast пользователю),
/// ни один не панику.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    /// Сеть/таймаут/HTTP-статус (провайдер недоступен, оборвался TLS, …).
    Transport(String),
    /// Протокол: битый JSON, нет ожидаемого поля (дрейф версии API).
    Protocol(String),
    /// Аутентификация: 401/403, невалидный ключ, истёкший OAuth-токен.
    /// Для OAuth refresh-флоу владеет Stream D (`chatgpt_oauth/`).
    Auth(String),
    /// Rate limit (429). `retry_after_secs` — из заголовка `Retry-After`,
    /// если провайдер прислал. Используется для backoff (NF-5: 1 повтор
    /// для suggest, retry с backoff для agent).
    RateLimit { retry_after_secs: Option<u32> },
    /// Операция не поддерживается данным провайдером. Статическая строка
    /// (`'static`) — варианты фиксированы: `"embed"`, `"tool_calling"`,
    /// `"choice"` и т.п. Пример: Anthropic не имеет embeddings API.
    NotSupported(&'static str),
    /// Неверная конфигурация провайдера: пустой `api_key`, неизвестная
    /// модель в `active_model()`, битый `endpoint` URL.
    InvalidConfig(String),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmError::Transport(s) => write!(f, "llm transport: {s}"),
            LlmError::Protocol(s) => write!(f, "llm protocol: {s}"),
            LlmError::Auth(s) => write!(f, "llm auth: {s}"),
            LlmError::RateLimit {
                retry_after_secs: Some(s),
            } => write!(f, "llm rate limit (retry after {s}s)"),
            LlmError::RateLimit {
                retry_after_secs: None,
            } => write!(f, "llm rate limit"),
            LlmError::NotSupported(op) => write!(f, "llm operation not supported: {op}"),
            LlmError::InvalidConfig(s) => write!(f, "llm invalid config: {s}"),
        }
    }
}

impl std::error::Error for LlmError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_variants() {
        assert_eq!(
            LlmError::Transport("timeout".into()).to_string(),
            "llm transport: timeout"
        );
        assert_eq!(
            LlmError::Protocol("bad json".into()).to_string(),
            "llm protocol: bad json"
        );
        assert_eq!(LlmError::Auth("401".into()).to_string(), "llm auth: 401");
        assert_eq!(
            LlmError::RateLimit {
                retry_after_secs: Some(30)
            }
            .to_string(),
            "llm rate limit (retry after 30s)"
        );
        assert_eq!(
            LlmError::RateLimit {
                retry_after_secs: None
            }
            .to_string(),
            "llm rate limit"
        );
        assert_eq!(
            LlmError::NotSupported("embed").to_string(),
            "llm operation not supported: embed"
        );
        assert_eq!(
            LlmError::InvalidConfig("empty key".into()).to_string(),
            "llm invalid config: empty key"
        );
    }

    #[test]
    fn clone_eq() {
        // Клонирование + PartialEq нужны для suggest-воркера (ошибка
        // уходит через канал, AssertEq в тестах).
        let e1 = LlmError::Auth("no token".into());
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        assert_ne!(e1, LlmError::Transport("no token".into()));
    }
}
