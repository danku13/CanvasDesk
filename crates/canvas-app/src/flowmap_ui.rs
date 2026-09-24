//! FR-050 Н9-4 (этап E): «Карта проливаний» — оверлей всех проливаний
//! канваса (источник → параметр → значение), клик по строке — переход к
//! истоку (полёт камеры + подсветка, Н9-3). Чистая модель и раскладка
//! (screen-space, логические px — образец [`crate::whatif_ui`]):
//! тестируется юнит-тестами, рендер и ввод — приложение (`app.rs`).
//!
//! Реализована поверх машинерии подсветки PRD-0007/FR-048 (focus-набор) —
//! без дублирования: переход использует ту же подсветку, что «Показать
//! источник» из контекст-меню параметра (Н9-3).
//!
//! FR-059 (волна 1 миграции кита, паттерн U5 — числа дословно): панель —
//! `kit::stack` (правый-верхний слот с полями, прежние формулы x/y);
//! строки — `kit::list_rows` + [`ScrollState`] (кит «список+скролл»):
//! прежний кап `VISIBLE_CAP` со строкой «… ещё N» и `break`-клампом
//! подгонки (G5-класс дефекта — молчаливое исчезновение строк) заменён
//! окном списка высотой до 12 строк ([`LIST_MAX_ROWS`] — прежняя
//! геометрия панели) со скроллбаром кита: ВСЕ строки доступны,
//! ни одна не исчезает. Бегунок — `kit::scroll_bar` (цвет — слот на
//! потребителе). `kit::card` НЕ применяется: его фикс-пад `SPACING_LG`
//! (12) даёт хедер/тело с отступами, несовместимыми с прежней геометрией
//! (строки вплотную к шапке, боковые поля 8 — дословно); раскладка
//! собрана из `stack`+`list_rows` — компоненты кита без визуального
//! скачка (правило U5 сильнее перечня «замена» из постановки).

use canvas_core::flow::AutoRow;
use canvas_scene::SpillView;
use canvas_ui::geometry::UiRect;
use canvas_ui::kit::{self, ScrollState};
use canvas_ui::layout::{stack, HAlign, VAlign};
use std::collections::HashMap;

/// Ширина панели (логические px) — читаемая строка «путь → параметр ·
/// значение» при типичной длине qualified-пути.
pub const PANEL_W: f32 = 340.0;
/// Маржа панели от правого края окна.
pub const PANEL_MARGIN: f32 = 16.0;
/// Отступ панели от верхнего края — ниже угловых кнопок (тема/?/⚙).
/// Верх панели (правый-верхний слот). 64 + web-инсет: угловые кнопки
/// (⚙/тема/«?») на web опущены под DOM-тулбар до y=84 — панель стартует
/// ниже их низа, иначе заголовок «Проливания» уезжает под кнопки
/// (wasm-аудит 2026-09-25).
pub const PANEL_TOP: f32 = 64.0 + crate::ui::WEB_TOOLBAR_INSET;
/// Высота заголовка (заголовок + кнопка «✕»).
pub const HEADER_H: f32 = 36.0;
/// Высота строки проливания.
pub const ROW_H: f32 = 26.0;
/// Высота окна списка: до 12 строк ([`LIST_MAX_ROWS`]) — прежняя
/// геометрия панели; строки сверх — доступны скроллом (FR-059: замена
/// капа `VISIBLE_CAP` = 12 со строкой «… ещё N» кит-скроллом, G5).
pub const LIST_MAX_ROWS: usize = 12;
/// Высота строки пустого состояния (пустой канвас — подсказка).
pub const FOOTER_H: f32 = 30.0;
/// Боковые поля строк списка (прежние +8/−16 от краёв панели).
pub const LIST_PAD_X: f32 = 8.0;

