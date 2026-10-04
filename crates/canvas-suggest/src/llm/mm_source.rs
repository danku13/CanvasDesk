//! FR-LLM-C / PRD-0010 F-2.1-F-2.8 (Stream C): LLM mm-source для fusion.
//!
//! `LlmMmSource<P>` — choice-ранжирование через любой `LlmProvider`
//! (OpenAI/Anthropic/z.ai/Moonshot/Ollama/OpenRouter). Тот же интерфейс
//! что `LayaClient::choice` (порт для fusion-воркера), но реализован через
//! `LlmProvider::choice()` (canvas-llm).
//!
//! ## Поток данных (FR-079 §3, Q1+Q2+Q4)
//!
//! 1. **Redact** (Q1): `redact_context(document, privacy_mode)` — заменяет
//!    числа в шаблонах `key=value[unit]` на `<redacted>`. Cloud mode →
//!    Redact; Local/SelfHosted → Off (данные не уходят).
//! 2. **Cache hit** (Q2): если `(redacted_doc, options)` уже в кэше и не
//!    протух (TTL 5 мин) — возвращаем кэш, без LLM-запроса.
//! 3. **Cache miss** → `LlmProvider::choice(redacted, llm_options)` —
//!    сетевой запрос к провайдеру. Таймаут — на стороне провайдера
//!    (`OpenAiCompatibleProvider` / `AnthropicClaudeProvider`).
//! 4. **Cache insert**: `ChoiceAnswer` сохраняется в кэш по хешу контекста.
//! 5. **Error** → `Err(LlmError)` — вызывающая сторона (воркер) подставляет
//!    пустой mm-лист, fusion вырождается в lex: `fusion(lex, ∅) = lex`
//!    (как Laya, FR-079 §3).
//!
//! ## Conformance (PRD-0010 NF-3)
//!
//! - LLM-mm **отключена в CI** (golden-тесты только lex): feature `l1-llm`
//!   не включается в CI-сборке suggest-крейта (см. `Cargo.toml` features).
//! - ADR-0010 (MCP-stdio чистота): LLM-транспорт отдельный от MCP-моста;
//!   `LlmMmSource` — чистый async-вызов `LlmProvider`, никакого stdio.
//!
//! ## Cost (Q4)
//!
//! Стоимость запроса считает `canvas_llm::cost::actual_cost()` после
//! получения ответа (usage из `ChoiceAnswer` или оценки по токенам).
//! Интеграция — на стороне App (статусная панель Stream B):
//! `ai_cost_session += actual_cost(...)`. Здесь — только сам запрос.
//!
//! ## Custom-node suggest (F-2.12-F-2.16)
//!
//! НЕ реализовано в этой сессии (вне рамок Stream C этап 1). Оставлено
//! как TODO — отдельный модуль `custom_suggest.rs` (см. план 4-streams).

// FR-LLM-C: маркер для поиска (grep) — все новые файлы/добавления помечены
// `// FR-LLM-C:` в комментариях.

use crate::llm::cache::ContextCache;
use crate::types::OptionDesc;
use canvas_llm::{redact_context, ChoiceAnswer, LlmError, LlmProvider, PrivacyMode};
use std::time::Duration;

/// TTL кэша по умолчанию — 5 минут (PRD-0010 F-2.8).
const DEFAULT_TTL: Duration = Duration::from_secs(300);

/// LLM mm-source для fusion (FR-079, PRD-0010 F-2).
///
/// Тот же интерфейс что Laya — `choice(document, options) → ChoiceAnswer`.
/// Используется fusion-воркером (`crate::fusion::fuse`) как mm-источник
/// вместо `LayaClient`, когда `LlmSettings::provider_suggest = Byok`.
///
/// Обобщён по `P: LlmProvider` — App создаёт `LlmMmSource<OpenAiCompatibleProvider>`
/// (или `AnthropicClaudeProvider` и т.п.) и хранит как `LlmMmSource<Box<dyn LlmProvider>>`
/// или конкретный тип. В простейшем случае — generic-инстанс.
pub struct LlmMmSource<P: LlmProvider> {
    /// LLM-провайдер (OpenAI/Anthropic/z.ai/…). Хранится владельчески —
    /// один инстанс на воркер (как `LayaClient`).
    provider: P,
    /// In-memory кэш ответов по хешу контекста (Q2 prefetch, F-2.8).
    cache: ContextCache,
    /// Privacy-режим для `redact_context` (Q1). Cloud → Redact, Local → Off.
    /// Управляется настройкой `LlmSettings::data_residency` (через
    /// `DataResidency::privacy_mode()`).
    privacy_mode: PrivacyMode,
    /// Таймаут suggest-запроса (мс). На стороне провайдера — `ureq::Agent`
    /// с этим таймаутом. По умолчанию 10с (NF-5: «Timeout: 10с для suggest»).
    #[allow(dead_code)] // FR-LLM-C: пока не используется (провайдер сам ставит таймаут);
    // оставлено для будущей поддержки per-source timeout.
    timeout_ms: u64,
}

