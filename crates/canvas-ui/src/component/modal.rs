//! FR-068 W3: модальная панель + focus_order — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-b): `Props` + `impl Component` для modal добавлены
//! (секция «Component» ниже); существующие функции — стабильный API.

use super::{Component, ComponentHit, KitPalette, PanelStyle};
use crate::component::panel::{panel_rect, panel_style};
use crate::geometry::{UiPoint, UiRect, UiVec2};

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

// === FR-068 W3: Component (агент 3-b) =======================================
//
// [`Modal`] — retained-обёртка над kit-функцией [`modal`]. Неинтерактивный
// компонент: [`WidgetState`](crate::widget::WidgetState) НЕ нужен — модальность
// и полупрозрачное затемнение — ответственность слоя Modals/Block реестра
// (кит не ведёт ввод и не рисует поверх сцены: F-8 PRD-0009).

/// Свойства modal-компонента (декларативный вход кадра).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModalProps {
    /// Инвариантный минимум панели (приоритетен — parity FR-060).
    pub min: UiVec2,
    /// Максимум панели (constrain).
    pub max: UiVec2,
    /// Желаемый размер (зажимается [min, max]).
    pub desired: UiVec2,
    /// Палитра-срез: панель модали — та же хром-поверхность, что панель
    /// (слоты `panel_fill`/`panel_border`, радиус RADIUS_PANEL — [`modal_style`]).
    pub palette: KitPalette,
}

/// Modal-компонент (FR-068 W3): неинтерактивный — только Props (состояние
/// ввода не нужно: модальность — слой Modals реестра; backdrop-клик —
/// решение потребителя по [`Component::hit_test`] index 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modal {
    /// Свойства кадра.
    pub props: ModalProps,
}

impl Component for Modal {
    type Props = ModalProps;

    fn props(&self) -> &Self::Props {
        &self.props
    }

    /// Контракт индексов (ДОКУМЕНТИРОВАН, тесты фиксируют): `rects[0]` —
    /// затемнение (= весь слот), `rects[1]` — панель (constrain+stack,
    /// центр). Порядок = draw-порядок слоёв: dim ниже панели. Native-путь —
    /// default-семантика (parity
    /// зафиксирован там же); backend вёрсткой модали не пользуется
    /// (rect'ы — [`modal`]/[`panel_rect`]).
    fn layout(&self, _backend: &dyn crate::layout::LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        let m = modal(slot, self.props.min, self.props.max, self.props.desired);
        vec![m.dim, m.panel]
    }

    /// Рисует ТОЛЬКО панель (`rects[1]`) — хром панели из слотов палитры
    /// ([`modal_style`]). Затемнение (`rects[0]`) НЕ рисуется:
    /// полупрозрачное перекрытие сцены — ответственность потребителя/слоя
    /// Modals (кит не изобретает цветов: слота dim-заливки в палитре нет,
    /// альфа-арифметика над слотами запрещена — контракт F-8).
    fn paint(&self, painter: &mut crate::paint::Painter, rects: &[UiRect]) {
        if let Some(panel) = rects.get(1) {
            painter.panel(*panel, &modal_style(&self.props.palette));
        }
    }

