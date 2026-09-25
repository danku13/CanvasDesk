//! `TaffyBackend` (FR-068 W1, ADR-0014 §Решение п.3): opt-in движок вёрстки
//! на taffy 0.14 за фичей `taffy` (default off — zero-dep инвариант G7,
//! §Контракт-2 FR-068). Компилируется ТОЛЬКО при `#[cfg(feature = "taffy")]`.
//!
//! # Immediate-mode (D2/ADR-0014, C2/ADR-0015)
//!
//! [`TaffyBackend`] — ZST без состояния: каждый вызов `lay_out_*` строит
//! ОДНОРАЗОВОЕ `TaffyTree` (создание → `compute_layout` → чтение rect'ов →
//! сброс). Осознанное отклонение от буквы ADR-0014 (`struct TaffyBackend
//! { tree: TaffyTree }`): retained-поле в W1 мертво (дерево всё равно
//! пересобирается на кадр — «пересоздаёт TaffyTree на кадр», §Решение п.3
//! ADR-0014), поэтому оно появится в W3 вместе с retained node-id кэшем
//! (FR-068, «если профиль W2 покажет > 1 мс»). Отсутствие состояния даёт
//! `Send + Sync` бесплатно и статический [`super::pilot_backend`].
//!
//! # Адаптер (ADR-0014 §Решение п.3)
//!
//! - `Row{gap, main, cross, policy}` → `Style` (flex row): `Fit` — дети
//!   `flex_shrink: 0` (Native не сжимает — переполнение видно, G4),
//!   `flex_grow` = `Child.grow`, `flex_basis` = базовая ширина;
//!   `SqueezeTail` — `flex_shrink: 1.0, min_size: 0` (документированное
//!   расхождение C3/§Контракт-4: CSS flex_shrink распределяет сжатие по
//!   ВСЕМ детям пропорционально, Native — сжимает только хвост дословно);
//!   `Wrap` — `flex_wrap: Wrap` (F-15; семантика совпадает).
//! - `Column` — симметрично (flex column).
//! - `MeasuredItem` — resolve через [`TextMeasurer`] ДО адаптера (тот же
//!   шейпинг, что у NativeBackend) → фиксированные размеры (бит-в-бит
//!   паритет по построению; measure-колбэки taffy не нужны).
//! - `grid_cells` — `display: Grid` с ЯВНЫМИ треками-Points (в т.ч.
//!   неравными — T2-триггер ADR-0013).
//!
//! # Документированные расхождения Native ↔ taffy
//!
//! - **C3 `SqueezeTail` ≠ `flex_shrink`** (см. выше; §Контракт-4 — «не
//!   fail в parity test»).
//! - **Переполнение + `MainAlign::End`**: Native прижимает детей к краю
//!   с клампом начала (`x = max(slot.x, …)`) — переполнение уходит к
//!   концу оси; CSS flex-end при переполнении выталкивает детей ЗА
//!   НАЧАЛЬНЫЙ край (unsafe alignment). Parity — только без переполнения.
//! - **position: sticky** в taffy 0.14 отсутствует — [`ScenePosition::
//!   Sticky`] эмулируется post-processing'ом ([`TaffyBackend::lay_out_scene`]
//!   клампит `y = max(flow_y − offset, container_y + top)`).
//! - **scroll/offset** — taffy не сдвигает контент сам: content-shift
//!   [`SceneNode::offset`] применён post-processing'ом (семантика отрисовки
//!   прокрученного контента).

use taffy::geometry::Point;
use taffy::prelude::*;
use taffy::style::Overflow;

use super::{
    Child, Column, CrossAlign, LayoutBackend, LayoutFeatures, MainAlign, MeasuredItem, Row,
    RowPolicy, SceneDim, SceneKind, SceneNode, SceneOverflow, ScenePosition, SceneTrack, UiRect,
    UiVec2,
};
use crate::measure::TextMeasurer;

/// Opt-in backend вёрстки на taffy (FR-068 W1; ADR-0014 §Решение п.3).
/// ZST — immediate-mode, состояние отсутствует (см. модульную доку).
#[derive(Debug, Clone, Copy, Default)]
pub struct TaffyBackend;

impl LayoutBackend for TaffyBackend {
    fn features(&self) -> LayoutFeatures {
        // Полный flexbox + grid + percent/aspect/overflow; sticky — эмуляция
        // в lay_out_scene (документировано в доке бита STICKY и модуле).
        LayoutFeatures::FLEX_GROW
            .union(LayoutFeatures::FLEX_SHRINK)
            .union(LayoutFeatures::FLEX_BASIS)
            .union(LayoutFeatures::FLEX_WRAP)
            .union(LayoutFeatures::GRID_2D)
            .union(LayoutFeatures::AUTO_SIZE)
            .union(LayoutFeatures::OVERFLOW_CLIP)
            .union(LayoutFeatures::PERCENT)
            .union(LayoutFeatures::ASPECT_RATIO)
            .union(LayoutFeatures::STICKY)
    }

    fn lay_out_row(&self, row: Row, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let mut tree = TaffyTree::<()>::new();
        let root = build_flex(
            &mut tree,
            FlexDirection::Row,
            &row_inputs(&row),
            slot,
            items,
        );
        read_children(&mut tree, root, slot)
    }

