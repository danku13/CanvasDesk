//! Контракты suggest-движка (FR-079 §2.1 техконцепта).
//!
//! Приложение знает только `Vec<ScoredOption>` — весь ИИ за трейтом
//! [`SuggestEngine`]. Гибрид — не движок, а композиция score-листов в
//! воркере ([`crate::fusion`]), поэтому конфигурируется без кода.

/// Описание кандидата (опции) для ранжирования.
///
/// `desc` — текст опции формата `template_option()` (порт `serialize.py`):
/// `«имя: глосс; params: …»`; для match_variable-задач может быть пустым.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionDesc {
    pub id: String,
    pub desc: String,
}

impl OptionDesc {
    pub fn new(id: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            desc: desc.into(),
        }
    }
}

/// Источник скоринга — показывается пользователю (доверие, гипотеза §15)
/// и пишется в suggest-log.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScoreSource {
    /// Чистая лексика (L0-совместимый режим, деградация sidecar).
    Lex,
    /// Гибридная fusion-оценка.
    Fusion { alpha: f64 },
}

/// Опция с оценкой. Сортировка — по убыванию `score`, при равенстве —
/// по `id` по возрастанию (лексикографически, как в harness PoC).
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredOption {
    pub id: String,
    pub score: f64,
    pub source: ScoreSource,
}

impl ScoredOption {
    pub fn new(id: impl Into<String>, score: f64, source: ScoreSource) -> Self {
        Self {
            id: id.into(),
            score,
            source,
        }
    }
}

/// Вход движка: сериализованный контекст (формат А, [`crate::context`])
/// и список кандидатов (уже отобранных шортлистом или полный каталог).
///
/// Проекционное решение: движки потребляют именно сериализованный документ,
/// а не структуру канваса — это же вход L1 (`state.document` протокола
/// `/v1/systemone`), что гарантирует одинаковый взгляд lex/mm на контекст.
#[derive(Debug, Clone, Copy)]
pub struct SuggestContext<'a> {
    pub document: &'a str,
    pub options: &'a [OptionDesc],
}

impl<'a> SuggestContext<'a> {
    pub fn new(document: &'a str, options: &'a [OptionDesc]) -> Self {
        Self { document, options }
    }
}

/// Трейт движка подсказок. Реализации: [`crate::lex::LexEngine`] (v1/v2).
///
/// Синхронный чистый вызов над контекстом; L1-версия (S2, feature
/// `l1-laya`) делает то же самое через HTTP и возвращает score-лист,
/// который воркер композирует с lex через [`crate::fusion::fuse`].
pub trait SuggestEngine: Send {
    /// Короткое имя для логов/HUD («lex-v1», «lex-v2», …).
    fn name(&self) -> &'static str;

    /// Отранжировать опции контекста. Пустой вход → пустой выход.
    fn rank(&self, ctx: &SuggestContext) -> Vec<ScoredOption>;
}

/// Шортлист: топ-N по ранжированию (для L1 комфорт Laya ~20 опций,
/// PoC-отчёт §15.5: top10 sweet-spot для чистого mm, топ-20 для fusion).
pub fn shortlist(ranked: &[ScoredOption], n: usize) -> &[ScoredOption] {
    let n = n.min(ranked.len());
    &ranked[..n]
}

/// Общий порядок сортировки score-листов: убывание score, затем id
/// по возрастанию. Побитово повторяет `sorted(key=lambda x: (-x[0], x[1]))`
/// harness PoC.
pub fn sort_scored(scored: &mut [ScoredOption]) {
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scored(id: &str, score: f64) -> ScoredOption {
        ScoredOption::new(id, score, ScoreSource::Lex)
    }

    #[test]
    fn sort_desc_then_id_asc() {
        let mut v = vec![scored("b", 1.0), scored("a", 1.0), scored("c", 2.0)];
        sort_scored(&mut v);
        assert_eq!(
            v.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["c", "a", "b"]
        );
    }

    #[test]
    fn shortlist_top_n() {
        let v = vec![scored("a", 3.0), scored("b", 2.0), scored("c", 1.0)];
        assert_eq!(
            shortlist(&v, 2)
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(shortlist(&v, 10).len(), 3);
        assert!(shortlist(&v, 0).is_empty());
    }
}
