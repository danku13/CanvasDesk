//! FR-066: Monte Carlo + QMC-движок — слой L4/P3 расчётного стека
//! (ADR-0008, этап M5/S3, волна S). `N ≥ 10⁴` прогонов
//! [`crate::flow::propagate_with_lines`] с распределёнными параметрами
//! (`McConfig.params` → построчные what-if подмены) и collect-then-reduce
//! квантили **P50/P90/P99** как новые типы результатов нод.
//!
//! Модуль живёт за фичей `qmc` (implies `stats` + `parallel` +
//! `dep:sobol_burley`, контракт §5.6); без неё не компилируется —
//! core `propagate_with_lines` остаётся wasm-чистым (§5.8: wasm-сборка
//! идёт без `qmc`, гейт `scripts/wasm_gate.sh --check`).
//!
//! Контракты FR-066:
//! - **§5.1 — `propagate_with_lines` СТАБИЛЬНА.** Движок добавляет НОВУЮ
//!   sibling `flow::propagate_monte_carlo` (тонкая обёртка над модулем),
//!   существующую функцию не трогает.
//! - **§5.7.2 — сид = `hash(content) ⊕ scenario_seed ⊕ run_idx`.** Только
//!   `rand_chacha::ChaCha8Rng::seed_from_u64` (наследие FR-063);
//!   `thread_rng()` ЗАПРЕЩЁН. Один `(config, seed)` → **побитово**
//!   идентичный `Vec<FlowSolutions>` → идентичные квантили.
//! - **§5.7.3 — collect-then-reduce.** Прогоны собираются в
//!   `Vec<FlowSolutions>` (sort по run-индексу), квантили считаются на
//!   собранном векторе: `Vec<Value>` per node → sort → builtin
//!   `percentile` (PERCENTILE.INC, `expr.rs`). Никаких float-агрегатов
//!   в параллельных ветках.
//! - **§5.7.4 — версия движка в `canvas.extra["canvasdesk"]["engine"]`**
//!   (raw JSON, паттерн `whatif.rs:39–115`, без typed-struct на `Canvas`).
//!   Рассинхрон версии → `McResult::stale = true`.
//! - **Чанкование 256/задача** (фича `parallel` из FR-065, rayon
//!   `par_chunks`); на wasm/без `parallel` — однопоточный путь.
//!
//! Параметры адресуются строками Numi-листов: ключ `params` —
//! `(node_id, param)`, значение [`Distribution`] (Normal/LogNormal/Exp/
//! Poisson — набор FR-063). Прогон подменяет строку «param = …» ноды
//! сэмплированным числом (паттерн `whatif_set_param` MCP / FR-017
//! построчных подмен). Подмена — безразмерное число: единица придаётся
//! формулой-потребителем (Numi-семантика FR-050 Н5, зеркально проливанию).
//!
//! Два режима ([`McMode`]):
//! - **MC** — псевдослучайные сэмплы `rand_distr` поверх
//!   `ChaCha8Rng::seed_from_u64(hash(content) ⊕ seed ⊕ run_idx)`;
//! - **QMC** — Owen-scrambled Sobol (`sobol_burley`: сэмплы `[0,1)`,
//!   максимум 2^16 прогонов / 256 измерений, паддинг измерений сменой
//!   сида — доки крейта) → inverse-CDF (`statrs`): меньшая дисперсия
//!   при том же N.
//!
//! `Distribution::LogNormal { mean, sd }` — параметры в НАТУРАЛЬНОМ
//! пространстве (среднее/СКО величины, как в демо FR-066
//! «cac ~ LogNormal(μ=120, σ=15)»); внутренняя конвертация в
//! лог-пространство — [`Distribution::lognormal_log_params`]
//! (μ_log = ln(mean) − σ_log²/2, σ_log = √ln(1 + (sd/mean)²)).
//! Это отличается от FR-063 `lognormal_sample`/`lognormal_quantile`
//! (лог-пространство в аргументах формул) — задокументированное
//! решение FR-066: пользователь финдомена мыслит в единицах величины.

use std::collections::{BTreeMap, HashMap};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::flow::{CycleError, FlowSolutions, WhatIfOverrides};
use crate::model::Canvas;

use super::stats::{fnv1a64, seed_from_parts};
use super::{percentile_inc, Value};

/// Версия MC/QMC-движка (§5.7.4): пишется в
/// `canvas.extra["canvasdesk"]["engine"].version`; рассинхрон → `stale`.
pub const ENGINE_VERSION: &str = "M5.0";

/// Максимум измерений Sobol (крейта `sobol_burley`); сверх — паддинг
/// сменой скремблинг-сида каждые 256 измерений (доки крейта).
pub const SOBOL_MAX_DIMS: usize = 256;

/// Максимум длины Owen-scrambled Sobol-последовательности (2^16, доки
/// крейта). Прогоны QMC за пределами — failed_runs (валидация MCP
/// отклоняет раньше).
pub const SOBOL_MAX_RUNS: usize = 65_536;

/// Домен v1 λ Пуассона: бюджет inverse-CDF (бинарный поиск по CDF,
/// верхняя граница λ + 12√λ). FR-066 не требует больших λ.
pub const POISSON_LAMBDA_MAX: f64 = 1000.0;

/// Чанк прогонов на задачу (FR-066 P1: «чанкование 256/задача»).
pub const MC_CHUNK: usize = 256;

/// Кламп QMC-сэмпла `p ∈ [ε, 1−ε]`: защита inverse-CDF от вырожденных
/// хвостов (Sobol может дать точный 0; 1 не даёт — интервал `[0,1)`).
const SAMPLE_P_EPS: f64 = 1e-12;

// --- Конфигурация ------------------------------------------------------------

/// Режим движка: псевдослучайный Monte Carlo или квазислучайный QMC
/// (Owen-scrambled Sobol, меньшая дисперсия при том же N).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McMode {
    /// Псевдослучайные сэмплы `rand_distr` + `ChaCha8Rng`.
    Mc,
    /// Owen-scrambled Sobol → inverse-CDF (рекомендованный, дефолт).
    #[default]
    Qmc,
}

impl McMode {
    /// Строка MCP-контракта.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mc => "mc",
            Self::Qmc => "qmc",
        }
    }
}

