//! Каталог шаблонов → опции ранжирования (порт `serialize.py:
//! template_option` + `templates_by_title`, волна 1).
//!
//! Побитовая совместимость с Python — контракт golden-теста
//! `tests/golden_catalog.rs` (фикстура `catalog_options.json` снята с
//! эталонного harness, 62 шаблона, оба языка).
//!
//! Развязка от canvas-core: адаптер приложения (S3) строит
//! [`CatalogTemplate`] из манифестов реестра — сам крейт знает только
//! плоские поля. [`Lang`] — своя пара значений (не canvas_core::Language):
//! ядро движка не зависит от продукта.

use std::collections::HashMap;

use crate::types::OptionDesc;

/// Язык описания опции (зеркалит `lang` из harness).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

/// Параметр шаблона для описания опции (поля манифеста, нужные порту).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogParam {
    pub name: String,
    pub unit: Option<String>,
    pub ptype: Option<String>,
}

/// Шаблон каталога в плоской форме (адаптер манифеста — на стороне
/// приложения; здесь только данные).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogTemplate {
    /// Короткий ключ (`com.canvasdesk.lb` → `lb`).
    pub key: String,
    pub name_ru: Option<String>,
    pub name_en: String,
    /// Описание (RU) — источник глосса для ru-опций.
    pub description: String,
    pub description_en: Option<String>,
    pub category: String,
    pub params: Vec<CatalogParam>,
}

/// `re.split(r"[^\w]+", s)` минус пустые: слова из букв/цифр/`_`
/// (Unicode, как Python `\w` для str).
fn word_set(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_owned())
        .collect()
}

/// `re.sub(r"[^\w]", "", w.lower())` — вычистить не-словесные символы.
fn clean_word(w: &str) -> String {
    w.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Описание опции формата `«имя: глосс; params: …»` (порт `template_option`
/// побитово; `max_desc` = 110 как в harness).
///
/// Глосс: первое предложение description, где ≥ половины слов — новые
/// (не из имени); кандидаты — куски по `.``;`; берётся до 6 слов.
pub fn option_desc(t: &CatalogTemplate, lang: Lang) -> OptionDesc {
    let name = match lang {
        Lang::Ru => t.name_ru.clone().unwrap_or_else(|| t.key.clone()),
        Lang::En => {
            if t.name_en.is_empty() {
                t.key.clone()
            } else {
                t.name_en.clone()
            }
        }
    };
    let desc = match lang {
        Lang::Ru => t.description.trim().to_owned(),
        Lang::En => t.description_en.as_deref().unwrap_or("").trim().to_owned(),
    };
    let name_words = word_set(&name);
    let mut gloss = String::new();
    for clause in desc.split(['.', ';']) {
        let words: Vec<&str> = clause.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }
        let new_share = words
            .iter()
            .filter(|w| !name_words.contains(&clean_word(w)))
            .count() as f64
            / words.len() as f64;
        gloss = words.iter().take(6).cloned().collect::<Vec<_>>().join(" ");
        if new_share >= 0.5 {
            break;
        }
    }
    let params = t
        .params
        .iter()
        .take(3)
        .map(|p| {
            let u = match p.unit.as_deref() {
                Some(u) if !u.is_empty() => u.to_owned(),
                _ => p.ptype.clone().unwrap_or_else(|| "?".to_owned()),
            };
            format!("{}({})", p.name, u)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut text = if gloss.is_empty() {
        name.clone()
    } else {
        format!("{}: {}", name, gloss)
    };
    if !params.is_empty() {
        text.push_str("; params: ");
        text.push_str(&params);
    }
    // Python `text[:110]` — срез по кодпоинтам
    let truncated: String = text.chars().take(110).collect();
    OptionDesc::new(t.key.clone(), truncated)
}

/// `_title_key`: `(title or "").strip().lower()`.
pub fn title_key(title: &str) -> String {
    title.trim().to_lowercase()
}

/// `templates_by_title`: `title_key(name) → категория` для подсчёта
/// категорий канваса в [`crate::context::build_context`] (первый
/// одноимённый шаблон выигрывает — setdefault-семантика Python).
pub fn categories_by_title(templates: &[CatalogTemplate]) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for t in templates {
        for cand in [t.name_ru.as_deref().unwrap_or(""), t.name_en.as_str()] {
            let k = title_key(cand);
            if !k.is_empty() {
                m.entry(k).or_insert_with(|| t.category.clone());
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tpl() -> CatalogTemplate {
        CatalogTemplate {
            key: "lb".to_owned(),
            name_ru: Some("Балансировщик".to_owned()),
            name_en: "Load Balancer".to_owned(),
            description: "Балансировщик: распределяет трафик. Считает utilization.".to_owned(),
            description_en: Some("Load balancer: distributes traffic.".to_owned()),
            category: "network".to_owned(),
            params: vec![
                CatalogParam {
                    name: "rps".to_owned(),
                    unit: Some("rps".to_owned()),
                    ptype: None,
                },
                CatalogParam {
                    name: "nodes".to_owned(),
                    unit: None,
                    ptype: Some("count".to_owned()),
                },
            ],
        }
    }

    #[test]
    fn option_desc_ru_gloss_and_params() {
        let d = option_desc(&tpl(), Lang::Ru);
        // Кандидаты по '.': ["Балансировщик: распределяет трафик", " Считает utilization", ""]
        // Глосс — первые 6 слов куска ДО чистки (двоеточие остаётся — поведение
        // Python, ср. фикстуру «API-шлюз: API-шлюз: …»); new_share 2/3 ≥ 0.5 → break
        assert_eq!(
            d.desc,
            "Балансировщик: Балансировщик: распределяет трафик; params: rps(rps), nodes(count)"
        );
        assert_eq!(d.id, "lb");
    }

    #[test]
    fn option_desc_en() {
        let d = option_desc(&tpl(), Lang::En);
        assert_eq!(
            d.desc,
            "Load Balancer: Load balancer: distributes traffic; params: rps(rps), nodes(count)"
        );
    }

    #[test]
    fn option_desc_no_gloss_falls_back_to_name() {
        let mut t = tpl();
        t.description = "Балансировщик.".to_owned();
        // Единственный кусок «Балансировщик» — слово совпадает с именем → new_share 0
        // но gloss всё равно присваивается (порт: присваивание до break-условия)
        let d = option_desc(&t, Lang::Ru);
        assert_eq!(
            d.desc,
            "Балансировщик: Балансировщик; params: rps(rps), nodes(count)"
        );
    }

    #[test]
    fn option_desc_truncates_110_codepoints() {
        let mut t = tpl();
        t.description = "а".repeat(300);
        let d = option_desc(&t, Lang::Ru);
        assert!(d.desc.chars().count() <= 110);
    }

    #[test]
    fn categories_first_wins() {
        let mut a = tpl();
        a.name_ru = Some("Штука".to_owned());
        let mut b = tpl();
        b.key = "other".to_owned();
        b.category = "custom".to_owned();
        b.name_ru = Some("Штука".to_owned());
        let m = categories_by_title(&[a, b]);
        assert_eq!(m.get("штука").map(String::as_str), Some("network"));
    }
}
