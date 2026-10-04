//! FR-LLM-OAUTH — OAuth 2.0 + PKCE клиент Sign-in-with-ChatGPT
//! (PRD-0010 F-5.1/F-5.2/F-5.4/F-5.7; дизайн-док §4.2).
//!
//! Flow (cookbook, дизайн-док §4.2):
//!
//! ```text
//! 1. build_login_session()  → login_url (PKCE + state + nonce)
//!    [desktop] CallbackListener::bind() → http://127.0.0.1:{port}/callback
//!    [web]     redirect_uri = DEEP_LINK_REDIRECT (canvasdesk://oauth/callback)
//! 2. Пользователь логинится в ChatGPT и подтверждает доступ.
//! 3. OpenAI редиректит на redirect_uri с ?code=..&state=..
//!    [desktop] wait_for_code(): localhost-listener принимает GET,
//!              проверяет state, отвечает HTML «Вход выполнен».
//!    [web]     canvas-web ловит deep-link и передаёт query сюда.
//! 4. exchange_code(): POST {token_url} grant_type=authorization_code …
//! 5. → OAuthTokens {access_token, refresh_token, id_token, expires_at}
//! 6. verify_id_token() (jwt.rs): iss/aud/nonce/exp (+ SignatureVerifier).
//! 7. access_token → Responses API (provider.rs); refresh_tokens() при истечении.
//! ```
//!
//! **Desktop / web (wasm):**
//! - Desktop (`cfg(not(target_arch = "wasm32"))`): localhost TCP-листенер
//!   (`std::net::TcpListener` на `127.0.0.1:0` — порт выбирает ОС) и открытие
//!   браузера (`std::process::Command`, без паник). Реальный HTTPS-обмен
//!   требует feature `l1-llm-tls` (ureq no-TLS не поднимает TLS-рукопожатие).
//! - Web/wasm: только чистые построители (login URL, тела запросов, парсинг);
//!   токен-обмен/refresh идут на `{proxy_url}/oauth/token` — stateless
//!   pass-through облачного воркера (PRD-0010 F-5.10: прокси НЕ хранит
//!   токены), HTTP-транспорт под wasm предоставляет canvas-web (fetch +
//!   cloud-proxy). Никакого `std::net`/`std::process` под wasm — ADR-0011.
//!
//! Константы эндпоинтов/параметров — `pub const` с русскими док-комментариями;
//! источник — дизайн-док §4.2 (cookbook Sign-in-with-ChatGPT, preview).

use crate::error::LlmError;
use std::time::Duration;

use super::jwt::JwksKey;
use super::pkce;
use super::tokens::OAuthTokens;

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

// ============================================================================
// Константы OAuth-flow (дизайн-док §4.2; cookbook Sign-in-with-ChatGPT)
// ============================================================================

/// Хост логина и token endpoint'а ChatGPT. С него стартует браузерный flow
/// (`{AUTH_HOST}{LOGIN_PATH}`) и берутся токены (`{AUTH_HOST}{TOKEN_PATH}`).
pub const AUTH_HOST: &str = "https://chatgpt.com";

/// Путь login-страницы (шаг 1 flow, дизайн-док §4.2).
pub const LOGIN_PATH: &str = "/auth/login";

/// Путь token endpoint'а: обмен `code` → токены и refresh (шаги 4–5).
pub const TOKEN_PATH: &str = "/oauth/token";

/// Путь JWKS-набора для верификации подписи `id_token` (см. `jwt.rs`;
/// SECURITY-стаб — дизайн-док §4.7).
pub const JWKS_PATH: &str = "/.well-known/jwks.json";

/// OAuth `client_id` динамической регистрации OpenAI («dynamic agent
/// client» — секрет в коде не нужен, дизайн-док §4.3).
pub const CLIENT_ID: &str = "dynamic_agent_client";

/// OAuth `response_type` — authorization code flow (RFC 6749 §4.1.1).
pub const RESPONSE_TYPE: &str = "code";

/// OAuth `scope`: профиль пользователя + `offline_access` (refresh_token,
/// дизайн-док §4.3 — долгоживущие сессии).
pub const SCOPE: &str = "openid profile email offline_access";

/// Подсказка имени агента в UI ChatGPT («агент CanvasDesk просит доступ»).
pub const AGENT_NAME_HINT: &str = "CanvasDesk";

/// `resource` — API, для которого выдаётся access_token (Responses API).
pub const RESOURCE: &str = "https://api.openai.com/v1";

/// `origin` — откуда инициирован вход (страница агента ChatGPT).
pub const ORIGIN: &str = "https://chatgpt.com";

