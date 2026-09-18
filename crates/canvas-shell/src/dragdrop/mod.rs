//! Drag-drop из Explorer (T9, SPEC §7.3): события перетаскивания и
//! (на Windows) COM-реализация IDropTarget на окне приложения.
//!
//! Типы `DragEvent`/`DragData` — кроссплатформенные чистые данные, M8/W3
//! (wasm-port §3.1) переехали в `canvas-core::dragdrop` (shell — один из
//! производителей; web-бинарь будет производить те же события из DOM,
//! W6): здесь только ре-экспорт для обратной совместимости путей
//! (`canvas_shell::dragdrop::DragEvent`). Shell снимает с `IDataObject`
//! сырые байты CF_HDROP/CF_UNICODETEXT, а парсинг и раскладку делает
//! `canvas_app::ui` (тестируется на любой ОС). COM-механика — в дочернем
//! модуле `com` под `cfg(windows)`; весь unsafe проекта — только там
//! (AGENTS.md).

pub use canvas_core::dragdrop::{DragData, DragEvent};

#[cfg(windows)]
pub use com::{install, DropWatcher};

/// COM-механика IDropTarget: OleInitialize, RegisterDragDrop, извлечение
/// данных из IDataObject. Реализация — по плану `docs/plans/T9-drag-drop.md`
/// §5, шаг 5; проверяется через win-check (cargo check msvc).
#[cfg(windows)]
mod com;
