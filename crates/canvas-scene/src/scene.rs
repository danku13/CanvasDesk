//! SceneState — модельное состояние сцены (FR-037/ADR-0012, перенос из
//! canvas-app/main.rs): канвас, spatial index, файл, история undo, кэши
//! результатов формул и анализа узких мест, viewport-зеркало MCP.
//! UI-состояние взаимодействия (выделение, drag) живёт в App (canvas-app).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use canvas_core::analyze::AnalysisConfig;
use canvas_core::expr::{self, Env as ExprEnv, ExprLineResults, ExprOutcome, ExprResults};
use canvas_core::flow::{self, FlowKind, FlowOutputs};
use canvas_core::time::Instant;
use canvas_core::{
    analyze, AnalysisState, Canvas, CanvasStorage, EdgeBundleIndex, FsCanvasStorage, Node,
    Scenario, SpatialIndex, StaleOverride,
};

use crate::measure::{ensure_result_reserve, formula_line_indices};
use crate::view::{SpillView, WhatIfNode};

/// FR-064 P1 (документ §5.2, вариант P1): двойная буферизация решений
/// потока — снимок [`flow::FlowSolutions`] за `Arc<RwLock>`. Писатель —
/// конвейер пересчёта (публикация снимка атомарна с выводкой O(N) —
/// читатель не видит рассинхрона), читатель — рендер/UI/MCP через
/// `RwLock::read()`. Без `arc_swap` (архдок §5.2 «без новых зависимостей»;
/// вариант v2 — по бенчмарку). На wasm ведёт себя как обычная обёртка
/// (sync-путь — единственный писатель).
pub type FlowBuffer = Arc<RwLock<flow::FlowSolutions>>;

/// Debounce автосейва (SPEC §9).
pub const AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(2);

/// FR-064 P1: бюджет ожидания воркера — если пара снимков не готова за
/// это время (зависание вычисления), пересчёт падает на sync-фолбэк +
/// `warn` (правило AGENTS «фолбэк + warn», live-инвариант не нарушается).
#[cfg(not(target_arch = "wasm32"))]
const FLOW_WORKER_TIMEOUT: Duration = Duration::from_secs(3);
/// Глубина истории undo (FR-006): не менее 50 последних действий (запрос
/// пользователя «не менее 50»); старейшие шаги вытесняются.
pub const UNDO_LIMIT: usize = 50;

/// Минимальный зум viewport (синхронно с canvas-render::camera::MIN_ZOOM,
/// SPEC §8; паритет — тестом в canvas-app).
pub const MIN_ZOOM: f32 = 0.05;
/// Максимальный зум viewport (синхронно с canvas-render::camera::MAX_ZOOM,
/// SPEC §8; паритет — тестом в canvas-app).
pub const MAX_ZOOM: f32 = 4.0;

/// ADR-0012 (Q2, viewport-зеркало): позиция/зум камеры в
/// платформенно-нейтральных числах — MCP-инструменты viewport_get/set
/// работают с этим значением; синхронизация с `canvas-render::Camera` —
/// в `on_mcp_wake` (canvas-app), числа идентичны (0/0/1 — дефолт Camera),
/// поведение viewport_get/set не меняется.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Мировая точка в центре viewport (Camera::position()[0]).
    pub x: f32,
    /// Мировая точка в центре viewport (Camera::position()[1]).
    pub y: f32,
    /// Абсолютный зум (Camera::zoom()).
    pub zoom: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

/// FR-013: извлечь формулу из текста заметки (смешанный редактор — решение
/// открытого вопроса дизайна): строки, начинающиеся с `=` (после пропуска
/// пробелов), — утверждения формулы без префикса. Текст ноды остаётся
/// пользовательским описанием вместе с `=`-строками; результат живёт в
/// `canvasdesk.expr`. None — формульных строк нет (calc-режим выключен).
pub fn split_formula_lines(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let formula = trimmed.strip_prefix('=')?.trim();
            (!formula.is_empty()).then_some(formula)
        })
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// FR-014: карта результатов propagator'а → display-карта приложения
/// (`expr_results`): типизированные ошибки становятся строками для рендера.
pub fn outputs_to_results(outputs: &FlowOutputs) -> ExprResults {
    outputs
        .iter()
        .map(|(id, result)| {
            let outcome = match result {
                Ok(value) => ExprOutcome::Ok(value.clone()),
                Err(err) => ExprOutcome::Err(err.to_string()),
            };
            (id.clone(), outcome)
        })
        .collect()
}

/// FR-029: текст ноды как на карточке — с подменой строк-присваиваний
/// пролитых параметров на подписи источников («param ← нода · выход»).
/// Единая точка для рендера (SceneView) и подгонки высоты.
pub fn display_body_text(node: &Node, param_spills: &HashMap<String, Vec<SpillView>>) -> String {
    let text = node.text.as_deref().unwrap_or("");
    match param_spills.get(&node.id) {
        Some(spills) if !spills.is_empty() => {
            flow::substitute_spilled_lines(text, spills.iter().map(SpillView::as_triple))
        }
        _ => text.to_owned(),
    }
}

/// FR-029: индекс строки-присваивания `param = …` в тексте ноды
/// (имя до '=' — одно слово). None — такой строки нет.
pub fn assignment_line(node: &Node, param: &str) -> Option<usize> {
    node.text
        .as_deref()
        .unwrap_or_default()
        .split('\n')
        .position(|line| {
            line.split_once('=')
                .map(|(name, _)| name.trim() == param)
                .unwrap_or(false)
        })
}

/// FR-029: значение, которое value-ребро с toParam приносит в параметр —
/// по адресации истока (зеркало `edge_source_value` flow): fromLine →
/// построчный выход источника, fromOutput → именованный выход, иначе
/// узловое значение источника. None — источник без значения (тихая
/// деградация: бейдж строки тогда по локальному результату).
pub fn spill_edge_value(
    solutions: &flow::FlowSolutions,
    spill: &canvas_core::ParamSpill,
) -> Option<canvas_core::Value> {
    if let Some(line) = spill.from_line {
        solutions
            .lines
            .get(&(spill.from_node.clone(), line))
            .cloned()
    } else if let Some(name) = &spill.from_output {
        solutions
            .named
            .get(&(spill.from_node.clone(), name.clone()))
            .cloned()
    } else {
        solutions
            .outputs
            .get(&spill.from_node)
            .and_then(|result| result.as_ref().ok().cloned())
    }
}

/// FR-017 (CP6): строка дельты «(+Δ)» между базовым и what-if значением.
/// PRD-0007 X3: реализация перенесена в ядро ([`canvas_core::expr`]) —
/// формат нужен и `lineage_deltas` explain-дерева; здесь делегация
/// (публичный API сцены сохранён — mcp.rs/lib.rs/whatif_ui).
pub fn whatif_delta_str(base: &expr::Value, whatif: &expr::Value) -> Option<String> {
    expr::whatif_delta_str(base, whatif)
}

/// FR-017: полный формат дельта-бейджа «было → стало (+Δ)» (гипотеза Q4) —
/// то, что рендер показывает вместо голого значения изменившейся строки/
/// итога. Без изменений — None (бейдж остаётся обычным).
pub fn whatif_full_delta(base: &expr::Value, whatif: &expr::Value) -> Option<String> {
    expr::whatif_full_delta(base, whatif)
}

/// Первый свободный id вида `{prefix}-N` (T9): N от 1, занятые в канвасе
/// пропускаются. Обобщение генератора id заметок на `file-N`/`note-N`
/// (вызовы с "note" — заметки, с "file" — ноды дропа). Реэкспортируется
/// `canvas_app::ui` для GUI-путей (единый источник после FR-037).
pub fn next_free_id(canvas: &Canvas, prefix: &str) -> String {
    let mut n = 1u32;
    while canvas
        .nodes
        .iter()
        .any(|node| node.id == format!("{prefix}-{n}"))
    {
        n += 1;
    }
    format!("{prefix}-{n}")
}

/// Стартовый канвас при отсутствии файла: заметка + файловые ноды (T4).
pub fn seed_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "Добро пожаловать в CanvasDesk", 80.0, 60.0);
    note.width = 280.0;
    note.color = Some("4".into());
    canvas.nodes.push(note);
    canvas.nodes.push(Node::file(
        "file-1",
        "docs/SPEC.md",
        440.0,
        60.0,
        320.0,
        220.0,
    ));
    canvas.nodes.push(Node::file(
        "file-2",
        "docs/TASKS.md",
        440.0,
        340.0,
        320.0,
        220.0,
    ));
    canvas
}

