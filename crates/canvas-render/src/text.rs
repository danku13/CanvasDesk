//! Текст на канвасе через glyphon (T4): один TextAtlas на сцену.
//!
//! Шрифты (SIL OFL 1.1) встроены в бинарь из `assets/fonts` — кириллица
//! поддерживается (CR-009): Noto Sans Display — Medium 500 (базовый текст) и
//! Bold 700 (акценты), Noto Sans Mono — Numi-строки, результаты и код-фенсы.
//! Семейства задаются ЯВНО (Family::Name) — дефолтный Family::Sans в
//! cosmic-text резолвится в отсутствующий «Fira Sans» и текст рендерился
//! системным fallback'ом, а не встроенным шрифтом.
//!
//! Производительность (T5): Buffer'ы заголовков кэшируются по ноде — шейпинг
//! (самая дорогая операция) повторяется только при смене текста, зума или ширины.
//! Позиция передаётся в TextArea покадрово, поэтому панорамирование кэш не ломает.

use std::collections::HashMap;

use canvas_core::{Canvas, Node, NodeKind};
use glyphon::{
    Attrs, Buffer, Cache, Color, Cursor, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight, Wrap,
};

use crate::camera::Camera;
use crate::cards::{dim_color, extension_letter, title_for, FocusView, HEADER_HEIGHT};
use crate::gfm;
use crate::markdown;
use crate::theme::ThemeColors;
use crate::zorder::ZPlan;
use canvas_core::expr::{ExprLineResults, ExprOutcome, ExprResults};

/// Встроенные шрифты (SIL OFL 1.1 — см. assets/fonts/OFL-NotoSans*.txt).
/// СТАТИЧЕСКИЕ инстансы (CR-009): cosmic-text 0.12 не инстанцирует вариации
/// вариативных шрифтов (сваш рендерит дефолт-инстанс), поэтому веса 500/700
/// обязаны быть отдельными файлами: fontdb выбирает лицо по весу из OS/2.
const FONT_DATA: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansDisplay-Bold.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansMono-Bold.ttf"),
];

/// Семейство базового текста канваса (CR-009): Noto Sans Display.
pub(crate) const SANS_FAMILY: &str = "Noto Sans Display";
/// Семейство Numi-строк, результатов и код-фенсов (CR-009): Noto Sans Mono.
pub(crate) const MONO_FAMILY: &str = "Noto Sans Mono";

/// Базовые атрибуты текста канваса (CR-009): Noto Sans Display Medium 500.
/// Явное Family::Name — иначе Family::Sans уходит в несуществующий «Fira Sans»
/// и рендерится системным fallback'ом (встроенный шрифт не используется).
pub(crate) fn sans_attrs() -> Attrs<'static> {
    Attrs::new()
        .family(Family::Name(SANS_FAMILY))
        .weight(Weight::MEDIUM)
}

/// Атрибуты Numi-строк, результатов и код-фенсов (CR-009): Noto Sans Mono
/// Regular 400. Жирные спаны внутри моно-блоков получают Weight::BOLD —
/// шрифт остаётся моно (вшито лицо Bold 700).
pub(crate) fn mono_attrs() -> Attrs<'static> {
    Attrs::new().family(Family::Name(MONO_FAMILY))
}

/// Размер заголовка в world-px (масштабируется зумом).
const TITLE_FONT_SIZE: f32 = 13.0;
/// Высота строки заголовка.
const TITLE_LINE_HEIGHT: f32 = 18.0;
/// Левый отступ заголовка в world-px (без иконки).
const TITLE_PADDING: f32 = 8.0;
/// Ширина зоны иконки-заглушки в world-px.
const ICON_WIDTH: f32 = 22.0;
/// Минимальный физический размер заголовка: ниже текст нечитаем — не готовим
/// (LOD-порог, уточняется в T11 по SPEC §6.2).
const MIN_TITLE_PX: f32 = 4.0;

/// Размер тела заметки в world-px (T7).
pub const BODY_FONT_SIZE: f32 = 14.0;
/// Высота строки тела заметки.
pub const BODY_LINE_HEIGHT: f32 = 20.0;
/// Внутренний отступ тела заметки по горизонтали и снизу в world-px.
pub const BODY_PADDING: f32 = 10.0;
/// Зазор между заголовком и телом заметки в world-px.
pub const BODY_TOP_GAP: f32 = 4.0;

/// Размер шрифта лейбла связи в world-px (T8).
const EDGE_LABEL_FONT_SIZE: f32 = 12.0;
/// Высота строки лейбла связи.
const EDGE_LABEL_LINE_HEIGHT: f32 = 16.0;

/// Размер шрифта строки результата формулы в world-px (FR-013).
const RESULT_FONT_SIZE: f32 = 12.0;
/// Высота строки результата формулы в world-px (FR-013) — резерв футера
/// карточки; приложение учитывает в fit_note_size.
pub const RESULT_LINE_HEIGHT: f32 = 16.0;
/// Цвет строки результата с ОШИБКОЙ (парсинг/вычисление) — красный акцент
/// (FR-013: «красная строка с тултипом»; согласован с рамкой битой ссылки).
const RESULT_ERROR_COLOR: Color = Color::rgb(0xe5, 0x5c, 0x5c);
/// FR-013 (правка 4): текст бейджа ошибки формульной строки — компактный
/// красный маркер у правого края СВОЕЙ строки; подробности — в тултипе
/// при наведении (длинные сообщения не влезают в строку ноды).
const LINE_ERROR_BADGE: &str = "!";
/// FR-013 (правка 4): расширение зоны наведения бейджа ошибки в логических
/// px в каждую сторону — один глиф «!» слишком мал для точного попадания
/// курсора.
const LINE_ERROR_HIT_PAD_PX: f32 = 10.0;

/// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки —
/// логические px окна (x, y, w, h) и текст ошибки для тултипа. Собирается
/// при подготовке текста кадра, вычитывается приложением после рендера
/// (hit-тест курсора → тултип, паттерн тултипа битой ссылки T10).
#[derive(Debug, Clone)]
pub struct LineErrorHit {
    pub rect: [f32; 4],
    pub message: String,
}
/// Размер шрифта бейджа «=» calc-ноды при дальнем зуме (FR-013) —
/// физические px (не масштабируется зумом, как HUD).
const BADGE_FONT_SIZE: f32 = 10.0;
/// Высота строки бейджа «=» в физических px.
const BADGE_LINE_HEIGHT: f32 = 12.0;

/// Размер шрифта HUD в физических px (не масштабируется зумом).
const HUD_FONT_SIZE: f32 = 14.0;
/// Высота строки HUD.
const HUD_LINE_HEIGHT: f32 = 18.0;
/// Отступ HUD от угла экрана в физических px.
const HUD_PADDING: f32 = 12.0;
/// Цвет HUD — акцентный (тот же, что рамка выделения).
const HUD_COLOR: Color = Color::rgb(0x65, 0x9c, 0xf8);

/// Как часто чистить кэш заголовков от давно невидимых нод (в кадрах).
const CACHE_SWEEP_INTERVAL: u64 = 128;
/// Записи старше этого возраста (в кадрах) вытесняются при чистке.
const CACHE_MAX_AGE: u64 = 600;

/// Снап экранной позиции к целым физическим пикселям: без него дробные
/// позиции при панораме порождают новый subpixel-бин глифа каждый кадр
/// (cosmic-text SubpixelBin — до 16 вариантов на глиф) и раздувают атлас.
fn snap_to_pixel(screen: [f32; 2], scale_factor: f32) -> [f32; 2] {
    [
        (screen[0] * scale_factor).round(),
        (screen[1] * scale_factor).round(),
    ]
}

/// Подготовить текст-группу с одним повтором после trim атласа: при
/// AtlasFull trim() освобождает место — повтор почти всегда успешен;
/// повторная ошибка уходит вызывающему (рендерер логирует и рисует stale).
#[allow(clippy::too_many_arguments)]
fn prepare_group<'a>(
    renderer: &mut TextRenderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    font_system: &mut FontSystem,
    atlas: &mut TextAtlas,
    viewport: &Viewport,
    areas: &[TextArea<'a>],
    swash_cache: &mut SwashCache,
) -> Result<(), glyphon::PrepareError> {
    let result = renderer.prepare(
        device,
        queue,
        font_system,
        atlas,
        viewport,
        areas.iter().cloned(),
        swash_cache,
    );
    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            tracing::warn!(
                ?err,
                "prepare текст-группы не удался — trim атласа и повтор"
            );
            atlas.trim();
            renderer.prepare(
                device,
                queue,
                font_system,
                atlas,
                viewport,
                areas.iter().cloned(),
                swash_cache,
            )
        }
    }
}

/// Заголовок ноды читаем только если он крупнее MIN_TITLE_PX физических px.
pub fn titles_visible(zoom_px: f32) -> bool {
    TITLE_FONT_SIZE * zoom_px >= MIN_TITLE_PX
}

/// Тело рисуется только у текстовых нод с непустым текстом и при читаемом зуме (T7).
fn body_visible(node: &Node, zoom_px: f32) -> bool {
    node.kind() == NodeKind::Text
        && node.text.as_deref().is_some_and(|text| !text.is_empty())
        && titles_visible(zoom_px)
}

/// Область тела заметки: world-координаты левого верхнего угла и (ширина, высота).
pub fn body_area(node: &Node) -> ([f32; 2], f32, f32) {
    let origin = [node.x + BODY_PADDING, node.y + HEADER_HEIGHT + BODY_TOP_GAP];
    let width = (node.width - BODY_PADDING * 2.0).max(0.0);
    let height = (node.height - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING).max(0.0);
    (origin, width, height)
}

/// Байтовый offset в тексте → курсор (строка, байтовый индекс в строке).
/// Offset за концом текста клампится в конец последней строки.
pub fn offset_to_cursor(text: &str, offset: usize) -> Cursor {
    let mut rest = offset.min(text.len());
    for (line_i, line) in text.split('\n').enumerate() {
        if rest <= line.len() {
            return Cursor::new(line_i, rest);
        }
        rest -= line.len() + 1;
    }
    let last = text.split('\n').count().saturating_sub(1);
    let last_len = text.rsplit('\n').next().map(str::len).unwrap_or(0);
    Cursor::new(last, last_len)
}

/// Спаны стилей (markdown.rs) → непрерывное покрытие текста парами
/// (&str, Attrs) для set_rich_text: bold → Weight::BOLD, italic → Style::Italic.
/// Highlight/strike здесь не применяются — это фон-подложка (квады, gfm.rs).
/// `base` — базовые атрибуты блока (моноширинный фенс, жирный заголовок).
pub(crate) fn rich_spans<'a, 'r>(
    plain: &'a str,
    spans: &[markdown::StyleSpan],
    base: Attrs<'r>,
) -> Vec<(&'a str, Attrs<'r>)> {
    let mut out = Vec::with_capacity(spans.len() * 2 + 1);
    let mut pos = 0usize;
    for span in spans {
        if span.start > pos {
            out.push((&plain[pos..span.start], base));
        }
        let mut attrs = base;
        if span.bold {
            attrs = attrs.weight(Weight::BOLD);
        }
        if span.italic {
            attrs = attrs.style(Style::Italic);
        }
        out.push((&plain[span.start..span.end], attrs));
        pos = span.end;
    }
    if pos < plain.len() {
        out.push((&plain[pos..], base));
    }
    // Пустой текст (например, "****" без контента) — один пустой спан
    if out.is_empty() {
        out.push(("", base));
    }
    out
}

/// Декоративные квады текстового блока (подсветка `==…==` и зачёркивание
/// `~~…~~`) в px буфера блока: диапазоны спанов → квады по layout runs
/// (та же механика, что у выделения в редакторе, edit.rs).
fn decoration_quads(
    buffer: &Buffer,
    plain: &str,
    spans: &[markdown::StyleSpan],
    zoom_px: f32,
) -> Vec<([f32; 4], BodyQuadKind)> {
    let mut quads = Vec::new();
    for span in spans.iter().filter(|s| s.highlight || s.strike) {
        let start = offset_to_cursor(plain, span.start);
        let end = offset_to_cursor(plain, span.end);
        for run in buffer.layout_runs() {
            if let Some((x, width)) = run.highlight(start, end) {
                if span.highlight {
                    quads.push((
                        [x, run.line_top, width.max(1.0), run.line_height],
                        BodyQuadKind::Highlight,
                    ));
                }
                if span.strike {
                    // Зачёркивание — тонкая линия чуть ниже середины строки
                    let y = run.line_top + run.line_height * 0.55;
                    quads.push((
                        [x, y, width.max(1.0), (zoom_px * 1.2).max(1.0)],
                        BodyQuadKind::Strike,
                    ));
                }
            }
        }
    }
    quads
}

/// Вид декоративного квада тела заметки (GFM) — рендерер маппит его на
/// заливку темы (HIGHLIGHT_FILL / gfm_muted_fill / gfm_quote_fill / …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyQuadKind {
    /// Фон-подсветка `==…==` (форматирование, как раньше).
    Highlight,
    /// Линия зачёркивания `~~…~~`.
    Strike,
    /// Маркер пункта маркированного списка.
    Bullet,
    /// Рамка чекбокса `[ ]` / `[x]`.
    CheckboxBox,
    /// Галочка чекбокса `[x]` (тонкие квады).
    CheckboxTick,
    /// Бар цитаты `> …` у левого края.
    QuoteBar,
    /// Фон фенса кода.
    CodeBg,
    /// Горизонтальная линия `---`.
    Rule,
}

