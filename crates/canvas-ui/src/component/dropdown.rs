//! FR-068 W3: dropdown/tooltip/toast — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-b): `Props` + `impl Component` для dropdown
//! добавлены (секция «Component» ниже)
//! W1 (семантика не менялась).

use super::{Component, KitPalette, DROPDOWN_GAP, TOOLTIP_OFFSET};
use crate::geometry::{UiPoint, UiRect, UiVec2};
use crate::widget::WidgetState;

// --- FR-068 W0: общий кламп rect'а к вьюпорту -------------------------------

/// FR-068 W0: общий хелпер клампа rect'а к вьюпорту — устраняет дублирующие
/// ручные clamp-выкладки popup-геометрии (`dropdown_menu`/`tooltip`/`modal`/
/// `toast_area`).
///
/// Контракт: непустое пересечение — возвращается `rect`, обрезанный до
/// вьюпорта (финальная гарантия «не выходим за вьюпорт»); пустое пересечение
/// (rect ЦЕЛИКОМ вне вьюпорта, включая вырожденный viewport с w/h ≤ 0) —
/// исходный `rect` возвращается КАК ЕСТЬ: хелпер не маскирует класс выхода
/// за вьюпорт, такой случай детектируется G4-линтом (`ui_layout_lint`).
/// Позиционный кламп с сохранением размера (тултипы) хелпером НЕ выражается —
/// см. [`tooltip`].
pub fn viewport_clamp(rect: UiRect, viewport: UiRect) -> UiRect {
    rect.intersection(&viewport).unwrap_or(rect)
}

// --- Dropdown («якорь + flip») ----------------------------------------------

/// Раскладка открытого dropdown-меню: под якорем, при нехватке места снизу —
/// НАД якорем (flip), иначе — прижато к низу вьюпорта; по горизонтали —
/// зажато во вьюпорт.
pub struct DropdownLayout {
    /// Rect меню.
    pub menu: UiRect,
    /// Меню открыто вверх (flip из-за нехватки места снизу).
    pub flipped: bool,
}

pub fn dropdown_menu(anchor: UiRect, viewport: UiRect, content: UiVec2) -> DropdownLayout {
    let width = content.x.max(anchor.w);
    // Горизонталь: левый край якоря, зажат во вьюпорт. FR-068 W0: это
    // position-clamp — сохраняет ширину меню (≥ ширины якоря) в нормальном
    // случае; финальная гарантия [`viewport_clamp`] ниже обрезает до
    // пересечения только меню, не помещающееся во вьюпорт целиком.
    let x = anchor.x.min((viewport.right() - width).max(viewport.x));
    let below_y = anchor.bottom() + DROPDOWN_GAP;
    let fits_below = below_y + content.y <= viewport.bottom();
    let (y, flipped) = if fits_below {
        (below_y, false)
    } else {
        let above_y = anchor.y - DROPDOWN_GAP - content.y;
        if above_y >= viewport.y {
            (above_y, true)
        } else {
            (viewport.bottom() - content.y, false)
        }
    };
    DropdownLayout {
        menu: viewport_clamp(UiRect::new(x, y, width, content.y), viewport),
        flipped,
    }
}

// --- Tooltip («якорь + flip + delay») ---------------------------------------

/// Раскладка тултипа.
pub struct TooltipLayout {
    pub rect: UiRect,
    /// Показан над якорем (flip у нижнего края).
    pub flipped: bool,
}

/// Тултип по точке якоря (курсор): появляется после `hovered_ms >= delay`,
/// у правого/нижнего края — flip (над якорем / прижат влево). Ширина —
/// измеренная (потребитель шейпит текст тем же `TextMeasurer`).
pub fn tooltip(
    anchor: UiPoint,
    text_size: UiVec2,
    viewport: UiRect,
    hovered_ms: u32,
    delay_ms: u32,
) -> Option<TooltipLayout> {
    if hovered_ms < delay_ms {
        return None;
    }
    let size = UiVec2::new(text_size.x.max(1.0), text_size.y.max(1.0));
    let mut x = anchor.x + TOOLTIP_OFFSET.x;
    let mut y = anchor.y + TOOLTIP_OFFSET.y;
    let mut flipped = false;
    if x + size.x > viewport.right() {
        // Перенос влево: якорь-точка остаётся правым краем тултипа
        x = (anchor.x - size.x).max(viewport.x);
    }
    if y + size.y > viewport.bottom() {
        y = anchor.y - TOOLTIP_OFFSET.y - size.y;
        flipped = true;
    }
    // FR-068 W0: семантика position-clamp (сохраняет размер тултипа — текст
    // не клипается; сдвигаем край, а не обрезаем пузырь) — хелпер
    // `viewport_clamp` (пересечение) НЕ эквивалентен. Пузырь размером больше
    // вьюпорта остаётся с полным размером у края — класс выхода за вьюпорт
    // детектируется G4-линтом, а не маскируется.
    let x = x.max(viewport.x);
    let y = y.max(viewport.y);
    Some(TooltipLayout {
        rect: UiRect::new(x, y, size.x, size.y),
        flipped,
    })
}

