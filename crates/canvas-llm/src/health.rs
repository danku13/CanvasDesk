//! FR-LLM-A — Health-check helpers (feature `l1-llm` only).
//!
//! Два уровня API:
//!
//! 1. [`check_endpoint`] — низкоуровневый helper (GET + проверка 2xx),
//!    используется `LlmProvider::health()` адаптеров. W1: выполняется через
//!    транспортную абстракцию ([`crate::transport::UreqTransport`]).
//! 2. [`HealthReport`] + [`check_provider`] — высокоуровневый API волны W1
//!    для UI-кнопок «Проверить ключ» (подключение — волна W2, точки
//!    `FR-LLM-FIX-TODO` в canvas-app): человекочитаемый результат вместо
//!    `Result<(), LlmError>`, состояния idle/проверяется ведёт сама UI-сторона.

use crate::error::LlmError;
use crate::transport::{HttpRequest, HttpTransport, UreqTransport};
use std::time::Duration;

/// Таймаут health-check (как раньше: 10 с на весь запрос).
const HEALTH_TIMEOUT: Duration = Duration::from_secs(10);

/// Выполнить health-check: GET к `url` с auth-заголовком, проверить 2xx.
///
/// `auth_header` / `auth_value` — например, `"Authorization"` / `"Bearer ..."`.
/// Если провайдер не требует auth (Ollama), передайте пустые строки —
/// заголовок не добавляется.
///
/// Возвращает `Ok(())` при 2xx, иначе — `LlmError`:
/// - 401/403 → `Auth`.
/// - 429 → `RateLimit` (пытается прочитать `Retry-After`).
/// - Другие статусы → `Transport`.
/// - Сеть/таймаут → `Transport`.
pub fn check_endpoint(url: &str, auth_header: &str, auth_value: &str) -> Result<(), LlmError> {
    let mut req = HttpRequest::get(url, HEALTH_TIMEOUT);
    if !auth_header.is_empty() && !auth_value.is_empty() {
        req = req.with_header(auth_header, auth_value);
    }
    let resp = UreqTransport::new().execute_blocking(&req)?;
    match resp.map_status("health-check") {
        // 2xx → успех (семантика прежнего «Ok только для 2xx» сохранена).
        None => Ok(()),
        Some(err) => Err(err),
    }
}

/// Высокоуровневый результат health-check (W1, для UI волны W2).
///
/// Человекочитаемая проекция `Result<(), LlmError>`: каждый вариант —
/// отдельное состояние строки настроек («ключ валиден», «401 — ключ
/// неверен», «rate limit, повтор через N с», «эндпоинт недоступен», …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthReport {
    /// Проверка прошла (2xx): ключ/эндпоинт живы.
    Ok,
    /// Аутентификация не прошла (401/403 или невалидный OAuth-токен).
    Auth(String),
    /// Rate limit (429). `retry_after_secs` — из заголовка `Retry-After`,
    /// если провайдер прислал.
    RateLimit { retry_after_secs: Option<u32> },
    /// Сеть/таймаут/неожиданный статус/битый протокол — с деталью.
    Transport(String),
    /// Конфигурация некорректна ДО запроса (пустой ключ, неизвестная модель).
    InvalidConfig(String),
    /// Операция health-check не поддерживается данным провайдером.
    NotSupported(&'static str),
}

impl HealthReport {
    /// Успешная ли проверка (для включения/выключения UI-индикатора).
    pub fn is_ok(&self) -> bool {
        matches!(self, HealthReport::Ok)
    }
}

impl std::fmt::Display for HealthReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthReport::Ok => write!(f, "OK"),
            HealthReport::Auth(s) => write!(f, "auth: {s}"),
            HealthReport::RateLimit {
                retry_after_secs: Some(s),
            } => {
                write!(f, "rate limit (retry after {s}s)")
            }
            HealthReport::RateLimit {
                retry_after_secs: None,
            } => write!(f, "rate limit"),
            HealthReport::Transport(s) => write!(f, "transport: {s}"),
            HealthReport::InvalidConfig(s) => write!(f, "invalid config: {s}"),
            HealthReport::NotSupported(op) => write!(f, "not supported: {op}"),
        }
    }
}

/// Спроецировать [`LlmError`] в [`HealthReport`] (сохранение семантики
/// вариантов; Protocol считается транспортной проблемой — дрейф API).
pub fn report_from(err: LlmError) -> HealthReport {
    match err {
        LlmError::Auth(s) => HealthReport::Auth(s),
        LlmError::RateLimit { retry_after_secs } => HealthReport::RateLimit { retry_after_secs },
        LlmError::InvalidConfig(s) => HealthReport::InvalidConfig(s),
        LlmError::NotSupported(op) => HealthReport::NotSupported(op),
        LlmError::Transport(s) | LlmError::Protocol(s) => HealthReport::Transport(s),
    }
}

/// Проверить провайдера высокоуровнево: вызывает `LlmProvider::health()`
/// и проецирует результат в [`HealthReport`] (W1 API; UI-сторона волны W2
/// вызывает это из worker-потока и ведёт состояния idle/проверяется сама).
pub async fn check_provider(provider: &dyn crate::LlmProvider) -> HealthReport {
    match provider.health().await {
        Ok(()) => HealthReport::Ok,
        Err(e) => report_from(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_from_maps_all_variants() {
        assert_eq!(
            report_from(LlmError::Auth("401".into())),
            HealthReport::Auth("401".into())
        );
        assert_eq!(
            report_from(LlmError::RateLimit {
                retry_after_secs: Some(7)
            }),
            HealthReport::RateLimit {
                retry_after_secs: Some(7)
            }
        );
        assert_eq!(
            report_from(LlmError::InvalidConfig("пустой ключ".into())),
            HealthReport::InvalidConfig("пустой ключ".into())
        );
        assert_eq!(
            report_from(LlmError::NotSupported("embed")),
            HealthReport::NotSupported("embed")
        );
        assert_eq!(
            report_from(LlmError::Transport("timeout".into())),
            HealthReport::Transport("timeout".into())
        );
        // Protocol проецируется в Transport (дрейф API — транспортная
        // категория для UI).
        assert_eq!(
            report_from(LlmError::Protocol("битый JSON".into())),
            HealthReport::Transport("битый JSON".into())
        );
    }

    #[test]
    fn is_ok_only_for_ok() {
        assert!(HealthReport::Ok.is_ok());
        assert!(!HealthReport::Auth("x".into()).is_ok());
        assert!(!HealthReport::RateLimit {
            retry_after_secs: None
        }
        .is_ok());
        assert!(!HealthReport::Transport("x".into()).is_ok());
        assert!(!HealthReport::InvalidConfig("x".into()).is_ok());
        assert!(!HealthReport::NotSupported("embed").is_ok());
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(HealthReport::Ok.to_string(), "OK");
        assert_eq!(
            HealthReport::RateLimit {
                retry_after_secs: Some(30)
            }
            .to_string(),
            "rate limit (retry after 30s)"
        );
        assert_eq!(
            HealthReport::InvalidConfig("пустой ключ".into()).to_string(),
            "invalid config: пустой ключ"
        );
    }

    #[test]
    fn check_endpoint_maps_status_semantics() {
        // Без сети проверяем только семантику маппинга статусов: недоступный
        // хост → Transport (сервер не существует, соединение не установится).
        // 2xx/401/429-пути покрывают transport_provider_tests через
        // MockTransport-провайдеров (см. tests/).
        let err = check_endpoint("http://127.0.0.1:1/v1/models", "", "")
            .expect_err("порт 1 не слушается");
        assert!(matches!(err, LlmError::Transport(_)));
    }
}