/// Декоративный квад тела заметки: rect — [x, y, w, h] в px виртуального
/// буфера всего тела (block-local px + offset блока, умноженный на zoom).
#[derive(Debug, Clone, Copy)]
pub struct BodyQuad {
    pub rect: [f32; 4],
    pub kind: BodyQuadKind,
}

/// Один отрисованный блок тела заметки (свой Buffer со своими метриками).
struct BodyBlock {
    buffer: Buffer,
    /// Смещение левого верхнего угла блока от левого верхнего угла области
    /// тела (world-px).
    offset: [f32; 2],
    /// Ширина области блока (world-px) — для bounds.
    width: f32,
    /// Высота блока (world-px), по layout_runs.
    height: f32,
    /// Цвет текста блока (цитата/код приглушены/акцентные).
    color: Color,
    /// FR-013 (правка 2): строка исходного текста — у блоков формульных
    /// строк (None — обычный блок из сплошного сегмента).
    source_line: Option<usize>,
}

/// Отрисованное тело заметки: вертикальный стек блоков (GFM).
struct BodyLayout {
    /// Блоки по порядку сверху вниз.
    blocks: Vec<BodyBlock>,
    /// Все декоративные квады блоков в px виртуального буфера тела —
    /// рендерер делит на zoom и кладёт от origin области тела.
    quads: Vec<BodyQuad>,
}

/// Вид декорации пункта списка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemDeco {
    None,
    Bullet,
    Checkbox(bool),
}

/// Плоское описание одного размещаемого элемента тела (блок или линия).
struct BodyItem {
    /// Зазор перед элементом (world-px), первый — 0.
    gap: f32,
    /// Горизонтальная линия (нет текста, высота 12).
    rule: bool,
    text: String,
    font_size: f32,
    line_height: f32,
    color: Color,
    /// Левый отступ текста блока (world-px).
    indent: f32,
    mono: bool,
    bold: bool,
    deco: ItemDeco,
    /// FR-013 (правка 2): индекс строки исходного текста — только у
    /// формульных строк (сегмент из одной строки): привязка результата
    /// Numi-стиля к своему ряду.
    source_line: Option<usize>,
}

/// Метрики заголовка по уровню ATX: 1–3 крупно, 4–6 как bold body.
fn heading_metrics(level: u8) -> (f32, f32) {
    match level {
        1 => (22.0, 28.0),
        2 => (19.0, 25.0),
        3 => (16.0, 22.0),
        _ => (BODY_FONT_SIZE, BODY_LINE_HEIGHT),
    }
}

/// Зазор перед элементом тела: после заголовка и вокруг линии — 8, между
/// пунктами одного списка — 2, иначе 6; первый элемент — без зазора.
fn body_gap(
    prev: Option<(bool, bool, Option<usize>)>,
    curr_heading: bool,
    curr_rule: bool,
    curr_list: Option<usize>,
) -> f32 {
    match prev {
        None => 0.0,
        Some((prev_heading, prev_rule, prev_list)) => {
            if prev_rule || curr_rule || prev_heading || curr_heading {
                8.0
            } else if let (Some(p), Some(c)) = (prev_list, curr_list) {
                if p == c {
                    2.0
                } else {
                    6.0
                }
            } else {
                6.0
            }
        }
    }
}

/// Добавить элемент тела: зазор перед ним считается по предыдущему блоку,
/// затем признак предыдущего обновляется (`kind` = (heading, rule, list)).
fn push_item(
    out: &mut Vec<BodyItem>,
    prev: &mut Option<(bool, bool, Option<usize>)>,
    kind: (bool, bool, Option<usize>),
    mut item: BodyItem,
) {
    item.gap = body_gap(*prev, kind.0, kind.1, kind.2);
    out.push(item);
    *prev = Some(kind);
}

/// Развернуть GFM-блоки в плоский список элементов тела с зазорами.
/// FR-013 (правка 2): формульные строки (formula_lines — индексы строк с
/// готовым результатом) становятся САМОСТОЯТЕЛЬНЫМИ абзацами — своя
/// строка-блок даёт точный Y-ряд для результата Numi-стиля. Остальные
/// строки — сплошные сегменты между формульными (структура GFM внутри
/// сегмента прежняя; формульные строки внутри код-фенса не выбираются —
/// eval_lines их пропускает, фенс не дробится).
fn body_items(theme: &ThemeColors, body_text: &str, formula_lines: &[usize]) -> Vec<BodyItem> {
    let mut out = Vec::new();
    let mut prev: Option<(bool, bool, Option<usize>)> = None;
    let mut list_id = 0usize;
    let lines: Vec<&str> = body_text.split('\n').collect();
    let mut segments: Vec<(usize, usize, Option<usize>)> = Vec::new();
    let mut seg_start = 0usize;
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && formula_lines.binary_search(&i).is_ok() {
            if seg_start < i {
                segments.push((seg_start, i, None));
            }
            segments.push((i, i + 1, Some(i)));
            seg_start = i + 1;
        }
    }
    if seg_start < lines.len() {
        segments.push((seg_start, lines.len(), None));
    }
    for (seg_start, seg_end, source_line) in segments {
        let seg_text = lines[seg_start..seg_end].join("\n");
        push_gfm_blocks(
            theme,
            &seg_text,
            source_line,
            &mut out,
            &mut prev,
            &mut list_id,
        );
    }
    out
}

/// Разобрать сегмент GFM-блоков и допушить элементы тела; `source_line`
/// проставляется элементам (Some — у однострочного сегмента формульной
/// строки).
fn push_gfm_blocks(
    theme: &ThemeColors,
    seg_text: &str,
    source_line: Option<usize>,
    out: &mut Vec<BodyItem>,
    prev: &mut Option<(bool, bool, Option<usize>)>,
    list_id: &mut usize,
) {
    for block in gfm::parse_blocks(seg_text) {
        match block {
            gfm::Block::Heading { level, text } => {
                let (font_size, line_height) = heading_metrics(level);
                push_item(
                    out,
                    prev,
                    (true, false, None),
                    BodyItem {
                        gap: 0.0,
                        rule: false,
                        text,
                        font_size,
                        line_height,
                        color: theme.body,
                        indent: 0.0,
                        mono: false,
                        bold: true,
                        deco: ItemDeco::None,
                        source_line,
                    },
                );
            }
            gfm::Block::Paragraph { text } => {
                push_item(
                    out,
                    prev,
                    (false, false, None),
                    BodyItem {
                        gap: 0.0,
                        rule: false,
                        text,
                        font_size: BODY_FONT_SIZE,
                        line_height: BODY_LINE_HEIGHT,
                        color: theme.body,
                        indent: 0.0,
                        // CR-009: сегмент формульной строки (source_line) —
                        // Numi-расчёт: моноширинное начертание. Проза — sans.
                        mono: source_line.is_some(),
                        bold: false,
                        deco: ItemDeco::None,
                        source_line,
                    },
                );
            }
            gfm::Block::Quote { text } => {
                push_item(
                    out,
                    prev,
                    (false, false, None),
                    BodyItem {
                        gap: 0.0,
                        rule: false,
                        text,
                        font_size: BODY_FONT_SIZE,
                        line_height: BODY_LINE_HEIGHT,
                        color: theme.quote,
                        indent: 10.0,
                        mono: false,
                        bold: false,
                        deco: ItemDeco::None,
                        source_line,
                    },
                );
            }
            gfm::Block::Code { text } => {
                push_item(
                    out,
                    prev,
                    (false, false, None),
                    BodyItem {
                        gap: 0.0,
                        rule: false,
                        text,
                        font_size: 13.0,
                        line_height: 18.0,
                        color: theme.code_text,
                        indent: 6.0,
                        mono: true,
                        bold: false,
                        deco: ItemDeco::None,
                        source_line,
                    },
                );
            }
            gfm::Block::Rule => {
                push_item(
                    out,
                    prev,
                    (false, true, None),
                    BodyItem {
                        gap: 0.0,
                        rule: true,
                        text: String::new(),
                        font_size: BODY_FONT_SIZE,
                        line_height: BODY_LINE_HEIGHT,
                        color: theme.body,
                        indent: 0.0,
                        mono: false,
                        bold: false,
                        deco: ItemDeco::None,
                        source_line,
                    },
                );
            }
            gfm::Block::List { ordered, items } => {
                *list_id += 1;
                for (n, item) in items.into_iter().enumerate() {
                    // Нумерация рендерится по порядку 1,2,3…; чекбокс вместо номера
                    let text = if ordered && item.checkbox.is_none() {
                        format!("{}. {}", n + 1, item.text)
                    } else {
                        item.text
                    };
                    let deco = match item.checkbox {
                        Some(checked) => ItemDeco::Checkbox(checked),
                        None if !ordered => ItemDeco::Bullet,
                        None => ItemDeco::None,
                    };
                    push_item(
                        out,
                        prev,
                        (false, false, Some(*list_id)),
                        BodyItem {
                            gap: 0.0,
                            rule: false,
                            text,
                            font_size: BODY_FONT_SIZE,
                            line_height: BODY_LINE_HEIGHT,
                            color: theme.body,
                            indent: 16.0,
                            mono: false,
                            bold: false,
                            deco,
                            source_line,
                        },
                    );
                }
            }
        }
    }
}

/// Зашейпить один текстовый блок тела: буфер с переносами по ширине области
/// блока (px), высота по layout_runs (px буфера). Квады подсветки/
/// зачёркивания — в px буфера блока; буллиты/чекбоксы позиционируются по
/// первой строке снаружи.
#[allow(clippy::too_many_arguments)]
fn shape_text_block(
    font_system: &mut FontSystem,
    theme: &ThemeColors,
    text: &str,
    font_size: f32,
    line_height: f32,
    width_px: f32,
    zoom_px: f32,
    base: Attrs,
) -> (Buffer, f32, Vec<([f32; 4], BodyQuadKind)>) {
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    // WordOrGlyph: перенос по словам; слишком длинное слово рвётся по глифам,
    // а не вылезает за карточку. Высота None — shape_until_scroll зашейпит
    // ВСЕ строки (scroll_end = бесконечность, buffer.rs cosmic-text).
    buffer.set_wrap(font_system, Wrap::WordOrGlyph);
    buffer.set_size(font_system, Some(width_px), None);
    // Инлайн-разбор: ссылки (gfm) → сегменты, маркеры (** * == ~~) → спаны.
    let mut plain = String::with_capacity(text.len());
    let mut spans_all: Vec<markdown::StyleSpan> = Vec::new();
    let mut rich: Vec<(String, Attrs)> = Vec::new();
    for seg in gfm::inline_segments(text) {
        let seg_attrs = if seg.link {
            // Ссылка: текст label акцентным цветом, URL не показываем
            base.color(theme.link)
        } else {
            base
        };
        let (seg_plain, seg_spans) = markdown::parse(&seg.text);
        let offset = plain.len();
        spans_all.extend(seg_spans.iter().map(|span| markdown::StyleSpan {
            start: span.start + offset,
            end: span.end + offset,
            ..*span
        }));
        plain.push_str(&seg_plain);
        rich.extend(
            rich_spans(&seg_plain, &seg_spans, seg_attrs)
                .into_iter()
                .map(|(s, attrs)| (s.to_owned(), attrs)),
        );
    }
    buffer.set_rich_text(
        font_system,
        rich.iter().map(|(s, attrs)| (s.as_str(), *attrs)),
        base,
        Shaping::Advanced,
    );
    buffer.shape_until_scroll(font_system, false);
    let height_px = buffer
        .layout_runs()
        .last()
        .map_or(0.0, |run| run.line_top + run.line_height);
    let quads = decoration_quads(&buffer, &plain, &spans_all, zoom_px);
    (buffer, height_px, quads)
}

