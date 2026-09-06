//! Замер производительности кадра для HUD (T5): fps и p95 frame time.
//!
//! Чистая логика без GPU — юнит-тестируется. Рендер-цикл идёт по
//! `request_redraw`, поэтому интервалы осмысленны во время активного
//! пан/зума (кадры рисуются подряд); в простое кадров нет и замер не растёт.

use std::collections::VecDeque;
use std::time::Duration;

/// Размер окна замера: p95 считается по последним 300 кадрам (TASKS T5).
pub const FRAME_WINDOW: usize = 300;

/// Кольцевой буфер интервалов между кадрами.
pub struct FrameMeter {
    samples: VecDeque<Duration>,
}

impl FrameMeter {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(FRAME_WINDOW),
        }
    }

    /// Добавить интервал между текущим и предыдущим кадром.
    pub fn push(&mut self, dt: Duration) {
        if self.samples.len() == FRAME_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(dt);
    }

    /// Средний fps по окну; None, если замеров ещё нет.
    pub fn fps(&self) -> Option<f32> {
        if self.samples.is_empty() {
            return None;
        }
        let total: Duration = self.samples.iter().sum();
        let secs = total.as_secs_f32();
        if secs <= 0.0 {
            return None;
        }
        Some(self.samples.len() as f32 / secs)
    }

    /// 95-й персентиль frame time в миллисекундах; None, если замеров нет.
    pub fn p95_ms(&self) -> Option<f32> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<f32> = self
            .samples
            .iter()
            .map(|dt| dt.as_secs_f32() * 1000.0)
            .collect();
        sorted.sort_by(f32::total_cmp);
        let idx = (0.95 * (sorted.len() - 1) as f32).ceil() as usize;
        sorted.get(idx).copied()
    }
}

impl Default for FrameMeter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Пустой замер — None, без паники.
    #[test]
    fn empty_meter() {
        let meter = FrameMeter::new();
        assert_eq!(meter.fps(), None);
        assert_eq!(meter.p95_ms(), None);
    }

    /// Синтетические кадры по 16.67 мс дают ~60 fps.
    #[test]
    fn fps_from_synthetic_intervals() {
        let mut meter = FrameMeter::new();
        for _ in 0..60 {
            meter.push(Duration::from_secs_f32(1.0 / 60.0));
        }
        let fps = meter.fps().expect("fps есть после замеров");
        assert!(
            (fps - 60.0).abs() < 0.5,
            "ожидалось ~60 fps, получено {fps}"
        );
    }

    /// p95 на известном наборе: 95 быстрых + 5 медленных кадров.
    #[test]
    fn p95_picks_slow_tail() {
        let mut meter = FrameMeter::new();
        for _ in 0..95 {
            meter.push(Duration::from_millis(10));
        }
        for _ in 0..5 {
            meter.push(Duration::from_millis(50));
        }
        let p95 = meter.p95_ms().expect("p95 есть после замеров");
        assert!(
            (p95 - 50.0).abs() < 0.01,
            "p95 должен попасть в медленный хвост, получено {p95}"
        );
    }

    /// Буфер ограничен 300 кадрами: старые вытесняются.
    #[test]
    fn window_evicts_old_samples() {
        let mut meter = FrameMeter::new();
        for _ in 0..FRAME_WINDOW {
            meter.push(Duration::from_millis(100)); // 10 fps
        }
        for _ in 0..FRAME_WINDOW {
            meter.push(Duration::from_millis(16)); // ~62 fps
        }
        let fps = meter.fps().expect("fps есть");
        assert!(
            fps > 50.0,
            "старые медленные кадры должны быть вытеснены, fps={fps}"
        );
        let p95 = meter.p95_ms().expect("p95 есть");
        assert!((p95 - 16.0).abs() < 0.01);
    }
}
