//! M8/W5 (wasm-port §3.2, «Буфер обмена»): `navigator.clipboard` за
//! нейтральным трейтом [`canvas_core::ClipboardBackend`] (паттерн W3-сервисов;
//! натив — `canvas_shell::clipboard::ArboardClipboard`).
//!
//! Браузерный Clipboard API — асинхронный (Promise), а контракт трейта —
//! синхронный (`set_text`/`get_text`, вызовы из обработчика клавиатуры в
//! кадре события). Мост через **локальный кэш**:
//!
//! - `set_text` — обновляет кэш немедленно и (wasm) отправляет текст в
//!   системный буфер через `writeText`; Promise — fire-and-forget, ошибки
//!   (нет user activation / не secure context) — warn, редактирование не
//!   ломается (та философия деградации, что у `Clipboard(Option)` в app);
//! - `get_text` — возвращает кэш немедленно и (wasm) запускает фоновый
//!   `readText` для обновления кэша к следующему Ctrl+V: Ctrl+C/X/V внутри
//!   страницы работают всегда; вставка скопированного *снаружи*
//!   подтягивается следующим жестом (первый Ctrl+V после внешнего
//!   копирования может вставить прежнее — осознанное ограничение волны 1;
//!   DOM-событие `paste` с синхронным `clipboardData` не используется:
//!   winit web с `prevent_default` (дефолт) гасит его `preventDefault()`
//!   на keydown — по исходникам winit 0.30.13, риск-таблица §7 плана).
//!
//! Транзиентная user activation от Ctrl+C/V в Chromium покрывает
//! `writeText`/`readText`, инициированные из того же жеста (Promise
//! разрешается позже жеста, но само разрешение — уже разрешено).
//!
//! Нативная компиляция (rlib-тесты): JS-мост не запускается — работает
//! только кэш (`Rc<RefCell>`), контракт трейта соблюдён.

use std::cell::RefCell;
use std::rc::Rc;

use canvas_core::ClipboardBackend;

/// Общее состояние: кэш последнего текста (единственный синхронный источник
/// `get_text`) + ручка системного буфера (wasm).
struct Inner {
    cache: RefCell<Option<String>>,
    /// wasm: `navigator.clipboard`; None — API недоступен (не secure
    /// context, старый браузер) — деградация до кэша с warn при первом
    /// жесте. Натив (rlib-тесты): поля нет — JS-рунтайма нет.
    #[cfg(target_arch = "wasm32")]
    system: Option<web_sys::Clipboard>,
}

/// Web-буфер обмена для инъекции в `App::new` (параметр `clipboard`,
/// паттерн сервисов W3). `!Send` — App на web однопоточен (главный поток).
pub struct WebClipboard {
    inner: Rc<Inner>,
}

impl WebClipboard {
    /// wasm: доступ к `navigator.clipboard` (None — нет API). Натив
    /// (rlib-тесты): системного буфера нет — кэш-only.
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        let system = web_sys::window().map(|window| window.navigator().clipboard());
        Self {
            inner: Rc::new(Inner {
                cache: RefCell::new(None),
                system,
            }),
        }
    }

    /// Нативная заглушка для тестов каркаса: только кэш.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(Inner {
                cache: RefCell::new(None),
            }),
        }
    }
}

impl Default for WebClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardBackend for WebClipboard {
    /// Записать текст: кэш — немедленно, системный буфер — через
    /// `writeText` (Promise, fire-and-forget). Вызывается из Ctrl+C/X
    /// (текст выделения) и Ctrl+X нод (текст заголовков) — user
    /// activation есть.
    fn set_text(&mut self, text: String) {
        *self.inner.cache.borrow_mut() = Some(text.clone());
        #[cfg(target_arch = "wasm32")]
        if let Some(clipboard) = &self.inner.system {
            let future = wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&text));
            // fire-and-forget: Promise разрешается после возврата из
            // обработчика; исход важен только для диагностики — успех
            // тихий, отказ (нет user activation / не secure context) —
            // warn. Кэш уже обновлён: внутристраничные операции не
            // зависят от исхода.
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = future.await {
                    tracing::warn!(target: "canvas_web", ?err, "writeText отклонён — буфер только внутренний");
                }
            });
        }
    }

    /// Прочитать текст: кэш — немедленно (синхронный контракт трейта);
    /// параллельно (wasm) фоновый `readText` обновит кэш к следующему
    /// Ctrl+V (внешнее копирование подтягивается следующим жестом).
    fn get_text(&mut self) -> Option<String> {
        let cached = self.inner.cache.borrow().clone();
        #[cfg(target_arch = "wasm32")]
        if let Some(clipboard) = &self.inner.system {
            let future = wasm_bindgen_futures::JsFuture::from(clipboard.read_text());
            let inner = Rc::clone(&self.inner);
            wasm_bindgen_futures::spawn_local(async move {
                match future.await {
                    Ok(value) => {
                        if let Some(text) = value.as_string() {
                            *inner.cache.borrow_mut() = Some(text);
                        }
                    }
                    // NotAllowedError без активации — штатная деградация;
                    // warn не шумим: кэш продолжает обслуживать жесты
                    Err(err) => {
                        tracing::debug!(target: "canvas_web", ?err, "readText отклонён — кэш не обновлён");
                    }
                }
            });
        }
        cached
    }
}

#[cfg(test)]
mod tests {
    use super::WebClipboard;
    use canvas_core::ClipboardBackend;

    /// Контракт трейта (натив, rlib-тесты каркаса): set→get roundtrip.
    /// На wasm проверяется та же логика кэша — системный Promise-мост
    /// асинхронен и в тестах не участвует.
    #[test]
    fn set_then_get_roundtrips() {
        let mut clipboard = WebClipboard::new();
        clipboard.set_text("привет мир".to_string());
        assert_eq!(clipboard.get_text().as_deref(), Some("привет мир"));
    }

    /// Пустой буфер: get до первого set — None (паттерн NoopClipboard —
    /// деградация не ломает Ctrl+V: App просто ничего не вставляет).
    #[test]
    fn get_before_set_is_none() {
        let mut clipboard = WebClipboard::new();
        assert_eq!(clipboard.get_text(), None);
    }

    /// Повторный set затирает предыдущий текст (последняя запись
    /// побеждает — семантика системного буфера).
    #[test]
    fn second_set_replaces_first() {
        let mut clipboard = WebClipboard::new();
        clipboard.set_text("первый".to_string());
        clipboard.set_text("второй".to_string());
        assert_eq!(clipboard.get_text().as_deref(), Some("второй"));
    }

    /// Кэш переживает апкаст до `Box<dyn ClipboardBackend>` — контракт
    /// инъекции в `App::new` (объект-безопасность трейта).
    #[test]
    fn trait_object_roundtrip() {
        let mut clipboard: Box<dyn ClipboardBackend> = Box::new(WebClipboard::new());
        clipboard.set_text("кириллица".to_string());
        assert_eq!(clipboard.get_text().as_deref(), Some("кириллица"));
    }
}