/// Зашейпить тело заметки: GFM-блоки → вертикальный стек буферов со своими
/// метриками/цветами + декоративные квады (в px виртуального буфера тела).
/// `body_width` — world-px, `zoom_px` — физический зум. GPU не нужен —
/// функция тестируема с настоящим FontSystem.
fn shape_body(
    font_system: &mut FontSystem,
    theme: &ThemeColors,
    body_text: &str,
    body_width: f32,
    zoom_px: f32,
    formula_lines: &[usize],
) -> BodyLayout {
    let mut layout = BodyLayout {
        blocks: Vec::new(),
        quads: Vec::new(),
    };
    let mut cursor_y = 0.0f32; // world-px, верх текущего элемента
    for item in body_items(theme, body_text, formula_lines) {
        cursor_y += item.gap;
        if item.rule {
            // Линия: высота блока 12, квад толщиной 2 по центру
            layout.quads.push(BodyQuad {
                rect: [
                    0.0,
                    (cursor_y + 5.0) * zoom_px,
                    body_width * zoom_px,
                    2.0 * zoom_px,
                ],
                kind: BodyQuadKind::Rule,
            });
            cursor_y += 12.0;
            continue;
        }
        let block_width = (body_width - item.indent).max(0.0);
        // CR-009: базис посемейственно — Noto Sans Mono (Numi-строки, фенсы)
        // или Noto Sans Display Medium (прочее); жирные GFM-заголовки —
        // Weight::BOLD (700) того же семейства.
        let mut base = if item.mono {
            mono_attrs()
        } else {
            sans_attrs()
        };
        if item.bold {
            base = base.weight(Weight::BOLD);
        }
        let (buffer, height_px, quads) = shape_text_block(
            font_system,
            theme,
            &item.text,
            item.font_size * zoom_px,
            item.line_height * zoom_px,
            block_width * zoom_px,
            zoom_px,
            base,
        );
        let height = height_px / zoom_px;
        // Маркеры пункта (буллит/чекбокс) — в колонке-gutter СЛЕВА от текста:
        // x задаётся в px виртуального буфера ТЕЛА (0..16), без смещения
        // блока по x; y — на первой строке блока (block-local + offset блока)
        let first = buffer.layout_runs().next();
        let oy = cursor_y * zoom_px;
        match item.deco {
            ItemDeco::Bullet => {
                let line_top = first.as_ref().map_or(0.0, |run| run.line_top);
                let line_h = first
                    .as_ref()
                    .map_or(item.line_height * zoom_px, |run| run.line_height);
                let z = zoom_px;
                layout.quads.push(BodyQuad {
                    rect: [
                        4.0 * z,
                        line_top + line_h / 2.0 - 2.5 * z + oy,
                        5.0 * z,
                        5.0 * z,
                    ],
                    kind: BodyQuadKind::Bullet,
                });
            }
            ItemDeco::Checkbox(checked) => {
                let line_top = first.as_ref().map_or(0.0, |run| run.line_top);
                let z = zoom_px;
                let y = line_top + 2.0 * z + oy;
                layout.quads.push(BodyQuad {
                    rect: [0.0, y, 11.0 * z, 11.0 * z],
                    kind: BodyQuadKind::CheckboxBox,
                });
                if checked {
                    // Галочка: квады без вращения, аппроксимация двумя
                    // перпендикулярными тонкими полосками (v1)
                    let y0 = line_top + 2.0 * z + oy;
                    layout.quads.push(BodyQuad {
                        rect: [2.2 * z, y0 + 6.0 * z, 3.4 * z, 1.6 * z],
                        kind: BodyQuadKind::CheckboxTick,
                    });
                    layout.quads.push(BodyQuad {
                        rect: [4.6 * z, y0 + 3.0 * z, 1.6 * z, 4.4 * z],
                        kind: BodyQuadKind::CheckboxTick,
                    });
                }
            }
            ItemDeco::None => {}
        }
        // Квады строк блока (подсветка/зачёркивание) → px виртуального буфера
        // тела: block-local px + offset блока по обеим осям
        let ox = item.indent * zoom_px;
        layout
            .quads
            .extend(quads.into_iter().map(|(rect, kind)| BodyQuad {
                rect: [rect[0] + ox, rect[1] + oy, rect[2], rect[3]],
                kind,
            }));
        // Бар цитаты / фон фенса — на всю высоту блока
        if item.color == theme.quote {
            layout.quads.push(BodyQuad {
                rect: [0.0, oy, 3.0 * zoom_px, height_px],
                kind: BodyQuadKind::QuoteBar,
            });
        }
        if item.mono {
            layout.quads.push(BodyQuad {
                rect: [0.0, oy, body_width * zoom_px, height_px],
                kind: BodyQuadKind::CodeBg,
            });
        }
        layout.blocks.push(BodyBlock {
            buffer,
            offset: [item.indent, cursor_y],
            width: block_width,
            height,
            color: item.color,
            source_line: item.source_line,
        });
        cursor_y += height;
    }
    layout
}

/// Ключ свежести кэша текста ноды: зум, ширина заголовка, заголовок, тело
/// и результаты формул (программный итог + построчные, Numi-стиль).
#[derive(Debug, Clone, Copy)]
struct CacheKey<'a> {
    zoom: f32,
    width: f32,
    title: &'a str,
    body: &'a str,
    /// FR-013: сводка результатов (пустая — результатов нет).
    results: &'a str,
}

/// Запись кэша свежа, если зум, ширина, заголовок, тело и результаты
/// формул не изменились.
fn cache_fresh(entry: CacheKey, current: CacheKey) -> bool {
    (entry.zoom - current.zoom).abs() < 1e-3
        && (entry.width - current.width).abs() < 0.5
        && entry.title == current.title
        && entry.body == current.body
        && entry.results == current.results
}

/// Оверлей-текст в world-координатах (контекстное меню, T7): шейпится
/// покадрово без кэша — меню открыто редко.
pub struct OverlayText<'a> {
    pub text: &'a str,
    /// World-координаты левого верхнего угла.
    pub origin: [f32; 2],
    /// Ширина области в world-px (bounds клипа).
    pub width: f32,
}

/// Выравнивание screen-текста в области `width` (центровка иконок кнопок;
/// glyphon 0.6 / cosmic-text 0.11 не имеют set_align — центрируем по
/// измеренной ширине строки, см. `TextEngine::prepare`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    /// Левый край в `origin` (подписи панелей, поиск).
    Left,
    /// Строка по центру области [`ScreenText::width`].
    Center,
}

/// Оверлей-текст в screen-space (панель настроек): константный размер
/// при любом зуме, координаты — логические px от левого верхнего угла окна.
pub struct ScreenText<'a> {
    pub text: &'a str,
    /// Логические px от левого верхнего угла окна.
    pub origin: [f32; 2],
    /// Ширина области в логических px (bounds клипа и опора выравнивания).
    pub width: f32,
    /// Размер шрифта в логических px (умножается на scale_factor).
    pub font_size: f32,
    pub color: Color,
    /// Выравнивание строки в области `width`.
    pub align: TextAlign,
}

/// Лейбл связи для кадра (T8): текст по центру кривой, шейпится с кэшем
/// по id связи (перешейп при смене текста или зума).
/// `factor` — множитель яркости (T23: не-фокусные лейблы затемняются
/// вместе со своими связями; 1.0 — полная яркость).
#[derive(Debug, Clone, Copy)]
pub struct EdgeLabel<'a> {
    /// id связи — ключ кэша шейпинга.
    pub id: &'a str,
    pub text: &'a str,
    /// Центр лейбла в world-координатах (середина кривой, t = 0.5).
    pub center: [f32; 2],
    /// T23: множитель альфы текста лейбла (1.0 — не затемнён).
    pub factor: f32,
}

/// Параметры кадра для подготовки текста (группировка аргументов prepare_titles).
pub struct TitleFrame<'a> {
    pub camera: &'a Camera,
    /// Размер viewport в физических пикселях.
    pub viewport_physical: [u32; 2],
    pub scale_factor: f32,
    pub canvas: &'a Canvas,
    /// Индексы видимых нод — выдача spatial index по viewport (culling, T5).
    pub indices: &'a [usize],
    /// Строка HUD-оверлея (F3), None — без оверлея.
    pub hud: Option<&'a str>,
    /// Индекс редактируемой ноды (T7): её тело рисует EditingSession, из кэша
    /// тела и из выдачи она исключается.
    pub editing: Option<usize>,
    /// Буфер активной EditingSession (T7) и world-прямоугольник области:
    /// левый верхний угол, ширина, высота — текст редактора рисуется поверх
    /// карточки (тело ноды) или бокса по центру кривой (лейбл связи).
    pub editing_buffer: Option<(&'a Buffer, [f32; 2], f32, f32)>,
    /// Оверлей-тексты кадра (контекстное меню, T7).
    pub overlay_texts: &'a [OverlayText<'a>],
    /// Screen-space тексты (панель настроек): константный размер при зуме.
    pub screen_texts: &'a [ScreenText<'a>],
    /// Z-план кадра (zorder.rs): текст-группы — тексты нод рисуются
    /// сегментами между карточками, чтобы текст фоновой ноды не ложился
    /// поверх карточек переднего плана. Финальная группа — лейблы связей,
    /// подписи меню и HUD; screen-тексты панелей — в `TextSystem::overlay_group`.
    pub zplan: &'a ZPlan,
    /// Лейблы связей (T8): по центрам кривых; лейбл редактируемой связи
    /// сюда не передаётся — его рисует EditingSession. Рисуются в финальной
    /// группе — поверх карточек, до оверлеев меню и HUD.
    pub edge_labels: &'a [EdgeLabel<'a>],
    /// Режим фокуса (T23, brainstorm-focus): не-фокусные ноды затемняются
    /// (цвет заголовка/иконки/тела — альфа × dim_factor). EMPTY — выключен.
    pub focus: FocusView<'a>,
    /// CR-004 v1: индексы виджет-нод, чьи заголовки рисуются несмотря на
    /// прозрачный хром (hover/выделение); отсортирован. Остальные виджет-ноды
    /// заголовков не имеют вовсе (хром скрыт, имя — в поиске и меню).
    pub widget_title_reveal: &'a [usize],
    /// FR-011: бейджи «+N» свернутых нод: (индекс, число скрытых потомков).
    pub collapsed_counts: &'a [(usize, usize)],
    /// FR-013: результаты формул (`canvasdesk.expr`) по id нод — строка
    /// результата берётся отсюда (`ExprOutcome::Ok` — значение,
    /// `ExprOutcome::Err` — красная диагностика).
    pub expr_results: &'a ExprResults,
    /// FR-013: построчные результаты формул (Numi-стиль).
    pub expr_line_results: &'a ExprLineResults,
    /// FR-013 (правка 4): живые построчные результаты редактируемой ноды
    /// (вычисляются приложением из текста СЕССИИ на каждом кадре — Numi
    /// показывает результаты по ходу набора). Привязка — к рядам буфера
    /// редактора (`LayoutRun.line_i`). meaningful при `editing: Some`.
    pub editing_line_results: Option<&'a [Option<ExprOutcome>]>,
}

/// text_groups z-плана хранят ПОЗИЦИИ в `frame.indices`, а не индексы нод
/// (zorder.rs: «text_groups[g] — позиции видимых нод»), тогда как кэш Buffer'ов
/// ключован индексами нод. Маппинг обязателен: при пане/зуме часть нод уходит
/// за viewport, позиции и индексы расходятся, и прямое использование позиции
/// как индекса тихо теряет текст нод (баг приёмки: имена файлов после дропа и
/// текст нод пропадали при частично видимой сцене).
fn index_at(indices: &[usize], pos: usize) -> Option<usize> {
    indices.get(pos).copied()
}

/// Редактируемая нода состоит в текст-группе? `group` — позиции, `editing`
/// — индекс ноды (T7): их надо сравнивать через маппинг `indices`.
fn group_contains_node(indices: &[usize], group: &[usize], editing: usize) -> bool {
    group
        .iter()
        .any(|&pos| index_at(indices, pos) == Some(editing))
}

/// FR-013 (правка 4): прямоугольник зоны наведения бейджа ошибки строки
/// (физические px входа → логические px окна в выходе): расширенный на
/// LINE_ERROR_HIT_PAD_PX в каждую сторону, чтобы малый глиф «!» легко
/// попадал под курсор. `message` — текст ошибки для тултипа.
fn error_hit_rect(
    left_phys: f32,
    top_phys: f32,
    width_px: f32,
    zoom_px: f32,
    scale_factor: f32,
    message: String,
) -> LineErrorHit {
    let pad = LINE_ERROR_HIT_PAD_PX * scale_factor;
    let height = RESULT_LINE_HEIGHT * zoom_px;
    LineErrorHit {
        rect: [
            (left_phys - pad) / scale_factor,
            (top_phys - pad) / scale_factor,
            (width_px + 2.0 * pad) / scale_factor,
            (height + 2.0 * pad) / scale_factor,
        ],
        message,
    }
}

/// Зашейпленные буферы заголовка и тела ноды: валидны, пока не изменились
/// зум, ширина или тексты (см. cache_fresh).
struct CachedTitle {
    title: Buffer,
    icon: Option<Buffer>,
    /// Тело заметки (T7, GFM) — вертикальный стек блоков: только у text-нод
    /// с непустым текстом.
    body: Option<BodyLayout>,
    /// FR-013: программный итог формулы в футере карточки (MCP-expr без
    /// формульных строк в тексте). None — итога нет.
    result: Option<Buffer>,
    /// FR-013: программный итог — диагностика (красный цвет строки).
    result_error: bool,
    /// FR-013: ширина зашейпленного программного итога в px буфера — для
    /// выравнивания по правому краю футера (TextArea.left = right − width).
    result_width_px: f32,
    /// FR-013 (правка 2): зашейпленные результаты формульных строк
    /// (Numi-стиль) — правый край своей строки.
    line_results: Vec<LineResultBuf>,
    zoom_px: f32,
    width_px: f32,
    title_text: String,
    body_text: String,
    /// Сводка результатов (см. CacheKey.results) — ключ свежести.
    results_key: String,
    /// Тик последнего использования — для вытеснения невидимых нод.
    last_used: u64,
}

/// FR-013 (правка 2): зашейпленный результат одной формульной строки.
struct LineResultBuf {
    buffer: Buffer,
    /// Ширина строки результата в px буфера — для правого выравнивания.
    width_px: f32,
    /// Индекс строки текста ноды, к которой привязан результат.
    source_line: usize,
    /// Ошибка — красный бейдж «!» вместо текста (правка 4).
    error: bool,
    /// FR-013 (правка 4): полный текст ошибки — для тултипа при наведении
    /// на бейдж (None для успешных строк).
    message: Option<String>,
}

/// Зашейпленный лейбл связи (T8): валиден при том же тексте и зуме.
struct CachedEdgeLabel {
    buffer: Buffer,
    text: String,
    zoom_px: f32,
    /// Размер контента в px буфера (ширина строки × высота) — для центрирования
    /// и подложки.
    size_px: [f32; 2],
}

