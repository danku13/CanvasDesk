//! Drag-drop из Explorer (T9, SPEC §7.3): события перетаскивания и
//! (на Windows) COM-реализация IDropTarget на окне приложения.
//!
//! Типы `DragEvent`/`DragData` — кроссплатформенные чистые данные:
//! shell снимает с `IDataObject` сырые байты CF_HDROP/CF_UNICODETEXT,
//! а парсинг и раскладку делает `canvas_app::ui` (тестируется на любой ОС,
//! без цикла зависимостей app <-> shell). COM-механика — в дочернем модуле
//! `com` под `cfg(windows)`; весь unsafe проекта — только там (AGENTS.md).

/// Сырые данные перетаскивания, снятые с `IDataObject` (копия, без
/// COM-lifetime).
///
/// Парсинг — на стороне приложения: CF_HDROP — DROPFILES-заголовок
/// (20 байт: pFiles-офсет, fWide-флаг) + список UTF-16 null-terminated
/// строк с DOUBLE null в конце; целиком разбирается в
/// `canvas_app::ui::parse_hdrop_bytes` (тестируется синтетикой на любой ОС).
#[derive(Debug, Clone, PartialEq)]
pub enum DragData {
    /// Содержимое CF_HDROP целиком (заголовок DROPFILES + список файлов),
    /// файлы/папки из Explorer.
    HdropBytes(Vec<u8>),
    /// CF_UNICODETEXT — текст или URL.
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
    /// Курсор вошёл в окно; данные сняты с IDataObject (и закэшированы
    /// в COM-объекте до Drop).
    Enter {
        data: DragData,
        client_pt: (f32, f32),
    },
    /// Курсор двигается над окном (данные те же, позиция обновилась).
    Over { client_pt: (f32, f32) },
    /// Курсор покинул окно или drag отменён (ESC).
    Leave,
    /// Отпускание кнопки: данные перечитаны с IDataObject заново
    /// (кэшу не доверяем, план T9 §5).
    Drop {
        data: DragData,
        client_pt: (f32, f32),
    },
}

#[cfg(windows)]
pub use com::{install, DropWatcher};

/// COM-механика IDropTarget: OleInitialize, RegisterDragDrop, извлечение
/// данных из IDataObject. Реализация — по плану `docs/plans/T9-drag-drop.md`
/// §5, шаг 5; проверяется через win-check (cargo check msvc).
#[cfg(windows)]
mod com;
