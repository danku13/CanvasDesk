//! Доменный слой A/B тестирования — по образцу `queueing` (FR-015) и
//! `stats` (FR-063): чистые функции `(&[Value]) -> Result<Value, EvalError>`
//! за единым диспетчером [`dispatch`], без I/O и состояния.
//!
//! Отличие от `stats` (FR-063): модуль НЕ за фичей — как `queueing`.
//! Обоснование: built-in шаблон `com.canvasdesk.ab-test` и схема
//! `com.canvasdesk.scheme.ab-testing` вызывают эти функции, а каталог
//! built-in обязан работать во ВСЕХ сборках (zero-dep инвариант B2B,
//! `cargo build --no-default-features`). Необходимая математика (Φ, Φ⁻¹)
//! реализована чисто, без statrs — точность ниже (`erfc`-ряд + цепная
//! дробь, ~1e-15; Φ⁻¹ — аппроксимация Acklam + шаги Halley), для доменных
//! решений A/B (n на группу, p-value vs α) более чем достаточна.
//!
//! Статистика (контракты):
//! - `ab_zscore`/`ab_pvalue` — pooled z-тест двух пропорций (стандартный
//!   двухвыборочный z-тест с объединённой дисперсией, two-sided):
//!   z = (p_b − p_a) / √(p̄(1−p̄)(1/n_a + 1/n_b)), p̄ = (c_a+c_b)/(n_a+n_b);
//!   p-value = 2·(1−Φ(|z|)) = erfc(|z|/√2) — точное тождество, без
//!   катастрофической потери точности в хвосте;
//! - `ab_sample_size` — размер выборки на группу для двухстороннего
//!   pooled z-теста (формула Флейса / канонический вид Evan Miller):
//!   n = ⌈(z_{1−α/2}·√(2·p̄(1−p̄)) + z_β·√(p_a(1−p_a)+p_b(1−p_b)))²/δ²⌉,
//!   p_b = p_a·(1+mde) (mde — ОТНОСИТЕЛЬный эффект), p̄ = (p_a+p_b)/2;
//! - `ab_lift` — относительный прирост (p_b − p_a)/p_a;
//! - `ab_ci` — полуширина нормального ДИ доли: z_{1−(1−conf)/2}·√(pq/n);
//!   границы решает формула потребителя: `p ± ab_ci(...)` (образец —
//!   `ci_mean` FR-063).
//!
//! Единицы выхода (инвариант 3 FR-015):
//! - `ab_zscore`/`ab_sample_size` — скаляры (z безразмерен, n — число);
//! - `ab_pvalue`/`ab_lift`/`ab_ci` — доли 0..1 с юнитом `%` (как
//!   `utilization`/`erlang_c` в queueing, `normal_cdf` в stats).
//!
//! Входы: конверсии/визиты — целые (Count или скаляр); вероятности
//! (p0, mde, alpha, power, conf) — строго безразмерные скаляры 0..1
//! (семантика параметров — образец `strict_scalar` FR-063). Конверсия
//! не может превышать визиты (домен: доля ≤ 1).

use super::args::{bad_arity, is_single_dim, percent_unit};
use super::{Dimension, EvalError, Value};

/// Канонический список abtest-функций — единая точка синхронизации трёх
/// поверхностей: маршрутизация [`super::eval_call`] (guard через
/// [`is_abtest_function`]), каталог подсказок `FN_HINTS` (super) и
/// parity-тест (множества обязаны совпадать — паттерн FR-021).
pub(super) const ABTEST_FUNCTIONS: &[&str] = &[
    "ab_sample_size",
    "ab_zscore",
    "ab_pvalue",
    "ab_lift",
    "ab_ci",
];

/// Маршрутизатор arm'а `eval_call`: имя принадлежит abtest-домену.
pub(super) fn is_abtest_function(name: &str) -> bool {
    ABTEST_FUNCTIONS.contains(&name)
}

