//! FR-LLM-D (llm-waves W2 п.7) — executor-сим для LLM-футур.
//!
//! Общий механизм запуска async-вызовов LLM вне UI-треда:
//! - **натив** — выделенный worker-поток + канал заданий (`std::thread` +
//!   `mpsc`, паттерны `suggest_worker` / `oauth_flow`); async-футуры
//!   исполняются `pollster::block_on` (крейт уже в дереве через
//!   `canvas-render` — новых внешних зависимостей нет);
//!   результат будит event loop через `EventLoopProxy` (`AppEvent::LlmTask`);
//! - **wasm** — стаб-драйвер: `submit` возвращает `false` («недоступно до
//!   W3»); инъекцию реального драйвера (`spawn_local` + пробуждение
//!   `about_to_wait`) выполнит `canvas-web` в W3 через
//!   [`App::attach_llm_executor`] / [`LlmExecutor::from_driver`].
//!
//! Задание — замыкание `FnOnce() -> AppEvent`: строится в точке вызова
//! (панель/настройки/suggest), исполняется в воркере, событие доставки
//! уходит через notifier. Паника задания ловится `catch_unwind` —
//! UI-состояния (busy-флаги панелей) сбрасываются исходом
//! [`LlmTaskOutcome::Panicked`].
//!
//! Потребители (все — точки волн W2):
//! - `agent_send` (agent_panel.rs) — `tool_calling` → [`LlmTaskOutcome::Agent`];
//! - `graph_builder_generate` (graph_builder_ui.rs) — `GraphBuilder::build`
//!   → [`LlmTaskOutcome::Graph`];
//! - «Проверить» в настройках AI (overlays.rs) — `provider.health()`
//!   → [`LlmTaskOutcome::Health`];
//! - suggest mm-источник в wasm-ветке (app.rs) — `choice` →
//!   [`LlmTaskOutcome::SuggestMm`].

// FR-LLM-D-W2: маркер для поиска (grep): файлы волны W2 помечены
// `// FR-LLM-D-W2:` в комментариях.

use std::sync::mpsc;
use std::sync::Arc;

use super::*;

/// Задание executor-сима: замыкание, исполняемое вне UI-треда; результат —
/// событие доставки в event loop (обычно `AppEvent::LlmTask`).
pub type LlmJob = Box<dyn FnOnce() -> AppEvent + Send + 'static>;

/// Notifier — доставка события в event loop (натив: `EventLoopProxy::send_event`
/// через клон прокси; W3/wasm: `spawn_local` + proxy).
pub type LlmNotifier = Arc<dyn Fn(AppEvent) + Send + Sync>;

/// Драйвер исполнения заданий. Натив — worker-поток с каналом; wasm (до W3) —
/// стаб; W3 подставит `spawn_local`-драйвер. Шов инъекции —
/// [`LlmExecutor::from_driver`] + [`App::attach_llm_executor`].
pub trait LlmExecutorDriver: Send + Sync {
    /// Поставить задание в исполнение. `false` — драйвер недоступен
    /// (wasm до W3 / воркер умер) — вызывающая сторона деградирует
    /// (mock-флоу панелей / lex-режим suggest).
    fn submit(&self, job: LlmJob) -> bool;
}

/// Стаб-драйвер (wasm до W3, дефолт до `attach_llm_executor`): задания
/// не исполняются, `submit` всегда `false`.
struct StubDriver;

impl LlmExecutorDriver for StubDriver {
    fn submit(&self, _job: LlmJob) -> bool {
        // FR-LLM-D-W2: wasm — «недоступно до W3» (llm-waves §3.7);
        // вызывающие ветки вырождаются в текущее поведение (lex/mock).
        false
    }
}

/// Executor-сим: обёртка над драйвером. Клонируется (`Arc` внутри),
/// `submit` дешёвый — можно звать из любых точек App.
pub struct LlmExecutor {
    driver: Arc<dyn LlmExecutorDriver>,
}

impl Default for LlmExecutor {
    fn default() -> Self {
        Self::stub()
    }
}

impl LlmExecutor {
    /// Стаб (wasm до W3): задания не исполняются.
    pub fn stub() -> Self {
        Self {
            driver: Arc::new(StubDriver),
        }
    }

    /// Собрать executor поверх готового драйвера (W3: `spawn_local`-драйвер
    /// canvas-web).
    pub fn from_driver(driver: Arc<dyn LlmExecutorDriver>) -> Self {
        Self { driver }
    }

