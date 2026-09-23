//! FR-021: контекстные подсказки при Numi-вводе — чистая модель
//! (образец [`crate::template_ui`]): токен перед кареткой, фильтрация
//! каталога движка, клавиатурный popup.
//!
//! Рендер и перехват клавиш — приложение (`main.rs`): popup собирается
//! по кадру из квадов + screen-текстов (паттерн `wheel_overlay`), а
//! принятие подсказки заменяет токен слева от каретки
//! ([`canvas_render::edit::EditingSession::replace_token_before_caret`]).
//!
//! FR-059 (волна 1 миграции кита, паттерн U5 — числа дословно): геометрия
//! popup — через кит v2 вместо ручного клампа к окну (класс дефекта
//! CR-015): `kit::dropdown_menu` («якорь + flip», PRD-0009 §2) + строки —
//! `kit::list_rows` ([`ScrollState`], окно без прокрутки — лимит
//! [`HINT_LIMIT`] сохранён). Числа прежние: ширина/высота строки/поля —
//! 0 визуального скачка; замена только там, где устраняется эвристика
//! (ручной кламп → flip кита: у нижнего края popup разворачивается НАД
//! строкой каретки — якорь-строка высотой [`HINT_CARET_LINE_H`], прежняя
//! формула «−20» = строка 16 + зазор 4).

use canvas_core::{expr, Language};
use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit::{self, ScrollState};
use canvas_ui::layout::constrain;

use crate::i18n::{self, keys};

/// Максимум элементов в popup (читабельность, стандарт автокомплитов) —
/// именованный лимит списка, не кламп раскладки (срезов «хвоста» нет).
pub const HINT_LIMIT: usize = 8;
/// Ширина popup (логические px; у узкого окна зажимается во вьюпорт —
/// `constrain`-семантика `dropdown_menu`).
pub const HINT_WIDTH: f32 = 300.0;
/// Высота строки popup.
pub const HINT_ROW_H: f32 = 24.0;
/// Внутренние поля popup (по вертикали; по горизонтали подсветка строки
/// инсетится на [`HINT_ROW_INSET_H`]).
pub const HINT_MARGIN: f32 = 6.0;
/// Горизонтальный инсет подсветки строки от краёв popup (прежние +4/−8).
pub const HINT_ROW_INSET_H: f32 = 4.0;
/// Высота строки каретки для якоря dropdown (прежний flip «−20» =
/// строка 16 + `DROPDOWN_GAP` 4 — числа прежней формулы дословно).
pub const HINT_CARET_LINE_H: f32 = 16.0;

/// Род подсказки (окраска/семантика в UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintKind {
    /// Переменная листа (присваивание выше по тексту).
    Var,
    /// `$`-ссылка: `$in`/`$1..$N` (FR-014) или параметр шаблона (FR-018).
    DollarRef,
    /// Функция движка (каталог `expr::fn_hints`).
    Fn,
    /// Токен единицы (`expr::unit_tokens`).
    Unit,
}

/// Элемент подсказки: что вставить и как подписать.
#[derive(Debug, Clone, PartialEq)]
pub struct HintItem {
    pub kind: HintKind,
    /// Текст вставки (замещает токен слева от каретки); функция — с
    /// открывающей скобкой.
    pub insert: String,
    /// Главная подпись.
    pub label: String,
    /// Серая деталь: сигнатура / описание / «переменная».
    pub detail: String,
}

/// Контекст ноды для подсказок.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HintContext {
    /// Переменные листа (присваивания ВЫШЕ строки каретки).
    pub vars: Vec<String>,
    /// Число value-рёбер на ноде (`$in`/`$1..$N` — FR-014).
    pub inbound: usize,
    /// Имена параметров шаблона (`$rps`… — FR-018); нешаблонная нода —
    /// пусто.
    pub params: Vec<String>,
}

/// Токен слева от каретки: `(токен, байтовая позиция начала токена)`.
/// Токен — `$`+идентификатор или идентификатор (буквы Unicode — латиница/
/// кириллица — цифры, `_`; тот же класс, что у лексера движка — правка
/// 2026-09-23: кириллические единицы `2 се` фильтруют каталог); любой
/// другой символ (пробел, оператор) обрывает токен. Пустая строка —
/// подсказки по контексту (переменные/единицы после числа).
pub fn token_before_caret(line: &str, caret: usize) -> (String, usize) {
    let caret = caret.min(line.len());
    let prefix = &line[..caret];
    // FR-059/G5: без `break`-выхода — скан от каретки влево `take_while`
    // (класс символов прежний: буквы Unicode/цифры/`_`, `$` включает и
    // завершает токен). Семантика байт-в-байт прежняя (тесты FR-021/FR-013).
    let start = prefix
        .char_indices()
        .rev()
        .take_while(|&(_, ch)| ch.is_alphanumeric() || ch == '_' || ch == '$')
        .last()
        .map(|(i, _)| i)
        .unwrap_or(caret);
    (prefix[start..].to_owned(), start)
}

