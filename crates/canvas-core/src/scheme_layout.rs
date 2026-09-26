//! FR-071: умная раскладка нод при инстансировании шаблонных сцен (схем) —
//! чистые функции над моделью канваса (без GPU/ОС/I-O, wasm-гейт ADR-0011).
//!
//! Запрос владельца (v1): «…чтобы ноды минимально пересекались edge-ами,
//! и группировались по смыслу». Запрос владельца (v2): текущая раскладка
//! «прилипает» ноды друг к другу и расставляет их хаотично — нужны ноды
//! «приблизительно по сетке, так чтобы человеку было удобно их визуально
//! считывать».
//!
//! Конвейер [`plan_scheme_layout`]:
//! 1. Семантические кластеры — union-find по явным группам (`Node.children`,
//!    FR-012) и связности рёбер. Ноды без рёбер (подсказки, вердикты) —
//!    standalone-аннотации.
//! 2. Слоистая DAG-раскладка кластера (Sugiyama-lite): слои — longest-path
//!    от истоков (поток читается слева направо — конвенция портов right→left
//!    инстансера схем, `scheme_apply.rs`); циклы — детерминированный фолбэк.
//! 3. Порядок в колонке — barycenter-проходы (2 полных свипа): ноды с общим
//!    родителем стоят рядом (смысловая группировка); seed-порядок — исходные
//!    координаты автора, тай-брейк по id.
//! 4. СЕТКА (v2, заменяет свободные y-позиции v1): шаг колонки
//!    `CELL_W = max_width + [`LAYER_GAP`]`, шаг ряда
//!    `CELL_H = max_height + [`ROW_GAP`]` — по максимальным размерам нод
//!    сцены. Позиция ноды — левый-верх ячейки: `x = колонка·CELL_W`,
//!    `y = ряд·CELL_H`; колонка занимает подряд идущие ряды
//!    `base..base+m-1`, `base` — по среднему ряду родителей. Ряды выровнены
//!    между всеми колонками и кластерами (сцена читается как таблица), а
//!    зазоры гарантированы конструктивно: по вертикали `≥ CELL_H − max_h ≥
//!    ROW_GAP`, по горизонтали `≥ LAYER_GAP` — «прилипание» невозможно.
//! 5. Минимизация пересечений «ребро × нода» — в единицах сетки: swap
//!    соседних рядов колонки, перенос на другой ряд, сдвиг колонки целыми
//!    рядами (±[`SHIFT_STEPS`]); ход — строгое падение стоимости
//!    (пересечения, тай-брейк — суммарная длина отрезков порт→порт).
//! 6. Рамки групп — bbox детей + [`GROUP_PAD`] (изнутри наружу); standalone
//!    — аннотационные колонки слева/справа от потока (через одну колонку
//!    сетки; сторона — по исходному x против исходного центра потока).
//!
//! Детерминизм (образец `layout.rs` FR-010): сортировка по индексу/id,
//! стабильные сортировки, никаких итераций по `HashMap` в порядковых
//! решениях, никакой рандомизации. План покрывает ВСЕ ноды: инстансер
//! ожидает вычисленную позицию для каждой (контракт FR-071).
//!
//! Группы с рёбрами (в built-in схемах их нет) — v1: участвуют в раскладке
//! как обычные вершины со размером из пакета, рамка по детям не строится
//! (ограничение зафиксировано в FR-071 §«Что сознательно не входит»).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::model::{Canvas, NodeKind};

/// Зазор между соседними колонками сетки (world px): шаг колонки
/// `CELL_W = max_width + LAYER_GAP` — минимум между правым краем ноды
/// колонки `k` и левым краем ноды колонки `k+1`.
pub const LAYER_GAP: f32 = 110.0;
/// Зазор между соседними рядами сетки (world px): шаг ряда
/// `CELL_H = max_height + ROW_GAP` — минимум между нижним краем ноды ряда
/// `k` и верхним краем ноды ряда `k+1` (для самой высокой ноды сцены).
/// Также гарантирует зазор между рамками соседних групп (2×[`GROUP_PAD`]
/// < ROW_GAP — рамки больше не соприкасаются, дефект v1).
pub const ROW_GAP: f32 = 80.0;
/// Пустых колонок сетки между кластерами потока при сборке.
pub const CLUSTER_GAP_COLS: usize = 1;
/// Отступ аннотационной колонки от потока (колонок сетки).
pub const ANNOT_COL_OFFSET: usize = 1;
/// Паддинг рамки группы вокруг bbox детей (world px).
pub const GROUP_PAD: f32 = 32.0;
/// Инфляция bbox нод при подсчёте пересечений «ребро × нода» (world px) —
/// образец `edgegeom::AVOID_MARGIN`.
pub const CROSSING_MARGIN: f32 = 12.0;
/// Максимум проходов swap-оптимизации пересечений.
pub const MAX_REFINE_PASSES: usize = 8;
/// Максимум проходов оптимизации сдвигами колонок.
pub const MAX_SHIFT_PASSES: usize = 2;
/// Диапазон сдвига колонки в шагах СЕТКИ (рядах, ±4 ряда).
pub const SHIFT_STEPS: i32 = 4;
/// Внешних раундов совместной оптимизации до фикспойнта (стадии строго
/// улучшают стоимость кластера; ход, недоступный в начале раунда, может
/// открыться после сдвигов/свапов соседних колонок).
const MAX_JOINT_ROUNDS: usize = 8;
/// Диапазон совместного сдвига ПАРЫ колонок (±2 ряда каждая).
const JOINT_STEPS: i32 = 2;

/// Шаг сетки сцены: ширина/высота ячейки (world px).
#[derive(Debug, Clone, Copy, PartialEq)]
struct GridSpec {
    cell_w: f32,
    cell_h: f32,
}

/// План раскладки схемы: позиции всех нод (индекс → top-left) и новые
/// размеры групп-рамок (индекс → [width, height]).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SchemeLayoutPlan {
    /// Целевые top-left позиции (индекс ноды → [x, y]); покрывает все ноды.
    pub positions: Vec<(usize, [f32; 2])>,
    /// Новые размеры групп-рамок (индекс группы → [width, height]).
    pub group_sizes: Vec<(usize, [f32; 2])>,
}

/// Прямоугольник [x, y, width, height].
pub(crate) type Rect = [f32; 4];

