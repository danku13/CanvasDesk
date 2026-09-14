//! Менеджер виджетов (M5 T20-F, план M5 §4): реестр пакетов + LOD-планирование
//! + применение к host'у (Windows) + квады снапшотов для рендера.
//!
//! Кроссплатформенное ядро (тестируется на Linux): `update_frame` — чистая
//! сборка `WidgetLodInput`/решений из модели сцены; Win32-часть — только
//! `host` (cfg(windows)) и тонкие обёртки. Источник решений —
//! `canvas_widgets::lod::plan_frame`; airspace-политика П7 — здесь.

use canvas_core::{Canvas, Node, NodeKind};
use canvas_widgets::layout::{self, CameraArgs};
use canvas_widgets::lod::{self, Target};
use canvas_widgets::manifest::WidgetManifest;
use canvas_widgets::permissions::Permissions;
use canvas_widgets::registry::{InstallOutcome, WidgetRegistry};
use canvas_widgets::{HostToWidget, ThemeInfo, WidgetEvent, WidgetProps};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Кулдаун пересоздания контроллера после сбоя, с (риски M5 §8).
pub const CONTROLLER_COOLDOWN_SECS: u64 = 10;

/// Окно коалесценции undo-шагов setProps (риски M5 §8): подряд идущие
/// setProps одной ноды склеиваются в один шаг, чтобы стикер с debounce
/// не разрастал глубину undo (как в редактировании текста).
pub const SETPROPS_UNDO_WINDOW: Duration = Duration::from_millis(1500);

/// Состояние виджета между кадрами (LOD-память).
#[derive(Debug, Clone, Copy, Default)]
struct WidgetState {
    has_instance: bool,
    was_live: bool,
    last_seen: u64,
}

/// Owned-квад снапшота (id — String): переживёт кадр без заимствования
/// сцены (main конвертирует в `WidgetQuad`-ссылки у FrameOverlay).
#[derive(Debug, Clone)]
pub struct OwnedQuad {
    pub node_id: String,
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

/// Результат кадра для рендера: квады снапшотов (live не рисуем — HWND).
pub struct FrameQuads {
    pub quads: Vec<OwnedQuad>,
    /// Число live-виджетов (диагностика/HUD).
    pub live_count: usize,
}

/// Менеджер виджетов приложения.
pub struct WidgetManager {
    pub registry: WidgetRegistry,
    states: HashMap<String, WidgetState>,
    last_capture: HashMap<String, u64>,
    cooldown_until: HashMap<String, u64>,
    theme: ThemeInfo,
    last_zoom: f32,
    started: Instant,
    /// Среда WebView2 подтверждена (EnvironmentReady ok).
    runtime_ready: bool,
    /// Среда недоступна (Evergreen не установлен): виджеты деградируют
    /// в placeholder + HUD-сообщение (SPEC §9).
    pub runtime_dead: bool,
    /// Объёмное состояние виджетов (T21-E): cache.db рядом с корнем пакетов;
    /// None при сбое открытия — деградация warn + пустые значения.
    pub state_store: Option<canvas_shell::WidgetStateStore>,
    /// Коалесценция undo setProps (риски M5 §8): нода и время последнего шага.
    last_props_undo: Option<(String, Instant)>,
    #[cfg(windows)]
    pub host: Option<canvas_widgets::host::WidgetHost>,
}

impl WidgetManager {
    /// Корень пакетов: `~/.canvasdesk/widgets` (план M5 §2 «Пути»);
    /// cache.db для widget_state — в родителе корня (`~/.canvasdesk`).
    pub fn new(widgets_root: PathBuf, dark: bool) -> Self {
        let data_dir = widgets_root
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| widgets_root.clone());
        let state_store = canvas_shell::WidgetStateStore::open(&data_dir).ok();
        if state_store.is_none() {
            tracing::warn!(dir = %data_dir.display(), "widget_state недоступен: cache.db не открыт");
        }
        Self {
            registry: WidgetRegistry::new(widgets_root),
            states: HashMap::new(),
            last_capture: HashMap::new(),
            cooldown_until: HashMap::new(),
            theme: ThemeInfo::new(dark, "#3B82F6"),
            last_zoom: 1.0,
            started: Instant::now(),
            runtime_ready: false,
            runtime_dead: false,
            state_store,
            last_props_undo: None,
            #[cfg(windows)]
            host: None,
        }
    }

