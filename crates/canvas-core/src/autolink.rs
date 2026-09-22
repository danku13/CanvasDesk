//! PRD-0007 F-7 (X4): автосвязь по именам — детектор предложений связей.
//!
//! Чистая детерминированная функция [`find_proposals`] сканирует расчётные
//! ноды канваса (`line_kind`, `expr.rs` — тот же скан листов, что и у
//! lineage-дерева, фенсы учитываются) и предлагает value-рёбра там, где
//! **точное имя присваивания** (`NumiLineKind::Assignment`) одной ноды
//! совпадает с именем параметра, который приёмник потребляет, но который
//! ещё не запитан (Q2 — решение владельца: матч по точным именам;
//! совпадение единиц — бонус-признак в ревью, [`AutolinkProposal::unit_match`]).
//!
//! Источники — текстовые ноды: присваивание Numi-листа адресуется
//! `fromOutput` (именованный выход FR-029 — «последнее определение имени»,
//! адресация живёт при сдвиге строк). Приёмники:
//! - шаблонные ноды — параметры манифеста (`canvasdesk.template.params`);
//! - текстовые ноды — ссылки `$имя` (`Expr::Param`) в формульных строках.
//!
//! Адресация создаваемого ребра — проливание `toParam` (сильнее дефолта,
//! FR-029): не занимает позиционные слоты `$1..$N` и регистрирует
//! qualified-ключ «Объект.Поле» (FR-050 Р-6), поэтому закрывает и ссылку
//! `$имя`, и адресную форму «Нода.переменная».
//!
//! Фильтры (AC-5.4): циклы ([`creates_value_cycle`], `flow.rs`), дубликаты
//! (любое существующее value-ребро между парой; параметр приёмника, уже
//! занятый проливанием из любого источника — конфликт «один вход на
//! параметр», FR-050 Н4). Порядок предложений детерминирован
//! (§9.4): ноды — в порядке `canvas.nodes`, имена — по алфавиту.
//!
//! Модуль НЕ создаёт связи (D1: фон не мутирует модель — только
//! предложения для ревью) и не зависит от пересчёта потока: топология +
//! род строк достаточны, детектор O(строк + рёбер).

use std::collections::BTreeMap;

use crate::expr::{self, NumiLineKind};
use crate::flow::{creates_value_cycle, value_param_compatible, FlowKind};
use crate::lineage::{collect_refs, Ref, Sheet};
use crate::model::Canvas;

/// Предложение автосвязи (§8 US-5): «исток → приёмник (параметр)».
///
/// Создаваемое ребро (по подтверждению в ревью): value-ребро
/// `from_node → to_node` с `fromOutput = param` (именованный выход
/// присваивания) и `toParam = param` (проливание в параметр приёмника).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutolinkProposal {
    /// id ноды-истока (текстовая нода с присваиванием имени).
    pub from_node: String,
    /// Индекс строки-присваивания в листе истока (последнее определение
    /// имени — для показа формулы в ревью; адресация ребра — `fromOutput`).
    pub from_line: usize,
    /// id ноды-приёмника (шаблонная или текстовая нода с потребностью).
    pub to_node: String,
    /// Имя параметра: присваивание истока == потребность приёмника
    /// (точный матч, Q2). Совпадает с `fromOutput` и `toParam` ребра.
    pub param: String,
    /// Процент совпадения имён (AC-5.2). Точный матч — всегда 100;
    /// поле задел под v2 (нечёткий матч/квалификаторы).
    pub percent: u8,
    /// Бонус-признак совпадения единиц (Q2, решение владельца): `Some(true)`
    /// — значение присваивания и единица параметра шаблона совместимы,
    /// `Some(false)` — размерности конфликтуют, `None` — единицы неизвестны
    /// (текстовый приёмник без спецификации параметра / RHS не константа).
    pub unit_match: Option<bool>,
}

/// Потребность приёмника: параметр, который нода потребляет без поставщика.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Demand {
    param: String,
}

/// Присваивание истока: имя + строка последнего определения.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Assignment {
    name: String,
    line: usize,
}

