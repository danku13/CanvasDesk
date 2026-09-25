//! canvas-render — wgpu-рендер: камера, батчинг, текст, текстуры, LOD.
//! T1: GPU-контекст, оконный рендер с clear-проходом.
//! T2: камера (screen↔world), бесконечная сетка.
//! T5: culling видимых нод по spatial index, замер кадра (HUD).
//! T13: миникарта (CPU-снимок + GPU-квад). T14: поиск (UI-модель, анимации).

/// PRD-0004 N1: анатомия ноды — контракт зон A–E и LOD-уровней.
pub mod anatomy;
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
pub mod guides;
/// FR-ICONS: растеризованные байты SVG-иконок (build-time сгенерированный файл).
pub mod icon_data;
/// FR-ICONS: screen-space wgpu-пайплайн SVG-иконок (атлас + tint + instanced).
pub mod icon_pipeline;
pub mod markdown;
pub mod minimap;
pub mod minimap_pass;
pub mod renderer;
pub mod renderer_init;
/// FR-061 этап B (Н-3 пре-PRD PRD-0004): табличная модель тела ноды —
/// декларативные строки данных (D-2) + проход A направляющих (D-6/D-11).
pub mod row_grid;
pub mod search_ui;
pub mod sectors;
pub mod stage;
pub mod stats;
pub mod text;
pub mod theme;
/// FR-047 (PRD-0006 D4/F-8): темы-пресеты — отображение разобранных
/// JSON-данных canvas-core в `ThemeColors` + контраст-тесты G3.
pub mod theme_presets;
pub mod thumbs;
pub mod widget_pass;
pub mod zorder;

pub use camera::Camera;
pub use glyphon::Color;
pub use guides::{GuideLine, GuideSource, GuidesFrame};
pub use icon_pipeline::{icon_uv, IconInstance, IconPipeline, ATLAS_H, ATLAS_W, ICON_CELL_PX};
pub use renderer::{
    FrameOverlay, FrameStats, ParamDropView, Renderer, SceneView, ScreenBand, Selection, SpillView,
    WhatIfNode,
};
pub use stage::{stage_rect_screen, StageTransform};
pub use stats::FrameMeter;
pub use theme::ThemeColors;
pub use widget_pass::{WidgetPass, WidgetQuad, WIDGET_SNAPSHOT_MAX};
