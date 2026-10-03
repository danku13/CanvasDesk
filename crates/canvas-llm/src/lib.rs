//! # canvas-llm — LLM-слой CanvasDesk (FR-LLM-A / ADR-0016).
//!
//! Унифицированный `LlmProvider` trait для 6 провайдеров (BYOK + Sign-in-with-
//! ChatGPT). Feature-gated: дефолтная сборка без сети (ADR-0011 wasm-гейт,
//! AGENTS.md «без сети в хосте»). Сетевые адаптеры (`OpenAiCompatibleProvider`,
//! `AnthropicClaudeProvider`) подключаются за feature `l1-llm`.
//!
//! ## Состав
//!
//! - [`provider`] — `LlmProvider` trait (FIXED API, ADR-0016 §4.5).
//! - [`types`] — `Message`, `ToolDef`, `ToolCall`, `ChoiceAnswer`, `ChatOpts`,
//!   `ToolCallingOpts`, `ModelInfo`, `ProviderCaps`, `Pricing`, `CostEstimate`,
//!   `ResponseFormat`, `ToolChoice`, `JsonVal`, `OptionDesc`.
//! - [`error`] — `LlmError`.
//! - [`redact`] — Q1 (privacy): `redact_context()`.
//! - [`cost`] — Q4 (cost visibility): `estimate_cost()`, `actual_cost()`,
//!   `estimate_tokens()`.
//! - [`compliance`] — Q1+Q5: `DataResidency`, `PrivacyMode`.
//! - [`settings`] — `LlmSettings` (новые поля Settings, Stream B вшивает).
//! - [`health`] — `check_endpoint()` helper (feature `l1-llm` only).
//! - [`openai_compat`] — `OpenAiCompatibleProvider` (5 endpoint'ов, feature
//!   `l1-llm`).
//! - [`anthropic`] — `AnthropicClaudeProvider` (feature `l1-llm`).
//!
//! ## Stream D (не трогать)
//!
//! - `crates/canvas-llm/src/chatgpt_oauth/` — OAuth-клиент (PKCE, JWKS,
//!   refresh, Responses API). Stream D владеет этой директорией.
//!
//! ## Wasm-гейт (ADR-0011)
//!
//! Дефолтная сборка (default features = []) компилируется под
//! wasm32-unknown-unknown без сети и без serde. Зависимые крейты
//! (canvas-app, canvas-suggest) могут хранить `Box<dyn LlmProvider>` без
//! обязательного ureq в дереве — feature `l1-llm` подключается только в
//! нативной (desktop) сборке.
//!
//! Web (wasm): cloud-proxy (Stream D `crates/canvas-web/src/llm_proxy.rs`)
//! держит API-ключи в env, `canvas-llm` делает fetch к proxy.

// FR-LLM-A: маркер для поиска (grep): все новые файлы/добавления помечены
// `// FR-LLM-A:` в комментариях.

pub mod compliance;
pub mod cost;
pub mod error;
pub mod provider;
pub mod redact;
pub mod settings;
pub mod types;

/// Health-check helper (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod health;

/// OpenAI-compatible адаптер для 5 провайдеров (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod openai_compat;

/// Anthropic Claude адаптер (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod anthropic;

// Re-export публичного API.
pub use compliance::{DataResidency, PrivacyMode};
pub use cost::{actual_cost, estimate_cost, estimate_tokens};
pub use error::LlmError;
pub use provider::LlmProvider;
pub use redact::redact_context;
pub use settings::{LlmProviderId, LlmSettings};
pub use types::{
    ChatOpts, ChoiceAnswer, CostEstimate, JsonVal, Message, MessageContent, ModelInfo, OptionDesc,
    Pricing, ProviderCaps, ResponseFormat, Role, ToolCall, ToolCallingOpts, ToolChoice, ToolDef,
};

// Сетевые адаптеры — только за feature `l1-llm`.
#[cfg(feature = "l1-llm")]
pub use anthropic::AnthropicClaudeProvider;
#[cfg(feature = "l1-llm")]
pub use openai_compat::OpenAiCompatibleProvider;
