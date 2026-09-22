//! FR-050 Н9-4 (этап E): «Карта проливаний» — оверлей всех проливаний
//! канваса (источник → параметр → значение), клик по строке — переход к
//! истоку (полёт камеры + подсветка, Н9-3). Чистая модель и раскладка
//! (screen-space, логические px — образец [`crate::whatif_ui`]):
//! тестируется юнит-тестами, рендер и ввод — приложение (`app.rs`).
//!
//! Реализована поверх машинерии подсветки PRD-0007/FR-048 (focus-набор) —
//! без дублирования: переход использует ту же подсветку, что «Показать
//! источник» из контекст-меню параметра (Н9-3).

use canvas_core::flow::AutoRow;
use canvas_scene::SpillView;
use std::collections::HashMap;

/// Ширина панели (логические px) — читаемая строка «путь → параметр ·
/// значение» при типичной длине qualified-пути.
pub const PANEL_W: f32 = 340.0;
/// Маржа панели от правого края окна.
pub const PANEL_MARGIN: f32 = 16.0;
/// Отступ панели от верхнего края — ниже угловых кнопок (тема/?/⚙).
pub const PANEL_TOP: f32 = 64.0;
/// Высота заголовка (заголовок + кнопка «✕»).
pub const HEADER_H: f32 = 36.0;
/// Высота строки проливания.
pub const ROW_H: f32 = 26.0;
/// Строк, видимых без прокрутки (кап): длинные списки честно
/// обрезаются строкой «… ещё N» (v1 без скролла — постановка Н9-4
/// не требует; кардинальное решение — вместе с прокруткой explain-панели).
pub const VISIBLE_CAP: usize = 12;
/// Высота строки «… ещё N» / пустого состояния.
pub const FOOTER_H: f32 = 30.0;

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

/// FR-050 Н9-4: раскладка панели карты — xywh прямоугольники (панель,
/// кнопка «✕», строки, строка «… ещё N»). Высота — по числу строк до
/// VISIBLE_CAP (перебор — «… ещё N»); пустой канвас — строка-подсказка.
#[derive(Debug, Clone, Default)]
pub struct FlowMapLayout {
    /// Панель xywh.
    pub panel: [f32; 4],
    /// Кнопка «✕» (закрыть) xywh.
    pub close: [f32; 4],
    /// Видимые строки xywh (индекс = строка в `flow_map_rows`).
    pub rows: Vec<[f32; 4]>,
    /// Строка «… ещё N» (xywh, число скрытых) — None в пределах капа.
    pub more: Option<([f32; 4], usize)>,
}

/// Раскладка панели по вьюпорту и числу проливаний.
pub fn flow_map_layout(viewport: [f32; 2], rows: usize) -> FlowMapLayout {
    let [vw, vh] = viewport;
    let x = (vw - PANEL_W - PANEL_MARGIN).max(PANEL_MARGIN);
    let visible = rows.min(VISIBLE_CAP);
    let overflow = rows.saturating_sub(visible);
    let list_h = if rows == 0 {
        FOOTER_H
    } else {
        visible as f32 * ROW_H + if overflow > 0 { FOOTER_H } else { 0.0 }
    };
    // Панель не выезжает за нижний край (малые окна): min с доступной
    // высотой; строки сверх — обрезаются тем же «… ещё N»
    let available = (vh - PANEL_TOP - PANEL_MARGIN).max(HEADER_H + FOOTER_H);
    let h = (HEADER_H + list_h).min(available);
    let panel = [x, PANEL_TOP, PANEL_W, h];
    let close = [x + PANEL_W - 28.0, PANEL_TOP + 6.0, 22.0, 22.0];
    let mut layout = FlowMapLayout {
        panel,
        close,
        rows: Vec::with_capacity(visible),
        more: None,
    };
    // Строки, влезающие в фактическую высоту панели
    let mut fitted = 0usize;
    while fitted < visible {
        let top = PANEL_TOP + HEADER_H + fitted as f32 * ROW_H;
        if top + ROW_H > PANEL_TOP + h + 1e-3 {
            break;
        }
        layout.rows.push([x + 8.0, top, PANEL_W - 16.0, ROW_H]);
        fitted += 1;
    }
    let hidden = rows.saturating_sub(fitted);
    if hidden > 0 {
        let top = PANEL_TOP + HEADER_H + fitted as f32 * ROW_H;
        let rect = [x + 8.0, top, PANEL_W - 16.0, FOOTER_H];
        // Прямоугольник «ещё» может выйти за низ панели при тесном окне —
        // клампим в панель, строка рисуется по факту
        let clamped_bottom = (rect[1] + rect[3]).min(PANEL_TOP + h);
        layout.more = Some((
            [rect[0], rect[1], rect[2], clamped_bottom - rect[1]],
            hidden,
        ));
    }
    layout
}

