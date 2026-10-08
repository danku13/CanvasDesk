//! W1 (wave-1) — интеграционные тесты провайдеров через [`MockTransport`]
//! (без сети). Проверяют новый транспортный шов `HttpTransport`:
//! построение запросов (URL/заголовки/тело), маппинг статусов, парсинг
//! ответов, OAuth refresh-flow и discovery.
//!
//! Файл целиком требует feature `l1-llm` (адаптеры + транспорт только за флаг).

#![cfg(feature = "l1-llm")]

use canvas_llm::chatgpt_oauth::{MemoryTokenStore, OAuthTokens};
use canvas_llm::transport::{HttpRequest, HttpTransport, MockTransport};
use canvas_llm::{
    health, AnthropicClaudeProvider, ChatGptOAuthProvider, ChatOpts, LlmError, LlmProvider,
    Message, OpenAiCompatibleProvider, TokenStore,
};

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Прогнать async-метод в тесте (паттерн крейта: pollster в dev-deps).
fn block<T>(fut: impl std::future::Future<Output = T>) -> T {
    pollster::block_on(fut)
}

// ============================================================================
// OpenAiCompatibleProvider — chat / tool_calling / choice / ошибки статусов
// ============================================================================

const CHAT_FIXTURE: &str = r#"{
    "choices": [{ "message": { "role": "assistant", "content": "привет от мока" } }]
}"#;

fn byok_with_arc(mock: Arc<MockTransport>) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::zai("sk-test-key", "glm-5.3-flash").with_transport(mock)
}

#[test]
fn openai_chat_builds_request_and_parses_answer() {
    let mock = Arc::new(MockTransport::new(vec![MockTransport::json_ok(
        200,
        CHAT_FIXTURE,
    )]));
    let p = byok_with_arc(mock.clone());

    let answer = block(p.chat(&[Message::user("привет")], &ChatOpts::default())).unwrap();
    assert_eq!(answer, "привет от мока");

    let req = mock.last_request().unwrap();
    assert_eq!(req.url, "https://api.z.ai/v1/chat/completions");
    assert_eq!(req.header("authorization"), Some("Bearer sk-test-key"));
    let body: serde_json::Value =
        serde_json::from_slice(req.body.as_deref().unwrap_or(&[])).unwrap();
    assert_eq!(body["model"], "glm-5.3-flash");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "привет");
    assert_eq!(body["stream"], false);
}

#[test]
fn openai_401_maps_to_auth_and_429_to_rate_limit() {
    // 401 → Auth.
    let mock = MockTransport::new(vec![MockTransport::json_ok(401, "невалидный ключ")]);
    let err =
        block(byok_with_arc(Arc::new(mock)).chat(&[Message::user("x")], &ChatOpts::default()))
            .unwrap_err();
    assert!(matches!(err, LlmError::Auth(_)));

    // 429 → RateLimit c Retry-After из заголовка.
    let mut resp = MockTransport::response_json(429, "{}");
    resp.headers
        .push(("retry-after".to_string(), "11".to_string()));
    let mock = MockTransport::new(vec![Ok(resp)]);
    let err =
        block(byok_with_arc(Arc::new(mock)).chat(&[Message::user("x")], &ChatOpts::default()))
            .unwrap_err();
    assert!(matches!(
        err,
        LlmError::RateLimit {
            retry_after_secs: Some(11)
        }
    ));
}

#[test]
fn openai_tool_calling_parses_tool_calls() {
    let fixture = r#"{
        "choices": [{
            "message": {
                "role": "assistant",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": { "name": "graph_apply", "arguments": "{\"ops\":[]}" }
                }]
            }
        }]
    }"#;
    let mock = Arc::new(MockTransport::new(vec![MockTransport::json_ok(
        200, fixture,
    )]));
    let p = byok_with_arc(mock);
    let tools = vec![canvas_llm::ToolDef::new(
        "graph_apply",
        "Apply ops",
        canvas_llm::JsonVal::object([("type", canvas_llm::JsonVal::string("object"))]),
    )];
    let calls = block(p.tool_calling(&[Message::user("собери граф")], &tools, &Default::default()))
        .unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "graph_apply");
}

