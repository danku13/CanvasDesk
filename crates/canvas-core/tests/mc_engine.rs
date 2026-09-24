//! FR-066: интеграционные гейты MC/QMC-движка (фазы P1/P2/P3 ядра).
//!
//! Гоняются только с фичей `qmc` (`cargo test -p canvas-core --features qmc`);
//! без неё файл компилируется в пустоту — zero-dep инвариант B2B (§5.6/§5.8).
//!
//! Покрытие чек-листа «Проверка» FR-066:
//! - seed-воспроизводимость: тот же seed → **побитово** тот же
//!   `Vec<FlowSolutions>` и те же квантили;
//! - квантили на эталоне №5 ADR-0006 (unit economics) против oracle ±1 %
//!   (паттерн `close_1pct`, `canvas-scene/src/tests.rs:1470`);
//! - QMC-дисперсия < MC-дисперсия при том же N;
//! - `analyze` на синтетическом снимке P90 — `Severity` корректен
//!   (overlay bottleneck на P90, FR-016 без правок);
//! - engine-метаданные §5.7.4 (stale при рассинхроне версии);
//! - бюджет производительности: 10⁴ прогонов эталона №5 (release < 1 с).

#![cfg(feature = "qmc")]

use std::collections::HashMap;

use canvas_core::analyze::{self, AnalysisConfig, Severity};
use canvas_core::expr::mc::{self, Distribution, McConfig, McMode, ENGINE_VERSION};
use canvas_core::flow::propagate_monte_carlo;
use canvas_core::time::Instant;
use canvas_core::{Canvas, Node};

/// Эталон №5 ADR-0006 (unit economics) одним Numi-листом: CAC/ARPU/churn →
/// LTV/LTV-CAC/payback (базовые значения — таблица «Эталонные значения»).
fn etalon5_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text(
        "ue",
        "spend = 60000\nnew_customers = 3000\ncac = spend / new_customers\narpu = 12\nmargin = 0.8\nchurn = 0.1\nltv = arpu * margin / churn\nratio = ltv / cac\npayback = cac / (arpu * margin)",
        0.0,
        0.0,
    ));
    canvas
}

fn etalon5_params() -> HashMap<(String, String), Distribution> {
    HashMap::from([(
        ("ue".to_owned(), "new_customers".to_owned()),
        Distribution::Normal {
            mean: 3000.0,
            sd: 100.0,
        },
    )])
}

/// ±1 % (паттерн close_1pct, гейт эталонов ADR-0005/0006).
fn close_1pct(actual: f64, oracle: f64) -> bool {
    (actual - oracle).abs() <= oracle.abs() * 0.01
}

/// Каноническое представление `Vec<FlowSolutions>` для побитовой сверки
/// (§5.7.2): HashMap-порядок итерации не детерминирован между потоками
/// (RandomState сид-ится на поток — FR-065 par_iter), значения — совпадают
/// побитово. Канонизация: ключи отсортированы, числа — `{:?}` (roundtrip).
fn canonical_runs(runs: &[canvas_core::FlowSolutions]) -> Vec<String> {
    runs.iter()
        .map(|s| {
            let mut parts: Vec<String> = Vec::new();
            let mut outputs: Vec<_> = s.outputs.iter().collect();
            outputs.sort_by(|a, b| a.0.cmp(b.0));
            for (key, outcome) in outputs {
                let value = match outcome {
                    Ok(v) => format!("{:?}", v.num),
                    Err(_) => "err".to_owned(),
                };
                parts.push(format!("o{key}={value}"));
            }
            let mut lines: Vec<_> = s.lines.iter().collect();
            lines.sort_by_key(|a| (a.0 .0.clone(), a.0 .1));
            for (key, value) in lines {
                parts.push(format!("l{:?}:{:?}={:?}", key.0, key.1, value.num));
            }
            let mut named: Vec<_> = s.named.iter().collect();
            named.sort_by(|a, b| a.0.cmp(b.0));
            for (key, value) in named {
                parts.push(format!("n{:?}:{:?}={:?}", key.0, key.1, value.num));
            }
            parts.join("|")
        })
        .collect()
}

