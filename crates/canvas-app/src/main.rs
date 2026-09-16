//! canvas-app — приложение: event loop, команды, UI-состояние, main().

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Чистые UI-helpers (геометрия, hit-тесты, меню, двойной клик) — единый
// источник в библиотеке, здесь только платформенно-зависимое состояние.
use canvas_app::hints_ui;
use canvas_app::palette::{
    color_to_rgba, icon_quads, icon_text, palette_bar_size, palette_groups, palette_hit,
    palette_layout, palette_origin, template_update_group, PaletteAction, PaletteHit, PaletteHover,
    PaletteLayout, PaletteTarget, PAL_ICON,
};
use canvas_app::settings_ui::{
    apply_dropdown_value, dropdown_item_at, dropdown_layout, dropdown_options, panel_layout,
    row_at, row_kind, DropdownState, PanelEntry, RowKind, SettingsRow, DROPDOWN_MARGIN,
    DROPDOWN_ROW_H,
};
use canvas_app::template_ui;
use canvas_app::template_ui::{
    panel_layout as template_panel_layout, panel_rows as template_panel_rows,
    row_of_ordinal as template_row_of_ordinal, split_two_lines, PanelRow, WheelHit,
};
use canvas_app::ui::{
    button_rect, canvas_menu_label, drag_origins, focus_seed_of, hotkeys_panel_rect,
    in_resize_corner, menu_item_at_for, menu_item_rect, menu_rect_for, next_free_id, nodes_in_rect,
    panel_rect, paste_nodes, plan_group_around, plan_group_around_nodes, plan_group_at,
    point_in_rect, reassign_ids, rubber_band_rect, select_node_hit, submenu_item_at,
    submenu_origin_next_to, submenu_rect, theme_button_rect, toggle_selection_with_primary,
    CanvasMenuItem, ContextMenu, DoubleClick, DragState, EdgeDrag, PastePlacement, Submenu,
    SubmenuEntry, CANVAS_MENU_ITEMS, DUPLICATE_OFFSET, MENU_LABEL_X, MENU_PADDING, MENU_WIDTH,
    MIN_NODE_HEIGHT, MIN_NODE_WIDTH, PANEL_HINT_HEIGHT, PANEL_PADDING, SELECT_DRAG_THRESHOLD,
};
use canvas_core::expr::{self, Env as ExprEnv, ExprLineResults, ExprOutcome, ExprResults};
use canvas_core::flow::{self, FlowKind, FlowOutputs};
use canvas_core::{
    apply_file_events, edge_at, focus_set, nearest_side, path_matches, port_at, resolve_node_path,
    watched_dirs, Canvas, Edge, FileEvent, FocusSeed, GridStyle, Node, NodeChange, NodeKind,
    Settings, Side, SpatialIndex, Theme, ThumbnailProvider,
};
use canvas_render::animate::{
    ease_out_cubic, focus_fade, focus_pulse, pulse_alpha, Flight, FLIGHT_DURATION_MS,
    FOCUS_FADE_MS, FOCUS_PULSE_MS,
};
use canvas_render::camera::Vec2;
use canvas_render::cards::{template_icon_quads, CardInstance, FocusView, HEADER_HEIGHT};
use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand,
};
use canvas_render::minimap::{Minimap, MINIMAP_H, MINIMAP_W};
use canvas_render::search_ui::{
    layout as search_layout, scan_scene, PanelAction, SceneEntry, SearchInput, SearchPanel,
    SearchRow,
};
use canvas_render::text::{
    body_area, LineErrorHit, OverlayText, ScreenText, TextAlign, BODY_LINE_HEIGHT, BODY_PADDING,
    BODY_TOP_GAP, RESULT_LINE_HEIGHT,
};
use canvas_render::ThemeColors;
use canvas_render::{Camera, Color, FrameMeter, FrameOverlay, FrameStats, SceneView, Selection};
use canvas_shell::{
    Priority, SearchCommand, SearchEvent, SearchHit, SearchService, ThumbService, WatchService,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};
// Атрибуты окна Windows: отключение своего IDropTarget у winit (T9, план §3)
#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;

/// Множитель зума на одну строку колеса мыши (Ctrl+колесо, SPEC §8).
const ZOOM_STEP_PER_LINE: f32 = 1.1;
/// Пикселей панорамирования на строку колеса без Ctrl (скролл тачпада).
const PAN_PX_PER_LINE: f32 = 40.0;
/// Debounce автосейва (SPEC §9).
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(2);
/// Глубина истории undo (FR-006): не менее 50 последних действий (запрос
/// пользователя «не менее 50»); старейшие шаги вытесняются.
const UNDO_LIMIT: usize = 50;
/// Ширина клип-бокса тултипа битой ссылки (T10): длинный путь переносится
/// на границы этой области, экран не покидает.
const TOOLTIP_WIDTH: f32 = 380.0;

/// Debounce запроса поиска (T14, план §3).
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
/// Лимит строк FTS-запроса (T14): в 2 р больше видимых — запас под скролл.
const SEARCH_RESULTS_LIMIT: usize = 16;

/// Screen-space текст с владеемой строкой (панель настроек): промежуточное
/// представление, конвертируется в `ScreenText` на кадр рендера.
struct OwnedScreenText {
    text: String,
    origin: [f32; 2],
    width: f32,
    font_size: f32,
    color: Color,
    /// Выравнивание в области `width` (иконки кнопок — по центру).
    align: TextAlign,
}

/// Буфер обмена ОС (T7, arboard): ошибки — warn, редактирование не ломается.
struct Clipboard(Option<arboard::Clipboard>);

impl Clipboard {
    fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(clipboard) => Self(Some(clipboard)),
            Err(err) => {
                tracing::warn!(%err, "буфер обмена недоступен");
                Self(None)
            }
        }
    }

    fn set(&mut self, text: String) {
        if let Some(clipboard) = &mut self.0 {
            if let Err(err) = clipboard.set_text(text) {
                tracing::warn!(%err, "не удалось записать в буфер обмена");
            }
        }
    }

    fn get(&mut self) -> Option<String> {
        self.0
            .as_mut()
            .and_then(|clipboard| match clipboard.get_text() {
                Ok(text) => Some(text),
                Err(err) => {
                    tracing::warn!(%err, "не удалось прочитать буфер обмена");
                    None
                }
            })
    }
}

/// Преобразование [x0, y0, x1, y1] → [x, y, w, h] (point_in_rect-конвенция).
fn rect_xywh(rect: [f32; 4]) -> [f32; 4] {
    [
        rect[0],
        rect[1],
        (rect[2] - rect[0]).max(0.0),
        (rect[3] - rect[1]).max(0.0),
    ]
}

/// Заголовок ноды для поиска/результатов (T14): имя файла или текст заметки.
fn node_title(node: &Node) -> &str {
    if let Some(file) = node.file.as_ref() {
        return Path::new(file)
            .file_name()
            .map(|name| name.to_str().unwrap_or(file))
            .unwrap_or(file);
    }
    node.text.as_deref().unwrap_or("Заметка")
}

/// Полный текст ноды для in-memory поиска (T14): содержимое заметки.
fn node_text(node: &Node) -> &str {
    node.text.as_deref().unwrap_or("")
}

/// Подзаголовок строки результата (T14): «заметка» или родительский каталог.
fn node_subtitle(node: &Node) -> &str {
    if node.file.is_some() {
        "файл"
    } else {
        "заметка"
    }
}

/// Подзаголовок FTS-хита (T14): имя родительского каталога пути.
fn hit_subtitle(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "файл".to_owned())
}

/// FR-013: извлечь формулу из текста заметки (смешанный редактор — решение
/// открытого вопроса дизайна): строки, начинающиеся с `=` (после пропуска
/// пробелов), — утверждения формулы без префикса. Текст ноды остаётся
/// пользовательским описанием вместе с `=`-строками; результат живёт в
/// `canvasdesk.expr`. None — формульных строк нет (calc-режим выключен).
fn split_formula_lines(text: &str) -> Option<String> {
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

/// FR-023: авто-высота шаблонной ноды — по числу строк листа параметров:
/// шапка + тело + футер результата. Общая для GUI-инстанциации и MCP
/// `template_instantiate`: новая нода сразу влезает целиком (без
/// «подгонки правкой»). Только рост (не сжимает пользовательский размер).
fn fit_template_node_height(node: &mut Node) {
    let lines = node
        .text
        .as_deref()
        .map(|text| text.lines().count().max(1))
        .unwrap_or(1);
    let needed = HEADER_HEIGHT
        + BODY_TOP_GAP
        + lines as f32 * BODY_LINE_HEIGHT
        + BODY_PADDING
        + RESULT_LINE_HEIGHT
        + 2.0;
    if needed > node.height {
        node.height = needed;
    }
}

/// FR-020: slug из имени шаблона: латиница/цифры/дефисы, кириллица —
/// транслитерация (решение владельца: «Нагрузка» → «nagruzka»).
/// Прочие символы — дефис; сжатие подряд идущих; обрезка краёв.
fn slugify(name: &str) -> String {
    const TRANSLIT: &[(&str, &str)] = &[
        ("а", "a"),
        ("б", "b"),
        ("в", "v"),
        ("г", "g"),
        ("д", "d"),
        ("е", "e"),
        ("ё", "e"),
        ("ж", "zh"),
        ("з", "z"),
        ("и", "i"),
        ("й", "y"),
        ("к", "k"),
        ("л", "l"),
        ("м", "m"),
        ("н", "n"),
        ("о", "o"),
        ("п", "p"),
        ("р", "r"),
        ("с", "s"),
        ("т", "t"),
        ("у", "u"),
        ("ф", "f"),
        ("х", "h"),
        ("ц", "ts"),
        ("ч", "ch"),
        ("ш", "sh"),
        ("щ", "sch"),
        ("ъ", ""),
        ("ы", "y"),
        ("ь", ""),
        ("э", "e"),
        ("ю", "yu"),
        ("я", "ya"),
    ];
    let lower = name.to_lowercase();
    let mut out = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if let Some((_, latin)) = TRANSLIT.iter().find(|(c, _)| *c == ch.to_string()) {
            out.push_str(latin);
        } else if ch == ' ' || ch == '_' || ch == '-' || ch == '.' || ch == '/' {
            out.push('-');
        }
        // прочие символы (эмодзи, знаки) — пропускаются
    }
    let mut collapsed = String::new();
    for ch in out.chars() {
        if ch == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(ch);
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "custom".to_owned()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// FR-020: тип параметра по токену единицы (подсказка UI в манифесте).
fn infer_param_type(unit: Option<&str>) -> canvas_core::templates::ParamType {
    use canvas_core::templates::ParamType;
    match unit {
        Some("rps") | Some("req/s") => ParamType::Rate,
        Some("ms") | Some("s") | Some("sec") | Some("secs") | Some("min") | Some("h")
        | Some("hour") => ParamType::Time,
        Some("B") | Some("KB") | Some("MB") | Some("GB") => ParamType::Bytes,
        Some("%") => ParamType::Percent,
        Some("req") | Some("reqs") => ParamType::Count,
        _ => ParamType::Scalar,
    }
}

/// FR-020: уникальный id custom-шаблона: базовый slug; конфликт с
/// существующим id (реестр или файловая система) — суффикс `-2`, `-3`…
fn unique_custom_id(
    base: &str,
    registry: &canvas_core::templates::TemplateRegistry,
    root: &std::path::Path,
) -> String {
    let base = base.chars().take(48).collect::<String>();
    let exists = |id: &str| registry.find(id).is_some() || root.join(id).exists();
    if !exists(&base) {
        return base;
    }
    for n in 2..=1000u32 {
        let candidate = format!("{base}-{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    // Практически недостижимо — последний рубеж: метка времени
    format!(
        "{base}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    )
}

/// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки под
/// курсором (все координаты — логические px окна). Вынесено из App для
/// прямого unit-тестирования.
fn expr_error_hit_at(hits: &[LineErrorHit], cursor: [f32; 2]) -> Option<&LineErrorHit> {
    hits.iter().find(|hit| {
        let [x, y, w, h] = hit.rect;
        cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
    })
}

/// FR-014: карта результатов propagator'а → display-карта приложения
/// (`expr_results`): типизированные ошибки становятся строками для рендера.
fn outputs_to_results(outputs: &FlowOutputs) -> ExprResults {
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

/// Стартовый канвас при отсутствии файла: заметка + файловые ноды (T4).
fn seed_canvas() -> Canvas {
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

/// Состояние сцены: модель, spatial index (T5), файл, выделение и перетаскивание.
struct SceneState {
    canvas: Canvas,
    /// R-tree над AABB нод; синхронизируется при каждом изменении геометрии.
    spatial: SpatialIndex,
    path: PathBuf,
    /// Первичное выделение: нода или связь (T8) — якорь для контекстного
    /// меню, редактирования, фокуса (T23), перепривязки (CR-002).
    selected: Option<Selection>,
    /// Множественное выделение нод (CR-001): рамка drag или Ctrl/Shift+клик.
    /// Порядок — порядок добавления (клики) или индексы (рамка).
    selected_nodes: Vec<usize>,
    /// Drag ноды (T7/CR-001): захваченная нода + исходные позиции всех
    /// перемещаемых (выделение или одна + дети групп).
    dragging: Option<DragState>,
    dirty_since: Option<Instant>,
    /// История undo (FR-006): снапшоты Canvas «до» действий (push ДО
    /// мутации). VecDeque — O(1) вытеснение старейшего при переполнении.
    undo_stack: VecDeque<Canvas>,
    /// Отменённые состояния (FR-006): текущее уходит сюда при undo; новое
    /// действие обнуляет ветку redo.
    redo_stack: Vec<Canvas>,
    /// FR-013: результаты формул (`canvasdesk.expr`) по id нод —
    /// runtime-кэш (инвариант 4 FR-013: НЕ сериализуется, источник
    /// истины — формула; пересчитывается при загрузке/commit/undo).
    /// Программный итог (MCP-expr) — в футере карточки.
    expr_results: ExprResults,
    /// FR-013 (правка 2): построчные результаты текста нод (Numi-стиль) —
    /// тоже runtime-кэш (инвариант 4).
    expr_line_results: ExprLineResults,
}

impl SceneState {
    /// Обернуть готовую модель: построить spatial index.
    fn new(canvas: Canvas, path: PathBuf) -> Self {
        let spatial = SpatialIndex::build(&canvas);
        let mut scene = Self {
            canvas,
            spatial,
            path,
            selected: None,
            selected_nodes: Vec::new(),
            dragging: None,
            dirty_since: None,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            expr_results: ExprResults::new(),
            expr_line_results: ExprLineResults::new(),
        };
        // FR-013: первичный пересчёт формул при загрузке (результат не
        // хранится в .canvas — вычисляется, см. инвариант 4 FR-013);
        // FR-014: пересчёт — живой propagator графа потока
        scene.recompute_flow();
        scene
    }

    /// FR-014: живой пересчёт графа потока значений (инвариант live — в
    /// пределах одного кадра). Заполняет `expr_results` (результаты формул
    /// всех expr-нод, с входами value-рёбер) и `expr_line_results`
    /// (построчные результаты Numi-листов — строки видят входы ноды).
    /// Запускается после ЛЮБОЙ мутации формул или топологии (правка
    /// текста/формулы, рёбра, удаление нод, undo) — propagator чистый,
    /// полный пересчёт ≤1000 нод <10 мс (SPEC §6.3).
    fn recompute_flow(&mut self) {
        let outputs = match flow::propagate(&self.canvas, &HashMap::new()) {
            Ok(outputs) => outputs,
            Err(cycle) => {
                // UI и MCP блокируют создание value-циклов; сюда попадаем
                // только из чужих .canvas-файлов — деградация до изолированного
                // расчёта (FR-013) с предупреждением (правило AGENTS: фолбэк + warn)
                tracing::warn!(cycle = %cycle, "цикл value-рёбер — расчёт без потока");
                self.recompute_all_expr();
                return;
            }
        };
        self.expr_results = outputs_to_results(&outputs);
        self.expr_line_results.clear();
        for node in &self.canvas.nodes {
            let text = node.text.clone().unwrap_or_default();
            let slots = flow::inbound_slots(&self.canvas, &node.id, &outputs);
            let line_results = if slots.is_empty() {
                expr::eval_lines(&text)
            } else {
                expr::eval_lines_in(&text, &ExprEnv::with_inbound(slots))
            };
            if line_results.iter().any(Option::is_some) {
                self.expr_line_results.insert(node.id.clone(), line_results);
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
    fn toggle_edge_flow(&mut self, edge_index: usize, kind: FlowKind) -> Result<bool, Vec<String>> {
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
    fn recompute_expr(&mut self, node_id: &str) {
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
    fn recompute_all_expr(&mut self) {
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

    fn load_or_seed(path: PathBuf) -> Self {
        let canvas = match Canvas::load(&path) {
            Ok(canvas) => {
                tracing::info!(path = %path.display(), nodes = canvas.nodes.len(), "канвас загружен");
                canvas
            }
            Err(err) => {
                tracing::info!(path = %path.display(), %err, "создаю стартовый канвас");
                let canvas = seed_canvas();
                if let Err(err) = canvas.save(&path) {
                    tracing::warn!(%err, "не удалось сохранить стартовый канвас");
                }
                canvas
            }
        };
        Self::new(canvas, path)
    }

    /// Переместить ноду: модель + инкрементальное обновление spatial index (T5).
    fn move_node(&mut self, index: usize, x: f32, y: f32) {
        if let Some(node) = self.canvas.nodes.get_mut(index) {
            node.x = x;
            node.y = y;
            self.spatial.update(index, node);
        }
    }

    /// Каталог .canvas-файла: база для относительных путей нод (конвенция
    /// JSON Canvas) и для директорий вотчера (T10).
    fn canvas_dir(&self) -> PathBuf {
        self.path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    /// Абсолютный путь файловой ноды: относительные резолвятся от каталога
    /// .canvas-файла (конвенция JSON Canvas); shell-API требуют абсолютных путей
    /// (SHCreateItemFromParsingName возвращает E_INVALIDARG на относительных).
    /// Логика — в canvas_core::resolve_node_path (единый источник, T10).
    fn resolve_file_path(&self, file: &str) -> PathBuf {
        canvas_core::resolve_node_path(file, &self.canvas_dir())
    }

    fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    /// Сохранить, если правки висят дольше debounce (SPEC §9). Возвращает true при записи.
    fn autosave_if_due(&mut self) -> bool {
        let due = self
            .dirty_since
            .is_some_and(|since| since.elapsed() >= AUTOSAVE_DEBOUNCE);
        if !due {
            return false;
        }
        self.save_now()
    }

    fn save_now(&mut self) -> bool {
        self.dirty_since = None;
        match self.canvas.save_with_backup(&self.path) {
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
    fn push_undo(&mut self, snapshot: Canvas) {
        self.redo_stack.clear();
        self.undo_stack.push_back(snapshot);
        while self.undo_stack.len() > UNDO_LIMIT {
            self.undo_stack.pop_front();
        }
    }

    /// Состояние «до» последнего действия (FR-006, Ctrl+Z): pop undo-стека,
    /// текущая модель уходит в redo. None — история пуста.
    fn take_undo(&mut self) -> Option<Canvas> {
        let before = self.undo_stack.pop_back()?;
        self.redo_stack.push(self.canvas.clone());
        Some(before)
    }

    /// Отменённое состояние (FR-006, Ctrl+Y / Ctrl+Shift+Z): pop redo-стека,
    /// текущая модель возвращается в undo. None — возвратить нечего.
    fn take_redo(&mut self) -> Option<Canvas> {
        let after = self.redo_stack.pop()?;
        self.undo_stack.push_back(self.canvas.clone());
        Some(after)
    }
}

/// Пользовательские события event loop (T6): worker-потоки ThumbService
/// будят цикл через EventLoopProxy, когда готовы тамбнейлы; shell шлёт
/// события drag-drop (T9) и файлового вотчера (T10).
enum AppEvent {
    /// В канале ThumbService появились результаты — забрать и перерисовать.
    ThumbsReady,
    /// Событие drag-drop из IDropTarget (T9): Enter/Over/Leave/Drop.
    Drag(canvas_shell::dragdrop::DragEvent),
    /// Батч событий файловой системы от WatchService (T10): debounce 300 мс
    /// уже отработан в shell, здесь — применение к модели и кэшам.
    FileEvents(Vec<FileEvent>),
    /// События поискового индекса (T14): ответы worker-потока FTS5
    /// (результаты запроса / завершение индексации).
    Search(SearchEvent),
    /// События shell-монитора режима десктопа (T15): разрушение WorkerW
    /// (WinEventHook/поллинг) и смена DPI после репарентинга (R10).
    #[cfg(windows)]
    Desktop(canvas_shell::DesktopEvent),
    /// M5 (T20-F): события виджетов — WebView2-колбэки через proxy
    /// (EnvironmentReady/ControllerReady/SnapshotReady/Message) + тик
    /// таймера refresh-снапшотов. Тип кроссплатформенный: на Linux
    /// события не приходят (host нет), матчинг единообразен.
    Widget(canvas_widgets::WidgetEvent),
    /// События шины системных событий (T16): сессия (lock/unlock, R8),
    /// suspend/resume, ExplorerStarted (TaskbarCreated, R7/R11),
    /// shell-hook/clipboard (потребители T18/будущее), SHCNE-мост в
    /// конвейер T10 (корзина → brokenLink, R12).
    #[cfg(windows)]
    Shell(canvas_shell::shell_events::ShellEvent),
    /// В канале MCP pipe-сервера появились запросы (MCP-интеграция):
    /// забрать через `take_request`, ответить через responder.
    #[cfg(windows)]
    McpWake,
    /// T15-relaunch: работающий инстанс получил exit-сигнал от нового
    /// запуска (single-instance handoff, desktop/single_instance) —
    /// штатное завершение: форс-сейв сцены + восстановление иконок
    /// (shutdown). Событие шлёт exit-листенер (поток в main()) через
    /// proxy; на Windows сигналит любой повторный запуск, в т.ч.
    /// перезапуск на --desktop из меню канваса.
    /// На не-Windows листенера нет — вариант не конструируется (dead_code).
    #[cfg_attr(not(windows), allow(dead_code))]
    InstanceExit,
}

/// Превью зоны дропа (T9): план вставки от DragEnter, origin следует за
/// курсором на DragOver; живёт до Leave/Drop.
struct DropPreview {
    /// Текущий origin сетки призраков в world-координатах.
    origin: Vec2,
    /// План вставки (id/тип/позиция) — переживает без изменений до Drop.
    plan: Vec<canvas_app::ui::DropInsert>,
}

/// Модальный диалог приложения (T21-B/C: П10/П11): подтверждение
/// установки виджета drag-ом и удаления пакета. Enter — подтвердить,
/// Esc — отменить, клики по кнопкам; остальной ввод глушится.
enum AppDialog {
    /// «Установить виджет <имя> <версия>?»: источник-папка, манифест,
    /// мировая точка дропа (куда встанет нода после install), признак
    /// обновления существующего пакета (П5 — другой заголовок).
    InstallWidget {
        src: PathBuf,
        manifest: canvas_widgets::manifest::WidgetManifest,
        pos: Vec2,
        updating: bool,
    },
    /// «Удалить пакет <имя>? Ноды пакета останутся как заглушки» (П11).
    RemovePackage { widget_id: String, name: String },
    /// FR-014: «Обнаружен цикл … Создать как контрольную связь?» —
    /// value-ребро замкнуло бы цикл потока. Да — создать control-ребро,
    /// Нет — ничего. Хранит параметры будущего ребра (концы и стороны).
    EdgeCycle {
        from_node: String,
        from_side: Side,
        to_node: String,
        to_side: Side,
    },
}

impl AppDialog {
    /// Кнопки диалога (screen-space rect'ы считаются от центра окна).
    fn buttons(&self) -> [(&'static str, bool); 2] {
        // (подпись, confirm?)
        [("Да", true), ("Нет", false)]
    }

    /// Заголовок диалога. `canvas` — для имени участников цикла (FR-014).
    fn title(&self, canvas: &Canvas) -> String {
        match self {
            AppDialog::InstallWidget {
                manifest, updating, ..
            } => {
                if *updating {
                    format!("Обновить виджет {} до {}?", manifest.name, manifest.version)
                } else {
                    format!("Установить виджет {} {}?", manifest.name, manifest.version)
                }
            }
            AppDialog::RemovePackage { name, .. } => {
                format!("Удалить пакет {name}?")
            }
            // FR-014: участники цикла — путь по value-рёбрам от стока к
            // истоку + замыкающее ребро (решение открытого вопроса:
            // диалог с фолбэком на control)
            AppDialog::EdgeCycle {
                from_node, to_node, ..
            } => {
                let mut chain = canvas_core::value_path(canvas, to_node, from_node)
                    .unwrap_or_else(|| vec![to_node.clone(), from_node.clone()])
                    .join(" → ");
                chain.push_str(" → ");
                chain.push_str(from_node);
                format!("Обнаружен цикл: {chain}")
            }
        }
    }

    /// Пояснение под заголовком.
    fn body(&self) -> String {
        match self {
            AppDialog::InstallWidget { manifest, .. } => {
                let perms = if manifest.permissions.is_empty() {
                    "без разрешений".to_owned()
                } else {
                    manifest
                        .permissions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                format!("Пакет скопируется в локальную папку виджетов.\nРазрешения: {perms}.")
            }
            AppDialog::RemovePackage { .. } => {
                "Ноды этого виджета останутся на канвасе как заглушки.\nПакет можно поставить снова перетаскиванием папки.".to_owned()
            }
            AppDialog::EdgeCycle { .. } => {
                "Ребро замкнуло бы цикл потока значений (граф обязан быть DAG).\nСоздать как контрольную связь — без передачи значения?".to_owned()
            }
        }
    }
}

/// Ярление заливки для hover-подсветки (кнопки настроек/темы): практика
/// аффорданса — интерактивная кнопка отвечает на курсор.
fn hover_fill(c: [f32; 4]) -> [f32; 4] {
    [
        (c[0] * 1.3 + 0.04).min(1.0),
        (c[1] * 1.3 + 0.04).min(1.0),
        (c[2] * 1.3 + 0.06).min(1.0),
        c[3],
    ]
}

/// FR-026: пересекаются ли два rect `[x, y, w, h]` — куллинг текстов строк
/// панели, перекрытых выпадающим меню (квады рисуются до screen-текстов).
fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// FR-012: settle-анимация после вставки в группу — (индекс, из, в) для
/// группы и раздвинутых соседей; интерполяция ease_out_cubic ~250 мс.
struct SettleAnim {
    moves: Vec<(usize, [f32; 2], [f32; 2])>,
    start: Instant,
}

/// Длительность settle-анимации вставки в группу (FR-012), мс.
const SETTLE_ANIM_MS: f32 = 250.0;

/// Состояние приложения: окно и рендерер создаются в `resumed`
/// (идиома winit 0.30 — окно создаётся только на активном event loop).
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<canvas_render::Renderer>,
    camera: Camera,
    scene: SceneState,
    /// Пул системных тамбнейлов (T6): заказы по видимым нодам, ответы в канал.
    thumbs: ThumbService,
    modifiers: ModifiersState,
    /// Позиция курсора в логических пикселях.
    cursor: Vec2,
    /// Текущая иконка курсора (аффорданс: пан — Grabbing, редактор — Text,
    /// resize-угол — NwseResize). Практики UI: форма курсора подсказывает
    /// жест; хранится, чтобы set_cursor вызывать только при смене.
    cursor_icon: CursorIcon,
    middle_pressed: bool,
    space_pressed: bool,
    left_pressed: bool,
    /// HUD с fps/p95/счётчиком видимых нод (F3, T5).
    hud_visible: bool,
    /// Замер интервалов между кадрами (окно 300 кадров).
    frame_meter: FrameMeter,
    /// Момент предыдущего отрисованного кадра.
    last_frame: Option<Instant>,
    /// Счётчики последнего кадра (для HUD).
    last_stats: FrameStats,
    /// Ноды, чей тамбнейл не удалось получить (битая ссылка и т.п.) —
    /// не перезаказывать каждый кадр; ретрай — при изменении файла вотчером (T10).
    thumbs_failed: std::collections::HashSet<usize>,
    /// Активная сессия инлайн-редактирования заметки (T7).
    editing: Option<EditingSession>,
    /// Драг внутри редактора (расширение выделения мышью, T7).
    editor_dragging: bool,
    /// Детектор двойного клика ЛКМ (T7).
    double_click: DoubleClick,
    /// Буфер обмена ОС (T7).
    clipboard: Clipboard,
    /// Открытое контекстное меню ноды (ПКМ, T7).
    menu: Option<ContextMenu>,
    /// Hover-раскрытие групп палитры (FR-009): hover-intent открытие,
    /// отсрочка закрытия, пин по клику — практики фронтенда для дропдаунов.
    palette_hover: PaletteHover,
    /// Цель палитры с прошлого кадра: смена (клик по другой ноде/связи,
    /// изменение выделения) сбрасывает раскрытие — индекс группы не должен
    /// переживать смену цели (состав групп у нод и связей разный).
    palette_seen: Option<PaletteTarget>,
    /// Ручной resize ноды за правый нижний угол (T7): индекс ноды.
    resizing: Option<usize>,
    /// Нода под курсором (T8): показываются порты для начала drag связи.
    hovered: Option<usize>,
    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк с
    /// прошлого кадра (логические px + текст ошибки). Заполняется после
    /// рендера, используется в сборке оверлея кадра (тултип у курсора —
    /// паттерн тултипа битой ссылки T10). Отставание в кадр незаметно.
    expr_error_hits: Vec<LineErrorHit>,
    /// Drag резиновой линии новой связи (T8): от порта до отпускания ЛКМ.
    edge_drag: Option<EdgeDrag>,
    /// Рамка выделения (CR-001): (start world, current world, press screen)
    /// — тянется от пустого места; отпускание > порога = выделение.
    select_rect: Option<(Vec2, Vec2, Vec2)>,
    /// Буфер нодов (FR-003, Ctrl+C/Ctrl+V): внутренний, НЕ системный
    /// clipboard (там текст редактора); вставка — с новыми id.
    node_clipboard: Vec<Node>,
    /// Панель горячих клавиш открыта (FR-004, F1): слева по центру.
    hotkeys_open: bool,
    /// Отложенный undo-снапшот (FR-006): «до» растянутого действия —
    /// drag/resize/редактирование. Ставится на старте, пушится в историю
    /// при фактическом изменении (клик без движения шага не создаёт).
    pending_undo: Option<Canvas>,
    /// Настройки приложения (config.toml).
    settings: Settings,
    /// Путь конфига (None — не сохраняем, работаем на дефолтах).
    config_path: Option<PathBuf>,
    /// Панель настроек открыта.
    settings_open: bool,
    /// Выпадающее меню строки настроек (FR-026): какая строка открыта;
    /// пункты вычисляются на кадр, состояние не устаревает.
    settings_dropdown: DropdownState,
    /// Превью зоны дропа (T9): план вставки на время DragOver.
    drop_preview: Option<DropPreview>,
    /// Модальный диалог T21 (установка/удаление пакета): глушит ввод канваса.
    dialog: Option<AppDialog>,
    /// Toast-строка (T21-A: bridge-toast, ошибки установки): живёт 3 с.
    toast: Option<(String, Instant)>,
    /// Файловый вотчер (T10): события ФС → AppEvent::FileEvents;
    /// набор директорий синхронизируется с моделью (sync_watch_dirs).
    watcher: WatchService,
    /// Регистрация IDropTarget (T9), Windows.
    #[cfg(windows)]
    drag_watcher: Option<canvas_shell::dragdrop::DropWatcher>,
    /// Отправитель drag-событий в event loop (T9). Читается только в
    /// cfg(windows)-ветке resumed(): единственный источник drag-событий —
    /// Windows IDropTarget (SPEC §7.3), на других ОС не читается.
    #[cfg_attr(not(windows), allow(dead_code))]
    drag_sender: Arc<dyn Fn(canvas_shell::dragdrop::DragEvent) + Send + Sync>,
    /// Отправитель событий WebView2-хоста виджетов в event loop (M5/T20).
    /// Прокси создаётся один раз в main() у `EventLoop` — у доступного в
    /// resumed() `ActiveEventLoop` метода create_proxy в winit 0.30 нет;
    /// тот же паттерн, что drag_sender. Читается только в cfg(windows)-ветке
    /// resumed(), на других ОС не читается.
    #[cfg_attr(not(windows), allow(dead_code))]
    widget_sender: canvas_widgets::WidgetEventSender,
    /// Миникарта (T13): снимок сцены + подгонка (CPU, SPEC §6.1).
    minimap: Option<Minimap>,
    /// Сигнатура состояния последней растеризации миникарты:
    /// (центр камеры, зум, размер буфера). Сцена отслеживается через
    /// dirty_since — правки/перемещения пересобирают снимок.
    minimap_sig: Option<([f32; 2], f32, u32, u32)>,
    /// Drag по миникарте (T13): world-точка под курсором следует за ним
    /// (клик без движения = мгновенное центрирование).
    minimap_drag: bool,
    /// M5 (T20-F): менеджер виджетов — реестр пакетов, LOD-план, host.
    widgets: canvas_app::widgets::WidgetManager,
    /// Панель поиска (T14): поле, строки, выбор, скролл.
    search: SearchPanel,
    /// Сервис FTS-индекса (T14): команды в worker-поток, ответы —
    /// AppEvent::Search через proxy.
    search_service: SearchService,
    /// Ноды результатов поиска — параллельно search.rows (T14).
    search_nodes: Vec<usize>,
    /// Debounce запроса (T14): (текст, момент последней правки) — отправка
    /// через 200 мс покоя в about_to_wait.
    search_pending: Option<(String, Instant)>,
    /// Полёт камеры к результату поиска (T14): (полёт, старт).
    flight: Option<(Flight, Instant)>,
    /// Пульс подсветки ноды-результата (T14): (нода, старт).
    pulse: Option<(usize, Instant)>,
    /// FR-019: реестр шаблонов — built-in библиотека из assets/templates
    /// (15 шаблонов, include_dir) + custom из ~/.canvasdesk/templates
    /// (FR-020, решение владельца — единый корень с виджетами).
    templates: canvas_core::templates::TemplateRegistry,
    /// FR-020: корень custom-шаблонов (`~/.canvasdesk/templates`).
    templates_root: std::path::PathBuf,
    /// FR-018: боковая палитра шаблонов (Ctrl+P): фильтр/категории/выбор.
    template_panel: template_ui::TemplatePanel,
    /// FR-018: радиальное wheel-меню шаблонов (Shift+клик по пустому
    /// месту): screen-центр + world-точка инстанциации + категория.
    wheel_menu: Option<template_ui::WheelMenu>,
    /// FR-021: popup контекстных подсказок Numi-ввода (состояние + якорь).
    hints: hints_ui::HintPopup,
    /// T23 (brainstorm-focus): затемнение сцены 0..1 (анимируется фейдом
    /// 150 мс при вкл/выкл и при появлении/исчезновении семени).
    focus_dim: f32,
    /// T23: активный фейд затемнения (от, к, старт).
    focus_fade: Option<(f32, f32, Instant)>,
    /// T23: «дыхание» подсвеченных связей: (семя, старт) — рестарт при
    /// смене семени, один цикл FOCUS_PULSE_MS, затем статика.
    focus_pulse: Option<(FocusSeed, Instant)>,
    /// T23: подсвеченные ноды (семя + соседи + выделенная) — данные
    /// FocusView кадра (пересчёт в update_focus_state).
    focus_nodes: Vec<usize>,
    /// T23: подсвеченные связи (инцидентные семени).
    focus_edges: Vec<usize>,
    /// FR-012: цель «втягивания» во время drag — группа под центром
    /// перетаскиваемой ноды (зона подсвечивается, отпускание — вставка).
    group_drop_target: Option<usize>,
    /// FR-012: settle-анимация после вставки в группу — плавный проезд
    /// группы и раздвинутых соседей к целевым позициям (~250 мс).
    settle_anim: Option<SettleAnim>,
    /// Режим десктопа (T15, флаг --desktop): окно встраивается в WorkerW
    /// (Windows; на других ОС — warn и обычный оконный режим, SPEC §9).
    desktop_mode: bool,
    /// Найденная иерархия десктопа (T15) после успешного attach: хэндлы
    /// Progman/DefView/WorkerW + стратегия. None — не встроены/фолбэк.
    #[cfg(windows)]
    desktop_hierarchy: Option<canvas_shell::desktop::hierarchy::DesktopHierarchy>,
    /// Последний достоверный DPI окна в desktop-режиме (T15, R10): снят
    /// GetDpiForWindow сразу после attach и обновляется поллингом монитора.
    /// None — оконный режим/до attach: scale берётся из window.scale_factor().
    #[cfg(windows)]
    desktop_dpi: Option<u32>,
    /// Shell-монитор (T15): WinEventHook на WorkerW + DPI-поллинг; события —
    /// AppEvent::Desktop через proxy. Спавнится в main() при --desktop,
    /// слежка (Watch) устанавливается в resumed() после attach.
    #[cfg(windows)]
    desktop_monitor: Option<canvas_shell::desktop::monitor::DesktopMonitorService>,
    /// Счётчик подряд неудач re-attach (T15): ≥3 — стоп автоматики + warn
    /// (анти-флуд упрощённый; полный по PID Shell_TrayWnd — T17).
    #[cfg(windows)]
    desktop_recover_failures: u32,
    /// WS_EX_NOACTIVATE уже снят первым кликом (T15, идемпотентный флаг).
    #[cfg(windows)]
    desktop_activation_enabled: bool,
    /// Шина системных событий (T16): message-only окно на отдельном потоке;
    /// события — AppEvent::Shell через proxy. Спавнится в main()
    /// без привязки к --desktop (события сессии/сна/shell-файлы полезны в
    /// любом режиме, план §8.7); провал — warn + деградация (R14).
    #[cfg(windows)]
    shell_events: Option<canvas_shell::shell_events::window::ShellEventService>,
    /// Последнее известное состояние гейта интерактивной сессии (T16, R8):
    /// лог/диагностика; потребление — T18. Старт true (как и атомик в шине).
    #[cfg(windows)]
    session_interactive: bool,
    /// Владелец скрытия системных иконок (T17-A, R5): capture+hide в
    /// attach_desktop; restore при штатном выходе, Drop-страховка от
    /// паник, sentinel — краш-сейф kill -9.
    #[cfg(windows)]
    icon_guard: Option<canvas_shell::desktop::icons::IconGuard>,
    /// Детект рестартов Explorer (T17-B, R7): PID Shell_TrayWnd
    /// до/после TaskbarCreated (из шины T16) + анти-флуд 30 с.
    #[cfg(windows)]
    explorer_tracker: canvas_shell::desktop::explorer::RestartTracker,
    /// MCP pipe-сервер (MCP-интеграция): worker-поток \\.\pipe\canvasdesk;
    /// запросы забираются по AppEvent::McpWake. None — MCP недоступен (деградация).
    #[cfg(windows)]
    mcp_server: Option<canvas_shell::mcp_pipe::McpPipeServer>,
}

impl App {
    // 8 аргументов — гейт-конфигурация сессии (сцена, сервисы, флаги);
    // группировать в структуру ради clippy — лишний слой на единственном
    // месте создания (main)
    #[allow(clippy::too_many_arguments)]
    fn new(
        scene: SceneState,
        thumbs: ThumbService,
        settings: Settings,
        config_path: Option<PathBuf>,
        drag_sender: Arc<dyn Fn(canvas_shell::dragdrop::DragEvent) + Send + Sync>,
        widget_sender: canvas_widgets::WidgetEventSender,
        watcher: WatchService,
        search_service: SearchService,
        desktop_mode: bool,
    ) -> Self {
        // M5 (T20-F): менеджер виджетов; реестр инициализируется в
        // main() (init_widgets) после настройки трейсинга
        let widgets = canvas_app::widgets::WidgetManager::new(
            canvas_shell::default_cache_dir()
                .unwrap_or_default()
                .join("widgets"),
            settings.theme == Theme::Dark,
        );
        Self {
            widgets,
            window: None,
            renderer: None,
            camera: Camera::default(),
            scene,
            thumbs,
            modifiers: ModifiersState::empty(),
            cursor: [0.0, 0.0],
            cursor_icon: CursorIcon::Default,
            middle_pressed: false,
            space_pressed: false,
            left_pressed: false,
            hud_visible: settings.hud_on_start,
            frame_meter: FrameMeter::new(),
            last_frame: None,
            last_stats: FrameStats::default(),
            thumbs_failed: std::collections::HashSet::new(),
            editing: None,
            editor_dragging: false,
            double_click: DoubleClick::new(),
            clipboard: Clipboard::new(),
            menu: None,
            palette_hover: PaletteHover::new(),
            palette_seen: None,
            resizing: None,
            hovered: None,
            expr_error_hits: Vec::new(),
            edge_drag: None,
            select_rect: None,
            node_clipboard: Vec::new(),
            hotkeys_open: false,
            pending_undo: None,
            settings,
            config_path,
            settings_open: false,
            settings_dropdown: DropdownState::default(),
            drop_preview: None,
            dialog: None,
            toast: None,
            watcher,
            #[cfg(windows)]
            drag_watcher: None,
            drag_sender,
            widget_sender,
            minimap: None,
            minimap_sig: None,
            minimap_drag: false,
            search: SearchPanel::default(),
            search_service,
            search_nodes: Vec::new(),
            search_pending: None,
            flight: None,
            pulse: None,
            templates_root: canvas_shell::default_cache_dir()
                .unwrap_or_default()
                .join("templates"),
            templates: {
                let root = canvas_shell::default_cache_dir()
                    .unwrap_or_default()
                    .join("templates");
                canvas_core::templates::TemplateRegistry::all_with_custom(&root)
            },
            template_panel: template_ui::TemplatePanel::new(),
            wheel_menu: None,
            hints: hints_ui::HintPopup::default(),
            focus_dim: 0.0,
            focus_fade: None,
            focus_pulse: None,
            focus_nodes: Vec::new(),
            focus_edges: Vec::new(),
            group_drop_target: None,
            settle_anim: None,
            desktop_mode,
            #[cfg(windows)]
            desktop_hierarchy: None,
            #[cfg(windows)]
            desktop_dpi: None,
            #[cfg(windows)]
            desktop_monitor: None,
            #[cfg(windows)]
            desktop_recover_failures: 0,
            #[cfg(windows)]
            desktop_activation_enabled: false,
            #[cfg(windows)]
            shell_events: None,
            // Старт true — зеркалит атомик IS_INTERACTIVE_SESSION в шине
            // (T16-A, план §8.3): залоченная до старта сессия события не
            // пришлёт до unlock
            #[cfg(windows)]
            session_interactive: true,
            #[cfg(windows)]
            icon_guard: None,
            #[cfg(windows)]
            explorer_tracker: Default::default(),
            #[cfg(windows)]
            mcp_server: None,
        }
    }

    /// Подключить shell-монитор десктопа (T15): спавнится в main() при
    /// --desktop (responder через EventLoopProxy), слежка — в resumed().
    #[cfg(windows)]
    fn set_desktop_monitor(
        &mut self,
        monitor: canvas_shell::desktop::monitor::DesktopMonitorService,
    ) {
        self.desktop_monitor = Some(monitor);
    }

    /// Подключить шину системных событий (T16; сеттер-паттерн T15 —
    /// сервис спавнится в main() до входа в event loop).
    #[cfg(windows)]
    fn set_shell_events(&mut self, service: canvas_shell::shell_events::window::ShellEventService) {
        self.shell_events = Some(service);
    }

    /// Подключить MCP pipe-сервер (MCP-интеграция; сеттер-паттерн T15/T16).
    #[cfg(windows)]
    fn set_mcp_server(&mut self, server: Option<canvas_shell::mcp_pipe::McpPipeServer>) {
        self.mcp_server = server;
    }

    /// HWND окна приложения через raw-window-handle (T15; тот же приём,
    /// что dragdrop::install в T9: winit 0.30 публично HWND не отдаёт).
    #[cfg(windows)]
    fn window_hwnd(&self) -> Option<isize> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let window = self.window.as_ref()?;
        match window.window_handle() {
            Ok(handle) => match handle.as_raw() {
                RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
                _ => None,
            },
            Err(_) => None,
        }
    }

    /// Конвертация raw-window-handle → HWND (идиома dragdrop/com.rs:
    /// Win32 HWND — указатель без внутренней структуры).
    #[cfg(windows)]
    fn hwnd(raw: isize) -> canvas_shell::desktop::HWND {
        canvas_shell::desktop::HWND(raw as *mut core::ffi::c_void)
    }

    /// Встройка в десктоп (T15, resumed): детект иерархии → идемпотентный
    /// спавн WorkerW → attach с верификацией стилей → слежка монитора.
    /// Любая ошибка — warn + MessageBox + фолбэк: окно остаётся обычным
    /// borderless top-level (R14; это и есть «обычное окно» — пересоздавать
    /// после winit-инициализации нельзя).
    #[cfg(windows)]
    fn attach_desktop(&mut self, raw: isize) {
        use canvas_shell::desktop::{attach, hierarchy};
        let hwnd = Self::hwnd(raw);
        let screen = hierarchy::virtual_screen_rect().unwrap_or_else(|| {
            tracing::warn!("виртуальный экран недоступен — экран 1280x720");
            canvas_shell::ScreenRect::from_ltrb(0, 0, 1280, 720)
        });
        let result = (|| -> Result<(hierarchy::DesktopHierarchy, ()), attach::AttachError> {
            let progman = hierarchy::find_progman()?;
            let hier = hierarchy::ensure_worker_w(progman)?;
            attach::attach(hwnd, &hier, screen).map(|_| (hier, ()))
        })();
        match result {
            Ok((hier, _)) => {
                tracing::info!(
                    strategy = ?hier.strategy,
                    worker_w = hier.worker_w.is_some(),
                    screen = ?(screen.left, screen.top, screen.right, screen.bottom),
                    "канвас встроен в рабочий стол (T15)"
                );
                // T17 (R5, SPEC §7.4 п.5): скрыть системные иконки — на
                // ОБЕИХ стратегиях. На Classic канвас встал НА МЕСТО слоя
                // иконок (без скрытия они невидимы, но кликабельны). На
                // Raised DefView непрозрачен для hit-test даже со скрытыми
                // иконками (проверено WindowFromPoint), поэтому канвас
                // поднят Z-order НАД DefView (attach шаг 4) и перекрывает
                // иконки опаком — скрытие 0x7402 держит поведение стратегий
                // одинаковым (канвас заменяет десктоп, M4) и синхронизирует
                // пункт меню «показать иконки» с фактом.
                let mut guard = canvas_shell::desktop::icons::IconGuard::capture(hier.def_view);
                guard.hide();
                self.icon_guard = Some(guard);
                // R10: зафиксировать достоверный DPI ДО первого тика
                // монитора (его первый замер — молчаливый бейзлайн): без
                // этого viewport_logical()/кнопки ещё один тик (500 мс) и
                // дольше — при стабильном DPI навсегда — считались бы от
                // врущего window.scale_factor() после репарентинга.
                let dpi = attach::window_dpi(hwnd);
                if dpi != 0 {
                    self.desktop_dpi = Some(dpi);
                    // R10: рендерер продолжает считать от врущего
                    // window.scale_factor() после репарентинга — передаём
                    // достоверный DPI из поллинга (как в DpiChanged ниже)
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_scale_factor(canvas_shell::dpi_to_scale(dpi));
                    }
                }
                self.desktop_hierarchy = Some(hier);
                self.watch_worker_w(hwnd, &hier);
            }
            Err(err) => {
                tracing::warn!(%err, "встройка в десктоп не удалась — оконный режим");
                attach::fallback_message_box(&err.to_string());
                // R14-деградация: desktop-окно создано borderless на весь
                // виртуальный экран — как top-level оно перекрывает Пуск и
                // иконки. Ужимаем до рабочей области (экран без таскбара).
                attach::shrink_to_work_area(hwnd);
            }
        }
    }

    /// Включить desktop-режим в рантайме (T15): вызвать `attach_desktop` и
    /// выставить `desktop_mode = true`. Если монитор не запущен (не было
    /// `--desktop` при старте) — запускаем его здесь же, чтобы слежка за
    /// WorkerW и DPI-поллинг работали сразу после встройки. Повторный вызов
    /// (уже в desktop-режиме) — no-op.
    ///
    /// T15: галочка «Режим десктопа» в меню канваса — режим активен И
    /// встойка удалась (иерархия найдена). Поле `desktop_hierarchy` есть
    /// только на Windows; на других ОС `desktop_mode` не поднимается
    /// (attach недоступен), поэтому там достаточно одного флага.
    #[cfg(windows)]
    fn desktop_menu_checked(&self) -> bool {
        self.desktop_mode && self.desktop_hierarchy.is_some()
    }

    /// Не-Windows вариант (см. windows-версию): attach недоступен —
    /// `desktop_mode` никогда не поднимается, галочка всегда снята.
    #[cfg(not(windows))]
    fn desktop_menu_checked(&self) -> bool {
        self.desktop_mode
    }

    /// Вход в desktop-режим в рантайме (T15-relaunch): перезапуск себя с
    /// флагом --desktop. In-place встройка (attach_desktop из меню, ee63b7a)
    /// НЕ работает: рендерер в оконном режиме создан с prefer_dx12=false,
    /// а Vulkan-swapchain не презентует в ребёнка Progman (проверено
    /// экспериментом, см. resumed()) — окно растягивается на виртуальный
    /// экран (Win32-шаги attach проходят), но канвас не рисуется и обои
    /// остаются видимыми. Новый процесс стартует с чистого листа: окно
    /// borderless → attach ДО создания GPU-surface → DX12-рендерер.
    /// Эксклюзивность — single-instance handoff (desktop/single_instance):
    /// новый инстанс сигналит exit-событие, этот инстанс штатно сохранится
    /// и выйдет (AppEvent::InstanceExit → shutdown), новый дождётся
    /// освобождения мьютекса и стартанёт в desktop-режиме. Ошибка спавна —
    /// строка для MessageBox (фолбэк R14: пользователь запустит вручную).
    #[cfg(windows)]
    fn spawn_desktop_relaunch(&self) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|err| format!("current_exe: {err}"))?;
        std::process::Command::new(&exe)
            .arg("--desktop")
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("spawn {:?}: {err}", exe))
    }

    /// Выключить desktop-режим в рантайме (T15): обратная к встройке —
    /// `attach::detach` (SetParent(None) + scrub-план «обычного» окна +
    /// shrink_to_work_area), восстановление иконок (IconGuard::restore),
    /// сброс `desktop_hierarchy` / `desktop_dpi` / `desktop_mode`. Монитор
    /// НЕ останавливаем (переживёт выход процесса — поток умрёт вместе с
    /// процессом при завершении).
    ///
    /// Здесь in-place detach корректен (в отличие от входа): рендерер
    /// создан с prefer_dx12=true и в обычном окне презентует нормально —
    /// пересоздавать его не нужно. Любая ошибка detach — не-фатальная:
    /// внутреннее состояние всё равно сбрасывается (канвас остаётся
    /// интерактивным в оконном режиме, даже если визуально окно «застряло»
    /// fullscreen — пользователь может перезапустить приложение).
    #[cfg(windows)]
    fn leave_desktop(&mut self) {
        if !self.desktop_mode {
            tracing::debug!("leave_desktop: не в desktop-режиме — no-op");
            return;
        }
        if let Some(raw) = self.window_hwnd() {
            let hwnd = Self::hwnd(raw);
            if let Err(err) = canvas_shell::desktop::attach::detach(hwnd) {
                tracing::warn!(%err, "leave_desktop: detach провален — состояние сброшено, окно м.б. некорректным");
            }
        } else {
            tracing::warn!("leave_desktop: нет HWND — только сброс состояния");
        }
        // Восстановить системные иконки (T17, R5): IconGuard::drop делает
        // restore, но мы явно вызываем restore() для ясности и сбрасываем
        // поле, чтобы Drop не сработал повторно при завершении приложения.
        if let Some(mut guard) = self.icon_guard.take() {
            guard.show();
            // guard тут drop-нется — restore через Drop страховкой не повторяем
            drop(guard);
        }
        // Снять слежку монитора (Watch с пустой иерархией — монитор переходит
        // в режим «ждать новой иерархии», не падает).
        self.desktop_hierarchy = None;
        self.desktop_dpi = None;
        self.desktop_mode = false;
        tracing::info!("desktop-режим выключен через меню (T15 runtime toggle)");
    }

    /// Установить/перенавесить слежку монитора на иерархию (T15):
    /// WinEventHook на поток WorkerW + DPI-поллинг нашего окна. WorkerW=None
    /// (фон без обоев) — hook не вешается, поллинг следит только за Progman.
    #[cfg(windows)]
    fn watch_worker_w(
        &self,
        ours: canvas_shell::desktop::HWND,
        hier: &canvas_shell::desktop::hierarchy::DesktopHierarchy,
    ) {
        if let Some(monitor) = &self.desktop_monitor {
            monitor.command(canvas_shell::desktop::monitor::MonitorCommand::Watch {
                progman: hier.progman,
                worker_w: hier.worker_w,
                ours,
            });
        }
    }

    /// R7-реакция на TaskbarCreated (T17, план §3): PID Shell_TrayWnd
    /// до/после; настоящий рестарт Explorer → FullReattach + репоинт
    /// иконок на новый DefView; DPI-смена/тема → игнор (поллинг T15
    /// догонит, R10); анти-флуд — crash-loop не утащит в бесконечный
    /// re-attach (R7 п.3).
    #[cfg(windows)]
    fn on_explorer_started(&mut self) {
        use canvas_shell::desktop::explorer::ExplorerRestart;
        // PID опрашиваем ДО register (совет T17-B worklog): окна может
        // не быть в момент события — это не рестарт
        let Some(pid) = canvas_shell::desktop::explorer::tray_pid() else {
            tracing::debug!("TaskbarCreated без Shell_TrayWnd — игнор");
            return;
        };
        match self.explorer_tracker.register(pid, Instant::now()) {
            ExplorerRestart::SameProcess => {
                tracing::info!(pid, "Explorer жив (TaskbarCreated от DPI/темы)");
            }
            ExplorerRestart::Restarted => {
                tracing::warn!(pid, "Explorer перезапущен — восстановление встройки");
                if self.explorer_tracker.suppress_automatic() {
                    // Недостижимо для семантики register (флуд →
                    // FloodStop), страховка от дрейфа
                    tracing::warn!("автоматика восстановления остановлена");
                    return;
                }
                self.recover_desktop(canvas_shell::RecoveryAction::FullReattach);
                // Иерархия пересоздана — guard уже репоинтнут внутри
                // recover_desktop (Ok-ветка); скрытие до-скрыто там же
            }
            ExplorerRestart::FloodStop => {
                // crash-loop: >1 рестарта за 30 с — автоматика стоп,
                // сообщение пользователю в лог (R7 п.3)
                tracing::warn!(
                    "crash-loop Explorer (>1 рестарта за 30 с) — автоматика восстановления остановлена"
                );
            }
        }
    }

    /// Обработка событий shell-монитора (T15): разрушение WorkerW →
    /// recovery по стратегии (R2-симметрия); смена DPI → переградуировка
    /// рендера (R10: winit-события после репарентинга не приходят).
    #[cfg(windows)]
    fn on_desktop_event(&mut self, event: canvas_shell::DesktopEvent) {
        match event {
            canvas_shell::DesktopEvent::WorkerWDestroyed => {
                let action =
                    canvas_shell::recovery_action(self.desktop_hierarchy.map(|h| h.strategy));
                match action {
                    canvas_shell::RecoveryAction::None => {}
                    canvas_shell::RecoveryAction::ReZOrder
                    | canvas_shell::RecoveryAction::FullReattach => self.recover_desktop(action),
                }
            }
            canvas_shell::DesktopEvent::DpiChanged { dpi } => {
                let scale = canvas_shell::dpi_to_scale(dpi);
                tracing::info!(dpi, scale, "DPI десктоп-окна изменился (поллинг R10)");
                self.desktop_dpi = Some(dpi);
                if let Some(renderer) = self.renderer.as_mut() {
                    // Пересоздание surface не нужно: размер HWND не менялся;
                    // минимап пересоберётся по сигнатуре кадра
                    renderer.set_scale_factor(scale);
                }
                self.request_redraw();
            }
        }
    }

    /// Обработка событий шины системных событий (T16, план §3): сессия —
    /// гейт интерактивности (лог/диагностика, R8; потребление — T18),
    /// Suspending — форс-сейв .canvas ДО ухода системы в сон (критерий
    /// TASKS T16; бэкап — внутри save_with_backup, SPEC §9), Resumed —
    /// прогрев кадра, файл-события SHCNE-моста (R12) — конвейер T10.
    #[cfg(windows)]
    fn on_shell_event(&mut self, event: canvas_shell::shell_events::ShellEvent) {
        use canvas_shell::shell_events::ShellEvent;
        match event {
            // classify_wts в шине уже отсёк чужие сессии (R8): дошли только
            // свои — гейт актуален; атомик шины обновлён там же (wndproc)
            ShellEvent::Session { event, session_id } => {
                if let Some(value) = event.gate_value() {
                    self.session_interactive = value;
                }
                tracing::info!(
                    ?event,
                    session_id,
                    interactive = self.session_interactive,
                    "событие сессии (R8)"
                );
            }
            // Безусловно (не только dirty): дебаунс-сейв может не успеть —
            // запись ДО сна обязательна (критерий TASKS T16)
            ShellEvent::Suspending => {
                tracing::info!("система уходит в сон — форс-сейв канваса");
                self.scene.save_now();
            }
            // Прогрев кадра: после сна первый кадр мог не прийти от winit
            ShellEvent::Resumed { kind } => {
                tracing::info!(?kind, "система проснулась");
                self.request_redraw();
            }
            // Потребитель — T17 (R7): PID Shell_TrayWnd отличает краш
            // от DPI-смены (TaskbarCreated приходит и на смену темы);
            // настоящий рестарт → FullReattach, анти-флуд 30 с
            ShellEvent::ExplorerStarted => self.on_explorer_started(),
            // Декод HSHELL_* — T18 (план §3); лог не спамим — debug
            ShellEvent::ShellHook { code, hwnd } => {
                tracing::debug!(code, hwnd, "shell-hook (декод — T18)");
            }
            // Задел «вставить как ноду» (SPEC §7.6) — потребитель будущего
            ShellEvent::ClipboardUpdated => {
                tracing::debug!("буфер обмена обновлён");
            }
            // SHCNE-мост (R12): тот же конвейер, что у вотчера T10 —
            // идемпотентен к дублям notify (§8.8); коалессер шины уже
            // сгладил шквал (150 мс, WM_TIMER-флаш)
            ShellEvent::FileEvents(events) => self.on_file_events(events),
        }
    }

    /// Восстановление встройки после разрушения WorkerW (T15): ReZOrder —
    /// перевыполнить только Z-order (raised, R2); FullReattach — полный
    /// re-attach (classic: SetParent на новый WorkerW). Анти-флуд:
    /// 3 подряд неудачи → стоп автоматики (полный анти-флуд — T17).
    #[cfg(windows)]
    fn recover_desktop(&mut self, action: canvas_shell::RecoveryAction) {
        use canvas_shell::desktop::{attach, hierarchy};
        if self.desktop_recover_failures >= 3 {
            // уже остановлены: не логировать спам — события могут идти потоком
            return;
        }
        let Some(raw) = self.window_hwnd() else {
            return;
        };
        let hwnd = Self::hwnd(raw);
        let result = (|| -> Result<(hierarchy::DesktopHierarchy, ()), attach::AttachError> {
            let progman = hierarchy::find_progman()?;
            let hier = hierarchy::ensure_worker_w(progman)?;
            match action {
                canvas_shell::RecoveryAction::ReZOrder => {
                    attach::refresh_z_order(hwnd, &hier).map(|_| (hier, ()))
                }
                canvas_shell::RecoveryAction::FullReattach => {
                    let screen = hierarchy::virtual_screen_rect()
                        .unwrap_or_else(|| canvas_shell::ScreenRect::from_ltrb(0, 0, 1280, 720));
                    attach::attach(hwnd, &hier, screen).map(|_| (hier, ()))
                }
                canvas_shell::RecoveryAction::None => Ok((hier, ())),
            }
        })();
        match result {
            Ok((hier, _)) => {
                tracing::info!(?action, strategy = ?hier.strategy, "встройка восстановлена");
                self.desktop_recover_failures = 0;
                // T17 (R7): DefView мог пересоздаться вместе с WorkerW —
                // guard репоинтится на живой слой иконок; hide
                // идемпотентен (SHELLSTATE персистентен) — до-скроет при
                // расхождении
                if let Some(guard) = self.icon_guard.as_mut() {
                    guard.repoint(hier.def_view);
                    guard.hide();
                }
                self.desktop_hierarchy = Some(hier);
                self.watch_worker_w(hwnd, &hier);
            }
            Err(err) => {
                self.desktop_recover_failures += 1;
                if self.desktop_recover_failures >= 3 {
                    tracing::warn!(
                        %err,
                        failures = self.desktop_recover_failures,
                        "re-attach не удаётся — автоматика восстановления остановлена"
                    );
                    self.desktop_hierarchy = None;
                } else {
                    tracing::warn!(%err, failures = self.desktop_recover_failures, "re-attach не удался");
                }
            }
        }
    }

    /// Scale factor окна (1.0 до создания окна). В desktop-режиме после
    /// attach — из desktop_dpi (GetDpiForWindow, R10: winit врёт после
    /// репарентинга), иначе — scale_factor окна.
    fn scale_factor(&self) -> f32 {
        let window_scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        #[cfg(windows)]
        {
            canvas_app::ui::effective_scale(window_scale, self.desktop_dpi)
        }
        #[cfg(not(windows))]
        {
            window_scale
        }
    }

    /// zoom * scale_factor — перевод world-px в физические (для буфера редактора).
    fn zoom_px(&self) -> f32 {
        self.camera.zoom() * self.scale_factor()
    }

    /// Начать редактирование текстовой ноды (T7) или подписи группы:
    /// text-нода редактирует `text`, группа — `label` (двойной клик).
    /// Прочие ноды игнорируются.
    fn begin_editing(&mut self, index: usize) {
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        let is_group = node.kind() == NodeKind::Group;
        if node.kind() != NodeKind::Text && !is_group {
            return;
        }
        let text = if is_group {
            node.label.clone().unwrap_or_default()
        } else {
            node.text.clone().unwrap_or_default()
        };
        let (_, width, height) = body_area(node);
        // FR-006: отложенный снапшот «до» правки — шаг закроется на commit
        // с фактическим изменением текста (finish_editing)
        self.begin_pending_undo();
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new(
            renderer.font_system_mut(),
            EditTarget::Node(index),
            &text,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        // FR-021: popup подсказок — с чистого листа на каждую правку
        self.hints.reset();
        self.scene.selected = Some(Selection::Node(index));
        self.scene.dragging = None;
        // Давняя заметка могла переполниться до нас (загрузка из файла) —
        // подгоняем размер сразу при входе в редактирование. Группу под
        // текст не подгоняем: рамку ресайзит только пользователь.
        if !is_group {
            self.fit_note_size();
        }
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// Начать редактирование лейбла связи (T8): двойной клик по линии.
    /// Бокс редактирования — по центру дуги связи (edge_edit_area).
    fn begin_editing_edge(&mut self, index: usize) {
        let Some(edge) = self.scene.canvas.edges.get(index) else {
            return;
        };
        let text = edge.label.clone().unwrap_or_default();
        let Some((_, width, height)) =
            edge_edit_area(&self.scene.canvas, index, self.settings.edges_avoid_nodes)
        else {
            return;
        };
        // FR-006: отложенный снапшот «до» правки лейбла (паттерн begin_editing)
        self.begin_pending_undo();
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new(
            renderer.font_system_mut(),
            EditTarget::Edge(index),
            &text,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        self.hints.reset();
        self.scene.selected = Some(Selection::Edge(index));
        self.scene.dragging = None;
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// Подрастить высоту редактируемой заметки под контент (T7): текст
    /// переносится по ширине карточки (Wrap::WordOrGlyph), за край не
    /// уходит — растёт только высота (по числу строк layout). Ширина
    /// карточки за пользователем: авто-растягивание по самой длинной
    /// строке убрано (правило «перенос даже одной строки»). Только рост.
    /// Только text-ноды: группы под текст не подгоняются (рамку ресайзит
    /// пользователь). Для лейблов связей (T8) не применяется — бокс фиксированный.
    fn fit_note_size(&mut self) {
        let zoom_px = self.zoom_px();
        let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut())
        else {
            return;
        };
        let EditTarget::Node(index) = session.target() else {
            return;
        };
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        if node.kind() != NodeKind::Text {
            return;
        }
        let (_content_w_px, content_h_px) = session.content_size_px(renderer.font_system_mut());
        // FR-013: резерв под строку результата формулы (футер карточки),
        // чтобы подрезка тела результатом не прятала последнюю строку.
        // Кандидат — явная формула или последняя строка текста (авто-детект,
        // Numi-семантика); при активном редактировании — живой текст сессии
        let live_text = session.text();
        // FR-013 (правка 2): резерв футера — только для программного итога
        // (MCP-expr без формульных строк в тексте). Построчные результаты
        // Numi-стиля места не требуют — ложатся на свои строки.
        // FR-023: у шаблонной ноды итог формулы шаблона показывается ВСЕГДА
        // (text.rs: правило `is_template_node`), поэтому резерв — всегда,
        // независимо от построчных результатов листа параметров
        let has_line_results = expr::eval_lines(&live_text).iter().any(Option::is_some);
        let is_template = self
            .scene
            .canvas
            .nodes
            .get(index)
            .is_some_and(|node| node.kind() == NodeKind::Text && node.template().is_some());
        let program_footer = self
            .scene
            .canvas
            .nodes
            .get(index)
            .is_some_and(|node| node.expr().is_some())
            && !has_line_results;
        let expr_footer = if is_template || program_footer {
            RESULT_LINE_HEIGHT + 2.0
        } else {
            0.0
        };
        let needed_h =
            HEADER_HEIGHT + BODY_TOP_GAP + content_h_px / zoom_px + BODY_PADDING + expr_footer;
        let Some(node) = self.scene.canvas.nodes.get_mut(index) else {
            return;
        };
        if needed_h > node.height + 0.5 {
            node.height = needed_h;
            self.scene.spatial.update(index, node);
            self.scene.mark_dirty();
        }
    }

    /// Завершить редактирование (T7/T8): commit — записать текст в модель и
    /// пометить канвас грязным (автосейв); cancel — откат, модель не менялась.
    /// Для связи (T8) и подписи группы пустой лейбл при commit сбрасывается
    /// в None.
    fn finish_editing(&mut self, commit: bool) {
        let Some(session) = self.editing.take() else {
            return;
        };
        // FR-021: сессия закрыта — popup подсказок больше не нужен
        self.hints.reset();
        self.editor_dragging = false;
        if commit && session.changed() {
            // FR-006: правка состоялась — отложенный снапшот «до» в историю
            // (мутация ниже); cancel-ветка дропнет его
            if let Some(snapshot) = self.pending_undo.take() {
                self.scene.push_undo(snapshot);
            }
            match session.target() {
                EditTarget::Node(index) => {
                    let node_id = self.scene.canvas.nodes.get(index).map(|n| n.id.clone());
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        if node.kind() == NodeKind::Group {
                            // Подпись группы: пустая — сброс в None
                            let text = session.text();
                            let text = text.trim();
                            node.label = if text.is_empty() {
                                None
                            } else {
                                Some(text.to_owned())
                            };
                        } else {
                            let text = session.text();
                            node.text = Some(text.clone());
                            // FR-013: строки «= …» — формула (смешанный
                            // редактор); commit выводит canvasdesk.expr и
                            // пересчитывает строку результата (один undo-шаг
                            // вместе с текстом — паттерн FR-006 выше)
                            node.set_expr(split_formula_lines(&text));
                            // FR-018: у шаблонной ноды текст — Numi-лист
                            // параметров; правка синхронизирует
                            // canvasdesk.template.params (id/version/expr
                            // сохраняются), propagator пересчитает
                            // формулу шаблона с новыми значениями.
                            // FR-023: слияние вместо замены — параметры,
                            // чьих строк нет в правке (удалены/переименованы/
                            // временно сломаны), сохраняются: формула
                            // шаблона остаётся вычислимой, итог (единица,
                            // напр. sec) не пропадает
                            if node.template().is_some() {
                                let fresh = canvas_core::templates::params_from_text(&text);
                                let params = canvas_core::templates::merge_params(
                                    node.template_params(),
                                    fresh,
                                );
                                node.set_template_params(params);
                            }
                        }
                    }
                    // FR-013: пересчёт результата (после мутации модели);
                    // FR-014: живой пересчёт downstream (весь граф — дёшево)
                    if node_id.is_some() {
                        self.scene.recompute_flow();
                    }
                }
                EditTarget::Edge(index) => {
                    if let Some(edge) = self.scene.canvas.edges.get_mut(index) {
                        let text = session.text();
                        let text = text.trim();
                        edge.label = if text.is_empty() {
                            None
                        } else {
                            Some(text.to_owned())
                        };
                        // Кэш лейблов в TextSystem перешейпится сам:
                        // ключ свежести — равенство текста (text.rs)
                    }
                }
            }
            self.scene.mark_dirty();
        } else {
            // FR-006: отмена правки — модель не менялась, отложенный
            // снапшот «до» дропается (no-op шагов в истории нет)
            self.pending_undo = None;
        }
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// Вставить готовые ноды в модель (FR-003, паттерн insert_group):
    /// push + spatial index; `select` — выделить вставленные пачкой
    /// (CR-001). Возвращает индексы вставленных (порядок сохранён).
    fn insert_nodes(&mut self, nodes: Vec<Node>, select: bool) -> Vec<usize> {
        // FR-006: вставка (paste/duplicate/drop-планы) — undo-шаг
        self.push_undo();
        let mut indices = Vec::with_capacity(nodes.len());
        for node in nodes {
            self.scene.canvas.nodes.push(node);
            let index = self.scene.canvas.nodes.len() - 1;
            let node_ref = &self.scene.canvas.nodes[index];
            self.scene.spatial.insert(index, node_ref);
            indices.push(index);
        }
        if select {
            self.scene.selected = None;
            self.scene.selected_nodes = indices.clone();
        }
        self.scene.mark_dirty();
        // Файловые копии — вотчер/поиск должны увидеть директории (T10)
        self.sync_watch_dirs();
        self.request_redraw();
        indices
    }

    /// Push undo-снапшота текущего состояния (FR-006): вызывать
    /// непосредственно ПЕРЕД мутацией модели.
    fn push_undo(&mut self) {
        let snapshot = self.scene.canvas.clone();
        self.scene.push_undo(snapshot);
    }

    /// Начать отложенное действие (FR-006): drag/resize/редактирование —
    /// снапшот «до» запоминается на старте; пуш в историю только при
    /// фактическом изменении (см. finish_interaction_undo / finish_editing).
    fn begin_pending_undo(&mut self) {
        self.pending_undo = Some(self.scene.canvas.clone());
    }

    /// Закрыть отложенное действие drag/resize (FR-006): вызывается на
    /// отпускании ЛКМ и при прерывании drag отпусканием Space. Push только
    /// если геометрия реально изменилась — клик без движения не шаг.
    fn finish_interaction_undo(&mut self) {
        let Some(snapshot) = self.pending_undo.take() else {
            return;
        };
        // Drag: позиции нод отличаются от исходных (origins хранит «до»)
        let moved = self.scene.dragging.as_ref().is_some_and(|drag| {
            drag.origins.iter().any(|(index, origin)| {
                self.scene
                    .canvas
                    .nodes
                    .get(*index)
                    .is_some_and(|node| node.x != origin[0] || node.y != origin[1])
            })
        });
        // Resize: размеры отличаются от снапшотных (кламп мог дать те же)
        let resized = self.resizing.is_some_and(|index| {
            self.scene
                .canvas
                .nodes
                .get(index)
                .zip(snapshot.nodes.get(index))
                .is_some_and(|(now, before)| {
                    now.width != before.width || now.height != before.height
                })
        });
        if moved || resized {
            self.scene.push_undo(snapshot);
        }
    }

    /// Отменить последнее действие (FR-006, Ctrl+Z): модель «до» из
    /// undo-стека, текущее состояние — в redo.
    fn undo_action(&mut self) {
        if let Some(before) = self.scene.take_undo() {
            self.restore_canvas(before);
            tracing::debug!(depth = self.scene.undo_stack.len(), "undo");
        }
    }

    /// Вернуть отменённое (FR-006, Ctrl+Y / Ctrl+Shift+Z).
    fn redo_action(&mut self) {
        if let Some(after) = self.scene.take_redo() {
            self.restore_canvas(after);
            tracing::debug!(depth = self.scene.redo_stack.len(), "redo");
        }
    }

    /// Восстановить снапшот (FR-006): модель + spatial + сброс кэшей
    /// (индексы из прошлых состояний недостоверны — паттерн
    /// delete_selected). Интеракции и редактирование прерываются без
    /// коммита; автосейв следует за mark_dirty.
    fn restore_canvas(&mut self, canvas: Canvas) {
        self.pending_undo = None;
        self.editing = None;
        self.editor_dragging = false;
        self.settle_anim = None; // FR-012: анимация не валидна после отката
        self.group_drop_target = None;
        self.scene.canvas = canvas;
        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
        // FR-013: снапшот мог изменить формулы и топологию — живой
        // пересчёт графа потока (FR-014)
        self.scene.recompute_flow();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.invalidate_node_caches();
        }
        self.thumbs_failed.clear();
        self.scene.selected = None;
        self.scene.selected_nodes.clear();
        self.scene.dragging = None;
        self.resizing = None;
        self.menu = None;
        self.hovered = None;
        self.edge_drag = None;
        self.select_rect = None;
        self.scene.mark_dirty();
        // Файловый состав мог измениться — вотчер и SHCNE-подписки (T10)
        self.sync_watch_dirs();
        self.request_redraw();
    }

    /// Индексы выделенных нод (FR-003): набор мультивыделения ∪ primary,
    /// по возрастанию без дубликатов; пусто — ничего не выделено.
    fn selection_node_indices(&self) -> Vec<usize> {
        let mut indices = self.scene.selected_nodes.clone();
        if let Some(Selection::Node(index)) = self.scene.selected {
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    /// Скопировать выделенные ноды в буфер (FR-003, Ctrl+C): первичное
    /// взаимное расположение сохраняется — вставка центром bbox на курсор.
    fn copy_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        self.node_clipboard = indices
            .into_iter()
            .filter_map(|index| self.scene.canvas.nodes.get(index).cloned())
            .collect();
        tracing::debug!(
            count = self.node_clipboard.len(),
            "ноды скопированы в буфер"
        );
    }

    /// Вставить буфер (FR-003, Ctrl+V): копии с новыми id — центром bbox
    /// в позицию курсора; вставленное становится мультивыделением (CR-001).
    fn paste_clipboard(&mut self) {
        if self.node_clipboard.is_empty() {
            return;
        }
        let copies = reassign_ids(&self.scene.canvas, &self.node_clipboard);
        let placement = PastePlacement::AtCursor(self.cursor_world());
        let nodes = paste_nodes(&copies, placement);
        self.insert_nodes(nodes, true);
    }

    /// Дублировать выделение (FR-003, Ctrl+D): копии с новыми id со
    /// сдвигом DUPLICATE_OFFSET; копии становятся мультивыделением.
    fn duplicate_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        let originals = indices
            .iter()
            .filter_map(|&index| self.scene.canvas.nodes.get(index))
            .cloned()
            .collect::<Vec<Node>>();
        let copies = reassign_ids(&self.scene.canvas, &originals);
        let nodes = paste_nodes(
            &copies,
            PastePlacement::Offset([DUPLICATE_OFFSET, DUPLICATE_OFFSET]),
        );
        self.insert_nodes(nodes, true);
    }

    /// Сгруппировать выделенные ноды (Ctrl+G): группа с bbox по всему
    /// набору (мультивыделение ∪ primary, CR-001) + GROUP_PADDING; дети —
    /// явный список id (FR-012). Пустое выделение — no-op (семантика
    /// Figma/PowerPoint: группировать нечего — действие не срабатывает).
    /// Undo-шаг, spatial index и перенос выделения на новую группу —
    /// внутри insert_group (паттерн «Сгруппировать» палитры).
    fn group_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        if let Some(group) =
            plan_group_around_nodes(&self.scene.canvas, &indices, canvas_app::ui::GROUP_PADDING)
        {
            self.insert_group(group);
            self.request_redraw();
        }
    }

    /// Вырезать выделенные ноды (FR-007, Ctrl+X): копирование в буфер
    /// (FR-003) + удаление (undo-шаг — внутри delete_selected, FR-006).
    /// Вставка — обычным Ctrl+V: копии с новыми id (правила FR-003).
    fn cut_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        self.copy_selection();
        self.delete_selected();
        tracing::debug!(count = self.node_clipboard.len(), "ноды вырезаны в буфер");
    }

    /// Удалить выделенное (T8, Del; CR-001 — мультивыделение): набор нод —
    /// пачкой (canvas-core remove_nodes); связь — по id; одиночную ноду —
    /// каскадно со связями. После удаления нод индексы в canvas.nodes
    /// сдвигаются, поэтому spatial index перестраивается, а все кэши,
    /// ключованные usize (текст, атлас тамбнейлов, негативный кэш),
    /// сбрасываются полностью.
    fn delete_selected(&mut self) {
        // CR-001: мультивыделение — удаляем весь набор (рамка/Ctrl+клик)
        if !self.scene.selected_nodes.is_empty() {
            // FR-006: удаление набора — undo-шаг
            self.push_undo();
            let indices = std::mem::take(&mut self.scene.selected_nodes);
            let removed = self.scene.canvas.remove_nodes(&indices);
            if removed.is_empty() {
                return;
            }
            self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.invalidate_node_caches();
            }
            self.thumbs_failed.clear();
            self.scene.selected = None;
            self.scene.dragging = None;
            self.resizing = None;
            self.editing = None;
            self.menu = None;
            self.hovered = None;
            self.edge_drag = None;
            self.scene.mark_dirty();
            self.sync_watch_dirs();
            // FR-014: downstream удалённых нод — «вход отсутствует»
            self.scene.recompute_flow();
            self.request_redraw();
            // Редактирование прервано удалением — отложенный снапшот (FR-006)
            // больше не актуален: правки умрут вместе с нодой
            self.pending_undo = None;
            return;
        }
        match self.scene.selected {
            Some(Selection::Edge(index)) => {
                let Some(edge) = self.scene.canvas.edges.get(index) else {
                    return;
                };
                let id = edge.id.clone();
                // FR-006: удаление связи — undo-шаг
                self.push_undo();
                self.scene.canvas.remove_edge(&id);
                self.scene.selected = None;
                self.scene.mark_dirty();
                // FR-014: downstream этой связи — «вход отсутствует»
                self.scene.recompute_flow();
                self.request_redraw();
            }
            Some(Selection::Node(index)) => {
                // FR-006: удаление ноды — undo-шаг (снапшот ДО мутации)
                if self.scene.canvas.nodes.get(index).is_some() {
                    self.push_undo();
                }
                if self.scene.canvas.remove_node(index).is_none() {
                    return;
                }
                self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.invalidate_node_caches();
                }
                self.thumbs_failed.clear();
                self.scene.selected = None;
                self.scene.selected_nodes.clear();
                self.scene.dragging = None;
                self.resizing = None;
                self.editing = None;
                self.menu = None;
                self.hovered = None;
                self.edge_drag = None;
                self.scene.mark_dirty();
                // Директории удалённых нод больше не нужны вотчеру (T10)
                self.sync_watch_dirs();
                // FR-014: downstream удалённой ноды — «вход отсутствует»
                self.scene.recompute_flow();
                self.request_redraw();
            }
            None => {}
        }
    }

    /// Создать пустую заметку в world-точке (T7): модель + spatial index.
    /// Возвращает индекс новой ноды.
    fn create_note_at(&mut self, world: Vec2) -> usize {
        // FR-006: создание заметки — undo-шаг
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "note");
        self.scene
            .canvas
            .nodes
            .push(Node::text(id, "", world[0], world[1]));
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.scene.selected = Some(Selection::Node(index));
        self.scene.selected_nodes.clear();
        self.scene.mark_dirty();
        index
    }

    /// Выборочный hit-test под world-точкой: сначала не-group ноды
    /// (меньшая площадь в приоритете — ребёнок группы раньше группы),
    /// затем группы. Кандидаты — точечный запрос spatial index.
    /// FR-011: скрытые ноды (свернутые поддеревья) из hit-test исключены.
    fn selective_hit(&self, world: Vec2) -> Option<usize> {
        let candidates = self
            .scene
            .spatial
            .query_rect([world[0], world[1], world[0], world[1]]);
        let hidden = self.hidden_subtree_nodes();
        let visible: Vec<usize> = candidates
            .into_iter()
            .filter(|index| hidden.binary_search(index).is_err())
            .collect();
        select_node_hit(&self.scene.canvas, &visible)
    }

    /// Хэндл конца выделенной связи под world-точкой (CR-002): конец, чей
    /// порт ближе к курсору в допуске зоны портов (CR-003, экранные px →
    /// world делением на zoom). None — мимо обоих концов/связь висячая.
    fn edge_handle_at(&self, edge_index: usize, world: Vec2) -> Option<canvas_core::EdgeEnd> {
        let tolerance = self.settings.port_zone_px / self.camera.zoom().max(1e-3);
        let dist = |p: &[f32; 2]| ((p[0] - world[0]).powi(2) + (p[1] - world[1]).powi(2)).sqrt();
        [canvas_core::EdgeEnd::From, canvas_core::EdgeEnd::To]
            .into_iter()
            .filter_map(|end| {
                canvas_core::edge_endpoint(&self.scene.canvas, edge_index, end)
                    .map(|(_, point)| (end, dist(&point)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(end, _)| end)
    }

    /// Центр видимого мира (мировые координаты) — для «Создать группу».
    fn viewport_center_world(&self) -> Vec2 {
        let rect = self.camera.visible_world_rect(self.viewport_logical());
        [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0]
    }

    /// Вставить готовую ноду-группу в модель (паттерн create_note_at):
    /// spatial index + выделение новой группы. Возвращает индекс.
    fn insert_group(&mut self, group: Node) -> usize {
        // FR-006: создание группы — undo-шаг
        self.push_undo();
        self.scene.canvas.nodes.push(group);
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.scene.selected = Some(Selection::Node(index));
        self.scene.selected_nodes.clear();
        self.scene.mark_dirty();
        index
    }

    /// Активно ли панорамирование (средняя кнопка или Space+drag, SPEC §8).
    fn panning(&self) -> bool {
        self.middle_pressed || (self.space_pressed && self.left_pressed)
    }

    /// Аффорданс курсора (практики UI: форма курсора подсказывает жест):
    /// пан — Grabbing, текстовый редактор — Text, resize-угол ноды —
    /// NwseResize, иначе Arrow. set_cursor вызывается только при смене.
    fn sync_cursor_icon(&mut self) {
        let desired = if self.panning() {
            CursorIcon::Grabbing
        } else if self.editing.is_some() {
            CursorIcon::Text
        } else {
            let world = self.cursor_world();
            let resize = self
                .hovered
                .and_then(|i| self.scene.canvas.nodes.get(i))
                .is_some_and(|node| canvas_app::ui::in_resize_corner(node, world));
            if resize {
                CursorIcon::NwseResize
            } else {
                CursorIcon::Default
            }
        };
        if self.cursor_icon != desired {
            self.cursor_icon = desired;
            if let Some(window) = &self.window {
                window.set_cursor(desired);
            }
        }
    }

    /// Сброс залипших pointer-transient состояний при потере фокуса окна
    /// (alt-tab во время drag оставлял «прилипшую» ноду/пан — кнопка
    /// Released приходит в другое окно). Практики UI: модальные переходы
    /// гасят активные жесты.
    fn cancel_pointer_transients(&mut self) {
        self.space_pressed = false;
        self.middle_pressed = false;
        self.left_pressed = false;
        self.minimap_drag = false;
        self.editor_dragging = false;
        self.resizing = None;
        self.edge_drag = None;
        self.select_rect = None;
        self.group_drop_target = None;
        if self.scene.dragging.is_some() {
            // FR-006: движение до потери фокуса — undo-шаг
            self.finish_interaction_undo();
            self.scene.dragging = None;
        }
        self.sync_cursor_icon();
    }

    /// Курсор над открытой screen-space поверхностью (кнопки/панель
    /// настроек, поиск, меню+подменю, палитра, хоткеи, миникарта, диалог).
    /// Практика canvas-приложений (Miro/Figma): колесо/пинч над плавающим
    /// UI холст не двигают.
    fn cursor_over_screen_surface(&self) -> bool {
        let viewport = self.viewport_logical();
        let over = |rect: [f32; 4]| point_in_rect(rect, self.cursor);
        if over(button_rect(self.settings.button_corner, viewport))
            || over(theme_button_rect(self.settings.button_corner, viewport))
        {
            return true;
        }
        if self.settings_open && over(panel_rect(self.settings.button_corner, viewport)) {
            return true;
        }
        // FR-026: открытое выпадающее меню настройки — тоже screen-поверхность
        // (может выходить за пределы панели, колесо/пинч над ним холст не двигают)
        if self.settings_open && self.settings_dropdown.is_open() {
            let layout = panel_layout(self.settings.button_corner, viewport);
            if let Some(row) = self.settings_dropdown.open_row {
                let items = dropdown_options(row, &self.settings);
                let anchor = layout.row_rect(row).unwrap_or([0.0; 4]);
                if over(dropdown_layout(anchor, viewport, items.len())) {
                    return true;
                }
            }
        }
        if self.search.is_open() {
            let lay = search_layout(viewport[0], viewport[1], &self.search);
            if over(rect_xywh(lay.panel_rect)) {
                return true;
            }
        }
        if let Some(rect) = self.menu_open_rect() {
            if over(rect) {
                return true;
            }
            if let Some(submenu) = self.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                if over(submenu_rect(submenu)) {
                    return true;
                }
            }
        }
        if let Some((lay, _, _)) = self.palette_geometry() {
            if over(lay.bar) {
                return true;
            }
            if let Some(group) = self.palette_hover.open.and_then(|g| lay.groups.get(g)) {
                if over(group.dropdown) {
                    return true;
                }
            }
        }
        if self.hotkeys_open && over(hotkeys_panel_rect(viewport)) {
            return true;
        }
        if let Some(rect) = self.minimap_rect() {
            if over([rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1]]) {
                return true;
            }
        }
        if self.dialog.is_some() && over(self.dialog_rect()) {
            return true;
        }
        false
    }

    /// Размер viewport в логических пикселях. Делитель — effective scale
    /// (R10: в desktop-режиме window.scale_factor() после репарентинга
    /// недостоверен — кнопки улетали за видимую область).
    fn viewport_logical(&self) -> Vec2 {
        match &self.window {
            Some(window) => {
                let size = window.inner_size();
                let scale = self.scale_factor();
                [size.width as f32 / scale, size.height as f32 / scale]
            }
            None => [0.0, 0.0],
        }
    }

    /// Позиция курсора в world-координатах.
    fn cursor_world(&self) -> Vec2 {
        self.camera
            .screen_to_world(self.cursor, self.viewport_logical())
    }

    /// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки
    /// под курсором (логические px окна), None — мимо всех бейджей. Зоны —
    /// с прошлого кадра (`expr_error_hits`); отставание в кадр незаметно.
    fn expr_error_hit_at(&self, cursor: [f32; 2]) -> Option<&LineErrorHit> {
        expr_error_hit_at(&self.expr_error_hits, cursor)
    }

    /// Клиентские ФИЗИЧЕСКИЕ px от shell (DragEvent) -> world-координаты:
    /// делим на scale_factor (масштаб учтён), затем через камеру (T9).
    fn drag_world_pt(&self, pt: (f32, f32)) -> Vec2 {
        let scale = self.scale_factor();
        let logical = [pt.0 / scale, pt.1 / scale];
        self.camera
            .screen_to_world(logical, self.viewport_logical())
    }

    /// События drag-drop (T9): превью зоны на Enter/Over, вставка нод на
    /// Drop. Данные приходят сырыми из shell, план строит canvas_app::ui.
    fn on_drag_event(&mut self, drag: canvas_shell::dragdrop::DragEvent) {
        use canvas_app::ui::{plan_drop, DropInsertKind};
        match drag {
            canvas_shell::dragdrop::DragEvent::Enter { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп одиночной папки с widget.json — призрак
                // установки виджета (перехват ДО plan_drop файлов)
                if let Some(src) = canvas_app::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            self.drop_preview = Some(DropPreview {
                                origin: world,
                                plan: vec![canvas_app::ui::DropInsert {
                                    id: "widget-install".to_owned(),
                                    kind: canvas_app::ui::DropInsertKind::InstallWidget(
                                        src,
                                        manifest.name.clone(),
                                    ),
                                    pos: world,
                                }],
                            });
                            self.request_redraw();
                            return;
                        }
                        // Битый манифест: честный призрак-ошибка + toast,
                        // как «файл недоступен» у битых ссылок (SPEC §7.5)
                        Err(e) => {
                            self.show_toast(format!("Виджет не установлен: {e}"));
                            self.drop_preview = None;
                            self.request_redraw();
                            return;
                        }
                    }
                }
                let plan = plan_drop(&self.scene.canvas, &data, world);
                // Пустой план (нет поддерживаемых форматов) — не подсвечиваем
                self.drop_preview = if plan.is_empty() {
                    None
                } else {
                    Some(DropPreview {
                        origin: world,
                        plan,
                    })
                };
            }
            canvas_shell::dragdrop::DragEvent::Over { client_pt } => {
                // Сетка призраков следует за курсором, сам план не меняется
                let world = self.drag_world_pt(client_pt);
                if let Some(preview) = self.drop_preview.as_mut() {
                    preview.origin = world;
                }
            }
            canvas_shell::dragdrop::DragEvent::Leave => self.drop_preview = None,
            canvas_shell::dragdrop::DragEvent::Drop { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп виджет-пакета — диалог П10 (Да/Нет), установка
                // и нода только после подтверждения; невалидный манифест —
                // toast (повторно не парсим успех — уже в призраке)
                if let Some(src) = canvas_app::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            let updating = self.widgets.registry.contains(&manifest.id);
                            self.dialog = Some(AppDialog::InstallWidget {
                                src,
                                manifest,
                                pos: world,
                                updating,
                            });
                        }
                        Err(e) => {
                            self.show_toast(format!("Виджет не установлен: {e}"));
                        }
                    }
                    self.drop_preview = None;
                    self.request_redraw();
                    return;
                }
                // План пересчитываем по СВЕЖИМ данным Drop (не из превью,
                // план T9 §5): источник мог обновить содержимое
                let plan = plan_drop(&self.scene.canvas, &data, world);
                if !plan.is_empty() {
                    // FR-006: дроп файлов/заметок — undo-шаг
                    self.push_undo();
                }
                let mut last: Option<usize> = None;
                for ins in plan {
                    let node = match ins.kind {
                        DropInsertKind::File(path) => Node::file(
                            ins.id,
                            path.to_string_lossy().into_owned(),
                            ins.pos[0],
                            ins.pos[1],
                            canvas_app::ui::DROP_CARD_W,
                            canvas_app::ui::DROP_CARD_H,
                        ),
                        DropInsertKind::Note(text) => {
                            Node::text(ins.id, text, ins.pos[0], ins.pos[1])
                        }
                        // Установка виджета перехвачена выше (T21-B: дроп
                        // открывает диалог, не вставляет ноду напрямую) —
                        // сюда попасть не можем; рамка на случай будущих
                        // прямых вставок (MCP widget_add — T22+)
                        DropInsertKind::InstallWidget(_, _) => {
                            tracing::warn!("дроп виджета прошёл мимо диалога — пропущен");
                            continue;
                        }
                    };
                    // Вставка как в create_note_at: модель + spatial index
                    self.scene.canvas.nodes.push(node);
                    let index = self.scene.canvas.nodes.len() - 1;
                    let node_ref = &self.scene.canvas.nodes[index];
                    self.scene.spatial.insert(index, node_ref);
                    last = Some(index);
                }
                if let Some(index) = last {
                    // Выделяем последнюю ноду группы; тамбнейлы закажет
                    // order_thumbnails в ближайшем кадре, автосейв — сам
                    self.scene.selected = Some(Selection::Node(index));
                    self.scene.mark_dirty();
                }
                // Поисковый индекс (T14): сброшенные файлы — сразу в FTS
                let canvas_dir = self.scene.canvas_dir();
                for node in &self.scene.canvas.nodes {
                    let Some(file) = node.file.as_ref() else {
                        continue;
                    };
                    self.search_service.command(SearchCommand::IndexFile {
                        path: resolve_node_path(file, &canvas_dir),
                        display_name: Path::new(file)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| file.clone()),
                    });
                }
                // Дроп мог добавить файловые ноды в новые директории —
                // синхронизируем вотчер (T10)
                self.sync_watch_dirs();
                self.drop_preview = None;
            }
        }
        self.request_redraw();
    }

    /// Синхронизировать вотчер с моделью (T10): родительские директории всех
    /// файловых нод → WatchService::sync_dirs (diff, повторный вызов — no-op),
    /// а на Windows — зеркало того же набора в SHChangeNotify-подписки шины
    /// T16 (R12). Вызывается после загрузки, дропа (T9), удаления нод и
    /// rename-событий.
    fn sync_watch_dirs(&mut self) {
        let dirs = watched_dirs(&self.scene.canvas, &self.scene.canvas_dir());
        self.watcher.sync_dirs(&dirs);
        // T16 (R12): зеркало того же набора в SHChangeNotify-подписки шины —
        // ЕДИНАЯ точка зеркалирования (план §3, все вызовы остаются как
        // есть); деградация шины — команды уходят впустую, молча (R14)
        #[cfg(windows)]
        if let Some(service) = &self.shell_events {
            service.command(canvas_shell::shell_events::window::ShellCommand::SyncFileDirs(dirs));
        }
    }

    /// Пересобрать/обновить миникарту (T13, SPEC §6.1): не каждый кадр, а по
    /// dirty-условиям — правки сцены (dirty_until save), движение камеры
    /// (пан/зум двигают рамку viewport) или смена размера буфера (DPI/resize).
    fn update_minimap(&mut self) {
        // Вычисления (immutable) — до mutable borrow рендерера
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let scale = self.scale_factor();
        let width_px = (MINIMAP_W as f32 * scale).round().max(1.0) as u32;
        let height_px = (MINIMAP_H as f32 * scale).round().max(1.0) as u32;
        let sig = (
            self.camera.position(),
            self.camera.zoom(),
            width_px,
            height_px,
        );
        let size_changed = self
            .minimap_sig
            .is_some_and(|prev| prev.2 != width_px || prev.3 != height_px);
        // dirty_until-автосейва: правки сцены пересобирают снимок; между
        // правкой и сейвом (2 с debounce) каждый запрошенный кадр обновляет
        // миникарту — это и есть видимость перемещений в реальном времени
        let scene_dirty = self.scene.dirty_since.is_some();
        if self.minimap.is_some() && self.minimap_sig == Some(sig) && !scene_dirty {
            return;
        }
        let viewport_world = self.camera.visible_world_rect(viewport);
        if self.minimap.is_none() || scene_dirty || size_changed {
            // сцена/размер изменились — полный снимок (T13-A)
            // FR-011: свернутые поддеревья не рисуются на миникарте
            let hidden = self.hidden_subtree_nodes();
            let scene_view = if hidden.is_empty() {
                self.scene.canvas.clone()
            } else {
                let mut filtered = self.scene.canvas.clone();
                filtered.nodes = filtered
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| hidden.binary_search(i).is_err())
                    .map(|(_, node)| node.clone())
                    .collect();
                filtered.edges.retain(|edge| {
                    let from_exists = filtered.nodes.iter().any(|node| node.id == edge.from_node);
                    let to_exists = filtered.nodes.iter().any(|node| node.id == edge.to_node);
                    from_exists && to_exists
                });
                filtered
            };
            self.minimap = Some(Minimap::capture(
                &scene_view,
                viewport_world,
                width_px,
                height_px,
            ));
        } else if let Some(minimap) = self.minimap.as_mut() {
            // только камера — пересчёт подгонки и рамки (дешевле снимка)
            minimap.set_viewport(viewport_world);
        }
        self.minimap_sig = Some(sig);
        // Загрузка текстуры (mutable borrow) — кадр растеризован заранее
        if let Some(minimap) = self.minimap.as_ref() {
            let image = minimap.render();
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.set_minimap(&image);
            }
        }
    }

    /// Прямоугольник миникарты в логических px (T13): None — не задана или
    /// окно меньше 252×172 (квад скрыт).
    fn minimap_rect(&self) -> Option<[f32; 4]> {
        self.renderer
            .as_ref()
            .and_then(|renderer| renderer.minimap_rect_logical())
    }

    /// Центрировать камеру на world-точке под курсором мыши в миникарте
    /// (T13): клик — прыжок, drag — world-точка следует за курсором.
    fn center_camera_on_minimap_cursor(&mut self) {
        let Some(minimap) = self.minimap.as_ref() else {
            return;
        };
        let Some(rect) = self.minimap_rect() else {
            return;
        };
        // rect — логические px, маппинг минимапы — в физических буфера
        let scale = self.scale_factor();
        let px = [
            (self.cursor[0] - rect[0]) * scale,
            (self.cursor[1] - rect[1]) * scale,
        ];
        self.camera.set_center(minimap.map_to_world(px));
    }

    /// Ответ поискового индекса (T14): результаты FTS + заметки → строки.
    fn on_search_event(&mut self, event: SearchEvent) {
        match event {
            SearchEvent::Ready(hits) => self.apply_search_hits(hits),
            SearchEvent::Indexed(count) => tracing::debug!(count, "поисковый индекс обновлён"),
        }
    }

    /// Склейка результатов (T14): FTS-хиты (bm25, путь → нода через
    /// path_matches) + in-memory substring по заметкам и именам нод
    /// (заметок без файла в индексе нет). Дедуп — по ноде.
    /// FR-011: ноды свернутых поддеревьев из результатов исключены.
    fn apply_search_hits(&mut self, hits: Vec<SearchHit>) {
        let canvas_dir = self.scene.canvas_dir();
        let hidden = self.hidden_subtree_nodes();
        let mut nodes: Vec<usize> = Vec::new();
        let mut rows: Vec<SearchRow> = Vec::new();
        for hit in &hits {
            let index = self.scene.canvas.nodes.iter().position(|node| {
                node.file
                    .as_ref()
                    .is_some_and(|file| path_matches(file, &canvas_dir, &hit.path))
            });
            let Some(index) = index else {
                continue; // файл не на канвасе — строка не показывается
            };
            if hidden.binary_search(&index).is_ok() {
                continue; // FR-011: свернутая ветка не ищется
            }
            if nodes.contains(&index) {
                continue;
            }
            nodes.push(index);
            rows.push(SearchRow {
                title: hit.display_name.clone(),
                subtitle: hit_subtitle(&hit.path),
            });
        }
        // In-memory: заметки и имена нод вне FTS-индекса (T14 §3)
        let query = self.search.input.query().to_owned();
        if !query.is_empty() {
            let entries: Vec<SceneEntry<'_>> = self
                .scene
                .canvas
                .nodes
                .iter()
                .enumerate()
                .filter(|(index, _)| !nodes.contains(index))
                .map(|(index, node)| SceneEntry {
                    node: index,
                    title: node_title(node),
                    text: node_text(node),
                })
                .collect();
            for hit in scan_scene(&query, &entries) {
                // FR-011: свернутые ветки в поиске не участвуют
                if hidden.binary_search(&hit).is_ok() {
                    continue;
                }
                let Some(node) = self.scene.canvas.nodes.get(hit) else {
                    continue;
                };
                nodes.push(hit);
                rows.push(SearchRow {
                    title: node_title(node).to_owned(),
                    subtitle: node_subtitle(node).to_owned(),
                });
            }
        }
        self.search.set_results(rows);
        self.search_nodes = nodes;
        self.request_redraw();
    }

    /// Правка поля запроса (T14): любое изменение перезапускает debounce.
    fn edit_search_input(&mut self, apply: impl FnOnce(&mut SearchInput) -> bool) {
        let changed = apply(&mut self.search.input);
        if changed {
            self.search_pending = Some((self.search.input.query().to_owned(), Instant::now()));
            self.request_redraw();
        }
    }

    /// Клавиатура открытой панели поиска (T14): ввод, каретка, выбор, прыжок.
    fn on_search_key(&mut self, event: &KeyEvent) {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        // Повторное Ctrl+F — очистить поле (первое — открытие с прошлым
        // запросом, ввод замещает его только после очистки)
        if ctrl
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("а"))
        {
            self.search.input.set_query("");
            self.search_pending = Some((String::new(), Instant::now()));
            self.request_redraw();
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.search.close();
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(PanelAction::Jump(row)) = self.search.confirm() {
                    self.jump_to_search_row(row);
                }
            }
            Key::Named(NamedKey::F3) => {
                self.cycle_search(if shift { -1 } else { 1 });
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.search.move_selection(-1);
                self.search.ensure_selection_visible();
                self.request_redraw();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.search.move_selection(1);
                self.search.ensure_selection_visible();
                self.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                self.edit_search_input(|input| input.backspace(ctrl));
            }
            Key::Named(NamedKey::Delete) => {
                self.edit_search_input(SearchInput::delete);
            }
            Key::Named(NamedKey::ArrowLeft) if !ctrl => {
                self.search.input.move_left();
                self.request_redraw();
            }
            Key::Named(NamedKey::ArrowRight) if !ctrl => {
                self.search.input.move_right();
                self.request_redraw();
            }
            Key::Named(NamedKey::Home) => {
                self.search.input.move_to_start();
                self.request_redraw();
            }
            Key::Named(NamedKey::End) => {
                self.search.input.move_to_end();
                self.request_redraw();
            }
            Key::Character(text) => {
                self.edit_search_input(|input| {
                    input.insert_str(text);
                    true
                });
            }
            _ => {}
        }
    }

    /// F3/Shift+F3 (T14): цикл по результатам с прыжком; работает и после
    /// закрытия панели (rows сохранены).
    fn cycle_search(&mut self, delta: i32) {
        if self.search.rows.is_empty() {
            return;
        }
        self.search.move_selection(delta);
        self.search.ensure_selection_visible();
        if let Some(PanelAction::Jump(row)) = self.search.confirm() {
            self.jump_to_search_row(row);
        } else {
            self.request_redraw();
        }
    }

    /// Прыжок к строке результата (T14): полёт камеры 300 мс ease-out,
    /// целевой зум не ниже 0.8 (нода читаема), пульс подсветки.
    fn jump_to_search_row(&mut self, row: usize) {
        let Some(&node) = self.search_nodes.get(row) else {
            return;
        };
        let Some(target) = self.scene.canvas.nodes.get(node) else {
            return;
        };
        let center = [
            target.x + target.width / 2.0,
            target.y + target.height / 2.0,
        ];
        let target_zoom = self.camera.zoom().max(0.8);
        self.flight = Some((
            Flight::new(
                self.camera.position(),
                self.camera.zoom(),
                center,
                target_zoom,
                FLIGHT_DURATION_MS,
            ),
            Instant::now(),
        ));
        self.pulse = Some((node, Instant::now()));
        self.request_redraw();
    }

    /// Оверлей панели поиска (T14): квады + тексты в screen-space
    /// (FrameOverlay), геометрия — search_ui::layout.
    fn search_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if !self.search.is_open() || viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let lay = search_layout(viewport[0], viewport[1], &self.search);
        let palette = ThemeColors::from_theme(self.settings.theme);
        let panel = rect_xywh(lay.panel_rect);
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.9],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        let input = rect_xywh(lay.input_rect);
        instances.push(CardInstance {
            pos: [input[0], input[1]],
            size: [input[2], input[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        // Каретка — литерал «|» в конце текста (MVP, без мерцания)
        let query_with_caret = format!("{}|", self.search.input.query());
        texts.push(OwnedScreenText {
            text: query_with_caret,
            origin: [input[0] + 10.0, input[1] + 9.0],
            width: (input[2] - 20.0).max(10.0),
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        for (visible, rect) in lay.row_rects.iter().enumerate() {
            let row = self.search.scroll_top + visible;
            let Some(entry) = self.search.rows.get(row) else {
                break;
            };
            let selected = self.search.selected == Some(row);
            let row_rect = rect_xywh(*rect);
            // Hover-подсветка результата (не выбранного — выделенный несёт
            // акцент): практика списков результатов (VS Code)
            let row_hover = point_in_rect(row_rect, self.cursor);
            instances.push(CardInstance {
                pos: [row_rect[0], row_rect[1]],
                size: [row_rect[2], row_rect[3]],
                fill: if selected {
                    [0.18, 0.29, 0.48, 0.95]
                } else if row_hover {
                    [0.24, 0.30, 0.42, 0.6]
                } else {
                    palette.search_row_fill
                },
                border: [0.0; 4],
                params: [4.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: entry.title.clone(),
                origin: [row_rect[0] + 10.0, row_rect[1] + 4.0],
                width: (row_rect[2] - 20.0).max(10.0),
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: entry.subtitle.clone(),
                origin: [row_rect[0] + 10.0, row_rect[1] + 18.0],
                width: (row_rect[2] - 20.0).max(10.0),
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
    }

    // --- FR-018: шаблоны ---

    /// Инстанцировать шаблон в world-точке (undo-шаг, выделение, пересчёт
    /// потока). Возвращает индекс новой ноды. Дефолты манифеста всегда в
    /// границах — Result разворачивается (ошибка границ возможна только для
    /// переопределений MCP).
    fn instantiate_template_at(
        &mut self,
        manifest: &canvas_core::templates::TemplateManifest,
        world: Vec2,
    ) -> usize {
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "tpl");
        let mut node =
            canvas_core::templates::instantiate(manifest, &BTreeMap::new(), id, world[0], world[1])
                .expect("дефолты манифеста в границах");
        // FR-023: авто-высота шаблонной ноды при инстанциации — по числу
        // строк листа параметров: шапка + тело + футер результата. Новая
        // нода сразу влезает целиком (без «подгонки правкой»).
        fit_template_node_height(&mut node);
        self.scene.canvas.nodes.push(node);
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.scene.selected = Some(Selection::Node(index));
        self.scene.selected_nodes.clear();
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        index
    }

    /// FR-021: пересчитать состояние popup подсказок после правки текста.
    /// Popup открывается только на Numi-строках каретки (вердикт
    /// `expr::line_kind`, вне код-фенсов) при непустом списке вариантов;
    /// якорь — низ каретки в логических px окна.
    fn update_hints(&mut self) {
        let Some(session) = self.editing.as_ref() else {
            self.hints.reset();
            return;
        };
        // Подсказки — только в тексте ноды (лейблы связей не Numi-редактор)
        if session.node_index().is_none() {
            self.hints.reset();
            return;
        }
        let (line_i, line_text, caret) = session.caret_line();
        let prefix = &line_text[..caret.min(line_text.len())];
        // Код-фенсы выше строки каретки (``` toggling, как eval_lines)
        let full_text = session.text();
        let in_fence = full_text
            .split('\n')
            .take(line_i)
            .fold(false, |fence, line| {
                fence ^ line.trim_start().starts_with("```")
            });
        if in_fence
            || !matches!(
                expr::line_kind(prefix),
                expr::NumiLineKind::Assignment { .. } | expr::NumiLineKind::Expression
            )
        {
            self.hints.reset();
            return;
        }
        // Контекст ноды: переменные выше, value-входы, параметры шаблона
        let vars: Vec<String> = full_text
            .split('\n')
            .take(line_i)
            .filter_map(|line| match expr::line_kind(line) {
                expr::NumiLineKind::Assignment { name } => Some(name),
                _ => None,
            })
            .collect();
        let ctx = hints_ui::HintContext {
            vars,
            inbound: self
                .scene
                .canvas
                .edges
                .iter()
                .filter(|edge| {
                    edge.to_node
                        == self
                            .scene
                            .canvas
                            .nodes
                            .get(session.node_index().unwrap_or(usize::MAX))
                            .map(|node| node.id.clone())
                            .unwrap_or_default()
                        && edge.flow_kind() == FlowKind::Value
                })
                .count(),
            params: self
                .scene
                .canvas
                .nodes
                .get(session.node_index().unwrap_or(usize::MAX))
                .and_then(|node| node.template())
                .map(|template| template.params.keys().cloned().collect())
                .unwrap_or_default(),
        };
        let items = hints_ui::hint_items(prefix, &ctx);
        let token = hints_ui::token_before_caret(prefix, prefix.len()).0;
        self.hints.sync(token, items);
        // Якорь — низ каретки (screen logical px): world-область тела ноды
        // + позиция каретки в буфере (физ. px)
        if self.hints.open {
            let caret_rect = if let (Some(session), Some(renderer)) =
                (self.editing.as_mut(), self.renderer.as_mut())
            {
                session.caret_rect(renderer.font_system_mut())
            } else {
                None
            };
            if let (Some(session), Some(rect)) = (self.editing.as_ref(), caret_rect) {
                if let Some((origin, _, _)) =
                    session_area(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
                {
                    let screen = self.camera.world_to_screen(origin, self.viewport_logical());
                    let scale = self.scale_factor();
                    self.hints.anchor = [
                        screen[0] + rect[0] / scale,
                        screen[1] + (rect[1] + rect[3]) / scale,
                    ];
                }
            }
        }
    }

    /// FR-021: принять выбранную подсказку — заменить токен слева от
    /// каретки текстом вставки. НЕ коммитит заметку; после вставки
    /// пересчитать высоту и список подсказок.
    fn accept_hint(&mut self) {
        let Some(item) = self.hints.selected_item().cloned() else {
            return;
        };
        let token = self.hints.token.clone();
        let applied = if let (Some(session), Some(renderer)) =
            (self.editing.as_mut(), self.renderer.as_mut())
        {
            session.replace_token_before_caret(renderer.font_system_mut(), &token, &item.insert);
            true
        } else {
            false
        };
        if applied {
            self.fit_note_size();
            self.update_hints();
            self.request_redraw();
        }
    }

    /// FR-021: оверлей popup подсказок — подложка + строки
    /// (имя + серая деталь), выделение акцентом. Паттерн wheel_overlay.
    fn hints_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        if !self.hints.open || self.hints.items.is_empty() {
            return (instances, texts);
        }
        let palette = ThemeColors::from_theme(self.settings.theme);
        let viewport = self.viewport_logical();
        let [px, py, pw, ph] =
            hints_ui::popup_layout(self.hints.anchor, viewport, self.hints.items.len());
        if pw <= 0.0 {
            return (instances, texts);
        }
        instances.push(CardInstance {
            pos: [px, py],
            size: [pw, ph],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.95],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        for (i, item) in self.hints.items.iter().enumerate() {
            let row_y = py + hints_ui::HINT_MARGIN + i as f32 * hints_ui::HINT_ROW_H;
            if i == self.hints.selected {
                instances.push(CardInstance {
                    pos: [px + 4.0, row_y],
                    size: [pw - 8.0, hints_ui::HINT_ROW_H],
                    fill: [0.18, 0.29, 0.48, 0.95],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: item.label.clone(),
                origin: [px + 10.0, row_y + 4.0],
                width: 118.0,
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: item.detail.clone(),
                origin: [px + 134.0, row_y + 6.0],
                width: (pw - 142.0).max(20.0),
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
    }

    /// Клавиатура открытой палитры шаблонов (Ctrl+P): ввод фильтра,
    /// стрелки/Enter/Esc. Вызывается из on_key, когда панель открыта.
    /// true — клавиша потреблена панелью.
    fn on_template_panel_key(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return true; // отпускания глотаются — канвасу не достаются
        }
        if event.logical_key == Key::Named(NamedKey::Escape) && !event.repeat {
            self.template_panel.close();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Enter) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            // FR-024: выделение — ординал среди строк-шаблонов (секции —
            // заголовки, не цели)
            if let Some(row_idx) = template_row_of_ordinal(&rows, self.template_panel.selected) {
                if let template_ui::PanelRow::Template(index) = rows[row_idx] {
                    let manifest = self.templates.list()[index].clone();
                    let center = self.viewport_center_world();
                    self.template_panel.close();
                    self.instantiate_template_at(&manifest, center);
                    self.request_redraw();
                }
            }
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowDown) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            self.template_panel.move_selection(1, &rows);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowUp) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            self.template_panel.move_selection(-1, &rows);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Backspace) && !event.repeat {
            self.template_panel.backspace();
            self.template_panel.selected = 0;
            self.template_panel.scroll_top = 0;
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowLeft) && !event.repeat {
            self.template_panel.move_left();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowRight) && !event.repeat {
            self.template_panel.move_right();
            self.request_redraw();
            return true;
        }
        // Печатаемый символ (включая кириллицу — logical_key уже раскладка)
        if let Key::Character(text) = &event.logical_key {
            if !event.repeat && !self.modifiers.control_key() {
                self.template_panel.insert_str(text.as_str());
                self.template_panel.selected = 0;
                self.template_panel.scroll_top = 0;
                self.request_redraw();
                return true;
            }
        }
        true
    }

    /// Оверлей боковой палитры шаблонов (FR-018, Ctrl+P): панель у правого
    /// края, поле фильтра, чипы категорий, строки шаблонов с квад-иконками
    /// и описанием (паттерн search_overlay).
    /// Оверлей боковой палитры шаблонов (FR-018, Ctrl+P). FR-024 — стиль
    /// Miro Template picker: левый док во всю высоту (чистая геометрия —
    /// `template_ui::panel_layout`), плотная подложка с рамкой, шапка
    /// «Шаблоны», поиск с placeholder, чипы категорий, секции с
    /// заголовками, строки-карточки (подложка + плитка иконки + имя +
    /// описание), hover/выбранное состояние, футер-подсказка.
    fn template_panel_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        if !self.template_panel.open {
            return (instances, texts);
        }
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let rows = template_panel_rows(&self.templates, &self.template_panel);
        let lay = template_panel_layout(
            viewport[0],
            viewport[1],
            &self.templates,
            &self.template_panel,
            &rows,
        );
        let palette = ThemeColors::from_theme(self.settings.theme);
        let icon_tint = color_to_rgba(palette.icon);
        let panel = rect_xywh(lay.panel_rect);
        // Подложка дока: плотная, с рамкой (отделяет панель от канваса)
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.9],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        // Шапка: название + счётчик шаблонов
        let total = template_ui::template_row_count(&rows);
        texts.push(OwnedScreenText {
            text: "Шаблоны".to_owned(),
            origin: [lay.header_rect[0], lay.header_rect[1] + 6.0],
            width: lay.header_rect[2] * 0.5,
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        texts.push(OwnedScreenText {
            text: format!("{total}"),
            origin: [
                lay.header_rect[0] + lay.header_rect[2] * 0.5,
                lay.header_rect[1] + 8.0,
            ],
            width: lay.header_rect[2] * 0.5 - 4.0,
            font_size: 11.0,
            color: palette.body,
            align: TextAlign::Center,
        });
        // Поле фильтра: placeholder при пустом вводе, иначе текст с кареткой
        let input = rect_xywh(lay.input_rect);
        instances.push(CardInstance {
            pos: [input[0], input[1]],
            size: [input[2], input[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: if self.template_panel.filter.is_empty() {
                "Поиск шаблонов…".to_owned()
            } else {
                format!("{}|", self.template_panel.filter)
            },
            origin: [input[0] + 10.0, input[1] + 8.0],
            width: (input[2] - 20.0).max(10.0),
            font_size: 13.0,
            color: if self.template_panel.filter.is_empty() {
                palette.body
            } else {
                palette.title
            },
            align: TextAlign::Left,
        });
        // Чипы категорий
        for (rect, name, active) in &lay.category_rects {
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if *active {
                    [0.18, 0.29, 0.48, 0.95]
                } else {
                    [0.17, 0.18, 0.22, 0.8]
                },
                border: [0.0; 4],
                params: [11.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: name.clone(),
                origin: [rect[0] + 10.0, rect[1] + 6.0],
                width: rect[2] - 12.0,
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // Строки: секции-заголовки и карточки шаблонов (Miro-стиль)
        for (row_i, (rect, row)) in lay.row_rects.iter().zip(lay.rows.iter()).enumerate() {
            match row {
                PanelRow::Section(name) => {
                    texts.push(OwnedScreenText {
                        text: name.clone(),
                        origin: [rect[0] + 2.0, rect[1] + 5.0],
                        width: rect[2] - 4.0,
                        font_size: 11.0,
                        color: palette.body,
                        align: TextAlign::Left,
                    });
                }
                PanelRow::Template(index) => {
                    let Some(manifest) = self.templates.list().get(*index) else {
                        continue;
                    };
                    // Ординал строки среди шаблонов (секции не считаются)
                    let ordinal = lay.rows[..row_i]
                        .iter()
                        .filter(|other| matches!(other, PanelRow::Template(_)))
                        .count();
                    let selected = self.template_panel.selected == ordinal;
                    let row_rect = rect_xywh(*rect);
                    let row_hover = point_in_rect(row_rect, self.cursor);
                    // Подложка-карточка строки (Miro: карточка с фоном)
                    instances.push(CardInstance {
                        pos: [row_rect[0], row_rect[1]],
                        size: [row_rect[2], row_rect[3]],
                        fill: if selected {
                            [0.18, 0.29, 0.48, 0.95]
                        } else if row_hover {
                            [0.24, 0.30, 0.42, 0.6]
                        } else {
                            [0.13, 0.14, 0.18, 0.65]
                        },
                        border: if selected || row_hover {
                            [0.30, 0.42, 0.65, 0.9]
                        } else {
                            [0.0; 4]
                        },
                        params: [6.0, 0.0, 0.0, 1.0],
                    });
                    // Плитка иконки (скруглённый квадрат) + квад-иконка роли
                    let tile = [
                        row_rect[0] + 8.0,
                        row_rect[1] + (row_rect[3] - template_ui::TEMPLATE_ROW_TILE) / 2.0,
                        template_ui::TEMPLATE_ROW_TILE,
                        template_ui::TEMPLATE_ROW_TILE,
                    ];
                    instances.push(CardInstance {
                        pos: [tile[0], tile[1]],
                        size: [tile[2], tile[3]],
                        fill: [0.20, 0.22, 0.28, 0.9],
                        border: [0.0; 4],
                        params: [6.0, 0.0, 0.0, 1.0],
                    });
                    instances.extend(template_icon_quads(
                        template_ui::icon_key(manifest),
                        [
                            tile[0] + (tile[2] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
                            tile[1] + (tile[3] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
                            template_ui::TEMPLATE_ROW_ICON,
                            template_ui::TEMPLATE_ROW_ICON,
                        ],
                        icon_tint,
                    ));
                    texts.push(OwnedScreenText {
                        text: manifest.display_name().to_owned(),
                        origin: [tile[0] + tile[2] + 8.0, row_rect[1] + 5.0],
                        width: row_rect[2] - (tile[2] + 24.0),
                        font_size: 13.0,
                        color: palette.title,
                        align: TextAlign::Left,
                    });
                    texts.push(OwnedScreenText {
                        text: manifest.description.clone(),
                        origin: [tile[0] + tile[2] + 8.0, row_rect[1] + 21.0],
                        width: row_rect[2] - (tile[2] + 24.0),
                        font_size: 11.0,
                        color: palette.body,
                        align: TextAlign::Left,
                    });
                }
            }
        }
        // Футер-подсказка (низ панели)
        texts.push(OwnedScreenText {
            text: "Enter — вставить в центр · Esc — закрыть".to_owned(),
            origin: [
                panel[0] + template_ui::PANEL_PADDING,
                panel[1] + panel[3] - 22.0,
            ],
            width: panel[2] - template_ui::PANEL_PADDING * 2.0,
            font_size: 10.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        (instances, texts)
    }

    /// Оверлей радиального wheel-меню шаблонов (FR-018, Shift+клик):
    /// плашки-мини-карточки (шаблоны + категории) из чистой геометрии
    /// `template_ui::wheel_geometry` — раскладка отталкивается от размера
    /// плашек, зазор гарантирован (правка владельца 2026-09-16). Пайплайн
    /// квадов без поворотов; hover — по тем же плашкам (WYSIWYG).
    /// FR-022 (бест-практики радиальных меню): затемнение фона под
    /// модальным пикером (паттерн Miro Template picker), круглая кнопка
    ///-хаб «назад/закрыть» (Kurtenbach/Buxton — центр отменяет уровень),
    /// крошки глубины в хабе (выбранная категория).
    fn wheel_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.wheel_menu else {
            return (instances, texts);
        };
        let palette = ThemeColors::from_theme(self.settings.theme);
        let icon_tint = color_to_rgba(palette.icon);
        let [vw, vh] = self.viewport_logical();
        // Затемнение фона: фокус на выборе, случайный клик по канвасу
        // исключён (клик мимо wheel закрывает меню — обработчик клика)
        instances.push(CardInstance {
            pos: [0.0, 0.0],
            size: [vw, vh],
            fill: [0.0, 0.0, 0.0, 0.35],
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 1.0],
        });
        let categories = self.templates.categories();
        let templates: Vec<_> = menu
            .category
            .as_deref()
            .map(|c| self.templates.by_category(c))
            .unwrap_or_default();
        let geo =
            template_ui::wheel_geometry(menu.screen, vw, vh, categories.len(), templates.len());
        let hovered = geo.hit(self.cursor);
        for plate in &geo.plates {
            let [qx, qy, w, h] = plate.rect;
            let active = hovered.as_ref() == Some(&plate.hit);
            let fill = if active {
                [0.18, 0.29, 0.48, 0.95]
            } else {
                match plate.hit {
                    WheelHit::Category(_) => [0.17, 0.18, 0.22, 0.92],
                    WheelHit::Template(_) => [0.20, 0.22, 0.27, 0.92],
                }
            };
            instances.push(CardInstance {
                pos: [qx, qy],
                size: [w, h],
                fill,
                border: [0.22, 0.24, 0.30, 0.9],
                params: [8.0, 0.0, 0.0, 1.0],
            });
            match plate.hit {
                WheelHit::Category(i) => {
                    texts.push(OwnedScreenText {
                        text: categories[i].to_owned(),
                        origin: [qx + 4.0, qy + h / 2.0 - 7.0],
                        width: w - 8.0,
                        font_size: 12.0,
                        color: palette.title,
                        align: TextAlign::Center,
                    });
                }
                WheelHit::Template(i) => {
                    let Some(manifest) = templates.get(i) else {
                        continue;
                    };
                    // Квад-иконка роли слева, имя справа (1–2 строки)
                    instances.extend(template_icon_quads(
                        template_ui::icon_key(manifest),
                        [qx + 8.0, qy + h / 2.0 - 8.0, 16.0, 16.0],
                        icon_tint,
                    ));
                    let (line1, line2) =
                        split_two_lines(manifest.display_name(), template_ui::WHEEL_TPL_TEXT_CHARS);
                    let push_line = |text: String, dy: f32| OwnedScreenText {
                        text,
                        origin: [qx + 30.0, qy + h / 2.0 + dy],
                        width: w - 36.0,
                        font_size: 11.0,
                        color: palette.title,
                        align: TextAlign::Left,
                    };
                    match line2 {
                        None => texts.push(push_line(line1, -7.0)),
                        Some(line2) => {
                            texts.push(push_line(line1, -14.0));
                            texts.push(push_line(line2, 0.0));
                        }
                    }
                }
            }
        }
        // Хаб: круглая кнопка «назад/закрыть» (FR-022). Без категории —
        // подсказка «закрыть»; с категорией — крошки глубины «← имя»
        let [hx, hy, hw, hh] = geo.hub;
        instances.push(CardInstance {
            pos: [hx, hy],
            size: [hw, hh],
            fill: if menu.category.is_some() {
                [0.18, 0.29, 0.48, 0.95]
            } else {
                [0.17, 0.18, 0.22, 0.92]
            },
            border: [0.22, 0.24, 0.30, 0.9],
            params: [hw / 2.0, 0.0, 0.0, 1.0], // круг — радиус = половина стороны
        });
        texts.push(OwnedScreenText {
            text: if let Some(category) = &menu.category {
                format!("← {category}")
            } else {
                "закрыть".to_owned()
            },
            origin: [geo.center[0] - 44.0, geo.center[1] - 6.0],
            width: 88.0,
            font_size: 10.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        (instances, texts)
    }

    /// Батч событий файловой системы (T10): применение к модели — в чистой
    /// canvas_core::apply_file_events, здесь — платформенные реакции: сброс
    /// тамбнейл-кэшей и негативного кэша, автосейв, пересборка вотчеров.
    fn on_file_events(&mut self, events: Vec<FileEvent>) {
        if events.is_empty() {
            return;
        }
        let canvas_dir = self.scene.canvas_dir();
        let changes = apply_file_events(&mut self.scene.canvas, &canvas_dir, &events);
        if changes.is_empty() {
            return; // чужие файлы в наблюдаемых папках — частый случай
        }
        tracing::debug!(
            events = events.len(),
            changes = changes.len(),
            "события файловой системы применены"
        );
        let mut invalidate_thumbs = false;
        let mut dirty = false;
        let mut resync = false;
        for change in changes {
            match change {
                // Modify (и atomic-save): атлас и SQLite-кэш перезапросятся,
                // неудавшийся тамбнейл — перезапросить
                NodeChange::ThumbStale(index) => {
                    invalidate_thumbs = true;
                    self.thumbs_failed.remove(&index);
                }
                // Путь обновлён: автосейв + возможно новая директория вотчинга
                NodeChange::PathUpdated(_) => {
                    dirty = true;
                    resync = true;
                }
                NodeChange::Broken(_) => {}
                // Восстановление: неудавшийся тамбнейл можно перезапросить
                NodeChange::Restored(index) => {
                    invalidate_thumbs = true;
                    self.thumbs_failed.remove(&index);
                }
            }
        }
        if invalidate_thumbs {
            if let Some(renderer) = self.renderer.as_mut() {
                // Полный сброс: ключ атласа — индекс ноды, точечного удаления
                // нет; SQLite промахнётся по mtime сам (ключ — путь+mtime)
                renderer.invalidate_node_caches();
            }
        }
        if dirty {
            self.scene.mark_dirty();
        }
        if resync {
            self.sync_watch_dirs();
        }
        // Поисковый индекс (T14): события ФС — только по путям нод канваса
        // (чужие файлы в наблюдаемых папках в индекс не попадают)
        {
            let canvas_dir = self.scene.canvas_dir();
            let node_path_matches = |path: &Path| {
                self.scene.canvas.nodes.iter().any(|node| {
                    node.file
                        .as_ref()
                        .is_some_and(|file| path_matches(file, &canvas_dir, path))
                })
            };
            for event in &events {
                match event {
                    FileEvent::Create(path) | FileEvent::Modify(path) => {
                        if node_path_matches(path) {
                            self.search_service.command(SearchCommand::IndexFile {
                                path: path.clone(),
                                display_name: Path::new(path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    FileEvent::Rename(from, to) => {
                        if node_path_matches(from) || node_path_matches(to) {
                            self.search_service
                                .command(SearchCommand::RemoveFile { path: from.clone() });
                            self.search_service.command(SearchCommand::IndexFile {
                                path: to.clone(),
                                display_name: Path::new(to)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    FileEvent::Remove(path) => {
                        if node_path_matches(path) {
                            self.search_service
                                .command(SearchCommand::RemoveFile { path: path.clone() });
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Строка HUD (F3): fps, p95 frame time, счётчик culling последнего кадра.
    /// Рендер идёт по request_redraw, поэтому fps осмыслен во время активного
    /// пан/зума; в простое кадры не рисуются и замер не обновляется.
    fn hud_text(&self) -> Option<String> {
        if !self.hud_visible {
            return None;
        }
        let fps = self
            .frame_meter
            .fps()
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "—".into());
        let p95 = self
            .frame_meter
            .p95_ms()
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "—".into());
        let thumbs = self
            .renderer
            .as_ref()
            .map(|r| r.thumbnail_count())
            .unwrap_or(0);
        Some(format!(
            "{fps} fps | p95 {p95} мс | кадр {:.1} мс | нод видно {}/{} | связей видно {}/{} | инстансов {} | тамбнейлов {} (очередь {})",
            self.last_stats.cpu_ms,
            self.last_stats.visible_nodes,
            self.last_stats.total_nodes,
            self.last_stats.visible_edges,
            self.last_stats.total_edges,
            self.last_stats.instances,
            thumbs,
            self.thumbs.queue_len()
        ))
    }

    /// Оверлей контекстного меню пустого канваса (T7): фон, подписи.
    /// Screen-space — логические px, константный читаемый размер при любом
    /// зуме (уточнение владельца). Меню ноды/связи заменены палитрой.
    fn canvas_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.menu else {
            return (instances, texts);
        };
        let palette = ThemeColors::from_theme(self.settings.theme);
        let [x, y, w, h] = menu_rect_for(menu.origin, CANVAS_MENU_ITEMS.len());
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 0.0],
        });
        // Hover-подсветка пункта (аффорданс — как строки палитры/поиска:
        // интерактивный элемент отвечает на курсор)
        let hovered_item = menu_item_at_for(menu.origin, self.cursor, CANVAS_MENU_ITEMS.len());
        for (i, item) in CANVAS_MENU_ITEMS.iter().enumerate() {
            let rect = menu_item_rect(menu.origin, i);
            if hovered_item == Some(i) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: [0.24, 0.30, 0.42, 0.9],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: canvas_menu_label(
                    *item,
                    self.settings.focus_mode,
                    self.hotkeys_open,
                    self.desktop_menu_checked(),
                ),
                origin: [rect[0] + MENU_LABEL_X, rect[1] + 5.0],
                width: rect[2] - MENU_LABEL_X,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // M5 (T20-F): колонка подменю «Виджеты ▸» — справа от меню
        if let Some(submenu) = &menu.submenu {
            let [sx, sy, sw, sh] = submenu_rect(submenu);
            instances.push(CardInstance {
                pos: [sx, sy],
                size: [sw, sh],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 0.0],
            });
            if submenu.entries.is_empty() {
                texts.push(OwnedScreenText {
                    text: "(нет установленных)".to_owned(),
                    origin: [
                        submenu.origin[0] + MENU_PADDING + 4.0,
                        submenu.origin[1] + MENU_PADDING + 5.0,
                    ],
                    width: MENU_WIDTH - MENU_PADDING * 2.0 - 8.0,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            } else {
                let hovered_sub = submenu_item_at(submenu, self.cursor);
                for (i, entry) in submenu.entries.iter().enumerate() {
                    let rect = menu_item_rect(submenu.origin, i);
                    if hovered_sub == Some(i) {
                        instances.push(CardInstance {
                            pos: [rect[0], rect[1]],
                            size: [rect[2], rect[3]],
                            fill: [0.24, 0.30, 0.42, 0.9],
                            border: [0.0; 4],
                            params: [4.0, 0.0, 0.0, 1.0],
                        });
                    }
                    texts.push(OwnedScreenText {
                        text: entry.label.clone(),
                        origin: [rect[0] + MENU_LABEL_X, rect[1] + 5.0],
                        width: rect[2] - MENU_LABEL_X,
                        font_size: 13.0,
                        color: palette.title,
                        align: TextAlign::Left,
                    });
                }
            }
        }
        (instances, texts)
    }

    // --- Палитра выделения (FR-009/FR-010) ---

    /// Цель палитры из текущего выделения; None — палитра скрыта
    /// (нет выделения, drag/редактирование/поиск/диалог/рамка выделения).
    fn palette_target(&self) -> Option<PaletteTarget> {
        if self.dialog.is_some()
            || self.search.is_open()
            || self.editing.is_some()
            || self.scene.dragging.is_some()
            || self.edge_drag.is_some()
            || self.select_rect.is_some()
            // Взаимоисключение поповеров: открытое меню канваса прячет
            // палитру (практика UI: transient-поповер один за раз;
            // обратное направление — RMB по ноде закрывает меню)
            || self.menu.is_some()
        {
            return None;
        }
        // Список выделенных нод: primary — одиночное/составное выделение
        // или первый из мультивыделения рамкой (CR-001, там selected = None)
        let selected: Vec<usize> = match self.scene.selected {
            Some(Selection::Node(primary)) => {
                let mut selected = vec![primary];
                for &index in &self.scene.selected_nodes {
                    if !selected.contains(&index) && self.scene.canvas.nodes.get(index).is_some() {
                        selected.push(index);
                    }
                }
                selected
            }
            None if !self.scene.selected_nodes.is_empty() => self
                .scene
                .selected_nodes
                .iter()
                .copied()
                .filter(|&index| self.scene.canvas.nodes.get(index).is_some())
                .collect(),
            _ => Vec::new(),
        };
        if selected.is_empty() {
            return match self.scene.selected {
                Some(Selection::Edge(edge_index))
                    if self.scene.canvas.edges.get(edge_index).is_some() =>
                {
                    Some(PaletteTarget::Edge(edge_index))
                }
                _ => None,
            };
        }
        // Primary — первый из списка (для одиночного выделения он и есть
        // единственный; для рамки — первый по порядку выделения)
        let primary = selected[0];
        Some(PaletteTarget::Nodes { primary, selected })
    }

    /// Screen-якорь палитры: низ bbox выделенных нод (или середина связи),
    /// в логических px; None — цель без геометрии.
    fn palette_anchor_screen(&self, target: &PaletteTarget) -> Option<[f32; 2]> {
        let viewport = self.viewport_logical();
        let gap = canvas_app::palette::PAL_ANCHOR_GAP;
        match target {
            PaletteTarget::Nodes { selected, .. } => {
                // bbox всех выделенных (первичный + мультивыделение)
                let mut bbox: Option<[f32; 4]> = None;
                for index in selected {
                    let node = self.scene.canvas.nodes.get(*index)?;
                    let rect = [node.x, node.y, node.width, node.height];
                    bbox = Some(match bbox {
                        None => rect,
                        Some(b) => [
                            b[0].min(rect[0]),
                            b[1].min(rect[1]),
                            (b[0] + b[2]).max(rect[0] + rect[2]) - b[0].min(rect[0]),
                            (b[1] + b[3]).max(rect[1] + rect[3]) - b[1].min(rect[1]),
                        ],
                    });
                }
                let b = bbox?;
                let bottom_center = [b[0] + b[2] / 2.0, b[1] + b[3]];
                let screen = self.camera.world_to_screen(bottom_center, viewport);
                Some([screen[0], screen[1] + gap])
            }
            PaletteTarget::Edge(edge_index) => {
                let edge = self.scene.canvas.edges.get(*edge_index)?;
                let avoid = self.settings.edges_avoid_nodes;
                let mid = canvas_core::edge_midpoint(&self.scene.canvas, edge, avoid)?;
                let screen = self.camera.world_to_screen(mid, viewport);
                Some([screen[0], screen[1] + gap])
            }
        }
    }

    /// Чистая геометрия палитры: (layout, группы, цель). Без обновления
    /// hover-состояния — используется и для отрисовки, и для проверки
    /// «курсор над screen-space поверхностью» (колесо над UI холст
    /// не двигает).
    fn palette_geometry(
        &self,
    ) -> Option<(
        PaletteLayout,
        Vec<canvas_app::palette::PaletteGroup>,
        PaletteTarget,
    )> {
        let target = self.palette_target()?;
        let mut groups = palette_groups(&self.scene.canvas, &target);
        // FR-019: linked-связь с шаблоном — при несовпадении версии ноды
        // с реестром группа «Шаблон» с ручным update
        if let PaletteTarget::Nodes { primary, .. } = &target {
            if let Some(group) =
                template_update_group(&self.scene.canvas, *primary, &self.templates)
            {
                groups.push(group);
            }
        }
        if groups.is_empty() {
            return None;
        }
        let viewport = self.viewport_logical();
        let anchor = self.palette_anchor_screen(&target)?;
        let origin = palette_origin(anchor, palette_bar_size(&groups), viewport);
        let lay = palette_layout(origin, &groups, viewport);
        Some((lay, groups, target))
    }

    /// Вид палитры на кадр: (layout, группы, открытая hover'ом группа).
    /// Побочно обновляет `palette_hover` (hover-intent/отсрочка закрытия)
    /// и сбрасывает его при смене цели — вызывается и на кликах, и на кадрах.
    fn palette_view(
        &mut self,
    ) -> Option<(
        PaletteLayout,
        Vec<canvas_app::palette::PaletteGroup>,
        Option<usize>,
    )> {
        let (lay, groups, target) = self.palette_geometry()?;
        let open = {
            if self.palette_seen.as_ref() != Some(&target) {
                // Смена цели: раскрытая группа прежней цели недействительна
                self.palette_hover.reset();
                self.palette_seen = Some(target);
            }
            self.palette_hover.update(&lay, self.cursor)
        };
        Some((lay, groups, open))
    }

    /// Оверлей палитры выделения: бар с кнопками групп (иконка + подпись),
    /// открытая hover'ом колонка (строки с иконками и подписями).
    /// Screen-space: константный размер при любом зуме.
    fn palette_overlay(
        &self,
        lay: &PaletteLayout,
        groups: &[canvas_app::palette::PaletteGroup],
        open: Option<usize>,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let palette = ThemeColors::from_theme(self.settings.theme);
        let tint = color_to_rgba(palette.icon);
        let title = palette.title;
        let accent = [0.18, 0.29, 0.48, 0.95];
        // Фон бара
        instances.push(CardInstance {
            pos: [lay.bar[0], lay.bar[1]],
            size: [lay.bar[2], lay.bar[3]],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.9],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        for (i, group) in groups.iter().enumerate() {
            let button = lay.groups[i].button;
            let hovered = open == Some(i);
            // Кнопка группы: подсветка при наведении (выпадашка открыта)
            instances.push(CardInstance {
                pos: [button[0], button[1]],
                size: [button[2], button[3]],
                fill: if hovered {
                    [0.24, 0.30, 0.42, 1.0]
                } else {
                    [0.17, 0.18, 0.22, 1.0]
                },
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            // Иконка группы (текстовый глиф — ScreenText'ом по центру)
            let icon_rect = [
                button[0] + (button[2] - PAL_ICON) / 2.0,
                button[1] + (button[3] - PAL_ICON) / 2.0,
                PAL_ICON,
                PAL_ICON,
            ];
            instances.extend(icon_quads(group.icon, icon_rect, tint, &palette));
            if let Some(glyph) = icon_text(group.icon) {
                texts.push(OwnedScreenText {
                    text: glyph.to_owned(),
                    origin: [icon_rect[0], icon_rect[1] + 2.0],
                    width: icon_rect[2],
                    font_size: 12.0,
                    color: title,
                    align: TextAlign::Center,
                });
            }
            // Подпись группы под кнопкой
            texts.push(OwnedScreenText {
                text: group.label.clone(),
                origin: lay.groups[i].caption,
                width: button[2],
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // Открытая колонка (hover): фон + строки
            if hovered {
                let drop = lay.groups[i].dropdown;
                instances.push(CardInstance {
                    pos: [drop[0], drop[1]],
                    size: [drop[2], drop[3]],
                    fill: palette.menu_fill,
                    border: [0.22, 0.24, 0.30, 0.9],
                    params: [6.0, 0.0, 0.0, 1.0],
                });
                for (k, entry) in group.entries.iter().enumerate() {
                    let row = lay.groups[i].rows[k];
                    let row_hovered = point_in_rect(row, self.cursor);
                    let fill = if row_hovered {
                        accent
                    } else if entry.current {
                        [0.18, 0.29, 0.48, 0.45]
                    } else {
                        [0.0; 4]
                    };
                    if fill[3] > 0.0 {
                        instances.push(CardInstance {
                            pos: [row[0], row[1]],
                            size: [row[2], row[3]],
                            fill,
                            border: [0.0; 4],
                            params: [4.0, 0.0, 0.0, 1.0],
                        });
                    }
                    if let Some(icon) = entry.icon {
                        let icon_rect = [
                            row[0] + 5.0,
                            row[1] + (row[3] - PAL_ICON) / 2.0,
                            PAL_ICON,
                            PAL_ICON,
                        ];
                        instances.extend(icon_quads(icon, icon_rect, tint, &palette));
                        if let Some(glyph) = icon_text(icon) {
                            texts.push(OwnedScreenText {
                                text: glyph.to_owned(),
                                origin: [icon_rect[0], icon_rect[1] + 2.0],
                                width: icon_rect[2],
                                font_size: 13.0,
                                color: title,
                                align: TextAlign::Center,
                            });
                        }
                        texts.push(OwnedScreenText {
                            text: entry.label.clone(),
                            origin: [row[0] + 28.0, row[1] + 5.0],
                            width: row[2] - 32.0,
                            font_size: 13.0,
                            color: title,
                            align: TextAlign::Left,
                        });
                    } else {
                        // Строка без иконки — текст по всей ширине
                        texts.push(OwnedScreenText {
                            text: entry.label.clone(),
                            origin: [row[0] + 8.0, row[1] + 5.0],
                            width: row[2] - 12.0,
                            font_size: 13.0,
                            color: title,
                            align: TextAlign::Left,
                        });
                    }
                }
            }
        }
        (instances, texts)
    }

    /// Действие палитры: клик по строке выпадашки. Мутирующие действия —
    /// undo-шаг (FR-006); настройки ноды — через apply_node_setting.
    fn apply_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Node {
                node_index,
                setting,
            } => self.apply_node_setting(node_index, setting),
            PaletteAction::NodeColor { targets, preset } => {
                let snapshot = self.scene.canvas.clone();
                for index in targets {
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        node.color = preset.map(str::to_owned);
                    }
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::NodeGroup(index) => {
                if let Some(group) =
                    plan_group_around(&self.scene.canvas, index, canvas_app::ui::GROUP_PADDING)
                {
                    self.insert_group(group);
                }
            }
            PaletteAction::Layout { seed, mode } => self.apply_related_layout(seed, mode),
            PaletteAction::EdgeStyle { edge_index, style } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.style = Some(style);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeThickness {
                edge_index,
                thickness,
            } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.thickness = Some(thickness);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeColor { edge_index, preset } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.color = preset.map(str::to_owned);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            // FR-014: тогл типа потока (Value ↔ Control) — undo-шаг +
            // живой пересчёт (downstream может потерять/обрести входы).
            // Тогл в Value, замыкающий цикл, отвергается (isError в MCP;
            // в палитре — toast с участниками). Правка 3: логика тогла —
            // в SceneState::toggle_edge_flow (мутация живого канваса,
            // тестируемо); палитра только показывает toast при цикле.
            PaletteAction::EdgeFlowKind { edge_index, kind } => {
                match self.scene.toggle_edge_flow(edge_index, kind) {
                    Err(participants) => {
                        let participants = participants.join(" → ");
                        self.show_toast(format!("Цикл потока: {participants} — тогл отклонён"));
                        self.request_redraw();
                    }
                    Ok(true) => self.request_redraw(),
                    Ok(false) => {}
                }
            }
            // CR-008: закрепить/освободить конец связи. Закрепление —
            // WYSIWYG: в fromSide/toSide фиксируется текущая эффективная
            // сторона (что видели — то и закрепили). Undo-шаг (FR-006);
            // no-op (состояние не изменилось) шаг не копит.
            PaletteAction::EdgePortsPin {
                edge_index,
                end,
                pin,
            } => self.set_edge_port_pin(edge_index, end, pin),
            PaletteAction::EdgePortsAuto { edge_index } => {
                let Some(edge) = self.scene.canvas.edges.get(edge_index) else {
                    return;
                };
                if !edge.ports_pinned() {
                    return; // no-op — шаг не копится
                }
                let mut snapshot = self.scene.canvas.clone();
                if let Some(edge) = snapshot.edges.get_mut(edge_index) {
                    edge.clear_port_pins();
                }
                self.scene.push_undo(snapshot);
                self.scene.mark_dirty();
                self.show_toast("Порты связи: авто (кратчайший путь)");
                self.request_redraw();
            }
            // FR-019: ручной update шаблонной ноды (linked-связь):
            // expr/version/icon/color — из манифеста реестра, params — по
            // именам (совпавшие сохраняются, новые — дефолты). Один
            // undo-шаг, пересчёт потока.
            PaletteAction::TemplateUpdate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let Some(manifest) = self.templates.find(&template.id) else {
                    return;
                };
                if manifest.version == template.version {
                    return; // no-op — шаг не копится
                }
                let mut updated = template.clone();
                updated.version = manifest.version.clone();
                updated.expr = manifest.expr.clone();
                updated.icon = manifest.icon.clone();
                updated.color = manifest.color.clone();
                // FR-023: имя шаблона тоже синхронизируется с манифестом
                // (заголовок ноды — актуальное имя из реестра)
                updated.name = Some(manifest.display_name().to_owned());
                let mut params = BTreeMap::new();
                for spec in &manifest.params {
                    let value = template.params.get(&spec.name).cloned().unwrap_or(
                        canvas_core::templates::TemplateParam {
                            num: spec.default,
                            unit: spec.unit.clone(),
                        },
                    );
                    params.insert(spec.name.clone(), value);
                }
                updated.params = params;
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.set_template(Some(updated));
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                self.show_toast(format!(
                    "Шаблон обновлён: {} → v{}",
                    manifest.display_name(),
                    manifest.version
                ));
                self.request_redraw();
            }
            // FR-020: «Сохранить как шаблон» — снимок template-ссылки ноды
            // в custom-манифест (~/.canvasdesk/templates/<id>/template.json).
            // Файловая операция — НЕ undo-able; реестр перечитается.
            PaletteAction::SaveAsTemplate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let name = node.label.clone().unwrap_or_else(|| template.id.clone());
                let id = unique_custom_id(&slugify(&name), &self.templates, &self.templates_root);
                let params: Vec<canvas_core::templates::ParamSpec> = template
                    .params
                    .iter()
                    .map(|(name_param, value)| canvas_core::templates::ParamSpec {
                        name: name_param.clone(),
                        // Тип — подсказка UI: выводим по токену единицы
                        kind: infer_param_type(value.unit.as_deref()),
                        default: value.num,
                        unit: value.unit.clone(),
                        min: None,
                        max: None,
                    })
                    .collect();
                let manifest = canvas_core::templates::TemplateManifest {
                    id: id.clone(),
                    name: name.clone(),
                    name_ru: None,
                    version: "1.0.0".to_owned(),
                    category: "custom".to_owned(),
                    description: format!("Сохранено с канваса {}", self.scene.path.display()),
                    description_en: None,
                    params,
                    expr: template.expr.clone(),
                    color: "#9B9B9B".to_owned(),
                    icon: "custom".to_owned(),
                    source: canvas_core::templates::TemplateSource::Custom,
                };
                match canvas_core::templates::save_custom(&manifest, &self.templates_root) {
                    Ok(path) => {
                        // Перезагрузка реестра: custom появится в
                        // палитре/wheel и в MCP template_list
                        self.templates = canvas_core::templates::TemplateRegistry::all_with_custom(
                            &self.templates_root,
                        );
                        self.show_toast(format!("Шаблон «{name}» сохранён: {}", path.display()));
                    }
                    Err(err) => {
                        self.show_toast(format!("Не удалось сохранить шаблон: {err}"));
                    }
                }
                self.request_redraw();
            }
        }
    }

    /// CR-008: закрепить/освободить конец связи. Закрепление фиксирует
    /// текущую эффективную сторону конца в `fromSide`/`toSide` (файл
    /// отражает то, что видно на экране); освобождение оставляет
    /// сохранённую сторону в файле, но визуально возвращает авто.
    fn set_edge_port_pin(&mut self, edge_index: usize, end: canvas_core::EdgeEnd, pin: bool) {
        if !pin {
            let Some(edge) = self.scene.canvas.edges.get(edge_index) else {
                return;
            };
            let (pin_from, pin_to) = edge.port_pins();
            let currently = match end {
                canvas_core::EdgeEnd::From => pin_from,
                canvas_core::EdgeEnd::To => pin_to,
            };
            if !currently {
                return; // no-op — шаг не копится
            }
            let mut snapshot = self.scene.canvas.clone();
            if let Some(edge) = snapshot.edges.get_mut(edge_index) {
                edge.set_port_pin(end, false);
            }
            self.scene.push_undo(snapshot);
            self.scene.mark_dirty();
            self.show_toast("Порт освобождён: кратчайший путь");
            self.request_redraw();
            return;
        }
        // Закрепление: текущая эффективная сторона конца (та же геометрия,
        // по которой рисуется линия)
        let Some((side, _)) = canvas_core::edge_endpoint(&self.scene.canvas, edge_index, end)
        else {
            return;
        };
        let mut snapshot = self.scene.canvas.clone();
        let Some(edge) = snapshot.edges.get_mut(edge_index) else {
            return;
        };
        match end {
            canvas_core::EdgeEnd::From => edge.from_side = Some(side),
            canvas_core::EdgeEnd::To => edge.to_side = Some(side),
        }
        edge.set_port_pin(end, true);
        self.scene.push_undo(snapshot);
        self.scene.mark_dirty();
        self.show_toast("Порт связи закреплён");
        self.request_redraw();
    }

    /// Переключить тему (кнопка-иконка рядом с кнопкой настроек) и сохранить конфиг.
    fn toggle_theme(&mut self) {
        self.settings.theme = self.settings.theme.next();
        // M5: смена темы уходит виджетам (themeChanged — T21 доведёт
        // рассылку до инстансов, пока обновляется init-данные будущих нод)
        self.widgets.set_theme(self.settings.theme == Theme::Dark);
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_theme(ThemeColors::from_theme(self.settings.theme));
        }
        if let Some(path) = &self.config_path {
            if let Err(err) = self.settings.save(path) {
                tracing::warn!(%err, "не удалось сохранить конфиг");
            }
        }
    }

    /// T23 (brainstorm-focus): пересчёт состояния фокуса на кадр —
    /// фейд затемнения, пульс «дыхания» и окрестность семени
    /// (hover → выделенная нода → выделенная связь; O(V+E) — на 5k нод
    /// ~0.3–0.5 мс, кадры вне изменений не генерируются). Выделенная нода
    /// добавляется в яркий набор: выделение не гаснет (приоритет над фокусом).
    fn update_focus_state(&mut self) {
        // CR-001: мультивыделение без primary — семя из первой выделенной
        // (фокус живёт и после сброса одиночного клика)
        let selected = self.scene.selected.or_else(|| {
            self.scene
                .selected_nodes
                .first()
                .copied()
                .map(Selection::Node)
        });
        let seed = focus_seed_of(self.hovered, selected);
        let focus_on = self.settings.focus_mode;
        // Цель затемнения: 1 — режим включён и семя есть; иначе всё гаснет
        let target = f32::from(focus_on && seed.is_some());
        // Фейд к новой цели (перезапуск при смене цели, продолжение — к той же)
        let needs_new_fade = match self.focus_fade {
            Some((_, to, _)) => (to - target).abs() > 1e-3,
            None => (self.focus_dim - target).abs() > 1e-3,
        };
        if needs_new_fade {
            self.focus_fade = Some((self.focus_dim, target, Instant::now()));
        }
        if let Some((from, to, start)) = self.focus_fade {
            let elapsed = start.elapsed().as_millis() as u32;
            if elapsed >= FOCUS_FADE_MS {
                self.focus_dim = to;
                self.focus_fade = None;
            } else {
                self.focus_dim = from + (to - from) * focus_fade(elapsed);
            }
        }
        // «Дыхание»: рестарт при смене семени (режим включён), один цикл,
        // затем поле очищается — кадры для статики не нужны
        if focus_on {
            if let Some(seed) = seed {
                // is_some_and (не is_none_or): MSRV проекта 1.80
                if !self.focus_pulse.as_ref().is_some_and(|(s, _)| *s == seed) {
                    self.focus_pulse = Some((seed, Instant::now()));
                }
                if let Some((_, start)) = self.focus_pulse {
                    if start.elapsed().as_millis() as u32 >= FOCUS_PULSE_MS {
                        self.focus_pulse = None;
                    }
                }
            } else {
                self.focus_pulse = None;
            }
        } else {
            self.focus_pulse = None;
        }
        // Окрестность: пересчёт только при включённом режиме (иначе пусто)
        if focus_on {
            if let Some(seed) = seed {
                let mut set = focus_set(&self.scene.canvas, seed);
                // Выделенная нода (кроме семени-связи — у неё свои концы)
                // не гаснет вместе с остальными (план T23 §7); CR-001 —
                // весь набор мультивыделения тоже остаётся ярким
                let seed_is_edge = matches!(seed, FocusSeed::Edge(_));
                if let (Some(Selection::Node(index)), false) = (self.scene.selected, seed_is_edge) {
                    if !set.contains_node(index) {
                        set.nodes.push(index);
                    }
                }
                if !seed_is_edge {
                    for index in &self.scene.selected_nodes {
                        if !set.contains_node(*index) {
                            set.nodes.push(*index);
                        }
                    }
                }
                set.nodes.sort_unstable();
                set.nodes.dedup();
                self.focus_nodes = set.nodes;
                self.focus_edges = set.edges;
            } else {
                self.focus_nodes.clear();
                self.focus_edges.clear();
            }
        } else if !self.focus_nodes.is_empty() || !self.focus_edges.is_empty() {
            self.focus_nodes.clear();
            self.focus_edges.clear();
        }
    }

    /// T23: анимации фокуса ещё идут (кадры держит about_to_wait)?
    fn focus_animating(&self) -> bool {
        self.focus_fade.is_some()
            || self
                .focus_pulse
                .as_ref()
                .is_some_and(|(_, start)| start.elapsed().as_millis() < u128::from(FOCUS_PULSE_MS))
    }

    /// T23: переключить режим фокуса связей (хоткей F / ПКМ-меню / панель
    /// настроек — панель сохраняет конфиг общим хвостом apply_settings_row,
    /// хоткей и меню — рантайм-переключение без записи).
    fn toggle_focus_mode(&mut self) {
        self.settings.focus_mode = !self.settings.focus_mode;
        tracing::info!(
            вкл = self.settings.focus_mode,
            "режим фокуса связей (brainstorm-focus)"
        );
        self.request_redraw();
    }

    /// FR-026: применить переключение булевой строки панели настроек
    /// (тумблер) и сохранить конфиг. Многозначные строки — через
    /// выпадающее меню ([`App::apply_dropdown_choice`]).
    fn apply_toggle_row(&mut self, row: SettingsRow) {
        match row {
            SettingsRow::Grid => {
                self.settings.grid_visible = !self.settings.grid_visible;
            }
            SettingsRow::EdgesAvoid => {
                self.settings.edges_avoid_nodes = !self.settings.edges_avoid_nodes;
            }
            // T23: состояние синхронно с settings — сохранение общим хвостом
            SettingsRow::FocusMode => self.toggle_focus_mode(),
            SettingsRow::HudOnStart => {
                self.settings.hud_on_start = !self.settings.hud_on_start;
                // Мгновенная обратная связь: HUD переключается сразу
                self.hud_visible = self.settings.hud_on_start;
            }
            SettingsRow::ButtonCorner
            | SettingsRow::GridStyle
            | SettingsRow::GridDensity
            | SettingsRow::PortZone => {
                debug_assert!(false, "dropdown-строка не тумблер: {row:?}");
                return;
            }
        }
        self.sync_settings_row(row);
        self.save_settings();
    }

    /// FR-026: применить выбор значения из выпадающего меню — та же чистая
    /// функция значений, что в тестах инвариантов; побочные эффекты рендера
    /// те же, что были в ветках apply_settings_row. Конфиг сохраняется
    /// общим хвостом. Смена угла кнопки перепривязывает панель — открытое
    /// меню закрывает вызывающий (состояние привязано к строке, не к точке).
    fn apply_dropdown_choice(&mut self, row: SettingsRow, index: usize) {
        apply_dropdown_value(&mut self.settings, row, index);
        self.sync_settings_row(row);
        self.save_settings();
    }

    /// FR-026: синхронизация рендера с настройками после изменения строки
    /// (то, что раньше делали ветки apply_settings_row по месту).
    fn sync_settings_row(&mut self, row: SettingsRow) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        match row {
            SettingsRow::Grid => renderer.set_grid_visible(self.settings.grid_visible),
            SettingsRow::GridStyle => {
                renderer.set_grid_dots(self.settings.grid_style == GridStyle::Dots)
            }
            SettingsRow::GridDensity => {
                let (minor, major) = self.settings.grid_density.steps();
                renderer.set_grid_steps(minor, major);
            }
            _ => {}
        }
    }

    /// FR-026: общий хвост применения настроек — сохранение config.toml.
    fn save_settings(&self) {
        if let Some(path) = &self.config_path {
            if let Err(err) = self.settings.save(path) {
                tracing::warn!(%err, "не удалось сохранить конфиг");
            }
        }
    }

    /// Screen-space оверлей настроек: летающая кнопка всегда, панель — когда
    /// открыта. Координаты — логические px от левого верхнего угла окна.
    fn settings_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = ThemeColors::from_theme(self.settings.theme);
        // Вертикальная центровка иконки: лайн-бокс высотой font*1.3 по центру
        // кнопки (так же считает рендер screen-текстов).
        let icon_top = |rect: [f32; 4], font_size: f32| rect[1] + (rect[3] - font_size * 1.3) / 2.0;
        let button = button_rect(self.settings.button_corner, viewport);
        // Hover-аффорданс: курсор над кнопкой — заливка ярче
        let settings_hovered = point_in_rect(button, self.cursor);
        instances.push(CardInstance {
            pos: [button[0], button[1]],
            size: [button[2], button[3]],
            fill: if settings_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            // params.y = рамка выделения: подсветка кнопки при открытой панели
            params: [8.0, self.settings_open as u8 as f32, 0.0, 0.0],
        });
        // Иконка-шестерёнка: горизонтально по центру кнопки (Align::Center
        // в области width = ширине кнопки) — не зависит от метрик глифа.
        texts.push(OwnedScreenText {
            text: "⚙".to_owned(),
            origin: [button[0], icon_top(button, 18.0)],
            width: button[2],
            font_size: 18.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Кнопка переключения темы — рядом с кнопкой настроек (в тот же угол).
        // Иконка показывает ЦЕЛЬ: в тёмной теме «солнце» (клик — светлая).
        let theme_button = theme_button_rect(self.settings.button_corner, viewport);
        let theme_hovered = point_in_rect(theme_button, self.cursor);
        instances.push(CardInstance {
            pos: [theme_button[0], theme_button[1]],
            size: [theme_button[2], theme_button[3]],
            fill: if theme_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        });
        let theme_icon = match self.settings.theme {
            Theme::Dark => "☀",
            Theme::Light => "🌙",
        };
        texts.push(OwnedScreenText {
            text: theme_icon.to_owned(),
            origin: [theme_button[0], icon_top(theme_button, 16.0)],
            width: theme_button[2],
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Панель горячих клавиш (FR-004): у левого края, по центру;
        // рендерится независимо от панели настроек
        if self.hotkeys_open {
            let panel = hotkeys_panel_rect(viewport);
            instances.push(CardInstance {
                pos: [panel[0], panel[1]],
                size: [panel[2], panel[3]],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 0.0],
            });
            let pad = canvas_app::ui::HOTKEYS_PADDING;
            let header_h = canvas_app::ui::HOTKEYS_HEADER_HEIGHT;
            let row_h = canvas_app::ui::HOTKEYS_ROW_HEIGHT;
            let key_w = canvas_app::ui::HOTKEYS_KEY_COLUMN;
            let key_x = panel[0] + pad;
            let desc_x = panel[0] + pad + key_w;
            let desc_w = (panel[2] - pad * 2.0 - key_w).max(10.0);
            texts.push(OwnedScreenText {
                text: "Горячие клавиши".to_owned(),
                origin: [key_x, panel[1] + pad + 7.0],
                width: panel[2] - pad * 2.0,
                font_size: 15.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            for (i, (key, description)) in canvas_app::ui::HOTKEYS.iter().enumerate() {
                let y = panel[1] + pad + header_h + i as f32 * row_h + 3.0;
                // Строки ниже кромки панели (кламп высоты) не рисуем
                if y + row_h > panel[1] + panel[3] - 2.0 {
                    break;
                }
                texts.push(OwnedScreenText {
                    text: (*key).to_owned(),
                    origin: [key_x, y],
                    width: key_w,
                    font_size: 12.0,
                    color: palette.link,
                    align: TextAlign::Left,
                });
                texts.push(OwnedScreenText {
                    text: (*description).to_owned(),
                    origin: [desc_x, y],
                    width: desc_w,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        if !self.settings_open {
            return (instances, texts);
        }
        // FR-026: панель по группам — layout несёт rect'ы заголовков и строк
        let layout = panel_layout(self.settings.button_corner, viewport);
        let panel = layout.rect;
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        });
        let text_x = panel[0] + PANEL_PADDING + 4.0;
        let text_w = panel[2] - PANEL_PADDING * 2.0 - 8.0;
        texts.push(OwnedScreenText {
            text: "Настройки".to_owned(),
            origin: [text_x, panel[1] + PANEL_PADDING + 5.0],
            width: text_w,
            font_size: 15.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // FR-026: открытое выпадающее меню — геометрия и пункты (состояние
        // не хранит список — вычисляется из настроек, устареть не может)
        let menu = self.settings_dropdown.open_row.map(|row| {
            let items = dropdown_options(row, &self.settings);
            let anchor = layout.row_rect(row).unwrap_or([0.0; 4]);
            let rect = dropdown_layout(anchor, viewport, items.len());
            (row, items, rect)
        });
        // Hover-подсветка кликабельной строки под курсором (аффорданс);
        // подсветка строки под меню уходит под фон меню — безвредно
        if let Some(row) = row_at(&layout, self.cursor) {
            if let Some(rect) = layout.row_rect(row) {
                instances.push(CardInstance {
                    pos: [panel[0] + PANEL_PADDING, rect[1] + 1.0],
                    size: [panel[2] - PANEL_PADDING * 2.0, rect[3] - 2.0],
                    fill: [0.24, 0.30, 0.42, 0.6],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
        }
        for (entry, rect) in &layout.entries {
            match entry {
                PanelEntry::Header(title) => {
                    // Заголовок секции: капс меньшим кеглем, цвет иконок
                    texts.push(OwnedScreenText {
                        text: title.to_uppercase(),
                        origin: [text_x, rect[1] + 4.0],
                        width: text_w,
                        font_size: 11.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                }
                PanelEntry::Row(row) => {
                    // Квады рисуются ДО всех screen-текстов (renderer.rs):
                    // строки, перекрытые меню, не рисуем — иначе их текст
                    // проступит сквозь фон меню
                    if menu
                        .as_ref()
                        .is_some_and(|(_, _, menu_rect)| rects_intersect(*menu_rect, *rect))
                    {
                        continue;
                    }
                    texts.push(OwnedScreenText {
                        text: row.label(&self.settings),
                        origin: [text_x, rect[1] + 5.0],
                        width: text_w,
                        font_size: 13.0,
                        color: palette.body,
                        align: TextAlign::Left,
                    });
                    // Аффорданс dropdown: ▾ у правого края строки (тумблерам
                    // не нужен — их цикл из двух значений виден целиком)
                    if row_kind(*row) == RowKind::Dropdown {
                        texts.push(OwnedScreenText {
                            text: "▾".to_owned(),
                            origin: [panel[0] + panel[2] - PANEL_PADDING - 12.0, rect[1] + 5.0],
                            width: 12.0,
                            font_size: 12.0,
                            color: palette.icon,
                            align: TextAlign::Left,
                        });
                    }
                }
            }
        }
        texts.push(OwnedScreenText {
            text: "Ctrl+, — открыть/закрыть".to_owned(),
            origin: [
                text_x,
                panel[1] + panel[3] - PANEL_PADDING - PANEL_HINT_HEIGHT + 4.0,
            ],
            width: text_w,
            font_size: 11.0,
            color: palette.icon,
            align: TextAlign::Left,
        });
        // FR-026: выпадающее меню — поверх панели: фон чуть ярче панели,
        // hover/клавиатурное выделение пункта, галочка у текущего значения
        if let Some((row, items, menu_rect)) = &menu {
            instances.push(CardInstance {
                pos: [menu_rect[0], menu_rect[1]],
                size: [menu_rect[2], menu_rect[3]],
                fill: hover_fill(palette.menu_fill),
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 0.0],
            });
            let hovered_item = dropdown_item_at(*menu_rect, items.len(), self.cursor);
            for (i, (label, current)) in items.iter().enumerate() {
                let y = menu_rect[1] + DROPDOWN_MARGIN + i as f32 * DROPDOWN_ROW_H;
                let highlighted = hovered_item == Some(i)
                    || (self.settings_dropdown.open_row == Some(*row)
                        && self.settings_dropdown.selected == i);
                if highlighted {
                    instances.push(CardInstance {
                        pos: [menu_rect[0] + 3.0, y + 1.0],
                        size: [menu_rect[2] - 6.0, DROPDOWN_ROW_H - 2.0],
                        fill: [0.24, 0.30, 0.42, 0.6],
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                if *current {
                    texts.push(OwnedScreenText {
                        text: "✓".to_owned(),
                        origin: [menu_rect[0] + 8.0, y + 5.0],
                        width: 18.0,
                        font_size: 12.0,
                        color: palette.link,
                        align: TextAlign::Left,
                    });
                }
                texts.push(OwnedScreenText {
                    text: label.clone(),
                    origin: [menu_rect[0] + MENU_LABEL_X, y + 4.0],
                    width: menu_rect[2] - MENU_LABEL_X - DROPDOWN_MARGIN,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
    }

    /// Заказать тамбнейлы видимых файловых нод (T6): приоритет High,
    /// дедупликация — в ThumbService, по наличию в атласе и по негативному кэшу.
    fn order_thumbnails(&self) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let visible = self.camera.visible_world_rect(viewport);
        for index in self.scene.spatial.query_rect(visible) {
            let node = &self.scene.canvas.nodes[index];
            let Some(file) = node.file.as_deref() else {
                continue;
            };
            if renderer.has_thumbnail(index) || self.thumbs_failed.contains(&index) {
                continue;
            }
            let path = self.scene.resolve_file_path(file);
            self.thumbs.request(Priority::High, index, path);
        }
    }
}

impl App {
    /// Забрать накопившиеся MCP-запросы из pipe-сервера и ответить на каждый
    /// (MCP-интеграция): tools/call → `mcp_dispatch`, ошибки валидации и
    /// «не найдено» — в MCP-идиоме isError (не JSON-RPC error); битый
    /// конверт — JSON-RPC error с null-id. Красим окно при любом обращении.
    #[cfg(windows)]
    fn on_mcp_wake(&mut self) {
        let Some(server) = self.mcp_server.as_ref() else {
            return;
        };
        let mut handled = false;
        while let Some((line, respond)) = server.take_request() {
            handled = true;
            let reply = match canvas_mcp::parse_envelope(&line) {
                // Notification (id == None) — отвечать нечему
                Ok(request) if request.id.is_none() => continue,
                Ok(request) => {
                    let id = request.id.unwrap_or(serde_json::Value::Null);
                    let (method, params) = mcp_unwrap_call(&request.method, &request.params);
                    match mcp_dispatch(
                        &mut self.scene,
                        &mut self.camera,
                        &self.templates,
                        &method,
                        &params,
                    ) {
                        Ok(value) => canvas_mcp::build_result(&id, &value),
                        Err(message) => canvas_mcp::build_call_error(&id, &message),
                    }
                }
                Err(err) => canvas_mcp::build_error(err.id.as_ref(), err.code, &err.message),
            };
            respond(reply);
        }
        if handled {
            self.request_redraw();
        }
    }
}

/// Распаковка MCP-конверта, пришедшего по pipe: `tools/call` несёт имя
/// инструмента и аргументы внутри params (`name`/`arguments`) — посредник
/// форвардит конверт как есть; прочие методы проходят без изменений.
/// Чистая функция — тестируется без pipe.
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_unwrap_call(method: &str, params: &serde_json::Value) -> (String, serde_json::Value) {
    if method == "tools/call" {
        let name = params
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_default();
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        (name, args)
    } else {
        (method.to_owned(), params.clone())
    }
}

/// Обязательный строковый параметр MCP-инструмента.
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_req_str<'v>(params: &'v serde_json::Value, name: &str) -> Result<&'v str, String> {
    params
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("отсутствует параметр '{name}'"))
}

/// Обязательный числовой параметр MCP-инструмента.
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_req_f32(params: &serde_json::Value, name: &str) -> Result<f32, String> {
    params
        .get(name)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32)
        .ok_or_else(|| format!("отсутствует числовой параметр '{name}'"))
}

/// Опциональный числовой параметр MCP-инструмента.
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_opt_f32(params: &serde_json::Value, name: &str) -> Option<f32> {
    params
        .get(name)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32)
}

/// Индекс ноды по строковому id (MCP-инструменты адресуют ноды id).
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_node_index(canvas: &Canvas, id: &str) -> Result<usize, String> {
    canvas
        .nodes
        .iter()
        .position(|node| node.id == id)
        .ok_or_else(|| format!("нода не найдена: {id}"))
}

/// Сводка ноды для списков; поле text — только по запросу (can be большим).
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_node_summary(node: &Node, with_text: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": node.id,
        "type": node.node_type,
        "x": node.x,
        "y": node.y,
        "width": node.width,
        "height": node.height,
        "label": node.label,
        "file": node.file,
        "color": node.color,
        // FR-013: Numi-формула (null — calc-режим выключен)
        "expr": node.expr(),
    });
    if with_text {
        value["text"] = serde_json::Value::from(node.text.clone());
    }
    value
}

/// Сторона связи MCP: "any"/отсутствие → None (автовывод из геометрии).
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_side(params: &serde_json::Value, name: &str) -> Result<Option<Side>, String> {
    match params.get(name).and_then(serde_json::Value::as_str) {
        None | Some("any") => Ok(None),
        Some(text) => serde_json::from_value(serde_json::Value::String(text.to_owned()))
            .map(Some)
            .map_err(|_| format!("неверная сторона '{name}': {text}")),
    }
}

/// Выполнить MCP-инструмент над сценой/камерой: 15 инструментов канваса
/// (tools/list — в canvas-mcp). Чистая функция над SceneState + Camera —
/// тестируется без окна и pipe; каждая мутирующая ветка обновляет spatial
/// index и помечает канвас грязным (автосейв). Ошибки — строки, посредник
/// заворачивает их в isError.
#[cfg_attr(not(windows), allow(dead_code))]
fn mcp_dispatch(
    scene: &mut SceneState,
    camera: &mut Camera,
    templates: &canvas_core::templates::TemplateRegistry,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method {
        "canvas_info" => Ok(serde_json::json!({
            "path": scene.path.to_string_lossy(),
            "nodes": scene.canvas.nodes.len(),
            "edges": scene.canvas.edges.len(),
        })),
        "nodes_list" => {
            let with_text = params
                .get("text")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let nodes = scene
                .canvas
                .nodes
                .iter()
                .map(|node| mcp_node_summary(node, with_text))
                .collect();
            Ok(serde_json::Value::Array(nodes))
        }
        "node_get" => {
            let id = mcp_req_str(params, "id")?;
            let node = scene
                .canvas
                .node(id)
                .ok_or_else(|| format!("нода не найдена: {id}"))?;
            serde_json::to_value(node).map_err(|err| err.to_string())
        }
        "nodes_search" => {
            let query = mcp_req_str(params, "query")?.to_lowercase();
            let nodes = scene
                .canvas
                .nodes
                .iter()
                .filter(|node| {
                    [
                        node.text.as_deref(),
                        node.label.as_deref(),
                        node.file.as_deref(),
                    ]
                    .into_iter()
                    .any(|field| field.is_some_and(|text| text.to_lowercase().contains(&query)))
                })
                .map(|node| mcp_node_summary(node, true))
                .collect();
            Ok(serde_json::Value::Array(nodes))
        }
        "node_create_note" => {
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let text = params
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let mut node = Node::text(next_free_id(&scene.canvas, "note"), text, x, y);
            if let Some(width) = mcp_opt_f32(params, "width") {
                node.width = width;
            }
            if let Some(height) = mcp_opt_f32(params, "height") {
                node.height = height;
            }
            // FR-013: строки «= …» в тексте — формула
            node.set_expr(split_formula_lines(text));
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг (после валидации, до вставки)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            // FR-014: живой пересчёт потока после создания expr-ноды
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": scene.canvas.nodes[index].id }))
        }
        "node_create_file" => {
            let path = mcp_req_str(params, "path")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            // Файл на диске НЕ создаём — только карточка в модели
            let node = Node::file(
                next_free_id(&scene.canvas, "file"),
                path,
                x,
                y,
                mcp_opt_f32(params, "width").unwrap_or(canvas_app::ui::DROP_CARD_W),
                mcp_opt_f32(params, "height").unwrap_or(canvas_app::ui::DROP_CARD_H),
            );
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": scene.canvas.nodes[index].id }))
        }
        "node_update_text" => {
            let id = mcp_req_str(params, "id")?;
            let text = mcp_req_str(params, "text")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes[index].text = Some(text.to_owned());
            // FR-013: строки «= …» в тексте — формула (единая семантика
            // с редактором); формула нет — сброс
            scene.canvas.nodes[index].set_expr(split_formula_lines(text));
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        // FR-005: редактирование ноды одним вызовом — обновляются ТОЛЬКО
        // переданные поля; label/color/expr = null — сброс; геометрия — с
        // обновлением spatial index; ответ — сводка с текстом
        "node_edit" => {
            let id = mcp_req_str(params, "id")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-013: expr валидируется ПЕРЕД undo-шагом — при ошибке
            // парсинга нода не меняется вовсе (isError с диагностикой)
            let expr_update = match params.get("expr") {
                None => None,
                Some(serde_json::Value::Null) => Some(None),
                Some(serde_json::Value::String(formula)) => match expr::parse(formula) {
                    Ok(_) => Some(Some(formula.clone())),
                    Err(err) => return Err(format!("expr: {err}")),
                },
                Some(other) => return Err(format!("expr должен быть строкой или null: {other}")),
            };
            let mut geometry = false;
            // FR-006: MCP-мутация — undo-шаг. Пушим до мутаций: валидация
            // отдельных полей переплетена с применением остальных (частичные
            // применения при Err тоже должны быть отменяемы)
            scene.push_undo(scene.canvas.clone());
            if let Some(text) = params.get("text").and_then(serde_json::Value::as_str) {
                scene.canvas.nodes[index].text = Some(text.to_owned());
            }
            match params.get("label") {
                None => {}
                Some(serde_json::Value::Null) => scene.canvas.nodes[index].label = None,
                Some(serde_json::Value::String(label)) => {
                    scene.canvas.nodes[index].label = Some(label.clone());
                }
                Some(other) => return Err(format!("label должен быть строкой или null: {other}")),
            }
            if params.get("color").is_some() {
                let color = match params
                    .get("color")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
                {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(preset)
                        if matches!(preset.as_str(), "1" | "2" | "3" | "4" | "5" | "6") =>
                    {
                        Some(preset)
                    }
                    other => {
                        return Err(format!(
                            "color должен быть пресетом \"1\"..\"6\" или null, получено {other}"
                        ));
                    }
                };
                scene.canvas.nodes[index].color = color;
            }
            if let Some(x) = mcp_opt_f32(params, "x") {
                scene.canvas.nodes[index].x = x;
                geometry = true;
            }
            if let Some(y) = mcp_opt_f32(params, "y") {
                scene.canvas.nodes[index].y = y;
                geometry = true;
            }
            for (name, field) in [("width", 0), ("height", 1)] {
                if let Some(value) = mcp_opt_f32(params, name) {
                    if value <= 0.0 {
                        return Err(format!("{name} должен быть > 0, получено {value}"));
                    }
                    if field == 0 {
                        scene.canvas.nodes[index].width = value;
                    } else {
                        scene.canvas.nodes[index].height = value;
                    }
                    geometry = true;
                }
            }
            if geometry {
                let node = &scene.canvas.nodes[index];
                scene.spatial.update(index, node);
            }
            // FR-013: формула уже провалидирована — применяем и пересчитываем;
            // FR-014: живой пересчёт downstream
            if let Some(new_expr) = expr_update {
                scene.canvas.nodes[index].set_expr(new_expr);
                scene.mark_dirty();
                scene.recompute_flow();
            }
            scene.mark_dirty();
            let node = &scene.canvas.nodes[index];
            Ok(mcp_node_summary(node, true))
        }
        "node_move" => {
            let id = mcp_req_str(params, "id")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.move_node(index, x, y);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "node_resize" => {
            let id = mcp_req_str(params, "id")?;
            let width = mcp_req_f32(params, "width")?;
            let height = mcp_req_f32(params, "height")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            let node = &mut scene.canvas.nodes[index];
            node.width = width;
            node.height = height;
            scene.spatial.update(index, node);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "node_delete" => {
            let id = mcp_req_str(params, "id")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            let removed = scene
                .canvas
                .remove_node(index)
                .ok_or_else(|| format!("нода не найдена: {id}"))?;
            // Индексы сдвинулись — spatial перестраивается (паттерн delete_selected)
            scene.spatial = SpatialIndex::build(&scene.canvas);
            scene.selected = None;
            scene.selected_nodes.clear();
            scene.dragging = None;
            scene.mark_dirty();
            // FR-014: downstream удалённой ноды — «вход отсутствует»
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": removed.id }))
        }
        "node_set_color" => {
            let id = mcp_req_str(params, "id")?;
            let color = match params
                .get("color")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
            {
                serde_json::Value::Null => None,
                serde_json::Value::String(preset)
                    if matches!(preset.as_str(), "1" | "2" | "3" | "4" | "5" | "6") =>
                {
                    Some(preset)
                }
                other => {
                    return Err(format!(
                        "color должен быть пресетом \"1\"..\"6\" или null, получено {other}"
                    ));
                }
            };
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг (валидация цвета прошла выше)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes[index].color = color;
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "edge_create" => {
            let from = mcp_req_str(params, "from")?.to_owned();
            let to = mcp_req_str(params, "to")?.to_owned();
            mcp_node_index(&scene.canvas, &from)?;
            mcp_node_index(&scene.canvas, &to)?;
            let edge = Edge::new(
                scene.canvas.next_edge_id(),
                &from,
                mcp_side(params, "fromSide")?,
                &to,
                mcp_side(params, "toSide")?,
            );
            // FR-006: MCP-мутация — undo-шаг (все валидации прошли)
            scene.push_undo(scene.canvas.clone());
            let id = edge.id.clone();
            scene.canvas.add_edge(edge);
            scene.mark_dirty();
            // FR-014: топология изменилась — живой пересчёт потока
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        "edge_delete" => {
            let id = mcp_req_str(params, "id")?;
            // FR-006: MCP-мутация — undo-шаг (проверка существования связи
            // идёт в remove — неудача шага не оставит: снапшот не изменится,
            // а лишний пуш свернётся сравнением ниже)
            let snapshot = scene.canvas.clone();
            if !scene.canvas.remove_edge(id) {
                return Err(format!("связь не найдена: {id}"));
            }
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            // FR-014: топология изменилась — живой пересчёт потока
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        // FR-014: тогл типа потока. Тогл в Value, замыкающий цикл value-рёбер,
        // — isError с участниками (пользователю UI показывает диалог, агенту
        // MCP — явную ошибку)
        "flow_set_kind" => {
            let id = mcp_req_str(params, "id")?;
            let kind = match params.get("kind").and_then(serde_json::Value::as_str) {
                Some("value") => FlowKind::Value,
                Some("control") => FlowKind::Control,
                other => {
                    return Err(format!(
                        "kind должен быть \"value\" или \"control\", получено {other:?}"
                    ));
                }
            };
            let edge_index = scene
                .canvas
                .edges
                .iter()
                .position(|edge| edge.id == id)
                .ok_or_else(|| format!("связь не найдена: {id}"))?;
            let (from, to) = {
                let edge = &scene.canvas.edges[edge_index];
                (edge.from_node.clone(), edge.to_node.clone())
            };
            if kind == FlowKind::Value
                && scene.canvas.edges[edge_index].flow_kind() != FlowKind::Value
                && canvas_core::creates_value_cycle(&scene.canvas, &from, &to)
            {
                let participants = canvas_core::value_path(&scene.canvas, &to, &from)
                    .unwrap_or_default()
                    .join(" → ");
                return Err(format!("цикл потока значений: {participants}"));
            }
            let snapshot = scene.canvas.clone();
            scene.canvas.edges[edge_index].set_flow_kind(kind);
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({
                "id": id,
                "kind": scene.canvas.edges[edge_index].flow_kind().as_str(),
            }))
        }
        // FR-014: форс-пересчёт всего графа — карта значений для агентов,
        // проверяющих сценарии (ноды без формулы не участвуют)
        "flow_recalc" => {
            let outputs = flow::propagate(&scene.canvas, &HashMap::new())
                .map_err(|cycle| cycle.to_string())?;
            let nodes: serde_json::Map<String, serde_json::Value> = outputs
                .iter()
                .map(|(id, result)| {
                    let entry = match result {
                        Ok(value) => serde_json::json!({
                            "value": value.num,
                            "unit": value.unit.display(),
                        }),
                        Err(err) => serde_json::json!({ "error": err.to_string() }),
                    };
                    (id.clone(), entry)
                })
                .collect();
            Ok(serde_json::Value::Object(nodes))
        }
        // FR-014: проверка DAG-инварианта — [] или участники цикла
        "flow_cycle_check" => match flow::topo_sort(&scene.canvas) {
            Ok(_) => Ok(serde_json::json!([])),
            Err(cycle) => Ok(serde_json::json!(cycle.nodes)),
        },
        // CR-008: стороны подключения связи. "auto" — снять закрепления
        // (кратчайший путь); "from"/"to"/"both" — закрепить концы, фиксируя
        // текущие эффективные стороны (WYSIWYG, как в палитре)
        "edge_ports" => {
            let id = mcp_req_str(params, "id")?;
            let pin = params.get("pin").and_then(serde_json::Value::as_str);
            let edge_index = scene
                .canvas
                .edges
                .iter()
                .position(|edge| edge.id == id)
                .ok_or_else(|| format!("связь не найдена: {id}"))?;
            let snapshot = scene.canvas.clone();
            match pin {
                Some("auto") => {
                    scene.canvas.edges[edge_index].clear_port_pins();
                }
                Some("from" | "to" | "both") => {
                    let pin_from = pin != Some("to");
                    let pin_to = pin != Some("from");
                    for (end, do_pin) in [
                        (canvas_core::EdgeEnd::From, pin_from),
                        (canvas_core::EdgeEnd::To, pin_to),
                    ] {
                        if !do_pin {
                            continue;
                        }
                        // Текущая эффективная сторона конца (геометрия как на экране)
                        let Some((side, _)) =
                            canvas_core::edge_endpoint(&scene.canvas, edge_index, end)
                        else {
                            return Err("висячая связь (нода не найдена)".to_owned());
                        };
                        let edge = &mut scene.canvas.edges[edge_index];
                        match end {
                            canvas_core::EdgeEnd::From => edge.from_side = Some(side),
                            canvas_core::EdgeEnd::To => edge.to_side = Some(side),
                        }
                        edge.set_port_pin(end, true);
                    }
                }
                other => {
                    return Err(format!(
                        "pin должен быть \"auto\", \"from\", \"to\" или \"both\", получено {other:?}"
                    ));
                }
            }
            let edge = &scene.canvas.edges[edge_index];
            let (pin_from, pin_to) = edge.port_pins();
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            Ok(serde_json::json!({
                "id": id,
                "pins": {
                    "from": pin_from,
                    "to": pin_to,
                },
            }))
        }
        // FR-018: список шаблонов реестра — те же, что в палитре/wheel
        // (инвариант 4: MCP-видимость эквивалентна UI)
        "template_list" => {
            let templates: Vec<serde_json::Value> = templates
                .list()
                .iter()
                .map(|manifest| {
                    let params: serde_json::Map<String, serde_json::Value> = manifest
                        .params
                        .iter()
                        .map(|spec| {
                            let mut entry = serde_json::json!({
                                "type": spec.kind.as_str(),
                                "default": spec.default,
                            });
                            if let Some(unit) = &spec.unit {
                                entry["unit"] = serde_json::json!(unit);
                            }
                            if let Some(min) = spec.min {
                                entry["min"] = serde_json::json!(min);
                            }
                            if let Some(max) = spec.max {
                                entry["max"] = serde_json::json!(max);
                            }
                            (spec.name.clone(), entry)
                        })
                        .collect();
                    serde_json::json!({
                        "id": manifest.id,
                        "name_en": manifest.name,
                        "name_ru": manifest.name_ru,
                        "version": manifest.version,
                        "category": manifest.category,
                        "description": manifest.description,
                        "description_en": manifest.description_en,
                        "expr": manifest.expr,
                        "icon": manifest.icon,
                        "color": manifest.color,
                        "source": manifest.source.as_str(),
                        "params": params,
                    })
                })
                .collect();
            Ok(serde_json::Value::Array(templates))
        }
        // FR-018: инстанциация шаблона (идентично wheel/палитре): text-нода
        // с Numi-листом параметров и снимком canvasdesk.template; undo-шаг
        "template_instantiate" => {
            let id = mcp_req_str(params, "id")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let manifest = templates
                .find(id)
                .ok_or_else(|| format!("шаблон не найден: {id}"))?
                .clone();
            let mut overrides: BTreeMap<String, canvas_core::templates::TemplateParam> =
                BTreeMap::new();
            if let Some(map) = params.get("params").and_then(serde_json::Value::as_object) {
                for (name, value) in map {
                    let param = match value {
                        serde_json::Value::Number(num) => {
                            let num = num
                                .as_f64()
                                .ok_or_else(|| format!("параметр {name}: число"))?;
                            // Число без единицы наследует единицу параметра
                            // манифеста ({"rps": 2000} = 2000 rps)
                            let unit = manifest
                                .params
                                .iter()
                                .find(|spec| &spec.name == name)
                                .and_then(|spec| spec.unit.clone());
                            canvas_core::templates::TemplateParam { num, unit }
                        }
                        serde_json::Value::Object(obj) => canvas_core::templates::TemplateParam {
                            num: obj
                                .get("num")
                                .and_then(serde_json::Value::as_f64)
                                .ok_or_else(|| format!("параметр {name}: num обязателен"))?,
                            unit: obj
                                .get("unit")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                        },
                        _ => {
                            return Err(format!("параметр {name}: число или {{num, unit}}"));
                        }
                    };
                    overrides.insert(name.clone(), param);
                }
            }
            let node_id = next_free_id(&scene.canvas, "tpl");
            let mut node =
                canvas_core::templates::instantiate(&manifest, &overrides, node_id, x, y)
                    .map_err(|err| err.to_string())?;
            // FR-023: авто-высота — единая с GUI-путём (см. комментарий)
            fit_template_node_height(&mut node);
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг (после валидации, до вставки)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            // Формула шаблона с параметрами — в поток (FR-014)
            scene.recompute_flow();
            let summary = mcp_node_summary(&scene.canvas.nodes[index], true);
            Ok(serde_json::json!({
                "id": scene.canvas.nodes[index].id,
                "index": index,
                "node": summary,
            }))
        }
        "viewport_get" => {
            let position = camera.position();
            Ok(serde_json::json!({
                "x": position[0],
                "y": position[1],
                "zoom": camera.zoom(),
            }))
        }
        "viewport_set" => {
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            camera.set_center([x, y]);
            if let Some(zoom) = mcp_opt_f32(params, "zoom") {
                camera.set_zoom(zoom);
            }
            let position = camera.position();
            Ok(serde_json::json!({
                "x": position[0],
                "y": position[1],
                "zoom": camera.zoom(),
            }))
        }
        other => Err(format!("неизвестный инструмент: {other}")),
    }
}

impl App {
    fn on_key(&mut self, event: &KeyEvent) {
        // Активное редактирование (T7): клавиатура уходит в редактор
        if self.editing.is_some() {
            if event.state != ElementState::Pressed {
                return;
            }
            let ctrl = self.modifiers.control_key();
            let shift = self.modifiers.shift_key();
            // FR-021: при открытом popup подсказок навигация/выбор
            // перехватываются ДО команд редактора: Enter/Tab принимают
            // подсказку (НЕ коммитят заметку), Esc закрывает только popup
            // (повторный Esc — откат правки, прежнее поведение)
            if self.hints.open {
                match &event.logical_key {
                    Key::Named(NamedKey::ArrowDown) if !event.repeat => {
                        self.hints.move_selection(1);
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::ArrowUp) if !event.repeat => {
                        self.hints.move_selection(-1);
                        self.request_redraw();
                        return;
                    }
                    Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab) if !event.repeat => {
                        self.accept_hint();
                        return;
                    }
                    Key::Named(NamedKey::Escape) if !event.repeat => {
                        self.hints.reset();
                        self.request_redraw();
                        return;
                    }
                    _ => {}
                }
            }
            let Some(command) = map_key(&event.logical_key, ctrl, shift) else {
                return;
            };
            match command {
                KeyCommand::Commit => self.finish_editing(true),
                KeyCommand::Cancel => self.finish_editing(false),
                KeyCommand::Copy => {
                    if let Some(text) = self.editing.as_ref().and_then(|s| s.copy_selection()) {
                        self.clipboard.set(text);
                    }
                }
                KeyCommand::Cut => {
                    let text = match (self.editing.as_mut(), self.renderer.as_mut()) {
                        (Some(session), Some(renderer)) => {
                            session.cut_selection(renderer.font_system_mut())
                        }
                        _ => None,
                    };
                    if let Some(text) = text {
                        self.clipboard.set(text);
                        self.request_redraw();
                    }
                }
                KeyCommand::Paste => {
                    let text = self.clipboard.get();
                    let pasted = if let (Some(text), Some(session), Some(renderer)) =
                        (text, self.editing.as_mut(), self.renderer.as_mut())
                    {
                        session.insert_text(renderer.font_system_mut(), &text);
                        true
                    } else {
                        false
                    };
                    if pasted {
                        self.fit_note_size();
                        self.update_hints();
                        self.request_redraw();
                    }
                }
                other => {
                    let applied = if let (Some(session), Some(renderer)) =
                        (self.editing.as_mut(), self.renderer.as_mut())
                    {
                        session.apply(renderer.font_system_mut(), other);
                        true
                    } else {
                        false
                    };
                    if applied {
                        // Текст мог вырасти (wrap/новые строки) — подгоняем
                        // высоту заметки под контент прямо во время набора
                        self.fit_note_size();
                        // FR-021: popup подсказок — следом за правкой текста
                        self.update_hints();
                        self.request_redraw();
                    }
                }
            }
            return;
        }
        // Панель поиска (T14): открыта — клавиатура уходит в панель
        // (ввод/каретка/Enter/Esc/F3), канвас-хоткеи приглушены
        if self.search.is_open() {
            if event.state == ElementState::Pressed {
                self.on_search_key(event);
            }
            return;
        }
        // Ctrl+F — открыть панель поиска (T14; кириллическая раскладка — «а»);
        // активное редактирование сначала фиксируется
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("а"))
        {
            if self.editing.is_some() {
                self.finish_editing(true);
            }
            self.search.open();
            self.request_redraw();
            return;
        }
        // FR-018: панель шаблонов открыта — клавиатура уходит в неё
        // (фильтр, стрелки, Enter, Esc), канвас-хоткеи приглушены
        if self.template_panel.open && self.on_template_panel_key(event) {
            return;
        }
        // Ctrl+P — палитра шаблонов (FR-018; кириллическая раскладка — «з»).
        // Взаимоисключающе с wheel-меню: открытие закрывает его
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("p") || c.eq_ignore_ascii_case("з"))
        {
            self.wheel_menu = None;
            if self.template_panel.open {
                self.template_panel.close();
            } else {
                self.template_panel.open();
            }
            self.request_redraw();
            return;
        }
        // T21: модальный диалог глушит весь ввод канваса — Enter/Esc —
        // подтвердить/отменить, остальное игнорируется (П10/П11)
        if self.dialog.is_some() && event.state == ElementState::Pressed && !event.repeat {
            match event.logical_key {
                Key::Named(NamedKey::Enter) => {
                    self.confirm_dialog();
                }
                Key::Named(NamedKey::Escape) => self.cancel_dialog(),
                _ => {}
            }
            return;
        }
        // FR-026: клавиатура выпадающего меню настроек — ↑/↓ сдвигают
        // выделение, Enter применяет (модель popup FR-021); Esc обрабатывается
        // ниже — первым делом закрывает меню, панель остаётся открытой
        if self.settings_open
            && self.settings_dropdown.is_open()
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            let row = self.settings_dropdown.open_row.expect("меню открыто");
            let count = dropdown_options(row, &self.settings).len();
            match &event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.settings_dropdown.move_selection(1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.settings_dropdown.move_selection(-1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    let selected = self.settings_dropdown.selected;
                    self.apply_dropdown_choice(row, selected);
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                _ => {}
            }
        }
        // Esc закрывает раскрытие палитры (верхний transient), затем —
        // контекстное меню (T7), панель настроек, панель хоткеев (FR-004)
        if event.logical_key == Key::Named(NamedKey::Escape)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            if self.palette_hover.open.is_some() || self.palette_hover.pending() {
                // Раскрытая колонка палитры закрывается без снятия выделения
                self.palette_hover.reset();
                self.request_redraw();
                return;
            }
            if self.menu.take().is_some() {
                self.request_redraw();
                return;
            }
            if self.settings_open {
                if self.settings_dropdown.is_open() {
                    // FR-026: Esc при открытом меню закрывает ТОЛЬКО меню
                    // (повторный Esc закроет панель — семантика popup FR-021)
                    self.settings_dropdown.reset();
                } else {
                    self.settings_open = false;
                }
                self.request_redraw();
                return;
            }
            if self.hotkeys_open {
                self.hotkeys_open = false;
                self.request_redraw();
                return;
            }
            // FR-018: wheel-меню закрывается Esc (панель шаблонов — раньше,
            // в on_template_panel_key)
            if self.wheel_menu.take().is_some() {
                self.request_redraw();
                return;
            }
        }
        // F1 — панель горячих клавиш (FR-004): раскладконезависимая
        // функциональная клавиша; внутри редактора/поиска не работает
        // (клавиатура ушла туда раньше — return выше)
        if event.logical_key == Key::Named(NamedKey::F1)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.hotkeys_open = !self.hotkeys_open;
            self.request_redraw();
            return;
        }
        // Ctrl+C/V/D/X — буфер нодов (FR-003/FR-007; кириллица: с/м/в/ч —
        // те же физические клавиши). Ctrl+Z/Y — undo/redo (FR-006;
        // кириллица: я/н). Внутри редактора эти клавиши — текстовые (выше
        // return: Ctrl+X там — вырезание текста), во время поиска — панель
        if event.state == ElementState::Pressed && !event.repeat && self.modifiers.control_key() {
            if let Key::Character(c) = &event.logical_key {
                match c.to_lowercase().as_str() {
                    "c" | "с" => self.copy_selection(),
                    "v" | "м" => self.paste_clipboard(),
                    "d" | "в" => self.duplicate_selection(),
                    "x" | "ч" => self.cut_selection(),
                    "z" | "я" => {
                        // Ctrl+Shift+Z — общепринятый синоним redo
                        if self.modifiers.shift_key() {
                            self.redo_action();
                        } else {
                            self.undo_action();
                        }
                    }
                    "y" | "н" => self.redo_action(),
                    "g" | "п" => self.group_selection(),
                    _ => {}
                }
            }
        }
        // Ctrl+, — toggle панели настроек (кириллическая «б» — та же клавиша;
        // во время редактирования сюда не доходим — там Ctrl+Б это Bold)
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c) if c == "," || c == "б" || c == "Б")
        {
            self.settings_open = !self.settings_open;
            self.settings_dropdown.reset();
            self.request_redraw();
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Space) && !event.repeat {
            self.space_pressed = event.state == ElementState::Pressed;
            self.sync_cursor_icon();
            if !self.space_pressed {
                // Отпускание Space во время drag не должно оставлять ноду "прилипшей"
                // FR-006: применённое движение — undo-шаг; далее drag прерывается
                self.finish_interaction_undo();
                self.scene.dragging = None;
            }
        }
        // T23 (brainstorm-focus): F (русская раскладка — «А») — переключить
        // режим фокуса связей. Конфликтов нет: Ctrl+F — поиск (обработан
        // выше с модификатором), F3 — HUD/цикл поиска (функциональная клавиша)
        if event.state == ElementState::Pressed
            && !event.repeat
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c == "а" || c == "А")
        {
            self.toggle_focus_mode();
            return;
        }
        // F3 — цикл по результатам поиска (T14), если они есть (в т.ч. после
        // закрытия панели — rows сохранены); иначе — HUD с fps/p95 (T5)
        if event.logical_key == Key::Named(NamedKey::F3)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            if !self.search.rows.is_empty() {
                self.cycle_search(if self.modifiers.shift_key() { -1 } else { 1 });
            } else {
                self.hud_visible = !self.hud_visible;
                self.request_redraw();
            }
        }
        // Del — удалить выделенную ноду (каскадно со связями) или связь (T8).
        // Во время редактирования сюда не доходим — там Delete работает в тексте
        if event.logical_key == Key::Named(NamedKey::Delete)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.delete_selected();
        }
        // FR-011: mindmap-ветвление — Tab (дочерняя), Enter (сиблинг),
        // Ctrl+← (свернуть ветку), Ctrl+→ (развернуть). Только при выделенной
        // text-ноде; редактор/поиск/диалог приглушают канвас-хоткеи (return
        // выше — клавиатура уходит туда)
        if event.state == ElementState::Pressed && !event.repeat && !self.modifiers.shift_key() {
            let selected_index = match self.scene.selected {
                Some(Selection::Node(index)) => Some(index),
                _ => None,
            };
            if let Some(index) = selected_index {
                let is_text = self
                    .scene
                    .canvas
                    .nodes
                    .get(index)
                    .is_some_and(|node| node.kind() == NodeKind::Text);
                if is_text {
                    let ctrl = self.modifiers.control_key();
                    match &event.logical_key {
                        Key::Named(NamedKey::Tab) => self.mindmap_add_child(index),
                        Key::Named(NamedKey::Enter) if !ctrl => self.mindmap_add_sibling(index),
                        Key::Named(NamedKey::ArrowLeft) if ctrl => {
                            self.mindmap_set_collapsed(index, true);
                        }
                        Key::Named(NamedKey::ArrowRight) if ctrl => {
                            self.mindmap_set_collapsed(index, false);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn on_left_button(&mut self, state: ElementState) {
        self.left_pressed = state == ElementState::Pressed;
        // T15: первый клик по канвасу снимает WS_EX_NOACTIVATE — с этого
        // момента окно может получать фокус («WS_EX_NOACTIVATE до первого
        // клика», TASKS T15); ошибки не критичны, флаг ставим до вызова
        // (повторные клики не ретраят). Клавиатурный фокус ставим явно на
        // КАЖДОМ нажатии: в ребёнке Progman клик активирует top-level-предка,
        // а фокус ввода нашему окну системой не передаётся — без SetFocus
        // WM_KEYDOWN не доходят и текст в нодах не редактируется
        // (attach::focus_window, идемпотентен — внутри GetFocus-проверка)
        #[cfg(windows)]
        if self.desktop_mode && state == ElementState::Pressed && self.desktop_hierarchy.is_some() {
            if !self.desktop_activation_enabled {
                self.desktop_activation_enabled = true;
                if let Some(raw) = self.window_hwnd() {
                    let hwnd = Self::hwnd(raw);
                    if let Err(err) = canvas_shell::desktop::attach::enable_activation(hwnd) {
                        tracing::warn!(%err, "не удалось снять WS_EX_NOACTIVATE");
                    }
                }
            }
            if let Some(raw) = self.window_hwnd() {
                let hwnd = Self::hwnd(raw);
                if let Err(err) = canvas_shell::desktop::attach::focus_window(hwnd) {
                    tracing::warn!(%err, "не удалось передать клавиатурный фокус");
                }
            }
        }
        if self.space_pressed {
            return; // Space+drag — панорамирование (SPEC §8)
        }
        match state {
            ElementState::Pressed => {
                // Панель поиска (T14): клик по строке — прыжок, мимо панели —
                // закрыть; канвасу клик не достаётся. Проверяется первой —
                // панель висит поверх всех оверлеев
                if self.search.is_open() {
                    let viewport = self.viewport_logical();
                    let lay = search_layout(viewport[0], viewport[1], &self.search);
                    let mut handled = false;
                    for (visible, rect) in lay.row_rects.iter().enumerate() {
                        let row_rect = rect_xywh(*rect);
                        if point_in_rect(row_rect, self.cursor) {
                            let row = self.search.scroll_top + visible;
                            self.search.selected = Some(row);
                            self.jump_to_search_row(row);
                            handled = true;
                            break;
                        }
                    }
                    if !handled && !point_in_rect(rect_xywh(lay.panel_rect), self.cursor) {
                        self.search.close();
                    }
                    self.request_redraw();
                    return;
                }
                // FR-018: wheel-меню шаблонов — клики обрабатываются до
                // канваса (оверлей поверх всего). Плашка категории — выбор
                // категории (растут шаблонные кольца); плашка шаблона —
                // инстанциация в world-точку открытия; мимо плашек, но
                // рядом — глотаем, заметно дальше — закрыть.
                // Любой клик глотается — dismiss не создаёт заметку.
                if let Some(menu) = self.wheel_menu.clone() {
                    let [vw, vh] = self.viewport_logical();
                    let categories = self.templates.categories();
                    let template_count = menu
                        .category
                        .as_deref()
                        .map(|c| self.templates.by_category(c).len())
                        .unwrap_or(0);
                    let geo = template_ui::wheel_geometry(
                        menu.screen,
                        vw,
                        vh,
                        categories.len(),
                        template_count,
                    );
                    match geo.hit(self.cursor) {
                        Some(WheelHit::Category(i)) => {
                            if let Some(menu_mut) = self.wheel_menu.as_mut() {
                                menu_mut.category = Some(categories[i].to_owned());
                            }
                        }
                        Some(WheelHit::Template(i)) => {
                            let category = menu.category.expect("категория выбрана");
                            let manifest = self.templates.by_category(&category)[i].clone();
                            let world = menu.world;
                            self.wheel_menu = None;
                            self.instantiate_template_at(&manifest, world);
                        }
                        None => {
                            // FR-022: клик по кнопке-хабу — «назад» (сброс
                            // категории) или «закрыть»; дальше — как раньше:
                            // рядом глотаем, заметно дальше — закрыть.
                            if geo.hub_hit(self.cursor) {
                                if let Some(menu_mut) = self.wheel_menu.as_mut() {
                                    menu_mut.category = None;
                                }
                                if menu.category.is_none() {
                                    self.wheel_menu = None;
                                }
                            } else {
                                let dx = self.cursor[0] - geo.center[0];
                                let dy = self.cursor[1] - geo.center[1];
                                let outside = (dx * dx + dy * dy).sqrt() > geo.extent + 12.0;
                                if outside {
                                    self.wheel_menu = None;
                                }
                            }
                        }
                    }
                    self.request_redraw();
                    return;
                }
                // FR-018: палитра шаблонов — клик по чипу категории (тогл
                // фильтра) или строке шаблона (инстанциация в центр
                // viewport), мимо панели — закрыть; канвасу клик не достаётся
                if self.template_panel.open {
                    let viewport = self.viewport_logical();
                    let rows = template_panel_rows(&self.templates, &self.template_panel);
                    let lay = template_panel_layout(
                        viewport[0],
                        viewport[1],
                        &self.templates,
                        &self.template_panel,
                        &rows,
                    );
                    let mut handled = false;
                    for (rect, name, _active) in &lay.category_rects {
                        if point_in_rect(rect_xywh(*rect), self.cursor) {
                            self.template_panel.category =
                                if self.template_panel.category.as_deref() == Some(name) {
                                    None
                                } else {
                                    Some(name.clone())
                                };
                            self.template_panel.selected = 0;
                            self.template_panel.scroll_top = 0;
                            handled = true;
                            break;
                        }
                    }
                    if !handled {
                        // FR-024: строки панели — секции (заголовки, клик
                        // глотается) и карточки шаблонов (вставка в центр)
                        for (rect, row) in lay.row_rects.iter().zip(lay.rows.iter()) {
                            if point_in_rect(rect_xywh(*rect), self.cursor) {
                                if let PanelRow::Template(index) = row {
                                    let manifest = self.templates.list()[*index].clone();
                                    let center = self.viewport_center_world();
                                    self.template_panel.close();
                                    self.instantiate_template_at(&manifest, center);
                                }
                                handled = true;
                                break;
                            }
                        }
                    }
                    if !handled && !point_in_rect(rect_xywh(lay.panel_rect), self.cursor) {
                        self.template_panel.close();
                    }
                    self.request_redraw();
                    return;
                }
                // Панель настроек (screen-space): клики обрабатываются до
                // канваса — кнопка/панель поверх и «прозрачности» не дают
                let viewport = self.viewport_logical();
                // Кнопка переключения темы — рядом с кнопкой настроек
                if point_in_rect(
                    theme_button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    self.toggle_theme();
                    self.request_redraw();
                    return;
                }
                if point_in_rect(
                    button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    self.settings_open = !self.settings_open;
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                if self.settings_open {
                    // FR-026: layout панели с группами — hit-тесты по строкам
                    let layout = panel_layout(self.settings.button_corner, viewport);
                    // Открытое выпадающее меню — первый приоритет: клик по
                    // пункту применяет значение; клик мимо меню закрывает
                    // ТОЛЬКО меню (панель остаётся открытой — двухэтапный
                    // dismiss), клик по другой строке обработается ниже
                    if let Some(open_row) = self.settings_dropdown.open_row {
                        let items = dropdown_options(open_row, &self.settings);
                        let anchor = layout.row_rect(open_row).unwrap_or([0.0; 4]);
                        let menu_rect = dropdown_layout(anchor, viewport, items.len());
                        if point_in_rect(menu_rect, self.cursor) {
                            if let Some(index) =
                                dropdown_item_at(menu_rect, items.len(), self.cursor)
                            {
                                self.apply_dropdown_choice(open_row, index);
                            }
                            // Выбор угла кнопки перепривязывает панель —
                            // меню закрывается в любом случае
                            self.settings_dropdown.reset();
                            self.request_redraw();
                            return;
                        }
                        self.settings_dropdown.reset();
                        if row_at(&layout, self.cursor).is_none() {
                            // Клик вне меню и не по строке панели: меню
                            // закрыто, панель осталась, канвасу клик не
                            // достаётся (иначе создал бы заметку)
                            self.request_redraw();
                            return;
                        }
                    }
                    if let Some(row) = row_at(&layout, self.cursor) {
                        match row_kind(row) {
                            RowKind::Toggle => self.apply_toggle_row(row),
                            RowKind::Dropdown => {
                                // Клик по dropdown-строке открывает меню
                                // значений (НЕ меняет значение); повторный
                                // клик по той же строке закрывает
                                if self.settings_dropdown.open_row == Some(row) {
                                    self.settings_dropdown.reset();
                                } else {
                                    self.settings_dropdown.open(row, &self.settings);
                                }
                            }
                        }
                    } else if !point_in_rect(layout.rect, self.cursor) {
                        // Клик мимо панели — закрыть; канвасу клик не достаётся
                        // (иначе двойной клик мимо создал бы заметку)
                        self.settings_open = false;
                        self.settings_dropdown.reset();
                    }
                    self.request_redraw();
                    return;
                }
                // Панель хоткеев (FR-004.1, тогл): панель «видно/не видно»
                // устойчива — клик мимо НЕ закрывает (переключение: F1,
                // пункт меню канваса, Esc) и проходит в канвас; клик по
                // самой панели — глотается (строки не интерактивны)
                if self.hotkeys_open {
                    let panel = hotkeys_panel_rect(viewport);
                    if point_in_rect(panel, self.cursor) {
                        self.request_redraw();
                        return;
                    }
                }
                // Миникарта (T13, SPEC §6.1): клик — центрирование камеры,
                // drag — world-точка под курсором следует за ним. Квад
                // рисуется поверх всего — проверка до канвас-хит-тестов
                if let Some(rect) = self.minimap_rect() {
                    if point_in_rect(
                        [rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1]],
                        self.cursor,
                    ) {
                        self.center_camera_on_minimap_cursor();
                        self.minimap_drag = true;
                        self.request_redraw();
                        return;
                    }
                }
                let world = self.cursor_world();
                // T21: модальный диалог поверх всего — кнопки Да/Нет
                // (клики мимо панели не закрывают: установка — явный выбор)
                if self.dialog.is_some() {
                    for (i, rect) in self.dialog_button_rects().iter().enumerate() {
                        let [x, y, w, h] = *rect;
                        if self.cursor[0] >= x
                            && self.cursor[0] <= x + w
                            && self.cursor[1] >= y
                            && self.cursor[1] <= y + h
                        {
                            if i == 0 {
                                self.confirm_dialog();
                            } else {
                                self.cancel_dialog();
                            }
                            break;
                        }
                    }
                    self.request_redraw();
                    return;
                }
                // Выборочный hit-test (T5 + группы): ребёнок группы раньше
                // самой группы, не-group с меньшей площадью в приоритете
                let hit = self.selective_hit(world);
                // Палитра выделения (FR-009/FR-010): клик по строке ОТКРЫТОЙ
                // группы — действие, по кнопке-триггеру — пин-переключение
                // раскрытия (WAI-ARIA menu button), по бару — глотается;
                // проверяется ДО канваса — тулбар поверх выделения
                if let Some((lay, groups, open)) = self.palette_view() {
                    match palette_hit(&lay, self.cursor, open) {
                        Some(PaletteHit::Entry { group, entry }) => {
                            let action = groups[group].entries[entry].action.clone();
                            self.apply_palette_action(action);
                            // Действие выполнено — раскрытие закрывается
                            // (состав групп мог измениться; Radix: закрытие
                            // меню по выбору пункта)
                            self.palette_hover.reset();
                            self.request_redraw();
                            return;
                        }
                        Some(PaletteHit::Trigger(group)) => {
                            // Пин: клик открывает без задержки / закрывает
                            // повторным кликом — стабильность для точного
                            // наведения, как у menu-button в вебе
                            self.palette_hover.toggle_trigger(group);
                            self.request_redraw();
                            return;
                        }
                        Some(PaletteHit::Bar) => {
                            self.request_redraw();
                            return;
                        }
                        None => {}
                    }
                }
                // Открытое меню канваса (T7): клик по пункту — действие,
                // клик по поверхности меню (паддинг) — глотается, меню
                // ОСТАЁТСЯ открытым (Radix: клик внутри поверхности меню
                // не закрывает), клик мимо — закрыть (dismiss-клик в канвас
                // не проходит). M5: подменю проверяется ПЕРВЫМ — его колонка
                // правее базового меню. Hit-test — в логических px (курсор).
                if self.menu.is_some() {
                    let in_base = self
                        .menu_open_rect()
                        .is_some_and(|rect| point_in_rect(rect, self.cursor));
                    let in_submenu = self
                        .menu
                        .as_ref()
                        .and_then(|m| m.submenu.as_ref())
                        .map(submenu_rect)
                        .is_some_and(|rect| point_in_rect(rect, self.cursor));
                    // 1. Пункт подменю — действие
                    if let Some(submenu) = self.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                        if let Some(i) = submenu_item_at(submenu, self.cursor) {
                            let action = submenu.entries[i].action.clone();
                            self.menu = None;
                            match action {
                                canvas_app::ui::SubmenuAction::Insert(widget_id) => {
                                    self.insert_widget_from_menu(&widget_id);
                                }
                                // T21-C (П11): удаление пакета — с подтверждением;
                                // меню уже закрыто, модальный диалог поверх
                                canvas_app::ui::SubmenuAction::Remove(widget_id) => {
                                    let name = self
                                        .widgets
                                        .registry
                                        .get(&widget_id)
                                        .map(|p| p.manifest.name.clone())
                                        .unwrap_or(widget_id.clone());
                                    self.dialog =
                                        Some(AppDialog::RemovePackage { widget_id, name });
                                }
                            }
                            self.request_redraw();
                            return;
                        }
                    }
                    // 2. Поверхность подменю без пункта — глотается, не закрывает
                    if in_submenu {
                        self.request_redraw();
                        return;
                    }
                    // 3. Пункт или паддинг базового меню
                    if let Some(menu) = self.menu.take() {
                        if let Some(i) =
                            menu_item_at_for(menu.origin, self.cursor, CANVAS_MENU_ITEMS.len())
                        {
                            match CANVAS_MENU_ITEMS[i] {
                                CanvasMenuItem::NewGroup => {
                                    let center = self.viewport_center_world();
                                    let group = plan_group_at(&self.scene.canvas, center);
                                    self.insert_group(group);
                                }
                                // T23: переключение из меню — рантайм,
                                // без записи конфига (как и хоткей F)
                                CanvasMenuItem::FocusMode => self.toggle_focus_mode(),
                                // FR-004.1: тогл оверлея хоткеев из меню
                                // (панель «видно/не видно», галочка ✓)
                                CanvasMenuItem::Hotkeys => {
                                    self.hotkeys_open = !self.hotkeys_open;
                                }
                                // M5 (T20-F): открыть подменю пакетов;
                                // повторный клик — тоггл (закрыть). Пустой
                                // список — честная строка «(нет установленных)».
                                // T21-C: под каждой вставкой — секция
                                // удаления пакетов (П11)
                                CanvasMenuItem::Widgets => {
                                    if menu.submenu.is_some() {
                                        // Тоггл: подменю уже открыто — закрыть
                                        self.menu = Some(ContextMenu {
                                            origin: menu.origin,
                                            submenu: None,
                                        });
                                    } else {
                                        let submenu_origin = submenu_origin_next_to(menu.origin);
                                        let mut entries: Vec<SubmenuEntry> = self
                                            .widgets
                                            .menu_entries()
                                            .into_iter()
                                            .map(|(widget_id, label)| SubmenuEntry {
                                                action: canvas_app::ui::SubmenuAction::Insert(
                                                    widget_id,
                                                ),
                                                label,
                                            })
                                            .collect();
                                        entries.extend(
                                            self.widgets.menu_entries().into_iter().map(
                                                |(widget_id, label)| SubmenuEntry {
                                                    action: canvas_app::ui::SubmenuAction::Remove(
                                                        widget_id,
                                                    ),
                                                    label: format!("— Удалить: {label}"),
                                                },
                                            ),
                                        );
                                        self.menu = Some(ContextMenu {
                                            origin: menu.origin,
                                            submenu: Some(Submenu {
                                                origin: submenu_origin,
                                                entries,
                                            }),
                                        });
                                    }
                                }
                                // T15: переключатель desktop-режима. Вход
                                // (runtime, без --desktop): перезапуск себя с
                                // --desktop через single-instance handoff —
                                // in-place SetParent не работает (Vulkan-swapchain
                                // не презентует в ребёнка Progman, Renderer
                                // фиксируется с prefer_dx12 при старте). Выход
                                // (уже встроены): in-place detach — DX12-рендерер
                                // в обычном окне презентует, пересоздание не нужно.
                                // На не-Windows — warn.
                                CanvasMenuItem::DesktopMode => {
                                    #[cfg(windows)]
                                    {
                                        if self.desktop_mode && self.desktop_hierarchy.is_some() {
                                            self.leave_desktop();
                                        } else {
                                            match self.spawn_desktop_relaunch() {
                                                Ok(()) => tracing::info!(
                                                    "перезапуск на --desktop: новый инстанс \
                                                     закроет текущий (single-instance handoff)"
                                                ),
                                                Err(err) => {
                                                    tracing::warn!(
                                                        %err,
                                                        "перезапуск на --desktop не удался"
                                                    );
                                                    canvas_shell::desktop::attach::fallback_message_box(&format!(
                                                        "Не удалось перезапустить CanvasDesk \
                                                         в режиме десктопа:\n{err}\n\nЗапустите \
                                                         приложение вручную с флагом --desktop."
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    #[cfg(not(windows))]
                                    {
                                        tracing::warn!(
                                            "desktop-режим не поддерживается на этой платформе"
                                        );
                                    }
                                }
                            }
                            self.request_redraw();
                            return;
                        }
                        if in_base {
                            // Паддинг базового меню — меню остаётся открытым
                            self.menu = Some(menu);
                            self.request_redraw();
                            return;
                        }
                        // Клик мимо — меню закрыто (take выше), клик глотается
                        self.request_redraw();
                        return;
                    }
                }
                // Активное редактирование (T7/T8): клик внутри области
                // редактирования — в курсор, клик снаружи — commit и обычная
                // обработка
                if let Some(target) = self.editing.as_ref().map(EditingSession::target) {
                    let avoid = self.settings.edges_avoid_nodes;
                    let inside = match target {
                        EditTarget::Node(index) => hit == Some(index),
                        EditTarget::Edge(index) => edge_edit_area(&self.scene.canvas, index, avoid)
                            .is_some_and(|(origin, width, height)| {
                                world[0] >= origin[0]
                                    && world[0] <= origin[0] + width
                                    && world[1] >= origin[1]
                                    && world[1] <= origin[1] + height
                            }),
                    };
                    if inside {
                        let zoom_px = self.zoom_px();
                        if let (Some(session), Some(renderer)) =
                            (self.editing.as_mut(), self.renderer.as_mut())
                        {
                            if let Some((origin, _, _)) =
                                session_area(&self.scene.canvas, session, avoid)
                            {
                                let x = ((world[0] - origin[0]) * zoom_px) as i32;
                                let y = ((world[1] - origin[1]) * zoom_px) as i32;
                                session.click(renderer.font_system_mut(), x, y);
                                self.editor_dragging = true;
                            }
                        }
                        self.request_redraw();
                        return;
                    }
                    self.finish_editing(true);
                }
                // Хэндлы концов выделенной связи (CR-002): захват хэндла —
                // drag перепривязки без удаления. Проверка ДО портов: хэндл
                // сидит на порту, занятом существующей связью. Зона — та же,
                // что у портов (CR-003, из настроек).
                if let Some(Selection::Edge(edge_index)) = self.scene.selected {
                    if let Some(end) = self.edge_handle_at(edge_index, world) {
                        self.edge_drag = Some(EdgeDrag::Rebind { edge_index, end });
                        self.request_redraw();
                        return;
                    }
                }
                // Порт hover-ноды (T8): начало drag резиновой линии новой
                // связи — drag ноды/resize/двойной клик не начинаются.
                // У групп портов нет: edge-drag с группы не начинается.
                // Зона захвата — из настроек (CR-003).
                if let Some(node_index) = self.hovered {
                    let port = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .filter(|node| node.kind() != NodeKind::Group)
                        .and_then(|node| {
                            port_at(node, world, self.camera.zoom(), self.settings.port_zone_px)
                        });
                    if let Some(side) = port {
                        let from_node = self.scene.canvas.nodes[node_index].id.clone();
                        // FR-014: Shift+drag — value-ребро (поток значений),
                        // обычный drag — контрольная связь (дефолт)
                        let value_flow = self.modifiers.shift_key();
                        self.edge_drag = Some(EdgeDrag::New {
                            from_node,
                            from_side: side,
                            value_flow,
                        });
                        self.request_redraw();
                        return;
                    }
                }
                // FR-018: Shift+клик по пустому месту — wheel-меню шаблонов
                // в точке курсора (мишень инстанциации — world-точка).
                // Нода/связь под курсором — обычная обработка выше.
                if self.modifiers.shift_key()
                    && hit.is_none()
                    && edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes).is_none()
                {
                    self.wheel_menu = Some(template_ui::WheelMenu {
                        screen: self.cursor,
                        world,
                        category: None,
                    });
                    self.request_redraw();
                    return;
                }
                // Двойной клик (winit его не даёт — свой детектор, T7):
                // по пустому месту — новая заметка, по text-ноде —
                // редактирование, по линии связи — лейбл связи (T8)
                if self.double_click.register(Instant::now(), self.cursor) {
                    let avoid = self.settings.edges_avoid_nodes;
                    match hit {
                        None => match edge_at(&self.scene.canvas, world, avoid) {
                            Some(edge_index) => self.begin_editing_edge(edge_index),
                            None => {
                                let index = self.create_note_at(world);
                                self.begin_editing(index);
                            }
                        },
                        // T17 (SPEC §7.4 п.7): двойной клик по файловой
                        // ноде — открыть ассоциацией «как в Explorer»
                        // (ShellExecuteEx SEE_MASK_INVOKEIDLIST);
                        // text-ноды — редактирование (T7)
                        Some(index) => {
                            // CR-006: у виджет-ноды текстового редактора нет —
                            // двойной клик (по хрому) не открывает его; клики
                            // по контенту до этой ветки не доходят (guard выше)
                            if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                                self.request_redraw();
                                return;
                            }
                            #[cfg(windows)]
                            if let Some(file) = self.scene.canvas.nodes[index].file.clone() {
                                let path = resolve_node_path(&file, &self.scene.canvas_dir());
                                if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                                    tracing::warn!(
                                        %err,
                                        path = %path.display(),
                                        "не удалось открыть файл"
                                    );
                                }
                            } else {
                                self.begin_editing(index);
                            }
                            #[cfg(not(windows))]
                            self.begin_editing(index);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                // Ручной resize (T7): захват за правый нижний угол ноды
                if let Some(index) = hit {
                    if in_resize_corner(&self.scene.canvas.nodes[index], world) {
                        self.scene.selected = Some(Selection::Node(index));
                        self.resizing = Some(index);
                        // FR-006: отложенный снапшот «до» resize — шаг
                        // закроется на отпускании при изменении размеров
                        self.begin_pending_undo();
                        self.request_redraw();
                        return;
                    }
                }
                match hit {
                    Some(index) => {
                        // CR-006: ЛКМ по КОНТЕНТУ виджет-ноды — ввод принадлежит
                        // виджету (WIDGETS.md §8.5). Ни выделения, ни drag,
                        // ни рамки выделения, ни семени фокуса: в live ввод
                        // и так уходит в HWND WebView2, в snapshot/placeholder
                        // клик глотается канвасом без оверлеев. Хром
                        // (заголовок 28 px / рамка 8 px) — прежнее поведение.
                        if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                            let node = &self.scene.canvas.nodes[index];
                            let rect = [node.x, node.y, node.width, node.height];
                            if canvas_widgets::layout::hit_test(&rect, world)
                                == canvas_widgets::layout::WidgetHit::Content
                            {
                                tracing::debug!(
                                    node_id = %node.id,
                                    "клик по контенту виджета — канвас без оверлеев"
                                );
                                self.request_redraw();
                                return;
                            }
                        }
                        // Ctrl/Shift + клик (CR-001.3): уже выделенное
                        // (в т.ч. одиночный якорь) остаётся, клик-нутая
                        // тоглится; drag с модификатором не начинается
                        // (это правка выделения, не перемещение)
                        if self.modifiers.control_key() || self.modifiers.shift_key() {
                            let primary =
                                self.scene.selected.and_then(|selection| match selection {
                                    Selection::Node(index) => Some(index),
                                    Selection::Edge(_) => None,
                                });
                            let anchor = toggle_selection_with_primary(
                                primary,
                                &mut self.scene.selected_nodes,
                                index,
                            );
                            self.scene.selected = anchor.map(Selection::Node);
                            self.request_redraw();
                            return;
                        }
                        // Обычный клик: нода вне набора — набор сбрасывается
                        // (одиночное выделение); нода В наборе — тянем набор
                        let in_set = self.scene.selected_nodes.contains(&index);
                        self.scene.selected = Some(Selection::Node(index));
                        if !in_set {
                            self.scene.selected_nodes.clear();
                        }
                        // Drag (T7/CR-001): исходные позиции — одна нода или
                        // весь набор (+ дети групп); на движении delta к всем
                        let origins =
                            drag_origins(&self.scene.canvas, index, &self.scene.selected_nodes);
                        // FR-006: отложенный снапшот «до» перемещения — шаг
                        // закроется на отпускании при фактическом сдвиге
                        self.begin_pending_undo();
                        // FR-012: новый drag отменяет settle-анимацию и
                        // сбрасывает цель втягивания
                        self.settle_anim = None;
                        self.group_drop_target = None;
                        self.scene.dragging = Some(DragState {
                            primary: index,
                            grab_world: world,
                            origins,
                        });
                    }
                    // Промах по нодам: hit-test связей (T8) — ближайшая
                    // в допуске EDGE_HIT_TOLERANCE, иначе сброс выделения.
                    // Рамка (CR-001): drag с пустого места тянет выделение —
                    // финал на отпускании (порог клик/драг отсекает клики)
                    None => {
                        self.scene.selected =
                            edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes)
                                .map(Selection::Edge);
                        self.scene.selected_nodes.clear();
                        self.select_rect = Some((world, world, self.cursor));
                    }
                }
                self.request_redraw();
            }
            ElementState::Released => {
                // Рамка выделения (CR-001): движение больше порога —
                // выделяем ноды, пересекающие прямоугольник (AABB,
                // частичное вхождение считается); клик без движения уже
                // отработал в Pressed (edge/сброс)
                if let Some((start, _, press)) = self.select_rect.take() {
                    let moved = (self.cursor[0] - press[0]).abs() > SELECT_DRAG_THRESHOLD
                        || (self.cursor[1] - press[1]).abs() > SELECT_DRAG_THRESHOLD;
                    if moved {
                        let rect = rubber_band_rect(start, self.cursor_world());
                        self.scene.selected_nodes = nodes_in_rect(&self.scene.canvas, rect);
                        self.scene.selected = None;
                    }
                    self.request_redraw();
                }
                // Drop резиновой линии: новая связь (T8) или перепривязка
                // конца существующей (CR-002). На другую ноду — применяем,
                // в пустоту/на ту же ноду/на зеркальный конец — отмена
                if let Some(drag) = self.edge_drag.take() {
                    let world = self.cursor_world();
                    match drag {
                        EdgeDrag::New {
                            from_node,
                            from_side,
                            value_flow,
                        } => {
                            if let Some(target) = self.selective_hit(world) {
                                let to_node = &self.scene.canvas.nodes[target];
                                let to_id = to_node.id.clone();
                                if to_id != from_node {
                                    let to_side = nearest_side(to_node, world);
                                    if value_flow {
                                        // FR-014: value-ребро, замыкающее цикл,
                                        // — диалог (контрольная связь / отмена);
                                        // валидное — создаётся сразу
                                        if canvas_core::creates_value_cycle(
                                            &self.scene.canvas,
                                            &from_node,
                                            &to_id,
                                        ) {
                                            self.dialog = Some(AppDialog::EdgeCycle {
                                                from_node,
                                                from_side,
                                                to_node: to_id,
                                                to_side,
                                            });
                                        } else {
                                            self.create_edge(
                                                from_node,
                                                from_side,
                                                to_id,
                                                to_side,
                                                FlowKind::Value,
                                            );
                                        }
                                    } else {
                                        self.create_edge(
                                            from_node,
                                            from_side,
                                            to_id,
                                            to_side,
                                            FlowKind::Control,
                                        );
                                    }
                                }
                            }
                        }
                        // CR-002: перепривязка конца — id/лейбл/цвет/стиль
                        // сохраняются (retarget_edge), сторона — ближайшая
                        // к курсору сторона целевой ноды
                        EdgeDrag::Rebind { edge_index, end } => {
                            if let Some(target) = self.selective_hit(world) {
                                let target_node = &self.scene.canvas.nodes[target];
                                let target_id = target_node.id.clone();
                                let side = nearest_side(target_node, world);
                                // FR-006: перепривязка — undo-шаг ДО мутации;
                                // retarget сам отклонит бесполезный перенос —
                                // тогда шаг снимается (no-op клики не копятся)
                                self.push_undo();
                                if canvas_core::retarget_edge(
                                    &mut self.scene.canvas,
                                    edge_index,
                                    end,
                                    &target_id,
                                    side,
                                ) {
                                    self.scene.mark_dirty();
                                    // FR-014: перепривязка могла изменить
                                    // топологию value-потока
                                    self.scene.recompute_flow();
                                } else {
                                    self.scene.undo_stack.pop_back();
                                }
                            }
                        }
                    }
                    self.request_redraw();
                }
                // FR-012: отпускание drag — втягивание в группу (зона была
                // подсвечена) или вынос из группы (отпускание вне rect своей
                // явной группы); каждое — свой undo-шаг membership
                let drop_target = self.group_drop_target.take();
                if let Some(group_index) = drop_target {
                    self.group_insert_dragged(group_index);
                } else {
                    self.group_drag_out_released();
                }
                // FR-006: закрытие отложенного drag/resize — undo-шаг при
                // фактическом изменении (клик без движения не шаг)
                self.finish_interaction_undo();
                self.scene.dragging = None;
                self.editor_dragging = false;
                self.resizing = None;
                self.minimap_drag = false;
            }
        }
    }

    fn on_right_button(&mut self, state: ElementState, event_loop: &ActiveEventLoop) {
        // Вне Windows параметр не читается (системное меню T17 — Win32);
        // явный let вместо underscore-имени: имя остаётся осмысленным
        #[cfg(not(windows))]
        let _ = event_loop;
        if state != ElementState::Pressed {
            return;
        }
        // ПКМ во время редактирования — сначала commit (T7)
        if self.editing.is_some() {
            self.finish_editing(true);
        }
        let world = self.cursor_world();
        match self.selective_hit(world) {
            // Нода: выделить → палитра выделения под нодой (FR-009/FR-010).
            // Мультивыделение сохраняется при ПКМ по выделенной ноде
            Some(index) => {
                if !self.scene.selected_nodes.contains(&index) {
                    self.scene.selected_nodes.clear();
                }
                self.scene.selected = Some(Selection::Node(index));
                self.menu = None;
            }
            // Связь: выделить → палитра связи (Стиль/Толщина/Цвет);
            // мимо — меню пустого канваса или закрытие (десктоп-меню T17)
            None => {
                let avoid = self.settings.edges_avoid_nodes;
                match edge_at(&self.scene.canvas, world, avoid) {
                    Some(edge_index) => {
                        self.scene.selected = Some(Selection::Edge(edge_index));
                        self.scene.selected_nodes.clear();
                        self.menu = None;
                    }
                    None => {
                        // T17 (SPEC §7.4 п.6): в --desktop ПКМ по пустому месту —
                        // системное меню десктопа (нативное Win32: Открыть
                        // канвас / Новый текстовый файл / иконки / автозапуск /
                        // Выход); вне --desktop — меню пустого канваса
                        // (создание группы), повторный ПКМ мимо закрывает его.
                        // Исключение: Shift+ПКМ в --desktop открывает canvas-меню
                        // (пункт «✓ Режим десктопа» — выход из встройки без
                        // выхода из приложения). Origin — логические px
                        // (screen-space меню).
                        // Взаимоисключение поповеров: открытие меню прячет
                        // палитру и сбрасывает её раскрытие
                        self.palette_hover.reset();
                        #[cfg(windows)]
                        let desktop_menu = self.desktop_mode
                            && self.desktop_hierarchy.is_some()
                            && !self.modifiers.shift_key();
                        #[cfg(not(windows))]
                        let desktop_menu = false;
                        if desktop_menu {
                            self.menu = None;
                            #[cfg(windows)]
                            self.desktop_menu(event_loop);
                        } else {
                            self.menu = match self.menu.take() {
                                // Повторный ПКМ по тому же пустому месту —
                                // закрыть (тоггл, как у ноды/связи)
                                Some(_) => None,
                                _ => Some(ContextMenu {
                                    origin: self.cursor,
                                    submenu: None,
                                }),
                            };
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    /// Системное контекстное меню десктопа (T17, план §3): нативное
    /// Win32-меню через TrackPopupMenu(TPM_RETURNCMD) — команда приходит
    /// return'ом, воронка WM_COMMAND не строится (отступление §8.1).
    /// Меню T7 (цвета нод) не затрагивается — зоны не пересекаются
    /// (§8.2). Отказы всех Win32-шагов — warn + деградация (R14).
    #[cfg(windows)]
    fn desktop_menu(&mut self, event_loop: &ActiveEventLoop) {
        use canvas_shell::desktop::menu::DesktopMenuCommand as Cmd;
        let Some(raw) = self.window_hwnd() else {
            return;
        };
        let hwnd = Self::hwnd(raw);
        // Галочки меню: иконки (сейчас скрыты — инверсная семантика
        // пункта «Показать») и автозапуск (факт реестра HKCU Run)
        let icons_hidden = self
            .icon_guard
            .as_ref()
            .is_some_and(|guard| guard.hidden_by_us());
        let autostart_on = canvas_shell::desktop::interop::autostart_enabled();
        let Some(command) = canvas_shell::desktop::menu::popup(hwnd, icons_hidden, autostart_on)
        else {
            return; // отмена — клик мимо/Esc
        };
        match command {
            // Второй экземпляр в оконном режиме с текущим канвасом
            // (план §8.3): редактирование не ломает десктоп-встройку
            Cmd::OpenCanvas => {
                canvas_shell::desktop::interop::spawn_window_instance(&self.scene.path);
            }
            // Файл в каталоге канваса + нода в точке ПКМ (план §8.4):
            // вотчер T10/поиск T14 подхватят автоматически
            Cmd::NewTextFile => self.create_text_file_node(),
            // Toggle иконок: show — «показать» независимо от того, кто
            // скрывал (ПКМ Explorer в --desktop перехвачен канвасом)
            Cmd::ToggleIcons => {
                if let Some(guard) = self.icon_guard.as_mut() {
                    if guard.hidden_by_us() {
                        guard.show();
                    } else {
                        guard.hide();
                    }
                }
            }
            // Toggle автозапуска (HKCU Run) — галочка перечитается при
            // следующем открытии меню
            Cmd::ToggleAutostart => {
                if let Err(err) = canvas_shell::desktop::interop::set_autostart(!autostart_on) {
                    tracing::warn!(%err, "не удалось переключить автозапуск");
                }
            }
            // Штатный выход — единая точка с CloseRequested
            Cmd::Exit => self.shutdown(event_loop),
        }
    }

    /// «Новый текстовый файл» из десктоп-меню (T17, план §8.4): файл в
    /// каталоге канваса (уникальное имя) + file-нода в позиции ПКМ —
    /// образец вставки дропа T9; вотчер T10 и поиск T14 подхватят
    /// автоматически.
    #[cfg(windows)]
    fn create_text_file_node(&mut self) {
        let world = self.cursor_world();
        let dir = self.scene.canvas_dir();
        // Уникальное имя: «Новая заметка.txt», при коллизии — « 2», « 3»…
        let base = "Новая заметка";
        let mut name = format!("{base}.txt");
        let mut counter = 1u32;
        while dir.join(&name).exists() {
            counter += 1;
            name = format!("{base} {counter}.txt");
        }
        let path = dir.join(&name);
        if let Err(err) = std::fs::write(&path, "") {
            tracing::warn!(%err, path = %path.display(), "не удалось создать файл");
            return;
        }
        // FR-006: файловая нода из десктоп-меню — undo-шаг (снапшот до —
        // файл на диске остаётся, откатывается только карточка)
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "file");
        let node = Node::file(
            id,
            &name,
            world[0],
            world[1],
            canvas_app::ui::DROP_CARD_W,
            canvas_app::ui::DROP_CARD_H,
        );
        self.scene.canvas.nodes.push(node);
        let index = self.scene.canvas.nodes.len() - 1;
        let node_ref = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node_ref);
        self.scene.selected = Some(Selection::Node(index));
        self.scene.mark_dirty();
        // Поисковый индекс (T14): новая нода — сразу в FTS
        self.search_service.command(SearchCommand::IndexFile {
            path: path.clone(),
            display_name: name,
        });
        // Каталог канваса мог не быть под слежкой (первая file-нода) —
        // синхронизируем вотчер и SHCNE-подписки (T17-E единая точка)
        self.sync_watch_dirs();
        tracing::info!(path = %path.display(), "создан текстовый файл + нода");
    }

    /// Штатный выход (T17): форс-сейв + восстановление системных иконок
    /// (R5) + завершение — единая точка для CloseRequested и пункта
    /// меню «Выход»; Drop-страховка guard'а остаётся на паниках, sentinel
    /// — на kill -9.
    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        if self.scene.dirty_since.is_some() {
            self.scene.save_now();
        }
        #[cfg(windows)]
        if let Some(guard) = self.icon_guard.as_mut() {
            guard.restore();
        }
        event_loop.exit();
    }

    fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self.scale_factor();
        let logical = [position.x as f32 / scale, position.y as f32 / scale];
        // Drag по миникарте (T13): пан следует за курсором — раньше
        // канвас-панорамирования, дрги не конкурируют (нажатие перехвачено)
        if self.minimap_drag {
            self.cursor = logical;
            self.center_camera_on_minimap_cursor();
            self.request_redraw();
            return;
        }
        if self.panning() {
            let delta = [logical[0] - self.cursor[0], logical[1] - self.cursor[1]];
            self.camera.pan(delta);
            self.request_redraw();
        }
        self.cursor = logical;
        // Драг внутри редактора — расширение выделения мышью (T7/T8)
        if self.editor_dragging && !self.space_pressed {
            let world = self.cursor_world();
            let zoom_px = self.zoom_px();
            if let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut())
            {
                if let Some((origin, _, _)) =
                    session_area(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
                {
                    let x = ((world[0] - origin[0]) * zoom_px) as i32;
                    let y = ((world[1] - origin[1]) * zoom_px) as i32;
                    session.drag(renderer.font_system_mut(), x, y);
                }
            }
            self.request_redraw();
        }
        if !self.space_pressed {
            // Рамка выделения (CR-001): тянется за курсором (перерисовка на
            // каждое движение — квад в оверлее); пан во время рамки —
            // Space недоступен (guard выше), средняя кнопка замораживает
            if self.select_rect.is_some() {
                let world = self.cursor_world();
                if let Some(rect) = self.select_rect.as_mut() {
                    rect.1 = world;
                }
                self.request_redraw();
            }
            // Ручной resize за правый нижний угол (T7): размеры клампятся
            // минимумом, spatial index обновляется инкрементально
            if let Some(index) = self.resizing {
                let world = self.cursor_world();
                if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                    node.width = (world[0] - node.x).max(MIN_NODE_WIDTH);
                    node.height = (world[1] - node.y).max(MIN_NODE_HEIGHT);
                    self.scene.spatial.update(index, node);
                    self.scene.mark_dirty();
                }
                self.request_redraw();
            } else if let Some(drag) = self.scene.dragging.clone() {
                // Drag (T7/CR-001): каждая перемещаемая нода — в исходную
                // позицию + дельта курсора от захвата (ровно один сдвиг за
                // кадр; дети групп — в origins с старта, дубликатов нет)
                let world = self.cursor_world();
                let delta = [world[0] - drag.grab_world[0], world[1] - drag.grab_world[1]];
                for (index, origin) in &drag.origins {
                    self.scene
                        .move_node(*index, origin[0] + delta[0], origin[1] + delta[1]);
                }
                // FR-012: зона втягивания — группа под центром первичной ноды
                let target = self.group_drop_target(&drag);
                if target != self.group_drop_target {
                    self.group_drop_target = target;
                }
                self.scene.mark_dirty();
                self.request_redraw();
            } else if self.edge_drag.is_some() {
                // Резиновая линия (T8) следует за курсором — курсор уже
                // обновлён выше, нужна только перерисовка
                self.request_redraw();
            } else if !self.panning() && !self.editor_dragging && self.editing.is_none() {
                // Hover (T8): порты ноды под курсором; перерисовка — только
                // при смене ноды, чтобы не крутить кадры на каждый пиксель.
                // Выборочный hit: над ребёнком группы hover уходит ему,
                // а не группе (порты групп не рисуются — cards.rs).
                // Модальность (практики UI): над открытым диалогом/поиском/
                // меню канваса hover-порты гасятся — сквозь оверлей
                // не подсвечивают
                let world = self.cursor_world();
                let hovered =
                    if self.dialog.is_some() || self.search.is_open() || self.menu.is_some() {
                        None
                    } else {
                        self.selective_hit(world)
                    };
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.request_redraw();
                } else if self.palette_target().is_some()
                    || self.menu.is_some()
                    || self.search.is_open()
                    || self.settings_open
                {
                    // Палитра/меню/поиск/настройки: hover-подсветка элементов
                    // следует за курсором
                    self.request_redraw();
                }
            }
        }
        // Аффорданс курсора (Grabbing/Text/NwseResize/Arrow) — после всех
        // смен состояний этого события
        self.sync_cursor_icon();
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // Колесо над screen-space UI (панели/меню/палитра/миникарта) холст
        // не двигает — практика canvas-приложений (Miro/Figma)
        if self.cursor_over_screen_surface() {
            return;
        }
        // Тачпады шлют PixelDelta (физические px), колёсики мышей — LineDelta
        let scale = self.scale_factor();
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * PAN_PX_PER_LINE, y * PAN_PX_PER_LINE),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / scale, pos.y as f32 / scale),
        };
        let viewport = self.viewport_logical();
        if self.modifiers.control_key() {
            // Ctrl+колесо — зум к позиции курсора (SPEC §8)
            let factor = match delta {
                MouseScrollDelta::LineDelta(_, y) => ZOOM_STEP_PER_LINE.powf(y),
                MouseScrollDelta::PixelDelta(pos) => (pos.y as f32 * 0.005).exp(),
            };
            self.camera.zoom_at(factor, self.cursor, viewport);
        } else {
            // Двухпальцевый скролл тачпада — панорамирование (SPEC §8)
            self.camera.pan([dx, dy]);
        }
        self.request_redraw();
    }

    fn on_pinch(&mut self, delta: f64) {
        // Пинч над screen-space UI — холст не зумит (как колесо выше)
        if self.cursor_over_screen_surface() {
            return;
        }
        let viewport = self.viewport_logical();
        let factor = (delta as f32).exp();
        self.camera.zoom_at(factor, self.cursor, viewport);
        self.request_redraw();
    }
}

/// Аргументы командной строки: `canvasdesk [--stress N] [path]`.
struct CliArgs {
    /// Нагрузочный режим (T5): сцена из N случайных нод вместо загрузки файла.
    stress: Option<usize>,
    /// M5 (T20): добавить N виджет-нод (встроенные часы) в открытую сцену —
    /// нагрузочная приёмка «10 виджетов не роняют fps» (SPEC §10 M5).
    stress_widgets: Option<usize>,
    /// Режим десктопа (T15, SPEC §7.4): встройка канваса в WorkerW за
    /// иконками рабочего стола. На не-Windows — warn и оконный режим.
    desktop: bool,
    path: PathBuf,
}

/// Разбор аргументов вручную — две опции не оправдывают зависимость от clap.
fn parse_args(args: &[String]) -> anyhow::Result<CliArgs> {
    let mut stress = None;
    let mut stress_widgets = None;
    let mut desktop = false;
    let mut path = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--stress" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("--stress требует число нод"))?;
            stress = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress: не число: {value}"))?,
            );
        } else if let Some(value) = arg.strip_prefix("--stress=") {
            stress = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress: не число: {value}"))?,
            );
        } else if arg == "--stress-widgets" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("--stress-widgets требует число виджетов"))?;
            stress_widgets = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress-widgets: не число: {value}"))?,
            );
        } else if let Some(value) = arg.strip_prefix("--stress-widgets=") {
            stress_widgets = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress-widgets: не число: {value}"))?,
            );
        } else if arg == "--desktop" {
            // Булев флаг: повтор допустим (идемпотентен)
            desktop = true;
        } else if arg == "--help" || arg == "-h" {
            println!(
                "Использование: canvasdesk [mcp [--no-spawn]] [--stress N] [--stress-widgets N] [--desktop] [путь к .canvas]\n\
                 \x20 mcp — режим MCP-посредника (stdio; автостарт сервиса, --no-spawn — отключить)\n\
                 \x20 --stress-widgets N — добавить N виджет-нод (нагрузочная приёмка M5)"
            );
            std::process::exit(0);
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            anyhow::bail!("лишний аргумент: {arg}");
        }
    }
    // В stress-режиме по умолчанию пишем в stress.canvas, чтобы не затирать default.canvas
    let default_path = if stress.is_some() {
        "stress.canvas"
    } else {
        "default.canvas"
    };
    Ok(CliArgs {
        stress,
        stress_widgets,
        desktop,
        path: path.unwrap_or_else(|| PathBuf::from(default_path)),
    })
}

/// Открыть файл/путь в системном приложении (T21-A: openFile моста,
/// permission shell:open). Windows — тот же ShellExecuteEx-путь, что у
/// файловых нод (interop::open_file); Linux/macOS — xdg-open/open
/// (M7: мост виджетов кроссплатформенен, host появится на T22+).
fn open_path_externally(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        canvas_shell::desktop::interop::open_file(path)
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program)
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{program}: {e}"))
    }
}

/// Детерминированный PRNG (xorshift32) — генератор стресс-сцены без зависимостей.
struct Xorshift(u32);

impl Xorshift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// Случайное f32 в [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

/// Слова для правдоподобных заголовков стресс-нод.
const STRESS_WORDS: [&str; 8] = [
    "отчёт",
    "смета",
    "презентация",
    "договор",
    "спецификация",
    "заметка",
    "план",
    "архив",
];

/// Нагрузочная сцена (T5): N текстовых нод со случайными rect/цветом/заголовком,
/// раскиданных по области, растущей как sqrt(N) — плотность стабильна.
/// Детерминирована: один и тот же N даёт одну и ту же сцену.
/// M5 (T20-F): добавить N виджет-нод (встроенные часы) детерминированной
/// сеткой — нагрузочная приёмка SPEC §10 («10 виджетов не роняют fps»).
/// Id — `widget-N` по порядку; возвращается число добавленных.
fn add_stress_widgets(canvas: &mut Canvas, n: usize) -> usize {
    let mut existing = 0u32;
    for node in &canvas.nodes {
        if let Some(tail) = node.id.strip_prefix("widget-") {
            if let Ok(k) = tail.parse::<u32>() {
                existing = existing.max(k);
            }
        }
    }
    let cols = 5;
    for i in 0..n {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        let ext = canvas_core::CanvasdeskExt {
            widget_id: Some("com.canvasdesk.clock".to_owned()),
            props: serde_json::Map::new(),
            expr: None,
            template: None,
        };
        canvas.nodes.push(Node::widget(
            format!("widget-{}", existing + i as u32 + 1),
            ext,
            "Clock",
            400.0 + col * 360.0,
            400.0 + row * 260.0,
            320.0,
            200.0,
        ));
    }
    n
}

fn stress_canvas(n: usize) -> Canvas {
    let mut canvas = Canvas::default();
    let mut rng = Xorshift(0x9E37_79B9);
    let extent = (n.max(1) as f32).sqrt() * 400.0;
    for i in 0..n {
        let x = rng.unit() * extent * 2.0 - extent;
        let y = rng.unit() * extent * 2.0 - extent;
        let width = 120.0 + rng.unit() * 300.0;
        let height = 80.0 + rng.unit() * 220.0;
        let word = STRESS_WORDS[i % STRESS_WORDS.len()];
        let mut node = Node::text(
            format!("stress-{i}"),
            format!("{word} #{i}\nнагрузочный тест"),
            x,
            y,
        );
        node.width = width;
        node.height = height;
        if rng.unit() < 0.3 {
            node.color = Some((1 + rng.next() % 6).to_string());
        }
        canvas.nodes.push(node);
    }
    canvas
}

fn main() -> anyhow::Result<()> {
    // FR-008: подкоманда `mcp` — режим MCP-посредника (stdio ↔ pipe) того
    // же бинарника: один exe на весь стек. Перехват ДО инициализации
    // трейсинга и parse_args: tracing пишет в stdout, а в mcp-режиме stdout
    // занят протоколом (run_stdio молчалив, диагностика — в stderr).
    // Файл с именем «mcp» открывается как ./mcp (путь с префиксом).
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("mcp") {
        return canvas_mcp::run_stdio(&argv[1..]);
    }
    // По умолчанию info, но без спама внутренних крейтов wgpu; переопределяется через RUST_LOG
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu_hal=warn,wgpu_core=warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
    // Версия сборки первой строкой лога: version (Cargo.toml) + git-коммит +
    // флаг «грязной» рабочей копии + профиль — по логу видно, какую именно
    // сборку запустили (env от build.rs; без git — «unknown»)
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = option_env!("CANVASDESK_GIT_COMMIT").unwrap_or("unknown"),
        dirty = option_env!("CANVASDESK_GIT_DIRTY").unwrap_or("?"),
        profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        "CanvasDesk запускается"
    );
    // T17 (R5 + краш-сейф): sentinel от прошлой аварийной сессии →
    // форс-восстановление иконок ДО всего остального, независимо от
    // режима запуска (TASKS T17: «kill -9 → следующий запуск
    // восстанавливает»; kill обходит Drop-страховку guard'а)
    #[cfg(windows)]
    if canvas_shell::desktop::icons::crash_recovery() {
        tracing::info!("иконки десктопа восстановлены после аварийной сессии (sentinel)");
    }
    let args = parse_args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    // T15-relaunch: single-instance handoff ДО загрузки сцены/конфига —
    // повторный запуск (в т.ч. перезапуск на --desktop из меню канваса)
    // сигналит работающему инстансу штатный выход и ждёт смерти предыдущего
    // владельца мьютекса. Пока ждём — сцена не читается: старый инстанс
    // успеет сохранить dirty-сцену без гонки записи/чтения default.canvas.
    #[cfg(windows)]
    let _instance_guard = {
        use canvas_shell::desktop::single_instance;
        if single_instance::signal_exit() {
            tracing::info!("работающий инстанс получил сигнал завершения — ждём его выхода");
        }
        match single_instance::InstanceGuard::acquire(single_instance::SINGLE_INSTANCE_WAIT_MS) {
            Some(guard) => Some(guard),
            None => {
                tracing::warn!(
                    wait_ms = single_instance::SINGLE_INSTANCE_WAIT_MS,
                    "предыдущий инстанс не завершился вовремя — запускаемся вторым (деградация R14)"
                );
                None
            }
        }
    };
    // Настройки приложения (~/.canvasdesk/config.toml); битый/отсутствующий
    // файл — дефолты + warn, приложение не падает
    let config_path = canvas_shell::default_config_path();
    let (settings, config_warn) = match &config_path {
        Some(path) => Settings::load(path),
        None => (Settings::default(), None),
    };
    if let Some(warn) = config_warn {
        tracing::warn!(%warn, "конфиг не применён, дефолты");
    }
    let scene = match args.stress {
        Some(n) => {
            tracing::info!(nodes = n, path = %args.path.display(), "нагрузочный режим --stress");
            let canvas = stress_canvas(n);
            if let Err(err) = canvas.save(&args.path) {
                tracing::warn!(%err, "не удалось сохранить стресс-сцену");
            }
            SceneState::new(canvas, args.path)
        }
        None => SceneState::load_or_seed(args.path),
    };
    // M5: --stress-widgets N — детерминированная сетка виджет-нод (часы)
    let mut scene = scene;
    if let Some(k) = args.stress_widgets {
        let added = add_stress_widgets(&mut scene.canvas, k);
        tracing::info!(widgets = added, "нагрузочные виджеты добавлены");
        scene.mark_dirty();
    }
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
    // Пул тамбнейлов (T6): провайдер Windows + SQLite-кэш; worker'ы будят
    // event loop через proxy — иначе при ControlFlow::Wait результаты
    // лежали бы в канале до следующего ввода
    let proxy: EventLoopProxy<AppEvent> = event_loop.create_proxy();
    // Exit-листенер single-instance (T15-relaunch): новый запуск (в т.ч.
    // перезапуск на --desktop из меню канваса) сигналит событие — поток будит
    // event loop через AppEvent::InstanceExit, приложение штатно сохраняется
    // и выходит, освобождая мьютекс для нового инстанса. Провал — warn:
    // повторные запуски не закроют этот инстанс сигналом (деградация R14).
    #[cfg(windows)]
    {
        let proxy = proxy.clone();
        if let Err(err) = canvas_shell::desktop::single_instance::spawn_exit_listener(move || {
            let _ = proxy.send_event(AppEvent::InstanceExit);
        }) {
            tracing::warn!(%err, "exit-листенер не запущен — повторный запуск не закроет этот инстанс");
        }
    }
    // Отправитель drag-событий в event loop (T9): тот же паттерн, что и
    // ThumbService-вокер — IDropTarget (shell) шлёт AppEvent::Drag через proxy
    let drag_sender: Arc<dyn Fn(canvas_shell::dragdrop::DragEvent) + Send + Sync> = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Drag(event));
        })
    };
    // M5 (T20-F): события host'а виджетов (WidgetEvent) — тем же паттерном;
    // прокси берём ЗДЕСЬ, у EventLoop: у ActiveEventLoop, доступного в
    // resumed(), нет create_proxy (winit 0.30) — локальный Linux-чек этого
    // не видит, виндовую компиляцию ловит только CI (урок a9488ae)
    let widget_sender: canvas_widgets::WidgetEventSender = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Widget(event));
        })
    };
    // Файловый вотчер (T10): агрегатор shell шлёт батчи FileEvent через proxy;
    // первичный набор директорий — сразу после загрузки сцены, дальше —
    // sync_watch_dirs по событиям модели (дроп/удаление/rename)
    let file_sender: canvas_shell::FileEventSender = {
        let proxy = proxy.clone();
        Arc::new(move |events| {
            let _ = proxy.send_event(AppEvent::FileEvents(events));
        })
    };
    let mut watcher = WatchService::new(file_sender);
    watcher.sync_dirs(&watched_dirs(&scene.canvas, &scene.canvas_dir()));
    #[cfg(windows)]
    let provider: Arc<dyn ThumbnailProvider + Send + Sync> =
        Arc::new(canvas_shell::ShellThumbnailProvider);
    #[cfg(not(windows))]
    let provider: Arc<dyn ThumbnailProvider + Send + Sync> =
        Arc::new(canvas_shell::NoopThumbnailProvider);
    let cache = canvas_shell::default_cache_dir().and_then(|dir| {
        match canvas_shell::ThumbCache::open(&dir) {
            Ok(cache) => Some(cache),
            Err(err) => {
                tracing::warn!(%err, "тамбнейл-кэш недоступен, работаем без него");
                None
            }
        }
    });
    let thumbs = ThumbService::new(provider, cache, {
        let proxy = proxy.clone();
        Some(Arc::new(move || {
            let _ = proxy.send_event(AppEvent::ThumbsReady);
        }))
    });
    // Поисковый индекс (T14): worker-поток FTS5 в общем cache.db; ответы —
    // AppEvent::Search через proxy (паттерн ThumbService/Watcher). Ошибка
    // открытия БД — деградация: warn внутри, пустые результаты (SPEC §5.3)
    let search_responder: canvas_shell::SearchResponder = {
        let proxy = proxy.clone();
        Arc::new(move |event| {
            let _ = proxy.send_event(AppEvent::Search(event));
        })
    };
    let search_cache_dir = canvas_shell::default_cache_dir().unwrap_or_else(|| PathBuf::from("."));
    let search_service = SearchService::spawn(search_cache_dir, search_responder);
    // Первичная индексация file-нод загруженного канваса (T14): полный
    // пересбор таблицы, лишние записи удаляются (ReplaceAll)
    {
        let canvas_dir = scene.canvas_dir();
        let entries: Vec<canvas_shell::IndexEntry> = scene
            .canvas
            .nodes
            .iter()
            .filter_map(|node| {
                let file = node.file.as_ref()?;
                Some(canvas_shell::IndexEntry {
                    path: resolve_node_path(file, &canvas_dir),
                    display_name: Path::new(file)
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file.clone()),
                })
            })
            .collect();
        search_service.command(SearchCommand::ReplaceAll { entries });
    }
    event_loop.run_app(&mut {
        let mut app = App::new(
            scene,
            thumbs,
            settings,
            config_path,
            drag_sender,
            widget_sender,
            watcher,
            search_service,
            args.desktop,
        );
        // M5 (T20-F): реестр виджетов (материализация встроенных + скан)
        app.init_widgets();
        // M5: тик-поток host'а (1 c) — будит цикл для refresh-снапшотов
        // (LOD-расписание считает менеджер по времени, тик — только побудка;
        // паттерн — сервисы T15/T16, sender через EventLoopProxy)
        {
            let proxy = proxy.clone();
            std::thread::Builder::new()
                .name("widget-tick".to_owned())
                .spawn(move || loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let _ = proxy.send_event(AppEvent::Widget(canvas_widgets::WidgetEvent::Tick));
                })
                .ok();
        }
        // Shell-монитор десктопа (T15): поток WinEventHook + DPI-поллинг;
        // слежка (Watch) устанавливается в resumed() после attach.
        // Спавним при --desktop до attach — событие WorkerWDestroyed может
        // прийти раньше, чем приложение дойдёт до recovery-логики
        #[cfg(windows)]
        if args.desktop {
            let proxy = proxy.clone();
            let responder: canvas_shell::desktop::monitor::DesktopResponder =
                Arc::new(move |event| {
                    let _ = proxy.send_event(AppEvent::Desktop(event));
                });
            app.set_desktop_monitor(
                canvas_shell::desktop::monitor::DesktopMonitorService::spawn(responder),
            );
        }
        // Шина системных событий (T16): message-only окно на отдельном
        // потоке; спавн БЕЗ привязки к --desktop — события сессии/сна/
        // shell-файлов полезны в любом режиме (план §8.7); провал — warn
        // внутри spawn + деградация (R14). Паттерн спавна — T15-монитор.
        #[cfg(windows)]
        {
            let proxy = proxy.clone();
            let responder: canvas_shell::shell_events::window::ShellResponder =
                Arc::new(move |event| {
                    let _ = proxy.send_event(AppEvent::Shell(event));
                });
            app.set_shell_events(
                canvas_shell::shell_events::window::ShellEventService::spawn(responder),
            );
            // Первичный набор SHChangeNotify-подписок — через единую точку
            // sync_watch_dirs (вотчер уже синхронизирован в main() выше —
            // дифф-синк идемпотентен; команды лягут в канал шины и дрени-
            // руются по WM_APP_WAKE после создания окна потоком)
            app.sync_watch_dirs();
        }
        // MCP named pipe (MCP-интеграция): worker-поток \\.\pipe\canvasdesk
        // принимает JSON-RPC от canvas-mcp-посредника; waker — тот же паттерн,
        // что ThumbService (worker будит event loop через proxy). Провал spawn —
        // warn внутри + None: MCP недоступен, приложение работает как обычно.
        #[cfg(windows)]
        {
            let proxy = proxy.clone();
            let mcp_waker: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let _ = proxy.send_event(AppEvent::McpWake);
            });
            app.set_mcp_server(canvas_shell::mcp_pipe::McpPipeServer::spawn(
                canvas_mcp::PIPE_NAME,
                mcp_waker,
            ));
        }
        // Не-Windows: режим десктопа недоступен — предупреждение и обычный
        // оконный режим (деградация, SPEC §9; ядро приложения то же)
        #[cfg(not(windows))]
        if args.desktop {
            tracing::warn!("--desktop поддерживается только на Windows — оконный режим");
        }
        app
    })?;
    Ok(())
}

impl App {
    /// M5 (T20-F): стартовая инициализация виджетов (реестр + встроенные).
    /// Отдельно от App::new — после настройки трейсинга в main().
    fn init_widgets(&mut self) {
        self.widgets.set_theme(self.settings.theme == Theme::Dark);
        self.widgets.init_registry();
    }

    /// M5: события host'а виджетов (из user_event).
    fn on_widget_event(&mut self, event: canvas_widgets::WidgetEvent) {
        self.widgets.on_event(&event);
        match event {
            canvas_widgets::WidgetEvent::SnapshotReady { node_id, snapshot } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_widget_snapshot(
                        &node_id,
                        snapshot.width,
                        snapshot.height,
                        &snapshot.rgba,
                    );
                }
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::Message {
                node_id,
                message,
                id,
            } => {
                self.on_widget_message(&node_id, message, id);
            }
            canvas_widgets::WidgetEvent::EnvironmentReady { ok } => {
                tracing::info!(ok, "виджеты: {}", self.widgets.runtime_status());
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::ControllerReady { .. } => {
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::Tick => {
                // refresh-расписание вычисляется в update_frame по времени;
                // тик только будит цикл
            }
        }
    }

    /// M5 (T21-A): сообщения моста — enforcement на каждый вызов.
    /// Разрешения берутся из манифеста ПАКЕТА ноды (не из сообщения!),
    /// отказ — warn + JSON-RPC error (для запросов с id). `id`
    /// передаётся из host'а для ответа на запросы readDir/state*.
    fn on_widget_message(
        &mut self,
        node_id: &str,
        message: canvas_widgets::WidgetToHost,
        id: Option<serde_json::Value>,
    ) {
        use canvas_widgets::WidgetToHost;
        // Enforcement (П-таблица §4.6): без permission — отказ + лог.
        // Пакет мог исчезнуть (удалён) — тоже отказ, не паника.
        let permissions = match self
            .widgets
            .permissions_of_node(&self.scene.canvas, node_id)
        {
            Some(p) => p,
            None => {
                tracing::warn!(
                    node_id,
                    msg = message.method(),
                    "мост: пакет ноды не установлен"
                );
                self.reply_err(node_id, id, "пакет виджета не установлен");
                return;
            }
        };
        if let Err(required) = permissions.check_call(&message) {
            tracing::warn!(
                node_id,
                method = message.method(),
                required = required.as_str(),
                "мост: вызов заблокирован — нет permission"
            );
            self.reply_err(
                node_id,
                id,
                format!("нет permission: {}", required.as_str()),
            );
            return;
        }
        match message {
            WidgetToHost::Ready => {
                let init = self
                    .scene
                    .canvas
                    .node(node_id)
                    .and_then(|node| self.widgets.init_message(node));
                if let Some(init) = init {
                    self.widgets.post_message(node_id, &init);
                }
            }
            WidgetToHost::SetProps { props } => {
                // Undo-шаг до мутации (коалесценция — риски M5 §8)
                if self.widgets.should_push_props_undo(node_id) {
                    self.push_undo();
                }
                let index = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .position(|node| node.id == node_id);
                let changed = index
                    .and_then(|i| self.scene.canvas.nodes[i].canvasdesk.as_mut())
                    .map(|ext| {
                        let changed = ext.props != props;
                        ext.props = props.clone();
                        changed
                    })
                    .unwrap_or(false);
                if changed {
                    self.scene.mark_dirty();
                    // Подтверждение виджету (propsChanged) — замкнутый цикл
                    // без эхо-повтора: повторный setProps тех же props не
                    // меняет модель (changed=false).
                    self.widgets.post_message(
                        node_id,
                        &canvas_widgets::HostToWidget::PropsChanged { props },
                    );
                }
            }
            WidgetToHost::OpenFile { path } => {
                // Путь — как дала нода/канвас: резолв от корня канваса,
                // произвольные системные пути виджету недоступны (П4-дух).
                let resolved = self.scene.canvas_dir().join(path.trim_end_matches('/'));
                if let Err(e) = open_path_externally(&resolved) {
                    tracing::warn!(node_id, path = %resolved.display(), error = %e, "openFile не удался");
                    self.show_toast(format!("Виджет: не удалось открыть {}", resolved.display()));
                }
            }
            WidgetToHost::ReadDir { path } => {
                // Allowlist П4: папки файловых нод + корень канваса
                let canvas_dir = self.scene.canvas_dir();
                let roots = canvas_core::watched_dirs(&self.scene.canvas, &canvas_dir);
                match canvas_widgets::permissions::resolve_fs_request(&path, &canvas_dir, &roots)
                    .and_then(|dir| canvas_widgets::bridge::read_dir_entries(&dir))
                {
                    Ok(entries) => self.widgets.reply(
                        node_id,
                        &canvas_widgets::bridge::Reply::ok(
                            id.clone().unwrap_or(serde_json::Value::Null),
                            entries,
                        ),
                    ),
                    Err(e) => {
                        tracing::warn!(node_id, path, error = %e, "readDir отказан");
                        self.reply_err(node_id, id, e);
                    }
                }
            }
            WidgetToHost::Toast { text } => {
                tracing::info!(node_id, %text, "widget toast");
                self.show_toast(text);
            }
            WidgetToHost::Resize { w, h } => {
                // Кламп манифестных границ (160..2000, SPEC §7.6)
                let w = w.clamp(160.0, 2000.0);
                let h = h.clamp(160.0, 2000.0);
                let index = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .position(|node| node.id == node_id);
                if let Some(node) = index.map(|i| &mut self.scene.canvas.nodes[i]) {
                    if node.width != w || node.height != h {
                        node.width = w;
                        node.height = h;
                        // Геометрия изменилась — обновляем пространственный индекс
                        // пересборкой (как при ручном resize)
                        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                        self.scene.mark_dirty();
                    }
                }
            }
            WidgetToHost::StateGet { key } => {
                let value = self.widgets.state_get(node_id, &key);
                let result = serde_json::json!({ "value": value });
                self.widgets.reply(
                    node_id,
                    &canvas_widgets::bridge::Reply::ok(
                        id.clone().unwrap_or(serde_json::Value::Null),
                        result,
                    ),
                );
            }
            WidgetToHost::StateSet { key, value } => {
                self.widgets.state_set(node_id, &key, &value);
                self.widgets.reply(
                    node_id,
                    &canvas_widgets::bridge::Reply::ok(
                        id.clone().unwrap_or(serde_json::Value::Null),
                        serde_json::json!({ "ok": true }),
                    ),
                );
            }
        }
    }

    /// Ответ-ошибка на запрос моста (T21-A): уведомления без id — только warn.
    fn reply_err(
        &mut self,
        node_id: &str,
        id: Option<serde_json::Value>,
        message: impl Into<String>,
    ) {
        if let Some(id) = id {
            self.widgets
                .reply(node_id, &canvas_widgets::bridge::Reply::err(id, message));
        }
    }

    /// Toast (T21-A): строка внизу центра на 3 с + перерисовка.
    fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
        self.request_redraw();
    }

    /// Rect модального диалога (screen-space, логические px): центр окна.
    fn dialog_rect(&self) -> [f32; 4] {
        let viewport = self.viewport_logical();
        let w = 440.0_f32.min(viewport[0] - 40.0).max(280.0);
        let h = 150.0;
        [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h]
    }

    /// Rect кнопок диалога: [Да][Нет] внизу панели (индексы как в buttons()).
    fn dialog_button_rects(&self) -> [[f32; 4]; 2] {
        let [x, y, w, h] = self.dialog_rect();
        let bw = 110.0;
        let bh = 30.0;
        let gap = 16.0;
        let total = bw * 2.0 + gap;
        let start = x + (w - total) / 2.0;
        let by = y + h - bh - 16.0;
        [[start, by, bw, bh], [start + bw + gap, by, bw, bh]]
    }

    /// Подтверждение диалога (Enter/клик «Да»): установка или удаление.
    fn confirm_dialog(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            AppDialog::InstallWidget {
                src,
                manifest,
                pos,
                updating,
            } => {
                match self.widgets.install_package(&src) {
                    Ok(canvas_widgets::registry::InstallOutcome::Installed) => {
                        self.show_toast(format!("Виджет {} установлен", manifest.name));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::Updated) => {
                        self.show_toast(format!(
                            "Виджет {} обновлён до {}",
                            manifest.name, manifest.version
                        ));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::SameVersion) => {
                        self.show_toast(format!("Виджет {} уже в этой версии", manifest.name));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "установка виджета не удалась");
                        self.show_toast(format!("Установка не удалась: {e}"));
                        self.request_redraw();
                        return;
                    }
                }
                // Нода в точке дропа (SPEC §10: «виджет ставится на канвас»);
                // при обновлении — не дублируем (П5: props/ноды сохраняются)
                if !updating {
                    self.push_undo();
                    let id = self.widgets.next_node_id(&self.scene.canvas);
                    let node = self.widgets.build_widget_node(
                        &manifest.id,
                        id,
                        [pos[0] + 40.0, pos[1] + 30.0],
                    );
                    if let Some(node) = node {
                        self.scene.canvas.nodes.push(node);
                        let index = self.scene.canvas.nodes.len() - 1;
                        let node_ref = &self.scene.canvas.nodes[index];
                        self.scene.spatial.insert(index, node_ref);
                        self.scene.selected = Some(Selection::Node(index));
                        self.scene.mark_dirty();
                    }
                }
                self.request_redraw();
            }
            AppDialog::RemovePackage { widget_id, name } => {
                match self.widgets.remove_package(&widget_id) {
                    Ok(()) => {
                        self.show_toast(format!("Пакет {name} удалён"));
                        // Ноды пакета остаются (деградируют в заглушки —
                        // package_ok=false в LOD); пересборка spatial не нужна
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "удаление пакета не удалось");
                        self.show_toast(format!("Удаление не удалось: {e}"));
                    }
                }
                self.request_redraw();
            }
            // FR-014: подтверждение цикла — ребро создаётся как
            // контрольная связь (без потока значений)
            AppDialog::EdgeCycle {
                from_node,
                from_side,
                to_node,
                to_side,
            } => {
                self.create_edge(from_node, from_side, to_node, to_side, FlowKind::Control);
            }
        }
    }

    /// Отмена диалога (Esc/клик «Нет»): ничего не меняется.
    fn cancel_dialog(&mut self) {
        self.dialog = None;
        self.request_redraw();
    }

    /// FR-014: создать связь заданного типа потока (общий путь drop
    /// резиновой линии и подтверждения диалога цикла). Undo-шаг (FR-006),
    /// mark_dirty + живой пересчёт потока: value-ребро сразу переносит
    /// значение в downstream.
    fn create_edge(
        &mut self,
        from_node: String,
        from_side: Side,
        to_node: String,
        to_side: Side,
        kind: FlowKind,
    ) {
        let mut edge = Edge::new(
            self.scene.canvas.next_edge_id(),
            from_node,
            Some(from_side),
            to_node,
            Some(to_side),
        );
        edge.set_flow_kind(kind);
        self.push_undo();
        self.scene.canvas.add_edge(edge);
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        self.request_redraw();
    }

    /// M5: установка виджет-ноды из подменю (центр viewport, defaultSize).
    fn insert_widget_from_menu(&mut self, widget_id: &str) {
        let center = self.viewport_center_world();
        let id = self.widgets.next_node_id(&self.scene.canvas);
        let Some(node) = self.widgets.build_widget_node(widget_id, id, center) else {
            tracing::warn!(widget_id, "пакет виджета не найден");
            return;
        };
        self.insert_nodes(vec![node], true);
    }

    // --- FR-010: авто-раскладка связанных карточек ---

    /// Применить план авто-раскладки (FR-010) от ноды-семени. Один undo-шаг
    /// (FR-006); ноды без связи с семенем не трогаются; spatial index
    /// обновляется точечно (паттерн drag группы).
    fn apply_related_layout(&mut self, seed: usize, mode: canvas_core::LayoutMode) {
        let plan = canvas_core::plan_related_layout(&self.scene.canvas, seed, mode);
        if plan.is_empty() {
            self.show_toast("Нет связанных карточек для раскладки");
            return;
        }
        // FR-006: раскладка — один undo-шаг
        self.push_undo();
        for (index, [x, y]) in plan {
            if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                node.x = x;
                node.y = y;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                self.scene.spatial.update(index, node);
            }
        }
        self.scene.mark_dirty();
        self.show_toast("Связанные карточки выровнены");
    }

    // --- FR-011: mindmap (Tab / Enter / сворачивание ветки) ---

    /// Прямые дети ноды по исходящим рёбрам (в порядке рёбер модели).
    fn mindmap_direct_children(canvas: &Canvas, parent_index: usize) -> Vec<usize> {
        let Some(parent) = canvas.nodes.get(parent_index) else {
            return Vec::new();
        };
        let parent_id = parent.id.as_str();
        let index_of: std::collections::HashMap<&str, usize> = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        canvas
            .edges
            .iter()
            .filter(|edge| edge.from_node == parent_id)
            .filter_map(|edge| index_of.get(edge.to_node.as_str()).copied())
            .filter(|&i| i != parent_index)
            .collect()
    }

    /// Создать дочернюю ветку (FR-011, Tab): text-нода правее родителя
    /// (под существующими детьми) + ребро родитель→новая + вход в
    /// редактирование. Один undo-шаг (нода + ребро + разворот свёрнутого).
    fn mindmap_add_child(&mut self, parent_index: usize) {
        let canvas = &self.scene.canvas;
        let Some(parent) = canvas.nodes.get(parent_index) else {
            return;
        };
        let parent_id = parent.id.clone();
        let x = parent.x + parent.width + canvas_core::LEVEL_GAP;
        let mut y_bottom: Option<f32> = None;
        for child in Self::mindmap_direct_children(canvas, parent_index) {
            if let Some(node) = canvas.nodes.get(child) {
                y_bottom =
                    Some(y_bottom.map_or(node.y + node.height, |b| b.max(node.y + node.height)));
            }
        }
        let y = match y_bottom {
            Some(bottom) => bottom + canvas_core::SIBLING_GAP,
            None => parent.y,
        };
        // Один undo-шаг на всю операцию (нода + ребро + возможный разворот)
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "note");
        self.scene.canvas.nodes.push(Node::text(id, "", x, y));
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        let edge = Edge::new(
            self.scene.canvas.next_edge_id(),
            parent_id,
            Some(Side::Right),
            self.scene.canvas.nodes[index].id.clone(),
            Some(Side::Left),
        );
        self.scene.canvas.add_edge(edge);
        // Свёрнутая ветка разворачивается: новая нода должна быть видна
        if let Some(parent) = self.scene.canvas.nodes.get_mut(parent_index) {
            if parent.collapsed == Some(true) {
                parent.collapsed = None;
            }
        }
        self.scene.mark_dirty();
        self.begin_editing(index);
    }

    /// Создать сиблинга (FR-011, Enter): та же родительская нода, что у
    /// текущей. У корня (нет входящих рёбер) — no-op (зафиксировано в
    /// FR-011). Один undo-шаг.
    fn mindmap_add_sibling(&mut self, node_index: usize) {
        let Some(parent) = canvas_core::parent_index(&self.scene.canvas, node_index) else {
            self.show_toast("У корневой ветки нет уровня — используйте Tab");
            return;
        };
        self.mindmap_add_child(parent);
    }

    /// Свернуть/развернуть ветку (FR-011): флаг `collapsed` ноды; поддерево
    /// скрывается из рендера/hit-test/миникарты/поиска (см.
    /// `hidden_subtree_nodes`). Undo-шаг — как изменение модели.
    fn mindmap_set_collapsed(&mut self, node_index: usize, collapsed: bool) {
        let Some(node) = self.scene.canvas.nodes.get(node_index) else {
            return;
        };
        if node.kind() != NodeKind::Text
            || node.collapsed == Some(collapsed)
            || canvas_core::subtree_ids(&self.scene.canvas, node_index).is_empty()
        {
            return; // нет детей — сворачивать нечего
        }
        self.push_undo();
        if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
            node.collapsed = Some(collapsed);
        }
        self.scene.mark_dirty();
        self.show_toast(if collapsed {
            "Ветка свёрнута"
        } else {
            "Ветка развёрнута"
        });
    }

    /// Индексы скрытых нод (свернутые поддеревья, FR-011): объединение
    /// поддеревьев всех нод с collapsed = Some(true); отсортирован —
    /// binary_search в горячих путях.
    fn hidden_subtree_nodes(&self) -> Vec<usize> {
        let mut hidden: Vec<usize> = Vec::new();
        for (index, node) in self.scene.canvas.nodes.iter().enumerate() {
            if node.collapsed == Some(true) {
                for id in canvas_core::subtree_ids(&self.scene.canvas, index) {
                    if !hidden.contains(&id) {
                        hidden.push(id);
                    }
                }
            }
        }
        hidden.sort_unstable();
        hidden.dedup();
        hidden
    }

    // --- FR-009: диспетчер «Настройки ▸» ---

    /// Применить настройку/действие ноды из палитры выделения
    /// (FR-009; диспетчер для `PaletteAction::Node`). Каждая мутирующая
    /// настройка — «push_undo → мутация → mark_dirty»; переименование
    /// входит в редактирование (его undo — commit сессии).
    fn apply_node_setting(&mut self, node_index: usize, setting: canvas_app::ui::NodeSetting) {
        use canvas_app::ui::NodeSetting;
        match setting {
            // FR-009: переименовать = вход в редактирование (двойной клик)
            NodeSetting::Rename => self.begin_editing(node_index),
            NodeSetting::Duplicate => {
                // Дублирование одной ноды (паттерн duplicate_selection)
                let Some(node) = self.scene.canvas.nodes.get(node_index).cloned() else {
                    return;
                };
                let copies = reassign_ids(&self.scene.canvas, &[node]);
                let nodes = paste_nodes(
                    &copies,
                    PastePlacement::Offset([DUPLICATE_OFFSET, DUPLICATE_OFFSET]),
                );
                self.insert_nodes(nodes, true);
            }
            // FR-011: mindmap из меню
            NodeSetting::AddChild => self.mindmap_add_child(node_index),
            NodeSetting::AddSibling => self.mindmap_add_sibling(node_index),
            NodeSetting::CollapseBranch => self.mindmap_set_collapsed(node_index, true),
            NodeSetting::ExpandBranch => self.mindmap_set_collapsed(node_index, false),
            // FR-009: файловые операции (открытие — Windows, SPEC §7.4)
            NodeSetting::OpenFile => {
                #[cfg(windows)]
                {
                    let path = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()));
                    if let Some(path) = path {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                            tracing::warn!(%err, path = %path.display(), "не удалось открыть файл");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::OpenFolder => {
                // Папка файла через ShellExecuteEx на директорию (Win)
                #[cfg(windows)]
                {
                    let dir = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()))
                        .and_then(|path| path.parent().map(|p| p.to_path_buf()));
                    if let Some(dir) = dir {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&dir) {
                            tracing::warn!(%err, dir = %dir.display(), "не удалось открыть папку");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::CopyPath => {
                let text = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .and_then(|node| node.file.clone().or_else(|| node.text.clone()))
                    .unwrap_or_default();
                if !text.is_empty() {
                    self.clipboard.set(text);
                    self.show_toast("Путь скопирован");
                }
            }
            NodeSetting::ClearText => {
                // FR-006: очистка текста — undo-шаг (no-op на пустой — без шага)
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.text = Some(String::new());
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            NodeSetting::Ungroup => {
                // Разгруппировать = удалить группу-ноду без каскада по детям
                // (removing_group_keeps_children); дети остаются на местах
                let is_group = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .is_some_and(|node| node.kind() == NodeKind::Group);
                if !is_group {
                    return;
                }
                self.push_undo();
                self.scene.canvas.remove_node(node_index);
                // Индексы сдвинулись — spatial/кэши перестраиваются
                // (паттерн delete_selected)
                self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                self.scene.selected = None;
                self.scene.selected_nodes.clear();
                self.scene.mark_dirty();
                self.show_toast("Группа разгруппирована");
            }
            NodeSetting::WidgetReload => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                if let Some(node_id) = node_id {
                    self.widgets.reload_widget(&node_id);
                    self.show_toast("Виджет перезагружается");
                }
            }
            NodeSetting::WidgetPermissions => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                let summary = node_id
                    .as_deref()
                    .and_then(|id| self.widgets.permissions_of_node(&self.scene.canvas, id))
                    .map(|permissions| {
                        let list: Vec<&str> =
                            permissions.list().iter().map(|p| p.as_str()).collect();
                        if list.is_empty() {
                            "нет особых разрешений".to_owned()
                        } else {
                            list.join(", ")
                        }
                    })
                    .unwrap_or_else(|| "пакет не установлен".to_owned());
                self.show_toast(format!("Разрешения: {summary}"));
            }
        }
    }

    // --- FR-012: жест «втягивания» в группу ---

    /// Цель втягивания при активном drag: верхняя группа (макс. индекс),
    /// чей rect содержит центр перетаскиваемой первичной ноды, при условии,
    /// что нода ещё НЕ ребёнок этой группы (иначе жест бессмысленен).
    fn group_drop_target(&self, dragging: &DragState) -> Option<usize> {
        let node = self.scene.canvas.nodes.get(dragging.primary)?;
        let center = [node.x + node.width / 2.0, node.y + node.height / 2.0];
        let dragged: Vec<usize> = std::iter::once(dragging.primary)
            .chain(dragging.origins.iter().map(|(i, _)| *i))
            .collect();
        let candidates = self
            .scene
            .spatial
            .query_rect([center[0], center[1], center[0], center[1]]);
        candidates
            .into_iter()
            .rev() // верхняя по z — последняя
            .find(|&index| {
                self.scene.canvas.nodes.get(index).is_some_and(|group| {
                    group.kind() == NodeKind::Group
                        && !dragged.contains(&index)
                        && center[0] >= group.x
                        && center[0] <= group.x + group.width
                        && center[1] >= group.y
                        && center[1] <= group.y + group.height
                        // уже ребёнок (явный список) — не «втягиваем» повторно
                        && !group
                            .children
                            .as_ref()
                            .is_some_and(|list| {
                                self.scene.canvas.nodes.get(dragging.primary).is_some_and(|n| {
                                    list.contains(&n.id)
                                })
                            })
                })
            })
    }

    /// Вставить перетаскиваемые ноды в группу (FR-012, отпускание над
    /// зоной): membership + авторасширение rect до bbox+padding + мягкое
    /// раздвигание пересекаемых соседей (с анимацией). Один undo-шаг.
    fn group_insert_dragged(&mut self, group_index: usize) {
        let Some(drag) = self.scene.dragging.as_ref() else {
            return;
        };
        // Вставляются: первичная нода + весь drag-набор (CR-001), кроме
        // самой группы-цели. Группа в наборе — вставляется ТОЛЬКО она
        // (вложенная группа едет как нода; её дети — через translate_group)
        let primary_is_group = self
            .scene
            .canvas
            .nodes
            .get(drag.primary)
            .is_some_and(|n| n.kind() == NodeKind::Group);
        let mut ids: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let dragged: Vec<usize> = if primary_is_group {
            vec![drag.primary]
        } else {
            std::iter::once(drag.primary)
                .chain(drag.origins.iter().map(|(i, _)| *i))
                .collect()
        };
        for index in dragged {
            if index == group_index || !seen.insert(index) {
                continue;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                ids.push(node.id.clone());
            }
        }
        if ids.is_empty() {
            return;
        }
        self.push_undo();
        canvas_core::group_add_children(&mut self.scene.canvas, group_index, &ids);
        // Авторасширение: rect группы = bbox(дети) + GROUP_PADDING
        let old = self
            .scene
            .canvas
            .nodes
            .get(group_index)
            .map(|g| [g.x, g.y])
            .unwrap_or([0.0, 0.0]);
        canvas_core::group_expand_to_children(
            &mut self.scene.canvas,
            group_index,
            canvas_app::ui::GROUP_PADDING,
        );
        let new_rect = self
            .scene
            .canvas
            .nodes
            .get(group_index)
            .map(|g| [g.x, g.y, g.width, g.height])
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);
        self.scene
            .spatial
            .update(group_index, &self.scene.canvas.nodes[group_index]);
        // Мягкое раздвигание: не-дети, чьи bbox пересеклись с новым rect,
        // сдвигаются на минимальный осевой вектор; группа и раздвинутые
        // соседи едут плавно (settle-анимация ~250 мс)
        let children: Vec<usize> = canvas_core::group_children(&self.scene.canvas, group_index);
        let others: Vec<(usize, [f32; 4])> = self
            .scene
            .canvas
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, node)| {
                *i != group_index && !children.contains(i) && node.kind() != NodeKind::Group
            })
            .map(|(i, node)| (i, [node.x, node.y, node.width, node.height]))
            .collect();
        let push_plan = canvas_core::plan_push_out(new_rect, &others);
        let mut moves: Vec<(usize, [f32; 2], [f32; 2])> = Vec::new();
        let new_pos = [new_rect[0], new_rect[1]];
        if (new_pos[0] - old[0]).abs() > f32::EPSILON || (new_pos[1] - old[1]).abs() > f32::EPSILON
        {
            moves.push((group_index, old, new_pos));
        }
        for (index, [dx, dy]) in push_plan {
            let (Some(from), Some(to_target)) = (
                self.scene.canvas.nodes.get(index).map(|n| [n.x, n.y]),
                self.scene
                    .canvas
                    .nodes
                    .get(index)
                    .map(|n| [n.x + dx, n.y + dy]),
            ) else {
                continue;
            };
            if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                node.x += dx;
                node.y += dy;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                self.scene.spatial.update(index, node);
            }
            moves.push((index, from, to_target));
        }
        if !moves.is_empty() {
            self.settle_anim = Some(SettleAnim {
                moves,
                start: Instant::now(),
            });
        }
        self.scene.mark_dirty();
        self.show_toast("Нода вставлена в группу");
    }

    /// Вынос детей из групп после drag (FR-012): нода, отпущенная вне rect
    /// своей ЯВНОЙ группы, удаляется из её детей. Легаси-группы (без
    /// списка) не участвуют — их геометрический фолбэк не меняется.
    fn group_drag_out_released(&mut self) {
        let Some(drag) = self.scene.dragging.as_ref() else {
            return;
        };
        let dragged: Vec<usize> = std::iter::once(drag.primary)
            .chain(drag.origins.iter().map(|(i, _)| *i))
            .collect();
        // (группа → [id нод к выносу])
        let mut removals: Vec<(usize, String)> = Vec::new();
        for index in dragged {
            let Some(node) = self.scene.canvas.nodes.get(index) else {
                continue;
            };
            let center = [node.x + node.width / 2.0, node.y + node.height / 2.0];
            for (gi, group) in self.scene.canvas.nodes.iter().enumerate() {
                if gi == index || group.kind() != NodeKind::Group {
                    continue;
                }
                let Some(children) = &group.children else {
                    continue; // легаси-группа: геометрия, жеста нет
                };
                if !children.contains(&node.id) {
                    continue;
                }
                let outside = center[0] < group.x
                    || center[0] > group.x + group.width
                    || center[1] < group.y
                    || center[1] > group.y + group.height;
                if outside {
                    removals.push((gi, node.id.clone()));
                }
            }
        }
        if removals.is_empty() {
            return;
        }
        // FR-006: вынос — undo-шаг (один на все удаления)
        self.push_undo();
        for (gi, id) in removals {
            canvas_core::group_remove_child(&mut self.scene.canvas, gi, &id);
        }
        self.scene.mark_dirty();
        self.show_toast("Нода вынесена из группы");
    }

    /// Rect открытого меню канваса (логические px) — airspace для виджетов.
    fn menu_open_rect(&self) -> Option<[f32; 4]> {
        let menu = self.menu.as_ref()?;
        Some(menu_rect_for(menu.origin, CANVAS_MENU_ITEMS.len()))
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Штатный выход (T17): форс-сейв (SPEC §9 — не ждать
                // debounce) + восстановление иконок (R5) — единая точка
                // с пунктом меню «Выход»
                self.shutdown(event_loop);
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_scale_factor(scale_factor);
                }
                // Миникарта (T13): буфер растеризован в физических px —
                // пересоберётся на ближайшем кадре (размер в сигнатуре)
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(&event),
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Middle => {
                    self.middle_pressed = state == ElementState::Pressed;
                    self.sync_cursor_icon();
                }
                MouseButton::Left => self.on_left_button(state),
                MouseButton::Right => self.on_right_button(state, event_loop),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),
            WindowEvent::CursorLeft { .. } => {
                // Курсор ушёл из окна: hover-порты гаснут (иначе подсветка
                // «залипает» до следующего входа)
                if self.hovered.take().is_some() {
                    self.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
                // Потеря фокуса окна — сброс залипших жестов (alt-tab во
                // время drag: Released придёт другому окну — нода/пан
                // оставались «прилипшими»)
                self.cancel_pointer_transients();
                self.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),
            WindowEvent::PinchGesture { delta, .. } => self.on_pinch(delta),
            WindowEvent::RedrawRequested => {
                // Замер интервала между кадрами для HUD (T5)
                let now = Instant::now();
                if let Some(prev) = self.last_frame {
                    self.frame_meter.push(now - prev);
                }
                self.last_frame = Some(now);
                // Полёт камеры к результату поиска (T14): семпл ease-out —
                // пока полёт активен, about_to_wait держит кадры идущими
                if let Some((flight, start)) = self.flight.take() {
                    let elapsed = start.elapsed().as_millis() as u32;
                    let (center, zoom) = flight.sample(elapsed);
                    self.camera.set_center(center);
                    self.camera.set_zoom(zoom);
                    if !flight.is_finished(elapsed) {
                        self.flight = Some((flight, start));
                    }
                }
                // FR-012: settle-анимация вставки в группу — группа и
                // раздвинутые соседи едут к целевым позициям ease_out_cubic
                if let Some(anim) = &self.settle_anim {
                    let t = (anim.start.elapsed().as_millis() as f32 / SETTLE_ANIM_MS).min(1.0);
                    let k = ease_out_cubic(t);
                    for (index, from, to) in &anim.moves {
                        if let Some(node) = self.scene.canvas.nodes.get_mut(*index) {
                            node.x = from[0] + (to[0] - from[0]) * k;
                            node.y = from[1] + (to[1] - from[1]) * k;
                        }
                        if let Some(node) = self.scene.canvas.nodes.get(*index) {
                            self.scene.spatial.update(*index, node);
                        }
                    }
                    if t >= 1.0 {
                        self.settle_anim = None;
                    }
                    self.request_redraw();
                }
                // Миникарта (T13): пересборка по dirty-условиям ДО отрисовки
                // (текстура должна быть готова к проходу кадра)
                self.update_minimap();
                let hud = self.hud_text();
                // World-оверлеи: Т9 призраки дропа добавляются в конец — mutable
                let mut overlay_instances: Vec<CardInstance> = Vec::new();
                let mut overlay_labels: Vec<String> = Vec::new();
                let mut overlay_label_pos: Vec<Vec2> = Vec::new();
                // Ширины подписей оверлея: призраки дропа — по ширине
                // карточки-призрака (Т9)
                let mut overlay_widths: Vec<f32> = Vec::new();
                // Панель настроек (screen-space): кнопка + строки переключателей
                let (mut screen_instances, mut owned_texts) = self.settings_overlay();
                // Меню пустого канваса (T7): screen-space, константный размер
                {
                    let (menu_instances, menu_texts) = self.canvas_menu_overlay();
                    screen_instances.extend(menu_instances);
                    owned_texts.extend(menu_texts);
                }
                // Палитра выделения (FR-009/FR-010): тулбар под выделением;
                // rect'ы запоминаются для airspace виджетов
                let palette_view = self.palette_view();
                if let Some((lay, groups, open)) = &palette_view {
                    let (pal_instances, pal_texts) = self.palette_overlay(lay, groups, *open);
                    screen_instances.extend(pal_instances);
                    owned_texts.extend(pal_texts);
                }
                // Панель поиска (T14): квады/тексты поверх всего канваса
                {
                    let (search_instances, search_texts) = self.search_overlay();
                    screen_instances.extend(search_instances);
                    owned_texts.extend(search_texts);
                }
                // FR-018: палитра шаблонов (Ctrl+P) и wheel-меню
                // (Shift+клик) — поверх канваса
                {
                    let (tpl_instances, tpl_texts) = self.template_panel_overlay();
                    screen_instances.extend(tpl_instances);
                    owned_texts.extend(tpl_texts);
                    let (wheel_instances, wheel_texts) = self.wheel_overlay();
                    screen_instances.extend(wheel_instances);
                    owned_texts.extend(wheel_texts);
                }
                // FR-021: popup подсказок Numi-ввода — поверх редактора
                {
                    let (hint_instances, hint_texts) = self.hints_overlay();
                    screen_instances.extend(hint_instances);
                    owned_texts.extend(hint_texts);
                }
                // Тултип битой ссылки (T10, SPEC §7.5): у курсора — старый путь
                // файла; screen-space, константный размер при любом зуме
                if let Some(file) = self.hovered.and_then(|index| {
                    self.scene.canvas.nodes.get(index).and_then(|node| {
                        (node.broken_link == Some(true))
                            .then(|| node.file.clone())
                            .flatten()
                    })
                }) {
                    // Ограничиваем правым краём окна, чтобы длинный путь
                    // не вылез за экран (width — только клип-бounds)
                    let viewport = self.viewport_logical();
                    let origin_x =
                        (self.cursor[0] + 14.0).min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                    owned_texts.push(OwnedScreenText {
                        text: format!("Файл недоступен: {file}"),
                        origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                        width: TOOLTIP_WIDTH,
                        font_size: 13.0,
                        color: Color::rgb(0xd4, 0xd4, 0xd4),
                        align: TextAlign::Left,
                    });
                }
                // Тултип ошибки формульной строки (FR-013, правка 4): курсор
                // над бейджем «!» (зоны — с прошлого кадра) — сообщение об
                // ошибке у курсора; так видно, ЧТО именно не так в расчёте
                if let Some(hit) = self.expr_error_hit_at(self.cursor) {
                    let viewport = self.viewport_logical();
                    let origin_x =
                        (self.cursor[0] + 14.0).min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                    owned_texts.push(OwnedScreenText {
                        text: hit.message.clone(),
                        origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                        width: TOOLTIP_WIDTH,
                        font_size: 13.0,
                        color: Color::rgb(0xe5, 0x5c, 0x5c),
                        align: TextAlign::Left,
                    });
                }
                // T21: модальный диалог (screen-space): панель + тексты +
                // кнопки; рендер после битой ссылки — поверх всего канваса
                if let Some(dialog) = &self.dialog {
                    let [dx, dy, dw, dh] = self.dialog_rect();
                    screen_instances.push(CardInstance {
                        pos: [dx, dy],
                        size: [dw, dh],
                        fill: [0.09, 0.11, 0.15, 0.97],
                        border: [0.23, 0.51, 0.96, 1.0],
                        params: [10.0, 0.0, 0.0, 1.0],
                    });
                    let buttons = self.dialog_button_rects();
                    for (i, (label, _)) in dialog.buttons().iter().enumerate() {
                        let [bx, by, bw, bh] = buttons[i];
                        screen_instances.push(CardInstance {
                            pos: [bx, by],
                            size: [bw, bh],
                            fill: if i == 0 {
                                [0.16, 0.32, 0.60, 1.0]
                            } else {
                                [0.20, 0.23, 0.29, 1.0]
                            },
                            border: [0.35, 0.40, 0.50, 1.0],
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                        owned_texts.push(OwnedScreenText {
                            text: (*label).to_owned(),
                            origin: [bx + bw / 2.0, by + 7.0],
                            width: bw - 8.0,
                            font_size: 14.0,
                            color: Color::rgb(0xe8, 0xec, 0xf4),
                            align: TextAlign::Center,
                        });
                    }
                    owned_texts.push(OwnedScreenText {
                        text: dialog.title(&self.scene.canvas),
                        origin: [dx + 20.0, dy + 16.0],
                        width: dw - 40.0,
                        font_size: 16.0,
                        color: Color::rgb(0xe8, 0xec, 0xf4),
                        align: TextAlign::Left,
                    });
                    owned_texts.push(OwnedScreenText {
                        text: dialog.body(),
                        origin: [dx + 20.0, dy + 46.0],
                        width: dw - 40.0,
                        font_size: 13.0,
                        color: Color::rgb(0xb6, 0xbe, 0xce),
                        align: TextAlign::Left,
                    });
                }
                // T21: toast — строка внизу центра, живёт 3 с (T21-A).
                // Истечение проверяем ДО рендера (без borrow-конфликта)
                let toast_alive = self
                    .toast
                    .as_ref()
                    .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < 3.0);
                if !toast_alive {
                    self.toast = None;
                } else if let Some((text, _)) = &self.toast {
                    let viewport = self.viewport_logical();
                    let ty = viewport[1] - 44.0;
                    owned_texts.push(OwnedScreenText {
                        text: text.clone(),
                        origin: [viewport[0] / 2.0, ty],
                        width: viewport[0] - 80.0,
                        font_size: 14.0,
                        color: Color::rgb(0xf0, 0xe6, 0xc2),
                        align: TextAlign::Center,
                    });
                }
                let screen_texts: Vec<ScreenText> = owned_texts
                    .iter()
                    .map(|t| ScreenText {
                        text: &t.text,
                        origin: t.origin,
                        width: t.width,
                        font_size: t.font_size,
                        color: t.color,
                        align: t.align,
                    })
                    .collect();
                // Призраки зоны дропа (T9): рамка bbox сетки + квады-призраки.
                // Кладём В КОНЕЦ оверлея: порядок инстансов = порядок рисования,
                // depth-теста нет — призраки поверх всего
                if let Some(preview) = &self.drop_preview {
                    let positions = canvas_app::ui::drop_grid(preview.origin, preview.plan.len());
                    if let Some(frame) = canvas_render::cards::drop_zone_frame(
                        &positions,
                        [canvas_app::ui::DROP_CARD_W, canvas_app::ui::DROP_CARD_H],
                        canvas_app::ui::DROP_GRID_GAP,
                    ) {
                        overlay_instances.push(frame);
                    }
                    overlay_instances.extend(canvas_render::cards::drop_ghosts(
                        &positions,
                        [canvas_app::ui::DROP_CARD_W, canvas_app::ui::DROP_CARD_H],
                        canvas_app::ui::DROP_PREVIEW_MAX,
                    ));
                    // Подписи призраков (Т9): во время перетаскивания имена
                    // файлов/первая строка заметки видны до самого дропа —
                    // раньше призраки были пустыми рамками
                    let pad = canvas_app::ui::DROP_GHOST_LABEL_PAD;
                    for (ins, pos) in preview.plan.iter().zip(&positions) {
                        overlay_labels.push(canvas_app::ui::drop_ghost_label(&ins.kind));
                        overlay_label_pos.push([pos[0] + pad, pos[1] + 6.0]);
                        overlay_widths.push(canvas_app::ui::DROP_CARD_W - pad * 2.0);
                    }
                }
                let overlay_texts: Vec<OverlayText> = overlay_labels
                    .iter()
                    .zip(&overlay_label_pos)
                    .zip(&overlay_widths)
                    .map(|((label, pos), width)| OverlayText {
                        text: label,
                        origin: *pos,
                        width: *width,
                    })
                    .collect();
                // Пульс подсветки ноды-результата (T14): world-квад с рамкой,
                // затухающей по pulse_alpha; за вырожденный — сброс (рамка
                // оверлейная — border.a, заливка прозрачна после фикса
                // cards.wgsl)
                if let Some((node, start)) = self.pulse {
                    let alpha = pulse_alpha(start.elapsed().as_millis() as u32);
                    if alpha <= 0.0 {
                        self.pulse = None;
                    } else if let Some(target) = self.scene.canvas.nodes.get(node) {
                        let grow = (1.0 - alpha) * 8.0;
                        overlay_instances.push(CardInstance {
                            pos: [target.x - grow, target.y - grow],
                            size: [target.width + grow * 2.0, target.height + grow * 2.0],
                            fill: [0.0; 4],
                            border: [1.0, 0.85, 0.35, alpha],
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // Рамка выделения (CR-001): полупрозрачный world-квад с
                // акцентной рамкой (стиль зоны дропа T9), без тени
                if let Some((start, current, _)) = self.select_rect {
                    let rect = rubber_band_rect(start, current);
                    overlay_instances.push(CardInstance {
                        pos: [rect[0], rect[1]],
                        size: [rect[2], rect[3]],
                        fill: canvas_app::ui::SELECT_RECT_FILL,
                        border: canvas_app::ui::SELECT_RECT_BORDER,
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                // FR-012: подсветка зоны втягивания — группа под drag-нодой
                if let Some(gi) = self.group_drop_target {
                    if let Some(group) = self.scene.canvas.nodes.get(gi) {
                        overlay_instances.push(CardInstance {
                            pos: [group.x - 4.0, group.y - 4.0],
                            size: [group.width + 8.0, group.height + 8.0],
                            fill: [0.396, 0.612, 0.969, 0.10],
                            border: [0.396, 0.612, 0.969, 0.9],
                            params: [8.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // M5 (T20-F): airspace-прямоугольники оверлеев (план П7) —
                // до LOD-кадра виджетов; большие панели (поиск/настройки)
                // упрощённо гасят все live (транзиентно), точные rect'ы —
                // меню/подменю/палитра/хоткеи/миникарта
                let mut widget_airspace: Vec<[f32; 4]> = Vec::new();
                if let Some(rect) = self.menu_open_rect() {
                    widget_airspace.push(rect);
                    if let Some(menu) = self.menu.as_ref() {
                        if let Some(submenu) = &menu.submenu {
                            widget_airspace.push(submenu_rect(submenu));
                        }
                    }
                }
                // Палитра выделения: бар + открытая колонка (логические px)
                if let Some((lay, _, open)) = &palette_view {
                    widget_airspace.push(lay.bar);
                    if let Some(open) = open {
                        widget_airspace.push(lay.groups[*open].dropdown);
                    }
                }
                if self.hotkeys_open {
                    widget_airspace.push(hotkeys_panel_rect(self.viewport_logical()));
                }
                if let Some(renderer) = self.renderer.as_ref() {
                    if let Some(rect) = renderer.minimap_rect_logical() {
                        widget_airspace.push(rect);
                    }
                }
                let widget_overlay_active = self.select_rect.is_some()
                    || self.edge_drag.is_some()
                    || self.drop_preview.is_some()
                    || self.settings_open
                    || self.dialog.is_some()
                    || self.search.is_open();
                // T21: модальный диалог — airspace-зона (П7): живые виджеты
                // под ним гасятся в снапшоты, пока диалог открыт
                if self.dialog.is_some() {
                    widget_airspace.push(self.dialog_rect());
                }
                let widget_frame = self.widgets.update_frame(
                    &self.scene.canvas,
                    &self.camera,
                    self.viewport_logical(),
                    self.scale_factor(),
                    &widget_airspace,
                    widget_overlay_active,
                );
                // Owned-квады → ссылки для FrameOverlay (локально: заём
                // живёт до конца кадра, конфликтов с &mut self нет)
                let widget_quad_refs: Vec<canvas_render::WidgetQuad> = widget_frame
                    .quads
                    .iter()
                    .map(|q| canvas_render::WidgetQuad {
                        node_id: q.node_id.as_str(),
                        pos: q.pos,
                        size: q.size,
                    })
                    .collect();
                // CR-004: прозрачные виджет-ноды — контент реально виден
                // (live-HWND или снапшот-текстура); placeholder битых
                // пакетов (broken) остаётся серой карточкой. Виджет без
                // снапшота и вне live (транзиент первого кадра) — тоже
                // непрозрачен: иначе нода исчезала бы целиком.
                let live_ids: std::collections::HashSet<&str> =
                    widget_frame.live.iter().map(|s| s.as_str()).collect();
                let broken_ids: std::collections::HashSet<&str> =
                    widget_frame.broken.iter().map(|s| s.as_str()).collect();
                let widget_transparent: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.kind() == NodeKind::Widget)
                    .filter(|(_, node)| {
                        let id = node.id.as_str();
                        !broken_ids.contains(id)
                            && (live_ids.contains(id)
                                || self
                                    .renderer
                                    .as_ref()
                                    .is_some_and(|r| r.has_widget_snapshot(id)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // CR-004 v1: заголовок виджет-ноды виден при hover/выделении
                let widget_title_reveal: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, node)| {
                        node.kind() == NodeKind::Widget
                            && (self.hovered == Some(*i)
                                || self.scene.selected == Some(Selection::Node(*i))
                                || self.scene.selected_nodes.contains(i))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // FR-011: скрытые ноды (свернутые поддеревья) + бейджи «+N»
                let hidden_nodes = self.hidden_subtree_nodes();
                let collapsed_counts: Vec<(usize, usize)> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.collapsed == Some(true))
                    .map(|(i, _)| (i, canvas_core::subtree_ids(&self.scene.canvas, i).len()))
                    .filter(|(_, count)| *count > 0)
                    .collect();
                let overlay = FrameOverlay {
                    instances: &overlay_instances,
                    texts: &overlay_texts,
                    screen_instances: &screen_instances,
                    screen_texts: &screen_texts,
                    widget_quads: &widget_quad_refs,
                };
                // Резиновая линия (T8/CR-002): от порта/неподвижного конца к
                // курсору; исходная линия перепривязываемой связи скрыта
                let edge_draft = self.edge_drag.as_ref().and_then(|drag| {
                    let (port, side) = drag.draft_origin(&self.scene.canvas)?;
                    Some((port, side, self.cursor_world()))
                });
                let hidden_edge = match self.edge_drag.as_ref() {
                    Some(EdgeDrag::Rebind { edge_index, .. }) => Some(*edge_index),
                    _ => None,
                };
                // T23 (brainstorm-focus): пересчёт анимации и окрестности
                // семени ДО сборки сцены — FocusView заимствует поля App
                self.update_focus_state();
                let focus = FocusView {
                    nodes: &self.focus_nodes,
                    edges: &self.focus_edges,
                    dim: self.focus_dim,
                    pulse: self
                        .focus_pulse
                        .as_ref()
                        .map(|(_, start)| focus_pulse(start.elapsed().as_millis() as u32))
                        .unwrap_or(0.0),
                };
                if let Some(renderer) = self.renderer.as_mut() {
                    // FR-013 (правка 4): живые построчные результаты (Numi —
                    // результаты по ходу набора): считаем из текста СЕССИИ
                    // на каждый кадр (дёшево: парсинг только формульных
                    // строк; fit_note_size уже вызывает eval_lines покадрово)
                    let editing_line_results: Option<Vec<Option<ExprOutcome>>> =
                        self.editing.as_ref().and_then(|session| {
                            let index = session.node_index()?;
                            let node = self.scene.canvas.nodes.get(index)?;
                            node.kind()
                                .eq(&NodeKind::Text)
                                .then(|| expr::eval_lines(&session.text()))
                        });
                    let scene = SceneView {
                        canvas: &self.scene.canvas,
                        spatial: &self.scene.spatial,
                        selected: self.scene.selected,
                        selected_nodes: &self.scene.selected_nodes,
                        hovered: self.hovered,
                        edge_draft,
                        hidden_edge,
                        edges_avoid: self.settings.edges_avoid_nodes,
                        port_zone_px: self.settings.port_zone_px,
                        focus,
                        widget_transparent: &widget_transparent,
                        widget_title_reveal: &widget_title_reveal,
                        hidden_nodes: &hidden_nodes,
                        collapsed_counts: &collapsed_counts,
                        expr_results: &self.scene.expr_results,
                        expr_line_results: &self.scene.expr_line_results,
                        expr_editing_results: editing_line_results.as_deref(),
                    };
                    match renderer.render(
                        &self.camera,
                        &scene,
                        hud.as_deref(),
                        self.editing.as_mut(),
                        &overlay,
                    ) {
                        Ok(stats) => self.last_stats = stats,
                        Err(err) => {
                            tracing::error!(%err, "ошибка рендера, завершение");
                            event_loop.exit();
                        }
                    }
                    // FR-013 (правка 4): зоны ошибок кадра — для тултипа в
                    // оверлее следующего кадра (отставание в кадр незаметно)
                    self.expr_error_hits = renderer.line_error_hits().to_vec();
                }
                // Тамбнейлы видимых нод (T6): заказ после кадра, когда камера
                // уже установилась; ответы придут через AppEvent::ThumbsReady
                self.order_thumbnails();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ThumbsReady => {
                // Забрать готовые тамбнейлы из канала и загрузить в атлас;
                // ошибки — в негативный кэш (не перезаказывать каждый кадр)
                let mut arrived = 0usize;
                for (node, result) in self.thumbs.drain() {
                    match result {
                        Some(thumb) => {
                            if let Some(renderer) = self.renderer.as_mut() {
                                renderer.set_thumbnail(node, &thumb);
                                arrived += 1;
                            }
                        }
                        None => {
                            self.thumbs_failed.insert(node);
                        }
                    }
                }
                if arrived > 0 {
                    self.request_redraw();
                }
            }
            AppEvent::Drag(event) => self.on_drag_event(event),
            AppEvent::FileEvents(events) => self.on_file_events(events),
            AppEvent::Search(event) => self.on_search_event(event),
            #[cfg(windows)]
            AppEvent::Desktop(event) => self.on_desktop_event(event),
            #[cfg(windows)]
            AppEvent::Shell(event) => self.on_shell_event(event),
            #[cfg(windows)]
            AppEvent::McpWake => self.on_mcp_wake(),
            AppEvent::Widget(event) => self.on_widget_event(event),
            // T15-relaunch: exit-сигнал от нового запуска (single-instance
            // handoff) — штатное завершение: форс-сейв сцены, восстановление
            // иконок, exit. Мьютекс освободится смертью процесса, новый
            // инстанс продолжит старт (актуально для перезапуска на --desktop).
            AppEvent::InstanceExit => self.shutdown(event_loop),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.scene.autosave_if_due();
        // Debounce запроса поиска (T14): 200 мс покоя после правки — отправка.
        // Панель/анимации держат цикл красным через request_redraw ниже,
        // иначе ControlFlow::Wait уснул бы до следующего события
        if let Some((query, edited_at)) = self.search_pending.take() {
            if edited_at.elapsed() < SEARCH_DEBOUNCE {
                self.search_pending = Some((query, edited_at));
            } else {
                self.search_service.command(SearchCommand::Query {
                    query,
                    limit: SEARCH_RESULTS_LIMIT,
                });
            }
        }
        // Полёт камеры и пульс (T14) + фокус (T23): непрерывные кадры
        // до завершения анимаций; hover-ожидание палитры (FR-009):
        // hover-intent открытие / отсрочка закрытия при неподвижном курсоре
        if self.search_pending.is_some()
            || self.flight.is_some()
            || self.pulse.is_some()
            || self.focus_animating()
            || self.palette_hover.pending()
        {
            self.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("CanvasDesk");
        // Режим десктопа (T15): borderless-окно на весь виртуальный экран
        // без активации при создании (WS_EX_NOACTIVATE до первого клика —
        // TASKS T15; winit with_active(false)). Это же окно — фолбэк-режим,
        // если встройка не удастся (R14: не пересоздаём после winit-инициализации).
        // После attach winit-API окна НЕ трогаем — стили перезапишет
        // библиотека (R3-урок tao/Seelen); размеры — только SetWindowPos.
        let attrs = if self.desktop_mode {
            attrs
                .with_decorations(false)
                .with_resizable(false)
                .with_active(false)
        } else {
            attrs
        };
        // winit сам ставит свой IDropTarget (RegisterDragDrop с assert S_OK) —
        // отключаем и ставим свой в canvas-shell (план T9 §3)
        #[cfg(windows)]
        let attrs = attrs.with_drag_and_drop(false);
        // Точная геометрия десктоп-окна (физ. px) — только на Windows:
        // виртуальный экран из EnumDisplayMonitors; до attach — стартовый
        // размер по экрану (потом attach растянет SetWindowPos'ом).
        #[cfg(windows)]
        let attrs = if self.desktop_mode {
            let screen = canvas_shell::desktop::hierarchy::virtual_screen_rect().unwrap_or(
                canvas_shell::desktop::ScreenRect::from_ltrb(0, 0, 1280, 720),
            );
            attrs
                .with_position(winit::dpi::PhysicalPosition::new(screen.left, screen.top))
                .with_inner_size(winit::dpi::PhysicalSize::new(
                    screen.width().max(1) as u32,
                    screen.height().max(1) as u32,
                ))
        } else {
            attrs
        };
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!(%err, "не удалось создать окно");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());
        // Регистрация своего IDropTarget (T9) и встройка в десктоп (T15) — ДО
        // создания GPU-surface: так attach (SetParent/scrub) не конфликтует
        // с живым swapchain. Сама по себе невидимость встроенного окна
        // порядком не лечилась (проверено экспериментом): Vulkan-swapchain
        // не презентует в ребёнка Progman вне зависимости от момента
        // создания surface — лечится выбором DX12 для desktop-режима
        // (Renderer::new, prefer_dx12). HWND достаём через raw-window-handle
        // (winit 0.30 публично Win32-HWND не отдаёт); ошибка drag-drop —
        // warn и живём без него (graceful degradation).
        #[cfg(windows)]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            // HWND через raw-window-handle: winit 0.30 публично
            // Win32-HWND не отдаёт (внутренний windows-sys); окно
            // создано на этом потоке, handle доступен
            match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::Win32(win32) => {
                        match canvas_shell::dragdrop::install(
                            win32.hwnd.get(),
                            self.drag_sender.clone(),
                        ) {
                            Ok(watcher) => self.drag_watcher = Some(watcher),
                            Err(err) => {
                                tracing::warn!(%err, "drag-drop недоступен, приложение работает без него")
                            }
                        }
                        // Встройка в десктоп (T15): после всей winit-настройки
                        // окна (R3-урок: сначала окно настраивается библиотекой,
                        // репарентинг — последним, с верификацией стилей в
                        // attach), но ДО создания GPU-surface (см. выше).
                        if self.desktop_mode {
                            self.attach_desktop(win32.hwnd.get());
                        }
                        // M5 (T20-F): WebView2-хост виджетов — ребёнок окна
                        // канваса; события хоста идут через widget_sender
                        // (прокси из main: у ActiveEventLoop нет create_proxy,
                        // winit 0.30). User-data — единый корень приложения
                        // (~/.canvasdesk/webview2)
                        {
                            let sender = self.widget_sender.clone();
                            let user_data = canvas_shell::default_cache_dir()
                                .unwrap_or_default()
                                .join("webview2");
                            // hwnd.get() уже isize (raw-window-handle 0.6):
                            // без каста — иначе clippy needless_cast на Windows
                            self.widgets
                                .attach_host(win32.hwnd.get(), user_data, sender);
                        }
                    }
                    // На Windows бывает только Win32-handle
                    _ => tracing::warn!("неожиданный handle окна — drag-drop выключен"),
                },
                Err(err) => {
                    tracing::warn!(%err, "handle окна недоступен — drag-drop выключен")
                }
            }
        }
        // GPU-инициализация блокирующая, один раз при старте (SPEC §6.3:
        // холодный старт < 2 с). prefer_dx12 = desktop-режим: Vulkan не
        // презентует в ребёнка Progman (подробности — в Renderer::new).
        match pollster::block_on(canvas_render::Renderer::new(
            window.clone(),
            self.desktop_mode,
        )) {
            Ok(mut renderer) => {
                renderer.set_grid_visible(self.settings.grid_visible);
                renderer.set_grid_dots(self.settings.grid_style == GridStyle::Dots);
                let (minor, major) = self.settings.grid_density.steps();
                renderer.set_grid_steps(minor, major);
                renderer.set_theme(ThemeColors::from_theme(self.settings.theme));
                tracing::info!(
                    width = window.inner_size().width,
                    height = window.inner_size().height,
                    scale_factor = window.scale_factor(),
                    "окно создано"
                );
                self.renderer = Some(renderer);
                self.request_redraw();
            }
            Err(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Стартовый канвас непустой и переживает round-trip.
    #[test]
    fn seed_canvas_is_valid() {
        let canvas = seed_canvas();
        assert!(!canvas.nodes.is_empty());
        let json = canvas.to_json().expect("сериализация seed");
        let restored = Canvas::from_str(&json).expect("seed парсится обратно");
        assert_eq!(canvas, restored);
    }

    /// Стресс-генератор (T5): ровно N нод, детерминизм, размеры в пределах.
    #[test]
    fn stress_canvas_is_deterministic_and_bounded() {
        let a = stress_canvas(5000);
        let b = stress_canvas(5000);
        assert_eq!(a.nodes.len(), 5000);
        assert_eq!(a, b, "одинаковый N должен давать одинаковую сцену");
        for node in &a.nodes {
            assert!((120.0..=420.0).contains(&node.width));
            assert!((80.0..=300.0).contains(&node.height));
            assert_eq!(node.kind(), canvas_core::NodeKind::Text);
            assert!(node.id.starts_with("stress-"));
        }
    }

    /// Резолв путей файловых нод (T6): относительные — от каталога канваса,
    /// результат всегда абсолютный (shell-API иначе отказывает).
    /// Windows-only: тест оперирует Windows-путями (диск `C:`), на Unix
    /// они не абсолютны — семантика проверяется на CI (windows-latest).
    #[cfg(windows)]
    #[test]
    fn resolve_file_path_is_absolute() {
        let scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/thumbs.canvas"));
        let abs = scene.resolve_file_path("C:/abs/photo.png");
        assert_eq!(abs, PathBuf::from("C:/abs/photo.png"));
        let rel = scene.resolve_file_path("thumbtest/photo1.png");
        assert!(
            rel.is_absolute(),
            "относительный путь не абсолютизирован: {rel:?}"
        );
        assert!(rel.ends_with(PathBuf::from("target/tmp/thumbtest/photo1.png")));
    }

    /// Парсинг аргументов: --stress N, --stress=N, --desktop, путь, дефолты.
    #[test]
    fn cli_args_parsing() {
        let args = parse_args(&[]).expect("пустые аргументы");
        assert_eq!(args.stress, None);
        assert!(!args.desktop);
        assert_eq!(args.path, PathBuf::from("default.canvas"));

        let args = parse_args(&["--stress".into(), "5000".into()]).expect("--stress N");
        assert_eq!(args.stress, Some(5000));
        assert!(!args.desktop);
        assert_eq!(args.path, PathBuf::from("stress.canvas"));

        let args =
            parse_args(&["--stress=100".into(), "my.canvas".into()]).expect("--stress=N path");
        assert_eq!(args.stress, Some(100));
        assert_eq!(args.path, PathBuf::from("my.canvas"));

        // T15: флаг --desktop (булев, повтор идемпотентен, порядок любой)
        let args = parse_args(&["--desktop".into()]).expect("--desktop");
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("default.canvas"));

        let args =
            parse_args(&["--desktop".into(), "board.canvas".into()]).expect("--desktop path");
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("board.canvas"));

        let args = parse_args(&["--stress=7".into(), "--desktop".into(), "--desktop".into()])
            .expect("--stress + двойной --desktop");
        assert_eq!(args.stress, Some(7));
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("stress.canvas"));

        assert!(parse_args(&["--stress".into()]).is_err());
        assert!(parse_args(&["--stress".into(), "abc".into()]).is_err());
        assert!(parse_args(&["a.canvas".into(), "b.canvas".into()]).is_err());
    }

    // --- MCP-интеграция: mcp_dispatch (15 инструментов) ---

    /// Тестовая сцена: заметка, файл, группа со связью (MCP-тесты).
    fn mcp_scene() -> SceneState {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("n1", "Привет Мир", 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("f1", "docs/SPEC.md", 500.0, 100.0, 320.0, 220.0));
        let mut group = Node::group("g1", 0.0, 0.0, 900.0, 600.0);
        group.label = Some("Зона работы".to_owned());
        canvas.nodes.push(group);
        canvas.add_edge(Edge::new("edge-1", "n1", None, "f1", Some(Side::Right)));
        SceneState::new(canvas, PathBuf::from("target/tmp/mcp.canvas"))
    }

    fn dispatch(
        scene: &mut SceneState,
        camera: &mut Camera,
        method: &str,
        params: &str,
    ) -> Result<serde_json::Value, String> {
        let params: serde_json::Value = serde_json::from_str(params).expect("params — JSON");
        let registry = canvas_core::templates::TemplateRegistry::builtin();
        mcp_dispatch(scene, camera, &registry, method, &params)
    }

    /// canvas_info: счётчики нод/связей и путь к файлу.
    #[test]
    fn mcp_canvas_info_counts() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let info = dispatch(&mut scene, &mut camera, "canvas_info", "{}").expect("canvas_info");
        assert_eq!(info["nodes"], 3);
        assert_eq!(info["edges"], 1);
        assert_eq!(info["path"], "target/tmp/mcp.canvas");
    }

    /// nodes_list: сводки без text по умолчанию, с text по флагу; node_get —
    /// полная нода.
    #[test]
    fn mcp_nodes_list_and_get() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let list = dispatch(&mut scene, &mut camera, "nodes_list", "{}").expect("nodes_list");
        let first = &list[0];
        assert_eq!(first["id"], "n1");
        assert_eq!(first["type"], "text");
        assert!(
            !first.as_object().unwrap().contains_key("text"),
            "text скрыт"
        );
        let list = dispatch(&mut scene, &mut camera, "nodes_list", r#"{"text":true}"#)
            .expect("nodes_list с text");
        assert_eq!(list[0]["text"], "Привет Мир");

        let node =
            dispatch(&mut scene, &mut camera, "node_get", r#"{"id":"f1"}"#).expect("node_get");
        assert_eq!(node["file"], "docs/SPEC.md");
        assert_eq!(node["width"], 320.0);
        let err = dispatch(&mut scene, &mut camera, "node_get", r#"{"id":"ghost"}"#)
            .expect_err("нет такой ноды");
        assert!(err.contains("не найдена"));
    }

    /// nodes_search: подстрока без учёта регистра по text/label/file.
    #[test]
    fn mcp_nodes_search_case_insensitive() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let hits = dispatch(
            &mut scene,
            &mut camera,
            "nodes_search",
            r#"{"query":"привет"}"#,
        )
        .expect("search");
        assert_eq!(hits.as_array().expect("массив").len(), 1);
        assert_eq!(hits[0]["id"], "n1");
        // по label группы
        let hits = dispatch(
            &mut scene,
            &mut camera,
            "nodes_search",
            r#"{"query":"ЗОНА"}"#,
        )
        .expect("search по label");
        assert_eq!(hits[0]["id"], "g1");
        // по file
        let hits = dispatch(
            &mut scene,
            &mut camera,
            "nodes_search",
            r#"{"query":"spec.md"}"#,
        )
        .expect("search по file");
        assert_eq!(hits[0]["id"], "f1");
        // мимо
        let hits = dispatch(
            &mut scene,
            &mut camera,
            "nodes_search",
            r#"{"query":"zzz"}"#,
        )
        .expect("search пусто");
        assert!(hits.as_array().expect("массив").is_empty());
    }

    /// node_create_note: дефолтные размеры 260×120, переопределение, id
    /// со свободным суффиксом, spatial index обновлён, канвас грязный.
    #[test]
    fn mcp_node_create_note() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let created = dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"x":50.0,"y":900.0}"#,
        )
        .expect("create_note");
        assert_eq!(created["id"], "note-1");
        let index = scene.canvas.nodes.len() - 1;
        let node = &scene.canvas.nodes[index];
        assert_eq!((node.x, node.y), (50.0, 900.0));
        assert_eq!((node.width, node.height), (260.0, 120.0));
        assert!(scene.dirty_since.is_some(), "канвас грязный");
        // spatial видит новую ноду
        let hits = scene.spatial.query_rect([50.0, 900.0, 60.0, 910.0]);
        assert!(hits.contains(&index), "новая нода в spatial");

        let created = dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"x":0.0,"y":0.0,"text":"abc","width":400.0,"height":300.0}"#,
        )
        .expect("create_note с размерами");
        assert_eq!(created["id"], "note-2");
        let node = scene.canvas.nodes.last().expect("нода");
        assert_eq!((node.width, node.height), (400.0, 300.0));
        assert_eq!(node.text.as_deref(), Some("abc"));

        // x обязателен
        assert!(dispatch(&mut scene, &mut camera, "node_create_note", r#"{"y":1.0}"#).is_err());
    }

    /// node_create_file: карточка по пути, файл на диске НЕ создаётся,
    /// дефолтные размеры DROP_CARD.
    #[test]
    fn mcp_node_create_file_no_disk_write() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let disk_path = PathBuf::from("target/tmp/mcp_never_created.txt");
        let _ = std::fs::remove_file(&disk_path);
        let params = format!(r#"{{"path":"{}","x":10.0,"y":20.0}}"#, disk_path.display());
        let created =
            dispatch(&mut scene, &mut camera, "node_create_file", &params).expect("create_file");
        assert_eq!(created["id"], "file-1");
        let node = scene.canvas.nodes.last().expect("нода");
        assert_eq!(node.kind(), NodeKind::File);
        assert_eq!(
            (node.width, node.height),
            (canvas_app::ui::DROP_CARD_W, canvas_app::ui::DROP_CARD_H)
        );
        assert!(!disk_path.exists(), "MCP не создаёт файл на диске");
    }

    /// node_update_text / node_move / node_resize: модель + spatial + dirty.
    #[test]
    fn mcp_node_update_move_resize() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"Новый текст"}"#,
        )
        .expect("update_text");
        assert_eq!(scene.canvas.nodes[0].text.as_deref(), Some("Новый текст"));

        dispatch(
            &mut scene,
            &mut camera,
            "node_move",
            r#"{"id":"n1","x":-50.0,"y":42.0}"#,
        )
        .expect("move");
        assert_eq!(
            (scene.canvas.nodes[0].x, scene.canvas.nodes[0].y),
            (-50.0, 42.0)
        );
        let hits = scene.spatial.query_rect([-50.0, 42.0, -40.0, 52.0]);
        assert!(hits.contains(&0), "spatial обновлён после move");

        dispatch(
            &mut scene,
            &mut camera,
            "node_resize",
            r#"{"id":"n1","width":500.0,"height":400.0}"#,
        )
        .expect("resize");
        assert_eq!(
            (scene.canvas.nodes[0].width, scene.canvas.nodes[0].height),
            (500.0, 400.0)
        );

        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_move",
            r#"{"id":"ghost","x":0.0,"y":0.0}"#
        )
        .is_err());
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_resize",
            r#"{"id":"n1","width":1.0}"#
        )
        .is_err());
    }

    /// FR-005 node_edit: обновляются ТОЛЬКО переданные поля; label/color
    /// null — сброс; геометрия — с обновлением spatial; ответ — сводка.
    #[test]
    fn mcp_node_edit_updates_only_given_fields() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Только text: координаты/размеры/подпись не тронуты
        let summary = dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","text":"Отредактировано"}"#,
        )
        .expect("node_edit text");
        assert_eq!(summary["id"], "n1");
        assert_eq!(summary["text"], "Отредактировано");
        assert_eq!(
            (scene.canvas.nodes[0].x, scene.canvas.nodes[0].y),
            (100.0, 100.0)
        );
        assert_eq!(
            (scene.canvas.nodes[0].width, scene.canvas.nodes[0].height),
            (260.0, 120.0)
        );
        assert_eq!(scene.canvas.nodes[0].label, None);
        // Только геометрия: text не тронут
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","x":500.0,"y":600.0,"width":300.0,"height":200.0}"#,
        )
        .expect("node_edit geometry");
        assert_eq!(
            scene.canvas.nodes[0].text.as_deref(),
            Some("Отредактировано")
        );
        let hits = scene.spatial.query_rect([500.0, 600.0, 510.0, 610.0]);
        assert!(hits.contains(&0), "spatial обновлён после node_edit");
        // label: строка — задан, null — сброс
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"g1","label":"Моя зона"}"#,
        )
        .expect("node_edit label");
        assert_eq!(scene.canvas.nodes[2].label.as_deref(), Some("Моя зона"));
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"g1","label":null}"#,
        )
        .expect("node_edit label null");
        assert_eq!(scene.canvas.nodes[2].label, None);
        // color: пресет и сброс
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","color":"3"}"#,
        )
        .expect("node_edit color");
        assert_eq!(scene.canvas.nodes[0].color.as_deref(), Some("3"));
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","color":null}"#,
        )
        .expect("node_edit color null");
        assert_eq!(scene.canvas.nodes[0].color, None);
    }

    /// FR-005 node_edit: валидация — несуществующая нода, плохой color,
    /// неположительные размеры, label не-строкой; сцена при ошибках
    /// не меняется.
    #[test]
    fn mcp_node_edit_validation() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        assert!(dispatch(&mut scene, &mut camera, "node_edit", r#"{"id":"ghost"}"#).is_err());
        let before = scene.canvas.nodes[0].clone();
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","color":"9"}"#
        )
        .is_err());
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","width":-5.0}"#
        )
        .is_err());
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","height":0.0}"#
        )
        .is_err());
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","label":42}"#
        )
        .is_err());
        assert_eq!(scene.canvas.nodes[0], before, "ошибки не меняют ноду");
        // Пустой вызов (только id) — валиден: ничего не изменилось, но
        // сводка возвращена (дешёвая «проверка связи»)
        let summary =
            dispatch(&mut scene, &mut camera, "node_edit", r#"{"id":"n1"}"#).expect("no-op");
        assert_eq!(summary["id"], "n1");
        assert_eq!(scene.canvas.nodes[0], before);
    }

    /// node_delete: каскад связей, spatial перестроен, дети группы живы.
    #[test]
    fn mcp_node_delete_cascades_edges() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // n1 связана с f1 — удаление n1 рвёт edge-1; дети группы g1 остаются
        dispatch(&mut scene, &mut camera, "node_delete", r#"{"id":"n1"}"#).expect("delete");
        assert_eq!(scene.canvas.nodes.len(), 2);
        assert!(scene.canvas.edges.is_empty(), "связь каскадно удалена");
        assert!(scene.canvas.node("g1").is_some(), "группа на месте");
        assert!(scene.canvas.node("f1").is_some(), "дети не удалены");
        // spatial консистентен с моделью: индексы пересчитаны (f1=0, g1=1)
        assert_eq!(
            scene
                .spatial
                .query_rect([-1000.0, -1000.0, 1000.0, 1000.0])
                .len(),
            2
        );
        // бывшее место n1 теперь покрывает группа (дети остались внутри)
        assert_eq!(scene.spatial.hit_test([110.0, 110.0]), Some(1));
        assert!(
            dispatch(&mut scene, &mut camera, "node_delete", r#"{"id":"n1"}"#).is_err(),
            "повторное удаление — ошибка"
        );
    }

    /// node_set_color: пресет, сброс null, отказ на мусоре.
    #[test]
    fn mcp_node_set_color_validation() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_set_color",
            r#"{"id":"n1","color":"4"}"#,
        )
        .expect("set_color");
        assert_eq!(scene.canvas.nodes[0].color.as_deref(), Some("4"));
        dispatch(
            &mut scene,
            &mut camera,
            "node_set_color",
            r#"{"id":"n1","color":null}"#,
        )
        .expect("сброс цвета");
        assert_eq!(scene.canvas.nodes[0].color, None);
        let err = dispatch(
            &mut scene,
            &mut camera,
            "node_set_color",
            r#"{"id":"n1","color":"red"}"#,
        )
        .expect_err("не пресет");
        assert!(err.contains("\"1\"..\"6\""));
    }

    /// edge_create: id вида edge-N, стороны any→None / явные, валидация нод
    /// и сторон; edge_delete по id и ошибка на отсутствующую связь.
    #[test]
    fn mcp_edge_create_delete() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // дефолт any → стороны не заданы
        let created = dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            r#"{"from":"n1","to":"g1"}"#,
        )
        .expect("edge_create");
        assert_eq!(created["id"], "edge-2");
        let edge = scene.canvas.edges.last().expect("связь");
        assert_eq!(edge.from_side, None);
        assert_eq!(edge.to_side, None);
        // явные стороны
        let created = dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            r#"{"from":"f1","to":"n1","fromSide":"left","toSide":"bottom"}"#,
        )
        .expect("edge_create со сторонами");
        assert_eq!(created["id"], "edge-3");
        let edge = scene.canvas.edges.last().expect("связь");
        assert_eq!(edge.from_side, Some(Side::Left));
        assert_eq!(edge.to_side, Some(Side::Bottom));

        // несуществующая нода — ошибка, связь не создана
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            r#"{"from":"n1","to":"ghost"}"#
        )
        .is_err());
        // мусорная сторона — ошибка
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            r#"{"from":"n1","to":"f1","fromSide":"diagonal"}"#
        )
        .is_err());
        assert_eq!(scene.canvas.edges.len(), 3, "валидные связи остались");

        dispatch(&mut scene, &mut camera, "edge_delete", r#"{"id":"edge-1"}"#)
            .expect("edge_delete");
        assert_eq!(scene.canvas.edges.len(), 2);
        assert!(
            dispatch(&mut scene, &mut camera, "edge_delete", r#"{"id":"edge-1"}"#).is_err(),
            "повторное удаление — ошибка"
        );
    }

    /// viewport_get/set: центр и зум, кламп зума камерой.
    #[test]
    fn mcp_viewport_get_set() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let view = dispatch(&mut scene, &mut camera, "viewport_get", "{}").expect("viewport_get");
        assert_eq!(view["x"], 0.0);
        assert_eq!(view["zoom"], 1.0);

        let view = dispatch(
            &mut scene,
            &mut camera,
            "viewport_set",
            r#"{"x":100.0,"y":-50.0,"zoom":2.5}"#,
        )
        .expect("viewport_set");
        assert_eq!(view["x"].as_f64().expect("x"), 100.0);
        assert_eq!(view["y"].as_f64().expect("y"), -50.0);
        assert_eq!(view["zoom"], 2.5);
        // зум клампится
        let view = dispatch(
            &mut scene,
            &mut camera,
            "viewport_set",
            r#"{"x":0.0,"y":0.0,"zoom":100.0}"#,
        )
        .expect("viewport_set зум");
        assert_eq!(view["zoom"], canvas_render::camera::MAX_ZOOM);
        // zoom опционален
        let view = dispatch(
            &mut scene,
            &mut camera,
            "viewport_set",
            r#"{"x":1.0,"y":2.0}"#,
        )
        .expect("viewport_set без зума");
        assert_eq!(
            view["zoom"],
            canvas_render::camera::MAX_ZOOM,
            "зум не задет"
        );
    }

    /// mcp_unwrap_call: tools/call → (name, arguments); прочие методы как есть.
    #[test]
    fn mcp_unwrap_call_passthrough_and_tool_name() {
        let params: serde_json::Value =
            serde_json::json!({"name": "node_move", "arguments": {"id": "n1", "x": 1.0, "y": 2.0}});
        let (method, args) = mcp_unwrap_call("tools/call", &params);
        assert_eq!(method, "node_move");
        assert_eq!(args, serde_json::json!({"id": "n1", "x": 1.0, "y": 2.0}));

        // метод напрямую (тесты dispatch) — без изменений
        let params = serde_json::json!({"id": "n1"});
        let (method, args) = mcp_unwrap_call("node_get", &params);
        assert_eq!(method, "node_get");
        assert_eq!(args, params);

        // arguments отсутствует → Null, не паника
        let params = serde_json::json!({"name": "canvas_info"});
        let (method, args) = mcp_unwrap_call("tools/call", &params);
        assert_eq!(method, "canvas_info");
        assert_eq!(args, serde_json::Value::Null);
    }

    /// Неизвестный инструмент и кривые параметры — Err (посредник сделает isError).
    #[test]
    fn mcp_unknown_tool_and_bad_params() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        assert!(dispatch(&mut scene, &mut camera, "canvas_destroy", "{}").is_err());
        assert!(dispatch(&mut scene, &mut camera, "node_get", "{}").is_err());
        assert!(dispatch(&mut scene, &mut camera, "nodes_search", r#"{"query":42}"#).is_err());
    }

    // --- FR-006: undo/redo ---

    /// Лимит истории — ровно 50 (запрос «не менее 50»): 55 шагов → 50,
    /// старейший вытеснен, 50-й отменяем.
    #[test]
    fn undo_stack_limit_is_fifty() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        for i in 0..55 {
            dispatch(
                &mut scene,
                &mut camera,
                "node_create_note",
                &format!(r#"{{"x": {i}.0, "y": 0.0}}"#),
            )
            .expect("node_create_note");
        }
        assert_eq!(scene.undo_stack.len(), 55.min(UNDO_LIMIT));
        assert_eq!(scene.undo_stack.len(), 50, "глубина ровно 50");
        // 55 созданий, отменяем 50: первые 5 созданий вне истории
        // (вытеснены) — в сцене 3 исходных + 5 = 8 нод
        for _ in 0..50 {
            let Some(before) = scene.take_undo() else {
                panic!("история не должна кончиться раньше 50 шагов");
            };
            scene.canvas = before;
            scene.spatial = SpatialIndex::build(&scene.canvas);
        }
        assert_eq!(
            scene.canvas.nodes.len(),
            8,
            "3 исходных + 5 вытеснённых из истории созданий"
        );
    }

    /// Удаление ноды через MCP → undo восстанавливает ноду И каскадную
    /// связь; redo возвращает удаление.
    #[test]
    fn mcp_delete_undo_redo_roundtrip() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let before = scene.canvas.clone();
        dispatch(&mut scene, &mut camera, "node_delete", r#"{"id":"n1"}"#).expect("node_delete");
        // n1 удалена, edge-1 оборвана каскадом
        assert_eq!(scene.canvas.nodes.len(), 2);
        assert!(scene.canvas.edges.is_empty());
        // undo: сцена «до» возвращается целиком
        let snapshot = scene.take_undo().expect("шаг undo есть");
        assert_eq!(snapshot, before, "снапшот — состояние до удаления");
        scene.canvas = snapshot;
        scene.spatial = SpatialIndex::build(&scene.canvas);
        assert_eq!(scene.canvas.nodes.len(), 3);
        assert_eq!(scene.canvas.edges.len(), 1);
        // redo: удаление возвращается
        let after = scene.take_redo().expect("шаг redo есть");
        assert_eq!(after.nodes.len(), 2);
        assert!(after.edges.is_empty());
    }

    /// push нового шага обнуляет ветку redo (стандарт undo-модели).
    #[test]
    fn new_action_clears_redo_branch() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_move",
            r#"{"id":"n1","x":10.0,"y":10.0}"#,
        )
        .expect("node_move");
        let _ = scene.take_undo().expect("undo доступен");
        assert_eq!(scene.redo_stack.len(), 1);
        // новое действие после undo — redo ветка сброшена
        dispatch(
            &mut scene,
            &mut camera,
            "node_set_color",
            r#"{"id":"n1","color":"3"}"#,
        )
        .expect("node_set_color");
        assert!(scene.redo_stack.is_empty(), "redo обнулён новым шагом");
        assert_eq!(scene.undo_stack.len(), 1, "в истории только новый шаг");
    }

    /// Валидационные ошибки MCP не оставляют пустых шагов: node_get /
    /// неизвестный id / кривой color — история пуста.
    #[test]
    fn mcp_validation_errors_leave_no_steps() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // чтение — не мутация
        dispatch(&mut scene, &mut camera, "nodes_list", "{}").expect("nodes_list");
        assert!(scene.undo_stack.is_empty(), "чтение не шаг");
        // несуществующий id — Err до мутации, шага нет
        assert!(dispatch(&mut scene, &mut camera, "node_delete", r#"{"id":"нет"}"#).is_err());
        assert!(scene.undo_stack.is_empty(), "ошибка валидации не шаг");
        // node_edit с невалидным width — Err
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","width":-5.0}"#
        )
        .is_err());
        assert_eq!(scene.undo_stack.len(), 1, "node_edit пушит до мутаций");
        // этот шаг откатывает частично применённые поля (text/label)
        let snapshot = scene.take_undo().expect("шаг есть");
        assert_eq!(snapshot, mcp_scene().canvas, "снапшот — исходная сцена");
    }

    /// edge_delete отсутствующей связи — Err без шага (сравнение после).
    #[test]
    fn mcp_edge_delete_missing_no_step() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        assert!(dispatch(&mut scene, &mut camera, "edge_delete", r#"{"id":"нет"}"#).is_err());
        assert!(scene.undo_stack.is_empty(), "no-op удаления — не шаг");
        // существующая связь — шаг есть
        dispatch(&mut scene, &mut camera, "edge_delete", r#"{"id":"edge-1"}"#)
            .expect("edge_delete");
        assert_eq!(scene.undo_stack.len(), 1);
    }

    // --- FR-013: Numi-формулы (canvasdesk.expr) ---

    /// Смешанный редактор: строки «= …» — формула без префикса; пустые
    /// формульные строки пропускаются; без «=» — None.
    #[test]
    fn split_formula_lines_extracts_equal_prefixed() {
        assert_eq!(split_formula_lines("= 5 ms"), Some("5 ms".to_owned()));
        assert_eq!(split_formula_lines("=  5 ms  "), Some("5 ms".to_owned()));
        assert_eq!(
            split_formula_lines("Gateway\n= rps = 1000\n= latency = 50 ms"),
            Some("rps = 1000\nlatency = 50 ms".to_owned()),
            "несколько утверждений соединяются переводом строки"
        );
        // Пустая формула («=» без содержимого) — не утверждение
        assert_eq!(split_formula_lines("=\n= 5 ms"), Some("5 ms".to_owned()));
        // Нет формульных строк — None
        assert_eq!(split_formula_lines("Просто текст"), None);
        assert_eq!(split_formula_lines(""), None);
        // «=» внутри строки не формула — только начало строки
        assert_eq!(split_formula_lines("a = b"), None);
    }

    /// FR-018: параметры шаблона из текста ноды — присваивания вычисляются
    /// Numi-eval'ом (единицы, суффиксы `2k`); проза и пустые строки
    /// пропускаются.
    #[test]
    fn template_params_from_text_parses_assignments() {
        let params = canvas_core::templates::params_from_text(
            "rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2k\nКомментарий-проза\n\nempty = ",
        );
        assert_eq!(params.len(), 3, "проза/пустые — мимо");
        assert_eq!(params["rps"].num, 1000.0);
        assert_eq!(params["rps"].unit.as_deref(), Some("rps"));
        assert_eq!(params["service_rate"].num, 1200.0);
        assert_eq!(params["servers"].num, 2000.0, "суффикс k разворачивается");
        assert_eq!(params["servers"].unit, None);
        // Скаляр без единицы
        let scalar = canvas_core::templates::params_from_text("k = 3");
        assert_eq!(scalar["k"].num, 3.0);
        assert_eq!(scalar["k"].unit, None);
        // Проза с «=» не парсится в значение — мимо
        let prose = canvas_core::templates::params_from_text("Server load = high");
        assert!(prose.is_empty(), "не-Numi-значение пропущено");
    }

    /// FR-020: slug имени шаблона — латиница/цифры/дефисы; кириллица
    /// транслитерируется («Нагрузка» → «nagruzka»).
    #[test]
    fn slugify_transliterates_cyrillic() {
        assert_eq!(slugify("My Custom LB"), "my-custom-lb");
        assert_eq!(slugify("Нагрузка"), "nagruzka");
        assert_eq!(slugify("БД SQL (мастер)"), "bd-sql-master");
        assert_eq!(slugify("  --weird name--  "), "weird-name");
        assert_eq!(slugify("!!!"), "custom", "нет символов — фолбэк custom");
    }

    /// FR-020: тип параметра по токену единицы.
    #[test]
    fn infer_param_type_maps_units() {
        use canvas_core::templates::ParamType;
        assert_eq!(infer_param_type(Some("rps")), ParamType::Rate);
        assert_eq!(infer_param_type(Some("ms")), ParamType::Time);
        assert_eq!(infer_param_type(Some("KB")), ParamType::Bytes);
        assert_eq!(infer_param_type(Some("req")), ParamType::Count);
        assert_eq!(infer_param_type(Some("%")), ParamType::Percent);
        assert_eq!(infer_param_type(None), ParamType::Scalar);
        assert_eq!(infer_param_type(Some("unknown")), ParamType::Scalar);
    }

    /// FR-020: уникальный id — без конфликтов; конфликт получает суффикс -2.
    #[test]
    fn unique_custom_id_avoids_conflicts() {
        let registry = canvas_core::templates::TemplateRegistry::empty();
        let root = std::env::temp_dir().join(format!(
            "canvasdesk-fr20-uid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("root");
        let first = unique_custom_id("my-lb", &registry, &root);
        assert_eq!(first, "my-lb");
        // «Занято» в реестре → суффикс
        let manifest = canvas_core::templates::TemplateManifest {
            id: "my-lb".to_owned(),
            name: "x".to_owned(),
            name_ru: None,
            version: "1.0.0".to_owned(),
            category: "custom".to_owned(),
            description: String::new(),
            description_en: None,
            params: Vec::new(),
            expr: "1".to_owned(),
            color: "#9B9B9B".to_owned(),
            icon: "custom".to_owned(),
            source: canvas_core::templates::TemplateSource::Custom,
        };
        canvas_core::templates::save_custom(&manifest, &root).expect("save");
        let registry = canvas_core::templates::TemplateRegistry::all_with_custom(&root);
        let second = unique_custom_id("my-lb", &registry, &root);
        assert_eq!(second, "my-lb-2");
        std::fs::remove_dir_all(&root).ok();
    }

    /// MCP node_edit { expr } — формула сохранена, результат пересчитан
    /// в expr_results (инвариант 4: runtime, не в .canvas).
    #[test]
    fn mcp_node_edit_expr_computes_result() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let summary = dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":"5 ms × 200 req/s"}"#,
        )
        .expect("node_edit expr");
        assert_eq!(summary["expr"], "5 ms × 200 req/s", "формула в сводке");
        // Результат — runtime-кэш, в модели его нет
        let value = scene.expr_results.get("n1").expect("результат есть");
        match value {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000 ms·req/s"),
            other => panic!("ожидался результат, получено: {other:?}"),
        }
        let json = scene.canvas.to_json().expect("сериализация");
        assert!(!json.contains("1000 ms"), "результат не сериализуется");
        assert!(json.contains("5 ms × 200 req/s"), "формула сериализуется");
    }

    /// MCP node_edit { expr: null } — сброс calc-режима; невалидная
    /// формула — Err с диагностикой, нода не менялась и шага undo нет
    /// (валидация ДО push_undo).
    #[test]
    fn mcp_node_edit_expr_null_and_invalid() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Сброс отсутствующей формулы — no-op без ошибки
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":null}"#,
        )
        .expect("expr null");
        assert!(!scene.expr_results.contains_key("n1"));

        // Установка, затем сброс через null
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":"1k rps"}"#,
        )
        .expect("expr set");
        assert!(scene.expr_results.contains_key("n1"));
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":null}"#,
        )
        .expect("expr reset");
        assert!(
            scene.canvas.node("n1").and_then(Node::expr).is_none(),
            "формула удалена из модели"
        );
        assert!(!scene.expr_results.contains_key("n1"), "результат удалён");

        // Невалидная формула: Err, нода не тронута, undo-шага нет
        let steps_before = scene.undo_stack.len();
        let err = dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":"= invalid @#$"}"#,
        )
        .expect_err("парсинг формулы");
        assert!(err.contains("expr"), "диагностика с префиксом поля: {err}");
        assert_eq!(
            scene.undo_stack.len(),
            steps_before,
            "невалидный expr не пушит шаг"
        );
        // Кривой тип expr — тоже Err
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":42}"#
        )
        .is_err());
    }

    /// Формула с ошибкой вычисления сохраняется (парсинг ок), результат —
    /// красная диагностика в expr_results (MCP отвергает только синтаксис).
    #[test]
    fn mcp_node_edit_expr_eval_error_is_stored() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":"5 ms + 3 rps"}"#,
        )
        .expect("синтаксически корректная формула сохраняется");
        match scene.expr_results.get("n1").expect("запись есть") {
            ExprOutcome::Err(msg) => {
                assert!(msg.contains("не совместимы"), "диагностика: {msg}")
            }
            other => panic!("ожидалась ошибка вычисления: {other:?}"),
        }
    }

    /// Интеграционный сценарий верификации FR-013: node_update_text со
    /// строками «= …» выводит формулу; построчный результат — Numi-стиль
    /// (строки сценария); undo восстанавливает пустой expr и убирает
    /// результаты; загрузка с canvasdesk.expr без формульных строк в
    /// тексте — программный итог в футере.
    #[test]
    fn expr_undo_redo_restores_formula() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Заметка с формулой: текст с «=»-строками → формула + построчный
        // результат (Numi-стиль)
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"Параметры\n= 1 sec + 500 ms"}"#,
        )
        .expect("node_update_text");
        assert_eq!(
            scene.canvas.node("n1").and_then(Node::expr),
            Some("1 sec + 500 ms"),
            "формула выведена из текста"
        );
        // FR-014: expr_results — карта потока значений (запись есть для
        // любой expr-ноды); вытеснение футера построчными результатами —
        // правило РЕНДЕРА (text.rs), а не отсутствие записи
        assert!(
            scene.expr_results.contains_key("n1"),
            "результат формулы в карте потока"
        );
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты есть");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], None, "проза без результата");
        match lines[1].as_ref().expect("результат «=»-строки") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1.5 sec"),
            other => panic!("ожидалось значение: {other:?}"),
        }

        // Undo: expr пуст, результатов нет
        let before = scene.take_undo().expect("шаг есть");
        scene.canvas = before;
        scene.recompute_all_expr();
        assert_eq!(
            scene.canvas.node("n1").and_then(Node::expr),
            None,
            "после undo формула из правки исчезла"
        );
        assert!(!scene.expr_results.contains_key("n1"), "итога нет");
        assert!(
            !scene.expr_line_results.contains_key("n1"),
            "построчных результатов нет"
        );

        // Загрузка с формулой (текст без формульных строк): recompute_all_expr
        // при SceneState::new даёт программный итог в футере
        let mut canvas = Canvas::default();
        let mut note = Node::text("calc", "Gateway", 0.0, 0.0);
        note.set_expr(Some("1k rps".to_owned()));
        canvas.nodes.push(note);
        let scene = SceneState::new(canvas, PathBuf::from("target/tmp/expr.canvas"));
        match scene
            .expr_results
            .get("calc")
            .expect("результат при загрузке")
        {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000 rps"),
            other => panic!("ожидалось значение: {other:?}"),
        }
    }

    /// FR-013 (правка 2, Numi-стиль): каждая формульная строка текста —
    /// свой результат; переменные протекают между строками; проза и
    /// пустые строки без результата; canvasdesk.expr не материализуется.
    /// FR-014: n1 входит в карту потока (expr_results — итог = последняя
    /// формульная строка), но футер на карточке не рисуется — построчные
    /// результаты вытесняют программный итог (правило рендера).
    #[test]
    fn per_line_results_numi_sheet() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"Gateway\nrps = 1000\n\nlatency = 50 ms\nlatency × rps"}"#,
        )
        .expect("node_update_text");
        assert_eq!(
            scene.canvas.node("n1").and_then(Node::expr),
            None,
            "авто-формулы не материализуются в canvasdesk.expr"
        );
        // FR-014: значение ноды в карте потока есть (последняя формульная
        // строка «latency × rps»); показ футера гасится построчными
        // результатами — правило рендера, не карты
        match scene.expr_results.get("n1").expect("итог в карте потока") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "50000 ms"),
            other => panic!("ожидалось значение: {other:?}"),
        };
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты есть");
        assert_eq!(lines.len(), 5, "Vec выровнен по строкам текста");
        assert_eq!(lines[0], None, "проза");
        match lines[1].as_ref().expect("присваивание — результат") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000"),
            other => panic!("ожидалось значение: {other:?}"),
        }
        assert_eq!(lines[2], None, "пустая строка");
        match lines[4]
            .as_ref()
            .expect("выражение с переменными — результат")
        {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "50000 ms"),
            other => panic!("ожидалось значение: {other:?}"),
        }
    }

    /// MCP node_edit { expr } при тексте без формульных строк — программный
    /// итог в expr_results (футер карточки), построчных результатов нет.
    #[test]
    fn mcp_program_result_fallback_for_prose_text() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            r#"{"id":"n1","expr":"5 ms × 200 req/s"}"#,
        )
        .expect("node_edit expr");
        assert!(
            !scene.expr_line_results.contains_key("n1"),
            "в прозе формульных строк нет — построчных результатов нет"
        );
        match scene.expr_results.get("n1").expect("программный итог") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000 ms·req/s"),
            other => panic!("ожидалось значение: {other:?}"),
        }
    }

    /// FR-013 (правка 2): выражения без «=» считаются построчно при
    /// node_update_text и при загрузке канваса; проза результата не создаёт.
    #[test]
    fn auto_lines_compute_without_equal_prefix() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Явной формулы нет, последняя строка — выражение: результат на ней
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"Пропускная способность\n1000 rps * 2"}"#,
        )
        .expect("node_update_text");
        assert_eq!(
            scene.canvas.node("n1").and_then(Node::expr),
            None,
            "авто-формула не материализуется в canvasdesk.expr"
        );
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты есть");
        assert_eq!(lines[0], None, "проза — не формула");
        match lines[1].as_ref().expect("результат авто-строки") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "2000 rps"),
            other => panic!("ожидалось значение: {other:?}"),
        }

        // Проза: цифры в строке есть, но выражение не парсится — результата нет
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"План на 15:00"}"#,
        )
        .expect("node_update_text проза");
        assert!(
            !scene.expr_line_results.contains_key("n1"),
            "проза — не формула"
        );
        assert!(!scene.expr_results.contains_key("n1"), "итога тоже нет");

        // Загрузка канваса с авто-формулой: результат пересчитывается
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("auto", "5 ms × 200 req/s", 0.0, 0.0));
        let scene = SceneState::new(canvas, PathBuf::from("target/tmp/auto.canvas"));
        let lines = scene
            .expr_line_results
            .get("auto")
            .expect("построчные результаты при загрузке");
        match lines[0].as_ref().expect("результат") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000 ms·req/s"),
            other => panic!("ожидалось значение: {other:?}"),
        }
    }

    /// FR-013 (правка 4): ТОЧНЫЕ листы владельца из фидбека — присваивания
    /// (обе формы), ссылки на переменные, кумулятивное окружение. Каждая
    /// строка показывает результат, присваивания наполняют Env.
    #[test]
    fn per_line_results_owner_sheets_v4() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Нода 1 владельца
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"123 + 5123 = a\n235 + 2323 = b\nx = 200\nc = a + b\n200 + x"}"#,
        )
        .expect("node_update_text нода 1");
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты ноды 1");
        assert_eq!(lines.len(), 5);
        let texts: Vec<String> = lines
            .iter()
            .map(|line| match line {
                Some(ExprOutcome::Ok(value)) => value.to_string(),
                other => panic!("ожидалось значение: {other:?}"),
            })
            .collect();
        assert_eq!(texts[0], "5246", "хвостовое присваивание a");
        assert_eq!(texts[1], "2558", "хвостовое присваивание b");
        assert_eq!(texts[2], "200", "чистое присваивание x");
        assert_eq!(texts[3], "7804", "ссылки на переменные c = a + b");
        assert_eq!(texts[4], "400", "ссылка на x");

        // Нода 2 владельца
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"x = 200\n250 + x"}"#,
        )
        .expect("node_update_text нода 2");
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты ноды 2");
        assert_eq!(lines.len(), 2);
        match lines[0].as_ref().expect("x = 200") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "200"),
            other => panic!("ожидалось значение: {other:?}"),
        }
        match lines[1].as_ref().expect("250 + x") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "450"),
            other => panic!("ожидалось значение: {other:?}"),
        }
    }

    /// FR-013 (правка 4): ошибки строк доходят до expr_line_results как
    /// ExprOutcome::Err (для красного бейджа и тултипа); ссылки на
    /// объявленную, но не вычислившуюся переменную тоже помечены.
    #[test]
    fn per_line_error_outcomes_reach_scene() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_update_text",
            r#"{"id":"n1","text":"x = 1 sec + 2 req\nx + 1\nитог = 2 + 2"}"#,
        )
        .expect("node_update_text ошибки");
        let lines = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты есть");
        assert_eq!(lines.len(), 3);
        assert!(
            matches!(&lines[0], Some(ExprOutcome::Err(_))),
            "несовместимость единиц видна"
        );
        assert!(
            matches!(&lines[1], Some(ExprOutcome::Err(_))),
            "ссылка на объявленную, но не вычисленную x видна"
        );
        assert!(
            matches!(&lines[2], Some(ExprOutcome::Ok(_))),
            "строка ниже по-прежнему вычисляется"
        );
    }

    /// FR-013 (правка 5): РЕАЛЬНЫЙ флоу набора — EditingSession (как
    /// begin_editing), ввод через insert_text (путь вставки/набора), живые
    /// результаты — ТОЧНО та же формула, что в RedrawRequested; затем
    /// commit (текст сессии в модель + recompute_expr). Регресс корневого
    /// бага «выражения так ничего и не показывают»: emit экранировал
    /// одиночное `=` (`x = 200` → `x \= 200`), expr-парсер падал на `\`,
    /// переменные не объявлялись — лист владельца молчал ЦЕЛИКОМ и в
    /// редакторе, и на карточке. MCP-тесты (node_update_text) этого не
    /// ловили: они кладут в модель чистый текст, минуя markdown-канонику.
    #[test]
    fn live_typing_flow_owner_sheets() {
        use cosmic_text::FontSystem;
        fn run_session(text: &str) -> (String, Vec<Option<ExprOutcome>>) {
            let mut fs = FontSystem::new();
            let mut session =
                EditingSession::new(&mut fs, EditTarget::Node(0), "", 360.0, 228.0, 1.0);
            session.insert_text(&mut fs, text);
            let canonical = session.text();
            // Та же формула, что в RedrawRequested для живых результатов
            let live = expr::eval_lines(&canonical);
            (canonical, live)
        }
        fn expect_ok(line: &Option<ExprOutcome>, expected: &str, context: &str) {
            match line {
                Some(ExprOutcome::Ok(value)) => {
                    assert_eq!(value.to_string(), expected, "{context}")
                }
                other => panic!("{context}: ожидалось {expected}, получено {other:?}"),
            }
        }
        fn expect_err(line: &Option<ExprOutcome>, name: &str, context: &str) {
            match line {
                Some(ExprOutcome::Err(msg)) => {
                    assert!(msg.contains(name), "{context}: имя {name} в ошибке: {msg}")
                }
                other => panic!("{context}: ожидалась ошибка про {name}: {other:?}"),
            }
        }

        // Лист 1 владельца (латиница): c = a + b — ссылки на необъявленные
        // a/b — ВИДИМАЯ ошибка строки (правка 5), остальные — значения
        let (canonical, live) = run_session("x = 200\nc = a + b\n200 + x");
        assert_eq!(canonical, "x = 200\nc = a + b\n200 + x", "emit без \\=");
        expect_ok(&live[0], "200", "присваивание x");
        expect_err(&live[1], "a", "необъявленная a в присваивании");
        expect_ok(&live[2], "400", "ссылка на x");

        // Та же раскладка кириллицей (русская раскладка владельца)
        let (_, live) = run_session("х = 200\nс = а + б\n200 + х");
        expect_ok(&live[0], "200", "кириллическое присваивание");
        expect_err(&live[1], "а", "необъявленная а");
        expect_ok(&live[2], "400", "ссылка на х");

        // Лист 2 владельца: x-умножение и ссылки
        let (canonical, live) = run_session("a=25+35x20\nb = 2\na+b");
        assert_eq!(canonical, "a=25+35x20\nb = 2\na+b");
        expect_ok(&live[0], "725", "a = 25+35x20");
        expect_ok(&live[1], "2", "b = 2");
        expect_ok(&live[2], "727", "a+b");

        // COMMIT: канонический текст сессии попадает в модель, построчные
        // результаты совпадают с живыми (карточка после клика мимо ноды)
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("n1", "", 0.0, 0.0));
        let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/live-flow.canvas"));
        let (canonical, live) =
            run_session("123 + 5123 = a\n235 + 2323 = b\nx = 200\nc = a + b\n200 + x");
        scene.canvas.nodes[0].text = Some(canonical.clone());
        scene.recompute_expr("n1");
        let committed = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты после commit");
        assert_eq!(committed.len(), live.len());
        expect_ok(&committed[0], "5246", "commit: хвостовое присваивание");
        expect_ok(&committed[1], "2558", "commit: b");
        expect_ok(&committed[2], "200", "commit: x");
        expect_ok(&committed[3], "7804", "commit: c = a + b");
        expect_ok(&committed[4], "400", "commit: 200 + x");
        // Совместимость: заметка прежних сборок с `\=` в модели оживает
        scene.canvas.nodes[0].text = Some("x \\= 200\n200 + x".to_owned());
        scene.recompute_expr("n1");
        let committed = scene
            .expr_line_results
            .get("n1")
            .expect("результаты для старой каноники");
        expect_ok(&committed[0], "200", "старая заметка: присваивание с \\=");
        expect_ok(&committed[1], "400", "старая заметка: ссылка");
    }

    /// FR-013 (правка 4): hit-тест зон наведения бейджей ошибок.
    #[test]
    fn expr_error_tooltip_hit_test() {
        let hits = vec![
            LineErrorHit {
                rect: [360.0, 34.0, 25.0, 36.0],
                message: "единицы не совместимы: 1 sec и 2 req".to_owned(),
            },
            LineErrorHit {
                rect: [360.0, 54.0, 25.0, 36.0],
                message: "деление на ноль".to_owned(),
            },
        ];
        // Внутри первой зоны
        assert_eq!(
            expr_error_hit_at(&hits, [372.0, 40.0]).map(|hit| &*hit.message),
            Some("единицы не совместимы: 1 sec и 2 req"),
        );
        // Внутри второй зоны (первая кончается на y=70 — берём точку ниже)
        assert_eq!(
            expr_error_hit_at(&hits, [378.0, 80.0]).map(|hit| &*hit.message),
            Some("деление на ноль"),
        );
        // Мимо всех зон
        assert!(
            expr_error_hit_at(&hits, [100.0, 40.0]).is_none(),
            "мимо по x"
        );
        assert!(
            expr_error_hit_at(&hits, [372.0, 200.0]).is_none(),
            "мимо по y"
        );
        // Пустой набор зон
        assert!(expr_error_hit_at(&[], [372.0, 40.0]).is_none());
    }

    // --- FR-014: поток значений по рёбрам ---

    /// Сцена потока: A «1200 + 480», B «$in / 3», ребро A→B (control).
    fn flow_scene() -> SceneState {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("fa", "A\n1200 + 480", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("fb", "B\n$in / 3", 500.0, 0.0));
        canvas.add_edge(Edge::new("e-ab", "fa", None, "fb", Some(Side::Left)));
        SceneState::new(canvas, PathBuf::from("target/tmp/flow.canvas"))
    }

    /// Правка 3 (регрессия UI-переключателя): тогл через палитру
    /// (SceneState::toggle_edge_flow) применяет flow.kind к ЖИВОМУ канвасу
    /// (раньше мутировался клон-снимок — переключатель не работал),
    /// копит undo-шаг «до» и пересчитывает downstream.
    #[test]
    fn palette_flow_toggle_applies_to_live_canvas() {
        let mut scene = flow_scene();
        // Тогл Control → Value применён к живому канвасу
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("тогл без цикла");
        assert!(applied, "тогл должен примениться");
        assert_eq!(scene.canvas.edges[0].flow_kind(), FlowKind::Value);
        // Живой пересчёт: B = 1680 / 3 = 560 (вход пришёл по value-ребру)
        let b = scene
            .expr_results
            .get("fb")
            .expect("результат B после тогла");
        match b {
            ExprOutcome::Ok(value) => assert!((value.num - 560.0).abs() < 1e-9, "{value:?}"),
            ExprOutcome::Err(err) => panic!("B не должен иметь ошибку: {err}"),
        }
        // Undo-шаг «до» тогла (FR-006): снят ДО мутации, а не после
        assert_eq!(scene.undo_stack.len(), 1, "один undo-шаг");
        let before = scene.undo_stack[0].clone();
        assert_eq!(
            before.edges[0].flow_kind(),
            FlowKind::Control,
            "снимок «до» — control"
        );
        // Round-trip в файл: kind=value сохраняется
        let json = scene.canvas.to_json().expect("сериализация");
        assert!(json.contains("\"value\""), "flow.kind в файле: {json}");
    }

    /// Правка 3: обратный тогл Value → Control — поле удаляется целиком,
    /// downstream теряет вход (MissingInbound), undo-шаг копится.
    #[test]
    fn palette_flow_toggle_back_removes_field_and_breaks_input() {
        let mut scene = flow_scene();
        scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("первый тогл");
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Control)
            .expect("обратный тогл");
        assert!(applied);
        assert_eq!(scene.canvas.edges[0].flow_kind(), FlowKind::Control);
        assert!(
            scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
            "control удаляет расширение целиком (как MCP)"
        );
        // Downstream потерял вход — у B ошибка MissingInbound
        match scene.expr_results.get("fb") {
            Some(ExprOutcome::Err(msg)) => {
                assert!(msg.contains("вход"), "ожидался missing inbound: {msg}");
            }
            other => panic!("ожидалась ошибка входа у B, получено: {other:?}"),
        }
        assert_eq!(scene.undo_stack.len(), 2, "два undo-шага (туда-обратно)");
    }

    /// Правка 3: no-op-варианты не копят шаг и не меняют модель — связи
    /// нет, тип уже такой.
    #[test]
    fn palette_flow_toggle_noop_cases() {
        let mut scene = flow_scene();
        // Несуществующая связь
        let applied = scene
            .toggle_edge_flow(7, FlowKind::Value)
            .expect("нет связи — Ok(false)");
        assert!(!applied);
        assert!(scene.undo_stack.is_empty());
        // Повторный тогл в тот же тип — no-op
        scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("первый тогл");
        let snapshot = scene.canvas.clone();
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("уже value — Ok(false)");
        assert!(!applied);
        assert_eq!(scene.canvas, snapshot, "модель не изменилась");
        assert_eq!(scene.undo_stack.len(), 1, "второй шаг не копится");
    }

    /// Правка 3: тогл в Value, замыкающий цикл, отклонён с участниками
    /// (Err), модель не меняется (DAG-инвариант в UI-пути).
    #[test]
    fn palette_flow_toggle_rejects_cycle() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("fa", "5", 0.0, 0.0));
        canvas.nodes.push(Node::text("fb", "$in", 500.0, 0.0));
        // A → B уже value, обратная связь B → A — control
        canvas.add_edge(Edge::new("e-ab", "fa", None, "fb", Some(Side::Left)));
        canvas.add_edge(Edge::new("e-ba", "fb", None, "fa", Some(Side::Right)));
        canvas.edges[0].set_flow_kind(FlowKind::Value);
        let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/flow-cycle.canvas"));
        // Тогл B → A в value замкнул бы цикл A → B → A
        let result = scene.toggle_edge_flow(1, FlowKind::Value);
        let participants = result.expect_err("цикл должен быть отклонён");
        assert_eq!(participants, vec!["fa".to_owned(), "fb".to_owned()]);
        // Модель не изменилась
        assert_eq!(scene.canvas.edges[1].flow_kind(), FlowKind::Control);
        assert!(scene.undo_stack.is_empty(), "отклонённый тогл без шага");
    }

    /// flow_set_kind: тогл control → value → control; round-trip в extra.
    #[test]
    fn mcp_flow_set_kind_toggles_flow() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let out = dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            r#"{"id":"edge-1","kind":"value"}"#,
        )
        .expect("flow_set_kind value");
        assert_eq!(out["kind"], "value");
        assert_eq!(
            scene.canvas.edges[0].flow_kind(),
            canvas_core::flow::FlowKind::Value
        );
        // Тогл обратно — поле удаляется
        let out = dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            r#"{"id":"edge-1","kind":"control"}"#,
        )
        .expect("flow_set_kind control");
        assert_eq!(out["kind"], "control");
        assert!(
            scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
            "control удаляет расширение целиком"
        );
        // Некорректный kind — ошибка
        let err = dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            r#"{"id":"edge-1","kind":"поток"}"#,
        )
        .expect_err("kind валидируется");
        assert!(err.contains("kind"), "{err}");
    }

    /// flow_set_kind при цикле — isError с участниками (DAG-инвариант MCP).
    #[test]
    fn mcp_flow_set_kind_rejects_cycle() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // A → B → A из новых нод и value-рёбер
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fa","x":0,"y":0,"text":"A"}"#,
        )
        .expect("fa");
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fb","x":300,"y":0,"text":"B"}"#,
        )
        .expect("fb");
        // id создаются автоматически (note-N) — найдём по тексту
        let id_a = scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref() == Some("A"))
            .map(|n| n.id.clone())
            .expect("нода A");
        let id_b = scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref() == Some("B"))
            .map(|n| n.id.clone())
            .expect("нода B");
        let e1 = dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
        )
        .expect("edge A→B")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            &format!(r#"{{"id":"{e1}","kind":"value"}}"#),
        )
        .expect("A→B value");
        let e2 = dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            &format!(r#"{{"from":"{id_b}","to":"{id_a}"}}"#),
        )
        .expect("edge B→A")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        let err = dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            &format!(r#"{{"id":"{e2}","kind":"value"}}"#),
        )
        .expect_err("цикл B→A→B отклонён");
        assert!(err.contains("цикл"), "{err}");
        // А control — пожалуйста
        dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            &format!(r#"{{"id":"{e2}","kind":"control"}}"#),
        )
        .expect("control допустим");
    }

    /// flow_recalc: живой пересчёт цепочки A→B→C; правка формулы A меняет
    /// downstream; удаление ребра — «вход отсутствует».
    #[test]
    fn mcp_flow_recalc_chain_live_reval() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // A=5, B=$in × 2, C=$in + 1
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fa","x":0,"y":0,"text":"A\n= 5"}"#,
        )
        .expect("fa");
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fb","x":300,"y":0,"text":"B\n= $in × 2"}"#,
        )
        .expect("fb");
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fc","x":600,"y":0,"text":"C\n= $in + 1"}"#,
        )
        .expect("fc");
        let id = |scene: &SceneState, text: &str| {
            scene
                .canvas
                .nodes
                .iter()
                .find(|n| {
                    n.text
                        .as_deref()
                        .map(|t| t.starts_with(text))
                        .unwrap_or(false)
                })
                .map(|n| n.id.clone())
                .expect("нода сценария")
        };
        let (id_a, id_b, id_c) = (id(&scene, "A"), id(&scene, "B"), id(&scene, "C"));
        for (from, to) in [(&id_a, &id_b), (&id_b, &id_c)] {
            let edge_id = dispatch(
                &mut scene,
                &mut camera,
                "edge_create",
                &format!(r#"{{"from":"{from}","to":"{to}"}}"#),
            )
            .expect("edge")["id"]
                .as_str()
                .expect("id")
                .to_owned();
            dispatch(
                &mut scene,
                &mut camera,
                "flow_set_kind",
                &format!(r#"{{"id":"{edge_id}","kind":"value"}}"#),
            )
            .expect("value-ребро");
        }
        // Верификация FR-014: {A: 5, B: 10, C: 11}
        let map = dispatch(&mut scene, &mut camera, "flow_recalc", "{}").expect("flow_recalc");
        assert_eq!(map[&id_a]["value"], 5.0);
        assert_eq!(map[&id_b]["value"], 10.0);
        assert_eq!(map[&id_c]["value"], 11.0);
        assert_eq!(map[&id_a]["unit"], "");
        // Правка A → downstream пересчитан: {A: 7, B: 14, C: 15}
        dispatch(
            &mut scene,
            &mut camera,
            "node_edit",
            &format!(r#"{{"id":"{id_a}","expr":"7"}}"#),
        )
        .expect("node_edit expr");
        let map = dispatch(&mut scene, &mut camera, "flow_recalc", "{}").expect("flow_recalc");
        assert_eq!(map[&id_b]["value"], 14.0, "downstream пересчитан живьём");
        assert_eq!(map[&id_c]["value"], 15.0);
        // Удаление value-ребра A→B — у B «вход отсутствует», C тоже
        let e_ab = scene
            .canvas
            .edges
            .iter()
            .find(|e| e.from_node == id_a && e.to_node == id_b)
            .map(|e| e.id.clone())
            .expect("ребро A→B");
        dispatch(
            &mut scene,
            &mut camera,
            "edge_delete",
            &format!(r#"{{"id":"{e_ab}"}}"#),
        )
        .expect("edge_delete");
        let map = dispatch(&mut scene, &mut camera, "flow_recalc", "{}").expect("flow_recalc");
        assert!(map[&id_b]["error"]
            .as_str()
            .expect("ошибка входа")
            .contains("вход"));
        assert!(map[&id_c]["error"].as_str().is_some(), "downstream тоже");
    }

    /// flow_cycle_check: без value-циклов — []; после value-цикла — участники.
    #[test]
    fn mcp_flow_cycle_check_reports_participants() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        assert_eq!(
            dispatch(&mut scene, &mut camera, "flow_cycle_check", "{}").expect("[]"),
            serde_json::json!([])
        );
        // Контрольный цикл (edge-1 n1→f1 + обратный f1→n1) — НЕ значение
        dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            r#"{"from":"f1","to":"n1"}"#,
        )
        .expect("обратное ребро");
        dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            r#"{"id":"edge-1","kind":"value"}"#,
        )
        .expect("прямое value");
        assert_eq!(
            dispatch(&mut scene, &mut camera, "flow_cycle_check", "{}").expect("[]"),
            serde_json::json!([]),
            "одно value-ребро цикла не создаёт"
        );
        // Обратное тоже value — цикл n1→f1→n1. Через MCP такой тогл
        // отклоняется (см. mcp_flow_set_kind_rejects_cycle), поэтому строим
        // чужой-файл сценарий прямой мутацией extra
        let back_index = scene
            .canvas
            .edges
            .iter()
            .position(|e| e.from_node == "f1" && e.to_node == "n1")
            .expect("обратное ребро");
        scene.canvas.edges[back_index].set_flow_kind(canvas_core::flow::FlowKind::Value);
        let participants =
            dispatch(&mut scene, &mut camera, "flow_cycle_check", "{}").expect("участники");
        // Участники отсортированы по id (лексикографически)
        assert_eq!(participants, serde_json::json!(["f1", "n1"]));
    }

    /// FR-014 + FR-006: undo тогла value → control восстанавливает поток
    /// (формула downstream снова получает вход).
    #[test]
    fn mcp_flow_toggle_undo_restores_downstream() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fa","x":0,"y":0,"text":"A\n= 5"}"#,
        )
        .expect("fa");
        dispatch(
            &mut scene,
            &mut camera,
            "node_create_note",
            r#"{"id":"fb","x":300,"y":0,"text":"B\n= $in × 2"}"#,
        )
        .expect("fb");
        let id_a = scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref() == Some("A\n= 5"))
            .map(|n| n.id.clone())
            .expect("A");
        let id_b = scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.text.as_deref() == Some("B\n= $in × 2"))
            .map(|n| n.id.clone())
            .expect("B");
        let e1 = dispatch(
            &mut scene,
            &mut camera,
            "edge_create",
            &format!(r#"{{"from":"{id_a}","to":"{id_b}"}}"#),
        )
        .expect("edge")["id"]
            .as_str()
            .expect("id")
            .to_owned();
        dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            &format!(r#"{{"id":"{e1}","kind":"value"}}"#),
        )
        .expect("value");
        // B = 10
        assert_eq!(
            scene.expr_results.get(&id_b),
            Some(&ExprOutcome::Ok(canvas_core::expr::Value::scalar(10.0)))
        );
        // Тогл в control — у B «вход отсутствует»
        dispatch(
            &mut scene,
            &mut camera,
            "flow_set_kind",
            &format!(r#"{{"id":"{e1}","kind":"control"}}"#),
        )
        .expect("control");
        match scene.expr_results.get(&id_b).expect("запись") {
            ExprOutcome::Err(msg) => assert!(msg.contains("вход"), "{msg}"),
            other => panic!("ожидалась ошибка входа: {other:?}"),
        }
        // Undo — снапшот «до тогла» возвращает value-ребро и пересчёт
        let before = scene.take_undo().expect("шаг undo");
        scene.canvas = before;
        scene.spatial = SpatialIndex::build(&scene.canvas);
        scene.recompute_flow();
        assert_eq!(
            scene.expr_results.get(&id_b),
            Some(&ExprOutcome::Ok(canvas_core::expr::Value::scalar(10.0))),
            "после undo поток восстановлен"
        );
    }

    // --- CR-008: умные порты связей ---

    /// edge_ports: закрепление обоих концов, авто сбрасывает пины;
    /// неизвестный id / невалидный pin — ошибка.
    #[test]
    fn mcp_edge_ports_pins_and_auto() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let out = dispatch(
            &mut scene,
            &mut camera,
            "edge_ports",
            r#"{"id":"edge-1","pin":"both"}"#,
        )
        .expect("pin both");
        assert_eq!(out["pins"]["from"], true);
        assert_eq!(out["pins"]["to"], true);
        assert_eq!(scene.canvas.edges[0].port_pins(), (true, true));

        // auto — снятие всех закреплений
        let out = dispatch(
            &mut scene,
            &mut camera,
            "edge_ports",
            r#"{"id":"edge-1","pin":"auto"}"#,
        )
        .expect("auto");
        assert_eq!(out["pins"]["from"], false);
        assert_eq!(out["pins"]["to"], false);
        assert!(!scene.canvas.edges[0].ports_pinned());
        assert!(
            scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
            "пустое расширение удалено"
        );

        // Ошибки: неизвестный pin и неизвестный id
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "edge_ports",
            r#"{"id":"edge-1","pin":"diagonal"}"#
        )
        .is_err());
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "edge_ports",
            r#"{"id":"ghost","pin":"auto"}"#
        )
        .is_err());
    }

    /// FR-019: template_list — built-in реестр отдаёт 15 шаблонов с полной
    /// схемой (инвариант 4: MCP-видимость эквивалентна UI; двуязычные имена).
    #[test]
    fn mcp_template_list_builtin_registry() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let list = dispatch(&mut scene, &mut camera, "template_list", "{}").expect("list");
        let templates = list.as_array().expect("массив");
        assert_eq!(templates.len(), 15, "все built-in шаблоны");
        let lb = templates
            .iter()
            .find(|t| t["id"] == "com.canvasdesk.lb")
            .expect("com.canvasdesk.lb");
        assert_eq!(lb["name_en"], "Load Balancer");
        assert_eq!(lb["name_ru"], "Балансировщик нагрузки");
        assert_eq!(lb["version"], "1.0.0");
        assert_eq!(lb["category"], "backend");
        assert_eq!(lb["expr"], "mm1($rps, $service_rate, $servers)");
        assert_eq!(lb["params"]["rps"]["type"], "rate");
        assert_eq!(lb["params"]["rps"]["default"], 1000.0);
        assert_eq!(lb["params"]["rps"]["unit"], "rps");
        // Схема для UI: иконка и цвет категории в списке
        assert_eq!(lb["icon"], "lb");
        assert_eq!(lb["color"], "#4A90E2");
        // FR-020: источник каждого шаблона в списке
        assert_eq!(lb["source"], "builtin");
        // Категории: 10 backend + 5 network
        let by_cat = |cat: &str| templates.iter().filter(|t| t["category"] == cat).count();
        assert_eq!(by_cat("backend"), 10);
        assert_eq!(by_cat("network"), 5);
    }

    /// FR-018: template_instantiate — text-нода с Numi-листом параметров
    /// и снимком canvasdesk.template; переопределение параметра учитывается
    /// формулой (пересчёт в потоке); undo возвращает состояние до создания.
    #[test]
    fn mcp_template_instantiate_creates_linked_node() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        let out = dispatch(
            &mut scene,
            &mut camera,
            "template_instantiate",
            r#"{"id":"com.canvasdesk.lb","x":150,"y":250,"params":{"rps":2000}}"#,
        )
        .expect("instantiate");
        let id = out["id"].as_str().expect("id").to_owned();
        let node = scene
            .canvas
            .nodes
            .iter()
            .find(|n| n.id == id)
            .expect("нода создана");
        assert_eq!(node.kind(), NodeKind::Text);
        assert_eq!(node.x, 150.0);
        // Numi-лист параметров с переопределением
        let text = node.text.as_deref().expect("текст");
        assert!(text.contains("rps = 2000 rps"), "текст: {text}");
        assert!(text.contains("servers = 2"));
        // Снимок template-ссылки
        let template = node.template().expect("template");
        assert_eq!(template.id, "com.canvasdesk.lb");
        assert_eq!(template.version, "1.0.0");
        assert_eq!(template.expr, "mm1($rps, $service_rate, $servers)");
        assert_eq!(template.params["rps"].num, 2000.0);
        assert_eq!(template.icon, "lb");
        // Формула в потоке (FR-014-стык): результат пересчитан
        assert!(
            scene.expr_results.contains_key(&id),
            "результат формулы шаблона в потоке"
        );
        // Undo: нода исчезает (undo-шаг при создании)
        let before = scene.canvas.nodes.len();
        let restored = scene.take_undo().expect("undo-шаг есть");
        scene.canvas = restored;
        scene.spatial = SpatialIndex::build(&scene.canvas);
        assert_eq!(scene.canvas.nodes.len(), before - 1);
        assert!(scene.canvas.nodes.iter().all(|n| n.id != id));
    }

    /// FR-018: template_instantiate — ошибки: неизвестный id, параметр вне
    /// границ манифеста (min/max), неизвестное имя параметра.
    #[test]
    fn mcp_template_instantiate_rejects_bad_input() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // Неизвестный шаблон
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "template_instantiate",
            r#"{"id":"ghost","x":0,"y":0}"#
        )
        .is_err());
        // rps < min 0
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "template_instantiate",
            r#"{"id":"com.canvasdesk.lb","x":0,"y":0,"params":{"rps":-5}}"#
        )
        .is_err());
        // Неизвестное имя параметра
        assert!(dispatch(
            &mut scene,
            &mut camera,
            "template_instantiate",
            r#"{"id":"com.canvasdesk.lb","x":0,"y":0,"params":{"ghost":1}}"#
        )
        .is_err());
        // Ни одна ошибочная ветка не мутировала модель
        assert!(scene.undo_stack.is_empty());
    }

    /// edge_ports "from": WYSIWYG — в fromSide фиксируется текущая
    /// эффективная сторона; свободный конец следует геометрии после
    /// переноса ноды. Undo возвращает состояние до пина.
    #[test]
    fn mcp_edge_ports_pin_freezes_effective_side() {
        let mut scene = mcp_scene();
        let mut camera = Camera::default();
        // n1(100,100) → f1(500,100): кратчайшая пара Right → Left
        let out = dispatch(
            &mut scene,
            &mut camera,
            "edge_ports",
            r#"{"id":"edge-1","pin":"from"}"#,
        )
        .expect("pin from");
        assert_eq!(out["pins"]["from"], true);
        assert_eq!(out["pins"]["to"], false);
        assert_eq!(scene.canvas.edges[0].from_side, Some(Side::Right));
        assert_eq!(scene.canvas.edges[0].port_pins(), (true, false));

        // Перенос f1 влево за n1: закреплённый исток остаётся Right,
        // свободный сток переходит на кратчайший порт
        scene.canvas.nodes[1].x = -500.0;
        scene.spatial = SpatialIndex::build(&scene.canvas);
        let curve = canvas_core::edge_curve(&scene.canvas, &scene.canvas.edges[0]).expect("кривая");
        assert_eq!(
            curve.p0,
            canvas_core::port_point(&scene.canvas.nodes[0], Side::Right),
            "right порт n1 закреплён"
        );
        assert_eq!(curve.p1, [-180.0, 210.0], "сток f1 — правый порт (авто)");

        // Undo: пин снят, снапшот до мутации
        let before = scene.take_undo().expect("шаг undo");
        scene.canvas = before;
        scene.spatial = SpatialIndex::build(&scene.canvas);
        assert!(!scene.canvas.edges[0].ports_pinned());
    }
}
