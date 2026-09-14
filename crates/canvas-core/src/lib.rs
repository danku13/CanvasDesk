//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

mod edgegeom;
mod error;
mod focus;
mod fs_events;
mod io;
mod layout;
mod model;
mod providers;
mod settings;
mod spatial;

pub use edgegeom::{
    bezier_between, curve_point, curve_tangent, distance_point_to_polyline, distance_to_edge,
    draft_curve, edge_at, edge_curve, edge_endpoint, edge_midpoint, edge_polyline, nearest_side,
    port_at, port_point, retarget_edge, route_polyline, side_normal, tessellate, CubicBezier,
    EdgeEnd, AVOID_MARGIN, EDGE_HIT_TOLERANCE, MAX_DETOURS, PORT_HIT_PX, TESSELLATION_SEGMENTS,
};
pub use error::CoreError;
pub use focus::{focus_set, FocusSeed, FocusSet};
pub use fs_events::{
    apply_file_events, normalize_path, path_matches, relative_if_inside, resolve_node_path,
    watched_dirs, FileEvent, NodeChange,
};
pub use layout::{
    plan_related_layout, LayoutMode, LayoutPlan, LEVEL_GAP, RADIAL_RING_STEP, SIBLING_GAP,
};
pub use model::{
    group_add_children, group_children, group_expand_to_children, group_materialize_children,
    group_remove_child, parent_index, plan_push_out, subtree_ids, Canvas, CanvasdeskExt, Edge,
    EdgeLineStyle, EdgeThickness, Node, NodeKind, PreviewState, Side,
};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
pub use settings::{
    clamp_port_zone, next_port_zone, Corner, GridDensity, GridStyle, Settings, Theme,
    PORT_ZONE_MAX, PORT_ZONE_MIN, PORT_ZONE_PRESETS,
};
pub use spatial::{SpatialIndex, WorldRect};
