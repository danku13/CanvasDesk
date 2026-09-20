//! FR-045 R-5 (приоритет владельца, сессия 2026-09-21) — квалифицированная
//! адресация «Объект.Поле»: **единственная точка** сборки отображаемых
//! путей входных параметров. FR-044 (§Changes-1) переиспользует этот модуль
//! для пилюль веера main stage и панели «Как считается» — определение
//! `QualifiedRef` живёт здесь, а не в `bundles.rs` (сметчивание 2026-09-21).
//!
//! Правила (FR-045 §Решения Р-5, проверены на прототипе):
//! - объект = имя ноды-истока ([`node_display_name`]; data-нода FR-045 R-2 —
//!   имя ноды-объекта, колонка приходит через `fromOutput`);
//! - поле = имя выхода (`fromOutput`) | «строка N» (`fromLine`, 1-based для
//!   отображения; полный текст — в `tooltip`) | fallback `edge.id`
//!   (ребро без адресации — значение ноды целиком);
//! - имена отображаются **дословно** (нормализация/транслитерация не
//!   выполняются); коллизия имён нод — fallback «Имя (node_id)» (§Q2);
//! - display-level: исходный синтаксис выражений (`$1`, `$параметр`) не
//!   меняется — подстановка делается только при отображении
//!   ([`formula_displays`]); именованный синтаксис Numi — v2 (FR-044 §Q1).
//!
//! Чистый Rust, без I/O и глобального состояния; wasm-гейт (ADR-0011).

use std::collections::BTreeMap;

use crate::expr::NumiLineKind;
#[cfg(test)]
use crate::model::CanvasdeskExt;
use crate::model::{Canvas, Edge};

/// Квалифицированный путь «Объект.Поле» (FR-045 Р-5, FR-044 Р-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedRef {
    /// Объект — имя ноды-истока (дословно; коллизия — «Имя (node_id)»).
    pub obj: String,
    /// Поле — выход (`fromOutput`), «строка N» (`fromLine`) или `edge.id`.
    pub field: String,
    /// Дополнительные данные для тултипа (FR-044 §Проверка: `fromLine` →
    /// полное «строка N»; короткая форма на канвасе — полный путь).
    pub tooltip: Option<String>,
}

impl QualifiedRef {
    /// Полный путь `Объект.Поле` — для stage, формул и тултипов (FR-044 Р-3).
    pub fn path(&self) -> String {
        format!("{}.{}", self.obj, self.field)
    }

    /// Короткая форма «Поле» — для плотного канваса вне stage (компромисс
    /// FR-044 Р-3: полный путь — в [`QualifiedRef::path`]/тултипе).
    pub fn short(&self) -> &str {
        &self.field
    }
}

/// Отображаемое имя ноды (дословно, FR-045 Р-5): `label` → имя снимка
/// шаблона → первая непустая строка текста → `id`. Детерминировано,
/// без нормализации.
pub fn node_display_name(canvas: &Canvas, node_id: &str) -> String {
    let Some(node) = canvas.node(node_id) else {
        return node_id.to_owned();
    };
    if let Some(label) = node.label.as_deref() {
        if !label.trim().is_empty() {
            return label.to_owned();
        }
    }
    if let Some(name) = node.template().and_then(|t| t.name) {
        if !name.trim().is_empty() {
            return name;
        }
    }
    if let Some(text) = node.text.as_deref() {
        if let Some(line) = text.lines().map(str::trim).find(|l| !l.is_empty()) {
            return line.to_owned();
        }
    }
    node.id.clone()
}

/// Счётчик отображаемых имён (для детекции коллизий §Q2): имя → сколько нод
/// его носят. Публичен для переиспользования рендером (один обход на кадр).
pub fn display_name_counts(canvas: &Canvas) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for node in &canvas.nodes {
        *counts
            .entry(node_display_name(canvas, &node.id))
            .or_insert(0) += 1;
    }
    counts
}

/// Объект для [`display_ref`]: имя ноды с разрешением коллизии — если
/// имя носят несколько нод, добавляется ` (node_id)` (FR-045 §Q2, базово).
pub fn qualified_obj_name(
    canvas: &Canvas,
    node_id: &str,
    counts: &BTreeMap<String, usize>,
) -> String {
    let name = node_display_name(canvas, node_id);
    if counts.get(&name).copied().unwrap_or(0) > 1 {
        format!("{name} ({node_id})")
    } else {
        name
    }
}

