//! Расширенная сцена вёрстки (FR-068 W1, ADR-0014 N2 «близость к web ui
//! html5»): нейтральное к backend'ам дерево для возможностей ЗА пределами
//! V-5 примитивов (§Контракт-1 — сигнатуры `Row/Column/grid_cells` не
//! меняются до W3, поэтому CSS-семантика percent/aspect-ratio/position/
//! overflow выражается отдельным деревом [`SceneNode`], а не расширением
//! примитивов).
//!
//! Сцена — ДАННЫЕ без вычислений (паттерн Painter FR-057): backend
//! ([`super::TaffyBackend`] в W1, `FlexLayoutEngine` в W2) конвертирует
//! дерево в свои стили и возвращает rect'ы всех узлов в DFS-порядке
//! ([`SceneNode::walk_preorder`] — тот же порядок, что у результата
//! раскладки). Типы без внешних зависимостей (G7): taffy-типы не протекают
//! наружу — конвертация внутри `taffy_backend.rs`.
//!
//! # Контракт `lay_out_scene` (TaffyBackend, W1)
//!
//! - Результат — rect'ы ВСЕХ узлов (контейнеры и листья) в DFS pre-order;
//!   `[0]` — корень (== слот), далее поддеревья по порядку детей.
//! - [`SceneDim::Percent`] — доля от соответствующего размера родителя
//!   (CSS-семантика content-box; паддингов в сцене нет).
//! - [`SceneDim::Fill`] — главной оси: flex-grow 1 при basis 0 (доля
//!   свободного места); поперечной оси: stretch до контент-бокса родителя.
//! - [`ScenePosition::Absolute`] — вне потока, координаты относительно
//!   border-box родителя (родители без паддингов — CSS padding-box);
//!   [`ScenePosition::Fixed`] — относительно корневого слота (viewport).
//! - [`ScenePosition::Sticky`] — в потоке; при прокрутке ([`SceneNode::offset`]
//!   предка) прилипает к `container_y + top` (эмуляция поверх taffy —
//!   taffy 0.14 не имеет position:sticky; ADR-0014 N2 «надстройки поверх»).
//! - [`SceneNode::offset`] (scroll-контейнер) — content-shift: все
//!   потомки смещаются на `−offset` по главной оси контейнера (семантика
//!   отрисовки прокрученного контента; вычисленные taffy позиции — до
//!   сдвига, shift — post-processing, как и sticky).
//! - [`SceneOverflow::Hidden`] — rect'ы потомков НЕ меняются (вычисляются
//!   полностью); клип — забота потребителя ([`crate::paint::PaintItem::
//!   ClipRect`] → scissor FR-056), как в HTML (overflow:hidden не меняет
//!   computed layout, только отрисовку).

use crate::geometry::UiVec2;
use crate::layout::{CrossAlign, MainAlign};

/// Размер по одной оси в расширенной сцене (FR-068 W1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SceneDim {
    /// Авто (контент или вывод из `aspect`) — default.
    #[default]
    Auto,
    /// Фиксированный размер (ui px).
    Length(f32),
    /// Процент от размера родителя по той же оси (CSS `%`).
    Percent(f32),
    /// Главная ось: flex-grow 1 (доля свободного места); поперечная:
    /// stretch до родителя. В листе трактуется как `Auto` (контент/aspect).
    Fill,
}

impl SceneDim {
    /// Фиксированный размер (сокращение; отрицательные — в 0).
    pub fn fixed(px: f32) -> Self {
        Self::Length(px.max(0.0))
    }
}

/// Размер узла сцены по обеим осям ([`SceneDim`]).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SceneSize {
    pub w: SceneDim,
    pub h: SceneDim,
}

impl SceneSize {
    /// Оба размера фиксированы.
    pub fn fixed(w: f32, h: f32) -> Self {
        Self {
            w: SceneDim::fixed(w),
            h: SceneDim::fixed(h),
        }
    }

    /// Ширина фиксирована, высота — авто (пара к `aspect`).
    pub fn fixed_w(w: f32) -> Self {
        Self {
            w: SceneDim::fixed(w),
            h: SceneDim::Auto,
        }
    }
}