/// Элементы подсказок для строки каретки (префикс ДО каретки) и контекста
/// ноды. Порядок (proposal FR-021): переменные → `$`-ссылки → функции →
/// единицы; лимит [`HINT_LIMIT`]. Единицы — после числа (токен-число или
/// число+пробел); `$`-токен — только `$`-ссылки; пустой токен — переменные.
pub fn hint_items(line_prefix: &str, ctx: &HintContext, language: Language) -> Vec<HintItem> {
    let (token, _start) = token_before_caret(line_prefix, line_prefix.len());
    let mut items: Vec<HintItem> = Vec::with_capacity(HINT_LIMIT);
    let mut push = |item: HintItem| {
        if items.len() < HINT_LIMIT {
            items.push(item);
        }
    };
    // `$`-контекст: входы value-рёбер и параметры шаблона
    if let Some(name) = token.strip_prefix('$') {
        let lower = name.to_lowercase();
        if ctx.inbound > 0 && "in".starts_with(&lower) {
            push(HintItem {
                kind: HintKind::DollarRef,
                insert: "$in".to_owned(),
                label: "$in".to_owned(),
                detail: i18n::tr(language, keys::HINT_DOLLAR_IN).to_owned(),
            });
        }
        for i in 1..=ctx.inbound {
            let label = format!("${i}");
            if label.starts_with(&token) {
                push(HintItem {
                    kind: HintKind::DollarRef,
                    insert: label.clone(),
                    label,
                    detail: i18n::trf(language, keys::HINT_DOLLAR_N, &[("{i}", &i.to_string())]),
                });
            }
        }
        for param in &ctx.params {
            let label = format!("${param}");
            if label.to_lowercase().starts_with(&token.to_lowercase()) {
                push(HintItem {
                    kind: HintKind::DollarRef,
                    insert: label.clone(),
                    label: param.clone(),
                    detail: i18n::tr(language, keys::HINT_PARAM).to_owned(),
                });
            }
        }
        return items;
    }
    let numeric = !token.is_empty()
        && token
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == ',');
    // Число + пробел — единицы с пустым префиксом
    let after_number = token.is_empty()
        && line_prefix
            .trim_end()
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_digit());
    if numeric || after_number {
        for unit in expr::unit_tokens() {
            if unit.to_lowercase().starts_with(&token.to_lowercase()) {
                push(HintItem {
                    kind: HintKind::Unit,
                    insert: unit.to_owned(),
                    label: unit.to_owned(),
                    detail: i18n::tr(language, keys::HINT_UNIT).to_owned(),
                });
            }
        }
        return items;
    }
    if token.is_empty() {
        // Пустой токен на Numi-строке — переменные ноды
        for var in &ctx.vars {
            push(HintItem {
                kind: HintKind::Var,
                insert: var.clone(),
                label: var.clone(),
                detail: i18n::tr(language, keys::HINT_VAR).to_owned(),
            });
        }
        return items;
    }
    // Идентификатор: переменные → функции → единицы (регистр не важен)
    let lower = token.to_lowercase();
    for var in &ctx.vars {
        if var.to_lowercase().starts_with(&lower) {
            push(HintItem {
                kind: HintKind::Var,
                insert: var.clone(),
                label: var.clone(),
                detail: i18n::tr(language, keys::HINT_VAR).to_owned(),
            });
        }
    }
    for hint in expr::fn_hints() {
        if hint.name.starts_with(&lower) {
            push(HintItem {
                kind: HintKind::Fn,
                insert: format!("{}(", hint.name),
                label: hint.name.to_owned(),
                detail: hint.signature.to_owned(),
            });
        }
    }
    for unit in expr::unit_tokens() {
        if unit.to_lowercase().starts_with(&lower) {
            push(HintItem {
                kind: HintKind::Unit,
                insert: unit.to_owned(),
                label: unit.to_owned(),
                detail: i18n::tr(language, keys::HINT_UNIT).to_owned(),
            });
        }
    }
    items
}

