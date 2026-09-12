//! Парсер markdown-подмножества для текста заметок (форматирование, пост-T7).
//!
//! Хранение — прямо в поле `text` ноды: `**жирный**`, `*курсив*`, `==подсветка==`,
//! `~~зачёркнутый~~`. Формат `.canvas` не меняется, Obsidian рендерит те же маркеры.
//!
//! Сознательные ограничения подмножества (не CommonMark):
//! - маркеры — тоггл флагов одним проходом; патологические вложения вида
//!   `**a*b**` не гарантируют результат CommonMark;
//! - незакрытый маркер остаётся литералом (проверка «есть ли закрывающий»);
//! - экранирование — `\*`, `\=` и `\~`.
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
    pub strike: bool,
}

/// Текущее состояние флагов сканера.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Style {
    bold: bool,
    italic: bool,
    highlight: bool,
    strike: bool,
}

impl Style {
    fn any(&self) -> bool {
        self.bold || self.italic || self.highlight || self.strike
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
                    strike: style.strike,
                });
            }
        };
    }

    while i < bytes.len() {
        // Экранирование: \*, \= и \~ — литералы
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && (bytes[i + 1] == b'*' || bytes[i + 1] == b'=' || bytes[i + 1] == b'~')
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
        // Маркер ~~ (strike) — тоггл; незакрытый — литерал
        if bytes[i] == b'~'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'~'
            && (style.strike || has_marker_ahead(text, i + 2, "~~"))
        {
            flush!();
            style.strike = !style.strike;
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

/// Флаг стиля — адресуемое поле StyleSpan (для toggle_style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleFlag {
    Bold,
    Italic,
    Highlight,
    Strike,
}

impl StyleFlag {
    fn get(self, span: &StyleSpan) -> bool {
        match self {
            StyleFlag::Bold => span.bold,
            StyleFlag::Italic => span.italic,
            StyleFlag::Highlight => span.highlight,
            StyleFlag::Strike => span.strike,
        }
    }
    /// Значение флага в покрытии (Style — внутреннее представление).
    fn get_style(self, style: &Style) -> bool {
        match self {
            StyleFlag::Bold => style.bold,
            StyleFlag::Italic => style.italic,
            StyleFlag::Highlight => style.highlight,
            StyleFlag::Strike => style.strike,
        }
    }
    fn set_style(self, style: &mut Style, value: bool) {
        match self {
            StyleFlag::Bold => style.bold = value,
            StyleFlag::Italic => style.italic = value,
            StyleFlag::Highlight => style.highlight = value,
            StyleFlag::Strike => style.strike = value,
        }
    }
    fn marker(self) -> &'static str {
        match self {
            StyleFlag::Bold => "**",
            StyleFlag::Italic => "*",
            StyleFlag::Highlight => "==",
            StyleFlag::Strike => "~~",
        }
    }
}

/// Покрытие чистого текста флагами по байтам из спанов (спаны
/// комбинируются по OR, как при парсинге вложенных маркеров).
fn build_coverage(plain_len: usize, spans: &[StyleSpan]) -> Vec<Style> {
    let mut coverage = vec![Style::default(); plain_len];
    for span in spans {
        for slot in coverage
            .iter_mut()
            .take(span.end.min(plain_len))
            .skip(span.start.min(plain_len))
        {
            slot.bold |= span.bold;
            slot.italic |= span.italic;
            slot.highlight |= span.highlight;
            slot.strike |= span.strike;
        }
    }
    coverage
}

/// Собрать спаны из покрытия: walk по байтам `plain`, максимальные отрезки
/// с одинаковым набором флагов (нет флагов — без спана). Нормализация:
/// смежные спаны с одинаковыми флагами сливаются, пустые отбрасываются.
fn spans_from_coverage(plain_len: usize, coverage: &[Style]) -> Vec<StyleSpan> {
    let mut spans = Vec::new();
    let mut start = 0usize;
    while start < plain_len {
        let style = coverage[start];
        let mut end = start + 1;
        while end < plain_len && coverage[end] == style {
            end += 1;
        }
        if style.any() {
            spans.push(StyleSpan {
                start,
                end,
                bold: style.bold,
                italic: style.italic,
                highlight: style.highlight,
                strike: style.strike,
            });
        }
        start = end;
    }
    spans
}

