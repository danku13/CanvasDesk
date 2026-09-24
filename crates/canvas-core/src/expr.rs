//! FR-013: Numi-base язык формул для text-нод (`canvasdesk.expr`).
//!
//! Чистый Rust, без I/O и глобального состояния (инварианты 1/2 FR-013):
//! [`parse`] — `&str -> Result<Expr, ParseError>`, [`eval`] —
//! `(&Expr, &Env) -> Result<Value, EvalError>`. Результат — число с
//! единицей измерения (композитные единицы вида `ms·req/s` поддержаны).
//!
//! Грамматика v1 (Numi-base):
//! - литералы: числа (`5`, `3.14`, `1k`, `2M`) и единицы (`ms`, `sec`,
//!   `min`, `h`, `req`, `req/s`, `rps`, `B`, `KB`, `MB`, `GB`, `$`, `%`;
//!   кириллические синонимы: `мс`, `сек`, `мин`, `ч`, `запр`, `запр/с`,
//!   `Б`, `КБ`, `МБ`, `ГБ`) — по одному токену на значение, без
//!   словоизменительных дублей (`s`/`secs`/`reqs`/`hour` убраны);
//! - операторы: `+ - * × · ⋅ ✕ ⨯ ÷ /`, скобки, унарный минус; неявное
//!   умножение (`5 ms` = `5 × ms`, `$5` = `5 × $`); `x`/`х` между
//!   операндами — тоже умножение (Numi: `35 x 20` = 700);
//! - переменные: `name = expr` и хвостовое присваивание `expr = name`
//!   (утверждения; результат программы — значение последнего утверждения);
//!   имена — буквы Unicode (латиница и кириллица: `х = 200`), цифры, `_`;
//! - функции v1: `sum`, `avg`, `max`, `min`, `percentile(p, …)`;
//! - сложение/вычитание требует одной размерности (правый операнд
//!   конвертируется к единице левого: `1 sec + 500 ms == 1.5 sec`);
//! - умножение/деление — символические (единицы сливаются/сокращаются,
//!   `Count/Time` нормализуется в `Rate`: `100 req / 2 sec == 50 req/s`).
//!
//! Единицы распознаются ТОЛЬКО сразу после числа (`1000 rps` — единица),
//! поэтому переменные могут называться как единицы (`rps = 1000` — пример
//! владельца). Поток значений между нодами — FR-014: входящие значения
//! value-рёбер доступны как `$in` (ровно одно входящее ребро) или `$1..$N`
//! (по индексу входящих рёбер в порядке `canvas.edges`); окружение с
//! входами — [`Env::with_inbound`], рантайм — `flow::propagate`. Доменные
//! функции (`mm1`, `littles_law`) — FR-015.
//!
//! Валюта и ссылки на вход: `$5` — валюта (5 долларов), пока окружение НЕ
//! содержит входов; в окружении со входами целое `$N` (N ≥ 1) — ссылка на
//! N-е входящее значение (для валюты в calc-ноде потока пишите `5 usd`).
//! Дробные суммы (`$2.5`) и `$0` — всегда валюта.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

mod queueing;

// FR-063 (Открытый вопрос № 1): общий разбор аргументов доменных слоёв L2
// (queueing/stats) — вынесен из queueing.rs без изменения его поведения.
mod args;
// FR-063: доменный слой статистики (L2, ADR-0008 M2/S1) — только с фичей
// `stats`; без неё имена не регистрируются и дают UnknownFunction (fallback).
#[cfg(feature = "stats")]
mod stats;
// FR-066: Monte Carlo + QMC-движок (L4/P3, ADR-0008 M5/S3) — только с
// фичей `qmc` (implies stats+parallel+sobol_burley, §5.6); wasm-сборка
// идёт без фичи (§5.8). Публичный модуль: McConfig/McResult/Distribution,
// сэмплы, квантили, синтетический FlowSolutions, engine-метаданные.
#[cfg(feature = "qmc")]
pub mod mc;

/// Результат вычисления формулы ноды — runtime-состояние приложения
/// (инвариант 4 FR-013: НЕ сериализуется в `.canvas`, пересчитывается
/// из формулы при загрузке/правке/undo).
#[derive(Debug, Clone, PartialEq)]
pub enum ExprOutcome {
    /// Формула вычислена — результат для строки под текстом ноды.
    Ok(Value),
    /// Диагностика (парсинг или вычисление) — красная строка на карточке.
    Err(String),
}

/// Результаты формул по id нод (`SceneState.expr_results`).
pub type ExprResults = HashMap<String, ExprOutcome>;

/// FR-013 (правка 2, Numi-стиль): построчные результаты текста ноды —
/// runtime-кэш приложения (инвариант 4: в `.canvas` не сериализуются).
/// Vec выровнен по строкам текста (`split('\n')`): `None` — строка без
/// результата (проза, пустая, внутри код-фенса).
pub type ExprLineResults = HashMap<String, Vec<Option<ExprOutcome>>>;

// --- Размерности и единицы ---

/// Размерность единицы (FR-013; доменные расширения — FR-015).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dimension {
    Time,
    Rate,
    Count,
    Bytes,
    Money,
    Percent,
    Custom(String),
}

/// Атом единицы: размерность, степень, множитель к базовой единице
/// размерности и имя для отображения (как в формуле).
#[derive(Debug, Clone)]
pub struct Atom {
    pub dim: Dimension,
    pub exp: i8,
    /// Множитель к базовой единице размерности (`ms` → 0.001 к `sec`).
    pub scale: f64,
    pub name: &'static str,
}

impl Atom {
    /// FR-016 (analyze): атом из таблицы для юнит-тестов смежных модулей.
    pub(crate) const fn new(dim: Dimension, exp: i8, scale: f64, name: &'static str) -> Self {
        Self {
            dim,
            exp,
            scale,
            name,
        }
    }

    fn key(&self) -> Dimension {
        self.dim.clone()
    }
}

/// Единица измерения: упорядоченный список атомов (порядок — для
/// отображения: `ms·req/s`, а не `ms·req·s⁻¹` — решение из FR-013).
/// Пустой список — скаляр.
#[derive(Debug, Clone, Default)]
pub struct Unit {
    pub atoms: Vec<Atom>,
}

impl Unit {
    /// Скаляр (безразмерный) — `Unit::Scalar` из верификации FR-013;
    /// имя сохранено как в документе (не SCALAR).
    #[allow(non_upper_case_globals)]
    pub const Scalar: Unit = Unit { atoms: Vec::new() };

    /// Единица из одного атома.
    pub fn atom(atom: Atom) -> Self {
        Self { atoms: vec![atom] }
    }

    /// Скалярная единица?
    pub fn is_scalar(&self) -> bool {
        self.atoms.is_empty()
    }

    /// Мультимножество (размерность → суммарная степень) — совместимость
    /// операндов сложения/вычитания и функций. FR-016 (analyze): чтение
    /// размерности значения из смежных модулей ядра.
    pub(crate) fn dims(&self) -> BTreeMap<Dimension, i16> {
        let mut map = BTreeMap::new();
        for atom in &self.atoms {
            *map.entry(atom.key()).or_insert(0) += atom.exp as i16;
        }
        map.retain(|_, exp| *exp != 0);
        map
    }

    /// Произведение масштабов атомов: значение единицы в базовых единицах
    /// её размерностей (`ms` → 0.001, `min` → 60, `MB` → 1024²).
    /// FR-016 (analyze): перевод значения в базу из смежных модулей ядра.
    pub(crate) fn scale(&self) -> f64 {
        self.atoms
            .iter()
            .map(|atom| atom.scale.powi(atom.exp as i32))
            .product()
    }

    /// Слияние для умножения (`sign = 1`) / деления (`sign = -1`) со
    /// сокращением нулевых степеней и нормализацией `Count¹·Time⁻¹ → Rate¹`
    /// (масштабы согласованы: база Count = 1 req, база Time = 1 sec,
    /// база Rate = 1 req/s).
    fn merged(mut self, other: &Unit, sign: i8) -> Unit {
        let mut atoms: Vec<Atom> = std::mem::take(&mut self.atoms);
        for rhs in &other.atoms {
            if let Some(lhs) = atoms.iter_mut().find(|a| a.key() == rhs.key()) {
                lhs.exp += sign * rhs.exp;
            } else {
                atoms.push(Atom {
                    dim: rhs.dim.clone(),
                    exp: sign * rhs.exp,
                    scale: rhs.scale,
                    name: rhs.name,
                });
            }
        }
        atoms.retain(|atom| atom.exp != 0);
        let count_pos = atoms
            .iter()
            .position(|a| a.dim == Dimension::Count && a.exp == 1);
        let time_neg = atoms
            .iter()
            .position(|a| a.dim == Dimension::Time && a.exp == -1);
        if let (Some(_), Some(t)) = (count_pos, time_neg) {
            atoms.remove(t);
            if let Some(c) = atoms
                .iter_mut()
                .find(|a| a.dim == Dimension::Count && a.exp == 1)
            {
                *c = Atom::new(Dimension::Rate, 1, 1.0, rate_name_for(c.name));
            }
        }
        Unit { atoms }
    }

    /// Отображение единицы: положительные степени через `·` (с˅2/˅3),
    /// отрицательные — через `/` (`ms·req/s`, `B/s`, `ms²`). Публичный:
    /// нужен MCP-ответам (`flow_recalc`) и рендеру лейблов value-рёбер.
    pub fn display(&self) -> String {
        const MIDDLE_DOT: char = '\u{b7}';
        const SUP2: char = '\u{b2}';
        const SUP3: char = '\u{b3}';
        let mut positive: Vec<String> = Vec::new();
        let mut negative: Vec<String> = Vec::new();
        for atom in &self.atoms {
            let name = atom.name;
            match atom.exp {
                1 => positive.push(name.to_owned()),
                2 => positive.push(format!("{name}{SUP2}")),
                3 => positive.push(format!("{name}{SUP3}")),
                e if e > 0 => positive.push(format!("{name}^{e}")),
                -1 => negative.push(name.to_owned()),
                -2 => negative.push(format!("{name}{SUP2}")),
                -3 => negative.push(format!("{name}{SUP3}")),
                e => negative.push(format!("{name}^{}", -e)),
            }
        }
        let mut out = positive.join(&MIDDLE_DOT.to_string());
        if !negative.is_empty() {
            if out.is_empty() {
                out = negative.join("/");
            } else {
                out.push('/');
                out.push_str(&negative.join("/"));
            }
        }
        out
    }
}

/// Равенство единиц — по мультимножеству (размерность, степень, масштаб):
/// синоним имени (`sec`/`сек`) равенству не мешает.
impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        let flat = |unit: &Self| -> BTreeMap<(Dimension, i8), (i8, f64)> {
            let mut map = BTreeMap::new();
            for atom in &unit.atoms {
                map.insert((atom.dim.clone(), atom.exp), (atom.exp, atom.scale));
            }
            map
        };
        flat(self) == flat(other)
    }
}

/// Таблица единиц v1 (FR-013, §Changes п.6): `(токен, размерность, масштаб
/// к базе)`. Базы: Time = sec, Bytes = B, Rate = req/s, Count = req,
/// Money = $, Percent = %. Килобайты — двоичные (1 GB = 1024 MB).
///
/// Канонизация (правка владельца, 2026-09-23): один токен на значение —
/// наиболее наглядная форма без словоизменительных дублей: `sec` (не
/// `s`/`secs`), `req` (не `reqs`), `h` (не `hour`); основа уже читается
/// как множественность (`300 req` — триста запросов). Отображение — токен,
/// которым единица введена (`50 мс` → `50 мс`, `50 ms` → `50 ms`).
///
/// Кириллические синонимы (были отложены до v2 — внесены 2026-09-23):
/// `мс`, `сек`, `мин`, `ч`, `запр`, `запр/с`, `Б`, `КБ`, `МБ`, `ГБ`.
/// Деньги и проценты нейтральны (`$`, `usd`, `%`); `руб` сознательно НЕ
/// синоним `$` — другая валюта, алиасинг смешал бы размерности
/// (`100 руб + 5 $ = 105` — бессмыслица).
const UNIT_TABLE: &[(&str, Dimension, f64)] = &[
    // Латиница — канонические формы
    ("ms", Dimension::Time, 0.001),
    ("sec", Dimension::Time, 1.0),
    ("min", Dimension::Time, 60.0),
    ("h", Dimension::Time, 3600.0),
    ("req", Dimension::Count, 1.0),
    ("rps", Dimension::Rate, 1.0),
    ("req/s", Dimension::Rate, 1.0),
    ("B", Dimension::Bytes, 1.0),
    ("KB", Dimension::Bytes, 1024.0),
    ("MB", Dimension::Bytes, 1024.0 * 1024.0),
    ("GB", Dimension::Bytes, 1024.0 * 1024.0 * 1024.0),
    ("$", Dimension::Money, 1.0),
    ("usd", Dimension::Money, 1.0),
    ("%", Dimension::Percent, 1.0),
    // Кириллица — синонимы (v2); отображение — как введено
    ("мс", Dimension::Time, 0.001),
    ("сек", Dimension::Time, 1.0),
    ("мин", Dimension::Time, 60.0),
    ("ч", Dimension::Time, 3600.0),
    ("запр", Dimension::Count, 1.0),
    ("запр/с", Dimension::Rate, 1.0),
    ("Б", Dimension::Bytes, 1.0),
    ("КБ", Dimension::Bytes, 1024.0),
    ("МБ", Dimension::Bytes, 1024.0 * 1024.0),
    ("ГБ", Dimension::Bytes, 1024.0 * 1024.0 * 1024.0),
];

/// Атом по токену таблицы.
fn unit_atom(token: &str) -> Option<Atom> {
    UNIT_TABLE
        .iter()
        .find(|(name, _, _)| *name == token)
        .map(|(name, dim, scale)| Atom::new(dim.clone(), 1, *scale, name))
}

/// Токен Rate для синтеза `Count/Time` по имени Count-атома: `req` →
/// `req/s`, `запр` → `запр/с` (правка 2026-09-23: деление кириллических
/// единиц показывает `50 запр/с`, а не смешанный `50 req/s`). Поиск — по
/// префиксу таблицы (`{count}/…`), fallback — `req/s`.
fn rate_name_for(count_name: &str) -> &'static str {
    let prefix = format!("{count_name}/");
    UNIT_TABLE
        .iter()
        .find(|(token, dim, _)| *dim == Dimension::Rate && token.starts_with(&prefix))
        .map(|(token, _, _)| *token)
        .unwrap_or("req/s")
}

/// FR-018: значение с единицей из токена таблицы (`Some("rps")`, `Some("ms")`);
/// `None` или неизвестный токен — скаляр. Публичный мост для реестра
/// шаблонов (`templates.rs`) и MCP-инстанциации.
pub fn unit_value(num: f64, unit: Option<&str>) -> Value {
    let unit = unit
        .and_then(unit_atom)
        .map(Unit::atom)
        .unwrap_or(Unit::Scalar);
    Value { num, unit }
}

/// FR-018: ASCII-идентификатор параметра сразу за `$` (`$rps`):
/// `[a-zA-Z_][a-zA-Z0-9_]*`; имена с префиксом `in` зарезервированы для
/// валютной семантики FR-013 (`$inn` — валюта × переменная). Кириллица —
/// НЕ параметр (`$ин` — валюта, как в FR-013).
fn dollar_param_ident(rest: &str) -> Option<String> {
    let bytes = rest.as_bytes();
    let first = *bytes.first()?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    let len = 1 + bytes[1..]
        .iter()
        .take_while(|&&b| b.is_ascii_alphanumeric() || b == b'_')
        .count();
    let name = &rest[..len];
    if name.starts_with("in") {
        return None;
    }
    Some(name.to_owned())
}

// --- Значение ---

/// Значение с единицей: результат вычисления (`Value { num, unit }`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Value {
    pub num: f64,
    pub unit: Unit,
}

impl Value {
    pub fn scalar(num: f64) -> Self {
        Self {
            num,
            unit: Unit::Scalar,
        }
    }

    pub fn with_unit(num: f64, unit: Unit) -> Self {
        Self { num, unit }
    }

    /// Мультимножество размерностей значения.
    pub(crate) fn dims(&self) -> BTreeMap<Dimension, i16> {
        self.unit.dims()
    }
}