/// Единая точка сборки отображаемого пути value-ребра (FR-044 §Changes-1,
/// тесты §Проверка; правила — FR-045 Р-5). `edge_index` вне диапазона —
/// пустой путь (без паники; чистая функция не падает).
pub fn display_ref(canvas: &Canvas, edge_index: usize) -> QualifiedRef {
    let Some(edge) = canvas.edges.get(edge_index) else {
        return QualifiedRef {
            obj: String::new(),
            field: String::new(),
            tooltip: None,
        };
    };
    display_ref_for_edge(canvas, edge, &display_name_counts(canvas))
}

/// То же, но с переиспользованием готового счётчика имён (горячий путь
/// рендера: один [`display_name_counts`] на кадр вместо обхода на ребро).
pub fn display_ref_for_edge(
    canvas: &Canvas,
    edge: &Edge,
    counts: &BTreeMap<String, usize>,
) -> QualifiedRef {
    let obj = qualified_obj_name(canvas, &edge.from_node, counts);
    // Приоритет адресации — как в flow::edge_source_value (FR-025/FR-029):
    // fromLine → fromOutput → значение ноды целиком.
    if let Some(line) = edge.from_line {
        return QualifiedRef {
            obj,
            // Отображение 1-based (человек считает строки с единицы);
            // в модели `from_line` — индекс листа (0-based).
            field: format!("строка {}", line + 1),
            tooltip: Some(format!("строка {}", line + 1)),
        };
    }
    if let Some(name) = &edge.from_output {
        return QualifiedRef {
            obj,
            field: name.clone(),
            tooltip: None,
        };
    }
    // Ребро без адресации: значение ноды целиком — fallback `edge.id`
    // (FR-044 Р-3/инвариант 5: «безымянный выход → edge.id»).
    QualifiedRef {
        obj,
        field: edge.id.clone(),
        tooltip: None,
    }
}

/// Вход приёмника для групп «Переменные» (FR-044 Р-4, FR-045 Р-4):
/// ребро + его позиция в адресации + квалифицированный путь.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRef {
    /// Индекс ребра в `canvas.edges`.
    pub edge_index: usize,
    /// Позиционный слот `$N` (0-based индекс среди позиционных рёбер);
    /// `None` — проливание в параметр.
    pub slot: Option<usize>,
    /// Имя параметра (`toParam`); `None` — позиционный слот.
    pub param: Option<String>,
    /// Квалифицированный путь истока.
    pub r: QualifiedRef,
}

/// Все входящие value-рёбра ноды с квалифицированными путями — порядок
/// `canvas.edges` (детерминирован); позиционные и проливания в параметры.
/// Единственный источник группы «Переменные · входящие значения» для
/// панели main stage (FR-044 Р-4) и рейлов ноды (FR-045 Р-4).
pub fn input_refs(canvas: &Canvas, node_id: &str) -> Vec<InputRef> {
    let counts = display_name_counts(canvas);
    let mut slot: usize = 0;
    canvas
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            edge.to_node == node_id && edge.flow_kind() == crate::flow::FlowKind::Value
        })
        .map(|(edge_index, edge)| {
            let (slot_no, param) = if edge.to_param.is_none() {
                let no = slot;
                slot += 1;
                (Some(no), None)
            } else {
                (None, edge.to_param.clone())
            };
            InputRef {
                edge_index,
                slot: slot_no,
                param,
                r: display_ref_for_edge(canvas, edge, &counts),
            }
        })
        .collect()
}

/// Строка расчёта приёмника (FR-044 Р-4/Р-5, FR-045 Р-4): исходник,
/// display-подстановка квалифицированных путей вместо `$N`/`$параметр`
/// и рёбра-операнды (для подсветки формула⇄переменные⇄рёбра, FR-044 Р-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaDisplay {
    /// Индекс строки тела (для template-ноды — 0: формула снимка).
    pub line: usize,
    /// Исходный текст строки (дословно).
    pub raw: String,
    /// Текст с подставленными путями `Объект.Поле`; неразрешённые токены
    /// (`$N` вне диапазона, параметр без рёбер) остаются как есть.
    pub display: String,
    /// Индексы рёбер-операндов в порядке первого упоминания (без дублей).
    pub operand_edges: Vec<usize>,
}