/// Тоггл флага стиля на диапазоне [start, end) чистого текста (байтовые
/// offsets; редактор работает в plain-координатах, WYSIWYG).
///
/// Правило (приёмка п.7):
/// - хоть один байт диапазона БЕЗ флага → флаг назначается ВСЕМУ диапазону
///   (спаны сплитятся/мержатся пересборкой покрытия);
/// - все байты уже с флагом: если затронутые раны флага целиком внутри
///   диапазона (границы выделения совпадают с границами стилизованного
///   региона) → флаг снимается; строгий поддиапазон рана → no-op (повторное
///   «сделать bold» уже bold-подстроки оставляет её bold).
///
/// Остальные флаги не трогаются. Возвращает нормализованный список спанов.
pub fn toggle_style(
    plain_len: usize,
    spans: &[StyleSpan],
    start: usize,
    end: usize,
    flag: StyleFlag,
) -> Vec<StyleSpan> {
    let start = start.min(plain_len);
    let end = end.min(plain_len).max(start);
    let mut coverage = build_coverage(plain_len, spans);
    if end > start {
        let all_flagged = (start..end).all(|pos| flag.get_style(&coverage[pos]));
        if !all_flagged {
            // Назначение всему диапазону
            for slot in coverage.iter_mut().take(end).skip(start) {
                flag.set_style(slot, true);
            }
        } else {
            // Максимальные раны флага по ВСЕМУ тексту, пересекающие диапазон
            let mut runs: Vec<(usize, usize)> = Vec::new();
            let mut pos = 0usize;
            while pos < plain_len {
                if flag.get_style(&coverage[pos]) {
                    let mut run_end = pos + 1;
                    while run_end < plain_len && flag.get_style(&coverage[run_end]) {
                        run_end += 1;
                    }
                    if pos < end && start < run_end {
                        runs.push((pos, run_end));
                    }
                    pos = run_end;
                } else {
                    pos += 1;
                }
            }
            // Снятие — только если каждый затронутый ран целиком в диапазоне;
            // ран шире выделения (строгий поддиапазон) — no-op
            if !runs.is_empty() && runs.iter().all(|&(s, e)| start <= s && e <= end) {
                for slot in coverage.iter_mut().take(end).skip(start) {
                    flag.set_style(slot, false);
                }
            }
        }
    }
    spans_from_coverage(plain_len, &coverage)
}

/// Назначить/снять флаг на диапазоне без тогл-семантики (покрытие +
/// нормализация). Используется редактором для «липкого» стиля ввода
/// (Ctrl+B без выделения: последующий ввод — bold).
pub fn set_style(
    plain_len: usize,
    spans: &[StyleSpan],
    start: usize,
    end: usize,
    flag: StyleFlag,
    value: bool,
) -> Vec<StyleSpan> {
    let start = start.min(plain_len);
    let end = end.min(plain_len).max(start);
    let mut coverage = build_coverage(plain_len, spans);
    for slot in coverage.iter_mut().take(end).skip(start) {
        flag.set_style(slot, value);
    }
    spans_from_coverage(plain_len, &coverage)
}

