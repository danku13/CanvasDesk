//! FR-068 W0 (§9, Контракт-9): snapshot-тесты `Painter.items()` — эталоны
//! геометрии kit-компонентов.
//!
//! Матрица: 30 kit-компонентов × 2 языка (RU/EN), состояния по волнам:
//! - 10 компонентов W0 (Panel/Button/IconButton/Dropdown/Chip/Toast/Tooltip/
//!   Modal/TextField/Switch) × 3 состояния (default/hover/disabled) =
//!   **60 эталонов** (W0, не расширяются);
//! - 20 компонентов Wave C+W2 (issue #67 V3): Wave C — checkbox/slider/radio/
//!   tabs/segmented/badge/progress/skeleton/popover/snackbar/avatar/tree/
//!   command_palette/accordion; W2 — banner/chat_bubble/crumbs/footer/
//!   radio_card/two_column; × 4 состояния (default/hover/disabled/error —
//!   KitState Normal/Hovered/Disabled/Error) = **160 эталонов**.
//!
//! Итого **220 эталонов** + тест счётчика = 221 тест.
//!
//! Формат дампа (нормализованная строка на Rect/Text-лист; клип — строка
//! контейнера + дети рекурсивно в общий отсортированный список):
//! - Rect: `rect x=… y=… w=… h=…`
//! - Text: `text x=… y=… w=… h=… text="…"`
//! - ClipRect (FR-068 W1): `clip x=… y=… w=… h=…` + рекурсивно дети
//!   (клип не отсекает детей в дампе — это данные; отсечение исполняет
//!   consumer/scissor FR-056)
//!
//! Координаты/размеры округляются до целого ui px (`f32::round() as i32`);
//! строки сортируются по `(x, y, w, h, type)` — дамп устойчив к порядку
//! вызовов отрисовки (сравнение по множеству items). Цвета/радиусы НЕ
//! включаются: это слоты темы (`KitPalette`/`ControlStyle`), а не геометрия —
//! фиксируются тип + геометрия + текст.
//!
//! Эталоны: `tests/snapshot/<component>_<state>_<lang>.txt`. Сравнение —
//! точное строковое; расхождение — осознанное обновление эталона в PR с diff
//! (FR-068 W0 §9). Регенерация: `CANVAS_UI_UPDATE_SNAPSHOTS=1 cargo test
//! -p canvas-ui --test snapshot` (тест пишет файл и не сравнивает).
//!
//! Детерминизм: шрифт — вшитый `NotoSansDisplay-Medium.ttf` (тот же файл, что
//! FONT_DATA рендера; паттерн `measure.rs` tests), строки-лейблы —
//! фиксированные RU/EN константы, слоты/вьюпорт — константы. Состояния, где
//! кит отвечает только слотами цвета, дают одинаковые дампы — это честно: цвет
//! исключён из дампа; там, где состояние меняет геометрию (Dropdown —
//! hover-строка), разница фиксируется. Компоненты без state-параметра в
//! API кита (badge/skeleton/avatar/chat_bubble/two_column — как Panel/
//! Toast/Tooltip в W0) рисуют одну и ту же сцену во всех состояниях —
//! их 4 эталона совпадают, это задокументированный прецедент.

use canvas_core::tokens::RADIUS_CHIP;
use canvas_ui::component::crumbs::{CRUMB_FONT_SIZE, CRUMB_GAP};
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{
    accordion_layout, accordion_style, avatar_layout, avatar_style, badge_layout, badge_style,
    banner, button_layout, button_style, chat_bubble, checkbox_glyph, checkbox_layout,
    checkbox_style, chip_layout, chip_style, command_palette_layout, control_style_of, crumbs,
    crumbs_active_index, dropdown_menu, icon_button_rect, icon_button_style, icon_glyph, modal,
    modal_style, panel_content, panel_rect, panel_style, popover, progress_layout, progress_style,
    radio_card, radio_group_layout, radio_style, search_actions, segmented_layout, segmented_style,
    skeleton_layout, slider_layout, slider_style, snackbar_layout, snackbar_style,
    split_footer_buttons, switch, tab_style, tabs_layout, text_field, toast_area, tooltip,
    tree_layout, two_column, AnchoredSide, AvatarSource, BadgeKind, BadgeTone, BannerKind,
    ButtonVariant, ChatBubbleKind, CheckboxState, CommandAction, CommandCategory, ControlSize,
    FooterGroup, Icon, KitPalette, KitState, ProgressKind, RadioGroup, RadioOrientation,
    SkeletonPattern, SliderOpts, Snackbar, TabStyle, TextFieldModel, TreeNode, BANNER_H,
    BUTTON_HEIGHT, BUTTON_PAD_H, CHAT_LINE_H, CHIP_PAD_H, GAP_CONTROLS, LIST_ROW_GAP, LIST_ROW_H,
    ROW_LINE_FRAC, TEXT_FIELD_HEIGHT, TEXT_FIELD_MIN_W, TOOLTIP_DELAY_MS,
};
use canvas_ui::layout::{HAlign, VAlign};
use canvas_ui::measure::TextMeasurer;
use canvas_ui::paint::{PaintAlign, PaintItem, Painter};

/// Семейство вшитого шрифта (паритет `measure.rs` tests и рендера).
const FAMILY: &str = "Noto Sans Display";
/// Кегль подписей кита (как у потребителей — kit_ui LABEL_SIZE).
const FONT_SIZE: f32 = 13.0;
/// Вьюпорт всех сцен (фиксированный — детерминизм координат).
const VIEWPORT: UiRect = UiRect::new(0.0, 0.0, 320.0, 240.0);
/// Пад меню dropdown (как у потребителей — 8 = SPACING_SM).
const MENU_PAD: f32 = 8.0;

/// Id компонентов матрицы (порядок = порядок файлов эталонов).
const COMP_IDS: [&str; 10] = [
    "panel",
    "button",
    "icon_button",
    "dropdown",
    "chip",
    "toast",
    "tooltip",
    "modal",
    "text_field",
    "switch",
];

/// Id состояний матрицы (default=Normal, hover=Hovered, disabled=Disabled).
const STATE_IDS: [&str; 3] = ["default", "hover", "disabled"];

/// Id языков матрицы.
const LANG_IDS: [&str; 2] = ["ru", "en"];

/// Id НОВЫХ компонентов матрицы (issue #67 V3: Wave C 14 + W2 6) —
/// отдельный массив, существующие COMP_IDS не тронуты (их 60 эталонов
/// остаются как есть).
const NEW_COMP_IDS: [&str; 20] = [
    "checkbox",
    "slider",
    "radio",
    "tabs",
    "segmented",
    "badge",
    "progress",
    "skeleton",
    "popover",
    "snackbar",
    "avatar",
    "tree",
    "command_palette",
    "accordion",
    "banner",
    "chat_bubble",
    "crumbs",
    "footer",
    "radio_card",
    "two_column",
];

/// Id состояний НОВЫХ компонентов (K4: Normal/Hovered/Disabled/Error).
const NEW_STATE_IDS: [&str; 4] = ["default", "hover", "disabled", "error"];

// --- Детерминированный шрифт (паттерн measure.rs tests) ----------------------

/// FontSystem только с вшитым шрифтом рендера — метрики одинаковы на всех
/// платформах CI.
fn font_system() -> cosmic_text::FontSystem {
    let mut fs = cosmic_text::FontSystem::new();
    const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
    fs.db_mut().load_font_data(FONT.to_vec());
    fs
}

// --- Палитра (фиксированные слоты; в дамп НЕ входят) -------------------------

/// Палитра сцены: все слоты заполнены фиксированными значениями (цвета в
/// дамп не попадают — важен только факт выбора слота внутри кита).
fn palette() -> KitPalette {
    KitPalette {
        panel_fill: [0.12, 0.13, 0.16, 1.0],
        panel_border: [0.24, 0.26, 0.30, 1.0],
        control_fill: [0.18, 0.19, 0.23, 1.0],
        control_border: [0.30, 0.32, 0.38, 1.0],
        control_primary: [0.20, 0.45, 0.90, 1.0],
        control_danger: [0.85, 0.25, 0.25, 1.0],
        hover_fill: [0.28, 0.30, 0.36, 1.0],
        primary_hover_fill: [0.28, 0.55, 0.95, 1.0],
        selected_fill: [0.22, 0.38, 0.62, 1.0],
        text: [0.88, 0.89, 0.92, 1.0],
        text_title: [0.96, 0.96, 0.98, 1.0],
        text_muted: [0.62, 0.64, 0.70, 1.0],
        disabled_text: [0.45, 0.46, 0.50, 1.0],
        accent: [0.35, 0.65, 1.0, 1.0],
        // FR-070: новые слоты — фиксированные значения (в дамп не попадают —
        // важен только факт выбора слота внутри кита).
        control_success: [0.30, 0.75, 0.55, 1.0],
        control_warning: [0.95, 0.65, 0.30, 1.0],
        stage_dim: [0.02, 0.02, 0.04, 0.85],
        scrollbar_thumb: [0.35, 0.38, 0.46, 0.7],
        rule_color: [0.30, 0.33, 0.40, 0.8],
    }
}

