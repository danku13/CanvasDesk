//! Wave D v1 (issue #32): интерактивная demo-витрина ui-kit.
//!
//! Модуль — чистая модель демо-состояния и переходов (TDD-ядро волны D):
//! [`KitDemoState`] — всё интерактивное состояние витрины (тулбар тем/языка/
//! размера/плотности, AS-IS/TO-BE, состояния компонентов D1/D2), реестр
//! секций сайдбара ([`SECTIONS`], группы по issue D5) и машина переходов
//! кликов ([`apply_click`]) — раскладка/отрисовка остаются в [`crate::kit_ui`].
//!
//! Контракт draw==hit (PRD-0009): id-строки хитов формирует
//! [`hit_id`]/[`HitId`]-константы — их же использует раскладка при
//! регистрации `demo_hits` и обработчик кликов в `input.rs`.

use crate::i18n::keys;
use canvas_ui::component::checkbox::CheckboxState;
use canvas_ui::component::ControlSize;
use canvas_ui::layout::Density;

/// Префикс id хитов демо-витрины (реестр поверхностей — `fill_hit_rects`).
pub const HIT_PREFIX: &str = "kit-demo:";

/// Хит-цели демо-витрины (строки после [`HIT_PREFIX`]; формирует раскладка).
/// Индексированные id — статические таблицы (реестр требует &'static str;
/// выход за таблицу — пустой id, клик игнорируется — без паник в проде).
pub mod hit {
    /// Тулбар: AS-IS/TO-BE.
    pub const TOOLBAR_MODE: &str = "toolbar:mode";
    /// Тулбар: следующий пресет темы.
    pub const TOOLBAR_THEME: &str = "toolbar:theme";
    /// Тулбар: RU/EN.
    pub const TOOLBAR_LANG: &str = "toolbar:lang";
    /// Тулбар: следующий ControlSize.
    pub const TOOLBAR_SIZE: &str = "toolbar:size";
    /// Тулбар: следующая Density.
    pub const TOOLBAR_DENSITY: &str = "toolbar:density";

    /// Таблица `prefix:i` (0..N) — задаётся вручную (статические строки).
    macro_rules! ids {
        ($prefix:literal, [$($i:literal),+]) => {
            [ $( concat!($prefix, ":", stringify!($i)) ),+ ]
        };
    }

    /// Чипы секции Chips.
    pub const CHIP_IDS: [&str; 4] = ids!("chips", [0, 1, 2, 3]);
    /// Кнопки интерактивного ряда секции Buttons.
    pub const BUTTON_IDS: [&str; 4] = ids!("button", [0, 1, 2, 3]);
    /// Строки демо-списка секции List.
    pub const LIST_IDS: [&str; 8] = ids!("list", [0, 1, 2, 3, 4, 5, 6, 7]);
    /// Пункты dropdown-меню.
    pub const DROPDOWN_IDS: [&str; 3] = ids!("dropdown", [0, 1, 2]);
    /// Радио-пункты.
    pub const RADIO_IDS: [&str; 3] = ids!("radio", [0, 1, 2]);
    /// Вкладки.
    pub const TABS_IDS: [&str; 3] = ids!("tabs", [0, 1, 2]);
    /// Сегменты SegmentedControl.
    pub const SEGMENTED_IDS: [&str; 3] = ids!("segmented", [0, 1, 2]);
    /// Листья дерева (корень — TREE_TOGGLE).
    pub const TREE_IDS: [&str; 3] = ids!("tree", [0, 1, 2]);
    /// Пункты демо-палитры команд.
    pub const PALETTE_IDS: [&str; 3] = ids!("palette", [0, 1, 2]);

