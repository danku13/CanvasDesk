//! FR-016: индикаторы узких мест и риска очередей — чистая функция над
//! результатами propagator-а (волна B1/CP5 продуктового роадмапа).
//!
//! [`analyze`] — `(&Canvas, &FlowSolutions, &AnalysisConfig) ->
//! HashMap<node_id, AnalysisFlags>`: без I/O, без мутаций, в духе
//! [`crate::validate`] (инвариант тестируемости FR-016). Читает
//! **посчитанное** — значения нод ([`FlowOutputs`]), именованные выходы
//! ([`NamedOutputs`]) — и не трогает движок (решение роадмапа §9 CP5:
//! «данные уже в FlowOutputs, дёшево»).
//!
//! ## Актуализация под скалярный движок (v1)
//!
//! Документ FR-016 предполагал `Value::Struct` от `mm1`/`mmc`
//! (`{utilization, queue_length, wait_time}`) — движок v1 возвращает
//! скаляры (queueing.rs: «структура — v2»). Правила детекции v1:
//!
//! | Источник | Правило |
//! |---|---|
//! | значение ноды = `Err(Overload { rho })` | `Severity::Overload`, ρ — из ошибки |
//! | именованный выход `utilization` (Percent) | ρ = доля 0..1 (может быть > 1; ≥ 1 → Overload) |
//! | значение ноды — Percent | трактуется как ρ (контракт `utilization()`/`erlang_c()`) |
//! | значение ноды — Time | W (среднее время пребывания), сравнивается с порогами ожидания |
//! | именованные выходы `wait_time` (Time) / `queue_length` (Count) | точки расширения для custom-шаблонов |
//!
//! Процентные значения — доля 0..1 (контракт FR-015: `utilization` →
//! Percent, «может быть > 1 при overload»). Прочие ошибки формул
//! (парсинг/размерности) — НЕ домен FR-016: их уже показывает красная
//! строка результата (FR-013). SLA-сравнение (`wait_time > SLA_target`) —
//! v2: задание SLA в формуле движком пока не выражается.
//!
//! `AnalysisConfig` вынесен отдельно (инвариант 2 FR-016): пороги — данные,
//! не код; FR-017 (what-if) получит «сценарий с другим порогом» просто
//! другим конфигом.

use std::collections::HashMap;

use serde::Serialize;

use crate::expr::{Dimension, EvalError, Value};
use crate::flow::FlowSolutions;
use crate::model::Canvas;

/// Канонические имена именованных выходов анализа (точки расширения
/// манифестов и custom-шаблонов): `utilization` — Percent, `wait_time` —
/// Time, `queue_length` — Count.
pub const UTILIZATION_OUTPUT: &str = "utilization";
pub const WAIT_TIME_OUTPUT: &str = "wait_time";
pub const QUEUE_LENGTH_OUTPUT: &str = "queue_length";

/// Пороги классификации (значения по умолчанию — из документа FR-016).
/// Время — в базовых секундах (движок: Time-база = sec).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AnalysisConfig {
    /// ρ ≥ порога — предупреждение (жёлтая рамка). По умолчанию 0.7.
    pub warn_util: f64,
    /// ρ ≥ порога — критично (красная рамка). По умолчанию 0.9.
    pub critical_util: f64,
    /// Ожидание W ≥ порога — предупреждение. По умолчанию 100 ms.
    pub warn_wait_sec: f64,
    /// Ожидание W ≥ порога — критично. По умолчанию 1 sec.
    pub critical_wait_sec: f64,
    /// Длина очереди L ≥ порога — предупреждение. По умолчанию 1.
    pub warn_queue: f64,
    /// Длина очереди L ≥ порога — критично. По умолчанию 10.
    pub critical_queue: f64,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            warn_util: 0.7,
            critical_util: 0.9,
            warn_wait_sec: 0.1,
            critical_wait_sec: 1.0,
            warn_queue: 1.0,
            critical_queue: 10.0,
        }
    }
}

