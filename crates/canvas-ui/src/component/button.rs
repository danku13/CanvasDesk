//! FR-068 W3: кнопка/чип/свитч/иконка — перенос из kit.rs 1:1 (W3).
//!
//! Три слоя модуля (карта):
//! 1. Kit-функции (immediate): [`button_size`]/[`button_layout`]/
//!    [`button_style`] + icon-кнопка/чип/свитч — стабильный API кита
//!    (§Контракт-1 PRD-0009 V-5).
//! 2. Sized-API (issue #67 V1): [`button_layout_sized`]/
//!    [`chip_layout_sized`]/[`icon_button_rect_sized`] — шкала
//!    [`ControlSize`] с паритетом Sm == legacy.
//! 3. Component-слой (retained): [`ButtonProps`]/[`Button`] +
//!    `impl Component` — машина состояний FR-057 ([`WidgetState`]);
//!    paint: слоты [`button_style`] + state-layer M3 (Wave T §5.1.1,
//!    только состояния без собственного fill-слота) + focus-ring
//!    FR-057 (Wave T §5.1.2, keyboard-origin).

use super::{
    state_layer_alpha, touch_expand, ButtonVariant, Component, ComponentHit, ControlSize,
    ControlStyle, KitPalette, KitState, BUTTON_HEIGHT, BUTTON_PAD_H, CHIP_HEIGHT, CHIP_PAD_H,
    ICON_BUTTON_SIZE, MIN_TOUCH_TARGET, SWITCH_H, SWITCH_KNOB_PAD, SWITCH_W,
};
use crate::geometry::{UiPoint, UiRect, UiVec2};
use crate::layout::{stack, HAlign, LayoutBackend, VAlign};
use crate::measure::TextMeasurer;
use crate::paint::Painter;
use crate::widget::WidgetState;

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

