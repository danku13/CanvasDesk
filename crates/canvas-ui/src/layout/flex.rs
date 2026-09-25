//! `FlexLayoutEngine` (FR-068 W2, ADR-0015 §Решение п.3 «свой layout»):
//! собственный движок вёрстки БЕЗ внешних зависимостей — реализация
//! [`LayoutBackend`] + расширенной сцены ([`SceneNode`]) на чистой f32-
//! арифметике (zero-dep инвариант G7; §Контракт-2 FR-068).
//!
//! # Побитовый паритет с taffy на совместимых политиках
//!
//! (FR-068 §W2: `Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка):
//! - grow-распределение — CSS flexbox §9.7 «Resolving Flexible Lengths»
//!   в порядке операций taffy 0.14: `target = basis + free · (grow / Σgrow)`,
//!   `free = inner − gap_total − Σ(баз unfrozen)`; при Σgrow < 1 —
//!   квотирование `initial_free · Σgrow − gap_total` (taffy-форма);
//! - justify-офсеты — `compute_alignment_offset` c fallback'ами
//!   `apply_alignment_fallback`: SpaceBetween при переполнении/одном
//!   ребёнке деградирует в Start; `End` — без fallback: при переполнении
//!   дети выталкиваются ЗА начальный край (CSS unsafe alignment —
//!   документированное расхождение с Native-клампом);
//! - поперечные офсеты — от линии: Center = `free/2`, End = `free`
//!   (могут быть отрицательными — ребёнок выступает за оба края);
//! - финальное приведение к целой px-сетке — зеркало
//!   `taffy::compute::round_layout`: локация округляется ПОЗИЦИОННО
//!   (parent-relative), размер — разность округлённых КРАЁВ по
//!   кумулятивной unrounded-координате от корня; функция round —
//!   дословная копия `taffy::util::sys::round` (std-путь).
//!
//! Паритет-гейты: `tests/flex_vs_taffy_parity.rs` (1000 деревьев,
//! ≥ 80% побитово) + 15 golden-эталонов html5/CD demos (общие для обоих
//! backend'ов).
//!
//! # Документированные семантики
//!
//! - `SqueezeTail` — ДОСЛОВНО (хвост до 0 ширины, half-open hit;
//!   §Контракт-4 FR-068: «FlexLayoutEngine решает»), не flex_shrink
//!   (расхождение C3 с taffy — документировано, не fail в parity).
//! - Сцена: percent (от внутреннего размера родителя), `Fill` (главная —
//!   grow 1 basis 0; поперечная — stretch до линии), aspect-ratio,
//!   Absolute (от border-box родителя) / Fixed (от корневого слота —
//!   ре-парент к корню) / Sticky (в потоке + пост-кламп с трансляцией
//!   поддерева; FR-074 — по ОБОИМ осям: `top` против Y-scroll-предка,
//!   `left` против X-scroll-предка), scroll-offset content-shift,
//!   overflow (rect'ы не меняет — клип у потребителя, `PaintItem::ClipRect`
//!   FR-056), Grid (треки Length/Percent/Fill + FR-074: Auto по контенту
//!   span-1 ячеек, `minmax(min, max)` — кламп контента в границы,
//!   `max: Fill` — гибкий трек с полом min) + span + definite/Auto высоты
//!   строк.
//! - Grid-ячейки: Fill/Auto тянутся в область трека (default stretch
//!   taffy); definite/Percent — собственный размер (CSS grid-item,
//!   старт-выравнивание, переполнение видно).
//! - Sticky/scroll — пост-обработка формул 1:1 из `taffy_backend.rs`.
//!
//! # Grid-треки: авторасчёт и minmax (FR-074)
//!
//! Колонки — шаги CSS Grid §11.5–11.8 в порядке taffy 0.14 (паритет
//! подтверждён parity-тестами; контент = max-content span-1 ячеек,
//! min-content отдельно не моделируется):
//! 1. Плейсмент row-major (sparse cursor со спанами) — [`grid_placements`].
//! 2. Контент-вклад трека = max max-content ширин ЯЧЕЕК SPAN 1 трека
//!    (span>1 не растят авторасчётные треки — упрощение, докурировано;
//!    ячейка всё равно тянется на область спана).
//! 3. База/лимит: Length/Percent — base=limit=definite; Auto — base=
//!    limit=контент; Fill — base 0, limit ∞ (Fr); MinMax{min,max} —
//!    base = min (definite или контент), limit = max definite | контент
//!    (max Auto) | ∞ (max Fill → Fr).
//! 4. §11.6 Maximise: свободное место (inner − Σбаз − зазоры) поровну
//!    трекам с конечным лимитом, с заморозкой достигших лимита.
//! 5. §11.7 Expand Flexible (taffy find_size_of_fr, факторы = 1): fr —
//!    наибольший, при котором Σ max(base, fr) ≤ inner; база больше fr —
//!    трек остаётся на базе (переполнение видно, G4).
//! 6. §11.8 Stretch auto (taffy default STRETCH): остаток поровну
//!    AutoMax-трекам (поверх лимитов — как в браузерах по умолчанию).

use super::{
    Child, Column, CrossAlign, LayoutBackend, LayoutFeatures, MainAlign, MeasuredItem, Row,
    RowPolicy, SceneDim, SceneKind, SceneNode, ScenePosition, SceneTrack, TrackMax, TrackMin,
    UiRect, UiVec2,
};
use crate::measure::TextMeasurer;

/// Собственный движок вёрстки (FR-068 W2). ZST без состояния
/// (immediate-mode, D2 ADR-0013) — `Send + Sync` бесплатно, статический
/// [`super::default_backend`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FlexLayoutEngine;

impl LayoutBackend for FlexLayoutEngine {
    fn features(&self) -> LayoutFeatures {
        // Полный flexbox (БЕЗ CSS flex_shrink — SqueezeTail дословен,
        // §Контракт-4) + сетка/measured + сцена (percent/aspect/clip/
        // sticky-эмуляция). Бит FLEX_SHRINK не выставляется: сжатие —
        // именованная политика `SqueezeTail`, а не CSS-фактор.
        LayoutFeatures::FLEX_GROW
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
        match row.policy {
            RowPolicy::SqueezeTail => self.squeeze_tail(row.gap, slot, items),
            _ => {
                let children: Vec<FlexChild> = items
                    .iter()
                    .map(|c| FlexChild {
                        basis: c.w.max(0.0),
                        cross: c.h.max(0.0),
                        grow: c.grow.max(0.0),
                    })
                    .collect();
                let (placed, _, _) = layout_flex_container(
                    Axis::X,
                    row.gap,
                    row.main,
                    row.cross,
                    row.policy == RowPolicy::Wrap,
                    (slot.w, slot.h),
                    &children,
                );
                finish_line(&placed, Axis::X, slot)
            }
        }
    }