/// Формат числа: целые без дробной части, дробные — до 6 значащих цифр
/// без хвостовых нулей (`1.5`, `166.667`, `1000`).
fn format_num(num: f64) -> String {
    if num.is_nan() {
        return "не число".to_owned();
    }
    if num.is_infinite() {
        return if num > 0.0 {
            "∞".to_owned()
        } else {
            "-∞".to_owned()
        };
    }
    if num == num.trunc() && num.abs() < 1e15 {
        return format!("{num:.0}");
    }
    // 6 значащих цифр: 0.30000000000000004 → 0.3, 166.6666… → 166.667
    let magnitude = num.abs().log10().floor();
    let decimals = ((5.0 - magnitude).clamp(-3.0, 6.0)) as i32;
    let factor = 10f64.powi(decimals);
    let rounded = (num * factor).round() / factor;
    let trimmed = format!("{rounded:.0$}", decimals.max(0) as usize);
    if trimmed.contains('.') {
        trimmed
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    } else {
        trimmed
    }
}

impl fmt::Display for Value {
    /// `{num} {unit}`: `1000 ms·req/s`, `1.5 sec`, `20 ms`; скаляр — число.
    /// D-1 (FR-061): композиция над [`Value::display_parts`] — единая точка
    /// сборки отображения значения (инвариант байт-паритета, тест-свойство).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&join_parts_of(&self.display_parts()))
    }
}

impl Value {
    /// D-1 (FR-061, табличное тело ноды): структурные части значения —
    /// `(число, юнит)` раздельно. Число — то же форматирование, что в
    /// [`Display`] (группировка/округление `format_num`); юнит —
    /// [`Unit::display`], скаляр → пустая строка. Потребители — табличные
    /// ячейки значения/юнита (этап B row_grid), MCP-тексты остаются на
    /// `Display` (обратная совместимость).
    pub fn display_parts(&self) -> (String, String) {
        (format_num(self.num), self.unit.display())
    }
}

/// D-1 (FR-061): единственная сборка отображения значения из частей —
/// `«num unit»`, для скаляра (пустой юнит) — `«num»`. Используется
/// `Display for Value`, `whatif_full_delta` и `AutoRow::display_text`:
/// одна точка сборки гарантирует байт-паритет всех трёх форматов
/// (тест-свойство `display_parity_property`).
pub fn join_parts(num: &str, unit: &str) -> String {
    if unit.is_empty() {
        num.to_owned()
    } else {
        format!("{num} {unit}")
    }
}

/// Внутренний хелпер — сборка из кортежа частей (см. [`join_parts`]).
fn join_parts_of(parts: &(String, String)) -> String {
    join_parts(&parts.0, &parts.1)
}

// --- Выражение ---

/// Двоичный оператор.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// Выражение/программа формулы (дерево парсера).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Число с единицей (`5`, `1000 rps`).
    Num(f64, Unit),
    /// Переменная окружения.
    Var(String),
    /// Унарный минус.
    Neg(Box<Expr>),
    /// Двоичная операция.
    Bin {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// Вызов функции v1 (`sum`, `avg`, `max`, `min`, `percentile`).
    Call { func: String, args: Vec<Expr> },
    /// Утверждение `name = expr`; значение — значение `rhs`.
    Assign { name: String, rhs: Box<Expr> },
    /// Программа: последовательность утверждений; результат — последний.
    Block(Vec<Expr>),
    /// FR-014: `$in` — значение единственного входящего value-ребра.
    /// При нескольких входах — `EvalError::AmbiguousInbound` (нужны `$1..$N`),
    /// при нуле — `EvalError::MissingInbound`.
    Inbound,
    /// FR-014: `$5` — валюта ИЛИ ссылка на 5-й вход; контекст — наличие
    /// входов в [`Env`], разрешается в [`eval`]. Целые `N ≥ 1` при наличии
    /// входов — ссылка на N-е входящее значение; иначе — валюта.
    DollarAmount(f64),
    /// FR-018: `$rps` — параметр шаблонной ноды; источник значений —
    /// `Env.params` (заполняется из `canvasdesk.template.params`).
    Param(String),
    /// FR-050 Р-6: qualified-путь «Объект.Поле» (`Заявки.Кол-во`) —
    /// именованная ссылка на входящее значение; резолв — по графу
    /// входящих value-рёбер (`Env.qualified`, flow.rs). Позиционные
    /// `$1..$N` — легаси (вариант Б: именованный синтаксис — сразу).
    Qualified { obj: String, field: String },
}

/// Окружение вычисления: значения переменных (FR-013) и входящие значения
/// value-рёбер (FR-014, [`Env::with_inbound`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Env {
    vars: HashMap<String, Value>,
    /// FR-018: параметры шаблонной ноды (`$имя`); заполняются из
    /// `canvasdesk.template.params` (flow) или конструкторами.
    params: HashMap<String, Value>,
    /// FR-014: входящие значения по индексам value-рёбер. `None` — входов
    /// нет (`$N` — валюта, `$in` — ошибка MissingInbound); `Some` — есть:
    /// `Some(None)` — ребро есть, значения нет (источник без формулы или
    /// с ошибкой), `Some(Some(v)) — значение источника.
    inbound: Option<Vec<Option<Value>>>,
    /// FR-050 Р-6: таблица резолва именованных путей «Объект.Поле» →
    /// значение — заполняется flow по графу входящих value-рёбер приёмника
    /// (порядок рёбер не влияет на ключи). Читается
    /// [`Expr::Qualified`].
    qualified: HashMap<(String, String), Value>,
}

impl Env {
    pub fn empty() -> Self {
        Self::default()
    }

    /// FR-014: окружение со входами value-рёбер (порядок — порядок
    /// входящих рёбер в `canvas.edges`). `None` — ребро есть, значения нет.
    pub fn with_inbound(inbound: Vec<Option<Value>>) -> Self {
        Self {
            vars: HashMap::new(),
            params: HashMap::new(),
            inbound: Some(inbound),
            qualified: HashMap::new(),
        }
    }

    /// FR-018: окружение с параметрами шаблона (`$имя`).
    pub fn with_params(params: BTreeMap<String, Value>) -> Self {
        Self {
            vars: HashMap::new(),
            params: params.into_iter().collect(),
            inbound: None,
            qualified: HashMap::new(),
        }
    }

    /// FR-050 Р-6: добавить таблицу резолва именованных путей «Объект.Поле»
    /// (flow: по графу входящих value-рёбер приёмника).
    pub fn with_qualified(mut self, qualified: HashMap<(String, String), Value>) -> Self {
        self.qualified = qualified;
        self
    }

    /// FR-050 Р-6: значение именованного пути «Объект.Поле».
    pub fn qualified_value(&self, obj: &str, field: &str) -> Option<&Value> {
        self.qualified.get(&(obj.to_owned(), field.to_owned()))
    }

    /// FR-018: добавить карту параметров к окружению (flow: входы value-
    /// рёбер + параметры шаблона в одном Env).
    pub fn with_param_map(mut self, params: BTreeMap<String, Value>) -> Self {
        self.params.extend(params);
        self
    }

    /// FR-018: добавить/заменить параметр (builder — как [`Env::set`]).
    pub fn set_param(mut self, name: impl Into<String>, value: Value) -> Self {
        self.params.insert(name.into(), value);
        self
    }

    /// FR-018: значение параметра `$name`.
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params.get(name)
    }

    /// FR-029: итератор переменных листа (именованные выходы текстовой
    /// ноды для адресации рёбер `fromOutput`). Порядок недетерминирован
    /// (HashMap) — потребитель собирает в свою карту.
    pub fn vars_iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.vars.iter()
    }

    pub fn set(mut self, name: impl Into<String>, value: Value) -> Self {
        self.vars.insert(name.into(), value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.vars.get(name)
    }

    /// FR-014: входящее значение по индексу (0-based). `None` — индекса нет
    /// или значения нет (ребро без значения).
    fn inbound_value(&self, index: usize) -> Option<&Value> {
        self.inbound.as_ref()?.get(index)?.as_ref()
    }

    /// Есть ли входы вообще (дискриминатор валюты `$N` vs входа `$N`).
    fn has_inbound(&self) -> bool {
        self.inbound.is_some()
    }

    /// Слоты входов (для проверки неоднозначности `$in`).
    fn inbound_slots(&self) -> Option<&[Option<Value>]> {
        self.inbound.as_deref()
    }
}

/// Ошибка разбора формулы: позиция (байтовый offset) и сообщение.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ошибка формулы (позиция {pos}): {msg}")]
pub struct ParseError {
    pub pos: usize,
    pub msg: String,
}

/// Ошибка вычисления формулы.
// Eq снят (FR-015): Overload { rho: f64 } — f64 не реализует Eq;
// PartialEq (assert_eq! в тестах) сохранён.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EvalError {
    #[error("единицы не совместимы: {lhs} и {rhs}")]
    UnitMismatch { lhs: String, rhs: String },
    #[error("неизвестная переменная: {0}")]
    UnknownVariable(String),
    /// FR-018: ссылка `$имя` не имеет значения в параметрах шаблона.
    #[error("неизвестный параметр: ${0}")]
    UnknownParam(String),
    /// FR-050 Р-6: именованный путь «Объект.Поле» не разрешён — среди
    /// входящих value-рёбер приёмника нет пары с таким адресом (источник
    /// не подключён, имя не совпадает или источник без значения —
    /// unmapped-диагностика Р-3 отдельным контуром).
    #[error("вход не найден: {0}")]
    UnknownInput(String),
    #[error("неизвестная функция: {0}")]
    UnknownFunction(String),
    #[error("{func}: {msg}")]
    BadCall { func: String, msg: String },
    #[error("деление на ноль")]
    DivisionByZero,
    /// FR-014: входящего значения нет (нет value-ребра, источник без
    /// формулы или источник с ошибкой). `index` — 0-based номер входа
    /// (`$in` — 0, синоним `$1`).
    #[error("вход отсутствует: ${}" , index + 1)]
    MissingInbound { index: usize },
    /// FR-014: `$in` при нескольких входящих value-рёбрах — неоднозначно.
    #[error("$in неоднозначен: {count} входящих — используйте $1..${}", count)]
    AmbiguousInbound { count: usize },
    /// FR-014 (flow::propagate): формула ноды не парсится (в обычном
    /// рендере ошибки парсинга ловятся до eval; в графе — часть downstream).
    #[error("{0}")]
    BadFormula(String),
    /// FR-015: перегрузка системы массового обслуживания — коэффициент
    /// использования ρ ≥ 1, очередь аналитически не ограничена. Красная
    /// строка диагностики на карточке (что и требовалось: ρ > 1 — узкое
    /// место ландшафта, а не тихий неверный расчёт).
    #[error("перегрузка: ρ = {} ≥ 1 — очередь растёт неограниченно", format_num(*rho))]
    Overload { rho: f64 },
}

// --- Лексер ---

/// Токен лексера.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    /// Единица из таблицы (распознаётся только после числа или `$`-валюта).
    Unit(&'static str),
    /// FR-014: `$in` — ссылка на единственный вход.
    DollarIn,
    /// FR-014: `$N` (целые N ≥ 1 сразу за `$`) — валюта или вход (контекст
    /// разрешает eval по наличию входов в Env).
    DollarNum(f64),
    /// FR-018: `$rps` — ссылка на параметр шаблона (ASCII-имя; имена с
    /// префиксом `in` зарезервированы — совместимость с валютной семантикой
    /// `$inn` из FR-013). Разрешается в `Env.params` (шаблонные ноды).
    DollarIdent(String),
    /// FR-050 Р-6: qualified-путь «Объект.Поле» (`Заявки.Кол-во`,
    /// `Заявки (2).Кол-во` — суффикс коллизии имён). Резолв — по графу
    /// входящих value-рёбер (`Env.qualified`, flow.rs).
    Qualified {
        obj: String,
        field: String,
    },
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    Comma,
    Assign,
    /// Конец утверждения: перевод строки или `;`.
    Sep,
}