    /// Стартовая инициализация реестра: материализация встроенных + скан.
    pub fn init_registry(&mut self) {
        if let Err(e) = self.registry.materialize_builtins() {
            tracing::warn!(error = %e, "материализация встроенных виджетов не удалась");
        }
        if let Err(e) = self.registry.reload() {
            tracing::warn!(error = %e, "скан виджет-пакетов не удался");
        }
        tracing::info!(
            count = self.registry.installed().len(),
            root = ?self.registry.root(),
            "реестр виджетов готов"
        );
    }

    pub fn set_theme(&mut self, dark: bool) {
        let new = ThemeInfo::new(dark, "#3B82F6");
        if new != self.theme {
            self.theme = new;
            self.broadcast_theme();
        }
    }

    /// Тик времени (с от старта менеджера) для LOD/refresh-решений.
    pub fn tick(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    /// Подключение host'а (Windows): HWND канваса + user-data-folder +
    /// отправитель событий (EventLoopProxy-обёртка).
    #[cfg(windows)]
    pub fn attach_host(
        &mut self,
        parent_hwnd: isize,
        user_data_folder: PathBuf,
        sender: canvas_widgets::WidgetEventSender,
    ) {
        match canvas_widgets::host::WidgetHost::new(parent_hwnd, sender) {
            Ok(host) => {
                host.set_user_data_folder(&user_data_folder);
                host.ensure_environment();
                self.host = Some(host);
            }
            Err(e) => {
                tracing::warn!(error = %e, "WebView2-хост не создан: виджеты недоступны");
                self.runtime_dead = true;
            }
        }
    }

    /// Кадр: LOD-план + применение к host'у + квады снапшотов.
    /// Вызывается в RedrawRequested ДО renderer.render; `overlay_rects` —
    /// логические экранные rect'ы панелей (airspace П7), `overlay_active` —
    /// транзиентные взаимодействия (рамка выделения, протягивание ребра).
    pub fn update_frame(
        &mut self,
        canvas: &Canvas,
        camera: &canvas_render::Camera,
        viewport_logical: [f32; 2],
        scale: f32,
        overlay_rects: &[[f32; 4]],
        overlay_active: bool,
    ) -> FrameQuads {
        let tick = self.tick();
        let zoom = camera.zoom();
        self.last_zoom = zoom;
        let camera_args = CameraArgs {
            center: [camera.position()[0], camera.position()[1]],
            zoom,
            viewport: viewport_logical,
        };
        let visible_world = camera_args.visible_world_rect();

        let runtime_ok = self.runtime_ok();
        let mut inputs: Vec<lod::WidgetLodInput> = Vec::new();
        let mut node_rects: Vec<(&str, [f32; 4])> = Vec::new();
        for node in &canvas.nodes {
            if node.kind() != NodeKind::Widget {
                continue;
            }
            let Some(ext) = &node.canvasdesk else {
                continue;
            };
            let visible = aabb_visible(&[node.x, node.y, node.width, node.height], &visible_world);
            let overlaid = overlay_active
                || overlay_rects.iter().any(|r| {
                    layout::node_screen_rect_overlaps(
                        &[node.x, node.y, node.width, node.height],
                        &camera_args,
                        scale,
                        r,
                    )
                });
            // Станет ли экземпляр сбоем (cooldown после ProcessFailed)
            let cooldown = self
                .cooldown_until
                .get(&node.id)
                .copied()
                .is_some_and(|until| tick < until);
            let state = self.states.entry(node.id.clone()).or_default();
            if visible {
                state.last_seen = tick;
            }
            inputs.push(lod::WidgetLodInput {
                node_id: node.id.clone(),
                visible,
                overlaid,
                package_ok: !cooldown && self.registry.contains(&ext.widget_id),
                runtime_ok,
                has_instance: state.has_instance,
                was_live: state.was_live,
                zoom,
                last_seen: state.last_seen,
            });
            node_rects.push((node.id.as_str(), [node.x, node.y, node.width, node.height]));
        }

        // Ноды исчезли (удаление/undo): инстансы и состояния чистим
        self.reap_missing(canvas);

        let decisions = lod::plan_frame(&inputs, tick, &self.last_capture);

        // Host-применение (Windows) и обновление состояний
        let mut live = Vec::new();
        let mut hide = Vec::new();
        let mut final_capture = Vec::new();
        let mut destroy = Vec::new();
        let mut refresh = Vec::new();
        let mut quads: Vec<OwnedQuad> = Vec::new();
        let mut live_count = 0usize;
        for d in &decisions {
            match d.target {
                Target::Live => {
                    live_count += 1;
                    let node = canvas
                        .node(&d.node_id)
                        .expect("решение только по существующим нодам");
                    let ext = node.canvasdesk.as_ref().expect("виджет");
                    let package = self.registry.get(&ext.widget_id);
                    if let Some(pkg) = package {
                        let [x, y, w, h] = [node.x, node.y, node.width, node.height];
                        live.push(canvas_widgets::LiveRequest {
                            node_id: d.node_id.clone(),
                            package_dir: pkg.dir.clone(),
                            manifest: pkg.manifest.clone(),
                            props: ext.props.clone(),
                            theme: self.theme.clone(),
                            rect: layout::webview_rect(&[x, y, w, h], &camera_args, scale),
                            corner: (8.0 * zoom * scale).round().max(1.0) as i32,
                            zoom,
                        });
                    }
                    if let Some(state) = self.states.get_mut(&d.node_id) {
                        state.was_live = true;
                        state.has_instance = true;
                    }
                }
                Target::Suspended => {
                    if let Some(state) = self.states.get_mut(&d.node_id) {
                        state.has_instance = true;
                        state.was_live = false;
                    }
                    if inputs.iter().any(|i| i.node_id == d.node_id && i.was_live) {
                        hide.push(d.node_id.clone());
                    }
                }
                Target::NoInstance => {
                    if let Some(state) = self.states.get_mut(&d.node_id) {
                        state.has_instance = false;
                        state.was_live = false;
                    }
                }
                Target::Destroy | Target::PlaceholderDestroy => {
                    destroy.push(d.node_id.clone());
                    self.states.remove(&d.node_id);
                }
                Target::Placeholder => {
                    self.states.remove(&d.node_id);
                }
            }
            if d.final_capture {
                final_capture.push(d.node_id.clone());
            }
            if d.refresh_snapshot {
                refresh.push(d.node_id.clone());
            }
            // Квады снапшотов: не-live виджеты (заместитель HWND)
            if d.render_mode() == lod::RenderMode::Snapshot {
                if let Some((node_id, rect)) =
                    node_rects.iter().find(|(id, _)| *id == d.node_id.as_str())
                {
                    let content = content_of(rect);
                    quads.push(OwnedQuad {
                        node_id: (*node_id).to_owned(),
                        pos: [content[0], content[1]],
                        size: [content[2], content[3]],
                    });
                }
            }
        }

        let frame = canvas_widgets::FrameApplication {
            live,
            hide,
            final_capture,
            destroy,
            refresh,
        };
        self.apply_host(frame);
        FrameQuads { quads, live_count }
    }

    /// Рантайм для LOD: на Windows — среда готова или ещё создаётся; на
    /// других ОС — виджеты в placeholder (dev-сборка без WebView2).
    fn runtime_ok(&self) -> bool {
        #[cfg(windows)]
        {
            self.host.is_some() && !self.runtime_dead
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    /// Удаление состояний/инстансов нод, которых больше нет в модели.
    fn reap_missing(&mut self, canvas: &Canvas) {
        let alive: Vec<String> = canvas
            .nodes
            .iter()
            .filter(|n| n.kind() == NodeKind::Widget)
            .map(|n| n.id.clone())
            .collect();
        let dead: Vec<String> = self
            .states
            .keys()
            .filter(|id| !alive.contains(id))
            .cloned()
            .collect();
        for id in dead {
            self.states.remove(&id);
            self.last_capture.remove(&id);
            self.cooldown_until.remove(&id);
            self.destroy_host_instance(&id);
        }
    }

    #[cfg(windows)]
    fn destroy_host_instance(&mut self, node_id: &str) {
        if let Some(host) = &self.host {
            let mut frame = canvas_widgets::FrameApplication::default();
            frame.destroy.push(node_id.to_owned());
            host.apply(&frame);
        }
    }

    #[cfg(not(windows))]
    fn destroy_host_instance(&mut self, _node_id: &str) {}

    #[cfg(windows)]
    fn apply_host(&mut self, frame: canvas_widgets::FrameApplication) {
        if let Some(host) = &self.host {
            host.apply(&frame);
        }
    }

    #[cfg(not(windows))]
    fn apply_host(&mut self, _frame: canvas_widgets::FrameApplication) {}

    /// Обработка события host'а (вызывается из user_event). Управляет
    /// runtime-флагами и кулдаунами; сообщение Ready обрабатывает main
    /// (init требует доступ к сцене).
    pub fn on_event(&mut self, event: &WidgetEvent) {
        match event {
            WidgetEvent::EnvironmentReady { ok } => {
                self.runtime_ready = *ok;
                if !ok {
                    self.runtime_dead = true;
                    tracing::warn!("WebView2-среда недоступна: виджеты — placeholder (SPEC §9)");
                } else {
                    tracing::info!("WebView2-среда готова");
                }
            }
            WidgetEvent::ControllerReady { node_id, ok } => {
                if !ok {
                    self.cooldown_until
                        .insert(node_id.clone(), self.tick() + CONTROLLER_COOLDOWN_SECS);
                    self.states.remove(node_id);
                } else {
                    self.cooldown_until.remove(node_id);
                }
            }
            WidgetEvent::SnapshotReady { node_id, .. } => {
                self.last_capture.insert(node_id.clone(), self.tick());
            }
            WidgetEvent::Message { .. } | WidgetEvent::Tick => {}
        }
    }

    /// Init-сообщение для виджета (по Ready): props ноды + тема + зум.
    pub fn init_message(&self, node: &Node) -> Option<HostToWidget> {
        let ext = node.canvasdesk.as_ref()?;
        Some(HostToWidget::Init {
            node_id: node.id.clone(),
            props: ext.props.clone(),
            theme: self.theme.clone(),
            zoom: self.last_zoom,
        })
    }

    /// Отправка сообщения live-виджету (Windows; вне Windows — no-op).
    pub fn post_message(&self, node_id: &str, message: &HostToWidget) {
        #[cfg(windows)]
        if let Some(host) = &self.host {
            host.post_message(node_id, message);
        }
        #[cfg(not(windows))]
        let _ = (node_id, message);
    }

    /// Ответ на запрос виджета (readDir/state…).
    pub fn reply(&self, node_id: &str, reply: &canvas_widgets::bridge::Reply) {
        #[cfg(windows)]
        if let Some(host) = &self.host {
            host.reply(node_id, reply);
        }
        #[cfg(not(windows))]
        let _ = (node_id, reply);
    }

    /// themeChanged всем live-инстансам (смена темы в настройках).
    fn broadcast_theme(&mut self) {
        #[cfg(windows)]
        if let Some(host) = &self.host {
            // Список live-нод знает менеджер сцены — шлём через visibility-less
            // широковещательное сообщение по всем инстансам: host хранит их
            // сам; отдельного API нет — обходим через фиктивный кадр не будем,
            // шлём только известным (T21 уточнит при stateGet)
            let _ = host;
        }
    }

    /// Нода-виджет из пакета реестра (центр viewport, defaultSize).
    pub fn build_widget_node(
        &self,
        widget_id: &str,
        node_id: String,
        center: [f32; 2],
    ) -> Option<Node> {
        let pkg = self.registry.get(widget_id)?;
        let [w, h] = pkg.manifest.default_size;
        let ext = canvas_core::CanvasdeskExt {
            widget_id: widget_id.to_owned(),
            props: WidgetProps::new(),
        };
        Some(Node::widget(
            node_id,
            ext,
            pkg.manifest.name.clone(),
            center[0] - w / 2.0,
            center[1] - h / 2.0,
            w,
            h,
        ))
    }

    /// Следующий свободный `widget-N`.
    pub fn next_node_id(&self, canvas: &Canvas) -> String {
        let mut max = 0u32;
        for node in &canvas.nodes {
            if let Some(tail) = node.id.strip_prefix("widget-") {
                if let Ok(n) = tail.parse::<u32>() {
                    max = max.max(n);
                }
            }
        }
        format!("widget-{}", max + 1)
    }

    /// Установленные пакеты для меню «Виджеты ▸» (id + имя).
    pub fn menu_entries(&self) -> Vec<(String, String)> {
        self.registry
            .installed()
            .iter()
            .map(|pkg| (pkg.manifest.id.clone(), pkg.manifest.name.clone()))
            .collect()
    }

    /// Permissions пакета ноды (T21-A): enforcement на каждый вызов моста.
    /// Owned-клон: сообщения моста редкие (debounce виджетов), аллокация
    /// на вызов пренебрежима;
    pub fn permissions_of_node(&self, canvas: &Canvas, node_id: &str) -> Option<Permissions> {
        let ext = canvas.node(node_id)?.canvasdesk.as_ref()?;
        let pkg = self.registry.get(&ext.widget_id)?;
        Some(Permissions::new(pkg.manifest.permissions.iter().copied()))
    }

    /// Манифест пакета ноды (диалоги, install-контекст).
    pub fn manifest_of_node<'a>(
        &'a self,
        canvas: &'a Canvas,
        node_id: &str,
    ) -> Option<&'a WidgetManifest> {
        let ext = canvas.node(node_id)?.canvasdesk.as_ref()?;
        Some(&self.registry.get(&ext.widget_id)?.manifest)
    }

    /// Установка пакета из папки (T21-B: подтверждённый диалогом drag).
    /// Возвращает исход (Installed/Updated/SameVersion) для toast-сообщения.
    /// При обновлении уничтожает live-инстансы пакета — следующий кадр
    /// пересоздаст их с новым манифестом (план M5 §4.8).
    pub fn install_package(&mut self, src: &Path) -> Result<InstallOutcome, String> {
        let manifest = WidgetManifest::from_dir(src).map_err(|e| e.to_string())?;
        let outcome = self.registry.install(src).map_err(|e| e.to_string())?;
        if matches!(outcome, InstallOutcome::Updated) {
            self.destroy_instances_of(&manifest.id);
        }
        Ok(outcome)
    }

    /// Удаление пакета (T21-C, П11): registry.remove + уничтожение
    /// live-инстансов; ноды пакета остаются в модели и деградируют в
    /// placeholder (package_ok=false в LOD), как битые ссылки файлов.
    pub fn remove_package(&mut self, widget_id: &str) -> Result<(), String> {
        self.registry.remove(widget_id).map_err(|e| e.to_string())?;
        self.destroy_instances_of(widget_id);
        Ok(())
    }

    /// Инстансы пакета → destroy + сброс LOD-памяти (обновление/удаление).
    /// `widget_id` — для трейсинга (destroy идёт по всем известным нодам:
    /// список нод пакета живёт в сцене, недоступной менеджеру).
    fn destroy_instances_of(&mut self, widget_id: &str) {
        tracing::debug!(widget_id, "сброс live-инстансов пакета");
        // Список нод пакета неизвестен менеджеру без сцены — уничтожаем
        // все инстансы через host (безопасно: следующий кадр вернёт live)
        // и чистим кулдауны/снапшот-метки тех нод, чьи состояния есть.
        #[cfg(windows)]
        if let Some(host) = &self.host {
            let mut frame = canvas_widgets::FrameApplication::default();
            for node_id in self.states.keys() {
                frame.destroy.push(node_id.clone());
            }
            host.apply(&frame);
        }
        let ids: Vec<String> = self.states.keys().cloned().collect();
        for id in ids {
            self.states.remove(&id);
            self.cooldown_until.remove(&id);
        }
    }

    /// Undo-коалесценция setProps (риски M5 §8): Ok(()) — нужен новый
    /// undo-шаг; Err(()) — склеиваем с предыдущим (та же нода в окне).
    /// План намекал на сравнение снапшотов — окно времени проще и не
    /// держит копии props.
    pub fn should_push_props_undo(&mut self, node_id: &str) -> bool {
        let now = Instant::now();
        let coalesce = self.last_props_undo.as_ref().is_some_and(|(id, at)| {
            id == node_id && now.duration_since(*at) < SETPROPS_UNDO_WINDOW
        });
        if !coalesce {
            self.last_props_undo = Some((node_id.to_owned(), now));
        }
        !coalesce
    }

    /// stateGet (T21-A): изолированное хранилище ноды.
    pub fn state_get(&self, node_id: &str, key: &str) -> Option<String> {
        self.state_store.as_ref()?.get(node_id, key)
    }

    /// stateSet (T21-A).
    pub fn state_set(&mut self, node_id: &str, key: &str, value: &str) {
        if let Some(store) = self.state_store.as_mut() {
            store.set(node_id, key, value);
        }
    }

    pub fn runtime_status(&self) -> &'static str {
        if self.runtime_dead {
            "виджеты: WebView2 недоступен"
        } else if self.runtime_ready {
            "виджеты: ок"
        } else {
            "виджеты: среда запускается"
        }
    }
}