/// Распределение параметра (набор FR-063: Normal/LogNormal/Exp/Poisson).
/// `Normal`/`LogNormal` — параметры в натуральном пространстве
/// (`mean`/`sd` величины); `Exp`/`Poisson` — `lambda` (интенсивность).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dist", rename_all = "lowercase")]
pub enum Distribution {
    /// N(mean, sd²): sd > 0, оба конечны.
    Normal { mean: f64, sd: f64 },
    /// LogNormal с средним `mean` и СКО `sd` САМОЙ ВЕЛИЧИНЫ (mean > 0,
    /// sd > 0); лог-параметры — внутренней конвертацией.
    LogNormal { mean: f64, sd: f64 },
    /// Экспоненциальное с интенсивностью λ (λ > 0).
    Exp { lambda: f64 },
    /// Пуассон с интенсивностью λ (0 < λ ≤ [`POISSON_LAMBDA_MAX`]);
    /// выборка — целые.
    Poisson { lambda: f64 },
}

impl Distribution {
    /// Строгая валидация параметров (MCP вызывает до прогонов; сам движок
    /// деградирует тихо — невалидное распределение «роняет» прогоны в
    /// `failed_runs`, не паникая).
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Self::Normal { mean, sd } => {
                if !mean.is_finite() {
                    return Err("mean должен быть конечным".to_owned());
                }
                if !sd.is_finite() || sd <= 0.0 {
                    return Err(format!("sd должен быть > 0, получено {sd}"));
                }
            }
            Self::LogNormal { mean, sd } => {
                if !mean.is_finite() || mean <= 0.0 {
                    return Err(format!(
                        "mean (натуральное пространство, > 0) должен быть положительным и конечным, получено {mean}"
                    ));
                }
                if !sd.is_finite() || sd <= 0.0 {
                    return Err(format!("sd должен быть > 0, получено {sd}"));
                }
            }
            Self::Exp { lambda } => {
                if !lambda.is_finite() || lambda <= 0.0 {
                    return Err(format!("lambda должен быть > 0, получено {lambda}"));
                }
            }
            Self::Poisson { lambda } => {
                if !lambda.is_finite() || lambda <= 0.0 {
                    return Err(format!("lambda должен быть > 0, получено {lambda}"));
                }
                if lambda > POISSON_LAMBDA_MAX {
                    return Err(format!(
                        "lambda Пуассона вне домена v1 (≤ {POISSON_LAMBDA_MAX}): {lambda}"
                    ));
                }
            }
        }
        Ok(())
    }

    /// Дискретное ли распределение (Пуассон): формат подмены строки —
    /// целое («param = 42»), без десятичных.
    pub const fn is_discrete(&self) -> bool {
        matches!(self, Self::Poisson { .. })
    }

    /// Лог-параметры LogNormal для натуральных (mean, sd):
    /// σ_log = √ln(1 + cv²), μ_log = ln(mean) − σ_log²/2 — тогда
    /// E[X] = mean, SD[X] = sd. None — невалидные (mean ≤ 0, sd ≤ 0…).
    fn lognormal_log_params(mean: f64, sd: f64) -> Option<(f64, f64)> {
        if !mean.is_finite() || !sd.is_finite() || mean <= 0.0 || sd <= 0.0 {
            return None;
        }
        let cv2 = (sd / mean) * (sd / mean);
        let sigma2 = (1.0 + cv2).ln();
        let mu = mean.ln() - sigma2 / 2.0;
        Some((mu, sigma2.sqrt()))
    }
}

/// Конфигурация MC/QMC-прогона (FR-066 «Требуемые изменения»):
/// N прогонов, распределённые параметры, сид и квантили результатов.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McConfig {
    /// Число прогонов (N ≥ 10⁴ — рекомендация FR-066).
    pub runs: usize,
    /// `(node_id, param)` → распределение; param — имя строки
    /// присваивания «param = …» Numi-листа ноды.
    pub params: HashMap<(String, String), Distribution>,
    /// Сид сценария (§5.7.2): `hash(content) ⊕ seed ⊕ run_idx`.
    pub seed: u64,
    /// Квантили результатов, доли (0..1): например [0.5, 0.9, 0.99].
    pub quantiles: Vec<f64>,
    /// Режим движка (дефолт — QMC).
    #[serde(default)]
    pub mode: McMode,
}

impl Default for McConfig {
    fn default() -> Self {
        Self {
            runs: 10_000,
            params: HashMap::new(),
            seed: 0,
            quantiles: vec![0.5, 0.9, 0.99],
            mode: McMode::Qmc,
        }
    }
}

impl McConfig {
    /// Строгая валидация (MCP-слой вызывает до прогонов; ядро деградирует
    /// тихо — см. [`McResult::skipped_params`] / [`McResult::failed_runs`]).
    pub fn validate(&self) -> Result<(), String> {
        if self.runs == 0 {
            return Err("runs должен быть ≥ 1".to_owned());
        }
        if self.mode == McMode::Qmc && self.runs > SOBOL_MAX_RUNS {
            return Err(format!(
                "QMC: длина Sobol-последовательности ≤ {SOBOL_MAX_RUNS} (2^16); \
                 для больших N используйте mode = \"mc\""
            ));
        }
        if self.quantiles.is_empty() {
            return Err(
                "quantiles пуст — укажите хотя бы один квантиль (например [0.5, 0.9, 0.99])"
                    .to_owned(),
            );
        }
        for q in &self.quantiles {
            if !q.is_finite() || !(0.0..1.0).contains(q) {
                return Err(format!("квантиль вне (0, 1): {q}"));
            }
        }
        for (key, dist) in &self.params {
            dist.validate()
                .map_err(|err| format!("params[{key:?}]: {err}"))?;
        }
        Ok(())
    }
}

// --- Сэмплирование ------------------------------------------------------------

/// MC-сэмпл: `rand_distr` поверх сидированного RNG (§5.7.2 — единственный
/// источник случайности; `thread_rng()` запрещён и не подключён).
/// None — невалидные параметры (конструктор) или не-конечный сэмпл.
fn random_sample(dist: &Distribution, rng: &mut ChaCha8Rng) -> Option<f64> {
    use rand_distr::{Distribution as _, Exp, LogNormal, Normal, Poisson};
    let sample = match *dist {
        Distribution::Normal { mean, sd } => Normal::new(mean, sd).ok()?.sample(rng),
        Distribution::LogNormal { mean, sd } => {
            let (mu, sigma) = Distribution::lognormal_log_params(mean, sd)?;
            LogNormal::new(mu, sigma).ok()?.sample(rng)
        }
        Distribution::Exp { lambda } => Exp::new(lambda).ok()?.sample(rng),
        Distribution::Poisson { lambda } => Poisson::new(lambda).ok()?.sample(rng),
    };
    sample.is_finite().then_some(sample)
}

