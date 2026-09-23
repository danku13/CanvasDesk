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
//!   (для формул — явный аргумент; контракт `hash(content) ⊕
//!   scenario_seed` для M5 — [`seed_from_parts`], P3);
//! - размерностные проверки через `Unit::dims()`/`Unit::scale()`;
//!   ошибки — только существующие варианты `EvalError` (`BadCall`,
//!   `UnitMismatch`);
//! - один сид → побитово одна выборка (независимо от времени прогона) —
//!   контракт для M5/FR-066.
//!
//! Единицы выхода (инвариант 3 FR-015 — образец):
//! - `normal_quantile`/`lognormal_quantile`/`exp_quantile`/
//!   `triangular_quantile` — размерность параметра положения (μ/a; для
//!   `exp_quantile` с Rate-λ — базовая секунда);
//! - `normal_cdf`/`poisson_pmf` — вероятности → `Percent` (доля 0..1, как
//!   `utilization`/`erlang_c` в queueing);
//! - `lognormal_quantile` — параметры в лог-пространстве безразмерны
//!   (лог размерной величины не определён); размерность придаётся
//!   умножением: `lognormal_quantile(0.95, 0.2, 0.5) × 200 ms`.

use super::args::{bad_arity, is_single_dim, percent_unit, time_unit};
use super::{format_num, Dimension, EvalError, Unit, Value};
use statrs::distribution::{ContinuousCDF, Discrete, Normal, Poisson};

/// Канонический список stats-функций — единая точка синхронизации трёх
/// поверхностей: маршрутизация [`super::eval_call`] (guard через
/// [`is_stats_function`]), каталог подсказок `FN_HINTS` (super) и
/// parity-тест (множества обязаны совпадать — паттерн FR-021).
///
/// P2: распределения и квантили; P3 добавит `ci_mean`/`normal_sample`/
/// `lognormal_sample` (порядок записи — по фазам FR-063).
#[cfg(feature = "stats")]
pub(super) const STATS_FUNCTIONS: &[&str] = &[
    "normal_quantile",
    "normal_cdf",
    "lognormal_quantile",
    "exp_quantile",
    "poisson_pmf",
    "triangular_quantile",
    "triangular",
];

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
#[cfg(feature = "stats")]
pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    match func {
        "normal_quantile" => {
            if values.len() != 3 {
                return Err(bad_arity(
                    func,
                    "normal_quantile(p, μ, σ): ровно 3 аргумента",
                ));
            }
            normal_quantile(values)
        }
        "normal_cdf" => {
            if values.len() != 3 {
                return Err(bad_arity(func, "normal_cdf(x, μ, σ): ровно 3 аргумента"));
            }
            normal_cdf(values)
        }
        "lognormal_quantile" => {
            if values.len() != 3 {
                return Err(bad_arity(
                    func,
                    "lognormal_quantile(p, μ, σ): ровно 3 аргумента",
                ));
            }
            lognormal_quantile(values)
        }
        "exp_quantile" => {
            if values.len() != 2 {
                return Err(bad_arity(func, "exp_quantile(p, λ): ровно 2 аргумента"));
            }
            exp_quantile(values)
        }
        "poisson_pmf" => {
            if values.len() != 2 {
                return Err(bad_arity(func, "poisson_pmf(k, λ): ровно 2 аргумента"));
            }
            poisson_pmf(values)
        }
        // FR-063: `triangular` — алиас `triangular_quantile` (в таблице
        // «Изменения» FR функция значится как `triangular`, golden-гейт
        // «Проверка» — `triangular_quantile`; поддержаны оба имени, записи
        // в FN_HINTS синхронны — parity-тест ловит расхождение).
        "triangular_quantile" | "triangular" => {
            if values.len() != 4 {
                return Err(bad_arity(
                    func,
                    "triangular_quantile(p, a, b, c): ровно 4 аргумента (c — мода)",
                ));
            }
            triangular_quantile(func, values)
        }
        _ => Err(EvalError::UnknownFunction(func.to_owned())),
    }
}

// --- P2: распределения и квантили -----------------------------------------