    fn lay_out_column(&self, column: Column, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let children: Vec<FlexChild> = items
            .iter()
            .map(|c| FlexChild {
                // Главная ось колонки — Y: basis = высота, cross = ширина.
                basis: c.h.max(0.0),
                cross: c.w.max(0.0),
                grow: c.grow.max(0.0),
            })
            .collect();
        let (placed, _, _) = layout_flex_container(
            Axis::Y,
            column.gap,
            column.main,
            column.cross,
            false,
            // (main, cross) в осях контейнера: главная колонки — Y (высота).
            (slot.h, slot.w),
            &children,
        );
        finish_line(&placed, Axis::Y, slot)
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
        // Тот же resolve, что у всех backend'ов — единая точка замера
        // (`MeasuredItem::resolve`), шейпинг бит-в-бит (через Shaper W2).
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
        // Явные колонки — накопление как taffy (Points-треки, row-major)
        // + round-layout pass (локация позиционно, размер — края).
        let row_h = row_h.max(0.0);
        let mut rects = Vec::with_capacity(cols.len().saturating_mul(rows));
        for r in 0..rows {
            let y = r as f32 * (row_h + gap.y.max(0.0));
            let mut x = 0.0f32;
            for &w in cols {
                let w = w.max(0.0);
                rects.push(UiRect::new(x, y, w, row_h));
                x += w + gap.x.max(0.0);
            }
        }
        rects
            .into_iter()
            .map(|r| {
                let (x, y, w, h) = (r.x, r.y, r.w, r.h);
                UiRect::new(
                    slot.x + taffy_round(x),
                    slot.y + taffy_round(y),
                    taffy_round(x + w) - taffy_round(x),
                    taffy_round(y + h) - taffy_round(y),
                )
            })
            .collect()
    }
}

impl FlexLayoutEngine {
    /// Раскладка расширенной сцены ([`SceneNode`], FR-068 W2): контракт —
    /// модульная докa `scene` (DFS pre-order, `[0]` — корень; percent/
    /// aspect/absolute/fixed/sticky/scroll/overflow — как у
    /// `TaffyBackend::lay_out_scene`, побитово на сценах demos).
    pub fn lay_out_scene(&self, slot: UiRect, scene: &SceneNode) -> Vec<UiRect> {
        // 1. DFS pre-order дерево с parent/children-индексами.
        let mut tree = SceneTree::build(scene);
        // 2. Раскладка (unrounded): локации parent-relative, корень (0,0).
        tree.layout_node(0, None, (slot.w, slot.h), (slot.w, slot.h));
        // 3. round-layout pass + абсолютные координаты (таффy-чтение:
        //    abs = abs(parent) + round(loc); размер — края cumulative).
        let rects = tree.round_and_absolve(slot);
        // 4. Пост-обработка (формулы taffy_backend 1:1): content-shift
        //    scroll-предков + sticky-кламп с трансляцией поддерева.
        post_process(&tree, rects)
    }

    /// `SqueezeTail` — дословная семантика (Native-формулы 1:1, БЕЗ
    /// round-layout: дробные ширины чипов сохраняются дословно — контракт
    /// «чип = измеренная подпись + пад» CR-015/F-17). Хвост сжимается до
    /// нуля, half-open hit. §Контракт-4 FR-068; расхождение с taffy
    /// (flex_shrink) — C3.
    fn squeeze_tail(&self, gap: f32, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let mut x = 0.0f32;
        let limit = slot.w;
        items
            .iter()
            .map(|c| {
                let remaining = (limit - x).max(0.0);
                let w = c.w.min(remaining);
                // Кламп x к limit — дословно Native (за слот позиция не
                // выходит; вырожденный хвост — пустой, half-open).
                let rect = UiRect::new(x.min(limit), 0.0, w, c.h);
                x += w + gap;
                rect
            })
            .map(|r| UiRect::new(slot.x + r.x, slot.y + r.y, r.w, r.h))
            .collect()
    }
}

// =============================================================================
// Ядро flexbox: ось, CSS §9.7, justify/align, wrap, round-layout
// =============================================================================

/// Главная ось контейнера (X — row, Y — column/grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// Гипотетические размеры flex-ребёнка (в осях КОНТЕЙНЕРА).
#[derive(Debug, Clone, Copy)]
struct FlexChild {
    /// flex-basis (главный размер до распределения).
    basis: f32,
    /// Гипотетический поперечный размер.
    cross: f32,
    /// flex-grow (CSS §9.7); 0 — фиксированный.
    grow: f32,
}

/// Округление taffy (`util::sys::round`, std-путь) — дословная копия для
/// побитового паритета round-layout (half-away-from-zero).
fn taffy_round(value: f32) -> f32 {
    let f = if value == 0.0 { 0.0 } else { value % 1.0 };
    if f.is_nan() || f == 0.0 {
        value
    } else if value > 0.0 {
        if f < 0.5 {
            value - f
        } else {
            value - f + 1.0
        }
    } else if -f < 0.5 {
        value - f
    } else {
        value - f - 1.0
    }
}

/// Unrounded размещение ребёнка в осях контейнера.
#[derive(Debug, Clone, Copy)]
struct Placed {
    main: f32,
    cross: f32,
    size_main: f32,
    size_cross: f32,
}

/// Финализация V-5 линии: round-layout + сдвиг в слот (один уровень от
/// корня: cum = локация).
fn finish_line(placed: &[Placed], axis: Axis, slot: UiRect) -> Vec<UiRect> {
    placed
        .iter()
        .map(|p| {
            let (x, y, w, h) = match axis {
                Axis::X => (p.main, p.cross, p.size_main, p.size_cross),
                Axis::Y => (p.cross, p.main, p.size_cross, p.size_main),
            };
            UiRect::new(
                slot.x + taffy_round(x),
                slot.y + taffy_round(y),
                taffy_round(x + w) - taffy_round(x),
                taffy_round(y + h) - taffy_round(y),
            )
        })
        .collect()
}

/// Сумма главных зазоров (taffy sum_axis_gaps: n−1 зазоров).
fn total_main_axis_gap(gap: f32, n: usize) -> f32 {
    gap * n.saturating_sub(1) as f32
}

/// Плейсмент row-major auto-flow (sparse cursor со спанами) — ЕДИНЫЙ
/// оракул для measure/layout/row-count (FR-074: раньше курсор был
/// продублирован трижды). Возвращает `(col, row, span)` в ПОРЯДКЕ детей.
/// `n_cols == 0` — вырожденная сетка: каждый ребёнок на своей строке
/// нулевой ширины (col = 0, span = 1).
fn grid_placements(
    spans: impl Iterator<Item = usize>,
    n_cols: usize,
) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    let mut cursor_col = 0usize;
    let mut cursor_row = 0usize;
    for span in spans {
        let span = span.max(1);
        if n_cols == 0 {
            out.push((0, out.len(), 1));
            continue;
        }
        let mut col = cursor_col;
        let mut row = cursor_row;
        while col + span > n_cols {
            col = 0;
            row += 1;
        }
        out.push((col, row, span));
        cursor_col = col + span;
        cursor_row = row;
    }
    out
}

