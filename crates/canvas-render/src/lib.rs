//! canvas-render — wgpu-рендер: камера, батчинг, текст, текстуры, LOD.
//! T1: GPU-контекст, оконный рендер с clear-проходом.
//! T2: камера (screen↔world), бесконечная сетка.
//! T5: culling видимых нод по spatial index, замер кадра (HUD).

pub mod atlas;
pub mod camera;
pub mod cards;
pub mod config;
pub mod edit;
pub mod gpu;
pub mod grid;
pub mod markdown;
pub mod renderer;
pub mod stats;
pub mod text;
pub mod thumbs;

pub use camera::Camera;
pub use renderer::{FrameOverlay, FrameStats, Renderer, SceneView};
pub use stats::FrameMeter;