/// Redirect URI web-платформы: deep-link, который ловит canvas-web
/// (PRD-0010 F-5.2: web не может слушать localhost).
pub const DEEP_LINK_REDIRECT: &str = "canvasdesk://oauth/callback";

/// Путь callback'а на localhost-листенере (desktop, F-5.2).
pub const CALLBACK_PATH: &str = "/callback";

/// Redirect URI desktop-платформы для порта листенера:
/// `http://127.0.0.1:{port}{CALLBACK_PATH}` (PRD-0010 F-5.2).
pub fn desktop_redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

// ============================================================================
// Построители query / form (чистые функции — покрываются тестами без сети)
// ============================================================================

/// Процентное кодирование значения query/form-параметра (RFC 3986 §2):
/// unreserved (`A-Z a-z 0-9 - . _ ~`) остаются как есть, прочее — `%XX`.
pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &b in value.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Обратное преобразование [`percent_encode`]: `%XX` → байт. `+` оставляется
/// как `+` (это НЕ HTML-form-декод; значения OAuth-параметров генерируются
/// нами или OpenAI и не содержат `+`). Битая последовательность — `None`.
pub fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = bytes.get(i + 1..i + 3)?;
                let hi = (hex[0] as char).to_digit(16)?;
                let lo = (hex[1] as char).to_digit(16)?;
                out.push((hi * 16 + lo) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Тело token-запроса обмена кода (`application/x-www-form-urlencoded`,
/// RFC 6749 §4.1.3): grant_type/client_id/code/code_verifier/redirect_uri/
/// resource. Порядок параметров фиксирован (детерминизм тестов/логов).
pub fn build_exchange_form(code: &str, code_verifier: &str, redirect_uri: &str) -> String {
    format!(
        "grant_type=authorization_code&client_id={}&code={}&code_verifier={}&redirect_uri={}&resource={}",
        percent_encode(CLIENT_ID),
        percent_encode(code),
        percent_encode(code_verifier),
        percent_encode(redirect_uri),
        percent_encode(RESOURCE),
    )
}

/// Тело refresh-запроса (RFC 6749 §6): grant_type=refresh_token + client_id +
/// refresh_token + resource. Истёкший refresh_token → сервер вернёт
/// `invalid_grant` → [`refresh_tokens`] транслирует в `LlmError::Auth`
/// (UI показывает «войти снова», PRD-0010 F-5.9).
pub fn build_refresh_form(refresh_token: &str) -> String {
    format!(
        "grant_type=refresh_token&client_id={}&refresh_token={}&resource={}",
        percent_encode(CLIENT_ID),
        percent_encode(refresh_token),
        percent_encode(RESOURCE),
    )
}

/// Результат построения login-сессии: всё, что нужно держать до обмена кода.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSession {
    /// PKCE code_verifier (секрет до обмена; RFC 7636 §4.1).
    pub verifier: String,
    /// PKCE code_challenge (уходит в login URL).
    pub challenge: String,
    /// CSRF-токен: сверяется с `state` из callback (обязательная проверка).
    pub state: String,
    /// OIDC nonce: сверяется с claim `nonce` в `id_token` (replay-защита).
    pub nonce: String,
    /// Redirect URI этой сессии (localhost deep-link desktop / deep-link web).
    pub redirect_uri: String,
    /// Полный login URL для открытия браузера / редиректа.
    pub login_url: String,
}

/// Параметры, пришедшие в callback от OpenAI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackParams {
    /// Authorization code — одноразовый, живёт минуты (RFC 6749 §4.1.2).
    pub code: String,
    /// `state` из callback — сверяется с `LoginSession.state`.
    pub state: String,
}

/// Построить login URL (шаг 1 flow, дизайн-док §4.2) с уже
/// сгенерированными PKCE/state/nonce. Публично для тестов и web-пути
/// (canvas-web строит redirect без desktop-листенера).
///
/// Все значения проходят [`percent_encode`]; порядок параметров фиксирован.
pub fn build_login_url(
    redirect_uri: &str,
    challenge: &str,
    state: &str,
    nonce: &str,
    ext_agent_host_id: &str,
) -> String {
    format!(
        "{AUTH_HOST}{LOGIN_PATH}\
         ?client_id={client_id}\
         &response_type={response_type}\
         &redirect_uri={redirect_uri}\
         &scope={scope}\
         &code_challenge={challenge}\
         &code_challenge_method={method}\
         &state={state}\
         &nonce={nonce}\
         &agent_name_hint={agent}\
         &ext_agent_host_id={host_id}\
         &resource={resource}\
         &origin={origin}",
        client_id = percent_encode(CLIENT_ID),
        response_type = percent_encode(RESPONSE_TYPE),
        redirect_uri = percent_encode(redirect_uri),
        scope = percent_encode(SCOPE),
        challenge = percent_encode(challenge),
        method = percent_encode(pkce::CODE_CHALLENGE_METHOD),
        state = percent_encode(state),
        nonce = percent_encode(nonce),
        agent = percent_encode(AGENT_NAME_HINT),
        host_id = percent_encode(ext_agent_host_id),
        resource = percent_encode(RESOURCE),
        origin = percent_encode(ORIGIN),
    )
}

