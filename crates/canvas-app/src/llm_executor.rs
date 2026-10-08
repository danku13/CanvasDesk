//! W2 (п.7 плана) — LLM-executor: общий механизм запуска async LLM-вызовов.
//!
//! `LlmProvider`-методы — async (async-trait). Панели/health/discovery —
//! синхронный UI-код winit. Этот модуль — мост:
//!
//! - **Натив** — джоба уходит в worker-поток (`std::thread`), future
//!   дожимается `pollster::block_on`, результат кладётся в shared-инбокс
//!   (паттерн `suggest_worker`/`oauth_flow`);
//! - **wasm** — future не-`Send` (`JsFuture`), блокирующего ожидания нет:
//!   инъектируется шов [`LlmSpawnFn`] (canvas-web W3 подставит
//!   `spawn_local`); без шва джоба мгновенно завершается ошибкой
//!   «LLM-вызовы недоступны до волны W3» (graceful, F-5.9);
//! - **Опрос** — инбокс дренируется в `about_to_wait` каждый кадр
//!   (`llm_poll`, паттерн `oauth_poll`/`suggest.pending`); устаревшие
//!   поколения отбрасываются диспетчером App.
//!
//! Feature-гейт: модуль целиком за `l1-llm` (дефолтная сборка без сети —
//! ADR-0011; панели там работают на mock-флоу).

use canvas_llm::health::HealthReport;
use canvas_llm::transport::{HttpTransport, UreqTransport};
use canvas_llm::{
    DataResidency, LlmProvider, Message, OptionDesc as LlmOptionDesc, ToolCall, ToolCallingOpts,
    ToolDef,
};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Джобы и результаты
// ---------------------------------------------------------------------------

/// Цель health-check (какая кнопка «Проверить» отправила джобу).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthTarget {
    /// Строка «API-ключ» (BYOK-провайдер: OpenRouter/OpenAI/z.ai/…).
    Byok,
    /// Строки self-hosted (endpoint + отдельный ключ).
    Selfhost,
}

/// Тип джобы (тег результата — диспетчеризация в `llm_poll`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmJobKind {
    /// Health-check ключа/endpoint'а (W2.4).
    Health,
    /// Discovery моделей (`GET /v1/models`) после успешного health (W2.5).
    Models,
    /// Agent Panel: `tool_calling` (W2.2).
    Agent,
    /// Graph Builder: генерация графа (W2.2).
    Graph,
    /// Suggest: LLM mm-ранжирование — choice (W2.8, wasm).
    SuggestChoice,
}

/// Payload результата (по [`LlmJobKind`]).
pub enum LlmOutcome {
    /// Health-check завершён (Ok/Auth/RateLimit/Transport/…).
    Health(HealthTarget, HealthReport),
    /// Discovery: список моделей (отсортирован в discovery.rs).
    Models(Vec<canvas_llm::DiscoveredModel>),
    /// Agent: tool-calls от модели.
    Agent(Vec<ToolCall>),
    /// Graph Builder: разобранный граф (применение — UI-тред).
    Graph(Box<canvas_graph_builder::GraphBuilderOutput>),
    /// Suggest: probs + confidence от LLM mm-источника.
    SuggestChoice {
        /// Вероятности (id → prob), как `ChoiceAnswer::probs`.
        probs: Vec<(String, f64)>,
        /// Уверенность модели (0..1).
        confidence: f64,
    },
}

/// Результат джобы (уходит в инбокс → `llm_poll` → диспетчер App).
pub struct LlmJobResult {
    /// Тег джобы.
    pub kind: LlmJobKind,
    /// Payload результата.
    pub outcome: Result<LlmOutcome, String>,
    /// Фактический/оценочный cost запроса (USD; 0.0 — если не считался).
    pub cost: f64,
}

// ---------------------------------------------------------------------------
// Запросы на исполнение
// ---------------------------------------------------------------------------

/// Джоба health-check: провайдер строится на UI-треде фабрикой
/// (`llm_factory` — та же валидация, что для отправки), проверка — в джобе.
pub struct HealthJob {
    pub target: HealthTarget,
    pub provider: Box<dyn LlmProvider>,
}

/// Джоба discovery моделей.
pub struct ModelsJob {
    /// Base URL OpenAI-compatible API (selfhost или пресет BYOK).
    pub base_url: String,
    /// API-ключ (`None`/пустой — запрос без Authorization).
    pub api_key: Option<String>,
}

