//! M8/W10 (wasm-port §6, §3.2): превью картинок — `WebImageThumbs` за
//! нейтральным трейтом `ThumbBackend` (натив — `ThumbService`:
//! IShellItemImageFactory + SQLite-кэш, план §3.2).
//!
//! Пайплайн (план W10): заказ `request(node, path)` → `spawn_local`:
//! чтение файла из OPFS (`files/<имя>` — путь у ноды от DOM-drop'а W10)
//! → `createImageBitmap` (нативный декод браузера, не wasm — риск
//! «однопоточный декод» митигирован, §7) → OffscreenCanvas downscale в
//! ячейку атласа 256² → `getImageData` → RGBA [`Thumbnail`] → очередь →
//! побудка `AppEvent::ThumbsReady` → `drain()` из app загружает в
//! существующий thumbs-атлас (`renderer.set_thumbnail`).
//!
//! «Заглушка для прочих типов» (приёмка): не-картинки и ошибки декода
//! дают `None`-результат → негативный кэш app (`thumbs_failed`) — нода
//! остаётся карточкой без превью, перезаказов нет.
//!
//! Дедупликация — как у натива: повторный `request` той же ноды до
//! `drain` отбрасывается. Кэша на диске нет (rusqlite на wasm
//! недоступен): декод дешёвый (нативный кодек) + лимит кадра атласа.

// Нативная компиляция: модуль собирается в workspace (rlib), web-типы —
// заглушки (контрольная сборка §3.5); вызовы невозможны по построению —
// app_spawn нативной ветки ставит NoopThumbs. Чистые функции покрыты
// тестами на любой ОС.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use canvas_core::{Priority, ThumbBackend, Thumbnail};

use canvas_app::app::AppEvent;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Длинная сторона тамбнейла: ячейка атласа 256² (render/thumbs;
/// паритет SIZE_CLASS нативного кэша).
pub const THUMB_MAX: u32 = 256;

/// Расширения, декодируемые `createImageBitmap` в Chromium (продуктовый
/// таргет): PNG/JPEG — приёмка W10, остальные — бонус; прочие типы —
/// честная заглушка (None → негативный кэш app).
pub fn is_image_ext(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" | "ico"
    )
}

/// Разбор OPFS-пути файл-ноды: ровно `files/<имя>` (приём W10 пишет в
/// подкаталог `files/`; ведущий `/` — OPFS-абсолютный путь, не диск).
/// Прочие пути не обслуживаем (None): на web диск недоступен по
/// построению.
pub fn split_files_path(path: &Path) -> Option<(String, String)> {
    let rest = path.to_str()?.strip_prefix('/')?;
    let (dir, name) = rest.split_once('/')?;
    if dir.is_empty() || name.is_empty() || name.contains('/') {
        return None;
    }
    Some((dir.to_owned(), name.to_owned()))
}

/// Вписать размер в `max` по длинной стороне (апскейл запрещён: маленькая
/// картинка остаётся своего размера — атлас ужимает сам).
pub fn fit_size(width: u32, height: u32, max: u32) -> (u32, u32) {
    let long = width.max(height);
    if long == 0 || long <= max {
        return (width.max(1), height.max(1));
    }
    let scale = f64::from(max) / f64::from(long);
    (
        ((f64::from(width) * scale).round() as u32).max(1),
        ((f64::from(height) * scale).round() as u32).max(1),
    )
}

/// Общее состояние: заказы в декоде (дедуп) + готовые результаты.
struct Shared {
    pending: RefCell<HashSet<usize>>,
    done: RefCell<Vec<(usize, Option<Thumbnail>)>>,
}

/// Провайдер превью картинок для web ([`ThumbBackend`]). Живёт в App всю
/// сессию; `Rc` — весь web-код на главном потоке (winit web + spawn_local).
pub struct WebImageThumbs {
    shared: Rc<Shared>,
    proxy: winit::event_loop::EventLoopProxy<AppEvent>,
}

impl WebImageThumbs {
    /// Новый провайдер; `proxy` — побудка event loop готовыми
    /// результатами (паттерн ThumbService натива).
    pub fn new(proxy: winit::event_loop::EventLoopProxy<AppEvent>) -> Self {
        Self {
            shared: Rc::new(Shared {
                pending: RefCell::new(HashSet::new()),
                done: RefCell::new(Vec::new()),
            }),
            proxy,
        }
    }
}

impl ThumbBackend for WebImageThumbs {
    /// Заказать тамбнейл: декод асинхронный, результат — через `drain`
    /// после `AppEvent::ThumbsReady`. Повторный заказ ноды в работе —
    /// no-op (дедуп).
    fn request(&self, _priority: Priority, node: usize, path: PathBuf) {
        if !self.shared.pending.borrow_mut().insert(node) {
            return;
        }
        tracing::debug!(target: "canvas_web", node, path = %path.display(), "превью заказано");
        let shared = Rc::clone(&self.shared);
        let proxy = self.proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = decode_thumbnail(&path).await;
            shared.pending.borrow_mut().remove(&node);
            let ready = result.is_some();
            shared.done.borrow_mut().push((node, result));
            let _ = proxy.send_event(AppEvent::ThumbsReady);
            if !ready {
                tracing::debug!(target: "canvas_web", node, "превью не удалось (заглушка)");
            }
        });
    }

    /// Забрать готовые результаты (вызывается в AppEvent::ThumbsReady).
    fn drain(&self) -> Vec<(usize, Option<Thumbnail>)> {
        std::mem::take(&mut *self.shared.done.borrow_mut())
    }

    /// Число заказов в декоде (HUD F3; аналог очереди натива).
    fn queue_len(&self) -> usize {
        self.shared.pending.borrow().len()
    }
}

