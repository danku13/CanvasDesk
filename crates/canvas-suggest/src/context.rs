//! Сериализация контекста «формат А» (порт `serialize.py`, волна 1).
//!
//! Детерминизм: один канвас → одна строка; фиксированный порядок полей,
//! стабильные сортировки (BTreeSet/`sorted`), никаких timestamp/random.
//! Побайтовая совместимость с Python — контракт golden-теста
//! `tests/golden_context.rs` (пары вход→выход сняты с эталонной
//! реализации harness).
//!
//! Формат:
//! ```text
//! [canvas] N nodes; cats: cat(к) …; vars: name=знач, … | -
//! [node] title="…"; editing="…(≤120 кодпоинтов)"
//! [up] "title" out: out1, out2 | имена определяемых
//! [down] "title" needs: need1, need2 | ссылки на чужие переменные | -
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Сырая нода канваса (лёгкий вход; адаптер canvas-core — S3).
#[derive(Debug, Clone, Copy)]
pub struct RawNode<'a> {
    pub id: &'a str,
    pub text: &'a str,
    pub label: Option<&'a str>,
}

/// Сырое ребро канваса.
#[derive(Debug, Clone, Copy)]
pub struct RawEdge<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub from_output: Option<&'a str>,
    pub to_param: Option<&'a str>,
}

/// Разобранная text-нода (порт словаря `parse_node`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedNode {
    pub title: String,
    pub desc: String,
    /// (lhs, rhs) строк-формул `name = выражение`.
    pub formulas: Vec<(String, String)>,
    /// LHS формул — что нода определяет.
    pub defines: Vec<String>,
    /// `a.b` dotted-ссылки в RHS — на что ссылается.
    pub refs: Vec<String>,
}

/// Первый символ имени: `[A-Za-zА-Яа-яЁё_]` (порт `_FORMULA_RE`).
fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || matches!(c, 'А'..='Я' | 'а'..='я' | 'Ё' | 'ё')
}

/// Продолжение имени: `[\w]` — Unicode alnum + `_`
/// (отклонение от Python `\w` только на экзотических категориях Nl/No —
/// в корпусах ru/en отсутствуют).
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Порт `^([A-Za-zА-Яа-яЁё_][\w]*)\s*=\s*(.+)$` на stripped-строке.
fn match_formula(s: &str) -> Option<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() || !is_name_start(chars[0]) {
        return None;
    }
    let mut i = 1;
    while i < chars.len() && is_word_char(chars[i]) {
        i += 1;
    }
    let lhs: String = chars[..i].iter().collect();
    let mut j = i;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j >= chars.len() || chars[j] != '=' {
        return None;
    }
    j += 1;
    while j < chars.len() && chars[j].is_whitespace() {
        j += 1;
    }
    if j >= chars.len() {
        return None; // `(.+)` требует непустой RHS
    }
    let rhs: String = chars[j..].iter().collect();
    Some((lhs, rhs.trim().to_string()))
}

