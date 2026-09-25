//! FR-071: умная раскладка нод при инстансировании шаблонных сцен (схем) —
//! чистые функции над моделью канваса (без GPU/ОС/I-O, wasm-гейт ADR-0011).
//!
//! Запрос владельца: «…чтобы ноды минимально пересекались edge-ами,
//! и группировались по смыслу». Координаты пакета схемы перестают быть
//! геометрией: план строится по графу, исходные (x, y) остаются только
//! семантическими подсказками порядка (seed barycenter, сторона и порядок
//! аннотационных колонок).
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
//! 4. Позиции: колонки — кумулятивно по максимальной ширине слоя +
//!    [`LAYER_GAP`]; ряды — стек с [`ROW_GAP`]; колонка центрируется на
//!    среднем центре родителей (компактный поток).
//! 5. Минимизация пересечений «ребро × нода»: точная дельта для swap соседей
//!    в колонке и для вертикальных сдвигов колонок целиком; ход принимается
//!    при строгом падении стоимости (пересечения, тай-брейк — суммарная
//!    длина отрезков); проходы ограничены [`MAX_REFINE_PASSES`].
//! 6. Рамки групп — bbox детей + [`GROUP_PAD`] (изнутри наружу); standalone
//!    — аннотационные колонки слева/справа от потока (сторона — по исходному
//!    x автора против исходного центра потока); кластеры потока стыкуются
//!    по горизонтали с [`CLUSTER_GAP`].
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

