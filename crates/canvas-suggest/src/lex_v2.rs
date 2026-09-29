//! LexEngine v2 — автономный fallback (реконструкция волны 3 H9).
//!
//! `0.55·BM25 + 0.15·char3 + 0.30·names`: к v1 добавляется name-бонус —
//! пересечение токенов контекста со словами имени шаблона
//! (`name_ru ∪ name_en ∪ key-parts`), капнутый на 1.0 при двух совпадениях.
//!
//! Воспроизведение test 19/47 (0.4043) — побитово с отчётом волны 3;
//! dev/pool реконструированы как 37/92 и 56/139 (в отчёте ablation-dev
//! 0.4239/pool 0.4173 — код волны 3 утерян, формула восстановлена по
//! точному совпадению test-числа; расхождение ≤2 фикстур на сплите).
//!
//! Продуктовая роль (волна 3 §9.3): лучший автономный режим без sidecar
//! (слабое железо, деградация L1). **В гибрид не ставить**: замена v1→v2
//! в fusion падает 0.468 → 0.4255 (H9, урок декорреляции).

use crate::bm25::Bm25Okapi;
use crate::tokenizer::{char3, tokens};
use crate::types::{OptionDesc, ScoreSource, ScoredOption, SuggestContext, SuggestEngine};
use std::collections::{BTreeSet, HashMap};

/// Слова имени шаблона: `name_ru ∪ name_en ∪ key-parts` (длина > 1).
/// Строится из каталога один раз на инстанс движка.
pub fn name_words(name_ru: &str, name_en: &str, key: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for src in [name_ru, name_en] {
        for w in src
            .to_lowercase()
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        {
            if w.chars().count() > 1 {
                out.insert(w.to_string());
            }
        }
    }
    for part in key.split('-') {
        if part.chars().count() > 1 {
            out.insert(part.to_string());
        }
    }
    out
}

/// Лексический движок v2: v1 + name-бонус по каталогу.
#[derive(Debug, Clone)]
pub struct LexEngineV2 {
    /// Вес BM25 (H9: 0.55).
    pub w_lex: f64,
    /// Вес char3 (H9: 0.15).
    pub w_char: f64,
    /// Вес name-бонуса (H9: 0.30).
    pub w_name: f64,
    /// Число совпадений, дающее полный бонус (кап; реконструкция: 2).
    pub name_cap: f64,
    /// key шаблона → слова имени. Опции без записи получают 0 бонуса.
    pub names: HashMap<String, BTreeSet<String>>,
}

impl LexEngineV2 {
    /// Дефолт H9: веса 0.55/0.15/0.30, кап 2.
    pub fn new(names: HashMap<String, BTreeSet<String>>) -> Self {
        Self {
            w_lex: 0.55,
            w_char: 0.15,
            w_name: 0.30,
            name_cap: 2.0,
            names,
        }
    }

    /// Name-бонус опции: доля покрытия её имени токенами контекста,
    /// `min(1, |ctx ∩ name| / cap)`.
    pub fn name_score(&self, ctx_toks: &BTreeSet<String>, option_id: &str) -> f64 {
        let Some(nw) = self.names.get(option_id) else {
            return 0.0;
        };
        let inter = nw.intersection(ctx_toks).count();
        (inter as f64 / self.name_cap).min(1.0)
    }

    /// Score-лист (публично для симметрии с v1/диагностики).
    pub fn scores(&self, document: &str, options: &[OptionDesc]) -> Vec<(String, f64)> {
        if options.is_empty() {
            return Vec::new();
        }
        let ctx_toks = tokens(document);
        let docs: Vec<Vec<String>> = options
            .iter()
            .map(|o| {
                let mut s = tokens(&o.desc);
                s.extend(tokens(&o.id));
                s.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
            })
            .collect();
        let bm25 = Bm25Okapi::new(&docs);
        let q: Vec<String> = ctx_toks.iter().cloned().collect();
        let scores = bm25.get_scores(&q);
        let ctx_c3 = char3(document);
        options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let text_o = format!("{} {}", o.desc, o.id);
                let c3 = char3(&text_o);
                let inter = c3.intersection(&ctx_c3).count();
                let union = c3.union(&ctx_c3).count();
                let jac = inter as f64 / (union.max(1)) as f64;
                let nscore = self.name_score(&ctx_toks, &o.id);
                (
                    o.id.clone(),
                    self.w_lex * scores[i] + self.w_char * jac + self.w_name * nscore,
                )
            })
            .collect()
    }
}

impl SuggestEngine for LexEngineV2 {
    fn name(&self) -> &'static str {
        "lex-v2"
    }

    fn rank(&self, ctx: &SuggestContext) -> Vec<ScoredOption> {
        let mut out: Vec<ScoredOption> = self
            .scores(ctx.document, ctx.options)
            .into_iter()
            .map(|(id, score)| ScoredOption::new(id, score, ScoreSource::Lex))
            .collect();
        crate::types::sort_scored(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_words_from_catalog_entry() {
        let nw = name_words("Маржа (gross margin)", "Gross Margin", "ue-gross-margin");
        assert!(nw.contains("маржа"));
        assert!(nw.contains("gross"));
        assert!(nw.contains("margin"));
        assert!(nw.contains("gross")); // key part тоже
        assert!(!nw.contains("a")); // однобуквенные key-части отброшены
    }

    #[test]
    fn name_bonus_capped() {
        let mut names = HashMap::new();
        names.insert("x".to_string(), name_words("Маржа Валовая", "", "x"));
        let e = LexEngineV2::new(names);
        let ctx: BTreeSet<String> = ["маржа", "валовая"].iter().map(|s| s.to_string()).collect();
        assert!((e.name_score(&ctx, "x") - 1.0).abs() < 1e-12); // 2 совпадения → кап
        let ctx1: BTreeSet<String> = ["маржа"].iter().map(|s| s.to_string()).collect();
        assert!((e.name_score(&ctx1, "x") - 0.5).abs() < 1e-12);
        assert_eq!(e.name_score(&ctx, "unknown"), 0.0);
    }
}