/// FR-050 Н9-4: строка карты — одно проливание (value-ребро):
/// `toParam`-проливание (`param: Some`) или позиционный вход-авто-строка
/// (`param: None`, слот `slot`).
#[derive(Debug, Clone, PartialEq)]
pub struct FlowMapRow {
    /// id ребра-источника истины (переход по клику).
    pub edge_id: String,
    /// id ноды-приёмника.
    pub node_id: String,
    /// Имя параметра приёмника (None — позиционный вход).
    pub param: Option<String>,
    /// Квалифицированный путь источника «Объект.Поле».
    pub path: String,
    /// Пролитое значение (None — unmapped «не подставлено»).
    pub value: Option<String>,
    /// Позиционный слот 0-based (для `param: None`).
    pub slot: usize,
}

/// FR-050 Н9-4: строки карты проливаний из готовых результатов пересчёта
/// (`param_spills` + `auto_rows` сцены) — рёбра адресуются по уникальности
/// инварианта Н4 («один вход на параметр»: победитель — последнее ребро по
/// `canvas.edges`). Сортировка по пути, затем параметру — детерминизм
/// (HashMap → выходной порядок фиксирован).
pub fn flow_map_rows(
    canvas: &canvas_core::Canvas,
    param_spills: &HashMap<String, Vec<SpillView>>,
    auto_rows: &HashMap<String, Vec<AutoRow>>,
) -> Vec<FlowMapRow> {
    let mut out: Vec<FlowMapRow> = Vec::new();
    for (node_id, views) in param_spills {
        for view in views {
            // Победитель — последнее ребро в параметр (как у диалога Н4)
            let edge_id = canvas_core::flow::occupying_param_edges(canvas, node_id, &view.param)
                .last()
                .map(|edge| edge.id.clone());
            let Some(edge_id) = edge_id else {
                // Кэш протух относительно рёбер (мутация между кадрами) —
                // строка неадресуема, в карту не входит
                continue;
            };
            out.push(FlowMapRow {
                edge_id,
                node_id: node_id.clone(),
                param: Some(view.param.clone()),
                path: view.path.clone(),
                value: view.value.clone(),
                slot: 0,
            });
        }
    }
    for (node_id, rows) in auto_rows {
        for row in rows {
            out.push(FlowMapRow {
                edge_id: row.edge_id.clone(),
                node_id: node_id.clone(),
                param: None,
                path: row.path.clone(),
                value: row.value.as_ref().map(|v| v.to_string()),
                slot: row.slot,
            });
        }
    }
    out.sort_by(|a, b| (&a.path, &a.param).cmp(&(&b.path, &b.param)));
    out
}

/// FR-050 Н9-4: раскладка панели карты (FR-059: кит `stack` + `list_rows`):
/// панель, кнопка «✕», окно списка, видимые строки. Высота окна — до
/// [`LIST_MAX_ROWS`] строк (перебор — прокрутка кита, строка «… ещё N»
/// удалена); пустой канвас — строка-подсказка высотой [`FOOTER_H`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowMapLayout {
    /// Панель.
    pub panel: UiRect,
    /// Кнопка «✕» (закрыть).
    pub close: UiRect,
    /// Окно списка строк (между шапкой и низом панели) — вьюпорт скролла.
    pub list: UiRect,
    /// Видимые строки `(индекс в `flow_map_rows`, rect)` — `kit::list_rows`.
    pub rows: Vec<(usize, UiRect)>,
}