    fn lay_out_column(&self, column: Column, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let mut tree = TaffyTree::<()>::new();
        let root = build_flex(
            &mut tree,
            FlexDirection::Column,
            &column_inputs(&column),
            slot,
            items,
        );
        read_children(&mut tree, root, slot)
    }

    fn lay_out_measured(
        &self,
        row: Row,
        slot: UiRect,
        items: &[MeasuredItem],
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        family: &str,
        size: f32,
    ) -> Vec<UiRect> {
        // Тот же resolve, что у NativeBackend — шейпинг идентичен бит-в-бит
        // (единая точка замера `MeasuredItem::resolve`); taffy получает уже
        // фиксированные размеры.
        let children: Vec<Child> = items
            .iter()
            .map(|item| item.resolve(m, fs, family, size))
            .collect();
        self.lay_out_row(row, slot, &children)
    }

    fn lay_out_grid(
        &self,
        slot: UiRect,
        cols: &[f32],
        rows: usize,
        row_h: f32,
        gap: UiVec2,
    ) -> Vec<UiRect> {
        let mut tree = TaffyTree::<()>::new();
        // Явные треки-Points (в т.ч. неравные — T2-триггер ADR-0013).
        let root = tree
            .new_leaf(Style {
                display: Display::Grid,
                grid_template_columns: cols.iter().map(|&w| length(w.max(0.0))).collect(),
                grid_template_rows: (0..rows).map(|_| length(row_h.max(0.0))).collect(),
                gap: Size {
                    width: length(gap.x.max(0.0)),
                    height: length(gap.y.max(0.0)),
                },
                size: Size {
                    width: length(slot.w),
                    height: length(slot.h),
                },
                min_size: Size::zero(),
                ..Default::default()
            })
            .expect("taffy: корневой узел сетки");
        // Ячейки: cols·rows листов, авто-поток row-major (как grid_cells).
        let n = cols.len().saturating_mul(rows);
        let cells: Vec<NodeId> = (0..n)
            .map(|_| {
                tree.new_leaf(Style {
                    min_size: Size::zero(),
                    ..Default::default()
                })
                .expect("taffy: ячейка сетки")
            })
            .collect();
        if n > 0 {
            tree.set_children(root, &cells).expect("taffy: дети сетки");
        }
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(slot.w),
                height: AvailableSpace::Definite(slot.h),
            },
        )
        .expect("taffy: compute_layout сетки");
        cells
            .iter()
            .map(|&id| {
                let l = tree.layout(id).expect("taffy: layout ячейки");
                UiRect::new(
                    slot.x + l.location.x,
                    slot.y + l.location.y,
                    l.size.width,
                    l.size.height,
                )
            })
            .collect()
    }
}

impl TaffyBackend {
    /// Раскладка расширенной сцены ([`SceneNode`], FR-068 W1): percent/
    /// aspect-ratio/position absolute|fixed|sticky/overflow/scroll —
    /// возможности за пределами V-5 примитивов. Контракт — модульная дока
    /// `scene`; post-processing (content-shift, sticky-кламп) — здесь.
    ///
    /// Возвращает rect'ы ВСЕХ узлов в DFS pre-order
    /// ([`SceneNode::walk_preorder`] — тот же порядок; `[0]` — корень).
    pub fn lay_out_scene(&self, slot: UiRect, scene: &SceneNode) -> Vec<UiRect> {
        let mut tree = TaffyTree::<()>::new();
        // Корень — definite размер слота; fixed-дети вешаются на корень
        // (viewport-семантика).
        let root = tree
            .new_leaf(scene_style(scene, ParentCtx::Root))
            .expect("taffy: корень сцены");
        let mut b = SceneBuilder {
            tree: &mut tree,
            order: Vec::new(),
            fixed_ids: Vec::new(),
        };
        b.order.push(SceneEntry {
            id: root,
            parent: None,
            position: ScenePosition::InFlow,
            offset: scene.offset,
            axis: MainAxis::Y,
        });
        b.build_children(root, scene, 0);
        // Разобрать builder (заканчиваем mutable borrow дерева) и прикрепить
        // fixed-детей к корню (viewport-семантика).
        let SceneBuilder {
            order, fixed_ids, ..
        } = b;
        if !fixed_ids.is_empty() {
            let mut cur = tree.children(root).expect("taffy: дети корня");
            cur.extend(fixed_ids.iter().copied());
            tree.set_children(root, &cur).expect("taffy: fixed-overlay");
        }

        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(slot.w),
                height: AvailableSpace::Definite(slot.h),
            },
        )
        .expect("taffy: compute_layout сцены");

        let origin = UiVec2::new(slot.x, slot.y);
        // Чтение: location — относительно taffy-родителя → абсолютные
        // координаты накоплением по DFS pre-order (родитель раньше детей).
        let mut rects = vec![UiRect::default(); order.len()];
        for (i, e) in order.iter().enumerate() {
            let l = tree.layout(e.id).expect("taffy: layout узла сцены");
            let (x, y) = match e.parent {
                None => (origin.x + l.location.x, origin.y + l.location.y),
                Some(p) => (rects[p].x + l.location.x, rects[p].y + l.location.y),
            };
            rects[i] = UiRect::new(x, y, l.size.width, l.size.height);
        }
        // Content-shift: сумма offset'ов scroll-предков по их главной оси
        // (fixed-узлы прицеплены к корню — shift'а не получают, как CSS
        // position:fixed внутри scroll-контейнера).
        for i in 1..order.len() {
            // Сам fixed-узел не сдвигается (его taffy-локация — уже
            // viewport-координаты через прикрепление к корню).
            if matches!(order[i].position, ScenePosition::Fixed { .. }) {
                continue;
            }
            let (sx, sy) = shift_for(&order, i);
            if sx != 0.0 || sy != 0.0 {
                rects[i].x -= sx;
                rects[i].y -= sy;
            }
        }
        // Sticky-кламп по ближайшему scroll-предку (после shift'ов: кламп
        // считается от отображаемой позиции контейнера).
        for i in 1..order.len() {
            if let ScenePosition::Sticky { top } = order[i].position {
                if let Some(ai) = scroll_ancestor(&order, i) {
                    let off = order[ai].offset;
                    let target = rects[ai].y + top;
                    if off > 0.0 && rects[i].y < target {
                        rects[i].y = target;
                    }
                }
            }
        }
        rects
    }

    /// Центрирование блока фиксированного размера в слоте через taffy
    /// (pilot explain-modal, FR-068 W1): CSS-семантика justify/align
    /// Center. Паритет с `stack(slot, size, Center, Center)` при
    /// помещении в слот; при переполнении CSS центрирует с выходом за
    /// ОБА края (Native клампит левый/верхний — документированное
    /// расхождение; у pilot-поверхностей размер ограничен constrain).
    pub fn centered(&self, slot: UiRect, size: UiVec2) -> UiRect {
        let mut tree = TaffyTree::<()>::new();
        let root = tree
            .new_leaf(Style {
                display: Display::Flex,
                justify_content: Some(JustifyContent::CENTER),
                align_items: Some(AlignItems::CENTER),
                size: Size {
                    width: length(slot.w),
                    height: length(slot.h),
                },
                min_size: Size::zero(),
                ..Default::default()
            })
            .expect("taffy: корень центрирования");
        let child = tree
            .new_leaf(Style {
                size: Size {
                    width: length(size.x),
                    height: length(size.y),
                },
                min_size: Size::zero(),
                ..Default::default()
            })
            .expect("taffy: ребёнок центрирования");
        tree.set_children(root, &[child])
            .expect("taffy: set_children");
        tree.compute_layout(
            root,
            Size {
                width: AvailableSpace::Definite(slot.w),
                height: AvailableSpace::Definite(slot.h),
            },
        )
        .expect("taffy: compute_layout центрирования");
        let l = tree.layout(child).expect("taffy: layout ребёнка");
        UiRect::new(
            slot.x + l.location.x,
            slot.y + l.location.y,
            l.size.width,
            l.size.height,
        )
    }
}

