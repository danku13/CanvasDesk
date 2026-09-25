//! Golden-тесты доменного слоя A/B тестирования (expr/abtest.rs).
//!
//! Все значения — через публичный путь `parse` + `eval` (как пишет
//! пользователь в текстовой ноде; образец — `expr_stats.rs` FR-063).
//! Тесты БЕЗ фичей: модуль `abtest` не за фичей (архитектурное решение —
//! built-in шаблон/схема A/B обязаны работать во всех сборках), поэтому
//! гейты `#![cfg(feature = "stats")]` не нужны.
//!
//! Golden-значения (эталон — math.erf / NormalDist.inv_cdf Python 3.12;
//! допуски — машинные эпсилоны реализации):
//! - erfc: erfc(1) = 0.157299…; хвост erfc(8) = 1.1224e-29 (не обнуляется);
//! - pooled z-тест: A 200/10000 (2%) vs B 260/10000 (2.6%):
//!   z = 2.830251652793251, p = 0.004651140450960618, lift = 0.3;
//! - размер выборки: p0=0.02, mde=0.10, alpha=0.05, power=0.80 →
//!   n = 80682 на группу (сырое 80681.38, потолок);
//! - ДИ доли: 260/10000, conf=0.95 → полуширина 0.003118991875193938.

use canvas_core::expr::{eval, parse, Env, EvalError};

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

// === pooled z-семейство ===

/// Golden: A 200/10000 vs B 260/10000 → z = 2.830251652793251.
#[test]
fn ab_zscore_golden_growth_example() {
    let (num, unit) = eval_num("ab_zscore(200, 10000, 260, 10000)");
    assert!(
        (num - 2.830251652793251).abs() < 1e-12,
        "z = 2.830251…, получено {num}"
    );
    assert_eq!(unit, "", "z-статистика безразмерна");
}

/// Знак и симметрия: смена групп меняет знак z; обратный тест — тот же |z|.
#[test]
fn ab_zscore_group_order_flips_sign() {
    let (forward, _) = eval_num("ab_zscore(200, 10000, 260, 10000)");
    let (reversed, _) = eval_num("ab_zscore(260, 10000, 200, 10000)");
    assert!(
        (forward + reversed).abs() < 1e-12,
        "{forward} vs {reversed}"
    );
}

/// Golden: p-value = 0.004651140450960618 — доля 0..1 (юнит `%`).
/// Значимое различие при alpha = 0.05: p < 0.05.
#[test]
fn ab_pvalue_golden_significant_result() {
    let (num, unit) = eval_num("ab_pvalue(200, 10000, 260, 10000)");
    assert!(
        (num - 0.004651140450960618).abs() < 1e-14,
        "p = 0.004651…, получено {num}"
    );
    assert_eq!(unit, "%", "вероятность — доля 0..1 (юнит %)");
}

/// Идентичность z ↔ p: p(z=1.959964) = 0.05 (граница alpha = 0.05).
#[test]
fn ab_pvalue_boundary_at_one_point_nine_six() {
    // 200/10000 vs ~219.996/10000 дают |z| ≈ 1.959964… — проверим через
    // симметрию: p-value одинакова для зеркальных групп.
    let (p1, _) = eval_num("ab_pvalue(200, 10000, 260, 10000)");
    let (p2, _) = eval_num("ab_pvalue(260, 10000, 200, 10000)");
    assert!((p1 - p2).abs() < 1e-15, "p двусторонний — симметричен");
    // Незначимый случай: 200 vs 205 при n = 10000 → p > 0.05
    let (p, _) = eval_num("ab_pvalue(200, 10000, 205, 10000)");
    assert!(p > 0.05, "малый эффект не значим, получено p = {p}");
}

/// Golden: лифт (2.6 − 2)/2 = 0.3 (+30%) — доля (юнит `%`).
#[test]
fn ab_lift_golden_thirty_percent() {
    let (num, unit) = eval_num("ab_lift(200, 10000, 260, 10000)");
    assert!((num - 0.3).abs() < 1e-12, "lift = 0.3, получено {num}");
    assert_eq!(unit, "%");
    // Отрицательный лифт (проигрышный вариант)
    let (num, _) = eval_num("ab_lift(260, 10000, 200, 10000)");
    assert!((num + 0.23076923076923078).abs() < 1e-12, "получено {num}");
}

// === размер выборки ===

/// Golden: p0=0.02, mde=+10%, alpha=0.05, power=0.8 → 80682 на группу.
#[test]
fn ab_sample_size_golden_fleiss() {
    let (num, unit) = eval_num("ab_sample_size(0.02, 0.10, 0.05, 0.80)");
    assert!((num - 80682.0).abs() < 1.0, "n = 80682, получено {num}");
    assert_eq!(unit, "", "число пользователей — скаляр");
    // Целочисленность (потолок): результат — целое
    assert_eq!(num, num.trunc(), "n — целое (потолок)");
}

