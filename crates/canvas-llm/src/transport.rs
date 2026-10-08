//! W1 (wave-1, `docs/plans/llm-waves-w1-w2-w3.md` §W1) — транспортная
//! абстракция HTTP для LLM-слоя.
//!
//! Единый шов [`HttpTransport`] для **всех** сетевых вызовов крейта
//! (openai_compat, anthropic, health, chatgpt_oauth, discovery). Реализации:
//!
//! - [`UreqTransport`] — нативный, блокирующий (ureq за кулисами);
//! - [`MockTransport`] — тестовый, скриптованные ответы без сети;
//! - [`WasmFetchTransport`] — браузерный `fetch` (feature `wasm-fetch`,
//!   только под wasm32; реальное включение в web-сборку — волна W3).
//!
//! ## Дизайн
//!
//! - **Статус — данные, не ошибка.** `execute` возвращает `Ok(HttpResponse)`
//!   для любого полученного HTTP-ответа (в т.ч. 4xx/5xx); маппинг статуса в
//!   [`LlmError`] — на вызывающей стороне ([`HttpResponse::map_status`] —
//!   общий helper, token endpoint OAuth маппит 400 отдельно).
//! - **Таймаут — свойство запроса** ([`HttpRequest::timeout`]): провайдеры
//!   держат свой `timeout` (30/60 с, suggest 10 с) и передают в каждом
//!   запросе, поэтому `with_timeout`-билдеры провайдеров работают без
//!   пересоздания транспорта.
//! - **Синхронный путь** (`execute_blocking`) — для desktop-вызовов из
//!   worker-потоков (`canvas-app/oauth_flow` — API-контракт сохранён,
//!   `check_endpoint`). По умолчанию `NotSupported`; блокирующий режим
//!   осмыслен только там, где блокирующий ввод-вывод возможен (натив).
//! - **ADR-0011 не нарушен:** модуль компилируется только за `l1-llm`;
//!   `wasm-fetch` — отдельная opt-in фича, не входящая в default (в
//!   дефолтной wasm-сборке canvas-web сети по-прежнему нет).

// FR-LLM-A: маркер для поиска (grep): файлы Stream A помечены `// FR-LLM-A:`.
// W1-маркер: `// W1-TRANSPORT:` — единственное место, где допустим `ureq::`.

use crate::error::LlmError;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Duration;

// ============================================================================
// Запрос / ответ
// ============================================================================

/// HTTP-метод запроса (`GET`/`POST` достаточно для всех endpoint'ов крейта).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// `GET` (health `/v1/models`, JWKS, discovery).
    Get,
    /// `POST` (chat/completions, messages, responses, token endpoint).
    Post,
}

impl HttpMethod {
    /// Строковое имя метода (для транспорта).
    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
        }
    }
}

/// HTTP-запрос к LLM-endpoint'у (транспортно-нейтральный).
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Метод.
    pub method: HttpMethod,
    /// Полный URL (`{base_url}/{path}`; для OAuth — token_url/JWKS).
    pub url: String,
    /// Заголовки в порядке добавления (Authorization, x-api-key,
    /// anthropic-version, content-type, origin, …).
    pub headers: Vec<(String, String)>,
    /// Тело (`None` для GET).
    pub body: Option<Vec<u8>>,
    /// Таймаут операции (NF-5: suggest 10 с; chat/graph/agent 30–60 с).
    pub timeout: Duration,
}

impl HttpRequest {
    /// GET без тела.
    pub fn get(url: impl Into<String>, timeout: Duration) -> Self {
        Self {
            method: HttpMethod::Get,
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout,
        }
    }

    /// POST с JSON-телом (`content-type: application/json`).
    pub fn post_json(url: impl Into<String>, body: &serde_json::Value, timeout: Duration) -> Self {
        Self::post(
            url,
            timeout,
            "application/json",
            serde_json::to_vec(body).unwrap_or_default(),
        )
    }

    /// POST с form-телом (`application/x-www-form-urlencoded` — token
    /// endpoint OAuth).
    pub fn post_form(url: impl Into<String>, form: &str, timeout: Duration) -> Self {
        Self::post(
            url,
            timeout,
            "application/x-www-form-urlencoded",
            form.as_bytes().to_vec(),
        )
    }

    /// POST с сырым телом и content-type.
    pub fn post(
        url: impl Into<String>,
        timeout: Duration,
        content_type: &str,
        body: Vec<u8>,
    ) -> Self {
        Self {
            method: HttpMethod::Post,
            url: url.into(),
            headers: vec![("content-type".to_string(), content_type.to_string())],
            body: Some(body),
            timeout,
        }
    }