/// Построить план умной раскладки для канваса-сцены (инстанс схемы до
/// сдвига к точке вставки). Детерминирован: два вызова на одном входе
/// дают идентичные планы.
pub fn plan_scheme_layout(canvas: &Canvas) -> SchemeLayoutPlan {
    let count = canvas.nodes.len();
    if count == 0 {
        return SchemeLayoutPlan::default();
    }

    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let is_group: Vec<bool> = canvas
        .nodes
        .iter()
        .map(|n| n.kind() == NodeKind::Group)
        .collect();

    // Направленные списки смежности: дедуп через BTreeSet, петли и висячие
    // концы пропускаются (образец layout.rs FR-010)
    let mut outgoing: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut incoming: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for edge in &canvas.edges {
        let (Some(&from), Some(&to)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if from == to {
            continue;
        }
        outgoing.entry(from).or_default().insert(to);
        incoming.entry(to).or_default().insert(from);
    }
    let degree = |i: usize| -> usize {
        outgoing.get(&i).map_or(0, BTreeSet::len) + incoming.get(&i).map_or(0, BTreeSet::len)
    };

    // Явные дети групп (FR-012): id → индексы, висячие пропускаются
    let mut group_kids: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, node) in canvas.nodes.iter().enumerate() {
        if !is_group[i] {
            continue;
        }
        if let Some(children) = &node.children {
            for child in children {
                if let Some(&ci) = index_of.get(child.as_str()) {
                    if ci != i {
                        group_kids.entry(i).or_default().push(ci);
                    }
                }
            }
        }
    }

    // Вершина потока: не группа, либо группа с рёбрами (v1 FR-071)
    let is_vertex: Vec<bool> = (0..count).map(|i| !is_group[i] || degree(i) > 0).collect();

    // Размер имеет значение для сетки у всех, кроме рамочных групп
    // (группа без рёбер с детьми — рамка пересчитывается по bbox детей).
    let is_framed: Vec<bool> = (0..count)
        .map(|i| is_group[i] && degree(i) == 0 && group_kids.contains_key(&i))
        .collect();
    let cell = grid_spec(canvas, &is_framed);

    // --- 1. Семантические кластеры: union-find по рёбрам и явным группам
    let mut parent: Vec<usize> = (0..count).collect();
    for (&from, targets) in &outgoing {
        for &to in targets {
            union(&mut parent, from, to);
        }
    }
    for (&group, kids) in &group_kids {
        // Дети одной группы — один кластер; группа-вершина едет с детьми
        let mut anchor: Option<usize> = if is_vertex[group] { Some(group) } else { None };
        for &kid in kids {
            if !is_vertex[kid] {
                continue;
            }
            match anchor {
                Some(a) => union(&mut parent, a, kid),
                None => anchor = Some(kid),
            }
        }
    }

    // Компоненты (корень — минимальный индекс: union выбирает меньший)
    let mut components: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (i, &iv) in is_vertex.iter().enumerate() {
        if iv {
            let root = find(&mut parent, i);
            components.entry(root).or_default().insert(i);
        }
    }

    let mut flow_clusters: Vec<BTreeSet<usize>> = Vec::new();
    let mut standalone: Vec<usize> = Vec::new();
    for verts in components.into_values() {
        if verts.len() >= 2 {
            flow_clusters.push(verts);
        } else {
            match verts.iter().next() {
                Some(&v) if degree(v) == 0 => standalone.push(v),
                _ => flow_clusters.push(verts),
            }
        }
    }

    // --- 2–5. Слоистая раскладка кластеров на сетке + оптимизация
    // пересечений. Каждый кластер занимает свои колонки сетки; кластеры
    // стыкуются слева направо с пустой колонкой-зазором; ряды — ОБЩАЯ
    // координата всей сцены (ряды выровнены и между кластерами).
    let mut rects: HashMap<usize, Rect> = HashMap::new();
    let mut cursor_col: usize = 0;
    let mut flow_max_col: usize = 0;
    let mut flow_min_band = i32::MAX;
    let mut flow_max_band = i32::MIN;
    for verts in &flow_clusters {
        let (local, cols_used) =
            layout_cluster(canvas, verts, &outgoing, &incoming, cursor_col, cell);
        if local.is_empty() {
            continue;
        }
        for (i, r) in local {
            let band = (r[1] / cell.cell_h).round() as i32;
            flow_min_band = flow_min_band.min(band);
            flow_max_band = flow_max_band.max(band);
            flow_max_col = flow_max_col.max((r[0] / cell.cell_w).round() as usize);
            rects.insert(i, r);
        }
        cursor_col += cols_used + CLUSTER_GAP_COLS;
    }
    let flow_has = !rects.is_empty();
    let flow_min_band = if flow_has { flow_min_band } else { 0 };
    let flow_max_band = if flow_has { flow_max_band } else { 0 };

    // Исходный центр потока по координатам АВТОРА (для стороны аннотаций)
    let laid_vertices: Vec<usize> = flow_clusters.iter().flatten().copied().collect();
    let seed_center_x = if laid_vertices.is_empty() {
        0.0
    } else {
        let min = laid_vertices
            .iter()
            .map(|&i| canvas.nodes[i].x)
            .fold(f32::MAX, f32::min);
        let max = laid_vertices
            .iter()
            .map(|&i| canvas.nodes[i].x + canvas.nodes[i].width)
            .fold(f32::MIN, f32::max);
        (min + max) / 2.0
    };

    // --- 6. Standalone-ноды и вырожденные группы (без детей) — аннотации
    let mut annots = standalone;
    for (i, _) in canvas.nodes.iter().enumerate() {
        if is_group[i] && degree(i) == 0 && !group_kids.contains_key(&i) {
            annots.push(i);
        }
    }
    annots.sort_by(|&a, &b| seed_order(canvas, a, b));
    let mut left_annots: Vec<usize> = Vec::new();
    let mut right_annots: Vec<usize> = Vec::new();
    for &i in &annots {
        let n = &canvas.nodes[i];
        let center_x = n.x + n.width / 2.0;
        if !flow_has || center_x < seed_center_x {
            left_annots.push(i);
        } else {
            right_annots.push(i);
        }
    }
    if flow_has {
        // Левая аннотационная колонка занимает колонку сетки слева от
        // потока; поток всегда стартует с колонки 0 — при наличии левых
        // аннотаций сдвигаем поток на одну колонку вправо (сетка цела).
        if !left_annots.is_empty() {
            for r in rects.values_mut() {
                r[0] += cell.cell_w;
            }
            flow_max_col += 1;
        }
        let band_center = (flow_min_band + flow_max_band) as f32 / 2.0;
        // Левая колонка — слева от потока, правая — справа
        place_annotation_column(canvas, &left_annots, 0, band_center, cell, &mut rects);
        place_annotation_column(
            canvas,
            &right_annots,
            flow_max_col + ANNOT_COL_OFFSET,
            band_center,
            cell,
            &mut rects,
        );
    } else {
        // Потока нет: одна колонка от (0, 0) в порядке автора
        place_annotation_column(canvas, &annots, 0, 0.0, cell, &mut rects);
    }

    // --- 7. Рамки групп (без рёбер, с детьми): bbox детей + GROUP_PAD,
    // изнутри наружу (вложенные группы — по убыванию глубины)
    let mut framed: Vec<usize> = group_kids
        .keys()
        .copied()
        .filter(|&g| degree(g) == 0)
        .collect();
    let mut depth_memo: HashMap<usize, usize> = HashMap::new();
    framed.sort_by(|&a, &b| {
        let da = group_depth(a, &group_kids, &mut depth_memo, &mut BTreeSet::new());
        let db = group_depth(b, &group_kids, &mut depth_memo, &mut BTreeSet::new());
        db.cmp(&da).then(a.cmp(&b))
    });
    let mut group_sizes: Vec<(usize, [f32; 2])> = Vec::new();
    for g in framed {
        let Some(kids) = group_kids.get(&g) else {
            continue;
        };
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut any = false;
        for &kid in kids {
            let Some(r) = rects.get(&kid) else {
                continue;
            };
            any = true;
            min_x = min_x.min(r[0]);
            min_y = min_y.min(r[1]);
            max_x = max_x.max(r[0] + r[2]);
            max_y = max_y.max(r[1] + r[3]);
        }
        if !any {
            continue;
        }
        let frame = [
            min_x - GROUP_PAD,
            min_y - GROUP_PAD,
            (max_x - min_x) + 2.0 * GROUP_PAD,
            (max_y - min_y) + 2.0 * GROUP_PAD,
        ];
        rects.insert(g, frame);
        group_sizes.push((g, [frame[2], frame[3]]));
    }

    // --- Сборка плана: стабильный порядок по индексу ноды
    let mut positions: Vec<(usize, [f32; 2])> =
        rects.into_iter().map(|(i, r)| (i, [r[0], r[1]])).collect();
    positions.sort_by_key(|(i, _)| *i);
    group_sizes.sort_by_key(|(i, _)| *i);
    SchemeLayoutPlan {
        positions,
        group_sizes,
    }
}

/// Шаг сетки сцены: по максимальным размерам нод, участвующих в раскладке
/// (вершины потока + аннотации, в т.ч. вырожденные группы; рамочные группы
/// исключены — их размер пересчитывается по bbox детей). Гарантирует: любая
/// нода помещается в ячейку, зазоры между соседними ячейками ≥
/// [`LAYER_GAP`]/[`ROW_GAP`].
fn grid_spec(canvas: &Canvas, is_framed: &[bool]) -> GridSpec {
    let mut max_w = 0.0f32;
    let mut max_h = 0.0f32;
    for (i, n) in canvas.nodes.iter().enumerate() {
        if is_framed[i] {
            continue;
        }
        max_w = max_w.max(n.width);
        max_h = max_h.max(n.height);
    }
    GridSpec {
        cell_w: max_w + LAYER_GAP,
        cell_h: max_h + ROW_GAP,
    }
}

