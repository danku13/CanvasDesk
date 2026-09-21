//! FR-051 U0 (PRD-0009 F-1/F-3, §7.3): capture-политики ввода — формализация
//! нынешнего поведения цепочки `on_left_button` (app.rs:8923–9510) правилом.
//!
//! Референсы подходов (анализ 2026-09-22, таксономия — не зависимость):
//! egui Order/backdrop семантика модалей, Ribir `IgnorePointer` (аналог
//! `PassThrough`).

/// Политика перехвата ввода поверхностью.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CapturePolicy {
    /// Модальная: глотает весь ввод под собой (онбординг, галерея, настройки,
    /// диалог, main stage, defense). Клик мимо своих rect'ов — backdrop.
    Block,
    /// Перехватывает клик в своих rect'ах; мимо — пропускает ниже
    /// (док палитры, хоткеи, what-if бар).
    Capture,
    /// Не перехватывает ввод, только рисуется (empty-state).
    PassThrough,
    /// Не интерактивна и hit-rect'ов не объявляет (тосты).
    Passive,
}

impl CapturePolicy {
    /// «Глотает ли клик мимо своих rect'ов» — семантика backdrop.
    pub const fn intercept_outside(self) -> bool {
        matches!(self, CapturePolicy::Block)
    }

    /// Объявляет ли поверхность перехватывающие hit-rect'ы.
    /// `Passive` не объявляет вовсе (контракт — тест в `frame.rs`).
    pub const fn is_interactive(self) -> bool {
        !matches!(self, CapturePolicy::PassThrough | CapturePolicy::Passive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_intercepts_everything() {
        assert!(CapturePolicy::Block.intercept_outside());
        assert!(CapturePolicy::Block.is_interactive());
    }

    #[test]
    fn capture_intercepts_only_own_rects() {
        assert!(!CapturePolicy::Capture.intercept_outside());
        assert!(CapturePolicy::Capture.is_interactive());
    }

    #[test]
    fn pass_through_and_passive_never_intercept() {
        for p in [CapturePolicy::PassThrough, CapturePolicy::Passive] {
            assert!(!p.intercept_outside());
            assert!(!p.is_interactive());
        }
    }
}
