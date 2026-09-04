//! canvas-render — wgpu-рендер: камера, батчинг, текст, текстуры, LOD.
//! T1: GPU-контекст, оконный рендер с clear-проходом.
//! T2: камера (screen↔world), бесконечная сетка.

pub mod camera;
pub mod config;
pub mod gpu;
pub mod grid;
pub mod renderer;

pub use camera::Camera;
pub use renderer::Renderer;
