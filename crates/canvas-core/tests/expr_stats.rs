//! FR-063: golden-тесты доменного слоя статистики (распределения,
//! квантили, доверительные интервалы, детерминированный RNG).
//!
//! Все значения — через публичный путь `parse` + `eval` (как пишет
//! пользователь в текстовой ноде; образец — `expr_queueing.rs`). Тесты
//! исполняются ТОЛЬКО с фичей `stats` (`cargo test -p canvas-core
//! --features stats` — гейт FR-063); без фичи модуль не компилируется и
//! функции деградируют до `UnknownFunction` — обратная совместимость
//! покрыта `expr_stats_compat.rs` (зеркальный `cfg(not(feature))`).
//!
//! Golden-значения (±1e-9 — инвариант 2-образец FR-015):
//! - Φ(1.96) = 0.9750021048517795; Φ⁻¹(0.975) = 1.9599639845400542;
//! - LogNormal-медиана: exp(0) = 1; exp-квантиль: −ln(1−p)/λ;
//! - Пуассон: e⁻²·2³/3! = 0.1804470443154836;
//! - треугольное (0,1, мода 0.5): q(0.5) = 0.5, q(0.25) = √0.125.
#![cfg(feature = "stats")]

use canvas_core::expr::{eval, parse, Env, EvalError, Expr};

/// Результат вычисления формулы — (число, отображение единицы).
fn eval_num(source: &str) -> (f64, String) {
    let expr = parse(source).expect("формула парсится");
    let value = eval(&expr, &Env::empty()).expect("формула вычисляется");
    (value.num, value.unit.display())
}

fn eval_err(source: &str) -> EvalError {
    let expr = parse(source).expect("формула парсится");
    eval(&expr, &Env::empty()).expect_err("ожидалась ошибка")
}

// === P2: нормальное распределение ===

/// Golden FR-063: normal_cdf(1.96, 0, 1) ≈ 0.975002 (±1e-9).
#[test]
fn normal_cdf_golden_one_point_nine_six() {
    let (num, unit) = eval_num("normal_cdf(1.96, 0, 1)");
    assert!(
        (num - 0.9750021048517795).abs() < 1e-9,
        "Φ(1.96) = 0.9750021…, получено {num}"
    );
    assert_eq!(unit, "%", "вероятность — доля 0..1 (юнит %)");
}

/// Точное обращение: Φ(Φ⁻¹(0.975)) = 0.975 (round-trip ±1e-9).
#[test]
fn normal_cdf_quantile_round_trip() {
    let (z, _) = eval_num("normal_quantile(0.975, 0, 1)");
    assert!(
        (z - 1.9599639845400542).abs() < 1e-9,
        "Φ⁻¹(0.975) = 1.959964…, получено {z}"
    );
    let (p, _) = eval_num("normal_cdf(1.9599639845400542, 0, 1)");
    assert!(
        (p - 0.975).abs() < 1e-9,
        "Φ(1.959964…) = 0.975, получено {p}"
    );
    // P95 стандартного нормального: 1.6448536269514722
    let (z95, _) = eval_num("normal_quantile(0.95, 0, 1)");
    assert!((z95 - 1.6448536269514722).abs() < 1e-9, "получено {z95}");
}

/// Сдвиг/масштаб: normal_quantile(p, μ, σ) = μ + σ·z (размерность μ).
#[test]
fn normal_quantile_location_scale_and_units() {
    let (num, unit) = eval_num("normal_quantile(0.975, 100, 10)");
    assert!(
        (num - (100.0 + 10.0 * 1.9599639845400542)).abs() < 1e-9,
        "μ + σ·z, получено {num}"
    );
    assert_eq!(unit, "", "скалярный μ — безразмерный результат");
    // P95 утилизации (пример FR-063 «Наглядная проверка»): безразмерный
    let (num, unit) = eval_num("normal_quantile(0.95, 0.83, 0.1)");
    assert!((num - (0.83 + 0.1 * 1.6448536269514722)).abs() < 1e-9);
    assert_eq!(unit, "");
    // Скорость: μ = 1000 rps, σ = 100 rps → результат в rps (z₀.₉₇₅ = 1.959964)
    let (num, unit) = eval_num("normal_quantile(0.975, 1000 rps, 100 rps)");
    assert!((num - 1195.9963984540054).abs() < 1e-9, "получено {num}");
    assert_eq!(unit, "rps", "размерность результата = размерность μ");
    // Время: μ = 50 ms, σ = 10 ms → результат в ms (юнит отображения μ)
    let (num, unit) = eval_num("normal_quantile(0.975, 50 ms, 10 ms)");
    assert!((num - 69.59963984540054).abs() < 1e-9, "получено {num}");
    assert_eq!(unit, "ms");
}

/// normal_cdf с размерностями: x/μ/σ одной размерности → вероятность.
#[test]
fn normal_cdf_with_units() {
    // P(X ≤ 1100 rps) для N(1000, 100): Φ(1) = 0.8413447460685429
    let (num, unit) = eval_num("normal_cdf(1100 rps, 1000 rps, 100 rps)");
    assert!((num - 0.8413447460685429).abs() < 1e-9, "получено {num}");
    assert_eq!(unit, "%");
    // ms-версия: P(X ≤ 60 ms) для N(50, 10): Φ(1)
    let (num, _) = eval_num("normal_cdf(60 ms, 50 ms, 10 ms)");
    assert!((num - 0.8413447460685429).abs() < 1e-9, "получено {num}");
}