    /// Добавить заголовок (builder-стиль).
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Заголовок по имени (регистронезависимо; удобно в тестах).
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }
}

/// HTTP-ответ (любой статус — статус обрабатывает вызывающий).
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Статус-код (`200`, `401`, `429`, …).
    pub status: u16,
    /// Заголовки (ключи в нижнем регистре).
    pub headers: Vec<(String, String)>,
    /// Тело (сырые байты).
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Заголовок по имени (регистронезависимо).
    pub fn header(&self, name: &str) -> Option<&str> {
        let lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_ascii_lowercase() == lower)
            .map(|(_, v)| v.as_str())
    }

    /// Тело как строка (потеря невалидных UTF-8 байт допустима — все
    /// endpoint'ы крейта возвращают JSON/text).
    pub fn body_str(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// `Retry-After` в секундах (заголовок `retry-after`, целое число).
    pub fn retry_after_secs(&self) -> Option<u32> {
        self.header("retry-after")
            .and_then(|v| v.trim().parse::<u32>().ok())
    }

    /// Общий маппинг не-2xx статуса в [`LlmError`] (семантика сохранена
    /// из предыдущих ureq-точек: 401/403 → Auth, 429 → RateLimit c
    /// `Retry-After`, остальное → Transport). `None` — статус 2xx, ошибки
    /// нет. Особый маппинг token endpoint (400 → Auth с `error_description`)
    /// — в `chatgpt_oauth/auth.rs` (RFC 6749 §5.2).
    pub fn map_status(&self, context: &str) -> Option<LlmError> {
        match self.status {
            200..=299 => None,
            401 | 403 => Some(LlmError::Auth(format!(
                "HTTP {}: {}",
                self.status,
                self.body_str()
            ))),
            429 => Some(LlmError::RateLimit {
                retry_after_secs: self.retry_after_secs(),
            }),
            code => Some(LlmError::Transport(format!(
                "{context}: HTTP {code}: {}",
                self.body_str()
            ))),
        }
    }
}

// ============================================================================
// Трейт транспорта
// ============================================================================

/// Единый HTTP-транспорт LLM-слоя (W1). Реализации: [`UreqTransport`]
/// (натив), [`MockTransport`] (тесты), [`WasmFetchTransport`] (wasm, за
/// `wasm-fetch`). Потокобезопасен — провайдеры держат `Arc<dyn HttpTransport>`.
#[async_trait::async_trait]
pub trait HttpTransport: Send + Sync {
    /// Выполнить запрос. `Ok(HttpResponse)` — для любого полученного ответа
    /// (статус маппит вызывающий); `Err(LlmError::Transport)` — сеть/таймаут.
    async fn execute(&self, req: HttpRequest) -> Result<HttpResponse, LlmError>;

    /// Синхронное исполнение (desktop-пути из worker-потоков: OAuth-флоу
    /// `canvas-app`, `health::check_endpoint`). Дефолт — `NotSupported`:
    /// блокирующий режим осмыслен только там, где возможен блокирующий
    /// ввод-вывод; web-код (W3) обязан использовать async [`Self::execute`].
    fn execute_blocking(&self, req: &HttpRequest) -> Result<HttpResponse, LlmError> {
        let _ = req;
        Err(LlmError::NotSupported("blocking transport execute"))
    }
}

// ============================================================================
// UreqTransport (натив; W1-TRANSPORT — единственное место с ureq в крейте)
// ============================================================================

/// Нативный блокирующий транспорт (ureq). Таймаут берётся из каждого
/// [`HttpRequest`] (agent создаётся на запрос — как раньше в провайдерах).
pub struct UreqTransport;

