//! FR-LLM-A / ADR-0016 §4.6 (Группа 2) — `AnthropicClaudeProvider`.
//!
//! Отдельный адаптер для Anthropic Claude (отличный от OpenAI API):
//! - Endpoint: `POST https://api.anthropic.com/v1/messages`.
//! - Auth: `x-api-key: {key}` + `anthropic-version: 2023-06-01`.
//! - System prompt: отдельное поле `system` (не в `messages`).
//! - Tool-calling: `tools=[{name, description, input_schema}]`,
//!   ответ содержит `content` с блоками `type=="tool_use"`.
//! - Choice: forced tool_use (tool "rank" с enum=option_ids,
//!   `tool_choice={type:"tool", name:"rank"}`).
//! - Embeddings: **не поддерживаются** (`embed()` → `NotSupported`).
//!
//! **Transport (W1):** все HTTP-вызовы через [`crate::transport::HttpTransport`]
//! (натив — UreqTransport; wasm — WasmFetchTransport за `wasm-fetch`, W3).
//! **Модели:** claude-3-5-sonnet, claude-3-5-haiku, claude-3-opus
//! (hardcoded список, тарифы из открытого прайса Anthropic).

use crate::error::LlmError;
use crate::transport::{HttpRequest, HttpTransport, UreqTransport};
use crate::types::{
    ChatOpts, ChoiceAnswer, JsonVal, Message, ModelInfo, OptionDesc, Pricing, ProviderCaps, Role,
    ToolCall, ToolCallingOpts, ToolChoice, ToolDef,
};
use crate::LlmProvider;

use std::sync::Arc;
use std::time::Duration;

/// Anthropic Claude адаптер (`/v1/messages`, `x-api-key`, `tool_use`).
pub struct AnthropicClaudeProvider {
    api_key: String,
    model: String,
    models: Vec<ModelInfo>,
    timeout: Duration,
    /// W1: HTTP-транспорт (натив — UreqTransport; тесты — MockTransport;
    /// wasm — WasmFetchTransport, волна W3).
    transport: Arc<dyn HttpTransport>,
}