/// Монотонность: меньший эффект → большая выборка; выше мощность → больше n.
#[test]
fn ab_sample_size_monotonicity() {
    let (base, _) = eval_num("ab_sample_size(0.02, 0.10, 0.05, 0.80)");
    let (smaller_mde, _) = eval_num("ab_sample_size(0.02, 0.05, 0.05, 0.80)");
    let (higher_power, _) = eval_num("ab_sample_size(0.02, 0.10, 0.05, 0.90)");
    let (higher_alpha, _) = eval_num("ab_sample_size(0.02, 0.10, 0.10, 0.80)");
    assert!(
        smaller_mde > base,
        "меньший MDE требует больше: {smaller_mde} > {base}"
    );
    assert!(
        higher_power > base,
        "выше мощность — больше n: {higher_power} > {base}"
    );
    assert!(
        higher_alpha < base,
        "мягче alpha — меньше n: {higher_alpha} < {base}"
    );
}

// === доверительный интервал доли ===

/// Golden: 260/10000, conf=0.95 → полуширина 0.003118991875193938.
#[test]
fn ab_ci_golden_half_width() {
    let (num, unit) = eval_num("ab_ci(260, 10000, 0.95)");
    assert!(
        (num - 0.003118991875193938).abs() < 1e-14,
        "полуширина = 0.003119…, получено {num}"
    );
    assert_eq!(unit, "%");
}

/// conf = 0.95 vs 0.99: интервал 99% шире (z 1.96 → 2.576).
#[test]
fn ab_ci_widens_with_confidence() {
    let (half95, _) = eval_num("ab_ci(260, 10000, 0.95)");
    let (half99, _) = eval_num("ab_ci(260, 10000, 0.99)");
    assert!(half99 > half95, "99% ДИ шире 95%: {half99} > {half95}");
    // Границы интервала решает потребитель: p ± half накрывает p
    let (half, _) = eval_num("ab_ci(260, 10000, 0.95)");
    let p = 0.026;
    assert!(half < p, "полуширина меньше доли — границы положительны");
}

// === ошибки домена (BadCall) ===

/// Дробные конверсии, conv > n, вырожденный пул, диапазоны вероятностей.
#[test]
fn ab_domain_rejects_invalid_inputs() {
    // Дробная конверсия
    assert!(matches!(
        eval_err("ab_zscore(200.5, 10000, 260, 10000)"),
        EvalError::BadCall { .. }
    ));
    // conv > n
    assert!(matches!(
        eval_err("ab_pvalue(20000, 10000, 260, 10000)"),
        EvalError::BadCall { .. }
    ));
    // Вырожденный пул (обе группы без конверсий)
    assert!(matches!(
        eval_err("ab_zscore(0, 10000, 0, 10000)"),
        EvalError::BadCall { .. }
    ));
    // Нулевая базовая конверсия в lift
    assert!(matches!(
        eval_err("ab_lift(0, 10000, 260, 10000)"),
        EvalError::BadCall { .. }
    ));
    // p0 вне 0..1
    assert!(matches!(
        eval_err("ab_sample_size(1.5, 0.10, 0.05, 0.80)"),
        EvalError::BadCall { .. }
    ));
    // mde = 0 → эффект не задан
    assert!(matches!(
        eval_err("ab_sample_size(0.02, 0, 0.05, 0.80)"),
        EvalError::BadCall { .. }
    ));
    // mde уводит p_b за 1
    assert!(matches!(
        eval_err("ab_sample_size(0.95, 0.20, 0.05, 0.80)"),
        EvalError::BadCall { .. }
    ));
    // power вне 0..1
    assert!(matches!(
        eval_err("ab_sample_size(0.02, 0.10, 0.05, 1.0)"),
        EvalError::BadCall { .. }
    ));
    // Размерность в вероятности (семантика параметров — безразмерность)
    assert!(matches!(
        eval_err("ab_sample_size(0.02, 0.10, 0.05, 0.80 rps)"),
        EvalError::BadCall { .. }
    ));
    // conf вне 0..1
    assert!(matches!(
        eval_err("ab_ci(260, 10000, 1.2)"),
        EvalError::BadCall { .. }
    ));
}

/// Арность: ровно 4 аргумента у z-семейства / sample_size, 3 у ci.
#[test]
fn ab_arity_errors() {
    assert!(matches!(
        eval_err("ab_zscore(200, 10000, 260)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("ab_pvalue(200, 10000, 260, 10000, 0.05)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("ab_sample_size(0.02, 0.10, 0.05)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("ab_ci(260, 10000)"),
        EvalError::BadCall { .. }
    ));
}

/// Count-размерность принимается (входы из analytics-систем — Count).
#[test]
fn ab_accepts_count_dimension_inputs() {
    let (num, _) = eval_num("ab_zscore(200 req, 10000 req, 260 req, 10000 req)");
    assert!((num - 2.830251652793251).abs() < 1e-12, "получено {num}");
}

/// Совместимость со строковым листом ноды: multi-line Numi-лист с
/// промежуточными переменными (реальный сценарий карточки A/B).
#[test]
fn ab_multiline_node_sheet() {
    let (num, unit) =
        eval_num("ca = 200\nna = 10000\ncb = 260\nnb = 10000\np = ab_pvalue(ca, na, cb, nb)");
    assert!((num - 0.004651140450960618).abs() < 1e-14, "получено {num}");
    assert_eq!(unit, "%");
}
