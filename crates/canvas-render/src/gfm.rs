//! GFM-рендеринг заметок (без таблиц): построчный парсер блоков GitHub
//! Flavored Markdown + разбиение инлайн-сегментов со ссылками.
//!
//! Заметка хранит сырую markdown-разметку в поле `text` ноды (формат `.canvas`
//! не меняется); этот модуль — чистая CPU-логика (без GPU и зависимостей),
//! рендер потребляет блоки в text.rs (`shape_body`).
//!
//! Поддерживаемое подмножество (v1, таблицы GFM не поддерживаем):
//! - ATX-заголовки `#{1,6}` + пробел (без пробела — литерал); закрывающая
//!   последовательность `###` в конце строки обрезается;
//! - списки: маркированные (`-`, `*`, `+`), нумерованные (`1.`/`1)`, число
//!   любое — рендер нумерует по порядку), чекбоксы `[ ]`/`[x]`/`[X]`;
//! - цитаты: строки с префиксом `> ` (один уровень, `>>` — текст с `>`);
//! - фенсы кода ` ``` ` / `~~~` (3+, язык после открытия игнорируется рендером),
//!   незакрытый фенс — код до конца текста; фенс внутри списка не парсится;
//! - горизонтальные линии `---`/`***`/`___` (3+, допускаются пробелы между);
//! - Setext-заголовки (`текст` + `---`) НЕ поддерживаем: `---` всегда линия;
//! - вложенность списков не поддерживаем: строка с отступом перед маркером
//!   (`  - влож.`) маркером не считается — текст пункта/параграфа;
//! - одиночный `\n` внутри параграфа — перенос строки, пустая строка
//!   разделяет блоки.

/// Блок тела заметки (GFM, без таблиц).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// ATX-заголовок, уровень 1..=6 (рендерим 1–3 крупно, 4–6 как bold body).
    Heading { level: u8, text: String },
    /// Параграф: непустые подряд идущие строки, склеенные `\n`.
    Paragraph { text: String },
    /// Список: однотипные подряд идущие пункты (тип сменился — новый блок).
    List { ordered: bool, items: Vec<ListItem> },
    /// Цитата: строки `> …`, склеенные `\n`, маркер `>` снят (один уровень).
    Quote { text: String },
    /// Содержимое фенса кода (язык рендеру не нужен).
    Code { text: String },
    /// Горизонтальная линия.
    Rule,
}

/// Пункт списка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    /// Чекбокс: None — нет, Some(false) — `[ ]`, Some(true) — `[x]`/`[X]`.
    pub checkbox: Option<bool>,
    pub text: String,
}

/// Инлайн-сегмент для отрисовки: кусок текста (с инлайн-маркерами
/// `**`, `*`, `==`, `~~` — их разбирает markdown.rs) и признак ссылки.
/// Ссылка `[label](url)` отображается как `label` (URL не показываем).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub text: String,
    pub link: bool,
}

/// Снять ATX-префикс заголовка: если строка после ltrim начинается с 1–6
/// `#` и далее пробел — вернуть текст после маркера, иначе строку как есть.
/// Используется и заголовком карточки (cards.rs `title_for`), и парсером.
pub fn strip_atx(line: &str) -> &str {
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) {
        let rest = &trimmed[hashes..];
        if rest.starts_with(' ') || rest.starts_with('\t') {
            return rest.trim_start();
        }
    }
    line
}

/// Обрезать закрывающую последовательность ATX (`###` в конце, если перед
/// ними пробел) — GFM closing sequence.
fn strip_closing_hashes(text: &str) -> &str {
    let trimmed = text.trim_end();
    let hashes = trimmed.bytes().rev().take_while(|&b| b == b'#').count();
    if hashes > 0 && trimmed.len() > hashes {
        let before = &trimmed[..trimmed.len() - hashes];
        if before.ends_with(' ') || before.ends_with('\t') {
            return before.trim_end();
        }
    }
    trimmed
}

/// Текст ATX-заголовка уровня `level` (без маркера и closing sequence).
/// None — строка заголовком не является.
fn atx_heading(line: &str) -> Option<(u8, String)> {
    let trimmed = line.trim_start();
    if trimmed.len() < line.len() && line.len() - trimmed.len() > 3 {
        return None; // GFM: до 3 пробелов отступа, больше — не заголовок
    }
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !(rest.starts_with(' ') || rest.starts_with('\t')) {
        return None; // `#без пробела` — литерал
    }
    let text = strip_closing_hashes(rest.trim_start());
    Some((hashes as u8, text.to_owned()))
}

