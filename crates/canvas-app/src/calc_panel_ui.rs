//! FR-044 Р-4/Р-5/Р-8: панель «Как считается» в main stage — чистая модель,
//! раскладка и hit-тесты (паттерн `flowmap_ui.rs`: логика без I/O и без
//! зависимости от кадра; рендер и ввод — в `app.rs`).
//!
//! Состав (Р-4, проверен на прототипе prototype-mainstage-anatomy.html):
//! - «Переменные · входящие значения» — ВСЕ входящие value-рёбра приёмника
//!   ([`canvas_core::dataref::input_refs`]) в стабильном порядке слотов:
//!   квалифицированный адрес + текущее значение; unmapped — «не подставлено»
//!   (FR-045 R-3);
//! - «Расчёт · формулы» — строки расчёта приёмника
//!   ([`canvas_core::dataref::formula_displays`]) в порядке тела с
//!   рёбрами-операндами для подсветки формула⇄переменные⇄рёбра (Р-5);
//! - внешние входы (не из просматриваемого пучка) входят в «Переменные»
//!   (панель полная), их источники агрегируются в [`ExtSource`] для
//!   мини-карточек под истоком и счётчика «+N внешн.» (Р-8).
//!
//! FR-059 (волна 1 миграции кита, паттерн U5 — числа дословно): строки
//! обеих колонок — `kit::list_rows` + [`ScrollState`] (кит «список+
//! скролл»): прежний кап высоты панели остаётся ОГРАНИЧЕНИЕМ РАЗМЕРА
//! ([`PANEL_MAX_H_FRACTION`]), но срез строк «… ещё N» (Q2 v1) удалён —
//! переполнение честно прокручивается (бегунок `kit::scroll_bar` — цвет
//! на потребителе), ВСЕ строки доступны клику/подсветке. Индексы строк
//! ([`StageCalcFocus`]) — модельные, скролл-независимые (инвариант 3).
//!
//! Чистый Rust, без I/O; детерминизм — порядок `canvas.edges`/тела.

use std::collections::{BTreeSet, HashSet};

use canvas_core::dataref::{formula_displays, input_refs};
use canvas_core::flow::{FlowOutputs, LineOutputs, NamedOutputs};
use canvas_core::{Canvas, Edge};
use canvas_ui::kit::{list_rows, ScrollState};
use canvas_ui::layout::{stack, HAlign, VAlign};

/// Значение строки «Переменных»: значение / ошибка вычисления / unmapped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowValue {
    /// Значение по адресации ребра (строка/выход/узел истока).
    Ok(String),
    /// Источник вычислился с ошибкой (текст ошибки).
    Err(String),
    /// Значения нет (источник pending/удалён) — «не подставлено» (Р-3).
    Unmapped,
}

/// Строка группы «Переменные · входящие значения» (Р-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRow {
    /// Индекс ребра в `canvas.edges` (live — для подсветки веера).
    pub edge_index: usize,
    /// Исток ребра (для мини-карточки внешнего источника, Р-8).
    pub from_node: String,
    /// Позиционный слот (0-based); `None` — проливание в параметр.
    pub slot: Option<usize>,
    /// Имя параметра проливания; `None` — позиционный слот.
    pub param: Option<String>,
    /// Квалифицированный путь истока «Объект.Поле».
    pub path: String,
    /// Ребро входит в просматриваемый пучок (`false` — внешний вход, Р-8).
    pub in_bundle: bool,
    pub value: RowValue,
}

/// Строка группы «Расчёт · формулы» (Р-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaRow {
    /// Индекс строки тела приёмника (для связи с карточкой/тултипом).
    pub line: usize,
    /// Имя результата присваивания; пусто для строки-выражения.
    pub name: String,
    /// Текст формулы с путями операндов (`FormulaDisplay::display`).
    pub display: String,
    /// Индексы рёбер-операндов (первое упоминание, без дублей — Р-5).
    pub operand_edges: Vec<usize>,
}

/// Внешний источник приёмника (Р-8): агрегат внешних входов по истоку.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtSource {
    /// id ноды-истока.
    pub from_node: String,
    /// Заголовок карточки (первая строка текста / имя шаблона).
    pub title: String,
    /// Число внешних value-входов от этого источника.
    pub count: usize,
}

/// Модель панели «Как считается» — детерминирована канвасом и решениями
/// потока; индексы строк стабильны в пределах среза (инвариант 3).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CalcPanelModel {
    pub vars: Vec<VarRow>,
    pub formulas: Vec<FormulaRow>,
    /// Внешние источники в порядке первого упоминания (Р-8).
    pub ext_sources: Vec<ExtSource>,
    /// Суммарное число внешних входов (счётчик «+N внешн.» в заголовке).
    pub ext_count: usize,
}