/// Состояние сеточной раскладки кластера: колонки (глобальный индекс →
/// порядок нод) и ряд-верх каждой колонки. Позиции всегда производные:
/// x = колонка·CELL_W, y = (base + позиция в колонке)·CELL_H.
struct GridState<'a> {
    canvas: &'a Canvas,
    cell: GridSpec,
    /// Глобальная колонка сетки → порядок нод колонки.
    columns: BTreeMap<usize, Vec<usize>>,
    /// Глобальная колонка сетки → ряд верха колонки (base).
    bases: BTreeMap<usize, i32>,
}

impl<'a> GridState<'a> {
    /// Материализовать прямоугольники всех нод состояния (позиции сетки).
    fn rects(&self) -> HashMap<usize, Rect> {
        let mut out = HashMap::new();
        for (&gcol, col) in &self.columns {
            let base = self.bases.get(&gcol).copied().unwrap_or(0);
            for (k, &v) in col.iter().enumerate() {
                let n = &self.canvas.nodes[v];
                let x = gcol as f32 * self.cell.cell_w;
                let y = (base + k as i32) as f32 * self.cell.cell_h;
                out.insert(v, [x, y, n.width, n.height]);
            }
        }
        out
    }
}

/// Слоистая раскладка одного кластера потока на сетке (шаги 2–5 конвейера).
/// `base_col` — глобальная колонка слоя 0. Возвращает прямоугольники вершин
/// в глобальных координатах сетки и число занятых колонок.
fn layout_cluster(
    canvas: &Canvas,
    verts: &BTreeSet<usize>,
    outgoing: &BTreeMap<usize, BTreeSet<usize>>,
    incoming: &BTreeMap<usize, BTreeSet<usize>>,
    base_col: usize,
    cell: GridSpec,
) -> (HashMap<usize, Rect>, usize) {
    if verts.is_empty() {
        return (HashMap::new(), 0);
    }

    // Рёбра кластера (дедуп через BTreeSet-смежность, порядок детерминирован)
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for &u in verts {
        if let Some(targets) = outgoing.get(&u) {
            for &v in targets {
                if verts.contains(&v) {
                    edges.push((u, v));
                }
            }
        }
    }

    // --- Слои: longest-path от истоков (Kahn); циклы — фолбэк ниже
    let mut layer: HashMap<usize, usize> = HashMap::new();
    let mut in_deg: HashMap<usize, usize> = verts.iter().map(|&v| (v, 0usize)).collect();
    for &(_, v) in &edges {
        if let Some(d) = in_deg.get_mut(&v) {
            *d += 1;
        }
    }
    let mut queued: BTreeSet<usize> = BTreeSet::new();
    let mut processed: BTreeSet<usize> = BTreeSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    for &v in verts {
        if in_deg.get(&v).copied().unwrap_or(0) == 0 {
            layer.insert(v, 0);
            queued.insert(v);
            queue.push_back(v);
        }
    }
    while let Some(u) = queue.pop_front() {
        processed.insert(u);
        let lu = layer.get(&u).copied().unwrap_or(0);
        if let Some(targets) = outgoing.get(&u) {
            for &w in targets {
                if !verts.contains(&w) {
                    continue;
                }
                // longest-path: слой потомка — максимум от предков + 1
                let lw = layer.get(&w).copied().unwrap_or(0).max(lu + 1);
                layer.insert(w, lw);
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

    // Циклы: вершины вне Кана назначаются детерминированным фолбэком —
    // слой = max(слои назначенных предков) + 1; если за проход никто не
    // назначен (чистый цикл) — принудительно минимальная вершина.
    let mut pending: BTreeSet<usize> = verts
        .iter()
        .copied()
        .filter(|&v| !processed.contains(&v))
        .collect();
    while !pending.is_empty() {
        let mut to_assign: Vec<(usize, usize)> = Vec::new();
        for &v in &pending {
            let preds: Vec<usize> = incoming
                .get(&v)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&p| verts.contains(&p))
                .collect();
            if preds.iter().all(|&p| processed.contains(&p)) {
                let lv = preds
                    .iter()
                    .map(|&p| layer.get(&p).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0)
                    + 1;
                to_assign.push((v, lv));
            }
        }
        if to_assign.is_empty() {
            if let Some(&v) = pending.iter().next() {
                let lv = incoming
                    .get(&v)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|&p| verts.contains(&p))
                    .map(|p| layer.get(&p).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0)
                    + 1;
                to_assign.push((v, lv));
            }
        }
        if to_assign.is_empty() {
            break; // страховка (недостижимо: pending непуст и предки есть)
        }
        for (v, lv) in to_assign {
            layer.insert(v, lv);
            processed.insert(v);
            pending.remove(&v);
        }
    }

    // --- Колонки: слой → порядок (seed: координаты автора, тай-брейк id);
    // ключ состояния — ГЛОБАЛЬНАЯ колонка сетки (base_col + слой)
    let mut columns: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &v in verts {
        let lv = layer.get(&v).copied().unwrap_or(0);
        columns.entry(base_col + lv).or_default().push(v);
    }
    for col in columns.values_mut() {
        col.sort_by(|&a, &b| seed_order(canvas, a, b));
    }

    // --- Barycenter: 2 полных свипа (вперёд по предкам, назад по потомкам):
    // ноды с общим родителем собираются рядом — смысловая группировка
    for _sweep in 0..2 {
        barycenter_pass(verts, incoming, &mut columns);
        barycenter_pass(verts, outgoing, &mut columns);
    }

    // --- Сетка: base каждой колонки — по среднему ряду родителей
    let mut state = GridState {
        canvas,
        cell,
        columns,
        bases: BTreeMap::new(),
    };
    assign_initial_bands(&mut state, incoming, verts);

    // --- Оптимизация пересечений в единицах сетки: раунды до фикспойнта
    // (свапы рядов → сдвиги колонок → переносы → совместные сдвиги пар);
    // каждая стадия строго улучшает общую стоимость кластера, порядок
    // фиксирован — детерминизм
    for _round in 0..MAX_JOINT_ROUNDS {
        let before = cluster_cost(verts, &edges, &state.rects());
        refine_by_swaps(&edges, &mut state);
        refine_by_column_shifts(verts, &edges, &mut state);
        refine_by_insertions(&edges, &mut state);
        refine_joint_column_shifts(verts, &edges, &mut state);
        let after = cluster_cost(verts, &edges, &state.rects());
        if after == before {
            break;
        }
    }

    let cols_used = state
        .columns
        .keys()
        .copied()
        .max()
        .map_or(0, |max| max - base_col + 1);
    (state.rects(), cols_used)
}

/// Начальные ряды колонок: base колонки — округлённый средний ряд центров
/// родителей минус половина высоты колонки (в рядах); колонки без
/// размещённых предков — от ряда 0. Колонка занимает ПОДРЯД идущие ряды
/// (base..base+m-1) — ряды выровнены между колонками, наложения внутри
/// колонки исключены (у каждой ноды свой ряд).
fn assign_initial_bands(
    state: &mut GridState,
    incoming: &BTreeMap<usize, BTreeSet<usize>>,
    verts: &BTreeSet<usize>,
) {
    let cols: Vec<usize> = state.columns.keys().copied().collect();
    let mut band_of: HashMap<usize, i32> = HashMap::new();
    let half_band = |v: usize| state.canvas.nodes[v].height / (2.0 * state.cell.cell_h);
    for &gcol in &cols {
        let col = state.columns.get(&gcol).cloned().unwrap_or_default();
        // цель: средний (по нодам колонки) средний ряд центров родителей —
        // в единицах рядов (центр ноды = ряд + высота/2·CELL_H)
        let mut target_sum = 0.0f32;
        let mut target_cnt = 0usize;
        for &v in &col {
            let centers: Vec<f32> = incoming
                .get(&v)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&p| verts.contains(&p))
                .filter_map(|p| band_of.get(&p).map(|&b| b as f32 + half_band(p)))
                .collect();
            if centers.is_empty() {
                continue;
            }
            target_sum += centers.iter().sum::<f32>() / centers.len() as f32;
            target_cnt += 1;
        }
        let m = col.len() as f32;
        let own_offset = col.iter().map(|&v| half_band(v)).sum::<f32>() / m.max(1.0);
        let base: i32 = if target_cnt == 0 {
            0
        } else {
            let target = target_sum / target_cnt as f32;
            let base = (target - (m - 1.0) / 2.0 - own_offset).round();
            base.max(0.0) as i32
        };
        state.bases.insert(gcol, base);
        for (k, &v) in col.iter().enumerate() {
            band_of.insert(v, base + k as i32);
        }
    }
}

