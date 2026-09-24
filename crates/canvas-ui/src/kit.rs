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
use crate::paint::{PaintAlign, Painter};
use crate::row_guides::{RowCellWidths, RowGuides};

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

// --- Метрики v2 (FR-058) ----------------------------------------------------

/// Высота текстового поля — прежняя высота полей поиска/фильтра пилотов.
pub const TEXT_FIELD_HEIGHT: f32 = 30.0;
/// Горизонтальный пад текстового поля: SPACING_SM (8).
pub const TEXT_FIELD_PAD_H: f32 = 8.0;
/// Минимальная ширина текстового поля.
pub const TEXT_FIELD_MIN_W: f32 = 80.0;

/// Высота строки списка (подсказки/flowmap/calc_panel) — без зазора.
pub const LIST_ROW_H: f32 = 26.0;
/// Зазор между строками списка: SPACING_S (6).
pub const LIST_ROW_GAP: f32 = 6.0;

/// Ширина трека скроллбара (узкая полоса у правого края).
pub const SCROLLBAR_WIDTH: f32 = 4.0;
/// Минимальная высота бегунка скроллбара (чтобы оставался захватимым).
pub const SCROLLBAR_KNOB_MIN: f32 = 20.0;

/// Ширина трека Switch (pill). Соразмерна высоте кнопки (BUTTON_HEIGHT).
pub const SWITCH_W: f32 = 36.0;
/// Высота трека Switch.
pub const SWITCH_H: f32 = 20.0;
/// Внутренний пад Switch: отступ бегунка от краёв трека.
pub const SWITCH_KNOB_PAD: f32 = 2.0;

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

// === FR-058: Компоненты кита v2 ============================================
//
// Чистые модели/функции в стиле kit v1: геометрия + стиль + модель состояния;
// рисование — через Painter (FR-057), ввод не перехватывают, событий не владеют.
//
// Инварианты (замороженные контракты FR-059/060 кодируют против них):
// - каретка/селекция `TextFieldModel` — в СИМВОЛАХ (`chars().count()`), не
//   байтах; IME/UTF-16-конвертация — на стороне ввода потребителя;
// - `list_rows` — чистая функция (без мутаций `ScrollState`);
// - `switch`/`card` — только геометрия и слоты (цвет — отдельной функцией);
// - 0 новых внешних зависимостей (G7); Slider НЕ включён (спекулятивный
//   компонент без потребителя — вернуть в постановку при появлении экрана
//   со слайдером).

// --- TextField --------------------------------------------------------------

/// Модель текстового поля: текст + каретка + селекция. Позиции — в СИМВОЛАХ
/// (`chars().count()`), не байтах: вставка/удаление/движение корректны на
/// юникоде (emoji, multi-byte). IME/UTF-16-конвертация — на стороне ввода
/// потребителя (контракт FR-058).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFieldModel {
    /// Текст поля.
    pub text: String,
    /// Позиция каретки в СИМВОЛАХ (`chars().count()` от начала).
    pub caret: usize,
    /// Селекция `(anchor, head)` в символах; `None` — нет селекции.
    /// `head` — текущая позиция каретки; `anchor` — начало выделения.
    pub sel: Option<(usize, usize)>,
}

impl TextFieldModel {
    /// Вставить строку на месте каретки (или заменить селекцию).
    pub fn insert(&mut self, s: &str) {
        let (start, end) = self.selection_range();
        let chars: Vec<char> = self.text.chars().collect();
        let insert_chars: Vec<char> = s.chars().collect();
        let mut new_chars: Vec<char> =
            Vec::with_capacity(chars.len() + insert_chars.len() - (end - start));
        new_chars.extend_from_slice(&chars[..start]);
        new_chars.extend_from_slice(&insert_chars);
        new_chars.extend_from_slice(&chars[end..]);
        self.text = new_chars.into_iter().collect();
        self.caret = start + insert_chars.len();
        self.sel = None;
    }

    /// Backspace: удалить символ перед кареткой (или селекцию).
    pub fn backspace(&mut self) {
        if self.sel.is_some() {
            self.delete_selection();
            return;
        }
        if self.caret == 0 {
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        chars.remove(self.caret - 1);
        self.text = chars.into_iter().collect();
        self.caret -= 1;
    }

    /// Delete: удалить символ после каретки (или селекцию).
    pub fn delete(&mut self) {
        if self.sel.is_some() {
            self.delete_selection();
            return;
        }
        let total = self.text.chars().count();
        if self.caret >= total {
            return;
        }
        let mut chars: Vec<char> = self.text.chars().collect();
        chars.remove(self.caret);
        self.text = chars.into_iter().collect();
    }

    /// Сдвинуть каретку на `chars` символов (отрицательное — влево).
    /// `extend = true` — расширять селекцию (shift+стрелки).
    pub fn move_caret(&mut self, chars: isize, extend: bool) {
        let total = self.text.chars().count();
        let new_pos = (self.caret as isize + chars).max(0).min(total as isize) as usize;
        if extend {
            match self.sel {
                None => self.sel = Some((self.caret, new_pos)),
                Some((anchor, _)) => self.sel = Some((anchor, new_pos)),
            }
        } else {
            self.sel = None;
        }
        self.caret = new_pos;
    }

    /// Выделить весь текст.
    pub fn select_all(&mut self) {
        let total = self.text.chars().count();
        self.sel = Some((0, total));
        self.caret = total;
    }

    /// Снять выделение (каретка остаётся на месте).
    pub fn clear_selection(&mut self) {
        self.sel = None;
    }

    /// Заменить текст целиком; каретка — в конец, селекция снята.
    pub fn set_text(&mut self, s: String) {
        let total = s.chars().count();
        self.text = s;
        self.caret = total;
        self.sel = None;
    }

    /// Диапазон удаления (start, end) в символах: селекция или пустая каретка.
    fn selection_range(&self) -> (usize, usize) {
        match self.sel {
            Some((a, b)) => (a.min(b), a.max(b)),
            None => (self.caret, self.caret),
        }
    }

    /// Удалить выделенный диапазон (приватный — публично через backspace/delete).
    fn delete_selection(&mut self) {
        let (start, end) = self.selection_range();
        let chars: Vec<char> = self.text.chars().collect();
        let new_chars: Vec<char> = chars[..start]
            .iter()
            .chain(&chars[end..])
            .copied()
            .collect();
        self.text = new_chars.into_iter().collect();
        self.caret = start;
        self.sel = None;
    }
}

/// Раскладка текстового поля. `caret_x = -1.0` — каретка не рисуется (поле не
/// в фокусе); иначе — x-координата каретки в `text_area` по замеру префикса.
#[derive(Debug, Clone, PartialEq)]
pub struct TextFieldLayout {
    /// Rect поля (constrain+stack в слоте).
    pub rect: UiRect,
    /// Внутренняя область текста (минус горизонтальный пад).
    pub text_area: UiRect,
    /// X каретки в `text_area` (по замеру текста до каретки); `-1.0` — нет каретки.
    pub caret_x: f32,
    /// Отображаемый текст: placeholder (если пусто) или сам текст — с `ellipsis`
    /// по ширине `text_area`.
    pub text_shown: String,
}

/// Текстовое поле в слоте. Стиль (фон/рамка/фокус-рамка) — отдельной функцией
/// (`control_style_of`/`button_style` — потребитель красит через Painter);
/// контракт: цвет отдельно от геометрии. `focused` управляет видимостью
/// каретки; `state`/`p` зарезервированы для будущих расширений стиля.
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    slot: UiRect,
    min: UiVec2,
    max: UiVec2,
    model: &TextFieldModel,
    placeholder: &str,
    focused: bool,
    _state: KitState,
    _p: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> TextFieldLayout {
    // Стиль — отдельной функцией потребителя (цвет отдельно от геометрии).
    let _ = (_state, _p);
    // Размер: constrain(min, max, desired=slot), позиция — stack по центру.
    let desired = max;
    let sz = constrain(min, max, desired);
    let rect = stack(slot, sz, HAlign::Center, VAlign::Center);
    // Пад: SPACING_SM горизонтально; вертикаль — вся высота rect.
    let text_area = UiRect::new(
        rect.x + TEXT_FIELD_PAD_H,
        rect.y,
        (rect.w - TEXT_FIELD_PAD_H * 2.0).max(0.0),
        rect.h,
    );
    // Текст/плейсхолдер: пустое → placeholder с ellipsis; иначе текст с ellipsis.
    // Каретка: по замеру префикса до caret в ИСХОДНОМ тексте, клампленный к
    // text_area; -1.0 — не сфокусировано (потребитель не рисует каретку).
    let (text_shown, caret_x) = if model.text.is_empty() {
        let ph = if placeholder.is_empty() {
            String::new()
        } else {
            m.ellipsis(fs, placeholder, family, size, text_area.w)
        };
        let cx = if focused { text_area.x } else { -1.0 };
        (ph, cx)
    } else {
        let shown = m.ellipsis(fs, &model.text, family, size, text_area.w);
        let chars: Vec<char> = model.text.chars().collect();
        let caret_idx = model.caret.min(chars.len());
        let prefix: String = chars[..caret_idx].iter().collect();
        let prefix_w = m.width_of(fs, &prefix, family, size);
        let cx = if focused {
            text_area.x + prefix_w.min(text_area.w)
        } else {
            -1.0
        };
        (shown, cx)
    };
    TextFieldLayout {
        rect,
        text_area,
        caret_x,
        text_shown,
    }
}

// --- Список + скролл ---------------------------------------------------------

/// Состояние скролла списка: `offset` — сдвиг контента вверх (px),
/// `content_h`/`viewport_h` — высота контента и окна видимости.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScrollState {
    /// Сдвиг контента вверх от начала (px); 0 — начало списка.
    pub offset: f32,
    /// Полная высота контента (сумма высот всех строк + зазоры).
    pub content_h: f32,
    /// Высота окна видимости (слота списка).
    pub viewport_h: f32,
}

