//! FR-032: чтение графа и валидация модели — чистая функция ядра (R2).
//!
//! [`validate`] — `(&Canvas) -> Vec<ValidationIssue>`: без I/O, без мутаций,
//! в духе [`crate::flow`] (инвариант тестируемости FR-014). Внутри исполняет
//! `propagate_with_lines` (перегрузки E-OVERLOAD, построчные выходы для
//! W-AMBIGUOUS-SRC) и `topo_sort` (циклы E-CYCLE); результат —
//! структурированный отчёт для MCP-инструмента `graph_validate`
//! (агент диагностирует модель, не читая формулы вслепую — CR-013 G7).
//!
//! ## Контракт кодов v1 (стабильный API для рецепта агента R5)
//!
//! | Код | Severity | Условие | Статус |
//! |---|---|---|---|
//! | `E-CYCLE` | error | цикл в value-подграфе (участники — `CycleError.nodes`) | реализован |
//! | `E-UNIT` | error | несовместимая размерность единицы выхода истока и параметра приёмника | **после FR-029 (CP1)**: определён на `toParam` + `OutputSpec.unit` |
//! | `E-PORT-UNKNOWN` | error | ребро адресует имя выхода/параметра, отсутствующее в снапшоте шаблона | **после FR-029 (CP1)**: определён на `fromOutput`/`toParam` |
//! | `E-DOUBLE-INPUT` | error | два value-ребра в один `toParam` (оба `edge_id` в отчёте) | **после FR-029 (CP1)**: определён на `toParam` |
//! | `E-OVERLOAD` | error | нода в `EvalError::Overload` (ρ ≥ 1 — очередь неограничена) | реализован |
//! | `W-AMBIGUOUS-SRC` | warning | value-ребро без `fromLine`/`fromOutput`, у истока > 1 формульной строки | реализован (v1: без `fromOutput` — поля ещё нет) |
//! | `W-UNUSED-SLOT` | warning | позиционный вход `$N` доставлен, но формулы приёмника его не читают | реализован |
//!
//! Три кода `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` определены на полях
//! FR-029 (`toParam`/`fromOutput`/`outputs`-манифесты), которые реализуются
//! параллельно (CP1). Точка их добавления — [`port_contract_issues`];
//! контракт и порядок зафиксированы здесь и в документе FR-032, чтобы влитие
//! CP1 не меняло внешний API отчёта.
//!
//! ## Детерминизм порядка
//!
//! Порядок issues стабилен между вызовами: E-CYCLE первым; далее проверки
//! группами в порядке фиксации кодов (E-PORT-*, E-DOUBLE-INPUT, E-OVERLOAD,
//! W-AMBIGUOUS-SRC, W-UNUSED-SLOT), внутри группы — по порядку
//! `canvas.edges` / `canvas.nodes`. Это позволяет автотестам и рецепту агента
//! сравнивать отчёты на равенство.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::expr::{self, EvalError, Expr};
use crate::flow::{self, FlowKind};
use crate::model::{Canvas, Edge, Node};

/// Серьёзность проблемы: `error` ломает расчёт, `warning` — подозрительное,
/// но рабочее состояние. `valid` в MCP-ответе = нет ни одного `error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

/// Код проблемы — стабильный контракт v1 (значения — строки `E-*`/`W-*`,
/// см. таблицу в модуле). Новые коды только добавляются, не переименовываются.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueCode {
    ECycle,
    EUnit,
    EPortUnknown,
    EDoubleInput,
    EOverload,
    WAmbiguousSrc,
    WUnusedSlot,
}

impl IssueCode {
    /// Строковое значение для MCP (контракт FR-032 п.2).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ECycle => "E-CYCLE",
            Self::EUnit => "E-UNIT",
            Self::EPortUnknown => "E-PORT-UNKNOWN",
            Self::EDoubleInput => "E-DOUBLE-INPUT",
            Self::EOverload => "E-OVERLOAD",
            Self::WAmbiguousSrc => "W-AMBIGUOUS-SRC",
            Self::WUnusedSlot => "W-UNUSED-SLOT",
        }
    }
}

impl Serialize for IssueCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Одна проблема модели: severity + код + точная локация (нода/ребро) +
/// человекочитаемое сообщение. Сериализация в snake_case для MCP
/// (`node_id`/`edge_id` — как в документе FR-032).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ValidationIssue {
    pub severity: Severity,
    pub code: IssueCode,
    pub node_id: Option<String>,
    pub edge_id: Option<String>,
    pub message: String,
}

