//! Токенизация и символьные триграммы — общий слой lex-движков.
//!
//! Порт `_tokens`/`_char3`/`SYNONYMS` из `baseline_lex.py` (архив PoC волны 1,
//! тег `baseline-frozen`). Побитовая совместимость с Python — требование
//! golden-тестов: любое изменение здесь меняет p@1 на фикстурах.
//!
//! Соответствие Python-регексов:
//! - `[^\w\s]` → замена символа на пробел, если он не (буква|цифра|`_`|пробел);
//!   Python `\w` = Unicode alnum + `_` ≈ `char::is_alphanumeric() || '_'`;
//! - длина токена > 1 считается в кодпоинтах (`str::len` в Python);
//! - `\s+` → ` ` (схлопывание пробельных прогонов) + `strip()`.

use std::collections::BTreeSet;

/// Доменный словарь синонимов ru/en (порт `SYNONYMS`, побайтно).
///
/// Обогащает токены контекста и опций: если токен встретился в тексте,
/// к множеству добавляются его синонимы. Порядок значений не влияет на
/// результат (множество), поэтому порт идёт как match-таблица.
pub fn synonyms(word: &str) -> Option<&'static [&'static str]> {
    Some(match word {
        // юнит-экономика
        "привлечение" => &["cac"],
        "cac" => &["привлечение", "acquisition"],
        "ценность" => &["ltv"],
        "ltv" => &["ценность", "lifetime", "value"],
        "маржа" => &["margin"],
        "margin" => &["маржа", "маржинальность"],
        "выручка" => &["revenue", "arpu", "arppu", "mrr", "arr"],
        "revenue" => &["выручка"],
        "churn" => &["отток"],
        "отток" => &["churn"],
        "окупаемость" => &["payback", "roi"],
        "payback" => &["окупаемость"],
        "срок" => &["lifetime", "months"],
        "жизни" => &["lifetime"],
        "удержание" => &["retention"],
        "retention" => &["удержание"],
        "конверсия" => &["conversion", "cr"],
        "воронка" => &["funnel"],
        // инфраструктура
        "мощность" => &["capacity"],
        "capacity" => &["мощность", "пропускная"],
        "нагрузка" => &["load", "rps"],
        "трафик" => &["traffic", "rps"],
        "хостинг" => &["hosting", "infra"],
        "инфраструктура" => &["infra"],
        "сервер" => &["server", "instance"],
        "штат" => &["staff", "agents"],
        "поддержка" => &["support"],
        "обращения" => &["calls", "tickets"],
        // финансы
        "бюджет" => &["budget"],
        "расход" => &["burn", "cost", "spend"],
        "burn" => &["расход", "сжигание"],
        "кэш" => &["cash"],
        "cash" => &["кэш"],
        "рунвей" => &["runway"],
        "runway" => &["рунвей", "взлётная"],
        "вложения" => &["investment", "capex"],
        "инвестиции" => &["investment", "npv", "irr"],
        "дисконт" => &["discount"],
        "ставка" => &["rate"],
        "рост" => &["growth", "cagr"],
        "дисконтирования" => &["discount"],
        // аналитика
        "тест" => &["test", "ab"],
        "значимость" => &["significance", "pvalue"],
        "вариант" => &["variant", "b"],
        "контроль" => &["control", "a"],
        "когорта" => &["cohort"],
        "волна" => &["wave", "cohort"],
        "активация" => &["activation"],
        "средний" => &["avg", "average"],
        _ => return None,
    })
}

/// Токены текста: lower → не-(alnum|`_`|пробел) → пробел → split →
/// длина > 1 → множество + синонимы.
///
/// Возвращает `BTreeSet` — итерация уже в сортированном порядке
/// (эквивалент `sorted(set)` в Python).
pub fn tokens(text: &str) -> BTreeSet<String> {
    let lowered = text.to_lowercase();
    // Python: re.sub(r"[^\w\s]", " ", text) — ЗАМЕНА на пробел, затем .split()
    let normalized: String = lowered
        .chars()
        .map(|c| if token_separator(c) { ' ' } else { c })
        .collect();
    let mut out: BTreeSet<String> = BTreeSet::new();
    for word in normalized.split_whitespace() {
        if word.chars().count() > 1 {
            out.insert(word.to_string());
            if let Some(syns) = synonyms(word) {
                for s in syns {
                    out.insert((*s).to_string());
                }
            }
        }
    }
    out
}

fn token_separator(c: char) -> bool {
    !(c.is_alphanumeric() || c == '_' || c.is_whitespace())
}

/// Символьные триграммы нормализованного текста (порт `_char3`):
/// lower, `\s+` → один пробел, `strip`, все окна длины 3 (кодпоинты).
pub fn char3(text: &str) -> BTreeSet<String> {
    let lowered = text.to_lowercase();
    let mut normalized = String::with_capacity(lowered.len());
    let mut in_ws = false;
    for c in lowered.chars() {
        if c.is_whitespace() {
            if !in_ws {
                normalized.push(' ');
                in_ws = true;
            }
        } else {
            normalized.push(c);
            in_ws = false;
        }
    }
    let trimmed = normalized.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    let mut out = BTreeSet::new();
    if chars.len() >= 3 {
        for i in 0..=chars.len() - 3 {
            let tri: String = chars[i..i + 3].iter().collect();
            out.insert(tri);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_basic_and_synonyms() {
        let t = tokens("Маржа = price - cogs");
        // lower, punctuation → пробел, длина > 1
        assert!(t.contains("маржа"));
        assert!(t.contains("price"));
        assert!(t.contains("cogs"));
        // синонимы добавлены
        assert!(t.contains("margin")); // от «маржа»
        assert!(!t.contains("a")); // односимвольные отброшены
    }

    #[test]
    fn tokens_underscore_kept() {
        let t = tokens("ue_gross_margin");
        assert!(t.contains("ue_gross_margin"));
    }

    #[test]
    fn char3_basic() {
        let t = char3("  ab  cd ");
        // нормализация: "ab cd" → триграммы
        let expected: BTreeSet<String> = ["ab ", "b c", " cd"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(t, expected);
    }

    #[test]
    fn char3_short_is_empty() {
        assert!(char3("ab").is_empty());
        assert!(char3("").is_empty());
        assert_eq!(char3("abc").len(), 1);
    }
}