/// `normal_quantile(p, μ, σ)` → размерность μ: квантиль N(μ, σ²) —
/// `μ + σ·Φ⁻¹(p)`. `p` — безразмерный скаляр 0..1; μ и σ — одной
/// размерности (σ > 0); результат — в единице μ (P95 утилизации:
/// `normal_quantile(0.95, utilization, 0.1)` — безразмерный).
fn normal_quantile(values: &[Value]) -> Result<Value, EvalError> {
    let func = "normal_quantile";
    let p = prob_arg(func, "p", &values[0])?;
    let (mu, sigma, unit) = location_scale_args(func, &values[1], &values[2])?;
    if sigma <= 0.0 || sigma.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "σ должен быть положительным, получено {}",
                format_num(sigma)
            ),
        });
    }
    // statrs::inverse_cdf паникует вне [0, 1] — p валидирован выше; края
    // (0/1) дают бесконечные хвосты (support = ℝ) — явно, без statrs.
    let z = if p == 0.0 {
        f64::NEG_INFINITY
    } else if p == 1.0 {
        f64::INFINITY
    } else {
        Normal::standard().inverse_cdf(p)
    };
    let base = mu + sigma * z;
    Ok(Value::with_unit(base / unit.scale(), unit))
}

/// `normal_cdf(x, μ, σ)` → `Percent`: вероятность X ≤ x для N(μ, σ²) —
/// Φ((x−μ)/σ). x, μ, σ — одной размерности (σ > 0); результат — доля 0..1
/// (юнит `%`, как `utilization` в queueing).
fn normal_cdf(values: &[Value]) -> Result<Value, EvalError> {
    let func = "normal_cdf";
    let (mu, sigma, _mu_unit) = location_scale_args(func, &values[1], &values[2])?;
    let x = &values[0];
    if x.dims() != values[1].dims() {
        return Err(EvalError::UnitMismatch {
            lhs: x.to_string(),
            rhs: values[1].to_string(),
        });
    }
    if sigma <= 0.0 || sigma.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "σ должен быть положительным, получено {}",
                format_num(sigma)
            ),
        });
    }
    let z = (x.num * x.unit.scale() - mu) / sigma;
    // Вероятность — доля 0..1 (юнит `%`), независимо от размерности x/μ/σ.
    Ok(Value::with_unit(Normal::standard().cdf(z), percent_unit()))
}

/// `lognormal_quantile(p, μ, σ)` → скаляр: квантиль LogNormal(μ, σ²) —
/// `exp(μ + σ·Φ⁻¹(p))`. Параметры — лог-пространство, БЕЗРАЗМЕРНЫ
/// (строгая проверка: лог размерной величины не определён; размерность
/// придаётся умножением результата на единицу). p ∈ [0, 1]; p=0 → 0
/// (support LogNormal = (0, ∞)), p=1 → +∞.
fn lognormal_quantile(values: &[Value]) -> Result<Value, EvalError> {
    let func = "lognormal_quantile";
    let p = prob_arg(func, "p", &values[0])?;
    let mu = strict_scalar(func, "μ (лог-пространство)", &values[1])?;
    let sigma = strict_scalar(func, "σ (лог-пространство)", &values[2])?;
    if sigma <= 0.0 || sigma.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "σ должен быть положительным, получено {}",
                format_num(sigma)
            ),
        });
    }
    let base = if p == 0.0 {
        0.0
    } else if p == 1.0 {
        f64::INFINITY
    } else {
        (mu + sigma * Normal::standard().inverse_cdf(p)).exp()
    };
    Ok(Value::scalar(base))
}

/// `exp_quantile(p, λ)` → Time (sec) для Rate-λ / скаляр для скалярного λ:
/// квантиль экспоненциального распределения −ln(1−p)/λ. p ∈ [0, 1];
/// λ > 0 (меж-прибытия: `exp_quantile(0.95, 100 rps)` → 0.03 sec).
fn exp_quantile(values: &[Value]) -> Result<Value, EvalError> {
    let func = "exp_quantile";
    let p = prob_arg(func, "p", &values[0])?;
    let lambda = &values[1];
    // λ — скорость (Rate) или безразмерный параметр; результат — базовая
    // секунда для Rate-входа, скаляр для скаляра (пары Rate↔Time — как в
    // littles_law FR-015: база Rate = req/s, база Time = sec).
    let (lam, unit) = if lambda.unit.is_scalar() {
        (lambda.num, Unit::Scalar)
    } else if is_single_dim(lambda, &Dimension::Rate) {
        (lambda.num * lambda.unit.scale(), time_unit())
    } else {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "λ должен быть скоростью (Rate) или скаляром, получено {}",
                lambda
            ),
        });
    };
    if lam <= 0.0 || lam.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("λ должен быть положительным, получено {}", format_num(lam)),
        });
    }
    // p=0 → 0 (support = [0, ∞)), p=1 → +∞; промежуточные — через ln.
    let base = if p == 0.0 {
        0.0
    } else if p == 1.0 {
        f64::INFINITY
    } else {
        -(1.0 - p).ln() / lam
    };
    Ok(Value::with_unit(base / unit.scale(), unit))
}