/// P1-гейт: тот же seed → побитово тот же `Vec<FlowSolutions>`
/// (каноническое сравнение через [`canonical_runs`] — значения f64
/// печатаются roundtrip-точно, порядок ключей стабилизирован сортировкой)
/// и тот же `McResult`.
#[test]
fn mc_seed_reproducibility_is_bitwise() {
    let canvas = etalon5_canvas();
    let config = McConfig {
        runs: 512,
        params: etalon5_params(),
        seed: 42,
        quantiles: vec![0.5, 0.9, 0.99],
        mode: McMode::Qmc,
    };
    let a = mc::run_solutions(&canvas, &config, &|_, _| {}).expect("прогоны A");
    let b = mc::run_solutions(&canvas, &config, &|_, _| {}).expect("прогоны B");
    assert_eq!(
        canonical_runs(&a),
        canonical_runs(&b),
        "тот же seed — побитово тот же Vec<FlowSolutions> (канонично)"
    );
    let ra = propagate_monte_carlo(&canvas, &config).expect("результат A");
    let rb = propagate_monte_carlo(&canvas, &config).expect("результат B");
    assert_eq!(ra, rb, "тот же seed — идентичные квантили");

    // MC-режим — тот же контракт
    let config_mc = McConfig {
        mode: McMode::Mc,
        ..config.clone()
    };
    let a = mc::run_solutions(&canvas, &config_mc, &|_, _| {}).expect("MC A");
    let b = mc::run_solutions(&canvas, &config_mc, &|_, _| {}).expect("MC B");
    assert_eq!(
        canonical_runs(&a),
        canonical_runs(&b),
        "MC: побитовая идентичность"
    );

    // другой seed — другой поток (дисперсия наблюдаема)
    let other = McConfig { seed: 43, ..config };
    let c = mc::run_solutions(&canvas, &other, &|_, _| {}).expect("прогоны C");
    assert_ne!(
        canonical_runs(&a),
        canonical_runs(&c),
        "другой seed — другие прогоны"
    );
}

/// P2-гейт: квантили эталона №5 против oracle ±1 %.
/// cac = 60000 / N(3000, 100): медиана — ровно 20 (симметрия нормали),
/// P90: 60000/(3000 − 100·Φ⁻¹(0.9)) = 60000/2871.845 = 20.895,
/// P99: 60000/(3000 − 100·Φ⁻¹(0.99)) = 60000/2767.365 = 21.684.
#[test]
fn mc_quantiles_match_etalon5_oracle() {
    let canvas = etalon5_canvas();
    let config = McConfig {
        runs: 10_000,
        params: etalon5_params(),
        seed: 7,
        quantiles: vec![0.5, 0.9, 0.99],
        mode: McMode::Qmc,
    };
    let result = propagate_monte_carlo(&canvas, &config).expect("QMC-прогон");
    assert_eq!(result.runs, 10_000);
    assert_eq!(result.failed_runs, 0, "валидный конфиг — без падений");
    assert_eq!(result.quantiles, vec![0.5, 0.9, 0.99]);

    // cac — именованный выход листа (переменная Numi)
    let cac = &result.named[&("ue".to_owned(), "cac".to_owned())];
    let (p50, p90, p99) = (cac[0].num, cac[1].num, cac[2].num);
    assert!(
        close_1pct(p50, 20.0),
        "P50(cac) = {p50} vs oracle 20.0 (±1 %)"
    );
    assert!(
        close_1pct(p90, 20.895),
        "P90(cac) = {p90} vs oracle 20.895 (±1 %)"
    );
    assert!(
        close_1pct(p99, 21.684),
        "P99(cac) = {p99} vs oracle 21.684 (±1 %)"
    );
    assert!(p50 < p90 && p90 < p99, "монотонность квантилей");

    // построчные выходы: строка «cac = spend / new_customers» (индекс 2)
    let line = &result.lines[&("ue".to_owned(), 2)];
    assert!(
        close_1pct(line[1].num, 20.895),
        "P90 строки cac = {} vs 20.895",
        line[1].num
    );

    // синтетический снимок: узловое значение ноды = последняя строка
    // (payback 2.083); P-метки — именованные выходы
    let snapshot = mc::synthetic_solutions(&result, 0.9).expect("P90-снимок");
    let payback = snapshot.outputs["ue"].as_ref().expect("значение ноды");
    assert!(
        close_1pct(payback.num, 20.895 / 9.6),
        "P90(payback) = {} vs oracle {}",
        payback.num,
        20.895 / 9.6
    );
    // P-метка P90 ноды == её P90-значение (снимок согласован)
    assert_eq!(
        snapshot.named[&("ue".to_owned(), "P90".to_owned())],
        *payback
    );
    assert!(snapshot
        .named
        .contains_key(&("ue".to_owned(), "P50".to_owned())));
    assert!(snapshot
        .named
        .contains_key(&("ue".to_owned(), "P99".to_owned())));
}

