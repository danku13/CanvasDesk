//! Чистые helper-функции и типы-помощники `app` — геометрия/текст/снап,
//! линияж explain (FR-046), спилл-хиты, шаблонные карточки.
//!
//! Выделены из `app.rs` сессией рефакторинга 2026-09-24 (этап 1) без
//! изменения поведения: функции не зависят от `App`, состояние получают
//! параметрами. Паттерн дочернего модуля — как `ui_registry` (FR-052):
//! `use super::*` даёт доступ к импортам и приватным элементам родителя,
//! обратный импорт в родителе — явным списком (`use support::{…}`).

use super::*;

/// FR-046: мост «примитив design-токенов → glyphon::Color» (константный):
/// текстовые цвета диалогов/тостов берутся из `canvas_core::tokens`.
pub(super) const fn token_color(rgb: [u8; 3]) -> Color {
    Color::rgb(rgb[0], rgb[1], rgb[2])
}

/// Опора для центрируемого screen-текста внутри `rect` (подписи кнопок/чипов).
/// Контракт рендера (`ScreenText`, text.rs): `origin` — ЛЕВЫЙ край области
/// выравнивания, Center центрирует строку в `[origin_x, origin_x + width]`.
/// Поэтому origin ставим на `rect[0] + inset`, а ширину берём с двусторонним
/// инсетом — итоговая область по центру rect. Передача центра rect как
/// origin сдвигает текст вправо на полширины области (дефект CR-015).
pub(super) fn centered_box(rect: [f32; 4], inset: f32) -> ([f32; 2], f32) {
    let width = (rect[2] - inset * 2.0).max(0.0);
    ([rect[0] + inset, rect[1]], width)
}

/// Усечение строки до `max` символов с многоточием (FR-044: ширина пилюль
/// и строк stage оценивается по числу символов — средняя advance моно 12 px;
/// точное измерение недоступно на стороне приложения — шейпинг в TextSystem).
pub(super) fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Screen-прямоугольник → world-квад (паттерн stage: позиция через
/// screen_to_world, размер/радиус делятся на зум — константный экранный
/// размер при любом зуме).
pub(super) fn screen_rect_quad(
    camera: &Camera,
    viewport: Vec2,
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    radius: f32,
) -> CardInstance {
    let zoom = camera.zoom();
    CardInstance {
        pos: camera.screen_to_world([rect[0], rect[1]], viewport),
        size: [rect[2] / zoom, rect[3] / zoom],
        fill,
        border,
        params: [radius / zoom, 0.0, 0.0, 1.0],
    }
}

/// FR-059 (волна 1 кита): конвертация items [`Painter`] (canvas-ui — ДАННЫЕ,
/// инвариант G7) в инстансы screen-полосы кадра — сырые логические px
/// (конвенция полос: рендер конвертирует screen→world ровно один раз —
/// `renderer::screen_instance_to_world`; радиус в params — логические px).
/// Тексты полос рисуются в screen-space — конвертации не требуют.
/// Порядок items = draw-порядок — сохранён дословно (0 визуального скачка:
/// та же раскладка, те же слоты, что при прежней ручной сборке квадов).
pub(super) fn paint_items_to_band(
    items: Vec<PaintItem>,
    quads: &mut Vec<CardInstance>,
    texts: &mut Vec<OwnedScreenText>,
) {
    for item in items {
        match item {
            PaintItem::Rect {
                rect,
                fill,
                border,
                radius,
            } => quads.push(CardInstance {
                pos: [rect.x, rect.y],
                size: [rect.w, rect.h],
                fill,
                border,
                params: [radius, 0.0, 0.0, 1.0],
            }),
            PaintItem::Text {
                area,
                text,
                color,
                size,
                align,
            } => texts.push(OwnedScreenText {
                text,
                origin: [area.x, area.y],
                width: area.w,
                font_size: size,
                color: crate::kit_ui::color4(color),
                align: match align {
                    PaintAlign::Left => TextAlign::Left,
                    PaintAlign::Center => TextAlign::Center,
                },
            }),
            // FR-068 W1: клип — прозрачный проход для consumer-обхода:
            // дети конвертируются как обычные items (draw-порядок сохранён);
            // отсечение — scissor FR-056 на стороне рендера, не здесь.
            PaintItem::ClipRect { items, .. } => paint_items_to_band(items, quads, texts),
        }
    }
}

/// FR-059: вариант [`paint_items_to_band`] для модального прохода main
/// stage (`stage_instances` — world-конвенция, как прежние ручные пушы):
/// Rect → screen_to_world + деление на zoom (радиус тоже), Text —
/// screen-space без конвертации (тексты stage рисуются после квадов).
pub(super) fn paint_items_to_stage(
    items: Vec<PaintItem>,
    camera: &Camera,
    viewport: Vec2,
    zoom: f32,
    quads: &mut Vec<CardInstance>,
    texts: &mut Vec<OwnedScreenText>,
) {
    for item in items {
        match item {
            PaintItem::Rect {
                rect,
                fill,
                border,
                radius,
            } => quads.push(CardInstance {
                pos: camera.screen_to_world([rect.x, rect.y], viewport),
                size: [rect.w / zoom, rect.h / zoom],
                fill,
                border,
                params: [radius / zoom, 0.0, 0.0, 1.0],
            }),
            PaintItem::Text {
                area,
                text,
                color,
                size,
                align,
            } => texts.push(OwnedScreenText {
                text,
                origin: [area.x, area.y],
                width: area.w,
                font_size: size,
                color: crate::kit_ui::color4(color),
                align: match align {
                    PaintAlign::Left => TextAlign::Left,
                    PaintAlign::Center => TextAlign::Center,
                },
            }),
            // FR-068 W1: клип — прозрачный проход для consumer-обхода:
            // дети конвертируются как обычные items (draw-порядок сохранён);
            // отсечение — scissor FR-056 на стороне рендера, не здесь.
            PaintItem::ClipRect { items, .. } => {
                paint_items_to_stage(items, camera, viewport, zoom, quads, texts)
            }
        }
    }
}

/// FR-059: приглушение цвета текста в f32-представлении (зеркало
/// `dim_text_color` без промежуточного u8-округления — итоговый округ
/// делает конвертация в Color на границе кадра).
pub(super) fn dim_color4(c: [f32; 4], alpha: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * alpha]
}

