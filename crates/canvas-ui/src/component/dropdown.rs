//! FR-068 W3: dropdown/tooltip/toast (+ taffy-пути) — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-b): дополнить `Props` + `impl Component` для
//! dropdown; taffy-варианты — opt-in parity-пути W1 (семантику не менять).

use super::{DROPDOWN_GAP, TOOLTIP_OFFSET};
use crate::geometry::{UiPoint, UiRect, UiVec2};

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

// === FR-068 W1 (ADR-0014 §Решение п.3): opt-in taffy-пути kit-функций =======
//
// Позиция/скролл-геометрия scroll-area/dropdown/tooltip/toast/modal,
// решаемая через `TaffyBackend` (сцена [`SceneNode`] / `TaffyBackend::
// centered`). Каждый taffy-путь:
// - OPT-IN: только за фичей `taffy`; default-сборка использует native-
//   функцию этого файла — поведение default НЕ меняется (§Контракт-2
//   FR-068, zero-dep G7);
// - parity с native зафиксирован тестами (`mod tests::taffy_parity`;
//   побитово на целых входах, документированные расхождения — отдельно);
// - финальные гарантии W0 сохранены ([`viewport_clamp`]-пересечение либо
//   position-clamp — ровно как у соответствующей native-функции).
// Решающая логика (flip/клампы/constrain) скопирована с native 1:1 — через
// taffy-сцену проводится только финальный rect (позиция absolute /
// центрирование), поэтому parity сводится к прозрачности транспорта.
//
// Известное свойство транспорта: `compute_layout` taffy ОКРУГЛЯЕТ позиции к
// целому ui px (round_layout, taffy 0.14; конфиг дерева фиксирован в
// `layout::taffy_backend`) — поэтому parity побитовый на ЦЕЛЫХ входах;
// дробные позиции — документированное расхождение ≤ 0.5 ui px (то же
// семейство, что дробные grow-доли, см. доку `taffy_backend`), фиксируется
// отдельным тестом (`tooltip_taffy_fractional_position_is_documented_divergence`).

#[cfg(feature = "taffy")]
use crate::layout::{SceneNode, ScenePosition, TaffyBackend};

/// ZST-backend taffy-путей (immediate-mode, без состояния — см. доку
/// `layout::taffy_backend`).
#[cfg(feature = "taffy")]
const TAFFY: TaffyBackend = TaffyBackend;

/// Провести rect через taffy-сцену «viewport (definite) + absolute-ребёнок»
/// (FR-068 W1): `x`/`y` — АБСОЛЮТНЫЕ экранные координаты (кандидат позиции
/// native-логики живёт в том же пространстве, что `anchor`/`viewport`);
/// внутри переводятся в относительные inset'ы ([`ScenePosition::Absolute`]
/// отсчитывается от border-box родителя — корень сцены стоит в origin
/// вьюпорта). Размер задан вызовом и сценой не меняется. Возвращает rect
/// ребёнка в абсолютных координатах (индекс `[1]` DFS pre-order; `[0]` —
/// корень == viewport). На целых входах транспорт побитово прозрачен
/// (дробные — округление taffy, см. доку секции).
#[cfg(feature = "taffy")]
fn scene_absolute(viewport: UiRect, x: f32, y: f32, w: f32, h: f32) -> UiRect {
    let scene = SceneNode::column(
        viewport.w,
        viewport.h,
        0.0,
        vec![SceneNode::leaf(w, h).at(ScenePosition::Absolute {
            x: x - viewport.x,
            y: y - viewport.y,
        })],
    );
    let rects = TAFFY.lay_out_scene(viewport, &scene);
    rects[1]
}

