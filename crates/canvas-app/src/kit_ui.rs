//! FR-055 (этап U4 PRD-0009, F-8): витрина кита — модель раскладки +
//! адаптер «модель кита → квад/текст кадра». Сборка кадра — метод
//! `App::kit_gallery_overlay` (app.rs, паттерн прочих оверлеев).
//!
//! Витрина = живой образец кита (Q5 PRD-0009: вариант (a) — пункт «О
//! интерфейсе» меню «?», доступна всегда; полезна для баг-репортов и
//! приёмки): компоненты × состояния × RU/EN × темы. Интерактив — только
//! кнопка темы (реальный kit::Button Primary) и «✕»; остальные контролы
//! показывают состояния статически (декоративные rect'ы — линт не считает
//! их интерактивными). Тема переключается на ходу — слоты палитры меняются,
//! виджеты перерисовываются теми же функциями (контракт F-8 «цвета — только
//! слоты» проверяется наглядно).
//!
//! Контракт поверхности (реестр U2): Modals/Block — клик мимо панели
//! (backdrop) закрывает и глотает; Esc — закрыть; «✕» — закрыть.
//! Отрисовка — полоса `UiLayer::Modals`.
//!
//! FR-059 (волна 1 миграции кита): секции компонентов v2 — TextField,
//! Switch, Card, список+скролл, Icon-глифы (контракт FR-058; состав
//! витрины FR-055 «компоненты × состояния × RU/EN × темы»). Контент выше
//! максимальной панели — колонка секций ПРОКРУЧИВАЕТСЯ (кит список+скролл,
//! [`ScrollState`] — состояние App; шапка фиксирована, при offset 0
//! прежние секции — в прежних местах, 0 скачка). Видимость секций —
//! целиком в окне контента (за краем — не рисуется: тексты кита не
//! клипятся по вертикали). Состояния контролов шапки — [`WidgetState`]
//! (FR-057) вместо deprecated-делегатов.

use canvas_core::Language;
use canvas_render::cards::CardInstance;
use canvas_render::text::{measure_font_system, TextAlign, SANS_FAMILY};
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
// FR-062 (layout v2): примитивы measured/flex/wrap/grid — раскладка витрины
// использует те же функции, что и потребители (живой образец).
use canvas_ui::kit::{self, ButtonVariant, ControlStyle, KitPalette, KitState};
// FR-068 W1 (ADR-0014 §Решение п.5 P1): витрина — pilot-поверхность, раскладка
// секций v2 идёт через [`pilot_backend`] (taffy за фичей — opt-in; на default
// сборке backend тот же Native — геометрия байт-в-байт прежняя).
use canvas_ui::layout::{grid_cells_with, pilot_backend, Child, MeasuredItem, Row, RowPolicy};
use canvas_ui::measure::TextMeasurer;
// FR-057 (волна 2 кита): draw-слой и машина состояний — в крейте canvas-ui;
// этот модуль — тонкий адаптер «items Painter'а → инстансы рендера».
use canvas_ui::paint::{PaintAlign, PaintItem, Painter};
use canvas_ui::widget::WidgetState;

/// Семейство шрифта подписей витрины (тот же SANS, что у рендера).
pub const FONT_FAMILY: &str = SANS_FAMILY;
/// Панель витрины: минимум (влезает в 800×560 окна линта с запасом).
pub const PANEL_MIN: UiVec2 = UiVec2::new(560.0, 420.0);
/// Панель витрины: максимум (на 1280×800 — 900×640).
pub const PANEL_MAX: UiVec2 = UiVec2::new(900.0, 640.0);
/// Отступ секций по вертикали (spacing-scale).
pub const SECTION_GAP: f32 = 12.0;
/// Ширина колонки подписи состояния.
pub const STATE_LABEL_W: f32 = 84.0;
/// Кегль подписей контролов.
pub const LABEL_SIZE: f32 = 13.0;
/// Ширина слота кнопки темы.
pub const THEME_SLOT_W: f32 = 170.0;

/// Подписи состояний (ключи i18n витрины).
pub const STATE_LABELS: [&str; 4] = [
    "kit.state.normal",
    "kit.state.hover",
    "kit.state.pressed",
    "kit.state.disabled",
];

/// Секции витрины (имена компонентов кита — классы, не переводятся).
pub const SECTION_BUTTONS: &str = "kit.section.buttons";
pub const SECTION_ICON: &str = "kit.section.icon_buttons";
pub const SECTION_CHIPS: &str = "kit.section.chips";
pub const SECTION_DROPDOWN: &str = "kit.section.dropdown";
pub const SECTION_TOAST: &str = "kit.section.toast";
pub const SECTION_TOOLTIP: &str = "kit.section.tooltip";
/// FR-059: секции компонентов v2 (FR-058).
pub const SECTION_TEXT_FIELD: &str = "kit.section.text_field";
pub const SECTION_SWITCH: &str = "kit.section.switch";
pub const SECTION_CARD: &str = "kit.section.card";
pub const SECTION_LIST: &str = "kit.section.list";
pub const SECTION_ICONS: &str = "kit.section.icons";
/// FR-061 (этап E, D-15): секция kit-Row — табличные строки на направляющих.
pub const SECTION_ROW: &str = "kit.section.row";
/// FR-068 W3 (этап M3): секция Table — ТЕ ЖЕ демо-данные через компонент
/// [`canvas_ui::kit::Table`] (витрина примитива Row остаётся рядом).
pub const SECTION_TABLE: &str = "kit.section.table";
/// FR-062: секции layout v2 (measured/flex/wrap/grid/focus).
pub const SECTION_MEASURED: &str = "kit.section.measured";
pub const SECTION_GROW: &str = "kit.section.grow";
pub const SECTION_WRAP: &str = "kit.section.wrap";
pub const SECTION_GRID: &str = "kit.section.grid";
pub const SECTION_FOCUS: &str = "kit.section.focus";

/// FR-059: демо-модель текстового поля витрины (обычное — текст без фокуса).
pub const GALLERY_FIELD_TEXT: &str = "50 rps";
/// FR-059: демо-модель текстового поля витрины (в фокусе — каретка видна).
pub const GALLERY_FIELD_FOCUSED_TEXT: &str = "1000 запр/с";
/// FR-059: строк в демо-списке витрины (в окне 3 — контент 8, скролл виден).
pub const GALLERY_LIST_ROWS: usize = 8;
/// FR-059: демо-сдвиг списка витрины (бегунок в середине трека).
pub const GALLERY_LIST_DEMO_OFFSET: f32 = 64.0;
/// FR-059: высота окна демо-списка (3 строки с зазорами).
pub const GALLERY_LIST_VIEWPORT_H: f32 = 3.0 * kit::LIST_ROW_H + 2.0 * kit::LIST_ROW_GAP;
/// FR-062 F-15: чипов в wrap-демо витрины (в слот на 2 строки влезает
/// не весь ряд — перенос виден на малых ширинах панели).
pub const GALLERY_WRAP_CHIPS: usize = 8;
/// FR-062 F-17: слотов в фокус-секции витрины (Tab-кольцо).
pub const GALLERY_FOCUS_SLOTS: usize = 4;
/// FR-062 F-17: ширина слота фокус-секции (фикс — подпись не измеряется).
pub const GALLERY_FOCUS_W: f32 = 72.0;
/// FR-061 (этап E): высота демо-строки секции Row (панель FR-044 — 22).
pub const GALLERY_ROW_H: f32 = 24.0;
/// FR-061 (этап E): демо-значения строк (числа — без i18n).
pub const GALLERY_ROW_VALUE_PRICE: &str = "50";
pub const GALLERY_ROW_VALUE_QTY: &str = "12";
pub const GALLERY_ROW_VALUE_TOTAL: &str = "600";
pub const GALLERY_ROW_VALUE_SUM: &str = "5 400";
/// FR-061 (этап E): глиф формульной строки демо (calc-маркер Р-4).
pub const GALLERY_ROW_FORMULA_GLYPH: &str = "ƒ";

/// Ряд кнопок одного варианта.
#[derive(Debug, Clone)]
pub struct ButtonRow {
    /// Вариант (заголовок ряда).
    pub variant: ButtonVariant,
    /// Слот ряда (полная ширина контента).
    pub slot: UiRect,
    /// Rect'ы 4 кнопок состояний (Normal/Hovered/Pressed/Disabled).
    pub buttons: Vec<UiRect>,
}