/// Состояние popup подсказок (открыт/список/выбор). Хранится в `App`,
/// сбрасывается при начале/завершении редактирования.
#[derive(Debug, Clone, Default)]
pub struct HintPopup {
    pub open: bool,
    pub items: Vec<HintItem>,
    pub selected: usize,
    /// Токен слева от каретки, который замещает принятая подсказка.
    pub token: String,
    /// Якорь popup в логических px окна — низ каретки.
    pub anchor: [f32; 2],
}

impl HintPopup {
    /// Закрыть и очистить.
    pub fn reset(&mut self) {
        self.open = false;
        self.items.clear();
        self.selected = 0;
        self.token.clear();
    }

    /// Обновить список после правки текста: выделение сохраняется, если
    /// влезает; пустой список закрывает popup.
    pub fn sync(&mut self, token: String, items: Vec<HintItem>) {
        self.token = token;
        if self.selected >= items.len() {
            self.selected = 0;
        }
        self.items = items;
        self.open = !self.items.is_empty();
    }

    /// Сдвиг выделения с закольцовыванием; true — было изменение.
    pub fn move_selection(&mut self, delta: i32) -> bool {
        if self.items.is_empty() {
            return false;
        }
        let len = self.items.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        if next == self.selected as i32 {
            return false;
        }
        self.selected = next as usize;
        true
    }

    /// Выбранный элемент (для принятия).
    pub fn selected_item(&self) -> Option<&HintItem> {
        self.items.get(self.selected)
    }
}

/// Геометрия popup — `kit::dropdown_menu` («якорь + flip», FR-059):
/// ниже якоря; не влезает снизу — НАД строкой каретки (flip кита);
/// по горизонтали зажат во вьюпорт с полями [`HINT_MARGIN`].
/// `count == 0` — `None` (popup не открывается).
/// Ширина — [`HINT_WIDTH`], зажатая во вьюпорт (узкие окна).
pub fn popup_rect(anchor: [f32; 2], window: [f32; 2], count: usize) -> Option<UiRect> {
    if count == 0 {
        return None;
    }
    let viewport = UiRect::new(
        HINT_MARGIN,
        HINT_MARGIN,
        (window[0] - HINT_MARGIN * 2.0).max(0.0),
        (window[1] - HINT_MARGIN * 2.0).max(0.0),
    );
    let height = count as f32 * HINT_ROW_H + HINT_MARGIN * 2.0;
    let content = constrain(
        UiVec2::new(0.0, 0.0),
        UiVec2::new(viewport.w, viewport.h),
        UiVec2::new(HINT_WIDTH, height),
    );
    // Якорь — строка каретки: низ каретки (`anchor`) — низ строки высотой
    // [`HINT_CARET_LINE_H`]; ниже — `+DROPDOWN_GAP` (прежние +4), flip —
    // над строкой (прежние «−20» = 16 + 4).
    let anchor_rect = UiRect::new(
        anchor[0],
        anchor[1] - HINT_CARET_LINE_H,
        1.0,
        HINT_CARET_LINE_H,
    );
    Some(kit::dropdown_menu(anchor_rect, viewport, content).menu)
}