// --- Flex-адаптер (Row/Column → Style) --------------------------------------

/// Параметры flex-линии (общие для Row/Column; различие — направление).
struct FlexInputs {
    gap: f32,
    main: MainAlign,
    cross: CrossAlign,
    wrap: bool,
    squeeze_tail: bool,
}

fn row_inputs(row: &Row) -> FlexInputs {
    FlexInputs {
        gap: row.gap,
        main: row.main,
        cross: row.cross,
        wrap: row.policy == RowPolicy::Wrap,
        squeeze_tail: row.policy == RowPolicy::SqueezeTail,
    }
}

fn column_inputs(column: &Column) -> FlexInputs {
    FlexInputs {
        gap: column.gap,
        main: column.main,
        cross: column.cross,
        wrap: false,
        squeeze_tail: false,
    }
}

fn build_flex(
    tree: &mut TaffyTree<()>,
    dir: FlexDirection,
    input: &FlexInputs,
    slot: UiRect,
    items: &[Child],
) -> NodeId {
    let is_row = dir == FlexDirection::Row;
    // gap: по главной оси — gap; поперечный (межстрочный при wrap) — gap
    // (Native wrap использует тот же gap между строками).
    let (gap_x, gap_y) = if is_row {
        (input.gap, if input.wrap { input.gap } else { 0.0 })
    } else {
        (0.0, input.gap)
    };
    let root = tree
        .new_leaf(Style {
            display: Display::Flex,
            flex_direction: dir,
            flex_wrap: if input.wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            },
            justify_content: Some(match input.main {
                MainAlign::Start => JustifyContent::FLEX_START,
                MainAlign::SpaceBetween => JustifyContent::SPACE_BETWEEN,
                MainAlign::End => JustifyContent::FLEX_END,
            }),
            align_items: Some(match input.cross {
                CrossAlign::Start => AlignItems::FLEX_START,
                CrossAlign::Center => AlignItems::CENTER,
                CrossAlign::End => AlignItems::FLEX_END,
            }),
            align_content: Some(AlignContent::FLEX_START), // строки сверху (native wrap)
            gap: Size {
                width: length(gap_x.max(0.0)),
                height: length(gap_y.max(0.0)),
            },
            size: Size {
                width: length(slot.w),
                height: length(slot.h),
            },
            min_size: Size::zero(),
            ..Default::default()
        })
        .expect("taffy: корень flex");
    let children: Vec<NodeId> = items
        .iter()
        .map(|c| {
            // Fit/Wrap: shrink 0 (переполнение видно — G4), grow = c.grow,
            // basis = главный размер. SqueezeTail: shrink 1, min 0 (C3).
            let (grow, shrink) = if input.squeeze_tail {
                (0.0, 1.0)
            } else {
                (c.grow, 0.0)
            };
            let (main_len, cross_len) = if is_row { (c.w, c.h) } else { (c.h, c.w) };
            let (size_w, size_h) = if is_row {
                (auto(), length(cross_len))
            } else {
                (length(cross_len), auto())
            };
            tree.new_leaf(Style {
                size: Size {
                    width: size_w,
                    height: size_h,
                },
                flex_grow: grow,
                flex_shrink: shrink,
                flex_basis: length(main_len),
                min_size: Size::zero(),
                ..Default::default()
            })
            .expect("taffy: flex-ребёнок")
        })
        .collect();
    if !children.is_empty() {
        tree.set_children(root, &children)
            .expect("taffy: set_children");
    }
    root
}

