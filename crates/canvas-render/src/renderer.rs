//! Оконный рендер: surface, конфигурация, сетка поверх clear-прохода (T1, T2).

use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use canvas_core::{curve_point, edge_curve, Canvas, Side, SpatialIndex, Thumbnail};

use crate::camera::{Camera, Vec2};
use crate::cards::{
    build_draft_instances, build_edge_instances, build_port_instances, card_instance, CardInstance,
    CardsPipeline, SELECTION_BORDER,
};
use crate::config::{
    background_color, choose_present_mode, choose_surface_format, surface_size_valid,
};
use crate::edit::{session_area, EditTarget, EditingSession};
use crate::gpu::GpuContext;
use crate::grid::GridPipeline;
use crate::minimap::MinimapImage;
use crate::minimap_pass::{quad_rect, quad_rect_logical, MinimapPipeline, MinimapTexture};
use crate::text::{
    body_area, titles_visible, EdgeLabel, OverlayText, ScreenText, TextSystem, TitleFrame,
};
use crate::thumbs::{thumb_instance, ThumbsPipeline, THUMB_MIN_ZOOM};
use crate::zorder;

/// Заливка выделения текста в редакторе (T7) — акцент с прозрачностью.
const TEXT_SELECTION_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.35];
/// Фон-подсветка `==текст==` в заметках — приглушённый жёлтый с прозрачностью.
const HIGHLIGHT_FILL: [f32; 4] = [0.85, 0.75, 0.30, 0.30];
/// Фон-подложка лейбла связи (T8) — тёмный, полупрозрачный.
const EDGE_LABEL_FILL: [f32; 4] = [0.11, 0.11, 0.13, 0.85];
/// Отступы подложки лейбла связи вокруг текста (world-px, по осям x и y).
const EDGE_LABEL_PADDING: [f32; 2] = [6.0, 3.0];
/// Фон бокса редактирования лейбла связи (T8) — у связи нет карточки.
const EDGE_EDIT_FILL: [f32; 4] = [0.13, 0.13, 0.16, 0.95];
/// Потолок текст-групп кадра (включая финальную): сегменты сверх потолка
/// теряют свою группу — их тексты рисуются в финальной поверх всего.
/// Защита от патологически глубоких каскадов перекрытий.
const MAX_TEXT_GROUPS: usize = 16;

/// Screen-space инстанс (логические px от левого верхнего угла окна) →
/// world-инстанс текущей камеры: на экране размер константен при любом зуме.
fn screen_instance_to_world(camera: &Camera, viewport: Vec2, inst: &CardInstance) -> CardInstance {
    let zoom = camera.zoom();
    let mut out = *inst;
    out.pos = camera.screen_to_world(inst.pos, viewport);
    out.size = [inst.size[0] / zoom, inst.size[1] / zoom];
    // Радиус скругления (params.x) тоже задан в логических px
    out.params[0] /= zoom;
    out
}

/// Оверлеи кадра от приложения (контекстное меню T7, панель настроек):
/// дополнительные инстансы квадов (поверх карточек, под текстом) и подписи.
/// `instances`/`texts` — world-координаты (масштабируются зумом);
/// `screen_instances`/`screen_texts` — логические px от угла окна,
/// константный размер при любом зуме.
pub struct FrameOverlay<'a> {
    pub instances: &'a [CardInstance],
    pub texts: &'a [OverlayText<'a>],
    pub screen_instances: &'a [CardInstance],
    pub screen_texts: &'a [ScreenText<'a>],
}

impl FrameOverlay<'_> {
    /// Пустой оверлей.
    pub const EMPTY: FrameOverlay<'static> = FrameOverlay {
        instances: &[],
        texts: &[],
        screen_instances: &[],
        screen_texts: &[],
    };
}

/// Выделение на канвасе (T8): нода или связь (индексы в canvas.nodes/edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Node(usize),
    Edge(usize),
}

