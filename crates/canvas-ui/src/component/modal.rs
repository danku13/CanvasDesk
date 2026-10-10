//! FR-068 W3: модальная панель + focus_order — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-b): `Props` + `impl Component` для modal добавлены
//! (секция «Component» ниже); существующие функции — стабильный API.

use super::{Component, ComponentHit, KitPalette, PanelStyle};
use crate::component::panel::{panel_rect, panel_style};
use crate::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use crate::layout::{constrain, stack, HAlign, VAlign};

// --- Modal ------------------------------------------------------------------

// === Wave M (issue #50, M1): адаптивное окно модали 80% =====================

/// Тир раскладки по ширине вьюпорта (R-A R2 — именованные диапазоны, не
/// устройства): Compact ≤767 — mobile-style (одноколоночная вёрстка,
/// full-bleed лист); Medium 768–1279; Expanded ≥1280. Границы совпадают с
/// `SETTINGS_BP_MOBILE = 768` / `SETTINGS_BP_COMPACT = 1280`
/// (canvas-app/ui_registry.rs) — settings уже живёт на этой лестнице.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutTier {
    /// ≤767 — mobile-style: full-bleed, thumb zone, таргеты ≥44×44 (T-M R1).
    Compact,
    /// 768–1279 — 80% слота, min-габариты контракта, скролл внутри (M-O R7).
    Medium,
    /// ≥1280 — 80% слота, max-ограничения, канонические пропорции.
    Expanded,
}

/// Тир по ширине вьюпорта. Некорректная/нулевая ширина — Compact (самая
/// консервативная ветка: full-bleed не может выйти за слот).
pub fn layout_tier(width: f32) -> LayoutTier {
    if width < 768.0 {
        LayoutTier::Compact
    } else if width < 1280.0 {
        LayoutTier::Medium
    } else {
        LayoutTier::Expanded
    }
}

/// Доля слота, занимаемая модалью в Medium/Expanded (Wave M: «модали — 80%»).
pub const MODAL_RATIO_DEFAULT: f32 = 0.8;