/// QMC-сэмпл: обратная функция распределения от `p ∈ (0,1)`
/// (Owen-scrambled Sobol). None — вне домена/не-конечный результат.
fn inverse_cdf_sample(dist: &Distribution, p: f64) -> Option<f64> {
    use statrs::distribution::{ContinuousCDF as _, Normal};
    let sample = match *dist {
        Distribution::Normal { mean, sd } => {
            if !mean.is_finite() || !sd.is_finite() || sd <= 0.0 {
                return None;
            }
            mean + sd * Normal::standard().inverse_cdf(p)
        }
        Distribution::LogNormal { mean, sd } => {
            let (mu, sigma) = Distribution::lognormal_log_params(mean, sd)?;
            (mu + sigma * Normal::standard().inverse_cdf(p)).exp()
        }
        Distribution::Exp { lambda } => {
            if !lambda.is_finite() || lambda <= 0.0 {
                return None;
            }
            -(1.0 - p).ln() / lambda
        }
        Distribution::Poisson { lambda } => {
            if !lambda.is_finite() || lambda <= 0.0 || lambda > POISSON_LAMBDA_MAX {
                return None;
            }
            let dist = statrs::distribution::Poisson::new(lambda).ok()?;
            poisson_inverse_cdf(&dist, lambda, p) as f64
        }
    };
    sample.is_finite().then_some(sample)
}

