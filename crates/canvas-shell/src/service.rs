//! ThumbService: пул из 4 worker-потоков с приоритетной очередью (SPEC §7.1, T6).
//!
//! Заказы — `request(node, path, priority)`, результаты — `drain()` из канала
//! в рендер-поток. Сначала проверяется SQLite-кэш (инвалидация по mtime),
//! промах — COM-запрос к IShellItemImageFactory. Рендер-поток не блокируется:
//! весь I/O и COM — в worker'ах (AGENTS.md).

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Condvar, Mutex};

use canvas_core::{Thumbnail, ThumbnailProvider};

use crate::cache::{file_mtime_secs, ThumbCache, SIZE_CLASS};

/// Число worker-потоков пула (SPEC §7.1).
const WORKERS: usize = 4;

/// Приоритет заказа: видимые ноды впереди остальных (SPEC §7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Нода в viewport — обработать первой.
    High,
    /// Нода вне viewport — фоновая подгрузка.
    Normal,
}

/// Заказ на тамбнейл одной ноды.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbRequest {
    /// Индекс ноды в `Canvas.nodes` (стабилен в M1 — допущение как у SpatialIndex).
    pub node: usize,
    pub path: PathBuf,
    /// Класс размера (длинная сторона), px.
    pub max_size: u32,
}

/// Очередь с двумя классами приоритета, FIFO внутри класса.
/// Чистая структура без COM/потоков — юнит-тестируется.
#[derive(Default)]
struct RequestQueue {
    high: VecDeque<ThumbRequest>,
    normal: VecDeque<ThumbRequest>,
}

impl RequestQueue {
    fn push(&mut self, priority: Priority, request: ThumbRequest) {
        match priority {
            Priority::High => self.high.push_back(request),
            Priority::Normal => self.normal.push_back(request),
        }
    }

    fn pop(&mut self) -> Option<ThumbRequest> {
        self.high.pop_front().or_else(|| self.normal.pop_front())
    }

    fn len(&self) -> usize {
        self.high.len() + self.normal.len()
    }
}

/// Разделяемое с worker'ами состояние: очередь + множество активных нод
/// (в очереди или в обработке) для дедупликации заказов.
#[derive(Default)]
struct Shared {
    queue: RequestQueue,
    /// Ноды, для которых тамбнейл уже заказан, но результат ещё не забран
    /// через `drain` — повторные заказы этих нод отбрасываются.
    active: HashSet<usize>,
}

/// Вписать тамбнейл в `max_size` по длинной стороне (nearest-neighbor).
/// IShellItemImageFactory с SIIGBF_BIGGERSIZEOK может вернуть больше запрошенного —
/// атлас ячеек 256² такое не примет. Чистая функция — тестируется без COM.
pub fn downscale_to_fit(thumb: Thumbnail, max_size: u32) -> Thumbnail {
    let long = thumb.width.max(thumb.height);
    if long <= max_size || long == 0 {
        return thumb;
    }
    let width = ((u64::from(thumb.width) * u64::from(max_size)) / u64::from(long)) as u32;
    let height = ((u64::from(thumb.height) * u64::from(max_size)) / u64::from(long)) as u32;
    let (width, height) = (width.max(1), height.max(1));
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let src_x = (x * thumb.width / width).min(thumb.width - 1);
            let src_y = (y * thumb.height / height).min(thumb.height - 1);
            let src = ((src_y * thumb.width + src_x) * 4) as usize;
            let dst = ((y * width + x) * 4) as usize;
            rgba[dst..dst + 4].copy_from_slice(&thumb.rgba[src..src + 4]);
        }
    }
    Thumbnail {
        width,
        height,
        rgba,
    }
}

type Notify = Arc<dyn Fn() + Send + Sync>;

/// Пул тамбнейлов. Живёт всё время работы приложения; потоки завершаются
/// вместе с процессом (задания безбожно прерывать — побочных эффектов, кроме
/// записи в пересоздаваемый кэш, нет).
pub struct ThumbService {
    shared: Arc<(Mutex<Shared>, Condvar)>,
    results: Receiver<(usize, Option<Thumbnail>)>,
}

