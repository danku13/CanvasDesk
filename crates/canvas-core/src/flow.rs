//! FR-014: поток значений по рёбрам — DAG-движок и live-ревал.
//!
//! Чистый Rust, без I/O и глобального состояния (инвариант тестируемости
//! FR-014): [`topo_sort`] — `(&Canvas) -> Result<Vec<usize>, CycleError>`,
//! [`propagate`] — `(&Canvas, &HashMap<String, Value>) ->
//! Result<FlowOutputs, CycleError>`, 0 side-эффектов.
//!
//! Семантика:
//! - поток значений идёт только по рёбрам с `flow.kind = "value"`
//!   ([`FlowKind::Value`]); контрольные рёбра — визуальные связи, в граф
//!   не входят;
//! - граф value-рёбер обязан быть DAG — цикл это [`CycleError`] со списком
//!   участников (SCC-поиск поверх алгоритма Кана);
//! - в пересчёте участвуют формульные ноды: явная формула `canvasdesk.expr`
//!   (FR-013, MCP) ИЛИ текст заметки в Numi-стиле — значение text-ноды =
//!   последняя формульная строка (семантика итога FR-013); проза значения
//!   не даёт — вход для downstream «отсутствует»;
//! - значение ноды считается в окружении со входами (`$in`, `$1..$N`);
//! - входящие value-рёбра ноды нумеруются в порядке `canvas.edges`:
//!   `$1` — первое, `$N` — N-е; `$in` требует ровно одно входящее ребро;
//! - `overrides` (what-if, FR-017): подменённое значение ноды используется
//!   вместо её формулы — сама нода и весь downstream видят override;
//! - результат (`FlowOutputs`) — runtime-данные: НЕ сериализуется в
//!   `.canvas`, источник истины — формулы + топология рёбер.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::expr::{self, Env, EvalError, ExprOutcome, Value};
use crate::model::{Canvas, Edge};
use crate::templates::OutputSource;

/// Тип потока ребра (FR-014): контрольная связь (по умолчанию — обратная
/// совместимость со старыми `.canvas`) или поток значений.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// Визуальная связь без потока значений (существующее поведение рёбер).
    Control,
    /// Ребро переносит значение источника в `$in`/`$1..$N` приёмника.
    Value,
}

impl FlowKind {
    /// Разбор значения `canvasdesk.flow.kind`; неизвестное — Control.
    pub fn from_kind_str(kind: &str) -> Self {
        match kind {
            "value" => Self::Value,
            _ => Self::Control,
        }
    }

    /// Строковое значение для `canvasdesk.flow.kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Value => "value",
        }
    }
}

/// Ошибка топологии: цикл из value-рёбер. `nodes` — участники цикла
/// (id нод, отсортированы) — пользователю показывается для отладки.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("цикл потока значений: {}", nodes.join(" → "))]
pub struct CycleError {
    pub nodes: Vec<String>,
}

/// Результат пересчёта графа: `node_id` — значение формулы или ошибка
/// вычисления (часть downstream может ошибаться без падения всего графа).
/// Ноды без формулы записи не получают.
pub type FlowOutputs = HashMap<String, Result<Value, EvalError>>;