/// Активные решения потока сцены (значения по адресации рёбер).
pub struct PanelValues<'a> {
    pub lines: &'a LineOutputs,
    pub named: &'a NamedOutputs,
    pub outputs: &'a FlowOutputs,
}

/// Построить модель панели для приёмника `receiver_id`. `bundle_edges` —
/// live-индексы рёбер просматриваемого пучка (внешние входы — Р-8).
pub fn build_model(
    canvas: &Canvas,
    receiver_id: &str,
    bundle_edges: &[usize],
    values: &PanelValues<'_>,
) -> CalcPanelModel {
    let bundle: HashSet<usize> = bundle_edges.iter().copied().collect();
    let mut vars = Vec::new();
    let mut ext_sources: Vec<ExtSource> = Vec::new();
    let mut ext_count = 0usize;
    for input in input_refs(canvas, receiver_id) {
        let in_bundle = bundle.contains(&input.edge_index);
        let edge = &canvas.edges[input.edge_index];
        let value = edge_value(edge, values);
        if !in_bundle {
            ext_count += 1;
            if let Some(ext) = ext_sources
                .iter_mut()
                .find(|ext| ext.from_node == edge.from_node)
            {
                ext.count += 1;
            } else {
                let title = canvas
                    .node(&edge.from_node)
                    .map(canvas_render::cards::title_for)
                    .unwrap_or_else(|| edge.from_node.clone());
                ext_sources.push(ExtSource {
                    from_node: edge.from_node.clone(),
                    title,
                    count: 1,
                });
            }
        }
        vars.push(VarRow {
            edge_index: input.edge_index,
            from_node: edge.from_node.clone(),
            slot: input.slot,
            param: input.param,
            path: input.r.path(),
            in_bundle,
            value,
        });
    }
    let formulas = formula_displays(canvas, receiver_id)
        .into_iter()
        .map(|row| FormulaRow {
            line: row.line,
            name: match canvas_core::expr::line_kind(&row.raw) {
                canvas_core::expr::NumiLineKind::Assignment { name } => name,
                _ => String::new(),
            },
            display: row.display,
            operand_edges: row.operand_edges,
        })
        .collect();
    CalcPanelModel {
        vars,
        formulas,
        ext_sources,
        ext_count,
    }
}

/// Значение ребра по адресации (зеркало `edge_source_value` потока:
/// `fromLine` → строка листа, `fromOutput` → именованный выход, иначе
/// узловой итог; ошибка вычисления — [`RowValue::Err`], отсутствие —
/// [`RowValue::Unmapped`]).
fn edge_value(edge: &Edge, values: &PanelValues<'_>) -> RowValue {
    if let Some(line) = edge.from_line {
        return match values.lines.get(&(edge.from_node.clone(), line)) {
            Some(value) => RowValue::Ok(value.to_string()),
            None => RowValue::Unmapped,
        };
    }
    if let Some(output) = edge.from_output.as_deref() {
        return match values
            .named
            .get(&(edge.from_node.clone(), output.to_owned()))
        {
            Some(value) => RowValue::Ok(value.to_string()),
            None => RowValue::Unmapped,
        };
    }
    match values.outputs.get(&edge.from_node) {
        Some(Ok(value)) => RowValue::Ok(value.to_string()),
        Some(Err(err)) => RowValue::Err(err.to_string()),
        None => RowValue::Unmapped,
    }
}

// --- Раскладка (screen px, координаты ОТНОСИТЕЛЬНЫЕ stage-rect) ----------

/// Высота строки панели.
pub const PANEL_ROW_H: f32 = 22.0;
/// Высота заголовка группы.
pub const PANEL_TITLE_H: f32 = 18.0;
/// Внутренний отступ панели.
pub const PANEL_PAD: f32 = 10.0;
/// Ширина колонки «Переменные» (прототип Р-4).
pub const VARS_COL_W: f32 = 360.0;
/// Минимальная ширина колонки «Расчёт».
pub const FORMULAS_COL_MIN_W: f32 = 240.0;
/// Отступ панели от нижней подсказки stage.
pub const PANEL_BOTTOM_GAP: f32 = 34.0;
/// Кап высоты панели (доля высоты stage) — выше поднимается зона пилюль.
pub const PANEL_MAX_H_FRACTION: f32 = 0.45;