/// Единая точка входа abtest-вызовов из [`super::eval_call`] (образец —
/// `queueing::dispatch` FR-015): arity-проверка → вызов чистой функции.
pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError> {
    match func {
        "ab_sample_size" => {
            if values.len() != 4 {
                return Err(bad_arity(
                    func,
                    "ab_sample_size(p0, mde, alpha, power): ровно 4 аргумента",
                ));
            }
            ab_sample_size(values)
        }
        "ab_zscore" => {
            if values.len() != 4 {
                return Err(bad_arity(
                    func,
                    "ab_zscore(conv_a, n_a, conv_b, n_b): ровно 4 аргумента",
                ));
            }
            ab_zscore(values)
        }
        "ab_pvalue" => {
            if values.len() != 4 {
                return Err(bad_arity(
                    func,
                    "ab_pvalue(conv_a, n_a, conv_b, n_b): ровно 4 аргумента",
                ));
            }
            ab_pvalue(values)
        }
        "ab_lift" => {
            if values.len() != 4 {
                return Err(bad_arity(
                    func,
                    "ab_lift(conv_a, n_a, conv_b, n_b): ровно 4 аргумента",
                ));
            }
            ab_lift(values)
        }
        "ab_ci" => {
            if values.len() != 3 {
                return Err(bad_arity(func, "ab_ci(conv, n, conf): ровно 3 аргумента"));
            }
            ab_ci(values)
        }
        _ => Err(EvalError::UnknownFunction(func.to_owned())),
    }
}

// --- Доменные функции --------------------------------------------------------

/// `ab_sample_size(p0, mde, alpha, power)` → скаляр: минимальная выборка
/// НА ГРУППУ для двухстороннего pooled z-теста двух пропорций.
/// p0 — базовая конверсия (0..1); mde — ОТНОСИТЕЛЬНЫЙ минимально
/// обнаруживаемый эффект (0.10 = +10%: p_b = p0·1.10); alpha — уровень
/// значимости (0.05); power — мощность (0.80). Формула Флейса (pooled):
/// n = ⌈(z_{1−α/2}·√(2·p̄q̄) + z_{power}·√(p_aq_a+p_bq_b))²/(p_b−p_a)²⌉.
fn ab_sample_size(values: &[Value]) -> Result<Value, EvalError> {
    let func = "ab_sample_size";
    let p0 = prob_arg(func, "p0", &values[0])?;
    let mde = strict_scalar(func, "mde", &values[1])?;
    let alpha = prob_arg(func, "alpha", &values[2])?;
    let power = prob_arg(func, "power", &values[3])?;
    // p0 на границах (0/1) — вырожденный тест (p_b(1−p_b) = 0): домен открыт.
    if p0 <= 0.0 || p0 >= 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("p0 должен быть строго в 0..1, получено {}", format_num(p0)),
        });
    }
    if !mde.is_finite() || mde == 0.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "mde должен быть ненулевым конечным, получено {}",
                format_num(mde)
            ),
        });
    }
    if alpha <= 0.0 || alpha >= 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "alpha должен быть строго в 0..1, получено {}",
                format_num(alpha)
            ),
        });
    }
    if power <= 0.0 || power >= 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "power должен быть строго в 0..1, получено {}",
                format_num(power)
            ),
        });
    }
    let p1 = p0 * (1.0 + mde);
    if p1 <= 0.0 || p1 >= 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "p_b = p0·(1+mde) = {} вне 0..1 — уменьшите |mde|",
                format_num(p1)
            ),
        });
    }
    let z_alpha = norm_quantile(1.0 - alpha / 2.0);
    let z_power = norm_quantile(power);
    let p_bar = (p0 + p1) / 2.0;
    let numerator = z_alpha * (2.0 * p_bar * (1.0 - p_bar)).sqrt()
        + z_power * (p0 * (1.0 - p0) + p1 * (1.0 - p1)).sqrt();
    let delta = (p1 - p0).abs();
    let n = (numerator / delta).powi(2).ceil();
    if !n.is_finite() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: "размер выборки не вычислим (вырожденные параметры)".to_owned(),
        });
    }
    Ok(Value::scalar(n))
}