/// Сцена кадра: модель канваса, spatial index (culling, T5), выделение
/// и интерактивные состояния связей (T8).
pub struct SceneView<'a> {
    pub canvas: &'a Canvas,
    pub spatial: &'a SpatialIndex,
    pub selected: Option<Selection>,
    /// Нода под курсором (hover, T8): рисуются порты для начала drag связи.
    pub hovered: Option<usize>,
    /// Резиновая линия новой связи (T8): (точка порта, сторона, курсор world).
    pub edge_draft: Option<([f32; 2], Side, [f32; 2])>,
}

/// Счётчики отрисованного кадра (T5) — для HUD и проверки culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Всего нод в сцене.
    pub total_nodes: usize,
    /// Нод попало в viewport (прошли culling).
    pub visible_nodes: usize,
    /// Всего связей в сцене.
    pub total_edges: usize,
    /// Связей, чьи ноды видны (принадлежат видимым нодам).
    pub visible_edges: usize,
    /// Инстансов карточек ушло в draw.
    pub instances: u32,
    /// CPU-время подготовки и кодирования кадра, мс.
    pub cpu_ms: f32,
}

/// Рендерер окна: владеет surface и выполняет кадр по запросу (`request_redraw`).
pub struct Renderer {
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    grid: GridPipeline,
    cards: CardsPipeline,
    /// Атлас тамбнейлов + их пайплайн (T6).
    thumbs: ThumbsPipeline,
    text: TextSystem,
    /// Пайплайн миникарты (T13-B) + текущий кадр (None — не задан).
    minimap_pipeline: MinimapPipeline,
    minimap: Option<MinimapTexture>,
    scale_factor: f32,
    /// Рисовать сетку канваса (настройки, панель из post-T7).
    grid_visible: bool,
}

impl Renderer {
    /// Создать рендерер для окна. Вызывается один раз при старте
    /// (блокирующе, через `pollster` в canvas-app).
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .context("создание surface")?;
        let gpu = GpuContext::new(instance, Some(&surface))
            .await
            .context("GPU-адаптер не найден")?;