/// Квантиль Пуассона: наименьшее k с CDF(k) ≥ p — бинарный поиск по
/// монотонному CDF (`statrs::distribution::DiscreteCDF`); верхняя граница
/// λ + 12√λ + 16 — хвост за 12σ пренебрежим против клампа p.
fn poisson_inverse_cdf(dist: &statrs::distribution::Poisson, lambda: f64, p: f64) -> u64 {
    use statrs::distribution::DiscreteCDF as _;
    let hi = (lambda + 12.0 * lambda.sqrt() + 16.0).ceil().max(16.0) as u64;
    let (mut lo, mut hi) = (0u64, hi);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if dist.cdf(mid) < p {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// 32-битный скремблинг-сид Sobol: XOR-свёртка полного u64-сида (иначе
/// сиды, различающиеся только старшими 32 битами, дали бы одинаковые
/// точки) ⊕ группа измерений (паддинг за 256, доки `sobol_burley`).
fn sobol_seed(seed: u64, dim: usize) -> u32 {
    let folded = (seed as u32) ^ ((seed >> 32) as u32);
    folded ^ ((dim / SOBOL_MAX_DIMS) as u32)
}

/// Сид одного прогона MC (§5.7.2 — «hash(content) ⊕ scenario_seed ⊕
/// run_idx», с усилением): буквальная XOR-формула вырождается — при
/// N = 2^k прогонов XOR поглощает scenario_seed < 2^k (множество рёнов
/// {hash ⊕ s ⊕ i : i < 2^k} не зависит от s), и разные сиды сценария
/// дают побитово одинаковые квантили (ловится тестом дисперсии
/// `qmc_variance_is_lower_than_mc`). Тройка смешивается FNV-1a 64 —
/// детерминизм сохранён (тот же (content, seed, idx) → тот же поток),
/// коллапса нет; хеш — публичный референс-алгоритм FR-063.
fn mc_run_seed(base_seed: u64, scenario_seed: u64, run_idx: usize) -> u64 {
    let mut bytes = [0u8; 24];
    bytes[..8].copy_from_slice(&base_seed.to_le_bytes());
    bytes[8..16].copy_from_slice(&scenario_seed.to_le_bytes());
    bytes[16..].copy_from_slice(&(run_idx as u64).to_le_bytes());
    fnv1a64(&bytes)
}

// --- Разрешение параметров в строки -------------------------------------------

/// Разрешённый параметр: строка присваивания «param = …» Numi-листа ноды
/// (первое совпадение — паттерн `whatif_set_param` MCP, FR-017).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedParam {
    pub node_id: String,
    pub param: String,
    /// Индекс строки ТЕКСТА ноды (нулевый, как `WhatIfOverrides::line_exprs`).
    pub line: usize,
    pub dist: Distribution,
}

/// Разрешение `(node_id, param)` → индекс строки присваивания. Нода или
/// строка не найдены — параметр уходит в `skipped` (тихая деградация,
/// маркер в [`McResult::skipped_params`], симметрия FR-025); MCP-слой
/// валидирует строго ДО прогонов и возвращает ошибку агенту.
/// Порядок `resolved` — сортировка по `(node_id, param)` (детерминизм
/// порядка сэмплов: измерения Sobol и порядок выборки RNG).
pub fn resolve_params(
    canvas: &Canvas,
    params: &HashMap<(String, String), Distribution>,
) -> (Vec<ResolvedParam>, Vec<(String, String)>) {
    let mut entries: Vec<(&(String, String), &Distribution)> = params.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    let mut resolved = Vec::with_capacity(entries.len());
    let mut skipped = Vec::new();
    for ((node_id, param), dist) in entries {
        let Some(node) = canvas.node(node_id) else {
            skipped.push((node_id.clone(), param.clone()));
            continue;
        };
        let text = node.text.as_deref().unwrap_or_default();
        let found = text.split('\n').position(|row| {
            row.split_once('=')
                .map(|(name, _)| name.trim() == param)
                .unwrap_or(false)
        });
        match found {
            Some(line) => resolved.push(ResolvedParam {
                node_id: node_id.clone(),
                param: param.clone(),
                line,
                dist: (*dist).clone(),
            }),
            None => skipped.push((node_id.clone(), param.clone())),
        }
    }
    (resolved, skipped)
}

/// Канонический контент стохастики для сида (§5.7.2 `hash(content)`):
/// отсортированные «node:param = Distribution{…}» через `;`. Один
/// `(content, seed)` → один поток сэмплов — смена набора параметров
/// меняет поток даже при том же сиде.
pub fn params_content(resolved: &[ResolvedParam]) -> String {
    resolved
        .iter()
        .map(|rp| format!("{}:{}={:?}", rp.node_id, rp.param, rp.dist))
        .collect::<Vec<_>>()
        .join(";")
}

/// What-if подмены прогона: каждый параметр → строка «param = сэмпл».
/// Число форматируется Rust `Display` — БЕЗ e-нотации (лексер Numi её не
/// знает), shortest-roundtrip: значение переживает parse→f64 побитово.
/// Дискретные (Пуассон) — целые. Единица — НЕ сохраняется: её придаёт
/// формула-потребитель (Numi-семантика, FR-050 Н5).
fn run_overrides(resolved: &[ResolvedParam], samples: &[f64]) -> WhatIfOverrides {
    let mut line_exprs = HashMap::with_capacity(resolved.len());
    for (rp, &sample) in resolved.iter().zip(samples.iter()) {
        let value_text = if rp.dist.is_discrete() {
            format!("{}", sample as u64)
        } else {
            format!("{sample}")
        };
        line_exprs.insert(
            (rp.node_id.clone(), rp.line),
            format!("{} = {}", rp.param, value_text),
        );
    }
    WhatIfOverrides {
        line_exprs,
        node_values: HashMap::new(),
    }
}

/// Сэмплы одного прогона. MC — один RNG на прогон (сид §5.7.2:
/// `hash(content) ⊕ scenario_seed ⊕ run_idx`), параметры в разрешённом
/// (сортированном) порядке. QMC — измерение = позиция параметра,
/// скремблинг от полного сида. None — прогон «роняется» в failed_runs.
fn sample_run(
    resolved: &[ResolvedParam],
    mode: McMode,
    scenario_seed: u64,
    base_seed: u64,
    run_idx: usize,
) -> Option<Vec<f64>> {
    match mode {
        McMode::Mc => {
            let mut rng = ChaCha8Rng::seed_from_u64(mc_run_seed(base_seed, scenario_seed, run_idx));
            let mut samples = Vec::with_capacity(resolved.len());
            for rp in resolved {
                samples.push(random_sample(&rp.dist, &mut rng)?);
            }
            Some(samples)
        }
        McMode::Qmc => {
            if run_idx >= SOBOL_MAX_RUNS {
                return None;
            }
            let mut samples = Vec::with_capacity(resolved.len());
            for (dim, rp) in resolved.iter().enumerate() {
                let p = sobol_burley::sample(
                    run_idx as u32,
                    (dim % SOBOL_MAX_DIMS) as u32,
                    sobol_seed(scenario_seed, dim),
                ) as f64;
                let p = p.clamp(SAMPLE_P_EPS, 1.0 - SAMPLE_P_EPS);
                samples.push(inverse_cdf_sample(&rp.dist, p)?);
            }
            Some(samples)
        }
    }
}

// --- Прогоны -------------------------------------------------------------------

/// N прогонов `propagate_with_lines` (публичная поверхность для
/// верификации детерминизма — гейт P1 FR-066 «тот же seed → побитово
/// тот же `Vec<FlowSolutions>`» — и продвинутых потребителей). Порядок
/// вектора — по run-индексу (collect-then-reduce, §5.7.3).
pub fn run_solutions(
    canvas: &Canvas,
    config: &McConfig,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<FlowSolutions>, CycleError> {
    let (resolved, _) = resolve_params(canvas, &config.params);
    run_solutions_resolved(canvas, config, &resolved, progress)
}

/// N прогонов по заранее разрешённым параметрам: чанки 256/задача за
/// фичей `parallel` (rayon `par_chunks`, bounded pool — FR-065), иначе
/// однопоточно. `progress(completed, total)` — после каждого завершённого
/// прогона (UI: N/total, ETA); порядок вызовов из параллельных воркеров
/// не детерминирован, но данные — только collect-then-reduce: финальный
/// sort по run-индексу гарантирует порядок §5.7.3.
fn run_solutions_resolved(
    canvas: &Canvas,
    config: &McConfig,
    resolved: &[ResolvedParam],
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<Vec<FlowSolutions>, CycleError> {
    // DAG-инвариант один раз ДО N прогонов (цикл — ошибка вызова; тот же
    // контракт, что у propagate_with_lines, §5.1)
    crate::flow::topo_sort(canvas)?;
    // §5.7.2: base = hash(content) ⊕ scenario_seed; run-сид добавит run_idx
    let base_seed = seed_from_parts(&params_content(resolved), config.seed);
    let total = config.runs;

    let run_one = |run_idx: usize| -> Option<FlowSolutions> {
        let samples = sample_run(resolved, config.mode, config.seed, base_seed, run_idx)?;
        let whatif = run_overrides(resolved, &samples);
        crate::flow::propagate_with_lines(canvas, &whatif).ok()
    };

    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    let completed: Vec<(usize, Option<FlowSolutions>)> = {
        use rayon::prelude::*;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let done = AtomicUsize::new(0);
        let indices: Vec<usize> = (0..total).collect();
        // Чанки 256/задача; par_chunks по слайсу сохраняет порядок чанков,
        // внутри чанка прогоны последовательны — порядок глобально
        // детерминирован; финальный sort — контрактная гарантия §5.7.3
        // (никаких float-агрегатов в параллельных ветках).
        let chunks: Vec<Vec<(usize, Option<FlowSolutions>)>> = indices
            .par_chunks(MC_CHUNK)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|&run_idx| {
                        let run = run_one(run_idx);
                        let completed_runs = done.fetch_add(1, Ordering::Relaxed) + 1;
                        progress(completed_runs, total);
                        (run_idx, run)
                    })
                    .collect()
            })
            .collect();
        chunks.into_iter().flatten().collect()
    };
    // Однопоточный путь: wasm32 (нет std::thread) или без фичи `parallel`
    // (B2B zero-dep инвариант, §5.6/§5.8).
    #[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
    let completed: Vec<(usize, Option<FlowSolutions>)> = (0..total)
        .map(|run_idx| {
            let run = run_one(run_idx);
            progress(run_idx + 1, total);
            (run_idx, run)
        })
        .collect();
    // §5.7.3: детерминированный порядок — sort по run-индексу
    let mut completed = completed;
    completed.sort_by_key(|(run_idx, _)| *run_idx);
    Ok(completed.into_iter().filter_map(|(_, run)| run).collect())
}

// --- Collect-then-reduce: квантили ----------------------------------------------