/// `ab_zscore(conv_a, n_a, conv_b, n_b)` → скаляр: pooled z-статистика
/// двух пропорций (two-sided). Знак — направление эффекта (положительный
/// z: вариант b лучше контроля a).
fn ab_zscore(values: &[Value]) -> Result<Value, EvalError> {
    let (p_a, p_b, pooled) = pooled_props("ab_zscore", values)?;
    let se = (pooled * (1.0 - pooled) * (1.0 / n_of(values, 1) + 1.0 / n_of(values, 3))).sqrt();
    Ok(Value::scalar((p_b - p_a) / se))
}

/// `ab_pvalue(conv_a, n_a, conv_b, n_b)` → `Percent`: двусторонний
/// p-value pooled z-теста — `erfc(|z|/√2)` (точное тождество для
/// 2·(1−Φ(|z|)), устойчивое в хвосте). Значение — доля 0..1 (юнит `%`).
fn ab_pvalue(values: &[Value]) -> Result<Value, EvalError> {
    let (p_a, p_b, pooled) = pooled_props("ab_pvalue", values)?;
    let se = (pooled * (1.0 - pooled) * (1.0 / n_of(values, 1) + 1.0 / n_of(values, 3))).sqrt();
    let z = (p_b - p_a) / se;
    Ok(Value::with_unit(erfc(z.abs() / SQRT_2), percent_unit()))
}

/// `ab_lift(conv_a, n_a, conv_b, n_b)` → `Percent`: относительный прирост
/// конверсии (p_b − p_a)/p_a (0.3 = +30%). p_a = 0 — BadCall (деление).
fn ab_lift(values: &[Value]) -> Result<Value, EvalError> {
    let (p_a, p_b, _pooled) = pooled_props("ab_lift", values)?;
    if p_a == 0.0 {
        return Err(EvalError::BadCall {
            func: "ab_lift".to_owned(),
            msg: "p_a = 0: относительный лифт не определён (нет базовой конверсии)".to_owned(),
        });
    }
    Ok(Value::with_unit((p_b - p_a) / p_a, percent_unit()))
}

/// `ab_ci(conv, n, conf)` → `Percent`: полуширина нормального
/// доверительного интервала доли — z_{1−(1−conf)/2}·√(p(1−p)/n).
/// Границы решает потребитель: `p − ab_ci(...)` / `p + ab_ci(...)`
/// (образец — `ci_mean` FR-063). conf = 0 → вырожденный интервал (0).
fn ab_ci(values: &[Value]) -> Result<Value, EvalError> {
    let func = "ab_ci";
    let conv = count_arg(func, "conv", &values[0])?;
    let n = count_arg(func, "n", &values[1])?;
    let conf = prob_arg(func, "conf", &values[2])?;
    if n < 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("n должен быть ≥ 1, получено {}", format_num(n)),
        });
    }
    if conv > n {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "conv не может превышать n: {} > {}",
                format_num(conv),
                format_num(n)
            ),
        });
    }
    let p = conv / n;
    let z = if conf <= 0.0 {
        0.0
    } else {
        norm_quantile(1.0 - (1.0 - conf) / 2.0)
    };
    Ok(Value::with_unit(
        z * (p * (1.0 - p) / n).sqrt(),
        percent_unit(),
    ))
}

// --- Общие проверки аргументов abtest-домена --------------------------------

