//! FR-027: доступ к документации из приложения — чистая модель (образец
//! [`crate::settings_ui`]/`hints_ui`): вшитые страницы
//! `user-docs/` ([`DOCS_PAGES`], `include_str!` — без сети и FS,
//! wasm-переносимо), меню помощи кнопки «?» (колонка + подменю разделов,
//! паттерн `ContextMenu.submenu`), геометрия просмотрщика — правый док
//! ([`viewer_rect`]) с клампом к окну, раскладка GFM-страницы в строки
//! ([`layout_page`], таблицы за флагом `parse_blocks_opts` — заметки не
//! затронуты), скролл с клампом ([`ScrollState`]) и hit-тесты ссылок.
//!
//! Волна W-c (аудит ui-kit, §10): скролл — ТИП КИТА
//! (`canvas_ui::kit::ScrollState`; локальный дубль `offset`/`max_offset`
//! удалён, семантика колеса/sync — [`wheel_scroll`]/[`sync_scroll`]);
//! стопки пунктов меню «?» и подменю разделов — `kit::list_rows`
//! (паттерн `hints_ui::hint_rows`); слот панели просмотрщика —
//! `kit::stack` (паттерн `flowmap_ui`). Раскладка markdown-блоков и
//! таблиц kit-примитивов не выражает (ячейки одной строки с одного y и
//! перенос по ширине колонки — вне контракта `list_rows`/`Table`),
//! поэтому в ней заменены только отступы на шкалу `canvas_core::tokens`
//! (SPACING_*), ноль визуального скачка.
//!
//! Рендер и ввод — приложение (`main.rs`): квады + screen-тексты
//! (паттерн `settings_overlay`), клик по разделу подменю открывает
//! просмотрщик, колесо скроллит, клик по внутренней ссылке ведёт на
//! страницу. Схема `config.toml` не меняется.

use canvas_core::tokens;
use canvas_core::Language;
use canvas_render::gfm::{self, Block, LinkSegment};
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit;
use canvas_ui::layout::{stack, HAlign, VAlign};
use canvas_ui::measure::TextMeasurer;

/// FR-059-паттерн (волна W-c): состояние скролла просмотрщика — ТИП КИТА
/// (`canvas_ui::kit::ScrollState`: поля `offset`/`content_h`/`viewport_h`,
/// кламп — `scroll_by`+`clamp`, максимум — `max_offset()`). Путь
/// `docs_ui::ScrollState` сохранён реэкспортом для потребителей
/// (`app.rs`/`overlays.rs`); прежние ассоциированные `new`/`wheel`/
/// `resize` локального дубля — свободные функции [`sync_scroll`] и
/// [`wheel_scroll`] поверх кита.
pub use canvas_ui::kit::ScrollState;

use crate::i18n::{self, keys};

/// Семейство измерения = семейство screen-текстов рендера (паритет метрик
/// FR-053: раскладка и отрисовка в одних единицах).
const FAMILY: &str = canvas_render::text::SANS_FAMILY;

/// Вшитая страница документации: `id` (basename файла), короткая подпись
/// для подменю «Документация ▸» и сырое markdown-тело (`include_str!`,
/// front matter срезается [`strip_front_matter`]).
pub struct DocsPage {
    pub id: &'static str,
    /// Ключ подписи в подменю (таблица [`crate::i18n`] — FR-040; контент
    /// страницы не переводится — данные, не UI-код).
    pub label_key: &'static str,
    /// Сырой markdown (с front matter).
    pub md: &'static str,
}

/// Страницы вшиты в бинарь на этапе сборки (FR-027): отсутствие файла =
/// ошибка сборки — доки нельзя «забыть»; версия страниц = версия бинарника.
/// Порядок = порядок подменю (стабилен).
pub const DOCS_PAGES: [DocsPage; 9] = [
    DocsPage {
        id: "index",
        label_key: keys::DOCS_PAGE_INDEX,
        md: include_str!("../../../user-docs/index.md"),
    },
    DocsPage {
        id: "quick-start",
        label_key: keys::DOCS_PAGE_QUICK_START,
        md: include_str!("../../../user-docs/quick-start.md"),
    },
    DocsPage {
        id: "interface",
        label_key: keys::DOCS_PAGE_INTERFACE,
        md: include_str!("../../../user-docs/interface.md"),
    },
    DocsPage {
        id: "hotkeys",
        label_key: keys::DOCS_PAGE_HOTKEYS,
        md: include_str!("../../../user-docs/hotkeys.md"),
    },
    DocsPage {
        id: "calculations",
        label_key: keys::DOCS_PAGE_CALCULATIONS,
        md: include_str!("../../../user-docs/calculations.md"),
    },
    DocsPage {
        id: "templates",
        label_key: keys::DOCS_PAGE_TEMPLATES,
        md: include_str!("../../../user-docs/templates.md"),
    },
    DocsPage {
        id: "faq",
        label_key: keys::DOCS_PAGE_FAQ,
        md: include_str!("../../../user-docs/faq.md"),
    },
    DocsPage {
        id: "agent-recipe",
        label_key: keys::DOCS_PAGE_AGENT_RECIPE,
        md: include_str!("../../../user-docs/agent-recipe.md"),
    },
    // FR-070: страница «UI-консоль» (админпанель дизайн-системы)
    DocsPage {
        id: "admin",
        label_key: keys::DOCS_PAGE_ADMIN,
        md: include_str!("../../../user-docs/admin.md"),
    },
];

/// Сниппет совпадения в поиске по документации (владелец 2026-10-02):
/// обрезается до [`SEARCH_SNIPPET_MAX`] символов по границе char_indices.
///
/// ИСКЛЮЧЕНИЕ CR-015 (owner-approved, решение владельца 2026-10-02):
/// контентный сниппет — срез по символам, не layout. Это текстовая выжимка
/// результата контентного поиска (не элемент раскладки): её длина не
/// участвует в геометрии UI, а замер шейпингом здесь не даёт эффекта
/// WYSIWYG. Поведение не меняется (W-f, 04.10.2026).
const SEARCH_SNIPPET_MAX: usize = 44;

/// Поиск по встроенной документации (владелец 2026-10-02, Ctrl+F): подстрока
/// без учёта регистра по (1) локализованной метке страницы, (2) id страницы,
/// (3) markdown-телу (после среза front matter). Порядок — по страницам
/// [`DOCS_PAGES`] (стабилен, как подменю «Документация ▸»); одна страница —
/// максимум одна строка. Пустой запрос — пустой результат.
///
/// Возвращает `(индекс страницы, сниппет)`: сниппет — первая строка тела
/// с совпадением (обрезка по границе символов); `None` — совпадение только
/// в метке/id (тела без запроса не показываем).
pub fn search_pages(query: &str, language: Language) -> Vec<(usize, Option<String>)> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (page, def) in DOCS_PAGES.iter().enumerate() {
        let label = crate::i18n::tr(language, def.label_key).to_lowercase();
        let by_label = label.contains(&needle) || def.id.contains(&needle);
        let mut snippet = None;
        if !by_label {
            let body = strip_front_matter(def.md);
            for line in body.lines() {
                if line.to_lowercase().contains(&needle) {
                    let trimmed = line.trim();
                    let end = trimmed
                        .char_indices()
                        .nth(SEARCH_SNIPPET_MAX)
                        .map(|(i, _)| i)
                        .unwrap_or(trimmed.len());
                    snippet = Some(if end < trimmed.len() {
                        format!("{}…", &trimmed[..end])
                    } else {
                        trimmed.to_owned()
                    });
                    break;
                }
            }
            if snippet.is_none() {
                continue; // страница не матчится — строки нет
            }
        }
        hits.push((page, snippet));
    }
    hits
}

/// Срезать Jekyll front matter (`---\n…\n---\n` в начале): просмотрщику
/// не нужны title/description — заголовок даёт первый ATX. Нет закрывающего
/// `---` — файл как есть (не front matter). Разрезы идут по границам строк
/// (`\n`), UTF-8 не ломается.
pub fn strip_front_matter(md: &str) -> &str {
    let Some(after_open) = md.strip_prefix("---\n") else {
        return md;
    };
    let mut off = 0usize;
    for (i, line) in after_open.lines().enumerate() {
        // front matter короткий (title/description) — дальше ищем бессмысленно
        if i >= 10 {
            break;
        }
        if line.trim().starts_with("---") {
            let rest = &after_open[(off + line.len()).min(after_open.len())..];
            return rest.strip_prefix('\n').unwrap_or(rest);
        }
        off += line.len() + 1;
    }
    md
}