// --- Toast ------------------------------------------------------------------

/// Область тоста: строка внизу по центру (T21: ширина [40, viewport−40],
/// отступ 44 от низа); `avoid` — rect, над которым тост поднимается
/// (CR-016: what-if бар). TTL — [`TOAST_TTL_MS`] (учёт в потребителе).
pub fn toast_area(viewport: UiRect, avoid: Option<UiRect>) -> UiRect {
    let width = (viewport.right() - 80.0).max(0.0);
    let mut y = viewport.bottom() - 44.0;
    if let Some(bar) = avoid {
        if bar.bottom() + 26.0 > y && bar.y < y {
            y = bar.y - 26.0;
        }
    }
    // FR-068 W0: финальная гарантия — тост не выходит за вьюпорт (подъём
    // над avoid-баром у верхнего края и смещённый вьюпорт клампятся к
    // пересечению); пустой тост (вьюпорт уже 80 — ширина 0) возвращается
    // как есть (пустое пересечение).
    viewport_clamp(UiRect::new(40.0, y, width, 20.0), viewport)
}

// === FR-068 W3: Component (агент 3-b) =======================================
//
// [`Dropdown`] — retained-обёртка над kit-функцией [`dropdown_menu`]: Props —
// декларативный вход кадра, layout делегирует в kit-функцию (паритет
// геометрии 1:1, §Контракт-3 FR-068), paint — хром-поверхность меню из СЛОТОВ
// палитры (контракт «цвета — только слоты», F-8). Интерактивность —
// [`WidgetState`] (FR-057): переходы указателя ведёт потребитель, кит —
// машина состояний.

/// Свойства dropdown-компонента (декларативный вход кадра).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropdownProps {
    /// Якорь — rect контрола, открывшего меню (flip/клампы от него).
    pub anchor: UiRect,
    /// Вьюпорт поверхности — финальная гарантия «меню не выходит за вьюпорт».
    pub viewport: UiRect,
    /// Желаемый размер меню (ширина ≥ ширины якоря — внутри [`dropdown_menu`]).
    pub content: UiVec2,
    /// Палитра-срез: меню — та же хром-поверхность, что панель/модаль
    /// (слоты `panel_fill`/`panel_border`, радиус RADIUS_PANEL).
    pub palette: KitPalette,
}

/// Dropdown-компонент (FR-068 W3): retained — Props + [`WidgetState`].
/// Popup-геометрия задаётся `anchor`/`viewport` из Props, родительский слот
/// в [`Component::layout`] не участвует (меню живёт НАД сценой).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dropdown {
    /// Свойства кадра.
    pub props: DropdownProps,
    /// Состояние интерактивного виджета (set_* — вызовы потребителя;
    /// hover/pressed пунктов меню — поверх, у контейнера своей хром-реакции
    /// на состояния нет — paint не зависит от `state`).
    pub state: WidgetState,
}

impl Component for Dropdown {
    type Props = DropdownProps;

    fn props(&self) -> &Self::Props {
        &self.props
    }

    fn layout(&self, _backend: &dyn crate::layout::LayoutBackend, _slot: UiRect) -> Vec<UiRect> {
        // Делегация в kit-функцию 1:1 (flip/клампы/[`viewport_clamp`] —
        // внутри неё): паритет геометрии с прямым вызовом гарантируется
        // тестом. Backend popup-геометрией не пользуется (rect определяется
        // якорем/вьюпортом).
        vec![dropdown_menu(self.props.anchor, self.props.viewport, self.props.content).menu]
    }