/// Маркер фенса: ``` или ~~~ (3+), в строке только маркер и info-string.
/// Возвращает (символ, длина маркера).
fn fence_marker(line: &str) -> Option<(char, usize)> {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let ch = chars.next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|&c| c == ch).count();
    if len < 3 {
        return None;
    }
    let rest = &trimmed[len..];
    // Info-string: непустая — только для backtick-фенса не может содержать `
    let valid_info = if ch == '`' { !rest.contains('`') } else { true };
    if valid_info {
        Some((ch, len))
    } else {
        None
    }
}

/// Закрывающий фенс: тот же символ, длина >= открывающей, далее только пробелы.
fn fence_close(line: &str, ch: char, open_len: usize) -> bool {
    let trimmed = line.trim();
    let len = trimmed.chars().take_while(|&c| c == ch).count();
    len >= open_len && trimmed[len..].trim().is_empty()
}

/// Thematic break: 3+ одинаковых символов из `- * _`, между ними только пробелы.
fn is_thematic_break(line: &str) -> bool {
    let trimmed = line.trim();
    let mut marker = None;
    let mut count = 0usize;
    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            continue;
        }
        match marker {
            None => {
                if ch != '-' && ch != '*' && ch != '_' {
                    return false;
                }
                marker = Some(ch);
                count = 1;
            }
            Some(m) if ch == m => count += 1,
            Some(_) => return false,
        }
    }
    count >= 3
}

/// Маркер пункта списка (только колонка 0 — вложенность не поддерживаем).
/// Возвращает (ordered, text) — text без маркера, с чекбоксом уже снятым.
fn list_marker(line: &str) -> Option<(bool, String, Option<bool>)> {
    let bytes = line.as_bytes();
    if bytes.is_empty() || bytes[0].is_ascii_whitespace() {
        return None;
    }
    let (ordered, rest) = if bytes[0].is_ascii_digit() {
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits > 9 {
            return None;
        }
        let sep = bytes.get(digits).copied();
        if sep != Some(b'.') && sep != Some(b')') {
            return None;
        }
        (true, &line[digits + 1..])
    } else if bytes[0] == b'-' || bytes[0] == b'*' || bytes[0] == b'+' {
        (false, &line[1..])
    } else {
        return None;
    };
    // Маркер должен быть отделён от текста пробелом (или строка кончилась)
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let rest = rest.trim_start();
    // Чекбокс: `[ ]` / `[x]` / `[X]` сразу после маркера
    let (checkbox, rest) = if rest.len() >= 3 && rest.starts_with('[') {
        let close = rest.as_bytes()[2];
        if close == b']'
            && (rest.as_bytes()[1] == b' '
                || rest.as_bytes()[1] == b'x'
                || rest.as_bytes()[1] == b'X')
        {
            let checked = rest.as_bytes()[1] != b' ';
            let after = &rest[3..];
            if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
                (Some(checked), after.trim_start())
            } else {
                (None, rest)
            }
        } else {
            (None, rest)
        }
    } else {
        (None, rest)
    };
    Some((ordered, rest.to_owned(), checkbox))
}

/// Строка начинает цитату: префикс `> ` (или строка из одного `>`).
/// `>>` — второй уровень не строим: снят один префикс, остальное текст.
fn quote_text(line: &str) -> Option<String> {
    if let Some(rest) = line.strip_prefix("> ") {
        Some(rest.to_owned())
    } else if line == ">" {
        Some(String::new())
    } else if line.starts_with(">>") {
        Some(line[1..].to_owned())
    } else {
        None
    }
}

