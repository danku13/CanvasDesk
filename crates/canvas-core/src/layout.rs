//! FR-010 v2: авто-раскладка связанных карточек — Sugiyama-lite над
//! моделью канваса (без GPU/ОС, тестируется везде; wasm-гейт ADR-0011).
//!
//! **v1 → v2.** v1 — BFS со знаковой глубиной: каждая нода получала ОДИН
//! уровень (первый BFS-визит), что ломало DAG (merge-point с двумя путями
//! терял топологию), не минимизировало пересечения рёбер с нодами и не
//! группировало смысловые соседи. v2 переиспользует движок Sugiyama-lite
//! из FR-071 (`scheme_layout.rs`): знаковое longest-path слоение (DAG
//! корректен — merge-point получает максимальный уровень по всем путям),
//! barycenter-упорядочивание (смысловая группировка), минимизация
//! пересечений «ребро × нода» (swap + shift + insertion). Геометрия
//! переиспользуется из `scheme_layout` (`right_center`/`left_center`/
//! `seg_len`/`seg_intersects_rect`).
//!
//! **Контракт сохранён** (образец v1): `plan_related_layout(canvas, seed,
//! mode) -> LayoutPlan`, 3 режима `LayoutMode`, семя-якорь (не двигается),
//! группы едут с детьми, ноды без связи с семенем не трогаются.
//!
//! **Слоение со знаком.** Из семени строятся две половины графа: прямая
//! (по исходящим рёбрам — потомки) и обратная (по входящим — предки).
//! longest-path (Kahn, образец `scheme_layout::layout_cluster`) даёт
//! `forward_level` (≥0, путь от семени) и `backward_level` (≥0, путь к
//! семени). Знаковый уровень = `forward_level − backward_level`: потомки
//! на +1..+N (справа/снизу), предки на −1..−N (слева/сверху), семя = 0.
//! DAG merge-point: нода с двумя путями от семени получает максимум
//! (топология сохранена). Циклы — детерминированный фолбэк (Kahn-остаток).
//!
//! **Radial v2.** v1 делил кольца между родителями и потомками по
//! `|level|` — наложение. v2 разводит их по половинам: потомки (level>0) —
//! на правой полуокружности (−90°…+90°), предки (level<0) — на левой
//! (+90°…+270°); радиус = `|level|·RADIAL_RING_STEP`. Конвенция потока
//! слева→справа сохранена, наложения устранены.
//!
//! **Минимизация пересечений** (TreeH/TreeV): swap соседей в колонке +
//! вертикальные сдвиги колонок целиком + переносы на другую строку
//! (образец `scheme_layout::refine_*`); ход — строгое падение стоимости
//! (пересечения, тай-брейк — суммарная длина отрезков). Колонка семени
//! (level 0) иммунна к сдвигам — якорь. Radial — угловой barycenter
//! (эстетика), без swap/shift (кольца, не сетка).
//!
//! Детерминизм (образец v1/FR-071): сортировка по `id`/индексу, стабильные
//! сортировки, никаких итераций по `HashMap` в порядковых решениях.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::model::{group_children, Canvas, NodeKind};
use crate::scheme_layout::{seg_intersects_rect, seg_len};

/// Схема раскладки связанных карточек (уточнение владельца FR-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Уровни — колонками: родители слева от семени, дети справа.
    TreeHorizontal,
    /// Уровни — строками: родители сверху, дети снизу.
    TreeVertical,
    /// Корень в центре, уровни — по кругу (радиус по модулю уровня;
    /// родители — левая полуокружность, дети — правая).
    Radial,
}

/// Зазор между колонками/строками уровней (world px).
pub const LEVEL_GAP: f32 = 80.0;
/// Зазор между соседями одного уровня (world px).
pub const SIBLING_GAP: f32 = 32.0;
/// Шаг радиуса за уровень глубины в radial-режиме (world px).
pub const RADIAL_RING_STEP: f32 = 260.0;

/// Инфляция bbox нод при подсчёте пересечений «ребро × нода» (world px) —
/// образец `scheme_layout::CROSSING_MARGIN` / `edgegeom::AVOID_MARGIN`.
const CROSSING_MARGIN: f32 = 12.0;
/// Максимум проходов swap/insertion-оптимизации пересечений.
const MAX_REFINE_PASSES: usize = 8;
/// Максимум проходов оптимизации сдвигами колонок.
const MAX_SHIFT_PASSES: usize = 2;
/// Диапазон сдвига колонки в шагах `SIBLING_GAP` (±4 шага).
const SHIFT_STEPS: i32 = 4;

/// План раскладки: целевые top-left позиции нод (индекс → позиция).
/// Семя в план не входит (остаётся на месте — якорь).
pub type LayoutPlan = Vec<(usize, [f32; 2])>;

/// Прямоугольник [x, y, width, height] — образец `scheme_layout::Rect`.
type Rect = [f32; 4];

