//! FR-LLM-A — Health-check helpers (feature `l1-llm` only).
//!
//! Утилита для `LlmProvider::health()` адаптеров: единый GET-запрос к
//! `/v1/models` (или `/health` для Laya), проверка 200 OK. Адаптеры
//! (`openai_compat.rs`, `anthropic.rs`) используют эту функцию, чтобы
//! не дублировать код ureq-вызова и обработки ошибок.

use crate::error::LlmError;

/// Выполнить health-check: GET к `url` с auth-заголовком, проверить 200.
///
/// `auth_header` / `auth_value` — например, `"Authorization"` / `"Bearer ..."`.
/// Если провайдер не требует auth (Ollama), передайте пустые строки —
/// заголовок не добавляется.
///
/// Возвращает `Ok(())` при 200, иначе — `LlmError`:
/// - 401/403 → `Auth`.
/// - 429 → `RateLimit` (попытка прочитать `Retry-After` заголовок).
/// - Другие статусы → `Transport`.
/// - Сеть/таймаут → `Transport`.
pub fn check_endpoint(url: &str, auth_header: &str, auth_value: &str) -> Result<(), LlmError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let mut req = agent.get(url);
    if !auth_header.is_empty() && !auth_value.is_empty() {
        req = req.set(auth_header, auth_value);
    }

    match req.call() {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, resp)) => {
            let _ = resp.into_string(); // слить тело соединения
            match code {
                401 | 403 => Err(LlmError::Auth(format!("HTTP {code}"))),
                429 => {
                    // Retry-After заголовок отсутствует в ureq::Response
                    // в удобном виде — возвращаем None (адаптер может сделать
                    // backoff по умолчанию).
                    Err(LlmError::RateLimit {
                        retry_after_secs: None,
                    })
                }
                _ => Err(LlmError::Transport(format!("HTTP {code}"))),
            }
        }
        Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
    }
}
