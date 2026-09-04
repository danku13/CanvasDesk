//! Оконный рендер: surface, конфигурация, сетка поверх clear-прохода (T1, T2).

use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use canvas_core::Canvas;

use crate::camera::Camera;
use crate::cards::{build_instances, CardsPipeline};
use crate::config::{
    background_color, choose_present_mode, choose_surface_format, surface_size_valid,
};
use crate::gpu::GpuContext;
use crate::grid::GridPipeline;
use crate::text::TextSystem;

/// Сцена кадра: модель канваса + состояние выделения (T4).
pub struct SceneView<'a> {
    pub canvas: &'a Canvas,
    pub selected: Option<usize>,
}

/// Рендерер окна: владеет surface и выполняет кадр по запросу (`request_redraw`).
pub struct Renderer {
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    grid: GridPipeline,
    cards: CardsPipeline,
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
        let text = TextSystem::new(&gpu.device, &gpu.queue, format);
        Ok(Self {
            gpu,
            surface,
            config,
            size,
            grid,
            cards,
            text,
            scale_factor: scale_factor as f32,
        })
    }

    /// Обновить scale factor окна (перенос между мониторами с разным DPI, SPEC §6.5).
    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
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

    /// Отрисовать кадр: фон, сетка, карточки нод, заголовки (T2/T4).
    pub fn render(&mut self, camera: &Camera, scene: &SceneView) -> anyhow::Result<()> {
        if !surface_size_valid(self.size.width, self.size.height) {
            return Ok(());
        }
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                // surface потерян/устарел (например, после смены DPI) — переконфигурация
                self.surface.configure(&self.gpu.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                anyhow::bail!("GPU: нехватка памяти под surface");
            }
            Err(err) => {
                tracing::warn!(?err, "кадр пропущен");
                return Ok(());
            }
        };

        self.grid.update_camera(
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
        );
        let instances = build_instances(scene.canvas, scene.selected);
        let instance_count = self.cards.update(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width as f32, self.size.height as f32],
            self.scale_factor,
            &instances,
        );
        if let Err(err) = self.text.prepare_titles(
            &self.gpu.device,
            &self.gpu.queue,
            camera,
            [self.size.width, self.size.height],
            self.scale_factor,
            scene.canvas,
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
            if let Err(err) = self.text.draw(&mut pass) {
                tracing::warn!(?err, "отрисовка текста пропущена");
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }
}
