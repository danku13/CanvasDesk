//! canvas-app — приложение: event loop, команды, UI-состояние, main().

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use canvas_core::{Canvas, Node};
use canvas_render::camera::Vec2;
use canvas_render::{Camera, SceneView};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

/// Множитель зума на одну строку колеса мыши (Ctrl+колесо, SPEC §8).
const ZOOM_STEP_PER_LINE: f32 = 1.1;
/// Пикселей панорамирования на строку колеса без Ctrl (скролл тачпада).
const PAN_PX_PER_LINE: f32 = 40.0;
/// Debounce автосейва (SPEC §9).
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_secs(2);

/// Стартовый канвас при отсутствии файла: заметка + файловые ноды (T4).
fn seed_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    let mut note = Node::text("note-1", "Добро пожаловать в CanvasDesk", 80.0, 60.0);
    note.width = 280.0;
    note.color = Some("4".into());
    canvas.nodes.push(note);
    canvas.nodes.push(Node::file(
        "file-1",
        "docs/SPEC.md",
        440.0,
        60.0,
        320.0,
        220.0,
    ));
    canvas.nodes.push(Node::file(
        "file-2",
        "docs/TASKS.md",
        440.0,
        340.0,
        320.0,
        220.0,
    ));
    canvas
}

/// Состояние сцены: модель, файл, выделение и перетаскивание.
struct SceneState {
    canvas: Canvas,
    path: PathBuf,
    selected: Option<usize>,
    /// (индекс ноды, смещение от курсора до левого верхнего угла ноды в world).
    dragging: Option<(usize, Vec2)>,
    dirty_since: Option<Instant>,
}

impl SceneState {
    fn load_or_seed(path: PathBuf) -> Self {
        let canvas = match Canvas::load(&path) {
            Ok(canvas) => {
                tracing::info!(path = %path.display(), nodes = canvas.nodes.len(), "канвас загружен");
                canvas
            }
            Err(err) => {
                tracing::info!(path = %path.display(), %err, "создаю стартовый канвас");
                let canvas = seed_canvas();
                if let Err(err) = canvas.save(&path) {
                    tracing::warn!(%err, "не удалось сохранить стартовый канвас");
                }
                canvas
            }
        };
        Self {
            canvas,
            path,
            selected: None,
            dragging: None,
            dirty_since: None,
        }
    }

    fn mark_dirty(&mut self) {
        self.dirty_since = Some(Instant::now());
    }

    /// Сохранить, если правки висят дольше debounce (SPEC §9). Возвращает true при записи.
    fn autosave_if_due(&mut self) -> bool {
        let due = self
            .dirty_since
            .is_some_and(|since| since.elapsed() >= AUTOSAVE_DEBOUNCE);
        if !due {
            return false;
        }
        self.save_now()
    }

    fn save_now(&mut self) -> bool {
        self.dirty_since = None;
        match self.canvas.save_with_backup(&self.path) {
            Ok(()) => {
                tracing::info!(path = %self.path.display(), "канвас сохранён");
                true
            }
            Err(err) => {
                tracing::error!(%err, "ошибка сохранения канваса");
                false
            }
        }
    }
}

/// Состояние приложения: окно и рендерер создаются в `resumed`
/// (идиома winit 0.30 — окно создаётся только на активном event loop).
struct App {
    window: Option<Arc<Window>>,
    renderer: Option<canvas_render::Renderer>,
    camera: Camera,
    scene: SceneState,
    modifiers: ModifiersState,
    /// Позиция курсора в логических пикселях.
    cursor: Vec2,
    middle_pressed: bool,
    space_pressed: bool,
    left_pressed: bool,
}

impl App {
    fn new(canvas_path: PathBuf) -> Self {
        Self {
            window: None,
            renderer: None,
            camera: Camera::default(),
            scene: SceneState::load_or_seed(canvas_path),
            modifiers: ModifiersState::empty(),
            cursor: [0.0, 0.0],
            middle_pressed: false,
            space_pressed: false,
            left_pressed: false,
        }
    }

    /// Активно ли панорамирование (средняя кнопка или Space+drag, SPEC §8).
    fn panning(&self) -> bool {
        self.middle_pressed || (self.space_pressed && self.left_pressed)
    }

    /// Размер viewport в логических пикселях.
    fn viewport_logical(&self) -> Vec2 {
        match &self.window {
            Some(window) => {
                let size = window.inner_size();
                let scale = window.scale_factor() as f32;
                [size.width as f32 / scale, size.height as f32 / scale]
            }
            None => [0.0, 0.0],
        }
    }

