//! Оконный рендер: surface, конфигурация, clear-проход (T1).

use std::sync::Arc;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use crate::config::{
    background_color, choose_present_mode, choose_surface_format, surface_size_valid,
};
use crate::gpu::GpuContext;

/// Рендерер окна: владеет surface и выполняет кадр по запросу (`request_redraw`).
pub struct Renderer {
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
}

impl Renderer {
    /// Создать рендерер для окна. Вызывается один раз при старте
    /// (блокирующе, через `pollster` в canvas-app).
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
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
        Ok(Self {
            gpu,
            surface,
            config,
            size,
        })
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

    /// Отрисовать кадр (пока — очистка фоном #1e1e22, T1).
    pub fn render(&mut self) -> anyhow::Result<()> {
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
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
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
        }
        self.gpu.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }
}