/// Джоба Agent Panel: provider.tool_calling(messages, tools, opts).
pub struct AgentJob {
    pub provider: Box<dyn LlmProvider>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub opts: ToolCallingOpts,
}

/// Джоба Graph Builder: GraphBuilder::build(input) через провайдера.
pub struct GraphJob {
    pub provider: BoxedProvider,
    pub text: String,
    pub mode: canvas_graph_builder::GraphBuilderMode,
    /// Privacy-режим (redact контекста, Q1).
    pub privacy: DataResidency,
}

/// Джоба suggest mm-ранжирования (W2.8: wasm — через executor; натив —
/// suggest-воркер как раньше).
pub struct SuggestChoiceJob {
    pub provider: Box<dyn LlmProvider>,
    /// Контекст «формат А» (редактируется перед отправкой).
    pub document: String,
    /// Опции каталога (canvas-suggest::OptionDesc → canvas_llm::OptionDesc
    /// внутри джобы).
    pub options: Vec<canvas_suggest::OptionDesc>,
    /// Privacy-режим (redact контекста, Q1).
    pub privacy: DataResidency,
}

/// Wasm-шов: closure «построить future» — canvas-web (W3) подставит
/// `spawn_local(thunk())` и callback результата в инбокс executor'а.
/// Инъекция — [`LlmExecutor::set_spawner`].
#[cfg(all(feature = "l1-llm", target_arch = "wasm32"))]
pub type LlmSpawnFn = std::rc::Rc<dyn Fn(LlmJobThunk)>;
/// Async-тело джобы (Output = () — результат future кладёт в инбокс сам,
/// `push_result`; натив — Send для worker-потока, wasm — без Send:
/// исполнение через spawn_local в UI-треде).
#[cfg(not(target_arch = "wasm32"))]
pub type LlmFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type LlmFuture = std::pin::Pin<Box<dyn std::future::Future<Output = ()>>>;

/// Thunk для wasm-шва.
#[cfg(not(target_arch = "wasm32"))]
pub type LlmJobThunk = Box<dyn FnOnce() -> LlmFuture + Send>;
#[cfg(target_arch = "wasm32")]
pub type LlmJobThunk = Box<dyn FnOnce() -> LlmFuture>;

/// Владеющий провайдер-обёртка: `Box<dyn LlmProvider>` → `LlmProvider`
/// (делегация). Нужна для generic-конструкторов (`GraphBuilder<P>`):
/// в canvas-llm нет blanket-impl для `Box<dyn _>` (W2 не трогает чужие
/// крейты — реализация локально).
pub struct BoxedProvider(pub Box<dyn LlmProvider>);

#[async_trait::async_trait]
impl LlmProvider for BoxedProvider {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn display_name(&self) -> &str {
        self.0.display_name()
    }
    fn caps(&self) -> canvas_llm::ProviderCaps {
        self.0.caps()
    }
    fn models(&self) -> &[canvas_llm::ModelInfo] {
        self.0.models()
    }
    fn active_model(&self) -> &str {
        self.0.active_model()
    }
    async fn chat(
        &self,
        messages: &[Message],
        opts: &canvas_llm::ChatOpts,
    ) -> Result<String, canvas_llm::LlmError> {
        self.0.chat(messages, opts).await
    }
    async fn choice(
        &self,
        document: &str,
        options: &[LlmOptionDesc],
    ) -> Result<canvas_llm::ChoiceAnswer, canvas_llm::LlmError> {
        self.0.choice(document, options).await
    }
    async fn tool_calling(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        opts: &ToolCallingOpts,
    ) -> Result<Vec<ToolCall>, canvas_llm::LlmError> {
        self.0.tool_calling(messages, tools, opts).await
    }
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, canvas_llm::LlmError> {
        self.0.embed(texts).await
    }
    async fn health(&self) -> Result<(), canvas_llm::LlmError> {
        self.0.health().await
    }
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

/// Реестр LLM-джоб: запуск (поток/wasm-шов) + инбокс результатов.
pub struct LlmExecutor {
    /// Shared-инбокс результатов.
    inbox: Arc<Mutex<Vec<LlmJobResult>>>,
    /// Счётчик id (трассировка).
    next_id: AtomicU64,
    /// Транспорт для джоб без провайдера (discovery): натив — Ureq;
    /// wasm — W3 инъектирует WasmFetchTransport (fetch-транспорт F-5.10).
    transport: Arc<dyn HttpTransport>,
    /// В работе (дедупликация: вторая «Проверить»/Generate игнорируется).
    inflight: Mutex<Vec<LlmJobKind>>,
    /// Wasm-шов (инъекция W3): `None` — джобы фейлятся сразу.
    #[cfg(all(feature = "l1-llm", target_arch = "wasm32"))]
    spawner: Option<LlmSpawnFn>,
}

impl Default for LlmExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Положить результат в инбокс (общее для натива/wasm-шва).
fn push_result(inbox: &Mutex<Vec<LlmJobResult>>, result: LlmJobResult) {
    if let Ok(mut q) = inbox.lock() {
        q.push(result);
    }
}

impl LlmExecutor {
    /// Новый executor (инбокс пуст; wasm — шов не инъектирован).
    pub fn new() -> Self {
        Self {
            inbox: Arc::new(Mutex::new(Vec::new())),
            next_id: AtomicU64::new(1),
            transport: Arc::new(UreqTransport::new()),
            inflight: Mutex::new(Vec::new()),
            #[cfg(all(feature = "l1-llm", target_arch = "wasm32"))]
            spawner: None,
        }
    }