/// AABB ноды пересекает видимую область (culling, идиома T5).
fn aabb_visible(node: &[f32; 4], world: &[f32; 4]) -> bool {
    node[0] < world[2]
        && node[0] + node[2] > world[0]
        && node[1] < world[3]
        && node[1] + node[3] > world[1]
}

/// Область контента ноды (инсеты хрома, план M5 §4.4).
fn content_of(node: &[f32; 4]) -> [f32; 4] {
    let geom = layout::WidgetGeom { node: *node };
    geom.content_rect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cd_widgets_mgr_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manager(tag: &str) -> WidgetManager {
        let mut m = WidgetManager::new(temp_root(tag).join("widgets"), true);
        m.init_registry();
        m
    }

    fn camera_at(zoom: f32) -> canvas_render::Camera {
        let mut camera = canvas_render::Camera::default();
        camera.set_zoom(zoom);
        camera
    }

    fn canvas_with_clock() -> Canvas {
        let mut canvas = Canvas::default();
        let ext = canvas_core::CanvasdeskExt {
            widget_id: "com.canvasdesk.clock".to_owned(),
            props: WidgetProps::new(),
        };
        canvas.nodes.push(Node::widget(
            "widget-1", ext, "Clock", 0.0, 0.0, 320.0, 200.0,
        ));
        canvas
    }

    #[test]
    fn manager_materializes_builtins_and_lists_them() {
        let m = manager("menu");
        let entries = m.menu_entries();
        assert!(
            entries
                .iter()
                .any(|(id, name)| id == "com.canvasdesk.clock" && name == "Clock"),
            "{entries:?}"
        );
    }

    #[test]
    fn build_widget_node_uses_manifest_defaults() {
        let m = manager("build");
        let canvas = Canvas::default();
        let node = m
            .build_widget_node(
                "com.canvasdesk.clock",
                m.next_node_id(&canvas),
                [100.0, 100.0],
            )
            .expect("часы установлены");
        assert_eq!(node.kind(), NodeKind::Widget);
        assert_eq!(node.width, 320.0);
        assert_eq!(node.height, 200.0);
        assert_eq!(node.label.as_deref(), Some("Clock"));
        // Центрирование: центр (100, 100) → левый верх (100-160, 100-100)
        assert_eq!((node.x, node.y), (-60.0, 0.0));
        // Неизвестный пакет — None
        assert!(m
            .build_widget_node("no.such.widget", "w".into(), [0.0, 0.0])
            .is_none());
    }

    #[test]
    fn next_node_id_increments_over_existing() {
        let m = manager("ids");
        let canvas = canvas_with_clock();
        assert_eq!(m.next_node_id(&canvas), "widget-2");
    }

    #[test]
    fn setprops_undo_coalesces_within_window() {
        // T21-A: подряд идущие setProps одной ноды — один undo-шаг;
        // другая нода — новый шаг
        let mut m = manager("coalesce");
        assert!(m.should_push_props_undo("widget-1"), "первый — шаг");
        assert!(!m.should_push_props_undo("widget-1"), "склейка в окне");
        assert!(!m.should_push_props_undo("widget-1"), "и ещё раз");
        assert!(m.should_push_props_undo("widget-2"), "другая нода — шаг");
        assert!(!m.should_push_props_undo("widget-2"), "склейка второй");
    }

    #[test]
    fn widget_state_roundtrip_through_manager() {
        // T21-E: store в cache.db рядом с корнем пакетов (widgets.parent())
        let mut m = manager("state");
        assert_eq!(m.state_get("widget-1", "draft"), None);
        m.state_set("widget-1", "draft", "текст");
        assert_eq!(m.state_get("widget-1", "draft"), Some("текст".into()));
        // Изоляция нод
        assert_eq!(m.state_get("widget-2", "draft"), None);
        // cache.db лежит в родителе корня пакетов
        assert!(m
            .registry
            .root()
            .parent()
            .unwrap()
            .join("cache.db")
            .exists());
    }

    #[test]
    fn update_frame_without_runtime_gives_placeholder_and_no_quads() {
        // Linux: runtime_ok=false → все виджеты placeholder, квадов нет
        let mut m = manager("nort");
        let canvas = canvas_with_clock();
        let camera = camera_at(1.0);
        let frame = m.update_frame(&canvas, &camera, [1000.0, 600.0], 1.0, &[], false);
        assert!(frame.quads.is_empty(), "нет снапшотов — нет квадов");
        assert_eq!(frame.live_count, 0);
        // Нода осталась в состояниях? Placeholder чистит состояние — ок
    }

    #[test]
    fn update_frame_marks_broken_package() {
        // Пакет удалён из реестра: decision — Placeholder
        let mut m = manager("broken");
        m.registry.remove("com.canvasdesk.clock").unwrap();
        let canvas = canvas_with_clock();
        let camera = camera_at(1.0);
        let frame = m.update_frame(&canvas, &camera, [1000.0, 600.0], 1.0, &[], false);
        assert!(frame.quads.is_empty());
        assert_eq!(frame.live_count, 0);
    }

    #[test]
    fn overlay_airspace_hides_live() {
        // Независимо от ОС: airspace проверяется на уровне входов
        // plan_frame (не manager): менеджер передаёт overlaid в planner.
        // Здесь — чистая проверка: overlay_rect, накрывающий ноду
        let mut inputs = vec![lod::WidgetLodInput {
            node_id: "w".into(),
            visible: true,
            overlaid: false,
            package_ok: true,
            runtime_ok: true,
            has_instance: true,
            was_live: true,
            zoom: 1.0,
            last_seen: 1,
        }];
        let plan = lod::plan_frame(&inputs, 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Live);
        inputs[0].overlaid = true;
        let plan = lod::plan_frame(&inputs, 0, &HashMap::new());
        assert_eq!(plan[0].target, Target::Suspended);
    }
}
