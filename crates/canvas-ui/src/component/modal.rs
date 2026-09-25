//! FR-068 W3: модальная панель + focus_order (+ taffy-путь) — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-b): дополнить `Props` + `impl Component` для
//! modal; существующие функции — стабильный API.

use super::{KitPalette, PanelStyle};
use crate::component::panel::{panel_rect, panel_style};
use crate::geometry::{UiRect, UiVec2};
#[cfg(feature = "taffy")]
use crate::layout::constrain;

#[cfg(feature = "taffy")]
use crate::layout::TaffyBackend;

// --- Modal ------------------------------------------------------------------

/// Раскладка модали: затемнение = весь слот, панель = constrain+stack
/// (центр). Слой/модальность — только из реестра (поверхность Modals/Block).
pub struct ModalLayout {
    pub dim: UiRect,
    pub panel: UiRect,
}

pub fn modal(slot: UiRect, min: UiVec2, max: UiVec2, desired: UiVec2) -> ModalLayout {
    // FR-068 W0: панель — constrain+stack БЕЗ финального viewport_clamp:
    // семантика «min-инвариант приоритетен» (parity FR-060) — при слоте
    // меньше инвариантного min панель прижимается к углу слота, СОХРАНЯЯ
    // размер min (documented деградация; parity-тесты canvas-app
    // `dialog_rect_kit_modal_matches_old_clamps` / `window_rect_...`).
    // Хелпер-пересечение НЕ эквивалентен: клипповал бы инвариантную панель.
    ModalLayout {
        dim: slot,
        panel: panel_rect(slot, min, max, desired),
    }
}

/// Стиль модали = стиль панели (тот же слот заливки/рамки).
pub fn modal_style(p: &KitPalette) -> PanelStyle {
    panel_style(p)
}

// --- Фокус контента (FR-062 F-17) -------------------------------------------

/// Текущий фокус [`FocusRing`] в Tab-порядке `rects` поверхности:
/// `Some((индекс, rect))` — кольцо указывает на rect из `rects`
/// (совпадение по значению — кольцо живёт в тех же координатах, что и
/// раскладка: контент-координаты + сдвиг скролла решает потребитель);
/// `None` — фокус не ставился или rect'ы перестроились (после
/// [`FocusRing::retain_order`] совпадение восстанавливается).
///
/// Потребитель даёт [`crate::widget::WidgetState::set_focused`] и рисует
/// рамку слотом `accent` (контракт FR-057: FocusRing — только навигация).
pub fn focus_order(rects: &[UiRect], ring: &crate::keyboard::FocusRing) -> Option<(usize, UiRect)> {
    let current = *ring.current()?;
    rects
        .iter()
        .position(|r| *r == current)
        .map(|i| (i, current))
}

#[cfg(feature = "taffy")]
const TAFFY: TaffyBackend = TaffyBackend;

/// Opt-in taffy-путь (FR-068 W1) [`modal`]: размер панели — тот же
/// [`constrain`] (min-инвариант приоритетен, parity FR-060); позиция панели —
/// `TaffyBackend::centered` (CSS justify/align Center) вместо
/// `stack(Center, Center)`. При помещении панели в слот — паритет с native
/// (тесты, побитово на целых входах). Переполнение Center (панель больше
/// слота, напр. min > слот) — ДОКУМЕНТИРОВАННОЕ расхождение (как у
/// `TaffyBackend::centered`): taffy сжимает ребёнка по ГЛАВНОЙ оси до слота
/// (flex_shrink 1 — CSS-семантика, родня расхождения C3) и центрирует по
/// поперечной с выходом за ОБА края; native [`stack`] клампит левый/верхний
/// край к слоту, СОХРАНЯЯ min-размер (parity FR-060). Из parity-матрицы
/// исключено, фиксируется отдельным тестом; pilot-поверхности держат
/// min ≤ слот через constrain (переполнение — деградация, ловимая G4).
/// Default-сборка использует native-функцию; финальные гарантии W0
/// сохранены: native-семантика БЕЗ [`viewport_clamp`] (min-инвариант —
/// пересечение клипповало бы инвариантную панель, см. [`modal`]).
#[cfg(feature = "taffy")]
pub fn modal_taffy(slot: UiRect, min: UiVec2, max: UiVec2, desired: UiVec2) -> ModalLayout {
    let size = constrain(min, max, desired);
    ModalLayout {
        dim: slot,
        panel: TAFFY.centered(slot, size),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::FocusRing;
    /// FR-068 W0: modal — панель БЕЗ финального [`viewport_clamp`] (семантика
    /// «min-инвариант приоритетен», parity FR-060): при слоте меньше
    /// инвариантного min панель прижимается к углу слота, СОХРАНЯЯ размер
    /// (parity-тесты canvas-app). Пересечение клипповало бы панель.
    #[test]
    fn modal_panel_min_invariant_not_clipped() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 100.0);
        let m = modal(
            slot,
            UiVec2::new(320.0, 240.0),
            UiVec2::new(640.0, 480.0),
            UiVec2::new(200.0, 200.0),
        );
        assert_eq!(m.panel, UiRect::new(0.0, 0.0, 320.0, 240.0));
        assert_eq!(m.dim, slot);
    }
    /// Текущий фокус кольца находится в Tab-порядке по значению rect'а.
    #[test]
    fn focus_order_finds_ring_current_in_tab_order() {
        let a = UiRect::new(0.0, 0.0, 40.0, 24.0);
        let b = UiRect::new(50.0, 0.0, 40.0, 24.0);
        let c = UiRect::new(100.0, 0.0, 40.0, 24.0);
        let mut ring = FocusRing::new();
        // пустое кольцо — None
        assert!(focus_order(&[a, b, c], &ring).is_none());
        ring.push(a);
        ring.push(b);
        ring.push(c);
        assert_eq!(ring.next(), Some(a));
        assert_eq!(focus_order(&[a, b, c], &ring), Some((0, a)));
        assert_eq!(ring.next(), Some(b));
        assert_eq!(focus_order(&[a, b, c], &ring), Some((1, b)));
        // rect'ы перестроились (нет совпадения по значению) — None;
        // retain_order сохраняет индекс — совпадение восстанавливается
        assert!(focus_order(&[a, c], &ring).is_none());
        ring.retain_order(&[a, c]);
        assert_eq!(focus_order(&[a, c], &ring), Some((1, c)));
    }

    // === FR-062 F-18: геометрический снапшот кит-компонента (без шрифтов) ===
}