/// Состояние сцены: модель, spatial index (T5), файл, кэши расчётов и
/// viewport-зеркало MCP. Выделение и перетаскивание (UI-ввод) — в App.
pub struct SceneState {
    pub canvas: Canvas,
    /// R-tree над AABB нод; синхронизируется при каждом изменении геометрии.
    pub spatial: SpatialIndex,
    pub path: PathBuf,
    pub dirty_since: Option<Instant>,
    /// История undo (FR-006): снапшоты Canvas «до» действий (push ДО
    /// мутации). VecDeque — O(1) вытеснение старейшего при переполнении.
    pub undo_stack: VecDeque<Canvas>,
    /// Отменённые состояния (FR-006): текущее уходит сюда при undo; новое
    /// действие обнуляет ветку redo.
    pub redo_stack: Vec<Canvas>,
    /// FR-013: результаты формул (`canvasdesk.expr`) по id нод —
    /// runtime-кэш (инвариант 4 FR-013: НЕ сериализуется, источник
    /// истины — формула; пересчитывается при загрузке/commit/undo).
    /// Программный итог (MCP-expr) — в футере карточки.
    pub expr_results: ExprResults,
    /// FR-013 (правка 2): построчные результаты текста нод (Numi-стиль) —
    /// тоже runtime-кэш (инвариант 4).
    pub expr_line_results: ExprLineResults,
    /// FR-016 (CP5): флаги анализа узких мест по id нод — runtime-кэш,
    /// обновляется хвостом [`SceneState::recompute_flow`] (analyze — чистая
    /// функция над теми же FlowSolutions; O(N), в бюджете кадра).
    pub analysis: AnalysisState,
    /// FR-029 (визуализация проливания): параметры нод, запитанные
    /// value-рёбрами с `toParam` — подпись источника и эффективное значение
    /// строки для рендера. Runtime-кэш (не сериализуется), пересчитывается
    /// в `recompute_flow` вместе с результатами потока.
    pub param_spills: HashMap<String, Vec<SpillView>>,
    /// FR-061 этап D (D-8): описания манифестов шаблонов (id → описание)
    /// — источник зоны описания шаблонных нод (Q3). Runtime-снимок
    /// реестра (не сериализуется), устанавливается приложением.
    pub template_descs: HashMap<String, String>,
    /// FR-050 Р-4 (Н10-а — строка-проекция): авто-строки приёмников —
    /// производные строки тела нод для value-рёбер без `toParam` к нодам
    /// без ожидающего порта (слот не читается формулой — W-UNUSED-SLOT).
    /// Runtime-кэш (не сериализуется, инвариант «формат .canvas не
    /// расширяется»), пересчитывается в `recompute_flow` — значения из
    /// активного сценария what-if. Рендер авто-строки — этап D FR-050.
    pub auto_rows: HashMap<String, Vec<flow::AutoRow>>,
    /// FR-050 Р-3 (этап C): id value-рёбер в состоянии «не подставлено»
    /// (unmapped, FR-045 R-3) — пунктир янтарным акцентом анализа + тултип
    /// «проблема + решение». Runtime-кэш (не сериализуется, тот же источник
    /// истины, что у `auto_rows`: готовые результаты пересчёта),
    /// пересчитывается в `recompute_flow`; порядок детерминирован
    /// (`canvas.nodes` × `canvas.edges`).
    pub unmapped_edges: Vec<String>,
    /// FR-042 (E2): индекс пучков рёбер — группировка по упорядоченной паре
    /// концов для LOD-0 агрегации и main stage. Runtime-кэш (не
    /// сериализуется, инвариант «формат .canvas не расширяется»);
    /// перестраивается в хвосте [`SceneState::recompute_flow`] — единой
    /// точке синхронизации мутаций (O(edges) поверх пересчёта потока).
    pub bundles: EdgeBundleIndex,
    /// FR-017 (CP6): what-if режим активен (нижний бар, override-поле
    /// вместо правки базы). Runtime-флаг — в `.canvas` не пишется.
    pub whatif_active: bool,
    /// FR-017: именованные сценарии — runtime-копия persisted
    /// `canvasdesk.whatif` (синхронизируется при открытии/Apply/правках
    /// списка сценариев одним undo-шагом).
    pub scenarios: Vec<Scenario>,
    /// FR-017: активный сценарий; `None` — «База» (overrides пусты,
    /// дельты нулевые).
    pub active_scenario: Option<usize>,
    /// FR-017: базовый пересчёт БЕЗ подмен — источник дельт (гипотеза Q9:
    /// чистый пересчёт на каждом ревале, не снапшот при входе). FR-064 P1:
    /// double buffer [`FlowBuffer`] — снимок за Arc<RwLock>, читатель
    /// (рендер/UI/MCP) берёт его через `read()`.
    pub flow_baseline: FlowBuffer,
    /// FR-017: пересчёт с подменами активного сценария — видимый канвасом.
    /// FR-064 P1: double buffer (см. [`SceneState::flow_baseline`]).
    pub flow_active: FlowBuffer,
    /// PRD-0007 (AC-2.4): ошибка цикла последнего пересчёта — explain-дерево
    /// строится в режиме [`canvas_core::LineageFlow::Cycled`] (топология без
    /// значений). None — пересчёт прошёл. Runtime-поле, не сериализуется.
    pub flow_cycle: Option<flow::CycleError>,
    /// FR-017: протухшие подмены активного сценария (нода/строка удалены,
    /// строка стала прозой) — маркеры в панели (гипотеза Q5c).
    pub whatif_stale: Vec<StaleOverride>,
    /// FR-017: what-if представления нод для рендера (виртуальный текст,
    /// подсветка подмен, дельта-бейджи). Runtime-кэш — пересчитывается в
    /// `recompute_flow` вместе с картами потока.
    pub whatif_nodes: HashMap<String, WhatIfNode>,
    /// FR-064 P2 (FR-017 v2): замороженные снимки сценариев — pinned
    /// значения потока на момент заморозки (правки канваса их не двигают).
    /// Runtime-кэш (не сериализуется); имена — в `canvasdesk.whatif.frozen`,
    /// восстанавливаются при загрузке ([`SceneState::restore_frozen`]).
    pub frozen: Vec<canvas_core::whatif::FrozenScenario>,
    /// FR-061 хвосты (D-7 runtime v1, Q4 — runtime): id нод со СВЁРНУТЫМ
    /// блоком-ведомостью (дефолт — развёрнут; тоггл — клик по заголовку).
    /// Очищается при загрузке схемы (сброс к дефолту), в .canvas не пишется.
    pub block_collapsed: std::collections::HashSet<String>,
    /// FR-061 хвосты (D-8 runtime v1, «Раскрыть+авто»): id нод с РАСКРЫТЫМ
    /// описанием («⋯ целиком ▾»). Автосворачивание — клик вне ноды /
    /// начало правки (метод collapse_descs_except); очистка при загрузке.
    pub desc_expanded: std::collections::HashSet<String>,
    /// ADR-0012: viewport-зеркало MCP (см. [`Viewport`]).
    pub viewport: Viewport,
    /// PRD-0007 (FR-048 X2, AC-3.3): монотонный счётчик ревизий модели —
    /// увеличивается в [`SceneState::recompute_flow`] (единая точка
    /// синхронизации мутаций). Explain-оверлей запоминает ревизию снапшота
    /// и сравнением показывает чип «Данные изменены»: правки канваса, MCP,
    /// перезапись файла, подмена листа, Apply — все проходят через
    /// пересчёт. В `.canvas` не пишется (runtime).
    pub revision: u64,
    /// FR-050 Н9-1 (этап E): ноды с изменившимся итогом последнего
    /// пересчёта (сравнение с прошлым `expr_results`) — seeds волны
    /// подсветки каскада ([`canvas_core::flow::spill_wave`]): изменил
    /// upstream → волна видимо бежит вниз по value-рёдрам. Первый
    /// пересчёт (загрузка файла) — список пуст (сравнивать не с чем).
    /// Runtime-поле, не сериализуется; порядок — сортировка по id
    /// (детерминизм).
    pub flow_changed_nodes: Vec<String>,
    /// Был ли хотя бы один пересчёт до текущего (первый — без волны).
    flow_computed: bool,
    /// Динамический перерасчёт `MeasuredReserveFn` (T9-сессия 2026-09-24):
    /// отпечаток контента ноды (text, formula_lines, desc, auto_rows,
    /// sigma, footer_reserve) И высота на момент замера — для хеш-гейта
    /// `refit_node_to_content`. Пара (hash, height): когда хеш меняется
    /// → контент факт-изменился → запускается реальный замер с
    /// усадкой/ростом. Когда хеш прежний → no-op (замер не запускается,
    /// перф). Когда текущая высота не совпадает с сохранённой — канвас
    /// заменён внешне (undo/redo/прямая замена) → доверяем текущей высоте,
    /// обновляем хеш без замера (самовосстановление). Mode-тогглы
    /// (block_collapsed/desc_expanded) в хеш НЕ входят — они остаются
    /// на growth-only `ensure_reserve_at` (I-6). Записи удалённых нод
    /// не чистятся (мелкий memory-leak, безопасно — ключ стабильный id).
    /// Не сериализуется.
    content_height_state: std::collections::HashMap<String, (u64, f32)>,
    /// M8/W3 (wasm-port §3.2/§6): хранилище `.canvas` как сервис — нативно
    /// `FsCanvasStorage` (диск + `.bak`, сегодняшнее поведение), web (W6) —
    /// FS Access/OPFS через `with_storage`.
    pub storage: Arc<dyn CanvasStorage>,
    /// PRD-0007 (FR-048 X4, AC-5.3): теги undo-записей — параллельный стек
    /// к [`SceneState::undo_stack`]. Тег ставится вызывающим ПЕРЕД
    /// `push_undo` (снапшот «до»); верхний тег читается ПЕРЕД `take_undo`
    /// ([`SceneState::peek_undo_tag`]) — так приложение узнаёт, что следующий
    /// undo откатит пачку автосвязи и обязан спросить подтверждение
    /// с подсветкой отменяемого. Runtime-поле, не сериализуется.
    undo_tags: VecDeque<Option<&'static str>>,
    /// Отложенный тег для СЛЕДУЮЩЕГО `push_undo` (см. [`SceneState::undo_tags`]).
    pending_undo_tag: Option<&'static str>,
    /// FR-064 P1: хэндл сценарного воркера (desktop-only, подключает
    /// приложение в main()). None — sync-режим: wasm/тесты/headless —
    /// тяжёлый пересчёт выполняется на вызывающем треде (штатный путь,
    /// контракт плана §5.8).
    #[cfg(not(target_arch = "wasm32"))]
    flow_worker: Option<crate::worker::FlowWorkerHandle>,
    /// FR-064 P1: пересчёт в полёте (запрос отправлен воркеру, ждём пару
    /// снимков; выводка O(N) выполнится в
    /// [`SceneState::complete_flow_recompute`]).
    #[cfg(not(target_arch = "wasm32"))]
    flow_pending: Option<FlowPending>,
    /// FR-064 P1: счётчик поколений запросов воркеру — ответы устаревших
    /// поколений (правки чаще, чем воркер успевает) отбрасываются.
    #[cfg(not(target_arch = "wasm32"))]
    flow_generation: u64,
}

/// FR-064 P1: слот pending-запроса — исход одного прогона воркера.
#[cfg(not(target_arch = "wasm32"))]
enum FlowSlot {
    /// Готовый исход: снимок или цикл value-рёбер.
    Done(Result<flow::FlowSolutions, flow::CycleError>),
    /// Паника вычисления воркером — sync-фолбэк + warn.
    Panicked,
    /// Активное решение зеркалит базу (подмен нет — отдельный прогон не
    /// заказывался).
    MirrorsBaseline,
}

/// FR-064 P1: пересчёт в полёте — контекст, необходимый хвосту выводки
/// O(N) (см. [`SceneState::apply_flow_pair`]) после получения пары снимков
/// от воркера. Поколение отбрасывает ответы устаревших запросов (правки
/// чаще, чем воркер успевает: новое поколение подменяет старое).
#[cfg(not(target_arch = "wasm32"))]
struct FlowPending {
    generation: u64,
    /// Подмены активного сценария на момент запроса (для виртуального
    /// текста построчных результатов и whatif-представлений нод).
    whatif: flow::WhatIfOverrides,
    /// Протухшие подмены (маркеры панели, Q5c).
    stale: Vec<StaleOverride>,
    /// Снапшоты «до» для волны каскада (FR-050 Н9-1).
    prev_results: ExprResults,
    prev_solutions: flow::FlowSolutions,
    /// Исходы слотов (None — ещё считается воркером).
    baseline: Option<FlowSlot>,
    active: Option<FlowSlot>,
    /// Бюджет ожидания воркера (таймаут → sync-фолбэк + warn).
    deadline: Instant,
}

/// FR-064 P1: чтение снимка double buffer (рендер/UI/MCP). Ядовитый замок
/// (паника под write-гардом) — восстановление через into_inner: писатель
/// один и паник под гардом не оставляет частичной публикации, в худшем
/// случае читатель увидит прежний консистентный снимок. Без unwrap/expect
/// (правило AGENTS).
pub fn read_flow(buf: &FlowBuffer) -> std::sync::RwLockReadGuard<'_, flow::FlowSolutions> {
    buf.read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// FR-064 P1: публикация снимка double buffer (см. [`read_flow`]).
