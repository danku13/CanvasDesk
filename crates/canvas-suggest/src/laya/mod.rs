//! L1-транспорт: клиент `/v1/systemone` протокола System One (Jev-совместим).
//!
//! Порт `client.py` PoC (волна 1). Ключевое правило: **явная `model` в каждом
//! запросе** — авто-роутинг Laya лениво грузит второй чекпойнт поверх
//! первого (RAM-ловушка, найдена в Э1). Один чекпойнт на инстанс.
//!
//! Транспорт — только localhost (ureq без TLS): сетевых вызовов наружу
//! движок не делает; sidecar — отдельный процесс (правило AGENTS.md).
//!
//! Деградация (FR-079 §3): ошибка/таймаут транспорта → `Err(LayaError)`,
//! вызывающая сторона (воркер) подставляет пустой mm-лист, и fusion
//! вырождается в lex: `fusion(lex, ∅) = lex`.

pub mod client;
pub mod manifest;
pub mod sidecar;

pub use client::{LayaClient, LayaError, MmAnswer};
pub use manifest::{run_smoke, sha256_file, verify_pins, SmokeFixture};
pub use sidecar::{Sidecar, SidecarConfig};

/// Инструкции choice-вопроса (порт `client.py: INSTR`).
pub const SUGGEST_INSTRUCTIONS: &str = "Which template best fits the node being edited?";