/// Кнопка в слоте с явной шкалой размера (issue #67 V1: demo-витрина
/// уважает [`ControlSize`] — высота через [`ControlSize::button_h`], не
/// литерал `BUTTON_HEIGHT`). Ширина/усечение — контракт [`button_layout`]
/// (пад канона BUTTON_PAD_H — эталоны V3 витрины заморожены); при
/// `ControlSize::Sm` высота совпадает с прежней константой (паритет
/// покрыт тестом `sized_button_parity_with_legacy`). Кегль замера —
/// потребитель; размерная шкала кегля — [`ControlSize::font_size`]
/// (полировка 2026-10, проводка в Component-путь — `Button::layout`).
#[allow(clippy::too_many_arguments)] // прецедент button_layout: layout-функции кита
pub fn button_layout_sized(
    slot: UiRect,
    label: &str,
    align: (HAlign, VAlign),
    control: ControlSize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> ButtonLayout {
    let desired = button_size(label, m, fs, family, size);
    let max_w = slot.w;
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
    let rect = stack(
        slot,
        UiVec2::new(width, control.button_h()),
        align.0,
        align.1,
    );
    ButtonLayout { rect, label: shown }
}

/// Стиль кнопки: слот заливки по варианту, слот hover/pressed по состоянию,
/// текст — слот по состоянию (disabled — свой слот). Никакой арифметики
/// над цветами: только выбор слота.
pub fn button_style(variant: ButtonVariant, state: KitState, p: &KitPalette) -> ControlStyle {
    // Wave T §5.1.3: + Tertiary, Text, Inverse variants.
    let base_fill = match variant {
        ButtonVariant::Primary => p.control_primary,
        ButtonVariant::Danger => p.control_danger,
        ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Tertiary => p.control_fill,
        // Text — прозрачный контейнер (M3 text button).
        ButtonVariant::Text => [0.0, 0.0, 0.0, 0.0],
        // Inverse — на цветном фоне; используем accent как «inverse» fill.
        ButtonVariant::Inverse => p.accent,
    };
    let fill = match state {
        KitState::Normal
        | KitState::Selected
        | KitState::Focused
        | KitState::Dragged
        | KitState::Error => base_fill,
        KitState::Hovered => match variant {
            ButtonVariant::Primary | ButtonVariant::Inverse => p.primary_hover_fill,
            ButtonVariant::Secondary
            | ButtonVariant::Ghost
            | ButtonVariant::Tertiary
            | ButtonVariant::Text
            | ButtonVariant::Danger => p.hover_fill,
        },
        KitState::Pressed => match variant {
            ButtonVariant::Primary | ButtonVariant::Inverse => p.primary_hover_fill,
            _ => p.hover_fill,
        },
        KitState::Disabled => base_fill,
    };
    let border = match (variant, state) {
        (ButtonVariant::Ghost | ButtonVariant::Text, KitState::Hovered | KitState::Pressed) => {
            p.accent
        }
        (ButtonVariant::Ghost | ButtonVariant::Text, _) => [0.0, 0.0, 0.0, 0.0],
        _ => p.control_border,
    };
    let text = match state {
        KitState::Disabled => p.disabled_text,
        _ => match variant {
            ButtonVariant::Ghost | ButtonVariant::Text => p.text,
            ButtonVariant::Inverse => p.text_title,
            _ => p.text_title,
        },
    };
    ControlStyle {
        fill,
        border,
        text,
        radius: canvas_core::tokens::RADIUS_CHIP,
        elevation: super::Elevation::None,
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

/// Квадратная icon-кнопка со шкалой размера (issue #67 V1): сторона —
/// [`ControlSize::icon_btn`]; при `ControlSize::Sm` — паритет с
/// [`icon_button_rect`] (константа `ICON_BUTTON_SIZE`), покрыт тестом.
pub fn icon_button_rect_sized(
    slot: UiRect,
    align: (HAlign, VAlign),
    control: ControlSize,
) -> UiRect {
    let side = control.icon_btn();
    stack(slot, UiVec2::new(side, side), align.0, align.1)
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

/// Чип со шкалой размера (issue #67 V1): высота — [`ControlSize::chip_h`],
/// ширина/усечение — контракт [`chip_layout`]; при `ControlSize::Sm` —
/// паритет с [`chip_layout`] (константа `CHIP_HEIGHT`), покрыт тестом.
#[allow(clippy::too_many_arguments)] // прецедент chip_layout: layout-функции кита
pub fn chip_layout_sized(
    origin: UiPoint,
    label: &str,
    max_width: f32,
    control: ControlSize,
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
        rect: UiRect::new(origin.x, origin.y, w, control.chip_h()),
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
        elevation: super::Elevation::None,
    }
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
        elevation: super::Elevation::None,
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

/// FR-ICONS: строковый идентификатор иконки для SVG-атласа (без набора —
/// набор задаётся отдельной настройкой `IconStyle`). Маппинг 1:1 с `Icon`
/// (тест `icon_name_mapping_is_complete`). Идентификаторы стабильны:
/// `close`, `gear`, `question`, `search`, `plus`, `arrow_left`,
/// `arrow_right`, `refresh` — соответствуют именам файлов в
/// `assets/icons/<set>/<name>.svg`.
pub fn icon_name(i: Icon) -> &'static str {
    match i {
        Icon::Close => "close",
        Icon::Gear => "gear",
        Icon::Question => "question",
        Icon::Search => "search",
        Icon::Plus => "plus",
        Icon::ArrowLeft => "arrow_left",
        Icon::ArrowRight => "arrow_right",
        Icon::Refresh => "refresh",
    }
}

// --- StageCloseButton (FR-070) ----------------------------------------------

/// Кнопка «×» в правом-верхнем углу панели/модали (FR-070, audit §6.1 —
/// самый повторяющийся паттерн: 7+ файлов дублировали ручную формулу
/// `x = panel.right() - 28.0, y = panel.y + 6.0` с дрейфом inset/size
/// между 22/24/26/28/30 — канонический размер [`ICON_BUTTON_SIZE`],
/// inset — [`canvas_core::tokens::SPACING_SM`] = 8).
///
/// Возвращает `UiRect` угловой кнопки в слоте `slot` (panel/modal rect);
/// позиция — «прижата к правому-верхнему углу слота» минус inset, размер —
/// квадрат `ICON_BUTTON_SIZE × ICON_BUTTON_SIZE` (та же метрика, что у
/// [`icon_button_rect`] с `HAlign::End, VAlign::Start`, но с явным inset —
/// не вплотную к углу).
///
/// Контракт F-8: цвет/стиль кнопки — забота потребителя
/// ([`icon_button_style`] с `KitState`/`KitPalette`); здесь — только
/// геометрия (как у [`icon_button_rect`]).
pub fn stage_close_button(slot: UiRect) -> UiRect {
    let s = ICON_BUTTON_SIZE;
    let inset = canvas_core::tokens::SPACING_SM;
    UiRect::new(slot.right() - s - inset, slot.y + inset, s, s)
}

/// LG-вариант [`stage_close_button`] — 30×30 (вместо канонического
/// [`ICON_BUTTON_SIZE`] = 26), тот же inset [`canvas_core::tokens::SPACING_SM`]
/// = 8. Выделен для dialog-шапок с HEADER_H = 56–58 (autolink/explain —
/// FR-060/FR-059 documented deviation): крупная кнопка визуально
/// сбалансирована с высоким заголовком; канонический 26 выглядит мелко.
///
/// **Визуальный скачок при миграции** с hand-rolled (inset 14 от прежнего
/// `panel.right() - 14 - 30`/`panel.y + 14`) на канон `SPACING_SM`=8 —
/// позиция кнопки сдвигается на ~6px по диагонали (вверх+вправо, к углу
/// панели). Это каноническое направление kit (FR-070/§6.1 — «× в углу
/// панели с inset `SPACING_SM`»); прежний inset 14 был наследием ручной
/// вертикальной центровки в HEADER_H=58 (где `(58−30)/2=14`). Когда
/// `HEADER_H` мигрирует на `PANEL_HEADER_H_L`=44 (отдельный TODO W-d),
/// `(44−30)/2`=7 — почти совпадает с `SPACING_SM`=8, и LG-вариант можно
/// будет пересмотреть (возможно, свернуть обратно в [`stage_close_button`]).
///
/// Контракт F-8 (как у [`stage_close_button`]): цвет/стиль — забота
/// потребителя (через [`icon_button_style`] с `KitState`/`KitPalette`);
/// здесь — только геометрия.
pub fn stage_close_button_lg(slot: UiRect) -> UiRect {
    let s = 30.0;
    let inset = canvas_core::tokens::SPACING_SM;
    UiRect::new(slot.right() - s - inset, slot.y + inset, s, s)
}

// === Component (FR-068 W3, агент 3-a) =======================================

/// Семейство подписи по умолчанию для [`Component::layout`] кнопки — то же
/// имя, что `Family::Name` screen-текстов рендера (паритет `sans_attrs`,
/// CR-015: замер тем же лицом, что и отрисовка).
pub const BUTTON_FAMILY: &str = "Noto Sans Display";

/// Кегль подписи по умолчанию для [`Component::layout`] кнопки (ui px) —
/// прежний кегль подписей контролов поверхностей (13.0). Потребитель с
/// собственным шрифтовым пулом передаёт свой кегль напрямую в
/// [`button_layout`] — значения здесь только для standalone-пути.
pub const BUTTON_FONT_SIZE: f32 = 13.0;

/// Свойства кнопки — декларативный вход кадра (FR-068 W3).
#[derive(Debug, Clone, PartialEq)]
pub struct ButtonProps {
    /// Подпись (узкий слот — ellipsis в [`button_layout`], деградация
    /// видна потребителю, не молчаливый срез).
    pub label: String,
    /// Вариант (семантика действия → слот заливки [`button_style`]).
    pub variant: ButtonVariant,
    /// Палитра-срез: цвета — только слоты (контракт F-8 PRD-0009 §8).
    pub palette: KitPalette,
    /// Недоступна: [`Button::new`] транслирует флаг в
    /// [`WidgetState::set_disabled`] (клик игнорируется — контракт
    /// [`KitState::Disabled`]). Смена между кадрами — [`Button::set_disabled`].
    pub disabled: bool,
    /// Шкала размера (issue #67 V1 + полировка 2026-10): высота layout —
    /// [`ControlSize::button_h`]; кегль подписи — размерная шкала
    /// [`ControlSize::font_size`] (Component-путь, замер тем же кеглем,
    /// что и подпись потребителя — CR-015); `Sm` — канон == legacy
    /// (паритет — тест `component_button_props_new_defaults_to_sm_parity`).
    /// Пад ширины в sized-layout — канон BUTTON_PAD_H (эталоны V3
    /// витрины заморожены; пад-шкала [`ControlSize::pad_h`] — к обработке
    /// потребителем).
    pub size: ControlSize,
    /// LAY8.3: hit-зона расширяется до [`MIN_TOUCH_TARGET`] центрированно
    /// (hit-only — визуал/геометрия прежние, [`Button::hit_test`]).
    /// Coarse-платформа — решение потребителя (FR-097 `POINTER_COARSE`):
    /// включайте на тач-вводе; кламп в контейнер — [`super::touch_hit`]
    /// у потребителя (компонент не знает своего контейнера).
    pub touch_hit: bool,
}

impl ButtonProps {
    /// Удобный конструктор: `size = ControlSize::Sm`, `disabled = false`,
    /// `touch_hit = false` (канон == legacy-кнопка [`BUTTON_HEIGHT`]);
    /// остальные поля — обновлением структуры.
    pub fn new(label: impl Into<String>, variant: ButtonVariant, palette: KitPalette) -> Self {
        Self {
            label: label.into(),
            variant,
            palette,
            disabled: false,
            size: ControlSize::Sm,
            touch_hit: false,
        }
    }
}

/// Кнопка как [`Component`] — retained-объект (FR-068 §W3): `props` —
/// вход кадра, `state` — машина состояний FR-057 (потребитель двигает
/// `set_pointer`/`clicked` по событиям указателя; интерактивность —
/// потому `WidgetState` обязателен).
#[derive(Debug, Clone, PartialEq)]
pub struct Button {
    /// Свойства кадра.
    pub props: ButtonProps,
    /// Состояние виджета (hover/pressed/disabled/... — матрица
    /// приоритетов [`WidgetState::kit_state`]).
    pub state: WidgetState,
}

impl Button {
    /// Новая кнопка: `props.disabled` прошивается в состояние виджета —
    /// дальше стиль (`paint`) и ребро клика (`WidgetState::clicked`)
    /// читают одно и то же состояние. Смена `disabled` между кадрами —
    /// через [`Button::set_disabled`] или пересоздание.
    pub fn new(props: ButtonProps) -> Self {
        let mut state = WidgetState::default();
        state.set_disabled(props.disabled);
        Self { props, state }
    }

    /// Смена disabled между кадрами без пересоздания: props и состояние
    /// обновляются синхронно — стиль ([`button_style`] через `paint`) и
    /// ребро клика ([`WidgetState::clicked`]) читают одно и то же
    /// ([`KitState::Disabled`]: клик игнорируется).
    pub fn set_disabled(&mut self, disabled: bool) {
        self.props.disabled = disabled;
        self.state.set_disabled(disabled);
    }
}

impl Component for Button {
    type Props = ButtonProps;

    fn props(&self) -> &ButtonProps {
        &self.props
    }

    /// Один rect — сама кнопка: делегирование [`button_layout_sized`] 1:1
    /// (размер = подпись + 2·[`BUTTON_PAD_H`], высота [`ControlSize::button_h`]
    /// от `props.size` — при `Sm` минимум [`BUTTON_HEIGHT`]; узкий слот —
    /// сжатие до ширины слота + ellipsis подписи). Порядок rects: `rects[0]`
    /// — кнопка; других rect'ов нет.
    ///
    /// Выравнивание в слоте — Center/Center (как у кнопок диалога/шапки);
    /// семейство — [`BUTTON_FAMILY`]; кегль — размерная шкала
    /// `props.size` ([`ControlSize::font_size`]: замер тем же кеглем, что
    /// и подпись потребителя — CR-015; Sm — прежние числа 13/30/12,
    /// бит-в-бит). Высота — [`ControlSize::button_h`].
    /// `backend` не участвует: layout — stack одного блока, контейнерной
    /// вёрстки нет (сигнатуры kit-функций НЕ менялись — §Контракт-1 FR-068).
    ///
    /// Standalone-семантика: измеритель и шрифтовой пул создаются на
    /// вызов (immediate, без retained-состояния — решение FR-068 §W3);
    /// горячий путь потребителя — [`button_layout_sized`] с собственным
    /// пулом (владелец пула — canvas-render::text, PRD-0009 §14/Q6).
    fn layout(&self, _backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let bl = button_layout_sized(
            slot,
            &self.props.label,
            (HAlign::Center, VAlign::Center),
            self.props.size,
            &mut m,
            &mut fs,
            BUTTON_FAMILY,
            self.props.size.font_size(),
        );
        vec![bl.rect]
    }

    /// Рисует `rects[0]` (rect из [`Component::layout`]):
    ///
    /// 1. Заливка/рамка/радиус — [`button_style`] по варианту и состоянию
    ///    [`WidgetState::kit_state`] (disabled прошит в state при
    ///    [`Button::new`]); hover/pressed отвечают слотами fill — это
    ///    задокументированное отклонение от чистого M3 (state-layer только
    ///    там, где слотового ответа нет).
    /// 2. State-layer M3 (Wave T §5.1.1) — overlay-квад слота `text` с
    ///    альфой [`state_layer_alpha`] для состояний БЕЗ собственного
    ///    fill-слота: keyboard-focus (10%, `:focus-visible`) и dragged
    ///    (16%); hover/pressed overlay не дублируют (задвоение отклика
    ///    исключено).
    /// 3. Focus-ring FR-057 (Wave T §5.1.2) — контур слота `accent` при
    ///    [`WidgetState::focus_ring_visible`] (keyboard-origin; та же
    ///    механика, что у K4-витрины). Компонент владеет состоянием —
    ///    ring рисует сам; [`button_style`] остаётся ring-агностиком.
    ///
    /// Подпись — забота потребителя (`Painter::label` слотом `style.text`;
    /// усечённая подпись живёт в [`button_layout`], кит цвет от геометрии
    /// не отделяет — F-8). Пустой `rects` — no-op (деградация, не паника).
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&rect) = rects.first() else {
            return;
        };
        let state = self.state.kit_state();
        let style = button_style(self.props.variant, state, &self.props.palette);
        painter.control(rect, &style);
        // Focus/drag-отклик (M3 state-layer): только состояния без
        // собственного fill-слота; максимум из активных (§5.1.1 — max,
        // не сумма).
        let mut alpha: f32 = 0.0;
        if self.state.focus_ring_visible() {
            alpha = alpha.max(state_layer_alpha(KitState::Focused));
        }
        if self.state.is_dragged() {
            alpha = alpha.max(state_layer_alpha(KitState::Dragged));
        }
        if alpha > 0.0 {
            let t = self.props.palette.text;
            painter.rect(rect, [t[0], t[1], t[2], alpha], [0.0; 4], style.radius);
        }
        // Focus-ring (FR-057 §5.1.2): keyboard-origin only, контур accent.
        if self.state.focus_ring_visible() {
            painter.rect(rect, [0.0; 4], self.props.palette.accent, style.radius);
        }
    }

    // hit_test — LAY8.3: при `props.touch_hit` точка проверяется против
    // hit-зоны `rects[0]`, расширенной [`touch_expand`] до
    // [`MIN_TOUCH_TARGET`] центрированно (hit-only: рисование/геометрия
    // прежние — расширяется только зона попадания). Кламп в контейнер —
    // забота потребителя ([`super::touch_hit`]: компонент не знает
    // контейнера; FR-097). Без флага — дефолт трейта (бит-в-бит):
    // первый rect, содержащий точку.
    fn hit_test(&self, rects: &[UiRect], point: UiPoint) -> Option<ComponentHit> {
        if !self.props.touch_hit {
            return ComponentHit::pick(rects, point);
        }
        let rect = *rects.first()?;
        touch_expand(rect, MIN_TOUCH_TARGET)
            .contains(point)
            .then_some(ComponentHit { index: 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::panel::panel_style;
    use crate::component::test_support::{palette_a, palette_b, FAMILY};
    use crate::component::{BUTTON_HEIGHT, BUTTON_PAD_H};
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

    // === FR-070: stage_close_button (кнопка «×» в углу панели) =============

    /// Геометрия канонической угловой кнопки закрытия: квадрат
    /// `ICON_BUTTON_SIZE × ICON_BUTTON_SIZE`, прижата к правому-верхнему
    /// углу слота с отступом `SPACING_SM` (= 8); `right == slot.right() -
    /// inset - size`, `top == slot.y + inset`. Контракт F-8: цвет/стиль —
    /// отдельно (через [`icon_button_style`]); здесь — геометрия.
    #[test]
    fn stage_close_button_is_inset_in_top_right_corner() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let r = stage_close_button(slot);
        let inset = canvas_core::tokens::SPACING_SM;
        assert_eq!(r.w, ICON_BUTTON_SIZE);
        assert_eq!(r.h, ICON_BUTTON_SIZE);
        assert!(
            (r.right() - (slot.right() - inset)).abs() < 0.01,
            "правый край кнопки = slot.right() - inset"
        );
        assert!(
            (r.y - (slot.y + inset)).abs() < 0.01,
            "верх = slot.y + inset"
        );
        // Кнопка внутри слота (не вылезает за границы)
        assert!(r.x >= slot.x && r.right() <= slot.right() + 0.01);
        assert!(r.y >= slot.y && r.bottom() <= slot.bottom() + 0.01);
        // Положение квадрата = правый-верхний угол с inset (формула из аудита
        // §6.1: канон `ICON_BUTTON_SIZE`=26 + `SPACING_SM`=8).
        assert_eq!(
            r,
            UiRect::new(
                slot.right() - ICON_BUTTON_SIZE - inset,
                slot.y + inset,
                ICON_BUTTON_SIZE,
                ICON_BUTTON_SIZE,
            )
        );
    }

    /// Каноническая формула не зависит от позиции/размера слота — invariant
    /// «прижата к правому-верхнему углу»: для слота со смещением и другой
    /// геометрией формула та же.
    #[test]
    fn stage_close_button_inset_invariant_for_arbitrary_slot() {
        let slot = UiRect::new(120.0, 80.0, 500.0, 400.0);
        let r = stage_close_button(slot);
        let inset = canvas_core::tokens::SPACING_SM;
        assert!((r.right() - (slot.right() - inset)).abs() < 0.01);
        assert!((r.y - (slot.y + inset)).abs() < 0.01);
        assert_eq!(r.w, ICON_BUTTON_SIZE);
        assert_eq!(r.h, ICON_BUTTON_SIZE);
    }

    // === FR-070: stage_close_button_lg (LG-вариант для tall-header dialogs) ==

    /// LG-вариант: 30×30 (а не [`ICON_BUTTON_SIZE`] = 26), inset
    /// `SPACING_SM`=8 — как у [`stage_close_button`], но больше размер.
    /// Контракт: прижата к правому-верхнему углу slot с тем же inset; паритет
    /// формулы с [`stage_close_button`] кроме `s = 30.0`.
    #[test]
    fn stage_close_button_lg_is_30x30_in_top_right_corner() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let r = stage_close_button_lg(slot);
        let inset = canvas_core::tokens::SPACING_SM;
        assert_eq!(r.w, 30.0);
        assert_eq!(r.h, 30.0);
        assert!(
            (r.right() - (slot.right() - inset)).abs() < 0.01,
            "правый край = slot.right() - inset (тот же inset, что у canonical)"
        );
        assert!(
            (r.y - (slot.y + inset)).abs() < 0.01,
            "верх = slot.y + inset"
        );
        assert!(r.x >= slot.x && r.right() <= slot.right() + 0.01);
        assert!(r.y >= slot.y && r.bottom() <= slot.bottom() + 0.01);
        assert_eq!(
            r,
            UiRect::new(slot.right() - 30.0 - inset, slot.y + inset, 30.0, 30.0),
        );
    }

    /// Инвариант «прижата к правому-верхнему углу» для LG-варианта на
    /// произвольном слоте (как [`stage_close_button_inset_invariant_for_arbitrary_slot`]).
    #[test]
    fn stage_close_button_lg_invariant_for_arbitrary_slot() {
        let slot = UiRect::new(120.0, 80.0, 500.0, 400.0);
        let r = stage_close_button_lg(slot);
        let inset = canvas_core::tokens::SPACING_SM;
        assert!((r.right() - (slot.right() - inset)).abs() < 0.01);
        assert!((r.y - (slot.y + inset)).abs() < 0.01);
        assert_eq!(r.w, 30.0);
        assert_eq!(r.h, 30.0);
    }

    // === FR-058: тесты компонентов v2 =======================================

    // --- TextFieldModel: insert/backspace/delete/move/select/set_text -------
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

    // === FR-062 F-17: focus_order ===

    // === FR-068 W3: Component Button (агент 3-a) ============================
    use crate::component::ComponentHit;
    use crate::layout::default_backend;
    use crate::paint::PaintItem;

    /// Component::layout: ровно один rect (сама кнопка), внутри слота,
    /// по центру (Center/Center); пустая подпись — минимальный квадрат
    /// BUTTON_HEIGHT (шрифто-независимо: 0 ширина текста).
    #[test]
    fn component_button_layout_single_rect_in_slot() {
        let slot = UiRect::new(10.0, 20.0, 400.0, 60.0);
        let btn = Button::new(ButtonProps::new("ОО", ButtonVariant::Primary, palette_a()));
        let rects = btn.layout(default_backend(), slot);
        assert_eq!(rects.len(), 1, "кнопка — один rect");
        let r = rects[0];
        assert_eq!(r.h, BUTTON_HEIGHT);
        assert!(r.w >= BUTTON_HEIGHT, "ширина не ниже минимума");
        assert!(
            r.x >= slot.x && r.right() <= slot.right() + 0.01,
            "в слоте по X"
        );
        assert!(
            r.y >= slot.y && r.bottom() <= slot.bottom() + 0.01,
            "в слоте по Y"
        );
        // Center/Center: слева/сверху поровну свободного места.
        assert!((r.x - slot.x - (slot.w - r.w) / 2.0).abs() < 0.01);
        assert!((r.y - slot.y - (slot.h - r.h) / 2.0).abs() < 0.01);
        // Пустая подпись — квадрат BUTTON_HEIGHT×BUTTON_HEIGHT (без
        // шейпинга: ширина текста 0 — детерминировано на любом CI).
        let empty = Button::new(ButtonProps {
            label: String::new(),
            ..btn.props.clone()
        });
        let rects = empty.layout(default_backend(), slot);
        assert_eq!(rects[0].w, BUTTON_HEIGHT);
        assert_eq!(rects[0].h, BUTTON_HEIGHT);
    }

    /// Component::paint: один Rect со слотами [`button_style`] —
    /// Primary/Normal → control_primary, hover (из WidgetState) →
    /// primary_hover_fill; props.disabled прошит в состояние при new;
    /// пустой rects — no-op.
    #[test]
    fn component_button_paint_emits_style_rect() {
        let pal = palette_a();
        let mut btn = Button::new(ButtonProps::new("ОК", ButtonVariant::Primary, pal));
        let rects = vec![UiRect::new(0.0, 0.0, 100.0, BUTTON_HEIGHT)];
        let mut painter = Painter::new();
        btn.paint(&mut painter, &rects);
        assert_eq!(painter.items().len(), 1, "контрольный rect — один item");
        assert_eq!(
            painter.items()[0],
            PaintItem::Rect {
                rect: rects[0],
                fill: pal.control_primary,
                border: pal.control_border,
                radius: canvas_core::tokens::RADIUS_CHIP,
            }
        );
        // Hover — из WidgetState (state и props разделены: стиль меняет
        // состояние, а не свойства).
        btn.state.set_pointer(true, false);
        let mut painter = Painter::new();
        btn.paint(&mut painter, &rects);
        assert_eq!(
            painter.items()[0],
            PaintItem::Rect {
                rect: rects[0],
                fill: pal.primary_hover_fill,
                border: pal.control_border,
                radius: canvas_core::tokens::RADIUS_CHIP,
            }
        );
        // disabled-проп → KitState::Disabled (прошивка в new).
        let dis = Button::new(ButtonProps {
            disabled: true,
            ..btn.props.clone()
        });
        assert_eq!(dis.state.kit_state(), KitState::Disabled);
        // Пустой rects — no-op, не паника.
        let mut painter = Painter::new();
        dis.paint(&mut painter, &[]);
        assert!(painter.items().is_empty());
    }

    /// Component::hit_test (дефолт из трейта): точка внутри rect →
    /// `ComponentHit { index: 0 }`, вне — None.
    #[test]
    fn component_button_hit_test_picks_first_rect() {
        let btn = Button::new(ButtonProps::new(
            "ОК",
            ButtonVariant::Secondary,
            palette_a(),
        ));
        let rects = btn.layout(default_backend(), UiRect::new(0.0, 0.0, 400.0, 60.0));
        assert_eq!(rects.len(), 1);
        let inside = UiPoint::new(rects[0].x + 1.0, rects[0].y + 1.0);
        assert_eq!(
            btn.hit_test(&rects, inside),
            Some(ComponentHit { index: 0 })
        );
        let outside = UiPoint::new(rects[0].right() + 10.0, rects[0].y + 1.0);
        assert_eq!(btn.hit_test(&rects, outside), None);
    }

    // --- issue #67 V1: sized-варианты (шкала ControlSize) -------------------

    /// Паритет: sized при `Sm` === legacy (высоты — прежние константы).
    /// Гарантия «AS-IS поверхности не дрейфуют» при переходе витрины на
    /// sized-API (issue #67 V1).
    #[test]
    fn sized_button_parity_with_legacy() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 60.0);
        let legacy = button_layout(
            slot,
            "Применить",
            (HAlign::Start, VAlign::Center),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        let sized = button_layout_sized(
            slot,
            "Применить",
            (HAlign::Start, VAlign::Center),
            ControlSize::Sm,
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(legacy.rect, sized.rect);
        assert_eq!(legacy.label, sized.label);
        // Иконка/чип — тот же паритет
        assert_eq!(
            icon_button_rect(slot, (HAlign::Center, VAlign::Center)),
            icon_button_rect_sized(slot, (HAlign::Center, VAlign::Center), ControlSize::Sm)
        );
        let chip_legacy = chip_layout(
            UiPoint::new(0.0, 0.0),
            "Поиск",
            160.0,
            &mut m,
            &mut fs,
            FAMILY,
            12.0,
        );
        let chip_sized = chip_layout_sized(
            UiPoint::new(0.0, 0.0),
            "Поиск",
            160.0,
            ControlSize::Sm,
            &mut m,
            &mut fs,
            FAMILY,
            12.0,
        );
        assert_eq!(chip_legacy.rect, chip_sized.rect);
    }

    /// Шкала размеров: высоты sized-вариантов = методы `ControlSize`
    /// (button 24/30/36/44, chip 20/24/28/32, icon 20/26/32/44).
    #[test]
    fn sized_variants_follow_control_size_scale() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        for size in [
            ControlSize::Xs,
            ControlSize::Sm,
            ControlSize::Md,
            ControlSize::Lg,
        ] {
            let btn = button_layout_sized(
                slot,
                "ОК",
                (HAlign::Start, VAlign::Center),
                size,
                &mut m,
                &mut fs,
                FAMILY,
                13.0,
            );
            assert_eq!(btn.rect.h, size.button_h(), "button_h {size:?}");
            let chip = chip_layout_sized(
                UiPoint::new(0.0, 0.0),
                "Чип",
                160.0,
                size,
                &mut m,
                &mut fs,
                FAMILY,
                12.0,
            );
            assert_eq!(chip.rect.h, size.chip_h(), "chip_h {size:?}");
            let icon = icon_button_rect_sized(slot, (HAlign::Start, VAlign::Center), size);
            assert_eq!(icon.w, size.icon_btn(), "icon_btn {size:?}");
            assert_eq!(icon.h, size.icon_btn(), "icon_btn {size:?}");
        }
    }

    // === Полировка компонента (kit-реэкспорт, ControlSize, state-layer M3,
    // focus-ring FR-057) =====================================================

    /// `ButtonProps::new`: `size = ControlSize::Sm`, `disabled = false` —
    /// канон == legacy: высота layout == [`BUTTON_HEIGHT`], один rect.
    #[test]
    fn component_button_props_new_defaults_to_sm_parity() {
        let btn = Button::new(ButtonProps::new("ОК", ButtonVariant::Primary, palette_a()));
        assert_eq!(btn.props.size, ControlSize::Sm);
        assert!(!btn.props.disabled);
        let rects = btn.layout(default_backend(), UiRect::new(0.0, 0.0, 300.0, 60.0));
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].h, BUTTON_HEIGHT);
    }

    /// Шкала размеров на уровне компонента (issue #67 V1): layout уважает
    /// `props.size` — высоты [`ControlSize::button_h`] (24/30/36/44).
    #[test]
    fn component_button_layout_respects_control_size() {
        let slot = UiRect::new(0.0, 0.0, 300.0, 80.0);
        for size in [
            ControlSize::Xs,
            ControlSize::Sm,
            ControlSize::Md,
            ControlSize::Lg,
        ] {
            let mut props = ButtonProps::new("ОК", ButtonVariant::Secondary, palette_a());
            props.size = size;
            let rects = Button::new(props).layout(default_backend(), slot);
            assert_eq!(rects[0].h, size.button_h(), "button_h {size:?}");
        }
    }

    /// `Button::set_disabled`: props и состояние обновляются синхронно;
    /// ребро клика гасится ([`WidgetState::clicked`] игнорирует disabled).
    #[test]
    fn button_set_disabled_syncs_props_state_and_click_edge() {
        let mut btn = Button::new(ButtonProps::new("ОК", ButtonVariant::Primary, palette_a()));
        btn.state.set_pointer(true, true); // press внутри → ребро armed
        btn.set_disabled(true);
        assert!(btn.props.disabled);
        assert_eq!(btn.state.kit_state(), KitState::Disabled);
        assert!(!btn.state.clicked(true), "disabled — клик игнорируется");
        btn.set_disabled(false);
        assert!(!btn.props.disabled);
        assert_ne!(btn.state.kit_state(), KitState::Disabled);
    }

    /// Focus-отклик в paint (Wave T §5.1.1/§5.1.2, FR-057) — только
    /// keyboard-origin: control + state-layer 10% (слот `text`) + ring
    /// (контур `accent`). Hover — ровно один item (слотовый отклик
    /// hover_fill, без задвоения); mouse-origin фокус и disabled — без
    /// focus-отклика.
    #[test]
    fn component_button_paint_focus_response_keyboard_origin_only() {
        let pal = palette_a();
        let rects = vec![UiRect::new(0.0, 0.0, 100.0, BUTTON_HEIGHT)];
        let paint_items = |btn: &Button| {
            let mut p = Painter::new();
            btn.paint(&mut p, &rects);
            p.items().to_vec()
        };

        // Hovered: один item — overlay не дублирует слотовый hover_fill.
        let mut hovered = Button::new(ButtonProps::new("ОК", ButtonVariant::Secondary, pal));
        hovered.state.set_pointer(true, false);
        assert_eq!(paint_items(&hovered).len(), 1);

        // Keyboard-origin focus: control + state-layer + focus-ring.
        let mut focused = Button::new(ButtonProps::new("ОК", ButtonVariant::Secondary, pal));
        focused.state.set_focused_visible(true, true);
        let items = paint_items(&focused);
        assert_eq!(items.len(), 3, "control + state-layer + focus-ring");
        match &items[1] {
            PaintItem::Rect {
                fill,
                border,
                radius,
                ..
            } => {
                let t = pal.text;
                assert_eq!(
                    *fill,
                    [t[0], t[1], t[2], state_layer_alpha(KitState::Focused)]
                );
                assert_eq!(*border, [0.0; 4]);
                assert_eq!(*radius, canvas_core::tokens::RADIUS_CHIP);
            }
            other => panic!("item[1] — overlay Rect, получен {other:?}"),
        }
        match &items[2] {
            PaintItem::Rect { fill, border, .. } => {
                assert_eq!(*fill, [0.0; 4]);
                assert_eq!(*border, pal.accent, "ring — контур accent");
            }
            other => panic!("item[2] — focus-ring Rect, получен {other:?}"),
        }

        // Mouse-origin (клик): ни overlay, ни ring (:focus-visible).
        let mut mouse = Button::new(ButtonProps::new("ОК", ButtonVariant::Secondary, pal));
        mouse.state.set_focused_visible(true, false);
        assert_eq!(paint_items(&mouse).len(), 1);

        // Disabled + фокус: отклик гасится (focus_ring_visible = false).
        let mut dis = Button::new(ButtonProps::new("ОК", ButtonVariant::Secondary, pal));
        dis.state.set_focused_visible(true, true);
        dis.state.set_disabled(true);
        assert_eq!(paint_items(&dis).len(), 1);
    }

    /// Dragged: state-layer 16% (M3) — control + overlay; ring не рисуется.
    #[test]
    fn component_button_paint_dragged_state_layer() {
        let pal = palette_a();
        let rects = vec![UiRect::new(0.0, 0.0, 100.0, BUTTON_HEIGHT)];
        let mut btn = Button::new(ButtonProps::new("ОК", ButtonVariant::Secondary, pal));
        btn.state.set_dragged(true);
        let mut p = Painter::new();
        btn.paint(&mut p, &rects);
        let items = p.items();
        assert_eq!(items.len(), 2, "control + state-layer 16%");
        match &items[1] {
            PaintItem::Rect { fill, border, .. } => {
                let t = pal.text;
                assert_eq!(
                    *fill,
                    [t[0], t[1], t[2], state_layer_alpha(KitState::Dragged)]
                );
                assert_eq!(*border, [0.0; 4]);
            }
            other => panic!("item[1] — overlay Rect, получен {other:?}"),
        }
    }

    // === Полировка кнопки 2026-10: размерные кегли + LAY8.3 =================

    /// Размерная типографика: кегли/пады по ступеням; Sm = канон
    /// BUTTON_FONT_SIZE/BUTTON_PAD_H (бит-в-бит прежним потребителям).
    #[test]
    fn control_size_label_typography_scale() {
        assert_eq!(ControlSize::Xs.font_size(), 11.0);
        assert_eq!(ControlSize::Sm.font_size(), BUTTON_FONT_SIZE);
        assert_eq!(ControlSize::Md.font_size(), 14.0);
        assert_eq!(ControlSize::Lg.font_size(), 15.0);
        assert_eq!(ControlSize::Xs.pad_h(), 8.0);
        assert_eq!(ControlSize::Sm.pad_h(), BUTTON_PAD_H);
        assert_eq!(ControlSize::Md.pad_h(), 14.0);
        assert_eq!(ControlSize::Lg.pad_h(), 16.0);
        // Воздух: пад не съедает кнопку ни на одной ступени
        for cs in [
            ControlSize::Xs,
            ControlSize::Sm,
            ControlSize::Md,
            ControlSize::Lg,
        ] {
            assert!(cs.pad_h() * 2.0 < cs.button_h(), "{cs:?}");
        }
    }

    /// Кегль Component-кнопки — размерная шкала: ширина layout измеряется
    /// кеглем `props.size.font_size()` (Sm — 13, канон; Lg — 15 → ширина
    /// больше, чем при замере 13, при той же высоте 44).
    #[test]
    fn component_button_layout_uses_size_font_scale() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let label = "Действие";
        let slot = UiRect::new(10.0, 20.0, 400.0, 80.0);
        let mut props = ButtonProps::new(label, ButtonVariant::Primary, palette_a());
        props.size = ControlSize::Lg;
        let rects = Button::new(props).layout(default_backend(), slot);
        assert_eq!(rects[0].h, 44.0, "высота Lg");
        let kegel = ControlSize::Lg.font_size();
        let expected_w = m.width_of(&mut fs, label, BUTTON_FAMILY, kegel) + BUTTON_PAD_H * 2.0;
        assert!(
            (rects[0].w - expected_w).abs() < 0.01,
            "ширина замерена кеглем шкалы Lg: {} vs {expected_w}",
            rects[0].w
        );
    }

    /// LAY8.3 hit-зона: touch_hit=true — точка вне визуала, но внутри
    /// расширенной зоны (44 центрированно) попадает; далеко — мимо;
    /// touch_hit=false — дефолт (бит-в-бит): вне визуала — None.
    #[test]
    fn button_touch_hit_zone_expands_hit_only() {
        let btn = Button::new(ButtonProps {
            touch_hit: true,
            ..ButtonProps::new("ОК", ButtonVariant::Primary, palette_a())
        });
        let rects = btn.layout(default_backend(), UiRect::new(0.0, 0.0, 400.0, 100.0));
        let r = rects[0];
        // Визуал 30 px высотой в слоте 100 (r.y=35, низ 65); зона 44:
        // 28..72. Точка на 5 px ниже/выше визуала — в зоне.
        let below = UiPoint::new(r.x + 5.0, r.bottom() + 5.0);
        assert_eq!(btn.hit_test(&rects, below), Some(ComponentHit { index: 0 }));
        let above = UiPoint::new(r.x + 5.0, r.y - 5.0);
        assert_eq!(btn.hit_test(&rects, above), Some(ComponentHit { index: 0 }));
        // Вне зоны (за пределами ±(44−30)/2 = 7) — None.
        let far = UiPoint::new(r.x + 5.0, r.bottom() + 20.0);
        assert_eq!(btn.hit_test(&rects, far), None);
        // Дефолт (touch_hit=false): вне визуала — None, внутри — Some
        // (бит-в-бит прежнему hit_test).
        let plain = Button::new(ButtonProps::new("ОК", ButtonVariant::Primary, palette_a()));
        assert_eq!(plain.hit_test(&rects, below), None);
        assert_eq!(
            plain.hit_test(&rects, UiPoint::new(r.x + 5.0, r.y + 5.0)),
            Some(ComponentHit { index: 0 }),
        );
    }
}