        let caps = surface.get_capabilities(&gpu.adapter);
        let format = choose_surface_format(&caps.formats);
        let present_mode = choose_present_mode(&caps.present_modes);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Auto),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        if surface_size_valid(size.width, size.height) {
            surface.configure(&gpu.device, &config);
        }

        let info = gpu.adapter.get_info();
        tracing::info!(
            adapter = %info.name,
            backend = ?info.backend,
            ?format,
            ?present_mode,
            "рендер инициализирован"
        );
        let grid = GridPipeline::new(&gpu.device, format);
        let cards = CardsPipeline::new(&gpu.device, format);
        let thumbs = ThumbsPipeline::new(&gpu.device, format);
        let minimap_pipeline = MinimapPipeline::new(&gpu.device, format);
        let text = TextSystem::new(&gpu.device, &gpu.queue, format);
        Ok(Self {
            gpu,
            surface,
            config,
            size,
            grid,
            cards,
            thumbs,
            text,
            minimap_pipeline,
            minimap: None,
            scale_factor: scale_factor as f32,
            grid_visible: true,
        })
    }

    /// Включить/выключить сетку канваса (настройки).
    pub fn set_grid_visible(&mut self, visible: bool) {
        self.grid_visible = visible;
    }

    /// Обновить scale factor окна (перенос между мониторами с разным DPI, SPEC §6.5).
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
    }

    /// Загрузить готовый тамбнейл ноды в атлас (T6). Вызывается из app
    /// по результатам ThumbService; аплоад идёт в write_texture до кадра.
    pub fn set_thumbnail(&mut self, node: usize, thumb: &Thumbnail) {
        self.thumbs.insert(&self.gpu.queue, node, thumb);
    }

    /// Есть ли у ноды тамбнейл в атласе — дедупликация заказов в app (T6).
    pub fn has_thumbnail(&self, node: usize) -> bool {
        self.thumbs.contains(node)
    }

    /// Число тамбнейлов в атласе (HUD, T6).
    pub fn thumbnail_count(&self) -> usize {
        self.thumbs.len()
    }

    /// Загрузить кадр миникарты (T13-B): растеризация T13-A передаётся в
    /// текстуру RGBA8 с bind group; при смене размера текстура пересоздаётся.
    /// Кадр 0×0 (MinimapImage::EMPTY) сбрасывает миникарту. При resize/DPI-смене
    /// текстура НЕ очищается — квад сам уедет за too-small-границу, а при
    /// смене scale_factor приложение перезагрузит кадр этим же методом.
    pub fn set_minimap(&mut self, image: &MinimapImage) {
        if image.width == 0 || image.height == 0 {
            self.minimap = None;
            return;
        }
        let expected = image.width as usize * image.height as usize * 4;
        if image.rgba.len() != expected {
            tracing::warn!(
                w = image.width,
                h = image.height,
                len = image.rgba.len(),
                "миникарта: буфер не совпадает с width×height×4 — кадр пропущен"
            );
            return;
        }
        let current = self.minimap.take();
        let texture =
            self.minimap_pipeline
                .upload(&self.gpu.device, &self.gpu.queue, current, image);
        self.minimap = Some(texture);
    }

    /// Прямоугольник миникарты в ЛОГИЧЕСКИХ px — hit-test приложения
    /// (T13-C): координаты курсора winit — логические. None — миникарта не
    /// задана или окно меньше 252×172 логических px (миникарта скрыта).
    pub fn minimap_rect_logical(&self) -> Option<[f32; 4]> {
        self.minimap.as_ref().and_then(|_| {
            quad_rect_logical(
                self.size.width as f32 / self.scale_factor,
                self.size.height as f32 / self.scale_factor,
            )
        })
    }

    /// Полная инвалидация кэшей по индексам нод (T8): после удаления ноды
    /// индексы сдвигаются — текстовый кэш и атлас тамбнейлов сбрасываются;
    /// тамбнейлы перезапросятся лениво из ThumbService/SQLite (SPEC §6.4).
    pub fn invalidate_node_caches(&mut self) {
        self.text.invalidate_all();
        self.thumbs.clear();
    }

    /// Доступ к FontSystem для операций EditingSession (T7) из приложения:
    /// ввод, клики, копирование — все шейпинг-операции идут через него.
    pub fn font_system_mut(&mut self) -> &mut glyphon::FontSystem {
        self.text.font_system_mut()
    }

    /// Оверлей-квад (каретка/выделение, T7): rect в пикселях буфера редактора
    /// → world-координаты относительно `origin` (левый верхний угол области).
    fn overlay_quad(
        origin: [f32; 2],
        rect: [f32; 4],
        zoom_px: f32,
        fill: [f32; 4],
    ) -> CardInstance {
        CardInstance {
            pos: [origin[0] + rect[0] / zoom_px, origin[1] + rect[1] / zoom_px],
            size: [rect[2] / zoom_px, rect[3] / zoom_px],
            fill,
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Переконфигурировать surface под новый размер окна.
    /// Нулевой размер (свёрнутое окно) игнорируется — кадр пропускается.
    pub fn resize(&mut self, width: u32, height: u32) {
        if !surface_size_valid(width, height) {
            return;
        }
        self.size = PhysicalSize::new(width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.gpu.device, &self.config);
    }

    /// Отрисовать кадр: фон, сетка, связи (T8), карточки видимых нод,
    /// заголовки, лейблы связей, HUD (T2/T4/T5).
    /// `hud` — строка оверлея (F3), None — без оверлея. Возвращает счётчики кадра.
    /// `editing` — активная сессия редактирования (T7/T8): её буфер рисуется
    /// вместо кэшированного тела ноды/лейбла, поверх — каретка и выделение.
    pub fn render(
        &mut self,
        camera: &Camera,
        scene: &SceneView,
        hud: Option<&str>,
        mut editing: Option<&mut EditingSession>,
        overlay: &FrameOverlay,
    ) -> anyhow::Result<FrameStats> {
        let cpu_start = std::time::Instant::now();
        if !surface_size_valid(self.size.width, self.size.height) {
            return Ok(FrameStats::default());
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // surface потерян/устарел (например, после смены DPI) — переконфигурация
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(FrameStats::default());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                anyhow::bail!("GPU: нехватка памяти под surface");
            }
            Err(err) => {
                tracing::warn!(?err, "кадр пропущен");
                return Ok(FrameStats::default());
            }
        };

        // Culling (T5): видимый world-rect → индексы видимых нод из spatial index
        let viewport_logical = [
            self.size.width as f32 / self.scale_factor,
            self.size.height as f32 / self.scale_factor,
        ];
        let visible = camera.visible_world_rect(viewport_logical);
        let indices = scene.spatial.query_rect(visible);

        // Актуальные метрики буфера редактирования под текущий зум (T7/T8) —
        // до вычисления каретки/выделения ниже
        let zoom_px = camera.zoom() * self.scale_factor;
        if let Some(session) = editing.as_deref_mut() {
            if let Some((_, width, height)) = session_area(scene.canvas, session) {
                session.set_layout(
                    self.text.font_system_mut(),
                    width * zoom_px,
                    height * zoom_px,
                    zoom_px,
                );
            }
        }

        // Разложить выделение по видам целей (T8)
        let (selected_node, selected_edge) = match scene.selected {
            Some(Selection::Node(index)) => (Some(index), None),
            Some(Selection::Edge(index)) => (None, Some(index)),
            None => (None, None),
        };
        // Лейбл редактируемой связи рисует сессия — из обычной выдачи исключён
        let editing_edge = editing
            .as_deref()
            .and_then(|session| match session.target() {
                EditTarget::Edge(index) => Some(index),
                EditTarget::Node(_) => None,
            });

        // Лейблы связей (T8): центр — середина кривой; подложка — квадом под
        // текстом по размеру из кэша шейпинга (перешейп при смене текста/зума)
        let mut edge_labels: Vec<EdgeLabel> = Vec::new();
        let mut label_backdrops: Vec<CardInstance> = Vec::new();
        if titles_visible(zoom_px) {
            for (index, edge) in scene.canvas.edges.iter().enumerate() {
                if editing_edge == Some(index) {
                    continue;
                }
                let Some(text) = edge.label.as_deref().filter(|text| !text.is_empty()) else {
                    continue;
                };
                let Some(curve) = edge_curve(scene.canvas, edge) else {
                    continue;
                };
                let center = curve_point(&curve, 0.5);
                let size = self.text.edge_label_size(&edge.id, text, zoom_px);
                let w = size[0] + EDGE_LABEL_PADDING[0] * 2.0;
                let h = size[1] + EDGE_LABEL_PADDING[1] * 2.0;
                label_backdrops.push(CardInstance {
                    pos: [center[0] - w / 2.0, center[1] - h / 2.0],
                    size: [w, h],
                    fill: EDGE_LABEL_FILL,
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
                edge_labels.push(EdgeLabel {
                    id: &edge.id,
                    text,
                    center,
                });
            }
        }

        if self.grid_visible {
            self.grid.update_camera(
                &self.gpu.queue,
                camera,
                [self.size.width as f32, self.size.height as f32],
                self.scale_factor,
            );
        }
        // Редакторские оверлеи (T7/T8): выделение и каретка. У ноды — квады на её
        // z-позиции (в z-проходе ниже: над её карточкой, под её текстом и под
        // перекрывающими карточками). У лейбла связи (T8) ноды нет — бокс и
        // квады идут в оверлей-регион поверх карточек, под текстом лейбла.
        let editing_node = editing.as_deref().and_then(EditingSession::node_index);
        let mut editing_quads: Vec<CardInstance> = Vec::new();
        let mut edge_edit_quads: Vec<CardInstance> = Vec::new();
        if let Some(session) = editing.as_deref_mut() {
            match session.target() {
                EditTarget::Node(_) => {
                    if let Some(node) = scene
                        .canvas
                        .nodes
                        .get(session.node_index().unwrap_or(usize::MAX))
                    {
                        let zoom_px = camera.zoom() * self.scale_factor;
                        let (origin, _, _) = body_area(node);
                        for rect in session.selection_rects(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                TEXT_SELECTION_FILL,
                            ));
                        }
                        if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                            editing_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                SELECTION_BORDER,
                            ));
                        }
                    }
                }
                EditTarget::Edge(_) => {
                    if let Some((origin, width, height)) = session_area(scene.canvas, session) {
                        // У лейбла связи нет карточки — бокс-подложка с рамкой
                        edge_edit_quads.push(CardInstance {
                            pos: origin,
                            size: [width, height],
                            fill: EDGE_EDIT_FILL,
                            border: SELECTION_BORDER,
                            params: [6.0, 1.0, 0.0, 0.0],
                        });
                        let zoom_px = camera.zoom() * self.scale_factor;
                        for rect in session.selection_rects(self.text.font_system_mut()) {
                            edge_edit_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                TEXT_SELECTION_FILL,
                            ));
                        }
                        if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                            edge_edit_quads.push(Self::overlay_quad(
                                origin,
                                rect,
                                zoom_px,
                                SELECTION_BORDER,
                            ));
                        }
                    }
                }
            }
        }

        // Z-план кадра (zorder.rs): видимые ноды (z-порядок = порядок в
        // Canvas.nodes) бьются на сегменты так, чтобы текст и тамбнейл ноды
        // рисовались после её карточки, но до перекрывающих её карточек.
        // Фикс наложения: раньше тамбнейлы и текст рисовались сплошными
        // проходами поверх всех карточек — иконка Word фоновой карточки
        // перекрывала заметки переднего плана, текст фоновой заметки лёг
        // поверх чужих карточек и текста.
        let zoom = camera.zoom();
        let show_titles = titles_visible(zoom * self.scale_factor);
        let rects: Vec<[f32; 4]> = indices
            .iter()
            .map(|&index| {
                scene
                    .canvas
                    .nodes
                    .get(index)
                    .map(|n| [n.x, n.y, n.x + n.width, n.y + n.height])
                    .unwrap_or([0.0, 0.0, 0.0, 0.0])
            })
            .collect();
        let has_thumb: Vec<bool> = indices
            .iter()
            .map(|&index| {
                zoom >= THUMB_MIN_ZOOM
                    && scene
                        .canvas
                        .nodes
                        .get(index)
                        .is_some_and(|n| n.file.is_some())
                    && self.thumbs.contains(index)
            })
            .collect();
        let has_text: Vec<bool> = indices
            .iter()
            .map(|&index| show_titles || editing_node == Some(index))
            .collect();
        let zplan = zorder::plan_z_order(&rects, &has_text, &has_thumb, MAX_TEXT_GROUPS);

        // Z-проход: инстансы карточек (карточка + квады подсветки/каретки на
        // z-позициях нод) и тамбнейлы, посегментно с границами для draw_range.
        let mut instances: Vec<CardInstance> = Vec::with_capacity(indices.len() + 8);
        let mut thumb_instances: Vec<crate::thumbs::ThumbInstance> = Vec::new();
        // Связи (T8) — ПОД карточками: depth-теста нет, порядок инстансов
        // в общем буфере = порядок рисования; рисуются диапазоном до сегментов
        instances.extend(build_edge_instances(scene.canvas, selected_edge));
        let edges_end = instances.len() as u32;
        // (диапазон инстансов карточек, диапазон тамбнейлов, текст-группа).
        let mut draw_ranges: Vec<(std::ops::Range<u32>, std::ops::Range<u32>, Option<usize>)> =
            Vec::new();
        for seg in &zplan.segments {
            let cards_start = instances.len() as u32;
            let thumbs_start = thumb_instances.len() as u32;
            for pos in seg.nodes.clone() {
                let Some(&index) = indices.get(pos) else {
                    continue;
                };
                let Some(node) = scene.canvas.nodes.get(index) else {
                    continue;
                };
                // Карточка ноды
                instances.push(card_instance(node, selected_node == Some(index)));
                // Фон-подсветка ==…== (форматирование): квады из кэша прошлого
                // шейпинга (при промахе появятся на следующий кадре) —
                // на z-позиции ноды, под её текстом и перекрывающими карточками
                if let Some((entry_zoom, highlight)) = self.text.highlight_rects(index) {
                    let (origin, _, _) = body_area(node);
                    for rect in highlight {
                        instances.push(Self::overlay_quad(
                            origin,
                            *rect,
                            entry_zoom,
                            HIGHLIGHT_FILL,
                        ));
                    }
                }
                // Выделение/каретка редактора (T7) — на z-позиции редактируемой ноды
                if editing_node == Some(index) {
                    instances.extend_from_slice(&editing_quads);
                }
                // Тамбнейл (T6): в сегменте своей ноды — под перекрывающими карточками
                if let Some(inst) =
                    thumb_instance(scene.canvas, index, self.thumbs.slots_mut(), zoom)
                {
                    thumb_instances.push(inst);
                }
            }
            draw_ranges.push((
                cards_start..instances.len() as u32,
                thumbs_start..thumb_instances.len() as u32,
                seg.group,
            ));
        }
        // T8: порты hover-ноды, резиновая линия новой связи, подложки лейблов и
        // бокс редактирования лейбла — поверх карточек всех сегментов, под их
        // текстом (лейблы рисуются финальной текст-группой ниже)
        if let Some(hovered) = scene.hovered {
            instances.extend(build_port_instances(scene.canvas, hovered));
        }
        if let Some((port, side, cursor)) = scene.edge_draft {
            instances.extend(build_draft_instances(port, side, cursor));
        }
        instances.extend_from_slice(&label_backdrops);
        instances.extend_from_slice(&edge_edit_quads);
        // Оверлеи приложения (контекстное меню, панель настроек) — поверх всех
        // карточек: расширяют диапазон карточек финального сегмента; их тексты
        // (подписи меню, строки панели) рисуются финальной текст-группой.
        instances.extend_from_slice(overlay.instances);
        for inst in overlay.screen_instances {
            instances.push(screen_instance_to_world(camera, viewport_logical, inst));
        }
        if let Some(last) = draw_ranges.last_mut() {
            last.0 = last.0.start..instances.len() as u32;
        }
        let instance_count = self.cards.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &instances,
        );
        // Тамбнейлы (T6): инстансы загружены в z-проходе; диапазоны
        // рисуются посегментно между карточками (draw_ranges)
        self.thumbs.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &thumb_instances,
        );
        // Актуальные метрики уже выставлены выше (до сборки оверлеев)
        let editing_ref = editing.as_deref();
        // Из кэша тела исключается только редактируемая НОДА; у лейбла связи
        // (T8) кэшированного тела нет — исключать нечего
        let editing_index = editing_ref.and_then(EditingSession::node_index);
        let editing_buffer = editing_ref.and_then(|session| {
            session_area(scene.canvas, session).map(|(origin, _, _)| (session.buffer(), origin))
        });
        if let Err(err) = self.text.prepare_titles(
            &self.gpu.device,
            &self.gpu.queue,
            &TitleFrame {
                camera,
                viewport_physical: [self.size.width, self.size.height],
                scale_factor: self.scale_factor,
                canvas: scene.canvas,
                indices: &indices,
                hud,
                editing: editing_index,
                editing_buffer,
                overlay_texts: overlay.texts,
                screen_texts: overlay.screen_texts,
                zplan: &zplan,
                edge_labels: &edge_labels,
            },
        ) {
            tracing::warn!(?err, "подготовка текста пропущена");
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        // Миникарта (T13-B): прямоугольник квада в физических px (None —
        // миникарта не задана или окно меньше 252×172 логических). Вычисляется
        // один раз — он же гейтит и загрузку uniform, и draw
        let minimap_quad = self
            .minimap
            .as_ref()
            .and_then(|_| quad_rect(self.size.width, self.size.height, self.scale_factor));
        // Uniform квада пишется до submit: write_buffer упорядочен раньше
        // команд кодировщика, создаваемого ниже
        if let Some(rect) = minimap_quad {
            self.minimap_pipeline.update_quad(
                &self.gpu.queue,
                rect,
                [self.size.width as f32, self.size.height as f32],
            );
        }
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("grid"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background_color()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if self.grid_visible {
                self.grid.draw(&mut pass);
            }
            // Сегменты z-порядка: карточки сегмента → тамбнейлы сегмента →
            // тексты сегмента. Следующий сегмент (перекрывающие карточки)
            // рисуется поверх текстов предыдущего — наложений нет.
            // Связи (T8) — под карточками всех сегментов, затем сегменты z-порядка
            if edges_end > 0 {
                self.cards.draw_range(&mut pass, 0..edges_end);
            }
            for (cards_range, thumbs_range, group) in &draw_ranges {
                self.cards.draw_range(&mut pass, cards_range.clone());
                self.thumbs.draw_range(&mut pass, thumbs_range.clone());
                if let Some(g) = group {
                    if let Err(err) = self.text.draw_group(&mut pass, *g) {
                        tracing::warn!(?err, "отрисовка текста пропущена");
                    }
                }
            }
            // Миникарта (T13-B): последний квад кадра — после карточек,
            // тамбнейлов и ВСЕХ текст-групп (HUD и оверлеи приложения —
            // финальная группа, уже нарисована выше). Правый нижний угол
            // против HUD слева сверху — пересечений по площади нет
            if let (Some(texture), Some(_)) = (self.minimap.as_ref(), minimap_quad) {
                self.minimap_pipeline.draw(&mut pass, texture);
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        frame.present();
        // Считаем связи, у которых хотя бы одна нода видна
        let visible_node_set: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let visible_edges = scene
            .canvas
            .edges
            .iter()
            .filter(|e| {
                visible_node_set.contains(
                    &scene
                        .canvas
                        .nodes
                        .iter()
                        .position(|n| n.id == e.from_node)
                        .unwrap_or(usize::MAX),
                ) || visible_node_set.contains(
                    &scene
                        .canvas
                        .nodes
                        .iter()
                        .position(|n| n.id == e.to_node)
                        .unwrap_or(usize::MAX),
                )
            })
            .count();
        Ok(FrameStats {
            total_nodes: scene.canvas.nodes.len(),
            visible_nodes: indices.len(),
            total_edges: scene.canvas.edges.len(),
            visible_edges,
            instances: instance_count,
            cpu_ms: cpu_start.elapsed().as_secs_f32() * 1000.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inst() -> CardInstance {
        CardInstance {
            pos: [100.0, 50.0],
            size: [200.0, 120.0],
            fill: [0.1, 0.1, 0.1, 1.0],
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        }
    }

    /// Screen→world конверсия оверлея: обратное преобразование камерой
    /// возвращает исходные логические px при любом зуме и позиции камеры.
    #[test]
    fn screen_overlay_round_trip() {
        let viewport = [1600.0, 900.0];
        for zoom in [0.05, 0.5, 1.0, 2.5, 4.0] {
            let mut camera = Camera::default();
            camera.set_zoom_at(zoom, [400.0, 300.0], viewport);
            camera.pan([33.0, -71.0]);
            let world = screen_instance_to_world(&camera, viewport, &inst());
            // Обратно на экран: позиция совпадает с исходной
            let screen = camera.world_to_screen(world.pos, viewport);
            assert!(
                (screen[0] - 100.0).abs() < 0.01,
                "zoom {zoom}: x={}",
                screen[0]
            );
            assert!(
                (screen[1] - 50.0).abs() < 0.01,
                "zoom {zoom}: y={}",
                screen[1]
            );
            // Размер на экране константен: world-размер * zoom = исходный
            assert!((world.size[0] * camera.zoom() - 200.0).abs() < 0.01);
            assert!((world.size[1] * camera.zoom() - 120.0).abs() < 0.01);
            // Радиус скругления тоже константен на экране
            assert!((world.params[0] * camera.zoom() - 8.0).abs() < 0.01);
        }
    }
}
