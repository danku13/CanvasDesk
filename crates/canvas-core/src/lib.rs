//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

mod edgegeom;
mod error;
mod fs_events;
mod io;
mod model;
mod providers;
mod settings;
mod spatial;

pub use edgegeom::{
    bezier_between, curve_point, curve_tangent, distance_point_to_polyline, distance_to_edge,
    draft_curve, edge_at, edge_curve, nearest_side, port_at, port_point, side_normal, tessellate,
    CubicBezier, EDGE_HIT_TOLERANCE, PORT_HIT_PX, TESSELLATION_SEGMENTS,
};
pub use error::CoreError;
pub use fs_events::{
    apply_file_events, normalize_path, path_matches, relative_if_inside, resolve_node_path,
    watched_dirs, FileEvent, NodeChange,
};
pub use model::{Canvas, CanvasdeskExt, Edge, Node, NodeKind, PreviewState, Side};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
pub use settings::{Corner, Settings};
pub use spatial::{SpatialIndex, WorldRect};
