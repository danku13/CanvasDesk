//! Камера канваса: преобразования screen↔world, панорамирование, зум (T2).
//!
//! Чистая математика без зависимостей от winit/wgpu — модуль тестируется юнит-тестами.
//! Screen-координаты — логические пиксели (SPEC §6.5); world — координаты канваса.

/// Минимальный зум камеры (SPEC §8).
pub const MIN_ZOOM: f32 = 0.05;
/// Максимальный зум камеры (SPEC §8).
pub const MAX_ZOOM: f32 = 4.0;

/// Двумерный вектор в логических пикселях (screen или world — по контексту вызова).
pub type Vec2 = [f32; 2];

/// Камера: мировая точка в центре viewport + зум.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    position: Vec2,
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            zoom: 1.0,
        }
    }
}

impl Camera {
    /// Мировая точка, отображаемая в центре viewport.
    pub fn position(&self) -> Vec2 {
        self.position
    }

    /// Установить центр viewport в world-координатах (T14: полёт камеры к
    /// результату поиска, центрирование по клику на миникарте T13).
    pub fn set_center(&mut self, world: Vec2) {
        self.position = world;
    }

    /// Установить абсолютный зум (кламп [MIN_ZOOM, MAX_ZOOM]) без якоря —
    /// анимация полёта (T14) интерполирует зум по кадрам, якорь не нужен.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Текущий зум (1.0 — 1 world-px = 1 screen-px).
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Перевести screen-координату (логические px) в world.
    /// `viewport` — размер viewport в логических px.
    pub fn screen_to_world(&self, screen: Vec2, viewport: Vec2) -> Vec2 {
        [
            self.position[0] + (screen[0] - viewport[0] / 2.0) / self.zoom,
            self.position[1] + (screen[1] - viewport[1] / 2.0) / self.zoom,
        ]
    }

    /// Перевести world-координату в screen (логические px).
    pub fn world_to_screen(&self, world: Vec2, viewport: Vec2) -> Vec2 {
        [
            (world[0] - self.position[0]) * self.zoom + viewport[0] / 2.0,
            (world[1] - self.position[1]) * self.zoom + viewport[1] / 2.0,
        ]
    }

    /// Панорамирование: сдвиг содержимого на `delta_screen` screen-пикселей.
    pub fn pan(&mut self, delta_screen: Vec2) {
        self.position[0] -= delta_screen[0] / self.zoom;
        self.position[1] -= delta_screen[1] / self.zoom;
    }

    /// Видимый прямоугольник в world-координатах: [min_x, min_y, max_x, max_y].
    /// Используется для culling по spatial index (T5). Нулевой viewport
    /// (свёрнутое окно) даёт вырожденный rect вокруг центра — без паники.
    pub fn visible_world_rect(&self, viewport: Vec2) -> [f32; 4] {
        let top_left = self.screen_to_world([0.0, 0.0], viewport);
        let bottom_right = self.screen_to_world(viewport, viewport);
        [
            top_left[0].min(bottom_right[0]),
            top_left[1].min(bottom_right[1]),
            top_left[0].max(bottom_right[0]),
            top_left[1].max(bottom_right[1]),
        ]
    }

    /// Зум множителем к screen-точке `anchor`: мировая точка под курсором неподвижна.
    /// Результат клампится в [MIN_ZOOM, MAX_ZOOM].
    pub fn zoom_at(&mut self, factor: f32, anchor: Vec2, viewport: Vec2) {
        let target = self.zoom * factor;
        self.set_zoom_at(target, anchor, viewport);
    }

    /// Установить абсолютный зум к screen-точке `anchor` (для pinch-жеста).
    pub fn set_zoom_at(&mut self, zoom: f32, anchor: Vec2, viewport: Vec2) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        let world_anchor = self.screen_to_world(anchor, viewport);
        self.zoom = zoom;
        // Подбираем position так, чтобы world_anchor остался на месте на экране
        self.position = [
            world_anchor[0] - (anchor[0] - viewport[0] / 2.0) / self.zoom,
            world_anchor[1] - (anchor[1] - viewport[1] / 2.0) / self.zoom,
        ];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-3;

    fn assert_vec2_close(a: Vec2, b: Vec2, eps: f32) {
        assert!(
            (a[0] - b[0]).abs() <= eps && (a[1] - b[1]).abs() <= eps,
            "ожидалось {b:?}, получено {a:?}"
        );
    }

    /// Round-trip screen→world→screen и world→screen→world при разных зумах.
    #[test]
    fn screen_world_round_trip() {
        let viewport = [1280.0, 720.0];
        for zoom in [MIN_ZOOM, 0.5, 1.0, 2.5, MAX_ZOOM] {
            let mut camera = Camera::default();
            camera.set_zoom_at(zoom, [0.0, 0.0], viewport);
            camera.pan([123.0, -45.0]);
            assert!((camera.zoom() - zoom).abs() < EPS);

            for screen in [[0.0, 0.0], [640.0, 360.0], [1279.0, 719.0], [33.3, 700.7]] {
                let world = camera.screen_to_world(screen, viewport);
                let back = camera.world_to_screen(world, viewport);
                // на MIN_ZOOM мировые координаты большие — точность f32 хуже
                let eps = if zoom <= 0.1 { 0.1 } else { EPS };
                assert_vec2_close(back, screen, eps);

                let screen2 = camera.world_to_screen(world, viewport);
                let world2 = camera.screen_to_world(screen2, viewport);
                assert_vec2_close(world2, world, eps * 2.0);
            }
        }
    }

