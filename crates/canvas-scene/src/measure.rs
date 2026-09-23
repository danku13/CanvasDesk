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
pub fn estimated_result_reserve_height(text: &str, node_width: f32, desc: &str) -> f32 {
    let body_width = (node_width - BODY_PADDING * 2.0).max(BODY_PADDING);
    let rows = wrapped_body_rows(text, body_width);
    // FR-061 этап D (D-8): зона описания — кламп ≤ 2 строк (токен
    // TABLE_DESC_CLAMP_LINES) + зазор после зоны; консервативная оценка.
    // ПРИЁМКА T9 (фикс подреза высоты): при описании, верстающемся в
    // CLAMP строк и больше, стек содержит СТРОКУ ЭКСПАНДЕРА («⋯ целиком
    // ▾») — раньше она не учитывалась, ранний выход уровня 1 оставлял
    // ноду на строку короче (контент вылезал за низ карточки). Оценка
    // числа строк описания — по средней ширине глифа; на границе (ровно
    // CLAMP) экспандер предполагается — консервативно в большую сторону
    // (уровень 2 — точное измерение — скорректирует при раннем выходе).
    let desc_rows = if desc.trim().is_empty() {
        0.0
    } else {
        let clamp = canvas_core::tokens::TABLE_DESC_CLAMP_LINES as f32;
        let wrapped = wrapped_body_rows(desc, body_width) as f32;
        let expander = if wrapped >= clamp {
            BODY_LINE_HEIGHT
        } else {
            0.0
        };
        wrapped.min(clamp) * BODY_LINE_HEIGHT + expander + 6.0
    };
    HEADER_HEIGHT
        + BODY_TOP_GAP
        + desc_rows
        + rows as f32 * BODY_LINE_HEIGHT
        + BODY_PADDING
        + RESULT_LINE_HEIGHT
        + 2.0
}

/// Точное измерение требуемой высоты (уровень 2, CR-012 правка 2):
/// (текст, ширина ноды, формульные строки) → высота. Продуктовая
/// реализация (canvas-app) шейпит реальными Noto-шрифтами через
/// canvas-render::text::measure_body_height; без установки — оценка
/// уровня 1 (консервативная, только растит высоту).
pub type MeasuredReserveFn =
    fn(text: &str, node_width: f32, formula_lines: &[usize], desc: &str) -> f32;

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
fn measured_reserve(text: &str, node_width: f32, formula_lines: &[usize], desc: &str) -> f32 {
    let guard = MEASURED_RESERVE.read().unwrap_or_else(|p| p.into_inner());
    match *guard {
        Some(f) => f(text, node_width, formula_lines, desc),
        None => estimated_result_reserve_height(text, node_width, desc),
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
) {
    let desc_text = desc.unwrap_or_default();
    if estimated_result_reserve_height(display_text, node.width, desc_text) <= node.height {
        return;
    }
    let needed = measured_reserve(display_text, node.width, formula_lines, desc_text);
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
    ensure_result_reserve(node, &text, &formula_lines, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Приёмка T9 FR-061 (фикс подреза высоты): оценка резерва для
    /// описания, верстающегося в CLAMP строк и больше, включает строку
    /// экспандера («⋯ целиком ▾») — раньше она не учитывалась, ранний
    /// выход уровня 1 оставлял ноду на строку короче.
    #[test]
    fn estimate_includes_desc_expander_row() {
        let width = 360.0;
        let short_desc = "Короткое описание."; // одна строка — без экспандера
        let long_desc = "Длинное описание узла расчёта нагрузки, которое заведомо \
не помещается в две строки клампа и потому сворачивается с аффордансом \
«⋯ целиком ▾» — строка экспандера обязана войти в резерв высоты.";
        let base = estimated_result_reserve_height("текст", width, "");
        let with_short = estimated_result_reserve_height("текст", width, short_desc);
        let with_long = estimated_result_reserve_height("текст", width, long_desc);
        // Короткое описание (1 строка): только строки клампа + зазор.
        assert!(
            with_short - base
                < canvas_core::tokens::TABLE_DESC_CLAMP_LINES as f32 * BODY_LINE_HEIGHT + 6.0,
            "короткое описание без экспандера"
        );
        // Длинное описание: кламп + ЭКСПАНДЕР — как минимум на строку больше
        // короткого (рост-only: занижать нельзя).
        assert!(
            with_long - with_short >= BODY_LINE_HEIGHT,
            "экспандер описания учтён в оценке ({} против {})",
            with_long - base,
            with_short - base
        );
    }
}
