//! FR-LLM-OAUTH — интеграционные тесты OAuth-клиента Sign-in-with-ChatGPT
//! (шаблон — provider_tests.rs; БЕЗ сети).
//!
//! Покрывается чистая логика Stream D: PKCE (RFC 7636 Appendix B), построение
//! login URL, валидация state, парсинг token-ответа и `/v1/responses`
//! (JSON-фикстуры инлайн), claims `id_token` (валидные/просроченные),
//! форма refresh-запроса, caps и `embed → NotSupported`.
//!
//! Все тесты требуют feature `l1-llm` — модуль `chatgpt_oauth` целиком
//! feature-gated (ADR-0011: дефолтная сборка без сети; см. док-комментарий
//! `lib.rs`). Сетевых вызовов нет: token endpoint / HTTP-транспорт
//! проверяются в приложении (canvas-app/desktop) против mock-сервера.

#![cfg(feature = "l1-llm")]

use canvas_llm::chatgpt_oauth::{
    auth, jwt, pkce, provider, MemoryTokenStore, NoopVerifier, OAuthClient, OAuthTokens, TokenStore,
};
use canvas_llm::{ChatGptOAuthProvider, LlmError, LlmProvider, Message};
use serde_json::json;

// ============================================================================
// PKCE — RFC 7636 (Appendix B) + границы валидатора
// ============================================================================

