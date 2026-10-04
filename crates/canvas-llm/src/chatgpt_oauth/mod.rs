//! FR-LLM-OAUTH / PRD-0010 F-5.1–F-5.7 — Sign-in-with-ChatGPT.
//!
//! Полный OAuth-клиент + `LlmProvider`-реализация поверх Responses API
//! (Stream D владеет этой директорией; см. «Состав» в док-комментарии
//! `lib.rs`). Суб-модули:
//!
//! - [`pkce`] — PKCE RFC 7636 (verifier/challenge S256, state, nonce);
//! - [`auth`] — OAuth-клиент: login URL, localhost-listener (desktop),
//!   token exchange/refresh, deep-link для web (`DEEP_LINK_REDIRECT`);
//! - [`tokens`] — `OAuthTokens` + трейт `TokenStore` (keychain/OPFS в
//!   продукте, memory — тесты);
//! - [`jwt`] — минимальный разбор `id_token` (iss/aud/nonce/exp) +
//!   JWKS-структуры и трейт `SignatureVerifier` (v1 — no-op, SECURITY-стаб);
//! - [`provider`] — `ChatGptOAuthProvider` (Responses API, auto-refresh,
//!   choice через forced tool `rank`, embed → `NotSupported`).
//!
//! Feature-гейт: весь модуль компилируется только за `l1-llm` (ADR-0011:
//! дефолтная сборка без сети). Desktop-only куски (TCP-листенер,
//! `std::process::Command`) — за `cfg(not(target_arch = "wasm32"))`; под
//! wasm32-unknown-unknown компилируются только чистые построители и парсеры
//! (транспорт — cloud-proxy через canvas-web, PRD-0010 F-5.10).
//!
//! Ограничения (дизайн-док §4.7): только OpenAI-модели, embeddings не входят
//! в подписку, rate limits подписки, API в preview.

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

pub mod auth;
pub mod b64;
pub mod jwt;
pub mod pkce;
pub mod provider;
pub mod tokens;

// Re-export публичного API модуля (lib.rs дополнительно поднимает наверх
// основное — см. док-комментарий lib.rs).
pub use auth::{CallbackParams, LoginSession, OAuthClient};
pub use jwt::{IdClaims, JwksKey, JwtHeader, NoopVerifier, SignatureVerifier};
pub use provider::ChatGptOAuthProvider;
pub use tokens::{MemoryTokenStore, OAuthTokens, TokenStore};
