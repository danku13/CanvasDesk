//! canvas-shell — Windows-only интеграция с оболочкой:
//! тамбнейлы, preview handlers, drag-drop, встройка в WorkerW (SPEC §7).
//! Весь unsafe — только здесь, с SAFETY-комментариями.

pub mod cache;
pub mod service;
#[cfg(windows)]
pub mod thumbs;

pub use cache::{default_cache_dir, ThumbCache, SIZE_CLASS};
pub use service::{downscale_to_fit, Priority, ThumbService};
#[cfg(windows)]
pub use thumbs::ShellThumbnailProvider;

use canvas_core::{CoreError, Thumbnail};

/// Провайдер-заглушка для сборки вне Windows: тамбнейлы недоступны,
/// приложение работает с иконками-заглушками (SPEC §4 — core кроссплатформен).
pub struct NoopThumbnailProvider;

impl canvas_core::ThumbnailProvider for NoopThumbnailProvider {
    fn thumbnail(&self, path: &std::path::Path, _max_size: u32) -> Result<Thumbnail, CoreError> {
        Err(CoreError::Platform(format!(
            "тамбнейлы поддерживаются только на Windows: {}",
            path.display()
        )))
    }
}