/// Топологический порядок нод по value-рёбрам (алгоритм Кана). В граф
/// входят только рёбра [`FlowKind::Value`] с существующими концами; порядок
/// детерминирован (очередь по возрастанию индексов). Цикл —
/// `Err(CycleError)` с участниками (SCC размера > 1 и петли).
pub fn topo_sort(canvas: &Canvas) -> Result<Vec<usize>, CycleError> {
    let ValueGraph {
        n,
        adj,
        mut indegree,
        self_loops,
    } = build_value_graph(canvas);
    // Кан: очередь заполняется по возрастанию индексов — детерминизм
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &j in &adj[i] {
            indegree[j] -= 1;
            if indegree[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    if order.len() == n {
        return Ok(order);
    }
    // Остаток — циклы и их окрестности: участники — SCC размера > 1 и петли
    Err(CycleError {
        nodes: cycle_participants(canvas, n, &adj, &self_loops),
    })
}

/// FR-065 P1 (M4/S3 волны S): топосортировка ярусами — Kahn с drain-фронтиром
/// в `sub-vec` на каждой итерации `while !queue.is_empty()`. Любые два узла
/// одного яруса не связаны value-рёбрами по построению (фронтир Кана) —
/// могут вычисляться параллельно без блокировок (контракт
/// `docs/plans/adr-0008-wave-s-plan.md` §5.2). Порядок внутри яруса —
/// детерминирован (тот же Kahn с очередью по возрастанию индексов, что и
/// [`topo_sort`]); flatten-эквивалентность:
/// `topo_levels(canvas)?.into_iter().flatten().collect::<Vec<_>>()` ==
/// `topo_sort(canvas)?` (инвариант-тест — `tests/parallel_determinism.rs`).
/// Цикл — `Err(CycleError)` (переиспользует [`cycle_participants`]).
pub fn topo_levels(canvas: &Canvas) -> Result<Vec<Vec<usize>>, CycleError> {
    let ValueGraph {
        n,
        adj,
        mut indegree,
        self_loops,
    } = build_value_graph(canvas);
    // Тот же Kahn, что и topo_sort — но drain-фронтир в sub-vec на каждой
    // итерации внешнего цикла. queue инициализируется по возрастанию
    // индексов (детерминизм); внутри яруса порядок = порядок очереди Кана
    // (FIFO преемников), что побитово совпадает с topo_sort при flatten.
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut levels: Vec<Vec<usize>> = Vec::new();
    let mut total = 0usize;
    while !queue.is_empty() {
        // drain-фронтир: все готовые к этому ярусу ноды (Кahn-фронтир).
        // порядок — FIFO queue (детерминизм: queue инициализирован по
        // возрастанию индексов, преемники push_back в порядке обхода).
        let level: Vec<usize> = queue.drain(..).collect();
        total += level.len();
        for &i in &level {
            for &j in &adj[i] {
                indegree[j] -= 1;
                if indegree[j] == 0 {
                    queue.push_back(j);
                }
            }
        }
        levels.push(level);
    }
    if total == n {
        return Ok(levels);
    }
    Err(CycleError {
        nodes: cycle_participants(canvas, n, &adj, &self_loops),
    })
}

/// Граф value-рёбер: смежность + полустепени захода + петли. Висячие рёбра
/// (конец не в `canvas.nodes`) пропускаются — они не создают циклов.
/// Используется и [`topo_sort`], и [`topo_levels`] (общая настройка графа,
/// инвариант flatten-эквивалентности: обе функции видят одинаковую топологию).
struct ValueGraph {
    n: usize,
    adj: Vec<Vec<usize>>,
    indegree: Vec<usize>,
    self_loops: Vec<usize>,
}

fn build_value_graph(canvas: &Canvas) -> ValueGraph {
    let n = canvas.nodes.len();
    let mut index_of: HashMap<&str, usize> = HashMap::with_capacity(n);
    for (i, node) in canvas.nodes.iter().enumerate() {
        index_of.insert(node.id.as_str(), i);
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree = vec![0usize; n];
    let mut self_loops: Vec<usize> = Vec::new();
    for edge in &canvas.edges {
        if edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let (Some(&from), Some(&to)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if from == to {
            // Петля — цикл из одного участника; нода блокируется в Кане
            // (полустепень захода никогда не станет 0)
            self_loops.push(from);
            indegree[from] += 1;
            continue;
        }
        adj[from].push(to);
        indegree[to] += 1;
    }
    ValueGraph {
        n,
        adj,
        indegree,
        self_loops,
    }
}

/// Участники циклов value-графа: ноды SCC размера > 1 (Тарьян) + петли.
/// id сортируются — детерминизм и читаемый список.
fn cycle_participants(
    canvas: &Canvas,
    n: usize,
    adj: &[Vec<usize>],
    self_loops: &[usize],
) -> Vec<String> {
    // Тарьян (итеративный — глубина графа не ограничена стеком вызовов)
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0usize;
    let mut participants: Vec<String> = Vec::new();

    // Кадр: (нода, позиция обхода смежности)
    let mut frames: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if indices[root].is_some() {
            continue;
        }
        frames.push((root, 0));
        while let Some(&(v, it)) = frames.last() {
            if it == 0 {
                indices[v] = Some(counter);
                low[v] = counter;
                counter += 1;
                stack.push(v);
                on_stack[v] = true;
            }
            if it < adj[v].len() {
                let w = adj[v][it];
                if let Some(frame) = frames.last_mut() {
                    frame.1 += 1;
                }
                match indices[w] {
                    None => frames.push((w, 0)),
                    Some(iw) if on_stack[w] => low[v] = low[v].min(iw),
                    Some(_) => {}
                }
            } else {
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == indices[v].unwrap_or(0) {
                    // Корень SCC: компонента — верх стека до v включительно
                    let mut size = 0usize;
                    let mut component: Vec<usize> = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component.push(w);
                        size += 1;
                        if w == v {
                            break;
                        }
                    }
                    if size > 1 {
                        component.sort_unstable();
                        participants.extend(
                            component
                                .iter()
                                .filter_map(|&i| canvas.nodes.get(i).map(|node| node.id.clone())),
                        );
                    }
                }
            }
        }
    }
    // Петли — участники из одного элемента
    for &i in self_loops {
        if let Some(node) = canvas.nodes.get(i) {
            participants.push(node.id.clone());
        }
    }
    participants.sort();
    participants.dedup();
    participants
}

/// Живой пересчёт всего графа (чистая функция, live-ревал FR-014).
///
/// Для каждой ноды в топологическом порядке: собрать значения входящих
/// value-рёбер (в порядке `canvas.edges`), вычислить `canvasdesk.expr` в
/// окружении со входами. Нода без формулы значения не даёт; ошибка формулы
/// не прерывает пересчёт — downstream этой ноды получает «вход
/// отсутствует». Возвращает карту результатов всех формульных нод.
///
/// `overrides` — подмена значений нод по id (FR-014): нода
/// получает override как своё значение, downstream видит его же.
/// Сохраняет сигнатуру FR-014; построчные подмены (FR-017) — через
/// [`propagate_with_lines`] с [`WhatIfOverrides`].
pub fn propagate(
    canvas: &Canvas,
    overrides: &HashMap<String, Value>,
) -> Result<FlowOutputs, CycleError> {
    let whatif = WhatIfOverrides::from_node_values(overrides);
    propagate_with_lines(canvas, &whatif).map(|solutions| solutions.outputs)
}

/// FR-017 (CP6): what-if подмены для [`propagate_with_lines`]. Пустая
/// структура — поведение идентично FR-014/FR-025 (обратная совместимость,
/// инвариант 1 FR-017).
#[derive(Debug, Clone, Default)]
pub struct WhatIfOverrides {
    /// Построчные подмены исходников: (id ноды, индекс строки ТЕКСТА)
    /// → новый исходник строки. Индексация едина с `FlowSolutions.lines`
    /// и `Edge::from_line` (инвариант 4 FR-017). Индексы вне диапазона
    /// текста игнорируются (тихая деградация протухших подмен).
    pub line_exprs: HashMap<(String, usize), String>,
    /// Value-level подмена значения ноды целиком (наследие FR-014; путь
    /// MCP): формула не выполняется, downstream видит подменённое значение.
    pub node_values: HashMap<String, Value>,
}

impl WhatIfOverrides {
    /// Value-only слой из старой карты FR-014 (миграция вызовов).
    pub fn from_node_values(values: &HashMap<String, Value>) -> Self {
        Self {
            line_exprs: HashMap::new(),
            node_values: values.clone(),
        }
    }

    /// Подмены строк одной ноды, отсортированные по индексу строки
    /// (порядок важен: последовательная оценка RHS в виртуальной
    /// param-карте шаблонной ноды).
    pub fn line_overrides(&self, node_id: &str) -> Vec<(usize, &String)> {
        let mut list: Vec<(usize, &String)> = self
            .line_exprs
            .iter()
            .filter(|((id, _), _)| id == node_id)
            .map(|((_, line), expr)| (*line, expr))
            .collect();
        list.sort_by_key(|(line, _)| *line);
        list
    }
}

/// FR-017: виртуальный исходник текста — строки с индексами из подмен
/// заменены новыми исходниками, остальные — как в persisted-тексте. Индексы
/// вне диапазона игнорируются (тихая деградация протухших подмен,
/// симметрия политики FR-025). Публична для приложения: построчные
/// результаты рендера считаются по тому же виртуальному листу, что и
/// propagator (инвариант 4 FR-017).
pub fn whatif_virtual_text(text: &str, overrides: &[(usize, &String)]) -> String {
    if overrides.is_empty() {
        return text.to_owned();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    for (index, expr) in overrides {
        if let Some(slot) = lines.get_mut(*index) {
            *slot = expr;
        }
    }
    lines.join("\n")
}

/// FR-017: разбор override-строки присваивания (`rps = 2000 rps`) в
/// (имя, значение) — RHS вычисляется в данном окружении. Не-присваивание
/// (проза, явная формула `= …`) — None (параметр не биндится).
fn override_assignment(expr: &str, env: &Env) -> Option<(String, Value)> {
    let (name, rhs) = expr.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    let parsed = expr::parse(rhs.trim()).ok()?;
    let value = expr::eval(&parsed, env).ok()?;
    Some((name.to_owned(), value))
}

/// FR-050 Н5: приведение пролитого значения к единицам параметра приёмника.
/// Безразмерное значение в параметр с единицей трактуется в единицах
/// приёмника (Numi-семантика: «500» в параметр rps = 500 rps — число
/// переносится, единица прикрепляется); значение с единицей приходит как
/// есть (в безразмерный параметр — тоже, отображается со своей единицей);
/// несовместимые размерности обеих сторон — диагноз `E-UNIT` в
/// [`crate::validate`] (здесь значение не искажаем). Неизвестный токен
/// единицы параметра — скаляр (тихая деградация, как в
/// [`expr::unit_value`]).
fn spill_value_in_param_units(
    tpl: &crate::templates::TemplateRef,
    param: &str,
    value: &Value,
) -> Value {
    if !value.unit.is_scalar() {
        return value.clone();
    }
    match tpl.params.get(param).and_then(|spec| spec.unit.as_deref()) {
        Some(unit) => {
            let expected = expr::unit_value(0.0, Some(unit));
            Value::with_unit(value.num, expected.unit)
        }
        None => value.clone(),
    }
}

/// FR-025: построчные выходы Numi-листов — значение каждой формульной
/// строки: `(id ноды, индекс строки) → Value`. Заполняется в
/// [`propagate_with_lines`] (тот же обход и то же окружение, что у
/// значения ноды — строки видят входы value-рёбер). Ошибки строк и проза
/// значений не дают (слот деградирует до `None`).
pub type LineOutputs = HashMap<(String, usize), Value>;

/// FR-029: именованные выходы — `(id ноды, имя выхода) → Value`.
/// Для шаблонных нод — секция `outputs` манифеста (снапшот
/// `canvasdesk.template.outputs`: строка листа или подвыражение от
/// параметров); для текстовых — переменные Numi-листа (последнее
/// определение). Источник адресуется value-рёбрами `fromOutput`.
pub type NamedOutputs = HashMap<(String, String), Value>;

/// FR-045 R-2 (§Q3): снапшоты источников данных data-нод (`canvasdesk.data`)
/// — `node_id → CsvSnapshot`. Содержимое источника НЕ хранится в `.canvas`
/// (модель несёт только `ref`/`fields`) — карту заполняет приложение при
/// создании/перезагрузке CSV-источника (снапшот при создании, §Q3); для
/// `dacdb`/`db` в PoC снапшота нет — рёбра остаются unmapped (§Q4).
pub type DataSnapshots = HashMap<String, crate::csv::CsvSnapshot>;

/// FR-029: полный результат пересчёта — значения нод ([`FlowOutputs`]),
/// построчные выходы ([`LineOutputs`]), именованные выходы
/// ([`NamedOutputs`]) и предупреждения по нодам (конфликты проливания).
#[derive(Debug, Clone, Default)]
pub struct FlowSolutions {
    /// Значения нод (как в [`propagate`]).
    pub outputs: FlowOutputs,
    /// Значения формульных строк текстовых нод (FR-025).
    pub lines: LineOutputs,
    /// Именованные выходы нод (FR-029).
    pub named: NamedOutputs,
    /// FR-029: предупреждения по нодам — несколько value-рёбер в один
    /// `toParam` (побеждает последнее по порядку `canvas.edges`).
    /// Строгая диагностика — `graph_validate` (FR-032).
    pub warnings: HashMap<String, Vec<String>>,
}

/// FR-029: входящие value-рёбра ноды, разобранные по адресации.
/// Позиционные слоты `$1..$N` — рёбра БЕЗ `toParam` (порядок `canvas.edges`;
/// для канвасов без адресованных рёбер — то же множество, что и раньше,
/// обратная совместимость FR-014); карта проливания — рёбра С `toParam`
/// (последнее по `canvas.edges` побеждает, детерминизм).
#[derive(Debug, Default)]
struct InboundValues {
    /// Слоты `$1..$N` (значение ребра или `None` — источник без значения).
    slots: Vec<Option<Value>>,
    /// Проливание в параметры: имя параметра → значение (последнее
    /// по `canvas.edges` ребро с этим `toParam`).
    spill: BTreeMap<String, Option<Value>>,
    /// Имена параметров с конфликтом (≥ 2 рёбер в один `toParam`).
    conflicts: Vec<String>,
    /// FR-050 Р-6 (таблица резолва имён): qualified-пути «Объект.Поле» →
    /// значение — по всем адресным формам истоков; ключи НЕ зависят от
    /// порядка `canvas.edges` (перенумерация не меняет резолва, инвариант 6).
    qualified: BTreeMap<(String, String), Value>,
}

/// [`propagate`] с построчными выходами (FR-025): для текстовых Numi-листов
/// дополнительно собирает значение каждой формульной строки. Нумерация
/// строк — индекс строки ТЕКСТА ноды (тот же, что в `ExprLineResults`
/// FR-013 и в бейджах результатов рендера). FR-025 (правка 2, по проверке
/// владельца): шаблонные ноды тоже дают построчные выходы — их текст (лист
/// параметров FR-018) вычисляется наравне с заметками, узловое значение
/// остаётся у формулы шаблона (футер, `line = None`).
pub fn propagate_with_lines(
    canvas: &Canvas,
    whatif: &WhatIfOverrides,
) -> Result<FlowSolutions, CycleError> {
    propagate_with_lines_data(canvas, whatif, &DataSnapshots::new())
}

/// FR-045 R-2 (§Q3): [`propagate_with_lines`] со снапшотами CSV-источников
/// — колонки data-нод проливаются в поток (см.
/// [`edge_source_value_with_data`]). Пустая карта — поведение идентично
/// [`propagate_with_lines`] (обратная совместимость всех вызовов).
///
/// FR-065 (M4/S3 волны S): за фичей `parallel` (desktop-only,
/// `cfg(not(target_arch="wasm32"))`) цикл пересчёта идёт по ярусам
/// [`topo_levels`] — каждый ярус вычисляется параллельно через
/// `rayon` `par_iter` (bounded thread pool, automatic parallelism-threshold
/// для мелких ярусов). Каждый вызов `eval_node` клонирует свой `Env`
/// (expr.rs `Env` — `Clone`); барьер между ярусами — по построению фронтира
/// Кана. На wasm/без `parallel` — однопоточный flatten-путь (контракт
/// `docs/plans/adr-0008-wave-s-plan.md` §5.1/§5.8: сигнатура СТАБИЛЬНА,
/// меняется только внутренность цикла). collect-then-reduce (контракт
/// §5.7.3): `par_iter().reduce()` ЗАПРЕЩЁН — только
/// `par_iter().collect() → sort by index → merge` в детерминированном порядке.
pub fn propagate_with_lines_data(
    canvas: &Canvas,
    whatif: &WhatIfOverrides,
    data: &DataSnapshots,
) -> Result<FlowSolutions, CycleError> {
    // FR-050 Р-6: адресные имена нод для qualified-резолва — один расчёт
    // на весь пересчёт (коллизии: «Заявки (2)»/«Заявки (id)», порядок
    // `canvas.nodes` — детерминизм, инвариант 6).
    let obj_names = QualifiedNames::build(canvas);
    let mut solutions = FlowSolutions::default();

    // FR-065 P2/P3: параллельный путь — desktop + feature `parallel`.
    // Ярусы topo_levels — карта независимости: любые два узла одного яруса
    // не связаны value-рёбрами (фронтир Кана), значит могут вычисляться
    // параллельно без блокировок. Барьер между ярусами — end of par_iter.
    //
    // Реализация: `rayon` `par_iter` (P3) вместо изначального плана P2
    // `std::thread::scope` — scope спавнит ОДИН OS-поток на узел, что на
    // тяжёлых графах (8192-нод exponential diamond, тест
    // `lineage::tests::budget_truncates_exponential_diamond`) превышает
    // лимит OS-потоков (EAGAIN). `rayon` использует bounded thread pool
    // (default = num_cpus), автоматический parallelism-threshold для
    // мелких ярусов (5–10 узлов — sequential) и naturally реализует
    // collect-then-reduce (контракт §5.7.3 — `par_iter().collect()` →
    // sort by index → merge; `par_iter().reduce()` ЗАПРЕЩЁН).
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        use rayon::prelude::*;
        let levels = topo_levels(canvas)?;
        for level in &levels {
            // rayon par_iter по узлам яруса: каждый вызов eval_node
            // клонирует свой Env (expr.rs Env — Clone). references на
            // canvas/whatif/data/obj_names/solutions — shared read-only
            // (Sync: plain structs + HashMap), writes — в ЛОКАЛЬНЫЙ
            // NodeResults каждого потока (no overlap by construction —
            // фронтир Кана гарантирует независимость узлов яруса).
            // rayon slice::par_iter().map().collect() — это и есть
            // collect-then-reduce: fixed order после sort_by_key(index).
            let mut results: Vec<NodeResults> = level
                .par_iter()
                .map(|&index| eval_node(canvas, whatif, data, &obj_names, &solutions, index))
                .collect();
            // Контракт §5.7.3: sort by index ascending перед merge —
            // детерминированный порядок (побитовая идентичность чисел
            // эталонов ADR-0005/0006). rayon par_iter preserve order
            // для slice, но sort — defensive (контракт явный).
            // Никакого par_iter().reduce() — недетерминированный порядок
            // float-агрегатов (архдок §5.6).
            results.sort_by_key(|r| r.index);
            for r in results {
                merge_node_results(&mut solutions, canvas, r);
            }
        }
    }
    // Однопоточный flatten-путь: wasm32 (нет std::thread) или без фичи
    // `parallel` (B2B zero-dep инвариант, контракт §5.6/§5.8).
    #[cfg(not(all(feature = "parallel", not(target_arch = "wasm32"))))]
    {
        let order = topo_sort(canvas)?;
        for index in order {
            let r = eval_node(canvas, whatif, data, &obj_names, &solutions, index);
            merge_node_results(&mut solutions, canvas, r);
        }
    }
    Ok(solutions)
}

// --- FR-066 (M5/S3): Monte Carlo + QMC-движок ----------------------------------

/// FR-066 (M5/S3, волна S ADR-0008): Monte Carlo + QMC-прогон — слой
/// L4/P3 поверх [`propagate_with_lines`]. Выполняет `N ≥ 10⁴` прогонов
/// расчётного графа с распределёнными параметрами
/// ([`crate::expr::mc::McConfig::params`] → построчные what-if подмены,
/// Normal/LogNormal/Exp/Poisson из FR-063) и сводит результаты к
/// квантилям P50/P90/P99 (collect-then-reduce, §5.7.3).
///
/// **СИБЛИНГ** (контракт §5.1): `propagate_with_lines` НЕ трогается —
/// сигнатура и поведение стабильны; M5 только добавляет эту функцию.
/// Реализация и контракты детерминизма — [`crate::expr::mc`]
/// (сид `hash(content) ⊕ scenario_seed ⊕ run_idx` §5.7.2, ChaCha8,
/// чанки 256/задача за фичей `parallel`, версия движка в `canvas.extra`
/// §5.7.4). Фича `qmc` не собирается на wasm (§5.8).
#[cfg(feature = "qmc")]
pub fn propagate_monte_carlo(
    canvas: &Canvas,
    mc_config: &crate::expr::mc::McConfig,
) -> Result<crate::expr::mc::McResult, CycleError> {
    crate::expr::mc::propagate_monte_carlo(canvas, mc_config)
}

/// [`propagate_monte_carlo`] с прогрессом `(completed, total)` после
/// каждого завершённого прогона (FR-066 P1: «прогресс через callback/
/// AppEvent» — UI показывает N/total и ETA; AppEvent-интеграция —
/// ответственность приложения).
#[cfg(feature = "qmc")]
pub fn propagate_monte_carlo_with_progress(
    canvas: &Canvas,
    mc_config: &crate::expr::mc::McConfig,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> Result<crate::expr::mc::McResult, CycleError> {
    crate::expr::mc::propagate_monte_carlo_with_progress(canvas, mc_config, progress)
}

/// FR-065 P2: результат вычисления одной ноды — собирается в параллельном
/// пути каждым потоком в свой аккумулятор, затем merge-ится в `solutions`
/// в детерминированном порядке (sort by `index` ascending — контракт §5.7.3
/// collect-then-reduce). Поля `lines`/`named` хранят ключи БЕЗ `id`
/// (id восстанавливается из `index` при merge — экономия на clone).
struct NodeResults {
    index: usize,
    /// `Some(outcome)` — значение ноды (Ok/Err); `None` — нода без формулы
    /// и без последней формульной строки (поведение `continue` оригинального
    /// цикла: `outputs.insert` не выполнялся).
    output: Option<Result<Value, EvalError>>,
    /// Построчные выходы: (line_index, value).
    lines: Vec<(usize, Value)>,
    /// Именованные выходы: (name, value).
    named: Vec<(String, Value)>,
    /// `Some(vec)` — есть предупреждения о конфликтах toParam; `None` — нет.
    warnings: Option<Vec<String>>,
}

/// FR-065 P2: вычисление одной ноды в подгонке окружения (чистая функция,
/// не мутирует `solutions` — только читает входы predecessors из
/// `&solutions`). Та же логика, что в оригинальном цикле `for index in order`
/// (`flow.rs:488` до рефактора) — вынесена в функцию для параллельного
/// вызова из `std::thread::scope`. Контракт §5.1: сигнатуры внешних
/// `propagate_with_lines`/`propagate_with_lines_data` НЕ меняются.
fn eval_node(
    canvas: &Canvas,
    whatif: &WhatIfOverrides,
    data: &DataSnapshots,
    obj_names: &QualifiedNames,
    solutions: &FlowSolutions,
    index: usize,
) -> NodeResults {
    let node = &canvas.nodes[index];
    let id = &node.id;
    // FR-014/FR-017: value-level подмена заменяет формулу целиком
    if let Some(value) = whatif.node_values.get(id) {
        return NodeResults {
            index,
            output: Some(Ok(value.clone())),
            lines: Vec::new(),
            named: Vec::new(),
            warnings: None,
        };
    }
    // FR-017: построчные подмены → виртуальный исходник текста
    // (пустой список — persisted-текст как есть, нулевой оверхед).
    let line_overrides = whatif.line_overrides(id);
    let text = whatif_virtual_text(&node.text.clone().unwrap_or_default(), &line_overrides);
    // FR-029: входы по адресации — позиционные слоты (рёбра без
    // toParam) и карта проливания в параметры (рёбра с toParam)
    // FR-045: снапшоты CSV — колонки data-нод адресуются fromOutput
    // FR-050 Р-6: + таблица резолва именованных путей «Объект.Поле»
    let inbound = inbound_values(
        canvas,
        id,
        &solutions.outputs,
        &solutions.lines,
        &solutions.named,
        data,
        obj_names,
    );
    let warnings = if !inbound.conflicts.is_empty() {
        Some(
            inbound
                .conflicts
                .iter()
                .map(|name| {
                    format!(
                        "несколько value-рёбер в параметр {name}: побеждает последнее по canvas.edges"
                    )
                })
                .collect(),
        )
    } else {
        None
    };
    let env = if inbound.slots.is_empty() {
        Env::empty()
    } else {
        Env::with_inbound(inbound.slots.clone())
    };
    // FR-050 Р-6 (таблица резолва имён): именованные пути «Объект.Поле»
    // → значения входящих рёбер — доступны и формулам шаблона, и
    // строкам листа, и what-if-подменам (RHS считается в этом же
    // окружении каскада Р-1).
    let env = if inbound.qualified.is_empty() {
        env
    } else {
        env.with_qualified(inbound.qualified.clone().into_iter().collect())
    };
    // FR-050 Р-1 (приоритет источников значения): параметры собираются
    // каскадом «локальный дефолт → проливание (`toParam`, «проливание
    // сильнее дефолта», FR-029) → what-if подмена строки-параметра
    // (перекрывает всё, FR-017 поверх, runtime-only)». Инвариант Р-1:
    // снятие what-if возвращает проливание, удаление ребра — локальное
    // значение. Н5: безразмерное пролитое значение трактуется в
    // единицах приёмника (Numi-семантика: «500» в параметр rps =
    // 500 rps); значение с единицей приходит как есть (в том числе в
    // безразмерный параметр); E-UNIT — только несовместимые размерности
    // (validate, FR-032).
    let template = node.template();
    let env = match &template {
        Some(tpl) => {
            let mut params = tpl.param_values();
            // Каскад, шаг 2: проливание перекрывает локальные значения.
            for (name, value) in &inbound.spill {
                if let Some(v) = value {
                    params.insert(name.clone(), spill_value_in_param_units(tpl, name, v));
                }
            }
            // Каскад, шаг 3: what-if подмена перекрывает всё; RHS
            // вычисляется последовательно в окружении каскада (входы +
            // локальные + проливание + уже подменённые параметры).
            if !line_overrides.is_empty() {
                let mut env_params = env.clone().with_param_map(params.clone());
                for (_, expr) in &line_overrides {
                    if let Some((name, value)) = override_assignment(expr, &env_params) {
                        params.insert(name.clone(), value.clone());
                        env_params = env_params.with_param_map(params.clone());
                    }
                }
            }
            env.with_param_map(params)
        }
        None => {
            // Текстовая нода: спецификаций параметров нет — проливание
            // напрямую в карту окружения (what-if действует через
            // виртуальный исходник текста выше, FR-017).
            if inbound.spill.is_empty() {
                env
            } else {
                let resolved: BTreeMap<String, Value> = inbound
                    .spill
                    .iter()
                    .filter_map(|(name, value)| value.as_ref().map(|v| (name.clone(), v.clone())))
                    .collect();
                env.with_param_map(resolved)
            }
        }
    };
    // FR-025 (правка 2): значение КАЖДОЙ формульной строки текста —
    // кандидат построчной точки выхода, теперь и у шаблонных нод
    // (лист параметров — присваивания со значениями). Ошибки строк
    // и проза значений не дают. FR-029: финальное окружение листа
    // (переменные) сохраняется — именованные выходы текстовой ноды.
    // FR-017: вычисляется ВИРТУАЛЬНЫЙ исходник (подменённые строки).
    let (line_outcomes, sheet_env) = expr::eval_lines_with_env(&text, &env);
    let mut lines: Vec<(usize, Value)> = Vec::new();
    for (line_index, line_outcome) in line_outcomes.iter().enumerate() {
        if let Some(ExprOutcome::Ok(value)) = line_outcome {
            lines.push((line_index, value.clone()));
        }
    }
    // FR-029: именованные выходы ноды — адресация `fromOutput`.
    // Шаблонная нода: снапшот outputs манифеста (Line(i) — из
    // построчных значений; Expr(s) — вычисление в окружении ноды —
    // входы + параметры + проливание). Текстовая нода: переменные
    // Numi-листа (последнее определение имени — «строка сдвинулась,
    // связь жива").
    let mut named: Vec<(String, Value)> = Vec::new();
    match &template {
        Some(tpl) => {
            for spec in &tpl.outputs {
                let value = match &spec.source {
                    // OutputSource::Line(line) в оригинале смотрит в
                    // solutions.lines — туда только что insert-нуты
                    // строки ЭТОЙ же ноды. Локально: look-up в MY OWN lines.
                    OutputSource::Line(line) => lines
                        .iter()
                        .find(|(i, _)| i == line)
                        .map(|(_, v)| v.clone()),
                    OutputSource::Expr(source) => expr::parse(source)
                        .ok()
                        .and_then(|parsed| expr::eval(&parsed, &env).ok()),
                };
                if let Some(value) = value {
                    named.push((spec.name.clone(), value));
                }
            }
        }
        None => {
            for (name, value) in sheet_env.vars_iter() {
                named.push((name.clone(), value.clone()));
            }
        }
    }
    // Значение ноды: шаблонная формула (FR-018), явная формула
    // `canvasdesk.expr` (MCP) или — для обычных заметок — последняя
    // формульная строка Numi-листа (FR-013: «итог заметки — последняя
    // формульная строка»; живой UI-путь: пользователь пишет
    // «1200 + 480» в заметке и тянет value-ребро). Проза/пустой текст
    // значения не дают — нода не участвует в потоке.
    let outcome = match &template {
        Some(tpl) => expr::parse(&tpl.expr)
            .map_err(|err| EvalError::BadFormula(err.to_string()))
            .and_then(|parsed| expr::eval(&parsed, &env)),
        None => match node.expr() {
            Some(formula) => expr::parse(formula)
                .map_err(|err| EvalError::BadFormula(err.to_string()))
                .and_then(|parsed| expr::eval(&parsed, &env)),
            None => {
                let last = line_outcomes.into_iter().flatten().last();
                match last {
                    Some(ExprOutcome::Ok(value)) => Ok(value),
                    Some(ExprOutcome::Err(msg)) => Err(EvalError::BadFormula(msg)),
                    None => {
                        // Поведение `continue` оригинального цикла: нода
                        // без формулы и без последней строки — нет output,
                        // но lines/named/warnings сохраняются.
                        return NodeResults {
                            index,
                            output: None,
                            lines,
                            named,
                            warnings,
                        };
                    }
                }
            }
        },
    };
    NodeResults {
        index,
        output: Some(outcome),
        lines,
        named,
        warnings,
    }
}

/// FR-065 P2: слияние `NodeResults` в `FlowSolutions` (детерминированный
/// порядок — caller sort-ит по `index` ascending). Вставки ключей по `id`
/// ноды (id восстанавливается из `canvas.nodes[index]`); коллизий БЕЗ по
/// построению (одна нода = один id; в параллельном пути ярус не содержит
/// повторов индексов).
fn merge_node_results(solutions: &mut FlowSolutions, canvas: &Canvas, r: NodeResults) {
    let id = canvas.nodes[r.index].id.clone();
    if let Some(outcome) = r.output {
        solutions.outputs.insert(id.clone(), outcome);
    }
    for (line_index, value) in r.lines {
        solutions.lines.insert((id.clone(), line_index), value);
    }
    for (name, value) in r.named {
        solutions.named.insert((id.clone(), name), value);
    }
    if let Some(warnings) = r.warnings {
        solutions.warnings.insert(id, warnings);
    }
}

/// Значения входящих value-рёбер ноды (в порядке `canvas.edges`) по карте
/// результатов: `Some(Some(v))` — значение источника; `Some(None)` — ребро
/// есть, значения нет (источник без формулы, с ошибкой или висячее ребро).
/// Слоты НЕ схлопываются — индексы `$1..$N` стабильны.
pub fn inbound_slots(canvas: &Canvas, node_id: &str, outputs: &FlowOutputs) -> Vec<Option<Value>> {
    inbound_slots_with_lines(
        canvas,
        node_id,
        outputs,
        &LineOutputs::new(),
        &NamedOutputs::new(),
    )
}

/// FR-050 этап F: окружение для ПОСТРОЧНОГО вычисления листа ноды —
/// позиционные слоты `$1..$N` + qualified-карта «Объект.Поле» (те же
/// ключи, что регистрирует узловой propagate в `inbound_values`):
/// формулы строк резолвят именованные ссылки ТОЧНО так же, как узловой
/// итог (инвариант «строка и узел видят одно окружение»). Без этого
/// формула строки с именованной ссылкой краснела «вход не найден» при
/// верном узловом итоге (найдено миграцией схем FR-049, этап F).
/// Рёбра с `toParam` в слоты не входят, но qualified-ключи регистрируют
/// (зеркало `inbound_values`); источник без значения — ключ не
/// регистрируется (видимая ошибка, диагностика Р-3).
pub fn line_eval_env(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
) -> Env {
    let obj_names = QualifiedNames::build(canvas);
    let mut slots: Vec<Option<Value>> = Vec::new();
    let mut qualified: HashMap<(String, String), Value> = HashMap::new();
    // Проливание в параметры (toParam, побеждает последнее ребро —
    // зеркально inbound_values/progagate): у шаблонной ноды — поверх
    // значений параметров манифеста, у текстовой — вся карта параметров.
    let mut spill: BTreeMap<String, Value> = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let value = edge_source_value(edge, outputs, lines, named);
        for key in obj_names.edge_keys(canvas, edge) {
            if let Some(v) = &value {
                qualified.insert(key, v.clone());
            }
        }
        match &edge.to_param {
            None => slots.push(value),
            Some(name) => {
                if let Some(v) = value {
                    spill.insert(name.clone(), v);
                }
            }
        }
    }
    let mut params: BTreeMap<String, Value> = canvas
        .node(node_id)
        .and_then(|node| node.template().map(|template| template.param_values()))
        .unwrap_or_default();
    for (name, value) in spill {
        params.insert(name, value);
    }
    let env = if slots.is_empty() {
        Env::empty()
    } else {
        Env::with_inbound(slots)
    };
    let env = if params.is_empty() {
        env
    } else {
        env.with_param_map(params)
    };
    if qualified.is_empty() {
        env
    } else {
        env.with_qualified(qualified)
    }
}

/// FR-025: [`inbound_slots`] с построчными выходами: ребро с
/// `from_line = Some(i)` уносит значение строки `i` источника; строка
/// удалена/стала прозой/ошибка — слот `Some(None)` (тихая деградация,
/// согласована с принципом тишины прозы Numi). Ребро без `from_line` —
/// значение ноды целиком (текущее поведение, инвариант флага FR-025).
///
/// FR-029 (семантика адресации рёбер, общая с [`inbound_values`]):
/// `fromOutput` — именованный выход источника (снапшот `outputs` шаблона
/// или переменная Numi-листа текстовой ноды), тихая деградация `None`;
/// приоритет при обоих полях — `fromLine`. Рёбра с `toParam` в слоты
/// НЕ входят (они «проливаются» в параметры — [`inbound_values`]); в
/// канвасах без адресованных рёбер поведение идентично прежнему.
/// `named` — карта именованных выходов потока (`FlowSolutions::named`):
/// до FR-049 карта передавалась пустой и `fromOutput`-рёбра тихо теряли
/// значение в построчных бейджах (красный «вход отсутствует: $1» при
/// корректном узловом итоге) — регресс-тест
/// `inbound_slots_resolve_named_outputs`.
pub fn inbound_slots_with_lines(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
) -> Vec<Option<Value>> {
    canvas
        .edges
        .iter()
        .filter(|edge| {
            edge.to_node == node_id
                && edge.flow_kind() == FlowKind::Value
                && edge.to_param.is_none()
        })
        .map(|edge| edge_source_value(edge, outputs, lines, named))
        .collect()
}

/// FR-029: значение, которое несёт value-ребро (источник по адресации):
/// `fromLine` → строка листа; иначе `fromOutput` → именованный выход;
/// иначе значение ноды целиком. Сломанная адресация — `None` (тихая
/// деградация: проваленный исток не роняет пересчёт downstream).
pub(crate) fn edge_source_value(
    edge: &Edge,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
) -> Option<Value> {
    if let Some(line) = edge.from_line {
        lines.get(&(edge.from_node.clone(), line)).cloned()
    } else if let Some(name) = &edge.from_output {
        named.get(&(edge.from_node.clone(), name.clone())).cloned()
    } else {
        outputs
            .get(&edge.from_node)
            .and_then(|result| result.as_ref().ok())
            .cloned()
    }
}

/// FR-045 R-2 (проливание колонок CSV, §Q3): [`edge_source_value`] со
/// снапшотами data-нод. Семантика адресации data-ноды (снапшот присутствует
/// в карте — карту заполняет приложение только для CSV):
/// - `fromOutput` = колонка; `fromLine` = запись снапшота (0-based;
///   для data-ноды адресует СТРОКУ ТАБЛИЦЫ, не строку листа); `fromLine`
///   нет — первая запись (детерминированное PoC-правило; пустой снапшот —
///   `None`);
/// - колонки нет в снапшоте / запись вне диапазона / ячейка пустая или
///   текстовая — `None` (unmapped, R-3: «значение не подставлено»);
/// - ребро без `fromOutput` — значения не несёт (значение data-ноды
///   «целиком» не определено: адресация колонки обязательна; согласовано
///   с fallback `edge.id` в `dataref::display_ref`);
/// - снапшота нет (dacdb/db PoC, CSV не загружен) — легаси-путь: у
///   data-ноды формулы обычно нет → `None` (источник pending, R-3).
///
/// What-if override data-ноды ([`WhatIfOverrides::node_values`]) подменяет
/// значение ноды, но НЕ колонки (§Q3 — связь с what-if открыта).
pub(crate) fn edge_source_value_with_data(
    edge: &Edge,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
    data: &DataSnapshots,
) -> Option<Value> {
    if let Some(snapshot) = data.get(&edge.from_node) {
        if let Some(field) = &edge.from_output {
            let row = edge.from_line.unwrap_or(0);
            return snapshot
                .cell(row, field)
                .and_then(crate::csv::csv_cell_value)
                .map(Value::scalar);
        }
        if edge.from_line.is_some() {
            // Запись без колонки — скаляра нет: значение не подставлено.
            return None;
        }
        // Ребро без адресации — легаси-путь ниже (значение ноды целиком;
        // у data-ноды формулы обычно нет → None → unmapped).
    }
    edge_source_value(edge, outputs, lines, named)
}

/// FR-029: входящие value-рёбра ноды по адресации — позиционные слоты
/// `$1..$N` (рёбра без `toParam`, порядок `canvas.edges`) и карта
/// проливания в параметры (рёбра с `toParam`; несколько рёбер в один
/// параметр — последнее по `canvas.edges` побеждает, имя попадает в
/// `conflicts` для предупреждения `flow_recalc`/`graph_validate`).
fn inbound_values(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
    data: &DataSnapshots,
    obj_names: &QualifiedNames,
) -> InboundValues {
    let mut result = InboundValues::default();
    // параметры, в которые уже приходили рёбра (для детекции конфликтов)
    let mut seen_params: BTreeMap<String, usize> = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let value = edge_source_value_with_data(edge, outputs, lines, named, data);
        // FR-050 Р-6: ключи qualified-резолва — все адресные формы имени
        // истока × поле (регистрируются и позиционные рёбра, и toParam-
        // проливания: формула может адресовать значение по имени в обоих
        // случаях). Источник без значения — ключ не регистрируется (eval
        // даст видимую ошибку «вход не найден», диагностика Р-3 — отдельно).
        for key in obj_names.edge_keys(canvas, edge) {
            if let Some(v) = &value {
                result.qualified.insert(key, v.clone());
            }
        }
        match &edge.to_param {
            None => result.slots.push(value),
            Some(name) => {
                let count = seen_params.entry(name.clone()).or_insert(0);
                *count += 1;
                if *count == 2 {
                    result.conflicts.push(name.clone());
                }
                result.spill.insert(name.clone(), value);
            }
        }
    }
    result
}

/// FR-050 Р-6: адресные имена нод канваса для qualified-резолва
/// «Объект.Поле». Для каждой ноды — все формы адресации её отображаемого
/// имени (`dataref::node_display_name`):
/// - уникальное имя — «Заявки»;
/// - коллизия (несколько нод с одним именем): первая — «Заявки»,
///   последующие — «Заявки (2)», «Заявки (3)»… (нумерация по порядку
///   `canvas.nodes` — детерминизм, инвариант 6); ВСЕ одноимённые
///   дополнительно — алиас «Заявки (node_id)» (зеркало
///   `dataref::qualified_obj_name`, FR-045 Q2 — авто-строки и подписи
///   показывают эту форму, формула может адресовать любую).
pub(crate) struct QualifiedNames {
    /// node_id → адресные формы имени.
    names: HashMap<String, Vec<String>>,
}

impl QualifiedNames {
    pub(crate) fn build(canvas: &Canvas) -> Self {
        let display: Vec<String> = canvas
            .nodes
            .iter()
            .map(|node| crate::dataref::node_display_name(canvas, &node.id))
            .collect();
        let mut totals: HashMap<&str, usize> = HashMap::new();
        for name in display.iter() {
            *totals.entry(name.as_str()).or_insert(0) += 1;
        }
        let mut seen: HashMap<&str, usize> = HashMap::new();
        let mut names: HashMap<String, Vec<String>> = HashMap::new();
        for (node, name) in canvas.nodes.iter().zip(display.iter()) {
            let count = seen.entry(name.as_str()).or_insert(0);
            *count += 1;
            let mut forms = Vec::new();
            if *count == 1 {
                forms.push(name.clone());
            } else {
                forms.push(format!("{name} ({count})"));
            }
            if totals[name.as_str()] > 1 {
                forms.push(format!("{name} ({})", node.id));
            }
            names.insert(node.id.clone(), forms);
        }
        Self { names }
    }

    /// Все qualified-ключи value-ребра: (адресные формы имени истока) ×
    /// (поле — приоритет адресации как у `edge_source_value`:
    /// `fromOutput` → имя присваивания `fromLine` → `edge.id`).
    pub(crate) fn edge_keys(&self, canvas: &Canvas, edge: &Edge) -> Vec<(String, String)> {
        let field = source_field_name(canvas, edge);
        match self.names.get(&edge.from_node) {
            Some(forms) => forms
                .iter()
                .map(|obj| (obj.clone(), field.clone()))
                .collect(),
            None => Vec::new(),
        }
    }
}

/// FR-029 (визуализация проливания): параметр ноды, запитанный входящим
/// value-ребром с `toParam` — источник значения для отображения
/// («rps ← Traffic Profile · peak_rps»). Runtime-данные: не сериализуются.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpill {
    /// Имя параметра (адрес `toParam`; совпадает с именем присваивания
    /// в Numi-листе ноды).
    pub param: String,
    /// id ноды-источника значения.
    pub from_node: String,
    /// Заголовок источника для отображения (снимок имени шаблона /
    /// первая строка текста / label / id как последний фолбэк).
    pub from_label: String,
    /// FR-025: построчная адресация истока (индекс строки листа источника).
    pub from_line: Option<usize>,
    /// FR-029: именованный выход истока (секция outputs шаблона /
    /// переменная Numi-листа текстовой ноды).
    pub from_output: Option<String>,
}

