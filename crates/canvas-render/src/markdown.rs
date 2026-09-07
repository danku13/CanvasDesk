//! Парсер markdown-подмножества для текста заметок (форматирование, пост-T7).
//!
//! Хранение — прямо в поле `text` ноды: `**жирный**`, `*курсив*`, `==подсветка==`.
//! Формат `.canvas` не меняется, Obsidian рендерит те же маркеры.
//!
//! Сознательные ограничения подмножества (не CommonMark):
//! - маркеры — тоггл флагов одним проходом; патологические вложения вида
//!   `**a*b**` не гарантируют результат CommonMark;
//! - незакрытый маркер остаётся литералом (проверка «есть ли закрывающий»);
//! - экранирование — `\*` и `\=`.
//!
//! Модуль — чистая CPU-логика (без GPU), offsets спанов — байтовые, в системе
//! координат «чистого» текста (без маркеров) — так их принимает set_rich_text.

/// Спан стиля в «чистом» тексте (байтовые offsets, end эксклюзивен).
/// Флаги комбинируются: `**==текст==**` — bold + highlight одним спаном.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StyleSpan {
    pub start: usize,
    pub end: usize,
    pub bold: bool,
    pub italic: bool,
    pub highlight: bool,
}

/// Текущее состояние флагов сканера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    bold: bool,
    italic: bool,
    highlight: bool,
}

impl Style {
    fn any(&self) -> bool {
        self.bold || self.italic || self.highlight
    }
}

/// Есть ли маркер `marker` в `text` начиная с позиции `from` (проверка
/// «незакрытый маркер — литерал»).
fn has_marker_ahead(text: &str, from: usize, marker: &str) -> bool {
    text.get(from..).is_some_and(|rest| rest.contains(marker))
}

/// Разобрать текст с маркерами: вернуть чистый текст и спаны стилей.
pub fn parse(text: &str) -> (String, Vec<StyleSpan>) {
    let bytes = text.as_bytes();
    let mut plain = String::with_capacity(text.len());
    let mut spans: Vec<StyleSpan> = Vec::new();
    let mut style = Style::default();
    // Байтовый offset начала текущего спана в чистом тексте
    let mut span_start = 0usize;
    let mut i = 0usize;

    // Закрыть текущий спан (если активен и непуст) — при смене флагов и в конце
    macro_rules! flush {
        () => {
            if style.any() && plain.len() > span_start {
                spans.push(StyleSpan {
                    start: span_start,
                    end: plain.len(),
                    bold: style.bold,
                    italic: style.italic,
                    highlight: style.highlight,
                });
            }
        };
    }

    while i < bytes.len() {
        // Экранирование: \* и \= — литералы
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && (bytes[i + 1] == b'*' || bytes[i + 1] == b'=')
        {
            plain.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        // Маркеры: ** (bold), * (italic), == (highlight) — тоггл флага.
        // Открытие без закрывающего маркера дальше по тексту — литерал.
        if bytes[i] == b'*' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                if style.bold || has_marker_ahead(text, i + 2, "**") {
                    flush!();
                    style.bold = !style.bold;
                    span_start = plain.len();
                    i += 2;
                    continue;
                }
            } else if style.italic || has_marker_ahead(text, i + 1, "*") {
                flush!();
                style.italic = !style.italic;
                span_start = plain.len();
                i += 1;
                continue;
            }
        }
        if bytes[i] == b'='
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'='
            && (style.highlight || has_marker_ahead(text, i + 2, "=="))
        {
            flush!();
            style.highlight = !style.highlight;
            span_start = plain.len();
            i += 2;
            continue;
        }
        // Обычный символ (маркеры выше — ASCII, граница UTF-8 сохранена)
        let Some(ch) = text.get(i..).and_then(|rest| rest.chars().next()) else {
            break;
        };
        plain.push(ch);
        i += ch.len_utf8();
    }
    flush!();
    (plain, spans)
}