impl UreqTransport {
    /// Конструктор (без состояния).
    pub fn new() -> Self {
        Self
    }
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl HttpTransport for UreqTransport {
    async fn execute(&self, req: HttpRequest) -> Result<HttpResponse, LlmError> {
        // Блокирующий вызов внутри async — установленный паттерн крейта:
        // футуры провайдеров доводятся до завершения в worker-потоках
        // (pollster в тестах, std::thread + канал в canvas-app).
        self.execute_blocking(&req)
    }

    fn execute_blocking(&self, req: &HttpRequest) -> Result<HttpResponse, LlmError> {
        // W1-TRANSPORT: единственный ureq-код в крейте.
        let agent = ureq::AgentBuilder::new().timeout(req.timeout).build();
        let mut ureq_req = match req.method {
            HttpMethod::Get => agent.get(&req.url),
            HttpMethod::Post => agent.post(&req.url),
        };
        for (name, value) in &req.headers {
            ureq_req = ureq_req.set(name, value);
        }
        // ureq возвращает Ok только для 2xx; 4xx/5xx приходят как
        // Error::Status(code, Response) — приводим оба случая к
        // HttpResponse (статус — данные для вызывающего).
        let result = match &req.body {
            Some(bytes) => ureq_req.send_bytes(bytes),
            None => ureq_req.call(),
        };
        match result {
            Ok(resp) => into_http_response(resp),
            Err(ureq::Error::Status(_code, resp)) => into_http_response(resp),
            Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
        }
    }
}

/// ureq::Response → HttpResponse (заголовки читаются ДО слива тела).
fn into_http_response(resp: ureq::Response) -> Result<HttpResponse, LlmError> {
    let status = resp.status();
    let headers = resp
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            resp.header(&name)
                .map(|v| (name.to_ascii_lowercase(), v.to_string()))
        })
        .collect();
    let body = resp
        .into_string()
        .map(String::into_bytes)
        .map_err(|e| LlmError::Transport(format!("тело ответа не прочитано: {e}")))?;
    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

// ============================================================================
// MockTransport (тесты провайдеров/OAuth/discovery без сети)
// ============================================================================

/// Скриптованный транспорт для тестов: возвращает заготовленные ответы по
/// порядку и записывает все запросы (проверка URL/заголовков/тела).
pub struct MockTransport {
    /// Очередь ответов (исчерпана → `Transport("mock: ответы исчерпаны")`).
    replies: Mutex<VecDeque<Result<HttpResponse, LlmError>>>,
    /// Все полученные запросы (в порядке поступления).
    requests: Mutex<Vec<HttpRequest>>,
}

impl MockTransport {
    /// Новая заготовка: последовательность ответов.
    pub fn new(replies: Vec<Result<HttpResponse, LlmError>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    /// Удобный конструктор успешного JSON-ответа.
    pub fn json_ok(status: u16, body: impl Into<String>) -> Result<HttpResponse, LlmError> {
        Ok(Self::response_json(status, body))
    }

    /// Успешный JSON-ответ (без Result-обёртки).
    pub fn response_json(status: u16, body: impl Into<String>) -> HttpResponse {
        HttpResponse {
            status,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: body.into().into_bytes(),
        }
    }

    /// Все записанные запросы (в порядке поступления).
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().expect("mock requests").clone()
    }

    /// Последний записанный запрос.
    pub fn last_request(&self) -> Option<HttpRequest> {
        self.requests.lock().expect("mock requests").last().cloned()
    }
}

#[async_trait::async_trait]
impl HttpTransport for MockTransport {
    async fn execute(&self, req: HttpRequest) -> Result<HttpResponse, LlmError> {
        self.execute_blocking(&req)
    }

    fn execute_blocking(&self, req: &HttpRequest) -> Result<HttpResponse, LlmError> {
        self.requests
            .lock()
            .expect("mock requests")
            .push(req.clone());
        self.replies
            .lock()
            .expect("mock replies")
            .pop_front()
            .unwrap_or_else(|| Err(LlmError::Transport("mock: ответы исчерпаны".into())))
    }
}

// ============================================================================
// WasmFetchTransport (feature `wasm-fetch`, только wasm32 — F-5.10)
// ============================================================================

#[cfg(all(feature = "wasm-fetch", not(target_arch = "wasm32")))]
compile_error!(
    "feature 'wasm-fetch' допустима только под wasm32-целью (ADR-0011: \
     браузерный fetch не существует на нативе; нативный путь — UreqTransport)"
);

/// Браузерный fetch-транспорт (web/wasm путь, PRD-0010 F-5.10). Запросы
/// уходят через `window.fetch` — из браузера напрямую к LLM API обычно
/// нельзя (CORS), поэтому web-сборка указывает URL'ы cloud-proxy
/// (`cloud/llm-proxy`: pass-through token exchange, CORS `/v1/responses`,
/// `/v1/models`).
///
/// Ограничение v1: таймаут [`HttpRequest::timeout`] не применяется
/// (AbortController — будущая волна); блокирующий `execute_blocking`
/// недоступен (браузер однопоточный) — web-код вызывает async `execute`.
#[cfg(all(feature = "wasm-fetch", target_arch = "wasm32"))]
pub struct WasmFetchTransport;