/// Присваивания текстовой ноды: имя → строка последнего определения
/// (семантика именованных выходов FR-029: «последнее определение имени»).
fn assignments_of(sheet: &Sheet) -> Vec<Assignment> {
    let mut by_name: BTreeMap<String, usize> = BTreeMap::new();
    for (i, kind) in sheet.kinds.iter().enumerate() {
        if let Some(NumiLineKind::Assignment { name }) = kind {
            // Повторное присваивание имени перекрывает предыдущее (движок:
            // env.set поверх) — оставляем последнюю строку
            by_name.insert(name.clone(), i);
        }
    }
    by_name
        .into_iter()
        .map(|(name, line)| Assignment { name, line })
        .collect()
}

/// Потребности текстовой ноды: `$имя`-ссылки формульных строк (фенсы
/// исключены сканом листа) без поставщика-проливания. Дедуп по имени.
fn text_demands(sheet: &Sheet, spilled: &BTreeMap<&str, ()>) -> Vec<Demand> {
    let mut found = std::collections::BTreeSet::new();
    for (i, kind) in sheet.kinds.iter().enumerate() {
        if !matches!(
            kind,
            Some(NumiLineKind::Assignment { .. } | NumiLineKind::Expression)
        ) {
            continue;
        }
        let Some(statement) = sheet.statement(i) else {
            continue;
        };
        let Ok(parsed) = expr::parse(statement) else {
            continue;
        };
        let mut refs = Vec::new();
        collect_refs(&parsed, &mut refs);
        for r in refs {
            if let Ref::Param(name) = r {
                if !spilled.contains_key(name.as_str()) {
                    found.insert(name);
                }
            }
        }
    }
    found.into_iter().map(|param| Demand { param }).collect()
}

/// Карта «параметр занят проливанием» для ноды `to_node`: имя → ()
/// (значение не нужно — только множество; BTreeMap для единообразия).
fn spilled_params(canvas: &Canvas, to_node: &str) -> BTreeMap<String, ()> {
    let mut map = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != to_node || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        if let Some(param) = &edge.to_param {
            map.insert(param.clone(), ());
        }
    }
    map
}

/// Живое ли value-ребро с данным проливанием уже есть (дубликат, AC-5.4)?
fn spill_edge_exists(canvas: &Canvas, from: &str, to: &str, param: &str) -> bool {
    canvas.edges.iter().any(|edge| {
        edge.from_node == from
            && edge.to_node == to
            && edge.flow_kind() == FlowKind::Value
            && edge.to_param.as_deref() == Some(param)
    })
}

/// Единица значения присваивания (для бонус-признака Q2): RHS разбирается
/// и вычисляется в пустом окружении — константы (`12 %`, `500 rps`)
/// дают единицу, ссылки на переменные — `None` (честное «неизвестно»).
fn assignment_value_unit(canvas: &Canvas, node_id: &str, line: usize) -> Option<expr::Value> {
    let node = canvas.node(node_id)?;
    let sheet = Sheet::build(node);
    let statement = sheet.statement(line)?;
    let parsed = expr::parse(statement).ok()?;
    // Присваивание: значение строки — значение RHS (зеркало eval_line)
    let rhs = match parsed {
        expr::Expr::Assign { rhs, .. } => *rhs,
        other => other,
    };
    expr::eval(&rhs, &expr::Env::empty()).ok()
}