/// Вклад трека в max-content ширину grid-контейнера (FR-074):
/// Length — definite; Percent — 0 (база неопределена в max-content, как
/// раньше); Auto — контент трека; Fill — 0 (гибкий); MinMax — кламп
/// контента в границы (max definite) или минимум (max Auto).
fn track_max_content(track: &SceneTrack, content_w: f32) -> f32 {
    match track {
        SceneTrack::Length(v) => v.max(0.0),
        SceneTrack::Percent(_) => 0.0,
        SceneTrack::Auto => content_w,
        SceneTrack::Fill => 0.0,
        SceneTrack::MinMax { min, max } => {
            let min_size = match min {
                TrackMin::Length(v) => v.max(0.0),
                TrackMin::Percent(_) => 0.0,
                TrackMin::Auto => content_w,
            };
            match max {
                TrackMax::Length(m) => min_size.max(content_w.min(m.max(0.0))),
                TrackMax::Percent(_) => min_size,
                TrackMax::Auto => min_size.max(content_w),
                TrackMax::Fill => min_size,
            }
        }
    }
}

/// CSS §9.7 «Resolving Flexible Lengths» (порядок операций taffy 0.14,
/// growing-ветка; shrink у Flex-детей отсутствует — переполнение видно,
/// G4; SqueezeTail — отдельная дословная политика). → целевые размеры.
fn resolve_flexible_lengths(inner_main: f32, gap_total: f32, line: &[FlexChild]) -> Vec<f32> {
    let n = line.len();
    let mut targets: Vec<f32> = line.iter().map(|c| c.basis).collect();
    if n == 0 {
        return targets;
    }
    // 1. Используемый flex-фактор: Σ баз + зазоры против inner.
    let total_hypothetical: f32 = line.iter().map(|c| c.basis).sum();
    let used_flex_factor = gap_total + total_hypothetical;
    let growing = used_flex_factor < inner_main;
    let shrinking = used_flex_factor > inner_main;
    if !growing && !shrinking {
        return targets; // exactly_sized — все заморожены на базах
    }
    // 2. Неподвижные: grow == 0 (shrink всегда 0 — overflow виден).
    let mut frozen: Vec<bool> = line.iter().map(|c| c.grow == 0.0).collect();

    // 3. Начальное свободное место (frozen — по базам, ещё без целей).
    let used_space = |frozen: &[bool], targets: &[f32]| -> f32 {
        gap_total
            + line
                .iter()
                .zip(frozen.iter().zip(targets))
                .map(|(c, (&fz, &t))| if fz { t } else { c.basis })
                .sum::<f32>()
    };
    let initial_free_space = inner_main - used_space(&frozen, &targets);

    // 4. Цикл (min 0 / max нет — violations нет → один проход).
    loop {
        if frozen.iter().all(|&f| f) {
            break;
        }
        let used = used_space(&frozen, &targets);
        let sum_grow: f32 = line
            .iter()
            .zip(&frozen)
            .filter(|(_, &fz)| !fz)
            .map(|(c, _)| c.grow)
            .sum();
        // b. Оставшееся свободное место (taffy-форма, вкл. квоту Σgrow<1).
        let free_space = if growing && sum_grow < 1.0 {
            (initial_free_space * sum_grow - gap_total).min(inner_main - used)
        } else {
            inner_main - used
        };
        // c. Распределение (is_normal отсекает 0 — как в taffy).
        if free_space.is_normal() && growing && sum_grow > 0.0 {
            for i in 0..n {
                if !frozen[i] {
                    targets[i] = line[i].basis + free_space * (line[i].grow / sum_grow);
                }
            }
        }
        // d/e. Кламп ≥ 0; violations нет → заморозка всех (один проход).
        for i in 0..n {
            if !frozen[i] {
                targets[i] = targets[i].max(0.0);
            }
        }
        for f in frozen.iter_mut() {
            *f = true;
        }
        let _ = shrinking;
    }
    targets
}

/// `apply_alignment_fallback` taffy (compute/common/alignment.rs):
/// SpaceBetween при ≤1 элементе или переполнении деградирует в Start
/// (safe-семантика распределительных ключей); Start/End — без fallback.
fn justify_fallback(free_space: f32, num_items: usize, main: MainAlign) -> MainAlign {
    if num_items <= 1 || free_space <= 0.0 {
        match main {
            MainAlign::SpaceBetween => MainAlign::Start,
            other => other,
        }
    } else {
        main
    }
}

