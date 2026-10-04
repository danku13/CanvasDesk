//! FR-LLM-C / PRD-0010 F-2.8 (Q2 prefetch): контекстный кэш для LLM mm-source.
//!
//! Кэш ответов `ChoiceAnswer` по хешу контекста (document + options).
//! TTL 5 мин (PRD-0010 F-2.8) — типичный suggest-сценарий: пользователь
//! правит ноду, через 2-3 минуты возвращается к той же — ответ уже готов.
//!
//! ## Хеш
//!
//! `DefaultHasher` (std) — достаточно для in-memory кэша одного процесса;
//! между запусками кэш не сохраняется (нет persistent storage).
//! Ключевое поле: `document` (redacted) + все `options[i].id` + `.desc`.
//! При изменении опций (например, новый шаблон в каталоге) — хеш меняется,
//! кэш-промах → новый запрос.
//!
//! ## Eviction
//!
//! Ленивая эвикция: при `insert` удаляем все протухшие записи. Этого
//! достаточно для типичной нагрузки suggest (1-10 записей в минуту);
//! полноценный LRU — оверкилл для single-process воркера.
//!
//! ## TTL
//!
//! 5 минут (300 сек). Если пользователь вернётся к ноде позже — кэш-промах,
//! новый запрос. Это баланс между stale-данными (модель могла обновиться)
//! и экономией cost (Q4 — re-request стоит денег).

// FR-LLM-C: маркер для поиска (grep).

use crate::types::OptionDesc;
use canvas_llm::ChoiceAnswer;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::time::Duration;
// W1 (wasm-порт §2 п.7): std::time::Instant паникует на
// wasm32-unknown-unknown; web-time на нативе — прозрачная обёртка над
// std, на wasm32 читает performance.now().
use web_time::Instant;

/// In-memory кэш ответов LLM по хешу контекста (Q2 prefetch, F-2.8).
///
/// TTL по умолчанию — 5 минут (300 сек, PRD-0010 F-2.8). Записи старше
/// TTL считаются протухшими и удаляются при следующем `insert` (lazy
/// eviction). Хеш-ключ — `DefaultHasher` от (document, options) —
/// стабилен в рамках процесса (между запусками не сохраняется).
pub struct ContextCache {
    /// Кэшированные записи: (ответ, время вставки).
    entries: HashMap<u64, (ChoiceAnswer, Instant)>,
    /// TTL — время жизни записи (5 мин по умолчанию).
    ttl: Duration,
}