/// Текстовая система сцены: шрифты, атлас глифов, пул рендереров (по одному
/// на текст-группу кадра — z-порядок), кэш заголовков.
pub struct TextSystem {
    font_system: FontSystem,
    swash_cache: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    /// Пул TextRenderer: glyphon рисует все подготовленные одним `prepare`
    /// области одним draw-вызовом, поэтому сегменты кадра со своим текстом
    /// требуют отдельных рендереров (каждому — свой vertex buffer).
    /// Атлас общий — переиспользуется группами и кадрами.
    renderers: Vec<TextRenderer>,
    /// Кэш Buffer'ов по индексу ноды (T5: не шейпить 1500 заголовков каждый кадр).
    cache: HashMap<usize, CachedTitle>,
    /// Кэш лейблов связей по id связи (T8).
    label_cache: HashMap<String, CachedEdgeLabel>,
    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк
    /// (логические px) — пересобираются каждый кадр в prepare_titles;
    /// приложение вычитывает после рендера для тултипа.
    line_error_hits: Vec<LineErrorHit>,
    /// Номер кадра для LRU-вытеснения кэша.
    tick: u64,
    /// Палитра темы: цвета заголовка/иконки/тела/лейбла связи.
    theme: ThemeColors,
}

impl TextSystem {
    /// Создать текстовую систему под формат surface (тёмная тема по умолчанию).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let mut font_system = FontSystem::new();
        for data in FONT_DATA {
            font_system.db_mut().load_font_data((*data).to_vec());
        }
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let renderer =
            TextRenderer::new(&mut atlas, device, wgpu::MultisampleState::default(), None);
        Self {
            font_system,
            swash_cache,
            atlas,
            viewport,
            renderers: vec![renderer],
            cache: HashMap::new(),
            label_cache: HashMap::new(),
            line_error_hits: Vec::new(),
            tick: 0,
            theme: ThemeColors::dark(),
        }
    }

    /// Применить тему: сбросить кэш заголовков/тел (цвета GFM-блоков
    /// запечены в BodyItem.color) и заменить палитру. Смена темы редкая,
    /// перешейп дешёвый.
    pub fn set_theme(&mut self, theme: ThemeColors) {
        self.theme = theme;
        // CR-007: цвета блоков тела запечены в кэше (BodyItem.color) —
        // без сброса после переключения темы тело остаётся в старых цветах
        self.cache.clear();
    }

    /// Зашейпить/обновить запись лейбла связи (T8). Ширина буфера не
    /// ограничена — измеряем фактическую для центрирования и подложки.
    fn shape_edge_label(&mut self, id: &str, text: &str, zoom_px: f32) {
        let line_height = EDGE_LABEL_LINE_HEIGHT * zoom_px;
        let mut buffer = Buffer::new(
            &mut self.font_system,
            Metrics::new(EDGE_LABEL_FONT_SIZE * zoom_px, line_height),
        );
        buffer.set_wrap(&mut self.font_system, Wrap::None);
        buffer.set_size(&mut self.font_system, None, Some(line_height));
        buffer.set_text(&mut self.font_system, text, sans_attrs(), Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);
        let width = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0f32, f32::max);
        self.label_cache.insert(
            id.to_owned(),
            CachedEdgeLabel {
                buffer,
                text: text.to_owned(),
                zoom_px,
                size_px: [width, line_height],
            },
        );
    }

    /// Размер лейбла связи в world-px (T8): шейпинг кэшируется по id связи,
    /// перешейп — при смене текста или зума. Рендер вызывает это при сборке
    /// подложки, до prepare_titles того же кадра — там запись уже свежая.
    pub fn edge_label_size(&mut self, id: &str, text: &str, zoom_px: f32) -> [f32; 2] {
        let fresh = self
            .label_cache
            .get(id)
            .is_some_and(|entry| entry.text == text && (entry.zoom_px - zoom_px).abs() < 1e-3);
        if !fresh {
            self.shape_edge_label(id, text, zoom_px);
        }
        self.label_cache
            .get(id)
            .map(|entry| [entry.size_px[0] / zoom_px, entry.size_px[1] / zoom_px])
            .unwrap_or([0.0, 0.0])
    }

    /// Полная инвалидация кэшей, ключованных индексами нод (T8): после
    /// удаления ноды индексы сдвигаются. Лейблы связей ключованы id (String) —
    /// удаление нод их не ломает, не трогаем.
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк,
    /// собранные в последнем prepare_titles (логические px окна). Приложение
    /// hit-тестит курсор и показывает тултип с текстом ошибки.
    pub fn line_error_hits(&self) -> &[LineErrorHit] {
        &self.line_error_hits
    }

    /// Подготовить тексты кадра по текст-группам z-плана (zorder.rs):
    /// заголовки/тела видимых нод (culling, T5: `frame.indices` — выдача
    /// spatial index по viewport), буфер редактора (T7) на z-позиции
    /// редактируемой ноды, оверлеи и HUD — в финальной группе. Каждая
    /// группа готовится своим TextRenderer из пула и рисуется одним
    /// draw-вызовом (`draw_group`) между сегментами карточек.
    pub fn prepare_titles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &TitleFrame,
    ) -> Result<(), glyphon::PrepareError> {
        self.tick += 1;
        // FR-013 (правка 4): зоны ошибок пересобираются заново каждый кадр
        // (позиции зависят от камеры/зума)
        let mut error_hits: Vec<LineErrorHit> = Vec::new();
        let viewport_physical = frame.viewport_physical;
        let scale_factor = frame.scale_factor;
        self.viewport.update(
            queue,
            Resolution {
                width: viewport_physical[0],
                height: viewport_physical[1],
            },
        );

        let zoom_px = frame.camera.zoom() * scale_factor;
        let font_size = TITLE_FONT_SIZE * zoom_px;
        let line_height = TITLE_LINE_HEIGHT * zoom_px;
        let viewport_logical = [
            viewport_physical[0] as f32 / scale_factor,
            viewport_physical[1] as f32 / scale_factor,
        ];
        let to_physical = |world: [f32; 2]| {
            snap_to_pixel(
                frame.camera.world_to_screen(world, viewport_logical),
                scale_factor,
            )
        };

        // Фаза 1: актуализация кэша — шейпинг только новых/изменившихся заголовков.
        let show_titles = titles_visible(zoom_px);
        if show_titles {
            for &index in frame.indices {
                let Some(node) = frame.canvas.nodes.get(index) else {
                    continue;
                };
                // CR-004 v1: у виджет-нод хром прозрачен — заголовок шейпится
                // только для «раскрытых» (hover/выделение); в покое имени нет
                let revealed_widget = node.kind() == NodeKind::Widget
                    && frame.widget_title_reveal.binary_search(&index).is_ok();
                if node.kind() == NodeKind::Widget && !revealed_widget {
                    continue;
                }
                let has_icon = extension_letter(node).is_some();
                let title_width =
                    (node.width - TITLE_PADDING * 2.0 - if has_icon { ICON_WIDTH } else { 0.0 })
                        .max(0.0);
                let width_px = title_width * zoom_px;
                // FR-011: у свернутой ноды в заголовке бейдж «+N» — число
                // скрытых потомков
                let collapsed_count = frame
                    .collapsed_counts
                    .binary_search_by_key(&index, |(i, _)| *i)
                    .ok()
                    .and_then(|pos| frame.collapsed_counts.get(pos).map(|(_, n)| *n));
                let title_text = match collapsed_count {
                    Some(count) => format!("{} +{}", title_for(node), count),
                    None => title_for(node),
                };
                // Тело редактируемой ноды рисует EditingSession — не шейпим дубль
                let body_text = if frame.editing == Some(index) || !body_visible(node, zoom_px) {
                    String::new()
                } else {
                    node.text.clone().unwrap_or_default()
                };
                // FR-013: программный итог формулы (футер карточки).
                // FR-014: expr_results — карта потока значений (пишется
                // propagator'ом для всех формульных нод), поэтому правило
                // показа — здесь: построчные результаты Numi-листа
                // вытесняют программный итог; источник истины — expr_results.
                // FR-018: у шаблонной ноды программный итог — результат
                // формулы шаблона (mm1/…) — показывается ВСЕГДА, поверх
                // построчных результатов параметров (лист параметров —
                // присваивания; итог шаблона — смысл ноды).
                let line_outcomes = frame.expr_line_results.get(&node.id);
                let has_line_results =
                    line_outcomes.is_some_and(|lines| lines.iter().any(Option::is_some));
                let is_template_node = node.template().is_some();
                let (result_text, result_error) = if has_line_results && !is_template_node {
                    (String::new(), false)
                } else {
                    match frame.expr_results.get(&node.id) {
                        Some(ExprOutcome::Ok(value)) => (value.to_string(), false),
                        Some(ExprOutcome::Err(msg)) => (msg.clone(), true),
                        _ => (String::new(), false),
                    }
                };
                // FR-013 (правка 2): формульные строки текста (для сегментации
                // тела) и сводка построчных результатов — ключ свежести кэша
                let formula_lines: Vec<usize> = line_outcomes
                    .map(|lines| {
                        lines
                            .iter()
                            .enumerate()
                            .filter(|(_, outcome)| outcome.is_some())
                            .map(|(i, _)| i)
                            .collect()
                    })
                    .unwrap_or_default();
                let line_key = if formula_lines.is_empty() {
                    String::new()
                } else {
                    line_outcomes
                        .map(|lines| {
                            lines
                                .iter()
                                .enumerate()
                                .filter_map(|(i, outcome)| {
                                    outcome.as_ref().map(|outcome| match outcome {
                                        ExprOutcome::Ok(value) => format!("{i}={value}"),
                                        ExprOutcome::Err(msg) => format!("{i}!{msg}"),
                                    })
                                })
                                .collect::<Vec<_>>()
                                .join(";")
                        })
                        .unwrap_or_default()
                };
                let results_key = if result_text.is_empty() && line_key.is_empty() {
                    String::new()
                } else {
                    format!("P:{result_text}|L:{line_key}")
                };

                let fresh = self.cache.get(&index).is_some_and(|e| {
                    cache_fresh(
                        CacheKey {
                            zoom: e.zoom_px,
                            width: e.width_px,
                            title: &e.title_text,
                            body: &e.body_text,
                            results: &e.results_key,
                        },
                        CacheKey {
                            zoom: zoom_px,
                            width: width_px,
                            title: &title_text,
                            body: &body_text,
                            results: &results_key,
                        },
                    )
                });
                if !fresh {
                    let mut title =
                        Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
                    title.set_wrap(&mut self.font_system, Wrap::None);
                    title.set_size(
                        &mut self.font_system,
                        Some(width_px),
                        Some(HEADER_HEIGHT * zoom_px),
                    );
                    title.set_text(
                        &mut self.font_system,
                        &title_text,
                        sans_attrs(),
                        Shaping::Advanced,
                    );
                    title.shape_until_scroll(&mut self.font_system, false);

                    let icon = extension_letter(node).map(|letter| {
                        let mut icon = Buffer::new(
                            &mut self.font_system,
                            Metrics::new(font_size, line_height),
                        );
                        icon.set_size(
                            &mut self.font_system,
                            Some(ICON_WIDTH * zoom_px),
                            Some(HEADER_HEIGHT * zoom_px),
                        );
                        let letter = letter.to_string();
                        icon.set_text(
                            &mut self.font_system,
                            &letter,
                            sans_attrs(),
                            Shaping::Advanced,
                        );
                        icon.shape_until_scroll(&mut self.font_system, false);
                        icon
                    });

                    // Тело заметки (T7, GFM): вертикальный стек блоков —
                    // заголовки/списки/цитаты/фенсы/линии со своими метриками
                    // и квадами (подсветка, зачёркивание, буллиты, бары).
                    let body = if body_text.is_empty() {
                        None
                    } else {
                        let (_, body_width, _) = body_area(node);
                        Some(shape_body(
                            &mut self.font_system,
                            &self.theme,
                            &body_text,
                            body_width,
                            zoom_px,
                            &formula_lines,
                        ))
                    };

                    // FR-013: строка результата — одна строка в футере
                    // карточки, шейпится вместе с остальным кэшем ноды;
                    // ширина строки замеряется для правого выравнивания
                    let (result, result_width_px) = if result_text.is_empty() {
                        (None, 0.0)
                    } else {
                        let (_, body_width, _) = body_area(node);
                        let mut buffer = Buffer::new(
                            &mut self.font_system,
                            Metrics::new(RESULT_FONT_SIZE * zoom_px, RESULT_LINE_HEIGHT * zoom_px),
                        );
                        buffer.set_wrap(&mut self.font_system, Wrap::None);
                        buffer.set_size(
                            &mut self.font_system,
                            Some(body_width * zoom_px),
                            Some(RESULT_LINE_HEIGHT * zoom_px),
                        );
                        buffer.set_text(
                            &mut self.font_system,
                            &result_text,
                            mono_attrs(),
                            Shaping::Advanced,
                        );
                        buffer.shape_until_scroll(&mut self.font_system, false);
                        let result_width_px = buffer
                            .layout_runs()
                            .next()
                            .map(|run| run.line_w)
                            .unwrap_or(0.0);
                        (Some(buffer), result_width_px)
                    };

                    // FR-013 (правка 2): буферы результатов формульных строк
                    // (Numi-стиль) — шейпятся вместе с кэшем ноды; ошибка —
                    // красный бейдж «!» (правка 4: полный текст уходит в
                    // тултип, длинные сообщения не влезают в строку ноды)
                    let line_results = line_outcomes
                        .map(|lines| {
                            let (_, body_width, _) = body_area(node);
                            let area_px = (body_width * zoom_px).max(1.0);
                            lines
                                .iter()
                                .enumerate()
                                .filter_map(|(i, outcome)| {
                                    let outcome = outcome.as_ref()?;
                                    let (text, message) = match outcome {
                                        ExprOutcome::Ok(value) => (value.to_string(), None),
                                        ExprOutcome::Err(msg) => {
                                            (LINE_ERROR_BADGE.to_owned(), Some(msg.clone()))
                                        }
                                    };
                                    if text.is_empty() {
                                        return None;
                                    }
                                    let mut buffer = Buffer::new(
                                        &mut self.font_system,
                                        Metrics::new(
                                            RESULT_FONT_SIZE * zoom_px,
                                            RESULT_LINE_HEIGHT * zoom_px,
                                        ),
                                    );
                                    buffer.set_wrap(&mut self.font_system, Wrap::None);
                                    buffer.set_size(
                                        &mut self.font_system,
                                        Some(area_px),
                                        Some(RESULT_LINE_HEIGHT * zoom_px),
                                    );
                                    buffer.set_text(
                                        &mut self.font_system,
                                        &text,
                                        mono_attrs(),
                                        Shaping::Advanced,
                                    );
                                    buffer.shape_until_scroll(&mut self.font_system, false);
                                    let width_px = buffer
                                        .layout_runs()
                                        .next()
                                        .map(|run| run.line_w)
                                        .unwrap_or(0.0);
                                    Some(LineResultBuf {
                                        buffer,
                                        width_px,
                                        source_line: i,
                                        error: matches!(outcome, ExprOutcome::Err(_)),
                                        message,
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    self.cache.insert(
                        index,
                        CachedTitle {
                            title,
                            icon,
                            body,
                            result,
                            result_error,
                            result_width_px,
                            line_results,
                            zoom_px,
                            width_px,
                            title_text,
                            body_text,
                            results_key,
                            last_used: self.tick,
                        },
                    );
                }
                if let Some(entry) = self.cache.get_mut(&index) {
                    entry.last_used = self.tick;
                }
            }
        }

        // Периодическое вытеснение заголовков давно невидимых нод
        if self.tick % CACHE_SWEEP_INTERVAL == 0 {
            let horizon = self.tick.saturating_sub(CACHE_MAX_AGE);
            self.cache.retain(|_, entry| entry.last_used >= horizon);
        }

        // Лейблы связей (T8): вытеснение удалённых из модели, перешейп
        // изменённых (текст/зум). Мутации кэша — до сборки TextArea ниже.
        self.label_cache
            .retain(|id, _| frame.edge_labels.iter().any(|label| label.id == id));
        if show_titles {
            for label in frame.edge_labels {
                let fresh = self.label_cache.get(label.id).is_some_and(|entry| {
                    entry.text == label.text && (entry.zoom_px - zoom_px).abs() < 1e-3
                });
                if !fresh {
                    self.shape_edge_label(label.id, label.text, zoom_px);
                }
            }
        }

        // HUD-оверлей (F3): фиксированный физический размер шрифта, левый верхний
        // угол; тень смещением на 1px для читаемости на светлых карточках.
        // Буферы живут до конца prepare — дальше в атлас не попадают.
        let mut hud_buffers: Vec<(Buffer, [f32; 2], f32, f32, Color)> = Vec::new();
        if let Some(hud) = frame.hud {
            let width = (viewport_physical[0] as f32 - HUD_PADDING * 2.0).max(1.0);
            let make_buffer = |font_system: &mut FontSystem| {
                let mut buffer =
                    Buffer::new(font_system, Metrics::new(HUD_FONT_SIZE, HUD_LINE_HEIGHT));
                buffer.set_wrap(font_system, Wrap::None);
                buffer.set_size(font_system, Some(width), Some(HUD_LINE_HEIGHT));
                buffer.set_text(font_system, hud, sans_attrs(), Shaping::Advanced);
                buffer.shape_until_scroll(font_system, false);
                buffer
            };
            // Тень (чёрная, +1px) рисуется первой — текст поверх
            hud_buffers.push((
                make_buffer(&mut self.font_system),
                [HUD_PADDING + 1.0, HUD_PADDING + 1.0],
                width,
                HUD_LINE_HEIGHT,
                Color::rgb(0x10, 0x10, 0x12),
            ));
            hud_buffers.push((
                make_buffer(&mut self.font_system),
                [HUD_PADDING, HUD_PADDING],
                width,
                HUD_LINE_HEIGHT,
                HUD_COLOR,
            ));
        }

        // Фаза 2: TextArea из кэша — по текст-группам z-плана (позиции
        // пересчитываются каждый кадр при пан, шейпинг — нет). Группа =
        // тексты одного сегмента кадра: рисуются после карточек сегмента
        // и до карточек, перекрывающих его ноды (z-порядок, zorder.rs).
        let group_count = frame.zplan.group_count();
        let final_group = frame.zplan.final_group();
        // Группа screen-space оверлеев (панель поиска/настроек, тултип):
        // индекс за пределами z-плана — её квады рисует рендерер финальным
        // проходом ПОСЛЕ всех сегментов, тексты — этой группой после квадов.
        // Раньше квады оверлея расширяли диапазон последнего сегмента, и
        // тамбнейлы/тексты его нод перекрывали панель (баг T14).
        let overlay_group = group_count;
        while self.renderers.len() <= overlay_group {
            let renderer = TextRenderer::new(
                &mut self.atlas,
                device,
                wgpu::MultisampleState::default(),
                None,
            );
            self.renderers.push(renderer);
        }
        // Оверлей-тексты (контекстное меню, T7): шейпинг покадрово, без кэша
        let mut overlay_buffers: Vec<(Buffer, [f32; 2], f32)> = Vec::new();
        for overlay in frame.overlay_texts {
            let mut buffer =
                Buffer::new(&mut self.font_system, Metrics::new(font_size, line_height));
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(
                &mut self.font_system,
                Some(overlay.width * zoom_px),
                Some(line_height),
            );
            buffer.set_text(
                &mut self.font_system,
                overlay.text,
                sans_attrs(),
                Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            overlay_buffers.push((buffer, overlay.origin, overlay.width));
        }

        // Screen-space тексты (панель настроек): константный физический
        // размер, позиции — логические px от угла окна, без камеры
        let mut screen_buffers: Vec<Buffer> = Vec::with_capacity(frame.screen_texts.len());
        for st in frame.screen_texts {
            let font = st.font_size * scale_factor;
            let line_height = font * 1.3;
            let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font, line_height));
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(
                &mut self.font_system,
                Some(st.width * scale_factor),
                Some(line_height),
            );
            buffer.set_text(
                &mut self.font_system,
                st.text,
                sans_attrs(),
                Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            screen_buffers.push(buffer);
        }

        // FR-013: бейдж «=» calc-нод при дальнем зуме (титулы скрыты) —
        // один общий буфер на кадр, константный физический размер (как HUD);
        // шейпится ДО цикла групп — TextArea заимствует его до prepare_group
        let mut badge_buffer: Option<Buffer> = None;
        if !show_titles
            && frame.indices.iter().any(|&index| {
                frame.canvas.nodes.get(index).is_some_and(|node| {
                    node.kind() == NodeKind::Text
                        && (frame.expr_results.contains_key(&node.id)
                            || frame.expr_line_results.contains_key(&node.id))
                })
            })
        {
            let mut buffer = Buffer::new(
                &mut self.font_system,
                Metrics::new(BADGE_FONT_SIZE, BADGE_LINE_HEIGHT),
            );
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(
                &mut self.font_system,
                Some(BADGE_FONT_SIZE * 2.0),
                Some(BADGE_LINE_HEIGHT),
            );
            buffer.set_text(&mut self.font_system, "=", mono_attrs(), Shaping::Advanced);
            buffer.shape_until_scroll(&mut self.font_system, false);
            badge_buffer = Some(buffer);
        }

        // FR-013 (правка 4): ЖИВЫЕ построчные результаты редактируемой ноды
        // (Numi показывает результаты по ходу набора) — шейпятся покадрово:
        // текст сессии меняется при каждой правке, буферы маленькие. Ошибка —
        // красный бейдж «!», полный текст ошибки — в кортеже для тултипа.
        let mut live_line_buffers: Vec<(Buffer, f32, usize, bool, String)> = Vec::new();
        if let (Some(edit_index), Some(outcomes)) = (frame.editing, frame.editing_line_results) {
            if let Some(node) = frame.canvas.nodes.get(edit_index) {
                if node.kind() == NodeKind::Text {
                    let (_, body_width, _) = body_area(node);
                    let area_px = (body_width * zoom_px).max(1.0);
                    for (i, outcome) in outcomes.iter().enumerate() {
                        let Some(outcome) = outcome.as_ref() else {
                            continue;
                        };
                        let (text, message) = match outcome {
                            ExprOutcome::Ok(value) => (value.to_string(), String::new()),
                            ExprOutcome::Err(msg) => (LINE_ERROR_BADGE.to_owned(), msg.clone()),
                        };
                        let mut buffer = Buffer::new(
                            &mut self.font_system,
                            Metrics::new(RESULT_FONT_SIZE * zoom_px, RESULT_LINE_HEIGHT * zoom_px),
                        );
                        buffer.set_wrap(&mut self.font_system, Wrap::None);
                        buffer.set_size(
                            &mut self.font_system,
                            Some(area_px),
                            Some(RESULT_LINE_HEIGHT * zoom_px),
                        );
                        buffer.set_text(
                            &mut self.font_system,
                            &text,
                            mono_attrs(),
                            Shaping::Advanced,
                        );
                        buffer.shape_until_scroll(&mut self.font_system, false);
                        let width_px = buffer
                            .layout_runs()
                            .next()
                            .map(|run| run.line_w)
                            .unwrap_or(0.0);
                        live_line_buffers.push((buffer, width_px, i, !message.is_empty(), message));
                    }
                }
            }
        }

        for (g, group) in frame.zplan.text_groups.iter().enumerate() {
            let mut areas: Vec<TextArea> = Vec::with_capacity(group.len() * 3 + 4);
            if show_titles {
                for &pos in group {
                    let Some(index) = index_at(frame.indices, pos) else {
                        continue;
                    };
                    let (Some(node), Some(entry)) =
                        (frame.canvas.nodes.get(index), self.cache.get(&index))
                    else {
                        continue;
                    };
                    // CR-004 v1: заголовок виджет-ноды рисуется только у
                    // «раскрытых» (hover/выделение) — хром прозрачен
                    if node.kind() == NodeKind::Widget
                        && !frame.widget_title_reveal.binary_search(&index).is_ok()
                    {
                        continue;
                    }
                    let has_icon = entry.icon.is_some();
                    // CR-007 (доступность): на окрашенной карточке цвета текста
                    // темы обязаны читаться на её заливке (WCAG AA ≥ 4.5:1).
                    // Группы исключены: заливка группы — theme.group_fill,
                    // а не цвет ноды (cards::card_instance).
                    let colored_fill = if node.kind() == NodeKind::Group {
                        None
                    } else {
                        crate::cards::named_color(node.color.as_deref(), &self.theme)
                    };
                    let on_card = |color: Color| match colored_fill {
                        Some(fill) => self.theme.readable_on_card(color, fill, true),
                        None => color,
                    };
                    // T23 (brainstorm-focus): тексты не-фокусной ноды гаснут
                    // вместе с карточкой (фокусная и выделенная — полная
                    // яркость: приложение включает выделенную в набор)
                    let text_factor = if frame.focus.dim > 0.0 && !frame.focus.has_node(index) {
                        frame.focus.dim_factor()
                    } else {
                        1.0
                    };
                    let title_x = node.x + TITLE_PADDING + if has_icon { ICON_WIDTH } else { 0.0 };
                    let pos = to_physical([title_x, node.y]);
                    areas.push(TextArea {
                        buffer: &entry.title,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + entry.width_px) as i32,
                            bottom: (pos[1] + HEADER_HEIGHT * zoom_px) as i32,
                        },
                        default_color: dim_color(on_card(self.theme.title), text_factor),
                        custom_glyphs: &[],
                    });
                    if let Some(icon) = &entry.icon {
                        let pos = to_physical([node.x + TITLE_PADDING, node.y]);
                        areas.push(TextArea {
                            buffer: icon,
                            left: pos[0],
                            top: pos[1],
                            scale: 1.0,
                            bounds: TextBounds {
                                left: pos[0] as i32,
                                top: pos[1] as i32,
                                right: (pos[0] + ICON_WIDTH * zoom_px) as i32,
                                bottom: (pos[1] + HEADER_HEIGHT * zoom_px) as i32,
                            },
                            default_color: dim_color(on_card(self.theme.icon), text_factor),
                            custom_glyphs: &[],
                        });
                    }
                    // Тело заметки (T7, GFM): вертикальный стек блоков —
                    // у редактируемой ноды body нет, его рисует буфер
                    // EditingSession (блок ниже). Клип блока: нижняя граница
                    // не ниже нижней границы области тела — текст нижнего
                    // блока не вылезает за карточку. FR-013: при наличии
                    // строки результата тело подрезается до её верхней
                    // границы — текст не заходит под футер с результатом.
                    let result_top_phys = if entry.result.is_some() {
                        to_physical([
                            0.0,
                            node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT,
                        ])[1]
                    } else {
                        f32::MAX
                    };
                    if let Some(layout) = &entry.body {
                        let (origin, _, body_height) = body_area(node);
                        let origin = to_physical(origin);
                        let body_bottom = (origin[1] + body_height * zoom_px).min(result_top_phys);
                        for block in &layout.blocks {
                            // Снап к целым физическим px — иначе каждый кадр
                            // панорамы даёт новый subpixel-бин глифа (см. snap_to_pixel)
                            let left = (origin[0] + block.offset[0] * zoom_px).round();
                            let top = (origin[1] + block.offset[1] * zoom_px).round();
                            let bottom = ((top + block.height * zoom_px).min(body_bottom)).round();
                            areas.push(TextArea {
                                buffer: &block.buffer,
                                left,
                                top,
                                scale: 1.0,
                                bounds: TextBounds {
                                    left: left as i32,
                                    top: top as i32,
                                    right: (left + block.width * zoom_px) as i32,
                                    bottom: bottom as i32,
                                },
                                default_color: dim_color(on_card(block.color), text_factor),
                                custom_glyphs: &[],
                            });
                        }
                    }
                    // FR-013 (правка 2): результат каждой формульной строки —
                    // ПРАВЫЙ край ЕЁ строки (Numi-стиль). Привязка — блок тела
                    // с source_line этой строки (формульная строка — отдельный
                    // блок, см. body_items).
                    if !entry.line_results.is_empty() {
                        let (origin, _, _) = body_area(node);
                        for line_result in &entry.line_results {
                            let Some(block) = entry.body.as_ref().and_then(|layout| {
                                layout.blocks.iter().find(|block| {
                                    block.source_line == Some(line_result.source_line)
                                })
                            }) else {
                                continue;
                            };
                            // Вертикальное центрирование результата в ряду
                            let row_y = origin[1]
                                + block.offset[1]
                                + (BODY_LINE_HEIGHT - RESULT_LINE_HEIGHT) / 2.0;
                            let top_phys = to_physical([origin[0], row_y])[1];
                            let right_phys =
                                to_physical([node.x + node.width - BODY_PADDING, row_y])[0];
                            let left_phys = (right_phys - line_result.width_px).round();
                            areas.push(TextArea {
                                buffer: &line_result.buffer,
                                left: left_phys,
                                top: top_phys,
                                scale: 1.0,
                                bounds: TextBounds {
                                    left: (to_physical([node.x + BODY_PADDING, row_y])[0].floor()
                                        as i32)
                                        - 1,
                                    top: top_phys as i32,
                                    right: (right_phys.round() as i32) + 1,
                                    bottom: (top_phys + RESULT_LINE_HEIGHT * zoom_px) as i32,
                                },
                                default_color: if line_result.error {
                                    dim_color(on_card(RESULT_ERROR_COLOR), text_factor)
                                } else {
                                    dim_color(on_card(self.theme.link), text_factor)
                                },
                                custom_glyphs: &[],
                            });
                            // FR-013 (правка 4): ошибка — расширенная зона
                            // наведения вокруг бейджа для тултипа
                            if line_result.error {
                                error_hits.push(error_hit_rect(
                                    left_phys,
                                    top_phys,
                                    line_result.width_px,
                                    zoom_px,
                                    scale_factor,
                                    line_result.message.clone().unwrap_or_default(),
                                ));
                            }
                        }
                    }
                    // FR-013 (правка 4): ЖИВЫЕ результаты редактируемой ноды —
                    // привязка к рядам буфера редактора: LayoutRun.line_i —
                    // индекс исходной строки, line_top/line_height — позиция
                    // её ПЕРВОГО визуального ряда (перенос игнорируется).
                    if frame.editing == Some(index) {
                        if let Some((edit_buffer, edit_origin, _, _)) = frame.editing_buffer {
                            if !live_line_buffers.is_empty() {
                                let mut rows: HashMap<usize, (f32, f32)> = HashMap::new();
                                for run in edit_buffer.layout_runs() {
                                    rows.entry(run.line_i)
                                        .or_insert((run.line_top, run.line_height));
                                }
                                let edit_top = to_physical(edit_origin)[1];
                                for (buffer, width_px, source_line, error, message) in
                                    &live_line_buffers
                                {
                                    let Some((row_top, row_h)) = rows.get(source_line) else {
                                        continue;
                                    };
                                    let result_h = RESULT_LINE_HEIGHT * zoom_px;
                                    let top_phys = edit_top + row_top + (row_h - result_h) / 2.0;
                                    let right_phys =
                                        to_physical([node.x + node.width - BODY_PADDING, 0.0])[0];
                                    let left_phys = (right_phys - width_px).round();
                                    areas.push(TextArea {
                                        buffer,
                                        left: left_phys,
                                        top: top_phys,
                                        scale: 1.0,
                                        bounds: TextBounds {
                                            left: (to_physical([node.x + BODY_PADDING, 0.0])[0]
                                                .floor()
                                                as i32)
                                                - 1,
                                            top: top_phys as i32,
                                            right: (right_phys.round() as i32) + 1,
                                            bottom: (top_phys + result_h) as i32,
                                        },
                                        default_color: if *error {
                                            dim_color(on_card(RESULT_ERROR_COLOR), text_factor)
                                        } else {
                                            dim_color(on_card(self.theme.link), text_factor)
                                        },
                                        custom_glyphs: &[],
                                    });
                                    if *error {
                                        error_hits.push(error_hit_rect(
                                            left_phys,
                                            top_phys,
                                            *width_px,
                                            zoom_px,
                                            scale_factor,
                                            message.clone(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    // FR-013 (правка): строка результата формулы — футер
                    // карточки, выравнивание по ПРАВОМУ краю ноды:
                    // TextArea.left = правая граница футера − ширина строки.
                    // Успех — акцентный цвет, ошибка — красная диагностика.
                    if let Some(result) = &entry.result {
                        let top_world = node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT;
                        let left_world = node.x + BODY_PADDING;
                        let right_world = node.x + node.width - BODY_PADDING;
                        let right_phys = to_physical([right_world, top_world])[0];
                        let pos = to_physical([left_world, top_world]);
                        let left_phys = (right_phys - entry.result_width_px).round();
                        areas.push(TextArea {
                            buffer: result,
                            left: left_phys,
                            top: pos[1],
                            scale: 1.0,
                            bounds: TextBounds {
                                left: (pos[0].floor() as i32) - 1,
                                top: pos[1] as i32,
                                right: (right_phys.round() as i32) + 1,
                                bottom: (pos[1] + RESULT_LINE_HEIGHT * zoom_px) as i32,
                            },
                            default_color: if entry.result_error {
                                dim_color(on_card(RESULT_ERROR_COLOR), text_factor)
                            } else {
                                dim_color(on_card(self.theme.link), text_factor)
                            },
                            custom_glyphs: &[],
                        });
                    }
                }
            }
            // Текст активной сессии редактирования (T7): буфер редактора на
            // z-позиции редактируемой ноды; клип — область тела карточки,
            // текст не выходит за пределы заметки (авторост — в fit_note_size).
            if let (Some((buffer, origin, area_w, area_h)), Some(editing_index)) =
                (frame.editing_buffer, frame.editing)
            {
                if group_contains_node(frame.indices, group, editing_index) {
                    let pos = to_physical(origin);
                    areas.push(TextArea {
                        buffer,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + area_w * zoom_px) as i32,
                            bottom: (pos[1] + area_h * zoom_px) as i32,
                        },
                        default_color: self.theme.body,
                        custom_glyphs: &[],
                    });
                }
            }
            // Буфер сессии редактирования лейбла связи (T8): для EditTarget::Edge
            // `frame.editing` = None (индекс ноды нет), поэтому блок выше не
            // срабатывал и текст лейбла исчезал при входе в редактирование.
            // Бокс по центру кривой — поверх всего кадра, как его подложка.
            if frame.editing.is_none() {
                if let Some((buffer, origin, area_w, area_h)) = frame.editing_buffer {
                    let pos = to_physical(origin);
                    areas.push(TextArea {
                        buffer,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + area_w * zoom_px) as i32,
                            bottom: (pos[1] + area_h * zoom_px) as i32,
                        },
                        default_color: self.theme.body,
                        custom_glyphs: &[],
                    });
                }
            }
            // Финальная группа поверх всего кадра: лейблы связей (T8),
            // подписи меню (T7) и HUD (F3). Screen-тексты панелей — в
            // overlay_group (после квадов оверлея, см. ниже)
            if g == final_group {
                // Лейблы связей (T8): текст по центру кривой — кэш обновлён
                // в фазе 1, подложка лейбла уходит в карточки оверлей-региона
                if show_titles {
                    for label in frame.edge_labels {
                        let Some(entry) = self.label_cache.get(label.id) else {
                            continue;
                        };
                        let pos = to_physical(label.center);
                        let left = (pos[0] - entry.size_px[0] / 2.0).round();
                        let top = (pos[1] - entry.size_px[1] / 2.0).round();
                        areas.push(TextArea {
                            buffer: &entry.buffer,
                            left,
                            top,
                            scale: 1.0,
                            bounds: TextBounds {
                                left: left as i32,
                                top: top as i32,
                                right: (left + entry.size_px[0] + 1.0) as i32,
                                bottom: (top + entry.size_px[1]) as i32,
                            },
                            // T23: не-фокусные лейблы гаснут вместе со связями
                            default_color: dim_color(self.theme.edge_label, label.factor),
                            custom_glyphs: &[],
                        });
                    }
                }
                // FR-013: бейдж «=» в правом верхнем углу каждой видимой
                // calc-ноды — единственный признак формулы при дальнем зуме
                if let Some(badge) = &badge_buffer {
                    for &index in frame.indices {
                        let Some(node) = frame.canvas.nodes.get(index) else {
                            continue;
                        };
                        if node.kind() != NodeKind::Text || node.expr().is_none() {
                            continue;
                        }
                        let pos = to_physical([
                            node.x + node.width - BADGE_FONT_SIZE - 2.0,
                            node.y + 2.0,
                        ]);
                        areas.push(TextArea {
                            buffer: badge,
                            left: pos[0],
                            top: pos[1],
                            scale: 1.0,
                            bounds: TextBounds {
                                left: pos[0] as i32,
                                top: pos[1] as i32,
                                right: (pos[0] + BADGE_FONT_SIZE * 2.0) as i32,
                                bottom: (pos[1] + BADGE_LINE_HEIGHT) as i32,
                            },
                            default_color: dim_color(self.theme.title, 0.75),
                            custom_glyphs: &[],
                        });
                    }
                }
                for (buffer, origin, width) in &overlay_buffers {
                    let pos = to_physical(*origin);
                    areas.push(TextArea {
                        buffer,
                        left: pos[0],
                        top: pos[1],
                        scale: 1.0,
                        bounds: TextBounds {
                            left: pos[0] as i32,
                            top: pos[1] as i32,
                            right: (pos[0] + width * zoom_px) as i32,
                            bottom: (pos[1] + line_height) as i32,
                        },
                        default_color: self.theme.title,
                        custom_glyphs: &[],
                    });
                }
                areas.extend(
                    hud_buffers
                        .iter()
                        .map(|(buffer, pos, width, height, color)| TextArea {
                            buffer,
                            left: pos[0],
                            top: pos[1],
                            scale: 1.0,
                            bounds: TextBounds {
                                left: pos[0] as i32,
                                top: pos[1] as i32,
                                right: (pos[0] + width) as i32,
                                bottom: (pos[1] + height) as i32,
                            },
                            default_color: *color,
                            custom_glyphs: &[],
                        }),
                );
            }
            if let Some(renderer) = self.renderers.get_mut(g) {
                prepare_group(
                    renderer,
                    device,
                    queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    &areas,
                    &mut self.swash_cache,
                )?;
            }
        }
        // FR-013 (правка 4): зоны ошибок кадра собраны — приложение вычитает
        // их после рендера для hit-теста курсора (тултип ошибки)
        self.line_error_hits = error_hits;
        // Screen-тексты (панель поиска/настроек, тултип): отдельная группа
        // ПОСЛЕ квадов оверлея — иначе их фон (квады) рисовался бы после
        // текстов и закрывал собственные строки панели
        let mut overlay_areas: Vec<TextArea> = Vec::with_capacity(screen_buffers.len());
        for (buffer, st) in screen_buffers.iter().zip(frame.screen_texts) {
            let mut left = st.origin[0] * scale_factor;
            // Центровка: glyphon 0.6 (cosmic-text 0.11) не имеет set_align —
            // сдвигаем левый край на половину разницы ширин областей.
            if st.align == TextAlign::Center {
                let line_w = buffer
                    .layout_runs()
                    .next()
                    .map(|run| run.line_w)
                    .unwrap_or(0.0);
                left += ((st.width * scale_factor) - line_w).max(0.0) / 2.0;
            }
            // Снап к целым физическим px — единообразно с мировыми текстами
            let left = left.round();
            let top = (st.origin[1] * scale_factor).round();
            let line_height = st.font_size * scale_factor * 1.3;
            overlay_areas.push(TextArea {
                buffer,
                left,
                top,
                scale: 1.0,
                bounds: TextBounds {
                    left: left as i32,
                    top: top as i32,
                    right: (left + st.width * scale_factor) as i32,
                    bottom: (top + line_height) as i32,
                },
                default_color: st.color,
                custom_glyphs: &[],
            });
        }
        if let Some(renderer) = self.renderers.get_mut(overlay_group) {
            prepare_group(
                renderer,
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                &overlay_areas,
                &mut self.swash_cache,
            )?;
        }
        // Восстановление LRU-эвикшна атласа: без trim() glyphs_in_use не
        // сбрасывается (glyphon 0.6) — атлас монотонно заполняется при
        // пан/зуме и prepare падает с AtlasFull.
        self.atlas.trim();
        Ok(())
    }

    /// Нарисовать текст-группу `group` в активном render pass. Группы
    /// рисуются сегментами кадра между диапазонами карточек и тамбнейлов
    /// (z-порядок, см. zorder.rs и Renderer::render).
    pub fn draw_group<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
        group: usize,
    ) -> Result<(), glyphon::RenderError> {
        match self.renderers.get(group) {
            Some(renderer) => renderer.render(&self.atlas, &self.viewport, pass),
            None => Ok(()),
        }
    }

    /// Индекс группы screen-space оверлеев (панель поиска/настроек, тултип):
    /// на один больше всех групп z-плана — её тексты рисуются ПОСЛЕ квадов
    /// оверлея, которые рендерер выводит финальным проходом после всех
    /// сегментов (иначе тамбнейлы/тексты последнего сегмента перекрывали
    /// панель, баг T14).
    pub fn overlay_group(zplan: &crate::zorder::ZPlan) -> usize {
        zplan.group_count()
    }

    /// Доступ к FontSystem для операций EditingSession (T7): ввод, каретка,
    /// выделение шейпятся через тот же FontSystem, что и вся сцена.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Декоративные квады тела ноды (GFM): (zoom_px записи кэша, квады в px
    /// виртуального буфера тела). None — квадов нет или тело не в кэше
    /// (промах — квады появятся со следующего кадра, после шейпинга в
    /// prepare_titles). Рендерер маппит BodyQuadKind на заливки темы.
    pub fn body_quads(&self, index: usize) -> Option<(f32, &[BodyQuad])> {
        let entry = self.cache.get(&index)?;
        let body = entry.body.as_ref()?;
        if body.quads.is_empty() {
            return None;
        }
        Some((entry.zoom_px, body.quads.as_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Маппинг позиции z-плана → индекс ноды: позиции — в `frame.indices`
    /// (culling, T5), индексы — в `Canvas.nodes` и кэше. При частично видимой
    /// сцене позиции меньше индексов: без маппинга текст нод терялся (баг
    /// приёмки), включая имена файлов после дропа.
    #[test]
    fn group_positions_map_to_node_indices() {
        // Видимы ноды с индексами 2, 5, 7: позиции 0..3 ≠ индексы
        let indices = [2, 5, 7];
        assert_eq!(index_at(&indices, 0), Some(2));
        assert_eq!(index_at(&indices, 2), Some(7));
        assert_eq!(index_at(&indices, 3), None, "позиция за выдачей");
        assert!(group_contains_node(&indices, &[0, 2], 7));
        assert!(group_contains_node(&indices, &[1], 5));
        assert!(
            !group_contains_node(&indices, &[0, 2], 5),
            "индекс 5 — позиция 1, а не 0/2"
        );
        assert!(!group_contains_node(&indices, &[], 2));
    }

    /// LOD-порог: заголовок мельче MIN_TITLE_PX физических px не готовится.
    #[test]
    fn titles_lod_threshold() {
        assert!(!titles_visible(0.0));
        assert!(!titles_visible(MIN_TITLE_PX / TITLE_FONT_SIZE - 0.001));
        assert!(titles_visible(MIN_TITLE_PX / TITLE_FONT_SIZE));
        assert!(titles_visible(1.0));
    }

    /// Снап к целым физическим пикселям: дробные экранные позиции дают
    /// целые физические, уже целые не меняются (защита от раздувания атласа
    /// subpixel-бинами cosmic-text при панораме).
    #[test]
    fn snap_to_pixel_rounds_to_physical_pixels() {
        let snapped = snap_to_pixel([10.4, 20.6], 2.0);
        assert_eq!(snapped, [21.0, 41.0]);
        assert_eq!(snapped[0].fract(), 0.0);
        assert_eq!(snapped[1].fract(), 0.0);
        // Уже целые физические пиксели — точно те же значения
        assert_eq!(snap_to_pixel([6.0, 4.5], 2.0), [12.0, 9.0]);
        assert_eq!(snap_to_pixel([0.0, 0.0], 1.5), [0.0, 0.0]);
    }

    /// Свежесть кэша: тот же зум/ширина/тексты — свежий; любое изменение — нет.
    #[test]
    fn cache_freshness() {
        let entry = CacheKey {
            zoom: 1.0,
            width: 300.0,
            title: "отчёт",
            body: "тело",
            results: "",
        };
        let same = CacheKey { ..entry };
        assert!(cache_fresh(entry, same));
        assert!(
            !cache_fresh(entry, CacheKey { zoom: 1.5, ..same }),
            "зум изменился"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    width: 250.0,
                    ..same
                }
            ),
            "ширина изменилась"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    title: "смета",
                    ..same
                }
            ),
            "заголовок изменился"
        );
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    body: "иное",
                    ..same
                }
            ),
            "тело изменилось"
        );
        // FR-013: пересчитанный результат формулы инвалидирует кэш
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    results: "P:1000 ms·req/s|L:",
                    ..same
                }
            ),
            "результат формулы изменился"
        );
        // Допуски: микродрейф зума и субпиксельная ширина не инвалидируют
        assert!(cache_fresh(
            entry,
            CacheKey {
                zoom: 1.0005,
                width: 300.3,
                ..same
            }
        ));
    }

    /// Тело (T7): только у text-нод с непустым текстом и при читаемом зуме.
    #[test]
    fn body_visibility_rules() {
        let note = Node::text("n", "текст заметки", 0.0, 0.0);
        assert!(body_visible(&note, 1.0));
        // Ниже LOD-порога — не рисуем
        assert!(!body_visible(&note, 0.0));
        // Пустой текст — тела нет
        let mut empty = Node::text("n", "t", 0.0, 0.0);
        empty.text = Some(String::new());
        assert!(!body_visible(&empty, 1.0));
        // Файловые ноды тела не имеют
        let file = Node::file("n", "C:/a.png", 0.0, 0.0, 10.0, 10.0);
        assert!(!body_visible(&file, 1.0));
    }

    /// Область тела (T7): внутри карточки, под заголовком, с отступами.
    #[test]
    fn body_area_inside_card() {
        let mut note = Node::text("n", "t", 100.0, 50.0);
        note.width = 300.0;
        note.height = 200.0;
        let (origin, width, height) = body_area(&note);
        assert_eq!(
            origin,
            [100.0 + BODY_PADDING, 50.0 + HEADER_HEIGHT + BODY_TOP_GAP]
        );
        assert_eq!(width, 300.0 - BODY_PADDING * 2.0);
        assert_eq!(height, 200.0 - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING);
        // Нода меньше заголовка — размеры клампятся в ноль, без отрицательных
        let tiny = Node::text("n", "t", 0.0, 0.0);
        let (_, width, height) = body_area(&Node {
            width: 5.0,
            height: 5.0,
            ..tiny
        });
        assert_eq!(width, 0.0);
        assert_eq!(height, 0.0);
    }

    /// offset_to_cursor: байтовый offset → (строка, индекс в строке),
    /// кириллица (2 байта/символ), границы строк, кламп за концом.
    #[test]
    fn offset_to_cursor_maps_bytes() {
        let text = "аб\nвгд"; // строки по 4 и 6 байт
        assert_eq!(offset_to_cursor(text, 0), Cursor::new(0, 0));
        assert_eq!(offset_to_cursor(text, 2), Cursor::new(0, 2));
        // Конец первой строки — курсор в конец её, а не в начало следующей
        assert_eq!(offset_to_cursor(text, 4), Cursor::new(0, 4));
        // '\n' (offset 4..5) пропускается: offset 5 — начало второй строки
        assert_eq!(offset_to_cursor(text, 5), Cursor::new(1, 0));
        assert_eq!(offset_to_cursor(text, 11), Cursor::new(1, 6));
        // За концом — кламп в конец последней строки
        assert_eq!(offset_to_cursor(text, 100), Cursor::new(1, 6));
        // Текст с trailing newline: offset '\n' — конец предыдущей строки
        assert_eq!(offset_to_cursor("а\n", 2), Cursor::new(0, 2));
    }

    /// rich_spans: непрерывное покрытие текста, стили на спанах, зазоры — base.
    #[test]
    fn rich_spans_cover_text() {
        let (plain, spans) = markdown::parse("а **б** в *г*");
        let rich = rich_spans(&plain, &spans, Attrs::new());
        // Склейка спанов возвращает исходный текст
        let joined: String = rich.iter().map(|(s, _)| *s).collect();
        assert_eq!(joined, plain);
        // 5 кусков: дефолт, bold, дефолт, italic, дефолт(пустой хвост — нет)
        assert_eq!(rich.len(), 4);
        assert_eq!(rich[1].0, "б");
        assert_eq!(rich[1].1.weight, Weight::BOLD);
        assert_eq!(rich[3].0, "г");
        assert_eq!(rich[3].1.style, Style::Italic);
        assert_eq!(rich[0].1.weight, Weight::NORMAL);
    }

    /// rich_spans: базовые атрибуты (моноширинный фенс) — на всех кусках.
    #[test]
    fn rich_spans_base_attrs_on_gaps() {
        let (plain, spans) = markdown::parse("**ж** и обычный");
        let base = Attrs::new().family(Family::Monospace);
        let rich = rich_spans(&plain, &spans, base);
        // rich[0] — "ж" (bold-спан), rich[1] — зазор " и обычный" (base)
        assert_eq!(rich[0].1.family, Family::Monospace, "base на спане");
        assert_eq!(rich[0].1.weight, Weight::BOLD);
        assert_eq!(rich[1].1.family, Family::Monospace, "base на зазоре");
        assert_eq!(rich[1].1.weight, Weight::NORMAL);
    }

    /// Пустой чистый текст (только маркеры) — один пустой спан, без паники.
    #[test]
    fn rich_spans_empty() {
        let (plain, spans) = markdown::parse("****");
        let rich = rich_spans(&plain, &spans, Attrs::new());
        assert_eq!(rich.len(), 1);
        assert_eq!(rich[0].0, "");
    }

    /// decoration_quads: квады подсветки по layout runs реального буфера;
    /// количество квадов = числу видимых строк спана; зачёркивание — Strike.
    #[test]
    fn decoration_quads_follow_layout() {
        let mut fs = FontSystem::new();
        let (plain, spans) = markdown::parse("==раз два==\nтри ==четыре==");
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
        buffer.set_wrap(&mut fs, Wrap::Word);
        buffer.set_size(&mut fs, Some(400.0), Some(200.0));
        buffer.set_rich_text(
            &mut fs,
            rich_spans(&plain, &spans, Attrs::new()),
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut fs, false);
        let quads = decoration_quads(&buffer, &plain, &spans, 1.0);
        assert_eq!(quads.len(), 2, "по одному кваду на строку спана: {quads:?}");
        // Первый спан — с начала строки, второй — после слова "три "
        assert_eq!(quads[0].0[0], 0.0);
        assert!(quads[1].0[0] > 0.0, "второй спан не с края: {quads:?}");
        assert_eq!(quads[0].0[3], 20.0, "высота квада = высоте строки");
        assert!(quads[1].0[1] > quads[0].0[1], "вторая строка ниже первой");
        assert!(quads
            .iter()
            .all(|(_, kind)| *kind == BodyQuadKind::Highlight));

        // Зачёркивание — отдельный квад Strike
        let (plain, spans) = markdown::parse("~~весь текст~~");
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
        buffer.set_size(&mut fs, Some(400.0), Some(200.0));
        buffer.set_rich_text(
            &mut fs,
            rich_spans(&plain, &spans, Attrs::new()),
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut fs, false);
        let quads = decoration_quads(&buffer, &plain, &spans, 1.0);
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].1, BodyQuadKind::Strike);
        assert!(
            quads[0].0[1] > 0.0 && quads[0].0[1] < 20.0,
            "линия внутри строки: {:?}",
            quads[0]
        );

        // Без маркеров — без квадов
        let (plain, spans) = markdown::parse("без подсветки");
        let mut buffer = Buffer::new(&mut fs, Metrics::new(14.0, 20.0));
        buffer.set_size(&mut fs, Some(400.0), Some(200.0));
        buffer.set_rich_text(
            &mut fs,
            rich_spans(&plain, &spans, Attrs::new()),
            Attrs::new(),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut fs, false);
        assert!(decoration_quads(&buffer, &plain, &spans, 1.0).is_empty());
    }

    // --- shape_body: вертикальный стек GFM-блоков ---

    fn shaped(text: &str) -> BodyLayout {
        let mut fs = FontSystem::new();
        shape_body(&mut fs, &ThemeColors::dark(), text, 300.0, 1.0, &[])
    }

    /// FR-013 (правка 2): формульная строка — самостоятельный блок с
    /// source_line (привязка результата Numi-стиля), проза — обычный абзац.
    #[test]
    fn shape_body_formula_lines_own_blocks() {
        let mut fs = FontSystem::new();
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            "Gateway\nrps = 1000\nlatency = 50 ms",
            300.0,
            1.0,
            &[1, 2],
        );
        assert_eq!(
            layout.blocks.len(),
            3,
            "проза + две формульные строки-абзаца"
        );
        assert_eq!(layout.blocks[0].source_line, None, "проза");
        assert_eq!(layout.blocks[1].source_line, Some(1), "формульная строка 1");
        assert_eq!(layout.blocks[2].source_line, Some(2), "формульная строка 2");
        assert!(
            layout.blocks[1].offset[1] < layout.blocks[2].offset[1],
            "ряды формульных строк идут сверху вниз"
        );
    }

    // --- CR-009: шрифтовая пара Noto Sans Display / Noto Sans Mono ---

    /// CR-009: FONT_DATA регистрирует лица нужных весов — Noto Sans Display
    /// 500/700 и Noto Sans Mono 400/700 (статические инстансы: cosmic-text
    /// 0.12 не инстанцирует вариации вариативных шрифтов).
    #[test]
    fn font_data_registers_noto_faces() {
        let mut fs = FontSystem::new();
        for data in FONT_DATA {
            fs.db_mut().load_font_data((*data).to_vec());
        }
        for (family, weight) in [
            (SANS_FAMILY, 500u16),
            (SANS_FAMILY, 700),
            (MONO_FAMILY, 400),
            (MONO_FAMILY, 700),
        ] {
            assert!(
                fs.db().faces().any(|face| {
                    face.families.iter().any(|(name, _)| name == family) && face.weight.0 == weight
                }),
                "нет вшитого лица {family} w{weight}"
            );
        }
    }

    /// CR-009: базовые атрибуты — sans = Noto Sans Display Medium 500,
    /// mono = Noto Sans Mono (Regular 400). Явное Family::Name: дефолтный
    /// Family::Sans резолвился в отсутствующий «Fira Sans» и текст уходил
    /// системному fallback'у.
    #[test]
    fn base_attrs_pin_noto_families() {
        let sans = sans_attrs();
        assert_eq!(sans.family, Family::Name(SANS_FAMILY));
        assert_eq!(sans.weight, Weight::MEDIUM);
        let mono = mono_attrs();
        assert_eq!(mono.family, Family::Name(MONO_FAMILY));
        assert_eq!(mono.weight, Weight::NORMAL);
    }

    /// CR-009: формульная строка (source_line) — Numi-расчёт → mono; проза — sans.
    #[test]
    fn body_items_formula_line_is_mono() {
        let theme = ThemeColors::dark();
        let items = body_items(&theme, "Gateway\ndeploy = 40 $", &[1]);
        assert_eq!(items.len(), 2, "проза + формульная строка");
        assert!(!items[0].mono, "проза — sans");
        assert!(items[1].mono, "Numi-строка — моно");
        assert_eq!(items[1].source_line, Some(1));
    }

    /// CR-009: атрибуты строк буферов тела — Numi-строка Noto Sans Mono,
    /// проза Noto Sans Display Medium, GFM-заголовок Bold 700 того же
    /// семейства (family сохраняется, вес — от базы блока).
    #[test]
    fn shape_body_fonts_by_line_kind() {
        let mut fs = FontSystem::new();
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            "# План\nпросто текст\ndeploy = 40 $",
            300.0,
            1.0,
            &[2],
        );
        assert_eq!(layout.blocks.len(), 3, "заголовок + проза + формула");
        let head = layout.blocks[0].buffer.lines[0].attrs_list().defaults();
        assert_eq!(head.family, Family::Name(SANS_FAMILY), "заголовок — sans");
        assert_eq!(head.weight, Weight::BOLD, "заголовок — bold 700");
        let prose = layout.blocks[1].buffer.lines[0].attrs_list().defaults();
        assert_eq!(prose.family, Family::Name(SANS_FAMILY), "проза — sans");
        assert_eq!(prose.weight, Weight::MEDIUM, "проза — medium 500");
        let mono = layout.blocks[2].buffer.lines[0].attrs_list().defaults();
        assert_eq!(mono.family, Family::Name(MONO_FAMILY), "формула — mono");
        assert_eq!(mono.weight, Weight::NORMAL, "формула — regular 400");
    }

    /// Обычный текст — один блок Paragraph с метриками тела 14/20, offset [0,0].
    #[test]
    fn shape_body_plain_paragraph() {
        let layout = shaped("просто текст");
        assert_eq!(layout.blocks.len(), 1);
        let block = &layout.blocks[0];
        assert_eq!(block.offset, [0.0, 0.0]);
        assert_eq!(block.width, 300.0);
        assert_eq!(block.height, BODY_LINE_HEIGHT);
        assert_eq!(block.color, ThemeColors::dark().body);
        assert!(layout.quads.is_empty());
    }

    /// Заголовок + параграф: два блока, второй ниже на высоту заголовка + зазор 8.
    #[test]
    fn shape_body_heading_then_paragraph() {
        let layout = shaped("# Заголовок\nтекст");
        assert_eq!(layout.blocks.len(), 2);
        // H1: 22/28
        assert_eq!(layout.blocks[0].height, 28.0);
        assert_eq!(layout.blocks[1].offset[1], 28.0 + 8.0);
        // H4–H6 — как bold body (14/20)
        let layout = shaped("#### H4");
        assert_eq!(layout.blocks[0].height, BODY_LINE_HEIGHT);
    }

    /// Список: два пункта — два блока, зазор между ними 2, буллиты — квады.
    #[test]
    fn shape_body_list_items() {
        let layout = shaped("- a\n- b");
        assert_eq!(layout.blocks.len(), 2);
        assert_eq!(layout.blocks[0].offset[1], 0.0);
        assert_eq!(
            layout.blocks[1].offset[1],
            BODY_LINE_HEIGHT + 2.0,
            "зазор между пунктами — 2"
        );
        // Текст пункта с отступом 16
        assert_eq!(layout.blocks[0].offset[0], 16.0);
        assert_eq!(layout.blocks[0].width, 300.0 - 16.0);
        assert_eq!(
            layout
                .quads
                .iter()
                .filter(|quad| quad.kind == BodyQuadKind::Bullet)
                .count(),
            2,
            "по буллиту на пункт: {:?}",
            layout.quads
        );
    }

    /// Чекбоксы: рамка + галочка у отмеченного пункта, только рамка у пустого.
    #[test]
    fn shape_body_checkboxes() {
        let layout = shaped("- [x] done\n- [ ] todo");
        let boxes = layout
            .quads
            .iter()
            .filter(|quad| quad.kind == BodyQuadKind::CheckboxBox)
            .count();
        let ticks = layout
            .quads
            .iter()
            .filter(|quad| quad.kind == BodyQuadKind::CheckboxTick)
            .count();
        assert_eq!(boxes, 2, "рамка у обоих пунктов");
        assert!(
            ticks >= 2,
            "галочка у отмеченного (2 тонких квада): {ticks}"
        );
    }

    /// Позиции квадов списка: маркеры — в колонке-gutter слева от текста
    /// (x от левого края области тела, меньше indent=16), y — на первой
    /// строке своего блока. Регрессия: маркеры не должны уезжать к тексту
    /// или к правому краю ноды.
    #[test]
    fn shape_body_list_quad_positions() {
        let layout =
            shaped("- [ ] невыполненная задача\n- [x] выполненная задача\n- пункт маркированный");
        let boxes: Vec<[f32; 4]> = layout
            .quads
            .iter()
            .filter(|quad| quad.kind == BodyQuadKind::CheckboxBox)
            .map(|quad| quad.rect)
            .collect();
        assert_eq!(
            boxes.len(),
            2,
            "по рамке на чекбокс-пункт: {:?}",
            layout.quads
        );
        // Рамка у левого края области тела (колонка 0..16), не у текста (16+)
        // и не у правого края ноды
        for rect in &boxes {
            assert!(
                (rect[0] - 0.0).abs() < 1e-3,
                "x рамки = 0 (gutter): {rect:?}"
            );
            assert!((rect[2] - 11.0).abs() < 1e-3, "ширина рамки 11: {rect:?}");
            assert!(
                rect[0] + rect[2] <= 16.0,
                "рамка в колонке-gutter: {rect:?}"
            );
        }
        // Первая рамка — на первой строке первого блока, вторая — второго
        // (высота строки 20, зазор между пунктами 2)
        assert!(
            (boxes[0][1] - 2.0).abs() < 1e-3,
            "первая строка блока 0: {:?}",
            boxes[0]
        );
        assert!(
            (boxes[1][1] - (20.0 + 2.0 + 2.0)).abs() < 1e-3,
            "первая строка блока 1: {:?}",
            boxes[1]
        );
        // Галочки внутри своей рамки
        for rect in layout
            .quads
            .iter()
            .filter(|quad| quad.kind == BodyQuadKind::CheckboxTick)
            .map(|quad| quad.rect)
        {
            let host = boxes[1];
            assert!(
                rect[0] >= host[0] && rect[0] + rect[2] <= host[0] + host[2],
                "галочка внутри рамки: {rect:?} в {host:?}"
            );
        }
        // Буллит: x = 4, размер 5×5, на первой строке третьего блока
        let bullet = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Bullet)
            .expect("буллит есть");
        assert!(
            (bullet.rect[0] - 4.0).abs() < 1e-3,
            "x буллита = 4 (gutter): {:?}",
            bullet.rect
        );
        assert!((bullet.rect[2] - 5.0).abs() < 1e-3 && (bullet.rect[3] - 5.0).abs() < 1e-3);
    }

    /// Незакрытый фенс — код до конца текста: моноширинный блок 13/18, фон-квад.
    #[test]
    fn shape_body_unclosed_fence() {
        let layout = shaped("```\nlet a = 1;\nlet b = 2;");
        assert_eq!(layout.blocks.len(), 1);
        let block = &layout.blocks[0];
        assert_eq!(block.height, 18.0 * 2.0, "две строки по 18");
        assert_eq!(block.color, ThemeColors::dark().code_text);
        assert_eq!(block.offset[0], 6.0, "padding фенса");
        assert!(
            layout
                .quads
                .iter()
                .any(|quad| quad.kind == BodyQuadKind::CodeBg),
            "фон фенса: {:?}",
            layout.quads
        );
    }

    /// Цитата: приглушённый цвет, отступ текста 10, бар слева на всю высоту.
    #[test]
    fn shape_body_quote() {
        let layout = shaped("> цитата");
        assert_eq!(layout.blocks.len(), 1);
        let block = &layout.blocks[0];
        assert_eq!(block.color, ThemeColors::dark().quote);
        assert_eq!(block.offset[0], 10.0);
        let bar = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::QuoteBar)
            .expect("бар цитаты есть");
        assert_eq!(bar.rect[0], 0.0);
        assert!(
            (bar.rect[3] - block.height).abs() < 1e-3,
            "бар на всю высоту"
        );
    }

    /// Горизонтальная линия: квад Rule, блоков текста нет.
    #[test]
    fn shape_body_rule() {
        let layout = shaped("а\n\n---\n\nб");
        assert_eq!(layout.blocks.len(), 2, "два параграфа вокруг линии");
        let rule = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Rule)
            .expect("квад линии есть");
        assert_eq!(rule.rect[2], 300.0, "линия на всю ширину тела");
        assert_eq!(rule.rect[3], 2.0, "толщина 2px");
    }

    /// Подсветка и зачёркивание в теле дают квады Highlight/Strike; квад
    /// страйка — посередине строки с шириной глифов слова (не всей строки).
    #[test]
    fn shape_body_highlight_and_strike_quads() {
        let layout = shaped("==важно== и ~~вычеркнуто~~");
        let highlight = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Highlight)
            .expect("квад подсветки");
        assert!(
            highlight.rect[2] < 300.0 / 2.0,
            "подсветка — ширина слова, не строки: {:?}",
            highlight.rect
        );
        let strike = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Strike)
            .expect("квад зачёркивания");
        // Слово «вычеркнуто» — после «==важно== и », занимает меньше половины
        // тела: квад со смещением и реальной шириной глифов, не всей строки
        assert!(
            strike.rect[0] > 20.0,
            "страйк после префикса «==важно== и »: {:?}",
            strike.rect
        );
        assert!(
            strike.rect[2] > 20.0 && strike.rect[2] < 300.0 / 2.0,
            "страйк — реальная ширина глифов слова: {:?}",
            strike.rect
        );
        assert!(
            strike.rect[1] > BODY_LINE_HEIGHT * 0.4 && strike.rect[1] < BODY_LINE_HEIGHT * 0.7,
            "страйк посередине строки: {:?}",
            strike.rect
        );
        // Один блок: оба маркера в одном параграфе
        assert_eq!(layout.blocks.len(), 1);

        // Страйк слова в начале строки: x = 0, ширина = глифы слова
        let layout = shaped("~~вычеркнуто~~ хвост");
        let strike = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Strike)
            .expect("квад зачёркивания");
        assert!(
            strike.rect[0] < 1e-3,
            "страйк с начала слова (слово — в начале строки): {:?}",
            strike.rect
        );
        assert!(
            strike.rect[2] > 20.0 && strike.rect[2] < 300.0 / 2.0,
            "страйк — реальная ширина глифов слова: {:?}",
            strike.rect
        );
        assert!(
            strike.rect[1] > BODY_LINE_HEIGHT * 0.4 && strike.rect[1] < BODY_LINE_HEIGHT * 0.7,
            "страйк посередине строки: {:?}",
            strike.rect
        );
    }

    /// Ссылка `[a](b)` рендерится без паники и без URL в тексте: label —
    /// единственная строка буфера.
    #[test]
    fn shape_body_link_label_without_url() {
        let layout = shaped("см. [документацию](https://example.com)");
        assert_eq!(layout.blocks.len(), 1);
        let line = layout.blocks[0]
            .buffer
            .layout_runs()
            .next()
            .expect("строка есть");
        let text = line.text;
        assert!(text.contains("документацию"), "label в строке: {text}");
        assert!(!text.contains("example.com"), "URL не показываем: {text}");
    }

    /// Стек при зуме 2: квады в px виртуального буфера масштабируются.
    /// Буллит — в колонке-gutter: x = 4*zoom (без indent, он для текста).
    #[test]
    fn shape_body_quads_scale_with_zoom() {
        let mut fs = FontSystem::new();
        let layout = shape_body(&mut fs, &ThemeColors::dark(), "- a", 300.0, 2.0, &[]);
        let bullet = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::Bullet)
            .expect("буллит есть");
        assert_eq!(bullet.rect[0], 4.0 * 2.0, "x буллита = 4 * zoom (gutter)");
        assert_eq!(bullet.rect[2], 5.0 * 2.0, "размер буллита * zoom");
        assert!(
            bullet.rect[0] + bullet.rect[2] <= 16.0 * 2.0,
            "буллит в колонке-gutter: {:?}",
            bullet.rect
        );
    }
}
