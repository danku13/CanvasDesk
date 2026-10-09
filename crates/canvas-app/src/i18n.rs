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
    /// Индикатор продолжения описания строки настроек («ещё N»): строки
    /// описания сверх лимита не теряются молча (дефект №15 аудита, W-f).
    pub const SETTINGS_DESC_MORE: &str = "settings.desc.more";
    pub const TAB_GENERAL: &str = "settings.tab.general";
    pub const TAB_CANVAS: &str = "settings.tab.canvas";
    pub const TAB_EDGES: &str = "settings.tab.edges";
    pub const TAB_APPEARANCE: &str = "settings.tab.appearance";
    /// FR-079 (S3): таб «Подсказки» — ИИ-карточки шаблонов (CR-022:
    /// попап при вводе — только L0, карточки C3 — после создания ноды).
    pub const TAB_SUGGEST: &str = "settings.tab.suggest";
    /// FR-087: таб «Профиль» — роль и фильтры подсказок.
    pub const TAB_PROFILE: &str = "settings.tab.profile";
    pub const ROW_ROLE: &str = "settings.row.role";
    pub const DESC_ROLE: &str = "settings.desc.role";
    /// FR-087: тумблеры категорий шаблонных нод (лейблы — «Шаблоны: <имя>»).
    pub const ROW_TPLCAT_BACKEND: &str = "settings.row.tplcat.backend";
    pub const ROW_TPLCAT_NETWORK: &str = "settings.row.tplcat.network";
    pub const ROW_TPLCAT_UNIT_ECONOMICS: &str = "settings.row.tplcat.unit_economics";
    pub const ROW_TPLCAT_PRODUCT_ANALYTICS: &str = "settings.row.tplcat.product_analytics";
    pub const DESC_TPLCAT: &str = "settings.desc.tplcat";
    /// FR-087: тумблеры категорий шаблонных схем («Схемы: <имя>»).
    pub const ROW_SCHEMECAT_ARCHITECTURE: &str = "settings.row.schemecat.architecture";
    pub const ROW_SCHEMECAT_BUSINESS: &str = "settings.row.schemecat.business";
    pub const ROW_SCHEMECAT_FRAMEWORK: &str = "settings.row.schemecat.framework";
    pub const ROW_SCHEMECAT_PLANNING: &str = "settings.row.schemecat.planning";
    pub const ROW_SCHEMECAT_ONBOARDING: &str = "settings.row.schemecat.onboarding";
    pub const DESC_SCHEMECAT: &str = "settings.desc.schemecat";
    /// FR-089: согласия телеметрии (чекбоксы первого запуска + таб «Профиль»).
    pub const ROW_TELEMETRY_COUNTER: &str = "settings.row.telemetry_counter";
    pub const DESC_TELEMETRY_COUNTER: &str = "settings.desc.telemetry_counter";
    pub const ROW_TELEMETRY_ANALYTICS: &str = "settings.row.telemetry_analytics";
    pub const DESC_TELEMETRY_ANALYTICS: &str = "settings.desc.telemetry_analytics";
    pub const TAB_SNAP: &str = "settings.tab.snap";
    pub const TAB_DRAG: &str = "settings.tab.drag";

    pub const ROW_DRAG_PUSH_ENABLED: &str = "settings.row.drag_push_enabled";
    pub const ROW_DRAG_PUSH_GAP: &str = "settings.row.drag_push_gap";
    pub const ROW_DRAG_PUSH_HALO: &str = "settings.row.drag_push_halo";
    pub const ROW_DRAG_PUSH_PREDICTIVE: &str = "settings.row.drag_push_predictive";
    pub const ROW_DRAG_PUSH_REBASE: &str = "settings.row.drag_push_rebase";

    pub const DESC_DRAG_PUSH_ENABLED: &str = "settings.desc.drag_push_enabled";
    pub const DESC_DRAG_PUSH_GAP: &str = "settings.desc.drag_push_gap";
    pub const DESC_DRAG_PUSH_HALO: &str = "settings.desc.drag_push_halo";
    pub const DESC_DRAG_PUSH_PREDICTIVE: &str = "settings.desc.drag_push_predictive";
    pub const DESC_DRAG_PUSH_REBASE: &str = "settings.desc.drag_push_rebase";

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
    /// FR-044 Р-4: панель «Как считается» — заголовки групп, unmapped-строка,
    /// счётчик внешних входов (Р-8), индикатор усечения (Q2 v1).
    pub const STAGE_CALC_VARS: &str = "stage.calc_vars";
    pub const STAGE_CALC_FORMULAS: &str = "stage.calc_formulas";
    pub const STAGE_CALC_UNMAPPED: &str = "stage.calc_unmapped";
    pub const STAGE_CALC_EXT: &str = "stage.calc_ext";
    pub const STAGE_CALC_MORE: &str = "stage.calc_more";
    /// FR-044 Р-3-а: лейбл слота выхода у истока (прототип drawPort R5).
    pub const STAGE_OUT_LABEL: &str = "stage.out_label";
    /// FR-044 Р-3-а: подпись control-ребра у приёмника (инвариант 5 —
    /// control-рёбра не отображаются value-путями).
    pub const STAGE_CTRL_LABEL: &str = "stage.ctrl_label";
    /// FR-044 Р-3-а: адрес control-ребра в пилюле — «to: <метка/приёмник>».
    pub const STAGE_CTRL_TO: &str = "stage.ctrl_to";
    /// FR-044 Q2 (Scroll): индикаторы прокрутки окна пилюль.
    pub const STAGE_PILL_ABOVE: &str = "stage.pill_above";
    pub const STAGE_PILL_BELOW: &str = "stage.pill_below";

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
    /// X3 (AC-4.1): кнопка «Изменить» на листе дерева — подмена what-if.
    pub const EXPLAIN_EDIT: &str = "explain.edit";
    /// Подсказка inline-поля подмены (AC-4.1).
    pub const EXPLAIN_EDIT_HINT: &str = "explain.edit_hint";
    /// X5 (AC-6.1): тумблер режима защиты — вход (шапка окна).
    pub const EXPLAIN_DEFENSE: &str = "explain.defense";
    /// X5 (AC-6.4): тумблер в защите — выход в обычный вид окна.
    pub const EXPLAIN_DEFENSE_EXIT: &str = "explain.defense_exit";
    /// X5 (AC-6.3): кнопка шага раскрытия (следующий уровень).
    pub const EXPLAIN_DEFENSE_STEP: &str = "explain.defense_step";
    /// X5 (AC-6.3): кнопка «Раскрыть всё».
    pub const EXPLAIN_DEFENSE_ALL: &str = "explain.defense_all";
    /// X5: подсказка футера защиты (пробел/клик — шаг, Esc — выход).
    pub const EXPLAIN_DEFENSE_HINT: &str = "explain.defense_hint";
    /// FR-083: тумблер направления схемы — поток слева-направо
    /// (источники слева, итог справа; глиф рисует направление потока).
    pub const EXPLAIN_DIR_LTR: &str = "explain.dir_ltr";
    /// FR-083: тумблер направления схемы — прежний вид (итог слева,
    /// источники справа).
    pub const EXPLAIN_DIR_RTL: &str = "explain.dir_rtl";
    /// FR-083: подсказка футера при переполнении дерева (пан включён).
    pub const EXPLAIN_OVERFLOW_HINT: &str = "explain.overflow_hint";
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

    // --- PRD-0007 (FR-048 X4): автосвязь по именам (F-7) ---
    /// Пункт меню канваса (AC-5.1): «Найти связи по именам».
    pub const MENU_AUTOLINK_FIND: &str = "menu.autolink_find";
    /// Бейдж-индикатор предложений (AC-5.5): «Связи по именам · {n}».
    pub const AUTOLINK_BADGE: &str = "autolink.badge";
    /// Заголовок диалога ревью (AC-5.2).
    pub const AUTOLINK_TITLE: &str = "autolink.title";
    /// Мета-строка шапки: «Найдено предложений: {n} · группировка: пара нод ·
    /// сортировка: по имени переменной».
    pub const AUTOLINK_META: &str = "autolink.meta";
    /// Пустой список: предложений нет.
    pub const AUTOLINK_EMPTY: &str = "autolink.empty";
    /// Баннер отклонённых (У8): «Отклонено: {n}. До закрытия диалога можно
    /// вернуть одним кликом.»
    pub const AUTOLINK_BANNER: &str = "autolink.banner";
    /// Кнопки строк и футера (AC-5.2).
    pub const AUTOLINK_ACCEPT: &str = "autolink.accept";
    pub const AUTOLINK_REJECT: &str = "autolink.reject";
    pub const AUTOLINK_ACCEPT_ALL: &str = "autolink.accept_all";
    pub const AUTOLINK_REJECT_ALL: &str = "autolink.reject_all";
    pub const AUTOLINK_RESTORE_ALL: &str = "autolink.restore_all";
    /// Главная кнопка футера: «Создать связи ({n})» (AC-5.3).
    pub const AUTOLINK_CREATE: &str = "autolink.create";
    /// Подсказка футера: создание — один undo-бат; откат — с подтверждением.
    pub const AUTOLINK_HINT: &str = "autolink.hint";
    /// Подпись адресации в строке: «параметр {name}».
    pub const AUTOLINK_PARAM: &str = "autolink.param";
    /// Тост после создания: «Создано связей: {n} — один undo-шаг.»
    pub const AUTOLINK_TOAST_CREATED: &str = "autolink.toast_created";
    /// Тост пустого результата команды: совпадений нет.
    pub const AUTOLINK_TOAST_NONE: &str = "autolink.toast_none";
    /// Настройка FR-039 (AC-5.5): тумблер фонового детектора.
    pub const ROW_AUTOLINK: &str = "settings.row.autolink";
    pub const DESC_AUTOLINK: &str = "settings.desc.autolink";
    /// FR-079 (S3): строки таба «Подсказки».
    pub const ROW_SUGGEST_ENABLED: &str = "settings.row.suggest_enabled";
    pub const DESC_SUGGEST_ENABLED: &str = "settings.desc.suggest_enabled";
    pub const ROW_SUGGEST_ENGINE: &str = "settings.row.suggest_engine";
    pub const DESC_SUGGEST_ENGINE: &str = "settings.desc.suggest_engine";
    pub const SUGGEST_ENGINE_LEX: &str = "settings.suggest.engine.lex";
    pub const SUGGEST_ENGINE_LEXLAYA: &str = "settings.suggest.engine.lexlaya";
    pub const SUGGEST_ENGINE_OFF: &str = "settings.suggest.engine.off";
    /// Настройка FR-039 (F-12, X6): opt-in тумблер индикатора покрытия.
    pub const ROW_EXPLAIN_COVERAGE: &str = "settings.row.explain_coverage";
    pub const DESC_EXPLAIN_COVERAGE: &str = "settings.desc.explain_coverage";
    /// FR-ICONS: набор иконок UI (Glyph/Lucide/Material/Feather/Bootstrap).
    pub const ROW_ICON_STYLE: &str = "settings.row.icon_style";
    pub const DESC_ICON_STYLE: &str = "settings.desc.icon_style";
    pub const ICON_STYLE_GLYPH: &str = "settings.value.icon_style.glyph";
    pub const ICON_STYLE_LUCIDE: &str = "settings.value.icon_style.lucide";
    pub const ICON_STYLE_MATERIAL: &str = "settings.value.icon_style.material";
    pub const ICON_STYLE_FEATHER: &str = "settings.value.icon_style.feather";
    pub const ICON_STYLE_BOOTSTRAP: &str = "settings.value.icon_style.bootstrap";
    /// Индикатор покрытия цепочками (F-12, X6): текст в углу канваса.
    pub const EXPLAIN_COVERAGE: &str = "explain.coverage";
    /// Подтверждение отката пачки (AC-5.3): заголовок/текст/кнопка.
    pub const AUTOLINK_UNDO_TITLE: &str = "autolink.undo_title";
    pub const AUTOLINK_UNDO_BODY: &str = "autolink.undo_body";
    pub const AUTOLINK_UNDO_YES: &str = "autolink.undo_yes";

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
    /// FR-080: «Автоширина по контенту» — пересчёт ширины выделенных нод
    /// по их тексту (target=10 слов/строку).
    pub const MENU_AUTOWIDTH: &str = "menu.autowidth";
    /// FR-087: «Об авторе» — открывает overlay с ссылками.
    pub const MENU_ABOUT: &str = "menu.about";

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
    pub const HKEY_F2: &str = "hotkeys.key.f2";
    pub const HKEY_RMB: &str = "hotkeys.key.rmb";
    pub const HKEY_SPACE_DRAG: &str = "hotkeys.key.space_drag";
    pub const HKEY_CTRL_WHEEL: &str = "hotkeys.key.ctrl_wheel";
    pub const HKEY_CTRL_ENTER: &str = "hotkeys.key.ctrl_enter";
    pub const HKEY_CTRL_COMMA: &str = "hotkeys.key.ctrl_comma";
    pub const HKEY_CTRL_SHIFT_I: &str = "hotkeys.key.ctrl_shift_i";
    pub const HKEY_F: &str = "hotkeys.key.f";
    pub const HKEY_SPACE: &str = "hotkeys.key.space";
    pub const HK_EXPLAIN_STEP: &str = "hotkeys.desc.explain_step";
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
    pub const HK_F2: &str = "hotkeys.desc.f2";
    pub const HK_CONTEXT_MENU: &str = "hotkeys.desc.context_menu";
    pub const HK_PAN: &str = "hotkeys.desc.pan";
    pub const HK_ZOOM: &str = "hotkeys.desc.zoom";
    pub const HK_COMMIT_NOTE: &str = "hotkeys.desc.commit_note";
    pub const HK_SETTINGS: &str = "hotkeys.desc.settings";
    pub const HK_WHATIF: &str = "hotkeys.desc.whatif";
    pub const HK_EDGE_FOCUS: &str = "hotkeys.desc.edge_focus";
    // FR-100: полный набор команд редактора ноды (UR-001-02) — F1-оверлей
    pub const HKEY_CTRL_BACKSPACE: &str = "hotkeys.key.ctrl_backspace";
    pub const HKEY_PAGE_UP_DOWN: &str = "hotkeys.key.page_up_down";
    pub const HKEY_TAB_INDENT: &str = "hotkeys.key.tab_indent";
    pub const HKEY_HOME_SMART: &str = "hotkeys.key.home_smart";
    pub const HKEY_CMD_EDITOR: &str = "hotkeys.key.cmd_editor";
    pub const HK_WORD_DELETE: &str = "hotkeys.desc.word_delete";
    pub const HK_EDITOR_PAGE: &str = "hotkeys.desc.editor_page";
    pub const HK_EDITOR_TAB: &str = "hotkeys.desc.editor_tab";
    pub const HK_EDITOR_HOME: &str = "hotkeys.desc.editor_home";
    pub const HK_CMD_PARITY: &str = "hotkeys.desc.cmd_parity";

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
    /// FR-045 F-5 v1 (PRD-0004 N3, R-5): лейбл якоря параметра шаблонной
    /// ноды при hover — qualified-адрес «to: Объект.Параметр» (полный путь
    /// в тултипе; короткая форма — на теле, решение FR-045 Р-5).
    pub const TOOLTIP_PORT_PARAM: &str = "tooltip.port.param";
    /// FR-045 F-5 v2 (PRD-0004 N3, R-3): маркер unmapped-истока в лейбле
    /// входного слота — короткая форма Р-3 («не подставлено»), полный
    /// диагноз — TOOLTIP_UNMAPPED_SLOT/PARAM (тултип ребра).
    pub const TOOLTIP_PORT_UNMAPPED: &str = "tooltip.port.unmapped";
    /// FR-045 F-5 v2 (PRD-0004 N3, R-5): свёртка длинного списка входов
    /// стороны — «+N ещё» (полный список — в stage, FR-044 Р-4).
    pub const TOOLTIP_PORT_MORE: &str = "tooltip.port.more";
    /// FR-050 Р-3 (этап C): тултип unmapped-позиционного входа.
    pub const TOOLTIP_UNMAPPED_SLOT: &str = "tooltip.unmapped.slot";
    /// FR-050 Р-3 (этап C): тултип unmapped-параметра (toParam без значения).
    pub const TOOLTIP_UNMAPPED_PARAM: &str = "tooltip.unmapped.param";
    /// FR-050 Н9-2 (этап D): тултип источника пролитого параметра — с
    /// прежним локальным значением.
    pub const TOOLTIP_SPILL_PARAM: &str = "tooltip.spill.param";
    /// FR-050 Н9-2 (этап D): тултип источника пролитого параметра — без
    /// локального литерала (строки не было / RHS пуст).
    pub const TOOLTIP_SPILL_PARAM_NO_LOCAL: &str = "tooltip.spill.param_no_local";
    /// FR-050 Н9-2 (этап D): тултип пролитого параметра при unmapped
    /// (источник не отдал значение).
    pub const TOOLTIP_SPILL_PARAM_NOVALUE: &str = "tooltip.spill.param_novalue";
    /// FR-050 Н9-2/Р-4 (этап D): тултип авто-строки приёмника — шаблонная
    /// нода (подсказка «подключите к параметру через toParam»).
    pub const TOOLTIP_SPILL_AUTOROW_TPL: &str = "tooltip.spill.autorow_tpl";
    /// FR-050 Н9-2/Р-4 (этап D): тултип авто-строки приёмника — текстовая
    /// нода (подсказка «используйте $N в формуле»).
    pub const TOOLTIP_SPILL_AUTOROW_TEXT: &str = "tooltip.spill.autorow_text";
    /// FR-050 Н9-2/Р-4 (этап D): тултип авто-строки при unmapped
    /// (значение не подставлено).
    pub const TOOLTIP_SPILL_AUTOROW_NOVALUE: &str = "tooltip.spill.autorow_novalue";
    /// FR-050 Н9-4 (этап E): пункт меню канваса — тогл панели карты
    /// проливаний.
    pub const MENU_FLOW_MAP: &str = "menu.flow_map";
    /// FR-050 Н9-4 (этап E): заголовок панели «Карта проливаний».
    pub const FLOW_MAP_TITLE: &str = "flow_map.title";
    /// FR-050 Н9-4 (этап E): пустое состояние панели (проливаний нет).
    pub const FLOW_MAP_EMPTY: &str = "flow_map.empty";
    /// FR-050 Н9-4 (этап E): строка «… ещё N» (кап видимых строк).
    pub const FLOW_MAP_MORE: &str = "flow_map.more";
    /// FR-050 Н9-4 (этап E): адрес позиционного приёмника в строке карты
    /// («вход {n}» — слот без имени параметра).
    pub const FLOW_MAP_INPUT: &str = "flow_map.input";
    /// FR-050 Н9-3 (этап E): заголовок контекст-меню пролитого параметра.
    pub const MENU_PARAM_TITLE: &str = "menu.param.title";
    /// FR-050 Н9-3 (этап E): заголовок контекст-меню авто-строки приёмника.
    pub const MENU_AUTOROW_TITLE: &str = "menu.autorow.title";
    /// FR-050 Н9-3 (этап E): пункт «Показать источник» (камера + подсветка).
    pub const MENU_PARAM_SOURCE: &str = "menu.param.source";
    /// FR-050 Н9-3 (этап E): пункт «Отключить проливание» (удалить ребро, Р-5).
    pub const MENU_PARAM_DISCONNECT: &str = "menu.param.disconnect";
    /// FR-050 Н9-3 (этап E): пункт «Что если…» (what-if режим FR-017).
    pub const MENU_PARAM_WHATIF: &str = "menu.param.whatif";
    /// FR-050 Н9-6 (этап E): тост при подключении проливания к параметру
    /// (строка присваивания была).
    pub const TOAST_SPILL_PARAM: &str = "toast.spill.param";
    /// FR-050 Н9-6 (этап E): тост при появлении авто-строки приёмника
    /// (параметра не было — строка-проекция).
    pub const TOAST_SPILL_AUTOROW: &str = "toast.spill.autorow";

    // --- Подсказки Numi-ввода (FR-021) ---
    pub const HINT_VAR: &str = "hints.var";
    pub const HINT_UNIT: &str = "hints.unit";
    pub const HINT_PARAM: &str = "hints.param";
    pub const HINT_DOLLAR_IN: &str = "hints.dollar_in";
    /// FR-079 (S3): деталь ИИ-строки в попапе FR-021 (источник виден —
    /// доверие, гипотеза §15).
    pub const HINT_AI_DETAIL: &str = "hints.ai_detail";
    pub const HINT_AI_DETAIL_FUSION: &str = "hints.ai_detail_fusion";
    pub const HINT_DOLLAR_N: &str = "hints.dollar_n";
    /// FR-101: деталь именованного входа — источник значения
    /// («проливание из ноды X»).
    pub const HINT_SPILL: &str = "hints.spill";
    /// FR-102: деталь подсказки поля после точки — источник значения
    /// («поле из ноды X»).
    pub const HINT_FIELD: &str = "hints.field";
    /// FR-079 follow-up: empty-state карточек C3 — предложений нет,
    /// тултип рядом с якорем.
    pub const SUGGEST_EMPTY: &str = "suggest.empty";

    // --- Онбординг (FR-028) ---
    pub const ONBOARDING_BACK: &str = "onboarding.back";
    // --- Галерея схем (FR-049) ---
    pub const GALLERY_TITLE: &str = "gallery.title";
    pub const GALLERY_SEARCH: &str = "gallery.search";
    pub const GALLERY_ALL: &str = "gallery.all";
    pub const GALLERY_META: &str = "gallery.meta";
    pub const GALLERY_FOOTER: &str = "gallery.footer";
    pub const GALLERY_EMPTY_TITLE: &str = "gallery.empty.title";
    pub const GALLERY_EMPTY_BODY: &str = "gallery.empty.body";
    pub const GALLERY_EMPTY_OPEN: &str = "gallery.empty.open";
    pub const GALLERY_EMPTY_DISMISS: &str = "gallery.empty.dismiss";
    /// Индикатор продолжения описания empty-state («ещё N»): строки
    /// описания сверх лимита не теряются молча (дефект №15 аудита, W-f).
    pub const GALLERY_EMPTY_MORE: &str = "gallery.empty.more";
    pub const GALLERY_APPLIED: &str = "gallery.applied";
    pub const GALLERY_UNKNOWN: &str = "gallery.unknown";
    pub const HELP_SCHEMES: &str = "help.schemes";
    /// FR-055 U4 (Q5-a): пункт меню «?» «О интерфейсе» — витрина кита.
    pub const HELP_INTERFACE: &str = "help.interface";
    /// FR-055 U4: заголовок витрины кита.
    pub const KIT_GALLERY_TITLE: &str = "kit.gallery.title";
    /// Кнопка темы в шапке витрины (реальный kit-контрол).
    pub const KIT_GALLERY_THEME: &str = "kit.gallery.theme";
    /// Варианты кнопок витрины.
    pub const KIT_BTN_PRIMARY: &str = "kit.button.primary";
    pub const KIT_BTN_SECONDARY: &str = "kit.button.secondary";
    pub const KIT_BTN_GHOST: &str = "kit.button.ghost";
    pub const KIT_BTN_DANGER: &str = "kit.button.danger";
    /// Dropdown-демо витрины.
    pub const KIT_DROPDOWN_ANCHOR: &str = "kit.dropdown.anchor";
    pub const KIT_DROPDOWN_ITEM: &str = "kit.dropdown.item";
    /// Toast/Tooltip-демо витрины.
    pub const KIT_TOAST_BODY: &str = "kit.toast.body";
    pub const KIT_TOOLTIP_ANCHOR: &str = "kit.tooltip.anchor";
    pub const KIT_TOOLTIP_BODY: &str = "kit.tooltip.body";
    /// Состояния витрины (подписи контролов).
    pub const KIT_STATE_NORMAL: &str = "kit.state.normal";
    pub const KIT_STATE_HOVER: &str = "kit.state.hover";
    pub const KIT_STATE_PRESSED: &str = "kit.state.pressed";
    pub const KIT_STATE_DISABLED: &str = "kit.state.disabled";
    pub const KIT_STATE_SELECTED: &str = "kit.state.selected";
    /// Секции витрины.
    pub const KIT_SECTION_BUTTONS: &str = "kit.section.buttons";
    pub const KIT_SECTION_ICON: &str = "kit.section.icon_buttons";
    pub const KIT_SECTION_CHIPS: &str = "kit.section.chips";
    pub const KIT_SECTION_DROPDOWN: &str = "kit.section.dropdown";
    pub const KIT_SECTION_TOAST: &str = "kit.section.toast";
    pub const KIT_SECTION_TOOLTIP: &str = "kit.section.tooltip";
    /// FR-059: секции компонентов v2 + их демо-подписи.
    pub const KIT_SECTION_TEXT_FIELD: &str = "kit.section.text_field";
    pub const KIT_SECTION_SWITCH: &str = "kit.section.switch";
    pub const KIT_SECTION_CARD: &str = "kit.section.card";
    pub const KIT_SECTION_LIST: &str = "kit.section.list";
    pub const KIT_SECTION_ICONS: &str = "kit.section.icons";
    pub const KIT_TEXTFIELD_PLACEHOLDER: &str = "kit.textfield.placeholder";
    pub const KIT_CARD_TITLE: &str = "kit.card.title";
    pub const KIT_CARD_BODY: &str = "kit.card.body";
    pub const KIT_LIST_ROW: &str = "kit.list.row";
    pub const KIT_STATE_FOCUSED: &str = "kit.state.focused";
    /// FR-062: секции layout v2 (measured/flex/wrap/grid/focus) + подписи демо.
    pub const KIT_SECTION_MEASURED: &str = "kit.section.measured";
    pub const KIT_SECTION_GROW: &str = "kit.section.grow";
    pub const KIT_SECTION_WRAP: &str = "kit.section.wrap";
    pub const KIT_SECTION_GRID: &str = "kit.section.grid";
    pub const KIT_SECTION_FOCUS: &str = "kit.section.focus";
    pub const KIT_MEASURED_A: &str = "kit.measured.a";
    pub const KIT_MEASURED_B: &str = "kit.measured.b";
    pub const KIT_MEASURED_C: &str = "kit.measured.c";
    pub const KIT_GROW_FIXED: &str = "kit.grow.fixed";
    pub const KIT_GROW_TWO: &str = "kit.grow.two";
    pub const KIT_GROW_ONE: &str = "kit.grow.one";
    pub const KIT_WRAP_CHIP: &str = "kit.wrap.chip";
    /// FR-068 W3 (этап M3): секция Table витрины — табличные строки через
    /// компонент [`canvas_ui::kit::Table`] (W3.3: витрина Row v1 заменена
    /// компонентом — секция единственная).
    pub const KIT_SECTION_TABLE: &str = "kit.section.table";
    pub const KIT_ROW_PRICE: &str = "kit.row.price";
    pub const KIT_ROW_QTY: &str = "kit.row.qty";
    pub const KIT_ROW_TOTAL: &str = "kit.row.total";
    pub const KIT_ROW_SUM: &str = "kit.row.sum";
    pub const KIT_ROW_UNIT_PRICE: &str = "kit.row.unit.price";
    pub const KIT_ROW_UNIT_QTY: &str = "kit.row.unit.qty";
    pub const KIT_ROW_UNIT_MONEY: &str = "kit.row.unit.money";
    pub const KIT_ROW_BADGE: &str = "kit.row.badge";
    /// Пересборка поверхностей (волна 2): демо компонентного слоя
    /// (`component::Row`/`component::Panel`) и недостающие layout-примитивы
    /// (SqueezeTail, MainAlign::SpaceBetween, Column) в витрине кита.
    pub const KIT_SECTION_COMPONENT_ROW: &str = "kit.section.component_row";
    pub const KIT_SECTION_COMPONENT_PANEL: &str = "kit.section.component_panel";
    pub const KIT_SECTION_SQUEEZE: &str = "kit.section.squeeze";
    pub const KIT_SECTION_ALIGN: &str = "kit.section.align";
    pub const KIT_COMPONENT_ROW_LABEL: &str = "kit.component.row_label";
    pub const KIT_COMPONENT_PANEL_BODY: &str = "kit.component.panel_body";
    pub const KIT_ALIGN_A: &str = "kit.align.a";
    pub const KIT_ALIGN_B: &str = "kit.align.b";
    pub const KIT_ALIGN_C: &str = "kit.align.c";
    /// DebugOverlay (F10): подсказка тогла.
    pub const KIT_DEBUG_HINT: &str = "kit.debug.hint";

    // --- FR-070: UI-админпанель (консоль дизайн-системы) ---
    pub const ADMIN_TITLE: &str = "admin.title";
    pub const ADMIN_HELP_ITEM: &str = "help.admin";
    pub const ADMIN_RESET: &str = "admin.reset";
    pub const ADMIN_SECTION_COMPONENTS: &str = "admin.section.components";
    pub const ADMIN_SECTION_FILL: &str = "admin.section.fill";
    pub const ADMIN_SECTION_CANVAS: &str = "admin.section.canvas";
    pub const ADMIN_SECTION_TOKENS: &str = "admin.section.tokens";
    pub const ADMIN_HINT_COMPONENTS: &str = "admin.hint.components";
    pub const ADMIN_HINT_FILL: &str = "admin.hint.fill";
    pub const ADMIN_HINT_CANVAS: &str = "admin.hint.canvas";
    pub const ADMIN_HINT_TOKENS: &str = "admin.hint.tokens";
    /// Этап 2: матрица состояний и уровни наполнения.
    pub const ADMIN_STATE_ERROR: &str = "admin.state.error";
    pub const ADMIN_FILL_EMPTY: &str = "admin.fill.empty";
    pub const ADMIN_FILL_MEDIUM: &str = "admin.fill.medium";
    pub const ADMIN_FILL_FULL: &str = "admin.fill.full";
    pub const ADMIN_ROW_LIST: &str = "admin.row.list";
    pub const ADMIN_ROW_CARD: &str = "admin.row.card";
    pub const ADMIN_ROW_FIELD: &str = "admin.row.field";
    pub const ADMIN_ROW_CHIPS: &str = "admin.row.chips";
    pub const ADMIN_ROW_DROPDOWN: &str = "admin.row.dropdown";
    pub const ADMIN_ROW_ROWS: &str = "admin.row.rows";
    pub const ADMIN_ROW_TOAST: &str = "admin.row.toast";
    pub const ADMIN_ROW_PALETTE: &str = "admin.row.palette";
    pub const ADMIN_ROW_WHEEL: &str = "admin.row.wheel";
    pub const ADMIN_EMPTY_PLACEHOLDER: &str = "admin.empty.placeholder";
    pub const ADMIN_CHIPS_EMPTY: &str = "admin.chips.empty";
    pub const ADMIN_ROWS_EMPTY: &str = "admin.rows.empty";
    pub const ADMIN_DROPDOWN_EMPTY: &str = "admin.dropdown.empty";
    pub const ADMIN_TOAST_EMPTY: &str = "admin.toast.empty";
    pub const ADMIN_PALETTE_EMPTY: &str = "admin.palette.empty";
    pub const ADMIN_WHEEL_EMPTY: &str = "admin.wheel.empty";
    pub const ADMIN_TOAST_SHORT: &str = "admin.toast.short";
    pub const ADMIN_TOAST_LONG: &str = "admin.toast.long";
    /// Этап 3: сущности канваса (ST4) и каталог токенов.
    pub const ADMIN_NODE_NORMAL: &str = "admin.node.normal";
    pub const ADMIN_NODE_SELECTED: &str = "admin.node.selected";
    pub const ADMIN_NODE_BROKEN: &str = "admin.node.broken";
    pub const ADMIN_NODE_GROUP: &str = "admin.node.group";
    pub const ADMIN_EDGE_DEFAULT: &str = "admin.edge.default";
    pub const ADMIN_EDGE_FLOW: &str = "admin.edge.flow";
    pub const ADMIN_EDGE_DRAFT: &str = "admin.edge.draft";
    pub const ADMIN_EDGE_DIMMED: &str = "admin.edge.dimmed";
    pub const ADMIN_PORT_IDLE: &str = "admin.port.idle";
    pub const ADMIN_PORT_HOVER: &str = "admin.port.hover";
    pub const ADMIN_PORT_ACTIVE: &str = "admin.port.active";
    pub const ADMIN_TOK_GROUP_SLOTS: &str = "admin.tok.group_slots";
    pub const ADMIN_TOK_GROUP_DIMS: &str = "admin.tok.group_dims";
    pub const ADMIN_TOK_GROUP_TYPE: &str = "admin.tok.group_type";
    pub const ADMIN_TOK_GROUP_MOTION: &str = "admin.tok.group_motion";
    /// Пересборка поверхностей (волна 2): расширение матрицы «Компоненты»
    /// (popup-контролы, состояния строк списков/таблиц) и демо табличного
    /// тела ноды (FR-061) в секции «Канвас».
    pub const ADMIN_MATRIX_DROPDOWN: &str = "admin.matrix.dropdown";
    pub const ADMIN_MATRIX_TOOLTIP: &str = "admin.matrix.tooltip";
    pub const ADMIN_MATRIX_TOAST: &str = "admin.matrix.toast";
    pub const ADMIN_MATRIX_LIST: &str = "admin.matrix.list";
    pub const ADMIN_MATRIX_ROWS: &str = "admin.matrix.rows";
    pub const ADMIN_NODE_TABLE: &str = "admin.node.table";
    /// Страница user-docs «UI-консоль» (FR-070, FR-031).
    pub const DOCS_PAGE_ADMIN: &str = "docs.page.admin";
    pub const ONBOARDING_NEXT: &str = "onboarding.next";
    pub const ONBOARDING_DONE: &str = "onboarding.done";
    pub const ONBOARDING_SKIP: &str = "onboarding.skip";
    pub const ONBOARDING_STEP1_TITLE: &str = "onboarding.step1.title";
    /// CR-031/S3: тело шага 1 — динамическое, по роли (см.
    /// [`onboarding_ui::welcome_body_key`]); базовый ключ — универсальное
    /// value без роли.
    pub const ONBOARDING_VALUE_DEFAULT: &str = "onboarding.value.default";
    pub const ONBOARDING_VALUE_ARCHITECT: &str = "onboarding.value.architect";
    pub const ONBOARDING_VALUE_DEVELOPER: &str = "onboarding.value.developer";
    pub const ONBOARDING_VALUE_PRODUCT: &str = "onboarding.value.product";
    pub const ONBOARDING_VALUE_ANALYST: &str = "onboarding.value.analyst";
    pub const ONBOARDING_VALUE_CIO: &str = "onboarding.value.cio";
    pub const ONBOARDING_VALUE_CTO: &str = "onboarding.value.cto";
    pub const ONBOARDING_VALUE_FOUNDER: &str = "onboarding.value.founder";
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
    /// CR-031: финальные CTA-опции последнего шага тура (вместо
    /// «Попробовать» на шаге «Шаблоны нод» — тот ведёт «Далее» до финала).
    pub const ONBOARDING_FINAL_OPEN: &str = "onboarding.final.open";
    pub const ONBOARDING_FINAL_EMPTY: &str = "onboarding.final.empty";

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
    /// Владелец 2026-10-02: пометка строк документации в глобальном поиске
    /// (Ctrl+F) — результаты по докам должны отличаться от поиска по нодам.
    pub const SEARCH_DOCS_SECTION: &str = "search.docs_section";

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
    pub const WHATIF_FREEZE: &str = "whatif.freeze";
    pub const WHATIF_UNFREEZE: &str = "whatif.unfreeze";
    pub const WHATIF_FROZEN_TOAST: &str = "whatif.frozen_toast";
    pub const WHATIF_UNFROZEN_TOAST: &str = "whatif.unfrozen_toast";
    pub const WHATIF_ROW_TOTAL: &str = "whatif.row_total";
    /// Индикатор усечённого хвоста списка подмен/таблицы сравнения —
    /// «… ещё N» (был хардкод RU в отрисовке).
    pub const WHATIF_MORE: &str = "whatif.more";
    /// Строка списка подмен «нода → стр. i: было → стало» (была хардкодом
    /// RU в отрисовке — CR-015/i18n).
    pub const WHATIF_OVERRIDE_ROW: &str = "whatif.override_row";
    /// Диалог полного сброса what-if (кнопка «Сброс» бара): заголовок.
    pub const DIALOG_WHATIF_RESET_TITLE: &str = "dialog.whatif_reset.title";
    /// Тело диалога сброса: статистика (сценарии/подмены) + гарантия,
    /// что текст нод не меняется + подсказка undo.
    pub const DIALOG_WHATIF_RESET_BODY: &str = "dialog.whatif_reset.body";
    /// Кнопка-подтверждение диалога сброса («Сбросить»).
    pub const DIALOG_WHATIF_RESET_YES: &str = "dialog.whatif_reset.yes";
    /// Тост после полного сброса what-if.
    pub const TOAST_WHATIF_RESET_DONE: &str = "toast.whatif_reset.done";
    /// Диалог удаления одного сценария («✕» чипа): заголовок с именем.
    pub const DIALOG_WHATIF_DELETE_TITLE: &str = "dialog.whatif_delete.title";
    /// Тело диалога удаления сценария: гарантии базы + undo.
    pub const DIALOG_WHATIF_DELETE_BODY: &str = "dialog.whatif_delete.body";
    /// Кнопка-подтверждение удаления сценария («Удалить»).
    pub const DIALOG_WHATIF_DELETE_YES: &str = "dialog.whatif_delete.yes";
    /// Тост после удаления сценария.
    pub const TOAST_WHATIF_DELETED: &str = "toast.whatif_deleted.done";

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
    /// FR-042 (правка сессии 2026-09-25): палитра связи пучка — удалить
    /// весь пучок одним действием (N ≥ 2 рёбер между упорядоченной парой
    /// нод). Удаление отдельного ребра — внутри main stage (ПКМ) + Del.
    pub const PAL_GROUP_BUNDLE: &str = "palette.group.bundle";
    pub const PAL_ACTION_DELETE_BUNDLE: &str = "palette.action.delete_bundle";

    // --- Тосты (T21-A, FR-016, CR-008 и др.) ---
    pub const TOAST_PORT_PINNED: &str = "toast.port_pinned";
    pub const TOAST_PORT_AUTO: &str = "toast.port_auto";
    pub const TOAST_PORT_FREED: &str = "toast.port_freed";
    pub const TOAST_FLOW_CYCLE: &str = "toast.flow_cycle";
    pub const TOAST_TEMPLATE_SAVED: &str = "toast.template_saved";
    pub const TOAST_TEMPLATE_SAVE_FAILED: &str = "toast.template_save_failed";
    // FR-076: тосты экспорта HTML-артефакта (Ctrl+Shift+E)
    pub const TOAST_EXPORT_HTML_OK: &str = "toast.export_html_ok";
    pub const TOAST_EXPORT_HTML_FAIL: &str = "toast.export_html_fail";
    // FR-078: deep-link ?focus= — нода не найдена
    pub const TOAST_FOCUS_NOT_FOUND: &str = "toast.focus_not_found";
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
    /// FR-040 v2: тост переключения языка кнопкой-иконкой. Подстановка
    /// `{lang}` — название языка в его собственной локали (`русский`/`English`).
    pub const TOAST_LANGUAGE_TOGGLED: &str = "toast.language_toggled";
    /// FR-087: тост смены роли (подстановка {role}).
    pub const TOAST_ROLE_CHANGED: &str = "toast.role_changed";
    pub const GROUP_DEFAULT_LABEL: &str = "group.default_label";
    pub const FILE_NEW_NOTE: &str = "file.new_note";

    // --- FR-LLM-B / PRD-0010 F-7: таб настроек «AI and Models» (9-й) ---
    /// Заголовок 9-го таба.
    pub const TAB_AI: &str = "settings.tab.ai";
    /// Секции таба AI (asgrp — group label).
    pub const AI_GRP_PROVIDER: &str = "ai.grp.provider";
    pub const AI_GRP_BYOK: &str = "ai.grp.byok";
    pub const AI_GRP_RATE: &str = "ai.grp.rate";
    pub const AI_GRP_LIMITS: &str = "ai.grp.limits";
    pub const AI_GRP_RESIDENCY: &str = "ai.grp.residency";
    pub const AI_GRP_TELEMETRY: &str = "ai.grp.telemetry";
    /// Лейблы строк (провайдеры per-feature).
    pub const AI_ROW_PROV_SUGGEST: &str = "ai.row.prov_suggest";
    pub const AI_ROW_PROV_GRAPH: &str = "ai.row.prov_graph";
    pub const AI_ROW_PROV_AGENT: &str = "ai.row.prov_agent";
    pub const AI_ROW_MODEL: &str = "ai.row.model";
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а вместо одной).
    pub const AI_ROW_MODEL_SUGGEST: &str = "ai.row.model_suggest";
    pub const AI_ROW_MODEL_GRAPH: &str = "ai.row.model_graph";
    pub const AI_ROW_MODEL_AGENT: &str = "ai.row.model_agent";
    pub const AI_ROW_KEY: &str = "ai.row.key";
    pub const AI_ROW_SH_URL: &str = "ai.row.sh_url";
    pub const AI_ROW_SH_KEY: &str = "ai.row.sh_key";
    pub const AI_ROW_TELEMETRY: &str = "ai.row.telemetry";
    // FR-LLM-FIX: лейблы/описания строк API-ключа BYOK и self-hosted endpoint
    // (видны при BYOK / SelfHosted). Существующие AI_ROW_KEY/AI_ROW_SH_URL/
    // AI_ROW_SH_KEY — legacy-ключи; новые — отдельный неймспейс FR-LLM-FIX.
    pub const AI_ROW_API_KEY: &str = "ai.row.api_key";
    pub const AI_ROW_SELFHOST_URL: &str = "ai.row.selfhost_url";
    pub const AI_ROW_SELFHOST_KEY: &str = "ai.row.selfhost_key";
    /// Описания строк.
    pub const AI_DESC_PROV_SUGGEST: &str = "ai.desc.prov_suggest";
    pub const AI_DESC_PROV_GRAPH: &str = "ai.desc.prov_graph";
    pub const AI_DESC_PROV_AGENT: &str = "ai.desc.prov_agent";
    pub const AI_DESC_FALLBACK: &str = "ai.desc.fallback";
    pub const AI_DESC_KEY: &str = "ai.desc.key";
    pub const AI_DESC_MODEL: &str = "ai.desc.model";
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а вместо одной).
    pub const AI_DESC_MODEL_SUGGEST: &str = "ai.desc.model_suggest";
    pub const AI_DESC_MODEL_GRAPH: &str = "ai.desc.model_graph";
    pub const AI_DESC_MODEL_AGENT: &str = "ai.desc.model_agent";
    pub const AI_DESC_RATE: &str = "ai.desc.rate";
    pub const AI_DESC_COST_LIMIT: &str = "ai.desc.cost_limit";
    pub const AI_DESC_CONF_THRESHOLD: &str = "ai.desc.conf_threshold";
    pub const AI_DESC_RESIDENCY: &str = "ai.desc.residency";
    pub const AI_DESC_TELEMETRY: &str = "ai.desc.telemetry";
    // FR-LLM-FIX: описания строк API-ключа и self-hosted endpoint.
    pub const AI_DESC_API_KEY: &str = "ai.desc.api_key";
    pub const AI_DESC_SELFHOST_URL: &str = "ai.desc.selfhost_url";
    pub const AI_DESC_SELFHOST_KEY: &str = "ai.desc.selfhost_key";
    /// FR-LLM-FIX: плейсхолдеры dropdown'а BYOK-модели (когда список пуст).
    pub const AI_MODEL_PH_NO_KEY: &str = "ai.model.ph_no_key";
    pub const AI_MODEL_PH_NO_HC: &str = "ai.model.ph_no_hc";
    pub const AI_MODEL_PH_EMPTY: &str = "ai.model.ph_empty";
    /// FR-LLM-FIX: бейдж «не выбрана» в статусной панели (активный провайдер
    /// без модели — ChatGPT/Ollama без подключения, BYOK без выбора модели).
    pub const AI_STATUS_MODEL_NOT_CHOSEN: &str = "ai.status.model_not_chosen";
    /// Опции dropdown (значения провайдеров + модели).
    pub const AI_OPT_OFF: &str = "ai.opt.off";
    pub const AI_OPT_LAYA: &str = "ai.opt.laya";
    pub const AI_OPT_OLLAMA: &str = "ai.opt.ollama";
    pub const AI_OPT_BYOK: &str = "ai.opt.byok";
    pub const AI_OPT_CHATGPT: &str = "ai.opt.chatgpt";
    pub const AI_OPT_CHATGPT_DISABLED: &str = "ai.opt.chatgpt_disabled";
    /// Опции data residency.
    pub const AI_RESIDENCY_LOCAL: &str = "ai.residency.local";
    pub const AI_RESIDENCY_CLOUD: &str = "ai.residency.cloud";
    pub const AI_RESIDENCY_SELFHOST: &str = "ai.residency.selfhost";
    pub const AI_RESIDENCY_LOCAL_DESC: &str = "ai.residency.local_desc";
    pub const AI_RESIDENCY_CLOUD_DESC: &str = "ai.residency.cloud_desc";
    pub const AI_RESIDENCY_SELFHOST_DESC: &str = "ai.residency.selfhost_desc";
    /// Кнопки/бейджи.
    pub const AI_BTN_CHECK_KEY: &str = "ai.btn.check_key";
    pub const AI_BTN_CHECK: &str = "ai.btn.check";
    pub const AI_KEY_VALID: &str = "ai.key_valid";
    pub const AI_KEY_NOT_CHECKED: &str = "ai.key_not_checked";
    pub const AI_KEY_EMPTY: &str = "ai.key_empty";
    pub const AI_SH_NOT_CHECKED: &str = "ai.sh_not_checked";
    pub const AI_SH_OK: &str = "ai.sh_ok";
    pub const AI_SH_BAD_URL: &str = "ai.sh_bad_url";
    // FR-LLM-OAUTH-APP / PRD-0010 F-5.8: блок «Вход ChatGPT» (строка
    // SettingsRow::AiOAuth: бейдж состояния + кнопка Войти/Отменить/
    // Выйти/Повторить).
    pub const AI_OAUTH_ROW: &str = "ai.oauth.row";
    pub const AI_OAUTH_DESC: &str = "ai.oauth.desc";
    pub const AI_OAUTH_BTN_LOGIN: &str = "ai.oauth.btn_login";
    pub const AI_OAUTH_HINT_BROWSER: &str = "ai.oauth.hint_browser";
    pub const AI_OAUTH_BTN_CANCEL: &str = "ai.oauth.btn_cancel";
    pub const AI_OAUTH_WAITING: &str = "ai.oauth.waiting";
    pub const AI_OAUTH_CONNECTED: &str = "ai.oauth.connected";
    pub const AI_OAUTH_ACCOUNT: &str = "ai.oauth.account";
    pub const AI_OAUTH_BTN_LOGOUT: &str = "ai.oauth.btn_logout";
    pub const AI_OAUTH_ERR: &str = "ai.oauth.err";
    pub const AI_OAUTH_BTN_RETRY: &str = "ai.oauth.btn_retry";
    pub const AI_OAUTH_UNAVAILABLE: &str = "ai.oauth.unavailable";
    // W2 (health/discovery/graph тосты + пункт меню Graph Builder).
    pub const AI_HEALTH_OK: &str = "ai.health.ok";
    pub const AI_HEALTH_FAIL: &str = "ai.health.fail";
    pub const AI_HEALTH_NO_KEY: &str = "ai.health.no_key";
    pub const AI_HEALTH_NO_ENDPOINT: &str = "ai.health.no_endpoint";
    pub const AI_DISCOVERY_OK: &str = "ai.discovery.ok";
    pub const AI_GRAPH_DONE: &str = "ai.graph.done";
    pub const AI_MENU_GRAPH_BUILDER: &str = "ai.menu.graph_builder";
    pub const AI_OAUTH_TOAST_LOGOUT: &str = "ai.oauth.toast_logout";
    pub const AI_COST_LIMIT_LABEL: &str = "ai.cost_limit_label";
    pub const AI_CONF_THR_LABEL: &str = "ai.conf_thr_label";
    pub const AI_FOOT_HINT: &str = "ai.foot_hint";
    pub const AI_RATE_BADGE: &str = "ai.rate_badge";

    // --- FR-LLM-B / PRD-0010 F-7.9: AI status panel ---
    pub const AI_STATUS_PAUSED: &str = "ai.status.paused";
    pub const AI_STATUS_SESSION: &str = "ai.status.session";
    pub const AI_STATUS_DAY: &str = "ai.status.day";
    pub const AI_STATUS_NO_MODEL: &str = "ai.status.no_model";
    // FR-LLM-FIX-2: короткие подписи провайдеров для чипа статусной панели
    // (паритет aiProvLabel() прототипа; полные подписи настроек —
    // «(свой ключ)»/«(вход)» — в шапке панели не помещаются в строку).
    pub const AI_PROV_SHORT_OFF: &str = "ai.status.prov.off";
    pub const AI_PROV_SHORT_LAYA: &str = "ai.status.prov.laya";
    pub const AI_PROV_SHORT_OLLAMA: &str = "ai.status.prov.ollama";
    pub const AI_PROV_SHORT_BYOK: &str = "ai.status.prov.byok";
    pub const AI_PROV_SHORT_CHATGPT: &str = "ai.status.prov.chatgpt";
    pub const AI_TOOLTIP_PAUSE: &str = "ai.tooltip.pause";
    pub const AI_TOOLTIP_SETTINGS: &str = "ai.tooltip.settings";
    pub const AI_TOOLTIP_SUGGEST: &str = "ai.tooltip.suggest";
    pub const AI_TOOLTIP_GRAPH: &str = "ai.tooltip.graph";
    pub const AI_TOOLTIP_AGENT: &str = "ai.tooltip.agent";

    // --- FR-LLM-B / PRD-0010 F-8: AI onboarding screen ---
    pub const AI_ONB_STEP: &str = "ai.onb.step";
    pub const AI_ONB_TITLE: &str = "ai.onb.title";
    pub const AI_ONB_SUB: &str = "ai.onb.sub";
    pub const AI_ONB_LOCAL_TITLE: &str = "ai.onb.local_title";
    pub const AI_ONB_CLOUD_TITLE: &str = "ai.onb.cloud_title";
    pub const AI_ONB_SELFHOST_TITLE: &str = "ai.onb.selfhost_title";
    pub const AI_ONB_LOCAL_DESC: &str = "ai.onb.local_desc";
    pub const AI_ONB_CLOUD_DESC: &str = "ai.onb.cloud_desc";
    pub const AI_ONB_SELFHOST_DESC: &str = "ai.onb.selfhost_desc";
    pub const AI_ONB_TAG_OFFLINE: &str = "ai.onb.tag_offline";
    pub const AI_ONB_TAG_QUALITY: &str = "ai.onb.tag_quality";
    pub const AI_ONB_TAG_CONTOUR: &str = "ai.onb.tag_contour";
    pub const AI_ONB_PRIV: &str = "ai.onb.priv";
    pub const AI_ONB_CONTINUE: &str = "ai.onb.continue";
    pub const AI_ONB_PRIV_TITLE: &str = "ai.onb.priv_title";
    pub const AI_ONB_PRIV_BODY: &str = "ai.onb.priv_body";
    pub const AI_ONB_PRIV_KEYS: &str = "ai.onb.priv_keys";
    pub const AI_ONB_PRIV_KEYS_BODY: &str = "ai.onb.priv_keys_body";
    pub const AI_ONB_PRIV_TELEMETRY: &str = "ai.onb.priv_telemetry";
    pub const AI_ONB_PRIV_TELEMETRY_BODY: &str = "ai.onb.priv_telemetry_body";
    pub const AI_ONB_PRIV_DOCS: &str = "ai.onb.priv_docs";
    pub const AI_ONB_TOAST_LOCAL: &str = "ai.onb.toast_local";
    pub const AI_ONB_TOAST_CLOUD: &str = "ai.onb.toast_cloud";
    pub const AI_ONB_TOAST_SELFHOST: &str = "ai.onb.toast_selfhost";
}