/// Адаптивное окно модали Wave M (issue #50): `eff = slot − margin` (uniform);
/// в Compact — full-bleed `eff` (mobile-style: окно занимает весь слот с
/// safe-margin, ветвление тиров инкапсулировано — потребители не дублируют);
/// иначе `size = constrain(min, max, ratio × eff)`, центр `eff`.
///
/// Семантика min-инварианта — как у [`modal`]/[`panel_rect`] (parity FR-060):
/// при `min > eff` панель сохраняет размер min и может выступать за `eff`
/// (центр `eff`, без клиппа — деградация документирована; «окно в пределах
/// вьюпорта» на гейте G4, выбор min, помещающихся в целевые вьюпорты, —
/// ответственность переводимой модали в M3). Финальный viewport-кламп НЕ
/// выполняется сознательно — пересечение клипповало бы инвариантную панель.
pub fn modal_rect_flex(slot: UiRect, margin: f32, ratio: f32, min: UiVec2, max: UiVec2) -> UiRect {
    let eff = slot.inset(&EdgeInsets::uniform(margin));
    if layout_tier(slot.w) == LayoutTier::Compact {
        return eff;
    }
    let desired = UiVec2::new(eff.w * ratio, eff.h * ratio);
    let size = constrain(min, max, desired);
    stack(eff, size, HAlign::Center, VAlign::Center)
}

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

    // === Wave M (issue #50, M1): layout_tier / modal_rect_flex ===============

    /// Сравнение rect'ов с допуском (умножения на 0.8 не точны в f32).
    fn assert_rect_close(a: UiRect, b: UiRect) {
        for (x, y, name) in [
            (a.x, b.x, "x"),
            (a.y, b.y, "y"),
            (a.w, b.w, "w"),
            (a.h, b.h, "h"),
        ] {
            assert!(
                (x - y).abs() < 0.01,
                "{name}: {x} != {y} (a={a:?}, b={b:?})"
            );
        }
    }

    /// Границы тиров: <768 Compact, 768–1279.99… Medium, ≥1280 Expanded;
    /// нулевая/отрицательная ширина — Compact (консервативная ветка).
    #[test]
    fn layout_tier_boundaries_are_named_ranges() {
        assert_eq!(layout_tier(375.0), LayoutTier::Compact);
        assert_eq!(layout_tier(767.9), LayoutTier::Compact);
        assert_eq!(layout_tier(0.0), LayoutTier::Compact);
        assert_eq!(layout_tier(-10.0), LayoutTier::Compact);
        assert_eq!(layout_tier(768.0), LayoutTier::Medium);
        assert_eq!(layout_tier(1279.9), LayoutTier::Medium);
        assert_eq!(layout_tier(1024.0), LayoutTier::Medium);
        assert_eq!(layout_tier(1280.0), LayoutTier::Expanded);
        assert_eq!(layout_tier(1920.0), LayoutTier::Expanded);
    }

    /// Expanded (1920×1080, margin 24): желаемое 0.8×eff = 1497.6×825.6
    /// срезается max 900×640, панель по центру eff.
    #[test]
    fn modal_rect_flex_expanded_clamps_to_max_and_centers() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 1920.0, 1080.0),
            24.0,
            MODAL_RATIO_DEFAULT,
            UiVec2::new(560.0, 420.0),
            UiVec2::new(900.0, 640.0),
        );
        assert_rect_close(r, UiRect::new(510.0, 220.0, 900.0, 640.0));
    }

    /// Medium (1024×640, margin 24): желаемое 0.8×eff = 780.8×473.6 внутри
    /// границ — проходит без клампа, центр eff.
    #[test]
    fn modal_rect_flex_medium_applies_ratio_when_within_bounds() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 1024.0, 640.0),
            24.0,
            MODAL_RATIO_DEFAULT,
            UiVec2::new(560.0, 400.0),
            UiVec2::new(900.0, 640.0),
        );
        assert_rect_close(r, UiRect::new(121.6, 83.2, 780.8, 473.6));
    }

    /// Compact (375×667, margin 12): full-bleed eff — min/max игнорируются
    /// (даже противоречивые), ветвление тиров инкапсулировано в примитиве.
    #[test]
    fn modal_rect_flex_compact_returns_full_bleed() {
        for (min, max) in [
            (UiVec2::new(560.0, 420.0), UiVec2::new(900.0, 640.0)),
            (UiVec2::new(2000.0, 2000.0), UiVec2::new(10.0, 10.0)),
        ] {
            let r = modal_rect_flex(
                UiRect::new(0.0, 0.0, 375.0, 667.0),
                12.0,
                MODAL_RATIO_DEFAULT,
                min,
                max,
            );
            assert_rect_close(r, UiRect::new(12.0, 12.0, 351.0, 643.0));
        }
    }

    /// Граница тира: 768 — уже Medium (не Compact): ratio применяется.
    #[test]
    fn modal_rect_flex_at_768_is_medium_not_compact() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 768.0, 600.0),
            12.0,
            MODAL_RATIO_DEFAULT,
            UiVec2::new(320.0, 240.0),
            UiVec2::new(760.0, 760.0),
        );
        // eff = 744×576; 0.8 → 595.2×460.8; центр eff.
        assert_rect_close(r, UiRect::new(86.4, 69.6, 595.2, 460.8));
    }

    /// Min-инвариант (parity FR-060): min 760×500 > 0.8×eff при 800×560 —
    /// панель СОХРАНЯЕТ min, НЕ клиппуется до желаемого (деградация
    /// документирована; здесь 24+760=784 ≤ 800 — в вьюпорте помещается).
    #[test]
    fn modal_rect_flex_min_invariant_wins_over_ratio() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 800.0, 560.0),
            24.0,
            MODAL_RATIO_DEFAULT,
            UiVec2::new(760.0, 500.0),
            UiVec2::new(1120.0, 720.0),
        );
        assert_rect_close(r, UiRect::new(24.0, 30.0, 760.0, 500.0));
    }

    /// Противоречивые границы min > max — нижняя приоритетна (constrain):
    /// размер = min, центр eff (24 + (976−760)/2, 24 + (592−500)/2).
    #[test]
    fn modal_rect_flex_min_above_max_yields_min() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 1024.0, 640.0),
            24.0,
            MODAL_RATIO_DEFAULT,
            UiVec2::new(760.0, 500.0),
            UiVec2::new(400.0, 300.0),
        );
        assert_rect_close(r, UiRect::new(132.0, 70.0, 760.0, 500.0));
    }

    /// ratio = 1.0 — модаль занимает весь eff (проверка проницаемости ratio).
    #[test]
    fn modal_rect_flex_respects_ratio_parameter() {
        let r = modal_rect_flex(
            UiRect::new(0.0, 0.0, 1024.0, 640.0),
            24.0,
            1.0,
            UiVec2::new(320.0, 240.0),
            UiVec2::new(2000.0, 2000.0),
        );
        assert_rect_close(r, UiRect::new(24.0, 24.0, 976.0, 592.0));
    }
}
