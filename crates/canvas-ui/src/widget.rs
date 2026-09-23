//! FR-057 (волна 2 кита, PRD-0009 §9.2): [`WidgetState`] — машина состояний
//! виджета кита. Переводит переходы указателя/селекции/фокуса в
//! [`KitState`] (слоты состояний палитры FR-053) и ведёт ребро клика
//! «press был внутри → release внутри».
//!
//! До FR-057 каждый потребитель отслеживал зажатие и клик вручную
//! (`cursor_state` kit_ui.rs:378–386 знал только hover/disabled); теперь
//! логика одна — миграции FR-059/FR-060 её переиспользуют вместо дублей.
//!
//! Приоритет состояний (детерминированная матрица):
//! **Disabled > Pressed > Hovered > Selected > Normal**.
//! Фокус в `KitState` не входит — рисуется отдельно (фокус-рамка по слоту
//! `accent`); [`WidgetState::is_focused`] — флаг потребителю.
//!
//! Ребро клика: `set_pointer(inside=true, pressed_now=true)` заряжает press
//! («press был внутри»), [`WidgetState::clicked`] на release возвращает
//! `true` ровно один раз, если release произошёл внутри. Press на
//! disabled-виджете не заряжается (клик игнорируется — контракт
//! `KitState::Disabled`); press вне виджета кликом не считается.

use crate::kit::KitState;

/// Состояние интерактивного виджета (поля приватные — переходы только
/// через `set_*`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WidgetState {
    /// Курсор над виджетом.
    inside: bool,
    /// Кнопка зажата над виджетом (inside && down).
    pressed: bool,
    /// Выбран (тоглы, строки списков, чипы фильтров).
    selected: bool,
    /// Недоступен.
    disabled: bool,
    /// В фокусе (Tab-навигация `keyboard::FocusRing` — рисует потребитель).
    focused: bool,
    /// Press начался внутри (до release/`clicked`) — ребро клика.
    press_armed: bool,
}

impl WidgetState {
    /// Переход указателя: `inside` — курсор над виджетом, `pressed_now` —
    /// кнопка зажата в этом кадре. Press внутри заряжает ребро клика.
    pub fn set_pointer(&mut self, inside: bool, pressed_now: bool) {
        self.inside = inside;
        self.pressed = inside && pressed_now;
        if inside && pressed_now && !self.disabled {
            self.press_armed = true;
        }
    }

    /// Селекция (тогл/строка списка/чип фильтра).
    pub fn set_selected(&mut self, v: bool) {
        self.selected = v;
    }

    /// Недоступность (визуально сильнее pressed/hover; клик не засчитывается).
    pub fn set_disabled(&mut self, v: bool) {
        self.disabled = v;
    }

    /// Фокус (Tab-навигация; на `KitState` не влияет — рамка отдельная).
    pub fn set_focused(&mut self, v: bool) {
        self.focused = v;
    }

    /// Состояние кита по матрице приоритетов
    /// Disabled > Pressed > Hovered > Selected > Normal.
    pub fn kit_state(&self) -> KitState {
        if self.disabled {
            KitState::Disabled
        } else if self.pressed {
            KitState::Pressed
        } else if self.inside {
            KitState::Hovered
        } else if self.selected {
            KitState::Selected
        } else {
            KitState::Normal
        }
    }