#[cfg(all(feature = "wasm-fetch", target_arch = "wasm32"))]
mod wasm_send {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    /// Обёртка Send для wasm-футур (`JsFuture` — `!Send`, т.к. держит
    /// `JsValue`; async-trait требует Send в боксе трейта).
    ///
    /// # Safety-обоснование
    /// Цель wasm32-unknown-unknown исполняется однопоточно: все футуры
    /// создаются и доводятся на main-loop браузера (spawn_local/блокирующий
    /// прогон на том же треде) — пересечения тредов нет, гонок данных нет.
    /// std::thread под wasm-целью для LLM-путей canvas-web не используется
    /// (ADR-0011; воркеры — будущая волна, потребует пересмотра).
    pub(crate) struct SendFuture<F>(pub(crate) F);

    // SAFETY: см. доку выше — однонитевой рантайм wasm.
    unsafe impl<F> Send for SendFuture<F> {}

    impl<F: Future> Future for SendFuture<F> {
        type Output = F::Output;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            // Структурная pin-проекция в поле .0 (обёртка не имеет Drop и
            // не перемещает поле — закрепление корректно передаётся внутрь).
            let inner = unsafe { self.map_unchecked_mut(|s| &mut s.0) };
            inner.poll(cx)
        }
    }
}

#[cfg(all(feature = "wasm-fetch", target_arch = "wasm32"))]
#[async_trait::async_trait]
impl HttpTransport for WasmFetchTransport {
    async fn execute(&self, req: HttpRequest) -> Result<HttpResponse, LlmError> {
        use wasm_send::SendFuture;
        // JsFuture !Send (JsValue); обёртка легальна в однопоточном
        // wasm-рантайме — см. wasm_send::SendFuture. Свободная функция —
        // вне async-trait (иначе макрос добавит Send-бонд и к ней).
        SendFuture(wasm_fetch_execute(req)).await
    }
}

/// Собственно fetch (весь объём JS-взаимодействия) — свободная async-функция,
/// чтобы async-trait не добавлял ей Send-бонд (футура !Send из-за JsFuture).
#[cfg(all(feature = "wasm-fetch", target_arch = "wasm32"))]
async fn wasm_fetch_execute(req: HttpRequest) -> Result<HttpResponse, LlmError> {
    use wasm_bindgen::JsCast;

    let window =
        web_sys::window().ok_or_else(|| LlmError::Transport("fetch: нет window".into()))?;

    let init = web_sys::RequestInit::new();
    init.set_method(req.method.as_str());
    if let Some(bytes) = &req.body {
        let arr = js_sys::Uint8Array::from(bytes.as_slice());
        init.set_body(arr.as_ref());
    }
    let js_headers = web_sys::Headers::new()
        .map_err(|e| LlmError::Transport(format!("fetch: headers: {e:?}")))?;
    for (name, value) in &req.headers {
        js_headers
            .set(name, value)
            .map_err(|e| LlmError::Transport(format!("fetch: header {name}: {e:?}")))?;
    }
    init.set_headers(js_headers.as_ref());

    let js_req = web_sys::Request::new_with_str_and_init(&req.url, &init)
        .map_err(|e| LlmError::Transport(format!("fetch: request: {e:?}")))?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&js_req))
        .await
        .map_err(|e| LlmError::Transport(format!("fetch: {e:?}")))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| LlmError::Transport("fetch: ответ не Response".into()))?;

    let status = resp.status();
    // v1: читаем только значимые для маппинга ошибок заголовки
    // (полный перебор Headers-итератора — будущая волна).
    let mut headers = Vec::new();
    for name in ["content-type", "retry-after"] {
        if let Ok(Some(v)) = resp.headers().get(name) {
            headers.push((name.to_string(), v));
        }
    }

    let buf_promise = resp
        .array_buffer()
        .map_err(|e| LlmError::Transport(format!("fetch: array_buffer: {e:?}")))?;
    let buf = wasm_bindgen_futures::JsFuture::from(buf_promise)
        .await
        .map_err(|e| LlmError::Transport(format!("fetch: тело: {e:?}")))?;
    let body = js_sys::Uint8Array::new(&buf).to_vec();

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