/// FR-029: параметры ноды, запитанные входящими value-рёбрами с `toParam`.
/// Последнее ребро по `canvas.edges` в параметр побеждает — зеркало
/// семантики проливания в [`propagate_with_lines`] (тихая деградация:
/// источник удалён/без значения — spill в списке остаётся, значение
/// подставляет приложение из результатов пересчёта).
pub fn param_spills(canvas: &Canvas, node_id: &str) -> Vec<ParamSpill> {
    let mut by_param: BTreeMap<String, ParamSpill> = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let Some(param) = &edge.to_param else {
            continue;
        };
        let from_label = canvas
            .node(&edge.from_node)
            .map(spill_source_title)
            .unwrap_or_else(|| edge.from_node.clone());
        by_param.insert(
            param.clone(),
            ParamSpill {
                param: param.clone(),
                from_node: edge.from_node.clone(),
                from_label,
                from_line: edge.from_line,
                from_output: edge.from_output.clone(),
            },
        );
    }
    by_param.into_values().collect()
}

/// FR-050 Н2 (этап C): совместимо ли значение-источник drag с параметром
/// приёмника — для подсветки допустимых целей во время drag. Правила те же,
/// что у `E-UNIT`/Н5: скаляр с любой стороны совместим (безразмерное
/// значение трактуется в единицах приёмника), несовместимые размерности
/// при единицах с обеих сторон — нет. Неизвестный токен единицы параметра —
/// скаляр (тихая деградация, как в [`expr::unit_value`]).
pub fn value_param_compatible(value: &Value, param_unit: Option<&str>) -> bool {
    let expected = expr::unit_value(0.0, param_unit);
    crate::validate::dimensions_compatible(&value.unit, &expected.unit)
}

/// FR-050 Н4 (этап C): value-рёбра, занимающие параметр `param` ноды —
/// для диалога «Заменить источник?» (победитель — последнее по
/// `canvas.edges`). Легаси-дубли входят все: замена источника удаляет
/// каждое, восстанавливая инвариант «один вход на параметр» (Н4).
/// Пусто — параметр свободен.
pub fn occupying_param_edges<'a>(canvas: &'a Canvas, node_id: &str, param: &str) -> Vec<&'a Edge> {
    canvas
        .edges
        .iter()
        .filter(|edge| {
            edge.to_node == node_id
                && edge.flow_kind() == FlowKind::Value
                && edge.to_param.as_deref() == Some(param)
        })
        .collect()
}

/// FR-050 Н9-1 (этап E): волна подсветки каскада — value-рёбра downstream
/// от изменённых нод (`seeds`), с порядком = топологическое расстояние от
/// ближайшего seed (BFS по value-рёбрам вниз). Ребро от seed получает
/// порядок 0 и подсвечивается первым, волна «бежит» по потоку значений:
/// пользователь видит распространение изменения. Control-рёбра значения
/// не несут — в волну не входят; seed без исходящих value-рёбер — волна
/// пуста (менять нечего). Value-циклы невозможны (UI/MCP блокируют
/// создание), но BFS с посещёнными не зациклится и на чужих файлах.
/// Выход отсортирован по индексу ребра — детерминизм (инвариант 6).
pub fn spill_wave(canvas: &Canvas, seeds: &HashSet<String>) -> Vec<(usize, u32)> {
    // Расстояние ноды от ближайшего seed (0 — сам seed); BFS-очередь.
    let mut dist: HashMap<&str, u32> = HashMap::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for node in &canvas.nodes {
        if seeds.contains(&node.id) {
            dist.insert(node.id.as_str(), 0);
            queue.push_back(node.id.as_str());
        }
    }
    // Исходящие value-рёбра по нодам (список смежности).
    let mut outgoing: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, edge) in canvas.edges.iter().enumerate() {
        if edge.flow_kind() == FlowKind::Value {
            outgoing
                .entry(edge.from_node.as_str())
                .or_default()
                .push(index);
        }
    }
    // BFS: порядок ребра = расстояние его истока; приёмник получает
    // dist+1 при первом достижении (минимум по всем путям).
    let mut out: Vec<(usize, u32)> = Vec::new();
    while let Some(current) = queue.pop_front() {
        let level = dist[current];
        if let Some(edges) = outgoing.get(current) {
            for &index in edges {
                let to = canvas.edges[index].to_node.as_str();
                out.push((index, level));
                if !dist.contains_key(to) {
                    dist.insert(to, level + 1);
                    queue.push_back(to);
                }
            }
        }
    }
    out.sort_unstable();
    out
}

/// FR-045 R-3: вход без пролитого значения — производное pending-состояние
/// «значение не подставлено». Определение (§Решения Р-3): value-ребро
/// подключено к приёмнику, но значения нет (источник pending/ошибка/
/// отсутствует, `fromLine` пуст или вне диапазона, именованный выход/
/// колонка отсутствует в снапшоте). Контрольные рёбра значения не несут —
/// в состояние не входят.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedInput {
    /// id ребра (пунктирная отрисовка unmapped-ребра, FR-045 Р-3).
    pub edge_id: String,
    /// Позиционный слот `$N` (0-based среди позиционных рёбер) — для
    /// слотов; `None` — проливание в параметр.
    pub slot: Option<usize>,
    /// Имя параметра (`toParam`); `None` — позиционный слот.
    pub param: Option<String>,
}

/// FR-045 R-3: множество unmapped-входов ноды — производное состояние,
/// **не сериализуется** (в `.canvas` не пишется); вычисляется на каждом
/// пересчёте из готовых результатов [`propagate_with_lines`]. Порядок —
/// `canvas.edges` (детерминирован). Подстановка значения снимает состояние
/// автоматически (следующий пересчёт; инвариант 4 FR-045).
pub fn unmapped_inputs(
    canvas: &Canvas,
    node_id: &str,
    solutions: &FlowSolutions,
) -> Vec<UnmappedInput> {
    unmapped_inputs_with_data(canvas, node_id, solutions, &DataSnapshots::new())
}

/// FR-045 R-3: [`unmapped_inputs`] со снапшотами CSV — колонка data-ноды,
/// пролитая из снапшота, снимает состояние (та же резолюция, что в
/// [`propagate_with_lines_data`] — инвариант согласованности состояния).
pub fn unmapped_inputs_with_data(
    canvas: &Canvas,
    node_id: &str,
    solutions: &FlowSolutions,
    data: &DataSnapshots,
) -> Vec<UnmappedInput> {
    let mut slot: usize = 0;
    let mut result = Vec::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let value = edge_source_value_with_data(
            edge,
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
            data,
        );
        if value.is_none() {
            let (slot_no, param) = if edge.to_param.is_none() {
                let no = slot;
                (Some(no), None)
            } else {
                (None, edge.to_param.clone())
            };
            result.push(UnmappedInput {
                edge_id: edge.id.clone(),
                slot: slot_no,
                param,
            });
        }
        if edge.to_param.is_none() {
            slot += 1;
        }
    }
    result
}