impl<P: LlmProvider> LlmMmSource<P> {
    /// Новый mm-source с заданным провайдером и privacy-режимом.
    ///
    /// TTL кэша — 5 минут (PRD-0010 F-2.8). Таймаут — 10 секунд (NF-5).
    /// Для иного TTL используйте [`Self::with_ttl`].
    pub fn new(provider: P, privacy_mode: PrivacyMode) -> Self {
        Self {
            provider,
            cache: ContextCache::new(DEFAULT_TTL),
            privacy_mode,
            timeout_ms: 10_000,
        }
    }

    /// Переопределить TTL кэша (например, для тестов — короткий TTL).
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.cache = ContextCache::new(ttl);
        self
    }

    /// Переопределить privacy-режим (если App сменил `data_residency`).
    /// После смены режима стоит [`Self::clear_cache`] — старые ответы
    /// могли быть от redacted-контекста, теперь пользователь сменил на
    /// Local → контекст не redacted → хеши не совпадают, но кэш чистить
    /// всё равно разумно (гигиена).
    pub fn set_privacy_mode(&mut self, mode: PrivacyMode) {
        self.privacy_mode = mode;
    }

    /// Очистить кэш (при смене провайдера / модели / privacy-режима —
    /// старые ответы могут быть не валидны для новой конфигурации).
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Choice-ранжирование (порт `LayaClient::choice`).
    ///
    /// 1. Redact context (Q1) — заменяет числа на `<redacted>` если Cloud.
    /// 2. Cache hit (Q2) — мгновенный ответ, без LLM-запроса.
    /// 3. Cache miss → `provider.choice(redacted, llm_options)` — запрос.
    /// 4. Cache insert — сохраняем ответ для будущих запросов.
    ///
    /// Конверсия `canvas_suggest::OptionDesc` → `canvas_llm::OptionDesc`:
    /// типы дублированы (cyclic-dep, см. `canvas-llm::types::OptionDesc`),
    /// тривиальная конверсия по одинаковым полям (id + desc).
    ///
    /// Возвращает `ChoiceAnswer` (probs + confidence). Ошибка → вызывающая
    /// сторона (воркер) подставляет пустой mm-лист, fusion вырождается в
    /// lex (как Laya, FR-079 §3).
    pub async fn choice(
        &mut self,
        document: &str,
        options: &[OptionDesc],
    ) -> Result<ChoiceAnswer, LlmError> {
        // Q1: redact контекста перед отправкой в cloud LLM (если Cloud mode).
        let redacted = redact_context(document, self.privacy_mode);

        // Q2: cache hit — мгновенный ответ, без LLM-запроса.
        let cache_key = self.cache.hash(&redacted, options);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        // Конверсия OptionDesc: canvas_suggest → canvas_llm (дублированный
        // тип, см. комментарий в `canvas-llm/src/types.rs::OptionDesc`).
        let llm_options: Vec<canvas_llm::OptionDesc> = options
            .iter()
            .map(|o| canvas_llm::OptionDesc::new(o.id.clone(), o.desc.clone()))
            .collect();

        // Cache miss → LLM-запрос.
        let result = self.provider.choice(&redacted, &llm_options).await?;

        // Кэшируем результат (для будущих запросов с тем же контекстом).
        self.cache.insert(cache_key, result.clone());
        Ok(result)
    }

    /// Доступ к провайдеру (для health-check, models list и т.п. —
    /// App может вызвать `provider.health()` перед использованием mm-source).
    pub fn provider(&self) -> &P {
        &self.provider
    }

    /// Мутабельный доступ к провайдеру (для `set_active_model` и т.п.).
    pub fn provider_mut(&mut self) -> &mut P {
        &mut self.provider
    }

    /// Число записей в кэше (для диагностики / HUD).
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_llm::{ChoiceAnswer, LlmError};
    use std::sync::{Arc, Mutex};

    /// Простейший мок: хранит число вызовов choice() и фиксированный ответ.
    /// `async_trait` нужен — `LlmProvider` объявлен через `#[async_trait]`
    /// (canvas-llm, feature `l1-llm`).
    struct MockProvider {
        calls: Arc<Mutex<usize>>,
        answer: ChoiceAnswer,
    }

    impl MockProvider {
        fn new(answer: ChoiceAnswer) -> Self {
            Self {
                calls: Arc::new(Mutex::new(0)),
                answer,
            }
        }

        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockProvider {
        fn id(&self) -> &str {
            "mock"
        }
        fn display_name(&self) -> &str {
            "Mock"
        }
        fn caps(&self) -> canvas_llm::ProviderCaps {
            canvas_llm::ProviderCaps {
                choice: true,
                ..Default::default()
            }
        }
        fn models(&self) -> &[canvas_llm::ModelInfo] {
            &[]
        }
        fn active_model(&self) -> &str {
            "mock-1"
        }
        async fn chat(
            &self,
            _messages: &[canvas_llm::Message],
            _opts: &canvas_llm::ChatOpts,
        ) -> Result<String, LlmError> {
            Err(LlmError::NotSupported("chat"))
        }
        async fn choice(
            &self,
            _document: &str,
            _options: &[canvas_llm::OptionDesc],
        ) -> Result<ChoiceAnswer, LlmError> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.answer.clone())
        }
        async fn tool_calling(
            &self,
            _messages: &[canvas_llm::Message],
            _tools: &[canvas_llm::ToolDef],
            _opts: &canvas_llm::ToolCallingOpts,
        ) -> Result<Vec<canvas_llm::ToolCall>, LlmError> {
            Err(LlmError::NotSupported("tool_calling"))
        }
        async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError> {
            Err(LlmError::NotSupported("embed"))
        }
        async fn health(&self) -> Result<(), LlmError> {
            Ok(())
        }
    }

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

    /// Блокирующий запуск async-фьючерса (mock-провайдер не имеет реальной
    /// сети — `pollster::block_on` достаточно; tokio не тянем ради тестов).
    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        pollster::block_on(f)
    }

    #[test]
    fn choice_calls_provider_on_cache_miss() {
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Off);
        let opts = opts(&["a", "b"]);
        let result = block_on(src.choice("doc", &opts)).unwrap();
        assert_eq!(result.best(), Some("a"));
        assert_eq!(src.provider().calls(), 1, "первый запрос — cache miss");
    }

    #[test]
    fn choice_returns_cached_on_hit() {
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Off);
        let opts = opts(&["a", "b"]);
        // Первый запрос — cache miss.
        block_on(src.choice("doc", &opts)).unwrap();
        // Второй запрос с тем же контекстом — cache hit, без вызова провайдера.
        let result = block_on(src.choice("doc", &opts)).unwrap();
        assert_eq!(result.best(), Some("a"));
        assert_eq!(
            src.provider().calls(),
            1,
            "второй запрос — cache hit, провайдер не дёргается"
        );
    }

    #[test]
    fn choice_different_documents_cause_miss() {
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Off);
        let opts = opts(&["a", "b"]);
        block_on(src.choice("doc1", &opts)).unwrap();
        block_on(src.choice("doc2", &opts)).unwrap();
        assert_eq!(
            src.provider().calls(),
            2,
            "разные document → разные ключи → 2 cache miss"
        );
    }

    #[test]
    fn choice_redacts_in_cloud_mode() {
        // PrivacyMode::Redact → числа заменяются на <redacted>.
        // Один и тот же raw document с разными числами → один redacted →
        // один хеш → cache hit (провайдер не дёргается повторно).
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Redact);
        let opts = opts(&["a", "b"]);
        // price=10руб → redacted; price=20руб → тоже redacted → тот же хеш.
        block_on(src.choice("price=10руб", &opts)).unwrap();
        block_on(src.choice("price=20руб", &opts)).unwrap();
        assert_eq!(
            src.provider().calls(),
            1,
            "redact унифицирует контекст → cache hit на 2-й запрос"
        );
    }

    #[test]
    fn choice_no_redact_in_off_mode() {
        // PrivacyMode::Off → контекст как есть → разные числа → разные хеши.
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Off);
        let opts = opts(&["a", "b"]);
        block_on(src.choice("price=10руб", &opts)).unwrap();
        block_on(src.choice("price=20руб", &opts)).unwrap();
        assert_eq!(
            src.provider().calls(),
            2,
            "Off mode → raw контекст → разные хеши → 2 cache miss"
        );
    }

    #[test]
    fn clear_cache_forces_next_request() {
        let mock = MockProvider::new(ans("a"));
        let mut src = LlmMmSource::new(mock, PrivacyMode::Off);
        let opts = opts(&["a", "b"]);
        block_on(src.choice("doc", &opts)).unwrap();
        assert_eq!(src.provider().calls(), 1);
        src.clear_cache();
        assert_eq!(src.cache_len(), 0);
        block_on(src.choice("doc", &opts)).unwrap();
        assert_eq!(src.provider().calls(), 2, "после clear_cache — новый miss");
    }

    #[test]
    fn default_ttl_is_5_min() {
        // PRD-0010 F-2.8: TTL 5 минут (300 секунд).
        assert_eq!(DEFAULT_TTL, Duration::from_secs(300));
    }

    #[test]
    fn default_timeout_is_10s() {
        // NF-5: «Timeout: 10с для suggest».
        let mock = MockProvider::new(ans("a"));
        let src = LlmMmSource::new(mock, PrivacyMode::Off);
        assert_eq!(src.timeout_ms, 10_000);
    }
}