/// Строки расчёта ноды (группа «Расчёт · формулы», FR-044 Р-4):
/// - template-нода (снимок с непустым `expr`) — одна строка: формула снимка;
/// - текстовая нода — строки тела с родом `Assignment`/`Expression`
///   ([`NumiLineKind`], детектор FR-021) в порядке тела; проза пропускается.
///
/// Детерминировано; подстановка — display-level, исходник не меняется.
pub fn formula_displays(canvas: &Canvas, node_id: &str) -> Vec<FormulaDisplay> {
    let Some(node) = canvas.node(node_id) else {
        return Vec::new();
    };
    let counts = display_name_counts(canvas);
    // Карта адресации приёмника: позиционные слоты и проливания.
    let mut slot_edges: Vec<usize> = Vec::new();
    let mut param_edges: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, edge) in canvas.edges.iter().enumerate() {
        if edge.to_node != node_id || edge.flow_kind() != crate::flow::FlowKind::Value {
            continue;
        }
        match &edge.to_param {
            None => slot_edges.push(index),
            Some(name) => param_edges.entry(name.clone()).or_default().push(index),
        }
    }
    // Подстановка одного токена: `$123` → слот, `$имя` → параметр.
    let substitute = |raw: &str, operands: &mut Vec<usize>| -> String {
        let chars: Vec<char> = raw.chars().collect();
        let mut out = String::with_capacity(raw.len() + 16);
        let mut i = 0;
        while i < chars.len() {
            if chars[i] != '$' {
                out.push(chars[i]);
                i += 1;
                continue;
            }
            // Токен: максимум цифр; если после цифр идёт буква/`_`/`-` —
            // это имя параметра, а не слот (детерминированное правило).
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_digit() {
                j += 1;
            }
            let digits_only = j > i + 1;
            let ident_next = j < chars.len() && is_ident_char(chars[j]);
            if digits_only && !ident_next {
                let slot_no: usize = chars[i + 1..j]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0);
                let edge_index = if slot_no >= 1 {
                    slot_edges.get(slot_no - 1).copied()
                } else {
                    None
                };
                match edge_index {
                    Some(index) => {
                        operands.push(index);
                        out.push_str(
                            &display_ref_for_edge(canvas, &canvas.edges[index], &counts).path(),
                        );
                    }
                    None => {
                        // Неразрешённый слот — токен остаётся как есть.
                        out.push('$');
                        out.push_str(&chars[i + 1..j].iter().collect::<String>());
                    }
                }
                i = j;
                continue;
            }
            // Имя параметра: `$` + идентификатор.
            let mut k = i + 1;
            while k < chars.len() && is_ident_char(chars[k]) {
                k += 1;
            }
            if k > i + 1 {
                let name: String = chars[i + 1..k].iter().collect();
                // Спец-токен `$in` (FR-014): ровно одно входящее ребро.
                if name == "in" && slot_edges.len() == 1 {
                    let index = slot_edges[0];
                    operands.push(index);
                    out.push_str(
                        &display_ref_for_edge(canvas, &canvas.edges[index], &counts).path(),
                    );
                    i = k;
                    continue;
                }
                if let Some(edges) = param_edges.get(&name) {
                    // Все рёбра параметра — операнды (последнее побеждает
                    // в проливе; для подсветки значимы все — конфликты
                    // предупреждаются отдельно, FR-029).
                    for (n, index) in edges.iter().enumerate() {
                        if n == 0 {
                            out.push_str(
                                &display_ref_for_edge(canvas, &canvas.edges[*index], &counts)
                                    .path(),
                            );
                        }
                        operands.push(*index);
                    }
                } else {
                    out.push('$');
                    out.push_str(&name);
                }
                i = k;
                continue;
            }
            // Одиночный `$` без токена — литерал.
            out.push('$');
            i += 1;
        }
        out
    };
    // Источник строк: снимок шаблона или текст тела.
    if let Some(expr) = node
        .template()
        .map(|t| t.expr)
        .filter(|e| !e.trim().is_empty())
    {
        let mut operands = Vec::new();
        let display = substitute(&expr, &mut operands);
        return vec![FormulaDisplay {
            line: 0,
            raw: expr,
            display,
            operand_edges: operands,
        }];
    }
    let Some(text) = node.text.as_deref() else {
        return Vec::new();
    };
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            matches!(
                crate::expr::line_kind(line),
                NumiLineKind::Assignment { .. } | NumiLineKind::Expression
            )
        })
        .map(|(line, raw)| {
            let mut operands = Vec::new();
            let display = substitute(raw, &mut operands);
            FormulaDisplay {
                line,
                raw: raw.to_owned(),
                display,
                operand_edges: operands,
            }
        })
        .collect()
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;

    fn text_node(id: &str, text: &str, x: f32) -> Node {
        Node::text(id, text, x, 0.0)
    }

    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(crate::flow::FlowKind::Value);
        canvas.edges.push(edge);
    }

    /// FR-044 §Проверка: `fromOutput` → «Нода.Выход».
    #[test]
    fn display_ref_from_output() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("a", "заявки", 0.0));
        canvas.nodes.push(text_node("b", "x", 1.0));
        let mut edge = Edge::new("e5", "a", None, "b", None);
        edge.set_flow_kind(crate::flow::FlowKind::Value);
        edge.from_output = Some("Средний_чек".to_owned());
        canvas.edges.push(edge);
        let r = display_ref(&canvas, 0);
        assert_eq!(r.path(), "заявки.Средний_чек");
        assert_eq!(r.short(), "Средний_чек");
        assert_eq!(r.tooltip, None);
    }

    /// FR-044 §Проверка: `fromLine` → поле «строка N» + полное в тултипе;
    /// отображение 1-based.
    #[test]
    fn display_ref_from_line() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("a", "заявки", 0.0));
        canvas.nodes.push(text_node("b", "x", 1.0));
        let mut edge = Edge::new("e1", "a", None, "b", None);
        edge.set_flow_kind(crate::flow::FlowKind::Value);
        edge.from_line = Some(2);
        canvas.edges.push(edge);
        let r = display_ref(&canvas, 0);
        assert_eq!(r.path(), "заявки.строка 3");
        assert_eq!(r.tooltip.as_deref(), Some("строка 3"));
    }

    /// FR-044 §Проверка: безымянный выход (ребро без адресации) → `edge.id`.
    #[test]
    fn display_ref_fallback_edge_id() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("a", "заявки", 0.0));
        canvas.nodes.push(text_node("b", "x", 1.0));
        value_edge(&mut canvas, "e7", "a", "b");
        let r = display_ref(&canvas, 0);
        assert_eq!(r.path(), "заявки.e7");
    }

    /// FR-045 §Q2: коллизия имён нод — fallback «Имя (node_id)».
    #[test]
    fn display_ref_name_collision() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("a1", "Заявки", 0.0));
        canvas.nodes.push(text_node("a2", "Заявки", 1.0));
        canvas.nodes.push(text_node("b", "x", 2.0));
        let mut edge = Edge::new("e1", "a1", None, "b", None);
        edge.set_flow_kind(crate::flow::FlowKind::Value);
        edge.from_output = Some("чек".to_owned());
        canvas.edges.push(edge);
        let r = display_ref(&canvas, 0);
        assert_eq!(r.path(), "Заявки (a1).чек");
    }

    /// FR-045 Р-5: имя — дословно, приоритет label; висячее ребро — id истока.
    #[test]
    fn display_ref_dangling_and_label_priority() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("a", "первая строка\nвторая", 0.0, 0.0));
        canvas.nodes[0].label = Some("Метка".to_owned());
        value_edge(&mut canvas, "e1", "a", "ghost");
        assert_eq!(display_ref(&canvas, 0).obj, "Метка");
        let mut canvas2 = Canvas::default();
        value_edge(&mut canvas2, "e9", "ghost", "b");
        assert_eq!(display_ref(&canvas2, 0).obj, "ghost");
    }

    /// display_ref вне диапазона — пустой путь без паники.
    #[test]
    fn display_ref_out_of_range_safe() {
        let canvas = Canvas::default();
        let r = display_ref(&canvas, 42);
        assert_eq!(r.obj, "");
        assert_eq!(r.field, "");
    }

    /// FR-045 Р-4/FR-044 Р-4: input_refs — позиционные слоты по порядку
    /// `canvas.edges`, проливания отдельно; control-рёбра не входят.
    #[test]
    fn input_refs_slots_and_params() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("s1", "5", 0.0));
        canvas.nodes.push(text_node("s2", "7", 1.0));
        canvas.nodes.push(text_node("p", "9", 2.0));
        canvas.nodes.push(text_node("t", "итог", 3.0));
        value_edge(&mut canvas, "e1", "s1", "t");
        let mut e2 = Edge::new("e2", "s2", None, "t", None);
        e2.set_flow_kind(crate::flow::FlowKind::Value);
        e2.to_param = Some("Сезон".to_owned());
        canvas.edges.push(e2);
        let mut e3 = Edge::new("e3", "p", None, "t", None);
        e3.set_flow_kind(crate::flow::FlowKind::Value);
        e3.from_output = Some("peak".to_owned());
        canvas.edges.push(e3);
        let refs = input_refs(&canvas, "t");
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].slot, Some(0));
        assert_eq!(refs[0].param, None);
        assert_eq!(refs[0].r.path(), "5.e1");
        assert_eq!(refs[1].slot, None);
        assert_eq!(refs[1].param.as_deref(), Some("Сезон"));
        assert_eq!(refs[2].r.path(), "9.peak");
    }

    /// FR-044 Р-5/инвариант 6: подстановка путей в формулу вместо `$N`
    /// и `$параметр`; операнды — ровно рёбра формулы (оракул «выручка»).
    #[test]
    fn formula_display_substitution_and_operands() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("z", "Заявки", 0.0));
        canvas.nodes.push(text_node(
            "t",
            "выручка = $1 · $2 · $Сезон\nпримечание без формулы",
            1.0,
        ));
        value_edge(&mut canvas, "eK", "z", "t");
        let mut e2 = Edge::new("eS", "z", None, "t", None);
        e2.set_flow_kind(crate::flow::FlowKind::Value);
        e2.from_output = Some("Средний_чек".to_owned());
        canvas.edges.push(e2);
        let mut e3 = Edge::new("eC", "z", None, "t", None);
        e3.set_flow_kind(crate::flow::FlowKind::Value);
        e3.to_param = Some("Сезон".to_owned());
        e3.from_output = Some("Сезон".to_owned());
        canvas.edges.push(e3);
        let rows = formula_displays(&canvas, "t");
        assert_eq!(rows.len(), 1, "проза пропущена, формульная строка одна");
        let row = &rows[0];
        assert_eq!(
            row.display,
            "выручка = Заявки.eK · Заявки.Средний_чек · Заявки.Сезон"
        );
        assert_eq!(row.operand_edges, vec![0, 1, 2]);
        assert_eq!(row.raw, "выручка = $1 · $2 · $Сезон");
    }

    /// FR-044 Р-5/инвариант 7 (обратная навигация — данные для неё):
    /// один и тот же параметр в двух строках → операнды в обеих.
    #[test]
    fn formula_display_param_shared() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(text_node("t", "a = $1\nb = $1 · 2", 0.0));
        canvas.nodes.push(text_node("s", "3", 1.0));
        value_edge(&mut canvas, "e1", "s", "t");
        let rows = formula_displays(&canvas, "t");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].display, "a = 3.e1");
        assert_eq!(rows[1].display, "b = 3.e1 · 2");
        assert_eq!(rows[1].operand_edges, vec![0]);
    }

    /// Токен вне диапазона остаётся как есть; `$123abc` — имя параметра.
    #[test]
    fn formula_display_unresolved_tokens() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(text_node("t", "x = $5 · $неизвестный · $2abc", 0.0));
        canvas.nodes.push(text_node("s", "1", 1.0));
        value_edge(&mut canvas, "e1", "s", "t");
        let rows = formula_displays(&canvas, "t");
        assert_eq!(
            rows[0].display, "x = $5 · $неизвестный · $2abc",
            "нет рёбер для слота 5, параметров и смешанного токена"
        );
    }

    /// FR-044 §Решения: template-нода — формула из снимка (`expr`).
    #[test]
    fn formula_display_template_expr() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("t", "Сезон = 1.15", 0.0, 0.0);
        let mut template = serde_json::Map::new();
        template.insert("id".into(), serde_json::json!("tpl"));
        template.insert("version".into(), serde_json::json!("1"));
        template.insert("expr".into(), serde_json::json!("$in * $Сезон"));
        template.insert("params".into(), serde_json::json!({}));
        template.insert("icon".into(), serde_json::json!("x"));
        template.insert("color".into(), serde_json::json!("1"));
        node.canvasdesk = Some(CanvasdeskExt {
            template: Some(serde_json::Value::Object(template)),
            ..CanvasdeskExt::empty()
        });
        canvas.nodes.push(node);
        canvas.nodes.push(text_node("s", "2", 1.0));
        value_edge(&mut canvas, "e1", "s", "t");
        let rows = formula_displays(&canvas, "t");
        assert_eq!(
            rows.len(),
            1,
            "формула снимка — одна строка, лист не сканируется"
        );
        assert_eq!(rows[0].display, "2.e1 * $Сезон");
    }
}