/// Полная раскладка flex-контейнера: линии (wrap), CSS §9.7, justify,
/// align → unrounded placements + контентный размер (для auto-сцен).
fn layout_flex_container(
    _axis: Axis,
    gap: f32,
    main: MainAlign,
    cross: CrossAlign,
    wrap: bool,
    inner: (f32, f32), // (main, cross) definite контейнера
    children: &[FlexChild],
) -> (Vec<Placed>, (f32, f32), Vec<f32>) {
    let inner_main = inner.0;
    let n = children.len();

    // Линии: no-wrap — одна; wrap — жадная упаковка (порог как у Native:
    // EPSILON против ложного переноса на точной подгонке).
    let mut lines: Vec<Vec<usize>> = Vec::new();
    if !wrap {
        lines.push((0..n).collect());
    } else {
        let mut cur: Vec<usize> = Vec::new();
        let mut cur_w = 0.0f32;
        for (i, c) in children.iter().enumerate() {
            let next_w = if cur.is_empty() {
                c.basis
            } else {
                cur_w + gap + c.basis
            };
            if !cur.is_empty() && next_w > inner_main + f32::EPSILON {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
            }
            cur.push(i);
            cur_w = if cur.len() == 1 {
                c.basis
            } else {
                cur_w + gap + c.basis
            };
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
    }

    let cross_gap = if wrap { gap } else { 0.0 };
    let mut placed = vec![
        Placed {
            main: 0.0,
            cross: 0.0,
            size_main: 0.0,
            size_cross: 0.0
        };
        n
    ];
    // Поперечный размер ЛИНИИ каждого ребёнка (для stretch Fill-cross).
    let mut line_cross_of = vec![0.0f32; n];
    let mut line_y = 0.0f32;
    let mut content_main = 0.0f32;

    for line in &lines {
        let items: Vec<FlexChild> = line.iter().map(|&i| children[i]).collect();
        // Главные размеры (CSS §9.7). Для auto-главной оси inner_main
        // уже = Σ баз + зазоры → free 0 → базы без распределения.
        let targets =
            resolve_flexible_lengths(inner_main, total_main_axis_gap(gap, line.len()), &items);
        // Линия: justify (свободное место — от целевых размеров).
        let used: f32 = total_main_axis_gap(gap, line.len()) + targets.iter().sum::<f32>();
        let free = inner_main - used;
        content_main = content_main.max(used);
        let mode = justify_fallback(free, line.len(), main);
        // Поперечный размер линии: single-line definite — контейнер;
        // wrap — max гипотетической поперечной детей линии.
        let line_cross = if !wrap {
            inner.1
        } else {
            items.iter().map(|c| c.cross).fold(0.0f32, f32::max)
        };
        for &i in line {
            line_cross_of[i] = line_cross;
        }
        // Позиции по главной оси: loc_0 = base; loc_i = loc_{i-1} + t + off
        // (off уже содержит зазор — форма compute_alignment_offset).
        let mut cursor = match mode {
            MainAlign::Start | MainAlign::SpaceBetween => 0.0,
            MainAlign::End => free,
        };
        for (k, (&i, &t)) in line.iter().zip(&targets).enumerate() {
            if k > 0 {
                let off = gap
                    + match mode {
                        MainAlign::SpaceBetween => free / (line.len() - 1) as f32,
                        _ => 0.0,
                    };
                cursor += targets[k - 1] + off;
            }
            // Поперечный офсет от линии (Center/End могут уйти в минус —
            // unsafe alignment, как taffy).
            let cross_free = line_cross - children[i].cross;
            let cross_off = line_y
                + match cross {
                    CrossAlign::Start => 0.0,
                    CrossAlign::Center => cross_free / 2.0,
                    CrossAlign::End => cross_free,
                };
            placed[i] = Placed {
                main: cursor,
                cross: cross_off,
                size_main: t,
                size_cross: children[i].cross,
            };
        }
        line_y += line_cross + cross_gap;
    }
    // Контентная поперечная: Σ линий + межстрочные зазоры.
    let content_cross = line_y - if lines.is_empty() { 0.0 } else { cross_gap };
    (placed, (content_main, content_cross), line_cross_of)
}

// =============================================================================
// Сцена: DFS-дерево, percent/aspect/auto/fixed/absolute/grid, round-layout
//
// Двухфазная архитектура (зеркало taffy):
//   A. `measure_content` — bottom-up max-content замер (для Auto-осей и
//      flex-basis Auto-детей; definite-оси читаются напрямую).
//   B. `layout_node` — top-down раскладка: родитель распределяет главные
//      размеры (CSS §9.7), затем рекурсивно раскладывает детей с УЖЕ
//      известными definite-размерами (Fill-дети получают target от
//      родителя — их поддеревья считаются от него, как в taffy).
// =============================================================================

/// Узел сцены в DFS pre-order (unrounded раскладка + пост-контекст).
struct SceneEntry {
    /// Индекс родителя (None — корень; fixed-дети — Some(0)).
    parent: Option<usize>,
    position: ScenePosition,
    /// SceneNode::offset (scroll-контейнер).
    offset: f32,
    /// Главная ось контейнера (направление content-shift).
    axis: Axis,
    /// Дети (все) в pre-order индексах.
    children: Vec<usize>,
    /// Дети в потоке (без fixed/absolute).
    flow: Vec<usize>,
    /// Ссылка-клон узла сцены (kind/size/aspect/span).
    node: SceneNode,
    /// Unrounded локация (parent-relative; fixed — root-relative).
    loc: (f32, f32),
    /// Unrounded размер.
    size: (f32, f32),
}

struct SceneTree {
    nodes: Vec<SceneEntry>,
}

/// Definite-оси: Length/Percent → Some (percent — от `base`); Fill/Auto → None.
fn definite_dim(dim: &SceneDim, base: f32) -> Option<f32> {
    match dim {
        SceneDim::Length(v) => Some(v.max(0.0)),
        SceneDim::Percent(p) => Some(base * p.clamp(0.0, 1.0)),
        SceneDim::Fill | SceneDim::Auto => None,
    }
}

impl SceneTree {
    fn build(scene: &SceneNode) -> Self {
        let mut tree = Self { nodes: Vec::new() };
        tree.push_node(scene, None);
        tree
    }

    fn push_node(&mut self, node: &SceneNode, parent: Option<usize>) -> usize {
        let idx = self.nodes.len();
        let axis = match &node.kind {
            SceneKind::Row { .. } => Axis::X,
            SceneKind::Column { .. } | SceneKind::Grid { .. } => Axis::Y,
            SceneKind::Leaf => Axis::Y,
        };
        self.nodes.push(SceneEntry {
            parent,
            position: node.position,
            offset: node.offset,
            axis,
            children: Vec::new(),
            flow: Vec::new(),
            node: node.clone(),
            loc: (0.0, 0.0),
            size: (0.0, 0.0),
        });
        for child in &node.children {
            let child_idx = self.push_node(child, Some(idx));
            self.nodes[idx].children.push(child_idx);
            match child.position {
                ScenePosition::Absolute { .. } | ScenePosition::Fixed { .. } => {}
                _ => self.nodes[idx].flow.push(child_idx),
            }
        }
        idx
    }

    // --- Фаза A: max-content замер (bottom-up, без мутаций) -----------------

    /// Max-content размер узла (для Auto-осей): definite-оси напрямую,
    /// контейнеры — сумма/максимум контента детей, лист — Length/Auto(0),
    /// aspect-вывод (fixed_w + ratio → h = w/ratio; fixed_h → w = h·ratio).
    /// Percent в max-content — 0 (база неопределена; demos процентов
    /// внутри Auto-контейнеров не используют).
    fn measure_content(&self, idx: usize) -> (f32, f32) {
        let node = &self.nodes[idx].node;
        let mut w = definite_dim(&node.size.w, 0.0);
        let mut h = definite_dim(&node.size.h, 0.0);
        let content = match &node.kind {
            SceneKind::Leaf => (0.0f32, 0.0f32),
            SceneKind::Row { gap, wrap, .. } => {
                // Max-content: Σ баз детей (главная) + зазоры; поперечная —
                // max. Wrap-контейнер: та же Σ (max-content одной линии).
                let mut sum = 0.0f32;
                let mut max_cross = 0.0f32;
                for (k, &ci) in self.nodes[idx].flow.iter().enumerate() {
                    let c = self.measure_content(ci);
                    sum += if k == 0 { c.0 } else { gap + c.0 };
                    max_cross = max_cross.max(c.1);
                }
                let _ = wrap;
                (sum, max_cross)
            }
            SceneKind::Column { gap, .. } => {
                let mut sum = 0.0f32;
                let mut max_cross = 0.0f32;
                for (k, &ci) in self.nodes[idx].flow.iter().enumerate() {
                    let c = self.measure_content(ci);
                    sum += if k == 0 { c.1 } else { gap + c.1 };
                    max_cross = max_cross.max(c.0);
                }
                (max_cross, sum)
            }
            SceneKind::Grid { cols, row_h, gap } => {
                // Ширина — Σ вкладов треков + зазоры (Length/Percent
                // definite; Auto — контент span-1 ячеек трека; MinMax —
                // кламп контента в границы; Fill — 0, гибкий); высота —
                // Σ строк (Length; Percent → 0 без базы; Fill/Auto →
                // контент строк) + зазоры.
                let gap_x = gap.x.max(0.0);
                let gap_y = gap.y.max(0.0);
                let flow = &self.nodes[idx].flow;
                let spans = flow
                    .iter()
                    .map(|&ci| (self.nodes[ci].node.span as usize).max(1));
                let placements = grid_placements(spans, cols.len());
                let mut content_w: Vec<f32> = vec![0.0; cols.len()];
                let mut row_h_max: Vec<f32> = Vec::new();
                for (k, &(col, row, span)) in placements.iter().enumerate() {
                    let ci = flow[k];
                    let c = self.measure_content(ci);
                    if span == 1 && col < cols.len() {
                        content_w[col] = content_w[col].max(c.0);
                    }
                    if row_h_max.len() <= row {
                        row_h_max.resize(row + 1, 0.0);
                    }
                    row_h_max[row] = row_h_max[row].max(c.1);
                }
                let w_sum: f32 = cols
                    .iter()
                    .enumerate()
                    .map(|(i, t)| track_max_content(t, content_w[i]))
                    .sum::<f32>()
                    + gap_x * cols.len().saturating_sub(1) as f32;
                let rows = placements.iter().map(|&(_, r, _)| r + 1).max().unwrap_or(0);
                let h_sum = match row_h {
                    SceneDim::Length(v) => {
                        v.max(0.0) * rows as f32 + gap_y * rows.saturating_sub(1) as f32
                    }
                    SceneDim::Percent(_) => 0.0,
                    SceneDim::Fill | SceneDim::Auto => {
                        row_h_max.iter().sum::<f32>() + gap_y * rows.saturating_sub(1) as f32
                    }
                };
                (w_sum, h_sum)
            }
        };
        if w.is_none() {
            w = Some(content.0);
        }
        if h.is_none() {
            h = Some(content.1);
        }
        let (mut ww, mut hh) = (w.unwrap_or(0.0), h.unwrap_or(0.0));
        if let SceneKind::Leaf = node.kind {
            if let Some(ratio) = node.aspect {
                let w_def = matches!(node.size.w, SceneDim::Length(_) | SceneDim::Percent(_));
                let h_def = matches!(node.size.h, SceneDim::Length(_) | SceneDim::Percent(_));
                if w_def && !h_def {
                    hh = ww / ratio;
                } else if h_def && !w_def {
                    ww = hh * ratio;
                }
            }
        }
        (ww, hh)
    }

    // --- Фаза B: top-down раскладка -----------------------------------------

    /// Раскладка узла. `provided` — definite размер от родителя (главная
    /// ось после распределения / stretch поперечной); None — размер из
    /// SceneSize (percent — от `percent_base`) или контента. `root_inner`
    /// — definite слот корня (percent fixed-детей). Возвращает размер.
    fn layout_node(
        &mut self,
        idx: usize,
        provided: Option<(f32, f32)>,
        percent_base: (f32, f32),
        root_inner: (f32, f32),
    ) -> (f32, f32) {
        let node = self.nodes[idx].node.clone();
        // 1. Размер: provided (Fill/stretch) > Length/Percent > контент.
        let (w, h) = match provided {
            Some((pw, ph)) => (pw, ph),
            None => {
                let w_def = definite_dim(&node.size.w, percent_base.0);
                let h_def = definite_dim(&node.size.h, percent_base.1);
                match (w_def, h_def) {
                    (Some(ww), Some(hh)) => (ww, hh),
                    (wd, hd) => {
                        let content = self.content_of(idx, percent_base);
                        (wd.unwrap_or(content.0), hd.unwrap_or(content.1))
                    }
                }
            }
        };
        self.nodes[idx].size = (w, h);

        // 2. Absolute/Fixed дети — вне потока: percent от содержащего
        //    блока (absolute — родитель, fixed — корень), aspect-вывод;
        //    локация — inset (absolute — border-box родителя, fixed —
        //    корневой слот). Рекурсия раскладывает их поддеревья.
        for &ai in &self.nodes[idx].children.clone() {
            let pos = self.nodes[ai].position;
            let containing = match pos {
                ScenePosition::Absolute { .. } => (w, h),
                ScenePosition::Fixed { .. } => root_inner,
                _ => continue,
            };
            let (ax, ay) = match pos {
                ScenePosition::Absolute { x, y } | ScenePosition::Fixed { x, y } => (x, y),
                _ => (0.0, 0.0),
            };
            self.layout_positioned(ai, containing, root_inner);
            self.nodes[ai].loc = (ax, ay);
        }

        // 3. Flex/Grid дети (в потоке) — рекурсивно с definite-целями.
        match &node.kind {
            SceneKind::Row {
                gap,
                main,
                cross,
                wrap,
            } => {
                self.layout_flex_children(
                    idx,
                    Axis::X,
                    *gap,
                    *main,
                    *cross,
                    *wrap,
                    (w, h),
                    root_inner,
                );
            }
            SceneKind::Column { gap, main, cross } => {
                self.layout_flex_children(
                    idx,
                    Axis::Y,
                    *gap,
                    *main,
                    *cross,
                    false,
                    (w, h),
                    root_inner,
                );
            }
            SceneKind::Grid { cols, row_h, gap } => {
                self.layout_grid_children(idx, cols, row_h, gap, (w, h), root_inner);
            }
            SceneKind::Leaf => {}
        }
        (w, h)
    }

    /// Контентный размер узла (для Auto/Fill-без-родителя осей):
    /// definite-оси поверх max-content замера.
    fn content_of(&self, idx: usize, percent_base: (f32, f32)) -> (f32, f32) {
        let measured = self.measure_content(idx);
        let node = &self.nodes[idx].node;
        let w = definite_dim(&node.size.w, percent_base.0).unwrap_or(measured.0);
        let h = definite_dim(&node.size.h, percent_base.1).unwrap_or(measured.1);
        (w, h)
    }

    /// Раскладка positioned-узла (absolute/fixed): размер от содержащего
    /// блока + рекурсия в поддерево от его definite inner.
    fn layout_positioned(&mut self, idx: usize, containing: (f32, f32), root_inner: (f32, f32)) {
        let node = self.nodes[idx].node.clone();
        let w_def = definite_dim(&node.size.w, containing.0);
        let h_def = definite_dim(&node.size.h, containing.1);
        let (ww, hh) = match (w_def, h_def) {
            (Some(ww), Some(hh)) => (ww, hh),
            (wd, hd) => {
                let content = self.content_of(idx, containing);
                (wd.unwrap_or(content.0), hd.unwrap_or(content.1))
            }
        };
        self.nodes[idx].size = (ww, hh);
        match &node.kind {
            SceneKind::Row {
                gap,
                main,
                cross,
                wrap,
            } => {
                self.layout_flex_children(
                    idx,
                    Axis::X,
                    *gap,
                    *main,
                    *cross,
                    *wrap,
                    (ww, hh),
                    root_inner,
                );
            }
            SceneKind::Column { gap, main, cross } => {
                self.layout_flex_children(
                    idx,
                    Axis::Y,
                    *gap,
                    *main,
                    *cross,
                    false,
                    (ww, hh),
                    root_inner,
                );
            }
            SceneKind::Grid { cols, row_h, gap } => {
                self.layout_grid_children(idx, cols, row_h, gap, (ww, hh), root_inner);
            }
            SceneKind::Leaf => {}
        }
    }

    /// Гипотеза flex-ребёнка (в осях контейнера): basis (Length/Percent/
    /// Fill/Auto→max-content), гипотетический cross, grow.
    fn child_hypo(&self, ci: usize, axis: Axis, inner: (f32, f32)) -> FlexChild {
        let node = &self.nodes[ci].node;
        let (main_d, cross_d) = if axis == Axis::X {
            (node.size.w, node.size.h)
        } else {
            (node.size.h, node.size.w)
        };
        let content = self.measure_content(ci);
        let (basis, grow) = match main_d {
            SceneDim::Length(v) => (v.max(0.0), 0.0),
            SceneDim::Percent(p) => {
                let d = if axis == Axis::X { inner.0 } else { inner.1 };
                (d * p.clamp(0.0, 1.0), 0.0)
            }
            SceneDim::Fill => (0.0, 1.0),
            SceneDim::Auto => {
                let v = if axis == Axis::X {
                    content.0
                } else {
                    content.1
                };
                (v, 0.0)
            }
        };
        let cross = match cross_d {
            SceneDim::Length(v) => v.max(0.0),
            SceneDim::Percent(p) => {
                let d = if axis == Axis::X { inner.1 } else { inner.0 };
                d * p.clamp(0.0, 1.0)
            }
            // Fill/Auto: гипотеза — контент; Fill stretch-ится до линии
            // на размещении.
            SceneDim::Fill | SceneDim::Auto => {
                if axis == Axis::X {
                    content.1
                } else {
                    content.0
                }
            }
        };
        FlexChild { basis, cross, grow }
    }

    /// Раскладка flex-детей узла idx: гипотезы → линии → CSS §9.7 →
    /// justify/align → рекурсия в детей с definite-целями + stretch.
    #[allow(clippy::too_many_arguments)]
    fn layout_flex_children(
        &mut self,
        idx: usize,
        axis: Axis,
        gap: f32,
        main: MainAlign,
        cross: CrossAlign,
        wrap: bool,
        inner: (f32, f32),
        root_inner: (f32, f32),
    ) {
        let flow: Vec<usize> = self.nodes[idx].flow.clone();
        // Геометрический (w, h) → осевой (main, cross) контейнера.
        let inner_mc = if axis == Axis::X {
            (inner.0, inner.1)
        } else {
            (inner.1, inner.0)
        };
        let hypos: Vec<FlexChild> = flow
            .iter()
            .map(|&ci| self.child_hypo(ci, axis, inner_mc))
            .collect();
        let (placed, _, line_cross_of) =
            layout_flex_container(axis, gap, main, cross, wrap, inner_mc, &hypos);
        for (k, &ci) in flow.iter().enumerate() {
            let p = placed[k];
            let node = self.nodes[ci].node.clone();
            let cross_d = if axis == Axis::X {
                node.size.h
            } else {
                node.size.w
            };
            let size_main = p.size_main;
            let mut size_cross = p.size_cross;
            // Stretch поперечной (Fill): до линии (line_cross_of[k]).
            if matches!(cross_d, SceneDim::Fill) {
                size_cross = line_cross_of[k];
            }
            let (x, y) = match axis {
                Axis::X => (p.main, p.cross),
                Axis::Y => (p.cross, p.main),
            };
            let (w, h) = match axis {
                Axis::X => (size_main, size_cross),
                Axis::Y => (size_cross, size_main),
            };
            self.nodes[ci].loc = (x, y);
            // Рекурсия: поддерево ребёнка от его definite inner.
            self.layout_node(ci, Some((w, h)), (w, h), root_inner);
        }
    }

    /// Раскладка grid-детей: треки (Length/Percent/Fill; FR-074 — Auto по
    /// контенту span-1 ячеек, `minmax(min, max)`), span, definite/Auto
    /// высоты строк. Ячейки тянутся в область трека (default stretch
    /// taffy). Алгоритм треков — модульная дока «Grid-треки» (CSS §7.5
    /// в упрощении: один проход, fr = 1, пол вместо итераций).
    fn layout_grid_children(
        &mut self,
        idx: usize,
        cols: &[SceneTrack],
        row_h: &SceneDim,
        gap: &UiVec2,
        inner: (f32, f32),
        root_inner: (f32, f32),
    ) {
        let gap_x = gap.x.max(0.0);
        let gap_y = gap.y.max(0.0);
        let n_cols = cols.len();
        let flow: Vec<usize> = self.nodes[idx].flow.clone();
        let spans = flow
            .iter()
            .map(|&ci| (self.nodes[ci].node.span as usize).max(1));
        // 1. Плейсмент (единый оракул с measure — FR-074).
        let placements = grid_placements(spans, n_cols);

        // 2. Контент-вклады треков: max max-content ширин span-1 ячеек
        //    (span>1 авторасчётные треки не растят — докурировано).
        let mut content_w: Vec<f32> = vec![0.0; n_cols];
        for (k, &ci) in flow.iter().enumerate() {
            let (col, _, span) = placements[k];
            if span == 1 && col < n_cols {
                let cw = self.measure_content(ci).0;
                content_w[col] = content_w[col].max(cw);
            }
        }

        // 3. База/лимит/вид трека (CSS §7.5-спецификации в терминах
        //    движка; контент = max-content span-1 ячеек):
        //      Length/Percent  — base = limit = definite;
        //      Auto            — base = limit = контент (интринсик);
        //      Fill            — base = 0, limit = ∞, вид Fr;
        //      MinMax{min,max} — base = min (definite/контент), limit =
        //                        max definite | контент (max Auto) | ∞
        //                        (max Fill, вид Fr).
        enum Kind {
            Fixed,
            AutoMax,
            Fr,
        }
        let mut bases: Vec<f32> = Vec::with_capacity(n_cols);
        let mut limits: Vec<f32> = Vec::with_capacity(n_cols);
        let mut kinds: Vec<Kind> = Vec::with_capacity(n_cols);
        for (i, t) in cols.iter().enumerate() {
            match t {
                SceneTrack::Length(v) => {
                    let v = v.max(0.0);
                    bases.push(v);
                    limits.push(v);
                    kinds.push(Kind::Fixed);
                }
                SceneTrack::Percent(p) => {
                    let v = inner.0 * p.clamp(0.0, 1.0);
                    bases.push(v);
                    limits.push(v);
                    kinds.push(Kind::Fixed);
                }
                SceneTrack::Auto => {
                    bases.push(content_w[i]);
                    limits.push(content_w[i]);
                    kinds.push(Kind::AutoMax);
                }
                SceneTrack::Fill => {
                    bases.push(0.0);
                    limits.push(f32::INFINITY);
                    kinds.push(Kind::Fr);
                }
                SceneTrack::MinMax { min, max } => {
                    let min_size = match min {
                        TrackMin::Length(v) => v.max(0.0),
                        TrackMin::Percent(p) => inner.0 * p.clamp(0.0, 1.0),
                        TrackMin::Auto => content_w[i],
                    };
                    match max {
                        TrackMax::Length(m) => {
                            bases.push(min_size);
                            limits.push(min_size.max(m.max(0.0)));
                            kinds.push(Kind::Fixed);
                        }
                        TrackMax::Percent(p) => {
                            let m = inner.0 * p.clamp(0.0, 1.0);
                            bases.push(min_size);
                            limits.push(min_size.max(m));
                            kinds.push(Kind::Fixed);
                        }
                        TrackMax::Auto => {
                            // Лимит ≥ базы (min Length может превысить
                            // max-content контента — CSS клампит).
                            bases.push(min_size);
                            limits.push(min_size.max(content_w[i]));
                            kinds.push(Kind::AutoMax);
                        }
                        TrackMax::Fill => {
                            bases.push(min_size);
                            limits.push(f32::INFINITY);
                            kinds.push(Kind::Fr);
                        }
                    }
                }
            }
        }
        let gap_total = gap_x * n_cols.saturating_sub(1) as f32;
        // 4. CSS 11.6 «Maximise Tracks»: свободное место (inner − Σ баз −
        //    зазоры) распределяется ПОРОВНУ трекам с конечным лимитом,
        //    с заморозкой достигших лимита (итеративно; fr с бесконечным
        //    лимитом не участвует).
        {
            let mut free = inner.0 - bases.iter().sum::<f32>() - gap_total;
            if free.is_finite() && free > 0.0 {
                // Заморожены: Fr (taffy resolve_intrinsic_track_sizes
                // step 5: бесконечный лимит fr = base — «Maximise не
                // трогает гибкие треки») и достигшие лимита (Fixed —
                // limit == base — сюда же).
                let mut frozen: Vec<bool> = limits
                    .iter()
                    .enumerate()
                    .map(|(i, l)| matches!(kinds[i], Kind::Fr) || *l <= bases[i])
                    .collect();
                loop {
                    let open: Vec<usize> = (0..n_cols).filter(|&i| !frozen[i]).collect();
                    if open.is_empty() || free <= f32::EPSILON {
                        break;
                    }
                    let share = free / open.len() as f32;
                    let mut all_frozen = true;
                    for &i in &open {
                        let room = limits[i] - bases[i];
                        let take = share.min(room);
                        bases[i] += take;
                        free -= take;
                        if take < room - f32::EPSILON {
                            all_frozen = false;
                        } else {
                            frozen[i] = true;
                        }
                    }
                    if all_frozen {
                        break;
                    }
                }
            }
        }
        // 5. CSS 11.7 «Expand Flexible Tracks» (taffy find_size_of_fr,
        //    факторы все = 1): fr = наибольший размер, при котором
        //    Σ max(base, fr) по fr-трекам ≤ inner; треки, чья база больше
        //    fr, остаются на базе (переполнение видно — G4).
        {
            let fr_count = kinds.iter().filter(|k| matches!(k, Kind::Fr)).count();
            if fr_count > 0 {
                let mut fr = f32::INFINITY;
                for _ in 0..=fr_count {
                    let mut used = gap_total;
                    let mut flex_sum = 0.0f32;
                    for (i, k) in kinds.iter().enumerate() {
                        match k {
                            Kind::Fr if bases[i] <= fr || fr.is_infinite() => flex_sum += 1.0,
                            _ => used += bases[i],
                        }
                    }
                    fr = (inner.0 - used) / flex_sum.max(1.0);
                    let consistent = kinds
                        .iter()
                        .enumerate()
                        .all(|(i, k)| !matches!(k, Kind::Fr) || bases[i] <= fr || fr <= 0.0);
                    if consistent {
                        break;
                    }
                }
                for (i, k) in kinds.iter().enumerate() {
                    if matches!(k, Kind::Fr) {
                        bases[i] = bases[i].max(fr);
                    }
                }
            }
        }
        // 6. CSS 11.8 «Stretch auto Tracks» (taffy default STRETCH —
        //    модульная докa): остаток делится поровну между AutoMax-
        //    треками (поверх лимитов — как в браузерах по умолчанию).
        {
            let leftover = inner.0 - bases.iter().sum::<f32>() - gap_total;
            let auto_n = kinds.iter().filter(|k| matches!(k, Kind::AutoMax)).count();
            if leftover > 0.0 && auto_n > 0 {
                let each = leftover / auto_n as f32;
                for (i, k) in kinds.iter().enumerate() {
                    if matches!(k, Kind::AutoMax) {
                        bases[i] += each;
                    }
                }
            }
        }
        let widths = bases;
        // 7. Позиции треков (накопление слева направо — как taffy).
        let mut track_x: Vec<f32> = Vec::with_capacity(n_cols);
        let mut x = 0.0f32;
        for &w in &widths {
            track_x.push(x);
            x += w + gap_x;
        }
        // 6. Высоты строк: definite (Length/Percent) или контент ячеек.
        let n_rows = placements.iter().map(|&(_, r, _)| r + 1).max().unwrap_or(0);
        let mut heights: Vec<f32> = vec![0.0; n_rows.max(1)];
        match row_h {
            SceneDim::Length(v) => {
                for hh in heights.iter_mut() {
                    *hh = v.max(0.0);
                }
            }
            SceneDim::Percent(p) => {
                let v = inner.1 * p.clamp(0.0, 1.0);
                for hh in heights.iter_mut() {
                    *hh = v;
                }
            }
            SceneDim::Fill | SceneDim::Auto => {
                for (k, &ci) in flow.iter().enumerate() {
                    let (_, row, _) = placements[k];
                    let ch = self.measure_content(ci).1;
                    heights[row] = heights[row].max(ch);
                }
            }
        }
        // 8. Размещение ячеек: loc = (track_x, Σ предыдущих строк+зазоры);
        //    размер ячейки — CSS-семантика grid-item: Length — definite
        //    (старт-выравнена в области, переполнение видно), Percent —
        //    доля области, Fill/Auto — stretch на всю область (default
        //    justify-self: stretch). Рекурсия в поддерево от размера
        //    ячейки (percent потомков — от неё же).
        for (k, &ci) in flow.iter().enumerate() {
            let (col, row, span) = placements[k];
            let track_w = |c: usize| widths.get(c).copied().unwrap_or(0.0);
            let mut area_w = track_w(col);
            for s in 1..span {
                area_w += track_w(col + s) + gap_x;
            }
            let area_h = heights[row];
            let mut y = 0.0f32;
            // Тот же порядок операций, что у наивного накопления (биты).
            for &h in heights[..row].iter() {
                y += h + gap_y;
            }
            let cell_node = &self.nodes[ci].node;
            let cw = match cell_node.size.w {
                SceneDim::Length(v) => v.max(0.0),
                SceneDim::Percent(p) => area_w * p.clamp(0.0, 1.0),
                SceneDim::Fill | SceneDim::Auto => area_w,
            };
            let ch = match cell_node.size.h {
                SceneDim::Length(v) => v.max(0.0),
                SceneDim::Percent(p) => area_h * p.clamp(0.0, 1.0),
                SceneDim::Fill | SceneDim::Auto => area_h,
            };
            self.nodes[ci].loc = (track_x.get(col).copied().unwrap_or(0.0), y);
            self.nodes[ci].size = (cw, ch);
            // Aspect-листья: derive внутри layout_node по definite осям.
            self.layout_node(ci, Some((cw, ch)), (cw, ch), root_inner);
        }
    }

    /// round-layout pass + абсолютные координаты (зеркало
    /// `taffy::compute::round_layout` + чтение taffy_backend):
    /// abs = abs(parent) + round(loc_unrounded); размер — разность
    /// округлённых краёв по кумулятивной unrounded-координате от корня.
    fn round_and_absolve(&self, slot: UiRect) -> Vec<UiRect> {
        let mut rects = vec![UiRect::default(); self.nodes.len()];
        let mut cum: Vec<(f32, f32)> = vec![(0.0, 0.0); self.nodes.len()];
        for (i, e) in self.nodes.iter().enumerate() {
            let (cx, cy) = match e.parent {
                None => (e.loc.0, e.loc.1),
                Some(p) => (cum[p].0 + e.loc.0, cum[p].1 + e.loc.1),
            };
            cum[i] = (cx, cy);
            let (ax, ay) = match e.parent {
                None => (slot.x + taffy_round(e.loc.0), slot.y + taffy_round(e.loc.1)),
                Some(p) => (
                    rects[p].x + taffy_round(e.loc.0),
                    rects[p].y + taffy_round(e.loc.1),
                ),
            };
            let w = taffy_round(cx + e.size.0) - taffy_round(cx);
            let h = taffy_round(cy + e.size.1) - taffy_round(cy);
            rects[i] = UiRect::new(ax, ay, w, h);
        }
        rects
    }
}
// =============================================================================
// Пост-обработка: content-shift + sticky (формулы taffy_backend 1:1)
// =============================================================================

/// Content-shift scroll-предков + sticky-кламп (taffy_backend.rs 1:1;
/// FR-074 — sticky по ОБОИМ осям: top против Y-scroll-предка, left против
/// X-scroll-предка, кламп двигает узел и всё его поддерево).
fn post_process(tree: &SceneTree, mut rects: Vec<UiRect>) -> Vec<UiRect> {
    let nodes = &tree.nodes;
    // Content-shift: сумма offset'ов scroll-предков по их главным осям
    // (fixed-узлы прицеплены к корню — shift'а не получают, как CSS
    // position:fixed внутри scroll-контейнера).
    for i in 1..nodes.len() {
        if matches!(nodes[i].position, ScenePosition::Fixed { .. }) {
            continue;
        }
        let (sx, sy) = shift_for(nodes, i);
        if sx != 0.0 || sy != 0.0 {
            rects[i].x -= sx;
            rects[i].y -= sy;
        }
    }
    // Sticky-кламп по ближайшему scroll-предку СООТВЕТСТВУЮЩЕЙ ОСИ
    // (после shift'ов); кламп двигает узел и ВСЁ его поддерево;
    // fixed-потомки исключены.
    for i in 1..nodes.len() {
        if let ScenePosition::Sticky { top, left } = nodes[i].position {
            // Вертикаль: Y-scroll-предок (Column/Grid).
            if let (Some(top), Some(ai)) = (top, scroll_ancestor(nodes, i, Axis::Y)) {
                let off = nodes[ai].offset;
                let target = rects[ai].y + top;
                if off > 0.0 && rects[i].y < target {
                    let dy = target - rects[i].y;
                    rects[i].y = target;
                    translate_subtree(nodes, &mut rects, i, 0.0, dy);
                }
            }
            // Горизонталь (FR-074): X-scroll-предок (Row).
            if let (Some(left), Some(ai)) = (left, scroll_ancestor(nodes, i, Axis::X)) {
                let off = nodes[ai].offset;
                let target = rects[ai].x + left;
                if off > 0.0 && rects[i].x < target {
                    let dx = target - rects[i].x;
                    rects[i].x = target;
                    translate_subtree(nodes, &mut rects, i, dx, 0.0);
                }
            }
        }
    }
    rects
}

/// Трансляция поддерева узла `i` на (dx, dy) (fixed-потомки исключены —
/// viewport-контекст).
fn translate_subtree(nodes: &[SceneEntry], rects: &mut [UiRect], i: usize, dx: f32, dy: f32) {
    for j in i + 1..nodes.len() {
        if !is_descendant_of(nodes, j, i) {
            continue;
        }
        if matches!(nodes[j].position, ScenePosition::Fixed { .. }) {
            continue;
        }
        rects[j].x += dx;
        rects[j].y += dy;
    }
}

/// Суммарный content-shift предков (по их главным осям) для узла `i`.
/// Fixed-предок ОБРЫВАЕТ цепочку (viewport-контекст).
fn shift_for(nodes: &[SceneEntry], i: usize) -> (f32, f32) {
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut p = nodes[i].parent;
    while let Some(pidx) = p {
        let a = &nodes[pidx];
        if matches!(a.position, ScenePosition::Fixed { .. }) {
            break;
        }
        if a.offset > 0.0 {
            match a.axis {
                Axis::X => sx += a.offset,
                Axis::Y => sy += a.offset,
            }
        }
        p = a.parent;
    }
    (sx, sy)
}

/// Ближайший scroll-предок вдоль оси `axis` (для sticky-клампа; fixed
/// обрывает цепочку). FR-074: ось-зависимо — вертикальный sticky ищет
/// Y-контейнер (Column/Grid), горизонтальный — X (Row).
fn scroll_ancestor(nodes: &[SceneEntry], i: usize, axis: Axis) -> Option<usize> {
    let mut p = nodes[i].parent;
    while let Some(pidx) = p {
        if matches!(nodes[pidx].position, ScenePosition::Fixed { .. }) {
            return None;
        }
        if nodes[pidx].offset > 0.0 && nodes[pidx].axis == axis {
            return Some(pidx);
        }
        p = nodes[pidx].parent;
    }
    None
}

/// Является ли узел `j` потомком узла `i` (по цепочке parent).
fn is_descendant_of(nodes: &[SceneEntry], j: usize, i: usize) -> bool {
    let mut p = nodes[j].parent;
    while let Some(pidx) = p {
        if pidx == i {
            return true;
        }
        if pidx < i {
            return false;
        }
        p = nodes[pidx].parent;
    }
    false
}
