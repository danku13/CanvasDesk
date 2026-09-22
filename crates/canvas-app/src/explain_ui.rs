//! PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры — чистая
//! модель (образец [`crate::whatif_ui`]/[`crate::settings_ui`]): геометрия
//! окна поверх полноэкранного канваса (паттерн main stage — решение
//! владельца, У9 v4: `docs/prototypes/ux-defense-mode.html`), tidy-лейаут
//! дерева (колонка = уровень, ряд = порядок листьев; безье от правого
//! порта родителя к левому порту ребёнка — как в прототипе), машина
//! состояний §6.4 (Loading → Ready; Stale — чип «Данные изменены») и
//! hit-тесты.
//!
//! Честный лоадер (AC-1.2, У5): построение асинхронное — на нативе
//! [`build_lineage`] уходит в фоновый поток (UI не блокируется, G5), окно
//! открывается сразу с ротацией подписей этапов; затемнение канваса и
//! подсветка появляются атомарно с готовым деревом (их включает App при
//! переходе в Ready). Рендер/ввод потребляют модель только через этот
//! модуль — инвариант F-5 (одна модель для окна и подсветки).
//!
//! Всё, кроме фонового приёмника, — чистые функции/структуры; юнит-тесты
//! внизу (§9.4-подобные сценарии лейаута/видимости/состояний).

use std::collections::{BTreeMap, BTreeSet};

// Instant — только через канонический alias `canvas_core::time::Instant`
// (W1/`docs/plans/wasm-port.md` §2 п. 7): прямой `std::time::Instant` под
// wasm32 — другой тип, чем alias (web_time), и валит wasm-check/Pages
// E0308 на границе вызовов из app.rs (красный CI по bbcdbc7).
use canvas_core::time::Instant;
use canvas_core::{LineageDelta, LineageError, LineageNodeKind, LineageTree, LineageVia};

// --- геометрия окна (паттерн main stage: затемнение + плавающее окно) -----

/// Поля окна от краёв вьюпорта (логические px).
pub const WIN_MARGIN: f32 = 20.0;
/// Доля ширины вьюпорта (прототип v4: `min(88vw, 1320px)`).
pub const WIN_FRAC_W: f32 = 0.88;
/// Доля высоты вьюпорта (прототип v4: `min(86vh, 900px)`).
pub const WIN_FRAC_H: f32 = 0.86;
/// Потолок ширины окна (логические px, прототип).
pub const WIN_MAX_W: f32 = 1320.0;
/// Потолок высоты окна (логические px, прототип).
pub const WIN_MAX_H: f32 = 900.0;
/// Инвариант читаемости узких окон (как у галереи схем): минимум 320×240.
pub const WIN_MIN_W: f32 = 320.0;
pub const WIN_MIN_H: f32 = 240.0;
/// Высота шапки окна (заголовок + строка крошек/подзаголовка).
pub const HEADER_H: f32 = 56.0;
/// Высота футера окна (статистика + подсказка Esc).
pub const FOOTER_H: f32 = 40.0;
/// Размер кнопки ✕.
pub const CLOSE_SIZE: f32 = 30.0;
/// Размер чипа «Данные изменены» (кнопка в шапке).
pub const CHIP_W: f32 = 210.0;
pub const CHIP_H: f32 = 28.0;

/// Прямоугольник окна проверки: центр вьюпорта, потолки прототипа v4,
/// кламп к окну с полями [`WIN_MARGIN`], минимум инварианта 320×240.
/// `[x, y, w, h]` в логических px.
pub fn window_rect(viewport: [f32; 2]) -> [f32; 4] {
    let w = (viewport[0] * WIN_FRAC_W)
        .clamp(WIN_MIN_W, WIN_MAX_W)
        .min((viewport[0] - WIN_MARGIN * 2.0).max(WIN_MIN_W));
    let h = (viewport[1] * WIN_FRAC_H)
        .clamp(WIN_MIN_H, WIN_MAX_H)
        .min((viewport[1] - WIN_MARGIN * 2.0).max(WIN_MIN_H));
    [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h]
}

/// Кнопка ✕ — правый верхний угол шапки (паттерн main stage).
pub fn close_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0] + win[2] - CLOSE_SIZE - 14.0,
        win[1] + (HEADER_H - CLOSE_SIZE) / 2.0,
        CLOSE_SIZE,
        CLOSE_SIZE,
    ]
}

/// Чип «Данные изменены» — в шапке, левее кнопки ✕ (AC-3.3).
pub fn chip_rect(win: [f32; 4]) -> [f32; 4] {
    let close = close_rect(win);
    [
        close[0] - CHIP_W - 12.0,
        win[1] + (HEADER_H - CHIP_H) / 2.0,
        CHIP_W,
        CHIP_H,
    ]
}

/// Мета-строка с крошками вида (AC-2.3) — нижняя половина шапки. X2: клик
/// по зоне — возврат к корню вида; X6 — ПОЛНЫЕ чипы-крошки: по кликабельному
/// чипу на уровень ([`crumb_rects`]), зона — клик по чипу.
pub fn meta_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0] + 16.0,
        win[1] + 30.0,
        (win[2] - 240.0).max(80.0),
        20.0,
    ]
}

// --- чипы-крошки (X6, AC-2.3) ----------------------------------------------

/// Высота чипа-крошки пути вида.
pub const CRUMB_H: f32 = 18.0;
/// Зазор между чипами-крошками.
pub const CRUMB_GAP: f32 = 4.0;
/// Потолок ширины чипа (длинные заголовки обрезаются рендером текста).
pub const CRUMB_W_MAX: f32 = 148.0;
/// Пол ширины чипа (читаемость; уже — рендер обрезает хвост).
pub const CRUMB_W_MIN: f32 = 48.0;
/// Максимум одновременно показанных чипов (переполнение — показаны
/// ПОСЛЕДНИЕ уровни: текущий фокус важнее корневых).
pub const CRUMB_MAX_CHIPS: usize = 12;

/// Полные чипы-крошки пути вида (AC-2.3, UX-шлифовка X2): по чипу на
/// уровень `view_path`, слева направо в мета-строке шапки; ширина делится
/// поровну (потолок [`CRUMB_W_MAX`], пол [`CRUMB_W_MIN`]). Возвращает
/// `(смещение, прямоугольники)`: смещение — индекс первой показанной
/// крошки в `view_path` (0, пока все помещаются; при переполнении
/// показаны последние [`CRUMB_MAX_CHIPS`]); чипы, не влезающие в мета-
/// зону по ширине, обрезаются/опускаются (рендер и hit используют одну
/// геометрию — детерминизм).
pub fn crumb_rects(win: [f32; 4], count: usize) -> (usize, Vec<[f32; 4]>) {
    let meta = meta_rect(win);
    if count == 0 {
        return (0, Vec::new());
    }
    let shown = count.min(CRUMB_MAX_CHIPS);
    let offset = count - shown;
    let avail = (meta[2] - CRUMB_GAP * (shown as f32 - 1.0)).max(0.0);
    let width = (avail / shown as f32).clamp(CRUMB_W_MIN, CRUMB_W_MAX);
    let meta_end = meta[0] + meta[2];
    let y = meta[1] + (meta[3] - CRUMB_H) / 2.0;
    let mut rects = Vec::with_capacity(shown);
    for index in 0..shown {
        let x = meta[0] + (width + CRUMB_GAP) * index as f32;
        if x >= meta_end {
            break; // дальше мета-зоны чипы не отрисовываются и не кликабельны
        }
        let w = width.min(meta_end - x);
        rects.push([x, y, w, CRUMB_H]);
    }
    (offset, rects)
}

/// Тело окна — между шапкой и футером.
pub fn body_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0],
        win[1] + HEADER_H,
        win[2],
        (win[3] - HEADER_H - FOOTER_H).max(0.0),
    ]
}

/// Внутренний паддинг тела (лейаут дерева от края тела).
pub const BODY_PAD: f32 = 16.0;

// --- лейаут дерева (прототип v4: COLW/CARDW/ROWH/PAD) ----------------------

/// Шаг колонки (уровень дерева) — прототип `COLW`.
pub const COL_W: f32 = 196.0;
/// Ширина карточки узла — прототип `CARDW`.
pub const CARD_W: f32 = 158.0;
/// Высота карточки узла (3 строки: заголовок+значение, формула, адрес).
pub const CARD_H: f32 = 74.0;
/// Шаг ряда (порядок листьев) — прототип `ROWH`.
pub const ROW_H: f32 = 100.0;
/// Поля лейаута — прототип `PAD`.
pub const LAYOUT_PAD: f32 = 14.0;
/// Период ротации подписей честного лоадера (мс, AC-1.2).
pub const LOADER_ROTATION_MS: u128 = 320;
/// Длительность вспышки канвас→дерево (мс, У2/§6.5): затухающая рамка
/// вокруг узла дерева после клика по подсвеченной ноде канваса.
pub const PICK_FLASH_MS: u128 = 700;

