//! FR-LLM-OAUTH / PRD-0010 F-5.6 — `ChatGptOAuthProvider`.
//!
//! Реализация [`LlmProvider`] поверх **Responses API** (`POST /v1/responses`,
//! НЕ `/v1/chat/completions`) с auth через Sign-in-with-ChatGPT:
//!
//! - **Auth:** Bearer access_token из [`TokenStore`]; авто-refresh при
//!   истечении `expires_at` или 401 в ответе (F-5.7); ошибка refresh →
//!   `LlmError::Auth` → UI показывает «войти снова» (F-5.9).
//! - **Proxy (F-5.10):** если в конструктор задан `proxy_url`, запросы идут
//!   на `{proxy_url}/v1/responses` — stateless pass-through облачного
//!   воркера (web/wasm путь); прокси НЕ хранит токены, Bearer-токен клиента
//!   проходит насквозь. Без прокси — прямой `https://api.openai.com/v1`.
//! - **Caps (дизайн-док §4.7):** только OpenAI-модели, streaming/vision нет,
//!   `embed()` → `NotSupported` (embeddings не входят в подписку — §4.7 п.5).
//! - **Модели:** статический список дефолтов подписки; реальный список
//!   приходит из `GET /v1/models` (discovery, F-5.5) — приложение может
//!   закэшировать через [`ChatGptOAuthProvider::discover_models`].
//!
//! Образец реализации — `openai_compat.rs` (ureq + async-trait, парсинг
//! JSON, никакой сети в тестах).

use crate::error::LlmError;
use crate::types::{
    ChatOpts, ChoiceAnswer, JsonVal, Message, ModelInfo, OptionDesc, Pricing, ProviderCaps, Role,
    ToolCall, ToolCallingOpts, ToolChoice, ToolDef,
};
use crate::LlmProvider;

use std::time::Duration;

use super::auth::OAuthClient;
use super::tokens::{OAuthTokens, TokenStore};

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

/// Идентификатор провайдера (`LlmProvider::id`, `settings::LlmProviderId::ChatGptOAuth`).
pub const PROVIDER_ID: &str = "chatgpt_oauth";

/// Дефолтная модель подписки (как в прототипе `AI.modelFor.chatgpt`).
pub const DEFAULT_MODEL: &str = "gpt-5.2";

/// Путь Responses API (относительно api-base).
const RESPONSES_PATH: &str = "/responses";

/// Обновлять access_token заранее за N секунд до `expires_at`
/// (компенсация дрейфа часов и времени запроса).
const REFRESH_MARGIN_SECS: u64 = 60;

/// LLM-провайдер Sign-in-with-ChatGPT (OAuth, Responses API).
///
/// Потокобезопасен (`&self`-методы, `TokenStore: Send + Sync`). Один
/// инстанс на приложение; store — из canvas-app (keychain/OPFS, см.
/// `tokens.rs`), для тестов — [`MemoryTokenStore`](super::MemoryTokenStore).
pub struct ChatGptOAuthProvider {
    /// Хранилище OAuth-токенов (keychain в продукте / память в тестах).
    store: Box<dyn TokenStore>,
    /// OAuth-клиент (login URL, token endpoint, refresh).
    client: OAuthClient,
    /// Активная модель (default [`DEFAULT_MODEL`], прототип `AI.modelFor.chatgpt`).
    model: String,
    /// Статический список моделей подписки (discovery — `discover_models`).
    models: Vec<ModelInfo>,
    /// Timeout HTTP-запросов.
    timeout: Duration,
    /// Base URL Responses API: `RESOURCE` или `{proxy_url}/v1` (F-5.10).
    api_base: String,
}

