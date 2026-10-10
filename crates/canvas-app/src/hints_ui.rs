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
/// FR-101: номера `$1..$N` свёрнуто при N больше этой границы — имена
/// входов главнее номеров (приоритизация под [`HINT_LIMIT`]); `$in`
/// покрывает агрегат, прямой ввод `$5` работает и без подсказки.
const HINT_NUMBER_CAP: usize = 4;
/// Ширина popup (логические px; у узкого окна зажимается во вьюпорт —
/// `constrain`-семантика `dropdown_menu`).
pub const HINT_WIDTH: f32 = 300.0;
/// Высота строки popup.
pub const HINT_ROW_H: f32 = 24.0;
/// Внутренние поля popup (по вертикали; по горизонтали подсветка строки
/// инсетится на [`HINT_ROW_INSET_H`]) — токен `canvas_core::tokens::SPACING_S`
/// (значение прежнего литерала 6).
pub use canvas_core::tokens::SPACING_S as HINT_MARGIN;
/// Горизонтальный инсет подсветки строки от краёв popup (прежние +4/−8) —
/// 4 px hairline, вне шкалы S1 (исключение LAY7 — «Исключения» 11-layouts.md,
/// «Hairline-микрозначения 2–4 px», LAY-W16: визуальный инсет подсветки,
/// 4→6 разъезжает её с краями popup).
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
    /// FR-079 (S3): ИИ-строка — шаблон каталога из suggest-движка.
    /// Принятие НЕ вставляет текст: редактируемая нода заменяется
    /// инстансом шаблона ([`HintItem::template`]).
    Ai,
}

/// Элемент подсказки: что вставить и как подписать.
#[derive(Debug, Clone, PartialEq)]
pub struct HintItem {
    pub kind: HintKind,
    /// Текст вставки (замещает токен слева от каретки); функция — с
    /// открывающей скобкой. Для [`HintKind::Ai`] — имя шаблона (не
    /// вставляется текстом).
    pub insert: String,
    /// Главная подпись.
    pub label: String,
    /// Серая деталь: сигнатура / описание / «переменная».
    pub detail: String,
    /// FR-079 (S3): ключ шаблона для [`HintKind::Ai`] (принятие = замена
    /// редактируемой ноды инстансом шаблона); `None` — текстовая вставка.
    pub template: Option<String>,
}

impl HintItem {
    /// Текстовая подсказка (L0 FR-021): вставка замещает токен.
    pub fn text(kind: HintKind, insert: String, label: String, detail: String) -> Self {
        Self {
            kind,
            insert,
            label,
            detail,
            template: None,
        }
    }
}

/// Контекст ноды для подсказок.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HintContext {
    /// Переменные листа (присваивания ВЫШЕ строки каретки).
    pub vars: Vec<String>,
    /// Число позиционных value-рёбер на ноде (`$in`/`$1..$N` — FR-014;
    /// FR-101: рёбра с `to_param` НЕ считаются — зеркало фильтра слотов
    /// `flow::inbound_slots_with_lines`, иначе попап предлагал `$N`,
    /// которого в eval не существует).
    pub inbound: usize,
    /// Имена параметров шаблона (`$rps`… — FR-018); нешаблонная нода —
    /// пусто.
    pub params: Vec<String>,
    /// FR-101: именованные входы — ссылки на входящие value-рёбра по
    /// имени (и в `$`-ветке, и по имени без `$`). Собирается в
    /// `update_hints`: имена `to_param`-спиллов и qualified-ключи
    /// адресных рёбер (`Нода.имя_присваивания`, единая точка
    /// `flow::source_line_name`).
    pub inbounds: Vec<InboundHint>,
}

