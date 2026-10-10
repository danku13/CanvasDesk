//! FR-055 (этап U4 PRD-0009): kit-анимации — L3-перенос идеи
//! `egui::animation_manager` (`animate_bool`/`animate_value`, Приложение А
//! FR-051): линейная интерполяция, детерминированная по `dt` (без
//! глобального времени — headless-тестируемо, повторяемость кадров).
//! Источник таймингов — motion-токены PRD-0006 (потребитель передаёт
//! скорость/длительность аргументом; кит не знает конкретных значений).
//!
//! Wave T (2026-10, ui-kit-deep-review §5.2.4): формализация motion-шкалы
//! [`Duration`] + [`Easing`] (Material 3 spec: 12 durations + 6 easings).
//! `effective_duration()` уважает `prefers-reduced-motion` (a11y).

use std::sync::atomic::AtomicBool;

static REDUCED_MOTION_OVERRIDE: AtomicBool = AtomicBool::new(false);

// --- Wave T §5.2.4: Motion tokens (Material 3 spec) ------------------------

/// Motion duration scale (Wave T §5.2.4, Material 3 spec — 12 шагов).
///
/// Используется в анимациях hover/press/focus/modal-enter/scene-transition.
/// Honor `prefers-reduced-motion` через [`effective_duration`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    /// 50ms — hover feedback, micro-interactions.
    Short1 = 50,
    /// 100ms — press, toggle, chip expand.
    Short2 = 100,
    /// 150ms — small panel slide, tooltip.
    Short3 = 150,
    /// 200ms — chip expand, small surface.
    Short4 = 200,
    /// 250ms — tooltip show, small modal.
    Medium1 = 250,
    /// 300ms — panel expand, camera flight.
    Medium2 = 300,
    /// 350ms — modal enter.
    Medium3 = 350,
    /// 400ms — large surface transition.
    Medium4 = 400,
    /// 450ms — scene transition.
    Long1 = 450,
    /// 500ms — onboarding step.
    Long2 = 500,
    /// 550ms — theme switch.
    Long3 = 550,
    /// 600ms — max (splash, full scene).
    Long4 = 600,
}

impl Duration {
    /// Длительность в миллисекундах.
    pub const fn ms(self) -> u64 {
        self as u64
    }
    /// Длительность в секундах (для `dt_seconds` в [`BoolAnim::step`]).
    pub const fn sec(self) -> f32 {
        self as u32 as f32 / 1000.0
    }
}

/// Easing curves (Wave T §5.2.4, Material 3 spec — 6 кривых).
///
/// Возвращает контрольные точки cubic-bezier для использования в
/// интерполяции. Для linear — `[0.0, 0.0, 1.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    /// Standard — общая анимация (cubic-bezier(0.2, 0, 0, 1)).
    Standard,
    /// Standard decelerate — элемент входит (0, 0, 0, 1).
    StandardDecelerate,
    /// Standard accelerate — элемент выходит (0.3, 0, 1, 1).
    StandardAccelerate,
    /// Emphasized — акцентная анимация (0.2, 0, 0, 1).
    Emphasized,
    /// Emphasized decelerate — крупный вход (0.05, 0.7, 0.1, 1).
    EmphasizedDecelerate,
    /// Emphasized accelerate — крупный выход (0.3, 0, 0.8, 0.15).
    EmphasizedAccelerate,
}

impl Easing {
    /// Контрольные точки cubic-bezier [x1, y1, x2, y2].
    pub const fn bezier(self) -> [f32; 4] {
        match self {
            Self::Standard => [0.2, 0.0, 0.0, 1.0],
            Self::StandardDecelerate => [0.0, 0.0, 0.0, 1.0],
            Self::StandardAccelerate => [0.3, 0.0, 1.0, 1.0],
            Self::Emphasized => [0.2, 0.0, 0.0, 1.0],
            Self::EmphasizedDecelerate => [0.05, 0.7, 0.1, 1.0],
            Self::EmphasizedAccelerate => [0.3, 0.0, 0.8, 0.15],
        }
    }
}

/// Эффективная длительность с учётом `prefers-reduced-motion` (Wave T §5.2.4).
///
/// Если reduced-motion активен — возвращает 0 (мгновенная анимация).
/// Источник флага — `canvas_core::web_bridge` (matchMedia `prefers-reduced-motion`
/// в canvas-web); fallback `false` (не активен) вне web.
pub fn effective_duration(d: Duration) -> u64 {
    if reduced_motion() {
        0
    } else {
        d.ms()
    }
}