/// Построить план авто-раскладки связанной окрестности `seed` (Sugiyama-lite).
/// Ноды без связи с семенем не затрагиваются; невалидный seed/изолированная
/// нода — пустой план. Детерминирован: обход, слоение и порядок внутри
/// уровней фиксированы (сортировка по `id`/индексу, стабильные сортировки).
pub fn plan_related_layout(canvas: &Canvas, seed: usize, mode: LayoutMode) -> LayoutPlan {
    let Some(seed_node) = canvas.nodes.get(seed) else {
        return Vec::new();
    };

    // id → индекс (для рёбер; висячие концы игнорируются)
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Направленные списки смежности: out — исходящие рёбра (дети),
    // in — входящие (родители). Сортировка по id — детерминизм.
    let mut outgoing: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut incoming: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for edge in &canvas.edges {
        let (Some(from), Some(to)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if from == to {
            continue;
        }
        outgoing.entry(*from).or_default().insert(*to);
        incoming.entry(*to).or_default().insert(*from);
    }

    // --- Достижимость от семени (оба направления): потомки + предки.
    // BFS по ненаправленному графу (по рёбрам обеих сторон) — кто достижим.
    let reachable: BTreeSet<usize> = {
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut q: VecDeque<usize> = VecDeque::new();
        seen.insert(seed);
        q.push_back(seed);
        while let Some(u) = q.pop_front() {
            for &w in outgoing.get(&u).into_iter().flatten() {
                if seen.insert(w) {
                    q.push_back(w);
                }
            }
            for &w in incoming.get(&u).into_iter().flatten() {
                if seen.insert(w) {
                    q.push_back(w);
                }
            }
        }
        seen
    };
    // Изолированное семя (без связей) — раскладывать нечего
    if reachable.len() == 1 {
        return Vec::new();
    }

    // --- Знаковое longest-path слоение относительно семени.
    // forward_level: longest path ОТ семени по исходящим (потомки, ≥0).
    // backward_level: longest path К семени по входящим, т.е. от семени по
    // обращённому графу (предки, ≥0). Знаковый уровень = f − b.
    let forward_level = longest_path_from(seed, &reachable, &outgoing);
    let backward_level = longest_path_from(seed, &reachable, &incoming);
    let mut level: HashMap<usize, i32> = HashMap::new();
    for &v in &reachable {
        let f = forward_level.get(&v).copied().unwrap_or(0);
        let b = backward_level.get(&v).copied().unwrap_or(0);
        level.insert(v, f as i32 - b as i32);
    }
    // Семя — гарантированно 0 (f=0, b=0)
    level.insert(seed, 0);

    // --- Колонки: знаковый уровень → порядок. Seed-порядок — текущие
    // координаты (ручные подсказки пользователя), тай-брейк по id.
    let mut columns: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for &v in &reachable {
        columns.entry(level[&v]).or_default().push(v);
    }
    for col in columns.values_mut() {
        col.sort_by(|&a, &b| seed_order(canvas, a, b));
    }

    // --- Barycenter: 2 свипа (по предкам, по потомкам) — ноды с общим
    // родителем собираются рядом (смысловая группировка).
    for _ in 0..2 {
        barycenter_pass(&reachable, &incoming, &mut columns);
        barycenter_pass(&reachable, &outgoing, &mut columns);
    }

    let (seed_cx, seed_cy) = (
        seed_node.x + seed_node.width / 2.0,
        seed_node.y + seed_node.height / 2.0,
    );

    // --- Позиции по режиму. Семя — якорь (не в плане, не двигается).
    let mut target: HashMap<usize, [f32; 2]> = HashMap::new();
    let mut rects: HashMap<usize, Rect> = HashMap::new();
    match mode {
        LayoutMode::TreeHorizontal => {
            place_tree_columns(
                canvas,
                seed,
                &columns,
                seed_cy,
                true,
                &mut target,
                &mut rects,
            );
            refine_crossings(&reachable, &outgoing, true, &mut columns, &mut rects);
            sync_targets(&rects, &mut target);
        }
        LayoutMode::TreeVertical => {
            place_tree_columns(
                canvas,
                seed,
                &columns,
                seed_cx,
                false,
                &mut target,
                &mut rects,
            );
            refine_crossings(&reachable, &outgoing, false, &mut columns, &mut rects);
            sync_targets(&rects, &mut target);
        }
        LayoutMode::Radial => {
            place_radial(
                canvas,
                seed,
                &columns,
                seed_cx,
                seed_cy,
                &mut target,
                &mut rects,
            );
        }
    }

    // --- Группы в плане едут вместе с геометрическими детьми (translate_group
    // без мутации): дельта группы распространяется на детей, не вошедших в
    // план по рёбрам. Вложенные группы — очередью, каждая нода один раз.
    let mut queue: VecDeque<usize> = target.keys().copied().collect();
    while let Some(node) = queue.pop_front() {
        if canvas.nodes.get(node).map(|n| n.kind()) != Some(NodeKind::Group) {
            continue;
        }
        let (Some(old), Some(&new)) = (
            canvas.nodes.get(node).map(|n| [n.x, n.y]),
            target.get(&node),
        ) else {
            continue;
        };
        let delta = [new[0] - old[0], new[1] - old[1]];
        for child in group_children(canvas, node) {
            if target.contains_key(&child) || child == seed {
                continue;
            }
            let Some(c) = canvas.nodes.get(child) else {
                continue;
            };
            target.insert(child, [c.x + delta[0], c.y + delta[1]]);
            queue.push_back(child);
        }
    }

    // Стабильный порядок применения — по индексу ноды
    let mut plan: LayoutPlan = target.into_iter().collect();
    plan.sort_by_key(|(index, _)| *index);
    plan
}

// ===========================================================================
// Слоение (longest-path от семени, Kahn + фолбэк для циклов)
// ===========================================================================

/// Longest-path от `seed` по рёбрам `adj` (исходящие для потомков, входящие
/// для предков): `level[seed]=0`, `level[v]=max(level[u]+1)` по всем
/// рёбрам `u→v` внутри `verts`. Циклы — детерминированный фолбэк (образец
/// `scheme_layout::layout_cluster`). Возвращает уровень только достижимых
/// от seed вершин (остальных нет в карте).
fn longest_path_from(
    seed: usize,
    verts: &BTreeSet<usize>,
    adj: &BTreeMap<usize, BTreeSet<usize>>,
) -> HashMap<usize, usize> {
    // Рёбра подграфа (оба конца в verts)
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for &u in verts {
        if let Some(ts) = adj.get(&u) {
            for &v in ts {
                if verts.contains(&v) {
                    edges.push((u, v));
                }
            }
        }
    }
    // Направленная достижимость от семени (BFS по adj) — только эти вершины
    // получают уровень. Вершины вне достижимости (например, предок `a` при
    // прямом обходе от семени-потомка) НЕ попадают в карту — их знак
    // определяется другой половиной (backward_level).
    let mut directed_reach: BTreeSet<usize> = BTreeSet::new();
    directed_reach.insert(seed);
    let mut rq: VecDeque<usize> = VecDeque::new();
    rq.push_back(seed);
    while let Some(u) = rq.pop_front() {
        if let Some(ts) = adj.get(&u) {
            for &w in ts {
                if verts.contains(&w) && directed_reach.insert(w) {
                    rq.push_back(w);
                }
            }
        }
    }
    if directed_reach.len() == 1 {
        // Только семя достижимо в этом направлении — никаких потомков/предков
        return HashMap::from([(seed, 0)]);
    }

    // in-degree внутри направленно-достижимого подграфа
    let mut in_deg: HashMap<usize, usize> = directed_reach.iter().map(|&v| (v, 0usize)).collect();
    for &(_, v) in &edges {
        if directed_reach.contains(&v) {
            if let Some(d) = in_deg.get_mut(&v) {
                *d += 1;
            }
        }
    }

    let mut level: HashMap<usize, usize> = HashMap::new();
    let mut processed: BTreeSet<usize> = BTreeSet::new();
    let mut queued: BTreeSet<usize> = BTreeSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();

    // Семя — стартовый слой 0. Если у семени есть входящие рёбра внутри
    // подграфа (цикл через семя), принудительно разрываем: семя всё равно
    // корень раскладки (пути считаются ОТ него).
    level.insert(seed, 0);
    in_deg.insert(seed, 0);
    queued.insert(seed);
    queue.push_back(seed);

    while let Some(u) = queue.pop_front() {
        processed.insert(u);
        let lu = level.get(&u).copied().unwrap_or(0);
        if let Some(ts) = adj.get(&u) {
            for &w in ts {
                if !directed_reach.contains(&w) {
                    continue;
                }
                let lw = level.get(&w).copied().unwrap_or(0).max(lu + 1);
                level.insert(w, lw);
                if let Some(d) = in_deg.get_mut(&w) {
                    *d = d.saturating_sub(1);
                    if *d == 0 && !queued.contains(&w) {
                        queued.insert(w);
                        queue.push_back(w);
                    }
                }
            }
        }
    }

    // Фолбэк для циклов (достижимые, но вне Кана): слой = max(слои
    // назначенных предков) + 1; чистый цикл — минимальная вершина (FR-071).
    let mut pending: BTreeSet<usize> = directed_reach
        .iter()
        .copied()
        .filter(|&v| !processed.contains(&v))
        .collect();
    // предки по adj (для фолбэка нужен обратный обход: кто указывает на v)
    let rev: HashMap<usize, Vec<usize>> = {
        let mut m: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(u, v) in &edges {
            m.entry(v).or_default().push(u);
        }
        m
    };
    while !pending.is_empty() {
        let mut to_assign: Vec<(usize, usize)> = Vec::new();
        for &v in &pending {
            let preds: Vec<usize> = rev
                .get(&v)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&p| directed_reach.contains(&p))
                .collect();
            if preds.iter().all(|&p| processed.contains(&p)) {
                let lv = preds
                    .iter()
                    .map(|&p| level.get(&p).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0)
                    + 1;
                to_assign.push((v, lv));
            }
        }
        if to_assign.is_empty() {
            if let Some(&v) = pending.iter().next() {
                let lv = rev
                    .get(&v)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|&p| directed_reach.contains(&p))
                    .map(|p| level.get(&p).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0)
                    + 1;
                to_assign.push((v, lv));
            }
        }
        if to_assign.is_empty() {
            break;
        }
        for (v, lv) in to_assign {
            level.insert(v, lv);
            processed.insert(v);
            pending.remove(&v);
        }
    }

    level
}

