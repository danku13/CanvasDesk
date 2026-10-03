//! FR-097 (мобильный web): тач-цели canvas-UI ≥ 44 лог. px.
//!
//! GPU-панели (палитра, флайаут, онбординг) свёрстаны под мышь: hit-зона
//! строки/кнопки = нарисованной. Палец (Apple HIG / Material — минимум
//! 44 pt) промахивается по строкам-карточкам высотой ~28–32 лог. px.
//!
//! Решение — hit-only расширение (hit ≥ draw, визуалы не меняются):
//! на coarse-указателе ([`canvas_core::web_bridge::POINTER_COARSE`],
//! источник — `matchMedia("(pointer: coarse)")` в canvas-web) hit-прямо-
//! угольник дотягивается до [`MIN_TOUCH_TARGET`] по обеим осям
//! центрированно, с клампом в контейнер (панель/строку), чтобы
//! расширенная зона не вылезала за панель и не перекрывала соседние
//! зоны канваса. На точном указателе (мышь/тачпад) — расширения нет:
//! поведение десктопа бит-в-бит прежнее.
//!
//! Конвенции прямоугольников: canvas-app использует обе — `[x, y, w, h]`
//! (`crate::point_in_rect`, онбординг) и `[x0, y0, x1, y1]` на входе
//! `app::support::rect_xywh`. Хелперы — для обеих (`*_xywh` / `*_xyxy`).

use canvas_core::web_bridge;

/// Минимальная сторона тач-цели, лог. px (Apple HIG / Material — 44).
pub const MIN_TOUCH_TARGET: f32 = 44.0;

/// Coarse-указатель активен? (тач — основной ввод).
pub fn pointer_coarse() -> bool {
    web_bridge::pointer_coarse()
}

/// Расширить rect `[x, y, w, h]` до `min` по обеим осям центрированно
/// (уже большая сторона не сжимается).
pub fn expand_xywh(rect: [f32; 4], min: f32) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let dw = (min - w).max(0.0);
    let dh = (min - h).max(0.0);
    [x - dw / 2.0, y - dh / 2.0, w + dw, h + dh]
}

/// Расширить rect `[x0, y0, x1, y1]` до `min` по обеим осям (центрированно).
pub fn expand_xyxy(rect: [f32; 4], min: f32) -> [f32; 4] {
    let [x0, y0, x1, y1] = rect;
    let dw = (min - (x1 - x0)).max(0.0);
    let dh = (min - (y1 - y0)).max(0.0);
    [x0 - dw / 2.0, y0 - dh / 2.0, x1 + dw / 2.0, y1 + dh / 2.0]
}

/// Пересечение `[x, y, w, h]`-ректов (пустое — нулевая ширина/высота).
pub fn intersect_xywh(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

/// Хит-зона `[x, y, w, h]` для coarse-указателя: расширение до
/// [`MIN_TOUCH_TARGET`] с клампом в `container` (те же `[x, y, w, h]`).
/// На точном указателе — rect без изменений.
pub fn touch_hit_xywh(rect: [f32; 4], container: [f32; 4]) -> [f32; 4] {
    if !pointer_coarse() {
        return rect;
    }
    intersect_xywh(expand_xywh(rect, MIN_TOUCH_TARGET), container)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    /// Расширение xywh: короткая сторона дотягивается до минимума
    /// центрированно, длинная не меняется.
    #[test]
    fn expand_xywh_centers_short_side() {
        // Строка 32 px высотой (палитра) → 44: +6 сверху/снизу
        let expanded = expand_xywh([100.0, 50.0, 200.0, 32.0], 44.0);
        assert!((expanded[0] - 100.0).abs() < EPS);
        assert!((expanded[1] - 44.0).abs() < EPS, "{expanded:?}");
        assert!((expanded[2] - 200.0).abs() < EPS);
        assert!((expanded[3] - 44.0).abs() < EPS);
        // Кнопка 20×20 → 44×44
        let button = expand_xywh([10.0, 10.0, 20.0, 20.0], 44.0);
        assert!((button[0] - (-2.0)).abs() < EPS);
        assert!((button[2] - 44.0).abs() < EPS);
        // Больше минимума — без изменений
        assert_eq!(
            expand_xywh([0.0, 0.0, 100.0, 60.0], 44.0),
            [0.0, 0.0, 100.0, 60.0]
        );
    }

    /// Расширение xyxy согласовано с xywh (та же геометрия, другой формат).
    #[test]
    fn expand_xyxy_matches_xywh() {
        let xyxy = expand_xyxy([100.0, 50.0, 300.0, 82.0], 44.0);
        let as_xywh = expand_xywh([100.0, 50.0, 200.0, 32.0], 44.0);
        assert!((xyxy[0] - as_xywh[0]).abs() < EPS);
        assert!((xyxy[1] - as_xywh[1]).abs() < EPS);
        assert!((xyxy[2] - (as_xywh[0] + as_xywh[2])).abs() < EPS);
        assert!((xyxy[3] - (as_xywh[1] + as_xywh[3])).abs() < EPS);
    }

    /// Кламп в контейнер: расширенная зона не выходит за панель; пустое
    /// пересечение даёт нулевую зону (а не вывернутый rect).
    #[test]
    fn intersect_clamps_to_container() {
        let panel = [0.0, 0.0, 340.0, 600.0];
        // Строка у края: расширение обрезается границей панели
        let hit = intersect_xywh(expand_xywh([10.0, 0.0, 100.0, 32.0], 44.0), panel);
        assert!((hit[1] - 0.0).abs() < EPS);
        // Контейнер целиком внутри расширенной зоны — контейнер
        assert_eq!(
            intersect_xywh(
                expand_xywh([10.0, 10.0, 4.0, 4.0], 44.0),
                [12.0, 12.0, 2.0, 2.0]
            ),
            [12.0, 12.0, 2.0, 2.0]
        );
        // Непересекающиеся — нули
        let empty = intersect_xywh([0.0, 0.0, 10.0, 10.0], [50.0, 50.0, 10.0, 10.0]);
        assert!((empty[2]).abs() < EPS && empty[3].abs() < EPS);
    }
}
