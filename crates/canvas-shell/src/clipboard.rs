//! Буфер обмена ОС (T7, M8/W3 — wasm-port §3.4 «Clipboard → за трейт»).
//!
//! Код переехал из `canvas-app` (was `struct Clipboard(Option<arboard>)`)
//! в shell — платформенный крейт: app теперь держит нейтральный
//! `Box<dyn ClipboardBackend>` (инъекция в `App::new`), web-бинарь
//! подключит `navigator.clipboard` своей реализацией (план §3.2,
//! деградация warn — семантика сохранена).

use canvas_core::ClipboardBackend;

/// Буфер обмена ОС (arboard): ошибки — warn, редактирование не ломается;
/// недоступный буфер (headless/wayland) — no-op через `None`.
pub struct ArboardClipboard(Option<arboard::Clipboard>);

impl ArboardClipboard {
    pub fn new() -> Self {
        match arboard::Clipboard::new() {
            Ok(clipboard) => Self(Some(clipboard)),
            Err(err) => {
                tracing::warn!(%err, "буфер обмена недоступен");
                Self(None)
            }
        }
    }
}

impl Default for ArboardClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardBackend for ArboardClipboard {
    fn set_text(&mut self, text: String) {
        if let Some(clipboard) = &mut self.0 {
            if let Err(err) = clipboard.set_text(text) {
                tracing::warn!(%err, "не удалось записать в буфер обмена");
            }
        }
    }

    fn get_text(&mut self) -> Option<String> {
        self.0
            .as_mut()
            .and_then(|clipboard| match clipboard.get_text() {
                Ok(text) => Some(text),
                Err(err) => {
                    tracing::warn!(%err, "не удалось прочитать буфер обмена");
                    None
                }
            })
    }
}