// ============================================================================
// Тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Мини-хелпер: pollster уже в dev-deps крейта (см. Cargo.toml).
    macro_rules! async_test {
        ($name:ident, $body:expr) => {
            #[test]
            fn $name() {
                pollster::block_on($body);
            }
        };
    }

    #[test]
    fn request_builders_set_method_and_content_type() {
        let get = HttpRequest::get("https://api.example.com/v1/models", Duration::from_secs(10));
        assert_eq!(get.method, HttpMethod::Get);
        assert!(get.body.is_none());
        assert!(get.headers.is_empty());

        let body = serde_json::json!({ "model": "m" });
        let post = HttpRequest::post_json(
            "https://api.example.com/v1/chat",
            &body,
            Duration::from_secs(30),
        );
        assert_eq!(post.method, HttpMethod::Post);
        assert_eq!(post.header("content-type"), Some("application/json"));
        let parsed: serde_json::Value =
            serde_json::from_slice(post.body.as_deref().unwrap_or(&[])).unwrap();
        assert_eq!(parsed["model"], "m");
    }

    #[test]
    fn with_header_appends_and_preserves_content_type() {
        let req = HttpRequest::post_form("https://host/oauth/token", "a=b", Duration::from_secs(5))
            .with_header("origin", "https://chatgpt.com")
            .with_header("Authorization", "Bearer x");
        assert_eq!(
            req.header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(req.header("origin"), Some("https://chatgpt.com"));
        assert_eq!(req.header("authorization"), Some("Bearer x"));
    }

    #[test]
    fn response_header_lookup_is_case_insensitive() {
        let resp = HttpResponse {
            status: 200,
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: b"{}".to_vec(),
        };
        assert_eq!(resp.header("content-type"), Some("application/json"));
        assert_eq!(resp.header("CONTENT-TYPE"), Some("application/json"));
        assert_eq!(resp.header("retry-after"), None);
    }

    #[test]
    fn map_status_2xx_is_none() {
        let resp = MockTransport::response_json(200, "{}");
        assert!(resp.map_status("ctx").is_none());
        let resp = MockTransport::response_json(204, "");
        assert!(resp.map_status("ctx").is_none());
    }

    #[test]
    fn map_status_auth_rate_limit_and_transport() {
        let resp = MockTransport::response_json(401, "no key");
        assert!(matches!(resp.map_status("ctx"), Some(LlmError::Auth(_))));

        let resp = MockTransport::response_json(403, "{}");
        assert!(matches!(resp.map_status("ctx"), Some(LlmError::Auth(_))));

        let mut resp = MockTransport::response_json(429, "{}");
        assert!(matches!(
            resp.map_status("ctx"),
            Some(LlmError::RateLimit {
                retry_after_secs: None
            })
        ));
        resp.headers.push(("retry-after".into(), "17".into()));
        assert!(matches!(
            resp.map_status("ctx"),
            Some(LlmError::RateLimit {
                retry_after_secs: Some(17)
            })
        ));

        let resp = MockTransport::response_json(500, "boom");
        match resp.map_status("myctx") {
            Some(LlmError::Transport(msg)) => {
                assert!(msg.contains("myctx"));
                assert!(msg.contains("HTTP 500"));
                assert!(msg.contains("boom"));
            }
            other => panic!("ожидался Transport, получено {other:?}"),
        }
    }

    async_test!(
        mock_transport_records_requests_and_plays_replies_in_order,
        async {
            let mock = MockTransport::new(vec![
                MockTransport::json_ok(200, r#"{"first": true}"#),
                Err(LlmError::RateLimit {
                    retry_after_secs: Some(3),
                }),
            ]);
            let req1 = HttpRequest::get("https://a/1", Duration::from_secs(1));
            let req2 = HttpRequest::get("https://a/2", Duration::from_secs(2));

            let r1 = mock.execute(req1.clone()).await.unwrap();
            assert_eq!(r1.status, 200);
            assert_eq!(r1.body_str(), r#"{"first": true}"#);

            let err = mock.execute(req2.clone()).await.unwrap_err();
            assert!(matches!(
                err,
                LlmError::RateLimit {
                    retry_after_secs: Some(3)
                }
            ));

            let err = mock.execute(req1).await.unwrap_err();
            assert!(matches!(err, LlmError::Transport(s) if s.contains("исчерпаны")));

            let requests = mock.requests();
            assert_eq!(requests.len(), 3);
            assert_eq!(requests[0].url, "https://a/1");
            assert_eq!(requests[1].url, "https://a/2");
            assert_eq!(requests[1].timeout, Duration::from_secs(2));
            assert!(mock.last_request().is_some());
        }
    );
}