/// Раскладка панели по вьюпорту, числу проливаний и состоянию скролла.
/// FR-059: `scroll` синхронизируется с контентом/вьюпортом здесь
/// (resize-паттерн `docs_ui` — идемпотентно, детерминизм рендер/hit).
pub fn flow_map_layout(
    viewport: [f32; 2],
    rows_count: usize,
    scroll: &mut ScrollState,
) -> FlowMapLayout {
    let [vw, vh] = viewport;
    // Панель: правый-верхний слот с полями — прежние формулы x/y дословно
    // (кит `stack`: End/Start; вырожденные окна — прежний кламп x ≥ маржи)
    let available = (vh - PANEL_TOP - PANEL_MARGIN).max(HEADER_H + FOOTER_H);
    let content_h = rows_count as f32 * ROW_H;
    let list_h = if rows_count == 0 {
        FOOTER_H
    } else {
        content_h
            .min(LIST_MAX_ROWS as f32 * ROW_H)
            .min(available - HEADER_H)
            .max(0.0)
    };
    let h = (HEADER_H + list_h).min(available);
    let slot = UiRect::new(
        PANEL_MARGIN,
        PANEL_TOP,
        (vw - PANEL_MARGIN * 2.0).max(0.0),
        (vh - PANEL_TOP - PANEL_MARGIN).max(0.0),
    );
    let panel = stack(
        slot,
        canvas_ui::geometry::UiVec2::new(PANEL_W, h),
        HAlign::End,
        VAlign::Start,
    );
    let close = UiRect::new(panel.right() - 28.0, panel.y + 6.0, 22.0, 22.0);
    // Окно списка: прежние боковые поля (+8/−16), шапка — вплотную
    let list = UiRect::new(
        panel.x + LIST_PAD_X,
        panel.y + HEADER_H,
        (PANEL_W - LIST_PAD_X * 2.0).max(0.0),
        (panel.h - HEADER_H).max(0.0),
    );
    // Синхронизация скролла с контентом/вьюпортом (идемпотентно) + строки
    scroll.content_h = content_h;
    scroll.viewport_h = list.h;
    scroll.clamp();
    // FR-059: частичные строки у краёв окна (тачпад-скролл) — обрезаются
    // по окну списка (пересечение; семантика клипа — строка не наезжает
    // на шапку/низ панели, band-клип полосы — весь вьюпорт)
    let rows = kit::list_rows(list, scroll, ROW_H, 0.0, rows_count)
        .into_iter()
        .filter_map(|(index, rect)| rect.intersection(&list).map(|r| (index, r)))
        .collect();
    FlowMapLayout {
        panel,
        close,
        list,
        rows,
    }
}

/// Hit-тест строки карты: индекс в `flow_map_rows` под курсором
/// (логические px); None — мимо строк. FR-059: индекс — из видимых строк
/// `kit::list_rows` (модельный индекс — скролл-независимый).
pub fn flow_map_row_at(layout: &FlowMapLayout, cursor: [f32; 2]) -> Option<usize> {
    let p = canvas_ui::geometry::UiPoint::new(cursor[0], cursor[1]);
    layout
        .rows
        .iter()
        .find(|(_, rect)| rect.contains(p))
        .map(|(index, _)| *index)
}

/// Курсор над кнопкой «✕»?
pub fn flow_map_close_at(layout: &FlowMapLayout, cursor: [f32; 2]) -> bool {
    layout
        .close
        .contains(canvas_ui::geometry::UiPoint::new(cursor[0], cursor[1]))
}