/// Лексер с окном на один токен (peek — для неявного умножения и отката).
struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    /// Последним значимым токеном было число — контекст распознавания единиц.
    after_number: bool,
    /// Последним токеном был ОПЕРАНД (число/единица/`)`) — контекст
    /// `x`-умножения (`35 x 20`); после операторов/в старте строки — false.
    operand_ended: bool,
    peeked: Option<Option<Tok>>,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            pos: 0,
            after_number: false,
            operand_ended: false,
            peeked: None,
        }
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        ParseError {
            pos: self.pos,
            msg: msg.into(),
        }
    }

    fn rest_starts(&self, pattern: &str) -> bool {
        self.text[self.pos..].starts_with(pattern)
    }

    /// Самая длинная единица таблицы в текущей позиции (max-munch:
    /// `req/s` раньше `req`).
    fn unit_here(&self) -> Option<(&'static str, usize)> {
        let rest = &self.text[self.pos..];
        UNIT_TABLE
            .iter()
            .filter(|(name, _, _)| rest.starts_with(name))
            .max_by_key(|(name, _, _)| name.len())
            .map(|(name, _, _)| (*name, name.len()))
    }

    /// `x`-умножение (Numi): после считанного `x`/`х` следующий значимый
    /// байт начинает операнд (цифра, `.`, `(`, `$`)? Пробелы пропускаются.
    /// `35 x 20` — да (умножение), `200 + x␣` в конце — нет (переменная).
    fn operand_starts_ahead(&self) -> bool {
        let rest = self.text[self.pos..].trim_start_matches([' ', '\t']);
        matches!(
            rest.as_bytes().first(),
            Some(b'0'..=b'9' | b'.' | b'(' | b'$')
        )
    }

    /// Идентификатор: буквы Unicode (латиница/кириллица — `х = 200`),
    /// цифры, `_`. Единица распознаётся ТОЛЬКО сразу после числа:
    /// `1000 rps` — единица, `rps = 1000` — переменная. `x`/`х` ПОСЛЕ
    /// операнда (числа, единицы, `)`, идентификатора) и ПЕРЕД числом/скобкой
    /// — умножение (`35 x 20` = 700, `latency х 3`; Numi); в конце строки,
    /// после оператора или перед идентификатором — переменная (`200 + x`,
    /// `latency х replicas`). СЛИТНОЕ `35x20` (без пробелов) — тоже
    /// умножение (правка 5): `x`/`х` после операнда с цифрой СРАЗУ за ним
    /// — знак умножения (`x20` как имя в этой позиции недостижимо:
    /// слитная запись Numi-листа означает произведение).
    fn lex_ident(&mut self) -> Tok {
        if self.after_number {
            if let Some((name, len)) = self.unit_here() {
                self.pos += len;
                self.after_number = false;
                self.operand_ended = true;
                return Tok::Unit(name);
            }
        }
        // Слитное `35x20`/`35х20`: `x` ПОСЛЕ операнда + цифра сразу за ним
        if self.operand_ended {
            let mut chars = self.text[self.pos..].chars();
            let first = chars.next();
            if matches!(first, Some('x' | 'х'))
                && matches!(chars.next(), Some(c) if c.is_ascii_digit())
            {
                self.pos += first.map(char::len_utf8).unwrap_or(0);
                self.after_number = false;
                self.operand_ended = false;
                return Tok::Star;
            }
        }
        let start = self.pos;
        for ch in self.text[start..].chars() {
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        // FR-050 Р-6: `.` сразу за идентификатором (поле начинается с
        // буквы/`_`) — qualified-путь «Объект.Поле»; `.` перед цифрой —
        // прежняя семантика (дробное число, `x.5` = x · 0.5)
        if let Some(tok) = self.try_qualified(start) {
            self.after_number = false;
            self.operand_ended = true;
            return tok;
        }
        let ident = self.text[start..self.pos].to_owned();
        self.after_number = false;
        if self.operand_ended && matches!(ident.as_str(), "x" | "х") && self.operand_starts_ahead()
        {
            self.operand_ended = false;
            return Tok::Star;
        }
        self.operand_ended = true;
        Tok::Ident(ident)
    }

    /// FR-050 Р-6: остаток начинается с `.Поля` (точка + буква/`_`)?
    fn dot_field_starts(rest: &str) -> bool {
        let mut chars = rest.chars();
        if chars.next() != Some('.') {
            return false;
        }
        chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
    }

    /// FR-050 Р-6: прочитать поле qualified-пути после потреблённой точки —
    /// идентификатор, в котором дефис продолжается именем, если сразу за ним
    /// буква/цифра/`_` («Кол-во»); `Заявки.Кол - во` (пробел) — вычитание,
    /// поле «Кол».
    fn lex_qualified_field(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.text[self.pos..].chars().next() {
            if ch.is_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else if ch == '-'
                && self.text[self.pos + 1..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.text[start..self.pos].to_owned()
    }

    /// FR-050 Р-6: qualified-путь сразу за лексическим идентификатором
    /// объекта (`self.pos` — за базовым именем, `ident_start` — его начало).
    /// Ветвь А: `Объект.Поле`. Ветвь Б: `Объект (N).Поле` / `Объект (id).Поле`
    /// — суффикс коллизии имён (резолвер регистрирует и номер «Имя (2)»,
    /// и node_id-алиас «Имя (id)» — как в dataref::qualified_obj_name).
    /// Lookahead без потребления: суффикс опознаётся ТОЛЬКО целиком (до
    /// точки с полем) — `Заявки (2)` без точки остаётся прежней семантикой
    /// (вызов/умножение); `f (12, x)` — запятая рушит суффикс, остаётся
    /// вызовом. Нормализация: ровно один пробел перед скобкой суффикса.
    fn try_qualified(&mut self, ident_start: usize) -> Option<Tok> {
        let base_end = self.pos;
        let rest = &self.text[base_end..];
        if Self::dot_field_starts(rest) {
            self.pos = base_end + 1;
            let field = self.lex_qualified_field();
            let obj = self.text[ident_start..base_end].to_owned();
            return Some(Tok::Qualified { obj, field });
        }
        // Ветвь Б: суффикс ` (N)`/` (id)` + `.Поле`
        let after_spaces = rest.trim_start_matches([' ', '\t']);
        if !after_spaces.starts_with('(') {
            return None;
        }
        let open = base_end + (rest.len() - after_spaces.len());
        let mut close = open + 1;
        let mut closed = false;
        while let Some(ch) = self.text[close..].chars().next() {
            if ch == ')' {
                closed = true;
                break;
            }
            if !(ch.is_alphanumeric() || ch == '_' || ch == '-') {
                return None;
            }
            close += ch.len_utf8();
        }
        if !closed {
            return None;
        }
        let content = &self.text[open + 1..close];
        if content.is_empty() {
            return None;
        }
        let after_paren = &self.text[close + 1..];
        let trimmed = after_paren.trim_start_matches([' ', '\t']);
        if !Self::dot_field_starts(trimmed) {
            return None;
        }
        self.pos = close + 1 + (after_paren.len() - trimmed.len()) + 1;
        let field = self.lex_qualified_field();
        let obj = format!("{} ({})", &self.text[ident_start..base_end], content);
        Some(Tok::Qualified { obj, field })
    }

    fn take_while<F: Fn(u8) -> bool>(&mut self, pred: F) {
        while let Some(byte) = self.text.as_bytes().get(self.pos) {
            if pred(*byte) {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Прочитать следующий токен (или None в конце ввода).
    fn next(&mut self) -> Result<Option<Tok>, ParseError> {
        if let Some(popped) = self.peeked.take() {
            return Ok(popped);
        }
        // Пропуск пробелов (переводы строк — значимые разделители)
        while matches!(
            self.text.as_bytes().get(self.pos),
            Some(b' ' | b'\t' | b'\r')
        ) {
            self.pos += 1;
        }
        let byte = match self.text.as_bytes().get(self.pos) {
            Some(&byte) => byte,
            None => return Ok(None),
        };
        let tok = match byte {
            b'\n' | b';' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Sep
            }
            b'+' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Plus
            }
            b'-' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Minus
            }
            b'*' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Star
            }
            b'/' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Slash
            }
            b'(' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::LParen
            }
            b')' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = true;
                Tok::RParen
            }
            b',' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Comma
            }
            b'=' => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Assign
            }
            b'$' => {
                // FR-013: `$5` — префикс валюты; `%` — только суффикс
                // (после числа). FR-014: `$in` и целое `$N` (N ≥ 1) —
                // ссылки на входящие value-рёбра (контекст — Env).
                // `$5.5`/`$0`/`$ин` — обычная валюта или валюта × переменная.
                // FR-018: `$rps` (ASCII-имя без префикса `in`) — параметр
                // шаблонной ноды (`Env.params`).
                let rest = &self.text[self.pos + 1..];
                let in_word = rest.strip_prefix("in").is_some_and(|tail| {
                    !tail
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                });
                let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
                let int_amount = digits > 0
                    && !rest
                        .as_bytes()
                        .get(digits)
                        .is_some_and(|&b| b == b'.' || b.is_ascii_digit());
                if in_word {
                    self.pos += 3; // "$in"
                    self.after_number = false;
                    self.operand_ended = true;
                    Tok::DollarIn
                } else if int_amount {
                    let num: f64 = rest[..digits]
                        .parse()
                        .map_err(|_| self.err("некорректное число"))?;
                    self.pos += 1 + digits;
                    self.after_number = false;
                    self.operand_ended = true;
                    Tok::DollarNum(num)
                } else if let Some(name) = dollar_param_ident(rest) {
                    self.pos += 1 + name.len();
                    self.after_number = false;
                    self.operand_ended = true;
                    Tok::DollarIdent(name)
                } else {
                    self.pos += 1;
                    self.after_number = false;
                    self.operand_ended = false;
                    Tok::Unit("$")
                }
            }
            b'%' if self.after_number => {
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = true;
                Tok::Unit("%")
            }
            b'0'..=b'9' | b'.' => self.lex_number()?,
            // Идентификатор: ASCII-буквы/`_` и ЛЮБАЯ Unicode-буква
            // (кириллица: `х = 200`, `с = а + b` — смешанная раскладка)
            _ if self.text[self.pos..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_') =>
            {
                self.lex_ident()
            }
            _ if self.rest_starts("\u{2212}") => {
                // Типографский минус − (U+2212)
                self.pos += 3;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Minus
            }
            _ if self.rest_starts("\u{00d7}")
                || self.rest_starts("\u{00b7}")
                || self.rest_starts("\u{22c5}")
                || self.rest_starts("\u{2715}")
                || self.rest_starts("\u{2a2f}") =>
            {
                // × (U+00D7), · (U+00B7), ⋅ (U+22C5), ✕ (U+2715), ⨯ (U+2A2F) —
                // умножение
                self.pos += self.text[self.pos..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
                self.after_number = false;
                self.operand_ended = false;
                Tok::Star
            }
            _ if self.rest_starts("\u{00f7}") => {
                // ÷ (U+00F7) — деление
                self.pos += 2;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Slash
            }
            other => {
                return Err(ParseError {
                    pos: self.pos,
                    msg: format!(
                        "неподдерживаемый символ '{}'",
                        char::from_u32(other as u32).unwrap_or('?')
                    ),
                })
            }
        };
        Ok(Some(tok))
    }

    /// Число: цифры с необязательной дробной частью и суффиксом порядка
    /// (`1k` = 1e3, `2M` = 2e6, `3G` = 3e9; `B` — байты, не суффикс).
    fn lex_number(&mut self) -> Result<Tok, ParseError> {
        let start = self.pos;
        self.take_while(|b| b.is_ascii_digit());
        if self.text.as_bytes().get(self.pos) == Some(&b'.') {
            self.pos += 1;
            self.take_while(|b| b.is_ascii_digit());
        }
        let num_text = &self.text[start..self.pos];
        let mut num: f64 = num_text.parse().map_err(|_| ParseError {
            pos: start,
            msg: "некорректное число".to_owned(),
        })?;
        if let Some(&next) = self.text.as_bytes().get(self.pos) {
            let factor = match next {
                b'k' | b'K' => Some(1e3),
                b'M' => Some(1e6),
                b'G' => Some(1e9),
                _ => None,
            };
            if let Some(factor) = factor {
                num *= factor;
                self.pos += 1;
            }
        }
        self.after_number = true;
        self.operand_ended = true;
        Ok(Tok::Num(num))
    }

    /// Посмотреть следующий токен без чтения.
    fn peek(&mut self) -> Result<Option<Tok>, ParseError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next()?);
        }
        Ok(self.peeked.as_ref().expect("заполнен выше").clone())
    }

    /// Съесть уже просмотренный токен (после peek).
    fn eat(&mut self) {
        self.peeked = None;
    }
}

// --- Разбор ---

/// Разобрать формулу Numi-base: последовательность утверждений, разделённых
/// переводами строк/`;`. Программа из одного утверждения возвращается без
/// обёртки [`Expr::Block`] (`parse("5") == Expr::Num(5.0, Scalar)`).
pub fn parse(input: &str) -> Result<Expr, ParseError> {
    let mut lexer = Lexer::new(input);
    let mut statements: Vec<Expr> = Vec::new();
    while let Some(tok) = lexer.next()? {
        match tok {
            Tok::Sep => {} // граница утверждений
            Tok::Star | Tok::Slash | Tok::RParen | Tok::Comma => {
                return Err(lexer.err("неожиданный оператор"));
            }
            // FR-013 (правка 3, Numi): хвостовое присваивание —
            // `выражение = имя` (`123 + 5123 = a`): имя связывается с
            // результатом последнего утверждения. Повтор то же имени в конце
            // (`b = 235 + 2323 = b` — форма «в обе стороны») игнорируется.
            Tok::Assign => {
                let name = match lexer.next()? {
                    Some(Tok::Ident(name)) => name,
                    _ => return Err(lexer.err("ожидалось имя после «=»")),
                };
                if !matches!(lexer.peek()?, None | Some(Tok::Sep)) {
                    return Err(lexer.err("имя после «=» должно завершать утверждение"));
                }
                let rhs = statements
                    .pop()
                    .ok_or_else(|| lexer.err("неожиданный оператор"))?;
                match rhs {
                    Expr::Assign { name: ref same, .. } if *same == name => statements.push(rhs),
                    other => statements.push(Expr::Assign {
                        name,
                        rhs: Box::new(other),
                    }),
                }
            }
            // Начала утверждений (в т.ч. унарный знак: `-3 ms`; FR-014:
            // `$in`/`$5` — операнды-ссылки; FR-018: `$rps` — параметр;
            // FR-050 Р-6: «Объект.Поле» — именованная ссылка на вход)
            Tok::Num(_)
            | Tok::Ident(_)
            | Tok::Unit(_)
            | Tok::DollarIn
            | Tok::DollarNum(_)
            | Tok::DollarIdent(_)
            | Tok::Qualified { .. }
            | Tok::LParen
            | Tok::Plus
            | Tok::Minus => {
                statements.push(parse_statement(&mut lexer, tok)?);
            }
        }
    }
    if statements.is_empty() {
        return Err(ParseError {
            pos: 0,
            msg: "пустая формула".to_owned(),
        });
    }
    if statements.len() == 1 {
        return Ok(statements.remove(0));
    }
    Ok(Expr::Block(statements))
}

/// Одно утверждение: `name = expr` или чистое выражение.
fn parse_statement(lexer: &mut Lexer, first: Tok) -> Result<Expr, ParseError> {
    if let Tok::Ident(name) = first {
        if matches!(lexer.peek()?, Some(Tok::Assign)) {
            lexer.eat(); // `=`
            let rhs = parse_expr(lexer)?;
            return Ok(Expr::Assign {
                name,
                rhs: Box::new(rhs),
            });
        }
        return parse_expr_from(lexer, Tok::Ident(name));
    }
    parse_expr_from(lexer, first)
}

/// Разбор выражения, начиная с уже прочитанного токена.
fn parse_expr_from(lexer: &mut Lexer, first: Tok) -> Result<Expr, ParseError> {
    let unary = parse_unary_from(lexer, first)?;
    // Левый операнд тоже проходит уровень умножений (`5 ms × 200 req/s`)
    let lhs = parse_mul_tail(lexer, unary)?;
    parse_add_tail(lexer, lhs)
}

fn parse_expr(lexer: &mut Lexer) -> Result<Expr, ParseError> {
    let tok = lexer
        .next()?
        .ok_or_else(|| lexer.err("ожидалось выражение"))?;
    parse_expr_from(lexer, tok)
}