// ===========================================================================
// Barycenter (образец scheme_layout::barycenter_pass)
// ===========================================================================

/// Один barycenter-проход: упорядочить каждую колонку по среднему индексу
/// соседей (предков для `incoming`, потомков для `outgoing`). Ноды без
/// соседей сохраняют позицию (stable sort по (bary, старый индекс)).
fn barycenter_pass(
    verts: &BTreeSet<usize>,
    neighbors: &BTreeMap<usize, BTreeSet<usize>>,
    columns: &mut BTreeMap<i32, Vec<usize>>,
) {
    let mut pos: HashMap<usize, usize> = HashMap::new();
    for col in columns.values() {
        for (k, &v) in col.iter().enumerate() {
            pos.insert(v, k);
        }
    }
    let keys: Vec<(i32, Vec<(f32, usize)>)> = columns
        .iter()
        .map(|(&lv, col)| {
            let keyed = col
                .iter()
                .enumerate()
                .map(|(k, &v)| {
                    let (mut sum, mut cnt) = (0.0f32, 0usize);
                    if let Some(ns) = neighbors.get(&v) {
                        for &n in ns {
                            if !verts.contains(&n) {
                                continue;
                            }
                            if let Some(&p) = pos.get(&n) {
                                sum += p as f32;
                                cnt += 1;
                            }
                        }
                    }
                    if cnt == 0 {
                        (k as f32, k)
                    } else {
                        (sum / cnt as f32, k)
                    }
                })
                .collect();
            (lv, keyed)
        })
        .collect();
    for (lv, keyed) in keys {
        let Some(col) = columns.get_mut(&lv) else {
            continue;
        };
        let mut paired: Vec<(f32, usize, usize)> = col
            .iter()
            .zip(keyed)
            .map(|(&v, (bary, old_k))| (bary, old_k, v))
            .collect();
        paired.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        });
        *col = paired.into_iter().map(|(_, _, v)| v).collect();
    }
}

// ===========================================================================
// Позиционирование колонок (TreeHorizontal / TreeVertical) + Radial
// ===========================================================================

