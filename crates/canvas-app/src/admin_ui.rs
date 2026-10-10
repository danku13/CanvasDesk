//! FR-070: UI-админпанель (консоль дизайн-системы) — модель раскладки +
//! адаптер отрисовки. Паттерн — витрина кита ([`crate::kit_ui`], FR-055):
//! чистая функция раскладки от вьюпорта + [`KitDraw`]-адаптер; сборка кадра —
//! метод `App::admin_panel_overlay` (app.rs), интеграция — реестр
//! поверхностей (Block-модаль, Esc/«✕»/backdrop закрывают).
//!
//! Состав (решения владельца, опрос 2026-09-25): сайдбар секций (паттерн
//! Storybook) + демо-зона справа; секции: «Компоненты» (полная матрица
//! состояний ST1 + Focused/Error), «Наполнение» (empty/medium/full
//! контейнеров), «Канвас» (состояния сущностей ST4), «Токены» (каталог
//! design-токенов + live-правка слотов палитры с кнопкой сброса). Темы —
//! кнопка цикла в шапке (как в витрине FR-055).
//!
//! Интерактив — реальные kit-контролы: пункты сайдбара (Button Secondary,
//! выбранная — слот Selected), кнопка темы (Primary), «Сброс» (Danger),
//! «✕» (IconButton). Демо-контент секций — декоративный (состояния
//! показываются статически, линт не считает его интерактивным).
//!
//! Исключения раскладки: см. `design/rules/11-layouts.md`, раздел
//! «Исключения» (строка «`row_gap: 2.0` — демо Table»; скелетный
//! `gap: 0.0` тел — подсекция «Скелетные ритмы gap:0.0»; backend линеек —
//! §LAY9.1 «паритет-оракул»).

use canvas_core::Language;
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{self, ButtonVariant, KitPalette, KitState};
use canvas_ui::layout::{pilot_backend, Column, HAlign, MeasuredItem, Row, VAlign};
use canvas_ui::measure::TextMeasurer;

/// Семейство шрифта подписей (тот же SANS, что у рендера и витрины).
pub const FONT_FAMILY: &str = crate::kit_ui::FONT_FAMILY;
/// Кегль подписей админпанели.
pub const LABEL_SIZE: f32 = crate::kit_ui::LABEL_SIZE;
/// Зазор между зонами панели — токен `canvas_core::tokens::SPACING_LG`
/// (значение прежнего литерала 12).
pub use canvas_core::tokens::SPACING_LG as ZONE_GAP;
/// Отступ панели админпанели от краёв вьюпорта (spacing-scale XL) — токен
/// `canvas_core::tokens::SPACING_XL` (значение прежнего литерала 24).
use canvas_core::tokens::SPACING_XL as VIEWPORT_MARGIN;
/// Высота строки сайдбара (kit Button).
pub const SIDEBAR_ITEM_H: f32 = kit::BUTTON_HEIGHT;
/// Зазор между пунктами сайдбара — токен `canvas_core::tokens::SPACING_S`
/// (значение прежнего литерала 6).
pub use canvas_core::tokens::SPACING_S as SIDEBAR_ITEM_GAP;
/// Ширина колонки сайдбара.
pub const SIDEBAR_W: f32 = 190.0;
/// Ширина слота кнопки «Сброс».
pub const RESET_SLOT_W: f32 = 96.0;
/// Ширина слота кнопки темы (как в витрине).
pub const THEME_SLOT_W: f32 = crate::kit_ui::THEME_SLOT_W;
/// Высота шапки (ряд кнопок + заголовок).
pub const HEADER_H: f32 = 30.0;

/// Панель админпанели: минимум (влезает в 800×560 окна линта с запасом).
pub const PANEL_MIN: UiVec2 = UiVec2::new(760.0, 500.0);
/// Панель админпанели: максимум (на 1280×800 — 1120×720).
pub const PANEL_MAX: UiVec2 = UiVec2::new(1120.0, 720.0);

/// Секции админпанели (сайдбар; порядок = порядок пунктов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSection {
    /// Все контролы × полная матрица состояний (ST1 + Focused/Error).
    Components,
    /// Контейнеры × наполнение (пустое / среднее / полное).
    Fill,
    /// Сущности канваса (ST4): карточка ноды, рёбра, порты.
    Canvas,
    /// Каталог design-токенов + live-правка слотов палитры.
    Tokens,
}

/// Все секции сайдбара (порядок пунктов).
pub const ADMIN_SECTIONS: [AdminSection; 4] = [
    AdminSection::Components,
    AdminSection::Fill,
    AdminSection::Canvas,
    AdminSection::Tokens,
];

impl AdminSection {
    /// Ключ i18n подписи пункта сайдбара.
    pub fn label_key(self) -> &'static str {
        match self {
            AdminSection::Components => crate::i18n::keys::ADMIN_SECTION_COMPONENTS,
            AdminSection::Fill => crate::i18n::keys::ADMIN_SECTION_FILL,
            AdminSection::Canvas => crate::i18n::keys::ADMIN_SECTION_CANVAS,
            AdminSection::Tokens => crate::i18n::keys::ADMIN_SECTION_TOKENS,
        }
    }

    /// Индекс секции (элемент сайдбара / суффикс hit-элемента).
    pub fn index(self) -> usize {
        ADMIN_SECTIONS.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Секция по индексу пункта сайдбара (вне диапазона — None).
    pub fn at(index: usize) -> Option<AdminSection> {
        ADMIN_SECTIONS.get(index).copied()
    }
}

/// Раскладка админпанели (чистая функция от вьюпорта и секции). Этап 1
/// (каркас): шапка + сайдбар + демо-зона с заголовком секции и подсказкой;
/// тела секций наполняются последующими этапами FR-070 (контент демо-зоны
/// сдвигается `scroll.offset` — контракт скролла кита).
#[derive(Debug, Clone)]
pub struct AdminLayout {
    /// Панель админпанели (kit Modal).
    pub panel: UiRect,
    /// Контент внутри панели (минус пад SPACING_LG).
    pub content: UiRect,
    /// Заголовок «UI-консоль» (шапка, слева).
    pub title: UiRect,
    /// Кнопка «Сброс» (kit Button Danger, интерактив).
    pub reset: UiRect,
    /// Кнопка темы (kit Button Primary, интерактив).
    pub theme: UiRect,
    /// Кнопка «✕» (kit IconButton, интерактив).
    pub close: UiRect,
    /// Колонка сайдбара (контейнер пунктов).
    pub sidebar: UiRect,
    /// Пункты сайдбара — по [`ADMIN_SECTIONS`] (kit Button, интерактив).
    pub sidebar_items: Vec<UiRect>,
    /// Демо-зона (окно скролла тела секции).
    pub demo: UiRect,
    /// Полная высота тела секции (для скролла — контент).
    pub demo_content_h: f32,
    /// Заголовок секции в демо-зоне (origin + ключ i18n).
    pub section_title: (UiPoint, &'static str),
    /// Подсказка тела: wrapped-строки (описание состава секции; перенос —
    /// [`wrap_text`], размер [`LABEL_SIZE`]).
    pub hint_lines: Vec<String>,
    /// Тело «Компоненты» (Some для AdminSection::Components — этап 2).
    pub components: Option<ComponentsLayout>,
    /// Тело «Наполнение» (Some для AdminSection::Fill — этап 2).
    pub fill: Option<FillLayout>,
    /// Тело «Канвас» (Some для AdminSection::Canvas — этап 3).
    pub canvas: Option<CanvasLayout>,
    /// Тело «Токены» (Some для AdminSection::Tokens — этап 3).
    pub tokens: Option<TokensLayout>,
}

/// Панель админпанели, зажатая во вьюпорт (тот же контракт, что у витрины:
/// constrain(min, max, desired) + кламп к вьюпорту — G4-линт).
pub fn admin_panel(vp: UiRect) -> UiRect {
    let avail = UiVec2::new(
        (vp.w - VIEWPORT_MARGIN).max(0.0),
        (vp.h - VIEWPORT_MARGIN).max(0.0),
    );
    let max = UiVec2::new(PANEL_MAX.x.min(avail.x), PANEL_MAX.y.min(avail.y));
    let min = UiVec2::new(PANEL_MIN.x.min(avail.x), PANEL_MIN.y.min(avail.y));
    kit::panel_rect(vp, min, max, PANEL_MAX)
}

/// Геометрия демо-зоны без полной раскладки (колесо скролла — hit-тест
/// зоны без перестроения тел секций; та же формула, что у [`admin_layout`]).
pub fn admin_demo_viewport(viewport: [f32; 2]) -> UiRect {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let body_top = content.y + HEADER_H + ZONE_GAP;
    let demo_x = content.x + SIDEBAR_W + ZONE_GAP;
    UiRect::new(
        demo_x,
        body_top,
        (content.right() - demo_x).max(0.0),
        (content.bottom() - body_top).max(0.0),
    )
}

/// Слоты шапки (кнопки темы/сброса/«✕») без полной раскладки — hit-rect'ы
/// реестра (те же формулы, что у [`admin_layout`]; шапка фиксирована —
/// скролл демо-зоны слоты не сдвигает).
///
/// FR-070 (агент N): миграция на `kit::panel_header` — те же слоты, что у
/// [`admin_layout`] (один вызов `kit::panel_header` даёт кнопки для обоих).
/// Канонизация (AGENTS.md I-1): close прижимается к `slot.right -
/// SPACING_SM` (8px inset), тогда как прежний код ставил close flush к
/// `content.right` — все 3 кнопки сдвигаются на 8px влево (canonical
/// panel-header inset, паритет с `kit_ui::gallery_hit_slots`).
pub fn admin_hit_slots(viewport: [f32; 2], palette: &KitPalette) -> (UiRect, UiRect, UiRect) {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let header_slot = UiRect::new(content.x, content.y, content.w, HEADER_H);
    // Порядок справа-налево: reset (toggle, левее), theme (toggle), close (icon, правее).
    let header_buttons = [
        kit::HeaderButton::Toggle {
            label_w: RESET_SLOT_W,
            id: "reset",
        },
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
    let mut reset = UiRect::default();
    for (r, id) in &header_layout.buttons {
        match *id {
            "close" => close = *r,
            "theme" => theme = *r,
            "reset" => reset = *r,
            _ => {}
        }
    }
    (theme, reset, close)
}

/// Раскладка админпанели. Шапка фиксирована (hit-слоты реестра —
/// [`admin_hit_slots`]); сайдбар и демо-зона — ниже шапки; тело секции
/// (этап 1) — заголовок + подсказка.
#[allow(clippy::too_many_arguments)]
pub fn admin_layout(
    viewport: [f32; 2],
    section: AdminSection,
    scroll_offset: f32,
    p: &KitPalette,
    card_fill: [f32; 4],
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> AdminLayout {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));

    // Шапка: заголовок слева; справа — «Сброс», тема, «✕» (край).
    let (theme, reset, close) = admin_hit_slots(viewport, p);
    let title_w = (reset.x - 8.0 - content.x).max(0.0);
    let title = UiRect::new(content.x, content.y + 6.0, title_w, 20.0);

    // Тело: сайдбар слева, демо-зона в остатке ширины.
    let body_top = content.y + HEADER_H + ZONE_GAP;
    let sidebar = UiRect::new(
        content.x,
        body_top,
        SIDEBAR_W,
        (content.bottom() - body_top).max(0.0),
    );
    let mut sidebar_items = Vec::with_capacity(ADMIN_SECTIONS.len());
    for (i, sec) in ADMIN_SECTIONS.iter().enumerate() {
        let item = kit::button_layout(
            UiRect::new(
                sidebar.x,
                sidebar.y + i as f32 * (SIDEBAR_ITEM_H + SIDEBAR_ITEM_GAP),
                SIDEBAR_W,
                SIDEBAR_ITEM_H,
            ),
            crate::i18n::tr(Language::Ru, sec.label_key()),
            (HAlign::Start, VAlign::Center),
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        sidebar_items.push(item.rect);
    }
    let demo = admin_demo_viewport(viewport);

    // Тело секции (этап 1): заголовок секции + подсказка с переносом.
    let section_title = (UiPoint::new(demo.x, demo.y), section.label_key());
    let hint_key = match section {
        AdminSection::Components => crate::i18n::keys::ADMIN_HINT_COMPONENTS,
        AdminSection::Fill => crate::i18n::keys::ADMIN_HINT_FILL,
        AdminSection::Canvas => crate::i18n::keys::ADMIN_HINT_CANVAS,
        AdminSection::Tokens => crate::i18n::keys::ADMIN_HINT_TOKENS,
    };
    let hint_text = crate::i18n::tr(Language::Ru, hint_key);
    let hint_w = (demo.w - 2.0 * 10.0).max(0.0);
    let hint_lines = wrap_text(m, fs, hint_text, hint_w, LABEL_SIZE);
    let hint_h = 24.0 + hint_lines.len() as f32 * 18.0 + ZONE_GAP;

    // Тело секции (этапы 2–4): у реализованных секций — своё, у остальных —
    // подсказка о составе (этап 1). Фикс налезания 2026-09-25: тело строится
    // ПОД подсказкой (demo, сдвинутый на hint_h) — раньше матрица/контент
    // начинались с demo.y и текст подсказки налезал на заголовки колонок и
    // первую строку контента (скриншот wasm-аудита 19_admin).
    let body_demo = UiRect::new(demo.x, demo.y + hint_h, demo.w, (demo.h - hint_h).max(0.0));
    let (demo_content_h, components, fill, canvas, tokens) = match section {
        AdminSection::Components => {
            let body = components_body(body_demo, scroll_offset, p, lang, m, fs);
            (hint_h + body.h, Some(body), None, None, None)
        }
        AdminSection::Fill => {
            let body = fill_body(body_demo, scroll_offset, p, lang, m, fs);
            (hint_h + body.h, None, Some(body), None, None)
        }
        AdminSection::Canvas => {
            let body = canvas_body(body_demo, scroll_offset, p, card_fill, lang, m, fs);
            (hint_h + body.h, None, None, Some(body), None)
        }
        AdminSection::Tokens => {
            let body = tokens_body(body_demo, scroll_offset, p, lang, m, fs);
            (hint_h + body.h, None, None, None, Some(body))
        }
    };

    AdminLayout {
        panel,
        content,
        title,
        theme,
        reset,
        close,
        sidebar,
        sidebar_items,
        demo,
        demo_content_h,
        section_title,
        hint_lines,
        components,
        fill,
        canvas,
        tokens,
    }
}

/// Перенос текста по словам под заданную ширину (чистая функция —
/// измерение TextMeasurer'ом; длинное слово не рвётся — строка шире слота).
/// FR-068 W3.3 (каталог §9.3.1): тело — делегат [`TextMeasurer::wrap`]
/// (ручной жадный цикл удалён — семантика бит-в-бит идентична, оракул
/// canvas-ui `measure::wrap_matches_manual_greedy_oracle` пинит паритет).
pub fn wrap_text(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    text: &str,
    max_w: f32,
    size: f32,
) -> Vec<String> {
    m.wrap(fs, text, FONT_FAMILY, size, max_w)
}

// === LAY-W13 (возрождение W3a): Column-скелет тел админки ===================
//
// Вертикальные линейки тел демо-секций — один `Column { gap: 0 }` из блоков
// [`MeasuredItem::Fixed`] через [`pilot_backend`] (NativeBackend —
// арифметика прежнего курсора `y +=` байт-в-байт, LAY9.1; образец — W3b,
// `kit_ui::gallery_layout`; аудит layouts-2026-10 §3.2 → §5 LAY-W3).
// Высота блока = прежний шаг курсора (числовые значения перенесены как
// есть — канонизация W6/W7 не менялась). Зазор — ХВОСТ блока, а не
// `MeasuredItem::Spacer`: в Column распорка занимает нулевую высоту
// (док-контракт canvas-ui, урок W3b) — линейка бы схлопнулась.
//
// Слот линейки — контент-координаты тела (y от 0): скелет строится от
// нуля, как прежний курсор; единый сдвиг на `demo.y − offset` (скролл,
// контракт FR-059) — при потреблении блоков. Полная высота тела — низ
// последнего блока линейки. Горизонтальные ряды чипов — `Row { gap }`
// measured (тот же `TextMeasurer`; ширина чипа `width_of + 2·CHIP_PAD_H` —
// бит-в-бит прежнему `chip_size`, поэтому прежний курсор `cx += w + gap`
// заменён без дрейфа). Золотой тест
// `admin_bodies_golden_column_skeleton` (135 тегов, допуск 0.005 px)
// пинит геометрию — дрейф 0.

/// Блок линейки тела: фиксированный [`MeasuredItem::Fixed`] (высота —
/// прежний шаг курсора; ширина номинальная — потребители читают y).
fn block(w: f32, h: f32) -> MeasuredItem<'static> {
    MeasuredItem::Fixed { w, h }
}

/// Линейка тела (LAY-W13): `Column { gap: 0 }` из блоков в
/// контент-координатах тела — см. модульный док «Column-скелет тел
/// админки» выше. Возвращает rect'ы блоков (параллельно `items`).
fn body_ruler(
    demo: UiRect,
    items: &[MeasuredItem<'_>],
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> std::vec::IntoIter<UiRect> {
    Column {
        gap: 0.0,
        ..Column::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        // Слот — контент-координаты (y от 0), как прежний курсор; сдвиг на
        // `demo.y − offset` — при потреблении (скролл-контракт FR-059).
        UiRect::new(demo.x, 0.0, demo.w, demo.h),
        items,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    )
    .into_iter()
}

// === FR-070 этап 2: секция «Компоненты» — полная матрица состояний =========

/// Подписи состояний матрицы (5 из ST1 + Focused; Error — TextField).
pub const STATE_MATRIX_LABELS: [&str; 6] = [
    "kit.state.normal",
    "kit.state.hover",
    "kit.state.selected",
    "kit.state.pressed",
    "kit.state.disabled",
    "kit.state.focused",
];

/// Ширина колонки подписи варианта/контейнера слева.
pub const VARIANT_LABEL_W: f32 = 96.0;
/// Высота строки-заголовка колонок матрицы.
pub const MATRIX_HEADER_H: f32 = 16.0;

/// Ячейка матрицы состояний (контент-координаты; сдвиг/фильтр — при раскладке).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateCell {
    /// Состояние (Normal для Focused-ячейки — рамка accent сверху).
    pub state: KitState,
    /// В фокусе (рамка accent).
    pub focused: bool,
    pub rect: UiRect,
}

/// Ряд кнопок одного варианта (6 состояний матрицы).
#[derive(Debug, Clone)]
pub struct VariantRow {
    pub variant: ButtonVariant,
    /// Ключ i18n названия варианта.
    pub label_key: &'static str,
    pub slot: UiRect,
    pub cells: Vec<StateCell>,
}

/// Демо текстового поля (расширенная матрица: Normal/Focused/Error/Disabled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldDemo {
    Normal,
    Focused,
    Error,
    Disabled,
}

impl FieldDemo {
    /// Ключ i18n подписи демо.
    pub fn label_key(self) -> &'static str {
        match self {
            FieldDemo::Normal => "kit.state.normal",
            FieldDemo::Focused => "kit.state.focused",
            FieldDemo::Error => crate::i18n::keys::ADMIN_STATE_ERROR,
            FieldDemo::Disabled => "kit.state.disabled",
        }
    }
}

