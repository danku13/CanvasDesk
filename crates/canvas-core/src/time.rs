//! Единая точка времени web-порта (M8/W1, `docs/plans/wasm-port.md`
//! §6 W1 и §2 п. 7): `std::time::Instant` и `std::time::SystemTime` под
//! wasm32-unknown-unknown компилируются, но паникуют в рантайме
//! ("time not implemented on this platform" → wasm-ловушка unreachable,
//! приложение зависает). На нативных целях `web_time` — тонкая
//! прозрачная обёртка над std (код подмены не замечает), на wasm32
//! читает `performance.now()` / `js_sys::Date`. Крейты волны
//! (render/scene/app/suggest) импортируют alias'ы отсюда, а не из std
//! напрямую; исключения допускает только scripts/wasm_time_audit.py
//! (wasm-гейт, ступень 0).

/// Монотонные «часы» приложения, безопасные под wasm32-unknown-unknown.
pub type Instant = web_time::Instant;

/// Календарное время (wall-clock), безопасное под wasm32-unknown-unknown
/// (FR-079 S3-fix: ISO-метки журнала suggest-log, суффиксы id — только
/// через этот alias; на нативе идентичен std).
pub type SystemTime = web_time::SystemTime;

/// Эпоха Unix для [`SystemTime`] — wasm-безопасный аналог
/// `std::time::UNIX_EPOCH`.
pub const UNIX_EPOCH: SystemTime = web_time::UNIX_EPOCH;
