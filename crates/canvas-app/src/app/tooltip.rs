//! Тултипы живого канваса: НЕПРОЗРАЧНАЯ подложка + перенос строк
//! (правка владельца 2026-09-26; W-c — перенос по ширине, kit-геометрия).
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
//! 2. Перенос строк — измеренный (W-c, дефект B8 аудита ui-kit:
//!    прежде — по ЧИСЛУ СЛОВ `wrap_words(text, 10)`, теперь — по ШИРИНЕ
//!    через кит-замер `TextMeasurer`): строка набирается, пока кандидат
//!    «строка + пробел + слово» укладывается в бюджет
//!    [`TOOLTIP_MAX_TEXT_W`] — семантика kit `TextMeasurer::wrap`
//!    (жадный перенос по измеренной ширине; слово шире бюджета — целой
//!    строкой, без разрыва по глифам). Неразрывный длинный токен (путь
//!    файла) больше НЕ клипается молча: бокс растёт по измеренной
//!    ширине строки, клампится только физической шириной окна.
//!    Расширения против kit `TextMeasurer::wrap` (его сигнатура —
//!    `split_whitespace`): явные `\n` — параграфы переносятся независимо
//!    (кит-обёртка схлопывает `\n`), и NBSP U+00A0 — не точка разбиения
//!    (разряды чисел `format_num` в canvas-core не рвутся;
//!    `char::is_whitespace` считает U+00A0 пробелом). Замер строк —
//!    кит-`TextMeasurer` (F-6): те же метрики, что у шейпинга рендера.
//! 3. Ширина подложки — **измеренный максимум строк**, кламп по правому
//!    краю окна; по вертикали — флип НАД курсором у нижнего края (§6
//!    use-case), затем кламп. Геометрия/позиционирование — kit
//!    [`kit::tooltip`] (якорь-курсор + `kit::TOOLTIP_OFFSET` + flip +
//!    клампы — контекст FR-068 W0): kit-раскладка применяется к СТЕКУ
//!    карточек как единому popup-блоку, карточки кладутся внутрь стека
//!    сверху вниз с зазором [`TOOLTIP_STACK_GAP`] — стек без наложений
//!    по построению (тултипы — passive-поверхность, hit-тестов нет).
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
use canvas_ui::geometry::{UiPoint, UiRect, UiVec2};
use canvas_ui::kit;
use canvas_ui::measure::{TextMeasurer, SCREEN_LINE_FACTOR};

use super::{band_rect_quad_pub, OwnedScreenText};

/// Кегль тултипа (лог. px) — прежний (правки не меняют кегль).
pub(crate) const TOOLTIP_FONT: f32 = 13.0;

/// Бюджет переноса строк (лог. px) — прежний клип 380 px (use-case §3),
/// снятый правкой 2026-09-26 как КЛИП; теперь — ширина жадного переноса
/// (не клип): неразрывный токен длиннее бюджета даёт ЦЕЛУЮ строку, бокс
/// растёт по измеренной ширине (дефект B8 «10 длинных слов шире окна →
/// текст молча клипается» устранён).
pub(crate) const TOOLTIP_MAX_TEXT_W: f32 = 380.0;

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
/// `chunks` — строки-чанки, КАЖДЫЙ переносится независимо (по ширине
/// [`TOOLTIP_MAX_TEXT_W`]) со своим цветом: одиночный тултип — один
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