impl ContextCache {
    /// Новый кэш с заданным TTL. Для suggest — `Duration::from_secs(300)`
    /// (PRD-0010 F-2.8).
    pub fn new(ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            ttl,
        }
    }

    /// Хеш контекста: document + все опции (id + desc). Стабилен в рамках
    /// процесса (DefaultHasher); меняется при любой правке контекста →
    /// кэш-промах → новый LLM-запрос.
    ///
    /// `document` — уже redacted (если Cloud mode); для одинакового redacted
    /// контекста хеш совпадает → кэш-хит.
    pub fn hash(&self, document: &str, options: &[OptionDesc]) -> u64 {
        let mut h = DefaultHasher::new();
        document.hash(&mut h);
        // Разделитель document и options — чтобы `"ab"+"cd"` и `"a"+"bcd"`
        // не совпали (хотя на практике document всегда заканчивается \n).
        0xFFu8.hash(&mut h);
        for o in options {
            o.id.hash(&mut h);
            0u8.hash(&mut h); // разделитель id/desc внутри опции
            o.desc.hash(&mut h);
            1u8.hash(&mut h); // разделитель между опциями
        }
        h.finish()
    }

    /// Получить кэшированный ответ по ключу, если он есть и не протух.
    /// Протухшие записи удаляются (lazy eviction в момент чтения).
    pub fn get(&mut self, key: &u64) -> Option<ChoiceAnswer> {
        let ttl = self.ttl;
        // Проверяем TTL; если протух — удаляем и возвращаем None.
        let expired = self
            .entries
            .get(key)
            .is_some_and(|(_, ts)| ts.elapsed() > ttl);
        if expired {
            self.entries.remove(key);
            return None;
        }
        self.entries.get(key).map(|(ans, _)| ans.clone())
    }

    /// Вставить ответ в кэш. Заодно — ленивая эвикция протухших записей
    /// (чтобы кэш не росл бесконечно при долгой сессии).
    pub fn insert(&mut self, key: u64, answer: ChoiceAnswer) {
        // Ленивая эвикция: удаляем все протухшие записи (TTL истёк).
        let ttl = self.ttl;
        self.entries.retain(|_, (_, ts)| ts.elapsed() <= ttl);
        self.entries.insert(key, (answer, Instant::now()));
    }

    /// Число записей в кэше (для диагностики / HUD).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Кэш пуст.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Очистить кэш (например, при смене провайдера / модели — старые
    /// ответы могли быть от другой модели, не валидны для нового).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(ids: &[&str]) -> Vec<OptionDesc> {
        ids.iter()
            .map(|id| OptionDesc::new(*id, format!("desc for {id}")))
            .collect()
    }

    fn ans(best: &str) -> ChoiceAnswer {
        ChoiceAnswer {
            probs: vec![(best.to_string(), 0.9)],
            confidence: 0.9,
        }
    }

    #[test]
    fn hash_stable_for_same_input() {
        let c = ContextCache::new(Duration::from_secs(300));
        let opts = opts(&["a", "b"]);
        let h1 = c.hash("doc1", &opts);
        let h2 = c.hash("doc1", &opts);
        assert_eq!(h1, h2, "одинаковый вход → одинаковый хеш");
    }

    #[test]
    fn hash_differs_on_document() {
        let c = ContextCache::new(Duration::from_secs(300));
        let opts = opts(&["a", "b"]);
        let h1 = c.hash("doc1", &opts);
        let h2 = c.hash("doc2", &opts);
        assert_ne!(h1, h2, "разный document → разный хеш");
    }

    #[test]
    fn hash_differs_on_options() {
        let c = ContextCache::new(Duration::from_secs(300));
        let opts1 = opts(&["a", "b"]);
        let opts2 = opts(&["a", "c"]);
        let h1 = c.hash("doc", &opts1);
        let h2 = c.hash("doc", &opts2);
        assert_ne!(h1, h2, "разные options → разный хеш");
    }

    #[test]
    fn hash_differs_on_option_desc() {
        let c = ContextCache::new(Duration::from_secs(300));
        let opts1 = vec![OptionDesc::new("a", "desc A")];
        let opts2 = vec![OptionDesc::new("a", "desc B")];
        let h1 = c.hash("doc", &opts1);
        let h2 = c.hash("doc", &opts2);
        assert_ne!(h1, h2, "разный desc → разный хеш");
    }

    #[test]
    fn insert_then_get_hit() {
        let mut c = ContextCache::new(Duration::from_secs(300));
        let opts = opts(&["a", "b"]);
        let key = c.hash("doc", &opts);
        c.insert(key, ans("a"));
        assert_eq!(c.len(), 1);
        let got = c.get(&key).expect("кэш-хит сразу после insert");
        assert_eq!(got.best(), Some("a"));
    }

    #[test]
    fn get_miss_for_unknown_key() {
        let mut c = ContextCache::new(Duration::from_secs(300));
        c.insert(42, ans("a"));
        assert!(c.get(&999).is_none(), "нет такого ключа → None");
    }

    #[test]
    fn get_miss_for_expired() {
        // TTL = 0 — любая запись сразу протухшая.
        let mut c = ContextCache::new(Duration::from_secs(0));
        c.insert(42, ans("a"));
        // Спим минимум — Instant не имеет sleep в std, но elapsed() > 0
        // достаточно (TTL = 0 → elapsed() >= 0 всегда протух).
        std::thread::sleep(Duration::from_millis(1));
        assert!(c.get(&42).is_none(), "TTL=0 → протухшая → None");
        assert_eq!(c.len(), 0, "протухшая запись удалена из кэша");
    }

    #[test]
    fn insert_evicts_expired() {
        let mut c = ContextCache::new(Duration::from_millis(1));
        c.insert(1, ans("a"));
        c.insert(2, ans("b"));
        std::thread::sleep(Duration::from_millis(5));
        // insert(3) должен эвикнуть протухшие 1 и 2.
        c.insert(3, ans("c"));
        assert_eq!(c.len(), 1, "протухшие записи эвикчены");
        assert!(c.get(&1).is_none());
        assert!(c.get(&2).is_none());
        assert!(c.get(&3).is_some());
    }

    #[test]
    fn clear_empties_cache() {
        let mut c = ContextCache::new(Duration::from_secs(300));
        c.insert(1, ans("a"));
        c.insert(2, ans("b"));
        assert_eq!(c.len(), 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn empty_cache_is_empty() {
        let c = ContextCache::new(Duration::from_secs(300));
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn ttl_5_min_default_for_suggest() {
        // PRD-0010 F-2.8: TTL 5 минут (300 секунд). Константа — в
        // LlmMmSource::new; здесь проверяем только что такое значение
        // допустимо и работает.
        let mut c = ContextCache::new(Duration::from_secs(300));
        c.insert(1, ans("a"));
        assert!(c.get(&1).is_some(), "запись свежая (< 300с) → кэш-хит");
    }
}
