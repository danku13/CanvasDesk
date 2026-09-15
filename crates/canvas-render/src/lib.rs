//! canvas-render — wgpu-рендер: камера, батчинг, текст, текстуры, LOD.
//! T1: GPU-контекст, оконный рендер с clear-проходом.
//! T2: камера (screen↔world), бесконечная сетка.
//! T5: culling видимых нод по spatial index, замер кадра (HUD).
//! T13: миникарта (CPU-снимок + GPU-квад). T14: поиск (UI-модель, анимации).

pub mod animate;
pub mod atlas;
pub mod camera;
pub mod cards;
pub mod config;
pub mod contrast;
pub mod edit;
pub mod gfm;
pub mod gpu;
pub mod grid;
pub mod markdown;
pub mod minimap;
pub mod minimap_pass;
pub mod renderer;
pub mod search_ui;
pub mod stats;
pub mod text;
pub mod theme;
pub mod thumbs;
pub mod widget_pass;
pub mod zorder;

pub use camera::Camera;
pub use glyphon::Color;
pub use renderer::{FrameOverlay, FrameStats, Renderer, SceneView, Selection};
pub use stats::FrameMeter;
pub use theme::ThemeColors;
pub use widget_pass::{WidgetPass, WidgetQuad, WIDGET_SNAPSHOT_MAX};
