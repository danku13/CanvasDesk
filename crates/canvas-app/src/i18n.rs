//! FR-040: локализация интерфейса — ключи-фразы + статические таблицы RU/EN
//! (ноль внешних крейтов, образец чистых модулей [`crate::settings_ui`]).
//!
//! Правила (FR-040 §2):
//! - ключ = полная фраза; склейка «лейбл + ": " + значение» запрещена —
//!   значение показывает контрол (FR-039);
//! - подстановки — плейсхолдеры `{name}` в значении, форматирование на
//!   стороне вызова через [`trf`];
//! - отсутствующий перевод — fallback на вторую таблицу, затем сам ключ
//!   (в рантайме не срабатывает: тест полноты гарантирует непустые RU и EN
//!   у каждого ключа обеих таблиц);
//! - названия языков в переключателе — на языке самого языка
//!   ([`canvas_core::Language::native_label`]), не через таблицу.
//!
//! Тексты читаются по кадру — смена языка применяется на лету без
//! пересборки состояния ([`App`], паттерн рендера canvas-app).

use canvas_core::Language;

/// Ключи фраз (константы — опечатка в литерале ловится компилятором).
pub mod keys {
    // --- Модалка настроек (FR-039) ---
    pub const SETTINGS_TITLE: &str = "settings.title";
    pub const SETTINGS_HINT: &str = "settings.hint";
    pub const TAB_GENERAL: &str = "settings.tab.general";
    pub const TAB_CANVAS: &str = "settings.tab.canvas";
    pub const TAB_EDGES: &str = "settings.tab.edges";
    pub const TAB_APPEARANCE: &str = "settings.tab.appearance";
    pub const TAB_SNAP: &str = "settings.tab.snap";

    pub const ROW_SNAP_ENABLED: &str = "settings.row.snap_enabled";
    pub const ROW_SNAP_GRID: &str = "settings.row.snap_grid";
    pub const ROW_SNAP_GUIDES: &str = "settings.row.snap_guides";
    pub const ROW_SNAP_COLLISION: &str = "settings.row.snap_collision";
    pub const ROW_SNAP_TOLERANCE: &str = "settings.row.snap_tolerance";
    pub const ROW_SNAP_SUB_ZOOM: &str = "settings.row.snap_sub_zoom";
    pub const ROW_SNAP_COARSE_ZOOM: &str = "settings.row.snap_coarse_zoom";

    pub const DESC_SNAP_ENABLED: &str = "settings.desc.snap_enabled";
    pub const DESC_SNAP_GRID: &str = "settings.desc.snap_grid";
    pub const DESC_SNAP_GUIDES: &str = "settings.desc.snap_guides";
    pub const DESC_SNAP_COLLISION: &str = "settings.desc.snap_collision";
    pub const DESC_SNAP_TOLERANCE: &str = "settings.desc.snap_tolerance";
    pub const DESC_SNAP_SUB_ZOOM: &str = "settings.desc.snap_sub_zoom";
    pub const DESC_SNAP_COARSE_ZOOM: &str = "settings.desc.snap_coarse_zoom";

    pub const ROW_BUTTON_CORNER: &str = "settings.row.button_corner";
    pub const ROW_GRID: &str = "settings.row.grid";
    pub const ROW_GRID_STYLE: &str = "settings.row.grid_style";
    pub const ROW_GRID_DENSITY: &str = "settings.row.grid_density";
    pub const ROW_EDGES_AVOID: &str = "settings.row.edges_avoid";
    pub const ROW_PORT_ZONE: &str = "settings.row.port_zone";
    pub const ROW_LINE_PORTS: &str = "settings.row.line_ports";
    pub const ROW_BOTTLENECK: &str = "settings.row.bottleneck";
    pub const ROW_FOCUS_MODE: &str = "settings.row.focus_mode";
    pub const ROW_HUD_ON_START: &str = "settings.row.hud_on_start";
    pub const ROW_LANGUAGE: &str = "settings.row.language";
    /// FR-047: тема-пресет (таб «Внешний вид»).
    pub const ROW_THEME_PRESET: &str = "settings.row.theme_preset";

    pub const DESC_BUTTON_CORNER: &str = "settings.desc.button_corner";
    pub const DESC_GRID: &str = "settings.desc.grid";
    pub const DESC_GRID_STYLE: &str = "settings.desc.grid_style";
    pub const DESC_GRID_DENSITY: &str = "settings.desc.grid_density";
    pub const DESC_EDGES_AVOID: &str = "settings.desc.edges_avoid";
    pub const DESC_PORT_ZONE: &str = "settings.desc.port_zone";
    pub const DESC_LINE_PORTS: &str = "settings.desc.line_ports";
    pub const DESC_BOTTLENECK: &str = "settings.desc.bottleneck";
    pub const DESC_FOCUS_MODE: &str = "settings.desc.focus_mode";
    pub const DESC_HUD_ON_START: &str = "settings.desc.hud_on_start";
    pub const DESC_LANGUAGE: &str = "settings.desc.language";
    /// FR-047: описание строки темы-пресета.
    pub const DESC_THEME_PRESET: &str = "settings.desc.theme_preset";
    /// FR-047: значение «нет пресета» (выбор по карточкам тёмной/светлой).
    pub const THEME_PRESET_CLASSIC: &str = "settings.value.theme_preset.classic";

    // --- Main stage / агрегация связей (FR-042) ---
    pub const ROW_EDGE_AGGREGATION: &str = "settings.row.edge_aggregation";
    pub const DESC_EDGE_AGGREGATION: &str = "settings.desc.edge_aggregation";
    pub const STAGE_TITLE: &str = "stage.title";
    pub const STAGE_HINT: &str = "stage.hint";
    pub const STAGE_LINE_LABEL: &str = "stage.line_label";
    pub const STAGE_PARAM_LABEL: &str = "stage.param_label";
    /// FR-044: заголовок main stage с составом пучка (паритет с прототипом).
    pub const STAGE_BUNDLE_TITLE: &str = "stage.bundle_title";
    /// FR-044: нижняя подсказка main stage с жестами закрытия/выделения.
    pub const STAGE_FOOT_HINT: &str = "stage.foot_hint";

    // --- PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры ---
    /// Заголовок окна проверки.
    pub const EXPLAIN_TITLE: &str = "explain.title";
    /// Подзаголовок шапки: цепочка корня «{title}».
    pub const EXPLAIN_META: &str = "explain.meta";
    /// Чип Stale (AC-3.3): модель изменилась при открытой панели.
    pub const EXPLAIN_STALE: &str = "explain.stale";
    /// Тост при закрытии из-за удалённого корня (AC-3.3).
    pub const EXPLAIN_GONE: &str = "explain.gone";
    /// Пометка листа-константы (AC-1.4): «исходное значение».
    pub const EXPLAIN_LEAF_TAG: &str = "explain.leaf_tag";
    /// Статистика футера: «Уровней: {lv} · Узлов: {n}».
    pub const EXPLAIN_STATS: &str = "explain.stats";
    /// Терминальные узлы дерева (§6.6).
    pub const EXPLAIN_UNMAPPED: &str = "explain.unmapped";
    pub const EXPLAIN_CYCLE: &str = "explain.cycle";
    pub const EXPLAIN_UNLINKED: &str = "explain.unlinked";
    pub const EXPLAIN_TRUNCATED: &str = "explain.truncated";
    /// Бейдж фронтира: «+{n} глубже» (AC-2.3 — ручное разворачивание).
    pub const EXPLAIN_EXPAND_BADGE: &str = "explain.expand_badge";
    /// Адресная строка узла (AC-2.1): «вход {n}» / «выход {name}».
    pub const EXPLAIN_ADDR_SLOT: &str = "explain.addr_slot";
    pub const EXPLAIN_ADDR_OUTPUT: &str = "explain.addr_output";
    /// Настройка FR-039 (AC-2.3): лимит глубины авто-раскрытия дерева.
    pub const ROW_EXPLAIN_DEPTH: &str = "settings.row.explain_depth";
    pub const DESC_EXPLAIN_DEPTH: &str = "settings.desc.explain_depth";
    /// Значение настройки «0 — без ограничения».
    pub const VALUE_EXPLAIN_ALL: &str = "settings.value.explain.all";
    /// Честный лоадер (AC-1.2, У5): 12 подписей этапов построения —
    /// ротация, без фейкового прогресс-бара.
    pub const EXPLAIN_LOADER_1: &str = "explain.loader.1";
    pub const EXPLAIN_LOADER_2: &str = "explain.loader.2";
    pub const EXPLAIN_LOADER_3: &str = "explain.loader.3";
    pub const EXPLAIN_LOADER_4: &str = "explain.loader.4";
    pub const EXPLAIN_LOADER_5: &str = "explain.loader.5";
    pub const EXPLAIN_LOADER_6: &str = "explain.loader.6";
    pub const EXPLAIN_LOADER_7: &str = "explain.loader.7";
    pub const EXPLAIN_LOADER_8: &str = "explain.loader.8";
    pub const EXPLAIN_LOADER_9: &str = "explain.loader.9";
    pub const EXPLAIN_LOADER_10: &str = "explain.loader.10";
    pub const EXPLAIN_LOADER_11: &str = "explain.loader.11";
    pub const EXPLAIN_LOADER_12: &str = "explain.loader.12";

    pub const VALUE_ON: &str = "settings.value.on";
    pub const VALUE_OFF: &str = "settings.value.off";
    pub const CORNER_TOP_LEFT: &str = "settings.value.corner.top_left";
    pub const CORNER_TOP_RIGHT: &str = "settings.value.corner.top_right";
    pub const CORNER_BOTTOM_LEFT: &str = "settings.value.corner.bottom_left";
    pub const CORNER_BOTTOM_RIGHT: &str = "settings.value.corner.bottom_right";
    pub const GRID_STYLE_LINES: &str = "settings.value.grid_style.lines";
    pub const GRID_STYLE_DOTS: &str = "settings.value.grid_style.dots";
    pub const GRID_DENSITY_DENSE: &str = "settings.value.grid_density.dense";
    pub const GRID_DENSITY_MEDIUM: &str = "settings.value.grid_density.medium";
    pub const GRID_DENSITY_SPARSE: &str = "settings.value.grid_density.sparse";
    pub const THEME_DARK: &str = "settings.value.theme.dark";
    pub const THEME_LIGHT: &str = "settings.value.theme.light";

    // --- Контекстное меню канваса ---
    pub const MENU_NEW_GROUP: &str = "menu.new_group";
    pub const MENU_FOCUS_MODE: &str = "menu.focus_mode";
    pub const MENU_HOTKEYS: &str = "menu.hotkeys";
    pub const MENU_WIDGETS: &str = "menu.widgets";
    pub const MENU_DESKTOP_MODE: &str = "menu.desktop_mode";
    pub const MENU_BOTTLENECK: &str = "menu.bottleneck";
    pub const MENU_WHATIF: &str = "menu.whatif";

    // --- Batch-операции выделения (FR-038 п.16, T-038.5): видны при N≥3 ---
    pub const MENU_ALIGN_HORIZONTAL: &str = "menu.align_horizontal";
    pub const MENU_ALIGN_VERTICAL: &str = "menu.align_vertical";
    pub const MENU_DISTRIBUTE_EVENLY: &str = "menu.distribute_evenly";

