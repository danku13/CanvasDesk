//! FR-LLM-A / Q1 (privacy) — Redact engine.
//!
//! `redact_context(context, mode)` — чистая функция (std only, zero-dep),
//! применяется к контексту ноды перед отправкой в cloud LLM (Q1 ответ
//! владельца 2026-10-03: явный маркер `<redacted>`, сохраняем unit).
//!
//! **Правила замены** (см. PRD-0010 F-2.9):
//! - Вход: `price=10руб, cogs=4руб, months=36, cac=120руб`
//! - Выход: `price=<redacted> руб, cogs=<redacted> руб, months=<redacted>,
//!   cac=<redacted> руб`
//! - Шаблон: `(\w+)=(-?\d+(?:\.\d+)?)\s*(руб|₽|usd|\$|%|ч|мес|год|дн|шт|кг|м|сек|мин|gb|mb|tb|kb)?`
//! - Заменяем group 2 (число) на `<redacted>`, оставляя group 1 (ключ) и
//!   group 3 (единица измерения, если есть — разделяем пробелом для
//!   читаемости: `price=<redacted> руб` вместо `price=<redacted>руб`).
//!
//! **Зачем сохранять unit:** LLM (suggest) использует единицы измерения
//! для выбора шаблона (CAC в рублях → `unit-economics-cac`, в долларах
//! → `unit-economics-cac-usd`). Само число — конфиденциально (бизнес-метрика).
//!
//! **Реализация без `regex` крейта:** ручной char-scanner, чтобы
//! не тянуть `regex` в zero-dep дерево (ADR-0011 wasm-гейт, AGENTS.md
//! «без сети в хосте» + минимальный размер wasm-бандла).
//!
//! Mode `Off` (Local/SelfHosted) — контекст уходит как есть (данные не
//! покидают контур пользователя). Mode `Redact` (Cloud) — заменяем.

use crate::compliance::PrivacyMode;

/// Список распознаваемых единиц измерения (lowercase-сравнение).
/// Порядок важен для matching: длинные (`мес`, `год`) матчатся раньше
/// коротких (`м`), иначе `месяцы` сопоставилось бы с `м`.
const UNITS: &[&str] = &[
    "руб", "usd", "мес", "год", "сек", "мин", "gb", "mb", "tb", "kb", "дн", "шт", "₽", "$", "%",
    "ч", "м", "кг",
];

/// Применить redact к контексту ноды (Q1 privacy).
///
/// Вход: сериализованный контекст (как `serialize.py` PoC Laya —
/// `crates/canvas-suggest/src/context.rs`).
/// Выход: контекст с заменёнными числами в шаблонах `key=value[unit]`.
///
/// Pure function (std only), детерминированная. Используется:
/// - `LlmMmSource` (Stream C) перед `LlmProvider::choice()`.
/// - `AgentPanel` (Stream D) перед `LlmProvider::tool_calling()`.
/// - `GraphBuilder` (Stream D) перед `LlmProvider::chat()`.
pub fn redact_context(context: &str, mode: PrivacyMode) -> String {
    match mode {
        // Local/SelfHosted — данные не уходят, redact не нужен.
        PrivacyMode::Off => context.to_string(),
        // Cloud — заменяем числа в шаблонах key=value[unit].
        PrivacyMode::Redact => redact_cloud(context),
    }
}

/// Результат сопоставления шаблона `ident=number[unit]` в `redact_cloud`.
/// `prefix_end` — индекс первого символа после `=` (включая опциональные
/// пробелы после `=`), `value_end` — индекс первого символа после числа,
/// `unit_end` — индекс первого символа после единицы (равен `value_end`,
/// если единицы нет).
struct PatternMatch {
    /// Позиция сразу после `=` (и пробелов после него).
    prefix_end: usize,
    /// Позиция сразу после числа (последняя цифра/точка + 1).
    value_end: usize,
    /// Позиция сразу после единицы измерения (если есть).
    unit_end: usize,
}