impl ChatGptOAuthProvider {
    /// Конструктор по образцу `openai_compat`:
    ///
    /// - `store` — [`TokenStore`] (canvas-app: keychain/OPFS; тесты: memory);
    /// - `ext_agent_host_id` — persistent device id (один на установку,
    ///   генерирует canvas-app; уходит в login URL, дизайн-док §4.3);
    /// - `proxy_url` — cloud-proxy для web/wasm (F-5.10); `None` — desktop,
    ///   прямой доступ к `https://api.openai.com/v1`.
    pub fn new(
        store: Box<dyn TokenStore>,
        ext_agent_host_id: impl Into<String>,
        proxy_url: Option<String>,
    ) -> Self {
        let proxy_url = proxy_url.filter(|u| !u.trim().is_empty());
        let client = OAuthClient::new(ext_agent_host_id).with_proxy(proxy_url.clone());
        // F-5.10: с прокси API-путь тоже через воркер ({proxy}/v1/responses),
        // Bearer-токен клиента проходит насквозь (прокси токены не хранит).
        let api_base = match proxy_url.as_deref() {
            Some(proxy) => format!("{}/v1", proxy.trim_end_matches('/')),
            None => super::auth::RESOURCE.to_string(),
        };
        Self {
            store,
            client,
            model: DEFAULT_MODEL.to_string(),
            models: default_models(),
            timeout: Duration::from_secs(60),
            api_base,
        }
    }

    /// Задать активную модель (builder-стиль, как `with_timeout`).
    /// Модель не валидируется по списку — подписка может открыть новые
    /// модели раньше обновления hardcoded-списка (discovery: F-5.5).
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Timeout HTTP-запросов (сек).
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// Установить активную модель с валидацией по списку (`InvalidConfig`,
    /// как `OpenAiCompatibleProvider::set_active_model`).
    pub fn set_active_model(&mut self, model: impl Into<String>) -> Result<(), LlmError> {
        let m = model.into();
        if !self.models.iter().any(|mi| mi.id == m) {
            return Err(LlmError::InvalidConfig(format!(
                "модель '{m}' не в списке провайдера '{PROVIDER_ID}' (список дефолтный — свежие модели подписки доступны через discover_models)"
            )));
        }
        self.model = m;
        Ok(())
    }

    /// Base URL API (для тестов/логов): `RESOURCE` или `{proxy}/v1`.
    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    /// Залогинен ли пользователь (токены в store).
    pub fn is_signed_in(&self) -> bool {
        matches!(self.store.load(), Ok(Some(_)))
    }

    /// E-mail аккаунта (из store, если сохранялся) — для Settings.
    pub fn account_email(&self) -> Option<String> {
        self.store
            .load()
            .ok()
            .flatten()
            .and_then(|t| t.account_email)
    }

    /// Logout: очистить токены (кнопка «Выйти» в Settings).
    pub fn sign_out(&self) -> Result<(), LlmError> {
        self.store.clear()
    }

    // ------------------------------------------------------------------
    // Токены: загрузка / авто-refresh (F-5.7)
    // ------------------------------------------------------------------

    /// Токены из store; нет сессии → `LlmError::Auth` («войти снова»).
    fn load_tokens(&self) -> Result<OAuthTokens, LlmError> {
        self.store
            .load()?
            .ok_or_else(|| LlmError::Auth("нет сохранённой сессии ChatGPT — требуется вход".into()))
    }

    /// Свежий access_token: если `expires_at` близко — refresh + сохранение.
    fn ensure_access_token(&self, tokens: &OAuthTokens) -> Result<String, LlmError> {
        let now = super::auth::now_unix()?;
        if tokens.expires_at > now.saturating_add(REFRESH_MARGIN_SECS) {
            return Ok(tokens.access_token.clone());
        }
        self.refresh_and_save(tokens)
    }