/// Демо-тексты полей (числа/формулы — без i18n).
pub const DEMO_FIELD_TEXT: &str = "50 rps";
pub const DEMO_FIELD_LONG: &str = "latency = 50 ms; rps = 1000; replicas = 8";
pub const DEMO_FIELD_ERROR: &str = "rps = abc";

/// Тело секции «Компоненты» (контент-координаты демо-зоны, уже сдвинуты
/// на скролл и отфильтрованы по полной видимости).
#[derive(Debug, Clone, Default)]
pub struct ComponentsLayout {
    /// Подписи колонок матрицы (6 состояний).
    pub headers: Vec<(UiPoint, &'static str)>,
    /// Ряды кнопок: 4 варианта × 6 состояний.
    pub button_rows: Vec<VariantRow>,
    /// Икон-кнопки: 6 состояний.
    pub icon_cells: Vec<StateCell>,
    /// Чипы: 6 состояний.
    pub chip_cells: Vec<StateCell>,
    /// Поля: (демо, раскладка kit::text_field).
    pub fields: Vec<(FieldDemo, kit::TextFieldLayout)>,
    /// Переключатели: (on, состояние, слот, в фокусе — рамка accent).
    pub switches: Vec<(bool, KitState, UiRect, bool)>,
    /// Пересборка поверхностей (волна 2): dropdown закрытый (якорь).
    pub dropdown_closed: Option<UiRect>,
    /// Пересборка поверхностей (волна 2): dropdown открытый (якорь + меню).
    pub dropdown_open: Option<DropdownOpenDemo>,
    /// Пересборка поверхностей (волна 2): tooltip (якорь-чип, пузырь).
    pub tooltip: Option<(UiRect, UiRect)>,
    /// Пересборка поверхностей (волна 2): toast (хром panel_fill + accent).
    pub toast: Option<UiRect>,
    /// Пересборка поверхностей (волна 2): строки списка в состояниях
    /// Normal/Hovered/Selected (статическая демо-матрица).
    pub list_state_rows: Vec<(KitState, UiRect)>,
    /// Пересборка поверхностей (волна 2): бегунок скролла демо-списка
    /// (kit::scroll_bar на демо-ScrollState).
    pub list_scroll_knob: Option<UiRect>,
    /// Пересборка поверхностей (волна 2): строки таблицы — зебра/фокус/
    /// выбор (kit-Row на общих направляющих, Table как источник геометрии).
    pub row_state_rows: Vec<RowStateDemo>,
    /// Полная высота тела (для скролла).
    pub h: f32,
}

/// Пересборка поверхностей (волна 2): открытый dropdown матрицы состояний.
#[derive(Debug, Clone)]
pub struct DropdownOpenDemo {
    pub anchor: UiRect,
    pub menu: UiRect,
    pub items: Vec<UiRect>,
    pub labels: Vec<String>,
}

/// Пересборка поверхностей (волна 2): строка таблицы в состоянии матрицы
/// (зебра/фокус/выбор) — данные + предвычисленная геометрия kit-Row.
#[derive(Debug, Clone)]
pub struct RowStateDemo {
    pub state: KitState,
    pub zebra: bool,
    /// В фокусе (рамка accent рисуется потребителем поверх строки).
    pub focused: bool,
    pub parts: kit::RowParts<'static>,
    pub lay: kit::RowLayout,
}

/// Тело секции «Компоненты»: матрица 4 варианта × 6 состояний + икон-кнопки
/// + чипы + поля (Normal/Focused/Error/Disabled) + переключатели.
#[allow(clippy::too_many_arguments)]
pub fn components_body(
    demo: UiRect,
    offset: f32,
    p: &KitPalette,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> ComponentsLayout {
    let mut lay = ComponentsLayout::default();
    let content_x = demo.x + 8.0;
    let content_w = (demo.w - 16.0).max(0.0);
    // Координаты в КОНТЕНТЕ демо-зоны (y от 0), сдвиг на offset при
    // потреблении блоков линейки.
    let top = demo.y;
    let fully = |ry: f32, rh: f32| ry >= top && ry + rh <= demo.bottom();

    let control_x = content_x + VARIANT_LABEL_W + canvas_core::tokens::SPACING_SM;
    let cells_w = (content_x + content_w - control_x).max(0.0);
    let per = ((cells_w - 5.0 * kit::GAP_CONTROLS) / 6.0).max(0.0);

    // === LAY-W13: вертикальная линейка тела — Column-скелет примитивов
    // (см. модульный док «Column-скелет тел админки»): высота блока =
    // прежний шаг курсора `y +=`; зазор — хвост блока. Вычисляемые высоты
    // (меню dropdown, окно списка, строки таблицы) — до раскладки линейки
    // (те же выражения, что у демо-блоков ниже).
    let menu_h = 3.0 * 26.0 + 8.0;
    let list_area_h = 3.0 * kit::LIST_ROW_H + 2.0 * kit::LIST_ROW_GAP;
    let table_row_h = kit::LIST_ROW_H;
    let mut items: Vec<MeasuredItem> = Vec::with_capacity(28);
    // Матрица состояний: заголовки колонок (+ хвост 6).
    items.push(block(content_w, MATRIX_HEADER_H + 6.0));
    // Ряды кнопок: 4 варианта (шаг 8 — после каждого ряда).
    for _ in 0..4 {
        items.push(block(content_w, kit::BUTTON_HEIGHT + 8.0));
    }
    // Хвост после рядов кнопок.
    items.push(block(content_w, 6.0));
    // Икон-кнопки.
    items.push(block(content_w, kit::ICON_BUTTON_SIZE + 10.0));
    // Чипы.
    items.push(block(content_w, kit::CHIP_HEIGHT + 12.0));
    // Поля: 4 демо × (заголовок 16 + поле с хвостом 6).
    for _ in 0..4 {
        items.push(block(content_w, 16.0));
        items.push(block(content_w, kit::TEXT_FIELD_HEIGHT + 6.0));
    }
    // Хвост после полей.
    items.push(block(content_w, 6.0));
    // Переключатели.
    items.push(block(content_w, kit::BUTTON_HEIGHT + 4.0));
    // Dropdown: заголовок + якоря/меню.
    items.push(block(content_w, 16.0));
    items.push(block(
        content_w,
        kit::BUTTON_HEIGHT + kit::DROPDOWN_GAP + menu_h + 10.0,
    ));
    // Tooltip.
    items.push(block(content_w, 16.0));
    items.push(block(content_w, 26.0 + 12.0));
    // Toast.
    items.push(block(content_w, 16.0));
    items.push(block(content_w, 24.0 + 12.0));
    // Список + бегунок.
    items.push(block(content_w, 16.0));
    items.push(block(content_w, list_area_h + 12.0));
    // Строки таблицы.
    items.push(block(content_w, 16.0));
    items.push(block(content_w, 3.0 * (table_row_h + 2.0) + 12.0));

    let mut ruler = body_ruler(demo, &items, m, fs);
    let matrix = ruler.next().unwrap_or_default();
    let button_slots: Vec<UiRect> = (0..4).map(|_| ruler.next().unwrap_or_default()).collect();
    let _after_buttons = ruler.next().unwrap_or_default(); // хвост 6
    let icons = ruler.next().unwrap_or_default();
    let chips = ruler.next().unwrap_or_default();
    let field_slots: Vec<(UiRect, UiRect)> = (0..4)
        .map(|_| {
            let head = ruler.next().unwrap_or_default();
            let field = ruler.next().unwrap_or_default();
            (head, field)
        })
        .collect();
    let _after_fields = ruler.next().unwrap_or_default(); // хвост 6
    let switches = ruler.next().unwrap_or_default();
    let dd_head = ruler.next().unwrap_or_default();
    let dd = ruler.next().unwrap_or_default();
    let tip_head = ruler.next().unwrap_or_default();
    let tip = ruler.next().unwrap_or_default();
    let toast_head = ruler.next().unwrap_or_default();
    let toast = ruler.next().unwrap_or_default();
    let list_head = ruler.next().unwrap_or_default();
    let list = ruler.next().unwrap_or_default();
    let rows_head = ruler.next().unwrap_or_default();
    let rows = ruler.next().unwrap_or_default();
    // Полная высота тела — низ последнего блока линейки (скролл-контракт
    // FR-059; накопление Column тождественно прежнему курсору).
    let body_h = rows.bottom();

    // Заголовки колонок (6 состояний)
    for (i, key) in STATE_MATRIX_LABELS.iter().enumerate() {
        let hy = demo.y + matrix.y - offset;
        if fully(hy, MATRIX_HEADER_H) {
            lay.headers.push((
                UiPoint::new(control_x + i as f32 * (per + kit::GAP_CONTROLS), hy),
                key,
            ));
        }
    }

    // Ряды кнопок: Primary/Secondary/Ghost/Danger × 6 состояний
    let variants = [
        (ButtonVariant::Primary, crate::i18n::keys::KIT_BTN_PRIMARY),
        (
            ButtonVariant::Secondary,
            crate::i18n::keys::KIT_BTN_SECONDARY,
        ),
        (ButtonVariant::Ghost, crate::i18n::keys::KIT_BTN_GHOST),
        (ButtonVariant::Danger, crate::i18n::keys::KIT_BTN_DANGER),
    ];
    for ((variant, label_key), slot) in variants.into_iter().zip(button_slots) {
        let slot_y = demo.y + slot.y - offset;
        let mut cells = Vec::with_capacity(6);
        for (i, matrix_state) in STATE_MATRIX
            .iter()
            .copied()
            .enumerate()
            .chain([(5usize, KitState::Normal)])
        {
            let focused = i == 5;
            let state = if focused {
                KitState::Normal
            } else {
                matrix_state
            };
            let rect = UiRect::new(
                control_x + i as f32 * (per + kit::GAP_CONTROLS),
                slot_y,
                per,
                kit::BUTTON_HEIGHT,
            );
            if fully(rect.y, rect.h) {
                cells.push(StateCell {
                    state,
                    focused,
                    rect,
                });
            }
        }
        if !cells.is_empty() {
            lay.button_rows.push(VariantRow {
                variant,
                label_key,
                slot: UiRect::new(content_x, slot_y, content_w, kit::BUTTON_HEIGHT),
                cells,
            });
        }
    }

    // Икон-кнопки: 5 состояний ST1 (Focused — не применим к икон-кнопкам v1)
    let icons_y = demo.y + icons.y - offset;
    for (i, st) in STATE_MATRIX.iter().copied().enumerate() {
        let rect = UiRect::new(
            control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
            icons_y,
            kit::ICON_BUTTON_SIZE,
            kit::ICON_BUTTON_SIZE,
        );
        if fully(rect.y, rect.h) {
            lay.icon_cells.push(StateCell {
                state: st,
                focused: false,
                rect,
            });
        }
    }

    // Чипы: 6 состояний — Row { gap } measured (LAY-W13: прежний курсор
    // `cx += w + gap` → примитив Row; замер тот же TextMeasurer — ширина
    // чипа `width_of + 2·CHIP_PAD_H` бит-в-бит прежнему `chip_size`).
    let chips_y = demo.y + chips.y - offset;
    let chip_items: Vec<MeasuredItem> = (0..6)
        .map(|i| MeasuredItem::Text {
            text: crate::i18n::tr(lang, STATE_MATRIX_LABELS[i]),
            max_w: Some(per * 1.5),
            min_w: 0.0,
            pad_x: kit::CHIP_PAD_H * 2.0,
            h: Some(kit::CHIP_HEIGHT),
        })
        .collect();
    let chip_rects = Row {
        gap: kit::GAP_CONTROLS,
        ..Row::default()
    }
    .lay_out_measured_with(
        pilot_backend(),
        UiRect::new(control_x, chips_y, cells_w, kit::CHIP_HEIGHT),
        &chip_items,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    for (i, rect) in chip_rects.iter().enumerate() {
        let focused = i == 5;
        let st = if focused {
            KitState::Normal
        } else {
            STATE_MATRIX[i]
        };
        if fully(rect.y, rect.h) {
            lay.chip_cells.push(StateCell {
                state: st,
                focused,
                rect: *rect,
            });
        }
    }

    // Поля: Normal / Focused / Error / Disabled
    let field_w = (cells_w - 2.0 * kit::GAP_CONTROLS).min(320.0);
    for (demo_kind, (head, field)) in [
        FieldDemo::Normal,
        FieldDemo::Focused,
        FieldDemo::Error,
        FieldDemo::Disabled,
    ]
    .into_iter()
    .zip(field_slots)
    {
        let ty = demo.y + head.y - offset;
        if fully(ty, 14.0) {
            lay.headers
                .push((UiPoint::new(content_x, ty), demo_kind.label_key()));
        }
        let fy = demo.y + field.y - offset;
        let slot = UiRect::new(content_x, fy, field_w, kit::TEXT_FIELD_HEIGHT);
        let model = match demo_kind {
            FieldDemo::Normal => kit::TextFieldModel {
                text: DEMO_FIELD_TEXT.to_owned(),
                caret: DEMO_FIELD_TEXT.chars().count(),
                sel: None,
                max_chars: None,
            },
            FieldDemo::Focused => kit::TextFieldModel {
                text: DEMO_FIELD_TEXT.to_owned(),
                caret: 2,
                sel: None,
                max_chars: None,
            },
            FieldDemo::Error => kit::TextFieldModel {
                text: DEMO_FIELD_ERROR.to_owned(),
                caret: DEMO_FIELD_ERROR.chars().count(),
                sel: None,
                max_chars: None,
            },
            FieldDemo::Disabled => kit::TextFieldModel::default(),
        };
        let fl = kit::text_field(
            slot,
            UiVec2::new(kit::TEXT_FIELD_MIN_W, kit::TEXT_FIELD_HEIGHT),
            UiVec2::new(field_w, kit::TEXT_FIELD_HEIGHT),
            &model,
            crate::i18n::tr(lang, "kit.textfield.placeholder"),
            demo_kind == FieldDemo::Focused,
            KitState::Normal,
            p,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        if fully(fl.rect.y, fl.rect.h) {
            lay.fields.push((demo_kind, fl));
        }
    }

    // Переключатели: on/off × Normal/Hover/Disabled + Pressed/в фокусе
    let sw_y = demo.y + switches.y - offset;
    let sw_states = [
        (true, KitState::Normal, false),
        (false, KitState::Normal, false),
        (true, KitState::Hovered, false),
        (false, KitState::Disabled, false),
        (true, KitState::Pressed, false),
        (true, KitState::Normal, true),
    ];
    for (i, (on, st, focused)) in sw_states.iter().copied().enumerate() {
        let rect = UiRect::new(
            control_x + i as f32 * (kit::SWITCH_W + kit::GAP_CONTROLS + 12.0),
            sw_y,
            kit::SWITCH_W,
            kit::BUTTON_HEIGHT,
        );
        if fully(rect.y, rect.h) {
            lay.switches.push((on, st, rect, focused));
        }
    }

    // === Пересборка поверхностей (волна 2): хвост матрицы состояний ===
    // Popup-контролы, строки списка и строки таблицы — состояния, которые
    // кит стилизует, но базовая матрица не показывала (аудит инвентаря).

    // Dropdown: закрытый (якорь) / открытый (якорь + меню + пункты).
    let dd_y = demo.y + dd_head.y - offset;
    if fully(dd_y, 14.0) {
        lay.headers.push((
            UiPoint::new(content_x, dd_y),
            crate::i18n::keys::ADMIN_MATRIX_DROPDOWN,
        ));
    }
    let closed_anchor = UiRect::new(control_x, demo.y + dd.y - offset, 170.0, kit::BUTTON_HEIGHT);
    if fully(closed_anchor.y, closed_anchor.h) {
        lay.dropdown_closed = Some(closed_anchor);
    }
    let open_anchor = UiRect::new(
        control_x + 186.0,
        demo.y + dd.y - offset,
        170.0,
        kit::BUTTON_HEIGHT,
    );
    // Вьюпорт меню — демо-зона (flip/клампы от неё; якорь в середине зоны —
    // меню открывается вниз без flip).
    let dd = kit::dropdown_menu(open_anchor, demo, UiVec2::new(190.0, menu_h));
    let mut dd_items = Vec::new();
    let mut dd_labels = Vec::new();
    for i in 0..3 {
        let ir = UiRect::new(
            dd.menu.x + 4.0,
            dd.menu.y + 4.0 + i as f32 * 26.0,
            dd.menu.w - 8.0,
            22.0,
        );
        if fully(ir.y, ir.h) {
            dd_items.push(ir);
            dd_labels.push(crate::i18n::trf(
                lang,
                crate::i18n::keys::KIT_DROPDOWN_ITEM,
                &[("{n}", &(i + 1).to_string())],
            ));
        }
    }
    if fully(open_anchor.y, open_anchor.h + kit::DROPDOWN_GAP + menu_h) {
        lay.dropdown_open = Some(DropdownOpenDemo {
            anchor: open_anchor,
            menu: dd.menu,
            items: dd_items,
            labels: dd_labels,
        });
    }

    // Tooltip: якорь-чип + пузырь (delay пройден — показан).
    let tip_y = demo.y + tip_head.y - offset;
    if fully(tip_y, 14.0) {
        lay.headers.push((
            UiPoint::new(content_x, tip_y),
            crate::i18n::keys::ADMIN_MATRIX_TOOLTIP,
        ));
    }
    let tip_anchor_label = crate::i18n::tr(lang, crate::i18n::keys::KIT_TOOLTIP_ANCHOR);
    let tip_anchor = kit::chip_layout(
        UiPoint::new(control_x, demo.y + tip.y - offset),
        tip_anchor_label,
        140.0,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let tip_text = crate::i18n::tr(lang, crate::i18n::keys::KIT_TOOLTIP_BODY);
    let tip_w = (m.width_of(fs, tip_text, FONT_FAMILY, 12.0) + 16.0).min(240.0);
    let tip = kit::tooltip(
        UiPoint::new(tip_anchor.rect.right(), tip_anchor.rect.y + 4.0),
        UiVec2::new(tip_w, 18.0),
        demo,
        kit::TOOLTIP_DELAY_MS,
        kit::TOOLTIP_DELAY_MS,
    );
    if fully(tip_anchor.rect.y, tip_anchor.rect.h) {
        lay.tooltip = Some((
            tip_anchor.rect,
            tip.map(|t| t.rect)
                .unwrap_or(UiRect::new(0.0, 0.0, 0.0, 0.0)),
        ));
    }

    // Toast: хром тоста (panel_fill + accent-рамка) — статическая демо-строка.
    let toast_y = demo.y + toast_head.y - offset;
    if fully(toast_y, 14.0) {
        lay.headers.push((
            UiPoint::new(content_x, toast_y),
            crate::i18n::keys::ADMIN_MATRIX_TOAST,
        ));
    }
    let toast_rect = UiRect::new(control_x, demo.y + toast.y - offset, 260.0, 24.0);
    if fully(toast_rect.y, toast_rect.h) {
        lay.toast = Some(toast_rect);
    }

    // Строки списка: Normal / Hovered / Selected + бегунок скролла
    // (kit::scroll_bar — контент 8 строк в окне 3).
    let list_y = demo.y + list_head.y - offset;
    if fully(list_y, 14.0) {
        lay.headers.push((
            UiPoint::new(content_x, list_y),
            crate::i18n::keys::ADMIN_MATRIX_LIST,
        ));
    }
    let list_area = UiRect::new(
        control_x,
        demo.y + list.y - offset,
        (cells_w * 0.6).max(160.0),
        list_area_h,
    );
    for (i, st) in [KitState::Normal, KitState::Hovered, KitState::Selected]
        .into_iter()
        .enumerate()
    {
        let r = UiRect::new(
            list_area.x + 4.0,
            list_area.y + 4.0 + i as f32 * (kit::LIST_ROW_H + kit::LIST_ROW_GAP),
            list_area.w - 12.0,
            kit::LIST_ROW_H,
        );
        if fully(r.y, r.h) {
            lay.list_state_rows.push((st, r));
        }
    }
    if fully(list_area.y, list_area.h) {
        let scroll = canvas_ui::kit::ScrollState {
            offset: 24.0,
            content_h: 8.0 * kit::LIST_ROW_H + 7.0 * kit::LIST_ROW_GAP,
            viewport_h: list_area.h - 8.0,
        };
        lay.list_scroll_knob = canvas_ui::kit::scroll_bar(
            UiRect::new(
                list_area.x + 4.0,
                list_area.y + 4.0,
                list_area.w - 8.0,
                list_area.h - 8.0,
            ),
            &scroll,
            p,
        );
    }

    // Строки таблицы: зебра (fill слотом hover_fill) / фокус (рамка accent
    // поверх строки) / выбор (Selected) — источник геометрии Table (M4).
    let rows_y = demo.y + rows_head.y - offset;
    if fully(rows_y, 14.0) {
        lay.headers.push((
            UiPoint::new(content_x, rows_y),
            crate::i18n::keys::ADMIN_MATRIX_ROWS,
        ));
    }
    {
        let row_specs: [(kit::RowMarker, &str, &str, &str, &str); 3] = [
            (
                kit::RowMarker::Dot,
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_PRICE),
                "50",
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_PRICE),
                "",
            ),
            (
                kit::RowMarker::Dot,
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_QTY),
                "12",
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_QTY),
                "",
            ),
            (
                kit::RowMarker::Glyph("ƒ"),
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_TOTAL),
                "600",
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_MONEY),
                crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_BADGE),
            ),
        ];
        let row_h = table_row_h;
        let mut table = kit::Table::new(kit::TableProps {
            size: LABEL_SIZE,
            family: FONT_FAMILY,
            opts: kit::TableOpts {
                leader: true,
                gap: canvas_core::tokens::TABLE_GUIDE_GAP,
                row_h,
                row_gap: 2.0,
                right_pad: 0.0,
            },
            palette: *p,
        });
        table.set_rows(
            row_specs
                .iter()
                .enumerate()
                .map(|(i, (marker, label, value, unit, badge))| kit::TableRow {
                    marker: *marker,
                    label: (*label).to_owned(),
                    value: (*value).to_owned(),
                    unit: (*unit).to_owned(),
                    badge: (!badge.is_empty()).then(|| (*badge).to_owned()),
                    state: if i == 2 {
                        KitState::Selected
                    } else {
                        KitState::Normal
                    },
                    style: kit::TableRowStyle {
                        fill: (i == 0).then_some(p.hover_fill),
                        ..kit::TableRowStyle::default()
                    },
                })
                .collect(),
        );
        let viewport_right = control_x + cells_w;
        if table.guides_with(m, fs, viewport_right).is_some() {
            for (i, (marker, label, value, unit, badge)) in row_specs.into_iter().enumerate() {
                let slot = UiRect::new(
                    control_x,
                    demo.y + rows.y - offset + i as f32 * (row_h + 2.0),
                    cells_w,
                    row_h,
                );
                if !fully(slot.y, slot.h) {
                    continue;
                }
                let Some(rl) = table.row_layout_with(m, fs, viewport_right, slot, i) else {
                    continue;
                };
                lay.row_state_rows.push(RowStateDemo {
                    state: if i == 2 {
                        KitState::Selected
                    } else {
                        KitState::Normal
                    },
                    zebra: i == 0,
                    focused: i == 1,
                    parts: kit::RowParts {
                        marker,
                        label,
                        value,
                        unit,
                        badge,
                    },
                    lay: rl,
                });
            }
        }
    }

    lay.h = body_h;
    lay
}

