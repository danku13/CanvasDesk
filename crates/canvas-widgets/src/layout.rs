//! Геометрия виджет-ноды (план M5 §4.4): область контента с инсетом хрома,
//! перевод в физические пиксели экрана, ZoomFactor, размер снапшота.
//!
//! Чистые функции без депа на canvas-render: камера передаётся значимыми
//! полями (центр world, зум, viewport в логических px) — тестируемость на
//! Linux (план M5 §4.1).

/// Заголовок ноды (канвасный хром): совпадает с `cards::HEADER_HEIGHT`
/// (canvas-render), продублирован, чтобы не тянуть GPU-крейт.
pub const HEADER_H: f32 = 28.0;
/// Инсет рамки-зоны слева/справа/снизу (drag за рамку, SPEC §8).
pub const EDGE_INSET: f32 = 8.0;

/// Минимальный ZoomFactor контроллера (ICoreWebView2Controller).
pub const ZOOM_FACTOR_MIN: f32 = 0.25;
/// Максимальный ZoomFactor контроллера (SPEC-ограничение, план П3).
pub const ZOOM_FACTOR_MAX: f32 = 5.0;

/// Кламп снапшота по стороне (план M5 §4.5: 512×512, память текстур).
pub const SNAPSHOT_MAX_SIDE: u32 = 512;

/// Параметры камеры в терминах canvas-widgets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraArgs {
    /// Мировая точка в центре viewport.
    pub center: [f32; 2],
    /// Зум камеры (world→logical).
    pub zoom: f32,
    /// Размер viewport в логических px.
    pub viewport: [f32; 2],
}

impl CameraArgs {
    /// world → логическая точка экрана.
    pub fn world_to_screen(&self, world: [f32; 2]) -> [f32; 2] {
        [
            (world[0] - self.center[0]) * self.zoom + self.viewport[0] / 2.0,
            (world[1] - self.center[1]) * self.zoom + self.viewport[1] / 2.0,
        ]
    }

    /// Логический rect в мировые границы (для видимости нод).
    pub fn visible_world_rect(&self) -> [f32; 4] {
        let hw = self.viewport[0] / 2.0 / self.zoom;
        let hh = self.viewport[1] / 2.0 / self.zoom;
        [
            self.center[0] - hw,
            self.center[1] - hh,
            self.center[0] + hw,
            self.center[1] + hh,
        ]
    }
}

/// Прямоугольник в физических пикселях окна (для SetWindowPos).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PhysRect {
    /// Вырожденный (скрытый) прямоугольник.
    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    /// Пересечение с другим прямоугольником (для airspace, логические
    /// координаты приводятся к тем же осям).
    pub fn intersects(self, other: PhysRect) -> bool {
        !(self.x + self.w <= other.x
            || other.x + other.w <= self.x
            || self.y + self.h <= other.y
            || other.y + other.h <= self.y)
    }
}

/// Геометрия ноды-виджета в мировых координатах.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetGeom {
    /// Полный rect ноды (x, y, w, h) — мировые.
    pub node: [f32; 4],
}

impl WidgetGeom {
    /// Область контента: нода минус заголовок сверху и рамка по бокам/низу
    /// (план M5 §2 «Хром виджет-ноды»).
    pub fn content_rect(&self) -> [f32; 4] {
        [
            self.node[0] + EDGE_INSET,
            self.node[1] + HEADER_H,
            (self.node[2] - 2.0 * EDGE_INSET).max(1.0),
            (self.node[3] - HEADER_H - EDGE_INSET).max(1.0),
        ]
    }
}

/// Rect WebView2 на экране (физические px): контент × зум × scale_factor,
/// округление к целым; вырожденные (<1 px) — скрываются хостом.
pub fn webview_rect(node: &[f32; 4], camera: &CameraArgs, scale_factor: f32) -> PhysRect {
    let geom = WidgetGeom { node: *node };
    let [cx, cy, cw, ch] = geom.content_rect();
    let [sx, sy] = camera.world_to_screen([cx, cy]);
    round_rect(sx, sy, cw * camera.zoom, ch * camera.zoom, scale_factor)
}

/// Rect всего хрома (заголовок + рамка) в мировых координатах — для
/// выделения/hit-теста (клик мимо контента — канвасу).
pub fn chrome_world_rect(node: &[f32; 4]) -> [f32; 4] {
    *node
}

/// ZoomFactor контроллера: зум канваса с клампом 0.25–5.0 (план П3).
pub fn zoom_factor(camera_zoom: f32) -> f32 {
    camera_zoom.clamp(ZOOM_FACTOR_MIN, ZOOM_FACTOR_MAX)
}

