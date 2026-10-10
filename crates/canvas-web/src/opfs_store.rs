//! FR-104 (мультиканвас C1): `OpfsStore` — реализация [`WorkspaceStore`]
//! над OPFS (план v2.1 §3.1). Паттерн — «зеркало + конвейер событий»
//! (контракт C0, `workspace.rs`): сигнатуры трейта синхронные, браузерные
//! async-оперы живут ВНУТРИ реализации.
//!
//! ## Устройство
//!
//! - **Зеркало** «имя файла → ts» наполняется при инициализации
//!   ([`seed_workspace`], вызывается из `init_scene`) и обновляется по
//!   запросу (`RequestCanvasList` → web-запрос `CanvasList`). Текст
//!   канвасов в зеркало не входит: активный канвас обслуживается
//!   `OpfsStorage` (автосейв), а листингу нужен только ts; полное чтение
//!   всех канвасов на старте — лишний I/O (отклонение от формулировки
//!   «имя → (текст, ts)» задачи, зафиксировано в FR-104 §Отклонения).
//! - **Мутации** (create/rename/delete) валидируются по зеркалу
//!   синхронно (семантика = `MemWorkspaceStore`, эталон C0), зеркало
//!   обновляется оптимистично, сама OPFS-операция уходит фоновым
//!   `spawn_local`-таском (fire-and-forget, ошибки — tracing, цель
//!   `canvas_web`, затем refresh зеркала — реальность побеждает).
//!   Результат для события `CanvasOpDone` несёт СИНХРОННУЮ часть
//!   (валидацию); async-отказ I/O — только лог + самолечение листингом.
//! - **JS-глю** (`opfsList/opfsRename/opfsDelete` в `index.html`,
//!   объект `window.__canvasdesk`): async-итератор `entries()` и
//!   нестандартный `fileHandle.move()` компактнее в JS; Rust владеет
//!   семантикой (белый список `.canvas`, скрытие `.bak` — R-T7).
//!
//! `JsValue`-хэндлы внутрь не попадают (только строки) — `Send + Sync`
//! трейт-объекта не нарушены; общий экземпляр живёт в
//! `web_state::opfs_workspace` (thread_local `Arc<OpfsStore>`, паттерн
//! `OPFS_STORAGE`).

use std::collections::BTreeMap;
#[cfg(target_arch = "wasm32")] // seed_workspace: общий экземпляр в web_state
use std::sync::Arc;
use std::sync::{Mutex, MutexGuard};

use canvas_core::workspace::{CanvasEntry, EntryKind, MAX_CANVASES};

use crate::workspace::{WorkspaceError, WorkspaceStore};

// ============================================================================
// Чистые функции выбора имени (№31c — битая ссылка / фолбэк Web Locks)
// ============================================================================

/// FR-104 (C1, №31c): каноническое имя для `?canvas=` — точное совпадение
/// с файлом; иначе регистронезависимое (санитайзер не меняет регистр, а
/// «Notes.CANVAS» в URL — законное имя, если файл «Notes.canvas»).
/// Битая ссылка → `None` (вызывающий идёт в [`broken_link_fallback`]).
pub fn resolve_url_canvas<'a>(url_name: &'a str, existing: &'a [String]) -> Option<&'a str> {
    if existing.iter().any(|name| name == url_name) {
        return Some(url_name);
    }
    let lower = url_name.to_lowercase();
    existing
        .iter()
        .find(|name| name.to_lowercase() == lower)
        .map(String::as_str)
}

/// FR-104 (C1, №31c): фолбэк битой ссылки `?canvas=` — верхний из недавних
/// (максимум ts; недавние без ts-порядка пропускаются), который
/// **существует** (точное имя — следующим шагом файл читается под ним) и
/// ≠ битому имени (регистронезависимо). `None` — вызывающий берёт
/// default.canvas (сеять допустимо только его) либо автоимя (Web Locks
/// «Выбрать другой», где default может быть сам занят — иначе модал
/// зациклится, см. FR-104 §Фолбэк занятости).
pub fn broken_link_fallback(
    broken: &str,
    recent: &[(String, f64)],
    existing: &[String],
) -> Option<String> {
    let broken_lower = broken.to_lowercase();
    let mut sorted = recent.to_vec();
    sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
    sorted
        .iter()
        .filter(|(name, _)| name.to_lowercase() != broken_lower)
        .find(|(name, _)| existing.iter().any(|file| file == name))
        .map(|(name, _)| name.clone())
}