/// Общие входы z-семейства: (conv_a, n_a, conv_b, n_b) → (p_a, p_b, p̄).
/// Все четыре — целые Count/скаляры; n ≥ 1; 0 ≤ conv ≤ n. Вырожденный
/// пул (p̄ = 0 или 1: обе группы без конверсий / 100%) — BadCall:
/// стандартная ошибка z-теста нулевая, z и p-value не определены.
fn pooled_props(func: &str, values: &[Value]) -> Result<(f64, f64, f64), EvalError> {
    let conv_a = count_arg(func, "conv_a", &values[0])?;
    let n_a = count_arg(func, "n_a", &values[1])?;
    let conv_b = count_arg(func, "conv_b", &values[2])?;
    let n_b = count_arg(func, "n_b", &values[3])?;
    if n_a < 1.0 || n_b < 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "размеры групп должны быть ≥ 1, получены n_a = {}, n_b = {}",
                format_num(n_a),
                format_num(n_b)
            ),
        });
    }
    if conv_a > n_a || conv_b > n_b {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!(
                "конверсии не могут превышать визиты: {} > {} / {} > {}",
                format_num(conv_a),
                format_num(n_a),
                format_num(conv_b),
                format_num(n_b)
            ),
        });
    }
    let pooled = (conv_a + conv_b) / (n_a + n_b);
    if pooled == 0.0 || pooled == 1.0 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: "вырожденный тест: обе группы 0% или 100% конверсий — эффект не обнаружим"
                .to_owned(),
        });
    }
    let p_a = conv_a / n_a;
    let p_b = conv_b / n_b;
    Ok((p_a, p_b, pooled))
}

/// n-й (по индексу 0/2) размер группы из значений z-семейства.
fn n_of(values: &[Value], index: usize) -> f64 {
    values[index].num * values[index].unit.scale()
}

/// Строго скалярный аргумент (вероятности/уровни): любая размерность —
/// BadCall (семантика параметра требует безразмерности; образец —
/// `strict_scalar` FR-063).
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

/// Вероятность/уровень: строго скаляр, p ∈ [0, 1] (NaN отсечён интервалом).
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

/// Целочисленный аргумент-счётчик: скаляр или Count, целое, конечное,
/// ≥ 0 (образец — `count_arg` FR-063).
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
    Ok(value.num * value.unit.scale())
}

// --- Чистая математика: Φ и Φ⁻¹ без statrs ----------------------------------

const SQRT_2: f64 = std::f64::consts::SQRT_2;
const FRAC_1_SQRT_PI: f64 = 0.5641895835477563; // 1/√π
const FRAC_1_SQRT_2PI: f64 = 0.3989422804014327; // 1/√(2π)

/// `format_num` из super — переэкспорт для сообщений ошибок.
use super::format_num;

/// erfc(x) через ряд для малых |x|: erf(x) = (2/√π)·Σ(−1)ⁿ·x^(2n+1)/(n!(2n+1)).
/// Домен: |x| ≤ 2.5 (выше — цепная дробь [`erfc_cf`]).
fn erf_series(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x; // член ряда t_0 = x (n = 0)
    let mut sum = x;
    let mut n = 1.0f64;
    // Знакочередующийся ряд; критерий останова — вклад ниже эпсилона
    // суммарной величины (двойная точность достигается при |x| ≤ 2.5).
    while term.abs() > sum.abs() * 1e-17 && n < 60.0 {
        term = -term * x2 / n; // t_n = −t_{n−1}·x²/n
        sum += term / (2.0 * n + 1.0); // вклад t_n/(2n+1)
        n += 1.0;
    }
    sum * 2.0 * FRAC_1_SQRT_PI // (2/√π)
}

/// erfc(x) для x > 2.5 — цепная дробь (A&S 7.1.14):
/// erfc(x) = e^(−x²)/√π · 1/(x + 1/(2x + 2/(x + 3/(2x + 4/(x + …))))).
/// Оценка — модифицированный алгоритм Лентца (b0 = 0 → tiny); устойчива
/// и точна в хвосте (катастрофическое вычитание ряда убрано — важна
/// для p-value сильных тестов: erfc(8) ≈ 1.1e-29 не обнуляется).
fn erfc_cf(x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    // b0 = 0 → старт с tiny (модифицированный Лентц).
    let mut f = TINY;
    let mut c = f;
    let mut d = 0.0f64;
    for i in 1..=300u32 {
        // a_1 = 1, b_1 = x; для i ≥ 2: a_i = i−1, b_i = 2x (чёт) / x (нечёт).
        let (a, b) = if i == 1 {
            (1.0, x)
        } else {
            ((i - 1) as f64, if i % 2 == 0 { 2.0 * x } else { x })
        };
        d = b + a * d;
        if d == 0.0 {
            d = TINY;
        }
        c = b + a / c;
        if c == 0.0 {
            c = TINY;
        }
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < 1e-17 {
            break;
        }
    }
    (-x * x).exp() * FRAC_1_SQRT_PI * f
}

