//! canvas-app — приложение: event loop, команды, UI-состояние, main().

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Чистые UI-helpers (геометрия, hit-тесты, меню, двойной клик) — единый
// источник в библиотеке, здесь только платформенно-зависимое состояние.
use canvas_app::ui::{
    button_rect, canvas_menu_label, drag_origins, edge_menu_label, focus_seed_of,
    hotkeys_panel_rect, in_resize_corner, menu_item_at_for, menu_item_rect, menu_rect_for,
    next_free_id, node_menu_label, nodes_in_rect, panel_rect, panel_row_at, paste_nodes,
    plan_group_around, plan_group_at, point_in_rect, reassign_ids, rubber_band_rect,
    select_node_hit, submenu_item_at, submenu_origin_next_to, submenu_rect, theme_button_rect,
    toggle_selection_with_primary, CanvasMenuItem, ContextMenu, DoubleClick, DragState, EdgeDrag,
    EdgeMenuItem, MenuTarget, NodeMenuItem, PastePlacement, SettingsRow, Submenu, SubmenuEntry,
    CANVAS_MENU_ITEMS, DUPLICATE_OFFSET, EDGE_MENU_ITEMS, MENU_ITEM_HEIGHT, MENU_LABEL_X,
    MENU_PADDING, MENU_WIDTH, MIN_NODE_HEIGHT, MIN_NODE_WIDTH, NODE_MENU_ITEMS,
    PANEL_HEADER_HEIGHT, PANEL_PADDING, PANEL_ROW_HEIGHT, SELECT_DRAG_THRESHOLD, SETTINGS_ROWS,
};
use canvas_core::{
    apply_file_events, edge_at, focus_set, nearest_side, next_port_zone, path_matches, port_at,
    resolve_node_path, watched_dirs, Canvas, Edge, FileEvent, FocusSeed, GridStyle, Node,
    NodeChange, NodeKind, Settings, Side, SpatialIndex, Theme, ThumbnailProvider,
};
use canvas_render::animate::{
    focus_fade, focus_pulse, pulse_alpha, Flight, FLIGHT_DURATION_MS, FOCUS_FADE_MS, FOCUS_PULSE_MS,
};
use canvas_render::camera::Vec2;
use canvas_render::cards::{preset_color, CardInstance, FocusView, HEADER_HEIGHT};
use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand,
};
use canvas_render::minimap::{Minimap, MINIMAP_H, MINIMAP_W};
use canvas_render::search_ui::{
    layout as search_layout, scan_scene, PanelAction, SceneEntry, SearchInput, SearchPanel,
    SearchRow,
};
use canvas_render::text::{
    body_area, OverlayText, ScreenText, TextAlign, BODY_PADDING, BODY_TOP_GAP,
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
use winit::window::{Window, WindowId};
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
}