/// Сложение/вычитание (низший приоритет).
fn parse_add_tail(lexer: &mut Lexer, lhs: Expr) -> Result<Expr, ParseError> {
    let mut lhs = lhs;
    loop {
        let op = match lexer.peek()? {
            Some(Tok::Plus) => {
                lexer.eat();
                BinOp::Add
            }
            Some(Tok::Minus) => {
                lexer.eat();
                BinOp::Sub
            }
            _ => return Ok(lhs),
        };
        let rhs = parse_mul(lexer)?;
        lhs = Expr::Bin {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
}

/// Умножение/деление и НЕЯВНОЕ умножение (`5 ms`, `3 (50 rps)`, `$5`) —
/// хвост после первого операнда.
fn parse_mul_tail(lexer: &mut Lexer, mut lhs: Expr) -> Result<Expr, ParseError> {
    loop {
        let (op, implicit) = match lexer.peek()? {
            Some(Tok::Star) => {
                lexer.eat();
                (BinOp::Mul, false)
            }
            Some(Tok::Slash) => {
                lexer.eat();
                (BinOp::Div, false)
            }
            // Сопоставление без знака: число/единица/скобка/переменная/вход
            // сразу за операндом (`5 ms`, `3 replicas` из примера владельца,
            // `(a + b) ms`, `2 $in`); Ident `(` уже разобран как вызов
            // на уровне primary; FR-050 Р-6: qualified-путь — операнд
            // неявного умножения (`2 Курсы.USD`)
            Some(
                Tok::Num(_)
                | Tok::Unit(_)
                | Tok::DollarIn
                | Tok::DollarNum(_)
                | Tok::DollarIdent(_)
                | Tok::Qualified { .. }
                | Tok::LParen
                | Tok::Ident(_),
            ) => (BinOp::Mul, true),
            _ => return Ok(lhs),
        };
        // В позиции СОПОСТАВЛЕНИЯ идентификатор с именем из таблицы —
        // единица (`(5-10) ms`), иначе переменная (`3 replicas`); в позиции
        // явного оператора (`latency × rps`) имя — всегда переменная
        let rhs = if implicit {
            match lexer.peek()? {
                Some(Tok::Ident(name)) if unit_atom(&name).is_some() => {
                    lexer.eat();
                    unit_literal(&name)
                }
                _ => parse_unary(lexer)?,
            }
        } else {
            parse_unary(lexer)?
        };
        lhs = Expr::Bin {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
    }
}

/// Умножение/деление со старта (правые операнды сложения).
fn parse_mul(lexer: &mut Lexer) -> Result<Expr, ParseError> {
    let lhs = parse_unary(lexer)?;
    parse_mul_tail(lexer, lhs)
}

fn parse_unary(lexer: &mut Lexer) -> Result<Expr, ParseError> {
    let tok = lexer
        .next()?
        .ok_or_else(|| lexer.err("ожидалось выражение"))?;
    parse_unary_from(lexer, tok)
}

/// Первичное выражение с унарным минусом.
fn parse_unary_from(lexer: &mut Lexer, first: Tok) -> Result<Expr, ParseError> {
    match first {
        Tok::Minus => {
            let inner = parse_unary(lexer)?;
            Ok(Expr::Neg(Box::new(inner)))
        }
        Tok::Num(num) => parse_unit_run(lexer, num),
        Tok::Unit(name) => Ok(unit_literal(name)),
        // FR-014: `$in` — единственный вход; `$N` — валюта или вход
        // (разрешение — в eval по наличию входов в Env)
        Tok::DollarIn => Ok(Expr::Inbound),
        Tok::DollarNum(num) => Ok(Expr::DollarAmount(num)),
        // FR-018: `$rps` — параметр шаблона
        Tok::DollarIdent(name) => Ok(Expr::Param(name)),
        // FR-050 Р-6: именованный путь «Объект.Поле» — ссылка на вход
        Tok::Qualified { obj, field } => Ok(Expr::Qualified { obj, field }),
        Tok::Ident(name) => {
            // Вызов функции: Ident `(` args `)`; иначе — переменная
            if matches!(lexer.peek()?, Some(Tok::LParen)) {
                lexer.eat(); // `(`
                let mut args = Vec::new();
                if !matches!(lexer.peek()?, Some(Tok::RParen)) {
                    loop {
                        args.push(parse_expr(lexer)?);
                        match lexer.next()? {
                            Some(Tok::Comma) => continue,
                            Some(Tok::RParen) => break,
                            _ => return Err(lexer.err("ожидалась `,` или `)` в вызове")),
                        }
                    }
                } else {
                    lexer.eat(); // `)`
                }
                return Ok(Expr::Call { func: name, args });
            }
            Ok(Expr::Var(name))
        }
        Tok::LParen => {
            let inner = parse_expr(lexer)?;
            match lexer.next()? {
                Some(Tok::RParen) => Ok(inner),
                _ => Err(lexer.err("ожидалась закрывающая скобка")),
            }
        }
        _ => Err(lexer.err("неожиданный токен")),
    }
}

/// Число + цепочка единиц (`1000 rps`) — неявное умножение на каждую.
fn parse_unit_run(lexer: &mut Lexer, num: f64) -> Result<Expr, ParseError> {
    let mut value = Expr::Num(num, Unit::Scalar);
    while let Some(Tok::Unit(name)) = lexer.peek()? {
        lexer.eat();
        let unit = Expr::Num(1.0, Unit::atom(unit_atom(name).expect("токен из таблицы")));
        value = Expr::Bin {
            op: BinOp::Mul,
            lhs: Box::new(value),
            rhs: Box::new(unit),
        };
    }
    Ok(value)
}

fn unit_literal(name: &str) -> Expr {
    Expr::Num(1.0, Unit::atom(unit_atom(name).expect("токен из таблицы")))
}

// --- Вычисление ---

/// FR-014: `$in` — значение единственного входа.
fn eval_inbound_auto(env: &Env) -> Result<Value, EvalError> {
    let Some(slots) = env.inbound_slots() else {
        return Err(EvalError::MissingInbound { index: 0 });
    };
    if slots.len() > 1 {
        return Err(EvalError::AmbiguousInbound { count: slots.len() });
    }
    env.inbound_value(0)
        .cloned()
        .ok_or(EvalError::MissingInbound { index: 0 })
}

/// Вычислить формулу в окружении `env` (чистая функция, инвариант 2 FR-013).
/// Блок заводит локальную область поверх `env`; результат — значение
/// последнего утверждения.
pub fn eval(expr: &Expr, env: &Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Num(num, unit) => Ok(Value {
            num: *num,
            unit: unit.clone(),
        }),
        // FR-014: `$in` и `$N` (см. DollarAmount)
        Expr::Inbound => eval_inbound_auto(env),
        Expr::DollarAmount(n) => {
            if env.has_inbound() && *n >= 1.0 && n.fract() == 0.0 {
                // Целое $N при наличии входов — ссылка на N-й вход
                let index = (*n as usize) - 1;
                env.inbound_value(index)
                    .cloned()
                    .ok_or(EvalError::MissingInbound { index })
            } else {
                // Валюта (FR-013): без входов, а также $0 и дробные суммы
                Ok(Value {
                    num: *n,
                    unit: Unit::atom(unit_atom("$").expect("валюта в таблице единиц")),
                })
            }
        }
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UnknownVariable(name.clone())),
        // FR-018: параметр шаблона — отдельное пространство имён за `$`
        Expr::Param(name) => env
            .param(name)
            .cloned()
            .ok_or_else(|| EvalError::UnknownParam(name.clone())),
        // FR-050 Р-6: именованный путь «Объект.Поле» — входящее значение по
        // графу рёбер; нет такой пары — видимая ошибка (не тихая проза)
        Expr::Qualified { obj, field } => env
            .qualified_value(obj, field)
            .cloned()
            .ok_or_else(|| EvalError::UnknownInput(format!("{obj}.{field}"))),
        Expr::Neg(inner) => {
            let value = eval(inner, env)?;
            Ok(Value {
                num: -value.num,
                unit: value.unit,
            })
        }
        Expr::Bin { op, lhs, rhs } => {
            let lhs = eval(lhs, env)?;
            let rhs = eval(rhs, env)?;
            eval_bin(*op, lhs, rhs)
        }
        Expr::Call { func, args } => eval_call(func, args, env),
        // Вне блока связывание не переживает вычисление — значение возвращается
        Expr::Assign { rhs, .. } => eval(rhs, env),
        Expr::Block(statements) => {
            let mut scope = env.clone();
            let mut last = Value::scalar(0.0);
            for statement in statements {
                last = match statement {
                    Expr::Assign { name, rhs } => {
                        let value = eval(rhs, &scope)?;
                        scope.vars.insert(name.clone(), value.clone());
                        value
                    }
                    other => eval(other, &scope)?,
                };
            }
            Ok(last)
        }
    }
}

/// FR-013 (правка 2, Numi-стиль): построчное вычисление текста ноды.
/// Строки образуют один сценарий с общим окружением — переменные,
/// объявленные выше, видны ниже (Numi). Строка с префиксом `=` — ЯВНАЯ
/// формула: ошибки парсинга/вычисления показываются; чистая строка,
/// парсящаяся как выражение, — авто-формула: показ ошибок выборочный
/// (см. [`auto_error_visible`]), прочие ошибки строку результата не
/// создают (проза «Встреча в 15:00» не краснеет).
/// Присваивание (`rps = 1000`) связывает переменную и возвращает
/// присвоенное значение. Внутри код-фенсов (``` … ```) формулы не
/// вычисляются — код не калькулятор.
pub fn eval_lines(source: &str) -> Vec<Option<ExprOutcome>> {
    eval_lines_in(source, &Env::empty())
}

/// FR-014: [`eval_lines`] с входящими значениями value-рёбер — строки листа
/// (`= $in × 2`) видят входы ноды. Входы читаются, локальные переменные
/// листа наслаиваются сверху.
pub fn eval_lines_in(source: &str, inbound: &Env) -> Vec<Option<ExprOutcome>> {
    eval_lines_with_env(source, inbound).0
}

/// FR-029: [`eval_lines_in`] с возвратом ФИНАЛЬНОГО окружения листа —
/// переменные, объявленные присваиваниями (`peak_rps = avg × 2.5`),
/// доступны по имени как именованные выходы текстовой ноды (адресация
/// рёбер `fromOutput` живёт при сдвиге строк: имя стабильнее индекса).
/// Окружение = база (входы/параметры) + переменные листа.
pub fn eval_lines_with_env(source: &str, inbound: &Env) -> (Vec<Option<ExprOutcome>>, Env) {
    // FR-013 (правка 5): канонический текст заметки экранирует литеральные
    // `=` (`\=` — от пары `==` подсветки в диалекте CanvasDesk; заметки
    // прежних сборок содержат `x \= 200` для КАЖДОГО `=`). Расчёт ведётся
    // по видимому тексту — экранирование снимается; проза и код-фенсы не
    // меняются (они не считаются).
    let source = source.replace("\\=", "=");
    let mut env = inbound.clone();
    let mut in_fence = false;
    // FR-013 (правка 4): имена, объявленные строками ВЫШЕ (похожими на
    // присваивание, даже не парсящимися или не вычислившимися) — контекст
    // показа ошибок UnknownVariable в ссылках ниже по листу.
    let mut declared: HashSet<String> = HashSet::new();
    let mut results = Vec::new();
    for line in source.split('\n') {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            results.push(None);
            continue;
        }
        if !in_fence {
            scan_declared_names(line, &mut declared);
        }
        results.push(eval_line(line, &mut env, &declared, in_fence));
    }
    (results, env)
}

/// Одна строка сценария: результат или None (не формула / тихая ошибка).
fn eval_line(
    line: &str,
    env: &mut Env,
    declared: &HashSet<String>,
    in_fence: bool,
) -> Option<ExprOutcome> {
    if in_fence {
        return None;
    }
    let trimmed = line.trim();
    let (statement, explicit) = match trimmed.strip_prefix('=') {
        Some(rest) => (rest.trim(), true),
        None => (trimmed, false),
    };
    if statement.is_empty() {
        return None;
    }
    match parse(statement) {
        Ok(parsed) => match eval_statement(&parsed, env) {
            Ok(value) => Some(ExprOutcome::Ok(value)),
            // Явная формула показывает ошибку вычисления; авто-строка —
            // только «заслуженную» (auto_error_visible)
            Err(err) if explicit => Some(ExprOutcome::Err(err.to_string())),
            Err(err) if auto_error_visible(&err, declared, statement) => {
                Some(ExprOutcome::Err(err.to_string()))
            }
            Err(_) => None,
        },
        // Явная формула показывает ошибку парсинга; авто-строка, ПОХОЖАЯ
        // на присваивание имени, тоже (пользователь явно описал связывание,
        // опечатка в значении не должна молчать); остальная проза молчит
        Err(err) if explicit => Some(ExprOutcome::Err(err.to_string())),
        Err(err) if assignment_shaped(statement) => Some(ExprOutcome::Err(err.to_string())),
        Err(_) => None,
    }
}

/// FR-013 (правка 4/5): видна ли ошибка вычисления АВТО-строки (без
/// префикса `=`). Numi-принцип: проза никогда не краснеет, но сломанный
/// расчёт — не проза. «Заслуженные» ошибки: несовместимость единиц,
/// деление на ноль, битый вызов (строка вычислилась как чистая
/// арифметика) и ссылка на переменную, ОБЪЯВЛЕННУЮ строкой выше (`x = …`
/// выше, даже если та строка сама ошибочна). Правка 5: ссылка на
/// НЕОБЪЯВЛЕННУЮ переменную тоже видна, если строка сама — присваивание
/// (`c = a + b` при отсутствии `a`) — пользователь явно описал связывание,
/// это расчёт, а не проза; тишина здесь и была «ошибки не выводятся».
/// Неизвестное слово вне объявлений (`- 5 яблок`) по-прежнему молчит.
fn auto_error_visible(err: &EvalError, declared: &HashSet<String>, statement: &str) -> bool {
    match err {
        EvalError::UnknownVariable(name) => declared.contains(name) || assignment_shaped(statement),
        // FR-018: `$param` вне шаблона молчит, как и проза с неизвестным
        // словом; в шаблонной ноде params приходят из flow — ошибки там
        // всегда видны
        EvalError::UnknownParam(_) => assignment_shaped(statement),
        _ => true,
    }
}

/// Похоже ли утверждение на присваивание имени: `имя = …` слева или
/// `… = имя` справа от первого `=`. Проза так не выглядит.
fn assignment_shaped(statement: &str) -> bool {
    let Some(eq) = statement.find('=') else {
        return false;
    };
    let (left, right) = statement.split_at(eq);
    single_identifier(left) || single_identifier(&right[1..])
}

/// Один идентификатор (ничего кроме букв/цифр/`_`, начинается с буквы или
/// `_`) — имя переменной Numi.
fn single_identifier(text: &str) -> bool {
    let text = text.trim();
    let Some(first) = text.chars().next() else {
        return false;
    };
    (first.is_alphabetic() || first == '_') && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// FR-013 (правка 4): объявления строки — левая часть `имя = …` и хвостовое
/// `… = имя`; имена запоминаются, даже если строка ошибочна, чтобы ссылки
/// на них ниже показывали ошибку «переменная объявлена, но не вычислена».
fn scan_declared_names(line: &str, declared: &mut HashSet<String>) {
    let trimmed = line.trim();
    let statement = trimmed.strip_prefix('=').map(str::trim).unwrap_or(trimmed);
    let Some(eq) = statement.find('=') else {
        return;
    };
    let (left, right) = statement.split_at(eq);
    if single_identifier(left) {
        declared.insert(left.trim().to_owned());
    }
    if single_identifier(&right[1..]) {
        declared.insert(right[1..].trim().to_owned());
    }
}

/// Вычислить утверждение со связыванием присваиваний в ПЕРЕДАННОЕ
/// окружение (в отличие от [`eval`], где Assign вне блока не переживает
/// вычисление). Блок (`;` в одной строке) разворачивается в общее
/// окружение сценария — Numi-семантика листа расчёта.
fn eval_statement(expr: &Expr, env: &mut Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Assign { name, rhs } => {
            let value = eval(rhs, env)?;
            env.vars.insert(name.clone(), value.clone());
            Ok(value)
        }
        Expr::Block(statements) => {
            let mut last = Value::scalar(0.0);
            for statement in statements {
                last = eval_statement(statement, env)?;
            }
            Ok(last)
        }
        other => eval(other, env),
    }
}

/// Двоичные операции с единицами (см. доку модуля): `+`/`-` — одна
/// размерность, правый операнд конвертируется к единице левого;
/// `*`/`/` — символическое слияние/сокращение атомов.
fn eval_bin(op: BinOp, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
    match op {
        BinOp::Add | BinOp::Sub => {
            if lhs.dims() != rhs.dims() {
                return Err(EvalError::UnitMismatch {
                    lhs: lhs.to_string(),
                    rhs: rhs.to_string(),
                });
            }
            // Правый операнд → единица левого: n·scale(r)/scale(l)
            let lhs_scale = lhs.unit.scale();
            let converted = if lhs_scale == 0.0 {
                rhs.num
            } else {
                rhs.num * rhs.unit.scale() / lhs_scale
            };
            let num = if op == BinOp::Add {
                lhs.num + converted
            } else {
                lhs.num - converted
            };
            Ok(Value {
                num,
                unit: lhs.unit,
            })
        }
        BinOp::Mul => {
            let raw = lhs.num * rhs.num;
            let raw_scale = lhs.unit.scale() * rhs.unit.scale();
            let unit = lhs.unit.merged(&rhs.unit, 1);
            Ok(Value {
                num: rescaled(raw, raw_scale, &unit),
                unit,
            })
        }
        BinOp::Div => {
            if rhs.num == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            let raw = lhs.num / rhs.num;
            let raw_scale = lhs.unit.scale() / rhs.unit.scale();
            let unit = lhs.unit.merged(&rhs.unit, -1);
            Ok(Value {
                num: rescaled(raw, raw_scale, &unit),
                unit,
            })
        }
    }
}

/// FR-018: num результата ×/÷ — в масштабе итоговой единицы. `merged`
/// нормализует `Count¹·Time⁻¹ → Rate¹`; если время было не в базовых sec,
/// масштаб единицы меняется (`1 req / 10 ms` — это 100 req/s, а не «0.1
/// req/s» с потерей ×1000 в базе). raw пересчитывается: raw × raw_scale /
/// new_scale (база сохранена). Если масштаб не сменился — сырой num
/// операндов (арифметика и точность прежнего поведения).
fn rescaled(raw: f64, raw_scale: f64, unit: &Unit) -> f64 {
    let new_scale = unit.scale();
    if (raw_scale - new_scale).abs() <= raw_scale.abs() * 1e-9 {
        raw
    } else {
        raw * raw_scale / new_scale
    }
}

/// Функции v1: агрегаты по аргументам одной размерности; результат —
/// в единице первого аргумента. `percentile(p, …)` — линейная интерполяция
/// (PERCENTILE.INC), `p` — скаляр 0..=100.
fn eval_call(func: &str, args: &[Expr], env: &Env) -> Result<Value, EvalError> {
    if matches!(func, "sum" | "avg" | "max" | "min") && args.is_empty() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: "нужен хотя бы один аргумент".to_owned(),
        });
    }
    if func == "percentile" && args.len() < 2 {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: "нужны p и хотя бы одно значение".to_owned(),
        });
    }
    let values: Vec<Value> = args
        .iter()
        .map(|arg| eval(arg, env))
        .collect::<Result<_, _>>()?;
    match func {
        "sum" => fold_values("sum", &values, |acc, v| acc + v),
        "avg" => {
            let total = fold_values("avg", &values, |acc, v| acc + v)?;
            Ok(Value {
                num: total.num / values.len() as f64,
                unit: total.unit,
            })
        }
        "max" => extremes("max", &values, |a, b| a > b),
        "min" => extremes("min", &values, |a, b| a < b),
        "percentile" => eval_percentile(&values),
        // FR-015: доменные функции теории очередей (чистые, см. queueing.rs);
        // FR-027: расширение — 4 финансовые функции (npv/cagr/irr/cohort_ltv).
        "utilization" | "mm1" | "mmc" | "littles_law" | "erlang_c" | "npv" | "cagr" | "irr"
        | "cohort_ltv" => queueing::dispatch(func, &values),
        // FR-063: доменный слой статистики (L2) — только за фичей `stats`
        // (список имён — единая точка `stats::STATS_FUNCTIONS`, parity-тест с
        // FN_HINTS обязателен). Без фичи arm не существует и имена падают в
        // fallback ниже — graceful-деградация UnknownFunction (контракт FR-063
        // «Контракты на стыках» §5.3: существующие arms не тронуты).
        #[cfg(feature = "stats")]
        other if stats::is_stats_function(other) => stats::dispatch(func, &values),
        other => Err(EvalError::UnknownFunction(other.to_owned())),
    }
}

/// max/min: аргументы одной размерности, единица — первого аргумента.
fn extremes(
    func: &str,
    values: &[Value],
    better: fn(f64, f64) -> bool,
) -> Result<Value, EvalError> {
    let base = &values[0];
    let mut best = base.clone();
    for v in &values[1..] {
        check_same_dims(func, base, v)?;
        if better(v.num, best.num) {
            best = v.clone();
        }
    }
    Ok(best)
}