/// FR-050 Р-4 (Н10-а — строка-проекция, решено раундом 4): авто-строка
/// приёмника — производная строка тела ноды для value-ребра БЕЗ `toParam`,
/// подключённого к ноде без ожидающего порта (позиционный слот не читается
/// формулами ноды — условие W-UNUSED-SLOT FR-032; шаблонная нода читает
/// `$параметры`, не `$N`). Display-level: НЕ сериализуется в `.canvas`
/// (round-trip байт-в-байт), в undo не участвует — единственный источник
/// истины ребро; повторный пересчёт даёт идентичную строку; удаление ребра
/// удаляет строку (конвертации в ручную нет — Р-5); ручная правка
/// невозможна до удаления связи.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoRow {
    /// id ноды-приёмника (строка отображается в теле этой ноды).
    pub node_id: String,
    /// id ребра — источник истины строки (Н10-а: производная ребра).
    pub edge_id: String,
    /// Позиционный слот (0-based среди позиционных value-рёбер, как
    /// `crate::dataref::InputRef::slot`); `$N` для тултипа = slot + 1
    /// (отображение `$N` на теле ноды запрещено FR-044 Р-3 — только тултип).
    pub slot: usize,
    /// Полный путь «Объект.Поле» (имена — единая точка сборки dataref,
    /// FR-045 Р-5; коллизия имён объектов — «Имя (node_id)»).
    pub path: String,
    /// Короткое имя поля (Н7: канвас LOD-2 вне stage — «Поле», полный
    /// путь — в тултипе).
    pub field: String,
    /// Пролитое значение слота; `None` — unmapped («не подставлено»,
    /// Р-3: пунктир + тултип «проблема + решение» — этапы C/D).
    pub value: Option<Value>,
}

impl AutoRow {
    /// FR-050 Р-4 (этап D): текст авто-строки — единая точка сборки для
    /// рендера (тело приёмника) и подгонки высоты (сцена): «Путь = значение
    /// единица»; unmapped — «Путь = —» (янтарная диагностика Р-3).
    /// D-1 (FR-061): композиция над [`AutoRow::display_parts`] — строка и
    /// структурные части собираются из одной точки (инвариант 2 FR-050,
    /// байт-паритет тестом).
    pub fn display_text(&self) -> String {
        let p = self.display_parts();
        if p.unit.is_empty() {
            format!("{} = {}", p.path, p.num)
        } else {
            format!("{} = {} {}", p.path, p.num, p.unit)
        }
    }

    /// D-1 (FR-061, табличное тело ноды): структурные части авто-строки —
    /// `path` (имя), `num` (число), `unit` (юнит) раздельно — ячейки
    /// таблицы (этап B row_grid) без повторного парсинга строки.
    /// Unmapped (`value = None`) → `num = "—"`, `unit` пуст (диагностика
    /// Р-3 сохраняется в той же ячейке числа). Части значения — те же, что
    /// [`crate::expr::Value::display_parts`]; сборка отображения —
    /// [`crate::expr::join_parts`].
    pub fn display_parts(&self) -> AutoRowParts {
        let (num, unit) = match &self.value {
            Some(value) => value.display_parts(),
            None => ("—".to_owned(), String::new()),
        };
        AutoRowParts {
            path: self.path.clone(),
            num,
            unit,
        }
    }
}

/// D-1 (FR-061, табличное тело ноды): части авто-строки для табличной
/// ячейки — путь (имя), число, юнит. `display_text` — композиция над
/// этими частями (единая точка сборки — инвариант 2 FR-050).
#[derive(Debug, Clone, PartialEq)]
pub struct AutoRowParts {
    pub path: String,
    pub num: String,
    pub unit: String,
}

/// FR-050 Р-4: авто-строки приёмника — производные данные пересчёта
/// ([`FlowSolutions`] активного состояния). Порядок — `canvas.edges`
/// (детерминирован; повторный пересчёт — идентичные строки, инвариант 2
/// FR-050).
pub fn auto_rows(canvas: &Canvas, node_id: &str, solutions: &FlowSolutions) -> Vec<AutoRow> {
    auto_rows_with_data(canvas, node_id, solutions, &DataSnapshots::new())
}

/// FR-050 Р-4: [`auto_rows`] со снапшотами CSV-источников — значения
/// data-нод резолвятся той же адресацией, что в пересчёте (FR-045 R-2:
/// `fromOutput` — колонка, `fromLine` — запись; снапшота нет — unmapped).
pub fn auto_rows_with_data(
    canvas: &Canvas,
    node_id: &str,
    solutions: &FlowSolutions,
    data: &DataSnapshots,
) -> Vec<AutoRow> {
    let Some(node) = canvas.node(node_id) else {
        return Vec::new();
    };
    // Позиционные value-рёбра приёмника — слоты $1..$N по порядку
    // `canvas.edges` (как `inbound_values`).
    let positional: Vec<&Edge> = canvas
        .edges
        .iter()
        .filter(|edge| {
            edge.to_node == node_id
                && edge.flow_kind() == FlowKind::Value
                && edge.to_param.is_none()
        })
        .collect();
    if positional.is_empty() {
        return Vec::new();
    }
    // «Ожидающий порт»: слот читается формулами ноды — `$N` (целые ≥ 1)
    // или `$in` (валиден при ровно одном входе) — зеркало логики
    // W-UNUSED-SLOT FR-032. Читается → значение уходит в формулу
    // (обычный поток FR-014), авто-строки нет.
    // FR-050 Р-6: именованный путь «Объект.Поле» тоже читает слот своего
    // ребра (ключи резолва — все адресные формы истока × поле).
    let refs = crate::validate::slot_references(node);
    let names = QualifiedNames::build(canvas);
    let counts = crate::dataref::display_name_counts(canvas);
    let mut result = Vec::new();
    for (slot, edge) in positional.iter().enumerate() {
        let read = (refs.in_ref && positional.len() == 1)
            || refs.slots.contains(&(slot + 1))
            || (!refs.qualified.is_empty()
                && names
                    .edge_keys(canvas, edge)
                    .iter()
                    .any(|k| refs.qualified.contains(k)));
        if read {
            continue;
        }
        let value = edge_source_value_with_data(
            edge,
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
            data,
        );
        let obj = crate::dataref::qualified_obj_name(canvas, &edge.from_node, &counts);
        let field = source_field_name(canvas, edge);
        result.push(AutoRow {
            node_id: node_id.to_owned(),
            edge_id: edge.id.clone(),
            slot,
            path: format!("{obj}.{field}"),
            field,
            value,
        });
    }
    result
}

/// Поле «Объект.Поле» авто-строки (Р-6, зеркалит приоритет адресации
/// `edge_source_value`): `fromOutput` — имя выхода (data-нода — колонка,
/// FR-045 R-2); `fromLine` — имя присваивания этой строки-источника
/// (fallback «строка N», отображение 1-based); ребро без адресации —
/// `edge.id` (значение ноды целиком, FR-045 Р-5).
fn source_field_name(canvas: &Canvas, edge: &Edge) -> String {
    spill_source_field(
        canvas,
        &edge.from_node,
        edge.from_output.as_deref(),
        edge.from_line,
        &edge.id,
    )
}

/// FR-050 Н9-2 (этап D): поле квалифицированного пути «Объект.Поле» для
/// тултипа проливания — общий приоритет адресации для авто-строк (Р-4) и
/// подписей параметров (`toParam`): `fromOutput` → `fromLine` → fallback.
pub fn spill_source_field(
    canvas: &Canvas,
    from_node: &str,
    from_output: Option<&str>,
    from_line: Option<usize>,
    fallback: &str,
) -> String {
    if let Some(name) = from_output {
        return name.to_owned();
    }
    if let Some(line) = from_line {
        let raw = canvas
            .node(from_node)
            .and_then(|node| node.text.as_deref())
            .and_then(|text| text.lines().nth(line));
        if let Some(raw) = raw {
            if let crate::expr::NumiLineKind::Assignment { name } = crate::expr::line_kind(raw) {
                return name;
            }
        }
        return format!("строка {}", line + 1);
    }
    fallback.to_owned()
}

/// Заголовок ноды-источника для подписи проливания: снимок имени шаблона
/// (FR-023) / первая непустая строка текста (ATX-маркеры не показываем) /
/// label / id. Легковесный аналог заголовка карточки рендера: file/группы
/// для источника значения не бывает (формульные ноды — text-ноды).
pub(crate) fn spill_source_title(node: &crate::model::Node) -> String {
    if let Some(name) = node
        .template()
        .and_then(|template| template.name)
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    if let Some(line) = node
        .text
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
    {
        return line.trim_start_matches('#').trim().to_owned();
    }
    node.label
        .as_deref()
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| node.id.clone())
}

/// FR-029 (визуализация проливания): заменить строки-присваивания
/// пролитых параметров на подпись источника — `name ← from_label[ · output]`
/// (как на карточке: вместо локального литерала показываем, ОТКУДА пришло
/// значение). Индексы строк сохраняются (меняется содержимое, не структура)
/// — построчные результаты и порты FR-025 остаются на своих рядах.
/// `spills` — кортежи `(параметр, заголовок источника, именованный выход)`.
/// Строка без `=`, с пустым/многословным именем или чужим параметром —
/// без изменений; список пуст или совпадений нет — исходный текст (копия).
pub fn substitute_spilled_lines<'a>(
    text: &str,
    spills: impl IntoIterator<Item = (&'a str, &'a str, Option<&'a str>)>,
) -> String {
    let spills: Vec<(&str, &str, Option<&str>)> = spills.into_iter().collect();
    if spills.is_empty() {
        return text.to_owned();
    }
    let mut changed = false;
    let lines: Vec<Cow<str>> = text
        .split('\n')
        .map(|line| {
            let Some((name, _)) = line.split_once('=') else {
                return Cow::Borrowed(line);
            };
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Cow::Borrowed(line);
            }
            match spills.iter().find(|(param, _, _)| *param == name) {
                Some((_, label, output)) => {
                    changed = true;
                    let suffix = output.map(|out| format!(" · {out}")).unwrap_or_default();
                    Cow::Owned(format!("{name} ← {label}{suffix}"))
                }
                None => Cow::Borrowed(line),
            }
        })
        .collect();
    if !changed {
        return text.to_owned();
    }
    lines.join("\n")
}

/// Путь по value-рёбрам от `start` до `goal` (включая концы) — участники
/// цикла при добавлении ребра `goal → start`. `None` — пути нет, ребро
/// цикл не замыкает. Поиск в ширину; путь детерминирован (первый найденный
/// в порядке `canvas.edges`).
pub fn value_path(canvas: &Canvas, start: &str, goal: &str) -> Option<Vec<String>> {
    if canvas.node(start).is_none() || canvas.node(goal).is_none() {
        return None;
    }
    // parents: нода → откуда пришли
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    queue.push_back(start.to_owned());
    visited.insert(start.to_owned());
    while let Some(current) = queue.pop_front() {
        if current == goal {
            // Восстановить путь start → … → goal
            let mut path = vec![goal.to_owned()];
            let mut node = goal;
            while node != start {
                let parent = parents.get(node)?;
                path.push(parent.clone());
                node = parent;
            }
            path.reverse();
            return Some(path);
        }
        for edge in &canvas.edges {
            if edge.flow_kind() != FlowKind::Value || edge.from_node != current {
                continue;
            }
            let next = &edge.to_node;
            if visited.insert(next.clone()) {
                parents.insert(next.clone(), current.clone());
                queue.push_back(next.clone());
            }
        }
    }
    None
}

/// Замкнёт ли новое value-ребро `from → to` цикл (быстрый предикат для UI
/// и MCP поверх [`value_path`]).
pub fn creates_value_cycle(canvas: &Canvas, from: &str, to: &str) -> bool {
    value_path(canvas, to, from).is_some()
}

/// Сериализация результатов для отображения/MCP (FR-014): id ноды →
/// отображаемое значение (`Ok`) или текст ошибки (`Err`).
pub fn outputs_display(outputs: &FlowOutputs) -> HashMap<String, Result<String, String>> {
    outputs
        .iter()
        .map(|(id, result)| {
            let mapped = match result {
                Ok(value) => Ok(value.to_string()),
                Err(err) => Err(err.to_string()),
            };
            (id.clone(), mapped)
        })
        .collect()
}