/// Есть ли в отчёте ошибки (`severity = error`) — `valid` для MCP-ответа.
pub fn has_errors(issues: &[ValidationIssue]) -> bool {
    issues.iter().any(|issue| issue.severity == Severity::Error)
}

/// Валидация модели: полный отчёт проблем (детерминированный порядок —
/// см. модуль). Чистая функция: канвас не меняется, пересчёт потока
/// выполняется внутри на копии значений (`propagate_with_lines`).
pub fn validate(canvas: &Canvas) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    // E-CYCLE + пересчёт: при цикле отчёт ограничивается топологией —
    // цикл многосубъектен и ломает пересчёт целиком, остальные проверки
    // дали бы шум (тишина до починки цикла; структурные проверки портов
    // FR-029 тоже вне отчёта — контракт «почини цикл, потом остальное»).
    let solutions = match flow::propagate_with_lines(canvas, &HashMap::new()) {
        Ok(solutions) => solutions,
        Err(cycle) => {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                code: IssueCode::ECycle,
                node_id: None,
                edge_id: None,
                message: format!("цикл потока значений: {}", cycle.nodes.join(" → ")),
            });
            return issues;
        }
    };

    // E-UNIT / E-PORT-UNKNOWN / E-DOUBLE-INPUT — контракт портов FR-029
    // (порядок рёбер — canvas.edges, детерминизм для E-DOUBLE-INPUT).
    issues.extend(port_contract_issues(canvas));

    {
        // E-OVERLOAD: ноды в перегрузке (порядок — canvas.nodes)
        for node in &canvas.nodes {
            if let Some(Err(err)) = solutions.outputs.get(&node.id) {
                if matches!(err, EvalError::Overload { .. }) {
                    issues.push(ValidationIssue {
                        severity: Severity::Error,
                        code: IssueCode::EOverload,
                        node_id: Some(node.id.clone()),
                        edge_id: None,
                        message: err.to_string(),
                    });
                }
            }
        }

        // W-AMBIGUOUS-SRC: value-ребро без адресации истока при многолинейном
        // источнике — несёт итог ноды (последняя формульная строка/формула
        // шаблона), возможно не то, что имел в виду автор (порядок — edges)
        let line_counts = formula_line_counts(&solutions.lines);
        for edge in &canvas.edges {
            if edge.flow_kind() != FlowKind::Value || edge.from_line.is_some() {
                // FR-029 (CP1): ребро с fromOutput тоже адресует исток —
                // добавить `|| edge.from_output.is_some()` при влитии поля
                continue;
            }
            let lines = line_counts.get(&edge.from_node).copied().unwrap_or(0);
            if lines > 1 {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    code: IssueCode::WAmbiguousSrc,
                    node_id: Some(edge.from_node.clone()),
                    edge_id: Some(edge.id.clone()),
                    message: format!(
                        "value-ребро без fromLine/fromOutput: у истока {} формульных строк — \
                         ребро несёт итог ноды (последняя строка); уточните адресацию",
                        lines
                    ),
                });
            }
        }
    }

    // W-UNUSED-SLOT: позиционные входы $1..$N, которые формулы приёмника
    // не читают (порядок — nodes, затем рёбра в порядке slots)
    for node in &canvas.nodes {
        let slots: Vec<&Edge> = canvas
            .edges
            .iter()
            .filter(|edge| edge.to_node == node.id && edge.flow_kind() == FlowKind::Value)
            .collect();
        if slots.is_empty() {
            continue;
        }
        let refs = slot_references(node);
        for (index, edge) in slots.iter().enumerate() {
            let slot = index + 1;
            // $in читает единственный вход — слот занят
            if refs.in_ref && slots.len() == 1 {
                continue;
            }
            if refs.slots.contains(&slot) {
                continue;
            }
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                code: IssueCode::WUnusedSlot,
                node_id: Some(node.id.clone()),
                edge_id: Some(edge.id.clone()),
                message: format!(
                    "позиционный вход ${slot} доставлен (ребро {} от {}), но формулы ноды \
                     его не читают — значение теряется",
                    edge.id, edge.from_node
                ),
            });
        }
    }

    issues
}