    /// Зум к курсору: мировая точка под anchor не смещается.
    #[test]
    fn zoom_at_keeps_anchor_fixed() {
        let viewport = [800.0, 600.0];
        let mut camera = Camera::default();
        camera.pan([100.0, 50.0]);
        for anchor in [[0.0, 0.0], [400.0, 300.0], [799.0, 599.0]] {
            let before = camera.screen_to_world(anchor, viewport);
            camera.zoom_at(1.3, anchor, viewport);
            let after = camera.screen_to_world(anchor, viewport);
            assert_vec2_close(after, before, EPS);
        }
    }

    /// Зум клампится в [0.05, 4.0].
    #[test]
    fn zoom_is_clamped() {
        let viewport = [800.0, 600.0];
        let mut camera = Camera::default();
        camera.zoom_at(1000.0, [400.0, 300.0], viewport);
        assert_eq!(camera.zoom(), MAX_ZOOM);
        camera.zoom_at(0.0001, [400.0, 300.0], viewport);
        assert_eq!(camera.zoom(), MIN_ZOOM);
    }

    /// Панорамирование на N screen-px сдвигает центр viewport на N/zoom world-px.
    #[test]
    fn pan_moves_center_by_delta_over_zoom() {
        let viewport = [800.0, 600.0];
        let mut camera = Camera::default();
        camera.set_zoom_at(2.0, [400.0, 300.0], viewport);
        let center_before = camera.screen_to_world([400.0, 300.0], viewport);
        camera.pan([40.0, -20.0]);
        let center_after = camera.screen_to_world([400.0, 300.0], viewport);
        assert_vec2_close(
            [
                center_after[0] - center_before[0],
                center_after[1] - center_before[1],
            ],
            [-20.0, 10.0], // -delta / zoom
            EPS,
        );
    }

    /// Позиция камеры после панорамирования согласована с screen_to_world центра.
    #[test]
    fn position_matches_viewport_center() {
        let viewport = [1024.0, 768.0];
        let mut camera = Camera::default();
        camera.pan([77.0, 33.0]);
        let center = camera.screen_to_world([512.0, 384.0], viewport);
        assert_vec2_close(center, camera.position(), EPS);
    }

    /// Нулевой viewport (свёрнутое окно) не паникует.
    #[test]
    fn zero_viewport_does_not_panic() {
        let mut camera = Camera::default();
        camera.zoom_at(1.5, [0.0, 0.0], [0.0, 0.0]);
        let world = camera.screen_to_world([10.0, 10.0], [0.0, 0.0]);
        let back = camera.world_to_screen(world, [0.0, 0.0]);
        assert_vec2_close(back, [10.0, 10.0], EPS);
    }

    /// visible_world_rect: при zoom=1 rect = viewport вокруг центра камеры;
    /// при других zoom границы совпадают с проекциями углов экрана.
    #[test]
    fn visible_world_rect_matches_screen_corners() {
        let viewport = [1280.0, 720.0];
        for zoom in [MIN_ZOOM, 0.5, 1.0, 2.5, MAX_ZOOM] {
            let mut camera = Camera::default();
            camera.set_zoom_at(zoom, [640.0, 360.0], viewport);
            camera.pan([55.0, -30.0]);

            let rect = camera.visible_world_rect(viewport);
            let eps = if zoom <= 0.1 { 0.5 } else { EPS };
            assert_vec2_close(
                [rect[0], rect[1]],
                camera.screen_to_world([0.0, 0.0], viewport),
                eps,
            );
            assert_vec2_close(
                [rect[2], rect[3]],
                camera.screen_to_world(viewport, viewport),
                eps,
            );
            assert!(
                rect[0] <= rect[2] && rect[1] <= rect[3],
                "min <= max: {rect:?}"
            );
        }

        // zoom=1, камера в начале координат: rect = [-w/2, -h/2, w/2, h/2]
        let camera = Camera::default();
        assert_eq!(
            camera.visible_world_rect(viewport),
            [-640.0, -360.0, 640.0, 360.0]
        );

        // Нулевой viewport — вырожденный rect в центре, без паники
        let rect = Camera::default().visible_world_rect([0.0, 0.0]);
        assert_eq!(rect, [0.0, 0.0, 0.0, 0.0]);
    }

    /// set_center перемещает центр viewport (T14: полёт камеры к результату
    /// поиска, T13: клик по миникарте), зум не задет.
    #[test]
    fn set_center_moves_position() {
        let viewport = [1280.0, 720.0];
        let mut camera = Camera::default();
        camera.set_center([100.0, -50.0]);
        assert_eq!(camera.position(), [100.0, -50.0]);
        // зум не задет
        assert!((camera.zoom() - 1.0).abs() < EPS);
        // position — центр viewport (согласовано с screen_to_world)
        let center = camera.screen_to_world([640.0, 360.0], viewport);
        assert_vec2_close(center, camera.position(), EPS);
        // повторная установка перезаписывает
        camera.set_center([0.0, 0.0]);
        assert_eq!(camera.position(), [0.0, 0.0]);
    }

    /// set_zoom клампится в [MIN_ZOOM, MAX_ZOOM] (T14: анимация полёта
    /// интерполирует зум по кадрам) и не смещает позицию — якорь не нужен.
    #[test]
    fn set_zoom_clamps_into_range() {
        let mut camera = Camera::default();
        camera.set_center([10.0, 20.0]);
        camera.set_zoom(100.0);
        assert_eq!(camera.zoom(), MAX_ZOOM);
        camera.set_zoom(0.0001);
        assert_eq!(camera.zoom(), MIN_ZOOM);
        camera.set_zoom(-3.0);
        assert_eq!(camera.zoom(), MIN_ZOOM);
        camera.set_zoom(1.25);
        assert!((camera.zoom() - 1.25).abs() < EPS);
        // set_zoom без якоря не двигает центр
        assert_eq!(camera.position(), [10.0, 20.0]);
    }
}
