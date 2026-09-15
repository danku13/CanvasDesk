//! Single-instance handoff (T15-relaunch): эксклюзивный мьютекс + exit-
//! событие. Повторный запуск CanvasDesk (в т.ч. перезапуск на --desktop из
//! меню канваса) закрывает работающий инстанс перед стартом нового.
//!
//! Зачем: рантайм-вход в desktop-режим невозможен in-place — рендерер в
//! оконном режиме создан на Vulkan, а Vulkan-swapchain не презентует в
//! ребёнка Progman (см. resumed()/Renderer::new prefer_dx12). Вход в режим
//! теперь = перезапуск себя с флагом --desktop. Новый процесс обязан
//! дождаться смерти старого (иначе два канваса на одном default.canvas,
//! pipe \\.\pipe\canvasdesk и thumb-кэше). ОС-мьютекс решает это надёжно:
//! освобождается сам при смерти процесса — включая kill -9 (в отличие от
//! lock-файлов, зависание старого инстанса не блокирует новый навсегда —
//! WAIT_ABANDONED отдаёт владение).
//!
//! Протокол запуска (main(), до загрузки сцены/конфига):
//! 1. `signal_exit()` — будит exit-листенер работающего инстанса (если есть):
//!    тот шлёт AppEvent::InstanceExit → штатный выход (форс-сейв сцены,
//!    восстановление иконок);
//! 2. `InstanceGuard::acquire(WAIT_MS)` — ждём освобождения мьютекса;
//!    удерживаем до конца процесса (Drop: ReleaseMutex + CloseHandle).
//!
//! Инстанс БЕЗ слушателя (старая сборка) не держит мьютекс и не реагирует
//! на сигнал: деградация — второй инстанс стартует рядом (R14).

use windows::core::w;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, ReleaseMutex, SetEvent, WaitForSingleObject,
    EVENT_MODIFY_STATE, INFINITE,
};

/// Имя ОС-мьютекса эксклюзивности (Session-local: у каждой сессии быстрого
/// переключения пользователей — свой рабочий стол и свой инстанс).
const MUTEX_NAME: windows::core::PCWSTR = w!("Local\\CanvasDesk.SingleInstance.Mutex");

/// Имя auto-reset события «завершись»: новый запуск сигналит его,
/// exit-листенер работающего инстанса будит event loop → штатный выход.
const EXIT_EVENT_NAME: windows::core::PCWSTR = w!("Local\\CanvasDesk.InstanceExit.Event");

/// Сколько ждать смерти предыдущего инстанса после exit-сигнала (мс).
/// Штатный выход — форс-сейв маленького .canvas + restore иконок: секунды;
/// 10 с — большой запас. Таймаут — не фатально: запускаемся вторым (warn).
pub const SINGLE_INSTANCE_WAIT_MS: u32 = 10_000;

/// Владение ОС-мьютексом эксклюзивности. Живёт до конца процесса (binding
/// в main()); при смерти процесса ОС снимает мьютекс сама (и для ждущего
/// возвращает WAIT_ABANDONED — владение всё равно передаётся).
pub struct InstanceGuard {
    handle: HANDLE,
}

impl InstanceGuard {
    /// Захватить эксклюзивность: открыть/создать мьютекс и ждать до
    /// `wait_ms` освобождения предыдущим владельцем. None — таймаут или
    /// провал Win32 (запуск продолжается вторым инстансом — деградация R14,
    /// решение за пользователем; лог warn — у вызывающего).
    pub fn acquire(wait_ms: u32) -> Option<InstanceGuard> {
        // SAFETY: имя — NUL-терминированный литерал w!(); SECURITY_ATTRIBUTES
        // None — дефолтный дескриптор; bInitialOwner=false — владение только
        // через WaitForSingleObject (иначе гонка с уже умирающим владельцем).
        let handle = unsafe { CreateMutexW(None, false, MUTEX_NAME) }.ok()?;
        // SAFETY: handle только что создан/открыт этим потоком.
        let wait = unsafe { WaitForSingleObject(handle, wait_ms) };
        if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
            // WAIT_ABANDONED — предыдущий владелец умер, не освободив мьютекс
            // (kill -9 / краш): владение передано нам, канвас-состояние на
            // диске консистентно (форс-сейв только при штатном выходе; при
            // краше sentinel восстановит иконки на следующем старте).
            Some(InstanceGuard { handle })
        } else {
            if wait != WAIT_TIMEOUT {
                tracing::warn!(
                    wait = wait.0,
                    "single_instance: неожиданный код ожидания мьютекса"
                );
            }
            // SAFETY: handle открыт выше и не передаётся никому дальше.
            unsafe {
                let _ = CloseHandle(handle);
            }
            None
        }
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // ReleaseMutex формально нужен только при живом процессе с захваченным
        // мьютексом (смерть процесса снимает владение сама); Err игнорируем.
        // SAFETY: handle захвачен acquire и жив до конца drop.
        unsafe {
            let _ = ReleaseMutex(self.handle);
            let _ = CloseHandle(self.handle);
        }
    }
}