impl AnthropicClaudeProvider {
    /// Конструктор с указанным API-ключом и моделью. Модель должна быть
    /// из списка `models()` (валидируется через `set_active_model`).
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let models = vec![
            ModelInfo {
                id: "claude-3-5-sonnet-20241022".into(),
                display_name: "Claude 3.5 Sonnet".into(),
                context_length: 200_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 3.0,
                    output_per_mtok: 15.0,
                }),
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".into(),
                display_name: "Claude 3.5 Haiku".into(),
                context_length: 200_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 0.80,
                    output_per_mtok: 4.0,
                }),
            },
            ModelInfo {
                id: "claude-3-opus-20240229".into(),
                display_name: "Claude 3 Opus (legacy)".into(),
                context_length: 200_000,
                supports_tools: true,
                supports_vision: true,
                pricing: Some(Pricing {
                    input_per_mtok: 15.0,
                    output_per_mtok: 75.0,
                }),
            },
        ];
        Self {
            api_key: api_key.into(),
            model: model.into(),
            models,
            timeout: Duration::from_secs(30),
            transport: Arc::new(UreqTransport::new()),
        }
    }

    /// Timeout запроса (для suggest — 10с, для graph/agent — больше).
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout = Duration::from_secs(secs);
        self
    }

    /// W1: подменить HTTP-транспорт (builder-стиль; по умолчанию —
    /// UreqTransport, существующие вызовы не меняются).
    pub fn with_transport(mut self, transport: Arc<dyn HttpTransport>) -> Self {
        self.transport = transport;
        self
    }

    /// Установить активную модель. `InvalidConfig` если модель не в списке.
    pub fn set_active_model(&mut self, model: impl Into<String>) -> Result<(), LlmError> {
        let m = model.into();
        if !self.models.iter().any(|mi| mi.id == m) {
            return Err(LlmError::InvalidConfig(format!(
                "модель '{m}' не в списке Anthropic"
            )));
        }
        self.model = m;
        Ok(())
    }

    /// Базовый URL Anthropic API (constexpr для тестов).
    const BASE_URL: &'static str = "https://api.anthropic.com/v1";

    /// POST к `/v1/messages` с auth-заголовками Anthropic (async — W1).
    async fn post_messages(&self, body: serde_json::Value) -> Result<serde_json::Value, LlmError> {
        let url = format!("{}/messages", Self::BASE_URL);
        let req = HttpRequest::post_json(&url, &body, self.timeout)
            .with_header("x-api-key", &self.api_key)
            .with_header("anthropic-version", "2023-06-01");
        let resp = self.transport.execute(req).await?;
        if let Some(err) = resp.map_status("anthropic") {
            return Err(err);
        }
        serde_json::from_str(&resp.body_str())
            .map_err(|e| LlmError::Protocol(format!("битый JSON ответа: {e}")))
    }

    /// Разделить messages на system-промпт (отдельное поле у Anthropic)
    /// и rest (user/assistant/tool).
    fn split_system(messages: &[Message]) -> (String, Vec<&Message>) {
        let mut system = String::new();
        let mut rest = Vec::with_capacity(messages.len());
        for m in messages {
            if matches!(m.role, Role::System) {
                if !system.is_empty() {
                    system.push('\n');
                }
                system.push_str(m.content.text());
            } else {
                rest.push(m);
            }
        }
        (system, rest)
    }

    /// Преобразовать rest-сообщения в формат Anthropic (`role`: user/assistant,
    /// `content`: строка или массив блоков).
    fn rest_to_json(messages: &[&Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    // Anthropic: tool-результат как role:"user" с content
                    // type:"tool_result".
                    Role::Tool => "user",
                    Role::System => "user", // не должно быть (отфильтровано)
                };
                if matches!(m.role, Role::Tool) {
                    serde_json::json!({
                        "role": role,
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": m.tool_call_id.as_deref().unwrap_or(""),
                            "content": m.content.text(),
                        }],
                    })
                } else {
                    serde_json::json!({
                        "role": role,
                        "content": m.content.text(),
                    })
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LlmProvider for AnthropicClaudeProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn display_name(&self) -> &str {
        "Anthropic Claude"
    }

    fn caps(&self) -> ProviderCaps {
        ProviderCaps {
            chat: true,
            choice: true, // через forced tool_use
            tool_calling: true,
            embed: false,     // Anthropic не имеет embeddings API
            streaming: false, // Stream A не реализует streaming
            vision: true,     // Claude 3.5+ поддерживает vision
        }
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn active_model(&self) -> &str {
        &self.model
    }

    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError> {
        let (system, rest) = Self::split_system(messages);
        let msgs_json = Self::rest_to_json(&rest);

        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        if !system.is_empty() {
            body.insert("system".into(), serde_json::Value::String(system));
        }
        body.insert("messages".into(), serde_json::Value::Array(msgs_json));
        body.insert(
            "max_tokens".into(),
            serde_json::Value::Number(serde_json::Number::from(
                opts.max_tokens.unwrap_or(4096) as u64
            )),
        );
        body.insert(
            "temperature".into(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(opts.temperature as f64)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );

        let resp = self.post_messages(serde_json::Value::Object(body)).await?;
        // resp.content[0].text (type=="text")
        let content = resp
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет content в ответе".into()))?;
        let text = content
            .iter()
            .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| LlmError::Protocol("нет content[].text в ответе".into()))?;
        Ok(text.to_string())
    }

    async fn choice(
        &self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        // Forced tool_use: создаём tool "rank" с enum = option_ids.
        let enum_vals: Vec<serde_json::Value> = options
            .iter()
            .map(|o| serde_json::Value::String(o.id.clone()))
            .collect();

        let input_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "ranking": {
                    "type": "array",
                    "items": { "type": "string", "enum": enum_vals },
                }
            },
            "required": ["ranking"],
        });

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
        let user_msg = format!(
            "Context (redacted if cloud):\n{document}\n\nOptions:\n{opts_desc}\n\nRank by best fit (best first). Return ranking array."
        );

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "temperature": 0.0,
            "system": "You are a suggest engine for CanvasDesk. Use the 'rank' tool to return the ranking of template ids.",
            "messages": [
                { "role": "user", "content": user_msg },
            ],
            "tools": [{
                "name": "rank",
                "description": "Return ranking of template ids by best fit.",
                "input_schema": input_schema,
            }],
            "tool_choice": { "type": "tool", "name": "rank" },
        });

        let resp = self.post_messages(body).await?;
        // Найти content-блок type=="tool_use" с name=="rank".
        let content = resp
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет content в ответе".into()))?;
        let tool_use = content
            .iter()
            .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .ok_or_else(|| {
                LlmError::Protocol("нет tool_use в ответе (forced choice failed)".into())
            })?;
        let args = tool_use
            .get("input")
            .ok_or_else(|| LlmError::Protocol("нет tool_use.input".into()))?;
        let ranking = args
            .get("ranking")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет ranking в tool_use.input".into()))?;

        // Position-based probs (как в openai_compat choice_chat_fallback).
        let probs: Vec<(String, f64)> = ranking
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let id = v.as_str()?.to_string();
                let p = 1.0 / (i + 1) as f64;
                Some((id, p))
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
        let (system, rest) = Self::split_system(messages);
        let msgs_json = Self::rest_to_json(&rest);
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema.to_serde(),
                })
            })
            .collect();

        let mut body = serde_json::Map::new();
        body.insert(
            "model".into(),
            serde_json::Value::String(self.model.clone()),
        );
        if !system.is_empty() {
            body.insert("system".into(), serde_json::Value::String(system));
        }
        body.insert("messages".into(), serde_json::Value::Array(msgs_json));
        body.insert("tools".into(), serde_json::Value::Array(tools_json));
        body.insert(
            "max_tokens".into(),
            serde_json::Value::Number(serde_json::Number::from(
                opts.max_tokens.unwrap_or(4096) as u64
            )),
        );
        body.insert(
            "temperature".into(),
            serde_json::Value::Number(
                serde_json::Number::from_f64(opts.temperature as f64)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ),
        );
        let tool_choice = match &opts.tool_choice {
            ToolChoice::Auto => serde_json::json!({ "type": "auto" }),
            ToolChoice::None => serde_json::json!({ "type": "none" }),
            ToolChoice::Specific(name) => serde_json::json!({
                "type": "tool",
                "name": name,
            }),
        };
        body.insert("tool_choice".into(), tool_choice);

        let resp = self.post_messages(serde_json::Value::Object(body)).await?;
        let content = resp
            .get("content")
            .and_then(|v| v.as_array())
            .ok_or_else(|| LlmError::Protocol("нет content в ответе".into()))?;
        let out: Vec<ToolCall> = content
            .iter()
            .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
            .filter_map(|b| {
                let id = b.get("id")?.as_str()?.to_string();
                let name = b.get("name")?.as_str()?.to_string();
                let input = b.get("input")?;
                Some(ToolCall {
                    id,
                    name,
                    arguments: JsonVal::from_serde(input),
                })
            })
            .collect();
        Ok(out)
    }

    async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError> {
        // Anthropic не имеет embeddings API — fallback на OpenAI/z.ai
        // (для catalog embeddings использовать BYOK-cloud с OpenAI).
        Err(LlmError::NotSupported("embed"))
    }

    async fn health(&self) -> Result<(), LlmError> {
        // Anthropic не имеет публичного /v1/models (или требует особый план).
        // Делаем минимальный запрос /v1/messages с 1 токеном — если 200/400
        // (валидация модели), ключ валиден. 401/403 — нет.
        let url = format!("{}/messages", Self::BASE_URL);
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{ "role": "user", "content": "ping" }],
        });
        let req = HttpRequest::post_json(&url, &body, self.timeout)
            .with_header("x-api-key", &self.api_key)
            .with_header("anthropic-version", "2023-06-01");
        let resp = self.transport.execute(req).await?;
        match resp.status {
            200..=299 => Ok(()),
            400 => {
                // 400 — модель не существует / неверный формат, но ключ
                // валиден. Для health-check это ОК.
                let body_text = resp.body_str();
                if body_text.contains("model")
                    || body_text.contains("invalid")
                    || body_text.contains("not_found")
                {
                    Ok(())
                } else {
                    Err(LlmError::Transport(format!("HTTP 400: {body_text}")))
                }
            }
            _ => resp.map_status("anthropic health").map_or(Ok(()), Err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anthropic_preset() {
        let p = AnthropicClaudeProvider::new("sk-ant-test", "claude-3-5-sonnet-20241022");
        assert_eq!(p.id(), "anthropic");
        assert_eq!(p.display_name(), "Anthropic Claude");
        assert_eq!(p.active_model(), "claude-3-5-sonnet-20241022");
        let caps = p.caps();
        assert!(caps.chat);
        assert!(caps.choice);
        assert!(caps.tool_calling);
        assert!(!caps.embed); // Anthropic без embeddings
        assert!(!caps.streaming);
        assert!(caps.vision);
        assert!(p.models().len() >= 3);
        // Все модели с pricing (нет free-tier у Anthropic)
        assert!(p.models().iter().all(|m| m.pricing.is_some()));
    }

    #[test]
    fn set_active_model_valid() {
        let mut p = AnthropicClaudeProvider::new("sk", "claude-3-5-sonnet-20241022");
        assert!(p.set_active_model("claude-3-5-haiku-20241022").is_ok());
        assert_eq!(p.active_model(), "claude-3-5-haiku-20241022");
    }

    #[test]
    fn set_active_model_invalid_rejected() {
        let mut p = AnthropicClaudeProvider::new("sk", "claude-3-5-sonnet-20241022");
        assert!(matches!(
            p.set_active_model("nonexistent").unwrap_err(),
            LlmError::InvalidConfig(_)
        ));
    }

    #[test]
    fn split_system_extracts_system_messages() {
        let messages = vec![
            Message::system("rules"),
            Message::user("hi"),
            Message::system("more rules"),
            Message::assistant("ok"),
        ];
        let (system, rest) = AnthropicClaudeProvider::split_system(&messages);
        assert_eq!(system, "rules\nmore rules");
        assert_eq!(rest.len(), 2);
        assert!(matches!(rest[0].role, Role::User));
        assert!(matches!(rest[1].role, Role::Assistant));
    }

    #[test]
    fn rest_to_json_user_assistant() {
        let messages = [Message::user("hi"), Message::assistant("hello")];
        let refs: Vec<&Message> = messages.iter().collect();
        let json = AnthropicClaudeProvider::rest_to_json(&refs);
        assert_eq!(json[0]["role"], "user");
        assert_eq!(json[0]["content"], "hi");
        assert_eq!(json[1]["role"], "assistant");
        assert_eq!(json[1]["content"], "hello");
    }

    #[test]
    fn rest_to_json_tool_result_block() {
        let m = Message::tool("call_1", "result");
        let refs: Vec<&Message> = vec![&m];
        let json = AnthropicClaudeProvider::rest_to_json(&refs);
        // Anthropic: tool-результат как role:"user" с content-type:"tool_result"
        assert_eq!(json[0]["role"], "user");
        let content = json[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "call_1");
        assert_eq!(content[0]["content"], "result");
    }

    #[test]
    fn caps_no_embed_for_anthropic() {
        // Anthropic не имеет embeddings API — caps().embed == false.
        // Async-метод `embed()` тестируется через caps в Stream A
        // (без запуска runtime); integration-тесты с mock HTTP — в Stream B.
        let p = AnthropicClaudeProvider::new("sk", "claude-3-5-sonnet-20241022");
        assert!(!p.caps().embed);
    }
}