/// Отрисовка тела «Компоненты» (ячейки уже сдвинуты/отфильтрованы).
pub(crate) fn draw_components(
    d: &mut crate::kit_ui::KitDraw,
    comp: &ComponentsLayout,
    p: &KitPalette,
    lang: Language,
) {
    let label = |key: &'static str| crate::i18n::tr(lang, key).to_owned();
    // Заголовки колонок
    for (origin, key) in &comp.headers {
        d.label_left(
            UiRect::new(origin.x, origin.y, VARIANT_LABEL_W, MATRIX_HEADER_H),
            &label(key),
            p.text_muted,
            11.0,
        );
    }
    // Ряды кнопок
    for row in &comp.button_rows {
        d.label_left(
            UiRect::new(row.slot.x, row.slot.y + 8.0, VARIANT_LABEL_W, 16.0),
            &label(row.label_key),
            p.text_muted,
            11.0,
        );
        for cell in &row.cells {
            let style = kit::button_style(row.variant, cell.state, p);
            d.control(cell.rect, &style);
            let text = if cell.focused {
                label(STATE_MATRIX_LABELS[5])
            } else {
                label(STATE_MATRIX_LABELS[state_matrix_label_index(cell.state)])
            };
            d.label_center(cell.rect, &text, style.text, LABEL_SIZE);
            if cell.focused {
                d.rect(cell.rect, [0.0; 4], p.accent, style.radius);
            }
        }
    }
    // Икон-кнопки
    let icons = ["×", "•••", "?", "+", "+"];
    for (i, cell) in comp.icon_cells.iter().enumerate() {
        let style = kit::icon_button_style(cell.state, p);
        d.control(cell.rect, &style);
        let area = UiRect::new(cell.rect.x, cell.rect.y + 1.0, cell.rect.w, cell.rect.h);
        d.label_center(area, icons[i], style.text, LABEL_SIZE);
        if cell.focused {
            d.rect(cell.rect, [0.0; 4], p.accent, style.radius);
        }
    }
    // Чипы
    for cell in &comp.chip_cells {
        let style = kit::chip_style(cell.state, p);
        d.control(cell.rect, &style);
        let text = if cell.focused {
            label(STATE_MATRIX_LABELS[5])
        } else {
            label(STATE_MATRIX_LABELS[state_matrix_label_index(cell.state)])
        };
        d.label_center(cell.rect, &text, style.text, 12.0);
        if cell.focused {
            d.rect(cell.rect, [0.0; 4], p.accent, style.radius);
        }
    }
    // Поля: заливка/рамка по демо; Error — примитив ERROR из design/tokens
    // (слот error в палитре кита не заведён — ST2; демо показывает примитив)
    for (demo_kind, fl) in &comp.fields {
        let (fill, border, text_color) = match demo_kind {
            FieldDemo::Normal | FieldDemo::Focused => (p.control_fill, p.control_border, p.text),
            FieldDemo::Error => (
                p.control_fill,
                [
                    canvas_core::tokens::ERROR[0] as f32 / 255.0,
                    canvas_core::tokens::ERROR[1] as f32 / 255.0,
                    canvas_core::tokens::ERROR[2] as f32 / 255.0,
                    1.0,
                ],
                [
                    canvas_core::tokens::ERROR[0] as f32 / 255.0,
                    canvas_core::tokens::ERROR[1] as f32 / 255.0,
                    canvas_core::tokens::ERROR[2] as f32 / 255.0,
                    1.0,
                ],
            ),
            FieldDemo::Disabled => (p.control_fill, p.control_border, p.disabled_text),
        };
        d.rect(fl.rect, fill, border, canvas_core::tokens::RADIUS_CHIP);
        if *demo_kind == FieldDemo::Focused {
            d.rect(
                fl.rect,
                [0.0; 4],
                p.accent,
                canvas_core::tokens::RADIUS_CHIP,
            );
        }
        d.label_left(fl.text_area, &fl.text_shown, text_color, LABEL_SIZE);
        if *demo_kind == FieldDemo::Focused && fl.caret_x >= 0.0 {
            d.rect(
                UiRect::new(
                    fl.text_area.x + fl.caret_x,
                    fl.text_area.y + 6.0,
                    1.0,
                    fl.text_area.h - 12.0,
                ),
                p.accent,
                [0.0; 4],
                0.0,
            );
        }
    }
    // Переключатели
    for (on, st, rect, focused) in &comp.switches {
        let sw = kit::switch(*rect, *on, *st, p);
        d.control(sw.track, &sw.track_style);
        d.rect(sw.knob, sw.knob_fill, [0.0; 4], 4.0);
        if *focused {
            d.rect(*rect, [0.0; 4], p.accent, canvas_core::tokens::RADIUS_CHIP);
        }
    }

    // === Пересборка поверхностей (волна 2): хвост матрицы состояний ===
    // Dropdown закрытый / открытый (kit::dropdown_menu — хром меню слотами)
    if let Some(anchor) = &comp.dropdown_closed {
        let style = kit::button_style(ButtonVariant::Secondary, KitState::Normal, p);
        d.control(*anchor, &style);
        let text = crate::i18n::tr(lang, crate::i18n::keys::KIT_DROPDOWN_ANCHOR).to_owned();
        d.label_center(*anchor, &text, style.text, LABEL_SIZE);
    }
    if let Some(dd) = &comp.dropdown_open {
        let style = kit::button_style(ButtonVariant::Secondary, KitState::Normal, p);
        d.control(dd.anchor, &style);
        let text = crate::i18n::tr(lang, crate::i18n::keys::KIT_DROPDOWN_ANCHOR).to_owned();
        d.label_center(dd.anchor, &text, style.text, LABEL_SIZE);
        d.rect(
            dd.menu,
            p.panel_fill,
            p.control_border,
            canvas_core::tokens::RADIUS_CHIP,
        );
        for (i, (ir, text)) in dd.items.iter().zip(dd.labels.iter()).enumerate() {
            // Демо состояний пунктов: первый — hover (статическая матрица).
            let item_style = kit::chip_style(
                if i == 0 {
                    KitState::Hovered
                } else {
                    KitState::Normal
                },
                p,
            );
            d.rect(*ir, item_style.fill, [0.0; 4], 4.0);
            d.label_left(
                UiRect::new(ir.x + 8.0, ir.y + 3.0, ir.w - 16.0, ir.h),
                text,
                item_style.text,
                12.0,
            );
        }
    }
    // Tooltip: якорь-чип + пузырь (панельная хрома + accent-рамка)
    if let Some((anchor, bubble)) = &comp.tooltip {
        let style = kit::chip_style(KitState::Normal, p);
        d.control(*anchor, &style);
        let text = crate::i18n::tr(lang, crate::i18n::keys::KIT_TOOLTIP_ANCHOR).to_owned();
        d.label_center(*anchor, &text, style.text, 12.0);
        if !bubble.is_empty() {
            d.rect(
                *bubble,
                p.panel_fill,
                p.accent,
                canvas_core::tokens::RADIUS_CHIP,
            );
            let text = crate::i18n::tr(lang, crate::i18n::keys::KIT_TOOLTIP_BODY).to_owned();
            d.label_left(
                UiRect::new(bubble.x + 8.0, bubble.y + 2.0, bubble.w - 16.0, 14.0),
                &text,
                p.text,
                12.0,
            );
        }
    }
    // Toast: хром тоста (панельная заливка + accent-рамка, слот текста)
    if let Some(rect) = &comp.toast {
        d.rect(
            *rect,
            p.panel_fill,
            p.accent,
            canvas_core::tokens::RADIUS_CHIP,
        );
        let text = crate::i18n::tr(lang, crate::i18n::keys::ADMIN_TOAST_SHORT).to_owned();
        d.label_center(*rect, &text, p.text, 12.0);
    }
    // Строки списка: Normal/Hovered/Selected + бегунок (kit::scroll_bar)
    for (i, (st, rect)) in comp.list_state_rows.iter().enumerate() {
        let style = kit::chip_style(*st, p);
        d.rect(*rect, style.fill, [0.0; 4], 4.0);
        let text = crate::i18n::trf(
            lang,
            crate::i18n::keys::KIT_LIST_ROW,
            &[("{n}", &(i + 1).to_string())],
        );
        d.label_left(
            UiRect::new(rect.x + 8.0, rect.y + 4.0, rect.w - 16.0, rect.h - 8.0),
            &text,
            style.text,
            12.0,
        );
    }
    if let Some(knob) = &comp.list_scroll_knob {
        d.rect(*knob, p.control_border, [0.0; 4], 2.0);
    }
    // Строки таблицы: зебра/фокус/выбор (paint_row по готовой геометрии)
    for demo_row in &comp.row_state_rows {
        let mut style = kit::row_style(demo_row.state, p);
        if demo_row.zebra {
            style.fill = p.hover_fill;
        }
        let mut painter = canvas_ui::paint::Painter::new();
        kit::paint_row(
            &mut painter,
            &demo_row.lay,
            &demo_row.parts,
            &style,
            LABEL_SIZE,
        );
        d.paint_items(painter.take_items());
        if demo_row.focused {
            d.rect(
                demo_row.lay.row,
                [0.0; 4],
                p.accent,
                canvas_core::tokens::RADIUS_CHIP,
            );
        }
    }
}

/// Матрица состояний ST1 (5) — порядок колонок.
const STATE_MATRIX: [KitState; 5] = [
    KitState::Normal,
    KitState::Hovered,
    KitState::Selected,
    KitState::Pressed,
    KitState::Disabled,
];

/// Маппинг KitState → индекс колонки матрицы (Normal/Hover/Selected/
/// Pressed/Disabled; Focused — 5-я колонка рядом) для подписи ячейки по
/// состоянию.
/// Fix main (Wave T af5ddb3): KitState вырос 5→8 и ПЕРЕУПОРЯДОЧИЛСЯ
/// (Focused/Dragged между старыми вариантами — `as usize`-индексация
/// константы уехала в панику). Исчерпывающий match компилируется на
/// любом порядке и охраняет расширение; новые Wave T-состояния матрицей
/// не демонстрируются (Dragged/Error — вне ST1, Error показан TextField) —
/// безопасный фолбэк на колонку Normal.
fn state_matrix_label_index(state: KitState) -> usize {
    match state {
        KitState::Normal => 0,
        KitState::Hovered => 1,
        KitState::Selected => 2,
        KitState::Pressed => 3,
        KitState::Disabled => 4,
        KitState::Focused => 5,
        KitState::Dragged | KitState::Error => 0,
    }
}

// === FR-070 этап 2: секция «Наполнение» — empty/medium/full ================

/// Уровень наполнения контейнера.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillLevel {
    /// Пустое (0 элементов — placeholder/empty-state).
    Empty,
    /// Среднее (несколько элементов).
    Medium,
    /// Полное (много элементов / скролл).
    Full,
}

/// Все уровни наполнения (порядок колонок).
pub const FILL_LEVELS: [FillLevel; 3] = [FillLevel::Empty, FillLevel::Medium, FillLevel::Full];

impl FillLevel {
    /// Ключ i18n подписи уровня.
    pub fn label_key(self) -> &'static str {
        match self {
            FillLevel::Empty => crate::i18n::keys::ADMIN_FILL_EMPTY,
            FillLevel::Medium => crate::i18n::keys::ADMIN_FILL_MEDIUM,
            FillLevel::Full => crate::i18n::keys::ADMIN_FILL_FULL,
        }
    }

    /// Индекс колонки.
    pub fn index(self) -> usize {
        FILL_LEVELS.iter().position(|l| *l == self).unwrap_or(0)
    }
}

/// Высота демо-ячейки контейнера.
pub const FILL_CELL_H: f32 = 96.0;
/// Зазор между демо-ячейками — токен `canvas_core::tokens::SPACING_SM`
/// (значение прежнего литерала 8).
pub use canvas_core::tokens::SPACING_SM as FILL_CELL_GAP;

/// Ячейка уровня наполнения (контент-координаты).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FillCell {
    pub level: FillLevel,
    pub rect: UiRect,
}

/// Ряд контейнера (3 уровня наполнения).
#[derive(Debug, Clone)]
pub struct FillRow {
    /// Ключ i18n названия контейнера.
    pub title_key: &'static str,
    pub slot: UiRect,
    pub cells: Vec<FillCell>,
}

/// Демо dropdown: уровень, якорь, меню, видимые пункты с подписями.
#[derive(Debug, Clone)]
pub struct DropdownDemo {
    pub level: FillLevel,
    pub anchor: UiRect,
    pub menu: UiRect,
    pub items: Vec<UiRect>,
    pub labels: Vec<String>,
}