// ============================================================================
// OpfsStore: зеркало + мутации (синхронная часть трейта)
// ============================================================================

/// Запись зеркала: ts последнего изменения (unix ms из `file.lastModified`;
/// `.bak`-близнецы и посторонние файлы хранятся в зеркале, но скрыты из
/// `list` белым списком `.canvas` — R-T7).
#[derive(Debug, Clone, Copy, Default)]
struct MirrorEntry {
    ts: u64,
}

/// Внутреннее состояние (Mutex: трейт берёт `&self`; отравленный лок не
/// роняет хранилище — паттерн ядра). `clock` — монотонный источник ts для
/// нативных тестов (на wasm ts = `Date.now()`).
#[derive(Debug, Default)]
struct OpfsInner {
    files: BTreeMap<String, MirrorEntry>,
    /// Монотонный источник ts (нативные тесты — детерминизм; на wasm
    /// ts = `Date.now()`).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    clock: u64,
}

/// OPFS-хранилище рабочего стола канвасов (FR-104, волна C1): синхронное
/// зеркало по трейту + фоновые мутации в OPFS (wasm). Нативный rlib-путь —
/// только зеркало (I/O web-only, паттерн `OpfsStorage`).
#[derive(Debug, Default)]
pub struct OpfsStore {
    inner: Mutex<OpfsInner>,
}

impl OpfsStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Отравленный Mutex не роняет хранилище (паттерн MemWorkspaceStore).
    fn lock(&self) -> MutexGuard<'_, OpfsInner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Свежий ts записи (wasm — `Date.now()`; натив — монотонный счётчик,
    /// детерминизм тестов).
    fn next_ts(inner: &mut OpfsInner) -> u64 {
        #[cfg(target_arch = "wasm32")]
        {
            // Счётчик — нативная деталь (детерминизм тестов); wasm живёт
            // на стенном времени из JS.
            let _ = inner;
            js_sys::Date::now() as u64
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            inner.clock += 1;
            inner.clock
        }
    }

    /// Наполнить зеркало листингом (инициализация/refresh): ВСЕ файлы корня
    /// OPFS (включая `.bak` и посторонние — переезд близнеца при rename
    /// и мягкое удаление их видят), `list` фильтрует белым списком.
    pub fn seed_listing(&self, files: Vec<(String, u64)>) {
        let mut inner = self.lock();
        inner.files = files
            .into_iter()
            .map(|(name, ts)| (name, MirrorEntry { ts }))
            .collect();
    }

    /// Добавить одну запись в зеркало (сеяние нового файла вне конвейера
    /// мутаций — стартовое `default.canvas`; существующая не трогается:
    /// полный листинг знает о ней больше).
    pub fn seed_file(&self, name: &str) {
        let mut inner = self.lock();
        if !inner.files.contains_key(name) {
            let ts = Self::next_ts(&mut inner);
            inner.files.insert(name.to_owned(), MirrorEntry { ts });
        }
    }

    /// Все имена файлов зеркала (без фильтра — выбор фолбэка №31c и
    /// автоимён работает по полному листингу).
    pub fn names(&self) -> Vec<String> {
        self.lock().files.keys().cloned().collect()
    }

    /// Текст файла-близнеца `.bak` есть? (нативные тесты контракта rename).
    #[cfg(test)]
    fn has_raw(&self, name: &str) -> bool {
        self.lock().files.contains_key(name)
    }
}

impl WorkspaceStore for OpfsStore {
    fn list(&self) -> Vec<CanvasEntry> {
        self.lock()
            .files
            .iter()
            // Белый список .canvas (R-T7): .bak-близнецы и посторонние
            // файлы (картинки превью и пр.) скрыты из листинга менеджера.
            .filter(|(name, _)| name.ends_with(".canvas"))
            .map(|(name, entry)| CanvasEntry {
                name: name.clone(),
                ts: entry.ts,
                kind: EntryKind::Opfs,
                repo: None,
            })
            .collect()
    }