/// Сумма значений одной размерности (единица — первого).
fn fold_values(func: &str, values: &[Value], op: fn(f64, f64) -> f64) -> Result<Value, EvalError> {
    let base = &values[0];
    let mut num = base.num;
    for v in &values[1..] {
        check_same_dims(func, base, v)?;
        num = op(num, v.num);
    }
    Ok(Value {
        num,
        unit: base.unit.clone(),
    })
}

fn check_same_dims(func: &str, base: &Value, other: &Value) -> Result<(), EvalError> {
    if base.dims() != other.dims() {
        return Err(EvalError::BadCall {
            func: func.to_owned(),
            msg: format!("разные размерности: {} и {}", base, other),
        });
    }
    Ok(())
}

/// `percentile(p, v…)`: `p` — скаляр 0..=100; значения одной размерности.
fn eval_percentile(values: &[Value]) -> Result<Value, EvalError> {
    let (p, rest) = values.split_first().expect("арность проверена");
    if !p.unit.is_scalar() {
        return Err(EvalError::BadCall {
            func: "percentile".to_owned(),
            msg: "p должен быть скаляром 0..100".to_owned(),
        });
    }
    if !(0.0..=100.0).contains(&p.num) {
        return Err(EvalError::BadCall {
            func: "percentile".to_owned(),
            msg: format!("p вне диапазона 0..100: {}", format_num(p.num)),
        });
    }
    let base = &rest[0];
    let mut nums = Vec::with_capacity(rest.len());
    for v in rest {
        check_same_dims("percentile", base, v)?;
        nums.push(v.num);
    }
    nums.sort_by(|a, b| a.partial_cmp(b).expect("конечные числа"));
    Ok(Value {
        num: percentile_inc(&nums, p.num),
        unit: base.unit.clone(),
    })
}

/// Линейная интерполяция перцентиля (PERCENTILE.INC); `sorted` по возрастанию.
fn percentile_inc(sorted: &[f64], p: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let index = (p / 100.0) * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    if lower == upper {
        return sorted[lower];
    }
    let frac = index - lower as f64;
    sorted[lower] + (sorted[upper] - sorted[lower]) * frac
}

// --- What-if дельты (FR-017/PRD-0007 F-6) --------------------------------

/// FR-017 (CP6): строка дельты «(+Δ)» между базовым и what-if значением.
/// Формат полный (гипотеза Q4): % — в процентных пунктах, иначе абсолют
/// с единицей (Q4c). None — значения совпадают (дельты нет).
///
/// PRD-0007 X3: перенесено из `canvas-scene` в ядро — формат нужен и
/// `lineage_deltas` (explain-дерево, AC-4.2), и карточкам канваса.
pub fn whatif_delta_str(base: &Value, whatif: &Value) -> Option<String> {
    let delta = whatif.num - base.num;
    if delta.abs() < 1e-9 {
        return None;
    }
    let rounded = (delta * 100.0).round() / 100.0;
    let unit = whatif.unit.display();
    if unit == "%" {
        Some(format!("{rounded:+.0} пп"))
    } else if unit.is_empty() {
        Some(format!("{rounded:+}"))
    } else {
        Some(format!("{rounded:+} {unit}"))
    }
}

/// FR-017: полный формат дельта-бейджа «было → стало (+Δ)» (гипотеза Q4) —
/// то, что рендер показывает вместо голого значения изменившейся строки/
/// итога. Без изменений — None (бейдж остаётся обычным). D-1 (FR-061):
/// композиция над [`whatif_delta_parts`] — строковый формат и структурные
/// части собираются из одной точки (байт-паритет тестом).
pub fn whatif_full_delta(base: &Value, whatif: &Value) -> Option<String> {
    let p = whatif_delta_parts(base, whatif)?;
    Some(format!(
        "{} → {} ({})",
        join_parts_of(&p.base),
        join_parts_of(&p.new),
        p.delta
    ))
}

/// D-1 (FR-061, табличное тело ноды): структурные части what-if бейджа —
/// `base`/`new` как `(число, юнит)` (те же части, что у [`Value::
/// display_parts`]) + строка дельты `delta` (единственная точка спец-логики
/// знака и «пп» — [`whatif_delta_str`], поведение не меняется). Потребитель —
/// бейдж-колонка таблицы (этап B): «было → стало (+Δ)» раскладывается по
/// ячейкам без повторного парсинга строки.
#[derive(Debug, Clone, PartialEq)]
pub struct DeltaParts {
    pub base: (String, String),
    pub new: (String, String),
    pub delta: String,
}

/// Части what-if дельты; `None` ⇔ дельты нет — тот же предикат, что и
/// [`whatif_full_delta`] (согласованность тестом).
pub fn whatif_delta_parts(base: &Value, whatif: &Value) -> Option<DeltaParts> {
    let delta = whatif_delta_str(base, whatif)?;
    Some(DeltaParts {
        base: base.display_parts(),
        new: whatif.display_parts(),
        delta,
    })
}

// --- FR-021: каталог подсказок и детектор рода строки ---

/// Подсказка функции (FR-021): имя, сигнатура, описание. Каталог —
/// публичный источник правды UI подсказок; синхронность с диспетчером
/// [`eval_call`] фиксируется тестом (каждая функция парсится грамматикой).
#[derive(Debug, Clone, PartialEq)]
pub struct FnHint {
    pub name: &'static str,
    pub signature: &'static str,
    pub summary: &'static str,
}

/// Каталог функций движка (FR-021): статистика FR-013 + queueing-набор
/// FR-015/FR-027. Сигнатуры синхронны `eval_call`/`queueing::dispatch`.
/// FR-063: stats-записи за фичей `stats`, parity с диспетчером — тестом.
pub const FN_HINTS: &[FnHint] = &[
    FnHint {
        name: "sum",
        signature: "sum(x, …)",
        summary: "сумма значений одной размерности",
    },
    FnHint {
        name: "avg",
        signature: "avg(x, …)",
        summary: "среднее значений одной размерности",
    },
    FnHint {
        name: "max",
        signature: "max(x, …)",
        summary: "максимум",
    },
    FnHint {
        name: "min",
        signature: "min(x, …)",
        summary: "минимум",
    },
    FnHint {
        name: "percentile",
        signature: "percentile(p, x, …)",
        summary: "перцентиль p (0..100), p — скаляр",
    },
    FnHint {
        name: "utilization",
        signature: "utilization(λ, μ[, c])",
        summary: "загрузка системы ρ = λ/(c·μ)",
    },
    FnHint {
        name: "mm1",
        signature: "mm1(λ, μ[, c])",
        summary: "M/M/1: отклик и очередь",
    },
    FnHint {
        name: "mmc",
        signature: "mmc(λ, μ, c)",
        summary: "M/M/c: c обслуживающих каналов",
    },
    FnHint {
        name: "littles_law",
        signature: "littles_law(λ, W)",
        summary: "закон Литтла: L = λ·W",
    },
    FnHint {
        name: "erlang_c",
        signature: "erlang_c(λ, μ, c)",
        summary: "вероятность ожидания Эрланга C",
    },
    // FR-027: финансовые функции (были в движке с FR-027, но отсутствовали
    // в каталоге — устранён пробел parity FR-021 при добавлении FR-063).
    FnHint {
        name: "npv",
        signature: "npv(rate, cf, …)",
        summary: "чистая приведённая стоимость потоков",
    },
    FnHint {
        name: "cagr",
        signature: "cagr(begin, end, periods)",
        summary: "среднегодовой темп роста",
    },
    FnHint {
        name: "irr",
        signature: "irr(cf, …)",
        summary: "внутренняя норма доходности",
    },
    FnHint {
        name: "cohort_ltv",
        signature: "cohort_ltv(arpu_m0, margin, r_d1, r_d7, r_d30, months)",
        summary: "LTV когорты через retention-кривую",
    },
    // FR-063 P2: распределения и квантили (за фичей `stats`, parity-тест —
    // stats_fn_hints_parity_with_eval_call).
    #[cfg(feature = "stats")]
    FnHint {
        name: "normal_quantile",
        signature: "normal_quantile(p, μ, σ)",
        summary: "квантиль N(μ, σ²): P95/P99 на канвасе",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "normal_cdf",
        signature: "normal_cdf(x, μ, σ)",
        summary: "вероятность X ≤ x для N(μ, σ²)",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "lognormal_quantile",
        signature: "lognormal_quantile(p, μ, σ)",
        summary: "квантиль LogNormal (параметры — лог-пространство)",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "exp_quantile",
        signature: "exp_quantile(p, λ)",
        summary: "квантиль экспоненциального распределения",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "poisson_pmf",
        signature: "poisson_pmf(k, λ)",
        summary: "вероятность ровно k событий (Пуассон)",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "triangular_quantile",
        signature: "triangular_quantile(p, a, b, c)",
        summary: "квантиль треугольного на [a, b] с модой c",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "triangular",
        signature: "triangular(p, a, b, c)",
        summary: "алиас triangular_quantile",
    },
    // FR-063 P3: ДИ и детерминированные выборки (за фичей `stats`).
    #[cfg(feature = "stats")]
    FnHint {
        name: "ci_mean",
        signature: "ci_mean(mean, sigma, n, conf)",
        summary: "полуширина ДИ среднего (норм. аппроксимация)",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "normal_sample",
        signature: "normal_sample(μ, σ, n, seed)",
        summary: "среднее детерминированной выборки N(μ, σ²)",
    },
    #[cfg(feature = "stats")]
    FnHint {
        name: "lognormal_sample",
        signature: "lognormal_sample(μ, σ, n, seed)",
        summary: "среднее выборки LogNormal (лог-пространство)",
    },
];

/// Срез каталога функций для подсказок (FR-021).
pub fn fn_hints() -> &'static [FnHint] {
    FN_HINTS
}

/// Токены единиц (`UNIT_TABLE`) для подсказок после числа (FR-021);
/// порядок — порядок таблицы (детерминизм списка).
pub fn unit_tokens() -> Vec<&'static str> {
    UNIT_TABLE.iter().map(|(token, _, _)| *token).collect()
}

/// Род строки для подсказок (FR-021) — вердикт [`eval_lines`] над одной
/// строкой. `Assignment` — присваивание (`rps = 1000`), `Expression` —
/// явная `= …` или выражение (включая ошибочное: явные ошибки видны,
/// присваиваниеподобные — тоже), `Prose` — проза (подсказки не открываются).
#[derive(Debug, Clone, PartialEq)]
pub enum NumiLineKind {
    Assignment { name: String },
    Expression,
    Prose,
}

/// Детектор рода строки (FR-021): чистая функция над одной строкой;
/// вердикт ПОЛНОСТЬЮ совпадает с движком ([`eval_line`] над одной строкой
/// с пустым окружением): строка Numi, если движок дал бы результат или
/// видимую ошибку, проза — если движок молчит (`встреча в 3` парсится как
/// неявное умножение, но неизвестные слова молчат — это проза).
/// Код-фенсы — контекст ВЫШЕ строки; вызывающий (UI) сам не вызывает
/// детектор внутри фенса.
pub fn line_kind(line: &str) -> NumiLineKind {
    // Канонический текст экранирует литеральный `=` (`\=`) — снимаем,
    // как eval_lines
    let line = line.replace("\\=", "=");
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return NumiLineKind::Prose;
    }
    // Присваивание: единственный идентификатор слева от первого `=`
    let statement = trimmed.strip_prefix('=').map(str::trim).unwrap_or(trimmed);
    if statement.is_empty() {
        return NumiLineKind::Prose;
    }
    if let Some(eq) = statement.find('=') {
        let left = statement[..eq].trim();
        if single_identifier(left) {
            return NumiLineKind::Assignment {
                name: left.to_owned(),
            };
        }
    }
    // Полный вердикт движка над одной строкой (без объявлений выше):
    // результат ИЛИ видимая ошибка (явный `= …`, присваиваниеподобная,
    // единицы/деление/битый вызов) — Numi; молчание — проза
    let mut env = Env::empty();
    match eval_line(&line, &mut env, &HashSet::new(), false) {
        Some(_) => NumiLineKind::Expression,
        None => NumiLineKind::Prose,
    }
}

/// FR-061 D-7 / FR-069 (этап F, перенос из canvas-render::text в ядро):
/// план заголовка блока-ведомости — `Some((первая расчётная строка, число
/// расчётных))`, когда строк данных (параметры + расчёт — все строки с
/// исходами; авто-строки не входят по построению) больше порога T
/// (Q2, дефолт [`crate::settings::NODE_BODY_BLOCK_THRESHOLD`]) и среди них
/// есть расчётные. Чистая функция — рендер и измерение (canvas-render) и
/// консервативная оценка высоты (canvas-scene, уровень 1 refit) считают
/// ОДИН план: оценка не занижает высоту на ряд заголовка (I-2).
pub fn block_header_plan(lines: &[&str], formula_lines: &[usize]) -> Option<(usize, usize)> {
    let calc: Vec<usize> = formula_lines
        .iter()
        .copied()
        .filter(|&i| {
            lines
                .get(i)
                .is_some_and(|line| !matches!(line_kind(line), NumiLineKind::Assignment { .. }))
        })
        .collect();
    if formula_lines.len() > crate::settings::NODE_BODY_BLOCK_THRESHOLD && !calc.is_empty() {
        Some((*calc.first()?, calc.len()))
    } else {
        None
    }
}

/// FR-069 (этап F, шаг 2 плана владельца): строковый диапазон первого
/// проза-абзаца — `[start, end)` по индексам `\n`-строк исходного текста.
/// Общий источник сцены (оценка уровня 1) и рендера (супрессия дубликата
/// зоны описания): измерение и рендер супрессируют один и тот же диапазон
/// (I-2), индексы прочих строк не сдвигаются (I-1/I-3 — строки никуда не
/// исчезают из текста, только из верстки тела). Границы прогонов и вердикт
/// «весь прогон — проза» — в точности как в [`first_prose_paragraph`]:
/// прогон обрывается пустой строкой или фенсом, прогон с любой
/// не-прозой (числа/присваивания/выражения) не qualifies, сканирование
/// продолжается до конца текста.
pub fn first_prose_paragraph_span(text: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let flush = |start: Option<usize>, end: usize| -> Option<(usize, usize)> {
        let start = start?;
        let all_prose = lines[start..end]
            .iter()
            .all(|line| line_kind(line) == NumiLineKind::Prose);
        if all_prose {
            Some((start, end))
        } else {
            None
        }
    };
    let mut run_start: Option<usize> = None;
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            // Фенс открывает/закрывает прогон — код описанием не является
            if let Some(found) = flush(run_start, i) {
                return Some(found);
            }
            run_start = None;
            continue;
        }
        if in_fence {
            continue;
        }
        if line.trim().is_empty() {
            if let Some(found) = flush(run_start, i) {
                return Some(found);
            }
            run_start = None;
            continue;
        }
        if run_start.is_none() {
            run_start = Some(i);
        }
    }
    flush(run_start, lines.len())
}

/// FR-061 D-8/Q3 (решение владельца 2026-09-23, «desc→манифест→проза»):
/// первый проза-абзац текста ноды — фолбэк источника описания (после
/// `canvasdesk.desc` и описания манифеста шаблона). Абзац = подряд идущие
/// непустые строки; qualifies, если КАЖДАЯ его строка — проза по
/// [`line_kind`] (числа/присваивания/выражения и код-фенсы описанием не
/// становятся — иначе зона D-8 дублировала бы тело). Возвращает абзац,
/// склеенный одинарными пробелами. Чистая функция — сцена (резерв высоты,
/// CR-012) и рендер (кэш текста) вызывают её с одним входом `node.text`,
/// поэтому измерение и рендер не разъезжаются (I-2).
/// FR-069: композиция над [`first_prose_paragraph_span`] (байт-паритет
/// прежней формулировки — тестом); диапазон — для супрессии дубликата
/// абзаца в теле (этап F).
pub fn first_prose_paragraph(text: &str) -> Option<String> {
    let (start, end) = first_prose_paragraph_span(text)?;
    Some(
        text.lines()
            .skip(start)
            .take(end - start)
            .collect::<Vec<&str>>()
            .join(" ")
            .trim()
            .to_owned(),
    )
}

// --- Тесты (верификационный список FR-013 + регрессии грамматики) ---

#[cfg(test)]
mod tests {
    use super::*;

    /// `5 ms` → число с единицей миллисекунд.
    fn ms_unit() -> Unit {
        Unit::atom(unit_atom("ms").unwrap())
    }

    fn sec_unit() -> Unit {
        Unit::atom(unit_atom("sec").unwrap())
    }

