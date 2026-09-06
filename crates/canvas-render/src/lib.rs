//! canvas-render — wgpu-рендер: камера, батчинг, текст, текстуры, LOD.
//! T1: GPU-контекст, оконный рендер с clear-проходом.
//! T2: камера (screen↔world), бесконечная сетка.
//! T5: culling видимых нод по spatial index, замер кадра (HUD).

pub mod camera;
pub mod cards;
pub mod config;
pub mod gpu;
pub mod grid;
pub mod renderer;
pub mod stats;
pub mod text;

pub use camera::Camera;
pub use renderer::{FrameStats, Renderer, SceneView};
pub use stats::FrameMeter;