/// Раскладка панели: нижняя зона stage, две колонки (переменные 360 px,
/// формулы — остальное); `trace_h = max(|vars|, |formulas|)` (Р-4).
/// Переполнение — прокрутка кита (FR-059): кап высоты
/// [`PANEL_MAX_H_FRACTION`] остаётся ограничением размера панели,
/// видимые строки — `kit::list_rows` (индексы — модельные), срез
/// «… ещё N» удалён (G5; прежние `vars_cut`/`formulas_cut`).
#[derive(Debug, Clone, PartialEq)]
pub struct CalcPanelLayout {
    /// Панель целиком `[x, y, w, h]` (относительно stage-rect).
    pub rect: [f32; 4],
    /// Заголовок группы «Переменные».
    pub vars_title: [f32; 4],
    /// Заголовок группы «Расчёт».
    pub formulas_title: [f32; 4],
    /// Окно списка переменных (вьюпорт скролла; сцена для бегунка/колеса).
    pub vars_area: [f32; 4],
    /// Окно списка формул (вьюпорт скролла).
    pub formulas_area: [f32; 4],
    /// Видимые строки переменных `(индекс в `model.vars`, rect)` —
    /// `kit::list_rows`; rect — прежний xywh строки.
    pub var_rows: Vec<(usize, [f32; 4])>,
    /// Видимые строки формул `(индекс в `model.formulas`, rect)`.
    pub formula_rows: Vec<(usize, [f32; 4])>,
    /// Верхняя кромка панели (screen px, rect-relative) — нижняя граница
    /// зоны клампа пилюль веера (Р-1: «между заголовком и панелью»).
    pub top: f32,
}

impl CalcPanelLayout {
    /// Индекс строки переменных под курсором (МОДЕЛЬНЫЙ — скролл-независимый).
    pub fn var_row_at(&self, cursor: [f32; 2]) -> Option<usize> {
        self.var_rows
            .iter()
            .find(|(_, r)| point_in(r, cursor))
            .map(|(i, _)| *i)
    }

    /// Индекс строки формул под курсором (модельный).
    pub fn formula_row_at(&self, cursor: [f32; 2]) -> Option<usize> {
        self.formula_rows
            .iter()
            .find(|(_, r)| point_in(r, cursor))
            .map(|(i, _)| *i)
    }

    /// Курсор внутри панели (клик глотается — панель жива).
    pub fn contains(&self, cursor: [f32; 2]) -> bool {
        point_in(&self.rect, cursor)
    }

    /// Курсор над окном списка переменных (скролл колесом — FR-059)?
    pub fn vars_area_at(&self, cursor: [f32; 2]) -> bool {
        point_in(&self.vars_area, cursor)
    }

    /// Курсор над окном списка формул?
    pub fn formulas_area_at(&self, cursor: [f32; 2]) -> bool {
        point_in(&self.formulas_area, cursor)
    }
}

fn point_in(rect: &[f32; 4], p: [f32; 2]) -> bool {
    p[0] >= rect[0] && p[0] <= rect[0] + rect[2] && p[1] >= rect[1] && p[1] <= rect[1] + rect[3]
}

/// Синхронизация скролла колонки с контентом/вьюпортом (идемпотентно,
/// resize-паттерн `docs_ui`) — детерминизм рендер/hit.
fn sync_scroll(scroll: &mut ScrollState, count: usize, viewport_h: f32) {
    scroll.content_h = count as f32 * PANEL_ROW_H;
    scroll.viewport_h = viewport_h;
    scroll.clamp();
}

/// Видимые строки колонки — `kit::list_rows` в xywh модели; частичные
/// строки у краёв окна обрезаются пересечением (клип-семантика — строка
/// не рисуется за пределами окна списка).
fn visible_rows(area: [f32; 4], scroll: &ScrollState, count: usize) -> Vec<(usize, [f32; 4])> {
    let window = canvas_ui::geometry::UiRect::new(area[0], area[1], area[2], area[3]);
    list_rows(window, scroll, PANEL_ROW_H, 0.0, count)
        .into_iter()
        .filter_map(|(index, rect)| {
            rect.intersection(&window)
                .map(|r| (index, [r.x, r.y, r.w, r.h]))
        })
        .collect()
}

