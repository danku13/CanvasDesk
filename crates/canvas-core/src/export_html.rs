//! FR-076: экспорт канваса в самодостаточный HTML — «артефакт защиты»
//! (закрытие GAP-01: асинхронный показ модели без живого окна приложения).
//!
//! Перенесено из Archify практика «portable by default» (карта переноса T1,
//! `docs/dev-researches/archify-transfer-analysis.md` §3.1): результат —
//! один офлайн-файл, который открывается в любом браузере и пересылается
//! без установки приложения.
//!
//! ## Контракт
//!
//! * [`export_html`] — чистая функция `(canvas, solutions, comparison,
//!   options) -> String`: без I/O, без мутаций, wasm-совместима (canvas-core
//!   не зависит от ОС/GPU — AGENTS.md).
//! * [`scenario_comparison_for_export`] — композиция what-if ядра
//!   (`scenarios_from_canvas` → `freeze_scenario` → `compare_scenarios`)
//!   для таблицы сравнения сценариев в артефакте.
//! * Детерминизм: тот же канвас и решения дают побитово тот же HTML —
//!   без меток времени и transient-состояний (канонический экспорт,
//!   как у Archify: «exports are canonical and free of temporary viewer
//!   state»). Прогон каждого сценария — свежий (не pinned-снимок UI):
//!   артефакт отражает текущую модель.
//! * Экранирование: весь пользовательский текст (строки нод, заголовки,
//!   подписи рёбер, значения) проходит [`escape_html`] — артефакт нельзя
//!   превратить в разметку (XSS-гигиена офлайн-файла).
//! * Значения: построчные выходы — справа от строки (конвенция Numi-листа),
//!   узловой итог — футером карточки; подпись на value-ребре — по адресу
//!   (`fromLine` → построчный, `fromOutput` → именованный, иначе узловой
//!   итог; control-ребро значения не несёт — как `stage_edge_value_text`,
//!   `app/stage.rs:1706`).
//!
//! ## Границы MVP (FR-076 §Требуемые изменения)
//!
//! Внутри: SVG-снимок (группы-подложки, карточки с заголовком/строками,
//! рёбра Безье со стрелками), значения, таблица what-if, тема светлая/тёмная,
//! зум кнопками — весь CSS/JS инлайн (офлайн). Вне: интерактивный редактор,
//! deep links внутри артефакта, PNG/WebM/share-карточки, дельта-сравнение —
//! задокументированы как v2+ (волны 2–3 карты переноса).

use crate::flow::{FlowKind, FlowSolutions};
use crate::model::{Canvas, Edge, EdgeLineStyle, Node, NodeKind, Side};
use crate::whatif::{self, FrozenScenario, ScenarioComparison};
use std::fmt::Write as _;

/// Опции экспорта артефакта.
#[derive(Debug, Clone)]
pub struct ExportHtmlOptions {
    /// Заголовок артефакта (шапка + `<title>`): имя канваса у вызывающего.
    pub title: String,
    /// Тема по умолчанию (переключается кнопкой в самом артефакте).
    pub dark: bool,
}

impl Default for ExportHtmlOptions {
    fn default() -> Self {
        Self {
            title: "CanvasDesk".to_owned(),
            dark: true,
        }
    }
}

/// Таблица сравнения what-if сценариев для артефакта: None — сценариев нет
/// (или все дали цикл — честное отсутствие таблицы вместо пустой).
pub fn scenario_comparison_for_export(
    canvas: &Canvas,
    base: &FlowSolutions,
) -> Option<ScenarioComparison> {
    let scenarios = whatif::scenarios_from_canvas(canvas);
    if scenarios.is_empty() {
        return None;
    }
    // Свежий прогон каждого сценария (детерминизм: тот же канвас → тот же
    // снимок, инвариант 2 FR-050). Цикличный сценарий пропускается молча —
    // сравнение с ним всё равно не определено.
    let snapshots: Vec<FrozenScenario> = scenarios
        .iter()
        .filter_map(|scenario| whatif::freeze_scenario(canvas, scenario).ok())
        .collect();
    if snapshots.is_empty() {
        return None;
    }
    // Ключи строк: union валидных подмен всех сценариев (как в
    // `App::whatif_compare_table`, overlays.rs:66) — узловые итоги по дифу
    // добавит ядро.
    let mut line_keys: Vec<(String, usize)> = scenarios
        .iter()
        .flat_map(|scenario| whatif::active_line_exprs(canvas, scenario).into_keys())
        .collect();
    line_keys.sort();
    line_keys.dedup();
    Some(whatif::compare_scenarios(base, &snapshots, &line_keys))
}