/// Кружок с центром в screen-точке → world-инстанс (паттерн `cards::dot`).
pub(super) fn screen_dot(
    camera: &Camera,
    viewport: Vec2,
    center: [f32; 2],
    diameter: f32,
    fill: [f32; 4],
) -> CardInstance {
    let zoom = camera.zoom();
    let world = camera.screen_to_world(center, viewport);
    let r = diameter / 2.0 / zoom;
    CardInstance {
        pos: [world[0] - r, world[1] - r],
        size: [diameter / zoom, diameter / zoom],
        fill,
        border: [0.0; 4],
        params: [r, 0.0, 0.0, 1.0],
    }
}

/// Точки кубической безье (`n` отсчётов, включая концы) — ветки дерева
/// (прототип v4: от правого порта родителя к левому порту ребёнка).
pub(super) fn bezier_samples(points: [[f32; 2]; 4], n: usize) -> Vec<[f32; 2]> {
    let [p0, c0, c1, p1] = points;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n.max(1) as f32;
            let u = 1.0 - t;
            [
                u * u * u * p0[0]
                    + 3.0 * u * u * t * c0[0]
                    + 3.0 * u * t * t * c1[0]
                    + t * t * t * p1[0],
                u * u * u * p0[1]
                    + 3.0 * u * u * t * c0[1]
                    + 3.0 * u * t * t * c1[1]
                    + t * t * t * p1[1],
            ]
        })
        .collect()
}

/// Сборка lineage-дерева по снапшоту сцены (общая для фонового потока и
/// фолбэка): Ready по значениям либо Cycled-топология (AC-2.4).
pub(super) fn build_lineage_snapshot(
    canvas: &Canvas,
    solutions: &flow::FlowSolutions,
    cycle: Option<&flow::CycleError>,
    root: LineageNodeId,
) -> Result<canvas_core::LineageTree, canvas_core::LineageError> {
    match cycle {
        Some(cycle) => canvas_core::build_lineage(
            &canvas.clone(),
            canvas_core::LineageFlow::Cycled(cycle),
            root,
        ),
        None => {
            let data = canvas_core::DataSnapshots::new();
            canvas_core::build_lineage(
                canvas,
                canvas_core::LineageFlow::Ready {
                    solutions,
                    data: &data,
                },
                root,
            )
        }
    }
}

/// Сборка ПАРЫ деревьев X3 (основное + база) одним проходом: основной —
/// из `solutions` (flow_active — с подменами), база — из `base_solutions`
/// (flow_baseline — без подмен; None — дельты не нужны, AC-4.2).
pub(super) fn build_lineage_outcome(
    canvas: &Canvas,
    solutions: &flow::FlowSolutions,
    base_solutions: Option<&flow::FlowSolutions>,
    cycle: Option<&flow::CycleError>,
    root: LineageNodeId,
) -> explain_ui::LineageOutcome {
    let base = base_solutions.map(|flow| {
        // Подмены в базе нет по определению — цикл там тот же (движок
        // падает одинаково), но Cycled-база не содержит значений — дельты
        // всё равно пусты; Ready-ветка достаточна.
        build_lineage_snapshot(canvas, flow, None, root.clone())
    });
    explain_ui::LineageOutcome {
        tree: build_lineage_snapshot(canvas, solutions, cycle, root),
        base,
    }
}

