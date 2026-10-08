//! FR-LLM-A / ADR-0016 §4.6 (Группа 1) — `OpenAiCompatibleProvider`.
//!
//! Единый адаптер для 5 провайдеров с OpenAI-compatible API:
//! - **OpenAI** (`api.openai.com/v1`) — gpt-4o, gpt-4o-mini, o1.
//! - **OpenRouter** (`openrouter.ai/api/v1`) — 100+ моделей + System One
//!   (`/api/alpha/decisions` для нативных probs в choice).
//! - **z.ai** (`api.z.ai/v1`) — glm-5.3-flash, glm-5.3-flashx
//!   (p@1=1.000 на benchmark).
//! - **Moonshot** (`api.moonshot.ai/v1`) — kimi-k3, kimi-k2 (vision input).
//! - **Ollama** (`localhost:11434/v1`) — local, без ключа.
//!
//! Провайдеры отличаются только `base_url` + `api_key` + списком моделей
//! (hardcoded, см. `openai()` / `openrouter()` / `zai()` / `moonshot()` /
//! `ollama()`). Auth — `Authorization: Bearer {api_key}` (Ollama без auth).
//!
//! **Transport (W1, `docs/plans/llm-waves-w1-w2-w3.md`):** все HTTP-вызовы
//! идут через [`crate::transport::HttpTransport`] (натив — UreqTransport;
//! wasm — WasmFetchTransport за `wasm-fetch`, волна W3). HTTPS-эндпоинты
//! на нативе требуют feature `l1-llm-tls`. desktop: прямые запросы
//! к провайдеру, ключ в OS keychain (НЕ в config.toml). Web: cloud-proxy
//! (PRD-0010 F-5.10, деплой `cloud/llm-proxy`).

use crate::error::LlmError;
use crate::transport::{HttpRequest, HttpTransport, UreqTransport};
use crate::types::{
    ChatOpts, ChoiceAnswer, JsonVal, Message, ModelInfo, OptionDesc, Pricing, ProviderCaps, Role,
    ToolCall, ToolCallingOpts, ToolDef,
};
use crate::LlmProvider;

use std::sync::Arc;
use std::time::Duration;

/// OpenAI-compatible адаптер для 5 провайдеров.
///
/// Потокобезопасен (`HttpTransport: Send + Sync`, состояние без мутаций).
/// Один инстанс на приложение (или на feature — suggest/graph/agent могут
/// иметь разные `model`/`endpoint`).
pub struct OpenAiCompatibleProvider {
    /// Идентификатор провайдера (для Settings): `"openai"`, `"openrouter"`,
    /// `"zai"`, `"moonshot"`, `"ollama"`.
    id: &'static str,
    /// Человекочитаемое имя (для UI): `"OpenAI"`, `"OpenRouter"`, …
    display_name: &'static str,
    /// Base URL: `https://api.openai.com/v1` и т.п. (без trailing slash).
    base_url: String,
    /// API-ключ (Bearer). Пустая строка — для Ollama (без auth).
    api_key: String,
    /// Активная модель (из списка `models`).
    model: String,
    /// Список доступных моделей (hardcoded в preset-конструкторах).
    models: Vec<ModelInfo>,
    /// Timeout запроса (10 сек по умолчанию, см. NF-5: 10с для suggest).
    timeout: Duration,
    /// Является ли OpenRouter (для choice через `/api/alpha/decisions`).
    is_openrouter: bool,
    /// W1: HTTP-транспорт (натив — UreqTransport; тесты — MockTransport;
    /// wasm — WasmFetchTransport, волна W3).
    transport: Arc<dyn HttpTransport>,
}

impl OpenAiCompatibleProvider {
    /// Базовый конструктор (для кастомных endpoint'ов — self-hosted).
    /// Для стандартных провайдеров используйте `openai()` / `openrouter()` /
    /// `zai()` / `moonshot()` / `ollama()`.
    pub fn new(
        id: &'static str,
        display_name: &'static str,
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        models: Vec<ModelInfo>,
    ) -> Self {
        Self {
            id,
            display_name,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            models,
            timeout: Duration::from_secs(30),
            is_openrouter: false,
            transport: Arc::new(UreqTransport::new()),
        }
    }