/// Найти предложения автосвязи по точным именам присваиваний (§7.2 F-7).
///
/// Детерминизм (§9.4): обход нод — порядок `canvas.nodes`; потребности
/// и присваивания — по алфавиту имён; сортировка результата —
/// `(from_node, to_node, param)`. Чистая функция: модель не мутируется.
pub fn find_proposals(canvas: &Canvas) -> Vec<AutolinkProposal> {
    // 1. Скан листов всех нод (один проход, как в build_lineage)
    let sheets: BTreeMap<&str, Sheet> = canvas
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), Sheet::build(node)))
        .collect();

    // 2. Источники: присваивания текстовых нод (шаблонные листы —
    //    параметры снапшота, не именованные выходы; v2)
    let mut sources: BTreeMap<String, Vec<(&str, usize)>> = BTreeMap::new();
    for node in &canvas.nodes {
        if node.template().is_some() {
            continue;
        }
        let sheet = sheets
            .get(node.id.as_str())
            .expect("лист построен для каждой ноды");
        for assignment in assignments_of(sheet) {
            sources
                .entry(assignment.name.clone())
                .or_default()
                .push((node.id.as_str(), assignment.line));
        }
    }
    // Стабильный порядок источников внутри имени — порядок canvas.nodes
    // (push в цикле выше уже даёт его; сортировка не требуется)

    // 3. Пары «исток → приёмник» до фильтров
    let mut raw: Vec<(String, String, String, usize)> = Vec::new(); // (from, to, param, from_line)
    for to_node in &canvas.nodes {
        let spilled = spilled_params(canvas, &to_node.id);
        // Шаблонная нода: параметры снапшота без поставщика-проливания
        // (BTreeMap — имена уже в алфавитном порядке, детерминизм)
        let mut demands: Vec<Demand> = Vec::new();
        if let Some(template) = to_node.template() {
            for name in template.params.keys() {
                if !spilled.contains_key(name.as_str()) {
                    demands.push(Demand {
                        param: name.clone(),
                    });
                }
            }
        } else {
            let sheet = sheets
                .get(to_node.id.as_str())
                .expect("лист построен для каждой ноды");
            let spilled_refs: BTreeMap<&str, ()> =
                spilled.keys().map(|k| (k.as_str(), ())).collect();
            demands = text_demands(sheet, &spilled_refs);
        }
        for demand in demands {
            let Some(candidates) = sources.get(&demand.param) else {
                continue;
            };
            for (from_id, from_line) in candidates {
                if *from_id == to_node.id.as_str() {
                    continue; // самосвязь не предлагается
                }
                raw.push((
                    (*from_id).to_owned(),
                    to_node.id.clone(),
                    demand.param.clone(),
                    *from_line,
                ));
            }
        }
    }
    raw.sort_by(|a, b| (&a.0, &a.1, &a.2).cmp(&(&b.0, &b.1, &b.2)));

    // 4. Фильтры (AC-5.4): дубликаты предложений, циклы; бонус единиц (Q2)
    let mut seen: std::collections::BTreeSet<(String, String, String)> =
        std::collections::BTreeSet::new();
    let mut proposals = Vec::new();
    for (from, to, param, from_line) in raw {
        if !seen.insert((from.clone(), to.clone(), param.clone())) {
            continue;
        }
        if spill_edge_exists(canvas, &from, &to, &param) {
            continue; // дубликат существующего ребра с той же адресацией
        }
        if creates_value_cycle(canvas, &from, &to) {
            continue; // цикл в данных
        }
        // Бонус-признак Q2: единицы сравнимы только у шаблонных приёмников
        // со спецификацией единицы параметра и константного RHS истока
        let unit_match = canvas
            .node(&to)
            .and_then(|target| target.template())
            .and_then(|template| {
                let unit = template.params.get(&param)?.unit.clone()?;
                let value = assignment_value_unit(canvas, &from, from_line)?;
                Some(value_param_compatible(&value, Some(unit.as_str())))
            });
        proposals.push(AutolinkProposal {
            from_node: from,
            from_line,
            to_node: to,
            param,
            percent: 100,
            unit_match,
        });
    }
    proposals
}