    /// Виджет в фокусе (фокус-рамка по слоту `accent` — рисует потребитель).
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Ребро клика: press был внутри, release внутри. Вызывать на release
    /// (`released_now_inside` — release произошёл внутри виджета). Возвращает
    /// true ровно один раз на press→release (ребро гасится любым вызовом —
    /// в том числе release мимо виджета).
    pub fn clicked(&mut self, released_now_inside: bool) -> bool {
        let fired = self.press_armed && released_now_inside && !self.disabled;
        self.press_armed = false;
        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Матрица переходов (приоритет Disabled>Pressed>Hovered>Selected>Normal)

    #[test]
    fn default_is_normal() {
        assert_eq!(WidgetState::default().kit_state(), KitState::Normal);
        assert!(!WidgetState::default().is_focused());
    }

    #[test]
    fn hover_press_release_ladder() {
        let mut w = WidgetState::default();
        // курсор вошёл — hover
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Hovered);
        // зажатие — pressed
        w.set_pointer(true, true);
        assert_eq!(w.kit_state(), KitState::Pressed);
        // отпустили, курсор ещё над виджетом — снова hover
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Hovered);
        // курсор ушёл — normal
        w.set_pointer(false, false);
        assert_eq!(w.kit_state(), KitState::Normal);
    }

    #[test]
    fn press_outside_does_not_press() {
        let mut w = WidgetState::default();
        // кнопка зажата, но курсор не над виджетом — не pressed
        w.set_pointer(false, true);
        assert_eq!(w.kit_state(), KitState::Normal);
    }

    #[test]
    fn disabled_beats_pressed_and_hovered() {
        let mut w = WidgetState::default();
        w.set_disabled(true);
        w.set_pointer(true, true);
        assert_eq!(w.kit_state(), KitState::Disabled);
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Disabled);
    }

    #[test]
    fn hover_beats_selected_and_selected_persists() {
        let mut w = WidgetState::default();
        w.set_selected(true);
        assert_eq!(w.kit_state(), KitState::Selected);
        // курсор над выбранным — hover (приоритет), селекция сохраняется
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Hovered);
        w.set_pointer(false, false);
        assert_eq!(w.kit_state(), KitState::Selected);
    }

    #[test]
    fn disabled_beats_selected() {
        let mut w = WidgetState::default();
        w.set_selected(true);
        w.set_disabled(true);
        assert_eq!(w.kit_state(), KitState::Disabled);
    }

    #[test]
    fn focus_is_separate_from_kit_state() {
        let mut w = WidgetState::default();
        w.set_focused(true);
        assert!(w.is_focused());
        // фокус не меняет KitState (рамка — отдельная отрисовка потребителя)
        assert_eq!(w.kit_state(), KitState::Normal);
        // фокус + hover: KitState по матрице, флаг фокуса не сбрасывается
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Hovered);
        assert!(w.is_focused());
        w.set_focused(false);
        assert!(!w.is_focused());
    }

    // --- Ребро клика (press→release внутри/снаружи)

    #[test]
    fn click_edge_inside_inside_fires_once() {
        let mut w = WidgetState::default();
        w.set_pointer(true, true); // press внутри
        w.set_pointer(true, false); // release внутри
        assert!(w.clicked(true));
        // ребро одноразовое: повторный вызов без нового press — false
        assert!(!w.clicked(true));
    }

    #[test]
    fn click_edge_released_outside_does_not_fire() {
        let mut w = WidgetState::default();
        w.set_pointer(true, true); // press внутри
        w.set_pointer(false, false); // release снаружи
        assert!(!w.clicked(false));
        // ребро погашено и не «доживает» до следующего release
        w.set_pointer(true, true); // новый press внутри
        w.set_pointer(true, false);
        assert!(w.clicked(true));
    }

    #[test]
    fn click_edge_press_outside_release_inside_is_not_click() {
        let mut w = WidgetState::default();
        w.set_pointer(false, true); // press ВНЕ виджета
        w.set_pointer(true, false); // курсор над виджетом на release
        assert!(!w.clicked(true));
    }

    #[test]
    fn click_edge_drag_out_and_back_releases_inside() {
        // press внутри → утащили с зажатой кнопкой → отпустили внутри:
        // «press был внутри, release внутри» — клик (контракт FR-057)
        let mut w = WidgetState::default();
        w.set_pointer(true, true);
        w.set_pointer(false, true); // курсор ушёл при зажатой кнопке
        assert_eq!(w.kit_state(), KitState::Normal); // press вне — не подсвечен
        w.set_pointer(true, false); // вернулись и отпустили
        assert!(w.clicked(true));
    }

    #[test]
    fn disabled_widget_does_not_click() {
        let mut w = WidgetState::default();
        w.set_disabled(true);
        w.set_pointer(true, true); // press по disabled — не заряжается
        w.set_pointer(true, false);
        assert!(!w.clicked(true));
    }

    /// Селекция/фокус не мешают ребру клика (тогл: клик и селекция вместе).
    #[test]
    fn click_edge_works_with_selected_and_focused() {
        let mut w = WidgetState::default();
        w.set_selected(true);
        w.set_focused(true);
        w.set_pointer(true, true);
        w.set_pointer(true, false);
        assert!(w.clicked(true));
        // курсор ещё над виджетом — hover сильнее selected (матрица приоритетов)
        assert_eq!(w.kit_state(), KitState::Hovered);
        assert!(w.is_focused());
        w.set_pointer(false, false);
        assert_eq!(w.kit_state(), KitState::Selected);
    }
}
