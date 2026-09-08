//! canvas-shell — Windows-only интеграция с оболочкой:
//! тамбнейлы, preview handlers, drag-drop, встройка в WorkerW (SPEC §7).
//! Весь unsafe — только здесь, с SAFETY-комментариями.

pub mod cache;
// Режим десктопа (T15): чистое ядро (desktop/mod.rs) кроссплатформенно —
// тесты гоняются в CI на Linux; Win32-механика (hierarchy/attach/monitor)
// — cfg(windows) внутри mod.rs (рекомендация воркера T15-A).
pub mod desktop;
pub mod dragdrop;
pub mod search;
pub mod service;
#[cfg(windows)]
pub mod thumbs;
pub mod watcher;
#[cfg(windows)]
pub use desktop::{
    dpi_to_scale, plan_style_scrub, recovery_action, union_rects, verify_styles, DesktopEvent,
    EmbedStrategy, RecoveryAction, ScreenRect, StyleMismatch, StylePlan, DPI_POLL_MS,
    PARENT_POLL_MS,
};

pub use cache::{default_cache_dir, default_config_path, ThumbCache, SIZE_CLASS};
pub use search::{
    IndexEntry, SearchCommand, SearchEvent, SearchHit, SearchResponder, SearchService,
};
pub use service::{downscale_to_fit, Priority, ThumbService};
#[cfg(windows)]
pub use thumbs::ShellThumbnailProvider;
pub use watcher::{FileEventSender, WatchService, DEBOUNCE};

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