/// PRD-0007 (X2, AC-1.2/G5): запуск сборки дерева — натив: фоновый поток
/// (UI не блокируется, честный лоадер крутится), wasm/сбой потока:
/// синхронный результат (Ready на первом же poll).
/// X3 (AC-4.2): при активном what-if тем же проходом строится БАЗА —
/// источник дельт в дереве.
pub(super) fn spawn_lineage_build(
    canvas: &Canvas,
    solutions: &flow::FlowSolutions,
    base_solutions: Option<&flow::FlowSolutions>,
    cycle: Option<&flow::CycleError>,
    root: LineageNodeId,
) -> ExplainBuild {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_canvas = canvas.clone();
        let worker_solutions = solutions.clone();
        let worker_base = base_solutions.cloned();
        let worker_cycle = cycle.cloned();
        let worker_root = root.clone();
        let spawned = std::thread::Builder::new()
            .name("lineage-build".into())
            .spawn(move || {
                let outcome = build_lineage_outcome(
                    &worker_canvas,
                    &worker_solutions,
                    worker_base.as_ref(),
                    worker_cycle.as_ref(),
                    worker_root,
                );
                let _ = tx.send(outcome);
            });
        match spawned {
            Ok(_handle) => ExplainBuild::Native(rx),
            // Поток не поднялся — синхронный фолбэк (окно честно ждёт)
            Err(_) => ExplainBuild::Done(build_lineage_outcome(
                canvas,
                solutions,
                base_solutions,
                cycle,
                root,
            )),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Однопоточный рантайм: сборка синхронная (Ready на первом poll)
        ExplainBuild::Done(build_lineage_outcome(
            canvas,
            solutions,
            base_solutions,
            cycle,
            root,
        ))
    }
}

/// PRD-0007 (F-4/AC-3.1): набор подсветки цепочки на канвасе из дерева
/// снапшота (F-5: одна модель для окна и подсветки). Узлы — все `node_id`
/// дерева, рёбра — все `via.edge_id`; индексы отсортированы (контракт
/// FocusSet). Пучки подсвечивает OR-семантика рендера (линию пучка рисует
/// доминанта); невалидные id (нода/ребро удалены) молча пропускаются.
pub(super) fn explain_chain_focus(canvas: &Canvas, tree: &canvas_core::LineageTree) -> FocusSet {
    let mut set = FocusSet::empty();
    let node_index: std::collections::HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let edge_index: std::collections::HashMap<&str, usize> = canvas
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    for node in &tree.nodes {
        if let Some(&i) = node_index.get(node.node_id.as_str()) {
            set.nodes.push(i);
        }
        for child in &node.children {
            let Some(via) = &child.via else { continue };
            if let Some(&i) = edge_index.get(via.edge_id.as_str()) {
                set.edges.push(i);
            }
        }
    }
    set.nodes.sort_unstable();
    set.nodes.dedup();
    set.edges.sort_unstable();
    set.edges.dedup();
    set
}

/// Данные snap-расчёта одного кадра драга (T-038.4): достаточно и для
/// перемещения (live-collision), и для предпросмотра, и для отпускания —
/// отпускание пересчитывает кадр с теми же входами (детерминизм п.20:
/// результат совпадает с последним предпросмотром).
pub(super) struct SnapFrame {
    /// Дельта перемещения ЭТОГО кадра: свободная (world−grab) или ужатая
    /// live-collision (п.15, если `snap_collision` включён).
    pub(super) eff_delta: [f32; 2],
    /// Полный расчёт снапа от bbox-origins с желаемой дельтой `eff_delta`:
    /// итоговая дельта (grid/guides/anchor + collision поверх) и оси
    /// направляющих. Итоговые позиции = origin + outcome.dx/dy.
    pub(super) outcome: SnapOutcome,
    /// bbox перемещаемого набора в позиции кадра (origins + eff_delta),
    /// [x0, y0, x1, y1] — вход `GuidesFrame::from_snap` для ghost.
    pub(super) bbox_current: [f32; 4],
    /// Допуск в world px (`snap_tolerance_px / zoom`) — для нелинейного
    /// усиления направляющих (п.8, `GuidesFrame.tolerance`).
    pub(super) tolerance_world: f32,
}

/// bbox перемещаемого набора ПО ИСХОДНЫМ позициям (origins): размеры берутся
/// из текущих нод — drag размеры не меняет. Групповой drag — union bbox
/// (п.14: дети групп уже в origins, дубликатов нет). Пустой/битый набор —
/// None (снап не применяется).
pub(super) fn drag_bbox(canvas: &Canvas, origins: &[(usize, Vec2)]) -> Option<SnapRect> {
    let mut acc: Option<SnapRect> = None;
    for (index, origin) in origins {
        let node = canvas.nodes.get(*index)?;
        let rect = SnapRect {
            x: origin[0],
            y: origin[1],
            w: node.width,
            h: node.height,
        };
        acc = Some(match acc {
            None => rect,
            Some(acc) => {
                let left = acc.x.min(rect.x);
                let top = acc.y.min(rect.y);
                let right = (acc.x + acc.w).max(rect.x + rect.w);
                let bottom = (acc.y + acc.h).max(rect.y + rect.h);
                SnapRect {
                    x: left,
                    y: top,
                    w: right - left,
                    h: bottom - top,
                }
            }
        });
    }
    acc
}

