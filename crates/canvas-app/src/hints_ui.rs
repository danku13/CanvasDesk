//! FR-021: контекстные подсказки при Numi-вводе — чистая модель
//! (образец [`crate::template_ui`]): токен перед кареткой, фильтрация
//! каталога движка, клавиатурный popup, кламп геометрии к окну.
//!
//! Рендер и перехват клавиш — приложение (`main.rs`): popup собирается
//! по кадру из квадов + screen-текстов (паттерн `wheel_overlay`), а
//! принятие подсказки заменяет токен слева от каретки
//! ([`canvas_render::edit::EditingSession::replace_token_before_caret`]).

use canvas_core::{expr, Language};

use crate::i18n::{self, keys};

/// Максимум элементов в popup (читабельность, стандарт автокомплитов).
pub const HINT_LIMIT: usize = 8;
/// Ширина popup (логические px, клампится к окну).
pub const HINT_WIDTH: f32 = 300.0;
/// Высота строки popup.
pub const HINT_ROW_H: f32 = 24.0;
/// Внутренние поля popup.
pub const HINT_MARGIN: f32 = 6.0;

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
/// Токен — `$`+идентификатор, идентификатор (`[A-Za-z_0-9]`) или число;
/// любой другой символ (пробел, оператор) обрывает токен. Пустая строка
/// — подсказки по контексту (переменные/единицы после числа).
pub fn token_before_caret(line: &str, caret: usize) -> (String, usize) {
    let caret = caret.min(line.len());
    let prefix = &line[..caret];
    let mut start = caret;
    for (i, ch) in prefix.char_indices().rev() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            start = i;
        } else if ch == '$' {
            start = i;
            break;
        } else {
            break;
        }
    }
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

/// Геометрия popup `[x, y, w, h]` с клампом к окну: ниже якоря; не
/// влезает снизу — выше строки каретки. `count == 0` — пустой rect
/// (popup не открывается).
pub fn popup_layout(anchor: [f32; 2], window: [f32; 2], count: usize) -> [f32; 4] {
    if count == 0 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let height = count as f32 * HINT_ROW_H + HINT_MARGIN * 2.0;
    let width = HINT_WIDTH.min((window[0] - HINT_MARGIN * 2.0).max(0.0));
    let mut x = anchor[0];
    if x + width > window[0] - HINT_MARGIN {
        x = window[0] - HINT_MARGIN - width;
    }
    let x = x.max(HINT_MARGIN);
    let mut y = anchor[1] + 4.0;
    if y + height > window[1] - HINT_MARGIN {
        y = anchor[1] - height - 20.0; // выше строки каретки
    }
    let y = y.max(HINT_MARGIN);
    [x, y, width, height]
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

    /// Прозаический контекст даёт пусто (детектор рода строки — на
    /// стороне вызывающего; тут проверяем фильтрацию мусорного токена).
    #[test]
    fn hints_unknown_token_is_empty() {
        let items = hint_items("встреча в 3", &ctx(), Language::Ru);
        assert!(items.is_empty(), "проза не подсказывает");
    }

    /// `popup_layout`: кламп к окну; у нижнего края — выше якоря; 0
    /// элементов — пустой rect.
    #[test]
    fn hints_popup_layout_clamps_to_window() {
        assert_eq!(popup_layout([100.0, 100.0], [800.0, 600.0], 0), [0.0; 4]);
        let rect = popup_layout([100.0, 100.0], [800.0, 600.0], 4);
        assert_eq!(rect[2], HINT_WIDTH);
        assert!((rect[3] - (4.0 * HINT_ROW_H + HINT_MARGIN * 2.0)).abs() < 0.01);
        // У правого края — сдвиг внутрь
        let rect = popup_layout([790.0, 100.0], [800.0, 600.0], 4);
        assert!(rect[0] + rect[2] <= 800.0 - HINT_MARGIN + 0.01);
        // У нижнего края — выше якоря
        let rect = popup_layout([100.0, 590.0], [800.0, 600.0], 4);
        assert!(rect[1] + rect[3] <= 600.0 - HINT_MARGIN + 0.01);
        assert!(rect[1] + rect[3] < 590.0, "popup выше строки каретки");
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
