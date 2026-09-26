//! FR-038 / T-038.2: snap-движок выравнивания — чистая геометрия.
//!
//! Магнитная раскладка канваса (требования владельца v2, 22 пункта): притяжение
//! к фоновой сетке с zoom-адаптивным шагом (п.1-3), направляющие по краям/
//! центрам соседних нод (п.6-7), majority-выбор оси (п.10), арбитраж
//! grid-vs-guide по минимальной дельте (п.11), равные интервалы (п.13),
//! collision-avoidance (п.15). Детерминизм (п.20): только std, без времени,
//! случайности и внешних крейтов — одинаковый вход даёт бит-в-бит одинаковый
//! выход (оракул-тест).
//!
//! Границы зоны движка:
//! - п.8 «нелинейное усиление» — визуальная интенсивность линии, живёт на
//!   рендере (T-038.3); сам снап внутри допуска бинарный.
//! - п.4 альтернативные точки привязки (угол/центр вместо bbox) — надстройка
//!   интеграции (T-038.4); движку достаточно bbox-семантики (габарит
//!   выделения при групповом drag, п.14).
//!
//! Семантика (зафиксирована для T-038.3/T-038.4):
//! - допуск `tolerance_px` — ЭКРАННЫЕ px; внутри движка переводится в world
//!   делением на `zoom` (screen = world * zoom).
//! - grid-snap: оси независимо; по каждой оси к линии притягивается ближайший
//!   край bbox (левый/правый, верхний/нижний) — min |дельта| из двух.
//! - guides: оси кандидатов — левый край/центр/правый край (для X), верх/
//!   центр/низ (для Y); якоря moving — те же три точки; совпадение — любая
//!   пара «якорь moving ↔ ось кандидата» в допуске (надмножество пар
//!   «соответствующих» точек; направляющая проходит по координате оси).
//! - направляющие выдаются ТОЛЬКО для осей, где guide победил (п.8-9: за
//!   допуском и у проигравшей оси линий нет).
//! - equal spacing: считается по центрам; соседи должны фланкировать moving
//!   по оси и лежать с ним примерно на одной поперечной координате (допуск);
//!   направляющая — линия через целевой (средний) центр.
//! - collision (п.15, при `collision_gap > 0`): движение по оси
//!   останавливается на границе расширенного на gap AABB кандидата, если
//!   moving приближался снаружи по этой оси и пересекается с зоной по
//!   поперечной. Уже начавшееся пересечение зоны не телепортирует — уход
//!   разрешён (клампится только приближение).
//! - `source`: `Guide`, если хотя бы одна ось привязалась к направляющей,
//!   иначе `Grid` (в т.ч. когда ни одна ось не сработала — дельта без изменений).
//! - полное равенство счётчиков/дельт разрешается порядком обхода: первый
//!   встреченный кандидат в `candidates` (детерминизм п.20).
//!
//! Batch-операции выделения (п.16-17, T-038.5): [`align_centers`] — ряд/
//! колонна по центрам с опорной осью = среднее центров; [`distribute_evenly`]
//! — равные зазоры между краями соседних при сохранении span. Обе — чистые
//! функции над `&mut [SnapRect]`, один вызов интеграции = один undo-шаг
//! (п.17); правила осей и сортировки зафиксированы в док-комментариях
//! (детерминизм п.20).

/// Источник сработавшей привязки (п.18: цвет/подпись линии на рендере).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapSource {
    /// Притяжение к линии сетки (п.1-3).
    Grid,
    /// Притяжение к направляющей соседа (п.6-13).
    Guide,
}

/// Конфигурация snap-движка (заполняется интеграцией из `Settings`, T-038.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapConfig {
    /// Притягивать к сетке (п.1-3).
    pub to_grid: bool,
    /// Притягивать к направляющим соседей (п.6-13).
    pub to_guides: bool,
    /// Текущий minor-шаг сетки, world px (из `GridDensity`).
    pub grid_minor: f32,
    /// Major-шаг сетки, world px (из `GridDensity`).
    pub grid_major: f32,
    /// Допуск совпадения, ЭКРАННЫЕ px (п.6; переводится в world через zoom).
    pub tolerance_px: f32,
    /// Текущий зум камеры (screen = world * zoom).
    pub zoom: f32,
    /// Порог sub-grid (дефолт 1.5 = 150% из v2, п.3).
    pub sub_zoom: f32,
    /// Порог coarse-grid (дефолт 0.5 = 50% из v2, п.3).
    pub coarse_zoom: f32,
    /// Минимальный зазор collision-avoidance, world px (п.15; 0.0 = выкл).
    pub collision_gap: f32,
}

impl Default for SnapConfig {
    /// Стартовые ориентиры v2: GridDensity::Medium (20/100), допуск 8 px,
    /// пороги 150%/50%, collision выключен.
    fn default() -> Self {
        Self {
            to_grid: true,
            to_guides: true,
            grid_minor: 20.0,
            grid_major: 100.0,
            tolerance_px: 8.0,
            zoom: 1.0,
            sub_zoom: 1.5,
            coarse_zoom: 0.5,
            collision_gap: 0.0,
        }
    }
}

/// Габарит перемещаемого прямоугольника (bbox выделения при групповом drag,
/// п.14). У `Node` в canvas-core поля `x/y/width/height`; отдельного Rect-
/// типа в ядре нет, поэтому bbox определён здесь — pure-тип движка.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapRect {
    /// Левая граница, world px.
    pub x: f32,
    /// Верхняя граница, world px.
    pub y: f32,
    /// Ширина, world px.
    pub w: f32,
    /// Высота, world px.
    pub h: f32,
}

impl SnapRect {
    /// Центр по X, world px.
    pub fn cx(&self) -> f32 {
        self.x + self.w / 2.0
    }

    /// Центр по Y, world px.
    pub fn cy(&self) -> f32 {
        self.y + self.h / 2.0
    }

    /// Правая граница, world px.
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Нижняя граница, world px.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// Результат снапа: скорректированная дельта + оси направляющих + источник.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapOutcome {
    /// Коррекция позиции bbox по X, world px (прибавляется к желаемой дельте).
    pub dx: f32,
    /// Коррекция позиции bbox по Y, world px.
    pub dy: f32,
    /// World-координаты вертикальных направляющих (для рендера, T-038.3).
    pub guides_x: Vec<f32>,
    /// World-координаты горизонтальных направляющих.
    pub guides_y: Vec<f32>,
    /// Что победило: направляющая, если сработала хотя бы на одной оси.
    pub source: SnapSource,
}

