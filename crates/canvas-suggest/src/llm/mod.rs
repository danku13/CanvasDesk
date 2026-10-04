//! FR-LLM-C / PRD-0010 F-2 (Stream C): LLM mm-source для suggest-fusion.
//!
//! Параллельный L1-транспорт `l1-llm` (рядом с `l1-laya`): тот же интерфейс
//! choice-ранжирования, но реализованный через `LlmProvider::choice()`
//! (canvas-llm). Используется fusion-воркером как mm-источник вместо Laya
//! sidecar'а, когда `LlmSettings::provider_suggest = Byok`.
//!
//! ## Состав
//!
//! - [`mm_source::LlmMmSource`] — choice-ранжирование через LLM-провайдер:
//!   1. Redact контекста (`redact_context`, Q1 privacy);
//!   2. Cache hit → мгновенный ответ (Q2 prefetch);
//!   3. Cache miss → `LlmProvider::choice()`;
//!   4. Кэширование результата (TTL 5 мин).
//! - [`cache::ContextCache`] — HashMap-кэш по хешу контекста, TTL 5 мин.
//!
//! ## Fusion integration point
//!
//! Сам fusion (`crate::fusion::fuse`) НЕ меняется — он работает с любым
//! mm-источником, который отдаёт `ChoiceAnswer` → `Vec<(String, f64)>`.
//! App создаёт `LlmMmSource` когда `settings.llm.provider_suggest = Byok`,
//! дёргает `choice()` → конвертирует `ChoiceAnswer.probs` в `Vec<(id, prob)>`
//! → кормит fusion (см. `laya/client.rs::MmAnswer` для паттерна).
//!
//! ## Что НЕ реализовано здесь (оставлено на потом / другие потоки)
//!
//! - Custom-node suggest (F-2.12-F-2.16) — отдельный модуль `custom_suggest`
//!   (TODO, вне рамок этой сессии Stream C).
//! - Loading-state skeleton UI — Stream B/D территория (UI).
//! - «Save as template» GitHub issue — TODO.
//! - Benchmark redacted vs raw — TODO (R10, F-2.15).

// FR-LLM-C: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-C:` в комментариях.

pub mod cache;
pub mod mm_source;

pub use cache::ContextCache;
pub use mm_source::LlmMmSource;