    /// W3 (canvas-web): инъекция wasm-шва (`spawn_local` + callback).
    #[cfg(all(feature = "l1-llm", target_arch = "wasm32"))]
    pub fn set_spawner(&mut self, spawner: LlmSpawnFn) {
        self.spawner = Some(spawner);
    }

    /// W3 (canvas-web): инъекция fetch-транспорта (discovery/health-джобы).
    pub fn set_transport(&mut self, transport: Arc<dyn HttpTransport>) {
        self.transport = transport;
    }

    /// Транспорт executor'а (для джоб, строящихся на стороне UI).
    pub fn transport(&self) -> Arc<dyn HttpTransport> {
        self.transport.clone()
    }

    /// Джоба в работе? (дедупликация повторных запусков).
    pub fn is_inflight(&self, kind: LlmJobKind) -> bool {
        self.inflight
            .lock()
            .map(|q| q.contains(&kind))
            .unwrap_or(false)
    }

    /// Взять джобу в работу (false — такая джоба уже в работе).
    fn begin(&self, kind: LlmJobKind) -> bool {
        let mut q = self.inflight.lock().unwrap();
        if q.contains(&kind) {
            return false;
        }
        q.push(kind);
        true
    }

    /// Дренаж инбокса: вернуть готовые результаты (снимая с учёта).
    pub fn drain(&self) -> Vec<LlmJobResult> {
        let results = self
            .inbox
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        for r in &results {
            if let Ok(mut q) = self.inflight.lock() {
                q.retain(|k| *k != r.kind);
            }
        }
        results
    }

    /// Запустить джобу health-check (W2.4).
    pub fn spawn_health(&self, job: HealthJob) {
        let kind = LlmJobKind::Health;
        if !self.begin(kind) {
            return;
        }
        let _id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let inbox = self.inbox.clone();
        let thunk: LlmJobThunk = Box::new(move || {
            Box::pin(async move {
                let report = canvas_llm::health::check_provider(&*job.provider).await;
                tracing::info!(target = ?job.target, report = %report, "llm-executor: health");
                let result = LlmJobResult {
                    kind,
                    outcome: Ok(LlmOutcome::Health(job.target, report)),
                    cost: 0.0,
                };
                push_result(&inbox, result);
            })
        });
        self.run_thunk(thunk);
    }

    /// Запустить джобу discovery моделей (W2.5; после успешного health).
    pub fn spawn_models(&self, job: ModelsJob) {
        let kind = LlmJobKind::Models;
        if !self.begin(kind) {
            return;
        }
        let _id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let inbox = self.inbox.clone();
        let transport = self.transport.clone();
        let thunk: LlmJobThunk = Box::new(move || {
            Box::pin(async move {
                let models = canvas_llm::discovery::list_models(
                    &*transport,
                    &job.base_url,
                    job.api_key.as_deref().filter(|k| !k.trim().is_empty()),
                    std::time::Duration::from_secs(10),
                )
                .await;
                let outcome = match models {
                    Ok(m) => {
                        tracing::info!(count = m.len(), "llm-executor: discovery");
                        Ok(LlmOutcome::Models(m))
                    }
                    Err(e) => Err(e.to_string()),
                };
                push_result(
                    &inbox,
                    LlmJobResult {
                        kind,
                        outcome,
                        cost: 0.0,
                    },
                );
            })
        });
        self.run_thunk(thunk);
    }