    fn create(&self, name: &str) -> Result<(), WorkspaceError> {
        let mut inner = self.lock();
        if name.is_empty() || !name.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(name.to_owned()));
        }
        let lower = name.to_lowercase();
        if inner
            .files
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(name.to_owned()));
        }
        if inner.files.len() >= MAX_CANVASES {
            return Err(WorkspaceError::LimitReached);
        }
        let ts = Self::next_ts(&mut inner);
        inner.files.insert(name.to_owned(), MirrorEntry { ts });
        drop(inner);
        // Фоновая запись пустого канваса (fire-and-forget; ошибки — лог +
        // refresh зеркала). create допустим только под финальным именем —
        // двойных расширений семантика имени не допускает.
        #[cfg(target_arch = "wasm32")]
        spawn_create(name.to_owned());
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            file = name,
            "native: OPFS-мутация пропущена (web-only путь)"
        );
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<(), WorkspaceError> {
        if new.is_empty() || !new.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(new.to_owned()));
        }
        let mut inner = self.lock();
        if old == new && inner.files.contains_key(old) {
            return Ok(()); // идемпотентный no-op
        }
        let lower = new.to_lowercase();
        if inner
            .files
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(new.to_owned()));
        }
        let _old = inner
            .files
            .remove(old)
            .ok_or_else(|| WorkspaceError::NotFound(old.to_owned()))?;
        let ts = Self::next_ts(&mut inner);
        inner.files.insert(new.to_owned(), MirrorEntry { ts });
        // .bak-близнец переезжает вместе с файлом (контракт rename).
        if let Some(bak) = inner.files.remove(&format!("{old}.bak")) {
            inner.files.insert(format!("{new}.bak"), bak);
        }
        drop(inner);
        #[cfg(target_arch = "wasm32")]
        spawn_rename(old.to_owned(), new.to_owned());
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            from = old,
            to = new,
            "native: OPFS-мутация пропущена (web-only путь)"
        );
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), WorkspaceError> {
        let mut inner = self.lock();
        let entry = inner
            .files
            .remove(name)
            .ok_or_else(|| WorkspaceError::NotFound(name.to_owned()))?;
        // Мягкое удаление (№15a): содержимое — в .bak (замещает прежний).
        inner
            .files
            .insert(format!("{name}.bak"), MirrorEntry { ts: entry.ts });
        drop(inner);
        #[cfg(target_arch = "wasm32")]
        spawn_delete(name.to_owned());
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            file = name,
            "native: OPFS-мутация пропущена (web-only путь)"
        );
        Ok(())
    }

    fn exists(&self, name: &str) -> bool {
        self.lock().files.contains_key(name)
    }
}

// ============================================================================
// Лифт зеркала в web_state (wasm): init + refresh
// ============================================================================

/// Наполнить/обновить зеркало общего `OpfsStore` (web_state) листингом
/// OPFS: создание при первом вызове, `RequestCanvasList` зовёт повторно —
/// «свежее зеркало» конвейера FR-104 §3.2. `false` — глю/листинг недоступны
/// (деградация: вызывающие держатся прежнего зеркала или пустого списка).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn seed_workspace() -> bool {
    let store = match crate::web_state::opfs_workspace() {
        Some(store) => store,
        None => {
            let store = Arc::new(OpfsStore::new());
            crate::web_state::set_opfs_workspace(Arc::clone(&store));
            store
        }
    };
    match opfs_list().await {
        Ok(files) => {
            store.seed_listing(files);
            true
        }
        Err(err) => {
            tracing::warn!(
                target: "canvas_web",
                %err,
                "листинг OPFS не получен — зеркало не обновлено"
            );
            false
        }
    }
}

/// Свежий листинг канвасов (зеркало + refresh) для ответа `CanvasList`.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn workspace_entries() -> Vec<CanvasEntry> {
    seed_workspace().await;
    crate::web_state::opfs_workspace()
        .map(|store| store.list())
        .unwrap_or_default()
}

// ============================================================================
// navigator.storage.persist() (R-T3 — защита OPFS от eviction)
// ============================================================================