    // --- Панель хоткеев (FR-004): колонка клавиши («ЛКМ» — раскладочная
    // аббревиатура, тоже переводится) и описания ---
    pub const HOTKEYS_TITLE: &str = "hotkeys.title";
    // Клавишная колонка: «ЛКМ»/«ПКМ»/«клик» — русские аббревиатуры,
    // в EN переводятся (LMB/RMB/click); остальные — языконезависимы,
    // но идут через таблицу для единообразия.
    pub const HKEY_F1: &str = "hotkeys.key.f1";
    pub const HKEY_CTRL_F: &str = "hotkeys.key.ctrl_f";
    pub const HKEY_CTRL_P: &str = "hotkeys.key.ctrl_p";
    pub const HKEY_SHIFT_CLICK: &str = "hotkeys.key.shift_click";
    pub const HKEY_F3: &str = "hotkeys.key.f3";
    pub const HKEY_ESC: &str = "hotkeys.key.esc";
    pub const HKEY_DEL: &str = "hotkeys.key.del";
    pub const HKEY_CTRL_Z: &str = "hotkeys.key.ctrl_z";
    pub const HKEY_CTRL_Y: &str = "hotkeys.key.ctrl_y";
    pub const HKEY_CTRL_C: &str = "hotkeys.key.ctrl_c";
    pub const HKEY_CTRL_X: &str = "hotkeys.key.ctrl_x";
    pub const HKEY_CTRL_V: &str = "hotkeys.key.ctrl_v";
    pub const HKEY_CTRL_D: &str = "hotkeys.key.ctrl_d";
    pub const HKEY_CTRL_G: &str = "hotkeys.key.ctrl_g";
    pub const HKEY_CTRL_B: &str = "hotkeys.key.ctrl_b";
    pub const HKEY_CTRL_CLICK: &str = "hotkeys.key.ctrl_click";
    pub const HKEY_LMB_DRAG: &str = "hotkeys.key.lmb_drag";
    pub const HKEY_LMB_PORT: &str = "hotkeys.key.lmb_port";
    pub const HKEY_LMB_HANDLE: &str = "hotkeys.key.lmb_handle";
    pub const HKEY_DOUBLE_CLICK: &str = "hotkeys.key.double_click";
    pub const HKEY_RMB: &str = "hotkeys.key.rmb";
    pub const HKEY_SPACE_DRAG: &str = "hotkeys.key.space_drag";
    pub const HKEY_CTRL_WHEEL: &str = "hotkeys.key.ctrl_wheel";
    pub const HKEY_CTRL_ENTER: &str = "hotkeys.key.ctrl_enter";
    pub const HKEY_CTRL_COMMA: &str = "hotkeys.key.ctrl_comma";
    pub const HKEY_CTRL_SHIFT_I: &str = "hotkeys.key.ctrl_shift_i";
    pub const HKEY_F: &str = "hotkeys.key.f";
    pub const HK_F1: &str = "hotkeys.desc.f1";
    pub const HK_SEARCH: &str = "hotkeys.desc.search";
    pub const HK_PALETTE: &str = "hotkeys.desc.palette";
    pub const HK_WHEEL: &str = "hotkeys.desc.wheel";
    pub const HK_HUD: &str = "hotkeys.desc.hud";
    pub const HK_ESC: &str = "hotkeys.desc.esc";
    pub const HK_DELETE: &str = "hotkeys.desc.delete";
    pub const HK_UNDO: &str = "hotkeys.desc.undo";
    pub const HK_REDO: &str = "hotkeys.desc.redo";
    pub const HK_COPY: &str = "hotkeys.desc.copy";
    pub const HK_CUT: &str = "hotkeys.desc.cut";
    pub const HK_PASTE: &str = "hotkeys.desc.paste";
    pub const HK_DUPLICATE: &str = "hotkeys.desc.duplicate";
    pub const HK_GROUP: &str = "hotkeys.desc.group";
    pub const HK_BOTTLENECK: &str = "hotkeys.desc.bottleneck";
    pub const HK_ADD_TO_SELECTION: &str = "hotkeys.desc.add_to_selection";
    pub const HK_SELECTION_FRAME: &str = "hotkeys.desc.selection_frame";
    pub const HK_DRAG_EDGE: &str = "hotkeys.desc.drag_edge";
    pub const HK_REBIND_EDGE: &str = "hotkeys.desc.rebind_edge";
    pub const HK_DOUBLE_CLICK: &str = "hotkeys.desc.double_click";
    pub const HK_CONTEXT_MENU: &str = "hotkeys.desc.context_menu";
    pub const HK_PAN: &str = "hotkeys.desc.pan";
    pub const HK_ZOOM: &str = "hotkeys.desc.zoom";
    pub const HK_COMMIT_NOTE: &str = "hotkeys.desc.commit_note";
    pub const HK_SETTINGS: &str = "hotkeys.desc.settings";
    pub const HK_WHATIF: &str = "hotkeys.desc.whatif";
    pub const HK_EDGE_FOCUS: &str = "hotkeys.desc.edge_focus";

    // --- Палитра шаблонов (FR-024/030) ---
    pub const TEMPLATES_TITLE: &str = "templates.title";
    pub const TEMPLATES_SEARCH: &str = "templates.search";
    pub const TEMPLATES_FOOTER: &str = "templates.footer";

    // --- Подменю «Виджеты» (M5) ---
    pub const WIDGETS_EMPTY: &str = "widgets.empty";
    pub const WIDGETS_REMOVE_ENTRY: &str = "widgets.remove_entry";

    // --- Модальные диалоги (T21, FR-014; FR-050 Н4/Р-3 — этап C) ---
    pub const DIALOG_YES: &str = "dialog.yes";
    pub const DIALOG_NO: &str = "dialog.no";
    pub const DIALOG_INSTALL_TITLE: &str = "dialog.install.title";
    pub const DIALOG_UPDATE_TITLE: &str = "dialog.update.title";
    pub const DIALOG_REMOVE_TITLE: &str = "dialog.remove.title";
    pub const DIALOG_CYCLE_TITLE: &str = "dialog.cycle.title";
    pub const DIALOG_INSTALL_BODY: &str = "dialog.install.body";
    pub const DIALOG_PERMS_NONE: &str = "dialog.perms_none";
    pub const DIALOG_REMOVE_BODY: &str = "dialog.remove.body";
    pub const DIALOG_CYCLE_BODY: &str = "dialog.cycle.body";
    /// FR-050 Н4 (этап C): диалог «Заменить источник?» — заголовок.
    pub const DIALOG_REPLACE_TITLE: &str = "dialog.replace.title";
    /// FR-050 Н4 (этап C): тело диалога замены (параметр + текущий источник).
    pub const DIALOG_REPLACE_BODY: &str = "dialog.replace.body";
    /// FR-050 Н4 (этап C): кнопка подтверждения замены.
    pub const DIALOG_REPLACE_YES: &str = "dialog.replace.yes";
    /// FR-050 Н4 (этап C): кнопка отмены (возврат к drag без создания).
    pub const DIALOG_CANCEL: &str = "dialog.cancel";
    /// FR-050 Н2 (этап C): меню выбора параметра приёмника (drop мимо якоря).
    pub const MENU_PICK_PARAM_TITLE: &str = "menu.pick_param.title";
    /// FR-050 Н2 (этап C): меню выбора строки-источника (W-AMBIGUOUS-SRC).
    pub const MENU_PICK_LINE_TITLE: &str = "menu.pick_line.title";
    /// FR-050 Р-3 (этап C): тултип unmapped-позиционного входа.
    pub const TOOLTIP_UNMAPPED_SLOT: &str = "tooltip.unmapped.slot";
    /// FR-050 Р-3 (этап C): тултип unmapped-параметра (toParam без значения).
    pub const TOOLTIP_UNMAPPED_PARAM: &str = "tooltip.unmapped.param";

    // --- Подсказки Numi-ввода (FR-021) ---
    pub const HINT_VAR: &str = "hints.var";
    pub const HINT_UNIT: &str = "hints.unit";
    pub const HINT_PARAM: &str = "hints.param";
    pub const HINT_DOLLAR_IN: &str = "hints.dollar_in";
    pub const HINT_DOLLAR_N: &str = "hints.dollar_n";

    // --- Онбординг (FR-028) ---
    pub const ONBOARDING_BACK: &str = "onboarding.back";
    // --- Галерея схем (FR-049) ---
    pub const GALLERY_TITLE: &str = "gallery.title";
    pub const GALLERY_SEARCH: &str = "gallery.search";
    pub const GALLERY_ALL: &str = "gallery.all";
    pub const GALLERY_META: &str = "gallery.meta";
    pub const GALLERY_FOOTER: &str = "gallery.footer";
    pub const GALLERY_TRY: &str = "gallery.try";
    pub const GALLERY_EMPTY_TITLE: &str = "gallery.empty.title";
    pub const GALLERY_EMPTY_BODY: &str = "gallery.empty.body";
    pub const GALLERY_EMPTY_OPEN: &str = "gallery.empty.open";
    pub const GALLERY_EMPTY_DISMISS: &str = "gallery.empty.dismiss";
    pub const GALLERY_APPLIED: &str = "gallery.applied";
    pub const GALLERY_UNKNOWN: &str = "gallery.unknown";
    pub const HELP_SCHEMES: &str = "help.schemes";
    pub const ONBOARDING_NEXT: &str = "onboarding.next";
    pub const ONBOARDING_DONE: &str = "onboarding.done";
    pub const ONBOARDING_SKIP: &str = "onboarding.skip";
    pub const ONBOARDING_STEP1_TITLE: &str = "onboarding.step1.title";
    pub const ONBOARDING_STEP1_BODY: &str = "onboarding.step1.body";
    pub const ONBOARDING_STEP2_TITLE: &str = "onboarding.step2.title";
    pub const ONBOARDING_STEP2_BODY: &str = "onboarding.step2.body";
    pub const ONBOARDING_STEP3_TITLE: &str = "onboarding.step3.title";
    pub const ONBOARDING_STEP3_BODY: &str = "onboarding.step3.body";
    pub const ONBOARDING_STEP4_TITLE: &str = "onboarding.step4.title";
    pub const ONBOARDING_STEP4_BODY: &str = "onboarding.step4.body";
    pub const ONBOARDING_STEP5_TITLE: &str = "onboarding.step5.title";
    pub const ONBOARDING_STEP5_BODY: &str = "onboarding.step5.body";
    pub const ONBOARDING_STEP6_TITLE: &str = "onboarding.step6.title";
    pub const ONBOARDING_STEP6_BODY: &str = "onboarding.step6.body";
    pub const ONBOARDING_STEP7_TITLE: &str = "onboarding.step7.title";
    pub const ONBOARDING_STEP7_BODY: &str = "onboarding.step7.body";
    pub const ONBOARDING_STEP8_TITLE: &str = "onboarding.step8.title";
    pub const ONBOARDING_STEP8_BODY: &str = "onboarding.step8.body";

    // --- Меню «?» и просмотрщик документации (FR-027/031) ---
    pub const HELP_DOCS: &str = "help.docs";
    pub const HELP_ONBOARDING: &str = "help.onboarding";
    pub const DOCS_FOOTER: &str = "docs.footer";
    pub const DOCS_PAGE_INDEX: &str = "docs.page.index";
    pub const DOCS_PAGE_QUICK_START: &str = "docs.page.quick_start";
    pub const DOCS_PAGE_INTERFACE: &str = "docs.page.interface";
    pub const DOCS_PAGE_HOTKEYS: &str = "docs.page.hotkeys";
    pub const DOCS_PAGE_CALCULATIONS: &str = "docs.page.calculations";
    pub const DOCS_PAGE_TEMPLATES: &str = "docs.page.templates";
    pub const DOCS_PAGE_FAQ: &str = "docs.page.faq";
    pub const DOCS_PAGE_AGENT_RECIPE: &str = "docs.page.agent_recipe";

    // --- What-if (FR-017) ---
    pub const WHATIF_PILL: &str = "whatif.pill";
    pub const WHATIF_OVERRIDES: &str = "whatif.overrides";
    pub const WHATIF_NO_OVERRIDES: &str = "whatif.no_overrides";
    pub const WHATIF_APPLY: &str = "whatif.apply";
    pub const WHATIF_RESET: &str = "whatif.reset";
    pub const WHATIF_COMPARE: &str = "whatif.compare";
    pub const WHATIF_BASE: &str = "whatif.base";
    pub const WHATIF_COLUMN_VAR: &str = "whatif.column.var";
    pub const WHATIF_SCENARIO_DEFAULT: &str = "whatif.scenario_default";
    pub const WHATIF_ONLY_CALC_LINES: &str = "whatif.only_calc_lines";

