//! K4-витрина (issue #62): секции 6 W2 + 14 Wave C компонентов во всех
//! применимых ST1-состояниях — раскладка + адаптер отрисовки для статичной
//! галереи ([`crate::kit_ui::gallery_layout`], «?» → «UI console»).
//!
//! Норматив — `design/rules/10-components.md` §K4:
//! - **K4.1** секции собраны ТОЛЬКО из `canvas_ui::layout`-примитивов и
//!   компонентных раскладок `canvas_ui::component::*` (зазоры — шкала S1
//!   `canvas_core::tokens::SPACING_*`, текст — `TextMeasurer`, silent
//!   clamps запрещены — G5);
//! - **K4.2** геометрия и подписи — из единой раскладки ([`k4_consume`]
//!   читает блоки линейки [`crate::kit_ui::gallery_layout_impl`]; draw
//!   рисует по сохранённым rect'ам — hit=draw; интерактива секции не имеют,
//!   поэтому hit-слотов нет — «when interaction appears» из LAY1.2);
//! - **K4.3** расширенные политики движка не используются (сцен нет —
//!   компонентные ряды), проверка маски `LayoutFeatures` не требуется.
//!
//! Состояния ST1 (8: Normal/Hovered/Focused/Pressed/Dragged/Disabled/
//! Selected/Error) показываются по ПРИМЕНИМОСТИ: компонент, чей стиль не
//! дифференцирует состояние, секцию с этим состоянием не получает — факт
//! зафиксирован в use-case-доке (§7) и golden-тесте (прецедент row/slider).
//! Визуализация состояний сверх компонентных стилей — контракт FR-057:
//! Focused — рамка accent (focus-ring, рисует потребитель), Dragged —
//! state-layer 0.16 поверх ([`canvas_ui::component::state_layer_alpha`],
//! M3), Error — семантика компонента (kind/слот `control_danger`).
//!
//! Секции декоративные (линт не считает их интерактивными) — паттерн
//! статичной витрины FR-055. Добавка АДДИТИВНА: хвост линейки после
//! LAY-SHOWCASE/Wave C, поле [`K4Sections`] в конце [`crate::kit_ui::GalleryLayout`],
//! отрисовка — один вызов из `App::kit_gallery_overlay`.

use canvas_core::Language;
use canvas_ui::component::accordion::{accordion_layout, accordion_style, AccordionLayout};
use canvas_ui::component::avatar::{avatar_layout, avatar_style, AvatarLayout, AvatarSource};
use canvas_ui::component::badge::{badge_layout, badge_style, BadgeKind, BadgeLayout, BadgeTone};
use canvas_ui::component::banner::{banner, paint_banner, BannerKind, BannerLayout, BANNER_H};
use canvas_ui::component::chat_bubble::{
    chat_bubble, paint_chat_bubble, ChatBubbleKind, ChatBubbleLayout, CHAT_HEADER_H, CHAT_LINE_H,
};
use canvas_ui::component::checkbox::{
    checkbox_glyph, checkbox_layout, checkbox_style, CheckboxLayout, CheckboxState,
};
use canvas_ui::component::command_palette::{command_palette_layout, CommandPaletteLayout};
use canvas_ui::component::crumbs::{crumbs, crumbs_active_index, CRUMB_H};
use canvas_ui::component::dropdown::AnchoredSide;
use canvas_ui::component::progress::{
    progress_layout, progress_style, ProgressKind, ProgressLayout,
};
use canvas_ui::component::radio::{radio_group_layout, radio_style, RadioLayout};
use canvas_ui::component::radio_card::{paint_radio_card, radio_card, RadioCardLayout};
use canvas_ui::component::segmented::{segmented_layout, segmented_style, SegmentedLayout};
use canvas_ui::component::skeleton::{
    skeleton_alpha, skeleton_layout, SkeletonLayout, SkeletonPattern,
};
use canvas_ui::component::slider::{slider_layout, slider_style, SliderLayout, SliderOpts};
use canvas_ui::component::snackbar::{snackbar_layout, snackbar_style, Snackbar};
use canvas_ui::component::tabs::{tab_style, tabs_layout, TabsLayout};
use canvas_ui::component::tree::{tree_layout, TreeNode};
use canvas_ui::component::two_column::{two_column, two_column_right};
use canvas_ui::geometry::{UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{
    self, BadgeTone as KitBadgeTone, ButtonVariant, ControlSize, ControlStyle, KitPalette, KitState,
};
use canvas_ui::measure::TextMeasurer;

use crate::kit_ui::{FONT_FAMILY, LABEL_SIZE, STATE_LABEL_W};

// === Константы демо K4 (размеры контента — не зазоры; зазоры только S1) ====

/// Ширина колонки демо-секции двух колонок (two_column) — «sidebar».
pub const K4_TWO_COLUMN_SIDEBAR_W: f32 = 96.0;
/// Высота строки демо two_column (высота панелей).
pub const K4_TWO_COLUMN_ROW_H: f32 = 64.0;
/// Высота строки-бара демо tabs (заголовки вкладок).
pub const K4_TABS_BAR_H: f32 = 32.0;
/// Высота строки-аккордеона (заголовок; тело — [`crate::kit_ui::DEMO_ACCORDION_BODY_H`]).
pub const K4_ACCORDION_HEADER_H: f32 = 32.0;
/// Высота строки скелетона «TextLines» (шаг строки паттерна).
pub const K4_SKELETON_ROW_H: f32 = 10.0;
/// Высота демо-карточки скелетона (паттерн Card).
pub const K4_SKELETON_CARD_H: f32 = 56.0;
/// Ширина демо-карточки скелетона (паттерн Card — компонентный параметр).
pub const K4_SKELETON_CARD_W: f32 = 160.0;
/// Высота демо-таблицы скелетона (паттерн Table 3×3).
pub const K4_SKELETON_TABLE_H: f32 = 60.0;
/// Статичный момент пульса скелетона (максимум альфы 0.12 — фаза 0.25
/// периода 1500 мс [`skeleton_alpha`]; витрина статична — анимации нет).
pub const K4_SKELETON_DEMO_MS: u64 = 375;
/// Головной отступ ячейки бейджа (якорь-чип ниже, бейдж заходит за верх:
/// половина высоты бейджа 16/2 + 2).
pub const K4_BADGE_HEADROOM: f32 = 10.0;
/// Высота ячейки бейджа (головной отступ + чип-якорь CHIP_HEIGHT + нижний
/// шаг S1/3 — см. [`k4_section_h`]; зафиксировано константой — не зазор
/// шкалы S1; бейдж заходит за верх якоря на половину своей высоты 8 ≤ 10).
pub const K4_BADGE_CELL_H: f32 = K4_BADGE_HEADROOM + kit::CHIP_HEIGHT + 6.0;
/// Размер контента поповера (компонентный демо-пузырь).
pub const K4_POPOVER_CONTENT: UiVec2 = UiVec2::new(160.0, 40.0);
/// Ширина якорной кнопки поповера.
pub const K4_POPOVER_ANCHOR_W: f32 = 120.0;
/// Вертикальный ход стрелки+зазора [`canvas_ui::component::popover::popover`]
/// (стрелка 8 + зазор 4 — внутренние литералы компонента, mirrored
/// константой: не зазор шкалы S1).
pub const K4_POPOVER_ARROW_SPAN: f32 = 12.0;
/// Верхний отступ [`command_palette_layout`] внутри «вьюпорта» (внутренний
/// литерал компонента — mirrored константой, не зазор шкалы S1).
pub const K4_PALETTE_TOP_OFFSET: f32 = 60.0;
/// Нижний отступ бара snackbar от низа вьюпорта (внутренний литерал
/// [`snackbar_layout`] — mirrored константой).
pub const K4_SNACKBAR_MARGIN: f32 = 24.0;
/// Ширина ячейки аватара (сторона Lg 44 + шаг S1/3).
pub const K4_AVATAR_CELL_W: f32 = 56.0;
/// Шаг отступа уровней дерева (демо-параметр [`canvas_ui::component::tree::tree_layout`]).
pub const K4_TREE_INDENT: f32 = 16.0;

/// Порядок K4-секций в хвосте линейки (6 W2, затем 14 Wave C — порядок
/// тела issue #62). Совпадает с порядком потребления [`k4_consume`].
pub const K4_ORDER: [&str; 20] = [
    "banner",
    "chat_bubble",
    "crumbs",
    "footer",
    "radio_card",
    "two_column",
    "checkbox",
    "slider",
    "radio",
    "tabs",
    "segmented",
    "command_palette",
    "accordion",
    "progress",
    "skeleton",
    "badge",
    "popover",
    "snackbar",
    "avatar",
    "tree",
];

/// Ключ i18n заголовка K4-секции (W2 — новые ключи; Wave C — ключи
/// реестра TO-BE переиспользуются: заголовок один на компонент).
pub fn k4_title_key(id: &str) -> &'static str {
    match id {
        "banner" => crate::i18n::keys::KIT_SECTION_BANNER,
        "chat_bubble" => crate::i18n::keys::KIT_SECTION_CHAT_BUBBLE,
        "crumbs" => crate::i18n::keys::KIT_SECTION_CRUMBS,
        "footer" => crate::i18n::keys::KIT_SECTION_FOOTER,
        "radio_card" => crate::i18n::keys::KIT_SECTION_RADIO_CARD,
        "two_column" => crate::i18n::keys::KIT_SECTION_TWO_COLUMN,
        "checkbox" => crate::i18n::keys::KIT_SECTION_CHECKBOX,
        "slider" => crate::i18n::keys::KIT_SECTION_SLIDER,
        "radio" => crate::i18n::keys::KIT_SECTION_RADIO,
        "tabs" => crate::i18n::keys::KIT_SECTION_TABS,
        "segmented" => crate::i18n::keys::KIT_SECTION_SEGMENTED,
        "command_palette" => crate::i18n::keys::KIT_SECTION_COMMAND_PALETTE,
        "accordion" => crate::i18n::keys::KIT_SECTION_ACCORDION,
        "progress" => crate::i18n::keys::KIT_SECTION_PROGRESS,
        "skeleton" => crate::i18n::keys::KIT_SECTION_SKELETON,
        "badge" => crate::i18n::keys::KIT_SECTION_BADGE,
        "popover" => crate::i18n::keys::KIT_SECTION_POPOVER,
        "snackbar" => crate::i18n::keys::KIT_SECTION_SNACKBAR,
        "avatar" => crate::i18n::keys::KIT_SECTION_AVATAR,
        "tree" => crate::i18n::keys::KIT_SECTION_TREE,
        _ => crate::i18n::keys::KIT_SECTION_BANNER,
    }
}

/// Высота контента K4-секции (формула строк — единый источник с
/// [`k4_consume`]:golden-тест `k4_sections_rows_fit_block` сверяет).
pub fn k4_section_h(id: &str) -> f32 {
    /// Строки одной высоты: Σ h + (n−1)·SPACING_SM (шаг рядов — как у
    /// секции Buttons линейки).
    fn rows_h(row_h: f32, n: usize) -> f32 {
        row_h * n as f32 + (n.saturating_sub(1)) as f32 * canvas_core::tokens::SPACING_SM
    }
    match id {
        // Баннер: 4 строки (пары состояний + Error во всю ширину).
        "banner" => rows_h(BANNER_H, 4),
        // Пузырь: 3 строки — Normal и Error парами (пузырь + подпись
        // состояния в правой ячейке), Success во всю ширину.
        "chat_bubble" => {
            let h = canvas_core::tokens::SPACING_SM * 0.5
                + CHAT_HEADER_H
                + CHAT_LINE_H
                + canvas_core::tokens::SPACING_SM;
            rows_h(h, 3)
        }
        // Крошки: путь + переполнение.
        "crumbs" => rows_h(CRUMB_H, 2),
        // Футер: правый ряд + split.
        "footer" => rows_h(kit::BUTTON_HEIGHT, 2),
        // Радио-карточка: 2 строки (3 карточки Normal/Hovered/Pressed +
        // 2 — Selected/Disabled; дубль Normal не показываем) — пад карточки
        // = SPACING_SM, label/desc — метрики компонента.
        "radio_card" => {
            let h = canvas_core::tokens::SPACING_SM
                + canvas_ui::component::radio_card::RADIO_LABEL_LINE_H
                + canvas_core::tokens::SPACING_S
                + canvas_ui::component::radio_card::RADIO_DESC_LINE_H
                + canvas_core::tokens::SPACING_SM;
            rows_h(h, 2)
        }
        // Две колонки: левый + правый вариант.
        "two_column" => rows_h(K4_TWO_COLUMN_ROW_H, 2),
        // Чек-бокс: 5 состояний Unchecked + Checked + Indeterminate.
        "checkbox" => rows_h(ControlSize::Sm.chip_h(), 7),
        // Слайдер: 4 состояния (прецедент row/slider — остальные не
        // дифференцируются, см. use-case §7).
        "slider" => rows_h(ControlSize::Sm.chip_h(), 4),
        // Радио: 5 состояний.
        "radio" => rows_h(ControlSize::Sm.chip_h(), 5),
        // Табы: 6 состояний баром на строку.
        "tabs" => rows_h(K4_TABS_BAR_H, 6),
        // Segmented: 4 состояния (Disabled не дифференцируется компонентом).
        "segmented" => rows_h(ControlSize::Md.chip_h(), 4),
        // Палитра команд: окно целиком (верхний отступ 60 компонента +
        // input + 3 строки результата + нижний шаг).
        "command_palette" => {
            K4_PALETTE_TOP_OFFSET
                + kit::TEXT_FIELD_HEIGHT
                + 3.0 * kit::LIST_ROW_H
                + canvas_core::tokens::SPACING_SM
        }
        // Аккордеон: 3 состояния заголовка + Selected раскрыт (тело —
        // DEMO_ACCORDION_BODY_H).
        "accordion" => {
            3.0 * K4_ACCORDION_HEADER_H
                + K4_ACCORDION_HEADER_H
                + crate::kit_ui::DEMO_ACCORDION_BODY_H
                + 3.0 * canvas_core::tokens::SPACING_SM
        }
        // Прогресс: Linear/LinearIndeterminate/Circular/Spinner + Disabled.
        "progress" => rows_h(ControlSize::Sm.chip_h(), 5),
        // Скелетон: TextLines + Card + Table.
        "skeleton" => {
            3.0 * K4_SKELETON_ROW_H
                + 2.0 * canvas_core::tokens::SPACING_S
                + K4_SKELETON_CARD_H
                + K4_SKELETON_TABLE_H
                + 2.0 * canvas_core::tokens::SPACING_SM
        }
        // Бейдж: 2 строки тонов × 3 ячейки (якорь-чип + бейдж за верхом).
        "badge" => rows_h(K4_BADGE_CELL_H, 2),
        // Поповер: 2 ряда (Below/Above) — каждый якорь + ход стрелки +
        // контент (стрелка+зазор — K4_POPOVER_ARROW_SPAN, контент —
        // компонентный размер).
        "popover" => rows_h(
            kit::BUTTON_HEIGHT + K4_POPOVER_ARROW_SPAN + K4_POPOVER_CONTENT.y,
            2,
        ),
        // Snackbar: 2 строки (бар + внутренний нижний отступ компонента).
        "snackbar" => rows_h(kit::BUTTON_HEIGHT + K4_SNACKBAR_MARGIN, 2),
        // Аватар: ряд размеров (Lg) + ряд тонов (Md).
        "avatar" => {
            ControlSize::Lg.icon_btn()
                + canvas_core::tokens::SPACING_SM
                + ControlSize::Md.icon_btn()
        }
        // Дерево: 4 видимых строки (root + 2 ветки, одна с листом).
        "tree" => 4.0 * kit::LIST_ROW_H,
        _ => 0.0,
    }
}

