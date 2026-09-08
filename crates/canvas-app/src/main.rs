//! canvas-app — приложение: event loop, команды, UI-состояние, main().

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// Чистые UI-helpers (геометрия, hit-тесты, меню, двойной клик) — единый
// источник в библиотеке, здесь только платформенно-зависимое состояние.
use canvas_app::ui::{
    button_rect, in_resize_corner, menu_item_at, menu_item_rect, menu_label, menu_rect,
    next_free_id, panel_rect, panel_row_at, point_in_rect, ContextMenu, DoubleClick, EdgeDrag,
    SettingsRow, MAX_NOTE_WIDTH, MENU_FILL, MENU_ITEMS, MENU_LABEL_X, MENU_PADDING, MENU_WIDTH,
    MIN_NODE_HEIGHT, MIN_NODE_WIDTH, PANEL_HEADER_HEIGHT, PANEL_PADDING, PANEL_ROW_HEIGHT,
    SETTINGS_ROWS,
};
use canvas_core::{
    apply_file_events, edge_at, nearest_side, port_at, port_point, watched_dirs, Canvas, Edge,
    FileEvent, Node, NodeChange, NodeKind, Settings, SpatialIndex, ThumbnailProvider,
};
use canvas_render::camera::Vec2;
use canvas_render::cards::{preset_color, CardInstance, HEADER_HEIGHT};
use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand,
};
use canvas_render::text::{body_area, OverlayText, ScreenText, BODY_PADDING, BODY_TOP_GAP};
use canvas_render::{Camera, Color, FrameMeter, FrameOverlay, FrameStats, SceneView, Selection};
use canvas_shell::{Priority, ThumbService, WatchService};
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
/// Ширина клип-бокса тултипа битой ссылки (T10): длинный путь переносится
/// на границы этой области, экран не покидает.
const TOOLTIP_WIDTH: f32 = 380.0;

/// Screen-space текст с владеемой строкой (панель настроек): промежуточное
/// представление, конвертируется в `ScreenText` на кадр рендера.
struct OwnedScreenText {
    text: String,
    origin: [f32; 2],
    width: f32,
    font_size: f32,
    color: Color,
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
    /// Выделение: нода или связь (T8).
    selected: Option<Selection>,
    /// (индекс ноды, смещение от курсора до левого верхнего угла ноды в world).
    dragging: Option<(usize, Vec2)>,
    dirty_since: Option<Instant>,
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
            dragging: None,
            dirty_since: None,
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
}

/// Превью зоны дропа (T9): план вставки от DragEnter, origin следует за
/// курсором на DragOver; живёт до Leave/Drop.
struct DropPreview {
    /// Текущий origin сетки призраков в world-координатах.
    origin: Vec2,
    /// План вставки (id/тип/позиция) — переживает без изменений до Drop.
    plan: Vec<canvas_app::ui::DropInsert>,
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
    /// Настройки приложения (config.toml).
    settings: Settings,
    /// Путь конфига (None — не сохраняем, работаем на дефолтах).
    config_path: Option<PathBuf>,
    /// Панель настроек открыта.
    settings_open: bool,
    /// Превью зоны дропа (T9): план вставки на время DragOver.
    drop_preview: Option<DropPreview>,
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
}

impl App {
    fn new(
        scene: SceneState,
        thumbs: ThumbService,
        settings: Settings,
        config_path: Option<PathBuf>,
        drag_sender: Arc<dyn Fn(canvas_shell::dragdrop::DragEvent) + Send + Sync>,
        watcher: WatchService,
    ) -> Self {
        Self {
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
            settings,
            config_path,
            settings_open: false,
            drop_preview: None,
            watcher,
            #[cfg(windows)]
            drag_watcher: None,
            drag_sender,
        }
    }

