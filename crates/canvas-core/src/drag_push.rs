//! FR-073: физика драга «расталкивание» (drag push-apart) с сейф-зонами.
//!
//! Модель из прототипа `docs/prototypes/prototype-unified.html` (ЧАСТЬ X),
//! перенесённая один-в-один в ядро. У каждой ноды есть якорь (её
//! зафиксированное место). Активные (таскаемые) ноды следуют за курсором
//! (позиции выставляет drag-пайплайн приложения); соседние ноды мягко
//! выталкиваются ореолом активных (минимальный сдвиг по AABB), а пружина
//! возврата тянет их назад к якорям. На drop нода, чей якорь накрыт
//! ореолом активной, получает новый якорь на вытесненной позиции.
//!
//! СЕЙФ-ЗОНА: каждая нода носит вокруг себя буфер `gap/2` со всех сторон —
//! расталкивание срабатывает при соприкосновении буферов, поэтому между
//! границами любых двух нод остаётся зазор ≥ `gap` (активная↔пассивная:
//! ≥ `halo + gap`). Прилипание/наложение невозможно ни в каком состоянии:
//! ореол активной дожимается жёстко (мягкая доля + полный дожим до
//! границы), включая финальный проход после парной фазы.
//!
//! Без сил/скоростей: только позиционные коррекции со сглаживанием —
//! стабильно, предсказуемо, детерминированно (pure-функция шага над
//! позициями). Zero-dep (ADR-0008); wasm-совместимо (без std-эксклюзива).

use std::collections::HashMap;

use crate::model::Canvas;

/// Затухание сглаженной скорости активной ноды за шаг (доля остатка).
const VEL_DECAY: f32 = 0.85;
/// Множитель скорости для предиктивного упреждения ореола.
const PREDICTIVE_GAIN: f32 = 3.0;
/// Максимальное упреждение ореола по одной оси, world px.
const PREDICTIVE_MAX: f32 = 110.0;
/// Сдвиги меньше эпсилона не считаются движением (кадры не будятся зря).
const EPS: f32 = 0.05;
/// Проходы разрешения сейф-зазора при перезакреплении якоря на drop.
const GAP_RESOLVE_PASSES: u32 = 3;

/// Параметры физики расталкивания (FR-073). UI показывает подмножество
/// (`drag_push_*` в [`crate::settings::Settings`]); тонкие константы —
/// config.toml, клампятся при загрузке.
#[derive(Debug, Clone, PartialEq)]
pub struct DragPushParams {
    /// Ореол активной ноды поверх её сейф-зоны, world px (0 — выключен).
    pub halo: f32,
    /// Сейф-зазор между границами любых двух нод, world px (0 — вплотную).
    pub gap: f32,
    /// Жёсткость возврата к якорю (доля за шаг, 0..=1).
    pub ret: f32,
    /// Мягкая доля выталкивания из ореола за шаг (0..=1; остаток дожимается).
    pub push_frac: f32,
    /// Доля парного расталкивания на пару за итерацию (0..=1, на каждую
    /// из двух нод — половина).
    pub pair_frac: f32,
    /// Итераций парной фазы за шаг.
    pub iters: u32,
    /// Предиктивное упреждение ореола по сглаженной скорости курсора.
    pub predictive: bool,
    /// Перезакреплять якоря накрытых нод на drop (false — все съезжаются
    /// обратно всегда).
    pub rebase: bool,
}

impl Default for DragPushParams {
    fn default() -> Self {
        Self {
            halo: 12.0,
            gap: 16.0,
            ret: 0.16,
            push_frac: 0.45,
            pair_frac: 0.30,
            iters: 3,
            predictive: true,
            rebase: true,
        }
    }
}

/// Состояние физики: якоря нод (id → позиция) и сглаженная скорость
/// активной ноды в world px/шаг (для предиктивного упреждения).
#[derive(Debug, Clone, Default)]
pub struct DragPushState {
    /// Якоря нод: id → (x, y). Отсутствующая запись — «якорь = текущая
    /// позиция» (нода внешне сдвинута между драгами).
    pub anchors: HashMap<String, [f32; 2]>,
    /// Сглаженная скорость активной ноды, world px/шаг.
    pub vel: [f32; 2],
    /// Позиция первичной активной ноды на предыдущем шаге (дельта → vel).
    last_active_pos: Option<[f32; 2]>,
}

