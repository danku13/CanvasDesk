//! FR-045 Р-2 — CSV-снапшот источника данных ноды «Входные данные».
//!
//! Чистый парсер (RFC 4180-подмножество): заголовок + строки; разделитель
//! настраивается (`,`/`;`/`\t` — с автоопределением по строке заголовка),
//! кавычки с удвоением `""`, переводы строк CRLF/CR/LF, BOM снимается,
//! пустые значения допустимы. Лимиты — по ориентиру PRD-0003 (текст ≤ 5 МБ),
//! плюс капы строк/колонок (детерминированный отказ до выделения памяти).
//!
//! Выход — [`CsvSnapshot { fields, rows }`]: `fields` — имена колонок
//! (= имена выходов data-ноды, квалифицированная адресация FR-045 Р-5),
//! `rows` — значения для пролива (подключение к потоку — следующий этап
//! FR-045; снапшот при создании, §Q3).
//!
//! Allowlist зависимостей (ADR-0008) не расширяется: парсер ручной,
//! без внешних крейтов; чистый Rust — wasm-гейт (ADR-0011).

use crate::error::CoreError;

/// Кап размера входа (байты) — ориентир PRD-0003 (текст ≤ 5 МБ).
pub const MAX_CSV_BYTES: usize = 5 * 1024 * 1024;
/// Кап числа строк данных (без заголовка).
pub const MAX_CSV_ROWS: usize = 100_000;
/// Кап числа колонок.
pub const MAX_CSV_COLS: usize = 256;

/// Снимок CSV-источника: имена колонок + строки значений (дословно).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvSnapshot {
    /// Имена колонок из заголовка — выходы data-ноды (FR-045 Р-2).
    pub fields: Vec<String>,
    /// Строки значений (дословно, без интерпретации чисел).
    pub rows: Vec<Vec<String>>,
}

impl CsvSnapshot {
    /// Число записей источника (для пометки «CSV · N записей», FR-045 Р-2).
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

/// Ошибка формата CSV: строка/колонка 1-based (как в `CoreError::Parse`).
fn err(line: usize, column: usize, message: impl Into<String>) -> CoreError {
    CoreError::Parse {
        line,
        column,
        message: message.into(),
    }
}

/// Парсинг CSV с явным разделителем (детерминировано; `delimiter` — один
/// байт-символ вне кавычек). Заголовок обязателен и непуст; имена колонок
/// пустые/дубликаты — отказ (адресация полей должна быть однозначной,
/// FR-045 Р-5); рваные строки — отказ (целостность пролива).
pub fn parse_csv(input: &str, delimiter: char) -> Result<CsvSnapshot, CoreError> {
    if input.len() > MAX_CSV_BYTES {
        return Err(err(
            1,
            1,
            format!("CSV превышает лимит {} МБ", MAX_CSV_BYTES / (1024 * 1024)),
        ));
    }
    let text = input.strip_prefix('\u{feff}').unwrap_or(input);
    let rows = split_rows(text, delimiter)?;
    let Some(header) = rows.first() else {
        return Err(err(1, 1, "пустой CSV: нет заголовка"));
    };
    if header.iter().all(|f| f.trim().is_empty()) {
        return Err(err(1, 1, "пустой CSV: заголовок без имён колонок"));
    }
    if header.len() > MAX_CSV_COLS {
        return Err(err(
            1,
            1,
            format!(
                "число колонок {} превышает лимит {}",
                header.len(),
                MAX_CSV_COLS
            ),
        ));
    }
    let mut fields: Vec<String> = Vec::with_capacity(header.len());
    for (i, name) in header.iter().enumerate() {
        if name.trim().is_empty() {
            return Err(err(1, i + 1, format!("пустое имя колонки {}", i + 1)));
        }
        if fields.iter().any(|f| f == name) {
            return Err(err(1, i + 1, format!("дубликат колонки «{name}»")));
        }
        fields.push(name.clone());
    }
    let cols = fields.len();
    let mut data_rows: Vec<Vec<String>> = Vec::with_capacity(rows.len().saturating_sub(1));
    for (n, row) in rows.iter().enumerate().skip(1) {
        if data_rows.len() >= MAX_CSV_ROWS {
            return Err(err(
                n + 1,
                1,
                format!("число строк превышает лимит {MAX_CSV_ROWS}"),
            ));
        }
        if row.len() != cols {
            return Err(err(
                n + 1,
                row.len() + 1,
                format!("в строке {} полей {}, ожидалось {}", n, row.len(), cols),
            ));
        }
        data_rows.push(row.clone());
    }
    Ok(CsvSnapshot {
        fields,
        rows: data_rows,
    })
}

/// Автоопределение разделителя по строке заголовка: максимум вхождений
/// среди `,` `;` `\t` вне кавычек; ничья/отсутствие — `,`. Детерминировано.
pub fn sniff_delimiter(header_line: &str) -> char {
    let mut best: (char, usize) = (',', 0);
    for d in [',', ';', '\t'] {
        let count = count_outside_quotes(header_line, d);
        if count > best.1 {
            best = (d, count);
        }
    }
    best.0
}

fn count_outside_quotes(line: &str, delimiter: char) -> usize {
    let mut count = 0;
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => in_quotes = !in_quotes,
            _ if c == delimiter && !in_quotes => count += 1,
            _ => {
                // Удвоенная кавычка внутри кавычек — литерал: пропустить пару.
                if in_quotes && c == '"' && chars.peek() == Some(&'"') {
                    chars.next();
                }
            }
        }
    }
    count
}