/// `poisson_pmf(k, λ)` → `Percent`: вероятность ровно `k` событий за
/// интервал для Пуассона(λ) — `e^(−λ)·λ^k/k!`. k — неотрицательное целое
/// (скаляр или Count); λ — конечное ≥ 0 (скаляр или Count). λ = 0 —
/// вырожденный случай честной математикой (statrs требует λ > 0):
/// P(X=0) = 1, P(X=k>0) = 0.
fn poisson_pmf(values: &[Value]) -> Result<Value, EvalError> {
    let func = "poisson_pmf";
    let k = count_arg(func, "k", &values[0])?;
    let lam = count_arg(func, "λ", &values[1])?;
    // k в u64 для statrs::Discrete<u64>: сверху — разумный домен (больше
    // событий за интервал формула не смыслит; as-каст в Rust сатурирует,
    // но домен отсечён явно).
    if k > 1e15 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("k вне домена (≤ 1e15): {}", format_num(k)),
        });
    }
    let prob = if lam == 0.0 {
        if k == 0.0 {
            1.0
        } else {
            0.0
        }
    } else {
        // λ > 0 здесь — конструктор statrs не ошибается; Err мапим без unwrap.
        match Poisson::new(lam) {
            Ok(dist) => dist.pmf(k as u64),
            Err(_) => {
                return Err(EvalError::BadCall {
                    func: func.to_owned(),
                    msg: format!(
                        "λ должен быть положительным и конечным, получено {}",
                        format_num(lam)
                    ),
                })
            }
        }
    };
    Ok(Value::with_unit(prob, percent_unit()))
}

/// `triangular_quantile(p, a, b, c)` (`triangular` — алиас) → размерность
/// a: квантиль треугольного распределения на [a, b] с модой c
/// (кусочно-квадратичная формула). p ∈ [0, 1]; a < b; a ≤ c ≤ b;
/// support ограничен — p=0/p=1 дают конечные a/b.
fn triangular_quantile(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    let p = prob_arg(func, "p", &values[0])?;
    let (a_val, b_val, c_val) = (&values[1], &values[2], &values[3]);
    if a_val.dims() != b_val.dims() || b_val.dims() != c_val.dims() {
        return Err(EvalError::UnitMismatch {
            lhs: a_val.to_string(),
            rhs: if a_val.dims() != b_val.dims() {
                b_val.to_string()
            } else {
                c_val.to_string()
            },
        });
    }
    let a = a_val.num * a_val.unit.scale();
    let b = b_val.num * b_val.unit.scale();
    let c = c_val.num * c_val.unit.scale();
    if a >= b || a.is_nan() || b.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "требуется a < b, получено {} .. {}",
                format_num(a),
                format_num(b)
            ),
        });
    }
    if c < a || c > b || c.is_nan() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "требуется a ≤ c ≤ b (c — мода), получено c = {}",
                format_num(c)
            ),
        });
    }
    let span = b - a;
    let base = if p * span < c - a {
        // Левая ветвь: F(x) = (x−a)²/((b−a)(c−a)) → x = a + √(p·(b−a)·(c−a)).
        a + (p * span * (c - a)).sqrt()
    } else {
        // Правая ветвь: x = b − √((1−p)·(b−a)·(b−c)).
        b - ((1.0 - p) * span * (b - c)).sqrt()
    };
    Ok(Value::with_unit(
        base / a_val.unit.scale(),
        a_val.unit.clone(),
    ))
}

// --- Аргументы и единицы (stats-специфичные; общий набор — expr/args.rs) --