impl ScrollState {
    /// Сдвинуть скролл на `dy` px (отрицательное — вверх). Без клампа (вызовите
    /// [`clamp`](Self::clamp) после, если нужно удержать в границах).
    pub fn scroll_by(&mut self, dy: f32) {
        self.offset = (self.offset + dy).max(0.0);
    }

    /// Зажать `offset` в `[0, max_offset]`.
    pub fn clamp(&mut self) {
        let max = self.max_offset();
        if self.offset > max {
            self.offset = max;
        }
        if self.offset < 0.0 {
            self.offset = 0.0;
        }
    }

    /// Нужен ли скролл (контент выше вьюпорта).
    pub fn needs_scroll(&self) -> bool {
        self.content_h > self.viewport_h
    }

    /// Максимальный сдвиг: `content_h - viewport_h`, не меньше 0.
    pub fn max_offset(&self) -> f32 {
        (self.content_h - self.viewport_h).max(0.0)
    }
}

/// Видимые строки списка: `(индекс, экранный rect)`. Чистая функция — без
/// мутаций `ScrollState`. Строка видима, если её низ ниже верха вьюпорта и
/// верх ниже низа вьюпорта (частичные строки на краях включаются).
pub fn list_rows(
    area: UiRect,
    s: &ScrollState,
    row_h: f32,
    gap: f32,
    count: usize,
) -> Vec<(usize, UiRect)> {
    if row_h <= 0.0 || count == 0 || s.viewport_h <= 0.0 {
        return Vec::new();
    }
    let stride = row_h + gap;
    // Первая видимая строка: наименьшее i, где низ (i*stride + row_h) > offset.
    // i > (offset - row_h) / stride. Учитываем частичную строку сверху.
    let first = (((s.offset - row_h) / stride).max(0.0).ceil() as usize).min(count);
    // Последняя видимая (включительно): наибольшее i, где верх (i*stride) <
    // offset + viewport_h. i < (offset + viewport_h) / stride.
    let last_inclusive = (((s.offset + s.viewport_h) / stride).ceil() as usize)
        .saturating_sub(1)
        .min(count.saturating_sub(1));
    let mut out = Vec::new();
    for i in first..=last_inclusive {
        let content_y = i as f32 * stride;
        let screen_y = area.y + content_y - s.offset;
        out.push((i, UiRect::new(area.x, screen_y, area.w, row_h)));
    }
    out
}

/// Бегунок скроллбара — только когда `needs_scroll`. Возвращает rect бегунка
/// (трек = правая полоса `area` шириной [`SCROLLBAR_WIDTH`]); `None` — скролл
/// не нужен. Цвет — на потребителе (слот `control_border`/`text_muted`).
pub fn scroll_bar(area: UiRect, s: &ScrollState, _p: &KitPalette) -> Option<UiRect> {
    if !s.needs_scroll() {
        return None;
    }
    let max = s.max_offset();
    if max <= 0.0 {
        return None;
    }
    let track_x = area.right() - SCROLLBAR_WIDTH;
    let track_h = area.h;
    // Бегунок: высота ∝ viewport/content, минимум SCROLLBAR_KNOB_MIN.
    let ratio = (s.viewport_h / s.content_h).clamp(0.0, 1.0);
    let knob_h = (track_h * ratio).max(SCROLLBAR_KNOB_MIN).min(track_h);
    // Позиция: 0 → верх, max → низ.
    let pos_ratio = (s.offset / max).clamp(0.0, 1.0);
    let knob_y = area.y + (track_h - knob_h) * pos_ratio;
    Some(UiRect::new(track_x, knob_y, SCROLLBAR_WIDTH, knob_h))
}

// --- Switch -----------------------------------------------------------------

/// Раскладка Switch (тогла): трек (pill) + бегунок + стиль трека + заливка
/// бегунка. `on` определяет позицию бегунка (вправо) и слот заливки трека.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchLayout {
    /// Rect трека (pill).
    pub track: UiRect,
    /// Rect бегунка (квадрат внутри трека).
    pub knob: UiRect,
    /// Стиль трека — слот заливки/рамки/радиуса (RADIUS_PILL).
    pub track_style: ControlStyle,
    /// Заливка бегунка (слот `text_title`/`disabled_text`).
    pub knob_fill: [f32; 4],
}

/// Switch в слоте: трек (pill) + бегунок. `on` — позиция бегунка и слот
/// заливки трека (`control_primary` on / `control_fill` off); `state` —
/// hover/pressed/disabled слоты; `knob_fill` — `text_title` (disabled —
/// `disabled_text`).
pub fn switch(slot: UiRect, on: bool, state: KitState, p: &KitPalette) -> SwitchLayout {
    let track = stack(
        slot,
        UiVec2::new(SWITCH_W, SWITCH_H),
        HAlign::Center,
        VAlign::Center,
    );
    // Бегунок: квадрат side = SWITCH_H - 2·pad; позиция — влево/вправо по `on`.
    let knob_side = (SWITCH_H - 2.0 * SWITCH_KNOB_PAD).max(0.0);
    let knob_x = if on {
        track.right() - SWITCH_KNOB_PAD - knob_side
    } else {
        track.x + SWITCH_KNOB_PAD
    };
    let knob_y = track.y + SWITCH_KNOB_PAD;
    let knob = UiRect::new(knob_x, knob_y, knob_side, knob_side);
    // Стиль трека: заливка — primary on / control_fill off; hover/pressed —
    // свои слоты; disabled — базовая. Радиус RADIUS_PILL (pill).
    let base_fill = if on {
        p.control_primary
    } else {
        p.control_fill
    };
    let fill = match state {
        KitState::Hovered | KitState::Pressed => {
            if on {
                p.primary_hover_fill
            } else {
                p.hover_fill
            }
        }
        _ => base_fill,
    };
    let track_style = ControlStyle {
        fill,
        border: p.control_border,
        text: p.text, // не используется у Switch (без подписи)
        radius: canvas_core::tokens::RADIUS_PILL,
    };
    // Бегунок: text_title (white/light); disabled — приглушён.
    let knob_fill = match state {
        KitState::Disabled => p.disabled_text,
        _ => p.text_title,
    };
    SwitchLayout {
        track,
        knob,
        track_style,
        knob_fill,
    }
}

// --- Card -------------------------------------------------------------------

/// Раскладка контентной карточки с хедером: внешний rect + хедер + body.
/// Хедер и body — внутри пад панели (`panel_style(p).pad` = SPACING_LG):
/// заголовок не впритык к краю, контент — ниже хедера с тем же падом.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardLayout {
    /// Внешний rect карточки (constrain+stack в слоте).
    pub rect: UiRect,
    /// Rect хедера (внутри пада; высота = `header_h` клампнутая к остатку).
    pub header: UiRect,
    /// Rect body (внутри пада; ниже хедера).
    pub body: UiRect,
}