/// Результат видимости: кто показан, кто фронтирует (свёрнут), глубины и
/// родители относительно корня ВИДА (поддерева фокуса, AC-2.3).
pub struct Visibility {
    /// Узел показан (индексы `LineageTree::nodes`).
    pub visible: Vec<bool>,
    /// Узел показан, но у него есть скрытые дети (бейдж «+N», раскрытие).
    pub frontier: Vec<bool>,
    /// Глубина от корня вида (корень = 0).
    pub depth: Vec<usize>,
    /// Родитель в дереве (None — корень дерева).
    pub parent: Vec<Option<usize>>,
    /// Число скрытых потомков (для бейджа «+N» фронтира).
    pub hidden_descendants: Vec<usize>,
}

/// Видимость узлов поддерева `root_idx`: авто-раскрытие до `auto_depth`
/// уровней от корня вида (0 — без ограничения, AC-2.3/FR-039) плюс всё,
/// что ниже вручную раскрытых узлов (`expanded` — индексы дерева).
/// Узлы вне поддерева `root_idx` невидимы.
pub fn visibility(
    tree: &LineageTree,
    root_idx: usize,
    auto_depth: u8,
    expanded: &BTreeSet<usize>,
) -> Visibility {
    let n = tree.nodes.len();
    let mut vis = Visibility {
        visible: vec![false; n],
        frontier: vec![false; n],
        depth: vec![usize::MAX; n],
        parent: vec![None; n],
        hidden_descendants: vec![0; n],
    };
    // Родители — обход в ширину от корня дерева (родитель всегда раньше
    // ребёнка в DFS-порядке построения; вид может фокусироваться на
    // поддереве — нужна полная карта предков).
    if n == 0 || root_idx >= n {
        return vis;
    }
    let mut queue: Vec<usize> = vec![0];
    vis.depth[0] = 0;
    let mut head = 0;
    while head < queue.len() {
        let i = queue[head];
        head += 1;
        for child in &tree.nodes[i].children {
            if vis.depth[child.child] == usize::MAX {
                vis.depth[child.child] = vis.depth[i] + 1;
                vis.parent[child.child] = Some(i);
                queue.push(child.child);
            }
        }
    }
    // Глубина от корня ВИДА (не дерева) — отдельный проход.
    let mut depth_from_view = vec![usize::MAX; n];
    depth_from_view[root_idx] = 0;
    let mut order: Vec<usize> = vec![root_idx];
    let mut head = 0;
    while head < order.len() {
        let i = order[head];
        head += 1;
        for child in &tree.nodes[i].children {
            if depth_from_view[child.child] == usize::MAX {
                depth_from_view[child.child] = depth_from_view[i] + 1;
                order.push(child.child);
            }
        }
    }
    // Раскрытые предки: для каждого узла — есть ли предок в expanded
    // (в пределах поддерева вида).
    let ancestor_expanded = |mut i: usize| -> bool {
        while let Some(p) = vis.parent[i] {
            if expanded.contains(&p) {
                return true;
            }
            i = p;
            if i == root_idx {
                break;
            }
        }
        false
    };
    for &i in &order {
        let within_limit = auto_depth == 0 || depth_from_view[i] <= auto_depth as usize;
        vis.visible[i] = i == root_idx || within_limit || ancestor_expanded(i);
    }
    // Фронтир: показан и есть невидимые дети; считаем скрытых потомков.
    for &i in order.iter().rev() {
        let children: Vec<usize> = tree.nodes[i].children.iter().map(|c| c.child).collect();
        let hidden: usize = children
            .iter()
            .filter(|&&c| !vis.visible[c])
            .map(|&c| 1 + vis.hidden_descendants[c])
            .sum();
        vis.hidden_descendants[i] = hidden;
        vis.frontier[i] = vis.visible[i] && hidden > 0;
    }
    vis
}

/// Узел дерева в лейауте: индекс, колонка/ряд, прямоугольник (локальные px
/// тела окна до масштаба), через какое ребро канваса пришёл (адресная
/// строка и подсветка У2).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidNode {
    /// Индекс в [`LineageTree::nodes`].
    pub idx: usize,
    /// Колонка = уровень от корня вида.
    pub col: usize,
    /// Ряд = среднее рядов детей (листья получают следующий свободный ряд).
    pub row: f32,
    /// `[x, y, w, h]` — локальные px тела (до fit-масштаба).
    pub rect: [f32; 4],
    /// Ребро, через которое узел пришёл к родителю (None — корень вида или
    /// локальная переменная листа).
    pub via: Option<LineageVia>,
}

/// Кривая ветки: (безье `[p0, c0, c1, p1]`, к листу — цвет листа).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidCurve {
    pub points: [[f32; 2]; 4],
    pub to_leaf: bool,
}

/// Tidy-лейаут видимого поддерева (прототип v4: колонка = уровень,
/// ряд = порядок листьев; дети без дедупликации — ромб разворачивается).
#[derive(Debug, Clone, Default)]
pub struct TreeLayout {
    /// Показанные узлы в порядке обхода (родитель раньше ребёнка).
    pub nodes: Vec<LaidNode>,
    /// Ветки между показанными узлами (порядок — по родителям).
    pub curves: Vec<LaidCurve>,
    /// Габариты контента `[w, h]` (локальные px, для fit-масштаба).
    pub bounds: [f32; 2],
    /// Максимальная видимых уровней от корня вида (для футера).
    pub levels: usize,
}

/// Построить лейаут видимого поддерева `root_idx` (чистая функция).
pub fn layout_tree(tree: &LineageTree, vis: &Visibility, root_idx: usize) -> TreeLayout {
    let mut layout = TreeLayout::default();
    if root_idx >= tree.nodes.len() || !vis.visible[root_idx] {
        return layout;
    }
    let mut next_row: f32 = 0.0;
    let mut rows: Vec<f32> = vec![0.0; tree.nodes.len()];
    // Итеративный DFS (G5-урок X1: глубина ограничена кучей, не стеком).
    // Стек задач: Enter(idx, col) / Exit(idx, col).
    enum Task {
        Enter(usize, usize),
        Exit(usize, usize),
    }
    let mut tasks = vec![Task::Enter(root_idx, 0)];
    while let Some(task) = tasks.pop() {
        match task {
            Task::Enter(idx, col) => {
                tasks.push(Task::Exit(idx, col));
                let visible_children: Vec<usize> = tree.nodes[idx]
                    .children
                    .iter()
                    .map(|c| c.child)
                    .filter(|&c| vis.visible[c])
                    .collect();
                for &c in visible_children.iter().rev() {
                    tasks.push(Task::Enter(c, col + 1));
                }
            }
            Task::Exit(idx, col) => {
                let visible_children: Vec<usize> = tree.nodes[idx]
                    .children
                    .iter()
                    .map(|c| c.child)
                    .filter(|&c| vis.visible[c])
                    .collect();
                let row = if visible_children.is_empty() {
                    let r = next_row;
                    next_row += 1.0;
                    r
                } else {
                    // Дети выходят из стека РАНЬШЕ родителя (LIFO) — их
                    // ряды уже посчитаны; родитель — среднее первое/последнего.
                    let first = rows[visible_children[0]];
                    let last = rows[*visible_children.last().expect("непусто")];
                    (first + last) / 2.0
                };
                rows[idx] = row;
                let x = LAYOUT_PAD + col as f32 * COL_W;
                let y = LAYOUT_PAD + row * ROW_H;
                let via = vis.parent[idx].and_then(|p| {
                    tree.nodes[p]
                        .children
                        .iter()
                        .find(|c| c.child == idx)
                        .and_then(|c| c.via.clone())
                });
                layout.nodes.push(LaidNode {
                    idx,
                    col,
                    row,
                    rect: [x, y, CARD_W, CARD_H],
                    via,
                });
                // Ветки: родитель → видимые дети (ряды детей уже в rows —
                // их Exit был раньше). Прототип: безье от правого порта
                // родителя к левому порту ребёнка, середина по X.
                let px = x + CARD_W;
                let py = y + CARD_H / 2.0;
                for &c in &visible_children {
                    let ccol = col + 1;
                    let cx = LAYOUT_PAD + ccol as f32 * COL_W;
                    let cy = LAYOUT_PAD + rows[c] * ROW_H + CARD_H / 2.0;
                    let mx = (px + cx) / 2.0;
                    layout.curves.push(LaidCurve {
                        points: [[px, py], [mx, py], [mx, cy], [cx, cy]],
                        to_leaf: tree.nodes[c].kind == LineageNodeKind::Leaf,
                    });
                }
            }
        }
    }
    layout.levels = layout.nodes.iter().map(|n| n.col + 1).max().unwrap_or(0);
    let max_w = layout
        .nodes
        .iter()
        .map(|n| n.rect[0] + n.rect[2])
        .fold(0.0f32, f32::max);
    let max_h = layout
        .nodes
        .iter()
        .map(|n| n.rect[1] + n.rect[3])
        .fold(0.0f32, f32::max);
    layout.bounds = [max_w + LAYOUT_PAD, max_h + LAYOUT_PAD];
    layout
}