fn read_children(tree: &mut TaffyTree<()>, root: NodeId, slot: UiRect) -> Vec<UiRect> {
    tree.compute_layout(
        root,
        Size {
            width: AvailableSpace::Definite(slot.w),
            height: AvailableSpace::Definite(slot.h),
        },
    )
    .expect("taffy: compute_layout");
    let children = tree.children(root).expect("taffy: children");
    children
        .into_iter()
        .map(|id| {
            let l = tree.layout(id).expect("taffy: layout ребёнка");
            UiRect::new(
                slot.x + l.location.x,
                slot.y + l.location.y,
                l.size.width,
                l.size.height,
            )
        })
        .collect()
}

// --- Сцена (SceneNode → taffy) ----------------------------------------------

/// Главная ось узла-контейнера (направление content-shift при прокрутке).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainAxis {
    X,
    Y,
}

/// Контекст родителя для стиля узла сцены (чьи flex-свойства применяются).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentCtx {
    /// Корень сцены (definite слот; flex-свойства ребёнка не нужны).
    Root,
    /// Родитель — flex row (главная ось X).
    Row,
    /// Родитель — flex column (главная ось Y).
    Column,
    /// Родитель — grid (flex-свойства игнорируются, действует span).
    Grid,
}

/// Запись узла сцены в DFS pre-order (1:1 с `SceneNode::walk_preorder`).
struct SceneEntry {
    id: NodeId,
    /// Индекс taffy-родителя в `order` (None — корень; fixed-узлы → 0).
    parent: Option<usize>,
    position: ScenePosition,
    /// SceneNode::offset узла (scroll-контейнер; 0 — не контейнер).
    offset: f32,
    /// Главная ось узла (для направления content-shift).
    axis: MainAxis,
}

struct SceneBuilder<'a> {
    tree: &'a mut TaffyTree<()>,
    order: Vec<SceneEntry>,
    /// Taffy-id fixed-узлов (прикрепляются к корню после обхода).
    fixed_ids: Vec<NodeId>,
}

impl<'a> SceneBuilder<'a> {
    /// Построить всех детей узла (в порядке сцены; fixed — мимо taffy-родителя).
    fn build_children(&mut self, taffy_parent: NodeId, node: &SceneNode, parent_idx: usize) {
        if node.children.is_empty() {
            return;
        }
        let ctx = match &node.kind {
            SceneKind::Row { .. } => ParentCtx::Row,
            SceneKind::Column { .. } => ParentCtx::Column,
            SceneKind::Grid { .. } => ParentCtx::Grid,
            SceneKind::Leaf => ParentCtx::Row, // у листа детей нет — недостижимо
        };
        let mut flow_ids: Vec<NodeId> = Vec::with_capacity(node.children.len());
        for child in &node.children {
            match child.position {
                ScenePosition::Fixed { .. } => {
                    // Viewport-семантика: taffy-родитель — КОРЕНЬ; порядок в
                    // order — как в сцене (DFS); прикрепление — после обхода.
                    let idx = self.build_node(child, 0, ParentCtx::Root);
                    self.fixed_ids.push(self.order[idx].id);
                }
                _ => {
                    let idx = self.build_node(child, parent_idx, ctx);
                    flow_ids.push(self.order[idx].id);
                }
            }
        }
        if !flow_ids.is_empty() {
            self.tree
                .set_children(taffy_parent, &flow_ids)
                .expect("taffy: set_children сцены");
        }
    }

    /// Построить узел (+ поддерево), вернуть его индекс в `order`.
    fn build_node(&mut self, node: &SceneNode, parent_idx: usize, ctx: ParentCtx) -> usize {
        let idx = self.order.len();
        let id = self
            .tree
            .new_leaf(scene_style(node, ctx))
            .expect("taffy: узел сцены");
        let position = node.position;
        let axis = match &node.kind {
            SceneKind::Row { .. } => MainAxis::X,
            SceneKind::Column { .. } | SceneKind::Grid { .. } => MainAxis::Y,
            SceneKind::Leaf => MainAxis::Y,
        };
        self.order.push(SceneEntry {
            id,
            parent: Some(parent_idx),
            position,
            offset: node.offset,
            axis,
        });
        self.build_children(id, node, idx);
        idx
    }
}

/// Суммарный content-shift предков (по их главным осям) для узла `i`.
/// Fixed-предок ОБРЫВАЕТ цепочку: его поддерево — viewport-контекст
/// (позиционируется от корня, сдвиги выше не действуют).
fn shift_for(order: &[SceneEntry], i: usize) -> (f32, f32) {
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut p = order[i].parent;
    while let Some(pidx) = p {
        let a = &order[pidx];
        if matches!(a.position, ScenePosition::Fixed { .. }) {
            break; // viewport-контекст начинается здесь
        }
        if a.offset > 0.0 {
            match a.axis {
                MainAxis::X => sx += a.offset,
                MainAxis::Y => sy += a.offset,
            }
        }
        p = a.parent;
    }
    (sx, sy)
}