/// Подпись-текст: ключ i18n или сырой класс (классы не переводятся —
/// прецедент PERCENT_LABELS/«16:9» секций LAY-SHOWCASE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K4Caption {
    /// Ключ i18n (RU/EN).
    Key(&'static str),
    /// Класс-имя (без перевода).
    Raw(&'static str),
}

impl K4Caption {
    /// Текст подписи на языке витрины.
    pub fn text(self, lang: Language) -> &'static str {
        match self {
            K4Caption::Key(k) => crate::i18n::tr(lang, k),
            K4Caption::Raw(s) => s,
        }
    }
}

/// Подпись состояния ST1 (ключ по варианту; Error/Dragged — новые ключи).
fn state_caption(state: KitState) -> K4Caption {
    match state {
        KitState::Normal => K4Caption::Key("kit.state.normal"),
        KitState::Hovered => K4Caption::Key("kit.state.hover"),
        KitState::Focused => K4Caption::Key("kit.state.focused"),
        KitState::Pressed => K4Caption::Key("kit.state.pressed"),
        KitState::Dragged => K4Caption::Key(crate::i18n::keys::KIT_STATE_DRAGGED),
        KitState::Disabled => K4Caption::Key("kit.state.disabled"),
        KitState::Selected => K4Caption::Key("kit.state.selected"),
        KitState::Error => K4Caption::Key(crate::i18n::keys::KIT_STATE_ERROR),
    }
}

// === Модель раскладки (геометрия + данные; стили — в draw из палитры) ======

/// Раскладка 20 K4-секций (контент-координаты витрины; сдвиг/фильтр —
/// [`K4Sections::shift_and_filter`], паттерн FR-059).
#[derive(Debug, Clone, Default)]
pub struct K4Sections {
    /// Подписи строк состояний/вариантов (левая колонка, 11px muted);
    /// канон вертикали #68 — строковый бокс `text_area_vcentered` строки.
    pub labels: Vec<(UiRect, K4Caption)>,
    // --- W2 (6) ---
    pub banner_rows: Vec<K4BannerRow>,
    pub chat_rows: Vec<K4ChatRow>,
    pub crumbs_rows: Vec<K4CrumbsRow>,
    pub footer_rows: Vec<K4FooterRow>,
    pub radio_card_cells: Vec<K4RadioCardCell>,
    pub two_column_rows: Vec<K4TwoColumnRow>,
    // --- Wave C (14) ---
    pub checkbox_cells: Vec<K4CheckboxCell>,
    pub slider_rows: Vec<K4SliderRow>,
    pub radio_cells: Vec<K4RadioCell>,
    pub tabs_rows: Vec<K4TabsRow>,
    pub segmented_rows: Vec<K4SegmentedRow>,
    pub palette: Option<K4PaletteDemo>,
    pub accordion_rows: Vec<K4AccordionRow>,
    pub progress_rows: Vec<K4ProgressRow>,
    pub skeleton_rows: Vec<K4SkeletonRow>,
    pub badge_cells: Vec<K4BadgeCell>,
    pub popover_rows: Vec<K4PopoverRow>,
    pub snackbar_rows: Vec<K4SnackbarRow>,
    pub avatar_cells: Vec<K4AvatarCell>,
    pub tree_rows: Vec<K4TreeRow>,
}

/// Строка секции Banner: (вид, состояние, измеренные ширины, раскладка).
#[derive(Debug, Clone)]
pub struct K4BannerRow {
    pub kind: BannerKind,
    pub state: KitState,
    /// Focused — рамка accent поверх (FR-057).
    pub focused: bool,
    /// Dragged — state-layer 0.16 поверх (M3).
    pub dragged: bool,
    /// Измеренная ширина подписи (тот же замер, что в раскладке).
    pub label_w: f32,
    /// Измеренная ширина подписи кнопки действия.
    pub action_w: f32,
    pub lay: BannerLayout,
}

/// Строка секции ChatBubble (вид = семантика; KitState компонентом не
/// принимается — честная фиксация в §7).
#[derive(Debug, Clone)]
pub struct K4ChatRow {
    pub kind: ChatBubbleKind,
    pub lay: ChatBubbleLayout,
}

/// Строка секции Crumbs: чипы (раскладка компонента) + активный индекс.
#[derive(Debug, Clone)]
pub struct K4CrumbsRow {
    /// true — демо переполнения («…» вместо корня).
    pub overflow: bool,
    pub chips: Vec<(UiRect, String)>,
    /// Индекс активной крошки (Selected) в ПОРЯДКЕ ОТОБРАЖЕНИЯ.
    pub active: Option<usize>,
}

/// Кнопка футера: вариант + состояние + подпись.
#[derive(Debug, Clone)]
pub struct K4FooterBtn {
    pub rect: UiRect,
    pub variant: ButtonVariant,
    pub state: KitState,
    /// Focused — рамка accent (FR-057).
    pub focused: bool,
    pub label: K4Caption,
}

/// Строка секции Footer: правый ряд ([`canvas_ui::component::footer::footer_buttons_measured`])
/// или split ([`canvas_ui::component::footer::split_footer_buttons`]).
#[derive(Debug, Clone)]
pub struct K4FooterRow {
    pub split: bool,
    pub buttons: Vec<K4FooterBtn>,
}

/// Ячейка секции RadioCard: (состояние, выбранность, раскладка).
#[derive(Debug, Clone)]
pub struct K4RadioCardCell {
    pub state: KitState,
    pub selected: bool,
    pub label_w: f32,
    pub desc_w: Option<f32>,
    pub lay: RadioCardLayout,
}

/// Строка секции TwoColumn: (правый вариант, sidebar, content).
#[derive(Debug, Clone)]
pub struct K4TwoColumnRow {
    pub right: bool,
    pub sidebar: UiRect,
    pub content: UiRect,
}

/// Ячейка секции Checkbox: (check-состояние, ST1, в фокусе, раскладка).
#[derive(Debug, Clone)]
pub struct K4CheckboxCell {
    pub check: CheckboxState,
    pub kit_state: KitState,
    pub focused: bool,
    pub lay: CheckboxLayout,
}

/// Строка секции Slider: (состояние, значение, раскладка).
#[derive(Debug, Clone)]
pub struct K4SliderRow {
    pub state: KitState,
    pub value: f32,
    pub lay: SliderLayout,
}

/// Ячейка секции Radio: (состояние, выбранность, раскладка пункта).
#[derive(Debug, Clone)]
pub struct K4RadioCell {
    pub state: KitState,
    pub selected: bool,
    pub lay: RadioLayout,
}

/// Строка секции Tabs: бар вкладок в одном состоянии строки (активная
/// вкладка — для Selected/Focused).
#[derive(Debug, Clone)]
pub struct K4TabsRow {
    pub state: KitState,
    /// Активная вкладка (индикатор) — Selected/Focused.
    pub active: Option<usize>,
    /// Focused — рамка accent на активной вкладке.
    pub focused: bool,
    pub lay: TabsLayout,
}

/// Строка секции Segmented: контейнер + сегменты в одном состоянии.
#[derive(Debug, Clone)]
pub struct K4SegmentedRow {
    pub state: KitState,
    /// Активный сегмент (заливка) — Selected.
    pub active: Option<usize>,
    pub lay: SegmentedLayout,
}

/// Демо палитры команд — окно ЦЕЛИКОМ (input + строки результатов).
#[derive(Debug, Clone)]
pub struct K4PaletteDemo {
    pub lay: CommandPaletteLayout,
    /// Запрос (ключ i18n).
    pub query: K4Caption,
    /// Подписи действий (ключи i18n) по индексу результата.
    pub actions: Vec<K4Caption>,
    /// Состояние строки результата (Normal/Hovered/Selected).
    pub row_states: Vec<KitState>,
}

/// Строка секции Accordion: (состояние, раскладка; Selected — раскрыт).
#[derive(Debug, Clone)]
pub struct K4AccordionRow {
    pub state: KitState,
    pub label: K4Caption,
    pub body: K4Caption,
    pub lay: AccordionLayout,
}

/// Строка секции Progress: вариант + состояние + раскладка.
#[derive(Debug, Clone)]
pub struct K4ProgressRow {
    pub kind: ProgressKind,
    pub state: KitState,
    pub label: K4Caption,
    pub lay: ProgressLayout,
}

/// Строка секции Skeleton: паттерн (класс) + rect'ы блоков.
#[derive(Debug, Clone)]
pub struct K4SkeletonRow {
    pub pattern: &'static str,
    pub lay: SkeletonLayout,
}

/// Ячейка секции Badge: тон + вид + якорь-чип + раскладка бейджа.
#[derive(Debug, Clone)]
pub struct K4BadgeCell {
    pub tone: BadgeTone,
    pub kind: BadgeKind,
    pub anchor: UiRect,
    pub anchor_label: K4Caption,
    pub lay: BadgeLayout,
}

/// Строка секции Popover: якорь + пузырь (Below/Above; стрелка).
#[derive(Debug, Clone)]
pub struct K4PopoverRow {
    /// true — пузырь под якорем (false — над).
    pub below: bool,
    pub anchor: UiRect,
    pub anchor_label: K4Caption,
    pub arrow: UiRect,
    pub content: UiRect,
    pub body: K4Caption,
}

/// Строка секции Snackbar: бар (тон + опц. действие).
#[derive(Debug, Clone)]
pub struct K4SnackbarRow {
    pub tone: BadgeTone,
    pub with_action: bool,
    pub label: K4Caption,
    pub action: K4Caption,
    pub lay: canvas_ui::component::snackbar::SnackbarLayout,
}

/// Ячейка секции Avatar: размер/тон/источник + раскладка.
#[derive(Debug, Clone)]
pub struct K4AvatarCell {
    pub size: ControlSize,
    pub tone: KitBadgeTone,
    pub source: AvatarSource,
    pub lay: AvatarLayout,
}

/// Строка секции Tree: видимая строка дерева + состояние.
#[derive(Debug, Clone)]
pub struct K4TreeRow {
    pub state: KitState,
    pub row: canvas_ui::component::tree::TreeRow,
}

/// Сдвиг rect на −off (скролл витрины — паттерн FR-059).
fn sh(r: UiRect, off: f32) -> UiRect {
    UiRect::new(r.x, r.y - off, r.w, r.h)
}