/// Карточка в слоте: внешний rect = constrain+stack; хедер и body — внутри
/// пада панели (`panel_style(p).pad`). Палитра — слот фона/рамки (потребитель
/// рисует через `panel_style(p)` отдельно; контракт: цвет отдельно от геометрии).
pub fn card(slot: UiRect, min: UiVec2, max: UiVec2, header_h: f32, p: &KitPalette) -> CardLayout {
    let pad = panel_style(p).pad;
    let rect = panel_rect(slot, min, max, max);
    let inner = rect.inset(&pad);
    let hh = header_h.min(inner.h);
    let header = UiRect::new(inner.x, inner.y, inner.w, hh);
    let body = UiRect::new(inner.x, inner.y + hh, inner.w, (inner.h - hh).max(0.0));
    CardLayout { rect, header, body }
}

// --- Icon -------------------------------------------------------------------

/// Семантическая иконка для IconButton. Глифы — существующим шрифтом
/// (NotoSansDisplay-Medium): 0 новых зависимостей (G7). Литералы потребителей
/// («✕»/«⚙»/«?»/«+») переносятся сюда; новые (Search/ArrowLeft/ArrowRight/
/// Refresh) — стандартные Unicode-символы, поддерживаемые NotoSansDisplay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Закрыть (✕).
    Close,
    /// Настройки/шестерёнка (⚙).
    Gear,
    /// Помощь/вопрос (?).
    Question,
    /// Поиск (⌕).
    Search,
    /// Добавить (+).
    Plus,
    /// Стрелка влево (←).
    ArrowLeft,
    /// Стрелка вправо (→).
    ArrowRight,
    /// Обновить (↻).
    Refresh,
}

/// Глиф иконки — `&'static str` для `TextMeasurer`/`Painter::label`. Маппинг
/// полон (все варианты `Icon` покрыты — тест `icon_glyph_mapping_is_complete`).
pub fn icon_glyph(i: Icon) -> &'static str {
    match i {
        Icon::Close => "×",
        Icon::Gear => "⚙",
        Icon::Question => "?",
        Icon::Search => "⌕",
        Icon::Plus => "+",
        Icon::ArrowLeft => "←",
        Icon::ArrowRight => "→",
        Icon::Refresh => "↻",
    }
}

/// Квадратная кнопка с иконкой в слоте — делегирует [`icon_button_rect`]
/// (та же геометрия; иконка — отдельным вызовом [`icon_glyph`] для замера/
/// отрисовки потребителем). `icon` зарезервирован для будущей текстовой
/// раскладки (ширина глифа может варьироваться — v2 с TextMeasurer).
pub fn icon_button(slot: UiRect, _icon: Icon, align: (HAlign, VAlign)) -> UiRect {
    icon_button_rect(slot, align)
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

// --- Row (FR-061 D-15, этап E) ----------------------------------------------
//
// Кит-строка табличного тела: декларативные данные ([`RowParts`]) +
// колоночные направляющие ([`RowGuides`], D-3) → геометрия ([`row_layout`])
// + стиль из слотов ([`row_style`]) → отрисовка ([`paint_row`], Painter —
// FR-057). Потребители дают данные, каркас считает геометрию (правило
// FR-061): значение/юнит прижаты вправо на направляющих (D-4), лидер —
// пунктирная дорожка от конца левого текста до направляющей чисел (D-5;
// штрихи — [`leader_dash_rects`], единая геометрия с телом ноды).

/// Высота линейной коробки текста для вертикальной центровки:
/// `size * ROW_LINE_FRAC` — практика потребителей (панель FR-044:
/// коробка 12 px при кегле 11).
pub const ROW_LINE_FRAC: f32 = 1.1;
/// Отступ точки-маркера от левого края строки (панель FR-044, Р-4).
pub const ROW_DOT_PAD: f32 = 6.0;
/// Диаметр точки-маркера.
pub const ROW_DOT: f32 = 6.0;
/// Зазор «маркер → левый текст» / «левый текст → значение».
pub const ROW_TEXT_GAP: f32 = 6.0;
/// Минимальная ширина зоны глифа-маркера («ƒ»). Глиф уже зоны — зона
/// фиксируется, чтобы колонки строк не «дышали».
pub const ROW_GLYPH_MIN_W: f32 = 12.0;
/// Зазор «зона глифа → левый текст».
pub const ROW_GLYPH_GAP: f32 = 2.0;
/// Горизонтальный пад пилюли бейджа (этап E: бейдж — пилюля — примечание
/// D-14 «пилюля отложена в этап E (kit-Row)»). Ширина бейдж-колонки —
/// текст + 2·[`ROW_BADGE_PAD_H`].
pub const ROW_BADGE_PAD_H: f32 = 6.0;

/// Маркер левой колонки строки (прототип Р-4: value-точка / «ƒ»). Глиф —
/// существующим шрифтом (G7), ширина зоны — TextMeasurer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowMarker {
    /// Без маркера — левый текст от края строки.
    None,
    /// Круглая точка (value-маркер переменных).
    Dot,
    /// Глиф существующим шрифтом («ƒ» формул).
    Glyph(&'static str),
}

/// Данные строки таблицы (декларативно): потребители дают данные, каркас
/// ([`row_guides`]/[`row_layout`]) считает геометрию. Пустое значение —
/// ячейка не рисуется и лидера нет (строки-формулы панели FR-044).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowParts<'a> {
    pub marker: RowMarker,
    /// Левый текст (имя/формула/путь) — усекается ellipsis'ом (класс CR-015).
    pub label: &'a str,
    /// Значение (прижато вправо на направляющей чисел).
    pub value: &'a str,
    /// Юнит (прижат вправо на направляющей юнитов; пусто — скаляр).
    pub unit: &'a str,
    /// Бейдж (пилюля в бейдж-колонке; пусто — колонки нет).
    pub badge: &'a str,
}

/// Стиль строки — только слоты (контракт F-8). Поля — plain data:
/// потребитель переопределяет конкретные поля семантикой своей поверхности
/// (value-точка/ошибка/приглушение — как в панели FR-044); скрытой
/// арифметики над цветами в ките нет.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub marker: [f32; 4],
    pub label: [f32; 4],
    pub value: [f32; 4],
    pub unit: [f32; 4],
    /// Цвет текста бейджа (пилюля — [`RowStyle::badge_fill`]/[`RowStyle::badge_border`]).
    pub badge: [f32; 4],
    pub badge_fill: [f32; 4],
    pub badge_border: [f32; 4],
}

/// Стиль строки из слотов состояний: Normal — прозрачная строка (фон/зебру
/// решает потребитель — слотами своей темы), Hovered — слот hover,
/// Selected — слот selected + рамка accent, Disabled — приглушённые тексты.
pub fn row_style(state: KitState, p: &KitPalette) -> RowStyle {
    let transparent = [0.0; 4];
    match state {
        KitState::Disabled => RowStyle {
            fill: transparent,
            border: transparent,
            marker: p.disabled_text,
            label: p.disabled_text,
            value: p.disabled_text,
            unit: p.disabled_text,
            badge: p.disabled_text,
            badge_fill: transparent,
            badge_border: transparent,
        },
        KitState::Selected => RowStyle {
            fill: p.selected_fill,
            border: p.accent,
            marker: p.text,
            label: p.text,
            value: p.text,
            unit: p.text_muted,
            badge: p.text,
            badge_fill: p.control_fill,
            badge_border: p.control_border,
        },
        KitState::Hovered => RowStyle {
            fill: p.hover_fill,
            ..row_style(KitState::Normal, p)
        },
        _ => RowStyle {
            fill: transparent,
            border: transparent,
            marker: p.text,
            label: p.text,
            value: p.text,
            unit: p.text_muted,
            badge: p.text,
            badge_fill: p.control_fill,
            badge_border: p.control_border,
        },
    }
}

/// Опции строки (не-дефолтное поведение).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowOpts {
    /// Строить дорожку лидера между левой частью и значением. Панель
    /// «Как считается» (прототип FR-044 без лидера) — выключает; тело ноды
    /// и витрина — дефолт.
    pub leader: bool,
    /// Зазор между соседними ячейками (параметр потребителя — контракт
    /// [`RowGuides::with_right_edge`]; дефолт — токен TABLE_GUIDE_GAP).
    pub gap: f32,
}

impl Default for RowOpts {
    fn default() -> Self {
        Self {
            leader: true,
            gap: canvas_core::tokens::TABLE_GUIDE_GAP,
        }
    }
}