/// Текст без маркеров (для заголовка карточки — первая строка тела).
pub fn strip(text: &str) -> String {
    parse(text).0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нет маркеров — текст как есть, спанов нет.
    #[test]
    fn plain_text_unchanged() {
        let (plain, spans) = parse("просто текст");
        assert_eq!(plain, "просто текст");
        assert!(spans.is_empty());
    }

    /// Каждый маркер отдельно: bold, italic, highlight.
    #[test]
    fn single_markers() {
        let (plain, spans) = parse("**жирный**");
        assert_eq!(plain, "жирный");
        assert_eq!(
            spans,
            [StyleSpan {
                start: 0,
                end: "жирный".len(),
                bold: true,
                italic: false,
                highlight: false,
            }]
        );

        let (plain, spans) = parse("*курсив*");
        assert_eq!(plain, "курсив");
        assert!(spans[0].italic && !spans[0].bold && !spans[0].highlight);

        let (plain, spans) = parse("==подсветка==");
        assert_eq!(plain, "подсветка");
        assert!(spans[0].highlight && !spans[0].bold && !spans[0].italic);
    }

    /// Маркер в середине текста: offsets — байтовые, в чистом тексте.
    #[test]
    fn marker_in_the_middle_byte_offsets() {
        // "аа **бб** вв": кириллица — 2 байта на символ
        let (plain, spans) = parse("аа **бб** вв");
        assert_eq!(plain, "аа бб вв");
        assert_eq!(spans.len(), 1);
        let span = spans[0];
        assert!(span.bold);
        assert_eq!(&plain[span.start..span.end], "бб");
        assert_eq!(span.start, 5); // "аа " = 4 байта + пробел
        assert_eq!(span.end, 9);
    }

    /// Комбинация флагов: **==текст==** — bold + highlight одним спаном.
    #[test]
    fn combined_flags() {
        let (plain, spans) = parse("**==важное==**");
        assert_eq!(plain, "важное");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold && spans[0].highlight && !spans[0].italic);

        // Вложение разных маркеров: *кур **жирн-кур** кур*
        let (plain, spans) = parse("*кур **жирн-кур** кур*");
        assert_eq!(plain, "кур жирн-кур кур");
        assert_eq!(spans.len(), 3);
        assert!(spans[0].italic && !spans[0].bold);
        assert!(spans[1].italic && spans[1].bold);
        assert!(spans[2].italic && !spans[2].bold);
    }

    /// Незакрытый маркер — литерал, флаг не включается.
    #[test]
    fn unterminated_marker_is_literal() {
        let (plain, spans) = parse("а ** не закрыто");
        assert_eq!(plain, "а ** не закрыто");
        assert!(spans.is_empty());

        let (plain, spans) = parse("а * не закрыто");
        assert_eq!(plain, "а * не закрыто");
        assert!(spans.is_empty());

        let (plain, spans) = parse("а == не закрыто");
        assert_eq!(plain, "а == не закрыто");
        assert!(spans.is_empty());

        // Маркер в самом конце — тоже литерал
        let (plain, spans) = parse("текст**");
        assert_eq!(plain, "текст**");
        assert!(spans.is_empty());
    }

    /// Пустая пара маркеров не даёт спана и текста.
    #[test]
    fn empty_pair_no_span() {
        let (plain, spans) = parse("****");
        assert_eq!(plain, "");
        assert!(spans.is_empty());
        let (plain, spans) = parse("====");
        assert_eq!(plain, "");
        assert!(spans.is_empty());
        // Одиночный маркер без пары — литерал
        let (plain, spans) = parse("**");
        assert_eq!(plain, "**");
        assert!(spans.is_empty());
    }

    /// Экранирование: \* и \= — литералы, маркером не считаются.
    #[test]
    fn escaped_markers_are_literal() {
        let (plain, spans) = parse(r"\*не курсив\*");
        assert_eq!(plain, "*не курсив*");
        assert!(spans.is_empty());

        let (plain, spans) = parse(r"\==не подсветка\==");
        assert_eq!(plain, "==не подсветка==");
        assert!(spans.is_empty());

        // Экранированный маркер внутри спана — обычный текст
        let (plain, spans) = parse(r"**жирный \* текст**");
        assert_eq!(plain, "жирный * текст");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold);
    }

    /// Спан через перевод строки: \n входит в спан (подсветка многострочного).
    #[test]
    fn span_across_newlines() {
        let (plain, spans) = parse("==раз\nдва==");
        assert_eq!(plain, "раз\nдва");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].highlight);
        assert_eq!(span_text(&plain, spans[0]), "раз\nдва");
    }

    /// Несколько спанов в одном тексте, обычный текст между ними — без спанов.
    #[test]
    fn multiple_spans_with_gaps() {
        let (plain, spans) = parse("**а** б *в*");
        assert_eq!(plain, "а б в");
        assert_eq!(spans.len(), 2);
        assert!(spans[0].bold);
        assert!(spans[1].italic);
        assert_eq!(span_text(&plain, spans[0]), "а");
        assert_eq!(span_text(&plain, spans[1]), "в");
    }

    /// strip убирает маркеры (для заголовка карточки).
    #[test]
    fn strip_removes_markers() {
        assert_eq!(
            strip("**важно**: сделать *сегодня*"),
            "важно: сделать сегодня"
        );
        assert_eq!(strip("без маркеров"), "без маркеров");
        assert_eq!(strip(""), "");
    }

    fn span_text(plain: &str, span: StyleSpan) -> &str {
        &plain[span.start..span.end]
    }
}
