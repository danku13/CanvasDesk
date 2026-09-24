//! CR-010/CR-012: двухуровневый ленивый refit высоты ноды под резерв
//! футера результата. Уровень 1 — консервативная оценка (переносы строк
//! по средней ширине глифа); уровень 2 — точное измерение шейпингом,
//! инжектируется приложением (canvas-render) через
//! [`install_measured_reserve`]: canvas-scene не зависит от canvas-render
//! (ADR-0012), в headless/wasm-режиме работает консервативная оценка
//! (growth-only, безопасность сохраняется).

use canvas_core::expr::{self, line_kind, ExprOutcome, NumiLineKind};
use canvas_core::Node;

// --- метрики раскладки (значения синхронны с canvas-render: cards.rs
// HEADER_HEIGHT, text.rs BODY_*/RESULT_LINE_HEIGHT; паритет — тестом
// canvas-app measure_layout_consts_match_render) ---
/// Высота шапки карточки (canvas-render/cards.rs).
pub const HEADER_HEIGHT: f32 = 34.0;
/// Размер шрифта тела (canvas-render/text.rs).
pub const BODY_FONT_SIZE: f32 = 14.0;
/// Высота ряда тела (canvas-render/text.rs).
pub const BODY_LINE_HEIGHT: f32 = 20.0;
/// Паддинг тела карточки (canvas-render/text.rs).
pub const BODY_PADDING: f32 = 10.0;
/// Зазор шапка→тело (canvas-render/text.rs).
pub const BODY_TOP_GAP: f32 = 4.0;
/// Высота ряда футера результата (canvas-render/text.rs).
pub const RESULT_LINE_HEIGHT: f32 = 16.0;

/// FR-067 (этап F): деривация супрессии первого проза-абзаца — зеркало
/// `with_body_stack` (canvas-render): зона описания показывает ЕГО ЖЕ
/// первый проза-абзац текста (фолбэк Q3 «desc→манифест→проза», либо
/// canvasdesk.desc/манифест, дословно равный абзацу) → эти строки из
/// вёрстки тела убраны. Общие чистые функции ядра
/// ([`canvas_core::expr::first_prose_paragraph`]/`_span`) не дают рендеру
/// и оценке разъехаться (I-2).
fn desc_paragraph_suppress_span(text: &str, desc: &str) -> Option<(usize, usize)> {
    if desc.trim().is_empty() {
        return None;
    }
    canvas_core::expr::first_prose_paragraph(text)
        .filter(|para| para == desc)
        .and_then(|_| canvas_core::expr::first_prose_paragraph_span(text))
}

/// CR-010: оценка числа визуальных рядов тела с учётом переносов. Рендер
/// шейпит тело с `Wrap::WordOrGlyph` в области шириной `body_width`, поэтому
/// длинная строка параметра даёт несколько рядов, хотя `\n`-строка одна.
/// Средняя ширина глифа Noto Sans 14 px (смешанная кириллица/латиница) —
/// оценка консервативная: функция только РАСТИТ высоту, занижать нельзя.
/// CJK-идеографы считаются двойными юнитами.
/// CR-012: строки Numi-листа (присваивания/выражения) рендерятся
/// моноширинным Noto Sans Mono (mono-флаг source_line, text.rs) — их
/// аванс шире пропорционального, и считаются они по моноширинной
/// метрике (`MONO_AVG_CHAR_W`), иначе переносы недооценивались на ряд.
pub fn wrapped_body_rows(text: &str, body_width: f32) -> usize {
    const AVG_CHAR_W: f32 = 7.0;
    let units = |c: char| -> f32 {
        match c {
            '\u{2E80}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF00}'..='\u{FF60}' => 2.0,
            _ => 1.0,
        }
    };
    text.lines()
        .map(|line| {
            let char_w = if line_kind(line) == NumiLineKind::Prose {
                AVG_CHAR_W
            } else {
                MONO_AVG_CHAR_W
            };
            let units_per_line = (body_width / char_w).floor().max(1.0);
            let line_units: f32 = line.chars().map(units).sum();
            (line_units / units_per_line).ceil().max(1.0) as usize
        })
        .sum::<usize>()
        .max(1)
}