/// P2-гейт: QMC-дисперсия < MC-дисперсия при том же N (статистика —
/// P50(cac) на эталоне №5; 12 сидов × 4096 прогонов; детерминированные
/// константы — прогон воспроизводим).
#[test]
fn qmc_variance_is_lower_than_mc() {
    let canvas = etalon5_canvas();
    let params = HashMap::from([(
        ("ue".to_owned(), "new_customers".to_owned()),
        Distribution::Normal {
            mean: 3000.0,
            sd: 300.0,
        },
    )]);
    let variance = |xs: &[f64]| {
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (xs.len() - 1) as f64
    };
    let mut mc_estimates = Vec::new();
    let mut qmc_estimates = Vec::new();
    for seed in 1..=12u64 {
        for mode in [McMode::Mc, McMode::Qmc] {
            let config = McConfig {
                runs: 4096,
                params: params.clone(),
                seed,
                quantiles: vec![0.5],
                mode,
            };
            let result = propagate_monte_carlo(&canvas, &config).expect("прогон");
            let p50 = result.named[&("ue".to_owned(), "cac".to_owned())][0].num;
            match mode {
                McMode::Mc => mc_estimates.push(p50),
                McMode::Qmc => qmc_estimates.push(p50),
            }
        }
    }
    let (mc_var, qmc_var) = (variance(&mc_estimates), variance(&qmc_estimates));
    assert!(
        qmc_var < mc_var,
        "QMC-дисперсия {qmc_var} должна быть меньше MC-дисперсии {mc_var} \
         (12 сидов × 4096 прогонов, статистика P50)"
    );
}

/// P3-гейт (ядро): `analyze` на синтетическом P90-снимке — Severity
/// корректно считается на хвосте (P50 → Warn, P90 → Critical), FR-016
/// без правок. Канвас: утилизация ρ как Percent-строка (анализ трактует
/// Percent-значение ноды как ρ); «load × 1 %» придаёт скалярному
/// распределению Percent-размерность (Numi-семантика FR-050 Н5).
#[test]
fn analyze_severity_escalates_on_p90_synthetic() {
    // load ~ N(0.85, 0.05) → rho = load × 1 % : P50 ≈ 0.85 (Warn ≥ 0.7),
    // P90 ≈ 0.85 + 1.28·0.05 = 0.914 (Critical ≥ 0.9)
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::text("srv", "load = 0.85\nrho = load × 1 %", 0.0, 0.0));
    let params = HashMap::from([(
        ("srv".to_owned(), "load".to_owned()),
        Distribution::Normal {
            mean: 0.85,
            sd: 0.05,
        },
    )]);
    let config = McConfig {
        runs: 4096,
        params,
        seed: 11,
        quantiles: vec![0.5, 0.9, 0.99],
        mode: McMode::Qmc,
    };
    let result = propagate_monte_carlo(&canvas, &config).expect("прогон");
    let cfg = AnalysisConfig::default();

    let p50 = mc::synthetic_solutions(&result, 0.5).expect("P50-снимок");
    let state50 = analyze::analyze(&canvas, &p50, &cfg);
    assert_eq!(
        state50.get("srv").expect("флаги srv").severity,
        Severity::Warn,
        "P50 ρ ≈ 0.85 — Warn (порог 0.7)"
    );

    let p90 = mc::synthetic_solutions(&result, 0.9).expect("P90-снимок");
    let state90 = analyze::analyze(&canvas, &p90, &cfg);
    let flags = state90.get("srv").expect("флаги srv");
    assert_eq!(
        flags.severity,
        Severity::Critical,
        "P90 ρ ≈ 0.914 — Critical"
    );
    assert!(
        flags.utilization.is_some_and(|u| u > 0.9),
        "ρ из P90-снимка: {:?}",
        flags.utilization
    );

    // хвостовой квантиль по умолчанию — P90 (для [0.5, 0.9, 0.99])
    assert_eq!(mc::tail_quantile(&result.quantiles), Some(0.9));
}