/// Opt-in taffy-путь (FR-068 W1) [`dropdown_menu`]: решающая логика native
/// скопирована 1:1 (ширина ≥ якоря, горизонтальный position-clamp, flip
/// вверх при нехватке места снизу, иначе прижатие к низу вьюпорта);
/// финальный rect меню проводится через taffy-сцену — корень = viewport
/// (definite), меню = [`ScenePosition::Absolute`] с кандидатной позицией
/// (taffy даёт позицию absolute + тот же rect; размер меню задан `content`),
/// затем ОБЯЗАТЕЛЬНАЯ финальная гарантия [`viewport_clamp`] (W0). Default-
/// сборка использует native-функцию; parity зафиксирован тестами — побитово
/// на целых входах (матрица якорей: края/центр/низ-флип + W0-переполнения).
#[cfg(feature = "taffy")]
pub fn dropdown_menu_taffy(anchor: UiRect, viewport: UiRect, content: UiVec2) -> DropdownLayout {
    let width = content.x.max(anchor.w);
    // Горизонталь — как native: position-clamp, ширина меню сохраняется
    // (обрезка до пересечения — только финальным viewport_clamp ниже).
    let x = anchor.x.min((viewport.right() - width).max(viewport.x));
    let below_y = anchor.bottom() + DROPDOWN_GAP;
    let fits_below = below_y + content.y <= viewport.bottom();
    let (x0, y, flipped) = if fits_below {
        (x, below_y, false)
    } else {
        let above_y = anchor.y - DROPDOWN_GAP - content.y;
        if above_y >= viewport.y {
            (x, above_y, true)
        } else {
            (x, viewport.bottom() - content.y, false)
        }
    };
    let menu = scene_absolute(viewport, x0, y, width, content.y);
    DropdownLayout {
        menu: viewport_clamp(menu, viewport),
        flipped,
    }
}

/// Opt-in taffy-путь (FR-068 W1) [`tooltip`] — БЕЗ delay-семантики
/// (гистерезис `hovered_ms ≥ delay` — забота потребителя; `anchor` — точка
/// курсора `anchor.x`/`anchor.y`, w/h якоря не используются, как у native
/// [`tooltip`]). Логика position-clamp native скопирована 1:1: у правого
/// края — перенос влево (якорь остаётся правым краем), у нижнего — flip
/// вверх, финальный сдвиг внутрь вьюпорта СОХРАНЯЕТ размер (W0: НЕ
/// [`viewport_clamp`]-пересечение — текст не клипается). Кандидатная позиция
/// проводится через taffy-сцену ([`ScenePosition::Absolute`]). Default-сборка
/// использует native-функцию; parity зафиксирован тестами (побитово на
/// матрице якорей, включая пузырь больше вьюпорта).
#[cfg(feature = "taffy")]
pub fn tooltip_taffy(anchor: UiRect, viewport: UiRect, text_w: f32, text_h: f32) -> UiRect {
    let size = UiVec2::new(text_w.max(1.0), text_h.max(1.0));
    let mut x = anchor.x + TOOLTIP_OFFSET.x;
    let mut y = anchor.y + TOOLTIP_OFFSET.y;
    if x + size.x > viewport.right() {
        // Перенос влево: якорь-точка остаётся правым краем тултипа
        x = (anchor.x - size.x).max(viewport.x);
    }
    if y + size.y > viewport.bottom() {
        // Flip вверх
        y = anchor.y - TOOLTIP_OFFSET.y - size.y;
    }
    // Position-clamp (W0): сдвиг внутрь вьюпорта, размер сохранён
    let x = x.max(viewport.x);
    let y = y.max(viewport.y);
    scene_absolute(viewport, x, y, size.x, size.y)
}