impl ThumbService {
    /// Запустить пул: `provider` — платформенный провайдер (ShellThumbnailProvider
    /// на Windows), `cache` — None при ошибке открытия (работаем без кэша),
    /// `notify` — пробуждение event loop при новых результатах (EventLoopProxy).
    pub fn new(
        provider: Arc<dyn ThumbnailProvider + Send + Sync>,
        cache: Option<ThumbCache>,
        notify: Option<Notify>,
    ) -> Self {
        let shared: Arc<(Mutex<Shared>, Condvar)> = Arc::default();
        let (tx, rx) = mpsc::channel::<(usize, Option<Thumbnail>)>();
        let cache = Arc::new(Mutex::new(cache));
        for _ in 0..WORKERS {
            let shared = Arc::clone(&shared);
            let provider = Arc::clone(&provider);
            let cache = Arc::clone(&cache);
            let tx = tx.clone();
            let notify = notify.clone();
            std::thread::spawn(move || worker_loop(shared, provider, cache, tx, notify));
        }
        Self {
            shared,
            results: rx,
        }
    }

    /// Заказать тамбнейл ноды. Дедупликация: повторный заказ ноды, чей результат
    /// ещё не забран через `drain`, отбрасывается.
    pub fn request(&self, priority: Priority, node: usize, path: PathBuf) {
        let (lock, condvar) = &*self.shared;
        let mut shared = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !shared.active.insert(node) {
            return;
        }
        shared.queue.push(
            priority,
            ThumbRequest {
                node,
                path,
                max_size: SIZE_CLASS,
            },
        );
        condvar.notify_one();
    }

    /// Забрать готовые результаты (рендер-поток): (нода, Some — тамбнейл,
    /// None — ошибка выборки, например битая ссылка). Любой результат снимает
    /// ноду с учёта — иначе она заказывалась бы каждый кадр.
    pub fn drain(&self) -> Vec<(usize, Option<Thumbnail>)> {
        let mut done = Vec::new();
        while let Ok(pair) = self.results.try_recv() {
            done.push(pair);
        }
        if !done.is_empty() {
            let (lock, _) = &*self.shared;
            let mut shared = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            for (node, _) in &done {
                shared.active.remove(node);
            }
        }
        done
    }

    /// Длина очереди (для HUD).
    pub fn queue_len(&self) -> usize {
        let (lock, _) = &*self.shared;
        match lock.lock() {
            Ok(shared) => shared.queue.len(),
            Err(_) => 0,
        }
    }
}