/// Разобрать query callback'а (`?code=..&state=..`) → [`CallbackParams`].
///
/// Игнорирует прочие параметры (error/error_description приходят при отказе
/// пользователя — обрабатываются до вызова через [`parse_callback_error`]).
pub fn parse_callback_query(query: &str) -> Result<CallbackParams, LlmError> {
    let query = query.trim_start_matches('?');
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        match k {
            "code" => code = percent_decode(v),
            "state" => state = percent_decode(v),
            _ => {}
        }
    }
    let code = code
        .filter(|c| !c.is_empty())
        .ok_or_else(|| LlmError::Auth("callback без code (вход отменён или истёк)".into()))?;
    let state = state
        .filter(|s| !s.is_empty())
        .ok_or_else(|| LlmError::Auth("callback без state (CSRF-проверка невозможна)".into()))?;
    Ok(CallbackParams { code, state })
}

/// Разобрать query callback'а при отказе пользователя
/// (`?error=access_denied&error_description=..`) → человекочитаемая строка.
pub fn parse_callback_error(query: &str) -> Option<String> {
    let query = query.trim_start_matches('?');
    let mut err = None;
    let mut desc = None;
    for pair in query.split('&') {
        let (k, v) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => continue,
        };
        match k {
            "error" => err = percent_decode(v),
            "error_description" => desc = percent_decode(v),
            _ => {}
        }
    }
    err.map(|e| match desc {
        Some(d) => format!("{e}: {d}"),
        None => e,
    })
}