/// Кандидаты снапа (T-038.4): bbox видимых нод ВНЕ перемещаемого набора
/// (края/центры/середины движок берёт из bbox сам). Группа — обычная нода
/// в `canvas.nodes`: её bbox и есть кандидат (п.14). Чистая функция.
pub(super) fn snap_candidates(
    canvas: &Canvas,
    visible: &[usize],
    moving: &[usize],
) -> Vec<SnapRect> {
    visible
        .iter()
        .filter(|index| !moving.contains(index))
        .filter_map(|index| canvas.nodes.get(*index))
        .map(|node| SnapRect {
            x: node.x,
            y: node.y,
            w: node.width,
            h: node.height,
        })
        .collect()
}

/// Дельта притяжения точки к ближайшей линии сетки (п.4 anchor-надстройка):
/// в пределах допуска — дельта до ближайшей линии (кратной `step`), иначе 0 —
/// сетка молчит, как в движке (п.1-3). Отрицательные координаты корректны
/// ((pos/step).round() математически одинаков).
pub(super) fn anchor_grid_delta(pos: f32, step: f32, tol: f32) -> f32 {
    let delta = (pos / step).round() * step - pos;
    if delta.abs() <= tol {
        delta
    } else {
        0.0
    }
}

/// Допуск в world px из экранных (п.6): screen = world·zoom → деление на
/// зум; некорректный зум — фолбэк 1.0 (зеркало движка, snap::tol_world).
pub(super) fn snap_tolerance_world(tolerance_px: f32, zoom: f32) -> f32 {
    if zoom > 0.0 {
        tolerance_px / zoom
    } else {
        tolerance_px
    }
}

/// FR-038 (п.4): anchor-надстройка над движком — тонкая коррекция grid-части.
/// `BoundingBox` — движок как есть (ближайший край bbox к линии). `Corner`/
/// `Center` — движок вызывается БЕЗ сетки (to_grid=false), grid-дельта
/// считается вручную для левого-верхнего угла / центра bbox (к пересечению
/// линий); направляющие всегда от движка (п.7 — по краям/центрам); арбитраж
/// grid-vs-guide по осям — минимальная |дельта| (п.11), при равенстве —
/// направляющая (как в движке), ось, выигранная grid, направляющей НЕ
/// помечается (п.9). Collision (п.15) — финальный кламп поверх итоговой
/// дельты. Детерминизм (п.20): только арифметика входа.
pub(super) fn snap_with_anchor(
    moving: SnapRect,
    dx: f32,
    dy: f32,
    candidates: &[SnapRect],
    cfg: &SnapConfig,
    anchor: SnapAnchor,
) -> SnapOutcome {
    if matches!(anchor, SnapAnchor::BoundingBox) {
        return snap_move(moving, dx, dy, candidates, cfg);
    }
    // Движок без сетки и без collision (клампим сами после anchor-дельты —
    // иначе collision «зафиксирует» дельту до арбитража осей)
    let mut outcome = snap_move(
        moving,
        dx,
        dy,
        candidates,
        &SnapConfig {
            collision_gap: 0.0,
            ..*cfg
        },
    );
    let step = effective_grid_step(cfg);
    let tol = snap_tolerance_world(cfg.tolerance_px, cfg.zoom);
    let (ax, ay) = match anchor {
        // Угол bbox — к пересечению линий (п.4)
        SnapAnchor::Corner => (moving.x, moving.y),
        // Центр bbox — аналогично по центру (п.4)
        _ => (moving.cx(), moving.cy()),
    };
    let grid_step_valid = cfg.to_grid && step > 0.0 && step.is_finite();
    let gx = if grid_step_valid {
        anchor_grid_delta(ax + dx, step, tol)
    } else {
        0.0
    };
    let gy = if grid_step_valid {
        anchor_grid_delta(ay + dy, step, tol)
    } else {
        0.0
    };

    // Арбитраж по осям: guide (уже в outcome — там, где сработал, ось
    // помечена линией) против anchor-grid. Дельта guide по оси = outcome −
    // желаемая. Равенство — направляющая (детерминизм, как в движке п.11).
    let guide_won_x = !outcome.guides_x.is_empty();
    let guide_won_y = !outcome.guides_y.is_empty();
    let guide_dx = outcome.dx - dx;
    let guide_dy = outcome.dy - dy;
    let mut out_dx = if !guide_won_x || gx.abs() < guide_dx.abs() {
        dx + gx
    } else {
        outcome.dx
    };
    let mut out_dy = if !guide_won_y || gy.abs() < guide_dy.abs() {
        dy + gy
    } else {
        outcome.dy
    };
    // Ось, выигранная anchor-grid, направляющей не помечается (п.9)
    let mut guides_x = std::mem::take(&mut outcome.guides_x);
    let mut guides_y = std::mem::take(&mut outcome.guides_y);
    if guide_won_x && out_dx != outcome.dx {
        guides_x.clear();
    }
    if guide_won_y && out_dy != outcome.dy {
        guides_y.clear();
    }
    // Collision поверх финальной дельты (п.15) — как в движке
    if cfg.collision_gap > 0.0 {
        let (fx, fy) = clamp_collision(
            moving,
            moving.x + out_dx,
            moving.y + out_dy,
            candidates,
            cfg.collision_gap,
            out_dx,
            out_dy,
        );
        // Кламп, сдвинувший ось с выигранной позиции, гасит её направляющую
        // (п.9: линия — только там, где снап определил финальную позицию;
        // иначе рендер показывает ось, которой позиция больше не касается)
        if guide_won_x && fx != out_dx {
            guides_x.clear();
        }
        if guide_won_y && fy != out_dy {
            guides_y.clear();
        }
        out_dx = fx;
        out_dy = fy;
    }

    // Источник — по выжившим направляющим (кламп мог погасить оси)
    let source = if !guides_x.is_empty() || !guides_y.is_empty() {
        SnapSource::Guide
    } else {
        SnapSource::Grid
    };

    SnapOutcome {
        dx: out_dx,
        dy: out_dy,
        guides_x,
        guides_y,
        source,
    }
}

