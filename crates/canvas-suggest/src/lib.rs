//! # canvas-suggest — suggest-движок FR-079 (гибрид lex + Laya)
//!
//! Чистое ядро движка подсказок шаблонов для CanvasDesk. Порт Python-harness
//! PoC Laya (архив `laya-poc-2026-09-27`, harness-коммит `e6e666e`, волна 1;
//! спецификации fusion/lex v2 — отчёты волн 2–3 в `docs/dev-researches/`).
//!
//! Состав (стадии FR-079):
//! - [`types`] — контракты: [`SuggestEngine`](types::SuggestEngine),
//!   [`SuggestContext`](types::SuggestContext), [`ScoredOption`](types::ScoredOption);
//! - [`context`] — сериализация контекста «формат А» (порт `serialize.py`);
//! - [`lex`] / [`lex_v2`] — лексические движки (BM25 + char3 + синонимы);
//! - [`bm25`] — BM25Okapi (порт `rank_bm25`, побитовая совместимость);
//! - [`fusion`] — score-fusion α·norm(lex) + (1−α)·norm(mm) + Platt-калибровка;
//! - [`gate`] — show-гейт «показывать ли подсказку» (детерминированное правило);
//! - [`domain`] — домен-детектор C4 («канвас вне каталога» → не предлагаем).
//!
//! Инварианты:
//! - **Детерминизм**: один вход → один выход; порядок суммирования f64
//!   воспроизводит Python (важно для golden-тестов, см. `bm25::Bm25Okapi`);
//! - **Ноль зависимостей** (std only) — крейт компилируется под
//!   wasm32-unknown-unknown без оговорок (правило FR-079 §«Влияние»);
//! - **Выпиливаемость**: ядро продукта не зависит от этого крейта; удаление
//!   крейта из workspace не ломает сборку (PoC-отчёт §12 «дёшево выпилить»).
//!
//! Числа воспроизведения (замороженные пробы волны 1, n: test 47 / dev 92 /
//! pool 139, основная метрика `n_options ≥ 8` без `below_min_options`):
//! lex v1 p@1 0.3191/0.4130/0.3813; lex v2 test 0.4043; fusion α=0.85
//! test 0.4681; show-гейт 37/39. Гейты CI: см. `tests/golden_*.rs`.

pub mod bm25;
pub mod context;
pub mod domain;
pub mod fusion;
pub mod gate;
pub mod lex;
pub mod lex_v2;
pub mod tokenizer;
pub mod types;

pub use domain::Verdict;
pub use types::{OptionDesc, ScoreSource, ScoredOption, SuggestContext, SuggestEngine};