/// Fit-масштаб лейаута в тело окна (≤ 1 — только сжатие; прототип вписывает
/// дерево в окно, потолок 1.0 — без растяжения маленьких деревьев).
pub fn fit_scale(bounds: [f32; 2], body: [f32; 4]) -> f32 {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(1.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(1.0);
    (1.0_f32)
        .min(avail_w / bounds[0].max(1.0))
        .min(avail_h / bounds[1].max(1.0))
        .max(0.05)
}

/// Узел под точкой `point` (логические px окна): обратный обход — верхние
/// карточки позже в списке (рисуются поверх).
pub fn node_at(layout: &TreeLayout, scale: f32, body: [f32; 4], point: [f32; 2]) -> Option<usize> {
    for laid in layout.nodes.iter().rev() {
        let x = body[0] + BODY_PAD + laid.rect[0] * scale;
        let y = body[1] + BODY_PAD + laid.rect[1] * scale;
        let rect = [x, y, laid.rect[2] * scale, laid.rect[3] * scale];
        if point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
        {
            return Some(laid.idx);
        }
    }
    None
}

// --- what-if из дерева (PRD-0007 X3, F-6/AC-4.1) ---------------------------

/// Кнопка «Изменить» на карточке ЛИСТА (AC-4.1): правый нижний угол
/// карточки. `rect` — локальные px карточки, `scale` — fit-масштаб.
pub fn edit_rect(card: [f32; 4], scale: f32) -> [f32; 4] {
    const W: f32 = 54.0;
    const H: f32 = 18.0;
    const PAD: f32 = 8.0;
    [
        card[0] + card[2] - (W + PAD) * scale,
        card[1] + card[3] - (H + 6.0) * scale,
        W * scale,
        H * scale,
    ]
}

/// Inline-поле подмены (X3): закреплено в правом нижнем углу тела окна
/// (одна геометрия для рендера и hit-теста — детерминизм).
pub fn field_rect(body: [f32; 4]) -> [f32; 4] {
    const W: f32 = 260.0;
    const H: f32 = 26.0;
    [
        body[0] + body[2] - W - BODY_PAD,
        body[1] + body[3] - H - BODY_PAD,
        W,
        H,
    ]
}

/// Кнопка «Изменить» под точкой: обходит карточки листьев (обратный
/// порядок — верхние позже; hit-тест той же геометрии, что у рендера).
pub fn edit_at(
    tree: &LineageTree,
    layout: &TreeLayout,
    scale: f32,
    body: [f32; 4],
    point: [f32; 2],
) -> Option<usize> {
    for laid in layout.nodes.iter().rev() {
        let node = tree.nodes.get(laid.idx)?;
        // Подмена адресует строку Numi-листа: у итога-программы/шаблона
        // её нет (line: None) — кнопка не показывается (X3-скоуп).
        let editable = node.kind == LineageNodeKind::Leaf
            && node.line.is_some()
            && matches!(&node.value, Some(Ok(_)));
        if !editable {
            continue;
        }
        let [x, y] = {
            [
                body[0] + BODY_PAD + laid.rect[0] * scale,
                body[1] + BODY_PAD + laid.rect[1] * scale,
            ]
        };
        let card = [x, y, laid.rect[2] * scale, laid.rect[3] * scale];
        let rect = edit_rect(card, scale);
        if point[0] >= rect[0]
            && point[0] <= rect[0] + rect[2]
            && point[1] >= rect[1]
            && point[1] <= rect[1] + rect[3]
        {
            return Some(laid.idx);
        }
    }
    None
}

/// Inline-поле подмены листа (AC-4.1): одна строка текста, открывается
/// по кнопке «Изменить», коммит — Enter/клик мимо, отмена — Esc.
#[derive(Debug, Clone, PartialEq)]
pub struct EditField {
    /// Индекс редактируемого узла дерева.
    pub idx: usize,
    /// Текст поля (preset — текущая подмена или исходник строки).
    pub text: String,
}

// --- режим защиты (PRD-0007 X5, F-8/AC-6.1–6.4) ----------------------------

/// Потолок укрупнения графа в режиме защиты (AC-6.2, прототип v4:
/// «×1.5 с вписыванием» — граф вписывается в тело окна, потолок 1.5;
/// маленькие деревья растягиваются только до потолка).
pub const DEFENSE_SCALE_MAX: f32 = 1.5;

/// Fit-масштаб режима защиты: вписать дерево в тело окна с УКРУПНЕНИЕМ
/// до потолка [`DEFENSE_SCALE_MAX`] (в отличие от [`fit_scale`] — там
/// потолок 1.0: обычный вид только сжимает).
pub fn defense_fit_scale(bounds: [f32; 2], body: [f32; 4]) -> f32 {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(1.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(1.0);
    (avail_w / bounds[0].max(1.0))
        .min(avail_h / bounds[1].max(1.0))
        .clamp(0.05, DEFENSE_SCALE_MAX)
}

/// Размер кнопки-тумблера «Режим защиты» (AC-6.1 — одним действием).
pub const DEFENSE_TOGGLE_W: f32 = 132.0;
pub const DEFENSE_TOGGLE_H: f32 = 28.0;

/// Тумблер режима защиты — шапка окна, левее чипа «Данные изменены»;
/// виден в Ready (вход) и Defense (выход в обычный вид, AC-6.4).
pub fn defense_toggle_rect(win: [f32; 4]) -> [f32; 4] {
    let chip = chip_rect(win);
    [
        chip[0] - DEFENSE_TOGGLE_W - 10.0,
        win[1] + (HEADER_H - DEFENSE_TOGGLE_H) / 2.0,
        DEFENSE_TOGGLE_W,
        DEFENSE_TOGGLE_H,
    ]
}

/// Кнопка «Раскрыть уровень» (AC-6.3, шаг) — правый край футера; Defense.
pub fn defense_step_rect(win: [f32; 4]) -> [f32; 4] {
    const W: f32 = 158.0;
    [
        win[0] + win[2] - W - BODY_PAD,
        win[1] + win[3] - FOOTER_H + (FOOTER_H - 28.0) / 2.0,
        W,
        28.0,
    ]
}

/// Кнопка «Раскрыть всё» (AC-6.3) — левее «Раскрыть уровень»; Defense.
pub fn defense_all_rect(win: [f32; 4]) -> [f32; 4] {
    const W: f32 = 126.0;
    let step = defense_step_rect(win);
    [step[0] - W - 10.0, step[1], W, step[3]]
}

/// Есть ли скрытые узлы в текущем виде (шаг AC-6.3 имеет смысл).
pub fn has_hidden(vis: &Visibility) -> bool {
    vis.visible.iter().any(|v| !*v)
}

impl EditField {
    /// Ввод строки (клавиши-символы события, включая кириллицу).
    pub fn type_str(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// Backspace: убрать последний графемный кластер (не байт — кириллица).
    pub fn backspace(&mut self) {
        self.text.pop();
    }
}

// --- машина состояний окна (§6.4: Loading → Ready; Stale — чип) ------------

/// Результат сборки X3: основное дерево (из `flow_active` — с подменами
/// при активном what-if) плюс опциональная БАЗА (из `flow_baseline`) —
/// источник дельт (AC-4.2). База строится тем же фоновым проходом —
/// оба дерева из одного снапшота (инвариант F-5).
pub struct LineageOutcome {
    pub tree: Result<LineageTree, LineageError>,
    pub base: Option<Result<LineageTree, LineageError>>,
}

/// Приёмник фоновой сборки: натив — канал потока; wasm/фолбэк — готово.
pub enum ExplainBuild {
    /// Сборка идёт в фоновом потоке (UI не блокируется, G5).
    Native(std::sync::mpsc::Receiver<LineageOutcome>),
    /// Результат готов сразу (wasm — однопоточный рантайм, фолбэк spawn).
    Done(LineageOutcome),
}

/// Снапшот объяснения для сессионного кэша (AC-3.3, §9.2): переживает
/// закрытие окна; переоткрытие — мгновенно из кэша (≤ 1 с, G1).
#[derive(Debug, Clone)]
pub struct ExplainSnapshot {
    /// Адрес корня — цифра, чья цепочка объяснена.
    pub root: canvas_core::LineageNodeId,
    /// Ревизия модели на момент сборки (чип «Данные изменены», AC-3.3).
    pub revision: u64,
    /// Дерево-снапшот (F-5: одна модель для окна и подсветки).
    pub tree: LineageTree,
    /// Базовое дерево (без what-if подмен) — источник дельт при
    /// переоткрытии при активном сценарии (AC-4.2); None — подмен нет.
    pub base_tree: Option<LineageTree>,
}

/// Что сделал клик по узлу дерева (AC-2.3): раскрыл фронтир / сфокусировал
/// поддерево / лист — только вспышка на канвасе (У2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeClick {
    /// Фронтир раскрыт (дети стали видимы).
    Expanded,
    /// Поддерево сфокусировано (крошка добавлена в путь вида).
    Focused,
    /// Лист — раскрывать нечего (только вспышка канваса).
    Leaf,
}

/// Открытое окно проверки (§6.4): Loading (честный лоадер) или Ready
/// (дерево + подсветка; Stale — чип). Runtime-состояние, не сериализуется.
pub struct ExplainState {
    /// Адрес корня — цифра, чью цепочку объясняем.
    pub root: canvas_core::LineageNodeId,
    /// Ревизия модели, на которой строится/построен снапшот.
    pub revision: u64,
    /// Фоновая сборка (Loading — пока дерево None).
    build: Option<ExplainBuild>,
    /// Готовое дерево (Ready). None — Loading.
    tree: Option<LineageTree>,
    /// Момент открытия (ротация лоадера, метрика G1).
    pub opened_at: Instant,
    /// Путь вида (крошки, AC-2.3): индексы дерева от корня; [0] — корень.
    pub view_path: Vec<usize>,
    /// Вручную раскрытые узлы (индексы дерева).
    pub expanded: BTreeSet<usize>,
    /// Узел дерева под курсором (hover — рамка акцентом, У2-вспышка).
    pub cursor: Option<usize>,
    /// Модель изменилась после сборки (чип «Данные изменены», AC-3.3).
    pub stale: bool,
    /// Базовое дерево (без what-if подмен) — источник дельт (AC-4.2);
    /// None — подмен нет (обычное открытие/перестройка).
    pub base_tree: Option<LineageTree>,
    /// Дельты what-if по адресу узла (node_id, line) — AC-4.2; заполняются
    /// при переходе в Ready, если base_tree есть.
    pub deltas: BTreeMap<(String, Option<usize>), LineageDelta>,
    /// Inline-поле подмены листа (AC-4.1) — одно за раз; не сериализуется.
    pub edit: Option<EditField>,
    /// Режим защиты (§6.4 Ready ↔ Defense): тумблер в шапке, одним
    /// действием. Runtime-состояние — не сериализуется (AC-6.3).
    pub defense: bool,
    /// Число видимых уровней от корня вида в защите (0 — всё дерево);
    /// шаг (AC-6.3) увеличивает, «Раскрыть всё» обнуляет.
    pub defense_reveal: u8,
    /// Вид Ready до входа в защиту (путь крошек + ручные раскрытия) —
    /// восстановление по Esc (AC-6.4 «окно возвращается к обычному виду»).
    pre_defense: Option<(Vec<usize>, BTreeSet<usize>)>,
    /// У2 (§6.5, X6): узел дерева, выделенный кликом по подсвеченной ноде
    /// канваса (индекс в `LineageTree::nodes`); рамка держится до следующего
    /// пика/смены вида. Runtime-состояние — не сериализуется.
    pub pick: Option<usize>,
    /// Момент пика (вспышка затухает за [`PICK_FLASH_MS`], У2).
    pub pick_at: Option<Instant>,
}

impl ExplainState {
    /// Открыть окно с честным лоадером (AC-1.2): панель открывается сразу,
    /// дерево приходит из фоновой сборки.
    pub fn loading(root: canvas_core::LineageNodeId, revision: u64, build: ExplainBuild) -> Self {
        Self {
            root,
            revision,
            build: Some(build),
            tree: None,
            opened_at: Instant::now(),
            view_path: vec![0],
            expanded: BTreeSet::new(),
            cursor: None,
            stale: false,
            base_tree: None,
            deltas: BTreeMap::new(),
            edit: None,
            defense: false,
            defense_reveal: 0,
            pre_defense: None,
            pick: None,
            pick_at: None,
        }
    }

    /// Переоткрыть из сессионного кэша (AC-3.3): Ready мгновенно; чип —
    /// если модель изменилась с момента сборки. Дельты what-if — из
    /// снапшота (AC-4.2: переоткрытие при активном сценарии).
    pub fn from_snapshot(snap: ExplainSnapshot, current_revision: u64) -> Self {
        let stale = snap.revision != current_revision;
        let deltas = snap
            .base_tree
            .as_ref()
            .map(|base| canvas_core::lineage_deltas(base, &snap.tree))
            .unwrap_or_default();
        Self {
            root: snap.root,
            revision: snap.revision,
            build: None,
            tree: Some(snap.tree),
            opened_at: Instant::now(),
            view_path: vec![0],
            expanded: BTreeSet::new(),
            cursor: None,
            stale,
            base_tree: snap.base_tree,
            deltas,
            edit: None,
            defense: false,
            defense_reveal: 0,
            pre_defense: None,
            pick: None,
            pick_at: None,
        }
    }

    /// Готово ли дерево (Ready).
    pub fn is_ready(&self) -> bool {
        self.tree.is_some()
    }

    /// Загрузка идёт (Loading — окно с честным лоадером).
    pub fn is_loading(&self) -> bool {
        self.tree.is_none()
    }

    /// Дерево-снапшот (F-5: один источник для окна и подсветки).
    pub fn tree(&self) -> Option<&LineageTree> {
        self.tree.as_ref()
    }

    /// Забрать дерево (закрытие → сессионный кэш).
    pub fn take_tree(&mut self) -> Option<LineageTree> {
        self.tree.take()
    }

    /// Вернуть дерево после неудачного кэширования (гигиена Option).
    pub fn restore_tree(&mut self, tree: LineageTree) {
        self.tree = Some(tree);
    }

    /// Забрать базовое дерево (закрытие → сессионный кэш, AC-4.2).
    pub fn take_base_tree(&mut self) -> Option<LineageTree> {
        self.base_tree.take()
    }

    /// Опрос фоновой сборки (кадр Loading): true — переход в Ready.
    pub fn poll(&mut self) -> bool {
        let Some(build) = self.build.as_mut() else {
            return false;
        };
        let outcome = match build {
            ExplainBuild::Native(rx) => match rx.try_recv() {
                Ok(outcome) => Some(outcome),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(LineageOutcome {
                    tree: Err(LineageError::RootNotFound(self.root.node_id.clone())),
                    base: None,
                }),
            },
            ExplainBuild::Done(outcome) => Some(std::mem::replace(
                outcome,
                LineageOutcome {
                    tree: Err(LineageError::RootNotFound(self.root.node_id.clone())),
                    base: None,
                },
            )),
        };
        if let Some(outcome) = outcome {
            self.build = None;
            if let Ok(tree) = outcome.tree {
                // Дельты what-if (AC-4.2): база построена тем же проходом
                // из flow_baseline — оба дерева из одного снапшота (F-5).
                self.deltas = outcome
                    .base
                    .as_ref()
                    .and_then(|base| base.as_ref().ok())
                    .map(|base| canvas_core::lineage_deltas(base, &tree))
                    .unwrap_or_default();
                self.base_tree = outcome
                    .base
                    .and_then(|base| base.ok())
                    .or(self.base_tree.take());
                self.tree = Some(tree);
                self.view_path = vec![0];
                self.expanded.clear();
                return true;
            }
            // Err — дерево не построилось (корень пропал между кликом и
            // сборкой): окно остаётся в Loading; App закроет его с тостом
            // (проверка is_failed).
            self.stale = true;
        }
        false
    }

    /// Сборка закончилась ошибкой (корень пропал) — окно надо закрыть
    /// с сообщением (AC-3.3 «корневая цифра удалена»).
    pub fn is_failed(&self) -> bool {
        self.tree.is_none() && self.build.is_none()
    }

    /// Текущая подпись честного лоадера (ротация, AC-1.2): `captions` —
    /// 12 ключей i18n; цикл по [`LOADER_ROTATION_MS`].
    pub fn loader_caption<'a>(&self, captions: &'a [&'a str]) -> &'a str {
        self.loader_caption_at(captions, self.opened_at.elapsed().as_millis())
    }

    /// Подпись по произвольному моменту (чистая версия — тестируется без
    /// ожидания): индекс = `(elapsed / период) % len`.
    pub fn loader_caption_at<'a>(&self, captions: &'a [&'a str], elapsed_ms: u128) -> &'a str {
        if captions.is_empty() {
            return "";
        }
        captions[(elapsed_ms / LOADER_ROTATION_MS) as usize % captions.len()]
    }

    /// Корень текущего вида (крошки, AC-2.3).
    pub fn view_root(&self) -> usize {
        *self.view_path.last().unwrap_or(&0)
    }

    /// Клик по узлу дерева (AC-2.3 + У2): фронтир — раскрыть; узел с
    /// видимыми детьми — сфокусировать поддерево (крошка); лист — ничего.
    pub fn click_node(&mut self, idx: usize, vis: &Visibility) -> NodeClick {
        if !vis.visible.get(idx).copied().unwrap_or(false) {
            return NodeClick::Leaf;
        }
        if vis.frontier[idx] {
            self.expanded.insert(idx);
            return NodeClick::Expanded;
        }
        if !self
            .tree
            .as_ref()
            .map(|t| t.nodes[idx].children.is_empty())
            .unwrap_or(true)
        {
            self.view_path.push(idx);
            // Вид сменился — У2-выделение сбрасывается.
            self.pick = None;
            self.pick_at = None;
            return NodeClick::Focused;
        }
        NodeClick::Leaf
    }

    /// Клик по крошке `level` (0 — корень): путь вида обрезается.
    pub fn click_crumb(&mut self, level: usize) {
        if level < self.view_path.len() {
            self.view_path.truncate(level + 1);
        }
        // Вид сменился — У2-выделение сбрасывается (узел может быть
        // вне нового поддерева вида).
        self.pick = None;
        self.pick_at = None;
    }

    // --- У2: синхронизация канвас→дерево (§6.5, X6) ------------------------

    /// Клик по подсвеченной ноде канваса — выделить соответствующий узел
    /// дерева и «подвести» к нему (У2 — да, решение владельца): предок на
    /// границе лимита глубины раскрывается вручную (узел становится виден),
    /// путь вида не меняется; вспышка — затухающая рамка за
    /// [`PICK_FLASH_MS`], выделение держится до следующего пика/смены вида.
    /// `false` — дерево не готово, индекс вне дерева или нода вне поддерева
    /// текущего вида (канвас-клик ведёт себя как раньше — закрытие на App-
    /// стороне).
    pub fn pick_from_canvas(&mut self, tree_idx: usize, auto_depth: u8) -> bool {
        let Some(tree) = self.tree.as_ref() else {
            return false;
        };
        if tree_idx >= tree.nodes.len() {
            return false;
        }
        // Карта родителей (родитель всегда раньше ребёнка — предзаказ).
        let mut parent = vec![None; tree.nodes.len()];
        for (index, node) in tree.nodes.iter().enumerate() {
            for child in &node.children {
                if child.child < tree.nodes.len() {
                    parent[child.child] = Some(index);
                }
            }
        }
        // Цепочка от корня вида к узлу; узел вне поддерева вида — отказ.
        let view_root = self.view_root();
        let mut chain = vec![tree_idx];
        let mut current = tree_idx;
        while current != view_root {
            let Some(p) = parent[current] else {
                return false; // дошли до корня дерева — view_root не предок
            };
            chain.push(p);
            current = p;
        }
        chain.reverse(); // [view_root, ..., tree_idx]
                         // Скрытый лимитом глубины узел — раскрываем родителя-фронтира.
        let depth = chain.len() - 1;
        if auto_depth > 0 && depth > auto_depth as usize {
            self.expanded.insert(chain[depth - 1]);
        }
        self.pick = Some(tree_idx);
        self.pick_at = Some(Instant::now());
        true
    }

    /// У2: пик по id ноды канваса — первый узел дерева с этим id (ромб
    /// разворачивается — адресов может быть несколько, берём первый в
    /// DFS-порядке, как в окне).
    pub fn pick_by_node_id(&mut self, node_id: &str, auto_depth: u8) -> bool {
        let Some(index) = self
            .tree
            .as_ref()
            .and_then(|tree| tree.nodes.iter().position(|node| node.node_id == node_id))
        else {
            return false;
        };
        self.pick_from_canvas(index, auto_depth)
    }

    /// У2: остаточная яркость вспышки [0..1] к моменту `now`; 0 — погасла
    /// (выделение остаётся). Чистая версия — тестируется без ожидания.
    pub fn pick_flash_at(&self, now: Instant) -> f32 {
        match (self.pick_at, self.pick) {
            (Some(at), Some(_)) => {
                let elapsed = now.duration_since(at).as_millis();
                (1.0 - elapsed as f32 / PICK_FLASH_MS as f32).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    // --- what-if из дерева (X3, AC-4.1) ------------------------------------

    /// Открыть inline-поле подмены на узле `idx` (AC-4.1). `preset` —
    /// текущая подмена активного сценария (если есть); иначе исходник
    /// строки (`formula` узла). Узел должен быть редактируемым листом:
    /// kind Leaf, `line: Some` (адрес строки Numi-листа), значение Ok.
    /// Возвращает true — поле открыто.
    pub fn start_edit(&mut self, idx: usize, tree: &LineageTree, preset: Option<String>) -> bool {
        let Some(node) = tree.nodes.get(idx) else {
            return false;
        };
        let editable = node.kind == LineageNodeKind::Leaf
            && node.line.is_some()
            && matches!(&node.value, Some(Ok(_)));
        if !editable {
            return false;
        }
        let text = preset.or_else(|| node.formula.clone()).unwrap_or_default();
        self.edit = Some(EditField { idx, text });
        true
    }

    /// Закрыть inline-поле с коммитом (Enter/клик мимо): Some((node_id,
    /// line, новый текст строки)) — приложение применит подмену через
    /// `WhatIfOverrides` (FR-017); None — узел/строка не найдены или
    /// текст пуст/равен исходнику (поле закрывается без подмены).
    pub fn finish_edit(&mut self) -> Option<(String, usize, String)> {
        let edit = self.edit.take()?;
        let tree = self.tree.as_ref()?;
        let node = tree.nodes.get(edit.idx)?;
        let line = node.line?;
        let source = node.formula.clone().unwrap_or_default();
        let text = edit.text.trim().to_owned();
        if text.is_empty() || text == source.trim() {
            return None; // пустое/неизменённое поле — отмена без подмены
        }
        Some((node.node_id.clone(), line, text))
    }

    /// Отмена inline-поля (Esc): просто закрыть, модель не меняется.
    pub fn cancel_edit(&mut self) {
        self.edit = None;
    }

    // --- режим защиты (X5, AC-6.1–6.4) --------------------------------------

    /// Открыт ли режим защиты (§6.4 Defense).
    pub fn is_defense(&self) -> bool {
        self.defense
    }

    /// Войти в режим защиты (AC-6.1, Ready → Defense — одним действием,
    /// тумблер в шапке). Вид Ready запоминается для восстановления по Esc
    /// (AC-6.4); вид сбрасывается на корень дерева, авто-раскрытие —
    /// `auto_depth` уровней (синк с лимитом FR-039), ручные раскрытия
    /// сброшены. Проза (AC-6.2) в дереве отсутствует структурно — lineage
    /// собирается только из формульных строк (`line_kind`-детектор).
    /// Inline-поле подмены Ready-вида в защиту не переносится (одно за раз).
    pub fn enter_defense(&mut self, auto_depth: u8) {
        if !self.is_ready() || self.defense {
            return;
        }
        self.pre_defense = Some((self.view_path.clone(), self.expanded.clone()));
        self.defense = true;
        self.defense_reveal = auto_depth;
        self.view_path = vec![0];
        self.expanded.clear();
        self.edit = None;
        self.pick = None;
        self.pick_at = None;
    }

    /// Выйти из режима защиты (AC-6.4, Defense → Ready): окно возвращается
    /// к обычному виду (путь крошек и ручные раскрытия как до входа);
    /// снапшот не меняется, канвас не затрагивается.
    pub fn exit_defense(&mut self) {
        if !self.defense {
            return;
        }
        self.defense = false;
        if let Some((path, expanded)) = self.pre_defense.take() {
            self.view_path = path;
            self.expanded = expanded;
        }
    }

    /// Шаг раскрытия (AC-6.3: пробел/кнопка — следующий уровень дерева от
    /// корня). `false` — шагать некуда (защита не активна, «раскрыть всё»
    /// уже нажато — 0, или скрытых уровней нет — проверка на App-стороне
    /// через [`has_hidden`]).
    pub fn defense_step(&mut self) -> bool {
        if !self.defense || self.defense_reveal == 0 {
            return false;
        }
        self.defense_reveal = self.defense_reveal.saturating_add(1);
        true
    }

    /// «Раскрыть всё» (AC-6.3): снять ограничение уровней (0 — без лимита,
    /// семантика [`visibility`]).
    pub fn defense_reveal_all(&mut self) {
        if self.defense {
            self.defense_reveal = 0;
        }
    }
}

// --- тесты -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::{LineageChild, LineageNode, LineageNodeId};

    /// Дерево-цепочка A → B → C(лист) + лист-константа D у A.
    fn sample_tree() -> LineageTree {
        LineageTree {
            root: LineageNodeId::total("a"),
            nodes: vec![
                LineageNode {
                    node_id: "a".into(),
                    line: None,
                    kind: LineageNodeKind::Calc,
                    value: None,
                    formula: Some("$1 + $2".into()),
                    title: "A".into(),
                    label: None,
                    children: vec![
                        LineageChild {
                            child: 1,
                            via: None,
                        },
                        LineageChild {
                            child: 3,
                            via: None,
                        },
                    ],
                },
                LineageNode {
                    node_id: "b".into(),
                    line: None,
                    kind: LineageNodeKind::Calc,
                    value: None,
                    formula: Some("$in".into()),
                    title: "B".into(),
                    label: None,
                    children: vec![LineageChild {
                        child: 2,
                        via: None,
                    }],
                },
                LineageNode {
                    node_id: "c".into(),
                    line: None,
                    kind: LineageNodeKind::Leaf,
                    value: None,
                    formula: Some("5".into()),
                    title: "C".into(),
                    label: None,
                    children: Vec::new(),
                },
                LineageNode {
                    node_id: "d".into(),
                    line: None,
                    kind: LineageNodeKind::Leaf,
                    value: None,
                    formula: Some("7".into()),
                    title: "D".into(),
                    label: None,
                    children: Vec::new(),
                },
            ],
        }
    }

    /// Окно: центр вьюпорта, потолки прототипа, кламп к краям.
    #[test]
    fn window_rect_centered_and_clamped() {
        let vp = [1600.0, 1000.0];
        let win = window_rect(vp);
        assert!((win[0] + win[2] / 2.0 - 800.0).abs() < 1e-3);
        assert!((win[1] + win[3] / 2.0 - 500.0).abs() < 1e-3);
        assert!(win[2] <= WIN_MAX_W + 1e-3 && win[3] <= WIN_MAX_H + 1e-3);
        // Узкое окно: кламп к маргинам, минимум инварианта.
        let small = window_rect([340.0, 250.0]);
        assert!(small[2] >= WIN_MIN_W - 1e-3);
        assert!(small[3] >= WIN_MIN_H - 1e-3);
        assert!(small[0] >= -0.01);
    }

    /// Видимость: авто-раскрытие 3 уровня (дефолт), глубже — фронтир с
    /// бейджем; ручное раскрытие делает потомков видимыми (AC-2.3).
    #[test]
    fn visibility_depth_limit_and_expand() {
        // Цепочка 5 узлов: 0→1→2→3→4.
        let mut tree = LineageTree {
            root: LineageNodeId::total("n0"),
            nodes: Vec::new(),
        };
        for i in 0..5 {
            tree.nodes.push(LineageNode {
                node_id: format!("n{i}"),
                line: None,
                kind: if i == 4 {
                    LineageNodeKind::Leaf
                } else {
                    LineageNodeKind::Calc
                },
                value: None,
                formula: None,
                title: format!("N{i}"),
                label: None,
                children: if i < 4 {
                    vec![LineageChild {
                        child: i + 1,
                        via: None,
                    }]
                } else {
                    Vec::new()
                },
            });
        }
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        assert!((0..=3).all(|i| vis.visible[i]));
        assert!(!vis.visible[4]);
        assert!(vis.frontier[3]);
        assert_eq!(vis.hidden_descendants[3], 1);
        // Раскрыли фронтир → узел 4 виден, фронтиров больше нет.
        let vis = visibility(&tree, 0, 3, &BTreeSet::from([3]));
        assert!(vis.visible[4]);
        assert!(!vis.frontier[3]);
        // Лимит 0 — всё дерево.
        let vis = visibility(&tree, 0, 0, &empty);
        assert!((0..5).all(|i| vis.visible[i]));
    }

    /// Лейаут: листья в разных рядах, родитель — по среднему; ветки по
    /// числу видимых детей; fit-масштаб ≤ 1.
    #[test]
    fn layout_rows_columns_and_fit() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0);
        // Все 4 узла видимы (глубина ≤ 3).
        assert_eq!(layout.nodes.len(), 4);
        assert_eq!(layout.curves.len(), 3);
        // Листья C (индекс 2, колонка 2) и D (индекс 3, колонка 1) —
        // разные ряды; B между ними по колонке 1.
        let by_idx = |i: usize| layout.nodes.iter().find(|n| n.idx == i).unwrap().clone();
        let (c, d) = (by_idx(2), by_idx(3));
        assert_ne!(c.row, d.row);
        assert_eq!(c.col, 2);
        assert_eq!(d.col, 1);
        // Уровни: корень (0) → C (2) — максимум 3 уровня.
        assert_eq!(layout.levels, 3);
        // Ветка к листу помечена (цвет листа в рендере).
        assert!(layout
            .curves
            .iter()
            .any(|c| c.to_leaf && c.points[3][0] > c.points[0][0]));
        // Fit-масштаб: маленькое дерево в большое тело — 1.0; в крошечное — жмётся.
        assert_eq!(fit_scale(layout.bounds, [0.0, 0.0, 1200.0, 800.0]), 1.0);
        let tiny = fit_scale(layout.bounds, [0.0, 0.0, 300.0, 200.0]);
        assert!((0.05..1.0).contains(&tiny));
    }

    /// Hit-тест узла: точка внутри прямоугольника (с масштабом) находит
    /// узел; мимо — None; обратный обход — верхняя карточка.
    #[test]
    fn node_at_hits_topmost() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0);
        let body = [0.0, 0.0, 1200.0, 800.0];
        let root = &layout.nodes[0];
        let x = body[0] + BODY_PAD + root.rect[0] + 5.0;
        let y = body[1] + BODY_PAD + root.rect[1] + 5.0;
        assert_eq!(node_at(&layout, 1.0, body, [x, y]), Some(root.idx));
        assert_eq!(node_at(&layout, 1.0, body, [5000.0, 5000.0]), None);
    }

    /// Машина состояний: Loading → poll(Done) → Ready; крошки обрезают
    /// путь; переоткрытие из кэша — Ready + чип при смене ревизии.
    #[test]
    fn state_machine_loading_ready_stale() {
        let tree = sample_tree();
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            7,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(tree.clone()),
                base: None,
            }),
        );
        assert!(st.is_loading());
        assert!(st.poll());
        assert!(st.is_ready());
        assert!(!st.is_failed());
        assert_eq!(st.revision, 7);
        assert!(!st.stale);
        // Крошки: фокус на поддереве 1 → путь [0, 1]; клик по корню — назад.
        let empty = BTreeSet::new();
        let vis = visibility(st.tree().unwrap(), 0, 3, &empty);
        assert_eq!(st.click_node(1, &vis), NodeClick::Focused);
        assert_eq!(st.view_path, vec![0, 1]);
        st.click_crumb(0);
        assert_eq!(st.view_path, vec![0]);
        // Кэш: та же ревизия — без чипа; другая — чип (AC-3.3).
        let snap = ExplainSnapshot {
            root: LineageNodeId::total("a"),
            revision: 7,
            tree,
            base_tree: None,
        };
        assert!(!ExplainState::from_snapshot(snap.clone(), 7).stale);
        assert!(ExplainState::from_snapshot(snap, 9).stale);
    }

    /// Лоадер: подписи ротируются по кругу (чистая версия без ожидания).
    #[test]
    fn loader_caption_rotates() {
        let captions = ["c1", "c2", "c3"];
        let st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Err(LineageError::RootNotFound("a".into())),
                base: None,
            }),
        );
        assert_eq!(st.loader_caption_at(&captions, 0), "c1");
        assert_eq!(st.loader_caption_at(&captions, LOADER_ROTATION_MS), "c2");
        assert_eq!(
            st.loader_caption_at(&captions, LOADER_ROTATION_MS * 2),
            "c3"
        );
        // Цикл замкнулся.
        assert_eq!(
            st.loader_caption_at(&captions, LOADER_ROTATION_MS * 3),
            "c1"
        );
        // Пустой список — пустая строка (без паники).
        assert_eq!(st.loader_caption_at(&[], 0), "");
    }

    /// Сбой сборки (корень пропал): poll фиксирует is_failed — App закроет
    /// окно с сообщением (AC-3.3).
    #[test]
    fn failed_build_marks_failed() {
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Err(LineageError::RootNotFound("a".into())),
                base: None,
            }),
        );
        assert!(!st.poll());
        assert!(st.is_failed());
        assert!(!st.is_ready());
    }

    /// X3 (AC-4.2): poll с базой — дельты считаются при переходе Ready;
    /// дерево без базы — дельт нет.
    #[test]
    fn poll_with_base_builds_deltas() {
        // sample_tree: узлы 0(a) → 1(b) → 2(c, лист); узел 3(d, лист).
        // Лист c — редактируемая строка (line Some(0)); в базе значение
        // 5, в what-if — 7 (дельта +2).
        let mut base = sample_tree();
        base.nodes[2].line = Some(0);
        base.nodes[2].value = Some(Ok(canvas_core::expr::Value::scalar(5.0)));
        let mut whatif = sample_tree();
        whatif.nodes[2].line = Some(0);
        whatif.nodes[2].value = Some(Ok(canvas_core::expr::Value::scalar(7.0)));
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            3,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(whatif),
                base: Some(Ok(base)),
            }),
        );
        assert!(st.poll());
        assert_eq!(st.deltas.len(), 1, "дельта на листе c");
        let delta = st
            .deltas
            .get(&("c".to_owned(), Some(0)))
            .expect("ключ узла (c, line 0)");
        assert_eq!(delta.delta, "+2");
        assert!(st.base_tree.is_some());
        // Переоткрытие из кэша сохраняет дельты (AC-4.2).
        let snap = ExplainSnapshot {
            root: LineageNodeId::total("a"),
            revision: 3,
            tree: st.tree().expect("дерево").clone(),
            base_tree: st.base_tree.clone(),
        };
        let reopened = ExplainState::from_snapshot(snap, 3);
        assert_eq!(reopened.deltas.len(), 1);
        // Дерево без базы — дельты пусты.
        let mut st2 = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(sample_tree()),
                base: None,
            }),
        );
        assert!(st2.poll());
        assert!(st2.deltas.is_empty());
    }

    /// X3 (AC-4.1): кнопка «Изменить» — только на редактируемых листьях
    /// (Leaf + line: Some + значение Ok); hit-тест edit_at.
    #[test]
    fn edit_button_targets_editable_leaves() {
        let mut tree = sample_tree();
        // Лист c (индекс 2) — редактируемый (line Some(0), value Ok);
        // лист d (индекс 3) — line None (итог-программа) — не редактируемый.
        tree.nodes[2].line = Some(0);
        tree.nodes[2].value = Some(Ok(canvas_core::expr::Value::scalar(5.0)));
        tree.nodes[3].value = Some(Ok(canvas_core::expr::Value::scalar(7.0)));
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0);
        let body = [0.0, 0.0, 1200.0, 800.0];
        // Точка кнопки листа c — из его карточки.
        let laid = layout
            .nodes
            .iter()
            .find(|n| n.idx == 2)
            .expect("лист c в лейауте");
        let x = body[0] + BODY_PAD + laid.rect[0] + laid.rect[2] - 20.0;
        let y = body[1] + BODY_PAD + laid.rect[1] + laid.rect[3] - 10.0;
        assert_eq!(edit_at(&tree, &layout, 1.0, body, [x, y]), Some(2));
        // Точка кнопки листа d (не редактируемый) — None.
        let laid_d = layout
            .nodes
            .iter()
            .find(|n| n.idx == 3)
            .expect("лист d в лейауте");
        let xd = body[0] + BODY_PAD + laid_d.rect[0] + laid_d.rect[2] - 20.0;
        let yd = body[1] + BODY_PAD + laid_d.rect[1] + laid_d.rect[3] - 10.0;
        assert_eq!(edit_at(&tree, &layout, 1.0, body, [xd, yd]), None);
    }

    /// X3 (AC-4.1): inline-поле — start_edit задаёт preset (подмена или
    /// исходник); finish_edit возвращает (node_id, line, текст) и
    /// игнорирует пустое/неизменённое значение; Esc — отмена; ввод
    /// строки/Backspace не рвут кириллицу.
    #[test]
    fn edit_field_lifecycle() {
        let mut tree = sample_tree();
        tree.nodes[2].line = Some(0);
        tree.nodes[2].value = Some(Ok(canvas_core::expr::Value::scalar(5.0)));
        tree.nodes[2].formula = Some("620".into());
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(tree.clone()),
                base: None,
            }),
        );
        st.poll();
        let tree_for_check = st.tree().unwrap().clone();
        // Расчётные узлы не редактируются (kind Calc).
        assert!(!st.start_edit(0, &tree_for_check, None));
        assert!(st.edit.is_none());
        // Лист: preset нет → исходник строки.
        assert!(st.start_edit(2, &tree_for_check, None));
        assert_eq!(st.edit.as_ref().expect("поле").text, "620");
        // Ввод кириллицы/символов + Backspace (pop последнего char —
        // «₽» и кириллица не режутся по байтам).
        let edit = st.edit.as_mut().expect("поле");
        edit.text.clear();
        edit.type_str("₽ 620");
        edit.backspace();
        assert_eq!(edit.text, "₽ 62");
        edit.text.clear();
        edit.type_str("700");
        // finish: узел/строка/текст.
        let outcome = st.finish_edit();
        assert_eq!(
            outcome,
            Some(("c".to_owned(), 0, "700".to_owned())),
            "коммит подмены"
        );
        assert!(st.edit.is_none());
        // Неизменённое/пустое значение — без подмены.
        assert!(st.start_edit(2, &tree_for_check, Some("620".into())));
        assert_eq!(st.finish_edit(), None, "текст = исходник");
        assert!(st.start_edit(2, &tree_for_check, Some("   ".into())));
        assert_eq!(st.finish_edit(), None, "пустой текст");
        // Esc — отмена.
        assert!(st.start_edit(2, &tree_for_check, Some("9".into())));
        st.cancel_edit();
        assert!(st.edit.is_none());
        assert_eq!(st.finish_edit(), None);
    }

    /// X3: кнопка «Изменить» рисуется в правом нижнем углу карточки
    /// (edit_rect) — не вылезает за карточку при масштабе < 1.
    #[test]
    fn edit_rect_stays_inside_card() {
        for scale in [1.0f32, 0.5, 0.25] {
            let card = [40.0, 30.0, 158.0, 74.0];
            let rect = edit_rect(card, scale);
            assert!(rect[0] >= card[0] && rect[1] >= card[1]);
            assert!(rect[0] + rect[2] <= card[0] + card[2] + 0.01);
            assert!(rect[1] + rect[3] <= card[1] + card[3] + 0.01);
            assert!(rect[2] > 0.0 && rect[3] > 0.0);
        }
    }

    /// X5 (AC-6.1/6.4): вход в защиту одним действием сбрасывает вид на
    /// корень (auto_depth уровней); Esc-выход восстанавливает обычный вид
    /// (путь крошек + ручные раскрытия); снапшот не меняется.
    #[test]
    fn defense_enter_exit_roundtrip() {
        let tree = sample_tree();
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            7,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(tree.clone()),
                base: None,
            }),
        );
        assert!(st.poll());
        assert!(st.is_ready());
        assert!(!st.is_defense());
        // Обычный вид: фокус на поддереве b + раскрытие фронтира.
        st.view_path = vec![0, 1];
        st.expanded.insert(1);
        // Вход (тумблер, одним действием): вид сброшен на корень.
        st.enter_defense(3);
        assert!(st.is_defense());
        assert_eq!(st.view_path, vec![0]);
        assert!(st.expanded.is_empty());
        assert_eq!(st.defense_reveal, 3);
        // Снапшот не изменился (F-5: тот же tree, та же ревизия).
        assert_eq!(st.revision, 7);
        // Выход (Esc): обычный вид восстановлен (AC-6.4).
        st.exit_defense();
        assert!(!st.is_defense());
        assert_eq!(st.view_path, vec![0, 1]);
        assert!(st.expanded.contains(&1));
        // Повторный вход/выход после восстановления — симметричен.
        st.enter_defense(2);
        assert_eq!(st.defense_reveal, 2);
        st.exit_defense();
        assert_eq!(st.view_path, vec![0, 1]);
        // Вход в Loading невозможен (защита поверх Ready, §6.4).
        let mut loading = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Err(LineageError::RootNotFound("a".into())),
                base: None,
            }),
        );
        loading.enter_defense(3);
        assert!(!loading.is_defense());
    }

    /// X5 (AC-6.3): шаг раскрывает следующий уровень; «Раскрыть всё»
    /// снимает лимит (0); шаг после «всё» — no-op; без защиты — no-op.
    #[test]
    fn defense_step_and_reveal_all() {
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(sample_tree()),
                base: None,
            }),
        );
        st.poll();
        // Вне защиты шаг не работает.
        assert!(!st.defense_step());
        st.enter_defense(1);
        assert_eq!(st.defense_reveal, 1);
        assert!(st.defense_step());
        assert_eq!(st.defense_reveal, 2);
        assert!(st.defense_step());
        assert_eq!(st.defense_reveal, 3);
        // «Раскрыть всё» — 0 (без лимита), шаги больше не меняют.
        st.defense_reveal_all();
        assert_eq!(st.defense_reveal, 0);
        assert!(!st.defense_step());
        assert_eq!(st.defense_reveal, 0);
    }

    /// X5 (AC-6.3): шаг имеет смысл, только если есть скрытые узлы
    /// (has_hidden); семантика reveal 0 = всё видимо.
    #[test]
    fn defense_has_hidden_and_visibility() {
        // Цепочка 5 узлов: reveal 3 → узел 4 скрыт; reveal 4 → всё видно.
        let mut tree = LineageTree {
            root: LineageNodeId::total("n0"),
            nodes: Vec::new(),
        };
        for i in 0..5 {
            tree.nodes.push(LineageNode {
                node_id: format!("n{i}"),
                line: None,
                kind: if i == 4 {
                    LineageNodeKind::Leaf
                } else {
                    LineageNodeKind::Calc
                },
                value: None,
                formula: None,
                title: format!("N{i}"),
                label: None,
                children: if i < 4 {
                    vec![LineageChild {
                        child: i + 1,
                        via: None,
                    }]
                } else {
                    Vec::new()
                },
            });
        }
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        assert!(has_hidden(&vis));
        let vis = visibility(&tree, 0, 4, &empty);
        assert!(!has_hidden(&vis), "все уровни раскрыты");
        // reveal 0 — без ограничения (семантика visibility).
        let vis = visibility(&tree, 0, 0, &empty);
        assert!(!has_hidden(&vis));
    }

    /// X5 (AC-6.2): fit-масштаб защиты вписывает дерево в тело окна с
    /// потолком 1.5 (маленькое дерево укрупняется, большое — сжимается).
    #[test]
    fn defense_fit_scale_up_to_ceiling() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0);
        let body = [0.0, 0.0, 1200.0, 800.0];
        // Маленькое дерево в большое тело: обычный вид — 1.0, защита —
        // укрупнение до потолка (×1.5, AC-6.2).
        assert_eq!(fit_scale(layout.bounds, body), 1.0);
        let d = defense_fit_scale(layout.bounds, body);
        assert!(d > 1.0, "защита укрупняет: {d}");
        assert!(d <= DEFENSE_SCALE_MAX + 1e-3);
        // Гигантское дерево в маленькое тело — сжатие, как обычно.
        let tiny = defense_fit_scale(layout.bounds, [0.0, 0.0, 300.0, 200.0]);
        assert!(tiny < 1.0);
        // Потолок соблюдён на любом входе.
        let huge = defense_fit_scale([1.0, 1.0], body);
        assert!((huge - DEFENSE_SCALE_MAX).abs() < 1e-3);
    }

    /// X5 (геометрия): тумблер — левее чипа в шапке; кнопки футера —
    /// внутри окна, «Раскрыть всё» левее «Раскрыть уровень».
    #[test]
    fn defense_button_geometry() {
        let win = [100.0, 100.0, 1200.0, 800.0];
        let toggle = defense_toggle_rect(win);
        let chip = chip_rect(win);
        assert!(toggle[0] + toggle[2] <= chip[0], "тумблер левее чипа");
        assert!((toggle[1] + toggle[3] / 2.0 - (win[1] + HEADER_H / 2.0)).abs() < 1e-3);
        let step = defense_step_rect(win);
        let all = defense_all_rect(win);
        assert!(step[0] + step[2] <= win[0] + win[2] + 0.01, "внутри окна");
        assert!(all[0] + all[2] <= step[0] + 0.01, "«всё» левее «уровня»");
        assert!(step[1] >= win[1] + win[3] - FOOTER_H);
        assert!(step[1] + step[3] <= win[1] + win[3] + 0.01);
    }

    /// X5 (AC-4.4): inline-поле подмены работает и в защите — подмена
    /// из режима защиты не требует выхода (G3).
    #[test]
    fn defense_keeps_edit_mechanics() {
        let mut tree = sample_tree();
        tree.nodes[2].line = Some(0);
        tree.nodes[2].value = Some(Ok(canvas_core::expr::Value::scalar(5.0)));
        tree.nodes[2].formula = Some("620".into());
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(tree.clone()),
                base: None,
            }),
        );
        st.poll();
        st.enter_defense(3);
        let tree_ref = st.tree().unwrap().clone();
        assert!(
            st.start_edit(2, &tree_ref, None),
            "лист редактируем в защите"
        );
        st.edit.as_mut().expect("поле").text = "700".into();
        assert_eq!(
            st.finish_edit(),
            Some(("c".to_owned(), 0, "700".to_owned())),
            "подмена из защиты коммитится"
        );
        // Вход в защиту закрывает открытое поле Ready-вида (одно за раз).
        st.start_edit(2, &st.tree().unwrap().clone(), None);
        st.exit_defense();
        st.enter_defense(3);
        assert!(st.edit.is_none(), "поле не переносится в защиту");
    }

    /// X6 (AC-2.3): полные чипы-крошки — по чипу на уровень, без
    /// переполнения смещение 0, ширины в [MIN, MAX], чипы стыкуются без
    /// налезаний; при переполнении показаны последние уровни (смещение > 0).
    #[test]
    fn crumb_rects_layout_and_overflow() {
        let win = window_rect([1600.0, 1000.0]);
        // Пустой путь — ничего.
        let (offset, rects) = crumb_rects(win, 0);
        assert_eq!((offset, rects.len()), (0, 0));
        // Три уровня: смещение 0, геометрия согласована.
        let (offset, rects) = crumb_rects(win, 3);
        assert_eq!((offset, rects.len()), (0, 3));
        let meta = meta_rect(win);
        for pair in rects.windows(2) {
            assert!((pair[0][0] + pair[0][2] + CRUMB_GAP - pair[1][0]).abs() < 1e-3);
        }
        for r in &rects {
            assert!(r[2] <= CRUMB_W_MAX + 1e-3);
            assert!(r[2] >= CRUMB_W_MIN - 1e-3);
            assert!(r[0] >= meta[0] - 1e-3);
            assert!(r[0] + r[2] <= meta[0] + meta[2] + 1e-3);
            assert!((r[1] - (meta[1] + (meta[3] - CRUMB_H) / 2.0)).abs() < 1e-3);
        }
        // Переполнение: 20 уровней — показаны последние 12, смещение 8.
        let (offset, rects) = crumb_rects(win, 20);
        assert_eq!(offset, 8);
        assert_eq!(rects.len(), CRUMB_MAX_CHIPS);
        // Узкое окно: чипы обрезаются мета-зоной (рендер = hit).
        let narrow = window_rect([340.0, 250.0]);
        let (_, rects) = crumb_rects(narrow, 6);
        let meta = meta_rect(narrow);
        for r in &rects {
            assert!(
                r[0] + r[2] <= meta[0] + meta[2] + 1e-3,
                "чип шире мета-зоны"
            );
        }
    }

    /// X6 (У2, §6.5): пик канвас→дерево — видимый узел выделяется сразу;
    /// узел глубже лимита раскрывает родителя-фронтира; узел вне поддерева
    /// вида и несуществующий id — отказ; смена вида/защита сбрасывают пик.
    #[test]
    fn canvas_pick_selects_and_reveals() {
        // sample_tree: a(0) → b(1) → c(2), a → d(3, лист).
        let tree = sample_tree();
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(tree),
                base: None,
            }),
        );
        st.poll();
        // Пик по id существующей ноды — выделение + вспышка 1.0.
        assert!(st.pick_by_node_id("c", 3));
        assert_eq!(st.pick, Some(2));
        assert!(st.pick_flash_at(Instant::now()) > 0.99);
        // Вспышка гаснет к концу окна, выделение остаётся.
        let later = Instant::now() + std::time::Duration::from_millis(PICK_FLASH_MS as u64 + 50);
        assert_eq!(st.pick_flash_at(later), 0.0);
        assert_eq!(st.pick, Some(2));
        // Лимит глубины 1: узел c (глубина 2 от корня) скрыт — пик
        // раскрывает родителя b, узел становится видимым («подводит»).
        st.click_crumb(0);
        assert_eq!(st.pick, None, "смена вида сбросила пик");
        assert!(st.pick_by_node_id("c", 1));
        assert!(st.expanded.contains(&1), "родитель-фронтир раскрыт");
        let vis = visibility(st.tree().unwrap(), st.view_root(), 1, &st.expanded);
        assert!(vis.visible[2], "узел раскрыт и видим");
        // Узел вне поддерева вида — отказ (в фокусе b: c виден, d — нет).
        st.view_path.push(1);
        assert!(!st.pick_by_node_id("d", 0), "d вне поддерева вида b");
        // Несуществующий id — отказ.
        assert!(!st.pick_by_node_id("no-such", 0));
        // Защита сбрасывает пик (канвас в защите не отвечает).
        st.click_crumb(0);
        st.pick_from_canvas(2, 3);
        st.enter_defense(3);
        assert_eq!(st.pick, None);
        // Пик по индексу вне дерева — отказ.
        assert!(!st.pick_from_canvas(99, 3));
    }
}