/// Пара «положение/масштаб» одной размерности (μ и σ): числа приводятся
/// к базовым единицам размерности (base = num × unit.scale()), юнит
/// результата — юнит положения (μ). Несовпадение размерностей —
/// `UnitMismatch` (контракт FR-063, ограничение 3/7).
fn location_scale_args(
    func: &str,
    center: &Value,
    scale: &Value,
) -> Result<(f64, f64, Unit), EvalError> {
    let _ = func; // сообщения формируют вызовы; параметр — для симметрии helpers
    if center.dims() != scale.dims() {
        return Err(EvalError::UnitMismatch {
            lhs: center.to_string(),
            rhs: scale.to_string(),
        });
    }
    Ok((
        center.num * center.unit.scale(),
        scale.num * scale.unit.scale(),
        center.unit.clone(),
    ))
}

/// Строго скалярный аргумент (p, сид, параметры лог-пространства): любая
/// размерность — BadCall. В отличие от loose `args::scalar_arg`, семантика
/// параметра требует безразмерности.
fn strict_scalar(func: &str, label: &str, value: &Value) -> Result<f64, EvalError> {
    if !value.unit.is_scalar() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "{label} должен быть безразмерным скаляром, получено {}",
                value
            ),
        });
    }
    Ok(value.num)
}

/// Вероятность/уровень квантиля: строго скаляр, p ∈ [0, 1] (NaN отсечён
/// интервалом — contains(NaN) = false).
fn prob_arg(func: &str, label: &str, value: &Value) -> Result<f64, EvalError> {
    let p = strict_scalar(func, label, value)?;
    if !(0.0..=1.0).contains(&p) {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("{label} вне диапазона 0..1: {}", format_num(p)),
        });
    }
    Ok(p)
}

/// Целочисленный аргумент-счётчик (k Пуассона): скаляр или Count, целое,
/// конечное, ≥ 0 (образец — servers_arg FR-015).
fn count_arg(func: &str, label: &str, value: &Value) -> Result<f64, EvalError> {
    let is_count = value.unit.is_scalar() || is_single_dim(value, &Dimension::Count);
    if !is_count || !value.num.is_finite() || value.num.fract() != 0.0 || value.num < 0.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "{label} должен быть неотрицательным целым (Count или скаляр), получено {}",
                value
            ),
        });
    }
    Ok(value.num)
}

// --- P3: детерминированный RNG и доверительные интервалы -------------------
// (наполняется фазой P3; сид-контракт M5 — seed_from_parts)

// --- Внутренние тесты ------------------------------------------------------

#[cfg(all(test, feature = "stats"))]
mod tests {
    use super::*;

    /// location_scale_args: базовые единицы и юнит результата = юнит μ.
    #[test]
    fn location_scale_converts_to_base_and_keeps_center_unit() {
        let ms = || Unit::atom(super::super::Atom::new(Dimension::Time, 1, 0.001, "ms"));
        let center = Value::with_unit(50.0, ms());
        let scale = Value::with_unit(10.0, ms());
        let (mu, sigma, unit) = location_scale_args("t", &center, &scale).unwrap();
        assert!((mu - 0.05).abs() < 1e-12, "μ в базовой секунде: {mu}");
        assert!((sigma - 0.01).abs() < 1e-12, "σ в базовой секунде: {sigma}");
        assert_eq!(unit, ms(), "юнит результата — юнит μ");
    }

    /// location_scale_args: несовпадение размерностей — UnitMismatch.
    #[test]
    fn location_scale_rejects_dim_mismatch() {
        let rps = Unit::atom(super::super::Atom::new(Dimension::Rate, 1, 1.0, "rps"));
        let err =
            location_scale_args("t", &Value::scalar(1.0), &Value::with_unit(1.0, rps)).unwrap_err();
        assert!(matches!(err, EvalError::UnitMismatch { .. }), "{err:?}");
    }

    /// prob_arg: NaN и вне диапазона — BadCall (интервал отсекает NaN).
    #[test]
    fn prob_arg_rejects_nan_and_out_of_range() {
        let rps = Unit::atom(super::super::Atom::new(Dimension::Rate, 1, 1.0, "rps"));
        for bad in [1.5, -0.1, f64::NAN] {
            let v = if bad.is_nan() {
                Value::scalar(f64::NAN)
            } else {
                Value::scalar(bad)
            };
            assert!(
                matches!(prob_arg("t", "p", &v), Err(EvalError::BadCall { .. })),
                "p = {bad} должен быть отклонён"
            );
        }
        // Размерностный p — тоже BadCall (strict_scalar)
        assert!(matches!(
            prob_arg("t", "p", &Value::with_unit(0.5, rps)),
            Err(EvalError::BadCall { .. })
        ));
        let _ = rps;
    }
}