    /// Взять статический id из таблицы (вне диапазона — пустой → игнор).
    fn pick(table: &[&'static str], i: usize) -> &'static str {
        table.get(i).copied().unwrap_or("")
    }

    /// Чип секции Chips (индекс).
    pub fn chip(i: usize) -> &'static str {
        pick(&CHIP_IDS, i)
    }
    /// Кнопка интерактивного ряда.
    pub fn button(i: usize) -> &'static str {
        pick(&BUTTON_IDS, i)
    }
    /// Строка списка секции List (индекс).
    pub fn list_row(i: usize) -> &'static str {
        pick(&LIST_IDS, i)
    }
    /// Пункт dropdown-меню (индекс).
    pub fn dropdown_item(i: usize) -> &'static str {
        pick(&DROPDOWN_IDS, i)
    }
    /// Радио-пункт (индекс).
    pub fn radio(i: usize) -> &'static str {
        pick(&RADIO_IDS, i)
    }
    /// Вкладка (индекс).
    pub fn tab(i: usize) -> &'static str {
        pick(&TABS_IDS, i)
    }
    /// Заголовок секции аккордеона (индекс).
    pub fn accordion(i: usize) -> &'static str {
        pick(&ACCORDION_IDS, i)
    }
    pub const ACCORDION_IDS: [&str; 3] = ids!("accordion", [0, 1, 2]);
    /// Сегмент SegmentedControl (индекс).
    pub fn segmented(i: usize) -> &'static str {
        pick(&SEGMENTED_IDS, i)
    }
    /// Узел дерева (индекс листа).
    pub fn tree(i: usize) -> &'static str {
        pick(&TREE_IDS, i)
    }
    /// Toggle-строка группы дерева.
    pub const TREE_TOGGLE: &str = "tree_toggle";
    /// Пункт палитры команд (индекс).
    pub fn palette_item(i: usize) -> &'static str {
        pick(&PALETTE_IDS, i)
    }
    /// Dropdown: якорь (open/close).
    pub const DROPDOWN_ANCHOR: &str = "dropdown:anchor";
    /// Переключатель секции Switch.
    pub const SWITCH: &str = "switch";
    /// Демо-текстовое поле (фокус/ввод).
    pub const TEXTFIELD: &str = "textfield";
    /// Модалка: открыть / закрыть (крестик).
    pub const MODAL_OPEN: &str = "modal:open";
    pub const MODAL_CLOSE: &str = "modal:close";
    /// Чек-бокс (цикл 3 состояний).
    pub const CHECKBOX: &str = "checkbox";
    /// Слайдер (drag/клик по треку).
    pub const SLIDER: &str = "slider";
    /// Снекбар: показать/закрыть.
    pub const SNACKBAR_SHOW: &str = "snackbar:show";
    pub const SNACKBAR_CLOSE: &str = "snackbar:close";
    /// Поповер: якорь / закрытие.
    pub const POPOVER_ANCHOR: &str = "popover:anchor";
    pub const POPOVER_CLOSE: &str = "popover:close";
    /// Command palette: открыть / закрыть (Esc/backdrop).
    pub const PALETTE_OPEN: &str = "palette:open";
    pub const PALETTE_CLOSE: &str = "palette:close";
    /// Тост: показать/скрыть (демо действия).
    pub const TOAST_TOGGLE: &str = "toast:toggle";

    /// Навигационные id сайдбара: `nav:<id секции>` (реестр требует
    /// статические строки — таблица выводится из реестра SECTIONS вручную).
    pub const NAV_IDS: [&str; 26] = [
        "nav:buttons",
        "nav:icon_buttons",
        "nav:chips",
        "nav:text_field",
        "nav:switch",
        "nav:checkbox",
        "nav:slider",
        "nav:radio",
        "nav:dropdown",
        "nav:tabs",
        "nav:segmented",
        "nav:command_palette",
        "nav:tree",
        "nav:card",
        "nav:modal",
        "nav:accordion",
        "nav:table",
        "nav:list",
        "nav:badge",
        "nav:progress",
        "nav:skeleton",
        "nav:icons",
        "nav:toast",
        "nav:tooltip",
        "nav:snackbar",
        "nav:popover",
    ];

    /// Статический nav-id по id секции (неизвестный — пустой → игнор).
    pub fn nav(section_id: &str) -> &'static str {
        NAV_IDS
            .iter()
            .copied()
            .find(|n| n.strip_prefix("nav:") == Some(section_id))
            .unwrap_or("")
    }
}