    /// Переопределён: панель лежит ПОВЕРХ затемнения в том же слоте, поэтому
    /// дефолтный [`ComponentHit::pick`] для точки в панели вернул бы dim —
    /// сначала проверяется панель: точка в ней — index 1; точка в dim мимо
    /// панели — index 0 (backdrop: закрытие модали — забота потребителя);
    /// вне слота — None. Ожидается порядок rects из [`Component::layout`]
    /// (`[dim, panel]`, ≥ 2 элементов — иначе None).
    fn hit_test(&self, rects: &[UiRect], point: UiPoint) -> Option<ComponentHit> {
        let dim = rects.first()?;
        let panel = rects.get(1)?;
        if panel.contains(point) {
            Some(ComponentHit { index: 1 })
        } else if dim.contains(point) {
            Some(ComponentHit { index: 0 })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{palette_a, palette_b};
    use crate::keyboard::FocusRing;
    use crate::paint::{PaintItem, Painter};

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

    // === FR-068 W3: Component (Modal) =======================================

    /// Component::layout — ровно `[dim, panel]` В ЭТОМ ПОРЯДКЕ
    /// (документированный контракт индексов: rects[0] — затемнение = слот,
    /// rects[1] — панель) и паритет с kit-функцией [`modal`] 1:1.
    #[test]
    fn modal_component_layout_returns_dim_then_panel() {
        let slot = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let m = Modal {
            props: ModalProps {
                min: UiVec2::new(320.0, 240.0),
                max: UiVec2::new(640.0, 480.0),
                desired: UiVec2::new(200.0, 200.0),
                palette: palette_a(),
            },
        };
        let rects = m.layout(crate::layout::default_backend(), slot);
        let kit = modal(slot, m.props.min, m.props.max, m.props.desired);
        assert_eq!(rects.len(), 2, "modal — два rect'а: dim + panel");
        assert_eq!(rects[0], kit.dim, "rects[0] — затемнение (= слот)");
        assert_eq!(rects[1], kit.panel, "rects[1] — панель (constrain+stack)");
        assert_eq!(rects[0], slot);
        assert_eq!(*m.props(), m.props, "props() — доступ к свойствам");
    }

    /// Component::paint — рисует ТОЛЬКО панель (rects[1]) хромой панели из
    /// СЛОТОВ палитры (panel_fill/panel_border, радиус RADIUS_PANEL — шкала
    /// токенов); затемнение НЕ рисуется (полупрозрачное перекрытие —
    /// ответственность потребителя/слоя Modals; смена палитры меняет item —
    /// контракт F-8).
    #[test]
    fn modal_component_paint_draws_panel_only() {
        let slot = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        for palette in [palette_a(), palette_b()] {
            let m = Modal {
                props: ModalProps {
                    min: UiVec2::new(320.0, 240.0),
                    max: UiVec2::new(640.0, 480.0),
                    desired: UiVec2::new(900.0, 500.0),
                    palette,
                },
            };
            let rects = m.layout(crate::layout::default_backend(), slot);
            let mut painter = Painter::new();
            m.paint(&mut painter, &rects);
            let items = painter.items();
            assert_eq!(items.len(), 1, "затемнение НЕ рисуется — только панель");
            assert_eq!(
                items[0],
                PaintItem::Rect {
                    rect: rects[1],
                    fill: palette.panel_fill,
                    border: palette.panel_border,
                    radius: canvas_core::tokens::RADIUS_PANEL,
                }
            );
        }
    }

    /// Component::hit_test (переопределён): панель ПОВЕРХ затемнения —
    /// точка в панели → index 1 (дефолтный pick вернул бы dim/index 0);
    /// точка в dim мимо панели → index 0 (backdrop); вне слота → None.
    #[test]
    fn modal_component_hit_test_panel_beats_dim() {
        let slot = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let m = Modal {
            props: ModalProps {
                min: UiVec2::new(320.0, 240.0),
                max: UiVec2::new(640.0, 480.0),
                desired: UiVec2::new(640.0, 480.0),
                palette: palette_a(),
            },
        };
        let rects = m.layout(crate::layout::default_backend(), slot);
        // Панель 640×480 по центру слота: (320..960, 160..640)
        assert_eq!(rects[1], UiRect::new(320.0, 160.0, 640.0, 480.0));
        assert_eq!(
            m.hit_test(&rects, UiPoint::new(321.0, 161.0)),
            Some(ComponentHit { index: 1 }),
            "точка в панели — index 1 (не дефолтный pick → dim)"
        );
        assert_eq!(
            m.hit_test(&rects, UiPoint::new(319.0, 160.0)),
            Some(ComponentHit { index: 0 }),
            "точка в dim мимо панели (левый край) — index 0 (backdrop)"
        );
        assert_eq!(
            m.hit_test(&rects, UiPoint::new(10.0, 10.0)),
            Some(ComponentHit { index: 0 })
        );
        assert_eq!(m.hit_test(&rects, UiPoint::new(2000.0, 2000.0)), None);
        // Некорректный rects (потребитель не вызвал layout) — None, не паника.
        assert_eq!(m.hit_test(&[], UiPoint::new(10.0, 10.0)), None);
    }

    // === FR-062 F-18: геометрический снапшот кит-компонента (без шрифтов) ===
}