#[test]
fn openrouter_choice_systemone_via_mock() {
    let fixture = r#"{
        "answers": {
            "main": {
                "probabilities": { "b": 0.2, "a": 0.8 },
                "answer_confidence": 0.9
            }
        }
    }"#;
    let mock = Arc::new(MockTransport::new(vec![MockTransport::json_ok(
        200, fixture,
    )]));
    let p = OpenAiCompatibleProvider::openrouter("sk-or", "z-ai/glm-5.3-flash")
        .with_transport(mock.clone());
    let options = vec![
        canvas_llm::OptionDesc {
            id: "a".into(),
            desc: "первая".into(),
        },
        canvas_llm::OptionDesc {
            id: "b".into(),
            desc: String::new(),
        },
    ];
    let answer = block(p.choice("документ", &options)).unwrap();
    // Сортировка: убывание prob, при равенстве — id.
    assert_eq!(answer.probs[0].0, "a");
    assert!((answer.confidence - 0.9).abs() < 1e-9);

    // URL System One сохранён (base + /api/alpha/decisions).
    let req = mock.last_request().unwrap();
    assert!(
        req.url.ends_with("/api/alpha/decisions"),
        "получено: {}",
        req.url
    );
}

// ============================================================================
// AnthropicClaudeProvider — заголовки + парсинг content
// ============================================================================

#[test]
fn anthropic_chat_sends_x_api_key_and_parses_content() {
    let fixture = r#"{ "content": [{ "type": "text", "text": "ответ claude" }] }"#;
    let mock = Arc::new(MockTransport::new(vec![MockTransport::json_ok(
        200, fixture,
    )]));
    let p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022")
        .with_transport(mock.clone());
    let answer = block(p.chat(&[Message::user("привет")], &ChatOpts::default())).unwrap();
    assert_eq!(answer, "ответ claude");

    let req = mock.last_request().unwrap();
    assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
    assert_eq!(req.header("x-api-key"), Some("sk-ant"));
    assert_eq!(req.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(req.header("content-type"), Some("application/json"));
}

// ============================================================================
// ChatGptOAuthProvider — Responses API + авто-refresh (F-5.7)
// ============================================================================

const RESPONSES_FIXTURE: &str = r#"{
    "output": [{ "type": "message", "content": [{ "type": "output_text", "text": "ответ подписки" }] }]
}"#;

fn token_json(access: &str) -> String {
    format!(
        r#"{{ "access_token": "{access}", "refresh_token": "", "id_token": "", "expires_in": 3600 }}"#
    )
}

fn expired_tokens() -> OAuthTokens {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    OAuthTokens {
        access_token: "old-access".into(),
        refresh_token: "refresh-1".into(),
        id_token: String::new(),
        expires_at: now.saturating_sub(100),
        account_email: None,
    }
}

#[test]
fn chatgpt_oauth_chat_happy_path_via_proxy() {
    let store = Box::new(MemoryTokenStore::new());
    store
        .save(&OAuthTokens {
            access_token: "valid-access".into(),
            refresh_token: "refresh-1".into(),
            id_token: String::new(),
            expires_at: u64::MAX / 2, // далеко не истёк
            account_email: None,
        })
        .unwrap();

    let mock = Arc::new(MockTransport::new(vec![MockTransport::json_ok(
        200,
        RESPONSES_FIXTURE,
    )]));
    let p = ChatGptOAuthProvider::new(store, "host-id", Some("http://proxy.test".into()))
        .with_transport(mock.clone());

    let answer = block(p.chat(&[Message::user("вопрос")], &ChatOpts::default())).unwrap();
    assert_eq!(answer, "ответ подписки");

    let req = mock.last_request().unwrap();
    assert_eq!(req.url, "http://proxy.test/v1/responses");
    assert_eq!(req.header("authorization"), Some("Bearer valid-access"));
}

#[test]
fn chatgpt_oauth_401_triggers_refresh_and_retry_f5_7() {
    let store = Box::new(MemoryTokenStore::new());
    // Токен НЕ истёк (expires_at далеко) — refresh провоцирует 401 от API
    // (токен отозван раньше expires_at), а не локальная проверка времени.
    store
        .save(&OAuthTokens {
            access_token: "stale-access".into(),
            refresh_token: "refresh-1".into(),
            id_token: String::new(),
            expires_at: u64::MAX / 2,
            account_email: None,
        })
        .unwrap();

    // Порядок ответов: 1) /responses → 401 (токен отозван), 2) refresh →
    // новая пара, 3) /responses повтор → 200.
    let mock = Arc::new(MockTransport::new(vec![
        Ok(MockTransport::response_json(401, "token revoked")),
        MockTransport::json_ok(200, token_json("new-access")),
        MockTransport::json_ok(200, RESPONSES_FIXTURE),
    ]));
    let p = ChatGptOAuthProvider::new(store, "host-id", Some("http://proxy.test".into()))
        .with_transport(mock.clone());

    let answer = block(p.chat(&[Message::user("вопрос")], &ChatOpts::default())).unwrap();
    assert_eq!(answer, "ответ подписки");

    // Все три запроса состояли; последний — retry с НОВЫМ access-токеном
    // (refresh + сохранение в store прошли).
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].url.ends_with("/v1/responses"));
    assert!(requests[1].url.ends_with("/oauth/token"));
    assert_eq!(requests[2].url, "http://proxy.test/v1/responses");
    assert_eq!(
        requests[2].header("authorization"),
        Some("Bearer new-access")
    );
}