/// Раскладка витрины (чистая функция от вьюпорта; ширины подписей —
/// измеренные TextMeasurer'ом). FR-059: секции v2 + скролл контента —
/// rect'ы секций уже сдвинуты на `scroll.offset` и отфильтрованы по
/// полной видимости в окне контента ([`GalleryLayout::sections_viewport`]);
/// при offset 0 — прежняя раскладка дословно.
#[derive(Debug, Clone)]
pub struct GalleryLayout {
    /// Панель витрины (kit Modal).
    pub panel: UiRect,
    /// Контент внутри панели (минус пад).
    pub content: UiRect,
    /// Окно скролла секций (контент ниже шапки; трек бегунка).
    pub sections_viewport: UiRect,
    /// Полная высота колонки секций (для скролла — контент).
    pub content_h: f32,
    /// Кнопка «✕» (kit IconButton, интерактив).
    pub close: UiRect,
    /// Кнопка темы (kit Button Primary, интерактив).
    pub theme: UiRect,
    /// Заголовок витрины.
    pub title: UiRect,
    /// Ряды кнопок: 4 варианта × 4 состояния.
    pub button_rows: Vec<ButtonRow>,
    /// Икон-кнопки: 4 состояния.
    pub icon_buttons: Vec<UiRect>,
    /// Чипы: 4 состояния.
    pub chips: Vec<UiRect>,
    /// Закрытый dropdown-якорь.
    pub dropdown_anchor: UiRect,
    /// Открытое dropdown-меню.
    pub dropdown_menu: UiRect,
    /// Строки dropdown-меню.
    pub dropdown_items: Vec<UiRect>,
    /// Область тоста (kit Toast).
    pub toast: UiRect,
    /// Чип-якорь тултипа.
    pub tooltip_anchor: UiRect,
    /// Пузырь тултипа (delay пройден — показан).
    pub tooltip: UiRect,
    /// Подписи секций (origin + текст).
    pub section_titles: Vec<(UiPoint, &'static str)>,
    /// FR-059: текстовые поля v2 — 3 демо (Normal/Focused/Disabled):
    /// (состояние, в фокусе — каретка видна, раскладка `kit::text_field`).
    pub text_fields: Vec<(KitState, bool, kit::TextFieldLayout)>,
    /// FR-059: переключатели v2 — (slot, on, состояние).
    pub switches: Vec<(UiRect, bool, KitState)>,
    /// FR-059: контентная карточка v2 (rect/header/body).
    pub card: Option<kit::CardLayout>,
    /// FR-059: окно демо-списка (вьюпорт скролла).
    pub list_area: UiRect,
    /// FR-059: видимые строки демо-списка (индекс, rect).
    pub list_rows: Vec<(usize, UiRect)>,
    /// FR-059: выделенная строка демо-списка.
    pub list_selected: usize,
    /// FR-059: скролл демо-списка (статичный демо-сдвиг — бегунок в треке).
    pub list_scroll: kit::ScrollState,
    /// FR-059: икон-кнопки с глифами v2 (rect, иконка).
    pub icon_glyphs: Vec<(UiRect, kit::Icon)>,
    /// FR-062 F-13: measured-ряд — чипы, ширины которых TextMeasurer
    /// посчитал внутри [`Row::lay_out_measured`] (ручной проводки нет).
    pub measured_chips: Vec<UiRect>,
    /// FR-062 F-14: flex-ряд — (rect, ключ подписи): fixed + grow ×2 +
    /// grow ×1 — свободное место распределено пропорционально.
    pub grow_cells: Vec<(UiRect, &'static str)>,
    /// FR-062 F-15: wrap-ряд — (rect, индекс чипа): жадная упаковка
    /// measured-чипов в строки слота ([`RowPolicy::Wrap`]).
    pub wrap_chips: Vec<(UiRect, usize)>,
    /// FR-062 F-16: сетка 4×2 равных колонок ([`grid_cells_with`],
    /// FR-068 W1 — через [`pilot_backend`]).
    pub grid_cells: Vec<UiRect>,
    /// FR-062 F-17: кнопки фокус-секции (сдвинуты/отфильтрованы — для
    /// отрисовки); рамка фокуса — по совпадению с Tab-кольцом App.
    pub focus_buttons: Vec<UiRect>,
    /// FR-062 F-17: Tab-порядок фокус-секции в КОНТЕНТ-координатах (без
    /// сдвига/фильтра) — кольцо [`canvas_ui::keyboard::FocusRing`] в App
    /// живёт в этих координатах; отрисовка рамки — сдвиг на offset.
    pub focus_targets: Vec<UiRect>,
    /// FR-061 (этап E, D-15) + FR-068 W3 (M3): демо-строки табличных секций
    /// Row (витрина примитива) и Table (компонент v2) — ТЕ ЖЕ данные на
    /// общих направляющих; отрисовка — [`canvas_ui::kit::paint_row`] по
    /// предвычисленному [`canvas_ui::kit::RowLayout`] (app/overlays.rs).
    pub row_rows: Vec<RowDemoRow>,
}

/// FR-061 (этап E, D-15) + FR-068 W3 (M3): демо-данные табличных секций
/// витрины — единый источник для Row и Table (4 строки: Dot/цена,
/// Dot/кол-во + зебра, Glyph ƒ/итого + бейдж, Σ Selected). Возвращает
/// `(части строки кит-Row, состояние, зебра)` — Table-версия строится
/// из тех же значений ([`TableRow`](canvas_ui::kit::TableRow)).
fn gallery_row_demo(lang: Language) -> [(kit::RowParts<'static>, KitState, bool); 4] {
    [
        (
            kit::RowParts {
                marker: kit::RowMarker::Dot,
                label: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_PRICE),
                value: GALLERY_ROW_VALUE_PRICE,
                unit: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_PRICE),
                badge: "",
            },
            KitState::Normal,
            false,
        ),
        (
            kit::RowParts {
                marker: kit::RowMarker::Dot,
                label: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_QTY),
                value: GALLERY_ROW_VALUE_QTY,
                unit: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_QTY),
                badge: "",
            },
            KitState::Normal,
            true,
        ),
        (
            kit::RowParts {
                marker: kit::RowMarker::Glyph(GALLERY_ROW_FORMULA_GLYPH),
                label: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_TOTAL),
                value: GALLERY_ROW_VALUE_TOTAL,
                unit: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_MONEY),
                badge: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_BADGE),
            },
            KitState::Normal,
            false,
        ),
        (
            kit::RowParts {
                marker: kit::RowMarker::None,
                label: crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_SUM),
                value: GALLERY_ROW_VALUE_SUM,
                unit: "",
                badge: "",
            },
            KitState::Selected,
            false,
        ),
    ]
}

/// FR-061 (этап E, D-15): строка демо-таблицы витрины — данные кит-Row
/// + геометрия ([`canvas_ui::kit::row_layout`] на общих направляющих).
///
/// FR-068 W3 (M3): структура используется ОБЕИМИ табличными секциями —
/// Row (витрина примитива) и Table (компонент: геометрия из
/// [`canvas_ui::kit::Table::row_layout_with`]).
#[derive(Debug, Clone)]
pub struct RowDemoRow {
    /// Состояние строки (слоты [`canvas_ui::kit::row_style`]).
    pub state: KitState,
    /// Зебра (альтернативный фон — демо слотом hover_fill).
    pub zebra: bool,
    /// Данные строки (тексты — &'static: константы/переводы).
    pub parts: kit::RowParts<'static>,
    /// Геометрия строки (значение/юнит — на направляющих демо-таблицы).
    pub lay: kit::RowLayout,
}

/// Сдвиг геометрии кит-строки по вертикали (скролл витрины — все ячейки
/// строки в одних координатах, кроме текста).
fn shift_row_lay(mut lay: kit::RowLayout, dy: f32) -> kit::RowLayout {
    lay.row.y -= dy;
    if let Some(r) = lay.dot.as_mut() {
        r.y -= dy;
    }
    if let Some(r) = lay.glyph.as_mut() {
        r.y -= dy;
    }
    lay.label.y -= dy;
    lay.leader_y -= dy;
    lay.value.y -= dy;
    lay.unit.y -= dy;
    if let Some(r) = lay.badge.as_mut() {
        r.y -= dy;
    }
    lay
}

/// Перевод ключа витрины (ключи — 'static константы модуля).
fn tr(lang: Language, key: &'static str) -> String {
    crate::i18n::tr(lang, key).to_owned()
}

/// Отступ панели витрины от краёв вьюпорта (spacing-scale XL).
const VIEWPORT_MARGIN: f32 = 24.0;

/// Панель витрины, зажатая во вьюпорт: constrain(min, max, desired) + кламп
/// к вьюпорту (модаль не вылезает на малых окнах — G4-линт: hit-rect'ы
/// интерактивных зон целиком во вьюпорте; поймал 800×560 на U4).
pub fn gallery_panel(vp: UiRect) -> UiRect {
    let avail = UiVec2::new(
        (vp.w - VIEWPORT_MARGIN).max(0.0),
        (vp.h - VIEWPORT_MARGIN).max(0.0),
    );
    let max = UiVec2::new(PANEL_MAX.x.min(avail.x), PANEL_MAX.y.min(avail.y));
    let min = UiVec2::new(PANEL_MIN.x.min(avail.x), PANEL_MIN.y.min(avail.y));
    kit::panel_rect(vp, min, max, PANEL_MAX)
}

