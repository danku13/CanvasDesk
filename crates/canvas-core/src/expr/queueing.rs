//! FR-015: функции теории очередей для доменных расчётов (`mm1`, `mmc`,
//! `utilization`, `littles_law`, `erlang_c`).
//!
//! Чистые функции `(&str, &[Value]) -> Result<Value, EvalError>` — без I/O
//! и состояния (инвариант 1 FR-015); вызываются из [`super::eval_call`]
//! после вычисления аргументов. Математика зафиксирована (инвариант 2):
//! M/M/1 и M/M/c (Erlang-C, рекуррентная формула Erlang-B — устойчива при
//! больших `c`); ссылки: https://en.wikipedia.org/wiki/Erlang_(unit)#Erlang_C_formula ,
//! https://en.wikipedia.org/wiki/Little%27s_law .
//!
//! Единицы выхода явные (инвариант 3 FR-015):
//! - `utilization` → `Percent` (доля 0..1; может быть > 1 при overload);
//! - `mm1`/`mmc` → `Time` — среднее время пребывания заявки в системе
//!   (W = W_q + 1/μ; v1 — минимальный скалярный результат по решению
//!   владельца, структура `{utilization, queue_length, ...}` — v2);
//! - `littles_law` → `Count` (L = λ·W);
//! - `erlang_c` → `Percent` (доля задержанных заявок).
//!
//! Скорости аргументов приводятся к базе размерности Rate (`req/s`, scale
//! 1.0) через `unit.scale`; время результата — в базовой секунде. Перегрузка
//! (ρ ≥ 1) для `mm1`/`mmc` — [`super::EvalError::Overload`] (красная строка
//! диагностики на карточке; очередь аналитически не ограничена).

use super::{Atom, Dimension, EvalError, Unit, Value};

/// Максимум серверов в v1 (рекуррентная Erlang-B устойчива, но arity
/// и числа выше — за пределами домена; решение FR-015 «Открытые вопросы»).
const MAX_SERVERS: usize = 1000;

/// Единая точка входа вызовов FR-015 из [`super::eval_call`]: arity и
/// диспетчеризация. `values` — уже вычисленные аргументы.
pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    match func {
        "utilization" => {
            if !(2..=3).contains(&values.len()) {
                return Err(bad_arity(func, "utilization(λ, μ[, c]): 2 или 3 аргумента"));
            }
            utilization(values)
        }
        "mm1" => {
            if !(2..=3).contains(&values.len()) {
                return Err(bad_arity(func, "mm1(λ, μ[, c]): 2 или 3 аргумента"));
            }
            queue_model(func, values)
        }
        "mmc" => {
            if values.len() != 3 {
                return Err(bad_arity(func, "mmc(λ, μ, c): ровно 3 аргумента"));
            }
            queue_model(func, values)
        }
        "littles_law" => {
            if values.len() != 2 {
                return Err(bad_arity(func, "littles_law(λ, W): ровно 2 аргумента"));
            }
            littles_law(values)
        }
        "erlang_c" => {
            if values.len() != 3 {
                return Err(bad_arity(func, "erlang_c(λ, μ, c): ровно 3 аргумента"));
            }
            erlang_c(values)
        }
        _ => Err(EvalError::UnknownFunction(func.to_owned())),
    }
}

/// `utilization(λ, μ[, c])` → `Percent`: ρ = λ/(c·μ). Без ограничения ρ ≤ 1
/// (перегрузка — валидный ответ для индикатора FR-016).
fn utilization(values: &[Value]) -> Result<Value, EvalError> {
    let lambda = rate_arg("utilization", "λ", &values[0])?;
    let mu = rate_arg("utilization", "μ", &values[1])?;
    let servers = servers_arg("utilization", values.get(2))?;
    if mu == 0.0 {
        return Err(bad_zero_service("utilization"));
    }
    let rho = lambda / (servers as f64 * mu);
    Ok(Value::with_unit(rho, percent_unit()))
}

/// `mm1(λ, μ[, c])` / `mmc(λ, μ, c)` → `Time`: среднее время пребывания
/// заявки в системе W = W_q + 1/μ, где W_q = C(c, a)/(c·μ − λ),
/// a = λ/μ — предложенная нагрузка, C — формула Erlang-C.
/// ρ ≥ 1 → [`super::EvalError::Overload`] (W_q аналитически ∞).
fn queue_model(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    let lambda = rate_arg(func, "λ", &values[0])?;
    let mu = rate_arg(func, "μ", &values[1])?;
    let servers = servers_arg(func, values.get(2).or(Some(&Value::scalar(1.0))))?;
    if lambda < 0.0 || mu < 0.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: "скорости должны быть неотрицательны".to_owned(),
        });
    }
    if mu == 0.0 {
        return Err(bad_zero_service(func));
    }
    if lambda == 0.0 {
        // Пустая система: заявка всё равно тратит 1/μ на обслуживание.
        return Ok(Value::with_unit(1.0 / mu, time_unit()));
    }
    let rho = lambda / (servers as f64 * mu);
    if rho >= 1.0 {
        return Err(EvalError::Overload { rho });
    }
    let offered = lambda / mu;
    let p_wait = erlang_c_probability(offered, servers);
    let wait_queue = p_wait / (servers as f64 * mu - lambda);
    Ok(Value::with_unit(wait_queue + 1.0 / mu, time_unit()))
}