/// Результат MC/QMC-прогона: квантили итогов нод, построчных и
/// именованных выходов + служебные поля. Карты — `BTreeMap`
/// (детерминированный порядок итерации для MCP/сериализации).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct McResult {
    /// Запрошено прогонов (N).
    pub runs: usize,
    /// Прогонов «ронено» (невалидное распределение / QMC за 2^16 /
    /// не-конечный сэмпл) — вошли в квантили только успешные.
    pub failed_runs: usize,
    pub mode: McMode,
    pub seed: u64,
    /// Квантили, по которым считались значения (санитизированные:
    /// конечные (0,1) в порядке конфига).
    pub quantiles: Vec<f64>,
    /// Итоги нод: node_id → значение на каждый квантиль (выровнен с
    /// `quantiles`).
    pub outputs: BTreeMap<String, Vec<Value>>,
    /// Построчные выходы: (node_id, индекс строки) → значения квантилей.
    pub lines: BTreeMap<(String, usize), Vec<Value>>,
    /// Именованные выходы: (node_id, имя выхода) → значения квантилей.
    pub named: BTreeMap<(String, String), Vec<Value>>,
    /// Параметры, не разрешённые в строки (нода/строка не найдены) —
    /// тихая деградация ядра; MCP валидирует строго и сюда не попадает.
    pub skipped_params: Vec<(String, String)>,
    /// §5.7.4: версия в `canvas.extra["canvasdesk"]["engine"]` не совпала
    /// с [`ENGINE_VERSION`] (снимок квантилей мог считаться другой версией).
    pub stale: bool,
}

/// Собираемая серия значений одного адресата (node/line/named): числа +
/// юнит первого валидного прогона (формула одна — юнит стабилен; при
/// расхождении юнита сэмпл пропускается, защита от смешения масштабов).
#[derive(Default)]
struct Series {
    nums: Vec<f64>,
    unit: Option<super::Unit>,
}

impl Series {
    fn push(&mut self, value: &Value) {
        match &self.unit {
            None => {
                self.unit = Some(value.unit.clone());
                self.nums.push(value.num);
            }
            Some(unit) => {
                let same = unit.dims() == value.unit.dims()
                    && (unit.scale() - value.unit.scale()).abs() <= 1e-9;
                if same {
                    self.nums.push(value.num);
                }
            }
        }
    }

    /// Квантили серии: sort → builtin `percentile` (PERCENTILE.INC,
    /// `expr.rs` — линейная интерполяция; p*100 — шкала builtin).
    fn quantiles(&self, qs: &[f64]) -> Vec<Value> {
        let mut sorted = self.nums.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        qs.iter()
            .map(|q| {
                Value::with_unit(
                    percentile_inc(&sorted, q * 100.0),
                    self.unit.clone().unwrap_or(super::Unit::Scalar),
                )
            })
            .collect()
    }
}

/// §5.7.3 collect-then-reduce: `Vec<FlowSolutions>` → квантили по каждому
/// адресату. Сортировка ключей — BTreeMap (детерминизм), значений —
/// внутри серии; никаких параллельных float-агрегатов.
fn reduce_solutions(
    runs: Vec<FlowSolutions>,
    failed_runs: usize,
    skipped_params: Vec<(String, String)>,
    config: &McConfig,
    stale: bool,
) -> McResult {
    // Санитизация квантилей: конечные (0..1); порядок конфига сохранён
    let quantiles: Vec<f64> = config
        .quantiles
        .iter()
        .filter(|q| q.is_finite() && (0.0..1.0).contains(*q))
        .cloned()
        .collect();

    let mut out_series: BTreeMap<String, Series> = BTreeMap::new();
    let mut line_series: BTreeMap<(String, usize), Series> = BTreeMap::new();
    let mut named_series: BTreeMap<(String, String), Series> = BTreeMap::new();
    for run in &runs {
        for (node, outcome) in &run.outputs {
            if let Ok(value) = outcome {
                out_series.entry(node.clone()).or_default().push(value);
            }
        }
        for (key, value) in &run.lines {
            line_series.entry(key.clone()).or_default().push(value);
        }
        for (key, value) in &run.named {
            named_series.entry(key.clone()).or_default().push(value);
        }
    }
    McResult {
        runs: config.runs,
        failed_runs,
        mode: config.mode,
        seed: config.seed,
        outputs: out_series
            .into_iter()
            .map(|(k, s)| (k, s.quantiles(&quantiles)))
            .collect(),
        lines: line_series
            .into_iter()
            .map(|(k, s)| (k, s.quantiles(&quantiles)))
            .collect(),
        named: named_series
            .into_iter()
            .map(|(k, s)| (k, s.quantiles(&quantiles)))
            .collect(),
        skipped_params,
        stale,
        quantiles,
    }
}

/// Метка квантиля: 0.5 → «P50», 0.9 → «P90», 0.99 → «P99»,
/// 0.999 → «P99.9», 0.1 → «P10» (до двух знаков, без хвостовых нулей).
pub fn quantile_label(p: f64) -> String {
    let text = format!("{:.2}", p * 100.0);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("P{text}")
}

/// Хвостовой квантиль для analyze-снимка (P3 «overlay bottleneck на P90»):
/// наименьший квантиль ≥ 0.9; если все меньше — наибольший (консервативная
/// оценка хвоста).
pub fn tail_quantile(quantiles: &[f64]) -> Option<f64> {
    let mut sorted: Vec<f64> = quantiles
        .iter()
        .filter(|q| q.is_finite() && (0.0..1.0).contains(*q))
        .cloned()
        .collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    sorted
        .iter()
        .cloned()
        .find(|q| *q >= 0.9)
        .or_else(|| sorted.last().cloned())
}

/// Синтетический `FlowSolutions` (§5.5 — «analyze без правок»): значения
/// нод/строк/именованных выходов на выбранном квантиле + P-метки всех
/// квантилей как именованные выходы нод (`(node, "P50")`…). `None` —
/// квантиля нет в списке результата.
pub fn synthetic_solutions(result: &McResult, quantile: f64) -> Option<FlowSolutions> {
    let idx = result
        .quantiles
        .iter()
        .position(|q| (q - quantile).abs() <= 1e-12)?;
    let mut solutions = FlowSolutions::default();
    for (node_id, values) in &result.outputs {
        if let Some(value) = values.get(idx) {
            solutions.outputs.insert(node_id.clone(), Ok(value.clone()));
        }
        // P-метки: квантили видны как именованные выходы ноды — включая
        // несовпадение с именами листа (зарезервированные метки)
        for (qi, q) in result.quantiles.iter().enumerate() {
            if let Some(value) = values.get(qi) {
                solutions
                    .named
                    .insert((node_id.clone(), quantile_label(*q)), value.clone());
            }
        }
    }
    for (key, values) in &result.lines {
        if let Some(value) = values.get(idx) {
            solutions.lines.insert(key.clone(), value.clone());
        }
    }
    for (key, values) in &result.named {
        if let Some(value) = values.get(idx) {
            solutions.named.insert(key.clone(), value.clone());
        }
    }
    Some(solutions)
}