/// Цель ссылки (FR-027): внутренняя страница вшитых доков или внешний URL
/// (v1 не кликабельны — рендер обычным текстом, без акцента).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// Внутренняя страница (`id` из [`DOCS_PAGES`], якорь `#…` срезан).
    Page(&'static str),
    /// Внешний `http(s)://`/якорь/неизвестная страница — не кликабельно.
    External,
}

/// Классификация href страницы: относительные `*.html`/`*.md` → `Page`
/// (маппинг на id вшитых страниц, без каталога и якоря `#…`); абсолютные
/// URL и якоря → `External`.
pub fn link_target(href: &str) -> LinkTarget {
    let trimmed = href.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return LinkTarget::External;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.contains("://")
    {
        return LinkTarget::External;
    }
    let path = trimmed.split('#').next().unwrap_or(trimmed);
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let stem = name
        .strip_suffix(".html")
        .or_else(|| name.strip_suffix(".md"))
        .unwrap_or(name);
    DOCS_PAGES
        .iter()
        .find(|page| page.id == stem)
        .map_or(LinkTarget::External, |page| LinkTarget::Page(page.id))
}

/// Индекс страницы по id (переход по внутренней ссылке).
pub fn page_index_by_id(id: &str) -> Option<usize> {
    DOCS_PAGES.iter().position(|page| page.id == id)
}

// --- Меню помощи (клик по кнопке «?») ---

/// Ширина колонки меню помощи (логические px): «Пройти онбординг» с запасом.
pub const HELP_MENU_WIDTH: f32 = 200.0;
/// Высота пункта меню помощи.
pub const HELP_MENU_ITEM_H: f32 = 28.0;
/// Внутренний отступ колонки меню помощи (`SPACING_S` — токен шкалы,
/// значение прежнее: ноль визуального скачка).
pub const HELP_MENU_PAD: f32 = tokens::SPACING_S;
/// Ширина колонки подменю «Документация ▸»: «Расчёты и поток значений».
pub const DOCS_SUBMENU_WIDTH: f32 = 260.0;
/// Зазор между колонкой меню и подменю (паттерн `submenu_origin_next_to`).
pub const HELP_SUBMENU_GAP: f32 = 2.0;

/// Пункт меню помощи (клик по кнопке «?»).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpMenuItem {
    /// «Документация ▸» — открывает подменю разделов (8 страниц).
    Docs,
    /// «Пройти онбординг» — перезапуск тура (FR-028).
    Onboarding,
    /// «Галерея схем» — модальная галерея готовых схем (FR-049).
    Schemes,
    /// «О интерфейсе» — витрина кита (FR-055 U4, Q5-a: доступна всегда).
    Interface,
    /// «UI-консоль» — админпанель дизайн-системы (FR-070: матрица
    /// состояний, наполнение, сущности канваса, токены с live-правкой).
    Admin,
}

/// Пункты меню помощи (порядок отображения).
pub const HELP_MENU_ITEMS: [HelpMenuItem; 5] = [
    HelpMenuItem::Docs,
    HelpMenuItem::Onboarding,
    HelpMenuItem::Schemes,
    HelpMenuItem::Interface,
    HelpMenuItem::Admin,
];

/// Подпись пункта меню помощи (таблица [`crate::i18n`] — FR-040).
pub fn help_menu_item_label(item: HelpMenuItem, language: Language) -> &'static str {
    match item {
        HelpMenuItem::Docs => i18n::tr(language, keys::HELP_DOCS),
        HelpMenuItem::Onboarding => i18n::tr(language, keys::HELP_ONBOARDING),
        HelpMenuItem::Schemes => i18n::tr(language, keys::HELP_SCHEMES),
        HelpMenuItem::Interface => i18n::tr(language, keys::HELP_INTERFACE),
        HelpMenuItem::Admin => i18n::tr(language, keys::ADMIN_HELP_ITEM),
    }
}

/// Rect колонки меню помощи `[x, y, w, h]` (2 пункта).
pub fn help_menu_rect(origin: [f32; 2]) -> [f32; 4] {
    [
        origin[0],
        origin[1],
        HELP_MENU_WIDTH,
        HELP_MENU_PAD * 2.0 + HELP_MENU_ITEMS.len() as f32 * HELP_MENU_ITEM_H,
    ]
}

/// Строки колонки меню — `kit::list_rows` (волна W-c; паттерн
/// `hints_ui::hint_rows`): окно списка без прокрутки — высота ровно в
/// пункты, зазор 0, offset 0 (прежняя стопка дословно, I-1).
fn menu_item_rows(origin: [f32; 2], width: f32, count: usize) -> Vec<UiRect> {
    let content_h = count as f32 * HELP_MENU_ITEM_H;
    let area = UiRect::new(
        origin[0] + HELP_MENU_PAD,
        origin[1] + HELP_MENU_PAD,
        (width - HELP_MENU_PAD * 2.0).max(0.0),
        content_h,
    );
    let scroll = ScrollState {
        offset: 0.0,
        content_h,
        viewport_h: content_h,
    };
    kit::list_rows(area, &scroll, HELP_MENU_ITEM_H, 0.0, count)
        .into_iter()
        .map(|(_, rect)| rect)
        .collect()
}

/// Rect кита → `[x, y, w, h]` (публичный API меню — в координатах окна).
fn rect_arr(rect: UiRect) -> [f32; 4] {
    [rect.x, rect.y, rect.w, rect.h]
}

/// Rect пункта меню помощи (hover/рендер). Стопка — `menu_item_rows`
/// (kit `list_rows`); валидные индексы — бит-в-бит прежней формуле, вне
/// диапазона — пустой rect (прежняя формула границу не проверяла).
pub fn help_menu_item_rect(origin: [f32; 2], i: usize) -> [f32; 4] {
    menu_item_rows(origin, HELP_MENU_WIDTH, HELP_MENU_ITEMS.len())
        .get(i)
        .map(|rect| rect_arr(*rect))
        .unwrap_or([0.0; 4])
}

/// Hit-test пункта меню помощи (вне пунктов/в паддингах — None).
pub fn help_menu_item_at(origin: [f32; 2], point: [f32; 2]) -> Option<HelpMenuItem> {
    let [x, y, w, h] = help_menu_rect(origin);
    if point[0] < x || point[0] > x + w || point[1] < y || point[1] > y + h {
        return None;
    }
    let rel = point[1] - y - HELP_MENU_PAD;
    if rel < 0.0 {
        return None;
    }
    let i = (rel / HELP_MENU_ITEM_H) as usize;
    HELP_MENU_ITEMS.get(i).copied()
}

/// Origin колонки меню у кнопки «?» с клампом к окну (паттерн FR-026
/// `dropdown_layout`): у левого края окна — правее кнопки, у правого —
/// левее; по вертикали — от кнопки вниз, не ниже низа окна.
pub fn help_menu_origin(button: [f32; 4], viewport: [f32; 2]) -> [f32; 2] {
    let menu_h = HELP_MENU_PAD * 2.0 + HELP_MENU_ITEMS.len() as f32 * HELP_MENU_ITEM_H;
    let on_left = button[0] < viewport[0] / 2.0;
    let x = if on_left {
        (button[0] + button[2] + 4.0).min((viewport[0] - HELP_MENU_WIDTH).max(0.0))
    } else {
        (button[0] - HELP_MENU_WIDTH - 4.0).max(0.0)
    };
    let y = button[1].min((viewport[1] - menu_h).max(0.0));
    [x, y]
}