/// Расставить колонки деревом: `horizontal=true` — уровни по X (колонки),
/// ряды по Y; `false` — уровни по Y (строки), ряды по X. Семя (колонка 0) —
/// якорь: его координаты не меняются. Положительные уровни — от семени в
/// сторону роста, отрицательные — в обратную; колонки центрируются на
/// `anchor_cross` (seed_cy для H, seed_cx для V) либо на среднем центре
/// размещённых предков (компактный поток, образец FR-071).
fn place_tree_columns(
    canvas: &Canvas,
    seed: usize,
    columns: &BTreeMap<i32, Vec<usize>>,
    anchor_cross: f32,
    horizontal: bool,
    target: &mut HashMap<usize, [f32; 2]>,
    rects: &mut HashMap<usize, Rect>,
) {
    let seed_node = &canvas.nodes[seed];
    let max_level = *columns.keys().max().unwrap_or(&0);
    let min_level = *columns.keys().min().unwrap_or(&0);

    // Кумулятивные смещения уровней по главной оси.
    // Положительные — от семени в сторону роста, отрицательные — обратно.
    let mut offsets: HashMap<i32, f32> = HashMap::new();
    if horizontal {
        let mut x_right = seed_node.x + seed_node.width + LEVEL_GAP;
        for lv in 1..=max_level {
            offsets.insert(lv, x_right);
            let w = col_extent(canvas, columns.get(&lv), true);
            x_right += w + LEVEL_GAP;
        }
        let mut x_left = seed_node.x - LEVEL_GAP;
        for lv in (min_level..=-1).rev() {
            x_left -= col_extent(canvas, columns.get(&lv), true);
            offsets.insert(lv, x_left);
            x_left -= LEVEL_GAP;
        }
    } else {
        let mut y_down = seed_node.y + seed_node.height + LEVEL_GAP;
        for lv in 1..=max_level {
            offsets.insert(lv, y_down);
            let h = col_extent(canvas, columns.get(&lv), false);
            y_down += h + LEVEL_GAP;
        }
        let mut y_up = seed_node.y - LEVEL_GAP;
        for lv in (min_level..=-1).rev() {
            y_up -= col_extent(canvas, columns.get(&lv), false);
            offsets.insert(lv, y_up);
            y_up -= LEVEL_GAP;
        }
    }

    // Расстановка: главная ось — offset[level], поперечная — стек с
    // SIBLING_GAP, центрированный на anchor_cross либо на среднем центре
    // размещённых предков (компактный поток).
    for (&lv, col) in columns {
        if lv == 0 {
            // Колонка семени: семя не двигается; прочие (цикл-0) — под семенем
            let mut y = seed_node.y + seed_node.height + SIBLING_GAP;
            let mut x = seed_node.x + seed_node.width + SIBLING_GAP;
            for &v in col {
                if v == seed {
                    continue;
                }
                let n = &canvas.nodes[v];
                let pos = if horizontal {
                    [seed_node.x, y]
                } else {
                    [x, seed_node.y]
                };
                target.insert(v, pos);
                rects.insert(v, [pos[0], pos[1], n.width, n.height]);
                if horizontal {
                    y += n.height + SIBLING_GAP;
                } else {
                    x += n.width + SIBLING_GAP;
                }
            }
            continue;
        }
        let main = offsets[&lv];
        // поперечная центровка: средний центр размещённых соседей (предков
        // для положительных уровней, потомков для отрицательных), иначе anchor
        let cross_target = avg_neighbor_cross(canvas, col, lv, rects, horizontal, anchor_cross);
        let total: f32 = col
            .iter()
            .map(|&v| {
                let n = &canvas.nodes[v];
                if horizontal {
                    n.height
                } else {
                    n.width
                }
            })
            .sum::<f32>()
            + SIBLING_GAP * col.len().saturating_sub(1) as f32;
        let mut cross = cross_target - total / 2.0;
        for &v in col {
            let n = &canvas.nodes[v];
            let pos = if horizontal {
                [main, cross]
            } else {
                [cross, main]
            };
            target.insert(v, pos);
            rects.insert(v, [pos[0], pos[1], n.width, n.height]);
            cross += if horizontal {
                n.height + SIBLING_GAP
            } else {
                n.width + SIBLING_GAP
            };
        }
    }
}

/// Максимальный размер колонки по главной оси (width для H, height для V).
fn col_extent(canvas: &Canvas, col: Option<&Vec<usize>>, horizontal: bool) -> f32 {
    col.into_iter()
        .flatten()
        .map(|&v| {
            let n = &canvas.nodes[v];
            if horizontal {
                n.width
            } else {
                n.height
            }
        })
        .fold(0.0f32, f32::max)
}

/// Средний центр соседей (предков для lvl>0, потомков для lvl<0) по
/// поперечной оси — для компактной центровки колонки. Нет размещённых —
/// `fallback` (anchor_cross).
fn avg_neighbor_cross(
    canvas: &Canvas,
    col: &[usize],
    lvl: i32,
    rects: &HashMap<usize, Rect>,
    horizontal: bool,
    fallback: f32,
) -> f32 {
    // Соседи по противоположному направлению: для потомков (lvl>0) — предки
    // (incoming), для предков (lvl<0) — потомки (outgoing). Это rebuild
    // без передачи adj — используем canvas.edges напрямую.
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let want_parent = lvl > 0;
    let mut centers: Vec<f32> = Vec::new();
    for &v in col {
        for e in &canvas.edges {
            let (Some(&a), Some(&b)) = (
                index_of.get(e.from_node.as_str()),
                index_of.get(e.to_node.as_str()),
            ) else {
                continue;
            };
            let (nb, me) = if want_parent { (a, b) } else { (b, a) };
            if me == v && rects.contains_key(&nb) {
                let r = rects[&nb];
                let c = if horizontal {
                    r[1] + r[3] / 2.0
                } else {
                    r[0] + r[2] / 2.0
                };
                centers.push(c);
            }
        }
    }
    if centers.is_empty() {
        fallback
    } else {
        centers.iter().sum::<f32>() / centers.len() as f32
    }
}