/// Один barycenter-проход: упорядочить каждую колонку по среднему индексу
/// соседей (предков для `incoming`, потомков для `outgoing`). Ноды без
/// соседей сохраняют текущую позицию (stable sort по (bary, старый индекс)).
fn barycenter_pass(
    verts: &BTreeSet<usize>,
    neighbors: &BTreeMap<usize, BTreeSet<usize>>,
    columns: &mut BTreeMap<usize, Vec<usize>>,
) {
    // позиция вершины внутри своей колонки
    let mut pos: HashMap<usize, usize> = HashMap::new();
    for col in columns.values() {
        for (k, &v) in col.iter().enumerate() {
            pos.insert(v, k);
        }
    }
    // ключи считать ДО мутаций (borrow + детерминизм)
    let keys: Vec<(usize, Vec<(f32, usize)>)> = columns
        .iter()
        .map(|(&lv, col)| {
            let keyed = col
                .iter()
                .enumerate()
                .map(|(k, &v)| {
                    let mut sum = 0.0f32;
                    let mut cnt = 0usize;
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

/// Стоимость двух состояний колонки: пересечения всех рёбер с
/// прямоугольниками колонки (старое состояние — `rects`, гипотетическое —
/// `new_rects`; концы рёбер и чужие колонки — из `rects`) + суммарная длина
/// инцидентных колонке отрезков в обоих состояниях.
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
        let seg_old = (
            right_center(rect_of(u, false)),
            left_center(rect_of(v, false)),
        );
        let seg_new = (
            right_center(rect_of(u, true)),
            left_center(rect_of(v, true)),
        );
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

/// Оптимизация пересечений перестановками соседних нод внутри колонки.
/// В единицах сетки обмен соседей = обмен их рядов (base колонки не меняется,
/// остальные ряды не двигаются). Стоимость — та же модель
/// [`column_state_cost`]; ход принимается при строгом падении (пересечения,
/// тай-брейк — суммарная длина инцидентных отрезков). Ограничено
/// [`MAX_REFINE_PASSES`].
fn refine_by_swaps(edges: &[(usize, usize)], state: &mut GridState) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let cols: Vec<usize> = state.columns.keys().copied().collect();
        for gcol in cols {
            let col = state.columns.get(&gcol).cloned().unwrap_or_default();
            for pair in col.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let mut trial_columns = state.columns.clone();
                let Some(c) = trial_columns.get_mut(&gcol) else {
                    continue;
                };
                let (Some(pa), Some(pb)) = (
                    c.iter().position(|&x| x == a),
                    c.iter().position(|&x| x == b),
                ) else {
                    continue;
                };
                c.swap(pa, pb);
                let old_rects = state.rects();
                let trial = GridState {
                    canvas: state.canvas,
                    cell: state.cell,
                    columns: trial_columns.clone(),
                    bases: state.bases.clone(),
                };
                let new_rects = trial.rects();
                let (cross_old, cross_new, len_old, len_new) =
                    column_state_cost(edges, &col, &old_rects, &new_rects);
                let improves =
                    cross_new < cross_old || (cross_new == cross_old && len_new < len_old);
                if improves {
                    if let Some(c) = state.columns.get_mut(&gcol) {
                        if let (Some(pa), Some(pb)) = (
                            c.iter().position(|&x| x == a),
                            c.iter().position(|&x| x == b),
                        ) {
                            c.swap(pa, pb);
                        }
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

/// Лучший ход переноса: стоимость (пересечения, длина) + порядок колонки
/// и гипотетические прямоугольники.
struct InsertionMove {
    cost: u32,
    len: f32,
    order: Vec<usize>,
}

/// Оптимизация пересечений переносом ноды на другую позицию её колонки
/// (обобщение swap: тянущаяся через колонку диагональ находит междурядный
/// канал, которого нет у соседних обменов). В единицах сетки перенос
/// пере-стекает ряды колонки от её base. Стоимость — та же модель
/// [`column_state_cost`]; ход — строгое улучшение. Ограничено
/// [`MAX_REFINE_PASSES`].
fn refine_by_insertions(edges: &[(usize, usize)], state: &mut GridState) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let cols: Vec<usize> = state.columns.keys().copied().collect();
        for gcol in cols {
            let col = state.columns.get(&gcol).cloned().unwrap_or_default();
            if col.len() < 2 {
                continue;
            }
            'node: for &v in &col {
                let from = match col.iter().position(|&x| x == v) {
                    Some(p) => p,
                    None => continue,
                };
                let old_rects = state.rects();
                let (cross_old, _, _, _) = column_state_cost(edges, &col, &old_rects, &old_rects);
                let mut best: Option<InsertionMove> = None;
                for to in 0..col.len() {
                    if to == from {
                        continue;
                    }
                    let mut order = col.clone();
                    order.remove(from);
                    order.insert(to, v);
                    let mut trial_columns = state.columns.clone();
                    trial_columns.insert(gcol, order.clone());
                    let trial = GridState {
                        canvas: state.canvas,
                        cell: state.cell,
                        columns: trial_columns,
                        bases: state.bases.clone(),
                    };
                    let new_rects = trial.rects();
                    let (_, cross_new, _, len_new) =
                        column_state_cost(edges, &col, &old_rects, &new_rects);
                    // строгое улучшение против текущего состояния
                    let cur = (cross_old, total_len(edges, &col, &old_rects));
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
                        });
                    }
                }
                if let Some(InsertionMove { order, .. }) = best {
                    if let Some(c) = state.columns.get_mut(&gcol) {
                        *c = order;
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

/// Суммарная длина инцидентных колонке отрезков (тай-брейк стоимости).
fn total_len(edges: &[(usize, usize)], col: &[usize], rects: &HashMap<usize, Rect>) -> f32 {
    let mut length = 0.0f32;
    for &(u, v) in edges {
        if !col.contains(&u) && !col.contains(&v) {
            continue;
        }
        let (Some(&ru), Some(&rv)) = (rects.get(&u), rects.get(&v)) else {
            continue;
        };
        length += seg_len(right_center(ru), left_center(rv));
    }
    length
}

/// Общая стоимость раскладки кластера: пересечения всех рёбер с всеми
/// вершинами (тай-брейк — суммарная длина отрезков порт→порт).
fn cluster_cost(
    verts: &BTreeSet<usize>,
    edges: &[(usize, usize)],
    rects: &HashMap<usize, Rect>,
) -> (u32, f32) {
    let mut crossings = 0u32;
    let mut length = 0.0f32;
    for &(u, v) in edges {
        let (Some(&ru), Some(&rv)) = (rects.get(&u), rects.get(&v)) else {
            continue;
        };
        let p0 = right_center(ru);
        let p1 = left_center(rv);
        length += seg_len(p0, p1);
        for &w in verts {
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
}

/// Оптимизация пересечений вертикальными сдвигами колонок ЦЕЛЫМИ РЯДАМИ
/// сетки (шаг CELL_H, диапазон ±[`SHIFT_STEPS`] рядов): длинные диагональные
/// рёбра проходят через свободные ячейки/междурядные каналы. Полная
/// стоимость всех рёбер против всех вершин кластера; принимается строгое
/// улучшение (тай-брейк — длина).
fn refine_by_column_shifts(
    verts: &BTreeSet<usize>,
    edges: &[(usize, usize)],
    state: &mut GridState,
) {
    for _pass in 0..MAX_SHIFT_PASSES {
        let mut changed = false;
        let cols: Vec<usize> = state.columns.keys().copied().collect();
        for gcol in cols {
            let col = state.columns.get(&gcol).cloned().unwrap_or_default();
            if col.is_empty() {
                continue;
            }
            let old_rects = state.rects();
            let mut best_step = 0i32;
            let mut best_cost = cluster_cost(verts, edges, &old_rects);
            for step in -SHIFT_STEPS..=SHIFT_STEPS {
                if step == 0 {
                    continue;
                }
                let mut trial_bases = state.bases.clone();
                let entry = trial_bases.entry(gcol).or_insert(0);
                *entry += step;
                let trial = GridState {
                    canvas: state.canvas,
                    cell: state.cell,
                    columns: state.columns.clone(),
                    bases: trial_bases,
                };
                let cost = cluster_cost(verts, edges, &trial.rects());
                if cost.0 < best_cost.0 || (cost.0 == best_cost.0 && cost.1 < best_cost.1) {
                    best_cost = cost;
                    best_step = step;
                }
            }
            if best_step != 0 {
                let entry = state.bases.entry(gcol).or_insert(0);
                *entry += best_step;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

/// Совместные сдвиги ПАР колонок целыми рядами (±[`JOINT_STEPS`] каждый):
/// ходы, требующие одновременного перемещения двух колонок (встречные
/// коридоры, разведение пучка на два канала). Стоимость — та же модель
/// [`cluster_cost`]; ход — строгое улучшение; перебор детерминирован.
fn refine_joint_column_shifts(
    verts: &BTreeSet<usize>,
    edges: &[(usize, usize)],
    state: &mut GridState,
) {
    for _pass in 0..MAX_SHIFT_PASSES {
        let mut changed = false;
        let cols: Vec<usize> = state.columns.keys().copied().collect();
        for i in 0..cols.len() {
            for j in (i + 1)..cols.len() {
                let (ci, cj) = (cols[i], cols[j]);
                let old_rects = state.rects();
                let mut best = (0i32, 0i32);
                let mut best_cost = cluster_cost(verts, edges, &old_rects);
                for si in -JOINT_STEPS..=JOINT_STEPS {
                    for sj in -JOINT_STEPS..=JOINT_STEPS {
                        if si == 0 && sj == 0 {
                            continue;
                        }
                        let mut trial_bases = state.bases.clone();
                        *trial_bases.entry(ci).or_insert(0) += si;
                        *trial_bases.entry(cj).or_insert(0) += sj;
                        let trial = GridState {
                            canvas: state.canvas,
                            cell: state.cell,
                            columns: state.columns.clone(),
                            bases: trial_bases,
                        };
                        let cost = cluster_cost(verts, edges, &trial.rects());
                        if cost.0 < best_cost.0 || (cost.0 == best_cost.0 && cost.1 < best_cost.1) {
                            best_cost = cost;
                            best = (si, sj);
                        }
                    }
                }
                if best != (0, 0) {
                    *state.bases.entry(ci).or_insert(0) += best.0;
                    *state.bases.entry(cj).or_insert(0) += best.1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Разложить аннотационную колонку (standalone-ноды) на сетке: колонка
/// `gcol`, ряды подряд от base, base — округлённый `band_center` минус
/// половина высоты колонки, clamp ≥ 0 (ряды сетки, зазоры гарантированы).
fn place_annotation_column(
    canvas: &Canvas,
    annots: &[usize],
    gcol: usize,
    band_center: f32,
    cell: GridSpec,
    rects: &mut HashMap<usize, Rect>,
) {
    if annots.is_empty() {
        return;
    }
    let n = annots.len() as f32;
    let base = (band_center - (n - 1.0) / 2.0).round().max(0.0) as i32;
    for (k, &i) in annots.iter().enumerate() {
        let node = &canvas.nodes[i];
        let x = gcol as f32 * cell.cell_w;
        let y = (base + k as i32) as f32 * cell.cell_h;
        rects.insert(i, [x, y, node.width, node.height]);
    }
}

/// Seed-порядок вершины: исходные координаты автора пакета (семантика
/// раскладки), тай-брейк по id — детерминизм.
fn seed_order(canvas: &Canvas, a: usize, b: usize) -> std::cmp::Ordering {
    let (na, nb) = (&canvas.nodes[a], &canvas.nodes[b]);
    na.x.partial_cmp(&nb.x)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(na.y.partial_cmp(&nb.y).unwrap_or(std::cmp::Ordering::Equal))
        .then(na.id.cmp(&nb.id))
}

/// Глубина вложенности группы по явным детям (для порядка рамок «изнутри
/// наружу»); защита от цикла в `children` (дефект пакета) — 0.
fn group_depth(
    group: usize,
    group_kids: &BTreeMap<usize, Vec<usize>>,
    memo: &mut HashMap<usize, usize>,
    visiting: &mut BTreeSet<usize>,
) -> usize {
    if let Some(&d) = memo.get(&group) {
        return d;
    }
    if !visiting.insert(group) {
        return 0;
    }
    let depth = group_kids.get(&group).map_or(0, |kids| {
        kids.iter()
            .map(|&k| group_depth(k, group_kids, memo, visiting))
            .max()
            .unwrap_or(0)
            + 1
    });
    visiting.remove(&group);
    memo.insert(group, depth);
    depth
}

/// DSU: найти корень (со сжатием пути).
fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// DSU: объединить; корень — меньший индекс (детерминизм компонентов).
fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra == rb {
        return;
    }
    if ra < rb {
        parent[rb] = ra;
    } else {
        parent[ra] = rb;
    }
}

/// Правый-центр прямоугольника (порт истока при потоке слева→справа).
pub(crate) fn right_center(r: Rect) -> [f32; 2] {
    [r[0] + r[2], r[1] + r[3] / 2.0]
}

/// Левый-центр прямоугольника (порт приёмника при потоке слева→справа).
pub(crate) fn left_center(r: Rect) -> [f32; 2] {
    [r[0], r[1] + r[3] / 2.0]
}

/// Длина отрезка.
pub(crate) fn seg_len(p0: [f32; 2], p1: [f32; 2]) -> f32 {
    (p1[0] - p0[0]).hypot(p1[1] - p0[1])
}

/// Пересекает ли отрезок AABB, инфлированный на `margin` (slab-тест
/// Лианга–Барски + проверка концов внутри).
pub(crate) fn seg_intersects_rect(p0: [f32; 2], p1: [f32; 2], rect: Rect, margin: f32) -> bool {
    let min_x = rect[0] - margin;
    let min_y = rect[1] - margin;
    let max_x = rect[0] + rect[2] + margin;
    let max_y = rect[1] + rect[3] + margin;
    let inside = |p: [f32; 2]| p[0] >= min_x && p[0] <= max_x && p[1] >= min_y && p[1] <= max_y;
    if inside(p0) || inside(p1) {
        return true;
    }
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    for (d, p, lo, hi) in [(dx, p0[0], min_x, max_x), (dy, p0[1], min_y, max_y)] {
        if d.abs() < f32::EPSILON {
            if p < lo || p > hi {
                return false;
            }
        } else {
            let mut ta = (lo - p) / d;
            let mut tb = (hi - p) / d;
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Отладка (примеры/диагностика): публичная обёртка slab-теста отрезка
/// с инфляцией прямоугольника — тем же правилом, что и внутренняя метрика.
pub fn debug_seg_intersects(p0: [f32; 2], p1: [f32; 2], rect: Rect, margin: f32) -> bool {
    seg_intersects_rect(p0, p1, rect, margin)
}

/// Диагностика и oracle-метрика тестов: число пересечений «ребро × нода»
/// по прямым отрезкам порт→порт (right-центр → left-центр — конвенция
/// инстансера схем). bbox нод инфлируется на [`CROSSING_MARGIN`]; группы
/// не препятствия (рамки прозрачны для связей — FR-071).
pub fn count_edge_node_crossings(canvas: &Canvas) -> usize {
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let obstacles: Vec<(usize, Rect)> = canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind() != NodeKind::Group)
        .map(|(i, n)| (i, [n.x, n.y, n.width, n.height]))
        .collect();
    let mut total = 0usize;
    for edge in &canvas.edges {
        let (Some(&u), Some(&v)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if u == v {
            continue;
        }
        let (Some(a), Some(b)) = (canvas.nodes.get(u), canvas.nodes.get(v)) else {
            continue;
        };
        let p0 = [a.x + a.width, a.y + a.height / 2.0];
        let p1 = [b.x, b.y + b.height / 2.0];
        total += obstacles
            .iter()
            .filter(|&(i, _)| *i != u && *i != v)
            .filter(|&(_, r)| seg_intersects_rect(p0, p1, *r, CROSSING_MARGIN))
            .count();
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    /// Добавить text-ноду заданного размера; вернуть индекс.
    fn sized(canvas: &mut Canvas, id: &str, x: f32, y: f32, w: f32, h: f32) -> usize {
        let mut n = Node::text(id, id, x, y);
        n.width = w;
        n.height = h;
        canvas.nodes.push(n);
        canvas.nodes.len() - 1
    }

    /// Добавить группу с явными детьми; вернуть индекс.
    fn group(canvas: &mut Canvas, id: &str, children: &[usize]) -> usize {
        let kids: Vec<String> = children
            .iter()
            .map(|&i| canvas.nodes[i].id.clone())
            .collect();
        let mut g = Node::group(id, 0.0, 0.0, 100.0, 100.0);
        g.children = Some(kids);
        canvas.nodes.push(g);
        canvas.nodes.len() - 1
    }

    /// Добавить ребро по индексам нод.
    fn link(canvas: &mut Canvas, id: &str, from: usize, to: usize) {
        let f = canvas.nodes[from].id.clone();
        let t = canvas.nodes[to].id.clone();
        canvas.edges.push(Edge::new(id, &f, None, &t, None));
    }

    /// Индекс ноды по id.
    fn by_id(canvas: &Canvas, id: &str) -> usize {
        canvas
            .nodes
            .iter()
            .position(|n| n.id == id)
            .unwrap_or(usize::MAX)
    }

    /// Применить план к копии канваса (как это делает инстансер).
    fn apply_plan(canvas: &Canvas, plan: &SchemeLayoutPlan) -> Canvas {
        let mut out = Canvas {
            nodes: canvas.nodes.clone(),
            edges: canvas.edges.clone(),
            ..Canvas::default()
        };
        for (i, [x, y]) in &plan.positions {
            if let Some(n) = out.nodes.get_mut(*i) {
                n.x = *x;
                n.y = *y;
            }
        }
        for (i, [w, h]) in &plan.group_sizes {
            if let Some(n) = out.nodes.get_mut(*i) {
                n.width = *w;
                n.height = *h;
            }
        }
        out
    }

    /// Шаг сетки канваса-результата (тем же правилом, что `grid_spec`):
    /// максимум по не-групповым нодам + зазоры.
    fn grid_of(canvas: &Canvas) -> GridSpec {
        let max_w = canvas
            .nodes
            .iter()
            .filter(|n| n.kind() != NodeKind::Group)
            .map(|n| n.width)
            .fold(0.0f32, f32::max);
        let max_h = canvas
            .nodes
            .iter()
            .filter(|n| n.kind() != NodeKind::Group)
            .map(|n| n.height)
            .fold(0.0f32, f32::max);
        GridSpec {
            cell_w: max_w + LAYER_GAP,
            cell_h: max_h + ROW_GAP,
        }
    }

    /// Пересекаются ли bbox каких-то двух не-групповых нод.
    fn node_rects_overlap(canvas: &Canvas) -> bool {
        for (i, a) in canvas.nodes.iter().enumerate() {
            if a.kind() == NodeKind::Group {
                continue;
            }
            for b in canvas.nodes.iter().skip(i + 1) {
                if b.kind() == NodeKind::Group {
                    continue;
                }
                if a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height
                {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn chain_reads_left_to_right() {
        // a → b → c: поток слева направо (конвенция портов right→left),
        // все три ноды на одном ряду сетки
        let mut canvas = Canvas::default();
        sized(&mut canvas, "a", 1200.0, 500.0, 240.0, 140.0);
        sized(&mut canvas, "b", 0.0, 0.0, 240.0, 140.0);
        sized(&mut canvas, "c", 600.0, 900.0, 240.0, 140.0);
        link(&mut canvas, "e1", 0, 1);
        link(&mut canvas, "e2", 1, 2);

        let plan = plan_scheme_layout(&canvas);
        assert_eq!(plan.positions.len(), 3, "план покрывает все ноды");
        let out = apply_plan(&canvas, &plan);
        let (xa, xb, xc) = (
            out.nodes[by_id(&out, "a")].x,
            out.nodes[by_id(&out, "b")].x,
            out.nodes[by_id(&out, "c")].x,
        );
        assert!(xa < xb && xb < xc, "a < b < c по x: {xa} {xb} {xc}");
        let (ya, yb, yc) = (
            out.nodes[by_id(&out, "a")].y,
            out.nodes[by_id(&out, "b")].y,
            out.nodes[by_id(&out, "c")].y,
        );
        assert!(
            (ya - yb).abs() < 1.0 && (yb - yc).abs() < 1.0,
            "цепочка — один ряд сетки: {ya} {yb} {yc}"
        );
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn nodes_snap_to_uniform_grid() {
        // Любая не-групповая нода стоит в ячейке сетки: x = col·CELL_W,
        // y = row·CELL_H; разные ноды — в разных ячейках своей колонки.
        let mut canvas = Canvas::default();
        sized(&mut canvas, "hint", 0.0, 0.0, 360.0, 240.0);
        let salaries = sized(&mut canvas, "salaries", 450.0, 60.0, 270.0, 150.0);
        let teamtools = sized(&mut canvas, "teamtools", 450.0, 220.0, 270.0, 150.0);
        group(&mut canvas, "group-team", &[salaries, teamtools]);
        let subtotals = sized(&mut canvas, "subtotals", 840.0, 60.0, 290.0, 220.0);
        let total = sized(&mut canvas, "total", 1220.0, 60.0, 290.0, 180.0);
        let _verdict = sized(&mut canvas, "verdict", 1600.0, 60.0, 320.0, 200.0);
        link(&mut canvas, "e1", salaries, subtotals);
        link(&mut canvas, "e2", teamtools, subtotals);
        link(&mut canvas, "e3", subtotals, total);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let cell = grid_of(&out);
        for n in out.nodes.iter().filter(|n| n.kind() != NodeKind::Group) {
            let dx = n.x.rem_euclid(cell.cell_w);
            let dy = n.y.rem_euclid(cell.cell_h);
            assert!(
                dx < 0.5 || dx > cell.cell_w - 0.5,
                "{}: x={} не на колонке сетки (шаг {})",
                n.id,
                n.x,
                cell.cell_w
            );
            assert!(
                dy < 0.5 || dy > cell.cell_h - 0.5,
                "{}: y={} не на ряду сетки (шаг {})",
                n.id,
                n.y,
                cell.cell_h
            );
        }
        // Зазоры гарантированы: пары в одной колонке разнесены минимум на
        // CELL_H по вертикали, в соседних колонках — минимум на LAYER_GAP
        // по горизонтали.
        let texts: Vec<&Node> = out
            .nodes
            .iter()
            .filter(|n| n.kind() == NodeKind::Text)
            .collect();
        for (i, a) in texts.iter().enumerate() {
            for b in texts.iter().skip(i + 1) {
                let same_col = (a.x - b.x).abs() < 0.5;
                if same_col {
                    let dy = (a.y - b.y).abs();
                    assert!(
                        dy + 0.5 >= cell.cell_h,
                        "{} и {} в одной колонке слишком близко: {dy}",
                        a.id,
                        b.id
                    );
                } else {
                    let gap = if b.x > a.x {
                        b.x - (a.x + a.width)
                    } else {
                        a.x - (b.x + b.width)
                    };
                    assert!(
                        gap + 0.5 >= LAYER_GAP,
                        "{} и {}: горизонтальный зазор {gap} < LAYER_GAP",
                        a.id,
                        b.id
                    );
                }
            }
        }
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn rows_align_across_columns() {
        // a, a2 → b → c: приёмники соседних колонок стоят на ОДНОМ ряду
        // сетки (b и c выровнены; b — между рядами родителей a и a2)
        let mut canvas = Canvas::default();
        let a = sized(&mut canvas, "a", 0.0, 0.0, 240.0, 140.0);
        let a2 = sized(&mut canvas, "a2", 0.0, 400.0, 240.0, 140.0);
        let b = sized(&mut canvas, "b", 500.0, 0.0, 240.0, 140.0);
        let c = sized(&mut canvas, "c", 1000.0, 0.0, 240.0, 140.0);
        link(&mut canvas, "e1", a, b);
        link(&mut canvas, "e2", a2, b);
        link(&mut canvas, "e3", b, c);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let (ya, ya2, yb, yc) = (
            out.nodes[by_id(&out, "a")].y,
            out.nodes[by_id(&out, "a2")].y,
            out.nodes[by_id(&out, "b")].y,
            out.nodes[by_id(&out, "c")].y,
        );
        assert!(ya < ya2, "a выше a2 (seed-порядок): {ya} {ya2}");
        assert!(
            ya - 1.0 <= yb && yb <= ya2 + 1.0,
            "b между рядами родителей: {ya} ≤ {yb} ≤ {ya2}"
        );
        assert!((yb - yc).abs() < 1.0, "b и c на одном ряду: {yb} {yc}");
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn branching_without_overlaps() {
        // a → (b, c) → d: ветвление, все позиции различны и без пересечений
        let mut canvas = Canvas::default();
        sized(&mut canvas, "a", 0.0, 0.0, 240.0, 140.0);
        sized(&mut canvas, "b", 500.0, -300.0, 240.0, 140.0);
        sized(&mut canvas, "c", 500.0, 300.0, 240.0, 140.0);
        sized(&mut canvas, "d", 1000.0, 0.0, 240.0, 140.0);
        link(&mut canvas, "e1", 0, 1);
        link(&mut canvas, "e2", 0, 2);
        link(&mut canvas, "e3", 1, 3);
        link(&mut canvas, "e4", 2, 3);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        assert!(!node_rects_overlap(&out), "{:?}", plan.positions);
        assert_eq!(count_edge_node_crossings(&out), 0, "{plan:?}");
    }

    #[test]
    fn barycenter_keeps_siblings_together() {
        // c1, c2 с общим родителем p1; c3 — ребёнок p2. Seed-порядок ставит
        // c3 первой по x — barycenter должен собрать детей p1 выше c3.
        let mut canvas = Canvas::default();
        let p1 = sized(&mut canvas, "p1", 0.0, 0.0, 240.0, 140.0);
        let p2 = sized(&mut canvas, "p2", 0.0, 400.0, 240.0, 140.0);
        let c3 = sized(&mut canvas, "c3", 0.0, 800.0, 240.0, 140.0);
        let c1 = sized(&mut canvas, "c1", 600.0, 0.0, 240.0, 140.0);
        let c2 = sized(&mut canvas, "c2", 600.0, 300.0, 240.0, 140.0);
        let s = sized(&mut canvas, "s", 1200.0, 0.0, 240.0, 140.0);
        link(&mut canvas, "e1", p1, c1);
        link(&mut canvas, "e2", p1, c2);
        link(&mut canvas, "e3", p2, c3);
        link(&mut canvas, "e4", c1, s);
        link(&mut canvas, "e5", c2, s);
        link(&mut canvas, "e6", c3, s);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let (y1, y2, y3) = (
            out.nodes[by_id(&out, "c1")].y,
            out.nodes[by_id(&out, "c2")].y,
            out.nodes[by_id(&out, "c3")].y,
        );
        assert!(y1.max(y2) < y3, "дети p1 выше c3: {y1} {y2} {y3}");
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn group_frame_covers_children() {
        // Явная группа из двух детей, оба кормят общий сток: дети рядом
        // (соседние ряды сетки), рамка покрывает bbox детей + GROUP_PAD.
        let mut canvas = Canvas::default();
        let k1 = sized(&mut canvas, "k1", 450.0, 60.0, 270.0, 150.0);
        let k2 = sized(&mut canvas, "k2", 450.0, 220.0, 270.0, 150.0);
        let _g = group(&mut canvas, "g", &[k1, k2]);
        let s = sized(&mut canvas, "s", 900.0, 0.0, 290.0, 220.0);
        link(&mut canvas, "e1", k1, s);
        link(&mut canvas, "e2", k2, s);

        let plan = plan_scheme_layout(&canvas);
        let gi = by_id(&canvas, "g");
        let sizes: HashMap<usize, [f32; 2]> = plan.group_sizes.iter().copied().collect();
        assert!(sizes.contains_key(&gi), "размер группы пересчитан");
        let out = apply_plan(&canvas, &plan);
        let frame = &out.nodes[gi];
        for kid_id in ["k1", "k2"] {
            let k = &out.nodes[by_id(&out, kid_id)];
            assert!(
                k.x >= frame.x - f32::EPSILON
                    && k.y >= frame.y - f32::EPSILON
                    && k.x + k.width <= frame.x + frame.width + f32::EPSILON
                    && k.y + k.height <= frame.y + frame.height + f32::EPSILON,
                "ребёнок {kid_id} внутри рамки группы"
            );
        }
        // дети в одной колонке на соседних рядах: шаг ровно CELL_H,
        // зазор ≥ ROW_GAP
        let cell = grid_of(&out);
        let (rk1, rk2) = (&out.nodes[by_id(&out, "k1")], &out.nodes[by_id(&out, "k2")]);
        assert!(
            ((rk2.y - rk1.y).abs() - cell.cell_h).abs() < 1.0,
            "ряд детей через CELL_H ({}): {:+}",
            cell.cell_h,
            rk2.y - rk1.y
        );
        let gap = (rk2.y - (rk1.y + rk1.height)).abs();
        assert!(gap + 0.5 >= ROW_GAP, "зазор детей {gap} < ROW_GAP");
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn standalone_nodes_go_to_annotation_columns() {
        // hint (исходный x левее потока) — слева, verdict (правее) — справа;
        // зазоры до потока ≥ LAYER_GAP (колонка сетки)
        let mut canvas = Canvas::default();
        sized(&mut canvas, "hint", 0.0, 0.0, 360.0, 240.0);
        let a = sized(&mut canvas, "a", 1200.0, 0.0, 260.0, 200.0);
        let b = sized(&mut canvas, "b", 1800.0, 0.0, 260.0, 200.0);
        sized(&mut canvas, "verdict", 2600.0, 0.0, 320.0, 220.0);
        link(&mut canvas, "e1", a, b);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let hint = &out.nodes[by_id(&out, "hint")];
        let verdict = &out.nodes[by_id(&out, "verdict")];
        let flow_min_x = out
            .nodes
            .iter()
            .filter(|n| n.id == "a" || n.id == "b")
            .map(|n| n.x)
            .fold(f32::MAX, f32::min);
        let flow_max_x = out
            .nodes
            .iter()
            .filter(|n| n.id == "a" || n.id == "b")
            .map(|n| n.x + n.width)
            .fold(f32::MIN, f32::max);
        assert!(
            hint.x + hint.width <= flow_min_x - LAYER_GAP + 1.0,
            "hint левее потока с зазором ≥ LAYER_GAP: {} <= {flow_min_x} - {LAYER_GAP}",
            hint.x + hint.width
        );
        assert!(
            verdict.x >= flow_max_x + LAYER_GAP - 1.0,
            "verdict правее потока с зазором ≥ LAYER_GAP: {} >= {flow_max_x} + {LAYER_GAP}",
            verdict.x
        );
        assert!(!node_rects_overlap(&out));
    }

    #[test]
    fn cycle_terminates_and_covers_all() {
        let mut canvas = Canvas::default();
        sized(&mut canvas, "a", 0.0, 0.0, 240.0, 140.0);
        sized(&mut canvas, "b", 600.0, 300.0, 240.0, 140.0);
        link(&mut canvas, "e1", 0, 1);
        link(&mut canvas, "e2", 1, 0);

        let plan = plan_scheme_layout(&canvas);
        assert_eq!(plan.positions.len(), 2, "цикл не зависает");
        let out = apply_plan(&canvas, &plan);
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn determinism_on_rich_graph() {
        // Граф в духе project-budget: группы, ветвления, standalone
        let mut canvas = Canvas::default();
        sized(&mut canvas, "hint", 0.0, 0.0, 360.0, 240.0);
        let salaries = sized(&mut canvas, "salaries", 450.0, 60.0, 270.0, 150.0);
        let teamtools = sized(&mut canvas, "teamtools", 450.0, 220.0, 270.0, 150.0);
        group(&mut canvas, "group-team", &[salaries, teamtools]);
        let hosting = sized(&mut canvas, "hosting", 450.0, 500.0, 270.0, 150.0);
        let licenses = sized(&mut canvas, "licenses", 450.0, 660.0, 270.0, 150.0);
        group(&mut canvas, "group-infra", &[hosting, licenses]);
        let subtotals = sized(&mut canvas, "subtotals", 840.0, 60.0, 290.0, 220.0);
        let share = sized(&mut canvas, "share", 840.0, 420.0, 290.0, 150.0);
        let reserve = sized(&mut canvas, "reserve", 840.0, 600.0, 290.0, 180.0);
        let total = sized(&mut canvas, "total", 1220.0, 60.0, 290.0, 180.0);
        let payment = sized(&mut canvas, "payment", 1220.0, 420.0, 290.0, 200.0);
        let cash = sized(&mut canvas, "cash", 1600.0, 420.0, 300.0, 220.0);
        sized(&mut canvas, "verdict", 1600.0, 60.0, 320.0, 200.0);
        for (id, from, to) in [
            ("e1", salaries, subtotals),
            ("e2", teamtools, subtotals),
            ("e3", hosting, subtotals),
            ("e4", licenses, subtotals),
            ("e5", subtotals, reserve),
            ("e6", share, reserve),
            ("e7", subtotals, total),
            ("e8", reserve, total),
            ("e9", total, cash),
            ("e10", payment, cash),
        ] {
            link(&mut canvas, id, from, to);
        }

        let p1 = plan_scheme_layout(&canvas);
        let p2 = plan_scheme_layout(&canvas);
        assert_eq!(p1, p2, "два вызова — идентичные планы");
        assert_eq!(
            p1.positions.len(),
            canvas.nodes.len(),
            "план покрывает все ноды (включая группы)"
        );
        let out = apply_plan(&canvas, &p1);
        assert!(!node_rects_overlap(&out), "без пересечений bbox");
        assert_eq!(
            count_edge_node_crossings(&out),
            0,
            "рёбра не пересекают ноды"
        );
    }

    #[test]
    fn long_edge_crossing_is_refined_away() {
        // Длинная диагональ через среднюю колонку: seed-порядок создаёт
        // пересечение s1→t2 с m1 — оптимизация (сдвиг колонки целыми
        // рядами сетки) обязана убрать.
        let mut canvas = Canvas::default();
        let s1 = sized(&mut canvas, "s1", 0.0, 0.0, 200.0, 120.0);
        let s2 = sized(&mut canvas, "s2", 0.0, 600.0, 200.0, 120.0);
        let m1 = sized(&mut canvas, "m1", 500.0, 0.0, 200.0, 120.0);
        let m2 = sized(&mut canvas, "m2", 500.0, 600.0, 200.0, 120.0);
        let t1 = sized(&mut canvas, "t1", 1000.0, 0.0, 200.0, 120.0);
        let t2 = sized(&mut canvas, "t2", 1000.0, 600.0, 200.0, 120.0);
        link(&mut canvas, "e1", s1, m1);
        link(&mut canvas, "e2", m1, t1);
        link(&mut canvas, "e3", s2, m2);
        link(&mut canvas, "e4", m2, t2);
        link(&mut canvas, "e5", s1, t2);

        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        assert_eq!(
            count_edge_node_crossings(&out),
            0,
            "пересечение устранено: {:?}",
            plan.positions
        );
        assert!(!node_rects_overlap(&out));
    }

    /// Регресс (аудит схем 2026-09-25): ноды РАЗНЫХ высот в одной колонке
    /// не слипаются и не накладываются — в сеточной модели у каждой ноды
    /// свой ряд, зазор между соседями по вертикали ≥ CELL_H − max_h ≥
    /// ROW_GAP конструктивно (cohort-launch: план × удержание).
    #[test]
    fn swap_refine_keeps_row_gaps_with_unequal_heights() {
        let mut canvas = Canvas::default();
        let a = sized(&mut canvas, "a", 0.0, 0.0, 260.0, 202.0);
        let b = sized(&mut canvas, "b", 0.0, 266.0, 260.0, 254.0);
        let c = sized(&mut canvas, "c", 0.0, 584.0, 260.0, 182.0);
        let d = sized(&mut canvas, "d", 370.0, 100.0, 280.0, 256.0);
        let e = sized(&mut canvas, "e", 370.0, 520.0, 280.0, 176.0);
        link(&mut canvas, "e1", a, d);
        link(&mut canvas, "e2", b, d);
        link(&mut canvas, "e3", c, d);
        link(&mut canvas, "e4", a, e);
        link(&mut canvas, "e5", c, e);
        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        assert!(
            !node_rects_overlap(&out),
            "наложений нет: {:?}",
            plan.positions
        );
        // Зазоры внутри каждой x-колонки: сортировка по y даёт шаги
        // ≥ ROW_GAP (минус численный эпсилон).
        let mut texts: Vec<&Node> = out
            .nodes
            .iter()
            .filter(|n| n.kind() == NodeKind::Text)
            .collect();
        texts.sort_by_key(|n| (n.x as i32, n.y as i32));
        for pair in texts.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let same_col = (a.x - b.x).abs() < 1.0;
            if !same_col {
                continue;
            }
            let gap_top_down = b.y - (a.y + a.height);
            if gap_top_down >= 0.0 {
                assert!(
                    gap_top_down + 1.0 >= ROW_GAP,
                    "зазор {gap_top_down} < ROW_GAP между {} и {}",
                    a.id,
                    b.id
                );
            }
        }
    }

    #[test]
    fn empty_canvas_and_no_flow() {
        assert_eq!(
            plan_scheme_layout(&Canvas::default()),
            SchemeLayoutPlan::default()
        );
        // Только standalone: одна колонка от (0, 0), ряды сетки
        let mut canvas = Canvas::default();
        sized(&mut canvas, "h1", 100.0, 100.0, 300.0, 200.0);
        sized(&mut canvas, "h2", 100.0, 500.0, 300.0, 200.0);
        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let (h1, h2) = (&out.nodes[by_id(&out, "h1")], &out.nodes[by_id(&out, "h2")]);
        assert!((h1.x - h2.x).abs() < f32::EPSILON, "одна колонка");
        let cell = grid_of(&out);
        assert!(
            (h2.y - h1.y - cell.cell_h).abs() < 1.0,
            "ряды сетки: шаг {}",
            cell.cell_h
        );
    }

    #[test]
    fn isolated_group_frame_follows_children() {
        // Группа без рёбер с двумя детьми без рёбер: дети — кластер
        // (одна колонка), рамка покрывает их bbox.
        let mut canvas = Canvas::default();
        let k1 = sized(&mut canvas, "k1", 0.0, 0.0, 270.0, 150.0);
        let k2 = sized(&mut canvas, "k2", 0.0, 300.0, 270.0, 150.0);
        group(&mut canvas, "g", &[k1, k2]);

        let plan = plan_scheme_layout(&canvas);
        let gi = by_id(&canvas, "g");
        let out = apply_plan(&canvas, &plan);
        let frame = &out.nodes[gi];
        for kid_id in ["k1", "k2"] {
            let k = &out.nodes[by_id(&out, kid_id)];
            assert!(
                k.x >= frame.x - f32::EPSILON
                    && k.y >= frame.y - f32::EPSILON
                    && k.x + k.width <= frame.x + frame.width + f32::EPSILON
                    && k.y + k.height <= frame.y + frame.height + f32::EPSILON,
                "ребёнок {kid_id} внутри рамки"
            );
        }
        assert!(!node_rects_overlap(&out));
    }
}