/// Origin подменю «Документация ▸» — колонка правее меню (паттерн
/// `submenu_origin_next_to`), с клампом к окну: справа не влезает — влево.
pub fn help_submenu_origin(menu_origin: [f32; 2], viewport: [f32; 2]) -> [f32; 2] {
    let right = menu_origin[0] + HELP_MENU_WIDTH + HELP_SUBMENU_GAP;
    if right + DOCS_SUBMENU_WIDTH <= viewport[0] {
        [right, menu_origin[1]]
    } else {
        [
            (menu_origin[0] - HELP_SUBMENU_GAP - DOCS_SUBMENU_WIDTH).max(0.0),
            menu_origin[1],
        ]
    }
}

/// Rect колонки подменю разделов `[x, y, w, h]` (7 пунктов).
pub fn help_submenu_rect(origin: [f32; 2]) -> [f32; 4] {
    [
        origin[0],
        origin[1],
        DOCS_SUBMENU_WIDTH,
        HELP_MENU_PAD * 2.0 + DOCS_PAGES.len() as f32 * HELP_MENU_ITEM_H,
    ]
}

/// Rect пункта подменю разделов (стопка — `menu_item_rows`,
/// kit `list_rows`; валидные индексы — бит-в-бит прежней формуле).
pub fn help_submenu_item_rect(origin: [f32; 2], i: usize) -> [f32; 4] {
    menu_item_rows(origin, DOCS_SUBMENU_WIDTH, DOCS_PAGES.len())
        .get(i)
        .map(|rect| rect_arr(*rect))
        .unwrap_or([0.0; 4])
}

/// Hit-test пункта подменю разделов: индекс страницы или None.
pub fn help_submenu_item_at(origin: [f32; 2], point: [f32; 2]) -> Option<usize> {
    let [x, y, w, h] = help_submenu_rect(origin);
    if point[0] < x || point[0] > x + w || point[1] < y || point[1] > y + h {
        return None;
    }
    let rel = point[1] - y - HELP_MENU_PAD;
    if rel < 0.0 {
        return None;
    }
    let i = (rel / HELP_MENU_ITEM_H) as usize;
    (i < DOCS_PAGES.len()).then_some(i)
}

// --- Просмотрщик: геометрия ---

/// Ширина панели просмотрщика (правый док, логические px).
pub const DOCS_PANEL_WIDTH: f32 = 480.0;
/// Высота шапки просмотрщика (заголовок раздела + ×).
// FR-046 W-d аудит §4: токен-источник `PANEL_HEADER_H_M` (38 — medium
// вариант шкалы высот шапок панелей). Прежний локальный литерал 38.0
// заменён реэкспортом токена — ноль визуального скачка (I-1).
pub use canvas_core::tokens::PANEL_HEADER_H_M as DOCS_HEADER_H;
/// Высота футера-подсказки просмотрщика.
pub const DOCS_FOOTER_H: f32 = 26.0;
/// Внутренние поля контента просмотрщика.
pub const DOCS_PADDING: f32 = 16.0;
/// Ширина скроллбара-аффорданса у правого края панели.
pub const DOCS_SCROLLBAR_W: f32 = 6.0;
/// Сторона кнопки × в шапке.
///
/// FR-070 (W-d): больше НЕ используется — заменён на канон
/// `canvas_ui::kit::ICON_BUTTON_SIZE` (= 26). Оставлен как `pub const`
/// для обратной совместимости внешних потребителей (если есть).
#[deprecated(
    since = "0.2.0",
    note = "FR-070 (W-d): use `canvas_ui::kit::ICON_BUTTON_SIZE` (canonical kit slot)"
)]
pub const DOCS_CLOSE_BUTTON: f32 = 26.0;

/// Rect панели просмотрщика: правый док на всю высоту окна, ширина
/// клампится к окну (320×240 — не шире окна). Волна W-c: слот — кит
/// `stack` (End/Start, паттерн `flowmap_ui`), x/y — прежняя формула
/// «правый край, ширина min(480, окно)» дословно.
pub fn viewer_rect(viewport: [f32; 2]) -> [f32; 4] {
    let w = DOCS_PANEL_WIDTH.min(viewport[0].max(0.0));
    let panel = stack(
        UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
        UiVec2::new(w, viewport[1]),
        HAlign::End,
        VAlign::Start,
    );
    [panel.x, panel.y, panel.w, panel.h]
}

/// Rect кнопки × (закрыть) в шапке панели.
///
/// FR-070 (W-d): каноническая «×»-кнопка — `kit::stage_close_button`
/// (size = `ICON_BUTTON_SIZE` = 26, inset = `SPACING_SM` = 8). Прежняя
/// локальная формула `pad = (DOCS_HEADER_H - DOCS_CLOSE_BUTTON) / 2.0`
/// (6px inset — вертикально по центру 38px шапки) заменена на канон 8px
/// (audit §6.1 — 7+ мест ручной формулы схлопнуты в kit). Tiny-panel
/// fallback сохранён (кламп size по полу-диагонали панели — как прежде).
pub fn viewer_close_rect(panel: [f32; 4]) -> [f32; 4] {
    let panel_min = panel[2].min(panel[3]);
    let kit_size = canvas_ui::kit::ICON_BUTTON_SIZE;
    if panel_min >= kit_size * 2.0 {
        // Нормальный случай — канон kit::stage_close_button (правый-верхний
        // угол панели, ICON_BUTTON_SIZE × ICON_BUTTON_SIZE, inset SPACING_SM).
        let slot = UiRect::new(panel[0], panel[1], panel[2], panel[3]);
        let r = kit::stage_close_button(slot);
        [r.x, r.y, r.w, r.h]
    } else {
        // Tiny-panel fallback — пропорциональный scale-down (как прежде:
        // size = kit_size.min(panel_min/2), inset = (panel_min - size)/2).
        let size = kit_size.min(panel_min * 0.5);
        let inset = (panel_min - size) * 0.5;
        [
            panel[0] + panel[2] - size - inset,
            panel[1] + inset,
            size,
            size,
        ]
    }
}

/// Rect зоны контента (между шапкой и футером), минус скроллбар.
pub fn viewer_content_rect(panel: [f32; 4]) -> [f32; 4] {
    [
        panel[0] + DOCS_PADDING,
        panel[1] + DOCS_HEADER_H,
        (panel[2] - DOCS_PADDING * 2.0 - DOCS_SCROLLBAR_W - 4.0).max(10.0),
        (panel[3] - DOCS_HEADER_H - DOCS_FOOTER_H).max(0.0),
    ]
}

// --- Раскладка страницы ---

/// Род строки раскладки (кегль/цвет/отступы на стороне рендера).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Заголовок ATX уровня 1..=6 (4–6 — как мелкий заголовок).
    Heading(u8),
    /// Строка абзаца.
    Body,
    /// Строка цитаты (квад-бар слева на стороне рендера).
    Quote,
    /// Строка фенса кода.
    Code,
    /// Строка пункта списка (маркер — в тексте первой строки).
    ListItem,
    /// Ячейка таблицы (x — от колонок; шапка подсвечивается цветом).
    TableCell { header: bool },
}

/// Кегль/высота строки по роду (логические px; публично — рендер
/// просмотрщика в main.rs берёт те же метрики).
pub fn kind_metrics(kind: RowKind) -> (f32, f32) {
    match kind {
        RowKind::Heading(1) => (20.0, 26.0),
        RowKind::Heading(2) => (16.0, 22.0),
        RowKind::Heading(_) => (14.0, 20.0),
        RowKind::Code | RowKind::TableCell { .. } => (12.0, 17.0),
        RowKind::Body | RowKind::Quote | RowKind::ListItem => (13.0, 19.0),
    }
}

/// Спан строки: текст + href (ссылка — акцентный цвет и клик).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSpan {
    pub text: String,
    pub href: Option<String>,
}

/// Строка раскладки: позиция в координатах контента (y от верха контента,
/// x — доп. отступ), род и спаны.
#[derive(Debug, Clone, PartialEq)]
pub struct DocLine {
    pub x: f32,
    pub y: f32,
    pub kind: RowKind,
    pub spans: Vec<DocSpan>,
}

/// Квад раскладки (линии `---` / подчёркивание шапки таблицы / бар цитаты).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocQuad {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Род квада: линия (тонкая, приглушённая) или бар цитаты (акцент).
    pub kind: QuadKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadKind {
    /// Горизонтальная линия (thematic break / под шапкой таблицы).
    Rule,
    /// Вертикальный бар цитаты (акцентный).
    QuoteBar,
}

