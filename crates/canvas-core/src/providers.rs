//! Трейты платформенных сервисов (SPEC §4). Реализации — в canvas-shell
//! (Windows) и canvas-render; core тестируется моками на любой ОС.

use std::path::Path;

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