/// Ближайший scroll-предок — индекс в `order` (для sticky-клампа;
/// отображаемый y контейнера caller берёт из уже сдвинутых rect'ов).
/// Fixed-предок обрывает цепочку (viewport-контекст).
fn scroll_ancestor(order: &[SceneEntry], i: usize) -> Option<usize> {
    let mut p = order[i].parent;
    while let Some(pidx) = p {
        if matches!(order[pidx].position, ScenePosition::Fixed { .. }) {
            return None;
        }
        if order[pidx].offset > 0.0 {
            return Some(pidx);
        }
        p = order[pidx].parent;
    }
    None
}

/// Style узла сцены (контейнерные свойства по [`SceneKind`] + общие
/// размер/позиция/overflow/aspect + flex-свойства от контекста родителя).
fn scene_style(node: &SceneNode, parent: ParentCtx) -> Style {
    let mut style = Style {
        min_size: Size::zero(),
        overflow: match node.overflow {
            SceneOverflow::Visible => Point {
                x: Overflow::Visible,
                y: Overflow::Visible,
            },
            SceneOverflow::Hidden => Point {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            },
        },
        aspect_ratio: node.aspect,
        ..Default::default()
    };
    // Позиция (absolute/fixed — вне потока; inset — от начала координат
    // контейнера; right/bottom auto).
    match node.position {
        ScenePosition::Absolute { x, y } | ScenePosition::Fixed { x, y } => {
            style.position = Position::Absolute;
            style.inset = Rect {
                left: length(x),
                top: length(y),
                right: auto(),
                bottom: auto(),
            };
        }
        ScenePosition::Sticky { .. } | ScenePosition::InFlow => {}
    }
    // Контейнерные свойства.
    match &node.kind {
        SceneKind::Row {
            gap,
            main,
            cross,
            wrap,
        } => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Row;
            style.flex_wrap = if *wrap {
                FlexWrap::Wrap
            } else {
                FlexWrap::NoWrap
            };
            style.justify_content = Some(match main {
                MainAlign::Start => JustifyContent::FLEX_START,
                MainAlign::SpaceBetween => JustifyContent::SPACE_BETWEEN,
                MainAlign::End => JustifyContent::FLEX_END,
            });
            style.align_items = Some(match cross {
                CrossAlign::Start => AlignItems::FLEX_START,
                CrossAlign::Center => AlignItems::CENTER,
                CrossAlign::End => AlignItems::FLEX_END,
            });
            style.align_content = Some(AlignContent::FLEX_START);
            style.gap = Size {
                width: length(gap.max(0.0)),
                height: length(if *wrap { gap.max(0.0) } else { 0.0 }),
            };
        }
        SceneKind::Column { gap, main, cross } => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.justify_content = Some(match main {
                MainAlign::Start => JustifyContent::FLEX_START,
                MainAlign::SpaceBetween => JustifyContent::SPACE_BETWEEN,
                MainAlign::End => JustifyContent::FLEX_END,
            });
            style.align_items = Some(match cross {
                CrossAlign::Start => AlignItems::FLEX_START,
                CrossAlign::Center => AlignItems::CENTER,
                CrossAlign::End => AlignItems::FLEX_END,
            });
            style.gap = Size {
                width: length(0.0),
                height: length(gap.max(0.0)),
            };
        }
        SceneKind::Grid { cols, row_h, gap } => {
            style.display = Display::Grid;
            style.grid_template_columns = cols
                .iter()
                .map(|t| match t {
                    SceneTrack::Length(v) => length(v.max(0.0)),
                    SceneTrack::Percent(p) => percent(p.clamp(0.0, 1.0)),
                    SceneTrack::Fill => {
                        GridTemplateComponent::Single(TrackSizingFunction::from_fr(1.0))
                    }
                    SceneTrack::Auto => GridTemplateComponent::Single(TrackSizingFunction::AUTO),
                })
                .collect();
            // Строки — неявные (auto-flow row) с высотой row_h.
            style.grid_auto_rows = match row_h {
                SceneDim::Length(v) => {
                    vec![TrackSizingFunction::from(Dimension::length(v.max(0.0)))]
                }
                SceneDim::Percent(p) => {
                    vec![TrackSizingFunction::from(Dimension::percent(
                        p.clamp(0.0, 1.0),
                    ))]
                }
                SceneDim::Fill | SceneDim::Auto => vec![TrackSizingFunction::AUTO],
            };
            style.gap = Size {
                width: length(gap.x.max(0.0)),
                height: length(gap.y.max(0.0)),
            };
        }
        SceneKind::Leaf => {}
    }
    // Размер по осям + flex-свойства по контексту родителя.
    // Absolute/Fixed — вне потока: flex-свойства не действуют, размер —
    // напрямую (percent — от содержащего блока родителя).
    let positioned = matches!(
        node.position,
        ScenePosition::Absolute { .. } | ScenePosition::Fixed { .. }
    );
    if positioned {
        style.size = Size {
            width: scene_dim(&node.size.w),
            height: scene_dim(&node.size.h),
        };
        return style;
    }
    let (main_is_x, in_flex) = match parent {
        ParentCtx::Root => (true, false),
        ParentCtx::Row => (true, true),
        ParentCtx::Column => (false, true),
        ParentCtx::Grid => (true, false),
    };
    let (mw, mh) = if main_is_x {
        (node.size.w, node.size.h)
    } else {
        (node.size.h, node.size.w)
    };
    // Главная ось: Fill → grow 1 при basis 0; Length → basis; Percent →
    // size percent; Auto → auto.
    if in_flex {
        match mw {
            SceneDim::Fill => {
                style.flex_grow = 1.0;
                style.flex_shrink = 0.0;
                style.flex_basis = length(0.0);
            }
            SceneDim::Length(v) => {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
                style.flex_basis = length(v.max(0.0));
            }
            SceneDim::Percent(p) => {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
                style.flex_basis = auto();
                if main_is_x {
                    style.size.width = percent(p.clamp(0.0, 1.0));
                } else {
                    style.size.height = percent(p.clamp(0.0, 1.0));
                }
            }
            SceneDim::Auto => {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
                style.flex_basis = auto();
            }
        }
        // Поперечная ось: Fill → stretch (align_self), Length/Percent → size.
        match mh {
            SceneDim::Fill => {
                style.align_self = Some(AlignSelf::STRETCH);
            }
            SceneDim::Length(v) => {
                if main_is_x {
                    style.size.height = length(v.max(0.0));
                } else {
                    style.size.width = length(v.max(0.0));
                }
            }
            SceneDim::Percent(p) => {
                if main_is_x {
                    style.size.height = percent(p.clamp(0.0, 1.0));
                } else {
                    style.size.width = percent(p.clamp(0.0, 1.0));
                }
            }
            SceneDim::Auto => {}
        }
        // Grid-ребёнок: span колонок.
        if matches!(parent, ParentCtx::Grid) && node.span > 1 {
            style.grid_column = Line {
                start: GridPlacement::Auto,
                end: GridPlacement::Span(node.span),
            };
        }
    } else {
        // Вне flex (корень/grid-ребёнок): размер напрямую; Fill → auto
        // (grid-ребёнок растягивается дефолтным stretch в ячейку).
        style.size = Size {
            width: scene_dim(&node.size.w),
            height: scene_dim(&node.size.h),
        };
        if matches!(parent, ParentCtx::Grid) && node.span > 1 {
            style.grid_column = Line {
                start: GridPlacement::Auto,
                end: GridPlacement::Span(node.span),
            };
        }
    }
    style
}