/// CR-012: аванс Noto Sans Mono на символ в px (≈0.614 em при размере
/// тела 14 px). Строки Numi-листа рендерятся моноширинным шрифтом —
/// пропорциональная оценка 7 px/символ занижала число рядов переносов.
/// Завязана на [`BODY_FONT_SIZE`]: при смене размера тела метрика едет
/// вместе с ним. Завышение здесь безопасно: высота только РАСТЁТ.
const MONO_AVG_CHAR_W: f32 = 0.614 * BODY_FONT_SIZE;

/// CR-012: оценочная требуемая высота ноды с учётом резерва футера
/// результата (шапка + тело с переносами + паддинг + резерв футера) —
/// дешевая метрика среднего аванса символа. Оценка только РАСТИТ высоту
/// (завышение безопасно), поэтому годится воротами двухуровневого refit:
/// если оценка влезает в текущую высоту, точное измерение не нужно.
pub fn estimated_result_reserve_height(
    text: &str,
    node_width: f32,
    formula_lines: &[usize],
    desc: &str,
    desc_expanded: bool,
    footer_reserve: bool,
) -> f32 {
    let body_width = (node_width - BODY_PADDING * 2.0).max(BODY_PADDING);
    // FR-067 (этап F): супрессия абзаца описания — зона описания показывает
    // первый проза-абзац → эти строки из верстки тела убраны; без учёта
    // оценка дважды считала абзац (завышение → лишние refit'ы уровня 2).
    let rows_text: String = match desc_paragraph_suppress_span(text, desc) {
        None => text.to_owned(),
        Some((start, end)) => text
            .lines()
            .enumerate()
            .filter(|(i, _)| *i < start || *i >= end)
            .map(|(_, line)| line)
            .collect::<Vec<&str>>()
            .join("\n"),
    };
    let rows = wrapped_body_rows(&rows_text, body_width);
    // FR-067 (этап F): строка заголовка блока-ведомости — отдельный ряд
    // стека (body_items), которого оценка не видела → гейт уровня 1
    // пропускал переполнение на один ряд. Общий план ядра — тот же, что
    // у рендера/измерения (I-2).
    let header_rows = if canvas_core::expr::block_header_plan(
        &text.lines().collect::<Vec<&str>>(),
        formula_lines,
    )
    .is_some()
    {
        1.0
    } else {
        0.0
    };
    // FR-061 этап D (D-8) / FR-067: зона описания — кламп ≤ 2 строк (токен
    // TABLE_DESC_CLAMP_LINES) либо полная вёрстка при раскрытии («⋯ целиком
    // ▾») + строка аффорданса экспандера + зазор после зоны; консервативная
    // оценка (без усечения экспандера нет — завышение безопасно).
    let desc_rows = if desc.trim().is_empty() {
        0.0
    } else if desc_expanded {
        (wrapped_body_rows(desc, body_width) as f32 + 1.0) * BODY_LINE_HEIGHT + 6.0
    } else {
        (canvas_core::tokens::TABLE_DESC_CLAMP_LINES as f32 + 1.0) * BODY_LINE_HEIGHT + 6.0
    };
    // FR-067 (этап F): резерв футера — только нодам, которым рендер его
    // покажет (footer_reserve = node_shows_result_footer); у прочих нод
    // футера нет — без флага высота росла с «пустым хвостом».
    let footer = if footer_reserve {
        RESULT_LINE_HEIGHT + 2.0
    } else {
        0.0
    };
    HEADER_HEIGHT
        + BODY_TOP_GAP
        + desc_rows
        + (rows as f32 + header_rows) * BODY_LINE_HEIGHT
        + BODY_PADDING
        + footer
}

/// Точное измерение требуемой высоты (уровень 2, CR-012 правка 2):
/// (текст, ширина ноды, формульные строки) → высота. Продуктовая
/// реализация (canvas-app) шейпит реальными Noto-шрифтами через
/// canvas-render::text::measure_body_height; без установки — оценка
/// уровня 1 (консервативная, только растит высоту).
/// FR-067 (этап F): расширение запроса резерва — `desc_expanded`
/// (раскрытое описание «⋯ целиком ▾» растит стек — refit по тогглу) и
/// `footer_reserve` (резерв футера — только нодам с футером; подгонка
/// тела работает для ВСЕХ нод — ранний выход снят в ensure_reserve_at).
pub type MeasuredReserveFn = fn(
    text: &str,
    node_width: f32,
    formula_lines: &[usize],
    desc: &str,
    desc_expanded: bool,
    footer_reserve: bool,
) -> f32;

