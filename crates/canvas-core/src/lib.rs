//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

mod error;
mod io;
mod model;
mod providers;
mod spatial;

pub use error::CoreError;
pub use model::{Canvas, CanvasdeskExt, Edge, Node, NodeKind, PreviewState, Side};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
pub use spatial::{SpatialIndex, WorldRect};