    /// Timeout запроса (для suggest — 10с, для graph/agent — больше).
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// W1: подменить HTTP-транспорт (builder-стиль).
    ///
    /// Тесты — [`crate::transport::MockTransport`] (без сети); web-сборка
    /// (волна W3) — [`crate::transport::WasmFetchTransport`] (браузерный
    /// fetch к cloud-proxy). Конструкторы по умолчанию ставят UreqTransport,
    /// поэтому существующие вызовы (canvas-app/llm_factory) не меняются.
    pub fn with_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self {
        self.transport = transport;
        self
    }

    /// Base URL провайдера (для тестов и логов). Без trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// API-ключ (пустая строка для Ollama — без auth).
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Является ли OpenRouter (для choice через `/api/alpha/decisions`).
    pub fn is_openrouter(&self) -> bool {
        self.is_openrouter
    }

    /// Установить активную модель (после выбора в Settings UI).
    /// Возвращает `InvalidConfig` если модель не в списке `models`.
    pub fn set_active_model(&mut self, model: impl Into<String>) -> Result<(), LlmError> {
        let m = model.into();
        if !self.models.iter().any(|mi| mi.id == m) {
            return Err(LlmError::InvalidConfig(format!(
                "модель '{m}' не в списке провайдера '{}'",
                self.id
            )));
        }
        self.model = m;
        Ok(())
    }

