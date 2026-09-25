//! PRD-0007 F-2 (X1): lineage — дерево происхождения цифры.
//!
//! Чистая детерминированная функция [`build_lineage`] собирает полное
//! рекурсивное дерево происхождения выбранной цифры (итог ноды или
//! построчный результат) из уже существующих связей модели — новая
//! сущность не вводится (§7.1 п.1 PRD-0007): value-рёбра с адресацией
//! (слоты `$1..$N`/`$in`, проливания `toParam`, именованные выходы
//! `fromOutput` — FR-014/FR-025/FR-029), локальные переменные Numi-листа
//! (последнее присваивание до строки — семантика движка FR-013) и спиллы
//! параметров шаблонных нод (FR-018). Движок расчётов не меняется
//! (`flow.rs`/`expr.rs` — только чтение, §10 PRD-0007).
//!
//! Семантика дерева:
//! - узел дерева — адрес `(нода, строка)`: `line = None` — итог ноды
//!   (полоса D), `Some(i)` — значение строки `i` Numi-листа (AC-2.1);
//! - листья — константы и допущения без спец-типа: отличаются только
//!   отсутствием формулы/детей (AC-2.2, решение владельца);
//! - цикл в данных — терминальный узел [`LineageNodeKind::Cycle`] с
//!   ребром-замыкателем в `via` (AC-2.4); режим [`LineageFlow::Cycled`]
//!   строит топологию дерева, когда пересчёт упал с [`CycleError`];
//! - неподставленные входы — терминальный узел
//!   [`LineageNodeKind::Unmapped`] (AC-2.5, стиль fr-045 R-3);
//! - переменная листа без источника — терминальный узел
//!   [`LineageNodeKind::Unlinked`] «не связано» (§6.6 PRD-0007);
//! - ромб (несколько потребителей одной переменной) разворачивается в
//!   дерево без дедупликации — каждый путь к листу отдельная ветка (§9.4);
//! - бюджет [`LINEAGE_MAX_NODES`] — защитный стоп построения на плотных
//!   моделях (G5: ≤ 100 мс; превышение — [`LineageNodeKind::Truncated`]);
//! - порядок детей — порядок `canvas.edges` (детерминизм, §9.4).
//!
//! Значения узлов берутся из готовых [`FlowSolutions`] (Ready) и
//! переопределяются значением ребра для адресованных выходов
//! (`fromOutput` шаблона может отличаться от итога ноды). Дерево —
//! runtime-данные: НЕ сериализуется в `.canvas` (§9.3 PRD-0007, G6).

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::expr::{self, line_kind, NumiLineKind, Value};
use crate::flow::{
    edge_source_value_with_data, spill_source_title, CycleError, DataSnapshots, FlowKind,
    FlowSolutions, QualifiedNames,
};
use crate::model::{Canvas, Edge, Node};
use crate::templates::OutputSource;

/// Защитный бюджет узлов дерева (G5 PRD-0007): на плотных моделях полное
/// разворачивание ромбов растёт экспоненциально; превышение бюджета
/// останавливает построение — ветка помечается
/// [`LineageNodeKind::Truncated`], UI показывает её свёрнутой (§6.6,
/// «ветви свёрнуты со счётчиками» — счётчики v2).
pub const LINEAGE_MAX_NODES: usize = 4096;

/// Адрес узла дерева: `(id ноды, строка Numi-листа)`. `line = None` —
/// итог ноды (полоса D, F-1); `Some(i)` — построчный результат (FR-025).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineageNodeId {
    /// id ноды канваса.
    pub node_id: String,
    /// Индекс строки текста ноды (тот же, что в `FlowSolutions.lines`);
    /// `None` — итог ноды.
    pub line: Option<usize>,
}

impl LineageNodeId {
    /// Адрес итога ноды (полоса D).
    pub fn total(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            line: None,
        }
    }

    /// Адрес построчного результата (строка `line` Numi-листа).
    pub fn line(node_id: impl Into<String>, line: usize) -> Self {
        Self {
            node_id: node_id.into(),
            line: Some(line),
        }
    }
}

/// Род узла дерева (AC-2.1/AC-2.2, AC-2.4/AC-2.5, §6.6 PRD-0007).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineageNodeKind {
    /// Расчётный узел: есть дети (входы цепочки).
    Calc,
    /// Лист-константа/допущение: детей нет, формулы нет (у листа может
    /// быть только значение); спец-типа нет — решение владельца (AC-2.2).
    Leaf,
    /// Терминальный узел «цикл»: на пути построения встретился уже
    /// раскрытый узел; ребро-замыкатель — в `via` родителя (AC-2.4).
    Cycle,
    /// Терминальный узел «значение не подставлено»: value-ребро есть,
    /// значения нет (fr-045 R-3, AC-2.5).
    Unmapped,
    /// Терминальный узел «не связано»: переменная листа без источника
    /// (§6.6 PRD-0007); автосвязь F-7 достраивает цепочку после ревью.
    Unlinked,
    /// Защитный стоп построения: бюджет [`LINEAGE_MAX_NODES`] исчерпан
    /// (G5); ветка показывается свёрнутой.
    Truncated,
}

/// Ребро канваса, через которое значение пришло в родителя, — адрес
/// подсветки цепочки на канвасе (F-4/AC-3.1: подсвечивается ребро;
/// в пучке — доминантное ребро, `bundles.rs:360`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageVia {
    /// id value-ребра.
    pub edge_id: String,
    /// id ноды-источника.
    pub from_node: String,
    /// id ноды-приёмника.
    pub to_node: String,
    /// Построчная адресация истока (FR-025).
    pub from_line: Option<usize>,
    /// Именованный выход истока (FR-029).
    pub from_output: Option<String>,
    /// Проливание в параметр приёмника (FR-029).
    pub to_param: Option<String>,
}

impl LineageVia {
    fn of(edge: &Edge) -> Self {
        Self {
            edge_id: edge.id.clone(),
            from_node: edge.from_node.clone(),
            to_node: edge.to_node.clone(),
            from_line: edge.from_line,
            from_output: edge.from_output.clone(),
            to_param: edge.to_param.clone(),
        }
    }
}

/// Ребро дерева: индекс узла-ребёнка в [`LineageTree::nodes`] + ребро
/// канваса, через которое пришло значение (`None` — локальная переменная
/// Numi-листа или переход «итог → последняя формульная строка»).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageChild {
    /// Индекс ребёнка в [`LineageTree::nodes`] (родитель всегда раньше
    /// ребёнка — DFS-порядок построения).
    pub child: usize,
    /// Ребро канваса для подсветки (см. [`LineageVia`]).
    pub via: Option<LineageVia>,
}

/// Узел дерева происхождения (AC-2.1: значение на каждом узле; расчётные
/// узлы несут формулу строки, листья — только значение).
#[derive(Debug, Clone, PartialEq)]
pub struct LineageNode {
    /// id ноды канваса (для подсветки канваса и клика канвас→дерево, У2).
    pub node_id: String,
    /// Индекс строки Numi-листа; `None` — итог ноды.
    pub line: Option<usize>,
    /// Род узла (см. [`LineageNodeKind`]).
    pub kind: LineageNodeKind,
    /// Значение цифры: `Ok(Value)` — вычислено; `Err(текст)` — ошибка
    /// вычисления ноды (показывается терминально); `None` — значения нет
    /// (unmapped/цикл/усечение — значение не подставлено).
    pub value: Option<Result<Value, String>>,
    /// Формула узла: текст строки листа или формула ноды
    /// (`canvasdesk.expr`/шаблон); у листа нет (AC-2.2).
    pub formula: Option<String>,
    /// Заголовок ноды-таблицы (тот же, что у подписей проливания FR-029).
    pub title: String,
    /// Метка терминального узла: имя переменной («не связано»), описание
    /// входа («вход $2»/«параметр rps»); у расчётных узлов и листьев нет.
    pub label: Option<String>,
    /// Дети в порядке `canvas.edges` (детерминизм §9.4).
    pub children: Vec<LineageChild>,
}

/// Дерево происхождения цифры — единый источник для окна проверки и
/// подсветки канваса (инвариант F-5: оба рендера потребляют одну модель
/// одного снапшота). Runtime-данные: НЕ сериализуется (G6).
#[derive(Debug, Clone, PartialEq)]
pub struct LineageTree {
    /// Адрес корня — цифра, чью цепочку объясняем.
    pub root: LineageNodeId,
    /// Узлы в DFS-порядке построения (родитель раньше ребёнка); ромб
    /// разворачивается без дедупликации — один узел канваса может
    /// встречаться несколько раз (§9.4).
    pub nodes: Vec<LineageNode>,
}

impl LineageTree {
    /// Индекс ПЕРВОГО узла с данным адресом (ромб даёт дубликаты адресов).
    pub fn index_of(&self, id: &LineageNodeId) -> Option<usize> {
        self.nodes
            .iter()
            .position(|n| n.node_id == id.node_id && n.line == id.line)
    }

    /// Первый узел с данным адресом (удобство для UI/MCP).
    pub fn node(&self, id: &LineageNodeId) -> Option<&LineageNode> {
        self.index_of(id).map(|i| &self.nodes[i])
    }
}

/// Ошибка построения дерева (§9.4: проза как корень, нода без результата).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LineageError {
    /// Ноды с таким id нет на канвасе (адрес `?` протух после удаления).
    #[error("нода «{0}» не найдена")]
    RootNotFound(String),
    /// Выбранная цифра не является вычисляемой: строка-проза, строка
    /// внутри код-фенса, нода без результата (§6.6, §9.4).
    #[error("выбранная цифра «{0}» не имеет вычислимого значения")]
    RootNotANumber(String),
}

/// Результаты пересчёта для построения дерева.
#[derive(Debug, Clone, Copy)]
pub enum LineageFlow<'a> {
    /// Пересчёт прошёл: значения и построчные решения доступны.
    Ready {
        /// Полные решения [`propagate_with_lines_data`] (`flow.rs:375`).
        solutions: &'a FlowSolutions,
        /// Снапшоты CSV-источников (FR-045 R-2; пустая карта — ок).
        data: &'a DataSnapshots,
    },
    /// Пересчёт упал с [`CycleError`]: значений нет нигде, но топология
    /// канваса читаема — дерево строится с терминальными узлами «цикл»
    /// на путях, возвращающихся к уже раскрытым узлам (AC-2.4).
    Cycled(&'a CycleError),
}