// === P2: логнормальное и экспоненциальное ===

/// Golden FR-063: lognormal_quantile(0.5, 0, 1) = 1.0 (медиана e⁰).
#[test]
fn lognormal_quantile_median_is_one() {
    let (num, unit) = eval_num("lognormal_quantile(0.5, 0, 1)");
    assert!((num - 1.0).abs() < 1e-12, "e⁰ = 1, получено {num}");
    assert_eq!(unit, "", "лог-пространство — безразмерный результат");
    // z = 1 → e¹
    let (num, _) = eval_num("lognormal_quantile(0.8413447460685429, 0, 1)");
    assert!((num - std::f64::consts::E).abs() < 1e-9, "получено {num}");
}

/// exp_quantile: golden −ln(1−p)/λ; Rate-λ → секунды.
#[test]
fn exp_quantile_golden_and_rate_units() {
    // FR-063 golden: exp_quantile(0.632, λ=1) ≈ 1.0 (−ln(0.368) = 0.99967)
    let (num, unit) = eval_num("exp_quantile(0.632, 1)");
    assert!((num - 0.9996723408132061).abs() < 1e-9, "получено {num}");
    assert_eq!(unit, "", "скалярный λ — безразмерный результат");
    // Точное p = 1 − e⁻¹ → ровно 1
    let (num, _) = eval_num("exp_quantile(0.6321205588285577, 1)");
    assert!((num - 1.0).abs() < 1e-9, "получено {num}");
    // λ = 2 rps → 0.5 sec (базовая секунда — контракт FR-015)
    let (num, unit) = eval_num("exp_quantile(0.6321205588285577, 2 rps)");
    assert!((num - 0.5).abs() < 1e-9, "получено {num}");
    assert_eq!(unit, "sec");
}

// === P2: Пуассон и треугольное ===

/// Golden FR-063: poisson_pmf(k=3, λ=2) ≈ 0.180447 (±1e-9).
#[test]
fn poisson_pmf_golden_k3_lambda2() {
    let (num, unit) = eval_num("poisson_pmf(3, 2)");
    assert!(
        (num - 0.1804470443154836).abs() < 1e-9,
        "e⁻²·2³/3! = 0.1804470…, получено {num}"
    );
    assert_eq!(unit, "%", "вероятность — доля 0..1");
    // k=0: e⁻²
    let (num, _) = eval_num("poisson_pmf(0, 2)");
    assert!((num - 0.1353352832366127).abs() < 1e-9, "получено {num}");
    // k в Count — валидно
    let (num, _) = eval_num("poisson_pmf(3 req, 2)");
    assert!((num - 0.1804470443154836).abs() < 1e-9);
}

/// Golden FR-063: triangular_quantile(0.5, 0, 1, 0.5) = 0.5.
#[test]
fn triangular_quantile_golden() {
    let (num, unit) = eval_num("triangular_quantile(0.5, 0, 1, 0.5)");
    assert!((num - 0.5).abs() < 1e-12, "мода-медиана, получено {num}");
    assert_eq!(unit, "");
    // q(0.25) = √0.125 (левая ветвь)
    let (num, _) = eval_num("triangular_quantile(0.25, 0, 1, 0.5)");
    assert!((num - 0.3535533905932738).abs() < 1e-9, "получено {num}");
    // Граничные p: p=0 → a, p=1 → b (support ограничен — конечен)
    let (num, _) = eval_num("triangular_quantile(0, 2 ms, 10 ms, 3 ms)");
    assert!((num - 2.0).abs() < 1e-12, "p=0 → a, получено {num}");
    let (num, unit) = eval_num("triangular_quantile(1, 2 ms, 10 ms, 3 ms)");
    assert!((num - 10.0).abs() < 1e-12, "p=1 → b, получено {num}");
    assert_eq!(unit, "ms", "размерность результата = размерность a");
    // Алиас `triangular` — та же функция (таблица «Изменения» FR-063)
    let (alias, _) = eval_num("triangular(0.5, 0, 1, 0.5)");
    let (canon, _) = eval_num("triangular_quantile(0.5, 0, 1, 0.5)");
    assert_eq!(alias, canon);
}

// === P2: ошибки арности, диапазона и размерности ===