    fn paint(&self, painter: &mut crate::paint::Painter, rects: &[UiRect]) {
        // Меню — панельная хрома из слотов палитры (panel_fill == menu_fill
        // исторически; радиус RADIUS_PANEL — шкала токенов): никаких новых
        // цветов/смешиваний (контракт F-8, [`crate::component::KitPalette`]).
        // Порядок rects = порядок [`Component::layout`] (один rect — меню).
        let style = crate::component::panel::panel_style(&self.props.palette);
        if let Some(menu) = rects.first() {
            painter.panel(*menu, &style);
        }
    }

    // hit_test — дефолтный ([`ComponentHit::pick`]): rect один — меню.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{palette_a, palette_b};
    use crate::component::{ComponentHit, KitState, TOOLTIP_DELAY_MS};
    use crate::paint::{PaintItem, Painter};

    #[test]
    fn dropdown_flips_when_bottom_full() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // Якорь у низа: меню разворачивается вверх
        let anchor = UiRect::new(100.0, 560.0, 200.0, 590.0);
        let d = dropdown_menu(anchor, vp, UiVec2::new(160.0, 90.0));
        assert!(d.flipped);
        assert!((d.menu.bottom() - (anchor.y - DROPDOWN_GAP)).abs() < 0.01);
        // Якорь у верха: меню снизу
        let anchor = UiRect::new(100.0, 10.0, 200.0, 40.0);
        let d = dropdown_menu(anchor, vp, UiVec2::new(160.0, 90.0));
        assert!(!d.flipped);
        assert!((d.menu.y - (anchor.bottom() + DROPDOWN_GAP)).abs() < 0.01);
        // Не влезает ни снизу, ни сверху — прижато к низу вьюпорта
        let anchor = UiRect::new(100.0, 290.0, 200.0, 310.0);
        let d = dropdown_menu(anchor, vp, UiVec2::new(160.0, 600.0));
        assert!(!d.flipped);
        assert!((d.menu.bottom() - vp.bottom()).abs() < 0.01);
        // Ширина меню ≥ ширины якоря, зажато во вьюпорт
        let anchor = UiRect::new(700.0, 10.0, 790.0, 40.0);
        let d = dropdown_menu(anchor, vp, UiVec2::new(50.0, 60.0));
        assert!(d.menu.right() <= vp.right() + 0.01);
        assert!(d.menu.w >= anchor.w - 0.01);
    }
    #[test]
    fn tooltip_delay_and_flip() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let size = UiVec2::new(120.0, 18.0);
        // До delay — None
        assert!(tooltip(UiPoint::new(400.0, 300.0), size, vp, 200, TOOLTIP_DELAY_MS).is_none());
        // После — Some, ниже-справа от якоря
        let t = tooltip(UiPoint::new(400.0, 300.0), size, vp, 500, TOOLTIP_DELAY_MS).unwrap();
        assert!(!t.flipped);
        assert!((t.rect.x - (400.0 + TOOLTIP_OFFSET.x)).abs() < 0.01);
        // У нижнего края — flip вверх
        let t = tooltip(UiPoint::new(400.0, 595.0), size, vp, 500, TOOLTIP_DELAY_MS).unwrap();
        assert!(t.flipped);
        assert!(t.rect.bottom() <= vp.bottom() + 0.01);
        // У правого края — влево (правый край не выходит за вьюпорт)
        let t = tooltip(UiPoint::new(795.0, 300.0), size, vp, 500, TOOLTIP_DELAY_MS).unwrap();
        assert!(t.rect.right() <= vp.right() + 0.01);
    }
    #[test]
    fn toast_area_lifts_above_avoid_bar() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let plain = toast_area(vp, None);
        assert!((plain.x - 40.0).abs() < 0.01);
        assert!((plain.right() - (800.0 - 40.0)).abs() < 0.01);
        assert!((plain.y - (600.0 - 44.0)).abs() < 0.01);
        // what-if бар занимает низ — тост над ним (CR-016)
        let bar = UiRect::new(0.0, 540.0, 800.0, 596.0);
        let lifted = toast_area(vp, Some(bar));
        assert!(lifted.bottom() <= bar.y + 0.01);
    }
    /// FR-068 W0: контракт хелпера [`viewport_clamp`] — пересечение при
    /// наличии, иначе исходный rect. (1) rect внутри вьюпорта — без
    /// изменений; (2) частично вне — обрезан до пересечения; (3) целиком
    /// вне — исходный rect КАК ЕСТЬ (класс выхода за вьюпорт не маскируется —
    /// детектируется G4-линтом); (4) вырожденный вьюпорт — исходный rect.
    #[test]
    fn viewport_clamp_intersects_or_keeps_original() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // 1) Внутри — без изменений
        let inside = UiRect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(viewport_clamp(inside, vp), inside);
        // 2) Частично вне — пересечение (обрезан до вьюпорта)
        let partial = UiRect::new(700.0, 500.0, 200.0, 200.0);
        assert_eq!(
            viewport_clamp(partial, vp),
            UiRect::new(700.0, 500.0, 100.0, 100.0)
        );
        // 3) Целиком вне — исходный rect как есть
        let outside = UiRect::new(1000.0, 700.0, 50.0, 50.0);
        assert_eq!(viewport_clamp(outside, vp), outside);
        // 4) Вырожденный (пустой) вьюпорт — исходный rect без изменений
        let degenerate = UiRect::new(0.0, 0.0, 0.0, 600.0);
        assert_eq!(viewport_clamp(inside, degenerate), inside);
    }
    /// FR-068 W0: dropdown — ручной position-clamp по горизонтали (ширина
    /// меню сохраняется) дополнен финальной гарантией [`viewport_clamp`]:
    /// меню шире/выше вьюпорта обрезается до пересечения (класс выхода
    /// устранён), flip-контракт не затронут.
    #[test]
    fn dropdown_menu_overflow_clamped_to_viewport() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // Меню шире вьюпорта: горизонталь обрезана до вьюпорта (w = vp.w)
        let d = dropdown_menu(
            UiRect::new(100.0, 10.0, 200.0, 40.0),
            vp,
            UiVec2::new(1000.0, 90.0),
        );
        assert!(!d.flipped);
        assert_eq!(d.menu, UiRect::new(0.0, 54.0, 800.0, 90.0));
        // Не влезает ни снизу, ни сверху, и выше вьюпорта: вертикаль обрезана
        let d = dropdown_menu(
            UiRect::new(100.0, 290.0, 200.0, 310.0),
            vp,
            UiVec2::new(160.0, 900.0),
        );
        assert!(!d.flipped);
        assert_eq!(d.menu, UiRect::new(100.0, 0.0, 200.0, 600.0));
    }
    /// FR-068 W0: toast — финальная гарантия [`viewport_clamp`]: смещённый
    /// вьюпорт и подъём над avoid-баром у верхнего края клампятся к
    /// пересечению; пустой тост (вьюпорт уже 80 — ширина 0) возвращается
    /// как есть.
    #[test]
    fn toast_area_clamped_to_viewport() {
        // Смещённый вьюпорт: левый край тоста (40) левее вьюпорта — обрезан
        let vp = UiRect::new(100.0, 0.0, 300.0, 600.0);
        assert_eq!(toast_area(vp, None), UiRect::new(100.0, 556.0, 260.0, 20.0));
        // Avoid-бар у самого верха: подъём выше вьюпорта клампится к краю
        let vp = UiRect::new(0.0, 0.0, 800.0, 100.0);
        let bar = UiRect::new(0.0, 10.0, 800.0, 90.0);
        assert_eq!(
            toast_area(vp, Some(bar)),
            UiRect::new(40.0, 0.0, 720.0, 4.0),
            "внутри вьюпорта остался только хвост тоста (4 px)"
        );
        // Вьюпорт уже 80 — ширина 0: пустой rect возвращается как есть
        let vp = UiRect::new(0.0, 0.0, 60.0, 600.0);
        let t = toast_area(vp, None);
        assert!(t.is_empty());
        assert_eq!(t, UiRect::new(40.0, 556.0, 0.0, 20.0));
    }
    /// FR-068 W0: tooltip — семантика position-clamp СОХРАНЯЕТ размер:
    /// пузырь, не помещающийся во вьюпорт, прижимается к краю с полным
    /// размером (текст не клипается; хелпер-пересечение не эквивалентен —
    /// выход пузыря детектируется G4-линтом).
    #[test]
    fn tooltip_position_clamp_keeps_size() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let huge = UiVec2::new(2000.0, 900.0);
        let t = tooltip(UiPoint::new(5.0, 5.0), huge, vp, 500, TOOLTIP_DELAY_MS).unwrap();
        // Перенос влево/вверх упирается в край (0,0), размер сохранён
        assert_eq!(t.rect, UiRect::new(0.0, 0.0, 2000.0, 900.0));
        assert!(t.flipped);
    }

    // === FR-068 W3: Component (Dropdown) ====================================

    /// Component::layout — rect меню из kit-функции [`dropdown_menu`] 1:1
    /// (паритет геометрии) с финальной гарантией вьюпорта W0: меню в
    /// пределах viewport (сценарий «правый низ»: флип вверх + сдвиг влево —
    /// как в `dropdown_flips_when_bottom_full`). Слот родителя popup-
    /// геометрией не участвует (якорь/вьюпорт — в Props).
    #[test]
    fn dropdown_component_layout_returns_kit_menu_within_viewport() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let d = Dropdown {
            props: DropdownProps {
                anchor: UiRect::new(700.0, 560.0, 90.0, 30.0),
                viewport: vp,
                content: UiVec2::new(160.0, 90.0),
                palette: palette_a(),
            },
            state: WidgetState::default(),
        };
        // golden: width = 160 (≥ якоря), x = 640 (кламп правого края),
        // флип вверх: y = 560 − 4 − 90 = 466
        assert_eq!(
            d.layout(crate::layout::default_backend(), vp),
            vec![UiRect::new(640.0, 466.0, 160.0, 90.0)]
        );
        assert_eq!(
            d.layout(crate::layout::default_backend(), vp)[0],
            dropdown_menu(d.props.anchor, d.props.viewport, d.props.content).menu,
            "Component::layout делегирует в kit-функцию 1:1"
        );
        let menu = d.layout(crate::layout::default_backend(), vp)[0];
        assert!(menu.x >= vp.x && menu.right() <= vp.right());
        assert!(menu.y >= vp.y && menu.bottom() <= vp.bottom());
        assert_eq!(*d.props(), d.props, "props() — доступ к свойствам");
    }

    /// Component::paint — меню рисуется панельной хромой из СЛОТОВ палитры
    /// (panel_fill/panel_border, радиус RADIUS_PANEL — шкала токенов):
    /// смена палитры меняет item (стиль выбирает слот, не вычисляет цвет —
    /// контракт F-8), порядок rects = порядок layout (один rect — меню).
    #[test]
    fn dropdown_component_paint_emits_panel_slots() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        for palette in [palette_a(), palette_b()] {
            let d = Dropdown {
                props: DropdownProps {
                    anchor: UiRect::new(100.0, 10.0, 200.0, 40.0),
                    viewport: vp,
                    content: UiVec2::new(160.0, 90.0),
                    palette,
                },
                state: WidgetState::default(),
            };
            let rects = d.layout(crate::layout::default_backend(), vp);
            let mut painter = Painter::new();
            d.paint(&mut painter, &rects);
            let items = painter.items();
            assert_eq!(items.len(), 1, "меню — один Rect-item");
            assert_eq!(
                items[0],
                PaintItem::Rect {
                    rect: rects[0],
                    fill: palette.panel_fill,
                    border: palette.panel_border,
                    radius: canvas_core::tokens::RADIUS_PANEL,
                }
            );
        }
    }

    /// Component: hit_test — дефолтный (первый содержащий rect — он же
    /// единственный): точка в меню — index 0, вне — None. WidgetState
    /// (FR-057) доступен потребителю: переход указателя даёт hover.
    #[test]
    fn dropdown_component_default_hit_test_and_state() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let mut d = Dropdown {
            props: DropdownProps {
                anchor: UiRect::new(100.0, 10.0, 200.0, 40.0),
                viewport: vp,
                content: UiVec2::new(160.0, 90.0),
                palette: palette_a(),
            },
            state: WidgetState::default(),
        };
        let rects = d.layout(crate::layout::default_backend(), vp);
        assert_eq!(
            d.hit_test(&rects, UiPoint::new(150.0, 60.0)),
            Some(ComponentHit { index: 0 })
        );
        assert_eq!(d.hit_test(&rects, UiPoint::new(5.0, 5.0)), None);
        // Пустой срез rects (потребитель не вызвал layout) — None, не паника.
        assert_eq!(d.hit_test(&[], UiPoint::new(150.0, 60.0)), None);
        // WidgetState — машина состояний потребителя (кит не ведёт ввод).
        d.state.set_pointer(true, false);
        assert_eq!(d.state.kit_state(), KitState::Hovered);
    }
}