/// Декод файла OPFS → [`Thumbnail`] (None — не картинка/битое/нет файла).
/// Wasm-only: JS-рунтайм обязателен (OPFS/createImageBitmap); чистая
/// логика выше покрыта нативными тестами.
#[cfg(target_arch = "wasm32")]
async fn decode_thumbnail(path: &Path) -> Option<Thumbnail> {
    let (dir, name) = match split_files_path(path) {
        Some(pair) => pair,
        None => {
            tracing::debug!(target: "canvas_web", path = %path.display(), "путь вне files/ — превью нет");
            return None;
        }
    };
    if !is_image_ext(Path::new(&name)) {
        tracing::debug!(target: "canvas_web", file = %name, "тип не картинка — превью-заглушка");
        return None;
    }
    let result = decode_from_opfs(&dir, &name).await;
    match result {
        Ok(thumb) => {
            tracing::info!(target: "canvas_web", file = %name, width = thumb.width, height = thumb.height,
                "превью готово");
            Some(thumb)
        }
        Err(err) => {
            tracing::debug!(target: "canvas_web", file = %name, error = ?err, "декод превью не удался");
            None
        }
    }
}

/// Чтение из OPFS и декод (вся тяжёлая работа — кодеки браузера).
#[cfg(target_arch = "wasm32")]
async fn decode_from_opfs(dir: &str, name: &str) -> Result<Thumbnail, wasm_bindgen::JsValue> {
    use wasm_bindgen_futures::JsFuture;
    let root = crate::opfs::opfs_root().await?;
    let dir_handle: web_sys::FileSystemDirectoryHandle =
        JsFuture::from(root.get_directory_handle(dir)).await?.into();
    let file_handle: web_sys::FileSystemFileHandle =
        JsFuture::from(dir_handle.get_file_handle(name))
            .await?
            .into();
    let file: web_sys::File = JsFuture::from(file_handle.get_file()).await?.into();
    let window = web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("нет window"))?;
    let bitmap_value = JsFuture::from(window.create_image_bitmap_with_blob(&file)?).await?;
    let bitmap: web_sys::ImageBitmap = bitmap_value.dyn_into()?;
    let (tw, th) = fit_size(bitmap.width(), bitmap.height(), THUMB_MAX);
    let canvas = web_sys::OffscreenCanvas::new(tw, th)?;
    let ctx_value = canvas
        .get_context("2d")?
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("нет 2d-контекста"))?;
    let ctx: web_sys::OffscreenCanvasRenderingContext2d = ctx_value.dyn_into()?;
    ctx.set_image_smoothing_enabled(true);
    let _ = ctx.draw_image_with_image_bitmap_and_dw_and_dh(
        &bitmap,
        0.0,
        0.0,
        f64::from(tw),
        f64::from(th),
    );
    let data = ctx.get_image_data(0, 0, tw as i32, th as i32)?;
    Ok(Thumbnail {
        width: tw,
        height: th,
        rgba: data.data().0,
    })
}

/// Нативная заглушка: тело `request` на нативе не исполняется (App
/// нативной ветки — ThumbService/NoopThumbs); компиляционный контракт.
#[cfg(not(target_arch = "wasm32"))]
async fn decode_thumbnail(_path: &Path) -> Option<Thumbnail> {
    None
}

#[cfg(test)]
mod tests {
    use super::{fit_size, is_image_ext, split_files_path};
    use std::path::Path;

    #[test]
    fn image_extension_classification() {
        for name in [
            "a.png", "b.JPG", "c.jpeg", "d.webp", "e.gif", "f.bmp", "g.avif", "h.ico",
        ] {
            assert!(is_image_ext(Path::new(name)), "{name}");
        }
        for name in ["a.txt", "b.canvas", "c", "d.pdf", "e.svg", "f"] {
            assert!(!is_image_ext(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn files_path_parsing() {
        assert_eq!(
            split_files_path(Path::new("/files/фото.png")),
            Some(("files".into(), "фото.png".into()))
        );
        assert_eq!(
            split_files_path(Path::new("files/a.png")),
            None,
            "без ведущего / — не OPFS-абсолютный, не обслуживаем"
        );
        assert_eq!(
            split_files_path(Path::new("/files/a/b.png")),
            None,
            "вложенность — не схема приёма"
        );
        assert_eq!(split_files_path(Path::new("/files/")), None);
        assert_eq!(
            split_files_path(Path::new("/other/a.png")),
            Some(("other".into(), "a.png".into())),
            "схема разделения общая — приём фильтрует типы по расширению"
        );
    }

    #[test]
    fn fit_size_caps_long_side_and_no_upscale() {
        assert_eq!(fit_size(1024, 512, 256), (256, 128));
        assert_eq!(fit_size(512, 1024, 256), (128, 256));
        assert_eq!(
            fit_size(256, 128, 256),
            (256, 128),
            "ровно вписано — как есть"
        );
        assert_eq!(fit_size(100, 50, 256), (100, 50), "апскейл запрещён");
        assert_eq!(fit_size(0, 0, 256), (1, 1), "нулевые — минимум 1");
        assert_eq!(fit_size(4096, 4096, 256), (256, 256));
    }

    /// Компиляционный контракт: WebImageThumbs реализует трейт (нативная
    /// сборка — только типы; вызовы — wasm).
    #[test]
    fn provider_satisfies_thumb_backend() {
        fn assert_backend<T: canvas_core::ThumbBackend>() {}
        assert_backend::<super::WebImageThumbs>();
    }
}