/// Русская таблица (эталон — порядок и полнота проверяются тестом).
const RU: &[(&str, &str)] = &[
    // --- Модалка настроек ---
    (keys::SETTINGS_TITLE, "Настройки"),
    (keys::SETTINGS_HINT, "Ctrl+, — открыть/закрыть"),
    (keys::SETTINGS_DESC_MORE, "ещё {n}"),
    (keys::TAB_GENERAL, "Общие"),
    (keys::TAB_CANVAS, "Канвас"),
    (keys::TAB_DRAG, "Драг"),
    (keys::ROW_DRAG_PUSH_ENABLED, "Расталкивание при драге"),
    (keys::ROW_DRAG_PUSH_GAP, "Сейф-зазор"),
    (keys::ROW_DRAG_PUSH_HALO, "Ореол активной ноды"),
    (keys::ROW_DRAG_PUSH_PREDICTIVE, "Предиктивное упреждение"),
    (keys::ROW_DRAG_PUSH_REBASE, "Новые якоря на drop"),
    (keys::TAB_EDGES, "Связи и порты"),
    (keys::TAB_APPEARANCE, "Внешний вид"),
    (keys::TAB_SUGGEST, "Подсказки"),
    (keys::TAB_PROFILE, "Профиль"),
    (keys::ROW_ROLE, "Роль"),
    (keys::DESC_ROLE, "Рабочая роль фильтрует подсказки шаблонов и схем: «Не выбрана» — все подсказки, роль — только соответствующие типы. Тумблеры ниже перекрывают выбор роли."),
    (keys::ROW_TPLCAT_BACKEND, "Шаблоны: Бэкенд"),
    (keys::ROW_TPLCAT_NETWORK, "Шаблоны: Сеть"),
    (keys::ROW_TPLCAT_UNIT_ECONOMICS, "Шаблоны: Юнит-экономика"),
    (keys::ROW_TPLCAT_PRODUCT_ANALYTICS, "Шаблоны: Продуктовая аналитика"),
    (keys::DESC_TPLCAT, "Показывать шаблонные ноды этой категории в палитре (Ctrl+P) и wheel-меню."),
    (keys::ROW_SCHEMECAT_ARCHITECTURE, "Схемы: Архитектура"),
    (keys::ROW_SCHEMECAT_BUSINESS, "Схемы: Бизнес"),
    (keys::ROW_SCHEMECAT_FRAMEWORK, "Схемы: Фреймворки"),
    (keys::ROW_SCHEMECAT_PLANNING, "Схемы: Планирование"),
    (keys::ROW_SCHEMECAT_ONBOARDING, "Схемы: Онбординг"),
    (keys::DESC_SCHEMECAT, "Показывать шаблонные схемы этой категории в галерее (Ctrl+T)."),
    (keys::ROW_TELEMETRY_COUNTER, "Анонимный счётчик использования"),
    (
        keys::DESC_TELEMETRY_COUNTER,
        "Раз в сутки одна отметка без личных данных — сколько людей пользуется приложением. Работает даже при выключенных метриках.",
    ),
    (keys::ROW_TELEMETRY_ANALYTICS, "Метрики и отчёты об ошибках"),
    (
        keys::DESC_TELEMETRY_ANALYTICS,
        "Помогает находить баги и улучшать CanvasDesk: события интерфейса и отчёты об ошибках (PostHog), без персональных данных.",
    ),
    (keys::TAB_SNAP, "Привязка"),
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
    (
        keys::DESC_DRAG_PUSH_ENABLED,
        "Соседи уступают дорогу таскаемой ноде и съезжаются обратно; накрытые получают новые якоря. Выключено — прежнее поведение (проход сквозь).",
    ),
    (
        keys::DESC_DRAG_PUSH_GAP,
        "Минимальная дистанция между границами любых двух нод, px сцены. 0 — вплотную (прежнее поведение). Не зависит от зума.",
    ),
    (
        keys::DESC_DRAG_PUSH_HALO,
        "Зона отчуждения вокруг таскаемой ноды поверх сейф-зазора, px сцены. Итоговый отступ от активной = ореол + зазор.",
    ),
    (
        keys::DESC_DRAG_PUSH_PREDICTIVE,
        "Ореол расширяется по направлению быстрого движения курсора — соседи готовятся заранее.",
    ),
    (
        keys::DESC_DRAG_PUSH_REBASE,
        "Нода, чьё место занято таскаемой, закрепляется на вытесненной позиции. Выключено — все вытесненные всегда съезжаются обратно.",
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
    (keys::MENU_AUTOWIDTH, "Автоширина по контенту"),
    (keys::MENU_ABOUT, "Об авторе"),
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
    (keys::HKEY_F2, "F2"),
    (keys::HKEY_RMB, "ПКМ"),
    (keys::HKEY_SPACE_DRAG, "Space+drag"),
    (keys::HKEY_CTRL_WHEEL, "Ctrl+колесо"),
    (keys::HKEY_CTRL_ENTER, "Ctrl+Enter"),
    (keys::HKEY_CTRL_COMMA, "Ctrl+,"),
    (keys::HKEY_CTRL_SHIFT_I, "Ctrl+Shift+I"),
    (keys::HKEY_F, "F"),
    (keys::HKEY_SPACE, "Space"),
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
    (keys::HK_F2, "правка выделенной ноды"),
    (keys::HK_CONTEXT_MENU, "меню объекта"),
    (keys::HK_PAN, "панорамирование"),
    (keys::HK_ZOOM, "масштаб"),
    (keys::HK_COMMIT_NOTE, "зафиксировать заметку"),
    (keys::HK_SETTINGS, "настройки"),
    (keys::HK_WHATIF, "what-if сценарии"),
    (keys::HK_EDGE_FOCUS, "фокус на связях"),
    (
        keys::HK_EXPLAIN_STEP,
        "проверка цепочки, режим защиты: раскрыть следующий уровень",
    ),
    // FR-100: команды редактора ноды
    (keys::HKEY_CTRL_BACKSPACE, "Ctrl+Backspace / Ctrl+Delete"),
    (keys::HKEY_PAGE_UP_DOWN, "PageUp / PageDown"),
    (keys::HKEY_TAB_INDENT, "Tab / Shift+Tab"),
    (keys::HKEY_HOME_SMART, "Home ×2"),
    (keys::HKEY_CMD_EDITOR, "Cmd+клавиша (macOS)"),
    (keys::HK_WORD_DELETE, "в редакторе текста: удалить слово слева / справа"),
    (
        keys::HK_EDITOR_PAGE,
        "в редакторе текста: страница строк (Shift — с выделением)",
    ),
    (
        keys::HK_EDITOR_TAB,
        "в редакторе текста: индент / убрать индент строк",
    ),
    (
        keys::HK_EDITOR_HOME,
        "в редакторе текста: первый непробельный строки, повторно — начало строки",
    ),
    (keys::HK_CMD_PARITY, "команды редактора — как с Ctrl"),
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
    (keys::TOOLTIP_PORT_UNMAPPED, "не подставлено"),
    (keys::TOOLTIP_PORT_MORE, "+{n} ещё"),
    (
        keys::TOOLTIP_UNMAPPED_SLOT,
        "Значение не подставлено: связь есть, но источник не отдал значение (строка-источник удалена / стала прозой / колонка отсутствует). Подключите ноду с актуальным значением или исправьте строку-источник.",
    ),
    (
        keys::TOOLTIP_PORT_PARAM,
        "to: {path}",
    ),
    (
        keys::TOOLTIP_UNMAPPED_PARAM,
        "Параметр {param} не получает значение: выход {output} отсутствует у ноды \"{node}\" или не отдал значение. Перепривяжите связь на существующий выход (клик по ребру → перепривязка) или подключите ноду с актуальным значением.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM,
        "Пролито: {path} = {value} (локально было: {local}). Правка значения — отключите проливание (контекст-меню строки или удаление связи), Ctrl+Z вернёт связь.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM_NO_LOCAL,
        "Пролито: {path} = {value}. Локального значения не было — при удалении связи параметр останется пустым.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM_NOVALUE,
        "Пролито из {path}: источник не отдал значение (строка-источник удалена / стала прозой / выход отсутствует). Исправьте строку-источник или перепривяжите связь.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_TPL,
        "Пролито: {path} = {value} — вход ${slot} приходит в ноду, но формула шаблона его не читает. Подключите связь к параметру (drag на якорь параметра, toParam) — значение подставится в расчёт.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_TEXT,
        "Пролито: {path} = {value} — вход ${slot} приходит в ноду, но формула его не читает. Используйте ${slot} в формуле (или именованный путь {path}) — значение подставится в расчёт.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_NOVALUE,
        "{path}: значение не подставлено — связь есть, но источник не отдал значение. Подключите ноду с актуальным значением или исправьте строку-источник.",
    ),
    // --- FR-050 этап E: Н9-3 контекст-меню параметра, Н9-4 карта, Н9-6 тост ---
    (
        keys::MENU_FLOW_MAP,
        "Карта проливаний (Ctrl+Shift+M)",
    ),
    (keys::FLOW_MAP_TITLE, "Проливания"),
    (
        keys::FLOW_MAP_EMPTY,
        "Проливаний нет — подключите значение к параметру или входу ноды",
    ),
    (keys::FLOW_MAP_MORE, "… ещё {n}"),
    (keys::FLOW_MAP_INPUT, "вход {n}"),
    (keys::MENU_PARAM_TITLE, "Проливание в параметр"),
    (keys::MENU_AUTOROW_TITLE, "Входящее значение"),
    (keys::MENU_PARAM_SOURCE, "Показать источник"),
    (keys::MENU_PARAM_DISCONNECT, "Отключить проливание"),
    (keys::MENU_PARAM_WHATIF, "Что если…"),
    (
        keys::TOAST_SPILL_PARAM,
        "Параметр {param} подтянулся из {path} — Ctrl+Z отменит",
    ),
    (
        keys::TOAST_SPILL_AUTOROW,
        "Значение подтянулось из {path} — Ctrl+Z отменит",
    ),
    // --- Подсказки Numi ---
    (keys::HINT_VAR, "переменная листа"),
    (keys::HINT_UNIT, "единица измерения"),
    (keys::HINT_PARAM, "параметр шаблона"),
    (keys::HINT_DOLLAR_IN, "вход value-рёбер (FR-014)"),
    (keys::HINT_AI_DETAIL, "ИИ · заменит ноду шаблоном"),
    (keys::HINT_AI_DETAIL_FUSION, "ИИ+ · заменит ноду шаблоном"),
    (keys::HINT_DOLLAR_N, "вход №{i}"),
    (keys::HINT_SPILL, "проливание из ноды «{node}»"),
    (keys::HINT_FIELD, "поле из ноды «{node}»"),
    (keys::SUGGEST_EMPTY, "✦ AI-дополнений нет"),
    // --- Онбординг ---
    // --- Галерея схем (FR-049) ---
    (keys::GALLERY_TITLE, "Шаблоны схем"),
    (keys::GALLERY_SEARCH, "Поиск схем"),
    (keys::GALLERY_ALL, "Все"),
    (keys::GALLERY_META, "ноды: {nodes}, связи: {edges}"),
    (keys::GALLERY_FOOTER, "Enter — открыть · Esc — закрыть"),
    (keys::GALLERY_EMPTY_TITLE, "Начните с шаблона"),
    (
        keys::GALLERY_EMPTY_BODY,
        "Готовые схемы со связями и расчётами: откройте одну и поменяйте числа — всё пересчитается на живую.",
    ),
    (keys::GALLERY_EMPTY_OPEN, "Открыть галерею"),
    (keys::GALLERY_EMPTY_DISMISS, "Пустой холст"),
    (keys::GALLERY_EMPTY_MORE, "ещё {n}"),
    (
        keys::GALLERY_APPLIED,
        "Схема «{name}» добавлена — одно Ctrl+Z отменяет",
    ),
    (
        keys::GALLERY_UNKNOWN,
        "Схема не найдена: {id} — проверьте параметр ?template",
    ),
    (keys::HELP_SCHEMES, "Галерея схем"),
    (keys::HELP_INTERFACE, "О интерфейсе"),
    (keys::KIT_GALLERY_TITLE, "Витрина интерфейса (UI kit v1)"),
    (keys::KIT_GALLERY_THEME, "Переключить тему"),
    (keys::KIT_BTN_PRIMARY, "Главная"),
    (keys::KIT_BTN_SECONDARY, "Вторичная"),
    (keys::KIT_BTN_GHOST, "Призрачная"),
    (keys::KIT_BTN_DANGER, "Опасная"),
    (keys::KIT_DROPDOWN_ANCHOR, "Выпадающий список"),
    (keys::KIT_DROPDOWN_ITEM, "Пункт списка"),
    (keys::KIT_TOAST_BODY, "Тост: внизу по центру, 3 с (T21-A)"),
    (keys::KIT_TOOLTIP_ANCHOR, "Наведи"),
    (
        keys::KIT_TOOLTIP_BODY,
        "Тултип: якорь + flip при нехватке места + delay 500 мс",
    ),
    (keys::KIT_STATE_NORMAL, "Обычное"),
    (keys::KIT_STATE_HOVER, "Наведение"),
    (keys::KIT_STATE_PRESSED, "Нажатие"),
    (keys::KIT_STATE_DISABLED, "Недоступно"),
    (keys::KIT_STATE_SELECTED, "Выбрано"),
    (keys::KIT_SECTION_BUTTONS, "Кнопки (Button × варианты × состояния)"),
    (keys::KIT_SECTION_ICON, "Икон-кнопки (IconButton)"),
    (keys::KIT_SECTION_CHIPS, "Чипы (Chip)"),
    (keys::KIT_SECTION_DROPDOWN, "Выпадающий список (Dropdown: якорь + flip)"),
    (keys::KIT_SECTION_TOAST, "Тост (Toast)"),
    (keys::KIT_SECTION_TOOLTIP, "Тултип (Tooltip: якорь + flip + delay)"),
    // FR-059: секции компонентов v2
    (keys::KIT_SECTION_TEXT_FIELD, "Текстовое поле (TextField: каретка в символах)"),
    (keys::KIT_SECTION_SWITCH, "Переключатель (Switch)"),
    (keys::KIT_SECTION_CARD, "Карточка (Card: хедер + тело)"),
    (keys::KIT_SECTION_LIST, "Список и скролл (list_rows + ScrollState)"),
    (keys::KIT_SECTION_ICONS, "Иконки (Icon: глифы существующим шрифтом)"),
    // FR-062: секции layout v2
    (
        keys::KIT_SECTION_MEASURED,
        "Measured-ряд (F-13: ширины из TextMeasurer)",
    ),
    (keys::KIT_SECTION_GROW, "Flex-факторы (F-14: grow 2:1 + End)"),
    (keys::KIT_SECTION_WRAP, "Перенос ряда (F-15: Wrap в слоте)"),
    (keys::KIT_SECTION_GRID, "Сетка (F-16: grid_cells 4×2)"),
    (
        keys::KIT_SECTION_FOCUS,
        "Фокус (F-17: FocusRing, Tab/Shift+Tab)",
    ),
    // FR-068 W3 (M3): секция Table — строки через компонент Table
    // (W3.3: единственная табличная секция витрины)
    (
        keys::KIT_SECTION_TABLE,
        "Таблица (Table: строки на общих направляющих)",
    ),
    (keys::KIT_ROW_PRICE, "цена"),
    (keys::KIT_ROW_QTY, "кол-во"),
    (keys::KIT_ROW_TOTAL, "итого = цена × кол-во"),
    (keys::KIT_ROW_SUM, "Σ чека"),
    (keys::KIT_ROW_UNIT_PRICE, "₽/шт"),
    (keys::KIT_ROW_UNIT_QTY, "шт"),
    (keys::KIT_ROW_UNIT_MONEY, "₽"),
    (keys::KIT_ROW_BADGE, "← источник"),
    (keys::KIT_MEASURED_A, "Введено"),
    (keys::KIT_MEASURED_B, "Формула"),
    (keys::KIT_MEASURED_C, "Итог, %"),
    (keys::KIT_GROW_FIXED, "фикс."),
    (keys::KIT_GROW_TWO, "grow ×2"),
    (keys::KIT_GROW_ONE, "grow ×1"),
    (keys::KIT_WRAP_CHIP, "Чип {n}"),
    (keys::KIT_TEXTFIELD_PLACEHOLDER, "Подсказка…"),
    (keys::KIT_CARD_TITLE, "Карточка"),
    (
        keys::KIT_CARD_BODY,
        "Тело карточки: хедер и контент внутри пада панели (SPACING_LG).",
    ),
    (keys::KIT_LIST_ROW, "Строка {n}"),
    (keys::KIT_STATE_FOCUSED, "В фокусе"),
    // Пересборка поверхностей: демо компонентного слоя + layout-примитивы
    (
        keys::KIT_SECTION_COMPONENT_ROW,
        "Компонент Row (component::Row: Props + состояние)",
    ),
    (
        keys::KIT_SECTION_COMPONENT_PANEL,
        "Компонент Panel (component::Panel: панель + контент)",
    ),
    (
        keys::KIT_SECTION_SQUEEZE,
        "Деградация ряда (SqueezeTail: хвост сжимается до нуля)",
    ),
    (
        keys::KIT_SECTION_ALIGN,
        "Выравнивание и колонка (SpaceBetween / Column + распорка)",
    ),
    (keys::KIT_COMPONENT_ROW_LABEL, "Компонентная строка"),
    (
        keys::KIT_COMPONENT_PANEL_BODY,
        "Контент компонента Panel (panel_content — минус пад SPACING_LG)",
    ),
    (keys::KIT_ALIGN_A, "Начало"),
    (keys::KIT_ALIGN_B, "Центр"),
    (keys::KIT_ALIGN_C, "Конец"),
    (
        keys::KIT_DEBUG_HINT,
        "DebugOverlay (F9): рамки слоёв, имя под курсором, пересечения",
    ),
    // --- FR-070: UI-админпанель ---
    (keys::ADMIN_TITLE, "UI-консоль"),
    (keys::ADMIN_HELP_ITEM, "UI-консоль (дизайн-система)"),
    (keys::ADMIN_RESET, "Сброс"),
    (keys::ADMIN_SECTION_COMPONENTS, "Компоненты"),
    (keys::ADMIN_SECTION_FILL, "Наполнение"),
    (keys::ADMIN_SECTION_CANVAS, "Канвас"),
    (keys::ADMIN_SECTION_TOKENS, "Токены"),
    (
        keys::ADMIN_HINT_COMPONENTS,
        "Матрица состояний контролов кита: обычное / hover / выбрано / нажато / недоступно + в фокусе / ошибка.",
    ),
    (
        keys::ADMIN_HINT_FILL,
        "Контейнеры с разным наполнением: пустое / среднее / полное — список, карточка, поле, чипы, dropdown, строки, тосты, палитра, wheel.",
    ),
    (
        keys::ADMIN_HINT_CANVAS,
        "Состояния сущностей канваса (ST4): карточка ноды, рёбра, порты.",
    ),
    (
        keys::ADMIN_HINT_TOKENS,
        "Каталог design-токенов: цвета-свотчи, размеры, типографика, движение; live-правка слотов палитры — клик по свотчу, «Сброс» в шапке.",
    ),
    // Этап 2: матрица состояний и наполнение
    (keys::ADMIN_STATE_ERROR, "Ошибка"),
    (keys::ADMIN_FILL_EMPTY, "Пустое"),
    (keys::ADMIN_FILL_MEDIUM, "Среднее"),
    (keys::ADMIN_FILL_FULL, "Полное"),
    (keys::ADMIN_ROW_LIST, "Список"),
    (keys::ADMIN_ROW_CARD, "Карточка"),
    (keys::ADMIN_ROW_FIELD, "Текстовое поле"),
    (keys::ADMIN_ROW_CHIPS, "Чипы"),
    (keys::ADMIN_ROW_DROPDOWN, "Dropdown"),
    (keys::ADMIN_ROW_ROWS, "Строки таблицы"),
    (keys::ADMIN_ROW_TOAST, "Тосты"),
    (keys::ADMIN_ROW_PALETTE, "Палитра шаблонов"),
    (keys::ADMIN_ROW_WHEEL, "Wheel-меню"),
    (keys::ADMIN_EMPTY_PLACEHOLDER, "Нет данных"),
    (keys::ADMIN_CHIPS_EMPTY, "Нет меток"),
    (keys::ADMIN_ROWS_EMPTY, "Нет строк"),
    (keys::ADMIN_DROPDOWN_EMPTY, "Пусто"),
    (keys::ADMIN_TOAST_EMPTY, "Тостов нет"),
    (keys::ADMIN_PALETTE_EMPTY, "Нет шаблонов"),
    (keys::ADMIN_WHEEL_EMPTY, "Нет категорий"),
    (keys::ADMIN_TOAST_SHORT, "Сохранено"),
    (keys::ADMIN_TOAST_LONG, "Схема применена: 12 нод обновлено"),
    // Этап 3: сущности канваса (ST4)
    (keys::ADMIN_NODE_NORMAL, "Обычная"),
    (keys::ADMIN_NODE_SELECTED, "Выбрана"),
    (keys::ADMIN_NODE_BROKEN, "Битая ссылка"),
    (keys::ADMIN_NODE_GROUP, "В группе"),
    (keys::ADMIN_EDGE_DEFAULT, "Обычное"),
    (keys::ADMIN_EDGE_FLOW, "Поток значения"),
    (keys::ADMIN_EDGE_DRAFT, "Черновик (drag)"),
    (keys::ADMIN_EDGE_DIMMED, "Приглушённое"),
    (keys::ADMIN_PORT_IDLE, "Точка"),
    (keys::ADMIN_PORT_HOVER, "Курсор"),
    (keys::ADMIN_PORT_ACTIVE, "Активный драфт"),
    (keys::ADMIN_TOK_GROUP_SLOTS, "Слоты палитры — клик по свотчу меняет цвет"),
    (keys::ADMIN_TOK_GROUP_DIMS, "Размеры (px)"),
    (keys::ADMIN_TOK_GROUP_TYPE, "Типографика (px)"),
    (keys::ADMIN_TOK_GROUP_MOTION, "Движение (мс)"),
    // Пересборка поверхностей: расширение матрицы «Компоненты» + канвас
    (
        keys::ADMIN_MATRIX_DROPDOWN,
        "Dropdown (закрыт / открыт)",
    ),
    (keys::ADMIN_MATRIX_TOOLTIP, "Tooltip (delay пройден)"),
    (keys::ADMIN_MATRIX_TOAST, "Toast"),
    (
        keys::ADMIN_MATRIX_LIST,
        "Строки списка (обычное / hover / выбор + скролл)",
    ),
    (
        keys::ADMIN_MATRIX_ROWS,
        "Строки таблицы (зебра / фокус / выбор)",
    ),
    (keys::ADMIN_NODE_TABLE, "Таблица в теле (FR-061)"),
    (keys::DOCS_PAGE_ADMIN, "UI-консоль"),
    (keys::ONBOARDING_BACK, "Назад"),
    (keys::ONBOARDING_NEXT, "Далее"),
    (keys::ONBOARDING_DONE, "Готово"),
    (keys::ONBOARDING_SKIP, "Пропустить"),
    (
        keys::ONBOARDING_STEP1_TITLE,
        "Добро пожаловать в CanvasDesk",
    ),
    // CR-031/S3: value-приветствие по роли, выбранной при входе (FR-087);
    // без роли — универсальное описание ценности
    (
        keys::ONBOARDING_VALUE_DEFAULT,
        "CanvasDesk — бесконечный зумируемый канвас, где заметки, файлы и расчёты живут рядом: собирайте идеи в схемы, а схемы считаются сами.",
    ),
    (
        keys::ONBOARDING_VALUE_ARCHITECT,
        "Проектируйте архитектуру на живом канвасе: сервисы, очереди и базы — готовые ноды-шаблоны, связи показывают зависимости, а расчёты с единицами измеряют нагрузки и стоимость.",
    ),
    (
        keys::ONBOARDING_VALUE_DEVELOPER,
        "Архитектура и потоки данных всегда перед глазами: ноды-сервисы, связи-зависимости и живые расчёты единиц (ms, MB/s) — вместо статичных диаграмм.",
    ),
    (
        keys::ONBOARDING_VALUE_PRODUCT,
        "Сценарии продукта и unit-экономика считаются сами: формулы прямо в заметках, what-if-подмены отвечают на «а что если», схемой легко поделиться.",
    ),
    (
        keys::ONBOARDING_VALUE_ANALYST,
        "Разбирайте процессы и метрики на одном канвасе: ноды-этапы, связи-потоки, расчёты конверсий и unit-экономики с единицами измерения.",
    ),
    (
        keys::ONBOARDING_VALUE_CIO,
        "ИТ-ландшафт как живая схема: системы и интеграции — на канвасе, расчёты стоимости и нагрузок — рядом с диаграммой, изменения — в два клика.",
    ),
    (
        keys::ONBOARDING_VALUE_CTO,
        "Технологическая карта продукта вместо статичных слайдов: архитектура, потоки данных и узкие места — на одном канвасе с живыми расчётами.",
    ),
    (
        keys::ONBOARDING_VALUE_FOUNDER,
        "От идеи к работающей схеме: unit-экономика, воронки и архитектура MVP считаются сами, меняются в два клика и сохраняются в файл.",
    ),
    (keys::ONBOARDING_STEP2_TITLE, "Заметки"),
    (
        keys::ONBOARDING_STEP2_BODY,
        "Двойной клик по пустому месту создаёт заметку — фиксируйте мысль, пока она не потерялась. Цвет и действия — через контекстное меню.",
    ),
    (keys::ONBOARDING_STEP3_TITLE, "Связи"),
    (
        keys::ONBOARDING_STEP3_BODY,
        "Наведите курсор на ноду — по краям появятся порты. Протяните от порта к порту: обычная (logical) связь показывает отношение «А связано с Б», а data-связь передаёт значение между нодами.",
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
        "Свяжите формулу с другой нодой data-связью (с зажатым Shift): значение придёт во вход $in и пересчитается на живую — цепочка всегда актуальна.",
    ),
    (keys::ONBOARDING_STEP7_TITLE, "Шаблоны нод"),
    (
        keys::ONBOARDING_STEP7_BODY,
        "Ctrl+P открывает палитру готовых нод — сервис, очередь, база данных — с параметрами и доменными типами. Shift+клик по пустому месту — то же колесом.",
    ),
    (keys::ONBOARDING_STEP8_TITLE, "Что дальше"),
    (
        keys::ONBOARDING_STEP8_BODY,
        "Готово! Дальше — на выбор: открыть готовую схему из галереи шаблонов и посмотреть, как всё устроено, или начать собирать свою с чистого холста. Документация и повтор тура — в меню «?», список горячих клавиш — F1.",
    ),
    // CR-031: финальные CTA-опции последнего шага тура
    (keys::ONBOARDING_FINAL_OPEN, "Открыть шаблонную схему"),
    (keys::ONBOARDING_FINAL_EMPTY, "Начать самому"),
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
    // Владелец 2026-10-02: пометка строк документации в глобальном поиске
    (keys::SEARCH_DOCS_SECTION, "Документация"),
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
    (keys::WHATIF_FREEZE, "❄ Заморозить"),
    (keys::WHATIF_UNFREEZE, "Разморозить"),
    (keys::WHATIF_FROZEN_TOAST, "Снимок заморожен: {name} — правки канваса его не сдвинут"),
    (keys::WHATIF_UNFROZEN_TOAST, "Заморозка снята: {name}"),
    (keys::WHATIF_ROW_TOTAL, "итог"),
    (keys::WHATIF_MORE, "… ещё {count}"),
    (
        keys::WHATIF_OVERRIDE_ROW,
        "{node} → стр. {line}: {was} → {became}",
    ),
    (
        keys::DIALOG_WHATIF_RESET_TITLE,
        "Сбросить все what-if сценарии?",
    ),
    (
        keys::DIALOG_WHATIF_RESET_BODY,
        "Сценариев: {scenarios}, подмен: {overrides}. Заморозки сравнения будут сняты. Текст нод не изменится; действие можно отменить (Ctrl+Z).",
    ),
    (keys::DIALOG_WHATIF_RESET_YES, "Сбросить"),
    (
        keys::TOAST_WHATIF_RESET_DONE,
        "What-if сброшен: сценариев удалено — {count}",
    ),
    (
        keys::DIALOG_WHATIF_DELETE_TITLE,
        "Удалить сценарий «{name}»?",
    ),
    (
        keys::DIALOG_WHATIF_DELETE_BODY,
        "Подмен сценария: {count}. Заморозка одноимённого снимка будет снята. Текст нод не изменится; действие можно отменить (Ctrl+Z).",
    ),
    (keys::DIALOG_WHATIF_DELETE_YES, "Удалить"),
    (
        keys::TOAST_WHATIF_DELETED,
        "Сценарий «{name}» удалён (подмен: {count})",
    ),
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
    (keys::PAL_GROUP_BUNDLE, "Пучок"),
    (keys::PAL_ACTION_DELETE_BUNDLE, "Удалить пучок"),
    // --- Тосты ---
    (keys::TOAST_PORT_PINNED, "Порт связи закреплён"),
    (keys::TOAST_PORT_AUTO, "Порты связи: авто (кратчайший путь)"),
    (keys::TOAST_PORT_FREED, "Порт освобождён: кратчайший путь"),
    (keys::TOAST_FLOW_CYCLE, "Цикл потока: {participants} — тогл отклонён"),
    (keys::TOAST_TEMPLATE_SAVED, "Шаблон «{name}» сохранён: {path}"),
    (keys::TOAST_TEMPLATE_SAVE_FAILED, "Не удалось сохранить шаблон: {err}"),
    // FR-076: экспорт HTML-артефакта
    (keys::TOAST_EXPORT_HTML_OK, "HTML-артефакт сохранён: {path}"),
    (keys::TOAST_EXPORT_HTML_FAIL, "Не удалось записать HTML-артефакт: {err}"),
    // FR-078: deep-link ?focus=
    (keys::TOAST_FOCUS_NOT_FOUND, "Нода не найдена: {id} (ссылка ?focus=)"),
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
    (keys::TOAST_LANGUAGE_TOGGLED, "Язык интерфейса: {lang}"),
    (keys::TOAST_ROLE_CHANGED, "Роль: {role}"),
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
    // FR-044 Р-4/Р-5/Р-8: панель «Как считается» и подсветка зависимостей
    (keys::STAGE_CALC_VARS, "Переменные · входящие значения"),
    (keys::STAGE_CALC_FORMULAS, "Расчёт · формулы"),
    (keys::STAGE_CALC_UNMAPPED, "не подставлено"),
    (keys::STAGE_CALC_EXT, "+{n} внешн. вход(а/ов)"),
    (keys::STAGE_CALC_MORE, "… ещё {n}"),
    // FR-044 Р-3-а: лейблы слотов и control-рёбра; Q2: индикаторы окна
    (keys::STAGE_OUT_LABEL, "out: {name}"),
    (keys::STAGE_CTRL_LABEL, "управление"),
    (keys::STAGE_CTRL_TO, "to: {node}"),
    (keys::STAGE_PILL_ABOVE, "↑ ещё {n}"),
    (keys::STAGE_PILL_BELOW, "ещё {n} ↓"),
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
    // X3 (AC-4.1): кнопка подмены листа и подсказка inline-поля
    (keys::EXPLAIN_EDIT, "Изменить"),
    (keys::EXPLAIN_EDIT_HINT, "Новое значение — Enter применит"),
    // X5 (AC-6.1–6.4): режим защиты — состояние окна проверки
    (keys::EXPLAIN_DEFENSE, "Режим защиты"),
    (keys::EXPLAIN_DEFENSE_EXIT, "Обычный вид"),
    (keys::EXPLAIN_DEFENSE_STEP, "Раскрыть уровень"),
    (keys::EXPLAIN_DEFENSE_ALL, "Раскрыть всё"),
    (
        keys::EXPLAIN_DEFENSE_HINT,
        "Пробел/клик по узлу — следующий уровень · Esc — обычный вид",
    ),
    // FR-083: тумблер направления (глиф потока) и подсказка переполнения
    (keys::EXPLAIN_DIR_LTR, "→"),
    (keys::EXPLAIN_DIR_RTL, "←"),
    (
        keys::EXPLAIN_OVERFLOW_HINT,
        "Не всё влезло — тяните фон или крутите колесо",
    ),
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
    // --- PRD-0007 (FR-048 X4): автосвязь по именам (F-7) ---
    (keys::MENU_AUTOLINK_FIND, "Найти связи по именам"),
    (keys::AUTOLINK_BADGE, "Связи по именам · {n}"),
    (keys::AUTOLINK_TITLE, "Ревью автосвязи"),
    (
        keys::AUTOLINK_META,
        "Найдено предложений: {n} · группировка: пара нод · сортировка: по имени переменной",
    ),
    (
        keys::AUTOLINK_EMPTY,
        "Предложений нет: совпадений имён присваиваний не найдено.",
    ),
    (
        keys::AUTOLINK_BANNER,
        "Отклонено: {n}. До закрытия диалога можно вернуть одним кликом.",
    ),
    (keys::AUTOLINK_ACCEPT, "Принять"),
    (keys::AUTOLINK_REJECT, "Отклонить"),
    (keys::AUTOLINK_ACCEPT_ALL, "Принять все"),
    (keys::AUTOLINK_REJECT_ALL, "Отклонить все"),
    (keys::AUTOLINK_RESTORE_ALL, "Вернуть все"),
    (keys::AUTOLINK_CREATE, "Создать связи ({n})"),
    (
        keys::AUTOLINK_HINT,
        "Создание — одним undo-батом; откат пачки — с подтверждением и подсветкой отменяемого (AC-5.3).",
    ),
    (keys::AUTOLINK_PARAM, "параметр {name}"),
    (
        keys::AUTOLINK_TOAST_CREATED,
        "Создано связей: {n} — один undo-шаг.",
    ),
    (
        keys::AUTOLINK_TOAST_NONE,
        "Совпадений имён не найдено — предложений нет.",
    ),
    (keys::ROW_AUTOLINK, "Автосвязь по именам (фон)"),
    // FR-079 (S3): таб «Подсказки»
    // CR-022: формулировка тумблера уточнена — ИИ-строки в попапе сняты
    // (c1_in_popup=false), тумблер управляет C3-карточками после создания ноды
    (keys::ROW_SUGGEST_ENABLED, "ИИ-карточки шаблонов (после создания ноды)"),
    (
        keys::DESC_SUGGEST_ENABLED,
        "Карточки «что дальше» у новой шаблонной ноды; попап при вводе — только L0-подсказки",
    ),
    (keys::ROW_SUGGEST_ENGINE, "Движок"),
    (keys::DESC_SUGGEST_ENGINE, "Локальная лексика или гибрид с Laya (sidecar)"),
    (keys::SUGGEST_ENGINE_LEX, "Лексика (локально)"),
    (keys::SUGGEST_ENGINE_LEXLAYA, "Гибрид lex+Laya"),
    (keys::SUGGEST_ENGINE_OFF, "Выключен"),
    (
        keys::DESC_AUTOLINK,
        "Фоновый детектор предлагает связи по совпадающим именам присваиваний; связь создаётся только после ревью.",
    ),
    (keys::ROW_EXPLAIN_COVERAGE, "Индикатор покрытия цепочками"),
    (
        keys::DESC_EXPLAIN_COVERAGE,
        "Показывает «Цепочки: N%» в углу канваса — долю вычисляемых цифр, чья цепочка доходит до листьев.",
    ),
    (keys::EXPLAIN_COVERAGE, "Цепочки: {n}%"),
    // FR-ICONS: набор иконок UI — RU
    (keys::ROW_ICON_STYLE, "Набор иконок"),
    (
        keys::DESC_ICON_STYLE,
        "Монохромные SVG-иконки взамен Unicode-глифов. «Глифы» — системный фолбэк (шрифт NotoSansDisplay).",
    ),
    (keys::ICON_STYLE_GLYPH, "Глифы (системный)"),
    (keys::ICON_STYLE_LUCIDE, "Lucide"),
    (keys::ICON_STYLE_MATERIAL, "Material Symbols"),
    (keys::ICON_STYLE_FEATHER, "Feather"),
    (keys::ICON_STYLE_BOOTSTRAP, "Bootstrap Icons"),
    (keys::AUTOLINK_UNDO_TITLE, "Откатить пачку автосвязи ({n})?"),
    (
        keys::AUTOLINK_UNDO_BODY,
        "Связи пачки подсвечены на канвасе. Откат — один undo-шаг.",
    ),
    (keys::AUTOLINK_UNDO_YES, "Откатить"),
    // --- FR-LLM-B / PRD-0010 F-7: таб настроек «AI and Models» (RU) ---
    (keys::TAB_AI, "AI и модели"),
    (keys::AI_GRP_PROVIDER, "Per-feature провайдер (Q2)"),
    (keys::AI_GRP_BYOK, "BYOK — API ключ и модель"),
    (keys::AI_GRP_RATE, "Rate limit подписки ChatGPT (F-7.3)"),
    (keys::AI_GRP_LIMITS, "Лимиты и качество"),
    (keys::AI_GRP_RESIDENCY, "Data residency (Q5)"),
    (keys::AI_GRP_TELEMETRY, "Телеметрия (opt-in)"),
    (keys::AI_ROW_PROV_SUGGEST, "Suggest"),
    (keys::AI_ROW_PROV_GRAPH, "Graph Builder"),
    (keys::AI_ROW_PROV_AGENT, "Agent Panel"),
    (keys::AI_ROW_MODEL, "Модель"),
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а).
    (keys::AI_ROW_MODEL_SUGGEST, "Модель Suggest"),
    (keys::AI_ROW_MODEL_GRAPH, "Модель Graph Builder"),
    (keys::AI_ROW_MODEL_AGENT, "Модель Agent Panel"),
    (keys::AI_ROW_KEY, "API ключ"),
    (keys::AI_ROW_SH_URL, "Self-hosted endpoint"),
    (keys::AI_ROW_SH_KEY, "API ключ endpoint'а"),
    (keys::AI_ROW_TELEMETRY, "Анонимные события использования"),
    // FR-LLM-FIX: лейблы новых строк (API-ключ BYOK + self-hosted endpoint).
    (keys::AI_ROW_API_KEY, "BYOK — API ключ и модель"),
    (keys::AI_ROW_SELFHOST_URL, "Self-hosted endpoint"),
    (keys::AI_ROW_SELFHOST_KEY, "API ключ endpoint'а"),
    (
        keys::AI_DESC_PROV_SUGGEST,
        "подсказки шаблонов и custom-ноды",
    ),
    (
        keys::AI_DESC_PROV_GRAPH,
        "генерация графов из текста",
    ),
    (
        keys::AI_DESC_PROV_AGENT,
        "агентные операции через MCP",
    ),
    (
        keys::AI_DESC_FALLBACK,
        "Fallback-цепочка при ошибке/429: ChatGPT OAuth → BYOK → lex (suggest) / отказ (graph, agent). Ключи хранятся в OS keychain (desktop) / OPFS encrypted (web) — не в config.",
    ),
    (
        keys::AI_DESC_KEY,
        "BYOK-ключ провайдера (OpenAI/Anthropic/z.ai/Moonshot/OpenRouter). Хранится в OS keychain (desktop) / OPFS encrypted (web).",
    ),
    (
        keys::AI_DESC_MODEL,
        "Список /v1/models провайдера. Загружается после health-check ключа — до проверки список пуст, выберите «Проверить ключ» выше.",
    ),
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а).
    (
        keys::AI_DESC_MODEL_SUGGEST,
        "BYOK-модель для Suggest. Список подгружается из /v1/models после health-check ключа — до проверки выберите «Проверить ключ» выше.",
    ),
    (
        keys::AI_DESC_MODEL_GRAPH,
        "BYOK-модель для Graph Builder. Список подгружается из /v1/models после health-check ключа.",
    ),
    (
        keys::AI_DESC_MODEL_AGENT,
        "BYOK-модель для Agent Panel. Список подгружается из /v1/models после health-check ключа.",
    ),
    (
        keys::AI_DESC_RATE,
        "локальный счётчик; при 0 — fallback на BYOK, тост «ChatGPT недоступен»",
    ),
    (
        keys::AI_DESC_COST_LIMIT,
        "80% лимита — диалог расширения; 100% — LLM-запросы отклоняются, работает lex (F-7.4, Q7)",
    ),
    (
        keys::AI_DESC_CONF_THRESHOLD,
        "варианты с confidence ниже порога скрываются (F-2.11); влияет на ghost-ноды suggest",
    ),
    (
        keys::AI_DESC_RESIDENCY,
        "Определяет, какие провайдеры доступны и применяется ли redact контекста (Q1+Q5).",
    ),
    (
        keys::AI_DESC_TELEMETRY,
        "default OFF (Q7): события показов/принятий подсказок и cost — только после явного согласия; без PII. Полный гайд: user-docs/ai-features.md",
    ),
    // FR-LLM-FIX: описания новых строк (API-ключ BYOK + self-hosted endpoint).
    (
        keys::AI_DESC_API_KEY,
        "BYOK-ключ провайдера (OpenAI/Anthropic/z.ai/Moonshot/OpenRouter). Хранится в OS keychain (desktop) / OPFS encrypted (web). Нажмите «Проверить ключ» — после проверки подгрузится список моделей из /v1/models.",
    ),
    (
        keys::AI_DESC_SELFHOST_URL,
        "URL OpenAI-compatible endpoint'а вашего GPU-сервера (например, https://llm.corp.local/v1). Проверка — кнопка «Проверить» в строке ключа ниже.",
    ),
    (
        keys::AI_DESC_SELFHOST_KEY,
        "API-ключ self-hosted endpoint'а (если endpoint требует авторизацию). Кнопка «Проверить» делает health-check /v1/models и фиксирует бейдж.",
    ),
    // FR-LLM-FIX: плейсхолдеры dropdown'а BYOK-модели (когда список пуст).
    (keys::AI_MODEL_PH_NO_KEY, "Введите API ключ"),
    (
        keys::AI_MODEL_PH_NO_HC,
        "Нажмите «Проверить ключ» для загрузки списка",
    ),
    (keys::AI_MODEL_PH_EMPTY, "Выберите модель"),
    // FR-LLM-FIX: бейдж статусной панели — активный провайдер без модели.
    (keys::AI_STATUS_MODEL_NOT_CHOSEN, "не выбрана"),
    // FR-LLM-FIX-2: короткие подписи провайдеров — чип статусной панели
    // (как aiProvLabel() прототипа: BYOK / ChatGPT OAuth / Ollama / …).
    (keys::AI_PROV_SHORT_OFF, "Выключено"),
    (keys::AI_PROV_SHORT_LAYA, "Laya (local)"),
    (keys::AI_PROV_SHORT_OLLAMA, "Ollama"),
    (keys::AI_PROV_SHORT_BYOK, "BYOK"),
    (keys::AI_PROV_SHORT_CHATGPT, "ChatGPT OAuth"),
    (keys::AI_OPT_OFF, "Выключено"),
    (keys::AI_OPT_LAYA, "Laya (локальная)"),
    (keys::AI_OPT_OLLAMA, "Ollama (локальная)"),
    (keys::AI_OPT_BYOK, "BYOK (свой ключ)"),
    (keys::AI_OPT_CHATGPT, "ChatGPT (вход)"),
    (
        keys::AI_OPT_CHATGPT_DISABLED,
        "ChatGPT OAuth — недоступен для Suggest (Q2)",
    ),
    (
        keys::AI_RESIDENCY_LOCAL,
        "Local only (Laya / Ollama)",
    ),
    (
        keys::AI_RESIDENCY_CLOUD,
        "Cloud (ChatGPT / BYOK)",
    ),
    (
        keys::AI_RESIDENCY_SELFHOST,
        "Self-hosted (ваш GPU-сервер)",
    ),
    (
        keys::AI_RESIDENCY_LOCAL_DESC,
        "Работает offline · данные не покидают машину · качество ниже cloud",
    ),
    (
        keys::AI_RESIDENCY_CLOUD_DESC,
        "Лучшее качество · контекст уходит провайдеру (redact: price=<redacted>) · дневной лимит cost",
    ),
    (
        keys::AI_RESIDENCY_SELFHOST_DESC,
        "Cloud-качество · данные остаются в вашем контуре · нужен endpoint (ответственность пользователя)",
    ),
    (keys::AI_BTN_CHECK_KEY, "Проверить ключ"),
    (keys::AI_BTN_CHECK, "Проверить"),
    (
        keys::AI_KEY_VALID,
        "ключ валиден · {n} моделей",
    ),
    (keys::AI_KEY_NOT_CHECKED, "ключ не проверен"),
    (keys::AI_KEY_EMPTY, "ключ пустой — введите ключ"),
    (keys::AI_SH_NOT_CHECKED, "не проверен"),
    (
        keys::AI_SH_OK,
        "endpoint отвечает · OpenAI-compatible",
    ),
    (keys::AI_SH_BAD_URL, "неверный URL"),
    (
        keys::AI_COST_LIMIT_LABEL,
        "$ {v} / день",
    ),
    (keys::AI_CONF_THR_LABEL, "{v}"),
    (
        keys::AI_FOOT_HINT,
        "Изменения применяются сразу · Esc — закрыть",
    ),
    (
        keys::AI_RATE_BADGE,
        "ChatGPT: {left}/{total} сообщений осталось",
    ),
    // --- FR-LLM-OAUTH-APP / PRD-0010 F-5.8: блок «Вход ChatGPT» (RU) ---
    (keys::AI_OAUTH_ROW, "Вход ChatGPT"),
    (
        keys::AI_OAUTH_DESC,
        "OAuth-вход по подписке ChatGPT — для Graph Builder и Agent Panel. Токены хранятся локально (0600), не в config.toml.",
    ),
    (keys::AI_OAUTH_BTN_LOGIN, "Войти через ChatGPT"),
    (
        keys::AI_OAUTH_HINT_BROWSER,
        "откроется браузер для подтверждения входа",
    ),
    (keys::AI_OAUTH_BTN_CANCEL, "Отменить"),
    (
        keys::AI_OAUTH_WAITING,
        "Ожидаю подтверждение в браузере…",
    ),
    (keys::AI_OAUTH_CONNECTED, "вход выполнен · {email}"),
    (keys::AI_OAUTH_ACCOUNT, "аккаунт"),
    (keys::AI_OAUTH_BTN_LOGOUT, "Выйти"),
    (keys::AI_OAUTH_ERR, "ошибка входа: {error}"),
    (keys::AI_OAUTH_BTN_RETRY, "Повторить"),
    (
        keys::AI_OAUTH_UNAVAILABLE,
        "OAuth-вход доступен только в нативной сборке с feature l1-llm-tls",
    ),
    (keys::AI_OAUTH_TOAST_LOGOUT, "Выход выполнен — токены удалены"),
    // --- W2: health/discovery/graph тосты (RU) ---
    (keys::AI_HEALTH_OK, "Ключ валиден — провайдер отвечает"),
    (keys::AI_HEALTH_FAIL, "Проверка не прошла"),
    (
        keys::AI_HEALTH_NO_KEY,
        "Ключ не задан — введите API-ключ в Настройках AI (9-й таб)",
    ),
    (
        keys::AI_HEALTH_NO_ENDPOINT,
        "Не задан self-hosted URL — укажите endpoint в Настройках AI",
    ),
    (keys::AI_DISCOVERY_OK, "Модели загружены: {n}"),
    (keys::AI_GRAPH_DONE, "Граф сгенерирован: нод {n}"),
    (keys::AI_MENU_GRAPH_BUILDER, "Генератор графа (AI)…"),
    // --- FR-LLM-B / PRD-0010 F-7.9: AI status panel (RU) ---
    (keys::AI_STATUS_PAUSED, "AI на паузе — нажмите ▶, чтобы возобновить"),
    (keys::AI_STATUS_SESSION, "Session: {v}"),
    (keys::AI_STATUS_DAY, "Day: {v} / {lim}"),
    (keys::AI_STATUS_NO_MODEL, "AI выключен"),
    (keys::AI_TOOLTIP_PAUSE, "Пауза всех AI-функций (⏸/▶)"),
    (
        keys::AI_TOOLTIP_SETTINGS,
        "Открыть настройки AI (9-й таб «AI и модели»)",
    ),
    (
        keys::AI_TOOLTIP_SUGGEST,
        "Suggest: LLM-подсказки шаблонов и custom-ноды. Клик — вкл/выкл",
    ),
    (
        keys::AI_TOOLTIP_GRAPH,
        "Graph Builder: генерация графов из текста. Клик — вкл/выкл",
    ),
    (
        keys::AI_TOOLTIP_AGENT,
        "Agent Panel: агентные операции через MCP. Клик — вкл/выкл",
    ),
    // --- FR-LLM-B / PRD-0010 F-8: AI onboarding screen (RU) ---
    (keys::AI_ONB_STEP, "Шаг 3 из 3 · AI-режим"),
    (keys::AI_ONB_TITLE, "Выберите режим AI"),
    (
        keys::AI_ONB_SUB,
        "CanvasDesk работает с любым режимом — поменять можно позже в настройках (9-й таб «AI и модели»).",
    ),
    (keys::AI_ONB_LOCAL_TITLE, "Local only (Laya / Ollama)"),
    (keys::AI_ONB_CLOUD_TITLE, "Cloud (ChatGPT / BYOK)"),
    (keys::AI_ONB_SELFHOST_TITLE, "Self-hosted (ваш GPU-сервер)"),
    (
        keys::AI_ONB_LOCAL_DESC,
        "Работает offline · данные не уходят с машины · подсказки ниже cloud-качества, graph/agent — через Ollama на вашей машине.",
    ),
    (
        keys::AI_ONB_CLOUD_DESC,
        "Лучшее качество · контекст уходит провайдеру (числа маскируются: price=<redacted>) · дневной лимит $1.00 по умолчанию.",
    ),
    (
        keys::AI_ONB_SELFHOST_DESC,
        "Cloud-качество · данные остаются в вашем контуре · нужен OpenAI-compatible endpoint (Настройки → AI и модели).",
    ),
    (keys::AI_ONB_TAG_OFFLINE, "offline"),
    (keys::AI_ONB_TAG_QUALITY, "лучшее качество"),
    (keys::AI_ONB_TAG_CONTOUR, "в контуре"),
    (keys::AI_ONB_PRIV, "Подробнее о privacy"),
    (keys::AI_ONB_CONTINUE, "Продолжить"),
    (
        keys::AI_ONB_PRIV_TITLE,
        "Что отправляется.",
    ),
    (
        keys::AI_ONB_PRIV_BODY,
        "Только контекст редактируемой ноды и её соседей; числа маскируются явным маркером (price=<redacted> руб — единица сохраняется, Q1). Сами канвасы, файлы и ключи — не отправляются никогда.",
    ),
    (
        keys::AI_ONB_PRIV_KEYS,
        "Где живут ключи.",
    ),
    (
        keys::AI_ONB_PRIV_KEYS_BODY,
        "BYOK-ключи — в OS keychain (desktop) / OPFS encrypted (web); OAuth-токены ChatGPT — тоже локально, бэкенд не участвует.",
    ),
    (
        keys::AI_ONB_PRIV_TELEMETRY,
        "Телеметрия.",
    ),
    (
        keys::AI_ONB_PRIV_TELEMETRY_BODY,
        "Отключена по умолчанию; включается только явным opt-in в настройках.",
    ),
    (
        keys::AI_ONB_PRIV_DOCS,
        "Полный гайд — user-docs/ai-features.md (в продукте открывается из этого экрана).",
    ),
    (keys::AI_ONB_TOAST_LOCAL, "AI-режим: Local only — offline, данные не уходят"),
    (
        keys::AI_ONB_TOAST_CLOUD,
        "AI-режим: Cloud — качество выше, данные у провайдеру (redact)",
    ),
    (
        keys::AI_ONB_TOAST_SELFHOST,
        "AI-режим: Self-hosted — cloud-качество в вашем контуре",
    ),
];