/// FR-101: именованный вход ноды — единица подсказок входящих параметров.
/// Один вход = одна строка popup с рабочей формой вставки: формат — по
/// языку имени (совпадает с языком набранного хвоста: кириллический
/// префикс матчит только кириллическое имя) — ASCII → `$имя`, кириллица →
/// `Нода.параметр` (грамматика FR-050 Р-6; `$`+кириллица = валюта,
/// FR-013 — такая вставка не может быть предложена).
#[derive(Debug, Clone, PartialEq)]
pub struct InboundHint {
    /// Имя для матчинга: имя параметра спилла (`toParam`) или поле
    /// qualified-ключа (имя присваивания строки-истока / `fromOutput`).
    pub name: String,
    /// Qualified-форма «Нода.поле» — запасная (для кириллицы — рабочая)
    /// форма вставки; пустая строка — адресованного имени у ребра нет.
    pub qualified: String,
    /// Заголовок ноды-источника — деталь подсказки «проливание из ноды X».
    pub source: String,
    /// Ребро проливает в параметр (`toParam`): `$имя` резолвится
    /// рантаймом (каскад параметров `Env::with_param_map`, flow.rs).
    /// Для позиционного ребра имя в `$`-форме не резолвится — только
    /// qualified-форма.
    pub spill: bool,
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

/// FR-102: контекст точки — «Объект.» или «Объект.поля» непосредственно
/// перед кареткой. Возвращает `(имя объекта, набранный префикс поля)`.
/// НЕ контекст (попап полей не открывается): число перед точкой («2.» —
/// семантика дробного числа/единиц, expr.rs FR-050 Р-6), `$`-токен перед
/// точкой («$имя.» — валюта/параметр, полей не имеет), пробел между
/// объектом и точкой (лексер соберёт qualified-путь только вплотную —
/// `dot_field_starts`; «Объект .» — не начало qualified), пустой объект
/// (точка без идентификатора). Объект — идентификатор до точки (буквы
/// Unicode/цифры/`_`, начинается с буквы/`_`), префикс поля — токен
/// после точки (может быть пуст — только точка набрана).
pub fn dot_field_context(line_prefix: &str) -> Option<(String, String)> {
    let (token, start) = token_before_caret(line_prefix, line_prefix.len());
    let before = &line_prefix[..start];
    if before.chars().next_back()? != '.' {
        return None;
    }
    let obj_end = before.len() - '.'.len_utf8();
    let obj_start = before[..obj_end]
        .char_indices()
        .rev()
        .take_while(|&(_, ch)| ch.is_alphanumeric() || ch == '_')
        .last()
        .map(|(i, _)| i)
        .unwrap_or(obj_end);
    let obj = &before[obj_start..obj_end];
    // Объект начинается с буквы/`_` (число перед точкой — не объект)
    let first = obj.chars().next()?;
    if !(first.is_alphabetic() || first == '_') {
        return None;
    }
    // `$`-токен перед точкой — валюта/параметр (FR-013), полей не имеет
    if before[..obj_start].ends_with('$') {
        return None;
    }
    Some((obj.to_owned(), token))
}

/// FR-101: рабочая форма вставки именованного входа — `Some(текст)` или
/// `None` (ни одной грамматически валидной формы — подсказка не
/// показывается: битая вставка хуже тишины).
fn inbound_insert(inbound: &InboundHint) -> Option<String> {
    // Спилл: `$имя` — основная форма (ASCII-имена резолвятся рантаймом)
    if inbound.spill {
        if let Some(form) = dollar_form(&inbound.name) {
            return Some(form);
        }
    }
    // Qualified «Нода.поле» — рабочая форма кириллицы и позиционных рёбер
    if qualified_form_is_lexable(&inbound.qualified) {
        return Some(inbound.qualified.clone());
    }
    None
}

/// FR-101: форма `$имя` — зеркало грамматики `dollar_param_ident`
/// (`expr.rs`: ASCII-идентификатор `[a-zA-Z_][a-zA-Z0-9_]*`; имя с
/// префиксом `in` зарезервировано валютной семантикой FR-013 — `$inn`).
fn dollar_form(name: &str) -> Option<String> {
    let first = name.chars().next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    if name.starts_with("in") {
        return None;
    }
    Some(format!("${name}"))
}

/// FR-101: вставляется ли qualified-строка лексером движка (FR-050 Р-6):
/// объект — лексический идентификатор без пробелов (алиасы коллизий
/// «Имя (id)» с пробелом не подсказываются — ручной ввод), поле —
/// идентификатор с внутренними дефисами («Кол-во», `lex_qualified_field`).
fn qualified_form_is_lexable(qualified: &str) -> bool {
    let Some((obj, field)) = qualified.split_once('.') else {
        return false;
    };
    let obj_ok = !obj.is_empty() && obj.chars().all(|ch| ch.is_alphanumeric() || ch == '_');
    let field_ok = !field.is_empty()
        && !field.starts_with('-')
        && !field.ends_with('-')
        && !field.contains("--")
        && field
            .chars()
            .all(|ch| ch.is_alphanumeric() || ch == '_' || ch == '-');
    obj_ok && field_ok
}

/// CR-029-A: совпадает ли префикс с ИМЕНЕМ ОБЪЕКТА qualified-ключа
/// («Корзина» в «Корзина.sum»). Имя поля может не совпадать с именем
/// ноды-источника (латиница под кириллицей) — а пользователь вводит
/// именно то, что видит в теле ноды («Корзина . sum»). Объект — часть
/// до первой точки; пустой объект (qualified без точки) не матчится.
fn inbound_object_matches_prefix(inbound: &InboundHint, lower: &str) -> bool {
    inbound
        .qualified
        .split_once('.')
        .map(|(obj, _)| !obj.is_empty() && obj.to_lowercase().starts_with(lower))
        .unwrap_or(false)
}

/// Один именованный вход в список (если есть рабочая форма вставки).
fn push_inbound(inbound: &InboundHint, language: Language, push: &mut dyn FnMut(HintItem)) {
    let Some(insert) = inbound_insert(inbound) else {
        return;
    };
    push(HintItem::text(
        HintKind::DollarRef,
        insert.clone(),
        insert,
        i18n::trf(
            language,
            keys::HINT_SPILL,
            &[("{node}", inbound.source.as_str())],
        ),
    ));
}

/// Элементы подсказок для строки каретки (префикс ДО каретки) и контекста
/// ноды. Порядок (proposal FR-021, FR-101): переменные → именованные
/// входы → параметры шаблона → функции → единицы; лимит
/// [`HINT_LIMIT`]. Единицы — после числа (токен-число или число+пробел);
/// `$`-токен: именованные входы (матчинг по имени без `$`) → `$in`/`$N`
/// → параметры; пустой токен — переменные. Дедуп по вставке: спилл и
/// параметр манифеста с одним именем — одна строка.
pub fn hint_items(line_prefix: &str, ctx: &HintContext, language: Language) -> Vec<HintItem> {
    let (token, _start) = token_before_caret(line_prefix, line_prefix.len());
    let mut items: Vec<HintItem> = Vec::with_capacity(HINT_LIMIT);
    let mut push = |item: HintItem| {
        // FR-101: дедуп по вставке — одна строка на одну рабочую ссылку
        if items.len() < HINT_LIMIT && !items.iter().any(|existing| existing.insert == item.insert)
        {
            items.push(item);
        }
    };
    // `$`-контекст: именованные входы, входы value-рёбер, параметры шаблона
    if let Some(name) = token.strip_prefix('$') {
        let lower = name.to_lowercase();
        // FR-101: именованные входы — раньше `$in`/номеров (ввод `$куп`
        // находит `Купон.купон`); форма вставки — по языку имени.
        // CR-029-A: второй проход — по имени ОБЪЕКТА («$Корз» →
        // «Корзина.sum»); матч по полю ранжируется раньше, дедуп по
        // вставке отсекает вход, найденный обоими проходами.
        for inbound in &ctx.inbounds {
            if inbound.name.to_lowercase().starts_with(&lower) {
                push_inbound(inbound, language, &mut push);
            }
        }
        for inbound in &ctx.inbounds {
            if !inbound.name.to_lowercase().starts_with(&lower)
                && inbound_object_matches_prefix(inbound, &lower)
            {
                push_inbound(inbound, language, &mut push);
            }
        }
        if ctx.inbound > 0 && "in".starts_with(&lower) {
            push(HintItem::text(
                HintKind::DollarRef,
                "$in".to_owned(),
                "$in".to_owned(),
                i18n::tr(language, keys::HINT_DOLLAR_IN).to_owned(),
            ));
        }
        // FR-101: номера свёрнуто при N > [`HINT_NUMBER_CAP`]
        for i in 1..=ctx.inbound.min(HINT_NUMBER_CAP) {
            let label = format!("${i}");
            if label.starts_with(&token) {
                push(HintItem::text(
                    HintKind::DollarRef,
                    label.clone(),
                    label,
                    i18n::trf(language, keys::HINT_DOLLAR_N, &[("{i}", &i.to_string())]),
                ));
            }
        }
        // Параметры манифеста — после `$in`/`$N` (порядок FR-021 v1): при
        // полном списке лимит срезает хвост, а не номера слотов
        for param in &ctx.params {
            let label = format!("${param}");
            if label.to_lowercase().starts_with(&token.to_lowercase()) {
                push(HintItem::text(
                    HintKind::DollarRef,
                    label.clone(),
                    param.clone(),
                    i18n::tr(language, keys::HINT_PARAM).to_owned(),
                ));
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
                push(HintItem::text(
                    HintKind::Unit,
                    unit.to_owned(),
                    unit.to_owned(),
                    i18n::tr(language, keys::HINT_UNIT).to_owned(),
                ));
            }
        }
        return items;
    }
    // FR-102: после точки — ПОЛЯ объекта (qualified-ключи входящих
    // value-рёбер). Ветка раньше переменных/идентификатора и завершает
    // разбор: после точки переменные/функции/единицы неуместны, а
    // вставка — только имя поля (дописывает набираемый путь «Объект. →
    // Объект.поле»; замена токена `replace_token_before_caret` — хвост
    // после точки, полный qualified дал бы «Объект.Нода.поле»).
    if let Some((obj, field_prefix)) = dot_field_context(line_prefix) {
        let lower_obj = obj.to_lowercase();
        let lower_field = field_prefix.to_lowercase();
        for inbound in &ctx.inbounds {
            let Some((inbound_obj, field)) = inbound.qualified.split_once('.') else {
                continue;
            };
            if inbound_obj.is_empty()
                || field.is_empty()
                || inbound_obj.to_lowercase() != lower_obj
                || !field.to_lowercase().starts_with(&lower_field)
            {
                continue;
            }
            push(HintItem::text(
                HintKind::DollarRef,
                field.to_owned(),
                field.to_owned(),
                i18n::trf(
                    language,
                    keys::HINT_FIELD,
                    &[("{node}", inbound.source.as_str())],
                ),
            ));
        }
        return items;
    }
    if token.is_empty() {
        // Пустой токен на Numi-строке — переменные ноды
        for var in &ctx.vars {
            push(HintItem::text(
                HintKind::Var,
                var.clone(),
                var.clone(),
                i18n::tr(language, keys::HINT_VAR).to_owned(),
            ));
        }
        return items;
    }
    // Идентификатор: переменные → именованные входы (FR-101) → параметры
    // шаблона → функции → единицы (регистр не важен)
    let lower = token.to_lowercase();
    for var in &ctx.vars {
        if var.to_lowercase().starts_with(&lower) {
            push(HintItem::text(
                HintKind::Var,
                var.clone(),
                var.clone(),
                i18n::tr(language, keys::HINT_VAR).to_owned(),
            ));
        }
    }
    // FR-101: имена входов без `$` — триггер по имени обязателен
    // (решение владельца Q4); вставка — рабочая форма.
    // CR-029-A: второй проход — триггер по префиксу ИМЕНИ ОБЪЕКТА
    // qualified-ключа («Корз» → «Корзина.sum»): у части входов имя поля
    // не совпадает с именем ноды-источника, и до фикса попап молчал
    // ровно на них (у «Купон.купон» совпадает — отсюда «иногда»).
    // Матч по полю ранжируется раньше объекта; дедуп по вставке
    // отсекает вход, найденный обоими проходами.
    for inbound in &ctx.inbounds {
        if inbound.name.to_lowercase().starts_with(&lower) {
            push_inbound(inbound, language, &mut push);
        }
    }
    for inbound in &ctx.inbounds {
        if !inbound.name.to_lowercase().starts_with(&lower)
            && inbound_object_matches_prefix(inbound, &lower)
        {
            push_inbound(inbound, language, &mut push);
        }
    }
    // FR-101: параметры манифеста — тоже по имени (вставка `$имя`)
    for param in &ctx.params {
        if param.to_lowercase().starts_with(&lower) {
            push(HintItem::text(
                HintKind::DollarRef,
                format!("${param}"),
                param.clone(),
                i18n::tr(language, keys::HINT_PARAM).to_owned(),
            ));
        }
    }
    for hint in expr::fn_hints() {
        if hint.name.starts_with(&lower) {
            push(HintItem::text(
                HintKind::Fn,
                format!("{}(", hint.name),
                hint.name.to_owned(),
                hint.signature.to_owned(),
            ));
        }
    }
    for unit in expr::unit_tokens() {
        if unit.to_lowercase().starts_with(&lower) {
            push(HintItem::text(
                HintKind::Unit,
                unit.to_owned(),
                unit.to_owned(),
                i18n::tr(language, keys::HINT_UNIT).to_owned(),
            ));
        }
    }
    items
}

/// FR-079 (S3): прогрессивный мердж — L0-строки сверху, ИИ-строки ниже
/// (источник виден пользователю), общий лимит [`HINT_LIMIT`]. Дедуп по
/// `insert` (ИИ-шаблон не дублирует совпавшую L0-строку). Пустой итог —
/// попап закрыт (семантика [`HintPopup::sync`]).
/// CR-022: из пользовательского пути НЕДОСТИЖИМА — [`crate::app`]
/// `suggest_remerge` гейтится флагом `suggest.c1_in_popup` (default false,
/// решение владельца «вообще не надо триггерить»). Функция СОХРАНЕНА для
/// тестов и будущих поверхностей — контракт не трогать без CR.
pub fn merge_ai_items(l0: Vec<HintItem>, ai: Vec<HintItem>) -> Vec<HintItem> {
    let mut items = l0;
    for item in ai {
        if items.len() >= HINT_LIMIT {
            break;
        }
        if !items.iter().any(|i| i.insert == item.insert) {
            items.push(item);
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
    /// CR-023 (UR-001-08): токен, на котором попап подавлен — принятая
    /// подсказка или Esc закрыли свою тему; [`HintPopup::sync`] держит
    /// попап закрытым, пока токен перед кареткой совпадает. Смена токена
    /// (ввод/удаление символа) снимает подавление.
    suppressed: Option<String>,
    /// CR-023: подавление взведено принятием ([`HintPopup::arm_suppress`]),
    /// но фактический токен после вставки ещё не известен — фиксируется
    /// на первом [`HintPopup::sync`] (вставка «mm1(» даёт токен «mm1» —
    /// считает `token_before_caret`, а не догадка места вызова).
    pending_suppress: bool,
}

impl HintPopup {
    /// Закрыть и очистить.
    pub fn reset(&mut self) {
        self.open = false;
        self.items.clear();
        self.selected = 0;
        self.token.clear();
        // CR-023: новая тема редактирования — подавление не переживает reset
        self.suppressed = None;
        self.pending_suppress = false;
    }

    /// CR-023: взвести подавление после принятия подсказки. Фактический
    /// токен после вставки неизвестен здесь — его зафиксирует ближайший
    /// [`HintPopup::sync`] (замена токена — backspace-семантика + вставка,
    /// итоговый токен считает `update_hints`).
    pub fn arm_suppress(&mut self) {
        self.pending_suppress = true;
        self.open = false;
    }

    /// CR-023: Esc — закрыть попап и подавить его на текущем токене до
    /// смены текста перед кареткой (единая семантика с подавлением после
    /// принятия; контракт FR-021). Список сохраняется.
    pub fn dismiss(&mut self) {
        self.pending_suppress = false;
        self.suppressed = Some(self.token.clone());
        self.open = false;
    }

    /// CR-029-B: снять подавление попапа — явная вставка (Paste) является
    /// новым пользовательским действием, отличным от вставки принятой
    /// подсказки: после неё попап оценивается заново, даже если токен
    /// перед кареткой совпал с подавленным (принятие/Esc). Вызывается
    /// приложением в ветке `KeyCommand::Paste` перед `update_hints`.
    pub fn lift_suppression(&mut self) {
        self.suppressed = None;
        self.pending_suppress = false;
    }

    /// Обновить список после правки текста: выделение сохраняется, если
    /// влезает; пустой список закрывает popup.
    /// CR-023: подавленный токен держит попап закрытым — принятое
    /// автодополнение не навязывается повторно, пока токен перед кареткой
    /// не изменится (движение каретки без правки — тишина).
    pub fn sync(&mut self, token: String, items: Vec<HintItem>) {
        // Первая синхронизация после принятия: фиксируем фактический токен
        // после вставки и подавляем его
        if self.pending_suppress {
            self.pending_suppress = false;
            self.suppressed = Some(token.clone());
        }
        // Подавление живёт ровно до смены токена
        let suppressed = self.suppressed.as_deref() == Some(token.as_str());
        if !suppressed {
            self.suppressed = None;
        }
        self.token = token;
        if self.selected >= items.len() {
            self.selected = 0;
        }
        self.items = items;
        // Список сохраняется и под подавлением — ручное открытие (если
        // появится по решению владельца) покажет его без пересчёта
        self.open = !self.items.is_empty() && !suppressed;
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
            vars: vec!["rps".to_owned(), "rate".to_owned(), "купон_лок".to_owned()],
            inbound: 2,
            params: vec!["service_rate".to_owned()],
            inbounds: vec![
                // спилл (toParam): кириллическое имя — рабочая форма qualified
                InboundHint {
                    name: "купон".to_owned(),
                    qualified: "Купон.купон".to_owned(),
                    source: "Купон".to_owned(),
                    spill: true,
                },
                // позиционное построчное ребро: только qualified-форма
                InboundHint {
                    name: "скидка".to_owned(),
                    qualified: "Скидка.скидка".to_owned(),
                    source: "Скидка".to_owned(),
                    spill: false,
                },
                // ASCII-спилл: рабочая форма `$имя`
                InboundHint {
                    name: "peak_rps".to_owned(),
                    qualified: "Профиль.peak_rps".to_owned(),
                    source: "Профиль трафика".to_owned(),
                    spill: true,
                },
            ],
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

    /// Токен `$` → именованные входы (FR-101) → `$in`/`$1..$N`/параметры
    /// шаблона; без inbound — `$in` нет.
    #[test]
    fn hints_dollar_refs() {
        let items = hint_items("w = $", &ctx(), Language::Ru);
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        // FR-101: именованные входы — qualified-форма спилла и построчного
        // ребра, параметры манифеста, `$`-форма ASCII-спилла
        assert!(labels.contains(&"Купон.купон"));
        assert!(labels.contains(&"Скидка.скидка"));
        assert!(labels.contains(&"$peak_rps"));
        assert!(labels.contains(&"service_rate"));
        assert!(labels.contains(&"$in"));
        assert!(labels.contains(&"$1"));
        assert!(labels.contains(&"$2"));
        // Порядок FR-101: имена входов раньше `$in`, `$in` раньше номеров
        let pos = |label: &str| labels.iter().position(|l| *l == label).expect(label);
        assert!(pos("Купон.купон") < pos("$in"), "имена раньше $in");
        assert!(pos("$in") < pos("$1"), "$in раньше номеров");
        // Фильтр по префиксу после `$`
        let items = hint_items("w = $se", &ctx(), Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "$service_rate");
        // FR-101 (кириллический кейс): `$купо` матчит по имени без учёта
        // регистра; вставка — рабочая qualified-форма (НЕ `$купон`:
        // `$`+кириллица = валюта, грамматика FR-013)
        let items = hint_items("w = $купо", &ctx(), Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "Купон.купон");
        assert_eq!(items[0].label, "Купон.купон");
        assert!(items[0].detail.contains("Купон"), "деталь — источник");
        // Без inbound и шаблона — `$`-подсказок нет
        let empty = HintContext::default();
        assert!(hint_items("w = $", &empty, Language::Ru).is_empty());
    }

    /// FR-101: триггер по имени без `$` — идентификаторный токен матчит
    /// имена входов; вставка — рабочая форма (кириллица → `Нода.параметр`,
    /// ASCII → `$имя`); порядок: переменные → входы → параметры → функции.
    #[test]
    fn hints_inbound_by_name_without_dollar() {
        // Кириллический ввод: переменная-тёзка раньше, вход — с qualified-
        // вставкой и источником в детали (репродуктор UR-001-05/07: «купо»)
        let items = hint_items("итог = купо", &ctx(), Language::Ru);
        assert_eq!(items.len(), 2, "переменная + именованный вход");
        assert_eq!(items[0].label, "купон_лок", "переменные раньше входов");
        assert_eq!(items[1].insert, "Купон.купон");
        assert_eq!(items[1].label, "Купон.купон");
        assert!(items[1].detail.contains("Купон"), "деталь — источник");
        // ASCII-ввод: спилл вставляется `$имя`-формой
        let items = hint_items("w = peak", &ctx(), Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "$peak_rps");
        assert!(items[0].detail.contains("Профиль"));
        // Параметр манифеста тоже матчится по имени (вставка `$имя`)
        let items = hint_items("w = service", &ctx(), Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "$service_rate");
    }

    /// FR-101: номера `$1..$N` свёрнуто при N > 4 — имена входов главнее
    /// номеров (приоритизация под [`HINT_LIMIT`]); при N ≤ 4 — все.
    #[test]
    fn hints_dollar_numbers_collapse_after_four() {
        let mut wide = ctx();
        wide.inbound = 6;
        let wide_labels: Vec<String> = hint_items("w = $", &wide, Language::Ru)
            .iter()
            .map(|item| item.label.clone())
            .collect();
        let labels: Vec<&str> = wide_labels.iter().map(|label| label.as_str()).collect();
        assert!(labels.contains(&"$1"));
        assert!(labels.contains(&"$4"));
        assert!(!labels.contains(&"$5"), "номера свёрнуто при N > 4");
        assert!(!labels.contains(&"$6"));
        // Малое N — без свёртывания
        let small_labels: Vec<String> = hint_items("w = $", &ctx(), Language::Ru)
            .iter()
            .map(|item| item.label.clone())
            .collect();
        assert!(small_labels.iter().any(|label| label == "$2"));
    }

    /// FR-101: дедуп именованных входов по вставке — спилл и параметр
    /// манифеста с одним именем дают одну строку попапа.
    #[test]
    fn hints_named_inputs_dedup() {
        let mut dup = ctx();
        dup.inbounds.push(InboundHint {
            name: "service_rate".to_owned(),
            qualified: "Профиль.service_rate".to_owned(),
            source: "Профиль".to_owned(),
            spill: true,
        });
        let dup_items = hint_items("w = $", &dup, Language::Ru);
        let inserts: Vec<&str> = dup_items.iter().map(|item| item.insert.as_str()).collect();
        assert_eq!(
            inserts.iter().filter(|i| **i == "$service_rate").count(),
            1,
            "одна строка на одну вставку"
        );
    }

    /// FR-101: невставляемые формы отсеиваются (битая вставка хуже
    /// тишины): `$имя` — зеркало `dollar_param_ident` (ASCII, не с
    /// префикса `in` — валюта FR-013); qualified — лексер «Объект.Поле»
    /// (объект без пробелов; алиас коллизии «Имя (id)» не вставляется).
    #[test]
    fn hints_inbound_unusable_forms_skipped() {
        let guarded = HintContext {
            vars: vec![],
            inbound: 1,
            params: vec![],
            inbounds: vec![
                // ASCII-имя + неlexable qualified (пробел в объекте) — `$форма`
                InboundHint {
                    name: "peak".to_owned(),
                    qualified: "Профиль (n1).peak".to_owned(),
                    source: "Профиль".to_owned(),
                    spill: true,
                },
                // кириллическое имя + неlexable qualified — формы нет
                InboundHint {
                    name: "купон".to_owned(),
                    qualified: "Финальная корзина.купон".to_owned(),
                    source: "Корзина".to_owned(),
                    spill: true,
                },
                // имя с префикса `in` — `$`-форма зарезервирована валютой
                InboundHint {
                    name: "inn".to_owned(),
                    qualified: "Налоги.inn".to_owned(),
                    source: "Налоги".to_owned(),
                    spill: true,
                },
                // позиционное ребро: qualified-форма ДАЖЕ для ASCII-имени
                // (`$имя` в рантайме не резолвится — имя не параметр)
                InboundHint {
                    name: "users".to_owned(),
                    qualified: "Спрос.users".to_owned(),
                    source: "Спрос".to_owned(),
                    spill: false,
                },
            ],
        };
        let guarded_items = hint_items("w = $", &guarded, Language::Ru);
        let inserts: Vec<&str> = guarded_items
            .iter()
            .map(|item| item.insert.as_str())
            .collect();
        assert!(
            inserts.contains(&"$peak"),
            "ASCII-имя — `$форма` без qualified"
        );
        assert!(
            !guarded_items.iter().any(|item| item.label == "купон"),
            "не вставляемое имя не подсказывается"
        );
        assert!(
            inserts.contains(&"Налоги.inn"),
            "`$inn` запрещён валютой — qualified-форма"
        );
        assert!(
            inserts.contains(&"Спрос.users"),
            "позиционное ребро — только qualified"
        );
        assert!(!inserts.contains(&"$users"));
    }

    /// FR-102: контекст точки — (объект, префикс поля); стражи контекста:
    /// число перед точкой, `$`-объект, пробел перед точкой, пустой объект.
    #[test]
    fn dot_field_context_parses() {
        assert_eq!(
            dot_field_context("итог = Корзина."),
            Some(("Корзина".to_owned(), String::new()))
        );
        assert_eq!(
            dot_field_context("итог = Корзина.су"),
            Some(("Корзина".to_owned(), "су".to_owned()))
        );
        // кириллица/цифры/подчёркивание в объекте; qualified-поле с дефисом
        // матчится как токен после точки (класс символов token_before_caret:
        // дефис обрывает — хвост подсказок набирается без дефисов)
        assert_eq!(
            dot_field_context("Заявки.Кол"),
            Some(("Заявки".to_owned(), "Кол".to_owned()))
        );
        assert_eq!(
            dot_field_context("Корзина"),
            None,
            "без точки — не контекст"
        );
        assert_eq!(
            dot_field_context("Корзина ."),
            None,
            "пробел перед точкой — лексер qualified не соберёт"
        );
        assert_eq!(dot_field_context("2."), None, "число — не объект");
        assert_eq!(dot_field_context("$Корз."), None, "$-токен — не объект");
        assert_eq!(dot_field_context("."), None, "пустой объект");
        assert_eq!(dot_field_context("итог = "), None);
    }

    /// FR-102: после точки подсказываются поля объекта — qualified-ключи
    /// входящих value-рёбер. Вставка — только ИМЯ ПОЛЯ (дописывает путь:
    /// «Корзина.s» + принятие → «Корзина.sum», полный qualified дал бы
    /// «Объект.Нода.поле»); деталь — нода-источник; регистр не важен.
    #[test]
    fn hints_fields_after_dot() {
        let mut multi = ctx();
        multi.inbounds.push(InboundHint {
            name: "sum".to_owned(),
            qualified: "Корзина.sum".to_owned(),
            source: "Корзина за вычетом скидок".to_owned(),
            spill: false,
        });
        multi.inbounds.push(InboundHint {
            name: "скидка".to_owned(),
            qualified: "Корзина.скидка".to_owned(),
            source: "Корзина за вычетом скидок".to_owned(),
            spill: false,
        });
        // Точка без хвоста — все поля объекта (репродуктор: «итог = Корзина.»)
        let items = hint_items("итог = Корзина.", &multi, Language::Ru);
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, vec!["sum", "скидка"]);
        assert!(items.iter().all(|item| item.kind == HintKind::DollarRef));
        assert!(
            items[0].detail.contains("Корзина"),
            "деталь — источник: {}",
            items[0].detail
        );
        // Дописываемый хвост фильтрует поля; вставка — только поле
        let items = hint_items("итог = Корзина.с", &multi, Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "скидка");
        // Регистр объекта и поля не важен
        let items = hint_items("итог = корзина.S", &multi, Language::Ru);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].insert, "sum");
        // Чужой объект/несуществующее поле — пусто (попап закрыт)
        assert!(hint_items("итог = Корзина.x", &multi, Language::Ru).is_empty());
        // Объект без qualified-ключа (пустая qualified) — не подсказывается
        multi.inbounds.push(InboundHint {
            name: "аноним".to_owned(),
            qualified: String::new(),
            source: "Аноним".to_owned(),
            spill: true,
        });
        assert!(hint_items("итог = Аноним.", &multi, Language::Ru).is_empty());
    }

    /// FR-102: стражи ветки в `hint_items` — «2.» остаётся единицами;
    /// «$Профиль.»/«Купон .»/«w = .» — НЕ контекст полей (разбор падает
    /// в прежнюю семантику пустого токена — только переменные, без
    /// DollarRef-полей); поле после точки НЕ подмешивает именованные
    /// входы (ветка завершает разбор — иначе матч по имени поля «с»
    /// предлагал бы «Скидка.скидка» внутрь набираемого пути).
    #[test]
    fn hints_fields_after_dot_guards() {
        // «2.» — число перед точкой, НЕ объект: ветка полей не срабатывает
        // (прежняя семантика пустого токена — без DollarRef-полей)
        let items = hint_items("w = 2.", &ctx(), Language::Ru);
        assert!(
            !items.iter().any(|item| item.kind == HintKind::DollarRef),
            "«2.» — не контекст полей"
        );
        // Не контекст полей → прежняя семантика пустого токена (переменные)
        for prefix in ["w = $Профиль.", "w = Купон .", "w = ."] {
            let items = hint_items(prefix, &ctx(), Language::Ru);
            assert!(
                !items.iter().any(|item| item.kind == HintKind::DollarRef),
                "без полей на {prefix:?}"
            );
            assert!(items.iter().all(|item| item.kind == HintKind::Var));
        }
        // «Купон.с» — префикс «с» не матчит поле «купон» — тишина
        // (именованный вход «Скидка.скидка» по имени НЕ подмешивается)
        assert!(hint_items("w = Купон.с", &ctx(), Language::Ru).is_empty());
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

    /// CR-023 (UR-001-08): разовое автодополнение — принятое не всплывает
    /// повторно: подавление взводится принятием (`arm_suppress`), первый
    /// sync после вставки фиксирует фактический токен, движение каретки
    /// без правки — тишина, смена токена снимает подавление.
    #[test]
    fn hints_popup_accept_suppresses_until_token_change() {
        let mut popup = HintPopup::default();
        // Репродуктор владельца: `100 rub` → принять единицу → попап
        // возвращался с той же подсказкой
        popup.sync("50".to_owned(), hint_items("w = 50 ", &ctx(), Language::Ru));
        assert!(popup.open);
        // Принятие: arm_suppress + первый sync после вставки «сек»
        popup.arm_suppress();
        let units = hint_items("w = 50 сек", &ctx(), Language::Ru);
        assert!(
            !units.is_empty(),
            "префикс принятой единицы снова даёт строки"
        );
        popup.sync("сек".to_owned(), units);
        assert!(!popup.open, "после принятия попап закрыт");
        assert!(
            !popup.items.is_empty(),
            "список сохраняется под подавлением"
        );
        // Движение каретки без правки — sync на том же токене — тишина
        popup.sync(
            "сек".to_owned(),
            hint_items("w = 50 сек", &ctx(), Language::Ru),
        );
        assert!(!popup.open, "подавление держится на принятом токене");
        // Смена токена (допечатать/стереть символ) — попап оценивает заново
        popup.sync(
            "се".to_owned(),
            hint_items("w = 50 се", &ctx(), Language::Ru),
        );
        assert!(popup.open, "смена токена снимает подавление");
    }

    /// CR-023: Esc — dismiss до смены токена (единая семантика с
    /// подавлением после принятия): правка токена возвращает попап,
    /// движение каретки — нет. Прежний `reset` возвращал попап на том же
    /// токене при первой же правке.
    #[test]
    fn hints_popup_esc_dismiss_holds_until_token_change() {
        let mut popup = HintPopup::default();
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        assert!(popup.open);
        popup.dismiss();
        assert!(!popup.open);
        // Тот же токен — попап не возвращается
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        assert!(!popup.open);
        // Смена токена — оценивается заново (префикс `m` — mm1/mmc)
        popup.sync("m".to_owned(), hint_items("w = m", &ctx(), Language::Ru));
        assert!(popup.open);
    }

    /// CR-023: `reset` (новая тема редактирования) снимает и зафиксированное,
    /// и взведённое подавление.
    #[test]
    fn hints_popup_reset_clears_suppression() {
        let mut popup = HintPopup::default();
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        popup.dismiss();
        assert!(!popup.open);
        popup.reset();
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        assert!(popup.open, "reset снимает зафиксированное подавление");
        popup.arm_suppress();
        popup.reset();
        popup.sync("mm".to_owned(), hint_items("w = mm", &ctx(), Language::Ru));
        assert!(popup.open, "reset снимает взведённое подавление");
    }
}