/// Построить дерево происхождения цифры `root` (§7.2 PRD-0007).
///
/// Обход — рекурсивный DFS вверх по зависимостям (обобщение
/// `value_path` `flow.rs:870` с полным разворачиванием в дерево); цикл
/// отслеживается по текущему пути построения. Бюджет —
/// [`LINEAGE_MAX_NODES`]. Чистая функция: модель не мутируется, 0
/// side-эффектов.
pub fn build_lineage(
    canvas: &Canvas,
    flow: LineageFlow<'_>,
    root: LineageNodeId,
) -> Result<LineageTree, LineageError> {
    // Листы всех нод — род каждой строки (фенсы учитываются, как в
    // eval_lines_with_env expr.rs:1277) — считаются один раз (§9.2:
    // один батч-обход без per-frame аллокаций).
    let mut sheets: HashMap<String, Sheet> = HashMap::new();
    for node in &canvas.nodes {
        sheets.insert(node.id.clone(), Sheet::build(node));
    }
    validate_root(canvas, &flow, &sheets, &root)?;
    let mut builder = Builder {
        canvas,
        flow,
        sheets,
        path: HashSet::new(),
        nodes: Vec::new(),
        truncated_idx: None,
    };
    let tree_root = root.clone();
    builder.run(tree_root);
    Ok(LineageTree {
        root,
        nodes: builder.nodes,
    })
}

/// Валидация корня (§9.4): нода существует, выбранная цифра — формула.
fn validate_root(
    canvas: &Canvas,
    flow: &LineageFlow<'_>,
    sheets: &HashMap<String, Sheet>,
    root: &LineageNodeId,
) -> Result<(), LineageError> {
    if canvas.node(&root.node_id).is_none() {
        return Err(LineageError::RootNotFound(root.node_id.clone()));
    }
    match root.line {
        Some(i) => {
            let formula_kind = sheets.get(&root.node_id).and_then(|s| s.kinds.get(i));
            let formula_kind = match formula_kind {
                None => return Err(LineageError::RootNotFound(root.node_id.clone())),
                Some(kind) => kind.clone(),
            };
            if !matches!(
                formula_kind,
                Some(NumiLineKind::Assignment { .. } | NumiLineKind::Expression)
            ) {
                // проза или код-фенс — «панель с ошибкой выбора» (§6.6)
                return Err(LineageError::RootNotANumber(root.node_id.clone()));
            }
            if let LineageFlow::Ready { solutions, .. } = flow {
                if !solutions.lines.contains_key(&(root.node_id.clone(), i)) {
                    // строка-формула не вычислилась (ошибка) — значения нет
                    return Err(LineageError::RootNotANumber(root.node_id.clone()));
                }
            }
            Ok(())
        }
        None => {
            let has_digit = match flow {
                LineageFlow::Ready { solutions, .. } => {
                    solutions.outputs.contains_key(&root.node_id)
                }
                LineageFlow::Cycled(_) => {
                    let node = canvas.node(&root.node_id);
                    node.is_some_and(|n| {
                        n.template().is_some()
                            || n.expr().is_some()
                            || sheets
                                .get(&root.node_id)
                                .is_some_and(|s| s.last_formula().is_some())
                    })
                }
            };
            if has_digit {
                Ok(())
            } else {
                Err(LineageError::RootNotANumber(root.node_id.clone()))
            }
        }
    }
}

/// Структура Numi-листа ноды: род каждой строки (фенсы — как в движке,
/// `eval_lines_with_env`) + текст строк после снятия экранирования `\=`.
/// X4 (autolink): `pub(crate)` — детектор автосвязи читает тот же род
/// строк/фенсов, что и дерево (один источник истины скана листа).
pub(crate) struct Sheet {
    /// Род строки: `Some(Assignment/Expression/Prose)` вне фенсов;
    /// `None` — маркер фенса или строка внутри фенса (не считается).
    pub(crate) kinds: Vec<Option<NumiLineKind>>,
    /// Текст строк (после снятия `\=` — как в `eval_lines_with_env`).
    pub(crate) texts: Vec<String>,
}

impl Sheet {
    pub(crate) fn build(node: &Node) -> Self {
        let source = node.text.as_deref().unwrap_or_default().replace("\\=", "=");
        let mut kinds = Vec::new();
        let mut texts = Vec::new();
        let mut in_fence = false;
        for line in source.split('\n') {
            texts.push(line.to_owned());
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                kinds.push(None);
                continue;
            }
            kinds.push(if in_fence {
                None
            } else {
                Some(line_kind(line))
            });
        }
        Self { kinds, texts }
    }

    /// Последняя присваивающая строка имени ДО строки `before`
    /// (последовательная семантика вычисления листа, FR-013).
    fn last_assignment_before(&self, name: &str, before: usize) -> Option<usize> {
        (0..before.min(self.kinds.len())).rev().find(|&i| {
            matches!(
                &self.kinds[i],
                Some(NumiLineKind::Assignment { name: n }) if n == name
            )
        })
    }

    /// Последнее присваивание имени во всём листе (семантика именованных
    /// выходов FR-029: «последнее определение имени»).
    fn last_assignment(&self, name: &str) -> Option<usize> {
        (0..self.kinds.len()).rev().find(
            |&i| matches!(&self.kinds[i], Some(NumiLineKind::Assignment { name: n }) if n == name),
        )
    }

    /// Последняя формульная строка листа — источник итога текстовой ноды
    /// (FR-013: «итог заметки — последняя формульная строка»).
    fn last_formula(&self) -> Option<usize> {
        (0..self.kinds.len()).rev().find(|&i| {
            matches!(
                &self.kinds[i],
                Some(NumiLineKind::Assignment { .. } | NumiLineKind::Expression)
            )
        })
    }

    /// Вычисляемое утверждение строки (после снятия `\=` и префикса `=`)
    /// — зеркало `eval_line` (`expr.rs:1306`).
    pub(crate) fn statement(&self, i: usize) -> Option<&str> {
        let trimmed = self.texts.get(i)?.trim();
        Some(trimmed.strip_prefix('=').map(str::trim).unwrap_or(trimmed))
    }
}

/// Ссылка выражения на источник значения (зеркало разрешения имён
/// движком: [`Env`] expr.rs:419 — входы, параметры, переменные листа).
/// X4 (autolink): `pub(crate)` — детектор автосвязи матчует `Param`-ссылки
/// (`$имя`) с именами присваиваний других нод.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ref {
    /// `$in` — единственный вход (при нескольких — ошибка движка).
    InAll,
    /// `$N` (целое ≥ 1 при наличии входов) — N-й позиционный вход.
    Slot(usize),
    /// `$имя` — параметр: проливание (`toParam`, сильнее дефолта) или
    /// локальный параметр шаблона (литерал — лист дерева).
    Param(String),
    /// Переменная Numi-листа (присваивание выше по листу).
    Var(String),
    /// FR-050 Р-6: именованный путь «Объект.Поле» — входящее значение по
    /// имени (резолв — входящие value-рёбра приёмника, все адресные формы
    /// имени истока × поле, `flow::QualifiedNames`).
    Qualified(String, String),
}

/// Собрать ссылки выражения (порядок обхода AST, без дедупликации —
/// дедуп внутри строки делается на уровне children).
pub(crate) fn collect_refs(expr: &expr::Expr, out: &mut Vec<Ref>) {
    match expr {
        expr::Expr::Num(..) => {}
        expr::Expr::Var(name) => out.push(Ref::Var(name.clone())),
        expr::Expr::Neg(inner) => collect_refs(inner, out),
        expr::Expr::Bin { lhs, rhs, .. } => {
            collect_refs(lhs, out);
            collect_refs(rhs, out);
        }
        expr::Expr::Call { args, .. } => {
            for arg in args {
                collect_refs(arg, out);
            }
        }
        // Утверждение `name = rhs`: значение строки — значение rhs.
        expr::Expr::Assign { rhs, .. } => collect_refs(rhs, out),
        expr::Expr::Block(items) => {
            for item in items {
                collect_refs(item, out);
            }
        }
        expr::Expr::Inbound => out.push(Ref::InAll),
        expr::Expr::DollarAmount(n) if n.fract() == 0.0 && *n >= 1.0 => {
            out.push(Ref::Slot(*n as usize - 1));
        }
        // Дробные `$1.5` и `$0` — валюта (константа, expr.rs:408).
        expr::Expr::DollarAmount(..) => {}
        expr::Expr::Param(name) => out.push(Ref::Param(name.clone())),
        // FR-050 Р-6: именованный путь «Объект.Поле» — вход по имени.
        expr::Expr::Qualified { obj, field } => {
            out.push(Ref::Qualified(obj.clone(), field.clone()))
        }
    }
}

/// Спецификация ребёнка до разворачивания (разделено с `expand`, чтобы
/// сборка спецификаций оставалась чистой функцией над `&self`).
enum ChildSpec {
    /// Обычный переход: цель + значение ребра (override, Ready) + ребро.
    Link {
        target: LineageNodeId,
        value: Option<Value>,
        via: Option<LineageVia>,
    },
    /// Неподставленный вход (AC-2.5).
    Unmapped {
        target: LineageNodeId,
        via: LineageVia,
        label: String,
    },
    /// Переменная листа без источника — «не связано» (§6.6).
    Unlinked { name: String },
}

struct Builder<'a> {
    canvas: &'a Canvas,
    flow: LineageFlow<'a>,
    sheets: HashMap<String, Sheet>,
    /// Адреса на текущем пути DFS — детектор цикла (AC-2.4).
    path: HashSet<LineageNodeId>,
    nodes: Vec<LineageNode>,
    /// Ленивый общий маркер усечения (создаётся один раз при исчерпании
    /// бюджета; все переграничные ссылки указывают на него).
    truncated_idx: Option<usize>,
}