// --- Языки (фиксированные детерминированные строки-лейблы) -------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

/// Строки-лейблы сцен: по одному лейблу на компонент на язык (+ «Отмена»
/// для второй кнопки модали — та же таблица, те же константы). Блок V3 —
/// 20 лейблов новых компонентов Wave C+W2 (issue #67).
struct I18n {
    panel: &'static str,
    apply: &'static str,
    cancel: &'static str,
    settings: &'static str,
    search: &'static str,
    hint: &'static str,
    saved: &'static str,
    dark_theme: &'static str,
    // --- V3 (Wave C) ---
    checkbox: &'static str,
    slider: &'static str,
    radio: &'static str,
    tabs: &'static str,
    segmented: &'static str,
    badge: &'static str,
    progress: &'static str,
    skeleton: &'static str,
    popover: &'static str,
    snackbar: &'static str,
    avatar: &'static str,
    tree: &'static str,
    command_palette: &'static str,
    accordion: &'static str,
    // --- V3 (W2) ---
    banner: &'static str,
    chat_bubble: &'static str,
    crumbs: &'static str,
    footer: &'static str,
    radio_card: &'static str,
    two_column: &'static str,
}

const RU: I18n = I18n {
    panel: "Панель",
    apply: "Применить",
    cancel: "Отмена",
    settings: "Настройки",
    search: "Поиск",
    hint: "Подсказка",
    saved: "Сохранено",
    dark_theme: "Тёмная тема",
    checkbox: "Флажок",
    slider: "Ползунок",
    radio: "Радио",
    tabs: "Вкладки",
    segmented: "Сегменты",
    badge: "Бейдж",
    progress: "Прогресс",
    skeleton: "Скелетон",
    popover: "Поповер",
    snackbar: "Снекбар",
    avatar: "Аватар",
    tree: "Дерево",
    command_palette: "Палитра",
    accordion: "Аккордеон",
    banner: "Баннер",
    chat_bubble: "Сообщение",
    crumbs: "Крошки",
    footer: "Подвал",
    radio_card: "Карточка",
    two_column: "Колонки",
};

const EN: I18n = I18n {
    panel: "Panel",
    apply: "Apply",
    cancel: "Cancel",
    settings: "Settings",
    search: "Search",
    hint: "Hint",
    saved: "Saved",
    dark_theme: "Dark theme",
    checkbox: "Checkbox",
    slider: "Slider",
    radio: "Radio",
    tabs: "Tabs",
    segmented: "Segments",
    badge: "Badge",
    progress: "Progress",
    skeleton: "Skeleton",
    popover: "Popover",
    snackbar: "Snackbar",
    avatar: "Avatar",
    tree: "Tree",
    command_palette: "Palette",
    accordion: "Accordion",
    banner: "Banner",
    chat_bubble: "Message",
    crumbs: "Crumbs",
    footer: "Footer",
    radio_card: "Card",
    two_column: "Columns",
};

fn tr(lang: Lang) -> &'static I18n {
    match lang {
        Lang::Ru => &RU,
        Lang::En => &EN,
    }
}

// --- Компоненты матрицы ------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Comp {
    Panel,
    Button,
    IconButton,
    Dropdown,
    Chip,
    Toast,
    Tooltip,
    Modal,
    TextField,
    Switch,
    // --- V3: Wave C (issue #67) ---
    Checkbox,
    Slider,
    Radio,
    Tabs,
    Segmented,
    Badge,
    Progress,
    Skeleton,
    Popover,
    Snackbar,
    Avatar,
    Tree,
    CommandPalette,
    Accordion,
    // --- V3: W2 (issue #67) ---
    Banner,
    ChatBubble,
    Crumbs,
    Footer,
    RadioCard,
    TwoColumn,
}

impl Comp {
    fn id(self) -> &'static str {
        match self {
            Comp::Panel => "panel",
            Comp::Button => "button",
            Comp::IconButton => "icon_button",
            Comp::Dropdown => "dropdown",
            Comp::Chip => "chip",
            Comp::Toast => "toast",
            Comp::Tooltip => "tooltip",
            Comp::Modal => "modal",
            Comp::TextField => "text_field",
            Comp::Switch => "switch",
            Comp::Checkbox => "checkbox",
            Comp::Slider => "slider",
            Comp::Radio => "radio",
            Comp::Tabs => "tabs",
            Comp::Segmented => "segmented",
            Comp::Badge => "badge",
            Comp::Progress => "progress",
            Comp::Skeleton => "skeleton",
            Comp::Popover => "popover",
            Comp::Snackbar => "snackbar",
            Comp::Avatar => "avatar",
            Comp::Tree => "tree",
            Comp::CommandPalette => "command_palette",
            Comp::Accordion => "accordion",
            Comp::Banner => "banner",
            Comp::ChatBubble => "chat_bubble",
            Comp::Crumbs => "crumbs",
            Comp::Footer => "footer",
            Comp::RadioCard => "radio_card",
            Comp::TwoColumn => "two_column",
        }
    }
}

// --- Дамп Painter.items (нормализация FR-068 W0 §9) --------------------------

/// Округление до целого ui px.
fn px(v: f32) -> i32 {
    v.round() as i32
}

