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

// FR-068 W3: kit.rs — тонкий фасад. Реализация перенесена в `component/*`
// (`component/mod.rs` — `Component` trait + общие типы). Публичное API кита
// сохранено 1:1 (§Контракт-1 PRD-0009 V-5: потребители не переписываются;
// `crate::component` — канонический путь, `crate::kit` — совместимость).

// FR-105 (C2): константы баннера — фасад кита (как RADIO_* из radio_card):
// потребители баннера потери доступа (storage_ui/overlays) не тянут
// внутренний путь `component::banner`.
pub use crate::component::banner::{
    banner, paint_banner, BannerKind, BannerLayout, BannerStyle, BANNER_H, BANNER_LABEL_LINE_H,
    BANNER_PAD, BANNER_RADIUS, BANNER_TINT_ALPHA,
};
pub use crate::component::button::{
    button_layout, button_size, button_style, chip_layout, chip_size, chip_style, icon_button_rect,
    icon_button_style, icon_glyph, icon_name, stage_close_button, stage_close_button_lg, switch,
    ChipLayout, Icon, SwitchLayout,
};
pub use crate::component::chat_bubble::{
    chat_bubble, paint_chat_bubble, ChatBubbleKind, ChatBubbleLayout, ChatBubbleStyle,
    CHAT_HEADER_H, CHAT_LINE_H, CHAT_TOOL_CALL_H,
};
pub use crate::component::chip::{
    chip_strip, chip_strip_wrap, CHIP_FAMILY, CHIP_FONT_SIZE, CHIP_GAP,
};
pub use crate::component::crumbs::{crumbs, crumbs_active_index};
pub use crate::component::dropdown::{
    anchored_stack, dropdown_menu, toast_area, tooltip, tooltip_rect_anchored, viewport_clamp,
    AnchoredSide, DropdownLayout, TooltipLayout,
};
pub use crate::component::footer::{footer_buttons_measured, split_footer_buttons, FooterGroup};
pub use crate::component::icon::{icon_composition, IconKind};
pub use crate::component::list::{list_rows, scroll_bar, ScrollState};
pub use crate::component::modal::{focus_order, modal, modal_style, ModalLayout};
pub use crate::component::panel::{
    backdrop, card, control_style_of, panel_content, panel_rect, panel_style, panel_style_of,
    CardLayout,
};
pub use crate::component::panel_header::{
    paint_panel_separator, panel_header, HeaderButton, IconKind as HeaderIconKind,
    PanelHeaderLayout, PanelHeaderStyle,
};
pub use crate::component::radio_card::{
    paint_radio_card, radio_card, RadioCardLayout, RadioCardStyle, RADIO_DESC_LINE_H,
    RADIO_INDICATOR_SIZE, RADIO_LABEL_LINE_H,
};
pub use crate::component::row::{
    leader_dash_rects, paint_row, row_guides, row_layout, row_style, RowLayout, RowMarker, RowOpts,
    RowParts, RowStyle, ROW_BADGE_PAD_H, ROW_DOT, ROW_DOT_PAD, ROW_GLYPH_GAP, ROW_GLYPH_MIN_W,
    ROW_LINE_FRAC, ROW_TEXT_GAP,
};
// FR-061 D-5 (аудит выравнивания 2026-09-26, docs/plans/
// fr-061-template-node-kit-alignment.md): зебра-маска прогонов строк данных —
// единое РЕШЕНИЕ с телом ноды (перенос 1:1 из canvas-render/text.rs); цвет
// зебры — у потребителя (F-8: слот темы, не кит).
pub use crate::row_guides::zebra_run_flags;
// FR-068 W3-продолжение (M1): Table v2 — retained-компонент таблицы
// (дизайн docs/plans/fr-068-table-v2.md; фасад 1:1, §Контракт-1).
// FR-UI-TABLE-IMMEDIATE: immediate-API для таблиц (без retained-state) —
// миграция whatif_ui::table_layout (audit §6.2).
pub use crate::component::table::{
    table_layout_immediate, Table, TableOpts, TableProps, TableRow, TableRowStyle,
};
// Волна «input-адекватность» 2026-10-09 (design/rules/09-input.md):
// действия/эффекты ввода, маска пароля, клик→каретка — единый контракт полей.
pub use crate::component::text_field::{
    caret_index_at_x, text_field, text_field_masked, TextFieldAction, TextFieldEffect,
    TextFieldLayout, TextFieldModel,
};
pub use crate::component::two_column::{two_column, two_column_right, TwoColumnLayout};