    /// Натив: воркер-поток + канал (паттерн `SuggestWorkerHandle::spawn`).
    /// Провал спавна не роняет запуск — возвращает stub (деградация).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn spawn(notifier: LlmNotifier) -> Self {
        let (tx, rx) = mpsc::channel::<LlmJob>();
        let builder = std::thread::Builder::new().name("llm-executor".to_owned());
        match builder.spawn(move || worker_loop(rx, notifier)) {
            Ok(_) => Self {
                driver: Arc::new(NativeDriver { jobs: tx }),
            },
            // Спавн не удался (rlimit/перегрузка) — тихая деградация на stub:
            // панели уходят в mock, suggest в lex (как мёртвый suggest-воркер).
            Err(err) => {
                tracing::warn!(%err, "llm-executor: спавн воркера не удался — LLM-вызовы деградируют");
                Self::stub()
            }
        }
    }

    /// Поставить задание. `false` — драйвер недоступен.
    pub fn submit(&self, job: LlmJob) -> bool {
        self.driver.submit(job)
    }
}

/// Цикл нативного воркера: задания по очереди, `catch_unwind` на каждое
/// (паника задания не роняет воркер), результат — notifier(event).
#[cfg(not(target_arch = "wasm32"))]
fn worker_loop(rx: mpsc::Receiver<LlmJob>, notifier: LlmNotifier) {
    while let Ok(job) = rx.recv() {
        let event = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(event) => event,
            Err(_) => {
                tracing::error!("llm-executor: паника задания — сбрасываю busy-состояния");
                AppEvent::LlmTask(Arc::new(LlmTaskOutcome::Panicked))
            }
        };
        notifier(event);
    }
}

/// Нативный драйвер: sender канала воркера.
#[cfg(not(target_arch = "wasm32"))]
struct NativeDriver {
    jobs: mpsc::Sender<LlmJob>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LlmExecutorDriver for NativeDriver {
    fn submit(&self, job: LlmJob) -> bool {
        self.jobs.send(job).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Результаты задач (payload AppEvent::LlmTask)
// ---------------------------------------------------------------------------

/// FR-LLM-D-W2: результат LLM-задачи executor-сима. Доставка — через
/// `AppEvent::LlmTask(Arc<…>)`, обработка — [`App::on_llm_task`].
#[derive(Debug, Clone)]
pub enum LlmTaskOutcome {
    /// Ответ агент-панели (`tool_calling`): вызовы инструментов + usage.
    Agent(Result<AgentChatResult, String>),
    /// Ответ генератора графа (`GraphBuilder::build`).
    #[cfg(feature = "l1-llm")]
    Graph(Result<GraphBuildResult, String>),
    /// Результат health-check строки настроек AI.
    Health {
        target: HealthCheckTarget,
        result: Result<HealthReport, String>,
    },
    /// Suggest mm-ранжирование (wasm-ветка): re-rank по LLM-пробе.
    SuggestMm {
        target: crate::suggest::SuggestTarget,
        generation: u64,
        result: Result<SuggestMmResult, String>,
    },
    /// Паника задания в воркере — сброс busy-состояний UI.
    Panicked,
}

/// Ответ агент-панели: tool-calls + оценка usage (для cost).
#[derive(Debug, Clone)]
pub struct AgentChatResult {
    /// Вызовы инструментов от LLM (name + arguments).
    pub tool_calls: Vec<canvas_llm::ToolCall>,
    /// Токены входа/выхода (оценка: провайдер не возвращает usage —
    /// `estimate_tokens` по промпту и сериализованному ответу).
    pub input_tokens: usize,
    pub output_tokens: usize,
    /// Фактический расход (USD) — `actual_cost` по тарифу модели.
    pub cost_usd: f64,
}

/// Ответ генератора графа: output + usage.
#[cfg(feature = "l1-llm")]
#[derive(Debug, Clone)]
pub struct GraphBuildResult {
    /// Сгенерированные ноды/рёбра (GraphBuilderOutput).
    pub output: canvas_graph_builder::GraphBuilderOutput,
    pub input_tokens: usize,
    pub output_tokens: usize,
    /// Фактический расход (USD).
    pub cost_usd: f64,
}

/// Health-отчёт: флаги бейджей + число моделей (для подсказки у строки).
#[derive(Debug, Clone)]
pub struct HealthReport {
    /// Число моделей, известное провайдеру (статический список `models()`;
    /// живой `/v1/models`-discovery — API W1, подключит W3).
    pub models_count: Option<usize>,
}

/// Цель health-check — строка настроек, чью кнопку «Проверить» нажали.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthCheckTarget {
    /// Строка BYOK API-ключа (`SettingsRow::AiApiKey`).
    ApiKey,
    /// Строка self-hosted endpoint (`AiSelfhostUrl` / `AiSelfhostKey`).
    Selfhost,
}

/// Suggest mm-результат: ответы после fusion + cost.
#[derive(Debug, Clone)]
pub struct SuggestMmResult {
    /// Re-rank ответы (`fusion(lex, mm)` — тот же конвейер, что у натив-
    /// воркера с Laya-пробой).
    pub answers: Arc<Vec<crate::suggest::SuggestAnswer>>,
    /// Фактический расход (USD; Ollama/free — 0.0).
    pub cost_usd: f64,
}

// ---------------------------------------------------------------------------
// UI-состояние health-check (строки настроек AI)
// ---------------------------------------------------------------------------

/// FR-LLM-D-W2 (llm-waves §3.4): состояние кнопки «Проверить» —
/// idle → проверяется → ok / ошибка. Заменяет mock-toggle бейджей.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum LlmCheckState {
    /// Ещё не проверяли (после смены ключа/endpoint возвращается сюда).
    #[default]
    Idle,
    /// Запрос в полёте (кнопка глушится, бейдж «проверяется…»).
    Checking,
    /// Успех: ok + число моделей (если известно).
    Ok { models: Option<usize> },
    /// Ошибка: человекочитаемый текст (из `LlmError`, redact-контракт).
    Error(String),
}

impl LlmCheckState {
    /// Успешен ли последний чек (для бейджей `ai_key_ok`/`ai_selfhost_ok`).
    pub fn is_ok(&self) -> bool {
        matches!(self, LlmCheckState::Ok { .. })
    }
}

// ---------------------------------------------------------------------------
// App-интеграция: attach + обработка AppEvent::LlmTask
// ---------------------------------------------------------------------------

impl App {
    /// Подключить executor (натив: `LlmExecutor::spawn(proxy-notifier)` из
    /// `main.rs`; W3/wasm: `from_driver(spawn_local-драйвер)` из canvas-web).
    pub fn attach_llm_executor(&mut self, executor: Arc<LlmExecutor>) {
        self.llm_executor = executor;
    }