    /// Позиция курсора в world-координатах.
    fn cursor_world(&self) -> Vec2 {
        self.camera
            .screen_to_world(self.cursor, self.viewport_logical())
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
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
                self.request_redraw();
            }
            Err(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Форс-сейв перед выходом — не ждать debounce (SPEC §9)
                if self.scene.dirty_since.is_some() {
                    self.scene.save_now();
                }
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_scale_factor(scale_factor);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(&event),
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Middle => self.middle_pressed = state == ElementState::Pressed,
                MouseButton::Left => self.on_left_button(state),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),
            WindowEvent::PinchGesture { delta, .. } => self.on_pinch(delta),
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = self.renderer.as_mut() {
                    let scene = SceneView {
                        canvas: &self.scene.canvas,
                        selected: self.scene.selected,
                    };
                    if let Err(err) = renderer.render(&self.camera, &scene) {
                        tracing::error!(%err, "ошибка рендера, завершение");
                        event_loop.exit();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.scene.autosave_if_due();
    }
}

impl App {
    fn on_key(&mut self, event: &KeyEvent) {
        if event.logical_key == Key::Named(NamedKey::Space) && !event.repeat {
            self.space_pressed = event.state == ElementState::Pressed;
            if !self.space_pressed {
                // Отпускание Space во время drag не должно оставлять ноду "прилипшей"
                self.scene.dragging = None;
            }
        }
    }

    fn on_left_button(&mut self, state: ElementState) {
        self.left_pressed = state == ElementState::Pressed;
        if self.space_pressed {
            return; // Space+drag — панорамирование (SPEC §8)
        }
        match state {
            ElementState::Pressed => {
                let world = self.cursor_world();
                match self.scene.canvas.hit_test(world) {
                    Some(index) => {
                        self.scene.selected = Some(index);
                        let node = &self.scene.canvas.nodes[index];
                        self.scene.dragging = Some((index, [node.x - world[0], node.y - world[1]]));
                    }
                    None => self.scene.selected = None,
                }
                self.request_redraw();
            }
            ElementState::Released => {
                self.scene.dragging = None;
            }
        }
    }

    fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        let logical = [position.x as f32 / scale, position.y as f32 / scale];
        if self.panning() {
            let delta = [logical[0] - self.cursor[0], logical[1] - self.cursor[1]];
            self.camera.pan(delta);
            self.request_redraw();
        }
        self.cursor = logical;
        if !self.space_pressed {
            if let Some((index, offset)) = self.scene.dragging {
                let world = self.cursor_world();
                let node = &mut self.scene.canvas.nodes[index];
                node.x = world[0] + offset[0];
                node.y = world[1] + offset[1];
                self.scene.mark_dirty();
                self.request_redraw();
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // Тачпады шлют PixelDelta (физические px), колёсики мышей — LineDelta
        let scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * PAN_PX_PER_LINE, y * PAN_PX_PER_LINE),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / scale, pos.y as f32 / scale),
        };
        let viewport = self.viewport_logical();
        if self.modifiers.control_key() {
            // Ctrl+колесо — зум к позиции курсора (SPEC §8)
            let factor = match delta {
                MouseScrollDelta::LineDelta(_, y) => ZOOM_STEP_PER_LINE.powf(y),
                MouseScrollDelta::PixelDelta(pos) => (pos.y as f32 * 0.005).exp(),
            };
            self.camera.zoom_at(factor, self.cursor, viewport);
        } else {
            // Двухпальцевый скролл тачпада — панорамирование (SPEC §8)
            self.camera.pan([dx, dy]);
        }
        self.request_redraw();
    }

    fn on_pinch(&mut self, delta: f64) {
        let viewport = self.viewport_logical();
        let factor = (delta as f32).exp();
        self.camera.zoom_at(factor, self.cursor, viewport);
        self.request_redraw();
    }
}

fn main() -> anyhow::Result<()> {
    // По умолчанию info, но без спама внутренних крейтов wgpu; переопределяется через RUST_LOG
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,wgpu_hal=warn,wgpu_core=warn")
    });
    tracing_subscriber::fmt().with_env_filter(filter).init();
    let canvas_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("default.canvas"));
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut App::new(canvas_path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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

    /// Стартовый канвас непустой и переживает round-trip.
    #[test]
    fn seed_canvas_is_valid() {
        let canvas = seed_canvas();
        assert!(!canvas.nodes.is_empty());
        let json = canvas.to_json().expect("сериализация seed");
        let restored = Canvas::from_str(&json).expect("seed парсится обратно");
        assert_eq!(canvas, restored);
    }
}