/// Раскладка панели. `rect_w`/`rect_h` — размер stage (screen px);
/// `zone_top` — верх доступной зоны (низ веера, screen px, rect-relative).
/// `None` — панель не строится (нет ни переменных, ни формул).
pub fn layout(
    model: &CalcPanelModel,
    rect_w: f32,
    rect_h: f32,
    zone_top: f32,
    vars_scroll: &mut ScrollState,
    formulas_scroll: &mut ScrollState,
) -> Option<CalcPanelLayout> {
    if model.vars.is_empty() && model.formulas.is_empty() {
        return None;
    }
    let margin = 16.0;
    let width = (rect_w - margin * 2.0).max(VARS_COL_W + FORMULAS_COL_MIN_W);
    // FR-059: позиция панели — кит `stack` (левый-нижний слот: поля
    // по бокам, низ слота — над нижней подсказкой `PANEL_BOTTOM_GAP`;
    // прежние x = маржа, низ = `rect_h − PANEL_BOTTOM_GAP` дословно)
    let available = (rect_h - PANEL_BOTTOM_GAP - zone_top).max(0.0);
    let max_h = (rect_h * PANEL_MAX_H_FRACTION)
        .min(available)
        .max(PANEL_TITLE_H + PANEL_PAD * 2.0 + PANEL_ROW_H);
    let inner_h = (max_h - PANEL_TITLE_H - PANEL_PAD * 2.0).max(PANEL_ROW_H);
    let max_rows = ((inner_h / PANEL_ROW_H).floor() as usize).max(1);
    // Окно списка каждой колонки — до `max_rows` строк (ограничение
    // размера панели); строки сверх — прокрутка кита (перебор не срезается)
    let vars_vis = model.vars.len().min(max_rows);
    let formulas_vis = model.formulas.len().min(max_rows);
    let rows = vars_vis.max(formulas_vis).max(1);
    let height = PANEL_TITLE_H + PANEL_PAD * 2.0 + rows as f32 * PANEL_ROW_H;
    let slot = canvas_ui::geometry::UiRect::new(
        margin,
        0.0,
        (rect_w - margin * 2.0).max(0.0),
        (rect_h - PANEL_BOTTOM_GAP).max(0.0),
    );
    let panel = stack(
        slot,
        canvas_ui::geometry::UiVec2::new(width, height),
        HAlign::Start,
        VAlign::End,
    );
    let x = panel.x;
    let y = panel.y;
    // Колонки: переменные 360 px (или меньше на узком stage), формулы —
    // остальное; заголовок группы — над своей колонкой
    let vars_w = VARS_COL_W.min(width * 0.6);
    let formulas_x = x + vars_w + 16.0;
    let formulas_w = (width - vars_w - 16.0).max(120.0);
    let rows_y = y + PANEL_PAD + PANEL_TITLE_H;
    let vars_area = [
        x + PANEL_PAD,
        rows_y,
        (vars_w - PANEL_PAD).max(0.0),
        vars_vis as f32 * PANEL_ROW_H,
    ];
    let formulas_area = [
        formulas_x,
        rows_y,
        formulas_w,
        formulas_vis as f32 * PANEL_ROW_H,
    ];
    // Скроллы колонок синхронизируются с контентом/вьюпортом
    // (идемпотентно — детерминизм рендер/hit), видимые строки —
    // kit::list_rows (модельные индексы, частичные — клип по окну)
    sync_scroll(vars_scroll, model.vars.len(), vars_area[3]);
    sync_scroll(formulas_scroll, model.formulas.len(), formulas_area[3]);
    let var_rows = visible_rows(vars_area, vars_scroll, model.vars.len());
    let formula_rows = visible_rows(formulas_area, formulas_scroll, model.formulas.len());
    Some(CalcPanelLayout {
        rect: [x, y, width, height],
        vars_title: [
            x + PANEL_PAD,
            y + PANEL_PAD - 2.0,
            vars_w - PANEL_PAD,
            PANEL_TITLE_H,
        ],
        formulas_title: [formulas_x, y + PANEL_PAD - 2.0, formulas_w, PANEL_TITLE_H],
        vars_area,
        formulas_area,
        var_rows,
        formula_rows,
        top: y,
    })
}

// --- Подсветка зависимостей (Р-5) ----------------------------------------

/// Подсветка «формула ⇄ переменные ⇄ рёбра» (Р-5): множество строк панели
/// (индексация: `0..vars.len()` — переменные, `vars.len()+i` — формулы)
/// и live-индексы рёбер-операндов. Runtime-состояние App: не пишется в
/// undo и `.canvas` (инвариант 8).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StageCalcFocus {
    pub rows: BTreeSet<usize>,
    pub edges: BTreeSet<usize>,
}

