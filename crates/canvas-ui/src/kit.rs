//! FR-055 (этап U4 PRD-0009, F-8): UI kit v1 — чистая модель виджетов поверх
//! примитивов U3. Кит НЕ рисует и ввод не перехватывает (слой/модальность —
//! только из реестра поверхностей): компоненты возвращают геометрию и стиль,
//! потребитель кладёт их в полосу своего слоя (адаптер в `canvas-app`).
//!
//! Контракты F-8 (PRD-0009 §8):
//! - **цвета — только слоты**: [`KitPalette`] — срез слотов `ThemeColors` v2
//!   (+ слоты состояний FR-053); маппинг из рендера в крейте-потребителе
//!   (`canvas-render` зависит от `canvas-ui`, не наоборот — G7). Кит не знает
//!   конкретных цветов — ни одной константы цвета в этом модуле;
//! - **отступы/радиусы — только шкала токенов** (`canvas_core::tokens`:
//!   SPACING_*, RADIUS_*); значения совпадают с прежними константами
//!   поверхностей — ноль визуального скачка (I-1 FR-046);
//! - **текст — только через `TextMeasurer`** (F-6): ширины — фактический
//!   шейпинг, усечение — `ellipsis` (класс дефекта CR-015 запрещён);
//! - паттерн Popup/Tooltip «якорь + flip при нехватке места + delay» —
//!   L3 из карты переносов egui (Приложение А FR-051).
//!
//! Состояния виджетов — [`KitState`]; стили — [`ControlStyle`]/[`PanelStyle`],
//! собранные ТОЛЬКО из слотов палитры (проверяется тестом: смена слота меняет
//! стиль, никакой скрытой арифметики над цветами).

use crate::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use crate::layout::{constrain, stack, HAlign, VAlign};
use crate::measure::TextMeasurer;

// --- Состояния и стили ------------------------------------------------------

/// Состояние интерактивного виджета кита (слоты состояний палитры — FR-053).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KitState {
    /// Обычное.
    Normal,
    /// Курсор над виджетом.
    Hovered,
    /// Кнопка зажата.
    Pressed,
    /// Недоступен (клик игнорируется потребителем).
    Disabled,
    /// Выбран (тоглы, строки списков, чипы фильтров).
    Selected,
}

/// Вариант кнопки (семантика действия, не цвет: цвет — из слотов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Главное действие модали/панели.
    Primary,
    /// Второстепенное действие.
    Secondary,
    /// Призрачная кнопка (иконка/текст без заливки) — хром, не акция.
    Ghost,
    /// Разрушающее действие (удаление/сброс).
    Danger,
}

/// Стиль интерактивного контрола — только слоты палитры + радиус шкалы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub text: [f32; 4],
    /// Радиус из radius-scale (chip/card/panel/pill).
    pub radius: f32,
}

/// Стиль панели (контейнер поверхности) — только слоты + шкала.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub radius: f32,
    /// Внутренние отступы из spacing-scale.
    pub pad: EdgeInsets,
}

// --- Палитра-срез (контракт «цвета — только слоты») -------------------------

/// Срез слотов палитры дизайн-системы для кита. Заполняется из
/// `canvas-render::theme::ThemeColors` (v2 + слоты состояний FR-053) —
/// impl в крейте рендера; кит остаётся без конкретных цветов.
///
/// Инвариант стиля: [`Button::style`] и др. выбирают СЛОТ по состоянию,
/// но не вычисляют новых цветов (без умножений/смешиваний).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KitPalette {
    /// Заливка панели/модали (`menu_fill`).
    pub panel_fill: [f32; 4],
    /// Рамка панели/модали (`palette_border`).
    pub panel_border: [f32; 4],
    /// Заливка secondary/ghost-контрола (`palette_chip_fill`).
    pub control_fill: [f32; 4],
    /// Рамка контрола (`DIALOG_BUTTON_BORDER` — слот `control_border`).
    pub control_border: [f32; 4],
    /// Заливка primary (`DIALOG_BUTTON_PRIMARY` — слот `control_primary`).
    pub control_primary: [f32; 4],
    /// Заливка danger (`error`, alpha 1.0 — слот `control_danger`).
    pub control_danger: [f32; 4],
    /// Hover вторичной строки/кнопки (слот состояний `control_hover_fill`).
    pub hover_fill: [f32; 4],
    /// Hover primary (слот состояний `control_primary_hover_fill`).
    pub primary_hover_fill: [f32; 4],
    /// Выбранная строка/чип (слот состояний `control_selected_fill`).
    pub selected_fill: [f32; 4],
    /// Основной текст (`body`).
    pub text: [f32; 4],
    /// Заголовки (`title`).
    pub text_title: [f32; 4],
    /// Приглушённый текст (`DIALOG_TEXT_MUTED` — слот `text_muted`).
    pub text_muted: [f32; 4],
    /// Текст disabled (слот состояний `control_disabled_text`).
    pub disabled_text: [f32; 4],
    /// Акцент (фокус-рамка, Ghost-hover).
    pub accent: [f32; 4],
}