impl K4Sections {
    /// Сдвиг на `scroll.offset` + фильтр ПОЛНОЙ видимости (тексты кита не
    /// клипятся — секция за краем окна не рисуется; паттерн секций
    /// [`crate::kit_ui::gallery_layout_impl`]).
    pub fn shift_and_filter(&mut self, off: f32, visible: &dyn Fn(&UiRect) -> bool) {
        let vis_shift_vec = |v: &mut Vec<UiRect>| {
            v.retain(|r| visible(r));
            for r in v.iter_mut() {
                *r = sh(*r, off);
            }
        };
        self.labels.retain(|(area, _)| visible(area));
        for (area, _) in self.labels.iter_mut() {
            *area = sh(*area, off);
        }
        self.banner_rows.retain(|r| visible(&r.lay.rect));
        for r in self.banner_rows.iter_mut() {
            r.lay.rect = sh(r.lay.rect, off);
            r.lay.label_area = sh(r.lay.label_area, off);
            r.lay.action_button = sh(r.lay.action_button, off);
        }
        self.chat_rows.retain(|r| visible(&r.lay.rect));
        for r in self.chat_rows.iter_mut() {
            r.lay.rect = sh(r.lay.rect, off);
            r.lay.text_area = sh(r.lay.text_area, off);
            if let Some(h) = r.lay.header_area.as_mut() {
                *h = sh(*h, off);
            }
            for tc in r.lay.tool_call_rows.iter_mut() {
                *tc = sh(*tc, off);
            }
        }
        self.crumbs_rows
            .retain(|r| r.chips.first().is_some_and(|(c, _)| visible(c)));
        for r in self.crumbs_rows.iter_mut() {
            for (c, _) in r.chips.iter_mut() {
                *c = sh(*c, off);
            }
        }
        self.footer_rows
            .retain(|r| r.buttons.first().is_some_and(|b| visible(&b.rect)));
        for r in self.footer_rows.iter_mut() {
            for b in r.buttons.iter_mut() {
                b.rect = sh(b.rect, off);
            }
        }
        self.radio_card_cells.retain(|c| visible(&c.lay.rect));
        for c in self.radio_card_cells.iter_mut() {
            c.lay.rect = sh(c.lay.rect, off);
            c.lay.indicator = sh(c.lay.indicator, off);
            c.lay.label = sh(c.lay.label, off);
            if let Some(d) = c.lay.desc.as_mut() {
                *d = sh(*d, off);
            }
        }
        self.two_column_rows.retain(|r| visible(&r.sidebar));
        for r in self.two_column_rows.iter_mut() {
            r.sidebar = sh(r.sidebar, off);
            r.content = sh(r.content, off);
        }
        self.checkbox_cells.retain(|c| visible(&c.lay.box_rect));
        for c in self.checkbox_cells.iter_mut() {
            c.lay.box_rect = sh(c.lay.box_rect, off);
            c.lay.label_rect = sh(c.lay.label_rect, off);
        }
        self.slider_rows.retain(|r| visible(&r.lay.track));
        for r in self.slider_rows.iter_mut() {
            r.lay.track = sh(r.lay.track, off);
            r.lay.filled = sh(r.lay.filled, off);
            r.lay.knob = sh(r.lay.knob, off);
        }
        self.radio_cells.retain(|c| visible(&c.lay.circle));
        for c in self.radio_cells.iter_mut() {
            c.lay.circle = sh(c.lay.circle, off);
            c.lay.label_rect = sh(c.lay.label_rect, off);
        }
        self.tabs_rows.retain(|r| visible(&r.lay.bar));
        for r in self.tabs_rows.iter_mut() {
            r.lay.bar = sh(r.lay.bar, off);
            r.lay.content = sh(r.lay.content, off);
            for t in r.lay.tabs.iter_mut() {
                t.rect = sh(t.rect, off);
                t.indicator = sh(t.indicator, off);
            }
        }
        self.segmented_rows.retain(|r| visible(&r.lay.container));
        for r in self.segmented_rows.iter_mut() {
            r.lay.container = sh(r.lay.container, off);
            for s in r.lay.segments.iter_mut() {
                *s = sh(*s, off);
            }
            r.lay.active_rect = sh(r.lay.active_rect, off);
        }
        if let Some(pl) = self.palette.as_mut() {
            // Окно целиком: input и ВСЕ строки результатов (полная видимость —
            // частичное окно палитры за краем не рисуется).
            let results_visible = pl.lay.results.iter().all(|(r, _)| visible(r));
            if !visible(&pl.lay.input) || !results_visible {
                self.palette = None;
            } else {
                pl.lay.input = sh(pl.lay.input, off);
                for (r, _) in pl.lay.results.iter_mut() {
                    *r = sh(*r, off);
                }
            }
        }
        // Заголовок + контент (у свёрнутых content — 0-height на нижней
        // кромке header — предикат совпадает с видимостью header).
        self.accordion_rows
            .retain(|r| visible(&r.lay.header) && visible(&r.lay.content));
        for r in self.accordion_rows.iter_mut() {
            r.lay.header = sh(r.lay.header, off);
            r.lay.content = sh(r.lay.content, off);
            r.lay.chevron = sh(r.lay.chevron, off);
        }
        self.progress_rows.retain(|r| visible(&r.lay.track));
        for r in self.progress_rows.iter_mut() {
            r.lay.track = sh(r.lay.track, off);
            r.lay.fill = sh(r.lay.fill, off);
        }
        self.skeleton_rows
            .retain(|r| r.lay.rects.first().is_some_and(visible));
        for r in self.skeleton_rows.iter_mut() {
            vis_shift_vec(&mut r.lay.rects);
        }
        // Якорь + сам бейдж (бейдж всплывает над якорем на h/2 — виден
        // только когда целиком в окне).
        self.badge_cells
            .retain(|c| visible(&c.anchor) && visible(&c.lay.rect));
        for c in self.badge_cells.iter_mut() {
            c.anchor = sh(c.anchor, off);
            c.lay.rect = sh(c.lay.rect, off);
        }
        // Якорь + стрелка + пузырь (у рядов Below пузырь НАМОГОГО ниже
        // якоря — видимость якоря недостаточна).
        self.popover_rows
            .retain(|r| visible(&r.anchor) && visible(&r.arrow) && visible(&r.content));
        for r in self.popover_rows.iter_mut() {
            r.anchor = sh(r.anchor, off);
            r.arrow = sh(r.arrow, off);
            r.content = sh(r.content, off);
        }
        self.snackbar_rows.retain(|r| visible(&r.lay.rect));
        for r in self.snackbar_rows.iter_mut() {
            r.lay.rect = sh(r.lay.rect, off);
            r.lay.message_rect = sh(r.lay.message_rect, off);
            if let Some(a) = r.lay.action_rect.as_mut() {
                *a = sh(*a, off);
            }
        }
        self.avatar_cells.retain(|c| visible(&c.lay.rect));
        for c in self.avatar_cells.iter_mut() {
            c.lay.rect = sh(c.lay.rect, off);
        }
        self.tree_rows.retain(|r| visible(&r.row.rect));
        for r in self.tree_rows.iter_mut() {
            r.row.rect = sh(r.row.rect, off);
            r.row.chevron = sh(r.row.chevron, off);
            if let Some(i) = r.row.icon.as_mut() {
                *i = sh(*i, off);
            }
            r.row.label = sh(r.row.label, off);
        }
    }
}

// === Потребление линейки (K4.2: единая геометрия с gallery_layout) =========

/// Потребление блоков K4-секций из линейки ([`crate::kit_ui::gallery_layout_impl`]):
/// по 2 rect'а на секцию (подпись, контент). Геометрия — контент-координаты
/// витрины ДО сдвига скролла (сдвиг — [`K4Sections::shift_and_filter`]).
#[allow(clippy::too_many_arguments)] // прецедент wave_c_consume
pub(crate) fn k4_consume(
    ruler: &mut std::vec::IntoIter<UiRect>,
    control_x: f32,
    control_w: f32,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    section_titles: &mut Vec<(UiPoint, &'static str)>,
) -> K4Sections {
    let mut k4 = K4Sections::default();
    for id in K4_ORDER {
        let title = ruler.next().unwrap_or_default();
        section_titles.push((UiPoint::new(title.x, title.y), k4_title_key(id)));
        let block = ruler.next().unwrap_or_default();
        let h = k4_section_h(id);
        let slot = UiRect::new(control_x, block.y, control_w, h);
        match id {
            "banner" => banner_section(&mut k4, slot, lang, m, fs),
            "chat_bubble" => chat_section(&mut k4, slot),
            "crumbs" => crumbs_section(&mut k4, slot, lang, m, fs),
            "footer" => footer_section(&mut k4, slot, lang, m, fs),
            "radio_card" => radio_card_section(&mut k4, slot, lang, m, fs),
            "two_column" => two_column_section(&mut k4, slot),
            "checkbox" => checkbox_section(&mut k4, slot, lang, m, fs),
            "slider" => slider_section(&mut k4, slot),
            "radio" => radio_section(&mut k4, slot, lang, m, fs),
            "tabs" => tabs_section(&mut k4, slot, lang, m, fs),
            "segmented" => segmented_section(&mut k4, slot, m, fs),
            "command_palette" => palette_section(&mut k4, slot, lang, m, fs),
            "accordion" => accordion_section(&mut k4, slot, lang, m, fs),
            "progress" => progress_section(&mut k4, slot),
            "skeleton" => skeleton_section(&mut k4, slot),
            "badge" => badge_section(&mut k4, slot, lang, m, fs),
            "popover" => popover_section(&mut k4, slot, lang),
            "snackbar" => snackbar_section(&mut k4, slot, lang, m, fs),
            "avatar" => avatar_section(&mut k4, slot),
            "tree" => tree_section(&mut k4, slot, lang, m, fs),
            _ => {}
        }
    }
    k4
}

/// X подписи строки состояния — левая колонка контента (как подписи
/// вариантов Buttons: слот подписи шириной STATE_LABEL_W левее контролов
/// с шагом SPACING_SM; раскладка знает control_x — подписи выводятся из
/// него, единая геометрия hit=draw с отрисовкой в k4_paint).
fn label_x(slot: UiRect) -> f32 {
    slot.x - STATE_LABEL_W - canvas_core::tokens::SPACING_SM
}

/// Подпись строки состояния в левую колонку (как подписи вариантов Buttons).
/// Канон вертикали #68: строковый бокс кегля 11 (`text_area_vcentered`),
/// центрированный в rect строки — ручной центровки `(row_h − H)/2` нет.
fn row_label_at(k4: &mut K4Sections, x: f32, y: f32, row_h: f32, caption: K4Caption) {
    k4.labels.push((
        kit::text_area_vcentered(UiRect::new(x, y, STATE_LABEL_W, row_h), 11.0),
        caption,
    ));
}

// --- W2 --------------------------------------------------------------------

/// Секция Banner: пары состояний в строке (вторая — Error во всю ширину);
/// подпись баннера = имя состояния (самодокументация — паттерн Buttons).
fn banner_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let label_text = |lang, st: KitState| -> &'static str { state_caption(st).text(lang) };
    let measure = |text: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem| -> f32 {
        m.width_of(fs, text, FONT_FAMILY, LABEL_SIZE)
    };
    let rows: [(KitState, KitState); 3] = [
        (KitState::Normal, KitState::Hovered),
        (KitState::Focused, KitState::Pressed),
        (KitState::Dragged, KitState::Disabled),
    ];
    let per = ((slot.w - kit::GAP_CONTROLS) / 2.0).max(0.0);
    let mut y = slot.y;
    for (a, b) in rows {
        for (i, st) in [a, b].into_iter().enumerate() {
            let x = slot.x + i as f32 * (per + kit::GAP_CONTROLS);
            let cell = UiRect::new(x, y, per, BANNER_H);
            let text = label_text(lang, st);
            let lw = measure(text, m, fs);
            let action_text = crate::i18n::tr(lang, crate::i18n::keys::KIT_BANNER_ACTION);
            let aw = measure(action_text, m, fs);
            let (lay, _) = banner(cell, lw, aw, BannerKind::Info, st, &neutral_palette());
            k4.banner_rows.push(K4BannerRow {
                kind: BannerKind::Info,
                state: st,
                focused: st == KitState::Focused,
                dragged: st == KitState::Dragged,
                label_w: lw,
                action_w: aw,
                lay,
            });
        }
        y += BANNER_H + canvas_core::tokens::SPACING_SM;
    }
    // Error — семантика компонента (kind=Error → control_danger).
    let text = label_text(lang, KitState::Error);
    let lw = measure(text, m, fs);
    let action_text = crate::i18n::tr(lang, crate::i18n::keys::KIT_BANNER_ACTION);
    let aw = measure(action_text, m, fs);
    let cell = UiRect::new(slot.x, y, slot.w, BANNER_H);
    let (lay, _) = banner(
        cell,
        lw,
        aw,
        BannerKind::Error,
        KitState::Error,
        &neutral_palette(),
    );
    k4.banner_rows.push(K4BannerRow {
        kind: BannerKind::Error,
        state: KitState::Error,
        focused: false,
        dragged: false,
        label_w: lw,
        action_w: aw,
        lay,
    });
}

/// ЗАГЛУШКА палитры для раскладки (геометрия баннера/пузыря/карточки от
/// палитры не зависит — компоненты выбирают только СЛОТЫ; настоящий
/// `KitPalette` подставляется в отрисовке через те же компонентные
/// функции-стили). G5/K4.1: раскладка геометрии чистая от палитры —
/// параметр нужен лишь сигнатурой компонента; инвариант «геометрия от
/// палитры не зависит» пинен golden-тестом `k4_layout_geometry_palette_free`.
fn neutral_palette() -> KitPalette {
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
        control_success: [0.30, 0.75, 0.55, 1.0],
        control_warning: [0.95, 0.65, 0.30, 1.0],
        stage_dim: [0.02, 0.02, 0.04, 0.6],
        scrollbar_thumb: [0.35, 0.38, 0.46, 0.7],
        rule_color: [0.30, 0.33, 0.40, 0.8],
    }
}

fn chat_section(k4: &mut K4Sections, slot: UiRect) {
    let h = canvas_core::tokens::SPACING_SM * 0.5
        + CHAT_HEADER_H
        + CHAT_LINE_H
        + canvas_core::tokens::SPACING_SM;
    let per = ((slot.w - kit::GAP_CONTROLS) / 2.0).max(0.0);
    let kinds = [
        (ChatBubbleKind::Normal, K4Caption::Key("kit.state.normal")),
        (
            ChatBubbleKind::Error,
            K4Caption::Key(crate::i18n::keys::KIT_STATE_ERROR),
        ),
    ];
    let mut y = slot.y;
    for (kind, cap) in kinds {
        for i in 0..2 {
            let x = slot.x + i as f32 * (per + kit::GAP_CONTROLS);
            let cell = UiRect::new(x, y, per, h);
            let (lay, _) = chat_bubble(cell, 1, 0, kind, &neutral_palette(), true);
            if i == 0 {
                k4.chat_rows.push(K4ChatRow { kind, lay });
            } else {
                row_label_at(k4, label_x(slot), y, h, cap);
            }
        }
        y += h + canvas_core::tokens::SPACING_SM;
    }
    let cell = UiRect::new(slot.x, y, slot.w, h);
    let (lay, _) = chat_bubble(
        cell,
        1,
        0,
        ChatBubbleKind::Success,
        &neutral_palette(),
        true,
    );
    k4.chat_rows.push(K4ChatRow {
        kind: ChatBubbleKind::Success,
        lay,
    });
}

