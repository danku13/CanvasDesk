//! Оконный рендер: surface, конфигурация, сетка поверх clear-прохода (T1, T2).

use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use canvas_core::{Canvas, SpatialIndex, Thumbnail};

use crate::camera::Camera;
use crate::cards::{build_instances, CardInstance, CardsPipeline, SELECTION_BORDER};
use crate::config::{
    background_color, choose_present_mode, choose_surface_format, surface_size_valid,
};
use crate::edit::EditingSession;
use crate::gpu::GpuContext;
use crate::grid::GridPipeline;
use crate::text::{body_area, OverlayText, TextSystem};
use crate::thumbs::{build_thumb_instances, ThumbsPipeline};

/// Заливка выделения текста в редакторе (T7) — акцент с прозрачностью.
const TEXT_SELECTION_FILL: [f32; 4] = [0.396, 0.612, 0.969, 0.35];
/// Фон-подсветка `==текст==` в заметках — приглушённый жёлтый с прозрачностью.
const HIGHLIGHT_FILL: [f32; 4] = [0.85, 0.75, 0.30, 0.30];

/// Оверлеи кадра от приложения (контекстное меню, T7): дополнительные
/// инстансы квадов (рисуются поверх карточек, под текстом) и их подписи.
pub struct FrameOverlay<'a> {
    pub instances: &'a [CardInstance],
    pub texts: &'a [OverlayText<'a>],
}

impl FrameOverlay<'_> {
    /// Пустой оверлей.
    pub const EMPTY: FrameOverlay<'static> = FrameOverlay {
        instances: &[],
        texts: &[],
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
        })
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

        self.grid.update_camera(
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
        );
        let instances = {
            let mut instances = build_instances(scene.canvas, &indices, scene.selected);
            // Фон-подсветка ==…== (форматирование): квады из кэша прошлого
            // шейпинга (при промахе появятся на следующий кадр), под текстом
            for &index in &indices {
                if let Some((entry_zoom, rects)) = self.text.highlight_rects(index) {
                    if let Some(node) = scene.canvas.nodes.get(index) {
                        let (origin, _, _) = body_area(node);
                        for rect in rects {
                            instances.push(Self::overlay_quad(
                                origin,
                                *rect,
                                entry_zoom,
                                HIGHLIGHT_FILL,
                            ));
                        }
                    }
                }
            }
            // Оверлеи редактирования (T7): выделение и каретка — квады поверх
            // карточки редактируемой ноды, под текстом (текст рисуется позже)
            if let Some(session) = editing.as_deref_mut() {
                if let Some(node) = scene.canvas.nodes.get(session.node()) {
                    let zoom_px = camera.zoom() * self.scale_factor;
                    let (origin, _, _) = body_area(node);
                    for rect in session.selection_rects(self.text.font_system_mut()) {
                        instances.push(Self::overlay_quad(
                            origin,
                            rect,
                            zoom_px,
                            TEXT_SELECTION_FILL,
                        ));
                    }
                    if let Some(rect) = session.caret_rect(self.text.font_system_mut()) {
                        instances.push(Self::overlay_quad(origin, rect, zoom_px, SELECTION_BORDER));
                    }
                }
            }
            // Оверлеи приложения (контекстное меню, T7)
            instances.extend_from_slice(overlay.instances);
            instances
        };
        let instance_count = self.cards.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &instances,
        );
        // Тамбнейлы (T6): тот же набор видимых нод, LOD по zoom внутри build
        let thumb_instances = build_thumb_instances(
            scene.canvas,
            &indices,
            self.thumbs.slots_mut(),
            camera.zoom(),
        );
        let thumb_count = self.thumbs.update(
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
            &crate::text::TitleFrame {
                camera,
                viewport_physical: [self.size.width, self.size.height],
                scale_factor: self.scale_factor,
                canvas: scene.canvas,
                indices: &indices,
                hud,
                editing: editing_index,
                editing_buffer,
                overlay_texts: overlay.texts,
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
            self.grid.draw(&mut pass);
            self.cards.draw(&mut pass, instance_count);
            self.thumbs.draw(&mut pass, thumb_count);
            if let Err(err) = self.text.draw(&mut pass) {
                tracing::warn!(?err, "отрисовка текста пропущена");
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