    /// Refresh flow (F-5.7): `grant_type=refresh_token` → новые токены →
    /// в store. Ответ без ротации refresh_token/id_token → сохраняются
    /// прежние (RFC 6749 §6: сервер может не выдавать новый refresh).
    /// Ошибка refresh (invalid_grant и т.п.) → `LlmError::Auth`.
    fn refresh_and_save(&self, old: &OAuthTokens) -> Result<String, LlmError> {
        if old.refresh_token.is_empty() {
            return Err(LlmError::Auth(
                "нет refresh_token — требуется повторный вход".into(),
            ));
        }
        let fresh = self.client.refresh_tokens(&old.refresh_token)?;
        let merged = OAuthTokens {
            refresh_token: if fresh.refresh_token.is_empty() {
                old.refresh_token.clone()
            } else {
                fresh.refresh_token.clone()
            },
            id_token: if fresh.id_token.is_empty() {
                old.id_token.clone()
            } else {
                fresh.id_token.clone()
            },
            account_email: if fresh.id_token.is_empty() {
                old.account_email.clone()
            } else {
                // Свежий id_token — попробовать вытащить email (не критично).
                super::jwt::parse_id_token(&fresh.id_token)
                    .ok()
                    .and_then(|c| c.email)
                    .or_else(|| old.account_email.clone())
            },
            ..fresh
        };
        self.store.save(&merged)?;
        Ok(merged.access_token)
    }

    // ------------------------------------------------------------------
    // Responses API
    // ------------------------------------------------------------------

    /// POST /v1/responses с Bearer-токеном; 401 → refresh + ОДНА повторная
    /// попытка (токен мог быть отозван раньше `expires_at`). Ошибка
    /// refresh → `LlmError::Auth` (F-5.9).
    fn post_responses(&self, body: serde_json::Value) -> Result<serde_json::Value, LlmError> {
        let tokens = self.load_tokens()?;
        let access = self.ensure_access_token(&tokens)?;
        match self.send_responses(&access, &body) {
            Ok(resp) => Ok(resp),
            Err(LlmError::Auth(_)) => {
                let access = self.refresh_and_save(&tokens)?;
                self.send_responses(&access, &body)
            }
            Err(e) => Err(e),
        }
    }

