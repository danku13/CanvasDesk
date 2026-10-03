//! FR-LLM-A / ADR-0016 §4.5 — `LlmProvider` trait (FIXED API, не менять).
//!
//! Трейт объявлен строго по ADR-0016: 4 операции (`chat`, `choice`,
//! `tool_calling`, `embed`) + `health` + метаданные (`id`, `display_name`,
//! `caps`, `models`, `active_model`). Сигнатуры методов зафиксированы —
//! на них полагаются потоки B, C, D (Settings UI, Suggest mm-source,
//! Agent panel, Graph builder, ChatGPT OAuth).
//!
//! **Двойная компиляция:**
//! - За feature `l1-llm` — полный async-trait с сетевыми методами.
//! - Без feature — stub-трейт с дефолтными реализациями (пустые `&str`,
//!   пустой список моделей). Нужен, чтобы зависимые крейты (canvas-app,
//!   canvas-suggest) компилировались без сети (ADR-0011 wasm-гейт),
//!   храня `Box<dyn LlmProvider>` как тип-заглушку.

// FR-LLM-A: фиксированный API из ADR-0016 §4.5. Методы async только за
// feature `l1-llm` (нужен `async-trait` макрос + `ureq::Agent` для
// блокирующего HTTP, обёрнутого в `spawn_blocking`). Без флага — трейт
// становится stub с дефолтами, зависимые крейты компилируются без сети.

// FR-LLM-A: stub-трейт использует только `ModelInfo` + `ProviderCaps`
// (для дефолтных реализаций). Полный async-трейт (за `l1-llm`) — все типы.
#[cfg(not(feature = "l1-llm"))]
use crate::types::{ModelInfo, ProviderCaps};

#[cfg(feature = "l1-llm")]
use crate::types::{
    ChatOpts, ChoiceAnswer, Message, ModelInfo, ProviderCaps, ToolCall, ToolCallingOpts, ToolDef,
};

/// Единый трейт для всех LLM-провайдеров (FIXED API, ADR-0016 §4.5).
///
/// Реализации (за feature `l1-llm`):
/// - [`crate::openai_compat::OpenAiCompatibleProvider`] — 5 endpoint'ов
///   (OpenAI, OpenRouter, z.ai, Moonshot, Ollama).
/// - [`crate::anthropic::AnthropicClaudeProvider`] — Claude 3.5/4.
///
/// Stream D добавит `ChatGptOAuthProvider` (OAuth, Responses API) и
/// `LayaLegacyProvider` (переиспользует `canvas-suggest/src/laya/client.rs`).
#[cfg(feature = "l1-llm")]
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Идентификатор провайдера (для Settings, логов): `"openai"`,
    /// `"openrouter"`, `"zai"`, `"moonshot"`, `"ollama"`, `"anthropic"`,
    /// `"chatgpt_oauth"`, `"laya"`.
    fn id(&self) -> &str;

    /// Человекочитаемое имя (для UI): `"OpenAI"`, `"Anthropic Claude"`, …
    fn display_name(&self) -> &str;

    /// Возможности провайдера (для UI feature-gating).
    fn caps(&self) -> ProviderCaps;

    /// Список доступных моделей (для dropdown в Settings).
    fn models(&self) -> &[ModelInfo];

    /// Активная модель (выбранная пользователем в Settings).
    fn active_model(&self) -> &str;

    /// Генерация текста (одно- или multi-turn).
    /// Возвращает текст ответа (без tool_calls — для этого есть
    /// [`Self::tool_calling`]).
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, crate::LlmError>;

    /// Choice-ранжирование: документ + опции → вероятности.
    /// Реализация зависит от провайдера (см. ADR-0016 §4.5):
    /// - System One protocol (OpenRouter `/api/alpha/decisions`, Laya)
    ///   → нативные probs.
    /// - OpenAI-compatible → chat с `response_format=json_object`, parse.
    /// - Anthropic → forced tool_use, probs из tool_result.
    async fn choice(
        &self,
        document: &str,
        options: &[crate::OptionDesc],
    ) -> Result<ChoiceAnswer, crate::LlmError>;

    /// Tool-calling: LLM получает список MCP-инструментов + запрос
    /// пользователя, возвращает список `ToolCall` (имя + аргументы).
    /// Для agent panel: приложение исполняет tool_calls через
    /// `graph_apply`, отправляет tool_results обратно (multi-turn).
    async fn tool_calling(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        opts: &ToolCallingOpts,
    ) -> Result<Vec<ToolCall>, crate::LlmError>;

    /// Векторное представление текста (для catalog embeddings, semantic
    /// search). Не все провайдеры поддерживают — `caps().embed`.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, crate::LlmError>;

    /// Проверка доступности (валидация ключа/токена, ping endpoint).
    /// Используется кнопкой «Проверить ключ» в Settings (Stream B).
    async fn health(&self) -> Result<(), crate::LlmError>;
}

// FR-LLM-A: stub-трейт для default-сборки (без `l1-llm`). Зависимые
// крейты могут хранить `Box<dyn LlmProvider>` и обращаться к метаданным,
// но не вызывать сетевые методы (их нет). Сетевые адаптеры подключаются
// только за feature.
//
// Это позволяет:
// - canvas-app: иметь поле `ai_provider: Option<Box<dyn LlmProvider>>`
//   без обязательного ureq в дереве (только при `l1-llm` флаге).
// - canvas-suggest: использовать трейт как тип-параметр `LlmMmSource<P:
//   LlmProvider>` без флага (для тестов stub'ом).
#[cfg(not(feature = "l1-llm"))]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str {
        ""
    }
    fn display_name(&self) -> &str {
        ""
    }
    fn caps(&self) -> ProviderCaps {
        ProviderCaps::default()
    }
    fn models(&self) -> &[ModelInfo] {
        &[]
    }
    fn active_model(&self) -> &str {
        ""
    }
}