/// §5.7.4: рассинхрон версии движка в extra → McResult.stale = true;
/// актуальная версия — false. Канвас после прогона MCP-слоем помечается
/// текущей версией — ядро только читает.
#[test]
fn engine_version_mismatch_marks_result_stale() {
    let canvas = etalon5_canvas();
    let config = McConfig {
        runs: 256,
        params: etalon5_params(),
        seed: 1,
        quantiles: vec![0.5, 0.9, 0.99],
        mode: McMode::Qmc,
    };
    let fresh = propagate_monte_carlo(&canvas, &config).expect("свежий канвас");
    assert!(!fresh.stale, "нет метаданных — не протухло");

    let mut old = etalon5_canvas();
    mc::engine_to_canvas(
        &mut old,
        &mc::EngineMeta {
            version: "M4.9".to_owned(),
            seed: 0,
            stats: true,
            parallel: true,
            qmc: false,
        },
    );
    let stale = propagate_monte_carlo(&old, &config).expect("прогон по старым метаданным");
    assert!(
        stale.stale,
        "версия M4.9 ≠ {ENGINE_VERSION} — снимок протух"
    );

    let mut current = etalon5_canvas();
    mc::engine_to_canvas(&mut current, &mc::current_engine_meta(0));
    let ok = propagate_monte_carlo(&current, &config).expect("прогон по текущим метаданным");
    assert!(!ok.stale, "текущая версия — свежий снимок");
}

/// Тихая деградация ядра: неразрешённые параметры (нода/строка не найдены)
/// — в skipped_params, прогоны идут по базовому канвасу; провальный
/// QMC-конфиг (runs > 2^16) — failed_runs без паники.
#[test]
fn unresolved_params_and_failed_runs_degrade_gracefully() {
    let canvas = etalon5_canvas();
    let params = HashMap::from([
        (
            ("ghost".to_owned(), "x".to_owned()),
            Distribution::Exp { lambda: 1.0 },
        ),
        (
            ("ue".to_owned(), "no_such_param".to_owned()),
            Distribution::Exp { lambda: 1.0 },
        ),
    ]);
    let config = McConfig {
        runs: 128,
        params,
        seed: 0,
        quantiles: vec![0.5],
        mode: McMode::Qmc,
    };
    let result = propagate_monte_carlo(&canvas, &config).expect("прогон с пропусами");
    assert_eq!(result.skipped_params.len(), 2, "оба параметра не разрешены");
    assert_eq!(result.failed_runs, 0);
    // прогоны по базе: медиана cac = 20 (параметры не подменялись)
    assert!(
        close_1pct(
            result.named[&("ue".to_owned(), "cac".to_owned())][0].num,
            20.0
        ),
        "базовые значения считаются"
    );

    // провальное распределение (validate отклонил бы; ядро деградирует):
    // sd = 0 → конструктор rand_distr/ inverse-CDF — каждый прогон роняется,
    // квантилей нет, паники нет
    let broken = McConfig {
        runs: 16,
        params: HashMap::from([(
            ("ue".to_owned(), "new_customers".to_owned()),
            Distribution::Normal { mean: 0.0, sd: 0.0 },
        )]),
        seed: 0,
        quantiles: vec![0.5],
        mode: McMode::Qmc,
    };
    let result = propagate_monte_carlo(&canvas, &broken).expect("без паники");
    assert_eq!(result.failed_runs, 16, "все прогоны роняются");
    assert!(result.outputs.is_empty(), "квантилей нет");
}

/// Бюджет производительности (FR-066 «Ограничение 7»): 10⁴ прогонов
/// эталона №5 — < 1 с / 4 ядра (release-норма; debug-сборке — щадящий
/// порог: парсинг формул в debug на порядок медленнее).
#[test]
fn mc_perf_10k_etalon5_under_budget() {
    let canvas = etalon5_canvas();
    let config = McConfig {
        runs: 10_000,
        params: etalon5_params(),
        seed: 3,
        quantiles: vec![0.5, 0.9, 0.99],
        mode: McMode::Qmc,
    };
    let started = Instant::now();
    let result = propagate_monte_carlo(&canvas, &config).expect("10^4 прогонов");
    let elapsed = started.elapsed();
    assert_eq!(result.failed_runs, 0);
    #[cfg(debug_assertions)]
    let budget = std::time::Duration::from_secs(30);
    #[cfg(not(debug_assertions))]
    let budget = std::time::Duration::from_secs(1);
    assert!(
        elapsed <= budget,
        "10^4 прогонов эталона №5 за {elapsed:?} (бюджет {budget:?}; \
         release-норма FR-066 — < 1 с / 4 ядра)"
    );
}