/// Сборка самодостаточного HTML-артефакта (см. контракт модуля).
pub fn export_html(
    canvas: &Canvas,
    solutions: &FlowSolutions,
    comparison: Option<&ScenarioComparison>,
    options: &ExportHtmlOptions,
) -> String {
    let title = escape_html(&options.title);
    let mut out = String::with_capacity(16 * 1024);
    out.push_str("<!doctype html>\n<html lang=\"ru\" data-theme=\"");
    out.push_str(if options.dark { "dark" } else { "light" });
    out.push_str("\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let _ = writeln!(out, "<title>{title} — CanvasDesk</title>");
    out.push_str("<style>\n");
    out.push_str(CSS);
    out.push_str("</style>\n</head>\n<body>\n");
    // Шапка: заголовок + счётчики + кнопки (тема/зум) — без метки времени
    // (детерминизм). Счётчики — по модели, не по решениям.
    let groups = canvas
        .nodes
        .iter()
        .filter(|node| node.kind() == NodeKind::Group)
        .count();
    let cards = canvas.nodes.len() - groups;
    let _ = writeln!(
        out,
        "<header class=\"bar\"><div class=\"bar-main\"><div class=\"bar-title\">{title}</div>\
<div class=\"bar-meta\">Нод: {cards} · Групп: {groups} · Связей: {}</div></div>\
<div class=\"bar-actions\"><button id=\"zoom-out\" type=\"button\">−</button>\
<button id=\"zoom-reset\" type=\"button\">100%</button>\
<button id=\"zoom-in\" type=\"button\">+</button>\
<button id=\"theme\" type=\"button\">Тема</button></div></header>",
        canvas.edges.len()
    );
    // SVG-снимок канваса. Явные width/height (из того же bbox, что и
    // viewBox) дают svg внутренний размер — без них svg без intrinsic-размера
    // схлопывается в 0×0 (браузерный пруф сессии, скриншот-верификация)
    let ((view_box, svg_w, svg_h), svg_body) = build_svg(canvas, solutions);
    let _ = writeln!(
        out,
        "<main class=\"stage\"><div class=\"scroller\"><div id=\"zoomable\">\
<svg width=\"{}\" height=\"{}\" viewBox=\"{view_box}\" role=\"img\" aria-label=\"Снимок канваса\">{svg_body}</svg>\
</div></div></main>",
        fmt_num(svg_w),
        fmt_num(svg_h)
    );
    // Таблица сравнения what-if (если есть)
    if let Some(comparison) = comparison {
        if !comparison.rows.is_empty() {
            out.push_str(&build_whatif_table(canvas, comparison));
        }
    }
    out.push_str(
        "<footer class=\"foot\">CanvasDesk — самодостаточный экспорт · \
значения последнего пересчёта · файл работает офлайн</footer>\n",
    );
    out.push_str("<script>\n");
    out.push_str(JS);
    out.push_str("</script>\n</body>\n</html>\n");
    out
}

/// SVG-снимок: группы-подложки → рёбра → карточки (порядок слоёв как на
/// канвасе). Возвращает ((viewBox, ширина, высота), тело svg) — размер и
/// viewBox из одного bbox.
fn build_svg(canvas: &Canvas, solutions: &FlowSolutions) -> ((String, f32, f32), String) {
    let mut body = String::with_capacity(8 * 1024);
    body.push_str("<defs>");
    body.push_str(
        "<marker id=\"arrow-flow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\
<path d=\"M0,0 L10,5 L0,10 z\" class=\"arrow-flow\"/></marker>",
    );
    body.push_str(
        "<marker id=\"arrow-ctl\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\">\
<path d=\"M0,0 L10,5 L0,10 z\" class=\"arrow-ctl\"/></marker>",
    );
    // Сетка-точки: деликатный фон снимка (CSS-переменная темы)
    body.push_str(
        "<pattern id=\"dots\" width=\"24\" height=\"24\" patternUnits=\"userSpaceOnUse\">\
<circle cx=\"1.5\" cy=\"1.5\" r=\"1.2\" class=\"dot\"/></pattern>",
    );
    body.push_str("</defs>");
    body.push_str("<rect class=\"bg-dots\" x=\"0\" y=\"0\" width=\"100%\" height=\"100%\"/>");
    // 1) Группы — подложки (у групп нет портов и тела, только рамка+подпись)
    for node in &canvas.nodes {
        if node.kind() == NodeKind::Group {
            push_group(&mut body, node);
        }
    }
    // 2) Рёбра — под карточками (как на канвасе: линии уходят под ноды)
    for edge in &canvas.edges {
        push_edge(&mut body, canvas, edge, solutions);
    }
    // 3) Карточки
    for node in &canvas.nodes {
        if node.kind() != NodeKind::Group {
            push_node(&mut body, node, solutions);
        }
    }
    let bounds = view_box(canvas);
    (bounds, body)
}

/// Границы снимка: bbox нод + поле 48 px; пустой канвас — дефолт 1200×800.
/// Возвращает (viewBox, ширина, высота).
fn view_box(canvas: &Canvas) -> (String, f32, f32) {
    const PAD: f32 = 48.0;
    let mut out = String::new();
    match (
        canvas
            .nodes
            .iter()
            .map(|n| n.x)
            .fold(f32::INFINITY, f32::min),
        canvas
            .nodes
            .iter()
            .map(|n| n.y)
            .fold(f32::INFINITY, f32::min),
        canvas
            .nodes
            .iter()
            .map(|n| n.x + n.width)
            .fold(f32::NEG_INFINITY, f32::max),
        canvas
            .nodes
            .iter()
            .map(|n| n.y + n.height)
            .fold(f32::NEG_INFINITY, f32::max),
    ) {
        (min_x, min_y, max_x, max_y) if min_x.is_finite() && min_y.is_finite() => {
            let w = (max_x - min_x) + PAD * 2.0;
            let h = (max_y - min_y) + PAD * 2.0;
            let _ = write!(
                out,
                "{} {} {} {}",
                fmt_num(min_x - PAD),
                fmt_num(min_y - PAD),
                fmt_num(w),
                fmt_num(h)
            );
            (out, w, h)
        }
        _ => {
            out.push_str("0 0 1200 800");
            (out, 1200.0, 800.0)
        }
    }
}

/// Группа-подложка: пунктирная рамка, полупрозрачная заливка, подпись.
fn push_group(out: &mut String, node: &Node) {
    let label = escape_html(&group_label(node));
    let _ = write!(
        out,
        "<g class=\"group\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"10\"/>\
<text x=\"{}\" y=\"{}\">{label}</text></g>",
        fmt_num(node.x),
        fmt_num(node.y),
        fmt_num(node.width),
        fmt_num(node.height),
        fmt_num(node.x + 12.0),
        fmt_num(node.y + 20.0)
    );
}

/// Ребро: кубическая Безье между якорями сторон + стрелка + подпись значения
/// (только value-рёбра, конвенция `stage_edge_value_text`).
fn push_edge(out: &mut String, canvas: &Canvas, edge: &Edge, solutions: &FlowSolutions) {
    let (Some(from), Some(to)) = (canvas.node(&edge.from_node), canvas.node(&edge.to_node)) else {
        return; // висячая ссылка — не рисуем (ядро толерантно к битым id)
    };
    let value_kind = edge.flow_kind() == FlowKind::Value;
    let from_side = edge.from_side.unwrap_or_else(|| auto_side(from, to));
    let to_side = edge.to_side.unwrap_or_else(|| auto_side(to, from));
    let p0 = anchor(from, from_side);
    let p1 = anchor(to, to_side);
    // Контрольные точки: вынос наружу стороны, масштаб — от расстояния
    let dist = ((p1.0 - p0.0).powi(2) + (p1.1 - p0.1).powi(2)).sqrt();
    let reach = (dist / 3.0).clamp(24.0, 80.0);
    let n0 = side_normal(from_side);
    let n1 = side_normal(to_side);
    let c0 = (p0.0 + n0.0 * reach, p0.1 + n0.1 * reach);
    let c1 = (p1.0 + n1.0 * reach, p1.1 + n1.1 * reach);
    let class = if value_kind { "edge-flow" } else { "edge-ctl" };
    let marker = if value_kind {
        "url(#arrow-flow)"
    } else {
        "url(#arrow-ctl)"
    };
    let dash = match edge.style {
        Some(EdgeLineStyle::Dashed) => " stroke-dasharray=\"7 5\"",
        Some(EdgeLineStyle::Dotted) => " stroke-dasharray=\"1.5 4\"",
        _ => "",
    };
    let _ = write!(
        out,
        "<path class=\"{class}\"{dash} marker-end=\"{marker}\" d=\"M {} {} C {} {} {} {} {} {}\"/>",
        fmt_num(p0.0),
        fmt_num(p0.1),
        fmt_num(c0.0),
        fmt_num(c0.1),
        fmt_num(c1.0),
        fmt_num(c1.1),
        fmt_num(p1.0),
        fmt_num(p1.1)
    );
    // Подпись значения на value-ребре (конвенция адресации FR-025/FR-029)
    if value_kind {
        if let Some(value) = edge_value(edge, solutions) {
            let text = escape_html(&value.to_string());
            // Точка Безье при t=0.5: 0.125·p0 + 0.375·c0 + 0.375·c1 + 0.125·p1
            let mx = 0.125 * p0.0 + 0.375 * c0.0 + 0.375 * c1.0 + 0.125 * p1.0;
            let my = 0.125 * p0.1 + 0.375 * c0.1 + 0.375 * c1.1 + 0.125 * p1.1;
            let _ = write!(
                out,
                "<text class=\"edge-value\" x=\"{}\" y=\"{}\">{text}</text>",
                fmt_num(mx),
                fmt_num(my - 6.0)
            );
        }
    }
    // Пользовательская подпись ребра (label) — над значением, приглушённо
    if let Some(label) = edge.label.as_deref().filter(|l| !l.is_empty()) {
        let text = escape_html(label);
        let ly = if value_kind {
            0.125 * p0.1 + 0.375 * c0.1 + 0.375 * c1.1 + 0.125 * p1.1 + 8.0
        } else {
            0.125 * p0.1 + 0.375 * c0.1 + 0.375 * c1.1 + 0.125 * p1.1 - 6.0
        };
        let lx = 0.125 * p0.0 + 0.375 * c0.0 + 0.375 * c1.0 + 0.125 * p1.0;
        let _ = write!(
            out,
            "<text class=\"edge-label\" x=\"{}\" y=\"{}\">{text}</text>",
            fmt_num(lx),
            fmt_num(ly)
        );
    }
}

/// Значение value-ребра по адресу: `fromLine` → построчный выход, `fromOutput`
/// → именованный, иначе узловой итог истока. None — значения нет.
fn edge_value(edge: &Edge, solutions: &FlowSolutions) -> Option<crate::expr::Value> {
    if let Some(line) = edge.from_line {
        return solutions
            .lines
            .get(&(edge.from_node.clone(), line))
            .cloned();
    }
    if let Some(output) = edge.from_output.as_deref() {
        return solutions
            .named
            .get(&(edge.from_node.clone(), output.to_owned()))
            .cloned();
    }
    solutions
        .outputs
        .get(&edge.from_node)
        .and_then(|outcome| outcome.as_ref().ok())
        .cloned()
}

/// Карточка: рамка, заголовок, строки тела (значения — справа, конвенция
/// Numi-листа), узловой итог — футером.
fn push_node(out: &mut String, node: &Node, solutions: &FlowSolutions) {
    let title = escape_html(&node_title(node));
    let fill = fill_attrs(node);
    let _ = write!(
        out,
        "<g class=\"node\"><rect {} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\"/>\
<text class=\"node-title\" x=\"{}\" y=\"{}\">{title}</text>",
        fill,
        fmt_num(node.x),
        fmt_num(node.y),
        fmt_num(node.width),
        fmt_num(node.height),
        fmt_num(node.x + 10.0),
        fmt_num(node.y + 19.0)
    );
    // Тело: строки текста; у формульных строк значение — справа (моно,
    // акцент). Переполнение — капом по высоте карточки (SVG-текст не
    // переносится, как и клипы приложения).
    const LINE_STEP: f32 = 16.0;
    const BODY_TOP: f32 = 36.0;
    let footer = solutions
        .outputs
        .get(&node.id)
        .and_then(|outcome| outcome.as_ref().ok())
        .map(|value| value.to_string());
    let budget = node.height - BODY_TOP - if footer.is_some() { 18.0 } else { 4.0 };
    let max_lines = ((budget / LINE_STEP).floor().max(0.0) as usize).min(64);
    let lines: Vec<&str> = node
        .text
        .as_deref()
        .map(|text| text.lines().collect())
        .unwrap_or_default();
    let shown = lines.len().min(max_lines);
    for (index, line) in lines.iter().take(shown).enumerate() {
        let y = node.y + BODY_TOP + LINE_STEP * index as f32;
        let text = escape_html(line);
        let _ = write!(
            out,
            "<text class=\"node-line\" x=\"{}\" y=\"{}\">{text}</text>",
            fmt_num(node.x + 10.0),
            fmt_num(y)
        );
        if let Some(value) = solutions.lines.get(&(node.id.clone(), index)) {
            let value = escape_html(&value.to_string());
            let _ = write!(
                out,
                "<text class=\"node-value\" x=\"{}\" y=\"{}\">{value}</text>",
                fmt_num(node.x + node.width - 10.0),
                fmt_num(y)
            );
        }
    }
    if shown < lines.len() {
        let y = node.y + BODY_TOP + LINE_STEP * shown as f32;
        let _ = write!(
            out,
            "<text class=\"node-more\" x=\"{}\" y=\"{}\">…</text>",
            fmt_num(node.x + 10.0),
            fmt_num(y)
        );
    }
    // Узловой итог — футер (шаблонные ноды: результат формулы манифеста)
    if let Some(footer) = footer {
        let footer = escape_html(&footer);
        let _ = write!(
            out,
            "<text class=\"node-total\" x=\"{}\" y=\"{}\">= {footer}</text>",
            fmt_num(node.x + 10.0),
            fmt_num(node.y + node.height - 7.0)
        );
    }
    out.push_str("</g>");
}

/// Таблица сравнения сценариев: «Переменная | База | С1 | С2 …», дельты —
/// в скобках (формат `whatif_delta_str`, как в панели приложения).
fn build_whatif_table(canvas: &Canvas, comparison: &ScenarioComparison) -> String {
    let mut out = String::with_capacity(2 * 1024);
    out.push_str(
        "<section class=\"whatif\"><h2>Сравнение сценариев</h2><table><thead><tr>\
<th>Переменная</th><th>База</th>",
    );
    for column in &comparison.columns {
        let column = escape_html(column);
        let _ = write!(out, "<th>{column}</th>");
    }
    out.push_str("</tr></thead><tbody>");
    for row in &comparison.rows {
        // Метка строки: заголовок ноды + адрес (строка N / итог)
        let node_label = canvas
            .node(&row.node)
            .map(node_title)
            .unwrap_or_else(|| row.node.clone());
        let label = match row.line {
            Some(line) => format!("{} : стр. {}", node_label, line + 1),
            None => format!("{} · итог", node_label),
        };
        let label = escape_html(&label);
        let _ = write!(out, "<tr><th>{label}</th>");
        for (index, value) in row.values.iter().enumerate() {
            match value {
                Some(value) => {
                    let value = escape_html(&value.to_string());
                    match row.deltas.get(index).and_then(|d| d.as_deref()) {
                        Some(delta) => {
                            let delta = escape_html(delta);
                            let _ = write!(
                                out,
                                "<td>{value} <span class=\"delta\">({delta})</span></td>"
                            );
                        }
                        None => {
                            let _ = write!(out, "<td>{value}</td>");
                        }
                    }
                }
                None => out.push_str("<td>—</td>"),
            }
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></section>");
    out
}

/// Заголовок карточки — упрощённый аналог `cards::title_for` (render-крейт
/// недоступен из ядра; конвенция фолбэков сохранена): файл → явный заголовок
/// → имя шаблона → первая строка текста → label.
fn node_title(node: &Node) -> String {
    if let Some(file) = &node.file {
        let name = file
            .rsplit(['/', '\\'])
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(file.as_str());
        return name.to_owned();
    }
    if let Some(title) = node.title() {
        if !title.is_empty() {
            return title.to_owned();
        }
        return "—".to_owned();
    }
    if let Some(name) = node
        .template()
        .and_then(|template| template.name)
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    if let Some(text) = &node.text {
        if let Some(line) = text.lines().next().filter(|line| !line.is_empty()) {
            return line.to_owned();
        }
    }
    if let Some(label) = node.label.as_deref().filter(|l| !l.is_empty()) {
        return label.to_owned();
    }
    "—".to_owned()
}

/// Подпись группы: label, иначе нейтральный дефолт (как `cards::title_for`).
fn group_label(node: &Node) -> String {
    node.label
        .as_deref()
        .filter(|label| !label.is_empty())
        .unwrap_or("Группа")
        .to_owned()
}

/// Атрибуты заливки карточки: пресет «1»..«6» — CSS-класс (адаптируется к
/// теме артефакта, палитры повторяют `PRESET_COLORS_*` render-крейта),
/// `#RRGGBB` — инлайн-стиль (тема не меняет явный цвет), иначе дефолт темы.
fn fill_attrs(node: &Node) -> String {
    if let Some(color) = node.color.as_deref() {
        if (1..=6).map(|i| i.to_string()).any(|preset| preset == color) {
            return format!("class=\"card np-{color}\"");
        }
        if color.starts_with('#')
            && color.len() == 7
            && color[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return format!("class=\"card\" style=\"fill:{color}\"");
        }
    }
    "class=\"card np-default\"".to_owned()
}

/// HTML-экранирование пользовательского текста (атрибуты не содержат
/// пользовательских данных — все значения идут в текстовые узлы).
fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Число для SVG-атрибутов: без хвостовых нулей (`100`, `12.5`).
fn fmt_num(value: f32) -> String {
    let mut text = format!("{value:.2}");
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_owned();
    }
    text
}

/// Якорь стороны: середина соответствующей грани прямоугольника.
fn anchor(node: &Node, side: Side) -> (f32, f32) {
    match side {
        Side::Right => (node.x + node.width, node.y + node.height / 2.0),
        Side::Left => (node.x, node.y + node.height / 2.0),
        Side::Top => (node.x + node.width / 2.0, node.y),
        Side::Bottom => (node.x + node.width / 2.0, node.y + node.height),
    }
}

/// Внешняя нормаль стороны (для выноса контрольной точки Безье).
fn side_normal(side: Side) -> (f32, f32) {
    match side {
        Side::Right => (1.0, 0.0),
        Side::Left => (-1.0, 0.0),
        Side::Top => (0.0, -1.0),
        Side::Bottom => (0.0, 1.0),
    }
}

/// Авто-сторона (сторона не задана): к цели — по доминирующей оси центров.
fn auto_side(node: &Node, target: &Node) -> Side {
    let dx = (target.x + target.width / 2.0) - (node.x + node.width / 2.0);
    let dy = (target.y + target.height / 2.0) - (node.y + node.height / 2.0);
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            Side::Right
        } else {
            Side::Left
        }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

/// Инлайн-CSS артефакта: темы через `[data-theme]`, пресеты `np-1..6`
/// (палитры повторяют `PRESET_COLORS_DARK/LIGHT` cards.rs:120/133), офлайн.
const CSS: &str = r#"
:root, [data-theme="dark"] {
  --bg:#14161a; --panel:#1b1e24; --card:#232730; --text:#e8eaf0; --muted:#9aa3b2;
  --accent:#7aa2f7; --edge:#565f70; --group:#3b4252; --group-fill:#1d2128;
  --np1:#6b3d3d; --np2:#735433; --np3:#6b6130; --np4:#3d6642; --np5:#336166; --np6:#5c4573;
  --dot:#262a32; --table-line:#2c313a;
}
[data-theme="light"] {
  --bg:#f5f6f8; --panel:#ffffff; --card:#ffffff; --text:#1f2430; --muted:#68708a;
  --accent:#3b5bdb; --edge:#a0a8ba; --group:#c8cede; --group-fill:#eef0f4;
  --np1:#edc2ba; --np2:#f0d4a3; --np3:#ebdea8; --np4:#bfd6bd; --np5:#b3d1d6; --np6:#d1bfde;
  --dot:#d8dce4; --table-line:#dfe3ea;
}
* { box-sizing: border-box; }
html, body { margin:0; padding:0; background:var(--bg); color:var(--text);
  font:14px/1.45 -apple-system, "Segoe UI", Roboto, "Noto Sans", sans-serif; }
.bar { display:flex; justify-content:space-between; align-items:center; gap:12px;
  padding:10px 16px; background:var(--panel); border-bottom:1px solid var(--table-line);
  position:sticky; top:0; z-index:2; flex-wrap:wrap; }
.bar-title { font-size:16px; font-weight:600; }
.bar-meta { color:var(--muted); font-size:12.5px; margin-top:2px; }
.bar-actions { display:flex; gap:6px; }
.bar-actions button { background:transparent; color:var(--text); border:1px solid var(--table-line);
  border-radius:6px; padding:5px 10px; font:inherit; font-size:12.5px; cursor:pointer; }
.bar-actions button:hover { border-color:var(--accent); }
.stage { overflow:auto; max-height:78vh; border-bottom:1px solid var(--table-line); }
.scroller { padding:8px; }
#zoomable { transform-origin:0 0; width:max-content; }
#zoomable svg { display:block; max-width:none; }
.bg-dots { fill:url(#dots); }
.dot { fill:var(--dot); }
.group rect { fill:var(--group-fill); fill-opacity:.55; stroke:var(--group); stroke-width:1.5;
  stroke-dasharray:8 6; }
.group text { fill:var(--muted); font-size:12px; font-weight:600; }
.node-title { fill:var(--text); font-size:13px; font-weight:600; }
.node-line { fill:var(--text); font-size:12px;
  font-family:ui-monospace, "Cascadia Mono", Consolas, "Noto Sans Mono", monospace; }
.node-value { fill:var(--accent); font-size:12px; text-anchor:end;
  font-family:ui-monospace, "Cascadia Mono", Consolas, "Noto Sans Mono", monospace; }
.node-total { fill:var(--accent); font-size:12.5px; font-weight:600;
  font-family:ui-monospace, "Cascadia Mono", Consolas, "Noto Sans Mono", monospace; }
.node-more { fill:var(--muted); font-size:12px; }
.card { stroke:var(--table-line); stroke-width:1; }
.np-default { fill:var(--card); }
.np-1 { fill:var(--np1); } .np-2 { fill:var(--np2); } .np-3 { fill:var(--np3); }
.np-4 { fill:var(--np4); } .np-5 { fill:var(--np5); } .np-6 { fill:var(--np6); }
.edge-flow { stroke:var(--accent); stroke-width:2; fill:none; }
.edge-ctl { stroke:var(--edge); stroke-width:1.8; fill:none; }
.arrow-flow { fill:var(--accent); } .arrow-ctl { fill:var(--edge); }
.edge-value { fill:var(--accent); font-size:11px; text-anchor:middle;
  paint-order:stroke; stroke:var(--bg); stroke-width:3px;
  font-family:ui-monospace, "Cascadia Mono", Consolas, "Noto Sans Mono", monospace; }
.edge-label { fill:var(--muted); font-size:11px; text-anchor:middle;
  paint-order:stroke; stroke:var(--bg); stroke-width:3px; }
.whatif { padding:14px 16px; }
.whatif h2 { font-size:15px; margin:0 0 10px; }
.whatif table { border-collapse:collapse; font-size:12.5px; }
.whatif th, .whatif td { border:1px solid var(--table-line); padding:6px 10px; text-align:left; }
.whatif thead th { background:var(--panel); }
.whatif tbody th { font-weight:600; }
.whatif td { font-family:ui-monospace, "Cascadia Mono", Consolas, "Noto Sans Mono", monospace; }
.delta { color:var(--muted); font-size:11.5px; }
.foot { color:var(--muted); font-size:12px; padding:10px 16px; }
"#;

/// Инлайн-JS: переключение темы и зум (кнопки) — без внешних зависимостей.
const JS: &str = r#"
(function () {
  "use strict";
  var root = document.documentElement;
  var themeBtn = document.getElementById("theme");
  var zoomable = document.getElementById("zoomable");
  var scale = 1;
  function applyZoom() {
    zoomable.style.transform = "scale(" + scale + ")";
  }
  function setThemeLabel() {
    var dark = root.getAttribute("data-theme") === "dark";
    themeBtn.textContent = dark ? "Светлая тема" : "Тёмная тема";
  }
  themeBtn.addEventListener("click", function () {
    var dark = root.getAttribute("data-theme") === "dark";
    root.setAttribute("data-theme", dark ? "light" : "dark");
    setThemeLabel();
  });
  document.getElementById("zoom-in").addEventListener("click", function () {
    scale = Math.min(3, scale * 1.25); applyZoom();
  });
  document.getElementById("zoom-out").addEventListener("click", function () {
    scale = Math.max(0.25, scale / 1.25); applyZoom();
  });
  document.getElementById("zoom-reset").addEventListener("click", function () {
    scale = 1; applyZoom();
  });
  setThemeLabel();
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow;
    use crate::model::{Edge, Node};

    /// Текстовая нода с формулой (id, x, y, текст).
    fn note(id: &str, x: f32, y: f32, text: &str) -> Node {
        let mut node = Node::text(id, text, x, y);
        node.width = 200.0;
        node.height = 90.0;
        node
    }

    /// Value-ребро A → B (последняя формульная строка).
    fn value_edge(from: &str, to: &str) -> Edge {
        let mut edge = Edge::new(format!("e-{from}-{to}"), from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        edge
    }

    fn canvas_with(nodes: Vec<Node>, edges: Vec<Edge>) -> Canvas {
        Canvas {
            nodes,
            edges,
            ..Default::default()
        }
    }

    #[test]
    fn empty_canvas_renders_skeleton() {
        let canvas = Canvas::default();
        let html = export_html(
            &canvas,
            &FlowSolutions::default(),
            None,
            &ExportHtmlOptions::default(),
        );
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("data-theme=\"dark\""));
        assert!(html.contains("<title>CanvasDesk — CanvasDesk</title>"));
        assert!(html.contains("viewBox=\"0 0 1200 800\""));
        assert!(html.contains("Нод: 0"));
        assert!(html.contains("Связей: 0"));
        // Таблицы what-if нет, футер-метка офлайн есть
        assert!(!html.contains("whatif\">"));
        assert!(html.contains("офлайн"));
    }

    #[test]
    fn formula_line_gets_value() {
        let canvas = canvas_with(vec![note("a", 0.0, 0.0, "rps = 1000 rps")], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        // Строка листа и её значение (справа; NBSP-группировка тысяч — Display ядра)
        assert!(html.contains("rps = 1000 rps"));
        assert!(html.contains("1\u{a0}000 rps"));
        // Заголовок — первая строка текста (фолбэк конвенции)
        assert!(html.contains("node-title"));
    }

    #[test]
    fn node_output_is_footer() {
        let canvas = canvas_with(vec![note("a", 0.0, 0.0, "x = 2\ny = x * 3")], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("node-total"));
        assert!(html.contains("= 6"));
    }

    #[test]
    fn user_text_is_escaped() {
        let canvas = canvas_with(
            vec![note("a", 0.0, 0.0, "<script>alert(1)</script>\nx = 2")],
            vec![],
        );
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn value_edge_carries_label() {
        let canvas = canvas_with(
            vec![
                note("a", 0.0, 0.0, "rps = 500 rps"),
                note("b", 400.0, 0.0, "lat = 20 ms"),
            ],
            vec![value_edge("a", "b")],
        );
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("edge-flow"));
        // Значение истока — подписью на ребре
        assert!(html.contains("edge-value"));
        assert!(html.contains("500 rps"));
    }

    #[test]
    fn deterministic_bytes() {
        let canvas = canvas_with(
            vec![
                note("a", 0.0, 0.0, "rps = 500 rps"),
                note("b", 400.0, 0.0, "lat = 20 ms"),
            ],
            vec![value_edge("a", "b")],
        );
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let first = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        let second = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert_eq!(first, second);
    }

    #[test]
    fn group_renders_as_backdrop() {
        let mut group = Node::text("g", "", -20.0, -20.0);
        group.width = 500.0;
        group.height = 300.0;
        group.label = Some("Кластер".to_owned());
        group.node_type = "group".to_owned();
        let canvas = canvas_with(vec![group, note("a", 0.0, 0.0, "x = 2")], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("class=\"group\""));
        assert!(html.contains("Кластер"));
        // Группа не попала в счётчик карточек
        assert!(html.contains("Нод: 1"));
        assert!(html.contains("Групп: 1"));
    }

    #[test]
    fn color_preset_and_custom_hex() {
        let mut preset = note("a", 0.0, 0.0, "x = 2");
        preset.color = Some("3".to_owned());
        let mut custom = note("b", 300.0, 0.0, "y = 3");
        custom.color = Some("#aabbcc".to_owned());
        let canvas = canvas_with(vec![preset, custom], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("class=\"card np-3\""));
        assert!(html.contains("style=\"fill:#aabbcc\""));
        // Обе карточки с цветом — дефолтной ЗАЛИВКИ нет (правило CSS остаётся)
        assert!(!html.contains("class=\"card np-default\""));
    }

    #[test]
    fn whatif_table_with_deltas() {
        // База: x = 2; сценарий подменяет строку на x = 5
        let mut canvas = canvas_with(vec![note("a", 0.0, 0.0, "x = 2")], vec![]);
        canvas.extra.insert(
            "canvasdesk".to_owned(),
            serde_json::json!({
                "whatif": {
                    "scenarios": [
                        { "name": "Рост", "overrides": [ { "node": "a", "line": 0, "expr": "x = 5" } ] }
                    ]
                }
            }),
        );
        let base = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let comparison = scenario_comparison_for_export(&canvas, &base);
        let comparison = comparison.expect("сценарий есть");
        let html = export_html(
            &canvas,
            &base,
            Some(&comparison),
            &ExportHtmlOptions::default(),
        );
        assert!(html.contains("Сравнение сценариев"));
        assert!(html.contains("Рост"));
        // База — без дельты, сценарий — со дельтой в скобках (формат FR-017)
        assert!(html.contains(">2</td>"));
        assert!(html.contains("5 <span class=\"delta\">"));
    }

    #[test]
    fn whatif_absent_without_scenarios() {
        let canvas = canvas_with(vec![note("a", 0.0, 0.0, "x = 2")], vec![]);
        let base = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        assert!(scenario_comparison_for_export(&canvas, &base).is_none());
    }

    #[test]
    fn title_uses_explicit_over_template() {
        let mut node = note("a", 0.0, 0.0, "rps = 1000 rps");
        node.canvasdesk = Some(crate::model::CanvasdeskExt {
            widget_id: None,
            props: serde_json::Map::new(),
            expr: None,
            template: None,
            desc: None,
            data: None,
            title: Some("Шлюз".to_owned()),
        });
        let canvas = canvas_with(vec![node], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("Шлюз"));
    }

    #[test]
    fn hanging_edge_is_skipped() {
        let canvas = canvas_with(
            vec![note("a", 0.0, 0.0, "x = 2")],
            vec![value_edge("a", "ghost")],
        );
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        // Ребро не рисуется, артефакт собирается
        assert!(html.contains("Связей: 1"));
        assert!(!html.contains("edge-flow\""));
    }

    #[test]
    fn line_overflow_is_capped() {
        // 10 строк в карточке высотой 54: влезет мало, хвост — «…»
        let long = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let canvas = canvas_with(vec![note("a", 0.0, 0.0, long)], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("node-more"));
    }

    #[test]
    fn golden_file_roundtrip_structure() {
        // Структурный скелет артефакта: шапка/сцена/футер/скрипт на месте
        let canvas = canvas_with(vec![note("a", 0.0, 0.0, "x = 2")], vec![]);
        let solutions = flow::propagate_with_lines(&canvas, &Default::default()).unwrap();
        let html = export_html(&canvas, &solutions, None, &ExportHtmlOptions::default());
        assert!(html.contains("<header class=\"bar\">"));
        assert!(html.contains("<main class=\"stage\">"));
        assert!(html.contains("<footer class=\"foot\">"));
        assert!(html.contains("<script>"));
        assert!(html.ends_with("</html>\n"));
        // Офлайн-инвариант: нет внешних ссылок (http/src=)
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("src="));
    }
}