/// Перенос текста по ШИРИНЕ (W-c, дефект B8) — семантика kit
/// `TextMeasurer::wrap` (жадный перенос по измеренной ширине), с двумя
/// расширениями потребителя (см. модульную доку):
///
/// - явные `\n` уважаются — параграфы переносятся независимо;
/// - слово — токен по whitespace (CSS-подобная семантика), КРОМЕ
///   неразрывного пробела U+00A0: им группируются разряды чисел
///   (`1 234 567`, `format_num` в canvas-core) — NBSP-токен не рвётся.
///
/// Ширины строк даёт `width_of` (кит-замер `TextMeasurer` — F-6, те же
/// метрики, что у рендера; в тестах — детерминированный мок).
/// Строка набирается, пока кандидат «строка + пробел + слово»
/// укладывается в `max_w`, иначе закрывается; токен шире `max_w` —
/// отдельной строкой (без разрыва по глифам — контракт kit `wrap`).
/// Пустой текст → пустой вектор (карточка без строк не рисуется).
pub(crate) fn wrap_width(
    text: &str,
    max_w: f32,
    width_of: &mut dyn FnMut(&str) -> f32,
) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para
            .split(|c: char| c.is_whitespace() && c != '\u{a0}')
            .filter(|word| !word.is_empty())
        {
            if line.is_empty() {
                line.push_str(word);
                continue;
            }
            let candidate = format!("{line} {word}");
            if width_of(&candidate) <= max_w {
                line = candidate;
            } else {
                out.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// Одна перенесённая карточка стека: строки с цветами + измеренный бокс
/// (приватная — только раскладка стека [`layout_tooltips`]).
struct StackCard {
    lines: Vec<(String, Color)>,
    w: f32,
    h: f32,
}

/// Собрать квады подложек и тексты тултипов кадра (screen-space, полоса
/// Popups). Геометрия — kit [`kit::tooltip`] (якорь — курсор, смещение
/// `kit::TOOLTIP_OFFSET`, flip у нижнего края, клампы вьюпорта),
/// применённая к СТЕКУ карточек как единому блоку; карточки стекаются
/// внутри блока сверху вниз с зазором [`TOOLTIP_STACK_GAP`].
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
    let mut width_of = |s: &str| measurer.width_of(&mut fs, s, SANS_FAMILY, phys_font) / scale;
    // Бюджет переноса: константа use-case, но не шире окна минус паддинги
    // (многословные строки всегда помещаются в подложку даже до flip).
    let max_text_w = TOOLTIP_MAX_TEXT_W.min((viewport[0] - 2.0 * TOOLTIP_PAD_X).max(40.0));
    // Карточки: перенос по ширине + измеренные боксы.
    let mut boxes: Vec<StackCard> = Vec::new();
    for card in &cards {
        let mut lines: Vec<(String, Color)> = Vec::new();
        for (text, color) in &card.chunks {
            for line in wrap_width(text, max_text_w, &mut width_of) {
                lines.push((line, *color));
            }
        }
        if lines.is_empty() {
            continue;
        }
        // Ширина бокса — измеренный максимум строк + паддинги; кламп
        // только по физической ширине окна (вырожденный неразрывный
        // токен шире ОКНА клипается краем бокса — обычный длинный путь
        // теперь даёт строку/бокс по своей измеренной ширине, B8).
        let mut text_w = 0.0f32;
        for (line, _) in &lines {
            text_w = text_w.max(width_of(line));
        }
        let box_w = (text_w + 2.0 * TOOLTIP_PAD_X)
            .ceil()
            .min(viewport[0].max(2.0 * TOOLTIP_PAD_X));
        let box_h = 2.0 * TOOLTIP_PAD_Y + lines.len() as f32 * TOOLTIP_LINE_STEP;
        boxes.push(StackCard {
            lines,
            w: box_w,
            h: box_h,
        });
    }
    if boxes.is_empty() {
        return (quads, texts);
    }
    // Стек — единый popup-блок: kit-геометрия тултипа (offset от курсора,
    // flip у нижнего края, position-clamp вьюпорта). Задержка показа
    // (`TOOLTIP_DELAY_MS`) отфильтрована потребителем (карточки строятся
    // только для активных тултипов) — раскладке блок всегда «активен».
    let total_h =
        boxes.iter().map(|b| b.h).sum::<f32>() + TOOLTIP_STACK_GAP * (boxes.len() - 1) as f32;
    let stack_w = boxes.iter().map(|b| b.w).fold(0.0f32, f32::max);
    let Some(layout) = kit::tooltip(
        UiPoint::new(cursor[0], cursor[1]),
        UiVec2::new(stack_w, total_h),
        UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
        kit::TOOLTIP_DELAY_MS,
        kit::TOOLTIP_DELAY_MS,
    ) else {
        return (quads, texts);
    };
    let mut top = layout.rect.y;
    for StackCard {
        lines,
        w: box_w,
        h: box_h,
    } in boxes
    {
        let x = layout.rect.x;
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
        top += box_h + TOOLTIP_STACK_GAP;
    }
    (quads, texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(text: &str) -> TooltipCard {
        TooltipCard::single(text.to_owned(), Color::rgb(0xd4, 0xd4, 0xd4))
    }

    /// Детерминированный мок замера: 10 лог. px на символ (включая NBSP).
    /// Перенос тестируется как АЛГОРИТМ; паритет метрик рендера —
    /// контракт кит-`TextMeasurer` (тесты canvas-ui/measure).
    fn mock_width() -> impl FnMut(&str) -> f32 {
        |s: &str| s.chars().count() as f32 * 10.0
    }

    /// W-c (B8): перенос — по ШИРИНЕ, не по числу слов: бюджет 100 px
    /// (мок 10/симв.) — 5 однобуквенных слов в строке, не «10 слов».
    #[test]
    fn wrap_width_respects_width_not_word_count() {
        let text = "а б в г д е ё ж з и";
        let lines = wrap_width(text, 100.0, &mut mock_width());
        assert_eq!(
            lines,
            vec!["а б в г д".to_owned(), "е ё ж з и".to_owned()],
            "строка — пока измеренная ширина ≤ бюджета"
        );
        // Каждая многословная строка укладывается в бюджет
        for line in &lines {
            assert!(mock_width()(line) <= 100.0);
        }
    }

    /// W-c, главная цель фикса: длинный неразрывный токен (путь) НЕ
    /// теряется — целой строкой шире бюджета, текст не клипается молча.
    #[test]
    fn wrap_width_long_unbreakable_token_not_lost() {
        let token = "C:/Users/name/Документы/очень/длинный/путь/без-пробелов/report-2026.txt";
        let text = format!("ошибка чтения {token} повторите");
        let lines = wrap_width(&text, 100.0, &mut mock_width());
        // Токен — целая строка (без разрыва по глифам)
        assert!(
            lines.iter().any(|l| l == token),
            "неразрывный токен не рвётся и не теряется: {lines:?}"
        );
        // Ни один токен не пропал (конкатенация строк восстанавливает текст)
        let mut recovered: Vec<&str> = lines
            .iter()
            .flat_map(|l| l.split(|c: char| c.is_whitespace() && c != '\u{a0}'))
            .collect();
        let mut expected: Vec<&str> = text
            .split(|c: char| c.is_whitespace() && c != '\u{a0}')
            .filter(|w| !w.is_empty())
            .collect();
        recovered.sort_unstable();
        expected.sort_unstable();
        assert_eq!(recovered, expected, "все слова текста сохранены");
    }

    /// Разряды чисел (NBSP-группы `format_num`) не переносятся по частям:
    /// NBSP — не точка разбиения, число — один токен.
    #[test]
    fn wrap_width_keeps_nbsp_digit_groups_whole() {
        let lines = wrap_width("итог 1\u{a0}234\u{a0}567 руб", 100.0, &mut mock_width());
        assert_eq!(
            lines,
            vec![
                "итог".to_owned(),
                "1\u{a0}234\u{a0}567".to_owned(),
                "руб".to_owned()
            ],
            "NBSP-число — один токен"
        );
        // Даже при переполнении строки число не рвётся посреди групп
        let lines = wrap_width("1\u{a0}000\u{a0}000 2\u{a0}000 3", 100.0, &mut mock_width());
        assert_eq!(
            lines,
            vec!["1\u{a0}000\u{a0}000".to_owned(), "2\u{a0}000 3".to_owned()]
        );
    }

    #[test]
    fn wrap_width_keeps_explicit_newlines() {
        let lines = wrap_width("строка раз\nстрока два", 100.0, &mut mock_width());
        assert_eq!(lines, vec!["строка раз", "строка два"]);
    }

    #[test]
    fn wrap_width_empty_text_is_empty() {
        assert!(wrap_width("", 100.0, &mut mock_width()).is_empty());
        assert!(wrap_width("   \n  ", 100.0, &mut mock_width()).is_empty());
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
        // kit-геометрия: смещение от курсора — kit::TOOLTIP_OFFSET
        assert!((quads[0].pos[0] - (100.0 + kit::TOOLTIP_OFFSET.x)).abs() < 0.01);
        assert!((quads[0].pos[1] - (100.0 + kit::TOOLTIP_OFFSET.y)).abs() < 0.01);
    }

    /// W-c: строки — из переноса ПО ШИРИНЕ (тот же кит-замер, что и
    /// раскладка): строка текста на строку рендера, бокс по измеренной
    /// ширине, клип строки — правый край бокса.
    #[test]
    fn box_hugs_text_and_lines_stack() {
        let text =
            "первые десять слов переносятся по измеренной ширине строки тултипа ок ещё слово";
        let (quads, texts) = layout_tooltips(
            vec![card(text)],
            [100.0, 100.0],
            [800.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        // Строки — тот же кит-замер (TextMeasurer), что и в раскладке
        let mut m = TextMeasurer::new();
        let mut fs = measure_font_system();
        let lines = wrap_width(text, TOOLTIP_MAX_TEXT_W, &mut |s: &str| {
            m.width_of(&mut fs, s, SANS_FAMILY, TOOLTIP_FONT)
        });
        assert!(lines.len() >= 2, "текст шире бюджета — переносился");
        assert_eq!(texts.len(), lines.len(), "строка текста на строку рендера");
        for (row, t) in texts.iter().enumerate() {
            let expected_y = quads[0].pos[1] + TOOLTIP_PAD_Y + row as f32 * TOOLTIP_LINE_STEP;
            assert!((t.origin[1] - expected_y).abs() < 1e-3);
            assert!(t.origin[0] >= quads[0].pos[0]);
            assert!(
                t.origin[0] + t.width <= quads[0].pos[0] + quads[0].size[0] + 1e-3,
                "клип строки — правый край бокса"
            );
        }
        // Бокс по ширине — измеренный максимум строк + паддинги (не жёсткий клип)
        let max_line_w = lines
            .iter()
            .map(|l| m.width_of(&mut fs, l, SANS_FAMILY, TOOLTIP_FONT))
            .fold(0.0f32, f32::max);
        assert!((quads[0].size[0] - (max_line_w + 2.0 * TOOLTIP_PAD_X).ceil()).abs() < 1.0);
    }

    /// W-c, интеграционный случай B8: неразрывный путь ШИРЕ бюджета —
    /// бокс растёт до измеренной ширины строки (текст виден целиком),
    /// кламп только по ширине окна.
    #[test]
    fn long_token_box_grows_no_silent_clip() {
        let token = "C:/Users/name/Документы/очень/длинный/путь/без-пробелов/report-2026-final.txt";
        let (quads, texts) = layout_tooltips(
            vec![card(&format!("ошибка чтения {token}"))],
            [100.0, 100.0],
            [900.0, 600.0],
            1.0,
            [0.11, 0.11, 0.13, 0.97],
            [0.22, 0.24, 0.30, 0.9],
        );
        assert_eq!(quads.len(), 1);
        // Премиса: токен реально шире бюджета переноса
        let mut m = TextMeasurer::new();
        let mut fs = measure_font_system();
        let tok_w = m.width_of(&mut fs, token, SANS_FAMILY, TOOLTIP_FONT);
        assert!(
            tok_w > TOOLTIP_MAX_TEXT_W,
            "премиса: путь шире бюджета ({tok_w})"
        );
        // Бокс — по измеренной ширине строки (кламп бюджета НЕ жёсткий)
        assert!(
            (quads[0].size[0] - (tok_w + 2.0 * TOOLTIP_PAD_X).ceil()).abs() < 1.0,
            "бокс растёт до измеренной ширины неразрывного токена"
        );
        // Текст не потерян: токен присутствует в строках рендера целиком
        assert!(
            texts.iter().any(|t| t.text == token),
            "неразрывный токен не клипается молча"
        );
        // Клип каждой строки ≥ измеренной ширины строки (ничто не режется)
        for t in &texts {
            let w = m.width_of(&mut fs, &t.text, SANS_FAMILY, TOOLTIP_FONT);
            assert!(
                t.width >= w - 0.5,
                "клип строки ({} ≥ {w}) не режет текст",
                t.width
            );
        }
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
            "правый край бокса не покидает окно (kit: перенос влево от якоря)"
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
            "бокс развёрнут НАД курсором (top выше cursor_y, kit-flip)"
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
