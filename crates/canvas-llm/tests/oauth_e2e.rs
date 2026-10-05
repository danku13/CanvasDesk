//! FR-LLM-OAUTH-E2E / PRD-0010 F-5.2–F-5.7 — E2E-смоук desktop OAuth-флоу.
//!
//! Полный цикл «как его видит пользователь» против **локального mock-сервера**
//! (без внешней сети — правило ADR-0011 «без сети в хосте» действует и в
//! тестах; mock подменяет cloud-proxy, `proxy_url` — штатная точка инъекции,
//! F-5.10):
//!
//! 1. `CallbackListener::bind()` → свободный порт loopback;
//! 2. `build_login_session` → login URL (PKCE S256, state, nonce);
//! 3. «Браузер» (TcpStream) шлёт `GET /callback?code=..&state=..`;
//! 4. `wait_for_code` → валидация state → `exchange_code` (mock token endpoint);
//! 5. `ChatGptOAuthProvider` (proxy = mock) → `chat()` / `health()`;
//! 6. авто-refresh: 401 от API → refresh → одна повторная попытка (F-5.7);
//!    истёкший `expires_at` → проактивный refresh до запроса.
//!
//! Метрика G4 (setup ≤ 30 сек) замеряется на шагах 1–4 — в реальной
//! установке сюда добавляется только время пользователя в браузере.
//!
//! Запуск: `cargo test -p canvas-llm --features l1-llm --test oauth_e2e`.
//! Файл целиком desktop-only (TcpListener недоступен под wasm32).

#![cfg(all(feature = "l1-llm", not(target_arch = "wasm32")))]

use canvas_llm::chatgpt_oauth::auth::{CallbackListener, OAuthClient};
use canvas_llm::chatgpt_oauth::b64;
use canvas_llm::chatgpt_oauth::{ChatGptOAuthProvider, MemoryTokenStore, OAuthTokens};
use canvas_llm::{LlmError, LlmProvider, Message, TokenStore};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Mock «cloud-proxy» (F-5.10): stateless роутер на loopback
// ---------------------------------------------------------------------------

/// Режим POST /v1/responses: обычный 200 или первый вызов 401 (для
/// проверки авто-refresh + одного повтора).
#[derive(Clone, Copy, PartialEq)]
enum ResponsesMode {
    Normal,
    UnauthorizedOnce,
}

struct MockState {
    mode: ResponsesMode,
    responses_calls: AtomicU32,
    token_calls: AtomicU32,
}

struct MockServer {
    base_url: String,
    state: Arc<MockState>,
}

impl MockServer {
    /// Поднять mock на 127.0.0.1:0 и стартовать роутер в фоне.
    fn start(mode: ResponsesMode) -> Self {
        let listener =
            TcpListener::bind(("127.0.0.1", 0)).expect("mock: bind 127.0.0.1:0 не удался");
        let port = listener.local_addr().expect("mock: local_addr").port();
        let state = Arc::new(MockState {
            mode,
            responses_calls: AtomicU32::new(0),
            token_calls: AtomicU32::new(0),
        });
        let state_for_thread = Arc::clone(&state);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let st = Arc::clone(&state_for_thread);
                std::thread::spawn(move || handle_connection(&mut stream, &st));
            }
        });
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            state,
        }
    }

    fn token_calls(&self) -> u32 {
        self.state.token_calls.load(Ordering::SeqCst)
    }

    fn responses_calls(&self) -> u32 {
        self.state.responses_calls.load(Ordering::SeqCst)
    }
}

/// Прочитать один HTTP-запрос (head + body по Content-Length).
/// Head ищется ВНУТРИ буфера (тело идёт сразу за ним в том же сегменте TCP —
/// ожидание `\r\n\r\n` в конце буфера = дедлок с ureq, который ждёт ответ).
fn read_request(stream: &mut TcpStream) -> Option<(String, String)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(head_end) = find_head_end(&buf) {
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let content_length = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())?
                })
                .unwrap_or(0);
            let body_needed = head_end + 4 + content_length;
            if buf.len() >= body_needed {
                let body = String::from_utf8_lossy(&buf[head_end + 4..body_needed]).to_string();
                let request_line = head.lines().next().unwrap_or_default().to_string();
                return Some((request_line, body));
            }
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 64 * 1024 {
            return None;
        }
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn respond(stream: &mut TcpStream, status: u16, reason: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.flush();
}