/// Нормализованный дамп items: строка на item, сортировка по
/// `(x, y, w, h, type)`; цвета/радиусы (слоты темы) не включаются.
/// ClipRect (FR-068 W1) — строка `clip` самого rect'а + рекурсивно дети
/// (в общий список; клип в дампе не отсекает детей — отсечение исполняет
/// consumer/scissor FR-056).
fn dump_items(items: &[PaintItem]) -> String {
    struct Line {
        key: (i32, i32, i32, i32, &'static str),
        body: String,
    }
    fn push_item_lines(item: &PaintItem, out: &mut Vec<Line>) {
        match item {
            PaintItem::Rect { rect, .. } => {
                let (x, y, w, h) = (px(rect.x), px(rect.y), px(rect.w), px(rect.h));
                out.push(Line {
                    key: (x, y, w, h, "rect"),
                    body: format!("rect x={x} y={y} w={w} h={h}"),
                });
            }
            PaintItem::Text { area, text, .. } => {
                let (x, y, w, h) = (px(area.x), px(area.y), px(area.w), px(area.h));
                out.push(Line {
                    key: (x, y, w, h, "text"),
                    body: format!("text x={x} y={y} w={w} h={h} text=\"{text}\""),
                });
            }
            PaintItem::Icon { rect, name, .. } => {
                // FR-ICONS: иконка в дампе — rect + имя (tint — слот темы).
                // Попутный фикс 2026-09-25 (FR-074): вариант добавлен в
                // PaintItem ранее, match дампа не обновили — тест не
                // компилировался на main (предсуществующий красный).
                let (x, y, w, h) = (px(rect.x), px(rect.y), px(rect.w), px(rect.h));
                out.push(Line {
                    key: (x, y, w, h, "icon"),
                    body: format!("icon x={x} y={y} w={w} h={h} name=\"{name}\""),
                });
            }
            PaintItem::ClipRect { rect, items } => {
                let (x, y, w, h) = (px(rect.x), px(rect.y), px(rect.w), px(rect.h));
                out.push(Line {
                    key: (x, y, w, h, "clip"),
                    body: format!("clip x={x} y={y} w={w} h={h}"),
                });
                for child in items {
                    push_item_lines(child, out);
                }
            }
            PaintItem::Transform { deg, origin, items } => {
                // FR-074: transform — строка маркера + рекурсивно дети
                // (не клип — геометрию детей не трогает).
                let (x, y) = (px(origin.x), px(origin.y));
                out.push(Line {
                    key: (x, y, 0, 0, "rotate"),
                    body: format!("rotate deg={deg} ox={x} oy={y}"),
                });
                for child in items {
                    push_item_lines(child, out);
                }
            }
            PaintItem::ZGroup { z, items } => {
                // FR-074: z-группа — маркер + рекурсивно дети (порядок
                // внутри — уже z-отсортированный take_items'ом).
                out.push(Line {
                    key: (0, 0, 0, 0, "zgroup"),
                    body: format!("zgroup z={z}"),
                });
                for child in items {
                    push_item_lines(child, out);
                }
            }
        }
    }
    let mut lines: Vec<Line> = Vec::new();
    for item in items {
        push_item_lines(item, &mut lines);
    }
    lines.sort_by_key(|l| l.key);
    let mut out = String::new();
    for line in &lines {
        out.push_str(&line.body);
        out.push('\n');
    }
    out
}

// --- Сцены компонентов (layout через kit-функции → отрисовка через Painter) --

/// Panel: `panel_rect` + `panel_style` + подпись в `panel_content`.
fn scene_panel(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let st = panel_style(pal);
    let rect = panel_rect(
        slot,
        UiVec2::new(120.0, 64.0),
        UiVec2::new(240.0, 120.0),
        UiVec2::new(240.0, 120.0),
    );
    p.panel(rect, &st);
    let content = panel_content(rect, &st);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    let area = UiRect::new(
        content.x,
        content.y + (content.h - line_h) / 2.0,
        content.w,
        line_h,
    );
    p.label(area, t.panel, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Button: `button_layout` (замер по подписи) + `button_style` (variant
/// Primary) + подпись по центру rect'а.
fn scene_button(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = button_layout(
        slot,
        t.apply,
        (HAlign::Center, VAlign::Center),
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = button_style(ButtonVariant::Primary, state, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, &lay.label, st.text, FONT_SIZE, PaintAlign::Center);
}

/// IconButton: `icon_button_rect` + `icon_button_style` + глиф `Icon::Gear`.
fn scene_icon_button(p: &mut Painter, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let rect = icon_button_rect(slot, (HAlign::Center, VAlign::Center));
    let st = icon_button_style(state, pal);
    p.control(rect, &st);
    p.label(
        rect,
        icon_glyph(Icon::Gear),
        st.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
}

/// Dropdown: якорь-кнопка + `dropdown_menu` (меню под якорем) + 2 строки
/// опций; hover — подсветка первой строки (геометрия меняется), disabled —
/// текст слотом disabled (цвет в дамп не входит).
fn scene_dropdown(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let anchor = UiRect::new(40.0, 40.0, 120.0, BUTTON_HEIGHT);
    let menu_h = 2.0 * LIST_ROW_H + LIST_ROW_GAP + 2.0 * MENU_PAD;
    let dd = dropdown_menu(anchor, VIEWPORT, UiVec2::new(160.0, menu_h));
    p.panel(dd.menu, &panel_style(pal));
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        pal.text
    };
    let mut y = dd.menu.y + MENU_PAD;
    for (i, opt) in [t.settings, t.search].iter().enumerate() {
        let row = UiRect::new(
            dd.menu.x + MENU_PAD,
            y,
            dd.menu.w - 2.0 * MENU_PAD,
            LIST_ROW_H,
        );
        if state == KitState::Hovered && i == 0 {
            p.rect(row, pal.hover_fill, [0.0; 4], RADIUS_CHIP);
        }
        p.label(row, opt, text, FONT_SIZE, PaintAlign::Left);
        y += LIST_ROW_H + LIST_ROW_GAP;
    }
}

/// Chip: `chip_layout` от левого края слота + `chip_style` + подпись.
fn scene_chip(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = chip_layout(
        UiPoint::new(slot.x, slot.y),
        t.search,
        160.0,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = chip_style(state, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, &lay.label, st.text, FONT_SIZE, PaintAlign::Center);
}

/// Toast: `toast_area` (строка внизу по центру) + панельный стиль + сообщение.
fn scene_toast(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let area = toast_area(VIEWPORT, None);
    p.panel(area, &panel_style(pal));
    p.label(area, t.saved, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Tooltip: ширина по замеру + `tooltip` (delay пройден; якорь у нижнего края
/// — flip вверх) + панельный стиль + текст.
fn scene_tooltip(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let tip_w = (m.width_of(fs, t.hint, FAMILY, FONT_SIZE) + 16.0).min(240.0);
    let size = UiVec2::new(tip_w, 18.0);
    let lay = tooltip(
        UiPoint::new(210.0, 210.0),
        size,
        VIEWPORT,
        TOOLTIP_DELAY_MS,
        TOOLTIP_DELAY_MS,
    )
    .expect("delay пройден — тултип показан");
    p.panel(lay.rect, &panel_style(pal));
    p.label(lay.rect, t.hint, pal.text, FONT_SIZE, PaintAlign::Center);
}

/// Modal: `modal` (затемнение + панель по центру) + заголовок + ряд кнопок
/// (Primary «Применить» / Secondary «Отмена») через `button_layout`.
fn scene_modal(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let mo = modal(
        VIEWPORT,
        UiVec2::new(240.0, 120.0),
        UiVec2::new(280.0, 140.0),
        UiVec2::new(280.0, 140.0),
    );
    p.rect(mo.dim, [0.0, 0.0, 0.0, 0.5], [0.0; 4], 0.0);
    let st = modal_style(pal);
    p.panel(mo.panel, &st);
    let content = panel_content(mo.panel, &st);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(content.x, content.y, content.w, line_h),
        t.settings,
        pal.text_title,
        FONT_SIZE,
        PaintAlign::Center,
    );
    let btn_w = 120.0;
    let row_y = content.bottom() - BUTTON_HEIGHT;
    let buttons = [
        (
            UiRect::new(content.x, row_y, btn_w, BUTTON_HEIGHT),
            t.apply,
            ButtonVariant::Primary,
        ),
        (
            UiRect::new(content.right() - btn_w, row_y, btn_w, BUTTON_HEIGHT),
            t.cancel,
            ButtonVariant::Secondary,
        ),
    ];
    for (slot, label, variant) in buttons {
        let bl = button_layout(
            slot,
            label,
            (HAlign::Center, VAlign::Center),
            m,
            fs,
            FAMILY,
            FONT_SIZE,
        );
        let bs = button_style(variant, state, pal);
        p.control(bl.rect, &bs);
        p.label(bl.rect, &bl.label, bs.text, FONT_SIZE, PaintAlign::Center);
    }
}

/// TextField: модель с текстом + `text_field` (каретка скрыта — поле не в
/// фокусе) + rect контролом + текст в `text_area`. Состояние —
/// зарезервированный слот кита (`_state` в kit::text_field): геометрия от
/// него не зависит.
fn scene_text_field(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(40.0, 96.0, 240.0, TEXT_FIELD_HEIGHT);
    let model = TextFieldModel {
        text: t.search.to_owned(),
        caret: t.search.chars().count(),
        sel: None,
        max_chars: None,
    };
    let lay = text_field(
        slot,
        UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
        UiVec2::new(200.0, TEXT_FIELD_HEIGHT),
        &model,
        "",
        false,
        state,
        pal,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = control_style_of(pal.control_fill, pal.control_border, pal.text, RADIUS_CHIP);
    p.rect(lay.rect, st.fill, st.border, st.radius);
    p.label(
        lay.text_area,
        &lay.text_shown,
        st.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
}

/// Switch: подпись строки + `switch` (on) — трек контролом + бегунок rect'ом.
fn scene_switch(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    let sw_slot = UiRect::new(slot.right() - 64.0, slot.y, 64.0, slot.h);
    let label_area = UiRect::new(
        slot.x,
        slot.y + (slot.h - line_h) / 2.0,
        sw_slot.x - GAP_CONTROLS - slot.x,
        line_h,
    );
    let lay = switch(sw_slot, true, state, pal);
    p.label(
        label_area,
        t.dark_theme,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    p.control(lay.track, &lay.track_style);
    p.rect(lay.knob, lay.knob_fill, [0.0; 4], lay.knob.w / 2.0);
}

// --- V3: сцены Wave C (issue #67) -------------------------------------------

/// Checkbox: `checkbox_layout` (Checked) + `checkbox_style` + глиф ✓ в
/// квадрате + подпись справа. Состояние — слоты цвета (геометрия одна).
fn scene_checkbox(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = checkbox_layout(
        slot,
        t.checkbox,
        CheckboxState::Checked,
        ControlSize::Sm,
        GAP_CONTROLS,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let st = checkbox_style(CheckboxState::Checked, state, pal);
    p.control(lay.box_rect, &st);
    p.label(
        lay.box_rect,
        checkbox_glyph(CheckboxState::Checked),
        st.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
    p.label(
        lay.label_rect,
        &lay.label,
        st.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
}

/// Slider: `slider_layout` (value=60 из 0..100) + `slider_style` — трек
/// контролом, filled rect'ом, knob кругом. Подпись над треком.
fn scene_slider(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y, slot.w, line_h),
        t.slider,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let slide_slot = UiRect::new(slot.x, slot.y + line_h + GAP_CONTROLS, slot.w, 32.0);
    let opts = SliderOpts {
        min: 0.0,
        max: 100.0,
        ..Default::default()
    };
    let lay = slider_layout(slide_slot, 60.0, &opts, ControlSize::Sm);
    let (track_st, knob_fill) = slider_style(state, pal);
    p.control(lay.track, &track_st);
    p.rect(lay.filled, pal.control_primary, [0.0; 4], RADIUS_CHIP);
    p.rect(lay.knob, knob_fill, [0.0; 4], lay.knob.w / 2.0);
}

/// Radio: `radio_group_layout` (Vertical, 2 опции, первая выбрана) +
/// `radio_style` + внутренняя точка выбранного + подписи. Подпись над группой.
fn scene_radio(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y + 7.0, slot.w, line_h),
        t.radio,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let group_slot = UiRect::new(16.0, 48.0, 160.0, 120.0);
    let group = RadioGroup::new(vec![t.settings.to_owned(), t.search.to_owned()]);
    let lays = radio_group_layout(
        group_slot,
        &group,
        RadioOrientation::Vertical,
        ControlSize::Sm,
        GAP_CONTROLS,
        GAP_CONTROLS,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    for lay in lays {
        let st = radio_style(lay.selected, state, pal);
        p.control(lay.circle, &st);
        if lay.selected {
            // Внутренняя точка выбранного radio (паритет потребителей кита).
            let d = lay.circle.w / 2.0;
            let dot = UiRect::new(lay.circle.x + d / 2.0, lay.circle.y + d / 2.0, d, d);
            p.rect(dot, pal.text_title, [0.0; 4], d / 2.0);
        }
        p.label(
            lay.label_rect,
            &lay.label,
            st.text,
            FONT_SIZE,
            PaintAlign::Left,
        );
    }
}

/// Tabs: `tabs_layout` (Underline, 2 таба, активен первый) + `tab_style` —
/// контент-область панелью, активный — индикатором, подписи по центру.
fn scene_tabs(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y, slot.w, line_h),
        t.tabs,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let tabs_slot = UiRect::new(
        slot.x,
        slot.y + line_h + GAP_CONTROLS,
        slot.w,
        slot.h - line_h - GAP_CONTROLS,
    );
    let labels = vec![t.settings.to_owned(), t.search.to_owned()];
    let lay = tabs_layout(
        tabs_slot,
        &labels,
        0,
        TabStyle::Underline,
        36.0,
        GAP_CONTROLS,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    p.panel(lay.content, &panel_style(pal));
    for tab in &lay.tabs {
        let st = tab_style(tab.active, state, TabStyle::Underline, pal);
        p.rect(tab.rect, st.fill, [0.0; 4], st.radius);
        if tab.active {
            // Индикатор рисует потребитель только у активного таба
            // (indicator_fill неактивного — прозрачный слот).
            p.rect(tab.indicator, st.indicator_fill, [0.0; 4], 0.0);
        }
        p.label(tab.rect, &tab.label, st.text, FONT_SIZE, PaintAlign::Center);
    }
}

/// Segmented: `segmented_layout` (2 сегмента, активен первый) +
/// `segmented_style` — контейнер контролом, затем сегменты (active_rect
/// совпадает с segments[active] — отдельным rect'ом не дублируется).
fn scene_segmented(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(16.0, 100.0, 240.0, 28.0);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y - line_h - GAP_CONTROLS, slot.w, line_h),
        t.segmented,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let labels = vec![t.settings.to_owned(), t.search.to_owned()];
    let lay = segmented_layout(slot, &labels, 0, m, fs, FAMILY, FONT_SIZE);
    let (container_st, _) = segmented_style(false, state, pal);
    p.control(lay.container, &container_st);
    for (i, seg) in lay.segments.iter().enumerate() {
        let (_, seg_st) = segmented_style(i == lay.active, state, pal);
        p.control(*seg, &seg_st);
        p.label(*seg, &labels[i], seg_st.text, FONT_SIZE, PaintAlign::Center);
    }
}

/// Badge: `badge_layout` (Text-бейдж у правого верхнего угла якоря-иконки) +
/// `badge_style` (Primary). State-параметра в API нет — сцена одинаковая во
/// всех состояниях (прецедент Panel/Toast/Tooltip, см. шапку файла).
fn scene_badge(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let anchor = UiRect::new(16.0, 72.0, 160.0, BUTTON_HEIGHT);
    let lay = badge_layout(
        anchor,
        BadgeKind::Text(t.badge.to_owned()),
        BadgeTone::Primary,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let anchor_st = control_style_of(pal.control_fill, pal.control_border, pal.text, RADIUS_CHIP);
    p.control(anchor, &anchor_st);
    p.label(
        anchor,
        icon_glyph(Icon::Gear),
        pal.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
    let st = badge_style(BadgeTone::Primary, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, t.badge, st.text, FONT_SIZE, PaintAlign::Center);
}

/// Progress: `progress_layout` (Linear determinate 0.6) + `progress_style` —
/// трек контролом, fill rect'ом. Подпись над треком.
fn scene_progress(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y, slot.w, line_h),
        t.progress,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let bar_slot = UiRect::new(slot.x, slot.y + line_h + GAP_CONTROLS, 200.0, 16.0);
    let lay = progress_layout(
        bar_slot,
        ProgressKind::Linear { value: 0.6 },
        ControlSize::Sm,
    );
    let (track_st, fill_color) = progress_style(state, pal);
    p.control(lay.track, &track_st);
    p.rect(lay.fill, fill_color, [0.0; 4], RADIUS_CHIP);
}

/// Skeleton: `skeleton_layout` (TextLines, 3 строки, последняя 70%) — блоки
/// rect'ами. State-параметра в API нет — сцена одинаковая во всех состояниях.
fn scene_skeleton(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y, slot.w, line_h),
        t.skeleton,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let body = UiRect::new(slot.x, slot.y + line_h + GAP_CONTROLS, slot.w, 120.0);
    let lay = skeleton_layout(
        body,
        &SkeletonPattern::TextLines {
            count: 3,
            row_h: 14.0,
        },
        GAP_CONTROLS,
    );
    for r in lay.rects {
        p.rect(r, pal.control_fill, [0.0; 4], RADIUS_CHIP);
    }
}

/// Popover: `popover` (под якорем, со стрелкой) + панельный стиль + текст.
/// State — только слот текста (как scene_dropdown): геометрия одна.
fn scene_popover(p: &mut Painter, t: &I18n, pal: &KitPalette, state: KitState) {
    let anchor = UiRect::new(100.0, 40.0, 120.0, BUTTON_HEIGHT);
    let lay = popover(
        anchor,
        VIEWPORT,
        UiVec2::new(160.0, 48.0),
        AnchoredSide::Bottom,
        true,
    );
    p.panel(lay.content, &panel_style(pal));
    p.rect(lay.arrow, pal.panel_fill, [0.0; 4], 0.0);
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        pal.text
    };
    p.label(lay.content, t.popover, text, FONT_SIZE, PaintAlign::Center);
}

/// Snackbar: `snackbar_layout` (сообщение + action-кнопка) +
/// `snackbar_style` (Default tone; elevation Md — shadow детерминирован).
/// State — только слот текста (как scene_dropdown).
fn scene_snackbar(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let snack = Snackbar {
        message: t.snackbar.to_owned(),
        action_label: Some(t.apply.to_owned()),
        ttl_ms: 3000,
        tone: BadgeTone::Default,
    };
    let lay = snackbar_layout(VIEWPORT, &snack, 36.0, 12.0, m, fs, FAMILY, FONT_SIZE);
    let st = snackbar_style(BadgeTone::Default, pal);
    p.control(lay.rect, &st);
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        st.text
    };
    p.label(
        lay.message_rect,
        &lay.label,
        text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    if let (Some(action_rect), Some(action_label)) = (lay.action_rect, lay.action_label.as_ref()) {
        p.label(
            action_rect,
            action_label,
            text,
            FONT_SIZE,
            PaintAlign::Center,
        );
    }
}

/// Avatar: `avatar_layout` (Initials из первых 2 символов лейбла —
/// детерминированно) + `avatar_style` (Primary) + подпись над аватаром.
/// State-параметра в API нет — сцена одинаковая во всех состояниях.
fn scene_avatar(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y, slot.w, line_h),
        t.avatar,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let initials: String = t.avatar.chars().take(2).collect();
    let av_slot = UiRect::new(slot.x, slot.y + line_h + GAP_CONTROLS, 96.0, 48.0);
    let lay = avatar_layout(
        av_slot,
        AvatarSource::Initials(initials.clone()),
        ControlSize::Md,
    );
    let st = avatar_style(BadgeTone::Primary, pal);
    p.control(lay.rect, &st);
    p.label(lay.rect, &initials, st.text, FONT_SIZE, PaintAlign::Center);
}

/// Tree: `tree_layout` (корень раскрыт + 2 потомка) — шевроны раскрытых
/// узлов + подписи строк. State — только слот текста (как scene_dropdown).
fn scene_tree(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let root = TreeNode::new("root", t.tree)
        .expanded(true)
        .child(TreeNode::new("child", t.settings))
        .child(TreeNode::new("leaf", t.search));
    let rows = tree_layout(slot, &root, LIST_ROW_H, 16.0, m, fs, FAMILY, FONT_SIZE);
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        pal.text
    };
    for row in rows {
        if row.has_children {
            // Шеврон — глиф по состоянию узла (паритет потребителей дерева).
            let glyph = if row.expanded { "▼" } else { "▶" };
            p.label(
                row.chevron,
                glyph,
                pal.text_muted,
                FONT_SIZE,
                PaintAlign::Center,
            );
        }
        p.label(
            row.label,
            &row.label_text,
            text,
            FONT_SIZE,
            PaintAlign::Left,
        );
    }
}

/// CommandPalette: `search_actions` (пустой запрос — все) +
/// `command_palette_layout` (input сверху по центру + строки результатов,
/// первая выбрана — selected_fill). State — только слот текста.
fn scene_command_palette(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let actions = vec![
        CommandAction {
            id: "apply".to_owned(),
            label: t.apply.to_owned(),
            kbd: None,
            category: CommandCategory::File,
        },
        CommandAction {
            id: "cancel".to_owned(),
            label: t.cancel.to_owned(),
            kbd: None,
            category: CommandCategory::Edit,
        },
    ];
    let results = search_actions(&actions, "");
    let lay = command_palette_layout(
        VIEWPORT,
        &results,
        0,
        TEXT_FIELD_HEIGHT,
        LIST_ROW_H,
        8,
        m,
        fs,
    );
    let field_st = control_style_of(pal.control_fill, pal.control_border, pal.text, RADIUS_CHIP);
    p.rect(lay.input, field_st.fill, field_st.border, field_st.radius);
    p.label(
        lay.input,
        t.command_palette,
        field_st.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let text = if state == KitState::Disabled {
        pal.disabled_text
    } else {
        pal.text
    };
    for (i, (rect, action_idx)) in lay.results.iter().enumerate() {
        if i == lay.selected {
            p.rect(*rect, pal.selected_fill, [0.0; 4], RADIUS_CHIP);
        }
        p.label(
            *rect,
            &actions[*action_idx].label,
            text,
            FONT_SIZE,
            PaintAlign::Left,
        );
    }
}

/// Accordion: `accordion_layout` (раскрыт) + `accordion_style` — header
/// контролом, шеврон глифом, подпись, контент панелью.
fn scene_accordion(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let lay = accordion_layout(
        slot,
        t.accordion,
        true,
        32.0,
        64.0,
        24.0,
        m,
        fs,
        FAMILY,
        FONT_SIZE,
    );
    let (st, chevron_color) = accordion_style(true, state, pal);
    p.control(lay.header, &st);
    p.label(
        lay.chevron,
        "▼",
        chevron_color,
        FONT_SIZE,
        PaintAlign::Center,
    );
    let label_area = UiRect::new(
        lay.header.x + 24.0,
        lay.header.y,
        lay.header.w - 24.0,
        lay.header.h,
    );
    p.label(
        label_area,
        t.accordion,
        st.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    p.panel(lay.content, &panel_style(pal));
    p.label(lay.content, t.hint, pal.text, FONT_SIZE, PaintAlign::Left);
}

// --- V3: сцены W2 (issue #67) ------------------------------------------------

/// Banner: `banner` (Warning, подпись + action-кнопка) — фон rect'ом
/// (паритет paint_banner), подпись, кнопка `button_style`/Secondary.
/// State — слот текста кнопки (Disabled → disabled_text, как в kit::banner).
fn scene_banner(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(16.0, 40.0, 288.0, BANNER_H);
    let label_w = m.width_of(fs, t.banner, FAMILY, FONT_SIZE);
    let action_w = m.width_of(fs, t.apply, FAMILY, FONT_SIZE);
    let (lay, style) = banner(slot, label_w, action_w, BannerKind::Warning, state, pal);
    p.rect(lay.rect, style.fill, style.border, style.radius);
    p.label(
        lay.label_area,
        t.banner,
        style.label_color,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let bs = button_style(ButtonVariant::Secondary, state, pal);
    p.control(lay.action_button, &bs);
    p.label(
        lay.action_button,
        t.apply,
        style.action_color,
        FONT_SIZE,
        PaintAlign::Center,
    );
}

/// ChatBubble: `chat_bubble` (Normal, header + 2 строки + 1 tool_call) — фон
/// rect'ом (паритет paint_chat_bubble), строки подписями. State-параметра в
/// API нет — сцена одинаковая во всех состояниях.
fn scene_chat_bubble(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = UiRect::new(16.0, 32.0, 240.0, 96.0);
    let (lay, style) = chat_bubble(slot, 2, 1, ChatBubbleKind::Normal, pal, true);
    p.rect(lay.rect, style.fill, style.border, style.radius);
    if let Some(header) = lay.header_area {
        p.label(
            header,
            t.chat_bubble,
            pal.text_muted,
            FONT_SIZE,
            PaintAlign::Left,
        );
    }
    let line1 = UiRect::new(
        lay.text_area.x,
        lay.text_area.y,
        lay.text_area.w,
        CHAT_LINE_H,
    );
    let line2 = UiRect::new(
        lay.text_area.x,
        lay.text_area.y + CHAT_LINE_H,
        lay.text_area.w,
        CHAT_LINE_H,
    );
    p.label(line1, t.hint, style.text_color, FONT_SIZE, PaintAlign::Left);
    p.label(
        line2,
        t.saved,
        style.text_color,
        FONT_SIZE,
        PaintAlign::Left,
    );
    for row in &lay.tool_call_rows {
        p.label(*row, t.search, pal.text_muted, FONT_SIZE, PaintAlign::Left);
    }
}

/// Crumbs: `crumbs` (3 уровня, все помещаются) — чипы `chip_style`
/// (последний — Selected, остальные — state сцены), кегль CRUMB_FONT_SIZE.
/// Подпись над рядом.
fn scene_crumbs(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, 72.0, slot.w, line_h),
        t.crumbs,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    let row_slot = UiRect::new(16.0, 96.0, 288.0, 24.0);
    let labels = vec![
        t.panel.to_owned(),
        t.settings.to_owned(),
        t.search.to_owned(),
    ];
    let out = crumbs(row_slot, &labels, CRUMB_GAP, CHIP_PAD_H, m, fs);
    let active = crumbs_active_index(labels.len(), out.len());
    for (i, (rect, label)) in out.iter().enumerate() {
        let ks = if Some(i) == active {
            KitState::Selected
        } else {
            state
        };
        let st = chip_style(ks, pal);
        p.control(*rect, &st);
        p.label(*rect, label, st.text, CRUMB_FONT_SIZE, PaintAlign::Center);
    }
}

/// Footer: `split_footer_buttons` (1 кнопка слева + 2 справа) на панели-полосе,
/// кнопки через `button_layout`/`button_style` (Secondary/Secondary/Primary).
/// State — слоты кнопок (цвета; геометрия одна). Подпись над полосой.
fn scene_footer(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(16.0, 176.0, 288.0, 44.0);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    p.label(
        UiRect::new(slot.x, slot.y - line_h - GAP_CONTROLS, slot.w, line_h),
        t.footer,
        pal.text,
        FONT_SIZE,
        PaintAlign::Left,
    );
    p.panel(slot, &panel_style(pal));
    let w_hint = m.width_of(fs, t.hint, FAMILY, FONT_SIZE) + 2.0 * BUTTON_PAD_H;
    let w_cancel = m.width_of(fs, t.cancel, FAMILY, FONT_SIZE) + 2.0 * BUTTON_PAD_H;
    let w_apply = m.width_of(fs, t.apply, FAMILY, FONT_SIZE) + 2.0 * BUTTON_PAD_H;
    // Правая группа раскладывается справа-налево: первый элемент — у правого
    // края (Apply/Primary flush right, Cancel левее — канон футера).
    let buttons = split_footer_buttons(slot, &[w_hint], &[w_apply, w_cancel], GAP_CONTROLS, 0.0);
    for (rect, group, idx) in buttons {
        let (label, variant) = match (group, idx) {
            (FooterGroup::Left, _) => (t.hint, ButtonVariant::Secondary),
            (FooterGroup::Right, 0) => (t.apply, ButtonVariant::Primary),
            (FooterGroup::Right, _) => (t.cancel, ButtonVariant::Secondary),
        };
        let bl = button_layout(
            rect,
            label,
            (HAlign::Center, VAlign::Center),
            m,
            fs,
            FAMILY,
            FONT_SIZE,
        );
        let bs = button_style(variant, state, pal);
        p.control(bl.rect, &bs);
        p.label(bl.rect, &bl.label, bs.text, FONT_SIZE, PaintAlign::Center);
    }
}

/// RadioCard: `radio_card` (selected) — фон + индикатор (паритет
/// paint_radio_card), label + desc подписями. State — слоты цвета.
fn scene_radio_card(
    p: &mut Painter,
    t: &I18n,
    pal: &KitPalette,
    state: KitState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    let slot = UiRect::new(16.0, 64.0, 240.0, 52.0);
    let label_w = m.width_of(fs, t.radio_card, FAMILY, FONT_SIZE);
    let desc_w = m.width_of(fs, t.hint, FAMILY, FONT_SIZE);
    let (lay, style) = radio_card(slot, label_w, Some(desc_w), true, state, pal);
    p.rect(lay.rect, style.card_fill, style.card_border, style.radius);
    p.rect(
        lay.indicator,
        style.indicator_fill,
        [0.0; 4],
        lay.indicator.w / 2.0,
    );
    p.label(
        lay.label,
        t.radio_card,
        style.label_color,
        FONT_SIZE,
        PaintAlign::Left,
    );
    if let Some(desc) = lay.desc {
        p.label(desc, t.hint, style.desc_color, FONT_SIZE, PaintAlign::Left);
    }
}

/// TwoColumn: `two_column` ([sidebar | content]) — обе колонки панелями с
/// центрированными подписями. State-параметра в API нет — сцена одинаковая
/// во всех состояниях (прецедент Panel).
fn scene_two_column(p: &mut Painter, t: &I18n, pal: &KitPalette) {
    let slot = VIEWPORT.inset(&EdgeInsets::uniform(16.0));
    let cols = two_column(slot, 96.0, GAP_CONTROLS);
    let st = panel_style(pal);
    p.panel(cols.sidebar, &st);
    p.panel(cols.content, &st);
    let line_h = FONT_SIZE * ROW_LINE_FRAC;
    let side_label = UiRect::new(
        cols.sidebar.x,
        cols.sidebar.y + (cols.sidebar.h - line_h) / 2.0,
        cols.sidebar.w,
        line_h,
    );
    p.label(
        side_label,
        t.two_column,
        pal.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
    let content_label = UiRect::new(
        cols.content.x,
        cols.content.y + (cols.content.h - line_h) / 2.0,
        cols.content.w,
        line_h,
    );
    p.label(
        content_label,
        t.settings,
        pal.text,
        FONT_SIZE,
        PaintAlign::Center,
    );
}

/// Сцена = layout через kit-функции → отрисовка через Painter (как это делают
/// потребители, `canvas-app/src/kit_ui.rs`).
fn paint_scene(
    comp: Comp,
    state: KitState,
    lang: Lang,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    p: &mut Painter,
) {
    let t = tr(lang);
    let pal = palette();
    match comp {
        Comp::Panel => scene_panel(p, t, &pal),
        Comp::Button => scene_button(p, t, &pal, state, m, fs),
        Comp::IconButton => scene_icon_button(p, &pal, state),
        Comp::Dropdown => scene_dropdown(p, t, &pal, state),
        Comp::Chip => scene_chip(p, t, &pal, state, m, fs),
        Comp::Toast => scene_toast(p, t, &pal),
        Comp::Tooltip => scene_tooltip(p, t, &pal, m, fs),
        Comp::Modal => scene_modal(p, t, &pal, state, m, fs),
        Comp::TextField => scene_text_field(p, t, &pal, state, m, fs),
        Comp::Switch => scene_switch(p, t, &pal, state),
        // --- V3: Wave C (issue #67) ---
        Comp::Checkbox => scene_checkbox(p, t, &pal, state, m, fs),
        Comp::Slider => scene_slider(p, t, &pal, state),
        Comp::Radio => scene_radio(p, t, &pal, state, m, fs),
        Comp::Tabs => scene_tabs(p, t, &pal, state, m, fs),
        Comp::Segmented => scene_segmented(p, t, &pal, state, m, fs),
        Comp::Badge => scene_badge(p, t, &pal, m, fs),
        Comp::Progress => scene_progress(p, t, &pal, state),
        Comp::Skeleton => scene_skeleton(p, t, &pal),
        Comp::Popover => scene_popover(p, t, &pal, state),
        Comp::Snackbar => scene_snackbar(p, t, &pal, state, m, fs),
        Comp::Avatar => scene_avatar(p, t, &pal),
        Comp::Tree => scene_tree(p, t, &pal, state, m, fs),
        Comp::CommandPalette => scene_command_palette(p, t, &pal, state, m, fs),
        Comp::Accordion => scene_accordion(p, t, &pal, state, m, fs),
        // --- V3: W2 (issue #67) ---
        Comp::Banner => scene_banner(p, t, &pal, state, m, fs),
        Comp::ChatBubble => scene_chat_bubble(p, t, &pal),
        Comp::Crumbs => scene_crumbs(p, t, &pal, state, m, fs),
        Comp::Footer => scene_footer(p, t, &pal, state, m, fs),
        Comp::RadioCard => scene_radio_card(p, t, &pal, state, m, fs),
        Comp::TwoColumn => scene_two_column(p, t, &pal),
    }
}

// --- Эталоны: чтение/запись/сравнение ---------------------------------------

fn update_mode() -> bool {
    std::env::var("CANVAS_UI_UPDATE_SNAPSHOTS").ok().as_deref() == Some("1")
}

fn golden_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshot")
}

fn golden_path(comp_id: &str, state_id: &str, lang_id: &str) -> std::path::PathBuf {
    golden_dir().join(format!("{comp_id}_{state_id}_{lang_id}.txt"))
}

/// Один эталонный прогон: сцена → дамп → сравнение с golden (или запись в
/// режиме `CANVAS_UI_UPDATE_SNAPSHOTS=1`).
fn run_snapshot_case(comp: Comp, state: KitState, state_id: &str, lang: Lang, lang_id: &str) {
    let mut fs = font_system();
    let mut m = TextMeasurer::new();
    let mut p = Painter::new();
    paint_scene(comp, state, lang, &mut m, &mut fs, &mut p);
    let dump = dump_items(p.items());
    assert!(
        !dump.is_empty(),
        "сцена {} отрисовала 0 items — сцена сломана",
        comp.id()
    );

    let path = golden_path(comp.id(), state_id, lang_id);
    if update_mode() {
        std::fs::create_dir_all(golden_dir()).expect("создать каталог эталонов tests/snapshot");
        std::fs::write(&path, dump).expect("записать эталон");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "эталон {} не читается ({e}) — сгенерируй: CANVAS_UI_UPDATE_SNAPSHOTS=1 \
             cargo test -p canvas-ui --test snapshot",
            path.display()
        )
    });
    // FR-068 W0 §9: Windows CI — git autocrlf конвертирует .txt-эталоны
    // в CRLF при checkout; тест-дамп всегда пишется через '\n'. Без
    // нормализации Windows-сборка паникует на эталонах (FR-068 W2 fix:
    // html5_demos — тот же паттерн; здесь — превентивно).
    let dump_norm = dump.replace("\r\n", "\n");
    let expected_norm = expected.replace("\r\n", "\n");
    assert_eq!(
        dump_norm,
        expected_norm,
        "golden-снапшот изменился — обнови эталон осознанно (FR-068 W0 §9): {}",
        path.display()
    );
}

// --- Матрица: 10 компонентов × 3 состояния × 2 языка = 60 эталонов -----------

macro_rules! snapshot_tests {
    ($($name:ident => ($comp:expr, $state:expr, $state_id:literal, $lang:expr, $lang_id:literal);)+) => {
        $(
            #[test]
            fn $name() {
                run_snapshot_case($comp, $state, $state_id, $lang, $lang_id);
            }
        )+
    };
}

snapshot_tests! {
    // Panel
    snapshot_panel_default_ru => (Comp::Panel, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_panel_default_en => (Comp::Panel, KitState::Normal, "default", Lang::En, "en");
    snapshot_panel_hover_ru => (Comp::Panel, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_panel_hover_en => (Comp::Panel, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_panel_disabled_ru => (Comp::Panel, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_panel_disabled_en => (Comp::Panel, KitState::Disabled, "disabled", Lang::En, "en");
    // Button
    snapshot_button_default_ru => (Comp::Button, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_button_default_en => (Comp::Button, KitState::Normal, "default", Lang::En, "en");
    snapshot_button_hover_ru => (Comp::Button, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_button_hover_en => (Comp::Button, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_button_disabled_ru => (Comp::Button, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_button_disabled_en => (Comp::Button, KitState::Disabled, "disabled", Lang::En, "en");
    // IconButton
    snapshot_icon_button_default_ru => (Comp::IconButton, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_icon_button_default_en => (Comp::IconButton, KitState::Normal, "default", Lang::En, "en");
    snapshot_icon_button_hover_ru => (Comp::IconButton, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_icon_button_hover_en => (Comp::IconButton, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_icon_button_disabled_ru => (Comp::IconButton, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_icon_button_disabled_en => (Comp::IconButton, KitState::Disabled, "disabled", Lang::En, "en");
    // Dropdown
    snapshot_dropdown_default_ru => (Comp::Dropdown, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_dropdown_default_en => (Comp::Dropdown, KitState::Normal, "default", Lang::En, "en");
    snapshot_dropdown_hover_ru => (Comp::Dropdown, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_dropdown_hover_en => (Comp::Dropdown, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_dropdown_disabled_ru => (Comp::Dropdown, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_dropdown_disabled_en => (Comp::Dropdown, KitState::Disabled, "disabled", Lang::En, "en");
    // Chip
    snapshot_chip_default_ru => (Comp::Chip, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_chip_default_en => (Comp::Chip, KitState::Normal, "default", Lang::En, "en");
    snapshot_chip_hover_ru => (Comp::Chip, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_chip_hover_en => (Comp::Chip, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_chip_disabled_ru => (Comp::Chip, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_chip_disabled_en => (Comp::Chip, KitState::Disabled, "disabled", Lang::En, "en");
    // Toast
    snapshot_toast_default_ru => (Comp::Toast, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_toast_default_en => (Comp::Toast, KitState::Normal, "default", Lang::En, "en");
    snapshot_toast_hover_ru => (Comp::Toast, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_toast_hover_en => (Comp::Toast, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_toast_disabled_ru => (Comp::Toast, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_toast_disabled_en => (Comp::Toast, KitState::Disabled, "disabled", Lang::En, "en");
    // Tooltip
    snapshot_tooltip_default_ru => (Comp::Tooltip, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_tooltip_default_en => (Comp::Tooltip, KitState::Normal, "default", Lang::En, "en");
    snapshot_tooltip_hover_ru => (Comp::Tooltip, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_tooltip_hover_en => (Comp::Tooltip, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_tooltip_disabled_ru => (Comp::Tooltip, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_tooltip_disabled_en => (Comp::Tooltip, KitState::Disabled, "disabled", Lang::En, "en");
    // Modal
    snapshot_modal_default_ru => (Comp::Modal, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_modal_default_en => (Comp::Modal, KitState::Normal, "default", Lang::En, "en");
    snapshot_modal_hover_ru => (Comp::Modal, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_modal_hover_en => (Comp::Modal, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_modal_disabled_ru => (Comp::Modal, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_modal_disabled_en => (Comp::Modal, KitState::Disabled, "disabled", Lang::En, "en");
    // TextField
    snapshot_text_field_default_ru => (Comp::TextField, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_text_field_default_en => (Comp::TextField, KitState::Normal, "default", Lang::En, "en");
    snapshot_text_field_hover_ru => (Comp::TextField, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_text_field_hover_en => (Comp::TextField, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_text_field_disabled_ru => (Comp::TextField, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_text_field_disabled_en => (Comp::TextField, KitState::Disabled, "disabled", Lang::En, "en");
    // Switch
    snapshot_switch_default_ru => (Comp::Switch, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_switch_default_en => (Comp::Switch, KitState::Normal, "default", Lang::En, "en");
    snapshot_switch_hover_ru => (Comp::Switch, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_switch_hover_en => (Comp::Switch, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_switch_disabled_ru => (Comp::Switch, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_switch_disabled_en => (Comp::Switch, KitState::Disabled, "disabled", Lang::En, "en");
    // Checkbox (Wave C, issue #67 V3)
    snapshot_checkbox_default_ru => (Comp::Checkbox, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_checkbox_default_en => (Comp::Checkbox, KitState::Normal, "default", Lang::En, "en");
    snapshot_checkbox_hover_ru => (Comp::Checkbox, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_checkbox_hover_en => (Comp::Checkbox, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_checkbox_disabled_ru => (Comp::Checkbox, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_checkbox_disabled_en => (Comp::Checkbox, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_checkbox_error_ru => (Comp::Checkbox, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_checkbox_error_en => (Comp::Checkbox, KitState::Error, "error", Lang::En, "en");
    // Slider (Wave C, issue #67 V3)
    snapshot_slider_default_ru => (Comp::Slider, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_slider_default_en => (Comp::Slider, KitState::Normal, "default", Lang::En, "en");
    snapshot_slider_hover_ru => (Comp::Slider, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_slider_hover_en => (Comp::Slider, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_slider_disabled_ru => (Comp::Slider, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_slider_disabled_en => (Comp::Slider, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_slider_error_ru => (Comp::Slider, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_slider_error_en => (Comp::Slider, KitState::Error, "error", Lang::En, "en");
    // Radio (Wave C, issue #67 V3)
    snapshot_radio_default_ru => (Comp::Radio, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_radio_default_en => (Comp::Radio, KitState::Normal, "default", Lang::En, "en");
    snapshot_radio_hover_ru => (Comp::Radio, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_radio_hover_en => (Comp::Radio, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_radio_disabled_ru => (Comp::Radio, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_radio_disabled_en => (Comp::Radio, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_radio_error_ru => (Comp::Radio, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_radio_error_en => (Comp::Radio, KitState::Error, "error", Lang::En, "en");
    // Tabs (Wave C, issue #67 V3)
    snapshot_tabs_default_ru => (Comp::Tabs, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_tabs_default_en => (Comp::Tabs, KitState::Normal, "default", Lang::En, "en");
    snapshot_tabs_hover_ru => (Comp::Tabs, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_tabs_hover_en => (Comp::Tabs, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_tabs_disabled_ru => (Comp::Tabs, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_tabs_disabled_en => (Comp::Tabs, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_tabs_error_ru => (Comp::Tabs, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_tabs_error_en => (Comp::Tabs, KitState::Error, "error", Lang::En, "en");
    // Segmented (Wave C, issue #67 V3)
    snapshot_segmented_default_ru => (Comp::Segmented, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_segmented_default_en => (Comp::Segmented, KitState::Normal, "default", Lang::En, "en");
    snapshot_segmented_hover_ru => (Comp::Segmented, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_segmented_hover_en => (Comp::Segmented, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_segmented_disabled_ru => (Comp::Segmented, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_segmented_disabled_en => (Comp::Segmented, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_segmented_error_ru => (Comp::Segmented, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_segmented_error_en => (Comp::Segmented, KitState::Error, "error", Lang::En, "en");
    // Badge (Wave C, issue #67 V3)
    snapshot_badge_default_ru => (Comp::Badge, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_badge_default_en => (Comp::Badge, KitState::Normal, "default", Lang::En, "en");
    snapshot_badge_hover_ru => (Comp::Badge, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_badge_hover_en => (Comp::Badge, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_badge_disabled_ru => (Comp::Badge, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_badge_disabled_en => (Comp::Badge, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_badge_error_ru => (Comp::Badge, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_badge_error_en => (Comp::Badge, KitState::Error, "error", Lang::En, "en");
    // Progress (Wave C, issue #67 V3)
    snapshot_progress_default_ru => (Comp::Progress, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_progress_default_en => (Comp::Progress, KitState::Normal, "default", Lang::En, "en");
    snapshot_progress_hover_ru => (Comp::Progress, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_progress_hover_en => (Comp::Progress, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_progress_disabled_ru => (Comp::Progress, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_progress_disabled_en => (Comp::Progress, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_progress_error_ru => (Comp::Progress, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_progress_error_en => (Comp::Progress, KitState::Error, "error", Lang::En, "en");
    // Skeleton (Wave C, issue #67 V3)
    snapshot_skeleton_default_ru => (Comp::Skeleton, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_skeleton_default_en => (Comp::Skeleton, KitState::Normal, "default", Lang::En, "en");
    snapshot_skeleton_hover_ru => (Comp::Skeleton, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_skeleton_hover_en => (Comp::Skeleton, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_skeleton_disabled_ru => (Comp::Skeleton, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_skeleton_disabled_en => (Comp::Skeleton, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_skeleton_error_ru => (Comp::Skeleton, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_skeleton_error_en => (Comp::Skeleton, KitState::Error, "error", Lang::En, "en");
    // Popover (Wave C, issue #67 V3)
    snapshot_popover_default_ru => (Comp::Popover, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_popover_default_en => (Comp::Popover, KitState::Normal, "default", Lang::En, "en");
    snapshot_popover_hover_ru => (Comp::Popover, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_popover_hover_en => (Comp::Popover, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_popover_disabled_ru => (Comp::Popover, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_popover_disabled_en => (Comp::Popover, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_popover_error_ru => (Comp::Popover, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_popover_error_en => (Comp::Popover, KitState::Error, "error", Lang::En, "en");
    // Snackbar (Wave C, issue #67 V3)
    snapshot_snackbar_default_ru => (Comp::Snackbar, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_snackbar_default_en => (Comp::Snackbar, KitState::Normal, "default", Lang::En, "en");
    snapshot_snackbar_hover_ru => (Comp::Snackbar, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_snackbar_hover_en => (Comp::Snackbar, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_snackbar_disabled_ru => (Comp::Snackbar, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_snackbar_disabled_en => (Comp::Snackbar, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_snackbar_error_ru => (Comp::Snackbar, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_snackbar_error_en => (Comp::Snackbar, KitState::Error, "error", Lang::En, "en");
    // Avatar (Wave C, issue #67 V3)
    snapshot_avatar_default_ru => (Comp::Avatar, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_avatar_default_en => (Comp::Avatar, KitState::Normal, "default", Lang::En, "en");
    snapshot_avatar_hover_ru => (Comp::Avatar, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_avatar_hover_en => (Comp::Avatar, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_avatar_disabled_ru => (Comp::Avatar, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_avatar_disabled_en => (Comp::Avatar, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_avatar_error_ru => (Comp::Avatar, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_avatar_error_en => (Comp::Avatar, KitState::Error, "error", Lang::En, "en");
    // Tree (Wave C, issue #67 V3)
    snapshot_tree_default_ru => (Comp::Tree, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_tree_default_en => (Comp::Tree, KitState::Normal, "default", Lang::En, "en");
    snapshot_tree_hover_ru => (Comp::Tree, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_tree_hover_en => (Comp::Tree, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_tree_disabled_ru => (Comp::Tree, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_tree_disabled_en => (Comp::Tree, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_tree_error_ru => (Comp::Tree, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_tree_error_en => (Comp::Tree, KitState::Error, "error", Lang::En, "en");
    // CommandPalette (Wave C, issue #67 V3)
    snapshot_command_palette_default_ru => (Comp::CommandPalette, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_command_palette_default_en => (Comp::CommandPalette, KitState::Normal, "default", Lang::En, "en");
    snapshot_command_palette_hover_ru => (Comp::CommandPalette, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_command_palette_hover_en => (Comp::CommandPalette, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_command_palette_disabled_ru => (Comp::CommandPalette, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_command_palette_disabled_en => (Comp::CommandPalette, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_command_palette_error_ru => (Comp::CommandPalette, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_command_palette_error_en => (Comp::CommandPalette, KitState::Error, "error", Lang::En, "en");
    // Accordion (Wave C, issue #67 V3)
    snapshot_accordion_default_ru => (Comp::Accordion, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_accordion_default_en => (Comp::Accordion, KitState::Normal, "default", Lang::En, "en");
    snapshot_accordion_hover_ru => (Comp::Accordion, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_accordion_hover_en => (Comp::Accordion, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_accordion_disabled_ru => (Comp::Accordion, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_accordion_disabled_en => (Comp::Accordion, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_accordion_error_ru => (Comp::Accordion, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_accordion_error_en => (Comp::Accordion, KitState::Error, "error", Lang::En, "en");
    // Banner (W2, issue #67 V3)
    snapshot_banner_default_ru => (Comp::Banner, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_banner_default_en => (Comp::Banner, KitState::Normal, "default", Lang::En, "en");
    snapshot_banner_hover_ru => (Comp::Banner, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_banner_hover_en => (Comp::Banner, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_banner_disabled_ru => (Comp::Banner, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_banner_disabled_en => (Comp::Banner, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_banner_error_ru => (Comp::Banner, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_banner_error_en => (Comp::Banner, KitState::Error, "error", Lang::En, "en");
    // ChatBubble (W2, issue #67 V3)
    snapshot_chat_bubble_default_ru => (Comp::ChatBubble, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_chat_bubble_default_en => (Comp::ChatBubble, KitState::Normal, "default", Lang::En, "en");
    snapshot_chat_bubble_hover_ru => (Comp::ChatBubble, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_chat_bubble_hover_en => (Comp::ChatBubble, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_chat_bubble_disabled_ru => (Comp::ChatBubble, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_chat_bubble_disabled_en => (Comp::ChatBubble, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_chat_bubble_error_ru => (Comp::ChatBubble, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_chat_bubble_error_en => (Comp::ChatBubble, KitState::Error, "error", Lang::En, "en");
    // Crumbs (W2, issue #67 V3)
    snapshot_crumbs_default_ru => (Comp::Crumbs, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_crumbs_default_en => (Comp::Crumbs, KitState::Normal, "default", Lang::En, "en");
    snapshot_crumbs_hover_ru => (Comp::Crumbs, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_crumbs_hover_en => (Comp::Crumbs, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_crumbs_disabled_ru => (Comp::Crumbs, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_crumbs_disabled_en => (Comp::Crumbs, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_crumbs_error_ru => (Comp::Crumbs, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_crumbs_error_en => (Comp::Crumbs, KitState::Error, "error", Lang::En, "en");
    // Footer (W2, issue #67 V3)
    snapshot_footer_default_ru => (Comp::Footer, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_footer_default_en => (Comp::Footer, KitState::Normal, "default", Lang::En, "en");
    snapshot_footer_hover_ru => (Comp::Footer, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_footer_hover_en => (Comp::Footer, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_footer_disabled_ru => (Comp::Footer, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_footer_disabled_en => (Comp::Footer, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_footer_error_ru => (Comp::Footer, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_footer_error_en => (Comp::Footer, KitState::Error, "error", Lang::En, "en");
    // RadioCard (W2, issue #67 V3)
    snapshot_radio_card_default_ru => (Comp::RadioCard, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_radio_card_default_en => (Comp::RadioCard, KitState::Normal, "default", Lang::En, "en");
    snapshot_radio_card_hover_ru => (Comp::RadioCard, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_radio_card_hover_en => (Comp::RadioCard, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_radio_card_disabled_ru => (Comp::RadioCard, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_radio_card_disabled_en => (Comp::RadioCard, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_radio_card_error_ru => (Comp::RadioCard, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_radio_card_error_en => (Comp::RadioCard, KitState::Error, "error", Lang::En, "en");
    // TwoColumn (W2, issue #67 V3)
    snapshot_two_column_default_ru => (Comp::TwoColumn, KitState::Normal, "default", Lang::Ru, "ru");
    snapshot_two_column_default_en => (Comp::TwoColumn, KitState::Normal, "default", Lang::En, "en");
    snapshot_two_column_hover_ru => (Comp::TwoColumn, KitState::Hovered, "hover", Lang::Ru, "ru");
    snapshot_two_column_hover_en => (Comp::TwoColumn, KitState::Hovered, "hover", Lang::En, "en");
    snapshot_two_column_disabled_ru => (Comp::TwoColumn, KitState::Disabled, "disabled", Lang::Ru, "ru");
    snapshot_two_column_disabled_en => (Comp::TwoColumn, KitState::Disabled, "disabled", Lang::En, "en");
    snapshot_two_column_error_ru => (Comp::TwoColumn, KitState::Error, "error", Lang::Ru, "ru");
    snapshot_two_column_error_en => (Comp::TwoColumn, KitState::Error, "error", Lang::En, "en");
}

/// Счётчик матрицы: РОВНО 220 эталонов = 60 (W0: 10 × 3 × 2) + 160
/// (V3 issue #67: 20 × 4 × 2) — каждая ячейка матрицы имеет файл на диске
/// и лишних .txt нет. Режим обновления пропускает подсчёт (файлы пишутся
/// параллельными тестами).
#[test]
fn snapshot_count_is_220() {
    // W0: 10 компонентов × 3 состояния × 2 языка = 60.
    assert_eq!(COMP_IDS.len(), 10, "матрица W0: 10 kit-компонентов");
    assert_eq!(STATE_IDS.len(), 3, "матрица W0: 3 состояния");
    assert_eq!(LANG_IDS.len(), 2, "матрица: 2 языка");
    assert_eq!(
        COMP_IDS.len() * STATE_IDS.len() * LANG_IDS.len(),
        60,
        "W0: 10 компонентов × 3 состояния × 2 языка = 60 эталонов"
    );
    // V3 (issue #67): 20 компонентов × 4 состояния × 2 языка = 160.
    assert_eq!(
        NEW_COMP_IDS.len(),
        20,
        "матрица V3: 20 компонентов Wave C+W2"
    );
    assert_eq!(NEW_STATE_IDS.len(), 4, "матрица V3: 4 состояния (K4)");
    assert_eq!(
        NEW_COMP_IDS.len() * NEW_STATE_IDS.len() * LANG_IDS.len(),
        160,
        "V3: 20 компонентов × 4 состояния × 2 языка = 160 эталонов"
    );
    assert_eq!(60 + 160, 220, "итого 220 эталонов в tests/snapshot");
    if update_mode() {
        return;
    }
    let files: Vec<String> = std::fs::read_dir(golden_dir())
        .expect("каталог эталонов tests/snapshot существует")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    assert_eq!(
        files.len(),
        220,
        "в tests/snapshot должно лежать ровно 220 эталонов (.txt)"
    );
    for comp in COMP_IDS {
        for state in STATE_IDS {
            for lang in LANG_IDS {
                let name = format!("{comp}_{state}_{lang}.txt");
                assert!(
                    files.iter().any(|f| f == &name),
                    "нет эталона {name} — ячейка матрицы W0 пуста"
                );
            }
        }
    }
    for comp in NEW_COMP_IDS {
        for state in NEW_STATE_IDS {
            for lang in LANG_IDS {
                let name = format!("{comp}_{state}_{lang}.txt");
                assert!(
                    files.iter().any(|f| f == &name),
                    "нет эталона {name} — ячейка матрицы V3 пуста"
                );
            }
        }
    }
}