    /// FR-LLM-D-W2: единый приёмник результатов LLM-задач (из `user_event`).
    /// Диспетчеризация по исходу: агент-панель / graph builder / health-чеки
    /// / suggest mm. События с отставшими поколениями отбрасываются внутри
    /// целевых обработчиков (паттерн `on_suggest_ready`).
    pub(super) fn on_llm_task(&mut self, outcome: Arc<LlmTaskOutcome>) {
        match &*outcome {
            LlmTaskOutcome::Agent(result) => {
                self.agent_llm_finished(result.clone());
            }
            #[cfg(feature = "l1-llm")]
            LlmTaskOutcome::Graph(result) => {
                self.graph_builder_llm_finished(result.clone());
            }
            LlmTaskOutcome::Health { target, result } => {
                self.llm_check_finished(*target, result.clone());
            }
            LlmTaskOutcome::SuggestMm {
                target,
                generation,
                result,
            } => {
                let target = *target;
                let generation = *generation;
                match result {
                    Ok(mm) => {
                        // Cost (Q4): успешный запрос — инкремент счётчиков
                        // ДО проверки поколения (запрос реально был оплачен).
                        self.ai_cost_session += mm.cost_usd;
                        self.ai_cost_day += mm.cost_usd;
                        let answers = (*mm.answers).clone();
                        match target {
                            crate::suggest::SuggestTarget::Popup => {
                                self.on_suggest_ready(generation, answers)
                            }
                            crate::suggest::SuggestTarget::Cards => {
                                self.on_cards_ready(generation, answers)
                            }
                        }
                    }
                    // Ошибка/таймаут — тихая деградация: lex-ответ уже показан
                    // (fusion(lex, ∅) = lex), журнал — debug-уровень (как
                    // мёртвый suggest-воркер).
                    Err(err) => {
                        tracing::debug!(%err, "suggest mm: деградация в lex");
                        self.request_redraw();
                    }
                }
            }
            LlmTaskOutcome::Panicked => {
                // Паника задания: сброс busy-флагов, панели возвращаются в
                // рабочее состояние (кнопки разблокированы).
                self.agent_panel.busy = false;
                self.graph_builder.busy = false;
                self.llm_check_reset_busy();
                self.request_redraw();
            }
        }
    }