/// Коды контракта портов FR-029: **E-UNIT / E-PORT-UNKNOWN /
/// E-DOUBLE-INPUT**. Определены на полях `toParam`/`fromOutput` и
/// `outputs`-снапшотах шаблонов, которые реализуются шагом CP1 (FR-029).
///
/// Точка интеграции CP1: после влития полей сюда добавляются проверки
/// (контракт — документ FR-032 п.2):
/// - `E-UNIT` — `OutputSpec.unit` истока против единицы параметра
///   (`templates::params_from_text`) приёмника;
/// - `E-PORT-UNKNOWN` — имя `fromOutput`/`toParam` отсутствует в снапшоте
///   шаблона соответствующего конца;
/// - `E-DOUBLE-INPUT` — два value-ребра в один `toParam` (оба `edge_id`,
///   порядок — `canvas.edges`); рёбра с `toParam` исключить из
///   позиционных слотов в W-UNUSED-SLOT выше.
fn port_contract_issues(_canvas: &Canvas) -> Vec<ValidationIssue> {
    Vec::new()
}

/// Число формульных строк по нодам (из построчных выходов пересчёта FR-025):
/// строка считается формульной, когда вычислилась — проза и ошибки
/// значения не дают. Это же определение использует выбор итога ноды.
fn formula_line_counts(lines: &flow::LineOutputs) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (node_id, _) in lines.keys() {
        *counts.entry(node_id.clone()).or_insert(0) += 1;
    }
    counts
}

/// Ссылки формул ноды на позиционные входы: `$N` (целые N ≥ 1) и `$in`.
///
/// Источники ссылок (все видят входы value-рёбер — `propagate_with_lines`):
/// формула шаблона (снапшот, приоритетна для шаблонной ноды) ИЛИ явная
/// `canvasdesk.expr`, плюс КАЖДАЯ формульная строка текста (строки листа
/// делят окружение — вход может читаться любой из них). Код-фенсы и проза
/// пропускаются: парсинг строки не удался — она не формула (Numi-тишина).
fn slot_references(node: &Node) -> SlotRefs {
    let mut refs = SlotRefs::default();
    let template = node.template();
    let main_formula: Option<String> = match &template {
        Some(tpl) => Some(tpl.expr.clone()),
        None => node.expr().map(str::to_owned),
    };
    if let Some(formula) = main_formula {
        // `canvasdesk.expr` канонически без префикса `=`; рукописные файлы
        // бывают с ним — снятие безвредно для канонической формы
        let trimmed = formula.trim();
        let formula = trimmed.strip_prefix('=').map(str::trim).unwrap_or(trimmed);
        if let Ok(parsed) = expr::parse(formula) {
            collect_slot_refs(&parsed, &mut refs);
        }
    }
    let text = node.text.clone().unwrap_or_default().replace("\\=", "=");
    let mut in_fence = false;
    for line in text.split('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let trimmed = line.trim();
        let statement = trimmed.strip_prefix('=').map(str::trim).unwrap_or(trimmed);
        if statement.is_empty() {
            continue;
        }
        if let Ok(parsed) = expr::parse(statement) {
            collect_slot_refs(&parsed, &mut refs);
        }
    }
    refs
}

/// Множество занятых позиционных входов формул ноды.
#[derive(Default)]
struct SlotRefs {
    slots: HashSet<usize>,
    /// Формула читает `$in` (валиден при ровно одном входе).
    in_ref: bool,
}

/// Обход дерева формулы: `$N` (целые ≥ 1 — входы, дробные — валюта) и
/// `$in`. `$N` вне окружения со входами — валюта, но у приёмника входы
/// есть по построению, поэтому целые читаются как ссылки на слоты.
fn collect_slot_refs(expr: &Expr, refs: &mut SlotRefs) {
    match expr {
        Expr::Inbound => refs.in_ref = true,
        Expr::DollarAmount(num) => {
            if *num >= 1.0 && num.fract() == 0.0 {
                refs.slots.insert(*num as usize);
            }
        }
        Expr::Neg(inner) => collect_slot_refs(inner, refs),
        Expr::Bin { lhs, rhs, .. } => {
            collect_slot_refs(lhs, refs);
            collect_slot_refs(rhs, refs);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_slot_refs(arg, refs);
            }
        }
        Expr::Assign { rhs, .. } => collect_slot_refs(rhs, refs),
        Expr::Block(statements) => {
            for statement in statements {
                collect_slot_refs(statement, refs);
            }
        }
        Expr::Num(..) | Expr::Var(_) | Expr::Param(_) => {}
    }
}

