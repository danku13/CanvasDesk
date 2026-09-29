//! LexEngine v1 — лексический baseline (порт `baseline_lex.py`, волна 1,
//! тег `baseline-frozen`).
//!
//! Ранжирование опций против контекста: `0.7·BM25 + 0.3·char3-жаккард`.
//! Воспроизведение на замороженном eval-сете (main-фильтр): p@1
//! test 15/47 (0.3191), dev 38/92 (0.4130), pool 53/139 (0.3813) —
//! см. `tests/golden_lex.rs`.
//!
//! Это единственный lex-компонент гибрида: fusion(lex v1, mm) = 0.468
//! сильнее fusion(lex v2, mm) = 0.4255, потому что v2 чинит те ошибки,
//! которые mm и так чинил сам (урок декорреляции, волна 3 §3).

use crate::bm25::Bm25Okapi;
use crate::tokenizer::{char3, tokens};
use crate::types::{OptionDesc, ScoreSource, ScoredOption, SuggestContext, SuggestEngine};
use std::collections::BTreeSet;

/// Лексический движок v1: BM25 по токенам опций + char3-жаккард.
#[derive(Debug, Clone)]
pub struct LexEngine {
    /// Вес BM25-компоненты (harness: 0.7).
    pub w_lex: f64,
    /// Вес char3-жаккарда (harness: 0.3).
    pub w_char: f64,
}

impl Default for LexEngine {
    fn default() -> Self {
        Self {
            w_lex: 0.7,
            w_char: 0.3,
        }
    }
}

impl LexEngine {
    pub fn new(w_lex: f64, w_char: f64) -> Self {
        Self { w_lex, w_char }
    }

    /// Score-лист опций против документа (id → score). Пустые опции → пустой
    /// выход. Публично для [`crate::fusion`]: гибриду нужны сырые lex-скоры.
    pub fn scores(&self, document: &str, options: &[OptionDesc]) -> Vec<(String, f64)> {
        if options.is_empty() {
            return Vec::new();
        }
        let ctx_toks = tokens(document);
        // корпус «документов» = опции: sorted(tokens(desc) | tokens(id))
        let docs: Vec<Vec<String>> = options
            .iter()
            .map(|o| {
                let mut s = tokens(&o.desc);
                s.extend(tokens(&o.id));
                // BTreeSet уже даёт сортировку объединения
                s.into_iter().collect::<BTreeSet<_>>().into_iter().collect()
            })
            .collect();
        let bm25 = Bm25Okapi::new(&docs);
        let q: Vec<String> = ctx_toks.into_iter().collect();
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
                (o.id.clone(), self.w_lex * scores[i] + self.w_char * jac)
            })
            .collect()
    }
}

impl SuggestEngine for LexEngine {
    fn name(&self) -> &'static str {
        "lex-v1"
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
    fn empty_options() {
        let e = LexEngine::default();
        assert!(e.scores("контекст", &[]).is_empty());
        assert!(e.rank(&SuggestContext::new("контекст", &[])).is_empty());
    }

    #[test]
    fn keyword_match_wins() {
        let e = LexEngine::default();
        let opts = vec![
            OptionDesc::new("ue-cac", "CAC (стоимость привлечения клиента): привлечение"),
            OptionDesc::new("ue-gross-margin", "Маржа (margin): маржинальность"),
            OptionDesc::new("pa-retention", "Удержание (retention): cohorts"),
        ];
        let ranked = e.rank(&SuggestContext::new(
            "[canvas] 3 nodes; cats: unit-economics(3)\n[node] title=\"Маржа\"\n[up] \"Продукт\" out: cogs, price",
            &opts,
        ));
        assert_eq!(ranked[0].id, "ue-gross-margin");
    }
}
