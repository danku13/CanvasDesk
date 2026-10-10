//! FR-104 (мультиканвас C1): обратный канал App → web-слой — обработчик
//! [`WebRequest`]-ов. App платформенно-нейтрален (canvas-web не может быть
//! его зависимостью), поэтому запросы складываются в очередь на стороне
//! App (`pending_web_requests`) и дренажируются обёрткой `TourAwareApp`
//! после КАЖДОГО события цикла (паттерн tour-сигналов FR-028 v2 — latency
//! минимальна, `about_to_wait` был бы реже). Ответы возвращаются
//! `AppEvent`-ами через `EventLoopProxy`.
//!
//! Конвейер (план §3.2):
//! - `CanvasList` → свежий листинг OPFS (глю `opfsList` → зеркало
//!   `OpfsStore` → `list()`) → `AppEvent::CanvasList`;
//! - `CanvasOp` → `WorkspaceStore`-операция (валидация синхронно, мутация
//!   фоном) → `AppEvent::CanvasOpDone` (ошибка — человекочитаемый текст
//!   `WorkspaceError::to_string`);
//! - `CanvasFallback` (№35a → №31c) — открыть канвас-фолбэк: верхний из
//!   недавних, который существует и ≠ занятому имени; недавних нет —
//!   СВЕЖИЙ «Canvas N» (default.canvas может быть сам занят другой
//!   вкладкой — иначе модал зациклится, см. FR-104 §Фолбэк занятости).
//!   TODO(FR-104): C3 заменит на менеджер канвасов.

#![cfg(target_arch = "wasm32")]

use std::path::{Path, PathBuf};

use canvas_app::app::{AppEvent, WebRequest};
use canvas_core::workspace::{auto_name, CanvasEntry, CanvasOp, EntryKind};
use winit::event_loop::EventLoopProxy;

use crate::workspace::WorkspaceStore;

/// Обработать запрос App (fire-and-forget: каждый запрос — свой
/// spawn_local-таск; ответ приедет AppEvent-ом через прокси).
pub(crate) fn handle(request: WebRequest, proxy: &EventLoopProxy<AppEvent>) {
    let proxy = proxy.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match request {
            WebRequest::CanvasList => {
                let entries = crate::opfs_store::workspace_entries().await;
                let _ = proxy.send_event(AppEvent::CanvasList(entries));
            }
            WebRequest::CanvasOp(op) => {
                let error = run_canvas_op(&op);
                let _ = proxy.send_event(AppEvent::CanvasOpDone { op, error });
            }
            WebRequest::CanvasFallback { avoid } => {
                open_fallback(&avoid, &proxy).await;
            }
        }
    });
}

/// Выполнить операцию хранилища (валидация — синхронно по зеркалу;
/// сама мутация уходит фоном внутри `OpfsStore`). Отказ → текст ошибки
/// для `CanvasOpDone` (будущие тосты менеджера C3).
fn run_canvas_op(op: &CanvasOp) -> Option<String> {
    let Some(store) = crate::web_state::opfs_workspace() else {
        return Some("OPFS-хранилище недоступно".to_owned());
    };
    let result = match op {
        CanvasOp::Create { name } => store.create(name),
        CanvasOp::Rename { old, new } => store.rename(old, new),
        CanvasOp::Delete { name } => store.delete(name),
    };
    result.err().map(|err| err.to_string())
}

/// Открыть канвас-фолбэк после «Выбрать другой» (№35a): механика битой
/// ссылки №31c — верхний существующий недавний ≠ `avoid`; недавних нет —
/// свежее автоимя «Canvas N» (НЕ default.canvas: он может быть сам
/// занят, зациклив модал). Открытие — как у DOM-drop/reopen: OPFS-текст
/// → зеркало `OpfsStorage` → `OpenScene`.
async fn open_fallback(avoid: &str, proxy: &EventLoopProxy<AppEvent>) {
    let Ok(root) = crate::opfs::opfs_root().await else {
        tracing::error!(target: "canvas_web", avoid, "фолбэк: OPFS недоступен");
        return;
    };
    // Свежий листинг — имена для выбора фолбэка/автоимени.
    let names = match crate::opfs_store::opfs_list().await {
        Ok(files) => files.into_iter().map(|(name, _)| name).collect::<Vec<_>>(),
        Err(err) => {
            tracing::error!(target: "canvas_web", %err, avoid, "фолбэк: листинг OPFS не получен");
            return;
        }
    };
    let recent = crate::recent::recent_list().await;
    let name = match crate::opfs_store::broken_link_fallback(avoid, &recent, &names) {
        Some(name) => name,
        None => fresh_canvas_name(&names),
    };
    // Текст: существующий файл читается; свежее автоимя сеется пустым
    // (default.canvas сюда не попадает — см. выше).
    let json = match crate::opfs::read_opfs_text(&root, &name).await {
        Ok(Some(text)) => text,
        Ok(None) => seed_canvas_text(&root, &name).await,
        Err(err) => {
            tracing::error!(
                target: "canvas_web",
                file = %name,
                error = ?err,
                "фолбэк: чтение не удалось — сцена не переключается"
            );
            return;
        }
    };
    let Some(storage) = crate::web_state::opfs_storage() else {
        tracing::error!(target: "canvas_web", file = %name, "фолбэк: OPFS-хранилище не инициализировано");
        return;
    };
    storage.seed_mirror(Path::new(&name), &json);
    crate::web_state::set_active(name.clone(), crate::web_state::ActiveKind::Opfs);
    crate::recent::record_recent(&name).await;
    crate::toolbar::set_recent_label(&name);
    tracing::info!(target: "canvas_web", file = %name, avoid, "фолбэк: открыт другой канвас");
    let _ = proxy.send_event(AppEvent::OpenScene {
        path: PathBuf::from(&name),
        json,
        storage: Some(storage),
    });
}

/// Свежее автоимя «Canvas»/«Canvas N» (№6/№39c) по текущему листингу:
/// `avoid` существует в листинге — auto_name с ним не столкнётся.
fn fresh_canvas_name(names: &[String]) -> String {
    let existing: Vec<CanvasEntry> = names
        .iter()
        .map(|name| CanvasEntry {
            name: name.clone(),
            ts: 0,
            kind: EntryKind::Opfs,
            repo: None,
        })
        .collect();
    auto_name(&existing)
}

/// Прописать новый пустой канвас в OPFS (сеяние свежего автоимени фолбэка)
/// и вернуть его текст для `OpenScene`.
async fn seed_canvas_text(root: &web_sys::FileSystemDirectoryHandle, name: &str) -> String {
    let text = crate::workspace::empty_canvas_text();
    if let Err(err) = crate::opfs::write_opfs_text(root, name, &text).await {
        tracing::warn!(
            target: "canvas_web",
            file = name,
            error = ?err,
            "фолбэк: сеяние нового канваса не записалось (автосейв повторит)"
        );
    }
    // Зеркало workspace должно увидеть новую запись сразу.
    let _ = crate::opfs_store::seed_workspace().await;
    text
}
