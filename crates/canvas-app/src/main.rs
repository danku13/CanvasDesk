//! canvas-app — приложение: event loop, команды, UI-состояние, main().

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Состояние приложения: окно и рендерер создаются в `resumed`
/// (идиома winit 0.30 — окно создаётся только на активном event loop).
#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<canvas_render::Renderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("CanvasDesk");
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
            Ok(renderer) => {
                tracing::info!(
                    width = window.inner_size().width,
                    height = window.inner_size().height,
                    scale_factor = window.scale_factor(),
                    "окно создано"
                );
                self.renderer = Some(renderer);
                self.window = Some(window);
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Err(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = self.renderer.as_mut() {
                    if let Err(err) = renderer.render() {
                        tracing::error!(%err, "ошибка рендера, завершение");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> anyhow::Result<()> {
    // По умолчанию info, но без спама внутренних крейтов wgpu; переопределяется через RUST_LOG
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu_hal=warn,wgpu_core=warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut App::default())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Версия пакета — валидный semver вида x.y.z.
    #[test]
    fn version_is_semver() {
        let version = env!("CARGO_PKG_VERSION");
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3, "версия должна быть вида x.y.z: {version}");
        for part in parts {
            assert!(
                part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty(),
                "компонент версии не числовой: {part}"
            );
        }
    }
}
