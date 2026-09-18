//! Единая точка монотонного времени web-порта (M8/W1,
//! `docs/plans/wasm-port.md` §6 W1 и §2 п. 7): `std::time::Instant` под
//! wasm32-unknown-unknown компилируется, но паникует в рантайме. На
//! нативных целях `web_time::Instant` — тонкая прозрачная обёртка над
//! std (код подмены не замечает), на wasm32 читает `performance.now()`.
//! Крейты волны (render/scene/app) импортируют alias отсюда, а не из
//! std напрямую.

/// Монотонные «часы» приложения, безопасные под wasm32-unknown-unknown.
pub type Instant = web_time::Instant;