    /// Запустить джобу Agent Panel (tool_calling, W2.2).
    pub fn spawn_agent(&self, job: AgentJob) {
        let kind = LlmJobKind::Agent;
        if !self.begin(kind) {
            return;
        }
        let _id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let inbox = self.inbox.clone();
        let thunk: LlmJobThunk = Box::new(move || {
            Box::pin(async move {
                // Cost-оценка (W2.3): по размеру промпта/ответа через
                // estimate_tokens + actual_cost (единая точка — cost.rs).
                // usage API не читаем — ToolCall его не несёт.
                let model = job
                    .provider
                    .models()
                    .iter()
                    .find(|m| m.id == job.provider.active_model())
                    .cloned();
                let in_tokens = canvas_llm::estimate_tokens(
                    &job.messages
                        .iter()
                        .map(|m| m.content.text())
                        .collect::<String>(),
                );
                let result = job
                    .provider
                    .tool_calling(&job.messages, &job.tools, &job.opts)
                    .await;
                let out_tokens = canvas_llm::estimate_tokens(&format!("{result:?}"));
                let cost = model
                    .as_ref()
                    .and_then(|m| m.pricing.as_ref())
                    .map(|p| canvas_llm::actual_cost(p, in_tokens, out_tokens))
                    .unwrap_or(0.0);
                let outcome = result.map(LlmOutcome::Agent).map_err(|e| e.to_string());
                push_result(
                    &inbox,
                    LlmJobResult {
                        kind,
                        outcome,
                        cost,
                    },
                );
            })
        });
        self.run_thunk(thunk);
    }

    /// Запустить джобу Graph Builder (W2.2).
    pub fn spawn_graph(&self, job: GraphJob) {
        let kind = LlmJobKind::Graph;
        if !self.begin(kind) {
            return;
        }
        let _id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let inbox = self.inbox.clone();
        let thunk: LlmJobThunk = Box::new(move || {
            Box::pin(async move {
                let privacy = job.privacy.privacy_mode();
                let builder = canvas_graph_builder::GraphBuilder::new(job.provider, privacy);
                let input = canvas_graph_builder::GraphBuilderInput::new(&job.text, job.mode);
                // Cost-оценка по входу (out — консервативно 800 токенов,
                // как в GraphBuilder::estimate).
                let cost = 0.0; // точная оценка — после ответа, по models()
                let result = builder.build(&input).await;
                let outcome = result
                    .map(|out| LlmOutcome::Graph(Box::new(out)))
                    .map_err(|e| e.to_string());
                push_result(
                    &inbox,
                    LlmJobResult {
                        kind,
                        outcome,
                        cost,
                    },
                );
            })
        });
        self.run_thunk(thunk);
    }

    /// Запустить джобу suggest mm-ранжирования (W2.8).
    pub fn spawn_suggest_choice(&self, job: SuggestChoiceJob) {
        let kind = LlmJobKind::SuggestChoice;
        if !self.begin(kind) {
            return;
        }
        let _id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let inbox = self.inbox.clone();
        let thunk: LlmJobThunk = Box::new(move || {
            Box::pin(async move {
                // Q1 redact (как LlmMmSource::choice) + choice.
                let redacted =
                    canvas_llm::redact_context(&job.document, job.privacy.privacy_mode());
                let llm_options: Vec<LlmOptionDesc> = job
                    .options
                    .iter()
                    .map(|o| LlmOptionDesc::new(o.id.clone(), o.desc.clone()))
                    .collect();
                let result = job.provider.choice(&redacted, &llm_options).await;
                // Ошибка mm-источника — НЕ ошибка джобы: caller оставляет
                // lex-ответ (fusion(lex, ∅) = lex — вырождение встроено).
                let outcome = match result {
                    Ok(answer) => LlmOutcome::SuggestChoice {
                        probs: answer.probs,
                        confidence: answer.confidence,
                    },
                    Err(e) => {
                        tracing::debug!(err = %e, "llm-executor: suggest choice failed → lex");
                        LlmOutcome::SuggestChoice {
                            probs: Vec::new(),
                            confidence: 0.0,
                        }
                    }
                };
                push_result(
                    &inbox,
                    LlmJobResult {
                        kind,
                        outcome: Ok(outcome),
                        cost: 0.0,
                    },
                );
            })
        });
        self.run_thunk(thunk);
    }

