//! FR-064 P1 (ADR-0008 M3/S2): сценарный воркер — вынос тяжёлого пересчёта
//! [`flow::propagate_with_lines`] с главного (UI) треда на отдельный поток
//! с двойной буферизацией результатов (архдок `math-computing-stack.md`
//! §5.2, вариант P1). Рецепт — план волны S §2.2: `std::thread::spawn` +
//! `std::sync::mpsc`, wake-up UI-треда — существующий паттерн
//! `EventLoopProxy<AppEvent>` (образец `McpPipeServer::spawn`,
//! `ThumbService::spawn`). Модуль desktop-only: на `wasm32-wasip1`
//! `std::thread` неработоспособен (контракт плана §5.8), сцена выполняет
//! sync-пересчёт на вызывающем треде (см. [`crate::scene::SceneState`]).
//!
//! Контракты FR-064:
//! - сигнатура `flow::propagate_with_lines` НЕ меняется (план §5.1) —
//!   воркер только ВЫЗЫВАЕТ её (`flow.rs` не трогается);
//! - без новых зависимостей (архдок §5.2) — только std;
//! - деградация = sync-пересчёт + `warn` (правило `AGENTS.md`
//!   «фолбэк + warn»), не молчаливое падение и не зависший UI;
//! - live-инвариант: правка → результат в пределах 1–2 кадров.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use canvas_core::flow::{self, CycleError, FlowSolutions, WhatIfOverrides};
use canvas_core::Canvas;

/// FR-064 P1: вид запрошенного снимка решений — BASELINE (чистый пересчёт
/// без подмен, источник дельт) или ACTIVE (пересчёт с подменами активного
/// what-if сценария). Имя совпадает с планом (`FlowKind`); с
/// `canvas_core::flow::FlowKind` (тип value-рёбра) не конфликтует — другой
/// модуль.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// Базовый пересчёт (`WhatIfOverrides::default()`).
    Baseline,
    /// Пересчёт с подменами активного сценария.
    Active,
}

/// Задание воркеру: снимок канваса (Arc — воркер не видит мутаций UI-треда),
/// подмены, вид снимка и поколение запроса (ответы устаревших поколений
/// UI-тред отбрасывает — правки чаще 1/кадр не копятся в очереди).
pub struct FlowJob {
    pub generation: u64,
    pub kind: FlowKind,
    pub canvas: Arc<Canvas>,
    pub whatif: WhatIfOverrides,
}

/// Результат воркера. Авторитетный канал данных: UI-тред забирает исходы
/// в [`SceneState::complete_flow_recompute`] (полезная нагрузка
/// `AppEvent::FlowReady` — информационный снимок для wake-up, истина здесь).
pub enum FlowOutcome {
    /// Пересчёт завершён: готовый снимок или цикл value-рёбер.
    Done {
        generation: u64,
        kind: FlowKind,
        solutions: Result<FlowSolutions, CycleError>,
    },
    /// Паника вычисления (поймана `catch_unwind`): воркер ЖИВ и продолжает
    /// принимать задания; UI-тред делает sync-фолбэк + `warn`.
    Panicked { generation: u64, kind: FlowKind },
}

/// FR-064 P1: уведомитель UI-треда — `(вид, снимок)`; приложение шлёт через
/// него `AppEvent::FlowReady { solutions, kind }` по `EventLoopProxy`
/// (паттерн ThumbService/McpPipe). При цикле/панике уведомление приходит с
/// пустым снимком-заполнителем (`FlowSolutions::default()`) —payload
/// информационный, исход ошибки забирается из канала исходов.
pub type FlowNotifier = Arc<dyn Fn(FlowKind, Arc<FlowSolutions>) + Send + Sync>;

/// Тип вычислителя — инъекция для fallback-теста (паника воркера → sync-
/// результат побитово идентичен). Production — [`flow::propagate_with_lines`].
pub type FlowCompute =
    Arc<dyn Fn(&Canvas, &WhatIfOverrides) -> Result<FlowSolutions, CycleError> + Send + Sync>;

/// Хэндл сценарного воркера: отправка заданий + дренаж исходов. Один воркер
/// обрабатывает задания FIFO (детерминизм: порядок ответов = порядок
/// запросов); дроп хэндла закрывает канал заданий — поток завершается сам.
pub struct FlowWorkerHandle {
    jobs: mpsc::Sender<FlowJob>,
    outcomes: Arc<Mutex<mpsc::Receiver<FlowOutcome>>>,
}

impl FlowWorkerHandle {
    /// Запустить воркер (по образцу `McpPipeServer::spawn`): поток-цикл
    /// забирает задания, считает [`flow::propagate_with_lines`], публикует
    /// исход в канал и будит UI-тред через `notifier`. Провал спавна потока
    /// возвращает хэндл с закрытым каналом — первый же `request` вернёт
    /// false и сцена уйдёт в sync-фолбэк + `warn` (деградация, не паника).
    pub fn spawn(notifier: FlowNotifier) -> Self {
        Self::spawn_with_compute(Arc::new(flow::propagate_with_lines), notifier)
    }

    /// То же с явным вычислителем (fallback-тест «воркер паникует»).
    pub fn spawn_with_compute(compute: FlowCompute, notifier: FlowNotifier) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<FlowJob>();
        let (out_tx, out_rx) = mpsc::channel::<FlowOutcome>();
        let spawned = std::thread::Builder::new()
            .name("flow-worker".to_owned())
            .spawn(move || {
                // Паника вычисления ловится ПО ЗАДАНИЮ: поток переживает её
                // (не тащим отравленный lock через join) и продолжает лоб.
                while let Ok(job) = job_rx.recv() {
                    let outcome =
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            (compute)(&job.canvas, &job.whatif)
                        })) {
                            Ok(solutions) => FlowOutcome::Done {
                                generation: job.generation,
                                kind: job.kind,
                                solutions,
                            },
                            Err(_) => FlowOutcome::Panicked {
                                generation: job.generation,
                                kind: job.kind,
                            },
                        };
                    // Плейсхолдер-снимок для wake-up при отказе: payload
                    // информационный (см. [FlowNotifier]).
                    let placeholder = Arc::new(FlowSolutions::default());
                    let (kind, snapshot) = match &outcome {
                        FlowOutcome::Done {
                            kind,
                            solutions: Ok(solutions),
                            ..
                        } => (*kind, Arc::new(solutions.clone())),
                        _ => (job.kind, placeholder),
                    };
                    if out_tx.send(outcome).is_err() {
                        // UI-тред ушёл (дроп сцены) — воркер завершается.
                        break;
                    }
                    notifier(kind, snapshot);
                }
            });
        if spawned.is_err() {
            // Поток не поднялся: канал заданий без читателя — request()
            // вернёт false, сцена выполнит sync-фолбэк + warn.
            tracing::warn!("поток flow-worker не запущен — синхронный режим пересчёта");
        }
        Self {
            jobs: job_tx,
            outcomes: Arc::new(Mutex::new(out_rx)),
        }
    }

    /// Поставить задание в очередь. `false` — воркер недоступен (канал
    /// закрыт): вызывающий обязан выполнить sync-фолбэк + `warn`.
    pub fn request(&self, job: FlowJob) -> bool {
        self.jobs.send(job).is_ok()
    }

    /// Забрать все готовые исходы (не блокируя). Вызывается ТОЛЬКО с
    /// UI-треда (один потребитель).
    pub fn take_outcomes(&self) -> Vec<FlowOutcome> {
        let rx = self
            .outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }
}
