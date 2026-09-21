//! FR-042 (E3): main stage — трансформация «stage-локальные px → экран →
//! мир реальной камеры» и сборка каркаса stage.
//!
//! Раскладка stage ([`canvas_core::stage_layout`]) вычисляется в
//! rect-относительных экранных px с единым масштабом сжатия (≤ 1) —
//! виртуальная камера реализована этой композицией: инстансы stage
//! переводятся в мир текущей камеры чистой функцией
//! [`StageTransform::instance_to_world`], поэтому второй GPU-бинд камеры
//! не требуется — визуальный результат и инварианты FR-042 (контент
//! умещается в rect, модальность, затемнение) те же, пайплайн не меняется.
//!
//! Всё — чистые функции/структуры без состояния; юнит-тесты внизу.

use crate::camera::{Camera, Vec2};
use crate::cards::CardInstance;

/// Трансформация stage-координат (rect-относительные px при масштабе
/// раскладки) в логические экранные px: `screen = stage·scale + origin`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageTransform {
    /// Масштаб раскладки stage (из [`canvas_core::stage_layout`], ≤ 1).
    pub scale: f32,
    /// Левый верхний угол main stage rect в логических px окна.
    pub origin: Vec2,
}

impl StageTransform {
    /// Трансформация из rect main stage и масштаба раскладки.
    pub fn new(rect_origin: Vec2, scale: f32) -> Self {
        Self {
            scale: scale.max(f32::EPSILON),
            origin: rect_origin,
        }
    }

    /// Точка stage → логические экранные px.
    pub fn map_point(&self, point: Vec2) -> Vec2 {
        [
            point[0] * self.scale + self.origin[0],
            point[1] * self.scale + self.origin[1],
        ]
    }

    /// Размер stage-единиц → логические экранные px.
    pub fn map_size(&self, size: f32) -> f32 {
        size * self.scale
    }

    /// Логические экранные px → точка stage (обратно: ввод внутри stage).
    pub fn unmap_point(&self, screen: Vec2) -> Vec2 {
        [
            (screen[0] - self.origin[0]) / self.scale,
            (screen[1] - self.origin[1]) / self.scale,
        ]
    }

    /// Stage-инстанс (позиция/размер в stage-локальных px) → world-инстанс
    /// реальной камеры: позиция — через map_point + screen_to_world, размер
    /// и радиус скругления — делятся на зум камеры (мир-единицы), заливка/
    /// рамка переносятся как есть.
    pub fn instance_to_world(
        &self,
        inst: &CardInstance,
        camera: &Camera,
        viewport: Vec2,
    ) -> CardInstance {
        let zoom = camera.zoom();
        CardInstance {
            pos: camera.screen_to_world(self.map_point(inst.pos), viewport),
            size: [
                self.map_size(inst.size[0]) / zoom,
                self.map_size(inst.size[1]) / zoom,
            ],
            fill: inst.fill,
            border: inst.border,
            params: [inst.params[0] * self.scale / zoom, 0.0, 0.0, 1.0],
        }
    }
}

/// Прямоугольник main stage в логических px окна: `[x, y, w, h]` —
/// результат [`canvas_core::main_stage_rect`], развёрнутый в массив
/// (экранная геометрия кадра).
pub fn stage_rect_screen(viewport: Vec2) -> [f32; 4] {
    let rect = canvas_core::main_stage_rect(viewport);
    [rect.x, rect.y, rect.w, rect.h]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: unmap(map(point)) == point при любом масштабе.
    #[test]
    fn map_unmap_round_trip() {
        let t = StageTransform::new([100.0, 50.0], 0.8);
        for point in [[0.0, 0.0], [120.0, 75.5], [300.0, 200.0]] {
            let screen = t.map_point(point);
            let back = t.unmap_point(screen);
            assert!((back[0] - point[0]).abs() < 1e-3);
            assert!((back[1] - point[1]).abs() < 1e-3);
        }
    }

    /// map_point: масштаб и сдвиг origin применяются линейно.
    #[test]
    fn map_point_linear() {
        let t = StageTransform::new([10.0, 20.0], 0.5);
        assert_eq!(t.map_point([100.0, 60.0]), [60.0, 50.0]);
        assert!((t.map_size(40.0) - 20.0).abs() < 1e-5);
    }

    /// stage_rect_screen согласован с canvas_core::main_stage_rect:
    /// центрирован и ≤ 70% сторон.
    #[test]
    fn stage_rect_matches_core() {
        let viewport = [1280.0, 720.0];
        let [x, y, w, h] = stage_rect_screen(viewport);
        assert!((x + w / 2.0 - 640.0).abs() < 1e-3);
        assert!((y + h / 2.0 - 360.0).abs() < 1e-3);
        assert!(w <= viewport[0] * 0.7 + 1e-3);
        assert!(h <= viewport[1] * 0.7 + 1e-3);
    }
}
