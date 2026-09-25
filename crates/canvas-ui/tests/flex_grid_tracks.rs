//! FR-074: Grid-треки `Auto` и `minmax()` в [`FlexLayoutEngine`] —
//! контрактные тесты авторасчёта колонок (CSS Grid §11.5–11.8 в
//! упрощении движка; см. доку `layout/flex.rs`).
//!
//! Детерминированно: фиксированные размеры ячеек (Leaf + Length),
//! вьюпорт-слот задан явно, без текст-замера (шрифто-независимость).
//! Плейсмент row-major со спанами — единый оракул `grid_placements`.
//!
//! Семантика (сводка):
//! - `Auto` — base/limit = max-content span-1 ячеек трека; остаток
//!   контейнера растягивает Auto-треки поровну (§11.8, taffy/browser
//!   default STRETCH);
//! - `minmax(min, max)` — base = min (definite/контент), лимит = max;
//!   §11.6 дорастает трек в лимит при свободном месте; `max: Fill` —
//!   fr-трек с полом min (§11.7 find_size_of_fr c базами);
//! - ячейка: definite-размер — собственный (не растягивается, CSS
//!   grid-item), Fill/Auto — stretch на область трека/строки.
//!
//! Индексация дампа: rects[0] — корень-сетка, rects[1..] — ячейки в
//! порядке flow (контракт `lay_out_scene` — DFS pre-order).

use canvas_ui::layout::{
    FlexLayoutEngine, SceneDim, SceneKind, SceneNode, SceneSize, SceneTrack, TrackMax, TrackMin,
};
use canvas_ui::{UiRect, UiVec2};

const SLOT: UiRect = UiRect::new(0.0, 0.0, 500.0, 300.0);

/// Grid фиксированного размера: треки + ячейки (размер, спан).
fn grid(cols: Vec<SceneTrack>, cells: &[(f32, f32, u16)], gap: UiVec2) -> SceneNode {
    let children: Vec<SceneNode> = cells
        .iter()
        .map(|&(w, h, span)| SceneNode::leaf(w, h).spanning(span))
        .collect();
    SceneNode {
        kind: SceneKind::Grid {
            cols,
            row_h: SceneDim::fixed(20.0),
            gap,
        },
        size: SceneSize::fixed(500.0, 40.0),
        children,
        ..SceneNode::default()
    }
}

/// Авто-ячейка (без размера) — тянется в область трека (stretch).
fn area_cell() -> SceneNode {
    SceneNode::default()
}

/// Auto-трек: base = контент span-1 ячейки; свободное место уходит
/// fr-треку (§11.7) — Auto остаётся на контенте.
#[test]
fn auto_track_sizes_to_content() {
    // 500 = 120(Auto) + 10 + 60(Length) + 10 + 300(1fr).
    let scene = grid(
        vec![SceneTrack::Auto, SceneTrack::Length(60.0), SceneTrack::Fill],
        &[(120.0, 20.0, 1), (20.0, 20.0, 1), (300.0, 20.0, 1)],
        UiVec2::new(10.0, 0.0),
    );
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // Позиции ячеек = позиции треков; definite-ячейки не растягиваются.
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 120.0, 20.0));
    assert_eq!(rects[2], UiRect::new(130.0, 0.0, 20.0, 20.0)); // x = 130 → Auto = 120
    assert_eq!(rects[3], UiRect::new(200.0, 0.0, 300.0, 20.0)); // x = 200 → Length = 60; Fill = 300
}

/// Auto-треки растягиваются на остаток контейнера поровну (CSS §11.8,
/// default STRETCH — как в браузерах и taffy): авто-ячейка показывает
/// область растянутого трека.
#[test]
fn auto_tracks_stretch_into_free_space() {
    // Контент 60; остаток 440 → по 220: треки [280, 220].
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Auto, SceneTrack::Auto],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: vec![SceneNode::leaf(60.0, 20.0), area_cell()],
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 60.0, 20.0)); // definite
    assert_eq!(rects[2], UiRect::new(280.0, 0.0, 220.0, 20.0)); // stretch: x=280 → трек1 = 280
}