/// Прямоугольник кликабельной ссылки в координатах контента. В список
/// попадают ТОЛЬКО внутренние ссылки (`LinkTarget::Page`).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRect {
    pub rect: [f32; 4],
    pub target: LinkTarget,
}

/// Раскладка страницы (FR-027): строки, квады, ссылки и высота контента.
/// Пересчитывается при смене страницы или ширины панели — не на каждый кадр.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PageLayout {
    pub lines: Vec<DocLine>,
    pub quads: Vec<DocQuad>,
    pub links: Vec<LinkRect>,
    /// Полная высота контента (px) — основа `ScrollState`.
    pub content_height: f32,
}

/// Снять инлайн-маркеры акцентов (`**`, `*`, `==`, `~~`, `` ` ``) — их
/// различие (жирный/моно) screen-тексты не поддерживают, содержимое
/// сохраняется. Непарный маркер — литерал.
fn strip_inline_markers(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    'outer: while !rest.is_empty() {
        for marker in ["**", "==", "~~", "`"] {
            if let Some(start) = rest.find(marker) {
                let after = &rest[start + marker.len()..];
                if let Some(end_rel) = after.find(marker) {
                    out.push_str(&rest[..start]);
                    out.push_str(&after[..end_rel]);
                    rest = &after[end_rel + marker.len()..];
                    continue 'outer;
                }
            }
        }
        if let Some(start) = rest.find('*') {
            // Одиночный `*` (курсив): пара в оставшемся тексте, НЕ
            // вплотную к открывающему (иначе это часть `**` — литерал)
            let after = &rest[start + 1..];
            if let Some(end_rel) = after.find('*') {
                if end_rel > 0 {
                    out.push_str(&rest[..start]);
                    out.push_str(&after[..end_rel]);
                    rest = &after[end_rel + 1..];
                    continue 'outer;
                }
            }
        }
        out.push_str(rest);
        break;
    }
    out
}

/// Спаны строки: инлайн-сегменты с href; маркеры акцентов сняты.
fn line_spans(text: &str) -> Vec<DocSpan> {
    gfm::inline_segments_links(&strip_inline_markers(text))
        .into_iter()
        .map(|LinkSegment { text, href }| DocSpan { text, href })
        .filter(|span| !span.text.is_empty())
        .collect()
}

/// Дописать слово в текущую строку с пробелом-разделителем: соседние
/// нессылочные спаны сливаются (меньше screen-текстов на кадр).
fn append_word(line: &mut Vec<DocSpan>, line_has_text: bool, word: &str, href: &Option<String>) {
    if line_has_text {
        match line.last_mut() {
            Some(last) if last.href.is_none() && href.is_none() => {
                last.text.push(' ');
                last.text.push_str(word);
                return;
            }
            _ => {
                // Пробел остаётся в хвосте предыдущего спана
                if let Some(last) = line.last_mut() {
                    last.text.push(' ');
                }
            }
        }
    }
    line.push(DocSpan {
        text: word.to_owned(),
        href: href.clone(),
    });
}