/// Флаг `prefers-reduced-motion` (Wave T §5.2.4).
/// В web — из `matchMedia("(prefers-reduced-motion: reduce)")` через
/// `canvas_core::web_bridge`; вне web — `false` (не активен).
pub fn reduced_motion() -> bool {
    // canvas_core::web_bridge недоступен из canvas-ui (G7: kit не зависит от
    // рендера/ОС). Consumer передаёт флаг явно через set_reduced_motion_override
    // или использует effective_duration_with(flag).
    REDUCED_MOTION_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Явный флаг reduced-motion (устанавливает consumer из web_bridge).
pub fn set_reduced_motion_override(v: bool) {
    REDUCED_MOTION_OVERRIDE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// Эффективная длительность с явным флагом (без глобального состояния).
pub fn effective_duration_with(d: Duration, reduced: bool) -> u64 {
    if reduced {
        0
    } else {
        d.ms()
    }
}

/// Apply easing to a linear progress value (0..1 → 0..1 eased).
/// cubic-bezier approximation via simple polynomial (good enough for UI).
pub fn ease(t01: f32, easing: Easing) -> f32 {
    let t = t01.clamp(0.0, 1.0);
    let [x1, y1, x2, y2] = easing.bezier();
    // Simple cubic bezier approximation (de Casteljau, 4 steps).
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let p01x = lerp(0.0, x1, t);
    let p01y = lerp(0.0, y1, t);
    let p12x = lerp(x1, x2, t);
    let p12y = lerp(y1, y2, t);
    let p23x = lerp(x2, 1.0, t);
    let p23y = lerp(y2, 1.0, t);
    let p012x = lerp(p01x, p12x, t);
    let p012y = lerp(p01y, p12y, t);
    let p123x = lerp(p12x, p23x, t);
    let p123y = lerp(p12y, p23y, t);
    let _final_x = lerp(p012x, p123x, t);
    lerp(p012y, p123y, t)
}

// --- Существующие анимации (FR-055) -----------------------------------------

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

    // --- Wave T §5.2.4: Duration / Easing / reduced-motion ---

    #[test]
    fn duration_ms_values_match_m3_spec() {
        assert_eq!(Duration::Short1.ms(), 50);
        assert_eq!(Duration::Short2.ms(), 100);
        assert_eq!(Duration::Short3.ms(), 150);
        assert_eq!(Duration::Short4.ms(), 200);
        assert_eq!(Duration::Medium1.ms(), 250);
        assert_eq!(Duration::Medium2.ms(), 300);
        assert_eq!(Duration::Medium3.ms(), 350);
        assert_eq!(Duration::Medium4.ms(), 400);
        assert_eq!(Duration::Long1.ms(), 450);
        assert_eq!(Duration::Long2.ms(), 500);
        assert_eq!(Duration::Long3.ms(), 550);
        assert_eq!(Duration::Long4.ms(), 600);
    }

    #[test]
    fn duration_sec_conversion() {
        assert!((Duration::Short2.sec() - 0.1).abs() < 1e-6);
        assert!((Duration::Long4.sec() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn easing_bezier_control_points() {
        assert_eq!(Easing::Standard.bezier(), [0.2, 0.0, 0.0, 1.0]);
        assert_eq!(Easing::StandardDecelerate.bezier(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(Easing::StandardAccelerate.bezier(), [0.3, 0.0, 1.0, 1.0]);
        assert_eq!(Easing::Emphasized.bezier(), [0.2, 0.0, 0.0, 1.0]);
        assert_eq!(Easing::EmphasizedDecelerate.bezier(), [0.05, 0.7, 0.1, 1.0]);
        assert_eq!(Easing::EmphasizedAccelerate.bezier(), [0.3, 0.0, 0.8, 0.15]);
    }

    #[test]
    fn effective_duration_respects_reduced_motion() {
        // default — reduced off
        set_reduced_motion_override(false);
        assert_eq!(effective_duration(Duration::Short2), 100);
        // reduced on — 0 (мгновенно)
        set_reduced_motion_override(true);
        assert_eq!(effective_duration(Duration::Long4), 0);
        // restore
        set_reduced_motion_override(false);
    }

    #[test]
    fn effective_duration_with_explicit_flag() {
        assert_eq!(effective_duration_with(Duration::Short2, false), 100);
        assert_eq!(effective_duration_with(Duration::Short2, true), 0);
        assert_eq!(effective_duration_with(Duration::Long4, true), 0);
    }

    #[test]
    fn ease_endpoints_are_0_and_1() {
        // At t=0 → 0, t=1 → 1 for all easings
        for e in [
            Easing::Standard,
            Easing::StandardDecelerate,
            Easing::StandardAccelerate,
            Easing::Emphasized,
            Easing::EmphasizedDecelerate,
            Easing::EmphasizedAccelerate,
        ] {
            assert!(ease(0.0, e).abs() < 1e-6, "ease(0) != 0 for {:?}", e);
            assert!(
                (ease(1.0, e) - 1.0).abs() < 1e-6,
                "ease(1) != 1 for {:?}",
                e
            );
        }
    }

    #[test]
    fn ease_monotonic_for_standard() {
        // Standard easing should be monotonically increasing
        let mut prev = -1.0;
        let steps = 20;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let v = ease(t, Easing::Standard);
            assert!(
                v >= prev - 1e-6,
                "not monotonic at t={}: {} < {}",
                t,
                v,
                prev
            );
            prev = v;
        }
    }
}
