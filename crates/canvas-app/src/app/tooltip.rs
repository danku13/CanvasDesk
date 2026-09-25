//! Тултипы живого канваса: НЕПРОЗРАЧНАЯ подложка + перенос строк
//! (правка владельца 2026-09-26).
//!
//! Прежнее поведение: тултип — голый `OwnedScreenText` без квада, одна
//! строка, `width` — жёсткий клип рендера (`screen_text_area`:
//! `bounds.right = left + width`, `bottom = top + line_height`) — длинный
//! путь/диагноз обрезался, текст терялся на фоне карточек канваса.
//!
//! Правка владельца (2026-09-26):
//! 1. **Подложка непрозрачная** — квад меню-тона темы с альфой,
//!    форсированной в 1.0 (menu_fill пресета — 0.97; полупрозрачность
//!    просвечивала фон канваса), рамка palette_border, радиус кита 6
//!    (design/use-cases/tooltip.md §2–3). Квад — в той же полосе
//!    `UiLayer::Popups`: рендер рисует квады полосы ДО её текстов
//!    (FR-052), фон гарантированно под строками.
//! 2. **Перенос по словам — не больше [`TOOLTIP_MAX_WORDS`] слов в
//!    строке** (требование владельца); каждая строка — отдельный
//!    `OwnedScreenText` (конвейер рендера держит одну строку на текст),
//!    высота подложки — по числу строк.
//! 3. Ширина подложки — **измеренный максимум строк** (TextMeasurer,
//!    паритет шейпинга рендера: sans/MEDIUM, кегль × scale_factor),
//!    кламп по правому краю окна; по вертикали — флип НАД курсором у
//!    нижнего края (§6 use-case), затем кламп.
//!
//! Замер — под guard `measure_font_system` в коротком скоупе функции
//! (контракт W3.2/фикса рекурсивного лока: вложенный лок глобального
//! FontSystem запрещён; внутри скоупа locking-функции не вызываются).
//!
//! Рендер (`canvas-render`) не тронут: подложка и строки собираются на
//! стороне приложения в штатные полосы FR-052.

use canvas_render::cards::CardInstance;
use canvas_render::text::{measure_font_system, TextAlign, SANS_FAMILY};
use canvas_render::Color;
use canvas_ui::measure::{TextMeasurer, SCREEN_LINE_FACTOR};

use super::{band_rect_quad_pub, OwnedScreenText};

/// Максимум слов в строке тултипа (требование владельца, 2026-09-26).
pub(crate) const TOOLTIP_MAX_WORDS: usize = 10;

/// Кегль тултипа (лог. px) — прежний (правка не меняет кегль).
pub(crate) const TOOLTIP_FONT: f32 = 13.0;

/// Смещение подложки от курсора — `kit::TOOLTIP_OFFSET` (14, 18).
pub(crate) const TOOLTIP_OFFSET_X: f32 = 14.0;
pub(crate) const TOOLTIP_OFFSET_Y: f32 = 18.0;

/// Внутренние паддинги подложки (use-case §2: слот 6–8).
pub(crate) const TOOLTIP_PAD_X: f32 = 8.0;
pub(crate) const TOOLTIP_PAD_Y: f32 = 6.0;

/// Радиус подложки (use-case §2: RADIUS_CHIP 6).
pub(crate) const TOOLTIP_RADIUS: f32 = 6.0;

/// Зазор между подложками, когда активны два тултипа разом (например,
/// битая ссылка на hovered-ноде + усечённая формула под курсором) —
/// прежний вариант рисовал оба текста в одной точке (наложение).
pub(crate) const TOOLTIP_STACK_GAP: f32 = 8.0;

/// Шаг строк (лог. px) — паритет конвейеру рендера
/// (`line_height = font_size · 1.3`, `canvas-render/src/text.rs`).
pub(crate) const TOOLTIP_LINE_STEP: f32 = TOOLTIP_FONT * SCREEN_LINE_FACTOR;

/// Одна тултип-карточка: подложка + перенесённые строки.
///
/// `chunks` — строки-чанки КАЖДЫЙ переносится независимо (по ≤
/// [`TOOLTIP_MAX_WORDS`] слов) со своим цветом: одиночный тултип — один
/// чанк; лейбл порта (FR-045 v2) — семантические строки-истоки, каждая
/// со своим тоном.
pub(crate) struct TooltipCard {
    pub(crate) chunks: Vec<(String, Color)>,
}

impl TooltipCard {
    /// Одиночный тултип (один текст — один цвет).
    pub(crate) fn single(text: String, color: Color) -> Self {
        Self {
            chunks: vec![(text, color)],
        }
    }

    /// Многострочный тултип: готовые строки-чанки со своими цветами.
    pub(crate) fn rows(chunks: Vec<(String, Color)>) -> Self {
        Self { chunks }
    }
}

