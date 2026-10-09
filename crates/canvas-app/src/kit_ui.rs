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
use canvas_ui::component::Component as _;
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
// FR-062 (layout v2): примитивы measured/flex/wrap/grid — раскладка витрины
// использует те же функции, что и потребители (живой образец).
use canvas_ui::kit::{self, ButtonVariant, ControlStyle, KitPalette, KitState};
// FR-068 W1 (ADR-0014 §Решение п.5 P1): витрина — pilot-поверхность, раскладка
// секций v2 идёт через [`pilot_backend`] (taffy за фичей — opt-in; на default
// сборке backend тот же Native — геометрия байт-в-байт прежняя).
// LAY-SHOWCASE (design/rules/11-layouts.md): примитивы LAY2 (constrain/pad/
// stack) и маска движка LAY5.2 (LayoutFeatures) для сцен-секций.
use canvas_ui::layout::{
    constrain, grid_cells_with, pad, pilot_backend, stack, Child, Column, FlexLayoutEngine, HAlign,
    LayoutFeatures, MeasuredItem, Row, RowPolicy, SceneDim, SceneNode, ScenePosition, SceneSize,
    VAlign,
};
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
/// Отступ секций по вертикали (spacing-scale) — токен
/// `canvas_core::tokens::SPACING_LG` (значение прежнего литерала 12).
pub use canvas_core::tokens::SPACING_LG as SECTION_GAP;
/// Высота строки подписи секции в линейке (шаг прежнего курсора; кегль
/// подписи 11, окно отрисовки 16 — overlays.rs). Вне S3 — пин геометрии,
/// кандидат в `kit::CONST` (LAY-W7).
const SECTION_TITLE_H: f32 = 18.0;
/// Историческая надбавка зазора после секции Buttons (+4 px к SECTION_GAP;
/// вне шкалы S1 — LAY7-P3). Геометрия пинена golden-тестом линейки.
const BUTTONS_SECTION_GAP_EXTRA: f32 = 4.0;
/// Исторический зазор между якорем и меню демо-dropdown (вне шкалы S1 —
/// LAY7-P3). Геометрия пинена golden-тестом линейки.
const DROPDOWN_ANCHOR_GAP: f32 = 4.0;
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
/// FR-068 W3 (этап M3): секция Table — табличные строки через компонент
/// [`canvas_ui::kit::Table`]. W3.3 (решение владельца): витрина Row v1
/// (FR-061) заменена компонентом — секция единственная; паритет Row-пути
/// («Row ≡ Table на одних данных») — оракул в тестах модуля.
pub const SECTION_TABLE: &str = "kit.section.table";
/// FR-062: секции layout v2 (measured/flex/wrap/grid/focus).
pub const SECTION_MEASURED: &str = "kit.section.measured";
pub const SECTION_GROW: &str = "kit.section.grow";
pub const SECTION_WRAP: &str = "kit.section.wrap";
pub const SECTION_GRID: &str = "kit.section.grid";
pub const SECTION_FOCUS: &str = "kit.section.focus";
/// Пересборка поверхностей (волна 2): демо компонентного слоя
/// (`component::Row`/`component::Panel`) и недостающих layout-примитивов
/// (SqueezeTail, MainAlign::SpaceBetween, Column) — полный инвентарь кита.
pub const SECTION_COMPONENT_ROW: &str = "kit.section.component_row";
pub const SECTION_COMPONENT_PANEL: &str = "kit.section.component_panel";
pub const SECTION_SQUEEZE: &str = "kit.section.squeeze";
pub const SECTION_ALIGN: &str = "kit.section.align";
/// LAY-SHOWCASE (design/rules/11-layouts.md LAY11 п.10): витрина раскладок —
/// механизмы, которых ещё не было в секциях (инвентаризация LAY2/LAY5/LAY7/
/// LAY8 против SECTION_MEASURED/GROW/WRAP/GRID/FOCUS/SQUEEZE/ALIGN).
pub const SECTION_LAYOUT_CONSTRAIN: &str = "kit.section.layout_constrain";
pub const SECTION_LAYOUT_PAD: &str = "kit.section.layout_pad";
pub const SECTION_LAYOUT_STACK: &str = "kit.section.layout_stack";
pub const SECTION_LAYOUT_GAPS: &str = "kit.section.layout_gaps";
pub const SECTION_LAYOUT_PERCENT: &str = "kit.section.layout_percent";
pub const SECTION_LAYOUT_ASPECT: &str = "kit.section.layout_aspect";
pub const SECTION_LAYOUT_STICKY: &str = "kit.section.layout_sticky";
pub const SECTION_LAYOUT_HIDE_BELOW: &str = "kit.section.layout_hide_below";

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
/// FR-068 (W3.3): высота демо-строки табличной секции Table
/// (панель FR-044 — 22; унаследована от демо Row FR-061).
pub const GALLERY_ROW_H: f32 = 24.0;
/// FR-068 (W3.3): демо-значения строк Table-секции (числа — без i18n;
/// те же данные строит оракул «Row ≡ Table» в тестах модуля).
pub const GALLERY_ROW_VALUE_PRICE: &str = "50";
pub const GALLERY_ROW_VALUE_QTY: &str = "12";
pub const GALLERY_ROW_VALUE_TOTAL: &str = "600";
pub const GALLERY_ROW_VALUE_SUM: &str = "5 400";
/// FR-068 (W3.3): глиф формульной строки демо (calc-маркер Р-4).
pub const GALLERY_ROW_FORMULA_GLYPH: &str = "ƒ";

/// Пересборка поверхностей (волна 2): боксов в SqueezeTail-демо (деградация
/// видна на всей шкале панели: 12·72 + 11·8 = 952 > ширины контрол-колонки).
pub const GALLERY_SQUEEZE_BOXES: usize = 12;
/// Пересборка поверхностей (волна 2): база SqueezeTail-бокса (ui px).
pub const GALLERY_SQUEEZE_BOX_W: f32 = 72.0;
/// Пересборка поверхностей (волна 2): высота бокса SqueezeTail/колонки.
pub const GALLERY_SQUEEZE_BOX_H: f32 = 20.0;
/// Пересборка поверхностей (волна 2): высота демо-панели component::Panel.
pub const GALLERY_COMPONENT_PANEL_H: f32 = 88.0;

// === LAY-SHOWCASE (design/rules/11-layouts.md, LAY11 п.10): константы
// демо витрины раскладок. Механизмы, которых ещё не было в секциях:
// constrain/pad/stack (LAY2), шкала зазоров S1 (LAY7), сцена
// percent+Fill / aspect-ratio / sticky (LAY5), деградация HideBelow (LAY8).

