//! `FlexLayoutEngine` (FR-068 W2, ADR-0015 §Решение п.3 «свой layout»):
//! собственный движок вёрстки БЕЗ внешних зависимостей — реализация
//! [`LayoutBackend`] + расширенной сцены ([`SceneNode`]) на чистой f32-
//! арифметике (zero-dep инвариант G7; §Контракт-2 FR-068).
//!
//! # W2-цель — побитовый паритет с taffy на совместимых политиках
//!
//! (`Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка — FR-068
//! §W2 «побитовая идентичность с taffy»): grow-распределение — CSS
//! flexbox §9.7 (resolve flexible lengths, включая rounding-on-freeze),
//! финальная приведение к px-сетке — алгоритм round-layout (позиции —
//! parent-relative round, размеры — через round краёв кумулятивных
//! координат; зеркало `taffy::compute::round_layout`). Паритет-тест —
//! `tests/flex_vs_taffy_parity.rs` (1000 деревьев, ≥ 80% побитово).
//!
//! # Документированные семантики
//!
//! - `SqueezeTail` — ДОСЛОВНО (хвост до 0 ширины, half-open hit;
//!   §Контракт-4 FR-068: «FlexLayoutEngine решает»), не flex_shrink
//!   (расхождение C3 с taffy — документировано, не fail в parity).
//! - `MainAlign::End` + переполнение — CSS unsafe alignment (дети
//!   выталкиваются за начальный край; расхождение с Native-клампом —
//!   документировано в доке `taffy_backend`).
//! - Grid-сцены — явные треки ([`SceneTrack`]); неравные треки со спанами
//!   — поддержка уровня сцены (parity-оракул — taffy).
//!
//! ⚠️ СТАБ W2 (заменяется агентом 2-a в этой же волне): V-5 методы
//! делегируют семантике `NativeBackend` 1:1 (байт-в-байт), сцена —
//! `todo!()`. Финальная реализация: собственный flexbox + round-layout
//! с побитовым паритетом к taffy.

use super::{
    Child, Column, LayoutBackend, LayoutFeatures, MeasuredItem, NativeBackend, Row, SceneNode,
    UiRect, UiVec2,
};
use crate::measure::TextMeasurer;

/// Собственный движок вёрстки (FR-068 W2). ZST без состояния
/// (immediate-mode, D2 ADR-0013) — `Send + Sync` бесплатно, статический
/// [`super::default_backend`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FlexLayoutEngine;

impl LayoutBackend for FlexLayoutEngine {
    fn features(&self) -> LayoutFeatures {
        // W2-стаб: маска Native (grow/basis/wrap/grid/measured).
        // Финальная реализация (агент 2-a): + OVERFLOW_CLIP/PERCENT/
        // ASPECT_RATIO/STICKY — сцена ([`FlexLayoutEngine::lay_out_scene`]);
        // FLEX_SHRINK не выставляется — SqueezeTail дословен (§Контракт-4).
        NATIVE_STUB.features()
    }

    fn lay_out_row(&self, row: Row, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        // Стаб: 1:1 семантика NativeBackend (заменяется собственным
        // flexbox с round-layout паритетом к taffy).
        NATIVE_STUB.lay_out_row(row, slot, items)
    }

    fn lay_out_column(&self, column: Column, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        NATIVE_STUB.lay_out_column(column, slot, items)
    }

    fn lay_out_measured(
        &self,
        row: Row,
        slot: UiRect,
        items: &[MeasuredItem],
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        family: &str,
        size: f32,
    ) -> Vec<UiRect> {
        NATIVE_STUB.lay_out_measured(row, slot, items, m, fs, family, size)
    }

    fn lay_out_grid(
        &self,
        slot: UiRect,
        cols: &[f32],
        rows: usize,
        row_h: f32,
        gap: UiVec2,
    ) -> Vec<UiRect> {
        NATIVE_STUB.lay_out_grid(slot, cols, rows, row_h, gap)
    }
}

impl FlexLayoutEngine {
    /// Раскладка расширенной сцены ([`SceneNode`], FR-068 W2): контракт —
    /// модульная докa `scene` (DFS pre-order, `[0]` — корень; percent/
    /// aspect/absolute/fixed/sticky/scroll/overflow — как у
    /// `TaffyBackend::lay_out_scene`, побитово на сценах demos).
    ///
    /// ⚠️ СТАБ: реализуется агентом 2-a в этой же волне.
    pub fn lay_out_scene(&self, slot: UiRect, scene: &SceneNode) -> Vec<UiRect> {
        let _ = (slot, scene);
        todo!("FR-068 W2 (агент 2-a): FlexLayoutEngine::lay_out_scene — собственная раскладка сцены, побитовый паритет с TaffyBackend")
    }
}

/// Стаб-делегат: единый статический NativeBackend для V-5 методов
/// (семантика байт-в-байт; в финальной реализации удаляется).
const NATIVE_STUB: NativeBackend = NativeBackend;