    // --- Палитра выделения (FR-009/010, FR-014, FR-019/020, FR-025, CR-008) ---
    pub const PAL_GROUP_COLOR: &str = "palette.group.color";
    pub const PAL_GROUP_LAYOUT: &str = "palette.group.layout";
    pub const PAL_GROUP_ACTIONS: &str = "palette.group.actions";
    pub const PAL_GROUP_BRANCHING: &str = "palette.group.branching";
    pub const PAL_GROUP_STYLE: &str = "palette.group.style";
    pub const PAL_GROUP_THICKNESS: &str = "palette.group.thickness";
    pub const PAL_GROUP_FLOW: &str = "palette.group.flow";
    pub const PAL_GROUP_PORTS: &str = "palette.group.ports";
    pub const PAL_GROUP_TEMPLATE: &str = "palette.group.template";
    pub const PAL_COLOR_PRESET: &str = "palette.color.preset";
    pub const PAL_COLOR_NONE: &str = "palette.color.none";
    pub const PAL_LAYOUT_TREE_LR: &str = "palette.layout.tree_lr";
    pub const PAL_LAYOUT_TREE_TB: &str = "palette.layout.tree_tb";
    pub const PAL_LAYOUT_RADIAL: &str = "palette.layout.radial";
    pub const PAL_STYLE_SOLID: &str = "palette.style.solid";
    pub const PAL_STYLE_DASHED: &str = "palette.style.dashed";
    pub const PAL_STYLE_DOTTED: &str = "palette.style.dotted";
    pub const PAL_THICKNESS_THIN: &str = "palette.thickness.thin";
    pub const PAL_THICKNESS_MEDIUM: &str = "palette.thickness.medium";
    pub const PAL_THICKNESS_THICK: &str = "palette.thickness.thick";
    pub const PAL_FLOW_VALUE: &str = "palette.flow.value";
    pub const PAL_FLOW_CONTROL: &str = "palette.flow.control";
    pub const PAL_PORTS_AUTO: &str = "palette.ports.auto";
    pub const PAL_PORTS_PIN_FROM: &str = "palette.ports.pin_from";
    pub const PAL_PORTS_PINNED_FROM: &str = "palette.ports.pinned_from";
    pub const PAL_PORTS_PIN_TO: &str = "palette.ports.pin_to";
    pub const PAL_PORTS_PINNED_TO: &str = "palette.ports.pinned_to";
    pub const PAL_ACTION_RENAME: &str = "palette.action.rename";
    pub const PAL_ACTION_DUPLICATE: &str = "palette.action.duplicate";
    pub const PAL_ACTION_GROUP: &str = "palette.action.group";
    pub const PAL_ACTION_UNGROUP: &str = "palette.action.ungroup";
    pub const PAL_ACTION_OPEN_FILE: &str = "palette.action.open_file";
    pub const PAL_ACTION_OPEN_FOLDER: &str = "palette.action.open_folder";
    pub const PAL_ACTION_COPY_PATH: &str = "palette.action.copy_path";
    pub const PAL_ACTION_COPY_LINK: &str = "palette.action.copy_link";
    pub const PAL_ACTION_CLEAR_TEXT: &str = "palette.action.clear_text";
    pub const PAL_ACTION_SAVE_AS_TEMPLATE: &str = "palette.action.save_as_template";
    pub const PAL_ACTION_ADD_CHILD: &str = "palette.action.add_child";
    pub const PAL_ACTION_ADD_SIBLING: &str = "palette.action.add_sibling";
    pub const PAL_ACTION_EXPAND_BRANCH: &str = "palette.action.expand_branch";
    pub const PAL_ACTION_COLLAPSE_BRANCH: &str = "palette.action.collapse_branch";
    pub const PAL_ACTION_WIDGET_RELOAD: &str = "palette.action.widget_reload";
    pub const PAL_ACTION_WIDGET_PERMISSIONS: &str = "palette.action.widget_permissions";
    pub const PAL_TEMPLATE_UPDATE_TO: &str = "palette.template.update_to";

    // --- Тосты (T21-A, FR-016, CR-008 и др.) ---
    pub const TOAST_PORT_PINNED: &str = "toast.port_pinned";
    pub const TOAST_PORT_AUTO: &str = "toast.port_auto";
    pub const TOAST_PORT_FREED: &str = "toast.port_freed";
    pub const TOAST_FLOW_CYCLE: &str = "toast.flow_cycle";
    pub const TOAST_TEMPLATE_SAVED: &str = "toast.template_saved";
    pub const TOAST_TEMPLATE_SAVE_FAILED: &str = "toast.template_save_failed";
    pub const TOAST_TEMPLATE_UPDATED: &str = "toast.template_updated";
    pub const TOAST_CANVAS_NOT_OPEN: &str = "toast.canvas_not_open";
    pub const TOAST_CANVAS_OPENED: &str = "toast.canvas_opened";
    pub const TOAST_APPLY_DONE: &str = "toast.apply_done";
    pub const TOAST_WIDGET_NOT_INSTALLED: &str = "toast.widget_not_installed";
    pub const TOAST_WIDGET_INSTALLED: &str = "toast.widget_installed";
    pub const TOAST_WIDGET_UPDATED: &str = "toast.widget_updated";
    pub const TOAST_WIDGET_SAME_VERSION: &str = "toast.widget_same_version";
    pub const TOAST_INSTALL_FAILED: &str = "toast.install_failed";
    pub const TOAST_PACKAGE_REMOVED: &str = "toast.package_removed";
    pub const TOAST_REMOVE_FAILED: &str = "toast.remove_failed";
    pub const TOAST_NO_RELATED_CARDS: &str = "toast.no_related_cards";
    pub const TOAST_RELATED_ALIGNED: &str = "toast.related_aligned";
    pub const TOAST_NO_LEVEL: &str = "toast.no_level";
    pub const TOAST_BRANCH_COLLAPSED: &str = "toast.branch_collapsed";
    pub const TOAST_BRANCH_EXPANDED: &str = "toast.branch_expanded";
    pub const TOAST_PATH_COPIED: &str = "toast.path_copied";
    pub const TOAST_GROUP_UNGROUPED: &str = "toast.group_ungrouped";
    pub const TOAST_WIDGET_RELOADING: &str = "toast.widget_reloading";
    pub const TOAST_PERMISSIONS: &str = "toast.permissions";
    pub const TOAST_NODE_INSERTED: &str = "toast.node_inserted";
    pub const TOAST_NODE_EXTRACTED: &str = "toast.node_extracted";
    pub const TOAST_ANALYSIS_ON: &str = "toast.analysis_on";
    pub const TOAST_FILE_UNAVAILABLE: &str = "toast.file_unavailable";
    pub const TOAST_WIDGET_OPEN_FAILED: &str = "toast.widget_open_failed";
    pub const GROUP_DEFAULT_LABEL: &str = "group.default_label";
    pub const FILE_NEW_NOTE: &str = "file.new_note";
}