fn write_flow(buf: &FlowBuffer) -> std::sync::RwLockWriteGuard<'_, flow::FlowSolutions> {
    buf.write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl SceneState {
    /// Обернуть готовую модель: построить spatial index. Хранилище —
    /// файловое по умолчанию (нативное поведение, диск + `.bak`).
    pub fn new(canvas: Canvas, path: PathBuf) -> Self {
        Self::with_storage(canvas, path, Arc::new(FsCanvasStorage))
    }

    /// Обернуть готовую модель с явным хранилищем (M8/W3): web-бинарь
    /// подставит FS Access/OPFS (W6), тесты — `MemStorage`.
    pub fn with_storage(canvas: Canvas, path: PathBuf, storage: Arc<dyn CanvasStorage>) -> Self {
        let spatial = SpatialIndex::build(&canvas);
        // FR-042: первичный индекс пучков (до переезда canvas в структуру)
        let bundles = EdgeBundleIndex::build(&canvas);
        // FR-017: сценарии what-if — загрузка persisted `canvasdesk.whatif`
        let scenarios = canvas_core::whatif::scenarios_from_canvas(&canvas);
        let mut scene = Self {
            canvas,
            spatial,
            path,
            dirty_since: None,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            expr_results: ExprResults::new(),
            expr_line_results: ExprLineResults::new(),
            param_spills: HashMap::new(),
            template_descs: HashMap::new(),
            auto_rows: HashMap::new(),
            unmapped_edges: Vec::new(),
            bundles,
            whatif_active: false,
            scenarios,
            active_scenario: None,
            flow_baseline: Arc::new(RwLock::new(flow::FlowSolutions::default())),
            flow_active: Arc::new(RwLock::new(flow::FlowSolutions::default())),
            flow_cycle: None,
            whatif_stale: Vec::new(),
            whatif_nodes: HashMap::new(),
            frozen: Vec::new(),
            block_collapsed: std::collections::HashSet::new(),
            desc_expanded: std::collections::HashSet::new(),
            analysis: AnalysisState::new(),
            viewport: Viewport::default(),
            revision: 0,
            flow_changed_nodes: Vec::new(),
            flow_computed: false,
            content_height_state: std::collections::HashMap::new(),
            storage,
            undo_tags: VecDeque::new(),
            pending_undo_tag: None,
            #[cfg(not(target_arch = "wasm32"))]
            flow_worker: None,
            #[cfg(not(target_arch = "wasm32"))]
            flow_pending: None,
            #[cfg(not(target_arch = "wasm32"))]
            flow_generation: 0,
        };
        // FR-013: первичный пересчёт формул при загрузке (результат не
        // хранится в .canvas — вычисляется, см. инвариант 4 FR-013);
        // FR-014: пересчёт — живой propagator графа потока
        scene.recompute_flow();
        // FR-064 P2: восстановление freeze-снимков по persisted именам
        // (пересчёт каждого сценария против персистентного канваса).
        let frozen_names = canvas_core::whatif::frozen_from_canvas(&scene.canvas);
        if !frozen_names.is_empty() {
            scene.restore_frozen(&frozen_names);
        }
        scene
    }

    /// FR-014: живой пересчёт графа потока значений (инвариант live — в
    /// пределах 1–2 кадров, FR-064). Заполняет `expr_results` (результаты
    /// формул всех expr-нод, с входами value-рёбер) и `expr_line_results`
    /// (построчные результаты Numi-листов — строки видят входы ноды).
    /// Запускается после ЛЮБОЙ мутации формул или топологии (правка
    /// текста/формулы, рёбра, удаление нод, undo) — propagator чистый,
    /// полный пересчёт ≤1000 нод <10 мс (SPEC §6.3).
    ///
    /// FR-064 P1: при подключённом воркере тяжёлые прогоны
    /// [`flow::propagate_with_lines`] (baseline + active) уезжают на
    /// воркер-тред, метод возвращается сразу; выводка O(N)
    /// (`expr_results`/`analysis`/`bundles`/diff волны) выполняется в
    /// [`SceneState::complete_flow_recompute`] по готовности пары снимков.
    /// Метод остаётся ЕДИНСТВЕННЫМ редактором производных кэшей (план §5.4).
    /// Без воркера (wasm/тесты/headless/фолбэк) — синхронный путь как
    /// раньше.
    pub fn recompute_flow(&mut self) {
        // PRD-0007 (AC-3.3): любая мутация модели — новая ревизия (обе ветки
        // выхода: цикл и штатная); кэш explain-оверлея сравнивает её со
        // своей, чтобы показать чип «Данные изменены».
        self.revision = self.revision.wrapping_add(1);
        // FR-050 Н9-1 (этап E): снапшоты ДО пересчёта — детект изменений
        // для волны каскада (первый пересчёт — без волны); снимаются до
        // любых перезаписей. Итоги нод (`expr_results`) покрывают узловые
        // результаты; `flow_active` — строки-значения и именованные выходы
        // ИСТОКОВ-листов (присваивающий Numi-лист без узлового итога тоже
        // даёт волну вниз — демо-критерий FR-050: «изменил DAU → видно
        // распространение»).
        let prev_results = self.expr_results.clone();
        let prev_solutions = read_flow(&self.flow_active).clone();
        // FR-017: активный сценарий → построчные подмены (протухшие
        // отфильтрованы — тихая деградация, маркеры в whatif_stale).
        let (whatif, stale) = self.active_whatif_overrides();
        // FR-064 P1: воркер подключён — запрос тяжёлых прогонов ему,
        // лёгкий выход (производные кэши обновятся по готовности снимков).
        // Клоны контекста — только при живом воркере (sync-путь без воркера
        // забирает значения по значению, лишних копий нет).
        #[cfg(not(target_arch = "wasm32"))]
        if self.flow_worker.is_some()
            && self.request_flow_recompute(
                &whatif,
                stale.clone(),
                prev_results.clone(),
                prev_solutions.clone(),
            )
        {
            return;
        }
        // FR-017 (гипотеза Q9): дельты — против ЧИСТОГО базового пересчёта
        // (не снапшота при входе): любая мутация канваса пересчитывает обе
        // карты заново, дельты консистентны текущему `.canvas`.
        let baseline =
            match flow::propagate_with_lines(&self.canvas, &flow::WhatIfOverrides::default()) {
                Ok(solutions) => solutions,
                Err(cycle) => {
                    self.apply_flow_cycle(cycle);
                    return;
                }
            };
        let active = self.compute_active(&whatif, &baseline);
        self.apply_flow_pair(
            baseline,
            active,
            whatif,
            stale,
            prev_results,
            prev_solutions,
        );
    }

    /// FR-064 P1: деградация при цикле value-рёбер — общая для sync-пути и
    /// для ветки воркера (база не считается): изолированный расчёт (FR-013)
    /// с предупреждением (правило AGENTS: фолбэк + warn).
    fn apply_flow_cycle(&mut self, cycle: flow::CycleError) {
        // UI и MCP блокируют создание value-циклов; сюда попадаем
        // только из чужих .canvas-файлов — деградация до изолированного
        // расчёта (FR-013) с предупреждением (правило AGENTS: фолбэк + warn)
        tracing::warn!(cycle = %cycle, "цикл value-рёбер — расчёт без потока");
        *write_flow(&self.flow_baseline) = flow::FlowSolutions::default();
        *write_flow(&self.flow_active) = flow::FlowSolutions::default();
        self.flow_cycle = Some(cycle);
        self.whatif_stale = Vec::new();
        self.whatif_nodes.clear();
        self.recompute_all_expr();
        self.param_spills.clear();
        self.auto_rows.clear();
        self.unmapped_edges.clear();
        // FR-050 Н9-1 (этап E): поток недоступен (цикл) — волна
        // каскада не строится (нет топологии потока); сравнение
        // итогов продолжится со следующего штатного пересчёта.
        self.flow_changed_nodes.clear();
        // FR-016: поток недоступен (цикл) — анализ пуст: без
        // FlowSolutions детекции не на чем (честное отсутствие, не
        // ложное «всё здорово»).
        self.analysis.clear();
        // FR-042: кэш пучков — в обеих ветках выхода пересчёта
        // (мутация возможна и при цикле потока)
        self.bundles = EdgeBundleIndex::build(&self.canvas);
        self.apply_result_reserve();
    }

    /// FR-017: пересчёт активного сценария. Пустые подмены — база (нулевая
    /// дельта); цикл из подмен невозможен (граф тот же), но страховка:
    /// показываем базу, не падая. Общая точка sync-пути и fallback'а
    /// воркера — одинаковое поведение обоих путей (контракт FR-064:
    /// «sync-результат побитово идентичен»).
    fn compute_active(
        &self,
        whatif: &flow::WhatIfOverrides,
        baseline: &flow::FlowSolutions,
    ) -> flow::FlowSolutions {
        if whatif.line_exprs.is_empty() && whatif.node_values.is_empty() {
            baseline.clone()
        } else {
            match flow::propagate_with_lines(&self.canvas, whatif) {
                Ok(solutions) => solutions,
                // Цикл из подмен невозможен (граф тот же), но страховка:
                // показываем базу, не падая
                Err(cycle) => {
                    tracing::warn!(cycle = %cycle, "what-if пересчёт отклонён — показана база");
                    baseline.clone()
                }
            }
        }
    }

    /// FR-064 P1: хвост пересчёта — публикация пары снимков в double buffer
    /// + выводка O(N) (остаётся на UI-треде, план §5.4).
    ///
    /// Единая точка и для sync-пути, и для завершения запроса воркера —
    /// оба пути дают идентичное состояние сцены.
    fn apply_flow_pair(
        &mut self,
        baseline: flow::FlowSolutions,
        active: flow::FlowSolutions,
        whatif: flow::WhatIfOverrides,
        stale: Vec<StaleOverride>,
        prev_results: ExprResults,
        prev_solutions: flow::FlowSolutions,
    ) {
        // FR-064 P1: публикация снимков double buffer — до выводки, чтобы
        // читатели увидели новую пару атомарно (RwLock гарантирует
        // целостность каждого снимка; выводка ниже консистентна ей же).
        *write_flow(&self.flow_baseline) = baseline;
        *write_flow(&self.flow_active) = active;
        // PRD-0007 (AC-2.4): цикл кончился — explain строится по значениям.
        self.flow_cycle = None;
        self.whatif_stale = stale;
        let solutions = read_flow(&self.flow_active);
        self.expr_results = outputs_to_results(&solutions.outputs);
        // FR-050 Н9-1 (этап E): детект изменений значений — seeds волны
        // каскада. Три наблюдаемых вывода: узловой итог (expr_results),
        // значения строк (lines — присваивающие листы-истоки) и
        // именованные выходы (named — адресация fromOutput). Первый
        // пересчёт (загрузка) — пусто. Сортировка + дедуп — детерминизм.
        // Вычисляется ДО мутаций сцены ниже (заимствование solutions);
        // присваивание — последней строкой хвоста.
        let flow_changed: Vec<String> = if self.flow_computed {
            let mut changed: Vec<String> = self
                .expr_results
                .iter()
                .filter(|(id, outcome)| prev_results.get(*id) != Some(*outcome))
                .map(|(id, _)| id.clone())
                .collect();
            changed.extend(
                solutions
                    .lines
                    .iter()
                    .filter(|(key, value)| prev_solutions.lines.get(key) != Some(*value))
                    .map(|(key, _)| key.0.clone()),
            );
            changed.extend(
                solutions
                    .named
                    .iter()
                    .filter(|(key, value)| prev_solutions.named.get(key) != Some(*value))
                    .map(|(key, _)| key.0.clone()),
            );
            changed.sort();
            changed.dedup();
            changed
        } else {
            Vec::new()
        };
        // FR-016 (CP5): анализ узких мест — чистая функция над теми же
        // решениями (значения + именованные выходы utilization). Пороги —
        // дефолт документа FR-016; кастомизация — v2 (конфиг в .canvas).
        self.analysis = analyze::analyze(&self.canvas, &solutions, &AnalysisConfig::default());
        self.expr_line_results.clear();
        for node in &self.canvas.nodes {
            let text = node.text.clone().unwrap_or_default();
            // FR-017: построчные результаты — по ВИРТУАЛЬНОМУ исходнику
            // (тем же подменам, что у propagator — инвариант 4)
            let text = flow::whatif_virtual_text(&text, &whatif.line_overrides(&node.id));
            // FR-025: слоты с учётом построчных истоков — строки downstream
            // нод видят значения строк источников (`= $in × 2` от строки).
            // FR-049: + `named` — слоты резолвляют и именованные выходы
            // (fromOutput-рёбра), иначе построчные бейджи приёмников
            // краснели бы «вход отсутствует» при верном узловом итоге.
            // FR-050 этап F: + qualified-карта «Объект.Поле» — формулы
            // строк резолвят именованные ссылки так же, как узловой итог
            // (инвариант «строка и узел видят одно окружение»; найдено
            // миграцией схем на именованные ссылки).
            let env = flow::line_eval_env(
                &self.canvas,
                &node.id,
                &solutions.outputs,
                &solutions.lines,
                &solutions.named,
            );
            let line_results = expr::eval_lines_in(&text, &env);
            if line_results.iter().any(Option::is_some) {
                self.expr_line_results.insert(node.id.clone(), line_results);
            }
        }
        // FR-029 (визуализация проливания): параметры, запитанные рёбрами
        // с toParam — рендер покажет «param ← источник» и эффективный бейдж.
        // Эффективное значение — значение РЕБРА-источника (адресация fromLine/
        // fromOutput/узловое), а не локальный RHS строки: бейдж и подпись
        // показывают истину потока, а не захардкоженный литерал листа.
        let mut param_spills: HashMap<String, Vec<SpillView>> = HashMap::new();
        // FR-050 Н9-2 (этап D): счётчики имён — квалифицированные пути
        // «Объект.Поле» источников (коллизия — «Имя (node_id)»).
        let name_counts = canvas_core::dataref::display_name_counts(&self.canvas);
        for node in &self.canvas.nodes {
            let spills = flow::param_spills(&self.canvas, &node.id);
            if spills.is_empty() {
                continue;
            }
            let views = spills
                .into_iter()
                .map(|spill| {
                    // Значение ребра-источника — что реально пролито
                    // в параметр (не локальный RHS строки).
                    let value = spill_edge_value(&solutions, &spill).map(|v| v.to_string());
                    // Н9-2: путь «Объект.Поле» для тултипа «пролито: …».
                    let obj = canvas_core::dataref::qualified_obj_name(
                        &self.canvas,
                        &spill.from_node,
                        &name_counts,
                    );
                    let field = flow::spill_source_field(
                        &self.canvas,
                        &spill.from_node,
                        spill.from_output.as_deref(),
                        spill.from_line,
                        &spill.from_label,
                    );
                    // Н9-2: локальный литерал RHS строки присваивания
                    // («(локально было: 500 rps)») — правка значения
                    // по Р-5 возвращает именно его.
                    let local = assignment_line(node, &spill.param).and_then(|line| {
                        node.text
                            .as_deref()
                            .and_then(|text| text.lines().nth(line))
                            .and_then(|line| line.split_once('='))
                            .map(|(_, rhs)| rhs.trim().to_owned())
                            .filter(|rhs| !rhs.is_empty())
                    });
                    SpillView {
                        line: assignment_line(node, &spill.param),
                        param: spill.param,
                        from_label: spill.from_label,
                        from_output: spill.from_output,
                        value,
                        path: format!("{obj}.{field}"),
                        local,
                    }
                })
                .collect();
            param_spills.insert(node.id.clone(), views);
        }
        self.param_spills = param_spills;
        // FR-050 Р-4 (Н10-а): авто-строки приёмников — производные данные
        // пересчёта (значения активного сценария): детерминированы,
        // повторный пересчёт даёт идентичные строки (инвариант 2 FR-050);
        // рендер — этап D.
        let mut auto_rows: HashMap<String, Vec<flow::AutoRow>> = HashMap::new();
        for node in &self.canvas.nodes {
            let rows = flow::auto_rows(&self.canvas, &node.id, &solutions);
            if rows.is_empty() {
                continue;
            }
            auto_rows.insert(node.id.clone(), rows);
        }
        self.auto_rows = auto_rows;
        // FR-050 Р-3 (этап C): unmapped-рёбра «не подставлено» — те же
        // готовые результаты пересчёта (дедупликация по id: ребро адресует
        // ровно одну ноду, но фильтр стоит дёшево и страхует порядок).
        let mut unmapped_edges: Vec<String> = Vec::new();
        for node in &self.canvas.nodes {
            for input in flow::unmapped_inputs(&self.canvas, &node.id, &solutions) {
                if !unmapped_edges.contains(&input.edge_id) {
                    unmapped_edges.push(input.edge_id);
                }
            }
        }
        self.unmapped_edges = unmapped_edges;
        // Заимствование `solutions` закончено — ниже мутации сцены.
        drop(solutions);
        // FR-050 Р-4 (этап D): рост высоты под авто-строки — growth-only
        // (как CR-012): карточка обязана вместить строку-проекцию, иначе
        // она обрежется клипом тела; достаточная высота не трогается.
        for index in 0..self.canvas.nodes.len() {
            self.ensure_spill_rows_reserve(index);
        }
        // FR-017 (CP6): what-if представления нод для рендера — виртуальный
        // исходник, подсветка подмен, дельта-бейджи (только ноды с подменами;
        // рельеф базы рендер рисует как есть).
        self.whatif_nodes = self.build_whatif_nodes(&whatif);
        // W8 (web-приёмка): оракул браузерного дыма — Numi-формулы и поток
        // значений живут на web-сцене (критерий приёмки W8, wasm-port §6):
        // values — ноды с вычисленным итогом; errors — бейджи ошибок
        // (парсинг/вычисление); lines — построчные результаты. DEBUG — на
        // нативе под дефолтным фильтром не виден, на web виден с ?log=debug.
        let errors = self
            .expr_results
            .values()
            .filter(|outcome| matches!(outcome, ExprOutcome::Err(_)))
            .count();
        tracing::debug!(
            values = self.expr_results.len(),
            errors,
            lines = self.expr_line_results.len(),
            "пересчёт потока: значения вычислены"
        );
        // CR-012: ленивый refit высоты — резерв футера результата.
        self.apply_result_reserve();
        // Динамический перерасчёт `MeasuredReserveFn` (T9-сессия 2026-09-24):
        // для нод с изменившимся хешем контента — реальный замер с усадкой
        // и ростом. Mode-тогглы (block_collapsed/desc_expanded) сюда НЕ
        // попадают — они на growth-only `ensure_reserve_at` (I-6). Хеш-гейт
        // гарантирует перф: при прежнем контенте замер не запускается.
        // Поверх `apply_result_reserve` — рост уже учтён, здесь ловим усадку
        // (и подтверждаем рост, если оценка уровня 1 его пропустила).
        for index in 0..self.canvas.nodes.len() {
            self.refit_node_to_content_if_changed(index);
        }
        // FR-042 (E2): перестройка индекса пучков — единственная точка
        // синхронизации (хвост recompute_flow): все мутации топологии
        // завершаются пересчётом потока; O(edges) поверх него, вне кадра.
        self.bundles = EdgeBundleIndex::build(&self.canvas);
        // FR-050 Н9-1 (этап E): присвоение диффа — в самом конце (все
        // мутации сцены завершены; borrow solutions уже освобождён)
        self.flow_changed_nodes = flow_changed;
        self.flow_computed = true;
    }

    // --- FR-064 P1: сценарный воркер (desktop-only) ------------------------

    /// FR-064 P1: подключить воркер (вызывает приложение в main() после
    /// создания `EventLoopProxy`; sync-фолбэк остаётся на все случаи
    /// отказа/таймаута/паники воркера). Повторный вызов подменяет хэндл.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn attach_flow_worker(&mut self, worker: crate::worker::FlowWorkerHandle) {
        self.flow_worker = Some(worker);
    }

    /// FR-064 P1: отправить тяжёлые прогоны воркеру. `true` — запрос ушёл,
    /// выводка отложена до готовности пары; `false` — воркера нет/он мёртв,
    /// вызывающий выполняет sync-путь.
    #[cfg(not(target_arch = "wasm32"))]
    fn request_flow_recompute(
        &mut self,
        whatif: &flow::WhatIfOverrides,
        stale: Vec<StaleOverride>,
        prev_results: ExprResults,
        prev_solutions: flow::FlowSolutions,
    ) -> bool {
        let Some(worker) = self.flow_worker.as_ref() else {
            return false;
        };
        self.flow_generation = self.flow_generation.wrapping_add(1);
        let generation = self.flow_generation;
        let canvas = Arc::new(self.canvas.clone());
        // База — всегда (дельты против чистого пересчёта, гипотеза Q9);
        // активный сценарий — только с непустыми подменами (иначе активное
        // решение зеркалит базу, лишний прогон не нужен).
        let mut sent = worker.request(crate::worker::FlowJob {
            generation,
            kind: crate::worker::FlowKind::Baseline,
            canvas: Arc::clone(&canvas),
            whatif: flow::WhatIfOverrides::default(),
        });
        let has_overrides = !whatif.line_exprs.is_empty() || !whatif.node_values.is_empty();
        if has_overrides {
            sent &= worker.request(crate::worker::FlowJob {
                generation,
                kind: crate::worker::FlowKind::Active,
                canvas,
                whatif: whatif.clone(),
            });
        }
        if !sent {
            // Воркер мёртв (канал закрыт) — sync-фолбэк + warn (правило
            // AGENTS «фолбэк + warn»; не молчаливое падение).
            tracing::warn!("воркер потока недоступен — синхронный пересчёт (фолбэк)");
            return false;
        }
        self.flow_pending = Some(FlowPending {
            generation,
            whatif: whatif.clone(),
            stale,
            prev_results,
            prev_solutions,
            baseline: None,
            active: if has_overrides {
                None
            } else {
                Some(FlowSlot::MirrorsBaseline)
            },
            deadline: Instant::now() + FLOW_WORKER_TIMEOUT,
        });
        true
    }

    /// FR-064 P1: завершить пересчёт по готовности снимков воркера (вызывается
    /// из обработчика `AppEvent::FlowReady`). Забирает исходы из канала,
    /// отбрасывает устаревшие поколения; когда пара собрана — публикует
    /// снимки в double buffer и выполняет выводку O(N) (единый редактор,
    /// план §5.4). Возвращает `true` — состояние сцены обновилось (нужен
    /// кадр). При панике/отказе воркера — sync-фолбэк + `warn`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn complete_flow_recompute(&mut self) -> bool {
        let Some(worker) = self.flow_worker.as_ref() else {
            return false;
        };
        let Some(mut pending) = self.flow_pending.take() else {
            // Нечего завершать; попутно дреним чужие/устаревшие исходы.
            let _ = worker.take_outcomes();
            return false;
        };
        for outcome in worker.take_outcomes() {
            match outcome {
                crate::worker::FlowOutcome::Done {
                    generation,
                    kind,
                    solutions,
                } if generation == pending.generation => match kind {
                    crate::worker::FlowKind::Baseline => {
                        pending.baseline = Some(FlowSlot::Done(solutions));
                    }
                    crate::worker::FlowKind::Active => {
                        pending.active = Some(FlowSlot::Done(solutions));
                    }
                },
                crate::worker::FlowOutcome::Panicked { generation, kind }
                    if generation == pending.generation =>
                {
                    match kind {
                        crate::worker::FlowKind::Baseline => {
                            pending.baseline = Some(FlowSlot::Panicked);
                        }
                        crate::worker::FlowKind::Active => {
                            pending.active = Some(FlowSlot::Panicked);
                        }
                    }
                }
                // Чужое поколение (пересчёт уже перезаказан) — отброс.
                _ => {}
            }
        }
        let Some(baseline_slot) = pending.baseline.take() else {
            // База ещё считается — ждём следующие FlowReady.
            self.flow_pending = Some(pending);
            return false;
        };
        let Some(active_slot) = pending.active.take() else {
            self.flow_pending = Some(pending);
            return false;
        };
        // База: цикл value-рёбер (чужой .canvas) — та же деградация, что в
        // sync-ветке (изолированный расчёт + warn); паника/неполный ответ
        // воркера — полный sync-фолбэк (контракт FR-064: «воркер паникует →
        // sync-результат побитово идентичный»).
        let baseline = match baseline_slot {
            FlowSlot::Done(Ok(solutions)) => solutions,
            FlowSlot::Done(Err(cycle)) => {
                self.apply_flow_cycle(cycle);
                return true;
            }
            FlowSlot::Panicked => {
                tracing::warn!("паника воркера потока — синхронный пересчёт (фолбэк)");
                return self.sync_flow_fallback(pending);
            }
            FlowSlot::MirrorsBaseline => {
                // База не может зеркалить саму себя — некорректный ответ
                // воркера; деградация как при панике.
                tracing::warn!(
                    "воркер потока вернул неполный ответ — синхронный пересчёт (фолбэк)"
                );
                return self.sync_flow_fallback(pending);
            }
        };
        // Активное решение: цикл из подмен невозможен (граф тот же), но
        // страховка — показываем базу (симметрия sync-пути).
        let active = match active_slot {
            FlowSlot::Done(Ok(solutions)) => solutions,
            FlowSlot::Done(Err(cycle)) => {
                tracing::warn!(cycle = %cycle, "what-if пересчёт отклонён — показана база");
                baseline.clone()
            }
            FlowSlot::MirrorsBaseline => baseline.clone(),
            FlowSlot::Panicked => {
                tracing::warn!("паника воркера потока — синхронный пересчёт (фолбэк)");
                return self.sync_flow_fallback(pending);
            }
        };
        self.apply_flow_pair(
            baseline,
            active,
            pending.whatif,
            pending.stale,
            pending.prev_results,
            pending.prev_solutions,
        );
        true
    }

    /// FR-064 P1: тик обслуживания воркера (вызывается из `about_to_wait`):
    /// таймаут зависшего запроса → sync-фолбэк + `warn`; попутный дренаж
    /// готовых исходов (если wake-событие потерялось). `true` — сцена
    /// обновилась (нужен кадр).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn flow_worker_tick(&mut self) -> bool {
        if self
            .flow_pending
            .as_ref()
            .is_some_and(|pending| Instant::now() >= pending.deadline)
        {
            tracing::warn!("таймаут воркера потока — синхронный пересчёт (фолбэк)");
            if let Some(pending) = self.flow_pending.take() {
                return self.sync_flow_fallback(pending);
            }
            return false;
        }
        self.complete_flow_recompute()
    }

    /// FR-064 P1: sync-фолбэк — тяжёлые прогоны на вызвавшем (UI) треде,
    /// состояние идентично штатному пути (тест worker_smoke проверяет
    /// побитовую идентичность). Правило AGENTS: деградация = sync + warn
    /// (warn уже отправлен вызывающим).
    #[cfg(not(target_arch = "wasm32"))]
    fn sync_flow_fallback(&mut self, pending: FlowPending) -> bool {
        let baseline =
            match flow::propagate_with_lines(&self.canvas, &flow::WhatIfOverrides::default()) {
                Ok(solutions) => solutions,
                Err(cycle) => {
                    self.apply_flow_cycle(cycle);
                    return true;
                }
            };
        let active = self.compute_active(&pending.whatif, &baseline);
        self.apply_flow_pair(
            baseline,
            active,
            pending.whatif,
            pending.stale,
            pending.prev_results,
            pending.prev_solutions,
        );
        true
    }

    /// FR-017: собрать what-if представления нод активного сценария
    /// (рендер): виртуальный исходник, индексы подменённых строк, дельты
    /// строк и узлового итога в полном формате «было → стало (+Δ)».
    fn build_whatif_nodes(&self, whatif: &flow::WhatIfOverrides) -> HashMap<String, WhatIfNode> {
        let mut map = HashMap::new();
        if whatif.line_exprs.is_empty() {
            return map;
        }
        // Группировка подмен по нодам (сортировка строк — детерминизм).
        let mut per_node: HashMap<String, Vec<(usize, String)>> = HashMap::new();
        for ((node_id, line), expr) in &whatif.line_exprs {
            per_node
                .entry(node_id.clone())
                .or_default()
                .push((*line, expr.clone()));
        }
        for (node_id, mut lines) in per_node {
            lines.sort_by_key(|(line, _)| *line);
            let Some(node) = self.canvas.node(&node_id) else {
                continue;
            };
            let base_text = node.text.clone().unwrap_or_default();
            let refs: Vec<(usize, &String)> = lines.iter().map(|(l, e)| (*l, e)).collect();
            let text = flow::whatif_virtual_text(&base_text, &refs);
            let mut line_deltas = Vec::new();
            // FR-064 P1: double buffer — чтение снимков через read()-гарды.
            let base_solutions = read_flow(&self.flow_baseline);
            let active_solutions = read_flow(&self.flow_active);
            for (line, _) in &lines {
                let key = (node_id.clone(), *line);
                let (Some(base), Some(whatif_value)) = (
                    base_solutions.lines.get(&key),
                    active_solutions.lines.get(&key),
                ) else {
                    continue;
                };
                if let Some(full) = whatif_full_delta(base, whatif_value) {
                    line_deltas.push((*line, full));
                }
            }
            // Дельта узлового итога — только если футер результата виден
            // (построчные результаты Numi-листа заменяют его у обычных нод;
            // у шаблонных футер — всегда).
            let footer_delta = self
                .canvas
                .nodes
                .iter()
                .position(|node| node.id == node_id)
                .filter(|index| self.node_shows_result_footer(*index))
                .and_then(|_| {
                    let base = base_solutions.outputs.get(&node_id);
                    let whatif_value = active_solutions.outputs.get(&node_id);
                    match (base, whatif_value) {
                        (Some(Ok(base)), Some(Ok(whatif_value))) => {
                            whatif_full_delta(base, whatif_value)
                        }
                        _ => None,
                    }
                });
            drop(base_solutions);
            drop(active_solutions);
            map.insert(
                node_id.clone(),
                WhatIfNode {
                    text,
                    overrides: lines.into_iter().map(|(line, _)| line).collect(),
                    line_deltas,
                    footer_delta,
                },
            );
        }
        map
    }

    /// FR-017: подмены активного сценария + протухшие маркеры. Режим не
    /// активен или «База» — пустые подмены (propagator = baseline).
    /// Активные what-if подмены (протухшие отфильтрованы — тихая
    /// деградация, маркеры в whatif_stale). pub(crate): свежий пересчёт
    /// активного состояния в MCP-инструментах (flow_recalc/analyze/
    /// lineage) — тот же источник подмен, что у recompute_flow.
    pub(crate) fn active_whatif_overrides(&self) -> (flow::WhatIfOverrides, Vec<StaleOverride>) {
        let mut whatif = flow::WhatIfOverrides::default();
        let mut stale = Vec::new();
        if self.whatif_active {
            if let Some(scenario) = self.active_scenario.and_then(|i| self.scenarios.get(i)) {
                stale = canvas_core::whatif::validate_scenario(&self.canvas, scenario);
                whatif.line_exprs = canvas_core::whatif::active_line_exprs(&self.canvas, scenario);
            }
        }
        (whatif, stale)
    }

    /// Публичный доступ к свежим подменам активного сценария (PRD-0007 X6,
    /// F-12: индикатор покрытия цепочками в canvas-app считает деревья из
    /// того же источника подмен, что recompute_flow и MCP-инструменты —
    /// инвариант «MCP-видимость = UI»). Свежий пересчёт валидности —
    /// ленивые мутации (CR-012) не искажают подмены.
    pub fn fresh_whatif_overrides(&self) -> flow::WhatIfOverrides {
        self.active_whatif_overrides().0
    }

    /// FR-017: число подмен активного сценария (для счётчика бара).
    pub fn whatif_override_count(&self) -> usize {
        self.active_scenario
            .and_then(|i| self.scenarios.get(i))
            .map(|scenario| scenario.line_exprs.len())
            .unwrap_or(0)
    }

    /// FR-017: переключить активный сценарий (`None` — «База») и
    /// пересчитать. Runtime-действие: `.canvas` не мутируется (инвариант 2).
    pub fn whatif_activate(&mut self, index: Option<usize>) {
        self.active_scenario = index;
        self.recompute_flow();
    }

    // --- FR-064 P2 (FR-017 v2): freeze/сравнение ---------------------------

    /// FR-064 P2: заморозить активный сценарий — снимок активных решений
    /// потока за Arc (правки канваса значения не двигают). Runtime-действие:
    /// персистентность имён — вызывающий (`frozen_to_canvas` + undo-шаг,
    /// паттерн whatif_create_scenario). `Err` — сценария нет / уже заморожен.
    pub fn whatif_freeze_active(&mut self) -> Result<String, String> {
        let name = match self.active_scenario {
            Some(index) => self
                .scenarios
                .get(index)
                .map(|scenario| scenario.name.clone())
                .ok_or_else(|| "сценарий не найден".to_owned())?,
            None => "База".to_owned(),
        };
        if self.frozen.iter().any(|snapshot| snapshot.name == name) {
            return Err(format!("сценарий уже заморожен: {name}"));
        }
        // Снимок — копия текущих активных решений (активное решение при
        // «Базе» зеркалит базу — заморозка честная в обоих случаях).
        let solutions = read_flow(&self.flow_active).clone();
        self.frozen.push(canvas_core::whatif::FrozenScenario {
            name: name.clone(),
            solutions: Arc::new(solutions),
        });
        Ok(name)
    }

    /// FR-064 P2: снять заморозку по имени. `true` — снято (заморозки не
    /// было — `false`, no-op).
    pub fn whatif_unfreeze(&mut self, name: &str) -> bool {
        let before = self.frozen.len();
        self.frozen.retain(|snapshot| snapshot.name != name);
        self.frozen.len() != before
    }

    /// FR-064 P2: имена замороженных снимков (порядок заморозки).
    pub fn whatif_frozen_names(&self) -> Vec<String> {
        self.frozen
            .iter()
            .map(|snapshot| snapshot.name.clone())
            .collect()
    }

    /// FR-064 P2: заморожен ли сценарий с таким именем.
    pub fn whatif_is_frozen(&self, name: &str) -> bool {
        self.frozen.iter().any(|snapshot| snapshot.name == name)
    }

    /// FR-064 P2: восстановить freeze-снимки по persisted именам (при
    /// загрузке сцены): пересчёт каждого сценария против персистентного
    /// канваса (детерминизм propagator'а — значения воспроизводятся).
    /// Отсутствующий сценарий пропускается (имя не восстанавливается).
    pub fn restore_frozen(&mut self, names: &[String]) {
        for name in names {
            let snapshot = self
                .scenarios
                .iter()
                .find(|scenario| &scenario.name == name)
                .and_then(|scenario| {
                    canvas_core::whatif::freeze_scenario(&self.canvas, scenario).ok()
                });
            if let Some(snapshot) = snapshot {
                self.frozen.push(snapshot);
            }
        }
    }

    /// FR-017: новый именованный сценарий (лимит 2–3 пользовательских —
    /// гипотеза Q5b; сверх лимита — отказ). Список сценариев персистентен:
    /// мутация `Canvas.extra` с push_undo вызывающей стороной.
    pub fn whatif_create_scenario(&mut self, name: &str) -> Result<usize, String> {
        const MAX_SCENARIOS: usize = 3;
        if self.scenarios.len() >= MAX_SCENARIOS {
            return Err(format!(
                "лимит сценариев: не более {MAX_SCENARIOS} (гипотеза Q5b)"
            ));
        }
        let name = if name.trim().is_empty() {
            // CR-016: автоимя — первый свободный номер, а не len+1: при
            // непоследовательных именах («Сценарий 2», «Сценарий 3») len+1
            // коллидирует с существующим и «+» падает с «уже существует».
            (1..)
                .map(|n| format!("Сценарий {n}"))
                .find(|candidate| {
                    !self
                        .scenarios
                        .iter()
                        .any(|scenario| &scenario.name == candidate)
                })
                .expect("свободный номер сценария")
        } else {
            name.trim().to_owned()
        };
        if self.scenarios.iter().any(|scenario| scenario.name == name) {
            return Err(format!("сценарий уже существует: {name}"));
        }
        self.scenarios.push(Scenario {
            name,
            line_exprs: HashMap::new(),
        });
        Ok(self.scenarios.len() - 1)
    }

    /// FR-017: удалить сценарий по индексу (переключение на «Базу», если
    /// удалён активный).
    pub fn whatif_delete_scenario(&mut self, index: usize) {
        if index >= self.scenarios.len() {
            return;
        }
        self.scenarios.remove(index);
        self.active_scenario = match self.active_scenario {
            Some(active) if active == index => None,
            Some(active) if active > index => Some(active - 1),
            other => other,
        };
    }

    /// FR-017 (Q6b): Apply активного сценария — записать подмены в
    /// persisted-строки/params и УДАЛИТЬ сценарий (его смысл исчерпан).
    /// Вызывающий отвечает за push_undo ДО вызова и mark_dirty/ревал ПОСЛЕ.
    pub fn whatif_apply_active(&mut self) -> usize {
        let Some(index) = self.active_scenario else {
            return 0;
        };
        let Some(scenario) = self.scenarios.get(index).cloned() else {
            return 0;
        };
        let applied = canvas_core::whatif::active_line_exprs(&self.canvas, &scenario);
        for ((node_id, line), expr) in &applied {
            let Some(node_index) = self.canvas.nodes.iter().position(|n| &n.id == node_id) else {
                continue;
            };
            let text = self.canvas.nodes[node_index]
                .text
                .clone()
                .unwrap_or_default();
            let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
            if line >= &lines.len() {
                continue;
            }
            lines[*line] = expr.clone();
            let text = lines.join("\n");
            let node = &mut self.canvas.nodes[node_index];
            node.text = Some(text.clone());
            // Шаблонная нода: синхронизация снапшота params (паттерн
            // finish_editing FR-018/FR-023 — слияние, не замена)
            if node.template().is_some() {
                let fresh = canvas_core::templates::params_from_text(&text);
                let params = canvas_core::templates::merge_params(node.template_params(), fresh);
                node.set_template_params(params);
            }
        }
        // Сценарий применён — удаляем (Q6b); остальные валидны против новой базы
        self.whatif_delete_scenario(index);
        self.active_scenario = None;
        applied.len()
    }

    /// CR-012: нода получит футер результата по правилу рендера
    /// (text.rs): шаблонные — всегда (итог формулы шаблона поверх
    /// построчных результатов), обычные — только при отсутствии
    /// построчных Numi-результатов и наличии итога/ошибки формулы.
    pub fn node_shows_result_footer(&self, index: usize) -> bool {
        let node = &self.canvas.nodes[index];
        let has_line_results = self
            .expr_line_results
            .get(&node.id)
            .is_some_and(|lines| lines.iter().any(Option::is_some));
        if has_line_results && node.template().is_none() {
            return false;
        }
        matches!(
            self.expr_results.get(&node.id),
            Some(ExprOutcome::Ok(_) | ExprOutcome::Err(_))
        )
    }

    /// CR-012 / FR-069 (этап F): growth-only рост высоты ноды под контент
    /// тела и резерв футера результата; spatial index обновляется только
    /// при реальном росте. formula_lines — из построчных результатов ноды
    /// (тот же источник, что у рендера). Ранний выход по
    /// `node_shows_result_footer` СНЯТ: подгонка работает для ВСЕХ нод —
    /// текст без футера больше не вылезает за низ карточки; резерв футера
    /// (`RESULT_LINE_HEIGHT + 2.0`) добавляется только нодам с футером —
    /// без «пустого хвоста» у нод без итога. Состояние тогглов учитывается:
    /// раскрытое описание («⋯ целиком ▾») измеряется целиком.
    pub fn ensure_reserve_at(&mut self, index: usize) {
        let formula_lines = self
            .expr_line_results
            .get(&self.canvas.nodes[index].id)
            .map(|lines| formula_line_indices(lines))
            .unwrap_or_default();
        // FR-029: подгонка по показываемому тексту (пролитые строки —
        // подписи источников, длиннее локальных литералов).
        let display = display_body_text(&self.canvas.nodes[index], &self.param_spills);
        let desc = self.node_desc_text(index);
        let desc_expanded = self.desc_expanded.contains(&self.canvas.nodes[index].id);
        let footer_reserve = self.node_shows_result_footer(index);
        // FR-069 (этап F): Σ-строка — итог есть (footer_reserve) и среди
        // строк с исходами есть расчётные; имя — общая функция ядра (I-2).
        let params = formula_lines
            .iter()
            .copied()
            .filter(|&i| {
                display.lines().nth(i).is_some_and(|line| {
                    matches!(
                        canvas_core::expr::line_kind(line),
                        canvas_core::expr::NumiLineKind::Assignment { .. }
                    )
                })
            })
            .count();
        let sigma_name = if footer_reserve && formula_lines.len() > params {
            format!("Σ {}", self.canvas.nodes[index].sigma_row_name())
        } else {
            String::new()
        };
        // FR-069 хвосты (T9-сессия 2026-09-24): `auto_rows = 0` — этот
        // путь НЕ видит авто-строк (они в ensure_spill_rows_reserve);
        // подпись зоны авто-строк не нужна.
        let auto_rows = 0;
        let before = self.canvas.nodes[index].height;
        ensure_result_reserve(
            &mut self.canvas.nodes[index],
            &display,
            &formula_lines,
            desc.as_deref(),
            desc_expanded,
            footer_reserve,
            &sigma_name,
            auto_rows,
        );
        if self.canvas.nodes[index].height > before {
            let node = &self.canvas.nodes[index];
            self.spatial.update(index, node);
        }
    }

    /// FR-061 этап D (D-8): текст описания ноды — `canvasdesk.desc` →
    /// описание манифеста шаблона (`template_descs` по снимку id).
    /// ПРИЁМКА T9 (фидбэк владельца 2026-09-24): prose-фолбэк «первый
    /// проза-абзац» УБРАН — синхронно с рендером (text.rs): фолбэк рисовал
    /// первый абзац тела дважды (зона описания + тело — дублирование
    /// текста). Измерение и рендер остаются на одном источнике (I-2).
    /// Пусто — зоны описания нет.
    pub(crate) fn node_desc_text(&self, index: usize) -> Option<String> {
        let node = self.canvas.nodes.get(index)?;
        node.canvasdesk
            .as_ref()
            .and_then(|ext| ext.desc.clone())
            .or_else(|| {
                node.template()
                    .and_then(|t| self.template_descs.get(&t.id).cloned())
            })
            .filter(|d| !d.trim().is_empty())
    }

    /// FR-061 хвосты (D-7 runtime v1): тоггл свёрнутости блока-ведомости
    /// ноды (клик по заголовку; runtime — Q4). Возвращает true, если блок
    /// после тоггла свёрнут (для отладки/тестов).
    pub fn toggle_block_collapsed(&mut self, node_id: &str) -> bool {
        if !self.block_collapsed.remove(node_id) {
            self.block_collapsed.insert(node_id.to_owned());
            true
        } else {
            false
        }
    }

    /// FR-061 хвосты (D-8 runtime v1): тоггл раскрытости описания ноды
    /// («⋯ целиком ▾» / «▴ свернуть»). Возвращает true, если после тоггла
    /// описание раскрыто.
    pub fn toggle_desc_expanded(&mut self, node_id: &str) -> bool {
        if !self.desc_expanded.remove(node_id) {
            self.desc_expanded.insert(node_id.to_owned());
            true
        } else {
            false
        }
    }

    /// FR-061 хвосты (D-8, «Раскрыть+авто»): автосворачивание раскрытых
    /// описаний — клик вне ноды `keep` / начало правки. `keep: Some(id)` —
    /// описание ноды `id` сохраняется (клик по её телу).
    pub fn collapse_descs_except(&mut self, keep: Option<&str>) {
        match keep {
            Some(id) => self.desc_expanded.retain(|x| x == id),
            None => self.desc_expanded.clear(),
        }
    }

    /// CR-012: ленивый refit всех нод канваса — резерв футера результата
    /// для нод, которым рендер его покажет. Вызывается в конце ЛЮБОГО
    /// пересчёта (recompute_flow, в т.ч. загрузка .canvas и MCP-мутации) —
    /// growth-only, поэтому повторные вызовы дёшевы и не осциллируют.
    pub fn apply_result_reserve(&mut self) {
        for index in 0..self.canvas.nodes.len() {
            self.ensure_reserve_at(index);
        }
    }

    /// FR-050 Р-4 (этап D): growth-only рост высоты под авто-строки
    /// приёмника. Измерение — через тот же двухуровневый механизм CR-012:
    /// строки-проекции препендятся показываемому тексту (метрики моно
    /// совпадают с формульными строками — префиксные индексы входят в
    /// formula_lines), индексы формул сдвигаются на длину префикса.
    /// FR-069 (этап F): резерв футера — по флагу
    /// `node_shows_result_footer` (побочный +RESULT_LINE_HEIGHT у нод без
    /// итога снят — футер-флаг двухуровневого refit). spatial index —
    /// только при реальном росте.
    pub fn ensure_spill_rows_reserve(&mut self, index: usize) {
        let Some(rows) = self
            .auto_rows
            .get(&self.canvas.nodes[index].id)
            .filter(|rows| !rows.is_empty())
        else {
            return;
        };
        let prefix: Vec<String> = rows.iter().map(|row| row.display_text()).collect();
        let display_body = display_body_text(&self.canvas.nodes[index], &self.param_spills);
        let display = if display_body.is_empty() {
            prefix.join("\n")
        } else {
            format!("{}\n{}", prefix.join("\n"), display_body)
        };
        let shift = prefix.len();
        let formula_lines: Vec<usize> = self
            .expr_line_results
            .get(&self.canvas.nodes[index].id)
            .map(|lines| formula_line_indices(lines))
            .unwrap_or_default()
            .into_iter()
            .map(|i| i + shift)
            .chain(0..shift)
            .collect();
        let mut formula_lines = formula_lines;
        formula_lines.sort_unstable();
        let desc = self.node_desc_text(index);
        let desc_expanded = self.desc_expanded.contains(&self.canvas.nodes[index].id);
        let footer_reserve = self.node_shows_result_footer(index);
        // FR-069 (этап F): Σ-строка — как в ensure_reserve_at (префиксные
        // индексы авто-строк — присваивания «путь = значение», в calcs не
        // попадают, счёт согласован с рендером).
        let params = formula_lines
            .iter()
            .copied()
            .filter(|&i| {
                display.lines().nth(i).is_some_and(|line| {
                    matches!(
                        canvas_core::expr::line_kind(line),
                        canvas_core::expr::NumiLineKind::Assignment { .. }
                    )
                })
            })
            .count();
        let sigma_name = if footer_reserve && formula_lines.len() > params {
            format!("Σ {}", self.canvas.nodes[index].sigma_row_name())
        } else {
            String::new()
        };
        // FR-069 хвосты (T9-сессия 2026-09-24): авто-строки есть — рендер
        // вставляет подпись зоны «ВХОДЯЩИЕ ЗНАЧЕНИЯ · N»; уровень 1
        // обязан учесть её ряд (I-2). Сами строки уже в `display` как
        // префикс «путь = значение» — +1 ряд только на метку.
        let auto_rows = rows.len();
        let before = self.canvas.nodes[index].height;
        ensure_result_reserve(
            &mut self.canvas.nodes[index],
            &display,
            &formula_lines,
            desc.as_deref(),
            desc_expanded,
            footer_reserve,
            &sigma_name,
            auto_rows,
        );
        if self.canvas.nodes[index].height > before {
            let node = &self.canvas.nodes[index];
            self.spatial.update(index, node);
        }
    }

    /// Динамический перерасчёт `MeasuredReserveFn` (T9-сессия 2026-09-24):
    /// отпечаток контента ноды для хеш-гейта. Входы — те же, что у
    /// [`ensure_reserve_at`]/[`ensure_spill_rows_reserve`] (I-2: мера =
    /// рендер), БЕЗ mode-тогглов (`block_collapsed`/`desc_expanded`) —
    /// они на growth-only пути (I-6). Когда хеш меняется, контент
    /// факт-изменился → [`refit_node_to_content`] запускает реальный замер
    /// с усадкой/ростом; прежний хеш → no-op (замер не запускается, перф).
    fn content_height_hash(&self, index: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let node = &self.canvas.nodes[index];
        // display_body_text (со вставленными проливаниями — FR-029):
        // именно этот текст шейпится рендером, его длина/содержание
        // определяют высоту стека.
        let display = display_body_text(node, &self.param_spills);
        display.hash(&mut hasher);
        // formula_lines: индексы строк с исходами (Numi-стиль) — влияют
        // на mono-флаг, переносы, число рядов. Что-if меняет результаты
        // строк → формула_lines меняется → хеш меняется → рефит.
        let formula_lines: Vec<usize> = self
            .expr_line_results
            .get(&node.id)
            .map(|lines| formula_line_indices(lines))
            .unwrap_or_default();
        formula_lines.hash(&mut hasher);
        // desc — зона описания (canvasdesk.desc → манифест шаблона);
        // длина/наличие влияет на высоту (кламп 2 строки + экспандер).
        let desc = self.node_desc_text(index);
        desc.hash(&mut hasher);
        // auto_rows — число авто-строк приёмника (FR-050 Р-4); влияет
        // на +1 ряд подписи зоны + сами авто-строки.
        let auto_rows = self
            .auto_rows
            .get(&node.id)
            .map(|rows| rows.len())
            .unwrap_or(0);
        auto_rows.hash(&mut hasher);
        // footer_reserve — есть ли футер результата; что-if/правка могут
        // убрать/вернуть итог → футер появляется/исчезает → высота.
        let footer_reserve = self.node_shows_result_footer(index);
        footer_reserve.hash(&mut hasher);
        // sigma_name — Σ-строка после расчётных строк; зависит от
        // footer_reserve + наличия расчётных строк (см. ensure_reserve_at).
        let params = formula_lines
            .iter()
            .copied()
            .filter(|&i| {
                display.lines().nth(i).is_some_and(|line| {
                    matches!(
                        canvas_core::expr::line_kind(line),
                        canvas_core::expr::NumiLineKind::Assignment { .. }
                    )
                })
            })
            .count();
        let sigma_name = if footer_reserve && formula_lines.len() > params {
            format!("Σ {}", node.sigma_row_name())
        } else {
            String::new()
        };
        sigma_name.hash(&mut hasher);
        // width — пользовательский ресайз меняет ширину → переносы → высота.
        node.width.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    /// Динамический перерасчёт `MeasuredReserveFn` (T9-сессия 2026-09-24):
    /// вызывается из [`recompute_flow`] для нод с изменившимся хешем
    /// контента. Реальный замер (`refit_to_measured_content`) — с усадкой
    /// и ростом; mode-тогглы (`block_collapsed`/`desc_expanded`) сюда НЕ
    /// попадают (они на growth-only `ensure_reserve_at`, I-6).
    ///
    /// Входы те же, что у `ensure_spill_rows_reserve` (auto_rows →
    /// spill-prefixed display + сдвинутые formula_lines + auto_rows count)
    /// — I-2: мера = рендер. `desc_expanded` берётся из текущего
    /// состояния — если пользователь раскрыл описание (mode-toggle,
    /// growth-only), последующий content-change замерит с раскрытым
    /// описанием (усадка возможна, если новый контент короче).
    pub(crate) fn refit_node_to_content(&mut self, index: usize) -> bool {
        let node_id = match self.canvas.nodes.get(index) {
            Some(n) => n.id.clone(),
            None => return false,
        };
        let formula_lines: Vec<usize> = self
            .expr_line_results
            .get(&node_id)
            .map(|lines| formula_line_indices(lines))
            .unwrap_or_default();
        let display_body = display_body_text(&self.canvas.nodes[index], &self.param_spills);
        let desc = self.node_desc_text(index);
        let desc_expanded = self.desc_expanded.contains(&node_id);
        let footer_reserve = self.node_shows_result_footer(index);
        let params = formula_lines
            .iter()
            .copied()
            .filter(|&i| {
                display_body.lines().nth(i).is_some_and(|line| {
                    matches!(
                        canvas_core::expr::line_kind(line),
                        canvas_core::expr::NumiLineKind::Assignment { .. }
                    )
                })
            })
            .count();
        let sigma_name = if footer_reserve && formula_lines.len() > params {
            format!("Σ {}", self.canvas.nodes[index].sigma_row_name())
        } else {
            String::new()
        };
        let auto_rows = self
            .auto_rows
            .get(&node_id)
            .map(|rows| rows.len())
            .unwrap_or(0);
        // Для нод с авто-строками — тот же ввод, что у
        // `ensure_spill_rows_reserve`: spill-prefixed display + сдвинутые
        // formula_lines + auto_rows count. Иначе — как `ensure_reserve_at`.
        let (display, formula_lines_arg, auto_rows_arg) = if auto_rows > 0 {
            let rows = self.auto_rows.get(&node_id).cloned().unwrap_or_default();
            let prefix: Vec<String> = rows.iter().map(|r| r.display_text()).collect();
            let display = if display_body.is_empty() {
                prefix.join("\n")
            } else {
                format!("{}\n{}", prefix.join("\n"), display_body)
            };
            let shift = prefix.len();
            let mut fl: Vec<usize> = formula_lines
                .into_iter()
                .map(|i| i + shift)
                .chain(0..shift)
                .collect();
            fl.sort_unstable();
            (display, fl, auto_rows)
        } else {
            (display_body, formula_lines, 0)
        };
        let before = self.canvas.nodes[index].height;
        let changed = crate::measure::refit_to_measured_content(
            &mut self.canvas.nodes[index],
            &display,
            &formula_lines_arg,
            desc.as_deref(),
            desc_expanded,
            footer_reserve,
            &sigma_name,
            auto_rows_arg,
        );
        if changed && (self.canvas.nodes[index].height - before).abs() >= 1.0 {
            let node = &self.canvas.nodes[index];
            self.spatial.update(index, node);
        }
        changed
    }

    /// Динамический перерасчёт `MeasuredReserveFn`: сброс состояния хешей
    /// контента — вызывается приложением при undo/redo (`restore_canvas`).
    /// Снапшот восстановил канвас (включая высоты нод), но
    /// `content_height_state` хранит пары (hash, height) ДО отката —
    /// следующий `recompute_flow` посчитал бы хеш изменившимся и запустил
    /// refit, испортив восстановленные высоты. Сброс = «первая встреча»:
    /// доверяем восстановленным высотам, хеши перевычисляются на ближайшем
    /// `recompute_flow` без refit.
    ///
    /// ВНИМАНИЕ: при нормальной работе вызывать НЕ нужно — механизм
    /// самовосстанавливается через проверку `height_at_measurement` (если
    /// текущая высота не совпадает с сохранённой — канвас заменён, доверяем
    /// текущей). Этот метод — для явного сброса (например, при загрузке
    /// нового файла).
    pub fn reset_content_height_state(&mut self) {
        self.content_height_state.clear();
    }

    /// Динамический перерасчёт `MeasuredReserveFn`: проверить хеш контента
    /// ноды, и если он изменился — запустить [`refit_node_to_content`]
    /// (реальный замер с усадкой/ростом). Вызывается из `recompute_flow`
    /// ПОСЛЕ growth-only `apply_result_reserve` — рост уже учтён, здесь
    /// ловим усадку (и подтверждаем рост, если хеш изменился). no-op при
    /// прежнем хеше (перф: реальный шейпинг не запускается).
    ///
    /// Самовосстановление: если текущая высота ноды не совпадает с
    /// `height_at_measurement` из сохранённого состояния — канвас был
    /// заменён внешне (undo/redo/прямая замена `scene.canvas = ...`).
    /// Доверяем текущей высоте, обновляем хеш без замера. Иначе undo/redo
    /// ломали бы восстановленные высоты: снапшот вернул height=120, но хеш
    /// хранит «users=1200» → следующий recompute_flow запустил бы refit и
    /// усел ноду до актуального контента.
    ///
    /// Первая встреча ноды (`prev = None`): доверяем текущей высоте
    /// (user-set/default/MCP-заданной), запоминаем (hash, height). Иначе
    /// свежесозданная нода с дефолтной высотой 120 уселась бы до min —
    /// UX-регрессия. Усадка разрешена только при фактическом content-change
    /// (правка текста, what-if, spill, sigma) — когда пользователь меняет
    /// именно контент, а не размеры.
    fn refit_node_to_content_if_changed(&mut self, index: usize) -> bool {
        let node_id = match self.canvas.nodes.get(index).map(|n| n.id.clone()) {
            Some(id) => id,
            None => return false,
        };
        let new_hash = self.content_height_hash(index);
        let current_height = self.canvas.nodes[index].height;
        let prev = self.content_height_state.get(&node_id).copied();
        match prev {
            None => {
                // Первая встреча: доверяем текущей высоте, запоминаем.
                self.content_height_state
                    .insert(node_id, (new_hash, current_height));
                false
            }
            Some((prev_hash, prev_height))
                if prev_hash == new_hash && (prev_height - current_height).abs() < 1.0 =>
            {
                // Контент прежний И высота та же — no-op (перф).
                false
            }
            Some((prev_hash, prev_height))
                if prev_hash == new_hash && (prev_height - current_height).abs() >= 1.0 =>
            {
                // Хеш тот же, но высота другая — канвас заменён (undo/redo
                // вернул ту же формулу, но другую высоту). Доверяем текущей,
                // обновляем height_at_measurement.
                self.content_height_state
                    .insert(node_id, (new_hash, current_height));
                false
            }
            Some((_, prev_height)) if (prev_height - current_height).abs() >= 1.0 => {
                // Хеш изменился, но высота тоже другая — канвас заменён
                // (undo/redo). Доверяем текущей высоте, обновляем хеш.
                self.content_height_state
                    .insert(node_id, (new_hash, current_height));
                false
            }
            Some(_) => {
                // Хеш изменился, высота та же — контент факт-изменился.
                // Реальный замер с усадкой/ростом.
                let changed = self.refit_node_to_content(index);
                let new_height = self.canvas.nodes[index].height;
                self.content_height_state
                    .insert(node_id, (new_hash, new_height));
                changed
            }
        }
    }

    /// FR-014: тогл типа потока связи (Value ↔ Control) из палитры
    /// (ПКМ по связи) — единая точка с MCP `flow_set_kind` по инвариантам:
    /// undo-шаг «до» (FR-006), mark_dirty, живой пересчёт downstream.
    /// Возвращает `Ok(true)` — применено; `Ok(false)` — no-op (связи нет
    /// или тип уже такой, шаг не копится); `Err(участники)` — тогл в Value
    /// замкнул бы цикл value-рёбер, отклонён (UI показывает toast).
    /// Правка 3: раньше мутация применялась к клону-снимку, живой канвас
    /// не менялся — переключатель в интерфейсе не работал (MCP работал).
    pub fn toggle_edge_flow(
        &mut self,
        edge_index: usize,
        kind: FlowKind,
    ) -> Result<bool, Vec<String>> {
        let Some(edge) = self.canvas.edges.get(edge_index) else {
            return Ok(false); // связи нет — no-op
        };
        if edge.flow_kind() == kind {
            return Ok(false); // no-op — шаг не копится
        }
        if kind == FlowKind::Value
            && canvas_core::creates_value_cycle(&self.canvas, &edge.from_node, &edge.to_node)
        {
            return Err(
                canvas_core::value_path(&self.canvas, &edge.to_node, &edge.from_node)
                    .unwrap_or_default(),
            );
        }
        // Снимок «до» мутации (FR-006), затем мутация ЖИВОГО канваса
        let snapshot = self.canvas.clone();
        if let Some(edge) = self.canvas.edges.get_mut(edge_index) {
            edge.set_flow_kind(kind);
        }
        if self.canvas != snapshot {
            self.push_undo(snapshot);
            self.mark_dirty();
            // Downstream мог потерять/обрести входы — живой пересчёт
            self.recompute_flow();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// FR-013 (правка 2): пересчитать результаты ноды. Текст вычисляется
    /// ПОСТРОЧНО (Numi-стиль, `expr::eval_lines`): общее окружение,
    /// результат каждой формульной строки. Если построчных результатов нет,
    /// а `canvasdesk.expr` задан (MCP) — программный итог в футере карточки.
    /// Env пустой: ИЗОЛИРОВАННЫЙ расчёт — фолбэк recompute_flow при цикле
    /// value-рёбер из чужих файлов и проверка FR-013-семантики в тестах;
    /// живой путь приложения — [`SceneState::recompute_flow`].
    pub fn recompute_expr(&mut self, node_id: &str) {
        let Some(node) = self.canvas.node(node_id) else {
            self.expr_line_results.remove(node_id);
            self.expr_results.remove(node_id);
            return;
        };
        let text = node.text.clone().unwrap_or_default();
        let line_results = expr::eval_lines(&text);
        if line_results.iter().any(Option::is_some) {
            // Построчные результаты есть — программный итог не показывается
            // (его значение — последняя формульная строка)
            self.expr_line_results
                .insert(node_id.to_owned(), line_results);
            self.expr_results.remove(node_id);
            return;
        }
        self.expr_line_results.remove(node_id);
        let outcome = match node.expr() {
            // Явная формула (MCP `node_edit { expr }`): ошибки показываются
            Some(formula) => match expr::parse(formula) {
                Ok(parsed) => match expr::eval(&parsed, &ExprEnv::empty()) {
                    Ok(value) => Some(ExprOutcome::Ok(value)),
                    Err(err) => Some(ExprOutcome::Err(err.to_string())),
                },
                Err(err) => Some(ExprOutcome::Err(err.to_string())),
            },
            None => None,
        };
        match outcome {
            Some(outcome) => {
                self.expr_results.insert(node_id.to_owned(), outcome);
            }
            None => {
                self.expr_results.remove(node_id);
            }
        }
    }

    /// FR-013: изолированный пересчёт формул всех нод (фолбэк при цикле
    /// value-рёбер; без учёта потока). undo/redo — см. recompute_flow.
    pub fn recompute_all_expr(&mut self) {
        self.expr_results.clear();
        self.expr_line_results.clear();
        let ids: Vec<String> = self
            .canvas
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect();
        for id in ids {
            self.recompute_expr(&id);
        }
    }

    pub fn load_or_seed(path: PathBuf) -> Self {
        Self::load_or_seed_with_storage(path, Arc::new(FsCanvasStorage))
    }

    /// Загрузить канвас с явным хранилищем (M8/W3): нет файла — стартовый;
    /// ошибка записи стартового — warn, приложение не падает. Web (W6)
    /// подставит FS Access/OPFS-хранилище.
    pub fn load_or_seed_with_storage(path: PathBuf, storage: Arc<dyn CanvasStorage>) -> Self {
        let canvas = match storage.load(&path) {
            Ok(canvas) => {
                tracing::info!(path = %path.display(), nodes = canvas.nodes.len(), "канвас загружен");
                canvas
            }
            Err(err) => {
                tracing::info!(path = %path.display(), %err, "создаю стартовый канвас");
                let canvas = seed_canvas();
                if let Err(err) = storage.save(&canvas, &path) {
                    tracing::warn!(%err, "не удалось сохранить стартовый канвас");
                }
                canvas
            }
        };
        Self::with_storage(canvas, path, storage)
    }

    /// Переместить ноду: модель + инкрементальное обновление spatial index (T5).
    pub fn move_node(&mut self, index: usize, x: f32, y: f32) {
        if let Some(node) = self.canvas.nodes.get_mut(index) {
            node.x = x;
            node.y = y;
            self.spatial.update(index, node);
        }
    }

    /// Каталог .canvas-файла: база для относительных путей нод (конвенция
    /// JSON Canvas) и для директорий вотчера (T10).
    pub fn canvas_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// Абсолютный путь файловой ноды: относительные резолвятся от каталога
    /// .canvas-файла (конвенция JSON Canvas); shell-API требуют абсолютных путей
    /// (SHCreateItemFromParsingName возвращает E_INVALIDARG на относительных).
    /// Логика — в canvas_core::resolve_node_path (единый источник, T10).
    pub fn resolve_file_path(&self, file: &str) -> PathBuf {
        canvas_core::resolve_node_path(file, &self.canvas_dir())
    }

    pub fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    /// Сохранить, если правки висят дольше debounce (SPEC §9). Возвращает true при записи.
    pub fn autosave_if_due(&mut self) -> bool {
        let due = self
            .dirty_since
            .is_some_and(|since| since.elapsed() >= AUTOSAVE_DEBOUNCE);
        if !due {
            return false;
        }
        self.save_now()
    }

    pub fn save_now(&mut self) -> bool {
        self.dirty_since = None;
        match self.storage.save(&self.canvas, &self.path) {
            Ok(()) => {
                tracing::info!(path = %self.path.display(), "канвас сохранён");
                true
            }
            Err(err) => {
                tracing::error!(%err, "ошибка сохранения канваса");
                false
            }
        }
    }

    /// Зафиксировать снапшот «до» действия (FR-006): вызывать ДО мутации.
    /// Новое действие обнуляет ветку redo; глубина — UNDO_LIMIT с
    /// вытеснением старейшего.
    pub fn push_undo(&mut self, snapshot: Canvas) {
        self.redo_stack.clear();
        self.undo_stack.push_back(snapshot);
        self.undo_tags.push_back(self.pending_undo_tag.take());
        while self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.pop_front();
            self.undo_tags.pop_front();
        }
    }

    /// PRD-0007 (FR-048 X4, AC-5.3): пометить следующий undo-снапшот тегом
    /// (вызывается ДО `push_undo`). Тег — класс действия (сейчас только
    /// "autolink_batch"); любой следующий `push_undo` без `set_undo_tag`
    /// кладёт `None` — тег естественным образом устаревает.
    pub fn set_undo_tag(&mut self, tag: &'static str) {
        self.pending_undo_tag = Some(tag);
    }

    /// PRD-0007 (FR-048 X4, AC-5.3): тег ВЕРХНЕГО undo-снапшота (без
    /// извлечения). `None` — история пуста или верхняя запись без тега.
    pub fn peek_undo_tag(&self) -> Option<&'static str> {
        self.undo_tags.back().and_then(|tag| *tag)
    }

    /// Состояние «до» последнего действия (FR-006, Ctrl+Z): pop undo-стека,
    /// текущая модель уходит в redo. None — история пуста.
    pub fn take_undo(&mut self) -> Option<Canvas> {
        let before = self.undo_stack.pop_back()?;
        self.undo_tags.pop_back();
        self.redo_stack.push(self.canvas.clone());
        Some(before)
    }

    /// Отменённое состояние (FR-006, Ctrl+Y / Ctrl+Shift+Z): pop redo-стека,
    /// текущая модель возвращается в undo. None — возвратить нечего.
    pub fn take_redo(&mut self) -> Option<Canvas> {
        let after = self.redo_stack.pop()?;
        self.undo_stack.push_back(self.canvas.clone());
        Some(after)
    }
}