/// Разбор хит-id вида `section[:index]` → (секция, индекс).
/// Числовой суффикс — индекс (`chips:2`); нечисловой — часть имени секции
/// (`dropdown:anchor`, `toolbar:mode` — цель без индекса). Пустой id — None.
pub fn parse_hit(id: &str) -> Option<(&str, Option<usize>)> {
    if id.is_empty() {
        return None;
    }
    match id.split_once(':') {
        Some((section, index)) => match index.parse::<usize>() {
            Ok(n) => Some((section, Some(n))),
            Err(_) => Some((id, None)),
        },
        None => Some((id, None)),
    }
}

/// Группы секций сайдбара (issue #32 D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoGroup {
    /// Ввод: кнопки, поля, переключатели.
    Inputs,
    /// Навигация: dropdown, табы, дерево, палитра команд.
    Navigation,
    /// Контейнеры: карточка, модалка, аккордеон, панель.
    Containers,
    /// Данные: таблица, список, бейджи, прогресс, скелетон.
    DataDisplay,
    /// Фидбек: тост, тултип, снекбар, поповер.
    Feedback,
}

/// Ключ i18n заголовка группы (сайдбар, RU/EN).
pub fn group_title_key(group: DemoGroup) -> &'static str {
    match group {
        DemoGroup::Inputs => keys::KIT_DEMO_GROUP_INPUTS,
        DemoGroup::Navigation => keys::KIT_DEMO_GROUP_NAVIGATION,
        DemoGroup::Containers => keys::KIT_DEMO_GROUP_CONTAINERS,
        DemoGroup::DataDisplay => keys::KIT_DEMO_GROUP_DATA,
        DemoGroup::Feedback => keys::KIT_DEMO_GROUP_FEEDBACK,
    }
}

/// Секция демо-витрины (сайдбар D5 + документация D7).
#[derive(Debug, Clone, Copy)]
pub struct DemoSection {
    /// Стабильный id (= первый сегмент хит-id секции, где применимо).
    pub id: &'static str,
    /// Группа сайдбара.
    pub group: DemoGroup,
    /// Ключ i18n заголовка секции.
    pub title_key: &'static str,
    /// Ключ i18n описания (D7: описание + API + a11y — краткая строка).
    pub desc_key: &'static str,
}