// --- Тесты (FR-032 п.5: фиксстура на каждый код; чистый эталон — пусто) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    /// Нода-заметка с формулой (`canvasdesk.expr`).
    fn note(canvas: &mut Canvas, id: &str, formula: &str, x: f32) {
        let mut node = Node::text(id, id, x, 0.0);
        node.set_expr(Some(formula.to_owned()));
        canvas.nodes.push(node);
    }

    /// Нода-заметка без явной формулы: значение — последняя формульная
    /// строка Numi-листа (семантика итога FR-013).
    fn sheet(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
        canvas.nodes.push(Node::text(id, text, x, 0.0));
    }

    fn value_edge(canvas: &mut Canvas, from: &str, to: &str) {
        let mut edge = Edge::new(canvas.next_edge_id(), from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        canvas.add_edge(edge);
    }

    fn codes(issues: &[ValidationIssue]) -> Vec<&'static str> {
        issues.iter().map(|i| i.code.as_str()).collect()
    }

    /// Чистый эталон (мини-сцена в текущей семантике): один вход, читается
    /// `$in`; источник однострочный — отчёт пуст (гейт FR-032: issues == []).
    #[test]
    fn clean_scene_has_no_issues() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "traffic", "dau = 100000", 0.0);
        note(&mut canvas, "downstream", "$in × 2", 220.0);
        value_edge(&mut canvas, "traffic", "downstream");
        assert_eq!(validate(&canvas), Vec::new());
    }

    /// E-CYCLE: цикл A→B→A — один issue, участники в сообщении, полей
    /// ноды/ребра нет (цикл многосубъектный — адресация в message).
    #[test]
    fn cycle_reports_participants() {
        let mut canvas = Canvas::default();
        note(&mut canvas, "A", "1", 0.0);
        note(&mut canvas, "B", "2", 220.0);
        value_edge(&mut canvas, "A", "B");
        value_edge(&mut canvas, "B", "A");
        let issues = validate(&canvas);
        assert_eq!(codes(&issues), vec!["E-CYCLE"]);
        let issue = &issues[0];
        assert_eq!(issue.severity, Severity::Error);
        assert_eq!(issue.node_id, None);
        assert!(issue.message.contains("A") && issue.message.contains("B"));
    }

    /// E-OVERLOAD: mm1 с ρ ≥ 1 — точный код + node_id.
    #[test]
    fn overload_reports_node() {
        let mut canvas = Canvas::default();
        note(&mut canvas, "mm1", "mm1(1200 rps, 1000 rps)", 0.0);
        let issues = validate(&canvas);
        assert_eq!(codes(&issues), vec!["E-OVERLOAD"]);
        let issue = &issues[0];
        assert_eq!(issue.node_id.as_deref(), Some("mm1"));
        assert!(
            issue.message.contains("ρ"),
            "в сообщении есть ρ: {}",
            issue.message
        );
    }

    /// W-AMBIGUOUS-SRC: value-ребро от многолинейного истока без fromLine —
    /// код + node_id истока + edge_id.
    #[test]
    fn ambiguous_source_warns() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "traffic", "dau = 100000\nrps = 1200 rps", 0.0);
        note(&mut canvas, "downstream", "$in × 2", 220.0);
        value_edge(&mut canvas, "traffic", "downstream");
        let issues = validate(&canvas);
        assert_eq!(codes(&issues), vec!["W-AMBIGUOUS-SRC"]);
        let issue = &issues[0];
        assert_eq!(issue.severity, Severity::Warning);
        assert_eq!(issue.node_id.as_deref(), Some("traffic"));
        let edge_id = issue.edge_id.clone().expect("edge_id присвоен");
        assert!(
            canvas.edges.iter().any(|e| e.id == edge_id),
            "edge_id существует в канвасе"
        );
    }

    /// W-AMBIGUOUS-SRC: ребро с fromLine от того же истока — НЕ предупреждать
    /// (адресация есть, неоднозначности нет).
    #[test]
    fn addressed_line_does_not_warn() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "traffic", "dau = 100000\nrps = 1200 rps", 0.0);
        note(&mut canvas, "downstream", "$in × 2", 220.0);
        let mut edge = Edge::new(canvas.next_edge_id(), "traffic", None, "downstream", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_line = Some(1);
        canvas.add_edge(edge);
        assert_eq!(validate(&canvas), Vec::new());
    }

    /// W-UNUSED-SLOT: два входа, формула читает только `$1` — предупреждение
    /// ровно о ребре второго слота; severity warning → валидатор не «валит»
    /// модель (has_errors = false).
    #[test]
    fn unused_slot_warns_for_unread_edge() {
        let mut canvas = Canvas::default();
        note(&mut canvas, "a", "10", 0.0);
        note(&mut canvas, "b", "20", 110.0);
        note(&mut canvas, "sum", "$1 + 100", 220.0);
        value_edge(&mut canvas, "a", "sum");
        value_edge(&mut canvas, "b", "sum");
        let issues = validate(&canvas);
        assert_eq!(codes(&issues), vec!["W-UNUSED-SLOT"]);
        let issue = &issues[0];
        assert_eq!(issue.node_id.as_deref(), Some("sum"));
        // Второй слот — ребро от «b»
        let second = canvas
            .edges
            .iter()
            .find(|e| e.from_node == "b")
            .expect("ребро от b");
        assert_eq!(issue.edge_id.as_deref(), Some(second.id.as_str()));
        assert!(!has_errors(&issues), "warning не делает модель невалидной");
    }

    /// W-UNUSED-SLOT: `$in` при ровно одном входе — слот занят, тишина;
    /// при двух входах `$in` неопределим — оба слота нечитаны.
    #[test]
    fn in_ref_counts_only_single_slot() {
        let mut canvas = Canvas::default();
        note(&mut canvas, "a", "10", 0.0);
        note(&mut canvas, "sum", "$in × 2", 220.0);
        value_edge(&mut canvas, "a", "sum");
        assert_eq!(validate(&canvas), Vec::new());

        let mut two = Canvas::default();
        note(&mut two, "a", "10", 0.0);
        note(&mut two, "b", "20", 110.0);
        note(&mut two, "sum", "$in × 2", 220.0);
        value_edge(&mut two, "a", "sum");
        value_edge(&mut two, "b", "sum");
        let issues = validate(&two);
        assert_eq!(issues.len(), 2, "оба слота нечитаны: {:?}", codes(&issues));
        assert!(issues.iter().all(|i| i.code == IssueCode::WUnusedSlot));
    }

    /// W-UNUSED-SLOT: шаблонная нода читает `$param`, а не `$1` —
    /// позиционное ребро теряется (диагностика G5 из CR-013); после CP1
    /// рёбра с toParam исключаются из позиционных слотов.
    #[test]
    fn template_node_positional_edge_is_unused() {
        let mut canvas = Canvas::default();
        note(&mut canvas, "traffic", "1200 rps", 0.0);
        let registry = crate::templates::TemplateRegistry::builtin();
        // com.canvasdesk.lb: expr = mm1($rps, $service_rate, $servers) —
        // формула читает только именованные параметры, не $1..$N
        let manifest = registry
            .find("com.canvasdesk.lb")
            .expect("builtin-шаблон lb (mm1)");
        let node = crate::templates::instantiate(
            manifest,
            &Default::default(),
            "lb_node".to_owned(),
            220.0,
            0.0,
        )
        .expect("инстанциация");
        canvas.nodes.push(node);
        value_edge(&mut canvas, "traffic", "lb_node");
        let issues = validate(&canvas);
        assert_eq!(codes(&issues), vec!["W-UNUSED-SLOT"]);
        assert_eq!(issues[0].node_id.as_deref(), Some("lb_node"));
    }

    /// Детерминизм: два прогона одного канваса — идентичные отчёты
    /// (порядок стабилен, HashMap внутри не просачивается).
    #[test]
    fn report_is_deterministic() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "traffic", "dau = 100000\nrps = 1200 rps", 0.0);
        note(&mut canvas, "a", "10", 110.0);
        note(&mut canvas, "b", "20", 220.0);
        note(&mut canvas, "sum", "$1 + 100", 330.0);
        value_edge(&mut canvas, "traffic", "sum");
        value_edge(&mut canvas, "a", "sum");
        value_edge(&mut canvas, "b", "sum");
        let first = validate(&canvas);
        let second = validate(&canvas);
        assert_eq!(first, second);
        assert!(!first.is_empty(), "фикстура содержит проблемы");
    }

    /// Сериализация: snake_case-поля и строковые коды — контракт MCP
    /// (агент парсит отчёт по ключам severity/code/node_id/edge_id/message).
    #[test]
    fn issue_serializes_to_mcp_contract() {
        let issue = ValidationIssue {
            severity: Severity::Error,
            code: IssueCode::EOverload,
            node_id: Some("mm1".to_owned()),
            edge_id: None,
            message: "перегрузка".to_owned(),
        };
        let json = serde_json::to_value(&issue).expect("сериализация");
        assert_eq!(json["severity"], "error");
        assert_eq!(json["code"], "E-OVERLOAD");
        assert_eq!(json["node_id"], "mm1");
        assert_eq!(json["edge_id"], serde_json::Value::Null);
        assert_eq!(json["message"], "перегрузка");
    }
}