/// Позиция узла сцены (CSS position-семантика, FR-068 W1).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ScenePosition {
    /// В потоке (обычный ребёнок flex/grid).
    #[default]
    InFlow,
    /// Вне потока; x/y относительно border-box родителя (ui px).
    Absolute { x: f32, y: f32 },
    /// Вне потока; x/y относительно КОРНЕВОГО слота (viewport-семантика;
    /// в taffy — re-parent в root-overlay при конвертации).
    Fixed { x: f32, y: f32 },
    /// В потоке, но при прокрутке предка со [`SceneNode::offset`]
    /// прилипает: `y = max(flow_y − offset, container_y + top)`
    /// (эмуляция CSS sticky; см. модульную доку).
    Sticky { top: f32 },
}

/// Политика переполнения контейнера сцены (CSS overflow). Rect'ы потомков
/// вычисляются полностью — клип исполняет потребитель
/// ([`crate::paint::PaintItem::ClipRect`] → scissor FR-056).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneOverflow {
    /// Переполнение видно (default; ловится линтом G4 на потребителе).
    #[default]
    Visible,
    /// Клип по контент-боксу контейнера (draw-семантика).
    Hidden,
}

/// Трек колонки/строки CSS Grid в расширенной сцене (FR-068 W1; неравные/
/// процентные/дробные треки — T2-триггер ADR-0013, территория taffy).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SceneTrack {
    /// Авто-трек (по контенту; default).
    #[default]
    Auto,
    /// Фиксированная ширина (ui px).
    Length(f32),
    /// Процент от ширины/высоты контейнера.
    Percent(f32),
    /// Доля свободного места (CSS `1fr`).
    Fill,
}

impl SceneTrack {
    /// Фиксированный трек (сокращение).
    pub fn fixed(px: f32) -> Self {
        Self::Length(px.max(0.0))
    }
}

/// Вид узла расширенной сцены (FR-068 W1).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SceneKind {
    /// Лист (не контейнер): фиксированный/процентный/aspect-размер.
    #[default]
    Leaf,
    /// Горизонтальный flex-контейнер (CSS flexbox row).
    Row {
        gap: f32,
        main: MainAlign,
        cross: CrossAlign,
        /// Перенос по строкам (CSS flex-wrap: wrap).
        wrap: bool,
    },
    /// Вертикальный flex-контейнер (CSS flexbox column).
    Column {
        gap: f32,
        main: MainAlign,
        cross: CrossAlign,
    },
    /// CSS Grid: явные треки колонок ([`SceneTrack`]), высота строки
    /// [`SceneDim`], зазор по обеим осям; дети размещаются row-major
    /// авто-потоком, спан ребёнка — поле [`SceneNode::span`].
    Grid {
        cols: Vec<SceneTrack>,
        row_h: SceneDim,
        gap: UiVec2,
    },
}

/// Узел расширенной сцены (FR-068 W1): [`SceneKind`] + общие поля
/// размера/позиции/переполнения. Конструкторы-сокращения — внизу.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneNode {
    /// Вид узла (лист/row/column/grid).
    pub kind: SceneKind,
    /// Размер по обеим осям.
    pub size: SceneSize,
    /// Позиция (в потоке / absolute / fixed / sticky).
    pub position: ScenePosition,
    /// Переполнение контейнера (клип — на потребителе, см. модульную доку).
    pub overflow: SceneOverflow,
    /// Scroll-offset контейнера (content-shift потомков по главной оси,
    /// ui px ≥ 0; семантика прокрутки — см. модульную доку).
    pub offset: f32,
    /// Соотношение сторон w/h (CSS aspect-ratio): при определённой ширине
    /// высота выводится как `w / aspect` (и наоборот).
    pub aspect: Option<f32>,
    /// Спан колонок в [`SceneKind::Grid`] (CSS grid-column: span N; ≥ 1).
    pub span: u16,
    /// Дети (у листа — пусто; absolute/fixed дети допустимы у любого
    /// контейнера).
    pub children: Vec<SceneNode>,
}

impl SceneNode {
    /// Лист фиксированного размера (в потоке).
    pub fn leaf(w: f32, h: f32) -> Self {
        Self {
            kind: SceneKind::Leaf,
            size: SceneSize::fixed(w, h),
            ..Self::default()
        }
    }

    /// Row-контейнер фиксированного размера с детьми (в потоке).
    pub fn row(w: f32, h: f32, gap: f32, children: Vec<SceneNode>) -> Self {
        Self {
            kind: SceneKind::Row {
                gap,
                main: MainAlign::Start,
                cross: CrossAlign::Start,
                wrap: false,
            },
            size: SceneSize::fixed(w, h),
            children,
            ..Self::default()
        }
    }