/// Демо-строка таблицы (kit-Row на общих направляющих).
/// FR-068 W3 (этап M4): источник значений/геометрии — компонент Table
/// ([`kit::Table::set_rows`] + [`kit::Table::row_layout_with`] в
/// `fill_body`); структура сохранена (предвычисленный [`kit::RowLayout`]) —
/// отрисовка прежняя ([`kit::paint_row`] в `draw_fill`).
#[derive(Debug, Clone)]
pub struct FillTableDemo {
    pub level: FillLevel,
    /// Состояние строки (Σ — Selected, зебра — hover_fill).
    pub state: KitState,
    pub zebra: bool,
    pub parts: kit::RowParts<'static>,
    pub lay: kit::RowLayout,
}

/// Тело секции «Наполнение»: 9 контейнеров × empty/medium/full. Геометрия
/// измеряемых демо (поля/чипы/dropdown/таблица) — предвычислена здесь;
/// простая (список/карточка/палитра/wheel/тост) — в отрисовке.
#[derive(Debug, Clone, Default)]
pub struct FillLayout {
    /// Подписи колонок уровней.
    pub headers: Vec<(UiPoint, &'static str)>,
    /// Ряды контейнеров.
    pub rows: Vec<FillRow>,
    /// Поля: (уровень, раскладка).
    pub field_lays: Vec<(FillLevel, kit::TextFieldLayout)>,
    /// Чипы: (уровень, rect).
    pub chip_lays: Vec<(FillLevel, UiRect)>,
    /// Dropdown: якорь + меню + видимые пункты.
    pub dropdown: Vec<DropdownDemo>,
    /// Таблица строк: демо kit-Row (геометрия — Table, FR-068 M4).
    pub row_table: Vec<FillTableDemo>,
    /// Полная высота тела (для скролла).
    pub h: f32,
}

/// Тело секции «Наполнение».
#[allow(clippy::too_many_arguments)]
pub fn fill_body(
    demo: UiRect,
    offset: f32,
    p: &KitPalette,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> FillLayout {
    let mut lay = FillLayout::default();
    let content_x = demo.x + 8.0;
    let content_w = (demo.w - 16.0).max(0.0);
    let top = demo.y;
    let fully = |ry: f32, rh: f32| ry >= top && ry + rh <= demo.bottom();

    let control_x = content_x + VARIANT_LABEL_W + canvas_core::tokens::SPACING_SM;
    let cells_w = (content_x + content_w - control_x).max(0.0);
    let per = ((cells_w - 2.0 * FILL_CELL_GAP) / 3.0).max(0.0);

    // Контейнеры: (ключ названия, высота ряда)
    let row_specs: [(&'static str, f32); 9] = [
        (crate::i18n::keys::ADMIN_ROW_LIST, FILL_CELL_H + 6.0),
        (crate::i18n::keys::ADMIN_ROW_CARD, FILL_CELL_H + 6.0),
        (
            crate::i18n::keys::ADMIN_ROW_FIELD,
            2.0 * 14.0 + kit::TEXT_FIELD_HEIGHT + 10.0,
        ),
        (
            crate::i18n::keys::ADMIN_ROW_CHIPS,
            2.0 * kit::CHIP_HEIGHT + 12.0,
        ),
        (
            crate::i18n::keys::ADMIN_ROW_DROPDOWN,
            2.4 * kit::BUTTON_HEIGHT + 4.0 * 26.0,
        ),
        (
            crate::i18n::keys::ADMIN_ROW_ROWS,
            5.0 * kit::LIST_ROW_H + 10.0,
        ),
        (crate::i18n::keys::ADMIN_ROW_TOAST, 2.0 * 24.0 + 10.0),
        (crate::i18n::keys::ADMIN_ROW_PALETTE, FILL_CELL_H + 6.0),
        (crate::i18n::keys::ADMIN_ROW_WHEEL, FILL_CELL_H + 6.0),
    ];

    // === LAY-W13: вертикальная линейка тела — Column-скелет примитивов
    // (см. модульный док «Column-скелет тел админки»): блок заголовка
    // (шаг MATRIX_HEADER_H + 6) + блок на ряд контейнера (высота —
    // прежний шаг курсора `y += row_h`).
    let mut items: Vec<MeasuredItem> = Vec::with_capacity(1 + row_specs.len());
    items.push(block(content_w, MATRIX_HEADER_H + 6.0));
    for (_, row_h) in &row_specs {
        items.push(block(content_w, *row_h));
    }
    let mut ruler = body_ruler(demo, &items, m, fs);
    let fill_header = ruler.next().unwrap_or_default();
    let row_slots: Vec<UiRect> = (0..row_specs.len())
        .map(|_| ruler.next().unwrap_or_default())
        .collect();
    // Полная высота тела — низ последнего блока линейки (скролл-контракт
    // FR-059; накопление Column тождественно прежнему курсору).
    let body_h = row_slots
        .last()
        .map(|r| r.bottom())
        .unwrap_or(MATRIX_HEADER_H + 6.0);

    // Заголовки колонок (3 уровня)
    let header_y = demo.y + fill_header.y - offset;
    for (i, level) in FILL_LEVELS.iter().copied().enumerate() {
        if fully(header_y, MATRIX_HEADER_H) {
            lay.headers.push((
                UiPoint::new(control_x + i as f32 * (per + FILL_CELL_GAP), header_y),
                level.label_key(),
            ));
        }
    }

    for ((title_key, row_h), slot) in row_specs.into_iter().zip(row_slots) {
        let slot_y = demo.y + slot.y - offset;
        let mut cells = Vec::with_capacity(3);
        for (i, level) in FILL_LEVELS.iter().copied().enumerate() {
            let rect = UiRect::new(
                control_x + i as f32 * (per + FILL_CELL_GAP),
                slot_y,
                per,
                row_h - 6.0,
            );
            if fully(rect.y, rect.h) {
                cells.push(FillCell { level, rect });
            }
        }
        if !cells.is_empty() {
            lay.rows.push(FillRow {
                title_key,
                slot: UiRect::new(content_x, slot_y, content_w, row_h),
                cells,
            });
        }
    }

    // Предвычисление измеряемых демо (в тех же слотах, что в lay.rows —
    // поиск по title_key и уровню)
    let cell_rect = |title_key: &str, level: FillLevel| -> UiRect {
        lay.rows
            .iter()
            .find(|r| r.title_key == title_key)
            .and_then(|r| r.cells.iter().find(|c| c.level == level))
            .map(|c| c.rect)
            .unwrap_or(UiRect::new(0.0, 0.0, 0.0, 0.0))
    };

    // Поля: пустое (placeholder) / среднее / полное (длинный текст)
    for (level, model, placeholder) in [
        (FillLevel::Empty, kit::TextFieldModel::default(), true),
        (
            FillLevel::Medium,
            kit::TextFieldModel {
                text: DEMO_FIELD_TEXT.to_owned(),
                caret: 0,
                sel: None,
                max_chars: None,
            },
            false,
        ),
        (
            FillLevel::Full,
            kit::TextFieldModel {
                text: DEMO_FIELD_LONG.to_owned(),
                caret: 0,
                sel: None,
                max_chars: None,
            },
            false,
        ),
    ] {
        let rect = cell_rect(crate::i18n::keys::ADMIN_ROW_FIELD, level);
        let fl = kit::text_field(
            rect,
            UiVec2::new(kit::TEXT_FIELD_MIN_W, kit::TEXT_FIELD_HEIGHT),
            UiVec2::new(rect.w, kit::TEXT_FIELD_HEIGHT),
            &model,
            if placeholder {
                crate::i18n::tr(lang, "kit.textfield.placeholder")
            } else {
                ""
            },
            false,
            KitState::Normal,
            p,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        lay.field_lays.push((level, fl));
    }

    // Чипы: пустое (нет) / 2 / 5 — метрики единиц измерения (без i18n) —
    // Row { gap } measured (LAY-W13: прежний курсор `cx += w + gap` →
    // примитив Row; ширина чипа бит-в-бит прежнему `chip_size`).
    let chip_labels: [&str; 5] = ["rps", "ms", "$/mo", "MB/s", "%"];
    for (level, count) in [(FillLevel::Medium, 2usize), (FillLevel::Full, 5usize)] {
        let rect = cell_rect(crate::i18n::keys::ADMIN_ROW_CHIPS, level);
        let chip_items: Vec<MeasuredItem> = chip_labels
            .iter()
            .take(count)
            .map(|label| MeasuredItem::Text {
                text: label,
                max_w: Some(rect.w),
                min_w: 0.0,
                pad_x: kit::CHIP_PAD_H * 2.0,
                h: Some(kit::CHIP_HEIGHT),
            })
            .collect();
        let chip_rects = Row {
            gap: kit::GAP_CONTROLS,
            ..Row::default()
        }
        .lay_out_measured_with(
            pilot_backend(),
            UiRect::new(rect.x, rect.y, rect.w, kit::CHIP_HEIGHT),
            &chip_items,
            m,
            fs,
            FONT_FAMILY,
            12.0,
        );
        for r in chip_rects {
            lay.chip_lays.push((level, r));
        }
    }

    // Dropdown: пустое (0 пунктов) / 3 / 6
    for (level, count) in [
        (FillLevel::Empty, 0usize),
        (FillLevel::Medium, 3),
        (FillLevel::Full, 6),
    ] {
        let rect = cell_rect(crate::i18n::keys::ADMIN_ROW_DROPDOWN, level);
        let anchor = UiRect::new(rect.x, rect.y, rect.w.min(170.0), kit::BUTTON_HEIGHT);
        let menu_h = (count.min(4) as f32 * 26.0 + 8.0).max(8.0);
        let menu = UiRect::new(
            anchor.x,
            anchor.bottom() + kit::DROPDOWN_GAP,
            anchor.w,
            menu_h,
        );
        let mut items = Vec::new();
        let mut labels = Vec::new();
        for i in 0..count.min(4) {
            let ir = UiRect::new(
                menu.x + 4.0,
                menu.y + 4.0 + i as f32 * 26.0,
                menu.w - 8.0,
                22.0,
            );
            items.push(ir);
            labels.push(crate::i18n::trf(
                lang,
                crate::i18n::keys::KIT_DROPDOWN_ITEM,
                &[("{n}", &(i + 1).to_string())],
            ));
        }
        lay.dropdown.push(DropdownDemo {
            level,
            anchor,
            menu,
            items,
            labels,
        });
    }

    // Таблица строк: компонент Table (FR-068 W3, этап M4) — источник
    // значений/геометрии демо (set_rows + row_layout_with per слот);
    // «пустое/2/4+Σ». Параметры — паритет прежнего ручного пути:
    // right_pad 8 (правый край направляющих был rect.right() − 8;
    // viewport_right = rect.right() — ПРАВЫЙ КРАЙ КОНТРОЛ-КОЛОНКИ В
    // КООРДИНАТАХ СЛОТОВ), шаг слотов LIST_ROW_H + 2, лидер on
    // (RowOpts::default), зазор направляющих TABLE_GUIDE_GAP. Геометрия/
    // стили демо не меняются: FillTableDemo хранит тот же предвычисленный
    // RowLayout, отрисовка — прежняя (paint_row в draw_fill).
    type TableRowSpec = (
        FillLevel,
        Vec<(&'static str, &'static str, &'static str, kit::RowMarker)>,
    );
    let table_specs: [TableRowSpec; 3] = [
        (FillLevel::Empty, vec![]),
        (
            FillLevel::Medium,
            vec![
                ("Цена", "50", "$", kit::RowMarker::Dot),
                ("Кол-во", "12", "шт", kit::RowMarker::Dot),
            ],
        ),
        (
            FillLevel::Full,
            vec![
                ("Цена", "50", "$", kit::RowMarker::Dot),
                ("Кол-во", "12", "шт", kit::RowMarker::Dot),
                ("Итого", "600", "$", kit::RowMarker::Glyph("ƒ")),
                ("Маржа", "38", "%", kit::RowMarker::Dot),
            ],
        ),
    ];
    let mut table = kit::Table::new(kit::TableProps {
        size: LABEL_SIZE,
        family: FONT_FAMILY,
        opts: kit::TableOpts {
            leader: true,
            gap: canvas_core::tokens::TABLE_GUIDE_GAP,
            row_h: kit::LIST_ROW_H,
            row_gap: 2.0,
            right_pad: 8.0,
        },
        palette: *p,
    });
    for (level, specs) in table_specs {
        let rect = cell_rect(crate::i18n::keys::ADMIN_ROW_ROWS, level);
        if specs.is_empty() || rect.w <= 0.0 {
            continue;
        }
        let parts: Vec<kit::RowParts<'static>> = specs
            .iter()
            .map(|(label, value, unit, marker)| kit::RowParts {
                marker: *marker,
                label,
                value,
                unit,
                badge: "",
            })
            .collect();
        // Строки Table — те же данные (+ состояния/зебра-override: Σ
        // полного ряда — Selected, нечётные — fill слотом hover_fill, F-8:
        // готовый слот, без арифметики).
        table.set_rows(
            parts
                .iter()
                .enumerate()
                .map(|(i, part)| kit::TableRow {
                    marker: part.marker,
                    label: part.label.to_owned(),
                    value: part.value.to_owned(),
                    unit: part.unit.to_owned(),
                    badge: (!part.badge.is_empty()).then(|| part.badge.to_owned()),
                    state: if i + 1 == parts.len() && level == FillLevel::Full {
                        KitState::Selected
                    } else {
                        KitState::Normal
                    },
                    style: kit::TableRowStyle {
                        fill: (i % 2 == 1).then_some(p.hover_fill),
                        ..kit::TableRowStyle::default()
                    },
                })
                .collect(),
        );
        // Деградация §4.2 (guides None) — паритет прежнего пути
        // (row_guides None → ряд пропускался).
        if table.guides_with(m, fs, rect.right()).is_none() {
            continue;
        }
        for (i, part) in parts.iter().copied().enumerate() {
            let row_slot = UiRect::new(
                rect.x,
                rect.y + i as f32 * (kit::LIST_ROW_H + 2.0),
                rect.w,
                kit::LIST_ROW_H,
            );
            let Some(rl) = table.row_layout_with(m, fs, rect.right(), row_slot, i) else {
                continue;
            };
            let is_sum = i + 1 == parts.len() && level == FillLevel::Full;
            lay.row_table.push(FillTableDemo {
                level,
                state: if is_sum {
                    KitState::Selected
                } else {
                    KitState::Normal
                },
                zebra: i % 2 == 1,
                parts: part,
                lay: rl,
            });
        }
    }

    lay.h = body_h;
    lay
}

/// Подпись-заглушка пустого контейнера (слот text_muted, по центру).
fn draw_empty_hint(d: &mut crate::kit_ui::KitDraw, cell: UiRect, p: &KitPalette, text: &str) {
    d.label_center(cell, text, p.text_muted, 12.0);
}

/// Отрисовка тела «Наполнение» (ячейки уже сдвинуты/отфильтрованы).
pub(crate) fn draw_fill(
    d: &mut crate::kit_ui::KitDraw,
    fill: &FillLayout,
    p: &KitPalette,
    lang: Language,
) {
    let label = |key: &'static str| crate::i18n::tr(lang, key).to_owned();
    let empty_hint = label(crate::i18n::keys::ADMIN_EMPTY_PLACEHOLDER);
    for (origin, key) in &fill.headers {
        d.label_left(
            UiRect::new(origin.x, origin.y, VARIANT_LABEL_W, MATRIX_HEADER_H),
            &label(key),
            p.text_muted,
            11.0,
        );
    }
    // Названия контейнеров (левая колонка)
    for row in &fill.rows {
        d.label_left(
            UiRect::new(row.slot.x, row.slot.y + 4.0, VARIANT_LABEL_W, 16.0),
            &label(row.title_key),
            p.text_muted,
            11.0,
        );
    }

    for row in &fill.rows {
        for cell in &row.cells {
            match row.title_key {
                k if k == crate::i18n::keys::ADMIN_ROW_LIST => {
                    draw_list_demo(d, cell.rect, cell.level, p, lang, &empty_hint);
                }
                k if k == crate::i18n::keys::ADMIN_ROW_CARD => {
                    draw_card_demo(d, cell.rect, cell.level, p, &empty_hint);
                }
                k if k == crate::i18n::keys::ADMIN_ROW_CHIPS => {
                    if cell.level == FillLevel::Empty {
                        draw_empty_hint(
                            d,
                            cell.rect,
                            p,
                            &label(crate::i18n::keys::ADMIN_CHIPS_EMPTY),
                        );
                    }
                }
                k if k == crate::i18n::keys::ADMIN_ROW_DROPDOWN => {
                    let anchor_style =
                        kit::button_style(ButtonVariant::Secondary, KitState::Normal, p);
                    if let Some(dd) = fill.dropdown.iter().find(|dd| dd.level == cell.level) {
                        let (anchor, menu, items, labels) =
                            (&dd.anchor, &dd.menu, &dd.items, &dd.labels);
                        d.control(*anchor, &anchor_style);
                        d.icon(*anchor, "chevron_down", "▼", anchor_style.text, LABEL_SIZE);
                        d.rect(
                            *menu,
                            p.panel_fill,
                            p.control_border,
                            canvas_core::tokens::RADIUS_CHIP,
                        );
                        if items.is_empty() {
                            draw_empty_hint(
                                d,
                                *menu,
                                p,
                                &label(crate::i18n::keys::ADMIN_DROPDOWN_EMPTY),
                            );
                        }
                        for (ir, text) in items.iter().zip(labels.iter()) {
                            d.rect(*ir, [0.0; 4], [0.0; 4], 0.0);
                            d.label_left(
                                UiRect::new(ir.x + 8.0, ir.y + 3.0, ir.w - 16.0, ir.h),
                                text,
                                p.text,
                                12.0,
                            );
                        }
                    }
                }
                k if k == crate::i18n::keys::ADMIN_ROW_ROWS => {
                    if cell.level == FillLevel::Empty {
                        draw_empty_hint(
                            d,
                            cell.rect,
                            p,
                            &label(crate::i18n::keys::ADMIN_ROWS_EMPTY),
                        );
                    }
                }
                k if k == crate::i18n::keys::ADMIN_ROW_TOAST => {
                    draw_toast_demo(d, cell.rect, cell.level, p, lang, &empty_hint);
                }
                k if k == crate::i18n::keys::ADMIN_ROW_PALETTE => {
                    draw_palette_demo(d, cell.rect, cell.level, p, lang, &empty_hint);
                }
                k if k == crate::i18n::keys::ADMIN_ROW_WHEEL => {
                    draw_wheel_demo(
                        d,
                        cell.rect,
                        cell.level,
                        p,
                        &label(crate::i18n::keys::ADMIN_WHEEL_EMPTY),
                    );
                }
                _ => {}
            }
        }
    }

    // Поля (предвычисленные раскладки)
    for (level, fl) in &fill.field_lays {
        d.rect(
            fl.rect,
            p.control_fill,
            p.control_border,
            canvas_core::tokens::RADIUS_CHIP,
        );
        let color = if fl.text_shown.is_empty() {
            p.text_muted
        } else {
            p.text
        };
        d.label_left(fl.text_area, &fl.text_shown, color, LABEL_SIZE);
        let _ = level;
    }
    // Чипы (предвычисленные rect'ы)
    for (_, rect) in &fill.chip_lays {
        let style = kit::chip_style(KitState::Normal, p);
        d.control(*rect, &style);
    }
    // Таблица строк (kit::paint_row — строка целиком одним вызовом)
    for demo in &fill.row_table {
        let mut style = kit::row_style(demo.state, p);
        if demo.zebra {
            style.fill = p.hover_fill;
        }
        let mut painter = canvas_ui::paint::Painter::new();
        kit::paint_row(&mut painter, &demo.lay, &demo.parts, &style, LABEL_SIZE);
        d.paint_items(painter.take_items());
    }
}

/// Демо списка: пустое (заглушка) / 3 строки / 8 строк + скролл-бегунок.
fn draw_list_demo(
    d: &mut crate::kit_ui::KitDraw,
    cell: UiRect,
    level: FillLevel,
    p: &KitPalette,
    lang: Language,
    empty_hint: &str,
) {
    d.rect(
        cell,
        p.control_fill,
        p.control_border,
        canvas_core::tokens::RADIUS_CHIP,
    );
    match level {
        FillLevel::Empty => {
            draw_empty_hint(d, cell, p, empty_hint);
        }
        FillLevel::Medium => {
            for i in 0..3 {
                let r = UiRect::new(
                    cell.x + 4.0,
                    cell.y + 4.0 + i as f32 * (kit::LIST_ROW_H + kit::LIST_ROW_GAP),
                    cell.w - 8.0,
                    kit::LIST_ROW_H,
                );
                d.rect(r, p.hover_fill, [0.0; 4], canvas_core::tokens::RADIUS_CHIP);
                let text = crate::i18n::trf(
                    lang,
                    crate::i18n::keys::KIT_LIST_ROW,
                    &[("{n}", &(i + 1).to_string())],
                );
                d.label_left(
                    UiRect::new(r.x + 8.0, r.y + 4.0, r.w - 16.0, r.h - 8.0),
                    &text,
                    p.text,
                    12.0,
                );
            }
        }
        FillLevel::Full => {
            for i in 0..3 {
                let r = UiRect::new(
                    cell.x + 4.0,
                    cell.y + 4.0 + i as f32 * (kit::LIST_ROW_H + kit::LIST_ROW_GAP),
                    cell.w - 10.0,
                    kit::LIST_ROW_H,
                );
                d.rect(
                    r,
                    if i == 1 {
                        p.selected_fill
                    } else {
                        p.hover_fill
                    },
                    [0.0; 4],
                    canvas_core::tokens::RADIUS_CHIP,
                );
                let text = crate::i18n::trf(
                    lang,
                    crate::i18n::keys::KIT_LIST_ROW,
                    &[("{n}", &(i + 1).to_string())],
                );
                d.label_left(
                    UiRect::new(r.x + 8.0, r.y + 4.0, r.w - 16.0, r.h - 8.0),
                    &text,
                    p.text,
                    12.0,
                );
            }
            // Скролл-бегунок: контент 8 строк — окно 3
            let knob = UiRect::new(
                cell.right() - 5.0,
                cell.y + 4.0,
                3.0,
                (cell.h - 8.0) * 3.0 / 8.0,
            );
            d.rect(knob, p.control_border, [0.0; 4], 2.0);
        }
    }
}

/// Демо карточки: пустое (хедер + «Нет данных») / 2 строки тела / полное.
fn draw_card_demo(
    d: &mut crate::kit_ui::KitDraw,
    cell: UiRect,
    level: FillLevel,
    p: &KitPalette,
    empty_hint: &str,
) {
    let card = kit::card(
        cell,
        UiVec2::new(cell.w, 70.0),
        UiVec2::new(cell.w, cell.h),
        24.0,
        p,
    );
    d.rect(
        card.rect,
        p.control_fill,
        p.control_border,
        canvas_core::tokens::CARD_CORNER_RADIUS,
    );
    d.rect(
        UiRect::new(card.header.x, card.header.y, card.header.w, card.header.h),
        p.hover_fill,
        [0.0; 4],
        0.0,
    );
    d.label_left(
        UiRect::new(
            card.header.x + 8.0,
            card.header.y + 5.0,
            card.header.w - 16.0,
            16.0,
        ),
        crate::i18n::tr(canvas_core::Language::Ru, crate::i18n::keys::KIT_CARD_TITLE),
        p.text_title,
        12.0,
    );
    match level {
        FillLevel::Empty => {
            draw_empty_hint(d, card.body, p, empty_hint);
        }
        FillLevel::Medium => {
            d.label_left(
                UiRect::new(
                    card.body.x + 8.0,
                    card.body.y + 4.0,
                    card.body.w - 16.0,
                    16.0,
                ),
                "rps = 1000",
                p.text,
                12.0,
            );
            d.label_left(
                UiRect::new(
                    card.body.x + 8.0,
                    card.body.y + 22.0,
                    card.body.w - 16.0,
                    16.0,
                ),
                "latency = 50 ms",
                p.text,
                12.0,
            );
        }
        FillLevel::Full => {
            for (i, line) in [
                "rps = 1000",
                "latency = 50 ms",
                "replicas = 8",
                "cost = $0.01/req",
            ]
            .iter()
            .enumerate()
            {
                d.label_left(
                    UiRect::new(
                        card.body.x + 8.0,
                        card.body.y + 4.0 + i as f32 * 18.0,
                        card.body.w - 16.0,
                        16.0,
                    ),
                    line,
                    p.text,
                    12.0,
                );
            }
            // Результат — полный GFM-результат (слот EDGE_FLOW — расчётное)
            d.rect(
                UiRect::new(
                    card.body.x + 4.0,
                    card.body.bottom() - 20.0,
                    card.body.w - 8.0,
                    18.0,
                ),
                p.selected_fill,
                [0.0; 4],
                canvas_core::tokens::RADIUS_CHIP,
            );
            d.label_center(
                UiRect::new(
                    card.body.x + 4.0,
                    card.body.bottom() - 20.0,
                    card.body.w - 8.0,
                    18.0,
                ),
                "= $600/mo",
                p.text_title,
                12.0,
            );
        }
    }
}

/// Демо тостов: пустое (нет) / короткий / длинный.
fn draw_toast_demo(
    d: &mut crate::kit_ui::KitDraw,
    cell: UiRect,
    level: FillLevel,
    p: &KitPalette,
    lang: Language,
    empty_hint: &str,
) {
    match level {
        FillLevel::Empty => draw_empty_hint(d, cell, p, empty_hint),
        FillLevel::Medium | FillLevel::Full => {
            let key = if level == FillLevel::Medium {
                crate::i18n::keys::ADMIN_TOAST_SHORT
            } else {
                crate::i18n::keys::ADMIN_TOAST_LONG
            };
            let text = crate::i18n::tr(lang, key);
            d.rect(
                cell,
                p.panel_fill,
                p.accent,
                canvas_core::tokens::RADIUS_CHIP,
            );
            d.label_center(cell, text, p.text, 12.0);
        }
    }
}

/// Демо палитры шаблонов: пустое / 3 плитки / 6 плиток (сетка 3×2).
fn draw_palette_demo(
    d: &mut crate::kit_ui::KitDraw,
    cell: UiRect,
    level: FillLevel,
    p: &KitPalette,
    lang: Language,
    empty_hint: &str,
) {
    d.rect(
        cell,
        p.panel_fill,
        p.control_border,
        canvas_core::tokens::RADIUS_PANEL,
    );
    match level {
        FillLevel::Empty => draw_empty_hint(d, cell, p, empty_hint),
        FillLevel::Medium | FillLevel::Full => {
            let count = if level == FillLevel::Medium { 3 } else { 6 };
            let tile_w = (cell.w - 16.0 - 2.0 * 6.0) / 3.0;
            let tile_h = ((cell.h - 16.0 - 6.0) / 2.0).min(34.0);
            for i in 0..count {
                let col = i % 3;
                let row = i / 3;
                let tile = UiRect::new(
                    cell.x + 8.0 + col as f32 * (tile_w + 6.0),
                    cell.y + 8.0 + row as f32 * (tile_h + 6.0),
                    tile_w,
                    tile_h,
                );
                d.rect(
                    tile,
                    p.control_fill,
                    p.control_border,
                    canvas_core::tokens::RADIUS_CHIP,
                );
                // Иконка-плитка (слот palette_tile_fill ≈ hover_fill кита)
                d.rect(
                    UiRect::new(tile.x + 4.0, tile.y + 4.0, tile_h - 8.0, tile_h - 8.0),
                    p.hover_fill,
                    [0.0; 4],
                    canvas_core::tokens::RADIUS_CHIP,
                );
                let text = crate::i18n::trf(
                    lang,
                    crate::i18n::keys::KIT_LIST_ROW,
                    &[("{n}", &(i + 1).to_string())],
                );
                d.label_left(
                    UiRect::new(
                        tile.x + tile_h,
                        tile.y + (tile.h - 16.0) / 2.0,
                        (tile.w - tile_h - 4.0).max(0.0),
                        16.0,
                    ),
                    &text,
                    p.text,
                    11.0,
                );
            }
        }
    }
}

/// Демо wheel-меню (слоты WHEEL_* из design/tokens): хаб + категории +
/// шаблоны; сектора упрощены до квадов (демо слотов, не геометрии).
fn draw_wheel_demo(
    d: &mut crate::kit_ui::KitDraw,
    cell: UiRect,
    level: FillLevel,
    p: &KitPalette,
    empty_hint: &str,
) {
    let hub = UiRect::new(cell.x + cell.w / 2.0 - 14.0, cell.y + 6.0, 28.0, 28.0);
    d.rect(
        hub,
        canvas_core::tokens::WHEEL_HUB_ACTIVE,
        canvas_core::tokens::WHEEL_BORDER,
        14.0,
    );
    match level {
        FillLevel::Empty => {
            d.label_center(
                UiRect::new(cell.x, hub.bottom() + 4.0, cell.w, cell.h - 34.0),
                empty_hint,
                p.text_muted,
                12.0,
            );
        }
        FillLevel::Medium | FillLevel::Full => {
            let cat_w = (cell.w - 16.0 - 2.0 * 6.0) / 3.0;
            for i in 0..3 {
                let cat = UiRect::new(
                    cell.x + 8.0 + i as f32 * (cat_w + 6.0),
                    hub.bottom() + 4.0,
                    cat_w,
                    24.0,
                );
                d.rect(
                    cat,
                    canvas_core::tokens::WHEEL_CATEGORY,
                    canvas_core::tokens::WHEEL_BORDER,
                    canvas_core::tokens::RADIUS_CHIP,
                );
                d.label_center(cat, "A", p.text, 11.0);
                if level == FillLevel::Full {
                    for j in 0..2 {
                        let tpl =
                            UiRect::new(cat.x, cat.bottom() + 3.0 + j as f32 * 17.0, cat.w, 15.0);
                        d.rect(tpl, canvas_core::tokens::WHEEL_TEMPLATE, [0.0; 4], 4.0);
                        d.label_center(tpl, "·", p.text_muted, 10.0);
                    }
                }
            }
        }
    }
}

// === FR-070 этап 3: секция «Канвас» — состояния сущностей (ST4) ============

/// Слоты демо карточки ноды.
pub const NODE_DEMO_W: f32 = 150.0;
pub const NODE_DEMO_H: f32 = 54.0;
/// Толщина линии демо-ребра.
pub const EDGE_DEMO_H: f32 = 3.0;

/// Тело секции «Канвас»: карточка ноды ×4, рёбра ×4, порты ×3 (ST4).
/// Цвета — слоты палитры + примитивы design/tokens (контракт F-8).
#[derive(Debug, Clone, Default)]
pub struct CanvasLayout {
    /// Демо карточки ноды: (ключ i18n, слот, рамка, заливка).
    pub node_demos: Vec<(&'static str, UiRect, [f32; 4], [f32; 4])>,
    /// Демо рёбер: (ключ i18n, зона линии, цвет).
    pub edge_demos: Vec<(&'static str, UiRect, [f32; 4])>,
    /// Демо портов: (ключ i18n, зона, диаметр точки, цвет).
    pub port_demos: Vec<(&'static str, UiRect, f32, [f32; 4])>,
    /// Пересборка поверхностей (волна 2): карточка ноды с табличным телом
    /// (FR-061 хром: направляющие/лидер/зебра/бейдж) — зеркало текущего
    /// рендера шаблонных нод.
    pub node_table: Option<NodeTableDemo>,
    /// Полная высота тела (для скролла).
    pub h: f32,
}

/// Пересборка поверхностей (волна 2): демо табличного тела ноды (FR-061) —
/// карточка (хедер + строки kit-Row на общих направляющих).
#[derive(Debug, Clone)]
pub struct NodeTableDemo {
    /// Карточка (внешний rect) и хедер.
    pub card: UiRect,
    pub header: UiRect,
    /// Строки тела: (данные, состояние, зебра, геометрия kit-Row).
    pub rows: Vec<(kit::RowParts<'static>, KitState, bool, kit::RowLayout)>,
}

/// Размеры демо карточки с табличным телом (хедер CARD_HEADER_HEIGHT +
/// 3 строки LIST_ROW_H + пад 4).
pub const NODE_TABLE_CARD_W: f32 = 250.0;
pub const NODE_TABLE_ROWS: usize = 3;

/// Тело секции «Канвас». `card_fill` — слот заливки карточки из темы
/// (ThemeColors.card_fill, извлекает вызывающий — кит без конкретных цветов).
#[allow(clippy::too_many_arguments)]
pub fn canvas_body(
    demo: UiRect,
    offset: f32,
    p: &KitPalette,
    card_fill: [f32; 4],
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> CanvasLayout {
    let mut lay = CanvasLayout::default();
    let content_x = demo.x + 8.0;
    let top = demo.y - offset;
    let fully = |ry: f32, rh: f32| ry >= top && ry + rh <= demo.bottom();

    // === LAY-W13: вертикальная линейка тела — Column-скелет примитивов
    // (см. модульный док «Column-скелет тел админки»): высота блока =
    // прежний шаг курсора `y +=`; межгрупповые хвосты 8 — отдельные блоки
    // (высота карточки — до раскладки линейки, то же выражение).
    let header_h = canvas_core::tokens::CARD_HEADER_HEIGHT;
    let card_h = header_h + NODE_TABLE_ROWS as f32 * kit::LIST_ROW_H + 8.0;
    let mut items: Vec<MeasuredItem> = Vec::with_capacity(13);
    // Карточки нод (шаг NODE_DEMO_H + 8).
    for _ in 0..4 {
        items.push(block(demo.w, NODE_DEMO_H + 8.0));
    }
    items.push(block(demo.w, 8.0)); // хвост после карточек
                                    // Рёбра: контент 20, шаг 28.
    for _ in 0..4 {
        items.push(block(demo.w, 28.0));
    }
    items.push(block(demo.w, 8.0)); // хвост после рёбер
                                    // Порты: контент 40, шаг 48.
    for _ in 0..3 {
        items.push(block(demo.w, 48.0));
    }
    items.push(block(demo.w, 8.0)); // хвост после портов
                                    // Карточка ноды с табличным телом.
    items.push(block(demo.w, card_h + 8.0));

    let mut ruler = body_ruler(demo, &items, m, fs);
    let node_slots: Vec<UiRect> = (0..4).map(|_| ruler.next().unwrap_or_default()).collect();
    let _after_nodes = ruler.next().unwrap_or_default(); // хвост 8
    let edge_slots: Vec<UiRect> = (0..4).map(|_| ruler.next().unwrap_or_default()).collect();
    let _after_edges = ruler.next().unwrap_or_default(); // хвост 8
    let port_slots: Vec<UiRect> = (0..3).map(|_| ruler.next().unwrap_or_default()).collect();
    let _after_ports = ruler.next().unwrap_or_default(); // хвост 8
    let table_slot = ruler.next().unwrap_or_default();
    // Полная высота тела — низ последнего блока линейки (скролл-контракт
    // FR-059; накопление Column тождественно прежнему курсору).
    let body_h = table_slot.bottom();

    // Карточка ноды: обычная / selected / broken / в группе (ST4)
    for ((key, border, fill), slot) in [
        (
            crate::i18n::keys::ADMIN_NODE_NORMAL,
            p.control_border,
            card_fill,
        ),
        (crate::i18n::keys::ADMIN_NODE_SELECTED, p.accent, card_fill),
        (
            crate::i18n::keys::ADMIN_NODE_BROKEN,
            canvas_core::tokens::BROKEN_BORDER,
            card_fill,
        ),
        (
            crate::i18n::keys::ADMIN_NODE_GROUP,
            p.control_border,
            [
                card_fill[0].max(p.accent[0] * 0.08),
                card_fill[1].max(p.accent[1] * 0.08),
                card_fill[2].max(p.accent[2] * 0.08),
                1.0,
            ],
        ),
    ]
    .into_iter()
    .zip(node_slots)
    {
        let sy = demo.y + slot.y - offset;
        let rect = UiRect::new(content_x, sy, NODE_DEMO_W, NODE_DEMO_H);
        if fully(rect.y, rect.h) {
            lay.node_demos.push((key, rect, border, fill));
        }
    }

    // Рёбра: default / flow / draft / dimmed (α floor 0.35 — ST4)
    let dimmed = [
        canvas_core::tokens::EDGE_DEFAULT[0],
        canvas_core::tokens::EDGE_DEFAULT[1],
        canvas_core::tokens::EDGE_DEFAULT[2],
        0.35,
    ];
    let edge_w = (demo.w - 16.0 - VARIANT_LABEL_W).max(60.0);
    for ((key, color), slot) in [
        (
            crate::i18n::keys::ADMIN_EDGE_DEFAULT,
            canvas_core::tokens::EDGE_DEFAULT,
        ),
        (
            crate::i18n::keys::ADMIN_EDGE_FLOW,
            canvas_core::tokens::EDGE_FLOW,
        ),
        (
            crate::i18n::keys::ADMIN_EDGE_DRAFT,
            canvas_core::tokens::EDGE_DRAFT,
        ),
        (crate::i18n::keys::ADMIN_EDGE_DIMMED, dimmed),
    ]
    .into_iter()
    .zip(edge_slots)
    {
        let sy = demo.y + slot.y - offset;
        let rect = UiRect::new(content_x + VARIANT_LABEL_W, sy, edge_w, 20.0);
        if fully(rect.y, rect.h) {
            lay.edge_demos.push((key, rect, color));
        }
    }

    // Порты: idle (точка 10) / hover (26) / active draft (ST4)
    for ((key, size, color), slot) in [
        (
            crate::i18n::keys::ADMIN_PORT_IDLE,
            10.0,
            canvas_core::tokens::ACCENT,
        ),
        (
            crate::i18n::keys::ADMIN_PORT_HOVER,
            26.0,
            canvas_core::tokens::ACCENT,
        ),
        (
            crate::i18n::keys::ADMIN_PORT_ACTIVE,
            26.0,
            canvas_core::tokens::EDGE_DRAFT,
        ),
    ]
    .into_iter()
    .zip(port_slots)
    {
        let sy = demo.y + slot.y - offset;
        let rect = UiRect::new(content_x + VARIANT_LABEL_W, sy, 40.0, 40.0);
        if fully(rect.y, rect.h) {
            lay.port_demos.push((key, rect, size, color));
        }
    }

    // === Пересборка поверхностей (волна 2): карточка ноды с табличным
    // телом (FR-061) — хедер + 3 строки kit-Row на общих направляющих
    // (Dot/цена, Dot/кол-во + зебра, Glyph ƒ/итого + бейдж) — зеркало
    // текущего рендера шаблонных нод (row_guides/row_layout кита). ===
    {
        let card = UiRect::new(
            content_x,
            demo.y + table_slot.y - offset,
            NODE_TABLE_CARD_W,
            card_h,
        );
        if fully(card.y, card.h) {
            let header = UiRect::new(card.x, card.y, card.w, header_h);
            let specs: [(kit::RowMarker, &str, &str, &str, &str); NODE_TABLE_ROWS] = [
                (
                    kit::RowMarker::Dot,
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_PRICE),
                    "50",
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_PRICE),
                    "",
                ),
                (
                    kit::RowMarker::Dot,
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_QTY),
                    "12",
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_QTY),
                    "",
                ),
                (
                    kit::RowMarker::Glyph("ƒ"),
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_TOTAL),
                    "600",
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_UNIT_MONEY),
                    crate::i18n::tr(lang, crate::i18n::keys::KIT_ROW_BADGE),
                ),
            ];
            let parts: Vec<kit::RowParts<'static>> = specs
                .iter()
                .map(|(marker, label, value, unit, badge)| kit::RowParts {
                    marker: *marker,
                    label,
                    value,
                    unit,
                    badge,
                })
                .collect();
            // Общие направляющие тела карточки (право — внутренний край
            // карточки, зазор — TABLE_GUIDE_GAP, лидер on).
            let inner_right = card.right() - 8.0;
            let rows: Vec<_> = if let Some(rg) = kit::row_guides(
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
                &parts,
                inner_right,
                canvas_core::tokens::TABLE_GUIDE_GAP,
            ) {
                specs
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let slot = UiRect::new(
                            card.x + 4.0,
                            header.bottom() + 4.0 + i as f32 * kit::LIST_ROW_H,
                            card.w - 8.0,
                            kit::LIST_ROW_H,
                        );
                        (
                            parts[i],
                            if i + 1 == NODE_TABLE_ROWS {
                                KitState::Selected
                            } else {
                                KitState::Normal
                            },
                            i == 1,
                            kit::row_layout(
                                m,
                                fs,
                                FONT_FAMILY,
                                LABEL_SIZE,
                                slot,
                                rg,
                                &parts[i],
                                &kit::RowOpts::default(),
                            ),
                        )
                    })
                    .collect()
            } else {
                Vec::new()
            };
            lay.node_table = Some(NodeTableDemo { card, header, rows });
        }
    }

    lay.h = body_h;
    lay
}

/// Отрисовка тела «Канвас».
pub(crate) fn draw_canvas(
    d: &mut crate::kit_ui::KitDraw,
    lay: &CanvasLayout,
    p: &KitPalette,
    lang: Language,
) {
    for (key, rect, border, fill) in &lay.node_demos {
        d.rect(
            *rect,
            *fill,
            *border,
            canvas_core::tokens::CARD_CORNER_RADIUS,
        );
        d.label_left(
            UiRect::new(rect.x + 10.0, rect.y + 6.0, rect.w - 20.0, 16.0),
            crate::i18n::tr(lang, crate::i18n::keys::KIT_CARD_TITLE),
            p.text_title,
            12.0,
        );
        d.label_left(
            UiRect::new(
                rect.right() + 8.0,
                rect.y + 8.0,
                VARIANT_LABEL_W * 2.0,
                16.0,
            ),
            crate::i18n::tr(lang, key),
            p.text_muted,
            12.0,
        );
    }
    for (key, area, color) in &lay.edge_demos {
        d.label_left(
            UiRect::new(
                area.x - VARIANT_LABEL_W,
                area.y + 2.0,
                VARIANT_LABEL_W,
                16.0,
            ),
            crate::i18n::tr(lang, key),
            p.text_muted,
            12.0,
        );
        let line = UiRect::new(
            area.x,
            area.y + area.h / 2.0 - EDGE_DEMO_H / 2.0,
            area.w - 10.0,
            EDGE_DEMO_H,
        );
        d.rect(line, *color, [0.0; 4], 1.0);
        // Стрелка — точка-бусина на конце (диаметр EDGE_DOT ×2)
        let dot = UiRect::new(
            line.right(),
            area.y + area.h / 2.0 - canvas_core::tokens::EDGE_DOT,
            canvas_core::tokens::EDGE_DOT * 2.0,
            canvas_core::tokens::EDGE_DOT * 2.0,
        );
        d.rect(dot, *color, [0.0; 4], canvas_core::tokens::EDGE_DOT);
    }
    for (key, area, size, color) in &lay.port_demos {
        d.label_left(
            UiRect::new(
                area.right() + 8.0,
                area.y + 8.0,
                VARIANT_LABEL_W * 2.0,
                16.0,
            ),
            crate::i18n::tr(lang, key),
            p.text_muted,
            12.0,
        );
        let dot = UiRect::new(area.x, area.y + (40.0 - size) / 2.0, *size, *size);
        d.rect(dot, *color, [0.0; 4], size / 2.0);
    }
    // Пересборка поверхностей (волна 2): карточка с табличным телом
    // (FR-061) — хром карточки + строки paint_row по готовой геометрии.
    if let Some(table) = &lay.node_table {
        d.label_left(
            UiRect::new(
                table.card.right() + 8.0,
                table.card.y + 8.0,
                VARIANT_LABEL_W * 2.0,
                16.0,
            ),
            crate::i18n::tr(lang, crate::i18n::keys::ADMIN_NODE_TABLE),
            p.text_muted,
            12.0,
        );
        d.rect(
            table.card,
            p.control_fill,
            p.control_border,
            canvas_core::tokens::CARD_CORNER_RADIUS,
        );
        d.rect(
            UiRect::new(
                table.header.x,
                table.header.y,
                table.header.w,
                table.header.h,
            ),
            p.hover_fill,
            [0.0; 4],
            0.0,
        );
        d.label_left(
            UiRect::new(
                table.header.x + 8.0,
                table.header.y + 3.0,
                table.header.w - 16.0,
                16.0,
            ),
            crate::i18n::tr(lang, crate::i18n::keys::KIT_CARD_TITLE),
            p.text_title,
            12.0,
        );
        for (parts, state, zebra, lay_row) in &table.rows {
            let mut style = kit::row_style(*state, p);
            if *zebra {
                style.fill = p.hover_fill;
            }
            let mut painter = canvas_ui::paint::Painter::new();
            kit::paint_row(&mut painter, lay_row, parts, &style, LABEL_SIZE);
            d.paint_items(painter.take_items());
        }
    }
}

// === FR-070 этап 3: секция «Токены» — каталог + live-правка слотов =========

/// Слоты KitPalette в порядке каталога (14; индекс = hit-суффикс).
pub const PALETTE_SLOTS: [&str; 14] = [
    "panel_fill",
    "panel_border",
    "control_fill",
    "control_border",
    "control_primary",
    "control_danger",
    "hover_fill",
    "primary_hover_fill",
    "selected_fill",
    "text",
    "text_title",
    "text_muted",
    "disabled_text",
    "accent",
];

/// Цвет слота по индексу каталога.
pub fn slot_color(p: &KitPalette, i: usize) -> [f32; 4] {
    match i {
        0 => p.panel_fill,
        1 => p.panel_border,
        2 => p.control_fill,
        3 => p.control_border,
        4 => p.control_primary,
        5 => p.control_danger,
        6 => p.hover_fill,
        7 => p.primary_hover_fill,
        8 => p.selected_fill,
        9 => p.text,
        10 => p.text_title,
        11 => p.text_muted,
        12 => p.disabled_text,
        _ => p.accent,
    }
}

/// Записать цвет слота по индексу каталога (live-правка).
pub fn set_slot_color(p: &mut KitPalette, i: usize, c: [f32; 4]) {
    match i {
        0 => p.panel_fill = c,
        1 => p.panel_border = c,
        2 => p.control_fill = c,
        3 => p.control_border = c,
        4 => p.control_primary = c,
        5 => p.control_danger = c,
        6 => p.hover_fill = c,
        7 => p.primary_hover_fill = c,
        8 => p.selected_fill = c,
        9 => p.text = c,
        10 => p.text_title = c,
        11 => p.text_muted = c,
        12 => p.disabled_text = c,
        _ => p.accent = c,
    }
}

/// Кандидаты live-правки (курсор по клику; первый — базовый акцент темы).
pub const TOKEN_CANDIDATES: [[f32; 4]; 12] = [
    [0.396, 0.612, 0.969, 1.0], // blue (базовый акцент)
    [0.130, 0.660, 0.550, 1.0], // teal
    [0.290, 0.680, 0.310, 1.0], // green
    [0.960, 0.650, 0.140, 1.0], // amber
    [0.900, 0.280, 0.300, 1.0], // red
    [0.830, 0.270, 0.620, 1.0], // magenta
    [0.550, 0.360, 0.900, 1.0], // purple
    [0.140, 0.660, 0.860, 1.0], // cyan
    [0.950, 0.950, 0.970, 1.0], // near-white
    [0.600, 0.610, 0.640, 1.0], // gray
    [0.230, 0.240, 0.280, 1.0], // dark gray
    [0.080, 0.090, 0.110, 1.0], // near-black
];

/// Следующий кандидат цвета слота по текущему значению (чистая функция —
/// клик по свотчу в каталоге: совпадение с точностью 0.5/255, иначе —
/// первый кандидат).
pub fn cycle_slot(current: [f32; 4]) -> [f32; 4] {
    const EPS: f32 = 0.002;
    TOKEN_CANDIDATES
        .iter()
        .position(|c| {
            c.iter()
                .zip(current.iter())
                .all(|(a, b)| (a - b).abs() <= EPS)
        })
        .map(|pos| TOKEN_CANDIDATES[(pos + 1) % TOKEN_CANDIDATES.len()])
        .unwrap_or(TOKEN_CANDIDATES[0])
}

/// Hex-представление цвета (#rrggbb; альфа — отдельной колонкой).
pub fn hex_of(c: [f32; 4]) -> String {
    let to = |v: f32| ((v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", to(c[0]), to(c[1]), to(c[2]))
}

/// Строка каталога токенов.
#[derive(Debug, Clone)]
pub struct TokenRow {
    /// Индекс слота палитры (Some — интерактивный свотч live-правки).
    pub slot_index: Option<usize>,
    /// Имя (идентификатор слота/токена).
    pub name: &'static str,
    /// Значение (hex / px / мс) — строкой (форматирование на раскладке).
    pub value: String,
    /// Слот строки (контент-координаты, уже сдвинут/отфильтрован).
    pub rect: UiRect,
    /// Свотч (Some — рисуется и пикается).
    pub swatch: Option<UiRect>,
}

/// Группа каталога.
#[derive(Debug, Clone)]
pub struct TokenGroup {
    /// Ключ i18n заголовка.
    pub title_key: &'static str,
    pub rows: Vec<TokenRow>,
}

/// Тело секции «Токены»: слоты палитры (live-правка) + размеры +
/// типографика + движение (read-only).
#[allow(clippy::too_many_arguments)]
pub fn tokens_body(
    demo: UiRect,
    offset: f32,
    p: &KitPalette,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> TokensLayout {
    let _ = lang; // подписи групп/строк — при отрисовке (draw_tokens)
    let mut lay = TokensLayout::default();
    let content_x = demo.x + 8.0;
    let top = demo.y - offset;
    let fully = |ry: f32, rh: f32| ry >= top && ry + rh <= demo.bottom();
    let row_h = 22.0;

    // Группа 1: слоты палитры (live-правка)
    let slot_rows: Vec<TokenRow> = PALETTE_SLOTS
        .iter()
        .enumerate()
        .map(|(i, name)| TokenRow {
            slot_index: Some(i),
            name,
            value: hex_of(slot_color(p, i)),
            rect: UiRect::default(),
            swatch: None,
        })
        .collect();

    // Группа 2: размеры (read-only) — spacing/radius/card из design/tokens
    let dims: Vec<(&'static str, String)> = vec![
        ("spacing.s", format!("{}", canvas_core::tokens::SPACING_S)),
        ("spacing.sm", format!("{}", canvas_core::tokens::SPACING_SM)),
        ("spacing.md", format!("{}", canvas_core::tokens::SPACING_MD)),
        ("spacing.lg", format!("{}", canvas_core::tokens::SPACING_LG)),
        ("spacing.xl", format!("{}", canvas_core::tokens::SPACING_XL)),
        (
            "radius.chip",
            format!("{}", canvas_core::tokens::RADIUS_CHIP),
        ),
        (
            "radius.card",
            format!("{}", canvas_core::tokens::CARD_CORNER_RADIUS),
        ),
        (
            "radius.panel",
            format!("{}", canvas_core::tokens::RADIUS_PANEL),
        ),
        (
            "radius.pill",
            format!("{}", canvas_core::tokens::RADIUS_PILL),
        ),
        (
            "card.header_h",
            format!("{}", canvas_core::tokens::CARD_HEADER_HEIGHT),
        ),
        ("button.h", format!("{}", kit::BUTTON_HEIGHT)),
        ("field.h", format!("{}", kit::TEXT_FIELD_HEIGHT)),
    ];
    let dim_rows = dims
        .into_iter()
        .map(|(name, value)| TokenRow {
            slot_index: None,
            name,
            value,
            rect: UiRect::default(),
            swatch: None,
        })
        .collect();

    // Группа 3: типографика (read-only)
    let typo: Vec<(&'static str, String)> = vec![
        ("title.size", format!("{}", canvas_core::tokens::TYPE_TITLE)),
        ("body.size", format!("{}", canvas_core::tokens::TYPE_BODY)),
        (
            "result.size",
            format!("{}", canvas_core::tokens::TYPE_RESULT),
        ),
        ("badge.size", format!("{}", canvas_core::tokens::TYPE_BADGE)),
    ];
    let typo_rows = typo
        .into_iter()
        .map(|(name, value)| TokenRow {
            slot_index: None,
            name,
            value,
            rect: UiRect::default(),
            swatch: None,
        })
        .collect();

    // Группа 4: движение (read-only, мс)
    let motion: Vec<(&'static str, String)> = vec![
        ("focus_fade_ms", "150".to_owned()),
        ("camera_flight_ms", "300".to_owned()),
        ("tooltip_delay_ms", "500".to_owned()),
        ("result_pulse_ms", "1200".to_owned()),
    ];
    let motion_rows = motion
        .into_iter()
        .map(|(name, value)| TokenRow {
            slot_index: None,
            name,
            value,
            rect: UiRect::default(),
            swatch: None,
        })
        .collect();

    // Группы в порядке сайдбара (данные — без геометрии; раскладка ниже).
    let groups: [(&'static str, Vec<TokenRow>); 4] = [
        (crate::i18n::keys::ADMIN_TOK_GROUP_SLOTS, slot_rows),
        (crate::i18n::keys::ADMIN_TOK_GROUP_DIMS, dim_rows),
        (crate::i18n::keys::ADMIN_TOK_GROUP_TYPE, typo_rows),
        (crate::i18n::keys::ADMIN_TOK_GROUP_MOTION, motion_rows),
    ];

    // === LAY-W13: вертикальная линейка тела — Column-скелет примитивов
    // (см. модульный док «Column-скелет тел админки»): на группу — блок
    // заголовка (шаг MATRIX_HEADER_H + 4), блоки строк (row_h) и хвостовой
    // зазор 10 — прежние шаги курсора push_group.
    let mut items: Vec<MeasuredItem> = Vec::new();
    for (_, rows) in &groups {
        items.push(block(demo.w, MATRIX_HEADER_H + 4.0));
        for _ in rows.iter() {
            items.push(block(demo.w, row_h));
        }
        items.push(block(demo.w, 10.0));
    }
    let mut ruler = body_ruler(demo, &items, m, fs);

    let mut body_h = 0.0f32;
    for (title_key, rows) in groups {
        let title = ruler.next().unwrap_or_default();
        let ty = demo.y + title.y - offset;
        let mut g = TokenGroup {
            title_key,
            rows: Vec::new(),
        };
        if fully(ty, MATRIX_HEADER_H) {
            g.rows.push(TokenRow {
                slot_index: None,
                name: "",
                value: String::new(),
                rect: UiRect::new(content_x, ty, 0.0, MATRIX_HEADER_H),
                swatch: None,
            });
        }
        for mut row in rows {
            let rr = ruler.next().unwrap_or_default();
            let ry = demo.y + rr.y - offset;
            row.rect = UiRect::new(content_x, ry, demo.w - 16.0, row_h);
            row.swatch = row
                .slot_index
                .map(|_| UiRect::new(content_x, ry + 3.0, 16.0, 16.0));
            if fully(row.rect.y, row_h) {
                g.rows.push(row);
            }
        }
        let tail = ruler.next().unwrap_or_default(); // хвостовой зазор группы
        body_h = tail.bottom();
        lay.groups.push(g);
    }

    // Полная высота тела — низ последнего блока линейки (скролл-контракт
    // FR-059; накопление Column тождественно прежнему курсору).
    lay.h = body_h;
    lay
}

/// Тело секции «Токены».
#[derive(Debug, Clone, Default)]
pub struct TokensLayout {
    pub groups: Vec<TokenGroup>,
    /// Полная высота тела (для скролла).
    pub h: f32,
}

/// Отрисовка тела «Токены» (свотчи слотов — реальные текущие значения
/// эффективной палитры: live-правка видна сразу).
pub(crate) fn draw_tokens(
    d: &mut crate::kit_ui::KitDraw,
    lay: &TokensLayout,
    p: &KitPalette,
    lang: Language,
) {
    for group in &lay.groups {
        for row in &group.rows {
            if row.slot_index.is_none() && row.name.is_empty() {
                // Заголовок группы
                d.label_left(
                    UiRect::new(row.rect.x, row.rect.y, row.rect.w.max(200.0), 16.0),
                    crate::i18n::tr(lang, group.title_key),
                    p.text_title,
                    12.0,
                );
                continue;
            }
            if let Some(swatch) = row.swatch {
                let color = row.slot_index.map(|i| slot_color(p, i)).unwrap_or([0.0; 4]);
                d.rect(swatch, color, p.control_border, 4.0);
            }
            d.label_left(
                UiRect::new(row.rect.x + 24.0, row.rect.y + 3.0, 190.0, 16.0),
                row.name,
                p.text,
                12.0,
            );
            d.label_left(
                UiRect::new(row.rect.x + 220.0, row.rect.y + 3.0, 120.0, 16.0),
                &row.value,
                p.text_muted,
                12.0,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit_ui::{color4, cursor_in, new_measurer, KitDraw};

    /// Раскладка детерминирована на канонических вьюпортах (G4-линт:
    /// 1280×800 / 1024×640 / 800×560), панель зажата во вьюпорт.
    #[test]
    fn layout_is_deterministic_and_clamped() {
        let palette = test_palette();
        for vp in [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]] {
            let mut m = new_measurer();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = admin_layout(
                vp,
                AdminSection::Components,
                0.0,
                &palette,
                [0.2, 0.2, 0.25, 1.0],
                Language::Ru,
                &mut m,
                &mut fs,
            );
            let vp_rect = UiRect::new(0.0, 0.0, vp[0], vp[1]);
            assert!(
                vp_rect.contains(UiPoint::new(lay.panel.x, lay.panel.y)),
                "панель выходит за вьюпорт {vp:?}: {:?}",
                lay.panel
            );
            assert!(
                lay.panel.right() <= vp_rect.right() + 0.5
                    && lay.panel.bottom() <= vp_rect.bottom() + 0.5,
                "панель выходит за вьюпорт {vp:?}: {:?}",
                lay.panel
            );
            assert_eq!(lay.sidebar_items.len(), ADMIN_SECTIONS.len());
            // Демо-зона справа от сайдбара, внутри панели.
            assert!(lay.demo.x >= lay.sidebar.right());
            assert!(lay.panel.contains(UiPoint::new(lay.demo.x, lay.demo.y)));
            // Слоты шапки не пересекаются.
            assert!(lay.reset.right() <= lay.theme.x);
            assert!(lay.theme.right() <= lay.close.x);
        }
    }

    /// Секции: индексы и обратное восстановление — полный цикл.
    #[test]
    fn sections_roundtrip() {
        for (i, sec) in ADMIN_SECTIONS.iter().enumerate() {
            assert_eq!(sec.index(), i);
            assert_eq!(AdminSection::at(i), Some(*sec));
        }
        assert_eq!(AdminSection::at(ADMIN_SECTIONS.len()), None);
        // Ключи подписей различны.
        let keys: Vec<&'static str> = ADMIN_SECTIONS.iter().map(|s| s.label_key()).collect();
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j]);
            }
        }
    }

    /// Hit-слоты шапки совпадают с полной раскладкой (один источник
    /// геометрии — контракт реестра FR-052 U2).
    #[test]
    fn hit_slots_match_full_layout() {
        let palette = test_palette();
        for vp in [[1280.0, 800.0], [800.0, 560.0]] {
            let mut m = new_measurer();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = admin_layout(
                vp,
                AdminSection::Tokens,
                0.0,
                &palette,
                [0.2, 0.2, 0.25, 1.0],
                Language::Ru,
                &mut m,
                &mut fs,
            );
            let (theme, reset, close) = admin_hit_slots(vp, &palette);
            assert_eq!(theme, lay.theme);
            assert_eq!(reset, lay.reset);
            assert_eq!(close, lay.close);
            let demo = admin_demo_viewport(vp);
            assert_eq!(demo, lay.demo);
        }
    }

    /// KitDraw-адаптер рисует каркас: панель, шапку, пункты сайдбара,
    /// рамку демо-зоны — quads/texts непусты и в screen-координатах.
    #[test]
    fn frame_assembly_smoke() {
        let palette = test_palette();
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let lay = admin_layout(
            [1280.0, 800.0],
            AdminSection::Fill,
            0.0,
            &palette,
            [0.2, 0.2, 0.25, 1.0],
            Language::Ru,
            &mut m,
            &mut fs,
        );
        let mut d = KitDraw::new();
        // Панель + рамка демо-зоны + пункт сайдбара + подписи.
        d.rect(lay.panel, palette.panel_fill, palette.panel_border, 10.0);
        d.rect(
            lay.demo,
            [0.0; 4],
            palette.control_border,
            canvas_core::tokens::RADIUS_PANEL,
        );
        for (i, item) in lay.sidebar_items.iter().enumerate() {
            let state = if i == 1 {
                kit::KitState::Selected
            } else {
                kit::KitState::Normal
            };
            let style = kit::button_style(kit::ButtonVariant::Secondary, state, &palette);
            d.control(*item, &style);
            d.label_center(*item, "X", style.text, LABEL_SIZE);
        }
        d.label_left(
            UiRect::new(
                lay.section_title.0.x,
                lay.section_title.0.y,
                lay.demo.w,
                18.0,
            ),
            "Секция",
            palette.text_title,
            LABEL_SIZE,
        );
        assert!(!d.quads.is_empty(), "кадр без квадов");
        assert!(!d.texts.is_empty(), "кадр без текстов");
        // Все квады — валидные размеры (screen-пространство вьюпорта).
        for q in &d.quads {
            assert!(q.size[0] >= 0.0 && q.size[1] >= 0.0);
        }
        let _ = color4([0.0, 0.0, 0.0, 1.0]);
        let _ = cursor_in(&lay.demo, [lay.demo.x + 1.0, lay.demo.y + 1.0]);
    }

    /// Перенос по словам: строки не шире слота (кроме одиночного длинного
    /// слова), порядок слов сохранён, пустой ввод — одна пустая строка.
    #[test]
    fn wrap_text_breaks_by_words() {
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let text = "короткий текст переносится по словам при превышении ширины слота";
        let lines = wrap_text(&mut m, &mut fs, text, 140.0, LABEL_SIZE);
        assert!(lines.len() >= 2, "ожидается перенос: {lines:?}");
        // Слова не теряются (сумма слов строк = число слов исходного текста).
        let words_in_lines: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
        assert_eq!(words_in_lines, text.split_whitespace().count());
        for line in &lines {
            let w = m.width_of(&mut fs, line, FONT_FAMILY, LABEL_SIZE);
            assert!(
                w <= 140.0 + 0.5 || line.split_whitespace().count() == 1,
                "строка шире слота и не одиночное слово: {line:?} ({w})"
            );
        }
        assert_eq!(
            wrap_text(&mut m, &mut fs, "", 140.0, LABEL_SIZE),
            vec![String::new()]
        );
        assert_eq!(
            wrap_text(&mut m, &mut fs, "одно", 140.0, LABEL_SIZE).len(),
            1
        );
    }

    /// Тело «Компоненты»: заголовки колонок, 4 ряда кнопок × 6 состояний,
    /// икон-кнопки, чипы, поля (4 демо), переключатели — на 1280×800 всё
    /// видно целиком (offset 0).
    #[test]
    fn components_body_full_matrix() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let body = components_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        assert!(
            body.headers.len() >= 10,
            "6 состояний + 4 подписи полей (хвост матрицы — по видимости): {}",
            body.headers.len()
        );
        assert_eq!(body.button_rows.len(), 4);
        for row in &body.button_rows {
            assert_eq!(row.cells.len(), 6, "5 из ST1 + Focused");
            let focused = row.cells.iter().filter(|c| c.focused).count();
            assert_eq!(focused, 1, "одна Focused-ячейка в ряду");
        }
        assert_eq!(body.icon_cells.len(), 5, "ST1 без Focused");
        assert_eq!(body.chip_cells.len(), 6);
        assert_eq!(body.fields.len(), 4, "Normal/Focused/Error/Disabled");
        assert_eq!(body.switches.len(), 6, "4 прежних + Pressed + фокус");
        // Каретка видна только у Focused-поля
        let caret_fields = body
            .fields
            .iter()
            .filter(|(_, fl)| fl.caret_x >= 0.0)
            .count();
        assert_eq!(caret_fields, 1);
    }

    /// Пересборка поверхностей (волна 2): хвост матрицы состояний —
    /// dropdown закрыт/открыт, tooltip, toast, строки списков в состояниях
    /// и бегунок, строки таблицы (зебра/фокус/выбор). На синтетическом
    /// высоком окне всё видно при offset 0.
    #[test]
    fn components_body_extended_matrix() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let big = UiRect::new(0.0, 0.0, demo.w, 2400.0);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let body = components_body(big, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        // Popup-контролы
        assert!(body.dropdown_closed.is_some(), "закрытый dropdown");
        let dd = body.dropdown_open.expect("открытый dropdown");
        assert_eq!(dd.items.len(), 3, "3 видимых пункта");
        assert_eq!(dd.items.len(), dd.labels.len());
        assert!(dd.menu.bottom() >= dd.anchor.bottom(), "меню под якорем");
        let (tip_anchor, tip_bubble) = body.tooltip.expect("tooltip-демо");
        assert!(tip_bubble.w > 0.0, "пузырь показан (delay пройден)");
        assert!(tip_bubble.x >= tip_anchor.right() - 1.0, "пузырь у якоря");
        assert!(body.toast.is_some(), "toast-хром");
        // Строки списка: Normal/Hovered/Selected + бегунок
        let states: Vec<KitState> = body.list_state_rows.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            states,
            vec![KitState::Normal, KitState::Hovered, KitState::Selected]
        );
        assert!(body.list_scroll_knob.is_some(), "бегунок скролла списка");
        // Строки таблицы: зебра / фокус / выбор; значения на направляющей
        assert_eq!(body.row_state_rows.len(), 3);
        assert!(body.row_state_rows[0].zebra, "первая строка — зебра");
        assert!(body.row_state_rows[1].focused, "вторая — в фокусе");
        assert_eq!(
            body.row_state_rows[2].state,
            KitState::Selected,
            "Σ — выбор"
        );
        assert!(
            body.row_state_rows[2].lay.badge.is_some(),
            "бейдж у Σ-строки"
        );
        let value_right = body.row_state_rows[0].lay.value.right();
        assert!(
            body.row_state_rows.iter().all(|r| {
                r.lay.value.w == 0.0 || (r.lay.value.right() - value_right).abs() < 0.01
            }),
            "значения строк — на колоночной направляющей (D-4)"
        );
        // Заголовки хвостовых групп присутствуют
        let header_keys: Vec<&str> = body.headers.iter().map(|(_, k)| *k).collect();
        for key in [
            crate::i18n::keys::ADMIN_MATRIX_DROPDOWN,
            crate::i18n::keys::ADMIN_MATRIX_TOOLTIP,
            crate::i18n::keys::ADMIN_MATRIX_TOAST,
            crate::i18n::keys::ADMIN_MATRIX_LIST,
            crate::i18n::keys::ADMIN_MATRIX_ROWS,
        ] {
            assert!(header_keys.contains(&key), "нет заголовка {key}");
        }
    }

    /// Пересборка поверхностей (волна 2): скролл хвоста матрицы — popup-
    /// контролы и строки за окном отфильтрованы целиком (контракт
    /// полной видимости).
    #[test]
    fn components_body_extended_filters_by_scroll() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let big = UiRect::new(0.0, 0.0, demo.w, 2400.0);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let full = components_body(big, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        let shifted = components_body(big, full.h, &palette, Language::Ru, &mut m, &mut fs);
        assert!(shifted.dropdown_closed.is_none());
        assert!(shifted.dropdown_open.is_none());
        assert!(shifted.tooltip.is_none());
        assert!(shifted.toast.is_none());
        assert!(shifted.list_state_rows.is_empty());
        assert!(shifted.list_scroll_knob.is_none());
        assert!(shifted.row_state_rows.is_empty());
    }

    /// Скролл тела: при большом offset ячейки за окном демо-зоны
    /// отфильтрованы (контракт видимости витрины FR-059).
    #[test]
    fn components_body_filters_by_scroll() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let full = components_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        let shifted = components_body(demo, full.h, &palette, Language::Ru, &mut m, &mut fs);
        assert!(shifted.button_rows.is_empty(), "весь контент выше окна");
        assert!(shifted.fields.is_empty());
    }

    /// Тело «Наполнение»: 9 контейнеров × 3 уровня, все демо-геометрии
    /// предвычислены (поля/чипы/dropdown/таблица).
    #[test]
    fn fill_body_all_containers() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let body = fill_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        assert_eq!(body.headers.len(), 3, "empty/medium/full");
        assert!(
            body.rows.len() >= 5,
            "первый экран: часть рядов, остальные — скроллом"
        );
        // Полный состав 9 контейнеров — на синтетическом высоком окне
        // (fill_body — чистая функция от окна, вьюпорт панели не обязан)
        let big = UiRect::new(0.0, 0.0, demo.w, 1200.0);
        let all = fill_body(big, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        assert_eq!(all.rows.len(), 9, "9 контейнеров");
        for row in &all.rows {
            assert_eq!(row.cells.len(), 3, "все уровни видны при offset 0");
        }
        assert_eq!(all.field_lays.len(), 3);
        // Среднее — 2 чипа, полное — 5
        assert_eq!(
            all.chip_lays
                .iter()
                .filter(|(l, _)| *l == FillLevel::Medium)
                .count(),
            2
        );
        assert_eq!(
            all.chip_lays
                .iter()
                .filter(|(l, _)| *l == FillLevel::Full)
                .count(),
            5
        );
        assert_eq!(all.dropdown.len(), 3);
        let full_dd = all
            .dropdown
            .iter()
            .find(|dd| dd.level == FillLevel::Full)
            .expect("dropdown full");
        assert_eq!(
            full_dd.items.len(),
            4,
            "видимых пунктов не больше окна меню"
        );
        // Таблица: среднее 2 строки, полное 4 строки
        let med = all
            .row_table
            .iter()
            .filter(|d| d.level == FillLevel::Medium)
            .count();
        let full_rows = all
            .row_table
            .iter()
            .filter(|d| d.level == FillLevel::Full)
            .count();
        assert_eq!(med, 2);
        assert_eq!(full_rows, 4);
        // Σ в полном ряду — Selected
        assert!(all
            .row_table
            .iter()
            .any(|d| d.level == FillLevel::Full && d.state == KitState::Selected));
        // FR-068 W3 (M4): геометрия — компонент Table (set_rows +
        // row_layout_with): общие направляющие уровня сохранены — значения
        // и юниты всех строк полного ряда на одних осях (проход A/B §4.2)
        let full_lays: Vec<&FillTableDemo> = all
            .row_table
            .iter()
            .filter(|d| d.level == FillLevel::Full)
            .collect();
        let value_right = full_lays[0].lay.value.right();
        let unit_right = full_lays[0].lay.unit.right();
        assert!(
            full_lays
                .iter()
                .all(|d| (d.lay.value.right() - value_right).abs() < 0.01),
            "значения полного ряда — на колоночной направляющей (D-4)"
        );
        assert!(
            full_lays
                .iter()
                .all(|d| (d.lay.unit.right() - unit_right).abs() < 0.01),
            "юниты полного ряда — на направляющей юнитов"
        );
        // Зебра-override демо (нечётные строки) — как прежде
        assert!(full_lays
            .iter()
            .enumerate()
            .all(|(i, d)| d.zebra == (i % 2 == 1)));
    }

    /// Отрисовка тел непуста (smoke: quads/texts от draw_components/draw_fill).
    #[test]
    fn draw_bodies_smoke() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let comp = components_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        let mut d = KitDraw::new();
        draw_components(&mut d, &comp, &palette, Language::Ru);
        assert!(d.quads.len() > 20 && d.texts.len() > 20);
        let fill = fill_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        let mut d2 = KitDraw::new();
        draw_fill(&mut d2, &fill, &palette, Language::Ru);
        assert!(d2.quads.len() > 30 && d2.texts.len() > 10);
    }