impl DragPushState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Якоря всех нод = текущие позиции (на старте drag; поглощает
    /// внешние сдвиги между драгами — автораскладка, MCP и т.п.).
    pub fn reanchor_all(&mut self, canvas: &Canvas) {
        self.anchors.clear();
        for node in &canvas.nodes {
            self.anchors.insert(node.id.clone(), [node.x, node.y]);
        }
        self.vel = [0.0, 0.0];
        self.last_active_pos = None;
    }

    /// Якорь ноды по индексу; без записи — текущая позиция.
    fn anchor_of(&self, canvas: &Canvas, index: usize) -> [f32; 2] {
        let node = &canvas.nodes[index];
        self.anchors
            .get(&node.id)
            .copied()
            .unwrap_or([node.x, node.y])
    }
}

/// AABB-прямоугольник ноды/ореола (world px).
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect {
    fn of(node: &crate::model::Node, pad: f32) -> Self {
        Self {
            x: node.x - pad,
            y: node.y - pad,
            w: node.width + pad * 2.0,
            h: node.height + pad * 2.0,
        }
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }
}

/// Минимальный вектор выталкивания `a` из `b` (по наименьшей глубине).
fn mtv(a: &Rect, b: &Rect) -> [f32; 2] {
    let left = b.x + b.w - a.x;
    let right = a.x + a.w - b.x;
    let top = b.y + b.h - a.y;
    let bottom = a.y + a.h - b.y;
    let m = left.min(right).min(top).min(bottom);
    if m == right {
        [-right, 0.0] // влево: правый край a уходит на левый край b
    } else if m == left {
        [left, 0.0] // вправо
    } else if m == bottom {
        [0.0, -bottom] // вверх
    } else {
        [0.0, top] // вниз
    }
}

/// Выталкивает ноду `index` из прямоугольника `other`: мягкая доля `frac`
/// за шаг, затем жёсткий дожим до границы — сейф-зона непроницаема даже
/// при удержании активной ноды с давлением (пружина возврата не продавит).
fn push_out(node: &mut crate::model::Node, other: &Rect, pad: f32, frac: f32) {
    let r = Rect::of(node, pad);
    if !r.overlaps(other) {
        return;
    }
    let p = mtv(&r, other);
    node.x += p[0] * frac;
    node.y += p[1] * frac;
    let r2 = Rect::of(node, pad);
    if r2.overlaps(other) {
        let p2 = mtv(&r2, other);
        node.x += p2[0];
        node.y += p2[1];
    }
}

/// Ореол активной ноды: AABB, надутый на `halo + gap/2`; при включённом
/// предиктивном упреждении расширяется по стороне движения (сглаженная
/// скорость, кламп [`PREDICTIVE_MAX`]).
fn active_halo(node: &crate::model::Node, params: &DragPushParams, vel: [f32; 2]) -> Rect {
    let pad = params.halo + params.gap / 2.0;
    let mut x1 = node.x - pad;
    let mut y1 = node.y - pad;
    let mut x2 = node.x + node.width + pad;
    let mut y2 = node.y + node.height + pad;
    if params.predictive {
        let dx = (vel[0] * PREDICTIVE_GAIN).clamp(-PREDICTIVE_MAX, PREDICTIVE_MAX);
        let dy = (vel[1] * PREDICTIVE_GAIN).clamp(-PREDICTIVE_MAX, PREDICTIVE_MAX);
        if dx < 0.0 {
            x1 += dx;
        } else {
            x2 += dx;
        }
        if dy < 0.0 {
            y1 += dy;
        } else {
            y2 += dy;
        }
    }
    Rect {
        x: x1,
        y: y1,
        w: x2 - x1,
        h: y2 - y1,
    }
}