impl StageCalcFocus {
    /// Клик по строке формулы (Р-5): сама формула + её переменные +
    /// рёбра-операнды (инвариант 6 — ровно множество операндов).
    pub fn for_formula(model: &CalcPanelModel, formula_i: usize) -> Self {
        let mut focus = Self::default();
        let var_base = model.vars.len();
        focus.rows.insert(var_base + formula_i);
        let Some(formula) = model.formulas.get(formula_i) else {
            return focus;
        };
        for (j, var) in model.vars.iter().enumerate() {
            if formula.operand_edges.contains(&var.edge_index) {
                focus.rows.insert(j);
                focus.edges.insert(var.edge_index);
            }
        }
        focus
    }

    /// Клик по переменной (Р-5, обратная навигация): переменная + её ребро
    /// + все формулы, где она участвует (инвариант 7).
    pub fn for_var(model: &CalcPanelModel, var_i: usize) -> Self {
        let mut focus = Self::default();
        let Some(var) = model.vars.get(var_i) else {
            return focus;
        };
        focus.rows.insert(var_i);
        focus.edges.insert(var.edge_index);
        for (i, formula) in model.formulas.iter().enumerate() {
            if formula.operand_edges.contains(&var.edge_index) {
                focus.rows.insert(model.vars.len() + i);
            }
        }
        focus
    }

    /// Клик по ребру/пилюле веера (Р-5): выделение ребра + синхронная
    /// подсветка связанной строки расчёта и её переменных.
    pub fn for_edge(model: &CalcPanelModel, edge_index: usize) -> Self {
        let mut focus = Self::default();
        focus.edges.insert(edge_index);
        for (j, var) in model.vars.iter().enumerate() {
            if var.edge_index == edge_index {
                focus.rows.insert(j);
            }
        }
        for (i, formula) in model.formulas.iter().enumerate() {
            if formula.operand_edges.contains(&edge_index) {
                focus.rows.insert(model.vars.len() + i);
            }
        }
        focus
    }
}

// --- Зона пилюль веера: переполнение (Q2, v2) -----------------------------

/// Высота двухстрочной пилюли (адрес + значение) — базовый режим.
pub const PILL_H_TWO_LINE: f32 = 34.0;
/// Высота однострочной пилюли (адрес · значение) — компактный режим Q2.
pub const PILL_H_ONE_LINE: f32 = 20.0;

/// Режим зоны пилюль при переполнении стопки (Q2: «сокращение текста →
/// тултип, или скролл зоны веера»; базовый кламп с уплотнением шага —
/// [`canvas_core::bundles::stage_fan_label_layout`] — остаётся первым
/// эшелоном внутри [`canvas_core::bundles::stage_fan_label_layout`]).
///
/// Эшелоны (детерминированы, чистая функция):
/// 1. **Full** — двухстрочные пилюли умещаются в зону (обычный случай);
/// 2. **Compact** — не умещаются: однострочный текст «адрес · значение»
///    (полный путь сохраняется — резать его нельзя, инвариант 5; высота
///    пилюли вдвое меньше — стопка помещается);
/// 3. **Scroll** — не помогает и компакт: окно стопки с прокруткой
///    (колесо над зоной веера, клики по индикаторам «↑ ещё N»/«ещё N ↓»).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillZoneMode {
    /// Двухстрочные пилюли — все видны (базовый режим Р-1).
    Full,
    /// Однострочные пилюли — все видны (первый эшелон Q2).
    Compact,
    /// Окно стопки с прокруткой (второй эшелон Q2): `first` — индекс
    /// первой видимой пилюли (порядок стопки — по вертикали середин),
    /// `visible` — число видимых, `above`/`below` — скрытые сверху/снизу
    /// (счётчики индикаторов).
    Scroll {
        first: usize,
        visible: usize,
        above: usize,
        below: usize,
    },
}

