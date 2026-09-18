//! Console-логирование web-слоя: события `tracing` → консоль браузера
//! (W4-каркас, wasm-port §6).
//!
//! Собственный слой вместо готовых крейтов (`tracing-wasm` и т.п.):
//! обвязка тривиальна, новые внешние зависимости не вводятся —
//! `tracing`/`tracing-subscriber` уже в дереве. Консольные extern'ы
//! объявлены точечно, без `js-sys`/`web-sys` — их привнесёт winit на
//! прошивке W4 (§2: winit 0.30 web уже зависит от bindgen-семейства).
//!
//! `cfg(target_arch)` встречается ТОЛЬКО здесь и в `panic_hook` — внутри
//! canvas-web (правило §3.1: web-код не расползается по общим крейтам).
//! На нативных целях (тесты каркаса) консольные вызовы заменяются
//! println/eprintln — extern'ы объявлены безусловно, но вызываются только
//! из wasm-веток (вызов extern на нативе — ошибка линковки).

use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    /// console.log — базовый вывод.
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
    /// console.info — информационные события.
    #[wasm_bindgen(js_namespace = console)]
    fn info(s: &str);
    /// console.warn — предупреждения.
    #[wasm_bindgen(js_namespace = console)]
    fn warn(s: &str);
    /// console.error — ошибки и паники (panic_hook).
    #[wasm_bindgen(js_namespace = console)]
    fn error(s: &str);
}

/// Уровень события → консольная функция (wasm) или поток stdout/stderr
/// (натив). WARN и выше — error/warn, INFO — info, остальное — log.
#[cfg(target_arch = "wasm32")]
fn console_out(level: &tracing::Level, line: &str) {
    match *level {
        tracing::Level::ERROR => error(line),
        tracing::Level::WARN => warn(line),
        tracing::Level::INFO => info(line),
        tracing::Level::DEBUG | tracing::Level::TRACE => log(line),
    }
}

/// Нативный фолбэк для тестов каркаса: те же строки, но в потоки
/// процесса (extern'ы на нативе не вызываются — см. шапку модуля).
#[cfg(not(target_arch = "wasm32"))]
fn console_out(level: &tracing::Level, line: &str) {
    if *level == tracing::Level::ERROR || *level == tracing::Level::WARN {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

/// Прямой вывод ошибки в консоль (используется panic-hook'ом).
#[cfg(target_arch = "wasm32")]
pub fn console_error(line: &str) {
    error(line);
}

/// Нативный фолбэк: та же строка в stderr (тесты каркаса).
#[cfg(not(target_arch = "wasm32"))]
pub fn console_error(line: &str) {
    eprintln!("{line}");
}

/// Формат строки события: `[LEVEL target] message`. Чистая функция —
/// нативные тесты каркаса (W4); формат сознательно компактен: консоль
/// браузера сама рисует уровни цветом по методу log/info/warn/error.
pub fn format_line(level: &tracing::Level, target: &str, message: &str) -> String {
    format!("[{level} {target}] {message}")
}

/// Посетитель полей события: выдёргивает поле `message` (стандартное поле
/// макросов `tracing::*!`; Debug-формат `format_args!` — готовый текст).
#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        }
    }
}

/// Слой `tracing` → консоль: единственная обязанность — отформатировать
/// событие и выбрать консольную функцию по уровню.
struct ConsoleLayer;

impl<S: Subscriber> Layer<S> for ConsoleLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let line = format_line(
            event.metadata().level(),
            event.metadata().target(),
            &visitor.message,
        );
        console_out(event.metadata().level(), &line);
    }
}

/// Установить глобальный подписчика: Registry + консольный слой + фильтр
/// INFO (без фильтра в консоль утекут TRACE/DEBUG крейтов — на web нет
/// RUST_LOG, детальная настройка — прошивка W4). Идемпотентно: повторный
/// вызов молча оставляет уже установленного подписчика (возврат Err от
/// `set_global_default` проглатывается).
pub fn init_tracing() {
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;

    let subscriber = tracing_subscriber::Registry::default()
        .with(ConsoleLayer)
        .with(LevelFilter::INFO);
    let _ = tracing::subscriber::set_global_default(subscriber);
}

#[cfg(test)]
mod tests {
    use super::{format_line, init_tracing};

    /// Формат строки: уровень, цель, сообщение — на месте.
    #[test]
    fn format_line_contains_parts() {
        let line = format_line(&tracing::Level::WARN, "canvas_web", "проверка каркаса");
        assert!(line.contains("WARN"), "строка: {line}");
        assert!(line.contains("canvas_web"), "строка: {line}");
        assert!(line.contains("проверка каркаса"), "строка: {line}");
        assert!(line.starts_with('['), "строка: {line}");
    }

    /// Слой прокладывает реальные события до консольного вывода без
    /// паник (нативный фолбэк — println/eprintln; сам факт прохождения
    /// on_event по всем уровням и есть проверка проводки).
    #[test]
    fn events_flow_through_console_layer() {
        init_tracing();
        tracing::error!(target: "canvas_web_test", "ошибка-дым {value}", value = 1);
        tracing::warn!(target: "canvas_web_test", "предупреждение-дым");
        tracing::info!(target: "canvas_web_test", "инфо-дым");
        // INFO-фильтр: DEBUG под фильтром — тоже не должен ронять слой
        tracing::debug!(target: "canvas_web_test", "отфильтрованный-дым");
    }

    /// Идемпотентность: второй set_global_default — молча, без паники
    /// (двойная инициализация при повторной загрузке модуля).
    #[test]
    fn init_twice_is_silent() {
        init_tracing();
        init_tracing();
    }
}
