//! Платформенно-нейтральные данные drag-drop (T9, M8/W3).
//!
//! Типы переехали из `canvas-shell` (wasm-port §3.1/§6 W3): shell их
//! производит (IDropTarget), web-бинарь `canvas-web` будет производить те
//! же события из DOM-листенеров (W6, план §3.2), приложение (`canvas-app`)
//! — единый потребитель. Здесь только данные; COM-механика остаётся в
//! shell (`dragdrop::com`, cfg(windows)).

/// Данные, снятые с источника перетаскивания (T9).
///
/// `HdropBytes` — сырой payload платформы (на Windows — CF_HDROP целиком,
/// заголовок DROPFILES + список файлов); разбор — на стороне потребителя
/// (`canvas_app::ui::plan_drop`/`dropped_widget_package`), формат чисто
/// байтовый и платформенно-независимо парсится.
#[derive(Debug, Clone, PartialEq)]
pub enum DragData {
    /// Сырые байты списка файлов платформы (CF_HDROP: заголовок DROPFILES
    /// + список файлов), файлы/папки из Explorer.
    HdropBytes(Vec<u8>),
    /// Текст или URL (CF_UNICODETEXT).
    Text(String),
    /// Поддерживаемых форматов нет — эффект DROPEFFECT_NONE.
    None,
}

/// Событие перетаскивания (Enter/Over/Leave/Drop).
///
/// `client_pt` — позиция курсора в ФИЗИЧЕСКИХ пикселях от угла клиентской
/// области окна (ScreenToClient уже применён в shell); приложение делит на
/// `scale_factor` и переводит в world-координаты через камеру.
#[derive(Debug, Clone, PartialEq)]
pub enum DragEvent {
    /// Курсор вошёл в окно; данные сняты с источника (и закэшированы
    /// в COM-объекте до Drop).
    Enter {
        data: DragData,
        client_pt: (f32, f32),
    },
    /// Курсор двигается над окном (данные те же, позиция обновилась).
    Over { client_pt: (f32, f32) },
    /// Курсор покинул окно или drag отменён (ESC).
    Leave,
    /// Отпускание кнопки: данные перечитаны с источника заново
    /// (кэшу не доверяем, план T9 §5).
    Drop {
        data: DragData,
        client_pt: (f32, f32),
    },
}