/// Реестр секций TO-BE витрины (порядок = порядок линейки раскладки).
/// Секции layout-инженерии (measured/grow/wrap/grid/…, LAY-SHOWCASE)
/// остаются в скролле, но в сайдбар не входят — это демонстрация движка
/// раскладки, а не компонентов (решение v1, документировано в D7).
pub const SECTIONS: &[DemoSection] = &[
    // --- Inputs ---
    DemoSection {
        id: "buttons",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_BUTTONS,
        desc_key: keys::KIT_DESC_BUTTONS,
    },
    DemoSection {
        id: "icon_buttons",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_ICON,
        desc_key: keys::KIT_DESC_ICON_BUTTONS,
    },
    DemoSection {
        id: "chips",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_CHIPS,
        desc_key: keys::KIT_DESC_CHIPS,
    },
    DemoSection {
        id: "text_field",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_TEXT_FIELD,
        desc_key: keys::KIT_DESC_TEXT_FIELD,
    },
    DemoSection {
        id: "switch",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_SWITCH,
        desc_key: keys::KIT_DESC_SWITCH,
    },
    DemoSection {
        id: "checkbox",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_CHECKBOX,
        desc_key: keys::KIT_DESC_CHECKBOX,
    },
    DemoSection {
        id: "slider",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_SLIDER,
        desc_key: keys::KIT_DESC_SLIDER,
    },
    DemoSection {
        id: "radio",
        group: DemoGroup::Inputs,
        title_key: keys::KIT_SECTION_RADIO,
        desc_key: keys::KIT_DESC_RADIO,
    },
    // --- Navigation ---
    DemoSection {
        id: "dropdown",
        group: DemoGroup::Navigation,
        title_key: keys::KIT_SECTION_DROPDOWN,
        desc_key: keys::KIT_DESC_DROPDOWN,
    },
    DemoSection {
        id: "tabs",
        group: DemoGroup::Navigation,
        title_key: keys::KIT_SECTION_TABS,
        desc_key: keys::KIT_DESC_TABS,
    },
    DemoSection {
        id: "segmented",
        group: DemoGroup::Navigation,
        title_key: keys::KIT_SECTION_SEGMENTED,
        desc_key: keys::KIT_DESC_SEGMENTED,
    },
    DemoSection {
        id: "command_palette",
        group: DemoGroup::Navigation,
        title_key: keys::KIT_SECTION_COMMAND_PALETTE,
        desc_key: keys::KIT_DESC_COMMAND_PALETTE,
    },
    DemoSection {
        id: "tree",
        group: DemoGroup::Navigation,
        title_key: keys::KIT_SECTION_TREE,
        desc_key: keys::KIT_DESC_TREE,
    },
    // --- Containers ---
    DemoSection {
        id: "card",
        group: DemoGroup::Containers,
        title_key: keys::KIT_SECTION_CARD,
        desc_key: keys::KIT_DESC_CARD,
    },
    DemoSection {
        id: "modal",
        group: DemoGroup::Containers,
        title_key: keys::KIT_SECTION_MODAL,
        desc_key: keys::KIT_DESC_MODAL,
    },
    DemoSection {
        id: "accordion",
        group: DemoGroup::Containers,
        title_key: keys::KIT_SECTION_ACCORDION,
        desc_key: keys::KIT_DESC_ACCORDION,
    },
    // --- Data Display ---
    DemoSection {
        id: "table",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_TABLE,
        desc_key: keys::KIT_DESC_TABLE,
    },
    DemoSection {
        id: "list",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_LIST,
        desc_key: keys::KIT_DESC_LIST,
    },
    DemoSection {
        id: "badge",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_BADGE,
        desc_key: keys::KIT_DESC_BADGE,
    },
    DemoSection {
        id: "progress",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_PROGRESS,
        desc_key: keys::KIT_DESC_PROGRESS,
    },
    DemoSection {
        id: "skeleton",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_SKELETON,
        desc_key: keys::KIT_DESC_SKELETON,
    },
    DemoSection {
        id: "icons",
        group: DemoGroup::DataDisplay,
        title_key: keys::KIT_SECTION_ICONS,
        desc_key: keys::KIT_DESC_ICONS,
    },
    // --- Feedback ---
    DemoSection {
        id: "toast",
        group: DemoGroup::Feedback,
        title_key: keys::KIT_SECTION_TOAST,
        desc_key: keys::KIT_DESC_TOAST,
    },
    DemoSection {
        id: "tooltip",
        group: DemoGroup::Feedback,
        title_key: keys::KIT_SECTION_TOOLTIP,
        desc_key: keys::KIT_DESC_TOOLTIP,
    },
    DemoSection {
        id: "snackbar",
        group: DemoGroup::Feedback,
        title_key: keys::KIT_SECTION_SNACKBAR,
        desc_key: keys::KIT_DESC_SNACKBAR,
    },
    DemoSection {
        id: "popover",
        group: DemoGroup::Feedback,
        title_key: keys::KIT_SECTION_POPOVER,
        desc_key: keys::KIT_DESC_POPOVER,
    },
];

/// Порядок секций Wave C в хвосте линейки раскладки (TO-BE) — должен
/// совпадать с порядком потребления в `kit_ui::wave_c_consume`.
pub const WAVE_C_ORDER: &[&str] = &[
    "checkbox",
    "slider",
    "radio",
    "tabs",
    "segmented",
    "command_palette",
    "tree",
    "modal",
    "accordion",
    "badge",
    "progress",
    "skeleton",
    "snackbar",
    "popover",
];

/// Секция по id (сайдбар → раскладка/линт).
pub fn section(id: &str) -> Option<&'static DemoSection> {
    SECTIONS.iter().find(|s| s.id == id)
}