    /// Preset: OpenAI (api.openai.com/v1).
    pub fn openai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo {
                id: "gpt-4o".into(),
                display_name: "GPT-4o".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 2.50,
                    output_per_mtok: 10.00,
                }),
            },
            ModelInfo {
                id: "gpt-4o-mini".into(),
                display_name: "GPT-4o mini".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 0.15,
                    output_per_mtok: 0.60,
                }),
            },
            ModelInfo {
                id: "o1-mini".into(),
                display_name: "o1-mini".into(),
                context_length: 128_000,
                supports_tools: false,
                supports_vision: false,
                pricing: Some(Pricing {
                    input_per_mtok: 3.0,
                    output_per_mtok: 12.0,
                }),
            },
        ];
        Self::new(
            "openai",
            "OpenAI",
            "https://api.openai.com/v1",
            api_key,
            model,
            models,
        )
    }

    /// Preset: OpenRouter (openrouter.ai/api/v1). Включает System One
    /// (`/api/alpha/decisions`) для нативных choice probs.
    pub fn openrouter(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo {
                id: "z-ai/glm-5.3-flash".into(),
                display_name: "GLM 5.3 Flash (free preview)".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: false,
                pricing: Some(Pricing {
                    input_per_mtok: 0.121,
                    output_per_mtok: 0.121,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-5.3-flashx".into(),
                display_name: "GLM 5.3 FlashX".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: false,
                pricing: Some(Pricing {
                    input_per_mtok: 0.121,
                    output_per_mtok: 0.121,
                }),
            },
            ModelInfo {
                id: "nemotron/nemotron-super:free".into(),
                display_name: "Nemotron Super (free)".into(),
                context_length: 32_000,
                supports_tools: false,
                supports_vision: false,
                pricing: None, // free
            },
            ModelInfo {
                id: "anthropic/claude-3.5-sonnet".into(),
                display_name: "Claude 3.5 Sonnet (via OpenRouter)".into(),
                context_length: 200_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 3.0,
                    output_per_mtok: 15.0,
                }),
            },
            ModelInfo {
                id: "moonshotai/kimi-k3".into(),
                display_name: "Kimi K3 (via OpenRouter)".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 0.55,
                    output_per_mtok: 2.19,
                }),
            },
        ];
        let mut p = Self::new(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            api_key,
            model,
            models,
        );
        p.is_openrouter = true;
        p
    }

    /// Preset: z.ai (api.z.ai/v1) — GLM 5.3 Flash, p@1=1.000 на benchmark.
    pub fn zai(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo {
                id: "glm-5.3-flash".into(),
                display_name: "GLM 5.3 Flash".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: false,
                pricing: Some(Pricing {
                    input_per_mtok: 0.121,
                    output_per_mtok: 0.121,
                }),
            },
            ModelInfo {
                id: "glm-5.3-flashx".into(),
                display_name: "GLM 5.3 FlashX".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: false,
                pricing: Some(Pricing {
                    input_per_mtok: 0.121,
                    output_per_mtok: 0.121,
                }),
            },
        ];
        Self::new(
            "zai",
            "z.ai (GLM)",
            "https://api.z.ai/v1",
            api_key,
            model,
            models,
        )
    }

    /// Preset: Moonshot Kimi (api.moonshot.ai/v1) — vision input.
    pub fn moonshot(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo {
                id: "kimi-k3".into(),
                display_name: "Kimi K3".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 0.55,
                    output_per_mtok: 2.19,
                }),
            },
            ModelInfo {
                id: "kimi-k2".into(),
                display_name: "Kimi K2".into(),
                context_length: 128_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 0.55,
                    output_per_mtok: 2.19,
                }),
            },
        ];
        Self::new(
            "moonshot",
            "Moonshot (Kimi)",
            "https://api.moonshot.ai/v1",
            api_key,
            model,
            models,
        )
    }

    /// Preset: Ollama (localhost:11434/v1) — local, без ключа.
    /// Только desktop (wasm требует cloud-proxy — Stream D).
    pub fn ollama(model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo::free_local("llama3", "Llama 3 (Ollama)", 8192, false, false),
            ModelInfo::free_local("llama3.1", "Llama 3.1 (Ollama)", 128_000, true, false),
            ModelInfo::free_local("qwen2.5", "Qwen 2.5 (Ollama)", 32_000, true, false),
            ModelInfo::free_local("mistral", "Mistral (Ollama)", 32_000, true, false),
        ];
        Self::new(
            "ollama",
            "Ollama (local)",
            "http://localhost:11434/v1",
            "", // без auth
            model,
            models,
        )
    }

    /// POST JSON по полному URL с auth + маппинг статусов (async — W1).
    async fn post_json_url(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        let mut req = HttpRequest::post_json(url, body, self.timeout);
        if !self.api_key.is_empty() {
            req = req.with_header("Authorization", format!("Bearer {}", self.api_key));
        }
        let resp = self.transport.execute(req).await?;
        if let Some(err) = resp.map_status("openai-compat") {
            return Err(err);
        }
        serde_json::from_str(&resp.body_str())
            .map_err(|e| LlmError::Protocol(format!("битый JSON ответа: {e}")))
    }

    /// POST JSON к `{base_url}/{path}` (async — W1).
    async fn post_json(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        self.post_json_url(&url, body).await
    }

    /// Преобразовать `Message` в OpenAI-совместимый JSON.
    /// `system` → `{role:"system", content}`. `user` → `{role:"user", content}`.
    /// `assistant` → `{role:"assistant", content}`. `tool` →
    /// `{role:"tool", tool_call_id, content}`.
    fn message_to_json(msg: &Message) -> serde_json::Value {
        let role = match msg.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };
        let mut obj = serde_json::Map::new();
        obj.insert("role".into(), serde_json::Value::String(role.into()));
        obj.insert(
            "content".into(),
            serde_json::Value::String(msg.content.text().into()),
        );
        if let Some(id) = &msg.tool_call_id {
            obj.insert("tool_call_id".into(), serde_json::Value::String(id.clone()));
        }
        serde_json::Value::Object(obj)
    }
}