/// Английская таблица — полный перевод каждого ключа (инвариант полноты).
const EN: &[(&str, &str)] = &[
    // --- Settings modal ---
    (keys::SETTINGS_TITLE, "Settings"),
    (keys::SETTINGS_HINT, "Ctrl+, — open/close"),
    (keys::SETTINGS_DESC_MORE, "{n} more"),
    (keys::TAB_GENERAL, "General"),
    (keys::TAB_CANVAS, "Canvas"),
    (keys::TAB_DRAG, "Drag"),
    (keys::ROW_DRAG_PUSH_ENABLED, "Push-apart while dragging"),
    (keys::ROW_DRAG_PUSH_GAP, "Safe gap"),
    (keys::ROW_DRAG_PUSH_HALO, "Active-node halo"),
    (keys::ROW_DRAG_PUSH_PREDICTIVE, "Predictive lead"),
    (keys::ROW_DRAG_PUSH_REBASE, "Re-anchor on drop"),
    (keys::TAB_EDGES, "Edges & ports"),
    (keys::TAB_APPEARANCE, "Appearance"),
    (keys::TAB_SUGGEST, "Suggestions"),
    (keys::TAB_PROFILE, "Profile"),
    (keys::ROW_ROLE, "Role"),
    (keys::DESC_ROLE, "Your work role filters template and scheme hints: \"Not selected\" shows everything, a role shows only matching types. The toggles below override the role."),
    (keys::ROW_TPLCAT_BACKEND, "Templates: Backend"),
    (keys::ROW_TPLCAT_NETWORK, "Templates: Network"),
    (keys::ROW_TPLCAT_UNIT_ECONOMICS, "Templates: Unit Economics"),
    (keys::ROW_TPLCAT_PRODUCT_ANALYTICS, "Templates: Product Analytics"),
    (keys::DESC_TPLCAT, "Show template nodes of this category in the palette (Ctrl+P) and the wheel menu."),
    (keys::ROW_SCHEMECAT_ARCHITECTURE, "Schemes: Architecture"),
    (keys::ROW_SCHEMECAT_BUSINESS, "Schemes: Business"),
    (keys::ROW_SCHEMECAT_FRAMEWORK, "Schemes: Frameworks"),
    (keys::ROW_SCHEMECAT_PLANNING, "Schemes: Planning"),
    (keys::ROW_SCHEMECAT_ONBOARDING, "Schemes: Onboarding"),
    (keys::DESC_SCHEMECAT, "Show template schemes of this category in the gallery (Ctrl+T)."),
    (keys::ROW_TELEMETRY_COUNTER, "Anonymous usage counter"),
    (
        keys::DESC_TELEMETRY_COUNTER,
        "One note per day, no personal data — how many people use the app. Works even with metrics off.",
    ),
    (keys::ROW_TELEMETRY_ANALYTICS, "Metrics & error reports"),
    (
        keys::DESC_TELEMETRY_ANALYTICS,
        "Helps find bugs and improve CanvasDesk: UI events and error reports (PostHog), no personal data.",
    ),
    (keys::TAB_SNAP, "Snapping"),
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
    (
        keys::DESC_DRAG_PUSH_ENABLED,
        "Neighbors yield to the dragged node and slide back; covered ones get new anchors. Off — legacy pass-through behavior.",
    ),
    (
        keys::DESC_DRAG_PUSH_GAP,
        "Minimum distance between borders of any two nodes, scene px. 0 — flush (legacy). Zoom-independent.",
    ),
    (
        keys::DESC_DRAG_PUSH_HALO,
        "Exclusion zone around the dragged node on top of the safe gap, scene px. Active clearance = halo + gap.",
    ),
    (
        keys::DESC_DRAG_PUSH_PREDICTIVE,
        "The halo extends toward fast cursor movement — neighbors prepare in advance.",
    ),
    (
        keys::DESC_DRAG_PUSH_REBASE,
        "A node whose place is taken by the dragged one anchors at the displaced position. Off — everything slides back.",
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
    (keys::MENU_AUTOWIDTH, "Auto-width by content"),
    (keys::MENU_ABOUT, "About the author"),
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
    (keys::HKEY_F2, "F2"),
    (keys::HKEY_RMB, "RMB"),
    (keys::HKEY_SPACE_DRAG, "Space+drag"),
    (keys::HKEY_CTRL_WHEEL, "Ctrl+wheel"),
    (keys::HKEY_CTRL_ENTER, "Ctrl+Enter"),
    (keys::HKEY_CTRL_COMMA, "Ctrl+,"),
    (keys::HKEY_CTRL_SHIFT_I, "Ctrl+Shift+I"),
    (keys::HKEY_F, "F"),
    (keys::HKEY_SPACE, "Space"),
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
    (keys::HK_F2, "edit selected node"),
    (keys::HK_CONTEXT_MENU, "object menu"),
    (keys::HK_PAN, "panning"),
    (keys::HK_ZOOM, "zoom"),
    (keys::HK_COMMIT_NOTE, "commit note"),
    (keys::HK_SETTINGS, "settings"),
    (keys::HK_WHATIF, "what-if scenarios"),
    (keys::HK_EDGE_FOCUS, "edge focus"),
    (
        keys::HK_EXPLAIN_STEP,
        "calc-chain defense mode: reveal the next level",
    ),
    // FR-100: editor commands of a note
    (keys::HKEY_CTRL_BACKSPACE, "Ctrl+Backspace / Ctrl+Delete"),
    (keys::HKEY_PAGE_UP_DOWN, "PageUp / PageDown"),
    (keys::HKEY_TAB_INDENT, "Tab / Shift+Tab"),
    (keys::HKEY_HOME_SMART, "Home ×2"),
    (keys::HKEY_CMD_EDITOR, "Cmd+key (macOS)"),
    (keys::HK_WORD_DELETE, "in the editor: delete word left / right"),
    (
        keys::HK_EDITOR_PAGE,
        "in the editor: page of lines (Shift — extend selection)",
    ),
    (
        keys::HK_EDITOR_TAB,
        "in the editor: indent / unindent lines",
    ),
    (
        keys::HK_EDITOR_HOME,
        "in the editor: first non-space of the line, again — line start",
    ),
    (keys::HK_CMD_PARITY, "editor commands — same as Ctrl"),
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
    (keys::TOOLTIP_PORT_UNMAPPED, "not mapped"),
    (keys::TOOLTIP_PORT_MORE, "+{n} more"),
    (
        keys::TOOLTIP_UNMAPPED_SLOT,
        "Value not applied: the edge exists but the source yielded no value (source line deleted / became prose / column missing). Connect a node with an up-to-date value or fix the source line.",
    ),
    (
        keys::TOOLTIP_PORT_PARAM,
        "to: {path}",
    ),
    (
        keys::TOOLTIP_UNMAPPED_PARAM,
        "Parameter {param} gets no value: output {output} is missing on node \"{node}\" or yielded no value. Rebind the edge to an existing output (click the edge → rebind) or connect a node with an up-to-date value.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM,
        "Spilled: {path} = {value} (local was: {local}). To edit the value, disconnect the spill (line context menu or delete the edge); Ctrl+Z restores it.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM_NO_LOCAL,
        "Spilled: {path} = {value}. There was no local value — removing the edge leaves the parameter empty.",
    ),
    (
        keys::TOOLTIP_SPILL_PARAM_NOVALUE,
        "Spilled from {path}: the source yielded no value (source line deleted / became prose / output missing). Fix the source line or rebind the edge.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_TPL,
        "Spilled: {path} = {value} — input ${slot} arrives at the node, but the template formula does not read it. Connect the edge to a parameter (drag onto the parameter anchor, toParam) to feed the calculation.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_TEXT,
        "Spilled: {path} = {value} — input ${slot} arrives at the node, but the formula does not read it. Use ${slot} in the formula (or the named path {path}) to feed the calculation.",
    ),
    (
        keys::TOOLTIP_SPILL_AUTOROW_NOVALUE,
        "{path}: value not applied — the edge exists but the source yielded no value. Connect a node with an up-to-date value or fix the source line.",
    ),
    // --- FR-050 этап E: Н9-3 контекст-меню параметра, Н9-4 карта, Н9-6 тост ---
    (
        keys::MENU_FLOW_MAP,
        "Spill map (Ctrl+Shift+M)",
    ),
    (keys::FLOW_MAP_TITLE, "Spills"),
    (
        keys::FLOW_MAP_EMPTY,
        "No spills — connect a value to a node parameter or input",
    ),
    (keys::FLOW_MAP_MORE, "… {n} more"),
    (keys::FLOW_MAP_INPUT, "input {n}"),
    (keys::MENU_PARAM_TITLE, "Spilled into parameter"),
    (keys::MENU_AUTOROW_TITLE, "Incoming value"),
    (keys::MENU_PARAM_SOURCE, "Show source"),
    (keys::MENU_PARAM_DISCONNECT, "Disconnect spill"),
    (keys::MENU_PARAM_WHATIF, "What if…"),
    (
        keys::TOAST_SPILL_PARAM,
        "Parameter {param} pulled from {path} — Ctrl+Z will undo",
    ),
    (
        keys::TOAST_SPILL_AUTOROW,
        "Value pulled from {path} — Ctrl+Z will undo",
    ),
    // --- Numi hints ---
    (keys::HINT_VAR, "sheet variable"),
    (keys::HINT_UNIT, "unit of measure"),
    (keys::HINT_PARAM, "template parameter"),
    (keys::HINT_DOLLAR_IN, "value-edge input (FR-014)"),
    (keys::HINT_AI_DETAIL, "AI · replaces node with template"),
    (keys::HINT_AI_DETAIL_FUSION, "AI+ · replaces node with template"),
    (keys::HINT_DOLLAR_N, "input #{i}"),
    (keys::HINT_SPILL, "spill from node \"{node}\""),
    (keys::HINT_FIELD, "field from node \"{node}\""),
    (keys::SUGGEST_EMPTY, "✦ No AI suggestions"),
    // --- Onboarding ---
    // --- Scheme gallery (FR-049) ---
    (keys::GALLERY_TITLE, "Scheme templates"),
    (keys::GALLERY_SEARCH, "Search schemes"),
    (keys::GALLERY_ALL, "All"),
    (keys::GALLERY_META, "nodes: {nodes}, edges: {edges}"),
    (keys::GALLERY_FOOTER, "Enter — open · Esc — close"),
    (keys::GALLERY_EMPTY_TITLE, "Start from a template"),
    (
        keys::GALLERY_EMPTY_BODY,
        "Ready-made diagrams with connections and calculations: open one and change the numbers — everything recalculates live.",
    ),
    (keys::GALLERY_EMPTY_OPEN, "Open gallery"),
    (keys::GALLERY_EMPTY_DISMISS, "Blank canvas"),
    (keys::GALLERY_EMPTY_MORE, "{n} more"),
    (
        keys::GALLERY_APPLIED,
        "Scheme «{name}» added — one Ctrl+Z undoes it",
    ),
    (
        keys::GALLERY_UNKNOWN,
        "Scheme not found: {id} — check the ?template parameter",
    ),
    (keys::HELP_SCHEMES, "Scheme gallery"),
    (keys::HELP_INTERFACE, "About the interface"),
    (keys::KIT_GALLERY_TITLE, "Interface showcase (UI kit v1)"),
    (keys::KIT_GALLERY_THEME, "Toggle theme"),
    (keys::KIT_BTN_PRIMARY, "Primary"),
    (keys::KIT_BTN_SECONDARY, "Secondary"),
    (keys::KIT_BTN_GHOST, "Ghost"),
    (keys::KIT_BTN_DANGER, "Danger"),
    (keys::KIT_DROPDOWN_ANCHOR, "Dropdown"),
    (keys::KIT_DROPDOWN_ITEM, "List item"),
    (keys::KIT_TOAST_BODY, "Toast: bottom center, 3 s (T21-A)"),
    (keys::KIT_TOOLTIP_ANCHOR, "Hover me"),
    (
        keys::KIT_TOOLTIP_BODY,
        "Tooltip: anchor + flip when out of space + 500 ms delay",
    ),
    (keys::KIT_STATE_NORMAL, "Normal"),
    (keys::KIT_STATE_HOVER, "Hover"),
    (keys::KIT_STATE_PRESSED, "Pressed"),
    (keys::KIT_STATE_DISABLED, "Disabled"),
    (keys::KIT_STATE_SELECTED, "Selected"),
    (keys::KIT_SECTION_BUTTONS, "Buttons (Button × variants × states)"),
    (keys::KIT_SECTION_ICON, "Icon buttons (IconButton)"),
    (keys::KIT_SECTION_CHIPS, "Chips (Chip)"),
    (keys::KIT_SECTION_DROPDOWN, "Dropdown (anchor + flip)"),
    (keys::KIT_SECTION_TOAST, "Toast"),
    (keys::KIT_SECTION_TOOLTIP, "Tooltip (anchor + flip + delay)"),
    // FR-059: v2 component sections
    (
        keys::KIT_SECTION_TEXT_FIELD,
        "Text field (TextField: caret in chars)",
    ),
    (keys::KIT_SECTION_SWITCH, "Switch"),
    (keys::KIT_SECTION_CARD, "Card (header + body)"),
    (keys::KIT_SECTION_LIST, "List & scroll (list_rows + ScrollState)"),
    (keys::KIT_SECTION_ICONS, "Icons (glyphs via the existing font)"),
    // FR-062: layout v2 sections
    (
        keys::KIT_SECTION_MEASURED,
        "Measured row (F-13: widths from TextMeasurer)",
    ),
    (keys::KIT_SECTION_GROW, "Flex factors (F-14: grow 2:1 + End)"),
    (keys::KIT_SECTION_WRAP, "Row wrap (F-15: Wrap in slot)"),
    (keys::KIT_SECTION_GRID, "Grid (F-16: grid_cells 4×2)"),
    (
        keys::KIT_SECTION_FOCUS,
        "Focus (F-17: FocusRing, Tab/Shift+Tab)",
    ),
    // FR-068 W3 (M3): Table section — rows via the Table component
    // (W3.3: the only table section of the showroom)
    (
        keys::KIT_SECTION_TABLE,
        "Table (rows on shared guides)",
    ),
    (keys::KIT_ROW_PRICE, "price"),
    (keys::KIT_ROW_QTY, "qty"),
    (keys::KIT_ROW_TOTAL, "total = price × qty"),
    (keys::KIT_ROW_SUM, "receipt Σ"),
    (keys::KIT_ROW_UNIT_PRICE, "₽/pc"),
    (keys::KIT_ROW_UNIT_QTY, "pc"),
    (keys::KIT_ROW_UNIT_MONEY, "₽"),
    (keys::KIT_ROW_BADGE, "← source"),
    (keys::KIT_MEASURED_A, "Input"),
    (keys::KIT_MEASURED_B, "Formula"),
    (keys::KIT_MEASURED_C, "Result, %"),
    (keys::KIT_GROW_FIXED, "fixed"),
    (keys::KIT_GROW_TWO, "grow ×2"),
    (keys::KIT_GROW_ONE, "grow ×1"),
    (keys::KIT_WRAP_CHIP, "Chip {n}"),
    (keys::KIT_TEXTFIELD_PLACEHOLDER, "Placeholder…"),
    (keys::KIT_CARD_TITLE, "Card"),
    (
        keys::KIT_CARD_BODY,
        "Card body: header and content inside the panel pad (SPACING_LG).",
    ),
    (keys::KIT_LIST_ROW, "Row {n}"),
    (keys::KIT_STATE_FOCUSED, "Focused"),
    // Surface rebuild wave: component-layer demos + layout primitives
    (
        keys::KIT_SECTION_COMPONENT_ROW,
        "Row component (component::Row: Props + state)",
    ),
    (
        keys::KIT_SECTION_COMPONENT_PANEL,
        "Panel component (component::Panel: panel + content)",
    ),
    (
        keys::KIT_SECTION_SQUEEZE,
        "Row degradation (SqueezeTail: tail squeezed to zero)",
    ),
    (
        keys::KIT_SECTION_ALIGN,
        "Alignment and column (SpaceBetween / Column + spacer)",
    ),
    (keys::KIT_COMPONENT_ROW_LABEL, "Component row"),
    (
        keys::KIT_COMPONENT_PANEL_BODY,
        "Panel component content (panel_content — minus SPACING_LG pad)",
    ),
    (keys::KIT_ALIGN_A, "Start"),
    (keys::KIT_ALIGN_B, "Center"),
    (keys::KIT_ALIGN_C, "End"),
    (
        keys::KIT_DEBUG_HINT,
        "DebugOverlay (F9): layer rects, name under cursor, intersections",
    ),
    // --- FR-070: UI admin panel ---
    (keys::ADMIN_TITLE, "UI console"),
    (keys::ADMIN_HELP_ITEM, "UI console (design system)"),
    (keys::ADMIN_RESET, "Reset"),
    (keys::ADMIN_SECTION_COMPONENTS, "Components"),
    (keys::ADMIN_SECTION_FILL, "Fill levels"),
    (keys::ADMIN_SECTION_CANVAS, "Canvas"),
    (keys::ADMIN_SECTION_TOKENS, "Tokens"),
    (
        keys::ADMIN_HINT_COMPONENTS,
        "Kit control state matrix: normal / hover / selected / pressed / disabled + focused / error.",
    ),
    (
        keys::ADMIN_HINT_FILL,
        "Containers at different fill levels: empty / medium / full — list, card, field, chips, dropdown, rows, toasts, palette, wheel.",
    ),
    (
        keys::ADMIN_HINT_CANVAS,
        "Canvas entity states (ST4): node card, edges, ports.",
    ),
    (
        keys::ADMIN_HINT_TOKENS,
        "Design token catalog: color swatches, dimensions, typography, motion; live editing of palette slots — click a swatch, Reset in the header.",
    ),
    // FR-070 stage 2: state matrix and fill levels
    (keys::ADMIN_STATE_ERROR, "Error"),
    (keys::ADMIN_FILL_EMPTY, "Empty"),
    (keys::ADMIN_FILL_MEDIUM, "Medium"),
    (keys::ADMIN_FILL_FULL, "Full"),
    (keys::ADMIN_ROW_LIST, "List"),
    (keys::ADMIN_ROW_CARD, "Card"),
    (keys::ADMIN_ROW_FIELD, "Text field"),
    (keys::ADMIN_ROW_CHIPS, "Chips"),
    (keys::ADMIN_ROW_DROPDOWN, "Dropdown"),
    (keys::ADMIN_ROW_ROWS, "Table rows"),
    (keys::ADMIN_ROW_TOAST, "Toasts"),
    (keys::ADMIN_ROW_PALETTE, "Template palette"),
    (keys::ADMIN_ROW_WHEEL, "Wheel menu"),
    (keys::ADMIN_EMPTY_PLACEHOLDER, "No data"),
    (keys::ADMIN_CHIPS_EMPTY, "No labels"),
    (keys::ADMIN_ROWS_EMPTY, "No rows"),
    (keys::ADMIN_DROPDOWN_EMPTY, "Empty"),
    (keys::ADMIN_TOAST_EMPTY, "No toasts"),
    (keys::ADMIN_PALETTE_EMPTY, "No templates"),
    (keys::ADMIN_WHEEL_EMPTY, "No categories"),
    (keys::ADMIN_TOAST_SHORT, "Saved"),
    (keys::ADMIN_TOAST_LONG, "Scheme applied: 12 nodes updated"),
    // FR-070 stage 3: canvas entities (ST4) and token catalog
    (keys::ADMIN_NODE_NORMAL, "Normal"),
    (keys::ADMIN_NODE_SELECTED, "Selected"),
    (keys::ADMIN_NODE_BROKEN, "Broken link"),
    (keys::ADMIN_NODE_GROUP, "In group"),
    (keys::ADMIN_EDGE_DEFAULT, "Default"),
    (keys::ADMIN_EDGE_FLOW, "Value flow"),
    (keys::ADMIN_EDGE_DRAFT, "Draft (drag)"),
    (keys::ADMIN_EDGE_DIMMED, "Dimmed"),
    (keys::ADMIN_PORT_IDLE, "Dot"),
    (keys::ADMIN_PORT_HOVER, "Cursor"),
    (keys::ADMIN_PORT_ACTIVE, "Active draft"),
    (keys::ADMIN_TOK_GROUP_SLOTS, "Palette slots — click a swatch to change color"),
    (keys::ADMIN_TOK_GROUP_DIMS, "Dimensions (px)"),
    (keys::ADMIN_TOK_GROUP_TYPE, "Typography (px)"),
    (keys::ADMIN_TOK_GROUP_MOTION, "Motion (ms)"),
    // Surface rebuild wave: extended Components matrix + canvas
    (keys::ADMIN_MATRIX_DROPDOWN, "Dropdown (closed / open)"),
    (keys::ADMIN_MATRIX_TOOLTIP, "Tooltip (delay elapsed)"),
    (keys::ADMIN_MATRIX_TOAST, "Toast"),
    (
        keys::ADMIN_MATRIX_LIST,
        "List rows (normal / hover / selected + scroll)",
    ),
    (
        keys::ADMIN_MATRIX_ROWS,
        "Table rows (zebra / focus / selected)",
    ),
    (keys::ADMIN_NODE_TABLE, "Table body (FR-061)"),
    (keys::DOCS_PAGE_ADMIN, "UI console"),
    (keys::ONBOARDING_BACK, "Back"),
    (keys::ONBOARDING_NEXT, "Next"),
    (keys::ONBOARDING_DONE, "Done"),
    (keys::ONBOARDING_SKIP, "Skip"),
    (keys::ONBOARDING_STEP1_TITLE, "Welcome to CanvasDesk"),
    // CR-031/S3: role-based value welcome (FR-087); universal without a role
    (
        keys::ONBOARDING_VALUE_DEFAULT,
        "CanvasDesk is an infinite zoomable canvas where notes, files and calculations live side by side: turn ideas into schemes, and the schemes compute themselves.",
    ),
    (
        keys::ONBOARDING_VALUE_ARCHITECT,
        "Design architecture on a living canvas: services, queues and databases as ready node templates, edges show dependencies, and calculations with units measure load and cost.",
    ),
    (
        keys::ONBOARDING_VALUE_DEVELOPER,
        "Keep the architecture and data flows in sight: service nodes, dependency edges and live unit calculations (ms, MB/s) — instead of static diagrams.",
    ),
    (
        keys::ONBOARDING_VALUE_PRODUCT,
        "Product scenarios and unit economics compute themselves: formulas right in notes, what-if overrides answer “what if”, and schemes are easy to share.",
    ),
    (
        keys::ONBOARDING_VALUE_ANALYST,
        "Break down processes and metrics on one canvas: stage nodes, flow edges, conversion and unit-economics calculations with units of measure.",
    ),
    (
        keys::ONBOARDING_VALUE_CIO,
        "The IT landscape as a living scheme: systems and integrations on the canvas, cost and load calculations next to the diagram, changes in two clicks.",
    ),
    (
        keys::ONBOARDING_VALUE_CTO,
        "A technology map of your product instead of static slides: architecture, data flows and bottlenecks — on one canvas with live calculations.",
    ),
    (
        keys::ONBOARDING_VALUE_FOUNDER,
        "From idea to a working scheme: unit economics, funnels and MVP architecture compute themselves, change in two clicks and save to a file.",
    ),
    (keys::ONBOARDING_STEP2_TITLE, "Notes"),
    (
        keys::ONBOARDING_STEP2_BODY,
        "Double-click empty space to create a note — capture a thought before it slips away. Color and actions live in the context menu.",
    ),
    (keys::ONBOARDING_STEP3_TITLE, "Edges"),
    (
        keys::ONBOARDING_STEP3_BODY,
        "Hover a node — ports appear on its sides. Drag from port to port: a regular (logical) edge shows that “A relates to B”, while a data edge passes a value between nodes.",
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
        "Connect a formula to another node with a data edge (hold Shift): the value feeds the $in input and recalculates live — the chain always stays current.",
    ),
    (keys::ONBOARDING_STEP7_TITLE, "Node templates"),
    (
        keys::ONBOARDING_STEP7_BODY,
        "Ctrl+P opens the palette of ready nodes — service, queue, database — with parameters and domain types. Shift+click on empty space — the same as a radial wheel menu.",
    ),
    (keys::ONBOARDING_STEP8_TITLE, "What's next"),
    (
        keys::ONBOARDING_STEP8_BODY,
        "Done! Now choose: open a template scheme from the gallery to see how it all works, or start building your own on a blank canvas. Docs and the tour replay live in the “?” menu, the hotkey list — F1.",
    ),
    // CR-031: финальные CTA-опции последнего шага тура
    (keys::ONBOARDING_FINAL_OPEN, "Open a template scheme"),
    (keys::ONBOARDING_FINAL_EMPTY, "Start from scratch"),
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
    // Владелец 2026-10-02: пометка строк документации в глобальном поиске
    (keys::SEARCH_DOCS_SECTION, "Documentation"),
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
    (keys::WHATIF_FREEZE, "❄ Freeze"),
    (keys::WHATIF_UNFREEZE, "Unfreeze"),
    (keys::WHATIF_FROZEN_TOAST, "Snapshot frozen: {name} — canvas edits will not move it"),
    (keys::WHATIF_UNFROZEN_TOAST, "Snapshot released: {name}"),
    (keys::WHATIF_ROW_TOTAL, "total"),
    (keys::WHATIF_MORE, "… {count} more"),
    (
        keys::WHATIF_OVERRIDE_ROW,
        "{node} → line {line}: {was} → {became}",
    ),
    (
        keys::DIALOG_WHATIF_RESET_TITLE,
        "Reset all what-if scenarios?",
    ),
    (
        keys::DIALOG_WHATIF_RESET_BODY,
        "Scenarios: {scenarios}, overrides: {overrides}. Comparison snapshots will be released. Node text stays untouched; the action can be undone (Ctrl+Z).",
    ),
    (keys::DIALOG_WHATIF_RESET_YES, "Reset"),
    (
        keys::TOAST_WHATIF_RESET_DONE,
        "What-if reset: {count} scenario(s) removed",
    ),
    (
        keys::DIALOG_WHATIF_DELETE_TITLE,
        "Delete scenario \"{name}\"?",
    ),
    (
        keys::DIALOG_WHATIF_DELETE_BODY,
        "Scenario overrides: {count}. The snapshot frozen under the same name will be released. Node text stays untouched; the action can be undone (Ctrl+Z).",
    ),
    (keys::DIALOG_WHATIF_DELETE_YES, "Delete"),
    (
        keys::TOAST_WHATIF_DELETED,
        "Scenario \"{name}\" deleted (overrides: {count})",
    ),
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
    (keys::PAL_GROUP_BUNDLE, "Bundle"),
    (keys::PAL_ACTION_DELETE_BUNDLE, "Delete bundle"),
    // --- Toasts ---
    (keys::TOAST_PORT_PINNED, "Edge port pinned"),
    (keys::TOAST_PORT_AUTO, "Edge ports: auto (shortest path)"),
    (keys::TOAST_PORT_FREED, "Port freed: shortest path"),
    (keys::TOAST_FLOW_CYCLE, "Flow cycle: {participants} — toggle rejected"),
    (keys::TOAST_TEMPLATE_SAVED, "Template “{name}” saved: {path}"),
    (keys::TOAST_TEMPLATE_SAVE_FAILED, "Failed to save template: {err}"),
    // FR-076: экспорт HTML-артефакта
    (keys::TOAST_EXPORT_HTML_OK, "HTML artifact saved: {path}"),
    (keys::TOAST_EXPORT_HTML_FAIL, "Failed to write HTML artifact: {err}"),
    // FR-078: deep-link ?focus=
    (keys::TOAST_FOCUS_NOT_FOUND, "Node not found: {id} (?focus= link)"),
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
    (keys::TOAST_LANGUAGE_TOGGLED, "Interface language: {lang}"),
    (keys::TOAST_ROLE_CHANGED, "Role: {role}"),
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
    // FR-044 Р-4/Р-5/Р-8: calculation panel and dependency focus
    (keys::STAGE_CALC_VARS, "Variables · incoming values"),
    (keys::STAGE_CALC_FORMULAS, "Calculation · formulas"),
    (keys::STAGE_CALC_UNMAPPED, "not mapped"),
    (keys::STAGE_CALC_EXT, "+{n} external input(s)"),
    (keys::STAGE_CALC_MORE, "… {n} more"),
    // FR-044 Р-3-а: slot labels and control edges; Q2: window indicators
    (keys::STAGE_OUT_LABEL, "out: {name}"),
    (keys::STAGE_CTRL_LABEL, "control"),
    (keys::STAGE_CTRL_TO, "to: {node}"),
    (keys::STAGE_PILL_ABOVE, "↑ {n} more"),
    (keys::STAGE_PILL_BELOW, "{n} more ↓"),
    // --- PRD-0007 (FR-048 X2): calculation chain check window ---
    (keys::ROW_EXPLAIN_DEPTH, "Calculation chain depth"),
    (
        keys::DESC_EXPLAIN_DEPTH,
        "How many levels of the explanation tree expand automatically (0 — all).",
    ),
    (keys::VALUE_EXPLAIN_ALL, "No limit"),
    (keys::EXPLAIN_TITLE, "Calculation chain check"),
    // X3 (AC-4.1): leaf override button and inline-field hint
    (keys::EXPLAIN_EDIT, "Edit"),
    (keys::EXPLAIN_EDIT_HINT, "New value — Enter to apply"),
    // X5 (AC-6.1–6.4): defense mode — a state of the check window
    (keys::EXPLAIN_DEFENSE, "Defense mode"),
    (keys::EXPLAIN_DEFENSE_EXIT, "Normal view"),
    (keys::EXPLAIN_DEFENSE_STEP, "Reveal level"),
    (keys::EXPLAIN_DEFENSE_ALL, "Reveal all"),
    (
        keys::EXPLAIN_DEFENSE_HINT,
        "Space/click a node — next level · Esc — normal view",
    ),
    // FR-083: direction toggle (flow glyph) and overflow hint
    (keys::EXPLAIN_DIR_LTR, "→"),
    (keys::EXPLAIN_DIR_RTL, "←"),
    (
        keys::EXPLAIN_OVERFLOW_HINT,
        "Content overflows — drag the background or scroll",
    ),
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
    // --- PRD-0007 (FR-048 X4): autolink by names (F-7) ---
    (keys::MENU_AUTOLINK_FIND, "Find links by names"),
    (keys::AUTOLINK_BADGE, "Links by names · {n}"),
    (keys::AUTOLINK_TITLE, "Autolink review"),
    (
        keys::AUTOLINK_META,
        "Proposals found: {n} · grouped by node pair · sorted by variable name",
    ),
    (
        keys::AUTOLINK_EMPTY,
        "No proposals: no matching assignment names were found.",
    ),
    (
        keys::AUTOLINK_BANNER,
        "Rejected: {n}. You can restore them with one click until the dialog closes.",
    ),
    (keys::AUTOLINK_ACCEPT, "Accept"),
    (keys::AUTOLINK_REJECT, "Reject"),
    (keys::AUTOLINK_ACCEPT_ALL, "Accept all"),
    (keys::AUTOLINK_REJECT_ALL, "Reject all"),
    (keys::AUTOLINK_RESTORE_ALL, "Restore all"),
    (keys::AUTOLINK_CREATE, "Create links ({n})"),
    (
        keys::AUTOLINK_HINT,
        "Creation is one undo batch; rollback asks for confirmation and highlights the affected links (AC-5.3).",
    ),
    (keys::AUTOLINK_PARAM, "parameter {name}"),
    (
        keys::AUTOLINK_TOAST_CREATED,
        "Links created: {n} — a single undo step.",
    ),
    (
        keys::AUTOLINK_TOAST_NONE,
        "No matching names — nothing to propose.",
    ),
    (keys::ROW_AUTOLINK, "Background autolink by names"),
    // FR-079 (S3): suggestions tab
    // CR-022: reworded — AI lines removed from the typing popup
    // (c1_in_popup=false); the toggle drives C3 cards after node creation
    (keys::ROW_SUGGEST_ENABLED, "AI template cards (after node creation)"),
    (
        keys::DESC_SUGGEST_ENABLED,
        "\u{201c}What comes next\u{201d} cards on a new template node; the typing popup shows L0 hints only",
    ),
    (keys::ROW_SUGGEST_ENGINE, "Engine"),
    (keys::DESC_SUGGEST_ENGINE, "Local lexics or Laya hybrid (sidecar)"),
    (keys::SUGGEST_ENGINE_LEX, "Lexics (local)"),
    (keys::SUGGEST_ENGINE_LEXLAYA, "lex+Laya hybrid"),
    (keys::SUGGEST_ENGINE_OFF, "Off"),
    (
        keys::DESC_AUTOLINK,
        "The background detector proposes links for matching assignment names; a link is created only after review.",
    ),
    (keys::ROW_EXPLAIN_COVERAGE, "Chain coverage indicator"),
    (
        keys::DESC_EXPLAIN_COVERAGE,
        "Shows “Chains: N%” in a canvas corner — the share of computed digits whose chain reaches leaves.",
    ),
    (keys::EXPLAIN_COVERAGE, "Chains: {n}%"),
    // FR-ICONS: icon set — EN
    (keys::ROW_ICON_STYLE, "Icon set"),
    (
        keys::DESC_ICON_STYLE,
        "Monochrome SVG icons in place of Unicode glyphs. “Glyphs” is the system fallback (NotoSansDisplay font).",
    ),
    (keys::ICON_STYLE_GLYPH, "Glyphs (system)"),
    (keys::ICON_STYLE_LUCIDE, "Lucide"),
    (keys::ICON_STYLE_MATERIAL, "Material Symbols"),
    (keys::ICON_STYLE_FEATHER, "Feather"),
    (keys::ICON_STYLE_BOOTSTRAP, "Bootstrap Icons"),
    (keys::AUTOLINK_UNDO_TITLE, "Roll back the autolink batch ({n})?"),
    (
        keys::AUTOLINK_UNDO_BODY,
        "The batch links are highlighted on the canvas. Rollback is a single undo step.",
    ),
    (keys::AUTOLINK_UNDO_YES, "Roll back"),
    // --- FR-LLM-B / PRD-0010 F-7: «AI and Models» settings tab (EN) ---
    (keys::TAB_AI, "AI & Models"),
    (keys::AI_GRP_PROVIDER, "Per-feature provider (Q2)"),
    (keys::AI_GRP_BYOK, "BYOK — API key and model"),
    (keys::AI_GRP_RATE, "ChatGPT subscription rate limit (F-7.3)"),
    (keys::AI_GRP_LIMITS, "Limits & quality"),
    (keys::AI_GRP_RESIDENCY, "Data residency (Q5)"),
    (keys::AI_GRP_TELEMETRY, "Telemetry (opt-in)"),
    (keys::AI_ROW_PROV_SUGGEST, "Suggest"),
    (keys::AI_ROW_PROV_GRAPH, "Graph Builder"),
    (keys::AI_ROW_PROV_AGENT, "Agent Panel"),
    (keys::AI_ROW_MODEL, "Model"),
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а).
    (keys::AI_ROW_MODEL_SUGGEST, "Suggest model"),
    (keys::AI_ROW_MODEL_GRAPH, "Graph Builder model"),
    (keys::AI_ROW_MODEL_AGENT, "Agent Panel model"),
    (keys::AI_ROW_KEY, "API key"),
    (keys::AI_ROW_SH_URL, "Self-hosted endpoint"),
    (keys::AI_ROW_SH_KEY, "Endpoint API key"),
    (keys::AI_ROW_TELEMETRY, "Anonymous usage events"),
    // FR-LLM-FIX: лейблы новых строк (API-ключ BYOK + self-hosted endpoint).
    (keys::AI_ROW_API_KEY, "BYOK — API key and model"),
    (keys::AI_ROW_SELFHOST_URL, "Self-hosted endpoint"),
    (keys::AI_ROW_SELFHOST_KEY, "Endpoint API key"),
    (keys::AI_DESC_PROV_SUGGEST, "template hints and custom nodes"),
    (keys::AI_DESC_PROV_GRAPH, "graph generation from text"),
    (keys::AI_DESC_PROV_AGENT, "agent operations via MCP"),
    (
        keys::AI_DESC_FALLBACK,
        "Fallback chain on error/429: ChatGPT OAuth → BYOK → lex (suggest) / refuse (graph, agent). Keys are stored in OS keychain (desktop) / OPFS encrypted (web) — not in config.",
    ),
    (
        keys::AI_DESC_KEY,
        "Provider BYOK key (OpenAI/Anthropic/z.ai/Moonshot/OpenRouter). Stored in OS keychain (desktop) / OPFS encrypted (web).",
    ),
    (
        keys::AI_DESC_MODEL,
        "List from the provider's /v1/models. Loaded after a key health-check — before the check the list is empty, click «Check key» above.",
    ),
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а).
    (
        keys::AI_DESC_MODEL_SUGGEST,
        "BYOK model for Suggest. List is loaded from /v1/models after a key health-check — before the check, click «Check key» above.",
    ),
    (
        keys::AI_DESC_MODEL_GRAPH,
        "BYOK model for Graph Builder. List is loaded from /v1/models after a key health-check.",
    ),
    (
        keys::AI_DESC_MODEL_AGENT,
        "BYOK model for Agent Panel. List is loaded from /v1/models after a key health-check.",
    ),
    (
        keys::AI_DESC_RATE,
        "local counter; at 0 — fallback to BYOK, toast «ChatGPT unavailable»",
    ),
    (
        keys::AI_DESC_COST_LIMIT,
        "80% of the limit — extension dialog; 100% — LLM requests are rejected, lex kicks in (F-7.4, Q7)",
    ),
    (
        keys::AI_DESC_CONF_THRESHOLD,
        "variants with confidence below the threshold are hidden (F-2.11); affects suggest ghost-nodes",
    ),
    (
        keys::AI_DESC_RESIDENCY,
        "Determines which providers are available and whether context redact is applied (Q1+Q5).",
    ),
    (
        keys::AI_DESC_TELEMETRY,
        "default OFF (Q7): events of hint impressions/acceptances and cost — only after explicit consent; no PII. Full guide: user-docs/ai-features.md",
    ),
    // FR-LLM-FIX: описания новых строк (API-ключ BYOK + self-hosted endpoint).
    (
        keys::AI_DESC_API_KEY,
        "BYOK provider key (OpenAI/Anthropic/z.ai/Moonshot/OpenRouter). Stored in OS keychain (desktop) / OPFS encrypted (web). Click «Check key» — after the check, the model list is loaded from /v1/models.",
    ),
    (
        keys::AI_DESC_SELFHOST_URL,
        "OpenAI-compatible endpoint URL of your GPU server (e.g. https://llm.corp.local/v1). Verification — «Check» button in the key row below.",
    ),
    (
        keys::AI_DESC_SELFHOST_KEY,
        "Self-hosted endpoint API key (if the endpoint requires authorization). «Check» runs a /v1/models health-check and pins the badge.",
    ),
    // FR-LLM-FIX: плейсхолдеры dropdown'а BYOK-модели (когда список пуст).
    (keys::AI_MODEL_PH_NO_KEY, "Enter API key"),
    (
        keys::AI_MODEL_PH_NO_HC,
        "Click «Check key» to load the list",
    ),
    (keys::AI_MODEL_PH_EMPTY, "Choose a model"),
    // FR-LLM-FIX: бейдж статусной панели — активный провайдер без модели.
    (keys::AI_STATUS_MODEL_NOT_CHOSEN, "not chosen"),
    // FR-LLM-FIX-2: короткие подписи провайдеров — чип статусной панели.
    (keys::AI_PROV_SHORT_OFF, "Off"),
    (keys::AI_PROV_SHORT_LAYA, "Laya (local)"),
    (keys::AI_PROV_SHORT_OLLAMA, "Ollama"),
    (keys::AI_PROV_SHORT_BYOK, "BYOK"),
    (keys::AI_PROV_SHORT_CHATGPT, "ChatGPT OAuth"),
    (keys::AI_OPT_OFF, "Off"),
    (keys::AI_OPT_LAYA, "Laya (local)"),
    (keys::AI_OPT_OLLAMA, "Ollama (local)"),
    (keys::AI_OPT_BYOK, "BYOK (own key)"),
    (keys::AI_OPT_CHATGPT, "ChatGPT (sign-in)"),
    (
        keys::AI_OPT_CHATGPT_DISABLED,
        "ChatGPT OAuth — not available for Suggest (Q2)",
    ),
    (keys::AI_RESIDENCY_LOCAL, "Local only (Laya / Ollama)"),
    (keys::AI_RESIDENCY_CLOUD, "Cloud (ChatGPT / BYOK)"),
    (keys::AI_RESIDENCY_SELFHOST, "Self-hosted (your GPU server)"),
    (
        keys::AI_RESIDENCY_LOCAL_DESC,
        "Works offline · data never leaves the machine · quality below cloud",
    ),
    (
        keys::AI_RESIDENCY_CLOUD_DESC,
        "Best quality · context goes to the provider (redact: price=<redacted>) · daily cost limit",
    ),
    (
        keys::AI_RESIDENCY_SELFHOST_DESC,
        "Cloud-quality · data stays in your contour · an endpoint is required (user responsibility)",
    ),
    (keys::AI_BTN_CHECK_KEY, "Check key"),
    (keys::AI_BTN_CHECK, "Check"),
    (keys::AI_KEY_VALID, "key is valid · {n} models"),
    (keys::AI_KEY_NOT_CHECKED, "key not verified"),
    (keys::AI_KEY_EMPTY, "key is empty — enter a key"),
    (keys::AI_SH_NOT_CHECKED, "not verified"),
    (keys::AI_SH_OK, "endpoint responds · OpenAI-compatible"),
    (keys::AI_SH_BAD_URL, "invalid URL"),
    (keys::AI_COST_LIMIT_LABEL, "$ {v} / day"),
    (keys::AI_CONF_THR_LABEL, "{v}"),
    (keys::AI_FOOT_HINT, "Changes apply immediately · Esc — close"),
    (keys::AI_RATE_BADGE, "ChatGPT: {left}/{total} messages left"),
    // --- FR-LLM-OAUTH-APP / PRD-0010 F-5.8: Sign-in-with-ChatGPT block (EN) ---
    (keys::AI_OAUTH_ROW, "Sign in with ChatGPT"),
    (
        keys::AI_OAUTH_DESC,
        "OAuth sign-in with a ChatGPT subscription — for Graph Builder and Agent Panel. Tokens are stored locally (0600), never in config.toml.",
    ),
    (keys::AI_OAUTH_BTN_LOGIN, "Sign in with ChatGPT"),
    (
        keys::AI_OAUTH_HINT_BROWSER,
        "a browser window will open to confirm the sign-in",
    ),
    (keys::AI_OAUTH_BTN_CANCEL, "Cancel"),
    (keys::AI_OAUTH_WAITING, "Waiting for confirmation in the browser…"),
    (keys::AI_OAUTH_CONNECTED, "signed in · {email}"),
    (keys::AI_OAUTH_ACCOUNT, "account"),
    (keys::AI_OAUTH_BTN_LOGOUT, "Sign out"),
    (keys::AI_OAUTH_ERR, "sign-in failed: {error}"),
    (keys::AI_OAUTH_BTN_RETRY, "Retry"),
    (
        keys::AI_OAUTH_UNAVAILABLE,
        "OAuth sign-in requires the native build with the l1-llm-tls feature",
    ),
    (keys::AI_OAUTH_TOAST_LOGOUT, "Signed out — tokens removed"),
    // --- W2: health/discovery/graph toasts (EN) ---
    (keys::AI_HEALTH_OK, "Key is valid — provider responds"),
    (keys::AI_HEALTH_FAIL, "Health check failed"),
    (
        keys::AI_HEALTH_NO_KEY,
        "API key is empty — enter a key in AI settings (tab 9)",
    ),
    (
        keys::AI_HEALTH_NO_ENDPOINT,
        "Self-hosted URL is empty — set the endpoint in AI settings",
    ),
    (keys::AI_DISCOVERY_OK, "Models loaded: {n}"),
    (keys::AI_GRAPH_DONE, "Graph generated: {n} nodes"),
    (keys::AI_MENU_GRAPH_BUILDER, "Graph builder (AI)…"),
    // --- FR-LLM-B / PRD-0010 F-7.9: AI status panel (EN) ---
    (keys::AI_STATUS_PAUSED, "AI is paused — press ▶ to resume"),
    (keys::AI_STATUS_SESSION, "Session: {v}"),
    (keys::AI_STATUS_DAY, "Day: {v} / {lim}"),
    (keys::AI_STATUS_NO_MODEL, "AI is off"),
    (keys::AI_TOOLTIP_PAUSE, "Pause all AI features (⏸/▶)"),
    (keys::AI_TOOLTIP_SETTINGS, "Open AI settings (9th tab «AI & Models»)"),
    (keys::AI_TOOLTIP_SUGGEST, "Suggest: LLM template hints and custom nodes. Click — on/off"),
    (keys::AI_TOOLTIP_GRAPH, "Graph Builder: generate graphs from text. Click — on/off"),
    (keys::AI_TOOLTIP_AGENT, "Agent Panel: agent operations via MCP. Click — on/off"),
    // --- FR-LLM-B / PRD-0010 F-8: AI onboarding screen (EN) ---
    (keys::AI_ONB_STEP, "Step 3 of 3 · AI mode"),
    (keys::AI_ONB_TITLE, "Choose AI mode"),
    (
        keys::AI_ONB_SUB,
        "CanvasDesk works with any mode — you can change it later in settings (9th tab «AI & Models»).",
    ),
    (keys::AI_ONB_LOCAL_TITLE, "Local only (Laya / Ollama)"),
    (keys::AI_ONB_CLOUD_TITLE, "Cloud (ChatGPT / BYOK)"),
    (keys::AI_ONB_SELFHOST_TITLE, "Self-hosted (your GPU server)"),
    (
        keys::AI_ONB_LOCAL_DESC,
        "Works offline · data never leaves the machine · hints are below cloud-quality, graph/agent — via Ollama on your machine.",
    ),
    (
        keys::AI_ONB_CLOUD_DESC,
        "Best quality · context goes to the provider (numbers are masked: price=<redacted>) · daily limit $1.00 by default.",
    ),
    (
        keys::AI_ONB_SELFHOST_DESC,
        "Cloud-quality · data stays in your contour · an OpenAI-compatible endpoint is required (Settings → AI & Models).",
    ),
    (keys::AI_ONB_TAG_OFFLINE, "offline"),
    (keys::AI_ONB_TAG_QUALITY, "best quality"),
    (keys::AI_ONB_TAG_CONTOUR, "in contour"),
    (keys::AI_ONB_PRIV, "More about privacy"),
    (keys::AI_ONB_CONTINUE, "Continue"),
    (keys::AI_ONB_PRIV_TITLE, "What is sent."),
    (
        keys::AI_ONB_PRIV_BODY,
        "Only the context of the node being edited and its neighbors; numbers are masked with an explicit marker (price=<redacted> — the unit is preserved, Q1). The canvases, files, and keys themselves are never sent.",
    ),
    (keys::AI_ONB_PRIV_KEYS, "Where keys live."),
    (
        keys::AI_ONB_PRIV_KEYS_BODY,
        "BYOK keys — in OS keychain (desktop) / OPFS encrypted (web); ChatGPT OAuth tokens — also locally, no backend involved.",
    ),
    (keys::AI_ONB_PRIV_TELEMETRY, "Telemetry."),
    (
        keys::AI_ONB_PRIV_TELEMETRY_BODY,
        "Off by default; enabled only by explicit opt-in in settings.",
    ),
    (
        keys::AI_ONB_PRIV_DOCS,
        "Full guide — user-docs/ai-features.md (in the product, opens from this screen).",
    ),
    (keys::AI_ONB_TOAST_LOCAL, "AI mode: Local only — offline, data stays"),
    (keys::AI_ONB_TOAST_CLOUD, "AI mode: Cloud — better quality, data goes to provider (redact)"),
    (keys::AI_ONB_TOAST_SELFHOST, "AI mode: Self-hosted — cloud-quality in your contour"),
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
        // Плейсхолдер допускается с фигурными скобками и без («{name}» и
        // «name»): часть вызовов передаёт голое имя — голая replace
        // оставляла скобки в тексте («{name}» → «{значение}», FR-044:
        // заголовок stage, счётчик внешних входов, «строка N», «→ param»).
        let braced = format!("{{{placeholder}}}");
        if text.contains(&braced) {
            text = text.replace(&braced, value);
        } else {
            text = text.replace(placeholder, value);
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-044 (фикс): trf принимает плейсхолдер и с фигурными скобками,
    /// и без — голая replace оставляла скобки в тексте («{name}» →
    /// «{значение}») у вызовов с голым именем (заголовок stage,
    /// «строка N», счётчик внешних входов).
    #[test]
    fn trf_accepts_braced_and_bare_placeholders() {
        // Таблица «строка {n}»: вызов с голым «n» — скобки не остаются
        assert_eq!(
            trf(Language::Ru, keys::STAGE_LINE_LABEL, &[("n", "3")],),
            "строка 3"
        );
        // Вызов со скобками «{n}» — прежнее поведение (T10/тултипы)
        assert_eq!(
            trf(Language::Ru, keys::STAGE_LINE_LABEL, &[("{n}", "3")],),
            "строка 3"
        );
        // Несколько подстановок в одной фразе
        assert_eq!(
            trf(
                Language::Ru,
                keys::STAGE_BUNDLE_TITLE,
                &[("from", "Заявки"), ("to", "Отчёт"), ("n", "6")],
            ),
            "Пучок: Заявки → Отчёт · ×6"
        );
        // Новые ключи Р-3-а/Q2
        assert_eq!(
            trf(Language::Ru, keys::STAGE_OUT_LABEL, &[("name", "users")]),
            "out: users"
        );
        assert_eq!(
            trf(Language::Ru, keys::STAGE_CTRL_TO, &[("node", "Отчёт")]),
            "to: Отчёт"
        );
        assert_eq!(
            trf(Language::Ru, keys::STAGE_PILL_BELOW, &[("n", "4")]),
            "ещё 4 ↓"
        );
    }

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
        // FR-101: деталь именованного входа — источник значения
        assert_eq!(
            trf(Language::Ru, keys::HINT_SPILL, &[("{node}", "Купон")]),
            "проливание из ноды «Купон»"
        );
        assert_eq!(
            trf(Language::En, keys::HINT_SPILL, &[("{node}", "Купон")]),
            "spill from node \"Купон\""
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
            // FR-LLM-B: 9-й таб «AI и модели» — ключи должны быть в обеих таблицах.
            keys::TAB_AI,
        ];
        for key in row_keys.into_iter().chain(desc_keys).chain(tab_keys) {
            assert!(!tr(Language::Ru, key).is_empty(), "RU пуст: {key}");
            assert!(!tr(Language::En, key).is_empty(), "EN пуст: {key}");
            // Усиление 2026-09-25 (wasm-аудит: сырой «settings.tab.snap» в
            // сайдбаре настроек — ключ без записи в таблицах проходит
            // «непустой» фолбэк на сам ключ). Перевод ОБЯЗАН отличаться.
            assert_ne!(tr(Language::Ru, key), key, "RU = сырой ключ: {key}");
            assert_ne!(tr(Language::En, key), key, "EN = сырой ключ: {key}");
        }
    }

    /// W3-фикс (репорт владельца 2026-10-09): бейдж строки API-ключа —
    /// «ключ валиден · {n} моделей» с РЕАЛЬНЫМ числом discovery (вместо
    /// захардкоженного «0») и честный «ключ пустой» для пустого поля.
    /// Регресс: плейсхолдер {n} подставляется в обоих языках, ключи
    /// присутствуют (полные таблицы — tables_are_complete_and_consistent).
    #[test]
    fn ai_key_badge_substitutes_discovered_count() {
        // RU: число моделей из /v1/models попадает в бейдж
        assert_eq!(
            trf(Language::Ru, keys::AI_KEY_VALID, &[("n", &42.to_string())]),
            "ключ валиден · 42 моделей"
        );
        assert_eq!(
            trf(Language::En, keys::AI_KEY_VALID, &[("n", &7.to_string())]),
            tr(Language::En, keys::AI_KEY_VALID).replace("{n}", "7")
        );
        // Пустой ключ — отдельный честный бейдж (не «валиден»)
        for lang in [Language::Ru, Language::En] {
            let empty = tr(lang, keys::AI_KEY_EMPTY);
            assert!(!empty.is_empty(), "{lang:?} пуст: AI_KEY_EMPTY");
            assert_ne!(empty, keys::AI_KEY_EMPTY, "{lang:?} = сырой ключ");
            let valid = tr(lang, keys::AI_KEY_VALID);
            assert_ne!(
                valid, empty,
                "{lang:?}: «валиден» не должен совпадать с «пустой»"
            );
        }
    }

    /// FR-LLM-OAUTH-APP / PRD-0010 F-5.8: ключи блока «Вход ChatGPT»
    /// (строка SettingsRow::AiOAuth) присутствуют в обоих языках и не
    /// откатываются на сырой ключ; плейсхолдеры {email}/{error} подставляются.
    #[test]
    fn ai_oauth_keys_present_in_both_tables() {
        for key in [
            keys::AI_OAUTH_ROW,
            keys::AI_OAUTH_DESC,
            keys::AI_OAUTH_BTN_LOGIN,
            keys::AI_OAUTH_HINT_BROWSER,
            keys::AI_OAUTH_BTN_CANCEL,
            keys::AI_OAUTH_WAITING,
            keys::AI_OAUTH_CONNECTED,
            keys::AI_OAUTH_ACCOUNT,
            keys::AI_OAUTH_BTN_LOGOUT,
            keys::AI_OAUTH_ERR,
            keys::AI_OAUTH_BTN_RETRY,
            keys::AI_OAUTH_UNAVAILABLE,
            keys::AI_OAUTH_TOAST_LOGOUT,
        ] {
            for lang in [Language::Ru, Language::En] {
                assert!(!tr(lang, key).is_empty(), "{lang:?} пуст: {key}");
                assert_ne!(tr(lang, key), key, "{lang:?} = сырой ключ: {key}");
            }
        }
        assert_eq!(
            trf(
                Language::Ru,
                keys::AI_OAUTH_CONNECTED,
                &[("email", "a@b.c")]
            ),
            "вход выполнен · a@b.c"
        );
        assert_eq!(
            trf(Language::En, keys::AI_OAUTH_CONNECTED, &[("email", "")]),
            "signed in · "
        );
        assert_eq!(
            trf(Language::Ru, keys::AI_OAUTH_ERR, &[("error", "таймаут")]),
            "ошибка входа: таймаут"
        );
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