/// Сверка `state` из callback с ожидаемым — постоянновременное сравнение
/// (RFC 6749 §10.12: защита от CSRF; timing-атаки на строку тут малореальны,
/// но константность бесплатна и дисциплинирует).
pub fn states_equal(expected: &str, received: &str) -> bool {
    let a = expected.as_bytes();
    let b = received.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

// ============================================================================
// OAuthClient — сетевой клиент token endpoint (ureq, feature l1-llm)
// ============================================================================

/// OAuth-клиент Sign-in-with-ChatGPT (PRD-0010 F-5.1).
///
/// Один инстанс на приложение; потокобезопасен (только `&self`-методы,
/// ureq::Agent создаётся на вызов). `ext_agent_host_id` — **persistent
/// device id** (один на установку; генерирует и хранит canvas-app) —
/// OpenAI связывает с ним повторные входы (дизайн-док §4.3).
pub struct OAuthClient {
    /// Хост auth (можно подменить для тестов/проксирования).
    auth_host: String,
    /// Cloud-proxy (PRD-0010 F-5.10): если задан, token exchange и refresh
    /// идут на `{proxy_url}{TOKEN_PATH}` — stateless pass-through, прокси
    /// не хранит токены. `None` — прямой запрос к [`AUTH_HOST`] (desktop).
    proxy_url: Option<String>,
    /// Persistent device id (параметр конструктора).
    ext_agent_host_id: String,
    /// Timeout HTTP-запросов.
    timeout: Duration,
}

impl OAuthClient {
    /// Конструктор: persistent device id (обязателен — параметр login URL).
    pub fn new(ext_agent_host_id: impl Into<String>) -> Self {
        Self {
            auth_host: AUTH_HOST.to_string(),
            proxy_url: None,
            ext_agent_host_id: ext_agent_host_id.into(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Задать cloud-proxy URL (web/wasm путь, F-5.10). `Some(url)` —
    /// token exchange/refresh идут на `{url}{TOKEN_PATH}`.
    pub fn with_proxy(mut self, proxy_url: Option<String>) -> Self {
        self.proxy_url = proxy_url.filter(|u| !u.trim().is_empty());
        self
    }

    /// Текущий cloud-proxy URL (если задан) — F-5.10.
    pub fn proxy_url(&self) -> Option<&str> {
        self.proxy_url.as_deref()
    }

    /// Persistent device id (параметр login URL, дизайн-док §4.3).
    pub fn ext_agent_host_id(&self) -> &str {
        &self.ext_agent_host_id
    }

    /// Подменить auth-хост (для тестов с локальным сервером).
    pub fn with_auth_host(mut self, auth_host: impl Into<String>) -> Self {
        self.auth_host = auth_host.into().trim_end_matches('/').to_string();
        self
    }

    /// Timeout HTTP-запросов (сек).
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// URL token endpoint с учётом proxy (F-5.10): `{proxy|auth_host}/oauth/token`.
    pub fn token_url(&self) -> String {
        let base = self.proxy_url.as_deref().unwrap_or(&self.auth_host);
        format!("{}{TOKEN_PATH}", base.trim_end_matches('/'))
    }

    /// Построить login-сессию: PKCE (verifier+challenge), state, nonce,
    /// login URL. `redirect_uri`:
    /// - desktop — [`desktop_redirect_uri`] от порта [`CallbackListener`];
    /// - web — [`DEEP_LINK_REDIRECT`].
    pub fn build_login_session(&self, redirect_uri: &str) -> Result<LoginSession, LlmError> {
        let verifier = pkce::generate_verifier()?;
        let challenge = pkce::challenge_from_verifier(&verifier);
        let state = pkce::generate_state()?;
        let nonce = pkce::generate_nonce()?;
        let login_url = build_login_url(
            redirect_uri,
            &challenge,
            &state,
            &nonce,
            &self.ext_agent_host_id,
        );
        Ok(LoginSession {
            verifier,
            challenge,
            state,
            nonce,
            redirect_uri: redirect_uri.to_string(),
            login_url,
        })
    }

    /// Обмен authorization code на токены (шаг 4–5 flow, дизайн-док §4.2).
    ///
    /// POST `{token_url}` (`application/x-www-form-urlencoded`), ответ —
    /// JSON `{access_token, refresh_token, id_token, expires_in}` →
    /// [`OAuthTokens`] (`expires_at = now + expires_in`).
    pub fn exchange_code(
        &self,
        session: &LoginSession,
        code: &str,
    ) -> Result<OAuthTokens, LlmError> {
        let form = build_exchange_form(code, &session.verifier, &session.redirect_uri);
        let now = now_unix()?;
        let body = self.post_token_endpoint(&form)?;
        tokens_from_json(&body, now)
    }

    /// Refresh flow (PRD-0010 F-5.7): `grant_type=refresh_token` → новая
    /// пара access/refresh (+id_token). Ответ сервера без нового
    /// `refresh_token` (нет ротации) → старый сохраняется вызывающим кодом.
    ///
    /// Истёкший/отозванный refresh_token (HTTP 400/401 `invalid_grant`) →
    /// `LlmError::Auth` — UI показывает «войти снова» (F-5.9).
    pub fn refresh_tokens(&self, refresh_token: &str) -> Result<OAuthTokens, LlmError> {
        let form = build_refresh_form(refresh_token);
        let now = now_unix()?;
        let body = self.post_token_endpoint(&form)?;
        tokens_from_json(&body, now)
    }

    /// Загрузить JWKS-набор (`GET {auth_host}/.well-known/jwks.json`) для
    /// [`super::jwt::verify_id_token`]. Вызывается один раз на сессию —
    /// кэширование на стороне приложения (дизайн-док §4.4 п.1).
    pub fn fetch_jwks(&self) -> Result<Vec<JwksKey>, LlmError> {
        let url = format!("{}{JWKS_PATH}", self.auth_host.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let resp = agent.get(&url).call().map_err(map_ureq_err("JWKS GET"))?;
        let text = resp
            .into_string()
            .map_err(|e| LlmError::Transport(format!("JWKS: тело не прочитано: {e}")))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| LlmError::Protocol(format!("JWKS: битый JSON: {e}")))?;
        super::jwt::parse_jwks(&value)
    }

    /// POST form в token endpoint с маппингом ошибок OAuth:
    /// 400/401 → `LlmError::Auth` (с `error_description` из тела),
    /// 429 → `LlmError::RateLimit`, остальное → `Transport`.
    fn post_token_endpoint(&self, form: &str) -> Result<serde_json::Value, LlmError> {
        let url = self.token_url();
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let req = agent
            .post(&url)
            .set("content-type", "application/x-www-form-urlencoded")
            .set("origin", ORIGIN);
        match req.send_string(form) {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| LlmError::Transport(format!("token endpoint: {e}")))?;
                serde_json::from_str(&text)
                    .map_err(|e| LlmError::Protocol(format!("token endpoint: битый JSON: {e}")))
            }
            Err(ureq::Error::Status(code, resp)) => {
                // Заголовки читаются ДО слива тела (into_string забирает
                // Response целиком).
                let retry_after_secs = resp
                    .header("retry-after")
                    .and_then(|v| v.trim().parse::<u32>().ok());
                let body = resp.into_string().unwrap_or_default();
                match code {
                    400 | 401 | 403 => {
                        // OAuth-ошибка: {"error": "invalid_grant", ...} (RFC 6749 §5.2).
                        let desc = serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|v| {
                                v.get("error_description")
                                    .or_else(|| v.get("error"))
                                    .and_then(|x| x.as_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or(body);
                        Err(LlmError::Auth(format!("HTTP {code}: {desc}")))
                    }
                    429 => Err(LlmError::RateLimit { retry_after_secs }),
                    _ => Err(LlmError::Transport(format!("HTTP {code}: {body}"))),
                }
            }
            Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
        }
    }
}

/// Текущее unix-время (сек) через `web-time` (M8/W1: на wasm32 идёт через
/// JS `Date.now`, нативно — тонкая обёртка над std; НЕ `std::time` напрямую,
/// чтобы не попасть под wasm-гейт времени FR-079 S3-fix).
pub fn now_unix() -> Result<u64, LlmError> {
    web_time::SystemTime::now()
        .duration_since(web_time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| LlmError::Transport(format!("системное время недоступно: {e}")))
}

/// Разобрать JSON-ответ token endpoint в [`OAuthTokens`] (чистая функция —
/// фикстурные тесты без сети). `now` передаётся снаружи (детерминизм).
///
/// `account_email` здесь `None` — caller достаёт claim `email` из
/// `id_token` через [`super::jwt::parse_id_token`].
pub fn tokens_from_json(body: &serde_json::Value, now: u64) -> Result<OAuthTokens, LlmError> {
    // OAuth-ошибка в JSON с HTTP 200 — нестандартно, но встречается у прокси.
    if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
        return Err(LlmError::Auth(format!("token endpoint: {err}")));
    }
    let access_token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LlmError::Protocol("token endpoint: нет access_token".into()))?;
    let expires_in = body
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| LlmError::Protocol("token endpoint: нет числового expires_in".into()))?;
    Ok(OAuthTokens {
        access_token: access_token.to_string(),
        // Ротация refresh_token не гарантирована (RFC 6749 §6): отсутствие
        // поля → пустая строка, caller сохранит прежний refresh_token.
        refresh_token: body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        id_token: body
            .get("id_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        expires_at: now.saturating_add(expires_in),
        account_email: None,
    })
}

/// Маппинг ошибок ureq → `LlmError` для GET-запросов (JWKS).
pub(crate) fn map_ureq_err(context: &str) -> impl Fn(ureq::Error) -> LlmError + '_ {
    move |e| match e {
        ureq::Error::Status(401 | 403, resp) => {
            let _ = resp.into_string();
            LlmError::Auth(format!("{context}: HTTP 401/403"))
        }
        ureq::Error::Status(429, resp) => {
            let retry = resp
                .header("retry-after")
                .and_then(|v| v.trim().parse::<u32>().ok());
            let _ = resp.into_string();
            LlmError::RateLimit {
                retry_after_secs: retry,
            }
        }
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            LlmError::Transport(format!("{context}: HTTP {code}: {body}"))
        }
        ureq::Error::Transport(t) => LlmError::Transport(format!("{context}: {t}")),
    }
}

