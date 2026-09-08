//! Анимации приложения (T14): полёт камеры к результату поиска (300 мс,
//! ease-out) и пульс подсветки ноды. Чистая математика времени — без wgpu/winit,
//! покрывается юнит-тестами.

/// Длительность полёта камеры к ноде (TASKS T14).
pub const FLIGHT_DURATION_MS: u32 = 300;
/// Длительность пульса подсветки ноды (план T14 §3).
pub const PULSE_TOTAL_MS: u32 = 1200;

/// Ease-out cubic: быстрый старт, плавное докатывание. t клампится в [0, 1].
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Полёт камеры: интерполяция центра и зума от текущего к целевому
/// (кламп зума выполняет `Camera::set_zoom`, не здесь).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    start_center: [f32; 2],
    start_zoom: f32,
    target_center: [f32; 2],
    target_zoom: f32,
    duration_ms: u32,
}

impl Flight {
    /// Новый полёт: старт/цель (центр viewport в world + зум), длительность
    /// (`FLIGHT_DURATION_MS` в приложении). `duration_ms == 0` — мгновенный
    /// прыжок (sample всегда возвращает цель).
    pub fn new(
        start_center: [f32; 2],
        start_zoom: f32,
        target_center: [f32; 2],
        target_zoom: f32,
        duration_ms: u32,
    ) -> Self {
        Self {
            start_center,
            start_zoom,
            target_center,
            target_zoom,
            duration_ms,
        }
    }

    /// Цель полёта: (центр, зум).
    pub const fn target(&self) -> ([f32; 2], f32) {
        (self.target_center, self.target_zoom)
    }

    /// Состояние камеры в момент `elapsed_ms` от старта: ease-out cubic
    /// по центру и зуму; за пределами длительности — цель.
    pub fn sample(&self, elapsed_ms: u32) -> ([f32; 2], f32) {
        let progress = if self.duration_ms == 0 {
            1.0
        } else {
            ease_out_cubic(elapsed_ms as f32 / self.duration_ms as f32)
        };
        let center = [
            self.start_center[0] + (self.target_center[0] - self.start_center[0]) * progress,
            self.start_center[1] + (self.target_center[1] - self.start_center[1]) * progress,
        ];
        let zoom = self.start_zoom + (self.target_zoom - self.start_zoom) * progress;
        (center, zoom)
    }

    /// Полёт завершён к моменту `elapsed_ms`.
    pub const fn is_finished(&self, elapsed_ms: u32) -> bool {
        elapsed_ms >= self.duration_ms
    }
}

/// Альфа пульса подсветки ноды: 1.0 в момент прыжка, линейное затухание до 0
/// за `PULSE_TOTAL_MS`; после — 0.0 (пульс выключен).
pub fn pulse_alpha(elapsed_ms: u32) -> f32 {
    if elapsed_ms >= PULSE_TOTAL_MS {
        0.0
    } else {
        1.0 - elapsed_ms as f32 / PULSE_TOTAL_MS as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    #[test]
    fn ease_out_bounds_and_shape() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < EPS);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < EPS);
        // кламп за границами
        assert!((ease_out_cubic(-1.0) - 0.0).abs() < EPS);
        assert!((ease_out_cubic(2.0) - 1.0).abs() < EPS);
        // «докатывание»: производная в конце меньше, чем в начале
        let mid = ease_out_cubic(0.5);
        assert!(mid > 0.5, "ease-out опережает линейный в середине: {mid}");
    }

    #[test]
    fn flight_start_target_and_finish() {
        let flight = Flight::new([0.0, 0.0], 0.5, [100.0, -40.0], 1.2, FLIGHT_DURATION_MS);
        let (c0, z0) = flight.sample(0);
        assert!((c0[0] - 0.0).abs() < EPS && (c0[1] - 0.0).abs() < EPS);
        assert!((z0 - 0.5).abs() < EPS);
        let (c1, z1) = flight.sample(FLIGHT_DURATION_MS);
        assert!((c1[0] - 100.0).abs() < EPS && (c1[1] + 40.0).abs() < EPS);
        assert!((z1 - 1.2).abs() < EPS);
        // за длительностью — цель
        let (c2, _) = flight.sample(FLIGHT_DURATION_MS + 5000);
        assert!((c2[0] - 100.0).abs() < EPS);
        assert!(flight.is_finished(FLIGHT_DURATION_MS));
        assert!(!flight.is_finished(FLIGHT_DURATION_MS - 1));
    }

    #[test]
    fn flight_zero_duration_is_instant() {
        let flight = Flight::new([0.0, 0.0], 1.0, [10.0, 10.0], 2.0, 0);
        let (c, z) = flight.sample(0);
        assert!((c[0] - 10.0).abs() < EPS && (c[1] - 10.0).abs() < EPS);
        assert!((z - 2.0).abs() < EPS);
    }

    #[test]
    fn pulse_decays_to_zero() {
        assert!((pulse_alpha(0) - 1.0).abs() < EPS);
        assert!((pulse_alpha(PULSE_TOTAL_MS) - 0.0).abs() < EPS);
        assert_eq!(pulse_alpha(PULSE_TOTAL_MS + 1), 0.0);
        let half = pulse_alpha(PULSE_TOTAL_MS / 2);
        assert!(half > 0.49 && half < 0.51, "середина = 0.5: {half}");
    }

    /// Интерполяция полёта монотонна (план T14 §6): с ростом времени центр и
    /// зум движутся к цели без откатов (ease-out — убывающая скорость, не
    /// отрицательная), значения не перелетают цель.
    #[test]
    fn flight_progress_is_monotonic() {
        let flight = Flight::new([0.0, 0.0], 0.5, [100.0, 50.0], 2.0, FLIGHT_DURATION_MS);
        let mut prev = flight.sample(0);
        for t in (0..=FLIGHT_DURATION_MS).step_by(25) {
            let (center, zoom) = flight.sample(t);
            assert!(
                center[0] >= prev.0[0] - EPS && center[1] >= prev.0[1] - EPS,
                "откат центра при t={t}: {prev:?} -> {center:?}"
            );
            assert!(zoom >= prev.1 - EPS, "откат зума при t={t}");
            // не дальше цели
            assert!(center[0] <= 100.0 + EPS && center[1] <= 50.0 + EPS);
            assert!(zoom <= 2.0 + EPS);
            prev = (center, zoom);
        }
    }

    /// Пульс не усиливается со временем и после PULSE_TOTAL_MS остаётся нулём.
    #[test]
    fn pulse_alpha_never_increases() {
        let mut prev = pulse_alpha(0);
        for t in (0..=PULSE_TOTAL_MS).step_by(100) {
            let alpha = pulse_alpha(t);
            assert!(alpha <= prev + EPS, "альфа выросла при t={t}: {alpha}");
            prev = alpha;
        }
        assert_eq!(pulse_alpha(PULSE_TOTAL_MS + 100), 0.0);
    }
}