/// Шаг клавиатурного nudge в world px (п.22): спецификация v2 задаёт шаг
/// «в пикселях экрана» — визуальный шаг одинаков на любом зуме; перевод в
/// world — делением на зум (screen = world·zoom). Детерминировано: шаг
/// зависит только от зума кадра. Некорректный зум — фолбэк к экранным px.
pub(super) fn nudge_step_world(screen_px: f32, zoom: f32) -> f32 {
    if zoom > 0.0 {
        screen_px / zoom
    } else {
        screen_px
    }
}

/// FR-038 п.16 (T-038.5): ось «Распределить равномерно» по контексту
/// выделения. Правило (детерминизм п.20): ось с БОЛЬШИМ размахом центров —
/// ряд (размах по X больше) распределяется по X, колонна — по Y; равенство
/// размахов — X. Пары с выравниванием: после «Выровнять по горизонтали»
/// (ряд) распределение идёт по X, после «по вертикали» — по Y. Решение в
/// пользу одного пункта меню (вместо двух) — компактность меню и
/// предсказуемая связка с только что выполненным выравниванием; правило
/// выводится из текущей раскладки, вопрос владельцу — на приёмке FR-038.
pub(super) fn distribute_axis_for(rects: &[SnapRect]) -> AlignAxis {
    if rects.len() < 2 {
        return AlignAxis::X;
    }
    let span = |axis: AlignAxis| -> f32 {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for rect in rects {
            let c = match axis {
                AlignAxis::X => rect.cx(),
                AlignAxis::Y => rect.cy(),
            };
            lo = lo.min(c);
            hi = hi.max(c);
        }
        hi - lo
    };
    if span(AlignAxis::Y) > span(AlignAxis::X) {
        AlignAxis::Y
    } else {
        AlignAxis::X
    }
}

/// Род batch-операции выделения (FR-038 п.16, T-038.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BatchOp {
    /// «Выровнять…» — ряд/колонна по центрам (общая ось из пункта меню).
    Align,
    /// «Распределить равномерно» — равные зазоры вдоль оси раскладки
    /// (ось из контекста выделения, [`distribute_axis_for`]).
    Distribute,
}

/// Преобразование [x0, y0, x1, y1] → [x, y, w, h] (point_in_rect-конвенция).
pub(super) fn rect_xywh(rect: [f32; 4]) -> [f32; 4] {
    [
        rect[0],
        rect[1],
        (rect[2] - rect[0]).max(0.0),
        (rect[3] - rect[1]).max(0.0),
    ]
}