// --- Метрики кита (spacing/radius-scale; значения — прежние константы) ------

/// Высота кнопки — прежняя высота кнопок диалога T21.
pub const BUTTON_HEIGHT: f32 = 30.0;
/// Горизонтальный пад кнопки: SPACING_LG (12) — прежний пад кнопок диалога.
pub const BUTTON_PAD_H: f32 = 12.0;
/// Сторона квадратной icon-кнопки (угловые кнопки ⚙/?).
pub const ICON_BUTTON_SIZE: f32 = 26.0;
/// Высота чипа (категории палитры — прежняя высота чипов).
pub const CHIP_HEIGHT: f32 = 24.0;
/// Горизонтальный пад чипа: SPACING_SM (8) — прежний пад чипов палитры.
pub const CHIP_PAD_H: f32 = 8.0;
/// Зазор между контролами в ряду: SPACING_SM.
pub const GAP_CONTROLS: f32 = 8.0;
/// TTL тоста (T21-A: 3 с).
pub const TOAST_TTL_MS: u64 = 3000;
/// Задержка показа тултипа (паттерн egui tooltip: delay 500 мс).
pub const TOOLTIP_DELAY_MS: u32 = 500;
/// Отступ тултипа от точки якоря (прежние +14/+18 тултипов канваса).
pub const TOOLTIP_OFFSET: UiPoint = UiPoint::new(14.0, 18.0);
/// Зазор dropdown от якоря.
pub const DROPDOWN_GAP: f32 = 4.0;

// --- Panel ------------------------------------------------------------------

/// Стиль панели: заливка/рамка `panel_*`, радиус RADIUS_PANEL, пад SPACING_LG.
pub fn panel_style(p: &KitPalette) -> PanelStyle {
    PanelStyle {
        fill: p.panel_fill,
        border: p.panel_border,
        radius: canvas_core::tokens::RADIUS_PANEL,
        pad: EdgeInsets::uniform(canvas_core::tokens::SPACING_LG),
    }
}

/// Панель-контейнер: размер = constrain(min,max,desired) в слоте, позиция =
/// stack (выравнивание задаёт потребитель). Возвращает rect панели.
pub fn panel_rect(slot: UiRect, min: UiVec2, max: UiVec2, desired: UiVec2) -> UiRect {
    let size = constrain(min, max, desired);
    stack(slot, size, HAlign::Center, VAlign::Center)
}

/// Внутренняя область контента панели (минус пад стиля).
pub fn panel_content(panel: UiRect, style: &PanelStyle) -> UiRect {
    panel.inset(&style.pad)
}

// --- Button -----------------------------------------------------------------

