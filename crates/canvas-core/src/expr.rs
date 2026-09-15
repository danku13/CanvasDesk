//! FR-013: Numi-base язык формул для text-нод (`canvasdesk.expr`).
//!
//! Чистый Rust, без I/O и глобального состояния (инварианты 1/2 FR-013):
//! [`parse`] — `&str -> Result<Expr, ParseError>`, [`eval`] —
//! `(&Expr, &Env) -> Result<Value, EvalError>`. Результат — число с
//! единицей измерения (композитные единицы вида `ms·req/s` поддержаны).
//!
//! Грамматика v1 (Numi-base):
//! - литералы: числа (`5`, `3.14`, `1k`, `2M`) и единицы (`ms`, `sec`,
//!   `min`, `h`, `req`, `req/s`, `rps`, `B`, `KB`, `MB`, `GB`, `$`, `%`);
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
//! владельца). Поток значений между нодами (`$in`) — FR-014; доменные
//! функции (`mm1`, `littles_law`) — FR-015.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

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
    const fn new(dim: Dimension, exp: i8, scale: f64, name: &'static str) -> Self {
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
    /// операндов сложения/вычитания и функций.
    fn dims(&self) -> BTreeMap<Dimension, i16> {
        let mut map = BTreeMap::new();
        for atom in &self.atoms {
            *map.entry(atom.key()).or_insert(0) += atom.exp as i16;
        }
        map.retain(|_, exp| *exp != 0);
        map
    }

    /// Произведение масштабов атомов: значение единицы в базовых единицах
    /// её размерностей (`ms` → 0.001, `min` → 60, `MB` → 1024²).
    fn scale(&self) -> f64 {
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
                *c = Atom::new(Dimension::Rate, 1, 1.0, "req/s");
            }
        }
        Unit { atoms }
    }

    /// Отображение единицы: положительные степени через `·` (с˅2/˅3),
    /// отрицательные — через `/` (`ms·req/s`, `B/s`, `ms²`).
    fn display(&self) -> String {
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
/// синоним имени (`s`/`sec`) равенству не мешает.
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
/// Кириллические синонимы — v2 (отложено владельцем).
const UNIT_TABLE: &[(&str, Dimension, f64)] = &[
    ("ms", Dimension::Time, 0.001),
    ("s", Dimension::Time, 1.0),
    ("sec", Dimension::Time, 1.0),
    ("secs", Dimension::Time, 1.0),
    ("min", Dimension::Time, 60.0),
    ("h", Dimension::Time, 3600.0),
    ("hour", Dimension::Time, 3600.0),
    ("req", Dimension::Count, 1.0),
    ("reqs", Dimension::Count, 1.0),
    ("rps", Dimension::Rate, 1.0),
    ("req/s", Dimension::Rate, 1.0),
    ("B", Dimension::Bytes, 1.0),
    ("KB", Dimension::Bytes, 1024.0),
    ("MB", Dimension::Bytes, 1024.0 * 1024.0),
    ("GB", Dimension::Bytes, 1024.0 * 1024.0 * 1024.0),
    ("$", Dimension::Money, 1.0),
    ("usd", Dimension::Money, 1.0),
    ("%", Dimension::Percent, 1.0),
];

/// Атом по токену таблицы.
fn unit_atom(token: &str) -> Option<Atom> {
    UNIT_TABLE
        .iter()
        .find(|(name, _, _)| *name == token)
        .map(|(name, dim, scale)| Atom::new(dim.clone(), 1, *scale, name))
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
    fn dims(&self) -> BTreeMap<Dimension, i16> {
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let num = format_num(self.num);
        let unit = self.unit.display();
        if unit.is_empty() {
            f.write_str(&num)
        } else {
            write!(f, "{num} {unit}")
        }
    }
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
}

/// Окружение вычисления: значения переменных (FR-013 — только локальные
/// переменные формулы; `$in`/`$1` из FR-014 придут как предзаполненный Env).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Env {
    vars: HashMap<String, Value>,
}