/// Русская таблица (эталон — порядок и полнота проверяются тестом).
const RU: &[(&str, &str)] = &[
    // --- Модалка настроек ---
    (keys::SETTINGS_TITLE, "Настройки"),
    (keys::SETTINGS_HINT, "Ctrl+, — открыть/закрыть"),
    (keys::TAB_GENERAL, "Общие"),
    (keys::TAB_CANVAS, "Канвас"),
    (keys::TAB_EDGES, "Связи и порты"),
    (keys::TAB_APPEARANCE, "Внешний вид"),
    (keys::ROW_BUTTON_CORNER, "Угол кнопки"),
    (
        keys::DESC_BUTTON_CORNER,
        "В каком углу экрана прижата летающая кнопка настроек.",
    ),
    (keys::ROW_GRID, "Сетка"),
    (keys::DESC_GRID, "Рисовать сетку канваса на заднем плане."),
    (keys::ROW_GRID_STYLE, "Вид сетки"),
    (
        keys::DESC_GRID_STYLE,
        "Сплошные линии или точки в узлах сетки.",
    ),
    (keys::ROW_GRID_DENSITY, "Плотность сетки"),
    (
        keys::DESC_GRID_DENSITY,
        "Шаг сетки: расстояние между линиями/точками.",
    ),
    (keys::ROW_EDGES_AVOID, "Связи огибают ноды"),
    (
        keys::DESC_EDGES_AVOID,
        "Связи обходят посторонние ноды полилинией вместо пересечения.",
    ),
    (keys::ROW_PORT_ZONE, "Зона портов"),
    (
        keys::DESC_PORT_ZONE,
        "Радиус захвата порта при протягивании связи (экранные px).",
    ),
    (keys::ROW_LINE_PORTS, "Построчные порты"),
    (
        keys::DESC_LINE_PORTS,
        "Отдельный выходной порт у каждой строки Numi-листа.",
    ),
    (keys::ROW_BOTTLENECK, "Узкие места"),
    (
        keys::DESC_BOTTLENECK,
        "Рамки и бейджи ρ/W по рассчитанному потоку.",
    ),
    (keys::ROW_FOCUS_MODE, "Фокус-режим"),
    (
        keys::DESC_FOCUS_MODE,
        "Hover/выделение ноды подсвечивает связи и соседей, остальное притемняется.",
    ),
    (keys::ROW_HUD_ON_START, "HUD при старте"),
    (
        keys::DESC_HUD_ON_START,
        "Показывать HUD (fps/p95, F3) сразу при запуске.",
    ),
    (keys::ROW_LANGUAGE, "Язык"),
    (
        keys::DESC_LANGUAGE,
        "Язык интерфейса — применяется на лету, без перезапуска.",
    ),
    (keys::ROW_THEME_PRESET, "Тема-пресет"),
    (
        keys::DESC_THEME_PRESET,
        "Встроенная палитра (Nord, Dracula, Catppuccin и другие) — перекрывает карточки тёмной/светлой выше.",
    ),
    (keys::THEME_PRESET_CLASSIC, "Классическая"),
    (keys::ROW_SNAP_ENABLED, "Snap-выравнивание (мастер)"),
    (
        keys::DESC_SNAP_ENABLED,
        "Мастер-выключатель магнитной раскладки: гасит весь снаппинг, не сбрасывая остальные настройки.",
    ),
    (keys::ROW_SNAP_GRID, "Привязка к сетке"),
    (
        keys::DESC_SNAP_GRID,
        "Притягивать ноду к линиям фоновой сетки в момент отпускания drag.",
    ),
    (keys::ROW_SNAP_GUIDES, "Направляющие соседей"),
    (
        keys::DESC_SNAP_GUIDES,
        "Умные направляющие по краям, центрам и серединам соседних нод, включая равные интервалы.",
    ),
    (keys::ROW_SNAP_COLLISION, "Не проходить сквозь ноды"),
    (
        keys::DESC_SNAP_COLLISION,
        "При перетаскивании движение останавливается на границе зазора вокруг чужих нод.",
    ),
    (keys::ROW_SNAP_TOLERANCE, "Допуск направляющих"),
    (
        keys::DESC_SNAP_TOLERANCE,
        "Радиус притяжения направляющих и сетки в экранных пикселях.",
    ),
    (keys::ROW_SNAP_SUB_ZOOM, "Порог sub-сетки"),
    (
        keys::DESC_SNAP_SUB_ZOOM,
        "При зуме выше порога появляются линии полушага для точной раскладки.",
    ),
    (keys::ROW_SNAP_COARSE_ZOOM, "Порог coarse-сетки"),
    (
        keys::DESC_SNAP_COARSE_ZOOM,
        "При зуме ниже порога линии укрупняются до major-шага для крупной компоновки.",
    ),
    (keys::VALUE_ON, "вкл"),
    (keys::VALUE_OFF, "выкл"),
    (keys::CORNER_TOP_LEFT, "верхний левый"),
    (keys::CORNER_TOP_RIGHT, "верхний правый"),
    (keys::CORNER_BOTTOM_LEFT, "нижний левый"),
    (keys::CORNER_BOTTOM_RIGHT, "нижний правый"),
    (keys::GRID_STYLE_LINES, "линии"),
    (keys::GRID_STYLE_DOTS, "точки"),
    (keys::GRID_DENSITY_DENSE, "частая"),
    (keys::GRID_DENSITY_MEDIUM, "средняя"),
    (keys::GRID_DENSITY_SPARSE, "редкая"),
    (keys::THEME_DARK, "тёмная"),
    (keys::THEME_LIGHT, "светлая"),
    // --- Контекстное меню ---
    (keys::MENU_NEW_GROUP, "Создать группу"),
    (keys::MENU_FOCUS_MODE, "Фокус на связях"),
    (keys::MENU_HOTKEYS, "Горячие клавиши (F1)"),
    (keys::MENU_WIDGETS, "Виджеты ▸…"),
    (keys::MENU_DESKTOP_MODE, "Режим десктопа"),
    (keys::MENU_BOTTLENECK, "Узкие места (Ctrl+B)"),
    (keys::MENU_WHATIF, "What-if режим (Ctrl+Shift+I)"),
    // --- Batch-операции выделения (FR-038 п.16) ---
    (keys::MENU_ALIGN_HORIZONTAL, "Выровнять по горизонтали"),
    (keys::MENU_ALIGN_VERTICAL, "Выровнять по вертикали"),
    (keys::MENU_DISTRIBUTE_EVENLY, "Распределить равномерно"),
    // --- Панель хоткеев ---
    (keys::HOTKEYS_TITLE, "Горячие клавиши"),
    (keys::HKEY_F1, "F1"),
    (keys::HKEY_CTRL_F, "Ctrl+F"),
    (keys::HKEY_CTRL_P, "Ctrl+P"),
    (keys::HKEY_SHIFT_CLICK, "Shift+клик (пусто)"),
    (keys::HKEY_F3, "F3"),
    (keys::HKEY_ESC, "Esc"),
    (keys::HKEY_DEL, "Del"),
    (keys::HKEY_CTRL_Z, "Ctrl+Z"),
    (keys::HKEY_CTRL_Y, "Ctrl+Y"),
    (keys::HKEY_CTRL_C, "Ctrl+C"),
    (keys::HKEY_CTRL_X, "Ctrl+X"),
    (keys::HKEY_CTRL_V, "Ctrl+V"),
    (keys::HKEY_CTRL_D, "Ctrl+D"),
    (keys::HKEY_CTRL_G, "Ctrl+G"),
    (keys::HKEY_CTRL_B, "Ctrl+B"),
    (keys::HKEY_CTRL_CLICK, "Ctrl+клик"),
    (keys::HKEY_LMB_DRAG, "ЛКМ + drag"),
    (keys::HKEY_LMB_PORT, "ЛКМ от порта"),
    (keys::HKEY_LMB_HANDLE, "ЛКМ за хэндл"),
    (keys::HKEY_DOUBLE_CLICK, "2× клик"),
    (keys::HKEY_RMB, "ПКМ"),
    (keys::HKEY_SPACE_DRAG, "Space+drag"),
    (keys::HKEY_CTRL_WHEEL, "Ctrl+колесо"),
    (keys::HKEY_CTRL_ENTER, "Ctrl+Enter"),
    (keys::HKEY_CTRL_COMMA, "Ctrl+,"),
    (keys::HKEY_CTRL_SHIFT_I, "Ctrl+Shift+I"),
    (keys::HKEY_F, "F"),
    (keys::HK_F1, "список горячих клавиш"),
    (keys::HK_SEARCH, "поиск по канвасу"),
    (keys::HK_PALETTE, "палитра шаблонов"),
    (keys::HK_WHEEL, "wheel-меню шаблонов"),
    (keys::HK_HUD, "HUD / следующий результат"),
    (keys::HK_ESC, "закрыть меню и панели"),
    (keys::HK_DELETE, "удалить выделенное"),
    (keys::HK_UNDO, "отменить действие"),
    (keys::HK_REDO, "вернуть отменённое"),
    (keys::HK_COPY, "копировать ноды"),
    (keys::HK_CUT, "вырезать ноды"),
    (keys::HK_PASTE, "вставить ноды"),
    (keys::HK_DUPLICATE, "дублировать ноды"),
    (keys::HK_GROUP, "сгруппировать выделенное"),
    (keys::HK_BOTTLENECK, "индикаторы узких мест"),
    (keys::HK_ADD_TO_SELECTION, "добавить к выделению"),
    (keys::HK_SELECTION_FRAME, "рамка выделения"),
    (keys::HK_DRAG_EDGE, "протянуть связь"),
    (keys::HK_REBIND_EDGE, "перепривязать связь"),
    (keys::HK_DOUBLE_CLICK, "заметка / открыть файл"),
    (keys::HK_CONTEXT_MENU, "меню объекта"),
    (keys::HK_PAN, "панорамирование"),
    (keys::HK_ZOOM, "масштаб"),
    (keys::HK_COMMIT_NOTE, "зафиксировать заметку"),
    (keys::HK_SETTINGS, "настройки"),
    (keys::HK_WHATIF, "what-if сценарии"),
    (keys::HK_EDGE_FOCUS, "фокус на связях"),
    // --- Палитра шаблонов ---
    (keys::TEMPLATES_TITLE, "Шаблоны"),
    (keys::TEMPLATES_SEARCH, "Поиск шаблонов…"),
    (keys::TEMPLATES_FOOTER, "Enter — вставить в центр · Esc — свернуть"),
    // --- Подменю «Виджеты» ---
    (keys::WIDGETS_EMPTY, "(нет установленных)"),
    (keys::WIDGETS_REMOVE_ENTRY, "— Удалить: {name}"),
    // --- Диалоги ---
    (keys::DIALOG_YES, "Да"),
    (keys::DIALOG_NO, "Нет"),
    (keys::DIALOG_INSTALL_TITLE, "Установить виджет {name} {version}?"),
    (keys::DIALOG_UPDATE_TITLE, "Обновить виджет {name} до {version}?"),
    (keys::DIALOG_REMOVE_TITLE, "Удалить пакет {name}?"),
    (keys::DIALOG_CYCLE_TITLE, "Обнаружен цикл: {chain}"),
    (
        keys::DIALOG_INSTALL_BODY,
        "Пакет скопируется в локальную папку виджетов.\nРазрешения: {perms}.",
    ),
    (keys::DIALOG_PERMS_NONE, "без разрешений"),
    (
        keys::DIALOG_REMOVE_BODY,
        "Ноды этого виджета останутся на канвасе как заглушки.\nПакет можно поставить снова перетаскиванием папки.",
    ),
    (
        keys::DIALOG_CYCLE_BODY,
        "Ребро замкнуло бы цикл потока значений (граф обязан быть DAG).\nСоздать как контрольную связь — без передачи значения?",
    ),
    // --- FR-050 (этап C): диалог замены, меню выбора, тултипы Р-3 ---
    (keys::DIALOG_REPLACE_TITLE, "Заменить источник?"),
    (
        keys::DIALOG_REPLACE_BODY,
        "Параметр {param} уже питается от {source}.\nЗаменить источник новым ребром?",
    ),
    (keys::DIALOG_REPLACE_YES, "Заменить"),
    (keys::DIALOG_CANCEL, "Отмена"),
    (keys::MENU_PICK_PARAM_TITLE, "Подключить значение к параметру"),
    (keys::MENU_PICK_LINE_TITLE, "Какая строка — источник значения?"),
    (
        keys::TOOLTIP_UNMAPPED_SLOT,
        "Значение не подставлено: связь есть, но источник не отдал значение (строка-источник удалена / стала прозой / колонка отсутствует). Подключите ноду с актуальным значением или исправьте строку-источник.",
    ),
    (
        keys::TOOLTIP_UNMAPPED_PARAM,
        "Параметр {param} не получает значение: выход {output} отсутствует у ноды \"{node}\" или не отдал значение. Перепривяжите связь на существующий выход (клик по ребру → перепривязка) или подключите ноду с актуальным значением.",
    ),
    // --- Подсказки Numi ---
    (keys::HINT_VAR, "переменная листа"),
    (keys::HINT_UNIT, "единица измерения"),
    (keys::HINT_PARAM, "параметр шаблона"),
    (keys::HINT_DOLLAR_IN, "вход value-рёбер (FR-014)"),
    (keys::HINT_DOLLAR_N, "вход №{i}"),
    // --- Онбординг ---
    // --- Галерея схем (FR-049) ---
    (keys::GALLERY_TITLE, "Шаблоны схем"),
    (keys::GALLERY_SEARCH, "Поиск схем"),
    (keys::GALLERY_ALL, "Все"),
    (keys::GALLERY_META, "ноды: {nodes}, связи: {edges}"),
    (keys::GALLERY_FOOTER, "Enter — открыть · Esc — закрыть"),
    (keys::GALLERY_TRY, "Попробовать"),
    (keys::GALLERY_EMPTY_TITLE, "Начните с шаблона"),
    (
        keys::GALLERY_EMPTY_BODY,
        "Готовые схемы со связями и расчётами: откройте одну и поменяйте числа — всё пересчитается на живую.",
    ),
    (keys::GALLERY_EMPTY_OPEN, "Открыть галерею"),
    (keys::GALLERY_EMPTY_DISMISS, "Пустой холст"),
    (
        keys::GALLERY_APPLIED,
        "Схема «{name}» добавлена — одно Ctrl+Z отменяет",
    ),
    (
        keys::GALLERY_UNKNOWN,
        "Схема не найдена: {id} — проверьте параметр ?template",
    ),
    (keys::HELP_SCHEMES, "Галерея схем"),
    (keys::ONBOARDING_BACK, "Назад"),
    (keys::ONBOARDING_NEXT, "Далее"),
    (keys::ONBOARDING_DONE, "Готово"),
    (keys::ONBOARDING_SKIP, "Пропустить"),
    (
        keys::ONBOARDING_STEP1_TITLE,
        "Добро пожаловать в CanvasDesk",
    ),
    (
        keys::ONBOARDING_STEP1_BODY,
        "Это бесконечный зумируемый канвас для вашего рабочего стола: файловые карточки, заметки, связи и расчёты. Панорамируйте мышью со Space, средней кнопкой или тачпадом; зум — Ctrl+колесо. Всё сохраняется в файл .canvas рядом с приложением.",
    ),
    (keys::ONBOARDING_STEP2_TITLE, "Заметки"),
    (
        keys::ONBOARDING_STEP2_BODY,
        "Двойной клик по пустому месту создаёт заметку с markdown-разметкой (списки, заголовки, ссылки). Цвет — через контекстное меню выделения. Ctrl+Enter фиксирует текст, Esc — откат правки.",
    ),
    (keys::ONBOARDING_STEP3_TITLE, "Связи"),
    (
        keys::ONBOARDING_STEP3_BODY,
        "Наведите курсор на ноду — по краям появятся порты. Протяните связь от порта к другой ноде; конец существующей связи можно перепривязать перетаскиванием за хэндл.",
    ),
    (keys::ONBOARDING_STEP4_TITLE, "Группы и отмена"),
    (
        keys::ONBOARDING_STEP4_BODY,
        "Обведите несколько нод рамкой и нажмите Ctrl+G — получится группа, которую можно двигать целиком. Ошибки отменяются: Ctrl+Z — отменить, Ctrl+Y — вернуть.",
    ),
    (keys::ONBOARDING_STEP5_TITLE, "Формулы Numi"),
    (
        keys::ONBOARDING_STEP5_BODY,
        "В заметках считайте прямо в тексте: «ширина = 120 mm * 4» или «частота = 60 rps». Единицы измерения (mm, ms, MB/s) и подсказки по ходу ввода поддерживаются; результат виден под строкой.",
    ),
    (keys::ONBOARDING_STEP6_TITLE, "Поток значений"),
    (
        keys::ONBOARDING_STEP6_BODY,
        "Протяните связь с зажатым Shift — это value-связь: значение вышестоящей формулы приходит во вход $in нижестоящей и пересчитывается на живую.",
    ),
    (keys::ONBOARDING_STEP7_TITLE, "Шаблоны нод"),
    (
        keys::ONBOARDING_STEP7_BODY,
        "Ctrl+P открывает палитру готовых архитектурных ролей (сервис, очередь, база данных) с параметрами и доменными типами. Shift+клик по пустому месту — радиальное wheel-меню.",
    ),
    (keys::ONBOARDING_STEP8_TITLE, "Что дальше"),
    (
        keys::ONBOARDING_STEP8_BODY,
        "Кнопка «?» рядом с настройками — документация и повтор этого тура в любой момент. F1 — список горячих клавиш прямо в приложении.",
    ),
    // --- Меню «?» и документация ---
    (keys::HELP_DOCS, "Документация ▸"),
    (keys::HELP_ONBOARDING, "Пройти онбординг"),
    (
        keys::DOCS_FOOTER,
        "Колесо — прокрутка · ссылки — переход · Esc — закрыть",
    ),
    (keys::DOCS_PAGE_INDEX, "Главная"),
    (keys::DOCS_PAGE_QUICK_START, "Быстрый старт"),
    (keys::DOCS_PAGE_INTERFACE, "Объекты интерфейса"),
    (keys::DOCS_PAGE_HOTKEYS, "Горячие клавиши"),
    (keys::DOCS_PAGE_CALCULATIONS, "Расчёты и поток значений"),
    (keys::DOCS_PAGE_TEMPLATES, "Шаблоны нод"),
    (keys::DOCS_PAGE_FAQ, "FAQ"),
    (keys::DOCS_PAGE_AGENT_RECIPE, "Рецепт для ИИ-агентов"),
    // --- What-if ---
    (keys::WHATIF_PILL, "What-if сценарии"),
    (keys::WHATIF_OVERRIDES, "подмен: {count}"),
    (
        keys::WHATIF_NO_OVERRIDES,
        "подмен нет — двойной клик по строке расчёта вводит подмену",
    ),
    (keys::WHATIF_APPLY, "Apply"),
    (keys::WHATIF_RESET, "Сброс"),
    (keys::WHATIF_COMPARE, "Сравнить"),
    (keys::WHATIF_BASE, "База"),
    (keys::WHATIF_COLUMN_VAR, "переменная"),
    (keys::WHATIF_SCENARIO_DEFAULT, "Сценарий {n}"),
    (keys::WHATIF_ONLY_CALC_LINES, "в what-if подменяются только строки расчёта"),
    // --- Палитра выделения ---
    (keys::PAL_GROUP_COLOR, "Цвет"),
    (keys::PAL_GROUP_LAYOUT, "Раскладка"),
    (keys::PAL_GROUP_ACTIONS, "Действия"),
    (keys::PAL_GROUP_BRANCHING, "Ветвление"),
    (keys::PAL_GROUP_STYLE, "Стиль"),
    (keys::PAL_GROUP_THICKNESS, "Толщина"),
    (keys::PAL_GROUP_FLOW, "Поток"),
    (keys::PAL_GROUP_PORTS, "Порты"),
    (keys::PAL_GROUP_TEMPLATE, "Шаблон"),
    (keys::PAL_COLOR_PRESET, "Цвет {preset}"),
    (keys::PAL_COLOR_NONE, "Без цвета"),
    (keys::PAL_LAYOUT_TREE_LR, "Дерево →"),
    (keys::PAL_LAYOUT_TREE_TB, "Дерево ↓"),
    (keys::PAL_LAYOUT_RADIAL, "Радиально"),
    (keys::PAL_STYLE_SOLID, "Сплошная"),
    (keys::PAL_STYLE_DASHED, "Пунктир"),
    (keys::PAL_STYLE_DOTTED, "Точки"),
    (keys::PAL_THICKNESS_THIN, "Тонкая"),
    (keys::PAL_THICKNESS_MEDIUM, "Обычная"),
    (keys::PAL_THICKNESS_THICK, "Толстая"),
    (keys::PAL_FLOW_VALUE, "Значение"),
    (keys::PAL_FLOW_CONTROL, "Контрольная"),
    (keys::PAL_PORTS_AUTO, "Авто (кратчайший путь)"),
    (keys::PAL_PORTS_PIN_FROM, "Исток: закрепить"),
    (keys::PAL_PORTS_PINNED_FROM, "Исток: закреплён"),
    (keys::PAL_PORTS_PIN_TO, "Сток: закрепить"),
    (keys::PAL_PORTS_PINNED_TO, "Сток: закреплён"),
    (keys::PAL_ACTION_RENAME, "Переименовать"),
    (keys::PAL_ACTION_DUPLICATE, "Дублировать"),
    (keys::PAL_ACTION_GROUP, "Сгруппировать"),
    (keys::PAL_ACTION_UNGROUP, "Разгруппировать"),
    (keys::PAL_ACTION_OPEN_FILE, "Открыть файл"),
    (keys::PAL_ACTION_OPEN_FOLDER, "Открыть папку с файлом"),
    (keys::PAL_ACTION_COPY_PATH, "Скопировать путь"),
    (keys::PAL_ACTION_COPY_LINK, "Скопировать ссылку"),
    (keys::PAL_ACTION_CLEAR_TEXT, "Очистить текст"),
    (keys::PAL_ACTION_SAVE_AS_TEMPLATE, "Сохранить как шаблон"),
    (keys::PAL_ACTION_ADD_CHILD, "Добавить дочернюю"),
    (keys::PAL_ACTION_ADD_SIBLING, "Добавить сиблинга"),
    (keys::PAL_ACTION_EXPAND_BRANCH, "Развернуть ветку"),
    (keys::PAL_ACTION_COLLAPSE_BRANCH, "Свернуть ветку"),
    (keys::PAL_ACTION_WIDGET_RELOAD, "Перезагрузить виджет"),
    (keys::PAL_ACTION_WIDGET_PERMISSIONS, "Разрешения виджета…"),
    (keys::PAL_TEMPLATE_UPDATE_TO, "Обновить до v{version} (была v{old})"),
    // --- Тосты ---
    (keys::TOAST_PORT_PINNED, "Порт связи закреплён"),
    (keys::TOAST_PORT_AUTO, "Порты связи: авто (кратчайший путь)"),
    (keys::TOAST_PORT_FREED, "Порт освобождён: кратчайший путь"),
    (keys::TOAST_FLOW_CYCLE, "Цикл потока: {participants} — тогл отклонён"),
    (keys::TOAST_TEMPLATE_SAVED, "Шаблон «{name}» сохранён: {path}"),
    (keys::TOAST_TEMPLATE_SAVE_FAILED, "Не удалось сохранить шаблон: {err}"),
    (keys::TOAST_TEMPLATE_UPDATED, "Шаблон обновлён: {name} → v{version}"),
    (keys::TOAST_CANVAS_NOT_OPEN, "Канвас не открыт: {err}"),
    (keys::TOAST_CANVAS_OPENED, "Открыт канвас: {name}"),
    (keys::TOAST_APPLY_DONE, "Apply: {count} подмен записано в модель"),
    (keys::TOAST_WIDGET_NOT_INSTALLED, "Виджет не установлен: {err}"),
    (keys::TOAST_WIDGET_INSTALLED, "Виджет {name} установлен"),
    (keys::TOAST_WIDGET_UPDATED, "Виджет {name} обновлён до {version}"),
    (keys::TOAST_WIDGET_SAME_VERSION, "Виджет {name} уже в этой версии"),
    (keys::TOAST_INSTALL_FAILED, "Установка не удалась: {err}"),
    (keys::TOAST_PACKAGE_REMOVED, "Пакет {name} удалён"),
    (keys::TOAST_REMOVE_FAILED, "Удаление не удалось: {err}"),
    (keys::TOAST_NO_RELATED_CARDS, "Нет связанных карточек для раскладки"),
    (keys::TOAST_RELATED_ALIGNED, "Связанные карточки выровнены"),
    (
        keys::TOAST_NO_LEVEL,
        "У корневой ветки нет уровня — используйте Tab",
    ),
    (keys::TOAST_BRANCH_COLLAPSED, "Ветка свёрнута"),
    (keys::TOAST_BRANCH_EXPANDED, "Ветка развёрнута"),
    (keys::TOAST_PATH_COPIED, "Путь скопирован"),
    (keys::TOAST_GROUP_UNGROUPED, "Группа разгруппирована"),
    (keys::TOAST_WIDGET_RELOADING, "Виджет перезагружается"),
    (keys::TOAST_PERMISSIONS, "Разрешения: {summary}"),
    (keys::TOAST_NODE_INSERTED, "Нода вставлена в группу"),
    (keys::TOAST_NODE_EXTRACTED, "Нода вынесена из группы"),
    (
        keys::TOAST_ANALYSIS_ON,
        "Включён режим анализа (Ctrl+B — выключить)",
    ),
    (keys::TOAST_FILE_UNAVAILABLE, "Файл недоступен: {file}"),
    (keys::TOAST_WIDGET_OPEN_FAILED, "Виджет: не удалось открыть {path}"),
    (keys::GROUP_DEFAULT_LABEL, "Группа"),
    (keys::FILE_NEW_NOTE, "Новая заметка"),
    (keys::ROW_EDGE_AGGREGATION, "Агрегация связей"),
    (
        keys::DESC_EDGE_AGGREGATION,
        "Пучок рёбер одной пары — одна линия с бейджем ×N; клик открывает main stage.",
    ),
    (keys::STAGE_TITLE, "Связи"),
    (keys::STAGE_HINT, "Esc — закрыть"),
    (keys::STAGE_LINE_LABEL, "строка {n}"),
    (keys::STAGE_PARAM_LABEL, "→ {param}"),
    // FR-044 (паритет с прототипом): заголовок stage с составом пучка
    // и нижняя подсказка с жестами закрытия/выделения
    (
        keys::STAGE_BUNDLE_TITLE,
        "Пучок: {from} → {to} · ×{n}",
    ),
    (
        keys::STAGE_FOOT_HINT,
        "Esc или клик по затемнённому фону — закрыть · клик по связи — выделить",
    ),
    // --- PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры ---
    (
        keys::ROW_EXPLAIN_DEPTH,
        "Глубина цепочки расчёта",
    ),
    (
        keys::DESC_EXPLAIN_DEPTH,
        "Сколько уровней дерева объяснения раскрывается автоматически (0 — всё).",
    ),
    (keys::VALUE_EXPLAIN_ALL, "Без ограничения"),
    (keys::EXPLAIN_TITLE, "Проверка цепочки расчёта"),
    (
        keys::EXPLAIN_META,
        "Цепочка расчёта цифры «{title}» · Esc — закрыть",
    ),
    (
        keys::EXPLAIN_STALE,
        "Данные изменены — обновить оверлей",
    ),
    (keys::EXPLAIN_GONE, "Цифра удалена — панель закрыта"),
    (keys::EXPLAIN_LEAF_TAG, "исходное значение"),
    (keys::EXPLAIN_STATS, "Уровней: {lv} · Узлов: {n}"),
    (
        keys::EXPLAIN_UNMAPPED,
        "значение не подставлено",
    ),
    (keys::EXPLAIN_CYCLE, "цикл"),
    (keys::EXPLAIN_UNLINKED, "не связано"),
    (keys::EXPLAIN_TRUNCATED, "дерево усечено"),
    (keys::EXPLAIN_EXPAND_BADGE, "+{n} глубже"),
    (keys::EXPLAIN_ADDR_SLOT, "вход {n}"),
    (keys::EXPLAIN_ADDR_OUTPUT, "выход {name}"),
    // Честный лоадер (AC-1.2, У5): 12 подписей этапов построения.
    (keys::EXPLAIN_LOADER_1, "Начали собирать дерево…"),
    (keys::EXPLAIN_LOADER_2, "Ищем корень цепочки…"),
    (keys::EXPLAIN_LOADER_3, "Разворачиваем формулы в узлы…"),
    (keys::EXPLAIN_LOADER_4, "Проверяем все ветки…"),
    (keys::EXPLAIN_LOADER_5, "Спускаемся к листьям-константам…"),
    (keys::EXPLAIN_LOADER_6, "Ищем неподставленные входы…"),
    (keys::EXPLAIN_LOADER_7, "Проверяем переменные листов…"),
    (keys::EXPLAIN_LOADER_8, "Сверяем проливания параметров…"),
    (keys::EXPLAIN_LOADER_9, "Ищем циклы в данных…"),
    (keys::EXPLAIN_LOADER_10, "Считаем значения на узлах…"),
    (keys::EXPLAIN_LOADER_11, "Готовим подсветку на канвасе…"),
    (keys::EXPLAIN_LOADER_12, "Почти готово — раскладываем ветки…"),
];

