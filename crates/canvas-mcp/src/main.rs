//! canvasdesk-mcp — исполняемый MCP-посредник: stdin/stdout ↔ named pipe
//! canvas-app (`\\.\pipe\canvasdesk`).
//!
//! Протокол по stdio — newline-delimited JSON-RPC 2.0 (без Content-Length,
//! как предписывает MCP spec для stdio-транспорта): каждое сообщение — одна
//! строка. На pipe уходит тот же line-framing. Автостарта приложения нет:
//! pipe недоступен → на initialize lib вернёт `Exit { code: 2, .. }` —
//! отвечаем JSON-RPC ошибкой и завершаемся с кодом 2.

use std::io::{Read, Write};

use canvas_mcp::{handle_line, HandleOutcome};
// Трейт нужен только windows-ветке PipeTransport; без gate — unused на Linux
#[cfg(windows)]
use canvas_mcp::AppTransport;

fn main() -> anyhow::Result<()> {
    // Транспорт к приложению: None, если canvas-app не запущен (нет pipe) —
    // lib ответит ошибкой на initialize и попросит выйти с кодом 2.
    let mut transport = connect_app();

    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        // EOF stdin — клиент (MCP-хост) закрыл канал: штатный выход
        let read = stdin.read(&mut buf)?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..read]);
        for line in canvas_mcp::split_frames(&mut pending) {
            match handle_line(&line, &mut transport) {
                HandleOutcome::Reply(reply) => {
                    writeln!(stdout, "{reply}")?;
                    stdout.flush()?;
                }
                HandleOutcome::Silent => {}
                HandleOutcome::Exit { code, reply } => {
                    writeln!(stdout, "{reply}")?;
                    stdout.flush()?;
                    std::process::exit(code);
                }
            }
        }
    }
    Ok(())
}