/// Hit-тест строки карты: индекс в `flow_map_rows` под курсором
/// (логические px); None — мимо строк.
pub fn flow_map_row_at(layout: &FlowMapLayout, cursor: [f32; 2]) -> Option<usize> {
    layout.rows.iter().position(|rect| point_in(rect, cursor))
}

/// Курсор над кнопкой «✕»?
pub fn flow_map_close_at(layout: &FlowMapLayout, cursor: [f32; 2]) -> bool {
    point_in(&layout.close, cursor)
}

fn point_in(rect: &[f32; 4], p: [f32; 2]) -> bool {
    p[0] >= rect[0] && p[0] <= rect[0] + rect[2] && p[1] >= rect[1] && p[1] <= rect[1] + rect[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Н9-4: пустой канвас — панель минимальной высоты с одной строкой-
    /// подсказкой, ни строк, ни «ещё».
    #[test]
    fn layout_empty() {
        let lay = flow_map_layout([1280.0, 800.0], 0);
        assert_eq!(lay.rows.len(), 0);
        assert!(lay.more.is_none());
        assert_eq!(lay.panel[2], PANEL_W);
        assert_eq!(lay.panel[3], HEADER_H + FOOTER_H);
        // Панель у правого края с маржой
        assert_eq!(lay.panel[0], 1280.0 - PANEL_W - PANEL_MARGIN);
        assert_eq!(lay.panel[1], PANEL_TOP);
    }

    /// Н9-4: строки в пределах капа — все видимые, «ещё» нет; строки
    /// укладываются вертикально с шагом ROW_H.
    #[test]
    fn layout_rows_within_cap() {
        let lay = flow_map_layout([1280.0, 800.0], 5);
        assert_eq!(lay.rows.len(), 5);
        assert!(lay.more.is_none());
        assert_eq!(lay.rows[0][1], PANEL_TOP + HEADER_H);
        assert_eq!(lay.rows[1][1], PANEL_TOP + HEADER_H + ROW_H);
        assert_eq!(lay.panel[3], HEADER_H + 5.0 * ROW_H);
    }

    /// Н9-4: перебор капа — видимых 12, «… ещё N» считает скрытые.
    #[test]
    fn layout_overflow_counts_hidden() {
        let lay = flow_map_layout([1280.0, 800.0], 20);
        assert_eq!(lay.rows.len(), VISIBLE_CAP);
        let (rect, hidden) = lay.more.expect("строка «ещё»");
        assert_eq!(hidden, 20 - VISIBLE_CAP);
        assert_eq!(rect[1], PANEL_TOP + HEADER_H + VISIBLE_CAP as f32 * ROW_H);
    }

    /// Н9-4: тесное окно — панель клампится к доступной высоте, строки
    /// сверх фактической высоты уходят в «… ещё N» (не рисуются за краем).
    #[test]
    fn layout_clamps_to_small_viewport() {
        let lay = flow_map_layout([1280.0, 300.0], 20);
        assert!(lay.panel[3] <= 300.0 - PANEL_TOP - PANEL_MARGIN + 1e-3);
        assert!(lay.panel[1] + lay.panel[3] <= 300.0 + 1e-3);
        let hidden = lay.more.expect("скрытые есть").1;
        assert!(hidden > 0);
        assert!(lay.rows.len() < 20);
        // Строки не вылезают за панель
        for rect in &lay.rows {
            assert!(rect[1] + rect[3] <= lay.panel[1] + lay.panel[3] + 1e-3);
        }
    }

    /// Н9-4: hit-тест строк и кнопки закрытия.
    #[test]
    fn hit_tests() {
        let lay = flow_map_layout([1280.0, 800.0], 3);
        // Первая строка
        let first = lay.rows[0];
        let center = [first[0] + 10.0, first[1] + first[3] / 2.0];
        assert_eq!(flow_map_row_at(&lay, center), Some(0));
        // Вторая
        let second = lay.rows[1];
        assert_eq!(
            flow_map_row_at(&lay, [second[0] + 10.0, second[1] + 5.0]),
            Some(1)
        );
        // Заголовок — мимо строк
        assert_eq!(
            flow_map_row_at(&lay, [lay.panel[0] + 20.0, PANEL_TOP + 10.0]),
            None
        );
        // Кнопка «✕»
        assert!(flow_map_close_at(
            &lay,
            [lay.close[0] + 5.0, lay.close[1] + 5.0]
        ));
        assert!(!flow_map_close_at(
            &lay,
            [lay.panel[0] + 20.0, PANEL_TOP + 12.0]
        ));
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
