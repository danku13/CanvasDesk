//! FR-015: функции теории очередей для доменных расчётов (`mm1`, `mmc`,
//! `utilization`, `littles_law`, `erlang_c`) + FR-027 расширение: 4
//! финансовые функции (`npv`, `cagr`, `irr`, `cohort_ltv`).
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
//! - FR-027: `npv` → скаляр (серия потоков дисконтируется по ставке);
//!   `cagr` → скаляр (доля роста за период);
//!   `irr` → скаляр (внутренняя норма доходности, Newton-Raphson);
//!   `cohort_ltv` → скаляр (интеграл retention-кривой × arpu × margin).
//!
//! Скорости аргументов приводятся к базе размерности Rate (`req/s`, scale
//! 1.0) через `unit.scale`; время результата — в базовой секунде. Перегрузка
//! (ρ ≥ 1) для `mm1`/`mmc` — [`super::EvalError::Overload`] (красная строка
//! диагностики на карточке; очередь аналитически не ограничена).

use super::args::{bad_arity, count_unit, is_single_dim, percent_unit, scalar_arg, time_unit};
use super::{Dimension, EvalError, Unit, Value};

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
        // FR-027: 4 финансовые функции (расширение FR-015).
        "npv" => {
            // npv(rate, *cf): первый аргумент — ставка, дальше ≥ 1 поток.
            if values.len() < 2 {
                return Err(bad_arity(
                    func,
                    "npv(rate, *cf): ставка и ≥ 1 денежный поток",
                ));
            }
            npv(values)
        }
        "cagr" => {
            if values.len() != 3 {
                return Err(bad_arity(
                    func,
                    "cagr(begin, end, periods): ровно 3 аргумента",
                ));
            }
            cagr(values)
        }
        "irr" => {
            // irr(*cf): ≥ 2 потока (иначе IRR не определён).
            if values.len() < 2 {
                return Err(bad_arity(func, "irr(*cf): ≥ 2 денежных потока"));
            }
            irr(values)
        }
        "cohort_ltv" => {
            if values.len() != 6 {
                return Err(bad_arity(
                    func,
                    "cohort_ltv(arpu_m0, margin, r_d1, r_d7, r_d30, months): 6 аргументов",
                ));
            }
            cohort_ltv(values)
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

fn bad_zero_service(func: &str) -> EvalError {
    EvalError::BadCall {
        func: func.to_owned(),
        msg: "нулевая скорость обслуживания".to_owned(),
    }
}

// --- FR-027: финансовые функции (расширение FR-015) ---

/// `npv(rate, *cf)` → скаляр (USD/исходная единица потоков): чистая
/// приведённая стоимость серии денежных потоков. `rate` — ставка
/// дисконтирования (скаляр, доля 0..1 или > 1 при гиперинфляции).
/// `cf[0]` дисконтируется при t=0 (cf₀/(1+r)⁰ = cf₀), cf[1] — при t=1 и т.д.
/// Результат — в размерности первого потока (валюта — Money).
/// https://en.wikipedia.org/wiki/Net_present_value
fn npv(values: &[Value]) -> Result<Value, EvalError> {
    let rate = scalar_arg("npv", &values[0])?;
    let unit = if values[1].unit.is_scalar() {
        Unit::Scalar
    } else {
        values[1].unit.clone()
    };
    let mut total = 0.0f64;
    for (t, v) in values[1..].iter().enumerate() {
        let cf = scalar_arg("npv", v)?;
        let factor = (1.0 + rate).powi(t as i32);
        total += cf / factor;
    }
    Ok(Value { num: total, unit })
}

/// `cagr(begin, end, periods)` → скаляр (доля, не %): среднегодовой темп
/// роста. `(end/begin)^(1/periods) − 1`. Если begin ≤ 0 — ошибка
/// (отрицательный старт бессмысленен для логарифмической модели).
/// https://en.wikipedia.org/wiki/Compound_annual_growth_rate
fn cagr(values: &[Value]) -> Result<Value, EvalError> {
    let begin = scalar_arg("cagr", &values[0])?;
    let end = scalar_arg("cagr", &values[1])?;
    let periods = scalar_arg("cagr", &values[2])?;
    if begin <= 0.0 {
        return Err(EvalError::BadCall {
            func: "cagr".to_owned(),
            msg: format!("begin должен быть положительным, получено {begin}"),
        });
    }
    if periods <= 0.0 {
        return Err(EvalError::BadCall {
            func: "cagr".to_owned(),
            msg: format!("periods должен быть положительным, получено {periods}"),
        });
    }
    let cagr_value = (end / begin).powf(1.0 / periods) - 1.0;
    Ok(Value::scalar(cagr_value))
}

/// `irr(*cf)` → скаляр (доля): внутренняя норма доходности — ставка r,
/// при которой NPV = 0. Newton-Raphson с начальной догадкой r₀ = 0.1
/// (10%), шаг ±0.05, лимит 100 итераций. Если не сошёлся — `BadCall`.
/// Требует, чтобы потоки имели разный знак (иначе нет корня).
/// https://en.wikipedia.org/wiki/Internal_rate_of_return
fn irr(values: &[Value]) -> Result<Value, EvalError> {
    let cashflows: Vec<f64> = values
        .iter()
        .map(|v| scalar_arg("irr", v))
        .collect::<Result<Vec<_>, _>>()?;
    // Без смены знака IRR не определён (нет корня NPV = 0).
    let has_positive = cashflows.iter().any(|&v| v > 0.0);
    let has_negative = cashflows.iter().any(|&v| v < 0.0);
    if !has_positive || !has_negative {
        return Err(EvalError::BadCall {
            func: "irr".to_owned(),
            msg: "потоки должны иметь разный знак (иначе IRR не определён)".to_owned(),
        });
    }
    // Newton-Raphson: r_{n+1} = r_n − npv(r_n) / npv'(r_n),
    // npv'(r) = Σ −t · cf_t / (1+r)^(t+1).
    let mut r = 0.1f64; // начальная догадка 10%
    const MAX_ITERS: usize = 100;
    const TOLERANCE: f64 = 1e-7;
    for _ in 0..MAX_ITERS {
        let one_plus_r = 1.0 + r;
        if one_plus_r.abs() < 1e-12 {
            break;
        }
        let mut npv_value = 0.0f64;
        let mut d_npv = 0.0f64;
        for (t, &cf) in cashflows.iter().enumerate() {
            let t_f = t as f64;
            let discount = one_plus_r.powi(t as i32);
            npv_value += cf / discount;
            d_npv += -t_f * cf / (one_plus_r * discount);
        }
        if npv_value.abs() < TOLERANCE {
            return Ok(Value::scalar(r));
        }
        if d_npv.abs() < 1e-12 {
            break; // производная 0 — невозможно продолжить
        }
        let next_r = r - npv_value / d_npv;
        // Ограничение шага, чтобы не уходить в ±бесконечность.
        let delta = (next_r - r).clamp(-0.5, 0.5);
        if delta == 0.0 {
            break;
        }
        r += delta;
        // Защита от r ≤ −1 (лог бессмысленен).
        if r <= -0.99 {
            r = -0.99;
        }
    }
    Err(EvalError::BadCall {
        func: "irr".to_owned(),
        msg: "Newton-Raphson не сошёлся за 100 итераций — проверьте знаки потоков".to_owned(),
    })
}

/// `cohort_ltv(arpu_m0, margin, r_d1, r_d7, r_d30, months)` → скаляр
/// (USD): LTV когорты через интеграл retention-кривой. retention_t —
/// линейная интерполяция между точками d0=1, d1=r_d1, d7=r_d7, d30=r_d30
/// (предполагается экспоненциальное затухание после d30).
/// `arpu_m0 × margin × Σ_{t=0}^{months} retention_t`.
fn cohort_ltv(values: &[Value]) -> Result<Value, EvalError> {
    let arpu_m0 = scalar_arg("cohort_ltv", &values[0])?;
    let margin = scalar_arg("cohort_ltv", &values[1])?;
    let r_d1 = scalar_arg("cohort_ltv", &values[2])?;
    let r_d7 = scalar_arg("cohort_ltv", &values[3])?;
    let r_d30 = scalar_arg("cohort_ltv", &values[4])?;
    let months = scalar_arg("cohort_ltv", &values[5])?;
    if margin < 0.0 || margin > 1.0 {
        return Err(EvalError::BadCall {
            func: "cohort_ltv".to_owned(),
            msg: format!("margin должен быть 0..1, получено {margin}"),
        });
    }
    if months < 0.0 || months.fract() != 0.0 {
        return Err(EvalError::BadCall {
            func: "cohort_ltv".to_owned(),
            msg: format!("months должен быть неотрицательным целым, получено {months}"),
        });
    }
    let months = months as usize;
    // Опорные точки retention (t, retention).
    let anchors: [(f64, f64); 4] = [(0.0, 1.0), (1.0, r_d1), (7.0, r_d7), (30.0, r_d30)];
    // Линейная интерполяция между точками; после d30 — экспоненциальное
    // затухание (r_d30^(extra_days/30)).
    let retention_at = |t: f64| -> f64 {
        if t <= 0.0 {
            return 1.0;
        }
        if t <= 30.0 {
            // Найти отрезок [anchors[i].0, anchors[i+1].0] с t внутри.
            for window in anchors.windows(2) {
                let (t0, r0) = window[0];
                let (t1, r1) = window[1];
                if t >= t0 && t <= t1 && t1 > t0 {
                    return r0 + (r1 - r0) * (t - t0) / (t1 - t0);
                }
            }
            // t == 30 — последний anchor
            return r_d30;
        }
        // t > 30: экспоненциальное затухание от r_d30.
        r_d30.powf(t / 30.0)
    };
    // Сумма retention за `months` месяцев (по дням, 30 дней на месяц).
    let total_days = months * 30;
    let mut retention_sum = 0.0f64;
    for d in 0..=total_days {
        retention_sum += retention_at(d as f64);
    }
    // Перевод в месяцы: сумма retention по дням / 30.
    let ltv = arpu_m0 * margin * retention_sum / 30.0;
    let unit = if values[0].unit.is_scalar() {
        Unit::Scalar
    } else {
        values[0].unit.clone()
    };
    Ok(Value { num: ltv, unit })
}