/// span-2 ячейка занимает оба трека; span-1 второй строки даёт Auto-
/// треку контент 100; fr-сосед забирает остаток (§11.7); авто-ячейка
/// демонстрирует площадь спана.
#[test]
fn spanning_cell_does_not_feed_auto_content() {
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Auto, SceneTrack::Fill],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 40.0),
        children: vec![area_cell().spanning(2), SceneNode::leaf(100.0, 20.0)],
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // [1] авто-ячейка span-2 → вся строка 0 (100+400); [2] definite 100
    // в Auto-треке (строка 1): Auto = 100, fr = 400.
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 500.0, 20.0));
    assert_eq!(rects[2], UiRect::new(0.0, 20.0, 100.0, 20.0));
}

/// minmax(definite, definite): база = min; свободное место дорастает
/// трек в лимит (§11.6), остаток после лимита никому не достаётся.
#[test]
fn minmax_definite_grows_to_max() {
    // [minmax(100, 300), Length(100)]: свободно 500−200 = 300 → minmax
    // дорастает до лимита 300; остаток 100 не занят (нет fr/Auto-max).
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(100.0), TrackMax::Length(300.0)),
            SceneTrack::fixed(100.0),
        ],
        &[(50.0, 20.0, 1), (20.0, 20.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 50.0, 20.0)); // definite
    assert_eq!(rects[2], UiRect::new(300.0, 0.0, 20.0, 20.0)); // x=300 → minmax = 300
}

/// minmax(definite, definite): два трека делят свободное место поровну,
/// лимиты здесь не достигаются (§11.6).
#[test]
fn minmax_definite_splits_free_space() {
    // 500 − 20 (Length) = 480; два minmax(100, 300): по 240 каждому.
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Length(100.0), TrackMax::Length(300.0)),
            SceneTrack::fixed(20.0),
            SceneTrack::minmax(TrackMin::Length(100.0), TrackMax::Length(300.0)),
        ],
        &[(50.0, 20.0, 1), (20.0, 20.0, 1), (50.0, 20.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 50.0, 20.0));
    assert_eq!(rects[2], UiRect::new(240.0, 0.0, 20.0, 20.0)); // x=240 → трек1 = 240
    assert_eq!(rects[3], UiRect::new(260.0, 0.0, 50.0, 20.0)); // x=260 → трек2 = 240
}

/// minmax(Auto, definite): база = контент; при свободном месте — рост
/// до max (кламп лимитом); остаток не распределяется.
#[test]
fn minmax_auto_min_uses_content() {
    // [minmax(Auto, 200), Length(282)]: контент 120 → свободно 98 → рост
    // до лимита 200; остаток 18 не занят.
    let scene = grid(
        vec![
            SceneTrack::minmax(TrackMin::Auto, TrackMax::Length(200.0)),
            SceneTrack::fixed(282.0),
        ],
        &[(120.0, 20.0, 1), (20.0, 20.0, 1)],
        UiVec2::new(0.0, 0.0),
    );
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 120.0, 20.0));
    assert_eq!(rects[2], UiRect::new(200.0, 0.0, 20.0, 20.0)); // x=200 → minmax = 200
}

