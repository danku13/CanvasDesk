//! M8/W6 (wasm-port §4): состояние web-оболочки — активный канвас (имя +
//! тип хранилища) и хэндл дискового файла (FS Access). Живёт в
//! `thread_local`: весь web-код CanvasDesk исполняется на главном потоке
//! браузера (winit web + spawn_local), гонок нет; `FileSystemFileHandle` —
//! JS-значение (`!Send`), поэтому в трейт-объекты хранилищ (`Send + Sync`)
//! оно не попадает — трейты держат только зеркало/очередь строк.
//!
//! Потребители: `opfs` (инициализация), `fs_access` (открытие с диска),
//! `drop_files` (импорт копии), `export` (экспорт активной версии),
//! `web_locks`/`url_sync` (FR-104: единая точка смены активного канваса
//! — `set_active` захватывает Web Lock и синкает `?canvas=`).

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребители — opfs/fs_access/drop/export (wasm); натив: только тесты

use std::cell::RefCell;
use std::sync::Arc;

use crate::opfs::OpfsStorage;
use crate::opfs_store::OpfsStore;

/// Где живёт активный канвас: OPFS origin'а или настоящий диск (FS Access
/// хэндл). Влияет на экспорт (чтение свежей версии) и reopen из недавних
/// (нужно ли перезапрашивать разрешение).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveKind {
    Opfs,
    Disk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// FR-104 (мультиканвас C1): общее workspace-хранилище (листинг
    /// канвасов OPFS — `OpfsStore`). Сеется в `init_scene` и по запросу
    /// `RequestCanvasList` (свежий листинг — конвейер FR-104 §3.2).
    /// `Arc<OpfsStore>` — `Send + Sync` (внутри только строки), JsValue
    /// в трейт-объекты не попадают.
    static OPFS_WORKSPACE: RefCell<Option<Arc<OpfsStore>>> =
        const { RefCell::new(None) };
    /// FR-104 (мультиканвас C1): прокси событий web-слоя (занятость
    /// Web Locks → `CanvasLockBusy`). Регистрируется spawn_desk_web ПОСЛЕ
    /// построения event loop — до этого init_scene копит занятость в
    /// `web_locks::take_pending_busy` (pending-флаг App, первый кадр).
    #[cfg(target_arch = "wasm32")]
    static EVENT_PROXY: RefCell<Option<winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>>> =
        const { RefCell::new(None) };
}

/// Запомнить активный канвас (вызывается из каждой точки открытия).
/// FR-104 (C1): единая точка смены активного канваса — заодно захватывает
/// Web Lock на его имя (№14b; занятость → модал №35a) и синкает
/// `?canvas=` в URL (№17a). Повторная установка ТОГО ЖЕ канваса — no-op
/// (не дёргает лок/URL: «Всё равно открыть» продолжает без лока).
pub(crate) fn set_active(name: impl Into<String>, kind: ActiveKind) {
    let name = name.into();
    let active = Active {
        name: name.clone(),
        kind,
    };
    let changed = ACTIVE.with(|cell| cell.borrow().as_ref() != Some(&active));
    if changed {
        // FR-104 (C1, №14b): Web Locks — лок имени активного канваса
        // (переключение отпускает предыдущий; занятость — модал №35a).
        #[cfg(target_arch = "wasm32")]
        crate::web_locks::on_active_change(&name, kind);
        // FR-104 (C1, №17a): URL-синк — OPFS ставит ?canvas=<имя>,
        // диск — убирает параметр (дисковый файл по ?canvas= не открыть).
        #[cfg(target_arch = "wasm32")]
        crate::url_sync::sync_active(kind, &name);
    }
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

/// Запомнить общее workspace-хранилище канвасов (FR-104, C1: init_scene /
/// первый `seed_workspace`).
pub(crate) fn set_opfs_workspace(store: Arc<OpfsStore>) {
    OPFS_WORKSPACE.with(|cell| *cell.borrow_mut() = Some(store));
}

/// Общее workspace-хранилище канвасов (клон Arc; листинг/операции
/// менеджера — конвейер FR-104).
pub(crate) fn opfs_workspace() -> Option<Arc<OpfsStore>> {
    OPFS_WORKSPACE.with(|cell| cell.borrow().clone())
}

/// Зарегистрировать прокси событий web-слоя (FR-104, C1: spawn_desk_web
/// сразу после построения event loop — занятость Web Locks со старта
/// копится в `web_locks::take_pending_busy` до этой точки).
#[cfg(target_arch = "wasm32")]
pub(crate) fn set_event_proxy(proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>) {
    EVENT_PROXY.with(|cell| *cell.borrow_mut() = Some(proxy));
}

/// Прокси событий web-слоя (CanvasLockBusy и ответы конвейера FR-104;
/// None — до построения event loop).
#[cfg(target_arch = "wasm32")]
pub(crate) fn event_proxy() -> Option<winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>>
{
    EVENT_PROXY.with(|cell| cell.borrow().clone())
}

/// Заглушка для нативных тестов: thread_local-контракт web-состояния.
/// (JsValue-типы хэндлов на нативе — непрозрачные заглушки, реальный
/// хэндл создаёт только JS-рунтайм на wasm.)
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// FR-104 (C1): общее workspace-хранилище — установка/клон Arc,
    /// отсутствующее — None (контракт thread_local, как OPFS_STORAGE).
    #[test]
    fn opfs_workspace_roundtrip() {
        assert!(OPFS_WORKSPACE.with(|cell| cell.borrow().is_none()));
        let store = Arc::new(OpfsStore::new());
        OPFS_WORKSPACE.with(|cell| *cell.borrow_mut() = Some(Arc::clone(&store)));
        assert!(OPFS_WORKSPACE.with(|cell| cell.borrow().is_some()));
    }

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
        // Повторная установка того же канваса — идемпотентна (значение то же)
        set_active("диск.canvas", ActiveKind::Disk);
        assert_eq!(active_name().as_deref(), Some("диск.canvas"));
    }
}
