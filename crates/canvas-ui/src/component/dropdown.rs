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

// --- FR-UI-ANCHORED-STACK: многоэлементный стек с flip+clamp ---------------

/// Сторона якоря, на которую раскрывается стек (FR-UI-ANCHORED-STACK).
///
/// Обобщение [`dropdown_menu`] (которая неявно использует «снизу от якоря»):
/// стек из N элементов может открываться с любой из 4 сторон `anchor`, при
/// нехватке места — флип на противоположную сторону, при нехватке и там —
/// position-clamp в сторону viewport (с сохранением размеров).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchoredSide {
    /// Стек справа от якоря (элементы стакаются сверху вниз).
    Right,
    /// Стек слева от якоря (элементы стакаются сверху вниз).
    Left,
    /// Стек снизу от якоря (элементы стакаются слева направо).
    Bottom,
    /// Стек сверху от якоря (элементы стакаются слева направо).
    Top,
}

/// FR-UI-ANCHORED-STACK: anchored stack of N elements with flip+clamp.
/// Generalizes [`dropdown_menu`] (1 элемент) на многоэлементные стеки
/// (suggest-карточки, tooltip-стеки, flyout-меню).
///
/// `anchor` — rect, к которому привязан стек (например, правый край ноды).
/// `side` — сторона якоря, в которую стек раскрывается естественным образом.
/// `element_sizes` — `(w, h)` для каждого элемента стека (допускаются разные).
/// `gap` — зазор между соседними элементами И от якоря до первого элемента
///   (типовой UI-паттерн — единый spacing-scale: [`GAP_CONTROLS`] / SPACING_S;
///   suggest-карточки используют `SUGGEST_CARD_GAP == SUGGEST_CARD_OFFSET_X`).
/// `viewport` — видимый rect для flip+clamp (потребитель кодирует поля:
///   viewport.x/y — внешние поля, viewport.right()/bottom() — внутренние
///   края клампов).
///
/// Возвращает `Vec<UiRect>` — один rect на элемент, позиционированный и
/// зажатый во вьюпорт. Если стек не помещается на `side`, флипает на
/// противоположную сторону (с сохранением размера — position-clamp,
/// семантика как у [`tooltip`]); если не помещается и там — прижимается к
/// краю viewport с тем же размером (выход за viewport НЕ маскируется —
/// класс детектируется G4-линтом). По поперечной оси (вертикаль для
/// Right/Left, горизонталь для Top/Bottom) стек выравнивается по
/// соответствующему краю якоря и прижимается к верхнему/левому краю viewport
/// при переполнении (см. тесты `suggest::card_rects_*` — бит-в-бит паритет).
pub fn anchored_stack(
    anchor: UiRect,
    side: AnchoredSide,
    element_sizes: &[(f32, f32)],
    gap: f32,
    viewport: UiRect,
) -> Vec<UiRect> {
    let n = element_sizes.len();
    let mut out = Vec::with_capacity(n);
    if n == 0 {
        return out;
    }
    match side {
        AnchoredSide::Right | AnchoredSide::Left => {
            let max_w = element_sizes.iter().map(|(w, _)| *w).fold(0.0f32, f32::max);
            let total_h: f32 =
                element_sizes.iter().map(|(_, h)| *h).sum::<f32>() + gap * (n - 1) as f32;
            // Поперечная ось (X): side=Right → стек справа, флип на Left.
            let x_natural_right = anchor.right() + gap;
            let x_flipped_left = anchor.x - gap - max_w;
            let x = match side {
                AnchoredSide::Right => {
                    if x_natural_right + max_w <= viewport.right() {
                        x_natural_right
                    } else {
                        // Флип на левую сторону, position-clamp к viewport.x
                        // (паритет suggest.rs `.max(viewport.x)`).
                        x_flipped_left.max(viewport.x)
                    }
                }
                AnchoredSide::Left => {
                    if x_flipped_left >= viewport.x {
                        x_flipped_left
                    } else {
                        // Флип на правую сторону, position-clamp к viewport.right()
                        x_natural_right.min(viewport.right() - max_w)
                    }
                }
                _ => unreachable!(),
            };
            // Продольная ось (Y): выравнивание по верху якоря, прижим к
            // низу viewport если не помещается естественной стопкой, к верху
            // viewport если не помещается никак (паритет suggest.rs::card_rects).
            let y0 = if anchor.y + total_h <= viewport.bottom() {
                anchor.y
            } else if total_h <= viewport.h {
                viewport.bottom() - total_h
            } else {
                viewport.y
            };
            let mut y = y0;
            for &(w, h) in element_sizes {
                out.push(UiRect::new(x, y, w, h));
                y += h + gap;
            }
        }
        AnchoredSide::Top | AnchoredSide::Bottom => {
            let max_h = element_sizes.iter().map(|(_, h)| *h).fold(0.0f32, f32::max);
            let total_w: f32 =
                element_sizes.iter().map(|(w, _)| *w).sum::<f32>() + gap * (n - 1) as f32;
            let y_natural_bottom = anchor.bottom() + gap;
            let y_flipped_top = anchor.y - gap - max_h;
            let y = match side {
                AnchoredSide::Bottom => {
                    if y_natural_bottom + max_h <= viewport.bottom() {
                        y_natural_bottom
                    } else {
                        y_flipped_top.max(viewport.y)
                    }
                }
                AnchoredSide::Top => {
                    if y_flipped_top >= viewport.y {
                        y_flipped_top
                    } else {
                        y_natural_bottom.min(viewport.bottom() - max_h)
                    }
                }
                _ => unreachable!(),
            };
            let x0 = if anchor.x + total_w <= viewport.right() {
                anchor.x
            } else if total_w <= viewport.w {
                viewport.right() - total_w
            } else {
                viewport.x
            };
            let mut x = x0;
            for &(w, h) in element_sizes {
                out.push(UiRect::new(x, y, w, h));
                x += w + gap;
            }
        }
    }
    out
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

    // === FR-UI-ANCHORED-STACK: tests ========================================

    /// Стек из 3 элементов на правой стороне якоря — помещается во вьюпорт
    /// без флипа: первый элемент у правого края якоря + gap, последующие
    /// стакаются вниз с тем же gap.
    #[test]
    fn anchored_stack_fits_on_right_side() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = UiRect::new(100.0, 100.0, 120.0, 60.0);
        let sizes = [(80.0, 40.0), (80.0, 40.0), (80.0, 40.0)];
        let rects = anchored_stack(anchor, AnchoredSide::Right, &sizes, 8.0, vp);
        assert_eq!(rects.len(), 3);
        // X единый для всей стопки (правее якоря на gap=8)
        let x_expected = anchor.right() + 8.0; // 228
        for r in &rects {
            assert!((r.x - x_expected).abs() < 0.01);
            assert!((r.w - 80.0).abs() < 0.01);
            assert!((r.h - 40.0).abs() < 0.01);
        }
        // Y — лесенка с шагом h+gap=48, стартует на anchor.y=100
        assert!((rects[0].y - 100.0).abs() < 0.01);
        assert!((rects[1].y - 148.0).abs() < 0.01);
        assert!((rects[2].y - 196.0).abs() < 0.01);
        // Все элементы — во вьюпорте
        for r in &rects {
            assert!(r.x >= vp.x && r.right() <= vp.right() + 0.01);
            assert!(r.y >= vp.y && r.bottom() <= vp.bottom() + 0.01);
        }
    }

    /// Стек не помещается справа (правый край якоря у самого края viewport) —
    /// флипает на левую сторону: X = anchor.x - gap - max_w, position-clamp
    /// к viewport.x при выходе за край.
    #[test]
    fn anchored_stack_flips_to_left_when_right_full() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        // Якорь у правого края вьюпорта: стек справа не помещается
        let anchor = UiRect::new(700.0, 100.0, 80.0, 60.0);
        let sizes = [(240.0, 80.0), (240.0, 80.0)];
        let rects = anchored_stack(anchor, AnchoredSide::Right, &sizes, 12.0, vp);
        assert_eq!(rects.len(), 2);
        // Флип на левую сторону: x = anchor.x - gap - max_w = 700 - 12 - 240 = 448
        assert!((rects[0].x - 448.0).abs() < 0.01);
        assert!((rects[1].x - 448.0).abs() < 0.01, "стопка вертикальная");
        // Y — лесенка с шагом 80+12=92, стартует на anchor.y=100 (помещается)
        assert!((rects[0].y - 100.0).abs() < 0.01);
        assert!((rects[1].y - 192.0).abs() < 0.01);
        // Флипнутый стек не выходит за viewport
        for r in &rects {
            assert!(r.x >= vp.x && r.right() <= vp.right() + 0.01);
        }
        // Если флипнутая позиция уходит левее viewport.x — position-clamp
        // к viewport.x (паритет suggest.rs `.max(viewport.x)`).
        let vp = UiRect::new(4.0, 4.0, 100.0, 600.0); // узкий вьюпорт
        let anchor = UiRect::new(50.0, 100.0, 40.0, 60.0);
        let sizes = [(240.0, 80.0)];
        let rects = anchored_stack(anchor, AnchoredSide::Right, &sizes, 12.0, vp);
        // x_alt = 50 - 12 - 240 = -202; .max(viewport.x=4) = 4
        assert!(
            (rects[0].x - 4.0).abs() < 0.01,
            "position-clamp к viewport.x"
        );
    }

    /// Стек выше viewport — прижимается к верхнему краю viewport (y0 =
    /// viewport.y), наложения между элементами сохранены (шаг h+gap).
    #[test]
    fn anchored_stack_taller_than_viewport_clamps_to_top() {
        let vp = UiRect::new(0.0, 4.0, 800.0, 552.0); // viewport.bottom()=556
                                                      // Якорь у нижнего края: естественная стопка уходит за низ.
        let anchor = UiRect::new(300.0, 520.0, 120.0, 60.0);
        // 8 карточек: total_h = 7*92 + 80 = 724 > viewport.h=552
        let sizes = [(240.0, 80.0); 8];
        let rects = anchored_stack(anchor, AnchoredSide::Right, &sizes, 12.0, vp);
        assert_eq!(rects.len(), 8);
        // Прижато к верхнему краю viewport (y0 = viewport.y = 4)
        assert!((rects[0].y - 4.0).abs() < 0.01, "прижата к верху viewport");
        // Шаг лесенки сохранён — наложений нет
        for pair in rects.windows(2) {
            assert!(
                pair[1].y >= pair[0].y + pair[0].h,
                "элементы не налагаются (шаг сохранён)"
            );
            assert!((pair[1].y - pair[0].y - 92.0).abs() < 0.01, "шаг h+gap");
        }
    }

    /// Bottom-side стек (горизонтальная стопка снизу от якоря) — элементы
    /// стакаются слева направо; при переполнении по ширине — position-clamp.
    #[test]
    fn anchored_stack_bottom_side_horizontal() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = UiRect::new(100.0, 100.0, 200.0, 40.0);
        let sizes = [(80.0, 30.0), (80.0, 30.0), (80.0, 30.0)];
        let rects = anchored_stack(anchor, AnchoredSide::Bottom, &sizes, 8.0, vp);
        assert_eq!(rects.len(), 3);
        // Y единый — ниже якоря на gap=8: anchor.bottom()+gap = 148
        for r in &rects {
            assert!((r.y - 148.0).abs() < 0.01);
            assert!((r.h - 30.0).abs() < 0.01);
        }
        // X — лесенка слева направо, стартует на anchor.x=100, шаг w+gap=88
        assert!((rects[0].x - 100.0).abs() < 0.01);
        assert!((rects[1].x - 188.0).abs() < 0.01);
        assert!((rects[2].x - 276.0).abs() < 0.01);
    }

    /// Пустой вход → пустой выход; один элемент — корректный rect.
    #[test]
    fn anchored_stack_empty_and_single() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = UiRect::new(100.0, 100.0, 120.0, 60.0);
        assert!(anchored_stack(anchor, AnchoredSide::Right, &[], 8.0, vp).is_empty());
        // Одиночная карточка у нижнего края — position-clamp к низу viewport
        // (паритет suggest.rs::card_rects `single[0].y == vh-4-H`).
        let vp = UiRect::new(0.0, 4.0, 800.0, 552.0); // bottom=556
        let anchor = UiRect::new(300.0, 520.0, 120.0, 60.0);
        let sizes = [(240.0, 80.0)];
        let rects = anchored_stack(anchor, AnchoredSide::Right, &sizes, 12.0, vp);
        assert_eq!(rects.len(), 1);
        // total_h=80 > viewport.h-... нет: 520+80=600 > 556 (не помещается естест.);
        // 80 <= 552 → y0 = 556-80 = 476
        assert!(
            (rects[0].y - 476.0).abs() < 0.01,
            "одиночный кламп к низу viewport"
        );
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
