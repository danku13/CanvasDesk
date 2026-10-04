//! PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры — чистая
//! модель (образец [`crate::whatif_ui`]/[`crate::settings_ui`]): геометрия
//! окна поверх полноэкранного канваса (паттерн main stage), tidy-лейаут
//! дерева (колонка = уровень, ряд = порядок листьев; безье от порта
//! родителя к порту ребёнка — сторона портов зависит от направления
//! [`LayoutDirection`], FR-083), машина состояний §6.4 (Loading → Ready;
//! Stale — чип «Данные изменены») и hit-тесты.
//!
//! FR-083 (решение владельца, три правки поверх FR-048): (1) окно — 80 %
//! вьюпорта без потолка (минимум 320×240 и поля [`WIN_MARGIN`]
//! сохраняются); (2) направление схемы — тумблер в шапке: [`LayoutDirection`]
//! с сохранением выбора в настройках (`explain_sources_left`, canvas-core);
//! (3) пол fit-масштаба [`SCALE_MIN`] + панорамирование тела
//! ([`pan_clamp`], колесо/драг по фону, индикатор переполнения).
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
//!
//! FR-060 (волна 2 миграции кита, паттерн U5 — числа дословно): окно —
//! `kit::modal` (слот = вьюпорт, сжатый на [`WIN_MARGIN`]; с FR-083 max =
//! размер слота — потолка нет); крошки — отбор по мета-зоне `take_while`
//! вместо break-клампа (G5: политика hide, семантика прежняя);
//! `Option::take` в автомате состояния — перенос владения, не срез
//! контента (аудит G5 — про срезы `take(n)`/break-клампы/`truncate_chars`).
//! Дерево (`layout_tree`/`fit_scale`) — 2D-tidy-лейаут прототипа: кит
//! список+скролл однородных строк здесь неприменим (documented отклонение,
//! как kit::card в FR-059); чип «Данные изменены»/кнопка ✕ — прежние
//! размеры (kit chip 24/icon_button 26 ≠ прежних 28/30 — числа дословно
//! сильнее перечня «замена»).
//!
//! FR-084 (волна 1 — чистая геометрия; рендер/ввод подхватит волна 2):
//! (1) [`content_origin`] — базовая точка контента в теле окна:
//! центрирование по осям, где дерево помещается (переполняющие оси —
//! прижаты, работает кламп пана [`pan_clamp`]); (2) сестринские листья
//! одного узла-источника (одинаковый `node_id` под одним родителем)
//! схлопываются в карточку-таблицу: [`LaidNode::rows`] несёт строки
//! [`LaidRow`] (ссылка на лист + локальная геометрия), кривые приходят
//! в порты строк (`port_y`), геометрия иконки редактирования строки —
//! [`row_edit_rect`] + [`table_row_text_width`]. Ряды дерева ([`ROW_H`],
//! родитель = среднее рядов детей) назначаются каждому листу по-прежнему;
//! карточка-таблица занимает место первого листа группы (порядок
//! `layout.nodes` сохранён).

use std::collections::{BTreeMap, BTreeSet};

// Instant — только через канонический alias `canvas_core::time::Instant`
// (W1/`docs/plans/wasm-port.md` §2 п. 7): прямой `std::time::Instant` под
// wasm32 — другой тип, чем alias (web_time), и валит wasm-check/Pages
// E0308 на границе вызовов из app.rs (красный CI по bbcdbc7).
use canvas_core::time::Instant;
use canvas_core::{LineageDelta, LineageError, LineageNodeKind, LineageTree, LineageVia};

use canvas_ui::geometry::{UiRect, UiVec2};
use canvas_ui::kit;

// --- геометрия окна (паттерн main stage: затемнение + плавающее окно) -----

/// Поля окна от краёв вьюпорта (логические px).
pub const WIN_MARGIN: f32 = 20.0;
/// Доля ширины вьюпорта (FR-083: окно = 80 % вьюпорта, без потолка).
pub const WIN_FRAC_W: f32 = 0.80;
/// Доля высоты вьюпорта (FR-083: окно = 80 % вьюпорта, без потолка).
pub const WIN_FRAC_H: f32 = 0.80;
/// Инвариант читаемости узких окон (как у галереи схем): минимум 320×240.
pub const WIN_MIN_W: f32 = 320.0;
pub const WIN_MIN_H: f32 = 240.0;
/// Высота шапки окна (заголовок + строка крошек/подзаголовка).
// TODO: migrate to PANEL_HEADER_H_L=44 (FR-046 W-d аудит §4) — текущее
// значение 56 на 12px отличается от large-варианта шкалы; оставлено как
// отклонение (визуальный скачок 56→44 нежелателен в W-d-волне без
// отдельной проверки геометрии шапки explain — заголовок + строка крошек
// требуют высоты 56; I-1: ноль скачка).
pub const HEADER_H: f32 = 56.0;
/// Высота футера окна (статистика + подсказка Esc).
pub const FOOTER_H: f32 = 40.0;
/// Размер кнопки ✕.
pub const CLOSE_SIZE: f32 = 30.0;
/// Размер чипа «Данные изменены» (кнопка в шапке).
pub const CHIP_W: f32 = 210.0;
pub const CHIP_H: f32 = 28.0;

/// Прямоугольник окна проверки — `kit::modal` (FR-060): слот = вьюпорт,
/// сжатый на поля [`WIN_MARGIN`]; FR-083: max = размер слота (потолка
/// нет — окно всегда 80 % вьюпорта), desired = доли вьюпорта, min =
/// инвариант 320×240. На крошечных вьюпортах инвариант приоритетен
/// (панель сохраняет min — documented деградация kit::modal).
/// `[x, y, w, h]` в логических px.
pub fn window_rect(viewport: [f32; 2]) -> [f32; 4] {
    let slot = UiRect::new(
        WIN_MARGIN,
        WIN_MARGIN,
        (viewport[0] - WIN_MARGIN * 2.0).max(0.0),
        (viewport[1] - WIN_MARGIN * 2.0).max(0.0),
    );
    let layout = kit::modal(
        slot,
        UiVec2::new(WIN_MIN_W, WIN_MIN_H),
        UiVec2::new(slot.w, slot.h),
        UiVec2::new(viewport[0] * WIN_FRAC_W, viewport[1] * WIN_FRAC_H),
    );
    [
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
    ]
}

/// Кнопка ✕ — правый верхний угол шапки (паттерн main stage).
///
/// TODO(G/FR-070): migrate to `kit::stage_close_button(panel_slot)` —
/// размер 30×30 (`CLOSE_SIZE`) против канонического `ICON_BUTTON_SIZE=26`
/// даёт визуальный скачок 4px (уменьшение кнопки) + сдвиг позиции
/// (inset 14 → `SPACING_SM=8`, `y=(HEADER_H-CLOSE_SIZE)/2=13` → `+8`);
/// `HEADER_H=56` уже имеет TODO на `PANEL_HEADER_H_L=44` (W-d аудит §4) —
/// миграция close_button связана с переносом всей шапки на новую шкалу
/// (I-1: ноль скачка).
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
    // FR-060/G5: без break-выхода — отбор по мета-зоне через `take_while`
    // (политика hide: чипы за зоной не отрисовываются и не кликабельны —
    // семантика прежняя дословно, паттерн token_before_caret из FR-059)
    rects.extend(
        (0..shown)
            .map(|index| meta[0] + (width + CRUMB_GAP) * index as f32)
            .take_while(|&x| x < meta_end)
            .map(|x| {
                let w = width.min(meta_end - x);
                [x, y, w, CRUMB_H]
            }),
    );
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

// --- направление схемы (FR-083) ---------------------------------------------

/// Направление потока схемы (FR-083).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutDirection {
    /// Источники слева, итог справа — чтение слева-направо (дефолт).
    #[default]
    Ltr,
    /// Итог слева, источники справа — прежнее поведение прототипа.
    Rtl,
}

impl LayoutDirection {
    /// Конвертация из настройки canvas-core `explain_sources_left`
    /// (core не знает тип направления — переводится здесь, в app).
    pub fn from_sources_left(sources_left: bool) -> Self {
        if sources_left {
            Self::Ltr
        } else {
            Self::Rtl
        }
    }