/// Замер кнопки по подписи: ширина = текст + 2·BUTTON_PAD_H, высота = const.
/// Пустая подпись → квадрат минимальной ширины BUTTON_HEIGHT.
pub fn button_size(
    label: &str,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> UiVec2 {
    let text_w = if label.is_empty() {
        0.0
    } else {
        m.width_of(fs, label, family, size)
    };
    let w = (text_w + BUTTON_PAD_H * 2.0).max(BUTTON_HEIGHT);
    UiVec2::new(w, BUTTON_HEIGHT)
}

/// Кнопка в слоте: измеренный размер, позиция — stack по выравниванию;
/// подпись усекается `ellipsis`, если слот уже кнопки (деградация видна
/// потребителю и линту, не молчаливый break).
pub struct ButtonLayout {
    /// Rect кнопки.
    pub rect: UiRect,
    /// Подпись после ellipsis (может отличаться от исходной).
    pub label: String,
}

pub fn button_layout(
    slot: UiRect,
    label: &str,
    align: (HAlign, VAlign),
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> ButtonLayout {
    let desired = button_size(label, m, fs, family, size);
    let max_w = slot.w;
    // Деградация узкого слота: усечение по фактической ширине (не срез);
    // кнопка не вылезает за слот — сжимается до его ширины.
    let width = desired.x.min(max_w);
    let shown = if desired.x > max_w {
        m.ellipsis(
            fs,
            label,
            family,
            size,
            (max_w - BUTTON_PAD_H * 2.0).max(0.0),
        )
    } else {
        label.to_owned()
    };
    let rect = stack(slot, UiVec2::new(width, BUTTON_HEIGHT), align.0, align.1);
    ButtonLayout { rect, label: shown }
}

/// Стиль кнопки: слот заливки по варианту, слот hover/pressed по состоянию,
/// текст — слот по состоянию (disabled — свой слот). Никакой арифметики
/// над цветами: только выбор слота.
pub fn button_style(variant: ButtonVariant, state: KitState, p: &KitPalette) -> ControlStyle {
    let base_fill = match variant {
        ButtonVariant::Primary => p.control_primary,
        ButtonVariant::Danger => p.control_danger,
        ButtonVariant::Secondary | ButtonVariant::Ghost => p.control_fill,
    };
    let fill = match state {
        KitState::Normal | KitState::Selected => base_fill,
        KitState::Hovered => match variant {
            ButtonVariant::Primary => p.primary_hover_fill,
            ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Danger => p.hover_fill,
        },
        KitState::Pressed => match variant {
            ButtonVariant::Primary => p.primary_hover_fill,
            _ => p.hover_fill,
        },
        KitState::Disabled => base_fill,
    };
    let border = match (variant, state) {
        (ButtonVariant::Ghost, KitState::Hovered | KitState::Pressed) => p.accent,
        (ButtonVariant::Ghost, _) => [0.0, 0.0, 0.0, 0.0],
        _ => p.control_border,
    };
    let text = match state {
        KitState::Disabled => p.disabled_text,
        _ => match variant {
            ButtonVariant::Ghost => p.text,
            _ => p.text_title,
        },
    };
    ControlStyle {
        fill,
        border,
        text,
        radius: canvas_core::tokens::RADIUS_CHIP,
    }
}

// --- IconButton -------------------------------------------------------------

/// Квадратная кнопка в слоте (угловые кнопки, ✕ модалей).
pub fn icon_button_rect(slot: UiRect, align: (HAlign, VAlign)) -> UiRect {
    stack(
        slot,
        UiVec2::new(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE),
        align.0,
        align.1,
    )
}

/// Стиль icon-кнопки = Ghost-кнопка (тот же слот hover, без заливки в Normal).
pub fn icon_button_style(state: KitState, p: &KitPalette) -> ControlStyle {
    button_style(ButtonVariant::Ghost, state, p)
}

// --- Chip -------------------------------------------------------------------

/// Замер чипа: ширина = текст + 2·CHIP_PAD_H, высота = const.
pub fn chip_size(
    label: &str,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> UiVec2 {
    let text_w = m.width_of(fs, label, family, size);
    UiVec2::new(text_w + CHIP_PAD_H * 2.0, CHIP_HEIGHT)
}

/// Чип от левого края `origin_x` (ряд чипов компонует потребитель через
/// `Row`/SqueezeTail); усечение подписи — ellipsis по `max_width`.
pub struct ChipLayout {
    pub rect: UiRect,
    pub label: String,
}

pub fn chip_layout(
    origin: UiPoint,
    label: &str,
    max_width: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> ChipLayout {
    let desired = chip_size(label, m, fs, family, size);
    let shown = if desired.x > max_width {
        m.ellipsis(
            fs,
            label,
            family,
            size,
            (max_width - CHIP_PAD_H * 2.0).max(0.0),
        )
    } else {
        label.to_owned()
    };
    let w = if shown == label { desired.x } else { max_width };
    ChipLayout {
        rect: UiRect::new(origin.x, origin.y, w, CHIP_HEIGHT),
        label: shown,
    }
}

/// Стиль чипа: selected → слот selected_fill, hovered → hover_fill,
/// disabled → текст disabled. Радиус RADIUS_CHIP.
pub fn chip_style(state: KitState, p: &KitPalette) -> ControlStyle {
    let fill = match state {
        KitState::Selected => p.selected_fill,
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => p.control_fill,
    };
    ControlStyle {
        fill,
        border: p.control_border,
        text: match state {
            KitState::Disabled => p.disabled_text,
            _ => p.text,
        },
        radius: canvas_core::tokens::RADIUS_CHIP,
    }
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
    // Горизонталь: левый край якоря, зажат во вьюпорт
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
        menu: UiRect::new(x, y, width, content.y),
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
    UiRect::new(40.0, y, width, 20.0)
}

// --- Modal ------------------------------------------------------------------

/// Раскладка модали: затемнение = весь слот, панель = constrain+stack
/// (центр). Слой/модальность — только из реестра (поверхность Modals/Block).
pub struct ModalLayout {
    pub dim: UiRect,
    pub panel: UiRect,
}

pub fn modal(slot: UiRect, min: UiVec2, max: UiVec2, desired: UiVec2) -> ModalLayout {
    ModalLayout {
        dim: slot,
        panel: panel_rect(slot, min, max, desired),
    }
}

/// Стиль модали = стиль панели (тот же слот заливки/рамки).
pub fn modal_style(p: &KitPalette) -> PanelStyle {
    panel_style(p)
}

// --- Явные слоты (миграция пилотов: I-1 ноль скачка) ------------------------

/// Стиль панели из ЯВНЫХ слотов темы поверхности. Контракт F-8 сохраняется:
/// аргументы — значения слотов `ThemeColors` (потребитель передаёт слот, кит
/// не изобретает цветов); радиус — из radius-scale/каноническая константа
/// поверхности (миграция I-1: ноль визуального скачка — приоритет над
/// унификацией радиусов, унификация — v2 с токен-паритетом).
pub fn panel_style_of(fill: [f32; 4], border: [f32; 4], radius: f32, pad: f32) -> PanelStyle {
    PanelStyle {
        fill,
        border,
        radius,
        pad: EdgeInsets::uniform(pad),
    }
}

/// Стиль контрола из ЯВНЫХ слотов (см. [`panel_style_of`]).
pub fn control_style_of(
    fill: [f32; 4],
    border: [f32; 4],
    text: [f32; 4],
    radius: f32,
) -> ControlStyle {
    ControlStyle {
        fill,
        border,
        text,
        radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::TextMeasurer;

    const FAMILY: &str = "Noto Sans Display";

    /// Палитра-двойка: два разных значения КАЖДОГО слота — тест «стиль
    /// выбирает слот, а не вычисляет цвет».
    fn palette_a() -> KitPalette {
        KitPalette {
            panel_fill: [0.1, 0.1, 0.1, 1.0],
            panel_border: [0.2, 0.2, 0.2, 1.0],
            control_fill: [0.3, 0.3, 0.3, 1.0],
            control_border: [0.4, 0.4, 0.4, 1.0],
            control_primary: [0.5, 0.5, 0.5, 1.0],
            control_danger: [0.6, 0.6, 0.6, 1.0],
            hover_fill: [0.7, 0.7, 0.7, 1.0],
            primary_hover_fill: [0.75, 0.75, 0.75, 1.0],
            selected_fill: [0.8, 0.8, 0.8, 1.0],
            text: [0.9, 0.9, 0.9, 1.0],
            text_title: [0.91, 0.91, 0.91, 1.0],
            text_muted: [0.92, 0.92, 0.92, 1.0],
            disabled_text: [0.93, 0.93, 0.93, 1.0],
            accent: [0.94, 0.94, 0.94, 1.0],
        }
    }

    fn palette_b() -> KitPalette {
        let mut p = palette_a();
        for v in [
            &mut p.panel_fill,
            &mut p.panel_border,
            &mut p.control_fill,
            &mut p.control_border,
            &mut p.control_primary,
            &mut p.control_danger,
            &mut p.hover_fill,
            &mut p.primary_hover_fill,
            &mut p.selected_fill,
            &mut p.text,
            &mut p.text_title,
            &mut p.text_muted,
            &mut p.disabled_text,
            &mut p.accent,
        ] {
            *v = [0.05, 0.05, 0.05, 0.5];
        }
        p
    }

    #[test]
    fn button_measure_is_text_plus_padding() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let w = button_size("ОО", &mut m, &mut fs, FAMILY, 13.0).x;
        let text_w = m.width_of(&mut fs, "ОО", FAMILY, 13.0);
        assert!((w - (text_w + BUTTON_PAD_H * 2.0)).abs() < 0.01);
        // Пустая подпись — минимальная квадратная кнопка
        assert_eq!(
            button_size("", &mut m, &mut fs, FAMILY, 13.0).x,
            BUTTON_HEIGHT
        );
        assert_eq!(
            button_size("", &mut m, &mut fs, FAMILY, 13.0).y,
            BUTTON_HEIGHT
        );
    }

    #[test]
    fn button_in_narrow_slot_ellipsizes() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 40.0, BUTTON_HEIGHT);
        let layout = button_layout(
            slot,
            "Длинная подпись кнопки",
            (HAlign::Center, VAlign::Center),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(layout.label != "Длинная подпись кнопки");
        assert!(layout.rect.w <= slot.w + 0.01);
        // Широкий слот — подпись целиком
        let wide = UiRect::new(0.0, 0.0, 400.0, BUTTON_HEIGHT);
        let layout = button_layout(
            wide,
            "Длинная подпись кнопки",
            (HAlign::Center, VAlign::Center),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(layout.label, "Длинная подпись кнопки");
    }

    /// Контракт F-8: стиль собирается ТОЛЬКО выбором слота палитры — смена
    /// палитры меняет стиль; состояния берут РАЗНЫЕ слоты.
    #[test]
    fn styles_use_palette_slots_only() {
        let a = palette_a();
        let b = palette_b();
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Ghost,
            ButtonVariant::Danger,
        ] {
            let sa = button_style(variant, KitState::Normal, &a);
            let sb = button_style(variant, KitState::Normal, &b);
            assert_ne!(sa.fill, sb.fill, "{variant:?}: fill не из слота");
            assert_ne!(sa.text, sb.text, "{variant:?}: text не из слота");
        }
        // Состояния: hover/pressed/disabled — свои слоты
        let normal = button_style(ButtonVariant::Secondary, KitState::Normal, &a);
        let hovered = button_style(ButtonVariant::Secondary, KitState::Hovered, &a);
        assert_eq!(hovered.fill, a.hover_fill);
        assert_ne!(normal.fill, hovered.fill);
        let disabled = button_style(ButtonVariant::Secondary, KitState::Disabled, &a);
        assert_eq!(disabled.text, a.disabled_text);
        // Ghost в Normal — без рамки
        let ghost = button_style(ButtonVariant::Ghost, KitState::Normal, &a);
        assert_eq!(ghost.border[3], 0.0);
        // Чип: selected — слот selected
        assert_eq!(chip_style(KitState::Selected, &a).fill, a.selected_fill);
        // Радиусы — из radius-scale
        assert_eq!(normal.radius, canvas_core::tokens::RADIUS_CHIP);
        assert_eq!(panel_style(&a).radius, canvas_core::tokens::RADIUS_PANEL);
    }

    #[test]
    fn panel_and_modal_are_centered_and_clamped() {
        let vp = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let panel = panel_rect(
            vp,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(600.0, 400.0),
            UiVec2::new(900.0, 500.0),
        );
        // desired зажат max → 600×400, центр вьюпорта
        assert!((panel.w - 600.0).abs() < 0.01);
        assert!((panel.x - (1280.0 - 600.0) / 2.0).abs() < 0.01);
        let modal = modal(
            vp,
            UiVec2::new(280.0, 150.0),
            UiVec2::new(440.0, 150.0),
            UiVec2::new(440.0, 150.0),
        );
        assert!((modal.panel.x - (1280.0 - 440.0) / 2.0).abs() < 0.01);
        assert_eq!(modal.dim, vp);
        // Контент панели минус пад
        let style = panel_style(&palette_a());
        let content = panel_content(panel, &style);
        assert!((content.x - (panel.x + style.pad.left)).abs() < 0.01);
    }

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

    #[test]
    fn chip_measures_and_ellipsizes() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let c = chip_layout(
            UiPoint::new(10.0, 20.0),
            "Категория",
            f32::INFINITY,
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        let text_w = m.width_of(&mut fs, "Категория", FAMILY, 13.0);
        assert!((c.rect.w - (text_w + CHIP_PAD_H * 2.0)).abs() < 0.01);
        assert_eq!(c.rect.h, CHIP_HEIGHT);
        let c = chip_layout(
            UiPoint::new(10.0, 20.0),
            "Очень длинная категория чипа",
            60.0,
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(c.label != "Очень длинная категория чипа");
        assert!(c.rect.w <= 60.0 + 0.01);
    }

    #[test]
    fn icon_button_is_square_in_slot() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let r = icon_button_rect(slot, (HAlign::End, VAlign::Start));
        assert_eq!(r.w, ICON_BUTTON_SIZE);
        assert_eq!(r.h, ICON_BUTTON_SIZE);
        assert!((r.right() - slot.right()).abs() < 0.01);
        assert!((r.y - slot.y).abs() < 0.01);
    }
}