// ============================================================================
// Desktop-only: localhost listener + открытие браузера (cfg-гейт ADR-0011)
// ============================================================================

/// HTML-страница, которую видит пользователь после успешного входа
/// (минимальная, без внешних ресурсов — правило «без сети в хосте»).
pub const SUCCESS_HTML: &str = "<!DOCTYPE html><html lang=\"ru\"><head><meta charset=\"utf-8\">\
<title>CanvasDesk</title></head><body style=\"font-family: sans-serif; text-align: center; padding-top: 4em;\">\
<h1>Вход выполнен</h1><p>Можно закрыть окно и вернуться в CanvasDesk.</p>\
</body></html>";

/// HTML-страница при ошибке callback (state mismatch, отказ пользователя).
pub const ERROR_HTML: &str = "<!DOCTYPE html><html lang=\"ru\"><head><meta charset=\"utf-8\">\
<title>CanvasDesk</title></head><body style=\"font-family: sans-serif; text-align: center; padding-top: 4em;\">\
<h1>Вход не завершён</h1><p>Вернитесь в CanvasDesk и попробуйте войти снова.</p>\
</body></html>";

#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::net::TcpStream;

    /// Лимит чтения HTTP-запроса браузера (заголовки callback'а крошечные).
    const MAX_REQUEST_BYTES: usize = 16 * 1024;
    /// Таймаут чтения установленного соединения (браузер шлёт запрос сразу).
    const READ_TIMEOUT: Duration = Duration::from_secs(10);
    /// Сколько «посторонних» запросов (favicon и т.п.) пережидаем в поиске
    /// GET {CALLBACK_PATH} прежде чем сдаться.
    const MAX_NON_CALLBACK_REQUESTS: usize = 8;

    /// Локальный TCP-листенер callback'а (desktop, PRD-0010 F-5.2).
    ///
    /// Биндится на `127.0.0.1:0` (порт выбирает ОС — коллизии невозможны),
    /// отдаёт `redirect_uri` и принимает ОДИН GET callback'а с валидным
    /// `state`. Живёт в потоке вызывающего кода (canvas-app вызывает из
    /// worker-потока — правило «не блокировать рендер-поток», AGENTS.md).
    pub struct CallbackListener {
        listener: TcpListener,
        redirect_uri: String,
    }

    impl CallbackListener {
        /// Занять свободный порт на loopback. Ошибка бинда → `Transport`
        /// (файрвол/нет IPv4-loopback) — graceful degradation F-5.9.
        pub fn bind() -> Result<Self, LlmError> {
            let listener = TcpListener::bind(("127.0.0.1", 0))
                .map_err(|e| LlmError::Transport(format!("не удалось занять порт: {e}")))?;
            let port = listener
                .local_addr()
                .map_err(|e| LlmError::Transport(format!("нет local_addr: {e}")))?
                .port();
            Ok(Self {
                listener,
                redirect_uri: desktop_redirect_uri(port),
            })
        }

        /// Redirect URI этого листенера (`http://127.0.0.1:{port}/callback`).
        pub fn redirect_uri(&self) -> &str {
            &self.redirect_uri
        }

        /// Порт (для логов/диагностики).
        pub fn port(&self) -> u16 {
            self.listener
                .local_addr()
                .map(|a| a.port())
                .unwrap_or_default()
        }

        /// Блокирующе ждать callback OpenAI, проверить `state`, ответить
        /// браузеру HTML-страницей. Возвращает `code` для обмена.
        ///
        /// Посторонние запросы (favicon.ico, автопрефетч) получают 404 и
        /// пропускаются; callback с чужим `state` — 403 + `LlmError::Auth`
        /// (CSRF-отказ обязателен, RFC 6749 §10.12).
        pub fn wait_for_code(&self, expected_state: &str) -> Result<CallbackParams, LlmError> {
            for _ in 0..MAX_NON_CALLBACK_REQUESTS {
                let (mut stream, _) = self
                    .listener
                    .accept()
                    .map_err(|e| LlmError::Transport(format!("accept: {e}")))?;
                let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
                let request = match read_request_head(&mut stream) {
                    Ok(r) => r,
                    Err(e) => {
                        // Мусорный запрос — отвечаем ошибкой и ждём дальше.
                        let _ = respond(&mut stream, 400, ERROR_HTML);
                        return Err(e);
                    }
                };
                let (path_query, _) = request
                    .split_once(' ')
                    .ok_or_else(|| LlmError::Protocol("битая request-line".into()))?;
                let (path, query) = match path_query.split_once('?') {
                    Some((p, q)) => (p, q),
                    None => (path_query, ""),
                };
                if path != CALLBACK_PATH {
                    // favicon.ico и прочие запросы браузера — не callback.
                    let _ = respond(&mut stream, 404, ERROR_HTML);
                    continue;
                }
                if let Some(err_text) = parse_callback_error(query) {
                    let _ = respond(&mut stream, 200, ERROR_HTML);
                    return Err(LlmError::Auth(format!("вход отменён: {err_text}")));
                }
                let params = parse_callback_query(query)?;
                if !states_equal(expected_state, &params.state) {
                    let _ = respond(&mut stream, 403, ERROR_HTML);
                    return Err(LlmError::Auth(
                        "state из callback не совпал с сессией (CSRF)".into(),
                    ));
                }
                let _ = respond(&mut stream, 200, SUCCESS_HTML);
                return Ok(params);
            }
            Err(LlmError::Transport(
                "callback не получен (только посторонние запросы)".into(),
            ))
        }
    }

    /// Прочитать заголовки HTTP-запроса (до `\r\n\r\n`, с лимитом).
    fn read_request_head(stream: &mut TcpStream) -> Result<String, LlmError> {
        let mut buf = Vec::with_capacity(1024);
        let mut chunk = [0u8; 512];
        loop {
            if buf.len() > MAX_REQUEST_BYTES {
                return Err(LlmError::Protocol("запрос слишком большой".into()));
            }
            let n = stream
                .read(&mut chunk)
                .map_err(|e| LlmError::Transport(format!("read: {e}")))?;
            if n == 0 {
                break; // EOF — некоторые клиенты не шлют тело
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8(buf).map_err(|_| LlmError::Protocol("запрос не UTF-8".into()))
    }

    /// Ответить минимальным HTTP: статус + HTML + `Connection: close`.
    fn respond(stream: &mut TcpStream, status: u16, html: &str) -> std::io::Result<()> {
        let reason = match status {
            200 => "OK",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            _ => "OK",
        };
        let body = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: text/html; charset=utf-8\r\n\
             content-length: {}\r\nconnection: close\r\n\r\n{html}",
            html.len()
        );
        stream.write_all(body.as_bytes())?;
        stream.flush()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use desktop::CallbackListener;

/// Открыть системный браузер на `url` (desktop only, PRD-0010 F-5.2).
///
/// Возвращает `io::Result` — НЕ паникует; ошибка спавна (нет xdg-open и
/// т.п.) транслируется вызывающим кодом в тост «откройте ссылку вручную»
/// (graceful degradation F-5.9). cfg-гейт по ОС: `xdg-open` (Linux/Unix),
/// `open` (macOS), `cmd /c start` (Windows).
///
/// Windows-нюанс: URL содержит `&` (разделители query-параметров), а
/// `cmd.exe` трактует его как разделитель команд — поэтому URL оборачивается
/// в кавычки (экранирование на уровне cmd, не shell-injection: URL строится
/// только из [`build_login_url`]).
#[cfg(not(target_arch = "wasm32"))]
pub fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &format!("\"{url}\"")])
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }
}