/// Один шаг физики (вызывать раз в кадр). `active` — индексы таскаемых нод
/// (не возвращаются к якорям и не выталкиваются; их позиции ведёт
/// drag-пайплайн). Возвращает индексы сдвинутых пассивных нод (для
/// обновления spatial-индекса приложения; пусто — движения нет).
pub fn step(
    canvas: &mut Canvas,
    active: &[usize],
    state: &mut DragPushState,
    params: &DragPushParams,
) -> Vec<usize> {
    // Сглаженная скорость активной ноды: дельта позиций между шагами
    // (кадровая, независимая от частоты событий мыши)
    match active.first().copied().and_then(|i| canvas.nodes.get(i)) {
        Some(node) => {
            let p = [node.x, node.y];
            if let Some(prev) = state.last_active_pos {
                state.vel[0] += (p[0] - prev[0] - state.vel[0]) * (1.0 - VEL_DECAY);
                state.vel[1] += (p[1] - prev[1] - state.vel[1]) * (1.0 - VEL_DECAY);
            }
            state.last_active_pos = Some(p);
        }
        None => {
            state.vel[0] *= VEL_DECAY;
            state.vel[1] *= VEL_DECAY;
            state.last_active_pos = None;
        }
    }
    let is_active = |i: usize| active.contains(&i);
    let mut touched: Vec<usize> = Vec::new();
    // Якоря собираются до мутации (borrow-разделение)
    let anchors: Vec<[f32; 2]> = (0..canvas.nodes.len())
        .map(|i| state.anchor_of(canvas, i))
        .collect();

    // 1) мягкий возврат к якорям
    for (i, node) in canvas.nodes.iter_mut().enumerate() {
        if is_active(i) {
            continue;
        }
        let anchor = anchors[i];
        let dx = (anchor[0] - node.x) * params.ret;
        let dy = (anchor[1] - node.y) * params.ret;
        if dx.abs() > EPS || dy.abs() > EPS {
            node.x += dx;
            node.y += dy;
            touched.push(i);
        }
    }

    // Ореол активных: суммарный AABB (активных может быть несколько —
    // мультивыделение тянется жёстко, ореол считаем вокруг всех)
    let halo = canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| is_active(*i))
        .map(|(_, node)| active_halo(node, params, state.vel))
        .reduce(|acc, r| Rect {
            x: acc.x.min(r.x),
            y: acc.y.min(r.y),
            w: (acc.x + acc.w).max(r.x + r.w) - acc.x.min(r.x),
            h: (acc.y + acc.h).max(r.y + r.h) - acc.y.min(r.y),
        });

    // 2) выталкивание ореолом активных (жёсткая сейф-зона)
    if let Some(halo) = &halo {
        for (i, node) in canvas.nodes.iter_mut().enumerate() {
            if is_active(i) {
                continue;
            }
            let before = (node.x, node.y);
            push_out(node, halo, params.gap / 2.0, params.push_frac);
            if (node.x, node.y) != before {
                touched.push(i);
            }
        }
    }

    // 3) взаимное расталкивание пассивных (цепочки нод)
    for _ in 0..params.iters {
        for i in 0..canvas.nodes.len() {
            if is_active(i) {
                continue;
            }
            for j in (i + 1)..canvas.nodes.len() {
                if is_active(j) {
                    continue;
                }
                let (ri, rj) = (
                    Rect::of(&canvas.nodes[i], params.gap / 2.0),
                    Rect::of(&canvas.nodes[j], params.gap / 2.0),
                );
                if !ri.overlaps(&rj) {
                    continue;
                }
                let p = mtv(&ri, &rj);
                let hx = p[0] * 0.5 * params.pair_frac;
                let hy = p[1] * 0.5 * params.pair_frac;
                let (a, b) = canvas.nodes.split_at_mut(j);
                let (bi, bj) = (&mut a[i], &mut b[0]);
                bi.x += hx;
                bi.y += hy;
                bj.x -= hx;
                bj.y -= hy;
                touched.push(i);
                touched.push(j);
            }
        }
    }

    // 4) финальный дожим ореолом: парная фаза не должна пробивать
    //    сейф-зону активных (полное выталкивание до границы)
    if let Some(halo) = &halo {
        for (i, node) in canvas.nodes.iter_mut().enumerate() {
            if is_active(i) {
                continue;
            }
            let before = (node.x, node.y);
            push_out(node, halo, params.gap / 2.0, 1.0);
            if (node.x, node.y) != before {
                touched.push(i);
            }
        }
    }

    touched.sort_unstable();
    touched.dedup();
    touched
}