/// Radial: кольца по |level|, родители (level<0) — левая полуокружность
/// (+90°…+270°), потомки (level>0) — правая (−90°…+90°); семя — центр.
/// Угловой barycenter (порядок по среднему углу соседей) для эстетики.
fn place_radial(
    canvas: &Canvas,
    seed: usize,
    columns: &BTreeMap<i32, Vec<usize>>,
    seed_cx: f32,
    seed_cy: f32,
    target: &mut HashMap<usize, [f32; 2]>,
    rects: &mut HashMap<usize, Rect>,
) {
    // Раздельные кольца для родителей и потомков: ключ (|level|, side).
    // side: 0 — потомки (правая полуокружность), 1 — предки (левая).
    let mut rings: BTreeMap<(u32, u8), Vec<usize>> = BTreeMap::new();
    for (&lv, col) in columns {
        if lv == 0 {
            continue;
        }
        let side: u8 = if lv < 0 { 1 } else { 0 };
        rings
            .entry((lv.unsigned_abs(), side))
            .or_default()
            .extend(col);
    }

    // Угловой barycenter: упорядочить каждое кольцо по среднему углу соседей.
    // Соседи — по canvas.edges (ненаправленно, в пределах reachable).
    let placed_angle = |v: usize, rects: &HashMap<usize, Rect>| -> Option<f32> {
        let r = rects.get(&v)?;
        let cx = r[0] + r[2] / 2.0 - seed_cx;
        let cy = r[1] + r[3] / 2.0 - seed_cy;
        Some(cy.atan2(cx))
    };
    for _ in 0..2 {
        for ((depth, side), ring) in rings.clone() {
            let mut keyed: Vec<(f32, usize)> = ring
                .iter()
                .enumerate()
                .map(|(k, &v)| {
                    let ns = neighbors_of(canvas, v);
                    let mut sum = 0.0f32;
                    let mut cnt = 0usize;
                    for n in ns {
                        if let Some(a) = placed_angle(n, rects) {
                            sum += a;
                            cnt += 1;
                        }
                    }
                    let bary = if cnt == 0 { k as f32 } else { sum / cnt as f32 };
                    (bary, v)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            rings.insert((depth, side), keyed.into_iter().map(|(_, v)| v).collect());
        }
    }

    // Расстановка по полуокружностям с центрированием на середине стороны:
    // потомки — вокруг 0° (справа), предки — вокруг 180° (слева). Каждая
    // сторона занимает инсет-SPAN (< π) — гарантированный зазор на вертикалях
    // (±90°), ноды разных сторон не сливаются. Одиночная нода — в центре
    // стороны (0° / 180°), не на границе.
    let _ = seed; // семя не двигается (не в target)
    const SPAN: f32 = std::f32::consts::PI * 0.84; // инсет ~8% с каждого края
    for ((depth, side), ring) in &rings {
        let radius = *depth as f32 * RADIAL_RING_STEP;
        let count = ring.len() as f32;
        if count == 0.0 {
            continue;
        }
        let center = if *side == 0 {
            0.0f32
        } else {
            std::f32::consts::PI
        };
        let step = if count > 1.0 {
            SPAN / (count - 1.0)
        } else {
            0.0
        };
        let start = center - SPAN / 2.0;
        for (k, &v) in ring.iter().enumerate() {
            // Одиночная нода — в центре стороны (0° / 180°); несколько —
            // равномерно по [center-SPAN/2, center+SPAN/2].
            let angle = if count <= 1.0 {
                center
            } else {
                start + k as f32 * step
            };
            let cx = seed_cx + angle.cos() * radius;
            let cy = seed_cy + angle.sin() * radius;
            let n = &canvas.nodes[v];
            target.insert(v, [cx - n.width / 2.0, cy - n.height / 2.0]);
            rects.insert(
                v,
                [cx - n.width / 2.0, cy - n.height / 2.0, n.width, n.height],
            );
        }
    }
}

/// Соседи ноды по рёбрам (ненаправленно, оба направления).
fn neighbors_of(canvas: &Canvas, v: usize) -> Vec<usize> {
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut out: Vec<usize> = Vec::new();
    for e in &canvas.edges {
        let (Some(&a), Some(&b)) = (
            index_of.get(e.from_node.as_str()),
            index_of.get(e.to_node.as_str()),
        ) else {
            continue;
        };
        if a == v {
            out.push(b);
        } else if b == v {
            out.push(a);
        }
    }
    out
}

// ===========================================================================
// Минимизация пересечений «ребро × нода» (swap + shift + insertion)
// ===========================================================================

/// Сбросить top-left позиции из rects в target (после refine).
fn sync_targets(rects: &HashMap<usize, Rect>, target: &mut HashMap<usize, [f32; 2]>) {
    for (v, r) in rects {
        target.insert(*v, [r[0], r[1]]);
    }
}

/// Оптимизация пересечений: swap соседей в колонке + поперечные сдвиги
/// колонок + переносы на другую строку. Колонка семени (level 0) иммунна к
/// сдвигам (якорь). Ход — строгое падение (пересечения, тай-брейк — длина).
/// `horizontal` — направление поперечного сдвига: true → по Y (TreeH),
/// false → по X (TreeV).
fn refine_crossings(
    verts: &BTreeSet<usize>,
    outgoing: &BTreeMap<usize, BTreeSet<usize>>,
    horizontal: bool,
    columns: &mut BTreeMap<i32, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    // Рёбра подграфа (дедуп BTreeSet-смежностью)
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for &u in verts {
        if let Some(ts) = outgoing.get(&u) {
            for &v in ts {
                if verts.contains(&v) {
                    edges.push((u, v));
                }
            }
        }
    }
    refine_by_swaps(&edges, columns, rects);
    refine_by_column_shifts(&edges, horizontal, columns, rects);
    refine_by_swaps(&edges, columns, rects);
    refine_by_insertions(&edges, columns, rects);
}

/// Центр прямоугольника (для direction-agnostic модели пересечений).
fn center(r: Rect) -> [f32; 2] {
    [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
}

/// Стоимость колонки: пересечения всех рёбер с её прямоугольниками (старое
/// `rects` vs гипотетическое `new_rects`) + суммарная длина инцидентных
/// отрезков. Концы ребра (u,v) исключаются из проверки (сегмент центр→центр
/// всегда «пересекает» свои концы). Образец `scheme_layout::column_state_cost`.
fn column_state_cost(
    edges: &[(usize, usize)],
    col: &[usize],
    rects: &HashMap<usize, Rect>,
    new_rects: &HashMap<usize, Rect>,
) -> (u32, u32, f32, f32) {
    let rect_of = |node: usize, new: bool| -> Rect {
        if new {
            new_rects
                .get(&node)
                .copied()
                .unwrap_or_else(|| rects.get(&node).copied().unwrap_or([0.0; 4]))
        } else {
            rects.get(&node).copied().unwrap_or([0.0; 4])
        }
    };
    let (mut cross_old, mut cross_new) = (0u32, 0u32);
    let (mut len_old, mut len_new) = (0.0f32, 0.0f32);
    for &(u, v) in edges {
        let seg_old = (center(rect_of(u, false)), center(rect_of(v, false)));
        let seg_new = (center(rect_of(u, true)), center(rect_of(v, true)));
        for &idx in col {
            if idx != u && idx != v {
                if seg_intersects_rect(seg_old.0, seg_old.1, rect_of(idx, false), CROSSING_MARGIN) {
                    cross_old += 1;
                }
                if seg_intersects_rect(seg_new.0, seg_new.1, rect_of(idx, true), CROSSING_MARGIN) {
                    cross_new += 1;
                }
            }
        }
        if col.contains(&u) || col.contains(&v) {
            len_old += seg_len(seg_old.0, seg_old.1);
            len_new += seg_len(seg_new.0, seg_new.1);
        }
    }
    (cross_old, cross_new, len_old, len_new)
}

/// Пере-стек колонки от текущего верха (порядок `order`) — гипотетические
/// прямоугольники с накопительным `SIBLING_GAP`. Образец
/// `scheme_layout::restacked_column`.
fn restacked_column(col: &[usize], rects: &HashMap<usize, Rect>) -> HashMap<usize, Rect> {
    let mut out = HashMap::new();
    if let Some(&first) = col.first() {
        let (x, top) = match rects.get(&first) {
            Some(r) => (r[0], r[1]),
            None => (0.0, 0.0),
        };
        let mut y = top;
        for &v in col {
            let (w, h) = match rects.get(&v) {
                Some(r) => (r[2], r[3]),
                None => (0.0, 0.0),
            };
            out.insert(v, [x, y, w, h]);
            y += h + SIBLING_GAP;
        }
    }
    out
}

/// Swap соседних нод в колонке. Пере-стек всей колонки; ход при строгом
/// падении (пересечения, тай-брейк — длина). Ограничено `MAX_REFINE_PASSES`.
fn refine_by_swaps(
    edges: &[(usize, usize)],
    columns: &mut BTreeMap<i32, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let levels: Vec<i32> = columns.keys().copied().collect();
        for lv in levels {
            let col = columns.get(&lv).cloned().unwrap_or_default();
            for pair in col.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if rects.get(&a).is_none() || rects.get(&b).is_none() {
                    continue;
                }
                let mut swapped = col.clone();
                if let (Some(pa), Some(pb)) = (
                    swapped.iter().position(|&x| x == a),
                    swapped.iter().position(|&x| x == b),
                ) {
                    swapped.swap(pa, pb);
                }
                let new_rects = restacked_column(&swapped, rects);
                let (cross_old, cross_new, len_old, len_new) =
                    column_state_cost(edges, &col, rects, &new_rects);
                let improves =
                    cross_new < cross_old || (cross_new == cross_old && len_new < len_old);
                if improves {
                    if let Some(c) = columns.get_mut(&lv) {
                        if let (Some(pa), Some(pb)) = (
                            c.iter().position(|&x| x == a),
                            c.iter().position(|&x| x == b),
                        ) {
                            c.swap(pa, pb);
                        }
                    }
                    for (v, r) in new_rects {
                        rects.insert(v, r);
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Перенос ноды на другую строку колонки (обобщение swap). Строгое
/// улучшение против текущего состояния. Ограничено `MAX_REFINE_PASSES`.
fn refine_by_insertions(
    edges: &[(usize, usize)],
    columns: &mut BTreeMap<i32, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let levels: Vec<i32> = columns.keys().copied().collect();
        for lv in levels {
            let col = columns.get(&lv).cloned().unwrap_or_default();
            if col.len() < 2 {
                continue;
            }
            'node: for &v in &col {
                if rects.get(&v).is_none() {
                    continue;
                }
                let from = match col.iter().position(|&x| x == v) {
                    Some(p) => p,
                    None => continue,
                };
                let (cross_old, _, _, _) = column_state_cost(edges, &col, rects, rects);
                let mut best: Option<InsertionMove> = None;
                for to in 0..col.len() {
                    if to == from {
                        continue;
                    }
                    let mut order = col.clone();
                    order.remove(from);
                    order.insert(to, v);
                    let new_rects = restacked_column(&order, rects);
                    let (_, cross_new, _, len_new) =
                        column_state_cost(edges, &col, rects, &new_rects);
                    let cur = (cross_old, total_len(edges, &col, rects));
                    let cand = (cross_new, len_new);
                    if cand >= cur {
                        continue;
                    }
                    let better = match &best {
                        None => true,
                        Some(InsertionMove { cost, len, .. }) => cand < (*cost, *len),
                    };
                    if better {
                        best = Some(InsertionMove {
                            cost: cross_new,
                            len: len_new,
                            order,
                            rects: new_rects,
                        });
                    }
                }
                if let Some(InsertionMove {
                    order,
                    rects: new_rects,
                    ..
                }) = best
                {
                    if let Some(c) = columns.get_mut(&lv) {
                        *c = order;
                    }
                    for (u, r) in new_rects {
                        rects.insert(u, r);
                    }
                    changed = true;
                    continue 'node;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

struct InsertionMove {
    cost: u32,
    len: f32,
    order: Vec<usize>,
    rects: HashMap<usize, Rect>,
}

/// Суммарная длина инцидентных колонке отрезков (тай-брейк).
fn total_len(edges: &[(usize, usize)], col: &[usize], rects: &HashMap<usize, Rect>) -> f32 {
    let mut length = 0.0f32;
    for &(u, v) in edges {
        if !col.contains(&u) && !col.contains(&v) {
            continue;
        }
        let (Some(&ru), Some(&rv)) = (rects.get(&u), rects.get(&v)) else {
            continue;
        };
        length += seg_len(center(ru), center(rv));
    }
    length
}

/// Вертикальные/горизонтальные сдвиги колонок целиком (шаг `SIBLING_GAP`,
/// ±`SHIFT_STEPS`). Колонка level 0 (семя) НЕ сдвигается — якорь.
/// `horizontal=true` → сдвиг по Y (TreeH), `false` → по X (TreeV). Полная
/// стоимость всех рёбер против всех вершин; строгое улучшение (тай-брейк — длина).
fn refine_by_column_shifts(
    edges: &[(usize, usize)],
    horizontal: bool,
    columns: &mut BTreeMap<i32, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    let verts: BTreeSet<usize> = columns.values().flatten().copied().collect();
    let total_cost = |rects: &HashMap<usize, Rect>| -> (u32, f32) {
        let (mut crossings, mut length) = (0u32, 0.0f32);
        for &(u, v) in edges {
            let (Some(&ru), Some(&rv)) = (rects.get(&u), rects.get(&v)) else {
                continue;
            };
            let p0 = center(ru);
            let p1 = center(rv);
            length += seg_len(p0, p1);
            for &w in &verts {
                if w == u || w == v {
                    continue;
                }
                let Some(&rw) = rects.get(&w) else {
                    continue;
                };
                if seg_intersects_rect(p0, p1, rw, CROSSING_MARGIN) {
                    crossings += 1;
                }
            }
        }
        (crossings, length)
    };
    // Главная ось определяется режимом: TreeH — сдвиг по Y, TreeV — по X.
    for _pass in 0..MAX_SHIFT_PASSES {
        let mut changed = false;
        let levels: Vec<i32> = columns.keys().copied().collect();
        for lv in levels {
            if lv == 0 {
                continue; // колонка семени — якорь
            }
            let Some(col) = columns.get(&lv).cloned() else {
                continue;
            };
            if col.is_empty() {
                continue;
            }
            let (mut best_shift, mut best_cost) = (0.0f32, total_cost(rects));
            for step in -SHIFT_STEPS..=SHIFT_STEPS {
                if step == 0 {
                    continue;
                }
                let shift = step as f32 * SIBLING_GAP;
                let mut trial = rects.clone();
                for &v in &col {
                    if let Some(r) = trial.get_mut(&v) {
                        if horizontal {
                            r[1] += shift;
                        } else {
                            r[0] += shift;
                        }
                    }
                }
                let cost = total_cost(&trial);
                if cost.0 < best_cost.0 || (cost.0 == best_cost.0 && cost.1 < best_cost.1) {
                    best_cost = cost;
                    best_shift = shift;
                }
            }
            if best_shift != 0.0 {
                for &v in &col {
                    if let Some(r) = rects.get_mut(&v) {
                        if horizontal {
                            r[1] += best_shift;
                        } else {
                            r[0] += best_shift;
                        }
                    }
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

// ===========================================================================
// Утилиты
// ===========================================================================

/// Seed-порядок: текущие координаты (ручные подсказки пользователя),
/// тай-брейк по id — детерминизм (образец `scheme_layout::seed_order`).
fn seed_order(canvas: &Canvas, a: usize, b: usize) -> std::cmp::Ordering {
    let (na, nb) = (&canvas.nodes[a], &canvas.nodes[b]);
    na.x.partial_cmp(&nb.x)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(na.y.partial_cmp(&nb.y).unwrap_or(std::cmp::Ordering::Equal))
        .then(na.id.cmp(&nb.id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    fn chain_canvas() -> Canvas {
        // a — b — c, ветвь b — d; e — без связей (не должен двигаться)
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 300.0));
        canvas.nodes.push(Node::text("c", "c", 900.0, 900.0));
        canvas.nodes.push(Node::text("d", "d", 1200.0, 0.0));
        canvas.nodes.push(Node::text("e", "e", 1500.0, 1500.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "c", None));
        // Ветвление b–d (по примеру проверки FR-010: d — справа/снизу)
        canvas.edges.push(Edge::new("e3", "b", None, "d", None));
        canvas
    }

    fn overlaps(canvas: &Canvas, plan: &LayoutPlan) -> bool {
        let rects: Vec<[f32; 4]> = plan
            .iter()
            .filter_map(|(i, [x, y])| canvas.nodes.get(*i).map(|n| [*x, *y, n.width, n.height]))
            .collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let [ax, ay, aw, ah] = rects[i];
                let [bx, by, bw, bh] = rects[j];
                if ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn tree_horizontal_chain() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        // e не связан с b — в плане нет; семя b не двигается
        assert_eq!(plan.len(), 3, "{plan:?}");
        assert!(plan.iter().all(|(i, _)| *i != 1 && *i != 4));
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let (a, c, d) = (by_id[&0], by_id[&2], by_id[&3]);
        // a — колонка слева от семени, c и d — справа
        assert!(a[0] < b.x, "a левее b");
        assert!(c[0] > b.x + b.width, "c правее b");
        assert!(d[0] > b.x + b.width, "d правее b");
        // c и d в одной колонке
        assert!((c[0] - d[0]).abs() < f32::EPSILON);
        // Без пересечений bbox
        assert!(!overlaps(&canvas, &plan), "{plan:?}");
    }

    #[test]
    fn tree_vertical_chain() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::TreeVertical);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let (a, c, d) = (by_id[&0], by_id[&2], by_id[&3]);
        assert!(a[1] < b.y, "a выше b");
        assert!(c[1] > b.y + b.height, "c ниже b");
        assert!((c[1] - d[1]).abs() < f32::EPSILON, "c и d в одной строке");
        assert!(!overlaps(&canvas, &plan));
    }

    #[test]
    fn radial_rings() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::Radial);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let b_c = [b.x + b.width / 2.0, b.y + b.height / 2.0];
        // Все соседи на первом кольце: одинаковое расстояние центров от семени
        let center_dist = |i: usize| -> f32 {
            let [x, y] = by_id[&i];
            let n = &canvas.nodes[i];
            let cx = x + n.width / 2.0;
            let cy = y + n.height / 2.0;
            ((cx - b_c[0]).powi(2) + (cy - b_c[1]).powi(2)).sqrt()
        };
        let (da, dc, dd) = (center_dist(0), center_dist(2), center_dist(3));
        assert!(
            (da - RADIAL_RING_STEP).abs() < 1.0,
            "a на первом кольце: {da}"
        );
        assert!(
            (dc - da).abs() < 1.0 && (dd - da).abs() < 1.0,
            "одно кольцо"
        );
        assert!(!overlaps(&canvas, &plan));
    }

    #[test]
    fn radial_parent_child_separated() {
        // Семя b с родителем a (level −1) и ребёнком c (level +1): родители
        // и потомки на разных полуокружностях → не накладываются.
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 400.0, 0.0));
        canvas.nodes.push(Node::text("c", "c", 800.0, 0.0));
        canvas.nodes.push(Node::text("d", "d", 1200.0, 0.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "c", None));
        canvas.edges.push(Edge::new("e3", "b", None, "d", None));
        let plan = plan_related_layout(&canvas, 1, LayoutMode::Radial);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let b_c = [b.x + b.width / 2.0, b.y + b.height / 2.0];
        // a (предок) — слева от семени (x < b_c[0]); c,d (потомки) — справа
        let a_c = by_id[&0][0] + canvas.nodes[0].width / 2.0;
        let c_c = by_id[&2][0] + canvas.nodes[2].width / 2.0;
        let d_c = by_id[&3][0] + canvas.nodes[3].width / 2.0;
        assert!(
            a_c < b_c[0],
            "предок a слева от семени: a_c={a_c} b_c={}",
            b_c[0]
        );
        assert!(c_c > b_c[0], "потомок c справа: c_c={c_c}");
        assert!(d_c > b_c[0], "потомок d справа: d_c={d_c}");
        assert!(!overlaps(&canvas, &plan), "без наложений bbox");
    }

    #[test]
    fn cycles_terminate() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 300.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "a", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        assert_eq!(plan.len(), 1, "цикл a↔b не зависает, b в плане");
    }

    #[test]
    fn invalid_seed_and_isolated() {
        let canvas = chain_canvas();
        assert!(plan_related_layout(&canvas, 99, LayoutMode::Radial).is_empty());
        // Изолированная нода — пустой план
        assert!(plan_related_layout(&canvas, 4, LayoutMode::Radial).is_empty());
    }

    #[test]
    fn determinism() {
        let canvas = chain_canvas();
        let p1 = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        let p2 = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        assert_eq!(p1, p2);
    }

    #[test]
    fn group_moves_children_once() {
        // b — группа с геометрическим ребёнком kid; группа связана с a
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::group("b", 300.0, 300.0, 400.0, 300.0));
        canvas.nodes.push(Node::text("kid", "kid", 350.0, 350.0));
        canvas.nodes.push(Node::text("far", "far", 2000.0, 2000.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        // kid в плане вместе с группой (тот же дельта-вектор)
        assert_eq!(
            by_id[&2][0] - 350.0,
            by_id[&1][0] - 300.0,
            "kid едет с группой"
        );
        assert_eq!(by_id[&2][1] - 350.0, by_id[&1][1] - 300.0);
        assert!(!by_id.contains_key(&3), "far не тронут");
    }

    /// DAG merge-point: d достижима через b→d и b→c→d; v1 давала 1 уровень,
    /// v2 (longest-path) — корректный макс. Проверяем: d на уровне c+1.
    #[test]
    fn dag_merge_point_levels() {
        // a → b → c → d, и b → d (два пути к d). Сея a.
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 0.0));
        canvas.nodes.push(Node::text("c", "c", 600.0, 0.0));
        canvas.nodes.push(Node::text("d", "d", 900.0, 0.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "c", None));
        canvas.edges.push(Edge::new("e3", "c", None, "d", None));
        canvas.edges.push(Edge::new("e4", "b", None, "d", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        // a (семя) не двигается; b уровень 1, c уровень 2, d уровень 3 (max)
        let (a, b, c, d) = (
            canvas.nodes[0].x + canvas.nodes[0].width,
            by_id[&1][0],
            by_id[&2][0],
            by_id[&3][0],
        );
        assert!(b > a, "b правее a");
        assert!(c > b, "c правее b");
        assert!(d > c, "d (merge) правее c — longest-path уровень 3, не 2");
        assert!(!overlaps(&canvas, &plan), "без пересечений bbox");
    }

    /// Barycenter: ноды с общим родителем стоят рядом в колонке.
    #[test]
    fn barycenter_groups_siblings() {
        // a → {b, c, d}; e → {b, d} (перекрёстные рёбра). После barycenter
        // b и d (общий родитель e) стремятся рядом, минимизируя пересечения.
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("e", "e", 0.0, 400.0));
        canvas.nodes.push(Node::text("b", "b", 400.0, 0.0));
        canvas.nodes.push(Node::text("c", "c", 400.0, 200.0));
        canvas.nodes.push(Node::text("d", "d", 400.0, 400.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "a", None, "c", None));
        canvas.edges.push(Edge::new("e3", "a", None, "d", None));
        canvas.edges.push(Edge::new("e4", "e", None, "b", None));
        canvas.edges.push(Edge::new("e5", "e", None, "d", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        // b и d (общий предок e) стоят рядом по y
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let (yb, yc, yd) = (by_id[&2][1], by_id[&3][1], by_id[&4][1]);
        let bd_adjacent = (yb - yc).abs() < f32::EPSILON || (yd - yc).abs() < f32::EPSILON;
        assert!(
            (yb - yd).abs() < (yb - yc).abs() + f32::EPSILON * 10.0
                || (yb - yd).abs() < (yc - yd).abs() + f32::EPSILON * 10.0
                || bd_adjacent,
            "b и d (общий предок e) стремятся рядом: yb={yb} yc={yc} yd={yd}"
        );
        assert!(!overlaps(&canvas, &plan));
    }

    /// Минимизация пересечений edge×node: длинная диагональ через среднюю
    /// колонку устраняется swap/shift-оптимизацией.
    #[test]
    fn crossing_minimization_diagonal() {
        // a → c (длинная диагональ через колонку b); b — отдельная ветка.
        // Без оптимизации отрезок a→c прошивает b. После refine — 0 пересечений.
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 400.0, 0.0));
        canvas.nodes.push(Node::text("c", "c", 800.0, 400.0));
        canvas.nodes.push(Node::text("x", "x", 400.0, 400.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "a", None, "x", None));
        canvas.edges.push(Edge::new("e3", "b", None, "c", None));
        canvas.edges.push(Edge::new("e4", "x", None, "c", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        // 0 пересечений edge×node (центр→центр, инфляция CROSSING_MARGIN)
        assert_eq!(count_crossings(&canvas, &plan), 0, "диагональ устранена");
        assert!(!overlaps(&canvas, &plan));
    }

    /// Счётчик пересечений edge×node для тестов (центр→центр, инфляция
    /// CROSSING_MARGIN; концы ребра исключаются).
    fn count_crossings(canvas: &Canvas, plan: &LayoutPlan) -> usize {
        let index_of: HashMap<&str, usize> = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let pos: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        // включая семя (не в плане) — его текущая позиция
        let mut rects: HashMap<usize, Rect> = HashMap::new();
        for (i, n) in canvas.nodes.iter().enumerate() {
            let p = pos.get(&i).copied().unwrap_or([n.x, n.y]);
            rects.insert(i, [p[0], p[1], n.width, n.height]);
        }
        let mut count = 0usize;
        for e in &canvas.edges {
            let (Some(&u), Some(&v)) = (
                index_of.get(e.from_node.as_str()),
                index_of.get(e.to_node.as_str()),
            ) else {
                continue;
            };
            let (Some(&ru), Some(&rv)) = (rects.get(&u), rects.get(&v)) else {
                continue;
            };
            let p0 = center(ru);
            let p1 = center(rv);
            for (&w, &rw) in &rects {
                if w == u || w == v {
                    continue;
                }
                if seg_intersects_rect(p0, p1, rw, CROSSING_MARGIN) {
                    count += 1;
                }
            }
        }
        count
    }
}