#[async_trait::async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        self.id
    }

    fn display_name(&self) -> &str {
        self.display_name
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            chat: true,
            // choice через `/api/alpha/decisions` только у OpenRouter; для
            // остальных — chat-fallback (всё равно считается доступным).
            choice: true,
            tool_calling: true,
            // embed есть у OpenAI (text-embedding-3-small) и z.ai; у Moonshot
            // и Ollama — зависит. Ставим true — если поддерживается, провайдер
            // ответит; иначе `LlmError::NotSupported` от API.
            embed: true,
            streaming: false, // Stream A не реализует streaming (Stream D — ChatGPT OAuth).
            vision: self.models.iter().any(|m| m.supports_vision),
        }
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn active_model(&self) -> &str {
        &self.model
    }

    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError> {
        let msgs_json: Vec<serde_json::Value> =
            messages.iter().map(Self::message_to_json).collect();
        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        body.insert("messages".into(), serde_json::Value::Array(msgs_json));
        body.insert(
            "temperature".into(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(opts.temperature as f64)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        if let Some(max) = opts.max_tokens {
            body.insert(
                "max_tokens".into(),
                serde_json::Value::Number(serde_json::Number::from(max as u64)),
            );
        }
        if opts.response_format == crate::types::ResponseFormat::JsonObject {
            body.insert(
                "response_format".into(),
                serde_json::json!({ "type": "json_object" }),
            );
        }
        body.insert("stream".into(), serde_json::Value::Bool(false));

        let resp = self
            .post_json("chat/completions", &serde_json::Value::Object(body))
            .await?;
        // resp.choices[0].message.content
        let content = resp
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::Protocol("нет choices[0].message.content".into()))?;
        Ok(content.to_string())
    }

    async fn choice(
        &self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        if self.is_openrouter {
            // OpenRouter System One: POST /api/alpha/decisions (нативные probs).
            // Тело как у Laya `/v1/systemone` (см. canvas-suggest/src/laya/client.rs).
            self.choice_openrouter_systemone(document, options).await
        } else {
            // Chat-fallback: prompt template + response_format=json_object.
            self.choice_chat_fallback(document, options).await
        }
    }

    async fn tool_calling(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        opts: &ToolCallingOpts,
    ) -> Result<Vec<ToolCall>, LlmError> {
        let msgs_json: Vec<serde_json::Value> =
            messages.iter().map(Self::message_to_json).collect();
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema.to_serde(),
                    }
                })
            })
            .collect();

        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        body.insert("messages".into(), serde_json::Value::Array(msgs_json));
        body.insert("tools".into(), serde_json::Value::Array(tools_json));
        body.insert(
            "temperature".into(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(opts.temperature as f64)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        if let Some(max) = opts.max_tokens {
            body.insert(
                "max_tokens".into(),
                serde_json::Value::Number(serde_json::Number::from(max as u64)),
            );
        }
        let tool_choice = match &opts.tool_choice {
            crate::types::ToolChoice::Auto => serde_json::Value::String("auto".into()),
            crate::types::ToolChoice::None => serde_json::Value::String("none".into()),
            crate::types::ToolChoice::Specific(name) => serde_json::json!({
                "type": "function",
                "function": { "name": name }
            }),
        };
        body.insert("tool_choice".into(), tool_choice);
        body.insert("stream".into(), serde_json::Value::Bool(false));

        let resp = self
            .post_json("chat/completions", &serde_json::Value::Object(body))
            .await?;
        let tool_calls = resp
            .pointer("/choices/0/message/tool_calls")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет choices[0].message.tool_calls".into()))?;

        let out: Vec<ToolCall> = tool_calls
            .iter()
            .filter_map(|tc| {
                let id = tc.get("id")?.as_str()?.to_string();
                let func = tc.get("function")?;
                let name = func.get("name")?.as_str()?.to_string();
                let args_str = func.get("arguments")?.as_str().unwrap_or("null");
                let args_val: serde_json::Value =
                    serde_json::from_str(args_str).unwrap_or(serde_json::Value::Null);
                Some(ToolCall {
                    id,
                    name,
                    arguments: JsonVal::from_serde(&args_val),
                })
            })
            .collect();
        Ok(out)
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError> {
        let inputs: Vec<serde_json::Value> = texts
            .iter()
            .map(|t| serde_json::Value::String((*t).to_string()))
            .collect();
        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        body.insert("input".into(), serde_json::Value::Array(inputs));

        let resp = self
            .post_json("embeddings", &serde_json::Value::Object(body))
            .await?;
        let data = resp
            .get("data")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет data в ответе embeddings".into()))?;
        let out: Vec<Vec<f32>> = data
            .iter()
            .filter_map(|d| {
                d.get("embedding").and_then(|e| e.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|x| x as f32))
                        .collect()
                })
            })
            .collect();
        Ok(out)
    }

    async fn health(&self) -> Result<(), LlmError> {
        // W1: GET /models через ИНЖЕКТИРОВАННЫЙ транспорт (не глобальный
        // UreqTransport) — mock в тестах, WasmFetchTransport в web (W3)
        // работают одинаково. Семантика прежняя: 2xx → Ok, 401/403 → Auth,
        // 429 → RateLimit (Retry-After), прочее → Transport.
        let url = format!("{}/models", self.base_url);
        let mut req = HttpRequest::get(url, self.timeout);
        if !self.api_key.is_empty() {
            req = req.with_header("Authorization", format!("Bearer {}", self.api_key));
        }
        let resp = self.transport.execute(req).await?;
        match resp.map_status("health-check") {
            None => Ok(()),
            Some(err) => Err(err),
        }
    }
}