impl Env {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn set(mut self, name: impl Into<String>, value: Value) -> Self {
        self.vars.insert(name.into(), value);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.vars.get(name)
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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
    #[error("единицы не совместимы: {lhs} и {rhs}")]
    UnitMismatch { lhs: String, rhs: String },
    #[error("неизвестная переменная: {0}")]
    UnknownVariable(String),
    #[error("неизвестная функция: {0}")]
    UnknownFunction(String),
    #[error("{func}: {msg}")]
    BadCall { func: String, msg: String },
    #[error("деление на ноль")]
    DivisionByZero,
}

// --- Лексер ---

/// Токен лексера.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    /// Единица из таблицы (распознаётся только после числа или `$`-валюта).
    Unit(&'static str),
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
    /// `latency х replicas`).
    fn lex_ident(&mut self) -> Tok {
        if self.after_number {
            if let Some((name, len)) = self.unit_here() {
                self.pos += len;
                self.after_number = false;
                self.operand_ended = true;
                return Tok::Unit(name);
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
                // `$5` — префикс валюты; `%` — только суффикс (после числа)
                self.pos += 1;
                self.after_number = false;
                self.operand_ended = false;
                Tok::Unit("$")
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
            // Начала утверждений (в т.ч. унарный знак: `-3 ms`)
            Tok::Num(_) | Tok::Ident(_) | Tok::Unit(_) | Tok::LParen | Tok::Plus | Tok::Minus => {
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
            // Сопоставление без знака: число/единица/скобка/переменная
            // сразу за операндом (`5 ms`, `3 replicas` из примера владельца,
            // `(a + b) ms`); Ident `(` уже разобран как вызов на уровне primary
            Some(Tok::Num(_) | Tok::Unit(_) | Tok::LParen | Tok::Ident(_)) => (BinOp::Mul, true),
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

/// Вычислить формулу в окружении `env` (чистая функция, инвариант 2 FR-013).
/// Блок заводит локальную область поверх `env`; результат — значение
/// последнего утверждения.
pub fn eval(expr: &Expr, env: &Env) -> Result<Value, EvalError> {
    match expr {
        Expr::Num(num, unit) => Ok(Value {
            num: *num,
            unit: unit.clone(),
        }),
        Expr::Var(name) => env
            .get(name)
            .cloned()
            .ok_or_else(|| EvalError::UnknownVariable(name.clone())),
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
/// парсящаяся как выражение, — авто-формула: любая ошибка строку
/// результата не создаёт (проза «Встреча в 15:00» не краснеет).
/// Присваивание (`rps = 1000`) связывает переменную и возвращает
/// присвоенное значение. Внутри код-фенсов (``` … ```) формулы не
/// вычисляются — код не калькулятор.
pub fn eval_lines(source: &str) -> Vec<Option<ExprOutcome>> {
    let mut env = Env::empty();
    let mut in_fence = false;
    source
        .split('\n')
        .map(|line| {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                return None;
            }
            if in_fence {
                return None;
            }
            eval_line(line, &mut env)
        })
        .collect()
}

/// Одна строка сценария: результат или None (не формула / тихая ошибка).
fn eval_line(line: &str, env: &mut Env) -> Option<ExprOutcome> {
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
            // Явная формула показывает ошибку вычисления; авто-строка тиха
            Err(err) if explicit => Some(ExprOutcome::Err(err.to_string())),
            Err(_) => None,
        },
        // Явная формула показывает ошибку парсинга; проза молчит
        Err(err) if explicit => Some(ExprOutcome::Err(err.to_string())),
        Err(_) => None,
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
        BinOp::Mul => Ok(Value {
            num: lhs.num * rhs.num,
            unit: lhs.unit.merged(&rhs.unit, 1),
        }),
        BinOp::Div => {
            if rhs.num == 0.0 {
                return Err(EvalError::DivisionByZero);
            }
            Ok(Value {
                num: lhs.num / rhs.num,
                unit: lhs.unit.merged(&rhs.unit, -1),
            })
        }
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

    /// Единицы после числа только в позиции единиц: переменная может
    /// называться как единица (`rps = 1000` — пример владельца).
    #[test]
    fn unit_names_are_valid_variables() {
        let value = eval(&parse("rps = 1000\nrps × 2").unwrap(), &Env::empty()).unwrap();
        assert_eq!(value, Value::scalar(2000.0));
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

    /// Явная («=») строка показывает ошибки парсинга/вычисления,
    /// авто-строка молчит при любых ошибках.
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
        assert_eq!(lines[3], None, "авто: ошибка вычисления тиха");
        assert_eq!(ok_text(&lines[4]), "4");
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
}