    /// Обратная конвертация в настройку `explain_sources_left`.
    pub fn sources_left(self) -> bool {
        matches!(self, Self::Ltr)
    }
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
/// FR-084: шапка таблицы-карточки (заголовок узла-источника).
pub const TABLE_HEADER_H: f32 = 30.0;
/// FR-084: шаг строки таблицы (значение строки Numi-листа).
pub const TABLE_ROW_STEP: f32 = 22.0;
/// FR-084: защищённая зона иконки-карандаша в конце строки таблицы
/// (текст строки не заходит в зону — пересечение невозможно).
pub const EDIT_ICON_ZONE: f32 = 18.0;
/// FR-084: глиф иконки редактирования строки (решение владельца:
/// иконка вместо текстовой кнопки «Изменить» — стандарт UX).
pub const EDIT_ICON_GLYPH: &str = "✎";
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
    // (в пределах поддерева вида). FR-060/G5: без break — страж корня
    // в match-ветке (корень проверяется как предок, дальше цепочки не идём —
    // семантика прежнего while-break дословно).
    let ancestor_expanded = |mut i: usize| -> bool {
        loop {
            match vis.parent[i] {
                Some(p) if p != root_idx => {
                    if expanded.contains(&p) {
                        return true;
                    }
                    i = p;
                }
                Some(p) => return expanded.contains(&p),
                None => return false,
            }
        }
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

/// FR-084: строка таблицы-карточки — ссылка на лист дерева и локальная
/// геометрия внутри карточки (до fit-масштаба).
#[derive(Debug, Clone, PartialEq)]
pub struct LaidRow {
    /// Индекс листа в [`LineageTree::nodes`].
    pub leaf_idx: usize,
    /// Верх строки, локальные px карточки (после шапки).
    pub y: f32,
    /// Высота строки (= [`TABLE_ROW_STEP`]).
    pub h: f32,
    /// Y центра строки — порт кривой со стороны родителя.
    pub port_y: f32,
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
    /// FR-084: строки таблицы-карточки — сестринские листья одного
    /// `node_id` под одним родителем, DFS-порядок (порядок children).
    /// Пусто — обычная карточка (не-лист или лист-корень без родителя).
    pub rows: Vec<LaidRow>,
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
/// FR-083: `direction` задаёт сторону источников — лейаут строится как
/// раньше (корень слева, [`LayoutDirection::Rtl`]), затем для Ltr
/// зеркалится по X (`x = bounds_w − x − CARD_W`, точки кривых
/// `p[0] = bounds_w − p[0]`; bounds после зеркалирования те же — ширина
/// контента не меняется). Вертикальный порядок детей и порядок Vec
/// nodes/curves не меняются; порты кривых зеркалятся преобразованием
/// (родитель принимает ребро со стороны источников, отдаёт в сторону
/// детей). FR-084: сестринские листья одного `node_id` под одним
/// родителем образуют ОДНУ карточку-таблицу (строки — [`LaidNode::rows`]);
/// карточка занимает место ПЕРВОГО листа группы (порядок обхода сохранён),
/// высота — шапка + строки, кривые к листам приходят в порты строк.
pub fn layout_tree(
    tree: &LineageTree,
    vis: &Visibility,
    root_idx: usize,
    direction: LayoutDirection,
) -> TreeLayout {
    let mut layout = TreeLayout::default();
    if root_idx >= tree.nodes.len() || !vis.visible[root_idx] {
        return layout;
    }
    let mut next_row: f32 = 0.0;
    let mut rows: Vec<f32> = vec![0.0; tree.nodes.len()];
    // FR-084: открытые группы листьев — (индекс родителя, node_id) →
    // индекс карточки-таблицы в layout.nodes (первый лист группы создаёт
    // карточку, остальные её расширяют); leaf_ports — абсолютный Y порта
    // строки каждого сгруппированного листа (кривые родителя приходят
    // в эти порты).
    let mut open_groups: BTreeMap<(usize, String), usize> = BTreeMap::new();
    let mut leaf_ports: BTreeMap<usize, f32> = BTreeMap::new();
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
                // FR-084: сестринские листья одного узла-источника (одинаковый
                // node_id под одним родителем) схлопываются в карточку-таблицу.
                // Карточка создаётся на месте ПЕРВОГО листа группы (его Exit —
                // порядок layout.nodes сохранён) и расширяется последующими
                // сёстрами той же группы; ряды дерева (next_row) назначены
                // каждому листу по-прежнему, rect[1] — ряд первого листа.
                let is_leaf = tree.nodes[idx].kind == LineageNodeKind::Leaf;
                let card_index = match (is_leaf, vis.parent[idx]) {
                    (true, Some(parent)) => {
                        let node_id = tree.nodes[idx].node_id.clone();
                        let existing = open_groups.get(&(parent, node_id.clone())).copied();
                        match existing {
                            Some(card_i) => {
                                // Расширяем таблицу: строка за строкой в
                                // DFS-порядке листьев (порядок children).
                                let card = &mut layout.nodes[card_i];
                                let n = card.rows.len() as f32;
                                let row_y = TABLE_HEADER_H + n * TABLE_ROW_STEP;
                                let laid_row = LaidRow {
                                    leaf_idx: idx,
                                    y: row_y,
                                    h: TABLE_ROW_STEP,
                                    port_y: row_y + TABLE_ROW_STEP / 2.0,
                                };
                                leaf_ports.insert(idx, card.rect[1] + laid_row.port_y);
                                card.rows.push(laid_row);
                                card.rect[3] = TABLE_HEADER_H + (n + 1.0) * TABLE_ROW_STEP;
                                card_i
                            }
                            None => {
                                let laid_row = LaidRow {
                                    leaf_idx: idx,
                                    y: TABLE_HEADER_H,
                                    h: TABLE_ROW_STEP,
                                    port_y: TABLE_HEADER_H + TABLE_ROW_STEP / 2.0,
                                };
                                layout.nodes.push(LaidNode {
                                    idx,
                                    col,
                                    row,
                                    rect: [x, y, CARD_W, TABLE_HEADER_H + TABLE_ROW_STEP],
                                    via,
                                    rows: vec![laid_row.clone()],
                                });
                                let card_i = layout.nodes.len() - 1;
                                leaf_ports.insert(idx, y + laid_row.port_y);
                                open_groups.insert((parent, node_id), card_i);
                                card_i
                            }
                        }
                    }
                    // Не-лист (Calc/Cycle/прочее) и лист-корень (родителя
                    // нет — группировать не с кем) — обычная карточка.
                    _ => {
                        layout.nodes.push(LaidNode {
                            idx,
                            col,
                            row,
                            rect: [x, y, CARD_W, CARD_H],
                            via,
                            rows: Vec::new(),
                        });
                        layout.nodes.len() - 1
                    }
                };
                // Ветки: родитель → видимые дети (ряды детей уже в rows —
                // их Exit был раньше). Прототип: безье от правого порта
                // родителя к левому порту ребёнка, середина по X. FR-084:
                // порт родителя — из фактического rect (у таблицы высота
                // своя; у обычной карточки это прежний y + CARD_H/2), а
                // кривая к листу приходит в порт ЕГО строки таблицы.
                let parent_rect = layout.nodes[card_index].rect;
                let px = parent_rect[0] + parent_rect[2];
                let py = parent_rect[1] + parent_rect[3] / 2.0;
                for &c in &visible_children {
                    let ccol = col + 1;
                    let cx = LAYOUT_PAD + ccol as f32 * COL_W;
                    let cy = if tree.nodes[c].kind == LineageNodeKind::Leaf {
                        leaf_ports.get(&c).copied().unwrap_or_else(|| {
                            // Недостижимо: каждый видимый лист Exit-ится
                            // раньше родителя и попадает в leaf_ports;
                            // защита от паники — прежняя середина карточки.
                            LAYOUT_PAD + rows[c] * ROW_H + CARD_H / 2.0
                        })
                    } else {
                        LAYOUT_PAD + rows[c] * ROW_H + CARD_H / 2.0
                    };
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
    // FR-083: Ltr — зеркалирование по X (корень становится самым правым).
    // bounds_w берётся ДО зеркалирования; после преобразования max(x + CARD_W)
    // = bounds_w − min(x) = bounds_w − LAYOUT_PAD = max_w — bounds те же.
    if direction == LayoutDirection::Ltr {
        let bounds_w = layout.bounds[0];
        for node in &mut layout.nodes {
            node.rect[0] = bounds_w - node.rect[0] - CARD_W;
        }
        for curve in &mut layout.curves {
            for p in &mut curve.points {
                p[0] = bounds_w - p[0];
            }
        }
    }
    layout
}

/// Пол fit-масштаба (FR-083, порог читаемости; FR-084): при масштабе 0.7
/// обычная карточка 158×74 даёт 110.6×51.8 screen-px — в неё помещаются
/// 4 строки текста с полом шрифта 8 px и межстрочным интервалом 1.5
/// (4 × 8 × 1.5 = 48 ≤ 51.8); строка таблицы-карточки [`TABLE_ROW_STEP`]
/// даёт 15.4 px ≥ 12 px (8 × 1.5). Обоснование — тест
/// `fit_scale_floor_keeps_text_readable`. Глубже — строки наезжают друг
/// на друга, вместо сжатия включается панорамирование ([`pan_clamp`]).
pub const SCALE_MIN: f32 = 0.7;

/// Fit-масштаб лейаута в тело окна: потолок 1.0 — только сжатие (без
/// растяжения маленьких деревьев), пол [`SCALE_MIN`] — глубже текст
/// нечитаем (FR-083: вместо сжатия — панорамирование).
pub fn fit_scale(bounds: [f32; 2], body: [f32; 4]) -> f32 {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(1.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(1.0);
    (1.0_f32)
        .min(avail_w / bounds[0].max(1.0))
        .min(avail_h / bounds[1].max(1.0))
        .max(SCALE_MIN)
}

/// Потолок пользовательского зума (FR-085): эффективный масштаб кадра =
/// [`base_scale`] · zoom ∈ [`SCALE_MIN`, `ZOOM_MAX`].
pub const ZOOM_MAX: f32 = 2.5;

/// Шаг зума колеса окна проверки за щелчок (CR-017: ±5 %, паритет с
/// `ZOOM_STEP_PER_LINE` основного канваса).
pub const ZOOM_STEP_PER_NOTCH: f32 = 1.05;

/// Вертикальная составляющая колеса для зума окна проверки — нормализованный
/// ввод без winit-типов (модуль чистый): [`WheelInput::Line`] — щелчки колеса
/// (LineDelta y, ±1 за щелчок), [`WheelInput::Pixel`] — физические пиксели
/// тачпада (PixelDelta y).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelInput {
    Line(f32),
    Pixel(f32),
}

/// Фактор зума колеса окна проверки (ревизия владельца 2026-10-02): знак и
/// шаг — паритет с основным канвасом (фактор Ctrl+колеса). Колесо ВВЕРХ
/// (Line(1.0)) — зум IN на +5 % за щелчок (CR-017); тачпад (Pixel) — плавный
/// экспоненциальный фактор, как на канвасе. Прежняя версия `1.05^(−y·40)`
/// была инвертирована (колесо вверх зумило OUT) и умножала щелчок на 40
/// строк-единиц — фактор 1.05^±40 бился в клампы [`SCALE_MIN`]/[`ZOOM_MAX`]
/// за один щелчок.
pub fn wheel_zoom_factor(input: WheelInput) -> f32 {
    match input {
        WheelInput::Line(y) => ZOOM_STEP_PER_NOTCH.powf(y),
        WheelInput::Pixel(y) => (y * 0.00125).exp(),
    }
}

/// Базовый масштаб кадра без пользовательского зума (FR-085): обычный
/// режим — [`fit_scale`], режим защиты — [`defense_fit_scale`] (укрупнение
/// ×1.5). Единая точка для рендера и ввода (зум-лимиты колеса).
pub fn base_scale(bounds: [f32; 2], body: [f32; 4], defense: bool) -> f32 {
    if defense {
        defense_fit_scale(bounds, body)
    } else {
        fit_scale(bounds, body)
    }
}

/// Пан при зуме к курсору (FR-085, чистая функция): контентная точка под
/// курсором сохраняется. До зума: `cursor = body + BODY_PAD + center(s_old)
/// + pan + c·s_old`; после — то же с `s_new` и пересчитанным центрированием
/// ([`content_origin`]). Возвращает НЕСКЛампленный пан — вызывающий
/// прогоняет через [`pan_clamp`] с новым масштабом.
pub fn zoom_adjust_pan(
    pan: [f32; 2],
    cursor: [f32; 2],
    body: [f32; 4],
    bounds: [f32; 2],
    scale_old: f32,
    scale_new: f32,
) -> [f32; 2] {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(0.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(0.0);
    let center = |_s: f32, avail: f32, content: f32| ((avail - content) / 2.0).max(0.0);
    let mut out = [0.0; 2];
    for axis in 0..2 {
        let avail = if axis == 0 { avail_w } else { avail_h };
        let content_old = bounds[axis] * scale_old;
        let content_new = bounds[axis] * scale_new;
        let c = (cursor[axis]
            - body[axis]
            - BODY_PAD
            - center(scale_old, avail, content_old)
            - pan[axis])
            / scale_old.max(f32::EPSILON);
        out[axis] = cursor[axis]
            - body[axis]
            - BODY_PAD
            - center(scale_new, avail, content_new)
            - c * scale_new;
    }
    out
}

/// Узел под точкой `point` (логические px окна): обратный обход — верхние
/// карточки позже в списке (рисуются поверх). FR-083: пан тела не входит
/// в геометрию — вызывающий передаёт `point − pan` (та же трансформация,
/// что у рендера с паном).
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

/// Кламп пана дерева в теле окна (FR-083, чистая функция): контент
/// размером `bounds * scale` рисуется в `body` с отступом [`BODY_PAD`];
/// ось, где контент помещается, — pan = 0 (контент прижат к левому-верхнему
/// углу); переполняющая ось — pan клампится в `[avail − content, 0]`
/// (отрицательный pan сдвигает контент влево/вверх, правый/нижний край
/// становится достижим).
pub fn pan_clamp(pan: [f32; 2], bounds: [f32; 2], scale: f32, body: [f32; 4]) -> [f32; 2] {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(0.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(0.0);
    let content_w = bounds[0] * scale;
    let content_h = bounds[1] * scale;
    let pan_x = if content_w > avail_w {
        pan[0].clamp(avail_w - content_w, 0.0)
    } else {
        0.0
    };
    let pan_y = if content_h > avail_h {
        pan[1].clamp(avail_h - content_h, 0.0)
    } else {
        0.0
    };
    [pan_x, pan_y]
}

/// Индикаторы переполнения тела (FR-083): `[лево, право, верх, низ]` —
/// какие стрелки показывать. Окно в контентных координатах:
/// `[-pan, -pan + avail]`; стрелка у края — если за ним есть скрытый
/// контент (переполнение возможно, только если `content > avail` по оси).
pub fn overflow_arrows(pan: [f32; 2], bounds: [f32; 2], scale: f32, body: [f32; 4]) -> [bool; 4] {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(0.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(0.0);
    let content_w = bounds[0] * scale;
    let content_h = bounds[1] * scale;
    const EPS: f32 = 0.5;
    let left = content_w > avail_w && pan[0] < -EPS;
    let right = content_w > avail_w && -pan[0] + avail_w < content_w - EPS;
    let top = content_h > avail_h && pan[1] < -EPS;
    let bottom = content_h > avail_h && -pan[1] + avail_h < content_h - EPS;
    [left, right, top, bottom]
}

/// FR-084: базовая точка контента в теле окна — [`BODY_PAD`] плюс
/// центрирование по оси, где контент меньше доступной области
/// (`body − 2·[`BODY_PAD`]`), плюс пан. Переполняющая ось — центрирование 0
/// (контент прижат к левому-верхнему углу, работает кламп пана
/// [`pan_clamp`] — вызывающий передаёт пан уже после клампа). Одна
/// геометрия для рендера и hit-теста (детерминизм).
pub fn content_origin(pan: [f32; 2], bounds: [f32; 2], scale: f32, body: [f32; 4]) -> [f32; 2] {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(0.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(0.0);
    let content_w = bounds[0] * scale;
    let content_h = bounds[1] * scale;
    [
        BODY_PAD + (avail_w - content_w).max(0.0) / 2.0 + pan[0],
        BODY_PAD + (avail_h - content_h).max(0.0) / 2.0 + pan[1],
    ]
}

// --- what-if из дерева (PRD-0007 X3, F-6/AC-4.1) ---------------------------

/// Кнопка «Изменить» на карточке ЛИСТА (AC-4.1): правый нижний угол
/// карточки. `rect` — локальные px карточки, `scale` — fit-масштаб.
/// (FR-084: устарела — заменена зонами иконок строк; будет удалена
/// интегратором.)
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

/// FR-084: прямоугольник иконки-карандаша строки таблицы в экранных px
/// (карточка уже преобразована локальный→экранный): правый край строки,
/// ширина [`EDIT_ICON_ZONE`], вертикаль — строка целиком. Одна геометрия
/// для рендера и hit-теста (детерминизм). Страж `max(card.x)` — иконка
/// не вылезает за карточку на вырожденной ширине.
///
/// Ревизия владельца 2026-10-02 (дефект «полоса рода наезжает на
/// карандаш»): `strip_inset` — ширина полосы-акцента рода узла у ПРАВОГО
/// края карточки (см. [`edit_icon_right_inset`]) — зона иконки отступает
/// от полосы, обе геометрии (рендер и hit) смещаются согласованно.
pub fn row_edit_rect(
    card_screen: [f32; 4],
    row_y: f32,
    row_h: f32,
    scale: f32,
    strip_inset: f32,
) -> [f32; 4] {
    let w = EDIT_ICON_ZONE * scale;
    let x = (card_screen[0] + card_screen[2] - w - strip_inset).max(card_screen[0]);
    [x, card_screen[1] + row_y * scale, w, row_h * scale]
}

/// Ревизия владельца 2026-10-02: ширина полосы-акцента рода узла
/// (рендер карточек — `(4.0 * scale).max(2.0)`), от которой отступает
/// зона иконки строки, когда полоса на стороне иконки (Ltr — правый
/// край). Единый источник для рендера и hit-теста (детерминизм).
pub fn edit_icon_right_inset(scale: f32, strip_on_right: bool) -> f32 {
    if strip_on_right {
        (4.0 * scale).max(2.0)
    } else {
        0.0
    }
}

/// FR-084: ширина текста строки таблицы — карточка минус паддинги (16) и
/// защищённая зона иконки ([`EDIT_ICON_ZONE`]); текст физически не заходит
/// под иконку. Пол 0.0.
pub fn table_row_text_width(card_w: f32, scale: f32) -> f32 {
    ((card_w - 16.0 - EDIT_ICON_ZONE) * scale).max(0.0)
}

/// Inline-поле подмены (X3) — тултип у якоря (FR-085): под строкой
/// карточки (+6 px); если снизу не влезает — над якорем; X клампится в
/// тело. Одна геометрия для рендера и hit-теста (детерминизм). Якорь —
/// экранный rect строки/карточки ([`edit_anchor_rect`]).
pub fn field_rect(body: [f32; 4], anchor: [f32; 4]) -> [f32; 4] {
    const W: f32 = 260.0;
    const H: f32 = 26.0;
    const GAP: f32 = 6.0;
    let x = anchor[0].clamp(
        body[0] + BODY_PAD,
        (body[0] + body[2] - W - BODY_PAD).max(body[0] + BODY_PAD),
    );
    let y_below = anchor[1] + anchor[3] + GAP;
    let y_above = anchor[1] - H - GAP;
    let body_bottom = body[1] + body[3] - BODY_PAD;
    let y = if y_below + H <= body_bottom {
        y_below
    } else if y_above >= body[1] + BODY_PAD {
        y_above
    } else {
        // Оба варианта не влезают (якорь выше тела — теоретический случай):
        // кламп к телу.
        y_below.clamp(body[1] + BODY_PAD, body_bottom - H)
    };
    [x, y, W, H]
}

/// Экранный rect якоря поля подмены (FR-085): строка таблицы
/// ([`LaidNode::rows`]) с листом `leaf_idx` — полоса строки; карточка без
/// строк — вся карточка. Координаты — экранные (body + BODY_PAD +
/// rect·scale + origin), как у рендера.
pub fn edit_anchor_rect(
    layout: &TreeLayout,
    leaf_idx: usize,
    scale: f32,
    body: [f32; 4],
    origin: [f32; 2],
) -> Option<[f32; 4]> {
    for laid in &layout.nodes {
        let x = body[0] + BODY_PAD + laid.rect[0] * scale + origin[0];
        let y = body[1] + BODY_PAD + laid.rect[1] * scale + origin[1];
        if laid.rows.is_empty() {
            if laid.idx == leaf_idx {
                return Some([x, y, laid.rect[2] * scale, laid.rect[3] * scale]);
            }
        } else {
            for row in &laid.rows {
                if row.leaf_idx == leaf_idx {
                    return Some([x, y + row.y * scale, laid.rect[2] * scale, row.h * scale]);
                }
            }
        }
    }
    None
}

/// Иконка редактирования под точкой: обходит карточки (обратный порядок —
/// верхние позже; hit-тест той же геометрии, что у рендера). FR-084:
/// точка — БЕЗ origin/базы контента (вызывающий вычитает origin из
/// курсора; экранный rect карточки здесь — `body + BODY_PAD + rect*scale`,
/// как раньше без пана). Для карточки-таблицы ([`LaidNode::rows`]) зона
/// редактирования — иконка-карандаш в правом краю СТРОКИ
/// ([`row_edit_rect`]): хит возвращает индекс ЛИСТА СТРОКИ (подмена
/// адресует строку Numi-листа), редактируемость проверяется на лист
/// строки. Карточка без строк (не-лист или лист-корень без группы) —
/// фолбэк на прежнюю кнопку в правом нижнем углу ([`edit_rect`]) — чтобы
/// не сломать не-табличные редактируемые карточки, если появятся
/// (обычно редактируемые — всегда таблицы).
pub fn edit_at(
    tree: &LineageTree,
    layout: &TreeLayout,
    scale: f32,
    body: [f32; 4],
    point: [f32; 2],
    strip_inset: f32,
) -> Option<usize> {
    // Подмена адресует строку Numi-листа: у итога-программы/шаблона её
    // нет (line: None) — иконка не показывается (X3-скоуп).
    let editable = |idx: usize| -> bool {
        match tree.nodes.get(idx) {
            Some(node) => {
                node.kind == LineageNodeKind::Leaf
                    && node.line.is_some()
                    && matches!(&node.value, Some(Ok(_)))
            }
            None => false,
        }
    };
    for laid in layout.nodes.iter().rev() {
        let [x, y] = [
            body[0] + BODY_PAD + laid.rect[0] * scale,
            body[1] + BODY_PAD + laid.rect[1] * scale,
        ];
        let card = [x, y, laid.rect[2] * scale, laid.rect[3] * scale];
        // FR-084: таблица-карточка — построчный hit по иконкам строк
        // (строки не перекрываются, порядок обхода не важен).
        for row in &laid.rows {
            if !editable(row.leaf_idx) {
                continue;
            }
            let rect = row_edit_rect(card, row.y, row.h, scale, strip_inset);
            if point[0] >= rect[0]
                && point[0] <= rect[0] + rect[2]
                && point[1] >= rect[1]
                && point[1] <= rect[1] + rect[3]
            {
                return Some(row.leaf_idx);
            }
        }
        // Фолбэк: карточка без строк — прежняя кнопка в правом нижнем
        // углу (лист-корень без группы; X3/AC-4.1).
        if laid.rows.is_empty() && editable(laid.idx) {
            let rect = edit_rect(card, scale);
            if point[0] >= rect[0]
                && point[0] <= rect[0] + rect[2]
                && point[1] >= rect[1]
                && point[1] <= rect[1] + rect[3]
            {
                return Some(laid.idx);
            }
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
/// потолок 1.0: обычный вид только сжимает). FR-083: пол [`SCALE_MIN`] —
/// глубже строки наезжают (переполнение закрывается панорамированием).
pub fn defense_fit_scale(bounds: [f32; 2], body: [f32; 4]) -> f32 {
    let avail_w = (body[2] - BODY_PAD * 2.0).max(1.0);
    let avail_h = (body[3] - BODY_PAD * 2.0).max(1.0);
    (avail_w / bounds[0].max(1.0))
        .min(avail_h / bounds[1].max(1.0))
        .clamp(SCALE_MIN, DEFENSE_SCALE_MAX)
}

/// Размер кнопки-тумблера «Режим защиты» (AC-6.1 — одним действием).
/// Ревизия владельца 2026-10-02 (дефект «мелкие контролы»): кегль
/// подписи — 13 (BUTTON_FONT_SIZE кита), высота — 32 (INPUT_HEIGHT
/// панели шаблонов — нижняя граница комфортного нажатия).
pub const DEFENSE_TOGGLE_W: f32 = 152.0;
pub const DEFENSE_TOGGLE_H: f32 = 32.0;

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

/// Ширина кнопки-тумблера направления схемы (FR-083) — квад под
/// иконку-стрелку (кегль 13 — паритет соседям по шапке); высота — как у
/// defense-тумблера.
pub const DIRECTION_TOGGLE_W: f32 = 40.0;
/// Высота тумблера направления — как у defense-тумблера (FR-083).
pub const DIRECTION_TOGGLE_H: f32 = DEFENSE_TOGGLE_H;

/// Кнопка-тумблер направления схемы (FR-083) — в шапке СЛЕВА от
/// [`defense_toggle_rect`] с зазором 8 px, вертикальное центрирование по
/// [`HEADER_H`] (как у соседей).
pub fn direction_toggle_rect(win: [f32; 4]) -> [f32; 4] {
    let defense = defense_toggle_rect(win);
    [
        defense[0] - DIRECTION_TOGGLE_W - 8.0,
        win[1] + (HEADER_H - DIRECTION_TOGGLE_H) / 2.0,
        DIRECTION_TOGGLE_W,
        DIRECTION_TOGGLE_H,
    ]
}

/// Виден ли тумблер направления на этом окне (FR-083, скрытие на узких
/// окнах): кнопка прячется, когда её левый край заходит левее границы
/// мета-зоны/крошек (`meta_rect` — левый край зоны подзаголовка в шапке;
/// на узких окнах цепочка кнопок правого края наезжает на неё).
pub fn direction_toggle_visible(win: [f32; 4]) -> bool {
    direction_toggle_rect(win)[0] >= meta_rect(win)[0]
}

/// Кнопка «Раскрыть уровень» (AC-6.3, шаг) — правый край футера; Defense.
/// Высота — паритет тумблерам шапки (ревизия «мелкие контролы»).
pub fn defense_step_rect(win: [f32; 4]) -> [f32; 4] {
    const W: f32 = 170.0;
    [
        win[0] + win[2] - W - BODY_PAD,
        win[1] + win[3] - FOOTER_H + (FOOTER_H - DEFENSE_TOGGLE_H) / 2.0,
        W,
        DEFENSE_TOGGLE_H,
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

/// Панорамирование тела драгом по фону (FR-083): якорь (курсор нажатия)
/// и pan на момент нажатия — на движении `pan = pan_start + (курсор −
/// якорь)` с клампом [`pan_clamp`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanDrag {
    /// Курсор нажатия (логические px окна).
    pub anchor: [f32; 2],
    /// Pan на момент нажатия.
    pub pan_start: [f32; 2],
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
    /// Направление потока схемы (FR-083): тумблер в шапке; выбор
    /// сохраняется в настройках (`explain_sources_left`). Дефолт — Ltr;
    /// при открытии окна App инициализирует из настроек.
    pub direction: LayoutDirection,
    /// Пан содержимого дерева в теле окна (FR-083): `[dx, dy]`, кламп —
    /// [`pan_clamp`] (контент прижат к левому-верхнему углу). Сбрасывается
    /// при смене вида/защиты/направления/открытии.
    pub pan: [f32; 2],
    /// FR-085: пользовательский зум (колесо к курсору); эффективный
    /// масштаб кадра = [`base_scale`] · zoom. Диапазон effective —
    /// [`SCALE_MIN`]…`base_scale`·[`ZOOM_MAX`]. Сбрасывается вместе с
    /// паном ([`Self::reset_pan`]).
    pub zoom: f32,
    /// Драг-панорамирование фона тела (FR-083): Some — драг идёт.
    /// Runtime-состояние — не сериализуется.
    pub pan_drag: Option<PanDrag>,
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
            direction: LayoutDirection::default(),
            pan: [0.0; 2],
            zoom: 1.0,
            pan_drag: None,
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
            direction: LayoutDirection::default(),
            pan: [0.0; 2],
            zoom: 1.0,
            pan_drag: None,
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
    /// FR-060/G5: `Option::take` — перенос владения состоянием, не срез
    /// контента (аудит G5 — про срезы `take(n)`/break-клампы/`truncate_chars`)
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

    /// Сброс пана (FR-083): смена вида/защиты/направления возвращает окно
    /// к левому-верхнему углу контента. FR-085: вместе с паном сбрасывается
    /// и пользовательский зум (вид схемы полностью меняется).
    pub fn reset_pan(&mut self) {
        self.pan = [0.0; 2];
        self.zoom = 1.0;
        self.pan_drag = None;
    }

    /// Тумблер направления схемы (FR-083): переключает [`LayoutDirection`]
    /// и сбрасывает пан (вид схемы полностью меняется). Сохранение в
    /// настройки — на стороне App (core не знает тип направления).
    pub fn toggle_direction(&mut self) {
        self.direction = match self.direction {
            LayoutDirection::Ltr => LayoutDirection::Rtl,
            LayoutDirection::Rtl => LayoutDirection::Ltr,
        };
        self.reset_pan();
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
            // Вид сменился — У2-выделение и пан сбрасываются (FR-083).
            self.pick = None;
            self.pick_at = None;
            self.reset_pan();
            return NodeClick::Focused;
        }
        NodeClick::Leaf
    }

    /// Клик по крошке `level` (0 — корень): путь вида обрезается.
    pub fn click_crumb(&mut self, level: usize) {
        if level < self.view_path.len() {
            self.view_path.truncate(level + 1);
        }
        // Вид сменился — У2-выделение и пан сбрасываются (узел может быть
        // вне нового поддерева вида; FR-083).
        self.pick = None;
        self.pick_at = None;
        self.reset_pan();
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
        // FR-083: «подвод» узла — пан возвращается к началу (смена вида).
        self.reset_pan();
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
        // FR-083: вид сброшен — пан тоже.
        self.reset_pan();
    }

    /// Выйти из режима защиты (AC-6.4, Defense → Ready): окно возвращается
    /// к обычному виду (путь крошек и ручные раскрытия как до входа);
    /// снапшот не меняется, канвас не затрагивается. FR-083: пан сбрасывается.
    pub fn exit_defense(&mut self) {
        if !self.defense {
            return;
        }
        self.defense = false;
        if let Some((path, expanded)) = self.pre_defense.take() {
            self.view_path = path;
            self.expanded = expanded;
        }
        self.reset_pan();
    }

    /// Шаг раскрытия (AC-6.3: пробел/кнопка — следующий уровень дерева от
    /// корня). `false` — шагать некуда (защита не активна, «раскрыть всё»
    /// уже нажато — 0, или скрытых уровней нет — проверка на App-стороне
    /// через [`has_hidden`]). FR-083: пан сбрасывается (вид изменился).
    pub fn defense_step(&mut self) -> bool {
        if !self.defense || self.defense_reveal == 0 {
            return false;
        }
        self.defense_reveal = self.defense_reveal.saturating_add(1);
        self.reset_pan();
        true
    }

    /// «Раскрыть всё» (AC-6.3): снять ограничение уровней (0 — без лимита,
    /// семантика [`visibility`]). FR-083: пан сбрасывается (вид изменился).
    pub fn defense_reveal_all(&mut self) {
        if self.defense {
            self.defense_reveal = 0;
            self.reset_pan();
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

    /// FR-084: лист-строка Numi-листа с заданным node_id (детей нет).
    fn leaf_node(node_id: &str) -> LineageNode {
        LineageNode {
            node_id: node_id.into(),
            line: None,
            kind: LineageNodeKind::Leaf,
            value: None,
            formula: Some("1".into()),
            title: "L".into(),
            label: None,
            children: Vec::new(),
        }
    }

    /// FR-084: родитель R (Calc) с тремя листьями-строками одного узла-
    /// источника «src» (индексы 1–3) — группа для карточки-таблицы.
    fn group_tree() -> LineageTree {
        LineageTree {
            root: LineageNodeId::total("r"),
            nodes: vec![
                LineageNode {
                    node_id: "r".into(),
                    line: None,
                    kind: LineageNodeKind::Calc,
                    value: None,
                    formula: Some("$1 + $2 + $3".into()),
                    title: "R".into(),
                    label: None,
                    children: vec![
                        LineageChild {
                            child: 1,
                            via: None,
                        },
                        LineageChild {
                            child: 2,
                            via: None,
                        },
                        LineageChild {
                            child: 3,
                            via: None,
                        },
                    ],
                },
                leaf_node("src"),
                leaf_node("src"),
                leaf_node("src"),
            ],
        }
    }

    /// FR-083: окно = 80 % вьюпорта без потолка — на большом вьюпорте
    /// ровно 80 % и центрировано (осознанное ломающее изменение
    /// «прототипа v4»: потолки 1320×900 удалены).
    #[test]
    fn window_rect_is_80_percent_and_centered() {
        let vp = [2560.0, 1300.0];
        let win = window_rect(vp);
        // 80 % вьюпорта без потолка (прежний потолок 1320×900 ограничивал).
        assert!((win[2] - 2048.0).abs() < 1e-3, "ширина 80 %: {}", win[2]);
        assert!((win[3] - 1040.0).abs() < 1e-3, "высота 80 %: {}", win[3]);
        assert!((win[0] - 256.0).abs() < 1e-3, "x = (vp−w)/2: {}", win[0]);
        assert!((win[1] - 130.0).abs() < 1e-3, "y = (vp−h)/2: {}", win[1]);
        // Центрировано на вьюпорте.
        assert!((win[0] + win[2] / 2.0 - vp[0] / 2.0).abs() < 1e-3);
        assert!((win[1] + win[3] / 2.0 - vp[1] / 2.0).abs() < 1e-3);
        // Обычный вьюпорт: те же 80 %.
        let mid = window_rect([1600.0, 1000.0]);
        assert!((mid[2] - 1280.0).abs() < 1e-3);
        assert!((mid[3] - 800.0).abs() < 1e-3);
        assert!((mid[0] + mid[2] / 2.0 - 800.0).abs() < 1e-3);
        assert!((mid[1] + mid[3] / 2.0 - 500.0).abs() < 1e-3);
    }

    /// FR-083: крошечный вьюпорт — окно прижато полями/инвариантом 320×240
    /// (kit::modal: min приоритетен — documented деградация), без паники.
    #[test]
    fn window_rect_tiny_viewport_clamped_without_panic() {
        // 340×250: слот 300×210 уже инварианта — панель сохраняет min
        // (320×240), сдвиг неотрицателен.
        let small = window_rect([340.0, 250.0]);
        assert!((small[2] - WIN_MIN_W).abs() < 1e-3, "инвариант ширины");
        assert!((small[3] - WIN_MIN_H).abs() < 1e-3, "инвариант высоты");
        assert!(small[0] >= -0.01, "без ухода за левый край: {}", small[0]);
        assert!(small[1] >= -0.01, "без ухода за верхний край: {}", small[1]);
        // Совсем крошечный вьюпорт: min приоритетен, паники нет.
        let tiny = window_rect([240.0, 200.0]);
        assert!((tiny[2] - WIN_MIN_W).abs() < 1e-3);
        assert!((tiny[3] - WIN_MIN_H).abs() < 1e-3);
    }

    /// FR-083: инвариант 320×240, когда 80 % вьюпорта меньше него
    /// (380×290: 80 % = 304×232 < min — окно ровно min и центрировано).
    #[test]
    fn window_rect_min_invariant_when_fraction_smaller() {
        let win = window_rect([380.0, 290.0]);
        assert!((win[2] - WIN_MIN_W).abs() < 1e-3);
        assert!((win[3] - WIN_MIN_H).abs() < 1e-3);
        // Центрировано (поля симметричны, инвариант влезает в слот).
        assert!((win[0] + win[2] / 2.0 - 190.0).abs() < 1e-3);
        assert!((win[1] + win[3] / 2.0 - 145.0).abs() < 1e-3);
        // Инвариант соблюдается на всех перечисленных вьюпортах.
        for &(vw, vh) in &[
            (1600.0, 1000.0),
            (1280.0, 800.0),
            (900.0, 640.0),
            (420.0, 320.0),
            (360.0, 300.0),
            (2400.0, 1200.0),
            (700.0, 1100.0),
        ] {
            let win = window_rect([vw, vh]);
            let fits = vw - WIN_MARGIN * 2.0 >= WIN_MIN_W && vh - WIN_MARGIN * 2.0 >= WIN_MIN_H;
            if fits {
                assert!(
                    win[2] >= WIN_MIN_W - 1e-3 && win[3] >= WIN_MIN_H - 1e-3,
                    "инвариант ≥ 320×240 при {vw}×{vh}"
                );
            }
        }
    }

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
    /// числу видимых детей; fit-масштаб: потолок 1.0, пол [`SCALE_MIN`].
    #[test]
    fn layout_rows_columns_and_fit() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        // Все 4 узла видимы (глубина ≤ 3).
        assert_eq!(layout.nodes.len(), 4);
        assert_eq!(layout.curves.len(), 3);
        // Листья C (индекс 2, колонка 2) и D (индекс 3, колонка 1) —
        // разные ряды; B между ними по колонке 1.
        let by_idx =
            |l: &TreeLayout, i: usize| l.nodes.iter().find(|n| n.idx == i).unwrap().clone();
        let (c, d) = (by_idx(&layout, 2), by_idx(&layout, 3));
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
        // Fit-масштаб: маленькое дерево в большое тело — 1.0; в крошечное —
        // жмётся, но не глубже пола SCALE_MIN (FR-083: дальше — пан).
        assert_eq!(fit_scale(layout.bounds, [0.0, 0.0, 1200.0, 800.0]), 1.0);
        let tiny = fit_scale(layout.bounds, [0.0, 0.0, 300.0, 200.0]);
        assert!((SCALE_MIN..1.0).contains(&tiny));
    }

    /// FR-083/FR-084: пол fit-масштаба — дерево втрое больше тела не
    /// сжимается глубже SCALE_MIN (0.7): строка таблицы-карточки
    /// TABLE_ROW_STEP (22 px) даёт 15.4 screen-px ≥ 8 × 1.5 = 12 px (пол
    /// шрифта с межстрочным интервалом); обычная карточка 74 px даёт
    /// 51.8 px ≥ 48 px (4 строки текста) — текст читаем, глубже — пан.
    #[test]
    fn fit_scale_floor_keeps_text_readable() {
        // Дерево втрое больше тела по обеим осям.
        let scale = fit_scale([3000.0, 2000.0], [0.0, 0.0, 1000.0, 700.0]);
        assert!((scale - SCALE_MIN).abs() < 1e-3, "пол 0.7, получен {scale}");
        // Таблица-строка: TABLE_ROW_STEP × SCALE_MIN ≥ 8 × 1.5.
        let row_h = TABLE_ROW_STEP * SCALE_MIN;
        assert!(
            row_h >= 8.0 * 1.5 - 1e-3,
            "строка таблицы {row_h} ≥ минимума 12 px"
        );
        // Обычная карточка 158×74 при SCALE_MIN держит 4 строки текста.
        let card_h = CARD_H * SCALE_MIN;
        let rows = 4.0 * 8.0 * 1.5;
        assert!(
            rows <= card_h + 1e-3,
            "4 строки ({rows}) в карточке ({card_h})"
        );
        // Потолок 1.0 не сломан: маленькое дерево — ровно 1.0.
        assert_eq!(fit_scale([100.0, 100.0], [0.0, 0.0, 1000.0, 700.0]), 1.0);
    }

    /// FR-083: pan_clamp — без переполнения pan = [0, 0]; переполнение по
    /// обеим осям — кламп в [avail − content, 0] × [avail − content, 0];
    /// частичный кламп — только по переполняющей оси.
    #[test]
    fn pan_clamp_cases() {
        let body = [0.0, 0.0, 500.0, 400.0];
        // Нет переполнения (контент 100×100 в avail 468×368) — pan = [0, 0].
        assert_eq!(
            pan_clamp([-50.0, -50.0], [100.0, 100.0], 1.0, body),
            [0.0, 0.0]
        );
        // Переполнение по обеим осям (контент 1000×800): диапазоны
        // x ∈ [468−1000, 0] = [−532, 0], y ∈ [368−800, 0] = [−432, 0].
        assert_eq!(
            pan_clamp([-600.0, 100.0], [1000.0, 800.0], 1.0, body),
            [-532.0, 0.0]
        );
        // Частичный кламп: по X переполнение, по Y контент помещается.
        assert_eq!(
            pan_clamp([-900.0, -30.0], [1000.0, 100.0], 1.0, body),
            [-532.0, 0.0]
        );
        // Положительный pan (тянем контент вправо-вниз) клампится в 0.
        assert_eq!(
            pan_clamp([40.0, 40.0], [1000.0, 800.0], 1.0, body),
            [0.0, 0.0]
        );
        // Масштаб учитывается: контент bounds×scale = 500×50, диапазон
        // X: [468−500, 0] = [−32, 0].
        assert_eq!(
            pan_clamp([-200.0, 0.0], [1000.0, 100.0], 0.5, body),
            [-32.0, 0.0]
        );
    }

    /// FR-083: индикаторы переполнения — стрелки только у краёв со
    /// скрытым контентом; без переполнения стрелок нет; при промотке до
    /// края стрелка у достигнутого края гаснет, у противоположного горит.
    #[test]
    fn overflow_arrows_sides() {
        let body = [0.0, 0.0, 500.0, 400.0];
        // Нет переполнения — стрелок нет.
        assert_eq!(
            overflow_arrows([0.0, 0.0], [100.0, 100.0], 1.0, body),
            [false; 4]
        );
        // Переполнение, pan = 0 (контент прижат влево-вверх): скрыты
        // правый и нижний края.
        assert_eq!(
            overflow_arrows([0.0, 0.0], [1000.0, 800.0], 1.0, body),
            [false, true, false, true]
        );
        // Промотано вправо-вниз до конца (pan = min): скрыты левый и верхний.
        assert_eq!(
            overflow_arrows([-532.0, -432.0], [1000.0, 800.0], 1.0, body),
            [true, false, true, false]
        );
    }

    /// Hit-тест узла: точка внутри прямоугольника (с масштабом) находит
    /// узел; мимо — None; обратный обход — верхняя карточка.
    #[test]
    fn node_at_hits_topmost() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
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

    /// X3 (AC-4.1): иконка «Изменить» — только на редактируемых листьях
    /// (Leaf + line: Some + значение Ok); hit-тест edit_at. FR-084: зона —
    /// иконка-карандаш у ПРАВОГО КРАЯ СТРОКИ таблицы-карточки (лист под
    /// родителем — таблица из одной строки).
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
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let body = [0.0, 0.0, 1200.0, 800.0];
        // Точка иконки строки листа c — правый край ЕГО строки (лист под
        // родителем — таблица из одной строки).
        let laid = layout
            .nodes
            .iter()
            .find(|n| n.idx == 2)
            .expect("лист c в лейауте");
        assert_eq!(laid.rows.len(), 1, "лист под родителем — таблица");
        let row = &laid.rows[0];
        let card_screen = [
            body[0] + BODY_PAD + laid.rect[0],
            body[1] + BODY_PAD + laid.rect[1],
            laid.rect[2],
            laid.rect[3],
        ];
        let rect = row_edit_rect(card_screen, row.y, row.h, 1.0, 0.0);
        let point = [rect[0] + rect[2] - 4.0, rect[1] + rect[3] / 2.0];
        assert_eq!(edit_at(&tree, &layout, 1.0, body, point, 0.0), Some(2));
        // Точка иконки строки листа d (не редактируемый) — None.
        let laid_d = layout
            .nodes
            .iter()
            .find(|n| n.idx == 3)
            .expect("лист d в лейауте");
        let row_d = &laid_d.rows[0];
        let card_d = [
            body[0] + BODY_PAD + laid_d.rect[0],
            body[1] + BODY_PAD + laid_d.rect[1],
            laid_d.rect[2],
            laid_d.rect[3],
        ];
        let rect_d = row_edit_rect(card_d, row_d.y, row_d.h, 1.0, 0.0);
        let point_d = [rect_d[0] + rect_d[2] - 4.0, rect_d[1] + rect_d[3] / 2.0];
        assert_eq!(edit_at(&tree, &layout, 1.0, body, point_d, 0.0), None);
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

    /// X3/FR-084 (AC-4.1): hit по иконке-карандашу строки таблицы
    /// возвращает индекс ЛИСТА ЭТОЙ строки (по таблице из 3 строк) —
    /// прежний тест edit_rect_stays_inside_card заменён: геометрия
    /// зоны покрыта row_edit_rect_and_text_width_geometry, здесь —
    /// построчная адресация edit_at (в т.ч. при масштабе < 1).
    #[test]
    fn edit_at_hits_table_row_icons_by_leaf() {
        let mut tree = group_tree();
        // Все три строки редактируемые (Leaf + line: Some + значение Ok).
        for (i, node) in tree.nodes.iter_mut().enumerate().skip(1) {
            node.line = Some(i - 1);
            node.value = Some(Ok(canvas_core::expr::Value::scalar(i as f64)));
        }
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        assert_eq!(
            card.rows.iter().map(|r| r.leaf_idx).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "строки таблицы в DFS-порядке"
        );
        let body = [0.0, 0.0, 1200.0, 800.0];
        for scale in [1.0f32, 0.7] {
            let card_screen = [
                body[0] + BODY_PAD + card.rect[0] * scale,
                body[1] + BODY_PAD + card.rect[1] * scale,
                card.rect[2] * scale,
                card.rect[3] * scale,
            ];
            for row in &card.rows {
                let rect = row_edit_rect(card_screen, row.y, row.h, scale, 0.0);
                // Точка внутри зоны: правый край минус 4, центр по вертикали.
                let point = [rect[0] + rect[2] - 4.0, rect[1] + rect[3] / 2.0];
                assert_eq!(
                    edit_at(&tree, &layout, scale, body, point, 0.0),
                    Some(row.leaf_idx),
                    "иконка строки → лист {} (scale {scale})",
                    row.leaf_idx
                );
            }
        }
    }

    /// FR-084: клик по ТЕКСТУ строки (вне зоны иконки) НЕ открывает
    /// редактирование — edit_at возвращает None; граница зоны: точка в
    /// 2 px правее — уже хит (зона — правые EDIT_ICON_ZONE px строки).
    #[test]
    fn edit_at_ignores_row_text_click() {
        let mut tree = group_tree();
        for node in tree.nodes.iter_mut().skip(1) {
            node.line = Some(0);
            node.value = Some(Ok(canvas_core::expr::Value::scalar(1.0)));
        }
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let body = [0.0, 0.0, 1200.0, 800.0];
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        let card_screen = [
            body[0] + BODY_PAD + card.rect[0],
            body[1] + BODY_PAD + card.rect[1],
            card.rect[2],
            card.rect[3],
        ];
        let row = &card.rows[0];
        let rect = row_edit_rect(card_screen, row.y, row.h, 1.0, 0.0);
        let cy = rect[1] + rect[3] / 2.0;
        // Текст строки: левый край + паддинг и точка сразу слева зоны —
        // обе мимо иконки.
        for px in [card_screen[0] + 10.0, rect[0] - 2.0] {
            assert_eq!(
                edit_at(&tree, &layout, 1.0, body, [px, cy], 0.0),
                None,
                "клик по тексту строки (x = {px}) — не иконка"
            );
        }
        // А в 1 px правее (внутри зоны) — хит по листу строки.
        assert_eq!(
            edit_at(&tree, &layout, 1.0, body, [rect[0] + 1.0, cy], 0.0),
            Some(row.leaf_idx)
        );
    }

    /// Ревизия владельца 2026-10-02 (дефект «полоса рода наезжает на
    /// карандаш»): зона иконки строки отступает от полосы-акцента на её
    /// ширину, hit-тест (edit_at) смещается с ней согласованно; при полосе
    /// на ЛЕВОМ краю (Rtl) зона не сдвигается.
    #[test]
    fn edit_icon_avoids_kind_strip() {
        let mut tree = group_tree();
        for (i, node) in tree.nodes.iter_mut().enumerate().skip(1) {
            node.line = Some(i - 1);
            node.value = Some(Ok(canvas_core::expr::Value::scalar(i as f64)));
        }
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        let body = [0.0, 0.0, 1200.0, 800.0];
        let scale = 1.0f32;
        let row = &card.rows[0];
        let card_screen = [
            body[0] + BODY_PAD + card.rect[0] * scale,
            body[1] + BODY_PAD + card.rect[1] * scale,
            card.rect[2] * scale,
            card.rect[3] * scale,
        ];
        for strip_inset in [0.0f32, 4.0, 6.0] {
            let rect = row_edit_rect(card_screen, row.y, row.h, scale, strip_inset);
            assert!(
                (rect[0] + rect[2] - (card_screen[0] + card_screen[2] - strip_inset)).abs() < 1e-3,
                "правый край зоны = правый край карточки минус полоса (inset {strip_inset})"
            );
            // Хит по центру сдвинутой зоны — по-прежнему лист строки.
            let point = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
            assert_eq!(
                edit_at(&tree, &layout, scale, body, point, strip_inset),
                Some(row.leaf_idx),
                "hit-тест смещается вместе с зоной (inset {strip_inset})"
            );
        }
        // Хелпер инсет-а: полоса справа (Ltr) — ширина полосы, слева (Rtl) — 0.
        assert_eq!(edit_icon_right_inset(1.0, true), 4.0);
        assert_eq!(edit_icon_right_inset(0.7, true), 2.8);
        assert_eq!(edit_icon_right_inset(0.4, true), 2.0, "пол 2 px");
        assert_eq!(edit_icon_right_inset(1.5, false), 0.0);
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
    /// потолком 1.5 (маленькое дерево укрупняется, большое — сжимается);
    /// FR-083: пол SCALE_MIN — глубже текст нечитаем.
    #[test]
    fn defense_fit_scale_up_to_ceiling() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let body = [0.0, 0.0, 1200.0, 800.0];
        // Маленькое дерево в большое тело: обычный вид — 1.0, защита —
        // укрупнение до потолка (×1.5, AC-6.2).
        assert_eq!(fit_scale(layout.bounds, body), 1.0);
        let d = defense_fit_scale(layout.bounds, body);
        assert!(d > 1.0, "защита укрупняет: {d}");
        assert!(d <= DEFENSE_SCALE_MAX + 1e-3);
        // Гигантское дерево в маленькое тело — сжатие, но не глубже пола
        // SCALE_MIN (FR-083: переполнение закрывает панорамирование).
        let tiny = defense_fit_scale(layout.bounds, [0.0, 0.0, 300.0, 200.0]);
        assert!((tiny - SCALE_MIN).abs() < 1e-3, "пол 0.7, получен {tiny}");
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

    /// FR-083 (а–г): лейаут в обоих направлениях на одном дереве — Ltr
    /// корень самый правый, листья слева; множества rect идентичны с
    /// точностью до зеркала X; кривые сидят на портах (сторона портов
    /// зеркалится); rows одинаковы.
    #[test]
    fn layout_direction_ltr_mirrors_tree() {
        let tree = sample_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let rtl = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let ltr = layout_tree(&tree, &vis, 0, LayoutDirection::Ltr);
        assert_eq!(rtl.nodes.len(), ltr.nodes.len());
        assert_eq!(rtl.curves.len(), ltr.curves.len());
        // (а) Ltr: корень вида — самый правый узел, листья — левые.
        fn by_idx(l: &TreeLayout, i: usize) -> &LaidNode {
            l.nodes
                .iter()
                .find(|n| n.idx == i)
                .unwrap_or_else(|| panic!("узел {i}"))
        }
        let max_x = ltr.nodes.iter().map(|n| n.rect[0]).fold(0.0f32, f32::max);
        assert!(
            (by_idx(&ltr, 0).rect[0] - max_x).abs() < 1e-3,
            "корень правее всех"
        );
        for leaf in [2usize, 3] {
            assert!(
                by_idx(&ltr, leaf).rect[0] < by_idx(&ltr, 0).rect[0],
                "лист {leaf} левее корня"
            );
        }
        // (б) Множества rect идентичны до зеркала X: rtl.x = bounds_w −
        // ltr.x − CARD_W (y/col/row/via неизменны); порядок Vec тот же.
        assert!((rtl.bounds[0] - ltr.bounds[0]).abs() < 1e-3);
        assert!((rtl.bounds[1] - ltr.bounds[1]).abs() < 1e-3);
        for (r, l) in rtl.nodes.iter().zip(&ltr.nodes) {
            assert_eq!(r.idx, l.idx, "порядок обхода не меняется");
            assert!((r.rect[0] + l.rect[0] + CARD_W - rtl.bounds[0]).abs() < 1e-3);
            assert!((r.rect[1] - l.rect[1]).abs() < 1e-3);
            assert_eq!(r.col, l.col);
            assert_eq!(r.via, l.via);
        }
        // (г) rows одинаковы в обоих направлениях.
        for (r, l) in rtl.nodes.iter().zip(&ltr.nodes) {
            assert!((r.row - l.row).abs() < 1e-3, "ряд узла {} сохранён", r.idx);
        }
        // (в) Кривые: первая/последняя точка на портах. Rtl — родитель
        // отдаёт правым портом, ребёнок принимает левым; Ltr — наоборот.
        let ports_match = |layout: &TreeLayout, rtl_ports: bool| {
            layout.curves.iter().all(|c| {
                let (start, end) = (c.points[0][0], c.points[3][0]);
                let start_ok = layout.nodes.iter().any(|n| {
                    if rtl_ports {
                        (n.rect[0] + CARD_W - start).abs() < 1e-3
                    } else {
                        (n.rect[0] - start).abs() < 1e-3
                    }
                });
                let end_ok = layout.nodes.iter().any(|n| {
                    if rtl_ports {
                        (n.rect[0] - end).abs() < 1e-3
                    } else {
                        (n.rect[0] + CARD_W - end).abs() < 1e-3
                    }
                });
                start_ok && end_ok
            })
        };
        assert!(ports_match(&rtl, true), "Rtl: правый порт → левый порт");
        assert!(ports_match(&ltr, false), "Ltr: левый порт → правый порт");
        // Кривые Ltr — точное зеркало Rtl (порядок Vec тот же), Y — без
        // изменений (вертикальный порядок детей сохранён).
        for (rc, lc) in rtl.curves.iter().zip(&ltr.curves) {
            assert_eq!(rc.to_leaf, lc.to_leaf);
            for (rp, lp) in rc.points.iter().zip(lc.points.iter()) {
                assert!((rp[0] + lp[0] - rtl.bounds[0]).abs() < 1e-3);
                assert!((rp[1] - lp[1]).abs() < 1e-3);
            }
        }
    }

    /// FR-083 (д): тумблер направления — левее defense-тумблера, без
    /// пересечения с ним, вертикально отцентрован по шапке; на узком
    /// окне прячется (пересекает мета-зону), на широком виден.
    #[test]
    fn direction_toggle_geometry_and_visibility() {
        let win = [100.0, 100.0, 1200.0, 800.0];
        let dir = direction_toggle_rect(win);
        let defense = defense_toggle_rect(win);
        // Левее defense-тумблера, зазор ~8 px, без пересечения.
        assert!(dir[0] + dir[2] <= defense[0], "тумблер левее defense");
        assert!((defense[0] - (dir[0] + dir[2]) - 8.0).abs() < 1e-3);
        assert!((dir[1] + dir[3] / 2.0 - (win[1] + HEADER_H / 2.0)).abs() < 1e-3);
        // Размер: ширина DIRECTION_TOGGLE_W, высота defense-тумблера.
        assert!((dir[2] - DIRECTION_TOGGLE_W).abs() < 1e-3);
        assert!((dir[3] - DEFENSE_TOGGLE_H).abs() < 1e-3);
        // Видимость: широкое окно — виден; узкое (цепочка кнопок правого
        // края заходит в мета-зону) — скрыт.
        assert!(direction_toggle_visible(win), "широкое окно — виден");
        assert!(
            !direction_toggle_visible([100.0, 100.0, 400.0, 300.0]),
            "узкое — скрыт"
        );
    }

    /// FR-083: конвертация настройки `explain_sources_left` ↔ направление;
    /// тумблер меняет направление и сбрасывает пан/драг.
    #[test]
    fn direction_settings_conversion_and_toggle_resets_pan() {
        assert_eq!(
            LayoutDirection::from_sources_left(true),
            LayoutDirection::Ltr
        );
        assert_eq!(
            LayoutDirection::from_sources_left(false),
            LayoutDirection::Rtl
        );
        assert!(LayoutDirection::Ltr.sources_left());
        assert!(!LayoutDirection::Rtl.sources_left());
        // Дефолт — Ltr (источники слева), pan — нулевой.
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(sample_tree()),
                base: None,
            }),
        );
        assert_eq!(st.direction, LayoutDirection::Ltr);
        assert_eq!(st.pan, [0.0; 2]);
        // Пан/драг установлены — тумблер сбрасывает их и меняет направление.
        st.pan = [-120.0, -40.0];
        st.pan_drag = Some(PanDrag {
            anchor: [10.0, 10.0],
            pan_start: [-120.0, -40.0],
        });
        st.toggle_direction();
        assert_eq!(st.direction, LayoutDirection::Rtl);
        assert_eq!(st.pan, [0.0; 2]);
        assert!(st.pan_drag.is_none());
        // Обратно — симметрично.
        st.toggle_direction();
        assert_eq!(st.direction, LayoutDirection::Ltr);
    }

    /// FR-083: смена вида (крошки/фокус/пик/защита/шаг) сбрасывает пан.
    #[test]
    fn pan_resets_on_view_and_defense_changes() {
        let mut st = ExplainState::loading(
            LineageNodeId::total("a"),
            0,
            ExplainBuild::Done(LineageOutcome {
                tree: Ok(sample_tree()),
                base: None,
            }),
        );
        st.poll();
        st.pan = [-90.0, -30.0];
        // Крошка к корню — пан сброшен.
        st.click_crumb(0);
        assert_eq!(st.pan, [0.0; 2]);
        // Фокус поддерева — пан сброшен.
        let vis = visibility(st.tree().unwrap(), 0, 3, &st.expanded);
        st.pan = [-90.0, -30.0];
        assert_eq!(st.click_node(1, &vis), NodeClick::Focused);
        assert_eq!(st.pan, [0.0; 2]);
        // Пик канвас→дерево — пан сброшен.
        st.pan = [-90.0, -30.0];
        assert!(st.pick_from_canvas(2, 3));
        assert_eq!(st.pan, [0.0; 2]);
        // Вход/выход защиты — пан сброшен.
        st.pan = [-90.0, -30.0];
        st.enter_defense(1);
        assert_eq!(st.pan, [0.0; 2]);
        st.pan = [-90.0, -30.0];
        st.defense_step();
        assert_eq!(st.pan, [0.0; 2]);
        st.pan = [-90.0, -30.0];
        st.defense_reveal_all();
        assert_eq!(st.pan, [0.0; 2]);
        st.pan = [-90.0, -30.0];
        st.exit_defense();
        assert_eq!(st.pan, [0.0; 2]);
    }

    // --- FR-084: карточка-таблица строк одного узла + центрирование ---------

    /// FR-084 (AC-2): три сестринских листа одного node_id — ОДНА карточка-
    /// таблица (строки по порядку детей), отдельных карточек у второго и
    /// третьего листа нет; высота = шапка + строки, ширина постоянна,
    /// rect[1] — ряд первого листа.
    #[test]
    fn table_card_groups_sibling_leaves_by_node_id() {
        let tree = group_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        // Карточек ровно две: родитель + одна таблица; листья 2/3 отдельных
        // карточек не получили (карточка — на месте первого листа).
        assert_eq!(layout.nodes.len(), 2);
        assert!(layout.nodes.iter().all(|n| n.idx != 2 && n.idx != 3));
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        assert_eq!(card.rows.len(), 3);
        // Строки в DFS-порядке (порядок children родителя).
        assert_eq!(
            card.rows.iter().map(|r| r.leaf_idx).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        // Локальная геометрия строки: шаг от шапки, port_y — центр строки,
        // по возрастанию и внутри карточки.
        for (i, r) in card.rows.iter().enumerate() {
            assert!((r.y - (TABLE_HEADER_H + i as f32 * TABLE_ROW_STEP)).abs() < 1e-3);
            assert!((r.h - TABLE_ROW_STEP).abs() < 1e-3);
            assert!((r.port_y - (r.y + r.h / 2.0)).abs() < 1e-3);
            assert!(r.port_y > 0.0 && r.port_y < card.rect[3]);
        }
        for pair in card.rows.windows(2) {
            assert!(pair[0].port_y < pair[1].port_y, "порты строк по росту");
        }
        // Высота таблицы = шапка + 3 строки; ширина постоянна (CARD_W —
        // Ltr/Rtl-зеркало и портовые тесты сохраняются).
        assert!((card.rect[3] - (TABLE_HEADER_H + 3.0 * TABLE_ROW_STEP)).abs() < 1e-3);
        assert!((card.rect[2] - CARD_W).abs() < 1e-3);
        // rect[1] — ряд ПЕРВОГО листа группы (ряд 0).
        assert!((card.rect[1] - (LAYOUT_PAD + 0.0 * ROW_H)).abs() < 1e-3);
    }

    /// FR-084: одинаковый node_id под РАЗНЫМИ родителями НЕ схлопывается —
    /// две карточки по одной строке; не-лист с детьми — обычная карточка
    /// (rows пуст, высота CARD_H); корень — тоже обычная карточка.
    #[test]
    fn same_node_id_under_different_parents_not_grouped() {
        // 0(Calc) → 1(Calc), 2(Leaf «src»); 1 → 3(Leaf «src»).
        let tree = LineageTree {
            root: LineageNodeId::total("m"),
            nodes: vec![
                LineageNode {
                    node_id: "m".into(),
                    line: None,
                    kind: LineageNodeKind::Calc,
                    value: None,
                    formula: Some("$a + $b".into()),
                    title: "M".into(),
                    label: None,
                    children: vec![
                        LineageChild {
                            child: 1,
                            via: None,
                        },
                        LineageChild {
                            child: 2,
                            via: None,
                        },
                    ],
                },
                LineageNode {
                    node_id: "k".into(),
                    line: None,
                    kind: LineageNodeKind::Calc,
                    value: None,
                    formula: Some("$src".into()),
                    title: "K".into(),
                    label: None,
                    children: vec![LineageChild {
                        child: 3,
                        via: None,
                    }],
                },
                leaf_node("src"),
                leaf_node("src"),
            ],
        };
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        // 4 карточки: корень, Calc 1 и ДВЕ отдельные таблицы «src».
        assert_eq!(layout.nodes.len(), 4);
        let t2 = layout.nodes.iter().find(|n| n.idx == 2).expect("таблица 2");
        let t3 = layout.nodes.iter().find(|n| n.idx == 3).expect("таблица 3");
        assert_eq!(t2.rows.len(), 1);
        assert_eq!(t3.rows.len(), 1);
        assert_eq!(t2.rows[0].leaf_idx, 2);
        assert_eq!(t3.rows[0].leaf_idx, 3);
        assert!((t2.rect[3] - (TABLE_HEADER_H + TABLE_ROW_STEP)).abs() < 1e-3);
        // Не-лист с детьми — обычная карточка (rows пуст).
        let k = layout.nodes.iter().find(|n| n.idx == 1).expect("calc 1");
        assert!(k.rows.is_empty());
        assert!((k.rect[3] - CARD_H).abs() < 1e-3);
        // Корень — обычная карточка.
        let root = layout.nodes.iter().find(|n| n.idx == 0).expect("корень");
        assert!(root.rows.is_empty());
    }

    /// FR-084: ряды родителя = среднее рядов листьев — группировка ряды не
    /// меняет (каждому листу назначается свой ряд по-прежнему; ряд карточки
    /// = ряд первого листа группы).
    #[test]
    fn parent_row_is_average_of_leaf_rows_with_grouping() {
        let tree = group_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        // Листья получили ряды 0, 1, 2 → родитель 1.0 (как без группировки).
        let parent = layout.nodes.iter().find(|n| n.idx == 0).expect("родитель");
        assert!((parent.row - 1.0).abs() < 1e-3);
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        assert!(
            (card.row - 0.0).abs() < 1e-3,
            "ряд карточки = ряд 1-го листа"
        );
        // sample_tree: одиночки-таблицы (по одной строке): c ряд 0 → b 0.0;
        // d ряд 1 → a (0 + 1) / 2 = 0.5.
        let st = sample_tree();
        let vis = visibility(&st, 0, 3, &empty);
        let layout = layout_tree(&st, &vis, 0, LayoutDirection::Rtl);
        let row_of =
            |l: &TreeLayout, i: usize| l.nodes.iter().find(|n| n.idx == i).expect("узел").row;
        assert!((row_of(&layout, 2) - 0.0).abs() < 1e-3);
        assert!((row_of(&layout, 3) - 1.0).abs() < 1e-3);
        assert!((row_of(&layout, 1) - 0.0).abs() < 1e-3);
        assert!((row_of(&layout, 0) - 0.5).abs() < 1e-3);
    }

    /// FR-084 (AC-3): кривых — по числу видимых детей (строк таблицы);
    /// конец кривой листа — порт ЕГО строки [x ребёнка, card_y + port_y];
    /// начало — правый порт родителя (у обычной карточки — как раньше).
    #[test]
    fn curves_end_at_table_row_ports() {
        let tree = group_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let card = layout.nodes.iter().find(|n| n.idx == 1).expect("таблица");
        let parent = layout.nodes.iter().find(|n| n.idx == 0).expect("родитель");
        assert_eq!(layout.curves.len(), 3, "по кривой на видимого ребёнка");
        // Порядок кривых = порядок строк (кривые идут по порядку детей).
        for (curve, r) in layout.curves.iter().zip(&card.rows) {
            assert!(curve.to_leaf, "кривая к листу помечена");
            // Конец — левый порт карточки, Y = порт строки.
            assert!((curve.points[3][0] - card.rect[0]).abs() < 1e-3);
            assert!(
                (curve.points[3][1] - (card.rect[1] + r.port_y)).abs() < 1e-3,
                "конец кривой = порт строки"
            );
            // Начало — правый порт родителя (Calc: прежняя середина).
            assert!((curve.points[0][0] - (parent.rect[0] + CARD_W)).abs() < 1e-3);
            assert!((curve.points[0][1] - (parent.rect[1] + CARD_H / 2.0)).abs() < 1e-3);
        }
    }

    /// FR-084: Ltr-зеркало для таблицы-карточки — rtl.x + ltr.x + CARD_W =
    /// bounds_w (ширина постоянна), Y/строки без изменений; кривые —
    /// точное зеркало Rtl по X (порты строк сидят на зеркальных X).
    #[test]
    fn ltr_mirror_keeps_table_card_geometry() {
        let tree = group_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let rtl = layout_tree(&tree, &vis, 0, LayoutDirection::Rtl);
        let ltr = layout_tree(&tree, &vis, 0, LayoutDirection::Ltr);
        assert_eq!(rtl.nodes.len(), ltr.nodes.len());
        assert_eq!(rtl.curves.len(), ltr.curves.len());
        let rc = rtl.nodes.iter().find(|n| n.idx == 1).expect("таблица rtl");
        let lc = ltr.nodes.iter().find(|n| n.idx == 1).expect("таблица ltr");
        assert!(
            (rc.rect[0] + lc.rect[0] + CARD_W - rtl.bounds[0]).abs() < 1e-3,
            "зеркало X для таблицы-карточки"
        );
        assert!((rc.rect[1] - lc.rect[1]).abs() < 1e-3);
        assert_eq!(rc.rows, lc.rows, "строки/порты зеркалятся без изменений");
        // Кривые Ltr — зеркало Rtl: сумма X = bounds_w, Y без изменений.
        for (rcurve, lcurve) in rtl.curves.iter().zip(&ltr.curves) {
            assert_eq!(rcurve.to_leaf, lcurve.to_leaf);
            for (rp, lp) in rcurve.points.iter().zip(lcurve.points.iter()) {
                assert!((rp[0] + lp[0] - rtl.bounds[0]).abs() < 1e-3);
                assert!((rp[1] - lp[1]).abs() < 1e-3);
            }
        }
    }

    /// FR-084: иконка строки — правый край строки (= правый край карточки),
    /// внутри карточки на любом масштабе; ширина текста: текст + зона
    /// иконки + паддинги ≤ CARD_W; пол 0.0 на вырожденной ширине.
    #[test]
    fn row_edit_rect_and_text_width_geometry() {
        let (ry, rh) = (TABLE_HEADER_H, TABLE_ROW_STEP);
        let card_l = [40.0, 30.0, 158.0, TABLE_HEADER_H + TABLE_ROW_STEP];
        for scale in [1.0f32, 0.7, 1.5] {
            // card_screen — УЖЕ экранный rect (локальный → экран, масштаб
            // от начала координат); row_y/row_h — локальные px строки.
            let card_screen = [
                card_l[0] * scale,
                card_l[1] * scale,
                card_l[2] * scale,
                card_l[3] * scale,
            ];
            let r = row_edit_rect(card_screen, ry, rh, scale, 0.0);
            // Правый край иконки = правый край карточки, левый — внутри.
            assert!((r[0] + r[2] - (card_screen[0] + card_screen[2])).abs() < 1e-3);
            assert!(r[0] >= card_screen[0] - 1e-3);
            // Вертикаль — строка целиком, внутри карточки.
            assert!((r[1] - (card_screen[1] + ry * scale)).abs() < 1e-3);
            assert!((r[3] - rh * scale).abs() < 1e-3);
            assert!(r[1] >= card_screen[1] - 1e-3);
            assert!(r[1] + r[3] <= card_screen[1] + card_screen[3] + 1e-3);
            assert!((r[2] - EDIT_ICON_ZONE * scale).abs() < 1e-3);
        }
        // Вырожденная карточка (экранные px, scale 1.0): x прижат к левому
        // краю (страж max).
        let tiny = row_edit_rect([40.0, 0.0, 5.0, 52.0], ry, rh, 1.0, 0.0);
        assert!((tiny[0] - 40.0).abs() < 1e-3, "x не левее карточки");
        assert!((tiny[2] - EDIT_ICON_ZONE).abs() < 1e-3);
        // Ширина текста: формула и неразрывность суммы с зоной иконки.
        for scale in [1.0f32, 0.7] {
            let w = table_row_text_width(CARD_W, scale);
            assert!((w - (CARD_W - 16.0 - EDIT_ICON_ZONE) * scale).abs() < 1e-3);
            assert!(w >= 0.0);
            assert!(
                w + EDIT_ICON_ZONE * scale + 16.0 * scale <= CARD_W + 1e-3,
                "текст + зона + паддинги ≤ CARD_W (scale {scale})"
            );
        }
        // Пол 0.0 на вырожденной ширине.
        assert_eq!(table_row_text_width(10.0, 1.0), 0.0);
    }

    /// FR-084 (AC-1): content_origin — (а) влезает по обеим осям →
    /// центрирование с равными отступами в доступной области (origin >
    /// BODY_PAD); (б) переполнение по обеим → origin = BODY_PAD + пан
    /// (пан — после клампа pan_clamp); (в) смешанный случай — X центр,
    /// Y прижат + пан.
    #[test]
    fn content_origin_centers_or_pins_content() {
        let body = [0.0, 0.0, 500.0, 400.0];
        let avail_w = body[2] - BODY_PAD * 2.0;
        let avail_h = body[3] - BODY_PAD * 2.0;
        // (а) Влезает по обеим осям: центрирование, равные отступы.
        let o = content_origin([0.0, 0.0], [100.0, 100.0], 1.0, body);
        assert!(o[0] > BODY_PAD && o[1] > BODY_PAD, "отступ больше поля");
        assert!(((o[0] - BODY_PAD) - (avail_w - 100.0) / 2.0).abs() < 1e-3);
        assert!(((o[1] - BODY_PAD) - (avail_h - 100.0) / 2.0).abs() < 1e-3);
        // Равные отступы слева/справа и сверху/снизу (в пределах avail).
        assert!(((o[0] - BODY_PAD) - (BODY_PAD + avail_w - (o[0] + 100.0))).abs() < 1e-3);
        assert!(((o[1] - BODY_PAD) - (BODY_PAD + avail_h - (o[1] + 100.0))).abs() < 1e-3);
        // (б) Переполнение по обеим осям: origin = BODY_PAD + пан.
        let pan = pan_clamp([-900.0, 50.0], [1000.0, 800.0], 1.0, body);
        let o = content_origin(pan, [1000.0, 800.0], 1.0, body);
        assert!((o[0] - (BODY_PAD + pan[0])).abs() < 1e-3);
        assert!((o[1] - (BODY_PAD + pan[1])).abs() < 1e-3);
        // Нулевой пан — прижат ровно к BODY_PAD (прежнее поведение).
        assert_eq!(
            content_origin([0.0, 0.0], [1000.0, 800.0], 1.0, body),
            [BODY_PAD, BODY_PAD]
        );
        // (в) Смешанный: X влезает (центр), Y переполнен (прижат + пан).
        let o = content_origin([0.0, -100.0], [100.0, 800.0], 1.0, body);
        assert!((o[0] - (BODY_PAD + (avail_w - 100.0) / 2.0)).abs() < 1e-3);
        assert!((o[1] - (BODY_PAD - 100.0)).abs() < 1e-3);
    }

    // --- FR-085: зум, base_scale, поле-тултип у якоря -------------------

    /// FR-085: zoom_adjust_pan — при равных масштабах пан не меняется.
    #[test]
    fn zoom_adjust_pan_identity_at_same_scale() {
        let body = [0.0, 0.0, 800.0, 600.0];
        let pan = [-40.0, -20.0];
        let out = zoom_adjust_pan(pan, [300.0, 200.0], body, [1600.0, 1200.0], 1.0, 1.0);
        assert!((out[0] - pan[0]).abs() < 1e-3 && (out[1] - pan[1]).abs() < 1e-3);
    }

    /// FR-085: зум к курсору — контентная точка под курсором сохраняется
    /// (после зума и клампа точка остаётся под курсором).
    #[test]
    fn zoom_adjust_pan_keeps_cursor_point() {
        let body = [0.0, 0.0, 800.0, 600.0];
        let bounds = [1600.0, 1200.0];
        let cursor = [500.0, 300.0];
        let (s_old, s_new) = (1.0, 1.5);
        let pan_old = pan_clamp([-100.0, -50.0], bounds, s_old, body);
        // Контентная точка под курсором ДО зума
        let origin_old = content_origin(pan_old, bounds, s_old, body);
        let c = [
            (cursor[0] - body[0] - origin_old[0]) / s_old,
            (cursor[1] - body[1] - origin_old[1]) / s_old,
        ];
        let raw = zoom_adjust_pan(pan_old, cursor, body, bounds, s_old, s_new);
        let pan_new = pan_clamp(raw, bounds, s_new, body);
        let origin_new = content_origin(pan_new, bounds, s_new, body);
        let screen = [
            body[0] + origin_new[0] + c[0] * s_new,
            body[1] + origin_new[1] + c[1] * s_new,
        ];
        assert!(
            (screen[0] - cursor[0]).abs() < 1e-2,
            "x: {} vs {}",
            screen[0],
            cursor[0]
        );
        assert!(
            (screen[1] - cursor[1]).abs() < 1e-2,
            "y: {} vs {}",
            screen[1],
            cursor[1]
        );
    }

    /// FR-085: base_scale — обычный режим fit, защита defense_fit.
    #[test]
    fn base_scale_picks_fit_or_defense() {
        let body = [0.0, 0.0, 800.0, 600.0];
        let bounds = [1600.0, 1200.0];
        assert!((base_scale(bounds, body, false) - fit_scale(bounds, body)).abs() < 1e-4);
        assert!((base_scale(bounds, body, true) - defense_fit_scale(bounds, body)).abs() < 1e-4);
    }

    /// FR-085: поле подмены — тултип у якоря: по умолчанию под якорем;
    /// у нижнего края — над якорем; X клампится в тело.
    #[test]
    fn field_rect_tooltip_near_anchor() {
        let body = [0.0, 0.0, 800.0, 600.0];
        // Под якорем (влезает)
        let anchor = [100.0, 100.0, 120.0, 60.0];
        let f = field_rect(body, anchor);
        assert!((f[1] - (anchor[1] + anchor[3] + 6.0)).abs() < 1e-3, "below");
        assert!((f[0] - anchor[0]).abs() < 1e-3, "x без клампа");
        // У нижнего края — флип над якорем
        let anchor_low = [100.0, 500.0, 120.0, 60.0];
        let f = field_rect(body, anchor_low);
        assert!((f[1] - (anchor_low[1] - 26.0 - 6.0)).abs() < 1e-3, "above");
        // Якорь у правого края — X клампится в тело
        let anchor_right = [700.0, 100.0, 120.0, 60.0];
        let f = field_rect(body, anchor_right);
        assert!(
            f[0] + f[2] <= body[0] + body[2] - BODY_PAD + 1e-3,
            "x clamp"
        );
    }

    /// FR-085: edit_anchor_rect — полоса строки таблицы; вся карточка
    /// для не-табличного узла.
    #[test]
    fn edit_anchor_rect_row_and_card() {
        let tree = group_tree();
        let empty = BTreeSet::new();
        let vis = visibility(&tree, 0, 3, &empty);
        let layout = layout_tree(&tree, &vis, 0, LayoutDirection::Ltr);
        let body = [0.0, 0.0, 900.0, 700.0];
        // Карточка-таблица листьев "src" — ищем её row-якорь
        let table = layout
            .nodes
            .iter()
            .find(|l| !l.rows.is_empty())
            .expect("group_tree даёт таблицу");
        let first_row_leaf = table.rows[0].leaf_idx;
        let origin = content_origin([0.0; 2], layout.bounds, 1.0, body);
        let anchor = edit_anchor_rect(&layout, first_row_leaf, 1.0, body, origin)
            .expect("строка таблицы найдена");
        assert!(
            (anchor[3] - TABLE_ROW_STEP).abs() < 1e-3,
            "высота якоря = строка"
        );
        assert!(anchor[1] > body[1], "якорь внутри тела");
        // Не-табличная карточка (корень) — якорь = карточка целиком
        // (порядок layout.nodes — пост-обход: ищем карточку по idx)
        let root_anchor = edit_anchor_rect(&layout, 0, 1.0, body, origin).expect("корень");
        let root_card = layout
            .nodes
            .iter()
            .find(|l| l.idx == 0)
            .expect("корень в лейауте");
        assert!(
            (root_anchor[3] - root_card.rect[3]).abs() < 1e-3,
            "высота карточки"
        );
        // Битый idx — None
        assert!(edit_anchor_rect(&layout, 9999, 1.0, body, origin).is_none());
    }

    /// FR-085: reset_pan сбрасывает и зум (вид сменился — масштаб к базе).
    #[test]
    fn reset_pan_resets_zoom() {
        let mut state = ExplainState::loading(
            canvas_core::LineageNodeId::total("r"),
            0,
            ExplainBuild::Native(std::sync::mpsc::channel().1),
        );
        state.zoom = 2.2;
        state.pan = [-100.0, -50.0];
        state.reset_pan();
        assert!((state.zoom - 1.0).abs() < 1e-6);
        assert_eq!(state.pan, [0.0; 2]);
    }

    /// Ревизия владельца 2026-10-02 (инверсия зума): колесо ВВЕРХ (Line +1)
    /// — зум IN, ровно +5 % за щелчок (CR-017); колесо вниз — −5 %. Прежний
    /// фактор `1.05^(−y·40)` давал 1.05^−40 ≈ 0.135 на колесо вверх
    /// (инверсия + щелчок бился в кламп SCALE_MIN/ZOOM_MAX).
    #[test]
    fn wheel_zoom_factor_line_notch_direction_and_step() {
        let up = wheel_zoom_factor(WheelInput::Line(1.0));
        let down = wheel_zoom_factor(WheelInput::Line(-1.0));
        assert!(
            up > 1.0,
            "колесо вверх должно зумить IN, получено {up} (инверсия)"
        );
        assert!(down < 1.0, "колесо вниз должно зумить OUT, получено {down}");
        assert!((up - 1.05).abs() < 1e-6, "шаг ровно +5 %, получено {up}");
        assert!(((1.0 / down) - 1.05).abs() < 1e-4, "шаг ровно −5 %");
        // Тилт колеса (x-составляющая) не участвует в вертикали: y = 0 —
        // фактор 1 (пан, не зум).
        assert!((wheel_zoom_factor(WheelInput::Line(0.0)) - 1.0).abs() < 1e-9);
    }

    /// Тачпад-канал (PixelDelta): знак совпадает с колесом (пиксели вверх —
    /// зум IN), фактор плавный (близко к 1 за 40 px, как у канваса).
    #[test]
    fn wheel_zoom_factor_pixel_smooth_and_signed() {
        let up = wheel_zoom_factor(WheelInput::Pixel(120.0));
        let down = wheel_zoom_factor(WheelInput::Pixel(-120.0));
        assert!(up > 1.0, "пиксели вверх — зум IN, получено {up}");
        assert!(down < 1.0, "пиксели вниз — зум OUT, получено {down}");
        assert!(
            (up - 1.0) < 0.5,
            "тачпад-фактор плавный (без прыжков клампа): {up}"
        );
    }
}