/// Drop активных нод: каждая активная закрепляется на новом месте; чьи
/// старые якоря накрыты суммарным ореолом активных — получают новый якорь
/// на вытесненной позиции (дополнительно разрешённый против чужих
/// сейф-зон), остальные плавно съезжаются обратно (пружина в [`step`]).
pub fn commit_drop(
    canvas: &Canvas,
    active: &[usize],
    state: &mut DragPushState,
    params: &DragPushParams,
) {
    let is_active = |i: usize| active.contains(&i);
    for (i, node) in canvas.nodes.iter().enumerate() {
        if is_active(i) {
            state.anchors.insert(node.id.clone(), [node.x, node.y]);
        }
    }
    if !params.rebase {
        return;
    }
    let pad = params.halo + params.gap / 2.0;
    // Суммарный ореол всех активных (как в step)
    let halo = canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| is_active(*i))
        .map(|(_, node)| Rect::of(node, pad))
        .reduce(|acc, r| Rect {
            x: acc.x.min(r.x),
            y: acc.y.min(r.y),
            w: (acc.x + acc.w).max(r.x + r.w) - acc.x.min(r.x),
            h: (acc.y + acc.h).max(r.y + r.h) - acc.y.min(r.y),
        });
    let Some(halo) = halo else {
        return;
    };
    let gap2 = params.gap / 2.0;
    for (i, n) in canvas.nodes.iter().enumerate() {
        if is_active(i) {
            continue;
        }
        let anchor = state.anchor_of(canvas, i);
        let old = Rect {
            x: anchor[0],
            y: anchor[1],
            w: n.width,
            h: n.height,
        };
        if !halo.overlaps(&old) {
            continue;
        }
        // Якорь накрыт — нода закрепляется на вытесненной позиции.
        let mut nx = n.x;
        let mut ny = n.y;
        let mut cur = Rect::of(n, gap2);
        if halo.overlaps(&cur) {
            let p = mtv(&cur, &halo);
            nx += p[0];
            ny += p[1];
            cur.x += p[0];
            cur.y += p[1];
        }
        // Сейф-зазор: новый якорь не должен упираться в чужие сейф-зоны.
        for _ in 0..GAP_RESOLVE_PASSES {
            let mut moved = false;
            for (j, m) in canvas.nodes.iter().enumerate() {
                if is_active(j) || j == i {
                    continue;
                }
                let rm = Rect::of(m, gap2);
                if cur.overlaps(&rm) {
                    let p = mtv(&cur, &rm);
                    nx += p[0];
                    ny += p[1];
                    cur.x += p[0];
                    cur.y += p[1];
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        state.anchors.insert(n.id.clone(), [nx, ny]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas_two(gap_x: f32) -> Canvas {
        let mut canvas = Canvas::default();
        let mut a = crate::model::Node::text("a", "A", 0.0, 0.0);
        a.width = 200.0;
        a.height = 80.0;
        let mut b = crate::model::Node::text("b", "B", gap_x, 0.0);
        b.width = 200.0;
        b.height = 80.0;
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        canvas
    }

    fn clearance(canvas: &Canvas, i: usize, j: usize) -> f32 {
        let (a, b) = (&canvas.nodes[i], &canvas.nodes[j]);
        let gx = (b.x - (a.x + a.width)).max(a.x - (b.x + b.width));
        let gy = (b.y - (a.y + a.height)).max(a.y - (b.y + b.height));
        gx.max(gy)
    }

    /// Пружина возврата: после ухода активной вытесненная нода съезжается
    /// к якорю (монотонно по дистанции).
    #[test]
    fn spring_returns_displaced_node() {
        let mut canvas = canvas_two(240.0);
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams::default();
        // активная "a" наехала на "b": ореол (halo 12 + gap 8) задевает b
        canvas.nodes[0].x = 100.0;
        for _ in 0..20 {
            step(&mut canvas, &[0], &mut state, &params);
        }
        let displaced = canvas.nodes[1].x;
        assert!(displaced > 240.0, "b вытеснен вправо: {displaced}");
        // активная ушла — b возвращается к якорю 240
        canvas.nodes[0].x = -600.0;
        let last = displaced;
        for _ in 0..200 {
            step(&mut canvas, &[0], &mut state, &params);
        }
        let back = canvas.nodes[1].x;
        assert!(back < last, "b движется назад: {last} -> {back}");
        assert!((back - 240.0).abs() < 1.0, "b вернулся к якорю: {back}");
    }

    /// Сейф-зона — жёсткая стена: при удержании активной с давлением
    /// зазор активная↔пассивная не падает ниже halo+gap (минус эпсилон).
    #[test]
    fn safe_zone_is_hard_wall_under_sustained_pressure() {
        let mut canvas = canvas_two(240.0);
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams::default(); // halo 12 + gap 16 => 28
        canvas.nodes[0].x = 200.0; // активная внахлёст с якорем b
        for _ in 0..120 {
            step(&mut canvas, &[0], &mut state, &params);
            let c = clearance(&canvas, 0, 1);
            assert!(
                c >= params.halo + params.gap - 0.5,
                "зазор {c} < {} на шаге",
                params.halo + params.gap
            );
        }
    }

    /// Парная фаза не пробивает ореол активной (финальный дожим).
    #[test]
    fn pair_phase_does_not_breach_halo() {
        // b зажата между активной a и якорем-соседом c сверху
        let mut canvas = Canvas::default();
        for (id, x, y) in [("a", 200.0, 0.0), ("b", 500.0, 0.0), ("c", 500.0, -200.0)] {
            let mut n = crate::model::Node::text(id, id, x, y);
            n.width = 200.0;
            n.height = 80.0;
            canvas.nodes.push(n);
        }
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams::default();
        for _ in 0..60 {
            step(&mut canvas, &[0], &mut state, &params);
            let c = clearance(&canvas, 0, 1);
            assert!(
                c >= params.halo + params.gap - 0.5,
                "b вдавлена в ореол: {c}"
            );
        }
    }

    /// commit_drop: накрытый якорь перезакрепляется, ненакрытый — нет.
    #[test]
    fn commit_drop_rebases_covered_only() {
        let mut canvas = Canvas::default();
        for (id, x, y) in [("a", 0.0, 0.0), ("b", 240.0, 0.0), ("c", 1500.0, 900.0)] {
            let mut n = crate::model::Node::text(id, id, x, y);
            n.width = 200.0;
            n.height = 80.0;
            canvas.nodes.push(n);
        }
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams::default();
        // активную a бросили на место b (b вытеснена заранее шагами)
        canvas.nodes[0].x = 260.0;
        canvas.nodes[0].y = 0.0;
        for _ in 0..15 {
            step(&mut canvas, &[0], &mut state, &params);
        }
        commit_drop(&canvas, &[0], &mut state, &params);
        let a_anchor = state.anchors["a"];
        assert_eq!(a_anchor, [260.0, 0.0], "активная закреплена где бросили");
        let b_anchor = state.anchors["b"];
        assert!(
            b_anchor[0] > 240.0 || b_anchor[1] != 0.0,
            "накрытый b перезакреплён: {b_anchor:?}"
        );
        let c_anchor = state.anchors["c"];
        assert_eq!(c_anchor, [1500.0, 900.0], "далёкий c не тронут");
    }

    /// Мультивыделение: все активные исключены из физики (не двигаются),
    /// пассивные выталкиваются суммарным ореолом.
    #[test]
    fn multi_active_excluded_and_pushes_others() {
        let mut canvas = Canvas::default();
        for (id, x, y) in [("a", 0.0, 0.0), ("b", 300.0, 0.0), ("c", 600.0, 0.0)] {
            let mut n = crate::model::Node::text(id, id, x, y);
            n.width = 200.0;
            n.height = 80.0;
            canvas.nodes.push(n);
        }
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams::default();
        canvas.nodes[1].x = 260.0; // активная b наехала на c
        let before = (canvas.nodes[0].x, canvas.nodes[0].y);
        let touched = step(&mut canvas, &[1], &mut state, &params);
        assert_eq!(
            (canvas.nodes[0].x, canvas.nodes[0].y),
            before,
            "чужая a не двигается без наезд"
        );
        assert!(!touched.contains(&1), "активная b не в touched");
        let c = clearance(&canvas, 1, 2);
        assert!(c >= params.halo + params.gap - 0.5, "c вытолкнут: {c}");
    }

    /// gap=0, halo=0 — вплотную без наложения (старое поведение).
    #[test]
    fn zero_params_allow_flush_but_not_overlap() {
        let mut canvas = canvas_two(240.0);
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams {
            halo: 0.0,
            gap: 0.0,
            ..DragPushParams::default()
        };
        canvas.nodes[0].x = 220.0; // наезд на 20px
        for _ in 0..60 {
            step(&mut canvas, &[0], &mut state, &params);
            let c = clearance(&canvas, 0, 1);
            assert!(c >= -0.5, "наложение: {c}");
        }
        assert!(clearance(&canvas, 0, 1) <= 1.0, "вплотную (без наезда)");
    }

    /// rebase=false — накрытые съезжаются обратно (якоря не перезакрепляются).
    #[test]
    fn commit_drop_without_rebase_keeps_old_anchors() {
        let mut canvas = canvas_two(240.0);
        let mut state = DragPushState::new();
        state.reanchor_all(&canvas);
        let params = DragPushParams {
            rebase: false,
            ..DragPushParams::default()
        };
        canvas.nodes[0].x = 260.0;
        for _ in 0..10 {
            step(&mut canvas, &[0], &mut state, &params);
        }
        commit_drop(&canvas, &[0], &mut state, &params);
        assert_eq!(state.anchors["b"], [240.0, 0.0], "b держит старый якорь");
    }

    /// Пустой канвас/пустой active — без паник и без движения.
    #[test]
    fn empty_canvas_and_empty_active_are_noops() {
        let mut canvas = Canvas::default();
        let mut state = DragPushState::new();
        let params = DragPushParams::default();
        assert!(step(&mut canvas, &[], &mut state, &params).is_empty());
        commit_drop(&canvas, &[0], &mut state, &params); // несуществующий индекс
        let mut canvas2 = canvas_two(240.0);
        assert!(step(&mut canvas2, &[99], &mut state, &params).is_empty());
    }
}