fn crumbs_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let labels: Vec<String> = [
        crate::i18n::keys::KIT_CRUMBS_A,
        crate::i18n::keys::KIT_CRUMBS_B,
        crate::i18n::keys::KIT_CRUMBS_C,
        crate::i18n::keys::KIT_CRUMBS_D,
    ]
    .iter()
    .map(|k| crate::i18n::tr(lang, k).to_owned())
    .collect();
    // Строка 1: путь целиком (хвост активен — Selected).
    let row1 = UiRect::new(slot.x, slot.y, slot.w, CRUMB_H);
    let chips1 = crumbs(
        row1,
        &labels,
        canvas_ui::component::crumbs::CRUMB_GAP,
        kit::CHIP_PAD_H,
        m,
        fs,
    );
    let active1 = crumbs_active_index(labels.len(), chips1.len());
    k4.crumbs_rows.push(K4CrumbsRow {
        overflow: false,
        chips: chips1,
        active: active1,
    });
    row_label_at(
        k4,
        label_x(slot),
        slot.y,
        CRUMB_H,
        K4Caption::Key(crate::i18n::keys::KIT_CRUMBS_PATH),
    );
    // Строка 2: переполнение — удвоенный путь в половине слота («…»).
    let y2 = slot.y + CRUMB_H + canvas_core::tokens::SPACING_SM;
    let row2 = UiRect::new(slot.x, y2, (slot.w / 2.0).max(0.0), CRUMB_H);
    let mut wide = labels.clone();
    wide.extend(labels.iter().cloned());
    let chips2 = crumbs(
        row2,
        &wide,
        canvas_ui::component::crumbs::CRUMB_GAP,
        kit::CHIP_PAD_H,
        m,
        fs,
    );
    k4.crumbs_rows.push(K4CrumbsRow {
        overflow: true,
        chips: chips2,
        active: None,
    });
    row_label_at(
        k4,
        label_x(slot),
        y2,
        CRUMB_H,
        K4Caption::Key(crate::i18n::keys::KIT_CRUMBS_OVERFLOW),
    );
}

fn footer_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let width_of =
        |cap: K4Caption, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem| -> f32 {
            let text = cap.text(lang);
            canvas_ui::kit::button_size(text, m, fs, FONT_FAMILY, LABEL_SIZE).x
        };
    // Строка 1: правый ряд — Normal (Primary) + Hovered (Secondary).
    let y1 = slot.y;
    let caps1 = [
        (
            K4Caption::Key(crate::i18n::keys::KIT_FOOTER_OK),
            ButtonVariant::Primary,
            KitState::Normal,
        ),
        (
            K4Caption::Key(crate::i18n::keys::KIT_FOOTER_CANCEL),
            ButtonVariant::Secondary,
            KitState::Hovered,
        ),
    ];
    let widths1: Vec<f32> = caps1.iter().map(|(c, _, _)| width_of(*c, m, fs)).collect();
    let row1 = UiRect::new(slot.x, y1, slot.w, kit::BUTTON_HEIGHT);
    let placed1 = canvas_ui::component::footer::footer_buttons_measured(row1, &widths1, m);
    let mut btns1: Vec<K4FooterBtn> = Vec::new();
    for ((rect, _idx), (cap, variant, state)) in placed1.into_iter().zip(caps1) {
        btns1.push(K4FooterBtn {
            rect,
            variant,
            state,
            focused: false,
            label: cap,
        });
    }
    k4.footer_rows.push(K4FooterRow {
        split: false,
        buttons: btns1,
    });
    row_label_at(
        k4,
        label_x(slot),
        y1,
        kit::BUTTON_HEIGHT,
        K4Caption::Key(crate::i18n::keys::KIT_FOOTER_ROW_RIGHT),
    );
    // Строка 2: split — Pressed слева; Disabled + Focused справа.
    let y2 = y1 + kit::BUTTON_HEIGHT + canvas_core::tokens::SPACING_SM;
    let row2 = UiRect::new(slot.x, y2, slot.w, kit::BUTTON_HEIGHT);
    let left = [(
        K4Caption::Key(crate::i18n::keys::KIT_FOOTER_PREV),
        KitState::Pressed,
    )];
    let right = [
        (
            K4Caption::Key(crate::i18n::keys::KIT_FOOTER_CANCEL),
            KitState::Disabled,
        ),
        (
            K4Caption::Key(crate::i18n::keys::KIT_FOOTER_NEXT),
            KitState::Focused,
        ),
    ];
    let lw: Vec<f32> = left.iter().map(|(c, _)| width_of(*c, m, fs)).collect();
    let rw: Vec<f32> = right.iter().map(|(c, _)| width_of(*c, m, fs)).collect();
    let placed2 =
        canvas_ui::component::footer::split_footer_buttons(row2, &lw, &rw, kit::GAP_CONTROLS, 0.0);
    let mut btns2: Vec<K4FooterBtn> = Vec::new();
    for (rect, group, idx) in placed2 {
        let (cap, state) = match group {
            canvas_ui::component::footer::FooterGroup::Left => left[idx],
            canvas_ui::component::footer::FooterGroup::Right => right[idx],
        };
        btns2.push(K4FooterBtn {
            rect,
            variant: if state == KitState::Focused {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            },
            state,
            focused: state == KitState::Focused,
            label: cap,
        });
    }
    k4.footer_rows.push(K4FooterRow {
        split: true,
        buttons: btns2,
    });
    row_label_at(
        k4,
        label_x(slot),
        y2,
        kit::BUTTON_HEIGHT,
        K4Caption::Key(crate::i18n::keys::KIT_FOOTER_ROW_SPLIT),
    );
}

fn radio_card_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let card_h = canvas_core::tokens::SPACING_SM
        + canvas_ui::component::radio_card::RADIO_LABEL_LINE_H
        + canvas_core::tokens::SPACING_S
        + canvas_ui::component::radio_card::RADIO_DESC_LINE_H
        + canvas_core::tokens::SPACING_SM;
    let label_text = crate::i18n::tr(lang, crate::i18n::keys::KIT_RADIO_CARD_LABEL);
    let label_w = m.width_of(fs, label_text, FONT_FAMILY, LABEL_SIZE);
    // Описание — класс-строка (единицы экспорта, без перевода).
    let desc_w = Some(m.width_of(fs, "LaTeX · MathML", FONT_FAMILY, LABEL_SIZE));
    let rows: [(KitState, bool); 3] = [
        (KitState::Normal, false),
        (KitState::Hovered, false),
        (KitState::Pressed, false),
    ];
    let rows2: [(KitState, bool); 3] = [
        (KitState::Selected, true),
        (KitState::Disabled, false),
        (KitState::Normal, false),
    ];
    // Ряд из ТРЁХ карточек — шаг третей слота (паттерн badge/avatar:
    // (w − 2·GAP)/3); ряд из двух — тот же шаг (выравнивание по колонкам).
    let per = ((slot.w - 2.0 * kit::GAP_CONTROLS) / 3.0).max(0.0);
    let mut y = slot.y;
    for half in [rows, rows2] {
        for (i, (state, selected)) in half.into_iter().enumerate() {
            // Третья ячейка второй строки — дубликат Normal не показываем.
            if half == rows2 && i == 2 {
                break;
            }
            let x = slot.x + i as f32 * (per + kit::GAP_CONTROLS);
            let cell = UiRect::new(x, y, per, card_h);
            let (lay, _) = radio_card(cell, label_w, desc_w, selected, state, &neutral_palette());
            k4.radio_card_cells.push(K4RadioCardCell {
                state,
                selected,
                label_w,
                desc_w,
                lay,
            });
            row_label_at(k4, label_x(slot), y, card_h, state_caption(state));
        }
        y += card_h + canvas_core::tokens::SPACING_SM;
    }
}

fn two_column_section(k4: &mut K4Sections, slot: UiRect) {
    let mut y = slot.y;
    for right in [false, true] {
        let row = UiRect::new(slot.x, y, slot.w, K4_TWO_COLUMN_ROW_H);
        let lay = if right {
            two_column_right(row, K4_TWO_COLUMN_SIDEBAR_W, kit::GAP_CONTROLS)
        } else {
            two_column(row, K4_TWO_COLUMN_SIDEBAR_W, kit::GAP_CONTROLS)
        };
        k4.two_column_rows.push(K4TwoColumnRow {
            right,
            sidebar: lay.sidebar,
            content: lay.content,
        });
        row_label_at(
            k4,
            label_x(slot),
            y,
            K4_TWO_COLUMN_ROW_H,
            K4Caption::Key(if right {
                crate::i18n::keys::KIT_TWO_COLUMN_RIGHT
            } else {
                crate::i18n::keys::KIT_TWO_COLUMN_LEFT
            }),
        );
        y += K4_TWO_COLUMN_ROW_H + canvas_core::tokens::SPACING_SM;
    }
}

// --- Wave C ----------------------------------------------------------------

