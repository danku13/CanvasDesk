//! BM25Okapi — порт `rank_bm25` (v0.2.2, dorianbrown/rank_bm25).
//!
//! Побитовая совместимость с Python важна для golden-тестов lex-движка:
//! порядок f64-суммирования воспроизводит numpy-версию `get_scores`
//! (накопление по query-токенам в порядке подачи), а `idf`-калибровка
//! epsilon проходит в порядке первого появления слов в корпусе — как
//! вставка в dict в Python (итерация HashMap в Rust недетерминирована,
//! поэтому внутри упорядоченный вектор).
//!
//! Параметры как в harness PoC: `k1=1.5`, `b=0.75`, `epsilon=0.25`.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Bm25Okapi {
    k1: f64,
    b: f64,
    doc_len: Vec<usize>,
    avgdl: f64,
    /// слово → idf (после epsilon-пола для отрицательных значений)
    idf: HashMap<String, f64>,
    /// частоты токенов по документам (словарь для O(1)-lookup)
    doc_freqs: Vec<HashMap<String, u32>>,
}

impl Bm25Okapi {
    /// Построить индекс по корпусу. Каждый документ — список токенов
    /// (в harness — отсортированное множество, но порт корректен и для
    /// повторов: doc_len и частоты считаются как в Python).
    pub fn new(corpus: &[Vec<String>]) -> Self {
        let k1 = 1.5;
        let b = 0.75;
        let epsilon = 0.25;

        let mut doc_len: Vec<usize> = Vec::with_capacity(corpus.len());
        let mut doc_freqs: Vec<HashMap<String, u32>> = Vec::with_capacity(corpus.len());
        // nd: слово → число документов с ним; порядок вставки — как в Python
        // (документы по порядку, слова в порядке первого появления в документе)
        let mut nd_order: Vec<String> = Vec::new();
        let mut nd: HashMap<String, u32> = HashMap::new();
        let mut num_doc: usize = 0;

        for doc in corpus {
            doc_len.push(doc.len());
            num_doc += doc.len();
            let mut freqs: HashMap<String, u32> = HashMap::with_capacity(doc.len());
            let mut seen_in_doc: Vec<&str> = Vec::new();
            for w in doc {
                match freqs.get_mut(w) {
                    Some(c) => *c += 1,
                    None => {
                        freqs.insert(w.clone(), 1);
                        seen_in_doc.push(w);
                    }
                }
            }
            for w in seen_in_doc {
                match nd.get_mut(w) {
                    Some(c) => *c += 1,
                    None => {
                        nd.insert(w.to_string(), 1);
                        nd_order.push(w.to_string());
                    }
                }
            }
            doc_freqs.push(freqs);
        }

        let corpus_size = corpus.len();
        let avgdl = if corpus_size == 0 {
            0.0
        } else {
            num_doc as f64 / corpus_size as f64
        };

        // _calc_idf: idf = log(N - df + 0.5) - log(df + 0.5);
        // отрицательные → eps = epsilon * average_idf (порядок суммирования
        // = порядок первого появления слова, как nd.items() в Python)
        let mut idf: HashMap<String, f64> = HashMap::with_capacity(nd.len());
        let mut idf_sum: f64 = 0.0;
        let mut negative: Vec<String> = Vec::new();
        for w in &nd_order {
            let freq = nd[w];
            let v = (corpus_size as f64 - freq as f64 + 0.5).ln() - (freq as f64 + 0.5).ln();
            idf.insert(w.clone(), v);
            idf_sum += v;
            if v < 0.0 {
                negative.push(w.clone());
            }
        }
        if !nd_order.is_empty() {
            let average_idf = idf_sum / nd_order.len() as f64;
            let eps = epsilon * average_idf;
            for w in negative {
                idf.insert(w, eps);
            }
        }

        Self {
            k1,
            b,
            doc_len,
            avgdl,
            idf,
            doc_freqs,
        }
    }

    /// Скоринг запроса: по токенам запроса в порядке подачи накапливает
    /// вклад в каждый документ (тот же порядок операций, что в numpy-
    /// версии `get_scores` — важно для побитовой совместимости f64).
    ///
    /// Вырожденные случаи (пустой корпус, avgdl = 0): все нули
    /// (в Python здесь NaN — отклонение зафиксировано сознательно:
    /// движок не должен возвращать NaN в продукт).
    pub fn get_scores(&self, query: &[String]) -> Vec<f64> {
        let mut score = vec![0.0; self.doc_freqs.len()];
        if self.avgdl <= 0.0 {
            return score;
        }
        for q in query {
            let idf = *self.idf.get(q).unwrap_or(&0.0);
            for ((freqs, dl), s) in self.doc_freqs.iter().zip(&self.doc_len).zip(&mut score) {
                let qf = *freqs.get(q).unwrap_or(&0) as f64;
                if qf == 0.0 {
                    continue; // вклад ровно ноль
                }
                *s += idf
                    * (qf * (self.k1 + 1.0)
                        / (qf + self.k1 * (1.0 - self.b + self.b * *dl as f64 / self.avgdl)));
            }
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs(specs: &[&[&str]]) -> Vec<Vec<String>> {
        specs
            .iter()
            .map(|d| d.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn empty_corpus_and_query() {
        let bm = Bm25Okapi::new(&[]);
        assert!(bm.get_scores(&[]).is_empty());
        let bm2 = Bm25Okapi::new(&docs(&[&["a", "b"], &["b"]]));
        assert_eq!(bm2.get_scores(&[]), vec![0.0, 0.0]);
    }

    #[test]
    fn degenerate_all_empty_docs() {
        // avgdl = 0 → нули (в Python NaN; отклонение задокументировано)
        let bm = Bm25Okapi::new(&docs(&[&[], &[]]));
        assert_eq!(bm.get_scores(&["x".to_string()]), vec![0.0, 0.0]);
    }

    #[test]
    fn rare_word_beats_common() {
        // «уникаль» встречается в 1 документе из 2 → выше idf
        let bm = Bm25Okapi::new(&docs(&[&["уникаль", "общее"], &["общее"]]));
        let s = bm.get_scores(&["уникаль".to_string(), "общее".to_string()]);
        assert!(s[0] > s[1], "s = {s:?}");
    }

    #[test]
    fn negative_idf_floored_to_eps() {
        // «общее» в 2 документах из 3 → отрицательный idf → пол eps.
        // Как и в rank_bm25, eps = 0.25 · average_idf может быть любого
        // знака; здесь average_idf > 0 → пол положительный.
        let bm = Bm25Okapi::new(&docs(&[&["общее", "раз"], &["общее", "два"], &["иное"]]));
        let s = bm.get_scores(&["общее".to_string()]);
        assert!(s[0] > 0.0 && s[1] > 0.0, "s = {s:?}");
    }
}
