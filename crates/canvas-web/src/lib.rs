//! canvas-web — web-платформенный слой CanvasDesk (M8, wasm-port §3.1):
//! зеркало `canvas-shell` по роли в архитектуре — bindgen-обвязка,
//! web-сервисы и запуск браузерной сборки. Крейт-лист в DAG (§3.5): ни
//! `canvas-app`, ни нативные бинари на него не ссылаются — граф сборки
//! `canvasdesk.exe` не меняется.
//!
//! Стадия **W6** (§6.1): каркас, прошивка (W4), ввод (W5), поиск (W7) и
//! хранение (план §4). OPFS-хранилище канвасов — зеркало с фоновой
//! записью и автосейвом с `.bak` (§4.1); FS Access — пикер и автосейв на
//! настоящий диск; IndexedDB — недавние канвасы; DOM-drop — приём файлов;
//! `?canvas=` — именованный старт; экспорт — download-blob; конфиг — из
//! localStorage.
//!
//! Нативная компиляция: макросы `#[wasm_bindgen]` на не-wasm целях
//! раскрываются в заглушки (контрольная сборка 2026-09-16, §2) — крейт
//! собирается в составе workspace, web-код при этом не вызывается.

pub mod app_spawn;
pub mod fs_access;
pub mod opfs;
pub mod panic_hook;
pub mod recent;
pub mod renderer_launch;
pub mod url_params;
pub mod web_clipboard;
pub mod web_log;
pub mod web_state;
pub mod web_thumbs;
pub mod widgets_web;

// Чисто web-модули: JS-рунтайм обязателен (spawn_local/web-sys-вызовы),
// нативная компиляция rlib их не включает.
#[cfg(target_arch = "wasm32")]
pub mod drop_files;
#[cfg(target_arch = "wasm32")]
pub mod export;
#[cfg(target_arch = "wasm32")]
pub mod js_glue;
#[cfg(target_arch = "wasm32")]
pub mod toolbar;

use wasm_bindgen::prelude::*;

/// Точка входа wasm-модуля: `#[wasm_bindgen(start)]` исполняется при
/// инстанцировании (trunk подключает модуль в `index.html`). Каркас
/// (panic-hook + tracing-консоль) — сразу, затем прошивка: App,
/// web-сервисы и event loop (`spawn_app`). W7: уровень консольного лога —
/// из URL (`?log=debug|trace|…`, дефолт INFO) — читать параметры надо ДО
/// инициализации трейсинга, поэтому `read_params` зовётся здесь.
#[wasm_bindgen(start)]
pub fn start() {
    let _banner = boot();
    if let Err(err) = app_spawn::spawn_desk() {
        tracing::error!(target: "canvas_web", %err, "не удалось запустить CanvasDesk (web)");
    }
}

/// Инициализация каркаса: panic-hook + tracing-консоль + строка версии
/// (в лог). `pub` — вызывается из `start` (wasm) и из тестов каркаса
/// (нативно); возвращает строку баннера для проверяемости.
pub fn boot() -> String {
    boot_with_level(app_spawn::read_params().log_level)
}

/// Вариант с явным уровнем лога (wasm: `?log=`; тесты: None → INFO).
fn boot_with_level(level: Option<url_params::LogLevel>) -> String {
    panic_hook::set_hook();
    use tracing_subscriber::filter::LevelFilter;
    match level {
        Some(url_params::LogLevel::Trace) => {
            web_log::init_tracing_with(LevelFilter::TRACE);
        }
        Some(url_params::LogLevel::Debug) => {
            web_log::init_tracing_with(LevelFilter::DEBUG);
        }
        Some(url_params::LogLevel::Warn) => {
            web_log::init_tracing_with(LevelFilter::WARN);
        }
        Some(url_params::LogLevel::Error) => {
            web_log::init_tracing_with(LevelFilter::ERROR);
        }
        // Info и «не уровень» (url_params уже смягчил) — дефолт INFO
        _ => web_log::init_tracing_with(LevelFilter::INFO),
    }
    let banner = format!(
        "canvas-web каркас загружен (W6): версия {}",
        env!("CARGO_PKG_VERSION")
    );
    tracing::info!(target: "canvas_web", "{banner}");
    banner
}

#[cfg(test)]
mod tests {
    use super::boot;

    /// Каркас стартует без паник и рапортует версию крейта (баннер —
    /// единственный «интерфейс» каркаса до прошивки).
    #[test]
    fn boot_reports_version() {
        let banner = boot();
        assert!(
            banner.contains(env!("CARGO_PKG_VERSION")),
            "баннер: {banner}"
        );
        assert!(banner.contains("каркас"), "баннер: {banner}");
    }

    /// Повторный `boot` не валится: `set_global_default` на втором вызове
    /// возвращает Err — каркас обязан это молча проглотить (идемпотентность
    /// инициализации, ср. двойную загрузку модуля).
    #[test]
    fn boot_is_idempotent() {
        let _ = boot();
        let _ = boot();
    }
}