/// Opt-in taffy-путь (FR-068 W1) [`toast_area`]: зона тоста (строка внизу по
/// центру: x = 40, ширина [0, viewport−80], отступ 44 от низа, avoid-подъём
/// CR-016) — логика native 1:1; финальный rect проводится через taffy-сцену
/// ([`ScenePosition::Absolute`]) с последующей финальной гарантией
/// [`viewport_clamp`] (W0: смещённый вьюпорт и подъём над avoid-баром
/// клампятся к пересечению; пустой тост (ширина 0) возвращается как есть).
/// Default-сборка использует native-функцию; parity зафиксирован тестами
/// (побитово: с avoid и без, смещённый/узкий вьюпорт).
#[cfg(feature = "taffy")]
pub fn toast_area_taffy(viewport: UiRect, avoid: Option<UiRect>) -> UiRect {
    let width = (viewport.right() - 80.0).max(0.0);
    let mut y = viewport.bottom() - 44.0;
    if let Some(bar) = avoid {
        if bar.bottom() + 26.0 > y && bar.y < y {
            y = bar.y - 26.0;
        }
    }
    let rect = scene_absolute(viewport, 40.0, y, width, 20.0);
    viewport_clamp(rect, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::TOOLTIP_DELAY_MS;
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
}

#[cfg(all(test, feature = "taffy"))]
mod taffy_parity {
    use super::*;
    use crate::component::TOOLTIP_DELAY_MS;

    /// dropdown: parity с native [`dropdown_menu`] — ПОБИТОВО на матрице
    /// якорей (все размеры целые): центр/обычный, правый край
    /// (сдвиг влево), низ (флип вверх), правый низ, якорь левее
    /// вьюпорта, широкий якорь; плюс W0-переполнения (меню шире/выше
    /// вьюпорта, точное прилегание к низу).
    #[test]
    fn dropdown_menu_taffy_parity_with_native() {
        let content = UiVec2::new(160.0, 90.0);
        let anchors = [
            UiRect::new(100.0, 10.0, 200.0, 40.0),  // под якорем
            UiRect::new(360.0, 280.0, 80.0, 30.0),  // центр вьюпорта
            UiRect::new(680.0, 100.0, 40.0, 30.0),  // правый край: сдвиг влево
            UiRect::new(100.0, 520.0, 200.0, 30.0), // низ: флип вверх
            UiRect::new(100.0, 570.0, 200.0, 30.0), // у самого низа: флип
            UiRect::new(620.0, 560.0, 60.0, 30.0),  // правый низ: флип + кламп
            UiRect::new(-50.0, 100.0, 60.0, 30.0),  // левее вьюпорта
            UiRect::new(700.0, 10.0, 790.0, 40.0),  // широкий якорь (w > content.x)
        ];
        for anchor in anchors {
            let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
            let native = dropdown_menu(anchor, vp, content);
            let taffy = dropdown_menu_taffy(anchor, vp, content);
            assert_eq!(
                (taffy.menu.x, taffy.menu.y, taffy.menu.w, taffy.menu.h),
                (native.menu.x, native.menu.y, native.menu.w, native.menu.h),
                "parity меню: anchor={anchor:?}"
            );
            assert_eq!(
                taffy.flipped, native.flipped,
                "parity флипа: anchor={anchor:?}"
            );
        }
        // Смещённый вьюпорт: флип вверх; кандидат позиции — в АБСОЛЮТНЫХ
        // координатах (транспорт переводит их в относительные inset'ы
        // сцены — контракт ScenePosition::Absolute от origin вьюпорта)
        let vp = UiRect::new(100.0, 50.0, 300.0, 400.0);
        let anchor = UiRect::new(150.0, 400.0, 60.0, 30.0);
        {
            let native = dropdown_menu(anchor, vp, content);
            let taffy = dropdown_menu_taffy(anchor, vp, content);
            assert!(native.flipped);
            assert_eq!(
                (taffy.menu.x, taffy.menu.y, taffy.menu.w, taffy.menu.h),
                (native.menu.x, native.menu.y, native.menu.w, native.menu.h),
                "parity меню (смещённый вьюпорт)"
            );
            assert_eq!(taffy.flipped, native.flipped);
        }
        // W0-переполнения: меню шире/выше вьюпорта, точное прилегание
        let contents = [
            UiVec2::new(1000.0, 90.0), // шире вьюпорта: клип до vp.w
            UiVec2::new(160.0, 900.0), // выше вьюпорта: клип до vp.h
            UiVec2::new(160.0, 600.0), // ни снизу ни сверху: прижат к низу
            UiVec2::new(160.0, 496.0), // впритык снизу (без клипа)
        ];
        let anchor = UiRect::new(100.0, 10.0, 200.0, 40.0);
        for content in contents {
            let native = dropdown_menu(anchor, vp, content);
            let taffy = dropdown_menu_taffy(anchor, vp, content);
            assert_eq!(
                (taffy.menu.x, taffy.menu.y, taffy.menu.w, taffy.menu.h),
                (native.menu.x, native.menu.y, native.menu.w, native.menu.h),
                "parity W0: content={content:?}"
            );
            assert_eq!(taffy.flipped, native.flipped);
        }
    }
    /// tooltip: parity с native [`tooltip`] — ПОБИТОВО на матрице якорей
    /// (ЦЕЛЫЕ входы: центр, правый край — перенос влево, нижний — флип,
    /// угол, пузырь больше вьюпорта — position-clamp W0 с сохранением
    /// размера, смещённый вьюпорт). taffy-путь без delay-семантики —
    /// native сравнивается с hovered_ms = delay (всегда Some).
    #[test]
    fn tooltip_taffy_parity_with_native() {
        let cases = [
            (
                UiRect::new(0.0, 0.0, 800.0, 600.0),
                400.0,
                300.0,
                120.0,
                18.0,
            ), // центр
            (
                UiRect::new(0.0, 0.0, 800.0, 600.0),
                795.0,
                300.0,
                120.0,
                18.0,
            ), // правый край: влево
            (
                UiRect::new(0.0, 0.0, 800.0, 600.0),
                400.0,
                595.0,
                120.0,
                18.0,
            ), // низ: флип вверх
            (
                UiRect::new(0.0, 0.0, 800.0, 600.0),
                795.0,
                595.0,
                120.0,
                18.0,
            ), // угол: влево + флип
            (UiRect::new(0.0, 0.0, 800.0, 600.0), 5.0, 5.0, 2000.0, 900.0), // пузырь больше вьюпорта (W0)
            (UiRect::new(0.0, 0.0, 800.0, 600.0), 0.0, 0.0, 100.0, 20.0),   // угол вьюпорта
            (
                UiRect::new(100.0, 50.0, 300.0, 400.0),
                350.0,
                430.0,
                120.0,
                18.0,
            ), // смещённый вьюпорт: влево + флип
        ];
        for (vp, ax, ay, tw, th) in cases {
            let native = tooltip(
                UiPoint::new(ax, ay),
                UiVec2::new(tw, th),
                vp,
                TOOLTIP_DELAY_MS,
                TOOLTIP_DELAY_MS,
            )
            .unwrap()
            .rect;
            let taffy = tooltip_taffy(UiRect::new(ax, ay, 0.0, 0.0), vp, tw, th);
            assert_eq!(
                (taffy.x, taffy.y, taffy.w, taffy.h),
                (native.x, native.y, native.w, native.h),
                "parity тултипа: anchor=({ax},{ay}) size=({tw},{th})"
            );
        }
    }
    /// Документированное расхождение (taffy-транспорт): `compute_layout`
    /// taffy округляет позиции к целому ui px (round_layout) — дробная
    /// позиция кандидата (якорь .5) даёт ±0.5 ui px к native (native
    /// арифметику не округляет). Фиксируется, чтобы расхождение было
    /// видимым; parity-матрица выше — на целых входах.
    #[test]
    fn tooltip_taffy_fractional_position_is_documented_divergence() {
        let vp = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let native = tooltip(
            UiPoint::new(100.5, 200.5),
            UiVec2::new(60.0, 16.0),
            vp,
            TOOLTIP_DELAY_MS,
            TOOLTIP_DELAY_MS,
        )
        .unwrap()
        .rect;
        assert_eq!(native, UiRect::new(114.5, 218.5, 60.0, 16.0));
        let taffy = tooltip_taffy(UiRect::new(100.5, 200.5, 0.0, 0.0), vp, 60.0, 16.0);
        // taffy: округление half-away-from-zero к целому ui px
        assert_eq!(
            (taffy.x, taffy.y, taffy.w, taffy.h),
            (115.0, 219.0, 60.0, 16.0),
            "round_layout taffy: (114.5, 218.5) → (115, 219)"
        );
    }
    /// toast: parity с native [`toast_area`] — ПОБИТОВО: без avoid,
    /// с what-if баром (CR-016), смещённый вьюпорт, подъём у верхнего
    /// края, узкий вьюпорт (ширина 0 — пустой rect возвращается как
    /// есть).
    #[test]
    fn toast_area_taffy_parity_with_native() {
        let cases: [(UiRect, Option<UiRect>); 5] = [
            (UiRect::new(0.0, 0.0, 800.0, 600.0), None),
            (
                UiRect::new(0.0, 0.0, 800.0, 600.0),
                Some(UiRect::new(0.0, 540.0, 800.0, 596.0)),
            ),
            (UiRect::new(100.0, 0.0, 300.0, 600.0), None),
            (
                UiRect::new(0.0, 0.0, 800.0, 100.0),
                Some(UiRect::new(0.0, 10.0, 800.0, 90.0)),
            ),
            (UiRect::new(0.0, 0.0, 60.0, 600.0), None),
        ];
        for (vp, avoid) in cases {
            let native = toast_area(vp, avoid);
            let taffy = toast_area_taffy(vp, avoid);
            assert_eq!(
                (taffy.x, taffy.y, taffy.w, taffy.h),
                (native.x, native.y, native.w, native.h),
                "parity тоста: vp={vp:?} avoid={avoid:?}"
            );
        }
    }
}