    /// Scale factor окна (1.0 до создания окна).
    fn scale_factor(&self) -> f32 {
        self.window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0)
    }

    /// zoom * scale_factor — перевод world-px в физические (для буфера редактора).
    fn zoom_px(&self) -> f32 {
        self.camera.zoom() * self.scale_factor()
    }

    /// Начать редактирование текстовой ноды (T7). Не-text ноды игнорируются.
    fn begin_editing(&mut self, index: usize) {
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        if node.kind() != NodeKind::Text {
            return;
        }
        let text = node.text.clone().unwrap_or_default();
        let (_, width, height) = body_area(node);
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
        // подгоняем размер сразу при входе в редактирование
        self.fit_note_size();
        self.request_redraw();
    }

    /// Начать редактирование лейбла связи (T8): двойной клик по линии.
    /// Бокс редактирования — по центру кривой (edge_edit_area).
    fn begin_editing_edge(&mut self, index: usize) {
        let Some(edge) = self.scene.canvas.edges.get(index) else {
            return;
        };
        let text = edge.label.clone().unwrap_or_default();
        let Some((_, width, height)) = edge_edit_area(&self.scene.canvas, index) else {
            return;
        };
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

    /// Подрастить редактируемую заметку под контент (T7): текст не должен
    /// уходить за границы карточки. Высота — по числу строк layout, ширина —
    /// по самой длинной строке (с потолком MAX_NOTE_WIDTH). Только рост.
    /// Для лейблов связей (T8) не применяется — бокс фиксированный.
    fn fit_note_size(&mut self) {
        let zoom_px = self.zoom_px();
        let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut())
        else {
            return;
        };
        let EditTarget::Node(index) = session.target() else {
            return;
        };
        let (content_w_px, content_h_px) = session.content_size_px(renderer.font_system_mut());
        let needed_h = HEADER_HEIGHT + BODY_TOP_GAP + content_h_px / zoom_px + BODY_PADDING;
        let needed_w = (content_w_px / zoom_px + BODY_PADDING * 2.0).min(MAX_NOTE_WIDTH);
        let Some(node) = self.scene.canvas.nodes.get_mut(index) else {
            return;
        };
        let mut changed = false;
        if needed_h > node.height + 0.5 {
            node.height = needed_h;
            changed = true;
        }
        if needed_w > node.width + 0.5 {
            node.width = needed_w;
            changed = true;
        }
        if changed {
            self.scene.spatial.update(index, node);
            self.scene.mark_dirty();
        }
    }

    /// Завершить редактирование (T7/T8): commit — записать текст в модель и
    /// пометить канвас грязным (автосейв); cancel — откат, модель не менялась.
    /// Для связи (T8) пустой лейбл при commit сбрасывается в None.
    fn finish_editing(&mut self, commit: bool) {
        let Some(session) = self.editing.take() else {
            return;
        };
        self.editor_dragging = false;
        if commit && session.changed() {
            match session.target() {
                EditTarget::Node(index) => {
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        node.text = Some(session.text());
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
        }
        self.request_redraw();
    }

    /// Удалить выделенное (T8, Del): связь — по id; ноду — каскадно со
    /// связями (canvas-core). После удаления ноды индексы в canvas.nodes
    /// сдвигаются, поэтому spatial index перестраивается, а все кэши,
    /// ключованные usize (текст, атлас тамбнейлов, негативный кэш),
    /// сбрасываются полностью.
    fn delete_selected(&mut self) {
        match self.scene.selected {
            Some(Selection::Edge(index)) => {
                let Some(edge) = self.scene.canvas.edges.get(index) else {
                    return;
                };
                let id = edge.id.clone();
                self.scene.canvas.remove_edge(&id);
                self.scene.selected = None;
                self.scene.mark_dirty();
                self.request_redraw();
            }
            Some(Selection::Node(index)) => {
                if self.scene.canvas.remove_node(index).is_none() {
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
        let id = next_free_id(&self.scene.canvas, "note");
        self.scene
            .canvas
            .nodes
            .push(Node::text(id, "", world[0], world[1]));
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.scene.selected = Some(Selection::Node(index));
        self.scene.mark_dirty();
        index
    }

    /// Активно ли панорамирование (средняя кнопка или Space+drag, SPEC §8).
    fn panning(&self) -> bool {
        self.middle_pressed || (self.space_pressed && self.left_pressed)
    }

    /// Размер viewport в логических пикселях.
    fn viewport_logical(&self) -> Vec2 {
        match &self.window {
            Some(window) => {
                let size = window.inner_size();
                let scale = window.scale_factor() as f32;
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
                // План пересчитываем по СВЕЖИМ данным Drop (не из превью,
                // план T9 §5): источник мог обновить содержимое
                let plan = plan_drop(&self.scene.canvas, &data, world);
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
                // Дроп мог добавить файловые ноды в новые директории —
                // синхронизируем вотчер (T10)
                self.sync_watch_dirs();
                self.drop_preview = None;
            }
        }
        self.request_redraw();
    }

    /// Синхронизировать вотчер с моделью (T10): родительские директории всех
    /// файловых нод → WatchService::sync_dirs (diff, повторный вызов — no-op).
    /// Вызывается после загрузки, дропа (T9), удаления нод и rename-событий.
    fn sync_watch_dirs(&mut self) {
        let dirs = watched_dirs(&self.scene.canvas, &self.scene.canvas_dir());
        self.watcher.sync_dirs(&dirs);
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

    /// Оверлей контекстного меню (T7): фон, образцы цветов, подписи пунктов.
    /// Возвращает (квады, подписи, world-позиции подписей).
    fn menu_overlay(&self) -> (Vec<CardInstance>, Vec<String>, Vec<Vec2>) {
        let mut instances = Vec::new();
        let mut labels = Vec::new();
        let mut label_pos = Vec::new();
        if let Some(menu) = &self.menu {
            let [x, y, w, h] = menu_rect(menu.origin);
            instances.push(CardInstance {
                pos: [x, y],
                size: [w, h],
                fill: MENU_FILL,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 0.0],
            });
            for (i, item) in MENU_ITEMS.iter().enumerate() {
                let rect = menu_item_rect(menu.origin, i);
                if let Some(color) = item.and_then(preset_color) {
                    // Образец цвета слева от подписи
                    instances.push(CardInstance {
                        pos: [rect[0] + 7.0, rect[1] + 7.0],
                        size: [12.0, 12.0],
                        fill: color,
                        border: [0.0; 4],
                        params: [2.0, 0.0, 0.0, 0.0],
                    });
                }
                labels.push(menu_label(*item));
                label_pos.push([rect[0] + MENU_LABEL_X, rect[1] + 6.0]);
            }
        }
        (instances, labels, label_pos)
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
        let button = button_rect(self.settings.button_corner, viewport);
        instances.push(CardInstance {
            pos: [button[0], button[1]],
            size: [button[2], button[3]],
            fill: [0.11, 0.11, 0.13, 0.9],
            border: [0.0; 4],
            // params.y = рамка выделения: подсветка кнопки при открытой панели
            params: [8.0, self.settings_open as u8 as f32, 0.0, 0.0],
        });
        texts.push(OwnedScreenText {
            text: "⚙".to_owned(),
            origin: [button[0] + 9.0, button[1] + 7.0],
            width: button[2],
            font_size: 18.0,
            color: Color::rgb(0xe6, 0xe6, 0xe6),
        });
        if !self.settings_open {
            return (instances, texts);
        }
        let panel = panel_rect(self.settings.button_corner, viewport);
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: MENU_FILL,
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
            color: Color::rgb(0xe6, 0xe6, 0xe6),
        });
        let rows_top = panel[1] + PANEL_PADDING + PANEL_HEADER_HEIGHT;
        for (i, row) in SETTINGS_ROWS.iter().enumerate() {
            texts.push(OwnedScreenText {
                text: row.label(&self.settings),
                origin: [text_x, rows_top + i as f32 * PANEL_ROW_HEIGHT + 5.0],
                width: text_w,
                font_size: 13.0,
                color: Color::rgb(0xd4, 0xd4, 0xd4),
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
            color: Color::rgb(0x8a, 0x8a, 0x92),
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

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("CanvasDesk");
        // winit сам ставит свой IDropTarget (RegisterDragDrop с assert S_OK) —
        // отключаем и ставим свой в canvas-shell (план T9 §3)
        #[cfg(windows)]
        let attrs = attrs.with_drag_and_drop(false);
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!(%err, "не удалось создать окно");
                event_loop.exit();
                return;
            }
        };
        // GPU-инициализация блокирующая, один раз при старте (SPEC §6.3: холодный старт < 2 с)
        match pollster::block_on(canvas_render::Renderer::new(window.clone())) {
            Ok(mut renderer) => {
                renderer.set_grid_visible(self.settings.grid_visible);
                tracing::info!(
                    width = window.inner_size().width,
                    height = window.inner_size().height,
                    scale_factor = window.scale_factor(),
                    "окно создано"
                );
                self.renderer = Some(renderer);
                self.window = Some(window.clone());
                // Регистрация своего IDropTarget (T9): HWND достаём через
                // raw-window-handle (winit 0.30 публично Win32-HWND не отдаёт);
                // ошибка — warn и живём без drag-drop (graceful degradation)
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
                                    Err(err) => tracing::warn!(
                                        %err,
                                        "drag-drop недоступен, приложение работает без него"
                                    ),
                                }
                            }
                            // На Windows бывает только Win32-handle
                            _ => tracing::warn!("неожидаемый handle окна — drag-drop выключен"),
                        },
                        Err(err) => {
                            tracing::warn!(%err, "handle окна недоступен — drag-drop выключен")
                        }
                    }
                }
                self.request_redraw();
            }
            Err(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Форс-сейв перед выходом — не ждать debounce (SPEC §9)
                if self.scene.dirty_since.is_some() {
                    self.scene.save_now();
                }
                event_loop.exit();
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
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(&event),
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Middle => self.middle_pressed = state == ElementState::Pressed,
                MouseButton::Left => self.on_left_button(state),
                MouseButton::Right => self.on_right_button(state),
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
                let hud = self.hud_text();
                // Оверлей контекстного меню (T7): квады + подписи пунктов
                // Т9 добавляет в конец призраков дропа — mutable
                let (mut overlay_instances, overlay_labels, overlay_label_pos) =
                    self.menu_overlay();
                let overlay_texts: Vec<OverlayText> = overlay_labels
                    .iter()
                    .zip(&overlay_label_pos)
                    .map(|(label, pos)| OverlayText {
                        text: label,
                        origin: *pos,
                        width: MENU_WIDTH - MENU_LABEL_X - MENU_PADDING,
                    })
                    .collect();
                // Панель настроек (screen-space): кнопка + строки переключателей
                let (screen_instances, mut owned_texts) = self.settings_overlay();
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
                }
                let overlay = FrameOverlay {
                    instances: &overlay_instances,
                    texts: &overlay_texts,
                    screen_instances: &screen_instances,
                    screen_texts: &screen_texts,
                };
                // Резиновая линия новой связи (T8): от порта к курсору
                let edge_draft = self.edge_drag.as_ref().and_then(|drag| {
                    let node = self.scene.canvas.node(&drag.from_node)?;
                    Some((
                        port_point(node, drag.from_side),
                        drag.from_side,
                        self.cursor_world(),
                    ))
                });
                if let Some(renderer) = self.renderer.as_mut() {
                    let scene = SceneView {
                        canvas: &self.scene.canvas,
                        spatial: &self.scene.spatial,
                        selected: self.scene.selected,
                        hovered: self.hovered,
                        edge_draft,
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
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.scene.autosave_if_due();
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
        // Esc закрывает контекстное меню (T7), затем — панель настроек
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
                self.scene.dragging = None;
            }
        }
        // F3 — переключить HUD с fps/p95/счётчиком видимых нод (T5)
        if event.logical_key == Key::Named(NamedKey::F3)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.hud_visible = !self.hud_visible;
            self.request_redraw();
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
        if self.space_pressed {
            return; // Space+drag — панорамирование (SPEC §8)
        }
        match state {
            ElementState::Pressed => {
                // Панель настроек (screen-space): клики обрабатываются до
                // канваса — кнопка/панель поверх и «прозрачности» не дают
                let viewport = self.viewport_logical();
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
                let world = self.cursor_world();
                // Hit-test через spatial index (T5): O(log n) вместо линейного обхода
                let hit = self.scene.spatial.hit_test(world);
                // Открытое меню (T7): клик по пункту — применить цвет, мимо — закрыть
                if let Some(menu) = self.menu.take() {
                    if let Some(i) = menu_item_at(menu.origin, world) {
                        if let Some(node) = self.scene.canvas.nodes.get_mut(menu.node) {
                            node.color = MENU_ITEMS[i].map(str::to_owned);
                        }
                        self.scene.mark_dirty();
                    }
                    self.request_redraw();
                    return;
                }
                // Активное редактирование (T7/T8): клик внутри области
                // редактирования — в курсор, клик снаружи — commit и обычная
                // обработка
                if let Some(target) = self.editing.as_ref().map(EditingSession::target) {
                    let inside = match target {
                        EditTarget::Node(index) => hit == Some(index),
                        EditTarget::Edge(index) => edge_edit_area(&self.scene.canvas, index)
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
                            if let Some((origin, _, _)) = session_area(&self.scene.canvas, session)
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
                // Порт hover-ноды (T8): начало drag резиновой линии новой
                // связи — drag ноды/resize/двойной клик не начинаются
                if let Some(node_index) = self.hovered {
                    let port = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| port_at(node, world, self.camera.zoom()));
                    if let Some(side) = port {
                        let from_node = self.scene.canvas.nodes[node_index].id.clone();
                        self.edge_drag = Some(EdgeDrag {
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
                    match hit {
                        None => match edge_at(&self.scene.canvas, world) {
                            Some(edge_index) => self.begin_editing_edge(edge_index),
                            None => {
                                let index = self.create_note_at(world);
                                self.begin_editing(index);
                            }
                        },
                        Some(index) => self.begin_editing(index),
                    }
                    self.request_redraw();
                    return;
                }
                // Ручной resize (T7): захват за правый нижний угол ноды
                if let Some(index) = hit {
                    if in_resize_corner(&self.scene.canvas.nodes[index], world) {
                        self.scene.selected = Some(Selection::Node(index));
                        self.resizing = Some(index);
                        self.request_redraw();
                        return;
                    }
                }
                match hit {
                    Some(index) => {
                        self.scene.selected = Some(Selection::Node(index));
                        let node = &self.scene.canvas.nodes[index];
                        self.scene.dragging = Some((index, [node.x - world[0], node.y - world[1]]));
                    }
                    // Промах по нодам: hit-test связей (T8) — ближайшая
                    // в допуске EDGE_HIT_TOLERANCE, иначе сброс выделения
                    None => {
                        self.scene.selected =
                            edge_at(&self.scene.canvas, world).map(Selection::Edge);
                    }
                }
                self.request_redraw();
            }
            ElementState::Released => {
                // Drop резиновой линии (T8): на другую ноду — создать связь
                // (to_side — ближайшая к курсору сторона), в пустоту или на
                // ту же ноду — отмена
                if let Some(drag) = self.edge_drag.take() {
                    let world = self.cursor_world();
                    if let Some(target) = self.scene.spatial.hit_test(world) {
                        let to_node = &self.scene.canvas.nodes[target];
                        let to_id = to_node.id.clone();
                        if to_id != drag.from_node {
                            let to_side = nearest_side(to_node, world);
                            let edge = Edge::new(
                                self.scene.canvas.next_edge_id(),
                                drag.from_node,
                                Some(drag.from_side),
                                to_id,
                                Some(to_side),
                            );
                            self.scene.canvas.add_edge(edge);
                            self.scene.mark_dirty();
                        }
                    }
                    self.request_redraw();
                }
                self.scene.dragging = None;
                self.editor_dragging = false;
                self.resizing = None;
            }
        }
    }

    fn on_right_button(&mut self, state: ElementState) {
        if state != ElementState::Pressed {
            return;
        }
        // ПКМ во время редактирования — сначала commit (T7)
        if self.editing.is_some() {
            self.finish_editing(true);
        }
        let world = self.cursor_world();
        match self.scene.spatial.hit_test(world) {
            // Меню ноды (T7): палитра цветов в точке клика
            Some(index) => {
                self.scene.selected = Some(Selection::Node(index));
                self.menu = Some(ContextMenu {
                    node: index,
                    origin: world,
                });
            }
            None => self.menu = None,
        }
        self.request_redraw();
    }

    fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        let logical = [position.x as f32 / scale, position.y as f32 / scale];
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
                if let Some((origin, _, _)) = session_area(&self.scene.canvas, session) {
                    let x = ((world[0] - origin[0]) * zoom_px) as i32;
                    let y = ((world[1] - origin[1]) * zoom_px) as i32;
                    session.drag(renderer.font_system_mut(), x, y);
                }
            }
            self.request_redraw();
        }
        if !self.space_pressed {
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
            } else if let Some((index, offset)) = self.scene.dragging {
                let world = self.cursor_world();
                // Модель + инкрементальное обновление spatial index (T5)
                self.scene
                    .move_node(index, world[0] + offset[0], world[1] + offset[1]);
                self.scene.mark_dirty();
                self.request_redraw();
            } else if self.edge_drag.is_some() {
                // Резиновая линия (T8) следует за курсором — курсор уже
                // обновлён выше, нужна только перерисовка
                self.request_redraw();
            } else if !self.panning() && !self.editor_dragging && self.editing.is_none() {
                // Hover (T8): порты ноды под курсором; перерисовка — только
                // при смене ноды, чтобы не крутить кадры на каждый пиксель
                let world = self.cursor_world();
                let hovered = self.scene.spatial.hit_test(world);
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.request_redraw();
                }
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // Тачпады шлют PixelDelta (физические px), колёсики мышей — LineDelta
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
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
    path: PathBuf,
}

/// Разбор аргументов вручную — две опции не оправдывают зависимость от clap.
fn parse_args(args: &[String]) -> anyhow::Result<CliArgs> {
    let mut stress = None;
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
        } else if arg == "--help" || arg == "-h" {
            println!("Использование: canvasdesk [--stress N] [путь к .canvas]");
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
        path: path.unwrap_or_else(|| PathBuf::from(default_path)),
    })
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
    // По умолчанию info, но без спама внутренних крейтов wgpu; переопределяется через RUST_LOG
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu_hal=warn,wgpu_core=warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
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
    let thumbs = ThumbService::new(
        provider,
        cache,
        Some(Arc::new(move || {
            let _ = proxy.send_event(AppEvent::ThumbsReady);
        })),
    );
    event_loop.run_app(&mut App::new(
        scene,
        thumbs,
        settings,
        config_path,
        drag_sender,
        watcher,
    ))?;
    Ok(())
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

    /// Парсинг аргументов: --stress N, --stress=N, путь, дефолты.
    #[test]
    fn cli_args_parsing() {
        let args = parse_args(&[]).expect("пустые аргументы");
        assert_eq!(args.stress, None);
        assert_eq!(args.path, PathBuf::from("default.canvas"));

        let args = parse_args(&["--stress".into(), "5000".into()]).expect("--stress N");
        assert_eq!(args.stress, Some(5000));
        assert_eq!(args.path, PathBuf::from("stress.canvas"));

        let args =
            parse_args(&["--stress=100".into(), "my.canvas".into()]).expect("--stress=N path");
        assert_eq!(args.stress, Some(100));
        assert_eq!(args.path, PathBuf::from("my.canvas"));

        assert!(parse_args(&["--stress".into()]).is_err());
        assert!(parse_args(&["--stress".into(), "abc".into()]).is_err());
        assert!(parse_args(&["a.canvas".into(), "b.canvas".into()]).is_err());
    }
}
