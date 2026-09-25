use canvas_ui::layout::{FlexLayoutEngine, SceneDim, SceneKind, SceneNode, SceneSize, SceneTrack};
use canvas_ui::{UiRect, UiVec2};

fn main() {
    let slot = UiRect::new(0.0, 0.0, 500.0, 300.0);
    let cells: Vec<SceneNode> = vec![
        SceneNode::leaf(120.0, 20.0),
        SceneNode::leaf(20.0, 20.0),
        SceneNode::leaf(300.0, 20.0),
    ];
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Auto, SceneTrack::Length(60.0), SceneTrack::Fill],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(10.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: cells,
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(slot, &scene);
    for (i, r) in rects.iter().enumerate() {
        println!("{}: x={} y={} w={} h={}", i, r.x, r.y, r.w, r.h);
    }
}