/// Перенос спанов по ширине: слова не рвутся; слово длиннее строки —
/// жёсткий разрыв по глифам. Ссылка может разбиться на части — каждая
/// несёт href и кликабельна.
fn wrap_spans(
    spans: &[DocSpan],
    max_w: f32,
    font: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<Vec<DocSpan>> {
    let max_w = max_w.max(1.0);
    // Плоский список слов (без пробелов) с href своего спана
    let mut words: Vec<(String, Option<String>)> = Vec::new();
    for span in spans {
        for word in span.text.split(' ') {
            if !word.is_empty() {
                words.push((word.to_owned(), span.href.clone()));
            }
        }
    }
    let mut lines: Vec<Vec<DocSpan>> = Vec::new();
    let mut cur: Vec<DocSpan> = Vec::new();
    let mut cur_w = 0.0;
    // Ширина пробела — измеренная (кэш TextMeasurer; единая для строки).
    let space_w = m.width_of(fs, " ", FAMILY, font);
    for (word, href) in &words {
        let word_w = m.width_of(fs, word, FAMILY, font);
        let need = word_w + if cur_w > 0.0 { space_w } else { 0.0 };
        if need <= max_w - cur_w {
            append_word(&mut cur, cur_w > 0.0, word, href);
            cur_w += need;
            continue;
        }
        // Не влезает: закрыть строку, сверхдлинное слово рвать по глифам
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        let mut rest: &str = word;
        while m.width_of(fs, rest, FAMILY, font) > max_w {
            // Сколько глифов влезает в пустую строку
            let mut fit = rest.chars().count();
            while fit > 1 {
                let head: String = rest.chars().take(fit).collect();
                if m.width_of(fs, &head, FAMILY, font) <= max_w {
                    break;
                }
                fit -= 1;
            }
            let head: String = rest.chars().take(fit).collect();
            let byte = rest
                .char_indices()
                .nth(fit)
                .map(|(b, _)| b)
                .unwrap_or(rest.len());
            cur.push(DocSpan {
                text: head,
                href: href.clone(),
            });
            lines.push(std::mem::take(&mut cur));
            rest = &rest[byte..];
        }
        if !rest.is_empty() {
            cur.push(DocSpan {
                text: rest.to_owned(),
                href: href.clone(),
            });
            cur_w = m.width_of(fs, rest, FAMILY, font);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Построитель раскладки: строки на ЯВНЫХ y-позициях (ячейки таблиц одной
/// строки идут с одного y), content_height — max по низу строк.
struct PageBuilder<'a> {
    layout: PageLayout,
    max_w: f32,
    /// Измеритель ширин (FR-054, F-6): реальный шейпинг cosmic-text.
    m: &'a mut TextMeasurer,
    fs: &'a mut cosmic_text::FontSystem,
}

impl<'a> PageBuilder<'a> {
    fn new(max_w: f32, m: &'a mut TextMeasurer, fs: &'a mut cosmic_text::FontSystem) -> Self {
        Self {
            layout: PageLayout {
                content_height: DOCS_PADDING,
                ..PageLayout::default()
            },
            max_w: max_w.max(10.0),
            m,
            fs,
        }
    }

    /// Поставить строку на явный y; ведёт ссылки; поднимает низ контента.
    fn place_line(&mut self, spans: &[DocSpan], kind: RowKind, x: f32, y: f32) {
        let (font, line_h) = kind_metrics(kind);
        let mut span_x = x;
        for span in spans {
            let w = self.m.width_of(self.fs, &span.text, FAMILY, font);
            if let Some(href) = &span.href {
                if let LinkTarget::Page(_) = link_target(href) {
                    self.layout.links.push(LinkRect {
                        rect: [span_x, y, w, line_h],
                        target: link_target(href),
                    });
                }
            }
            span_x += w;
        }
        self.layout.lines.push(DocLine {
            x,
            y,
            kind,
            spans: spans.to_vec(),
        });
        let bottom = y + line_h;
        if bottom > self.layout.content_height {
            self.layout.content_height = bottom;
        }
    }

    /// Текстовый блок с переносом по ширине (абзац/заголовок/цитата).
    fn push_wrapped(&mut self, text: &str, kind: RowKind, x: f32, gap_before: f32) {
        let (font, line_h) = kind_metrics(kind);
        let spans = line_spans(text);
        let width = self.max_w - x;
        let lines = wrap_spans(&spans, width, font, self.m, self.fs);
        let mut y = self.layout.content_height + gap_before;
        for line in &lines {
            self.place_line(line, kind, x, y);
            y += line_h;
        }
    }
}

/// Раскладка страницы (FR-027): GFM-блоки (`parse_blocks_opts(_, true)` —
/// таблицы включены, заметки не затронуты) → строки/квады/ссылки. Чистая
/// функция от индекса страницы, ширины контента и measurer'а (FR-054:
/// ширины — измеренные, family = FAMILY рендера); вызывается при открытии
/// страницы и смене размера панели, не на каждый кадр.
pub fn layout_page(
    page: usize,
    content_width: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> PageLayout {
    let Some(page) = DOCS_PAGES.get(page) else {
        return PageLayout::default();
    };
    let body = strip_front_matter(page.md);
    let mut b = PageBuilder::new(content_width, m, fs);
    for block in &gfm::parse_blocks_opts(body, true) {
        match block {
            Block::Heading { level, text } => {
                let kind = RowKind::Heading((*level).clamp(1, 6));
                b.push_wrapped(
                    text,
                    kind,
                    0.0,
                    if *level <= 2 {
                        tokens::SPACING_SM
                    } else {
                        tokens::SPACING_S
                    },
                );
            }
            Block::Paragraph { text } => {
                b.push_wrapped(text, RowKind::Body, 0.0, tokens::SPACING_S)
            }
            Block::Quote { text } => {
                let start_y = b.layout.content_height + tokens::SPACING_S;
                b.push_wrapped(text, RowKind::Quote, tokens::SPACING_LG, tokens::SPACING_S);
                let end_y = b.layout.content_height;
                if end_y > start_y {
                    b.layout.quads.push(DocQuad {
                        x: 0.0,
                        y: start_y,
                        width: 3.0,
                        height: end_y - start_y,
                        kind: QuadKind::QuoteBar,
                    });
                }
            }
            Block::Code { text } => {
                let mut y = b.layout.content_height + tokens::SPACING_SM;
                for line in text.split('\n') {
                    b.place_line(
                        &[DocSpan {
                            text: line.to_owned(),
                            href: None,
                        }],
                        RowKind::Code,
                        tokens::SPACING_SM,
                        y,
                    );
                    y += kind_metrics(RowKind::Code).1;
                }
                b.layout.content_height += tokens::SPACING_S;
            }
            Block::Rule => {
                let y = b.layout.content_height + tokens::SPACING_SM;
                b.layout.quads.push(DocQuad {
                    x: 0.0,
                    y,
                    width: b.max_w,
                    height: 1.5,
                    kind: QuadKind::Rule,
                });
                b.layout.content_height = y + tokens::SPACING_MD;
            }
            Block::List { ordered, items } => {
                // Отступы пункта (зазор 4, индент 16, хвост 4) — вне шкалы
                // SPACING_*, дословно (кит-аналога блока списка нет).
                for (n, item) in items.iter().enumerate() {
                    let marker = match item.checkbox {
                        Some(true) => "[x] ".to_owned(),
                        Some(false) => "[ ] ".to_owned(),
                        None if *ordered => format!("{}. ", n + 1),
                        None => "• ".to_owned(),
                    };
                    let marker_w = b.m.width_of(b.fs, &marker, FAMILY, 13.0);
                    let spans = line_spans(&item.text);
                    let lines = wrap_spans(&spans, b.max_w - 16.0 - marker_w, 13.0, b.m, b.fs);
                    let mut y = b.layout.content_height + 4.0;
                    for (i, line) in lines.iter().enumerate() {
                        if i == 0 {
                            let mut full = vec![DocSpan {
                                text: marker.clone(),
                                href: None,
                            }];
                            full.extend(line.iter().cloned());
                            b.place_line(&full, RowKind::ListItem, 16.0, y);
                        } else {
                            // Продолжение пункта — под текстом, без маркера
                            b.place_line(line, RowKind::ListItem, 16.0 + marker_w, y);
                        }
                        y += kind_metrics(RowKind::ListItem).1;
                    }
                }
                b.layout.content_height += 4.0;
            }
            Block::Table { header, rows } => layout_table(&mut b, header, rows),
        }
    }
    b.layout.content_height += DOCS_PADDING;
    b.layout
}

/// Раскладка таблицы (FR-027): ширины колонок — по максимальной ширине
/// ячейки с пропорциональным сжатием к доступной ширине; ячейки строки —
/// с одного y; линия-подчёркивание под шапкой.
fn layout_table(b: &mut PageBuilder, header: &[String], rows: &[Vec<String>]) {
    let cols = header.len().max(1);
    // Зазор колонок и пад ячейки — шкала токенов (значения прежние:
    // ноль визуального скачка); остальная геометрия (сжатие колонок,
    // ячейки одной строки с одного y) — вне kit-аналогов, дословно.
    let gap = tokens::SPACING_LG;
    let avail = (b.max_w - gap * (cols - 1) as f32).max(10.0);
    // Естественная ширина колонки — самая широкая ячейка (без переноса)
    let mut cell_w = |text: &str| {
        b.m.width_of(b.fs, &strip_inline_markers(text), FAMILY, 12.0) + tokens::SPACING_S
    };
    let natural: Vec<f32> = (0..cols)
        .map(|c| {
            let mut w = header.get(c).map(|h| cell_w(h)).unwrap_or(0.0);
            for row in rows {
                w = w.max(row.get(c).map(|cell| cell_w(cell)).unwrap_or(0.0));
            }
            w.max(30.0)
        })
        .collect();
    let total: f32 = natural.iter().sum();
    let widths: Vec<f32> = natural.iter().map(|w| w * avail / total.max(1.0)).collect();
    let mut col_x: Vec<f32> = Vec::with_capacity(cols);
    let mut x = 0.0;
    for w in &widths {
        col_x.push(x);
        x += w + gap;
    }
    let table_w = (x - gap).min(b.max_w);
    // Шапка: ячейки с одного y; строки-обёртки всех ячеек идут в порядке
    // y (внешний цикл — индекс строки, внутренний — колонка), чтобы
    // вектор lines оставался монотонным по y (инвариант раскладки)
    let header_y = b.layout.content_height + tokens::SPACING_SM;
    let (font, cell_h) = kind_metrics(RowKind::TableCell { header: false });
    let header_cells: Vec<Vec<Vec<DocSpan>>> = header
        .iter()
        .enumerate()
        .map(|(c, cell)| {
            let w = widths.get(c).copied().unwrap_or(avail);
            wrap_spans(&line_spans(cell), w, font, b.m, b.fs)
        })
        .collect();
    let header_rows = header_cells.iter().map(Vec::len).max().unwrap_or(0);
    let mut header_bottom = header_y;
    for i in 0..header_rows {
        for (c, cell_lines) in header_cells.iter().enumerate() {
            if let Some(line) = cell_lines.get(i) {
                let y = header_y + i as f32 * cell_h;
                b.place_line(line, RowKind::TableCell { header: true }, col_x[c], y);
                header_bottom = header_bottom.max(y + cell_h);
            }
        }
    }
    b.layout.quads.push(DocQuad {
        x: 0.0,
        y: header_bottom + 2.0,
        width: table_w,
        height: 1.0,
        kind: QuadKind::Rule,
    });
    // Тело: строки таблицы — ячейки с одного y, высота по максимуму строк
    let mut row_y = header_bottom + tokens::SPACING_MD;
    for row in rows {
        let cells: Vec<Vec<Vec<DocSpan>>> = row
            .iter()
            .take(cols)
            .enumerate()
            .map(|(c, cell)| wrap_spans(&line_spans(cell), widths[c], font, b.m, b.fs))
            .collect();
        let row_lines = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut row_bottom = row_y;
        for i in 0..row_lines {
            for (c, cell_lines) in cells.iter().enumerate() {
                if let Some(line) = cell_lines.get(i) {
                    let y = row_y + i as f32 * cell_h;
                    b.place_line(line, RowKind::TableCell { header: false }, col_x[c], y);
                    row_bottom = row_bottom.max(y + cell_h);
                }
            }
        }
        row_y = row_bottom + 4.0;
    }
    b.layout.content_height = b.layout.content_height.max(row_y);
}

// --- Скролл (kit ScrollState, волна W-c) ------------------------------------

// Локальный дубль `ScrollState` (`offset`/`max_offset` + `new`/`wheel`/
// `resize`) УДАЛЁН (волна W-c, аудит ui-kit §10): состояние скролла —
// ТИП КИТА [`ScrollState`] (реэкспорт `canvas_ui::kit::ScrollState`,
// поля `offset`/`content_h`/`viewport_h`). Инвариант FR-027 «offset в
// `[0, max_offset]` после любого колеса» держат [`wheel_scroll`] и
// [`sync_scroll`] через `ScrollState::clamp` кита.

/// Колесо просмотрщика — семантика прежнего `ScrollState::wheel` поверх
/// кита: прокрутка на `dy` (вниз > 0) с клампом в `[0, max_offset()]`
/// (`scroll_by` + `clamp`); `true` — offset изменился.
pub fn wheel_scroll(state: &mut ScrollState, dy: f32) -> bool {
    let before = state.offset;
    state.scroll_by(dy);
    state.clamp();
    (state.offset - before).abs() >= f32::EPSILON
}

/// Синхронизация скролла с контентом/окном — семантика прежних
/// `ScrollState::new`/`resize`: полная высота контента → `content_h`
/// («полный контент» кита), высота окна → `viewport_h`, затем кламп
/// позиции без сброса. Идемпотентно — вызывает раскладка при открытии
/// страницы/смене ширины панели (паттерн `flow_map_layout`).
pub fn sync_scroll(state: &mut ScrollState, content_height: f32, view_height: f32) {
    state.content_h = content_height;
    state.viewport_h = view_height;
    state.clamp();
}

/// Hit-test ссылки (FR-027): точка окна → цель ссылки или None. Точка
/// пересчитывается в координаты контента (минус origin зоны и скролл);
/// некликабельные внешние ссылки в `links` не попадают (фильтр сборки).
/// Волна W-c: скролл — kit [`ScrollState`] по ссылке (тип кита не
/// `Copy`; прежний параметр по значению работал на копии).
pub fn link_at<'a>(
    layout: &'a PageLayout,
    scroll: &ScrollState,
    content_origin: [f32; 2],
    point: [f32; 2],
) -> Option<&'a LinkRect> {
    let x = point[0] - content_origin[0];
    let y = point[1] - content_origin[1] + scroll.offset;
    layout.links.iter().find(|link| {
        let [lx, ly, lw, lh] = link.rect;
        x >= lx && x <= lx + lw && y >= ly && y <= ly + lh
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Инвариант полноты (FR-027): 8 страниц, id/подписи уникальны и
    /// непусты — пункт подменю ↔ страница, без пропусков и дублей.
    #[test]
    fn pages_complete_and_unique() {
        assert_eq!(DOCS_PAGES.len(), 9, "9 страниц документации");
        let mut ids = Vec::new();
        let mut labels = Vec::new();
        for page in &DOCS_PAGES {
            assert!(!page.id.is_empty());
            assert!(!page.label_key.is_empty());
            assert!(!ids.contains(&page.id), "дубль id: {}", page.id);
            assert!(
                !labels.contains(&page.label_key),
                "дубль подписи: {}",
                page.label_key
            );
            ids.push(page.id);
            labels.push(page.label_key);
        }
    }

    /// Инвариант парсинга (FR-027): все страницы парсятся с таблицами без
    /// паник; front matter срезан; тело не пустое.
    #[test]
    fn pages_parse_and_strip_front_matter() {
        for page in &DOCS_PAGES {
            let body = strip_front_matter(page.md);
            assert!(
                !body.starts_with("---\n"),
                "{}: front matter не срезан",
                page.id
            );
            assert!(!body.trim().is_empty(), "{}: пустое тело", page.id);
            let blocks = gfm::parse_blocks_opts(body, true);
            assert!(!blocks.is_empty(), "{}: блоки не нашлись", page.id);
            if ["hotkeys", "templates", "index", "calculations"].contains(&page.id) {
                assert!(
                    blocks.iter().any(|blk| matches!(blk, Block::Table { .. })),
                    "{}: ожидались таблицы",
                    page.id
                );
            }
        }
        assert_eq!(strip_front_matter("просто текст"), "просто текст");
        assert_eq!(strip_front_matter("---\nбитый"), "---\nбитый");
        let with_fm = "---\ntitle: X\n---\n# Заголовок\n";
        assert_eq!(strip_front_matter(with_fm), "# Заголовок\n");
    }

    /// Инвариант ссылок (FR-027): каждая внутренняя ссылка каждой страницы
    /// резолвится в существующий id (линк-чек — сломанная ссылка валит CI,
    /// а не пользователя).
    #[test]
    fn internal_links_resolve() {
        for page in &DOCS_PAGES {
            for line in strip_front_matter(page.md).lines() {
                for seg in gfm::inline_segments_links(&strip_inline_markers(line)) {
                    let Some(href) = seg.href else { continue };
                    if let LinkTarget::Page(id) = link_target(&href) {
                        assert!(
                            page_index_by_id(id).is_some(),
                            "{}: битая внутренняя ссылка {href}",
                            page.id
                        );
                    }
                }
            }
        }
    }

    /// Классификация ссылок: относительные .html/.md → Page; внешние,
    /// якоря и неизвестные страницы → External; якорь отрезается.
    #[test]
    fn link_target_classification() {
        assert_eq!(
            link_target("quick-start.html"),
            LinkTarget::Page("quick-start")
        );
        assert_eq!(link_target("./faq.html"), LinkTarget::Page("faq"));
        assert_eq!(link_target("index.md"), LinkTarget::Page("index"));
        assert_eq!(link_target("hotkeys.html#top"), LinkTarget::Page("hotkeys"));
        assert_eq!(link_target("https://jsoncanvas.org"), LinkTarget::External);
        assert_eq!(link_target("http://x.y/z"), LinkTarget::External);
        assert_eq!(link_target("#anchor"), LinkTarget::External);
        assert_eq!(link_target(""), LinkTarget::External);
        assert_eq!(link_target("unknown-page.html"), LinkTarget::External);
        assert_eq!(page_index_by_id("faq"), Some(6));
        assert_eq!(page_index_by_id("no-such"), None);
    }

    /// Просмотрщик: правый док на всю высоту; на 320×240 — не шире окна;
    /// × и зона контента внутри панели.
    #[test]
    fn viewer_rect_clamps_to_window() {
        let wide = viewer_rect([1600.0, 900.0]);
        assert_eq!(wide[0], 1600.0 - DOCS_PANEL_WIDTH);
        assert_eq!(wide[1], 0.0);
        assert_eq!(wide[3], 900.0);
        let tiny = viewer_rect([320.0, 240.0]);
        assert!(tiny[2] <= 320.0, "панель не шире окна");
        assert_eq!(tiny[0], 320.0 - tiny[2]);
        assert_eq!(tiny[3], 240.0);
        let close = viewer_close_rect(wide);
        assert!(close[0] >= wide[0] && close[0] + close[2] <= wide[0] + wide[2]);
        let content = viewer_content_rect(wide);
        assert!(content[1] >= wide[1] + DOCS_HEADER_H - 1.0);
        assert!(content[1] + content[3] <= wide[1] + wide[3]);
    }

    /// Скролл (kit `ScrollState` + [`wheel_scroll`]/[`sync_scroll`]): кламп
    /// offset после «перемотки» за границы; короткая страница —
    /// `max_offset() = 0`; sync (resize) пересчитает без сброса позиции.
    #[test]
    fn scroll_clamps_offset() {
        let mut s = ScrollState {
            offset: 0.0,
            content_h: 2000.0,
            viewport_h: 500.0,
        };
        assert_eq!(s.max_offset(), 1500.0);
        assert!(
            !wheel_scroll(&mut s, -500.0),
            "вверх из 0 — кламп, без изменения"
        );
        assert_eq!(s.offset, 0.0);
        assert!(wheel_scroll(&mut s, 9999.0));
        assert_eq!(s.offset, 1500.0, "вниз — кламп к max_offset()");
        assert!(!wheel_scroll(&mut s, 10.0), "на границе — без изменений");
        let short = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 500.0,
        };
        assert_eq!(short.max_offset(), 0.0);
        assert!(!short.needs_scroll(), "короткая страница — скролл не нужен");
        let mut s = ScrollState {
            offset: 0.0,
            content_h: 2000.0,
            viewport_h: 500.0,
        };
        assert!(wheel_scroll(&mut s, 1000.0));
        sync_scroll(&mut s, 2000.0, 1600.0);
        assert_eq!(s.max_offset(), 400.0);
        assert_eq!(s.offset, 400.0, "sync клампит позицию, не сбрасывая");
    }

    /// Волна W-c: kit `ScrollState` клампится так же, как прежний локальный
    /// дубль (`offset`/`max_offset` + `new`/`wheel`/`resize`) — граничные
    /// случаи (оракул — прежние ожидания теста `scroll_clamps_offset`).
    /// Прямые kit-операции (`scroll_by`+`clamp`) дают тот же результат,
    /// что и обёртки [`wheel_scroll`]/[`sync_scroll`].
    #[test]
    fn kit_scroll_state_clamps_like_old_local() {
        // «new»: max = max(0, content − view) — метод `max_offset()` кита
        let mk = |content: f32, view: f32| ScrollState {
            offset: 0.0,
            content_h: content,
            viewport_h: view,
        };
        assert_eq!(mk(2000.0, 500.0).max_offset(), 1500.0);
        assert_eq!(mk(100.0, 500.0).max_offset(), 0.0, "короткая страница");
        assert_eq!(mk(500.0, 500.0).max_offset(), 0.0, "контент = окну");
        assert_eq!(mk(50.0, 100.0).max_offset(), 0.0, "не уходит в минус");
        // «wheel»: кламп с обеих сторон; на границе — false (без изменения)
        let mut s = mk(200.0, 100.0);
        assert!(!wheel_scroll(&mut s, -1.0), "выше 0 не уходит");
        assert_eq!(s.offset, 0.0);
        assert!(wheel_scroll(&mut s, 150.0));
        assert_eq!(s.offset, 100.0, "кламп к max_offset()");
        assert!(
            !wheel_scroll(&mut s, 0.5),
            "на нижней границе — без изменений"
        );
        assert!(
            wheel_scroll(&mut s, -0.5),
            "у max прокрутка вверх — позиция меняется (не граница)"
        );
        assert_eq!(s.offset, 99.5, "сдвиг ровно на dy");
        assert!(
            wheel_scroll(&mut s, -200.0),
            "прокрутка выше начала — позиция изменилась"
        );
        assert_eq!(s.offset, 0.0, "кламп к 0 после прокрутки выше начала");
        // «resize»: max пересчитан, позиция сохраняется и клампится к новой
        let mut s = mk(200.0, 100.0);
        assert!(wheel_scroll(&mut s, 100.0)); // offset = max = 100
        sync_scroll(&mut s, 200.0, 200.0);
        assert_eq!(s.max_offset(), 0.0);
        assert_eq!(s.offset, 0.0, "sync ужал позицию к новой границе");
        // Прямые kit-операции — паритет с обёртками
        let mut direct = mk(200.0, 100.0);
        direct.scroll_by(150.0);
        direct.clamp();
        assert_eq!(direct.offset, 100.0, "scroll_by+clamp = wheel_scroll вниз");
        direct.scroll_by(-500.0);
        direct.clamp();
        assert_eq!(direct.offset, 0.0, "scroll_by+clamp = wheel_scroll вверх");
    }

    /// Hit-тест ссылок: по ссылке — цель; мимо/снаружи зоны — None.
    #[test]
    fn link_at_finds_only_internal() {
        let mut layout = PageLayout {
            content_height: 100.0,
            ..PageLayout::default()
        };
        layout.links.push(LinkRect {
            rect: [10.0, 20.0, 80.0, 19.0],
            target: LinkTarget::Page("faq"),
        });
        let scroll = ScrollState {
            offset: 5.0,
            content_h: 600.0,
            viewport_h: 100.0,
        };
        let origin = [100.0, 50.0];
        let hit = link_at(&layout, &scroll, origin, [100.0 + 40.0, 50.0 + 20.0 + 5.0]);
        assert_eq!(hit.map(|l| l.target.clone()), Some(LinkTarget::Page("faq")));
        assert!(link_at(&layout, &scroll, origin, [100.0 + 40.0, 50.0 + 100.0]).is_none());
        assert!(link_at(&layout, &scroll, origin, [100.0 + 200.0, 50.0 + 25.0]).is_none());
    }

    /// Волна W-c: стопки пунктов меню «?» и подменю — kit `list_rows`
    /// (`menu_item_rows`); геометрия бит-в-бит прежней формуле
    /// `[origin+PAD, origin+PAD+i·H, W−2·PAD, H]` (I-1 ноль скачка); вне
    /// диапазона — пустой rect (не паника).
    #[test]
    fn menu_item_rows_match_old_stack_formula() {
        let origin = [40.0, 60.0];
        for i in 0..HELP_MENU_ITEMS.len() {
            let [x, y, w, h] = help_menu_item_rect(origin, i);
            assert_eq!(x, origin[0] + HELP_MENU_PAD);
            assert_eq!(y, origin[1] + HELP_MENU_PAD + i as f32 * HELP_MENU_ITEM_H);
            assert_eq!(w, HELP_MENU_WIDTH - HELP_MENU_PAD * 2.0);
            assert_eq!(h, HELP_MENU_ITEM_H);
        }
        for i in 0..DOCS_PAGES.len() {
            let [x, y, w, h] = help_submenu_item_rect(origin, i);
            assert_eq!(x, origin[0] + HELP_MENU_PAD);
            assert_eq!(y, origin[1] + HELP_MENU_PAD + i as f32 * HELP_MENU_ITEM_H);
            assert_eq!(w, DOCS_SUBMENU_WIDTH - HELP_MENU_PAD * 2.0);
            assert_eq!(h, HELP_MENU_ITEM_H);
        }
        // Вне диапазона — пустой rect (прежняя формула границу не проверяла)
        assert_eq!(help_menu_item_rect(origin, HELP_MENU_ITEMS.len()), [0.0; 4]);
        assert_eq!(help_submenu_item_rect(origin, DOCS_PAGES.len()), [0.0; 4]);
        // Окно списка menu_item_rows — ровно в пункты, offset 0: прокрутка
        // не нужна (kit-контракт «list_rows — чистая геометрия»)
        assert!(menu_item_rows(origin, HELP_MENU_WIDTH, 5)
            .iter()
            .enumerate()
            .all(|(i, rect)| {
                rect.y - (origin[1] + HELP_MENU_PAD + i as f32 * HELP_MENU_ITEM_H) == 0.0
            }));
    }

    /// Меню помощи: hit-тесты пунктов и подменю; биекция подменю ↔
    /// DOCS_PAGES (подпись пункта = label страницы).
    #[test]
    fn help_menu_hit_tests() {
        let origin = [400.0, 100.0];
        let docs_rect = help_menu_item_rect(origin, 0);
        assert_eq!(
            help_menu_item_at(origin, [docs_rect[0] + 5.0, docs_rect[1] + 5.0]),
            Some(HelpMenuItem::Docs)
        );
        let onb_rect = help_menu_item_rect(origin, 1);
        assert_eq!(
            help_menu_item_at(origin, [onb_rect[0] + 5.0, onb_rect[1] + 5.0]),
            Some(HelpMenuItem::Onboarding)
        );
        assert_eq!(
            help_menu_item_at(origin, [origin[0] - 5.0, origin[1]]),
            None
        );
        assert_eq!(
            help_menu_item_at(origin, [origin[0] + 2.0, origin[1] + 2.0]),
            None,
            "паддинг — не пункт"
        );
        for i in 0..DOCS_PAGES.len() {
            let rect = help_submenu_item_rect(origin, i);
            assert_eq!(
                help_submenu_item_at(origin, [rect[0] + 5.0, rect[1] + 10.0]),
                Some(i)
            );
            assert_eq!(rect[2], DOCS_SUBMENU_WIDTH - HELP_MENU_PAD * 2.0);
        }
        assert_eq!(
            help_submenu_item_at(origin, [origin[0], origin[1] + 400.0]),
            None
        );
        // Подписи пунктов непустые
        for item in HELP_MENU_ITEMS {
            assert!(!help_menu_item_label(item, Language::Ru).is_empty());
        }
    }

    /// Кламп колонок к окну: меню «?» у правого края — влево; подменю —
    /// вправо, если влезает, иначе влево от меню; низ не ниже окна.
    #[test]
    fn menu_origin_clamped_to_window() {
        let button_left = [12.0, 12.0, 36.0, 36.0];
        let origin = help_menu_origin(button_left, [1600.0, 900.0]);
        assert_eq!(origin[0], button_left[0] + button_left[2] + 4.0);
        assert!(origin[1] + help_menu_rect(origin)[3] <= 900.0);
        let button_right = [1600.0 - 12.0 - 36.0, 12.0, 36.0, 36.0];
        let origin = help_menu_origin(button_right, [1600.0, 900.0]);
        assert_eq!(origin[0], button_right[0] - HELP_MENU_WIDTH - 4.0);
        let sub = help_submenu_origin(origin, [1600.0, 900.0]);
        // Справа от меню не влезает (правый край) — подменю УЛЕВО от меню,
        // без наложения на колонку меню и целиком в окне
        assert!(
            sub[0] + DOCS_SUBMENU_WIDTH <= origin[0],
            "подменю левее меню без наложения: {sub:?}"
        );
        assert!(sub[0] >= 0.0);
        // Слева от меню — влезает: колонка правее
        let sub = help_submenu_origin([300.0, 100.0], [1600.0, 900.0]);
        assert_eq!(sub[0], 300.0 + HELP_MENU_WIDTH + HELP_SUBMENU_GAP);
        let narrow = help_submenu_origin([1200.0, 100.0], [1300.0, 900.0]);
        assert!(
            narrow[0] + DOCS_SUBMENU_WIDTH <= 1300.0,
            "подменю целиком в окне"
        );
        // У нижнего края: колонка не вылезает за низ окна
        let bottom_button = [1552.0, 852.0, 36.0, 36.0];
        let origin = help_menu_origin(bottom_button, [1600.0, 900.0]);
        let rect = help_menu_rect(origin);
        assert!(
            rect[1] + rect[3] <= 900.0 + 1.0,
            "низ меню в окне: {rect:?}"
        );
    }

    /// Раскладка страницы: строки упорядочены, таблицы дают ячейки,
    /// ссылки в пределах ширины, контент выше нуля.
    /// Детерминированный FontSystem тестов: только вшитый рендером шрифт
    /// (паттерн measure.rs — метрики одинаковы на всех платформах CI).
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    #[test]
    fn layout_page_orders_lines() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let layout = layout_page(3, 440.0, &mut m, &mut fs); // hotkeys — таблицы
        assert!(!layout.lines.is_empty());
        assert!(layout.content_height > 100.0);
        for pair in layout.lines.windows(2) {
            assert!(pair[0].y <= pair[1].y, "y не монотонен");
        }
        assert!(layout
            .lines
            .iter()
            .any(|l| matches!(l.kind, RowKind::TableCell { .. })));
        for link in &layout.links {
            assert!(
                link.rect[0] + link.rect[2] <= 440.0 + 1.0,
                "ссылка шире контента"
            );
        }
        // Главная: внутренние ссылки есть (таблица разделов)
        let index = layout_page(0, 440.0, &mut m, &mut fs);
        assert!(index
            .links
            .iter()
            .any(|l| matches!(l.target, LinkTarget::Page(_))));
        // Все 8 страниц раскладываются без паник
        for page in 0..DOCS_PAGES.len() {
            let layout = layout_page(page, 440.0, &mut m, &mut fs);
            assert!(!layout.lines.is_empty(), "страница {page} пустая");
        }
    }

    /// Перенос: длинный текст разбивается; сверхдлинное слово рвётся по
    /// глифам без паник; ссылка разбивается на кликабельные части.
    #[test]
    fn wrap_spans_breaks_long_text() {
        let spans = vec![DocSpan {
            text: "а б в г д е ж з и к л м н о п".to_owned(),
            href: None,
        }];
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let lines = wrap_spans(&spans, 40.0, 13.0, &mut m, &mut fs);
        assert!(lines.len() >= 2, "перенос обязан случиться");
        for line in &lines {
            let w = line
                .iter()
                .map(|s| m.width_of(&mut fs, &s.text, FAMILY, 13.0))
                .sum::<f32>();
            assert!(w <= 40.0 + 1.0, "строка шире лимита: {w}");
        }
        let long = vec![DocSpan {
            text: "очень-очень-длинное-слово-без-пробелов-совсем".to_owned(),
            href: None,
        }];
        let lines = wrap_spans(&long, 60.0, 13.0, &mut m, &mut fs);
        assert!(lines.len() >= 2, "жёсткий разрыв обязан случиться");
        let linked = vec![DocSpan {
            text: "подробности в разделе расчётов и здесь хвост".to_owned(),
            href: Some("calculations.html".to_owned()),
        }];
        let lines = wrap_spans(&linked, 50.0, 13.0, &mut m, &mut fs);
        assert!(lines.len() >= 2);
        assert!(lines.iter().all(|l| l.iter().all(|s| s.href.is_some())),);
    }

    /// Снятие инлайн-маркеров: содержимое сохраняется, маркеры уходят.
    #[test]
    fn strip_inline_markers_keeps_content() {
        assert_eq!(strip_inline_markers("**жирный**"), "жирный");
        assert_eq!(strip_inline_markers("`code`"), "code");
        assert_eq!(strip_inline_markers("==выделение=="), "выделение");
        assert_eq!(strip_inline_markers("~~зачёркнутый~~"), "зачёркнутый");
        assert_eq!(strip_inline_markers("*курсив*"), "курсив");
        assert_eq!(strip_inline_markers("без маркеров"), "без маркеров");
        assert_eq!(strip_inline_markers("**незакрытый"), "**незакрытый");
        assert_eq!(strip_inline_markers("смесь **a** и `b`"), "смесь a и b");
    }

    /// Владелец 2026-10-02 (поиск по докам в Ctrl+F): метка/id страницы
    /// матчится без сниппета; пустой запрос и промах — пустой результат.
    #[test]
    fn search_pages_matches_label_and_id() {
        // «faq» — метка «FAQ» и id «faq» (строка FAQ: сниппета нет, даже
        // когда тело тоже матчится — метка приоритетна)
        let hits = search_pages("faq", Language::Ru);
        let faq = hits
            .iter()
            .find(|(p, _)| DOCS_PAGES[*p].id == "faq")
            .expect("страница FAQ в результатах");
        assert!(faq.1.is_none(), "совпадение по метке — без сниппета");
        // Пустой/пробельный запрос — пусто
        assert!(search_pages("", Language::Ru).is_empty());
        assert!(search_pages("   ", Language::Ru).is_empty());
        // Промах
        assert!(search_pages("несуществующий-термин-xyz", Language::Ru).is_empty());
    }

    /// Совпадение в теле страницы даёт сниппет (первая строка с запросом,
    /// обрезка по границе символов с «…»); регистр не важен.
    #[test]
    fn search_pages_body_match_yields_snippet() {
        let hits = search_pages("ГОТОВАЯ РАСЧЁТНАЯ РОЛЬ", Language::Ru);
        assert_eq!(hits.len(), 1, "страница шаблонов матчится по телу");
        let (page, snippet) = &hits[0];
        assert_eq!(DOCS_PAGES[*page].id, "templates");
        let text = snippet.as_ref().expect("совпадение в теле — со сниппетом");
        assert!(
            text.starts_with("Шаблон"),
            "сниппет от начала строки: {text}"
        );
        assert!(text.ends_with('…'), "сниппет длиннее лимита — с «…»");
        assert!(text.chars().count() <= 45, "сниппет обрезан: {text}");
    }

    /// Порядок результатов — по DOCS_PAGES (стабилен); одна страница —
    /// одна строка даже при нескольких совпадениях в теле.
    #[test]
    fn search_pages_order_stable_one_row_per_page() {
        let hits = search_pages("canvasdesk", Language::Ru);
        let pages: Vec<usize> = hits.iter().map(|(p, _)| *p).collect();
        let mut sorted = pages.clone();
        sorted.sort_unstable();
        assert_eq!(pages, sorted, "порядок = порядок DOCS_PAGES");
        let mut unique = pages.clone();
        unique.dedup();
        assert_eq!(unique, pages, "одна страница — максимум одна строка");
    }
}