/// Порт `_DOTTED_RE.findall(rhs)`: последовательный неперекрывающийся
/// поиск `имя.имя`; при неудаче на позиции — сдвиг на 1 (как regex-движок).
fn find_dotted(s: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_name_start(chars[i]) {
            let mut j = i + 1;
            while j < chars.len() && is_word_char(chars[j]) {
                j += 1;
            }
            if j + 1 < chars.len() && chars[j] == '.' && is_name_start(chars[j + 1]) {
                let mut k = j + 2;
                while k < chars.len() && is_word_char(chars[k]) {
                    k += 1;
                }
                let a: String = chars[i..j].iter().collect();
                let b: String = chars[j + 1..k].iter().collect();
                out.push((a, b));
                i = k;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Разбор text-ноды: title (1-я непустая строка, иначе label), описание,
/// формулы, определения, ссылки (порт `parse_node`).
pub fn parse_node(text: &str, label: Option<&str>) -> ParsedNode {
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end()).collect();
    let first = lines.first().copied().unwrap_or("").trim();
    let title = if !first.is_empty() {
        first.to_string()
    } else {
        label.unwrap_or("").to_string()
    };

    let mut desc_lines: Vec<String> = Vec::new();
    let mut formulas: Vec<(String, String)> = Vec::new();
    for l in lines.iter().skip(1) {
        let s = l.trim();
        if s.is_empty() {
            continue;
        }
        if let Some((lhs, rhs)) = match_formula(s) {
            formulas.push((lhs, rhs));
        } else {
            desc_lines.push(s.to_string());
        }
    }
    let defines: Vec<String> = formulas.iter().map(|(lhs, _)| lhs.clone()).collect();
    let mut refs: Vec<String> = Vec::new();
    for (_, rhs) in &formulas {
        for (a, b) in find_dotted(rhs) {
            refs.push(format!("{a}.{b}"));
        }
    }
    ParsedNode {
        title,
        desc: desc_lines.join(" "),
        formulas,
        defines,
        refs,
    }
}

/// Что нода отдаёт наружу: имена определённых переменных (или `-`).
pub fn node_output_names(parsed: &ParsedNode) -> String {
    if parsed.defines.is_empty() {
        "-".to_string()
    } else {
        parsed.defines.join(", ")
    }
}

/// Что нода потребляет: dotted-ссылки на переменные, определённые
/// в других нодах (по title), дедуп с сохранением порядка (или `-`).
pub fn node_input_needs(parsed: &ParsedNode, defined_elsewhere: &HashSet<String>) -> String {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut needs: Vec<&str> = Vec::new();
    for r in &parsed.refs {
        let prefix = r.split('.').next().unwrap_or("");
        if defined_elsewhere.contains(prefix) && seen.insert(r.as_str()) {
            needs.push(r.as_str());
        }
    }
    if needs.is_empty() {
        "-".to_string()
    } else {
        needs.join(", ")
    }
}

/// Секции `[canvas]`: категории нод по маппингу lower(title)→категория
/// (в harness — `templates_by_title`; без записи — `misc`).
fn cat_counts(nodes: &[&RawNode], categories: &HashMap<String, String>) -> String {
    let mut cats: BTreeMap<&str, usize> = BTreeMap::new();
    for n in nodes {
        let p = parse_node(n.text, n.label);
        let cat = categories
            .get(&p.title.to_lowercase())
            .map(|s| s.as_str())
            .unwrap_or("misc");
        *cats.entry(cat).or_insert(0) += 1;
    }
    cats.iter()
        .map(|(c, k)| format!("{c}({k})"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Порт fullmatch `[\d\s.,]+(?:руб|rub|\$|€|мес|дн|ч|%|шт)?\.?` (re.I).
/// `\d` аппроксимирован `is_numeric` (отклонение только на Nl/No —
/// в корпусах отсутствуют).
fn is_scalar_rhs(rhs: &str) -> bool {
    const UNITS: [&str; 9] = ["руб", "rub", "$", "€", "мес", "дн", "ч", "%", "шт"];
    let chars: Vec<char> = rhs.chars().collect();
    let mut i = 0;
    while i < chars.len()
        && (chars[i].is_numeric() || chars[i].is_whitespace() || chars[i] == '.' || chars[i] == ',')
    {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    let mut j = i;
    'units: for u in UNITS {
        let uc: Vec<char> = u.chars().collect();
        if j + uc.len() <= chars.len() {
            let window: String = chars[j..j + uc.len()].iter().collect();
            if window.to_lowercase() == u {
                j += uc.len();
                break 'units;
            }
        }
    }
    if j < chars.len() && chars[j] == '.' {
        j += 1;
    }
    j == chars.len()
}

/// Top-level константы вида `name = число [единица]` — для секции vars
/// (первые `limit` по порядку обхода нод и строк).
fn scalar_vars(nodes: &[&RawNode], limit: usize) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for n in nodes {
        for line in n.text.split('\n') {
            if let Some((lhs, rhs)) = match_formula(line.trim()) {
                if is_scalar_rhs(&rhs) {
                    let val = rhs.replace(' ', "");
                    out.push(format!("{lhs}={val}"));
                    if out.len() >= limit {
                        return Some(out.join(", "));
                    }
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(", "))
    }
}

/// Дедуп с сохранением порядка первого вхождения (порт `dict.fromkeys`).
fn dedup_ids<'a>(ids: &'a [&'a str]) -> Vec<&'a str> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for id in ids {
        if seen.insert(id) {
            out.push(id);
        }
    }
    out
}

/// Сериализация контекста вокруг «дырки» `hole_id` (сама нода удалена;
/// nodes/edges — полное множество ДО удаления, фильтрация здесь).
///
/// `categories` — маппинг `lower(title) → категория шаблона` для подсчёта
/// категорий канваса (строится из каталога; в harness — плюс overrides
/// из mapping.json).
pub fn build_context(
    nodes: &[RawNode],
    edges: &[RawEdge],
    hole_id: &str,
    editing: &str,
    hole_title: &str,
    categories: &HashMap<String, String>,
) -> String {
    // соседи дырки (порядок рёбер сохраняется, дедуп first-wins)
    let up_ids: Vec<&str> = edges
        .iter()
        .filter(|e| e.to == hole_id)
        .map(|e| e.from)
        .collect();
    let down_ids: Vec<&str> = edges
        .iter()
        .filter(|e| e.from == hole_id)
        .map(|e| e.to)
        .collect();
    let rest: Vec<&RawNode> = nodes.iter().filter(|n| n.id != hole_id).collect();
    let by_id: HashMap<&str, &RawNode> = rest.iter().map(|n| (n.id, *n)).collect();

    // title → id (в Python dict; здесь нужна только множество title)
    let mut defined_titles: HashSet<String> = HashSet::new();
    for n in &rest {
        let p = parse_node(n.text, n.label);
        if !p.title.is_empty() {
            defined_titles.insert(p.title);
        }
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "[canvas] {} nodes; cats: {}; vars: {}",
        rest.len(),
        cat_counts(&rest, categories),
        scalar_vars(&rest, 6).unwrap_or_else(|| "-".to_string())
    ));
    let editing_cut: String = editing.chars().take(120).collect();
    lines.push(format!(
        "[node] title=\"{}\"; editing=\"{}\"",
        hole_title, editing_cut
    ));

    for nid in dedup_ids(&up_ids) {
        if let Some(n) = by_id.get(nid) {
            let p = parse_node(n.text, n.label);
            let outs: BTreeSet<&str> = edges
                .iter()
                .filter(|e| e.from == nid && e.to == hole_id)
                .filter_map(|e| e.from_output)
                .filter(|o| !o.is_empty())
                .collect();
            let joined = outs.iter().copied().collect::<Vec<_>>().join(", ");
            let val = if joined.is_empty() {
                node_output_names(&p)
            } else {
                joined
            };
            lines.push(format!("[up] \"{}\" out: {}", p.title, val));
        }
    }
    for nid in dedup_ids(&down_ids) {
        if let Some(n) = by_id.get(nid) {
            let p = parse_node(n.text, n.label);
            let needs_set: BTreeSet<&str> = edges
                .iter()
                .filter(|e| e.from == hole_id && e.to == nid)
                .filter_map(|e| match e.to_param {
                    Some(tp) if !tp.is_empty() => Some(tp),
                    _ => e.from_output.filter(|o| !o.is_empty()),
                })
                .collect();
            let mut need = needs_set.iter().copied().collect::<Vec<_>>().join(", ");
            if need.is_empty() {
                need = node_input_needs(&p, &defined_titles);
            }
            if need.is_empty() {
                need = "-".to_string();
            }
            lines.push(format!("[down] \"{}\" needs: {}", p.title, need));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_node_basic() {
        let p = parse_node(
            "Маржа\nваловая = цена - себестоимость\nкомментарий\nдоля = маржа.итог / выручка.год",
            None,
        );
        assert_eq!(p.title, "Маржа");
        assert_eq!(p.desc, "комментарий");
        assert_eq!(
            p.formulas,
            vec![
                ("валовая".to_string(), "цена - себестоимость".to_string()),
                ("доля".to_string(), "маржа.итог / выручка.год".to_string()),
            ]
        );
        assert_eq!(p.defines, ["валовая", "доля"]);
        assert_eq!(p.refs, ["маржа.итог", "выручка.год"]);
    }

    #[test]
    fn parse_node_title_fallback_to_label() {
        let p = parse_node("", Some("Подпись"));
        assert_eq!(p.title, "Подпись");
        assert!(p.formulas.is_empty());
    }

    #[test]
    fn formula_requires_lhs_name() {
        assert!(match_formula("= 5").is_none());
        assert!(match_formula("1x = 5").is_none()); // цифра — не первый символ
        assert!(match_formula("x =").is_none()); // пустой RHS
        assert_eq!(
            match_formula("срок  =  36 мес"),
            Some(("срок".to_string(), "36 мес".to_string()))
        );
    }

    #[test]
    fn dotted_scan_like_regex() {
        let r = find_dotted("a.b + ab1.2x + cd.");
        assert_eq!(r, vec![("a".to_string(), "b".to_string())]);
        let r2 = find_dotted("abc.def.ghi");
        assert_eq!(r2, vec![("abc".to_string(), "def".to_string())]);
    }

    #[test]
    fn scalar_rhs_units() {
        assert!(is_scalar_rhs("10руб"));
        assert!(is_scalar_rhs("10 РУБ")); // re.I
        assert!(is_scalar_rhs("2 000 000"));
        assert!(is_scalar_rhs("36 мес."));
        assert!(is_scalar_rhs("4.5"));
        assert!(is_scalar_rhs("70%"));
        assert!(!is_scalar_rhs("цена - cogs"));
        assert!(!is_scalar_rhs("x1"));
        assert!(!is_scalar_rhs(""));
    }

    #[test]
    fn build_context_format_a() {
        let nodes = vec![
            RawNode {
                id: "unit",
                text: "Продукт\nprice = 10руб\ncogs = 4руб",
                label: None,
            },
            RawNode {
                id: "hole",
                text: "Маржа",
                label: None,
            },
            RawNode {
                id: "ltv",
                text: "LTV\nltv = маржа.итог * 36",
                label: None,
            },
        ];
        let edges = vec![
            RawEdge {
                from: "unit",
                to: "hole",
                from_output: Some("price"),
                to_param: None,
            },
            RawEdge {
                from: "unit",
                to: "hole",
                from_output: Some("cogs"),
                to_param: None,
            },
            RawEdge {
                from: "hole",
                to: "ltv",
                from_output: Some("margin"),
                to_param: None,
            },
        ];
        let mut cats = HashMap::new();
        cats.insert("продукт".to_string(), "unit-economics".to_string());
        cats.insert("ltv".to_string(), "unit-economics".to_string());
        let ctx = build_context(&nodes, &edges, "hole", "", "Маржа", &cats);
        let lines: Vec<&str> = ctx.split('\n').collect();
        assert_eq!(
            lines[0],
            "[canvas] 2 nodes; cats: unit-economics(2); vars: price=10руб, cogs=4руб"
        );
        assert_eq!(lines[1], "[node] title=\"Маржа\"; editing=\"\"");
        assert_eq!(lines[2], "[up] \"Продукт\" out: cogs, price"); // sorted set
                                                                   // у ltv нет toParam-рёбер → потребности из refs по title «маржа»? —
                                                                   // refs «маржа.итог» ссылается на title «маржа», которого нет → '-'
        assert!(lines[3].starts_with("[down] \"LTV\" needs:"));
    }

    #[test]
    fn editing_truncated_to_120_codepoints() {
        let nodes = vec![RawNode {
            id: "a",
            text: "нода",
            label: None,
        }];
        let long: String = "я".repeat(200);
        let ctx = build_context(&nodes, &[], "x", &long, "", &HashMap::new());
        let line = ctx.split('\n').nth(1).unwrap();
        // editing обрезан до 120 кодпоинтов; строка заканчивается закрывающей кавычкой
        assert!(line.contains(&"я".repeat(120)));
        assert!(!line.contains(&"я".repeat(121)));
    }
}