/// Роутер mock-сервера: token endpoint, Responses API, models.
fn handle_connection(stream: &mut TcpStream, st: &MockState) {
    let Some((request_line, body)) = read_request(stream) else {
        return;
    };
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();

    match (method.as_str(), path.as_str()) {
        // Token endpoint: exchange_code / refresh_tokens (stateless pass-through)
        ("POST", "/oauth/token") => {
            st.token_calls.fetch_add(1, Ordering::SeqCst);
            let grant_present = body.contains("grant_type");
            assert!(
                grant_present,
                "mock: token endpoint без grant_type в теле ({body})"
            );
            respond(stream, 200, "OK", &token_json());
        }
        // Responses API: 401 один раз (тест авто-refresh) либо сразу 200
        ("POST", "/v1/responses") => {
            let call = st.responses_calls.fetch_add(1, Ordering::SeqCst);
            if st.mode == ResponsesMode::UnauthorizedOnce && call == 0 {
                respond(
                    stream,
                    401,
                    "Unauthorized",
                    r#"{"error":{"message":"token expired (mock)"}}"#,
                );
            } else {
                respond(stream, 200, "OK", r#"{"output_text":"E2E ok"}"#);
            }
        }
        // Model discovery (health / F-5.5)
        ("GET", "/v1/models") => {
            respond(
                stream,
                200,
                "OK",
                r#"{"data":[{"id":"gpt-5.2"},{"id":"gpt-5.2-mini"},{"id":"gpt-4o"}]}"#,
            );
        }
        _ => {
            respond(stream, 404, "Not Found", r#"{"error":"not found (mock)"}"#);
        }
    }
}

/// Валидный ответ token endpoint: access/refresh + id_token с корректными
/// claims (iss/aud/nonce/exp/email) — email парсится в `account_email`.
fn token_json() -> String {
    format!(
        r#"{{"access_token":"at-e2e-fresh","refresh_token":"rt-e2e-fresh","id_token":{},"token_type":"Bearer","expires_in":3600}}"#,
        serde_json::to_string(&id_token_fixture("e2e@example.com")).expect("id_token json")
    )
}

/// Минимальный JWT (header.payload.signature, base64url) — подпись mock
/// (криптопроверка — v1 SECURITY-стаб, см. jwt.rs; claims проверяются).
fn id_token_fixture(email: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let header = r#"{"alg":"RS256","kid":"mock-key","typ":"JWT"}"#;
    let payload = format!(
        r#"{{"iss":"https://auth.openai.com","aud":"dynamic_agent_client","nonce":"nonce-e2e","exp":{},"iat":{},"email":"{}"}}"#,
        now + 3600,
        now,
        email
    );
    format!(
        "{}.{}.{}",
        b64::encode(header.as_bytes()),
        b64::encode(payload.as_bytes()),
        b64::encode(b"mock-signature")
    )
}

// ---------------------------------------------------------------------------
// Тесты
// ---------------------------------------------------------------------------

/// Полный desktop-цикл F-5.2→F-5.6: listener → login URL → callback браузера
/// → state-валидация → token exchange → chat/health через proxy. Метрика G4
/// (setup ≤ 30 сек) — на сетевой части flow (без пользователя в браузере).
#[test]
fn desktop_oauth_flow_end_to_end() {
    let started = Instant::now();
    let mock = MockServer::start(ResponsesMode::Normal);

    // 1) listener + login session (шаги 1–2 flow, дизайн-док §4.2)
    let listener = CallbackListener::bind().expect("listener bind");
    let client = OAuthClient::new("device-e2e-1")
        .with_proxy(Some(mock.base_url.clone()))
        .with_timeout(5);
    let session = client
        .build_login_session(listener.redirect_uri())
        .expect("login session");

    // login URL: все обязательные параметры cookbook (§4.2)
    let url = &session.login_url;
    assert!(url.starts_with("https://chatgpt.com/auth/login?"), "{url}");
    for fragment in [
        "client_id=dynamic_agent_client",
        "response_type=code",
        "code_challenge_method=S256",
        "code_challenge=",
        "state=",
        "nonce=",
        "agent_name_hint=CanvasDesk",
        "ext_agent_host_id=device-e2e-1",
        "scope=openid",
        "resource=",
    ] {
        assert!(url.contains(fragment), "login URL без '{fragment}': {url}");
    }

    // 3) «Браузер»: GET callback с кодом и правильным state
    let redirect = listener.redirect_uri().to_string();
    let code = "e2e-auth-code-123".to_string();
    let state = session.state.clone();
    let browser_code = code.clone();
    let browser = std::thread::spawn(move || {
        let addr = redirect
            .trim_start_matches("http://")
            .trim_end_matches("/callback")
            .to_string();
        let mut s = TcpStream::connect(addr).expect("browser: connect");
        let req = format!(
            "GET /callback?code={browser_code}&state={state} HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n"
        );
        s.write_all(req.as_bytes()).expect("browser: send");
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    });

    // 4) wait_for_code → exchange (шаги 3–5)
    let callback = listener
        .wait_for_code(&session.state)
        .expect("wait_for_code");
    assert_eq!(callback.code, code);
    let html = browser.join().expect("browser thread");
    assert!(
        html.contains("Вход выполнен"),
        "браузеру отдали SUCCESS_HTML: {html}"
    );

    let tokens = client
        .exchange_code(&session, &callback.code)
        .expect("exchange");
    assert_eq!(tokens.access_token, "at-e2e-fresh");
    assert_eq!(tokens.refresh_token, "rt-e2e-fresh");
    assert!(
        tokens.expires_at
            > std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_secs(),
        "expires_at в будущем"
    );
    assert_eq!(
        tokens.account_email.as_deref(),
        Some("e2e@example.com"),
        "email из id_token"
    );

    // G4: setup ≤ 30 сек (сетевая часть; в жизни — + время пользователя)
    let elapsed = started.elapsed();
    assert!(
        elapsed <= Duration::from_secs(30),
        "G4 нарушен: setup занял {elapsed:?}"
    );

    // 5) provider через proxy (F-5.10) → chat + health
    let store = MemoryTokenStore::new();
    store.save(&tokens).expect("save tokens");
    let provider =
        ChatGptOAuthProvider::new(Box::new(store), "device-e2e-1", Some(mock.base_url.clone()))
            .with_model("gpt-5.2");
    assert_eq!(provider.api_base(), &format!("{}/v1", mock.base_url));
    assert!(provider.is_signed_in(), "сессия сохранена");

    let text = pollster::block_on(provider.chat(
        &[Message::user("приведи граф CAC→LTV")],
        &Default::default(),
    ))
    .expect("chat");
    assert_eq!(text, "E2E ok");
    pollster::block_on(provider.health()).expect("health");
    assert_eq!(mock.responses_calls(), 1, "один вызов /v1/responses");
    assert_eq!(mock.token_calls(), 1, "только exchange, без refresh");
}

/// state mismatch (CSRF, RFC 6749 §10.12): wait_for_code → Auth, браузеру 403.
#[test]
fn callback_state_mismatch_rejected() {
    let mock = MockServer::start(ResponsesMode::Normal);
    let listener = CallbackListener::bind().expect("listener bind");
    let client = OAuthClient::new("device-e2e-2")
        .with_proxy(Some(mock.base_url.clone()))
        .with_timeout(5);
    let session = client
        .build_login_session(listener.redirect_uri())
        .expect("login session");

    let redirect = listener.redirect_uri().to_string();
    let browser = std::thread::spawn(move || {
        let addr = redirect
            .trim_start_matches("http://")
            .trim_end_matches("/callback")
            .to_string();
        let mut s = TcpStream::connect(addr).expect("browser: connect");
        let req =
            format!("GET /callback?code=x&state=EVIL-STATE HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n");
        s.write_all(req.as_bytes()).expect("browser: send");
        let mut buf = Vec::new();
        let _ = s.read_to_end(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    });

    let err = listener
        .wait_for_code(&session.state)
        .expect_err("чужой state обязан дать Auth");
    assert!(
        matches!(err, LlmError::Auth(_)),
        "ожидали Auth, получили {err:?}"
    );
    let html = browser.join().expect("browser thread");
    assert!(html.contains("403"), "браузеру отдали 403: {html}");
}

// ---------------------------------------------------------------------------
// Авто-refresh (F-5.7)
// ---------------------------------------------------------------------------

/// Тестовый TokenStore с внешним доступом (Arc) — проверка содержимого
/// после auto-refresh внутри provider'а.
#[derive(Clone)]
struct SharedStore(Arc<Mutex<Option<OAuthTokens>>>);

impl SharedStore {
    fn empty() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
    fn with_tokens(tokens: OAuthTokens) -> Self {
        Self(Arc::new(Mutex::new(Some(tokens))))
    }
    fn loaded(&self) -> Option<OAuthTokens> {
        self.0.lock().expect("store lock").clone()
    }
}

impl canvas_llm::chatgpt_oauth::TokenStore for SharedStore {
    fn save(&self, tokens: &OAuthTokens) -> Result<(), LlmError> {
        *self.0.lock().expect("store lock") = Some(tokens.clone());
        Ok(())
    }
    fn load(&self) -> Result<Option<OAuthTokens>, LlmError> {
        Ok(self.loaded())
    }
    fn clear(&self) -> Result<(), LlmError> {
        *self.0.lock().expect("store lock") = None;
        Ok(())
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

/// 401 от Responses API → авто-refresh → ОДНА повторная попытка (F-5.7),
/// store обновлён свежими токенами (ротация refresh_token сохранена).
#[test]
fn chat_401_triggers_refresh_and_retry() {
    let mock = MockServer::start(ResponsesMode::UnauthorizedOnce);
    let store = SharedStore::with_tokens(OAuthTokens {
        access_token: "at-old".into(),
        refresh_token: "rt-old".into(),
        id_token: String::new(),
        expires_at: unix_now() + 3600, // ещё валиден — 401 приходит снаружи
        account_email: None,
    });
    let provider = ChatGptOAuthProvider::new(
        Box::new(store.clone()),
        "device-e2e-3",
        Some(mock.base_url.clone()),
    );

    let text = pollster::block_on(provider.chat(&[Message::user("x")], &Default::default()))
        .expect("chat после refresh+retry");
    assert_eq!(text, "E2E ok");

    let t = store.loaded().expect("сессия в store");
    assert_eq!(t.access_token, "at-e2e-fresh", "store обновлён");
    assert_eq!(t.refresh_token, "rt-e2e-fresh", "ротация refresh_token");
    assert_eq!(
        t.account_email.as_deref(),
        Some("e2e@example.com"),
        "email из свежего id_token"
    );
    assert_eq!(mock.responses_calls(), 2, "401 + один повтор");
    assert_eq!(mock.token_calls(), 1, "ровно один refresh");
}

/// Проактивный refresh: expires_at в прошлом → refresh ДО запроса API
/// (ensure_access_token, REFRESH_MARGIN_SECS).
#[test]
fn expired_token_refreshes_before_request() {
    let mock = MockServer::start(ResponsesMode::Normal);
    let store = SharedStore::with_tokens(OAuthTokens {
        access_token: "at-expired".into(),
        refresh_token: "rt-expired".into(),
        id_token: String::new(),
        expires_at: unix_now() - 10, // истёк
        account_email: None,
    });
    let provider = ChatGptOAuthProvider::new(
        Box::new(store.clone()),
        "device-e2e-4",
        Some(mock.base_url.clone()),
    );

    let text = pollster::block_on(provider.chat(&[Message::user("x")], &Default::default()))
        .expect("chat после проактивного refresh");
    assert_eq!(text, "E2E ok");
    assert_eq!(mock.token_calls(), 1, "refresh до /v1/responses");
    assert_eq!(mock.responses_calls(), 1, "первый же вызов успешен");
    assert_eq!(
        store.loaded().expect("сессия").access_token,
        "at-e2e-fresh",
        "store обновлён до запроса"
    );
}