/// Полный desktop-login (удобство для canvas-app): бинд листенера →
/// login-сессия → браузер → ожидание callback → обмен кода. Возвращает
/// токены + сессию (caller валидирует `id_token` claims через `jwt.rs`
/// и сохраняет токены в `TokenStore`).
///
/// Блокирующий вызов (ждёт пользователя в браузере) — вызывать из
/// worker-потока. Под wasm недоступен (ADR-0011).
#[cfg(not(target_arch = "wasm32"))]
pub fn run_desktop_login(client: &OAuthClient) -> Result<(OAuthTokens, LoginSession), LlmError> {
    let listener = CallbackListener::bind()?;
    let session = client.build_login_session(listener.redirect_uri())?;
    open_browser(&session.login_url)
        .map_err(|e| LlmError::Transport(format!("не удалось открыть браузер: {e}")))?;
    let callback = listener.wait_for_code(&session.state)?;
    let tokens = client.exchange_code(&session, &callback.code)?;
    Ok((tokens, session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_url_contains_all_params_escaped() {
        let url = build_login_url(
            "http://127.0.0.1:8765/callback",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            "st_1",
            "nn_1",
            "host-42",
        );
        assert!(url.starts_with("https://chatgpt.com/auth/login?"));
        assert!(url.contains("client_id=dynamic_agent_client"));
        assert!(url.contains("response_type=code"));
        // Пробелы в scope и слэши в redirect_uri/resource — процент-энкод.
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcallback"));
        assert!(url.contains("scope=openid%20profile%20email%20offline_access"));
        assert!(url.contains("code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=st_1"));
        assert!(url.contains("nonce=nn_1"));
        assert!(url.contains("agent_name_hint=CanvasDesk"));
        assert!(url.contains("ext_agent_host_id=host-42"));
        assert!(url.contains("resource=https%3A%2F%2Fapi.openai.com%2Fv1"));
        assert!(url.contains("origin=https%3A%2F%2Fchatgpt.com"));
    }

    #[test]
    fn percent_encode_decode_roundtrip() {
        assert_eq!(percent_encode("a b&c=d/e"), "a%20b%26c%3Dd%2Fe");
        assert_eq!(
            percent_decode("a%20b%26c%3Dd%2Fe").as_deref(),
            Some("a b&c=d/e")
        );
        assert_eq!(percent_decode("%2B%2F%3D").as_deref(), Some("+/="));
        assert!(percent_decode("%G1").is_none()); // битый hex
        assert!(percent_decode("%2").is_none()); // обрезанная последовательность
                                                 // Круговой trip на «злой» строке.
        let evil = "код&=??%%с пробелом";
        let enc = percent_encode(evil);
        assert_eq!(percent_decode(&enc).as_deref(), Some(evil));
    }

    #[test]
    fn exchange_form_shape() {
        let form = build_exchange_form(
            "ABC123",
            "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
            "http://127.0.0.1:8765/callback",
        );
        let parts: Vec<&str> = form.split('&').collect();
        assert_eq!(
            parts,
            vec![
                "grant_type=authorization_code",
                "client_id=dynamic_agent_client",
                "code=ABC123",
                "code_verifier=dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk",
                "redirect_uri=http%3A%2F%2F127.0.0.1%3A8765%2Fcallback",
                "resource=https%3A%2F%2Fapi.openai.com%2Fv1",
            ]
        );
    }

    #[test]
    fn refresh_form_shape() {
        let form = build_refresh_form("rt-token");
        let parts: Vec<&str> = form.split('&').collect();
        assert_eq!(
            parts,
            vec![
                "grant_type=refresh_token",
                "client_id=dynamic_agent_client",
                "refresh_token=rt-token",
                "resource=https%3A%2F%2Fapi.openai.com%2Fv1",
            ]
        );
    }

    #[test]
    fn callback_query_parse_ok_and_missing() {
        let cb = parse_callback_query("code=abc.def&state=xyz&other=1").unwrap();
        assert_eq!(cb.code, "abc.def");
        assert_eq!(cb.state, "xyz");
        // Процент-энкод значения декодируются.
        let cb = parse_callback_query("code=a%2Fb&state=s%20t").unwrap();
        assert_eq!(cb.code, "a/b");
        assert_eq!(cb.state, "s t");
        // Отсутствие code/state — ошибка (CSRF-проверка обязательна).
        assert!(parse_callback_query("state=xyz").is_err());
        assert!(parse_callback_query("code=abc").is_err());
        assert!(parse_callback_query("").is_err());
    }

    #[test]
    fn callback_error_parse() {
        let err = parse_callback_error("error=access_denied&error_description=user%20declined");
        assert_eq!(err.as_deref(), Some("access_denied: user declined"));
        assert!(parse_callback_error("code=abc").is_none());
    }

    #[test]
    fn state_comparison() {
        assert!(states_equal("abc", "abc"));
        assert!(!states_equal("abc", "abd"));
        assert!(!states_equal("abc", "abcd"));
        assert!(!states_equal("", "a"));
    }

    #[test]
    fn tokens_json_full_and_partial() {
        let now = 1_700_000_000;
        let body = serde_json::json!({
            "access_token": "at_1",
            "refresh_token": "rt_1",
            "id_token": "h.p.s",
            "expires_in": 3600,
        });
        let t = tokens_from_json(&body, now).unwrap();
        assert_eq!(t.access_token, "at_1");
        assert_eq!(t.refresh_token, "rt_1");
        assert_eq!(t.id_token, "h.p.s");
        assert_eq!(t.expires_at, now + 3600);
        assert_eq!(t.account_email, None);

        // Refresh-ответ без refresh_token/id_token — валиден (нет ротации).
        let body = serde_json::json!({"access_token": "at_2", "expires_in": 60});
        let t = tokens_from_json(&body, now).unwrap();
        assert!(t.refresh_token.is_empty());
        assert!(t.id_token.is_empty());
        assert_eq!(t.expires_at, now + 60);
    }

    #[test]
    fn tokens_json_errors_rejected() {
        let now = 1_700_000_000;
        // OAuth-ошибка в теле.
        let err =
            tokens_from_json(&serde_json::json!({"error": "invalid_grant"}), now).unwrap_err();
        assert!(matches!(err, LlmError::Auth(_)));
        // Нет access_token / expires_in → Protocol.
        assert!(matches!(
            tokens_from_json(&serde_json::json!({"expires_in": 60}), now),
            Err(LlmError::Protocol(_))
        ));
        assert!(matches!(
            tokens_from_json(&serde_json::json!({"access_token": "x"}), now),
            Err(LlmError::Protocol(_))
        ));
    }

    #[test]
    fn token_url_proxy_and_direct() {
        let direct = OAuthClient::new("host-1");
        assert_eq!(direct.token_url(), "https://chatgpt.com/oauth/token");
        let proxied =
            OAuthClient::new("host-1").with_proxy(Some("https://proxy.example.com".into()));
        assert_eq!(proxied.token_url(), "https://proxy.example.com/oauth/token");
        // Пустой proxy → прямой путь; trailing slash у прокси срезается.
        let empty = OAuthClient::new("host-1").with_proxy(Some("  ".into()));
        assert_eq!(empty.token_url(), "https://chatgpt.com/oauth/token");
        let slashed = OAuthClient::new("h").with_proxy(Some("https://p.io/".into()));
        assert_eq!(slashed.token_url(), "https://p.io/oauth/token");
    }

    #[test]
    fn desktop_redirect_uri_shape() {
        assert_eq!(desktop_redirect_uri(0), "http://127.0.0.1:0/callback");
        assert_eq!(
            desktop_redirect_uri(65_535),
            "http://127.0.0.1:65535/callback"
        );
    }

    #[test]
    fn success_html_is_russian_minimal() {
        assert!(SUCCESS_HTML.contains("Вход выполнен"));
        assert!(SUCCESS_HTML.contains("utf-8"));
        assert!(ERROR_HTML.contains("не завершён"));
    }
}