    /// Сброс «проверяется»-состояний health-чеков (при панике задания).
    fn llm_check_reset_busy(&mut self) {
        if self.ai_key_check == LlmCheckState::Checking {
            self.ai_key_check = LlmCheckState::Idle;
        }
        if self.ai_selfhost_check == LlmCheckState::Checking {
            self.ai_selfhost_check = LlmCheckState::Idle;
        }
    }

    /// Health-чек завершён: обновить состояние строки + бейдж-флаги.
    /// Флаги `ai_key_ok`/`ai_selfhost_ok` (совместимость рендера бейджей)
    /// выставляются из исхода; текст ошибки — i18n-обёртка над деталью
    /// `LlmError` (сам детальный текст не переводим — он из провайдера).
    fn llm_check_finished(
        &mut self,
        target: HealthCheckTarget,
        result: Result<HealthReport, String>,
    ) {
        let state = match result {
            Ok(report) => LlmCheckState::Ok {
                models: report.models_count,
            },
            Err(err) => LlmCheckState::Error(err),
        };
        match target {
            HealthCheckTarget::ApiKey => {
                self.ai_key_ok = state.is_ok();
                self.ai_key_check = state;
            }
            HealthCheckTarget::Selfhost => {
                self.ai_selfhost_ok = state.is_ok();
                self.ai_selfhost_check = state;
            }
        }
        self.request_redraw();
    }
}

// ---------------------------------------------------------------------------
// FR-LLM-D-W2: хелперы LLM-задач (l1-llm)
// ---------------------------------------------------------------------------

/// FR-LLM-D-W2 (llm-waves §3.2): curated-подмножество MCP-инструментов для
/// агент-панели (PRD-0010 F-4). Полный реестр (42) — тяжёлый промпт на запрос;
/// агенту достаточно создания/чтения/валидации. Конверсия — из
/// `canvas_mcp::tools_list()` (единый источник истины реестра).
#[cfg(feature = "l1-llm")]
pub(crate) const AGENT_TOOL_NAMES: &[&str] = &[
    "canvas_info",
    "nodes_list",
    "node_create_note",
    "node_create_file",
    "template_instantiate",
    "node_edit",
    "group_create",
    "edge_create",
    "edge_delete",
    "graph_validate",
];

/// FR-LLM-D-W2: ToolDef-список инструментов агент-панели.
#[cfg(feature = "l1-llm")]
pub(crate) fn agent_tools() -> Vec<canvas_llm::ToolDef> {
    mcp_tools_as_tooldefs(AGENT_TOOL_NAMES)
}

/// FR-LLM-D-W2: конверсия MCP-реестра (`canvas_mcp::tools_list`, JSON) в
/// `Vec<canvas_llm::ToolDef>`. `keep` — фильтр по именам (пустой = всё).
/// Повреждённые записи пропускаются тихо (реестр покрыт контракт-тестом
/// canvas-mcp — на живом дереве не бывает).
#[cfg(feature = "l1-llm")]
pub(crate) fn mcp_tools_as_tooldefs(keep: &[&str]) -> Vec<canvas_llm::ToolDef> {
    let list = canvas_mcp::tools_list();
    let Some(tools) = list.get("tools").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tools
        .iter()
        .filter(|t| {
            let name = t
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            keep.is_empty() || keep.contains(&name)
        })
        .filter_map(|t| {
            let name = t.get("name").and_then(serde_json::Value::as_str)?;
            let description = t
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let input_schema = t
                .get("inputSchema")
                .map(canvas_llm::JsonVal::from_serde)
                .unwrap_or_else(canvas_llm::JsonVal::null);
            Some(canvas_llm::ToolDef::new(name, description, input_schema))
        })
        .collect()
}

/// FR-LLM-D-W2: тариф активной модели провайдера (для `actual_cost`, Q4).
/// `None` — free/local (Ollama) → cost 0.0.
#[cfg(feature = "l1-llm")]
pub(crate) fn provider_pricing(
    provider: &dyn canvas_llm::LlmProvider,
) -> Option<canvas_llm::Pricing> {
    let active = provider.active_model();
    provider
        .models()
        .iter()
        .find(|m| m.id == active)
        .and_then(|m| m.pricing)
}

/// FR-LLM-D-W2: обёртка `Box<dyn LlmProvider>` под generic-движки
/// (`GraphBuilder<P>` / `LlmMmSource<P>` — P: LlmProvider; blanket-impl для
/// Box в canvas-llm нет, трейт менять нельзя — волна владеет только
/// canvas-app). Все методы делегируют внутреннему Box. async-trait 0.1 уже
/// в дереве за l1-llm (canvas-llm/dep:async-trait) — новых крейтов нет.
#[cfg(feature = "l1-llm")]
pub(crate) struct BoxedProvider(pub Box<dyn canvas_llm::LlmProvider>);

#[cfg(feature = "l1-llm")]
#[async_trait::async_trait]
impl canvas_llm::LlmProvider for BoxedProvider {
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
        messages: &[canvas_llm::Message],
        opts: &canvas_llm::ChatOpts,
    ) -> Result<String, canvas_llm::LlmError> {
        self.0.chat(messages, opts).await
    }
    async fn choice(
        &self,
        document: &str,
        options: &[canvas_llm::OptionDesc],
    ) -> Result<canvas_llm::ChoiceAnswer, canvas_llm::LlmError> {
        self.0.choice(document, options).await
    }
    async fn tool_calling(
        &self,
        messages: &[canvas_llm::Message],
        tools: &[canvas_llm::ToolDef],
        opts: &canvas_llm::ToolCallingOpts,
    ) -> Result<Vec<canvas_llm::ToolCall>, canvas_llm::LlmError> {
        self.0.tool_calling(messages, tools, opts).await
    }
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, canvas_llm::LlmError> {
        self.0.embed(texts).await
    }
    async fn health(&self) -> Result<(), canvas_llm::LlmError> {
        self.0.health().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Стаб-драйвер отклоняет задания (wasm до W3).
    #[test]
    fn stub_driver_rejects_jobs() {
        let executor = LlmExecutor::stub();
        let job: LlmJob = Box::new(|| AppEvent::ThumbsReady);
        assert!(!executor.submit(job));
    }

    /// Default executor = stub.
    #[test]
    fn default_is_stub() {
        let e: LlmExecutor = LlmExecutor::default();
        let job: LlmJob = Box::new(|| AppEvent::ThumbsReady);
        assert!(!e.submit(job));
    }

    /// from_driver прокидывает драйвер.
    struct OkDriver;
    impl LlmExecutorDriver for OkDriver {
        fn submit(&self, _job: LlmJob) -> bool {
            true
        }
    }

    #[test]
    fn from_driver_uses_driver() {
        let e = LlmExecutor::from_driver(Arc::new(OkDriver));
        let job: LlmJob = Box::new(|| AppEvent::ThumbsReady);
        assert!(e.submit(job));
    }

    /// Нативный спавн: задание исполняется в воркере, результат приходит
    /// через notifier (проверяем канал + дожидаюсь доставки).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_spawn_executes_job() {
        let (tx, rx) = mpsc::channel::<AppEvent>();
        let notifier: LlmNotifier = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let executor = LlmExecutor::spawn(notifier);
        let job: LlmJob = Box::new(|| {
            let two = std::thread::current().name().map(|n| n.to_owned());
            AppEvent::ImeCommit(two.unwrap_or_default())
        });
        assert!(executor.submit(job));
        // Время на исполнение (воркер отдельный поток).
        let event = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("событие доставлено");
        match event {
            AppEvent::ImeCommit(name) => assert_eq!(name, "llm-executor"),
            _ => panic!("ожидался ImeCommit-носитель"),
        }
    }

    /// Паника задания не роняет воркер: доставляется Panicked, воркер жив.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_worker_survives_panic() {
        let (tx, rx) = mpsc::channel::<AppEvent>();
        let notifier: LlmNotifier = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        let executor = LlmExecutor::spawn(notifier);
        let panicking: LlmJob = Box::new(|| panic!("boom"));
        assert!(executor.submit(panicking));
        match rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("доставка Panicked")
        {
            AppEvent::LlmTask(outcome) => assert!(matches!(*outcome, LlmTaskOutcome::Panicked)),
            _ => panic!("ожидался LlmTask(Panicked)"),
        }
        // Воркер жив: следующее задание исполняется.
        let ok_job: LlmJob = Box::new(|| AppEvent::ThumbsReady);
        assert!(executor.submit(ok_job));
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(AppEvent::ThumbsReady)
        ));
    }

    /// LlmCheckState::is_ok — только Ok.
    #[test]
    fn check_state_is_ok_semantics() {
        assert!(!LlmCheckState::Idle.is_ok());
        assert!(!LlmCheckState::Checking.is_ok());
        assert!(LlmCheckState::Ok { models: None }.is_ok());
        assert!(!LlmCheckState::Error("x".into()).is_ok());
    }
}