#[cfg(test)]
mod reserve_tests {
    use super::*;
    use crate::measure::install_measured_reserve;
    use canvas_core::Node;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Параллельные тесты меняют глобальный уровень 2 (install) — сериализуем.
    static INSTALL_LOCK: Mutex<()> = Mutex::new(());

    fn scene_with(node: Node) -> SceneState {
        let mut canvas = Canvas::default();
        canvas.nodes.push(node);
        SceneState::new(canvas, PathBuf::from("target/tmp/fr067-reserve.canvas"))
    }

    /// FR-069 (этап F): ранний выход снят — нода БЕЗ футера результата
    /// (обычный Numi-лист с построчными результатами) растёт под тело;
    /// резерв футера ей не добавляется (нет «пустого хвоста»).
    #[test]
    fn ensure_reserve_at_fits_body_without_footer() {
        let _guard = INSTALL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        // Уровень 2 — отключаем (None): работает консервативная оценка,
        // детерминированная и без шрифтов (wasm-путь).
        install_measured_reserve(
            |text, width, lines, desc, expanded, footer, sigma, auto_rows| {
                crate::measure::estimated_result_reserve_height(
                    text, width, lines, desc, expanded, footer, sigma, auto_rows,
                )
            },
        );
        let mut node = Node::text(
            "n1",
            format!("{} = 5\n{} = 6", "a".repeat(30), "b".repeat(30)),
            0.0,
            0.0,
        );
        node.width = 260.0;
        node.height = 80.0; // занижено — тело не влезает
        let mut scene = scene_with(node);
        // Построчные результаты есть, шаблона нет → футера нет (правило
        // node_shows_result_footer), но подгонка тела обязана сработать.
        scene.recompute_flow();
        scene.ensure_reserve_at(0);
        let grown = scene.canvas.nodes[0].height;
        assert!(grown > 80.0, "нода без футера выросла под тело: {grown}");
        // Высота НЕ включает резерв футера: ровно как оценка с footer=false
        let expected = crate::measure::estimated_result_reserve_height(
            &display_body_text(&scene.canvas.nodes[0], &std::collections::HashMap::new()),
            scene.canvas.nodes[0].width,
            &crate::measure::formula_line_indices(
                scene
                    .expr_line_results
                    .get("n1")
                    .map(|l| l.as_slice())
                    .unwrap_or(&[]),
            ),
            "",
            false,
            false,
            "",
            0,
        );
        assert!(
            (grown - expected).abs() < 1.0,
            "рост без «пустого хвоста» футера: {grown} ≈ {expected}"
        );
    }

