//! L0-префильтр каталога до ≤N опций (порт `shortlist.py`, волна 1).
//!
//! Правила (детерминированы, дословно из harness):
//! 1) скоринг — пересечение токенов контекста с именами+параметрами+ключом
//!    шаблона (стоп-слова выкинуты);
//! 2) категорийный бонус по категории схемы (в продукте — пустая категория,
//!    бонус 0; правило оставлено для совместимости эталона);
//! 3) `none` добавляется всегда последней опцией (продукт не знает golden —
//!    слота под golden нет, берётся топ `n−2` как в harness без golden);
//! 4) сортировка: score убывание, затем ключ лексикографически.
//!
//! Побитовая совместимость — golden-тест `tests/golden_shortlist.rs`
//! (фикстура `shortlist_picked.json`: 226 контекстов evalset).

use std::collections::HashSet;

use crate::catalog::{option_desc, CatalogTemplate, Lang};
use crate::types::OptionDesc;

/// Стоп-слова `_tok` (дословно из harness).
const STOP: &[&str] = &[
    "в", "и", "на", "с", "от", "по", "за", "к", "до", "из", "у", "не", "the", "of", "a", "to",
    "per", "for", "and",
];

/// `_SCHEME_CAT_BONUS` (дословно; категория схемы → бонус категории шаблона).
fn scheme_cat_bonus(scheme_category: &str, category: &str) -> i64 {
    match scheme_category {
        "business" => match category {
            "unit-economics" => 2,
            "product-analytics" => 1,
            _ => 0,
        },
        "architecture" => match category {
            "infra" => 2,
            "capacity" => 2,
            _ => 0,
        },
        "planning" => match category {
            "unit-economics" => 1,
            _ => 0,
        },
        _ => 0,
    }
}

/// `re.sub(r"[^\w\s]", " ", s.lower()).split()` минус стоп-слова.
fn tok(s: &str) -> HashSet<String> {
    s.to_lowercase()
        .chars()
        .map(|c| {
            // \w = alnum + '_', \s = whitespace; прочее → пробел
            if c.is_alphanumeric() || c == '_' || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|w| !STOP.contains(w))
        .map(|w| w.to_owned())
        .collect()
}

/// Описание none-опции (порт `options_payload`; products-режим без golden).
pub fn none_desc(lang: Lang) -> OptionDesc {
    let desc = match lang {
        Lang::Ru => "ни один шаблон не подходит",
        Lang::En => "no template fits",
    };
    OptionDesc::new("none", desc)
}

/// Отобрать ≤`n` ключей шаблонов под контекст (последний — всегда `none`).
/// `n` клампится снизу двумя (top-`n−2` + `none`).
pub fn shortlist_keys(
    context: &str,
    scheme_category: &str,
    templates: &[CatalogTemplate],
    n: usize,
) -> Vec<String> {
    let n = n.max(2);
    let ctx_tok = tok(context);
    let mut scored: Vec<(i64, &str)> = templates
        .iter()
        .map(|t| {
            let mut t_tok = tok(t.name_ru.as_deref().unwrap_or(""));
            t_tok.extend(tok(&t.name_en));
            // имена параметров — сырыми (не токенизируются; порт дословно)
            for p in &t.params {
                t_tok.insert(p.name.clone());
            }
            t_tok.insert(t.key.clone());
            let score = ctx_tok.intersection(&t_tok).count() as i64
                + scheme_cat_bonus(scheme_category, &t.category);
            (score, t.key.as_str())
        })
        .collect();
    // (-score, key): score убывание, ключ возрастание — как sorted(key=lambda)
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    let mut picked: Vec<String> = scored
        .iter()
        .take(n - 2)
        .map(|&(_, k)| k.to_owned())
        .collect();
    picked.push("none".to_owned());
    picked
}

/// Опции по ключам (порт `options_payload`): короткий список → описания
/// в порядке отбора; неизвестный ключ (дрейф реестра) пропускается мягко.
pub fn options_for_keys(
    keys: &[String],
    templates: &[CatalogTemplate],
    lang: Lang,
) -> Vec<OptionDesc> {
    let mut out = Vec::with_capacity(keys.len());
    for k in keys {
        if k == "none" {
            out.push(none_desc(lang));
        } else if let Some(t) = templates.iter().find(|t| t.key == *k) {
            out.push(option_desc(t, lang));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CatalogParam;

    fn tpl(key: &str, name_ru: &str, name_en: &str, category: &str) -> CatalogTemplate {
        CatalogTemplate {
            key: key.to_owned(),
            name_ru: Some(name_ru.to_owned()),
            name_en: name_en.to_owned(),
            description: String::new(),
            description_en: None,
            category: category.to_owned(),
            params: vec![CatalogParam {
                name: "peak_rps".to_owned(),
                unit: None,
                ptype: None,
            }],
        }
    }

    #[test]
    fn tok_strips_punct_and_stopwords() {
        let t = tok("The API-шлюз: 42 rps, и cache!");
        assert!(t.contains("api"));
        assert!(t.contains("шлюз"));
        assert!(t.contains("42"));
        assert!(t.contains("rps"));
        assert!(t.contains("cache"));
        assert!(!t.contains("the"));
        assert!(!t.contains("и"));
        // пунктуация схлопнулась: «api» без дефиса-осколка
        assert!(!t.contains(""));
    }

    #[test]
    fn shortlist_orders_by_overlap_then_key() {
        let templates = vec![
            tpl("lb", "Балансировщик", "Load Balancer", "infra"),
            tpl("cdn", "CDN", "CDN", "infra"),
            tpl("ab-test", "Значимость A/B", "A/B Test", "product-analytics"),
        ];
        let picked = shortlist_keys(
            "load balancer распределяет трафик peak_rps",
            "",
            &templates,
            3,
        );
        // lb: пересечения {load, balancer, peak_rps, lb?} — топ; none — последняя
        assert_eq!(picked.last().map(String::as_str), Some("none"));
        assert_eq!(picked[0], "lb");
        assert_eq!(picked.len(), 2, "n=3 → top-1 + none");
    }

    #[test]
    fn shortlist_category_bonus() {
        let templates = vec![
            tpl("ue-ltv", "LTV", "LTV", "unit-economics"),
            tpl("lb", "Балансировщик", "Load Balancer", "infra"),
        ];
        // контекст без пересечений: бизнес-бонус решает
        let picked = shortlist_keys("что-то непонятное", "business", &templates, 3);
        assert_eq!(picked[0], "ue-ltv");
        // без категории — лексикографический порядок при нулях
        let picked = shortlist_keys("что-то непонятное", "", &templates, 3);
        assert_eq!(picked[0], "lb");
    }

    #[test]
    fn options_for_keys_maps_none_and_templates() {
        let templates = vec![tpl("lb", "Балансировщик", "Load Balancer", "infra")];
        let keys = vec!["lb".to_owned(), "ghost".to_owned(), "none".to_owned()];
        let opts = options_for_keys(&keys, &templates, Lang::Ru);
        assert_eq!(opts.len(), 2, "неизвестный ключ пропущен");
        assert_eq!(opts[0].id, "lb");
        assert_eq!(opts[1].id, "none");
        assert_eq!(opts[1].desc, "ни один шаблон не подходит");
        let opts = options_for_keys(&keys, &templates, Lang::En);
        assert_eq!(opts[1].desc, "no template fits");
    }
}
