//! Оконный рендер: surface, конфигурация, сетка поверх clear-прохода (T1, T2).

use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use canvas_core::{Canvas, SpatialIndex, Thumbnail};

use crate::camera::{Camera, Vec2};
use crate::cards::{card_instance, CardInstance, CardsPipeline, SELECTION_BORDER};
use crate::config::{
    background_color, choose_present_mode, choose_surface_format, surface_size_valid,
};
use crate::edit::EditingSession;
use crate::gpu::GpuContext;
use crate::grid::GridPipeline;
use crate::text::{body_area, titles_visible, OverlayText, ScreenText, TextSystem, TitleFrame};
use crate::thumbs::{thumb_instance, ThumbsPipeline, THUMB_MIN_ZOOM};
use crate::zorder;

/// Заливка выделения текста в редакторе (T7) — акцент с прозрачностью.
const TEXT_SELECTION_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.35];
/// Фон-подсветка `==текст==` в заметках — приглушённый жёлтый с прозрачностью.
const HIGHLIGHT_FILL: [f32; 4] = [0.85, 0.75, 0.30, 0.30];
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

/// Сцена кадра: модель канваса, spatial index (culling, T5) и выделение.
pub struct SceneView<'a> {
    pub canvas: &'a Canvas,
    pub spatial: &'a SpatialIndex,
    pub selected: Option<usize>,
}

/// Счётчики отрисованного кадра (T5) — для HUD и проверки culling.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Всего нод в сцене.
    pub total_nodes: usize,
    /// Нод попало в viewport (прошли culling).
    pub visible_nodes: usize,
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

    /// Отрисовать кадр: фон, сетка, карточки видимых нод, заголовки, HUD (T2/T4/T5).
    /// `hud` — строка оверлея (F3), None — без оверлея. Возвращает счётчики кадра.
    /// `editing` — активная сессия редактирования (T7): её буфер рисуется вместо
    /// кэшированного тела ноды, поверх карточки — каретка и выделение.
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

        // Актуальные метрики буфера редактирования под текущий зум (T7) —
        // до вычисления каретки/выделения ниже
        if let Some(session) = editing.as_deref_mut() {
            if let Some(node) = scene.canvas.nodes.get(session.node()) {
                let zoom_px = camera.zoom() * self.scale_factor;
                let (_, width, height) = body_area(node);
                session.set_layout(
                    self.text.font_system_mut(),
                    width * zoom_px,
                    height * zoom_px,
                    zoom_px,
                );
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
        // Редакторские оверлеи (T7): выделение и каретка — квады на z-позиции
        // редактируемой ноды (над её карточкой, под её текстом и под
        // перекрывающими карточками). Метрики считаются до z-прохода:
        // session-операции требуют FontSystem.
        let editing_node = editing.as_deref().map(EditingSession::node);
        let mut editing_quads: Vec<CardInstance> = Vec::new();
        if let Some(session) = editing.as_deref_mut() {
            if let Some(node) = scene.canvas.nodes.get(session.node()) {
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
                    editing_quads.push(Self::overlay_quad(origin, rect, zoom_px, SELECTION_BORDER));
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
                instances.push(card_instance(node, scene.selected == Some(index)));
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
        let editing_index = editing_ref.map(EditingSession::node);
        let editing_buffer = editing_ref.and_then(|session| {
            scene
                .canvas
                .nodes
                .get(session.node())
                .map(|node| (session.buffer(), body_area(node).0))
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
            },
        ) {
            tracing::warn!(?err, "подготовка текста пропущена");
        }

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
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
            for (cards_range, thumbs_range, group) in &draw_ranges {
                self.cards.draw_range(&mut pass, cards_range.clone());
                self.thumbs.draw_range(&mut pass, thumbs_range.clone());
                if let Some(g) = group {
                    if let Err(err) = self.text.draw_group(&mut pass, *g) {
                        tracing::warn!(?err, "отрисовка текста пропущена");
                    }
                }
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        frame.present();
        Ok(FrameStats {
            total_nodes: scene.canvas.nodes.len(),
            visible_nodes: indices.len(),
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