// --- Тесты (§9.4 autolink) ------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    /// Текстовая нода с Numi-листом.
    fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
        canvas.nodes.push(Node::text(id, id, x, 0.0));
        canvas.nodes.last_mut().expect("нода").text = Some(text.to_owned());
    }

    /// Value-ребро без адресации.
    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        canvas.add_edge(edge);
    }

    /// AC-5.1: точный матч присваивания истока с `$имя`-ссылкой приёмника.
    #[test]
    fn exact_name_match_proposes_value_edge() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1, "одно предложение");
        let p = &proposals[0];
        assert_eq!(p.from_node, "A");
        assert_eq!(p.to_node, "B");
        assert_eq!(p.param, "rate");
        assert_eq!(p.from_line, 0, "строка присваивания");
        assert_eq!(p.percent, 100, "точный матч (Q2)");
    }

    /// Обратная пара тоже находится: ссылка у A, присваивание у B.
    #[test]
    fn direction_follows_demand() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "total = $rate × 3", 0.0);
        calc_node(&mut canvas, "B", "rate = 12 %", 300.0);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].from_node, "B");
        assert_eq!(proposals[0].to_node, "A");
    }

    /// AC-5.4: дубликат — существующее value-ребро с той же адресацией
    /// (проливание из того же источника в тот же параметр) глушит
    /// предложение; позиционное ребро той же пары — нет (другая адресация:
    /// слот `$1` ≠ проливание `$rate`, оба имеют право на жизнь).
    #[test]
    fn existing_edge_is_not_duplicated() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        // Позиционное ребро пары — предложение проливания остаётся:
        let mut positional = Edge::new("e1", "A", None, "B", None);
        positional.set_flow_kind(FlowKind::Value);
        canvas.add_edge(positional);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1, "позиционное ребро — не дубликат спилла");
        // То же проливание A→B rate — дубликат:
        let mut spill = Edge::new("e2", "A", None, "B", None);
        spill.set_flow_kind(FlowKind::Value);
        spill.from_output = Some("rate".to_owned());
        spill.to_param = Some("rate".to_owned());
        canvas.add_edge(spill);
        assert!(
            find_proposals(&canvas).is_empty(),
            "ребро с той же адресацией уже есть"
        );
    }

    /// AC-5.4: параметр, занятый проливанием из другого источника, —
    /// потребности нет (конфликт «один вход на параметр», FR-050 Н4).
    #[test]
    fn occupied_param_produces_no_demand() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %", 0.0);
        calc_node(&mut canvas, "C", "rate = 15 %", 150.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        let mut spill = Edge::new("e1", "C", None, "B", None);
        spill.set_flow_kind(FlowKind::Value);
        spill.to_param = Some("rate".to_owned());
        canvas.add_edge(spill);
        assert!(
            find_proposals(&canvas).is_empty(),
            "параметр занят — предложений нет"
        );
    }

    /// AC-5.4: цикл — A питается от B, предложение B→A не создаётся.
    #[test]
    fn cycle_is_filtered() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "B", "rate = 12 %", 0.0);
        calc_node(&mut canvas, "A", "total = $in × 2", 300.0);
        // A уже питается от B (позиционный слот) — ребро B→A есть;
        // замкнуть: предложение A→B по $rate у B
        value_edge(&mut canvas, "e1", "B", "A");
        calc_node(&mut canvas, "C", "helper = 1", 600.0);
        // Потребность у B нет; проверим фильтр цикла через третью ноду:
        // D ссылается $total (A), A питается от B, B... строим цепь
        // B → A (есть) и предложение A → B по $rate у B
        let sheet_b = "rate = $total × 2";
        canvas.nodes[0].text = Some(sheet_b.to_owned());
        let proposals = find_proposals(&canvas);
        assert!(
            !proposals
                .iter()
                .any(|p| p.from_node == "A" && p.to_node == "B"),
            "предложение A→B замкнуло бы цикл B→A→B — фильтр обязан сработать"
        );
    }

    /// Детерминизм порядка (§9.4): две пары — сортировка (from, to, param),
    /// повторный вызов даёт идентичный список.
    #[test]
    fn deterministic_order() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %\nfee = 100 $", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        calc_node(&mut canvas, "C", "sum = $fee + $rate", 600.0);
        let first = find_proposals(&canvas);
        let second = find_proposals(&canvas);
        assert_eq!(first, second, "детерминизм: два прогона совпадают");
        let keys: Vec<(String, String, String)> = first
            .iter()
            .map(|p| (p.from_node.clone(), p.to_node.clone(), p.param.clone()))
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "порядок (from, to, param)");
        // A → B (rate), A → C (fee), A → C (rate): три предложения
        assert_eq!(first.len(), 3, "A питает B и C двумя именами");
    }

    /// AC-5.5 (перепроверка при переименовании): переименование присваивания
    /// меняет набор предложений (детектор — чистая функция над моделью,
    /// фоновый перескан после правки даёт свежий список).
    #[test]
    fn rename_recheck_updates_proposals() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        assert_eq!(find_proposals(&canvas).len(), 1);
        // Переименование источника: rate → npl
        canvas.nodes[0].text = Some("npl = 12 %".to_owned());
        assert!(
            find_proposals(&canvas).is_empty(),
            "старое имя пропало — предложение исчезло"
        );
        // Переименование ссылки приёмника: $rate → $npl
        canvas.nodes[1].text = Some("total = $npl × 3".to_owned());
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1, "новое имя появилось");
        assert_eq!(proposals[0].param, "npl");
    }

    /// Проза и код-фенсы не участвуют (скан листа — как в движке).
    #[test]
    fn prose_and_fences_are_ignored() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "встреча в 3\n```\nrate = 12 %\n```", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        assert!(
            find_proposals(&canvas).is_empty(),
            "присваивание в фенсе/проза — не присваивание"
        );
    }

    /// Самопотребность (нода ссылается на собственное имя) — не предложение.
    #[test]
    fn self_demand_is_skipped() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %\ntotal = $rate × 3", 0.0);
        // ВНИМАНИЕ: $rate — Param-ссылка; в текстовой ноде это потребность,
        // но источник — та же нода: самосвязь отфильтрована
        assert!(find_proposals(&canvas).is_empty());
    }

    /// Шаблонная нода-приёмник: параметры снапшота без проливания —
    /// потребности; бонус единиц Q2 — `Some` при константном RHS.
    #[test]
    fn template_param_demand_with_unit_bonus() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 12 %", 0.0);
        let mut node = Node::text("T", "Сетка", 300.0, 0.0);
        let mut params = BTreeMap::new();
        params.insert(
            "rate".to_owned(),
            crate::templates::TemplateParam {
                num: 0.0,
                unit: Some("%".to_owned()),
            },
        );
        node.set_template(Some(crate::templates::TemplateRef {
            id: "grid".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rate × 3".to_owned(),
            params,
            icon: "custom".to_owned(),
            color: "#4f8cff".to_owned(),
            name: Some("Сетка".to_owned()),
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1, "параметр шаблона — потребность");
        assert_eq!(proposals[0].to_node, "T");
        assert_eq!(proposals[0].param, "rate");
        assert_eq!(
            proposals[0].unit_match,
            Some(true),
            "% против % — совместимы (Q2 бонус)"
        );
    }

    /// Единицы конфликтуют — бонус-признак честно `Some(false)`.
    #[test]
    fn unit_mismatch_reported() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 500 rps", 0.0);
        let mut node = Node::text("T", "Сетка", 300.0, 0.0);
        let mut params = BTreeMap::new();
        params.insert(
            "rate".to_owned(),
            crate::templates::TemplateParam {
                num: 0.0,
                unit: Some("%".to_owned()),
            },
        );
        node.set_template(Some(crate::templates::TemplateRef {
            id: "grid".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rate × 3".to_owned(),
            params,
            icon: "custom".to_owned(),
            color: "#4f8cff".to_owned(),
            name: Some("Сетка".to_owned()),
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].unit_match, Some(false), "rps против %");
    }

    /// Повторное присваивание имени: источником адресуется ПОСЛЕДНЕЕ
    /// определение (семантика именованных выходов FR-029).
    #[test]
    fn last_assignment_wins() {
        let mut canvas = Canvas::default();
        calc_node(&mut canvas, "A", "rate = 1 %\nrate = 12 %", 0.0);
        calc_node(&mut canvas, "B", "total = $rate × 3", 300.0);
        let proposals = find_proposals(&canvas);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].from_line, 1, "последнее присваивание");
    }
}