/// Серьёзность узкого места: максимум по всем правилам детекции.
/// Порядок усиления: None < Warn < Critical < Overload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Нет признаков узкого места.
    #[default]
    None,
    /// ρ/ожидание/очередь у порога — жёлтая рамка.
    Warn,
    /// ρ/ожидание/очередь за порогом — красная рамка.
    Critical,
    /// ρ ≥ 1 — очередь неограничена, тёмно-красная рамка + OVERLOAD.
    Overload,
}

impl Severity {
    /// Строковое значение для MCP и логов.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Warn => "warn",
            Self::Critical => "critical",
            Self::Overload => "overload",
        }
    }

    /// Максимум двух серьёзностей (усиление). Публичен с FR-066: MCP-слой
    /// monte_carlo_run агрегирует худшую серьёзность P90-снимка.
    pub const fn max(self, other: Self) -> Self {
        use Severity::*;
        match (self, other) {
            (Overload, _) | (_, Overload) => Overload,
            (Critical, _) | (_, Critical) => Critical,
            (Warn, _) | (_, Warn) => Warn,
            (None, None) => None,
        }
    }
}

/// Флаги анализа одной ноды: извлечённые метрики + итоговая серьёзность.
/// Сериализация (snake_case, None-поля опускаются) — контракт
/// MCP-инструмента `analyze_bottlenecks`: то же, что видит пользователь
/// на канвасе (инвариант 4 FR-016).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
pub struct AnalysisFlags {
    /// Утилизация ρ (доля 0..1; > 1 при перегрузке). None — данных нет.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utilization: Option<f64>,
    /// Длина очереди L (в запросах). None — данных нет (v1: только
    /// именованный выход `queue_length`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_length: Option<f64>,
    /// Среднее время пребывания W в базовых секундах. None — данных нет.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_sec: Option<f64>,
    /// Итоговая серьёзность (максимум по правилам).
    pub severity: Severity,
}

impl AnalysisFlags {
    /// Есть ли хоть одна метрика (для MCP: нода без метрик не отчёт).
    pub const fn has_metrics(&self) -> bool {
        self.utilization.is_some()
            || self.queue_length.is_some()
            || self.wait_sec.is_some()
            || matches!(self.severity, Severity::Overload)
    }
}

/// Состояние анализа сцены: `node_id` → флаги. Runtime-only — не
/// сериализуется в `.canvas` (источник истины — формулы и propagator).
pub type AnalysisState = HashMap<String, AnalysisFlags>;

/// Размерность значения — ровно одна, указанная, со степенью 1
/// (как `is_single_dim` в queueing.rs: «42 %» — Percent¹, «1.2 req·%» — нет).
fn is_single_dim(value: &Value, dim: &Dimension) -> bool {
    let dims = value.dims();
    dims.len() == 1 && dims.get(dim) == Some(&1)
}

/// Правила детекции для ОДНОЙ ноды с успешным значением: собирает метрики
/// из значения ноды и именованных выходов, классифицирует по порогам.
fn flags_for_value(
    value: &Value,
    node_id: &str,
    solutions: &FlowSolutions,
    config: &AnalysisConfig,
) -> AnalysisFlags {
    let named_of = |name: &str| -> Option<&Value> {
        solutions.named.get(&(node_id.to_owned(), name.to_owned()))
    };
    let named_percent = named_of(UTILIZATION_OUTPUT)
        .filter(|v| is_single_dim(v, &Dimension::Percent))
        .map(|v| v.num);
    let named_wait = named_of(WAIT_TIME_OUTPUT)
        .filter(|v| is_single_dim(v, &Dimension::Time))
        .map(|v| v.num * v.unit.scale());
    let named_queue = named_of(QUEUE_LENGTH_OUTPUT)
        .filter(|v| is_single_dim(v, &Dimension::Count))
        .map(|v| v.num * v.unit.scale());

    // Значение ноды: Percent — утилизация (контракт utilization()/erlang_c()),
    // Time — время пребывания W. Count сам по себе НЕ трактуется как длина
    // очереди (ложные срабатывания на innocent-списках «10 req»); длина
    // очереди v1 — только явный именованный выход.
    let value_util = if is_single_dim(value, &Dimension::Percent) && named_percent.is_none() {
        Some(value.num)
    } else {
        None
    };
    let value_wait = if is_single_dim(value, &Dimension::Time) && named_wait.is_none() {
        Some(value.num * value.unit.scale())
    } else {
        None
    };

    let mut flags = AnalysisFlags {
        utilization: named_percent.or(value_util),
        queue_length: named_queue,
        wait_sec: named_wait.or(value_wait),
        severity: Severity::None,
    };
    flags.severity = classify(&flags, config);
    flags
}