/// Английская таблица — полный перевод каждого ключа (инвариант полноты).
const EN: &[(&str, &str)] = &[
    // --- Settings modal ---
    (keys::SETTINGS_TITLE, "Settings"),
    (keys::SETTINGS_HINT, "Ctrl+, — open/close"),
    (keys::TAB_GENERAL, "General"),
    (keys::TAB_CANVAS, "Canvas"),
    (keys::TAB_EDGES, "Edges & ports"),
    (keys::TAB_APPEARANCE, "Appearance"),
    (keys::ROW_BUTTON_CORNER, "Button corner"),
    (
        keys::DESC_BUTTON_CORNER,
        "Which screen corner the floating settings button sits in.",
    ),
    (keys::ROW_GRID, "Grid"),
    (keys::DESC_GRID, "Draw the canvas grid in the background."),
    (keys::ROW_GRID_STYLE, "Grid style"),
    (keys::DESC_GRID_STYLE, "Solid lines or dots at grid intersections."),
    (keys::ROW_GRID_DENSITY, "Grid density"),
    (keys::DESC_GRID_DENSITY, "Grid step: distance between lines/dots."),
    (keys::ROW_EDGES_AVOID, "Edges avoid nodes"),
    (
        keys::DESC_EDGES_AVOID,
        "Edges route around unrelated nodes with polylines instead of crossing them.",
    ),
    (keys::ROW_PORT_ZONE, "Port zone"),
    (
        keys::DESC_PORT_ZONE,
        "Port grab radius when dragging an edge (screen px).",
    ),
    (keys::ROW_LINE_PORTS, "Per-line ports"),
    (
        keys::DESC_LINE_PORTS,
        "A separate output port for each line of a Numi sheet.",
    ),
    (keys::ROW_BOTTLENECK, "Bottlenecks"),
    (
        keys::DESC_BOTTLENECK,
        "Frames and ρ/W badges based on the computed flow.",
    ),
    (keys::ROW_FOCUS_MODE, "Focus mode"),
    (
        keys::DESC_FOCUS_MODE,
        "Hovering or selecting a node highlights its edges and neighbours, the rest dims.",
    ),
    (keys::ROW_HUD_ON_START, "HUD at startup"),
    (
        keys::DESC_HUD_ON_START,
        "Show the HUD (fps/p95, F3) right at startup.",
    ),
    (keys::ROW_LANGUAGE, "Language"),
    (
        keys::DESC_LANGUAGE,
        "Interface language — applied instantly, no restart needed.",
    ),
    (keys::ROW_THEME_PRESET, "Theme preset"),
    (
        keys::DESC_THEME_PRESET,
        "Built-in palette (Nord, Dracula, Catppuccin and more) — overrides the dark/light cards above.",
    ),
    (keys::THEME_PRESET_CLASSIC, "Classic"),
    (keys::ROW_SNAP_ENABLED, "Snap alignment (master)"),
    (
        keys::DESC_SNAP_ENABLED,
        "Master switch of magnetic layout: disables all snapping without resetting other settings.",
    ),
    (keys::ROW_SNAP_GRID, "Snap to grid"),
    (
        keys::DESC_SNAP_GRID,
        "Snap the node to background grid lines on drag release.",
    ),
    (keys::ROW_SNAP_GUIDES, "Neighbor guides"),
    (
        keys::DESC_SNAP_GUIDES,
        "Smart guides by edges, centers and midpoints of neighbor nodes, including equal spacing.",
    ),
    (keys::ROW_SNAP_COLLISION, "Collision avoidance"),
    (
        keys::DESC_SNAP_COLLISION,
        "While dragging, movement stops at the gap boundary around other nodes.",
    ),
    (keys::ROW_SNAP_TOLERANCE, "Guide tolerance"),
    (
        keys::DESC_SNAP_TOLERANCE,
        "Attraction radius of guides and grid, in screen pixels.",
    ),
    (keys::ROW_SNAP_SUB_ZOOM, "Sub-grid threshold"),
    (
        keys::DESC_SNAP_SUB_ZOOM,
        "Above this zoom, half-step lines appear for precise layout.",
    ),
    (keys::ROW_SNAP_COARSE_ZOOM, "Coarse-grid threshold"),
    (
        keys::DESC_SNAP_COARSE_ZOOM,
        "Below this zoom, lines coarsen to the major step for large-scale composition.",
    ),
    (keys::VALUE_ON, "on"),
    (keys::VALUE_OFF, "off"),
    (keys::CORNER_TOP_LEFT, "top left"),
    (keys::CORNER_TOP_RIGHT, "top right"),
    (keys::CORNER_BOTTOM_LEFT, "bottom left"),
    (keys::CORNER_BOTTOM_RIGHT, "bottom right"),
    (keys::GRID_STYLE_LINES, "lines"),
    (keys::GRID_STYLE_DOTS, "dots"),
    (keys::GRID_DENSITY_DENSE, "dense"),
    (keys::GRID_DENSITY_MEDIUM, "medium"),
    (keys::GRID_DENSITY_SPARSE, "sparse"),
    (keys::THEME_DARK, "Dark"),
    (keys::THEME_LIGHT, "Light"),
    // --- Context menu ---
    (keys::MENU_NEW_GROUP, "New group"),
    (keys::MENU_FOCUS_MODE, "Focus on edges"),
    (keys::MENU_HOTKEYS, "Hotkeys (F1)"),
    (keys::MENU_WIDGETS, "Widgets ▸…"),
    (keys::MENU_DESKTOP_MODE, "Desktop mode"),
    (keys::MENU_BOTTLENECK, "Bottlenecks (Ctrl+B)"),
    (keys::MENU_WHATIF, "What-if mode (Ctrl+Shift+I)"),
    // --- Batch-операции выделения (FR-038 п.16) ---
    (keys::MENU_ALIGN_HORIZONTAL, "Align horizontally"),
    (keys::MENU_ALIGN_VERTICAL, "Align vertically"),
    (keys::MENU_DISTRIBUTE_EVENLY, "Distribute evenly"),
    // --- Hotkeys panel ---
    (keys::HOTKEYS_TITLE, "Hotkeys"),
    (keys::HKEY_F1, "F1"),
    (keys::HKEY_CTRL_F, "Ctrl+F"),
    (keys::HKEY_CTRL_P, "Ctrl+P"),
    (keys::HKEY_SHIFT_CLICK, "Shift+click (empty)"),
    (keys::HKEY_F3, "F3"),
    (keys::HKEY_ESC, "Esc"),
    (keys::HKEY_DEL, "Del"),
    (keys::HKEY_CTRL_Z, "Ctrl+Z"),
    (keys::HKEY_CTRL_Y, "Ctrl+Y"),
    (keys::HKEY_CTRL_C, "Ctrl+C"),
    (keys::HKEY_CTRL_X, "Ctrl+X"),
    (keys::HKEY_CTRL_V, "Ctrl+V"),
    (keys::HKEY_CTRL_D, "Ctrl+D"),
    (keys::HKEY_CTRL_G, "Ctrl+G"),
    (keys::HKEY_CTRL_B, "Ctrl+B"),
    (keys::HKEY_CTRL_CLICK, "Ctrl+click"),
    (keys::HKEY_LMB_DRAG, "LMB + drag"),
    (keys::HKEY_LMB_PORT, "LMB from port"),
    (keys::HKEY_LMB_HANDLE, "LMB on handle"),
    (keys::HKEY_DOUBLE_CLICK, "Double-click"),
    (keys::HKEY_RMB, "RMB"),
    (keys::HKEY_SPACE_DRAG, "Space+drag"),
    (keys::HKEY_CTRL_WHEEL, "Ctrl+wheel"),
    (keys::HKEY_CTRL_ENTER, "Ctrl+Enter"),
    (keys::HKEY_CTRL_COMMA, "Ctrl+,"),
    (keys::HKEY_CTRL_SHIFT_I, "Ctrl+Shift+I"),
    (keys::HKEY_F, "F"),
    (keys::HK_F1, "hotkey list"),
    (keys::HK_SEARCH, "search the canvas"),
    (keys::HK_PALETTE, "template palette"),
    (keys::HK_WHEEL, "template wheel menu"),
    (keys::HK_HUD, "HUD / next result"),
    (keys::HK_ESC, "close menus and panels"),
    (keys::HK_DELETE, "delete selection"),
    (keys::HK_UNDO, "undo"),
    (keys::HK_REDO, "redo"),
    (keys::HK_COPY, "copy nodes"),
    (keys::HK_CUT, "cut nodes"),
    (keys::HK_PASTE, "paste nodes"),
    (keys::HK_DUPLICATE, "duplicate nodes"),
    (keys::HK_GROUP, "group selection"),
    (keys::HK_BOTTLENECK, "bottleneck indicators"),
    (keys::HK_ADD_TO_SELECTION, "add to selection"),
    (keys::HK_SELECTION_FRAME, "selection frame"),
    (keys::HK_DRAG_EDGE, "drag an edge"),
    (keys::HK_REBIND_EDGE, "rebind an edge"),
    (keys::HK_DOUBLE_CLICK, "note / open file"),
    (keys::HK_CONTEXT_MENU, "object menu"),
    (keys::HK_PAN, "panning"),
    (keys::HK_ZOOM, "zoom"),
    (keys::HK_COMMIT_NOTE, "commit note"),
    (keys::HK_SETTINGS, "settings"),
    (keys::HK_WHATIF, "what-if scenarios"),
    (keys::HK_EDGE_FOCUS, "edge focus"),
    // --- Template palette ---
    (keys::TEMPLATES_TITLE, "Templates"),
    (keys::TEMPLATES_SEARCH, "Search templates…"),
    (keys::TEMPLATES_FOOTER, "Enter — insert at center · Esc — collapse"),
    // --- Widgets submenu ---
    (keys::WIDGETS_EMPTY, "(none installed)"),
    (keys::WIDGETS_REMOVE_ENTRY, "— Remove: {name}"),
    // --- Dialogs ---
    (keys::DIALOG_YES, "Yes"),
    (keys::DIALOG_NO, "No"),
    (keys::DIALOG_INSTALL_TITLE, "Install widget {name} {version}?"),
    (keys::DIALOG_UPDATE_TITLE, "Update widget {name} to {version}?"),
    (keys::DIALOG_REMOVE_TITLE, "Remove package {name}?"),
    (keys::DIALOG_CYCLE_TITLE, "Cycle detected: {chain}"),
    (
        keys::DIALOG_INSTALL_BODY,
        "The package will be copied to the local widgets folder.\nPermissions: {perms}.",
    ),
    (keys::DIALOG_PERMS_NONE, "no permissions"),
    (
        keys::DIALOG_REMOVE_BODY,
        "This widget's nodes will remain on the canvas as placeholders.\nThe package can be installed again by dragging its folder.",
    ),
    (
        keys::DIALOG_CYCLE_BODY,
        "The edge would close a value-flow cycle (the graph must be a DAG).\nCreate as a control edge — without carrying a value?",
    ),
    // --- FR-050 (stage C): replace dialog, pick menus, R-3 tooltips ---
    (keys::DIALOG_REPLACE_TITLE, "Replace source?"),
    (
        keys::DIALOG_REPLACE_BODY,
        "Parameter {param} is already fed by {source}.\nReplace the source with the new edge?",
    ),
    (keys::DIALOG_REPLACE_YES, "Replace"),
    (keys::DIALOG_CANCEL, "Cancel"),
    (keys::MENU_PICK_PARAM_TITLE, "Connect the value to a parameter"),
    (keys::MENU_PICK_LINE_TITLE, "Which line is the value source?"),
    (
        keys::TOOLTIP_UNMAPPED_SLOT,
        "Value not applied: the edge exists but the source yielded no value (source line deleted / became prose / column missing). Connect a node with an up-to-date value or fix the source line.",
    ),
    (
        keys::TOOLTIP_UNMAPPED_PARAM,
        "Parameter {param} gets no value: output {output} is missing on node \"{node}\" or yielded no value. Rebind the edge to an existing output (click the edge → rebind) or connect a node with an up-to-date value.",
    ),
    // --- Numi hints ---
    (keys::HINT_VAR, "sheet variable"),
    (keys::HINT_UNIT, "unit of measure"),
    (keys::HINT_PARAM, "template parameter"),
    (keys::HINT_DOLLAR_IN, "value-edge input (FR-014)"),
    (keys::HINT_DOLLAR_N, "input #{i}"),
    // --- Onboarding ---
    // --- Scheme gallery (FR-049) ---
    (keys::GALLERY_TITLE, "Scheme templates"),
    (keys::GALLERY_SEARCH, "Search schemes"),
    (keys::GALLERY_ALL, "All"),
    (keys::GALLERY_META, "nodes: {nodes}, edges: {edges}"),
    (keys::GALLERY_FOOTER, "Enter — open · Esc — close"),
    (keys::GALLERY_TRY, "Try it"),
    (keys::GALLERY_EMPTY_TITLE, "Start from a template"),
    (
        keys::GALLERY_EMPTY_BODY,
        "Ready-made diagrams with connections and calculations: open one and change the numbers — everything recalculates live.",
    ),
    (keys::GALLERY_EMPTY_OPEN, "Open gallery"),
    (keys::GALLERY_EMPTY_DISMISS, "Blank canvas"),
    (
        keys::GALLERY_APPLIED,
        "Scheme «{name}» added — one Ctrl+Z undoes it",
    ),
    (
        keys::GALLERY_UNKNOWN,
        "Scheme not found: {id} — check the ?template parameter",
    ),
    (keys::HELP_SCHEMES, "Scheme gallery"),
    (keys::ONBOARDING_BACK, "Back"),
    (keys::ONBOARDING_NEXT, "Next"),
    (keys::ONBOARDING_DONE, "Done"),
    (keys::ONBOARDING_SKIP, "Skip"),
    (keys::ONBOARDING_STEP1_TITLE, "Welcome to CanvasDesk"),
    (
        keys::ONBOARDING_STEP1_BODY,
        "This is an infinite zoomable canvas for your desktop: file cards, notes, edges and calculations. Pan with Space, the middle mouse button or a touchpad; zoom with Ctrl+wheel. Everything is saved to a .canvas file next to the app.",
    ),
    (keys::ONBOARDING_STEP2_TITLE, "Notes"),
    (
        keys::ONBOARDING_STEP2_BODY,
        "Double-click empty space to create a note with markdown markup (lists, headings, links). Color — via the selection context menu. Ctrl+Enter commits the text, Esc reverts the edit.",
    ),
    (keys::ONBOARDING_STEP3_TITLE, "Edges"),
    (
        keys::ONBOARDING_STEP3_BODY,
        "Hover a node — ports appear on its sides. Drag from a port to another node to connect; the end of an existing edge can be rebinded by dragging its handle.",
    ),
    (keys::ONBOARDING_STEP4_TITLE, "Groups and undo"),
    (
        keys::ONBOARDING_STEP4_BODY,
        "Frame several nodes and press Ctrl+G — you get a group that moves as one. Mistakes are undoable: Ctrl+Z to undo, Ctrl+Y to redo.",
    ),
    (keys::ONBOARDING_STEP5_TITLE, "Numi formulas"),
    (
        keys::ONBOARDING_STEP5_BODY,
        "Calculate right in the text: “width = 120 mm * 4” or “rate = 60 rps”. Units of measure (mm, ms, MB/s) and input hints are supported; the result shows under the line.",
    ),
    (keys::ONBOARDING_STEP6_TITLE, "Value flow"),
    (
        keys::ONBOARDING_STEP6_BODY,
        "Drag an edge with Shift held — that is a value edge: the upstream formula's value feeds the downstream $in input and recalculates live.",
    ),
    (keys::ONBOARDING_STEP7_TITLE, "Node templates"),
    (
        keys::ONBOARDING_STEP7_BODY,
        "Ctrl+P opens the palette of ready architectural roles (service, queue, database) with parameters and domain types. Shift+click on empty space — the radial wheel menu.",
    ),
    (keys::ONBOARDING_STEP8_TITLE, "What's next"),
    (
        keys::ONBOARDING_STEP8_BODY,
        "The “?” button next to settings — documentation and this tour again anytime. F1 — the hotkey list right in the app.",
    ),
    // --- Help menu and docs viewer ---
    (keys::HELP_DOCS, "Documentation ▸"),
    (keys::HELP_ONBOARDING, "Run onboarding"),
    (
        keys::DOCS_FOOTER,
        "Wheel — scroll · links — navigate · Esc — close",
    ),
    (keys::DOCS_PAGE_INDEX, "Home"),
    (keys::DOCS_PAGE_QUICK_START, "Quick start"),
    (keys::DOCS_PAGE_INTERFACE, "Interface objects"),
    (keys::DOCS_PAGE_HOTKEYS, "Hotkeys"),
    (keys::DOCS_PAGE_CALCULATIONS, "Calculations and value flow"),
    (keys::DOCS_PAGE_TEMPLATES, "Node templates"),
    (keys::DOCS_PAGE_FAQ, "FAQ"),
    (keys::DOCS_PAGE_AGENT_RECIPE, "AI-agent recipe"),
    // --- What-if ---
    (keys::WHATIF_PILL, "What-if scenarios"),
    (keys::WHATIF_OVERRIDES, "overrides: {count}"),
    (
        keys::WHATIF_NO_OVERRIDES,
        "no overrides — double-click a calculation line to add an override",
    ),
    (keys::WHATIF_APPLY, "Apply"),
    (keys::WHATIF_RESET, "Reset"),
    (keys::WHATIF_COMPARE, "Compare"),
    (keys::WHATIF_BASE, "Base"),
    (keys::WHATIF_COLUMN_VAR, "Variable"),
    (keys::WHATIF_SCENARIO_DEFAULT, "Scenario {n}"),
    (keys::WHATIF_ONLY_CALC_LINES, "only calculation lines can be overridden in what-if"),
    // --- Selection palette ---
    (keys::PAL_GROUP_COLOR, "Color"),
    (keys::PAL_GROUP_LAYOUT, "Layout"),
    (keys::PAL_GROUP_ACTIONS, "Actions"),
    (keys::PAL_GROUP_BRANCHING, "Branching"),
    (keys::PAL_GROUP_STYLE, "Style"),
    (keys::PAL_GROUP_THICKNESS, "Thickness"),
    (keys::PAL_GROUP_FLOW, "Flow"),
    (keys::PAL_GROUP_PORTS, "Ports"),
    (keys::PAL_GROUP_TEMPLATE, "Template"),
    (keys::PAL_COLOR_PRESET, "Color {preset}"),
    (keys::PAL_COLOR_NONE, "No color"),
    (keys::PAL_LAYOUT_TREE_LR, "Tree →"),
    (keys::PAL_LAYOUT_TREE_TB, "Tree ↓"),
    (keys::PAL_LAYOUT_RADIAL, "Radial"),
    (keys::PAL_STYLE_SOLID, "Solid"),
    (keys::PAL_STYLE_DASHED, "Dashed"),
    (keys::PAL_STYLE_DOTTED, "Dotted"),
    (keys::PAL_THICKNESS_THIN, "Thin"),
    (keys::PAL_THICKNESS_MEDIUM, "Medium"),
    (keys::PAL_THICKNESS_THICK, "Thick"),
    (keys::PAL_FLOW_VALUE, "Value"),
    (keys::PAL_FLOW_CONTROL, "Control"),
    (keys::PAL_PORTS_AUTO, "Auto (shortest path)"),
    (keys::PAL_PORTS_PIN_FROM, "Source: pin"),
    (keys::PAL_PORTS_PINNED_FROM, "Source: pinned"),
    (keys::PAL_PORTS_PIN_TO, "Target: pin"),
    (keys::PAL_PORTS_PINNED_TO, "Target: pinned"),
    (keys::PAL_ACTION_RENAME, "Rename"),
    (keys::PAL_ACTION_DUPLICATE, "Duplicate"),
    (keys::PAL_ACTION_GROUP, "Group"),
    (keys::PAL_ACTION_UNGROUP, "Ungroup"),
    (keys::PAL_ACTION_OPEN_FILE, "Open file"),
    (keys::PAL_ACTION_OPEN_FOLDER, "Open file folder"),
    (keys::PAL_ACTION_COPY_PATH, "Copy path"),
    (keys::PAL_ACTION_COPY_LINK, "Copy link"),
    (keys::PAL_ACTION_CLEAR_TEXT, "Clear text"),
    (keys::PAL_ACTION_SAVE_AS_TEMPLATE, "Save as template"),
    (keys::PAL_ACTION_ADD_CHILD, "Add child"),
    (keys::PAL_ACTION_ADD_SIBLING, "Add sibling"),
    (keys::PAL_ACTION_EXPAND_BRANCH, "Expand branch"),
    (keys::PAL_ACTION_COLLAPSE_BRANCH, "Collapse branch"),
    (keys::PAL_ACTION_WIDGET_RELOAD, "Reload widget"),
    (keys::PAL_ACTION_WIDGET_PERMISSIONS, "Widget permissions…"),
    (keys::PAL_TEMPLATE_UPDATE_TO, "Update to v{version} (was v{old})"),
    // --- Toasts ---
    (keys::TOAST_PORT_PINNED, "Edge port pinned"),
    (keys::TOAST_PORT_AUTO, "Edge ports: auto (shortest path)"),
    (keys::TOAST_PORT_FREED, "Port freed: shortest path"),
    (keys::TOAST_FLOW_CYCLE, "Flow cycle: {participants} — toggle rejected"),
    (keys::TOAST_TEMPLATE_SAVED, "Template “{name}” saved: {path}"),
    (keys::TOAST_TEMPLATE_SAVE_FAILED, "Failed to save template: {err}"),
    (keys::TOAST_TEMPLATE_UPDATED, "Template updated: {name} → v{version}"),
    (keys::TOAST_CANVAS_NOT_OPEN, "Canvas not open: {err}"),
    (keys::TOAST_CANVAS_OPENED, "Opened canvas: {name}"),
    (keys::TOAST_APPLY_DONE, "Apply: {count} overrides written to the model"),
    (keys::TOAST_WIDGET_NOT_INSTALLED, "Widget not installed: {err}"),
    (keys::TOAST_WIDGET_INSTALLED, "Widget {name} installed"),
    (keys::TOAST_WIDGET_UPDATED, "Widget {name} updated to {version}"),
    (keys::TOAST_WIDGET_SAME_VERSION, "Widget {name} already at this version"),
    (keys::TOAST_INSTALL_FAILED, "Installation failed: {err}"),
    (keys::TOAST_PACKAGE_REMOVED, "Package {name} removed"),
    (keys::TOAST_REMOVE_FAILED, "Removal failed: {err}"),
    (keys::TOAST_NO_RELATED_CARDS, "No related cards to lay out"),
    (keys::TOAST_RELATED_ALIGNED, "Related cards aligned"),
    (keys::TOAST_NO_LEVEL, "The root branch has no level — use Tab"),
    (keys::TOAST_BRANCH_COLLAPSED, "Branch collapsed"),
    (keys::TOAST_BRANCH_EXPANDED, "Branch expanded"),
    (keys::TOAST_PATH_COPIED, "Path copied"),
    (keys::TOAST_GROUP_UNGROUPED, "Group ungrouped"),
    (keys::TOAST_WIDGET_RELOADING, "Widget is reloading"),
    (keys::TOAST_PERMISSIONS, "Permissions: {summary}"),
    (keys::TOAST_NODE_INSERTED, "Node inserted into group"),
    (keys::TOAST_NODE_EXTRACTED, "Node moved out of group"),
    (keys::TOAST_ANALYSIS_ON, "Analysis mode enabled (Ctrl+B to disable)"),
    (keys::TOAST_FILE_UNAVAILABLE, "File unavailable: {file}"),
    (keys::TOAST_WIDGET_OPEN_FAILED, "Widget: failed to open {path}"),
    (keys::GROUP_DEFAULT_LABEL, "Group"),
    (keys::FILE_NEW_NOTE, "New note"),
    (keys::ROW_EDGE_AGGREGATION, "Edge aggregation"),
    (
        keys::DESC_EDGE_AGGREGATION,
        "A bundle between a pair is one line with a ×N badge; click opens the main stage.",
    ),
    (keys::STAGE_TITLE, "Connections"),
    (keys::STAGE_HINT, "Esc — close"),
    (keys::STAGE_LINE_LABEL, "line {n}"),
    (keys::STAGE_PARAM_LABEL, "→ {param}"),
    (
        keys::STAGE_BUNDLE_TITLE,
        "Bundle: {from} → {to} · ×{n}",
    ),
    (
        keys::STAGE_FOOT_HINT,
        "Esc or click the dimmed background — close · click an edge — select",
    ),
    // --- PRD-0007 (FR-048 X2): calculation chain check window ---
    (keys::ROW_EXPLAIN_DEPTH, "Calculation chain depth"),
    (
        keys::DESC_EXPLAIN_DEPTH,
        "How many levels of the explanation tree expand automatically (0 — all).",
    ),
    (keys::VALUE_EXPLAIN_ALL, "No limit"),
    (keys::EXPLAIN_TITLE, "Calculation chain check"),
    (
        keys::EXPLAIN_META,
        "Calculation chain for “{title}” · Esc — close",
    ),
    (keys::EXPLAIN_STALE, "Data changed — refresh overlay"),
    (keys::EXPLAIN_GONE, "The number is gone — panel closed"),
    (keys::EXPLAIN_LEAF_TAG, "source value"),
    (keys::EXPLAIN_STATS, "Levels: {lv} · Nodes: {n}"),
    (keys::EXPLAIN_UNMAPPED, "value not substituted"),
    (keys::EXPLAIN_CYCLE, "cycle"),
    (keys::EXPLAIN_UNLINKED, "unlinked"),
    (keys::EXPLAIN_TRUNCATED, "tree truncated"),
    (keys::EXPLAIN_EXPAND_BADGE, "+{n} deeper"),
    (keys::EXPLAIN_ADDR_SLOT, "input {n}"),
    (keys::EXPLAIN_ADDR_OUTPUT, "output {name}"),
    // Honest loader (AC-1.2): 12 build-stage captions.
    (keys::EXPLAIN_LOADER_1, "Started building the tree…"),
    (keys::EXPLAIN_LOADER_2, "Locating the chain root…"),
    (keys::EXPLAIN_LOADER_3, "Unfolding formulas into nodes…"),
    (keys::EXPLAIN_LOADER_4, "Checking every branch…"),
    (keys::EXPLAIN_LOADER_5, "Descending to constant leaves…"),
    (keys::EXPLAIN_LOADER_6, "Looking for unmapped inputs…"),
    (keys::EXPLAIN_LOADER_7, "Checking sheet variables…"),
    (keys::EXPLAIN_LOADER_8, "Matching parameter spills…"),
    (keys::EXPLAIN_LOADER_9, "Looking for data cycles…"),
    (keys::EXPLAIN_LOADER_10, "Computing node values…"),
    (keys::EXPLAIN_LOADER_11, "Preparing the canvas highlight…"),
    (keys::EXPLAIN_LOADER_12, "Almost there — laying out branches…"),
];