/// Геометрия строки на направляющих ([`row_layout`]): rect'ы ячеек в
/// координатах потребителя. Тексты уже усечены ([`RowLayout::label_shown`],
/// политика Ellipsis); value/unit — area'ы под фактический текст (прижат
/// вправо: left = right − width, D-4).
#[derive(Debug, Clone, PartialEq)]
pub struct RowLayout {
    /// Слот строки (фон/зебра — на всю ширину).
    pub row: UiRect,
    /// Точка-маркер ([`RowMarker::Dot`]) — круг (радиус = w/2).
    pub dot: Option<UiRect>,
    /// Зона глифа-маркера ([`RowMarker::Glyph`]).
    pub glyph: Option<UiRect>,
    /// Зона левого текста (усечённого — [`RowLayout::label_shown`]).
    pub label: UiRect,
    /// Левый текст после ellipsis (последняя точка усечения — CR-015).
    pub label_shown: String,
    /// Дорожка лидера (x0, x1) на [`RowLayout::leader_y`]; `None` — значения
    /// нет / лидер выключен / дорожка короче TABLE_LEADER_MIN.
    pub leader: Option<(f32, f32)>,
    /// Вертикаль дорожки лидера (доля строки TABLE_LEADER_Y_FRAC — как в
    /// теле ноды).
    pub leader_y: f32,
    /// Ячейка значения (area уже под фактический текст).
    pub value: UiRect,
    /// Ячейка юнита.
    pub unit: UiRect,
    /// Пилюля бейджа.
    pub badge: Option<UiRect>,
}

/// Проход A кит-строк: замер ячеек по всем строкам ([`RowGuides::measure`])
/// и проход B от правого края ([`RowGuides::with_right_edge`]). Бейдж —
/// пилюля: текст + 2·[`ROW_BADGE_PAD_H`]. `None` — строк нет.
#[allow(clippy::too_many_arguments)]
pub fn row_guides(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
    rows: &[RowParts<'_>],
    right_edge: f32,
    gap: f32,
) -> Option<RowGuides> {
    let widths: Vec<RowCellWidths> = rows
        .iter()
        .map(|row| RowCellWidths {
            value_w: if row.value.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.value, family, size)
            },
            unit_w: if row.unit.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.unit, family, size)
            },
            badge_w: if row.badge.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.badge, family, size) + 2.0 * ROW_BADGE_PAD_H
            },
        })
        .collect();
    RowGuides::measure(&widths).map(|g| g.with_right_edge(right_edge, gap))
}

/// Геометрия одной строки: маркер, левый текст (ellipsis до ячейки
/// значения), лидер (D-5), value/unit на направляющих (D-4), пилюля бейджа.
/// Ширины ячеек направляющих — вход [`RowGuides`] (проход A — [`row_guides`]
/// или собственный проход потребителя, как в теле ноды).
#[allow(clippy::too_many_arguments)]
pub fn row_layout(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
    slot: UiRect,
    guides: RowGuides,
    parts: &RowParts<'_>,
    opts: &RowOpts,
) -> RowLayout {
    let text_y = slot.y + (slot.h - size * ROW_LINE_FRAC) / 2.0;
    let (dot, glyph, label_x) = match parts.marker {
        RowMarker::Dot => (
            Some(UiRect::new(
                slot.x + ROW_DOT_PAD,
                slot.y + (slot.h - ROW_DOT) / 2.0,
                ROW_DOT,
                ROW_DOT,
            )),
            None,
            slot.x + ROW_DOT_PAD + ROW_DOT + ROW_TEXT_GAP,
        ),
        RowMarker::Glyph(glyph) => {
            let glyph_w = if glyph.is_empty() {
                0.0
            } else {
                m.width_of(fs, glyph, family, size)
            };
            let zone_w = glyph_w.max(ROW_GLYPH_MIN_W);
            (
                None,
                Some(UiRect::new(slot.x + ROW_DOT_PAD, text_y, zone_w, slot.h)),
                slot.x + ROW_DOT_PAD + zone_w + ROW_GLYPH_GAP,
            )
        }
        RowMarker::None => (None, None, slot.x),
    };
    let value_w = if parts.value.is_empty() {
        0.0
    } else {
        m.width_of(fs, parts.value, family, size)
    };
    let unit_w = if parts.unit.is_empty() {
        0.0
    } else {
        m.width_of(fs, parts.unit, family, size)
    };
    let value = if parts.value.is_empty() {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    } else {
        UiRect::new(guides.value_right() - value_w, text_y, value_w, slot.h)
    };
    let unit = if parts.unit.is_empty() {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    } else {
        UiRect::new(guides.unit_right() - unit_w, text_y, unit_w, slot.h)
    };
    let badge = if parts.badge.is_empty() {
        None
    } else {
        let pill_h = (size * ROW_LINE_FRAC + 6.0).min(slot.h);
        Some(UiRect::new(
            guides.unit_right() + opts.gap,
            slot.y + (slot.h - pill_h) / 2.0,
            guides.badge_w,
            pill_h,
        ))
    };
    // Левый текст: до ячейки значения / юнита / края строки (что первое) —
    // колонка левых текстов стабильна по ноде (табличная семантика §3.1).
    let label_right = if !parts.value.is_empty() {
        guides.value_x - ROW_TEXT_GAP
    } else if !parts.unit.is_empty() {
        guides.unit_x - ROW_TEXT_GAP
    } else {
        slot.right() - ROW_TEXT_GAP
    };
    let label_w = (label_right - label_x).max(0.0);
    let label_shown = m.ellipsis(fs, parts.label, family, size, label_w);
    let label = UiRect::new(label_x, text_y, label_w, slot.h);
    // Лидер: от конца фактического текста до направляющей чисел (D-5 —
    // как в теле ноды: left_end + PAD → value_right − PAD).
    let leader = if opts.leader && !parts.value.is_empty() && !label_shown.is_empty() {
        let x0 = label.x
            + m.width_of(fs, &label_shown, family, size)
            + canvas_core::tokens::TABLE_LEADER_PAD;
        let x1 = guides.value_right() - canvas_core::tokens::TABLE_LEADER_PAD;
        if x1 - x0 >= canvas_core::tokens::TABLE_LEADER_MIN {
            Some((x0, x1))
        } else {
            None
        }
    } else {
        None
    };
    RowLayout {
        row: slot,
        dot,
        glyph,
        label,
        label_shown,
        leader,
        leader_y: slot.y + slot.h * canvas_core::tokens::TABLE_LEADER_Y_FRAC,
        value,
        unit,
        badge,
    }
}

/// Отрисовка строки в Painter (FR-057): фон/рамка → маркер → левый текст →
/// лидер → значение → юнит → бейдж (порядок = draw-порядок). Ширины и
/// усечение уже посчитаны в [`row_layout`] — здесь только место и стиль
/// (контракт Painter::Text).
pub fn paint_row(p: &mut Painter, lay: &RowLayout, parts: &RowParts<'_>, s: &RowStyle, size: f32) {
    p.rect(lay.row, s.fill, s.border, canvas_core::tokens::RADIUS_CHIP);
    if let Some(dot) = lay.dot {
        p.rect(dot, s.marker, [0.0; 4], dot.w / 2.0);
    }
    if let (Some(zone), RowMarker::Glyph(glyph)) = (lay.glyph, parts.marker) {
        if !glyph.is_empty() {
            p.label(zone, glyph, s.marker, size, PaintAlign::Left);
        }
    }
    p.label(lay.label, &lay.label_shown, s.label, size, PaintAlign::Left);
    if let Some((x0, x1)) = lay.leader {
        for dash in leader_dash_rects(
            x0,
            x1,
            lay.leader_y,
            1.0,
            canvas_core::tokens::TABLE_LEADER_MIN,
        ) {
            p.rect(
                UiRect::new(dash[0], dash[1], dash[2], dash[3]),
                s.marker,
                [0.0; 4],
                0.0,
            );
        }
    }
    if lay.value.w > 0.0 {
        p.label(lay.value, parts.value, s.value, size, PaintAlign::Left);
    }
    if lay.unit.w > 0.0 {
        p.label(lay.unit, parts.unit, s.unit, size, PaintAlign::Left);
    }
    if let Some(pill) = lay.badge {
        p.rect(pill, s.badge_fill, s.badge_border, pill.h / 2.0);
        let text = UiRect::new(
            pill.x + ROW_BADGE_PAD_H,
            pill.y + (pill.h - size * ROW_LINE_FRAC) / 2.0,
            (pill.w - 2.0 * ROW_BADGE_PAD_H).max(0.0),
            pill.h,
        );
        p.label(text, parts.badge, s.badge, size, PaintAlign::Left);
    }
}