/// Классификация собранных метрик по порогам: максимум по правилам.
/// ρ ≥ 1 — Overload (математика M/M/c: очередь неограничена) даже без
/// ошибки движка (custom-шаблон может считать utilization своей формулой).
fn classify(flags: &AnalysisFlags, config: &AnalysisConfig) -> Severity {
    let mut severity = Severity::None;
    if let Some(u) = flags.utilization {
        if u >= 1.0 {
            severity = severity.max(Severity::Overload);
        } else if u >= config.critical_util {
            severity = severity.max(Severity::Critical);
        } else if u >= config.warn_util {
            severity = severity.max(Severity::Warn);
        }
    }
    if let Some(w) = flags.wait_sec {
        if w >= config.critical_wait_sec {
            severity = severity.max(Severity::Critical);
        } else if w >= config.warn_wait_sec {
            severity = severity.max(Severity::Warn);
        }
    }
    if let Some(q) = flags.queue_length {
        if q >= config.critical_queue {
            severity = severity.max(Severity::Critical);
        } else if q >= config.warn_queue {
            severity = severity.max(Severity::Warn);
        }
    }
    severity
}

/// Анализ сцены: для каждой ноды со значением в потоке — флаги и
/// серьёзность. Ноды без формулы (нет записи в `FlowOutputs`) в отчёт не
/// попадают — рендер трактует отсутствие как `Severity::None`. Порядок
/// обхода — `canvas.nodes` (детерминизм; HashMap только для адресации).
pub fn analyze(
    canvas: &Canvas,
    solutions: &FlowSolutions,
    config: &AnalysisConfig,
) -> AnalysisState {
    let mut state = AnalysisState::new();
    for node in &canvas.nodes {
        let Some(outcome) = solutions.outputs.get(&node.id) else {
            continue;
        };
        let flags = match outcome {
            // ρ ≥ 1: mm1/mmc вернул ошибку — ρ берём из самой ошибки
            // (он там точный, эталон ADR-0006 №2: «2.23»).
            Err(EvalError::Overload { rho }) => AnalysisFlags {
                utilization: Some(*rho),
                queue_length: None,
                wait_sec: None,
                severity: Severity::Overload,
            },
            // Прочие ошибки формул — не домен FR-016 (красная строка FR-013).
            Err(_) => continue,
            Ok(value) => flags_for_value(value, &node.id, solutions, config),
        };
        if flags.has_metrics() {
            state.insert(node.id.clone(), flags);
        }
    }
    state
}

/// Есть ли в состоянии хоть один узкий места уровня Warn и выше —
/// авто-включение оверлея (FR-016 «Открытые вопросы», решение v1).
pub fn has_risk(state: &AnalysisState) -> bool {
    state
        .values()
        .any(|flags| !matches!(flags.severity, Severity::None))
}