impl<'a> Builder<'a> {
    /// Итеративный DFS построения (явный стек задач вместо рекурсии:
    /// глубина цепочки ограничена кучей, а не стеком потока — цепочки
    /// 1000+ нод и глубже не переполняют стек, §9.2/G5).
    ///
    /// Порядок детей — порядок спек (обратная укладка задач; LIFO даёт
    /// прямой порядок); Exit-задача снимает адрес с пути и вычисляет
    /// итоговый род узла (Leaf/Calc по числу детей).
    fn run(&mut self, root: LineageNodeId) {
        enum Task {
            /// Развернуть узел (push узла + задачи детей + Exit).
            Enter {
                id: LineageNodeId,
                override_value: Option<Value>,
                via: Option<LineageVia>,
                parent: Option<usize>,
            },
            /// Готовый терминал (unmapped/не связано — без разворачивания).
            Terminal {
                id: LineageNodeId,
                kind: LineageNodeKind,
                via: Option<LineageVia>,
                label: Option<String>,
                parent: Option<usize>,
            },
            /// Все дети узла обработаны: снять с пути, финализировать род.
            Exit { idx: usize, id: LineageNodeId },
        }
        let attach = |nodes: &mut Vec<LineageNode>,
                      parent: Option<usize>,
                      child: usize,
                      via: Option<LineageVia>| {
            if let Some(p) = parent {
                nodes[p].children.push(LineageChild { child, via });
            }
        };
        let mut tasks = vec![Task::Enter {
            id: root,
            override_value: None,
            via: None,
            parent: None,
        }];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Terminal {
                    id,
                    kind,
                    via,
                    label,
                    parent,
                } => {
                    if self.nodes.len() >= LINEAGE_MAX_NODES - 1 {
                        let marker = self.truncated_marker();
                        attach(&mut self.nodes, parent, marker, via);
                        continue;
                    }
                    let idx = self.push_terminal(id, kind, label);
                    attach(&mut self.nodes, parent, idx, via);
                }
                Task::Exit { idx, id } => {
                    self.path.remove(&id);
                    let kind = if self.nodes[idx].children.is_empty() {
                        LineageNodeKind::Leaf
                    } else {
                        LineageNodeKind::Calc
                    };
                    self.nodes[idx].kind = kind;
                }
                Task::Enter {
                    id,
                    override_value,
                    via,
                    parent,
                } => {
                    // Бюджет (G5): один слот резервируется под общий
                    // маркер усечения; переграничные ссылки — на него.
                    if self.nodes.len() >= LINEAGE_MAX_NODES - 1 {
                        let marker = self.truncated_marker();
                        attach(&mut self.nodes, parent, marker, via);
                        continue;
                    }
                    // Цикл: адрес уже на пути построения (AC-2.4);
                    // ребро-замыкатель несёт ссылка родителя (via).
                    if self.path.contains(&id) {
                        let idx = self.push_terminal(id, LineageNodeKind::Cycle, None);
                        attach(&mut self.nodes, parent, idx, via);
                        continue;
                    }
                    // Цель не существует (висячее ребро в Cycled-режиме) —
                    // значения нет (Ready отсекает это на уровне спеков).
                    let Some(node) = self.canvas.node(&id.node_id) else {
                        let idx = self.push_terminal(id, LineageNodeKind::Unmapped, None);
                        attach(&mut self.nodes, parent, idx, via);
                        continue;
                    };
                    let specs = self.child_specs(&id);
                    let idx = self.nodes.len();
                    let value = match override_value {
                        Some(v) => Some(Ok(v)),
                        None => self.value_at(&id),
                    };
                    let formula = self.formula_of(node, &id);
                    self.nodes.push(LineageNode {
                        node_id: id.node_id.clone(),
                        line: id.line,
                        kind: LineageNodeKind::Calc,
                        value,
                        formula,
                        title: spill_source_title(node),
                        label: None,
                        children: Vec::new(),
                    });
                    self.path.insert(id.clone());
                    attach(&mut self.nodes, parent, idx, via);
                    // Exit укладывается ПЕРВЫМ (LIFO снимет его последним,
                    // после всех детей); дети — в обратном порядке, чтобы
                    // pop дал исходный порядок спек.
                    tasks.push(Task::Exit { idx, id });
                    for spec in specs.into_iter().rev() {
                        let task = match spec {
                            ChildSpec::Link { target, value, via } => Task::Enter {
                                id: target,
                                override_value: value,
                                via,
                                parent: Some(idx),
                            },
                            ChildSpec::Unmapped { target, via, label } => Task::Terminal {
                                id: target,
                                kind: LineageNodeKind::Unmapped,
                                via: Some(via),
                                label: Some(label),
                                parent: Some(idx),
                            },
                            ChildSpec::Unlinked { name } => Task::Terminal {
                                id: LineageNodeId::total(&self.nodes[idx].node_id),
                                kind: LineageNodeKind::Unlinked,
                                via: None,
                                label: Some(name),
                                parent: Some(idx),
                            },
                        };
                        tasks.push(task);
                    }
                }
            }
        }
    }

    /// Терминальный узел (цикл/unmapped/не связано/усечение). Ребро,
    /// приведшее к терминалу, несёт родитель в `LineageChild.via`.
    fn push_terminal(
        &mut self,
        id: LineageNodeId,
        kind: LineageNodeKind,
        label: Option<String>,
    ) -> usize {
        let title = self
            .canvas
            .node(&id.node_id)
            .map(spill_source_title)
            .unwrap_or_else(|| id.node_id.clone());
        self.nodes.push(LineageNode {
            node_id: id.node_id,
            line: id.line,
            kind,
            value: None,
            formula: None,
            title,
            label,
            children: Vec::new(),
        });
        self.nodes.len() - 1
    }

    /// Общий маркер усечения — ленивый одиночный узел [`
    /// LineageNodeKind::Truncated`] (пустой адрес: синтетический узел,
    /// канвасной ноды нет).
    fn truncated_marker(&mut self) -> usize {
        if let Some(idx) = self.truncated_idx {
            return idx;
        }
        let idx = self.nodes.len();
        self.nodes.push(LineageNode {
            node_id: String::new(),
            line: None,
            kind: LineageNodeKind::Truncated,
            value: None,
            formula: None,
            title: String::new(),
            label: None,
            children: Vec::new(),
        });
        self.truncated_idx = Some(idx);
        idx
    }

    /// Значение узла из решений пересчёта (Ready); Cycled — значения нет.
    fn value_at(&self, id: &LineageNodeId) -> Option<Result<Value, String>> {
        let LineageFlow::Ready { solutions, .. } = self.flow else {
            return None;
        };
        match id.line {
            Some(i) => solutions
                .lines
                .get(&(id.node_id.clone(), i))
                .cloned()
                .map(Ok),
            None => solutions
                .outputs
                .get(&id.node_id)
                .map(|r| r.clone().map_err(|e| e.to_string())),
        }
    }

    /// Формула узла: текст строки листа; для итога — формула шаблона или
    /// `canvasdesk.expr` (у текстовой ноды формулу несёт child-строка).
    fn formula_of(&self, node: &Node, id: &LineageNodeId) -> Option<String> {
        if let Some(i) = id.line {
            return self
                .sheets
                .get(&id.node_id)
                .and_then(|s| s.statement(i))
                .map(str::to_owned);
        }
        if let Some(tpl) = node.template() {
            return Some(tpl.expr.clone());
        }
        node.expr().map(str::to_owned)
    }

    /// Входящие value-рёбра ноды: позиционные слоты (порядок
    /// `canvas.edges`) и карта проливаний (последнее ребро в параметр
    /// побеждает — зеркало `inbound_values` `flow.rs:642`).
    fn slots_and_spills(&self, node_id: &str) -> (Vec<&'a Edge>, BTreeMap<String, &'a Edge>) {
        let mut slots = Vec::new();
        let mut spills: BTreeMap<String, &'a Edge> = BTreeMap::new();
        for edge in &self.canvas.edges {
            if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
                continue;
            }
            match &edge.to_param {
                None => slots.push(edge),
                Some(name) => {
                    spills.insert(name.clone(), edge);
                }
            }
        }
        (slots, spills)
    }

    /// Спецификации детей узла (§7.2): значение рёбер —
    /// `edge_source_value_with_data` (`flow.rs:612`), ссылки выражений —
    /// AST-обход (зеркало разрешения имён движка), порядок —
    /// `canvas.edges`.
    fn child_specs(&self, id: &LineageNodeId) -> Vec<ChildSpec> {
        let (slots, spills) = self.slots_and_spills(&id.node_id);
        let refs: Vec<Ref> = match id.line {
            Some(i) => {
                let Some(sheet) = self.sheets.get(&id.node_id) else {
                    return Vec::new();
                };
                let Some(statement) = sheet.statement(i) else {
                    return Vec::new();
                };
                match expr::parse(statement) {
                    Ok(parsed) => {
                        let mut refs = Vec::new();
                        collect_refs(&parsed, &mut refs);
                        refs
                    }
                    // Строка не парсится (движок дал бы ошибку) — детей нет.
                    Err(_) => Vec::new(),
                }
            }
            None => {
                let Some(node) = self.canvas.node(&id.node_id) else {
                    return Vec::new();
                };
                if let Some(tpl) = node.template() {
                    match expr::parse(&tpl.expr) {
                        Ok(parsed) => {
                            let mut refs = Vec::new();
                            collect_refs(&parsed, &mut refs);
                            refs
                        }
                        Err(_) => Vec::new(),
                    }
                } else if let Some(formula) = node.expr() {
                    match expr::parse(formula) {
                        Ok(parsed) => {
                            let mut refs = Vec::new();
                            collect_refs(&parsed, &mut refs);
                            refs
                        }
                        Err(_) => Vec::new(),
                    }
                } else {
                    // Итог текстовой ноды = последняя формульная строка
                    // (FR-013): единственный переход внутри листа.
                    return match self.sheets.get(&id.node_id).and_then(Sheet::last_formula) {
                        Some(i) => vec![ChildSpec::Link {
                            target: LineageNodeId::line(&id.node_id, i),
                            value: None,
                            via: None,
                        }],
                        None => Vec::new(),
                    };
                }
            }
        };
        // Дедупликация ссылок внутри одной строки (первое вхождение):
        // двойное упоминание `$1` — один ребёнок.
        let mut seen: Vec<Ref> = Vec::new();
        let mut specs: Vec<ChildSpec> = Vec::new();
        for r#ref in refs {
            if seen.contains(&r#ref) {
                continue;
            }
            seen.push(r#ref.clone());
            match r#ref {
                Ref::InAll => {
                    // `$in` — единственный вход; при нескольких движок дал
                    // бы AmbiguousInbound — показываем все слоты.
                    if slots.len() == 1 {
                        specs.push(self.edge_spec(slots[0], &slots, None));
                    } else {
                        for (n, edge) in slots.iter().enumerate() {
                            specs.push(self.edge_spec(edge, &slots, Some(n)));
                        }
                    }
                }
                Ref::Slot(n) => {
                    // `$N` — ссылка на вход только при наличии входов
                    // (иначе это валюта — константа, expr.rs:408).
                    if !slots.is_empty() {
                        if let Some(edge) = slots.get(n) {
                            specs.push(self.edge_spec(edge, &slots, Some(n)));
                        }
                    }
                }
                Ref::Param(name) => {
                    if let Some(edge) = spills.get(&name) {
                        specs.push(self.edge_spec(edge, &slots, None));
                    }
                    // Проливания нет — локальный параметр/литерал: лист.
                }
                Ref::Var(name) => {
                    let sheet = self.sheets.get(&id.node_id);
                    let defining = match id.line {
                        Some(i) => sheet.and_then(|s| s.last_assignment_before(&name, i)),
                        None => sheet.and_then(|s| s.last_assignment(&name)),
                    };
                    match defining {
                        Some(i) => specs.push(ChildSpec::Link {
                            target: LineageNodeId::line(&id.node_id, i),
                            value: None,
                            via: None,
                        }),
                        // Переменная без источника — «не связано» (§6.6).
                        None => specs.push(ChildSpec::Unlinked { name }),
                    }
                }
                // FR-050 Р-6: именованный путь — ребро, чей qualified-ключ
                // совпадает (все адресные формы имени истока × поле);
                // такого ребра нет — терминал «не связано» (движок в этой
                // строке дал бы UnknownInput).
                Ref::Qualified(obj, field) => {
                    let qnames = QualifiedNames::build(self.canvas);
                    let all = slots.iter().chain(spills.values());
                    let edge = all.copied().find(|edge| {
                        qnames
                            .edge_keys(self.canvas, edge)
                            .contains(&(obj.clone(), field.clone()))
                    });
                    match edge {
                        Some(edge) => {
                            let slot_no = slots.iter().position(|e| std::ptr::eq(*e, edge));
                            specs.push(self.edge_spec(edge, &slots, slot_no));
                        }
                        None => specs.push(ChildSpec::Unlinked {
                            name: format!("{obj}.{field}"),
                        }),
                    }
                }
            }
        }
        specs
    }

    /// Спецификация ребёнка через value-ребро (Ready: unmapped — значение
    /// отсутствует; data-нода — лист со значением колонки; Cycled —
    /// значения недоступны, разворачиваем топологию).
    fn edge_spec(&self, edge: &'a Edge, slots: &[&'a Edge], slot_no: Option<usize>) -> ChildSpec {
        let via = LineageVia::of(edge);
        let target = self.edge_target(edge);
        match self.flow {
            LineageFlow::Ready { solutions, data } => {
                let value = edge_source_value_with_data(
                    edge,
                    &solutions.outputs,
                    &solutions.lines,
                    &solutions.named,
                    data,
                );
                match value {
                    None => ChildSpec::Unmapped {
                        target,
                        via,
                        label: match &edge.to_param {
                            Some(name) => format!("параметр {name}"),
                            None => format!(
                                "вход ${}",
                                slot_no
                                    .or_else(|| slots.iter().position(|e| e.id == edge.id))
                                    .map(|n| n + 1)
                                    .unwrap_or(0)
                            ),
                        },
                    },
                    // Колонка CSV-источника — внешний лист: значение
                    // ребра (из снапшота), разворачивать некуда (R-2).
                    Some(v) if data.contains_key(&edge.from_node) => ChildSpec::Link {
                        target: LineageNodeId::total(&edge.from_node),
                        value: Some(v),
                        via: Some(via),
                    },
                    Some(v) => ChildSpec::Link {
                        target,
                        value: Some(v),
                        via: Some(via),
                    },
                }
            }
            LineageFlow::Cycled(_) => ChildSpec::Link {
                target,
                value: None,
                via: Some(via),
            },
        }
    }

    /// Цель value-ребра по адресации — зеркало приоритета движка
    /// (`edge_source_value` `flow.rs:577`): `fromLine` → строка листа;
    /// иначе `fromOutput` → именованный выход (шаблон: секция outputs;
    /// текстовая нода: последняя присваивающая строка имени); иначе итог.
    fn edge_target(&self, edge: &Edge) -> LineageNodeId {
        if let Some(line) = edge.from_line {
            return LineageNodeId::line(&edge.from_node, line);
        }
        if let Some(name) = &edge.from_output {
            if let Some(node) = self.canvas.node(&edge.from_node) {
                if let Some(tpl) = node.template() {
                    for spec in &tpl.outputs {
                        if spec.name == *name {
                            if let OutputSource::Line(l) = &spec.source {
                                return LineageNodeId::line(&edge.from_node, *l);
                            }
                            break;
                        }
                    }
                    return LineageNodeId::total(&edge.from_node);
                }
            }
            if let Some(i) = self
                .sheets
                .get(&edge.from_node)
                .and_then(|s| s.last_assignment(name))
            {
                return LineageNodeId::line(&edge.from_node, i);
            }
        }
        LineageNodeId::total(&edge.from_node)
    }
}

