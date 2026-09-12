//! MCP named pipe сервер (Windows): транспорт между canvas-mcp-посредником
//! (stdio ↔ pipe) и запущенным canvas-app.
//!
//! Архитектура (план MCP-задачи): worker-поток держит `\\.\pipe\canvasdesk`,
//! принимает ОДНОГО клиента за раз (после разрыва — пересоздание pipe и
//! ожидание нового). Строки-запросы складываются в канал вместе с
//! ответчиком; waker будит event loop приложения (EventLoopProxy нельзя
//! держать в worker — он не `Sync`-безопасен для нашей схемы, поэтому
//! сюда передаётся generic-замыкание `Arc<dyn Fn() + Send + Sync>`).
//! Ответы приложения возвращаются через responder и пишутся writer-нитью
//! в тот же pipe.
//!
//! ВАЖНО (механика Win32): хэндл pipe открыт с FILE_FLAG_OVERLAPPED.
//! На блокирующем (не-overlapped) хэндле ядро сериализует операции —
//! висящий ReadFile в reader-нити блокировал бы WriteFile writer-нити
//! (дедлок «клиент ждёт ответ, сервер ждёт запрос»). Overlapped-хэндл
//! допускает конкурентные pending-операции по разным направлениям —
//! стандартная схема «reader-поток + writer-поток на одном pipe».
//!
//! Line-framing: сообщения разделяются `\n` (UTF-8, битые кодировки —
//! через `from_utf8_lossy`). Не Win32-специфичной логики здесь нет, кроме
//! транспорта, поэтому тест — интеграционный round-trip на реальном pipe.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tracing::{debug, warn};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_PIPE_LISTENING, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{
    ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject, INFINITE};
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};

/// Ответчик на один MCP-запрос: строка-ответ уходит обратно в pipe.
/// `FnOnce` — ответ однократен; `Send` — создаётся в worker-потоке,
/// вызывается в event loop приложения.
pub type McpResponder = Box<dyn FnOnce(String) + Send>;

/// Сервер MCP-pipe. `spawn` поднимает worker-поток; запросы забираются
/// из event loop через `take_request` (неблокирующе).
pub struct McpPipeServer {
    requests: Mutex<Receiver<(String, McpResponder)>>,
    _worker: JoinHandle<()>,
}

impl McpPipeServer {
    /// Поднять сервер на именованном pipe. `waker` вызывается в worker-потоке
    /// после каждой постановки запроса — должен будить event loop (proxy).
    /// None — не удалось создать поток (деградация: MCP недоступен, warn).
    pub fn spawn(pipe_name: &str, waker: Arc<dyn Fn() + Send + Sync>) -> Option<Self> {
        let (to_app_tx, to_app_rx) = mpsc::channel::<(String, McpResponder)>();
        let name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
        let worker = std::thread::Builder::new()
            .name("mcp-pipe".to_owned())
            .spawn(move || server_loop(&name, to_app_tx, waker));
        match worker {
            Ok(_worker) => Some(Self {
                requests: Mutex::new(to_app_rx),
                _worker,
            }),
            Err(err) => {
                warn!(%err, "MCP pipe: не удалось создать worker-поток, MCP недоступен");
                None
            }
        }
    }

    /// Забрать следующий запрос (строка JSON-RPC + ответчик). None — пока
    /// запросов нет (неблокирующий try_recv из event loop).
    pub fn take_request(&self) -> Option<(String, McpResponder)> {
        // Блокировка короткая: try_recv + Mutex; poisoned — деградация без паники
        self.requests
            .lock()
            .ok()
            .and_then(|rx| match rx.try_recv() {
                Ok(request) => Some(request),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => None,
            })
    }
}

/// Главный цикл worker-потока: создать pipe → ждать клиента → обслужить →
/// повторить. Любая ошибка создания pipe — warn и выход (деградация, R14).
fn server_loop(
    name: &[u16],
    to_app: Sender<(String, McpResponder)>,
    waker: Arc<dyn Fn() + Send + Sync>,
) {
    loop {
        let handle = match create_pipe(name) {
            Ok(handle) => handle,
            Err(err) => {
                warn!(%err, "MCP pipe: CreateNamedPipeW не удался, MCP-сервер остановлен");
                return;
            }
        };
        if !wait_for_client(handle) {
            // Ошибка ожидания — закрываем инстанс и пробуем снова с новым
            close_pipe(handle);
            continue;
        }
        debug!("MCP pipe: клиент подключён");
        serve_client(handle, &to_app, &waker);
        // Клиент отвалился (read-loop вышла): разрыв и новый цикл
        let _ = unsafe { DisconnectNamedPipe(handle) };
        close_pipe(handle);
        debug!("MCP pipe: клиент отключён, ожидаю нового");
    }
}