/// Курсор над окном списка (скролл колесом — FR-059)?
pub fn flow_map_list_at(layout: &FlowMapLayout, cursor: [f32; 2]) -> bool {
    layout
        .list
        .contains(canvas_ui::geometry::UiPoint::new(cursor[0], cursor[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Н9-4: пустой канвас — панель минимальной высоты с одной строкой-
    /// подсказкой, ни строк.
    #[test]
    fn layout_empty() {
        let mut scroll = ScrollState::default();
        let lay = flow_map_layout([1280.0, 800.0], 0, &mut scroll);
        assert_eq!(lay.rows.len(), 0);
        assert_eq!(lay.panel.w, PANEL_W);
        assert_eq!(lay.panel.h, HEADER_H + FOOTER_H);
        // Панель у правого края с маржой (кит stack: End/Start)
        assert_eq!(lay.panel.x, 1280.0 - PANEL_W - PANEL_MARGIN);
        assert_eq!(lay.panel.y, PANEL_TOP);
    }

    /// Н9-4: строки в пределах окна списка — все видимые; укладываются
    /// вертикально с шагом ROW_H (кит list_rows, зазор 0 — прежняя стопка).
    #[test]
    fn layout_rows_within_cap() {
        let mut scroll = ScrollState::default();
        let lay = flow_map_layout([1280.0, 800.0], 5, &mut scroll);
        assert_eq!(lay.rows.len(), 5);
        assert_eq!(lay.rows[0].0, 0);
        assert_eq!(lay.rows[0].1.y, PANEL_TOP + HEADER_H);
        assert_eq!(lay.rows[1].1.y, PANEL_TOP + HEADER_H + ROW_H);
        assert_eq!(lay.panel.h, HEADER_H + 5.0 * ROW_H);
        // Прежние боковые поля строк (+8/−16)
        assert_eq!(lay.rows[0].1.x, lay.panel.x + LIST_PAD_X);
        assert_eq!(lay.rows[0].1.w, PANEL_W - LIST_PAD_X * 2.0);
    }

    /// FR-059: перебор окна списка — видимых 12 (геометрия панели прежняя),
    /// остальные доступны скроллом («… ещё N» удалено); сдвиг offset
    /// прокручивает строки, индексы — модельные.
    #[test]
    fn layout_overflow_scrolls_with_kit_list() {
        let mut scroll = ScrollState::default();
        let lay = flow_map_layout([1280.0, 800.0], 20, &mut scroll);
        assert_eq!(lay.rows.len(), LIST_MAX_ROWS);
        assert_eq!(lay.panel.h, HEADER_H + LIST_MAX_ROWS as f32 * ROW_H);
        // Скролл: контент 20·26, вьюпорт 12·26 — прокрутка есть
        assert!(scroll.needs_scroll());
        assert!((scroll.max_offset() - 8.0 * ROW_H).abs() < 0.01);
        // Прокрутка на 8 строк — последняя строка видна (индекс 19)
        scroll.scroll_by(8.0 * ROW_H);
        scroll.clamp();
        let lay = flow_map_layout([1280.0, 800.0], 20, &mut scroll);
        assert_eq!(lay.rows.last().expect("строки есть").0, 19);
        // Строки не вылезают за окно списка
        for (_, rect) in &lay.rows {
            assert!(rect.bottom() <= lay.list.bottom() + 0.01);
        }
    }

    /// Н9-4: тесное окно — панель клампится к доступной высоте, видимых
    /// строк меньше (скролл открывает остальные; за краем не рисуются).
    #[test]
    fn layout_clamps_to_small_viewport() {
        let mut scroll = ScrollState::default();
        let lay = flow_map_layout([1280.0, 300.0], 20, &mut scroll);
        assert!(lay.panel.h <= 300.0 - PANEL_TOP - PANEL_MARGIN + 1e-3);
        assert!(lay.panel.bottom() <= 300.0 + 1e-3);
        assert!(lay.rows.len() < 20);
        assert!(scroll.needs_scroll());
        // Строки не вылезают за панель
        for (_, rect) in &lay.rows {
            assert!(rect.bottom() <= lay.panel.bottom() + 1e-3);
        }
    }

    /// Н9-4: hit-тест строк и кнопки закрытия (индексы — модельные).
    #[test]
    fn hit_tests() {
        let mut scroll = ScrollState::default();
        let lay = flow_map_layout([1280.0, 800.0], 3, &mut scroll);
        // Первая строка
        let first = lay.rows[0].1;
        let center = [first.x + 10.0, first.y + first.h / 2.0];
        assert_eq!(flow_map_row_at(&lay, center), Some(0));
        // Вторая
        let second = lay.rows[1].1;
        assert_eq!(
            flow_map_row_at(&lay, [second.x + 10.0, second.y + 5.0]),
            Some(1)
        );
        // Заголовок — мимо строк
        assert_eq!(
            flow_map_row_at(&lay, [lay.panel.x + 20.0, PANEL_TOP + 10.0]),
            None
        );
        // Кнопка «✕»
        assert!(flow_map_close_at(
            &lay,
            [lay.close.x + 5.0, lay.close.y + 5.0]
        ));
        assert!(!flow_map_close_at(
            &lay,
            [lay.panel.x + 20.0, PANEL_TOP + 12.0]
        ));
        // Окно списка — для скролла колесом (FR-059)
        assert!(flow_map_list_at(&lay, [lay.list.x + 5.0, lay.list.y + 5.0]));
    }

    /// Н9-4: сбор строк — toParam-проливания (параметр из SpillView,
    /// ребро-победитель) и авто-строки (позиционные, слот), сортировка
    /// по пути — детерминизм независимо от порядка HashMap.
    #[test]
    fn rows_collect_spills_and_autorows() {
        let mut canvas = canvas_core::Canvas::default();
        canvas.nodes.push(canvas_core::Node::text(
            "traffic",
            "Трафик\npeak_rps = 1389 rps",
            0.0,
            0.0,
        ));
        canvas.nodes.push(canvas_core::Node::text(
            "gw",
            "rps = 50 rps\nитог := $rps × 2",
            400.0,
            0.0,
        ));
        canvas.nodes.push(canvas_core::Node::text(
            "obs",
            "итог := $in + 1",
            800.0,
            0.0,
        ));
        let mut edge = canvas_core::Edge::new("e1", "traffic", None, "gw", None);
        edge.set_flow_kind(canvas_core::FlowKind::Value);
        edge.from_line = Some(1);
        edge.to_param = Some("rps".to_owned());
        canvas.add_edge(edge);
        let mut positional = canvas_core::Edge::new("e2", "traffic", None, "obs", None);
        positional.set_flow_kind(canvas_core::FlowKind::Value);
        positional.from_line = Some(1);
        canvas.add_edge(positional);

        let mut param_spills: HashMap<String, Vec<SpillView>> = HashMap::new();
        param_spills.insert(
            "gw".to_owned(),
            vec![SpillView {
                param: "rps".to_owned(),
                line: Some(0),
                from_label: "Трафик".to_owned(),
                from_output: Some("peak_rps".to_owned()),
                value: Some("1389 rps".to_owned()),
                path: "Трафик.peak_rps".to_owned(),
                local: Some("50 rps".to_owned()),
            }],
        );
        let mut auto: HashMap<String, Vec<AutoRow>> = HashMap::new();
        auto.insert(
            "obs".to_owned(),
            vec![AutoRow {
                node_id: "obs".to_owned(),
                edge_id: "e2".to_owned(),
                slot: 0,
                path: "Трафик.peak_rps".to_owned(),
                field: "peak_rps".to_owned(),
                value: None,
            }],
        );

        let rows = flow_map_rows(&canvas, &param_spills, &auto);
        // Путь одинаковый — параметр сортируется первым (Some < None по
        // порядку cmp для Option: None меньше! — проверим фактический порядок)
        assert_eq!(rows.len(), 2, "проливание + авто-строка");
        // Обе строки адресуют свои рёбра
        assert!(rows
            .iter()
            .any(|r| r.edge_id == "e1" && r.param.as_deref() == Some("rps")));
        assert!(rows
            .iter()
            .any(|r| r.edge_id == "e2" && r.param.is_none() && r.slot == 0));
        // Сортировка по пути стабильна
        assert_eq!(rows[0].path, "Трафик.peak_rps");
        assert_eq!(rows[1].path, "Трафик.peak_rps");
        // Протухший spill (ребро удалено) — неадресуен, в карту не входит
        let mut stale = param_spills.clone();
        stale.insert(
            "ghost".to_owned(),
            vec![SpillView {
                param: "x".to_owned(),
                line: None,
                from_label: String::new(),
                from_output: None,
                value: None,
                path: "Ghost.x".to_owned(),
                local: None,
            }],
        );
        let rows = flow_map_rows(&canvas, &stale, &auto);
        assert_eq!(rows.len(), 2, "протухшая запись отфильтрована");
    }
}