// --- What-if дельты дерева (PRD-0007 X3, AC-4.2/F-6) ----------------------

/// Дельта what-if на узле дерева: базовое и сценарное значения плюс
/// строка дельты формата FR-017 («(+33 пп)» / «(+2)»); `delta: None` не
/// встречается (совпадающие значения не попадают в карту).
#[derive(Debug, Clone, PartialEq)]
pub struct LineageDelta {
    /// Значение узла в базовой модели.
    pub base: Value,
    /// Значение узла при активных подменах.
    pub whatif: Value,
    /// Строка дельты формата FR-017 (`expr::whatif_delta_str`).
    pub delta: String,
}

/// Сопоставить базовое и what-if дерево происхождения по адресу узла
/// `(node_id, line)` и собрать дельты изменившихся значений (AC-4.2:
/// «пересчитанные значения и дельты на затронутых узлах дерева»).
///
/// Чтоif-дерево строится из `flow_active` (подмены), базовое — из
/// `flow_baseline`; оба из одного снапшота `.canvas` (инвариант F-5:
/// структура ветвей совпадает, различаются только вычисленные значения —
/// но сопоставление по адресу устойчиво и к структурным сдвигам: узлы
/// без пары просто не дают дельту). Чистая функция, порядок — BTreeMap.
pub fn lineage_deltas(
    base: &LineageTree,
    whatif: &LineageTree,
) -> BTreeMap<(String, Option<usize>), LineageDelta> {
    let mut base_values: BTreeMap<(String, Option<usize>), &Value> = BTreeMap::new();
    for node in &base.nodes {
        if let Some(Ok(value)) = &node.value {
            base_values.insert((node.node_id.clone(), node.line), value);
        }
    }
    let mut deltas = BTreeMap::new();
    for node in &whatif.nodes {
        let Some(Ok(value)) = &node.value else {
            continue;
        };
        let Some(base_value) = base_values.get(&(node.node_id.clone(), node.line)).copied() else {
            continue;
        };
        if let Some(delta) = expr::whatif_delta_str(base_value, value) {
            deltas.insert(
                (node.node_id.clone(), node.line),
                LineageDelta {
                    base: base_value.clone(),
                    whatif: value.clone(),
                    delta,
                },
            );
        }
    }
    deltas
}

// --- Текстовая выдача объяснения (F-9, X6) --------------------------------

/// Человекочитаемая линейная развёртка дерева происхождения — MCP-инструмент
/// `explain_number` (F-9, must: решение владельца «MCP-инструменты в ту же
/// итерацию»; персона 4 §4 — ИИ-агент готовится к защите).
///
/// Требование PRD-0007: «линейная развёртка дерева с адресами и значениями»:
/// каждый узел — самодостаточная строка (заголовок таблицы, адрес
/// `node_id[#строка]`, значение/терминал, формула/пометка листа, канал
/// прихода значения — выход/строка/проливание/ребро). Развёртка — DFS
/// предзаказ ([`LineageTree::nodes`] уже хранится в этом порядке), отступ
/// кодирует глубину; нумерация сквозная (родитель раньше ребёнка, ромб
/// разворачивается — как в окне проверки). Итеративный проход (глубина
/// дерева не ограничена — цепочка 1000+ нод, G5). Чистая функция.
pub fn explain_text(tree: &LineageTree) -> String {
    // Один линейный проход (предзаказ — родитель раньше ребёнка):
    // глубины всех узлов по children-индексам + канал прихода значения
    // (адресация ребра родителя) на каждый узел.
    let mut depth = vec![0usize; tree.nodes.len()];
    let mut channel: Vec<Option<String>> = vec![None; tree.nodes.len()];
    for (index, node) in tree.nodes.iter().enumerate() {
        for child in &node.children {
            if child.child > index && child.child < tree.nodes.len() {
                depth[child.child] = depth[index] + 1;
                channel[child.child] = Some(match &child.via {
                    Some(via) => {
                        let mut text = match (&via.to_param, &via.from_output, via.from_line) {
                            (Some(param), _, _) => format!(" · пролито в параметр {param}"),
                            (_, Some(output), _) => format!(" · через выход {output}"),
                            (_, _, Some(line)) => format!(" · через строку {}", line + 1),
                            (None, None, None) => " · через value-связь".to_owned(),
                        };
                        text.push_str(&format!(" (ребро {})", via.edge_id));
                        text
                    }
                    // Локальная переменная Numi-листа / переход «итог →
                    // последняя формульная строка» — ребра канваса нет.
                    None => " · локальная переменная листа".to_owned(),
                });
            }
        }
    }
    let mut out = String::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        if index == 0 {
            // Корень — заголовок развёртки (цифра, чью цепочку объясняем).
            out.push_str("Цепочка расчёта: ");
        } else {
            // Отступ глубины (2 пробела на уровень) + сквозной номер.
            for _ in 0..depth[index] {
                out.push_str("  ");
            }
            out.push_str(&format!("{index}. "));
        }
        // Заголовок таблицы + машинный адрес (node_id и строка Numi-листа).
        out.push_str(&node.title);
        out.push_str(" [");
        out.push_str(&node.node_id);
        if let Some(line) = node.line {
            out.push_str(&format!(":{line}"));
        }
        out.push(']');
        // Значение / ошибка / терминал.
        match &node.value {
            Some(Ok(value)) => out.push_str(&format!(" = {value}")),
            Some(Err(error)) => out.push_str(&format!(" — ошибка: {error}")),
            None => match node.kind {
                LineageNodeKind::Cycle => out.push_str(" — цикл (значение не определено)"),
                LineageNodeKind::Unmapped => {
                    out.push_str(" — значение не подставлено");
                }
                LineageNodeKind::Unlinked => {
                    let label = node.label.as_deref().unwrap_or("вход");
                    out.push_str(&format!(" — не связано: {label}"));
                }
                LineageNodeKind::Truncated => {
                    out.push_str(&format!(
                        " — усечено: достигнут бюджет {LINEAGE_MAX_NODES} узлов"
                    ));
                }
                _ => {}
            },
        }
        // Формула расчётного узла / пометка листа («исходное значение»).
        if let Some(formula) = &node.formula {
            out.push_str(&format!(" · формула: {formula}"));
        } else if node.kind == LineageNodeKind::Leaf {
            out.push_str(" · исходное значение");
        }
        if let Some(text) = &channel[index] {
            out.push_str(text);
        }
        out.push('\n');
    }
    // Итоговая статистика — агенту объём объяснения без парсинга строк.
    let leaves = tree
        .nodes
        .iter()
        .filter(|node| node.kind == LineageNodeKind::Leaf)
        .count();
    out.push_str(&format!(
        "Всего узлов: {} (листьев: {})",
        tree.nodes.len(),
        leaves
    ));
    out
}