/// erfc(x) — дополнительная функция ошибок, двойная точность:
/// |x| ≤ 2.5 — ряд [`erf_series`], x > 2.5 — цепная дробь [`erfc_cf`],
/// отрицательные — симметрия erfc(−x) = 2 − erfc(x).
fn erfc(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.0 {
        2.0 - erfc(-x)
    } else if x <= 2.5 {
        1.0 - erf_series(x)
    } else {
        erfc_cf(x)
    }
}

/// Φ(z) — CDF стандартного нормального: Φ(z) = erfc(−z/√2)/2 (точно).
fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / SQRT_2)
}

/// Плотность стандартного нормального φ(z) = e^(−z²/2)/√(2π).
fn norm_pdf(z: f64) -> f64 {
    FRAC_1_SQRT_2PI * (-z * z / 2.0).exp()
}

/// Φ⁻¹(p) — квантиль стандартного нормального: аппроксимация Acklam
/// (отн. ошибка < 1.15e-9) + два шага Halley по [`norm_cdf`] —
/// до машинной точности (нужно для размера выборки: ошибка z → ошибка n).
fn norm_quantile(p: f64) -> f64 {
    // Коэффициенты Питера Аклама (P. Acklam, fino 2002; public domain).
    const A: [f64; 6] = [
        -3.969683028665376e+01,
        2.209460984245205e+02,
        -2.759285104469687e+02,
        1.383577518672690e+02,
        -3.066479806614716e+01,
        2.506628277459239e+00,
    ];
    const B: [f64; 5] = [
        -5.447609879822406e+01,
        1.615858368580409e+02,
        -1.556989798598866e+02,
        6.680131188771972e+01,
        -1.328068155288572e+01,
    ];
    const C: [f64; 6] = [
        -7.784894002430293e-03,
        -3.223964580411365e-01,
        -2.400758277161838e+00,
        -2.549732539343734e+00,
        4.374664141464968e+00,
        2.938163982698783e+00,
    ];
    const D: [f64; 4] = [
        7.784695709041462e-03,
        3.224671290700398e-01,
        2.445134137142996e+00,
        3.754408661907416e+00,
    ];
    let p_low = 0.02425;
    let p_high = 1.0 - p_low;
    let mut x: f64;
    if p < p_low {
        // Нижний хвост
        let q = (-2.0 * p.ln()).sqrt();
        x = (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    } else if p <= p_high {
        // Центральная область
        let q = p - 0.5;
        let r = q * q;
        x = (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0);
    } else if p < 1.0 {
        // Верхний хвост
        let q = (-2.0 * (1.0 - p).ln()).sqrt();
        x = -(((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0);
    } else {
        return f64::INFINITY;
    }
    // Уточнение Halley (2 шага): e = Φ(x) − p; поправка через φ(x).
    for _ in 0..2 {
        let e = norm_cdf(x) - p;
        let u = e / norm_pdf(x);
        x -= u / (1.0 + x * u / 2.0);
    }
    x
}

// --- Внутренние тесты --------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// erfc: симметрия и табличные значения (math.erf Python как эталон,
    /// допуски — эпсилоны реализации).
    #[test]
    fn erfc_known_values() {
        assert!((erfc(0.0) - 1.0).abs() < 1e-15);
        assert!(
            (erfc(1.0) - 0.15729920705028513).abs() < 1e-14,
            "{}",
            erfc(1.0)
        );
        assert!(
            (erfc(0.5) - 0.4795001221869535).abs() < 1e-14,
            "{}",
            erfc(0.5)
        );
        // Хвост (цепная дробь): erfc(3) = 2.2090496998585438e-5
        assert!(
            (erfc(3.0) / 2.2090496998585438e-5 - 1.0).abs() < 1e-14,
            "{}",
            erfc(3.0)
        );
        // erfc(8) ≈ 1.1224297172982928e-29 — хвост без обнуления
        // (сравнение относительное: абсолютная величина — 1e-29)
        assert!(
            (erfc(8.0) / 1.1224297172982928e-29 - 1.0).abs() < 1e-14,
            "{}",
            erfc(8.0)
        );
        // Симметрия
        assert!((erfc(-1.5) - (2.0 - erfc(1.5))).abs() < 1e-16);
    }

    /// Φ: табличные значения Φ(1.96) = 0.9750021048517795 (statrs-золотые).
    #[test]
    fn norm_cdf_golden() {
        assert!(
            (norm_cdf(1.959963984540054) - 0.975).abs() < 1e-14,
            "{}",
            norm_cdf(1.959963984540054)
        );
        assert!(
            (norm_cdf(1.96) - 0.9750021048517795).abs() < 1e-14,
            "{}",
            norm_cdf(1.96)
        );
        assert!((norm_cdf(0.0) - 0.5).abs() < 1e-15);
        // Хвост: 1 − Φ(8) = 6.22096057427178e-16 — без катастрофы
        assert!((norm_cdf(8.0) - 1.0).abs() < 1e-15);
    }

    /// Φ⁻¹: золотые квантили (statrs-золотые FR-063) + round-trip.
    #[test]
    fn norm_quantile_golden_and_round_trip() {
        assert!(
            (norm_quantile(0.975) - 1.9599639845400542).abs() < 1e-12,
            "{}",
            norm_quantile(0.975)
        );
        assert!(
            (norm_quantile(0.95) - 1.6448536269514722).abs() < 1e-12,
            "{}",
            norm_quantile(0.95)
        );
        assert!(
            (norm_quantile(0.8) - 0.8416212335729143).abs() < 1e-12,
            "{}",
            norm_quantile(0.8)
        );
        // Round-trip: Φ(Φ⁻¹(p)) = p на сетке
        for &p in &[0.001, 0.025, 0.2, 0.5, 0.8, 0.975, 0.999] {
            let z = norm_quantile(p);
            assert!(
                (norm_cdf(z) - p).abs() < 1e-13,
                "p = {p}: round-trip {z} → {}",
                norm_cdf(z)
            );
        }
    }

    /// pooled_props: n ≥ 1, conv ≤ n — доменные ошибки домена.
    #[test]
    fn pooled_props_rejects_bad_groups() {
        let ok = vec![
            Value::scalar(200.0),
            Value::scalar(10000.0),
            Value::scalar(260.0),
            Value::scalar(10000.0),
        ];
        assert!(pooled_props("ab_zscore", &ok).is_ok());
        // conv > n
        let bad = vec![
            Value::scalar(200.0),
            Value::scalar(100.0),
            Value::scalar(0.0),
            Value::scalar(100.0),
        ];
        assert!(matches!(
            pooled_props("ab_zscore", &bad),
            Err(EvalError::BadCall { .. })
        ));
        // n = 0
        let zero = vec![
            Value::scalar(0.0),
            Value::scalar(0.0),
            Value::scalar(0.0),
            Value::scalar(100.0),
        ];
        assert!(matches!(
            pooled_props("ab_zscore", &zero),
            Err(EvalError::BadCall { .. })
        ));
    }

    /// count_arg: Count-размерность принимается, дробные — нет.
    #[test]
    fn count_arg_accepts_count_dimension() {
        let count = Value::with_unit(
            100.0,
            super::super::Unit::atom(super::super::Atom::new(Dimension::Count, 1, 1.0, "req")),
        );
        assert_eq!(count_arg("t", "n", &count).unwrap(), 100.0);
        assert!(matches!(
            count_arg("t", "n", &Value::scalar(10.5)),
            Err(EvalError::BadCall { .. })
        ));
    }
}