/// Создать именованный pipe (дуплексный, байтовый, overlapped).
fn create_pipe(name: &[u16]) -> windows::core::Result<HANDLE> {
    // SAFETY: name — NUL-терминированный UTF-16 буфер, живёт дольше вызова;
    // SECURITY_ATTRIBUTES None — дефолтный дескриптор. Хэндл валиден до
    // CloseHandle (владение переходит вызывающему).
    let handle = unsafe {
        CreateNamedPipeW(
            windows::core::PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            None,
        )
    };
    // В windows 0.62 CreateNamedPipeW возвращает "сырой" HANDLE:
    // INVALID_HANDLE_VALUE — проверяем вручную
    if handle.is_invalid() {
        Err(windows::core::Error::from_thread())
    } else {
        Ok(handle)
    }
}

/// Ручной-reset событие для overlapped-операций. None — ошибка Win32.
fn create_event() -> Option<windows::Win32::Foundation::HANDLE> {
    // CreateEventW в windows 0.62 возвращает Result; хэндл события
    // INVALID_HANDLE_VALUE не бывает — ошибку даёт сам Result.
    // SAFETY: все параметры по умолчанию; имя None — анонимное событие.
    unsafe { CreateEventW(None, true, false, None) }.ok()
}

/// Ждать подключения клиента (overlapped ConnectNamedPipe). Возвращает
/// false при ошибке (вызывающий закрывает инстанс и пересоздаёт pipe).
fn wait_for_client(handle: HANDLE) -> bool {
    let Some(event) = create_event() else {
        warn!("MCP pipe: не удалось создать событие для ConnectNamedPipe");
        return false;
    };
    let mut overlapped = OVERLAPPED {
        hEvent: event,
        ..Default::default()
    };
    // SAFETY: handle валиден; overlapped жив в стеке до GetOverlappedResult.
    let result = unsafe { ConnectNamedPipe(handle, Some(&mut overlapped)) };
    let connected = match result {
        Ok(()) => true,
        Err(err) if err.code() == ERROR_IO_PENDING.to_hresult() => {
            // SAFETY: event валиден.
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                warn!("MCP pipe: ожидание клиента прервано нештатно");
                false
            } else {
                // Дождались: результат операции — успех или реальная ошибка
                // SAFETY: overlapped завершён (событие в сигнальном состоянии).
                let mut _dummy = 0u32;
                unsafe { GetOverlappedResult(handle, &overlapped, &mut _dummy, false) }.is_ok()
            }
        }
        Err(err) => {
            // ERROR_PIPE_LISTENING — клиент подключился между CreateNamedPipeW
            // и ConnectNamedPipe: соединение валидно
            err.code() == ERROR_PIPE_LISTENING.to_hresult()
        }
    };
    close_event(event);
    connected
}

fn close_event(event: windows::Win32::Foundation::HANDLE) {
    // SAFETY: event валиден и больше не используется.
    let _ = unsafe { CloseHandle(event) };
}

/// Обслуживание одного клиентского соединения: reader — этот поток
/// (overlapped ReadFile → строки в канал приложения), writer — отдельная
/// нить (ответы из канала → overlapped WriteFile). Overlapped-хэндл
/// допускает конкурентные pending-операции по разным направлениям.
/// Выход — при ошибке чтения (разрыв) или когда приложение закрыло канал.
fn serve_client(
    handle: HANDLE,
    to_app: &Sender<(String, McpResponder)>,
    waker: &Arc<dyn Fn() + Send + Sync>,
) {
    let (to_pipe_tx, to_pipe_rx) = mpsc::channel::<String>();
    // HANDLE в windows 0.62 не Send — в нить передаём usize и собираем
    // handle обратно (передача значения, не владения: хэндл живёт, пока
    // serve_client не закроет его после join writer'а).
    let writer_handle = handle.0 as usize;
    let writer = std::thread::Builder::new()
        .name("mcp-pipe-writer".to_owned())
        .spawn(move || {
            let writer_handle = HANDLE(writer_handle as *mut core::ffi::c_void);
            let Some(event) = create_event() else {
                warn!("MCP pipe: writer без события — ответы недоступны");
                return;
            };
            while let Ok(line) = to_pipe_rx.recv() {
                let mut bytes = line.into_bytes();
                bytes.push(b'\n');
                if write_all_overlapped(writer_handle, &bytes, event).is_err() {
                    break;
                }
            }
            close_event(event);
        });
    let writer = match writer {
        Ok(writer) => writer,
        Err(err) => {
            warn!(%err, "MCP pipe: writer-поток не создан, соединение закрыто");
            close_pipe(handle);
            return;
        }
    };

    read_loop(handle, &to_pipe_tx, to_app, waker);

    // Закрываем все копии отправителя: writer увидит disconnect и выйдет
    drop(to_pipe_tx);
    let _ = writer.join();
}