// --- Покрытие цепочками (F-12, X6) -----------------------------------------

/// Статистика индикатора «Цепочки: N%» (F-12, should — решение владельца,
/// раунд 2 (в); opt-in настройка FR-039). Доля вычисляемых цифр (итоги нод
/// + построчные результаты), чья цепочка происхождения доходит до листьев.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainCoverage {
    /// Всего вычисляемых цифр: итоги нод со значением `Ok` + построчные
    /// результаты (`FlowSolutions.lines` — хранятся только вычисленные).
    pub total: usize,
    /// Из них цепочка доходит до листьев: в дереве нет терминалов
    /// «цикл»/«значение не подставлено»/«не связано»/«усечено» и значение
    /// корня — `Ok` (ошибка вычисления обрывает цепочку так же, как
    /// неподставленный вход).
    pub covered: usize,
}

impl ChainCoverage {
    /// Процент покрытия 0..=100; `None` — вычисляемых цифр нет (индикатор
    /// скрывается — процент не определён).
    pub fn percent(&self) -> Option<u8> {
        if self.total == 0 {
            return None;
        }
        Some(((self.covered as f64 / self.total as f64) * 100.0).round() as u8)
    }
}

/// Посчитать покрытие цепочками для канваса (F-12). Чистая функция над
/// той же парой (канвас, пересчёт), что [`build_lineage`]. Вычисляемые
/// цифры: итоги формульных нод (все записи — включая Err: цифра с
/// ошибкой вычисляемая, но её цепочка обрывается) + вычисляемые строки
/// Numi-листов (`Sheet::build` — фенсы и `\=` как в движке). Для каждой
/// цифры строится её дерево (та же семантика, что окно проверки —
/// инвариант F-5) и проверяется на терминалы. Опт-индикатор: вызывается
/// только при включённой настройке (FR-039), стоимость —
/// O(цифры × размер дерева), на демо-модели мгновенно, на 1000 нод —
/// фоновая нагрузка вне кадра.
pub fn chain_coverage(canvas: &Canvas, flow: LineageFlow) -> ChainCoverage {
    // Кандидаты: (node_id, line) всех вычисляемых цифр. Итоги формульных
    // нод (все записи, Ok и Err) — только в Ready; в Cycled значений нет
    // нигде (0% — модель сломана).
    let mut candidates: Vec<LineageNodeId> = match flow {
        LineageFlow::Ready { solutions, .. } => solutions
            .outputs
            .keys()
            .map(|node_id| LineageNodeId::total(node_id.clone()))
            .collect(),
        LineageFlow::Cycled(_) => Vec::new(),
    };
    // Вычисляемые строки текстовых нод (Assignment/Expression — фенсы
    // исключены, `\=` снят — тот же детектор, что у движка).
    for node in &canvas.nodes {
        let sheet = Sheet::build(node);
        for (index, kind) in sheet.kinds.iter().enumerate() {
            if matches!(
                kind,
                Some(NumiLineKind::Assignment { .. } | NumiLineKind::Expression)
            ) {
                candidates.push(LineageNodeId::line(node.id.clone(), index));
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    let mut stat = ChainCoverage {
        total: candidates.len(),
        covered: 0,
    };
    for root in candidates {
        let tree = match build_lineage(canvas, flow, root) {
            Ok(tree) => tree,
            // Построить дерево не удалось (нестандартный корень) — цифра
            // вычисляемая, но цепочка не объясняется: не покрыта.
            Err(_) => continue,
        };
        let root_ok = matches!(&tree.nodes[0].value, Some(Ok(_)));
        let clean = tree.nodes.iter().all(|node| {
            !matches!(
                node.kind,
                LineageNodeKind::Cycle
                    | LineageNodeKind::Unmapped
                    | LineageNodeKind::Unlinked
                    | LineageNodeKind::Truncated
            )
        });
        if root_ok && clean {
            stat.covered += 1;
        }
    }
    stat
}

// --- Тесты (верификационный список §9.4 PRD-0007) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::parse_csv;
    use crate::flow::{propagate_with_lines_data, WhatIfOverrides};
    use crate::model::{Edge, Node, Side};

    /// Нода-заметка с Numi-текстом.
    fn sheet(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
        canvas.nodes.push(Node::text(id, text, x, 0.0));
    }

    /// Нода с явной формулой `canvasdesk.expr`.
    fn formula(canvas: &mut Canvas, id: &str, expr: &str, x: f32) {
        let mut node = Node::text(id, id, x, 0.0);
        node.set_expr(Some(expr.to_owned()));
        canvas.nodes.push(node);
    }

    /// Value-ребро (поток значений).
    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        canvas.add_edge(edge);
    }

    /// Полные решения пересчёта (Ready).
    fn solutions(canvas: &Canvas) -> FlowSolutions {
        propagate_with_lines_data(canvas, &WhatIfOverrides::default(), &DataSnapshots::new())
            .expect("пересчёт без цикла")
    }

    /// Дерево в Ready-режиме.
    fn tree(canvas: &Canvas, root: LineageNodeId) -> LineageTree {
        let solutions = solutions(canvas);
        build_lineage(
            canvas,
            LineageFlow::Ready {
                solutions: &solutions,
                data: &DataSnapshots::new(),
            },
            root,
        )
        .expect("дерево построено")
    }

    /// Отображаемое значение узла (`Ok` → строка; иначе пусто).
    fn shown(tree: &LineageTree, index: usize) -> String {
        tree.nodes[index]
            .value
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .map(Value::to_string)
            .unwrap_or_default()
    }

    /// Дети узла (индексы).
    fn kids(tree: &LineageTree, index: usize) -> Vec<usize> {
        tree.nodes[index].children.iter().map(|c| c.child).collect()
    }

    /// §9.4, линейная цепочка: C ← B ← A; итог C = 11, значения на каждом
    /// узле (AC-2.1), листья — константы без формулы (AC-2.2).
    #[test]
    fn linear_chain_root_total() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "a", "5", 0.0);
        formula(&mut canvas, "b", "$in × 2", 1.0);
        formula(&mut canvas, "c", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "a", "b");
        value_edge(&mut canvas, "e2", "b", "c");
        let tree = tree(&canvas, LineageNodeId::total("c"));
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].kind, LineageNodeKind::Calc);
        assert_eq!(shown(&tree, 0), "11");
        let kids0 = kids(&tree, 0);
        assert_eq!(kids0, vec![1]);
        assert_eq!(
            tree.nodes[0].children[0]
                .via
                .as_ref()
                .map(|v| v.edge_id.clone()),
            Some("e2".to_owned())
        );
        assert_eq!(shown(&tree, 1), "10");
        let kids1 = kids(&tree, 1);
        assert_eq!(kids1, vec![2]);
        assert_eq!(
            tree.nodes[1].children[0]
                .via
                .as_ref()
                .map(|v| v.edge_id.clone()),
            Some("e1".to_owned())
        );
        assert_eq!(tree.nodes[2].kind, LineageNodeKind::Leaf);
        assert_eq!(shown(&tree, 2), "5");
        // У листа формула-константа видна, детей нет (AC-2.2).
        assert_eq!(tree.nodes[2].formula.as_deref(), Some("5"));
    }

    /// §9.4, ромб: несколько потребителей одной переменной — общий исток
    /// разворачивается без дедупликации, у каждого потребителя своя ветка.
    #[test]
    fn diamond_expands_without_dedup() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "a", "100", 0.0);
        formula(&mut canvas, "b", "$in × 2", 1.0);
        formula(&mut canvas, "c", "$in + 1", 2.0);
        formula(&mut canvas, "d", "$1 + $2", 3.0);
        value_edge(&mut canvas, "e1", "a", "b");
        value_edge(&mut canvas, "e2", "a", "c");
        value_edge(&mut canvas, "e3", "b", "d");
        value_edge(&mut canvas, "e4", "c", "d");
        let tree = tree(&canvas, LineageNodeId::total("d"));
        assert_eq!(shown(&tree, 0), "301");
        let root_kids = kids(&tree, 0);
        assert_eq!(root_kids.len(), 2);
        let b = root_kids[0];
        let c = root_kids[1];
        assert_eq!(tree.nodes[b].node_id, "b");
        assert_eq!(tree.nodes[c].node_id, "c");
        assert_eq!(shown(&tree, b), "200");
        assert_eq!(shown(&tree, c), "101");
        // Общий исток a — под КАЖДЫМ потребителем (дерево, не DAG).
        let a_count = tree.nodes.iter().filter(|n| n.node_id == "a").count();
        assert_eq!(a_count, 2);
        assert_eq!(shown(&tree, kids(&tree, b)[0]), "100");
        assert_eq!(shown(&tree, kids(&tree, c)[0]), "100");
    }
    /// §9.4, переменные Numi-листа: присваивания листа — узлы дерева
    /// (последнее присваивание ДО строки — семантика движка FR-013).
    #[test]
    fn sheet_variables_diamond() {
        let text = "x = 10\na = x × 2\nb = x + 1\ntotal = a + b";
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "t", text, 0.0);
        let tree = tree(&canvas, LineageNodeId::total("t"));
        assert_eq!(shown(&tree, 0), "31");
        // Итог → последняя формульная строка (total = a + b).
        let root_kids = kids(&tree, 0);
        assert_eq!(root_kids.len(), 1);
        let total_line = root_kids[0];
        assert_eq!(tree.nodes[total_line].line, Some(3));
        assert_eq!(
            tree.nodes[total_line].formula.as_deref(),
            Some("total = a + b")
        );
        // a и b — переменные листа (via нет — связь внутри листа).
        let ab = kids(&tree, total_line);
        assert_eq!(ab.len(), 2);
        assert_eq!(tree.nodes[ab[0]].line, Some(1));
        assert_eq!(shown(&tree, ab[0]), "20");
        assert_eq!(tree.nodes[ab[1]].line, Some(2));
        assert_eq!(shown(&tree, ab[1]), "11");
        assert!(tree.nodes[total_line].children[0].via.is_none());
        // x — под каждым (ромб внутри листа).
        let xa = kids(&tree, ab[0])[0];
        let xb = kids(&tree, ab[1])[0];
        assert_eq!(tree.nodes[xa].line, Some(0));
        assert_eq!(tree.nodes[xb].line, Some(0));
        assert_eq!(shown(&tree, xa), "10");
        assert_ne!(xa, xb);
    }

    /// §9.4, спиллы параметров: `toParam`-ребро питает `$rps` строки
    /// (проливание сильнее дефолта — FR-029).
    #[test]
    fn spill_param_into_text_node() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "s", "1000", 0.0);
        sheet(&mut canvas, "t", "= $rps × 2", 1.0);
        let mut edge = Edge::new("e1", "s", None, "t", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("rps".to_owned());
        canvas.add_edge(edge);
        let tree = tree(&canvas, LineageNodeId::total("t"));
        // Итог = единственная формульная строка.
        assert_eq!(shown(&tree, 0), "2\u{a0}000");
        let line = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[line].line, Some(0));
        let source = kids(&tree, line)[0];
        assert_eq!(tree.nodes[source].node_id, "s");
        assert_eq!(shown(&tree, source), "1\u{a0}000");
        let via = tree.nodes[line].children[0].via.as_ref().expect("ребро");
        assert_eq!(via.to_param.as_deref(), Some("rps"));
        assert_eq!(via.edge_id, "e1");
    }

    /// §9.4, unmapped-вход: источник без значения — терминальный узел
    /// «значение не подставлено» (AC-2.5, стиль fr-045 R-3).
    #[test]
    fn unmapped_input_terminal() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "a", "привет мир", 0.0);
        formula(&mut canvas, "b", "$in × 2", 1.0);
        value_edge(&mut canvas, "e1", "a", "b");
        let tree = tree(&canvas, LineageNodeId::total("b"));
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].kind, LineageNodeKind::Calc);
        let kid = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[kid].kind, LineageNodeKind::Unmapped);
        assert_eq!(tree.nodes[kid].label.as_deref(), Some("вход $1"));
        assert_eq!(tree.nodes[kid].value, None);
        let via = tree.nodes[0].children[0].via.as_ref().expect("ребро");
        assert_eq!(via.edge_id, "e1");
    }

    /// §9.4, цикл: пересчёт упал с CycleError — дерево строится в режиме
    /// Cycled, путь к раскрытому узлу — терминальный узел «цикл»
    /// (AC-2.4), построение не зависает.
    #[test]
    fn cycle_terminal_in_cycled_mode() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "a", "$in × 2", 0.0);
        formula(&mut canvas, "b", "$in + 1", 1.0);
        value_edge(&mut canvas, "e1", "a", "b");
        value_edge(&mut canvas, "e2", "b", "a");
        let err = crate::flow::topo_sort(&canvas).expect_err("цикл");
        let tree = build_lineage(
            &canvas,
            LineageFlow::Cycled(&err),
            LineageNodeId::total("a"),
        )
        .expect("дерево строится и на цикле");
        // a → b → (a уже на пути) терминал «цикл».
        assert_eq!(tree.nodes.len(), 3);
        assert_eq!(tree.nodes[0].kind, LineageNodeKind::Calc);
        assert_eq!(tree.nodes[0].value, None);
        let b = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[b].node_id, "b");
        let cycle = kids(&tree, b)[0];
        assert_eq!(tree.nodes[cycle].kind, LineageNodeKind::Cycle);
        assert_eq!(tree.nodes[cycle].node_id, "a");
        let via = tree.nodes[b].children[0].via.as_ref().expect("ребро");
        assert_eq!(via.edge_id, "e1");
    }

    /// §9.4, лист-константа: цифра без входов — цепочка без входов;
    /// у явной формулы-константы — единственный узел (AC-1.4,
    /// «исходное значение»), у Numi-листа — переход к строке.
    #[test]
    fn leaf_constant_root() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "a", "1240", 0.0);
        sheet(&mut canvas, "b", "1240", 1.0);
        // Явная формула-константа: единственный узел.
        let tree_a = tree(&canvas, LineageNodeId::total("a"));
        assert_eq!(tree_a.nodes.len(), 1);
        assert_eq!(tree_a.nodes[0].kind, LineageNodeKind::Leaf);
        assert_eq!(shown(&tree_a, 0), "1\u{a0}240");
        // Numi-лист: итог → последняя формульная строка → лист.
        let tree_b = tree(&canvas, LineageNodeId::total("b"));
        assert_eq!(tree_b.nodes.len(), 2);
        assert_eq!(shown(&tree_b, 0), "1\u{a0}240");
        let line = kids(&tree_b, 0)[0];
        assert_eq!(tree_b.nodes[line].line, Some(0));
        assert_eq!(tree_b.nodes[line].kind, LineageNodeKind::Leaf);
        assert!(kids(&tree_b, line).is_empty());
    }

    /// §9.4, корень-строка vs корень-итог: разные корни — разные цепочки.
    #[test]
    fn line_root_vs_total_root() {
        let text = "x = 10\ny = x × 2\n= y + 1";
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "t", text, 0.0);
        // Корень-строка (y = x × 2): один ребёнок — x.
        let line_tree = tree(&canvas, LineageNodeId::line("t", 1));
        assert_eq!(shown(&line_tree, 0), "20");
        assert_eq!(line_tree.nodes.len(), 2);
        assert_eq!(line_tree.nodes[1].line, Some(0));
        assert_eq!(shown(&line_tree, 1), "10");
        // Корень-итог: последняя формульная строка → y → x.
        let total_tree = tree(&canvas, LineageNodeId::total("t"));
        assert_eq!(shown(&total_tree, 0), "21");
        assert_eq!(total_tree.nodes.len(), 4);
        let last = kids(&total_tree, 0)[0];
        assert_eq!(total_tree.nodes[last].line, Some(2));
        let y = kids(&total_tree, last)[0];
        assert_eq!(total_tree.nodes[y].line, Some(1));
        let x = kids(&total_tree, y)[0];
        assert_eq!(total_tree.nodes[x].line, Some(0));
    }

    /// §9.4, крайние случаи: проза как корень, нода без результата,
    /// несуществующая нода, строка вне диапазона (§6.6).
    #[test]
    fn root_errors() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "t", "привет мир", 0.0);
        let solutions = solutions(&canvas);
        let ready = |root| {
            build_lineage(
                &canvas,
                LineageFlow::Ready {
                    solutions: &solutions,
                    data: &DataSnapshots::new(),
                },
                root,
            )
        };
        // Строка-проза — ошибка выбора (§6.6/§9.4).
        assert_eq!(
            ready(LineageNodeId::line("t", 0)),
            Err(LineageError::RootNotANumber("t".to_owned()))
        );
        // Нода без результата (только проза) — не вычисляемая.
        assert_eq!(
            ready(LineageNodeId::total("t")),
            Err(LineageError::RootNotANumber("t".to_owned()))
        );
        // Несуществующая нода и строка вне диапазона.
        assert_eq!(
            ready(LineageNodeId::total("nope")),
            Err(LineageError::RootNotFound("nope".to_owned()))
        );
        assert_eq!(
            ready(LineageNodeId::line("t", 99)),
            Err(LineageError::RootNotFound("t".to_owned()))
        );
    }

    /// §9.4, именованный выход: `fromOutput` адресует последнюю
    /// присваивающую строку имени («строка сдвинулась, связь жива»).
    #[test]
    fn named_output_targets_last_assignment() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "s", "npl = 0.92\nreserve = npl × 2", 0.0);
        formula(&mut canvas, "t", "$in × 100", 1.0);
        let mut edge = Edge::new("e1", "s", None, "t", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some("npl".to_owned());
        canvas.add_edge(edge);
        let tree = tree(&canvas, LineageNodeId::total("t"));
        assert_eq!(shown(&tree, 0), "92");
        let source = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[source].node_id, "s");
        assert_eq!(tree.nodes[source].line, Some(0));
        assert_eq!(shown(&tree, source), "0.92");
    }

    /// Приоритет адресации — `fromLine` сильнее `fromOutput` (зеркало
    /// `edge_source_value`, `flow.rs:583`).
    #[test]
    fn from_line_priority_over_output() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "s", "a = 5\nb = 7", 0.0);
        formula(&mut canvas, "t", "$in × 2", 1.0);
        let mut edge = Edge::new("e1", "s", None, "t", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_line = Some(1);
        edge.from_output = Some("a".to_owned());
        canvas.add_edge(edge);
        let tree = tree(&canvas, LineageNodeId::total("t"));
        assert_eq!(shown(&tree, 0), "14");
        let source = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[source].line, Some(1));
        assert_eq!(shown(&tree, source), "7");
    }

    /// §9.4, внешний источник данных (FR-045 R-2): колонка CSV — лист
    /// дерева со значением ячейки снапшота.
    #[test]
    fn data_node_column_leaf() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "d", "источник данных", 0.0);
        formula(&mut canvas, "t", "$in × 2", 1.0);
        let mut edge = Edge::new("e1", "d", None, "t", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some("price".to_owned());
        canvas.add_edge(edge);
        let mut data = DataSnapshots::new();
        data.insert("d".to_owned(), parse_csv("price\n80\n", ',').expect("csv"));
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        let tree = build_lineage(
            &canvas,
            LineageFlow::Ready {
                solutions: &solutions,
                data: &data,
            },
            LineageNodeId::total("t"),
        )
        .expect("дерево");
        assert_eq!(shown(&tree, 0), "160");
        let leaf = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[leaf].node_id, "d");
        assert_eq!(tree.nodes[leaf].kind, LineageNodeKind::Leaf);
        assert_eq!(shown(&tree, leaf), "80");
    }

    /// §9.4, код-фенсы: строки внутри ``` не считаются и в дерево не
    /// попадают (зеркало `eval_lines_with_env`).
    #[test]
    fn fence_lines_skipped() {
        let text = "x = 10\n```\nx = 999\n```\ny = x + 1";
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "t", text, 0.0);
        let tree = tree(&canvas, LineageNodeId::line("t", 4));
        assert_eq!(shown(&tree, 0), "11");
        let x = kids(&tree, 0)[0];
        assert_eq!(tree.nodes[x].line, Some(0));
        assert_eq!(shown(&tree, x), "10");
    }

    /// §9.4, 1000+ узлов: цепочка 1005 нод строится целиком, бюджет
    /// [`LINEAGE_MAX_NODES`] не исчерпан (G5).
    #[test]
    fn thousand_node_chain() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "n0", "1", 0.0);
        for i in 1..1005 {
            formula(&mut canvas, &format!("n{i}"), "$in + 1", i as f32);
            value_edge(
                &mut canvas,
                &format!("e{i}"),
                &format!("n{}", i - 1),
                &format!("n{i}"),
            );
        }
        let tree = tree(&canvas, LineageNodeId::total("n1004"));
        assert_eq!(shown(&tree, 0), "1\u{a0}005");
        assert!(tree.nodes.len() >= 1001);
        assert!(tree.nodes.len() <= LINEAGE_MAX_NODES);
        // Линейная цепочка: каждый узел — ровно один ребёнок, глубина
        // равна числу узлов дерева минус один (итог + строка на ноду).
        let mut depth = 0;
        let mut cursor = 0;
        while let Some(next) = tree.nodes[cursor].children.first().map(|c| c.child) {
            cursor = next;
            depth += 1;
        }
        assert_eq!(depth, tree.nodes.len() - 1);
    }

    /// Бюджет [`LINEAGE_MAX_NODES`]: upstream-ромб (каждый узел потребляет
    /// двух родителей) глубины 13 — дерево > 4096 узлов усекается,
    /// построение завершается (G5, §12 «дерево взрывается»).
    #[test]
    fn budget_truncates_exponential_diamond() {
        let mut canvas = Canvas::default();
        formula(&mut canvas, "s", "1", 0.0);
        let mut prev = vec!["s".to_owned()];
        let mut edge_no = 0;
        for level in 1..=13 {
            let mut current = Vec::new();
            let m = prev.len();
            for i in 0..(1usize << level) {
                let id = format!("n{level}_{i}");
                formula(&mut canvas, &id, "$1 + $2", level as f32);
                // РОВНО два входящих ребра у каждого узла (родители по
                // модулю размера предыдущего уровня) — вверх каждый узел
                // дерева происхождения расходится вдвое (2^13 листьев).
                for half in 0..2 {
                    let parent = &prev[(2 * i + half) % m];
                    value_edge(&mut canvas, &format!("e{edge_no}"), parent, &id);
                    edge_no += 1;
                }
                current.push(id);
            }
            prev = current;
        }
        let root = prev[0].clone();
        let solutions = solutions(&canvas);
        let tree = build_lineage(
            &canvas,
            LineageFlow::Ready {
                solutions: &solutions,
                data: &DataSnapshots::new(),
            },
            LineageNodeId::total(&root),
        )
        .expect("дерево усечено, а не упало");
        assert_eq!(tree.nodes.len(), LINEAGE_MAX_NODES);
        assert!(tree
            .nodes
            .iter()
            .any(|n| n.kind == LineageNodeKind::Truncated));
    }
    /// FR-050 Р-6: именованный путь в формуле — дерево происхождения
    /// раскрывается через ребро с этим qualified-ключом (итог приёмника →
    /// ребро e1 → строка присваивания истока). Неразрешённый путь (value-
    /// подмена what-if при удаляемом ребре) — терминал «не связано».
    #[test]
    fn qualified_ref_follows_edge_to_source_line() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "s", "Заявки\nКол = 40", 0.0);
        sheet(&mut canvas, "r", "итог = Заявки.Кол · 2", 1.0);
        let mut edge = Edge::new("e1", "s", None, "r", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some("Кол".to_owned());
        canvas.add_edge(edge);
        let t = tree(&canvas, LineageNodeId::total("r"));
        assert_eq!(t.nodes[0].kind, LineageNodeKind::Calc);
        assert_eq!(shown(&t, 0), "80");
        // Итог текстовой ноды → последняя формульная строка (переход
        // внутри листа, via None — §7.2), уже в ней — именованная ссылка.
        let root_kids = kids(&t, 0);
        assert_eq!(root_kids.len(), 1, "итог → строка формулы");
        let formula_idx = root_kids[0];
        assert_eq!(t.nodes[formula_idx].line, Some(0));
        let ref_kids = kids(&t, formula_idx);
        assert_eq!(ref_kids.len(), 1, "единственный операнд строки — путь");
        assert_eq!(
            t.nodes[formula_idx].children[0]
                .via
                .as_ref()
                .map(|v| v.edge_id.clone()),
            Some("e1".to_owned()),
            "переход через ребро именованной ссылки"
        );
        let src = ref_kids[0];
        assert_eq!(t.nodes[src].node_id, "s");
        assert_eq!(t.nodes[src].line, Some(1), "строка присваивания «Кол = 40»");
        assert_eq!(shown(&t, src), "40");

        // Неразрешённый путь: value-подмена what-if даёт ноде итог, а
        // persisted-формула ссылается на путь без ребра — терминал
        // «не связано» (движок в этой строке дал бы UnknownInput).
        let mut canvas2 = Canvas::default();
        sheet(&mut canvas2, "s", "Заявки\nКол = 40", 0.0);
        sheet(&mut canvas2, "r", "итог = Заявки.Нет", 1.0);
        let mut node_values = std::collections::HashMap::new();
        node_values.insert("r".to_owned(), Value::scalar(5.0));
        let solutions2 = propagate_with_lines_data(
            &canvas2,
            &WhatIfOverrides {
                line_exprs: std::collections::HashMap::new(),
                node_values,
            },
            &DataSnapshots::new(),
        )
        .expect("пересчёт");
        let t2 = build_lineage(
            &canvas2,
            LineageFlow::Ready {
                solutions: &solutions2,
                data: &DataSnapshots::new(),
            },
            LineageNodeId::total("r"),
        )
        .expect("дерево построено (итог — подмена)");
        assert!(
            t2.nodes
                .iter()
                .any(|n| n.kind == LineageNodeKind::Unlinked
                    && n.label.as_deref() == Some("Заявки.Нет")),
            "неразрешённый путь — терминал «не связано»: {:?}",
            t2.nodes.iter().map(|n| n.label.clone()).collect::<Vec<_>>()
        );
    }

    /// X3 (AC-4.2/F-6): дельты what-if в дереве — подмена листа «a = 5»
    /// → «a = 7» даёт дельты на всех затронутых узлах (лист a, узел b,
    /// строка c и итог корня); совпадающие значения дельт не дают.
    #[test]
    fn lineage_deltas_track_overrides() {
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "a", "a = 5", 0.0);
        sheet(&mut canvas, "b", "b = $in × 2", 1.0);
        sheet(&mut canvas, "c", "c = $in + 1", 2.0);
        value_edge(&mut canvas, "e1", "a", "b");
        value_edge(&mut canvas, "e2", "b", "c");
        let base = tree(&canvas, LineageNodeId::total("c"));
        // What-if: подмена строки 0 ноды a («a = 5» → «a = 7»).
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("a".to_owned(), 0usize), "a = 7".to_owned());
        let overrides = WhatIfOverrides {
            line_exprs,
            ..WhatIfOverrides::default()
        };
        let whatif_solutions =
            propagate_with_lines_data(&canvas, &overrides, &DataSnapshots::new())
                .expect("пересчёт с подменой");
        let whatif = build_lineage(
            &canvas,
            LineageFlow::Ready {
                solutions: &whatif_solutions,
                data: &DataSnapshots::new(),
            },
            LineageNodeId::total("c"),
        )
        .expect("what-if дерево построено");
        let deltas = lineage_deltas(&base, &whatif);
        // 6 узлов: итоги нод (a/b/c, None) + строки Numi-листов (b/c, 0) +
        // лист (a, 0) — каждый узел с изменившимся значением.
        assert_eq!(deltas.len(), 6, "все затронутые узлы: {deltas:?}");
        let root = deltas
            .get(&("c".to_owned(), None))
            .expect("дельта итога корня");
        assert_eq!(root.base.to_string(), "11");
        assert_eq!(root.whatif.to_string(), "15");
        assert_eq!(root.delta, "+4");
        assert!(deltas.contains_key(&("a".to_owned(), Some(0))), "лист a");
        assert!(deltas.contains_key(&("a".to_owned(), None)), "итог a");
        assert!(deltas.contains_key(&("b".to_owned(), Some(0))), "узел b");
        assert!(
            deltas.contains_key(&("c".to_owned(), Some(0))),
            "строка c: {deltas:?}"
        );
        // Совпадающие значения (подмен нет) — дельт нет.
        let same = tree(&canvas, LineageNodeId::total("c"));
        assert!(lineage_deltas(&base, &same).is_empty());
    }

    /// X3: узел what-if дерева без пары в базе (структурный сдвиг) или
    /// с ошибочным значением не даёт дельту — сопоставление по адресу
    /// устойчиво (F-5).
    #[test]
    fn lineage_deltas_skip_unmatched_and_errors() {
        let mut base = LineageTree {
            root: LineageNodeId::total("x"),
            nodes: Vec::new(),
        };
        base.nodes.push(LineageNode {
            node_id: "x".into(),
            line: None,
            kind: LineageNodeKind::Calc,
            value: Some(Ok(Value::scalar(10.0))),
            formula: None,
            title: "X".into(),
            label: None,
            children: Vec::new(),
        });
        // What-if: значение стало ошибкой — дельты нет.
        let mut errored = base.clone();
        errored.nodes[0].value = Some(Err("цикл".into()));
        assert!(lineage_deltas(&base, &errored).is_empty());
        // What-if: пара отсутствует (узел без значения) — дельты нет.
        let mut absent = base.clone();
        absent.nodes[0].value = None;
        assert!(lineage_deltas(&base, &absent).is_empty());
        // Новый узел what-if без базовой пары — дельты нет.
        let mut extra = base.clone();
        extra.nodes.push(LineageNode {
            node_id: "y".into(),
            line: None,
            kind: LineageNodeKind::Leaf,
            value: Some(Ok(Value::scalar(1.0))),
            formula: None,
            title: "Y".into(),
            label: None,
            children: Vec::new(),
        });
        assert!(lineage_deltas(&base, &extra).is_empty());
    }

    /// F-9 (X6): линейная развёртка — корень без отступа, дети с отступом
    /// и сквозной нумерацией, значения/формулы/каналы в строках.
    #[test]
    fn explain_text_linear_deployment() {
        let mut tree = LineageTree {
            root: LineageNodeId::total("cost"),
            nodes: Vec::new(),
        };
        // cost = arpu + rent; arpu (calc) ← ad (leaf); rent (leaf).
        tree.nodes.push(LineageNode {
            node_id: "cost".into(),
            line: None,
            kind: LineageNodeKind::Calc,
            value: Some(Ok(Value::scalar(1240.0))),
            formula: Some("total = arpu + rent".into()),
            title: "Итог".into(),
            label: None,
            children: vec![
                LineageChild {
                    child: 1,
                    via: Some(LineageVia {
                        edge_id: "e1".into(),
                        from_node: "arpu".into(),
                        to_node: "cost".into(),
                        from_line: None,
                        from_output: Some("arpu".into()),
                        to_param: None,
                    }),
                },
                LineageChild {
                    child: 2,
                    via: None, // локальная переменная листа
                },
            ],
        });
        tree.nodes.push(LineageNode {
            node_id: "ad".into(),
            line: None,
            kind: LineageNodeKind::Calc,
            value: Some(Ok(Value::scalar(810.0))),
            formula: Some("arpu = ads * price".into()),
            title: "Выручка".into(),
            label: None,
            children: vec![LineageChild {
                child: 3,
                via: Some(LineageVia {
                    edge_id: "e2".into(),
                    from_node: "ad".into(),
                    to_node: "arpu".into(),
                    from_line: Some(1),
                    from_output: None,
                    to_param: None,
                }),
            }],
        });
        tree.nodes.push(LineageNode {
            node_id: "cost".into(),
            line: Some(3),
            kind: LineageNodeKind::Leaf,
            value: Some(Ok(Value::scalar(620.0))),
            formula: None,
            title: "Аренда".into(),
            label: None,
            children: Vec::new(),
        });
        tree.nodes.push(LineageNode {
            node_id: "ad".into(),
            line: Some(1),
            kind: LineageNodeKind::Leaf,
            value: Some(Ok(Value::scalar(100.0))),
            formula: None,
            title: "Трафик".into(),
            label: None,
            children: Vec::new(),
        });
        let text = explain_text(&tree);
        let lines: Vec<&str> = text.lines().collect();
        // Корень — заголовок без номера; статистика в последней строке.
        assert!(
            lines[0].starts_with("Цепочка расчёта: Итог [cost] = 1\u{a0}240"),
            "{}",
            lines[0]
        );
        assert!(
            lines[0].contains("формула: total = arpu + rent"),
            "{}",
            lines[0]
        );
        // Дети: отступ + сквозная нумерация + канал (выход / локальная переменная).
        assert!(
            lines[1].starts_with("  1. Выручка [ad] = 810"),
            "{}",
            lines[1]
        );
        assert!(
            lines[1].contains("через выход arpu (ребро e1)"),
            "{}",
            lines[1]
        );
        assert!(
            lines[2].starts_with("  2. Аренда [cost:3] = 620"),
            "{}",
            lines[2]
        );
        assert!(lines[2].contains("исходное значение"), "{}", lines[2]);
        assert!(
            lines[2].contains("локальная переменная листа"),
            "{}",
            lines[2]
        );
        // Внук: двойной отступ, канал «через строку N» (from_line + 1).
        assert!(
            lines[3].starts_with("    3. Трафик [ad:1] = 100"),
            "{}",
            lines[3]
        );
        assert!(
            lines[3].contains("через строку 2 (ребро e2)"),
            "{}",
            lines[3]
        );
        assert_eq!(lines[4], "Всего узлов: 4 (листьев: 2)");
    }

    /// F-9 (X6): терминальные узлы — цикл/не связано/не подставлено/усечено
    /// и ошибка значения — текстом, без паники на пустом дереве.
    #[test]
    fn explain_text_terminals_and_errors() {
        // Пустое дерево — только корневой заголовок и статистика.
        let empty = LineageTree {
            root: LineageNodeId::total("x"),
            nodes: Vec::new(),
        };
        assert_eq!(explain_text(&empty), "Всего узлов: 0 (листьев: 0)");

        let mut tree = LineageTree {
            root: LineageNodeId::total("x"),
            nodes: Vec::new(),
        };
        tree.nodes.push(LineageNode {
            node_id: "x".into(),
            line: None,
            kind: LineageNodeKind::Calc,
            value: Some(Err("деление на ноль".into())),
            formula: None,
            title: "Битый".into(),
            label: None,
            children: (1..5)
                .map(|child| LineageChild { child, via: None })
                .collect(),
        });
        let terminals = [
            (LineageNodeKind::Cycle, "цикл (значение не определено)"),
            (LineageNodeKind::Unmapped, "значение не подставлено"),
            (LineageNodeKind::Unlinked, "не связано: npl"),
            (LineageNodeKind::Truncated, "усечено"),
        ];
        for (kind, _needle) in terminals {
            let label = if kind == LineageNodeKind::Unlinked {
                Some("npl".to_owned())
            } else {
                None
            };
            tree.nodes.push(LineageNode {
                node_id: format!("t{:?}", kind),
                line: None,
                kind,
                value: None,
                formula: None,
                title: "Т".into(),
                label,
                children: Vec::new(),
            });
        }
        let text = explain_text(&tree);
        assert!(text.contains("— ошибка: деление на ноль"), "{text}");
        for (_, needle) in terminals {
            assert!(text.contains(needle), "{needle} absent: {text}");
        }
        // Статистика: листья из терминалов не считаются.
        assert!(text.ends_with("Всего узлов: 5 (листьев: 0)"), "{text}");
    }

    /// F-9 (X6): глубина на цепочке 1005 нод — итеративный проход, отступы
    /// растут линейно, порядок предзаказа не ломается (G5-паритет).
    #[test]
    fn explain_text_deep_chain_no_recursion() {
        const N: usize = 1005;
        let mut tree = LineageTree {
            root: LineageNodeId::total("n0"),
            nodes: Vec::new(),
        };
        for index in 0..N {
            tree.nodes.push(LineageNode {
                node_id: format!("n{index}"),
                line: None,
                kind: if index + 1 < N {
                    LineageNodeKind::Calc
                } else {
                    LineageNodeKind::Leaf
                },
                value: Some(Ok(Value::scalar(index as f64))),
                formula: None,
                title: format!("Н{index}"),
                label: None,
                children: if index + 1 < N {
                    vec![LineageChild {
                        child: index + 1,
                        via: None,
                    }]
                } else {
                    Vec::new()
                },
            });
        }
        let text = explain_text(&tree);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), N + 1, "все узлы + статистика");
        // Последний узел — глубина N-1: отступ 2·(N-1) пробелов.
        let last = lines[N - 1];
        let indent = last.len() - last.trim_start().len();
        assert_eq!(indent, 2 * (N - 1));
        assert!(text.ends_with(&format!("Всего узлов: {N} (листьев: 1)")));
    }

    /// F-12 (X6): покрытие цепочками — связанная модель: 100% (итоги +
    /// построчные результаты); висячее проливание в параметр шаблона от
    /// ноды без значения даёт unmapped-терминал — цифра приёмника не
    /// покрыта; при цикле — 0% (значений нет).
    #[test]
    fn chain_coverage_counts_explained_digits() {
        // a = 5 (константа) → b = $in × 2 (итог + построчный результат).
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "a", "A\n= 5", 0.0);
        sheet(&mut canvas, "b", "B\n= $in × 2", 300.0);
        canvas.edges.push(Edge::new(
            "e1",
            "a",
            Some(Side::Right),
            "b",
            Some(Side::Left),
        ));
        canvas.edges[0].set_flow_kind(crate::flow::FlowKind::Value);
        let solutions = crate::flow::propagate_with_lines(&canvas, &WhatIfOverrides::default())
            .expect("без циклов");
        let data = crate::DataSnapshots::new();
        let stat = chain_coverage(
            &canvas,
            LineageFlow::Ready {
                solutions: &solutions,
                data: &data,
            },
        );
        // Цифры: итог a (Ok 5), строка a:1 (Ok 5), итог b (Ok 10), строка
        // b:1 (Ok 10) — все объяснимы (лист a / calc b через ребро).
        assert_eq!(stat.total, 4, "2 итога + 2 построчных");
        assert_eq!(stat.covered, 4);
        assert_eq!(stat.percent(), Some(100));

        // Висячее проливание: a — проза (значения нет), шаблонная t с
        // параметром-дефолтом считает итог (R-1: проливание без значения →
        // дефолт), но в дереве t unmapped-терминал «параметр rate» —
        // цепочка до листьев не доходит (покрытие 0% при вычислимой цифре).
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "a", "просто текст", 0.0);
        let mut node = Node::text("t", "Сетка", 300.0, 0.0);
        let mut params = BTreeMap::new();
        params.insert(
            "rate".to_owned(),
            crate::templates::TemplateParam {
                num: 2.0,
                unit: None,
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
        let mut edge = Edge::new("e1", "a", None, "t", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("rate".to_owned());
        canvas.add_edge(edge);
        let solutions = crate::flow::propagate_with_lines(&canvas, &WhatIfOverrides::default())
            .expect("без циклов");
        let stat = chain_coverage(
            &canvas,
            LineageFlow::Ready {
                solutions: &solutions,
                data: &DataSnapshots::new(),
            },
        );
        assert_eq!(stat.total, 1, "итог шаблона t вычислен через дефолт");
        assert_eq!(stat.covered, 0, "unmapped-терминал «параметр rate»");
        assert_eq!(stat.percent(), Some(0));

        // Цикл: значений нет — цифр нет, процент не определён.
        let mut canvas = Canvas::default();
        sheet(&mut canvas, "a", "A\n= $in", 0.0);
        sheet(&mut canvas, "b", "B\n= $in", 300.0);
        canvas.edges.push(Edge::new(
            "e1",
            "a",
            Some(Side::Right),
            "b",
            Some(Side::Left),
        ));
        canvas.edges[0].set_flow_kind(crate::flow::FlowKind::Value);
        canvas.edges.push(Edge::new(
            "e2",
            "b",
            Some(Side::Left),
            "a",
            Some(Side::Right),
        ));
        canvas.edges[1].set_flow_kind(crate::flow::FlowKind::Value);
        let cycle = crate::flow::propagate_with_lines(&canvas, &WhatIfOverrides::default())
            .expect_err("цикл");
        // Цикл: значений нет нигде — все вычисляемые цифры не покрыты
        // (0% — модель сломана, индикатор честно это показывает).
        let stat = chain_coverage(&canvas, LineageFlow::Cycled(&cycle));
        assert_eq!(stat.total, 2, "2 вычисляемые строки");
        assert_eq!(stat.covered, 0, "при цикле ни одна цепочка не объясняется");
        assert_eq!(stat.percent(), Some(0));
    }
}