    /// FR-069 (этап F): тоггл раскрытия описания — уровень 2 знает
    /// состояние (desc_expanded): раскрытие растит высоту сразу,
    /// свёртывание её не уменьшает (I-6 growth-only).
    #[test]
    fn ensure_reserve_at_respects_desc_expanded() {
        let _guard = INSTALL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        install_measured_reserve(
            |text, width, lines, desc, expanded, footer, sigma, auto_rows| {
                let base = crate::measure::estimated_result_reserve_height(
                    text, width, lines, desc, expanded, footer, sigma, auto_rows,
                );
                if expanded {
                    base + 60.0 // имитация полного описания (3 ряда)
                } else {
                    base
                }
            },
        );
        let mut node = Node::text("n2", "rps = 800 rps", 0.0, 0.0);
        node.width = 260.0;
        node.height = 100.0; // занижено — кламп-оценка тоже растит
        let mut scene = scene_with(node);
        scene.recompute_flow();
        scene.ensure_reserve_at(0);
        let clamped = scene.canvas.nodes[0].height;
        if let Some(n) = scene.canvas.nodes.get_mut(0) {
            n.set_desc(Some("длинное описание ноды для раскрытия".into()));
        }
        scene.toggle_desc_expanded("n2");
        scene.ensure_reserve_at(0);
        let expanded_h = scene.canvas.nodes[0].height;
        assert!(
            expanded_h >= clamped + 40.0,
            "раскрытие описания выросло: {expanded_h} ≥ {clamped} + 40"
        );
        scene.toggle_desc_expanded("n2");
        scene.ensure_reserve_at(0);
        assert_eq!(
            scene.canvas.nodes[0].height, expanded_h,
            "свёртывание не усаживает (I-6 growth-only)"
        );
    }