/// Чтение строк из pipe: блокирующий по смыслу цикл overlapped ReadFile
/// (ожидание через WaitForSingleObject). Каждая завершённая `\n`-строка
/// уходит в канал приложения вместе с responder'ом, пишущим в `to_pipe`.
fn read_loop(
    handle: HANDLE,
    to_pipe: &Sender<String>,
    to_app: &Sender<(String, McpResponder)>,
    waker: &Arc<dyn Fn() + Send + Sync>,
) {
    let Some(event) = create_event() else {
        warn!("MCP pipe: не удалось создать событие для чтения");
        return;
    };
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        // SAFETY: event валиден; buf и overlapped живут до GetOverlappedResult.
        let _ = unsafe { ResetEvent(event) };
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        // Для overlapped ReadFile lpNumberOfBytesRead обязан быть NULL.
        let result = unsafe { ReadFile(handle, Some(&mut buf), None, Some(&mut overlapped)) };
        if let Err(err) = result {
            if err.code() != ERROR_IO_PENDING.to_hresult() {
                // ERROR_BROKEN_PIPE и т.п. — клиент ушёл
                break;
            }
            // SAFETY: event валиден; ждём бесконечно.
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                break;
            }
        }
        // Завершено (синхронно или через ожидание): забираем число байт
        let mut read = 0u32;
        // SAFETY: overlapped завершён; read — out-параметр.
        let completed = unsafe { GetOverlappedResult(handle, &overlapped, &mut read, false) };
        if completed.is_err() || read == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..read as usize]);
        let mut app_gone = false;
        for line in drain_lines(&mut pending) {
            let tx = to_pipe.clone();
            let responder: McpResponder = Box::new(move |response| {
                let _ = tx.send(response);
            });
            if to_app.send((line, responder)).is_err() {
                // Приложение завершилось — дочитывать бессмысленно
                app_gone = true;
                break;
            }
            waker();
        }
        if app_gone {
            break;
        }
    }
    close_event(event);
}

/// Вырезать завершённые `\n`-строки из буфера (хвост сохраняется до след.
/// чтения). Пустые строки пропускаются; `\r` срезается.
fn drain_lines(pending: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = pending.drain(..=pos).collect();
        let line = &line[..line.len() - 1]; // без '\n'
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        lines.push(String::from_utf8_lossy(line).into_owned());
    }
    lines
}

/// Overlapped-запись всего буфера в pipe (WriteFile возвращается сразу;
/// ждём событие операции). Ошибка — разрыв/закрытие со стороны клиента.
fn write_all_overlapped(
    handle: HANDLE,
    mut bytes: &[u8],
    event: windows::Win32::Foundation::HANDLE,
) -> std::io::Result<()> {
    while !bytes.is_empty() {
        // SAFETY: event валиден; буферы живы до GetOverlappedResult.
        let _ = unsafe { ResetEvent(event) };
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        let result = unsafe { WriteFile(handle, Some(bytes), None, Some(&mut overlapped)) };
        if let Err(err) = result {
            if err.code() != ERROR_IO_PENDING.to_hresult() {
                return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, err));
            }
            // SAFETY: event валиден.
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "wait"));
            }
        }
        let mut written = 0u32;
        // SAFETY: overlapped завершён; written — out-параметр.
        let completed = unsafe { GetOverlappedResult(handle, &overlapped, &mut written, false) };
        if completed.is_err() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "result",
            ));
        }
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "WriteFile",
            ));
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

/// Блокирующая запись на не-overlapped хэндле (тестовый клиент).
#[cfg(test)]
fn write_all_blocking(handle: HANDLE, mut bytes: &[u8]) -> std::io::Result<()> {
    while !bytes.is_empty() {
        let mut written = 0u32;
        // SAFETY: handle валиден; bytes жив до конца вызова; written — out.
        let result = unsafe { WriteFile(handle, Some(bytes), Some(&mut written), None) };
        result.map_err(|err| std::io::Error::new(std::io::ErrorKind::BrokenPipe, err))?;
        if written == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "WriteFile",
            ));
        }
        bytes = &bytes[written as usize..];
    }
    Ok(())
}