impl SceneState {
    /// Обернуть готовую модель: построить spatial index.
    fn new(canvas: Canvas, path: PathBuf) -> Self {
        let spatial = SpatialIndex::build(&canvas);
        Self {
            canvas,
            spatial,
            path,
            selected: None,
            selected_nodes: Vec::new(),
            dragging: None,
            dirty_since: None,
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
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
}

impl AppDialog {
    /// Кнопки диалога (screen-space rect'ы считаются от центра окна).
    fn buttons(&self) -> [(&'static str, bool); 2] {
        // (подпись, confirm?)
        [("Да", true), ("Нет", false)]
    }

    /// Заголовок диалога.
    fn title(&self) -> String {
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
        }
    }
}

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
    /// Ручной resize ноды за правый нижний угол (T7): индекс ноды.
    resizing: Option<usize>,
    /// Нода под курсором (T8): показываются порты для начала drag связи.
    hovered: Option<usize>,
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
            resizing: None,
            hovered: None,
            edge_drag: None,
            select_rect: None,
            node_clipboard: Vec::new(),
            hotkeys_open: false,
            pending_undo: None,
            settings,
            config_path,
            settings_open: false,
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
            focus_dim: 0.0,
            focus_fade: None,
            focus_pulse: None,
            focus_nodes: Vec::new(),
            focus_edges: Vec::new(),
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
        self.scene.selected = Some(Selection::Node(index));
        self.scene.dragging = None;
        // Давняя заметка могла переполниться до нас (загрузка из файла) —
        // подгоняем размер сразу при входе в редактирование. Группу под
        // текст не подгоняем: рамку ресайзит только пользователь.
        if !is_group {
            self.fit_note_size();
        }
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
        self.scene.selected = Some(Selection::Edge(index));
        self.scene.dragging = None;
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
        let needed_h = HEADER_HEIGHT + BODY_TOP_GAP + content_h_px / zoom_px + BODY_PADDING;
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
        self.editor_dragging = false;
        if commit && session.changed() {
            // FR-006: правка состоялась — отложенный снапшот «до» в историю
            // (мутация ниже); cancel-ветка дропнет его
            if let Some(snapshot) = self.pending_undo.take() {
                self.scene.push_undo(snapshot);
            }
            match session.target() {
                EditTarget::Node(index) => {
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
                            node.text = Some(session.text());
                        }
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
        self.scene.canvas = canvas;
        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
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
    fn selective_hit(&self, world: Vec2) -> Option<usize> {
        let candidates = self
            .scene
            .spatial
            .query_rect([world[0], world[1], world[0], world[1]]);
        select_node_hit(&self.scene.canvas, &candidates)
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
            self.minimap = Some(Minimap::capture(
                &self.scene.canvas,
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
    fn apply_search_hits(&mut self, hits: Vec<SearchHit>) {
        let canvas_dir = self.scene.canvas_dir();
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
            instances.push(CardInstance {
                pos: [row_rect[0], row_rect[1]],
                size: [row_rect[2], row_rect[3]],
                fill: if selected {
                    [0.18, 0.29, 0.48, 0.95]
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

    /// Оверлей контекстного меню (T7): фон, образцы, подписи пунктов.
    /// Нода — палитра цветов + действия; связь — стиль линии, толщина, цвет;
    /// пустое место — действия канваса.
    /// Возвращает (квады, подписи, world-позиции подписей).
    fn menu_overlay(&self) -> (Vec<CardInstance>, Vec<String>, Vec<Vec2>) {
        let mut instances = Vec::new();
        let mut labels = Vec::new();
        let mut label_pos = Vec::new();
        let Some(menu) = &self.menu else {
            return (instances, labels, label_pos);
        };
        let palette = ThemeColors::from_theme(self.settings.theme);
        match menu.target {
            MenuTarget::Node(_) => {
                let items = NODE_MENU_ITEMS.len();
                let [x, y, w, h] = menu_rect_for(menu.origin, items);
                instances.push(CardInstance {
                    pos: [x, y],
                    size: [w, h],
                    fill: palette.menu_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 0.0],
                });
                for (i, item) in NODE_MENU_ITEMS.iter().enumerate() {
                    let rect = menu_item_rect(menu.origin, i);
                    match item {
                        NodeMenuItem::Color(color) => {
                            if let Some(color) = color.and_then(preset_color) {
                                // Образец цвета слева от подписи
                                instances.push(CardInstance {
                                    pos: [rect[0] + 7.0, rect[1] + 7.0],
                                    size: [12.0, 12.0],
                                    fill: color,
                                    border: [0.0; 4],
                                    params: [2.0, 0.0, 0.0, 0.0],
                                });
                            }
                            labels.push(node_menu_label(*item).unwrap_or_default());
                            label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
                        }
                        // Разделитель палитры и действий: тонкая линия
                        NodeMenuItem::Separator => {
                            let line_y = rect[1] + MENU_ITEM_HEIGHT / 2.0;
                            instances.push(CardInstance {
                                pos: [rect[0] + 4.0, line_y],
                                size: [rect[2] - 8.0, 1.0],
                                fill: palette.body_fill(),
                                border: [0.0; 4],
                                params: [0.0, 0.0, 0.0, 1.0],
                            });
                        }
                        NodeMenuItem::Group => {
                            labels.push(node_menu_label(*item).unwrap_or_default());
                            label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
                        }
                    }
                }
            }
            MenuTarget::Edge(edge_index) => {
                let [x, y, w, h] = menu_rect_for(menu.origin, EDGE_MENU_ITEMS.len());
                instances.push(CardInstance {
                    pos: [x, y],
                    size: [w, h],
                    fill: palette.menu_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 0.0],
                });
                for (i, item) in EDGE_MENU_ITEMS.iter().enumerate() {
                    let rect = menu_item_rect(menu.origin, i);
                    match item {
                        EdgeMenuItem::Color(color) => {
                            // Образец цвета слева от подписи
                            if let Some(fill) = color.and_then(preset_color) {
                                instances.push(CardInstance {
                                    pos: [rect[0] + 7.0, rect[1] + 7.0],
                                    size: [12.0, 12.0],
                                    fill,
                                    border: [0.0; 4],
                                    params: [2.0, 0.0, 0.0, 0.0],
                                });
                            }
                        }
                        EdgeMenuItem::Thickness(thickness) => {
                            // Образец-толщина: полоска высотой dot()
                            let line_h = thickness.dot();
                            instances.push(CardInstance {
                                pos: [rect[0] + 7.0, rect[1] + (26.0 - line_h) / 2.0],
                                size: [12.0, line_h],
                                fill: palette.body_fill(),
                                border: [0.0; 4],
                                params: [line_h / 2.0, 0.0, 0.0, 0.0],
                            });
                        }
                        EdgeMenuItem::Style(_) => {}
                    }
                    let label = self
                        .scene
                        .canvas
                        .edges
                        .get(edge_index)
                        .map(|edge| edge_menu_label(*item, edge))
                        .unwrap_or_default();
                    labels.push(label);
                    label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
                }
            }
            MenuTarget::Canvas => {
                let [x, y, w, h] = menu_rect_for(menu.origin, CANVAS_MENU_ITEMS.len());
                instances.push(CardInstance {
                    pos: [x, y],
                    size: [w, h],
                    fill: palette.menu_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 0.0],
                });
                for (i, item) in CANVAS_MENU_ITEMS.iter().enumerate() {
                    let rect = menu_item_rect(menu.origin, i);
                    labels.push(canvas_menu_label(
                        *item,
                        self.settings.focus_mode,
                        self.hotkeys_open,
                    ));
                    label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
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
                        labels.push("(нет установленных)".to_owned());
                        label_pos.push([
                            submenu.origin[0] + MENU_PADDING + 4.0,
                            submenu.origin[1] + MENU_PADDING + 6.0,
                        ]);
                    } else {
                        for (i, entry) in submenu.entries.iter().enumerate() {
                            let rect = menu_item_rect(submenu.origin, i);
                            labels.push(entry.label.clone());
                            label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
                        }
                    }
                }
            }
        }
        (instances, labels, label_pos)
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

    /// Применить переключение строки панели настроек и сохранить конфиг.
    fn apply_settings_row(&mut self, row: usize) {
        match SETTINGS_ROWS[row] {
            SettingsRow::ButtonCorner => {
                self.settings.button_corner = self.settings.button_corner.next();
            }
            SettingsRow::Grid => {
                self.settings.grid_visible = !self.settings.grid_visible;
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_grid_visible(self.settings.grid_visible);
                }
            }
            SettingsRow::GridStyle => {
                self.settings.grid_style = self.settings.grid_style.next();
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_grid_dots(self.settings.grid_style == GridStyle::Dots);
                }
            }
            SettingsRow::GridDensity => {
                self.settings.grid_density = self.settings.grid_density.next();
                if let Some(renderer) = self.renderer.as_mut() {
                    let (minor, major) = self.settings.grid_density.steps();
                    renderer.set_grid_steps(minor, major);
                }
            }
            SettingsRow::EdgesAvoid => {
                self.settings.edges_avoid_nodes = !self.settings.edges_avoid_nodes;
            }
            // CR-003: зона портов — цикл по пресетам, радиус кружков портов
            // следует за значением автоматически (рендер читает настройки)
            SettingsRow::PortZone => {
                self.settings.port_zone_px = next_port_zone(self.settings.port_zone_px);
            }
            // T23: состояние синхронно с settings — сохранение общим хвостом
            SettingsRow::FocusMode => self.toggle_focus_mode(),
            SettingsRow::HudOnStart => {
                self.settings.hud_on_start = !self.settings.hud_on_start;
                // Мгновенная обратная связь: HUD переключается сразу
                self.hud_visible = self.settings.hud_on_start;
            }
        }
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
        instances.push(CardInstance {
            pos: [button[0], button[1]],
            size: [button[2], button[3]],
            fill: palette.menu_fill,
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
        instances.push(CardInstance {
            pos: [theme_button[0], theme_button[1]],
            size: [theme_button[2], theme_button[3]],
            fill: palette.menu_fill,
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
        let panel = panel_rect(self.settings.button_corner, viewport);
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
        let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
        for (i, row) in SETTINGS_ROWS.iter().enumerate() {
            texts.push(OwnedScreenText {
                text: row.label(&self.settings),
                origin: [text_x, rows_top + i as f32 * PANEL_ROW_HEIGHT + 5.0],
                width: text_w,
                font_size: 13.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        texts.push(OwnedScreenText {
            text: "Ctrl+, — открыть/закрыть".to_owned(),
            origin: [
                text_x,
                rows_top + SETTINGS_ROWS.len() as f32 * PANEL_ROW_HEIGHT + 4.0,
            ],
            width: text_w,
            font_size: 11.0,
            color: palette.icon,
            align: TextAlign::Left,
        });
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
                    match mcp_dispatch(&mut self.scene, &mut self.camera, &method, &params) {
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
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг (после валидации, до вставки)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
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
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        // FR-005: редактирование ноды одним вызовом — обновляются ТОЛЬКО
        // переданные поля; label/color = null — сброс; геометрия — с
        // обновлением spatial index; ответ — сводка с текстом
        "node_edit" => {
            let id = mcp_req_str(params, "id")?;
            let index = mcp_node_index(&scene.canvas, id)?;
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
            Ok(serde_json::json!({ "id": id }))
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
        // Esc закрывает контекстное меню (T7), затем — панель настроек,
        // затем — панель хоткеев (FR-004)
        if event.logical_key == Key::Named(NamedKey::Escape)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            if self.menu.take().is_some() {
                self.request_redraw();
                return;
            }
            if self.settings_open {
                self.settings_open = false;
                self.request_redraw();
                return;
            }
            if self.hotkeys_open {
                self.hotkeys_open = false;
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
            self.request_redraw();
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Space) && !event.repeat {
            self.space_pressed = event.state == ElementState::Pressed;
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
                    self.request_redraw();
                    return;
                }
                if self.settings_open {
                    let panel = panel_rect(self.settings.button_corner, viewport);
                    if let Some(row) = panel_row_at(panel, self.cursor) {
                        self.apply_settings_row(row);
                    } else if !point_in_rect(panel, self.cursor) {
                        // Клик мимо панели — закрыть; канвасу клик не достаётся
                        // (иначе двойной клик мимо создал бы заметку)
                        self.settings_open = false;
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
                // Открытое меню (T7): клик по пункту — применить, мимо — закрыть.
                // M5: открытое подменю виджетов проверяется ПЕРВЫМ — его
                // колонка правее базового меню (клик там не попадает в base)
                if let Some(submenu) = self.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                    if let Some(i) = submenu_item_at(submenu, world) {
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
                                self.dialog = Some(AppDialog::RemovePackage { widget_id, name });
                            }
                        }
                        self.request_redraw();
                        return;
                    }
                }
                if let Some(menu) = self.menu.take() {
                    match menu.target {
                        MenuTarget::Node(node_index) => {
                            if let Some(i) =
                                menu_item_at_for(menu.origin, world, NODE_MENU_ITEMS.len())
                            {
                                match NODE_MENU_ITEMS[i] {
                                    NodeMenuItem::Color(color) => {
                                        // FR-006: смена цвета — undo-шаг;
                                        // повторный клик того же цвета (no-op)
                                        // шага не создаёт — сравнение после
                                        let snapshot = self.scene.canvas.clone();
                                        if let Some(node) =
                                            self.scene.canvas.nodes.get_mut(node_index)
                                        {
                                            node.color = color.map(str::to_owned);
                                        }
                                        if self.scene.canvas != snapshot {
                                            self.scene.push_undo(snapshot);
                                        }
                                        self.scene.mark_dirty();
                                    }
                                    // Разделитель не кликабелен — меню просто закрывается
                                    NodeMenuItem::Separator => {}
                                    // Обернуть ноду в группу (bbox = нода + padding)
                                    NodeMenuItem::Group => {
                                        if let Some(group) = plan_group_around(
                                            &self.scene.canvas,
                                            node_index,
                                            canvas_app::ui::GROUP_PADDING,
                                        ) {
                                            self.insert_group(group);
                                        }
                                    }
                                }
                            }
                        }
                        MenuTarget::Edge(edge_index) => {
                            if let Some(i) =
                                menu_item_at_for(menu.origin, world, EDGE_MENU_ITEMS.len())
                            {
                                // FR-006: смена стиля/толщины/цвета связи —
                                // undo-шаг (no-op клик шага не создаёт)
                                let snapshot = self.scene.canvas.clone();
                                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                                    match EDGE_MENU_ITEMS[i] {
                                        EdgeMenuItem::Style(style) => edge.style = Some(style),
                                        EdgeMenuItem::Thickness(thickness) => {
                                            edge.thickness = Some(thickness)
                                        }
                                        EdgeMenuItem::Color(color) => {
                                            edge.color = color.map(str::to_owned)
                                        }
                                    }
                                }
                                if self.scene.canvas != snapshot {
                                    self.scene.push_undo(snapshot);
                                }
                                self.scene.mark_dirty();
                            }
                        }
                        MenuTarget::Canvas => {
                            if let Some(i) =
                                menu_item_at_for(menu.origin, world, CANVAS_MENU_ITEMS.len())
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
                                    // M5 (T20-F): открыть подменю пакетов
                                    // (план П2); пустой список — честная
                                    // строка «(нет установленных)».
                                    // T21-C: под каждой вставкой — секция
                                    // удаления пакетов (П11)
                                    CanvasMenuItem::Widgets => {
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
                                            target: MenuTarget::Canvas,
                                            origin: menu.origin,
                                            submenu: Some(Submenu {
                                                origin: submenu_origin,
                                                entries,
                                            }),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    self.request_redraw();
                    return;
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
                        self.edge_drag = Some(EdgeDrag::New {
                            from_node,
                            from_side: side,
                        });
                        self.request_redraw();
                        return;
                    }
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
                        } => {
                            if let Some(target) = self.selective_hit(world) {
                                let to_node = &self.scene.canvas.nodes[target];
                                let to_id = to_node.id.clone();
                                if to_id != from_node {
                                    let to_side = nearest_side(to_node, world);
                                    let edge = Edge::new(
                                        self.scene.canvas.next_edge_id(),
                                        from_node,
                                        Some(from_side),
                                        to_id,
                                        Some(to_side),
                                    );
                                    // FR-006: новая связь — undo-шаг
                                    self.push_undo();
                                    self.scene.canvas.add_edge(edge);
                                    self.scene.mark_dirty();
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
                                } else {
                                    self.scene.undo_stack.pop_back();
                                }
                            }
                        }
                    }
                    self.request_redraw();
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
            // Меню ноды (T7): палитра цветов + действия в точке клика
            Some(index) => {
                self.scene.selected = Some(Selection::Node(index));
                self.menu = Some(ContextMenu {
                    target: MenuTarget::Node(index),
                    origin: world,
                    submenu: None,
                });
            }
            // Промах по нодам: меню связи (стиль/толщина/цвет линии);
            // мимо связи — меню пустого канваса (создание группы) или
            // закрытие меню (и десктоп-меню T17 в --desktop)
            None => {
                let avoid = self.settings.edges_avoid_nodes;
                match edge_at(&self.scene.canvas, world, avoid) {
                    Some(edge_index) => {
                        self.scene.selected = Some(Selection::Edge(edge_index));
                        self.menu = Some(ContextMenu {
                            target: MenuTarget::Edge(edge_index),
                            origin: world,
                            submenu: None,
                        });
                    }
                    None => {
                        // T17 (SPEC §7.4 п.6): в --desktop ПКМ по пустому месту —
                        // системное меню десктопа (нативное Win32: Открыть
                        // канвас / Новый текстовый файл / иконки / автозапуск /
                        // Выход); вне --desktop — меню пустого канваса
                        // (создание группы), повторный ПКМ мимо закрывает его
                        #[cfg(windows)]
                        let desktop_menu = self.desktop_mode && self.desktop_hierarchy.is_some();
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
                                Some(ContextMenu {
                                    target: MenuTarget::Canvas,
                                    ..
                                }) => None,
                                _ => Some(ContextMenu {
                                    target: MenuTarget::Canvas,
                                    origin: world,
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
                // а не группе (порты групп не рисуются — cards.rs)
                let world = self.cursor_world();
                let hovered = self.selective_hit(world);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.request_redraw();
                }
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
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
            widget_id: "com.canvasdesk.clock".to_owned(),
            props: serde_json::Map::new(),
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
        }
    }

    /// Отмена диалога (Esc/клик «Нет»): ничего не меняется.
    fn cancel_dialog(&mut self) {
        self.dialog = None;
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

    /// M5: rect открытого меню (airspace П7): по цели — количество пунктов.
    fn menu_open_rect(&self) -> Option<[f32; 4]> {
        let menu = self.menu.as_ref()?;
        let items = match menu.target {
            MenuTarget::Node(_) => NODE_MENU_ITEMS.len(),
            MenuTarget::Edge(_) => EDGE_MENU_ITEMS.len(),
            MenuTarget::Canvas => CANVAS_MENU_ITEMS.len(),
        };
        Some(menu_rect_for(menu.origin, items))
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
                MouseButton::Middle => self.middle_pressed = state == ElementState::Pressed,
                MouseButton::Left => self.on_left_button(state),
                MouseButton::Right => self.on_right_button(state, event_loop),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),
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
                // Миникарта (T13): пересборка по dirty-условиям ДО отрисовки
                // (текстура должна быть готова к проходу кадра)
                self.update_minimap();
                let hud = self.hud_text();
                // Оверлей контекстного меню (T7): квады + подписи пунктов
                // Т9 добавляет в конец призраков дропа — mutable
                let (mut overlay_instances, mut overlay_labels, mut overlay_label_pos) =
                    self.menu_overlay();
                // Ширины подписей оверлея: меню — от констант, призраки дропа —
                // по ширине карточки-призрака (Т9)
                let mut overlay_widths: Vec<f32> = overlay_labels
                    .iter()
                    .map(|_| MENU_WIDTH - MENU_LABEL_X - MENU_PADDING)
                    .collect();
                // Панель настроек (screen-space): кнопка + строки переключателей
                let (mut screen_instances, mut owned_texts) = self.settings_overlay();
                // Панель поиска (T14): квады/тексты поверх всего канваса
                {
                    let (search_instances, search_texts) = self.search_overlay();
                    screen_instances.extend(search_instances);
                    owned_texts.extend(search_texts);
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
                        text: dialog.title(),
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
                // M5 (T20-F): airspace-прямоугольники оверлеев (план П7) —
                // до LOD-кадра виджетов; большие панели (поиск/настройки)
                // упрощённо гасят все live (транзиентно), точные rect'ы —
                // меню/подменю/хоткеи/миникарта
                let mut widget_airspace: Vec<[f32; 4]> = Vec::new();
                if let Some(rect) = self.menu_open_rect() {
                    widget_airspace.push(rect);
                    if let Some(menu) = self.menu.as_ref() {
                        if let Some(submenu) = &menu.submenu {
                            widget_airspace.push(submenu_rect(submenu));
                        }
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
                }
                // Тамбнейлы видимых нод (T6): заказ после кадра, когда камера
                // уже установилась; ответы придут через AppEvent::ThumbsReady
                self.order_thumbnails();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
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
        // до завершения анимаций
        if self.search_pending.is_some()
            || self.flight.is_some()
            || self.pulse.is_some()
            || self.focus_animating()
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
        mcp_dispatch(scene, camera, method, &params)
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
}