static MEASURED_RESERVE: std::sync::RwLock<Option<MeasuredReserveFn>> =
    std::sync::RwLock::new(None);

/// Установить точное измерение (вызывает canvas-app при старте и в тестах
/// с измеренными ассертами). Идемпотентно: повторная установка тем же
/// значением — no-op, другой функцией — замена (последняя установка
/// действует; в продукте функция одна).
pub fn install_measured_reserve(f: MeasuredReserveFn) {
    let mut guard = MEASURED_RESERVE.write().unwrap_or_else(|p| p.into_inner());
    *guard = Some(f);
}

/// Текущее измерение уровня 2: установленное приложением или оценка.
fn measured_reserve(
    text: &str,
    node_width: f32,
    formula_lines: &[usize],
    desc: &str,
    desc_expanded: bool,
    footer_reserve: bool,
) -> f32 {
    let guard = MEASURED_RESERVE.read().unwrap_or_else(|p| p.into_inner());
    match *guard {
        Some(f) => f(
            text,
            node_width,
            formula_lines,
            desc,
            desc_expanded,
            footer_reserve,
        ),
        None => estimated_result_reserve_height(
            text,
            node_width,
            formula_lines,
            desc,
            desc_expanded,
            footer_reserve,
        ),
    }
}

/// CR-012 (правка 2): двухуровневый ленивый refit высоты под резерв футера
/// результата. Growth-only: растит высоту, только если она занижена;
/// достаточную не трогает — без осцилляций при частых вызовах из
/// `recompute_flow`. Уровень 1 — дешёвая оценка (`estimated_result_reserve_height`):
/// влезает → выход (99 % вызовов, измерение не грузит перф). Уровень 2 —
/// точное измерение реальным шейпингом: рост ровно до измеренного needed,
/// без фантомных рядов. `display_text` — текст как на карточке (FR-029:
/// пролитые строки показаны подписями источников — они длиннее локальных
/// литералов, подгонка идёт по ним, иначе подпись вылезет за низ карточки).
pub fn ensure_result_reserve(
    node: &mut Node,
    display_text: &str,
    formula_lines: &[usize],
    desc: Option<&str>,
    desc_expanded: bool,
    footer_reserve: bool,
) {
    let desc_text = desc.unwrap_or_default();
    if estimated_result_reserve_height(
        display_text,
        node.width,
        formula_lines,
        desc_text,
        desc_expanded,
        footer_reserve,
    ) <= node.height
    {
        return;
    }
    let needed = measured_reserve(
        display_text,
        node.width,
        formula_lines,
        desc_text,
        desc_expanded,
        footer_reserve,
    );
    if needed > node.height {
        node.height = needed;
    }
}

/// CR-012 (правка 2): индексы строк с результатом из построчных исходов —
/// тот же источник, что у рендера (`expr_line_results` → formula_lines,
/// text.rs): по ним `body_items` ставит mono-флаг `source_line`.
pub fn formula_line_indices(line_results: &[Option<ExprOutcome>]) -> Vec<usize> {
    line_results
        .iter()
        .enumerate()
        .filter(|(_, outcome)| outcome.is_some())
        .map(|(i, _)| i)
        .collect()
}