/// Размер захвата снапшота: экранная площадь контента с клампом по стороне.
pub fn snapshot_size(node: &[f32; 4], camera: &CameraArgs, scale_factor: f32) -> (u32, u32) {
    let geom = WidgetGeom { node: *node };
    let [_, _, cw, ch] = geom.content_rect();
    let w = (cw * camera.zoom * scale_factor).round().max(1.0);
    let h = (ch * camera.zoom * scale_factor).round().max(1.0);
    let scale = (SNAPSHOT_MAX_SIDE as f32 / w.max(h)).min(1.0);
    ((w * scale).round() as u32, (h * scale).round() as u32)
}

/// Округление к физическим пикселям (снап — чтобы WebView не «дрожал»
/// субпиксельно при неизменной камере).
fn round_rect(x: f32, y: f32, w: f32, h: f32, scale: f32) -> PhysRect {
    let px = (x * scale).round() as i32;
    let py = (y * scale).round() as i32;
    let pw = (w * scale).round() as i32;
    let ph = (h * scale).round() as i32;
    PhysRect {
        x: px,
        y: py,
        w: pw,
        h: ph,
    }
}

/// Пересечение world-rect ноды с прямоугольником (ЛОГИЧЕСКИЕ координаты
/// экрана) — airspace-проверка (П7): оверлеи отдают свои логические rect'ы.
pub fn node_screen_rect_overlaps(
    node: &[f32; 4],
    camera: &CameraArgs,
    scale_factor: f32,
    overlay_logical: &[f32; 4],
) -> bool {
    let geom = WidgetGeom { node: *node };
    let [cx, cy, cw, ch] = geom.content_rect();
    let [sx, sy] = camera.world_to_screen([cx, cy]);
    let node_screen = PhysRect {
        x: (sx * scale_factor).round() as i32,
        y: (sy * scale_factor).round() as i32,
        w: (cw * camera.zoom * scale_factor).round() as i32,
        h: (ch * camera.zoom * scale_factor).round() as i32,
    };
    let overlay = PhysRect {
        x: (overlay_logical[0] * scale_factor).round() as i32,
        y: (overlay_logical[1] * scale_factor).round() as i32,
        w: (overlay_logical[2] * scale_factor).round() as i32,
        h: (overlay_logical[3] * scale_factor).round() as i32,
    };
    node_screen.intersects(overlay)
}

/// Хит-тест клика по виджет-ноде: контент (WebView получает ввод) или хром
/// (заголовок/рамка — канвасу: выделение/drag/resize/порты).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetHit {
    /// Контентная область — ввод уходит WebView2 (host).
    Content,
    /// Заголовок — drag/выделение канвасом.
    Header,
    /// Боковая/нижняя рамка — drag канвасом.
    Frame,
    /// Мимо ноды.
    Miss,
}