/// Карточка строки шаблона палитры (FR-024/FR-025): подложка + плитка
/// квад-иконки + имя + описание. Общий рендер строк развёрнутого дока и
/// flyout свёрнутой полосы — WYSIWYG: клик по нарисованному. `rect` — xywh.
#[allow(clippy::too_many_arguments)]
pub(super) fn template_card_row(
    manifest: &canvas_core::templates::TemplateManifest,
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    palette: &ThemeColors,
    icon_tint: [f32; 4],
    instances: &mut Vec<CardInstance>,
    texts: &mut Vec<OwnedScreenText>,
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) {
    instances.push(CardInstance {
        pos: [rect[0], rect[1]],
        size: [rect[2], rect[3]],
        fill,
        border,
        params: [6.0, 0.0, 0.0, 1.0],
    });
    // Плитка иконки (скруглённый квадрат) + квад-иконка роли
    let tile = [
        rect[0] + 8.0,
        rect[1] + (rect[3] - template_ui::TEMPLATE_ROW_TILE) / 2.0,
        template_ui::TEMPLATE_ROW_TILE,
        template_ui::TEMPLATE_ROW_TILE,
    ];
    instances.push(CardInstance {
        pos: [tile[0], tile[1]],
        size: [tile[2], tile[3]],
        fill: palette.palette_tile_fill,
        border: [0.0; 4],
        params: [6.0, 0.0, 0.0, 1.0],
    });
    instances.extend(template_icon_quads(
        template_ui::icon_key(manifest),
        [
            tile[0] + (tile[2] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
            tile[1] + (tile[3] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
            template_ui::TEMPLATE_ROW_ICON,
            template_ui::TEMPLATE_ROW_ICON,
        ],
        icon_tint,
    ));
    texts.push(OwnedScreenText {
        text: manifest.display_name().to_owned(),
        origin: [tile[0] + tile[2] + 8.0, rect[1] + 5.0],
        width: rect[2] - (tile[2] + 24.0),
        font_size: 13.0,
        color: palette.title,
        align: TextAlign::Left,
    });
    // Фикс среза 2026-09-25 (wasm-аудит 13_palette): описание шаблона
    // рвалось кромкой панели без эллипсиса — перенос на 2 строки
    // (ROW_HEIGHT 46 → 52). Хвост длиннее 2 строк не рисуется.
    // m/fs приходят от вызова: в кадре глобальный measure_font_system
    // уже захвачен оверлеем — повторный захват на wasm паникует
    // (recursive mutex, no_threads std; выловлено wasm-аудитом).
    {
        let desc = manifest.description.clone();
        let desc_w = (rect[2] - (tile[2] + 24.0)).max(10.0);
        for (line_idx, line) in crate::admin_ui::wrap_text(m, fs, &desc, desc_w, 11.0)
            .into_iter()
            .take(2)
            .enumerate()
        {
            texts.push(OwnedScreenText {
                text: line,
                origin: [
                    tile[0] + tile[2] + 8.0,
                    rect[1] + 21.0 + line_idx as f32 * 14.0,
                ],
                width: desc_w,
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
    }
}

/// Заголовок ноды для поиска/результатов (T14): имя файла или текст заметки.
pub(super) fn node_title(node: &Node) -> &str {
    if let Some(file) = node.file.as_ref() {
        return Path::new(file)
            .file_name()
            .map(|name| name.to_str().unwrap_or(file))
            .unwrap_or(file);
    }
    node.text.as_deref().unwrap_or("Заметка")
}

/// Полный текст ноды для in-memory поиска (T14): содержимое заметки.
pub(super) fn node_text(node: &Node) -> &str {
    node.text.as_deref().unwrap_or("")
}

/// Подзаголовок строки результата (T14): «заметка» или родительский каталог.
pub(super) fn node_subtitle(node: &Node) -> &str {
    if node.file.is_some() {
        "файл"
    } else {
        "заметка"
    }
}

/// Подзаголовок FTS-хита (T14): имя родительского каталога пути.
pub(super) fn hit_subtitle(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "файл".to_owned())
}

/// CR-012 (правка 2): точная требуемая высота — тело измеряется реальным
/// шейпингом (`measure_body_height`: те же встроенные Noto-шрифты и
/// mono-сегментация формульных строк, что у рендера). Оценка среднего
/// аванса принципиально хрупка — измерение устраняет класс дефектов
/// «футер налезает на перенос». Формула та же, что и у
/// [`estimated_result_reserve_height`], с измеренной высотой тела.
///
/// FR-069 хвосты (T9-сессия 2026-09-24): `auto_rows` — число авто-строк
/// приёмника. Уровень 2 (шейпинг тела) не видит ни авто-строки, ни их
/// подпись зоны (мера вызывает `with_body_stack` с `spill_prefix =
/// Vec::new()`); добавляем ряд подписи зоны здесь, чтобы уровень 2 не
/// занизил против рендера (I-2).
#[allow(clippy::too_many_arguments)] // FR-069 хвосты: 8 согласованных входов резерва (I-2)
pub fn measured_result_reserve_height(
    text: &str,
    node_width: f32,
    formula_lines: &[usize],
    desc: &str,
    // FR-069 (этап F): раскрытое описание («⋯ целиком ▾») растит стек —
    // refit по тогглу; футер-резерв — только нодам с футером (подгонка
    // тела работает для ВСЕХ нод — ранний выход сцены снят); Σ-строка —
    // «Σ <имя узла>» (пусто — строки нет).
    desc_expanded: bool,
    footer_reserve: bool,
    sigma_name: &str,
    auto_rows: usize,
) -> f32 {
    let body_width = (node_width - BODY_PADDING * 2.0).max(BODY_PADDING);
    // FR-061 этап D (D-8): зона описания — часть стека (I-2: measure = render).
    let body = measure_body_height(
        text,
        body_width,
        formula_lines,
        desc,
        desc_expanded,
        sigma_name,
    );
    // FR-069 хвосты: ряд подписи зоны авто-строк — мера его не видит
    // (spill_prefix пуст), но рендер вставляет. ZONE_LABEL_LINE_HEIGHT
    // — тот же токен, что у оценки уровня 1 (canvas-scene::measure).
    let auto_label = if auto_rows > 0 {
        canvas_scene::measure::ZONE_LABEL_LINE_HEIGHT
    } else {
        0.0
    };
    let footer = if footer_reserve {
        RESULT_LINE_HEIGHT + 2.0
    } else {
        0.0
    };
    HEADER_HEIGHT + BODY_TOP_GAP + body + auto_label + BODY_PADDING + footer
}

/// FR-020: slug из имени шаблона: латиница/цифры/дефисы, кириллица —
/// транслитерация (решение владельца: «Нагрузка» → «nagruzka»).
/// Прочие символы — дефис; сжатие подряд идущих; обрезка краёв.
pub(super) fn slugify(name: &str) -> String {
    const TRANSLIT: &[(&str, &str)] = &[
        ("а", "a"),
        ("б", "b"),
        ("в", "v"),
        ("г", "g"),
        ("д", "d"),
        ("е", "e"),
        ("ё", "e"),
        ("ж", "zh"),
        ("з", "z"),
        ("и", "i"),
        ("й", "y"),
        ("к", "k"),
        ("л", "l"),
        ("м", "m"),
        ("н", "n"),
        ("о", "o"),
        ("п", "p"),
        ("р", "r"),
        ("с", "s"),
        ("т", "t"),
        ("у", "u"),
        ("ф", "f"),
        ("х", "h"),
        ("ц", "ts"),
        ("ч", "ch"),
        ("ш", "sh"),
        ("щ", "sch"),
        ("ъ", ""),
        ("ы", "y"),
        ("ь", ""),
        ("э", "e"),
        ("ю", "yu"),
        ("я", "ya"),
    ];
    let lower = name.to_lowercase();
    let mut out = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if let Some((_, latin)) = TRANSLIT.iter().find(|(c, _)| *c == ch.to_string()) {
            out.push_str(latin);
        } else if ch == ' ' || ch == '_' || ch == '-' || ch == '.' || ch == '/' {
            out.push('-');
        }
        // прочие символы (эмодзи, знаки) — пропускаются
    }
    let mut collapsed = String::new();
    for ch in out.chars() {
        if ch == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(ch);
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "custom".to_owned()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// FR-020: тип параметра по токену единицы (подсказка UI в манифесте).
/// Правка владельца (2026-09-23): таблица канонизирована (`s`/`secs`/
/// `hour`/`reqs` убраны), добавлены кириллические синонимы (FR-013).
pub(super) fn infer_param_type(unit: Option<&str>) -> canvas_core::templates::ParamType {
    use canvas_core::templates::ParamType;
    match unit {
        Some("rps") | Some("req/s") | Some("запр/с") => ParamType::Rate,
        Some("ms") | Some("sec") | Some("min") | Some("h") | Some("мс") | Some("сек")
        | Some("мин") | Some("ч") => ParamType::Time,
        Some("B") | Some("KB") | Some("MB") | Some("GB") | Some("Б") | Some("КБ") | Some("МБ")
        | Some("ГБ") => ParamType::Bytes,
        Some("%") => ParamType::Percent,
        Some("req") | Some("запр") => ParamType::Count,
        _ => ParamType::Scalar,
    }
}

/// FR-020: уникальный id custom-шаблона: базовый slug; конфликт с
/// существующим id (реестр или файловая система) — суффикс `-2`, `-3`…
pub(super) fn unique_custom_id(
    base: &str,
    registry: &canvas_core::templates::TemplateRegistry,
    root: &std::path::Path,
) -> String {
    let base = base.chars().take(48).collect::<String>();
    let exists = |id: &str| registry.find(id).is_some() || root.join(id).exists();
    if !exists(&base) {
        return base;
    }
    for n in 2..=1000u32 {
        let candidate = format!("{base}-{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    // Практически недостижимо — последний рубеж: метка времени
    format!(
        "{base}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    )
}

/// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки под
/// курсором (все координаты — логические px окна). Вынесено из App для
/// прямого unit-тестирования.
pub(super) fn expr_error_hit_at(hits: &[LineErrorHit], cursor: [f32; 2]) -> Option<&LineErrorHit> {
    hits.iter().find(|hit| {
        let [x, y, w, h] = hit.rect;
        cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
    })
}

/// FR-050 Н9-2 (этап D): зона наведения пролитой строки под курсором
/// (параметр с toParam / авто-строка приёмника; логические px окна).
/// Вынесено из App для прямого unit-тестирования (паттерн
/// `expr_error_hit_at`).
pub(super) fn spill_hit_at(hits: &[SpillHit], cursor: [f32; 2]) -> Option<&SpillHit> {
    hits.iter().find(|hit| {
        let [x, y, w, h] = hit.rect;
        cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
    })
}

/// FR-050 Н9-6 (этап E): ключ тоста по представлению проливания — строка
/// присваивания была (line: Some) — «Параметр…»; параметра не было
/// (строка-проекция Р-4) — «Значение…». Чистая функция — юнит-тест.
pub(super) fn spill_toast_key(view: &SpillView) -> &'static str {
    if view.line.is_some() {
        keys::TOAST_SPILL_PARAM
    } else {
        keys::TOAST_SPILL_AUTOROW
    }
}

/// FR-050 Н9-3 (этап E): цель контекст-меню параметра — ребро-источник
/// проливания и заголовок меню. Параметр с `toParam` резолвится по
/// инварианту Н4 (победитель — последнее ребро); авто-строка несёт
/// id ребра в hit-зоне (Н10-а — строка производна ребра). Чистая функция
/// над канвасом и hit-зоной — юнит-тест.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SpillMenuTarget {
    /// id ребра-источника проливания.
    pub(super) edge_id: String,
    /// Имя параметра (None — авто-строка/позиционный вход).
    pub(super) param: Option<String>,
    /// Ключ i18n заголовка меню (параметр/входящее значение).
    pub(super) title_key: &'static str,
}

/// FR-045 F-5 v1 (PRD-0004 N3, R-5): адресат лейбла порта — порт без
/// ссылки на ребро (порты — точки присоединения, ребро может отсутствовать).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PortTarget {
    /// Построчный порт: Some(line) — формульная строка (0-based),
    /// None — футер шаблонной ноды (значение ноды целиком).
    Line(Option<usize>),
    /// Якорь параметра шаблонной ноды (toParam).
    Param(String),
    /// Сторонный порт значения ноды (CR-008: сторона выбирается
    /// геометрией ребра — лейбл один и тот же на любой стороне).
    Out,
}

/// FR-045 F-5 v2 (PRD-0004 N3, R-5/R-3): строка лейбла порта — текст и
/// тон. Unmapped-исток (`scene.unmapped_edges`, Р-3) — янтарный акцент
/// анализа (тот же, что у тултипа unmapped-ребра); прочие — акцент
/// потока значений (как в v1). Тон — данные, цвет — на вызове.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PortLabelLine {
    pub(super) text: String,
    pub(super) unmapped: bool,
}

pub(super) fn spill_hit_target(canvas: &Canvas, hit: &SpillHit) -> Option<SpillMenuTarget> {
    let node_id = canvas.nodes.get(hit.node)?.id.clone();
    match &hit.kind {
        SpillHitKind::Param { param, .. } => {
            // Н4: победитель — последнее ребро в параметр
            let edges = canvas_core::flow::occupying_param_edges(canvas, &node_id, param);
            let edge_id = edges.last()?.id.clone();
            Some(SpillMenuTarget {
                edge_id,
                param: Some(param.clone()),
                title_key: keys::MENU_PARAM_TITLE,
            })
        }
        SpillHitKind::AutoRow { edge_id, .. } => {
            // Строка-проекция производна ребра — id уже в hit-зоне
            let exists = canvas.edges.iter().any(|edge| edge.id == *edge_id);
            exists.then(|| SpillMenuTarget {
                edge_id: edge_id.clone(),
                param: None,
                title_key: keys::MENU_AUTOROW_TITLE,
            })
        }
    }
}

/// FR-050 Н2 (этап C): id ноды-истока активного drag (None — drag не
/// активен). Свободная функция — для `compute_param_drop` (self-edge
/// исключается из подсветки целей).
pub(super) fn drag_from_node(drag: Option<&EdgeDrag>) -> Option<String> {
    match drag {
        Some(EdgeDrag::New { from_node, .. }) => Some(from_node.clone()),
        _ => None,
    }
}

/// FR-050 Н4 (этап C): подпись ноды-источника для диалога «Заменить
/// источник?» — снимок имени шаблона (FR-023) / первая непустая строка
/// текста / label / id (тот же приоритет, что у `spill_source_title`
/// ядра; публичная копия — core не экспортирует ту).
pub(super) fn node_display_label(node: &Node) -> String {
    if let Some(name) = node
        .template()
        .and_then(|template| template.name)
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    if let Some(first) = node
        .text
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
    {
        return first.trim().to_owned();
    }
    node.label.clone().unwrap_or_else(|| node.id.clone())
}

/// FR-044 Р-5: приглушение screen-текста — множитель альфы packed-RGBA
/// (glyphon `Color`: a<<24|r<<16|g<<8|b); паттерн dim_factor для текстов.
pub(super) fn dim_text_color(color: canvas_render::Color, alpha: f32) -> canvas_render::Color {
    let a = ((color.a() as f32) * alpha).round().clamp(0.0, 255.0) as u8;
    canvas_render::Color::rgba(color.r(), color.g(), color.b(), a)
}

/// Ярление заливки для hover-подсветки (кнопки настроек/темы): практика
/// аффорданса — интерактивная кнопка отвечает на курсор.
pub(super) fn hover_fill(c: [f32; 4]) -> [f32; 4] {
    [
        (c[0] * 1.3 + 0.04).min(1.0),
        (c[1] * 1.3 + 0.04).min(1.0),
        (c[2] * 1.3 + 0.06).min(1.0),
        c[3],
    ]
}

/// FR-026: пересекаются ли два rect `[x, y, w, h]` — куллинг текстов строк
/// панели, перекрытых выпадающим меню (квады рисуются до screen-текстов).
pub(super) fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}