/// p вне [0,1] и σ ≤ 0 — BadCall (контракт FR-063 «Проверка»).
#[test]
fn quantile_domain_errors_are_bad_call() {
    // p > 1
    match eval_err("normal_quantile(1.5, 0, 1)") {
        EvalError::BadCall { func, msg } => {
            assert_eq!(func, "normal_quantile");
            assert!(msg.contains("0..1"), "{msg}");
        }
        other => panic!("ожидался BadCall (p > 1), получено {other:?}"),
    }
    // p < 0
    assert!(matches!(
        eval_err("normal_quantile(-0.1, 0, 1)"),
        EvalError::BadCall { .. }
    ));
    // σ < 0
    match eval_err("normal_quantile(0.5, 0, -1)") {
        EvalError::BadCall { func, .. } => assert_eq!(func, "normal_quantile"),
        other => panic!("ожидался BadCall (σ < 0), получено {other:?}"),
    }
    // σ = 0
    assert!(matches!(
        eval_err("normal_quantile(0.5, 0, 0)"),
        EvalError::BadCall { .. }
    ));
    // размерностный p — BadCall (строгая безразмерность)
    assert!(matches!(
        eval_err("normal_quantile(0.5 rps, 0, 1)"),
        EvalError::BadCall { .. }
    ));
}

/// Несовпадение размерностей μ/σ (x) — UnitMismatch (ограничение 3/7).
#[test]
fn dimension_mismatch_is_unit_mismatch() {
    match eval_err("normal_quantile(0.95, 5 sec, 1 rps)") {
        EvalError::UnitMismatch { .. } => {}
        other => panic!("ожидался UnitMismatch, получено {other:?}"),
    }
    match eval_err("normal_cdf(1 rps, 5 sec, 1 sec)") {
        EvalError::UnitMismatch { .. } => {}
        other => panic!("ожидался UnitMismatch (x vs μ), получено {other:?}"),
    }
    // Треугольное: b не в размерности a
    assert!(matches!(
        eval_err("triangular_quantile(0.5, 0 sec, 1 rps, 0.5 sec)"),
        EvalError::UnitMismatch { .. }
    ));
    // Логнормальное: параметры строго безразмерны
    assert!(matches!(
        eval_err("lognormal_quantile(0.5, 1 sec, 1)"),
        EvalError::BadCall { .. }
    ));
}

/// Доменные ошибки Пуассона и треугольного.
#[test]
fn poisson_and_triangular_domain_errors() {
    // Дробный k
    assert!(matches!(
        eval_err("poisson_pmf(1.5, 2)"),
        EvalError::BadCall { .. }
    ));
    // Отрицательный k
    assert!(matches!(
        eval_err("poisson_pmf(-1, 2)"),
        EvalError::BadCall { .. }
    ));
    // Отрицательный λ
    assert!(matches!(
        eval_err("poisson_pmf(3, -2)"),
        EvalError::BadCall { .. }
    ));
    // Треугольное: a ≥ b
    assert!(matches!(
        eval_err("triangular_quantile(0.5, 1, 0, 0.5)"),
        EvalError::BadCall { .. }
    ));
    // Треугольное: мода вне [a, b]
    assert!(matches!(
        eval_err("triangular_quantile(0.5, 0, 1, 2)"),
        EvalError::BadCall { .. }
    ));
    // exp_quantile: λ ≤ 0 и чужая размерность
    assert!(matches!(
        eval_err("exp_quantile(0.5, -1)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("exp_quantile(0.5, 5 sec)"),
        EvalError::BadCall { .. }
    ));
}

/// Arity guard — недостаток/избыток аргументов → BadCall.
#[test]
fn stats_wrong_arity_is_rejected() {
    assert!(matches!(
        eval_err("normal_quantile(0.95, 0)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("normal_cdf(1)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("exp_quantile(0.5)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("poisson_pmf(3)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("triangular_quantile(0.5, 0, 1)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("normal_quantile(0.95, 0, 1, 2)"),
        EvalError::BadCall { .. }
    ));
}

/// Все stats-функции маршрутизируются как `Expr::Call` (FR-013 грамматика).
#[test]
fn stats_calls_parse_as_function_call() {
    for (source, name) in [
        ("normal_quantile(0.975, 0, 1)", "normal_quantile"),
        ("normal_cdf(1.96, 0, 1)", "normal_cdf"),
        ("lognormal_quantile(0.5, 0, 1)", "lognormal_quantile"),
        ("exp_quantile(0.632, 1)", "exp_quantile"),
        ("poisson_pmf(3, 2)", "poisson_pmf"),
        ("triangular_quantile(0.5, 0, 1, 0.5)", "triangular_quantile"),
        ("triangular(0.5, 0, 1, 0.5)", "triangular"),
    ] {
        let expr = parse(source).unwrap();
        assert!(
            matches!(&expr, Expr::Call { func, .. } if func == name),
            "{source} не распознан как вызов {name}"
        );
    }
}

/// Формулы в строках листа (eval_lines): P95 утилизации на канвасе —
/// сценарий «Наглядная проверка» FR-063.
#[test]
fn stats_in_note_lines_shows_result() {
    let lines = canvas_core::expr::eval_lines(
        "utilization = 0.83\nsigma = 0.1\n= normal_quantile(0.95, utilization, sigma)",
    );
    match &lines[2] {
        Some(canvas_core::expr::ExprOutcome::Ok(value)) => {
            assert!(
                (value.num - (0.83 + 0.1 * 1.6448536269514722)).abs() < 1e-9,
                "P95 = μ + σ·z, получено {}",
                value.num
            );
        }
        other => panic!("ожидался результат P95, получено {other:?}"),
    }
}
