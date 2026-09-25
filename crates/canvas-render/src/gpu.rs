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
        let (device, queue) = {
            // FR-WASM-02 + §8 (canvas resolution): boost лимитов текстур для
            // GL-бэкенда до реального максимума адаптера — см. boost_gl_limits.
            let limits = if adapter.get_info().backend == wgpu::Backend::Gl {
                boost_gl_limits(wgpu::Limits::downlevel_webgl2_defaults(), &adapter.limits())
            } else {
                wgpu::Limits::default()
            };
            adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        required_limits: limits,
                        ..Default::default()
                    },
                    None,
                )
                .await
                .ok()?
        };
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

/// FR-WASM-02 §8 (canvas resolution): boost лимитов текстур для GL-бэкенда
/// до реального максимума адаптера. `downlevel_webgl2_defaults()` режет
/// `max_texture_dimension_2d` до 2048 — это МИНИМУМ спецификации WebGL2, но
/// реальные GPU через ANGLE/D3D11 (Intel Arc, NVIDIA, AMD) поддерживают
/// 8192 или 16384. На HiDPI мониторах (DPR=2) canvas хочет 3044×2610 →
/// `clamp_surface_extent` резал до 2048×2048, браузер растягивал —
/// размытие/искажение пропорций.
///
/// Boost берёт `max(downlevel_default, adapter_limit)` по каждому измерению
/// — это безопасно: `request_device` с `required_limits = adapter.limits()`
/// всегда succeeds, wgpu не запрашивает больше, чем поддерживает адаптер.
/// Если адаптер даёт только 2048 (минимум WebGL2), остаётся 2048 —
/// поведение идентично предыдущей версии (до §8).
///
/// Чистая функция (без GPU) — тестируется без устройства; вызывается из
/// `GpuContext::new` только для `Backend::Gl` (WebGL2 path).
pub fn boost_gl_limits(mut downlevel: wgpu::Limits, adapter: &wgpu::Limits) -> wgpu::Limits {
    downlevel.max_texture_dimension_2d = downlevel
        .max_texture_dimension_2d
        .max(adapter.max_texture_dimension_2d);
    downlevel.max_texture_dimension_3d = downlevel
        .max_texture_dimension_3d
        .max(adapter.max_texture_dimension_3d);
    downlevel.max_texture_array_layers = downlevel
        .max_texture_array_layers
        .max(adapter.max_texture_array_layers);
    downlevel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter_limits(max_2d: u32) -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_2d: max_2d,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    /// Адаптер поддерживает 8192 (типичный ANGLE/D3D11) → boost поднимает
    /// лимит с downlevel_default 2048 до 8192. Это ключевой кейс §8: canvas
    /// на HiDPI мониторе (DPR=2, 3044×2610) больше НЕ режется clamp'ом.
    #[test]
    fn boost_gl_limits_raises_to_adapter_max_when_higher() {
        let downlevel = wgpu::Limits::downlevel_webgl2_defaults();
        let adapter = adapter_limits(8192);
        let boosted = boost_gl_limits(downlevel.clone(), &adapter);
        assert_eq!(boosted.max_texture_dimension_2d, 8192);
        // 3d и array_layers — boost тоже сработал бы, но adapter_limits()
        // в тесте возвращает downlevel-defaults для них → остаётся как было.
    }

    /// Адаптер поддерживает только 2048 (минимум WebGL2) → boost НЕ уменьшает
    /// downlevel_default. Идёмпотентность: max(2048, 2048) = 2048.
    #[test]
    fn boost_gl_limits_noop_when_adapter_at_minimum() {
        let downlevel = wgpu::Limits::downlevel_webgl2_defaults();
        let adapter = adapter_limits(2048);
        let boosted = boost_gl_limits(downlevel.clone(), &adapter);
        assert_eq!(
            boosted.max_texture_dimension_2d,
            downlevel.max_texture_dimension_2d
        );
    }

    /// Адаптер поддерживает 16384 (топовые GPU) → boost поднимает до 16384.
    /// Гарантирует, что CanvasDesk использует весь потенциал GPU, а не
    /// искусственный 2048-кап.
    #[test]
    fn boost_gl_limits_uses_full_adapter_capacity_on_high_end_gpus() {
        let downlevel = wgpu::Limits::downlevel_webgl2_defaults();
        let adapter = adapter_limits(16384);
        let boosted = boost_gl_limits(downlevel, &adapter);
        assert_eq!(boosted.max_texture_dimension_2d, 16384);
    }

    /// Адаптер поддерживает 4096 (mid-range) → boost поднимает до 4096.
    /// Промежуточный кейс: 2048 < 4096 < 8192.
    #[test]
    fn boost_gl_limits_handles_intermediate_values() {
        let downlevel = wgpu::Limits::downlevel_webgl2_defaults();
        let adapter = adapter_limits(4096);
        let boosted = boost_gl_limits(downlevel, &adapter);
        assert_eq!(boosted.max_texture_dimension_2d, 4096);
    }

    /// Boost НЕ уменьшает другие лимиты (storage buffers, bind groups и пр.) —
    /// downlevel_webgl2_defaults остаётся как есть, только текстурные boosted.
    #[test]
    fn boost_gl_limits_preserves_non_texture_limits() {
        let downlevel = wgpu::Limits::downlevel_webgl2_defaults();
        let original_max_bind_groups = downlevel.max_bind_groups;
        let original_max_buffer_size = downlevel.max_buffer_size;
        let adapter = adapter_limits(8192);
        let boosted = boost_gl_limits(downlevel, &adapter);
        assert_eq!(boosted.max_bind_groups, original_max_bind_groups);
        assert_eq!(boosted.max_buffer_size, original_max_buffer_size);
    }
}
