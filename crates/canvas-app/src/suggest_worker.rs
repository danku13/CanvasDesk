//! FR-079 (S3): SuggestWorker — фоновый поток ранжирования подсказок.
//!
//! Паттерн FR-064 flow-worker (`canvas-scene/src/worker.rs`): `std::thread`
//! + `mpsc`, wake-up UI-треда — `EventLoopProxy<AppEvent>` (образец
//! `ThumbService::spawn`). Desktop-only: на wasm потоков нет — сцена
//! считает синхронно в `about_to_wait` (lex < 1 мс на каталоге 62 опций).
//!
//! Контракты:
//! - в задании `generation` (счётчик триггеров), UI-тред отбрасывает
//!   ответы устаревших поколений — правки чаще дебаунса не копятся;
//! - паника вычисления ловится ПО ЗАДАНИЮ (поток жив), ответ — пустой
//!   список (попап остаётся с L0 — правило «фолбэк + warn»);
//! - гибрид = композиция в воркере: lex → [L1 mm, feature `l1-laya`] →
//!   fusion → Platt; транспорт упал → fusion(lex, ∅) = lex (S2-инвариант).

use std::sync::mpsc;
use std::sync::Arc;
#[cfg(feature = "l1-laya")]
use std::time::Duration;

#[cfg(feature = "l1-laya")]
use canvas_core::SuggestEngineKind;
use canvas_core::SuggestSettings;
use canvas_suggest::types::OptionDesc;

use super::suggest::{rank, MmProbe, SuggestAnswer, SuggestTarget};

/// Задание воркеру: сериализованный контекст + шортлист-опции + снапшот
/// настроек (UI-тред не мутирует данные воркера — владение переносом).
pub struct SuggestJob {
    pub target: SuggestTarget,
    pub generation: u64,
    pub document: String,
    pub options: Arc<Vec<OptionDesc>>,
    pub settings: SuggestSettings,
}

/// Уведомитель UI-треда: `(цель, поколение, ответы)`; приложение шлёт
/// через него `AppEvent::SuggestReady` по `EventLoopProxy` (паттерн
/// FlowNotifier).
pub type SuggestNotifier = Arc<dyn Fn(SuggestTarget, u64, Arc<Vec<SuggestAnswer>>) + Send + Sync>;

/// L1-состояние воркера (только сборка с feature `l1-laya`).
#[cfg(feature = "l1-laya")]
struct L1State {
    client: canvas_suggest::laya::LayaClient,
    /// Sidecar, поднятый самим приложением (конфиг-команда); `None` —
    /// внешний процесс (режим detect v1: пользователь запустил сам).
    sidecar: Option<canvas_suggest::laya::Sidecar>,
    /// «L1 недоступен» — warn один раз на серию отказов (план §3).
    warned: bool,
}

/// Хэндл suggest-воркера: отправка заданий; дроп закрывает канал — поток
/// завершается сам (Sidecar при дропе убивается — S2-контракт).
pub struct SuggestWorkerHandle {
    jobs: mpsc::Sender<SuggestJob>,
}

impl SuggestWorkerHandle {
    /// Запустить воркер. Провал спавна потока возвращает хэндл с закрытым
    /// каналом — первый же `request` вернёт false, приложение деградирует
    /// на lex-синхронный путь + `warn`.
    pub fn spawn(notifier: SuggestNotifier) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<SuggestJob>();
        let _spawned = std::thread::Builder::new()
            .name("suggest-worker".to_owned())
            .spawn(move || {
                #[cfg(feature = "l1-laya")]
                let mut l1: Option<L1State> = None;
                while let Ok(job) = job_rx.recv() {
                    let target = job.target;
                    let generation = job.generation;
                    let answers = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[allow(unused_mut)]
                        let mut mm: Option<MmProbe> = None;
                        #[cfg(feature = "l1-laya")]
                        if job.settings.enabled && job.settings.engine == SuggestEngineKind::LexLaya
                        {
                            mm = mm_probe(&mut l1, &job);
                        }
                        let _ = &mut mm;
                        rank(&job.document, &job.options, &job.settings, mm.as_ref())
                    }))
                    .unwrap_or_default();
                    (notifier)(target, generation, Arc::new(answers));
                }
            });
        Self { jobs: job_tx }
    }

    /// Отправить задание; false — воркер мёртв (канал закрыт).
    pub fn request(&self, job: SuggestJob) -> bool {
        self.jobs.send(job).is_ok()
    }
}