    /// Column-контейнер фиксированного размера с детьми (в потоке).
    pub fn column(w: f32, h: f32, gap: f32, children: Vec<SceneNode>) -> Self {
        Self {
            kind: SceneKind::Column {
                gap,
                main: MainAlign::Start,
                cross: CrossAlign::Start,
            },
            size: SceneSize::fixed(w, h),
            children,
            ..Self::default()
        }
    }

    /// Задать позицию (builder; FR-068 W1 — сцена строится декларативно).
    pub fn at(mut self, position: ScenePosition) -> Self {
        self.position = position;
        self
    }

    /// Задать overflow:hidden (builder).
    pub fn clipped(mut self) -> Self {
        self.overflow = SceneOverflow::Hidden;
        self
    }

    /// Задать scroll-offset (builder).
    pub fn scrolled(mut self, offset: f32) -> Self {
        self.offset = offset.max(0.0);
        self
    }

    /// Задать aspect-ratio (builder).
    pub fn ratio(mut self, w_over_h: f32) -> Self {
        self.aspect = Some(w_over_h.max(0.0));
        self
    }

    /// Задать размер (builder).
    pub fn sized(mut self, size: SceneSize) -> Self {
        self.size = size;
        self
    }

    /// Задать спан grid-колонок (builder).
    pub fn spanning(mut self, cols: u16) -> Self {
        self.span = cols.max(1);
        self
    }

    /// Плоский обход дерева в DFS pre-order (корень первым, затем дети
    /// по порядку) — порядок результата `lay_out_scene` совпадает с этим
    /// обходом (контракт [`super::TaffyBackend::lay_out_scene`]).
    pub fn walk_preorder(&self) -> Vec<&SceneNode> {
        let mut out = Vec::new();
        self.push_preorder(&mut out);
        out
    }

    fn push_preorder<'a>(&'a self, out: &mut Vec<&'a SceneNode>) {
        out.push(self);
        for child in &self.children {
            child.push_preorder(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DFS pre-order: корень, потом поддеревья детей по порядку.
    #[test]
    fn walk_preorder_is_dfs() {
        let tree = SceneNode::row(
            100.0,
            50.0,
            4.0,
            vec![
                SceneNode::leaf(10.0, 10.0),
                SceneNode::column(
                    20.0,
                    40.0,
                    2.0,
                    vec![SceneNode::leaf(8.0, 8.0), SceneNode::leaf(8.0, 8.0)],
                ),
                SceneNode::leaf(12.0, 12.0),
            ],
        );
        let nodes = tree.walk_preorder();
        assert_eq!(
            nodes.len(),
            6,
            "корень + 3 поддерева (лист, колонка+2, лист)"
        );
        assert_eq!(
            nodes[0].kind,
            SceneKind::Row {
                gap: 4.0,
                main: MainAlign::Start,
                cross: CrossAlign::Start,
                wrap: false
            }
        );
        assert_eq!(nodes[1], &SceneNode::leaf(10.0, 10.0));
        assert!(matches!(nodes[2].kind, SceneKind::Column { .. }));
        assert_eq!(nodes[3], &SceneNode::leaf(8.0, 8.0));
        assert_eq!(nodes[4], &SceneNode::leaf(8.0, 8.0));
        assert_eq!(nodes[5], &SceneNode::leaf(12.0, 12.0));
    }

    /// Builder'ы не рвут цепочку и нормализуют значения (offset ≥ 0,
    /// span ≥ 1, aspect ≥ 0, фиксированные размеры ≥ 0).
    #[test]
    fn builders_normalize() {
        let n = SceneNode::leaf(-5.0, 10.0)
            .sized(SceneSize::fixed_w(200.0))
            .ratio(1.5)
            .at(ScenePosition::Absolute { x: 3.0, y: -1.0 })
            .clipped()
            .scrolled(-7.0)
            .spanning(0);
        assert_eq!(n.size, SceneSize::fixed_w(200.0));
        assert_eq!(n.aspect, Some(1.5));
        assert_eq!(n.position, ScenePosition::Absolute { x: 3.0, y: -1.0 });
        assert_eq!(n.overflow, SceneOverflow::Hidden);
        assert_eq!(n.offset, 0.0, "отрицательный offset → 0");
        assert_eq!(n.span, 1, "span 0 → 1");
        assert_eq!(SceneDim::fixed(-2.0), SceneDim::Length(0.0));
        assert_eq!(SceneTrack::fixed(-2.0), SceneTrack::Length(0.0));
    }
}