/// Решение режима зоны пилюль (Q2): `count` — число пилюль (рёбер веера),
/// `zone_h` — высота зоны клампа, `scroll` — текущее смещение окна
/// (в пилюлях; значимо только в режиме Scroll). Шаг стопки —
/// [`FAN_LABEL_GAP_PX`] (тот же, что в базовой раскладке).
pub fn pill_zone_mode(count: usize, zone_h: f32, scroll: usize) -> PillZoneMode {
    if count == 0 || zone_h <= 0.0 {
        return PillZoneMode::Full;
    }
    let gap = canvas_core::bundles::FAN_LABEL_GAP_PX;
    let fits = |h: f32| count as f32 * h + gap * (count - 1) as f32 <= zone_h;
    if fits(PILL_H_TWO_LINE) {
        return PillZoneMode::Full;
    }
    if fits(PILL_H_ONE_LINE) {
        return PillZoneMode::Compact;
    }
    // Scroll: сколько однострочных пилюль умещается (с зазорами;
    // последняя — без зазора), окно с клампом смещения.
    let step = PILL_H_ONE_LINE + gap;
    let visible_max = (((zone_h + gap) / step).floor() as usize).max(1).min(count);
    let max_first = count - visible_max;
    let first = scroll.min(max_first);
    PillZoneMode::Scroll {
        first,
        visible: visible_max,
        above: first,
        below: count - first - visible_max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::expr::Value;
    use canvas_core::flow::FlowKind;
    use canvas_core::{Edge, Node};

    fn text_node(id: &str, text: &str, x: f32) -> Node {
        Node::text(id, text, x, 0.0)
    }

    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str, output: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some(output.to_owned());
        canvas.edges.push(edge);
    }

    fn values(lines: LineOutputs, named: NamedOutputs) -> PanelValues<'static> {
        // Узловой итог в этих тестах не читается — outputs пуст.
        let lines: &'static LineOutputs = Box::leak(Box::new(lines));
        let named: &'static NamedOutputs = Box::leak(Box::new(named));
        let outputs: &'static FlowOutputs = Box::leak(Box::new(FlowOutputs::new()));
        PanelValues {
            lines,
            named,
            outputs,
        }
    }

    /// Модель Р-4: переменные — все входы приёмника (пучок + внешние),
    /// формулы — строки присваиваний; внешние агрегируются (Р-8).
    #[test]
    fn model_groups_and_external_sources() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(text_node("src", "Заявки\nusers = 10\nconv = 0.2", 0.0));
        canvas
            .nodes
            .push(text_node("prices", "Цены\nusd = 90", 1.0));
        canvas.nodes.push(text_node(
            "dst",
            "Отчёт\nx = Заявки.users * 2\ny = Заявки.conv + Цены.usd",
            2.0,
        ));
        value_edge(&mut canvas, "e1", "src", "dst", "users");
        value_edge(&mut canvas, "e2", "src", "dst", "conv");
        value_edge(&mut canvas, "e3", "prices", "dst", "usd");
        let mut named = NamedOutputs::new();
        named.insert(("src".to_owned(), "users".to_owned()), Value::scalar(10.0));
        named.insert(("src".to_owned(), "conv".to_owned()), Value::scalar(0.2));
        named.insert(("prices".to_owned(), "usd".to_owned()), Value::scalar(90.0));
        // Пучок — только e1/e2; e3 — внешний вход (Р-8)
        let model = build_model(&canvas, "dst", &[0, 1], &values(LineOutputs::new(), named));
        assert_eq!(model.vars.len(), 3);
        assert!(model.vars[0].in_bundle);
        assert!(model.vars[1].in_bundle);
        assert!(!model.vars[2].in_bundle, "e3 — внешний вход");
        assert_eq!(model.vars[0].path, "Заявки.users");
        assert_eq!(
            model.vars[0].value,
            RowValue::Ok("10".to_owned()),
            "значение по именованному выходу"
        );
        assert_eq!(model.ext_count, 1);
        assert_eq!(model.ext_sources.len(), 1);
        assert_eq!(model.ext_sources[0].from_node, "prices");
        assert_eq!(
            model.ext_sources[0].title,
            canvas_render::cards::title_for(canvas.node("prices").expect("нода"))
        );
        // Формулы: 2 присваивания; операнды — по qualified-ключам
        assert_eq!(model.formulas.len(), 2);
        assert_eq!(model.formulas[0].name, "x");
        assert_eq!(model.formulas[0].operand_edges, vec![0]);
        assert_eq!(model.formulas[1].operand_edges, vec![1, 2]);
    }

    /// Unmapped-вход (источник без значения адресованного выхода) —
    /// `RowValue::Unmapped` («не подставлено», Р-3/Р-4).
    #[test]
    fn model_unmapped_when_output_missing() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(text_node("src", "Заявки\nusers = 10", 0.0));
        canvas.nodes.push(text_node("dst", "x = Заявки.conv", 1.0));
        value_edge(&mut canvas, "e1", "src", "dst", "conv");
        let model = build_model(
            &canvas,
            "dst",
            &[0],
            &values(LineOutputs::new(), NamedOutputs::new()),
        );
        assert_eq!(model.vars[0].value, RowValue::Unmapped);
    }

    /// Раскладка Р-4: нижняя зона, две колонки, окно списка с прокруткой
    /// (FR-059: срез «… ещё N» удалён — модельные индексы скролл-независимы);
    /// hit-тест строк; детерминизм.
    #[test]
    fn layout_columns_cap_and_hits() {
        let model = CalcPanelModel {
            vars: (0..14)
                .map(|i| VarRow {
                    edge_index: i,
                    from_node: format!("s{i}"),
                    slot: Some(i),
                    param: None,
                    path: format!("S{i}.v"),
                    in_bundle: true,
                    value: RowValue::Ok("1".to_owned()),
                })
                .collect(),
            formulas: (0..3)
                .map(|i| FormulaRow {
                    line: i,
                    name: format!("f{i}"),
                    display: format!("f{i} = S0.v"),
                    operand_edges: vec![0],
                })
                .collect(),
            ..CalcPanelModel::default()
        };
        let rect_w = 900.0;
        let rect_h = 600.0;
        let mut vars_scroll = ScrollState::default();
        let mut formulas_scroll = ScrollState::default();
        let lay = layout(
            &model,
            rect_w,
            rect_h,
            120.0,
            &mut vars_scroll,
            &mut formulas_scroll,
        )
        .expect("панель есть");
        // Нижняя кромка над подсказкой
        assert!((lay.rect[1] + lay.rect[3] - (rect_h - PANEL_BOTTOM_GAP)).abs() < 0.01);
        // Кап: max_h = 600*0.45 = 270 → окно 10 строк (геометрия прежняя)
        assert_eq!(lay.var_rows.len(), 10);
        assert_eq!(lay.vars_area[3], 10.0 * PANEL_ROW_H);
        // FR-059: перебор прокручивается — индексы модельные, скролл активен
        assert!(vars_scroll.needs_scroll());
        assert_eq!(vars_scroll.max_offset(), 4.0 * PANEL_ROW_H);
        assert_eq!(lay.var_rows[0].0, 0);
        assert_eq!(lay.var_rows[9].0, 9);
        assert_eq!(lay.formula_rows.len(), 3);
        assert!(!formulas_scroll.needs_scroll(), "3 строки в окне 10");
        // Прокрутка переменных: последние строки становятся видимыми
        vars_scroll.scroll_by(4.0 * PANEL_ROW_H);
        vars_scroll.clamp();
        let lay = layout(
            &model,
            rect_w,
            rect_h,
            120.0,
            &mut vars_scroll,
            &mut formulas_scroll,
        )
        .expect("панель есть");
        assert_eq!(lay.var_rows.last().expect("строки есть").0, 13);
        // Колонки: формулы правее колонки переменных
        let f = lay.formula_rows[0].1;
        let v = lay.var_rows[0].1;
        assert!(f[0] > v[0] + v[2], "колонки не пересекаются");
        // Хит-тест: внутри строки и мимо (индекс — модельный; после
        // прокрутки первая видимая строка — № 4)
        let mid = [v[0] + v[2] / 2.0, v[1] + PANEL_ROW_H / 2.0];
        assert_eq!(lay.var_row_at(mid), Some(4));
        assert_eq!(lay.formula_row_at(mid), None);
        let fmid = [f[0] + 10.0, f[1] + 5.0];
        assert_eq!(lay.formula_row_at(fmid), Some(0));
        assert!(!lay.contains([rect_w, rect_h]), "мимо панели");
        // Зоны списков — для скролла колесом
        assert!(lay.vars_area_at([lay.vars_area[0] + 5.0, lay.vars_area[1] + 5.0]));
        assert!(lay.formulas_area_at([lay.formulas_area[0] + 5.0, lay.formulas_area[1] + 5.0]));
        // Детерминизм: повторный вызов — идентичная раскладка
        assert_eq!(
            layout(
                &model,
                rect_w,
                rect_h,
                120.0,
                &mut vars_scroll,
                &mut formulas_scroll
            ),
            Some(lay)
        );
    }

    /// Пустая модель — панели нет.
    #[test]
    fn layout_none_when_empty() {
        let model = CalcPanelModel::default();
        let mut vars_scroll = ScrollState::default();
        let mut formulas_scroll = ScrollState::default();
        assert!(layout(
            &model,
            900.0,
            600.0,
            120.0,
            &mut vars_scroll,
            &mut formulas_scroll
        )
        .is_none());
    }

    /// Р-5/инвариант 6: клик по формуле — ровно её переменные и операнды.
    #[test]
    fn focus_for_formula_highlights_operands() {
        let model = demo_model();
        // f1 = a·e0 + b·e2 (e2 — внешний): строки 0 и 2, рёбра 0 и 2
        let focus = StageCalcFocus::for_formula(&model, 1);
        assert_eq!(focus.rows, BTreeSet::from([0, 2, model.vars.len() + 1]));
        assert_eq!(focus.edges, BTreeSet::from([0, 2]));
    }

    /// Р-5/инвариант 7: клик по переменной — она сама, её ребро и все
    /// формулы, где она участвует.
    #[test]
    fn focus_for_var_reverse_navigation() {
        let model = demo_model();
        // var 0 (e0) участвует в f0 и f1
        let focus = StageCalcFocus::for_var(&model, 0);
        assert_eq!(
            focus.rows,
            BTreeSet::from([0, model.vars.len(), model.vars.len() + 1])
        );
        assert_eq!(focus.edges, BTreeSet::from([0]));
    }

    /// Р-5: клик по ребру/пилюле — синхронная подсветка строки и переменных.
    #[test]
    fn focus_for_edge_sync() {
        let model = demo_model();
        let focus = StageCalcFocus::for_edge(&model, 2);
        assert_eq!(focus.edges, BTreeSet::from([2]));
        assert_eq!(focus.rows, BTreeSet::from([2, model.vars.len() + 1]));
    }

    fn demo_model() -> CalcPanelModel {
        CalcPanelModel {
            vars: (0..3)
                .map(|i| VarRow {
                    edge_index: i,
                    from_node: format!("s{i}"),
                    slot: Some(i),
                    param: None,
                    path: format!("S{i}.v"),
                    in_bundle: i != 2,
                    value: RowValue::Ok("1".to_owned()),
                })
                .collect(),
            formulas: vec![
                FormulaRow {
                    line: 1,
                    name: "f0".to_owned(),
                    display: "f0 = S0.v".to_owned(),
                    operand_edges: vec![0],
                },
                FormulaRow {
                    line: 2,
                    name: "f1".to_owned(),
                    display: "f1 = S0.v + S2.v".to_owned(),
                    operand_edges: vec![0, 2],
                },
            ],
            ..CalcPanelModel::default()
        }
    }

    /// Q2: режим зоны пилюль — Full на обычных высотах, Compact при
    /// переполнении двухстрочными, Scroll при переполнении и компактными.
    #[test]
    fn pill_zone_mode_tiers() {
        let gap = canvas_core::bundles::FAN_LABEL_GAP_PX;
        // ×6 (демо-пучок): две строки (34) + зазоры — 6·34 + 5·8 = 244
        let zone6 = 6.0 * PILL_H_TWO_LINE + 5.0 * gap;
        assert_eq!(pill_zone_mode(6, zone6, 0), PillZoneMode::Full);
        // Те же ×6 в зоне ниже: двухстрочные не влезают, однострочные
        // (6·20 + 5·8 = 160) — влезают → Compact
        let zone_c = 6.0 * PILL_H_ONE_LINE + 5.0 * gap;
        assert_eq!(pill_zone_mode(6, zone_c, 0), PillZoneMode::Compact);
        assert!(pill_zone_mode(6, zone_c - 1.0, 0) != PillZoneMode::Compact);
        // ×12 в малой зоне: только окно с прокруткой
        let small = 3.0 * PILL_H_ONE_LINE + 2.0 * gap;
        match pill_zone_mode(12, small, 0) {
            PillZoneMode::Scroll {
                first,
                visible,
                above,
                below,
            } => {
                assert_eq!(first, 0);
                assert_eq!(visible, 3);
                assert_eq!(above, 0);
                assert_eq!(below, 9);
            }
            other => panic!("ожидался Scroll: {other:?}"),
        }
        // Смещение окна с клампом: scroll 5 → first 5, below 4
        match pill_zone_mode(12, small, 5) {
            PillZoneMode::Scroll { first, below, .. } => {
                assert_eq!(first, 5);
                assert_eq!(below, 4);
            }
            other => panic!("ожидался Scroll: {other:?}"),
        }
        // Кламп: scroll 99 → first = 12 − 3 = 9, ниже ни одной
        match pill_zone_mode(12, small, 99) {
            PillZoneMode::Scroll { first, below, .. } => {
                assert_eq!(first, 9);
                assert_eq!(below, 0);
            }
            other => panic!("ожидался Scroll: {other:?}"),
        }
        // Вырожденные входы — без паники
        assert_eq!(pill_zone_mode(0, 100.0, 0), PillZoneMode::Full);
        assert_eq!(pill_zone_mode(3, 0.0, 0), PillZoneMode::Full);
        // Одна пилюля в слишком низкой зоне — окно из одной
        match pill_zone_mode(1, 15.0, 0) {
            PillZoneMode::Scroll { visible, .. } => assert_eq!(visible, 1),
            other => panic!("ожидался Scroll: {other:?}"),
        }
    }
}
