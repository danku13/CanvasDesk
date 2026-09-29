//! Score-fusion гибрида + Platt-калибровка (FR-079 §2.5, волна 2 §15.1).
//!
//! `s = α·norm(lex) + (1−α)·norm(mm)`, α = 0.85 (dev-fit волны 2–3),
//! нормализация min-max по кандидатам фикстуры. Кандидаты — объединение
//! ключей lex и mm: у mm бывает опция `none` (none-зона «не предлагать»),
//! у lex её нет → скор 0.0. Отсутствующие у mm опции → 0.0.
//!
//! Воспроизведение на замороженных mm-пробах (main-фильтр): p@1
//! test 22/47 (0.4681), dev 39/92, pool 61/139 — см. `tests/golden_fusion.rs`.
//!
//! Platt-калибровка: `confidence' = sigmoid(a·c + b)`, a=−0.018, b=−0.814
//! (волна 3 §7: ECE test 0.0986 ≤ 0.15). Confidence используется только для
//! HUD/логов/сортировки, НЕ для show-гейта (нейро-гейтинг недопустим до
//! fine-tune — волна 3 §7).

use crate::types::{ScoreSource, ScoredOption};
use std::collections::HashMap;

/// Коэффициенты Platt-скейлинга (волна 3, ECE 0.099; после H8 — re-fit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Platt {
    pub a: f64,
    pub b: f64,
}

impl Default for Platt {
    fn default() -> Self {
        Self {
            a: -0.018,
            b: -0.814,
        }
    }
}

impl Platt {
    /// sigmoid(a·confidence + b).
    pub fn scale(&self, confidence: f64) -> f64 {
        1.0 / (1.0 + (-(self.a * confidence + self.b)).exp())
    }
}

/// Min-max нормализация значений. Вырожденный случай (все значения равны)
/// → все 0.0 (реконструкция волны 2; NaN-поведение Python отброшено).
pub fn min_max(values: &HashMap<String, f64>) -> HashMap<String, f64> {
    if values.is_empty() {
        return HashMap::new();
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in values.values() {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    if hi - lo < 1e-12 {
        return values.keys().map(|k| (k.clone(), 0.0)).collect();
    }
    values
        .iter()
        .map(|(k, v)| (k.clone(), (v - lo) / (hi - lo)))
        .collect()
}

/// Слить два score-листа (lex и mm) fusion-оценкой.
///
/// Кандидаты — объединение ключей; отсутствующее значение = 0.0
/// (у mm — опции без prob, у lex — `none`). Сортировка: score по убыванию,
/// id по возрастанию.
pub fn fuse(lex: &[(String, f64)], mm: &[(String, f64)], alpha: f64) -> Vec<ScoredOption> {
    let mut cands: HashMap<String, (f64, f64)> = HashMap::new();
    for (k, v) in lex {
        cands.insert(k.clone(), (*v, 0.0));
    }
    for (k, v) in mm {
        match cands.get_mut(k) {
            Some(pair) => pair.1 = *v,
            None => {
                cands.insert(k.clone(), (0.0, *v));
            }
        }
    }
    if cands.is_empty() {
        return Vec::new();
    }
    let lex_v: HashMap<String, f64> = cands.iter().map(|(k, (l, _))| (k.clone(), *l)).collect();
    let mm_v: HashMap<String, f64> = cands.iter().map(|(k, (_, m))| (k.clone(), *m)).collect();
    let ln = min_max(&lex_v);
    let mn = min_max(&mm_v);
    // ln и mn построены по одному множеству ключей (cands)
    let mut out: Vec<ScoredOption> = ln
        .iter()
        .map(|(k, l)| {
            let s = alpha * l + (1.0 - alpha) * mn[k];
            ScoredOption::new(k, s, ScoreSource::Fusion { alpha })
        })
        .collect();
    crate::types::sort_scored(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platt_default_inverts_overconfidence() {
        let p = Platt::default();
        let lo = p.scale(0.0);
        let hi = p.scale(1.0);
        // b < 0 → обе оценки ниже 0.5 (mm zero-shot переуверен)
        assert!(lo < 0.5 && hi < 0.5);
        // a < 0 → наклон отрицательный: высокая уверенность mm СНИЖАЕТСЯ
        // калибровкой сильнее низкой (волна 3 §7: поправка переуверенности)
        assert!(hi < lo, "lo={lo}, hi={hi}");
        assert!((lo - 1.0 / (1.0 + 0.814_f64.exp())).abs() < 1e-12);
    }

    #[test]
    fn min_max_basic() {
        let mut v = HashMap::new();
        v.insert("a".to_string(), 2.0);
        v.insert("b".to_string(), 4.0);
        let n = min_max(&v);
        assert_eq!(n["a"], 0.0);
        assert_eq!(n["b"], 1.0);
    }

    #[test]
    fn min_max_degenerate_all_zero() {
        let mut v = HashMap::new();
        v.insert("a".to_string(), 1.0);
        v.insert("b".to_string(), 1.0);
        let n = min_max(&v);
        assert_eq!(n["a"], 0.0);
        assert_eq!(n["b"], 0.0);
    }

    #[test]
    fn fuse_union_and_none() {
        let lex = vec![("a".to_string(), 1.0), ("b".to_string(), 0.5)];
        let mm = vec![("b".to_string(), 0.8), ("none".to_string(), 0.2)];
        let fused = fuse(&lex, &mm, 0.85);
        // кандидаты = {a, b, none}
        assert_eq!(fused.len(), 3);
        // a: lex-norm 1, mm-norm 0 → 0.85; b: 0.15·1... проверим порядок
        let ids: Vec<&str> = fused.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"a") && ids.contains(&"b") && ids.contains(&"none"));
        // a выше none (none: lex 0, mm 0 → 0)
        let find = |id: &str| fused.iter().find(|s| s.id == id).unwrap().score;
        assert!(find("a") > find("none"));
    }

    #[test]
    fn fuse_empty() {
        assert!(fuse(&[], &[], 0.85).is_empty());
    }
}