fn checkbox_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let row_h = ControlSize::Sm.chip_h();
    let demo: [(KitState, CheckboxState, bool); 7] = [
        (KitState::Normal, CheckboxState::Unchecked, false),
        (KitState::Hovered, CheckboxState::Unchecked, false),
        (KitState::Pressed, CheckboxState::Unchecked, false),
        (KitState::Focused, CheckboxState::Unchecked, true),
        (KitState::Disabled, CheckboxState::Unchecked, false),
        (KitState::Normal, CheckboxState::Checked, false),
        (KitState::Normal, CheckboxState::Indeterminate, false),
    ];
    let label = crate::i18n::tr(lang, crate::i18n::keys::KIT_CHECKBOX_LABEL);
    let mut y = slot.y;
    for (state, check, focused) in demo {
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let lay = checkbox_layout(
            row,
            label,
            check,
            ControlSize::Sm,
            kit::GAP_CONTROLS,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        k4.checkbox_cells.push(K4CheckboxCell {
            check,
            kit_state: state,
            focused,
            lay,
        });
        row_label_at(k4, label_x(slot), y, row_h, state_caption(state));
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

fn slider_section(k4: &mut K4Sections, slot: UiRect) {
    let row_h = ControlSize::Sm.chip_h();
    let opts = SliderOpts::default();
    let value = 0.35;
    let mut y = slot.y;
    for state in [
        KitState::Normal,
        KitState::Hovered,
        KitState::Pressed,
        KitState::Disabled,
    ] {
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let lay = slider_layout(row, value, &opts, ControlSize::Sm);
        k4.slider_rows.push(K4SliderRow { state, value, lay });
        row_label_at(k4, label_x(slot), y, row_h, state_caption(state));
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

fn radio_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let row_h = ControlSize::Sm.chip_h();
    let option = crate::i18n::tr(lang, crate::i18n::keys::KIT_DEMO_TAB_1).to_owned();
    let states = [
        KitState::Normal,
        KitState::Hovered,
        KitState::Pressed,
        KitState::Selected,
        KitState::Disabled,
    ];
    let mut y = slot.y;
    for state in states {
        let selected = state == KitState::Selected;
        let group = canvas_ui::component::radio::RadioGroup {
            options: vec![option.clone()],
            selected: if selected { 0 } else { usize::MAX },
        };
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let mut lays = radio_group_layout(
            row,
            &group,
            canvas_ui::component::radio::RadioOrientation::Vertical,
            ControlSize::Sm,
            0.0,
            kit::GAP_CONTROLS,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        if let Some(lay) = lays.pop() {
            k4.radio_cells.push(K4RadioCell {
                state,
                selected,
                lay,
            });
        }
        row_label_at(k4, label_x(slot), y, row_h, state_caption(state));
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

fn tabs_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let labels: Vec<String> = [
        crate::i18n::keys::KIT_DEMO_TAB_1,
        crate::i18n::keys::KIT_DEMO_TAB_2,
        crate::i18n::keys::KIT_DEMO_TAB_3,
    ]
    .iter()
    .map(|k| crate::i18n::tr(lang, k).to_owned())
    .collect();
    let states = [
        KitState::Normal,
        KitState::Hovered,
        KitState::Pressed,
        KitState::Selected,
        KitState::Disabled,
        KitState::Focused,
    ];
    let mut y = slot.y;
    for state in states {
        let active = if state == KitState::Selected || state == KitState::Focused {
            Some(0usize)
        } else {
            None
        };
        let row = UiRect::new(slot.x, y, slot.w, K4_TABS_BAR_H);
        let lay = tabs_layout(
            row,
            &labels,
            active.unwrap_or(usize::MAX),
            canvas_ui::component::tabs::TabStyle::Underline,
            K4_TABS_BAR_H,
            kit::GAP_CONTROLS,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        k4.tabs_rows.push(K4TabsRow {
            state,
            active,
            focused: state == KitState::Focused,
            lay,
        });
        row_label_at(k4, label_x(slot), y, K4_TABS_BAR_H, state_caption(state));
        y += K4_TABS_BAR_H + canvas_core::tokens::SPACING_SM;
    }
}

/// Секция Segmented: 4 состояния строкой (Disabled компонентом не
/// дифференцируется — честная фиксация в §7 use-case). Активный сегмент —
/// Selected (заливка control_primary). Замер не влияет на равные ширины
/// сегментов, но сигнатура компонента принимает &mut TextMeasurer —
/// передаётся общий замер consume (без заглушек).
fn segmented_section(
    k4: &mut K4Sections,
    slot: UiRect,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let row_h = ControlSize::Md.chip_h();
    let labels: Vec<String> = ["1", "2", "3"].iter().map(|s| s.to_string()).collect();
    let mut y = slot.y;
    for state in [
        KitState::Normal,
        KitState::Hovered,
        KitState::Pressed,
        KitState::Selected,
    ] {
        let active = (state == KitState::Selected).then_some(0usize);
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let lay = segmented_layout(
            row,
            &labels,
            active.unwrap_or(usize::MAX),
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        k4.segmented_rows
            .push(K4SegmentedRow { state, active, lay });
        row_label_at(k4, label_x(slot), y, row_h, state_caption(state));
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

// --- Wave C: палитра команд / аккордеон / прогресс / скелетон / бейдж /
// поповер / снекбар / аватар / дерево ---------------------------------------

/// Секция CommandPalette — окно ЦЕЛИКОМ (input + 3 строки результата):
/// K4 требует живой образец компонента, а не якорь-кнопку открытия
/// (якорь остаётся в интерактивной demo-секции Wave C). Верхний отступ
/// компонента — [`K4_PALETTE_TOP_OFFSET`] (внутренний литерал раскладки
/// палитры); ширина окна — формула компонента (560, кламп во вьюпорт).
fn palette_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let results = [0usize, 1, 2];
    let lay = command_palette_layout(
        slot,
        &results,
        // Выбранная строка — третья (Selected в row_states).
        2,
        kit::TEXT_FIELD_HEIGHT,
        kit::LIST_ROW_H,
        3,
        m,
        fs,
    );
    k4.palette = Some(K4PaletteDemo {
        lay,
        query: K4Caption::Key(crate::i18n::keys::KIT_COMMAND_QUERY),
        actions: [
            K4Caption::Key(crate::i18n::keys::KIT_COMMAND_1),
            K4Caption::Key(crate::i18n::keys::KIT_COMMAND_2),
            K4Caption::Key(crate::i18n::keys::KIT_COMMAND_3),
        ]
        .to_vec(),
        row_states: vec![KitState::Normal, KitState::Hovered, KitState::Selected],
    });
    let _ = lang; // подписи — K4Caption::Key (разрешение в отрисовке)
}

/// Секция Accordion: 4 строки — Normal/Hovered/Pressed свёрнуты, Selected
/// раскрыт (тело видимо — [`crate::kit_ui::DEMO_ACCORDION_BODY_H`]).
/// Disabled/Error компонентным стилем не дифференцируются (accordion_style:
/// hover_fill только для Hovered/Pressed) — честная фиксация в §7.
fn accordion_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let rows: [(KitState, bool); 4] = [
        (KitState::Normal, false),
        (KitState::Hovered, false),
        (KitState::Pressed, false),
        (KitState::Selected, true),
    ];
    let mut y = slot.y;
    for (state, expanded) in rows {
        let row = UiRect::new(
            slot.x,
            y,
            slot.w,
            K4_ACCORDION_HEADER_H
                + if expanded {
                    crate::kit_ui::DEMO_ACCORDION_BODY_H
                } else {
                    0.0
                },
        );
        let lay = accordion_layout(
            row,
            "",
            expanded,
            K4_ACCORDION_HEADER_H,
            crate::kit_ui::DEMO_ACCORDION_BODY_H,
            K4_ACCORDION_CHEVRON_W,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        k4.accordion_rows.push(K4AccordionRow {
            state,
            label: K4Caption::Key(crate::i18n::keys::KIT_DEMO_ACCORDION_1),
            body: K4Caption::Key(crate::i18n::keys::KIT_DEMO_ACCORDION_BODY),
            lay,
        });
        row_label_at(
            k4,
            label_x(slot),
            y,
            K4_ACCORDION_HEADER_H,
            state_caption(state),
        );
        y += row.h + canvas_core::tokens::SPACING_SM;
    }
    let _ = lang;
}

/// Ширина колонки шеврона аккордеона (глиф ▸/▾ — контент-размер, не зазор).
pub const K4_ACCORDION_CHEVRON_W: f32 = 16.0;

/// Секция Progress: Linear/LinearIndeterminate/Circular/Spinner (Normal) +
/// Linear Disabled. Радиус круглых — половина строки за вычетом шага.
fn progress_section(k4: &mut K4Sections, slot: UiRect) {
    let row_h = ControlSize::Sm.chip_h();
    let radius = (row_h - 2.0 * canvas_core::tokens::SPACING_S) * 0.5;
    let rows: [(ProgressKind, KitState, K4Caption); 5] = [
        (
            ProgressKind::Linear { value: 0.6 },
            KitState::Normal,
            K4Caption::Raw("Linear"),
        ),
        (
            ProgressKind::LinearIndeterminate,
            KitState::Normal,
            K4Caption::Raw("LinearIndeterminate"),
        ),
        (
            ProgressKind::Circular {
                value: 0.75,
                radius,
            },
            KitState::Normal,
            K4Caption::Raw("Circular"),
        ),
        (
            ProgressKind::Spinner { radius },
            KitState::Normal,
            K4Caption::Raw("Spinner"),
        ),
        (
            ProgressKind::Linear { value: 0.6 },
            KitState::Disabled,
            state_caption(KitState::Disabled),
        ),
    ];
    let mut y = slot.y;
    for (kind, state, label) in rows {
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let lay = progress_layout(row, kind, ControlSize::Sm);
        k4.progress_rows.push(K4ProgressRow {
            kind,
            state,
            label,
            lay,
        });
        row_label_at(k4, label_x(slot), y, row_h, label);
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

/// Секция Skeleton: TextLines (3 строки) + Card + Table 3×3 — статичный
/// кадр пульса [`K4_SKELETON_DEMO_MS`] (витрина не анимируется).
fn skeleton_section(k4: &mut K4Sections, slot: UiRect) {
    let gap_s = canvas_core::tokens::SPACING_S;
    let gap_sm = canvas_core::tokens::SPACING_SM;
    let text_h = 3.0 * K4_SKELETON_ROW_H + 2.0 * gap_s;
    let subslots: [(&'static str, SkeletonPattern, f32); 3] = [
        (
            "TextLines",
            SkeletonPattern::TextLines {
                count: 3,
                row_h: K4_SKELETON_ROW_H,
            },
            text_h,
        ),
        (
            "Card",
            SkeletonPattern::Card {
                w: K4_SKELETON_CARD_W.min(slot.w.max(0.0)),
                h: K4_SKELETON_CARD_H,
            },
            K4_SKELETON_CARD_H,
        ),
        (
            "Table",
            SkeletonPattern::Table { rows: 3, cols: 3 },
            K4_SKELETON_TABLE_H,
        ),
    ];
    let mut y = slot.y;
    for (pattern, pat, h) in subslots {
        let sub = UiRect::new(slot.x, y, slot.w, h);
        let lay = skeleton_layout(sub, &pat, gap_s);
        k4.skeleton_rows.push(K4SkeletonRow { pattern, lay });
        y += h + gap_sm;
    }
}

/// Секция Badge: якорь-чип + бейдж за его верхом (badge_layout позиционирует
/// top-right якоря). Ряд 1 — тона (Primary/Success/Danger), ряд 2 — виды
/// (Count/Dot/Text на Default/Warning/Info).
fn badge_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let cells: [(BadgeTone, BadgeKind, K4Caption); 6] = [
        (
            BadgeTone::Primary,
            BadgeKind::Count(3),
            K4Caption::Raw("Count"),
        ),
        (BadgeTone::Success, BadgeKind::Dot, K4Caption::Raw("Dot")),
        (
            BadgeTone::Danger,
            BadgeKind::Text("NEW".into()),
            K4Caption::Raw("Text"),
        ),
        (
            BadgeTone::Default,
            BadgeKind::Count(12),
            K4Caption::Raw("Count"),
        ),
        (
            BadgeTone::Warning,
            BadgeKind::Text("β".into()),
            K4Caption::Raw("Text"),
        ),
        (BadgeTone::Info, BadgeKind::Dot, K4Caption::Raw("Dot")),
    ];
    let per = ((slot.w - 2.0 * kit::GAP_CONTROLS) / 3.0).max(0.0);
    let mut y = slot.y;
    for (row_idx, row) in cells.chunks(3).enumerate() {
        for (i, (tone, kind, label)) in row.iter().enumerate() {
            let x = slot.x + i as f32 * (per + kit::GAP_CONTROLS);
            let anchor = UiRect::new(x, y + K4_BADGE_HEADROOM, per.min(72.0), kit::CHIP_HEIGHT);
            let lay = badge_layout(anchor, kind.clone(), *tone, m, fs, FONT_FAMILY, LABEL_SIZE);
            k4.badge_cells.push(K4BadgeCell {
                tone: *tone,
                kind: kind.clone(),
                anchor,
                anchor_label: *label,
                lay,
            });
        }
        row_label_at(
            k4,
            label_x(slot),
            y,
            K4_BADGE_CELL_H,
            if row_idx == 0 {
                K4Caption::Key(crate::i18n::keys::KIT_BADGE_ROW_TONES)
            } else {
                K4Caption::Key(crate::i18n::keys::KIT_BADGE_ROW_KINDS)
            },
        );
        y += K4_BADGE_CELL_H + canvas_core::tokens::SPACING_SM;
    }
    let _ = lang;
}

/// Секция Popover: 2 ряда — пузырь ПОД якорем (ряд 1) и НАД якорем (ряд 2,
/// контент в верхней половине ряда — flip не срабатывает: вьюпорт ряда
/// начинается выше якоря ровно на ход стрелки+контента).
fn popover_section(k4: &mut K4Sections, slot: UiRect, lang: Language) {
    let row_h = kit::BUTTON_HEIGHT + K4_POPOVER_ARROW_SPAN + K4_POPOVER_CONTENT.y;
    let mut y = slot.y;
    for below in [true, false] {
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let anchor = if below {
            UiRect::new(slot.x, y, K4_POPOVER_ANCHOR_W, kit::BUTTON_HEIGHT)
        } else {
            UiRect::new(
                slot.x,
                y + K4_POPOVER_ARROW_SPAN + K4_POPOVER_CONTENT.y,
                K4_POPOVER_ANCHOR_W,
                kit::BUTTON_HEIGHT,
            )
        };
        let pl = canvas_ui::component::popover::popover(
            anchor,
            row,
            K4_POPOVER_CONTENT,
            if below {
                AnchoredSide::Bottom
            } else {
                AnchoredSide::Top
            },
            true,
        );
        k4.popover_rows.push(K4PopoverRow {
            below,
            anchor,
            anchor_label: K4Caption::Key(crate::i18n::keys::KIT_FOOTER_OK),
            arrow: pl.arrow,
            content: pl.content,
            body: K4Caption::Key(crate::i18n::keys::KIT_POPOVER_BODY),
        });
        row_label_at(
            k4,
            label_x(slot),
            y,
            row_h,
            K4Caption::Key(if below {
                crate::i18n::keys::KIT_POPOVER_BELOW
            } else {
                crate::i18n::keys::KIT_POPOVER_ABOVE
            }),
        );
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
    let _ = lang;
}

/// Секция Snackbar: 2 бара — Default с действием и Danger без (тон = Error
/// семантика компонента; ST1-состояний, кроме тона, компонент не имеет —
/// snackbar_style без параметра состояния; фиксация в §7).
fn snackbar_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let rows: [(BadgeTone, bool); 2] = [(BadgeTone::Default, true), (BadgeTone::Danger, false)];
    let row_h = kit::BUTTON_HEIGHT + K4_SNACKBAR_MARGIN;
    let mut y = slot.y;
    for (tone, with_action) in rows {
        let row = UiRect::new(slot.x, y, slot.w, row_h);
        let snack = Snackbar {
            message: crate::i18n::tr(lang, crate::i18n::keys::KIT_SNACKBAR_MESSAGE).to_owned(),
            action_label: with_action
                .then(|| crate::i18n::tr(lang, crate::i18n::keys::KIT_SNACKBAR_ACTION).to_owned()),
            ttl_ms: 3000,
            tone,
        };
        let lay = snackbar_layout(
            row,
            &snack,
            kit::BUTTON_HEIGHT,
            kit::CHIP_PAD_H,
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        k4.snackbar_rows.push(K4SnackbarRow {
            tone,
            with_action,
            label: K4Caption::Key(crate::i18n::keys::KIT_SNACKBAR_MESSAGE),
            action: K4Caption::Key(crate::i18n::keys::KIT_SNACKBAR_ACTION),
            lay,
        });
        y += row_h + canvas_core::tokens::SPACING_SM;
    }
}

/// Секция Avatar: ряд 1 — размеры (Lg ×3 источника), ряд 2 — тоны (Md).
fn avatar_section(k4: &mut K4Sections, slot: UiRect) {
    let row1_h = ControlSize::Lg.icon_btn();
    let row2_h = ControlSize::Md.icon_btn();
    let per = ((slot.w - 2.0 * kit::GAP_CONTROLS) / 3.0).max(0.0);
    // Ряд 1: Initials / Icon / Image (Lg).
    let sources = [
        AvatarSource::Initials("AB".to_owned()),
        AvatarSource::Icon("user"),
        AvatarSource::Image(0),
    ];
    for (i, source) in sources.into_iter().enumerate() {
        let cell = UiRect::new(
            slot.x + i as f32 * (per + kit::GAP_CONTROLS),
            slot.y,
            per,
            row1_h,
        );
        k4.avatar_cells.push(K4AvatarCell {
            size: ControlSize::Lg,
            tone: KitBadgeTone::Default,
            source: source.clone(),
            lay: avatar_layout(cell, source, ControlSize::Lg),
        });
    }
    // Ряд 2: тоны Default/Primary/Success (Md).
    let tones = [
        KitBadgeTone::Default,
        KitBadgeTone::Primary,
        KitBadgeTone::Success,
    ];
    let y2 = slot.y + row1_h + canvas_core::tokens::SPACING_SM;
    for (i, tone) in tones.into_iter().enumerate() {
        let cell = UiRect::new(
            slot.x + i as f32 * (per + kit::GAP_CONTROLS),
            y2,
            per,
            row2_h,
        );
        let source = AvatarSource::Initials("CD".to_owned());
        k4.avatar_cells.push(K4AvatarCell {
            size: ControlSize::Md,
            tone,
            source: source.clone(),
            lay: avatar_layout(cell, source, ControlSize::Md),
        });
    }
}

/// Секция Tree: 4 видимые строки (root раскрыт → ветка раскрыта → лист +
/// вторая ветка). Состояния — по строкам: Normal/Hovered/Selected/Disabled
/// (Disabled — семантика «недоступен», честная фиксация в §7).
fn tree_section(
    k4: &mut K4Sections,
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let text = |key: &'static str| crate::i18n::tr(lang, key).to_owned();
    let leaf = TreeNode::new("leaf", text(crate::i18n::keys::KIT_CRUMBS_B));
    let branch1 = TreeNode::new("branch1", text(crate::i18n::keys::KIT_TREE_BRANCH))
        .expanded(true)
        .child(leaf);
    let branch2 = TreeNode::new("branch2", text(crate::i18n::keys::KIT_CRUMBS_C));
    let root = TreeNode::new("root", text(crate::i18n::keys::KIT_CRUMBS_A))
        .expanded(true)
        .child(branch1)
        .child(branch2);
    let rows = tree_layout(
        slot,
        &root,
        kit::LIST_ROW_H,
        K4_TREE_INDENT,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let states = [
        KitState::Normal,
        KitState::Hovered,
        KitState::Selected,
        KitState::Disabled,
    ];
    for (row, state) in rows.into_iter().zip(states) {
        k4.tree_rows.push(K4TreeRow { state, row });
    }
}

// === Отрисовка (адаптер «раскладка → кадр»; один вызов из overlays.rs) =====

/// Рамка фокуса (FR-057): контур accent поверх контрола — рисует
/// потребитель, компонентные стили рамку фокуса не знают.
fn focus_ring(d: &mut crate::kit_ui::KitDraw, rect: UiRect, radius: f32, p: &KitPalette) {
    d.rect(rect, [0.0; 4], p.accent, radius);
}

/// State-layer (M3): overlay-квад с альфой ступени [`state_layer_alpha`]
/// поверх base-fill (цвет overlay = content-слот; FR-057/M3).
fn state_layer(
    d: &mut crate::kit_ui::KitDraw,
    rect: UiRect,
    radius: f32,
    state: KitState,
    p: &KitPalette,
) {
    let a = canvas_ui::component::state_layer_alpha(state);
    if a > 0.0 {
        d.rect(rect, [p.text[0], p.text[1], p.text[2], a], [0.0; 4], radius);
    }
}

/// Отрисовка K4-секций: единственный вызов из `App::kit_gallery_overlay`
/// (хвост витрины, после существующих секций). Геометрия — ТОЛЬКО из
/// [`K4Sections`] (hit=draw — K4.2); стили — компонентные функции от
/// слотов палитры (K4.1); интерактива нет — hit-слотов не добавляет.
pub(crate) fn k4_paint(
    d: &mut crate::kit_ui::KitDraw,
    k4: &K4Sections,
    p: &KitPalette,
    lang: Language,
) {
    let text = |cap: K4Caption| cap.text(lang).to_owned();

    // Подписи строк состояний/вариантов (левая колонка, 11px muted —
    // паттерн подписей Buttons в overlays.rs).
    for (area, cap) in &k4.labels {
        // Область построена в row_label_at через kit::text_area_vcentered —
        // дублировать канон здесь не нужно.
        d.label_left(*area, &text(*cap), p.text_muted, 11.0); // v3:allow компонентный строковый бокс row_label_at (IN13)
    }

    // --- W2 ----------------------------------------------------------------

    // Banner: контейнер (paint_banner = один Rect) + подпись + кнопка
    // действия; Focused — рамка accent, Dragged — state-layer 0.16.
    for r in &k4.banner_rows {
        let (_, style) = banner(r.lay.rect, r.label_w, r.action_w, r.kind, r.state, p);
        d.paint_items(paint_banner(&r.lay, &style));
        d.label_left(
            r.lay.label_area,
            &text(K4Caption::Key(crate::i18n::keys::KIT_BANNER_LABEL)),
            style.label_color,
            12.0,
        ); // v3:allow компонентный строковый бокс banner BANNER_LABEL_LINE_H (IN13)
        d.control(
            r.lay.action_button,
            &kit::button_style(ButtonVariant::Secondary, r.state, p),
        );
        d.label_center(
            kit::text_area_vcentered(r.lay.action_button, 12.0),
            &text(K4Caption::Key(crate::i18n::keys::KIT_BANNER_ACTION)),
            style.action_color,
            12.0,
        );
        if r.focused {
            focus_ring(d, r.lay.rect, style.radius, p);
        }
        if r.dragged {
            state_layer(d, r.lay.rect, style.radius, KitState::Dragged, p);
        }
    }

    // ChatBubble: контейнер (paint_chat_bubble = один Rect) + header
    // (автор) + строка сообщения. KitState компонентом не принимается —
    // вид (Normal/Error/Success) = семантика (фиксация в §7).
    for r in &k4.chat_rows {
        let (_, style) = chat_bubble(r.lay.rect, 1, 0, r.kind, p, true);
        d.paint_items(paint_chat_bubble(&r.lay, &style));
        if let Some(h) = r.lay.header_area {
            d.label_left(
                h,
                &text(K4Caption::Key(crate::i18n::keys::KIT_CHAT_AUTHOR)),
                style.text_color,
                9.0,
            ); // v3:allow компонентный строковый бокс chat_bubble CHAT_HEADER_H (IN13)
        }
        let msg_key = match r.kind {
            ChatBubbleKind::Normal => crate::i18n::keys::KIT_CHAT_MESSAGE,
            ChatBubbleKind::Error | ChatBubbleKind::Success => crate::i18n::keys::KIT_CHAT_SUCCESS,
        };
        d.label_left(
            r.lay.text_area,
            &text(K4Caption::Key(msg_key)),
            style.text_color,
            11.0,
        ); // v3:allow компонентный стек строк chat_bubble CHAT_LINE_H (IN11)
    }

    // Crumbs: чипы; активная крошка — Selected (crumbs_active_index),
    // «…» — Normal.
    for r in &k4.crumbs_rows {
        for (i, (rect, label)) in r.chips.iter().enumerate() {
            let active = r.active == Some(i);
            let style = kit::chip_style(
                if active {
                    KitState::Selected
                } else {
                    KitState::Normal
                },
                p,
            );
            d.control(*rect, &style);
            d.label_center(
                kit::text_area_vcentered(*rect, 11.0),
                label,
                style.text,
                11.0,
            );
        }
    }

    // Footer: кнопки вариантов со своими состояниями; Focused — рамка.
    for r in &k4.footer_rows {
        for b in &r.buttons {
            let style = kit::button_style(b.variant, b.state, p);
            d.control(b.rect, &style);
            d.label_center(
                kit::text_area_vcentered(b.rect, 12.0),
                &text(b.label),
                style.text,
                12.0,
            );
            if b.focused {
                focus_ring(d, b.rect, style.radius, p);
            }
        }
    }

    // RadioCard: фон карточки + индикатор (paint_radio_card) + label/desc
    // (тексты — забота потребителя).
    for c in &k4.radio_card_cells {
        let (_, style) = radio_card(c.lay.rect, c.label_w, c.desc_w, c.selected, c.state, p);
        d.paint_items(paint_radio_card(&c.lay, &style));
        d.label_left(
            c.lay.label,
            &text(K4Caption::Key(crate::i18n::keys::KIT_RADIO_CARD_LABEL)),
            style.label_color,
            12.0,
        ); // v3:allow компонентный строковый бокс radio_card RADIO_LABEL_LINE_H (IN13)
        if let Some(desc) = c.lay.desc {
            // Область — компонентный бокс RADIO_DESC_LINE_H из radio_card.
            d.label_left(desc, "LaTeX · MathML", style.desc_color, 11.0); // v3:allow компонентный строковый бокс radio_card (IN13)
        }
    }

    // TwoColumn: панели sidebar/content (панельный хром) + подписи.
    for r in &k4.two_column_rows {
        for (rect, key) in [
            (r.sidebar, crate::i18n::keys::KIT_TWO_COLUMN_SIDEBAR),
            (r.content, crate::i18n::keys::KIT_TWO_COLUMN_CONTENT),
        ] {
            d.rect(
                rect,
                p.panel_fill,
                p.panel_border,
                canvas_core::tokens::RADIUS_PANEL,
            );
            // Канон вертикали (issue #68, IN11): строковый бокс подписи
            // панели — по центру панели; x-инсеты SPACING_SM сохранены.
            d.label_left(
                kit::text_area_vcentered(
                    UiRect::new(
                        rect.x + canvas_core::tokens::SPACING_SM,
                        rect.y,
                        (rect.w - 2.0 * canvas_core::tokens::SPACING_SM).max(0.0),
                        rect.h,
                    ),
                    11.0,
                ),
                &text(K4Caption::Key(key)),
                p.text_muted,
                11.0,
            );
        }
    }

    // --- Wave C ------------------------------------------------------------

    // Checkbox: чип-контрол по состоянию (checkbox_style) + глиф + подпись;
    // Focused — рамка вокруг чипа.
    for c in &k4.checkbox_cells {
        let style = checkbox_style(c.check, c.kit_state, p);
        d.control(c.lay.box_rect, &style);
        d.label_center(
            kit::text_area_vcentered(c.lay.box_rect, 11.0),
            checkbox_glyph(c.check),
            style.text,
            11.0,
        );
        d.label_left(
            kit::text_area_vcentered(c.lay.label_rect, 12.0),
            &text(K4Caption::Key(crate::i18n::keys::KIT_CHECKBOX_LABEL)),
            if c.kit_state == KitState::Disabled {
                p.disabled_text
            } else {
                p.text
            },
            12.0,
        );
        if c.focused {
            focus_ring(d, c.lay.box_rect, style.radius, p);
        }
    }

    // Slider: трек + fill + ручка (slider_style: (контрол, цвет ручки));
    // состояния, которые компонент не дифференцирует, наследуют Normal.
    for r in &k4.slider_rows {
        let (style, knob_fill) = slider_style(r.state, p);
        d.rect(r.lay.track, style.fill, style.border, style.radius);
        d.rect(r.lay.filled, knob_fill, [0.0; 4], style.radius);
        d.control(
            r.lay.knob,
            &ControlStyle {
                fill: knob_fill,
                border: [0.0; 4],
                text: p.text_title,
                radius: canvas_core::tokens::RADIUS_PILL,
                elevation: canvas_ui::component::Elevation::None,
            },
        );
    }

    // Radio: круг + точка выбора (radio_style) + подпись.
    for c in &k4.radio_cells {
        let style = radio_style(c.selected, c.state, p);
        d.control(
            c.lay.circle,
            &ControlStyle {
                radius: c.lay.circle.w * 0.5,
                ..style
            },
        );
        if c.selected {
            let dot = c
                .lay
                .circle
                .inset(&canvas_ui::geometry::EdgeInsets::uniform(
                    c.lay.circle.w * 0.25,
                ));
            d.rect(dot, style.fill, [0.0; 4], dot.w * 0.5);
        }
        d.label_left(
            kit::text_area_vcentered(c.lay.label_rect, 12.0),
            &text(K4Caption::Key(crate::i18n::keys::KIT_DEMO_TAB_1)),
            if c.state == KitState::Disabled {
                p.disabled_text
            } else {
                p.text
            },
            12.0,
        );
    }

    // Tabs: заливка активного/ховера + индикатор + подписи; Focused —
    // рамка на активной вкладке.
    for r in &k4.tabs_rows {
        for (i, t) in r.lay.tabs.iter().enumerate() {
            let active = r.active == Some(i);
            let st = tab_style(
                active,
                r.state,
                canvas_ui::component::tabs::TabStyle::Underline,
                p,
            );
            if st.fill[3] > 0.0 {
                d.rect(t.rect, st.fill, [0.0; 4], st.radius);
            }
            d.label_center(
                kit::text_area_vcentered(t.rect, 12.0),
                &t.label,
                st.text,
                12.0,
            );
            if active && t.indicator.h > 0.0 {
                d.rect(t.indicator, st.indicator_fill, [0.0; 4], 0.0);
            }
            if r.focused && active {
                focus_ring(d, t.rect, st.radius, p);
            }
        }
    }

    // Segmented: контейнер + сегменты (активный — заливка primary).
    for r in &k4.segmented_rows {
        let (container, active_style) = segmented_style(false, r.state, p);
        d.control(r.lay.container, &container);
        for (i, seg) in r.lay.segments.iter().enumerate() {
            let active = r.active == Some(i);
            let style = if active {
                segmented_style(true, r.state, p).1
            } else {
                container
            };
            let _ = active_style;
            d.control(*seg, &style);
            d.label_center(
                kit::text_area_vcentered(*seg, 12.0),
                &((i + 1).to_string()),
                style.text,
                12.0,
            );
        }
    }

    // CommandPalette: окно целиком — input (текстовое поле с запросом) +
    // строки результата (Normal/Hovered/Selected).
    if let Some(pl) = &k4.palette {
        d.rect(
            pl.lay.input,
            p.control_fill,
            p.control_border,
            canvas_core::tokens::RADIUS_CHIP,
        );
        d.label_left(
            kit::text_area_vcentered(
                UiRect::new(
                    pl.lay.input.x + kit::TEXT_FIELD_PAD_H,
                    pl.lay.input.y,
                    (pl.lay.input.w - 2.0 * kit::TEXT_FIELD_PAD_H).max(0.0),
                    pl.lay.input.h,
                ),
                12.0,
            ),
            &text(pl.query),
            p.text_muted,
            12.0,
        );
        for (i, (rect, action_idx)) in pl.lay.results.iter().enumerate() {
            let state = pl.row_states.get(i).copied().unwrap_or(KitState::Normal);
            let style = kit::chip_style(state, p);
            d.rect(*rect, style.fill, [0.0; 4], 4.0);
            if let Some(cap) = pl.actions.get(*action_idx) {
                d.label_left(
                    kit::text_area_vcentered(
                        UiRect::new(
                            rect.x + kit::CHIP_PAD_H,
                            rect.y,
                            (rect.w - 2.0 * kit::CHIP_PAD_H).max(0.0),
                            rect.h,
                        ),
                        12.0,
                    ),
                    &text(*cap),
                    style.text,
                    12.0,
                );
            }
        }
    }

    // Accordion: заголовок (контрол + шеврон + подпись); Selected раскрыт —
    // тело панелью с текстом.
    for r in &k4.accordion_rows {
        let (style, chevron_color) = accordion_style(r.lay.expanded, r.state, p);
        d.control(r.lay.header, &style);
        let chevron = if r.lay.expanded { "▾" } else { "▸" };
        d.label_center(
            kit::text_area_vcentered(r.lay.chevron, 11.0),
            chevron,
            chevron_color,
            11.0,
        );
        d.label_left(
            kit::text_area_vcentered(
                UiRect::new(
                    r.lay.chevron.right() + 4.0,
                    r.lay.header.y,
                    (r.lay.header.right() - r.lay.chevron.right() - 8.0).max(0.0),
                    K4_ACCORDION_HEADER_H,
                ),
                12.0,
            ),
            &text(r.label),
            style.text,
            12.0,
        );
        if r.lay.expanded && r.lay.content.h > 0.0 {
            d.rect(r.lay.content, p.panel_fill, [0.0; 4], 0.0);
            d.label_left(
                UiRect::new(
                    r.lay.content.x + canvas_core::tokens::SPACING_SM,
                    r.lay.content.y + canvas_core::tokens::SPACING_SM,
                    (r.lay.content.w - 2.0 * canvas_core::tokens::SPACING_SM).max(0.0),
                    14.0,
                ),
                &text(r.body),
                p.text_muted,
                11.0,
            ); // v3:allow тело аккордеона — топ-анкерный контент-стек с токен-паддингом (IN11)
        }
    }

    // Progress: трек + fill (Linear); Circular/Spinner — кольцо-рамка
    // (круг border'ом — паттерн «кольцо» демо Wave C). Disabled — слоты
    // progress_style(Disabled).
    for r in &k4.progress_rows {
        let (style, fill_color) = progress_style(r.state, p);
        match r.kind {
            ProgressKind::Linear { .. } | ProgressKind::LinearIndeterminate => {
                d.rect(r.lay.track, style.fill, style.border, 2.0);
                d.rect(r.lay.fill, fill_color, [0.0; 4], 2.0);
            }
            ProgressKind::Circular { .. } | ProgressKind::Spinner { .. } => {
                d.rect(r.lay.track, [0.0; 4], fill_color, r.lay.track.w * 0.5);
            }
        }
    }

    // Skeleton: блоки-плейсхолдеры со статичной альфой пульса
    // ([`K4_SKELETON_DEMO_MS`]; витрина статична — анимации нет).
    let alpha = skeleton_alpha(K4_SKELETON_DEMO_MS);
    let (prog_style, _) = progress_style(KitState::Normal, p);
    let mut skel_fill = prog_style.fill;
    skel_fill[3] = alpha;
    for r in &k4.skeleton_rows {
        for rect in &r.lay.rects {
            d.rect(*rect, skel_fill, [0.0; 4], 4.0);
        }
    }

    // Badge: якорь-чип + бейдж (badge_style по тону; Count/Text — подпись).
    for c in &k4.badge_cells {
        let anchor_style = kit::chip_style(KitState::Normal, p);
        d.control(c.anchor, &anchor_style);
        d.label_center(
            kit::text_area_vcentered(c.anchor, 11.0),
            &text(c.anchor_label),
            anchor_style.text,
            11.0,
        );
        let style = badge_style(c.tone, p);
        d.control(c.lay.rect, &style);
        match &c.lay.kind {
            BadgeKind::Count(n) => d.label_center(
                kit::text_area_vcentered(c.lay.rect, 9.0),
                &n.to_string(),
                style.text,
                9.0,
            ),
            BadgeKind::Text(t) => d.label_center(
                kit::text_area_vcentered(c.lay.rect, 9.0),
                t,
                style.text,
                9.0,
            ),
            BadgeKind::Dot => {}
        }
    }

    // Popover: якорь-кнопка + пузырь (панельный хром) + стрелка + тело.
    for r in &k4.popover_rows {
        let anchor_style = kit::button_style(ButtonVariant::Secondary, KitState::Normal, p);
        d.control(r.anchor, &anchor_style);
        d.label_center(
            kit::text_area_vcentered(r.anchor, 12.0),
            &text(r.anchor_label),
            anchor_style.text,
            12.0,
        );
        d.rect(
            r.content,
            p.panel_fill,
            p.panel_border,
            canvas_core::tokens::RADIUS_PANEL,
        );
        d.label_left(
            UiRect::new(
                r.content.x + canvas_core::tokens::SPACING_SM,
                r.content.y + canvas_core::tokens::SPACING_SM,
                (r.content.w - 2.0 * canvas_core::tokens::SPACING_SM).max(0.0),
                14.0,
            ),
            &text(r.body),
            p.text_muted,
            11.0,
        ); // v3:allow тело поповера — топ-анкерный контент-стек с токен-паддингом (IN11)
        if r.arrow.w > 0.0 {
            d.rect(r.arrow, p.panel_fill, [0.0; 4], 2.0);
        }
    }

    // Snackbar: бар (тон) + сообщение + опц. действие.
    for r in &k4.snackbar_rows {
        let style = snackbar_style(r.tone, p);
        d.control(r.lay.rect, &style);
        d.label_left(
            kit::text_area_vcentered(r.lay.message_rect, 12.0),
            &text(r.label),
            style.text,
            12.0,
        );
        if let Some(a) = r.lay.action_rect {
            d.label_center(
                kit::text_area_vcentered(a, 12.0),
                &text(r.action),
                style.text,
                12.0,
            );
        }
    }

    // Avatar: круг (avatar_style по тону; radius = половина стороны) +
    // содержимое (инициалы/иконка/плейсхолдер).
    for c in &k4.avatar_cells {
        let style = avatar_style(c.tone, p);
        d.control(
            c.lay.rect,
            &ControlStyle {
                radius: c.lay.rect.w * 0.5,
                ..style
            },
        );
        let glyph_size = c.size.button_h() * 0.45;
        match &c.source {
            AvatarSource::Initials(s) => d.label_center(
                kit::text_area_vcentered(c.lay.rect, glyph_size),
                s,
                style.text,
                glyph_size,
            ),
            AvatarSource::Icon(_) => d.label_center(
                kit::text_area_vcentered(c.lay.rect, glyph_size),
                "user",
                style.text,
                glyph_size,
            ),
            AvatarSource::Image(_) => d.label_center(
                kit::text_area_vcentered(c.lay.rect, glyph_size * 0.7),
                "IMG",
                style.text,
                glyph_size * 0.7,
            ),
        }
    }

    // Tree: строки (Ghost-кнопки — паттерн demo-дерева Wave C) + шеврон +
    // подпись; состояния — по строкам.
    for r in &k4.tree_rows {
        let style = kit::button_style(ButtonVariant::Ghost, r.state, p);
        d.control(r.row.rect, &style);
        if r.row.has_children {
            let chevron = if r.row.expanded { "▾" } else { "▸" };
            d.label_center(
                kit::text_area_vcentered(r.row.chevron, 10.0),
                chevron,
                style.text,
                10.0,
            );
        }
        d.label_left(
            kit::text_area_vcentered(r.row.label, 12.0),
            &r.row.label_text,
            style.text,
            12.0,
        );
    }
}

// === Golden-тесты секций (issue #62; паттерн golden-ов kit_ui.rs) ==========

#[cfg(test)]
mod k4_golden {
    use super::*;
    use crate::kit_ui::{gallery_layout, new_measurer, SECTION_GAP, SECTION_TITLE_H};
    use canvas_core::tokens::SPACING_SM;
    use canvas_render::text::measure_font_system;
    use std::iter::once;

    /// Допуск геометрии (сравнение f32 — значения из целых констант).
    const EPS: f32 = 0.01;

    /// Альтернативная палитра для [`k4_layout_geometry_palette_free`]
    /// (значения слотов иные — геометрия от них зависеть не должна).
    fn alt_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.9; 4],
            panel_border: [0.8; 4],
            control_fill: [0.7; 4],
            control_border: [0.6; 4],
            control_primary: [0.5; 4],
            control_danger: [0.4; 4],
            hover_fill: [0.3; 4],
            primary_hover_fill: [0.25; 4],
            selected_fill: [0.2; 4],
            text: [0.1; 4],
            text_title: [0.09; 4],
            text_muted: [0.08; 4],
            disabled_text: [0.07; 4],
            accent: [0.06; 4],
            control_success: [0.10, 0.65, 0.45, 1.0],
            control_warning: [0.85, 0.55, 0.20, 1.0],
            stage_dim: [0.04, 0.04, 0.08, 0.6],
            scrollbar_thumb: [0.45, 0.48, 0.56, 0.7],
            rule_color: [0.20, 0.23, 0.30, 0.8],
        }
    }

    /// Синтетическая линейка K4-хвоста — те же 2 rect'а на секцию (подпись,
    /// контент), что читает [`k4_consume`] из `gallery_layout_impl`; шаг
    /// SECTION_GAP — как между секциями реальной линейки. Старт y = 100
    /// (не 0 — ловит забытый slot.y). Возвращает секции и слоты по id.
    fn consume_synthetic(
        lang: Language,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
    ) -> (K4Sections, Vec<(&'static str, UiRect)>) {
        let full_w = 1024.0;
        let control_x = 150.0;
        let control_w = full_w - control_x - 40.0;
        let mut items: Vec<UiRect> = Vec::new();
        let mut slots: Vec<(&'static str, UiRect)> = Vec::new();
        let mut y = 100.0;
        for id in K4_ORDER {
            items.push(UiRect::new(0.0, y, full_w, SECTION_TITLE_H));
            y += SECTION_TITLE_H;
            let h = k4_section_h(id);
            slots.push((id, UiRect::new(control_x, y, control_w, h)));
            items.push(UiRect::new(0.0, y, full_w, h));
            y += h + SECTION_GAP;
        }
        let mut titles = Vec::new();
        let mut it = items.into_iter();
        let k4 = k4_consume(&mut it, control_x, control_w, lang, m, fs, &mut titles);
        (k4, slots)
    }

    /// Все rect'ы секции из раскладки (для инвариантов границ слота).
    fn section_rects(k4: &K4Sections, id: &str) -> Vec<UiRect> {
        match id {
            "banner" => k4.banner_rows.iter().map(|r| r.lay.rect).collect(),
            "chat_bubble" => k4.chat_rows.iter().map(|r| r.lay.rect).collect(),
            "crumbs" => k4
                .crumbs_rows
                .iter()
                .flat_map(|r| r.chips.iter().map(|(c, _)| *c))
                .collect(),
            "footer" => k4
                .footer_rows
                .iter()
                .flat_map(|r| r.buttons.iter().map(|b| b.rect))
                .collect(),
            "radio_card" => k4.radio_card_cells.iter().map(|c| c.lay.rect).collect(),
            "two_column" => k4
                .two_column_rows
                .iter()
                .flat_map(|r| [r.sidebar, r.content])
                .collect(),
            "checkbox" => k4
                .checkbox_cells
                .iter()
                .flat_map(|c| [c.lay.box_rect, c.lay.label_rect])
                .collect(),
            "slider" => k4
                .slider_rows
                .iter()
                .flat_map(|r| [r.lay.track, r.lay.filled, r.lay.knob])
                .collect(),
            "radio" => k4
                .radio_cells
                .iter()
                .flat_map(|c| [c.lay.circle, c.lay.label_rect])
                .collect(),
            "tabs" => k4
                .tabs_rows
                .iter()
                .flat_map(|r| once(r.lay.bar).chain(r.lay.tabs.iter().map(|t| t.rect)))
                .collect(),
            "segmented" => k4
                .segmented_rows
                .iter()
                .flat_map(|r| {
                    once(r.lay.container)
                        // active_rect — геометрия заливки только у Selected;
                        // у прочих строк это сигнальный нулевой rect
                        // «нет активного сегмента» (не геометрия слота).
                        .chain(r.active.is_some().then_some(r.lay.active_rect))
                        .chain(r.lay.segments.iter().copied())
                })
                .collect(),
            "command_palette" => match &k4.palette {
                // Окно целиком либо в раскладке, либо отброшено
                // shift_and_filter (FR-059: частичное окно не рисуется).
                Some(pl) => once(pl.lay.input)
                    .chain(pl.lay.results.iter().map(|(r, _)| *r))
                    .collect(),
                None => Vec::new(),
            },
            "accordion" => k4
                .accordion_rows
                .iter()
                .flat_map(|r| [r.lay.header, r.lay.chevron, r.lay.content])
                .collect(),
            "progress" => k4
                .progress_rows
                .iter()
                .flat_map(|r| [r.lay.track, r.lay.fill])
                .collect(),
            "skeleton" => k4
                .skeleton_rows
                .iter()
                .flat_map(|r| r.lay.rects.iter().copied())
                .collect(),
            "badge" => k4
                .badge_cells
                .iter()
                .flat_map(|c| [c.anchor, c.lay.rect])
                .collect(),
            "popover" => k4
                .popover_rows
                .iter()
                .flat_map(|r| [r.anchor, r.arrow, r.content])
                .collect(),
            "snackbar" => k4.snackbar_rows.iter().map(|r| r.lay.rect).collect(),
            "avatar" => k4.avatar_cells.iter().map(|c| c.lay.rect).collect(),
            "tree" => k4.tree_rows.iter().map(|r| r.row.rect).collect(),
            _ => Vec::new(),
        }
    }

    /// Порядок и состав K4-ORDER: 20 уникальных id — 6 W2, затем 14 Wave C
    /// (порядок тела issue #62).
    #[test]
    fn k4_order_is_20_w2_then_wave_c() {
        const W2: [&str; 6] = [
            "banner",
            "chat_bubble",
            "crumbs",
            "footer",
            "radio_card",
            "two_column",
        ];
        const WAVE_C: [&str; 14] = [
            "checkbox",
            "slider",
            "radio",
            "tabs",
            "segmented",
            "command_palette",
            "accordion",
            "progress",
            "skeleton",
            "badge",
            "popover",
            "snackbar",
            "avatar",
            "tree",
        ];
        assert_eq!(K4_ORDER.len(), 20);
        let mut uniq = K4_ORDER.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 20, "id секций уникальны");
        assert_eq!(&K4_ORDER[..6], &W2, "первые 6 — W2");
        assert_eq!(&K4_ORDER[6..], &WAVE_C, "хвост — 14 Wave C");
    }

    /// Метки секций зарегистрированы в i18n RU и EN (tr не отдаёт ключ) и
    /// различаются между языками.
    #[test]
    fn k4_section_titles_registered_ru_en() {
        for id in K4_ORDER {
            let key = k4_title_key(id);
            let ru = crate::i18n::tr(Language::Ru, key);
            let en = crate::i18n::tr(Language::En, key);
            assert_ne!(ru, key, "RU-метка секции {id} не зарегистрирована");
            assert_ne!(en, key, "EN-метка секции {id} не зарегистрирована");
            assert_ne!(ru, en, "метка секции {id} различает RU/EN");
        }
    }

    /// Формула высоты [`k4_section_h`] = фактический контент секции:
    /// ряды начинаются от верха слота, помещаются в него БЕЗ переполнения
    /// (G5: silent clamps запрещены) и последний ряд оканчивается ровно на
    /// нижней кромке слота — формула строк едина с раскладкой.
    #[test]
    fn k4_sections_rows_fit_block() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let (k4, slots) = consume_synthetic(Language::Ru, &mut m, &mut fs);
        // Количество строк/ячеек по секциям (демо-состав из body issue #62).
        let counts: &[(&str, usize)] = &[
            ("banner", 7), // 3 пары (6 ячеек) + Error во всю ширину
            ("chat_bubble", 3),
            ("footer", 5),     // 2 + 3 кнопки в двух строках
            ("radio_card", 5), // 3 + 2 карточки (дубликат Normal не показываем)
            ("two_column", 4), // 2 строки × (sidebar + content)
            ("checkbox", 7),
            ("slider", 4),
            ("radio", 5),
            ("tabs", 6),
            ("segmented", 4),
            ("command_palette", 1), // Option palette
            ("accordion", 4),
            ("progress", 5),
            ("skeleton", 3),
            ("badge", 6),
            ("popover", 2),
            ("snackbar", 2),
            ("avatar", 6),
            ("tree", 4),
        ];
        for (id, want) in counts {
            let n = match *id {
                "banner" => k4.banner_rows.len(),
                "chat_bubble" => k4.chat_rows.len(),
                "crumbs" => k4.crumbs_rows.len(),
                "footer" => k4.footer_rows.iter().map(|r| r.buttons.len()).sum(),
                "radio_card" => k4.radio_card_cells.len(),
                "two_column" => k4.two_column_rows.len() * 2,
                "checkbox" => k4.checkbox_cells.len(),
                "slider" => k4.slider_rows.len(),
                "radio" => k4.radio_cells.len(),
                "tabs" => k4.tabs_rows.len(),
                "segmented" => k4.segmented_rows.len(),
                "command_palette" => k4.palette.is_some().into(),
                "accordion" => k4.accordion_rows.len(),
                "progress" => k4.progress_rows.len(),
                "skeleton" => k4.skeleton_rows.len(),
                "badge" => k4.badge_cells.len(),
                "popover" => k4.popover_rows.len(),
                "snackbar" => k4.snackbar_rows.len(),
                "avatar" => k4.avatar_cells.len(),
                "tree" => k4.tree_rows.len(),
                _ => usize::MAX,
            };
            assert_eq!(n, *want, "количество контента секции {id}");
        }
        // Инвариант границ: весь контент внутри слота секции (вертикаль и
        // горизонталь, допуск EPS).
        for (id, slot) in &slots {
            let rects = section_rects(&k4, id);
            assert!(!rects.is_empty(), "секция {id} без контента");
            for r in &rects {
                assert!(
                    r.y >= slot.y - EPS
                        && r.bottom() <= slot.bottom() + EPS
                        && r.x >= slot.x - EPS
                        && r.right() <= slot.right() + EPS,
                    "секция {id}: rect {:?} вне слота {:?}",
                    r,
                    slot
                );
            }
        }
        // Флеш-посадка: последний внешний rect каждой секции оканчивается
        // ровно на нижней кромке слота (формула строк = раскладка; для
        // palette — последняя строка результатов на SPACING_SM выше кромки,
        // для snackbar — бар на внутренней нижней марже компонента 24 px,
        // для badge — якорь на 6 px выше кромки ячейки).
        let slot = |id: &str| slots.iter().find(|(i, _)| *i == id).unwrap().1;
        let flush = |got: f32, id: &str, want: f32| {
            assert!(
                (got - want).abs() < EPS,
                "секция {id}: нижняя кромка {got} != {want}"
            );
        };
        flush(
            k4.banner_rows.last().unwrap().lay.rect.bottom(),
            "banner",
            slot("banner").bottom(),
        );
        flush(
            k4.chat_rows.last().unwrap().lay.rect.bottom(),
            "chat_bubble",
            slot("chat_bubble").bottom(),
        );
        flush(
            k4.footer_rows
                .last()
                .unwrap()
                .buttons
                .iter()
                .map(|b| b.rect.bottom())
                .fold(f32::MIN, f32::max),
            "footer",
            slot("footer").bottom(),
        );
        flush(
            k4.radio_card_cells
                .iter()
                .map(|c| c.lay.rect.bottom())
                .fold(f32::MIN, f32::max),
            "radio_card",
            slot("radio_card").bottom(),
        );
        flush(
            k4.two_column_rows
                .iter()
                .map(|r| r.sidebar.bottom())
                .fold(f32::MIN, f32::max),
            "two_column",
            slot("two_column").bottom(),
        );
        flush(
            k4.checkbox_cells
                .iter()
                .map(|c| c.lay.box_rect.bottom())
                .fold(f32::MIN, f32::max),
            "checkbox",
            slot("checkbox").bottom(),
        );
        flush(
            k4.radio_cells
                .iter()
                .map(|c| c.lay.circle.bottom())
                .fold(f32::MIN, f32::max),
            "radio",
            slot("radio").bottom(),
        );
        flush(
            k4.tabs_rows.last().unwrap().lay.bar.bottom(),
            "tabs",
            slot("tabs").bottom(),
        );
        flush(
            k4.segmented_rows.last().unwrap().lay.container.bottom(),
            "segmented",
            slot("segmented").bottom(),
        );
        let pl = k4.palette.as_ref().unwrap();
        assert!(
            (pl.lay.input.y - (slot("command_palette").y + K4_PALETTE_TOP_OFFSET)).abs() < EPS,
            "палитра: input на верхнем отступе компонента"
        );
        flush(
            pl.lay.results.last().unwrap().0.bottom(),
            "command_palette",
            slot("command_palette").bottom() - SPACING_SM,
        );
        flush(
            k4.accordion_rows
                .iter()
                .map(|r| r.lay.content.bottom())
                .fold(f32::MIN, f32::max),
            "accordion",
            slot("accordion").bottom(),
        );
        flush(
            k4.skeleton_rows
                .iter()
                .flat_map(|r| r.lay.rects.iter().map(|q| q.bottom()))
                .fold(f32::MIN, f32::max),
            "skeleton",
            slot("skeleton").bottom(),
        );
        flush(
            k4.badge_cells
                .iter()
                .map(|c| c.anchor.bottom())
                .fold(f32::MIN, f32::max),
            "badge",
            slot("badge").bottom() - 6.0,
        );
        let pop = k4.popover_rows.last().unwrap();
        assert!(
            (!pop.below
                && (pop.anchor.bottom().max(pop.content.bottom()) - slot("popover").bottom())
                    .abs()
                    < EPS),
            "popover: у ряда Above пузырь прижат к верху строки, кромку слота \
             держит якорь (bottom = якорь/пузырь последнего ряда)"
        );
        flush(
            k4.snackbar_rows
                .iter()
                .map(|r| r.lay.rect.bottom())
                .fold(f32::MIN, f32::max),
            "snackbar",
            slot("snackbar").bottom() - K4_SNACKBAR_MARGIN,
        );
        flush(
            k4.avatar_cells
                .iter()
                .map(|c| c.lay.rect.bottom())
                .fold(f32::MIN, f32::max),
            "avatar",
            slot("avatar").bottom(),
        );
        flush(
            k4.tree_rows.last().unwrap().row.rect.bottom(),
            "tree",
            slot("tree").bottom(),
        );
    }

    /// Геометрия раскладки НЕ зависит от палитры (K4.1: компоненты
    /// выбирают только слоты; палитра — параметр отрисовки). Претенденты —
    /// компоненты, принимающие палитру в layout-сигнатуре.
    #[test]
    fn k4_layout_geometry_palette_free() {
        let p1 = neutral_palette();
        let p2 = alt_palette();
        let cell = UiRect::new(150.0, 100.0, 480.0, BANNER_H);
        let (a, _) = banner(cell, 60.0, 48.0, BannerKind::Info, KitState::Hovered, &p1);
        let (b, _) = banner(cell, 60.0, 48.0, BannerKind::Info, KitState::Hovered, &p2);
        assert_eq!(a.rect, b.rect, "banner: rect от палитры не зависит");
        assert_eq!(a.label_area, b.label_area);
        assert_eq!(a.action_button, b.action_button);

        let chat_h = SPACING_SM * 0.5 + CHAT_HEADER_H + CHAT_LINE_H + SPACING_SM;
        let cell = UiRect::new(150.0, 100.0, 480.0, chat_h);
        let (a, _) = chat_bubble(cell, 1, 0, ChatBubbleKind::Error, &p1, true);
        let (b, _) = chat_bubble(cell, 1, 0, ChatBubbleKind::Error, &p2, true);
        assert_eq!(a.rect, b.rect, "chat_bubble: rect от палитры не зависит");
        assert_eq!(a.text_area, b.text_area);

        let card_h = SPACING_SM
            + canvas_ui::component::radio_card::RADIO_LABEL_LINE_H
            + canvas_core::tokens::SPACING_S
            + canvas_ui::component::radio_card::RADIO_DESC_LINE_H
            + SPACING_SM;
        let cell = UiRect::new(150.0, 100.0, 320.0, card_h);
        let (a, _) = radio_card(cell, 90.0, Some(110.0), true, KitState::Selected, &p1);
        let (b, _) = radio_card(cell, 90.0, Some(110.0), true, KitState::Selected, &p2);
        assert_eq!(a.rect, b.rect, "radio_card: rect от палитры не зависит");
        assert_eq!(a.indicator, b.indicator);
    }

    /// K4-хвост в РЕАЛЬНОЙ раскладке: последние 20 подписей секций
    /// gallery_layout — ровно K4_ORDER (k4_title_key), без пропусков и
    /// перестановок; попутно на каждом шаге скана — весь удержанный
    /// K4-контент целиком в границах sections_viewport (hit=draw: рисуются
    /// только полностью видимые секции, выход за окно невозможен).
    #[test]
    fn gallery_layout_k4_titles_tail_in_order() {
        let mut m = new_measurer();
        let mut fs = measure_font_system();
        let p = neutral_palette();
        // FR-059: section_titles отфильтрованы по видимости — полный список
        // собирается детерминированным сканом offset (шаг 8 px, как в
        // golden-линейке kit_ui.rs).
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
        let mut seen: Vec<&'static str> = Vec::new();
        let mut off = 0.0f32;
        while off <= content_h - viewport_h {
            let s = kit::ScrollState {
                offset: off,
                content_h,
                viewport_h,
            };
            let lay = gallery_layout([1280.0, 800.0], Language::Ru, &s, &p, &mut m, &mut fs);
            for (_, key) in &lay.section_titles {
                if !seen.contains(key) {
                    seen.push(key);
                }
            }
            // Геометрия в границах sections_viewport: после shift_and_filter
            // весь удержанный K4-контент целиком внутри окна секций
            // (вертикаль — контракт видимости FR-059 с допуском 0.01,
            // горизонталь — слот control-колонки внутри окна).
            let vp = lay.sections_viewport;
            for id in K4_ORDER {
                for r in section_rects(&lay.k4, id) {
                    assert!(
                        r.y >= vp.y - 0.01
                            && r.bottom() <= vp.bottom() + 0.01
                            && r.x >= vp.x - EPS
                            && r.right() <= vp.right() + EPS,
                        "offset {off}: {id} rect {:?} вне sections_viewport {:?}",
                        r,
                        vp
                    );
                }
            }
            off += 8.0;
        }
        assert_eq!(seen.len(), 49, "29 прежних секций + 20 K4 (скан полноты)");
        let keys: Vec<&'static str> = K4_ORDER.iter().map(|id| k4_title_key(id)).collect();
        let tail: Vec<&'static str> = seen[seen.len() - 20..].to_vec();
        assert_eq!(tail, keys, "хвост подписей линейки — K4-секции по порядку");
        assert!(
            !seen[..seen.len() - 20].iter().any(|k| keys.contains(k)),
            "K4-подписи ниже всех прежних секций (аддитивный хвост)"
        );
    }
}