/// Раскладка витрины. Секции — Column-поток от контента; высоты — метрики
/// кита, ширины подписей — измеренные. FR-059: контент выше максимальной
/// панели — секции сдвигаются на `scroll.offset` и фильтруются по полной
/// видимости в окне секций (при offset 0 — прежняя раскладка дословно);
/// полная высота колонки — в `content_h` (скролл-контракт кита).
pub fn gallery_layout(
    viewport: [f32; 2],
    lang: Language,
    scroll: &kit::ScrollState,
    p: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> GalleryLayout {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    // Панель: kit Modal (constrain + stack по центру, зажата во вьюпорт)
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));

    let mut y = content.y;
    let full_w = content.w;

    // Шапка: заголовок слева, кнопка темы справа (перед ✕), ✕ — край
    // (ФИКСИРОВАНА — не скроллится: hit-слоты реестра без изменений)
    let title = UiRect::new(
        content.x,
        y,
        (full_w - 2.0 * (kit::ICON_BUTTON_SIZE + 8.0)).max(0.0),
        30.0,
    );
    let close = kit::icon_button_rect(
        UiRect::new(
            content.right() - kit::ICON_BUTTON_SIZE,
            y,
            kit::ICON_BUTTON_SIZE,
            30.0,
        ),
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
    );
    let theme = UiRect::new(
        close.x - 8.0 - THEME_SLOT_W,
        y + (30.0 - kit::BUTTON_HEIGHT) / 2.0,
        THEME_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    y += 30.0 + SECTION_GAP;
    // Окно скролла секций (шапка выше — фиксирована)
    let sections_viewport = UiRect::new(content.x, y, content.w, (content.bottom() - y).max(0.0));
    let sections_top = y;

    let mut section_titles: Vec<(UiPoint, &'static str)> = Vec::new();
    let mut button_rows: Vec<ButtonRow> = Vec::new();
    let mut icon_buttons: Vec<UiRect> = Vec::new();
    let mut chips: Vec<UiRect> = Vec::new();
    let mut dropdown_items: Vec<UiRect> = Vec::new();

    // Подпись состояния слева + контрол в остатке ширины
    let control_x = content.x + STATE_LABEL_W + canvas_core::tokens::SPACING_SM;
    let control_w = (content.right() - control_x).max(0.0);

    // --- Buttons: 4 варианта × 4 состояния ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_BUTTONS));
    y += 18.0;
    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ];
    for variant in variants {
        let slot = UiRect::new(content.x, y, full_w, kit::BUTTON_HEIGHT);
        let per = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        let mut buttons = Vec::with_capacity(4);
        for (i, state_label) in STATE_LABELS.iter().enumerate() {
            let cell = UiRect::new(
                control_x + i as f32 * (per + kit::GAP_CONTROLS),
                y,
                per,
                kit::BUTTON_HEIGHT,
            );
            let label = tr(lang, state_label);
            let bl = kit::button_layout(
                cell,
                &label,
                (
                    canvas_ui::layout::HAlign::Start,
                    canvas_ui::layout::VAlign::Center,
                ),
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            buttons.push(bl.rect);
        }
        button_rows.push(ButtonRow {
            variant,
            slot,
            buttons,
        });
        y += kit::BUTTON_HEIGHT + 8.0;
    }
    y += SECTION_GAP - 4.0;

    // --- IconButtons: 4 состояния ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_ICON));
    y += 18.0;
    for i in 0..4 {
        let cell = UiRect::new(
            control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
            y,
            kit::ICON_BUTTON_SIZE,
            kit::ICON_BUTTON_SIZE,
        );
        icon_buttons.push(kit::icon_button_rect(
            cell,
            (
                canvas_ui::layout::HAlign::Start,
                canvas_ui::layout::VAlign::Center,
            ),
        ));
    }
    y += kit::ICON_BUTTON_SIZE + SECTION_GAP;

    // --- Chips: 4 состояния (измеренные ширины) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_CHIPS));
    y += 18.0;
    {
        let mut x = control_x;
        let max_w = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        for state_label in STATE_LABELS.iter() {
            let label = tr(lang, state_label);
            let cl = kit::chip_layout(
                UiPoint::new(x, y),
                &label,
                max_w,
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            chips.push(cl.rect);
            x += cl.rect.w + kit::GAP_CONTROLS;
        }
    }
    y += kit::CHIP_HEIGHT + SECTION_GAP;

    // --- Dropdown: якорь-кнопка + открытое меню (3 строки) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_DROPDOWN));
    y += 18.0;
    let anchor_label = tr(lang, "kit.dropdown.anchor");
    let anchor_btn = kit::button_layout(
        UiRect::new(control_x, y, 190.0, kit::BUTTON_HEIGHT),
        &anchor_label,
        (
            canvas_ui::layout::HAlign::Start,
            canvas_ui::layout::VAlign::Center,
        ),
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let dropdown_anchor = anchor_btn.rect;
    y += kit::BUTTON_HEIGHT + 4.0;
    let item_h = 26.0;
    let menu_h = 3.0 * item_h + 2.0 * 4.0;
    let dd = kit::dropdown_menu(dropdown_anchor, vp, UiVec2::new(190.0, menu_h));
    for i in 0..3 {
        dropdown_items.push(UiRect::new(
            dd.menu.x + 4.0,
            dd.menu.y + 4.0 + i as f32 * (item_h + 4.0),
            dd.menu.w - 8.0,
            item_h,
        ));
    }
    let dropdown_menu = dd.menu;
    y += menu_h + SECTION_GAP;

    // --- Toast: строка внизу контента (kit Toast) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_TOAST));
    y += 18.0;
    let toast = UiRect::new(content.x, y, content.w, 20.0);
    y += 20.0 + SECTION_GAP;

    // --- Tooltip: якорь-чип + пузырь (delay пройден) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_TOOLTIP));
    y += 18.0;
    let anchor_label = tr(lang, "kit.tooltip.anchor");
    let cl = kit::chip_layout(
        UiPoint::new(control_x, y),
        &anchor_label,
        140.0,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let tooltip_anchor = cl.rect;
    let tip_text = tr(lang, "kit.tooltip.body");
    let tip_w = (m.width_of(fs, &tip_text, FONT_FAMILY, 12.0) + 16.0).min(240.0);
    let tip = kit::tooltip(
        UiPoint::new(tooltip_anchor.right(), tooltip_anchor.y + 4.0),
        UiVec2::new(tip_w, 18.0),
        vp,
        kit::TOOLTIP_DELAY_MS,
        kit::TOOLTIP_DELAY_MS,
    );
    let tooltip = tip
        .map(|t| t.rect)
        .unwrap_or(UiRect::new(0.0, 0.0, 0.0, 0.0));
    y += 18.0 + 26.0 + SECTION_GAP;

    // === FR-059: секции компонентов v2 (FR-058) — после секций v1 ===

    // --- TextField: Normal / Focused / Disabled (3 поля в ряд) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_TEXT_FIELD));
    y += 18.0;
    let mut text_fields: Vec<(KitState, bool, kit::TextFieldLayout)> = Vec::new();
    {
        let per = ((control_w - 2.0 * kit::GAP_CONTROLS) / 3.0).max(kit::TEXT_FIELD_MIN_W);
        let models: [(KitState, kit::TextFieldModel, &str); 3] = [
            (
                KitState::Normal,
                kit::TextFieldModel {
                    text: GALLERY_FIELD_TEXT.to_owned(),
                    caret: GALLERY_FIELD_TEXT.chars().count(),
                    sel: None,
                },
                "",
            ),
            (
                KitState::Normal,
                kit::TextFieldModel {
                    text: GALLERY_FIELD_FOCUSED_TEXT.to_owned(),
                    caret: GALLERY_FIELD_FOCUSED_TEXT.chars().count(),
                    sel: None,
                },
                "",
            ),
            (
                KitState::Disabled,
                kit::TextFieldModel::default(),
                "kit.textfield.placeholder",
            ),
        ];
        for (i, (state, model, placeholder_key)) in models.into_iter().enumerate() {
            let slot = UiRect::new(
                control_x + i as f32 * (per + kit::GAP_CONTROLS),
                y,
                per,
                kit::TEXT_FIELD_HEIGHT,
            );
            let focused = i == 1; // второе поле — в фокусе (каретка видна)
            let placeholder = if placeholder_key.is_empty() {
                ""
            } else {
                &tr(lang, "kit.textfield.placeholder")
            };
            let lay = kit::text_field(
                slot,
                UiVec2::new(kit::TEXT_FIELD_MIN_W, kit::TEXT_FIELD_HEIGHT),
                UiVec2::new(per, kit::TEXT_FIELD_HEIGHT),
                &model,
                placeholder,
                focused,
                state,
                p,
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            text_fields.push((state, focused, lay));
        }
    }
    y += kit::TEXT_FIELD_HEIGHT + SECTION_GAP;

    // --- Switch: Off/Normal, On/Normal, On/Hovered, Off/Disabled ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_SWITCH));
    y += 18.0;
    let mut switches: Vec<(UiRect, bool, KitState)> = Vec::new();
    {
        let demo: [(bool, KitState); 4] = [
            (false, KitState::Normal),
            (true, KitState::Normal),
            (true, KitState::Hovered),
            (false, KitState::Disabled),
        ];
        for (i, (on, state)) in demo.into_iter().enumerate() {
            let slot = UiRect::new(
                control_x + i as f32 * (kit::SWITCH_W + kit::GAP_CONTROLS),
                y,
                kit::SWITCH_W,
                kit::SWITCH_H,
            );
            switches.push((slot, on, state));
        }
    }
    y += kit::SWITCH_H + SECTION_GAP;

    // --- Card: хедер + тело внутри пада панели (kit::card) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_CARD));
    y += 18.0;
    let card_slot = UiRect::new(control_x, y, control_w, 64.0);
    let card = kit::card(
        card_slot,
        UiVec2::new(0.0, 64.0),
        UiVec2::new(600.0, 64.0),
        20.0,
        p,
    );
    y += 64.0 + SECTION_GAP;

    // --- Список + скролл: 8 строк в окне 3 (бегунок в треке) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_LIST));
    y += 18.0;
    let list_area = UiRect::new(control_x, y, control_w, GALLERY_LIST_VIEWPORT_H);
    let mut list_scroll = kit::ScrollState {
        offset: GALLERY_LIST_DEMO_OFFSET,
        content_h: GALLERY_LIST_ROWS as f32 * kit::LIST_ROW_H
            + (GALLERY_LIST_ROWS.saturating_sub(1)) as f32 * kit::LIST_ROW_GAP,
        viewport_h: GALLERY_LIST_VIEWPORT_H,
    };
    list_scroll.clamp();
    let list_rows = kit::list_rows(
        list_area,
        &list_scroll,
        kit::LIST_ROW_H,
        kit::LIST_ROW_GAP,
        GALLERY_LIST_ROWS,
    );
    let list_selected = 2usize;
    y += GALLERY_LIST_VIEWPORT_H + SECTION_GAP;

    // --- Icon-глифы v2: Search/ArrowLeft/ArrowRight/Refresh (Normal) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_ICONS));
    y += 18.0;
    let mut icon_glyphs: Vec<(UiRect, kit::Icon)> = Vec::new();
    {
        let glyphs = [
            kit::Icon::Search,
            kit::Icon::ArrowLeft,
            kit::Icon::ArrowRight,
            kit::Icon::Refresh,
        ];
        for (i, icon) in glyphs.into_iter().enumerate() {
            let cell = UiRect::new(
                control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
                y,
                kit::ICON_BUTTON_SIZE,
                kit::ICON_BUTTON_SIZE,
            );
            let rect = kit::icon_button(
                cell,
                icon,
                (
                    canvas_ui::layout::HAlign::Start,
                    canvas_ui::layout::VAlign::Center,
                ),
            );
            icon_glyphs.push((rect, icon));
        }
    }
    y += kit::ICON_BUTTON_SIZE + SECTION_GAP;

    // === FR-061 (этап E, D-15): секция Row — табличные строки на
    // направляющих (параметр ×2 / формула с бейджем / Σ), состояния
    // Normal/Zebra/Selected — те же функции кита, что у панели
    // «Как считается» (row_guides/row_layout/paint_row).
    section_titles.push((UiPoint::new(content.x, y), SECTION_ROW));
    y += 18.0;
    let mut row_rows: Vec<RowDemoRow> = Vec::new();
    {
        let demo = gallery_row_demo(lang);
        let parts: Vec<kit::RowParts<'static>> = demo.iter().map(|(p, _, _)| *p).collect();
        // Общие направляющие демо-таблицы (право — край контрол-колонки)
        if let Some(rg) = kit::row_guides(
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
            &parts,
            control_x + control_w,
            canvas_core::tokens::TABLE_GUIDE_GAP,
        ) {
            for (i, (row_parts, state, zebra)) in demo.into_iter().enumerate() {
                let slot = UiRect::new(
                    control_x,
                    y + i as f32 * GALLERY_ROW_H,
                    control_w,
                    GALLERY_ROW_H,
                );
                let lay = kit::row_layout(
                    m,
                    fs,
                    FONT_FAMILY,
                    LABEL_SIZE,
                    slot,
                    rg,
                    &row_parts,
                    &kit::RowOpts::default(),
                );
                row_rows.push(RowDemoRow {
                    state,
                    zebra,
                    parts: row_parts,
                    lay,
                });
            }
        }
    }
    y += 4.0 * GALLERY_ROW_H + SECTION_GAP;

    // === FR-068 W3 (этап M3): секция Table — ТЕ ЖЕ демо-данные, что у
    // секции Row выше, через компонент Table (retained-вход кадра:
    // set_rows + row_layout_with по фиксированным слотам GALLERY_ROW_H).
    // Параметры — паритет Row-секции: right_pad 0 (правый край направляющих
    // — край контрол-колонки; viewport_right — ПРАВЫЙ КРАЙ В КООРДИНАТАХ
    // СЛОТОВ, слоты здесь x=control_x), зазор TABLE_GUIDE_GAP, лидер on
    // (RowOpts::default), кегль LABEL_SIZE. Зебра — переопределением слота
    // fill (TableRowStyle { fill: Some(hover_fill) }, F-8 — готовый слот,
    // без арифметики), Σ — state: Selected. Отрисовка — общий путь секции
    // Row (paint_row по предвычисленному RowLayout в app/overlays.rs):
    // строки попадают в `row_rows` — ноль правок в отрисовке; оракул
    // бит-в-бит «Row ≡ Table на одних данных» — в тестах модуля.
    section_titles.push((UiPoint::new(content.x, y), SECTION_TABLE));
    y += 18.0;
    {
        let viewport_right = control_x + control_w;
        let mut table = kit::Table::new(kit::TableProps {
            size: LABEL_SIZE,
            family: FONT_FAMILY,
            opts: kit::TableOpts {
                leader: true,
                gap: canvas_core::tokens::TABLE_GUIDE_GAP,
                row_h: GALLERY_ROW_H,
                row_gap: 0.0,
                right_pad: 0.0,
            },
            palette: *p,
        });
        let demo = gallery_row_demo(lang);
        table.set_rows(
            demo.iter()
                .copied()
                .map(|(parts, state, zebra)| kit::TableRow {
                    marker: parts.marker,
                    label: parts.label.to_owned(),
                    value: parts.value.to_owned(),
                    unit: parts.unit.to_owned(),
                    badge: (!parts.badge.is_empty()).then(|| parts.badge.to_owned()),
                    state,
                    style: kit::TableRowStyle {
                        // Зебра — слот hover_fill (паритет демо Row: тот же
                        // слот подставляется в row_style в отрисовке).
                        fill: zebra.then_some(p.hover_fill),
                        ..kit::TableRowStyle::default()
                    },
                })
                .collect(),
        );
        // Деградация §4.2 (guides None — правые колонки пусты у всех
        // строк): паритет Row-секции (row_guides None → строки не строятся).
        if table.guides_with(m, fs, viewport_right).is_some() {
            for (i, (row_parts, state, zebra)) in demo.into_iter().enumerate() {
                let slot = UiRect::new(
                    control_x,
                    y + i as f32 * GALLERY_ROW_H,
                    control_w,
                    GALLERY_ROW_H,
                );
                if let Some(lay) = table.row_layout_with(m, fs, viewport_right, slot, i) {
                    row_rows.push(RowDemoRow {
                        state,
                        zebra,
                        parts: row_parts,
                        lay,
                    });
                }
            }
        }
    }
    y += 4.0 * GALLERY_ROW_H + SECTION_GAP;

    // === FR-062: секции layout v2 (F-13…F-17) — после секций компонентов v2 ===

    // --- Measured-ряд (F-13): ширины чипов — TextMeasurer внутри
    // раскладки ([`Row::lay_out_measured`]); пад чипа — измеренные
    // пробелы вокруг подписи (рисуется исходная подпись по центру).
    section_titles.push((UiPoint::new(content.x, y), SECTION_MEASURED));
    y += 18.0;
    let measured_chips: Vec<UiRect> = {
        let l_a = tr(lang, crate::i18n::keys::KIT_MEASURED_A);
        let l_b = tr(lang, crate::i18n::keys::KIT_MEASURED_B);
        let l_c = tr(lang, crate::i18n::keys::KIT_MEASURED_C);
        let t_a = format!(" {l_a} ");
        let t_b = format!(" {l_b} ");
        let t_c = format!(" {l_c} ");
        Row {
            gap: kit::GAP_CONTROLS,
            ..Row::default()
        }
        .lay_out_measured_with(
            pilot_backend(),
            UiRect::new(control_x, y, control_w, kit::CHIP_HEIGHT),
            &[
                MeasuredItem::Text {
                    text: &t_a,
                    max_w: None,
                    min_w: kit::CHIP_PAD_H * 2.0,
                    pad_x: 0.0,
                    h: None,
                },
                MeasuredItem::Text {
                    text: &t_b,
                    max_w: None,
                    min_w: kit::CHIP_PAD_H * 2.0,
                    pad_x: 0.0,
                    h: None,
                },
                MeasuredItem::Text {
                    text: &t_c,
                    max_w: None,
                    min_w: kit::CHIP_PAD_H * 2.0,
                    pad_x: 0.0,
                    h: None,
                },
            ],
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        )
    };
    y += kit::CHIP_HEIGHT + SECTION_GAP;

    // --- Flex-факторы (F-14): fixed + grow ×2 + grow ×1 — свободное
    // место слота распределяется пропорционально grow.
    section_titles.push((UiPoint::new(content.x, y), SECTION_GROW));
    y += 18.0;
    let grow_cells: Vec<(UiRect, &'static str)> = Row {
        gap: kit::GAP_CONTROLS,
        ..Row::default()
    }
    .lay_out_with(
        pilot_backend(),
        UiRect::new(control_x, y, control_w, kit::BUTTON_HEIGHT),
        &[
            Child::fixed(64.0, kit::BUTTON_HEIGHT),
            Child::flexible(40.0, kit::BUTTON_HEIGHT, 2.0),
            Child::flexible(40.0, kit::BUTTON_HEIGHT, 1.0),
        ],
    )
    .into_iter()
    .zip([
        crate::i18n::keys::KIT_GROW_FIXED,
        crate::i18n::keys::KIT_GROW_TWO,
        crate::i18n::keys::KIT_GROW_ONE,
    ])
    .collect();
    y += kit::BUTTON_HEIGHT + SECTION_GAP;

    // --- Wrap (F-15): жадная упаковка measured-чипов в строки слота
    // (2 строки видимы; переполнение — за нижний край, видно линту).
    section_titles.push((UiPoint::new(content.x, y), SECTION_WRAP));
    y += 18.0;
    let wrap_slot_h = 2.0 * kit::CHIP_HEIGHT + kit::GAP_CONTROLS;
    let wrap_chips: Vec<(UiRect, usize)> = {
        // подписи живут в блоке — MeasuredItem заимствует из них (без leak)
        let padded_labels: Vec<String> = (0..GALLERY_WRAP_CHIPS)
            .map(|i| {
                let label = crate::i18n::trf(
                    lang,
                    crate::i18n::keys::KIT_WRAP_CHIP,
                    &[("{n}", &(i + 1).to_string())],
                );
                format!(" {label} ")
            })
            .collect();
        let items: Vec<MeasuredItem> = padded_labels
            .iter()
            .map(|s| MeasuredItem::Text {
                text: s,
                max_w: None,
                min_w: kit::CHIP_PAD_H * 2.0,
                pad_x: 0.0,
                h: None,
            })
            .collect();
        Row {
            gap: kit::GAP_CONTROLS,
            policy: RowPolicy::Wrap,
            ..Row::default()
        }
        .lay_out_measured_with(
            pilot_backend(),
            UiRect::new(control_x, y, control_w, wrap_slot_h),
            &items,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        )
        .into_iter()
        .enumerate()
        .map(|(i, r)| (r, i))
        .collect()
    };
    y += wrap_slot_h + SECTION_GAP;

    // --- Сетка (F-16): 4 равные колонки × 2 строки ([`grid_cells_with`],
    // FR-068 W1 — через [`pilot_backend`]; T2-триггер ADR-0013 — Grid
    // с явными треками в TaffyBackend).
    section_titles.push((UiPoint::new(content.x, y), SECTION_GRID));
    y += 18.0;
    let grid_cell_h = 32.0;
    let grid_col_w = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
    let grid_cells: Vec<UiRect> = grid_cells_with(
        pilot_backend(),
        UiRect::new(
            control_x,
            y,
            control_w,
            2.0 * grid_cell_h + kit::GAP_CONTROLS,
        ),
        &[grid_col_w; 4],
        2,
        grid_cell_h,
        UiVec2::new(kit::GAP_CONTROLS, kit::GAP_CONTROLS),
    );
    y += 2.0 * grid_cell_h + kit::GAP_CONTROLS + SECTION_GAP;

    // --- Фокус (F-17): 4 слота фиксированной ширины; Tab-кольцо App
    // живёт в КОНТЕНТ-координатах ([`Self::focus_targets`]), рамка —
    // при отрисовке по совпадению с кольцом (слот accent).
    section_titles.push((UiPoint::new(content.x, y), SECTION_FOCUS));
    y += 18.0;
    let focus_targets: Vec<UiRect> = (0..GALLERY_FOCUS_SLOTS)
        .map(|i| {
            UiRect::new(
                control_x + i as f32 * (GALLERY_FOCUS_W + kit::GAP_CONTROLS),
                y,
                GALLERY_FOCUS_W,
                kit::BUTTON_HEIGHT,
            )
        })
        .collect();
    let focus_buttons: Vec<UiRect> = focus_targets.clone();
    y += kit::BUTTON_HEIGHT;

    // Полная высота колонки секций (для скролла)
    let content_h = (y - sections_top).max(0.0);

    // === FR-059: сдвиг на scroll.offset + фильтр полной видимости ===
    // Тексты кита не клипятся по вертикали — секция за краем окна не
    // рисуется вовсе (при offset 0 фильтр ничего не отрезает).
    let off = scroll.offset;
    let visible = |r: &UiRect| -> bool {
        r.y - off >= sections_viewport.y - 0.01
            && r.bottom() - off <= sections_viewport.bottom() + 0.01
    };
    let section_titles: Vec<(UiPoint, &'static str)> = section_titles
        .into_iter()
        .filter(|(origin, _)| visible(&UiRect::new(origin.x, origin.y, content.w, 16.0)))
        .map(|(origin, key)| (UiPoint::new(origin.x, origin.y - off), key))
        .collect();
    let button_rows: Vec<ButtonRow> = button_rows
        .into_iter()
        .filter(|row| visible(&row.slot))
        .map(|row| ButtonRow {
            slot: UiRect::new(row.slot.x, row.slot.y - off, row.slot.w, row.slot.h),
            buttons: row
                .buttons
                .into_iter()
                .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
                .collect(),
            ..row
        })
        .collect();
    let icon_buttons: Vec<UiRect> = icon_buttons
        .into_iter()
        .filter(|r| visible(r))
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let chips: Vec<UiRect> = chips
        .into_iter()
        .filter(|r| visible(r))
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let dropdown_anchor = if visible(&dropdown_anchor) {
        UiRect::new(
            dropdown_anchor.x,
            dropdown_anchor.y - off,
            dropdown_anchor.w,
            dropdown_anchor.h,
        )
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let dropdown_visible = visible(&dropdown_menu);
    let dropdown_menu = if dropdown_visible {
        UiRect::new(
            dropdown_menu.x,
            dropdown_menu.y - off,
            dropdown_menu.w,
            dropdown_menu.h,
        )
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let dropdown_items: Vec<UiRect> = if dropdown_visible {
        dropdown_items
            .into_iter()
            .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
            .collect()
    } else {
        Vec::new()
    };
    let toast = if visible(&toast) {
        UiRect::new(toast.x, toast.y - off, toast.w, toast.h)
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let tooltip_anchor = if visible(&tooltip_anchor) {
        UiRect::new(
            tooltip_anchor.x,
            tooltip_anchor.y - off,
            tooltip_anchor.w,
            tooltip_anchor.h,
        )
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let tooltip = if !tooltip.is_empty() && visible(&tooltip) {
        UiRect::new(tooltip.x, tooltip.y - off, tooltip.w, tooltip.h)
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let text_fields: Vec<(KitState, bool, kit::TextFieldLayout)> = text_fields
        .into_iter()
        .filter(|(_, _, lay)| visible(&lay.rect))
        .map(|(state, focused, lay)| {
            (
                state,
                focused,
                kit::TextFieldLayout {
                    rect: UiRect::new(lay.rect.x, lay.rect.y - off, lay.rect.w, lay.rect.h),
                    text_area: UiRect::new(
                        lay.text_area.x,
                        lay.text_area.y - off,
                        lay.text_area.w,
                        lay.text_area.h,
                    ),
                    ..lay
                },
            )
        })
        .collect();
    let switches: Vec<(UiRect, bool, KitState)> = switches
        .into_iter()
        .filter(|(r, _, _)| visible(r))
        .map(|(r, on, state)| (UiRect::new(r.x, r.y - off, r.w, r.h), on, state))
        .collect();
    let card = if visible(&card.rect) {
        Some(kit::CardLayout {
            rect: UiRect::new(card.rect.x, card.rect.y - off, card.rect.w, card.rect.h),
            header: UiRect::new(
                card.header.x,
                card.header.y - off,
                card.header.w,
                card.header.h,
            ),
            body: UiRect::new(card.body.x, card.body.y - off, card.body.w, card.body.h),
        })
    } else {
        None
    };
    let list_area = if visible(&list_area) {
        UiRect::new(list_area.x, list_area.y - off, list_area.w, list_area.h)
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let list_rows: Vec<(usize, UiRect)> = if list_area.w > 0.0 {
        list_rows
            .into_iter()
            .filter(|(_, r)| visible(r))
            .map(|(i, r)| (i, UiRect::new(r.x, r.y - off, r.w, r.h)))
            .collect()
    } else {
        Vec::new()
    };
    let icon_glyphs: Vec<(UiRect, kit::Icon)> = icon_glyphs
        .into_iter()
        .filter(|(r, _)| visible(r))
        .map(|(r, icon)| (UiRect::new(r.x, r.y - off, r.w, r.h), icon))
        .collect();
    // FR-062: секции layout v2 — сдвиг + фильтр полной видимости
    let measured_chips: Vec<UiRect> = measured_chips
        .into_iter()
        .filter(visible)
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let grow_cells: Vec<(UiRect, &'static str)> = grow_cells
        .into_iter()
        .filter(|(r, _)| visible(r))
        .map(|(r, k)| (UiRect::new(r.x, r.y - off, r.w, r.h), k))
        .collect();
    let wrap_chips: Vec<(UiRect, usize)> = wrap_chips
        .into_iter()
        .filter(|(r, _)| visible(r))
        .map(|(r, i)| (UiRect::new(r.x, r.y - off, r.w, r.h), i))
        .collect();
    let grid_cells: Vec<UiRect> = grid_cells
        .into_iter()
        .filter(visible)
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let focus_buttons: Vec<UiRect> = focus_buttons
        .into_iter()
        .filter(visible)
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    // FR-061: секция Row — сдвиг всех ячеек каждой строки + фильтр
    let row_rows: Vec<RowDemoRow> = row_rows
        .into_iter()
        .filter(|d| visible(&d.lay.row))
        .map(|d| RowDemoRow {
            lay: shift_row_lay(d.lay, off),
            ..d
        })
        .collect();
    // focus_targets НЕ сдвигаются/фильтруются — контент-координаты Tab-кольца

    GalleryLayout {
        panel,
        content,
        sections_viewport,
        content_h,
        close,
        theme,
        title,
        button_rows,
        icon_buttons,
        chips,
        dropdown_anchor,
        dropdown_menu,
        dropdown_items,
        toast,
        tooltip_anchor,
        tooltip,
        section_titles,
        text_fields,
        switches,
        card,
        list_area,
        list_rows,
        list_selected,
        list_scroll,
        icon_glyphs,
        measured_chips,
        grow_cells,
        wrap_chips,
        grid_cells,
        focus_buttons,
        focus_targets,
        row_rows,
    }
}

/// Слот-раскладка интерактивных зон для hit-rect'ов реестра (без
/// измерителя — фиксированные слоты шапки; совпадает с полной раскладкой —
/// одна геометрия для ввода и отрисовки). Возврат: (кнопка темы, «✕»).
pub fn gallery_hit_slots(viewport: [f32; 2]) -> (UiRect, UiRect) {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let close = kit::icon_button_rect(
        UiRect::new(
            content.right() - kit::ICON_BUTTON_SIZE,
            content.y,
            kit::ICON_BUTTON_SIZE,
            30.0,
        ),
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
    );
    let theme = UiRect::new(
        close.x - 8.0 - THEME_SLOT_W,
        content.y + (30.0 - kit::BUTTON_HEIGHT) / 2.0,
        THEME_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    (theme, close)
}

/// FR-059: окно скролла секций витрины (для wheel-hit без полной
/// раскладки — та же математика panel/content/шапки, что в раскладке).
pub fn gallery_scroll_viewport(viewport: [f32; 2]) -> UiRect {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let sections_top = content.y + 30.0 + SECTION_GAP;
    UiRect::new(
        content.x,
        sections_top,
        content.w,
        (content.bottom() - sections_top).max(0.0),
    )
}

/// Состояние интерактивного контрола по курсору (hover).
///
/// Deprecated (FR-057/FR-059): канонический путь — [`WidgetState`]
/// (`canvas_ui::widget`) — машина состояний hover/pressed/selected/disabled/
/// focused → [`KitState`] + ребро клика. Все потребители app.rs мигрированы
/// (FR-059 — витрина на [`WidgetState`]); делегат оставлен для совместимости
/// внешних вызывателей до FR-060.
#[deprecated(
    note = "FR-059: используйте canvas_ui::widget::WidgetState (set_pointer/set_disabled → kit_state)"
)]
pub fn cursor_state(hovered: bool, disabled: bool) -> KitState {
    let mut w = WidgetState::default();
    w.set_pointer(hovered, false);
    w.set_disabled(disabled);
    w.kit_state()
}

/// [f32;4] sRGB → glyphon Color (представление, не арифметика цвета).
pub fn color4(c: [f32; 4]) -> canvas_render::Color {
    canvas_render::Color::rgba(
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
        (c[3] * 255.0).round() as u8,
    )
}

/// Адаптер: модель кита → квад/текст ПОЛОСЫ кадра (правка дрейфа 2026-09-25: сырые
/// логические px — конвенция полос; единственный screen→world делает
/// рендер — `renderer::screen_instance_to_world`).
///
/// FR-057: тонкая обёртка над [`Painter`] (`canvas_ui::paint`) — модель items
/// собирается в крейте, конвертация в `CardInstance`/`OwnedText` — здесь
/// (забота потребителя, контракт G7). Методы и поведение 1:1 с прежней
/// реализацией: каждый вызов делегирует Painter'у и сразу конвертирует
/// добавленный item — quads/texts актуальны для app.rs после каждого вызова
/// (поля читаются напрямую — контракт сохранён дословно).
///
/// Правка дрейфа 2026-09-25: раньше квад здесь конвертировался screen→world
/// (`screen_rect_quad_pub`), а рендер полос делал это ВТОРОЙ раз — квады
/// поверхностей на KitDraw (витрина кита/админпанель/DebugOverlay/бейдж
/// автосвязи/чип покрытия) уезжали с камерой относительно screen-текстов
/// той же полосы («разъезжается при движении канваса»). Теперь квад —
/// сырой px полосы, как у не-дрейфующих поверхностей (settings/docs/…).
pub(crate) struct KitDraw {
    /// Журнал items Painter'а (модель крейта; порядок = draw-порядок).
    painter: Painter,
    pub quads: Vec<CardInstance>,
    pub texts: Vec<OwnedText>,
    /// FR-ICONS: инстансы SVG-иконок кадра (screen-space, поверх всех полос).
    /// Накапливаются при вызовах `icon()` (если активный набор — SVG; для
    /// Glyph — fallback через `label_center`, в `icons` ничего не падает).
    pub icons: Vec<canvas_render::IconInstance>,
    /// FR-ICONS: активный набор иконок (None = Glyph fallback; Some(set_id)
    /// = SVG-набор из `IconStyle::id()`). Устанавливается потребителем через
    /// `set_icon_set` после `KitDraw::new()`.
    icon_set: Option<&'static str>,
}

/// Владеемый screen-текст кадра (зеркало `app::OwnedScreenText` — модуль
/// без доступа к приватным полям app.rs; конвертация на кадре).
pub struct OwnedText {
    pub text: String,
    pub origin: [f32; 2],
    pub width: f32,
    pub font_size: f32,
    pub color: canvas_render::Color,
    pub align: TextAlign,
}

impl KitDraw {
    pub fn new() -> Self {
        Self {
            painter: Painter::new(),
            quads: Vec::new(),
            texts: Vec::new(),
            icons: Vec::new(),
            icon_set: None,
        }
    }

    /// FR-ICONS: установить активный набор иконок для последующих вызовов
    /// `icon()`. `None` = Glyph fallback (Unicode-глиф шрифтом); `Some(set_id)`
    /// = SVG-набор (`"lucide"`/`"material"`/`"feather"`/`"bootstrap"`).
    /// Вызывается потребителем после `KitDraw::new()` на основе
    /// `settings.icon_style`.
    pub fn set_icon_set(&mut self, set: Option<&'static str>) {
        self.icon_set = set;
    }

    /// FR-ICONS: иконка в области. Если активный набор — SVG (`set_icon_set`
    /// был вызван с `Some(set_id)`), ищется UV в атласе и в `self.icons`
    /// пушится `IconInstance` (квадратная вписка по центру `rect`, tint —
    /// RGBA). Иначе (Glyph fallback) — рисуется Unicode-глиф через
    /// `label_center` (прежнее поведение). Неизвестная пара (set, name) —
    /// fallback на глиф (мягкая деградация).
    pub fn icon(&mut self, rect: UiRect, name: &str, glyph: &str, tint: [f32; 4], glyph_size: f32) {
        if let Some(set) = self.icon_set {
            if let Some((uv_min, uv_max)) = canvas_render::icon_uv(set, name) {
                // Квадратная вписка по центру: размер = min(w, h), pos —
                // центрирован в rect. Сохраняем пропорции SVG-силуэта.
                let size = rect.w.min(rect.h);
                let pos = [
                    rect.x + (rect.w - size) * 0.5,
                    rect.y + (rect.h - size) * 0.5,
                ];
                self.icons.push(canvas_render::IconInstance {
                    pos,
                    size: [size, size],
                    uv_min,
                    uv_max,
                    tint,
                });
                return;
            }
            // Неизвестная пара (set, name) — fallback на глиф с предупреждением.
            tracing::warn!(set, name, "иконка не найдена в атласе — глиф-фолбэк");
        }
        // Glyph fallback (прежнее поведение): глиф шрифтом через label_center.
        self.label_center(rect, glyph, tint, glyph_size);
    }

    /// Прямоугольник с заливкой/рамкой/радиусом.
    pub fn rect(&mut self, r: UiRect, fill: [f32; 4], border: [f32; 4], radius: f32) {
        self.painter.rect(r, fill, border, radius);
        self.flush_last_quad();
    }

    /// Стиль контрола (заливка + рамка).
    pub fn control(&mut self, r: UiRect, s: &ControlStyle) {
        self.painter.control(r, s);
        self.flush_last_quad();
    }

    /// Подпись по центру области (контракт ScreenText: origin — левый край
    /// области выравнивания, Center центрирует в [origin, origin+width]).
    pub fn label_center(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32) {
        self.painter
            .label(area, text, color, size, PaintAlign::Center);
        self.flush_last_text();
    }

    /// Подпись слева.
    pub fn label_left(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32) {
        self.painter
            .label(area, text, color, size, PaintAlign::Left);
        self.flush_last_text();
    }

    /// Конвертация ПАЧКИ items Painter'а в квад/текст кадра (FR-061 этап E:
    /// составные кит-виджеты — [`canvas_ui::kit::paint_row`] — отдают сразу
    /// всю строку; порядок items = draw-порядок).
    pub fn paint_items(&mut self, items: Vec<canvas_ui::paint::PaintItem>) {
        for item in items {
            match item {
                canvas_ui::paint::PaintItem::Rect {
                    rect,
                    fill,
                    border,
                    radius,
                } => {
                    self.painter.rect(rect, fill, border, radius);
                    self.flush_last_quad();
                }
                canvas_ui::paint::PaintItem::Text {
                    area,
                    text,
                    color,
                    size,
                    align,
                } => {
                    self.painter.label(area, &text, color, size, align);
                    self.flush_last_text();
                }
                // FR-ICONS: иконка из составного кит-виджета (FR-061 строка
                // и т.п.) — делегирует `icon()` (SVG-атлас или глиф-фолбэк).
                // Глиф-фолбэк берётся из `icon_name` → `icon_glyph` (canvas-ui):
                // иконка типизирована, имя — стабильный идентификатор.
                canvas_ui::paint::PaintItem::Icon { rect, name, tint } => {
                    // Глиф для fallback: имя → Icon (если в реестре) → glyph.
                    // Неизвестное имя — пустой глиф (SVG уже отрисован, если
                    // набор активен; Glyph-набор с неизвестным именем —
                    // невалидная пара, Glyph fallback ничего не рисует).
                    let glyph = icon_name_to_glyph(&name);
                    self.icon(rect, &name, glyph, tint, 13.0);
                }
                // FR-068 W1: клип — прозрачный проход для consumer-обхода:
                // дети конвертируются как обычные items (draw-порядок
                // сохранён); отсечение — scissor FR-056 на стороне рендера.
                canvas_ui::paint::PaintItem::ClipRect { items, .. } => self.paint_items(items),
                // FR-074: Transform (rotate) и ZGroup — прозрачный проход
                // (как ClipRect в W1 до scissor): ZGroup-дети уже в
                // z-отсортированном порядке (Painter::take_items); поворот
                // пока не применяется рендером — формат инстансов без
                // rotation, конвертация в rotate-инстансы — отдельная
                // задача (данные Transform сохраняются на уровне canvas-ui).
                canvas_ui::paint::PaintItem::Transform { items, .. } => self.paint_items(items),
                canvas_ui::paint::PaintItem::ZGroup { items, .. } => self.paint_items(items),
            }
        }
    }

    /// Конвертация последнего Rect-item'а Painter'а в квад кадра
    /// (правка дрейфа 2026-09-25: сырые screen-px — конвенция полос; единственный
    /// screen→world делает рендер — двойная конверсия сюда не вернётся,
    /// см. `band_rect_quad_pub` в app.rs).
    fn flush_last_quad(&mut self) {
        if let Some(PaintItem::Rect {
            rect,
            fill,
            border,
            radius,
        }) = self.painter.items().last()
        {
            let quad = crate::app::band_rect_quad_pub(
                [rect.x, rect.y, rect.w, rect.h],
                *fill,
                *border,
                *radius,
            );
            self.quads.push(quad);
        }
    }

    /// Конвертация последнего Text-item'а Painter'а в владеемый текст кадра.
    fn flush_last_text(&mut self) {
        if let Some(PaintItem::Text {
            area,
            text,
            color,
            size,
            align,
        }) = self.painter.items().last()
        {
            let owned = OwnedText {
                text: text.clone(),
                origin: [area.x, area.y],
                width: area.w,
                font_size: *size,
                color: color4(*color),
                align: match align {
                    PaintAlign::Left => TextAlign::Left,
                    PaintAlign::Center => TextAlign::Center,
                },
            };
            self.texts.push(owned);
        }
    }
}

/// Подбор стиля строки dropdown по курсору (hover — реальный слот).
///
/// Deprecated (FR-057/FR-059): канонический путь — [`WidgetState`] (см.
/// [`cursor_state`]); потребители мигрированы в FR-059.
#[deprecated(
    note = "FR-059: используйте canvas_ui::widget::WidgetState (set_pointer(hovered, false) → kit_state)"
)]
pub fn dropdown_item_state(hovered: bool) -> KitState {
    #[allow(deprecated)]
    cursor_state(hovered, false)
}

/// Проверка «курсор внутри rect'а» (xywh UiRect).
pub fn cursor_in(r: &UiRect, cursor: [f32; 2]) -> bool {
    r.contains(UiPoint::new(cursor[0], cursor[1]))
}

/// FR-ICONS: обратное отображение `icon_name → glyph` для Glyph-fallback
/// (когда активный набор — Glyph, или пара (set, name) неизвестна). Имя —
/// стабильный идентификатор из `canvas_ui::kit::icon_name`; для табов
/// настроек (имена `tab_*`) — Unicode-глифы табов (прежнее поведение).
/// Неизвестное имя — пустой глиф (ничего не рисуется; SVG уже отрисован
/// для SVG-наборов, для Glyph — невалидная пара).
pub fn icon_name_to_glyph(name: &str) -> &'static str {
    use canvas_ui::kit::{icon_glyph, icon_name, Icon};
    // Сначала проверяем kit::Icon варианты (close/gear/question/search/plus/
    // arrow_left/arrow_right/refresh).
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
        if icon_name(icon) == name {
            return icon_glyph(icon);
        }
    }
    // Иконки табов настроек (прежние Unicode-глифы) — fallback для Glyph-набора.
    match name {
        "tab_general" => "◎",
        "tab_canvas" => "▦",
        "tab_snap" => "≡",
        "tab_edges" => "⇄",
        "tab_appearance" => "◐",
        _ => "",
    }
}

/// Публичный доступ к измерителю для сборки в app.rs (единый кадр —
/// один measurer).
pub fn new_measurer() -> TextMeasurer {
    TextMeasurer::new()
}

/// Обёртка измерения ширины (app.rs не тянет cosmic-text типы).
pub fn measured_width(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    text: &str,
    size: f32,
) -> f32 {
    m.width_of(fs, text, FONT_FAMILY, size)
}

/// Замер кнопки темы в шапке (та же функция кита, что и у отрисовки).
pub fn theme_button_layout(
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> (UiRect, String) {
    let label = tr(lang, "kit.gallery.theme");
    let bl = kit::button_layout(
        slot,
        &label,
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    (bl.rect, bl.label)
}

/// Замер, что нужен FontSystem — инициализация guard'а в app.rs (владение
/// рендера); модуль даёт тонкую обёртку.
pub fn with_font_system<R>(f: impl FnOnce(&mut cosmic_text::FontSystem) -> R) -> R {
    let mut guard = measure_font_system();
    f(&mut guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_ui::kit::{ButtonVariant, ControlStyle};

    /// FR-057: эквивалентность до/после делегирования — KitDraw через
    /// Painter даёт те же quads/texts, что прямой путь Painter-пути полос
    /// (правка дрейфа 2026-09-25: band_rect_quad_pub + OwnedText вручную) на фиксированном
    /// примере — 0 визуального скачка (критерий приёмки FR-057).
    #[test]
    fn kitdraw_delegation_matches_direct_path() {
        let fill = [0.2, 0.4, 0.6, 1.0];
        let border = [0.1, 0.2, 0.3, 0.9];
        let radius = 4.0;
        let style = ControlStyle {
            fill: [0.3, 0.3, 0.3, 1.0],
            border: [0.4, 0.4, 0.4, 1.0],
            text: [0.9, 0.9, 0.9, 1.0],
            radius: 6.0,
        };
        let area_c = UiRect::new(10.0, 20.0, 120.0, 30.0);
        let area_l = UiRect::new(4.0, 8.0, 80.0, 16.0);

        // НОВАЯ реализация (KitDraw → Painter → сырые px полосы)
        let mut d = KitDraw::new();
        d.rect(UiRect::new(1.0, 2.0, 3.0, 4.0), fill, border, radius);
        d.control(UiRect::new(5.0, 6.0, 7.0, 8.0), &style);
        d.label_center(area_c, "Центр", style.text, 13.0);
        d.label_left(area_l, "Лево", style.text, 11.0);

        // ЭТАЛОН — прямой путь Painter-пути полос (support::paint_items_to_band:
        // сырые логические px — единственный screen→world делает рендер)
        let mut quads = Vec::new();
        let mut texts = Vec::new();
        quads.push(crate::app::band_rect_quad_pub(
            [1.0, 2.0, 3.0, 4.0],
            fill,
            border,
            radius,
        ));
        // control = rect со слотами ControlStyle
        quads.push(crate::app::band_rect_quad_pub(
            [5.0, 6.0, 7.0, 8.0],
            style.fill,
            style.border,
            style.radius,
        ));
        texts.push(OwnedText {
            text: "Центр".to_owned(),
            origin: [area_c.x, area_c.y],
            width: area_c.w,
            font_size: 13.0,
            color: color4(style.text),
            align: TextAlign::Center,
        });
        texts.push(OwnedText {
            text: "Лево".to_owned(),
            origin: [area_l.x, area_l.y],
            width: area_l.w,
            font_size: 11.0,
            color: color4(style.text),
            align: TextAlign::Left,
        });

        // quads: CardInstance без PartialEq — сравнение по полям
        assert_eq!(d.quads.len(), quads.len());
        for (a, b) in d.quads.iter().zip(quads.iter()) {
            assert_eq!(a.pos, b.pos);
            assert_eq!(a.size, b.size);
            assert_eq!(a.fill, b.fill);
            assert_eq!(a.border, b.border);
            assert_eq!(a.params, b.params);
        }
        // texts: дословное равенство
        assert_eq!(d.texts.len(), texts.len());
        for (a, b) in d.texts.iter().zip(texts.iter()) {
            assert_eq!(a.text, b.text);
            assert_eq!(a.origin, b.origin);
            assert_eq!(a.width, b.width);
            assert_eq!(a.font_size, b.font_size);
            assert_eq!(a.color, b.color);
            assert_eq!(a.align, b.align);
        }
        // Журнал Painter отражает состав и порядок вызовов (модель крейта)
        assert_eq!(d.painter.items().len(), 4);
        assert!(matches!(d.painter.items()[0], PaintItem::Rect { .. }));
        assert!(matches!(d.painter.items()[1], PaintItem::Rect { .. }));
        assert!(matches!(d.painter.items()[2], PaintItem::Text { .. }));
        assert!(matches!(d.painter.items()[3], PaintItem::Text { .. }));
    }

    /// Deprecated-делегаты на WidgetState дают прежние результаты
    /// (эквивалентность таблицы состояний: disabled > hovered > normal).
    #[test]
    #[allow(deprecated)]
    fn cursor_state_delegates_match_old_matrix() {
        assert_eq!(cursor_state(false, false), KitState::Normal);
        assert_eq!(cursor_state(true, false), KitState::Hovered);
        assert_eq!(cursor_state(true, true), KitState::Disabled);
        assert_eq!(cursor_state(false, true), KitState::Disabled);
        // dropdown-строка: hover без disabled
        assert_eq!(dropdown_item_state(true), KitState::Hovered);
        assert_eq!(dropdown_item_state(false), KitState::Normal);
        // контрольный: ButtonVariant по-прежнему различим (делегаты не тронули кит)
        let _ = ButtonVariant::Primary;
    }

    /// FR-059: палитра-двойка для тестов витрины (значения слотов не важны —
    /// раскладка от палитры зависит только падом Card).
    fn gallery_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.1; 4],
            panel_border: [0.2; 4],
            control_fill: [0.3; 4],
            control_border: [0.4; 4],
            control_primary: [0.5; 4],
            control_danger: [0.6; 4],
            hover_fill: [0.7; 4],
            primary_hover_fill: [0.75; 4],
            selected_fill: [0.8; 4],
            text: [0.9; 4],
            text_title: [0.91; 4],
            text_muted: [0.92; 4],
            disabled_text: [0.93; 4],
            accent: [0.94; 4],
        }
    }

    /// FR-059: секции v2 в витрине — при offset 0 видимы только секции v1
    /// (v2 — за нижним краем); в нижнем положении скролла — TextField ×3,
    /// Switch ×4, Card, список (строки + бегунок), Icon-глифы ×4.
    #[test]
    fn gallery_layout_has_v2_sections_and_scroll() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let scroll = kit::ScrollState::default();
        let lay0 = gallery_layout([1280.0, 800.0], Language::Ru, &scroll, &p, &mut m, &mut fs);
        // Контент выше окна секций — скролл контента нужен
        assert!(
            lay0.content_h > lay0.sections_viewport.h,
            "контент {} > окна {}",
            lay0.content_h,
            lay0.sections_viewport.h
        );
        // При offset 0 v2-секции за краем — не рисуются (тексты кита
        // не клипятся по вертикали); Tab-цели (F-17) — БЕЗ фильтра
        assert!(lay0.text_fields.is_empty(), "v2 за краем при offset 0");
        assert_eq!(
            lay0.focus_targets.len(),
            GALLERY_FOCUS_SLOTS,
            "Tab-цели не фильтруются (контент-координаты)"
        );
        assert!(!lay0.button_rows.is_empty(), "секции v1 видимы");
        // Окно скролла из хелпера совпадает с раскладкой
        assert_eq!(
            gallery_scroll_viewport([1280.0, 800.0]),
            lay0.sections_viewport
        );
        // Нижнее положение скролла — хвост витрины (секции FR-062)
        // видим целиком
        let bottom = kit::ScrollState {
            offset: lay0.content_h - lay0.sections_viewport.h,
            content_h: lay0.content_h,
            viewport_h: lay0.sections_viewport.h,
        };
        let lay1 = gallery_layout([1280.0, 800.0], Language::Ru, &bottom, &p, &mut m, &mut fs);
        assert_eq!(lay1.measured_chips.len(), 3, "measured-ряд ×3 (F-13)");
        assert_eq!(lay1.grow_cells.len(), 3, "flex-ряд ×3 (F-14)");
        assert_eq!(lay1.wrap_chips.len(), GALLERY_WRAP_CHIPS, "wrap ×8 (F-15)");
        assert_eq!(lay1.grid_cells.len(), 8, "сетка 4×2 (F-16)");
        assert_eq!(
            lay1.focus_buttons.len(),
            GALLERY_FOCUS_SLOTS,
            "фокус ×4 (F-17)"
        );
        assert!(lay1.button_rows.is_empty(), "секции v1 ушли вверх");
        // Секции v2 — середина колонки: детерминированный скан смещения
        // (хвост витрины растёт — якоримся на факт видимости, не на
        // константу высот). FR-068 M3: колонка выросла (+ секция Table) —
        // блок TextField…Icons и табличные секции (Row+Table) не влезают в
        // окно ОДНИМ смещением; каждое семейство сканируется отдельно.
        let max_offset = lay0.content_h - lay0.sections_viewport.h;
        fn scan_offset(
            want: &dyn Fn(&GalleryLayout) -> bool,
            p: &KitPalette,
            m: &mut TextMeasurer,
            fs: &mut cosmic_text::FontSystem,
            content_h: f32,
            viewport_h: f32,
            max_offset: f32,
        ) -> GalleryLayout {
            let mut off = 0.0f32;
            loop {
                let s = kit::ScrollState {
                    offset: off,
                    content_h,
                    viewport_h,
                };
                let lay = gallery_layout([1280.0, 800.0], Language::Ru, &s, p, m, fs);
                if want(&lay) || off >= max_offset {
                    break lay;
                }
                off += 8.0;
            }
        }
        let lay_v2 = scan_offset(
            &|lay| lay.text_fields.len() == 3 && lay.icon_glyphs.len() == 4,
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
            max_offset,
        );
        assert_eq!(lay_v2.text_fields.len(), 3, "TextField ×3 состояния");
        assert!(
            lay_v2.text_fields[1].2.caret_x >= 0.0,
            "второе поле в фокусе"
        );
        assert_eq!(lay_v2.switches.len(), 4, "Switch ×4 состояния");
        assert!(lay_v2.card.is_some(), "Card построена");
        assert!(!lay_v2.list_rows.is_empty(), "строки списка видимы");
        assert!(
            lay_v2.list_scroll.needs_scroll(),
            "демо-список прокручивается"
        );
        assert_eq!(lay_v2.icon_glyphs.len(), 4, "Icon-глифы ×4");
        // Табличные секции (FR-061 D-15 Row + FR-068 M3 Table) — отдельный
        // скан: 4 демо-строки Row + 4 строки Table (те же данные через
        // компонент Table), значения всех 8 — на одной направляющей чисел
        let lay_rows = scan_offset(
            &|lay| lay.row_rows.len() == 8,
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
            max_offset,
        );
        assert_eq!(
            lay_rows.row_rows.len(),
            8,
            "kit-Row ×4 + Table ×4 (D-15, M3)"
        );
        let value_right = lay_rows.row_rows[0].lay.value.right();
        assert!(
            lay_rows
                .row_rows
                .iter()
                .all(|d| d.lay.value.w == 0.0 || (d.lay.value.right() - value_right).abs() < 0.01),
            "значения — на колоночной направляющей (D-4)"
        );
        // По одной строке с бейджем в КАЖДОЙ секции — пилюля на
        // бейдж-колонке
        assert_eq!(
            lay_rows
                .row_rows
                .iter()
                .filter(|d| d.lay.badge.is_some())
                .count(),
            2,
            "бейдж-демо «← источник» — по одной строке в Row и Table"
        );
        // Состояния/зебра демо — в обеих секциях (порядок: 4 строки Row,
        // затем 4 строки Table)
        for section in lay_rows.row_rows.chunks(4) {
            assert!(section[1].zebra, "вторая строка — зебра (hover_fill)");
            assert_eq!(section[3].state, KitState::Selected, "Σ — Selected");
        }
        // FR-068 W3 (M3) — оракул бит-в-бит на уровне приложения: Table
        // (row_layout_with) ≡ Row v1 (row_guides+row_layout) на ТЕХ ЖЕ
        // данных, слотах и замерщике — геометрия строк совпадает дословно
        // с точностью до вертикального сдвига секций (прецедент T1
        // canvas-ui; RowLayout — PartialEq, включая label_shown). Пустые
        // ячейки value/unit — сентинел (0,0,0,0) в координатах кадра
        // (после сдвига скролла y = −offset) — нормализуются: w=0 → нули.
        let dy = lay_rows.row_rows[4].lay.row.y - lay_rows.row_rows[0].lay.row.y;
        assert!(
            (dy - (4.0 * GALLERY_ROW_H + SECTION_GAP + 18.0)).abs() < 0.01,
            "Table-секция следует за Row с шагом секции (18 + 4·row_h + gap)"
        );
        let normalize = |lay: &kit::RowLayout| -> kit::RowLayout {
            let mut l = lay.clone();
            if l.value.w == 0.0 {
                l.value = UiRect::new(0.0, 0.0, 0.0, 0.0);
            }
            if l.unit.w == 0.0 {
                l.unit = UiRect::new(0.0, 0.0, 0.0, 0.0);
            }
            l
        };
        for i in 0..4 {
            assert_eq!(
                normalize(&shift_row_lay(lay_rows.row_rows[i + 4].lay.clone(), dy)),
                normalize(&lay_rows.row_rows[i].lay),
                "строка {i}: Table ≡ Row (бит-в-бит, M3)"
            );
        }
    }

    /// FR-059: скролл витрины — сдвиг секций, шапка на месте; после сдвига
    /// все видимые подписи — внутри окна секций.
    #[test]
    fn gallery_scroll_shifts_sections() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let scroll = kit::ScrollState::default();
        let lay0 = gallery_layout([1280.0, 800.0], Language::Ru, &scroll, &p, &mut m, &mut fs);
        let scrolled = kit::ScrollState {
            offset: 60.0,
            ..kit::ScrollState::default()
        };
        let lay1 = gallery_layout(
            [1280.0, 800.0],
            Language::Ru,
            &scrolled,
            &p,
            &mut m,
            &mut fs,
        );
        assert_eq!(lay1.content_h, lay0.content_h, "контент не меняется");
        // После сдвига первая подпись (Buttons) ушла, ни одна подпись
        // не осталась выше окна секций
        assert!(
            lay1.section_titles
                .iter()
                .all(|(origin, _)| { origin.y >= lay1.sections_viewport.y - 0.01 }),
            "подписи — внутри окна после сдвига"
        );
        // Шапка не скроллится
        assert_eq!(lay1.close, lay0.close);
        assert_eq!(lay1.theme, lay0.theme);
    }
    /// FR-068 W3 (этап M3, оракул бит-в-бит): секция Table ≡ секция Row на
    /// ОДНИХ демо-данных и параметрах — раскладки строк ([`kit::RowLayout`],
    /// включая `label_shown`) равны дословно: [`kit::Table::row_layout_with`]
    /// делегирует тем же kit-функциям ([`kit::row_guides`]/[`kit::row_layout`]).
    #[test]
    fn table_section_row_layouts_match_row_section() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let demo = gallery_row_demo(Language::Ru);
        let (control_x, control_w, y0) = (40.0, 300.0, 100.0);
        let viewport_right = control_x + control_w;

        // Путь Row (витрина примитива): row_guides + row_layout per слот
        let parts: Vec<kit::RowParts<'_>> = demo.iter().map(|(parts, _, _)| *parts).collect();
        let rg = kit::row_guides(
            &mut m,
            &mut fs,
            FONT_FAMILY,
            LABEL_SIZE,
            &parts,
            viewport_right,
            canvas_core::tokens::TABLE_GUIDE_GAP,
        )
        .expect("демо имеет живые правые колонки");
        let row_lays: Vec<kit::RowLayout> = demo
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let slot = UiRect::new(
                    control_x,
                    y0 + i as f32 * GALLERY_ROW_H,
                    control_w,
                    GALLERY_ROW_H,
                );
                kit::row_layout(
                    &mut m,
                    &mut fs,
                    FONT_FAMILY,
                    LABEL_SIZE,
                    slot,
                    rg,
                    &parts[i],
                    &kit::RowOpts::default(),
                )
            })
            .collect();

        // Путь Table (M3-секция): set_rows + row_layout_with (те же слоты)
        let mut table = kit::Table::new(kit::TableProps {
            size: LABEL_SIZE,
            family: FONT_FAMILY,
            opts: kit::TableOpts {
                leader: true,
                gap: canvas_core::tokens::TABLE_GUIDE_GAP,
                row_h: GALLERY_ROW_H,
                row_gap: 0.0,
                right_pad: 0.0,
            },
            palette: p,
        });
        table.set_rows(
            demo.iter()
                .copied()
                .map(|(parts, state, zebra)| kit::TableRow {
                    marker: parts.marker,
                    label: parts.label.to_owned(),
                    value: parts.value.to_owned(),
                    unit: parts.unit.to_owned(),
                    badge: (!parts.badge.is_empty()).then(|| parts.badge.to_owned()),
                    state,
                    style: kit::TableRowStyle {
                        fill: zebra.then_some(p.hover_fill),
                        ..kit::TableRowStyle::default()
                    },
                })
                .collect(),
        );
        let table_lays: Vec<kit::RowLayout> = (0..demo.len())
            .map(|i| {
                let slot = UiRect::new(
                    control_x,
                    y0 + i as f32 * GALLERY_ROW_H,
                    control_w,
                    GALLERY_ROW_H,
                );
                table
                    .row_layout_with(&mut m, &mut fs, viewport_right, slot, i)
                    .expect("индекс в границах демо")
            })
            .collect();

        assert_eq!(
            row_lays, table_lays,
            "Row ≡ Table: раскладки строк дословно (геометрия + label_shown)"
        );
    }
}