/// Интерактивное состояние demo-витрины (волна D v1, issue #32).
///
/// Живёт в App рядом с `kit_gallery_scroll`; раскладка читает его для
/// TO-BE-режима (интерактив), отрисовка — для состояний контролов.
#[derive(Debug, Clone)]
pub struct KitDemoState {
    /// D4: TO-BE (интерактивная витрина) / AS-IS (статичный «до волн»).
    pub tobe: bool,
    /// D3: индекс пресета темы ([`canvas_core::theme_presets::PRESETS`]).
    pub theme_idx: usize,
    /// D3: язык демо — None = следовать языку приложения.
    pub lang_override: Option<canvas_core::Language>,
    /// D3: размер контролов (Xs/Sm/Md/Lg).
    pub size: ControlSize,
    /// D3: плотность (Compact/Comfortable/Spacious).
    pub density: Density,
    /// Зажатый контрол (press-визуал; ставит input на Pressed-фазе).
    pub pressed: Option<&'static str>,
    // --- D1: интерактивность существующих секций ---
    /// Чипы секции Chips: click → selected toggle.
    pub chips: [bool; 4],
    /// Dropdown: меню открыто.
    pub dropdown_open: bool,
    /// Dropdown: выбранный пункт.
    pub dropdown_sel: usize,
    /// TextField: содержимое демо-поля (ввод через IME-маршрут).
    pub text_value: String,
    /// TextField: поле в фокусе (каретка).
    pub text_focus: bool,
    /// Switch: включён.
    pub switch_on: bool,
    /// List: выделенная строка.
    pub list_sel: usize,
    /// Modal: демо-модалка открыта.
    pub modal_open: bool,
    /// Toast: тост показан (демо действия «показать/скрыть»).
    pub toast_visible: bool,
    // --- D2: компоненты Wave C ---
    /// Checkbox: состояние (клик — цикл Unchecked→Checked→Indeterminate).
    pub checkbox: CheckboxState,
    /// Slider: значение 0.0..=1.0 (drag/клик по треку).
    pub slider_value: f32,
    /// RadioGroup: выбранный пункт.
    pub radio_idx: usize,
    /// Tabs: активная вкладка.
    pub tab_idx: usize,
    /// Accordion: раскрытые секции (3 заголовка).
    pub accordion_open: [bool; 3],
    /// SegmentedControl: активный сегмент.
    pub segmented_idx: usize,
    /// Tree: группа раскрыта.
    pub tree_open: bool,
    /// Tree: выбранный узел.
    pub tree_sel: usize,
    /// Snackbar: показан.
    pub snackbar_visible: bool,
    /// Popover: пузырь открыт.
    pub popover_open: bool,
    /// Command palette: открыта (демо-минивитрина внутри панели).
    pub palette_open: bool,
    /// Command palette: запрос (поле ввода).
    pub palette_query: String,
}

impl Default for KitDemoState {
    fn default() -> Self {
        Self {
            tobe: false,
            theme_idx: 0,
            lang_override: None,
            size: ControlSize::Md,
            density: Density::Comfortable,
            pressed: None,
            chips: [false; 4],
            dropdown_open: false,
            dropdown_sel: 0,
            text_value: String::new(),
            text_focus: false,
            switch_on: false,
            list_sel: 0,
            modal_open: false,
            toast_visible: true,
            checkbox: CheckboxState::Unchecked,
            slider_value: 0.35,
            radio_idx: 0,
            tab_idx: 0,
            accordion_open: [true, false, false],
            segmented_idx: 0,
            tree_open: true,
            tree_sel: 0,
            snackbar_visible: false,
            popover_open: false,
            palette_open: false,
            palette_query: String::new(),
        }
    }
}

/// Эффект клика (переход + побочный эффект для App).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DemoEffect {
    /// Только перерисовка (состояние изменилось).
    Redraw,
    /// Изменений нет (клик мимо интерактивной цели).
    Ignored,
}

/// Следующий размер контролов (цикл Xs→Sm→Md→Lg→Xs).
pub fn next_size(size: ControlSize) -> ControlSize {
    match size {
        ControlSize::Xs => ControlSize::Sm,
        ControlSize::Sm => ControlSize::Md,
        ControlSize::Md => ControlSize::Lg,
        ControlSize::Lg => ControlSize::Xs,
    }
}