    fn rps_unit() -> Unit {
        Unit::atom(unit_atom("rps").unwrap())
    }

    fn req_unit() -> Unit {
        Unit::atom(unit_atom("req").unwrap())
    }

    fn reqps_unit() -> Unit {
        Unit::atom(unit_atom("req/s").unwrap())
    }

    /// Верификация FR-013: `parse("5") == Ok(Expr::Num(5.0, Unit::Scalar))`.
    #[test]
    fn parse_scalar_literal() {
        assert_eq!(parse("5"), Ok(Expr::Num(5.0, Unit::Scalar)));
        #[allow(clippy::approx_constant)] // литерал пользователя, не PI
        let pi_like = 3.14;
        assert_eq!(parse("3.14"), Ok(Expr::Num(pi_like, Unit::Scalar)));
    }

    /// Верификация FR-013: композитная единица `ms·req/s` разбирается.
    #[test]
    fn parse_composite_unit() {
        let expected = Expr::Bin {
            op: BinOp::Mul,
            lhs: Box::new(Expr::Bin {
                op: BinOp::Mul,
                lhs: Box::new(Expr::Num(5.0, Unit::Scalar)),
                rhs: Box::new(Expr::Num(1.0, ms_unit())),
            }),
            rhs: Box::new(Expr::Bin {
                op: BinOp::Mul,
                lhs: Box::new(Expr::Num(200.0, Unit::Scalar)),
                rhs: Box::new(Expr::Num(1.0, reqps_unit())),
            }),
        };
        assert_eq!(parse("5 ms × 200 req/s"), Ok(expected));
    }

    /// Верификация FR-013: `1 sec + 500 ms == 1.5 sec` (конвертация вправо
    /// к единице левого операнда).
    #[test]
    fn eval_time_conversion_add() {
        let value = eval(&parse("1 sec + 500 ms").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(1.5, sec_unit()));
    }