    /// Тело «Канвас»: 4 карточки-состояния, 4 ребра, 3 порта (ST4);
    /// цвета рёбер — примитивы EDGE_*, dimmed — α 0.35.
    #[test]
    fn canvas_body_st4_states() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let body = canvas_body(
            demo,
            0.0,
            &palette,
            [0.2, 0.2, 0.25, 1.0],
            Language::Ru,
            &mut m,
            &mut fs,
        );
        assert_eq!(body.node_demos.len(), 4, "normal/selected/broken/group");
        // Selected — рамка accent
        assert_eq!(body.node_demos[1].2, palette.accent);
        // Broken — примитив BROKEN_BORDER
        assert_eq!(body.node_demos[2].2, canvas_core::tokens::BROKEN_BORDER);
        assert_eq!(body.edge_demos.len(), 4);
        assert_eq!(body.edge_demos[0].2, canvas_core::tokens::EDGE_DEFAULT);
        assert_eq!(body.edge_demos[1].2, canvas_core::tokens::EDGE_FLOW);
        assert_eq!(body.edge_demos[2].2, canvas_core::tokens::EDGE_DRAFT);
        // Dimmed — альфа 0.35
        assert!((body.edge_demos[3].2[3] - 0.35).abs() < 0.001);
        assert_eq!(body.port_demos.len(), 3, "idle/hover/active");
        // Порты: точки 10 → 26
        assert_eq!(body.port_demos[0].2, 10.0);
        assert_eq!(body.port_demos[1].2, 26.0);
    }

    /// Пересборка поверхностей (волна 2): демо табличного тела ноды
    /// (FR-061) — карточка с хедером, 3 строки на общих направляющих,
    /// зебра + бейдж у Σ-строки; строки внутри карточки.
    #[test]
    fn canvas_body_node_table_demo() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let body = canvas_body(
            demo,
            0.0,
            &palette,
            [0.2, 0.2, 0.25, 1.0],
            Language::Ru,
            &mut m,
            &mut fs,
        );
        let table = body.node_table.expect("демо табличного тела построено");
        assert_eq!(table.rows.len(), NODE_TABLE_ROWS);
        // Хедер — верх карточки, строки — под ним и внутри карточки.
        assert_eq!(table.header.y, table.card.y);
        assert!(table
            .rows
            .iter()
            .all(|(.., lay)| lay.row.x >= table.card.x
                && lay.row.right() <= table.card.right() + 0.01));
        // Общие направляющие: значения всех строк — на одной оси (D-4).
        let value_right = table.rows[0].3.value.right();
        assert!(table
            .rows
            .iter()
            .all(|(.., lay)| lay.value.w == 0.0 || (lay.value.right() - value_right).abs() < 0.01));
        // Зебра (вторая) + бейдж/Selected (третья).
        assert!(table.rows[1].2, "вторая строка — зебра");
        assert!(table.rows[2].1 == KitState::Selected, "Σ — выбор");
        assert!(!table.rows[2].0.badge.is_empty(), "у Σ — бейдж-демо");
    }

    /// Пересборка поверхностей (волна 2): новые ключи i18n админпанели
    /// существуют в ОБОИХ языках.
    #[test]
    fn extended_admin_i18n_keys_exist() {
        let keys = [
            crate::i18n::keys::ADMIN_MATRIX_DROPDOWN,
            crate::i18n::keys::ADMIN_MATRIX_TOOLTIP,
            crate::i18n::keys::ADMIN_MATRIX_TOAST,
            crate::i18n::keys::ADMIN_MATRIX_LIST,
            crate::i18n::keys::ADMIN_MATRIX_ROWS,
            crate::i18n::keys::ADMIN_NODE_TABLE,
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

    /// Тело «Токены»: 4 группы; группа слотов — 14 интерактивных свотчей;
    /// read-only группы — размеры/типографика/движение.
    #[test]
    fn tokens_body_catalog() {
        let palette = test_palette();
        let demo = admin_demo_viewport([1280.0, 800.0]);
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let big = UiRect::new(0.0, 0.0, demo.w, 1600.0);
        let body = tokens_body(big, 0.0, &palette, Language::Ru, &mut m, &mut fs);
        assert_eq!(body.groups.len(), 4);
        let slots = &body.groups[0];
        let slot_rows: Vec<&TokenRow> = slots
            .rows
            .iter()
            .filter(|r| r.slot_index.is_some())
            .collect();
        assert_eq!(slot_rows.len(), PALETTE_SLOTS.len(), "все 14 слотов");
        assert!(slot_rows.iter().all(|r| r.swatch.is_some()));
        // Значение — hex текущего слота
        assert_eq!(slot_rows[13].value, hex_of(palette.accent));
        // Размеры: 12 строк, типографика 4, движение 4 (плюс заголовок-
        // псевдострока группы — строки данных без пустого имени)
        let data_rows = |g: &TokenGroup| g.rows.iter().filter(|r| !r.name.is_empty()).count();
        assert_eq!(data_rows(&body.groups[1]), 12);
        assert_eq!(data_rows(&body.groups[2]), 4);
        assert_eq!(data_rows(&body.groups[3]), 4);
        // У группы слотов псевдостроки нет данных — все строки слоты
        assert_eq!(
            body.groups[0]
                .rows
                .iter()
                .filter(|r| !r.name.is_empty())
                .count(),
            14
        );
    }

    /// Live-правка: cycle_slot идёт по кандидатам по кругу; неизвестный
    /// цвет → первый кандидат; hex_of — корректный формат.
    #[test]
    fn token_edit_cycles_candidates() {
        let first = TOKEN_CANDIDATES[0];
        let second = cycle_slot(first);
        assert_eq!(second, TOKEN_CANDIDATES[1]);
        // Полный круг
        let mut cur = first;
        for _ in 0..TOKEN_CANDIDATES.len() {
            cur = cycle_slot(cur);
        }
        assert_eq!(cur, first);
        // Неизвестный цвет — первый кандидат
        assert_eq!(cycle_slot([0.123, 0.456, 0.789, 1.0]), TOKEN_CANDIDATES[0]);
        // hex
        assert_eq!(hex_of([1.0, 0.0, 0.0, 0.5]), "#ff0000");
        assert_eq!(hex_of([0.0, 1.0, 0.0, 1.0]), "#00ff00");
    }

    /// slot_color/set_slot_color: полный цикл по каталогу — запись читается.
    #[test]
    fn slot_accessors_roundtrip() {
        let mut p = test_palette();
        for i in 0..PALETTE_SLOTS.len() {
            let c = [0.1 * i as f32, 0.2, 0.3, 1.0];
            set_slot_color(&mut p, i, c);
            assert_eq!(slot_color(&p, i), c);
        }
    }

    /// Фикс налезания 2026-09-25 (wasm-аудит 19_admin): тело секции
    /// строится ПОД подсказкой — верх матрицы ниже низа последней строки
    /// hint'а, налезания текста на колонки/кнопки нет (3 вьюпорта × RU/EN).
    #[test]
    fn components_body_starts_below_hint() {
        for vp in [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]] {
            for lang in [Language::Ru, Language::En] {
                let mut m = new_measurer();
                let mut fs = canvas_render::text::measure_font_system();
                let lay = admin_layout(
                    vp,
                    AdminSection::Components,
                    0.0,
                    &test_palette(),
                    [0.2, 0.2, 0.25, 1.0],
                    lang,
                    &mut m,
                    &mut fs,
                );
                let Some(comp) = &lay.components else {
                    panic!("нет тела компонентов при {vp:?}/{lang:?}");
                };
                // Низ hint'а — формула рисования: demo.y + 24 + n·18.
                let hint_bottom = lay.demo.y + 24.0 + lay.hint_lines.len() as f32 * 18.0;
                let first_header_y = comp
                    .headers
                    .iter()
                    .map(|(p, _)| p.y)
                    .fold(f32::INFINITY, f32::min);
                assert!(
                    first_header_y >= hint_bottom - 0.01,
                    "{vp:?}/{lang:?}: заголовок колонки {first_header_y} выше низа подсказки {hint_bottom}"
                );
            }
        }
    }

    /// Тестовая палитра (значения не важны — важны различные слоты).
    fn test_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.1, 0.1, 0.12, 0.97],
            panel_border: [0.3, 0.3, 0.35, 1.0],
            control_fill: [0.2, 0.2, 0.24, 1.0],
            control_border: [0.35, 0.4, 0.5, 1.0],
            control_primary: [0.16, 0.32, 0.6, 1.0],
            control_danger: [0.8, 0.2, 0.2, 1.0],
            hover_fill: [0.25, 0.25, 0.3, 1.0],
            primary_hover_fill: [0.25, 0.46, 0.84, 1.0],
            selected_fill: [0.3, 0.3, 0.4, 1.0],
            text: [0.9, 0.9, 0.95, 1.0],
            text_title: [0.95, 0.95, 1.0, 1.0],
            text_muted: [0.7, 0.7, 0.75, 1.0],
            disabled_text: [0.5, 0.5, 0.55, 1.0],
            accent: [0.396, 0.612, 0.969, 1.0],
            // FR-070 (волна W-d): новые слоты — не используются в тестах
            // admin_ui (геометрия галереи), но значения берём из
            // `KitPalette::dark()` (Agent B) — чтобы test fixture не расходился
            // с production-коридором (single source of truth).
            control_success: [0.30, 0.75, 0.55, 1.0],
            control_warning: [0.95, 0.65, 0.30, 1.0],
            stage_dim: [0.02, 0.02, 0.04, 0.6],
            scrollbar_thumb: [0.35, 0.38, 0.46, 0.7],
            rule_color: [0.30, 0.33, 0.40, 0.8],
        }
    }

    /// LAY-W3a: золотая геометрия тел демо-секций админки — инвариант
    /// рефакторинга «ручной курсор `y +=` → Column-скелет примитивов»
    /// (аудит layouts-2026-10 §3.2/§5 LAY-W3). Y-координаты всех элементов
    /// сняты с ПРЕЖНЕГО кода пробом (синтетическое окно 800×2400, offset 0,
    /// RU) и пинены с допуском 0.005 px. Позиционные теги (индексы —
    /// порядок векторов раскладки).
    #[test]
    fn admin_bodies_golden_column_skeleton() {
        const TOL: f32 = 0.005;
        let expected: &[(&str, f32)] = &[
            ("components.h", 982.0),
            ("c.header.0", 60.0),
            ("c.header.1", 60.0),
            ("c.header.2", 60.0),
            ("c.header.3", 60.0),
            ("c.header.4", 60.0),
            ("c.header.5", 60.0),
            ("c.header.6", 312.0),
            ("c.header.7", 364.0),
            ("c.header.8", 416.0),
            ("c.header.9", 468.0),
            ("c.header.10", 560.0),
            ("c.header.11", 706.0),
            ("c.header.12", 760.0),
            ("c.header.13", 812.0),
            ("c.header.14", 930.0),
            ("c.btnrow.0", 82.0),
            ("c.btnrow.1", 120.0),
            ("c.btnrow.2", 158.0),
            ("c.btnrow.3", 196.0),
            ("c.icon.0", 240.0),
            ("c.icon.1", 240.0),
            ("c.icon.2", 240.0),
            ("c.icon.3", 240.0),
            ("c.icon.4", 240.0),
            ("c.chip.0", 276.0),
            ("c.chip.1", 276.0),
            ("c.chip.2", 276.0),
            ("c.chip.3", 276.0),
            ("c.chip.4", 276.0),
            ("c.chip.5", 276.0),
            ("c.field.0", 328.0),
            ("c.field.1", 380.0),
            ("c.field.2", 432.0),
            ("c.field.3", 484.0),
            ("c.switch.0", 526.0),
            ("c.switch.1", 526.0),
            ("c.switch.2", 526.0),
            ("c.switch.3", 526.0),
            ("c.switch.4", 526.0),
            ("c.switch.5", 526.0),
            ("c.dd_closed", 576.0),
            ("c.dd_open.anchor", 576.0),
            ("c.dd_open.menu", 610.0),
            ("c.tip.anchor", 722.0),
            ("c.tip.bubble", 744.0),
            ("c.toast", 776.0),
            ("f.header.0", 60.0),
            ("f.header.1", 60.0),
            ("f.header.2", 60.0),
            ("f.row.0", 82.0),
            ("f.row.1", 184.0),
            ("f.row.2", 286.0),
            ("f.row.3", 354.0),
            ("f.row.4", 414.0),
            ("f.row.5", 590.0),
            ("f.row.6", 730.0),
            ("f.row.7", 788.0),
            ("f.row.8", 890.0),
            ("f.field.0", 302.0),
            ("f.field.1", 302.0),
            ("f.field.2", 302.0),
            ("f.chip.0", 354.0),
            ("f.chip.1", 354.0),
            ("f.chip.2", 354.0),
            ("f.chip.3", 354.0),
            ("f.chip.4", 354.0),
            ("f.chip.5", 354.0),
            ("f.chip.6", 354.0),
            ("f.dd.0.anchor", 414.0),
            ("f.dd.0.menu", 448.0),
            ("f.dd.1.anchor", 414.0),
            ("f.dd.1.menu", 448.0),
            ("f.dd.2.anchor", 414.0),
            ("f.dd.2.menu", 448.0),
            ("f.table.0", 590.0),
            ("f.table.1", 618.0),
            ("f.table.2", 590.0),
            ("f.table.3", 618.0),
            ("f.table.4", 646.0),
            ("f.table.5", 674.0),
            ("c2.node.0", 60.0),
            ("c2.node.1", 122.0),
            ("c2.node.2", 184.0),
            ("c2.node.3", 246.0),
            ("c2.edge.0", 316.0),
            ("c2.edge.1", 344.0),
            ("c2.edge.2", 372.0),
            ("c2.edge.3", 400.0),
            ("c2.port.0", 436.0),
            ("c2.port.1", 484.0),
            ("c2.port.2", 532.0),
            ("c2.table.card", 588.0),
            ("c2.table.header", 588.0),
            ("t.row.0", 60.0),
            ("t.row.1", 80.0),
            ("t.row.2", 102.0),
            ("t.row.3", 124.0),
            ("t.row.4", 146.0),
            ("t.row.5", 168.0),
            ("t.row.6", 190.0),
            ("t.row.7", 212.0),
            ("t.row.8", 234.0),
            ("t.row.9", 256.0),
            ("t.row.10", 278.0),
            ("t.row.11", 300.0),
            ("t.row.12", 322.0),
            ("t.row.13", 344.0),
            ("t.row.14", 366.0),
            ("t.row.15", 398.0),
            ("t.row.16", 418.0),
            ("t.row.17", 440.0),
            ("t.row.18", 462.0),
            ("t.row.19", 484.0),
            ("t.row.20", 506.0),
            ("t.row.21", 528.0),
            ("t.row.22", 550.0),
            ("t.row.23", 572.0),
            ("t.row.24", 594.0),
            ("t.row.25", 616.0),
            ("t.row.26", 638.0),
            ("t.row.27", 660.0),
            ("t.row.28", 692.0),
            ("t.row.29", 712.0),
            ("t.row.30", 734.0),
            ("t.row.31", 756.0),
            ("t.row.32", 778.0),
            ("t.row.33", 810.0),
            ("t.row.34", 830.0),
            ("t.row.35", 852.0),
            ("t.row.36", 874.0),
            ("t.row.37", 896.0),
            ("fill.h", 932.0),
            ("canvas.h", 656.0),
            ("tokens.h", 868.0),
        ];
        let palette = test_palette();
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut check = |tag: &str, val: f32| {
            if seen.insert(tag.to_owned()) {
                let (_, want) = expected
                    .iter()
                    .find(|(t, _)| *t == tag)
                    .unwrap_or_else(|| panic!("золотой тег без ожидаемого значения: {tag}"));
                assert!(
                    (val - want).abs() < TOL,
                    "дрейф тел админки {tag}: {val} != {want}"
                );
            }
        };
        // --- components_body ---
        {
            let demo = UiRect::new(40.0, 60.0, 800.0, 2400.0);
            let b = components_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
            check("components.h", b.h);
            for (i, (p, _)) in b.headers.iter().enumerate() {
                check(&format!("c.header.{i}"), p.y);
            }
            for (i, r) in b.button_rows.iter().enumerate() {
                check(&format!("c.btnrow.{i}"), r.slot.y);
            }
            for (i, c) in b.icon_cells.iter().enumerate() {
                check(&format!("c.icon.{i}"), c.rect.y);
            }
            for (i, c) in b.chip_cells.iter().enumerate() {
                check(&format!("c.chip.{i}"), c.rect.y);
            }
            for (i, (_, fl)) in b.fields.iter().enumerate() {
                check(&format!("c.field.{i}"), fl.rect.y);
            }
            for (i, (_, _, r, _)) in b.switches.iter().enumerate() {
                check(&format!("c.switch.{i}"), r.y);
            }
            if let Some(a) = &b.dropdown_closed {
                check("c.dd_closed", a.y);
            }
            if let Some(dd) = &b.dropdown_open {
                check("c.dd_open.anchor", dd.anchor.y);
                check("c.dd_open.menu", dd.menu.y);
            }
            if let Some((a, t)) = &b.tooltip {
                check("c.tip.anchor", a.y);
                check("c.tip.bubble", t.y);
            }
            if let Some(t) = &b.toast {
                check("c.toast", t.y);
            }
        }
        // --- fill_body ---
        {
            let demo = UiRect::new(40.0, 60.0, 800.0, 2400.0);
            let b = fill_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
            check("fill.h", b.h);
            for (i, (p, _)) in b.headers.iter().enumerate() {
                check(&format!("f.header.{i}"), p.y);
            }
            for (i, r) in b.rows.iter().enumerate() {
                check(&format!("f.row.{i}"), r.slot.y);
            }
            for (i, (_, fl)) in b.field_lays.iter().enumerate() {
                check(&format!("f.field.{i}"), fl.rect.y);
            }
            for (i, (_, r)) in b.chip_lays.iter().enumerate() {
                check(&format!("f.chip.{i}"), r.y);
            }
            for (i, d) in b.dropdown.iter().enumerate() {
                check(&format!("f.dd.{i}.anchor"), d.anchor.y);
                check(&format!("f.dd.{i}.menu"), d.menu.y);
            }
            for (i, t) in b.row_table.iter().enumerate() {
                check(&format!("f.table.{i}"), t.lay.row.y);
            }
        }
        // --- canvas_body ---
        {
            let demo = UiRect::new(40.0, 60.0, 800.0, 2400.0);
            let b = canvas_body(
                demo,
                0.0,
                &palette,
                [0.26, 0.27, 0.30, 1.0],
                Language::Ru,
                &mut m,
                &mut fs,
            );
            check("canvas.h", b.h);
            for (i, (_, r, _, _)) in b.node_demos.iter().enumerate() {
                check(&format!("c2.node.{i}"), r.y);
            }
            for (i, (_, r, _)) in b.edge_demos.iter().enumerate() {
                check(&format!("c2.edge.{i}"), r.y);
            }
            for (i, (_, r, _, _)) in b.port_demos.iter().enumerate() {
                check(&format!("c2.port.{i}"), r.y);
            }
            if let Some(t) = &b.node_table {
                check("c2.table.card", t.card.y);
                check("c2.table.header", t.header.y);
            }
        }
        // --- tokens_body ---
        {
            let demo = UiRect::new(40.0, 60.0, 800.0, 2400.0);
            let b = tokens_body(demo, 0.0, &palette, Language::Ru, &mut m, &mut fs);
            check("tokens.h", b.h);
            let mut idx = 0usize;
            for g in &b.groups {
                for r in &g.rows {
                    check(&format!("t.row.{idx}"), r.rect.y);
                    idx += 1;
                }
            }
        }
        // Полнота: каждое ожидание снято (секция не исчезла).
        for (tag, _) in expected {
            assert!(seen.iter().any(|t| t == tag), "золотой тег не снят: {tag}");
        }
    }
}