/// Разобрать текст заметки на GFM-блоки (без таблиц). Пустой текст —
/// пустой вектор.
pub fn parse_blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut blocks = Vec::new();
    let mut i = 0usize;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        // Фенс кода (только top-level)
        if let Some((ch, open_len)) = fence_marker(line) {
            let mut code_lines: Vec<&str> = Vec::new();
            i += 1;
            let mut closed = false;
            while i < lines.len() {
                if fence_close(lines[i], ch, open_len) {
                    closed = true;
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            let _ = closed; // незакрытый фенс — код до конца текста
            blocks.push(Block::Code {
                text: code_lines.join("\n"),
            });
            continue;
        }
        // ATX-заголовок
        if let Some((level, heading_text)) = atx_heading(line) {
            blocks.push(Block::Heading {
                level,
                text: heading_text,
            });
            i += 1;
            continue;
        }
        // Горизонтальная линия (Setext не поддерживаем — `---` всегда линия)
        if is_thematic_break(line) {
            blocks.push(Block::Rule);
            i += 1;
            continue;
        }
        // Цитата
        if let Some(first) = quote_text(line) {
            let mut quote_lines = vec![first];
            i += 1;
            while i < lines.len() {
                if let Some(rest) = quote_text(lines[i]) {
                    quote_lines.push(rest);
                    i += 1;
                } else {
                    break;
                }
            }
            blocks.push(Block::Quote {
                text: quote_lines.join("\n"),
            });
            continue;
        }
        // Списки: однотипные подряд идущие пункты — один блок
        if let Some((ordered, item_text, checkbox)) = list_marker(line) {
            let mut items = vec![ListItem {
                checkbox,
                text: item_text,
            }];
            i += 1;
            while i < lines.len() {
                // Продолжение пункта: строка с отступом (без нового маркера)
                if lines[i].starts_with(' ') || lines[i].starts_with('\t') {
                    if lines[i].trim().is_empty() {
                        break;
                    }
                    if let Some(last) = items.last_mut() {
                        last.text.push('\n');
                        last.text.push_str(lines[i].trim_start());
                    }
                    i += 1;
                    continue;
                }
                match list_marker(lines[i]) {
                    Some((o, text, checkbox)) if o == ordered => {
                        items.push(ListItem { checkbox, text });
                        i += 1;
                    }
                    _ => break,
                }
            }
            blocks.push(Block::List { ordered, items });
            continue;
        }
        // Параграф: непустые строки до пустой/другого блока
        let mut para = String::from(line);
        i += 1;
        while i < lines.len() && !lines[i].trim().is_empty() {
            let next = lines[i];
            if fence_marker(next).is_some()
                || atx_heading(next).is_some()
                || is_thematic_break(next)
                || quote_text(next).is_some()
                || list_marker(next).is_some()
            {
                break;
            }
            para.push('\n');
            para.push_str(next);
            i += 1;
        }
        blocks.push(Block::Paragraph { text: para });
    }
    blocks
}