/// Поиск по таблице (линейный — таблицы статические, чтение раз в кадр).
fn lookup(table: &'static [(&'static str, &'static str)], key: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, value)| *value)
}

/// Перевод по ключу: таблица языка → fallback на вторую таблицу → сам ключ
/// (последний случай в рантайме не срабатывает — тест полноты гарантирует
/// непустые переводы в обеих таблицах; провал теста = перевод не теряется
/// молча).
pub fn tr(language: Language, key: &'static str) -> &'static str {
    let (primary, fallback) = match language {
        Language::Ru => (RU, EN),
        Language::En => (EN, RU),
    };
    lookup(primary, key)
        .or_else(|| lookup(fallback, key))
        .unwrap_or(key)
}

/// Перевод с подстановками: `{name}`-плейсхолдеры заменяются значениями
/// `subs` (форматирование на стороне вызова — FR-040, правило полной фразы).
pub fn trf(language: Language, key: &'static str, subs: &[(&str, &str)]) -> String {
    let mut text = tr(language, key).to_owned();
    for (placeholder, value) in subs {
        text = text.replace(placeholder, value);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Инвариант полноты (FR-040): у каждого ключа — непустой RU и непустой
    /// EN перевод; дублей ключей в таблице нет; множества ключей совпадают.
    #[test]
    fn tables_are_complete_and_consistent() {
        for table in [RU, EN] {
            for (i, (key, value)) in table.iter().enumerate() {
                assert!(!key.is_empty(), "пустой ключ в таблице {i}");
                assert!(!value.is_empty(), "пустое значение у {key}");
                assert!(
                    !table[..i].iter().any(|(k, _)| k == key),
                    "дубль ключа: {key}"
                );
            }
        }
        for (key, _) in RU {
            assert!(
                EN.iter().any(|(k, _)| k == key),
                "ключ без EN-перевода: {key}"
            );
        }
        for (key, _) in EN {
            assert!(
                RU.iter().any(|(k, _)| k == key),
                "ключ без RU-значения: {key}"
            );
        }
    }

    /// Переключение языка меняет репрезентативные тексты (заголовок
    /// настроек, пункт меню, тост, шаг онбординга); RU-значения — исходные
    /// (эталон постановки).
    #[test]
    fn tr_switches_representative_phrases() {
        assert_eq!(tr(Language::Ru, keys::SETTINGS_TITLE), "Настройки");
        assert_eq!(tr(Language::En, keys::SETTINGS_TITLE), "Settings");
        assert_eq!(tr(Language::Ru, keys::ROW_GRID), "Сетка");
        assert_eq!(tr(Language::En, keys::ROW_GRID), "Grid");
        assert_eq!(tr(Language::Ru, keys::MENU_NEW_GROUP), "Создать группу");
        assert_eq!(tr(Language::En, keys::MENU_NEW_GROUP), "New group");
        assert_eq!(tr(Language::Ru, keys::DIALOG_YES), "Да");
        assert_eq!(tr(Language::En, keys::DIALOG_YES), "Yes");
        assert_eq!(
            tr(Language::Ru, keys::ONBOARDING_STEP1_TITLE),
            "Добро пожаловать в CanvasDesk"
        );
        assert_eq!(
            tr(Language::En, keys::ONBOARDING_STEP1_TITLE),
            "Welcome to CanvasDesk"
        );
        // Значения настроек — из таблицы, не из core::label()
        assert_eq!(tr(Language::Ru, keys::CORNER_TOP_LEFT), "верхний левый");
        assert_eq!(tr(Language::En, keys::CORNER_TOP_LEFT), "top left");
        assert_eq!(tr(Language::Ru, keys::GRID_DENSITY_SPARSE), "редкая");
        assert_eq!(tr(Language::En, keys::GRID_DENSITY_SPARSE), "sparse");
    }

    /// Подстановки: плейсхолдеры заменяются, отсутствующие — остаются как есть.
    #[test]
    fn trf_substitutes_placeholders() {
        assert_eq!(
            trf(
                Language::Ru,
                keys::DIALOG_REMOVE_TITLE,
                &[("{name}", "Clock")]
            ),
            "Удалить пакет Clock?"
        );
        assert_eq!(
            trf(
                Language::En,
                keys::DIALOG_REMOVE_TITLE,
                &[("{name}", "Clock")]
            ),
            "Remove package Clock?"
        );
        assert_eq!(
            trf(Language::En, keys::HINT_DOLLAR_N, &[("{i}", "2")]),
            "input #2"
        );
        assert_eq!(
            trf(Language::Ru, keys::HINT_DOLLAR_N, &[("{i}", "2")]),
            "вход №2"
        );
        // Неизвестный плейсхолдер — текст без изменений
        assert_eq!(trf(Language::Ru, keys::DIALOG_YES, &[("{x}", "y")]), "Да");
    }

    /// Fallback: ключ, отсутствующий в обеих таблицах, возвращается как есть
    /// (рантайм не паникует; тест полноты не даёт переводу потеряться молча).
    #[test]
    fn tr_missing_key_falls_back_to_key_itself() {
        assert_eq!(tr(Language::Ru, "test.missing.key"), "test.missing.key");
        assert_eq!(tr(Language::En, "test.missing.key"), "test.missing.key");
    }

    /// Ключи настроек FR-039 (лейбл + описание каждой строки, табы,
    /// заголовок, подсказка) присутствуют в обоих языках — таблица строк
    /// модалки держится на общем источнике.
    #[test]
    fn settings_keys_present_in_both_tables() {
        let row_keys = [
            keys::ROW_BUTTON_CORNER,
            keys::ROW_GRID,
            keys::ROW_GRID_STYLE,
            keys::ROW_GRID_DENSITY,
            keys::ROW_EDGES_AVOID,
            keys::ROW_PORT_ZONE,
            keys::ROW_LINE_PORTS,
            keys::ROW_BOTTLENECK,
            keys::ROW_FOCUS_MODE,
            keys::ROW_HUD_ON_START,
            keys::ROW_LANGUAGE,
            keys::ROW_SNAP_ENABLED,
            keys::ROW_SNAP_GRID,
            keys::ROW_SNAP_GUIDES,
            keys::ROW_SNAP_COLLISION,
            keys::ROW_SNAP_TOLERANCE,
            keys::ROW_SNAP_SUB_ZOOM,
            keys::ROW_SNAP_COARSE_ZOOM,
        ];
        let desc_keys = [
            keys::DESC_BUTTON_CORNER,
            keys::DESC_GRID,
            keys::DESC_GRID_STYLE,
            keys::DESC_GRID_DENSITY,
            keys::DESC_EDGES_AVOID,
            keys::DESC_PORT_ZONE,
            keys::DESC_LINE_PORTS,
            keys::DESC_BOTTLENECK,
            keys::DESC_FOCUS_MODE,
            keys::DESC_HUD_ON_START,
            keys::DESC_LANGUAGE,
            keys::DESC_SNAP_ENABLED,
            keys::DESC_SNAP_GRID,
            keys::DESC_SNAP_GUIDES,
            keys::DESC_SNAP_COLLISION,
            keys::DESC_SNAP_TOLERANCE,
            keys::DESC_SNAP_SUB_ZOOM,
            keys::DESC_SNAP_COARSE_ZOOM,
        ];
        let tab_keys = [
            keys::TAB_GENERAL,
            keys::TAB_CANVAS,
            keys::TAB_EDGES,
            keys::TAB_APPEARANCE,
            keys::TAB_SNAP,
        ];
        for key in row_keys.into_iter().chain(desc_keys).chain(tab_keys) {
            assert!(!tr(Language::Ru, key).is_empty(), "RU пуст: {key}");
            assert!(!tr(Language::En, key).is_empty(), "EN пуст: {key}");
        }
    }

    /// Ключи пунктов batch-выравнивания (FR-038 п.16, T-038.5) присутствуют
    /// в обоих языках с точными фразами постановки (ключ = полная фраза,
    /// FR-040); полнота таблиц в целом — tables_are_complete_and_consistent.
    #[test]
    fn menu_align_keys_present_in_both_tables() {
        let phrases = [
            (
                keys::MENU_ALIGN_HORIZONTAL,
                "Выровнять по горизонтали",
                "Align horizontally",
            ),
            (
                keys::MENU_ALIGN_VERTICAL,
                "Выровнять по вертикали",
                "Align vertically",
            ),
            (
                keys::MENU_DISTRIBUTE_EVENLY,
                "Распределить равномерно",
                "Distribute evenly",
            ),
        ];
        for (key, ru, en) in phrases {
            assert_eq!(tr(Language::Ru, key), ru, "RU фраза: {key}");
            assert_eq!(tr(Language::En, key), en, "EN фраза: {key}");
        }
    }
}