/// Сигнализировать работающему инстансу «завершись». false — слушателя нет
/// (инстанс не запущен или это старая сборка без exit-листенера).
pub fn signal_exit() -> bool {
    // SAFETY: имя — литерал w!(); EVENT_MODIFY_STATE — достаточно для SetEvent.
    let event = match unsafe { OpenEventW(EVENT_MODIFY_STATE, false, EXIT_EVENT_NAME) } {
        Ok(event) => event,
        // События нет: либо никто не работает, либо инстанс старой сборки
        // (без листенера) — сигнализировать некому, это не ошибка потока.
        Err(_) => return false,
    };
    // SAFETY: event открыт выше этим потоком.
    let ok = unsafe { SetEvent(event) }.is_ok();
    // SAFETY: event больше не используется.
    unsafe {
        let _ = CloseHandle(event);
    }
    ok
}

/// HANDLE не реализует Send (сырой указатель), но событие — потокобезопасный
/// ОС-объект: WaitForSingleObject по нему валиден из любого потока (событие
/// создаётся в вызывающем потоке ДО спавна — между main() и потоком нет
/// окна, когда сигнал нового запуска мог бы потеряться). Обёртка — единственный
/// способ перенести хэндл в Builder::spawn (маркер-тип без dropped-семантики:
/// CloseHandle делает сам поток при выходе).
struct SendHandle(HANDLE);
// SAFETY: HANDLE события — указатель на объект ядра, не на память потока;
// ожидание/сброс события потокобезопасны по документации Win32.
unsafe impl Send for SendHandle {}

/// Поднять exit-листенер: фоновый поток ждёт сигнал события и вызывает
/// `wake()` на каждый сигнал (auto-reset: один сигнал — одно пробуждение,
/// каждый новый запуск сигналит отдельно). Поток живёт до смерти процесса.
///
/// `wake()` — typically EventLoopProxy::send_event(AppEvent::InstanceExit):
/// поток не трогает GUI/состояние приложения, только будит event loop.
/// Err — провал CreateEventW или спавна потока (текст для лога).
pub fn spawn_exit_listener(wake: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
    // Auto-reset (bManualReset=false), изначально несигнальное. Имя общее:
    // CreateEventW вернёт хэндл существующего события, если оно живо (маловероятно:
    // создатель — предыдущий инстанс, он умирает до нашего старта).
    // SAFETY: имя — литерал w!(); SECURITY_ATTRIBUTES None.
    let event = unsafe { CreateEventW(None, false, false, EXIT_EVENT_NAME) }
        .map_err(|err| format!("CreateEventW({EXIT_EVENT_NAME:?}): {err}"))?;
    let event = SendHandle(event); // HANDLE не Send — переносим в обёртке
    let spawned = std::thread::Builder::new()
        .name("single-instance-exit".to_owned())
        .spawn(move || {
            // Цельное связывание ДО деструктуризации — отключение disjoint
            // capture (Rust 2021): паттерн `let SendHandle(event) = event;`
            // иначе захватил бы в замыкание ТОЛЬКО поле .0 (сырой HANDLE,
            // не Send — E0277), минуя Send-обёртку. Теневое `let event =
            // event;` — паттерн-связывание целиком: захватывается весь
            // SendHandle (Send через unsafe impl), деструктуризация дальше
            // работает уже с локальной копией.
            let event = event;
            let SendHandle(event) = event; // HANDLE обратно из Send-обёртки
            loop {
                // SAFETY: event валиден (создан до спавна, живёт в замыкании);
                // INFINITE — поток будится SetEvent'ом нового запуска.
                let wait = unsafe { WaitForSingleObject(event, INFINITE) };
                if wait != WAIT_OBJECT_0 {
                    // WAIT_FAILED (умирающий процесс на этапе выхода) — выходим;
                    // WAIT_TIMEOUT невозможен при INFINITE, WAIT_ABANDONED — не
                    // для событий. Ошибка — листенер мёртв: повторные запуски не
                    // закроют этот инстанс сигналом (деградация, warn в лог).
                    tracing::warn!(
                        wait = wait.0,
                        "exit-листенер: ожидание прервано — сигнал завершения больше не принимается"
                    );
                    break;
                }
                tracing::info!("exit-сигнал от нового запуска — штатное завершение (handoff)");
                wake();
            }
            // SAFETY: event больше никому не нужен (поток умирает).
            unsafe {
                let _ = CloseHandle(event);
            }
        })
        .map_err(|err| format!("spawn exit-listener: {err}"))?;
    drop(spawned); // JoinHandle не нужен — поток живёт до конца процесса
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MUTEX_NAME/EXIT_EVENT_NAME — валидные NUL-терминированные UTF-16
    /// (контракт w!): последний элемент буфера — 0. Проверяем дёшево на
    /// любой ОС: константы — PCWSTR (тонкая обёртка указателя), сам буфер
    /// статический; здесь только смоук, что модуль импортируется и имена
    /// не пересекаются.
    #[test]
    fn names_are_distinct() {
        assert_ne!(
            MUTEX_NAME.as_ptr(),
            EXIT_EVENT_NAME.as_ptr(),
            "имена мьютекса и события обязаны отличаться"
        );
    }

    /// acquire на несуществующем владельце должен вернуть Some мгновенно
    /// только на Windows (ОС-объекты); на других ОС модуль не собирается —
    /// тест-заглушка для симметрии с остальными чистыми модулями.
    #[cfg(windows)]
    #[test]
    fn acquire_immediate_when_free() {
        let guard = InstanceGuard::acquire(0);
        assert!(guard.is_some(), "свободный мьютекс должен захватываться");
        drop(guard);
        // Повторный захват после release — тоже успешен (идемпотентность).
        assert!(InstanceGuard::acquire(0).is_some());
    }
}