// Wave C §5.3: новые компоненты (2026-10).
pub use crate::component::accordion::{
    accordion_hit, accordion_layout, accordion_style, AccordionLayout,
};
pub use crate::component::avatar::{avatar_layout, avatar_style, AvatarLayout, AvatarSource};
pub use crate::component::badge::{badge_layout, badge_style, BadgeKind, BadgeLayout, BadgeTone};
pub use crate::component::checkbox::{
    checkbox_glyph, checkbox_hit, checkbox_layout, checkbox_style, CheckboxLayout, CheckboxState,
};
pub use crate::component::command_palette::{
    command_palette_hit, command_palette_layout, search_actions, CommandAction, CommandCategory,
    CommandPaletteKey, CommandPaletteLayout,
};
pub use crate::component::popover::{popover, PopoverLayout};
pub use crate::component::progress::{
    progress_layout, progress_phase, progress_style, ProgressKind, ProgressLayout,
};
pub use crate::component::radio::{
    radio_group_key, radio_group_layout, radio_hit, radio_style, RadioGroup, RadioKey, RadioLayout,
    RadioOrientation,
};
pub use crate::component::segmented::{
    segmented_hit, segmented_key, segmented_layout, segmented_style, SegmentedKey, SegmentedLayout,
};
pub use crate::component::skeleton::{
    skeleton_alpha, skeleton_layout, SkeletonLayout, SkeletonPattern,
};
pub use crate::component::slider::{
    slider_hit, slider_key, slider_layout, slider_style, SliderHit, SliderKey, SliderLayout,
    SliderOpts,
};
pub use crate::component::snackbar::{
    snackbar_layout, snackbar_style, Snackbar, SnackbarLayout, SnackbarQueue,
};
pub use crate::component::tabs::{
    tab_style, tabs_hit, tabs_key, tabs_layout, TabKey, TabLayout, TabStyle, TabStyleResult,
    TabsLayout,
};
pub use crate::component::tree::{tree_hit, tree_layout, TreeAction, TreeNode, TreeRow};

// Wave A §5.5: архитектурные паттерны (Response, WidgetExt, NavRail, ActionRegistry, FocusTrap).
pub use crate::component::{Response, WidgetExt};
pub use crate::component::nav_rail::{
    nav_rail_activate, nav_rail_activate_by_id, nav_rail_button_style, nav_rail_hit,
    nav_rail_layout, NavRail, NavRailItem, NavRailLayout,
};
pub use crate::action::{Action, ActionRegistry};
pub use crate::keyboard::FocusTrap;

pub use crate::component::{
    resolve_state, state_layer_alpha, ButtonVariant, ControlSize, ControlStyle, Elevation,
    KitPalette, KitState, PanelStyle, Shape, Spacing, BUTTON_HEIGHT, BUTTON_PAD_H, BUTTON_WIDTH,
    CHIP_HEIGHT, CHIP_PAD_H, DROPDOWN_GAP, GAP_CONTROLS, ICON_BUTTON_SIZE, LIST_ROW_GAP,
    LIST_ROW_H, SCROLLBAR_KNOB_MIN, SCROLLBAR_WIDTH, SWITCH_H, SWITCH_KNOB_PAD, SWITCH_W,
    TEXT_FIELD_HEIGHT, TEXT_FIELD_MIN_W, TEXT_FIELD_PAD_H, TOAST_TTL_MS, TOOLTIP_DELAY_MS,
    TOOLTIP_OFFSET,
};
// Wave T §5.2.4: motion tokens (Duration, Easing, ease, effective_duration).
pub use crate::anim::{ease, effective_duration, effective_duration_with, Duration, Easing};
