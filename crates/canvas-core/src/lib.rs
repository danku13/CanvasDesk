//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

mod error;
mod model;
mod providers;

pub use error::CoreError;
pub use model::{Canvas, Edge, Node, Side};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