    /// Один HTTP-вызов Responses API с готовым access_token.
    fn send_responses(
        &self,
        access_token: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        let url = format!("{}{RESPONSES_PATH}", self.api_base.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let req = agent
            .post(&url)
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("content-type", "application/json");
        match req.send_json(body.clone()) {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| LlmError::Transport(format!("responses: {e}")))?;
                serde_json::from_str(&text)
                    .map_err(|e| LlmError::Protocol(format!("responses: битый JSON: {e}")))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let retry_after_secs = resp
                    .header("retry-after")
                    .and_then(|v| v.trim().parse::<u32>().ok());
                let body_text = resp.into_string().unwrap_or_default();
                match code {
                    401 | 403 => Err(LlmError::Auth(format!("HTTP {code}: {body_text}"))),
                    429 => Err(LlmError::RateLimit { retry_after_secs }),
                    _ => Err(LlmError::Transport(format!("HTTP {code}: {body_text}"))),
                }
            }
            Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
        }
    }

    /// GET `{api_base}/models` — discovery моделей подписки (F-5.5).
    /// 200 → список id; 401 → `LlmError::Auth` (провоцирует refresh);
    /// 429 → `RateLimit` c `Retry-After`.
    fn get_models_json(&self) -> Result<serde_json::Value, LlmError> {
        let tokens = self.load_tokens()?;
        let access = self.ensure_access_token(&tokens)?;
        let url = format!("{}/models", self.api_base.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        let req = agent
            .get(&url)
            .set("Authorization", &format!("Bearer {access}"));
        match req.call() {
            Ok(resp) => {
                let text = resp
                    .into_string()
                    .map_err(|e| LlmError::Transport(format!("models: {e}")))?;
                serde_json::from_str(&text)
                    .map_err(|e| LlmError::Protocol(format!("models: битый JSON: {e}")))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let retry_after_secs = resp
                    .header("retry-after")
                    .and_then(|v| v.trim().parse::<u32>().ok());
                let _ = resp.into_string();
                match code {
                    401 | 403 => Err(LlmError::Auth(format!("HTTP {code}"))),
                    429 => Err(LlmError::RateLimit { retry_after_secs }),
                    _ => Err(LlmError::Transport(format!("HTTP {code}"))),
                }
            }
            Err(ureq::Error::Transport(t)) => Err(LlmError::Transport(t.to_string())),
        }
    }

    /// Discovery реального списка моделей подписки (`GET /v1/models`,
    /// F-5.5). Приложение может закэшировать результат в Settings.
    pub fn discover_models(&self) -> Result<Vec<String>, LlmError> {
        let json = self.get_models_json()?;
        let data = json
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("models: нет data[]".into()))?;
        Ok(data
            .iter()
            .filter_map(|m| m.get("id").and_then(|v| v.as_str()))
            .map(str::to_string)
            .collect())
    }

    // ------------------------------------------------------------------
    // Чистые построители (unit-тесты без сети)
    // ------------------------------------------------------------------

    /// Сообщения → `input`-элементы Responses API:
    /// - `System` → поле `instructions` (склеиваются через '\n');
    /// - `User`/`Assistant` → `{role, content}` (content строкой);
    /// - `Tool` → `{type: "function_call_output", call_id, output}`
    ///   (multi-turn agent panel: результат вызова инструмента).
    pub(crate) fn split_input(messages: &[Message]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut instructions = String::new();
        let mut input = Vec::with_capacity(messages.len());
        for m in messages {
            match m.role {
                Role::System => {
                    if !instructions.is_empty() {
                        instructions.push('\n');
                    }
                    instructions.push_str(m.content.text());
                }
                Role::Tool => input.push(serde_json::json!({
                    "type": "function_call_output",
                    "call_id": m.tool_call_id.as_deref().unwrap_or(""),
                    "output": m.content.text(),
                })),
                Role::User | Role::Assistant => input.push(serde_json::json!({
                    "role": match m.role {
                        Role::User => "user",
                        _ => "assistant",
                    },
                    "content": m.content.text(),
                })),
            }
        }
        (
            if instructions.is_empty() {
                None
            } else {
                Some(instructions)
            },
            input,
        )
    }

    /// `ToolDef[]` → `tools` Responses API (плоский формат, БЕЗ вложенного
    /// `function`, в отличие от chat/completions):
    /// `[{type:"function", name, description, parameters}]`.
    pub(crate) fn tools_to_json(tools: &[ToolDef]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema.to_serde(),
                })
            })
            .collect()
    }

    /// `ToolCallingOpts.tool_choice` → формат Responses API:
    /// `Auto` → `"auto"`, `None` → `"none"`,
    /// `Specific(name)` → `{type:"function", name}`.
    pub(crate) fn tool_choice_to_json(tc: &ToolChoice) -> serde_json::Value {
        match tc {
            ToolChoice::Auto => serde_json::Value::String("auto".into()),
            ToolChoice::None => serde_json::Value::String("none".into()),
            ToolChoice::Specific(name) => serde_json::json!({ "type": "function", "name": name }),
        }
    }

    /// Тело `chat()`/`tool_calling()` для Responses API (общее). Публично —
    /// интеграционные тесты сверяют shape запроса без сети.
    pub fn build_responses_body(
        model: &str,
        messages: &[Message],
        tools: Option<&[ToolDef]>,
        tool_choice: Option<&ToolChoice>,
        temperature: f32,
        max_tokens: Option<usize>,
    ) -> serde_json::Value {
        let (instructions, input) = Self::split_input(messages);
        let mut body = serde_json::Map::new();
        body.insert("model".into(), serde_json::Value::String(model.to_string()));
        body.insert("input".into(), serde_json::Value::Array(input));
        if let Some(instr) = instructions {
            body.insert("instructions".into(), serde_json::Value::String(instr));
        }
        body.insert(
            "temperature".into(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(temperature as f64)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        if let Some(max) = max_tokens {
            body.insert(
                "max_output_tokens".into(),
                serde_json::Value::Number(serde_json::Number::from(max as u64)),
            );
        }
        if let Some(tools) = tools {
            body.insert(
                "tools".into(),
                serde_json::Value::Array(Self::tools_to_json(tools)),
            );
        }
        if let Some(tc) = tool_choice {
            body.insert("tool_choice".into(), Self::tool_choice_to_json(tc));
        }
        body.insert("stream".into(), serde_json::Value::Bool(false));
        serde_json::Value::Object(body)
    }
}

/// Статический список моделей подписки (дефолт; реальный — из /v1/models,
/// F-5.5). Контекстные окна — оценочные (128k), тариф — подписка ChatGPT
/// (per-token cost для пользователя 0 → `Pricing::free`, cost-панель
/// показывает 0; дизайн-док §4.8: ChatGPT OAuth без per-token биллинга).
fn default_models() -> Vec<ModelInfo> {
    fn subscription_model(id: &str, display_name: &str, context_length: usize) -> ModelInfo {
        ModelInfo {
            id: id.to_string(),
            display_name: display_name.to_string(),
            context_length,
            supports_tools: true,
            supports_vision: false, // caps.vision = false по §4.7
            pricing: Some(Pricing::free()),
        }
    }
    vec![
        subscription_model("gpt-5.2", "GPT-5.2", 128_000),
        subscription_model("gpt-5.2-mini", "GPT-5.2 mini", 128_000),
        subscription_model("gpt-4o", "GPT-4o", 128_000),
        subscription_model("gpt-4o-mini", "GPT-4o mini", 128_000),
        subscription_model("o3", "o3", 200_000),
    ]
}

#[async_trait::async_trait]
impl LlmProvider for ChatGptOAuthProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn display_name(&self) -> &str {
        "ChatGPT (Sign-in)"
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            chat: true,
            // choice через forced tool "rank" (как Anthropic) — доступен.
            choice: true,
            tool_calling: true,
            // §4.7 п.5: embeddings не входят в подписку ChatGPT.
            embed: false,
            // §4.7 п.3: preview — SSE streaming не включаем (резерв).
            streaming: false,
            // §4.7 п.1: только текстовые OpenAI-модели (vision cap false).
            vision: false,
        }
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn active_model(&self) -> &str {
        &self.model
    }

    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError> {
        let body = Self::build_responses_body(
            &self.model,
            messages,
            None,
            None,
            opts.temperature,
            opts.max_tokens,
        );
        let resp = self.post_responses(body)?;
        parse_output_text(&resp)
    }

    async fn choice(
        &self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        // Forced tool "rank" с enum по option_ids (образец — anthropic.rs).
        let enum_vals: Vec<serde_json::Value> = options
            .iter()
            .map(|o| serde_json::Value::String(o.id.clone()))
            .collect();
        let rank_tool = ToolDef {
            name: "rank".into(),
            description: "Return ranking of option ids by best fit.".into(),
            input_schema: JsonVal::from_serde(&serde_json::json!({
                "type": "object",
                "properties": {
                    "ranking": {
                        "type": "array",
                        "items": { "type": "string", "enum": enum_vals },
                    }
                },
                "required": ["ranking"],
            })),
        };
        let opts_desc = options
            .iter()
            .map(|o| {
                format!(
                    "  - {}: {}",
                    o.id,
                    if o.desc.is_empty() {
                        "(no desc)"
                    } else {
                        &o.desc
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let messages = [
            Message::system("You are a suggest engine for CanvasDesk. Use the 'rank' tool to return the ranking of option ids."),
            Message::user(format!(
                "Context (redacted if cloud):\n{document}\n\nOptions:\n{opts_desc}\n\nRank by best fit (best first)."
            )),
        ];
        let body = Self::build_responses_body(
            &self.model,
            &messages,
            Some(std::slice::from_ref(&rank_tool)),
            Some(&ToolChoice::Specific("rank".into())),
            0.0,
            None,
        );
        let resp = self.post_responses(body)?;
        let calls = parse_function_calls(&resp)?;
        let call = calls.first().ok_or_else(|| {
            LlmError::Protocol("нет function_call в ответе (forced choice failed)".into())
        })?;
        let ranking = match &call.arguments {
            JsonVal::Object(pairs) => pairs.iter().find(|(k, _)| k == "ranking").map(|(_, v)| v),
            _ => None,
        }
        .ok_or_else(|| LlmError::Protocol("нет ranking в аргументах rank".into()))?;
        // Position-based probs: 1-й = 1.0, 2-й = 0.5, … (как в anthropic.rs;
        // fusion в canvas-suggest калибрует Platt-преобразованием).
        let items = match ranking {
            JsonVal::Array(items) => items.clone(),
            _ => return Err(LlmError::Protocol("ranking не массив".into())),
        };
        let probs: Vec<(String, f64)> = items
            .iter()
            .enumerate()
            .filter_map(|(i, v)| match v {
                JsonVal::String(id) => Some((id.clone(), 1.0 / (i + 1) as f64)),
                _ => None,
            })
            .collect();
        let sum: f64 = probs.iter().map(|(_, p)| p).sum();
        let top1 = probs.first().map(|(_, p)| *p).unwrap_or(0.0);
        let confidence = if sum > 0.0 {
            (top1 / sum).min(1.0)
        } else {
            0.0
        };
        Ok(ChoiceAnswer { probs, confidence })
    }

    async fn tool_calling(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        opts: &ToolCallingOpts,
    ) -> Result<Vec<ToolCall>, LlmError> {
        let body = Self::build_responses_body(
            &self.model,
            messages,
            Some(tools),
            Some(&opts.tool_choice),
            opts.temperature,
            opts.max_tokens,
        );
        let resp = self.post_responses(body)?;
        parse_function_calls(&resp)
    }

    async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError> {
        // §4.7 п.5: embeddings НЕ входят в подписку ChatGPT — для catalog
        // embeddings использовать BYOK (OpenAI/z.ai). Ошибка фиксирована
        // и совпадает с caps().embed == false.
        Err(LlmError::NotSupported("embed"))
    }

    async fn health(&self) -> Result<(), LlmError> {
        // GET /v1/models: 200 → Ok; 401 → Auth (провоцирует refresh/UI
        // «войти снова»); 429 → RateLimit с Retry-After. Discovery-тело
        // здесь не нужно — достаточно статуса.
        self.get_models_json().map(|_| ())
    }
}

// ============================================================================
// Парсинг ответа Responses API (чистые функции — тесты без сети)
// ============================================================================

/// Достать текст ответа Responses API:
/// 1) поле `output_text` (SDK-совместимое агрегированное поле, если прокси
///    его добавляет);
/// 2) иначе проход `output[]`: элементы `type == "message"` с массивом
///    `content[]`, где `type == "output_text"` → конкатенация `text`.
pub fn parse_output_text(resp: &serde_json::Value) -> Result<String, LlmError> {
    if let Some(text) = resp.get("output_text").and_then(|v| v.as_str()) {
        return Ok(text.to_string());
    }
    let output = resp
        .get("output")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LlmError::Protocol("responses: нет output[]".into()))?;
    let mut text = String::new();
    for item in output {
        if item.get("type").and_then(|v| v.as_str()) != Some("message") {
            continue;
        }
        if let Some(content) = item.get("content").and_then(|v| v.as_array()) {
            for part in content {
                if part.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                    if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                        text.push_str(t);
                    }
                }
            }
        }
    }
    // Пустой текст при завершённом статусе — протокольный дрейф.
    if text.is_empty() {
        return Err(LlmError::Protocol(
            "responses: нет output_text в output[]".into(),
        ));
    }
    Ok(text)
}

/// Достать `function_call`-элементы `output[]` → `Vec<ToolCall>`:
/// `id` = `call_id` (парный к `function_call_output.call_id`; fallback — `id`),
/// `arguments` — JSON-строка → [`JsonVal`] (битая строка → `Null`, как в
/// `openai_compat`).
pub fn parse_function_calls(resp: &serde_json::Value) -> Result<Vec<ToolCall>, LlmError> {
    let output = resp
        .get("output")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LlmError::Protocol("responses: нет output[]".into()))?;
    Ok(output
        .iter()
        .filter(|item| item.get("type").and_then(|v| v.as_str()) == Some("function_call"))
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let id = item
                .get("call_id")
                .and_then(|v| v.as_str())
                .or_else(|| item.get("id").and_then(|v| v.as_str()))?
                .to_string();
            let args_str = item
                .get("arguments")
                .and_then(|v| v.as_str())
                .unwrap_or("null");
            let args_val: serde_json::Value =
                serde_json::from_str(args_str).unwrap_or(serde_json::Value::Null);
            Some(ToolCall {
                id,
                name,
                arguments: JsonVal::from_serde(&args_val),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> ChatGptOAuthProvider {
        ChatGptOAuthProvider::new(
            Box::new(super::super::MemoryTokenStore::new()),
            "test-host-id",
            None,
        )
    }

    #[test]
    fn defaults_and_metadata() {
        let p = provider();
        assert_eq!(p.id(), "chatgpt_oauth");
        assert_eq!(p.display_name(), "ChatGPT (Sign-in)");
        // Дефолт модели — как в прототипе AI.modelFor.chatgpt.
        assert_eq!(p.active_model(), "gpt-5.2");
        assert_eq!(p.api_base(), "https://api.openai.com/v1");
        assert!(!p.is_signed_in());
        assert!(p.account_email().is_none());
    }

    #[test]
    fn caps_per_design_4_7() {
        let caps = provider().caps();
        assert!(caps.chat);
        assert!(caps.choice); // forced tool "rank"
        assert!(caps.tool_calling);
        assert!(!caps.embed, "embeddings не входят в подписку (§4.7 п.5)");
        assert!(!caps.streaming, "preview — streaming выключен (§4.7 п.3)");
        assert!(!caps.vision, "только текстовые модели (§4.7 п.1)");
    }

    #[test]
    fn default_models_list() {
        let p = provider();
        let ids: Vec<&str> = p.models().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["gpt-5.2", "gpt-5.2-mini", "gpt-4o", "gpt-4o-mini", "o3"]
        );
        // Подписка: per-token cost 0 (Pricing::free), tools да, vision нет.
        assert!(p
            .models()
            .iter()
            .all(|m| m.pricing == Some(Pricing::free())));
        assert!(p.models().iter().all(|m| m.supports_tools));
        assert!(p.models().iter().all(|m| !m.supports_vision));
    }

    #[test]
    fn with_model_and_set_active_model() {
        let p = provider().with_model("gpt-4o");
        assert_eq!(p.active_model(), "gpt-4o");
        let mut p2 = provider();
        assert!(p2.set_active_model("o3").is_ok());
        assert_eq!(p2.active_model(), "o3");
        // Модель вне статического списка — InvalidConfig (список дефолтный).
        assert!(matches!(
            p2.set_active_model("gpt-9"),
            Err(LlmError::InvalidConfig(_))
        ));
        assert_eq!(p2.active_model(), "o3");
    }

    #[test]
    fn api_base_via_proxy_f5_10() {
        let p = ChatGptOAuthProvider::new(
            Box::new(super::super::MemoryTokenStore::new()),
            "host",
            Some("https://proxy.example.com".into()),
        );
        assert_eq!(p.api_base(), "https://proxy.example.com/v1");
    }

    #[test]
    fn split_input_maps_roles() {
        let messages = vec![
            Message::system("правило 1"),
            Message::system("правило 2"),
            Message::user("вопрос"),
            Message::assistant("ответ"),
            Message::tool("call_9", "результат"),
        ];
        let (instructions, input) = ChatGptOAuthProvider::split_input(&messages);
        assert_eq!(instructions.as_deref(), Some("правило 1\nправило 2"));
        // 5 сообщений − 2 system (уходят в instructions) = 3 input-элемента.
        assert_eq!(input.len(), 3);
        assert_eq!(input[0]["role"], "user");
        assert_eq!(input[0]["content"], "вопрос");
        assert_eq!(input[1]["role"], "assistant");
        // Tool → function_call_output с call_id (multi-turn agent panel).
        assert_eq!(input[2]["type"], "function_call_output");
        assert_eq!(input[2]["call_id"], "call_9");
        assert_eq!(input[2]["output"], "результат");
    }

    #[test]
    fn build_responses_body_shape() {
        let tools = vec![ToolDef::new(
            "graph_apply",
            "Apply ops",
            JsonVal::object([("type", JsonVal::string("object"))]),
        )];
        let body = ChatGptOAuthProvider::build_responses_body(
            "gpt-5.2",
            &[Message::user("hi")],
            Some(&tools),
            Some(&ToolChoice::Specific("graph_apply".into())),
            0.7,
            Some(1024),
        );
        assert_eq!(body["model"], "gpt-5.2");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "hi");
        // Плоский tools-формат Responses API (не вложенный function).
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "graph_apply");
        assert_eq!(body["tools"][0]["parameters"]["type"], "object");
        assert!(body["tools"][0].get("function").is_none());
        assert_eq!(body["tool_choice"]["type"], "function");
        assert_eq!(body["tool_choice"]["name"], "graph_apply");
        // max_output_tokens (НЕ max_tokens) + temperature + stream:false.
        assert_eq!(body["max_output_tokens"], 1024);
        assert!(body.get("max_tokens").is_none());
        // f32→f64 даёт артефакты представления — сравниваем приближённо.
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert_eq!(body["stream"], false);
        assert!(body.get("instructions").is_none());
    }

    #[test]
    fn tool_choice_mapping() {
        assert_eq!(
            ChatGptOAuthProvider::tool_choice_to_json(&ToolChoice::Auto),
            serde_json::json!("auto")
        );
        assert_eq!(
            ChatGptOAuthProvider::tool_choice_to_json(&ToolChoice::None),
            serde_json::json!("none")
        );
        assert_eq!(
            ChatGptOAuthProvider::tool_choice_to_json(&ToolChoice::Specific("rank".into())),
            serde_json::json!({"type": "function", "name": "rank"})
        );
    }

    #[test]
    fn parse_output_text_field_and_items() {
        // Вариант 1: агрегированное output_text (SDK/прокси).
        let resp = serde_json::json!({"output_text": "готово"});
        assert_eq!(parse_output_text(&resp).unwrap(), "готово");
        // Вариант 2: output[] c message/output_text (raw API).
        let resp = serde_json::json!({
            "id": "resp_1",
            "status": "completed",
            "output": [
                {"type": "reasoning", "summary": []},
                {"type": "message", "role": "assistant", "content": [
                    {"type": "output_text", "text": "Привет, "},
                    {"type": "output_text", "text": "мир!"}
                ]}
            ]
        });
        assert_eq!(parse_output_text(&resp).unwrap(), "Привет, мир!");
        // Нет ни поля, ни message → Protocol.
        let resp = serde_json::json!({"output": [{"type": "reasoning"}]});
        assert!(matches!(
            parse_output_text(&resp),
            Err(LlmError::Protocol(_))
        ));
        let resp = serde_json::json!({});
        assert!(matches!(
            parse_output_text(&resp),
            Err(LlmError::Protocol(_))
        ));
    }

    #[test]
    fn parse_function_calls_from_output() {
        let resp = serde_json::json!({
            "output": [
                {"type": "reasoning"},
                {
                    "type": "function_call",
                    "id": "fc_1",
                    "call_id": "call_1",
                    "name": "graph_apply",
                    "arguments": "{\"ops\": [1, 2]}"
                }
            ]
        });
        let calls = parse_function_calls(&resp).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1"); // call_id приоритетнее id
        assert_eq!(calls[0].name, "graph_apply");
        assert_eq!(
            calls[0].arguments,
            JsonVal::Object(vec![(
                "ops".into(),
                JsonVal::Array(vec![JsonVal::Number(1.0), JsonVal::Number(2.0)])
            )])
        );
        // Без call_id — fallback на id.
        let resp = serde_json::json!({
            "output": [{"type": "function_call", "id": "fc_2", "name": "t", "arguments": "null"}]
        });
        let calls = parse_function_calls(&resp).unwrap();
        assert_eq!(calls[0].id, "fc_2");
        // Битая JSON-строка аргументов → Null (не паника).
        let resp = serde_json::json!({
            "output": [{"type": "function_call", "id": "fc_3", "name": "t", "arguments": "{oops"}]
        });
        let calls = parse_function_calls(&resp).unwrap();
        assert_eq!(calls[0].arguments, JsonVal::Null);
    }
}