/// Строки popup — `kit::list_rows` ([`ScrollState`], окно без прокрутки:
/// контент = [`HINT_LIMIT`]·[`HINT_ROW_H`] максимум, зазор 0 — прежняя
/// стопка дословно). Возвращает `(индекс, rect подсветки)`; rect —
/// прежняя зона подсветки `[px+4, row_y, pw−8, 24]`.
pub fn hint_rows(popup: UiRect, count: usize) -> Vec<(usize, UiRect)> {
    let area = UiRect::new(
        popup.x + HINT_ROW_INSET_H,
        popup.y + HINT_MARGIN,
        (popup.w - HINT_ROW_INSET_H * 2.0).max(0.0),
        (popup.h - HINT_MARGIN * 2.0).max(0.0),
    );
    let scroll = ScrollState {
        offset: 0.0,
        content_h: count as f32 * HINT_ROW_H,
        viewport_h: area.h,
    };
    kit::list_rows(area, &scroll, HINT_ROW_H, 0.0, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> HintContext {
        HintContext {
            vars: vec!["rps".to_owned(), "rate".to_owned()],
            inbound: 2,
            params: vec!["service_rate".to_owned()],
        }
    }

    /// Префикс `mm` → mm1/mmc (регистр не важен); функции — со скобкой.
    #[test]
    fn hints_filter_by_prefix() {
        let items = hint_items("w = mm", &ctx(), Language::Ru);
        let names: Vec<&str> = items
            .iter()
            .filter(|item| item.kind == HintKind::Fn)
            .map(|item| item.label.as_str())
            .collect();
        assert_eq!(names, vec!["mm1", "mmc"]);
        let mm1 = items.iter().find(|item| item.label == "mm1").unwrap();
        assert_eq!(mm1.insert, "mm1(");
        assert_eq!(mm1.detail, "mm1(λ, μ[, c])"); // сигнатура функции — из каталога движка, не переводится
                                                  // Регистр не важен
        let items = hint_items("w = MM", &ctx(), Language::Ru);
        assert!(items.iter().any(|item| item.label == "mm1"));
    }

    /// Пустой токен на Numi-строке — переменные ноды; идентификатор —
    /// переменные + функции + единицы по префиксу.
    #[test]
    fn hints_vars_and_units_order() {
        // Пустой токен (строка кончается не числом) → только переменные
        let items = hint_items("rps = 1000 rps\n", &ctx(), Language::Ru);
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.kind == HintKind::Var));
        assert!(items.iter().any(|item| item.label == "rps"));
        // Идентификатор `s`: переменная rate? нет — но unit `s`/`sec`,
        // функций нет; переменные по префиксу — нет подходящих из ctx
        let items = hint_items("w = s", &ctx(), Language::Ru);
        let kinds: Vec<HintKind> = items.iter().map(|item| item.kind.clone()).collect();
        assert!(
            kinds.contains(&HintKind::Unit),
            "единицы после идентификатора"
        );
        // Лимит списка
        let items = hint_items("", &ctx(), Language::Ru);
        assert!(items.len() <= HINT_LIMIT);
    }

    /// Токен `$` → `$in`/`$1..$N`/параметры шаблона; без inbound — `$in`
    /// нет.
    #[test]
    fn hints_dollar_refs() {
        let items = hint_items("w = $", &ctx(), Language::Ru);
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.contains(&"$in"));
        assert!(labels.contains(&"$1"));
        assert!(labels.contains(&"$2"));
        assert!(labels.contains(&"service_rate"));
        // Фильтр по префиксу после `$`
        let items = hint_items("w = $se", &ctx(), Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "$service_rate");
        // Без inbound и шаблона — `$`-подсказок нет
        let empty = HintContext::default();
        assert!(hint_items("w = $", &empty, Language::Ru).is_empty());
    }

    /// Число и число+пробел → единицы; вставка замещает только число.
    #[test]
    fn hints_units_after_number() {
        let items = hint_items("w = 50 ", &ctx(), Language::Ru);
        assert!(!items.is_empty());
        assert!(items.iter().all(|item| item.kind == HintKind::Unit));
        assert!(items.iter().any(|item| item.label == "sec"));
        // Число частично: `50 s`е... — токен числа целиком → единицы
        let items = hint_items("w = 50", &ctx(), Language::Ru);
        assert!(items.iter().all(|item| item.kind == HintKind::Unit));
        // `token_before_caret` возвращает число целиком
        let (token, start) = token_before_caret("w = 50", 6);
        assert_eq!((token.as_str(), start), ("50", 4));
    }

    /// Кириллический токен перед кареткой фильтрует единицы (правка
    /// 2026-09-23: класс символов совпадает с лексером движка — буквы
    /// Unicode). Вставка замещает кириллический префикс целиком.
    #[test]
    fn hints_cyrillic_unit_prefix() {
        let items = hint_items("w = 50 се", &ctx(), Language::Ru);
        assert_eq!(
            items
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            vec!["сек"],
            "префикс `се` — только кириллическая секунда"
        );
        assert_eq!(items[0].insert, "сек");
        // Токен и его байтовый якорь — для замещения в редакторе
        let (token, start) = token_before_caret("w = 50 се", "w = 50 се".len());
        assert_eq!(token, "се");
        assert_eq!(start, "w = 50 ".len(), "якорь — байтовая позиция `се`");
        // Регистр не важен; байтовые единицы тоже фильтруются
        let items = hint_items("w = 50 ГБ", &ctx(), Language::Ru);
        assert!(items.iter().any(|item| item.label == "ГБ"));
    }

    /// Прозаический контекст даёт пусто (детектор рода строки — на
    /// стороне вызывающего; тут проверяем фильтрацию мусорного токена).
    #[test]
    fn hints_unknown_token_is_empty() {
        let items = hint_items("встреча в 3", &ctx(), Language::Ru);
        assert!(items.is_empty(), "проза не подсказывает");
    }

    /// `popup_rect` (FR-059: `kit::dropdown_menu`): якорь+flip — числа
    /// прежней раскладки дословно; 0 элементов — `None`.
    #[test]
    fn hints_popup_layout_clamps_to_window() {
        assert!(popup_rect([100.0, 100.0], [800.0, 600.0], 0).is_none());
        let rect = popup_rect([100.0, 100.0], [800.0, 600.0], 4).expect("popup есть");
        assert_eq!(rect.w, HINT_WIDTH);
        assert!((rect.h - (4.0 * HINT_ROW_H + HINT_MARGIN * 2.0)).abs() < 0.01);
        // Ниже якоря: прежний шаг «+4» от низа каретки
        assert!((rect.y - (100.0 + 4.0)).abs() < 0.01);
        // У правого края — сдвиг внутрь (кламп во вьюпорт с полями)
        let rect = popup_rect([790.0, 100.0], [800.0, 600.0], 4).expect("popup есть");
        assert!(rect.right() <= 800.0 - HINT_MARGIN + 0.01);
        // У нижнего края — flip НАД строкой каретки: прежняя формула
        // «anchor − высота − 20» (строка 16 + зазор 4)
        let rect = popup_rect([100.0, 590.0], [800.0, 600.0], 4).expect("popup есть");
        assert!(rect.bottom() <= 600.0 - HINT_MARGIN + 0.01);
        assert!((rect.y - (590.0 - 4.0 * HINT_ROW_H - HINT_MARGIN * 2.0 - 20.0)).abs() < 0.01);
    }

    /// Строки popup (`kit::list_rows`): прежняя стопка дословно —
    /// подсветка `[px+4, py+6+i·24, pw−8, 24]`, окно без прокрутки.
    #[test]
    fn hints_rows_via_list_rows_match_old_stack() {
        let popup = popup_rect([100.0, 100.0], [800.0, 600.0], 3).expect("popup есть");
        let rows = hint_rows(popup, 3);
        assert_eq!(rows.len(), 3);
        for (i, (idx, rect)) in rows.iter().enumerate() {
            assert_eq!(*idx, i);
            assert!((rect.x - (popup.x + HINT_ROW_INSET_H)).abs() < 0.01);
            assert!((rect.y - (popup.y + HINT_MARGIN + i as f32 * HINT_ROW_H)).abs() < 0.01);
            assert!((rect.w - (popup.w - HINT_ROW_INSET_H * 2.0)).abs() < 0.01);
            assert!((rect.h - HINT_ROW_H).abs() < 0.01);
        }
        // Лимит: 8 строк максимум — окно списка без прокрутки
        let popup = popup_rect([100.0, 100.0], [800.0, 600.0], HINT_LIMIT).expect("popup есть");
        assert_eq!(hint_rows(popup, HINT_LIMIT).len(), HINT_LIMIT);
    }

    /// Клавиатурный контракт popup: выделение закольцовано, sync сохраняет
    /// выделение, пустой список закрывает.
    #[test]
    fn hints_popup_keyboard_model() {
        let mut popup = HintPopup::default();
        let items = hint_items("w = mm", &ctx(), Language::Ru);
        popup.sync("mm".to_owned(), items);
        assert!(popup.open);
        assert!(popup.move_selection(1));
        assert_eq!(popup.selected, 1);
        assert!(popup.move_selection(-1));
        assert_eq!(popup.selected, 0);
        // Закольцовывание назад от 0 — последний
        let count = popup.items.len();
        assert!(popup.move_selection(-1));
        assert_eq!(popup.selected, count - 1);
        popup.move_selection(1);
        assert_eq!(popup.selected, 0);
        // Выбранный элемент — подсказка mm1
        assert_eq!(popup.selected_item().unwrap().label, "mm1");
        // Обновление с сохранением выделения; пустой список закрывает
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        assert_eq!(popup.selected, 0, "выделение вне границ сбрасывается");
        popup.reset();
        assert!(!popup.open);
        popup.sync("".to_owned(), Vec::new());
        assert!(!popup.open, "пустой список закрывает popup");
    }
}