#[test]
fn pkce_rfc7636_appendix_b_vector() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert!(pkce::is_valid_verifier(verifier));
    assert_eq!(
        pkce::challenge_from_verifier(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    assert_eq!(pkce::CODE_CHALLENGE_METHOD, "S256");
}

#[test]
fn pkce_verifier_boundaries() {
    assert!(!pkce::is_valid_verifier(&"a".repeat(42)));
    assert!(pkce::is_valid_verifier(&"a".repeat(43)));
    assert!(pkce::is_valid_verifier(&"a".repeat(128)));
    assert!(!pkce::is_valid_verifier(&"a".repeat(129)));
    // Не-unreserved символы (RFC 7636 §4.1) — отказ.
    assert!(!pkce::is_valid_verifier(&format!("{}+/=", "a".repeat(40))));
}

#[test]
fn pkce_generated_verifier_state_nonce() {
    // CSPRNG (getrandom), сеть не нужна.
    let verifier = pkce::generate_verifier().unwrap();
    assert_eq!(verifier.len(), 43); // base64url(32 байта) без паддинга
    assert!(pkce::is_valid_verifier(&verifier));
    let state = pkce::generate_state().unwrap();
    let nonce = pkce::generate_nonce().unwrap();
    assert_ne!(state, nonce);
    assert_ne!(verifier, state);
}

// ============================================================================
// Login URL — все параметры + экранирование
// ============================================================================

#[test]
fn login_url_all_params_and_escaping() {
    let url = auth::build_login_url(
        "http://127.0.0.1:8765/callback",
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        "st",
        "nn",
        "device-1",
    );
    assert!(url.starts_with("https://chatgpt.com/auth/login?"));
    assert!(url.contains("client_id=dynamic_agent_client"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcallback"));
    assert!(url.contains("scope=openid%20profile%20email%20offline_access"));
    assert!(url.contains("code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state=st"));
    assert!(url.contains("nonce=nn"));
    assert!(url.contains("agent_name_hint=CanvasDesk"));
    assert!(url.contains("ext_agent_host_id=device-1"));
    assert!(url.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
    assert!(url.contains("origin=https%3A%2F%2Fchatgpt.com"));
}

#[test]
fn build_login_session_binds_components() {
    let client = OAuthClient::new("device-1");
    let session = client
        .build_login_session("http://127.0.0.1:8765/callback")
        .unwrap();
    // PKCE-замкнутость: challenge = BASE64URL(SHA256(verifier)).
    assert_eq!(
        session.challenge,
        pkce::challenge_from_verifier(&session.verifier)
    );
    assert!(pkce::is_valid_verifier(&session.verifier));
    assert!(!session.state.is_empty() && !session.nonce.is_empty());
    // Login URL содержит challenge/state/nonce этой сессии.
    assert!(session
        .login_url
        .contains(&format!("code_challenge={}", session.challenge)));
    assert!(session
        .login_url
        .contains(&format!("state={}", session.state)));
    assert!(session
        .login_url
        .contains(&format!("nonce={}", session.nonce)));
}

// ============================================================================
// State — валидация (CSRF, RFC 6749 §10.12)
// ============================================================================

#[test]
fn state_validation() {
    assert!(auth::states_equal("s3cret-state", "s3cret-state"));
    assert!(!auth::states_equal("s3cret-state", "s3cret-statf"));
    assert!(!auth::states_equal("s3cret-state", "s3cret-state2"));
    assert!(!auth::states_equal("s3cret-state", ""));
    // Callback без state → отказ (проверка обязательна).
    assert!(auth::parse_callback_query("code=abc").is_err());
    // Callback с чужим state разбирается, но равенство не проходит.
    let cb = auth::parse_callback_query("code=abc&state=evil").unwrap();
    assert!(!auth::states_equal("expected", &cb.state));
}

#[test]
fn callback_parse_and_error_page() {
    let cb = auth::parse_callback_query("code=c%2F1&state=s%20t").unwrap();
    assert_eq!(cb.code, "c/1");
    assert_eq!(cb.state, "s t");
    // Пользователь отказался → error-параметры читаются.
    let err = auth::parse_callback_error("error=access_denied&error_description=no");
    assert_eq!(err.as_deref(), Some("access_denied: no"));
}

// ============================================================================
// Token endpoint — парсинг JSON-ответа (фикстура) + формы запросов
// ============================================================================

/// Фикстура: ответ token endpoint на exchange (cookbook §4.2 шаг 5).
const TOKEN_RESPONSE_JSON: &str = r#"{
    "access_token": "eyJhbGciOiJSUzI1NiJ9.access.sig",
    "refresh_token": "rt_permanent_1",
    "id_token": "eyJhbGciOiJSUzI1NiIsImtpZCI6ImsxIn0.eyJpc3MiOiJodHRwczovL2F1dGgub3BlbmFpLmNvbSIsImF1ZCI6ImR5bmFtaWNfYWdlbnRfY2xpZW50Iiwic3ViIjoidXNlci0xIiwibm9uY2UiOiJubjEiLCJlbWFpbCI6InVzZXJAZXhhbXBsZS5jb20iLCJleHAiOjE3MDAwMDM2MDB9.c2ln",
    "expires_in": 3600
}"#;

#[test]
fn token_response_parse_fixture() {
    let body: serde_json::Value = serde_json::from_str(TOKEN_RESPONSE_JSON).unwrap();
    let now = 1_700_000_000;
    let tokens = auth::tokens_from_json(&body, now).unwrap();
    assert_eq!(tokens.access_token, "eyJhbGciOiJSUzI1NiJ9.access.sig");
    assert_eq!(tokens.refresh_token, "rt_permanent_1");
    assert!(!tokens.id_token.is_empty());
    assert_eq!(tokens.expires_at, now + 3600);
    // Email добирается из id_token сразу при exchange (fbb7536: симметрично
    // refresh-пути) — бейдж Settings «вход выполнен · email» полон сразу.
    assert_eq!(tokens.account_email.as_deref(), Some("user@example.com"));
}

#[test]
fn token_response_without_rotation_keeps_empty_fields() {
    // Refresh-ответ может не содержать refresh_token/id_token (RFC 6749 §6):
    // парсер даёт пустые строки, провайдер сохраняет прежние значения.
    let body: serde_json::Value = serde_json::json!({"access_token": "at2", "expires_in": 60});
    let tokens = auth::tokens_from_json(&body, 1_700_000_000).unwrap();
    assert!(tokens.refresh_token.is_empty());
    assert!(tokens.id_token.is_empty());
    assert_eq!(tokens.expires_at, 1_700_000_060);
}

#[test]
fn exchange_and_refresh_request_shapes() {
    // Обмен кода: RFC 6749 §4.1.3 + resource (дизайн-док §4.2 шаг 4).
    let exchange = auth::build_exchange_form(
        "SPLXO",
        "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
        "http://127.0.0.1:8765/callback",
    );
    assert_eq!(
        exchange,
        "grant_type=authorization_code\
         &client_id=dynamic_agent_client\
         &code=SPLXO\
         &code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk\
         &redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcallback\
         &resource=https%3A%2F%2Fapi.openai.com%2Fv1"
    );
    // Refresh: RFC 6749 §6 (F-5.7).
    let refresh = auth::build_refresh_form("rt_1");
    assert_eq!(
        refresh,
        "grant_type=refresh_token\
         &client_id=dynamic_agent_client\
         &refresh_token=rt_1\
         &resource=https%3A%2F%2Fapi.openai.com%2Fv1"
    );
}

#[test]
fn token_url_proxy_vs_direct() {
    // F-5.10: прокси меняет только базу; путь /oauth/token фиксирован.
    assert_eq!(
        OAuthClient::new("h").token_url(),
        "https://chatgpt.com/oauth/token"
    );
    assert_eq!(
        OAuthClient::new("h")
            .with_proxy(Some("https://llm-proxy.example.com".into()))
            .token_url(),
        "https://llm-proxy.example.com/oauth/token"
    );
    assert_eq!(auth::DEEP_LINK_REDIRECT, "canvasdesk://oauth/callback");
}

// ============================================================================
// id_token — парсинг claims (iss/aud/nonce/exp), валидные и просроченные
// ============================================================================

fn make_jwt(header: &serde_json::Value, payload: &serde_json::Value) -> String {
    let b64 = |v: &serde_json::Value| {
        canvas_llm::chatgpt_oauth::b64::encode(serde_json::to_string(v).unwrap().as_bytes())
    };
    format!("{}.{}.c2ln", b64(header), b64(payload))
}

const NOW: u64 = 1_700_000_000;

fn claims_payload(exp: u64) -> serde_json::Value {
    json!({
        "iss": jwt::EXPECTED_ISSUER,
        "aud": auth::CLIENT_ID,
        "sub": "user-1",
        "nonce": "nn1",
        "email": "user@example.com",
        "exp": exp,
        "iat": exp - 3600,
    })
}

#[test]
fn id_token_valid_claims_accepted() {
    let token = make_jwt(
        &json!({"alg": "RS256", "kid": "k1"}),
        &claims_payload(NOW + 3600),
    );
    let claims = jwt::verify_id_token(
        &token,
        Some("nn1"),
        auth::CLIENT_ID,
        NOW,
        &NoopVerifier,
        &[],
    )
    .expect("валидные claims должны проходить (preview-стаб подписи)");
    assert_eq!(claims.issuer, jwt::EXPECTED_ISSUER);
    assert_eq!(claims.audience, auth::CLIENT_ID);
    assert_eq!(claims.nonce.as_deref(), Some("nn1"));
    assert_eq!(claims.email.as_deref(), Some("user@example.com"));
    assert_eq!(claims.expiry, NOW + 3600);
}

#[test]
fn id_token_expired_rejected() {
    let token = make_jwt(&json!({"alg": "RS256"}), &claims_payload(NOW - 60));
    let err = jwt::verify_id_token(
        &token,
        Some("nn1"),
        auth::CLIENT_ID,
        NOW,
        &NoopVerifier,
        &[],
    )
    .unwrap_err();
    assert!(matches!(err, LlmError::Auth(_)), "{err}");
    // exp в пределах leeway (30 c) — принимается (дрейф часов).
    let token = make_jwt(&json!({"alg": "RS256"}), &claims_payload(NOW - 10));
    assert!(jwt::verify_id_token(
        &token,
        Some("nn1"),
        auth::CLIENT_ID,
        NOW,
        &NoopVerifier,
        &[]
    )
    .is_ok());
}

#[test]
fn id_token_bad_issuer_audience_nonce_rejected() {
    let mut p = claims_payload(NOW + 3600);
    p["iss"] = json!("https://evil.example.com");
    let token = make_jwt(&json!({"alg": "RS256"}), &p);
    assert!(matches!(
        jwt::verify_id_token(&token, None, auth::CLIENT_ID, NOW, &NoopVerifier, &[]),
        Err(LlmError::Auth(_))
    ));

    let mut p = claims_payload(NOW + 3600);
    p["aud"] = json!("another_client");
    let token = make_jwt(&json!({"alg": "RS256"}), &p);
    assert!(matches!(
        jwt::verify_id_token(&token, None, auth::CLIENT_ID, NOW, &NoopVerifier, &[]),
        Err(LlmError::Auth(_))
    ));

    // nonce не совпал — replay-отказ (OIDC Core §3.1.2.1).
    let token = make_jwt(&json!({"alg": "RS256"}), &claims_payload(NOW + 3600));
    assert!(matches!(
        jwt::verify_id_token(
            &token,
            Some("other"),
            auth::CLIENT_ID,
            NOW,
            &NoopVerifier,
            &[]
        ),
        Err(LlmError::Auth(_))
    ));
}

#[test]
fn id_token_alg_none_rejected() {
    let token = make_jwt(&json!({"alg": "none"}), &claims_payload(NOW + 3600));
    let err =
        jwt::verify_id_token(&token, None, auth::CLIENT_ID, NOW, &NoopVerifier, &[]).unwrap_err();
    assert!(err.to_string().contains("alg"));
}

// ============================================================================
// /v1/responses — парсинг фикстур (текстовый ответ; ответ с function_call)
// ============================================================================

/// Фикстура: текстовый ответ Responses API (raw-формат с output[]).
const RESPONSES_TEXT_JSON: &str = r#"{
    "id": "resp_abc123",
    "object": "response",
    "status": "completed",
    "model": "gpt-5.2",
    "output": [
        { "type": "reasoning", "id": "rs_1", "summary": [] },
        {
            "type": "message",
            "id": "msg_1",
            "role": "assistant",
            "status": "completed",
            "content": [
                { "type": "output_text", "text": "Схема модели: " },
                { "type": "output_text", "text": "выручка = цена × объём." }
            ]
        }
    ],
    "usage": { "input_tokens": 12, "output_tokens": 9 }
}"#;

#[test]
fn responses_text_fixture_parse() {
    let resp: serde_json::Value = serde_json::from_str(RESPONSES_TEXT_JSON).unwrap();
    let text = provider::parse_output_text(&resp).unwrap();
    assert_eq!(text, "Схема модели: выручка = цена × объём.");
    // Агрегированное поле (SDK/прокси) тоже принимается.
    let resp = json!({"output_text": "кратко"});
    assert_eq!(provider::parse_output_text(&resp).unwrap(), "кратко");
    // Пустой output / нет output → Protocol.
    assert!(matches!(
        provider::parse_output_text(&json!({"output": []})),
        Err(LlmError::Protocol(_))
    ));
}

/// Фикстура: ответ с function_call (agent panel, multi-turn).
const RESPONSES_FUNCTION_CALL_JSON: &str = r#"{
    "id": "resp_fc9",
    "object": "response",
    "status": "completed",
    "model": "gpt-5.2",
    "output": [
        { "type": "reasoning", "id": "rs_2", "summary": [] },
        {
            "type": "function_call",
            "id": "fc_a1",
            "call_id": "call_a1",
            "name": "graph_apply",
            "arguments": "{\"ops\":[{\"op\":\"node_create\",\"text\":\"Выручка\"}]}"
        },
        {
            "type": "function_call",
            "id": "fc_a2",
            "call_id": "call_a2",
            "name": "graph_validate",
            "arguments": "{}"
        }
    ]
}"#;

#[test]
fn responses_function_call_fixture_parse() {
    let resp: serde_json::Value = serde_json::from_str(RESPONSES_FUNCTION_CALL_JSON).unwrap();
    let calls = provider::parse_function_calls(&resp).unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].id, "call_a1");
    assert_eq!(calls[0].name, "graph_apply");
    match &calls[0].arguments {
        canvas_llm::JsonVal::Object(pairs) => {
            assert!(pairs.iter().any(|(k, _)| k == "ops"));
        }
        other => panic!("ожидался Object-аргумент, получено {other:?}"),
    }
    assert_eq!(calls[1].id, "call_a2");
    assert_eq!(calls[1].name, "graph_validate");
}