/// Цикл worker'а: ждать заказ → кэш → COM-провайдер → запись в кэш → канал.
fn worker_loop(
    shared: Arc<(Mutex<Shared>, Condvar)>,
    provider: Arc<dyn ThumbnailProvider + Send + Sync>,
    cache: Arc<Mutex<Option<ThumbCache>>>,
    tx: mpsc::Sender<(usize, Option<Thumbnail>)>,
    notify: Option<Notify>,
) {
    loop {
        let request = {
            let (lock, condvar) = &*shared;
            let mut shared = match lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            loop {
                if let Some(request) = shared.queue.pop() {
                    break request;
                }
                shared = match condvar.wait(shared) {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
            }
        };

        let mtime = file_mtime_secs(&request.path);
        // Сначала кэш (инвалидация по mtime внутри get), промах — COM-запрос
        let cached = mtime.and_then(|mtime| {
            let guard = cache.lock().ok()?;
            guard
                .as_ref()
                .and_then(|cache| cache.get(&request.path, mtime, request.max_size))
        });
        let result = match cached {
            Some(thumb) => {
                tracing::debug!(path = %request.path.display(), "тамбнейл из кэша");
                Some(thumb)
            }
            None => match provider.thumbnail(&request.path, request.max_size) {
                Ok(thumb) => {
                    let thumb = downscale_to_fit(thumb, request.max_size);
                    if let Some(mtime) = mtime {
                        if let Ok(guard) = cache.lock() {
                            if let Some(cache) = guard.as_ref() {
                                if let Err(err) =
                                    cache.put(&request.path, mtime, request.max_size, &thumb)
                                {
                                    tracing::warn!(%err, "запись в тамбнейл-кэш пропущена");
                                }
                            }
                        }
                    }
                    Some(thumb)
                }
                Err(err) => {
                    tracing::debug!(path = %request.path.display(), %err, "тамбнейл недоступен");
                    None
                }
            },
        };
        if tx.send((request.node, result)).is_err() {
            // Приёмник умер (приложение закрывается) — worker больше не нужен
            return;
        }
        if let Some(notify) = &notify {
            notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(node: usize) -> ThumbRequest {
        ThumbRequest {
            node,
            path: PathBuf::from(format!("C:/f{node}.png")),
            max_size: SIZE_CLASS,
        }
    }

    /// Видимые ноды обрабатываются раньше фоновых, FIFO внутри класса.
    #[test]
    fn priority_order() {
        let mut queue = RequestQueue::default();
        queue.push(Priority::Normal, req(1));
        queue.push(Priority::High, req(2));
        queue.push(Priority::High, req(3));
        queue.push(Priority::Normal, req(4));
        assert_eq!(queue.len(), 4);
        assert_eq!(queue.pop().map(|r| r.node), Some(2));
        assert_eq!(queue.pop().map(|r| r.node), Some(3));
        assert_eq!(queue.pop().map(|r| r.node), Some(1));
        assert_eq!(queue.pop().map(|r| r.node), Some(4));
        assert_eq!(queue.pop(), None);
    }

    /// Downscale: длинная сторона приводится к max_size, пропорции сохраняются.
    #[test]
    fn downscale_preserves_aspect() {
        let thumb = Thumbnail {
            width: 1024,
            height: 512,
            rgba: vec![7u8; 1024 * 512 * 4],
        };
        let out = downscale_to_fit(thumb, 256);
        assert_eq!((out.width, out.height), (256, 128));
        assert_eq!(out.rgba.len(), 256 * 128 * 4);
        assert!(out.rgba.iter().all(|&b| b == 7));
    }

    /// Тамбнейл в пределах max_size возвращается как есть.
    #[test]
    fn downscale_noop_when_fits() {
        let thumb = Thumbnail {
            width: 100,
            height: 200,
            rgba: vec![1u8; 100 * 200 * 4],
        };
        let out = downscale_to_fit(thumb.clone(), 256);
        assert_eq!(out, thumb);
    }

    /// Вырожденные размеры не паникуют.
    #[test]
    fn downscale_degenerate() {
        let thumb = Thumbnail {
            width: 0,
            height: 0,
            rgba: Vec::new(),
        };
        let out = downscale_to_fit(thumb, 256);
        assert_eq!((out.width, out.height), (0, 0));
    }

    /// Дедупликация: повторный request ноды до drain игнорируется.
    #[test]
    fn dedup_until_drained() {
        let service = ThumbService::new(Arc::new(NoopProvider), None, None);
        service.request(Priority::High, 5, PathBuf::from("C:/x.png"));
        service.request(Priority::High, 5, PathBuf::from("C:/x.png"));
        service.request(Priority::Normal, 6, PathBuf::from("C:/y.png"));
        // Нода 5 попала в очередь один раз (уже могла уйти worker'у — active всё равно один)
        let queue = service.queue_len();
        assert!(queue <= 2);
        // Результат ошибки NoopProvider снимает ноду с учёта
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let _ = service.drain();
            let (lock, _) = &*service.shared;
            if lock.lock().map(|s| s.active.is_empty()).unwrap_or(false) {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "workers не отработали"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Провайдер-заглушка: всегда ошибка (нет COM в тесте очереди).
    struct NoopProvider;

    impl ThumbnailProvider for NoopProvider {
        fn thumbnail(
            &self,
            path: &std::path::Path,
            _max_size: u32,
        ) -> Result<Thumbnail, canvas_core::CoreError> {
            Err(canvas_core::CoreError::Platform(format!(
                "noop: {}",
                path.display()
            )))
        }
    }
}