/// Эффективный шаг сетки с учётом зума (п.1-3): sub-grid (minor/2) при
/// крупном зуме, coarse-grid (major) при мелком, иначе minor. Используется
/// и движком снапа, и рендером zoom-адаптивной сетки (T-038.3).
pub fn effective_grid_step(cfg: &SnapConfig) -> f32 {
    if cfg.zoom > cfg.sub_zoom {
        cfg.grid_minor / 2.0
    } else if cfg.zoom < cfg.coarse_zoom {
        cfg.grid_major
    } else {
        cfg.grid_minor
    }
}

/// Допуск в world-px: экранные px → world делением на зум (screen = world *
/// zoom). Некорректный зум (0/отрицательный) не паникует — фолбэк на 1.0.
fn tol_world(cfg: &SnapConfig) -> f32 {
    let zoom = if cfg.zoom > 0.0 { cfg.zoom } else { 1.0 };
    cfg.tolerance_px / zoom
}

/// Основная функция: габарит перемещаемого + желаемая дельта + кандидаты →
/// скорректированная дельта, оси направляющих, источник (T-038.2, п.6-15/20).
/// Кандидаты служат и источниками направляющих, и препятствиями collision —
/// историческая семантика (тесты движка). Интеграция с раздельными наборами
/// (направляющие ≠ препятствия, FR-012) — [`snap_move_ex`].
pub fn snap_move(
    moving: SnapRect,
    dx: f32,
    dy: f32,
    candidates: &[SnapRect],
    cfg: &SnapConfig,
) -> SnapOutcome {
    snap_move_ex(moving, dx, dy, candidates, candidates, cfg)
}

/// Вариант с раздельными наборами (FR-012 v3): `candidates` — источники
/// направляющих/равных интервалов (группы ВКЛЮЧЕНЫ — выравнивание к краю/
/// центру группы легально, п.14), `obstacles` — препятствия collision-
/// avoidance (группы ИСКЛЮЧЕНЫ — рамка группы не сплошная стена: кламп
/// останавливал ноду на границе группы, центр не доходил до rect, жест
/// втягивания не срабатывал; на отпускании snap телепортировал ноду
/// обратно за границу). Семантика осей/арбитража не изменилась.
pub fn snap_move_ex(
    moving: SnapRect,
    dx: f32,
    dy: f32,
    candidates: &[SnapRect],
    obstacles: &[SnapRect],
    cfg: &SnapConfig,
) -> SnapOutcome {
    let tol = tol_world(cfg);
    let step = effective_grid_step(cfg);
    // Тройки точек оси (низ/центр/верх) для moving и кандидатов — единый
    // алгоритм для X и Y без дублирования.
    let m_x = [moving.x, moving.cx(), moving.right()];
    let m_y = [moving.y, moving.cy(), moving.bottom()];
    let cand_x: Vec<[f32; 3]> = candidates
        .iter()
        .map(|c| [c.x, c.cx(), c.right()])
        .collect();
    let cand_y: Vec<[f32; 3]> = candidates
        .iter()
        .map(|c| [c.y, c.cy(), c.bottom()])
        .collect();

    // Оси независимы (п.1, п.6): каждая решает свои grid/guide/арбитраж.
    // (candidates — только направляющие; collision ниже идёт по obstacles)
    let (dx_snapped, guide_x) = resolve_axis(
        &AxisData {
            m_axis: m_x,
            m_cross: m_y,
            cand_axis: &cand_x,
            cand_cross: &cand_y,
        },
        dx,
        dy,
        step,
        tol,
        cfg,
    );
    let (dy_snapped, guide_y) = resolve_axis(
        &AxisData {
            m_axis: m_y,
            m_cross: m_x,
            cand_axis: &cand_y,
            cand_cross: &cand_x,
        },
        dy,
        dx,
        step,
        tol,
        cfg,
    );

    let mut guides_x = Vec::new();
    let mut guides_y = Vec::new();
    if let Some(g) = guide_x {
        guides_x.push(g);
    }
    if let Some(g) = guide_y {
        guides_y.push(g);
    }

    // Collision-avoidance (п.15) — финальный кламп поверх снапа, при
    // включённом зазоре. Препятствия — obstacles (без групп, FR-012 v3).
    let (out_dx, out_dy) = if cfg.collision_gap > 0.0 {
        clamp_collision(
            moving,
            moving.x + dx_snapped,
            moving.y + dy_snapped,
            obstacles,
            cfg.collision_gap,
            dx_snapped,
            dy_snapped,
        )
    } else {
        (dx_snapped, dy_snapped)
    };

    SnapOutcome {
        dx: out_dx,
        dy: out_dy,
        guides_x,
        guides_y,
        source: if guide_x.is_some() || guide_y.is_some() {
            SnapSource::Guide
        } else {
            SnapSource::Grid
        },
    }
}

/// Параллельные данные одной оси: тройки якорей moving и оси кандидатов по
/// основной (axis) и поперечной (cross) осям — единый алгоритм для X и Y
/// без дублирования (структура — против clippy::too_many_arguments).
struct AxisData<'a> {
    m_axis: [f32; 3],
    m_cross: [f32; 3],
    cand_axis: &'a [[f32; 3]],
    cand_cross: &'a [[f32; 3]],
}

/// Решение по одной оси: направляющие (выравнивание п.6-10 + равные интервалы
/// п.13) против сетки (п.1-3), арбитраж по минимальной |дельте| (п.11).
/// Возвращает (дельта оси, координата направляющей — если guide победил).
/// `desired_cross` — желаемая дельта поперечной оси (для проверки «соседи на
/// одной поперечной координате» в п.13; оценивается до снапа — детерминизм).
fn resolve_axis(
    ax: &AxisData<'_>,
    desired: f32,
    desired_cross: f32,
    step: f32,
    tol: f32,
    cfg: &SnapConfig,
) -> (f32, Option<f32>) {
    // Направляющие: выравнивание против равных интервалов — меньшая |дельта|;
    // при равенстве выравнивание (первый встреченный — детерминизм п.20).
    let guide = if cfg.to_guides {
        let align = best_alignment(ax.m_axis, ax.cand_axis, desired, tol);
        let spacing = best_equal_spacing(
            ax.m_axis,
            ax.m_cross,
            ax.cand_axis,
            ax.cand_cross,
            desired,
            desired_cross,
            tol,
        );
        match (align, spacing) {
            (Some(a), Some(s)) => {
                if s.1.abs() < a.1.abs() {
                    Some(s)
                } else {
                    Some(a)
                }
            }
            (Some(a), None) => Some(a),
            (None, s) => s,
        }
    } else {
        None
    };

    // Сетка (п.1-3): шаг валиден — иначе ось без сетки (защита от кривого конфига).
    let grid = if cfg.to_grid && step > 0.0 && step.is_finite() {
        grid_axis_delta(ax.m_axis, desired, step, tol)
    } else {
        None
    };

    // Арбитраж grid-vs-guide (п.11): меньшая |дельта|; при равенстве —
    // направляющая (позиция оси та же, а выравнивание с соседями информативнее
    // для рендера; зафиксированный детерминированный выбор, п.20).
    match (guide, grid) {
        (Some(g), Some(gr)) => {
            if g.1.abs() <= gr.abs() {
                (desired + g.1, Some(g.0))
            } else {
                (desired + gr, None)
            }
        }
        (Some(g), None) => (desired + g.1, Some(g.0)),
        (None, Some(gr)) => (desired + gr, None),
        (None, None) => (desired, None),
    }
}

