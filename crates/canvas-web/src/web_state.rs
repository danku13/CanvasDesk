//! M8/W6 (wasm-port §4): состояние web-оболочки — активный канвас (имя +
//! тип хранилища) и хэндл дискового файла (FS Access). Живёт в
//! `thread_local`: весь web-код CanvasDesk исполняется на главном потоке
//! браузера (winit web + spawn_local), гонок нет; `FileSystemFileHandle` —
//! JS-значение (`!Send`), поэтому в трейт-объекты хранилищ (`Send + Sync`)
//! оно не попадает — трейты держат только зеркало/очередь строк.
//!
//! Потребители: `opfs` (инициализация), `fs_access` (открытие с диска),
//! `drop_files` (импорт копии), `export` (экспорт активной версии).

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребители — opfs/fs_access/drop/export (wasm); натив: только тесты

use std::cell::RefCell;
use std::sync::Arc;

use crate::opfs::OpfsStorage;

/// Где живёт активный канвас: OPFS origin'а или настоящий диск (FS Access
/// хэндл). Влияет на экспорт (чтение свежей версии) и reopen из недавних
/// (нужно ли перезапрашивать разрешение).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveKind {
    Opfs,
    Disk,
}

#[derive(Debug, Clone)]
pub(crate) struct Active {
    pub name: String,
    pub kind: ActiveKind,
}

thread_local! {
    /// Активный канвас (последний открытый/проинициализированный).
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    /// Дисковый хэндл активного канваса (FS Access). Автосейв на диск
    /// пишется именно через него (`createWritable`).
    static DISK_HANDLE: RefCell<Option<web_sys::FileSystemFileHandle>> =
        const { RefCell::new(None) };
    /// Общее OPFS-хранилище сцены (одно на страницу): DOM-drop и reopen
    /// подставляют его в `OpenScene`, чтобы автосейв переключился на OPFS.
    /// Конкретный тип — ради seed_mirror (наполнение зеркала при импорте).
    static OPFS_STORAGE: RefCell<Option<Arc<OpfsStorage>>> =
        const { RefCell::new(None) };
}

/// Запомнить активный канвас (вызывается из каждой точки открытия).
pub(crate) fn set_active(name: impl Into<String>, kind: ActiveKind) {
    let active = Active {
        name: name.into(),
        kind,
    };
    ACTIVE.with(|cell| *cell.borrow_mut() = Some(active));
}

/// Имя активного канваса (None — ещё не открыт; экспорт честно откажется).
pub(crate) fn active_name() -> Option<String> {
    ACTIVE.with(|cell| cell.borrow().as_ref().map(|a| a.name.clone()))
}

/// Тип хранилища активного канваса.
pub(crate) fn active_kind() -> Option<ActiveKind> {
    ACTIVE.with(|cell| cell.borrow().as_ref().map(|a| a.kind))
}

/// Запомнить дисковый хэндл (после пикера/reopen с granted-разрешением).
pub(crate) fn set_disk_handle(handle: web_sys::FileSystemFileHandle) {
    DISK_HANDLE.with(|cell| *cell.borrow_mut() = Some(handle));
}

/// Хэндл дискового файла, если он есть (клон JsValue-ссылки — дёшево).
pub(crate) fn disk_handle() -> Option<web_sys::FileSystemFileHandle> {
    DISK_HANDLE.with(|cell| cell.borrow().clone())
}

/// Запомнить общее OPFS-хранилище (один раз при инициализации сцены).
#[cfg(target_arch = "wasm32")]
pub(crate) fn set_opfs_storage(storage: Arc<OpfsStorage>) {
    OPFS_STORAGE.with(|cell| *cell.borrow_mut() = Some(storage));
}

/// Общее OPFS-хранилище (клон Arc; подстановка в `OpenScene` — унсайз-
/// коэрция в `Arc<dyn CanvasStorage>` на месте вызова).
#[cfg(target_arch = "wasm32")]
pub(crate) fn opfs_storage() -> Option<Arc<OpfsStorage>> {
    OPFS_STORAGE.with(|cell| cell.borrow().clone())
}

/// Заглушка для нативных тестов: thread_local-контракт web-состояния.
/// (JsValue-типы хэндлов на нативе — непрозрачные заглушки, реальный
/// хэндл создаёт только JS-рунтайм на wasm.)
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// Активный канвас: set/активное имя/тип — до установки None.
    #[test]
    fn active_roundtrip() {
        assert_eq!(active_name(), None);
        assert_eq!(active_kind(), None);
        set_active("проект.canvas", ActiveKind::Opfs);
        assert_eq!(active_name().as_deref(), Some("проект.canvas"));
        assert_eq!(active_kind(), Some(ActiveKind::Opfs));
        // Повторная установка перезаписывает (последнее открытие выигрывает)
        set_active("диск.canvas", ActiveKind::Disk);
        assert_eq!(active_name().as_deref(), Some("диск.canvas"));
        assert_eq!(active_kind(), Some(ActiveKind::Disk));
    }
}
