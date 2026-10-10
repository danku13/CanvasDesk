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
//! **Disabled > Pressed > Hovered > Focused > Dragged > Selected > Normal > Error**.
//!
//! Wave T (2026-10, ui-kit-deep-review §5.1.1-5.1.2): состояния **additive**
//! — [`WidgetState::active_states`] возвращает все активные одновременно
//! (hover+focused+selected могут стекаться). Deprecated [`WidgetState::kit_state`]
//! возвращает «highest-priority» единственное состояние для backwards-compat.
//!
//! Wave T §5.1.2: `key-focus` ≠ mouse-focus — [`WidgetState::set_focused`]
//! принимает `visible` флаг: keyboard-origin (Tab) → `focus_visible=true`
//! (рисует focus-ring); mouse-click → `focus_visible=false` (не рисует).
//! Семантика CSS `:focus-visible`.
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
    /// Keyboard-originated focus (`:focus-visible` семантика, Wave T §5.1.2).
    /// true — фокус пришёл через Tab/Shift+Tab (рисуем focus-ring);
    /// false — фокус пришёл через mouse-click (НЕ рисуем ring).
    focus_visible: bool,
    /// Drag-операция в процессе (Wave T §5.1.1). Set by consumer during drag.
    dragged: bool,
    /// Error-состояние (невалидное значение, Wave T §5.1.1).
    error: bool,
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

    /// Фокус (Tab-навигация; на `KitState` влияет через Focused state-layer).
    ///
    /// Wave T §5.1.2: `visible` — keyboard-origin (true) vs mouse-origin (false).
    /// Только keyboard-origin рисует focus-ring (см. [`Self::focus_ring_visible`]).
    /// Семантика CSS `:focus-visible`.
    pub fn set_focused(&mut self, v: bool) {
        self.focused = v;
        // backwards-compat: без visible-флага — считаем keyboard (как раньше).
        self.focus_visible = v;
    }

    /// Wave T §5.1.2: установить фокус с явным `visible` флагом.
    /// `visible=true` — keyboard-origin (Tab), рисует focus-ring.
    /// `visible=false` — mouse-origin, НЕ рисует ring (`:focus-visible`).
    pub fn set_focused_visible(&mut self, focused: bool, visible: bool) {
        self.focused = focused;
        self.focus_visible = focused && visible;
    }

    /// Wave T §5.1.1: drag-операция в процессе.
    pub fn set_dragged(&mut self, v: bool) {
        self.dragged = v;
    }

    /// Wave T §5.1.1: error-состояние (невалидное значение).
    pub fn set_error(&mut self, v: bool) {
        self.error = v;
    }

    /// Состояние кита по матрице приоритетов (soft-deprecated — Wave T §5.1.1).
    ///
    /// **Soft-deprecated:** возвращает единственное «highest-priority»
    /// состояние; новые потребители должны использовать [`Self::active_states`]
    /// для additive-состояний (hover+focused+selected могут стекаться).
    ///
    /// Приоритет: Disabled > Error > Pressed > Hovered > Focused > Dragged
    /// > Selected > Normal.
    ///
    /// Атрибут `#[deprecated]` НЕ ставится (сознательно, fix main после
    /// af5ddb3/Wave T): 40+ существующих потребителей (`button_style` и др.
    /// принимают одиночный `KitState`) — жёсткая депрекация роняет гейт
    /// `clippy -D warnings` всего воркспейса. Атрибут вернёт Wave C/A вместе
    /// с миграцией потребителей на additive-состояния.
    pub fn kit_state(&self) -> KitState {
        if self.disabled {
            KitState::Disabled
        } else if self.error {
            KitState::Error
        } else if self.pressed {
            KitState::Pressed
        } else if self.inside {
            KitState::Hovered
        } else if self.focused {
            KitState::Focused
        } else if self.dragged {
            KitState::Dragged
        } else if self.selected {
            KitState::Selected
        } else {
            KitState::Normal
        }
    }

    /// Wave T §5.1.1: все активные состояния (additive — могут стекаться).
    ///
    /// Возвращает до 4 состояний одновременно: например `[Hovered, Focused,
    /// Selected]` если курсор над выбранным сфокусированным виджетом.
    /// Disabled приоритетен — если disabled, другие не возвращаются.
    pub fn active_states(&self) -> [Option<KitState>; 4] {
        if self.disabled {
            return [Some(KitState::Disabled), None, None, None];
        }
        let mut states: [Option<KitState>; 4] = [None, None, None, None];
        let mut i = 0;
        if self.error {
            states[i] = Some(KitState::Error);
            i += 1;
        }
        if self.pressed {
            states[i] = Some(KitState::Pressed);
            i += 1;
        }
        if self.inside {
            states[i] = Some(KitState::Hovered);
            i += 1;
        }
        if self.focused {
            states[i] = Some(KitState::Focused);
        }
        states
    }

    /// Виджет в фокусе (флаг для backwards-compat).
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Wave T §5.1.2: рисовать ли focus-ring (только keyboard-origin focus).
    /// `:focus-visible` семантика — mouse-click не активирует.
    pub fn focus_ring_visible(&self) -> bool {
        self.focus_visible && !self.disabled
    }

    /// Wave T §5.1.1: drag в процессе.
    pub fn is_dragged(&self) -> bool {
        self.dragged
    }

    /// Wave T §5.1.1: error-состояние.
    pub fn is_error(&self) -> bool {
        self.error
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
        assert!(!WidgetState::default().focus_ring_visible());
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
    fn focus_participates_in_kit_state_after_wave_t() {
        let mut w = WidgetState::default();
        w.set_focused(true);
        assert!(w.is_focused());
        // Wave T §5.1.1: фокус участвует в матрице приоритетов kit_state
        // (прежде «фокус — только рамка потребителя»; старый контракт теста
        // обновлён вместе с Wave T — fix main).
        assert_eq!(w.kit_state(), KitState::Focused);
        // фокус + hover: hover сильнее фокуса (матрица приоритетов)
        w.set_pointer(true, false);
        assert_eq!(w.kit_state(), KitState::Hovered);
        assert!(w.is_focused());
        w.set_focused(false);
        assert!(!w.is_focused());
    }

    // --- Wave T §5.1.1: Focused/Dragged/Error states ---

    #[test]
    fn focused_state_in_kit_state() {
        let mut w = WidgetState::default();
        w.set_focused(true);
        // set_focused (backwards-compat) устанавливает focus_visible=true
        assert!(w.focus_ring_visible());
        // kit_state (deprecated) возвращает Focused
        assert_eq!(w.kit_state(), KitState::Focused);
    }

    #[test]
    fn dragged_state() {
        let mut w = WidgetState::default();
        w.set_dragged(true);
        assert!(w.is_dragged());
        assert_eq!(w.kit_state(), KitState::Dragged);
    }

    #[test]
    fn error_state() {
        let mut w = WidgetState::default();
        w.set_error(true);
        assert!(w.is_error());
        assert_eq!(w.kit_state(), KitState::Error);
    }

    #[test]
    fn disabled_beats_error() {
        let mut w = WidgetState::default();
        w.set_error(true);
        w.set_disabled(true);
        assert_eq!(w.kit_state(), KitState::Disabled);
    }

    // --- Wave T §5.1.1: active_states (additive) ---

    #[test]
    fn active_states_default_empty() {
        let w = WidgetState::default();
        let states = w.active_states();
        assert!(states.iter().all(|s| s.is_none()));
    }

    #[test]
    fn active_states_hover_and_focused_stack() {
        let mut w = WidgetState::default();
        w.set_focused(true);
        w.set_pointer(true, false); // hover
        let states = w.active_states();
        assert!(states.contains(&Some(KitState::Hovered)));
        assert!(states.contains(&Some(KitState::Focused)));
    }

    #[test]
    fn active_states_disabled_is_exclusive() {
        let mut w = WidgetState::default();
        w.set_focused(true);
        w.set_pointer(true, true); // hover + press
        w.set_selected(true);
        w.set_disabled(true);
        let states = w.active_states();
        assert_eq!(states[0], Some(KitState::Disabled));
        assert!(states[1].is_none());
    }

    // --- Wave T §5.1.2: key-focus ≠ mouse-focus ---

    #[test]
    fn key_focus_visible_true_for_keyboard() {
        let mut w = WidgetState::default();
        w.set_focused_visible(true, true); // keyboard-origin
        assert!(w.is_focused());
        assert!(w.focus_ring_visible()); // ring виден
    }

    #[test]
    fn key_focus_visible_false_for_mouse() {
        let mut w = WidgetState::default();
        w.set_focused_visible(true, false); // mouse-origin
        assert!(w.is_focused());
        assert!(!w.focus_ring_visible()); // ring НЕ виден (:focus-visible)
    }

    #[test]
    fn key_focus_visible_false_when_not_focused() {
        let mut w = WidgetState::default();
        w.set_focused_visible(false, true);
        assert!(!w.is_focused());
        assert!(!w.focus_ring_visible());
    }

    #[test]
    fn key_focus_visible_false_when_disabled() {
        let mut w = WidgetState::default();
        w.set_focused_visible(true, true);
        w.set_disabled(true);
        assert!(!w.focus_ring_visible()); // disabled — ring не рисуем
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
        // курсор ещё над виджетом — hover сильнее фокуса (матрица приоритетов)
        assert_eq!(w.kit_state(), KitState::Hovered);
        assert!(w.is_focused());
        w.set_pointer(false, false);
        // Wave T §5.1.1: курсор ушёл — Focused сильнее Selected
        assert_eq!(w.kit_state(), KitState::Focused);
    }
}
