//! Трейты платформенных сервисов (SPEC §4). Реализации — в canvas-shell
//! (Windows) и canvas-render; core тестируется моками на любой ОС.
//! M8/W3 (wasm-port §6): сюда же добавлены трейты сервисов сеанса —
//! буфер обмена, файловый вотчер, тамбнейл-очередь (+Noop-заглушки по
//! образцу NoopThumbnailProvider); инъекция — в `App::new` (canvas-app).

use std::path::{Path, PathBuf};

use crate::CoreError;

/// RGBA8-растр (тамбнейл или превью), готовый к загрузке в GPU-атлас.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Системные тамбнейлы файлов (SPEC §7.1). Реализация — canvas-shell (T6),
/// запросы выполняются в пуле потоков, не в рендер-потоке.
pub trait ThumbnailProvider {
    /// Вернуть тамбнейл файла, вписанный в `max_size` по длинной стороне.
    fn thumbnail(&self, path: &Path, max_size: u32) -> Result<Thumbnail, CoreError>;
}

/// Живое превью содержимого файла (SPEC §6.2, §7.2). Реализации: image/text/PDF
/// (T11) и out-of-process preview host (T12).
pub trait PreviewProvider {
    /// Вернуть превью файла под размер карточки `width`x`height`.
    fn preview(&self, path: &Path, width: u32, height: u32) -> Result<Thumbnail, CoreError>;
}

/// Интеграция с оболочкой ОС: открытие файлов, режим десктопа (SPEC §7.4).
pub trait ShellIntegration {
    /// Открыть файл в ассоциированном приложении (как двойной клик в Explorer).
    fn open_file(&self, path: &Path) -> Result<(), CoreError>;
}

/// Приоритет заказа тамбнейла (SPEC §7.1). Переехал из canvas-shell
/// (M8/W3): тип протокола [`ThumbBackend`], нужен обеим сторонам трейта.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Нода в viewport — обработать первой.
    High,
    /// Нода вне viewport — фоновая подгрузка.
    Normal,
}

/// Буфер обмена ОС (T7): текстовые операции редактирования. Ошибки —
/// внутри реализации (warn), редактирование не ломается; недоступный
/// буфер — no-op (деградация, как `Clipboard(Option)` в app).
pub trait ClipboardBackend {
    /// Записать текст в буфер обмена.
    fn set_text(&mut self, text: String);
    /// Прочитать текст из буфера обмена; None — недоступен/не текст.
    fn get_text(&mut self) -> Option<String>;
}

/// Noop-буфер обмена (паттерн NoopThumbnailProvider): тесты и деградация
/// на платформах без буфера (web до подключения navigator.clipboard).
pub struct NoopClipboard;

impl ClipboardBackend for NoopClipboard {
    fn set_text(&mut self, _text: String) {}
    fn get_text(&mut self) -> Option<String> {
        None
    }
}

/// Файловый вотчер (T10): события ФС приходят приложению через
/// `FileEventSender`, переданный при создании backend'а; трейт — только
/// синхронизация набора директорий с моделью.
pub trait WatchBackend {
    /// Синхронизировать набор вотченных директорий (diff; повторный
    /// вызов с тем же набором — no-op).
    fn sync_dirs(&mut self, dirs: &[PathBuf]);
}

/// Noop-вотчер: событий ФС нет (web: перечитывание по жесту
/// «Перезагрузить», план §3.2; тесты).
pub struct NoopWatch;

impl WatchBackend for NoopWatch {
    fn sync_dirs(&mut self, _dirs: &[PathBuf]) {}
}

/// Очередь тамбнейлов (T6): заказ растеризации, выдача готовых результатов
/// и длина очереди (HUD). Результаты — RGBA8 [`Thumbnail`] из core.
pub trait ThumbBackend {
    /// Заказать тамбнейл ноды (дедупликация — внутри реализации).
    fn request(&self, priority: Priority, node: usize, path: PathBuf);
    /// Забрать готовые результаты: (индекс ноды, растр или отказ).
    fn drain(&self) -> Vec<(usize, Option<Thumbnail>)>;
    /// Текущая длина очереди (HUD F3).
    fn queue_len(&self) -> usize;
}

/// Noop-очередь тамбнейлов (тесты; платформы без системных превью).
pub struct NoopThumbs;

impl ThumbBackend for NoopThumbs {
    fn request(&self, _priority: Priority, _node: usize, _path: PathBuf) {}
    fn drain(&self) -> Vec<(usize, Option<Thumbnail>)> {
        Vec::new()
    }
    fn queue_len(&self) -> usize {
        0
    }
}

/// Изолированное key-value хранилище состояния виджетов (T21-A, M8/W3):
/// натив — таблица widget_state в cache.db (shell, SQLite); web —
/// localStorage (W11, план §3.2). Ключи — (id ноды, ключ состояния).
pub trait WidgetStateBackend {
    /// Прочитать значение состояния ноды.
    fn get(&self, node_id: &str, key: &str) -> Option<String>;
    /// Записать значение состояния ноды.
    fn set(&mut self, node_id: &str, key: &str, value: &str);
}

/// Хранилище состояния виджетов в памяти — заглушка для тестов
/// (паттерн NoopThumbnailProvider) и web-сборки до W11.
#[derive(Default)]
pub struct MemWidgetState {
    entries: std::sync::Mutex<std::collections::BTreeMap<(String, String), String>>,
}

impl WidgetStateBackend for MemWidgetState {
    fn get(&self, node_id: &str, key: &str) -> Option<String> {
        self.entries
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .get(&(node_id.to_owned(), key.to_owned()))
            .cloned()
    }

    fn set(&mut self, node_id: &str, key: &str, value: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .insert((node_id.to_owned(), key.to_owned()), value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noop-заглушки сервисов (паттерн NoopThumbnailProvider): операции
    /// без паник, чтения — пустые результаты.
    #[test]
    fn noop_stubs_are_inert() {
        let mut clipboard = NoopClipboard;
        clipboard.set_text("тест".into());
        assert_eq!(clipboard.get_text(), None);

        let mut watch = NoopWatch;
        watch.sync_dirs(&[PathBuf::from("/tmp/canvas")]);

        let thumbs = NoopThumbs;
        thumbs.request(Priority::High, 0, PathBuf::from("/tmp/a.png"));
        assert!(thumbs.drain().is_empty());
        assert_eq!(thumbs.queue_len(), 0);
    }
}