    /// Платформенный запуск thunk: натив — поток + pollster; wasm —
    /// инъектированный шов или мгновенный фейл (до W3).
    fn run_thunk(&self, thunk: LlmJobThunk) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let spawn = std::thread::Builder::new()
                .name("llm-executor".to_owned())
                .spawn(move || {
                    // Результат кладётся future'ом в инбокс (push_result
                    // внутри async-блока) — общий контракт для натива и
                    // wasm-шва (spawn_local в UI-треде).
                    let future = thunk();
                    let _ = pollster::block_on(future);
                });
            if spawn.is_err() {
                self.push_fail("llm-executor: поток не запущен");
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            match &self.spawner {
                Some(spawn) => spawn(thunk),
                None => self.push_fail(
                    "LLM-вызовы из web-сборки включаются волной W3 (fetch-транспорт F-5.10)",
                ),
            }
        }
    }

    /// Мгновенный фейл (поток не стартовал / wasm-шов не инъектирован).
    /// kind неточен (джоба ещё не знает исхода) — диспетчер показывает
    /// только текст ошибки.
    fn push_fail(&self, reason: &str) {
        push_result(
            &self.inbox,
            LlmJobResult {
                kind: LlmJobKind::Health,
                outcome: Err(reason.to_string()),
                cost: 0.0,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Тесты (механики executor'а — без сети)
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "l1-llm"))]
mod tests {
    use super::*;
    use canvas_llm::MockTransport;

    /// Ожидать результат worker-потока (дренаж с ретраями — поток
    /// асинхронен по отношению к тесту).
    fn wait_results(ex: &LlmExecutor, tries: usize) -> Vec<LlmJobResult> {
        for _ in 0..tries {
            let r = ex.drain();
            if !r.is_empty() {
                return r;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        Vec::new()
    }

    #[test]
    fn drain_empty_and_inflight_clean() {
        let ex = LlmExecutor::new();
        assert!(ex.drain().is_empty());
        for kind in [
            LlmJobKind::Health,
            LlmJobKind::Models,
            LlmJobKind::Agent,
            LlmJobKind::Graph,
            LlmJobKind::SuggestChoice,
        ] {
            assert!(!ex.is_inflight(kind));
        }
    }

    #[test]
    fn health_job_refused_connection_is_transport_report() {
        // Порт 1 на loopback — ничего не слушает: без внешней сети.
        let ex = LlmExecutor::new();
        let provider = canvas_llm::OpenAiCompatibleProvider::new(
            "selfhost",
            "Self-hosted",
            "http://127.0.0.1:1/v1",
            "sk",
            "m",
            Vec::new(),
        )
        .with_timeout(1);
        ex.spawn_health(HealthJob {
            target: HealthTarget::Selfhost,
            provider: Box::new(provider),
        });
        let results = wait_results(&ex, 400);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, LlmJobKind::Health);
        assert_eq!(results[0].cost, 0.0);
        match &results[0].outcome {
            Ok(LlmOutcome::Health(target, report)) => {
                assert_eq!(*target, HealthTarget::Selfhost);
                assert!(!report.is_ok(), "connection refused не может быть Ok");
                assert!(matches!(
                    report,
                    canvas_llm::health::HealthReport::Transport(_)
                ));
            }
            _ => panic!("ожидаем Health"),
        }
        // После дренажа джоба снята с учёта.
        assert!(!ex.is_inflight(LlmJobKind::Health));
    }

    #[test]
    fn models_job_parses_or_fails_gracefully() {
        // Отказ соединения → Err в инбоксе (UI покажет fallback).
        let ex = LlmExecutor::new();
        ex.spawn_models(ModelsJob {
            base_url: "http://127.0.0.1:1/v1".into(),
            api_key: Some("sk".into()),
        });
        let results = wait_results(&ex, 400);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, LlmJobKind::Models);
        assert!(results[0].outcome.is_err());
    }

    #[test]
    fn executor_transport_injectable() {
        // W3-шов: подмена транспорта (здесь — mock из canvas-llm).
        let mut ex = LlmExecutor::new();
        let mock: Arc<dyn HttpTransport> = Arc::new(MockTransport::new(Vec::new()));
        ex.set_transport(mock);
        // Проверяем, что транспорт заменён (джобы с пустым mock — Err).
        ex.spawn_models(ModelsJob {
            base_url: "https://x/v1".into(),
            api_key: None,
        });
        // MockTransport::new(пустой) → фейл ответа → Err (graceful).
        let results = wait_results(&ex, 400);
        assert_eq!(results.len(), 1);
        assert!(results[0].outcome.is_err());
    }
}