    /// Обратный порядок: `500 ms + 1 sec == 1500 ms` (единица левого).
    #[test]
    fn eval_time_conversion_add_reversed() {
        let value = eval(&parse("500 ms + 1 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(1500.0, ms_unit()));
    }

    /// Верификация FR-013: `avg(10 ms, 20 ms, 30 ms) == 20 ms`.
    #[test]
    fn eval_avg_ms() {
        let value = eval(&parse("avg(10 ms, 20 ms, 30 ms)").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(20.0, ms_unit()));
    }

    /// Верификация FR-013: `1k rps == 1000 rps` (суффиксы порядка).
    #[test]
    fn eval_kilo_suffix() {
        let value = eval(&parse("1k rps").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(1000.0, rps_unit()));
        let value = eval(&parse("2M").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::scalar(2_000_000.0));
    }

    /// Верификация FR-013: `= invalid @#$` — ParseError с диагностикой.
    #[test]
    fn parse_invalid_input_err() {
        let err = parse("= invalid @#$").expect_err("некорректная формула");
        assert!(!err.msg.is_empty());
        // Позиция указывает на проблемное место (байтовый offset)
        let err = parse("@ 5").expect_err("@ не поддержан");
        assert_eq!(err.pos, 0);
    }

    /// Верификация FR-013: `5 ms + 3 rps` — Err(UnitMismatch).
    #[test]
    fn eval_unit_mismatch() {
        let parsed = parse("5 ms + 3 rps").expect("синтаксически корректно");
        let err = eval(&parsed, &Env::empty()).expect_err("разные размерности");
        assert_eq!(
            err,
            EvalError::UnitMismatch {
                lhs: "5 ms".to_owned(),
                rhs: "3 rps".to_owned()
            }
        );
    }

    /// Переменные: результат программы — значение последнего утверждения.
    #[test]
    fn eval_variables_program() {
        let program = "rps = 1000\nlatency = 50 ms\ncpu = latency × rps / 3";
        let value = eval(&parse(program).unwrap(), &Env::empty()).unwrap();
        // 50 ms × 1000 / 3 = 16666.6… ms
        assert_eq!(value.unit, ms_unit());
        assert!((value.num - 16666.666666666668).abs() < 1e-6);
    }

    /// Пример владельца: `3 replicas` — неявное умножение на переменную;
    /// `rps / replicas` даёт Time·Rate (ms·rps).
    #[test]
    fn eval_implicit_var_mul() {
        let program = "replicas = 3\n0.5 ms × 1000 rps / replicas";
        let value = eval(&parse(program).unwrap(), &Env::empty()).unwrap();
        assert_eq!(
            value.unit,
            Unit {
                atoms: vec![unit_atom("ms").unwrap(), unit_atom("rps").unwrap(),]
            }
        );
        assert!((value.num - 166.66666666666666).abs() < 1e-6);
    }

    /// Окружение: переменные из Env видны, локальные переопределяют.
    #[test]
    fn eval_env_seeding() {
        let env = Env::empty().set("base", Value::with_unit(10.0, ms_unit()));
        let value = eval(&parse("base + 5 ms").unwrap(), &env).unwrap();
        assert_eq!(value, Value::with_unit(15.0, ms_unit()));
    }

    /// Деление с нормализацией: `100 req / 2 sec == 50 req/s`.
    #[test]
    fn eval_div_normalizes_rate() {
        let value = eval(&parse("100 req / 2 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(50.0, reqps_unit()));
    }

    /// Сокращение одинаковых размерностей: `5 ms / 2 sec` — символическое
    /// отношение (число 2.5, единица ms/s — физически 0.0025).
    #[test]
    fn eval_div_symbolic() {
        let value = eval(&parse("10 req / 4 req").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::scalar(2.5));
    }

    /// Байты двоичные: `1 GB == 1024 MB`.
    #[test]
    fn eval_bytes_binary() {
        let gb = eval(&parse("1 GB").unwrap(), &Env::empty()).unwrap();
        let mb = eval(&parse("1024 MB").unwrap(), &Env::empty()).unwrap();
        assert_eq!(gb.dims(), mb.dims());
        assert!((gb.num * gb.unit.scale() - mb.num * mb.unit.scale()).abs() < 1e-6);
        // И сложение с конвертацией: 1 GB + 1 MB — единица левого (GB)
        let value = eval(&parse("1 GB + 1 MB").unwrap(), &Env::empty()).unwrap();
        assert!((value.num - (1.0 + 1.0 / 1024.0)).abs() < 1e-9);
    }

    /// Проценты: `50% + 10% == 60%`; скобки и унарный минус; единица
    /// после скобки (`(5 - 10) ms`) — литерал единицы, не переменная.
    #[test]
    fn eval_percent_and_unary() {
        let value = eval(&parse("50% + 10%").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.num, 60.0);
        assert_eq!(value.unit, Unit::atom(unit_atom("%").unwrap()));
        let value = eval(&parse("(5 - 10) ms").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::with_unit(-5.0, ms_unit()));
    }

    /// Функции: sum/max/min/percentile.
    #[test]
    fn eval_functions() {
        let sum = eval(&parse("sum(1, 2, 3.5)").unwrap(), &Env::empty()).unwrap();
        assert_eq!(sum, Value::scalar(6.5));
        let max = eval(&parse("max(10 ms, 30 ms, 20 ms)").unwrap(), &Env::empty()).unwrap();
        assert_eq!(max, Value::with_unit(30.0, ms_unit()));
        let min = eval(&parse("min(10 ms, 30 ms)").unwrap(), &Env::empty()).unwrap();
        assert_eq!(min, Value::with_unit(10.0, ms_unit()));
        // Линейная интерполяция: p90 из [10, 20] → 19
        let p90 = eval(
            &parse("percentile(90, 10 ms, 20 ms)").unwrap(),
            &Env::empty(),
        )
        .unwrap();
        assert_eq!(p90, Value::with_unit(19.0, ms_unit()));
    }

    /// Ошибки вызовов: неизвестная функция/переменная, разные размерности
    /// аргументов, деление на ноль, p вне диапазона.
    #[test]
    fn eval_error_paths() {
        assert!(matches!(
            eval(&parse("nope(1)").unwrap(), &Env::empty()),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(matches!(
            eval(&parse("missing + 1").unwrap(), &Env::empty()),
            Err(EvalError::UnknownVariable(_))
        ));
        assert!(matches!(
            eval(&parse("sum(1 ms, 2 rps)").unwrap(), &Env::empty()),
            Err(EvalError::BadCall { .. })
        ));
        assert_eq!(
            eval(&parse("5 / 0").unwrap(), &Env::empty()),
            Err(EvalError::DivisionByZero)
        );
        assert!(matches!(
            eval(&parse("percentile(120, 1, 2)").unwrap(), &Env::empty()),
            Err(EvalError::BadCall { .. })
        ));
    }

    /// Разбор ошибок: незакрытая скобка, неожиданный оператор, пустая формула.
    #[test]
    fn parse_error_paths() {
        assert!(parse("(1 + 2").is_err());
        assert!(parse("* 3").is_err());
        assert!(parse("").is_err());
        assert!(parse("   \n  ").is_err());
        assert!(parse("5 +").is_err());
        // `rps = ` без значения
        assert!(parse("rps = ").is_err());
    }

    /// Утверждения через `;` и пустые строки.
    #[test]
    fn parse_statement_separators() {
        let value = eval(&parse("a = 2; b = 3\n\na × b").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::scalar(6.0));
    }

    /// Отображение значений: `1000 ms·req/s`, `1.5 sec`, скаляр, 6 значащих.
    #[test]
    fn display_formatting() {
        let value = eval(&parse("5 ms × 200 req/s").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "1000 ms·req/s");
        assert_eq!(Value::with_unit(1.5, sec_unit()).to_string(), "1.5 sec");
        assert_eq!(Value::scalar(1000.0).to_string(), "1000");
        assert_eq!(Value::scalar(166.66666666).to_string(), "166.667");
        assert_eq!(Value::scalar(0.30000000000000004).to_string(), "0.3");
        let rate = eval(&parse("100 req / 2 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "50 req/s");
    }

    /// D-1 (FR-061, T1-oracle): структурные части значения — число и юнит
    /// раздельно; форматирование числа идентично `Display` (та же
    /// `format_num`); скаляр → юнит пуст; NaN → «не число».
    #[test]
    fn display_parts_oracle() {
        let (num, unit) = Value::with_unit(800.0, rps_unit()).display_parts();
        assert_eq!(num, "800");
        assert_eq!(unit, "rps");
        // Составная единица — как в Display («1000 ms·req/s»)
        let composite = eval(&parse("5 ms × 200 req/s").unwrap(), &Env::empty()).unwrap();
        let (num, unit) = composite.display_parts();
        assert_eq!(num, "1000");
        assert_eq!(unit, "ms·req/s");
        // Скаляр — юнит пуст
        assert_eq!(
            Value::scalar(20.0).display_parts(),
            ("20".to_owned(), String::new())
        );
        // Дробное — те же 6 значащих цифр, что в Display
        assert_eq!(
            Value::scalar(166.66666666).display_parts(),
            ("166.667".to_owned(), String::new())
        );
        assert_eq!(
            Value::scalar(f64::NAN).display_parts(),
            ("не число".to_owned(), String::new())
        );
    }

    /// D-1 (FR-061): инвариант байт-паритета — `Display` значения
    /// восстанавливается из частей `display_parts` через единственную
    /// сборку `join_parts` на любом значении корпуса (MCP-тексты, которые
    /// остаются на `Display`, не меняются ни на байт).
    #[test]
    fn display_parity_property() {
        let corpus: Vec<Value> = [
            "800 rps",
            "5 ms × 200 req/s",
            "1.5 sec",
            "20",
            "166.66666666",
            "0.30000000000000004",
            "1 req / 10 ms",
            "100 req / 2 sec",
            "0.5 h",
        ]
        .iter()
        .map(|src| {
            eval(&parse(src).unwrap(), &Env::empty())
                .unwrap_or_else(|err| panic!("корпус: {src:?} → {err:?}"))
        })
        .collect();
        for value in &corpus {
            let (num, unit) = value.display_parts();
            assert_eq!(value.to_string(), join_parts(&num, &unit));
        }
    }

    /// D-1 (FR-061): структурные части what-if дельты — base/new как
    /// части значения + строка дельты; строковый `whatif_full_delta` —
    /// композиция над частями (байт-паритет), предикат None совпадает.
    #[test]
    fn whatif_delta_parts_oracle() {
        let base = Value::with_unit(1000.0, rps_unit());
        let whatif = Value::with_unit(1200.0, rps_unit());
        let p = whatif_delta_parts(&base, &whatif).unwrap();
        assert_eq!(p.base, ("1000".to_owned(), "rps".to_owned()));
        assert_eq!(p.new, ("1200".to_owned(), "rps".to_owned()));
        assert_eq!(p.delta, "+200 rps");
        assert_eq!(
            whatif_full_delta(&base, &whatif).unwrap(),
            "1000 rps → 1200 rps (+200 rps)"
        );
        // Процентная ветка «пп» — спец-логика знака не тронута
        let base = Value::with_unit(50.0, unit_atom("%").map(Unit::atom).unwrap());
        let whatif = Value::with_unit(53.0, unit_atom("%").map(Unit::atom).unwrap());
        let p = whatif_delta_parts(&base, &whatif).unwrap();
        assert_eq!(p.delta, "+3 пп");
        assert_eq!(p.new, ("53".to_owned(), "%".to_owned()));
        // Нет дельты — None у обеих форм (согласованность предиката)
        assert_eq!(whatif_delta_parts(&base, &base), None);
        assert_eq!(whatif_full_delta(&base, &base), None);
    }

    /// FR-018: деление/умножение с ненормализованным временем — num в
    /// масштабе итоговой единицы (`1 req / 10 ms` = 100 req/s, а не 0.1);
    /// стык с шаблонными формулами `mm1($qps, 1 req / $t, …)`.
    #[test]
    fn division_rescales_normalized_rate() {
        let rate = eval(&parse("1 req / 10 ms").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "100 req/s");
        // База через rate-путь queueing: utilisation(80 rps, 1 req/10 ms) = 0.8
        let util = eval(
            &parse("utilization(80 rps, 1 req / 10 ms)").unwrap(),
            &Env::empty(),
        )
        .unwrap();
        assert!((util.num - 0.8).abs() < 1e-9, "utilization = {}", util);
        // Деление в базовых единицах — прежний путь без пересчёта
        let rate = eval(&parse("1000 req / 4 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "250 req/s");
    }

    /// Единицы после числа только в позиции единиц: переменная может
    /// называться как единица (`rps = 1000` — пример владельца).
    #[test]
    fn unit_names_are_valid_variables() {
        let value = eval(&parse("rps = 1000\nrps × 2").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::scalar(2000.0));
    }

    /// Правка владельца (2026-09-23): словоизменительные дубликаты убраны
    /// из таблицы — один наглядный токен на значение (`sec`, `req`, `h`).
    #[test]
    fn unit_table_canonical_no_inflections() {
        assert!(unit_atom("s").is_none(), "`s` заменён каноном `sec`");
        assert!(unit_atom("secs").is_none(), "`secs` заменён каноном `sec`");
        assert!(unit_atom("reqs").is_none(), "`reqs` заменён каноном `req`");
        assert!(unit_atom("hour").is_none(), "`hour` заменён каноном `h`");
        assert!(unit_atom("sec").is_some());
        assert!(unit_atom("req").is_some());
        // Убранный токен после числа — уже не единица: `300 reqs` —
        // умножение на неизвестную переменную (диагностика вместо ответа)
        let parsed = parse("300 reqs").expect("лексер: число + идентификатор");
        let err = eval(&parsed, &Env::empty()).unwrap_err();
        assert!(err.to_string().contains("неизвестная переменная"));
    }

    /// Кириллические единицы (v2, правка 2026-09-23): парсинг после числа,
    /// конвертация при сложении, деление с синтезом Rate, отображение —
    /// токен, которым единица введена.
    #[test]
    fn cyrillic_units_parse_eval_display() {
        // Сложение с конвертацией вправо; единица левого операнда
        let value = eval(&parse("1 сек + 500 мс").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "1.5 сек");
        let value = eval(&parse("500 мс + 1 сек").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "1500 мс");
        // Смешанный ввод (латиница + кириллица): ответ — в единице левого
        let value = eval(&parse("1 sec + 500 мс").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "1.5 sec");
        let value = eval(&parse("1 сек + 1 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "2 сек");
        // Деление Count/Time → Rate на языке Count-операнда (`запр/с`)
        let rate = eval(&parse("100 запр / 2 сек").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "50 запр/с");
        // Слитный Rate-токен: max-munch `запр/с` раньше `запр`
        let rate = eval(&parse("50 запр/с").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "50 запр/с");
        // Байты, часы, гигабайты с конвертацией
        let value = eval(&parse("2 КБ").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "2 КБ");
        let value = eval(&parse("2 ч").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "2 ч");
        let value = eval(&parse("1 ГБ + 512 МБ").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "1.5 ГБ");
        // Равенство единиц не зависит от алфавита синонима
        assert_eq!(
            eval(&parse("1 сек").unwrap(), &Env::empty()).unwrap(),
            eval(&parse("1 sec").unwrap(), &Env::empty()).unwrap()
        );
    }

    /// Латинская канонизация не сломана: синтез Rate по-прежнему `req/s`.
    #[test]
    fn latin_rate_synthesis_unchanged() {
        let rate = eval(&parse("100 req / 2 sec").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "50 req/s");
        let rate = eval(&parse("1 req / 10 ms").unwrap(), &Env::empty()).unwrap();
        assert_eq!(rate.to_string(), "100 req/s");
    }

    /// Ввод-вывод без паники на кириллице и незнакомых символах.
    #[test]
    /// FR-013 (правка 3): буквы Unicode — идентификаторы (`5 μs` — ссылка
    /// на переменную), диагностика лексера — для небуквенных символов.
    fn lexer_unicode_diagnostics() {
        let err = parse("5 €").expect_err("€ — не буква, не поддержан");
        assert!(err.msg.contains("неподдерживаемый символ"));
        let parsed = parse("5 μs").expect("μ — буква Unicode: идентификатор");
        let err = eval(&parsed, &Env::empty()).expect_err("переменной μs нет");
        assert!(err.to_string().contains("неизвестная переменная"));
    }

    /// Текст результата строки сценария (для кратких проверок).
    fn ok_text(outcome: &Option<ExprOutcome>) -> String {
        match outcome {
            Some(ExprOutcome::Ok(value)) => value.to_string(),
            other => panic!("ожидался результат, получено: {other:?}"),
        }
    }

    /// Построчное вычисление (Numi-стиль): присваивания возвращают
    /// значение и связывают переменные, доступные нижним строкам;
    /// проза и пустые строки — без результата.
    #[test]
    fn eval_lines_numi_sheet() {
        let lines = eval_lines("Gateway\nrps = 1000\n\nlatency = 50 ms\nlatency × rps");
        assert_eq!(lines.len(), 5, "Vec выровнен по строкам текста");
        assert_eq!(lines[0], None, "проза — не формула");
        assert_eq!(
            ok_text(&lines[1]),
            "1000",
            "присваивание возвращает присвоенное значение (скаляр)"
        );
        assert_eq!(lines[2], None, "пустая строка");
        assert_eq!(ok_text(&lines[3]), "50 ms");
        assert_eq!(
            ok_text(&lines[4]),
            "50000 ms",
            "переменные протекают между строками"
        );
    }

    /// Явная («=») строка показывает любые ошибки парсинга/вычисления.
    /// Авто-строка показывает «заслуженные» ошибки вычисления (правка 4:
    /// сломанный расчёт — не проза), но молчит при ошибках парсинга.
    #[test]
    fn eval_lines_explicit_errors_visible() {
        let lines = eval_lines("= 5 ms +\n5 ms +\n= 5 ms + 3 rps\n5 ms + 3 rps\n2+2");
        assert_eq!(lines.len(), 5);
        assert!(
            matches!(&lines[0], Some(ExprOutcome::Err(_))),
            "явная: ошибка парсинга видна"
        );
        assert_eq!(lines[1], None, "авто: ошибка парсинга тиха");
        assert!(
            matches!(&lines[2], Some(ExprOutcome::Err(_))),
            "явная: ошибка вычисления видна"
        );
        assert!(
            matches!(&lines[3], Some(ExprOutcome::Err(_))),
            "авто: несовместимость единиц видна (правка 4)"
        );
        assert_eq!(ok_text(&lines[4]), "4");
    }

    /// FR-013 (правка 4): правила видимости ошибок авто-строк.
    /// Несовместимость единиц/деление на ноль — видны; ссылка на переменную,
    /// объявленную выше (даже ошибочной строкой), — видна; неизвестное слово
    /// вне объявлений и проза — молчат.
    #[test]
    fn auto_error_visibility_rules() {
        // Ссылка на переменную, чьё присваивание выше не вычислилось:
        // обе строки показывают ошибку (объявление x видно ссылке)
        let lines = eval_lines("x = 1 sec + 2 req\nx + 1");
        assert!(
            matches!(&lines[0], Some(ExprOutcome::Err(_))),
            "несовместимость единиц в присваивании видна"
        );
        assert!(
            matches!(&lines[1], Some(ExprOutcome::Err(_))),
            "ссылка на объявленную, но не вычисленную переменную видна"
        );
        // Незакрытая скобка в присваивании: строка похожа на присваивание —
        // ошибка парсинга видна; ссылка ниже тоже
        let lines = eval_lines("x = (2+\n200 + x");
        assert!(
            matches!(&lines[0], Some(ExprOutcome::Err(_))),
            "присваивание с ошибкой парсинга показывает её"
        );
        assert!(
            matches!(&lines[1], Some(ExprOutcome::Err(_))),
            "ссылка на объявленную переменную видна"
        );
        // Деление на ноль в авто-строке видно
        let lines = eval_lines("a = 5\na / (2 - 2)");
        assert_eq!(ok_text(&lines[0]), "5");
        assert!(
            matches!(&lines[1], Some(ExprOutcome::Err(_))),
            "деление на ноль в авто-строке видно"
        );
        // Неизвестное слово вне объявлений молчит (проза/список с числом)
        let lines = eval_lines("- 5 яблок\n2 + несуществующая");
        assert_eq!(lines[0], None, "слово не объявлено — тихо");
        assert_eq!(lines[1], None, "неизвестное имя вне объявлений — тихо");
        // Проза с двоеточием не краснеет
        let lines = eval_lines("Встреча в 15:00");
        assert_eq!(lines[0], None, "проза тиха");
    }

    /// Внутри код-фенсов формулы не вычисляются; после закрытия фенса
    /// сценарий продолжается.
    #[test]
    fn eval_lines_skips_code_fences() {
        let lines = eval_lines("```\nx = 5\n```\n2+2");
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], None, "открывающий фенс");
        assert_eq!(lines[1], None, "код — не калькулятор");
        assert_eq!(lines[2], None, "закрывающий фенс");
        assert_eq!(ok_text(&lines[3]), "4");
    }

    /// Авто-строки: выражения с единицами и суффиксами считаются построчно.
    #[test]
    fn eval_lines_auto_expressions() {
        let lines = eval_lines("1000 rps * 2\n5 ms × 200 req/s\n2k");
        assert_eq!(ok_text(&lines[0]), "2000 rps");
        assert_eq!(ok_text(&lines[1]), "1000 ms·req/s");
        assert_eq!(ok_text(&lines[2]), "2000");
    }

    /// FR-013 (правка 3, Numi): хвостовое присваивание `выражение = имя`
    /// связывает имя с результатом; форма «в обе стороны»
    /// (`b = 235 + 2323 = b`) не ломает строку.
    #[test]
    fn trailing_assignment_saves_result() {
        // Парсер: утверждение — Assign с именем-целью
        match parse("123 + 5123 = a").expect("хвостовое присваивание") {
            Expr::Assign { name, .. } => assert_eq!(name, "a"),
            other => panic!("ожидалось присваивание, получено: {other:?}"),
        }
        let lines = eval_lines("123 + 5123 = a\n235 + 2323 = b\nc = a + b");
        assert_eq!(ok_text(&lines[0]), "5246", "результат строки — значение");
        assert_eq!(ok_text(&lines[1]), "2558");
        assert_eq!(ok_text(&lines[2]), "7804", "a и b видны ниже");
        // «В обе стороны»: то же имя в конце игнорируется
        let lines = eval_lines("b = 235 + 2323 = b");
        assert_eq!(ok_text(&lines[0]), "2558");
        // Имя после «=» обязано завершать утверждение; не-имя — тихая ошибка
        assert_eq!(eval_lines("2 = 3")[0], None);
        assert_eq!(eval_lines("2 = 3 + 1")[0], None);
    }

    /// FR-013 (правка 3, Numi): `x`/`х` между операндами — умножение
    /// (латиница и кириллица, после числа/единицы/скобки/переменной);
    /// в конце строки, после оператора и в начале — переменная.
    #[test]
    fn x_between_operands_is_multiplication() {
        let lines = eval_lines("25 + 35 x 20\n25 + 35 х 20\n(2+3) x 4\n$5 x 3\n5 ms x 3");
        assert_eq!(ok_text(&lines[0]), "725", "латинская x");
        assert_eq!(ok_text(&lines[1]), "725", "кириллическая х");
        assert_eq!(ok_text(&lines[2]), "20", "после скобки");
        assert_eq!(ok_text(&lines[3]), "15 $", "после валюты");
        assert_eq!(ok_text(&lines[4]), "15 ms", "после единицы");
        // Ссылки на переменную x сохраняются
        let lines = eval_lines("x = 200\n200 + x\nx 20\n35 x");
        assert_eq!(ok_text(&lines[0]), "200");
        assert_eq!(ok_text(&lines[1]), "400", "x в конце строки — переменная");
        assert_eq!(ok_text(&lines[2]), "4000", "x в начале строки — переменная");
        assert_eq!(ok_text(&lines[3]), "7000", "x в конце строки — переменная");
        // После переменной: перед числом — умножение, перед именем — переменная
        let lines = eval_lines("latency = 50 ms\nlatency х 3\nlatency х y");
        assert_eq!(ok_text(&lines[1]), "150 ms");
        assert!(lines[2].is_none(), "y не задан — строка тиха");
        // СЛИТНОЕ умножение (правка 5): `число x число` без пробелов —
        // лист владельца `a=25+35x20`; `x20` как имя в этой позиции
        // недостижимо
        let lines = eval_lines("25+35x20\n25+35х20\n2x3\n(2)x3\n$5x3\n5msx2");
        assert_eq!(ok_text(&lines[0]), "725", "слитно латинская x");
        assert_eq!(ok_text(&lines[1]), "725", "слитно кириллическая х");
        assert_eq!(ok_text(&lines[2]), "6", "2x3");
        assert_eq!(ok_text(&lines[3]), "6", "слитно после скобки");
        assert_eq!(ok_text(&lines[4]), "15 $", "слитно после валюты");
        assert_eq!(ok_text(&lines[5]), "10 ms", "слитно после единицы");
    }

    /// FR-013 (правка 3): имена переменных — буквы Unicode (кириллица):
    /// смешанная раскладка клавиатуры не ломает лист расчёта.
    #[test]
    fn cyrillic_variable_names() {
        let lines = eval_lines("х = 200\nа = 123 + 5123\nс = а + х\n200 + х");
        assert_eq!(ok_text(&lines[0]), "200");
        assert_eq!(ok_text(&lines[1]), "5246");
        assert_eq!(ok_text(&lines[2]), "5446");
        assert_eq!(ok_text(&lines[3]), "400");
    }

    /// Лист владельца (обратная связь по правке 2): все строки сценария
    /// выдают результат — присваивания в обе стороны, переменные,
    /// x-умножение, `*`.
    #[test]
    fn eval_lines_owner_sheet() {
        let lines = eval_lines(
            "123 + 5123 = a\n\
             235 + 2323 = b\n\
             \n\
             x = 200\n\
             a = 123 + 5123\n\
             b = 235 + 2323 = b\n\
             \n\
             c = a + b\n\
             \n\
             200 + x\n\
             25 + 35 x 20\n\
             123 + 23 * 5",
        );
        assert_eq!(lines.len(), 12, "Vec выровнен по строкам текста");
        assert_eq!(ok_text(&lines[0]), "5246");
        assert_eq!(ok_text(&lines[1]), "2558");
        assert_eq!(lines[2], None, "пустая строка");
        assert_eq!(ok_text(&lines[3]), "200");
        assert_eq!(ok_text(&lines[4]), "5246");
        assert_eq!(ok_text(&lines[5]), "2558", "форма «в обе стороны»");
        assert_eq!(lines[6], None, "пустая строка");
        assert_eq!(ok_text(&lines[7]), "7804");
        assert_eq!(lines[8], None, "пустая строка");
        assert_eq!(ok_text(&lines[9]), "400");
        assert_eq!(ok_text(&lines[10]), "725");
        assert_eq!(ok_text(&lines[11]), "238");
    }

    /// FR-013 (правка 5): заметки прежних сборок содержат экранированное
    /// каноникой `=` (`x \= 200`) — расчёт снимает экранирование и лист
    /// оживет без пересохранения. Регресс корневого бага «ничего не
    /// показывают»: emit экранировал КАЖДОЕ `=`, expr-парсер падал на `\`.
    #[test]
    fn eval_lines_escaped_equals_from_canonical_text() {
        // Лист 1 владельца в канонической записи прежних сборок
        let lines = eval_lines("х \\= 200\nс \\= а + б\n200 + х");
        assert_eq!(ok_text(&lines[0]), "200", "присваивание с \\= считается");
        assert_eq!(ok_text(&lines[2]), "400", "ссылка на объявленную x");
        // Полный лист во «взрослой» форме
        let lines = eval_lines("123 + 5123 \\= a\n235 + 2323 \\= b\nc \\= a + b");
        assert_eq!(ok_text(&lines[0]), "5246");
        assert_eq!(ok_text(&lines[1]), "2558");
        assert_eq!(ok_text(&lines[2]), "7804");
    }

    /// FR-013 (правка 5): ссылка на НЕОБЪЯВЛЕННУЮ переменную видна, если
    /// строка сама — присваивание (`c = a + b`): это расчёт, а не проза;
    /// требование владельца «ошибки тоже не выводятся». Проза (`- 5
    /// яблок`, `2 + слово`) молчит, как и раньше.
    #[test]
    fn eval_lines_unknown_variable_visible_on_assignment() {
        let lines = eval_lines("x = 200\nc = a + b\n200 + x");
        assert_eq!(ok_text(&lines[0]), "200");
        match &lines[1] {
            Some(ExprOutcome::Err(msg)) => {
                assert!(
                    msg.contains('a'),
                    "ошибка называет отсутствующее имя: {msg}"
                )
            }
            other => panic!("c = a + b без a — видимая ошибка: {other:?}"),
        }
        assert_eq!(ok_text(&lines[2]), "400");
        // Проза и не-присваивания с неизвестными словами молчат
        let lines = eval_lines("- 5 яблок\n2 + несуществующая");
        assert_eq!(lines[0], None);
        assert_eq!(lines[1], None);
    }

    // --- FR-014: входящие значения value-рёбер ($in, $1..$N) ---

    /// Верификация FR-014: `$in × 2` со входом 5 → 10.
    #[test]
    fn eval_inbound_single() {
        let env = Env::with_inbound(vec![Some(Value::scalar(5.0))]);
        let value = eval(&parse("$in × 2").unwrap(), &env).unwrap();
        assert_eq!(value, Value::scalar(10.0));
    }

    /// Верификация FR-014: `$1 + $2` со входами 3 и 7 → 10.
    #[test]
    fn eval_inbound_indexed() {
        let env = Env::with_inbound(vec![Some(Value::scalar(3.0)), Some(Value::scalar(7.0))]);
        let value = eval(&parse("$1 + $2").unwrap(), &env).unwrap();
        assert_eq!(value, Value::scalar(10.0));
    }

    /// Входы с единицами: `$in × 2 ms` — умножение req×ms как обычно.
    #[test]
    fn eval_inbound_with_units() {
        let env = Env::with_inbound(vec![Some(Value::with_unit(100.0, req_unit()))]);
        let value = eval(&parse("$in × 2 ms").unwrap(), &env).unwrap();
        assert_eq!(value.to_string(), "200 req·ms");
    }

    /// `$in` без входов — Err(MissingInbound); `$2` при одном входе — тоже.
    #[test]
    fn eval_inbound_missing() {
        let err = eval(&parse("$in").unwrap(), &Env::empty()).unwrap_err();
        assert_eq!(err, EvalError::MissingInbound { index: 0 });
        let env = Env::with_inbound(vec![Some(Value::scalar(5.0))]);
        let err = eval(&parse("$2").unwrap(), &env).unwrap_err();
        assert_eq!(err, EvalError::MissingInbound { index: 1 });
    }

    /// `$in` при двух входах — Err(AmbiguousInbound): нужен явный `$1`/`$2`.
    #[test]
    fn eval_inbound_ambiguous() {
        let env = Env::with_inbound(vec![Some(Value::scalar(3.0)), Some(Value::scalar(7.0))]);
        let err = eval(&parse("$in + 1").unwrap(), &env).unwrap_err();
        assert_eq!(err, EvalError::AmbiguousInbound { count: 2 });
        // Индексные ссылки при этом работают
        let value = eval(&parse("$1 + $2").unwrap(), &env).unwrap();
        assert_eq!(value, Value::scalar(10.0));
    }

    /// Слот «ребро есть, значения нет» — MissingInbound (источник без
    /// формулы или с ошибкой).
    #[test]
    fn eval_inbound_slot_none_is_missing() {
        let env = Env::with_inbound(vec![None]);
        let err = eval(&parse("$in + 1").unwrap(), &env).unwrap_err();
        assert_eq!(err, EvalError::MissingInbound { index: 0 });
    }

    /// Регрессия FR-013: `$5` без входов — валюта (5 $).
    #[test]
    fn eval_dollar_amount_is_currency_without_inbound() {
        let value = eval(&parse("$5 × 3").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value.to_string(), "15 $");
    }

    /// `$5` при наличии входов — ссылка на 5-й вход (контекстное
    /// разрешение FR-014); `$2.5` и `$0` — всегда валюта.
    #[test]
    fn eval_dollar_amount_is_inbound_reference_with_inbound() {
        let env = Env::with_inbound(vec![
            Some(Value::scalar(1.0)),
            Some(Value::scalar(2.0)),
            Some(Value::scalar(3.0)),
            Some(Value::scalar(4.0)),
            Some(Value::with_unit(10.0, rps_unit())),
        ]);
        let value = eval(&parse("$5").unwrap(), &env).unwrap();
        assert_eq!(value.to_string(), "10 rps");
        // Дробные суммы и ноль — валюта даже при наличии входов
        let value = eval(&parse("$2.5").unwrap(), &env).unwrap();
        assert_eq!(value.to_string(), "2.5 $");
        let value = eval(&parse("$0").unwrap(), &env).unwrap();
        assert_eq!(value.to_string(), "0 $");
    }

    /// FR-014: строки листа видят входы ноды (`eval_lines_in`).
    #[test]
    fn eval_lines_see_inbound() {
        let env = Env::with_inbound(vec![Some(Value::with_unit(1000.0, rps_unit()))]);
        let lines = eval_lines_in("= $in / 4", &env);
        assert_eq!(ok_text(&lines[0]), "250 rps");
        // Без входов та же строка — видимая ошибка отсутствия входа
        let lines = eval_lines("= $in / 4");
        match &lines[0] {
            Some(ExprOutcome::Err(msg)) => assert!(msg.contains("вход"), "{msg}"),
            other => panic!("без входа — ошибка: {other:?}"),
        }
    }

    /// Входы не вытесняются присваиваниями листа и переживают блок.
    #[test]
    fn eval_inbound_survives_assignments() {
        let env = Env::with_inbound(vec![Some(Value::scalar(6.0))]);
        let program = "half = $in / 2\nhalf × 3";
        let value = eval(&parse(program).unwrap(), &env).unwrap();
        assert_eq!(value, Value::scalar(9.0));
    }

    // --- FR-021: каталог подсказок и детектор рода строки ---

    // --- FR-063: parity stats-домена (диспетчер ↔ каталог) ---

    /// FR-063 P1: множество stats-имён в `eval_call` (единая точка
    /// `stats::STATS_FUNCTIONS`) совпадает со множеством stats-записей в
    /// `FN_HINTS` (паттерн FR-021). P1: оба множества пусты — тест фиксирует
    /// контракт до наполнения (P2/P3 расширяют обе стороны синхронно).
    #[cfg(feature = "stats")]
    #[test]
    fn stats_fn_hints_parity_with_eval_call() {
        let hint_names: std::collections::BTreeSet<&str> =
            fn_hints().iter().map(|h| h.name).collect();
        let stats_names: std::collections::BTreeSet<&str> =
            super::stats::STATS_FUNCTIONS.iter().copied().collect();
        // Каждое stats-имя диспетчера подсказывается UI
        for name in &stats_names {
            assert!(
                hint_names.contains(name),
                "{name}: движок знает (stats-arm), каталог подсказок — нет"
            );
        }
        // Обратное направление: полный каталог = встроенные + queueing +
        // stats, без лишних записей (если тест упал — каталог и диспетчер
        // разошлись; обнови BUILTIN/hints/stats синхронно).
        const BUILTIN_AND_QUEUEING: &[&str] = &[
            "sum",
            "avg",
            "max",
            "min",
            "percentile",
            "utilization",
            "mm1",
            "mmc",
            "littles_law",
            "erlang_c",
            "npv",
            "cagr",
            "irr",
            "cohort_ltv",
        ];
        let expected: std::collections::BTreeSet<&str> = BUILTIN_AND_QUEUEING
            .iter()
            .copied()
            .chain(stats_names.iter().copied())
            .collect();
        assert_eq!(
            hint_names, expected,
            "FN_HINTS разошёлся с диспетчером eval_call"
        );
    }

    /// Инвариант 2 FR-021: каждая функция каталога известна грамматике
    /// (parse), а каждый токен единиц — таблице (unit_value).
    #[test]
    fn fn_hints_and_unit_tokens_sync_with_engine() {
        for hint in fn_hints() {
            assert!(
                parse(&format!("{}(1)", hint.name)).is_ok(),
                "{} не парсится — каталог разошёлся с движком",
                hint.name
            );
        }
        // FR-063: каждая подсказка — известная ДИСПЕТЧЕРУ функция (не только
        // грамматике): eval не даёт UnknownFunction (расширенный parity —
        // закрывает и обратное направление: «движок знает, UI не подсказывает»).
        for hint in fn_hints() {
            if let Err(err) = eval(&parse(&format!("{}(1)", hint.name)).unwrap(), &Env::empty()) {
                assert!(
                    !matches!(err, EvalError::UnknownFunction(_)),
                    "{} в каталоге, но диспетчер его не знает",
                    hint.name
                );
            }
        }
        let tokens = unit_tokens();
        assert!(tokens.contains(&"ms"));
        assert!(tokens.contains(&"sec"));
        assert!(tokens.contains(&"rps"));
        assert!(tokens.contains(&"KB"));
        assert!(tokens.contains(&"%"));
        // Кириллические синонимы в каталоге подсказок (правка 2026-09-23)
        assert!(tokens.contains(&"сек"));
        assert!(tokens.contains(&"запр"));
        assert!(tokens.contains(&"запр/с"));
        assert!(tokens.contains(&"ГБ"));
        for token in &tokens {
            let value = unit_value(1.0, Some(token));
            // Токен таблицы даёт атом-размерность (не скаляр)
            assert!(
                !value.unit.is_scalar(),
                "токен {token} из unit_tokens — скаляр"
            );
        }
    }

    /// Детектор рода строки = вердикт движка (инвариант 1 FR-021).
    #[test]
    fn line_kind_matches_eval_verdict() {
        assert_eq!(
            line_kind("rps = 1000 rps"),
            NumiLineKind::Assignment {
                name: "rps".to_owned()
            }
        );
        assert_eq!(line_kind("= 2 + 2"), NumiLineKind::Expression);
        assert_eq!(line_kind("2 + 2"), NumiLineKind::Expression);
        // Ошибочное присваивание — всё равно Numi (ошибка видна)
        assert!(matches!(
            line_kind("c = a + b"),
            NumiLineKind::Assignment { .. }
        ));
        // Явная ошибочная формула — Expression
        assert_eq!(line_kind("= 5 +"), NumiLineKind::Expression);
        // Проза
        assert_eq!(line_kind("встреча в 3"), NumiLineKind::Prose);
        assert_eq!(line_kind(""), NumiLineKind::Prose);
        assert_eq!(line_kind("   "), NumiLineKind::Prose);
        // Экранированный литеральный `=` — как в eval_lines
        assert!(matches!(
            line_kind("x \\= 200"),
            NumiLineKind::Assignment { .. }
        ));
        // Инвариант детектора: что движок ВЫЧИСЛИЛ (Some) — не проза;
        // что детектор считает прозой — движок молчит
        for line in [
            "rps = 1000 rps",
            "= 2 + 2",
            "2 + 2",
            "встреча в 3",
            "",
            "5 ms",
        ] {
            let produced = eval_lines(line).iter().any(Option::is_some);
            let kind = line_kind(line);
            if kind == NumiLineKind::Prose {
                assert!(!produced, "детектор: проза, но движок вычислил {line:?}");
            } else {
                let outcomes = eval_lines(line);
                let has_visible_error = outcomes
                    .iter()
                    .any(|outcome| matches!(outcome, Some(ExprOutcome::Err(_))));
                assert!(
                    produced
                        || has_visible_error
                        || line.trim_start().starts_with('=')
                        || line.trim().contains('='),
                    "детектор: Numi, но движок полностью молчит: {line:?}"
                );
            }
        }
    }

    // --- FR-050 Р-6: qualified-пути «Объект.Поле» (грамматика) ---

    /// Парсинг форм имени: «Объект.Поле», дефис внутри поля («Кол-во»),
    /// суффикс коллизии « (N)» / « (id)»; «Объект.Кол - во» (пробел) —
    /// вычитание; `x.5` — прежняя семантика (неявное умножение на дробь).
    #[test]
    fn parse_qualified_path_forms() {
        use super::{BinOp, Expr};
        // Базовая форма
        assert_eq!(
            parse("Заявки.Кол-во"),
            Ok(Expr::Qualified {
                obj: "Заявки".to_owned(),
                field: "Кол-во".to_owned()
            })
        );
        // Суффикс коллизии: номер и node_id-алиас, пробелы нормализуются
        assert_eq!(
            parse("Заявки (2).Кол"),
            Ok(Expr::Qualified {
                obj: "Заявки (2)".to_owned(),
                field: "Кол".to_owned()
            })
        );
        assert_eq!(
            parse("Заявки(a7).Кол"),
            Ok(Expr::Qualified {
                obj: "Заявки (a7)".to_owned(),
                field: "Кол".to_owned()
            })
        );
        // Пробел после точки-поля: вычитание Qualified - Var
        assert_eq!(
            parse("Заявки.Кол - во"),
            Ok(Expr::Bin {
                op: BinOp::Sub,
                lhs: Box::new(Expr::Qualified {
                    obj: "Заявки".to_owned(),
                    field: "Кол".to_owned()
                }),
                rhs: Box::new(Expr::Var("во".to_owned()))
            })
        );
        // Регресс: `x.5` — точка перед цифрой остаётся дробным числом
        assert_eq!(
            parse("x.5"),
            Ok(Expr::Bin {
                op: BinOp::Mul,
                lhs: Box::new(Expr::Var("x".to_owned())),
                rhs: Box::new(Expr::Num(0.5, super::Unit::Scalar))
            })
        );
        // Операнд неявного умножения: `2 Курсы.USD`
        assert_eq!(
            parse("2 Курсы.USD"),
            Ok(Expr::Bin {
                op: BinOp::Mul,
                lhs: Box::new(Expr::Num(2.0, super::Unit::Scalar)),
                rhs: Box::new(Expr::Qualified {
                    obj: "Курсы".to_owned(),
                    field: "USD".to_owned()
                })
            })
        );
        // Суффикс без точки — прежняя семантика (вызов функции)
        assert!(matches!(parse("sum (12)").unwrap(), Expr::Call { .. }));
        // Присваивание: LHS — имя, RHS — пути (пример инварианта 6)
        assert!(matches!(
            parse("выручка = Заявки.Кол-во * Заявки.Средний_чек").unwrap(),
            Expr::Assign { .. }
        ));
    }

    /// FR-050 Р-6: eval неразрешённого пути — видимая ошибка «вход не
    /// найден» с полным путём.
    #[test]
    fn eval_qualified_missing_is_error() {
        let parsed = parse("Заявки.Кол").expect("парсится");
        let err = eval(&parsed, &Env::empty()).expect_err("нет таблицы — ошибка");
        assert!(
            err.to_string().contains("вход не найден"),
            "текст ошибки: {err}"
        );
        assert!(
            err.to_string().contains("Заявки.Кол"),
            "ошибка называет путь: {err}"
        );
        // Разрешённый путь — значение из таблицы
        let mut qualified = std::collections::HashMap::new();
        qualified.insert(
            ("Заявки".to_owned(), "Кол".to_owned()),
            super::Value::scalar(42.0),
        );
        let env = Env::empty().with_qualified(qualified);
        assert_eq!(eval(&parsed, &env), Ok(super::Value::scalar(42.0)));
    }

    /// FR-061 Q3: первый проза-абзац — источник описания (фолбэк).
    #[test]
    fn first_prose_paragraph_takes_first_all_prose_paragraph() {
        let text =
            "Нода считает нагрузку на шлюз.\nВторая строка абзаца.\n\n800 rps\n\nЕщё проза позже.";
        assert_eq!(
            first_prose_paragraph(text).as_deref(),
            Some("Нода считает нагрузку на шлюз. Вторая строка абзаца.")
        );
    }

    /// FR-061 Q3: числовой/присваивающий абзац пропускается — следующая проза.
    #[test]
    fn first_prose_paragraph_skips_numi_paragraphs() {
        let text = "800 rps\n\nОписание после расчёта.";
        assert_eq!(
            first_prose_paragraph(text).as_deref(),
            Some("Описание после расчёта.")
        );
        let assign = "вход = 800\n\nПроза описания.";
        assert_eq!(
            first_prose_paragraph(assign).as_deref(),
            Some("Проза описания.")
        );
    }

    /// FR-061 Q3: код-фенс не описывает ноду; только проза после него.
    #[test]
    fn first_prose_paragraph_skips_fences() {
        let text = "```\nкод внутри фенса\n```\n\nПроза после фенса.";
        assert_eq!(
            first_prose_paragraph(text).as_deref(),
            Some("Проза после фенса.")
        );
    }

    /// FR-061 Q3: только Numi-контент — описания нет (None).
    #[test]
    fn first_prose_paragraph_none_when_all_numi() {
        assert_eq!(first_prose_paragraph("800 rps\n= 800\n"), None);
        assert_eq!(first_prose_paragraph(""), None);
        assert_eq!(first_prose_paragraph("```\nкод\n```\n"), None);
    }

    /// FR-069 (этап F): диапазон абзаца — [start, end) по строкам текста;
    /// склейка диапазона даёт байт-паритет с first_prose_paragraph.
    #[test]
    fn first_prose_paragraph_span_matches_paragraph() {
        let text = "Нода считает нагрузку.\nВторая строка.\n\n800 rps\n\nЕщё проза.";
        assert_eq!(first_prose_paragraph_span(text), Some((0, 2)));
        let joined = text
            .lines()
            .take(2)
            .collect::<Vec<&str>>()
            .join(" ")
            .trim()
            .to_owned();
        assert_eq!(
            first_prose_paragraph(text).as_deref(),
            Some(joined.as_str())
        );
        // Абзац после формул — индексы считаются от начала текста
        let text = "rps = 800\n\nэто описание системы\nв две строки\n\n800 * 2";
        assert_eq!(first_prose_paragraph_span(text), Some((2, 4)));
        assert_eq!(
            first_prose_paragraph(text).as_deref(),
            Some("это описание системы в две строки")
        );
    }

    /// FR-069: прогон с любой не-прозой не qualifies — диапазон ищется
    /// дальше (границы прогонов те же, что у first_prose_paragraph).
    #[test]
    fn first_prose_paragraph_span_skips_mixed_run() {
        let text = "заметка\n42 ms\n\nчистый абзац";
        assert_eq!(first_prose_paragraph_span(text), Some((3, 4)));
        // Присваивание внутри прогона — тоже не проза
        let text = "вход = 800\nи снова проза";
        assert_eq!(first_prose_paragraph_span(text), None);
    }

    /// FR-069: фенс обрывает прогон с обеих сторон; абзац у конца текста
    /// (без пустой строки) закрывается концом строк.
    #[test]
    fn first_prose_paragraph_span_fence_and_eof() {
        let text = "```\ncode = 1\n```\n\nабзац у фенса";
        assert_eq!(first_prose_paragraph_span(text), Some((4, 5)));
        let text = "x = 1\n\nхвост без пустой строки\nвторой ряд";
        assert_eq!(first_prose_paragraph_span(text), Some((2, 4)));
        assert_eq!(first_prose_paragraph_span("rps = 800"), None);
        assert_eq!(first_prose_paragraph_span(""), None);
    }
}