/// Сдвинуть/укоротить спаны после редактирования чистого текста:
/// `prefix` — общий префикс до/после, `before_edit`/`after_edit` — длины
/// заменённого хвоста (суффикс одинаков у обеих версий). Спаны до edit —
/// без изменений, после — сдвиг на (after_edit − before_edit),
/// пересекающиеся — прижимаются к границам (стиль сохраняется над
/// вставленным текстом, как в обычных редакторах).
pub fn adjust_spans(
    spans: &[StyleSpan],
    prefix: usize,
    before_edit: usize,
    after_edit: usize,
) -> Vec<StyleSpan> {
    let delta = after_edit as isize - before_edit as isize;
    let mut out = Vec::with_capacity(spans.len());
    for &span in spans {
        if span.end <= prefix {
            out.push(span);
        } else if span.start >= prefix + before_edit {
            let shifted = StyleSpan {
                start: (span.start as isize + delta).max(0) as usize,
                end: (span.end as isize + delta).max(0) as usize,
                ..span
            };
            if shifted.end > shifted.start {
                out.push(shifted);
            }
        } else {
            // Пересечение с edit-диапазоном: прижатие к новым границам
            let new_start = span.start.min(prefix);
            let tail_start = prefix + before_edit;
            let new_end = if span.end > tail_start {
                (span.end as isize + delta).max(0) as usize
            } else {
                prefix + after_edit
            };
            if new_end > new_start {
                out.push(StyleSpan {
                    start: new_start,
                    end: new_end,
                    ..span
                });
            }
        }
    }
    // Нормализация: сортировка + слияние смежных с одинаковыми флагами
    out.sort_by_key(|span| (span.start, span.end));
    let mut merged: Vec<StyleSpan> = Vec::with_capacity(out.len());
    for span in out {
        if let Some(last) = merged.last_mut() {
            let same_flags = last.bold == span.bold
                && last.italic == span.italic
                && last.highlight == span.highlight
                && last.strike == span.strike;
            if same_flags && span.start <= last.end {
                last.end = last.end.max(span.end);
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

/// Сериализация спанов обратно в markdown-текст для хранения в поле
/// `text` ноды (формат `.canvas` не меняется). Литеральные `*`, `=` и `~`
/// чистого текста экранируются (`\*`, `\=`, `\~` — тот же диалект, что парсит
/// `parse`), маркеры эмитятся на границах изменения флагов.
pub fn emit(plain: &str, spans: &[StyleSpan]) -> String {
    let plain_len = plain.len();
    // Границы изменения каждого флага (байтовые offsets чистого текста)
    let mut events: Vec<(usize, StyleFlag, bool)> = Vec::new();
    for &flag in &[
        StyleFlag::Bold,
        StyleFlag::Italic,
        StyleFlag::Highlight,
        StyleFlag::Strike,
    ] {
        let mut prev = false;
        for pos in 0..=plain_len {
            let cur = pos < plain_len
                && spans
                    .iter()
                    .any(|span| flag.get(span) && span.start <= pos && pos < span.end);
            if cur != prev {
                events.push((pos, flag, cur));
            }
            prev = cur;
        }
    }
    events.sort_by_key(|&(pos, _, _)| pos);
    let mut out = String::with_capacity(plain.len() + events.len() * 2);
    let mut pos = 0usize;
    for (event_pos, flag, on) in events {
        // Текст до события (экранирование литеральных маркеров)
        push_escaped(&mut out, &plain[pos..event_pos]);
        out.push_str(flag.marker());
        // Парный маркер: у одиночного '*' closing-скан parse'а всё равно
        // найдёт; порядок эмита открытий/закрытий на одной позиции — по
        // сортировке выше (стабильна по флагу)
        let _ = on;
        pos = event_pos;
    }
    push_escaped(&mut out, &plain[pos..]);
    out
}

/// Экранирование литеральных маркеров диалекта (`*`, `\=`- и `\~~`-пары в plain).
fn push_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        if ch == '*' || ch == '=' || ch == '~' {
            out.push('\\');
        }
        out.push(ch);
    }
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
                strike: false,
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

    // --- toggle_style / set_style / emit (WYSIWYG-модель редактора) ---

    fn bold_span(start: usize, end: usize) -> StyleSpan {
        StyleSpan {
            start,
            end,
            bold: true,
            italic: false,
            highlight: false,
            strike: false,
        }
    }

    /// Тоггл на полностью нестилизованном диапазоне: флаг назначается,
    /// смежные спаны сливаются в один.
    #[test]
    fn toggle_style_sets_on_plain_range() {
        // plain "раз два три": bold на "два" (7..13)
        let spans = toggle_style("раз два три".len(), &[], 7, 13, StyleFlag::Bold);
        assert_eq!(spans, [bold_span(7, 13)]);
        // Повтор на том же диапазоне (весь ран покрыт) — снятие
        let spans = toggle_style("раз два три".len(), &spans, 7, 13, StyleFlag::Bold);
        assert!(spans.is_empty());
    }

    /// Приёмка п.7: подстрока строго внутри bold-рана — no-op,
    /// подстрока НЕ теряет стиль.
    #[test]
    fn toggle_style_substring_inside_bold_run_is_noop() {
        // plain "привет мир", bold целиком (0..len): выделяем "риве" (2..10)
        let plain = "привет мир";
        let spans = toggle_style(
            plain.len(),
            &[bold_span(0, plain.len())],
            2,
            10,
            StyleFlag::Bold,
        );
        assert_eq!(
            spans,
            [bold_span(0, plain.len())],
            "no-op: стиль сохранился"
        );
    }

    /// Ровно весь ран — снятие; ран шире выделения с одной стороны — no-op.
    #[test]
    fn toggle_style_exact_run_removes_partial_run_noops() {
        let plain = "привет мир";
        let all = vec![bold_span(0, plain.len())];
        // Весь ран целиком — снятие
        let spans = toggle_style(plain.len(), &all, 0, plain.len(), StyleFlag::Bold);
        assert!(spans.is_empty());
        // Ран шире выделения справа (выделение — начало рана) — no-op
        let spans = toggle_style(plain.len(), &all, 0, 6, StyleFlag::Bold);
        assert_eq!(spans, all);
        // Ран шире выделения слева (выделение — конец рана) — no-op
        let spans = toggle_style(plain.len(), &all, 6, plain.len(), StyleFlag::Bold);
        assert_eq!(spans, all);
    }

    /// Диапазон частично стилизован (хоть один байт без флага) —
    /// флаг назначается ВСЕМУ диапазону, включая уже стилизованное.
    #[test]
    fn toggle_style_partial_range_sets_all() {
        // plain "aa bb cc", bold на "bb" (3..5); выделяем "a bb c" (1..6)
        let plain = "aa bb cc";
        let spans = toggle_style(plain.len(), &[bold_span(3, 5)], 1, 6, StyleFlag::Bold);
        assert_eq!(spans, [bold_span(1, 6)]);
    }

    /// Тоггл одного флага не трогает остальные (bold-подстрока внутри
    /// italic-рана: снятие italic с подстроки сплитит спан).
    #[test]
    fn toggle_style_keeps_other_flags() {
        let plain = "курсив жирный";
        let italic = StyleSpan {
            start: 0,
            end: plain.len(),
            bold: false,
            italic: true,
            highlight: false,
            strike: false,
        };
        // Назначаем bold на "жирный" (13..len): italic сохраняется везде
        let spans = toggle_style(plain.len(), &[italic], 13, plain.len(), StyleFlag::Bold);
        assert_eq!(spans.len(), 2);
        assert!(spans[0].italic && !spans[0].bold);
        assert!(spans[1].italic && spans[1].bold);
    }

    /// set_style: назначение/снятие без тогл-семантики (липкий стиль ввода).
    #[test]
    fn set_style_direct_assign_and_clear() {
        let plain = "текст";
        let spans = set_style(plain.len(), &[], 0, 5, StyleFlag::Bold, true);
        assert_eq!(spans, [bold_span(0, 5)]);
        let spans = set_style(plain.len(), &spans, 0, 5, StyleFlag::Bold, false);
        assert!(spans.is_empty());
    }

    /// adjust_spans: вставка внутрь рана расширяет стиль на вставленное
    /// (как в обычных редакторах — набор внутри bold остаётся bold).
    #[test]
    fn adjust_spans_extends_style_over_insert() {
        // plain "жирный", bold (0..12); вставляем "XY" на позиции 6:
        // prefix=6, before=0, after=2 → новый спан (0..14)
        let spans = adjust_spans(&[bold_span(0, 12)], 6, 0, 2);
        assert_eq!(spans, [bold_span(0, 14)]);
    }

    /// Round-trip emit → parse: спаны и чистый текст восстанавливаются.
    #[test]
    fn emit_parse_round_trip() {
        // Bold + italic смежно, highlight в хвосте
        let plain = "раз два три четыре";
        let spans = vec![
            StyleSpan {
                start: 0,
                end: 6,
                bold: true,
                italic: false,
                highlight: false,
                strike: false,
            },
            StyleSpan {
                start: 7,
                end: 13,
                bold: false,
                italic: true,
                highlight: false,
                strike: false,
            },
            StyleSpan {
                start: 14,
                end: plain.len(),
                bold: false,
                italic: false,
                highlight: true,
                strike: false,
            },
        ];
        let text = emit(plain, &spans);
        let (back_plain, back_spans) = parse(&text);
        assert_eq!(back_plain, plain, "текст: {text}");
        assert_eq!(back_spans, spans, "спаны: {text}");
    }

    /// Round-trip: литеральные маркеры в plain экранируются и не
    /// превращаются в маркеры при повторном parse.
    #[test]
    fn emit_escapes_literal_markers() {
        let plain = "a*b=c";
        let text = emit(plain, &[]);
        assert_eq!(text, r"a\*b\=c");
        let (back, spans) = parse(&text);
        assert_eq!(back, plain);
        assert!(spans.is_empty());
    }

    // --- strike (~~…~~) ---

    /// Парный маркер ~~ — спан со strike.
    #[test]
    fn strike_marker() {
        let (plain, spans) = parse("~~зачёркнуто~~");
        assert_eq!(plain, "зачёркнуто");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].strike && !spans[0].bold && !spans[0].italic && !spans[0].highlight);
    }

    /// Незакрытый ~~ — литерал, флаг не включается.
    #[test]
    fn unterminated_strike_is_literal() {
        let (plain, spans) = parse("а ~~ не закрыто");
        assert_eq!(plain, "а ~~ не закрыто");
        assert!(spans.is_empty());

        let (plain, spans) = parse("текст~~");
        assert_eq!(plain, "текст~~");
        assert!(spans.is_empty());
    }

    /// Экранирование \~ — литерал, маркером не считается.
    #[test]
    fn escaped_tilde_is_literal() {
        let (plain, spans) = parse(r"\~не strike\~");
        assert_eq!(plain, "~не strike~");
        assert!(spans.is_empty());

        // Экранированный маркер внутри спана — обычный текст
        let (plain, spans) = parse(r"**жирный \~\~ текст**");
        assert_eq!(plain, "жирный ~~ текст");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold);
    }

    /// Комбинация с bold: **~~ж~~** — один спан bold + strike.
    #[test]
    fn strike_combines_with_bold() {
        let (plain, spans) = parse("**~~ж~~**");
        assert_eq!(plain, "ж");
        assert_eq!(spans.len(), 1);
        assert!(spans[0].bold && spans[0].strike);
    }

    /// emit со strike: round-trip спанов и текста.
    #[test]
    fn emit_strike_round_trip() {
        let plain = "раз два";
        let spans = vec![StyleSpan {
            start: 0,
            end: "раз".len(),
            bold: false,
            italic: false,
            highlight: false,
            strike: true,
        }];
        let text = emit(plain, &spans);
        assert_eq!(text, "~~раз~~ два");
        let (back_plain, back_spans) = parse(&text);
        assert_eq!(back_plain, plain, "текст: {text}");
        assert_eq!(back_spans, spans, "спаны: {text}");
    }

    /// emit экранирует литеральную ~ в plain.
    #[test]
    fn emit_escapes_literal_tilde() {
        let plain = "a~b";
        let text = emit(plain, &[]);
        assert_eq!(text, r"a\~b");
        let (back, spans) = parse(&text);
        assert_eq!(back, plain);
        assert!(spans.is_empty());
    }
}