/// FR-104 (C1, R-T3): запросить persistent-статус хранилища
/// (`navigator.storage.persist()`) — защита OPFS от eviction браузера
/// до переезда на диск (№51a). Fire-and-forget: результат (grant/refuse)
/// — в лог; повторный вызов дешёв и идемпотентен со стороны браузера.
/// TODO(FR-104): C3 перенесёт вызов в точку первого открытия менеджера
/// канвасов — сейчас менеджера нет, вызов живёт на старте (init_scene).
#[cfg(target_arch = "wasm32")]
pub(crate) fn request_storage_persist() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let storage = window.navigator().storage();
    let Ok(promise) = storage.persist() else {
        tracing::debug!(target: "canvas_web", "storage.persist() недоступен");
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(granted) => tracing::info!(
                target: "canvas_web",
                granted = granted.as_bool(),
                "navigator.storage.persist(): запрос выполнен"
            ),
            Err(err) => tracing::warn!(
                target: "canvas_web",
                error = ?err,
                "navigator.storage.persist(): отказ"
            ),
        }
    });
}

// ============================================================================
// JS-глю: opfsList / opfsRename / opfsDelete (index.html)
// ============================================================================

/// Разбор ответа `opfsList`: `Array<{name: string, ts: number}>` →
/// `Vec<(имя, ts)>`. Битые элементы молча пропускаются (данные сторонних
/// версий не должны ронять листинг — паттерн `parse_recent`).
#[cfg(target_arch = "wasm32")]
fn parse_files(value: &wasm_bindgen::JsValue) -> Vec<(String, u64)> {
    use wasm_bindgen::JsCast;
    let Ok(array) = value.clone().dyn_into::<js_sys::Array>() else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|entry| {
            let object = entry.dyn_into::<js_sys::Object>().ok()?;
            let name = js_sys::Reflect::get(&object, &"name".into())
                .ok()?
                .as_string()?;
            let ts = js_sys::Reflect::get(&object, &"ts".into()).ok()?.as_f64()? as u64;
            Some((name, ts))
        })
        .collect()
}

/// Листинг всех файлов корня OPFS через JS-глю (`for await … entries()`,
/// ts = `file.lastModified`; фильтрация — Rust, белый список `.canvas`).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn opfs_list() -> Result<Vec<(String, u64)>, String> {
    let Some(result) = crate::js_glue::call("opfsList", &[]).await else {
        return Err("JS-глю opfsList недоступен".to_owned());
    };
    let value = result.map_err(|err| format!("opfsList: {err:?}"))?;
    Ok(parse_files(&value))
}

/// Ответ глю-мутации: строка `"ok" | "notfound" | "error:<текст>"`.
#[cfg(target_arch = "wasm32")]
async fn glue_mutation(
    function: &str,
    args: &[wasm_bindgen::JsValue],
    what: &str,
) -> Result<(), WorkspaceError> {
    let Some(result) = crate::js_glue::call(function, args).await else {
        return Err(WorkspaceError::Io(format!("{what}: JS-глю недоступен")));
    };
    let answer = result
        .map_err(|err| WorkspaceError::Io(format!("{what}: {err:?}")))?
        .as_string()
        .ok_or_else(|| WorkspaceError::Io(format!("{what}: нестандартный ответ глю")))?;
    match answer.as_str() {
        "ok" => Ok(()),
        "notfound" => Err(WorkspaceError::NotFound(what.to_owned())),
        other => Err(WorkspaceError::Io(format!("{what}: {other}"))),
    }
}

/// Переименование в OPFS через глю (`fileHandle.move` с фолбэком
/// read→write→removeEntry + перенос `.bak`-близнеца — реализовано в JS).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn opfs_rename(old: &str, new: &str) -> Result<(), WorkspaceError> {
    glue_mutation("opfsRename", &[old.into(), new.into()], old).await
}

/// Мягкое удаление через глю (текст → `<name>.bak`, затем removeEntry).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn opfs_delete(name: &str) -> Result<(), WorkspaceError> {
    glue_mutation("opfsDelete", &[name.into()], name).await
}