/// Лучшее выравнивание по оси (п.6-7, п.10): совпадения «якорь moving (в
/// желаемой позиции) ↔ ось кандидата» группируются по точной координате оси;
/// победитель — группа с наибольшим числом РАЗЛИЧНЫХ соседей, при равенстве —
/// с минимальной |дельтой|, при полном равенстве — первая по порядку обхода
/// `candidates` (детерминизм п.20). Возвращает (координата оси, дельта).
fn best_alignment(
    m_axis: [f32; 3],
    cand_axis: &[[f32; 3]],
    desired: f32,
    tol: f32,
) -> Option<(f32, f32)> {
    let mut coords: Vec<f32> = Vec::new();
    let mut hits: Vec<Vec<(usize, f32)>> = Vec::new(); // (индекс соседа, дельта)
    for (ni, cand) in cand_axis.iter().enumerate() {
        for &axis_v in cand {
            for anchor in m_axis {
                let a = anchor + desired;
                let d = axis_v - a;
                if d.abs() <= tol {
                    match coords.iter().position(|&c| c == axis_v) {
                        Some(pos) => hits[pos].push((ni, d)),
                        None => {
                            coords.push(axis_v);
                            hits.push(vec![(ni, d)]);
                        }
                    }
                }
            }
        }
    }

    let mut best: Option<(usize, f32, f32)> = None; // (соседи, координата, дельта)
    for (k, &coord) in coords.iter().enumerate() {
        let mut neighbors: Vec<usize> = hits[k].iter().map(|(n, _)| *n).collect();
        neighbors.sort_unstable();
        neighbors.dedup();
        let count = neighbors.len();
        // Дельта группы — минимальная |дельта| среди её совпадений.
        let mut delta = hits[k][0].1;
        for &(_, d) in hits[k].iter().skip(1) {
            if d.abs() < delta.abs() {
                delta = d;
            }
        }
        let take = match best {
            None => true,
            Some((bc, _, _)) if count > bc => true,
            Some((bc, _, bd)) if count == bc && delta.abs() < bd.abs() => true,
            _ => false,
        };
        if take {
            best = Some((count, coord, delta));
        }
    }
    best.map(|(_, coord, delta)| (coord, delta))
}

/// Лучшее положение равных интервалов по оси (п.13): пара соседей фланкирует
/// желаемый центр moving по оси, оба центра лежат с центром moving примерно
/// на одной поперечной координате (допуск), а разница зазоров |gap_a - gap_b|
/// в допуске. Возвращает (целевой центр, дельта) — минимальная |дельта| по
/// парам, при равенстве первая по порядку `candidates` (детерминизм).
fn best_equal_spacing(
    m_axis: [f32; 3],
    m_cross: [f32; 3],
    cand_axis: &[[f32; 3]],
    cand_cross: &[[f32; 3]],
    desired: f32,
    desired_cross: f32,
    tol: f32,
) -> Option<(f32, f32)> {
    let mc = m_axis[1] + desired; // желаемый центр по оси
    let mc_cross = m_cross[1] + desired_cross; // желаемый центр поперёк
    let mut best: Option<(f32, f32)> = None; // (целевой центр, дельта)
    for i in 0..cand_axis.len() {
        for j in i + 1..cand_axis.len() {
            // Упорядочиваем пару по центрам: l — левее, r — правее.
            let (l, r) = if cand_axis[i][1] <= cand_axis[j][1] {
                (i, j)
            } else {
                (j, i)
            };
            let lc = cand_axis[l][1];
            let rc = cand_axis[r][1];
            if !(lc < mc && mc < rc) {
                continue; // нет фланкирования по центрам
            }
            // Поперечная близость обоих соседей к moving (ряд «примерно ровный»).
            if (cand_cross[l][1] - mc_cross).abs() > tol
                || (cand_cross[r][1] - mc_cross).abs() > tol
            {
                continue;
            }
            let gap_l = mc - lc;
            let gap_r = rc - mc;
            if (gap_l - gap_r).abs() > tol {
                continue; // разница зазоров вне допуска
            }
            let target = (lc + rc) / 2.0;
            let delta = target - mc;
            let take = match best {
                None => true,
                Some((_, d)) => delta.abs() < d.abs(),
            };
            if take {
                best = Some((target, delta));
            }
        }
    }
    best
}

/// Дельта притяжения оси к сетке (п.1-3): к ближайшей линии тянется
/// БЛИЖАЙНИЙ край bbox (низ/центр не участвуют — центральные якоря, п.4 v2,
/// надстраивает интеграция). В пределах допуска — min |дельта| из двух краёв,
/// при равенстве первый (низ) — детерминизм.
fn grid_axis_delta(m_axis: [f32; 3], desired: f32, step: f32, tol: f32) -> Option<f32> {
    let lo = m_axis[0] + desired;
    let hi = m_axis[2] + desired;
    let mut best: Option<f32> = None;
    for edge in [lo, hi] {
        let d = (edge / step).round() * step - edge;
        if d.abs() <= tol {
            let take = match best {
                None => true,
                Some(b) => d.abs() < b.abs(),
            };
            if take {
                best = Some(d);
            }
        }
    }
    best
}