/// L1-проба mm: sidecar lifecycle + choice-запрос. Любой отказ → `None`
/// (деградация lex) + warn один раз на серию.
#[cfg(feature = "l1-laya")]
fn mm_probe(l1: &mut Option<L1State>, job: &SuggestJob) -> Option<MmProbe> {
    let laya = &job.settings.laya;
    if l1.is_none() {
        let client = canvas_suggest::laya::LayaClient::new(
            laya.endpoint.clone(),
            laya.model.clone(),
            laya.timeout_ms,
        );
        let sidecar = if laya.command.is_empty() {
            None // внешний sidecar (detect v1): пробуем endpoint как есть
        } else {
            match spawn_and_wait(laya) {
                Ok(sc) => Some(sc),
                Err(err) => {
                    warn_once(
                        l1,
                        || tracing::warn!(%err, command = %laya.command, "sidecar не поднялся — L1 деградирует на lex"),
                    );
                    return None;
                }
            }
        };
        *l1 = Some(L1State {
            client,
            sidecar,
            warned: false,
        });
    }
    let state = l1.as_mut().expect("l1 инициализирован выше");
    // idle-shutdown: простаивающий sidecar выгружен — поднять заново
    let idle_limit = Duration::from_secs(state_idle_limit(&job.settings));
    if let Some(sc) = state.sidecar.as_mut() {
        if !sc.is_running() || sc.shutdown_if_idle(idle_limit) {
            sc.shutdown();
            state.sidecar = None;
        }
    }
    if state.sidecar.is_none() && !job.settings.laya.command.is_empty() {
        match spawn_and_wait(&job.settings.laya) {
            Ok(sc) => state.sidecar = Some(sc),
            Err(err) => {
                warn_once(
                    l1,
                    || tracing::warn!(%err, "respawn sidecar не удался — L1 деградирует на lex"),
                );
                return None;
            }
        }
    }
    if let Some(sc) = state.sidecar.as_mut() {
        sc.touch();
    }
    match state.client.choice(&job.document, &job.options) {
        Ok(answer) => {
            if state.warned {
                tracing::info!("L1 снова доступен");
            }
            state.warned = false;
            Some(MmProbe {
                probs: answer.probs,
                confidence: answer.confidence,
            })
        }
        Err(err) => {
            warn_once(
                l1,
                || tracing::warn!(%err, endpoint = %job.settings.laya.endpoint, "L1 недоступен — деградация на lex"),
            );
            None
        }
    }
}

/// Idle-лимит с конфига (0 — не выключать; Minutes-масштаб не нужен).
#[cfg(feature = "l1-laya")]
fn state_idle_limit(settings: &SuggestSettings) -> u64 {
    if settings.laya.idle_shutdown_s == 0 {
        u64::MAX
    } else {
        settings.laya.idle_shutdown_s
    }
}

/// Spawn sidecar + health-wait (cold start до 60 с — план §5.1; UI не
/// блокируется: ожидание живёт в этом потоке, попап работает на lex).
#[cfg(feature = "l1-laya")]
fn spawn_and_wait(
    laya: &canvas_core::LayaSettings,
) -> Result<canvas_suggest::laya::Sidecar, String> {
    let mut command = laya.command.split_whitespace();
    let bin = command.next().unwrap_or_default();
    if bin.is_empty() {
        return Err("пустая команда sidecar".to_owned());
    }
    let args: Vec<String> = command.map(|s| s.to_owned()).collect();
    let cfg = canvas_suggest::laya::SidecarConfig::new(bin).with_args(args);
    let mut sidecar = canvas_suggest::laya::Sidecar::spawn(&cfg).map_err(|e| e.to_string())?;
    let client = canvas_suggest::laya::LayaClient::new(
        &laya.endpoint,
        &laya.model,
        laya.timeout_ms.max(2_000),
    );
    // health-wait: до 60 с с шагом 1 с (cold start чекпойнта, план §5.1)
    for _ in 0..60 {
        if !sidecar.is_running() {
            return Err("sidecar умер при старте".to_owned());
        }
        if client.health().is_ok() {
            return Ok(sidecar);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    Err("health-wait таймаут 60 с".to_owned())
}

/// Warn один раз на серию отказов (сбрасывается успехом).
#[cfg(feature = "l1-laya")]
fn warn_once(l1: &mut Option<L1State>, f: impl FnOnce()) {
    if let Some(state) = l1.as_mut() {
        if !state.warned {
            state.warned = true;
            f();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Воркер отвечает и завершается: дроп хэндла закрывает канал.
    /// Паника вычисления не роняет поток и не будит UI мусором.
    #[test]
    fn worker_answers_and_generation_passes() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Mutex;

        let seen = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(AtomicU32::new(0));
        let notifier: SuggestNotifier = {
            let seen = Arc::clone(&seen);
            let calls = Arc::clone(&calls);
            Arc::new(move |target, gen, answers| {
                seen.lock().unwrap().push((target, gen, answers.len()));
                calls.fetch_add(1, Ordering::SeqCst);
            })
        };
        let handle = SuggestWorkerHandle::spawn(notifier);
        let options: Arc<Vec<OptionDesc>> = Arc::new(vec![
            OptionDesc::new("lb", "Балансировщик: распределяет трафик"),
            OptionDesc::new("none", "ни один шаблон не подходит"),
        ]);
        assert!(handle.request(SuggestJob {
            target: SuggestTarget::Popup,
            generation: 7,
            document: "балансировщик rps".to_owned(),
            options,
            settings: SuggestSettings::default(),
        }));
        // воркер отвечает быстро; ждём с запасом
        for _ in 0..100 {
            if calls.load(Ordering::SeqCst) >= 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "ровно один ответ");
        let answers = seen.lock().unwrap();
        assert_eq!(
            answers[0],
            (SuggestTarget::Popup, 7, 1),
            "popup-цель, generation 7, одна ИИ-строка (none отфильтрован)"
        );
        // Дроп хэндла → канал закрыт → request false (воркер завершается сам)
        drop(handle);
    }
}