/// Разбить строку на инлайн-сегменты: `[label](url)` → сегмент с
/// link = true (URL скрывается, показывается label). Непарные `[`/`](` —
/// литералами; пустой label (`[]()`) — литералом.
pub fn inline_segments(text: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // Поиск `]` (без вложенности) и сразу за ним `(…)`
            if let Some(close) = text[i + 1..].find(']').map(|pos| i + 1 + pos) {
                let after = close + 1;
                if text[after..].starts_with('(') {
                    if let Some(url_close) = text[after + 1..].find(')').map(|pos| after + 1 + pos)
                    {
                        let label = &text[i + 1..close];
                        let _url = &text[after + 1..url_close];
                        // Пустой label — не ссылка (литералом)
                        if !label.is_empty() {
                            if !literal.is_empty() {
                                segments.push(Segment {
                                    text: std::mem::take(&mut literal),
                                    link: false,
                                });
                            }
                            segments.push(Segment {
                                text: label.to_owned(),
                                link: true,
                            });
                            i = url_close + 1;
                            continue;
                        }
                    }
                }
            }
        }
        // Литеральный символ (байтовая граница ASCII `[` сохранена)
        let Some(ch) = text.get(i..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        literal.push(ch);
        i += ch.len_utf8();
    }
    if !literal.is_empty() {
        segments.push(Segment {
            text: literal,
            link: false,
        });
    }
    // Текст без сегментов (в т.ч. пустой) — один пустой сегмент не нужен:
    // пустой ввод → пустой вектор
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ATX-заголовки уровней 1–6; `#` без пробела — литерал/параграф.
    #[test]
    fn headings_levels_and_no_space() {
        assert_eq!(
            parse_blocks("# H1"),
            [Block::Heading {
                level: 1,
                text: "H1".into()
            }]
        );
        assert_eq!(
            parse_blocks("###### H6"),
            [Block::Heading {
                level: 6,
                text: "H6".into()
            }]
        );
        assert_eq!(
            parse_blocks("####### семь решёток — параграф"),
            [Block::Paragraph {
                text: "####### семь решёток — параграф".into()
            }]
        );
        // `#` без пробела — литерал
        assert_eq!(
            parse_blocks("#заголовок"),
            [Block::Paragraph {
                text: "#заголовок".into()
            }]
        );
        // Закрывающая последовательность обрезается
        assert_eq!(
            parse_blocks("## Заголовок ##"),
            [Block::Heading {
                level: 2,
                text: "Заголовок".into()
            }]
        );
        // `#прилипший` в конце не closing sequence (нет пробела перед)
        assert_eq!(
            parse_blocks("# Заг#оловок"),
            [Block::Heading {
                level: 1,
                text: "Заг#оловок".into()
            }]
        );
    }

    /// Параграф из двух строк склеивается `\n`; пустая строка разделяет блоки.
    #[test]
    fn paragraphs_join_lines() {
        assert_eq!(
            parse_blocks("первая\nвторая"),
            [Block::Paragraph {
                text: "первая\nвторая".into()
            }]
        );
        assert_eq!(
            parse_blocks("а\n\nб"),
            [
                Block::Paragraph { text: "а".into() },
                Block::Paragraph { text: "б".into() },
            ]
        );
        assert!(parse_blocks("").is_empty());
        assert!(parse_blocks("\n\n").is_empty());
    }

    /// Маркированные и нумерованные списки (число любое, порядок сохраняется).
    #[test]
    fn lists_unordered_and_ordered() {
        assert_eq!(
            parse_blocks("- a\n- b"),
            [Block::List {
                ordered: false,
                items: vec![
                    ListItem {
                        checkbox: None,
                        text: "a".into()
                    },
                    ListItem {
                        checkbox: None,
                        text: "b".into()
                    },
                ],
            }]
        );
        assert_eq!(
            parse_blocks("5. пятый\n7) седьмой"),
            [Block::List {
                ordered: true,
                items: vec![
                    ListItem {
                        checkbox: None,
                        text: "пятый".into()
                    },
                    ListItem {
                        checkbox: None,
                        text: "седьмой".into()
                    },
                ],
            }]
        );
        // Смена типа — новый блок
        assert_eq!(
            parse_blocks("- a\n1. b"),
            [
                Block::List {
                    ordered: false,
                    items: vec![ListItem {
                        checkbox: None,
                        text: "a".into()
                    }],
                },
                Block::List {
                    ordered: true,
                    items: vec![ListItem {
                        checkbox: None,
                        text: "b".into()
                    }],
                },
            ]
        );
        // `*` и `+` — тоже маркеры; `-` без пробела — не маркер
        assert_eq!(
            parse_blocks("* a\n+ b"),
            [Block::List {
                ordered: false,
                items: vec![
                    ListItem {
                        checkbox: None,
                        text: "a".into()
                    },
                    ListItem {
                        checkbox: None,
                        text: "b".into()
                    },
                ],
            }]
        );
        assert_eq!(parse_blocks("-a"), [Block::Paragraph { text: "-a".into() }]);
    }

    /// Чекбоксы трёх состояний: нет / `[ ]` / `[x]` (и `[X]`).
    #[test]
    fn list_checkboxes() {
        assert_eq!(
            parse_blocks("- [ ] todo\n- [x] done\n- [X] тоже done\n- plain"),
            [Block::List {
                ordered: false,
                items: vec![
                    ListItem {
                        checkbox: Some(false),
                        text: "todo".into()
                    },
                    ListItem {
                        checkbox: Some(true),
                        text: "done".into()
                    },
                    ListItem {
                        checkbox: Some(true),
                        text: "тоже done".into()
                    },
                    ListItem {
                        checkbox: None,
                        text: "plain".into()
                    },
                ],
            }]
        );
    }

    /// Цитата из двух строк склеивается; `>>` — один префикс снят, остальное текст.
    #[test]
    fn quotes_join_lines() {
        assert_eq!(
            parse_blocks("> раз\n> два"),
            [Block::Quote {
                text: "раз\nдва".into()
            }]
        );
        assert_eq!(
            parse_blocks(">> вложенная"),
            [Block::Quote {
                text: "> вложенная".into()
            }]
        );
        // Цитата прерывается пустой строкой
        assert_eq!(
            parse_blocks("> а\n\n> б"),
            [
                Block::Quote { text: "а".into() },
                Block::Quote { text: "б".into() },
            ]
        );
    }

    /// Фенс с языком и незакрытый фенс (код до конца текста).
    #[test]
    fn fences_open_and_unclosed() {
        assert_eq!(
            parse_blocks("```rust\nlet a = 1;\n```"),
            [Block::Code {
                text: "let a = 1;".into()
            }]
        );
        assert_eq!(
            parse_blocks("~~~\nодин\nдва"),
            [Block::Code {
                text: "один\nдва".into()
            }]
        );
        // Закрытие короче открытия — не закрытие
        assert_eq!(
            parse_blocks("````\ncode\n```\nеще code"),
            [Block::Code {
                text: "code\n```\nеще code".into()
            }]
        );
    }

    /// Горизонтальные линии: `---`, `- - -`, `***`, `___`; `--` — не линия.
    #[test]
    fn thematic_breaks() {
        for src in ["---", "- - -", "***", "___", " _ _ _ "] {
            assert_eq!(parse_blocks(src), [Block::Rule], "источник: {src}");
        }
        assert_eq!(parse_blocks("--"), [Block::Paragraph { text: "--".into() }]);
        // `---` после текста — всегда линия (Setext не поддерживаем)
        assert_eq!(
            parse_blocks("заголовок?\n---"),
            [
                Block::Paragraph {
                    text: "заголовок?".into()
                },
                Block::Rule,
            ]
        );
    }

    /// Строка с отступом перед маркером — не пункт (вложенность v1 не парсим).
    #[test]
    fn indented_marker_is_not_a_list_item() {
        // Внутри списка — продолжение предыдущего пункта
        assert_eq!(
            parse_blocks("- a\n  - не пункт"),
            [Block::List {
                ordered: false,
                items: vec![ListItem {
                    checkbox: None,
                    text: "a\n- не пункт".into()
                }],
            }]
        );
        // Вне списка — параграф
        assert_eq!(
            parse_blocks("  - не пункт"),
            [Block::Paragraph {
                text: "  - не пункт".into()
            }]
        );
    }

    /// Смешанный документ: заголовок, список, цитата, фенс, линия, параграф.
    #[test]
    fn mixed_document() {
        let text = "# Тема\n\n- пункт\n\n> цитата\n\n```\ncode\n```\n\n---\n\nхвост";
        assert_eq!(
            parse_blocks(text),
            [
                Block::Heading {
                    level: 1,
                    text: "Тема".into()
                },
                Block::List {
                    ordered: false,
                    items: vec![ListItem {
                        checkbox: None,
                        text: "пункт".into()
                    }],
                },
                Block::Quote {
                    text: "цитата".into()
                },
                Block::Code {
                    text: "code".into()
                },
                Block::Rule,
                Block::Paragraph {
                    text: "хвост".into()
                },
            ]
        );
    }

    /// inline_segments: ссылка → один сегмент с link; URL скрыт.
    #[test]
    fn inline_segments_link() {
        assert_eq!(
            inline_segments("[a](b)"),
            [Segment {
                text: "a".into(),
                link: true
            }]
        );
        assert_eq!(
            inline_segments("x [a](b) y [c](d)"),
            [
                Segment {
                    text: "x ".into(),
                    link: false
                },
                Segment {
                    text: "a".into(),
                    link: true
                },
                Segment {
                    text: " y ".into(),
                    link: false
                },
                Segment {
                    text: "c".into(),
                    link: true
                },
            ]
        );
    }

    /// Непарные скобки — литералы одним сегментом; пустой label — литерал.
    #[test]
    fn inline_segments_unpaired_are_literal() {
        assert_eq!(
            inline_segments("[a](b"),
            [Segment {
                text: "[a](b".into(),
                link: false
            }]
        );
        assert_eq!(
            inline_segments("[]()"),
            [Segment {
                text: "[]()".into(),
                link: false
            }]
        );
        assert_eq!(
            inline_segments("без ссылок"),
            [Segment {
                text: "без ссылок".into(),
                link: false
            }]
        );
        // Непарная `[` среди текста — литерал, остальное разбирается
        assert_eq!(
            inline_segments("а [b](c"),
            [Segment {
                text: "а [b](c".into(),
                link: false
            }]
        );
    }

    /// strip_atx: снятие маркера заголовка для заголовка карточки.
    #[test]
    fn strip_atx_heading_prefix() {
        assert_eq!(strip_atx("# Заголовок"), "Заголовок");
        assert_eq!(strip_atx("### Третий"), "Третий");
        assert_eq!(strip_atx("#без пробела"), "#без пробела");
        assert_eq!(strip_atx("обычная строка"), "обычная строка");
        assert_eq!(strip_atx("# "), "");
    }
}