#[test]
fn chatgpt_oauth_expired_token_refreshes_before_request() {
    // Истёкший токен → refresh ДО первого запроса (ensure_access_token).
    let store = Box::new(MemoryTokenStore::new());
    store.save(&expired_tokens()).unwrap();

    let mock = Arc::new(MockTransport::new(vec![
        MockTransport::json_ok(200, token_json("fresh-access")),
        MockTransport::json_ok(200, RESPONSES_FIXTURE),
    ]));
    let p = ChatGptOAuthProvider::new(store, "host-id", Some("http://proxy.test".into()))
        .with_transport(mock.clone());

    let answer = block(p.chat(&[Message::user("вопрос")], &ChatOpts::default())).unwrap();
    assert_eq!(answer, "ответ подписки");

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].url.ends_with("/oauth/token"),
        "первый запрос — refresh"
    );
    assert_eq!(
        requests[1].header("authorization"),
        Some("Bearer fresh-access")
    );
}

#[test]
fn chatgpt_oauth_no_session_maps_to_auth_f5_9() {
    // Пустой store → load_tokens даёт Auth ещё до сети (mock не нужен).
    let mock = MockTransport::new(vec![]);
    let p = ChatGptOAuthProvider::new(
        Box::new(MemoryTokenStore::new()),
        "host-id",
        Some("http://proxy.test".into()),
    )
    .with_transport(Arc::new(mock));
    let err = block(p.chat(&[Message::user("x")], &ChatOpts::default())).unwrap_err();
    assert!(matches!(err, LlmError::Auth(_)));
}

// ============================================================================
// health::check_provider + discovery через MockTransport
// ============================================================================

#[test]
fn check_provider_reports_ok_and_auth() {
    // BYOK с валидным ключом: health GET /models → 200.
    let mock = MockTransport::new(vec![MockTransport::json_ok(200, r#"{"data":[]}"#)]);
    let report = block(health::check_provider(&byok_with_arc(Arc::new(mock))));
    assert_eq!(report, health::HealthReport::Ok);

    // Невалидный ключ: 401 → Auth с деталью.
    let mock = MockTransport::new(vec![MockTransport::json_ok(401, "нет доступа")]);
    let report = block(health::check_provider(&byok_with_arc(Arc::new(mock))));
    assert!(matches!(report, health::HealthReport::Auth(s) if s.contains("401")));
}

#[test]
fn discovery_list_models_blocking_via_mock() {
    let fixture = r#"{ "data": [ { "id": "glm-5.3-flashx", "owned_by": "z.ai" }, { "id": "glm-5.3-flash" } ] }"#;
    let mock = MockTransport::new(vec![MockTransport::json_ok(200, fixture)]);
    let models = canvas_llm::discovery::list_models_blocking(
        &mock,
        "https://api.z.ai/v1/",
        Some("sk-zai"),
        Duration::from_secs(10),
    )
    .unwrap();
    // trailing slash base_url обрезан; сортировка по id.
    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["glm-5.3-flash", "glm-5.3-flashx"]);
    assert_eq!(models[1].owned_by.as_deref(), Some("z.ai"));

    let req: &HttpRequest = &mock.requests()[0];
    assert_eq!(req.url, "https://api.z.ai/v1/models");
    assert_eq!(req.header("authorization"), Some("Bearer sk-zai"));
}

// ============================================================================
// HttpTransport-контракты (для W2/W3)
// ============================================================================

#[test]
fn blocking_execute_on_mock_works_and_records() {
    // MockTransport поддерживает и блокирующий путь (sync-методы OAuth на
    // desktop); запрос записывается в обоих режимах.
    let mock = MockTransport::new(vec![MockTransport::json_ok(200, r#"{"ok":true}"#)]);
    let req = HttpRequest::get("https://x.test/ping", Duration::from_secs(1));
    let resp = mock.execute_blocking(&req).unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(mock.requests().len(), 1);
}
