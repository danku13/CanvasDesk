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
pub mod edit;
pub mod gpu;
pub mod grid;
pub mod markdown;
pub mod minimap;
pub mod minimap_pass;
pub mod renderer;
pub mod search_ui;
pub mod stats;
pub mod text;
pub mod thumbs;
pub mod zorder;

pub use camera::Camera;
pub use glyphon::Color;
pub use renderer::{FrameOverlay, FrameStats, Renderer, SceneView, Selection};
pub use stats::FrameMeter;