/// Куда попал клик в мировых координатах.
pub fn hit_test(node: &[f32; 4], world: [f32; 2]) -> WidgetHit {
    let [x, y, w, h] = *node;
    let [px, py] = world;
    if px < x || px > x + w || py < y || py > y + h {
        return WidgetHit::Miss;
    }
    if py < y + HEADER_H {
        return WidgetHit::Header;
    }
    if px < x + EDGE_INSET || px > x + w - EDGE_INSET || py > y + h - EDGE_INSET {
        return WidgetHit::Frame;
    }
    WidgetHit::Content
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera() -> CameraArgs {
        CameraArgs {
            center: [0.0, 0.0],
            zoom: 1.0,
            viewport: [1000.0, 600.0],
        }
    }

    #[test]
    fn content_rect_insets() {
        let node = [100.0, 200.0, 320.0, 200.0];
        let [cx, cy, cw, ch] = WidgetGeom { node }.content_rect();
        assert_eq!((cx, cy), (108.0, 228.0), "инсет 8 слева, 28 сверху");
        assert_eq!((cw, ch), (304.0, 164.0), "8+8 по бокам, 28+8 по вертикали");
    }

    #[test]
    fn content_rect_min_size_degenerate_node() {
        // Вырожденная нода: контент не уходит в минус, min 1×1
        let [cx, cy, cw, ch] = WidgetGeom {
            node: [0.0, 0.0, 10.0, 10.0],
        }
        .content_rect();
        assert_eq!((cw, ch), (1.0, 1.0));
        assert_eq!(cx, 8.0);
        assert_eq!(cy, 28.0);
    }

    #[test]
    fn webview_rect_at_identity() {
        let cam = camera();
        let node = [0.0, 0.0, 320.0, 200.0];
        let r = webview_rect(&node, &cam, 1.0);
        // Центр камеры (0,0) → экран (500, 300); контент начинается с +8/+28
        assert_eq!((r.x, r.y), (508, 328));
        assert_eq!((r.w, r.h), (304, 164));
    }

    #[test]
    fn webview_rect_zoom_and_dpi() {
        let node = [0.0, 0.0, 320.0, 200.0];
        // Зум 2.0, scale 1.5: размеры 304*2*1.5=912, 164*2*1.5=492
        let cam = CameraArgs {
            zoom: 2.0,
            ..camera()
        };
        let r = webview_rect(&node, &cam, 1.5);
        assert_eq!(r.w, 912);
        assert_eq!(r.h, 492);
        // Позиция: (8*2*1.5)+756... проверим через формулу
        let [sx, sy] = cam.world_to_screen([8.0, 28.0]);
        assert_eq!(r.x, (sx * 1.5).round() as i32);
        assert_eq!(r.y, (sy * 1.5).round() as i32);
    }

    #[test]
    fn webview_rect_rounds_stably() {
        let node = [0.0, 0.0, 320.0, 200.0];
        let cam = camera();
        let a = webview_rect(&node, &cam, 1.0);
        let b = webview_rect(&node, &cam, 1.0);
        assert_eq!(a, b, "одинаковая камера → одинаковый rect (снап)");
        // Дрожь субпикселя: z=1.00001 не должна менять целые пиксели здесь
        let cam2 = CameraArgs {
            zoom: 1.0000001,
            ..cam
        };
        let c = webview_rect(&node, &cam2, 1.0);
        assert_eq!(a, c);
    }

    #[test]
    fn zoom_factor_clamp() {
        assert_eq!(zoom_factor(0.10), 0.25);
        assert_eq!(zoom_factor(0.25), 0.25);
        assert_eq!(zoom_factor(1.0), 1.0);
        assert_eq!(zoom_factor(5.0), 5.0);
        assert_eq!(zoom_factor(8.0), 5.0, "за пределами — кламп (план П3)");
    }

    #[test]
    fn snapshot_size_clamped() {
        let cam = camera();
        let node = [0.0, 0.0, 2000.0, 1500.0];
        // Контент 1984×1464, зум 1, scale 1 → кламп к 512
        let (w, h) = snapshot_size(&node, &cam, 1.0);
        assert_eq!(w, 512);
        assert!(h <= 512, "пропорции сохранены: {h}");
        // Малая нода — без клампа
        let (w, h) = snapshot_size(&[0.0, 0.0, 320.0, 200.0], &cam, 1.0);
        assert_eq!((w, h), (304, 164));
    }

    #[test]
    fn overlap_detection_for_airspace() {
        let cam = camera();
        let node = [0.0, 0.0, 320.0, 200.0];
        // Контент на экране: x 508..812, y 328..492
        let far = [900.0, 100.0, 100.0, 100.0];
        assert!(!node_screen_rect_overlaps(&node, &cam, 1.0, &far));
        let near = [700.0, 400.0, 300.0, 200.0];
        assert!(node_screen_rect_overlaps(&node, &cam, 1.0, &near));
        let touching_edge = [812.0, 100.0, 50.0, 50.0];
        // Касание — не перекрытие (строгое «>», не «≥»)
        assert!(!node_screen_rect_overlaps(&node, &cam, 1.0, &touching_edge));
    }

    #[test]
    fn hit_test_zones() {
        let node = [100.0, 100.0, 320.0, 200.0];
        assert_eq!(
            hit_test(&node, [200.0, 110.0]),
            WidgetHit::Header,
            "заголовок"
        );
        assert_eq!(
            hit_test(&node, [103.0, 200.0]),
            WidgetHit::Frame,
            "левая рамка"
        );
        assert_eq!(
            hit_test(&node, [415.0, 200.0]),
            WidgetHit::Frame,
            "правая рамка"
        );
        assert_eq!(
            hit_test(&node, [200.0, 295.0]),
            WidgetHit::Frame,
            "нижняя рамка"
        );
        assert_eq!(
            hit_test(&node, [200.0, 200.0]),
            WidgetHit::Content,
            "контент"
        );
        assert_eq!(hit_test(&node, [50.0, 50.0]), WidgetHit::Miss, "мимо");
    }

    #[test]
    fn visible_world_rect_math() {
        let cam = CameraArgs {
            center: [100.0, 50.0],
            zoom: 2.0,
            viewport: [400.0, 200.0],
        };
        let [x0, y0, x1, y1] = cam.visible_world_rect();
        assert_eq!(x0, 0.0);
        assert_eq!(x1, 200.0);
        assert_eq!(y0, 0.0);
        assert_eq!(y1, 100.0);
    }
}