/// LAY2: высота демо-блока constrain/percent.
pub const GALLERY_LAYOUT_BLOCK_H: f32 = 24.0;
/// LAY2 constrain: нижняя граница демо (min приоритетнее max — LAY2).
pub const GALLERY_CONSTRAIN_MIN: f32 = 48.0;
/// LAY2 constrain: верхняя граница демо.
pub const GALLERY_CONSTRAIN_MAX: f32 = 120.0;
/// LAY2 pad/stack: высота демо-ячеек.
pub const GALLERY_LAYOUT_CELL_H: f32 = 56.0;
/// LAY2 stack: размер фиксированного блока в слоте («модалка»);
/// высота = 2×BUTTON_HEIGHT — вторична, ширина — демо-константа слота.
pub const GALLERY_STACK_BLOCK: UiVec2 = UiVec2::new(120.0, 32.0);
/// LAY7: ширина блока демо шкалы зазоров.
pub const GALLERY_GAP_BLOCK_W: f32 = 64.0;
/// LAY7: высота блока демо шкалы зазоров.
pub const GALLERY_GAP_BLOCK_H: f32 = 20.0;
/// LAY5 sticky: высота строки демо-окна прокрутки.
pub const GALLERY_STICKY_ROW_H: f32 = 24.0;
/// LAY5 sticky: строк контента в демо (в окне видны 3 — контент прокручен).
pub const GALLERY_STICKY_ROWS: usize = 8;
/// LAY5 sticky: демо-сдвиг окна (2 строки — шапка прилипает к top: 0).
pub const GALLERY_STICKY_DEMO_OFFSET: f32 = 48.0;
/// LAY5 sticky: строк в видимом окне (высота окна выводится из константы).
pub const GALLERY_STICKY_VISIBLE_ROWS: usize = 3;
/// LAY8: высота слота демо HideBelow.
pub const GALLERY_HIDE_BELOW_H: f32 = 56.0;
/// LAY8: размер демо-панели внутри слота (stack по центру).
pub const GALLERY_HIDE_BELOW_PANEL: UiVec2 = UiVec2::new(220.0, 36.0);
/// LAY8 п.3: порог демо HideBelow — канонический минимум what-if бара
/// (registry.rs: surface "whatif" — HideBelow 900×600; ниже минимума
/// панель скрывается ЦЕЛИКОМ, не сжимается — LAY8 п.3).
pub const GALLERY_HIDE_BELOW_MIN: UiVec2 = UiVec2::new(900.0, 600.0);
/// Пересборка поверхностей (волна 2): ширина/высота ячейки демо-колонки
/// (Column::lay_out_measured, MeasuredItem::Fixed — именованный эквивалент).
pub const GALLERY_COLUMN_CELL_W: f32 = 160.0;
pub const GALLERY_COLUMN_CELL_H: f32 = 20.0;
/// Пересборка поверхностей (волна 2): значение демо-строки component::Row.
pub const GALLERY_COMPONENT_ROW_VALUE: &str = "800";

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
    /// FR-059: переключатели v2 — (slot, on, состояние, в фокусе — рамка
    /// accent рисуется потребителем).
    pub switches: Vec<(UiRect, bool, KitState, bool)>,
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
    /// FR-068 W3 (M3, W3.3): демо-строки табличной секции Table — данные
    /// [`gallery_row_demo`] на направляющих компонента Table; отрисовка —
    /// [`canvas_ui::kit::paint_row`] по предвычисленному
    /// [`canvas_ui::kit::RowLayout`] (app/overlays.rs).
    pub row_rows: Vec<RowDemoRow>,
    /// Пересборка поверхностей (волна 2): слот демо component::Row
    /// (отрисовка — [`canvas_ui::component::row::Row::paint`] в оверлее;
    /// нулевой rect — секция за краем окна секций).
    pub component_row_slot: UiRect,
    /// Пересборка поверхностей (волна 2): Props демо component::Row
    /// (retained-контракт компонента — владеет строками; язык — в label).
    pub component_row_props: canvas_ui::component::row::RowProps,
    /// Пересборка поверхностей (волна 2): демо component::Panel —
    /// (панель, контент) из [`canvas_ui::component::panel::Panel::layout`].
    pub component_panel: Option<(UiRect, UiRect)>,
    /// Пересборка поверхностей (волна 2): слот SqueezeTail-демо (рамка) и
    /// боксы: хвост ряда сжат политикой ([`RowPolicy::SqueezeTail`]).
    pub squeeze_slot: UiRect,
    pub squeeze_cells: Vec<UiRect>,
    /// Пересборка поверхностей (волна 2): MainAlign::SpaceBetween —
    /// (rect, ключ подписи): свободное место распределено в зазоры.
    pub align_between: Vec<(UiRect, &'static str)>,
    /// Пересборка поверхностей (волна 2): Column + распорка — ячейки
    /// вертикального стека (зазор между 1-й и 2-й = gap + Spacer).
    pub column_cells: Vec<UiRect>,
    /// LAY-SHOWCASE (LAY2 constrain): (блок, желаемая ширина); ширина блока =
    /// результат `constrain(min, max, desired)` — подпись «желаемое → итог».
    pub layout_constrain: Vec<(UiRect, f32)>,
    /// LAY-SHOWCASE (LAY2 pad + LAY7): (ячейка, внутренний rect после
    /// `pad(cell, EdgeInsets::uniform(g))`, имя ступени S1).
    pub layout_pad: Vec<(UiRect, UiRect, &'static str)>,
    /// LAY-SHOWCASE (LAY2 stack): (слот, блок) — Center/Center и End/End
    /// («модалка по центру» / «прижата к углу»).
    pub layout_stack: Vec<(UiRect, UiRect)>,
    /// LAY-SHOWCASE (LAY7): строки шкалы зазоров S1 — пара блоков с зазором
    /// ступени + подпись «ступень · значение» (значение — токен шкалы).
    pub layout_gaps: Vec<GapDemoRow>,
    /// LAY-SHOWCASE (LAY5, SceneNode): percent-треки + Fill — доли ширины
    /// (20% / 30% / Fill); пусто, если маска движка без PERCENT/FLEX_GROW.
    pub layout_percent: Vec<UiRect>,
    /// LAY-SHOWCASE (LAY5, SceneNode): aspect-ratio — превью-плитки 16:9;
    /// пусто, если маска движка без ASPECT_RATIO.
    pub layout_aspect: Vec<UiRect>,
    /// LAY-SHOWCASE (LAY5, SceneNode): sticky-шапка в прокручиваемом окне
    /// (демо-сдвиг; строки — полностью видимые); None — секция за краем
    /// окна или маска движка без STICKY.
    pub layout_sticky: Option<StickyDemo>,
    /// LAY-SHOWCASE (LAY8): HideBelow — слот + панель (None при скрытии
    /// ниже порога [`GALLERY_HIDE_BELOW_MIN`] — скрывается ЦЕЛИКОМ).
    pub layout_hide_below: HideBelowDemo,
}

/// LAY-SHOWCASE (LAY7): строка демо шкалы зазоров S1 — два блока с зазором
/// ступени + подпись (текст = измеренная строка [`MeasuredItem::Text`]).
#[derive(Debug, Clone)]
pub struct GapDemoRow {
    /// Пара блоков (зазор между ними = значение ступени шкалы S1).
    pub blocks: Vec<UiRect>,
    /// Подпись ступени (rect из раскладки; ширина = измеренная + pad).
    pub label: UiRect,
    /// Текст подписи («S · 6» — тот же, что в замере раскладки).
    pub caption: String,
}

/// LAY-SHOWCASE (LAY5 sticky): демо «sticky-шапка в окне прокрутки» —
/// окно (SceneOverflow::Hidden + offset), шапка (Sticky top 0, кламп
/// post-processing'ом сцены) и полностью видимые строки контента.
#[derive(Debug, Clone)]
pub struct StickyDemo {
    /// Окно прокрутки (контейнер сцены; == слот секции).
    pub window: UiRect,
    /// Sticky-шапка (прилипла к `window.y + top` при демо-сдвиге).
    pub header: UiRect,
    /// Строки контента, целиком видимые в окне (частичные — не рисуются:
    /// тексты кита не клипятся — паттерн видимости витрины).
    pub rows: Vec<UiRect>,
}

/// LAY-SHOWCASE (LAY8): демо деградации HideBelow — панель с подписью
/// минимума; при окне ниже порога панель исчезает ЦЕЛИКОМ (п.3 LAY8:
/// «промежуточных ступеней нет»).
#[derive(Debug, Clone)]
pub struct HideBelowDemo {
    /// Слот секции (рамка видна всегда — демо-контейнер).
    pub slot: UiRect,
    /// Панель ([`stack`] по центру слота) — None, если скрыта порогом или
    /// секция за краем окна секций.
    pub panel: Option<UiRect>,
    /// Порог сработал (`DegradationPolicy::HideBelow.hidden_at(viewport)`)
    /// — флаг от вьюпорта приложения, не зависит от скролла витрины.
    pub hidden: bool,
}

/// FR-068 (W3.3): демо-данные табличной секции витрины (Table) — 4 строки:
/// Dot/цена, Dot/кол-во + зебра, Glyph ƒ/итого + бейдж, Σ Selected.
/// Возвращает `(части строки кит-Row, состояние, зебра)` — Table-версия
/// строится из тех же значений ([`TableRow`](canvas_ui::kit::TableRow));
/// те же данные использует оракул «Row ≡ Table» в тестах модуля.
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

/// FR-068 (W3.3): строка демо-таблицы витрины — данные кит-строки +
/// геометрия ([`canvas_ui::kit::Table::row_layout_with`]). Структуру
/// наполняет секция Table (единый vec [`GalleryLayout::row_rows`]);
/// оракул бит-в-бит «Row ≡ Table на одних данных» (kit::row_guides/
/// row_layout против Table) — в тестах модуля.
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

/// Отступ панели витрины от краёв вьюпорта (spacing-scale XL) — токен
/// `canvas_core::tokens::SPACING_XL` (значение прежнего литерала 24).
use canvas_core::tokens::SPACING_XL as VIEWPORT_MARGIN;

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

    let y = content.y;
    let full_w = content.w;

    // Шапка: заголовок слева, кнопка темы + «✕» справа — раскладываются
    // вместе через `kit::panel_header` (FR-070, агент L: panel_header
    // раскладывает toggle + icon-кнопки в едином right-aligned ряду с
    // каноничным inset SPACING_SM и зазором GAP_CONTROLS). Шапка
    // ФИКСИРОВАНА — не скроллится (hit-слоты реестра без изменений).
    //
    // Канонизация (AGENTS.md I-1): `kit::panel_header` прижимает close к
    // `slot.right - SPACING_SM` (8px inset), тогда как прежний код ставил
    // close flush к `content.right` (0 inset) — close и theme сдвигаются
    // на 8px влево (внутрь панели), визуально canonical-выравнивание с
    // `kit::modal`/`kit::panel` хромой.
    let title = UiRect::new(
        content.x,
        y,
        (full_w - 2.0 * (kit::ICON_BUTTON_SIZE + 8.0)).max(0.0),
        30.0,
    );
    let header_slot = UiRect::new(content.x, y, content.w, 30.0);
    // Порядок справа-налево: theme (toggle, левее), close (icon, правее).
    // `panel_header` класть кнопки right-to-left от `slot.right - SPACING_SM`.
    let header_buttons = [
        kit::HeaderButton::Toggle {
            label_w: THEME_SLOT_W,
            id: "theme",
        },
        kit::HeaderButton::Icon {
            kind: kit::HeaderIconKind::Close,
            id: "close",
        },
    ];
    let (header_layout, _header_style) = kit::panel_header(header_slot, &header_buttons, p);
    let mut close = UiRect::default();
    let mut theme = UiRect::default();
    for (r, id) in &header_layout.buttons {
        match *id {
            "close" => close = *r,
            "theme" => theme = *r,
            _ => {}
        }
    }
    // Шаг шапки (30 + SECTION_GAP) фиксирован — скролл-контракт FR-059:
    // тот же шаг повторяет `gallery_scroll_viewport` (окно секций).
    let sections_top = y + 30.0 + SECTION_GAP;
    // Окно скролла секций (шапка выше — фиксирована)
    let sections_viewport = UiRect::new(
        content.x,
        sections_top,
        content.w,
        (content.bottom() - sections_top).max(0.0),
    );

    let mut section_titles: Vec<(UiPoint, &'static str)> = Vec::new();
    let mut button_rows: Vec<ButtonRow> = Vec::new();
    let mut icon_buttons: Vec<UiRect> = Vec::new();
    let mut chips: Vec<UiRect> = Vec::new();
    let mut dropdown_items: Vec<UiRect> = Vec::new();

    // Подпись состояния слева + контрол в остатке ширины
    let control_x = content.x + STATE_LABEL_W + canvas_core::tokens::SPACING_SM;
    let control_w = (content.right() - control_x).max(0.0);

    // === LAY-W3b: вертикальная линейка секций — Column-скелет примитивов
    // (аудит layouts-2026-10 §3.2/§3.10 → §5 LAY-W3). Одна раскладка
    // `Column { gap: 0 }` через [`pilot_backend`] (NativeBackend — семантика
    // прежнего курсора байт-в-байт, LAY9.1) из фиксированных блоков
    // «подпись + контент» и именованных распорок-разделителей — вместо
    // ручного курсора `y +=`. Зазоры — шкала S1: SECTION_GAP (алиас
    // SPACING_LG), шаги демо-рядов Buttons/Gaps — SPACING_SM; исторические
    // отклонения (+4 px после Buttons, нулевые после Focus/Align) сохранены
    // и пинены golden-тестом gallery_layout_ruler_golden_column_skeleton.
    // Сами демо-ряды внутри секций (Row/lay_out_measured/pilot_backend)
    // не затронуты. ===

    // Высоты контента секций — до раскладки линейки (те же выражения, что
    // у демо-блоков ниже).
    let toast_h = 20.0;
    let card_h = 64.0;
    let dropdown_item_h = 26.0;
    let dropdown_menu_h = 3.0 * dropdown_item_h + 2.0 * 4.0;
    let wrap_slot_h = 2.0 * kit::CHIP_HEIGHT + kit::GAP_CONTROLS;
    let grid_cell_h = 32.0;
    let grid_slot_h = 2.0 * grid_cell_h + kit::GAP_CONTROLS;
    let align_column_block_h = 3.0 * GALLERY_COLUMN_CELL_H + 3.0 * 6.0 + 12.0;
    let aspect_tile_w = ((control_w - 2.0 * kit::GAP_CONTROLS) / 3.0).max(0.0);
    let aspect_h = aspect_tile_w * 9.0 / 16.0;
    let sticky_window_h = GALLERY_STICKY_ROW_H
        + canvas_core::tokens::SPACING_S
        + GALLERY_STICKY_VISIBLE_ROWS as f32 * GALLERY_STICKY_ROW_H
        + (GALLERY_STICKY_VISIBLE_ROWS.saturating_sub(1)) as f32 * canvas_core::tokens::SPACING_S;

    // Блок линейки = (ширина колонки, высота); распорка = зазор линейки.
    // ВАЖНО: зазор — Fixed{h}, а НЕ `Spacer`: в Column спейсер занимает
    // нулевую высоту (он про ширину ряда — см. тест canvas-ui
    // «Spacer в колонке — нулевая высота»), линейка бы схлопнулась.
    // Отдельный rect-зазор перед раскладкой СЛИВАЕТСЯ в предыдущий блок
    // (см. merge-проход ниже): потребители читают из итератора ровно
    // 2 rect'а на секцию (подпись, контент).
    fn block(w: f32, h: f32) -> MeasuredItem<'static> {
        MeasuredItem::Fixed { w, h }
    }
    fn ruler_gap(len: f32) -> MeasuredItem<'static> {
        MeasuredItem::Fixed { w: 0.0, h: len }
    }
    let mut ruler_items: Vec<MeasuredItem> = Vec::with_capacity(64);
    // --- Buttons: подпись + 4 ряда (шаг SPACING_SM; хвост +4 px — пин) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(
            full_w,
            4.0 * kit::BUTTON_HEIGHT + 3.0 * canvas_core::tokens::SPACING_SM,
        ),
        ruler_gap(SECTION_GAP + BUTTONS_SECTION_GAP_EXTRA),
    ]);
    // --- IconButtons ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::ICON_BUTTON_SIZE),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Chips ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::CHIP_HEIGHT),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Dropdown: якорь + зазор + меню (зазор — DROPDOWN_ANCHOR_GAP, пин) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(
            full_w,
            kit::BUTTON_HEIGHT + DROPDOWN_ANCHOR_GAP + dropdown_menu_h,
        ),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Toast ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, toast_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Tooltip (хвост 18 + 26 — шаг прежнего курсора, пин) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, 18.0 + 26.0),
        ruler_gap(SECTION_GAP),
    ]);
    // --- TextField ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::TEXT_FIELD_HEIGHT),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Switch ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::SWITCH_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Card ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, card_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Список + скролл ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_LIST_VIEWPORT_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Icon-глифы v2 ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::ICON_BUTTON_SIZE),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Table: 4 строки без межстрочных зазоров (row_gap: 0) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, 4.0 * GALLERY_ROW_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Measured-ряд (F-13) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::CHIP_HEIGHT),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Flex-факторы (F-14) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::BUTTON_HEIGHT),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Wrap (F-15) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, wrap_slot_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Сетка (F-16) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, grid_slot_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Фокус (F-17): нулевой хвост до ComponentRow — исторический, пин ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, kit::BUTTON_HEIGHT),
    ]);
    // --- Компонент Row ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_ROW_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Компонент Panel ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_COMPONENT_PANEL_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- SqueezeTail ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_SQUEEZE_BOX_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Align: SpaceBetween-ряд + Column-демо с зазором SPACING_S;
    // нулевой хвост до Constrain — исторический, пин ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(
            full_w,
            kit::CHIP_HEIGHT + canvas_core::tokens::SPACING_S + align_column_block_h,
        ),
    ]);
    // --- Constrain (LAY2) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_LAYOUT_BLOCK_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Pad (LAY2 + LAY7) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_LAYOUT_CELL_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Stack (LAY2) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_LAYOUT_CELL_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Шкала зазоров S1 (LAY7): 5 рядов с шагом SPACING_SM ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(
            full_w,
            5.0 * GALLERY_GAP_BLOCK_H + 4.0 * canvas_core::tokens::SPACING_SM,
        ),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Сцена: percent + Fill (LAY5) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_LAYOUT_BLOCK_H),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Сцена: aspect-ratio (LAY5) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, aspect_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Сцена: sticky-шапка (LAY5) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, sticky_window_h),
        ruler_gap(SECTION_GAP),
    ]);
    // --- Деградация HideBelow (LAY8 п.3) + хвостовая распорка линейки
    // (входит в content_h — скролл-контракт FR-059) ---
    ruler_items.extend([
        block(full_w, SECTION_TITLE_H),
        block(full_w, GALLERY_HIDE_BELOW_H),
        ruler_gap(SECTION_GAP),
    ]);

    // Merge-проход: rect-зазоры (Fixed{w:0}) сливаются в высоту ПРЕДЫДУЩЕГО
    // блока. Потребители линейки читают из итератора ровно 2 rect'а на
    // секцию (подпись, контент) — отдельный зазор съедался бы как подпись
    // следующей секции (сдвиг линейки на величину зазора). Слияние
    // сохраняет сумму высот и все позиции бит-в-бит; зазоры остаются
    // токенами S1 (SECTION_GAP / BUTTONS_SECTION_GAP_EXTRA).
    let mut merged_items: Vec<MeasuredItem> = Vec::with_capacity(ruler_items.len());
    for item in ruler_items {
        let is_gap = matches!(item, MeasuredItem::Fixed { w: 0.0, .. });
        if is_gap {
            let gap_h = match item {
                MeasuredItem::Fixed { h, .. } => h,
                _ => 0.0,
            };
            if let Some(MeasuredItem::Fixed { h, .. }) = merged_items.last_mut() {
                *h += gap_h;
            } else {
                merged_items.push(item);
            }
        } else {
            merged_items.push(item);
        }
    }
    let ruler_items = merged_items;

    // Раскладка линейки: Column (NativeBackend — прежняя арифметика курсора
    // байт-в-байт; MainAlign::Start — блоки от верха окна секций).
    let mut ruler = Column {
        gap: 0.0,
        ..Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        sections_viewport,
        &ruler_items,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    )
    .into_iter();

    // --- Buttons: 4 варианта × 4 состояния (ряды — вложенный Column
    // линейки с шагом SPACING_SM) ---
    let buttons_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(buttons_title.x, buttons_title.y),
        SECTION_BUTTONS,
    ));
    let button_row_rects = Column {
        gap: canvas_core::tokens::SPACING_SM,
        ..Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        ruler.next().unwrap_or_default(),
        &[
            block(full_w, kit::BUTTON_HEIGHT),
            block(full_w, kit::BUTTON_HEIGHT),
            block(full_w, kit::BUTTON_HEIGHT),
            block(full_w, kit::BUTTON_HEIGHT),
        ],
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ];
    for (variant, slot) in variants.into_iter().zip(button_row_rects) {
        let per = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        let mut buttons = Vec::with_capacity(4);
        for (i, state_label) in STATE_LABELS.iter().enumerate() {
            let cell = UiRect::new(
                control_x + i as f32 * (per + kit::GAP_CONTROLS),
                slot.y,
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
    }

    // --- IconButtons: 4 состояния ---
    let icon_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(icon_title.x, icon_title.y), SECTION_ICON));
    let icon_y = ruler.next().unwrap_or_default().y;
    for i in 0..4 {
        let cell = UiRect::new(
            control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
            icon_y,
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

    // --- Chips: 4 состояния (измеренные ширины) ---
    let chips_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(chips_title.x, chips_title.y), SECTION_CHIPS));
    let chips_y = ruler.next().unwrap_or_default().y;
    {
        let mut x = control_x;
        let max_w = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        for state_label in STATE_LABELS.iter() {
            let label = tr(lang, state_label);
            let cl = kit::chip_layout(
                UiPoint::new(x, chips_y),
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

    // --- Dropdown: якорь-кнопка + открытое меню (3 строки) ---
    let dropdown_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(dropdown_title.x, dropdown_title.y),
        SECTION_DROPDOWN,
    ));
    let dropdown_y = ruler.next().unwrap_or_default().y;
    let anchor_label = tr(lang, "kit.dropdown.anchor");
    let anchor_btn = kit::button_layout(
        UiRect::new(control_x, dropdown_y, 190.0, kit::BUTTON_HEIGHT),
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
    let dd = kit::dropdown_menu(dropdown_anchor, vp, UiVec2::new(190.0, dropdown_menu_h));
    for i in 0..3 {
        dropdown_items.push(UiRect::new(
            dd.menu.x + 4.0,
            dd.menu.y + 4.0 + i as f32 * (dropdown_item_h + 4.0),
            dd.menu.w - 8.0,
            dropdown_item_h,
        ));
    }
    let dropdown_menu = dd.menu;

    // --- Toast: строка внизу контента (kit Toast) ---
    let toast_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(toast_title.x, toast_title.y), SECTION_TOAST));
    let toast = UiRect::new(
        content.x,
        ruler.next().unwrap_or_default().y,
        content.w,
        toast_h,
    );

    // --- Tooltip: якорь-чип + пузырь (delay пройден) ---
    let tooltip_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(tooltip_title.x, tooltip_title.y),
        SECTION_TOOLTIP,
    ));
    let tooltip_y = ruler.next().unwrap_or_default().y;
    let anchor_label = tr(lang, "kit.tooltip.anchor");
    let cl = kit::chip_layout(
        UiPoint::new(control_x, tooltip_y),
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

    // === FR-059: секции компонентов v2 (FR-058) — после секций v1 ===

    // --- TextField: Normal / Focused / Disabled (3 поля в ряд) ---
    let text_field_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(text_field_title.x, text_field_title.y),
        SECTION_TEXT_FIELD,
    ));
    let text_field_y = ruler.next().unwrap_or_default().y;
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
                    max_chars: None,
                },
                "",
            ),
            (
                KitState::Normal,
                kit::TextFieldModel {
                    text: GALLERY_FIELD_FOCUSED_TEXT.to_owned(),
                    caret: GALLERY_FIELD_FOCUSED_TEXT.chars().count(),
                    sel: None,
                    max_chars: None,
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
                text_field_y,
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

    // --- Switch: Off/Normal, On/Normal, On/Hovered, Off/Disabled + Pressed,
    // On/в фокусе (рамка accent — рисует потребитель, контракт FR-057) ---
    let switch_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(switch_title.x, switch_title.y), SECTION_SWITCH));
    let switch_y = ruler.next().unwrap_or_default().y;
    let mut switches: Vec<(UiRect, bool, KitState, bool)> = Vec::new();
    {
        let demo: [(bool, KitState, bool); 6] = [
            (false, KitState::Normal, false),
            (true, KitState::Normal, false),
            (true, KitState::Hovered, false),
            (false, KitState::Disabled, false),
            (false, KitState::Pressed, false),
            (true, KitState::Normal, true),
        ];
        for (i, (on, state, focused)) in demo.into_iter().enumerate() {
            let slot = UiRect::new(
                control_x + i as f32 * (kit::SWITCH_W + kit::GAP_CONTROLS),
                switch_y,
                kit::SWITCH_W,
                kit::SWITCH_H,
            );
            switches.push((slot, on, state, focused));
        }
    }

    // --- Card: хедер + тело внутри пада панели (kit::card) ---
    let card_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(card_title.x, card_title.y), SECTION_CARD));
    let card_y = ruler.next().unwrap_or_default().y;
    let card_slot = UiRect::new(control_x, card_y, control_w, card_h);
    let card = kit::card(
        card_slot,
        UiVec2::new(0.0, card_h),
        UiVec2::new(600.0, card_h),
        20.0,
        p,
    );

    // --- Список + скролл: 8 строк в окне 3 (бегунок в треке) ---
    let list_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(list_title.x, list_title.y), SECTION_LIST));
    let list_area = UiRect::new(
        control_x,
        ruler.next().unwrap_or_default().y,
        control_w,
        GALLERY_LIST_VIEWPORT_H,
    );
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

    // --- Icon-глифы v2: ПОЛНЫЙ инвентарь Icon (8 вариантов — витрина
    // покрывает все записи enum; SVG-наборы — те же имена) ---
    let icons_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(icons_title.x, icons_title.y), SECTION_ICONS));
    let icons_y = ruler.next().unwrap_or_default().y;
    let mut icon_glyphs: Vec<(UiRect, kit::Icon)> = Vec::new();
    {
        let glyphs = [
            kit::Icon::Search,
            kit::Icon::ArrowLeft,
            kit::Icon::ArrowRight,
            kit::Icon::Refresh,
            kit::Icon::Close,
            kit::Icon::Gear,
            kit::Icon::Question,
            kit::Icon::Plus,
        ];
        for (i, icon) in glyphs.into_iter().enumerate() {
            let cell = UiRect::new(
                control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
                icons_y,
                kit::ICON_BUTTON_SIZE,
                kit::ICON_BUTTON_SIZE,
            );
            let rect = kit::icon_button_rect(
                cell,
                (
                    canvas_ui::layout::HAlign::Start,
                    canvas_ui::layout::VAlign::Center,
                ),
            );
            icon_glyphs.push((rect, icon));
        }
    }

    // === FR-068 W3 (этап M3): секция Table — табличные строки через
    // компонент Table (retained-вход кадра: set_rows + row_layout_with по
    // фиксированным слотам GALLERY_ROW_H). W3.3 (решение владельца):
    // витрина Row v1 (FR-061) заменена компонентом — табличная секция
    // одна, следует за Icons. Параметры: right_pad 0 (правый край
    // направляющих — край контрол-колонки; viewport_right — ПРАВЫЙ КРАЙ
    // В КООРДИНАТАХ СЛОТОВ, слоты здесь x=control_x), зазор
    // TABLE_GUIDE_GAP, лидер on (RowOpts::default), кегль LABEL_SIZE.
    // Зебра — переопределением слота fill (TableRowStyle {
    // fill: Some(hover_fill) }, F-8 — готовый слот, без арифметики),
    // Σ — state: Selected. Отрисовка — общий путь `row_rows` (paint_row
    // по предвычисленному RowLayout в app/overlays.rs) — ноль правок в
    // отрисовке; оракул бит-в-бит «Row ≡ Table на одних данных»
    // (kit::row_guides/row_layout против Table::row_layout_with) — в
    // тестах модуля.
    let table_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(table_title.x, table_title.y), SECTION_TABLE));
    let table_y = ruler.next().unwrap_or_default().y;
    let mut row_rows: Vec<RowDemoRow> = Vec::new();
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
                        // Зебра — слот hover_fill (тот же слот подставляется
                        // в row_style при отрисовке).
                        fill: zebra.then_some(p.hover_fill),
                        ..kit::TableRowStyle::default()
                    },
                })
                .collect(),
        );
        // Деградация §4.2 (guides None — правые колонки пусты у всех
        // строк): паритет Row-пути (row_guides None → строки не строятся).
        if table.guides_with(m, fs, viewport_right).is_some() {
            for (i, (row_parts, state, zebra)) in demo.into_iter().enumerate() {
                let slot = UiRect::new(
                    control_x,
                    table_y + i as f32 * GALLERY_ROW_H,
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

    // === FR-062: секции layout v2 (F-13…F-17) — после секций компонентов v2 ===

    // --- Measured-ряд (F-13): ширины чипов — TextMeasurer внутри
    // раскладки ([`Row::lay_out_measured`]); пад чипа — измеренные
    // пробелы вокруг подписи (рисуется исходная подпись по центру).
    let measured_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(measured_title.x, measured_title.y),
        SECTION_MEASURED,
    ));
    let measured_y = ruler.next().unwrap_or_default().y;
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
            UiRect::new(control_x, measured_y, control_w, kit::CHIP_HEIGHT),
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

    // --- Flex-факторы (F-14): fixed + grow ×2 + grow ×1 — свободное
    // место слота распределяется пропорционально grow.
    let grow_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(grow_title.x, grow_title.y), SECTION_GROW));
    let grow_y = ruler.next().unwrap_or_default().y;
    let grow_cells: Vec<(UiRect, &'static str)> = Row {
        gap: kit::GAP_CONTROLS,
        ..Row::default()
    }
    .lay_out_with(
        pilot_backend(),
        UiRect::new(control_x, grow_y, control_w, kit::BUTTON_HEIGHT),
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

    // --- Wrap (F-15): жадная упаковка measured-чипов в строки слота
    // (2 строки видимы; переполнение — за нижний край, видно линту).
    let wrap_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(wrap_title.x, wrap_title.y), SECTION_WRAP));
    let wrap_y = ruler.next().unwrap_or_default().y;
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
            UiRect::new(control_x, wrap_y, control_w, wrap_slot_h),
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

    // --- Сетка (F-16): 4 равные колонки × 2 строки ([`grid_cells_with`],
    // FR-068 W1 — через [`pilot_backend`]; T2-триггер ADR-0013 — Grid
    // с явными треками в TaffyBackend).
    let grid_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(grid_title.x, grid_title.y), SECTION_GRID));
    let grid_y = ruler.next().unwrap_or_default().y;
    let grid_col_w = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
    let grid_cells: Vec<UiRect> = grid_cells_with(
        pilot_backend(),
        UiRect::new(control_x, grid_y, control_w, grid_slot_h),
        &[grid_col_w; 4],
        2,
        grid_cell_h,
        UiVec2::new(kit::GAP_CONTROLS, kit::GAP_CONTROLS),
    );

    // --- Фокус (F-17): 4 слота фиксированной ширины; Tab-кольцо App
    // живёт в КОНТЕНТ-координатах ([`Self::focus_targets`]), рамка —
    // при отрисовке по совпадению с кольцом (слот accent).
    let focus_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(focus_title.x, focus_title.y), SECTION_FOCUS));
    let focus_y = ruler.next().unwrap_or_default().y;
    let focus_targets: Vec<UiRect> = (0..GALLERY_FOCUS_SLOTS)
        .map(|i| {
            UiRect::new(
                control_x + i as f32 * (GALLERY_FOCUS_W + kit::GAP_CONTROLS),
                focus_y,
                GALLERY_FOCUS_W,
                kit::BUTTON_HEIGHT,
            )
        })
        .collect();
    let focus_buttons: Vec<UiRect> = focus_targets.clone();

    // === Пересборка поверхностей (волна 2): демо компонентного слоя и
    // недостающих layout-примитивов — хвост колонки секций ===

    // --- Компонент Row (component::Row): Props + retained-компонент.
    // Отрисовка — Component::paint компонента в оверлее (Row::new поднимает
    // собственный FontSystem — как Table::new в fill_body FR-070). ---
    let comp_row_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(comp_row_title.x, comp_row_title.y),
        SECTION_COMPONENT_ROW,
    ));
    let comp_row_y = ruler.next().unwrap_or_default().y;
    let component_row_slot = UiRect::new(control_x, comp_row_y, control_w, GALLERY_ROW_H);
    let component_row_props = canvas_ui::component::row::RowProps {
        marker: kit::RowMarker::Dot,
        label: tr(lang, crate::i18n::keys::KIT_COMPONENT_ROW_LABEL),
        value: GALLERY_COMPONENT_ROW_VALUE.to_owned(),
        badge: None,
        opts: kit::RowOpts::default(),
        palette: *p,
    };

    // --- Компонент Panel (component::Panel): layout = [панель, контент]
    // (контракт порядка rects — panel.rs §Component); отрисовка хрома —
    // panel_style слотами в оверлее. ---
    let comp_panel_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(comp_panel_title.x, comp_panel_title.y),
        SECTION_COMPONENT_PANEL,
    ));
    let comp_panel_y = ruler.next().unwrap_or_default().y;
    let panel_comp =
        canvas_ui::component::panel::Panel::new(canvas_ui::component::panel::PanelProps {
            min: UiVec2::new(0.0, GALLERY_COMPONENT_PANEL_H),
            max: UiVec2::new(control_w, GALLERY_COMPONENT_PANEL_H),
            desired: UiVec2::new(control_w, GALLERY_COMPONENT_PANEL_H),
            palette: *p,
        });
    let panel_rects = panel_comp.layout(
        pilot_backend(),
        UiRect::new(
            control_x,
            comp_panel_y,
            control_w,
            GALLERY_COMPONENT_PANEL_H,
        ),
    );
    let component_panel = Some((panel_rects[0], panel_rects[1]));

    // --- SqueezeTail: именованная деградация узкого слота — хвост ряда
    // сжимается (до нуля), ничего не выходит за слот (переполнение видно
    // линту у Fit — здесь поглощается политикой). Боксы — MeasuredItem::Fixed
    // (именованное измеренное-эквивалентное spelling — правило W3 «0
    // Child::fixed у потребителей»). ---
    let squeeze_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(squeeze_title.x, squeeze_title.y),
        SECTION_SQUEEZE,
    ));
    let squeeze_slot = UiRect::new(
        control_x,
        ruler.next().unwrap_or_default().y,
        control_w,
        GALLERY_SQUEEZE_BOX_H,
    );
    let squeeze_cells: Vec<UiRect> = Row {
        gap: kit::GAP_CONTROLS,
        policy: RowPolicy::SqueezeTail,
        ..Row::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        squeeze_slot,
        &(0..GALLERY_SQUEEZE_BOXES)
            .map(|_| MeasuredItem::Fixed {
                w: GALLERY_SQUEEZE_BOX_W,
                h: GALLERY_SQUEEZE_BOX_H,
            })
            .collect::<Vec<MeasuredItem>>(),
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );

    // --- MainAlign::SpaceBetween (Row): свободное место слота — в зазоры
    // между measured-чипами; и Column с распоркой (Spacer): зазор между
    // 1-й и 2-й ячейкой = gap + Spacer. ---
    let align_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(align_title.x, align_title.y), SECTION_ALIGN));
    // Два блока секции (SpaceBetween-ряд + Column-демо) — вертикальный
    // Column линейки с зазором SPACING_S; нулевой хвост до Constrain —
    // исторический, пинен golden-тестом линейки.
    let mut align_blocks = Column {
        gap: canvas_core::tokens::SPACING_S,
        ..Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        UiRect::new(
            control_x,
            ruler.next().unwrap_or_default().y,
            control_w,
            kit::CHIP_HEIGHT + canvas_core::tokens::SPACING_S + align_column_block_h,
        ),
        &[
            block(control_w, kit::CHIP_HEIGHT),
            block(control_w, align_column_block_h),
        ],
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    )
    .into_iter();
    let align_between_y = align_blocks.next().unwrap_or_default().y;
    let align_column_y = align_blocks.next().unwrap_or_default().y;
    let align_between: Vec<(UiRect, &'static str)> = {
        let keys = [
            crate::i18n::keys::KIT_ALIGN_A,
            crate::i18n::keys::KIT_ALIGN_B,
            crate::i18n::keys::KIT_ALIGN_C,
        ];
        let labels: Vec<String> = keys.iter().map(|k| format!(" {} ", tr(lang, k))).collect();
        let items: Vec<MeasuredItem> = labels
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
            main: canvas_ui::layout::MainAlign::SpaceBetween,
            ..Row::default()
        }
        .lay_out_measured_with(
            pilot_backend(),
            UiRect::new(control_x, align_between_y, control_w, kit::CHIP_HEIGHT),
            &items,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        )
        .into_iter()
        .zip(keys)
        .collect()
    };
    // Column (вертикальный стек, Fit): 3 ячейки + явная распорка 12 px
    // (Fixed 0×12 — Child::spacer в Column не участвует: «распорка» кита
    // занимает главную ось РЯДА). Зазор между 2-й и 3-й ячейкой =
    // column-gap + распорка + column-gap. rect распорки — не рисуется.
    let column_cells: Vec<UiRect> = canvas_ui::layout::Column {
        // LAY7: 6.0 = SPACING_S (шкала S1). Литерал убран — гейт
        // scripts/lay7_lint.sh (LAY-W12) ловит inline-литералы gap.
        gap: canvas_core::tokens::SPACING_S,
        ..canvas_ui::layout::Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        UiRect::new(control_x, align_column_y, control_w, align_column_block_h),
        &[
            MeasuredItem::Fixed {
                w: GALLERY_COLUMN_CELL_W,
                h: GALLERY_COLUMN_CELL_H,
            },
            MeasuredItem::Fixed {
                w: GALLERY_COLUMN_CELL_W,
                h: GALLERY_COLUMN_CELL_H,
            },
            MeasuredItem::Fixed { w: 0.0, h: 12.0 },
            MeasuredItem::Fixed {
                w: GALLERY_COLUMN_CELL_W,
                h: GALLERY_COLUMN_CELL_H,
            },
        ],
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );

    // === LAY-SHOWCASE (design/rules/11-layouts.md, LAY11 п.10): витрина
    // раскладок — механизмы LAY2/LAY5/LAY7/LAY8, которых ещё не было в
    // секциях. Демо строятся ТОЛЬКО примитивами canvas_ui::layout и сценой
    // SceneNode (lay_out_scene — как html5_demos.rs); зазоры/паддинги —
    // SPACING_* (LAY7); геометрия — из результатов раскладки (LAY1.2);
    // hit-зон нет (декоративные демо — интерактив витрины только в шапке).

    // --- Constrain (LAY2): желаемое → min/max (min приоритетнее max).
    // Три блока: ниже минимума / внутри границ / выше максимума —
    // ширины блоков = результат `constrain(min, max, desired)`.
    let constrain_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(constrain_title.x, constrain_title.y),
        SECTION_LAYOUT_CONSTRAIN,
    ));
    let constrain_y = ruler.next().unwrap_or_default().y;
    let layout_constrain: Vec<(UiRect, f32)> = {
        let desired = [24.0, 80.0, 160.0];
        let blocks: Vec<Child> = desired
            .iter()
            .map(|&d| {
                let clamped = constrain(
                    UiVec2::new(GALLERY_CONSTRAIN_MIN, GALLERY_LAYOUT_BLOCK_H),
                    UiVec2::new(GALLERY_CONSTRAIN_MAX, GALLERY_LAYOUT_BLOCK_H),
                    UiVec2::new(d, GALLERY_LAYOUT_BLOCK_H),
                );
                Child::fixed(clamped.x, clamped.y)
            })
            .collect();
        Row {
            gap: kit::GAP_CONTROLS,
            ..Row::default()
        }
        .lay_out_with(
            pilot_backend(),
            UiRect::new(control_x, constrain_y, control_w, GALLERY_LAYOUT_BLOCK_H),
            &blocks,
        )
        .into_iter()
        .zip(desired)
        .collect()
    };

    // --- Pad (LAY2 + LAY7): внутренние поля контейнера — EdgeInsets
    // из шкалы S1 (S/SM/MD/LG): рамка-ячейка + внутренний rect после pad.
    let pad_title = ruler.next().unwrap_or_default();
    section_titles.push((UiPoint::new(pad_title.x, pad_title.y), SECTION_LAYOUT_PAD));
    let pad_y = ruler.next().unwrap_or_default().y;
    let layout_pad: Vec<(UiRect, UiRect, &'static str)> = {
        let scales: [(&'static str, f32); 4] = [
            ("S", canvas_core::tokens::SPACING_S),
            ("SM", canvas_core::tokens::SPACING_SM),
            ("MD", canvas_core::tokens::SPACING_MD),
            ("LG", canvas_core::tokens::SPACING_LG),
        ];
        Row {
            gap: kit::GAP_CONTROLS,
            ..Row::default()
        }
        .lay_out_with(
            pilot_backend(),
            UiRect::new(control_x, pad_y, control_w, GALLERY_LAYOUT_CELL_H),
            &[
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
            ],
        )
        .into_iter()
        .zip(scales)
        .map(|(cell, (name, g))| (cell, pad(cell, EdgeInsets::uniform(g)), name))
        .collect()
    };

    // --- Stack (LAY2): фиксированный блок в слоте — «модалка по центру»
    // (Center/Center) и «прижата к углу» (End/End); слоты — равные доли
    // ширины (grow 1:1).
    let stack_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(stack_title.x, stack_title.y),
        SECTION_LAYOUT_STACK,
    ));
    let stack_y = ruler.next().unwrap_or_default().y;
    let layout_stack: Vec<(UiRect, UiRect)> = {
        let aligns = [(HAlign::Center, VAlign::Center), (HAlign::End, VAlign::End)];
        Row {
            gap: kit::GAP_CONTROLS,
            ..Row::default()
        }
        .lay_out_with(
            pilot_backend(),
            UiRect::new(control_x, stack_y, control_w, GALLERY_LAYOUT_CELL_H),
            &[
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
                Child::flexible(0.0, GALLERY_LAYOUT_CELL_H, 1.0),
            ],
        )
        .into_iter()
        .zip(aligns)
        .map(|(cell, (h, v))| (cell, stack(cell, GALLERY_STACK_BLOCK, h, v)))
        .collect()
    };

    // --- Шкала зазоров S1 (LAY7): пары блоков с зазорами S/SM/MD/LG/XL —
    // зазор пары == значение ступени; подпись — MeasuredItem::Text
    // («ступень · значение», значение — токен шкалы, не литерал).
    let gaps_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(gaps_title.x, gaps_title.y),
        SECTION_LAYOUT_GAPS,
    ));
    // Ряды шкалы — вложенный Column линейки с шагом SPACING_SM.
    let gap_row_rects = Column {
        gap: canvas_core::tokens::SPACING_SM,
        ..Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        UiRect::new(
            control_x,
            ruler.next().unwrap_or_default().y,
            control_w,
            5.0 * GALLERY_GAP_BLOCK_H + 4.0 * canvas_core::tokens::SPACING_SM,
        ),
        &[
            block(control_w, GALLERY_GAP_BLOCK_H),
            block(control_w, GALLERY_GAP_BLOCK_H),
            block(control_w, GALLERY_GAP_BLOCK_H),
            block(control_w, GALLERY_GAP_BLOCK_H),
            block(control_w, GALLERY_GAP_BLOCK_H),
        ],
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let layout_gaps: Vec<GapDemoRow> = {
        let scales: [(&'static str, f32); 5] = [
            ("S", canvas_core::tokens::SPACING_S),
            ("SM", canvas_core::tokens::SPACING_SM),
            ("MD", canvas_core::tokens::SPACING_MD),
            ("LG", canvas_core::tokens::SPACING_LG),
            ("XL", canvas_core::tokens::SPACING_XL),
        ];
        let mut rows = Vec::with_capacity(scales.len());
        for (row, (name, g)) in gap_row_rects.into_iter().zip(scales) {
            // Подпись живёт до конца итерации — MeasuredItem заимствует из
            // неё (паттерн wrap-секции). Pad подписи — SM слева+справа
            // (2×SM — суммарный пад из ступеней шкалы, LAY7.1).
            let caption = format!("{name} · {}", g as i32);
            let cells = Row {
                gap: g,
                ..Row::default()
            }
            .lay_out_measured_with(
                pilot_backend(),
                UiRect::new(control_x, row.y, control_w, GALLERY_GAP_BLOCK_H),
                &[
                    MeasuredItem::Fixed {
                        w: GALLERY_GAP_BLOCK_W,
                        h: GALLERY_GAP_BLOCK_H,
                    },
                    MeasuredItem::Fixed {
                        w: GALLERY_GAP_BLOCK_W,
                        h: GALLERY_GAP_BLOCK_H,
                    },
                    MeasuredItem::Text {
                        text: &caption,
                        max_w: None,
                        min_w: 0.0,
                        pad_x: 2.0 * canvas_core::tokens::SPACING_SM,
                        h: Some(GALLERY_GAP_BLOCK_H),
                    },
                ],
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            let mut it = cells.into_iter();
            rows.push(GapDemoRow {
                blocks: vec![it.next().unwrap_or_default(), it.next().unwrap_or_default()],
                label: it.next().unwrap_or_default(),
                caption,
            });
        }
        rows
    };

    // --- Сцена: percent + Fill (LAY5): доли ширины — 20% / 30% / Fill
    // (остаток). LAY5.2: перед расширенными политиками проверяется маска
    // движка — код не молчит в возможностях, которых нет (демо пустое).
    let percent_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(percent_title.x, percent_title.y),
        SECTION_LAYOUT_PERCENT,
    ));
    let percent_y = ruler.next().unwrap_or_default().y;
    let layout_percent: Vec<UiRect> = {
        let feats = canvas_ui::layout::default_backend().features();
        if feats.contains(LayoutFeatures::PERCENT) && feats.contains(LayoutFeatures::FLEX_GROW) {
            let shares = [
                SceneDim::Percent(0.20),
                SceneDim::Percent(0.30),
                SceneDim::Fill,
            ];
            let scene = SceneNode::row(
                control_w,
                GALLERY_LAYOUT_BLOCK_H,
                kit::GAP_CONTROLS,
                shares
                    .iter()
                    .map(|&w| {
                        SceneNode::default().sized(SceneSize {
                            w,
                            h: SceneDim::Length(GALLERY_LAYOUT_BLOCK_H),
                        })
                    })
                    .collect(),
            );
            // Контракт lay_out_scene: [0] — корень (== слот); доли — дети
            // с индекса 1 (DFS pre-order).
            FlexLayoutEngine
                .lay_out_scene(
                    UiRect::new(control_x, percent_y, control_w, GALLERY_LAYOUT_BLOCK_H),
                    &scene,
                )
                .into_iter()
                .skip(1)
                .collect()
        } else {
            Vec::new()
        }
    };

    // --- Сцена: aspect-ratio (LAY5): превью-плитки 16:9 — ширина задана
    // (равные трети контрол-колонки), высота выводится движком из ratio.
    let aspect_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(aspect_title.x, aspect_title.y),
        SECTION_LAYOUT_ASPECT,
    ));
    let aspect_y = ruler.next().unwrap_or_default().y;
    let layout_aspect: Vec<UiRect> = {
        let feats = canvas_ui::layout::default_backend().features();
        if feats.contains(LayoutFeatures::ASPECT_RATIO) && aspect_tile_w > 0.0 {
            let scene = SceneNode::row(
                control_w,
                aspect_h,
                kit::GAP_CONTROLS,
                (0..3)
                    .map(|_| {
                        SceneNode::default()
                            .sized(SceneSize::fixed_w(aspect_tile_w))
                            .ratio(16.0 / 9.0)
                    })
                    .collect(),
            );
            // Контракт lay_out_scene: [0] — корень (== слот); плитки —
            // дети с индекса 1 (DFS pre-order).
            FlexLayoutEngine
                .lay_out_scene(
                    UiRect::new(control_x, aspect_y, control_w, aspect_h),
                    &scene,
                )
                .into_iter()
                .skip(1)
                .collect()
        } else {
            Vec::new()
        }
    };

    // --- Сцена: sticky-шапка (LAY5): окно-колонка (Hidden + offset) —
    // шапка Sticky{top: 0} прилипает к верху окна при демо-сдвиге
    // (post-processing сцены — формула demo_01_sticky_header_column);
    // строки контента за краем окна не рисуются (полная видимость).
    let sticky_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(sticky_title.x, sticky_title.y),
        SECTION_LAYOUT_STICKY,
    ));
    let sticky_y = ruler.next().unwrap_or_default().y;
    let layout_sticky: Option<StickyDemo> = {
        let feats = canvas_ui::layout::default_backend().features();
        let window = UiRect::new(control_x, sticky_y, control_w, sticky_window_h);
        if feats.contains(LayoutFeatures::STICKY) && control_w > 0.0 {
            let header =
                SceneNode::leaf(control_w, GALLERY_STICKY_ROW_H).at(ScenePosition::Sticky {
                    top: Some(0.0),
                    left: None,
                });
            let mut children = Vec::with_capacity(1 + GALLERY_STICKY_ROWS);
            children.push(header);
            children.extend(
                (0..GALLERY_STICKY_ROWS).map(|_| SceneNode::leaf(control_w, GALLERY_STICKY_ROW_H)),
            );
            let scene = SceneNode::column(
                control_w,
                sticky_window_h,
                canvas_core::tokens::SPACING_S,
                children,
            )
            .clipped()
            .scrolled(GALLERY_STICKY_DEMO_OFFSET);
            // Контракт lay_out_scene: [0] — корень-окно, [1] — шапка,
            // далее строки по порядку (DFS pre-order == порядок rect'ов).
            let rects = FlexLayoutEngine.lay_out_scene(window, &scene);
            let mut it = rects.into_iter();
            let root = it.next().unwrap_or_default();
            let sticky_header = it.next().unwrap_or_default();
            let rows: Vec<UiRect> = it
                .filter(|r| r.y >= root.y - 0.01 && r.bottom() <= root.bottom() + 0.01)
                .collect();
            Some(StickyDemo {
                window: root,
                header: sticky_header,
                rows,
            })
        } else {
            None
        }
    };
    // --- Деградация HideBelow (LAY8 п.3): панель с подписью минимума —
    // при окне ниже порога скрывается ЦЕЛИКОМ (не сжимается); порог —
    // канонический what-if (900×600), политика — kit API реестра.
    let hide_below_title = ruler.next().unwrap_or_default();
    section_titles.push((
        UiPoint::new(hide_below_title.x, hide_below_title.y),
        SECTION_LAYOUT_HIDE_BELOW,
    ));
    let hide_below_y = ruler.next().unwrap_or_default().y;
    let hide_below_hidden = canvas_ui::registry::DegradationPolicy::HideBelow {
        min_width: GALLERY_HIDE_BELOW_MIN.x,
        min_height: GALLERY_HIDE_BELOW_MIN.y,
    }
    .hidden_at(viewport[0], viewport[1]);
    let hide_below_slot = UiRect::new(control_x, hide_below_y, control_w, GALLERY_HIDE_BELOW_H);
    let hide_below_panel = if hide_below_hidden {
        None
    } else {
        Some(stack(
            hide_below_slot,
            GALLERY_HIDE_BELOW_PANEL,
            HAlign::Center,
            VAlign::Center,
        ))
    };
    let layout_hide_below = HideBelowDemo {
        slot: hide_below_slot,
        panel: hide_below_panel,
        hidden: hide_below_hidden,
    };
    // Полная высота колонки секций (для скролла) — сумма блоков и распорок
    // линейки (накопление Column тождественно прежнему курсору `y +=`;
    // скролл-контракт FR-059 сохранён, включая хвостовую распорку).
    let content_h = ruler_items
        .iter()
        .map(|item| match item {
            MeasuredItem::Fixed { h, .. } => *h,
            _ => 0.0,
        })
        .sum::<f32>()
        .max(0.0);

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
    let switches: Vec<(UiRect, bool, KitState, bool)> = switches
        .into_iter()
        .filter(|(r, _, _, _)| visible(r))
        .map(|(r, on, state, focused)| (UiRect::new(r.x, r.y - off, r.w, r.h), on, state, focused))
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
    // FR-068: табличная секция Table — сдвиг всех ячеек каждой строки + фильтр
    let row_rows: Vec<RowDemoRow> = row_rows
        .into_iter()
        .filter(|d| visible(&d.lay.row))
        .map(|d| RowDemoRow {
            lay: shift_row_lay(d.lay, off),
            ..d
        })
        .collect();
    // Пересборка поверхностей: секции компонентного слоя/layout-примитивов —
    // сдвиг + фильтр полной видимости (ноль — секция за краем окна)
    let component_row_slot = if visible(&component_row_slot) {
        UiRect::new(
            component_row_slot.x,
            component_row_slot.y - off,
            component_row_slot.w,
            component_row_slot.h,
        )
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let component_panel =
        component_panel
            .filter(|(prect, _)| visible(prect))
            .map(|(prect, crect)| {
                (
                    UiRect::new(prect.x, prect.y - off, prect.w, prect.h),
                    UiRect::new(crect.x, crect.y - off, crect.w, crect.h),
                )
            });
    let squeeze_slot = if visible(&squeeze_slot) {
        UiRect::new(
            squeeze_slot.x,
            squeeze_slot.y - off,
            squeeze_slot.w,
            squeeze_slot.h,
        )
    } else {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    };
    let squeeze_cells: Vec<UiRect> = squeeze_cells
        .into_iter()
        .filter(|r| visible(r))
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let align_between: Vec<(UiRect, &'static str)> = align_between
        .into_iter()
        .filter(|(r, _)| visible(r))
        .map(|(r, k)| (UiRect::new(r.x, r.y - off, r.w, r.h), k))
        .collect();
    let column_cells: Vec<UiRect> = column_cells
        .into_iter()
        .filter(|r| visible(r))
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    // LAY-SHOWCASE: секции витрины раскладок — сдвиг + фильтр полной
    // видимости (паттерн секций выше: геометрия одна — draw/hit не дублируется)
    let layout_constrain: Vec<(UiRect, f32)> = layout_constrain
        .into_iter()
        .filter(|(r, _)| visible(r))
        .map(|(r, d)| (UiRect::new(r.x, r.y - off, r.w, r.h), d))
        .collect();
    let layout_pad: Vec<(UiRect, UiRect, &'static str)> = layout_pad
        .into_iter()
        .filter(|(cell, _, _)| visible(cell))
        .map(|(cell, inner, name)| {
            (
                UiRect::new(cell.x, cell.y - off, cell.w, cell.h),
                UiRect::new(inner.x, inner.y - off, inner.w, inner.h),
                name,
            )
        })
        .collect();
    let layout_stack: Vec<(UiRect, UiRect)> = layout_stack
        .into_iter()
        .filter(|(slot, _)| visible(slot))
        .map(|(slot, block)| {
            (
                UiRect::new(slot.x, slot.y - off, slot.w, slot.h),
                UiRect::new(block.x, block.y - off, block.w, block.h),
            )
        })
        .collect();
    let layout_gaps: Vec<GapDemoRow> = layout_gaps
        .into_iter()
        .filter(|row| row.blocks.first().is_some_and(visible))
        .map(|row| GapDemoRow {
            blocks: row
                .blocks
                .into_iter()
                .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
                .collect(),
            label: UiRect::new(row.label.x, row.label.y - off, row.label.w, row.label.h),
            caption: row.caption,
        })
        .collect();
    let layout_percent: Vec<UiRect> = layout_percent
        .into_iter()
        .filter(visible)
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let layout_aspect: Vec<UiRect> = layout_aspect
        .into_iter()
        .filter(visible)
        .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
        .collect();
    let layout_sticky = layout_sticky
        .filter(|demo| visible(&demo.window))
        .map(|demo| StickyDemo {
            window: UiRect::new(
                demo.window.x,
                demo.window.y - off,
                demo.window.w,
                demo.window.h,
            ),
            header: UiRect::new(
                demo.header.x,
                demo.header.y - off,
                demo.header.w,
                demo.header.h,
            ),
            rows: demo
                .rows
                .into_iter()
                .map(|r| UiRect::new(r.x, r.y - off, r.w, r.h))
                .collect(),
        });
    let layout_hide_below = if visible(&layout_hide_below.slot) {
        HideBelowDemo {
            slot: UiRect::new(
                layout_hide_below.slot.x,
                layout_hide_below.slot.y - off,
                layout_hide_below.slot.w,
                layout_hide_below.slot.h,
            ),
            panel: layout_hide_below
                .panel
                .map(|p| UiRect::new(p.x, p.y - off, p.w, p.h)),
            hidden: layout_hide_below.hidden,
        }
    } else {
        HideBelowDemo {
            slot: UiRect::new(0.0, 0.0, 0.0, 0.0),
            panel: None,
            hidden: layout_hide_below.hidden,
        }
    };
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
        component_row_slot,
        component_row_props,
        component_panel,
        squeeze_slot,
        squeeze_cells,
        align_between,
        column_cells,
        layout_constrain,
        layout_pad,
        layout_stack,
        layout_gaps,
        layout_percent,
        layout_aspect,
        layout_sticky,
        layout_hide_below,
    }
}

/// Слот-раскладка интерактивных зон для hit-rect'ов реестра (без
/// измерителя — фиксированные слоты шапки; совпадает с полной раскладкой —
/// одна геометрия для ввода и отрисовки). Возврат: (кнопка темы, «✕»).
///
/// FR-070 (агент N): миграция на `kit::panel_header` — те же слоты, что у
/// [`gallery_layout`] (один вызов `kit::panel_header` даёт кнопки для обоих).
/// Палитра нужна только для separator-style (который hit-слотам не нужен),
/// но подпись сохранена для симметрии с `gallery_layout`.
pub fn gallery_hit_slots(viewport: [f32; 2], palette: &KitPalette) -> (UiRect, UiRect) {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let header_slot = UiRect::new(content.x, content.y, content.w, 30.0);
    let header_buttons = [
        kit::HeaderButton::Toggle {
            label_w: THEME_SLOT_W,
            id: "theme",
        },
        kit::HeaderButton::Icon {
            kind: kit::HeaderIconKind::Close,
            id: "close",
        },
    ];
    let (header_layout, _header_style) = kit::panel_header(header_slot, &header_buttons, palette);
    let mut close = UiRect::default();
    let mut theme = UiRect::default();
    for (r, id) in &header_layout.buttons {
        match *id {
            "close" => close = *r,
            "theme" => theme = *r,
            _ => {}
        }
    }
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
    // Ревизия 2026-10-02: таб_drag/tab_suggest добавлены в атлас (FR-073/079
    // не смаплили их) — фолбэк дополнен теми же глифами, что в SETTINGS_TABS.
    match name {
        "tab_general" => "◎",
        "tab_canvas" => "▦",
        "tab_snap" => "≡",
        "tab_drag" => "✥",
        "tab_edges" => "⇄",
        "tab_suggest" => "✦",
        "tab_appearance" => "◐",
        "more" => "•••",
        "chevron_down" => "▾",
        "chevron_right" => "▸",
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
            // FR-070 (волна W-d): новые слоты — для тестов раскладки витрины
            // не читаются (геометрия от них не зависит), но значения берём из
            // `KitPalette::dark()` (Agent B) — чтобы test fixture не расходился
            // с production-коридором (single source of truth — AGENTS.md §UI-кит).
            control_success: [0.30, 0.75, 0.55, 1.0],
            control_warning: [0.95, 0.65, 0.30, 1.0],
            stage_dim: [0.02, 0.02, 0.04, 0.6],
            scrollbar_thumb: [0.35, 0.38, 0.46, 0.7],
            rule_color: [0.30, 0.33, 0.40, 0.8],
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
        // Нижнее положение скролла — хвост витрины: LAY-SHOWCASE (витрина
        // раскладок, 11-layouts.md LAY11 п.10) — новый хвост колонки;
        // секции волны 2 (компонентный слой + layout-примитивы) — выше,
        // видны детерминированным сканом (окно секций ≈ 550 px, обе семьи
        // целиком не влезают вместе с новым хвостом).
        let bottom = kit::ScrollState {
            offset: lay0.content_h - lay0.sections_viewport.h,
            content_h: lay0.content_h,
            viewport_h: lay0.sections_viewport.h,
        };
        let lay1 = gallery_layout([1280.0, 800.0], Language::Ru, &bottom, &p, &mut m, &mut fs);
        assert!(
            lay1.layout_hide_below.slot.w > 0.0,
            "HideBelow (LAY-SHOWCASE) — хвост колонки"
        );
        assert!(lay1.button_rows.is_empty(), "секции v1 ушли вверх");
        // Секции волны 2 — выше нового хвоста: скан до их полной видимости.
        let lay_w2 = scan_offset(
            &|lay| {
                lay.component_row_slot.w > 0.0
                    && lay.component_panel.is_some()
                    && lay.squeeze_cells.len() == GALLERY_SQUEEZE_BOXES
                    && lay.align_between.len() == 3
                    && lay.column_cells.len() == 4
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
            lay0.content_h - lay0.sections_viewport.h,
        );
        assert!(
            lay_w2.component_row_slot.w > 0.0,
            "component::Row — над хвостом LAY-SHOWCASE"
        );
        assert!(
            lay_w2.component_panel.is_some(),
            "component::Panel — хвост v2"
        );
        assert_eq!(
            lay_w2.squeeze_cells.len(),
            GALLERY_SQUEEZE_BOXES,
            "SqueezeTail — хвост v2"
        );
        assert_eq!(lay_w2.align_between.len(), 3, "SpaceBetween — хвост v2");
        assert_eq!(lay_w2.column_cells.len(), 4, "Column — хвост v2");
        // Секции layout v2 (F-13…F-17) — середина колонки: один скан на
        // семейство (в одном окне секций видны вместе)
        let max_offset = lay0.content_h - lay0.sections_viewport.h;
        let lay_f = scan_offset(
            &|lay| {
                lay.measured_chips.len() == 3
                    && lay.grow_cells.len() == 3
                    && lay.wrap_chips.len() == GALLERY_WRAP_CHIPS
                    && lay.grid_cells.len() == 8
                    && lay.focus_buttons.len() == GALLERY_FOCUS_SLOTS
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
            max_offset,
        );
        assert_eq!(lay_f.measured_chips.len(), 3, "measured-ряд ×3 (F-13)");
        assert_eq!(lay_f.grow_cells.len(), 3, "flex-ряд ×3 (F-14)");
        assert_eq!(lay_f.wrap_chips.len(), GALLERY_WRAP_CHIPS, "wrap ×8 (F-15)");
        assert_eq!(lay_f.grid_cells.len(), 8, "сетка 4×2 (F-16)");
        assert_eq!(
            lay_f.focus_buttons.len(),
            GALLERY_FOCUS_SLOTS,
            "фокус ×4 (F-17)"
        );
        // Секции v2 — середина колонки: детерминированный скан смещения
        // (хвост витрины растёт — якоримся на факт видимости, не на
        // константу высот). FR-068 M3: колонка выросла (+ секция Table) —
        // блок TextField…Icons и табличная секция Table не влезают в окно
        // ОДНИМ смещением; каждое семейство сканируется отдельно.
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
            &|lay| lay.text_fields.len() == 3 && lay.icon_glyphs.len() == 8,
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
        assert_eq!(lay_v2.switches.len(), 6, "Switch ×6 (вкл ST1 + фокус)");
        assert!(
            lay_v2.switches.iter().any(|(_, _, _, focused)| *focused),
            "одно состояние — в фокусе"
        );
        assert!(lay_v2.card.is_some(), "Card построена");
        assert!(!lay_v2.list_rows.is_empty(), "строки списка видимы");
        assert!(
            lay_v2.list_scroll.needs_scroll(),
            "демо-список прокручивается"
        );
        assert_eq!(
            lay_v2.icon_glyphs.len(),
            8,
            "Icon-глифы ×8 (полный инвентарь Icon)"
        );
        // Табличная секция (FR-068 M3 Table; W3.3 — единственная) —
        // отдельный скан: 4 строки Table на одной направляющей чисел.
        let lay_rows = scan_offset(
            &|lay| lay.row_rows.len() == 4,
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
            max_offset,
        );
        assert_eq!(
            lay_rows.row_rows.len(),
            4,
            "Table ×4 (M3; W3.3 — секция Row удалена)"
        );
        let value_right = lay_rows.row_rows[0].lay.value.right();
        assert!(
            lay_rows
                .row_rows
                .iter()
                .all(|d| d.lay.value.w == 0.0 || (d.lay.value.right() - value_right).abs() < 0.01),
            "значения — на колоночной направляющей (D-4)"
        );
        // Строка с бейджем — пилюля на бейдж-колонке
        assert_eq!(
            lay_rows
                .row_rows
                .iter()
                .filter(|d| d.lay.badge.is_some())
                .count(),
            1,
            "бейдж-демо «← источник» — строка Table"
        );
        // Состояния/зебра демо Table
        assert!(
            lay_rows.row_rows[1].zebra,
            "вторая строка — зебра (hover_fill)"
        );
        assert_eq!(
            lay_rows.row_rows[3].state,
            KitState::Selected,
            "Σ — Selected"
        );
        // W3.3 (переписанный пин шага секций): Table следует за Icons —
        // фактический шаг SECTION_GAP (после контента Icons) + 18.0
        // (заголовок секции Table) — пин бит-в-бит; шаг строк —
        // GALLERY_ROW_H (row_gap 0).
        let last_glyph_bottom = lay_rows.icon_glyphs[3].0.bottom();
        let step = lay_rows.row_rows[0].lay.row.y - last_glyph_bottom;
        assert!(
            (step - (SECTION_GAP + 18.0)).abs() < 0.01,
            "Table-секция следует за Icons с шагом SECTION_GAP + 18 (заголовок): {} != {}",
            step,
            SECTION_GAP + 18.0
        );
        assert!(
            (lay_rows.row_rows[1].lay.row.y - lay_rows.row_rows[0].lay.row.y - GALLERY_ROW_H).abs()
                < 0.01,
            "шаг строк Table — GALLERY_ROW_H (row_gap 0)"
        );
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
    /// FR-068 W3 (этап M3, оракул бит-в-бит; W3.3 — паритет Row-пути после
    /// замены витрины Row компонентом): путь Table
    /// ([`kit::Table::row_layout_with`]) ≡ путь Row ([`kit::row_guides`] +
    /// [`kit::row_layout`]) на ОДНИХ демо-данных и параметрах — раскладки
    /// строк ([`kit::RowLayout`], включая `label_shown`) равны дословно:
    /// Table делегирует тем же kit-функциям.
    #[test]
    fn table_row_layouts_match_row_path() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let demo = gallery_row_demo(Language::Ru);
        let (control_x, control_w, y0) = (40.0, 300.0, 100.0);
        let viewport_right = control_x + control_w;

        // Путь Row (kit-функции, бывший путь витрины FR-061):
        // row_guides + row_layout per слот
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

        // Путь Table (компонент витрины): set_rows + row_layout_with (те же слоты)
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

    // === Пересборка поверхностей (волна 2): тесты новых секций ===

    /// Скан смещения скролла до первого состояния, где предикат истинен
    /// (хвост витрины растёт — детерминированный скан вместо констант
    /// высот; шаг 8 — как в scan_offset теста v2-секций).
    fn scan_to(
        want: &dyn Fn(&GalleryLayout) -> bool,
        p: &KitPalette,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        content_h: f32,
        viewport_h: f32,
    ) -> GalleryLayout {
        let mut off = 0.0f32;
        loop {
            let s = kit::ScrollState {
                offset: off,
                content_h,
                viewport_h,
            };
            let lay = gallery_layout([1280.0, 800.0], Language::Ru, &s, p, m, fs);
            if want(&lay) || off >= content_h - viewport_h {
                return lay;
            }
            off += 8.0;
        }
    }

    /// Хвост витрины (волна 2): демо component::Panel (панель + контент
    /// минус пад), SqueezeTail (хвост ряда сжат, ничего не выходит за
    /// слот), SpaceBetween (зазоры больше базового) и Column (стек с
    /// распоркой).
    #[test]
    fn gallery_layout_extended_sections() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let scroll = kit::ScrollState::default();
        let lay0 = gallery_layout([1280.0, 800.0], Language::Ru, &scroll, &p, &mut m, &mut fs);
        let lay = scan_to(
            &|lay| {
                lay.component_panel.is_some()
                    && lay.column_cells.len() == 4
                    && lay.align_between.len() == 3
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );

        // Компонент Panel: layout = [панель, контент]; контент — минус пад
        // panel_style (SPACING_LG), оба внутри слота.
        let (prect, crect) = lay.component_panel.expect("демо component::Panel видимо");
        assert!((prect.h - GALLERY_COMPONENT_PANEL_H).abs() < 0.01);
        let pad = canvas_core::tokens::SPACING_LG;
        assert!((crect.x - (prect.x + pad)).abs() < 0.01);
        assert!((crect.w - (prect.w - 2.0 * pad)).abs() < 0.01);

        // component::Row: слот живой, Props построены (компонент собирается
        // из них в оверлее — см. component_row_demo_layout_and_paint).
        assert!(lay.component_row_slot.w > 0.0);
        assert_eq!(lay.component_row_props.value, GALLERY_COMPONENT_ROW_VALUE);

        // SqueezeTail: боксы не выходят за слот, хвост сжат (деградация
        // видна: база 72 → хвост меньше первого бокса).
        assert_eq!(lay.squeeze_cells.len(), GALLERY_SQUEEZE_BOXES);
        assert!(lay
            .squeeze_cells
            .iter()
            .all(|r| r.right() <= lay.squeeze_slot.right() + 0.01));
        let first = lay.squeeze_cells[0].w;
        let last = lay.squeeze_cells[GALLERY_SQUEEZE_BOXES - 1].w;
        assert!(last < first, "хвост сжат политикой: {last} < {first}");

        // SpaceBetween: зазор между чипами больше базового GAP_CONTROLS.
        for pair in lay.align_between.windows(2) {
            let gap = pair[1].0.x - pair[0].0.right();
            assert!(
                gap > kit::GAP_CONTROLS + 0.5,
                "свободное место распределено в зазоры: {gap}"
            );
        }

        // Column: 3 ячейки + явная распорка 12 px — вертикальный стек; зазор
        // между 2-й и 3-й ячейкой = column-gap + распорка + column-gap.
        assert_eq!(lay.column_cells.len(), 4, "3 ячейки + распорка");
        assert!(lay.column_cells[1].h > 0.0);
        // Распорка — 12 px по высоте, нулевая по ширине (не рисуется).
        assert!(lay.column_cells[2].w.abs() < 0.01, "распорка — нулевая");
        assert!(
            (lay.column_cells[2].h - 12.0).abs() < 0.01,
            "распорка 12 px"
        );
        let gap_mid = lay.column_cells[3].y - lay.column_cells[1].bottom();
        assert!(
            (gap_mid - (6.0 + 12.0 + 6.0)).abs() < 0.01,
            "gap + распорка + gap: {gap_mid}"
        );
    }

    /// Икон-глифы витрины покрывают ВСЕ варианты `kit::Icon` (полный
    /// инвентарь enum — новые варианты не проходят мимо витрины).
    #[test]
    fn gallery_icon_glyphs_cover_all_icon_variants() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let scroll = kit::ScrollState::default();
        let lay0 = gallery_layout([1280.0, 800.0], Language::Ru, &scroll, &p, &mut m, &mut fs);
        let lay = scan_to(
            &|lay| lay.icon_glyphs.len() == 8,
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );
        let all = [
            kit::Icon::Close,
            kit::Icon::Gear,
            kit::Icon::Question,
            kit::Icon::Search,
            kit::Icon::Plus,
            kit::Icon::ArrowLeft,
            kit::Icon::ArrowRight,
            kit::Icon::Refresh,
        ];
        for icon in all {
            assert!(
                lay.icon_glyphs.iter().any(|(_, i)| *i == icon),
                "вариант {icon:?} не показан в витрине"
            );
        }
    }

    /// Демо component::Row: Component::layout от слота витрины даёт
    /// [строку, точку, текст, значение] (row_rects), paint — непустые
    /// items (строка рисуется компонентом, не дублируется потребителем).
    #[test]
    fn component_row_demo_layout_and_paint() {
        use canvas_ui::layout::default_backend;
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let scroll = kit::ScrollState::default();
        let lay0 = gallery_layout([1280.0, 800.0], Language::Ru, &scroll, &p, &mut m, &mut fs);
        let lay = scan_to(
            &|lay| {
                lay.component_row_slot.w > 0.0
                    && lay.component_row_slot.y >= lay0.sections_viewport.y
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );
        let row = canvas_ui::component::row::Row::new(lay.component_row_props.clone());
        let rects = row.layout(default_backend(), lay.component_row_slot);
        assert_eq!(rects[0], lay.component_row_slot, "rects[0] — слот строки");
        assert!(rects.len() >= 4, "строка + точка + текст + значение");
        let mut painter = canvas_ui::paint::Painter::new();
        row.paint(&mut painter, &rects);
        assert!(!painter.items().is_empty(), "компонент рисует строку");
        // hit_test дефолтный: точка внутри слота строки — индекс 0.
        let point = canvas_ui::geometry::UiPoint::new(
            lay.component_row_slot.x + 1.0,
            lay.component_row_slot.y + 1.0,
        );
        assert_eq!(
            row.hit_test(&rects, point),
            Some(canvas_ui::component::ComponentHit { index: 0 })
        );
    }

    /// Новые ключи i18n волны 2 существуют в ОБОИХ языках (tr не отдаёт
    /// сам ключ — тест полноты таблиц i18n не ловит забытый вызов).
    #[test]
    fn extended_gallery_i18n_keys_exist() {
        let keys = [
            crate::i18n::keys::KIT_SECTION_COMPONENT_ROW,
            crate::i18n::keys::KIT_SECTION_COMPONENT_PANEL,
            crate::i18n::keys::KIT_SECTION_SQUEEZE,
            crate::i18n::keys::KIT_SECTION_ALIGN,
            crate::i18n::keys::KIT_COMPONENT_ROW_LABEL,
            crate::i18n::keys::KIT_COMPONENT_PANEL_BODY,
            crate::i18n::keys::KIT_ALIGN_A,
            crate::i18n::keys::KIT_ALIGN_B,
            crate::i18n::keys::KIT_ALIGN_C,
            // LAY-SHOWCASE: витрина раскладок (11-layouts.md, LAY11 п.10)
            crate::i18n::keys::KIT_SECTION_LAYOUT_CONSTRAIN,
            crate::i18n::keys::KIT_SECTION_LAYOUT_PAD,
            crate::i18n::keys::KIT_SECTION_LAYOUT_STACK,
            crate::i18n::keys::KIT_SECTION_LAYOUT_GAPS,
            crate::i18n::keys::KIT_SECTION_LAYOUT_PERCENT,
            crate::i18n::keys::KIT_SECTION_LAYOUT_ASPECT,
            crate::i18n::keys::KIT_SECTION_LAYOUT_STICKY,
            crate::i18n::keys::KIT_SECTION_LAYOUT_HIDE_BELOW,
            crate::i18n::keys::KIT_LAYOUT_STACK_CENTER,
            crate::i18n::keys::KIT_LAYOUT_STACK_END,
            crate::i18n::keys::KIT_LAYOUT_STICKY_HEADER,
            crate::i18n::keys::KIT_LAYOUT_HIDE_BELOW_PANEL,
            crate::i18n::keys::KIT_LAYOUT_HIDE_BELOW_HIDDEN,
        ];
        for lang in [Language::Ru, Language::En] {
            for key in keys {
                assert_ne!(
                    crate::i18n::tr(lang, key),
                    key,
                    "нет перевода {key} для {lang:?}"
                );
            }
        }
    }

    /// Значение ступени шкалы S1 по имени (тестовая карта имён демо —
    /// значения берутся из токенов, не литералов).
    fn scale_value(name: &str) -> f32 {
        match name {
            "S" => canvas_core::tokens::SPACING_S,
            "SM" => canvas_core::tokens::SPACING_SM,
            "MD" => canvas_core::tokens::SPACING_MD,
            "LG" => canvas_core::tokens::SPACING_LG,
            "XL" => canvas_core::tokens::SPACING_XL,
            _ => panic!("неизвестная ступень шкалы S1: {name}"),
        }
    }

    /// LAY-SHOWCASE (design/rules/11-layouts.md, LAY11 п.10): витрина
    /// раскладок — секции механизмов LAY2/LAY7 (constrain/pad/stack/gaps),
    /// LAY5 (сцена percent/Fill, aspect, sticky) и LAY8 (HideBelow).
    /// Геометрия — из результатов раскладки; сцены — с проверкой маски
    /// движка (LAY5.2); hit-зон у секций нет (декоративные демо).
    #[test]
    fn gallery_layout_showcase_sections() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let lay0 = gallery_layout(
            [1280.0, 800.0],
            Language::Ru,
            &kit::ScrollState::default(),
            &p,
            &mut m,
            &mut fs,
        );

        // === Примитивы: constrain / pad / stack (LAY2) + gaps (LAY7) ===
        let lay_prim = scan_to(
            &|lay| {
                lay.layout_constrain.len() == 3
                    && lay.layout_pad.len() == 4
                    && lay.layout_stack.len() == 2
                    && lay.layout_gaps.len() == 5
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );
        // Constrain: desired [24, 80, 160] → ширины [min, 80, max]
        // (min приоритетнее max — контракт constrain).
        let desired: Vec<f32> = lay_prim.layout_constrain.iter().map(|(_, d)| *d).collect();
        assert_eq!(desired, vec![24.0, 80.0, 160.0], "желаемые ширины демо");
        let widths: Vec<f32> = lay_prim.layout_constrain.iter().map(|(r, _)| r.w).collect();
        assert!(
            (widths[0] - GALLERY_CONSTRAIN_MIN).abs() < 0.01
                && (widths[1] - 80.0).abs() < 0.01
                && (widths[2] - GALLERY_CONSTRAIN_MAX).abs() < 0.01,
            "кламп в min/max: {widths:?}"
        );
        // Блоки — в одном ряду без наложений (шаг = ширина + зазор).
        for pair in lay_prim.layout_constrain.windows(2) {
            let gap = pair[1].0.x - pair[0].0.right();
            assert!((gap - kit::GAP_CONTROLS).abs() < 0.01, "зазор ряда: {gap}");
        }
        // Pad: внутренний rect = ячейка минус EdgeInsets ступени (обе оси).
        for (cell, inner, name) in &lay_prim.layout_pad {
            let g = scale_value(name);
            assert!((inner.w - (cell.w - 2.0 * g)).abs() < 0.01);
            assert!((inner.h - (cell.h - 2.0 * g)).abs() < 0.01);
            assert!(inner.w > 0.0 && inner.h > 0.0, "пад не съел ячейку");
        }
        // Stack: [0] — блок по центру слота, [1] — прижат к правому нижнему
        // углу (End/End).
        let (slot0, block0) = &lay_prim.layout_stack[0];
        assert!(
            (block0.x - (slot0.x + (slot0.w - block0.w) / 2.0)).abs() < 0.01
                && (block0.y - (slot0.y + (slot0.h - block0.h) / 2.0)).abs() < 0.01,
            "stack Center/Center"
        );
        let (slot1, block1) = &lay_prim.layout_stack[1];
        assert!(
            (slot1.right() - block1.right()).abs() < 0.01
                && (slot1.bottom() - block1.bottom()).abs() < 0.01,
            "stack End/End"
        );
        // Gaps: зазор пары == значение ступени И зазор перед подписью тот же
        // (Row gap един для всех детей); подписи — измеренные (LAY6).
        for (i, row) in lay_prim.layout_gaps.iter().enumerate() {
            assert_eq!(row.blocks.len(), 2);
            let g = scale_value(&row.caption[..row.caption.find(' ').unwrap_or(0)]);
            let pair_gap = row.blocks[1].x - row.blocks[0].right();
            assert!(
                (pair_gap - g).abs() < 0.01,
                "ступень {i}: зазор {pair_gap} != {g}"
            );
            let label_gap = row.label.x - row.blocks[1].right();
            assert!(
                (label_gap - g).abs() < 0.01,
                "зазор до подписи: {label_gap}"
            );
            assert!(!row.caption.is_empty(), "подпись ступени");
        }

        // === Сцена: percent/Fill, aspect, sticky (LAY5) ===
        // LAY5.2: маска движка заявляет сцена-возможности демо.
        let feats = canvas_ui::layout::default_backend().features();
        assert!(
            feats.contains(LayoutFeatures::PERCENT)
                && feats.contains(LayoutFeatures::ASPECT_RATIO)
                && feats.contains(LayoutFeatures::STICKY),
            "FlexLayoutEngine заявляет percent/aspect/sticky"
        );
        let lay_scene = scan_to(
            &|lay| {
                lay.layout_percent.len() == 3
                    && lay.layout_aspect.len() == 3
                    && lay.layout_sticky.is_some()
            },
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );
        // Percent: 20% / 30% / Fill(остаток); сумма + зазоры == ширина слота.
        let slot_w = lay_scene
            .layout_sticky
            .as_ref()
            .map(|s| s.window.w)
            .unwrap_or(0.0);
        let pw: Vec<f32> = lay_scene.layout_percent.iter().map(|r| r.w).collect();
        assert!((pw[0] - 0.20 * slot_w).abs() < 0.6, "20%: {}", pw[0]);
        assert!((pw[1] - 0.30 * slot_w).abs() < 0.6, "30%: {}", pw[1]);
        let fill_w = pw[2];
        assert!(
            (pw.iter().sum::<f32>() + 2.0 * kit::GAP_CONTROLS - slot_w).abs() < 1.0,
            "Fill — остаток ширины: {pw:?} против {slot_w}"
        );
        assert!(fill_w > pw[1], "Fill шире 30%-трека: {fill_w}");
        // Aspect: высота выводится из ratio 16:9 (rounding ≤ 0.5 px — LAY9.3).
        for tile in &lay_scene.layout_aspect {
            assert!(
                (tile.h - tile.w * 9.0 / 16.0).abs() < 0.6,
                "16:9: {}×{}",
                tile.w,
                tile.h
            );
        }
        // Sticky: шапка прилипла к верху окна; 3 строки целиком видны,
        // шаг строк = row_h + зазор S; контент прокручен (offset демо).
        let sticky = lay_scene.layout_sticky.as_ref().expect("sticky-демо");
        assert!(
            (sticky.header.y - sticky.window.y).abs() < 0.01,
            "шапка у top"
        );
        assert_eq!(sticky.rows.len(), GALLERY_STICKY_VISIBLE_ROWS);
        for r in &sticky.rows {
            assert!(r.y >= sticky.window.y - 0.01 && r.bottom() <= sticky.window.bottom() + 0.01);
        }
        let step = sticky.rows[1].y - sticky.rows[0].y;
        assert!(
            (step - (GALLERY_STICKY_ROW_H + canvas_core::tokens::SPACING_S)).abs() < 0.01,
            "шаг строк: {step}"
        );
        let first_rel = sticky.rows[0].y - sticky.window.y;
        let expected_rel = GALLERY_STICKY_ROW_H
            + canvas_core::tokens::SPACING_S
            + (GALLERY_STICKY_ROW_H + canvas_core::tokens::SPACING_S)
            - GALLERY_STICKY_DEMO_OFFSET;
        assert!(
            (first_rel - expected_rel).abs() < 0.01,
            "первая видимая строка после сдвига: {first_rel} != {expected_rel}"
        );

        // === Деградация: HideBelow (LAY8 п.3) ===
        let lay_hide = scan_to(
            &|lay| lay.layout_hide_below.slot.w > 0.0,
            &p,
            &mut m,
            &mut fs,
            lay0.content_h,
            lay0.sections_viewport.h,
        );
        assert!(!lay_hide.layout_hide_below.hidden, "1280×800 ≥ 900×600");
        let panel = lay_hide.layout_hide_below.panel.expect("панель видна");
        assert!((panel.w - GALLERY_HIDE_BELOW_PANEL.x).abs() < 0.01);
        // Панель — stack по центру слота.
        let slot = &lay_hide.layout_hide_below.slot;
        assert!(
            (panel.x - (slot.x + (slot.w - panel.w) / 2.0)).abs() < 0.01,
            "панель по центру слота"
        );
        // Окно ниже порога — панель скрыта ЦЕЛИКОМ (флаг от вьюпорта,
        // не от скролла витрины: lay0 — offset 0).
        let lay_small = gallery_layout(
            [800.0, 560.0],
            Language::Ru,
            &kit::ScrollState::default(),
            &p,
            &mut m,
            &mut fs,
        );
        assert!(
            lay_small.layout_hide_below.hidden,
            "800×560 < 900×600 — порог сработал"
        );
        assert!(
            lay_small.layout_hide_below.panel.is_none(),
            "панель скрыта целиком (нет промежуточных ступеней)"
        );
    }

    /// LAY-W3b: золотая геометрия вертикальной линейки секций витрины —
    /// инвариант рефакторинга «ручной курсор `y +=` → Column-скелет»
    /// (аудит layouts-2026-10 §3.2/§3.10, §5 LAY-W3). Y-координаты подписей
    /// секций и якорей контента сняты с ПРЕЖНЕГО кода пробом (скан смещения
    /// шагом 8 px, окно 1280×800, RU — как `scan_to`) и пинены с допуском
    /// 0.005 px (< 0.01 px цели задачи). Исторические особенности линейки
    /// зафиксированы как есть: хвост +4 px после Buttons, нулевые зазоры
    /// после Focus (F-17) и Align (SpaceBetween+Column), шаг рядов Buttons
    /// SPACING_SM, рядов шкалы Gaps — SPACING_SM.
    #[test]
    fn gallery_layout_ruler_golden_column_skeleton() {
        const TOL: f32 = 0.005;
        let expected: &[(&str, f32)] = &[
            ("content_h", 2624.0),
            ("TITLE kit.section.buttons", 134.0),
            ("TITLE kit.section.icon_buttons", 312.0),
            ("TITLE kit.section.chips", 368.0),
            ("TITLE kit.section.dropdown", 422.0),
            ("TITLE kit.section.toast", 572.0),
            ("TITLE kit.section.tooltip", 622.0),
            ("TITLE kit.section.text_field", 688.0),
            ("TITLE kit.section.switch", 692.0),
            ("TITLE kit.section.card", 686.0),
            ("TITLE kit.section.list", 692.0),
            ("TITLE kit.section.icons", 692.0),
            ("TITLE kit.section.table", 692.0),
            ("TITLE kit.section.measured", 690.0),
            ("TITLE kit.section.grow", 688.0),
            ("TITLE kit.section.wrap", 692.0),
            ("TITLE kit.section.grid", 690.0),
            ("TITLE kit.section.focus", 688.0),
            ("TITLE kit.section.component_row", 688.0),
            ("TITLE kit.section.component_panel", 686.0),
            ("TITLE kit.section.squeeze", 692.0),
            ("TITLE kit.section.align", 686.0),
            ("TITLE kit.section.layout_constrain", 688.0),
            ("TITLE kit.section.layout_pad", 686.0),
            ("TITLE kit.section.layout_stack", 692.0),
            ("TITLE kit.section.layout_gaps", 690.0),
            ("TITLE kit.section.layout_percent", 692.0),
            ("TITLE kit.section.layout_aspect", 690.0),
            ("TITLE kit.section.layout_sticky", 688.0),
            ("TITLE kit.section.layout_hide_below", 688.0),
            ("ANCHOR buttons.first", 152.0),
            ("ANCHOR buttons.last", 266.0),
            ("ANCHOR icon", 330.0),
            ("ANCHOR chips", 386.0),
            ("ANCHOR dropdown.anchor", 440.0),
            ("ANCHOR dropdown.menu", 474.0),
            ("ANCHOR dropdown.item", 478.0),
            ("ANCHOR toast", 590.0),
            ("ANCHOR tooltip.anchor", 640.0),
            ("ANCHOR tooltip.bubble", 662.0),
            ("ANCHOR textfield", 674.0),
            ("ANCHOR switch", 686.0),
            ("ANCHOR card", 640.0),
            ("ANCHOR list.area", 614.0),
            ("ANCHOR icons.glyph", 678.0),
            ("ANCHOR table.row", 678.0),
            ("ANCHOR measured.chip", 684.0),
            ("ANCHOR grow.cell", 674.0),
            ("ANCHOR wrap.chip", 686.0),
            ("ANCHOR grid.cell", 676.0),
            ("ANCHOR focus.button", 674.0),
            ("ANCHOR component.row", 682.0),
            ("ANCHOR component.panel", 616.0),
            ("ANCHOR squeeze.slot", 686.0),
            ("ANCHOR align.between", 688.0),
            ("ANCHOR align.column", 686.0),
            ("ANCHOR constrain", 682.0),
            ("ANCHOR pad", 648.0),
            ("ANCHOR stack", 646.0),
            ("ANCHOR gaps.first", 684.0),
            ("ANCHOR gaps.last", 684.0),
            ("ANCHOR percent", 678.0),
            ("ANCHOR aspect", 564.0),
            ("ANCHOR sticky.window", 594.0),
            ("ANCHOR hidebelow.slot", 650.0),
        ];
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = gallery_palette();
        let lay0 = gallery_layout(
            [1280.0, 800.0],
            Language::Ru,
            &kit::ScrollState::default(),
            &p,
            &mut m,
            &mut fs,
        );
        let content_h = lay0.content_h;
        let viewport_h = lay0.sections_viewport.h;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut check = |tag: &str, val: f32, off: f32| {
            if seen.insert(tag.to_owned()) {
                let (_, want) = expected
                    .iter()
                    .find(|(t, _)| t == &tag)
                    .unwrap_or_else(|| panic!("золотой тег без ожидаемого значения: {tag}"));
                assert!(
                    (val - want).abs() < TOL,
                    "дрейф линейки {tag} (off={off}): {val} != {want}"
                );
            }
        };
        // Полная высота колонки (скролл-контракт) — сверяется первым.
        check("content_h", content_h, 0.0);
        let mut off = 0.0f32;
        while off <= content_h - viewport_h {
            let s = kit::ScrollState {
                offset: off,
                content_h,
                viewport_h,
            };
            let lay = gallery_layout([1280.0, 800.0], Language::Ru, &s, &p, &mut m, &mut fs);
            for (origin, key) in &lay.section_titles {
                check(&format!("TITLE {key}"), origin.y, off);
            }
            let mut pin = |tag: &str, val: f32, off: f32| check(tag, val, off);
            if let Some(r) = lay.button_rows.first() {
                pin("ANCHOR buttons.first", r.slot.y, off);
            }
            if let Some(r) = lay.button_rows.last() {
                pin("ANCHOR buttons.last", r.slot.y, off);
            }
            if let Some(r) = lay.icon_buttons.first() {
                pin("ANCHOR icon", r.y, off);
            }
            if let Some(r) = lay.chips.first() {
                pin("ANCHOR chips", r.y, off);
            }
            if lay.dropdown_anchor.w > 0.0 {
                pin("ANCHOR dropdown.anchor", lay.dropdown_anchor.y, off);
            }
            if lay.dropdown_menu.w > 0.0 {
                pin("ANCHOR dropdown.menu", lay.dropdown_menu.y, off);
            }
            if let Some(r) = lay.dropdown_items.first() {
                pin("ANCHOR dropdown.item", r.y, off);
            }
            if lay.toast.w > 0.0 {
                pin("ANCHOR toast", lay.toast.y, off);
            }
            if lay.tooltip_anchor.w > 0.0 {
                pin("ANCHOR tooltip.anchor", lay.tooltip_anchor.y, off);
            }
            if !lay.tooltip.is_empty() {
                pin("ANCHOR tooltip.bubble", lay.tooltip.y, off);
            }
            if let Some((_, _, f)) = lay.text_fields.first() {
                pin("ANCHOR textfield", f.rect.y, off);
            }
            if let Some((r, _, _, _)) = lay.switches.first() {
                pin("ANCHOR switch", r.y, off);
            }
            if let Some(c) = &lay.card {
                pin("ANCHOR card", c.rect.y, off);
            }
            if lay.list_area.w > 0.0 {
                pin("ANCHOR list.area", lay.list_area.y, off);
            }
            if let Some((r, _)) = lay.icon_glyphs.first() {
                pin("ANCHOR icons.glyph", r.y, off);
            }
            if let Some(r) = lay.row_rows.first() {
                pin("ANCHOR table.row", r.lay.row.y, off);
            }
            if let Some(r) = lay.measured_chips.first() {
                pin("ANCHOR measured.chip", r.y, off);
            }
            if let Some((r, _)) = lay.grow_cells.first() {
                pin("ANCHOR grow.cell", r.y, off);
            }
            if let Some((r, _)) = lay.wrap_chips.first() {
                pin("ANCHOR wrap.chip", r.y, off);
            }
            if let Some(r) = lay.grid_cells.first() {
                pin("ANCHOR grid.cell", r.y, off);
            }
            if let Some(r) = lay.focus_buttons.first() {
                pin("ANCHOR focus.button", r.y, off);
            }
            if lay.component_row_slot.w > 0.0 {
                pin("ANCHOR component.row", lay.component_row_slot.y, off);
            }
            if let Some((panel, _)) = &lay.component_panel {
                pin("ANCHOR component.panel", panel.y, off);
            }
            if lay.squeeze_slot.w > 0.0 {
                pin("ANCHOR squeeze.slot", lay.squeeze_slot.y, off);
            }
            if let Some((r, _)) = lay.align_between.first() {
                pin("ANCHOR align.between", r.y, off);
            }
            if let Some(r) = lay.column_cells.first() {
                pin("ANCHOR align.column", r.y, off);
            }
            if let Some((r, _)) = lay.layout_constrain.first() {
                pin("ANCHOR constrain", r.y, off);
            }
            if let Some((r, _, _)) = lay.layout_pad.first() {
                pin("ANCHOR pad", r.y, off);
            }
            if let Some((r, _)) = lay.layout_stack.first() {
                pin("ANCHOR stack", r.y, off);
            }
            if let Some(g) = lay.layout_gaps.first() {
                if let Some(b) = g.blocks.first() {
                    pin("ANCHOR gaps.first", b.y, off);
                }
            }
            if lay.layout_gaps.len() >= 5 {
                if let Some(b) = lay.layout_gaps[4].blocks.first() {
                    pin("ANCHOR gaps.last", b.y, off);
                }
            }
            if let Some(r) = lay.layout_percent.first() {
                pin("ANCHOR percent", r.y, off);
            }
            if let Some(r) = lay.layout_aspect.first() {
                pin("ANCHOR aspect", r.y, off);
            }
            if let Some(st) = &lay.layout_sticky {
                pin("ANCHOR sticky.window", st.window.y, off);
            }
            if lay.layout_hide_below.slot.w > 0.0 {
                pin("ANCHOR hidebelow.slot", lay.layout_hide_below.slot.y, off);
            }
            off += 8.0;
        }
        // Полнота: каждый золотой тег снят сканом (секция не исчезла).
        for (tag, _) in expected {
            assert!(
                seen.iter().any(|t| t == tag),
                "золотой тег не снят сканом: {tag}"
            );
        }
    }
}
