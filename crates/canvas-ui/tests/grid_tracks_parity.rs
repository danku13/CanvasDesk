//! FR-074 (ВРЕМЕННЫЙ файл — удаляется в FR-068 W4 вместе с taffy):
//! parity Auto/minmax-треков FlexLayoutEngine против TaffyBackend.
//!
//! Оракул — taffy 0.14 (полный CSS Grid): сцены с Auto/MinMax-треками;
//! ожидание — побитовое совпадение rect'ов на целых входах (round-layout
//! у обоих движков). min-content ≠ max-content сознательно не тестируется
//! (движок моделирует один «контент» — документированное упрощение).
//! Ячейки — Auto (stretch на область) и definite (собственный размер) —
//! обе семантики grid-item.
//!
//! Запуск: `cargo test -p canvas-ui --test grid_tracks_parity --features taffy`.

#![cfg(feature = "taffy")]

use canvas_ui::layout::{
    FlexLayoutEngine, LayoutBackend, SceneDim, SceneKind, SceneNode, SceneSize, SceneTrack,
    TaffyBackend, TrackMax, TrackMin,
};
use canvas_ui::{UiRect, UiVec2};

const SLOT: UiRect = UiRect::new(0.0, 0.0, 500.0, 300.0);

/// Grid с треками: ячейки `(w, h, span)`; `w == 0` — авто-ячейка
/// (SceneNode::default(), stretch на область трека).
fn grid(cols: Vec<SceneTrack>, cells: &[(f32, f32, u16)], gap: UiVec2) -> SceneNode {
    let children: Vec<SceneNode> = cells
        .iter()
        .map(|&(w, h, span)| {
            let n = if w == 0.0 {
                SceneNode::default()
            } else {
                SceneNode::leaf(w, h)
            };
            n.spanning(span)
        })
        .collect();
    SceneNode {
        kind: SceneKind::Grid {
            cols,
            row_h: SceneDim::fixed(20.0),
            gap,
        },
        size: SceneSize::fixed(500.0, 60.0),
        children,
        ..SceneNode::default()
    }
}

fn assert_parity(scene: &SceneNode) {
    let flex = FlexLayoutEngine.lay_out_scene(SLOT, scene);
    let taffy = TaffyBackend.lay_out_scene(SLOT, scene);
    assert_eq!(
        flex.len(),
        taffy.len(),
        "разное число rect'ов (DFS pre-order)"
    );
    for (i, (f, t)) in flex.iter().zip(taffy.iter()).enumerate() {
        assert_eq!(f, t, "rect {i} расходится: flex={f:?} taffy={t:?}");
    }
}

/// Auto-трек по контенту + Length + Fill; зазоры; definite-ячейки.
#[test]
fn parity_auto_length_fill() {
    let scene = grid(
        vec![SceneTrack::Auto, SceneTrack::Length(60.0), SceneTrack::Fill],
        &[(120.0, 20.0, 1), (20.0, 20.0, 1), (300.0, 20.0, 1)],
        UiVec2::new(10.0, 8.0),
    );
    assert_parity(&scene);
}

/// Два Auto-трека: растяжение остатком поровну (§11.8), авто-ячейки.
#[test]
fn parity_two_auto_tracks_stretch() {
    let scene = grid(
        vec![SceneTrack::Auto, SceneTrack::Auto],
        &[(60.0, 20.0, 1), (0.0, 0.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    assert_parity(&scene);
}

/// Auto-треки с разными контентами и авто-ячейками (растяжение).
#[test]
fn parity_auto_with_span() {
    let scene = grid(
        vec![SceneTrack::Auto, SceneTrack::Auto, SceneTrack::Fill],
        &[
            (80.0, 20.0, 1),
            (140.0, 20.0, 1),
            (0.0, 0.0, 1),
            (50.0, 20.0, 2),
            (0.0, 0.0, 1),
        ],
        UiVec2::new(12.0, 6.0),
    );
    assert_parity(&scene);
}

/// minmax(definite, definite): рост в лимит при свободном месте.
#[test]
fn parity_minmax_definite() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(100.0), TrackMax::Length(300.0)),
            SceneTrack::fixed(100.0),
        ],
        &[(50.0, 20.0, 1), (20.0, 20.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    assert_parity(&scene);
}

/// minmax(definite, definite) с перетяжкой: лимиты достигаются неравномерно.
#[test]
fn parity_minmax_freeze_order() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(50.0), TrackMax::Length(120.0)),
            SceneTrack::minmax(TrackMin::Length(50.0), TrackMax::Length(400.0)),
            SceneTrack::fixed(40.0),
        ],
        &[(30.0, 20.0, 1), (30.0, 20.0, 1), (20.0, 20.0, 1)],
        UiVec2::new(10.0, 0.0),
    );
    assert_parity(&scene);
}

/// minmax(Auto, definite) ×2: база = контент, рост до max.
#[test]
fn parity_minmax_auto_min() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Auto, TrackMax::Length(200.0)),
            SceneTrack::minmax(TrackMin::Auto, TrackMax::Length(100.0)),
        ],
        &[(120.0, 20.0, 1), (40.0, 20.0, 1)],
        UiVec2::new(8.0, 0.0),
    );
    assert_parity(&scene);
}

/// minmax(min, Fill): fr с полом; пол перераспределяет остаток.
#[test]
fn parity_minmax_fill_floor() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(260.0), TrackMax::Fill),
            SceneTrack::Fill,
        ],
        &[(0.0, 0.0, 1), (0.0, 0.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    assert_parity(&scene);
}

/// Auto + minmax(definite) + Fill + minmax(Auto, Fill) — полный микс.
#[test]
fn parity_mixed_tracks() {
    let scene = grid(
        vec![
            SceneTrack::Auto,
            SceneTrack::minmax(TrackMin::Length(80.0), TrackMax::Length(180.0)),
            SceneTrack::Fill,
            SceneTrack::minmax(TrackMin::Auto, TrackMax::Fill),
        ],
        &[
            (90.0, 20.0, 1),
            (0.0, 0.0, 1),
            (0.0, 0.0, 1),
            (110.0, 20.0, 1),
            (70.0, 20.0, 1),
        ],
        UiVec2::new(6.0, 10.0),
    );
    assert_parity(&scene);
}

/// Процентные min/max в minmax.
#[test]
fn parity_minmax_percent() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Percent(0.2), TrackMax::Percent(0.4)),
            SceneTrack::Fill,
        ],
        &[(0.0, 0.0, 1), (0.0, 0.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    assert_parity(&scene);
}

/// Переполнение от полов minmax (мин шире контейнера).
#[test]
fn parity_minmax_overflow() {
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(300.0), TrackMax::Length(400.0)),
            SceneTrack::minmax(TrackMin::Length(300.0), TrackMax::Length(400.0)),
        ],
        &[(20.0, 20.0, 1), (20.0, 20.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    assert_parity(&scene);
}