/// Следующая плотность (цикл Compact→Comfortable→Spacious→Compact).
pub fn next_density(d: Density) -> Density {
    match d {
        Density::Compact => Density::Comfortable,
        Density::Comfortable => Density::Spacious,
        Density::Spacious => Density::Compact,
    }
}

/// Полный хит-id из сегментов (`kit-demo:` + rest).
pub fn hit_id(rest: &str) -> String {
    format!("{HIT_PREFIX}{rest}")
}

/// Применить клик по демо-контролу (id БЕЗ [`HIT_PREFIX`]).
///
/// Чистая функция переходов — тесты пинят машину состояний (TDD-ядро D1/D2);
/// навигация сайдбара обрабатывается в App (нужен скролл-стейт).
pub fn apply_click(demo: &mut KitDemoState, id: &str) -> DemoEffect {
    let Some((section, index)) = parse_hit(id) else {
        return DemoEffect::Ignored;
    };
    match (section, index) {
        // --- Тулбар (D3/D4) ---
        (hit::TOOLBAR_MODE, _) => {
            demo.tobe = !demo.tobe;
            if !demo.tobe {
                // AS-IS закрывает всплывающие демо (модалка/палитра и т.п.)
                demo.modal_open = false;
                demo.palette_open = false;
                demo.popover_open = false;
                demo.dropdown_open = false;
                demo.text_focus = false;
            }
        }
        (hit::TOOLBAR_THEME, _) => {
            demo.theme_idx = (demo.theme_idx + 1) % canvas_core::theme_presets::PRESETS.len();
        }
        (hit::TOOLBAR_LANG, _) => {
            demo.lang_override = match demo.lang_override {
                Some(canvas_core::Language::Ru) | None => Some(canvas_core::Language::En),
                Some(canvas_core::Language::En) => Some(canvas_core::Language::Ru),
            };
        }
        (hit::TOOLBAR_SIZE, _) => demo.size = next_size(demo.size),
        (hit::TOOLBAR_DENSITY, _) => demo.density = next_density(demo.density),
        // --- D1: существующие секции ---
        ("button", Some(_)) => {
            // Клик-фидбек — визуальное состояние (M3 state-layer) в draw.
        }
        (hit::TEXTFIELD, _) => demo.text_focus = true,
        ("chips", Some(i)) if i < demo.chips.len() => demo.chips[i] = !demo.chips[i],
        (hit::DROPDOWN_ANCHOR, _) => demo.dropdown_open = !demo.dropdown_open,
        ("dropdown", Some(i)) => {
            demo.dropdown_sel = i;
            demo.dropdown_open = false;
        }
        (hit::SWITCH, _) => demo.switch_on = !demo.switch_on,
        ("list", Some(i)) => demo.list_sel = i,
        (hit::MODAL_OPEN, _) => demo.modal_open = true,
        (hit::MODAL_CLOSE, _) => demo.modal_open = false,
        (hit::TOAST_TOGGLE, _) => demo.toast_visible = !demo.toast_visible,
        // --- D2: Wave C ---
        (hit::CHECKBOX, _) => {
            demo.checkbox = match demo.checkbox {
                CheckboxState::Unchecked => CheckboxState::Checked,
                CheckboxState::Checked => CheckboxState::Indeterminate,
                CheckboxState::Indeterminate => CheckboxState::Unchecked,
            };
        }
        ("radio", Some(i)) => demo.radio_idx = i,
        ("tabs", Some(i)) => demo.tab_idx = i,
        ("accordion", Some(i)) if i < 3 => demo.accordion_open[i] = !demo.accordion_open[i],
        ("segmented", Some(i)) => demo.segmented_idx = i,
        ("tree", Some(i)) => demo.tree_sel = i,
        (hit::TREE_TOGGLE, _) => demo.tree_open = !demo.tree_open,
        (hit::SNACKBAR_SHOW, _) => demo.snackbar_visible = true,
        (hit::SNACKBAR_CLOSE, _) => demo.snackbar_visible = false,
        (hit::POPOVER_ANCHOR, _) => demo.popover_open = !demo.popover_open,
        (hit::POPOVER_CLOSE, _) => demo.popover_open = false,
        (hit::PALETTE_OPEN, _) => demo.palette_open = true,
        (hit::PALETTE_CLOSE, _) => demo.palette_open = false,
        ("palette", Some(i)) => {
            demo.palette_open = false;
            demo.palette_query.clear();
            let _ = i;
        }
        // Слайдер обрабатывается drag/клик-координатой (input.rs), не id.
        (hit::SLIDER, _) => {}
        _ => return DemoEffect::Ignored,
    }
    DemoEffect::Redraw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_asis_static() {
        let d = KitDemoState::default();
        // Витрина по умолчанию — прежнее поведение (AS-IS), все всплывшие
        // демо закрыты: regression-гарантия для существующих потребителей.
        assert!(!d.tobe);
        assert!(!d.dropdown_open);
        assert!(!d.modal_open);
        assert!(!d.palette_open);
        assert!(!d.text_focus);
        assert!(d.toast_visible);
    }

    #[test]
    fn sections_registry_groups_complete() {
        // Реестр покрывает группы issue D5; у каждой секции — уникальный id.
        let mut ids: Vec<_> = SECTIONS.iter().map(|s| s.id).collect();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "id секций уникальны");
        for group in [
            DemoGroup::Inputs,
            DemoGroup::Navigation,
            DemoGroup::Containers,
            DemoGroup::DataDisplay,
            DemoGroup::Feedback,
        ] {
            assert!(
                SECTIONS.iter().any(|s| s.group == group),
                "группа {group:?} пуста"
            );
        }
        // 13 компонентов Wave C (issue D2) присутствуют реальными секциями.
        for id in [
            "checkbox",
            "slider",
            "radio",
            "tabs",
            "accordion",
            "progress",
            "skeleton",
            "badge",
            "popover",
            "snackbar",
            "tree",
            "segmented",
            "command_palette",
        ] {
            assert!(section(id).is_some(), "нет секции Wave C {id}");
        }
    }

    #[test]
    fn parse_hit_sections_and_indexes() {
        assert_eq!(parse_hit("switch"), Some(("switch", None)));
        assert_eq!(parse_hit("chips:2"), Some(("chips", Some(2))));
        assert_eq!(
            parse_hit("dropdown:10"),
            Some(("dropdown", Some(10))),
            "числовой суффикс — индекс (границы знает apply_click/таблицы id)"
        );
        assert_eq!(parse_hit(""), None);
        assert_eq!(
            parse_hit("dropdown:anchor"),
            Some(("dropdown:anchor", None)),
            "нешисловой суффикс — часть имени цели"
        );
    }

    #[test]
    fn toolbar_cycles_and_mode() {
        let mut d = KitDemoState::default();
        assert_eq!(apply_click(&mut d, hit::TOOLBAR_MODE), DemoEffect::Redraw);
        assert!(d.tobe);
        // AS-IS закрывает всплывающие демо
        d.modal_open = true;
        d.dropdown_open = true;
        apply_click(&mut d, hit::TOOLBAR_MODE);
        assert!(!d.tobe && !d.modal_open && !d.dropdown_open);

        let themes = canvas_core::theme_presets::PRESETS.len();
        apply_click(&mut d, hit::TOOLBAR_THEME);
        assert_eq!(d.theme_idx, 1 % themes);
        d.theme_idx = themes - 1;
        apply_click(&mut d, hit::TOOLBAR_THEME);
        assert_eq!(d.theme_idx, 0, "цикл тем заворачивается");

        assert_eq!(d.size, ControlSize::Md);
        apply_click(&mut d, hit::TOOLBAR_SIZE);
        assert_eq!(d.size, ControlSize::Lg);
        apply_click(&mut d, hit::TOOLBAR_SIZE);
        assert_eq!(d.size, ControlSize::Xs, "цикл размеров Lg→Xs");

        apply_click(&mut d, hit::TOOLBAR_DENSITY);
        assert_eq!(d.density, Density::Spacious);
        apply_click(&mut d, hit::TOOLBAR_DENSITY);
        assert_eq!(
            d.density,
            Density::Compact,
            "цикл плотности Spacious→Compact"
        );

        let before = d.lang_override;
        apply_click(&mut d, hit::TOOLBAR_LANG);
        assert_ne!(d.lang_override, before, "язык переключился");
    }

    #[test]
    fn d1_existing_sections_transitions() {
        let mut d = KitDemoState::default();
        // Чипы — toggle по индексу
        apply_click(&mut d, "chips:1");
        assert!(d.chips[1]);
        apply_click(&mut d, "chips:1");
        assert!(!d.chips[1]);
        // Dropdown: открыть → выбрать (закрыть + селект)
        apply_click(&mut d, hit::DROPDOWN_ANCHOR);
        assert!(d.dropdown_open);
        apply_click(&mut d, "dropdown:2");
        assert_eq!(d.dropdown_sel, 2);
        assert!(!d.dropdown_open);
        // Switch
        apply_click(&mut d, hit::SWITCH);
        assert!(d.switch_on);
        // List: выбор строки
        apply_click(&mut d, "list:3");
        assert_eq!(d.list_sel, 3);
        // Модалка: открыть/закрыть
        apply_click(&mut d, hit::MODAL_OPEN);
        assert!(d.modal_open);
        apply_click(&mut d, hit::MODAL_CLOSE);
        assert!(!d.modal_open);
        // Тост-демо
        apply_click(&mut d, hit::TOAST_TOGGLE);
        assert!(!d.toast_visible);
    }

    #[test]
    fn d2_wave_c_transitions() {
        let mut d = KitDemoState::default();
        // Чек-бокс — цикл 3 состояний
        apply_click(&mut d, hit::CHECKBOX);
        assert_eq!(d.checkbox, CheckboxState::Checked);
        apply_click(&mut d, hit::CHECKBOX);
        assert_eq!(d.checkbox, CheckboxState::Indeterminate);
        apply_click(&mut d, hit::CHECKBOX);
        assert_eq!(d.checkbox, CheckboxState::Unchecked);
        // Радио/табы/сегменты — выбор по индексу
        apply_click(&mut d, "radio:2");
        assert_eq!(d.radio_idx, 2);
        apply_click(&mut d, "tabs:1");
        assert_eq!(d.tab_idx, 1);
        apply_click(&mut d, "segmented:3");
        assert_eq!(d.segmented_idx, 3);
        // Аккордеон — toggle по индексу
        assert!(d.accordion_open[0]);
        apply_click(&mut d, "accordion:0");
        assert!(!d.accordion_open[0]);
        apply_click(&mut d, "accordion:2");
        assert!(d.accordion_open[2]);
        // Дерево — выбор узла + toggle группы
        apply_click(&mut d, "tree:1");
        assert_eq!(d.tree_sel, 1);
        apply_click(&mut d, "tree_toggle");
        assert!(!d.tree_open);
        // Снекбар/поповер/палитра
        apply_click(&mut d, hit::SNACKBAR_SHOW);
        assert!(d.snackbar_visible);
        apply_click(&mut d, hit::SNACKBAR_CLOSE);
        assert!(!d.snackbar_visible);
        apply_click(&mut d, hit::POPOVER_ANCHOR);
        assert!(d.popover_open);
        apply_click(&mut d, hit::POPOVER_CLOSE);
        assert!(!d.popover_open);
        d.palette_query = "x".into();
        apply_click(&mut d, hit::PALETTE_OPEN);
        assert!(d.palette_open);
        apply_click(&mut d, "palette:1");
        assert!(!d.palette_open);
        assert!(d.palette_query.is_empty(), "палитра сбрасывает запрос");
    }

    #[test]
    fn unknown_click_ignored() {
        let mut d = KitDemoState::default();
        assert_eq!(apply_click(&mut d, "nope"), DemoEffect::Ignored);
        assert_eq!(apply_click(&mut d, "chips:99"), DemoEffect::Ignored);
        assert_eq!(apply_click(&mut d, ""), DemoEffect::Ignored);
    }

    #[test]
    fn hit_id_prefix() {
        assert_eq!(hit_id("switch"), format!("{HIT_PREFIX}switch"));
    }
}