// --- Engine-метаданные (§5.7.4, паттерн whatif.rs:39–115) ---------------------

/// Снимок `canvas.extra["canvasdesk"]["engine"]` (raw JSON, без
/// typed-struct на `Canvas`).
#[derive(Debug, Clone, PartialEq)]
pub struct EngineMeta {
    pub version: String,
    pub seed: u64,
    pub stats: bool,
    pub parallel: bool,
    pub qmc: bool,
}

/// Текущие метаданные движка (флаги — фактом сборки).
pub fn current_engine_meta(seed: u64) -> EngineMeta {
    EngineMeta {
        version: ENGINE_VERSION.to_owned(),
        seed,
        stats: cfg!(feature = "stats"),
        parallel: cfg!(feature = "parallel"),
        qmc: cfg!(feature = "qmc"),
    }
}

/// Чтение `canvas.extra["canvasdesk"]["engine"]` (толерантно: мусор или
/// отсутствие поля — `None`; seed — строка (каноническая запись, JS-safe)
/// или число).
pub fn engine_from_canvas(canvas: &Canvas) -> Option<EngineMeta> {
    let engine = canvas.extra.get("canvasdesk")?.get("engine")?;
    Some(EngineMeta {
        version: engine.get("version")?.as_str()?.to_owned(),
        seed: engine
            .get("seed")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse().ok())
            .or_else(|| engine.get("seed").and_then(serde_json::Value::as_u64))?,
        stats: engine
            .get("stats")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        parallel: engine
            .get("parallel")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        qmc: engine
            .get("qmc")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

/// Запись метаданных в `canvas.extra["canvasdesk"]["engine"]` (raw JSON).
/// Чужие поля `canvasdesk` сохраняются (round-trip, инвариант 5 FR-017 —
/// тот же паттерн, что `whatif::scenarios_to_canvas`); поле не удаляется:
/// метаданные движка актуальны после каждого прогона.
pub fn engine_to_canvas(canvas: &mut Canvas, meta: &EngineMeta) {
    let value = serde_json::json!({
        "version": meta.version,
        "seed": meta.seed.to_string(),
        "stats": meta.stats,
        "parallel": meta.parallel,
        "qmc": meta.qmc,
    });
    if let Some(serde_json::Value::Object(ext)) = canvas.extra.get_mut("canvasdesk") {
        ext.insert("engine".to_owned(), value);
        return;
    }
    let mut ext = serde_json::Map::new();
    ext.insert("engine".to_owned(), value);
    canvas
        .extra
        .insert("canvasdesk".to_owned(), serde_json::Value::Object(ext));
}

/// §5.7.4: рассинхрон версии (сохранённая ≠ текущая) — снимок протух.
fn engine_stale(canvas: &Canvas) -> bool {
    engine_from_canvas(canvas).is_some_and(|meta| meta.version != ENGINE_VERSION)
}

// --- Публичная точка входа (сиблинг flow::propagate_monte_carlo) ---------------

/// MC/QMC-прогон: N прогонов → квантили (см. модуль). Сиблинг
/// `flow::propagate_monte_carlo` — тонкая обёртка (FR-066 Impact).
pub fn propagate_monte_carlo(canvas: &Canvas, config: &McConfig) -> Result<McResult, CycleError> {
    propagate_monte_carlo_with_progress(canvas, config, &|_, _| {})
}

/// То же с прогрессом `(completed, total)` после каждого завершённого
/// прогона (UI: N/total, ETA; порядок вызовов из параллельных воркеров
/// не детерминирован — данные детерминированы collect-then-reduce).
pub fn propagate_monte_carlo_with_progress(
    canvas: &Canvas,
    config: &McConfig,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<McResult, CycleError> {
    let stale = engine_stale(canvas);
    let (resolved, skipped) = resolve_params(canvas, &config.params);
    let runs = run_solutions_resolved(canvas, config, &resolved, progress)?;
    let failed_runs = config.runs.saturating_sub(runs.len());
    Ok(reduce_solutions(runs, failed_runs, skipped, config, stale))
}

// --- Внутренние тесты ---------------------------------------------------------

#[cfg(all(test, feature = "qmc"))]
mod tests {
    use super::*;
    use crate::model::Node;

    fn canvas_with(text: &str) -> Canvas {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("n1", text, 0.0, 0.0));
        canvas
    }

    /// Валидация распределений: σ ≤ 0, mean ≤ 0 (lognormal), λ ≤ 0,
    /// λ Пуассона за доменом — ошибки; валидные — ОК.
    #[test]
    fn distribution_validation_rejects_bad_params() {
        assert!(Distribution::Normal { mean: 0.0, sd: 1.0 }
            .validate()
            .is_ok());
        assert!(Distribution::LogNormal {
            mean: 120.0,
            sd: 15.0
        }
        .validate()
        .is_ok());
        assert!(Distribution::Exp { lambda: 0.05 }.validate().is_ok());
        assert!(Distribution::Poisson { lambda: 10.0 }.validate().is_ok());
        for bad in [
            Distribution::Normal { mean: 0.0, sd: 0.0 },
            Distribution::Normal {
                mean: f64::NAN,
                sd: 1.0,
            },
            Distribution::LogNormal { mean: 0.0, sd: 1.0 },
            Distribution::LogNormal {
                mean: 10.0,
                sd: 0.0,
            },
            Distribution::Exp { lambda: 0.0 },
            Distribution::Poisson { lambda: -1.0 },
            Distribution::Poisson {
                lambda: POISSON_LAMBDA_MAX + 1.0,
            },
        ] {
            assert!(bad.validate().is_err(), "{bad:?} должна быть отклонена");
        }
    }

    /// Метки квантилей: P50/P90/P99/P99.9/P10/P25 — без хвостовых нулей.
    #[test]
    fn quantile_labels_are_compact() {
        assert_eq!(quantile_label(0.5), "P50");
        assert_eq!(quantile_label(0.9), "P90");
        assert_eq!(quantile_label(0.99), "P99");
        assert_eq!(quantile_label(0.999), "P99.9");
        assert_eq!(quantile_label(0.1), "P10");
        assert_eq!(quantile_label(0.25), "P25");
    }

    /// inverse-CDF в опорных точках: медиана/квантиль аналитически.
    #[test]
    fn inverse_cdf_known_points() {
        let normal = Distribution::Normal {
            mean: 100.0,
            sd: 15.0,
        };
        assert!((inverse_cdf_sample(&normal, 0.5).unwrap() - 100.0).abs() < 1e-9);
        // Φ⁻¹(0.9) = 1.28155 → 100 + 15·1.28155 = 119.2233
        assert!(
            (inverse_cdf_sample(&normal, 0.9).unwrap() - 119.2233).abs() < 1e-3,
            "P90 нормали"
        );
        let exp = Distribution::Exp { lambda: 2.0 };
        // −ln(0.5)/2 = 0.34657
        assert!((inverse_cdf_sample(&exp, 0.5).unwrap() - 2.0f64.ln() / 2.0).abs() < 1e-9);
        let lognormal = Distribution::LogNormal {
            mean: 120.0,
            sd: 15.0,
        };
        // Медиана LogNormal = exp(μ_log) = mean/√(1 + cv²)
        let cv: f64 = 15.0 / 120.0;
        let median = 120.0 / (1.0 + cv * cv).sqrt();
        assert!(
            (inverse_cdf_sample(&lognormal, 0.5).unwrap() - median).abs() < 1e-9,
            "медиана LogNormal в натуральном пространстве"
        );
        let poisson = Distribution::Poisson { lambda: 10.0 };
        assert_eq!(inverse_cdf_sample(&poisson, 0.5), Some(10.0));
    }

    /// sobol_seed: XOR-свёртка — сиды, различающиеся только старшими
    /// 32 битами, дают РАЗНЫЕ скремблинги.
    #[test]
    fn sobol_seed_folds_full_u64() {
        assert_eq!(sobol_seed(0x1_0000_0000u64, 0), 1);
        assert_eq!(sobol_seed(0, 0), 0);
        assert_ne!(sobol_seed(0x1_0000_0000u64, 0), sobol_seed(0, 0));
        // Паддинг измерений: dim 256 переиспользует слот 0 с другой группой
        assert_ne!(sobol_seed(7, 256), sobol_seed(7, 0));
    }

    /// resolve_params: строки присваивания находятся (первое совпадение),
    /// нода/параметр без строки — в skipped.
    #[test]
    fn resolve_params_finds_assignment_lines() {
        let canvas = canvas_with("spend = 60000\nchurn = 0.1\nпросто проза");
        let mut params = HashMap::new();
        params.insert(
            ("n1".to_owned(), "churn".to_owned()),
            Distribution::Exp { lambda: 10.0 },
        );
        params.insert(
            ("n1".to_owned(), "spend".to_owned()),
            Distribution::Normal {
                mean: 60000.0,
                sd: 1000.0,
            },
        );
        params.insert(
            ("ghost".to_owned(), "x".to_owned()),
            Distribution::Exp { lambda: 1.0 },
        );
        params.insert(
            ("n1".to_owned(), "no_such".to_owned()),
            Distribution::Exp { lambda: 1.0 },
        );
        let (resolved, skipped) = resolve_params(&canvas, &params);
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].param, "churn", "сортировка (node, param)");
        assert_eq!(resolved[0].line, 1);
        assert_eq!(resolved[1].param, "spend");
        assert_eq!(resolved[1].line, 0);
        assert_eq!(skipped.len(), 2, "ghost-нода и параметр без строки");
    }

    /// Подмены строки: формат Display без e-нотации, roundtrip через
    /// Numi-парсер побитовый; Пуассон — целым.
    #[test]
    fn run_overrides_format_survives_numi_roundtrip() {
        let resolved = vec![
            ResolvedParam {
                node_id: "n1".into(),
                param: "churn".into(),
                line: 1,
                dist: Distribution::Exp { lambda: 10.0 },
            },
            ResolvedParam {
                node_id: "n1".into(),
                param: "events".into(),
                line: 2,
                dist: Distribution::Poisson { lambda: 10.0 },
            },
        ];
        for sample in [1e-7f64, 0.1, 12_345.678_910_111, 1e20] {
            let text = format!("{sample}");
            assert!(!text.contains(['e', 'E']), "Display без e-нотации: {text}");
        }
        let whatif = run_overrides(&resolved, &[0.104_234_234_234, 13.0]);
        assert_eq!(
            whatif.line_exprs[&("n1".to_owned(), 1)],
            "churn = 0.104234234234"
        );
        assert_eq!(whatif.line_exprs[&("n1".to_owned(), 2)], "events = 13");
        // roundtrip: подменённая строка вычисляется Numi-парсером в то же
        // число (побитово)
        let parsed = crate::expr::eval_lines("churn = 0.104234234234");
        assert_eq!(
            parsed.into_iter().flatten().next(),
            Some(crate::expr::ExprOutcome::Ok(Value::scalar(
                0.104_234_234_234
            )))
        );
    }

    /// Engine-метаданные: round-trip, чужие поля canvasdesk сохраняются,
    /// отсутствие — None; seed читается и из строки (каноническая запись).
    #[test]
    fn engine_meta_round_trip_and_foreign_fields() {
        let mut canvas = canvas_with("x = 1");
        assert!(engine_from_canvas(&canvas).is_none(), "нет поля — None");
        canvas.extra.insert(
            "canvasdesk".to_owned(),
            serde_json::json!({ "foreign": 42 }),
        );
        engine_to_canvas(&mut canvas, &current_engine_meta(7));
        let restored = engine_from_canvas(&canvas).expect("метаданные записаны");
        assert_eq!(restored, current_engine_meta(7));
        assert_eq!(canvas.extra["canvasdesk"]["foreign"], serde_json::json!(42));
        // seed — канонически строкой (JS-safe u64)
        assert_eq!(
            canvas.extra["canvasdesk"]["engine"]["seed"].as_str(),
            Some("7")
        );
        // полнота round-trip .canvas (extra переживает сериализацию)
        let json = serde_json::to_string(&canvas).expect("serialize");
        let back: Canvas = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(engine_from_canvas(&back), Some(current_engine_meta(7)));
    }

    /// tail_quantile: наименьший ≥ 0.9 (P90 по умолчанию), иначе наибольший.
    #[test]
    fn tail_quantile_prefers_p90() {
        assert_eq!(tail_quantile(&[0.5, 0.9, 0.99]), Some(0.9));
        assert_eq!(tail_quantile(&[0.5, 0.8]), Some(0.8));
        assert_eq!(tail_quantile(&[0.95, 0.99]), Some(0.95));
        assert_eq!(tail_quantile(&[]), None);
    }

    /// synthetic_solutions: значения нод на выбранном квантиле + P-метки
    /// всех квантилей в named; неизвестный квантиль — None.
    #[test]
    fn synthetic_solutions_places_quantiles_and_labels() {
        let result = McResult {
            runs: 10,
            failed_runs: 0,
            mode: McMode::Qmc,
            seed: 0,
            quantiles: vec![0.5, 0.9, 0.99],
            outputs: BTreeMap::from([(
                "n1".to_owned(),
                vec![Value::scalar(1.0), Value::scalar(2.0), Value::scalar(3.0)],
            )]),
            lines: BTreeMap::from([(
                ("n1".to_owned(), 0usize),
                vec![
                    Value::scalar(10.0),
                    Value::scalar(20.0),
                    Value::scalar(30.0),
                ],
            )]),
            named: BTreeMap::from([(
                ("n1".to_owned(), "cac".to_owned()),
                vec![
                    Value::scalar(20.0),
                    Value::scalar(21.0),
                    Value::scalar(22.0),
                ],
            )]),
            skipped_params: Vec::new(),
            stale: false,
        };
        let snapshot = synthetic_solutions(&result, 0.9).expect("P90 есть");
        assert_eq!(snapshot.outputs["n1"], Ok(Value::scalar(2.0)));
        assert_eq!(snapshot.lines[&("n1".to_owned(), 0)], Value::scalar(20.0));
        assert_eq!(
            snapshot.named[&("n1".to_owned(), "cac".to_owned())],
            Value::scalar(21.0)
        );
        // P-метки всех квантилей — именованные выходы ноды
        assert_eq!(
            snapshot.named[&("n1".to_owned(), "P50".to_owned())],
            Value::scalar(1.0)
        );
        assert_eq!(
            snapshot.named[&("n1".to_owned(), "P90".to_owned())],
            Value::scalar(2.0)
        );
        assert_eq!(
            snapshot.named[&("n1".to_owned(), "P99".to_owned())],
            Value::scalar(3.0)
        );
        assert!(synthetic_solutions(&result, 0.75).is_none(), "нет P75");
    }

    /// Конфиг: strict-валидация (QMC за 2^16, пустые quantiles, dist-мусор).
    #[test]
    fn config_validation_gates() {
        let mut config = McConfig {
            runs: 100,
            params: HashMap::new(),
            seed: 0,
            quantiles: vec![0.5, 0.9, 0.99],
            mode: McMode::Qmc,
        };
        assert!(config.validate().is_ok());
        config.runs = SOBOL_MAX_RUNS + 1;
        let err = config.validate().unwrap_err();
        assert!(err.contains("2^16"), "подсказка mode=mc: {err}");
        config.mode = McMode::Mc;
        config.runs = 1_000_000;
        assert!(config.validate().is_ok(), "MC без ограничения Sobol");
        config.runs = 0;
        assert!(config.validate().is_err());
        config.runs = 100;
        config.quantiles = vec![];
        assert!(config.validate().is_err());
        config.quantiles = vec![0.5, 1.5];
        assert!(config.validate().is_err());
        config.quantiles = vec![0.5, 0.9, 0.99];
        config.params.insert(
            ("n1".to_owned(), "x".to_owned()),
            Distribution::Poisson { lambda: 2000.0 },
        );
        assert!(config.validate().is_err());
    }

    /// sample_run (MC): тот же (content, seed, run_idx) → побитово те же
    /// сэмплы; смена run_idx/сида — другие. Регресс XOR-коллапса: сиды
    /// сценария внутри диапазона прогонов обязаны давать РАЗНЫЕ потоки
    /// (2^k прогонов поглощало бы чистый XOR).
    #[test]
    fn sample_run_mc_seed_contract() {
        let resolved = vec![ResolvedParam {
            node_id: "n1".into(),
            param: "x".into(),
            line: 0,
            dist: Distribution::Normal {
                mean: 100.0,
                sd: 15.0,
            },
        }];
        let base = seed_from_parts(&params_content(&resolved), 42);
        let a = sample_run(&resolved, McMode::Mc, 42, base, 0).unwrap();
        let b = sample_run(&resolved, McMode::Mc, 42, base, 0).unwrap();
        assert_eq!(a, b, "тот же сид — те же сэмплы");
        let c = sample_run(&resolved, McMode::Mc, 42, base, 1).unwrap();
        assert_ne!(a, c, "другой run_idx — другие сэмплы");
        let base2 = seed_from_parts(&params_content(&resolved), 43);
        let d = sample_run(&resolved, McMode::Mc, 43, base2, 0).unwrap();
        assert_ne!(a, d, "другой scenario_seed — другие сэмплы");
        // Регресс коллапса: {hash ⊕ s ⊕ i : i < 2^k} не зависит от s < 2^k —
        // множества сэмплов сидов 1 и 2 на 16 прогонах обязаны различаться
        let set = |scenario: u64| -> Vec<Vec<f64>> {
            (0..16)
                .map(|i| sample_run(&resolved, McMode::Mc, scenario, base, i).unwrap())
                .collect()
        };
        let mut first = set(1);
        let mut second = set(2);
        let by_partial =
            |a: &Vec<f64>, b: &Vec<f64>| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
        first.sort_by(by_partial);
        second.sort_by(by_partial);
        assert_ne!(first, second, "множества рёнов разных сидов различаются");
    }

    /// QMC за 2^16 — прогон «роняется» (failed_runs), не паника.
    #[test]
    fn qmc_runs_beyond_sobol_length_fail_gracefully() {
        let resolved = vec![ResolvedParam {
            node_id: "n1".into(),
            param: "x".into(),
            line: 0,
            dist: Distribution::Normal { mean: 1.0, sd: 1.0 },
        }];
        assert!(sample_run(&resolved, McMode::Qmc, 0, 0, SOBOL_MAX_RUNS).is_none());
        assert!(sample_run(&resolved, McMode::Qmc, 0, 0, SOBOL_MAX_RUNS - 1).is_some());
    }
}