/// Collision-avoidance (п.15): движение по каждой оси останавливается на
/// границе расширенного на `gap` AABB кандидата, если moving приближался
/// снаружи по этой оси (стартовый bbox целиком до зоны зазора) и пересекается
/// с зоной по поперечной оси (расширенной на gap). Уже начавшееся пересечение
/// не телепортирует — уход из зоны разрешён. Кандидаты обрабатываются в
/// порядке среза (детерминизм п.20). Возвращает клампнутую (dx, dy).
///
/// Публична с T-038.4: интеграция клампит движение КАЖДЫЙ кадр драга
/// (live-collision — grid/guides остаются release-time) и финальную дельту
/// anchor-надстройки (п.4) после арбитража — семантика функции не менялась.
pub fn clamp_collision(
    moving: SnapRect,
    fx: f32,
    fy: f32,
    candidates: &[SnapRect],
    gap: f32,
    dx: f32,
    dy: f32,
) -> (f32, f32) {
    let mut fx = fx;
    let mut fy = fy;

    // X-проход: поперечная (Y) близость считается для желаемой fy.
    if dx > 0.0 {
        for c in candidates {
            let y_over = fy < c.y + c.h + gap && fy + moving.h > c.y - gap;
            if !y_over {
                continue;
            }
            // Стартовали левее зоны зазора → стоп правым краем на её границе.
            if moving.x + moving.w <= c.x - gap {
                let limit = c.x - gap - moving.w;
                if fx > limit {
                    fx = limit;
                }
            }
        }
    } else if dx < 0.0 {
        for c in candidates {
            let y_over = fy < c.y + c.h + gap && fy + moving.h > c.y - gap;
            if !y_over {
                continue;
            }
            // Стартовали правее зоны → стоп левым краем на границе.
            if moving.x >= c.x + c.w + gap {
                let limit = c.x + c.w + gap;
                if fx < limit {
                    fx = limit;
                }
            }
        }
    }

    // Y-проход: поперечная (X) близость — уже с клампнутым fx (диагональ
    // скользит вдоль границы зазора).
    if dy > 0.0 {
        for c in candidates {
            let x_over = fx < c.x + c.w + gap && fx + moving.w > c.x - gap;
            if !x_over {
                continue;
            }
            if moving.y + moving.h <= c.y - gap {
                let limit = c.y - gap - moving.h;
                if fy > limit {
                    fy = limit;
                }
            }
        }
    } else if dy < 0.0 {
        for c in candidates {
            let x_over = fx < c.x + c.w + gap && fx + moving.w > c.x - gap;
            if !x_over {
                continue;
            }
            if moving.y >= c.y + c.h + gap {
                let limit = c.y + c.h + gap;
                if fy < limit {
                    fy = limit;
                }
            }
        }
    }

    (fx - moving.x, fy - moving.y)
}

// ---------------------------------------------------------------------------
// Batch-операции выделения (FR-038 п.16-17, T-038.5)
// ---------------------------------------------------------------------------

/// Ось batch-операции выравнивания/распределения (п.16).
///
/// Семантика оси у ОПЕРАЦИЙ разнонаправленная и зафиксирована FR-038:
/// - `align_centers`: axis=X → колонна (общий center X), axis=Y → ряд
///   (общий center Y) — ось задаёт совмещаемую координату;
/// - `distribute_evenly`: axis=X → равные зазоры вдоль X (ряд), axis=Y →
///   вдоль Y (колонна) — ось задаёт направление раскладки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignAxis {
    /// Ось X: колонна при выравнивании / горизонтальная раскладка при
    /// распределении.
    X,
    /// Ось Y: ряд при выравнивании / вертикальная раскладка при
    /// распределении.
    Y,
}

/// Центр rect по оси операции (хелпер — единая точка семантики оси).
fn axis_center(rect: &SnapRect, axis: AlignAxis) -> f32 {
    match axis {
        AlignAxis::X => rect.cx(),
        AlignAxis::Y => rect.cy(),
    }
}

/// Размер rect вдоль оси операции.
fn axis_size(rect: &SnapRect, axis: AlignAxis) -> f32 {
    match axis {
        AlignAxis::X => rect.w,
        AlignAxis::Y => rect.h,
    }
}

/// Начало rect вдоль оси операции (левый край / верх).
fn axis_start(rect: &SnapRect, axis: AlignAxis) -> f32 {
    match axis {
        AlignAxis::X => rect.x,
        AlignAxis::Y => rect.y,
    }
}

/// Конец rect вдоль оси операции (правый край / низ).
fn axis_end(rect: &SnapRect, axis: AlignAxis) -> f32 {
    match axis {
        AlignAxis::X => rect.right(),
        AlignAxis::Y => rect.bottom(),
    }
}

/// Записать начало rect вдоль оси операции (поперечное и размеры нетронуты).
fn set_axis_start(rect: &mut SnapRect, axis: AlignAxis, start: f32) {
    match axis {
        AlignAxis::X => rect.x = start,
        AlignAxis::Y => rect.y = start,
    }
}

/// FR-038 п.16 (T-038.5): «Выровнять по горизонтали/вертикали» — ряд/колонна
/// ПО ЦЕНТРАМ (интерпретация зафиксирована во FR-038; края/углы владелец
/// подтвердит на приёмке).
///
/// Опорная ось — СРЕДНЕЕ арифметическое центров по оси (правило детерминизма
/// п.20 зафиксировано: сумма берётся в порядке входа — порядок индексов
/// выделения, деление на N; медиана отвергнута — требует соглашения о
/// чётности, а среднее однозначно). Поперечная координата и размеры не
/// меняются. Меньше двух элементов — no-op.
pub fn align_centers(rects: &mut [SnapRect], axis: AlignAxis) {
    if rects.len() < 2 {
        return;
    }
    let total: f32 = rects.iter().map(|rect| axis_center(rect, axis)).sum();
    let target = total / rects.len() as f32;
    for rect in rects.iter_mut() {
        let start = target - axis_size(rect, axis) / 2.0;
        set_axis_start(rect, axis, start);
    }
}

/// FR-038 п.16 (T-038.5): «Распределить равномерно» — равные интервалы между
/// соседними (по порядку координаты оси) элементами.
///
/// Правило (детерминизм п.20): сортировка по центру оси (`total_cmp` —
/// устойчива к NaN, равные центры сохраняют порядок входа); span = [начало
/// первого, конец последнего] НЕ меняется; зазор = (span − Σ размеров) / (N−1)
/// — равные зазоры между КРАЯМИ соседних (при равных размерах — между
/// центрами). Каждый элемент ставится напрямую: start_k = span_start +
/// Σ_{j<k} size_j + k·gap (без накопления f32-дрейфа). Отрицательный зазор
/// (элементы перекрываются уже в span) не клампится — детерминированное
/// «равномерное сжатие». Меньше двух элементов — no-op (N=2: крайние и так
/// на границах span).
pub fn distribute_evenly(rects: &mut [SnapRect], axis: AlignAxis) {
    let n = rects.len();
    if n < 2 {
        return;
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| axis_center(&rects[a], axis).total_cmp(&axis_center(&rects[b], axis)));

    let first = &rects[order[0]];
    let last = &rects[order[n - 1]];
    let span_start = axis_start(first, axis);
    let span_end = axis_end(last, axis);
    // Сумма размеров в отсортированном порядке — фиксированный порядок
    // суммирования (детерминизм п.20)
    let total_size: f32 = order.iter().map(|&i| axis_size(&rects[i], axis)).sum();
    let gap = (span_end - span_start - total_size) / (n - 1) as f32;

    // Префиксная сумма размеров до k-го элемента в отсортированном порядке
    let mut prefix: Vec<f32> = Vec::with_capacity(n);
    let mut acc = 0.0;
    for &i in &order {
        prefix.push(acc);
        acc += axis_size(&rects[i], axis);
    }
    for (k, &i) in order.iter().enumerate() {
        let start = span_start + prefix[k] + k as f32 * gap;
        set_axis_start(&mut rects[i], axis, start);
    }
}