/// Транспорт к приложению. Вне Windows pipe недоступен — None, lib ответит
/// ошибкой на initialize (код 2).
#[cfg(windows)]
fn connect_app() -> Option<PipeTransport> {
    use canvas_mcp::PIPE_NAME;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Pipes::WaitNamedPipeW;

    let wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let name = windows::core::PCWSTR(wide.as_ptr());
    // WaitNamedPipeW возвращает BOOL (не Result): true — pipe появился.
    // SAFETY: wide NUL-терминирован и живёт до конца вызова.
    let waited = unsafe { WaitNamedPipeW(name, 2000) }.as_bool();
    if !waited {
        return None;
    }
    // SAFETY: name валиден; параметры — константы Win32.
    let handle = unsafe {
        CreateFileW(
            name,
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .ok()?;
    PipeTransport::new(handle)
}

#[cfg(not(windows))]
fn connect_app() -> Option<canvas_mcp::OfflineTransport> {
    None
}

/// Клиент named pipe: дуплексный overlapped-хэндл (копия в reader-потоке),
/// ответы — строки в канале с таймаутом CALL_TIMEOUT (lib кодирует None
/// в isError). Overlapped обязателен: на блокирующем хэндле ядро
/// сериализовало бы операции, и запись из main-потока встала бы за
/// висящим чтением reader-потока (дедлок «запрос ждёт, ответ ждёт»).
#[cfg(windows)]
struct PipeTransport {
    handle: windows::Win32::Foundation::HANDLE,
    write_event: windows::Win32::Foundation::HANDLE,
    inbox: std::sync::mpsc::Receiver<String>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(windows)]
impl PipeTransport {
    fn new(handle: windows::Win32::Foundation::HANDLE) -> Option<Self> {
        // Событие для записи из main-потока (reader живёт со своим).
        // CreateEventW в windows 0.62 возвращает Result.
        let write_event = unsafe { CreateEventW(None, true, false, None) }.ok()?;
        if write_event.is_invalid() {
            return None;
        }
        let (tx, inbox) = std::sync::mpsc::channel::<String>();
        let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let connected_reader = std::sync::Arc::clone(&connected);
        // HANDLE в windows 0.62 не Send — в нить передаём usize и собираем
        // handle обратно (передача значения, не владения).
        let reader_handle = handle.0 as usize;
        let reader = std::thread::Builder::new()
            .name("mcp-pipe-reader".to_owned())
            .spawn(move || {
                let reader_handle =
                    windows::Win32::Foundation::HANDLE(reader_handle as *mut core::ffi::c_void);
                read_loop(reader_handle, tx, connected_reader);
            });
        // Reader не поднялся — транспорт бесполезен (деградация без паники)
        reader.ok()?;
        Some(Self {
            handle,
            write_event,
            inbox,
            connected,
        })
    }
}

/// Overlapped-цикл чтения строк: блокирующий по смыслу (ожидание события),
/// но оставляет хэндл открытым для конкурентной записи из main-потока.
#[cfg(windows)]
fn read_loop(
    handle: windows::Win32::Foundation::HANDLE,
    tx: std::sync::mpsc::Sender<String>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let event = unsafe { CreateEventW(None, true, false, None) }.ok();
    let event = match event {
        Some(event) if !event.is_invalid() => event,
        _ => {
            connected.store(false, std::sync::atomic::Ordering::SeqCst);
            return;
        }
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
                // Приложение закрыло pipe — ReadFile вернёт ошибку
                connected.store(false, std::sync::atomic::Ordering::SeqCst);
                break;
            }
            // SAFETY: event валиден; ждём бесконечно.
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                break;
            }
        }
        let mut read = 0u32;
        // SAFETY: overlapped завершён; read — out-параметр.
        let completed = unsafe { GetOverlappedResult(handle, &overlapped, &mut read, false) };
        if completed.is_err() || read == 0 {
            connected.store(false, std::sync::atomic::Ordering::SeqCst);
            break;
        }
        pending.extend_from_slice(&buf[..read as usize]);
        for line in canvas_mcp::split_frames(&mut pending) {
            if tx.send(line).is_err() {
                // Транспорт уничтожен (main-поток вышел)
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(event) };
}

#[cfg(windows)]
impl AppTransport for PipeTransport {
    fn send_line(&mut self, line: &str) -> Result<(), String> {
        if !self.is_connected() {
            return Err("соединение с CanvasDesk разорвано".to_owned());
        }
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        let mut rest: &[u8] = &bytes;
        while !rest.is_empty() {
            // SAFETY: write_event валиден; буферы живы до GetOverlappedResult.
            let _ = unsafe { ResetEvent(self.write_event) };
            let mut overlapped = OVERLAPPED {
                hEvent: self.write_event,
                ..Default::default()
            };
            let result = unsafe { WriteFile(self.handle, Some(rest), None, Some(&mut overlapped)) };
            if let Err(err) = result {
                if err.code() != ERROR_IO_PENDING.to_hresult() {
                    return Err(format!("WriteFile: {err}"));
                }
                let wait = unsafe { WaitForSingleObject(self.write_event, INFINITE) };
                if wait != WAIT_OBJECT_0 {
                    return Err("ожидание записи прервано".to_owned());
                }
            }
            let mut written = 0u32;
            // SAFETY: overlapped завершён; written — out-параметр.
            let completed =
                unsafe { GetOverlappedResult(self.handle, &overlapped, &mut written, false) };
            if completed.is_err() {
                return Err("WriteFile: соединение разорвано".to_owned());
            }
            if written == 0 {
                return Err("WriteFile: записано 0 байт".to_owned());
            }
            rest = &rest[written as usize..];
        }
        Ok(())
    }

    fn recv_line(&mut self) -> Option<String> {
        self.inbox.recv_timeout(canvas_mcp::CALL_TIMEOUT).ok()
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, WAIT_OBJECT_0};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{ReadFile, WriteFile};
#[cfg(windows)]
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject, INFINITE};
#[cfg(windows)]
use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};