/// Разбиение на строки полей (RFC 4180-подмножество): кавычки в начале
/// поля, `""` — литеральная кавычка, переводы строк вне кавычек завершают
/// запись; кавычка внутри некавыченного поля — литерал (терпимость).
// `unused_assignments`: трекинг строки/колонки продолжается до конца ввода —
// присвоения в хвосте последней итерации макросов не читаются (ложное
// срабатывание на деградационном пути), но убирать их нельзя: позиция
// нужна для диагностик на СЛЕДУЮЩИХ итерациях.
#[allow(unused_assignments)]
fn split_rows(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, CoreError> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut line = 1usize;
    let mut col = 0usize;
    let mut field_line = 1usize;
    let mut field_col = 0usize;
    let mut any = false;
    let mut chars = text.chars().peekable();
    macro_rules! push_field {
        () => {{
            if row.len() >= MAX_CSV_COLS {
                return Err(err(
                    line,
                    col,
                    format!("число колонок превышает лимит {MAX_CSV_COLS}"),
                ));
            }
            row.push(std::mem::take(&mut field));
        }};
    }
    macro_rules! push_row {
        () => {{
            push_field!();
            // Хвостовые пустые строки (финальный перевод строки) не создают записей.
            if !(row.len() == 1 && row[0].is_empty() && !any) {
                any = true;
                rows.push(row.clone());
            }
            row.clear();
            line += 1;
            col = 0;
        }};
    }
    while let Some(c) = chars.next() {
        col += 1;
        if in_quotes {
            match c {
                '"' => {
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quotes = false;
                    }
                }
                '\n' => {
                    field.push('\n');
                    line += 1;
                    col = 0;
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' if field.is_empty() => {
                    in_quotes = true;
                    field_line = line;
                    field_col = col;
                }
                _ if c == delimiter => push_field!(),
                '\n' => push_row!(),
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    push_row!();
                }
                _ => field.push(c),
            }
        }
    }
    if in_quotes {
        return Err(err(field_line, field_col, "незакрытая кавычка поля"));
    }
    if !row.is_empty() || !field.is_empty() {
        push_row!();
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Базовый разбор: заголовок + строки, значения дословно (инвариант 9).
    #[test]
    fn parse_basic() {
        let snap = parse_csv("товар,цена\nхлеб,50\nмолоко,90", ',').expect("ok");
        assert_eq!(snap.fields, vec!["товар", "цена"]);
        assert_eq!(snap.row_count(), 2);
        assert_eq!(snap.rows[0], vec!["хлеб", "50"]);
        assert_eq!(snap.rows[1], vec!["молоко", "90"]);
    }

    /// Кавычки: разделитель и переводы строк внутри поля, `""` — литерал.
    #[test]
    fn parse_quoted() {
        let snap = parse_csv(
            "name,note\n\"Иванов, И.\",\"сказал \"\"привет\"\"\"\n\"много\nстрок\",x",
            ',',
        )
        .expect("ok");
        assert_eq!(snap.rows[0], vec!["Иванов, И.", "сказал \"привет\""]);
        assert_eq!(snap.rows[1], vec!["много\nстрок", "x"]);
    }

    /// Разделители: точка с запятой и таб; sniff по заголовку.
    #[test]
    fn parse_delimiters_and_sniff() {
        let semi = "a;b\n1;2";
        assert_eq!(sniff_delimiter("a;b;c"), ';');
        assert_eq!(sniff_delimiter("a\tb\tc"), '\t');
        assert_eq!(sniff_delimiter("a,b\tc"), ',', "ничья → запятая");
        let snap = parse_csv(semi, sniff_delimiter("a;b")).expect("ok");
        assert_eq!(snap.rows[0], vec!["1", "2"]);
        let tab = parse_csv("a\tb\n1\t2", '\t').expect("ok");
        assert_eq!(tab.fields, vec!["a", "b"]);
    }

    /// BOM, CRLF/CR, пустые значения (инвариант 9).
    #[test]
    fn parse_bom_crlf_empty() {
        let snap = parse_csv("\u{feff}a,b\r\n1,\r\n,2", ',').expect("ok");
        assert_eq!(snap.fields, vec!["a", "b"]);
        assert_eq!(snap.rows[0], vec!["1", ""]);
        assert_eq!(snap.rows[1], vec!["", "2"]);
        let cr = parse_csv("a,b\r1,2", ',').expect("ok");
        assert_eq!(cr.rows[0], vec!["1", "2"]);
    }

    /// Рваная строка — детерминированный отказ с координатой.
    #[test]
    fn parse_ragged_row_error() {
        let e = parse_csv("a,b\n1\n2,3", ',').expect_err("рваная строка");
        match e {
            CoreError::Parse { line, message, .. } => {
                assert_eq!(line, 2);
                assert!(message.contains("1 пол"), "сообщение: {message}");
            }
            _ => panic!("не та ошибка"),
        }
    }

    /// Пустое имя колонки и дубликат — отказ (однозначность адресации).
    #[test]
    fn parse_header_errors() {
        let e = parse_csv("a,\n1,2", ',').expect_err("пустое имя");
        assert!(matches!(e, CoreError::Parse { line: 1, .. }));
        let e = parse_csv("a,a\n1,2", ',').expect_err("дубликат");
        assert!(matches!(e, CoreError::Parse { line: 1, .. }));
    }

    /// Незакрытая кавычка — отказ с позицией начала поля.
    #[test]
    fn parse_unclosed_quote_error() {
        let e = parse_csv("a,b\n\"x,2", ',').expect_err("незакрытая кавычка");
        assert!(matches!(e, CoreError::Parse { line: 2, .. }));
    }

    /// Лимит размера — отказ до разбора (ориентир PRD-0003).
    #[test]
    fn parse_size_limit() {
        let big = format!("a\n{}", "x".repeat(MAX_CSV_BYTES + 1));
        let e = parse_csv(&big, ',').expect_err("лимит");
        assert!(matches!(e, CoreError::Parse { line: 1, .. }));
    }

    /// Пустой вход — отказ; финальный перевод строки не создаёт запись.
    #[test]
    fn parse_empty_and_trailing_newline() {
        assert!(parse_csv("", ',').is_err());
        assert!(parse_csv("\n\n", ',').is_err());
        let snap = parse_csv("a\n1\n", ',').expect("ok");
        assert_eq!(snap.row_count(), 1, "хвостовой перевод строки — не запись");
    }
}