// ---------------------------------------------------------------------------
// Оракул-тесты правил п.1-15/20 (TDD: написаны до реализации).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Краткая форма конструктора прямоугольника для тестов.
    fn r(x: f32, y: f32, w: f32, h: f32) -> SnapRect {
        SnapRect { x, y, w, h }
    }

    /// Только guides (сетка выключена) — для изоляции правил п.6-13.
    fn cfg_only_guides() -> SnapConfig {
        SnapConfig {
            to_grid: false,
            ..SnapConfig::default()
        }
    }

    /// Только сетка (guides выключены) — для изоляции правил п.1-3.
    fn cfg_only_grid() -> SnapConfig {
        SnapConfig {
            to_guides: false,
            ..SnapConfig::default()
        }
    }

    /// Оба тумблера выключены (мастер-отключение, п.5).
    fn cfg_no_snap() -> SnapConfig {
        SnapConfig {
            to_grid: false,
            to_guides: false,
            ..SnapConfig::default()
        }
    }

    #[test]
    fn эффективный_шаг_sub_grid_выше_порога_зума() {
        let cfg = SnapConfig {
            zoom: 2.0,
            ..SnapConfig::default()
        };
        assert_eq!(effective_grid_step(&cfg), 10.0); // minor/2
                                                     // Граница: ровно 1.5 — ещё не sub (строгое неравенство).
        let boundary = SnapConfig {
            zoom: 1.5,
            ..SnapConfig::default()
        };
        assert_eq!(effective_grid_step(&boundary), 20.0);
    }

    #[test]
    fn эффективный_шаг_coarse_grid_ниже_порога_зума() {
        let cfg = SnapConfig {
            zoom: 0.4,
            ..SnapConfig::default()
        };
        assert_eq!(effective_grid_step(&cfg), 100.0); // major
                                                      // Граница: ровно 0.5 — уже не coarse.
        let boundary = SnapConfig {
            zoom: 0.5,
            ..SnapConfig::default()
        };
        assert_eq!(effective_grid_step(&boundary), 20.0);
    }

    #[test]
    fn эффективный_шаг_minor_в_среднем_диапазоне_зума() {
        let cfg = SnapConfig {
            zoom: 1.0,
            ..SnapConfig::default()
        };
        assert_eq!(effective_grid_step(&cfg), 20.0);
    }

    #[test]
    fn сетка_притягивает_ближайший_край_в_пределах_допуска() {
        // Края (99..142, 33..63): X → левый край до 100 (+1 против -2 у правого),
        // Y → правый (нижний) край до 60 (-3 против +7 у верхнего).
        let out = snap_move(
            r(99.0, 33.0, 43.0, 30.0),
            0.0,
            0.0,
            &[],
            &SnapConfig::default(),
        );
        assert_eq!(out.dx, 1.0);
        assert_eq!(out.dy, -3.0);
        assert!(out.guides_x.is_empty() && out.guides_y.is_empty());
        assert_eq!(out.source, SnapSource::Grid);
    }

    #[test]
    fn сетка_молчит_за_допуском() {
        // Края 90/110 ровно посередине между линиями 100/120: дельта ±10 > 8.
        let out = snap_move(
            r(90.0, 0.0, 20.0, 10.0),
            0.0,
            0.0,
            &[],
            &SnapConfig::default(),
        );
        assert_eq!(out.dx, 0.0);
        assert_eq!(out.dy, 0.0);
        assert!(out.guides_x.is_empty() && out.guides_y.is_empty());
    }

    #[test]
    fn допуск_толкуется_в_экранных_пикселях() {
        // Правый край moving желает 112, направляющая по левому краю 100:
        // дельта 12 world. При zoom 0.5 это 6 экранных px (<= 8) — снап;
        // при zoom 1.0 — 12 экранных px (> 8) — молчание.
        let cand = [r(100.0, 500.0, 50.0, 50.0)];
        let zoomed_out = SnapConfig {
            to_grid: false,
            zoom: 0.5,
            ..SnapConfig::default()
        };
        let out = snap_move(r(0.0, 0.0, 100.0, 50.0), 12.0, 0.0, &cand, &zoomed_out);
        assert_eq!(out.dx, 0.0);
        assert_eq!(out.guides_x, vec![100.0]);

        let normal = SnapConfig {
            to_grid: false,
            zoom: 1.0,
            ..SnapConfig::default()
        };
        let out = snap_move(r(0.0, 0.0, 100.0, 50.0), 12.0, 0.0, &cand, &normal);
        assert_eq!(out.dx, 12.0);
        assert!(out.guides_x.is_empty());
    }

    #[test]
    fn направляющая_по_краю_соседа() {
        // Правый край moving (162) в допуске от левого края соседа (160).
        let cand = [r(160.0, 500.0, 50.0, 50.0)];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            162.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 160.0);
        assert_eq!(out.guides_x, vec![160.0]);
        assert_eq!(out.source, SnapSource::Guide);
    }

    #[test]
    fn направляющая_по_центру_соседа() {
        // Центр moving (120) совпадает с центром соседа (120) — нулевая дельта
        // побеждает соседние края (±5).
        let cand = [r(100.0, 500.0, 40.0, 40.0)];
        let out = snap_move(
            r(0.0, 0.0, 30.0, 30.0),
            105.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 105.0);
        assert_eq!(out.guides_x, vec![120.0]);
        assert_eq!(out.source, SnapSource::Guide);
    }

    #[test]
    fn направляющая_молчит_за_допуском() {
        // Все пары «якорь moving ↔ ось соседа» дальше 8 world px — ни снапа,
        // ни линии (п.8-9).
        let cand = [r(100.0, 500.0, 40.0, 40.0)];
        let out = snap_move(
            r(0.0, 0.0, 30.0, 30.0),
            160.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 160.0);
        assert!(out.guides_x.is_empty() && out.guides_y.is_empty());
        assert_eq!(out.source, SnapSource::Grid);
    }

    #[test]
    fn majority_выбирает_координату_с_большинством_соседей() {
        // Координата 104 поддержана двумя соседями (B, D), 100 — одним (A):
        // при обеих дельтах в допуске побеждает большинство (п.10).
        let cand = [
            r(100.0, 500.0, 50.0, 50.0),
            r(104.0, 600.0, 50.0, 50.0),
            r(104.0, 700.0, 50.0, 50.0),
        ];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            63.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 64.0); // правый край → 104, а не 100
        assert_eq!(out.guides_x, vec![104.0]);
    }

    #[test]
    fn majority_при_равенстве_берёт_минимальную_дельту() {
        // По одному соседу на координату 100 (дельта -2) и 106 (дельта +4):
        // равенство счётчиков → минимальная |дельта| (п.10).
        let cand = [r(100.0, 500.0, 50.0, 50.0), r(106.0, 600.0, 50.0, 50.0)];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            62.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 60.0);
        assert_eq!(out.guides_x, vec![100.0]);
    }

    #[test]
    fn арбитраж_сетка_против_направляющей_по_минимальной_дельте() {
        // Желаемый правый край 102: сетка даёт -2 (линия 100).
        // Сосед на 103: guide-дельта +1 < 2 → направляющая побеждает (п.11).
        let near = [r(103.0, 500.0, 50.0, 50.0)];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            62.0,
            0.0,
            &near,
            &SnapConfig::default(),
        );
        assert_eq!(out.dx, 63.0);
        assert_eq!(out.guides_x, vec![103.0]);
        assert_eq!(out.source, SnapSource::Guide);

        // Сосед на 105: guide-дельта +3 > 2 → сетка побеждает, линий нет.
        let far = [r(105.0, 500.0, 50.0, 50.0)];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            62.0,
            0.0,
            &far,
            &SnapConfig::default(),
        );
        assert_eq!(out.dx, 60.0);
        assert!(out.guides_x.is_empty());
        assert_eq!(out.source, SnapSource::Grid);
    }

    #[test]
    fn равные_интервалы_выравнивают_центр_между_соседями() {
        // Центры соседей 100 и 300, желаемый центр moving 197: зазоры 97/103
        // (разница 6 <= 8) → снап на центр 200, зазоры по 100 (п.13).
        let cand = [r(80.0, 100.0, 40.0, 40.0), r(280.0, 100.0, 40.0, 40.0)];
        let out = snap_move(
            r(0.0, 100.0, 40.0, 40.0),
            177.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 180.0);
        assert_eq!(out.guides_x, vec![200.0]);
        assert_eq!(out.source, SnapSource::Guide);
    }

    #[test]
    fn равные_интервалы_требуют_фланкирования() {
        // Оба соседа левее moving — фланкирования нет, равные интервалы не
        // применяются (п.13).
        let cand = [r(80.0, 90.0, 40.0, 40.0), r(120.0, 90.0, 40.0, 40.0)];
        let out = snap_move(
            r(0.0, 100.0, 40.0, 40.0),
            177.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 177.0);
        assert!(out.guides_x.is_empty() && out.guides_y.is_empty());
        assert_eq!(out.source, SnapSource::Grid);
    }

    #[test]
    fn равные_интервалы_требуют_поперечной_близости() {
        // Соседи фланкируют по X, но их центры на 10 px выше по Y (вне
        // допуска) — ряд не ровный, снап не применяется (п.13).
        let cand = [r(80.0, 90.0, 40.0, 40.0), r(280.0, 90.0, 40.0, 40.0)];
        let out = snap_move(
            r(0.0, 100.0, 40.0, 40.0),
            177.0,
            0.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dx, 177.0);
        assert!(out.guides_x.is_empty());
    }

    #[test]
    fn коллизия_останавливает_движение_на_границе_зазора() {
        // Движение вправо на 200 при зазоре 10: правый край замирает на
        // 190 = 200 - 10 (п.15).
        let cand = [r(200.0, 0.0, 50.0, 50.0)];
        let cfg = SnapConfig {
            collision_gap: 10.0,
            ..cfg_no_snap()
        };
        let out = snap_move(r(0.0, 0.0, 50.0, 50.0), 200.0, 0.0, &cand, &cfg);
        assert_eq!(out.dx, 140.0);
        assert_eq!(out.dy, 0.0);
    }

    #[test]
    fn коллизия_останавливает_подход_справа() {
        // Движение влево на 100: левый край замирает на 260 = 250 + 10.
        let cand = [r(200.0, 0.0, 50.0, 50.0)];
        let cfg = SnapConfig {
            collision_gap: 10.0,
            ..cfg_no_snap()
        };
        let out = snap_move(r(300.0, 0.0, 50.0, 50.0), -100.0, 0.0, &cand, &cfg);
        assert_eq!(out.dx, -40.0);
    }

    #[test]
    fn коллизия_не_мешает_уходу_из_зоны_зазора() {
        // moving УЖЕ пересекает зону зазора (правый край 200 > 190):
        // движение вправо не клампится — запрещено телепортировать назад.
        let cand = [r(200.0, 0.0, 50.0, 50.0)];
        let cfg = SnapConfig {
            collision_gap: 10.0,
            ..cfg_no_snap()
        };
        let out = snap_move(r(150.0, 0.0, 50.0, 50.0), 50.0, 0.0, &cand, &cfg);
        assert_eq!(out.dx, 50.0);
    }

    #[test]
    fn коллизия_выключена_при_нулевом_зазоре() {
        // collision_gap = 0 — функция выкл (п.15): сквозной проход.
        let cand = [r(200.0, 0.0, 50.0, 50.0)];
        let out = snap_move(r(0.0, 0.0, 50.0, 50.0), 200.0, 0.0, &cand, &cfg_no_snap());
        assert_eq!(out.dx, 200.0);
    }

    #[test]
    fn разделение_кандидатов_и_препятствий_группа_проходима() {
        // FR-012 v3: snap_move_ex с раздельными наборами — «группа» есть в
        // кандидатах направляющих, но НЕ в препятствиях collision: нода
        // свободно проходит сквозь рамку группы (жест втягивания), а
        // snap_move (слитная семантика) по-прежнему клампит по тем же
        // кандидатам — регресс-защита обоих поведений.
        let group_rect = [r(100.0, 0.0, 400.0, 300.0)];
        let cfg = SnapConfig {
            collision_gap: 10.0,
            ..cfg_no_snap()
        };
        let out = snap_move_ex(
            r(0.0, 0.0, 50.0, 50.0),
            200.0,
            100.0,
            &group_rect,
            &[],
            &cfg,
        );
        assert_eq!(out.dx, 200.0, "группа не препятствие — дельта не клампится");
        assert_eq!(out.dy, 100.0);
        // Слитная семантика (кандидаты = препятствия) сохранена для движка:
        let out = snap_move_ex(
            r(0.0, 0.0, 50.0, 50.0),
            200.0,
            100.0,
            &group_rect,
            &group_rect,
            &cfg,
        );
        assert_eq!(
            out.dx, 40.0,
            "рамка как препятствие останавливает на зазоре"
        );
    }

    #[test]
    fn детерминизм_повторный_вызов_даёт_идентичный_результат() {
        // п.20: та же сцена + те же параметры → бит-в-бит тот же outcome.
        let cand = [
            r(100.0, 0.0, 50.0, 50.0),
            r(100.0, 200.0, 50.0, 50.0),
            r(280.0, 0.0, 40.0, 40.0),
        ];
        let cfg = SnapConfig {
            collision_gap: 12.0,
            ..SnapConfig::default()
        };
        let a = snap_move(r(0.0, 0.0, 40.0, 40.0), 177.0, -2.0, &cand, &cfg);
        let b = snap_move(r(0.0, 0.0, 40.0, 40.0), 177.0, -2.0, &cand, &cfg);
        assert_eq!(a, b);
        // Зафиксированный вручную оракул: сетка ведёт желаемую дельту X 177
        // к 180 (+3), Y — majority-центр 0 (два соседа) → dy 0. Collision
        // (зазор 12) останавливает проход сквозь соседа A (100..150):
        // правый край замирает на границе зазора 88 → итоговая дельта 48.
        assert_eq!(a.dx, 48.0);
        assert_eq!(a.dy, 0.0);
        assert_eq!(a.guides_y, vec![0.0]);
        assert_eq!(a.source, SnapSource::Guide);
    }

    #[test]
    fn выключенные_тумблеры_гасят_весь_снап() {
        // Мастер-отключение (п.5): и сетка, и направляющие молчат, дельта
        // проходит как есть.
        let cand = [r(41.0, 500.0, 50.0, 50.0)];
        let out = snap_move(r(0.0, 0.0, 40.0, 40.0), 1.0, 0.0, &cand, &cfg_no_snap());
        assert_eq!(out.dx, 1.0);
        assert!(out.guides_x.is_empty() && out.guides_y.is_empty());
        assert_eq!(out.source, SnapSource::Grid);
    }

    #[test]
    fn направляющие_работают_по_вертикальной_оси_y() {
        // Нижний край moving (355) в допуске от нижнего края соседа (350).
        let cand = [r(100.0, 300.0, 50.0, 50.0)];
        let out = snap_move(
            r(0.0, 0.0, 40.0, 40.0),
            0.0,
            315.0,
            &cand,
            &cfg_only_guides(),
        );
        assert_eq!(out.dy, 310.0);
        assert_eq!(out.guides_y, vec![350.0]);
        assert!(out.guides_x.is_empty());
        assert_eq!(out.source, SnapSource::Guide);
    }

    #[test]
    fn sub_grid_при_крупном_зуме_использует_половинный_шаг() {
        // zoom 2.0 → шаг 10: правый край 29.5 тянется к 30 (+0.5); при
        // шаге 20 (zoom 1.0) тот же край тянулся бы к 20 (-4.5) — значения
        // разные, что доказывает смену шага.
        let sub = SnapConfig {
            zoom: 2.0,
            ..cfg_only_grid()
        };
        let out = snap_move(r(24.5, 0.0, 5.0, 5.0), 0.0, 0.0, &[], &sub);
        assert_eq!(out.dx, 0.5);

        let minor = SnapConfig {
            zoom: 1.0,
            ..cfg_only_grid()
        };
        let out = snap_move(r(24.5, 0.0, 5.0, 5.0), 0.0, 0.0, &[], &minor);
        assert_eq!(out.dx, -4.5);
    }

    #[test]
    fn coarse_grid_при_мелком_зуме_использует_major_шаг() {
        // zoom 0.4 → шаг 100 (не minor 20): левый край 115 тянется к 100
        // (дельта -15; minor-шаг дал бы 120/+5... у правого края 120 дельта
        // к линии 100 = -20). Допуск в world вырос до 20 (= 8 / 0.4).
        let coarse = SnapConfig {
            zoom: 0.4,
            ..cfg_only_grid()
        };
        let out = snap_move(r(115.0, 0.0, 5.0, 5.0), 0.0, 0.0, &[], &coarse);
        assert_eq!(out.dx, -15.0);
    }

    // --- FR-038 T-038.5: batch-операции выделения (п.16-17) ---

    /// Выравнивание в ряд (axis=Y): центры на ОДНОЙ горизонтали; опорная
    /// ось — СРЕДНЕЕ центров Y (правило зафиксировано, п.20); X и размеры
    /// не меняются.
    #[test]
    fn выравнивание_ряд_ставит_центры_на_среднюю_горизонталь() {
        // Центры Y: 25, 120, 40 → среднее 185/3
        let mut rects = vec![
            r(0.0, 0.0, 100.0, 50.0),
            r(200.0, 90.0, 80.0, 60.0),
            r(400.0, 30.0, 40.0, 20.0),
        ];
        align_centers(&mut rects, AlignAxis::Y);
        let expected = (25.0 + 120.0 + 40.0) / 3.0;
        for (i, rect) in rects.iter().enumerate() {
            assert!(
                (rect.cy() - expected).abs() < 1e-3,
                "центр {i} на оси: {} vs {expected}",
                rect.cy()
            );
        }
        // X и размеры не тронуты
        assert_eq!(rects[0].x, 0.0);
        assert_eq!(rects[1].x, 200.0);
        assert_eq!(rects[2].x, 400.0);
        assert_eq!((rects[0].w, rects[0].h), (100.0, 50.0));
        assert_eq!((rects[1].w, rects[1].h), (80.0, 60.0));
        assert_eq!((rects[2].w, rects[2].h), (40.0, 20.0));
    }

    /// Выравнивание в колонну (axis=X): центры на одной вертикали; Y не меняется.
    #[test]
    fn выравнивание_колонна_ставит_центры_на_среднюю_вертикаль() {
        // Центры X: 50, 240, 420 → среднее 710/3
        let mut rects = vec![
            r(0.0, 0.0, 100.0, 50.0),
            r(200.0, 90.0, 80.0, 60.0),
            r(400.0, 30.0, 40.0, 20.0),
        ];
        align_centers(&mut rects, AlignAxis::X);
        let expected = (50.0 + 240.0 + 420.0) / 3.0;
        for rect in &rects {
            assert!((rect.cx() - expected).abs() < 1e-3);
        }
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[1].y, 90.0);
        assert_eq!(rects[2].y, 30.0);
    }

    /// Меньше двух элементов — no-op (выравнивать нечего).
    #[test]
    fn выравнивание_меньше_двух_элементов_nop() {
        let mut one = vec![r(10.0, 20.0, 30.0, 40.0)];
        align_centers(&mut one, AlignAxis::Y);
        assert_eq!(one[0], r(10.0, 20.0, 30.0, 40.0));
        let mut empty: Vec<SnapRect> = Vec::new();
        align_centers(&mut empty, AlignAxis::X);
        assert!(empty.is_empty());
    }

    /// Группа — обычный rect по габаритам (п.14-семантика): большой bbox
    /// группы выравнивается как любой элемент, поперечное не трогается.
    #[test]
    fn выравнивание_работает_по_габаритам_группы() {
        // Группа 300×200 рядом с двумя мелкими: ряд по Y
        let mut rects = vec![
            r(1000.0, 500.0, 300.0, 200.0), // группа, центр Y 600
            r(0.0, 0.0, 50.0, 40.0),        // центр Y 20
            r(50.0, 108.0, 60.0, 24.0),     // центр Y 120
        ];
        align_centers(&mut rects, AlignAxis::Y);
        let expected = (600.0 + 20.0 + 120.0) / 3.0;
        for rect in &rects {
            assert!((rect.cy() - expected).abs() < 1e-3);
        }
        assert_eq!(rects[0].x, 1000.0, "группа: X не тронут");
        assert_eq!(rects[0].w, 300.0, "группа: габарит не меняется");
    }

    /// Распределение по X: разные ширины — равные ЗАЗОРЫ между краями
    /// соседних; span (крайний левый/правый) сохранён; Y не тронут.
    #[test]
    fn распределение_по_x_даёт_равные_зазоры_между_краями() {
        let mut rects = vec![
            r(0.0, 0.0, 100.0, 10.0),
            r(150.0, 0.0, 50.0, 10.0),
            r(500.0, 0.0, 30.0, 10.0),
        ];
        distribute_evenly(&mut rects, AlignAxis::X);
        // span 0..530, сумма ширин 180 → зазор (530-180)/2 = 175
        assert_eq!(rects[0].x, 0.0, "первый — на начале span");
        assert_eq!(rects[1].x, 100.0 + 175.0);
        assert_eq!(
            rects[2].x,
            100.0 + 175.0 + 50.0 + 175.0,
            "впритык к span_end"
        );
        for rect in &rects {
            assert_eq!(rect.y, 0.0, "Y не тронут");
        }
    }

    /// Распределение по Y (колонна): то же правило вдоль Y, X не тронут.
    #[test]
    fn распределение_по_y_даёт_равные_зазоры_между_краями() {
        let mut rects = vec![
            r(0.0, 0.0, 10.0, 40.0),
            r(10.0, 60.0, 10.0, 80.0),
            r(5.0, 300.0, 10.0, 20.0),
        ];
        distribute_evenly(&mut rects, AlignAxis::Y);
        // По центрам Y: 20, 100, 310; span 0..320, сумма высот 140 → зазор 90
        assert_eq!(rects[0].y, 0.0);
        assert_eq!(rects[1].y, 40.0 + 90.0);
        assert_eq!(rects[2].y, 40.0 + 90.0 + 80.0 + 90.0);
        assert_eq!(rects[0].x, 0.0, "X не тронут");
        assert_eq!(rects[1].x, 10.0);
        assert_eq!(rects[2].x, 5.0);
    }

    /// Порядок входа не важен: сортировка по центру оси — раскладка та же,
    /// что у заведомо упорядоченного входа.
    #[test]
    fn распределение_сортирует_по_центру_оси() {
        let mut scrambled = vec![
            r(500.0, 0.0, 30.0, 10.0),
            r(0.0, 0.0, 100.0, 10.0),
            r(150.0, 0.0, 50.0, 10.0),
        ];
        distribute_evenly(&mut scrambled, AlignAxis::X);
        let mut sorted = vec![
            r(0.0, 0.0, 100.0, 10.0),
            r(150.0, 0.0, 50.0, 10.0),
            r(500.0, 0.0, 30.0, 10.0),
        ];
        distribute_evenly(&mut sorted, AlignAxis::X);
        // По размеру элемента — итоговая позиция совпадает
        for size in [100.0, 50.0, 30.0] {
            let a = scrambled.iter().find(|rt| rt.w == size).expect("w");
            let b = sorted.iter().find(|rt| rt.w == size).expect("w");
            assert_eq!((a.x, a.y), (b.x, b.y), "элемент ширины {size}");
        }
    }

    /// N=2 — no-op: крайние элементы уже образуют границы span (зазор
    /// помещает их ровно на места).
    #[test]
    fn распределение_двух_элементов_nop() {
        let mut rects = vec![r(0.0, 5.0, 100.0, 10.0), r(300.0, 6.0, 50.0, 10.0)];
        let before = rects.clone();
        distribute_evenly(&mut rects, AlignAxis::X);
        assert_eq!(rects, before);
    }

    /// Меньше двух элементов — no-op.
    #[test]
    fn распределение_меньше_двух_элементов_nop() {
        let mut one = vec![r(10.0, 20.0, 30.0, 40.0)];
        distribute_evenly(&mut one, AlignAxis::X);
        assert_eq!(one[0], r(10.0, 20.0, 30.0, 40.0));
    }

    /// Идемпотентность распределения: уже равные зазоры — второй вызов
    /// ничего не меняет (повторный пункт меню не «гуляет»).
    #[test]
    fn распределение_идемпотентно() {
        let mut rects = vec![
            r(0.0, 0.0, 100.0, 10.0),
            r(150.0, 0.0, 50.0, 10.0),
            r(500.0, 0.0, 30.0, 10.0),
        ];
        distribute_evenly(&mut rects, AlignAxis::X);
        let after_first = rects.clone();
        distribute_evenly(&mut rects, AlignAxis::X);
        assert_eq!(rects, after_first);
    }

    /// Детерминизм (п.20): тот же вход — бит-в-бит тот же выход для обеих
    /// операций и обеих осей.
    #[test]
    fn детерминизм_batch_операций_повтор_даёт_бит_в_бит() {
        let input = vec![
            r(0.0, 0.0, 100.0, 50.0),
            r(200.0, 90.0, 80.0, 60.0),
            r(400.0, 30.0, 40.0, 20.0),
        ];
        for axis in [AlignAxis::X, AlignAxis::Y] {
            let mut a = input.clone();
            let mut b = input.clone();
            align_centers(&mut a, axis);
            align_centers(&mut b, axis);
            assert_eq!(a, b, "align_centers {axis:?}");
            let mut a = input.clone();
            let mut b = input.clone();
            distribute_evenly(&mut a, axis);
            distribute_evenly(&mut b, axis);
            assert_eq!(a, b, "distribute_evenly {axis:?}");
        }
    }
}
