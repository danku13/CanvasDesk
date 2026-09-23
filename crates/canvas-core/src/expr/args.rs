//! FR-063 (Открытый вопрос № 1, вариант (a)): общий модуль разбора
//! аргументов для доменных слоёв L2 расчётного стека — `queueing` (FR-015)
//! и `stats` (FR-063). Хелперы вынесены из `queueing.rs` дословно: публичный
//! API и поведение queueing не меняются (контракт FR-063 «Границы
//! изменений»), дубли устранены, будущие домены (FR-066 `mc`) берут тот же
//! набор отсюда.
//!
//! Видимость — `pub(super)`: хелперы приватны для eval-модуля (`expr`) и
//! его потомков, наружу крейта не торчат.

use super::{Atom, Dimension, EvalError, Unit, Value};

/// Размерность значения — ровно одна, указанная, со степенью 1.
pub(super) fn is_single_dim(value: &Value, dim: &Dimension) -> bool {
    let dims = value.dims();
    dims.len() == 1 && dims.get(dim) == Some(&1)
}

/// Скалярный аргумент: безразмерное значение (Rate/Count/Money/Percent
/// приведутся к f64 через `unit.scale()`; скаляр — как есть). Любая
/// размерность принимается: для финансовых формул единицы — только
/// семантика (USD для денег, доли для процентов).
pub(super) fn scalar_arg(func: &str, value: &Value) -> Result<f64, EvalError> {
    let _ = func; // для диагностики через panic-сообщение в вызывающем коде
    if value.unit.is_scalar() {
        return Ok(value.num);
    }
    // Не-скаляр приводим к числу с учётом масштаба единицы (для Money
    // масштаб = 1.0 для $ и usd, поэтому результат — просто value.num).
    Ok(value.num * value.unit.scale())
}

pub(super) fn percent_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Percent, 1, 1.0, "%"))
}

pub(super) fn time_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Time, 1, 1.0, "sec"))
}

pub(super) fn count_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Count, 1, 1.0, "req"))
}

pub(super) fn bad_arity(func: &str, usage: &str) -> EvalError {
    EvalError::BadCall {
        func: func.to_owned(),
        msg: usage.to_owned(),
    }
}