/// Текст бейджа узкого места (та же строка на канвасе и в MCP — инвариант 4
/// FR-016: AI-клиент видит то же, что пользователь). Формат: непустые
/// метрики через « · » — «42%», «95% · Q: 4.2», «OVERLOAD 223% · W: 1.2 s».
/// ASCII-только (моно-шрифт рендера; ρ передаётся процентами — >100 %
/// читается как перегрузка).
pub fn badge_text(flags: &AnalysisFlags) -> String {
    let mut parts: Vec<String> = Vec::new();
    let util = flags.utilization.map(|u| (u * 100.0).round());
    match flags.severity {
        Severity::Overload => parts.push(match util {
            Some(pct) => format!("OVERLOAD {pct}%"),
            None => "OVERLOAD".to_owned(),
        }),
        _ => {
            if let Some(pct) = util {
                parts.push(format!("{pct}%"));
            }
        }
    }
    if let Some(q) = flags.queue_length {
        parts.push(format!("Q: {q:.1}"));
    }
    if let Some(w) = flags.wait_sec {
        // Время — в базовых секундах; человекочитаемо: < 1 s — миллисекунды.
        let text = if w.abs() < 1.0 {
            format!("W: {} ms", (w * 1000.0).round())
        } else {
            format!("W: {w:.1} s")
        };
        parts.push(text);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Unit;
    use crate::flow;
    use crate::model::Node;

    fn percent_unit() -> Unit {
        Unit::atom(crate::expr::Atom::new(Dimension::Percent, 1, 1.0, "%"))
    }

    fn time_unit() -> Unit {
        Unit::atom(crate::expr::Atom::new(Dimension::Time, 1, 1.0, "sec"))
    }

    fn ms_unit() -> Unit {
        Unit::atom(crate::expr::Atom::new(Dimension::Time, 1, 0.001, "ms"))
    }

    fn count_unit() -> Unit {
        Unit::atom(crate::expr::Atom::new(Dimension::Count, 1, 1.0, "req"))
    }

    /// Нода с формулой `mm1(...)`: значение из потока = W (Time), ρ — в
    /// named-выходе (как у шаблонов после CP5).
    fn queue_canvas() -> Canvas {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "a", 0.0, 0.0);
        a.set_expr(Some("mm1(100 rps, 240 rps)".to_owned()));
        let mut b = Node::text("b", "b", 220.0, 0.0);
        b.set_expr(Some("utilization(216 rps, 240 rps)".to_owned()));
        let plain = Node::text("plain", "просто текст", 440.0, 0.0);
        let mut over = Node::text("over", "over", 660.0, 0.0);
        over.set_expr(Some("mm1(1200 rps, 1000 rps)".to_owned()));
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        canvas.nodes.push(plain);
        canvas.nodes.push(over);
        canvas
    }

    /// Инвариант 1 FR-016: пустая сцена — пустой отчёт.
    #[test]
    fn analyze_empty_canvas_is_empty() {
        let state = analyze(
            &Canvas::default(),
            &FlowSolutions::default(),
            &AnalysisConfig::default(),
        );
        assert!(state.is_empty());
    }

    /// Нода без формулы — не в отчёте (нет анализа: severity None не
    /// занимает место в карте).
    #[test]
    fn analyze_skips_nodes_without_formula() {
        let canvas = queue_canvas();
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert!(!state.contains_key("plain"));
        assert!(state.contains_key("a"));
    }

    /// ρ = 0.9 (named-выход utilization) → Critical; значение ноды — Time
    /// (W = 6.5 ms < warn) — не мешает.
    #[test]
    fn analyze_utilization_critical_from_named_output() {
        let mut solutions =
            flow::propagate_with_lines(&queue_canvas(), &flow::WhatIfOverrides::default()).unwrap();
        // Шаблонный путь: named-выход utilization = 0.9 (Percent).
        solutions.named.insert(
            ("a".to_owned(), "utilization".to_owned()),
            Value::with_unit(0.9, percent_unit()),
        );
        let state = analyze(&queue_canvas(), &solutions, &AnalysisConfig::default());
        let a = state.get("a").expect("a в отчёте");
        assert_eq!(a.severity, Severity::Critical);
        assert_eq!(a.utilization, Some(0.9));
        // W = 1/240 + W_q ≈ 0.00686 sec — ниже warn (0.1)
        assert!(a.wait_sec.unwrap() > 0.0 && a.wait_sec.unwrap() < 0.1);
    }

    /// Скалярный Percent (значение ноды = utilization(...)) → Warn при 0.9
    /// в конфиге с critical 0.95 (инвариант 2: пороги — данные).
    #[test]
    fn analyze_custom_thresholds_change_severity() {
        let canvas = queue_canvas();
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        // b = utilization(216, 240) = 0.9 → default Critical.
        let default = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert_eq!(default.get("b").unwrap().severity, Severity::Critical);
        // Тот же 0.9 при critical_util = 0.95 → Warn.
        let config = AnalysisConfig {
            critical_util: 0.95,
            ..AnalysisConfig::default()
        };
        let shifted = analyze(&canvas, &solutions, &config);
        assert_eq!(shifted.get("b").unwrap().severity, Severity::Warn);
    }

    /// ρ < warn → Severity::None, но метрика в отчёте (MCP видит число).
    #[test]
    fn analyze_below_thresholds_is_none_with_metric() {
        let mut canvas = Canvas::default();
        let mut healthy = Node::text("healthy", "h", 0.0, 0.0);
        healthy.set_expr(Some("utilization(50 rps, 240 rps)".to_owned()));
        canvas.nodes.push(healthy);
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        let flags = state.get("healthy").expect("метрика есть");
        assert_eq!(flags.severity, Severity::None);
        assert!((flags.utilization.unwrap() - 50.0 / 240.0).abs() < 1e-9);
    }

    /// Overload из ошибки: ρ достаётся из ошибки, severity = Overload.
    #[test]
    fn analyze_overload_from_error() {
        let canvas = queue_canvas();
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        let over = state.get("over").expect("over в отчёте");
        assert_eq!(over.severity, Severity::Overload);
        assert!((over.utilization.unwrap() - 1.2).abs() < 1e-9);
        assert!(over.wait_sec.is_none(), "W при перегрузке нет");
    }

    /// Named-выход utilization ≥ 1 без ошибки значения → Overload
    /// (custom-шаблон считает ρ сам).
    #[test]
    fn analyze_utilization_named_over_one_is_overload() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("x", "x", 0.0, 0.0);
        node.set_expr(Some("5".to_owned()));
        canvas.nodes.push(node);
        let mut solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        solutions.named.insert(
            ("x".to_owned(), "utilization".to_owned()),
            Value::with_unit(1.5, percent_unit()),
        );
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert_eq!(state.get("x").unwrap().severity, Severity::Overload);
        assert_eq!(state.get("x").unwrap().utilization, Some(1.5));
    }

    /// W из значения ноды: 34 ms в unit ms → wait_sec = 0.034 (база sec),
    /// ниже warn 100 ms → None; 1.2 sec → Critical.
    #[test]
    fn analyze_wait_time_thresholds() {
        let mut canvas = Canvas::default();
        let mut fast = Node::text("fast", "f", 0.0, 0.0);
        fast.set_expr(Some("34 ms".to_owned()));
        let mut slow = Node::text("slow", "s", 220.0, 0.0);
        slow.set_expr(Some("1.2 sec".to_owned()));
        let mut mid = Node::text("mid", "m", 440.0, 0.0);
        mid.set_expr(Some("150 ms".to_owned()));
        canvas.nodes.push(fast);
        canvas.nodes.push(slow);
        canvas.nodes.push(mid);
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert_eq!(state.get("fast").unwrap().severity, Severity::None);
        assert!((state.get("fast").unwrap().wait_sec.unwrap() - 0.034).abs() < 1e-9);
        assert_eq!(state.get("mid").unwrap().severity, Severity::Warn);
        assert_eq!(state.get("slow").unwrap().severity, Severity::Critical);
    }

    /// Named-выход queue_length (Count) → метрика L и Warn при 4.2.
    #[test]
    fn analyze_queue_length_from_named_output() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("q", "q", 0.0, 0.0);
        node.set_expr(Some("littles_law(100 rps, 5 ms)".to_owned()));
        canvas.nodes.push(node);
        let mut solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        solutions.named.insert(
            ("q".to_owned(), "queue_length".to_owned()),
            Value::with_unit(4.2, count_unit()),
        );
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        let flags = state.get("q").expect("q в отчёте");
        assert_eq!(flags.queue_length, Some(4.2));
        assert_eq!(flags.severity, Severity::Warn);
    }

    /// Count-значение ноды НЕ трактуется как длина очереди (иннокентный
    /// «10 req» не поджигает рамку).
    #[test]
    fn analyze_count_value_is_not_queue() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("c", "c", 0.0, 0.0);
        node.set_expr(Some("10 req".to_owned()));
        canvas.nodes.push(node);
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert!(!state.contains_key("c"), "нет метрик — нет записи");
    }

    /// Максимум по правилам: utilization 0.5 (None) + wait 1.2 sec
    /// (Critical) → Critical.
    #[test]
    fn analyze_severity_is_max_across_rules() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("mix", "m", 0.0, 0.0);
        node.set_expr(Some("1.2 sec".to_owned()));
        canvas.nodes.push(node);
        let mut solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        solutions.named.insert(
            ("mix".to_owned(), "utilization".to_owned()),
            Value::with_unit(0.5, percent_unit()),
        );
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert_eq!(state.get("mix").unwrap().severity, Severity::Critical);
    }

    /// has_risk — авто-включение оверлея.
    #[test]
    fn has_risk_detects_any_severity() {
        let canvas = queue_canvas();
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert!(has_risk(&state), "over-нода (ρ 1.2) — риск есть");

        let mut healthy = Canvas::default();
        let mut node = Node::text("h", "h", 0.0, 0.0);
        node.set_expr(Some("utilization(50 rps, 240 rps)".to_owned()));
        healthy.nodes.push(node);
        let solutions =
            flow::propagate_with_lines(&healthy, &flow::WhatIfOverrides::default()).unwrap();
        let state = analyze(&healthy, &solutions, &AnalysisConfig::default());
        assert!(!has_risk(&state));
    }

    /// Детерминизм: два вызова — равные состояния.
    #[test]
    fn analyze_is_deterministic() {
        let canvas = queue_canvas();
        let solutions =
            flow::propagate_with_lines(&canvas, &flow::WhatIfOverrides::default()).unwrap();
        let first = analyze(&canvas, &solutions, &AnalysisConfig::default());
        let second = analyze(&canvas, &solutions, &AnalysisConfig::default());
        assert_eq!(first, second);
    }

    /// Контракт сериализации (MCP): snake_case, severity — строка,
    /// None-метрики опускаются (компактный отчёт).
    #[test]
    fn analysis_flags_serialize_snake_case() {
        let flags = AnalysisFlags {
            utilization: Some(2.23),
            queue_length: None,
            wait_sec: None,
            severity: Severity::Overload,
        };
        let json = serde_json::to_value(flags).unwrap();
        assert_eq!(json["utilization"], 2.23);
        assert_eq!(json["severity"], "overload");
        assert!(json.get("queue_length").is_none(), "None опущен");
        assert!(json.get("wait_sec").is_none(), "None опущен");
    }

    /// Юнит-хелперы: ms-масштаб в базу секунд (0.034), percent — доля.
    #[test]
    fn unit_scales_as_expected() {
        assert!((ms_unit().scale() - 0.001).abs() < 1e-12);
        assert!((time_unit().scale() - 1.0).abs() < 1e-12);
        assert!((percent_unit().scale() - 1.0).abs() < 1e-12);
        assert_eq!(
            Value::with_unit(0.417, percent_unit()).num,
            0.417,
            "процент — доля, не сотые"
        );
    }

    /// Бейдж-текст (канвас = MCP): перегрузка с ρ, W в ms, Q с 1 знаком.
    #[test]
    fn badge_text_formats_metrics() {
        assert_eq!(
            badge_text(&AnalysisFlags {
                utilization: Some(0.417),
                ..AnalysisFlags::default()
            }),
            "42%"
        );
        assert_eq!(
            badge_text(&AnalysisFlags {
                utilization: Some(2.23),
                queue_length: None,
                wait_sec: Some(1.2),
                severity: Severity::Overload,
            }),
            "OVERLOAD 223% · W: 1.2 s"
        );
        assert_eq!(
            badge_text(&AnalysisFlags {
                utilization: Some(0.95),
                queue_length: Some(4.16),
                wait_sec: Some(0.034),
                severity: Severity::Critical,
            }),
            "95% · Q: 4.2 · W: 34 ms"
        );
        assert_eq!(
            badge_text(&AnalysisFlags::default()),
            "",
            "без метрик — пустой бейдж (рендер не рисует)"
        );
    }
}
