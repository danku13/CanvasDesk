//! FR-074: sticky-позиционирование по ОБОИМ осям в [`FlexLayoutEngine`].
//!
//! Контракт (сцена, `layout::scene`): `ScenePosition::Sticky { top, left }`
//! — в потоке; при прокрутке предка со `offset` прилипает:
//! - вертикаль (`top: Some`) — против ближайшего Y-scroll-предка
//!   (Column/Grid): `y = max(flow_y − offset, container_y + top)`;
//! - горизонталь (`left: Some`) — против ближайшего X-scroll-предка
//!   (Row): `x = max(flow_x − offset, container_x + left)`;
//! - кламп транслирует всё поддерево sticky-узла (fixed-потомки —
//!   viewport-контекст, не двигаются);
//! - оси независимы: `None` — нет прилипания по оси.

use canvas_ui::layout::{FlexLayoutEngine, SceneNode, ScenePosition};
use canvas_ui::UiRect;

const SLOT: UiRect = UiRect::new(0.0, 0.0, 300.0, 200.0);

/// Горизонтальный sticky: Row-scroll-предок, кламп к container_x + left,
/// поток уезжает влево, sticky остаётся.
#[test]
fn horizontal_sticky_clamps_left() {
    let scene = SceneNode::row(
        300.0,
        80.0,
        0.0,
        vec![
            SceneNode::leaf(100.0, 40.0).at(ScenePosition::Sticky {
                top: None,
                left: Some(0.0),
            }),
            SceneNode::leaf(100.0, 40.0),
            SceneNode::leaf(100.0, 40.0),
        ],
    )
    .scrolled(50.0);
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // shift = −50: sticky: flow 0 − 50 = −50 → кламп к 0 + 0 = 0.
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 100.0, 40.0));
    // остальные уезжают: 100−50 = 50; 200−50 = 150.
    assert_eq!(rects[2], UiRect::new(50.0, 0.0, 100.0, 40.0));
    assert_eq!(rects[3], UiRect::new(150.0, 0.0, 100.0, 40.0));
}

/// Горизонтальный sticky с отступом left > 0 и поддеревом (кламп
/// транслирует потомков).
#[test]
fn horizontal_sticky_offset_and_subtree() {
    let header = SceneNode::row(
        100.0,
        40.0,
        8.0,
        vec![SceneNode::leaf(30.0, 40.0), SceneNode::leaf(30.0, 40.0)],
    )
    .at(ScenePosition::Sticky {
        top: None,
        left: Some(12.0),
    });
    let scene =
        SceneNode::row(300.0, 80.0, 0.0, vec![header, SceneNode::leaf(100.0, 40.0)]).scrolled(50.0);
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // шапка: 0 − 50 = −50 → кламп к 0 + 12 = 12 (dx = 62).
    assert_eq!(rects[1], UiRect::new(12.0, 0.0, 100.0, 40.0));
    // дети шапки двигаются с ней: flow 0/38 − 50 → −50/−12 → +62 = 12/50.
    assert_eq!(rects[2], UiRect::new(12.0, 0.0, 30.0, 40.0));
    assert_eq!(rects[3], UiRect::new(50.0, 0.0, 30.0, 40.0));
    // второй лист ряда: 100 − 50 = 50.
    assert_eq!(rects[4], UiRect::new(50.0, 0.0, 100.0, 40.0));
}

/// Оси независимы: вертикальный sticky в Column не реагирует на left,
/// горизонтальный — на top.
#[test]
fn sticky_axes_independent() {
    let scene = SceneNode::column(
        200.0,
        120.0,
        0.0,
        vec![
            SceneNode::leaf(200.0, 40.0).at(ScenePosition::Sticky {
                top: Some(0.0),
                left: Some(0.0),
            }),
            SceneNode::leaf(200.0, 40.0),
        ],
    )
    .scrolled(20.0);
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // Вертикаль: 0 − 20 = −20 → кламп к 0. Горизонталь: нет X-предка.
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 200.0, 40.0));
    assert_eq!(rects[2], UiRect::new(0.0, 20.0, 200.0, 40.0));
}

/// Ось-зависимость поиска scroll-предка: вертикальный sticky внутри
/// Row-with-offset внутри Column-with-offset прилипает к КОЛОНКЕ
/// (Row с offset — не Y-предок).
#[test]
fn vertical_sticky_skips_row_scroll_ancestor() {
    let inner = SceneNode::row(
        200.0,
        40.0,
        0.0,
        vec![SceneNode::leaf(200.0, 40.0).at(ScenePosition::Sticky {
            top: Some(0.0),
            left: None,
        })],
    )
    .scrolled(100.0);
    let scene = SceneNode::column(200.0, 120.0, 0.0, vec![inner, SceneNode::leaf(200.0, 40.0)])
        .scrolled(20.0);
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // Content-shift: Row-offset 100 по X (x не меняет — под ним нет
    // контента по X), Column-offset 20 по Y.
    // Sticky-лист: flow y = 0; shift Y = 20 → y = −20 → кламп к
    // container_y(колонки) + 0 = 0.
    assert_eq!(
        rects[2].y, 0.0,
        "vertical sticky прилип к колонке, а не к Row"
    );
}