/// Перенос текста по словам: строка — не больше `max_words` слов.
///
/// Явные `\n` уважаются (параграфы переносятся независимо); слово —
/// токен по whitespace (CSS-подобная семантика `split_whitespace`),
/// КРОМЕ неразрывного пробела U+00A0: им группируются разряды чисел
/// (`1 234 567`, `format_num` в canvas-core) — NBSP-токен не рвётся,
/// число всегда переносится целиком. Неразрывный токен длиннее лимита
/// остаётся целой строкой — его клампит только ширина окна (вырожденный
/// случай: путь файла без пробелов). Пустой текст → пустой вектор
/// (карточка без строк не рисуется).
pub(crate) fn wrap_words(text: &str, max_words: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line: Vec<&str> = Vec::new();
        for word in para
            .split(|c: char| c.is_whitespace() && c != '\u{a0}')
            .filter(|word| !word.is_empty())
        {
            if line.len() >= max_words {
                out.push(line.join(" "));
                line.clear();
            }
            line.push(word);
        }
        if !line.is_empty() {
            out.push(line.join(" "));
        }
    }
    out
}

/// Собрать квады подложек и тексты тултипов кадра (screen-space, полоса
/// Popups). Карточки стекаются вниз с зазором [`TOOLTIP_STACK_GAP`];
/// позиция первой — курсор + [`TOOLTIP_OFFSET_X/Y`] с клампами окна.
pub(crate) fn layout_tooltips(
    cards: Vec<TooltipCard>,
    cursor: [f32; 2],
    viewport: [f32; 2],
    scale_factor: f32,
    mut fill: [f32; 4],
    border: [f32; 4],
) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    if cards.is_empty() || viewport[0] <= 0.0 || viewport[1] <= 0.0 {
        return (quads, texts);
    }
    // НЕПРОЗРАЧНАЯ подложка (требование владельца): menu_fill пресета —
    // α 0.97, форсируем 1.0 — фон канваса не просвечивает.
    fill[3] = 1.0;
    let scale = scale_factor.max(1e-6);
    // Замер — кегль в физических px (паритет шейпинга рендера: буферы
    // полос шейпятся `font_size · scale_factor`), ширины — обратно в лог.
    let phys_font = TOOLTIP_FONT * scale;
    let mut measurer = TextMeasurer::new();
    let mut fs = measure_font_system();
    let mut y = cursor[1] + TOOLTIP_OFFSET_Y;
    for card in cards {
        let mut lines: Vec<(String, Color)> = Vec::new();
        for (text, color) in &card.chunks {
            for line in wrap_words(text, TOOLTIP_MAX_WORDS) {
                lines.push((line, *color));
            }
        }
        if lines.is_empty() {
            continue;
        }
        // Ширина бокса — измеренный максимум строк + паддинги; кламп по
        // ширине окна (вырожденный неразрывный токен клипается краем
        // бокса, окно не покидает).
        let mut text_w = 0.0f32;
        for (line, _) in &lines {
            let w = measurer.width_of(&mut fs, line, SANS_FAMILY, phys_font) / scale;
            text_w = text_w.max(w);
        }
        let box_w = (text_w + 2.0 * TOOLTIP_PAD_X)
            .ceil()
            .min(viewport[0].max(2.0 * TOOLTIP_PAD_X));
        let box_h = 2.0 * TOOLTIP_PAD_Y + lines.len() as f32 * TOOLTIP_LINE_STEP;
        // Правый край ≤ окна (как прежде, но по фактической ширине бокса);
        // у нижнего края — флип НАД курсором (use-case §6), затем кламп.
        // top — локальная позиция ЭТОЙ карточки; курсор-каретка `y`
        // (карусель стека) обновляется в конце итерации явно.
        let x = (cursor[0] + TOOLTIP_OFFSET_X)
            .min((viewport[0] - box_w).max(0.0))
            .max(0.0);
        let mut top = y;
        if top + box_h > viewport[1] {
            top = cursor[1] - TOOLTIP_OFFSET_Y - box_h;
        }
        let top = top.max(0.0);
        quads.push(band_rect_quad_pub(
            [x, top, box_w, box_h],
            fill,
            border,
            TOOLTIP_RADIUS,
        ));
        for (row, (line, color)) in lines.iter().enumerate() {
            texts.push(OwnedScreenText {
                text: line.clone(),
                origin: [
                    x + TOOLTIP_PAD_X,
                    top + TOOLTIP_PAD_Y + row as f32 * TOOLTIP_LINE_STEP,
                ],
                // Клип строки — правый край бокса (текст уложен с запасом
                // паддинга; клипается только кламп-вырожденный случай).
                width: (box_w - TOOLTIP_PAD_X).max(1.0),
                font_size: TOOLTIP_FONT,
                color: *color,
                align: TextAlign::Left,
            });
        }
        y = top + box_h + TOOLTIP_STACK_GAP;
    }
    (quads, texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(text: &str) -> TooltipCard {
        TooltipCard::single(text.to_owned(), Color::rgb(0xd4, 0xd4, 0xd4))
    }

    #[test]
    fn wrap_respects_max_words() {
        let text = "слово ".repeat(23).trim().to_owned();
        let lines = wrap_words(&text, TOOLTIP_MAX_WORDS);
        assert_eq!(lines.len(), 3, "23 слова → 10/10/3");
        assert_eq!(lines[0].split_whitespace().count(), 10);
        assert_eq!(lines[1].split_whitespace().count(), 10);
        assert_eq!(lines[2].split_whitespace().count(), 3);
        // Порядок слов сохранён
        assert!(text.starts_with(&lines[0]));
    }

    #[test]
    fn wrap_keeps_explicit_newlines() {
        let lines = wrap_words("строка раз\nстрока два", TOOLTIP_MAX_WORDS);
        assert_eq!(lines, vec!["строка раз", "строка два"]);
    }

    /// Разряды чисел (NBSP-группы `format_num`) не переносятся по частям:
    /// NBSP — не точка разбиения, число — один токен.
    #[test]
    fn wrap_keeps_nbsp_digit_groups_whole() {
        let lines = wrap_words("итог 1\u{a0}234\u{a0}567 руб", 2);
        assert_eq!(
            lines,
            vec!["итог 1\u{a0}234\u{a0}567".to_owned(), "руб".to_owned()],
            "NBSP-число — один токен"
        );
        // Даже при переполнении строки число не рвётся посреди групп
        let lines = wrap_words("1\u{a0}000\u{a0}000 2\u{a0}000 3", 2);
        assert_eq!(
            lines,
            vec!["1\u{a0}000\u{a0}000 2\u{a0}000".to_owned(), "3".to_owned()]
        );
    }

    #[test]
    fn wrap_long_unbreakable_token_stays_whole() {
        let token = "C:/Users/name/Документы/очень/длинный/путь/файла.txt";
        let lines = wrap_words(token, TOOLTIP_MAX_WORDS);
        assert_eq!(lines, vec![token], "неразрывный токен — целая строка");
    }

    #[test]
    fn wrap_empty_text_is_empty() {
        assert!(wrap_words("", TOOLTIP_MAX_WORDS).is_empty());
        assert!(wrap_words("   \n  ", TOOLTIP_MAX_WORDS).is_empty());
    }

    #[test]
    fn fill_is_forced_opaque() {
        let (quads, texts) = layout_tooltips(
            vec![card("непрозрачность подложки")],
            [100.0, 100.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].fill[3], 1.0, "альфа подложки форсируется в 1.0");
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn box_hugs_text_and_lines_stack() {
        let text = "первые десять слов переносятся на следующую строку тултипа ок ещё слово";
        let (quads, texts) = layout_tooltips(
            vec![card(text)],
            [100.0, 100.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        let lines = wrap_words(text, TOOLTIP_MAX_WORDS);
        assert_eq!(texts.len(), lines.len(), "строка текста на строку рендера");
        // Строки внутри бокса с шагом TOOLTIP_LINE_STEP
        for (row, t) in texts.iter().enumerate() {
            let expected_y = quads[0].pos[1] + TOOLTIP_PAD_Y + row as f32 * TOOLTIP_LINE_STEP;
            assert!((t.origin[1] - expected_y).abs() < 1e-3);
            assert!(t.origin[0] >= quads[0].pos[0]);
            assert!(
                t.origin[0] + t.width <= quads[0].pos[0] + quads[0].size[0] + 1e-3,
                "клип строки — правый край бокса"
            );
        }
        // Бокс по ширине не меньше текста + паддинги
        assert!(quads[0].size[0] >= 2.0 * TOOLTIP_PAD_X + 1.0);
    }

    #[test]
    fn right_edge_clamped_to_viewport() {
        let (quads, _) = layout_tooltips(
            vec![card("тултип у самого правого края окна канваса")],
            [790.0, 100.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        assert!(
            quads[0].pos[0] + quads[0].size[0] <= 800.0 + 1e-3,
            "правый край бокса не покидает окно"
        );
        assert!(quads[0].pos[0] >= 0.0);
    }

    #[test]
    fn bottom_edge_flips_above_cursor() {
        let text = "тултип у нижнего края окна\nвторая строка";
        let (quads, _) = layout_tooltips(
            vec![card(text)],
            [100.0, 580.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        assert!(
            quads[0].pos[1] + quads[0].size[1] <= 600.0 + 1e-3,
            "нижний край бокса не покидает окно"
        );
        assert!(
            quads[0].pos[1] < 580.0,
            "бокс развёрнут НАД курсором (top выше cursor_y)"
        );
    }

    #[test]
    fn two_cards_stack_without_overlap() {
        let (quads, _) = layout_tooltips(
            vec![
                card("первая карточка тултипа"),
                card("вторая карточка тултипа"),
            ],
            [100.0, 100.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 2, "два тултипа — две подложки");
        let [first, second] = [quads[0], quads[1]];
        assert!(
            second.pos[1] >= first.pos[1] + first.size[1],
            "вторая подложка — ниже первой (без наложения, зазор ≥ 0)"
        );
    }
}