/// FR-023: авто-высота шаблонной ноды — по списку параметров: шапка,
/// тело и футер результата. CR-010/CR-012: тело — с переносами,
/// двухуровневый refit `ensure_result_reserve`: длинное значение параметра
/// не вылезает за низ карточки у новой ноды. formula_lines — из
/// `expr::eval_lines` текста листа (тот же источник, что пишет
/// `expr_line_results` при пересчёте). Общая для GUI-инстанциации и MCP
/// `template_instantiate`: новая нода сразу влезает целиком (без «подгонки
/// правкой»). Только рост (не сжимает пользовательский размер).
pub fn fit_template_node_height(node: &mut Node) {
    let formula_lines = node
        .text
        .as_deref()
        .map(|text| formula_line_indices(&expr::eval_lines(text)))
        .unwrap_or_default();
    // Инстанциация: проливания ещё нет — display_text = исходный текст.
    // FR-061 D (D-8): desc — None (реестр шаблонов недоступен здесь);
    // ленивый refit (apply_result_reserve) догонит зону описания
    // growth-only при следующем пересчёте — документированная цена.
    let text = node.text.clone().unwrap_or_default();
    // FR-067: desc — None (реестр манифестов недоступен здесь, оценка без
    // супрессии консервативна); desc_expanded — дефолт (кламп);
    // footer_reserve — шаблонная нода показывает футер итога.
    ensure_result_reserve(node, &text, &formula_lines, None, false, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-067 (этап F): супрессия абзаца описания в оценке уровня 1 —
    /// desc == первый проза-абзац убирает его строки из счёта (зона
    /// описания уже показывает их); посторонний desc — нет.
    #[test]
    fn estimate_suppresses_desc_paragraph() {
        let text = "шлюз обрабатывает поток\n\nrps = 800 rps\nlatency = 12 ms";
        let para = canvas_core::expr::first_prose_paragraph(text).unwrap();
        let with_para = estimated_result_reserve_height(text, 300.0, &[], &para, false, true);
        let other =
            estimated_result_reserve_height(text, 300.0, &[], "постороннее описание", false, true);
        let none = estimated_result_reserve_height(text, 300.0, &[], "", false, true);
        // Абзац в зоне описания + супрессия тела: дешевле постороннего desc
        // (тело сохранило абзац — двойной счёт) и дороже отсутствия desc.
        assert!(with_para < other, "{with_para} < {other}");
        assert!(with_para > none, "{with_para} > {none}");
        // Деривация видна напрямую (зеркало with_body_stack)
        assert_eq!(desc_paragraph_suppress_span(text, &para), Some((0, 1)));
        assert_eq!(
            desc_paragraph_suppress_span(text, "постороннее описание"),
            None
        );
    }

    /// FR-067 (этап F): строка заголовка блока-ведомости — отдельный ряд
    /// в оценке (общий план ядра); без исходов плана нет — ряда нет.
    #[test]
    fn estimate_counts_block_header_row() {
        let five = "a = 1\nb = 2\nc = 3\nd = 4\nd * 2";
        let lines: Vec<usize> = vec![0, 1, 2, 3, 4];
        let with_header = estimated_result_reserve_height(five, 300.0, &lines, "", false, true);
        let without = estimated_result_reserve_height(five, 300.0, &[], "", false, true);
        assert_eq!(
            with_header - without,
            BODY_LINE_HEIGHT,
            "план блока добавляет ровно один ряд в оценку"
        );
        // Ниже порога T (4 строки данных) — заголовка нет
        let four = "a = 1\nb = 2\nc = 3\nd * 2";
        let four_lines: Vec<usize> = vec![0, 1, 2, 3];
        assert_eq!(
            estimated_result_reserve_height(four, 300.0, &four_lines, "", false, true),
            estimated_result_reserve_height(four, 300.0, &[], "", false, true),
            "порог T не достигнут — ряда заголовка в оценке нет"
        );
    }

    /// FR-067 (этап F): резерв футера — только по флагу; раскрытое
    /// описание длиннее клампа оценивается ПОЛНОЙ вёрсткой (+ экспандер).
    #[test]
    fn estimate_footer_flag_and_desc_expanded() {
        let text = "a = 1\nb = 2";
        let lines: Vec<usize> = vec![0, 1];
        let with_footer = estimated_result_reserve_height(text, 300.0, &lines, "", false, true);
        let without_footer = estimated_result_reserve_height(text, 300.0, &lines, "", false, false);
        assert_eq!(
            with_footer - without_footer,
            RESULT_LINE_HEIGHT + 2.0,
            "футер-флаг убирает «пустой хвост» у нод без футера"
        );
        let long_desc = "очень длинное описание ноды, которое точно не укладывается в кламп двух строк и раскрывается целиком по клику";
        let clamped = estimated_result_reserve_height(text, 300.0, &lines, long_desc, false, true);
        let expanded = estimated_result_reserve_height(text, 300.0, &lines, long_desc, true, true);
        assert!(
            expanded > clamped,
            "раскрытое описание оценок выше клампа: {expanded} > {clamped}"
        );
    }
}