/// `littles_law(λ, W)` → `Count`: L = λ·W (закон Литтла). Скорость — база
/// Rate (`req/s`), время — база Time (`sec`); L — среднее число заявок
/// в системе.
fn littles_law(values: &[Value]) -> Result<Value, EvalError> {
    let lambda = rate_arg("littles_law", "λ", &values[0])?;
    let wait = time_arg("littles_law", "W", &values[1])?;
    if lambda < 0.0 || wait < 0.0 {
        return Err(EvalError::BadCall {
            func: "littles_law".to_owned(),
            msg: "аргументы должны быть неотрицательны".to_owned(),
        });
    }
    Ok(Value::with_unit(lambda * wait, count_unit()))
}

/// `erlang_c(λ, μ, c)` → `Percent`: доля заявок, ожидающих в очереди
/// (P_wait). При ρ ≥ 1 ждут все — `1.0` (не ошибка: это SLA-ответ,
/// перегрузку отдельно показывает `mm1`/`mmc`).
fn erlang_c(values: &[Value]) -> Result<Value, EvalError> {
    let lambda = rate_arg("erlang_c", "λ", &values[0])?;
    let mu = rate_arg("erlang_c", "μ", &values[1])?;
    let servers = servers_arg("erlang_c", values.get(2))?;
    if mu == 0.0 {
        return Err(bad_zero_service("erlang_c"));
    }
    let rho = lambda / (servers as f64 * mu);
    let p_wait = if rho >= 1.0 {
        1.0
    } else {
        erlang_c_probability(lambda / mu, servers)
    };
    Ok(Value::with_unit(p_wait, percent_unit()))
}

// --- Erlang ---

/// Вероятность ожидания в системе M/M/c (формула Erlang-C) через
/// рекуррентную Erlang-B: B(n, a) = a·B(n−1, a)/(n + a·B(n−1, a)), B(0) = 1;
/// C = B / (1 − ρ·(1 − B)). Численно устойчива при любом `c`
/// (без факториалов — решение FR-015 «Открытые вопросы»).
/// Предусловие: ρ < 1.
fn erlang_c_probability(offered_load: f64, servers: usize) -> f64 {
    let mut b = 1.0f64;
    for n in 1..=servers {
        b = offered_load * b / (n as f64 + offered_load * b);
    }
    let rho = offered_load / servers as f64;
    b / (1.0 - rho * (1.0 - b))
}

// --- Аргументы и единицы ---

/// Аргумент-скорость: размерность ровно `Rate¹`; результат — в базе
/// `req/s` (число × масштаб единицы).
fn rate_arg(func: &str, label: &str, value: &Value) -> Result<f64, EvalError> {
    if !is_single_dim(value, &Dimension::Rate) {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("{label} должен быть скоростью (Rate), получено {}", value),
        });
    }
    // unit.scale() — приватный метод expr-модуля; queueing — потомок,
    // доступ разрешён правилами видимости Rust.
    Ok(value.num * value.unit.scale())
}

/// Аргумент-время: размерность ровно `Time¹`; результат — в базе `sec`.
fn time_arg(func: &str, label: &str, value: &Value) -> Result<f64, EvalError> {
    if !is_single_dim(value, &Dimension::Time) {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("{label} должен быть временем (Time), получено {}", value),
        });
    }
    Ok(value.num * value.unit.scale())
}

/// Аргумент-серверы: скаляр или `Count`, целое 1..=MAX_SERVERS.
fn servers_arg(func: &str, value: Option<&Value>) -> Result<usize, EvalError> {
    let one_server = Value::scalar(1.0);
    let value = value.unwrap_or(&one_server);
    let is_count = is_single_dim(value, &Dimension::Count) || value.unit.is_scalar();
    if !is_count || value.num.fract() != 0.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("c должен быть целым числом серверов, получено {}", value),
        });
    }
    if !(1.0..=MAX_SERVERS as f64).contains(&value.num) {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("c вне диапазона 1..={MAX_SERVERS}: {}", value.num),
        });
    }
    Ok(value.num as usize)
}

/// Размерность значения — ровно одна, указанная, со степенью 1.
fn is_single_dim(value: &Value, dim: &Dimension) -> bool {
    let dims = value.dims();
    dims.len() == 1 && dims.get(dim) == Some(&1)
}

fn percent_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Percent, 1, 1.0, "%"))
}

fn time_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Time, 1, 1.0, "sec"))
}

fn count_unit() -> Unit {
    Unit::atom(Atom::new(Dimension::Count, 1, 1.0, "req"))
}

fn bad_arity(func: &str, usage: &str) -> EvalError {
    EvalError::BadCall {
        func: func.to_owned(),
        msg: usage.to_owned(),
    }
}

fn bad_zero_service(func: &str) -> EvalError {
    EvalError::BadCall {
        func: func.to_owned(),
        msg: "нулевая скорость обслуживания".to_owned(),
    }
}