#[cfg(all(test, feature = "taffy"))]
mod taffy_parity {
    use super::*;

    /// modal: parity с native [`modal`] — ПОБИТОВО при ПОМЕЩЕНИИ панели
    /// в слот (центрирование `TaffyBackend::centered` ≡
    /// `stack(Center, Center)` на целых входах): desired > max
    /// (сжатие constrain), desired < min (min-кламп), смещённый слот,
    /// нечётная разница (x.5 — точное f32). Переполненный Center
    /// (панель больше слота) ИСКЛЮЧЁН из матрицы — документированное
    /// расхождение (следующий тест).
    #[test]
    fn modal_taffy_parity_with_native() {
        let cases = [
            (
                UiRect::new(0.0, 0.0, 1280.0, 800.0),
                UiVec2::new(200.0, 100.0),
                UiVec2::new(600.0, 400.0),
                UiVec2::new(900.0, 500.0),
            ), // desired > max → 600×400
            (
                UiRect::new(0.0, 0.0, 1280.0, 800.0),
                UiVec2::new(280.0, 150.0),
                UiVec2::new(440.0, 150.0),
                UiVec2::new(440.0, 150.0),
            ), // впритык
            (
                UiRect::new(0.0, 0.0, 1280.0, 800.0),
                UiVec2::new(100.0, 80.0),
                UiVec2::new(600.0, 400.0),
                UiVec2::new(50.0, 40.0),
            ), // desired < min → min
            (
                UiRect::new(50.0, 40.0, 400.0, 300.0),
                UiVec2::new(100.0, 80.0),
                UiVec2::new(380.0, 280.0),
                UiVec2::new(900.0, 500.0),
            ), // смещённый слот, desired > max
            (
                UiRect::new(50.0, 40.0, 401.0, 301.0),
                UiVec2::new(100.0, 80.0),
                UiVec2::new(379.0, 279.0),
                UiVec2::new(379.0, 279.0),
            ), // нечётная разница: (401−379)/2 = 11
        ];
        for (slot, min, max, desired) in cases {
            let native = modal(slot, min, max, desired);
            let taffy = modal_taffy(slot, min, max, desired);
            assert_eq!(
                (taffy.panel.x, taffy.panel.y, taffy.panel.w, taffy.panel.h),
                (
                    native.panel.x,
                    native.panel.y,
                    native.panel.w,
                    native.panel.h
                ),
                "parity модали: slot={slot:?}"
            );
            assert_eq!(taffy.dim, native.dim, "dim == slot");
        }
    }
    /// Документированное расхождение (исключено из parity-матрицы):
    /// переполненный Center (панель больше слота, напр. min-инвариант
    /// при слоте меньше min) — taffy сжимает ребёнка по ГЛАВНОЙ оси до
    /// слота (flex_shrink 1 — CSS flex, родня C3) и центрирует по
    /// ПОПЕРЕЧНОЙ с выходом за ОБА края; native [`stack`] клампит
    /// левый/верхний край к слоту, СОХРАНЯЯ min-размер (parity FR-060).
    /// Тест ФИКСИРУЕТ поведение, чтобы расхождение было видимым.
    #[test]
    fn modal_taffy_center_overflow_is_documented_divergence() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 100.0);
        let (min, max, desired) = (
            UiVec2::new(320.0, 240.0),
            UiVec2::new(640.0, 480.0),
            UiVec2::new(200.0, 200.0),
        );
        let native = modal(slot, min, max, desired);
        let taffy = modal_taffy(slot, min, max, desired);
        // native: кламп левого/верхнего края к слоту, размер = min
        // (min-инвариант, parity FR-060)
        assert_eq!(native.panel, UiRect::new(0.0, 0.0, 320.0, 240.0));
        // taffy (CSS): главная ось — ребёнок СЖИМАЕТСЯ до слота
        // (flex_shrink 1 у `centered`), поперечная — центрируется с
        // выходом за ОБА края: x = 0 (ширина = слоту), y = (100−240)/2
        assert_eq!(
            (taffy.panel.x, taffy.panel.y, taffy.panel.w, taffy.panel.h),
            (0.0, -70.0, 100.0, 240.0),
            "CSS-переполнение: shrink по главной оси + unsafe Center по поперечной"
        );
        assert_ne!(native.panel, taffy.panel, "расхождение зафиксировано");
    }
}