    /// Динамический перерасчёт `MeasuredReserveFn` (T9-сессия 2026-09-24):
    /// правка текста ноды (content-change) → `recompute_flow` →
    /// `refit_node_to_content` ДОПУСКАЕТ усадку (в отличие от growth-only
    /// `ensure_reserve_at`). Сценарий: нода с длинным текстом → высота
    /// выросла; правка в короткий текст → высота УСЕЛАСЬ.
    #[test]
    fn refit_node_to_content_shrinks_on_text_edit() {
        let _guard = INSTALL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        install_measured_reserve(
            |text, width, lines, desc, expanded, footer, sigma, auto_rows| {
                crate::measure::estimated_result_reserve_height(
                    text, width, lines, desc, expanded, footer, sigma, auto_rows,
                )
            },
        );
        // Длинный текст (5 формул) → нода вырастет под контент.
        let long_text = format!(
            "{} = 1\n{} = 2\n{} = 3\n{} = 4\n{} = 5",
            "a".repeat(20),
            "b".repeat(20),
            "c".repeat(20),
            "d".repeat(20),
            "e".repeat(20)
        );
        let mut node = Node::text("n1", long_text.clone(), 0.0, 0.0);
        node.width = 260.0;
        node.height = 60.0; // занижено — вырастет
        let mut scene = scene_with(node);
        scene.recompute_flow();
        let grown = scene.canvas.nodes[0].height;
        assert!(grown > 60.0, "нода выросла под длинный контент: {grown}");

        // Правка в короткий текст (1 формула) → recompute_flow →
        // refit_node_to_content УСЕЛА ноду (content-change, не mode-toggle).
        scene.canvas.nodes[0].text = Some("x = 1".to_owned());
        scene.canvas.nodes[0].set_expr(crate::split_formula_lines("x = 1"));
        scene.recompute_flow();
        let shrunken = scene.canvas.nodes[0].height;
        assert!(
            shrunken < grown - 50.0,
            "нода уселась после правки (content-change): {shrunken} < {grown} - 50"
        );
    }

