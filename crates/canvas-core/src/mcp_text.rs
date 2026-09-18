//! Нормализация текстов, приходящих по MCP (диспетчер canvas-app).
//!
//! MCP-клиенты (ИИ-агенты) нередко передают многострочный текст ноды
//! эскейп-последовательностями: в аргументах вызова пишется `\\n`
//! (двойное экранирование), после JSON-декода в строке остаются два символа
//! `\` + `n`, и рендер показывает их как текст. Транспорт (pipe, JSON-RPC)
//! при этом корректен — это поведение клиента.
//!
//! [`normalize_escapes`] — толерантная нормализация на границе приложения:
//! двухсимвольные `\r\n`/`\n`/`\t` принимаются как реальные управляющие
//! символы. Инварианты:
//! - идемпотентность: повторная нормализация не меняет результат
//!   (после замены пар `\`+`n` реальный LF — одиночный символ, больше не
//!   матчится);
//! - любая другая `\x`-последовательность сохраняет обратный слеш как есть
//!   (следующий символ не потребляется);
//! - применяется ТОЛЬКО к текстам, записанным через MCP; редактор UI и
//!   round-trip `.canvas` нормализацию не проходят.
//! Документированное ограничение: литеральная пара «обратный слеш + n»
//! в MCP-тексте недостижима (пишите `\\` в JSON — стандартный эскейп).
//! Тесты модуля — контракт правил выше.

/// Заменить двухсимвольные эскейп-последовательности на реальные управляющие
/// символы: `\r\n` → LF, `\n` → LF, `\t` → TAB. Прочие `\x` — без изменений
/// (слеш сохраняется, `x` не потребляется). Без слеша возвращает копию как есть.
pub fn normalize_escapes(text: &str) -> String {
    if !text.contains('\\') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('n') => {
                chars.next();
                out.push('\n');
            }
            Some('t') => {
                chars.next();
                out.push('\t');
            }
            Some('r') => {
                chars.next();
                // Literal-пара `\r\n` (два эскейпа подряд) — один LF:
                // подглядываем `\` + `n` без потери позиции при промахе.
                let mut lookahead = chars.clone();
                if matches!(lookahead.next(), Some('\\')) && matches!(lookahead.next(), Some('n')) {
                    chars.next();
                    chars.next();
                }
                out.push('\n');
            }
            // Неизвестная последовательность: слеш сохраняем, следующий
            // символ оставляем для следующей итерации (`\\n` → `\` + LF).
            _ => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Главный кейс агента: лист параметров одной строкой с literal `\n`.
    #[test]
    fn literal_newlines_become_real() {
        let normalized = normalize_escapes("rps = 1389 rps\\ncache_hit = 0.6");
        assert_eq!(normalized, "rps = 1389 rps\ncache_hit = 0.6");
        assert_eq!(normalized.lines().count(), 2);
    }

    /// Реальные переводы строк не трогаем.
    #[test]
    fn real_newlines_untouched() {
        let text = "a = 1\nb = 2";
        assert_eq!(normalize_escapes(text), text);
    }

    /// Текст без слешей возвращается без изменений (нет лишних аллокаций-сюрпризов).
    #[test]
    fn plain_text_unchanged() {
        assert_eq!(normalize_escapes("проза без слешей"), "проза без слешей");
    }

    /// `\t` — табуляция.
    #[test]
    fn literal_tab_becomes_tab() {
        assert_eq!(normalize_escapes("a\\tb"), "a\tb");
    }

    /// Literal `\r\n` → один LF (не два перевода).
    #[test]
    fn literal_crlf_becomes_single_lf() {
        assert_eq!(normalize_escapes("a\\r\\nb"), "a\nb");
        assert_eq!(normalize_escapes("a\\rb"), "a\nb");
    }

    /// Идемпотентность: второй проход — no-op.
    #[test]
    fn idempotent() {
        let once = normalize_escapes("a = 1\\nb = 2\\tc");
        let twice = normalize_escapes(&once);
        assert_eq!(once, twice);
    }

    /// Неизвестные последовательности: слеш сохраняется, символ не глотается.
    #[test]
    fn unknown_escapes_kept_verbatim() {
        assert_eq!(normalize_escapes("a\\xb"), "a\\xb");
        assert_eq!(normalize_escapes("C:\\Program Files"), "C:\\Program Files");
    }

    /// Двойной слеш перед n: первый слеш сохраняется, `\n` всё равно newline
    /// (документированное правило — пара «слеш+n» в MCP-тексте недостижима).
    #[test]
    fn double_backslash_then_n() {
        assert_eq!(normalize_escapes("a\\\\nb"), "a\\\nb");
    }

    /// Одиночный слеш в конце строки не падает.
    #[test]
    fn trailing_backslash() {
        assert_eq!(normalize_escapes("a\\"), "a\\");
    }
}
