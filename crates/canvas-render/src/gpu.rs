//! GPU-контекст wgpu: instance / adapter / device / queue.
//! Общий для оконного рендера (`Renderer`) и headless-режима (тесты, превью).

use anyhow::Context;

use crate::config::background_color;

/// Инициализированный GPU-контекст.
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Создать контекст; `compatible_surface` — для оконного режима, чтобы
    /// адаптер гарантированно умел рисовать в surface.
    pub async fn new(
        instance: wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
    ) -> Option<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface,
                force_fallback_adapter: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        Some(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }

    /// Headless-контекст без surface (тесты, будущие offscreen-превью).
    /// `None` — GPU-адаптер недоступен (например, CI без WARP).
    pub async fn headless() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        Self::new(instance, None).await
    }

    /// Clear-проход цветом фона в offscreen-текстуру с readback пикселей.
    /// Используется интеграционным тестом `headless_render_clear`.
    pub fn render_clear_to_texture(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> anyhow::Result<Vec<u8>> {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless-clear"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless-clear"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless-clear"),
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

        // Readback: строки в буфере выравниваются по 256 байт
        let unpadded_row = width * 4;
        let padded_row = unpadded_row.div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless-clear-readback"),
            size: u64::from(padded_row * height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row),
                    rows_per_image: None,
                },
            },
            size,
        );
        self.queue.submit([encoder.finish()]);

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .context("map_async: канал закрыт")?
            .context("map_async: ошибка маппинга")?;

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height as usize {
            let start = row * padded_row as usize;
            pixels.extend_from_slice(&data[start..start + unpadded_row as usize]);
        }
        drop(data);
        buffer.unmap();
        Ok(pixels)
    }
}