    /// Динамический перерасчёт: самовосстановление при undo/redo. Сценарий:
    /// нода с высотой H1 → правка (height → H2) → откат канваса (height → H1)
    /// → recompute_flow → высота остаётся H1 (НЕ пересчитывается, т.к.
    /// текущая высота не совпадает с сохранённой H2 → канвас заменён,
    /// доверяем текущей). Без самовосстановления undo ломал бы высоты.
    #[test]
    fn refit_node_to_content_self_heals_on_canvas_restore() {
        let _guard = INSTALL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        install_measured_reserve(
            |text, width, lines, desc, expanded, footer, sigma, auto_rows| {
                crate::measure::estimated_result_reserve_height(
                    text, width, lines, desc, expanded, footer, sigma, auto_rows,
                )
            },
        );
        let mut node = Node::text("n1", "x = 1".to_owned(), 0.0, 0.0);
        node.width = 260.0;
        node.height = 120.0; // user-set
        let mut scene = scene_with(node);
        scene.recompute_flow();
        let initial_h = scene.canvas.nodes[0].height;
        // Первая встреча → доверяем 120, запоминаем (hash, 120).

        // Правка в длинный текст → height растёт.
        let long_text = format!(
            "{} = 1\n{} = 2\n{} = 3\n{} = 4",
            "a".repeat(20),
            "b".repeat(20),
            "c".repeat(20),
            "d".repeat(20)
        );
        let saved_canvas = scene.canvas.clone(); // снапшот «до» для undo
        scene.canvas.nodes[0].text = Some(long_text.clone());
        scene.canvas.nodes[0].set_expr(crate::split_formula_lines(&long_text));
        scene.recompute_flow();
        let grown_h = scene.canvas.nodes[0].height;
        assert!(grown_h > initial_h, "нода выросла: {grown_h} > {initial_h}");

        // Undo: восстанавливаем канвас (включая height = initial_h) —
        // симулируем model-level undo (как в integration_explain_x6).
        // Без самовосстановления следующий recompute_flow запустил бы
        // refit (хеш изменился: long_text → "x = 1") и усел ноду.
        scene.canvas = saved_canvas;
        scene.recompute_flow();
        let restored_h = scene.canvas.nodes[0].height;
        assert_eq!(
            restored_h, initial_h,
            "undo восстановил высоту (самовосстановление): {restored_h} == {initial_h}"
        );
    }

