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
        // FR-WEBGPU-DIAG: детальное логирование + retry. На некоторых конфигах
        // (Intel Arc + Chrome) `navigator.gpu.requestAdapter()` возвращает null
        // при первом вызове на старте страницы (GPU-процесс ещё инициализируется,
        // canvas только что вставлен в DOM), но работает при повторном. Браузер
        // пишет в console `No available adapters.` — это его нативное сообщение,
        // не наше. Retry с задержкой + явный power_preference решают timing-race.
        let backend_name = format!("{:?}", instance.enumerate_adapters(wgpu::Backends::all()));
        tracing::info!(
            backends = %backend_name,
            has_surface = compatible_surface.is_some(),
            "GpuContext::new: запрос адаптера"
        );

        let adapter = request_adapter_with_retry(&instance, compatible_surface).await?;

        let info = adapter.get_info();
        tracing::info!(
            backend = ?info.backend,
            name = %info.name,
            vendor = ?info.vendor,
            "GpuContext::new: адаптер получен"
        );

        let (device, queue) = {
            // FR-WASM-02 + §8 (canvas resolution): boost лимитов текстур для
            // GL-бэкенда до реального максимума адаптера — см. boost_gl_limits.
            let limits = if info.backend == wgpu::Backend::Gl {
                boost_gl_limits(wgpu::Limits::downlevel_webgl2_defaults(), &adapter.limits())
            } else {
                wgpu::Limits::default()
            };
            match adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        required_limits: limits,
                        ..Default::default()
                    },
                    None,
                )
                .await
            {
                Ok(dq) => dq,
                Err(err) => {
                    tracing::warn!(%err, backend = ?info.backend, "GpuContext::new: request_device неудачен");
                    return None;
                }
            }
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
}

/// FR-WEBGPU-DIAG: запрос адаптера с retry и fallback по power_preference.
///
/// На web (особенно Intel Arc + Chrome) `navigator.gpu.requestAdapter()`
/// может вернуть null при первом вызове на старте страницы — GPU-процесс
/// ещё инициализируется, canvas только что вставлен в DOM. Браузер пишет
/// в console `No available adapters.` (его нативное сообщение). Стратегия:
/// 1. default power_preference (None) — как раньше;
/// 2. если null — high-performance (на Windows powerPreference игнорируется,
///    но вызов повторяется с задержкой, давая GPU-процессу время);
/// 3. если null — low-power (последний шанс).
///
/// Между попытками — `wait` (через `wasm_bindgen_futures` на web, нативный
/// sleep на desktop) — даёт браузеру квант event loop для инициализации.
async fn request_adapter_with_retry(
    instance: &wgpu::Instance,
    compatible_surface: Option<&wgpu::Surface<'_>>,
) -> Option<wgpu::Adapter> {
    // Попытка 1: default (как раньше — backward compat).
    match instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface,
            force_fallback_adapter: false,
        })
        .await
    {
        Some(adapter) => {
            tracing::info!(attempt = 1, "request_adapter: успех (default)");
            return Some(adapter);
        }
        None => tracing::warn!(attempt = 1, "request_adapter: null (default)"),
    }

    // Попытка 2: high-performance. На Windows powerPreference игнорируется
    // (crbug.com/369219127), но повторный вызов + задержка дают GPU-процессу
    // время инициализироваться — это решает timing-race на старте.
    wait_briefly().await;
    match instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface,
            force_fallback_adapter: false,
        })
        .await
    {
        Some(adapter) => {
            tracing::info!(attempt = 2, "request_adapter: успех (high-performance)");
            return Some(adapter);
        }
        None => tracing::warn!(attempt = 2, "request_adapter: null (high-performance)"),
    }

    // Попытка 3: low-power — последний шанс (вдруг Chrome отдаст интегрированную).
    wait_briefly().await;
    match instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface,
            force_fallback_adapter: false,
        })
        .await
    {
        Some(adapter) => {
            tracing::info!(attempt = 3, "request_adapter: успех (low-power)");
            return Some(adapter);
        }
        None => tracing::warn!(attempt = 3, "request_adapter: null (low-power) — фолбэк"),
    }

    None
}

/// FR-WEBGPU-DIAG: короткая асинхронная задержка между retry-попытками.
/// На web — через `wasm_bindgen_futures::spawn_local` + `Promise::timeout`;
/// на desktop — `std::thread::sleep` в `pollster`-блокировке (нативный путь
/// `Renderer::new` блокирующий). 50 мс достаточно для инициализации
/// GPU-процесса Chrome (эмпирически).
async fn wait_briefly() {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen_futures::JsFuture;
        let promise = js_sys::Promise::new(|resolve, _| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
                .ok();
        });
        let _ = JsFuture::from(promise).await;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

impl GpuContext {
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