impl OpenAiCompatibleProvider {
    /// OpenRouter System One choice (нативные probs через `/api/alpha/decisions`).
    /// Протокол тот же что Laya `/v1/systemone` (см. `canvas-suggest/src/laya/client.rs:70`).
    async fn choice_openrouter_systemone(
        &self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        let mut criteria = serde_json::Map::new();
        for o in options {
            let v = if o.desc.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::String(o.desc.clone())
            };
            criteria.insert(o.id.clone(), v);
        }
        let body = serde_json::json!({
            "state": { "document": document },
            "questions": {
                "main": {
                    "type": "choice",
                    "instructions": "Which template best fits the node being edited?",
                    "criteria": criteria,
                }
            },
            "model": self.model,
        });
        let url = format!("{}/api/alpha/decisions", self.base_url);
        // W1: тот же transport-путь (URL целиком — путь выходит за `{base}/v1`).
        let resp_value: serde_json::Value = self.post_json_url(&url, &body).await?;

        // answers.main.probabilities + answer_confidence (как Laya).
        let main = resp_value
            .pointer("/answers/main")
            .ok_or_else(|| LlmError::Protocol("нет answers.main".into()))?;
        let probs_map = main
            .get("probabilities")
            .and_then(|p| p.as_object())
            .ok_or_else(|| LlmError::Protocol("нет answers.main.probabilities".into()))?;
        let mut probs: Vec<(String, f64)> = probs_map
            .iter()
            .map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0)))
            .collect();
        // Сортировка как в Laya: убывание prob, при равенстве — id по возрастанию.
        probs.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        let confidence = main
            .get("answer_confidence")
            .and_then(|c| c.as_f64())
            .unwrap_or(0.0);
        Ok(ChoiceAnswer { probs, confidence })
    }

    /// Chat-fallback для choice: prompt template + response_format=json_object.
    /// Используется всеми OpenAI-compat провайдерами кроме OpenRouter.
    async fn choice_chat_fallback(
        &self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        let opts_str = options
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
        let system = "You are a suggest engine for CanvasDesk. Pick the template that best fits the user's context. Respond as JSON: {\"ranking\": [\"id1\", \"id2\", ...]} — best first. Only include ids from the options list.";
        let user = format!(
            "Context (redacted if cloud):\n{document}\n\nOptions:\n{opts_str}\n\nRank by best fit."
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "temperature": 0.0,
            "response_format": { "type": "json_object" },
            "stream": false,
        });

        let resp = self.post_json("chat/completions", &body).await?;
        let content = resp
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LlmError::Protocol("нет choices[0].message.content".into()))?;

        let parsed: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| LlmError::Protocol(format!("парсинг JSON ответа: {e}")))?;
        let ranking = parsed
            .get("ranking")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет ranking в JSON ответа".into()))?;

        // Преобразовать ранжирование в probs: position-based
        // (1-й = 1.0, 2-й = 0.5, 3-й = 0.33, ...). Не нативные probs,
        // но достаточно для fusion (α·lex + (1−α)·llm) — Platt-калибровка
        // (canvas-suggest/src/fusion.rs) корректирует.
        let n = ranking.len().max(1);
        let probs: Vec<(String, f64)> = ranking
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let id = v.as_str()?.to_string();
                let p = 1.0 / (i + 1) as f64;
                Some((id, p))
            })
            .collect();
        let confidence = probs.first().map(|(_, p)| *p).unwrap_or(0.0).min(1.0);
        // Нормализуем confidence через softmax-подобное преобразование:
        // если у топ-1 большая доля — confidence высокий. Здесь грубо:
        // confidence = top1 / sum.
        let sum: f64 = probs.iter().map(|(_, p)| p).sum();
        let normalized_confidence = if sum > 0.0 {
            (confidence / sum).min(1.0)
        } else {
            0.0
        };
        let _ = n; // silence unused warning
        Ok(ChoiceAnswer {
            probs,
            confidence: normalized_confidence,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_preset() {
        let p = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o-mini");
        assert_eq!(p.id(), "openai");
        assert_eq!(p.display_name(), "OpenAI");
        assert_eq!(p.active_model(), "gpt-4o-mini");
        assert_eq!(p.base_url, "https://api.openai.com/v1");
        assert!(!p.is_openrouter);
        let caps = p.caps();
        assert!(caps.chat);
        assert!(caps.choice);
        assert!(caps.tool_calling);
        assert!(caps.embed);
        assert!(!caps.streaming);
        // GPT-4o — vision
        assert!(caps.vision);
        assert!(p.models().len() >= 3);
        // Все модели с pricing
        assert!(p.models().iter().all(|m| m.pricing.is_some()));
    }

    #[test]
    fn openrouter_preset_marks_flag() {
        let p = OpenAiCompatibleProvider::openrouter("sk-or-test", "z-ai/glm-5.3-flash");
        assert_eq!(p.id(), "openrouter");
        assert!(p.is_openrouter);
        assert!(p.caps().choice); // через /api/alpha/decisions
    }

    #[test]
    fn openrouter_has_free_nemotron() {
        let p = OpenAiCompatibleProvider::openrouter("sk", "nemotron/nemotron-super:free");
        assert!(p.models().iter().any(|m| m.pricing.is_none()));
    }

    #[test]
    fn zai_preset() {
        let p = OpenAiCompatibleProvider::zai("sk-zai", "glm-5.3-flash");
        assert_eq!(p.id(), "zai");
        assert_eq!(p.base_url, "https://api.z.ai/v1");
        assert_eq!(p.active_model(), "glm-5.3-flash");
    }

    #[test]
    fn moonshot_preset_vision() {
        let p = OpenAiCompatibleProvider::moonshot("sk-ms", "kimi-k3");
        assert_eq!(p.id(), "moonshot");
        assert!(p.caps().vision); // Kimi поддерживает vision
    }

    #[test]
    fn ollama_preset_no_auth() {
        let p = OpenAiCompatibleProvider::ollama("llama3");
        assert_eq!(p.id(), "ollama");
        assert!(p.api_key.is_empty());
        assert!(p.base_url.starts_with("http://localhost"));
        // Все модели free/local (pricing = None)
        assert!(p.models().iter().all(|m| m.pricing.is_none()));
    }

    #[test]
    fn set_active_model_valid() {
        let mut p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
        assert!(p.set_active_model("gpt-4o-mini").is_ok());
        assert_eq!(p.active_model(), "gpt-4o-mini");
    }

    #[test]
    fn set_active_model_invalid_rejected() {
        let mut p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
        let err = p.set_active_model("nonexistent-model").unwrap_err();
        assert!(matches!(err, LlmError::InvalidConfig(_)));
        // Активная модель не изменилась
        assert_eq!(p.active_model(), "gpt-4o");
    }

    #[test]
    fn base_url_trims_trailing_slash() {
        let p = OpenAiCompatibleProvider::new(
            "custom",
            "Custom",
            "https://example.com/v1/",
            "key",
            "m",
            vec![],
        );
        assert_eq!(p.base_url, "https://example.com/v1");
    }

    #[test]
    fn message_to_json_roles() {
        let m = Message::user("hi");
        let v = OpenAiCompatibleProvider::message_to_json(&m);
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "hi");

        let s = Message::system("rules");
        assert_eq!(
            OpenAiCompatibleProvider::message_to_json(&s)["role"],
            "system"
        );

        let t = Message::tool("call_1", "result");
        let v = OpenAiCompatibleProvider::message_to_json(&t);
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "call_1");
    }

    #[test]
    fn message_to_json_multimodal_text_extracted() {
        // Multimodal — для OpenAI-compat берётся только text (vision-формат
        // не реализован в Stream A, резерв под Stream D).
        let m = Message {
            role: Role::User,
            content: crate::types::MessageContent::Multimodal {
                text: "see image".into(),
                images: vec!["data:image/png;base64,...".into()],
            },
            tool_call_id: None,
        };
        let v = OpenAiCompatibleProvider::message_to_json(&m);
        assert_eq!(v["content"], "see image");
    }
}
