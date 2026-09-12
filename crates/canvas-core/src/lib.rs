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
    draft_curve, edge_at, edge_curve, edge_midpoint, edge_polyline, nearest_side, port_at,
    port_point, route_polyline, side_normal, tessellate, CubicBezier, AVOID_MARGIN,
    EDGE_HIT_TOLERANCE, MAX_DETOURS, PORT_HIT_PX, TESSELLATION_SEGMENTS,
};
pub use error::CoreError;
pub use fs_events::{
    apply_file_events, normalize_path, path_matches, relative_if_inside, resolve_node_path,
    watched_dirs, FileEvent, NodeChange,
};
pub use model::{
    group_children, Canvas, CanvasdeskExt, Edge, EdgeLineStyle, EdgeThickness, Node, NodeKind,
    PreviewState, Side,
};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
pub use settings::{Corner, GridDensity, GridStyle, Settings, Theme};
pub use spatial::{SpatialIndex, WorldRect};