/// Штрихи лидера (D-5) — ЕДИНАЯ геометрия с телом ноды
/// (`canvas-render/text.rs`): токены TABLE_LEADER_{DASH,GAP,H}, штрихи
/// высотой `TABLE_LEADER_H·scale` на ординате `y` от `x0` до `x1`;
/// дорожка короче `min_track·scale` — штрихов нет (тело ноды — 6 px
/// исторически; кит — TABLE_LEADER_MIN).
pub fn leader_dash_rects(x0: f32, x1: f32, y: f32, scale: f32, min_track: f32) -> Vec<[f32; 4]> {
    let dash = canvas_core::tokens::TABLE_LEADER_DASH * scale;
    let step =
        (canvas_core::tokens::TABLE_LEADER_DASH + canvas_core::tokens::TABLE_LEADER_GAP) * scale;
    let h = canvas_core::tokens::TABLE_LEADER_H * scale;
    let mut out = Vec::new();
    if x1 - x0 >= min_track * scale {
        let mut x = x0;
        while x + dash <= x1 {
            out.push([x, y, dash, h]);
            x += step;
        }
    }
    out
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

    // === FR-058: тесты компонентов v2 =======================================

    // --- TextFieldModel: insert/backspace/delete/move/select/set_text -------

    #[test]
    fn text_field_insert_at_start_middle_end() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        // Вставка в начало
        m.caret = 0;
        m.insert("X");
        assert_eq!(m.text, "Xhello");
        assert_eq!(m.caret, 1);
        // Вставка в середину
        m.caret = 3;
        m.insert("Y");
        assert_eq!(m.text, "XheYllo");
        assert_eq!(m.caret, 4);
        // Вставка в конец
        m.caret = m.text.chars().count();
        m.insert("Z");
        assert_eq!(m.text, "XheYlloZ");
        assert_eq!(m.caret, 8);
    }

    #[test]
    fn text_field_insert_replaces_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello world".to_owned());
        // Выделить "lo wo"
        m.sel = Some((3, 8));
        m.caret = 8;
        m.insert("XYZ");
        assert_eq!(m.text, "helXYZrld");
        assert_eq!(m.caret, 6);
        assert!(m.sel.is_none(), "селекция снята после insert");
    }

    #[test]
    fn text_field_backspace_at_start_is_noop() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 0;
        m.backspace();
        assert_eq!(m.text, "abc");
        assert_eq!(m.caret, 0);
    }

    #[test]
    fn text_field_backspace_deletes_char_before_caret() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 2;
        m.backspace();
        assert_eq!(m.text, "ac");
        assert_eq!(m.caret, 1);
    }

    #[test]
    fn text_field_backspace_deletes_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.sel = Some((1, 4)); // "ell"
        m.caret = 4;
        m.backspace();
        assert_eq!(m.text, "ho");
        assert_eq!(m.caret, 1);
        assert!(m.sel.is_none());
    }

    #[test]
    fn text_field_delete_at_end_is_noop() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 3;
        m.delete();
        assert_eq!(m.text, "abc");
        assert_eq!(m.caret, 3);
    }

    #[test]
    fn text_field_delete_deletes_char_after_caret() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        m.caret = 0;
        m.delete();
        assert_eq!(m.text, "bc");
        assert_eq!(m.caret, 0);
    }

    #[test]
    fn text_field_delete_deletes_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.sel = Some((1, 4));
        m.caret = 1;
        m.delete();
        assert_eq!(m.text, "ho");
        assert_eq!(m.caret, 1);
        assert!(m.sel.is_none());
    }

    #[test]
    fn text_field_move_caret_left_right_clamps_at_edges() {
        let mut m = TextFieldModel::default();
        m.set_text("abc".to_owned());
        // Левее начала — кламп к 0
        m.move_caret(-5, false);
        assert_eq!(m.caret, 0);
        // Правее конца — кламп к 3
        m.move_caret(10, false);
        assert_eq!(m.caret, 3);
        // В середину
        m.move_caret(-1, false);
        assert_eq!(m.caret, 2);
        assert!(m.sel.is_none(), "без extend — селекция снята");
    }

    #[test]
    fn text_field_move_caret_extend_creates_and_extends_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.caret = 3;
        // extend влево — селекция (3, 2)
        m.move_caret(-1, true);
        assert_eq!(m.caret, 2);
        assert_eq!(m.sel, Some((3, 2)));
        // extend дальше — anchor сохраняется, head движется
        m.move_caret(-1, true);
        assert_eq!(m.caret, 1);
        assert_eq!(m.sel, Some((3, 1)));
        // extend вправо — head обратно к anchor
        m.move_caret(2, true);
        assert_eq!(m.caret, 3);
        assert_eq!(m.sel, Some((3, 3)));
    }

    #[test]
    fn text_field_select_all_and_clear_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.select_all();
        assert_eq!(m.sel, Some((0, 5)));
        assert_eq!(m.caret, 5);
        m.clear_selection();
        assert!(m.sel.is_none());
        assert_eq!(m.caret, 5, "каретка остаётся после clear_selection");
    }

    #[test]
    fn text_field_set_text_resets_caret_and_selection() {
        let mut m = TextFieldModel::default();
        m.set_text("hello".to_owned());
        m.caret = 2;
        m.sel = Some((0, 2));
        m.set_text("new".to_owned());
        assert_eq!(m.text, "new");
        assert_eq!(m.caret, 3);
        assert!(m.sel.is_none());
    }

    /// Инвариант FR-058: каретка/селекция — в СИМВОЛАХ, не байтах.
    /// Тест на emoji (4-байтный глиф) и multi-byte (кириллица).
    #[test]
    fn text_field_unicode_emoji_and_cyrillic_positions() {
        let mut m = TextFieldModel::default();
        // 🎉 — U+1F389, 4 байта в UTF-8, 1 char (в utf-16 — 2 единицы).
        m.set_text("a🎉b".to_owned());
        assert_eq!(m.text.len(), 6, "4 байта для emoji + 2 ascii");
        assert_eq!(m.text.chars().count(), 3, "3 символа");
        // Каретка после emoji (position=2 в символах)
        m.caret = 2;
        m.insert("X");
        assert_eq!(m.text, "a🎉Xb");
        assert_eq!(m.caret, 3);
        // Backspace удаляет emoji целиком (1 символ)
        m.caret = 2;
        m.backspace();
        assert_eq!(m.text, "aXb");
        assert_eq!(m.caret, 1);
        // Кириллица (2 байта на символ в UTF-8)
        m.set_text("привет".to_owned());
        assert_eq!(m.text.len(), 12, "6 символов × 2 байта");
        assert_eq!(m.text.chars().count(), 6);
        m.caret = 3; // после "при" — delete() удаляет символ по индексу 3 ("в")
        m.delete();
        assert_eq!(m.text, "приет");
        assert_eq!(m.caret, 3);
        // Select all + delete selection — удаляет все 6 символов
        m.select_all();
        m.delete();
        assert_eq!(m.text, "");
        assert_eq!(m.caret, 0);
    }

    // --- TextFieldLayout: text_field() --------------------------------------

    /// Детерминированный FontSystem: только NotoSansDisplay-Medium (как в
    /// measure.rs tests) — метрики тестов = метрикам рендера.
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    #[test]
    fn text_field_layout_empty_shows_placeholder_and_caret_at_start() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "Поиск…",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(
            lay.text_shown, "Поиск…",
            "placeholder показан целиком (помещается)"
        );
        assert!(
            (lay.caret_x - lay.text_area.x).abs() < 0.01,
            "каретка у левого края"
        );
        assert!((lay.rect.h - TEXT_FIELD_HEIGHT).abs() < 0.01);
        // text_area уже rect на пад
        assert!((lay.text_area.x - (lay.rect.x + TEXT_FIELD_PAD_H)).abs() < 0.01);
        assert!((lay.text_area.w - (lay.rect.w - TEXT_FIELD_PAD_H * 2.0)).abs() < 0.01);
    }

    #[test]
    fn text_field_layout_empty_no_caret_when_not_focused() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "Поиск…",
            false,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(lay.caret_x, -1.0, "не сфокусировано — каретка не рисуется");
    }

    #[test]
    fn text_field_layout_non_empty_shows_text_and_caret_at_measured_prefix() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let mut model = TextFieldModel::default();
        model.set_text("hello".to_owned());
        model.caret = 2; // после "he"
        let slot = UiRect::new(0.0, 0.0, 200.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
            UiVec2::new(400.0, TEXT_FIELD_HEIGHT),
            &model,
            "",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(lay.text_shown, "hello");
        let prefix_w = m.width_of(&mut fs, "he", FAMILY, 13.0);
        assert!((lay.caret_x - (lay.text_area.x + prefix_w)).abs() < 0.1);
    }

    #[test]
    fn text_field_layout_placeholder_ellipsized_when_too_wide() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let model = TextFieldModel::default();
        // Узкий слот — placeholder не помещается, усекается с ellipsis
        let slot = UiRect::new(0.0, 0.0, 30.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(0.0, TEXT_FIELD_HEIGHT),
            UiVec2::new(30.0, TEXT_FIELD_HEIGHT),
            &model,
            "Очень длинный placeholder",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(lay.text_shown != "Очень длинный placeholder");
        assert!(lay.text_shown.ends_with('\u{2026}') || lay.text_shown.is_empty());
    }

    #[test]
    fn text_field_layout_caret_clamped_to_text_area_when_prefix_too_wide() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let mut model = TextFieldModel::default();
        model.set_text("очень длинный текст не помещается в слот".to_owned());
        model.caret = model.text.chars().count(); // каретка в конце
        let slot = UiRect::new(0.0, 0.0, 50.0, TEXT_FIELD_HEIGHT);
        let lay = text_field(
            slot,
            UiVec2::new(0.0, TEXT_FIELD_HEIGHT),
            UiVec2::new(50.0, TEXT_FIELD_HEIGHT),
            &model,
            "",
            true,
            KitState::Normal,
            &palette_a(),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        // Каретка клампнута к правому краю text_area
        assert!(lay.caret_x <= lay.text_area.right() + 0.01);
        assert!(lay.caret_x >= lay.text_area.x);
    }

    // --- ScrollState: scroll_by/clamp/needs_scroll/max_offset ----------------

    #[test]
    fn scroll_state_scroll_by_positive_and_negative() {
        let mut s = ScrollState {
            offset: 10.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        s.scroll_by(50.0);
        assert_eq!(s.offset, 60.0);
        s.scroll_by(-30.0);
        assert_eq!(s.offset, 30.0);
        // Отрицательный результат клампится к 0 (не уходит в минус)
        s.scroll_by(-100.0);
        assert_eq!(s.offset, 0.0);
    }

    #[test]
    fn scroll_state_clamp_at_edges() {
        let mut s = ScrollState {
            offset: 200.0, // больше max_offset
            content_h: 200.0,
            viewport_h: 100.0,
        };
        s.clamp();
        assert_eq!(s.offset, 100.0, "clamp к max_offset");
        // offset < 0 — кламп к 0
        s.offset = -10.0;
        s.clamp();
        assert_eq!(s.offset, 0.0);
        // offset в пределах — без изменений
        s.offset = 50.0;
        s.clamp();
        assert_eq!(s.offset, 50.0);
    }

    #[test]
    fn scroll_state_needs_scroll_and_max_offset() {
        // Контент выше вьюпорта — нужен скролл
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        assert!(s.needs_scroll());
        assert_eq!(s.max_offset(), 100.0);
        // Контент равен вьюпорту — скролл не нужен, max_offset = 0
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(!s.needs_scroll());
        assert_eq!(s.max_offset(), 0.0);
        // Контент меньше вьюпорта — скролл не нужен, max_offset = 0 (не отрицательный)
        let s = ScrollState {
            offset: 0.0,
            content_h: 50.0,
            viewport_h: 100.0,
        };
        assert!(!s.needs_scroll());
        assert_eq!(s.max_offset(), 0.0);
    }

    // --- list_rows: оффсет → индексы/rect'ы, частичные строки ----------------

    #[test]
    fn list_rows_empty_when_no_rows_or_zero_height() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 0.0,
            content_h: 0.0,
            viewport_h: 100.0,
        };
        assert!(list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 0).is_empty());
        // row_h = 0 — вырожденный, пустой результат
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(list_rows(area, &s, 0.0, LIST_ROW_GAP, 5).is_empty());
    }

    #[test]
    fn list_rows_all_visible_when_no_scroll() {
        let area = UiRect::new(10.0, 20.0, 200.0, 100.0);
        // 3 строки по 26px + 2 зазора по 6 = 90px — помещаются в 100px
        let s = ScrollState {
            offset: 0.0,
            content_h: 90.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, 0);
        assert_eq!(rows[1].0, 1);
        assert_eq!(rows[2].0, 2);
        // Геометрия: первая строка у верха area
        assert!((rows[0].1.y - area.y).abs() < 0.01);
        assert!((rows[0].1.x - area.x).abs() < 0.01);
        assert_eq!(rows[0].1.w, area.w);
        assert_eq!(rows[0].1.h, LIST_ROW_H);
        // Вторая — на stride ниже
        let stride = LIST_ROW_H + LIST_ROW_GAP;
        assert!((rows[1].1.y - (area.y + stride)).abs() < 0.01);
    }

    #[test]
    fn list_rows_offset_skips_hidden_rows() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // 5 строк по 26 + 4 зазора по 6 = 154px content; viewport 100
        // stride = 32. offset = 32 (1 stride) — скрываем строку 0.
        let s = ScrollState {
            offset: 32.0,
            content_h: 154.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 5);
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(!indices.contains(&0), "строка 0 скрыта offset'ом");
        assert!(indices.contains(&1), "строка 1 видна");
        assert!(indices.contains(&2), "строка 2 видна");
        assert!(indices.contains(&3), "строка 3 видна");
        assert!(indices.contains(&4), "строка 4 видна");
        // Строка 1 — у верха area (screen_y = 0 + 32 - 32 = 0)
        let row1 = rows.iter().find(|(i, _)| *i == 1).unwrap().1;
        assert!(
            (row1.y - 0.0).abs() < 0.01,
            "строка 1 у верха viewport: y={}",
            row1.y
        );
    }

    #[test]
    fn list_rows_includes_partial_row_at_top() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // offset=40 — строка 1 (top=32, bottom=58) видна частично сверху (8px).
        let s = ScrollState {
            offset: 40.0,
            content_h: 154.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 5);
        // Строка 0 скрыта (bottom=26 < offset=40)
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(!indices.contains(&0), "строка 0 скрыта");
        // Строка 1 видна частично сверху: screen_y = 0 + 32 - 40 = -8
        let row1 = rows.iter().find(|(i, _)| *i == 1).unwrap().1;
        assert!(
            (row1.y - (-8.0)).abs() < 0.01,
            "частичная строка сверху: y={}",
            row1.y
        );
    }

    #[test]
    fn list_rows_includes_partial_rows_at_bottom() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // offset=0, viewport=100, row_h=26, gap=6 → stride=32
        // Строки 0..3 полностью (0..96), строка 3 (96..122) — частично снизу (96..100)
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 10);
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(indices.contains(&3), "частичная строка 3 включена снизу");
        assert!(!indices.contains(&4), "строка 4 за пределами viewport");
    }

    #[test]
    fn list_rows_does_not_mutate_scroll_state() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 30.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let s_before = s.clone();
        let _ = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 10);
        assert_eq!(s, s_before, "чистая функция — без мутаций");
    }

    // --- scroll_bar: None когда не нужен, knob при needs_scroll --------------

    #[test]
    fn scroll_bar_none_when_no_scroll_needed() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(scroll_bar(area, &s, &palette_a()).is_none());
        // content_h < viewport_h — тоже нет
        let s = ScrollState {
            offset: 0.0,
            content_h: 50.0,
            viewport_h: 100.0,
        };
        assert!(scroll_bar(area, &s, &palette_a()).is_none());
    }

    #[test]
    fn scroll_bar_knob_geometry_proportional_and_positioned() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // content=200, viewport=100 → ratio=0.5, knob_h=50, max_offset=100
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert_eq!(knob.x, area.right() - SCROLLBAR_WIDTH);
        assert_eq!(knob.w, SCROLLBAR_WIDTH);
        assert!((knob.h - 50.0).abs() < 0.01, "knob_h = 0.5 * 100 = 50");
        // offset=0 → knob у верха
        assert!((knob.y - area.y).abs() < 0.01);
        // offset=max → knob у низа
        let s = ScrollState {
            offset: 100.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert!(
            (knob.bottom() - area.bottom()).abs() < 0.01,
            "knob у низа при max offset"
        );
        // offset=50 → knob в середине
        let s = ScrollState {
            offset: 50.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        let expected_y = area.y + (100.0 - 50.0) * 0.5; // 25
        assert!((knob.y - expected_y).abs() < 0.01);
    }

    #[test]
    fn scroll_bar_knob_min_height_enforced() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // content=1000, viewport=100 → ratio=0.1 → knob_h=10 → clamped to MIN=20
        let s = ScrollState {
            offset: 0.0,
            content_h: 1000.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert!(knob.h >= SCROLLBAR_KNOB_MIN, "минимальная высота бегунка");
    }

    // --- switch: on/off позиция, геометрия в слоте, стили --------------------

    #[test]
    fn switch_geometry_in_slot_and_knob_position_by_on() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 30.0);
        let p = palette_a();
        // ON
        let on = switch(slot, true, KitState::Normal, &p);
        assert!((on.track.w - SWITCH_W).abs() < 0.01);
        assert!((on.track.h - SWITCH_H).abs() < 0.01);
        assert!(
            (on.track.x - (slot.x + (slot.w - SWITCH_W) / 2.0)).abs() < 0.01,
            "трек по центру слота"
        );
        let knob_side = SWITCH_H - 2.0 * SWITCH_KNOB_PAD;
        assert!((on.knob.w - knob_side).abs() < 0.01);
        assert!((on.knob.h - knob_side).abs() < 0.01);
        // ON: knob у правого края трека
        assert!((on.knob.right() - (on.track.right() - SWITCH_KNOB_PAD)).abs() < 0.01);
        // OFF: knob у левого края трека
        let off = switch(slot, false, KitState::Normal, &p);
        assert!((off.knob.x - (off.track.x + SWITCH_KNOB_PAD)).abs() < 0.01);
    }

    #[test]
    fn switch_uses_palette_slots_for_fill() {
        let p = palette_a();
        // ON normal → control_primary
        let on = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Normal,
            &p,
        );
        assert_eq!(on.track_style.fill, p.control_primary);
        // OFF normal → control_fill
        let off = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            false,
            KitState::Normal,
            &p,
        );
        assert_eq!(off.track_style.fill, p.control_fill);
        // ON hovered → primary_hover_fill
        let on_h = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Hovered,
            &p,
        );
        assert_eq!(on_h.track_style.fill, p.primary_hover_fill);
        // OFF hovered → hover_fill
        let off_h = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            false,
            KitState::Hovered,
            &p,
        );
        assert_eq!(off_h.track_style.fill, p.hover_fill);
        // Knob fill: text_title (normal), disabled_text (disabled)
        assert_eq!(on.knob_fill, p.text_title);
        let dis = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Disabled,
            &p,
        );
        assert_eq!(dis.knob_fill, p.disabled_text);
        // Радиус — RADIUS_PILL
        assert_eq!(on.track_style.radius, canvas_core::tokens::RADIUS_PILL);
    }

    // --- card: геометрия в слоте, min/max, header/body ----------------------

    #[test]
    fn card_geometry_in_slot_with_header_and_body() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let p = palette_a();
        let lay = card(
            slot,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(400.0, 300.0),
            30.0,
            &p,
        );
        // rect = 400×300 (по max)
        assert!((lay.rect.w - 400.0).abs() < 0.01);
        assert!((lay.rect.h - 300.0).abs() < 0.01);
        assert!((lay.rect.x - slot.x).abs() < 0.01);
        // header и body — внутри пада SPACING_LG (12)
        let pad = canvas_core::tokens::SPACING_LG;
        assert!((lay.header.x - (lay.rect.x + pad)).abs() < 0.01);
        assert!((lay.header.y - (lay.rect.y + pad)).abs() < 0.01);
        assert!((lay.header.w - (lay.rect.w - 2.0 * pad)).abs() < 0.01);
        assert!((lay.header.h - 30.0).abs() < 0.01, "header_h как передано");
        // body — ниже header, внутри пада
        assert!((lay.body.x - lay.header.x).abs() < 0.01);
        assert!((lay.body.y - (lay.header.y + lay.header.h)).abs() < 0.01);
        assert!((lay.body.w - lay.header.w).abs() < 0.01);
        assert!((lay.body.bottom() - (lay.rect.bottom() - pad)).abs() < 0.01);
    }

    #[test]
    fn card_clamps_to_min_max() {
        let slot = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let p = palette_a();
        // desired = max = 400×300 → clamp к max
        let lay = card(
            slot,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(400.0, 300.0),
            30.0,
            &p,
        );
        assert!((lay.rect.w - 400.0).abs() < 0.01);
        assert!((lay.rect.h - 300.0).abs() < 0.01);
    }

    #[test]
    fn card_header_h_clamped_when_too_tall() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let p = palette_a();
        // header_h = 200, но inner.h < 200 → header сжимается до inner.h
        let lay = card(
            slot,
            UiVec2::new(100.0, 50.0),
            UiVec2::new(200.0, 100.0),
            200.0,
            &p,
        );
        let pad = canvas_core::tokens::SPACING_LG;
        let inner_h = (lay.rect.h - 2.0 * pad).max(0.0);
        assert!(
            (lay.header.h - inner_h).abs() < 0.01,
            "header_h сжат до inner.h"
        );
        // body — нулевой (всё съел header)
        assert!(lay.body.h.abs() < 0.01);
    }

    // --- icon_glyph: маппинг полон ------------------------------------------

    #[test]
    fn icon_glyph_mapping_is_complete() {
        // Все варианты Icon возвращают непустой &str
        for icon in [
            Icon::Close,
            Icon::Gear,
            Icon::Question,
            Icon::Search,
            Icon::Plus,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::Refresh,
        ] {
            let g = icon_glyph(icon);
            assert!(!g.is_empty(), "{icon:?} — пустой глиф");
        }
        // Существующие литералы потребителей сохранены (I-1 ноль скачка)
        assert_eq!(icon_glyph(Icon::Close), "×");
        assert_eq!(icon_glyph(Icon::Gear), "⚙");
        assert_eq!(icon_glyph(Icon::Question), "?");
        assert_eq!(icon_glyph(Icon::Plus), "+");
    }

    #[test]
    fn icon_button_delegates_to_icon_button_rect() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let align = (HAlign::End, VAlign::Start);
        // icon_button возвращает тот же rect, что icon_button_rect
        for icon in [
            Icon::Close,
            Icon::Gear,
            Icon::Question,
            Icon::Search,
            Icon::Plus,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::Refresh,
        ] {
            let r = icon_button(slot, icon, align);
            let expected = icon_button_rect(slot, align);
            assert_eq!(
                r, expected,
                "{icon:?}: icon_button делегирует icon_button_rect"
            );
        }
    }

    // === FR-062 F-17: focus_order ===

    /// Текущий фокус кольца находится в Tab-порядке по значению rect'а.
    #[test]
    fn focus_order_finds_ring_current_in_tab_order() {
        use crate::keyboard::FocusRing;
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

    /// Золотая геометрия Switch (фикс-слоты: трек/курок) и Card
    /// (хедер/тело) — изменение раскладки ловится эталоном.
    #[test]
    fn snapshot_switch_and_card_geometry_golden() {
        use crate::testing::{assert_snapshot, snap};
        let p = palette_a();
        // Switch: трек 36×20 по центру слота; бегунок 16×16 (on — справа)
        let slot = UiRect::new(10.0, 20.0, 200.0, 40.0);
        let sw = switch(slot, true, KitState::Normal, &p);
        assert_snapshot(
            format!("{}\n{}", snap("track", sw.track), snap("knob", sw.knob)),
            "track x=92 y=30 w=36 h=20\nknob x=110 y=32 w=16 h=16",
        );
        // Card: пад панели SPACING_LG=12; хедер 24, тело — остаток
        let card = card(
            UiRect::new(0.0, 0.0, 400.0, 120.0),
            UiVec2::new(0.0, 120.0),
            UiVec2::new(400.0, 120.0),
            24.0,
            &p,
        );
        assert_snapshot(
            format!(
                "{}\n{}",
                snap("card_header", card.header),
                snap("card_body", card.body)
            ),
            "card_header x=12 y=12 w=376 h=24\ncard_body x=12 y=36 w=376 h=72",
        );
    }

    // --- Row (FR-061 D-15, этап E) ------------------------------------------

    use crate::paint::PaintItem;

    const ROW_SIZE: f32 = 12.0;

    fn row_parts<'a>(
        marker: RowMarker,
        label: &'a str,
        value: &'a str,
        unit: &'a str,
        badge: &'a str,
    ) -> RowParts<'a> {
        RowParts {
            marker,
            label,
            value,
            unit,
            badge,
        }
    }

    /// D-15: `row_guides` — направляющие по max-ширинам ячеек кит-строк,
    /// правый край — параметр потребителя; бейдж-колонка = текст + 2·пад.
    #[test]
    fn row_guides_takes_max_and_positions_from_right_edge() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let rows = [
            row_parts(RowMarker::Dot, "a", "800", "rps", ""),
            row_parts(RowMarker::None, "b", "20", "", "← источник"),
        ];
        let g = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            &rows,
            300.0,
            canvas_core::tokens::TABLE_GUIDE_GAP,
        )
        .unwrap();
        let badge_w = m.width_of(&mut fs, "← источник", FAMILY, ROW_SIZE) + 2.0 * ROW_BADGE_PAD_H;
        assert_eq!(g.badge_w, badge_w, "бейдж — текст + 2 пада пилюли");
        assert_eq!(g.value_w, m.width_of(&mut fs, "800", FAMILY, ROW_SIZE));
        assert_eq!(g.unit_w, m.width_of(&mut fs, "rps", FAMILY, ROW_SIZE));
        // Правый край: бейдж [300−badge..300], юнит левее, значение ещё левее
        assert_eq!(
            g.unit_right(),
            300.0 - badge_w - canvas_core::tokens::TABLE_GUIDE_GAP
        );
        assert_eq!(
            g.value_right(),
            g.unit_right() - canvas_core::tokens::TABLE_GUIDE_GAP - g.unit_w
        );
        // Пустой список — таблицы нет
        assert!(row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &[], 300.0, 6.0).is_none());
    }

    /// D-15: `row_layout` — значение/юнит прижаты вправо на направляющих
    /// (D-4), маркеры занимают прежние зоны панели (точка x+6, текст x+18),
    /// пустое значение — ячейки нет.
    #[test]
    fn row_layout_aligns_cells_to_guides() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(10.0, 20.0, 300.0, 22.0);
        let rows = [row_parts(RowMarker::Dot, "путь", "1389", "rps", "")];
        let g = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            &rows,
            slot.right(),
            canvas_core::tokens::TABLE_GUIDE_GAP,
        )
        .unwrap();
        let lay = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        // Точка: x+6, диаметр 6, центр по вертикали
        let dot = lay.dot.unwrap();
        assert_eq!(dot.x, slot.x + ROW_DOT_PAD);
        assert_eq!(dot.w, ROW_DOT);
        // Левый текст начинается в x+18 (6+6+6) — зона панели FR-044
        assert_eq!(lay.label.x, slot.x + 18.0);
        // Значение: право на направляющую чисел, left = right − width (D-4)
        let vw = m.width_of(&mut fs, "1389", FAMILY, ROW_SIZE);
        assert!((lay.value.right() - g.value_right()).abs() < 0.01);
        assert!((lay.value.w - vw).abs() < 0.01);
        // Юнит: право на направляющую юнитов (O-4)
        let uw = m.width_of(&mut fs, "rps", FAMILY, ROW_SIZE);
        assert!((lay.unit.right() - g.unit_right()).abs() < 0.01);
        assert!((lay.unit.w - uw).abs() < 0.01);
        assert!(
            (lay.leader_y - slot.y - slot.h * canvas_core::tokens::TABLE_LEADER_Y_FRAC).abs()
                < 0.01
        );
        // Формульная строка (глиф, без значения): ячейки нет, лидера нет
        let formula = row_parts(RowMarker::Glyph("ƒ"), "итого = a * b", "", "", "");
        let lf = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &formula,
            &RowOpts::default(),
        );
        assert!(lf.glyph.is_some());
        assert_eq!(lf.value.w, 0.0);
        assert!(lf.leader.is_none());
    }

    /// D-15: RowOpts::leader=false (панель FR-044 — прототип без лидера) —
    /// дорожки нет, прочая геометрия без изменений.
    #[test]
    fn row_opts_disable_leader() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let rows = [row_parts(RowMarker::Dot, "путь", "800", "", "")];
        let g = row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &rows, 300.0, 6.0).unwrap();
        let on = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        let off = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts {
                leader: false,
                ..RowOpts::default()
            },
        );
        assert!(on.leader.is_some());
        assert!(off.leader.is_none());
        assert_eq!(on.value, off.value);
        assert_eq!(on.label, off.label);
    }

    /// D-15: `paint_row` — порядок items = фон → маркер → текст → лидер →
    /// значение → бейдж (draw-порядок), цвета — из RowStyle дословно.
    #[test]
    fn paint_row_emits_items_in_draw_order() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let rows = [
            row_parts(RowMarker::Dot, "a", "800", "rps", "← ист"),
            row_parts(RowMarker::Dot, "a", "800", "rps", "← ист"),
        ];
        let g = row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &rows, 300.0, 6.0).unwrap();
        let lay = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        let style = row_style(KitState::Normal, &palette_a());
        let mut p = Painter::new();
        paint_row(&mut p, &lay, &rows[0], &style, ROW_SIZE);
        let items = p.items();
        // фон, точка, label, ≥1 штрих, value, unit, пилюля, текст бейджа
        assert!(
            items.len() >= 7,
            "ожидались все слои строки: {}",
            items.len()
        );
        assert!(matches!(items[0], PaintItem::Rect { .. }), "фон — rect");
        assert!(matches!(items[1], PaintItem::Rect { .. }), "точка — rect");
        assert!(matches!(items[2], PaintItem::Text { .. }), "label — text");
        // штрихи между label и value — rect с цветом маркера
        let dashes = items[3..]
            .iter()
            .take_while(|i| matches!(i, PaintItem::Rect { .. }))
            .count();
        assert!(dashes >= 1, "лидер — хотя бы один штрих");
        assert!(
            matches!(items[3 + dashes], PaintItem::Text { .. }),
            "value — text"
        );
        let last = items.last().unwrap();
        match last {
            PaintItem::Text { text, .. } => assert_eq!(text, "← ист"),
            other => panic!("последний — текст бейджа, получено {:?}", other),
        }
    }

    /// D-5/этап E: `leader_dash_rects` — та же арифметика, что в теле ноды
    /// (штрих TABLE_LEADER_DASH, шаг DASH+GAP, min_track); детерминизм.
    #[test]
    fn leader_dashes_match_node_arithmetic() {
        // Прежний цикл text.rs: x от x0 с шагом (DASH+GAP)·z, пока x+DASH·z <= x1
        let z = 1.25_f32;
        let (x0, x1, y) = (10.0_f32, 200.0_f32, 13.75_f32);
        let expected = {
            let mut v = Vec::new();
            let step = (canvas_core::tokens::TABLE_LEADER_DASH
                + canvas_core::tokens::TABLE_LEADER_GAP)
                * z;
            let mut x = x0;
            while x + canvas_core::tokens::TABLE_LEADER_DASH * z <= x1 {
                v.push([
                    x,
                    y,
                    canvas_core::tokens::TABLE_LEADER_DASH * z,
                    canvas_core::tokens::TABLE_LEADER_H * z,
                ]);
                x += step;
            }
            v
        };
        let got = leader_dash_rects(x0, x1, y, z, 6.0);
        assert_eq!(got.len(), expected.len());
        for (a, b) in got.iter().zip(&expected) {
            assert_eq!(a, b);
        }
        // Дорожка короче минимума — штрихов нет
        assert!(leader_dash_rects(0.0, 5.0, y, 1.0, 6.0).is_empty());
    }

    /// D-15: `row_style` — состояния берут слоты состояний (Selected —
    /// selected_fill + accent, Disabled — disabled_text), Normal —
    /// прозрачный фон (зебра — решение потребителя).
    #[test]
    fn row_style_uses_state_slots() {
        let p = palette_a();
        let normal = row_style(KitState::Normal, &p);
        assert_eq!(normal.fill, [0.0; 4], "Normal — прозрачная строка");
        assert_eq!(normal.label, p.text);
        assert_eq!(normal.unit, p.text_muted);
        let selected = row_style(KitState::Selected, &p);
        assert_eq!(selected.fill, p.selected_fill);
        assert_eq!(selected.border, p.accent);
        let disabled = row_style(KitState::Disabled, &p);
        assert_eq!(disabled.label, p.disabled_text);
        assert_eq!(disabled.badge_fill, [0.0; 4]);
    }
}
