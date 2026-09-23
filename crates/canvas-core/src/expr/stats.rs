//! FR-063: доменный слой статистики — слой L2 расчётного стека (ADR-0008,
//! этап M2/S1). Чистые функции распределений, квантилей и доверительных
//! интервалов за единым диспетчером [`dispatch`] — по образцу
//! `queueing::dispatch` (FR-015); детерминированный RNG
//! (`rand_chacha::ChaCha8Rng::seed_from_u64`) — для выборок Monte Carlo
//! (M5, FR-066) и воспроизводимых what-if-экспериментов (FR-017).
//!
//! Модуль и его arm в [`super::eval_call`] живут за фичей `stats`
//! (`crates/canvas-core/Cargo.toml`, deps — волна S0); без фичи модуль не
//! компилируется и функции не регистрируются — формулы с ними получают
//! `UnknownFunction` (graceful-деградация, zero-dep инвариант B2B-сборки).
//!
//! Контракты (FR-063 «Ограничения для агента-реализатора»):
//! - чистые функции `(&[Value]) -> Result<Value, EvalError>` — без `&mut
//!   Env`, без доступа к `Canvas`/`SceneState` (статистика живёт в L2, как
//!   и queueing);
//! - `thread_rng()` запрещён (архдок §5.6.2): единственный источник
//!   случайности — `ChaCha8Rng::seed_from_u64(u64)`, сид детерминирован
//!   (для формул — явный аргумент, контракт `hash(content) ⊕
//!   scenario_seed` для M5 — см. P3);
//! - размерностные проверки через `Unit::dims()`/`Unit::scale()`;
//!   ошибки — только существующие варианты `EvalError` (`BadCall`,
//!   `UnitMismatch`);
//! - один сид → побитово одна выборка (независимо от времени прогона) —
//!   контракт для M5/FR-066.

use super::{EvalError, Value};

/// Канонический список stats-функций — единая точка синхронизации трёх
/// поверхностей: маршрутизация [`super::eval_call`] (guard через
/// [`is_stats_function`]), каталог подсказок `FN_HINTS` (super) и
/// parity-тест (множества обязаны совпадать — паттерн FR-021).
///
/// P1 skeleton: множество пусто — наполнение в P2 (распределения и
/// квантили) и P3 (RNG + доверительные интервалы).
#[cfg(feature = "stats")]
pub(super) const STATS_FUNCTIONS: &[&str] = &[];

/// Маршрутизатор arm'а `eval_call`: имя принадлежит stats-домену.
/// Линейный поиск по короткому списку — вызов только для имён, не
/// совпавших со встроенными и queueing-arms.
#[cfg(feature = "stats")]
pub(super) fn is_stats_function(name: &str) -> bool {
    STATS_FUNCTIONS.contains(&name)
}

/// Единая точка входа stats-вызовов из [`super::eval_call`] (образец —
/// `queueing::dispatch`, FR-015): arity-проверка → вызов чистой функции.
/// `values` — уже вычисленные аргументы.
///
/// P1 skeleton: список функций пуст, любой вызов деградирует до
/// `UnknownFunction` — то же поведение, что и без фичи.
#[cfg(feature = "stats")]
pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    let _ = values;
    let _ = func;
    Err(EvalError::UnknownFunction(func.to_owned()))
}
