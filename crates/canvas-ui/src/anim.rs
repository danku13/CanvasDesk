//! FR-055 (этап U4 PRD-0009): kit-анимации — L3-перенос идеи
//! `egui::animation_manager` (`animate_bool`/`animate_value`, Приложение А
//! FR-051): линейная интерполяция, детерминированная по `dt` (без
//! глобального времени — headless-тестируемо, повторяемость кадров).
//! Источник таймингов — motion-токены PRD-0006 (потребитель передаёт
//! скорость/длительность аргументом; кит не знает конкретных значений).

/// Линейная интерполяция `from → to` при доле пути `t01` (зажата 0..1).
pub fn animate_value(from: f32, to: f32, t01: f32) -> f32 {
    from + (to - from) * t01.clamp(0.0, 1.0)
}

/// Плавный тогл 0..1 (egui `animate_bool`): значение движется к цели
/// линейно со скоростью `speed_per_sec` (1.0 — полный ход за 1 с).
/// Детерминизм: результат — функция только (значение, цель, dt, скорость).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoolAnim {
    value: f32,
}

impl Default for BoolAnim {
    fn default() -> Self {
        Self::new()
    }
}

impl BoolAnim {
    pub const fn new() -> Self {
        Self { value: 0.0 }
    }

    /// Начальное значение (1.0 — сразу открыто).
    pub const fn started(open: bool) -> Self {
        Self {
            value: if open { 1.0 } else { 0.0 },
        }
    }

    /// Текущее значение 0..1.
    pub const fn value(&self) -> f32 {
        self.value
    }

    /// Шаг кадра: движение к цели; dt ≤ 0 — без изменений (пауза кадра).
    /// Значение зажато в 0..1; при достижении цели — ровно 0.0/1.0.
    pub fn step(&mut self, target: bool, dt_seconds: f32, speed_per_sec: f32) -> f32 {
        let goal = if target { 1.0 } else { 0.0 };
        if dt_seconds > 0.0 && speed_per_sec > 0.0 && self.value != goal {
            let d = speed_per_sec * dt_seconds;
            let v = if goal > self.value {
                (self.value + d).min(goal)
            } else {
                (self.value - d).max(goal)
            };
            self.value = v.clamp(0.0, 1.0);
        }
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animate_value_clamps_t() {
        assert!((animate_value(0.0, 10.0, -0.5)).abs() < 1e-6);
        assert!((animate_value(0.0, 10.0, 0.25) - 2.5).abs() < 1e-6);
        assert!((animate_value(0.0, 10.0, 2.0) - 10.0).abs() < 1e-6);
        // Обратное направление
        assert!((animate_value(10.0, 0.0, 0.5) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn bool_anim_approaches_monotonically_and_clamps() {
        let mut a = BoolAnim::new();
        let mut prev = a.value();
        for _ in 0..20 {
            let v = a.step(true, 0.05, 2.0);
            assert!(v >= prev - 1e-6, "монотонный рост");
            assert!((0.0..=1.0).contains(&v));
            prev = v;
        }
        assert!((a.value() - 1.0).abs() < 1e-6, "цель достигнута ровно");
        // Обратно к 0
        let mut prev = a.step(false, 0.05, 2.0);
        for _ in 0..20 {
            let v = a.step(false, 0.05, 2.0);
            assert!(v <= prev + 1e-6, "монотонное убывание");
            prev = v;
        }
        assert!((a.value()).abs() < 1e-6);
    }

    #[test]
    fn bool_anim_is_dt_deterministic() {
        // Сумма мелких шагов == один большой шаг (линейность + детерминизм)
        let mut fine = BoolAnim::new();
        for _ in 0..10 {
            fine.step(true, 0.02, 1.0);
        }
        let mut coarse = BoolAnim::new();
        coarse.step(true, 0.2, 1.0);
        assert!((fine.value() - coarse.value()).abs() < 1e-5);
        // dt = 0 — пауза (значение не меняется)
        let mut paused = BoolAnim::started(true);
        assert!((paused.step(false, 0.0, 1.0) - 1.0).abs() < 1e-6);
    }
}