#[test]
fn responses_body_builder_shape() {
    use canvas_llm::{JsonVal, Message, ToolChoice, ToolDef};
    let tools = vec![ToolDef::new(
        "rank",
        "Rank options",
        JsonVal::object([("type", JsonVal::string("object"))]),
    )];
    let body = ChatGptOAuthProvider::build_responses_body(
        "gpt-5.2",
        &[Message::system("инструкция"), Message::user("вопрос")],
        Some(&tools),
        Some(&ToolChoice::Specific("rank".into())),
        0.0,
        Some(512),
    );
    assert_eq!(body["model"], "gpt-5.2");
    assert_eq!(body["instructions"], "инструкция");
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["name"], "rank");
    assert_eq!(
        body["tool_choice"],
        json!({"type": "function", "name": "rank"})
    );
    assert_eq!(body["max_output_tokens"], 512);
    assert_eq!(body["stream"], false);
}

// ============================================================================
// ChatGptOAuthProvider — caps / models / embed → NotSupported (F-5.x)
// ============================================================================

fn make_provider() -> ChatGptOAuthProvider {
    ChatGptOAuthProvider::new(Box::new(MemoryTokenStore::new()), "test-device-id", None)
}

#[test]
fn provider_metadata_and_caps() {
    let p = make_provider();
    assert_eq!(LlmProvider::id(&p), "chatgpt_oauth");
    assert_eq!(LlmProvider::display_name(&p), "ChatGPT (Sign-in)");
    assert_eq!(LlmProvider::active_model(&p), "gpt-5.2"); // прототип AI.modelFor.chatgpt
    let caps = LlmProvider::caps(&p);
    assert!(caps.chat);
    assert!(caps.choice); // fallback через forced tool "rank"
    assert!(caps.tool_calling);
    assert!(!caps.embed, "embeddings не входят в подписку (§4.7 п.5)");
    assert!(!caps.streaming);
    assert!(!caps.vision);
}