fn close_pipe(handle: HANDLE) {
    // SAFETY: handle валиден и больше нигде не используется (writer join'нут).
    let _ = unsafe { CloseHandle(handle) };
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
    use std::sync::mpsc as std_mpsc;
    use std::time::{Duration, Instant};

    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        OPEN_EXISTING,
    };

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn unique_pipe_name() -> String {
        let pid = std::process::id();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!(r"\\.\pipe\canvasdesk_test_{pid}_{n}")
    }

    /// Подключить блокирующего клиента к pipe с ретраями (сервер может ещё
    /// не создать pipe). Не-overlapped хэндл — перекрёстная проверка
    /// совместимости с простейшими клиентами.
    fn connect_with_retry(name: &str) -> HANDLE {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            // SAFETY: wide NUL-терминирован и живёт до конца вызова.
            let result = unsafe {
                CreateFileW(
                    windows::core::PCWSTR(wide.as_ptr()),
                    (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                    windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                )
            };
            match result {
                Ok(handle) => return handle,
                Err(_) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(err) => panic!("CreateFileW не удалось за 5 с: {err}"),
            }
        }
    }

    /// Round-trip на реальном pipe: клиент шлёт две строки одним write,
    /// сервер забирает два запроса, ответы возвращаются через responder.
    /// Проверяет, в том числе, конкурентные чтение/запись (overlapped):
    /// ответ пишется, пока reader ждёт следующий запрос.
    #[test]
    fn pipe_round_trip_two_lines() {
        let name = unique_pipe_name();
        let wake_count = Arc::new(AtomicUsize::new(0));
        let waker = {
            let count = Arc::clone(&wake_count);
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            })
        };
        let server = McpPipeServer::spawn(&name, waker).expect("сервер запущен");

        // Клиент в нити: подключение, запись двух строк одним буфером,
        // чтение двух ответов. Результат — обратно в тест через канал.
        // HANDLE не Send — в нить передаём usize (сборка handle внутри).
        let client_name = name.clone();
        let (client_tx, client_rx) = std_mpsc::channel::<Vec<String>>();
        let client = std::thread::spawn(move || {
            let handle = connect_with_retry(&client_name);
            let handle_usize = handle.0 as usize;
            let request = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"canvas_info\"}\n\
                           {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"nodes_list\"}\n";
            write_all_blocking(handle, request.as_bytes()).expect("запрос записан");
            let mut pending: Vec<u8> = Vec::new();
            let mut buf = [0u8; 4096];
            let mut responses = Vec::new();
            while responses.len() < 2 {
                let mut read = 0u32;
                // SAFETY: handle валиден; буферы живы до конца вызова.
                unsafe { ReadFile(handle, Some(&mut buf), Some(&mut read), None) }
                    .expect("чтение ответа");
                pending.extend_from_slice(&buf[..read as usize]);
                responses.extend(drain_lines(&mut pending));
            }
            close_pipe(HANDLE(handle_usize as *mut core::ffi::c_void));
            client_tx.send(responses).expect("ответы в тест");
        });

        // Забираем два запроса (поллинг: клиент мог ещё не подключиться),
        // отвечаем через responder — клиент ждёт ровно два ответа.
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut requests = Vec::new();
        while requests.len() < 2 && Instant::now() < deadline {
            while let Some(request) = server.take_request() {
                requests.push(request);
            }
            if requests.len() < 2 {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert_eq!(requests.len(), 2, "оба запроса получены");
        assert!(
            wake_count.load(Ordering::SeqCst) >= 2,
            "waker звонил не менее двух раз"
        );
        let (line2, respond2) = requests.pop().expect("второй запрос");
        let (line1, respond1) = requests.pop().expect("первый запрос");
        assert!(line1.contains("\"method\":\"canvas_info\""));
        assert!(line2.contains("\"method\":\"nodes_list\""));
        respond1("{\"ok\":1}".to_owned());
        respond2("{\"ok\":2}".to_owned());

        // Клиент дочитывает ответы и завершается
        let responses = client_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("клиент завершился за 10 с");
        client.join().expect("join клиента");
        assert_eq!(responses, vec!["{\"ok\":1}", "{\"ok\":2}"]);
    }
}
