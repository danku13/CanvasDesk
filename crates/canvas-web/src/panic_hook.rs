//! Паники web-сборки → консоль браузера (W4-каркас, wasm-port §6).
//!
//! Без хука wasm-паника невидима: браузер показывает только «Uncaught
//! RuntimeError: unreachable» без стека и сообщений — разбора падений
//! не будет. Хук форматирует панику в строку и пишет её в console.error
//! (на нативных целях — stderr, те же строки видны в тестах).

/// Формат строки паники. Чистая функция — нативные тесты каркаса.
pub fn format_panic(location: &str, message: &str) -> String {
    format!("canvas-web: паника в {location}: {message}")
}

/// Установить panic-hook. Вызывается из [`crate::boot`] при старте
/// модуля; типичный вывод: `canvas-web: паника в src/lib.rs:42: …`.
pub fn set_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "неизвестная позиция".to_string());
        let payload = info.payload();
        // payload паник: &str (panic!("литерал")) или String (формат) —
        // прочее кодом не порождается, честная заглушка
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "нетекстовая паника".to_string()
        };
        let line = format_panic(&location, &message);
        #[cfg(target_arch = "wasm32")]
        crate::web_log::console_error(&line);
        #[cfg(not(target_arch = "wasm32"))]
        eprintln!("{line}");
    }));
}

#[cfg(test)]
mod tests {
    use super::{format_panic, set_hook};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// Формат строки паники: маркер крейта, позиция, сообщение.
    #[test]
    fn format_panic_contains_parts() {
        let line = format_panic("src/web_log.rs:7", "стресс-тест");
        assert!(line.starts_with("canvas-web:"), "строка: {line}");
        assert!(line.contains("src/web_log.rs:7"), "строка: {line}");
        assert!(line.contains("стресс-тест"), "строка: {line}");
    }

    /// Хук установлен и не ломает обычную unwind-панику: catch_unwind
    /// возвращает Err (паника прокатилась через наш хук в stderr —
    /// нативная ветка), тестовый прогон не умирает.
    #[test]
    fn hook_routes_panic_without_killing_unwind() {
        set_hook();
        let result = catch_unwind(AssertUnwindSafe(|| panic!("дым-паника каркаса")));
        assert!(result.is_err(), "паника обязана дойти до catch_unwind");
    }
}