#[test]
fn provider_models_static_list() {
    // Статический дефолт; реальный список — GET /v1/models (discovery,
    // F-5.5) — приложение может закэшировать через discover_models().
    let p = make_provider();
    let ids: Vec<&str> = LlmProvider::models(&p)
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["gpt-5.2", "gpt-5.2-mini", "gpt-4o", "gpt-4o-mini", "o3"]
    );
    assert!(LlmProvider::models(&p).iter().all(|m| m.supports_tools));
}

#[test]
fn provider_embed_not_supported() {
    // §4.7 п.5: embeddings не входят в подписку ChatGPT.
    let p = make_provider();
    let err =
        pollster::block_on(async { LlmProvider::embed(&p, &["текст"]).await.unwrap_err() });
    assert_eq!(err, LlmError::NotSupported("embed"));
}

#[test]
fn provider_without_session_chat_fails_auth() {
    // Нет сессии в store → chat() обязан вернуть Auth (не панику), UI
    // показывает «войти снова» (F-5.9). Сеть не вызывается — отказ до HTTP.
    let p = make_provider();
    let err = pollster::block_on(async {
        LlmProvider::chat(&p, &[Message::user("hi")], &canvas_llm::ChatOpts::default())
            .await
            .unwrap_err()
    });
    assert!(matches!(err, LlmError::Auth(_)), "{err}");
}

#[test]
fn provider_auto_refresh_on_expiry() {
    // F-5.7: истёкший access_token при отсутствии refresh_token → Auth
    // («войти снова»); refresh-POST не вызывается (сеть не трогаем).
    let store = MemoryTokenStore::new();
    store
        .save(&OAuthTokens {
            access_token: "stale".into(),
            refresh_token: String::new(),
            id_token: String::new(),
            expires_at: 1, // давно истёк
            account_email: None,
        })
        .unwrap();
    let p = ChatGptOAuthProvider::new(Box::new(store), "device", None);
    let err = pollster::block_on(async {
        LlmProvider::chat(&p, &[Message::user("hi")], &canvas_llm::ChatOpts::default())
            .await
            .unwrap_err()
    });
    assert!(matches!(err, LlmError::Auth(_)), "{err}");
}

#[test]
fn provider_store_roundtrip_and_signout() {
    let store = MemoryTokenStore::new();
    let p = ChatGptOAuthProvider::new(Box::new(store), "device", None);
    assert!(!p.is_signed_in());
    p.sign_out().unwrap(); // идемпотентно для пустого store
}
