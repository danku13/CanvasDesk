//! # canvas-llm — LLM-слой CanvasDesk (FR-LLM-A / ADR-0016).
//!
//! Унифицированный `LlmProvider` trait для 6 провайдеров (BYOK + Sign-in-with-
//! ChatGPT). Feature-gated: дефолтная сборка без сети (ADR-0011 wasm-гейт,
//! AGENTS.md "no network in the host"). Сетевые адаптеры (`OpenAiCompatibleProvider`,
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
//! - [`chatgpt_oauth`] — Sign-in-with-ChatGPT: OAuth-клиент (PKCE, localhost
//!   listener/deep-link, token exchange/refresh), `TokenStore`, `id_token`
//!   валидация и `ChatGptOAuthProvider` (Responses API) — feature `l1-llm`
//!   (FR-LLM-OAUTH, PRD-0010 F-5.1…F-5.7).
//!
//! ## Stream D
//!
//! - `crates/canvas-llm/src/chatgpt_oauth/` — OAuth-клиент (PKCE, JWKS,
//!   refresh, Responses API). Stream D владеет этой директорией.
//!   Статус: **реализовано** (FR-LLM-OAUTH, Stream D 2-a): PKCE S256
//!   (RFC 7636, Appendix B-вектор), desktop localhost-listener + web
//!   deep-link `canvasdesk://oauth/callback` (F-5.2), token exchange /
//!   refresh (F-5.7), `TokenStore` (F-5.3 — keychain/OPFS поверх трейта),
//!   `id_token` iss/aud/nonce/exp (F-5.4; подпись RS256 — v1 no-op стаб,
//!   см. SECURITY-блок `chatgpt_oauth/jwt.rs`), Responses API `/v1/responses`
//!   (F-5.6), discovery `/v1/models` (F-5.5), proxy_url для wasm (F-5.10).
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

/// Health-check helper + HealthReport (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod health;

/// W1 (wave-1): транспортная абстракция HttpTransport (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod transport;

/// W1 (wave-1): discovery моделей `GET /v1/models` + TTL-кэш (feature `l1-llm`).
#[cfg(feature = "l1-llm")]
pub mod discovery;

/// OpenAI-compatible адаптер для 5 провайдеров (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod openai_compat;

/// Anthropic Claude адаптер (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod anthropic;

/// FR-LLM-OAUTH (Stream D): Sign-in-with-ChatGPT — OAuth-клиент, TokenStore,
/// id_token-валидация и ChatGptOAuthProvider (feature `l1-llm` only).
#[cfg(feature = "l1-llm")]
pub mod chatgpt_oauth;

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

// W1 (wave-1): транспорт + discovery — публичный API для W2/W3.
#[cfg(feature = "l1-llm")]
pub use discovery::{DiscoveredModel, ModelCache};
#[cfg(all(feature = "wasm-fetch", target_arch = "wasm32"))]
pub use transport::WasmFetchTransport;
#[cfg(feature = "l1-llm")]
pub use transport::{
    HttpMethod, HttpRequest, HttpResponse, HttpTransport, MockTransport, UreqTransport,
};

// FR-LLM-OAUTH (Stream D): re-export рядом с OpenAiCompatibleProvider.
// pkce публичных типов не имеет (только функции) — доступ через
// `chatgpt_oauth::pkce`.
#[cfg(feature = "l1-llm")]
pub use chatgpt_oauth::{
    CallbackParams, ChatGptOAuthProvider, IdClaims, JwksKey, LoginSession, MemoryTokenStore,
    NoopVerifier, OAuthClient, OAuthTokens, SignatureVerifier, TokenStore,
};
