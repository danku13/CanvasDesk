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

use std::collections::BTreeSet;
use std::time::Instant;

use canvas_core::{LineageError, LineageNodeKind, LineageTree, LineageVia};

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
/// по зоне — возврат к корню вида (полные чипы-крошки — позже, UX-шлифовка).
pub fn meta_rect(win: [f32; 4]) -> [f32; 4] {
    [
        win[0] + 16.0,
        win[1] + 30.0,
        (win[2] - 240.0).max(80.0),
        20.0,
    ]
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

// --- машина состояний окна (§6.4: Loading → Ready; Stale — чип) ------------

/// Приёмник фоновой сборки: натив — канал потока; wasm/фолбэк — готово.
pub enum ExplainBuild {
    /// Сборка идёт в фоновом потоке (UI не блокируется, G5).
    Native(std::sync::mpsc::Receiver<Result<LineageTree, LineageError>>),
    /// Результат готов сразу (wasm — однопоточный рантайм, фолбэк spawn).
    Done(Result<LineageTree, LineageError>),
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
        }
    }

    /// Переоткрыть из сессионного кэша (AC-3.3): Ready мгновенно; чип —
    /// если модель изменилась с момента сборки.
    pub fn from_snapshot(snap: ExplainSnapshot, current_revision: u64) -> Self {
        let stale = snap.revision != current_revision;
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

    /// Опрос фоновой сборки (кадр Loading): true — переход в Ready.
    pub fn poll(&mut self) -> bool {
        let Some(build) = self.build.as_mut() else {
            return false;
        };
        let result = match build {
            ExplainBuild::Native(rx) => match rx.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err(LineageError::RootNotFound(self.root.node_id.clone())))
                }
            },
            ExplainBuild::Done(result) => Some(std::mem::replace(
                result,
                Err(LineageError::RootNotFound(self.root.node_id.clone())),
            )),
        };
        if let Some(result) = result {
            self.build = None;
            if let Ok(tree) = result {
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
            return NodeClick::Focused;
        }
        NodeClick::Leaf
    }

    /// Клик по крошке `level` (0 — корень): путь вида обрезается.
    pub fn click_crumb(&mut self, level: usize) {
        if level < self.view_path.len() {
            self.view_path.truncate(level + 1);
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
            ExplainBuild::Done(Ok(tree.clone())),
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
            ExplainBuild::Done(Err(LineageError::RootNotFound("a".into()))),
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
            ExplainBuild::Done(Err(LineageError::RootNotFound("a".into()))),
        );
        assert!(!st.poll());
        assert!(st.is_failed());
        assert!(!st.is_ready());
    }
}