/// Фоновые мутации (fire-and-forget): ошибка — tracing + refresh зеркала
/// (реальность побеждает оптимизм). Синхронная часть уже вернула Ok в
/// трейт — `CanvasOpDone` несёт валидацию, async-отказ лечится листингом.
#[cfg(target_arch = "wasm32")]
fn spawn_create(name: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let text = crate::workspace::empty_canvas_text();
        let result = async {
            let root = crate::opfs::opfs_root()
                .await
                .map_err(|err| format!("OPFS root: {err:?}"))?;
            crate::opfs::write_opfs_text(&root, &name, &text)
                .await
                .map_err(|err| format!("запись: {err:?}"))
        }
        .await;
        report_mutation(&name, "create", result);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_rename(old: String, new: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = opfs_rename(&old, &new).await.map_err(|err| err.to_string());
        report_mutation(&old, "rename", result);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_delete(name: String) {
    wasm_bindgen_futures::spawn_local(async move {
        let result = opfs_delete(&name).await.map_err(|err| err.to_string());
        report_mutation(&name, "delete", result);
    });
}

/// Лог результата фоновой мутации + самолечение зеркала при отказе
/// (refresh листингом — оптимистичная запись могла не дойти до OPFS).
#[cfg(target_arch = "wasm32")]
fn report_mutation(name: &str, op: &str, result: Result<(), String>) {
    match result {
        Ok(()) => tracing::debug!(
            target: "canvas_web",
            op,
            file = name,
            "OPFS: мутация применена"
        ),
        Err(err) => {
            tracing::error!(target: "canvas_web", op, file = name, %err, "OPFS: мутация не удалась — зеркало будет обновлено листингом");
            wasm_bindgen_futures::spawn_local(async move {
                let _ = seed_workspace().await;
            });
        }
    }
}

// ============================================================================
// Нативные тесты: семантика зеркала = контракт MemWorkspaceStore (эталон C0)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    fn store_with(files: &[(&str, u64)]) -> OpfsStore {
        let store = OpfsStore::new();
        store.seed_listing(
            files
                .iter()
                .map(|(name, ts)| ((*name).to_owned(), *ts))
                .collect(),
        );
        store
    }

    fn names(entries: &[CanvasEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    // --- белый список листинга (R-T7) ---------------------------------------

    #[test]
    fn list_whitelists_canvas_and_hides_bak() {
        let store = store_with(&[
            ("a.canvas", 1),
            ("a.canvas.bak", 2),
            ("b.canvas.bak", 3),
            ("картинка.png", 4),
            ("без-расширения", 5),
            ("X.CANVAS", 6), // верхний регистр расширения — НЕ белый список
        ]);
        assert_eq!(
            names(&store.list()),
            ["a.canvas"],
            "только точный суффикс .canvas"
        );
        let entry = &store.list()[0];
        assert_eq!(entry.ts, 1, "ts из листинга OPFS проходит насквозь");
        assert_eq!(entry.kind, EntryKind::Opfs);
        assert!(entry.repo.is_none());
    }

    // --- create: семантика эталона -------------------------------------------

    #[test]
    fn create_rejects_collisions_invalid_names_and_limit() {
        let store = store_with(&[("x.canvas", 1)]);
        assert_eq!(
            store.create("x.canvas"),
            Err(WorkspaceError::NameTaken("x.canvas".into())),
            "коллизия"
        );
        assert_eq!(
            store.create("X.canvas"),
            Err(WorkspaceError::NameTaken("X.canvas".into())),
            "регистронезависимая коллизия"
        );
        assert_eq!(
            store.create("без-расширения"),
            Err(WorkspaceError::NameInvalid("без-расширения".into()))
        );
        assert_eq!(
            store.create(""),
            Err(WorkspaceError::NameInvalid("".into()))
        );
        assert_eq!(
            store.create("X.CANVAS"),
            Err(WorkspaceError::NameInvalid("X.CANVAS".into())),
            "расширение строго .canvas — иначе невидимо для листинга"
        );
        store.create("новый.canvas").expect("создание");
        assert!(store.exists("новый.canvas"));

        // Лимит MAX_CANVASES — следующая отказывает (№16)
        let full = OpfsStore::new();
        for i in 0..MAX_CANVASES {
            full.create(&format!("c{i:05}.canvas"))
                .expect("в пределах лимита");
        }
        assert_eq!(
            full.create("overflow.canvas"),
            Err(WorkspaceError::LimitReached)
        );
    }

    // --- rename: файл + .bak-близнец -----------------------------------------

    #[test]
    fn rename_moves_file_and_bak_twin() {
        let store = store_with(&[("a.canvas", 1), ("a.canvas.bak", 2)]);
        store.rename("a.canvas", "b.canvas").expect("ренейм");
        assert!(!store.exists("a.canvas"));
        assert!(store.exists("b.canvas"));
        assert!(
            store.has_raw("b.canvas.bak"),
            ".bak-близнец переехал вместе с файлом"
        );
        assert!(!store.has_raw("a.canvas.bak"));
        // ренейм в занятое / несуществующее / идемпотентный no-op
        store.create("c.canvas").expect("создание");
        assert_eq!(
            store.rename("b.canvas", "c.canvas"),
            Err(WorkspaceError::NameTaken("c.canvas".into()))
        );
        assert_eq!(
            store.rename("ghost.canvas", "d.canvas"),
            Err(WorkspaceError::NotFound("ghost.canvas".into()))
        );
        assert_eq!(store.rename("b.canvas", "b.canvas"), Ok(()));
    }

    // --- delete: мягкое удаление (№15a) ---------------------------------------

    #[test]
    fn delete_is_soft_and_hidden_from_listing() {
        let store = store_with(&[("gone.canvas", 7), ("gone.canvas.bak", 3)]);
        store.delete("gone.canvas").expect("удаление");
        assert!(!store.exists("gone.canvas"));
        assert!(
            store.has_raw("gone.canvas.bak"),
            "мягкое удаление: .bak на месте"
        );
        assert!(store.list().is_empty(), ".bak скрыт из листинга");
        // повторное удаление уже удалённого
        assert_eq!(
            store.delete("gone.canvas"),
            Err(WorkspaceError::NotFound("gone.canvas".into()))
        );
        // повторное создание того же имени допустимо (бак не мешает)
        store
            .create("gone.canvas")
            .expect("создание после удаления");
        assert_eq!(store.list().len(), 1);
    }

    // --- №31c: битая ссылка и фолбэк ------------------------------------------

    #[test]
    fn resolve_url_canvas_exact_then_case_insensitive() {
        let existing = vec!["Notes.canvas".to_owned(), "second.canvas".to_owned()];
        assert_eq!(
            resolve_url_canvas("Notes.canvas", &existing),
            Some("Notes.canvas"),
            "точное совпадение — как есть"
        );
        assert_eq!(
            resolve_url_canvas("notes.canvas", &existing),
            Some("Notes.canvas"),
            "регистронезависимое — каноническое имя файла"
        );
        assert_eq!(
            resolve_url_canvas("Notes.CANVAS", &existing),
            Some("Notes.canvas"),
            "URL-санитайзер не меняет регистр расширения"
        );
        assert_eq!(
            resolve_url_canvas("missing.canvas", &existing),
            None,
            "файла нет — битая ссылка"
        );
    }

    #[test]
    fn broken_link_fallback_picks_top_existing_recent() {
        let recent = vec![
            ("a.canvas".to_owned(), 100.0),
            ("b.canvas".to_owned(), 300.0),
            ("c.canvas".to_owned(), 200.0),
        ];
        let existing = vec!["a.canvas".to_owned(), "c.canvas".to_owned()];
        // Верхний недавний b не существует — следующий существующий c
        assert_eq!(
            broken_link_fallback("битый.canvas", &recent, &existing).as_deref(),
            Some("c.canvas")
        );
        // Битое имя исключается (даже регистронезависимо) — c становится верхним
        let broken_is_top = vec![("A.CANVAS".to_owned(), 999.0), ("c.canvas".to_owned(), 1.0)];
        assert_eq!(
            broken_link_fallback("a.canvas", &broken_is_top, &existing).as_deref(),
            Some("c.canvas"),
            "недавний = битому имени (регистр) — пропущен"
        );
        // Ничего подходящего — None (вызывающий берёт default.canvas):
        // единственный недавний — само битое имя
        let only_broken = vec![("a.canvas".to_owned(), 5.0)];
        assert_eq!(
            broken_link_fallback("a.canvas", &only_broken, &existing),
            None
        );
        assert_eq!(
            broken_link_fallback("a.canvas", &[], &existing),
            None,
            "пустые недавние — None"
        );
        // Недавние есть, но ни один не существует в хранилище
        let ghosts = vec![("ghost.canvas".to_owned(), 9.0)];
        assert_eq!(broken_link_fallback("a.canvas", &ghosts, &existing), None);
        // Дефолт-канвас как недавний допустим, если он не битое имя
        let recent_default = vec![("default.canvas".to_owned(), 5.0)];
        let existing_default = vec!["default.canvas".to_owned()];
        assert_eq!(
            broken_link_fallback("x.canvas", &recent_default, &existing_default).as_deref(),
            Some("default.canvas")
        );
    }
}