/// minmax(min, Fill) — fr-трек с полом min (§11.7); авто-ячейки
/// показывают области треков.
#[test]
fn minmax_fill_floor_wins() {
    // A: [Fill, minmax(200, Fill)]: свободно 500 → по 250 ≥ пола.
    let scene_a = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![
                SceneTrack::Fill,
                SceneTrack::minmax(TrackMin::Length(200.0), TrackMax::Fill),
            ],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: vec![area_cell(), area_cell()],
        ..SceneNode::default()
    };
    let a = FlexLayoutEngine.lay_out_scene(SLOT, &scene_a);
    assert_eq!(a[1], UiRect::new(0.0, 0.0, 250.0, 20.0));
    assert_eq!(a[2], UiRect::new(250.0, 0.0, 250.0, 20.0));

    // B: [Length(400), minmax(200, Fill)]: fr = 100 < пола 200 → трек 200
    // (переполнение видно — G4).
    let scene_b = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![
                SceneTrack::fixed(400.0),
                SceneTrack::minmax(TrackMin::Length(200.0), TrackMax::Fill),
            ],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: vec![area_cell(), area_cell()],
        ..SceneNode::default()
    };
    let b = FlexLayoutEngine.lay_out_scene(SLOT, &scene_b);
    assert_eq!(b[1], UiRect::new(0.0, 0.0, 400.0, 20.0));
    assert_eq!(b[2], UiRect::new(400.0, 0.0, 200.0, 20.0));
}

/// minmax(min, Fill): пол floored-трека перераспределяет остаток
/// соседнему fr-треку (§11.7.1: floored → «inflexible», fr пересчёт).
#[test]
fn minmax_fill_floor_redistributes() {
    // [minmax(260, Fill), Fill]: fr₁ = 250 < 260 → floored 260;
    // fr₂ = (500−260)/1 = 240.
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![
                SceneTrack::minmax(TrackMin::Length(260.0), TrackMax::Fill),
                SceneTrack::Fill,
            ],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: vec![area_cell(), area_cell()],
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 260.0, 20.0));
    assert_eq!(rects[2], UiRect::new(260.0, 0.0, 240.0, 20.0));
}

/// Контент Auto-трека — только span-1 ячейки; §11.8 дорастает Auto до
/// остатка; definite-ячейка не растягивается своей спан-областью.
#[test]
fn auto_track_ignores_spanning_cells_for_content() {
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![SceneTrack::Auto, SceneTrack::fixed(60.0)],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 40.0),
        children: vec![area_cell().spanning(2), SceneNode::leaf(80.0, 20.0)],
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    // Плейсмент: span-2 занимает строку 0 целиком → span-1 переносится
    // на строку 1 в Auto-трек. Контент Auto = 80 (span-1); §11.8: 80+360
    // = 440. [1] авто-ячейка span-2 → 440+60 = 500; [2] definite 80.
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 500.0, 20.0));
    assert_eq!(rects[2], UiRect::new(0.0, 20.0, 80.0, 20.0));
}

/// Процентный min в minmax — от внутреннего размера контейнера.
#[test]
fn minmax_percent_min() {
    // [minmax(20%, Fill), Fill]: min = 100; fr = 250 ≥ 100.
    let scene = SceneNode {
        kind: SceneKind::Grid {
            cols: vec![
                SceneTrack::minmax(TrackMin::Percent(0.2), TrackMax::Fill),
                SceneTrack::Fill,
            ],
            row_h: SceneDim::fixed(20.0),
            gap: UiVec2::new(0.0, 0.0),
        },
        size: SceneSize::fixed(500.0, 20.0),
        children: vec![area_cell(), area_cell()],
        ..SceneNode::default()
    };
    let rects = FlexLayoutEngine.lay_out_scene(SLOT, &scene);
    assert_eq!(rects[1], UiRect::new(0.0, 0.0, 250.0, 20.0));
    assert_eq!(rects[2], UiRect::new(250.0, 0.0, 250.0, 20.0));
}

/// Построитель minmax нормализует отрицательные значения.
#[test]
fn minmax_constructor_normalizes() {
    let t = SceneTrack::minmax(TrackMin::Length(-5.0), TrackMax::Length(100.0));
    assert_eq!(
        t,
        SceneTrack::MinMax {
            min: TrackMin::Length(0.0),
            max: TrackMax::Length(100.0)
        }
    );
}