// --- Тесты (верификационный список FR-014 + регрессии топологии) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};
    use std::str::FromStr;

    /// Сцена: ноды с формулами + value-ребро.
    fn node_with_expr(canvas: &mut Canvas, id: &str, formula: &str, x: f32) {
        let mut node = Node::text(id, id, x, 0.0);
        node.set_expr(Some(formula.to_owned()));
        canvas.nodes.push(node);
    }

    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        canvas.add_edge(edge);
    }

    fn control_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Control);
        canvas.add_edge(edge);
    }

    /// Верификация FR-014: пустой граф — пустой порядок.
    #[test]
    fn topo_sort_empty_canvas() {
        let canvas = Canvas::default();
        assert_eq!(topo_sort(&canvas), Ok(Vec::new()));
    }

    /// Изолированные ноды без рёбер — порядок по индексам (детерминизм Кана).
    #[test]
    fn topo_sort_no_edges() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "b", "2", 0.0);
        node_with_expr(&mut canvas, "a", "1", 1.0);
        assert_eq!(topo_sort(&canvas), Ok(vec![0, 1]));
    }

    /// Верификация FR-014: цепочка A→B→C (value) — [A, B, C].
    #[test]
    fn topo_sort_chain() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        assert_eq!(topo_sort(&canvas), Ok(vec![0, 1, 2]));
    }

    /// Верификация FR-014: цикл A→B→A — Err с участниками ["A", "B"].
    #[test]
    fn topo_sort_cycle_two_nodes() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "A");
        let err = topo_sort(&canvas).expect_err("цикл");
        assert_eq!(err.nodes, vec!["A".to_owned(), "B".to_owned()]);
    }

    /// Цикл из трёх участников + downstream-нода за циклом — в участники
    /// downstream НЕ входит (она не на цикле).
    #[test]
    fn topo_sort_cycle_three_with_downstream() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        node_with_expr(&mut canvas, "D", "4", 3.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        value_edge(&mut canvas, "e3", "C", "A");
        value_edge(&mut canvas, "e4", "C", "D");
        let err = topo_sort(&canvas).expect_err("цикл A→B→C→A");
        assert_eq!(
            err.nodes,
            vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]
        );
    }

    /// Петля A→A — цикл с единственным участником.
    #[test]
    fn topo_sort_self_loop() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "A");
        value_edge(&mut canvas, "e2", "A", "B");
        let err = topo_sort(&canvas).expect_err("петля");
        assert_eq!(err.nodes, vec!["A".to_owned()]);
    }

    /// Контрольные рёбра НЕ создают циклов (визуальные связи свободны).
    #[test]
    fn topo_sort_control_edges_ignore_cycles() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        control_edge(&mut canvas, "e1", "A", "B");
        control_edge(&mut canvas, "e2", "B", "A");
        let order = topo_sort(&canvas).expect("контрольные рёбра не создают циклов");
        assert_eq!(order.len(), 2);
    }

    /// Верификация FR-014: ромб A→B, A→C, B→D, C→D — D последняя.
    #[test]
    fn topo_sort_diamond() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        node_with_expr(&mut canvas, "D", "4", 3.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "A", "C");
        value_edge(&mut canvas, "e3", "B", "D");
        value_edge(&mut canvas, "e4", "C", "D");
        let order = topo_sort(&canvas).expect("DAG");
        assert_eq!(order.last(), Some(&3), "D — последняя");
        assert_eq!(order.len(), 4);
    }

    /// Висячее value-ребро (конец не существует) не входит в граф.
    #[test]
    fn topo_sort_dangling_value_edge_ignored() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        value_edge(&mut canvas, "e1", "A", "ghost");
        assert_eq!(topo_sort(&canvas), Ok(vec![0]));
    }

    /// Верификация FR-014: цепочка A=5, B=$in × 2, C=$in + 1 → {5, 10, 11}.
    #[test]
    fn propagate_chain() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(5.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(10.0));
        assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(11.0));
    }

    /// Слияние потоков: C = $1 + $2 по двум входам (порядок — canvas.edges).
    #[test]
    fn propagate_merge_slots() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "3", 0.0);
        node_with_expr(&mut canvas, "B", "7", 1.0);
        node_with_expr(&mut canvas, "C", "$1 + $2", 2.0);
        value_edge(&mut canvas, "e1", "A", "C");
        value_edge(&mut canvas, "e2", "B", "C");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(10.0));
    }

    /// Контрольное ребро не переносит значение — вход отсутствует.
    #[test]
    fn propagate_control_edge_carries_nothing() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        control_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(5.0));
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Верификация FR-014: вход отсутствует — ошибка downstream, не падение
    /// (A удалена; висячее ребро даёт пустой слот).
    #[test]
    fn propagate_missing_inbound_is_per_node_error() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        // Удаляем A (каскадно уходит и ребро e1)
        canvas.remove_node(0);
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
        assert!(matches!(
            outputs["C"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Источник БЕЗ формулы (проза) — значение не даёт, downstream
    /// получает MissingInbound; сам источник в результатах отсутствует.
    #[test]
    fn propagate_prose_source_has_no_value() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "Встреча в 15:00", 0.0, 0.0));
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(!outputs.contains_key("A"), "проза не участвует");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Живой UI-путь FR-014: заметки с Numi-формулами в тексте (без
    /// `canvasdesk.expr`) участвуют в потоке — итог = последняя строка.
    #[test]
    fn propagate_text_source_line_formula() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "1200 + 480", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in / 3", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(1680.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(560.0));
    }

    /// Многострочный текст: проза не считается, итог — последняя строка.
    #[test]
    fn propagate_text_last_formula_line_is_value() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "смета:\n100\n200 × 2", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in - 30", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(400.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(370.0));
    }

    /// Ошибка построчной формулы источника — downstream видит «вход
    /// отсутствует» (ошибочный слот), а не значение. Построчные ошибки
    /// обоих нод приходят обёрнутыми в BadFormula (единый вариант графа).
    #[test]
    fn propagate_text_error_breaks_downstream() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "1 / 0", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in × 2", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(matches!(
            outputs["A"],
            Err(EvalError::BadFormula(ref msg)) if msg.contains("деление на ноль")
        ));
        assert!(matches!(
            outputs["B"],
            Err(EvalError::BadFormula(ref msg)) if msg.contains("вход отсутствует")
        ));
    }

    /// Явная формула `canvasdesk.expr` приоритетнее текста (совместимость).
    #[test]
    fn propagate_explicit_expr_beats_text() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("A", "1200 + 480", 0.0, 0.0);
        a.set_expr(Some("7".to_owned()));
        canvas.nodes.push(a);
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(7.0));
    }

    /// Ошибка формулы источника → downstream «вход отсутствует», остальной
    /// граф считается.
    #[test]
    fn propagate_source_error_does_not_stop_graph() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5 ms + 3 rps", 0.0); // UnitMismatch
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "solo", "7", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(outputs["A"].is_err(), "источник с ошибкой");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
        assert_eq!(outputs["solo"].as_ref().unwrap(), &Value::scalar(7.0));
    }

    /// Цикл value-рёбер — propagate возвращает ту же CycleError.
    #[test]
    fn propagate_cycle_is_error() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "A");
        let err = propagate(&canvas, &HashMap::new()).expect_err("цикл");
        assert_eq!(err.nodes, vec!["A".to_owned(), "B".to_owned()]);
    }

    /// What-if (FR-017): override значения ноды меняет её и downstream;
    /// формула ноды с override не вычисляется.
    #[test]
    fn propagate_overrides_flow_downstream() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let mut overrides = HashMap::new();
        overrides.insert("A".to_owned(), Value::scalar(100.0));
        let outputs = propagate(&canvas, &overrides).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(100.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(200.0));
    }

    /// Unit-значения текут по рёбрам: 1000 rps → $in / 4 → 250 rps.
    #[test]
    fn propagate_units_flow() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1000 rps", 0.0);
        node_with_expr(&mut canvas, "B", "$in / 4", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["B"].as_ref().unwrap().to_string(), "250 rps");
    }

    /// Верификация FR-014: round-trip flow.kind + Control по умолчанию.
    #[test]
    fn edge_flow_kind_round_trip() {
        let mut edge = Edge::new("e1", "A", None, "B", None);
        assert_eq!(edge.flow_kind(), FlowKind::Control, "дефолт — control");
        edge.set_flow_kind(FlowKind::Value);
        assert_eq!(edge.flow_kind(), FlowKind::Value);
        // Serialize → deserialize
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "A", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "B", 1.0, 0.0));
        canvas.add_edge(edge);
        let json = canvas.to_json().expect("сериализация");
        let restored = Canvas::from_str(&json).expect("парсинг");
        assert_eq!(restored.edges[0].flow_kind(), FlowKind::Value);
        // Сброс в Control убирает поле целиком (чистый round-trip)
        let mut restored_edge = restored.edges[0].clone();
        restored_edge.set_flow_kind(FlowKind::Control);
        assert_eq!(restored_edge.flow_kind(), FlowKind::Control);
        assert!(
            restored_edge.extra.get("canvasdesk").is_none(),
            "пустое расширение не хранится"
        );
    }

    /// Соседние неизвестные поля `canvasdesk` ребра не теряются при тогле.
    #[test]
    fn edge_flow_kind_preserves_siblings() {
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.extra.insert(
            "canvasdesk".to_owned(),
            serde_json::json!({ "note": "моё", "flow": { "kind": "control" } }),
        );
        edge.set_flow_kind(FlowKind::Value);
        assert_eq!(edge.flow_kind(), FlowKind::Value);
        let note = edge
            .extra
            .get("canvasdesk")
            .and_then(|ext| ext.get("note"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(note, Some("моё"), "чужие поля canvasdesk сохранены");
        edge.set_flow_kind(FlowKind::Control);
        let note = edge
            .extra
            .get("canvasdesk")
            .and_then(|ext| ext.get("note"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(note, Some("моё"), "и после сброса flow");
    }

    /// value_path: цикл A→B→C + попытка C→A — путь A→B→C.
    #[test]
    fn value_path_detects_cycle_participants() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        assert_eq!(
            value_path(&canvas, "A", "C"),
            Some(vec!["A".to_owned(), "B".to_owned(), "C".to_owned()])
        );
        assert!(creates_value_cycle(&canvas, "C", "A"), "C→A замкнёт цикл");
        assert!(
            !creates_value_cycle(&canvas, "A", "C"),
            "A→C — расширение существующего пути, не цикл"
        );
        // Контрольные рёбра не участвуют в путях: D→E только control —
        // value-пути E→…→D нет
        node_with_expr(&mut canvas, "D", "4", 3.0);
        node_with_expr(&mut canvas, "E", "5", 4.0);
        control_edge(&mut canvas, "e3", "D", "E");
        assert!(!creates_value_cycle(&canvas, "E", "D"));
    }

    /// Display CycleError — цепочка участников через « → ».
    #[test]
    fn cycle_error_display() {
        let err = CycleError {
            nodes: vec!["A".to_owned(), "B".to_owned()],
        };
        assert_eq!(err.to_string(), "цикл потока значений: A → B");
    }

    /// outputs_display: значения и ошибки в строках (для MCP/рендера).
    #[test]
    fn outputs_display_maps_to_strings() {
        let mut outputs: FlowOutputs = HashMap::new();
        outputs.insert("A".to_owned(), Ok(Value::scalar(5.0)));
        outputs.insert("B".to_owned(), Err(EvalError::MissingInbound { index: 0 }));
        let display = outputs_display(&outputs);
        assert_eq!(display["A"].as_ref().unwrap(), "5");
        assert!(display["B"].as_ref().unwrap_err().contains("вход"));
    }

    // --- FR-029: визуализация проливания (param_spills) ---

    /// value-ребро с toParam → spill с адресацией истока и заголовком
    /// (первая строка текста источника).
    #[test]
    fn param_spills_collects_to_param_edges() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "Traffic Profile — ADR005 (MVP)", 0.0, 0.0));
        node_with_expr(&mut canvas, "B", "$rps × 2", 1.0);
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("rps".to_owned());
        edge.from_output = Some("peak_rps".to_owned());
        canvas.add_edge(edge);
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills.len(), 1);
        assert_eq!(spills[0].param, "rps");
        assert_eq!(spills[0].from_node, "A");
        assert_eq!(spills[0].from_label, "Traffic Profile — ADR005 (MVP)");
        assert_eq!(spills[0].from_output, Some("peak_rps".to_owned()));
        assert_eq!(spills[0].from_line, None);
        // У ноды без входящих toParam-рёбер — пусто.
        assert!(param_spills(&canvas, "A").is_empty());
    }

    /// Два ребра в один параметр — побеждает последнее по canvas.edges
    /// (зеркало семантики проливания propagate).
    #[test]
    fn param_spills_last_edge_wins() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "C", "2", 1.0);
        node_with_expr(&mut canvas, "B", "$rps", 2.0);
        for (id, from) in [("e1", "A"), ("e2", "C")] {
            let mut edge = Edge::new(id, from, None, "B", None);
            edge.set_flow_kind(FlowKind::Value);
            edge.to_param = Some("rps".to_owned());
            canvas.add_edge(edge);
        }
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills.len(), 1, "один параметр — один spill");
        assert_eq!(spills[0].from_node, "C", "последнее ребро побеждает");
    }

    /// Control-рёбра и value-рёбра без toParam в список не попадают.
    #[test]
    fn param_spills_ignores_control_and_slot_edges() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "$1 + $rps", 1.0);
        let mut control = Edge::new("e1", "A", None, "B", None);
        control.to_param = Some("rps".to_owned());
        canvas.add_edge(control);
        let mut slot = Edge::new("e2", "A", None, "B", None);
        slot.set_flow_kind(FlowKind::Value);
        canvas.add_edge(slot);
        assert!(
            param_spills(&canvas, "B").is_empty(),
            "control с toParam и value без toParam — не проливание"
        );
    }

    /// Заголовок источника-шаблона — снимок имени шаблона (FR-023),
    /// а не первая строка листа параметров.
    #[test]
    fn param_spills_template_snapshot_name_as_label() {
        let mut canvas = Canvas::default();
        let mut source = Node::text("A", "rps = 1000 rps", 0.0, 0.0);
        source.set_template(Some(crate::templates::TemplateRef {
            id: "t".to_owned(),
            version: "1".to_owned(),
            expr: "$rps".to_owned(),
            params: Default::default(),
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: Some("Сервис аутентификации".to_owned()),
            outputs: Vec::new(),
        }));
        canvas.nodes.push(source);
        node_with_expr(&mut canvas, "B", "$token_verify × 2", 1.0);
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("token_verify".to_owned());
        edge.from_line = Some(1);
        canvas.add_edge(edge);
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills[0].from_label, "Сервис аутентификации");
        assert_eq!(spills[0].from_line, Some(1));
    }

    /// FR-050 Н2 (этап C): совместимость значения с параметром — правила
    /// Н5/E-UNIT: скаляры совместимы с любой стороной, одинаковые
    /// размерности совместимы, несовместимые — нет.
    #[test]
    fn value_param_compatible_h5_rules() {
        use crate::expr::{self, Value};
        // Скаляр в параметр с единицей — совместим (трактуется в единицах
        // приёмника)
        assert!(value_param_compatible(&Value::scalar(500.0), Some("rps")));
        // Единица в безразмерный параметр — совместим (приходит как есть)
        assert!(value_param_compatible(
            &Value::with_unit(1389.0, expr::unit_value(0.0, Some("rps")).unit),
            None
        ));
        // Совместимые размерности (rps → rps)
        assert!(value_param_compatible(
            &Value::with_unit(1389.0, expr::unit_value(0.0, Some("rps")).unit),
            Some("rps")
        ));
        // Конвертируемые масштабы одной размерности (ms → s)
        assert!(value_param_compatible(
            &Value::with_unit(200.0, expr::unit_value(0.0, Some("ms")).unit),
            Some("s")
        ));
        // Несовместимые размерности (ms в параметр rps) — НЕсовместимо
        assert!(!value_param_compatible(
            &Value::with_unit(200.0, expr::unit_value(0.0, Some("ms")).unit),
            Some("rps")
        ));
        // Скаляр в скаляр — совместимо
        assert!(value_param_compatible(&Value::scalar(2.0), None));
    }

    /// FR-050 Н4 (этап C): занимающие value-рёбра параметра — control-рёбра
    /// и чужие параметры не считаются; легаси-дубли входят все.
    #[test]
    fn occupying_param_edges_filters_and_collects_duplicates() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "C", "2", 1.0);
        node_with_expr(&mut canvas, "B", "$rps × $cpu", 2.0);
        // Value в rps (e1) + control с toParam=cpu (не считается) +
        // value без toParam (позиционный, не считается) + value в cpu (e4)
        let mut e1 = Edge::new("e1", "A", None, "B", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.to_param = Some("rps".to_owned());
        canvas.add_edge(e1);
        let mut control = Edge::new("e2", "A", None, "B", None);
        control.to_param = Some("rps".to_owned());
        canvas.add_edge(control);
        value_edge(&mut canvas, "e3", "A", "B");
        let mut e4 = Edge::new("e4", "C", None, "B", None);
        e4.set_flow_kind(FlowKind::Value);
        e4.to_param = Some("cpu".to_owned());
        canvas.add_edge(e4);
        // Легаси-дубль в rps
        let mut e5 = Edge::new("e5", "C", None, "B", None);
        e5.set_flow_kind(FlowKind::Value);
        e5.to_param = Some("rps".to_owned());
        canvas.add_edge(e5);

        let rps = occupying_param_edges(&canvas, "B", "rps");
        assert_eq!(
            rps.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["e1", "e5"],
            "value-рёбра в rps (дубль входит), control/позиционные — нет"
        );
        let cpu = occupying_param_edges(&canvas, "B", "cpu");
        assert_eq!(cpu.len(), 1);
        assert_eq!(cpu[0].id, "e4");
        assert!(
            occupying_param_edges(&canvas, "B", "unknown").is_empty(),
            "свободный параметр — пусто"
        );
        assert!(occupying_param_edges(&canvas, "A", "rps").is_empty());
    }

    /// Подмена строки-присваивания на подпись источника: индексы строк
    /// сохраняются, чужие строки и проза не тронуты.
    #[test]
    fn substitute_spilled_lines_rewrites_assignment() {
        let text = "rps = 1389 rps\ncache_hit = 0.6\nlatency = 200 ms";
        let out = substitute_spilled_lines(
            text,
            [("rps", "Traffic Profile — ADR005 (MVP)", Some("peak_rps"))],
        );
        assert_eq!(
            out,
            "rps ← Traffic Profile — ADR005 (MVP) · peak_rps\ncache_hit = 0.6\nlatency = 200 ms"
        );
        assert_eq!(out.lines().count(), 3, "структура строк сохранена");
    }

    /// Подмена без именованного выхода — подпись без суффикса «· output».
    #[test]
    fn substitute_spilled_lines_without_output() {
        let out = substitute_spilled_lines("load = 100", [("load", "CDN", None)]);
        assert_eq!(out, "load ← CDN");
    }

    /// Совпадений нет / список пуст — исходный текст без изменений.
    #[test]
    fn substitute_spilled_lines_no_match_is_identity() {
        let text = "a = 1\nпроза\nb = 2";
        assert_eq!(substitute_spilled_lines(text, []), text);
        assert_eq!(
            substitute_spilled_lines(text, [("c", "X", None)]),
            text,
            "чужой параметр — без изменений"
        );
        // Многословное имя до '=' (проза с равенством) не присваивание.
        assert_eq!(
            substitute_spilled_lines("it = quality = 5", [("it = quality", "X", None)]),
            "it = quality = 5"
        );
    }

    // --- FR-025: построчные точки выхода (значение строки в потоке) ---

    /// FR-025: propagate_with_lines собирает значение КАЖДОЙ формульной
    /// строки Numi-листа (присваивания и выражения); проза и ошибки — нет.
    #[test]
    fn propagate_collects_line_outputs() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("A", "", 0.0, 0.0);
        node.text = Some("встреча в 15:00\nrps = 1000\nlatency = 50 ms\nrps × latency".to_owned());
        canvas.nodes.push(node);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // Формульные строки 1, 2, 3 — значения есть (индексы ТЕКСТА;
        // числа сверяем по .num — юниты строк сохраняются: ms у latency)
        assert_eq!(
            solutions.lines[&("A".to_owned(), 1)].num,
            1000.0,
            "присваивание rps"
        );
        assert_eq!(
            solutions.lines[&("A".to_owned(), 2)].num,
            50.0,
            "присваивание latency (юнит ms сохранён)"
        );
        assert!(
            solutions.lines.contains_key(&("A".to_owned(), 3)),
            "выражение — строка 3"
        );
        // Проза (строка 0) значения не даёт
        assert!(!solutions.lines.contains_key(&("A".to_owned(), 0)));
        // Значение ноды = последняя формульная строка (инвариант FR-013)
        assert_eq!(
            solutions.outputs.get("A"),
            Some(&Ok(solutions.lines[&("A".to_owned(), 3)].clone()))
        );
    }

    /// FR-025 (правка 2, по проверке владельца): шаблонная нода даёт
    /// построчные выходы листа параметров (присваивания текста), узловое
    /// значение — формула шаблона (НЕ последняя строка листа).
    #[test]
    fn propagate_template_line_outputs() {
        use crate::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        let mut node = Node::text("T", "", 0.0, 0.0);
        node.text = Some("rps = 1000\nservers = 2".to_owned());
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "rps".to_owned(),
            TemplateParam {
                num: 1000.0,
                unit: None,
            },
        );
        params.insert(
            "servers".to_owned(),
            TemplateParam {
                num: 2.0,
                unit: None,
            },
        );
        node.set_template(Some(TemplateRef {
            id: "com.canvasdesk.test".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rps × $servers".to_owned(),
            params,
            icon: String::new(),
            color: String::new(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions.lines[&("T".to_owned(), 0)].num,
            1000.0,
            "строка 0 листа параметров (rps)"
        );
        assert_eq!(
            solutions.lines[&("T".to_owned(), 1)].num,
            2.0,
            "строка 1 листа параметров (servers)"
        );
        // Узловое значение — формула шаблона (2000), не последняя строка (2)
        assert_eq!(
            solutions.outputs.get("T"),
            Some(&Ok(Value::scalar(2000.0))),
            "значение шаблонной ноды — формула"
        );
    }

    /// FR-025 (правка 2): drag от строки листа параметров шаблонной ноды —
    /// downstream получает значение ИМЕННО этой строки (регрессия проверки
    /// владельца: передавалось только узловое значение — результат формулы).
    #[test]
    fn inbound_slot_from_template_line_carries_param_value() {
        use crate::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        let mut node = Node::text("T", "", 0.0, 0.0);
        node.text = Some("rps = 1000\nservers = 2".to_owned());
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "rps".to_owned(),
            TemplateParam {
                num: 1000.0,
                unit: None,
            },
        );
        params.insert(
            "servers".to_owned(),
            TemplateParam {
                num: 2.0,
                unit: None,
            },
        );
        node.set_template(Some(TemplateRef {
            id: "com.canvasdesk.test".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rps × $servers".to_owned(),
            params,
            icon: String::new(),
            color: String::new(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        // Ребро от строки 0 шаблонной ноды (rps = 1000)
        let mut line_edge = Edge::new("e1", "T", None, "B", None);
        line_edge.set_flow_kind(FlowKind::Value);
        line_edge.from_line = Some(0);
        canvas.add_edge(line_edge);

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(
            slots[0].as_ref().map(|value| value.num),
            Some(1000.0),
            "слот — значение строки 0 (rps), а не результат формулы (2000)"
        );
        // Downstream: 1000 × 2 (значение строки умножается в приёмнике)
        assert_eq!(
            solutions.outputs.get("B"),
            Some(&Ok(Value::scalar(2000.0))),
            "$in приёмника — значение строки шаблонной ноды"
        );
    }

    /// FR-025: слот value-ребра с `from_line` == значению строки-истока;
    /// ребро без `from_line` — значение ноды (инвариант флага).
    #[test]
    fn inbound_slots_from_line_carries_line_value() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        sheet.text = Some("rps = 1000\ncpu = 4\nrps × cpu".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        // Ребро 1: значение ноды целиком (последняя строка)
        value_edge(&mut canvas, "e1", "A", "B");
        // Ребро 2: построчный исток — строка 0 (rps)
        let mut line_edge = Edge::new("e2", "A", None, "B", None);
        line_edge.set_flow_kind(FlowKind::Value);
        line_edge.from_line = Some(0);
        canvas.add_edge(line_edge);

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(slots.len(), 2, "оба value-ребра");
        assert_eq!(
            slots[0],
            Some(solutions.lines[&("A".to_owned(), 2)].clone()),
            "без from_line — значение ноды (последняя строка)"
        );
        assert_eq!(
            slots[1].as_ref().map(|value| value.num),
            Some(1000.0),
            "from_line 0 — rps"
        );
    }

    /// FR-025: тихая деградация слота — строка удалена/ошибка/проза →
    /// `Some(None)` (как «источник без значения»), индексы `$N` стабильны.
    #[test]
    fn inbound_slots_missing_or_error_line_degrade_to_none() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        // Строка 0 — ошибка (деление на ноль), строка 1 — проза
        sheet.text = Some("x = 1 / 0\nпросто текст".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        let mut edge0 = Edge::new("e0", "A", None, "B", None);
        edge0.set_flow_kind(FlowKind::Value);
        edge0.from_line = Some(0);
        canvas.add_edge(edge0);
        let mut edge1 = Edge::new("e1", "A", None, "B", None);
        edge1.set_flow_kind(FlowKind::Value);
        edge1.from_line = Some(1);
        canvas.add_edge(edge1);
        // Строка 9 не существует вовсе
        let mut edge9 = Edge::new("e9", "A", None, "B", None);
        edge9.set_flow_kind(FlowKind::Value);
        edge9.from_line = Some(9);
        canvas.add_edge(edge9);

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(
            slots,
            vec![None, None, None],
            "ошибка/проза/нет строки — Some(None)"
        );
    }

    /// FR-049 (регресс слотов + fromOutput): построчные слоты резолвят
    /// именованные выходы текстовой ноды (`FlowSolutions::named`), а не
    /// тихо теряют значение. До фикса карта `named` не передавалась вовсе:
    /// приёмник с `$1`/`$2` от fromOutput-рёбер показывал красный бейдж
    /// «вход отсутствует» при корректном узловом итоге — паттерн адресации
    /// встроенных схем PRD-0008 этого не допускает.
    #[test]
    fn inbound_slots_resolve_named_outputs() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        sheet.text = Some("users = 1000\nsessions = 5".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$1 × $2", 1.0);
        let mut users_edge = Edge::new("e1", "A", None, "B", None);
        users_edge.set_flow_kind(FlowKind::Value);
        users_edge.from_output = Some("users".to_owned());
        canvas.add_edge(users_edge);
        let mut sessions_edge = Edge::new("e2", "A", None, "B", None);
        sessions_edge.set_flow_kind(FlowKind::Value);
        sessions_edge.from_output = Some("sessions".to_owned());
        canvas.add_edge(sessions_edge);

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(
            slots[0].as_ref().map(|value| value.num),
            Some(1000.0),
            "слот $1 — именованный выход users"
        );
        assert_eq!(
            slots[1].as_ref().map(|value| value.num),
            Some(5.0),
            "слот $2 — именованный выход sessions"
        );
        assert_eq!(
            solutions.outputs.get("B"),
            Some(&Ok(Value::scalar(5000.0))),
            "узловой итог B жив"
        );
    }

    /// Инвариант флага FR-025: для рёбер БЕЗ `from_line` построчная
    /// механика ничего не меняет — propagate даёт прежние значения.
    #[test]
    fn propagate_without_from_line_matches_legacy() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        sheet.text = Some("rps = 1000\nrps × 2".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");

        let legacy = propagate(&canvas, &HashMap::new()).expect("DAG");
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(legacy, solutions.outputs, "значения нод совпадают");
        assert_eq!(
            legacy.get("A").and_then(|r| r.as_ref().ok()),
            solutions.lines.get(&("A".to_owned(), 1)),
            "значение ноды == последняя формульная строка"
        );
    }

    // --- FR-029: именованные порты значений (fromOutput / toParam) ---

    /// Хелпер: шаблонная нода со снапшотом (параметры + outputs).
    /// Параметры — (имя, значение, юнит): Rate-параметры требуют "rps".
    fn template_node_with_outputs(
        canvas: &mut Canvas,
        id: &str,
        params: &[(&str, f64, Option<&str>)],
        expr: &str,
        outputs: &[(&str, &str)],
    ) {
        use crate::templates::{OutputSource, OutputSpec, TemplateParam, TemplateRef};
        let text = params
            .iter()
            .map(|(name, value, unit)| match unit {
                Some(unit) => format!("{name} = {value} {unit}"),
                None => format!("{name} = {value}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut node = Node::text(id, text, 0.0, 0.0);
        let mut param_map = std::collections::BTreeMap::new();
        for (name, value, unit) in params {
            param_map.insert(
                (*name).to_owned(),
                TemplateParam {
                    num: *value,
                    unit: unit.map(str::to_owned),
                },
            );
        }
        node.set_template(Some(TemplateRef {
            id: format!("mock.{id}"),
            version: "1.0.0".to_owned(),
            expr: expr.to_owned(),
            params: param_map,
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: None,
            outputs: outputs
                .iter()
                .map(|(name, expr)| OutputSpec {
                    name: (*name).to_owned(),
                    unit: None,
                    source: OutputSource::Expr((*expr).to_owned()),
                })
                .collect(),
        }));
        canvas.nodes.push(node);
    }

    /// value-ребро с адресацией портов (fromOutput / toParam).
    fn ported_value_edge(
        canvas: &mut Canvas,
        id: &str,
        from: &str,
        to: &str,
        from_output: Option<&str>,
        to_param: Option<&str>,
    ) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = from_output.map(str::to_owned);
        edge.to_param = to_param.map(str::to_owned);
        canvas.add_edge(edge);
    }

    /// FR-029: проливание входа в параметр шаблонной ноды — ребро с
    /// `toParam` подставляет значение как `$rps`, перекрывая локальное
    /// значение параметра (семантика «проливание сильнее дефолта»).
    /// Сценарий ADR-0005: `Трафик.peak_rps → gateway.rps`.
    #[test]
    fn spill_overrides_local_param() {
        let mut canvas = Canvas::default();
        // Текстовая нода «Трафик»: peak_rps = 2500
        let traffic = Node::text("traffic", "peak_rps = 2500 rps", 0.0, 0.0);
        canvas.nodes.push(traffic);
        // Шаблонная нода: utilization($rps, 1 req / 10 ms) — локальный
        // rps = 1000 дал бы ρ = 10; проливание 2500 даёт ρ = 25
        template_node_with_outputs(
            &mut canvas,
            "gateway",
            &[("rps", 1000.0, Some("rps"))],
            "utilization($rps, 1 req / 10 ms)",
            &[],
        );
        // value-ребро: значение traffic (2500) → параметр rps
        ported_value_edge(&mut canvas, "e1", "traffic", "gateway", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // utilization = 2500 / 100 = 25 — проливание перекрыло локальный 1000
        let utilization = solutions
            .outputs
            .get("gateway")
            .and_then(|result| result.as_ref().ok())
            .expect("формула шаблона вычислилась");
        assert!(
            (utilization.num - 25.0).abs() < 1e-9,
            "проливание перекрывает локальный параметр: {utilization:?}"
        );
        // Проливание видно и в именованном… нет — в значениях строк листа:
        // текст gateway (`rps = 1000`) НЕ переписывается (инвариант «без
        // правки формулы») — перекрытие только в окружении вычисления.
        let gateway_text = canvas
            .node("gateway")
            .and_then(|node| node.text.clone())
            .unwrap_or_default();
        assert!(
            gateway_text.contains("1000"),
            "текст ноды не мутируется проливанием"
        );
    }

    /// FR-029: ребро БЕЗ `toParam` — прежнее поведение FR-014:
    /// позиционный слот `$1` (обратная совместимость).
    #[test]
    fn edge_without_to_param_keeps_positional_slots() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "7", 0.0);
        node_with_expr(&mut canvas, "B", "$1 × 3", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("B")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(21.0),
            "позиционные слоты работают как до FR-029"
        );
    }

    /// FR-029: `fromOutput` на шаблонной ноде — именованный выход (Expr
    /// от параметров) адресуется downstream-ребром.
    #[test]
    fn from_output_template_named_output() {
        let mut canvas = Canvas::default();
        // CDN: rps = 1000, hit = 60% → выход origin_rps = $rps × (1 - $hit)
        template_node_with_outputs(
            &mut canvas,
            "cdn",
            &[("rps", 1000.0, Some("rps")), ("hit", 0.6, None)],
            "mm1($rps × (1 - $hit), 1 req / 200 ms)",
            &[("origin_rps", "$rps × (1 - $hit)")],
        );
        // Шаблонная нода-приёмник: lb с параметром connections_per_sec
        template_node_with_outputs(
            &mut canvas,
            "lb",
            &[("connections_per_sec", 100.0, Some("rps"))],
            "$connections_per_sec",
            &[],
        );
        ported_value_edge(
            &mut canvas,
            "e1",
            "cdn",
            "lb",
            Some("origin_rps"),
            Some("connections_per_sec"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // Именованный выход зарегистрирован
        assert_eq!(
            solutions
                .named
                .get(&("cdn".to_owned(), "origin_rps".to_owned()))
                .map(|value| value.num),
            Some(400.0),
            "origin_rps = 1000 × 0.4"
        );
        // Проливание: lb.connections_per_sec = 400 (не локальные 100)
        assert_eq!(
            solutions
                .outputs
                .get("lb")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(400.0),
            "значение lb = пролитый origin_rps"
        );
    }

    /// FR-029: `fromOutput` на ТЕКСТОВОЙ ноде — переменная Numi-листа;
    /// правка текста выше (сдвиг строк) НЕ рвёт связь (имя стабильнее
    /// индекса — мотивация именованных портов).
    #[test]
    fn from_output_text_variable_survives_line_shift() {
        let mut canvas = Canvas::default();
        // Текстовая нода: peak_rps объявлен в середине листа
        let traffic = Node::text(
            "traffic",
            "dau = 1000000\npeak_rps = 1389 rps\navg = dau / 86400 s",
            0.0,
            0.0,
        );
        canvas.nodes.push(traffic);
        template_node_with_outputs(
            &mut canvas,
            "gateway",
            &[("rps", 1.0, Some("rps"))],
            "utilization($rps, 1 req / 10 ms)",
            &[],
        );
        ported_value_edge(
            &mut canvas,
            "e1",
            "traffic",
            "gateway",
            Some("peak_rps"),
            Some("rps"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .named
                .get(&("traffic".to_owned(), "peak_rps".to_owned()))
                .map(|value| value.num),
            Some(1389.0),
            "переменная листа — именованный выход текстовой ноды"
        );
        assert_eq!(
            solutions
                .outputs
                .get("gateway")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(13.89),
            "проливание peak_rps в параметр rps: ρ = 1389/100"
        );
        // Сдвиг строк: комментарий добавлен ВЫШЕ peak_rps — индекс строки
        // изменился, имя живёт
        let shifted = canvas
            .nodes
            .iter_mut()
            .find(|node| node.id == "traffic")
            .expect("нода traffic");
        shifted.text = Some(
            "# примечание\nX = 1\ndau = 1000000\npeak_rps = 1389 rps\navg = dau / 86400 s"
                .to_owned(),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gateway")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(13.89),
            "адресация по имени живёт при сдвиге строк: ρ = 1389/100"
        );
    }

    /// FR-029: несколько value-рёбер в один `toParam` — побеждает
    /// последнее по порядку `canvas.edges` (детерминизм) + предупреждение.
    #[test]
    fn multiple_spill_edges_last_wins_with_warning() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        node_with_expr(&mut canvas, "B", "2500", 1.0);
        template_node_with_outputs(&mut canvas, "gw", &[("rps", 1.0, None)], "$rps", &[]);
        // Порядок в canvas.edges: e1 (A) раньше e2 (B) — победит B
        ported_value_edge(&mut canvas, "e1", "A", "gw", None, Some("rps"));
        ported_value_edge(&mut canvas, "e2", "B", "gw", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(2500.0),
            "последнее ребро по canvas.edges побеждает"
        );
        let warnings = solutions
            .warnings
            .get("gw")
            .expect("предупреждение записано");
        assert!(
            warnings.iter().any(|w| w.contains("rps")),
            "предупреждение называет параметр: {warnings:?}"
        );
        // Слоты $1..$N не занимаются адресованными рёбрами: позиционная
        // формула `$1` у gw не видит рёбер с toParam (их нет в слотах)
    }

    /// FR-029: несуществующий `fromOutput` / `toParam` у рукописного
    /// файла — тихая деградация: значение не приходит, локальный
    /// параметр остаётся, пересчёт не падает (симметрия FR-025).
    #[test]
    fn broken_port_addressing_degrades_silently() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        template_node_with_outputs(&mut canvas, "gw", &[("rps", 500.0, None)], "$rps", &[]);
        // fromOutput указывает на несуществующее имя; toParam — на
        // несуществующий параметр: формула должна взять локальный rps
        ported_value_edge(
            &mut canvas,
            "e1",
            "A",
            "gw",
            Some("no_such"),
            Some("also_missing"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(500.0),
            "локальный параметр остаётся при провале адресации"
        );
        // А точный toParam с провалившимся значением (источник без формулы):
        // параметр тоже локальный — ребро не «обнуляет» его
        let mut node = Node::text("prose", "просто проза", 2.0, 0.0);
        node.set_expr(None);
        canvas.nodes.push(node);
        ported_value_edge(&mut canvas, "e2", "prose", "gw", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(500.0),
            "ребро без значения не сбрасывает локальный параметр"
        );
    }

    // --- FR-017 (CP6): what-if построчные подмены ---

    /// Инвариант 1 FR-017: пустой WhatIfOverrides — результат байт-в-байт
    /// как прежний propagate без подмен (обратная совместимость FR-014).
    #[test]
    fn whatif_empty_overrides_matches_baseline() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in * 2", 1.0);
        value_edge(&mut canvas, "e", "A", "B");
        let legacy = propagate(&canvas, &HashMap::new()).expect("DAG");
        let whatif = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(legacy, whatif.outputs, "baseline идентичен");
        // Value-слой — прежняя семантика FR-014 (тест
        // overrides_flow_downstream_what_if перенесён на WhatIfOverrides)
        let mut node_values = HashMap::new();
        node_values.insert("A".to_owned(), Value::scalar(20.0));
        let whatif = WhatIfOverrides {
            line_exprs: HashMap::new(),
            node_values,
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("B")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(40.0),
            "node_values подмена течёт в downstream"
        );
    }

    /// FR-017: подмена строки заметки пересчитывает зависимые строки ниже
    /// по листу; независимые строки не тронуты (инвариант 4).
    #[test]
    fn whatif_line_override_note_recomputes_dependents() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "n",
            "rps = 1000\nlat = 50 ms\ncpu = rps * 2",
            0.0,
            0.0,
        ));
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 0), "rps = 2000".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        // Строка-подмена: lines видит новое значение (индекс един с
        // FlowSolutions.lines / from_line — тот же слот)
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 0)).map(|v| v.num),
            Some(2000.0),
            "подменённая строка пересчитана в lines"
        );
        // Зависимая строка ниже пересчиталась
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 2)).map(|v| v.num),
            Some(4000.0),
            "зависимая строка видит подменённую переменную"
        );
        // Независимая строка не тронута
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 1)).map(|v| v.num),
            Some(50.0),
            "независимая строка не изменилась"
        );
        // Итог ноды — последняя формульная строка виртуального листа
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(4000.0),
            "итог ноды = пересчитанная последняя строка"
        );
    }

    /// FR-017: подмена ПОСЛЕДНЕЙ строки заменяет итог ноды.
    #[test]
    fn whatif_line_override_last_line_becomes_node_value() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("n", "10 + 5", 0.0, 0.0));
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 0), "7 * 3".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(21.0),
            "итог ноды = подменённая последняя строка"
        );
    }

    /// FR-017: override строки-параметра шаблонной ноды мержится в
    /// ВИРТУАЛЬНУЮ param-карту (снапшот не тронут): формула пересчитана,
    /// соседний параметр — нет.
    #[test]
    fn whatif_template_param_virtual_map() {
        let mut canvas = Canvas::default();
        template_node_with_outputs(
            &mut canvas,
            "svc",
            &[("rps", 100.0, None), ("capacity", 200.0, None)],
            "$rps / $capacity",
            &[],
        );
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("svc".to_owned(), 0), "rps = 300".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("svc")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(1.5),
            "формула пересчитана с виртуальной param-картой"
        );
        // Соседний параметр не подменён
        assert_eq!(
            solutions.lines.get(&("svc".to_owned(), 1)).map(|v| v.num),
            Some(200.0),
            "capacity остался локальным"
        );
    }

    /// FR-017: каскад — подмена в A пересчитывает B и C по value-рёбрам.
    #[test]
    fn whatif_line_override_cascades_downstream() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "5", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in * 2", 1.0, 0.0));
        canvas.nodes.push(Node::text("C", "$in + 1", 2.0, 0.0));
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("A".to_owned(), 0), "20".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        let num = |id: &str| {
            solutions
                .outputs
                .get(id)
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num)
        };
        assert_eq!(num("A"), Some(20.0));
        assert_eq!(num("B"), Some(40.0), "B пересчитан");
        assert_eq!(num("C"), Some(41.0), "C пересчитан");
    }

    /// FR-017: индекс вне текста — тихая деградация (протухшая подмена
    /// не ломает пересчёт, симметрия политики FR-025).
    #[test]
    fn whatif_out_of_range_line_ignored() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "n", "5", 0.0);
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 7), "999".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(5.0),
            "протухшая подмена проигнорирована"
        );
    }
    /// FR-045 R-3 (инвариант 4): unmapped ставится — ребро есть, значения
    /// нет (источник без значения); подача значения снимает состояние.
    #[test]
    fn unmapped_inputs_set_and_unset() {
        let mut canvas = Canvas::default();
        // Источник — проза: значения не даёт → unmapped.
        node_with_expr(&mut canvas, "A", "заметка без формулы", 0.0);
        node_with_expr(&mut canvas, "B", "$in * 2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "B", &solutions);
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].edge_id, "e1");
        assert_eq!(unmapped[0].slot, Some(0));
        assert_eq!(unmapped[0].param, None);
        // Значение появилось — состояние снято (следующий пересчёт).
        node_with_expr(&mut canvas, "A", "21", 0.0);
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        assert!(unmapped_inputs(&canvas, "B", &solutions).is_empty());
    }

    /// FR-045 R-3: mixed-кейс — часть слотов пролитая, часть unmapped;
    /// слот-индекс учитывает только позиционные рёбра.
    #[test]
    fn unmapped_inputs_mixed_slots_and_params() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "P", "проза", 1.0);
        node_with_expr(&mut canvas, "T", "$1 * $2 + $Сезон", 2.0);
        value_edge(&mut canvas, "e1", "A", "T");
        value_edge(&mut canvas, "e2", "P", "T"); // позиционный слот 1 — без значения
        let mut e3 = Edge::new("e3", "A", None, "T", None);
        e3.set_flow_kind(FlowKind::Value);
        e3.to_param = Some("Сезон".to_owned());
        canvas.edges.push(e3); // проливание — значение есть
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "T", &solutions);
        assert_eq!(unmapped.len(), 1, "unmapped только у e2");
        assert_eq!(unmapped[0].slot, Some(1));
        // Проливание без значения источника → unmapped-параметр.
        let mut canvas2 = Canvas::default();
        node_with_expr(&mut canvas2, "P", "проза", 0.0);
        node_with_expr(&mut canvas2, "T", "$Сезон", 1.0);
        let mut e = Edge::new("eP", "P", None, "T", None);
        e.set_flow_kind(FlowKind::Value);
        e.to_param = Some("Сезон".to_owned());
        canvas2.edges.push(e);
        let solutions =
            propagate_with_lines(&canvas2, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas2, "T", &solutions);
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].slot, None);
        assert_eq!(unmapped[0].param.as_deref(), Some("Сезон"));
    }

    /// FR-045 R-3: контрольные рёбра и висячее ребро; control не входит
    /// в состояние, висячее (исток не существует) — unmapped.
    #[test]
    fn unmapped_inputs_control_and_dangling() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "B", "$in", 0.0);
        control_edge(&mut canvas, "ec", "ghost1", "B");
        value_edge(&mut canvas, "ev", "ghost2", "B");
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "B", &solutions);
        assert_eq!(unmapped.len(), 1, "control не в состоянии");
        assert_eq!(unmapped[0].edge_id, "ev");
    }

    /// FR-045 (инвариант 1): `canvasdesk.desc` не влияет на поток —
    /// расчёт ноды с описанием и без идентичен.
    #[test]
    fn desc_does_not_affect_flow() {
        let mut base = Canvas::default();
        node_with_expr(&mut base, "A", "3", 0.0);
        node_with_expr(&mut base, "B", "$in + 1", 1.0);
        value_edge(&mut base, "e1", "A", "B");
        let mut with_desc = base.clone();
        with_desc.nodes[1].set_desc(Some("Итоговый коэффициент".to_owned()));
        let out_base = propagate_with_lines(&base, &WhatIfOverrides::default()).expect("пересчёт");
        let out_desc =
            propagate_with_lines(&with_desc, &WhatIfOverrides::default()).expect("пересчёт");
        assert_eq!(out_base.outputs, out_desc.outputs);
        assert_eq!(out_base.lines, out_desc.lines);
    }

    /// Data-нода FR-045 R-2: `canvasdesk.data` + снапшот CSV.
    fn data_node(canvas: &mut Canvas, id: &str, label: &str, x: f32, fields: &[&str]) {
        let mut node = Node::text(id, "", x, 0.0);
        node.label = Some(label.to_owned());
        node.set_data(Some(crate::model::DataRef {
            kind: "csv".to_owned(),
            source_ref: format!("{label}.csv"),
            fields: fields.iter().map(|f| f.to_string()).collect(),
        }));
        canvas.nodes.push(node);
    }

    fn snapshot(fields: &[&str], rows: &[&[&str]]) -> crate::csv::CsvSnapshot {
        crate::csv::CsvSnapshot {
            fields: fields.iter().map(|f| f.to_string()).collect(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect(),
        }
    }

    /// FR-045 R-2 (проливание, §Q3): колонка data-ноды (`fromOutput`)
    /// проливается в слот приёмника — формула считает значение снапшота;
    /// сама data-нода значения «целиком» не даёт (адресация обязательна).
    #[test]
    fn data_column_spills_into_slot() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "K", "Курсы", 0.0, &["USD", "EUR"]);
        node_with_expr(&mut canvas, "T", "$1 * 2", 1.0);
        let mut e1 = Edge::new("e1", "K", None, "T", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("USD".to_owned());
        canvas.add_edge(e1);
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert("K".to_owned(), snapshot(&["USD", "EUR"], &[&["90", "100"]]));
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        assert_eq!(
            solutions
                .outputs
                .get("T")
                .and_then(|r| r.as_ref().ok())
                .map(|v| v.num),
            Some(180.0),
            "$1 = Курсы.USD = 90"
        );
        assert!(
            !solutions.outputs.contains_key("K"),
            "data-нода без формулы значения целиком не даёт"
        );
        // Устаревший вызов (пустая карта) — колонка не проливается (совместимость).
        let legacy = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        assert!(!legacy.outputs.contains_key("T") || legacy.outputs["T"].is_err());
    }

    /// FR-045 R-2: проливание колонки в параметр (`toParam` + `fromOutput`).
    #[test]
    fn data_column_spills_into_param() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "K", "Курсы", 0.0, &["USD"]);
        node_with_expr(&mut canvas, "T", "$Rate * $Rate", 1.0);
        let mut e1 = Edge::new("e1", "K", None, "T", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.to_param = Some("Rate".to_owned());
        e1.from_output = Some("USD".to_owned());
        canvas.add_edge(e1);
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert("K".to_owned(), snapshot(&["USD"], &[&["7"]]));
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        assert_eq!(
            solutions
                .outputs
                .get("T")
                .and_then(|r| r.as_ref().ok())
                .map(|v| v.num),
            Some(49.0)
        );
    }

    /// FR-045 (§Q3, семантика строк): `fromLine` адресует запись снапшота
    /// (0-based); без `fromLine` — первая запись; вне диапазона — unmapped.
    #[test]
    fn data_row_addressing() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "K", "Курсы", 0.0, &["USD"]);
        node_with_expr(&mut canvas, "T0", "$1", 1.0);
        node_with_expr(&mut canvas, "T1", "$1", 2.0);
        node_with_expr(&mut canvas, "T9", "$1", 3.0);
        let mut e0 = Edge::new("e0", "K", None, "T0", None);
        e0.set_flow_kind(FlowKind::Value);
        e0.from_output = Some("USD".to_owned());
        canvas.add_edge(e0);
        let mut e1 = Edge::new("e1", "K", None, "T1", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("USD".to_owned());
        e1.from_line = Some(1);
        canvas.add_edge(e1);
        let mut e9 = Edge::new("e9", "K", None, "T9", None);
        e9.set_flow_kind(FlowKind::Value);
        e9.from_output = Some("USD".to_owned());
        e9.from_line = Some(9);
        canvas.add_edge(e9);
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert(
            "K".to_owned(),
            snapshot(&["USD"], &[&["90"], &["91"], &["92"]]),
        );
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        let num = |id: &str| {
            solutions
                .outputs
                .get(id)
                .and_then(|r| r.as_ref().ok())
                .map(|v| v.num)
        };
        assert_eq!(num("T0"), Some(90.0), "без fromLine — первая запись");
        assert_eq!(num("T1"), Some(91.0), "fromLine = запись 1 (0-based)");
        assert_eq!(num("T9"), None, "запись вне диапазона — значения нет");
        // Состояние согласовано резолюции: unmapped только у e9.
        let unmapped = unmapped_inputs_with_data(&canvas, "T9", &solutions, &data);
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].edge_id, "e9");
        assert!(unmapped_inputs_with_data(&canvas, "T0", &solutions, &data).is_empty());
        assert!(unmapped_inputs_with_data(&canvas, "T1", &solutions, &data).is_empty());
    }

    /// FR-045 R-3: колонки нет в снапшоте / ячейка текстовая / пустая /
    /// ребро без адресации — unmapped; пустой снапшот — тоже.
    #[test]
    fn data_unmapped_cases() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "K", "Курсы", 0.0, &["USD", "note"]);
        node_with_expr(&mut canvas, "T", "$1", 1.0);
        // e1: колонка не из снапшота (GBP отсутствует в fields)
        let mut e1 = Edge::new("e1", "K", None, "T", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("GBP".to_owned());
        canvas.add_edge(e1);
        // e2: текстовая ячейка
        let mut e2 = Edge::new("e2", "K", None, "T", None);
        e2.set_flow_kind(FlowKind::Value);
        e2.from_output = Some("note".to_owned());
        canvas.add_edge(e2);
        // e3: без адресации — значение data-ноды целиком не определено
        let mut e3 = Edge::new("e3", "K", None, "T", None);
        e3.set_flow_kind(FlowKind::Value);
        canvas.add_edge(e3);
        // e4: пустая ячейка числовой колонки
        let mut e4 = Edge::new("e4", "K", None, "T", None);
        e4.set_flow_kind(FlowKind::Value);
        e4.from_output = Some("USD".to_owned());
        e4.from_line = Some(2);
        canvas.add_edge(e4);
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert(
            "K".to_owned(),
            snapshot(
                &["USD", "note"],
                &[&["90", "утро"], &["91", ""], &["", "ночь"]],
            ),
        );
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        let unmapped = unmapped_inputs_with_data(&canvas, "T", &solutions, &data);
        let ids: Vec<&str> = unmapped.iter().map(|u| u.edge_id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["e1", "e2", "e3", "e4"],
            "все четыре случая без значения"
        );
        // Формула не считается: слот $1 — первое позиционное ребро (e1) — None.
        assert!(!solutions.outputs.contains_key("T") || solutions.outputs["T"].is_err());
    }

    /// FR-045 (§Q4): dacdb/db — структура без снапшота (PoC) — рёбра
    /// unmapped; CSV без загруженного снапшота — источник pending.
    #[test]
    fn data_without_snapshot_is_unmapped() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "D", "Проекты", 0.0, &["name"]);
        canvas.nodes[0].set_data(Some(crate::model::DataRef {
            kind: "dacdb".to_owned(),
            source_ref: "projects".to_owned(),
            fields: vec!["name".to_owned()],
        }));
        node_with_expr(&mut canvas, "T", "$1", 1.0);
        let mut e1 = Edge::new("e1", "D", None, "T", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("name".to_owned());
        canvas.add_edge(e1);
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        assert_eq!(
            unmapped_inputs(&canvas, "T", &solutions).len(),
            1,
            "нет снапшота — значение не подставлено (R-3)"
        );
        // Снапшот появился (перезагрузка источника) — состояние снято.
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert("D".to_owned(), snapshot(&["name"], &[&["apollo"]]));
        // Текст «apollo» — не число: ячейка без значения (R-3, PoC числовых).
        let solutions =
            propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data).expect("ok");
        assert_eq!(
            unmapped_inputs_with_data(&canvas, "T", &solutions, &data).len(),
            1,
            "текстовая колонка значения не даёт"
        );
        data.insert("D".to_owned(), snapshot(&["name"], &[&["42"]]));
        let solutions =
            propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data).expect("ok");
        assert!(unmapped_inputs_with_data(&canvas, "T", &solutions, &data).is_empty());
        assert_eq!(
            solutions
                .outputs
                .get("T")
                .and_then(|r| r.as_ref().ok())
                .map(|v| v.num),
            Some(42.0)
        );
    }

    /// FR-045: резолюция колонки не зависит от топологии/формул data-ноды —
    /// снапшот решает; downstream нода (через строку-переменную) тоже видит
    /// пролив (значение идёт при обработке ПРИЁМНИКА).
    #[test]
    fn data_spill_independent_of_source_formula() {
        let mut canvas = Canvas::default();
        data_node(&mut canvas, "K", "Курсы", 0.0, &["USD"]);
        // У data-ноды есть проза и даже локальная переменная листа —
        // колонка решает по снапшоту, `named` не участвует.
        canvas.nodes[0].set_expr(Some("USD = 1".to_owned()));
        node_with_expr(&mut canvas, "T", "$1 + 1", 1.0);
        let mut e1 = Edge::new("e1", "K", None, "T", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("USD".to_owned());
        canvas.add_edge(e1);
        let mut data: DataSnapshots = DataSnapshots::new();
        data.insert("K".to_owned(), snapshot(&["USD"], &[&["10"]]));
        let solutions = propagate_with_lines_data(&canvas, &WhatIfOverrides::default(), &data)
            .expect("пересчёт");
        assert_eq!(
            solutions
                .outputs
                .get("T")
                .and_then(|r| r.as_ref().ok())
                .map(|v| v.num),
            Some(11.0),
            "снапшот (10), а не переменная листа (1)"
        );
    }

    // --- FR-050: наглядное проливание — этап A (ядро семантики) ---

    /// FR-050 Р-1 (инвариант 1): приоритет источников значения —
    /// what-if > проливание > локальный параметр. Снятие what-if
    /// возвращает проливание, удаление ребра — локальное значение.
    #[test]
    fn whatif_beats_spill_beats_local() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "src", "1389", 0.0);
        // Шаблонная нода: локальный rps = 500 (rps)
        template_node_with_outputs(
            &mut canvas,
            "gw",
            &[("rps", 500.0, Some("rps"))],
            "$rps",
            &[],
        );
        ported_value_edge(&mut canvas, "e1", "src", "gw", None, Some("rps"));
        let num = |solutions: &crate::flow::FlowSolutions| {
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num)
        };
        // Проливание: 1389 (перекрыло локальный 500)
        let base = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(num(&base), Some(1389.0), "проливание перекрывает локальный");
        // What-if подмена строки-параметра: 2000 (перекрыла проливание)
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("gw".to_owned(), 0), "rps = 2000 rps".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let active = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(num(&active), Some(2000.0), "what-if перекрывает всё");
        // Снятие what-if → снова проливание
        let base2 = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            num(&base2),
            Some(1389.0),
            "снятие what-if возвращает проливание"
        );
        // Удаление ребра → локальное значение
        canvas.edges.clear();
        let local = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            num(&local),
            Some(500.0),
            "удаление ребра возвращает локальное"
        );
    }

    /// FR-050 Н5: безразмерное пролитое значение трактуется в единицах
    /// приёмника («500» в параметр rps = 500 rps); значение с единицей в
    /// безразмерный параметр приходит как есть.
    #[test]
    fn spill_units_receiver_semantics() {
        // Скаляр 500 → параметр rps: единица приёмника прикрепляется
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "src", "500", 0.0);
        template_node_with_outputs(
            &mut canvas,
            "gw",
            &[("rps", 100.0, Some("rps"))],
            "$rps",
            &[],
        );
        ported_value_edge(&mut canvas, "e1", "src", "gw", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let value = solutions
            .outputs
            .get("gw")
            .and_then(|result| result.as_ref().ok())
            .expect("формула вычислилась");
        assert!(
            (value.num - 500.0).abs() < 1e-9,
            "число перенеслось: {value:?}"
        );
        assert!(
            value.to_string().ends_with("rps"),
            "единица приёмника прикрепилась: {value}"
        );
        // rps-значение → безразмерный параметр: приходит с единицей
        let mut canvas2 = Canvas::default();
        canvas2
            .nodes
            .push(Node::text("src", "v = 300 rps", 0.0, 0.0));
        template_node_with_outputs(&mut canvas2, "gw", &[("k", 2.0, None)], "$k", &[]);
        ported_value_edge(&mut canvas2, "e2", "src", "gw", Some("v"), Some("k"));
        let solutions2 = propagate_with_lines(&canvas2, &WhatIfOverrides::default()).expect("DAG");
        let value2 = solutions2
            .outputs
            .get("gw")
            .and_then(|result| result.as_ref().ok())
            .expect("формула вычислилась");
        assert!(
            (value2.num - 300.0).abs() < 1e-9,
            "число перенеслось: {value2:?}"
        );
        assert!(
            value2.to_string().ends_with("rps"),
            "единица значения сохранена: {value2}"
        );
    }

    /// FR-050 Р-4 (инвариант 2): авто-строка приёмника — производная
    /// value-ребра без toParam к ноде без ожидающего порта: путь
    /// «Объект.Поле» (fromOutput — имя выхода; fromLine — имя
    /// присваивания, fallback «строка N»), значение слота; повторный
    /// пересчёт — идентичные строки; удаление ребра — строка исчезла.
    #[test]
    fn auto_row_appears_for_unread_slot() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "traffic",
            "Трафик\npeak_rps = 1389 rps",
            0.0,
            0.0,
        ));
        node_with_expr(&mut canvas, "gateway", "заметка без формулы", 1.0);
        // value-ребро БЕЗ toParam; формула gateway не читает $1
        ported_value_edge(
            &mut canvas,
            "e1",
            "traffic",
            "gateway",
            Some("peak_rps"),
            None,
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let rows = auto_rows(&canvas, "gateway", &solutions);
        assert_eq!(rows.len(), 1, "ровно одна авто-строка: {rows:?}");
        assert_eq!(rows[0].node_id, "gateway");
        assert_eq!(rows[0].edge_id, "e1");
        assert_eq!(rows[0].slot, 0);
        assert_eq!(rows[0].path, "Трафик.peak_rps");
        assert_eq!(rows[0].field, "peak_rps");
        let value = rows[0].value.as_ref().expect("значение пролито");
        assert!(value.to_string().contains("1389"), "значение: {value}");
        // Детерминизм: повторный пересчёт — идентичные строки
        let solutions2 = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let rows2 = auto_rows(&canvas, "gateway", &solutions2);
        assert_eq!(rows, rows2, "повторный пересчёт идентичен");
        // Удаление ребра — строка исчезла
        canvas.edges.clear();
        let solutions3 = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert!(
            auto_rows(&canvas, "gateway", &solutions3).is_empty(),
            "ребра нет — строки нет"
        );
    }

    /// D-1 (FR-061): части авто-строки (`AutoRowParts`) — путь/число/юнит
    /// раздельно; unmapped → num «—», unit пуст; `display_text` —
    /// композиция над частями (байт-паритет строки, инвариант 2 FR-050).
    #[test]
    fn auto_row_parts_oracle() {
        let row = |value: Option<Value>| AutoRow {
            node_id: "gateway".to_owned(),
            edge_id: "e1".to_owned(),
            slot: 0,
            path: "Трафик.peak_rps".to_owned(),
            field: "peak_rps".to_owned(),
            value,
        };
        // Пролитое значение с юнитом: ячейки имя/число/юнит
        let p = row(Some(expr::unit_value(1389.0, Some("rps")))).display_parts();
        assert_eq!(p.path, "Трафик.peak_rps");
        assert_eq!(p.num, "1389");
        assert_eq!(p.unit, "rps");
        // Строка — байт-в-байт как раньше (единая точка сборки)
        assert_eq!(
            row(Some(expr::unit_value(1389.0, Some("rps")))).display_text(),
            "Трафик.peak_rps = 1389 rps"
        );
        // Скаляр — юнит пуст, строка без хвостового пробела
        let p = row(Some(Value::scalar(20.0))).display_parts();
        assert_eq!(p.num, "20");
        assert_eq!(p.unit, "");
        assert_eq!(
            row(Some(Value::scalar(20.0))).display_text(),
            "Трафик.peak_rps = 20"
        );
        // Unmapped — «—» в ячейке числа (диагностика Р-3), юнит пуст
        let p = row(None).display_parts();
        assert_eq!(p.num, "—");
        assert_eq!(p.unit, "");
        assert_eq!(row(None).display_text(), "Трафик.peak_rps = —");
    }

    /// FR-050 Р-4: «ожидающий порт» — слот читается формулой (`$in` при
    /// единственном входе, `$N`) → авто-строки нет; шаблонная нода читает
    /// `$параметры` → позиционное ребро даёт авто-строку.
    #[test]
    fn auto_row_absent_when_slot_read() {
        // $in при единственном входе — слот занят
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "7", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert!(
            auto_rows(&canvas, "B", &solutions).is_empty(),
            "$in читает единственный вход"
        );
        // $2 читается, $1 нет — авто-строка только для первого слота
        let mut canvas2 = Canvas::default();
        node_with_expr(&mut canvas2, "A", "7", 0.0);
        node_with_expr(&mut canvas2, "C", "5", 1.0);
        node_with_expr(&mut canvas2, "D", "$2 + 1", 2.0);
        value_edge(&mut canvas2, "e1", "A", "D");
        value_edge(&mut canvas2, "e2", "C", "D");
        let solutions2 = propagate_with_lines(&canvas2, &WhatIfOverrides::default()).expect("DAG");
        let rows = auto_rows(&canvas2, "D", &solutions2);
        assert_eq!(rows.len(), 1, "только неиспользуемый слот: {rows:?}");
        assert_eq!(rows[0].slot, 0, "слот 0 ($1) не читается");
        assert_eq!(rows[0].edge_id, "e1");
        // Шаблонная нода: формула читает $параметры — позиционное ребро
        // даёт авто-строку (W-UNUSED-SLOT условие)
        let mut canvas3 = Canvas::default();
        node_with_expr(&mut canvas3, "A", "7", 0.0);
        template_node_with_outputs(
            &mut canvas3,
            "gw",
            &[("rps", 100.0, Some("rps"))],
            "utilization($rps, 1 req / 10 ms)",
            &[],
        );
        value_edge(&mut canvas3, "e1", "A", "gw");
        let solutions3 = propagate_with_lines(&canvas3, &WhatIfOverrides::default()).expect("DAG");
        let rows3 = auto_rows(&canvas3, "gw", &solutions3);
        assert_eq!(rows3.len(), 1, "шаблон не читает $N: {rows3:?}");
        assert_eq!(rows3[0].field, "e1", "ребро без адресации — edge.id");
        assert_eq!(rows3[0].path, "A.e1");
    }

    /// FR-050 Р-4 (Н7/Р-6): поле авто-строки — fromLine → имя присваивания
    /// строки-источника (fallback «строка N», 1-based); unmapped-источник
    /// (проза) — строка с value = None (пунктир/тултип — этапы C/D).
    #[test]
    fn auto_row_field_names_and_unmapped() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nкол = 42\nпросто текст", 0.0, 0.0));
        node_with_expr(&mut canvas, "t", "заметка", 1.0);
        // fromLine = 1 → присваивание «кол»
        let mut e1 = Edge::new("e1", "s", None, "t", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_line = Some(1);
        canvas.add_edge(e1);
        // fromLine = 2 → проза — fallback «строка 3», значение None
        let mut e2 = Edge::new("e2", "s", None, "t", None);
        e2.set_flow_kind(FlowKind::Value);
        e2.from_line = Some(2);
        canvas.add_edge(e2);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let rows = auto_rows(&canvas, "t", &solutions);
        assert_eq!(rows.len(), 2, "оба слота не читаются: {rows:?}");
        assert_eq!(rows[0].field, "кол", "имя присваивания строки 1");
        assert_eq!(rows[0].path, "Заявки.кол");
        assert_eq!(
            rows[0].value.as_ref().map(|v| v.num),
            Some(42.0),
            "значение строки листа"
        );
        assert_eq!(rows[1].field, "строка 3", "проза — fallback 1-based");
        assert!(
            rows[1].value.is_none(),
            "источник-проза — значение не подставлено (unmapped)"
        );
    }

    // --- FR-050 Р-6 (этап B): именованный синтаксис «Объект.Поле» ---

    /// Р-6: формулы приёмника пишутся именованными путями; резолв — по графу
    /// входящих value-рёбер (fromOutput — имя выхода; итог — как у
    /// `edge_source_value`). Пример FR-050 §Решения.
    #[test]
    fn qualified_path_resolves_from_edges() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nКол = 40\nчек = 25 $", 0.0, 0.0));
        canvas.nodes.push(Node::text(
            "r",
            "выручка = Заявки.Кол · Заявки.чек",
            1.0,
            0.0,
        ));
        ported_value_edge(&mut canvas, "e1", "s", "r", Some("Кол"), None);
        ported_value_edge(&mut canvas, "e2", "s", "r", Some("чек"), None);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let value = solutions
            .outputs
            .get("r")
            .and_then(|result| result.as_ref().ok())
            .expect("формула вычислилась");
        assert!((value.num - 1000.0).abs() < 1e-9, "40 · 25 $: {value:?}");
        assert!(value.to_string().contains("$"), "единицы: {value}");
    }

    /// Инвариант 6: резолв НЕ зависит от порядка рёбер — перенумерация
    /// `canvas.edges` не меняет значений (в отличие от позиционных $N).
    #[test]
    fn qualified_path_independent_of_edge_order() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nКол = 40\nчек = 25", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r", "итог = Заявки.чек - Заявки.Кол", 1.0, 0.0));
        ported_value_edge(&mut canvas, "e1", "s", "r", Some("Кол"), None);
        ported_value_edge(&mut canvas, "e2", "s", "r", Some("чек"), None);
        let first = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // Перестановка рёбер местами (позиционные $1/$2 поменялись бы)
        canvas.edges.swap(0, 1);
        let second = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let num = |s: &crate::flow::FlowSolutions| {
            s.outputs
                .get("r")
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num)
        };
        assert_eq!(num(&first), Some(-15.0), "25 - 40");
        assert_eq!(
            num(&first),
            num(&second),
            "перенумерация canvas.edges не меняет значения"
        );
    }

    /// Инвариант 6: легаси-`$N`-формула в том же файле считается по-прежнему.
    #[test]
    fn qualified_and_legacy_coexist() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nКол = 40\nчек = 25 $", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("named", "a = Заявки.Кол · 2", 1.0, 0.0));
        canvas
            .nodes
            .push(Node::text("legacy", "b = $1 · 2", 2.0, 0.0));
        ported_value_edge(&mut canvas, "e1", "s", "named", Some("Кол"), None);
        ported_value_edge(&mut canvas, "e2", "s", "legacy", None, None);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let num = |id: &str| {
            solutions
                .outputs
                .get(id)
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num)
        };
        // «Заявки.Кол» — именованный выход (присваивание) = 40
        assert_eq!(num("named"), Some(80.0), "именованное значение · 2");
        // $1 — значение ноды целиком (итог = последняя строка листа: 25 $)
        assert_eq!(num("legacy"), Some(50.0), "легаси $1 (итог ноды 25 $) · 2");
    }

    /// Инвариант 6: коллизия имён объектов — «Имя (2).Поле» (номер по
    /// порядку `canvas.nodes`); алиас «Имя (node_id)» и плоское имя первой
    /// ноды также адресуют (зеркало dataref::qualified_obj_name).
    #[test]
    fn qualified_name_collision_forms() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("a1", "Заявки\nКол = 10", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("a2", "Заявки\nКол = 20", 1.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r2", "итог = Заявки (2).Кол", 2.0, 0.0));
        canvas
            .nodes
            .push(Node::text("rid", "итог = Заявки (a2).Кол", 3.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r1", "итог = Заявки.Кол", 4.0, 0.0));
        ported_value_edge(&mut canvas, "e1", "a2", "r2", Some("Кол"), None);
        ported_value_edge(&mut canvas, "e2", "a2", "rid", Some("Кол"), None);
        ported_value_edge(&mut canvas, "e3", "a1", "r1", Some("Кол"), None);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let num = |id: &str| {
            solutions
                .outputs
                .get(id)
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num)
        };
        assert_eq!(num("r2"), Some(20.0), "«Заявки (2)» — вторая нода");
        assert_eq!(num("rid"), Some(20.0), "«Заявки (a2)» — алиас node_id");
        assert_eq!(num("r1"), Some(10.0), "«Заявки» — первая нода");
    }

    /// Р-6: неразрешённый путь — видимая ошибка (не тихая проза): строка
    /// краснеет «вход не найден: Заявки.Нет».
    #[test]
    fn qualified_missing_reference_is_visible_error() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nКол = 40", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r", "итог = Заявки.Нет", 1.0, 0.0));
        ported_value_edge(&mut canvas, "e1", "s", "r", Some("Кол"), None);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let outcome = solutions
            .outputs
            .get("r")
            .expect("ошибка записана (не тишина прозы)");
        let err = outcome.as_ref().expect_err("пути нет — ошибка");
        assert!(
            err.to_string().contains("вход не найден: Заявки.Нет"),
            "текст ошибки: {err}"
        );
    }

    /// Р-6: поле пути fromLine-ребра — имя присваивания строки-источника
    /// (fallback «строка N»); data-семантика зеркалит edge_source_value.
    #[test]
    fn qualified_from_line_field_is_assignment_name() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nкол = 42", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r", "итог = Заявки.кол", 1.0, 0.0));
        let mut e = Edge::new("e1", "s", None, "r", None);
        e.set_flow_kind(FlowKind::Value);
        e.from_line = Some(1);
        canvas.add_edge(e);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("r")
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num),
            Some(42.0),
            "имя присваивания второй строки"
        );
    }

    /// Р-6: именованный путь читает слот своего ребра — W-UNUSED-SLOT не
    /// срабатывает, авто-строка не появляется (значение не «теряется»).
    /// Тебро без адресации в путь имени — поле edge.id.
    #[test]
    fn qualified_consumes_slot_no_warning_no_auto_row() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("s", "Заявки\nКол = 40", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("r", "итог = Заявки.Кол + 1", 1.0, 0.0));
        ported_value_edge(&mut canvas, "e1", "s", "r", Some("Кол"), None);
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert!(
            auto_rows(&canvas, "r", &solutions).is_empty(),
            "слот читается именованным путём — авто-строки нет"
        );
        assert_eq!(
            crate::validate::validate(&canvas),
            Vec::new(),
            "W-UNUSED-SLOT не срабатывает: слот занят путём"
        );
        // Контраст: ребро БЕЗ адресации — поле edge.id («Заявки.e2»)
        ported_value_edge(&mut canvas, "e2", "s", "r", None, None);
        canvas
            .nodes
            .push(Node::text("r2", "итог = Заявки.e2 + 1", 2.0, 0.0));
        canvas.edges.last_mut().unwrap().to_node = "r2".to_owned();
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("r2")
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num),
            Some(41.0),
            "безымянный выход — edge.id (итог ноды 40 + 1)"
        );
    }

    /// Р-6: пути доступны формулам шаблона и what-if-подменам (RHS
    /// считается в окружении каскада Р-1); toParam-проливание тоже
    /// адресуемо по имени.
    #[test]
    fn qualified_in_template_formula_and_whatif_rhs() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "traffic",
            "Трафик\npeak_rps = 200 rps",
            0.0,
            0.0,
        ));
        template_node_with_outputs(
            &mut canvas,
            "gw",
            &[("k", 8.0, None)],
            "Трафик.peak_rps / $k",
            &[],
        );
        // Позиционное value-ребро: значение адресуемо путём «Трафик.peak_rps»
        ported_value_edge(&mut canvas, "e1", "traffic", "gw", Some("peak_rps"), None);
        let num = |solutions: &crate::flow::FlowSolutions| {
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|v| v.num)
        };
        let base = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(num(&base), Some(25.0), "200 rps / локальный k=8");
        // What-if подмена k: RHS видит и параметры, и qualified-пути
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("gw".to_owned(), 0), "k = Трафик.peak_rps / 100".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let active = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            num(&active),
            Some(100.0),
            "what-if k = 200/100 = 2; итог 200/2"
        );
    }

    /// FR-050 Н9-1 (этап E): порядок волны = топологическое расстояние —
    /// цепочка A→B→C от seed A: ребро A→B порядок 0, B→C порядок 1;
    /// выход отсортирован по индексу ребра (детерминизм).
    #[test]
    fn spill_wave_orders_by_topological_distance() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 2", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        let seeds: HashSet<String> = ["A".to_owned()].into_iter().collect();
        let wave = spill_wave(&canvas, &seeds);
        assert_eq!(wave, vec![(0, 0), (1, 1)], "порядок: A→B = 0, B→C = 1");
    }

    /// Н9-1: control-рёбра значения не несут — в волну не входят; seed без
    /// исходящих value-рёбер и неизвестный seed — пустая волна.
    #[test]
    fn spill_wave_skips_control_and_unknown() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        control_edge(&mut canvas, "e1", "A", "B");
        let seeds: HashSet<String> = ["A".to_owned()].into_iter().collect();
        assert!(spill_wave(&canvas, &seeds).is_empty(), "control не в волне");
        // Неизвестный seed (нода удалена) — не паника, волна пуста
        let ghosts: HashSet<String> = ["ghost".to_owned()].into_iter().collect();
        assert!(spill_wave(&canvas, &ghosts).is_empty());
        // Seed без исходящих рёбер — менять нечего
        let leaf: HashSet<String> = ["B".to_owned()].into_iter().collect();
        assert!(spill_wave(&canvas, &leaf).is_empty());
    }

    /// Н9-1: ромб A→B→D, A→C→D — приёмник D достижим по двум путям,
    /// оба ребра второго уровня получают порядок 1 (расстояние истока);
    /// BFS с посещёнными не зацикливается на value-цикле чужого файла.
    #[test]
    fn spill_wave_diamond_and_cycle_safety() {
        let mut canvas = Canvas::default();
        for (id, x) in [("A", 0.0), ("B", 1.0), ("C", 1.0), ("D", 2.0)] {
            node_with_expr(&mut canvas, id, "1", x);
        }
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "A", "C");
        value_edge(&mut canvas, "e3", "B", "D");
        value_edge(&mut canvas, "e4", "C", "D");
        let seeds: HashSet<String> = ["A".to_owned()].into_iter().collect();
        let wave = spill_wave(&canvas, &seeds);
        assert_eq!(wave, vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
        // Value-цикл (чужой .canvas): обход конечен, рёбра в волне, дублей нет
        let mut cyclic = Canvas::default();
        node_with_expr(&mut cyclic, "A", "1", 0.0);
        node_with_expr(&mut cyclic, "B", "$in + 1", 1.0);
        value_edge(&mut cyclic, "e1", "A", "B");
        value_edge(&mut cyclic, "e2", "B", "A");
        let seeds: HashSet<String> = ["A".to_owned()].into_iter().collect();
        let wave = spill_wave(&cyclic, &seeds);
        assert_eq!(wave, vec![(0, 0), (1, 1)], "цикл обошёлся без дублей");
    }

    /// Н9-1: приёмник-«изменённая» тоже seed — волна продолжается от неё
    /// вниз (каскад: изменение итога ноды B подсвечивает и её исходящие).
    #[test]
    fn spill_wave_cascade_from_changed_receiver() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 2", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        // Изменился только B (например, правка её формулы): A не seed
        let seeds: HashSet<String> = ["B".to_owned()].into_iter().collect();
        let wave = spill_wave(&canvas, &seeds);
        assert_eq!(wave, vec![(1, 0)], "только ребро B→C, порядок 0");
    }
}