    /// Динамический перерасчёт: I-6 для mode-тогглов сохранён. Mode-тогглы
    /// (block_collapsed/desc_expanded) идут через `ensure_reserve_at`
    /// (growth-only), НЕ через `refit_node_to_content`. Сценарий: раскрыть
    /// описание → высота растёт; свернуть → высота НЕ усаживается.
    #[test]
    fn refit_node_to_content_preserves_i6_for_mode_toggles() {
        let _guard = INSTALL_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        install_measured_reserve(
            |text, width, lines, desc, expanded, footer, sigma, auto_rows| {
                crate::measure::estimated_result_reserve_height(
                    text, width, lines, desc, expanded, footer, sigma, auto_rows,
                )
            },
        );
        let mut node = Node::text("n2", "x = 1".to_owned(), 0.0, 0.0);
        node.width = 260.0;
        node.height = 120.0;
        // Длинное описание — заведомо больше клампа (2 строки), чтобы
        // раскрытие растянуло высоту (кламп → +экспандер → раскрыто → +ряды).
        node.set_desc(Some(
            "Очень длинное описание ноды для проверки раскрытия: оно заведомо \
             не помещается в две строки клампа и потому сворачивается с \
             аффордансом «⋯ целиком ▾» — раскрытие растит стек на ряды."
                .into(),
        ));
        let mut scene = scene_with(node);
        scene.recompute_flow();
        let clamped_h = scene.canvas.nodes[0].height;

        // Раскрыть описание → growth-only растит высоту.
        scene.toggle_desc_expanded("n2");
        scene.ensure_reserve_at(0);
        let expanded_h = scene.canvas.nodes[0].height;
        assert!(
            expanded_h >= clamped_h + 20.0,
            "раскрытие растит высоту: {expanded_h} >= {clamped_h} + 20"
        );

        // Свернуть описание → I-6: высота НЕ усаживается.
        scene.toggle_desc_expanded("n2");
        scene.ensure_reserve_at(0);
        assert_eq!(
            scene.canvas.nodes[0].height, expanded_h,
            "свёртывание не усаживает (I-6 growth-only для mode-тогглов)"
        );
    }
}