/// Алгоритм redact для Cloud-режима. Сканирует строку слева направо,
/// ищет шаблон `идентификатор=число[unit]`, заменяет число на
/// `<redacted>`. Все прочие подстроки копируются как есть.
fn redact_cloud(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(s.len() + 32);
    let mut i = 0;

    while i < n {
        // Пытаемся сопоставить шаблон `ident=number[unit]` начиная с i.
        if let Some(m) = try_match_pattern(&chars, i) {
            // Дописываем идентификатор и `=` (включая опциональные пробелы
            // вокруг — `prefix_end` указывает на первый символ числа).
            for ch in &chars[i..m.prefix_end] {
                out.push(*ch);
            }
            // Заменяем число на `<redacted>`.
            out.push_str("<redacted>");
            // Дописываем единицу измерения (если есть), с пробелом перед ней.
            if m.unit_end > m.value_end {
                out.push(' ');
                for ch in &chars[m.value_end..m.unit_end] {
                    out.push(*ch);
                }
            }
            i = m.unit_end;
        } else {
            // Не совпало — копируем символ и двигаемся дальше.
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Сопоставить шаблон `ident=number[unit]` начиная с позиции `start`.
/// Возвращает `PatternMatch` или `None`, если шаблон не сопоставился.
fn try_match_pattern(chars: &[char], start: usize) -> Option<PatternMatch> {
    let n = chars.len();
    let mut i = start;

    // Идентификатор: первая буква или _, дальше буквы/цифры/_.
    if i >= n || !(chars[i].is_alphabetic() || chars[i] == '_') {
        return None;
    }
    while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }

    // Пропускаем пробелы между идентификатором и `=` (resilience:
    // `price = 10руб` тоже должно сработать — формат PoC Laya иногда
    // вставлял пробелы).
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    if i >= n || chars[i] != '=' {
        return None;
    }
    i += 1;
    while i < n && chars[i].is_whitespace() {
        i += 1;
    }
    let prefix_end = i;

    // Число: опциональный минус, дальше цифры с опциональной точкой.
    if i < n && chars[i] == '-' {
        i += 1;
    }
    let mut seen_digit = false;
    while i < n && chars[i].is_ascii_digit() {
        i += 1;
        seen_digit = true;
    }
    if i < n && chars[i] == '.' {
        i += 1;
        while i < n && chars[i].is_ascii_digit() {
            i += 1;
            seen_digit = true;
        }
    }
    if !seen_digit {
        return None;
    }
    let value_end = i;

    // Опциональная единица измерения: пропускаем максимум 1 пробел,
    // затем пытаемся сопоставить одну из UNITS (case-insensitive).
    let mut j = i;
    if j < n && chars[j] == ' ' {
        j += 1;
    }
    let unit_end = match try_match_unit(chars, j) {
        Some(end) => end,
        None => {
            // Единицы нет — возвращаем `unit_end == value_end`
            // (число без unit, как `months=36`).
            value_end
        }
    };

    Some(PatternMatch {
        prefix_end,
        value_end,
        unit_end,
    })
}

/// Попытаться сопоставить единицу измерения в `chars` начиная с `start`.
/// Возвращает индекс после сопоставленной единицы или `None`.
fn try_match_unit(chars: &[char], start: usize) -> Option<usize> {
    let rest: String = chars[start..].iter().collect();
    // Сортируем UNITS по убыванию длины, чтобы `мес` матчило раньше `м`.
    let mut sorted: Vec<&str> = UNITS.to_vec();
    sorted.sort_by_key(|u| std::cmp::Reverse(u.len()));

    for unit in sorted {
        let unit_lower = unit.to_lowercase();
        // `String::to_lowercase` корректно обрабатывает ASCII и кириллицу.
        if rest.to_lowercase().starts_with(&unit_lower) {
            return Some(start + unit.chars().count());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_mode_passthrough() {
        let ctx = "price=10руб, cogs=4руб";
        assert_eq!(redact_context(ctx, PrivacyMode::Off), ctx);
    }

    #[test]
    fn redact_basic_example_from_prd() {
        // Пример из PRD-0010 F-2.9:
        // Input:  price=10руб, cogs=4руб, months=36, cac=120руб
        // Output: price=<redacted> руб, cogs=<redacted> руб, months=<redacted>, cac=<redacted> руб
        let ctx = "price=10руб, cogs=4руб, months=36, cac=120руб";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(
            out,
            "price=<redacted> руб, cogs=<redacted> руб, months=<redacted>, cac=<redacted> руб"
        );
    }

    #[test]
    fn redact_decimal_and_negative() {
        let ctx = "price=10.5руб, discount=-2.0%";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "price=<redacted> руб, discount=<redacted> %");
    }

    #[test]
    fn redact_various_units() {
        let ctx = "cac=120$, roi=15%, hours=8ч, days=30дн";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(
            out,
            "cac=<redacted> $, roi=<redacted> %, hours=<redacted> ч, days=<redacted> дн"
        );
    }

    #[test]
    fn redact_no_unit_keeps_comma() {
        // months=36 — без единицы, просто число. Запятая после — обычный текст.
        let ctx = "months=36, year=2026";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "months=<redacted>, year=<redacted>");
    }

    #[test]
    fn redact_preserves_unrelated_text() {
        // Текст без `key=value[unit]` шаблона — без изменений.
        let ctx = "Hello world, just a free text without numbers.";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, ctx);
    }

    #[test]
    fn redact_no_value_no_change() {
        // `price=` без числа — не считается шаблоном, остаётся как есть.
        let ctx = "price=, cogs=4руб";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "price=, cogs=<redacted> руб");
    }

    #[test]
    fn redact_underscore_identifier() {
        let ctx = "unit_cac=120руб";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "unit_cac=<redacted> руб");
    }

    #[test]
    fn redact_long_units_first() {
        // `мес` должно матчиться как единое целое, а не `м` + `ес`.
        let ctx = "duration=36мес, length=10м";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "duration=<redacted> мес, length=<redacted> м");
    }

    #[test]
    fn redact_case_insensitive_ascii_units() {
        let ctx = "size=100MB, ram=8GB";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "size=<redacted> MB, ram=<redacted> GB");
    }

    #[test]
    fn redact_with_space_around_eq() {
        // Resilience: `price = 10руб` — пробелы вокруг `=`.
        let ctx = "price = 10руб";
        let out = redact_context(ctx, PrivacyMode::Redact);
        assert_eq!(out, "price = <redacted> руб");
    }

    #[test]
    fn redact_empty_string() {
        assert_eq!(redact_context("", PrivacyMode::Redact), "");
        assert_eq!(redact_context("", PrivacyMode::Off), "");
    }
}