/// Зазор между колонками слоёв (world px).
pub const LAYER_GAP: f32 = 110.0;
/// Зазор между рядами одной колонки (world px).
pub const ROW_GAP: f32 = 64.0;
/// Зазор между кластерами потока при сборке (world px).
pub const CLUSTER_GAP: f32 = 160.0;
/// Паддинг рамки группы вокруг bbox детей (world px).
pub const GROUP_PAD: f32 = 32.0;
/// Отступ аннотационной колонки от bbox потока (world px).
pub const ANNOT_GAP: f32 = 120.0;
/// Инфляция bbox нод при подсчёте пересечений «ребро × нода» (world px) —
/// образец `edgegeom::AVOID_MARGIN`.
pub const CROSSING_MARGIN: f32 = 12.0;
/// Максимум проходов swap-оптимизации пересечений.
pub const MAX_REFINE_PASSES: usize = 8;
/// Максимум проходов оптимизации сдвигами колонок.
pub const MAX_SHIFT_PASSES: usize = 2;
/// Диапазон сдвига колонки в шагах [`ROW_GAP`] (±4 шага = ±256 px).
pub const SHIFT_STEPS: i32 = 4;

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
type Rect = [f32; 4];

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

    // --- 2–5. Слоистая раскладка кластеров + оптимизация пересечений.
    // Каждый кластер раскладывается в собственных координатах (от 0), затем
    // кластеры стыкуются слева направо с CLUSTER_GAP (top-align).
    let mut rects: HashMap<usize, Rect> = HashMap::new();
    let mut cursor_x = 0.0f32;
    for verts in &flow_clusters {
        let local = layout_cluster(canvas, verts, &outgoing, &incoming);
        if local.is_empty() {
            continue;
        }
        let cluster_max_x = local.values().map(|r| r[0] + r[2]).fold(f32::MIN, f32::max);
        for (i, r) in local {
            rects.insert(i, [r[0] + cursor_x, r[1], r[2], r[3]]);
        }
        cursor_x += cluster_max_x + CLUSTER_GAP;
    }
    let flow_has = !rects.is_empty();
    let flow_min_x = rects.values().map(|r| r[0]).fold(f32::MAX, f32::min);
    let flow_max_x = rects.values().map(|r| r[0] + r[2]).fold(f32::MIN, f32::max);
    let flow_cy = if flow_has {
        let min_y = rects.values().map(|r| r[1]).fold(f32::MAX, f32::min);
        let max_y = rects.values().map(|r| r[1] + r[3]).fold(f32::MIN, f32::max);
        (min_y + max_y) / 2.0
    } else {
        0.0
    };

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
        // Левая колонка — правым краем к потоку, правая — левым краем
        place_annotation_column(
            canvas,
            &left_annots,
            flow_min_x - ANNOT_GAP,
            flow_cy,
            true,
            &mut rects,
        );
        place_annotation_column(
            canvas,
            &right_annots,
            flow_max_x + ANNOT_GAP,
            flow_cy,
            false,
            &mut rects,
        );
    } else {
        // Потока нет: одна колонка от (0, 0) в порядке автора
        let mut y = 0.0f32;
        for &i in &annots {
            let n = &canvas.nodes[i];
            rects.insert(i, [0.0, y, n.width, n.height]);
            y += n.height + ROW_GAP;
        }
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

/// Слоистая раскладка одного кластера потока (шаги 2–5 конвейера).
/// Возвращает прямоугольники вершин в локальных координатах (от 0).
fn layout_cluster(
    canvas: &Canvas,
    verts: &BTreeSet<usize>,
    outgoing: &BTreeMap<usize, BTreeSet<usize>>,
    incoming: &BTreeMap<usize, BTreeSet<usize>>,
) -> HashMap<usize, Rect> {
    if verts.is_empty() {
        return HashMap::new();
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

    // --- Колонки: слой → порядок (seed: координаты автора, тай-брейк id)
    let mut columns: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &v in verts {
        let lv = layer.get(&v).copied().unwrap_or(0);
        columns.entry(lv).or_default().push(v);
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

    // --- Позиции: x кумулятивно по максимальной ширине колонки,
    // y — стек рядов, колонка центрируется на среднем центре родителей
    let mut x_cursor = 0.0f32;
    let mut col_x: BTreeMap<usize, f32> = BTreeMap::new();
    for (&lv, col) in &columns {
        col_x.insert(lv, x_cursor);
        let w = col
            .iter()
            .map(|&v| canvas.nodes[v].width)
            .fold(0.0f32, f32::max);
        x_cursor += w + LAYER_GAP;
    }

    let mut rects_local: HashMap<usize, Rect> = HashMap::new();
    for (&lv, col) in &columns {
        let x = col_x.get(&lv).copied().unwrap_or(0.0);
        let rows = col.len().saturating_sub(1) as f32;
        let total_h = col.iter().map(|&v| canvas.nodes[v].height).sum::<f32>() + ROW_GAP * rows;
        // цель-центр колонки: среднее (по нодам) средних центров предков
        let mut target_sum = 0.0f32;
        let mut target_cnt = 0usize;
        for &v in col {
            let centers: Vec<f32> = incoming
                .get(&v)
                .into_iter()
                .flatten()
                .copied()
                .filter(|&p| rects_local.contains_key(&p))
                .filter_map(|p| rects_local.get(&p).map(|r| r[1] + r[3] / 2.0))
                .collect();
            if centers.is_empty() {
                continue;
            }
            target_sum += centers.iter().sum::<f32>() / centers.len() as f32;
            target_cnt += 1;
        }
        let target = if target_cnt == 0 {
            total_h / 2.0 // первая колонка / без размещённых предков — центр 0
        } else {
            target_sum / target_cnt as f32
        };
        let mut y = target - total_h / 2.0;
        for &v in col {
            let n = &canvas.nodes[v];
            rects_local.insert(v, [x, y, n.width, n.height]);
            y += n.height + ROW_GAP;
        }
    }

    // --- Оптимизация пересечений: swap соседей в колонке, сдвиги колонок,
    // переносы на другие строки, финальный проход swap-ами (порядок стадий
    // фиксирован — детерминизм)
    refine_by_swaps(&edges, &mut columns, &mut rects_local);
    refine_by_column_shifts(verts, &edges, &mut columns, &mut rects_local);
    refine_by_swaps(&edges, &mut columns, &mut rects_local);
    refine_by_insertions(&edges, &mut columns, &mut rects_local);

    rects_local
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

/// Пере-стек колонки от текущего верха (порядок `order`) — гипотетические
/// прямоугольники с сохранением накопительного зазора [`ROW_GAP`].
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
            y += h + ROW_GAP;
        }
    }
    out
}

/// Оптимизация пересечений перестановками соседних нод внутри колонки.
/// Дельта стоимости ТОЧНА: обмен пере-стекует всю колонку от текущего
/// верха (прямой обмен y-позициями валиден только при равных высотах),
/// поэтому пересечения считаются всех рёбер против всех прямоугольников
/// колонки (старые vs новые); ход принимается при строгом падении
/// (пересечения, тай-брейк — суммарная длина инцидентных отрезков).
/// Ограничено [`MAX_REFINE_PASSES`].
fn refine_by_swaps(
    edges: &[(usize, usize)],
    columns: &mut BTreeMap<usize, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let layers: Vec<usize> = columns.keys().copied().collect();
        for lv in layers {
            let col = columns.get(&lv).cloned().unwrap_or_default();
            for pair in col.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if rects.get(&a).is_none() || rects.get(&b).is_none() {
                    continue;
                }
                let mut swapped_order = col.clone();
                if let (Some(pa), Some(pb)) = (
                    swapped_order.iter().position(|&x| x == a),
                    swapped_order.iter().position(|&x| x == b),
                ) {
                    swapped_order.swap(pa, pb);
                }
                let new_rects = restacked_column(&swapped_order, rects);
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

/// Оптимизация пересечений переносом ноды на другую строку её колонки
/// (обобщение swap: тянущаяся через колонку диагональ находит междурядный
/// канал, которого нет у соседних обменов). Стоимость — та же модель
/// пере-стека колонки, что в [`refine_by_swaps`]; ход — строгое улучшение.
/// Ограничено [`MAX_REFINE_PASSES`].
fn refine_by_insertions(
    edges: &[(usize, usize)],
    columns: &mut BTreeMap<usize, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    for _pass in 0..MAX_REFINE_PASSES {
        let mut changed = false;
        let layers: Vec<usize> = columns.keys().copied().collect();
        for lv in layers {
            let col = columns.get(&lv).cloned().unwrap_or_default();
            if col.len() < 2 {
                continue;
            }
            'node: for &v in &col {
                let Some(_) = rects.get(&v) else {
                    continue;
                };
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
                    // строгое улучшение против текущего состояния
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

/// Лучший ход переноса: стоимость (пересечения, длина) + порядок колонки
/// и гипотетические прямоугольники.
struct InsertionMove {
    cost: u32,
    len: f32,
    order: Vec<usize>,
    rects: HashMap<usize, Rect>,
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

/// Оптимизация пересечений вертикальными сдвигами колонок целиком
/// (шаг [`ROW_GAP`], диапазон ±[`SHIFT_STEPS`]): длинные диагональные рёбра
/// проходят через междурядные каналы. Полная стоимость всех рёбер против
/// всех вершин кластера; принимается строгое улучшение (тай-брейк — длина).
fn refine_by_column_shifts(
    verts: &BTreeSet<usize>,
    edges: &[(usize, usize)],
    columns: &mut BTreeMap<usize, Vec<usize>>,
    rects: &mut HashMap<usize, Rect>,
) {
    let total_cost = |rects: &HashMap<usize, Rect>| -> (u32, f32) {
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
    };
    for _pass in 0..MAX_SHIFT_PASSES {
        let mut changed = false;
        let layers: Vec<usize> = columns.keys().copied().collect();
        for lv in layers {
            let Some(col) = columns.get(&lv).cloned() else {
                continue;
            };
            if col.is_empty() {
                continue;
            }
            let mut best_shift = 0.0f32;
            let mut best_cost = total_cost(rects);
            for step in -SHIFT_STEPS..=SHIFT_STEPS {
                if step == 0 {
                    continue;
                }
                let shift = step as f32 * ROW_GAP;
                let mut trial = rects.clone();
                for &v in &col {
                    if let Some(r) = trial.get_mut(&v) {
                        r[1] += shift;
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
                        r[1] += best_shift;
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

/// Разложить аннотационную колонку (standalone-ноды): `align_right = true`
/// — колонка правым краем к `edge_x` (левая сторона потока), иначе левым
/// краем (правая сторона). Ряды — стек с [`ROW_GAP`], колонка центрируется
/// по вертикали на `flow_cy`.
fn place_annotation_column(
    canvas: &Canvas,
    annots: &[usize],
    edge_x: f32,
    flow_cy: f32,
    align_right: bool,
    rects: &mut HashMap<usize, Rect>,
) {
    if annots.is_empty() {
        return;
    }
    let col_w = annots
        .iter()
        .map(|&i| canvas.nodes[i].width)
        .fold(0.0f32, f32::max);
    let rows = annots.len().saturating_sub(1) as f32;
    let total_h = annots.iter().map(|&i| canvas.nodes[i].height).sum::<f32>() + ROW_GAP * rows;
    let x = if align_right { edge_x - col_w } else { edge_x };
    let mut y = flow_cy - total_h / 2.0;
    for &i in annots {
        let n = &canvas.nodes[i];
        rects.insert(i, [x, y, n.width, n.height]);
        y += n.height + ROW_GAP;
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

fn right_center(r: Rect) -> [f32; 2] {
    [r[0] + r[2], r[1] + r[3] / 2.0]
}

fn left_center(r: Rect) -> [f32; 2] {
    [r[0], r[1] + r[3] / 2.0]
}

fn seg_len(p0: [f32; 2], p1: [f32; 2]) -> f32 {
    (p1[0] - p0[0]).hypot(p1[1] - p0[1])
}

/// Пересекает ли отрезок AABB, инфлированный на `margin` (slab-тест
/// Лианга–Барски + проверка концов внутри).
fn seg_intersects_rect(p0: [f32; 2], p1: [f32; 2], rect: Rect, margin: f32) -> bool {
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
        // a → b → c: поток слева направо (конвенция портов right→left)
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
        // (ряд через ROW_GAP), рамка покрывает bbox детей + GROUP_PAD.
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
        // дети рядом: вертикальный зазор ровно ROW_GAP
        let (rk1, rk2) = (&out.nodes[by_id(&out, "k1")], &out.nodes[by_id(&out, "k2")]);
        let gap = (rk2.y - (rk1.y + rk1.height)).abs();
        assert!(
            (gap - ROW_GAP).abs() < 1.0 || (rk1.y - (rk2.y + rk2.height)).abs() - ROW_GAP < 1.0,
            "ряд детей через ROW_GAP: {gap}"
        );
        assert!(!node_rects_overlap(&out));
        assert_eq!(count_edge_node_crossings(&out), 0);
    }

    #[test]
    fn standalone_nodes_go_to_annotation_columns() {
        // hint (исходный x левее потока) — слева, verdict (правее) — справа
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
            hint.x + hint.width <= flow_min_x - ANNOT_GAP + 1.0,
            "hint левее потока: {} <= {flow_min_x} - {ANNOT_GAP}",
            hint.x + hint.width
        );
        assert!(
            verdict.x >= flow_max_x + ANNOT_GAP - 1.0,
            "verdict правее потока: {} >= {flow_max_x} + {ANNOT_GAP}",
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
        // пересечение s1→t2 с m1 — оптимизация (shift/swap) обязана убрать.
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

    /// Регресс (аудит схем 2026-09-25): обмен соседей при РАЗНЫХ высотах
    /// не рвёт стек колонки — зазоры в каждой колонке ≥ [`ROW_GAP`],
    /// наложений нет. Старый refine_by_swaps обменивал y-позиции дословно
    /// и давал наложения-«внахлёст» (cohort-launch: план × удержание).
    #[test]
    fn swap_refine_keeps_row_gaps_with_unequal_heights() {
        let mut canvas = Canvas::default();
        // Колонка источников трёх разных высот с общим родителем —
        // barycenter стянет их к центру родителя, refine обязан
        // сохранять стек.
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
        // Только standalone: одна колонка от (0, 0)
        let mut canvas = Canvas::default();
        sized(&mut canvas, "h1", 100.0, 100.0, 300.0, 200.0);
        sized(&mut canvas, "h2", 100.0, 500.0, 300.0, 200.0);
        let plan = plan_scheme_layout(&canvas);
        let out = apply_plan(&canvas, &plan);
        let (h1, h2) = (&out.nodes[by_id(&out, "h1")], &out.nodes[by_id(&out, "h2")]);
        assert!((h1.x - h2.x).abs() < f32::EPSILON, "одна колонка");
        assert!(
            (h2.y - (h1.y + h1.height + ROW_GAP)).abs() < 1.0,
            "стек с ROW_GAP"
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