/// [`SceneDim`] → taffy [`Dimension`] (Percent — доля 0..1, как в taffy).
fn scene_dim(d: &SceneDim) -> Dimension {
    match d {
        SceneDim::Length(v) => length(v.max(0.0)),
        SceneDim::Percent(p) => percent(p.clamp(0.0, 1.0)),
        SceneDim::Fill | SceneDim::Auto => auto(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{stack, HAlign, NativeBackend, SceneSize, VAlign};

    const NATIVE: NativeBackend = NativeBackend;
    const TAFFY: TaffyBackend = TaffyBackend;

    fn slot() -> UiRect {
        UiRect::new(100.0, 50.0, 300.0, 200.0)
    }

    /// Побитовое сравнение Vec<UiRect> (parity-оракул FR-068 W1).
    fn assert_same(a: &[UiRect], b: &[UiRect], what: &str) {
        assert_eq!(a.len(), b.len(), "{what}: разное число rect'ов");
        for (i, (ra, rb)) in a.iter().zip(b.iter()).enumerate() {
            assert_eq!(
                (ra.x, ra.y, ra.w, ra.h),
                (rb.x, rb.y, rb.w, rb.h),
                "{what}[{i}]: native {ra:?} vs taffy {rb:?}"
            );
        }
    }

    /// Fit-ряд с зазором: дети фиксированные — побитовый паритет.
    #[test]
    fn row_fit_matches_native() {
        let row = Row {
            gap: 8.0,
            ..Default::default()
        };
        let items = [
            Child::fixed(40.0, 20.0),
            Child::fixed(60.0, 30.0),
            Child::fixed(10.0, 10.0),
        ];
        assert_same(
            &NATIVE.lay_out_row(row, slot(), &items),
            &TAFFY.lay_out_row(row, slot(), &items),
            "row fit",
        );
    }

    /// Grow-распределение свободного места (F-14) — паритет на ЦЕЛЫХ долях.
    /// Дробные доли — документированное расхождение (см. следующий тест):
    /// taffy округляет целевые main-размеры по CSS spec §9.7 (rounding on
    /// freeze), Native — без округления (≤ 0.5 ui px на доле).
    #[test]
    fn row_grow_matches_native() {
        let row = Row {
            gap: 10.0,
            ..Default::default()
        };
        // свободное = 300 − (50+20+20) − 20 = 190 → по 95 (целые)
        let items = [
            Child::fixed(50.0, 20.0),
            Child::flexible(20.0, 20.0, 1.0),
            Child::flexible(20.0, 20.0, 1.0),
        ];
        assert_same(
            &NATIVE.lay_out_row(row, slot(), &items),
            &TAFFY.lay_out_row(row, slot(), &items),
            "row grow integral",
        );
    }

    /// Дробные доли grow — документированное расхождение (≤ 0.5 ui px):
    /// тест ФИКСИРУЕТ поведение taffy (rounding on freeze), чтобы
    /// расхождение было видимым, а не случайным.
    #[test]
    fn row_grow_fractional_is_documented_divergence() {
        let row = Row {
            gap: 10.0,
            ..Default::default()
        };
        let items = [
            Child::fixed(50.0, 20.0),
            Child::flexible(20.0, 20.0, 1.0),
            Child::flexible(20.0, 20.0, 3.0),
        ];
        let native = NATIVE.lay_out_row(row, slot(), &items);
        let taffy = TAFFY.lay_out_row(row, slot(), &items);
        // Native: 20 + 190·0.25 = 67.5 (без округления)
        assert_eq!(native[1].w, 67.5);
        // taffy: округление к целому ui px (CSS spec rounding on freeze)
        assert_eq!(taffy[1].w, 68.0, "taffy округляет долю к целому px");
        assert!((native[1].w - taffy[1].w).abs() <= 0.5);
    }

    /// SpaceBetween и End (без переполнения) — паритет.
    #[test]
    fn row_aligns_match_native() {
        let items = [Child::fixed(40.0, 20.0), Child::fixed(60.0, 30.0)];
        for main in [MainAlign::Start, MainAlign::SpaceBetween, MainAlign::End] {
            let row = Row {
                gap: 8.0,
                main,
                ..Default::default()
            };
            assert_same(
                &NATIVE.lay_out_row(row, slot(), &items),
                &TAFFY.lay_out_row(row, slot(), &items),
                "row main align",
            );
        }
        // поперечные
        for cross in [CrossAlign::Start, CrossAlign::Center, CrossAlign::End] {
            let row = Row {
                gap: 8.0,
                cross,
                ..Default::default()
            };
            assert_same(
                &NATIVE.lay_out_row(row, slot(), &items),
                &TAFFY.lay_out_row(row, slot(), &items),
                "row cross align",
            );
        }
    }

    /// Wrap (F-15): перенос по строкам, высота строки = max — паритет.
    #[test]
    fn row_wrap_matches_native() {
        let row = Row {
            gap: 8.0,
            policy: RowPolicy::Wrap,
            ..Default::default()
        };
        let items = [
            Child::fixed(120.0, 20.0),
            Child::fixed(120.0, 40.0),
            Child::fixed(80.0, 10.0),
            Child::fixed(60.0, 12.0),
        ];
        assert_same(
            &NATIVE.lay_out_row(row, slot(), &items),
            &TAFFY.lay_out_row(row, slot(), &items),
            "row wrap",
        );
    }

    /// Column (Fit + grow + End) — паритет.
    #[test]
    fn column_matches_native() {
        for main in [MainAlign::Start, MainAlign::End] {
            let column = Column {
                gap: 6.0,
                main,
                ..Default::default()
            };
            let items = [Child::fixed(40.0, 50.0), Child::flexible(40.0, 30.0, 1.0)];
            assert_same(
                &NATIVE.lay_out_column(column, slot(), &items),
                &TAFFY.lay_out_column(column, slot(), &items),
                "column",
            );
        }
    }

    /// grid_cells с неравными явными треками (T2) — побитовый паритет.
    #[test]
    fn grid_matches_native() {
        let s = slot();
        let cols = [60.0, 120.0, 30.0];
        assert_same(
            &NATIVE.lay_out_grid(s, &cols, 2, 24.0, UiVec2::new(8.0, 6.0)),
            &TAFFY.lay_out_grid(s, &cols, 2, 24.0, UiVec2::new(8.0, 6.0)),
            "grid unequal tracks",
        );
    }

    /// Расхождение C3: SqueezeTail ≠ flex_shrink (задокументировано,
    /// §Контракт-4) — тест ФИКСИРУЕТ различие, чтобы расхождение было
    /// видимым, а не случайным.
    #[test]
    fn squeeze_tail_is_documented_divergence() {
        let row = Row {
            policy: RowPolicy::SqueezeTail,
            gap: 4.0,
            ..Default::default()
        };
        let items = [
            Child::fixed(150.0, 20.0),
            Child::fixed(150.0, 20.0),
            Child::fixed(150.0, 20.0),
        ];
        let native = NATIVE.lay_out_row(row, slot(), &items);
        let taffy = TAFFY.lay_out_row(row, slot(), &items);
        // Native: хвост сжимается до 0 дословно
        assert_eq!(native[2].w, 0.0, "native SqueezeTail: хвост == 0");
        assert_eq!(native[0].w, 150.0, "native: голова не сжата");
        // taffy (flex_shrink по всем): голова тоже сжимается — расхождение
        assert!(
            taffy[0].w < 150.0,
            "taffy flex_shrink сжимает и голову (C3)"
        );
    }

    /// `centered` ≡ `stack(slot, size, Center, Center)` при помещении.
    #[test]
    fn centered_matches_stack() {
        let s = slot();
        let size = UiVec2::new(120.0, 80.0);
        let native = stack(s, size, HAlign::Center, VAlign::Center);
        let taffy = TAFFY.centered(s, size);
        assert_eq!(
            (native.x, native.y, native.w, native.h),
            (taffy.x, taffy.y, taffy.w, taffy.h),
            "centered vs stack(Center, Center)"
        );
    }

    /// Сцена: percent-размер + absolute-позиция (CSS-семантика).
    #[test]
    fn scene_percent_and_absolute() {
        let scene = SceneNode::column(
            200.0,
            100.0,
            0.0,
            vec![
                SceneNode::leaf(50.0, 20.0),
                SceneNode {
                    kind: SceneKind::Leaf,
                    size: SceneSize {
                        w: SceneDim::Percent(0.5),
                        h: SceneDim::fixed(10.0),
                    },
                    position: ScenePosition::Absolute { x: 10.0, y: 5.0 },
                    ..SceneNode::default()
                },
            ],
        );
        let rects = TAFFY.lay_out_scene(UiRect::new(0.0, 0.0, 400.0, 300.0), &scene);
        assert_eq!(rects.len(), 3);
        // корень == слот
        assert_eq!(rects[0], UiRect::new(0.0, 0.0, 200.0, 100.0));
        // лист в потоке
        assert_eq!(rects[1], UiRect::new(0.0, 0.0, 50.0, 20.0));
        // absolute: 50% ширины родителя = 100, позиция (10, 5)
        assert_eq!(rects[2], UiRect::new(10.0, 5.0, 100.0, 10.0));
    }

    /// Сцена: fill по главной оси (flex-grow 1, basis 0) — доля свободного.
    #[test]
    fn scene_fill_main_axis() {
        let scene = SceneNode::row(
            100.0,
            50.0,
            10.0,
            vec![
                SceneNode::leaf(30.0, 20.0),
                SceneNode {
                    kind: SceneKind::Leaf,
                    size: SceneSize {
                        w: SceneDim::Fill,
                        h: SceneDim::fixed(20.0),
                    },
                    ..SceneNode::default()
                },
            ],
        );
        let rects = TAFFY.lay_out_scene(UiRect::new(0.0, 0.0, 300.0, 200.0), &scene);
        // свободное = 100 − 30 − 10 = 60 → fill-лист занимает 60
        assert_eq!(rects[2], UiRect::new(40.0, 0.0, 60.0, 20.0));
    }

    /// Сцена: scroll content-shift (offset сдвигает потомков по −Y) и
    /// sticky-кламп (прилипание к container_y + top).
    #[test]
    fn scene_scroll_shift_and_sticky() {
        let scene = SceneNode::column(
            100.0,
            80.0,
            0.0,
            vec![
                SceneNode::leaf(100.0, 40.0).at(ScenePosition::Sticky { top: 0.0 }),
                SceneNode::leaf(100.0, 40.0),
                SceneNode::leaf(100.0, 40.0),
            ],
        )
        .scrolled(30.0);
        let rects = TAFFY.lay_out_scene(UiRect::new(0.0, 0.0, 300.0, 200.0), &scene);
        // shift = −30: поток был y = 0/40/80 → 40-лист ушёл в −30+... :
        // sticky-лист: flow 0 − 30 = −30 → кламп к 80+0? НЕТ: container_y=0,
        // top=0 → target = 0; −30 < 0 → y = 0.
        assert_eq!(rects[1].y, 0.0, "sticky прилип к верху контейнера");
        // второй лист: 40 − 30 = 10
        assert_eq!(rects[2], UiRect::new(0.0, 10.0, 100.0, 40.0));
        // третий: 80 − 30 = 50
        assert_eq!(rects[3], UiRect::new(0.0, 50.0, 100.0, 40.0));
    }

    /// Сцена: fixed-позиция (viewport-семантика — относительно корня,
    /// сдвиг scroll-предка не действует).
    #[test]
    fn scene_fixed_ignores_scroll() {
        let scene = SceneNode::column(
            100.0,
            80.0,
            0.0,
            vec![SceneNode {
                kind: SceneKind::Leaf,
                size: SceneSize::fixed(50.0, 20.0),
                position: ScenePosition::Fixed { x: 200.0, y: 150.0 },
                ..SceneNode::default()
            }],
        )
        .scrolled(25.0);
        let rects = TAFFY.lay_out_scene(UiRect::new(0.0, 0.0, 300.0, 200.0), &scene);
        // fixed: корневые координаты, без content-shift
        assert_eq!(rects[1], UiRect::new(200.0, 150.0, 50.0, 20.0));
    }

    /// Сцена: aspect-ratio (высота выводится из ширины) и grid-span.
    /// Ячейки сетки — авто-размеры (растягиваются в трек дефолтным
    /// stretch; явный 0-размер не растягивается).
    #[test]
    fn scene_aspect_and_grid_span() {
        let scene = SceneNode {
            kind: SceneKind::Grid {
                cols: vec![
                    SceneTrack::Length(50.0),
                    SceneTrack::Length(50.0),
                    SceneTrack::Length(50.0),
                ],
                row_h: SceneDim::fixed(20.0),
                gap: UiVec2::new(0.0, 0.0),
            },
            size: SceneSize::fixed(150.0, 60.0),
            children: vec![
                SceneNode::default(),
                SceneNode::default().spanning(2),
                SceneNode {
                    kind: SceneKind::Leaf,
                    size: SceneSize::default(),
                    aspect: Some(2.0),
                    ..SceneNode::default()
                },
            ],
            ..SceneNode::default()
        };
        let rects = TAFFY.lay_out_scene(UiRect::new(0.0, 0.0, 300.0, 200.0), &scene);
        // span=2: второй лист занимает колонки 2–3 (x = 50, w = 100)
        assert_eq!(rects[2].x, 50.0);
        assert_eq!(rects[2].w, 100.0);
        // aspect 2.0 при растянутой ширине ячейки 50 → высота 25
        assert_eq!(rects[3].h, 25.0, "aspect-ratio: h = w / 2");
    }
}
