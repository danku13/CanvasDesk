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
use std::sync::{Mutex, MutexGuard, OnceLock};

use canvas_core::{Canvas, Node, NodeKind};
use glyphon::{
    Attrs, Buffer, Cache, Color, Cursor, Family, FontSystem, Metrics, Resolution, Shaping, Style,
    SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport, Weight, Wrap,
};

use crate::camera::Camera;
use crate::cards::{dim_color, extension_letter, title_for, FocusView, HEADER_HEIGHT};
use crate::gfm;
use crate::markdown;
/// FR-061 этап B: табличная модель тела ноды (D-2) + проход A (D-6/D-11).
use crate::row_grid;
use crate::theme::ThemeColors;
use crate::zorder::ZPlan;
use canvas_core::expr::{line_kind, ExprLineResults, ExprOutcome, ExprResults, NumiLineKind};

/// Встроенные шрифты (SIL OFL 1.1 — см. assets/fonts/OFL-NotoSans*.txt).
/// СТАТИЧЕСКИЕ инстансы (CR-009): cosmic-text 0.12 не инстанцирует вариации
/// вариативных шрифтов (сваш рендерит дефолт-инстанс), поэтому веса 500/700
/// обязаны быть отдельными файлами: fontdb выбирает лицо по весу из OS/2.
const FONT_DATA: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansDisplay-Bold.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/NotoSansMono-Bold.ttf"),
    // FR-050 Р-2 (этап D): наклонная производная Noto Sans Mono (oblique-синтез
    // 11°, авансы/метрики сохранены; генерация — scripts/gen_oblique_font.py,
    // лицензия OFL — имя без RFN «Noto»). У Noto Sans Mono НЕТ официального
    // italic-начертания, наклонный синтез — единственный путь получить курсив
    // моно с теми же метриками (раскладка не разъезжается).
    include_bytes!("../../../assets/fonts/CanvasDeskMonoOblique.ttf"),
];

/// Семейство базового текста канваса (CR-009): Noto Sans Display.
/// FR-053 (U3): pub — измерение раскладки (canvas-ui::measure) обязано
/// использовать то же семейство, что и рендер (parity метрик).
pub const SANS_FAMILY: &str = "Noto Sans Display";
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

/// FR-050 Р-2 (этап D): семейство пролитых значений — CanvasDesk Mono
/// Oblique (наклонный синтез Noto Sans Mono, те же авансы/метрики).
pub(crate) const MONO_OBLIQUE_FAMILY: &str = "CanvasDesk Mono Oblique";

/// FR-050 Р-2 (этап D): атрибуты пролитого значения — наклонное моно
/// начертание. Значение, пришедшее по связи, визуально отличается от
/// локального (решение владельца Q9); поверхности — строка параметра
/// шаблонной ноды и авто-строка приёмника (Р-4). Метрики идентичны
/// [`mono_attrs`] — измерение и раскладка не меняются.
pub(crate) fn mono_oblique_attrs() -> Attrs<'static> {
    Attrs::new().family(Family::Name(MONO_OBLIQUE_FAMILY))
}

/// Размер заголовка в world-px (масштабируется зумом).
/// FR-023: 13 → 16 — заголовок иерархически главнее тела и набирается
/// КРУПНЕЕ основного текста: 16 / 14 ≈ +14 % (вилка владельца 10–20 %).
/// FR-046: размеры типографики — из design-токенов (dimensions.json).
use canvas_core::tokens::{
    TYPE_BADGE as BADGE_FONT_SIZE, TYPE_BADGE_LINE as BADGE_LINE_HEIGHT,
    TYPE_EDGE_LABEL as EDGE_LABEL_FONT_SIZE, TYPE_EDGE_LABEL_LINE as EDGE_LABEL_LINE_HEIGHT,
    TYPE_HUD as HUD_FONT_SIZE, TYPE_HUD_LINE as HUD_LINE_HEIGHT, TYPE_RESULT as RESULT_FONT_SIZE,
    TYPE_TITLE as TITLE_FONT_SIZE, TYPE_TITLE_LINE as TITLE_LINE_HEIGHT,
};
/// Левый отступ заголовка в world-px (без иконки).
/// FR-023: 8 → 12 — адекватный отступ заголовка от края карточки
/// (согласован с BODY_PADDING и визуальным ритмом шапки).
const TITLE_PADDING: f32 = 12.0;
/// Ширина зоны иконки-заглушки в world-px.
const ICON_WIDTH: f32 = 22.0;
/// Минимальный физический размер заголовка: ниже текст нечитаем — не готовим
/// (LOD-порог, уточняется в T11 по SPEC §6.2).
const MIN_TITLE_PX: f32 = 4.0;

/// Ширина клипа заголовка (CR-010): резерв под иконку вычитается и для
/// файловой ноды (буква расширения слева), и для шаблонной (квад-иконка
/// справа, `cards::template_icon_rect`) — иначе длинное имя шаблона
/// рисовалось под иконкой. Рендер и шейпинг используют одну формулу.
pub(crate) fn title_clip_width(node_width: f32, reserves_icon: bool) -> f32 {
    (node_width - TITLE_PADDING * 2.0 - if reserves_icon { ICON_WIDTH } else { 0.0 }).max(0.0)
}

/// Размер тела заметки в world-px (T7) — из design-токенов (FR-046).
pub use canvas_core::tokens::TYPE_BODY as BODY_FONT_SIZE;
/// Высота строки тела заметки — из design-токенов (FR-046).
pub use canvas_core::tokens::TYPE_BODY_LINE as BODY_LINE_HEIGHT;
/// Внутренний отступ тела заметки по горизонтали и снизу в world-px.
pub const BODY_PADDING: f32 = 10.0;
/// Зазор между заголовком и телом заметки в world-px.
pub const BODY_TOP_GAP: f32 = 4.0;

/// Размер шрифта лейбла связи в world-px (T8) — из design-токенов (FR-046;
/// алиас EDGE_LABEL_* — в блоке use выше).
/// Размер шрифта строки результата формулы в world-px (FR-013) — из design-токенов.
/// Высота строки результата формулы в world-px (FR-013) — резерв футера
/// карточки; приложение учитывает в fit_note_size.
pub use canvas_core::tokens::TYPE_RESULT_LINE as RESULT_LINE_HEIGHT;
/// FR-046: красный строки результата с ошибкой — слот темы `error`
/// (примитив `canvas_core::tokens::ERROR`), литерал удалён (G1).
/// FR-013 (правка 4): текст бейджа ошибки формульной строки — компактный
/// красный маркер у правого края СВОЕЙ строки; подробности — в тултипе
/// при наведении (длинные сообщения не влезают в строку ноды).
const LINE_ERROR_BADGE: &str = "!";
/// FR-013 (правка 4): расширение зоны наведения бейджа ошибки в логических
/// px в каждую сторону — один глиф «!» слишком мал для точного попадания
/// курсора.
const LINE_ERROR_HIT_PAD_PX: f32 = 10.0;

/// FR-061 этап B: высота строки авто-строки (world-px, FR-050 Р-4) —
/// метрика префикса «Переменные · входящие значения»; ячейки таблицы
/// центрируются в своей строке по этой высоте (I-1: Y-ряд не меняется).
const AUTO_ROW_LINE_HEIGHT: f32 = 18.0;
// FR-061 этап B (D-5): длина штриха/зазора и толщина линии лидера —
// с этапа E единая геометрия с китом (`canvas_ui::kit::leader_dash_rects`,
// токены TABLE_LEADER_*; локальные константы удалены — D-15).
/// FR-061 этап B (D-5): зебра — фон через строку в прогонах ≥ 4 строк данных.
const ZEBRA_RUN_MIN: usize = canvas_core::tokens::TABLE_ZEBRA_RUN_MIN;
/// FR-061 этап B (D-5): вертикаль лидера в строке (доля высоты строки —
/// базовая линия прототипа).
const LEADER_Y_FRAC: f32 = canvas_core::tokens::TABLE_LEADER_Y_FRAC;

/// FR-061 этап B: янтарный цвет unmapped-значений (Р-3) — тот же тон,
/// что пунктир unmapped-ребра (см. spill_row_items).
fn unmapped_color() -> Color {
    let c = crate::cards::UNMAPPED_EDGE_COLOR;
    Color::rgba(
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
        (c[3] * 255.0) as u8,
    )
}

/// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки —
/// логические px окна (x, y, w, h) и текст ошибки для тултипа. Собирается
/// при подготовке текста кадра, вычитывается приложением после рендера
/// (hit-тест курсора → тултип, паттерн тултипа битой ссылки T10).
#[derive(Debug, Clone)]
pub struct LineErrorHit {
    pub rect: [f32; 4],
    pub message: String,
}

/// FR-050 Н9-2 (этап D): вид проливаемой строки для тултипа источника —
/// данные (не текст): форматирование в приложении (i18n RU/EN FR-040).
/// «Param» — строка параметра шаблонной ноды, запитанная `toParam`-ребром;
/// «AutoRow» — авто-строка приёмника (Р-4, позиционный вход без читающего
/// порта). `value: None` — unmapped («не подставлено», Р-3).
#[derive(Debug, Clone, PartialEq)]
pub enum SpillHitKind {
    Param {
        param: String,
        /// Квалифицированный путь источника «Объект.Поле».
        path: String,
        value: Option<String>,
        /// Локальный литерал RHS («(локально было: 500 rps)»).
        local: Option<String>,
    },
    AutoRow {
        path: String,
        /// Позиционный слот 0-based; тултип показывает «вход $N» = slot + 1
        /// (отображение «$N» на теле ноды запрещено FR-044 Р-3).
        slot: usize,
        value: Option<String>,
        /// Приёмник — шаблонная нода: подсказка «подключите к параметру
        /// (toParam)» вместо «используйте $N в формуле» (матрица §1 FR-050).
        template: bool,
        /// FR-050 Н9-3 (этап E): id ребра-источника строки (Н10-а:
        /// строка — производная ребра) — контекст-меню параметра
        /// («Показать источник»/«Отключить проливание») адресует ребро
        /// без поиска по слоту.
        edge_id: String,
    },
}

/// FR-050 Н9-2 (этап D): зона наведения пролитой строки — логические px
/// окна (x, y, w, h) и данные тултипа источника («пролито: Трафик.peak_rps
/// = 1389 rps (локально было: 500 rps)»). Паттерн [`LineErrorHit`]:
/// собирается в цикле отрисовки тела, вычитывается приложением.
/// FR-050 Н9-3 (этап E): `node` — индекс ноды-приёмника (владелец строки):
/// контекст-меню параметра резолвит ребро по приёмнику + имени параметра.
#[derive(Debug, Clone)]
pub struct SpillHit {
    pub rect: [f32; 4],
    pub kind: SpillHitKind,
    /// Индекс ноды-приёмника в `canvas.nodes` (чьё тело рисует строку).
    pub node: usize,
}

/// FR-061 хвосты (D-7/D-8 runtime v1): род кликабельной зоны тела ноды —
/// тогглы свёрнутости блока-ведомости и экспандера описания (решения
/// владельца: клик по всей строке + chevron-ховер; автосворачивание описания).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyHitKind {
    /// Заголовок блока-ведомости (Н-2) — клик тогглит свёрнутость.
    BlockHeader,
    /// Аффорданс экспандера описания — клик тогглит раскрытость.
    DescExpander,
}

/// FR-061 хвосты: кликабельная зона тела ноды (логические px окна,
/// паттерн SpillHit/LineErrorHit) — пересобирается каждый кадр в
/// prepare_titles; приложение вычитывает после рендера для тогглов.
#[derive(Debug, Clone, Copy)]
pub struct BodyHit {
    /// [x, y, w, h] в логических px окна.
    pub rect: [f32; 4],
    pub kind: BodyHitKind,
    /// Индекс ноды в `canvas.nodes` (чьё тело рисует зону).
    pub node: usize,
}
/// Размер шрифта бейджа «=» calc-ноды при дальнем зуме (FR-013) —
/// физические px (не масштабируется зумом, как HUD) — из design-токенов
/// (алиасы BADGE_* — в блоке use выше).
/// Размер шрифта HUD в физических px (не масштабируется зумом) — из design-токенов.
/// FR-046: цвет HUD — слот темы `hud` (примитив tokens::HUD), литерал удалён (G1).
/// Отступ HUD от угла экрана в физических px.
const HUD_PADDING: f32 = 12.0;

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

/// TextArea для screen-space текста (панели, тултипы, тексты stage FR-042):
/// центровка сдвигом левого края (glyphon 0.6 не имеет set_align), снап к
/// целым физическим px — единый рендер для всех screen-текстов кадра.
fn screen_text_area<'a>(
    buffer: &'a Buffer,
    st: &ScreenText<'_>,
    scale_factor: f32,
) -> TextArea<'a> {
    let mut left = st.origin[0] * scale_factor;
    if st.align == TextAlign::Center {
        let line_w = buffer
            .layout_runs()
            .next()
            .map(|run| run.line_w)
            .unwrap_or(0.0);
        left += ((st.width * scale_factor) - line_w).max(0.0) / 2.0;
    }
    let left = left.round();
    let top = (st.origin[1] * scale_factor).round();
    let line_height = st.font_size * scale_factor * 1.3;
    TextArea {
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
    }
}

/// FR-056 (F-5 PRD-0009): клип текста полосы — пересечение собственных
/// `TextBounds` текста со scissor-бакетом полосы (физ. px; конверсия
/// логического клипа — та же чистая функция [`crate::renderer::
/// band_scissor_rect`], что и у квадов). Пустое пересечение → `None` —
/// текст за клипом полосы не готовится вовсе (инвариант «клип не
/// расширяет видимое»; scissor на текст-группу не тратится).
fn clip_text_bounds(own: TextBounds, clip: (i32, i32, i32, i32)) -> Option<TextBounds> {
    let left = own.left.max(clip.0);
    let top = own.top.max(clip.1);
    let right = own.right.min(clip.2);
    let bottom = own.bottom.min(clip.3);
    if right > left && bottom > top {
        Some(TextBounds {
            left,
            top,
            right,
            bottom,
        })
    } else {
        None
    }
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

/// FR-025: world-вертикаль ряда результата формульной строки — ЕДИНЫЙ
/// расчёт для бейджа результата (FR-013) и построчного порта: инвариант
/// вертикали (порт и бейдж не разъезжаются ни при каком зуме/ширине).
/// `block_offset_y` — смещение блока тела с `source_line` этой строки.
pub fn result_row_y(node: &Node, block_offset_y: f32) -> f32 {
    body_area(node).0[1] + block_offset_y + (BODY_LINE_HEIGHT - RESULT_LINE_HEIGHT) / 2.0
}

/// FR-061 этап B: блок тела по исходной строке — общий поиск для портов/
/// якорей (строки таблицы ссылаются на ту же геометрию блоков, I-1).
fn entry_body_block(entry: &CachedTitle, source_line: usize) -> Option<&BodyBlock> {
    entry
        .body
        .as_ref()?
        .blocks
        .iter()
        .find(|block| block.source_line == Some(source_line))
}

/// FR-061 этап B (D-4): зашейпить ячейку строки таблицы (значение/юнит/
/// бейдж) — метрики строки результата (RESULT_*), база передаётся вызывающим
/// (моно/наклонное моно Р-2); цвет ячейки — default_color TextArea (как у
/// результатов FR-013). Пустой текст — ячейки нет.
fn shape_row_cell(
    font_system: &mut FontSystem,
    text: &str,
    attrs: Attrs<'static>,
    color: Color,
    area_px: f32,
    zoom_px: f32,
) -> Option<CachedCell> {
    if text.is_empty() {
        return None;
    }
    let mut buffer = Buffer::new(
        font_system,
        Metrics::new(RESULT_FONT_SIZE * zoom_px, RESULT_LINE_HEIGHT * zoom_px),
    );
    buffer.set_wrap(font_system, Wrap::None);
    buffer.set_size(
        font_system,
        Some(area_px),
        Some(RESULT_LINE_HEIGHT * zoom_px),
    );
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);
    let width_px = buffer
        .layout_runs()
        .next()
        .map(|run| run.line_w)
        .unwrap_or(0.0);
    Some(CachedCell {
        buffer,
        width_px,
        color,
    })
}

/// FR-025: world-вертикаль ряда результата шаблонной/expr-ноды — центр
/// футера результата (FR-023): узловое значение сидит в футере карточки.
pub fn result_footer_y(node: &Node) -> f32 {
    node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT / 2.0
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
    /// FR-017 (CP6): фон подменённой what-if строки (акцентный тинт поверх
    /// CodeBg формульной строки).
    WhatIfBg,
    /// Горизонтальная линия `---`.
    Rule,
    /// FR-061 этап B (D-5): пунктирная дорожка лидера от конца формулы
    /// до направляющей чисел (паттерн чека/оглавления, прототип O-2).
    /// Рисуется штрихами 2/3 px на базовой линии строки данных.
    Leader,
    /// FR-061 этап B (D-5): фон зебры — полупрозрачная подложка через
    /// строку в прогонах ≥ 4 строк данных (прототип O-7).
    RowBg,
    /// FR-061 этап D (D-14/Q9): вертикальная линия диагностики колоночной
    /// направляющей (value/unit right-края) — рисуется ТОЛЬКО при
    /// включённом DebugOverlay (F9/?ui=debug), на ноде невидима (Q9).
    GuideDebug,
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
    /// FR-061 этап B: ширина ЗАШЕЙПЛЕННОГО текста блока (px буфера) —
    /// конец левого текста строки данных (старт лидера D-5, проход A
    /// row_grid::pass_a).
    line_w: f32,
    /// Цвет текста блока (цитата/код приглушены/акцентные).
    color: Color,
    /// FR-013 (правка 2): строка исходного текста — у блоков формульных
    /// строк (None — обычный блок из сплошного сегмента).
    source_line: Option<usize>,
    /// FR-061 этап C (D-7): заголовок блока-ведомости — привязка Σ-строки.
    header: bool,
    /// FR-061 хвосты (D-7 runtime v1): превью-строка СВЁРНУТОЙ ведомости
    /// («параметры · P · формулы · K» + Σ первого расчёта на направляющей) —
    /// привязка ячейки превью (RowKind::Preview), как у заголовка (Total).
    preview: bool,
    /// FR-061 хвосты (D-8 runtime v1): аффорданс экспандера описания
    /// «⋯ целиком ▾»/«▴ свернуть» — кликабельная строка (hit-зона app.rs).
    expander: bool,
    /// FR-050 Н9-2 (этап D): данные тултипа проливания — блок
    /// пролитой строки (авто-строка/параметр); hit-зона собирается
    /// в цикле отрисовки тела.
    spill: Option<SpillHitKind>,
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
#[derive(Clone)]
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
    /// FR-050 Р-2 (этап D): наклонное моно-начертание — пролитое значение
    /// (CanvasDesk Mono Oblique, метрики те же, что у mono).
    oblique: bool,
    /// FR-050 Н9-2 (этап D): данные тултипа проливания (пролитая строка).
    spill: Option<SpillHitKind>,
    /// FR-061 этап C (D-7): заголовок блока-ведомости «▸ расчёт · N строк»
    /// — привязка Σ-ячейки (RowKind::Total) к своему блоку.
    header: bool,
    /// FR-061 хвосты (D-7 runtime v1): превью-строка свёрнутой ведомости —
    /// привязка ячейки «Σ первое-значение» (RowKind::Preview).
    preview: bool,
    /// FR-061 хвосты (D-8 runtime v1): аффорданс экспандера описания —
    /// кликабельная строка (BodyHit::DescExpander, hit-зона app.rs).
    expander: bool,
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
/// FR-050 Р-2 (этап D): строка-присваивание пролитого параметра
/// (spill_params — строка с подписью источника «param ← Источник · выход»)
/// помечается наклонным моно-начертанием + данными тултипа Н9-2.
fn body_items(
    theme: &ThemeColors,
    body_text: &str,
    formula_lines: &[usize],
    spill_params: &[crate::SpillView],
    language: canvas_core::Language,
    block_expanded: bool,
    // FR-067 (этап F): диапазон строк первого проза-абзаца, показанного
    // зоной описания — из вёрстки тела он убран (супрессия дубликата),
    // сам текст ноды не меняется (I-1/I-3). None — супрессии нет.
    suppress: Option<(usize, usize)>,
) -> Vec<BodyItem> {
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
    // FR-067 (этап F): супрессия — None-сегменты, пересекающие диапазон
    // абзаца, дробятся на части ДО/ПОСЛЕ диапазона (сами строки абзаца
    // не рендерятся). Формульные сегменты (Some) не трогаются: строки
    // абзаца — проза, в формула-строки не попадают (по построению
    // first_prose_paragraph_span). Привязки блоков/портов не страдают:
    // у None-сегментов source_line нет, hit-зоны и порты живут только на
    // строках с исходами (прецедент — выбрасывание свёрнутых строк).
    let segments: Vec<(usize, usize, Option<usize>)> = match suppress {
        None => segments,
        Some((sup_start, sup_end)) => segments
            .into_iter()
            .flat_map(|(start, end, source_line)| {
                if source_line.is_some() || end <= sup_start || start >= sup_end {
                    return vec![(start, end, source_line)];
                }
                let mut parts = Vec::with_capacity(2);
                if start < sup_start {
                    parts.push((start, sup_start, None));
                }
                if sup_end < end {
                    parts.push((sup_end, end, None));
                }
                parts
            })
            .collect(),
    };
    // FR-061 этап C (D-7): заголовок блока-ведомости — вставка перед
    // ПЕРВОЙ расчётной строкой при числе данных > T (Q2); общий расчёт
    // для рендера и измерения (один body_items в общем стеке, I-2).
    // FR-061 хвосты (D-7 runtime v1): при свёрнутом блоке (дефолт —
    // развёрнут; свёрнутость = явный клик, сброс при перезагрузке) расчётные
    // сегменты НЕ рендерятся — ведомость представлена заголовком + превью
    // (прототип .blk-hdr + .preview-row); блоки расчётных строк исчезают —
    // строки без блоков выбрасываются циклом привязки (порты тоже).
    let header_plan = block_header_plan(&lines, formula_lines);
    let collapsed = header_plan.is_some() && !block_expanded;
    for (seg_start, seg_end, source_line) in segments {
        if let Some((header_line, calc_count)) = header_plan {
            if source_line == Some(header_line) {
                push_item(
                    &mut out,
                    &mut prev,
                    (true, false, None),
                    block_header_item(theme, calc_count, language, block_expanded),
                );
                if collapsed {
                    // Превью свёрнутой ведомости: «параметры · P · формулы · K».
                    let param_count = formula_lines
                        .iter()
                        .filter(|&i| {
                            lines.get(*i).is_some_and(|line| {
                                matches!(line_kind(line), NumiLineKind::Assignment { .. })
                            })
                        })
                        .count();
                    push_item(
                        &mut out,
                        &mut prev,
                        (true, false, None),
                        block_preview_item(theme, param_count, calc_count, language),
                    );
                }
            }
        }
        // Свёрнутый блок: расчётная строка (не-присваивание с исходной
        // строкой) не рендерится — блок не создаётся (строки без блоков
        // выбрасываются циклом привязки — ячейки и порты исчезают).
        if collapsed
            && source_line.is_some_and(|line| {
                lines
                    .get(line)
                    .is_some_and(|l| !matches!(line_kind(l), NumiLineKind::Assignment { .. }))
            })
        {
            continue;
        }
        let seg_text = lines[seg_start..seg_end].join("\n");
        // FR-050: пролитая строка параметра — наклонное начертание Р-2
        // и данные тултипа источника (Н9-2).
        let spill =
            source_line.and_then(|line| spill_params.iter().find(|spill| spill.line == Some(line)));
        push_gfm_blocks(
            theme,
            &seg_text,
            source_line,
            spill,
            &mut out,
            &mut prev,
            &mut list_id,
        );
    }
    out
}

/// FR-061 этап C (D-7): план заголовка блока-ведомости — `Some((первая
/// расчётная строка, число расчётных))`, когда строк данных (параметры +
/// расчёт — все строки с исходами; авто-строки не входят по построению)
/// больше порога T (Q2, дефолт 4) и среди них есть расчётные. Чистая
/// функция — рендер и измерение не разъезжаются (I-2).
fn block_header_plan(lines: &[&str], formula_lines: &[usize]) -> Option<(usize, usize)> {
    let calc: Vec<usize> = formula_lines
        .iter()
        .copied()
        .filter(|&i| {
            lines
                .get(i)
                .is_some_and(|line| !matches!(line_kind(line), NumiLineKind::Assignment { .. }))
        })
        .collect();
    if formula_lines.len() > canvas_core::NODE_BODY_BLOCK_THRESHOLD && !calc.is_empty() {
        Some((*calc.first()?, calc.len()))
    } else {
        None
    }
}

/// FR-061 этап C (D-7): элемент-заголовок блока «▸/▾ расчёт · N строк» —
/// моно-жирный, приглушённый цвет кода; высота строки тела (I-1).
/// D-14 (этап D): текст локализован ([`row_grid::block_header_text_lang`]).
/// FR-061 хвосты (D-7 runtime v1): шеврон состояния — ▾ развёрнут, ▸ свёрнут.
fn block_header_item(
    theme: &ThemeColors,
    calc_count: usize,
    language: canvas_core::Language,
    expanded: bool,
) -> BodyItem {
    BodyItem {
        gap: 0.0, // push_item пересчитает по предыдущему блоку
        rule: false,
        text: row_grid::block_header_text_lang(calc_count, language, expanded),
        font_size: BODY_FONT_SIZE,
        line_height: BODY_LINE_HEIGHT,
        color: theme.code_text,
        indent: 0.0,
        mono: true,
        bold: true,
        deco: ItemDeco::None,
        source_line: None,
        oblique: false,
        spill: None,
        header: true,
        preview: false,
        expander: false,
    }
}

/// FR-061 хвосты (D-7 runtime v1): превью-строка свёрнутой ведомости —
/// «параметры · P · формулы · K» (прототип .preview-row): sans, приглушённый
/// тон (слабее заголовка), высота строки тела (I-1). Ячейка значения
/// («Σ первый-расчёт») добавляется ряд-таблицей RowKind::Preview.
fn block_preview_item(
    theme: &ThemeColors,
    param_count: usize,
    calc_count: usize,
    language: canvas_core::Language,
) -> BodyItem {
    BodyItem {
        gap: 0.0, // push_item пересчитает по предыдущему блоку
        rule: false,
        text: row_grid::block_preview_text_lang(param_count, calc_count, language),
        font_size: BODY_FONT_SIZE,
        line_height: BODY_LINE_HEIGHT,
        color: theme.quote,
        indent: 0.0,
        mono: false,
        bold: false,
        deco: ItemDeco::None,
        source_line: None,
        oblique: false,
        spill: None,
        header: false,
        preview: true,
        expander: false,
    }
}

/// FR-050 Н9-2 (этап D): данные тултипа пролитой строки параметра —
/// из представления проливания сцены (path/local собирает recompute_flow).
fn spill_hit_param(view: &crate::SpillView) -> SpillHitKind {
    SpillHitKind::Param {
        param: view.param.clone(),
        path: view.path.clone(),
        value: view.value.clone(),
        local: view.local.clone(),
    }
}

/// FR-050 Р-4 (этап D): элементы авто-строк приёмника — ПРЕФИКС тела
/// (зона «Переменные · входящие значения», FR-045 R-4 / PRD-0004 зона C):
/// одна строка-блок на каждое ребро без ожидающего порта. Наклонное моно
/// начертание Р-2; unmapped — янтарный акцент анализа (Р-3). Зазоры:
/// первый — 0, между строками — 2 (плотный список переменных).
fn spill_row_items(
    theme: &ThemeColors,
    rows: &[canvas_core::flow::AutoRow],
    template: bool,
) -> Vec<BodyItem> {
    let mut out = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let unmapped = row.value.is_none();
        // Р-3: unmapped — янтарный акцент анализа (тот же тон, что
        // пунктир unmapped-ребра UNMAPPED_EDGE_COLOR, f32 → u8).
        let amber = unmapped_color();
        // FR-061 этап B: левая часть авто-строки — ТОЛЬКО имя (путь);
        // значение/юнит — ячейки таблицы на направляющих (D-2/D-4),
        // высота строки прежняя (I-1). display_text сохранён для ключа
        // кэша (значение upstream меняет строку → перешейп).
        out.push(BodyItem {
            gap: if i == 0 { 0.0 } else { 2.0 },
            rule: false,
            text: row.path.clone(),
            font_size: 13.0,
            line_height: AUTO_ROW_LINE_HEIGHT,
            color: if unmapped { amber } else { theme.code_text },
            indent: 6.0,
            mono: true,
            bold: false,
            deco: ItemDeco::None,
            source_line: None,
            oblique: true,
            spill: Some(SpillHitKind::AutoRow {
                path: row.path.clone(),
                slot: row.slot,
                value: row.value.as_ref().map(|v| v.to_string()),
                template,
                edge_id: row.edge_id.clone(),
            }),
            header: false,
            preview: false,
            expander: false,
        });
    }
    out
}

/// Разобрать сегмент GFM-блоков и допушить элементы тела; `source_line`
/// проставляется элементам (Some — у однострочного сегмента формульной
/// строки). `spill` — проливание в строку этого сегмента (наклонное
/// начертание Р-2 + данные тултипа Н9-2; None — обычная строка).
fn push_gfm_blocks(
    theme: &ThemeColors,
    seg_text: &str,
    source_line: Option<usize>,
    spill: Option<&crate::SpillView>,
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
                        oblique: false,
                        spill: None,
                        header: false,
                        preview: false,
                        expander: false,
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
                        // FR-050 Р-2 (этап D): пролитая строка параметра —
                        // наклонное моно-начертание (различение пролитого и
                        // локального) + данные тултипа источника (Н9-2).
                        oblique: spill.is_some(),
                        spill: spill.map(spill_hit_param),
                        header: false,
                        preview: false,
                        expander: false,
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
                        oblique: false,
                        spill: None,
                        header: false,
                        preview: false,
                        expander: false,
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
                        oblique: false,
                        spill: None,
                        header: false,
                        preview: false,
                        expander: false,
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
                        oblique: false,
                        spill: None,
                        header: false,
                        preview: false,
                        expander: false,
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
                            oblique: false,
                            spill: None,
                            header: false,
                            preview: false,
                            expander: false,
                        },
                    );
                }
            }
            // FR-027: таблицы в заметках не парсятся (`parse_blocks` — флаг
            // выкл, инвариант регрессии), ветка недостижима; рендер таблиц —
            // только в просмотрщике документации (docs_ui).
            gfm::Block::Table { .. } => {}
        }
    }
}

/// FR-061 этап D (O-5, D-4): rich-раны формульной строки — чисто
/// визуальная раскраска по прототипу (ux-node-body-fill): идентификатор,
/// за которым следует «(» — функция (formula_fn, курсив); операторные
/// символы — приглушённый тон (formula_op); числа/переменные — базовые
/// атрибуты. РАЗБОР ГРАММАТИКИ НЕ ДУБЛИРУЕТСЯ: оценка строки уже вычислена
/// движком (line_kind/eval_lines) — здесь только классификация символов
/// для цвета. Побочный фикс: маркеры GFM (`*`, `==`, `~~`) в формулах
/// больше не интерпретируются (умножение «a * 2 * 3» раньше попадало в
/// italic-спан markdown-парсера). Возвращает раны для `set_rich_text`.
fn formula_rich_runs<'a, 'r>(
    text: &'a str,
    base: Attrs<'r>,
    fn_color: Color,
    op_color: Color,
) -> Vec<(&'a str, Attrs<'r>)> {
    fn is_ident(c: char) -> bool {
        c.is_alphanumeric() || c == '_' || c == '.' || c == '\''
    }
    fn is_op(c: char) -> bool {
        matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '=' | '<' | '>' | '!')
    }
    // (байтовый офсет, символ)
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let byte_at = |idx: usize| chars.get(idx).map(|&(b, _)| b).unwrap_or(text.len());
    let mut out: Vec<(&'a str, Attrs)> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let (_, c) = chars[i];
        if is_op(c) {
            let start = i;
            while i < chars.len() && is_op(chars[i].1) {
                i += 1;
            }
            out.push((&text[byte_at(start)..byte_at(i)], base.color(op_color)));
            continue;
        }
        if is_ident(c) && !c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && is_ident(chars[i].1) {
                i += 1;
            }
            // Идентификатор, за которым (после пробелов) следует «(» — функция.
            let mut peek = i;
            while peek < chars.len() && chars[peek].1 == ' ' {
                peek += 1;
            }
            let is_fn = peek < chars.len() && chars[peek].1 == '(';
            let attrs = if is_fn {
                base.color(fn_color).style(Style::Italic)
            } else {
                base
            };
            out.push((&text[byte_at(start)..byte_at(i)], attrs));
            continue;
        }
        // Прочее (пробелы, скобки, запятые, числа) — базовые атрибуты.
        let start = i;
        while i < chars.len()
            && !is_op(chars[i].1)
            && !(is_ident(chars[i].1) && !chars[i].1.is_ascii_digit())
        {
            i += 1;
        }
        out.push((&text[byte_at(start)..byte_at(i)], base));
    }
    // Пустые раны не пушим; смежные раны одного стиля не сливаем —
    // set_rich_text корректно шейпит соседние раны (оптимизация не нужна).
    let mut merged: Vec<(&'a str, Attrs)> =
        out.into_iter().filter(|(s, _)| !s.is_empty()).collect();
    if merged.is_empty() {
        merged.push((text, base));
    }
    merged
}

/// Зашейпить один текстовый блок тела: буфер с переносами по ширине области
/// блока (px), высота по layout_runs (px буфера). Квады подсветки/
/// зачёркивания — в px буфера блока; буллиты/чекбоксы позиционируются по
/// первой строке снаружи. `formula` — формульная/параметрская строка
/// (mono + source_line): раскраска лексем O-5, GFM-маркеры не парсятся.
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
    formula: bool,
) -> (Buffer, f32, Vec<([f32; 4], BodyQuadKind)>) {
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
    // WordOrGlyph: перенос по словам; слишком длинное слово рвётся по глифам,
    // а не вылезает за карточку. Высота None — shape_until_scroll зашейпит
    // ВСЕ строки (scroll_end = бесконечность, buffer.rs cosmic-text).
    buffer.set_wrap(font_system, Wrap::WordOrGlyph);
    buffer.set_size(font_system, Some(width_px), None);
    // FR-061 этап D (O-5): формульная строка — раскраска лексем БЕЗ
    // GFM-парсинга (маркеры `*`/`==`/`~~` в формулах — литералы).
    if formula {
        let rich = formula_rich_runs(text, base, theme.formula_fn, theme.formula_op);
        buffer.set_rich_text(
            font_system,
            rich.iter().map(|&(s, attrs)| (s, attrs)),
            base,
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(font_system, false);
        let height_px = buffer
            .layout_runs()
            .last()
            .map_or(0.0, |run| run.line_top + run.line_height);
        return (buffer, height_px, Vec::new());
    }
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

/// Общая геометрия вертикального стека тела (CR-012, правка 2): единый
/// проход `gap → shape → cursor_y += height`, который делят рендер
/// (`shape_body`) и измерение (`measure_body_height`) — метрики стека
/// определены в одном месте, измерение и рендер не могут разъехаться.
/// `on_block` получает элемент, зашейпленный буфер, высоту в px буфера,
/// высоту в world-px, ширину области блока (world-px), Y верха блока
/// (world-px, после зазора) и блок-квады для раскладки; `on_rule` — Y верха
/// линии `---` (world-px, после зазора, до прибавки 12). Декоративные квады
/// оба колбэка складывают в `quads_out`. Возвращает полную высоту стека
/// в world-px.
/// FR-050 (этап D): `spill_prefix` — элементы авто-строк приёмника (Р-4,
/// верх тела, зона «Переменные · входящие значения») — идут ДО собственных
/// строк тела; `spill_params` — пролитые строки параметров (Р-2 наклонное
/// начертание, Н9-2 данные тултипа).
#[allow(clippy::too_many_arguments)]
fn with_body_stack(
    font_system: &mut FontSystem,
    theme: &ThemeColors,
    body_text: &str,
    body_width: f32,
    zoom_px: f32,
    formula_lines: &[usize],
    spill_prefix: Vec<BodyItem>,
    spill_params: &[crate::SpillView],
    language: canvas_core::Language,
    desc: Option<&str>,
    // FR-061 хвосты (D-7/D-8 runtime v1): состояние свёрнутости блока
    // ведомости и раскрытости описания (решения владельца: дефолты —
    // развёрнут/кламп; состояние runtime, в .canvas не пишется — Q4).
    block_expanded: bool,
    desc_expanded: bool,
    quads_out: &mut Vec<BodyQuad>,
    mut on_block: impl FnMut(
        &BodyItem,
        Buffer,
        f32,
        f32,
        f32,
        f32,
        Vec<([f32; 4], BodyQuadKind)>,
        &mut Vec<BodyQuad>,
    ),
    mut on_rule: impl FnMut(f32, &mut Vec<BodyQuad>),
) -> f32 {
    let mut cursor_y = 0.0f32; // world-px, верх текущего элемента
                               // FR-050 Р-4: авто-строки — префикс стека (до собственного тела).
                               // FR-061 этап D (D-8): зона описания — САМАЯ первая (до чисел/авто-строк),
                               // кламп 2 строки (токен TABLE_DESC_CLAMP_LINES).
                               // FR-061 хвосты (D-8 runtime v1, «Раскрыть+авто»): desc_expanded —
                               // кламп не применяется (полный текст); при усечении клампом ИЛИ в
                               // раскрытом состоянии добавляется аффорданс-строка экспандера
                               // («⋯ целиком ▾» / «▴ свернуть») — hit-зона BodyHit::DescExpander.
    let desc_items: Vec<BodyItem> = match desc {
        Some(d) if !d.is_empty() => {
            let (clamped, truncated) = clamp_desc_text(
                font_system,
                d,
                body_width * zoom_px,
                canvas_core::tokens::TABLE_DESC_CLAMP_LINES,
            );
            if clamped.is_empty() {
                Vec::new()
            } else {
                let mut items = vec![desc_zone_item(theme, clamped)];
                if desc_expanded || truncated {
                    let text = if desc_expanded {
                        row_grid::desc_collapse_text_lang(language)
                    } else {
                        row_grid::desc_expand_text_lang(language)
                    };
                    items.push(desc_expander_item(theme, text));
                }
                items
            }
        }
        _ => Vec::new(),
    };
    // Зазор между зоной описания и следующей зоной (авто-строки/тело).
    let mut spill_prefix = spill_prefix;
    if !desc_items.is_empty() {
        if let Some(first) = spill_prefix.first_mut() {
            first.gap = first.gap.max(6.0);
        }
    }
    let mut items: Vec<BodyItem> = desc_items;
    // FR-050 Р-4: авто-строки — после зоны описания, до собственного тела.
    items.extend(spill_prefix);
    // FR-067 (этап F): супрессия первого проза-абзаца — зона описания
    // показывает ЕГО ЖЕ текст (фолбэк Q3 «desc→манифест→проза», либо
    // canvasdesk.desc/манифест, дословно равный абзацу) → из вёрстки тела
    // абзац убран, зона не дублируется. Сам текст ноды не меняется —
    // индексы формул/портов/проливаний стабильны (I-1/I-3). Деривация по
    // равенству через ОБЩИЕ чистые функции ядра — рендер и измерение
    // супрессируют одинаково (I-2), контракты стека не расширяются.
    let suppress = desc
        .filter(|d| !d.is_empty())
        .and_then(|d| {
            canvas_core::expr::first_prose_paragraph(body_text)
                .filter(|para| para == d)
                .and_then(|_| canvas_core::expr::first_prose_paragraph_span(body_text))
        });
    let mut body = body_items(
        theme,
        body_text,
        formula_lines,
        spill_params,
        language,
        block_expanded,
        suppress,
    );
    // Зона «Переменные» отделяется от собственного контента зазором
    // (первый элемент тела в покое имеет gap 0 — переопределяем).
    if !items.is_empty() {
        if let Some(first) = body.first_mut() {
            first.gap = first.gap.max(6.0);
        }
    }
    items.extend(body);
    for item in items {
        cursor_y += item.gap;
        if item.rule {
            // Линия: высота блока 12, квад толщиной 2 по центру
            on_rule(cursor_y, quads_out);
            cursor_y += 12.0;
            continue;
        }
        let block_width = (body_width - item.indent).max(0.0);
        // CR-009: базис посемейственно — Noto Sans Mono (Numi-строки, фенсы)
        // или Noto Sans Display Medium (прочее); жирные GFM-заголовки —
        // Weight::BOLD (700) того же семейства.
        // FR-050 Р-2: пролитое значение — наклонная производная моно
        // (CanvasDesk Mono Oblique, авансы/метрики те же).
        let mut base = if item.oblique {
            mono_oblique_attrs()
        } else if item.mono {
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
            // O-5: формульная/параметрская строка (mono + привязка к строке
            // текста) — раскраска лексем; авто-строки/заголовок — нет.
            item.mono && item.source_line.is_some() && !item.oblique,
        );
        let height = height_px / zoom_px;
        on_block(
            &item,
            buffer,
            height_px,
            height,
            block_width,
            cursor_y,
            quads,
            quads_out,
        );
        cursor_y += height;
    }
    cursor_y
}

/// FR-061 этап D (D-8): кламп текста описания по ЧИСЛУ СТРОК фактической
/// верстки (тот же Wrap::WordOrGlyph и кегль, что у стека тела — паритет
/// с мерой CR-012): укладывается в `max_lines` — возвращается целиком;
/// иначе — обрезка по словам + «…» в последней строке. Космический буфер
/// здесь обязателен (кламп должен совпадать с версткой стека); TextMeasurer
/// (canvas-ui) сознательно остаётся без космических буферов.
fn clamp_desc_text(
    font_system: &mut FontSystem,
    text: &str,
    width_px: f32,
    max_lines: usize,
) -> (String, bool) {
    let wrapped_lines = |fs: &mut FontSystem, s: &str| -> usize {
        let mut buffer = Buffer::new(fs, Metrics::new(BODY_FONT_SIZE, BODY_LINE_HEIGHT));
        buffer.set_wrap(fs, Wrap::WordOrGlyph);
        buffer.set_size(fs, Some(width_px), None);
        buffer.set_text(fs, s, sans_attrs(), Shaping::Advanced);
        buffer.shape_until_scroll(fs, false);
        buffer.layout_runs().count()
    };
    if text.trim().is_empty() {
        return (String::new(), false);
    }
    if wrapped_lines(font_system, text) <= max_lines {
        return (text.to_owned(), false);
    }
    // Обрезка по словам: наибольший префикс, чей кандидат с «…» укладывается
    // в max_lines строк (детерминированный бинарный поиск).
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lo = 0usize;
    let mut hi = words.len();
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate = format!("{} …", words[..mid].join(" "));
        if wrapped_lines(font_system, &candidate) <= max_lines {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if lo == 0 {
        return ("…".to_owned(), true);
    }
    (format!("{} …", words[..lo].join(" ")), true)
}

/// FR-061 этап D (D-8): элемент зоны описания — первая зона тела (до
/// чисел), sans, приглушённый тон цитаты. Источник — Q3 (решение владельца
/// 2026-09-23, «desc→манифест→проза»): `canvasdesk.desc` → описание
/// манифеста шаблона → первый проза-абзац текста
/// ([`canvas_core::expr::first_prose_paragraph`]).
fn desc_zone_item(theme: &ThemeColors, text: String) -> BodyItem {
    BodyItem {
        gap: 0.0, // первый в стеке; зазор после зоны — у следующего элемента
        rule: false,
        text,
        font_size: BODY_FONT_SIZE,
        line_height: BODY_LINE_HEIGHT,
        color: theme.quote,
        indent: 0.0,
        mono: false,
        bold: false,
        deco: ItemDeco::None,
        source_line: None,
        oblique: false,
        spill: None,
        header: false,
        preview: false,
        expander: false,
    }
}

/// FR-061 хвосты (D-8 runtime v1): аффорданс экспандера описания — строка
/// после текста описания («⋯ целиком ▾» при клампе / «▴ свернуть» в
/// раскрытом состоянии), приглушённый акцент цитаты, sans; кликабельна
/// (BodyHit::DescExpander, hit-зона app.rs). Высота строки тела (I-1).
fn desc_expander_item(theme: &ThemeColors, text: String) -> BodyItem {
    BodyItem {
        gap: 0.0,
        rule: false,
        text,
        font_size: BODY_FONT_SIZE,
        line_height: BODY_LINE_HEIGHT,
        color: theme.link,
        indent: 0.0,
        mono: false,
        bold: false,
        deco: ItemDeco::None,
        source_line: None,
        oblique: false,
        spill: None,
        header: false,
        preview: false,
        expander: true,
    }
}

/// FR-061 этап D (D-14/Q9): квады диагностики колоночных направляющих —
/// вертикальные линии на right-краях колонок (значение/юнит) через зону
/// строк данных таблицы. Толщина 1 world-px, цвет — токен
/// TABLE_GUIDE_DEBUG_COLOR (маппинг в renderer::body_quad_fill). Чистая
/// функция — юнит-тест без GPU.
fn guide_debug_quads(
    guides: &canvas_ui::row_guides::RowGuides,
    rows: &[CachedRow],
    zoom_px: f32,
) -> Vec<BodyQuad> {
    let Some((y0, y1)) = rows.iter().fold(None::<(f32, f32)>, |acc, row| {
        Some(match acc {
            None => (row.row_top, row.row_top + row.row_line_h),
            Some((a, b)) => (a.min(row.row_top), b.max(row.row_top + row.row_line_h)),
        })
    }) else {
        return Vec::new();
    };
    let z = zoom_px;
    vec![
        BodyQuad {
            rect: [guides.value_right() * z, y0 * z, z, (y1 - y0) * z],
            kind: BodyQuadKind::GuideDebug,
        },
        BodyQuad {
            rect: [guides.unit_right() * z, y0 * z, z, (y1 - y0) * z],
            kind: BodyQuadKind::GuideDebug,
        },
    ]
}

/// Зашейпить тело заметки: GFM-блоки → вертикальный стек буферов со своими
/// метриками/цветами + декоративные квады (в px виртуального буфера тела).
/// `body_width` — world-px, `zoom_px` — физический зум. GPU не нужен —
/// функция тестируема с настоящим FontSystem. Стек блоков — общий
/// [`with_body_stack`] (с измерением не разъезжается).
#[allow(clippy::too_many_arguments)]
fn shape_body(
    font_system: &mut FontSystem,
    theme: &ThemeColors,
    body_text: &str,
    body_width: f32,
    zoom_px: f32,
    formula_lines: &[usize],
    whatif_lines: &[usize],
    spill_prefix: Vec<BodyItem>,
    spill_params: &[crate::SpillView],
    language: canvas_core::Language,
    desc: Option<&str>,
    // FR-061 хвосты (D-7/D-8 runtime v1): см. with_body_stack.
    block_expanded: bool,
    desc_expanded: bool,
) -> BodyLayout {
    let mut blocks: Vec<BodyBlock> = Vec::new();
    let mut quads: Vec<BodyQuad> = Vec::new();
    with_body_stack(
        font_system,
        theme,
        body_text,
        body_width,
        zoom_px,
        formula_lines,
        spill_prefix,
        spill_params,
        language,
        desc,
        block_expanded,
        desc_expanded,
        &mut quads,
        |item, buffer, height_px, height, block_width, cursor_y, block_quads, quads| {
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
                    quads.push(BodyQuad {
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
                    quads.push(BodyQuad {
                        rect: [0.0, y, 11.0 * z, 11.0 * z],
                        kind: BodyQuadKind::CheckboxBox,
                    });
                    if checked {
                        // Галочка: квады без вращения, аппроксимация двумя
                        // перпендикулярными тонкими полосками (v1)
                        let y0 = line_top + 2.0 * z + oy;
                        quads.push(BodyQuad {
                            rect: [2.2 * z, y0 + 6.0 * z, 3.4 * z, 1.6 * z],
                            kind: BodyQuadKind::CheckboxTick,
                        });
                        quads.push(BodyQuad {
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
            quads.extend(block_quads.into_iter().map(|(rect, kind)| BodyQuad {
                rect: [rect[0] + ox, rect[1] + oy, rect[2], rect[3]],
                kind,
            }));
            // Бар цитаты / фон фенса — на всю высоту блока
            if item.color == theme.quote {
                quads.push(BodyQuad {
                    rect: [0.0, oy, 3.0 * zoom_px, height_px],
                    kind: BodyQuadKind::QuoteBar,
                });
            }
            if item.mono {
                quads.push(BodyQuad {
                    rect: [0.0, oy, body_width * zoom_px, height_px],
                    kind: BodyQuadKind::CodeBg,
                });
            }
            // FR-017: подменённая what-if строка — акцентная подсветка
            // поверх фона формульной строки (видна при любом теле).
            if let Some(source_line) = item.source_line {
                if whatif_lines.contains(&source_line) {
                    quads.push(BodyQuad {
                        rect: [0.0, oy, body_width * zoom_px, height_px],
                        kind: BodyQuadKind::WhatIfBg,
                    });
                }
            }
            // FR-061 этап B: ширина зашейпленного текста — старт лидера
            // строки данных (D-5) и вход прохода A (left_max); до переноса
            // буфера в блок (move).
            let line_w = buffer
                .layout_runs()
                .map(|run| run.line_w)
                .fold(0.0f32, f32::max);
            blocks.push(BodyBlock {
                buffer,
                offset: [item.indent, cursor_y],
                width: block_width,
                height,
                line_w,
                color: item.color,
                source_line: item.source_line,
                header: item.header,
                preview: item.preview,
                expander: item.expander,
                spill: item.spill.clone(),
            });
        },
        |cursor_y, quads| {
            quads.push(BodyQuad {
                rect: [
                    0.0,
                    (cursor_y + 5.0) * zoom_px,
                    body_width * zoom_px,
                    2.0 * zoom_px,
                ],
                kind: BodyQuadKind::Rule,
            });
        },
    );
    BodyLayout { blocks, quads }
}

/// CR-012 (правка 2): общий ленивый FontSystem для измерения высоты тела —
/// те же 4 встроенных Noto-шрифта, что грузит `TextSystem::new`
/// (метрики измерения идентичны рендеру).
static MEASURE_FS: OnceLock<Mutex<FontSystem>> = OnceLock::new();

/// Достать общий FontSystem измерения. Отравленный мьютекс восстанавливаем
/// через `into_inner`: отравление возможно только при панике внутри
/// шейпинга, FontSystem после неё консистентен (layout-кэш пересчитывается
/// заново), поэтому измерение не падает, а продолжает работать.
/// FR-053 (U3): pub — владелец инстанса FontSystem для TextMeasurer
/// (PRD-0009 §14: раскладка пилотов шейпит теми же метриками, что рендер).
pub fn measure_font_system() -> MutexGuard<'static, FontSystem> {
    let mutex = MEASURE_FS.get_or_init(|| {
        let mut font_system = FontSystem::new();
        for data in FONT_DATA {
            font_system.db_mut().load_font_data((*data).to_vec());
        }
        Mutex::new(font_system)
    });
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// CR-012 (правка 2): точная высота тела заметки по реальному шейпингу —
/// тот же пайплайн, что и рендер ([`with_body_stack`], zoom=1.0; цвета темы
/// на метрики не влияют, берётся `ThemeColors::dark()`). Зеркало `shape_body`:
/// сумма высот и зазоров GFM-блоков, включая 12 px линий `---`. GPU не нужен.
/// `formula_lines` — те же индексы строк с результатом, что получает рендер
/// из `expr_line_results` (по ним `body_items` ставит mono-флаг).
/// FR-050 (этап D): проливания НЕ входят — наклонное начертание имеет те же
/// авансы/метрики (генерация шрифта), высота стека не меняется; рост высоты
/// под авто-строки делает сцена (CR-012-механизм, текст-префикс).
pub fn measure_body_height(
    text: &str,
    body_width: f32,
    formula_lines: &[usize],
    desc: &str,
) -> f32 {
    let mut guard = measure_font_system();
    with_body_stack(
        &mut guard,
        &ThemeColors::dark(),
        text,
        body_width.max(0.0),
        1.0,
        formula_lines,
        Vec::new(),
        &[],
        // D-14: высота стека от языка не зависит (блок-заголовок — одна
        // строка в любой локализации) — измерение фиксированным RU.
        canvas_core::Language::Ru,
        // D-8: зона описания — часть стека (I-2: measure = render).
        if desc.is_empty() { None } else { Some(desc) },
        // FR-061 хвосты: резерв считает РАЗВЁРНУТЫЙ блок и кламп описания
        // (дефолты) — узел обязан вмещать контент по умолчанию; свёрнутость
        // высоту не уменьшает (I-6, growth-only).
        true,
        false,
        // Измерению квады и буферы не нужны — нужна только высота стека.
        &mut Vec::new(),
        |_, _, _, _, _, _, _, _| {},
        |_, _| {},
    )
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
    /// FR-061 этап D (D-8): текст описания (пустой — зоны нет).
    desc: &'a str,
    /// FR-061 хвосты (D-7/D-8 runtime v1): биты состояния —
    /// bit0 = блок ведомости развёрнут, bit1 = описание раскрыто.
    /// Тоггл меняет стек (свёрнутость режет блоки, экспандер добавляет
    /// строку) — запись кэша обязана перешейпиться.
    mode: u8,
}

/// Запись кэша свежа, если зум, ширина, заголовок, тело и результаты
/// формул не изменились.
fn cache_fresh(entry: CacheKey, current: CacheKey) -> bool {
    (entry.zoom - current.zoom).abs() < 1e-3
        && (entry.width - current.width).abs() < 0.5
        && entry.title == current.title
        && entry.body == current.body
        && entry.results == current.results
        && entry.desc == current.desc
        && entry.mode == current.mode
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

/// FR-016 (CP5): бейдж узкого места ноды — текст метрик (`badge_text`:
/// «42%», «OVERLOAD 223% · W: 1.2 s») цветом серьёзности, world-якорь
/// ПРАВОГО края (выравнивание вправо-минус-ширина), рисуется НАД правым
/// верхним углом карточки в финальной группе (поверх карточек, вместе с
/// лейблами связей). Шейпинг покадровый — тексты короткие и меняются
/// только при пересчёте (паттерн live-line бейджей FR-013 правка 4).
#[derive(Debug, Clone)]
pub struct AnalysisBadge {
    pub text: String,
    /// World-координата правого края бейджа (якорь выравнивания).
    pub anchor: [f32; 2],
    /// Цвет текста — серьёзность по теме (`cards::severity_text`).
    pub color: Color,
}

/// FR-016: отступ правого края бейджа от правой границы карточки (world-px).
pub const ANALYSIS_BADGE_MARGIN_X: f32 = 6.0;
/// FR-016: зазор между НИЗОМ бейджа и верхом карточки (world-px).
pub const ANALYSIS_BADGE_GAP_Y: f32 = 2.0;

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
    /// Screen-space ПОЛОСЫ слоёв (FR-052, U2 PRD-0009): тексты каждой
    /// полосы готовятся в СВОЕЙ группе (`TextSystem::band_group`) —
    /// рендерер рисует полосы по очереди (квады полосы → тексты полосы),
    /// поэтому тексты нижней полосы не ложатся поверх квадов верхней.
    pub screen_bands: &'a [crate::renderer::ScreenBand<'a>],
    /// Z-план кадра (zorder.rs): текст-группы — тексты нод рисуются
    /// сегментами между карточками, чтобы текст фоновой ноды не ложился
    /// поверх карточек переднего плана. Финальная группа — лейблы связей,
    /// подписи меню и HUD; screen-тексты панелей — в группах полос
    /// (`TextSystem::band_group`, FR-052).
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
    /// FR-029 (визуализация проливания): параметры нод, запитанные
    /// value-рёбрами с `toParam`. Строка-присваивание пролитого параметра
    /// рендерится как подпись источника («param ← нода · выход»), бейдж
    /// строки — эффективным значением (пролитым), а не локальным литералом.
    pub param_spills: &'a std::collections::HashMap<String, Vec<crate::SpillView>>,
    /// FR-050 Р-4 (этап D): авто-строки приёмников — производные строки
    /// пересчёта потока (canvas-core `AutoRow`): рендерятся ПРЕФИКСОМ тела
    /// (зона «Переменные · входящие значения», наклонное начертание Р-2);
    /// hit-зоны Н9-2 собираются в цикле отрисовки. Пусто — рендер тела
    /// байт-в-байт прежний.
    pub auto_rows: &'a std::collections::HashMap<String, Vec<canvas_core::flow::AutoRow>>,
    /// FR-017 (CP6): what-if представления нод активного сценария —
    /// виртуальный исходник (подмены строк), подсветка подменённых строк,
    /// дельта-бейджи «было → стало (+Δ)». Пусто — режим выключен или подмен
    /// нет (рельеф базы не тронут, инвариант 2 FR-017).
    pub whatif_nodes: &'a std::collections::HashMap<String, crate::WhatIfNode>,
    /// FR-061 хвосты (D-7 runtime v1): id нод со СВЁРНУТЫМ блоком-ведомостью
    /// (дефолт — развёрнут; свёрнутость = явный клик по заголовку, состояние
    /// runtime, сброс при перезагрузке — Q4). Пусто — все блоки развёрнуты
    /// (прежний рельеф, I-1/T5 без правок).
    pub block_collapsed: &'a std::collections::HashSet<String>,
    /// FR-061 хвосты (D-8 runtime v1): id нод с РАСКРЫТЫМ описанием
    /// («⋯ целиком ▾» → полный текст; автосворачивание — клик вне/правка).
    pub desc_expanded: &'a std::collections::HashSet<String>,
    /// FR-016 (CP5): бейджи узких мест видимых нод — шейпятся покадрово,
    /// рисуются в финальной группе (поверх карточек, рядом с лейблами
    /// связей). Пустой список — оверлей выключен или рисков нет.
    pub analysis_badges: &'a [AnalysisBadge],
    /// FR-042/FR-044: тексты main stage (screen-space) — отдельная группа
    /// ПОСЛЕ модального прохода квадов stage (рендерер рисует её самой
    /// последней, поверх пилюль/карточек stage — см. `TextSystem::stage_group`).
    pub stage_texts: &'a [ScreenText<'a>],
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
    /// FR-061 этап B: табличные строки ноды (D-2) — ячейки
    /// значение/юнит/бейдж шейпятся вместе с кэшем, позиционируются
    /// по направляющим ([`CachedTitle::row_guides`]) покадрово.
    rows: Vec<CachedRow>,
    /// FR-061 этап B: колоночные направляющие таблицы ноды (проход A,
    /// row_grid::pass_a). None — строк данных нет (таблицы нет).
    row_guides: Option<canvas_ui::row_guides::RowGuides>,
    zoom_px: f32,
    width_px: f32,
    title_text: String,
    body_text: String,
    /// Сводка результатов (см. CacheKey.results) — ключ свежести.
    results_key: String,
    /// FR-061 этап D (D-8): текст описания ноды (ключ свежести D-8).
    desc_text: String,
    /// FR-061 хвосты (D-7/D-8): биты состояния тогглов (см. CacheKey.mode).
    mode: u8,
    /// Тик последнего использования — для вытеснения невидимых нод.
    last_used: u64,
}

/// FR-061 этап B: строка таблицы ноды в кэше (D-2/D-4): левой частью
/// строки остаётся блок тела (I-1 — Y-ряд не тронут), правые ячейки —
/// свои буферы, право-выровненные по направляющим.
struct CachedRow {
    /// Род строки — хром: зебра не заходит на заголовок блока (Total),
    /// лидер у заголовка не рисуется.
    kind: row_grid::RowKind,
    /// Индекс строки текста (Param/Calc); None — авто-строка префикса/заголовок.
    source_line: Option<usize>,
    /// Имя строки (параметр/путь авто-строки) — якоря параметров Н2
    /// (param_ports) сверяют со снапшотом шаблона по нему.
    name: String,
    /// Верх строки (world-px, block-local — как [`BodyBlock::offset`]).
    row_top: f32,
    /// Высота строки (world-px): BODY_LINE_HEIGHT у Param/Calc,
    /// AUTO_ROW_LINE_HEIGHT у авто-строк — вертикальное центрирование ячеек.
    row_line_h: f32,
    /// Конец левого текста (world-px от левого края тела) — старт лидера.
    left_end: f32,
    /// Зебра (D-5): строка в прогоне ≥ 4, чётная позиция внутри прогона.
    zebra: bool,
    value: Option<CachedCell>,
    unit: Option<CachedCell>,
    badge: Option<CachedCell>,
    /// Полный текст ошибки (тултип «!», механика FR-013 пр.4).
    error_message: Option<String>,
}

/// Зашейпленная ячейка строки: буфер + ширина (px буфера) + цвет.
struct CachedCell {
    buffer: Buffer,
    width_px: f32,
    color: Color,
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
    /// FR-061 этап B: измеритель ячеек таблицы (TextMeasurer F-6, кэш
    /// ширин) — проход A направляющих (row_grid::pass_a).
    measurer: canvas_ui::measure::TextMeasurer,
    /// Кэш лейблов связей по id связи (T8).
    label_cache: HashMap<String, CachedEdgeLabel>,
    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк
    /// (логические px) — пересобираются каждый кадр в prepare_titles;
    /// приложение вычитывает после рендера для тултипа.
    line_error_hits: Vec<LineErrorHit>,
    /// FR-050 Н9-2 (этап D): зоны наведения пролитых строк (параметр с
    /// toParam / авто-строка приёмника, логические px) — пересобираются
    /// каждый кадр; приложение вычитывает после рендера для тултипа
    /// источника («пролито: …»).
    spill_hits: Vec<SpillHit>,
    /// FR-061 хвосты (D-7/D-8 runtime v1): кликабельные зоны тела
    /// (заголовок блока-ведомости, экспандер описания; логические px) —
    /// пересобираются каждый кадр; приложение вычитывает после рендера
    /// для тогглов свёрнутости/раскрытости (решения владельца).
    body_hits: Vec<BodyHit>,
    /// Номер кадра для LRU-вытеснения кэша.
    tick: u64,
    /// Палитра темы: цвета заголовка/иконки/тела/лейбла связи.
    theme: ThemeColors,
    /// FR-061 этап D (D-14): язык таблицы тела (блок-заголовок Н-2).
    /// Глобальная настройка (не данные кадра) — устанавливается сеттером;
    /// свежесть кэша — через отпечаток языка в results_key.
    language: canvas_core::Language,
    /// FR-061 этап D (D-14/Q9): диагностика колоночных направляющих —
    /// рисуется только при включённом DebugOverlay (F9/?ui=debug).
    guides_visible: bool,
    /// FR-061 этап D (D-8): описания манифестов шаблонов (id → описание)
    /// — источник зоны описания шаблонных нод (Q3). Пустой — зоны нет.
    template_descs: HashMap<String, String>,
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
            measurer: canvas_ui::measure::TextMeasurer::new(),
            label_cache: HashMap::new(),
            line_error_hits: Vec::new(),
            spill_hits: Vec::new(),
            body_hits: Vec::new(),
            tick: 0,
            theme: ThemeColors::dark(),
            language: canvas_core::Language::Ru,
            guides_visible: false,
            template_descs: HashMap::new(),
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

    /// FR-061 этап D (D-14): язык таблицы тела (текст блока-заголовка Н-2).
    /// Кэш НЕ сбрасываем: отпечаток языка в results_key делает записи
    /// устаревшими точечно — перешейпятся только ноды с таблицей.
    pub fn set_table_language(&mut self, language: canvas_core::Language) {
        self.language = language;
    }

    /// FR-061 этап D (D-14/Q9): видимость диагностики колоночных направляющих
    /// (тумблер DebugOverlay F9/?ui=debug). Кэш не сбрасывается — квады
    /// диагностики добавляются поверх готового кадра без перешейпа.
    pub fn set_table_guides_visible(&mut self, visible: bool) {
        self.guides_visible = visible;
    }

    /// FR-061 этап D (D-8): описания манифестов шаблонов (id → описание) —
    /// источник зоны описания для шаблонных нод (Q3). Устанавливается
    /// приложением при построении реестра (снимок, не данные кадра).
    pub fn set_template_descs(&mut self, descs: std::collections::HashMap<String, String>) {
        // Кэш сбрасываем: описания могли измениться (импорт/обновление
        // шаблонов) — записи с зоной описания обязаны перешейпиться.
        self.cache.clear();
        self.template_descs = descs;
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

    /// FR-050 Н9-2 (этап D): зоны наведения пролитых строк кадра —
    /// тултип источника в оверлее следующего кадра.
    pub fn spill_hits(&self) -> &[SpillHit] {
        &self.spill_hits
    }

    /// FR-061 хвосты (D-7/D-8): кликабельные зоны тела ПОСЛЕДНЕГО кадра
    /// (заголовок блока-ведомости, экспандер описания; логические px).
    pub fn body_hits(&self) -> &[BodyHit] {
        &self.body_hits
    }

    /// FR-025: построчные точки выхода ноды из кэша раскладки: для каждой
    /// строки с бейджем результата — [`LinePort`] на правом краю ноды
    /// (вертикаль — [`result_row_y`] ряда бейджа — инвариант вертикали).
    /// FR-025 (правка 2, по проверке владельца): шаблонная нода — порты у
    /// каждой строки листа параметров ПЛЮС порт футера результата
    /// ([`result_footer_y`], `line = None` — узловое значение, формула
    /// шаблона FR-023; у построчных портов шаблона `is_final = false`).
    /// Нет кэша/результатов (нода вне экрана, виджет, проза) — портов нет,
    /// hit-test промахивается.
    pub fn line_ports(&self, index: usize, node: &Node) -> Vec<canvas_core::LinePort> {
        let right = node.x + node.width;
        let is_template = node.template().is_some();
        // FR-061 этап B: источники портов — строки таблицы с исходной
        // строкой (Param/Calc; авто-строки портов не дают, прежняя
        // семантика line_results сохранена — D-12, I-1).
        let source_rows: Vec<&CachedRow> = match self.cache.get(&index) {
            Some(entry) => entry
                .rows
                .iter()
                .filter(|row| row.source_line.is_some())
                .collect(),
            None => Vec::new(),
        };
        let total = source_rows.len();
        let mut ports: Vec<canvas_core::LinePort> = source_rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let source_line = row.source_line?;
                let block = entry_body_block(self.cache.get(&index)?, source_line)?;
                Some(canvas_core::LinePort {
                    line: Some(source_line),
                    point: [right, result_row_y(node, block.offset[1])],
                    // У шаблонной ноды «финальный» порт один — футер;
                    // строки листа параметров всегда промежуточные
                    is_final: i + 1 == total && !is_template,
                })
            })
            .collect();
        if is_template {
            ports.push(canvas_core::LinePort {
                line: None,
                point: [right, result_footer_y(node)],
                is_final: true,
            });
        }
        ports
    }

    /// FR-050 Н2 (этап C): входные якоря параметров шаблонной ноды из кэша
    /// раскладки: для каждой строки-присваивания, чьё имя входит в снапшот
    /// параметров шаблона (`TemplateRef::params` — канонический адрес
    /// `toParam`, тот же источник истины, что у валидации E-PORT-UNKNOWN),
    /// — [`ParamPort`] на ЛЕВОМ краю ноды (вертикаль — [`result_row_y`]
    /// ряда строки, зеркально построчным выходам FR-025; hit-тест — допуск
    /// CR-003). Текстовая нода якорей не имеет (toParam к ней не адресуется).
    /// Нет кэша/строк (нода вне экрана, виджет) — якорей нет.
    pub fn param_ports(&self, index: usize, node: &Node) -> Vec<canvas_core::ParamPort> {
        let Some(template) = node.template() else {
            return Vec::new();
        };
        let Some(entry) = self.cache.get(&index) else {
            return Vec::new();
        };
        entry
            .rows
            .iter()
            .filter_map(|row| {
                // FR-061 этап B: имя — из строки таблицы (тот же line_kind,
                // что и при сборке D-2 — дубля разбора нет).
                let source_line = row.source_line?;
                if !template.params.contains_key(&row.name) {
                    return None;
                }
                let block = entry_body_block(entry, source_line)?;
                Some(canvas_core::ParamPort {
                    param: row.name.clone(),
                    point: [node.x, result_row_y(node, block.offset[1])],
                })
            })
            .collect()
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
        // FR-050 Н9-2 (этап D): зоны пролитых строк — пересобираются каждый
        // кадр (позиции зависят от камеры/зума/раскладки тела)
        let mut spill_hits: Vec<SpillHit> = Vec::new();
        // FR-061 хвосты (D-7/D-8): кликабельные зоны тела (см. BodyHit).
        let mut body_hits: Vec<BodyHit> = Vec::new();
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
        // FR-017 (CP6): what-if представление — только при читаемом теле
        // (LOD SPEC §6.2: ниже порога превью тело и подсветка не рисуются).
        let empty_whatif: std::collections::HashMap<String, crate::WhatIfNode> =
            std::collections::HashMap::new();
        let whatif_nodes = if frame.camera.zoom() >= 0.6 {
            frame.whatif_nodes
        } else {
            &empty_whatif
        };
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
                // CR-010: резерв под иконку — и у файловой ноды (буква
                // расширения), и у шаблонной (квад-иконка справа): без
                // резерва длинное имя шаблона рисовалось под иконкой
                let has_icon = extension_letter(node).is_some() || node.template().is_some();
                let title_width = title_clip_width(node.width, has_icon);
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
                    let raw = node.text.clone().unwrap_or_default();
                    // FR-017: виртуальный исходник активного сценария
                    // (подменённые строки заменены) — рельеф базы в файле
                    // не тронут (инвариант 2).
                    let raw = whatif_nodes
                        .get(&node.id)
                        .map(|whatif| whatif.text.clone())
                        .unwrap_or(raw);
                    match frame.param_spills.get(&node.id) {
                        // FR-029: пролитый параметр показываем подписью
                        // источника — «rps ← Traffic Profile · peak_rps»
                        // вместо локального литерала (в файле он остаётся
                        // фолбэком на случай удаления ребра).
                        Some(spills) if !spills.is_empty() => {
                            canvas_core::flow::substitute_spilled_lines(
                                &raw,
                                spills.iter().map(crate::SpillView::as_triple),
                            )
                        }
                        _ => raw,
                    }
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
                // FR-017: дельта узлового итога активного сценария — полный
                // формат «было → стало (+Δ)» вместо голого значения.
                let result_text = whatif_nodes
                    .get(&node.id)
                    .and_then(|whatif| whatif.footer_delta.clone())
                    .unwrap_or(result_text);
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
                // FR-017: дельты/подмены активного сценария — в ключе свежести
                // (бейджи и подсветка зависят от сценария, не от базы).
                let whatif_key = whatif_nodes
                    .get(&node.id)
                    .map(|whatif| {
                        let mut parts: Vec<String> = whatif
                            .line_deltas
                            .iter()
                            .map(|(line, delta)| format!("{line}~{delta}"))
                            .collect();
                        parts.sort();
                        if let Some(footer) = &whatif.footer_delta {
                            parts.push(format!("F~{footer}"));
                        }
                        parts.join(";")
                    })
                    .unwrap_or_default();
                let results_key = if whatif_key.is_empty() {
                    results_key
                } else {
                    format!("{results_key}|W:{whatif_key}")
                };
                // FR-029: эффективные значения пролитых строк — в ключе
                // свежести (бейдж зависит от значения upstream).
                // FR-050 (этап D): в ключ входит НАБОР строк (наклонное
                // начертание Р-2 и данные тултипа Н9-2 зависят от него),
                // поэтому строки без значения тоже представлены («—»).
                let spill_views: &[crate::SpillView] = frame
                    .param_spills
                    .get(&node.id)
                    .map(|spills| spills.as_slice())
                    .unwrap_or(&[]);
                let spill_key = if spill_views.is_empty() {
                    String::new()
                } else {
                    spill_views
                        .iter()
                        .map(|spill| {
                            let line = spill.line.map(|l| l.to_string()).unwrap_or_default();
                            let value = spill.value.as_deref().unwrap_or("—");
                            format!("{line}~{value}")
                        })
                        .collect::<Vec<_>>()
                        .join(";")
                };
                let results_key = if results_key.is_empty() && spill_key.is_empty() {
                    results_key
                } else {
                    format!("{results_key}|S:{spill_key}")
                };
                // FR-050 Р-4 (этап D): авто-строки приёмника — тексты в ключе
                // свежести (значение upstream меняет строку → перешейп).
                let auto_rows: &[canvas_core::flow::AutoRow] = frame
                    .auto_rows
                    .get(&node.id)
                    .map(|rows| rows.as_slice())
                    .unwrap_or(&[]);
                let auto_key = if auto_rows.is_empty() {
                    String::new()
                } else {
                    auto_rows
                        .iter()
                        .map(|row| row.display_text())
                        .collect::<Vec<_>>()
                        .join(";")
                };
                let results_key = if auto_key.is_empty() {
                    results_key
                } else {
                    format!("{results_key}|R:{auto_key}")
                };
                // FR-061 этап D (D-14): язык таблицы — в ключе свежести
                // (текст блока-заголовка запечён в кэше; смена языка →
                // точечный перешейп нод с таблицей).
                let results_key = if results_key.is_empty() {
                    results_key
                } else {
                    format!("{results_key}|Lang:{:?}", self.language)
                };

                // FR-061 этап D (D-8): источник описания Q3 (решение владельца
                // 2026-09-23 — «desc→манифест→проза»): canvasdesk.desc →
                // описание манифеста шаблона (снимок id — template_descs) →
                // первый проза-абзац текста ноды (canvas-core, чистая функция —
                // та же в сцене для резерва высоты, I-2).
                let desc_text = node
                    .canvasdesk
                    .as_ref()
                    .and_then(|ext| ext.desc.clone())
                    .or_else(|| {
                        node.template()
                            .and_then(|t| self.template_descs.get(&t.id).cloned())
                    })
                    .or_else(|| {
                        node.text
                            .as_deref()
                            .and_then(canvas_core::expr::first_prose_paragraph)
                    })
                    .unwrap_or_default();
                let desc_ref = if desc_text.is_empty() {
                    None
                } else {
                    Some(desc_text.as_str())
                };
                // FR-061 хвосты (D-7/D-8 runtime v1): состояние тогглов ноды
                // (дефолты — развёрнут/кламп; в ключе свежести — mode).
                let block_expanded = !frame.block_collapsed.contains(&node.id);
                let desc_expanded = frame.desc_expanded.contains(&node.id);
                let mode = (u8::from(block_expanded)) | (u8::from(desc_expanded) << 1);

                let fresh = self.cache.get(&index).is_some_and(|e| {
                    cache_fresh(
                        CacheKey {
                            zoom: e.zoom_px,
                            width: e.width_px,
                            title: &e.title_text,
                            body: &e.body_text,
                            results: &e.results_key,
                            desc: &e.desc_text,
                            mode: e.mode,
                        },
                        CacheKey {
                            zoom: zoom_px,
                            width: width_px,
                            title: &title_text,
                            body: &body_text,
                            results: &results_key,
                            desc: &desc_text,
                            mode,
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
                    // FR-050 Р-4 (этап D): авто-строки приёмника — префикс
                    // тела (зона «Переменные · входящие значения», Р-2
                    // наклонное начертание); пустое тело при наличии
                    // авто-строк всё равно шейпится (стек из одного префикса).
                    // Редактируемая нода / LOD-скрытие тела — как у тела:
                    // тело рисует EditingSession, авто-строки вернутся после.
                    let body_hidden = frame.editing == Some(index) || !body_visible(node, zoom_px);
                    let spill_prefix = if body_hidden {
                        Vec::new()
                    } else {
                        spill_row_items(&self.theme, auto_rows, node.template().is_some())
                    };
                    let mut body =
                        if body_text.is_empty() && spill_prefix.is_empty() && desc_ref.is_none() {
                            None
                        } else {
                            let (_, body_width, _) = body_area(node);
                            // FR-017: подсветка подменённых строк активного сценария.
                            let whatif_lines: &[usize] = whatif_nodes
                                .get(&node.id)
                                .map(|whatif| whatif.overrides.as_slice())
                                .unwrap_or(&[]);
                            Some(shape_body(
                                &mut self.font_system,
                                &self.theme,
                                &body_text,
                                body_width,
                                zoom_px,
                                &formula_lines,
                                whatif_lines,
                                spill_prefix,
                                spill_views,
                                self.language,
                                desc_ref,
                                block_expanded,
                                desc_expanded,
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

                    // FR-061 этап B (D-2/D-4/D-5/D-6): табличные строки ноды —
                    // декларативная сборка ячеек (row_grid::build_rows),
                    // проход A направляющих с лестницей деградации бейджей
                    // (row_grid::pass_a), шейп ячеек значение/юнит/бейдж и
                    // квады хрома (лидер/зебра — D-5). Левая часть строки —
                    // существующий блок тела (I-1: Y-ряд не тронут).
                    let mut rows: Vec<CachedRow> = Vec::new();
                    let mut row_guides = None;
                    if let Some(layout) = body.as_mut() {
                        let (_, body_width, _) = body_area(node);
                        let whatif_deltas: &[(usize, String)] = whatif_nodes
                            .get(&node.id)
                            .map(|whatif| whatif.line_deltas.as_slice())
                            .unwrap_or(&[]);
                        let mut rows_data = row_grid::build_rows(
                            &body_text,
                            line_outcomes.map(|lines| lines.as_slice()),
                            whatif_deltas,
                            spill_views,
                            auto_rows,
                        );
                        // FR-061 этап C (D-7/D-9): заголовок блока-ведомости
                        // (Н-2) — Σ узлового итога на направляющей чисел;
                        // вставка перед первой расчётной строкой, зеркально
                        // заголовочному блоку в body_items (тот же план —
                        // block_header_plan в общем стеке, I-2).
                        // FR-061 хвосты (D-7 runtime v1): при свёрнутом блоке
                        // после Total вставляется превью-строка «Σ первый
                        // расчёт» (прототип .preview-row); расчётные строки
                        // блоков не имеют и выбрасываются циклом привязки.
                        if row_grid::block_mode(&rows_data, canvas_core::NODE_BODY_BLOCK_THRESHOLD)
                        {
                            let calc_count = row_grid::calc_row_count(&rows_data);
                            let sigma = if result_error || result_text.is_empty() {
                                String::new()
                            } else {
                                result_text.clone()
                            };
                            if let Some(idx) = rows_data
                                .iter()
                                .position(|row| row.kind == row_grid::RowKind::Calc)
                            {
                                rows_data.insert(
                                    idx,
                                    row_grid::RowCells {
                                        kind: row_grid::RowKind::Total,
                                        source_line: None,
                                        name: row_grid::block_header_text(calc_count),
                                        formula: String::new(),
                                        value: sigma,
                                        unit: String::new(),
                                        upstream: false,
                                        dim_value: false,
                                        badge: None,
                                        error_message: None,
                                    },
                                );
                                if !block_expanded {
                                    // Превью: Σ ПЕРВОГО расчёта (значение до
                                    // выбрасывания — Calc-строки ещё в списке).
                                    let first = rows_data
                                        .iter()
                                        .find(|row| row.kind == row_grid::RowKind::Calc);
                                    let (value, unit) = first
                                        .map(|row| (format!("Σ {}", row.value), row.unit.clone()))
                                        .unwrap_or(("Σ —".to_owned(), String::new()));
                                    let param_count = rows_data
                                        .iter()
                                        .filter(|row| row.kind == row_grid::RowKind::Param)
                                        .count();
                                    let after_total = idx + 1;
                                    rows_data.insert(
                                        after_total,
                                        row_grid::RowCells {
                                            kind: row_grid::RowKind::Preview,
                                            source_line: None,
                                            name: row_grid::block_preview_text_lang(
                                                param_count,
                                                calc_count,
                                                self.language,
                                            ),
                                            formula: String::new(),
                                            value,
                                            unit,
                                            upstream: false,
                                            dim_value: false,
                                            badge: None,
                                            error_message: None,
                                        },
                                    );
                                }
                            }
                        }
                        // Привязка строк к геометрии блоков: авто-строки —
                        // префиксные блоки (по порядку), Param/Calc — блок
                        // своей строки текста (I-1: те же Y, что у портов).
                        let auto_blocks: Vec<usize> = layout
                            .blocks
                            .iter()
                            .enumerate()
                            .filter_map(|(pos, block)| {
                                matches!(block.spill, Some(SpillHitKind::AutoRow { .. }))
                                    .then_some(pos)
                            })
                            .collect();
                        let mut auto_i = 0usize;
                        // Геометрия строк: (row_top, line_h, left_end, block_idx)
                        let mut geo: Vec<(f32, f32, f32, usize)> =
                            Vec::with_capacity(rows_data.len());
                        for row in &rows_data {
                            // FR-061 этап C: заголовок блока (Total) — блок с
                            // маркером header (вставка в body_items, I-2).
                            let found = match row.kind {
                                row_grid::RowKind::Total => {
                                    layout.blocks.iter().position(|block| block.header)
                                }
                                // FR-061 хвосты (D-7 runtime v1): превью
                                // свёрнутой ведомости — блок с маркером
                                // preview (вставка в body_items, I-2).
                                row_grid::RowKind::Preview => {
                                    layout.blocks.iter().position(|block| block.preview)
                                }
                                row_grid::RowKind::Auto => {
                                    let pos = auto_blocks.get(auto_i).copied();
                                    auto_i += 1;
                                    pos
                                }
                                _ => row.source_line.and_then(|line| {
                                    layout
                                        .blocks
                                        .iter()
                                        .position(|block| block.source_line == Some(line))
                                }),
                            };
                            match found {
                                Some(pos) => {
                                    let block = &layout.blocks[pos];
                                    let line_h = match row.kind {
                                        row_grid::RowKind::Auto => AUTO_ROW_LINE_HEIGHT,
                                        _ => BODY_LINE_HEIGHT,
                                    };
                                    let left_end =
                                        block.offset[0] + block.line_w / zoom_px.max(1e-6);
                                    geo.push((block.offset[1], line_h, left_end, pos));
                                }
                                // Строка без блока (рассинхрон текста/исходов)
                                // — не рисуется, портов не даёт.
                                None => geo.push((f32::NAN, 0.0, 0.0, usize::MAX)),
                            }
                        }
                        // Строки без блоков выбрасываются (геометрии нет).
                        let mut i = 0;
                        while i < rows_data.len() {
                            if geo[i].3 == usize::MAX {
                                rows_data.remove(i);
                                geo.remove(i);
                            } else {
                                i += 1;
                            }
                        }
                        // Проход A: направляющие + режим бейджей (детерминизм
                        // — входы уже в ключе кэша: текст/ширина/зум/исходы).
                        let left_max = geo.iter().map(|g| g.2).fold(0.0f32, f32::max);
                        let pass = row_grid::pass_a(
                            &mut self.measurer,
                            &mut self.font_system,
                            &rows_data,
                            left_max,
                            body_width,
                            MONO_FAMILY,
                            RESULT_FONT_SIZE,
                        );
                        row_guides = pass.guides;
                        // Зебра (D-5): прогоны ПОДРЯД идущих строк данных —
                        // соседство по индексам блоков (проза между строками
                        // рвёт прогон), чётные позиции внутри прогона ≥ 4.
                        let mut zebra: Vec<bool> = vec![false; rows_data.len()];
                        let mut j = 0;
                        while j < geo.len() {
                            let mut k = j + 1;
                            while k < geo.len()
                                && geo[k].3 == geo[k - 1].3 + 1
                                && !matches!(
                                    rows_data[k].kind,
                                    row_grid::RowKind::Total | row_grid::RowKind::Preview
                                )
                            {
                                k += 1;
                            }
                            if k - j >= ZEBRA_RUN_MIN {
                                for (pos, z) in zebra[j..k].iter_mut().enumerate() {
                                    // Заголовок блока зебры не получает (D-5:
                                    // зебра — фон строк данных; превью — тоже).
                                    *z = pos % 2 == 1
                                        && !matches!(
                                            rows_data[j + pos].kind,
                                            row_grid::RowKind::Total | row_grid::RowKind::Preview
                                        );
                                }
                            }
                            j = k;
                        }
                        // Шейп ячеек (D-4): значение/юнит — по частям D-1,
                        // бейдж — по режиму лестницы (D-6). Пролитые/авто —
                        // наклонное моно Р-2 (метрики те же — I-1).
                        let area_px = (body_width * zoom_px).max(1.0);
                        let amber = unmapped_color();
                        for ((row, g), z) in rows_data.iter().zip(&geo).zip(&zebra) {
                            let attrs = if row.upstream {
                                mono_oblique_attrs()
                            } else {
                                mono_attrs()
                            };
                            let value_color =
                                if row.kind == row_grid::RowKind::Auto && row.dim_value {
                                    amber
                                } else {
                                    self.theme.link
                                };
                            let badge = match (&row.badge, pass.badge_mode) {
                                (
                                    Some(badge),
                                    mode @ (row_grid::BadgeMode::Text | row_grid::BadgeMode::Icon),
                                ) => {
                                    let color = match badge {
                                        row_grid::RowBadge::Spill { .. } => self.theme.link,
                                        row_grid::RowBadge::Delta(_) => self.theme.whatif_badge,
                                        row_grid::RowBadge::Error => self.theme.error,
                                    };
                                    let text = match mode {
                                        row_grid::BadgeMode::Text => badge.text(),
                                        _ => badge.icon(),
                                    };
                                    shape_row_cell(
                                        &mut self.font_system,
                                        text,
                                        mono_attrs(),
                                        color,
                                        area_px,
                                        zoom_px,
                                    )
                                }
                                _ => None,
                            };
                            rows.push(CachedRow {
                                kind: row.kind,
                                source_line: row.source_line,
                                name: row.name.clone(),
                                row_top: g.0,
                                row_line_h: g.1,
                                left_end: g.2,
                                zebra: *z,
                                value: shape_row_cell(
                                    &mut self.font_system,
                                    &row.value,
                                    attrs,
                                    value_color,
                                    area_px,
                                    zoom_px,
                                ),
                                unit: shape_row_cell(
                                    &mut self.font_system,
                                    &row.unit,
                                    attrs,
                                    self.theme.quote,
                                    area_px,
                                    zoom_px,
                                ),
                                badge,
                                error_message: row.error_message.clone(),
                            });
                        }
                        // Квады хрома (D-5) — НИЖЕ всех существующих квадов
                        // (зебра/лидер под CodeBg/WhatIfBg и текстом).
                        let mut table_quads: Vec<BodyQuad> = Vec::new();
                        if let Some(g) = row_guides {
                            let z = zoom_px;
                            for row in &rows {
                                if row.zebra {
                                    table_quads.push(BodyQuad {
                                        rect: [
                                            0.0,
                                            row.row_top * z,
                                            body_width * z,
                                            row.row_line_h * z,
                                        ],
                                        kind: BodyQuadKind::RowBg,
                                    });
                                }
                                // Лидер: от конца левого текста до направляющей
                                // чисел, штрихи 2/3 px на базовой линии строки;
                                // у заголовка блока (Total) и превью лидера нет —
                                // Σ стоит на направляющей сама (анализ §3.1;
                                // превью свёрнутой ведомости — как Σ-строка).
                                if !matches!(
                                    row.kind,
                                    row_grid::RowKind::Total | row_grid::RowKind::Preview
                                ) {
                                    let x0 = (row.left_end + row_grid::LEADER_PAD) * z;
                                    let x1 = (g.value_right() - row_grid::LEADER_PAD) * z;
                                    let y = (row.row_top + row.row_line_h * LEADER_Y_FRAC) * z;
                                    // FR-061 этап E (D-15): штрихи — ЕДИНАЯ геометрия
                                    // кита (kit::leader_dash_rects); токены и арифметика
                                    // прежние (I-1, байт-паритет — тест kit.rs
                                    // leader_dashes_match_node_arithmetic). Минимум
                                    // дорожки — исторические 6 px тела ноды.
                                    for dash in canvas_ui::kit::leader_dash_rects(x0, x1, y, z, 6.0)
                                    {
                                        table_quads.push(BodyQuad {
                                            rect: dash,
                                            kind: BodyQuadKind::Leader,
                                        });
                                    }
                                }
                            }
                        }
                        if !table_quads.is_empty() {
                            let old = std::mem::take(&mut layout.quads);
                            layout.quads = table_quads;
                            layout.quads.extend(old);
                        }
                        // FR-061 этап D (D-14/Q9): диагностика направляющих —
                        // поверх хрома, ТОЛЬКО при включённом DebugOverlay
                        // (F9/?ui=debug); в проде невидима (решение Q9).
                        if self.guides_visible {
                            if let Some(g) = row_guides {
                                layout.quads.extend(guide_debug_quads(&g, &rows, zoom_px));
                            }
                        }
                    }

                    self.cache.insert(
                        index,
                        CachedTitle {
                            title,
                            icon,
                            body,
                            result,
                            result_error,
                            result_width_px,
                            rows,
                            row_guides,
                            zoom_px,
                            width_px,
                            title_text,
                            body_text,
                            results_key,
                            desc_text,
                            mode,
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
                Color::rgb(
                    canvas_core::tokens::HUD_SHADOW[0],
                    canvas_core::tokens::HUD_SHADOW[1],
                    canvas_core::tokens::HUD_SHADOW[2],
                ),
            ));
            hud_buffers.push((
                make_buffer(&mut self.font_system),
                [HUD_PADDING, HUD_PADDING],
                width,
                HUD_LINE_HEIGHT,
                self.theme.hud,
            ));
        }

        // Фаза 2: TextArea из кэша — по текст-группам z-плана (позиции
        // пересчитываются каждый кадр при пан, шейпинг — нет). Группа =
        // тексты одного сегмента кадра: рисуются после карточек сегмента
        // и до карточек, перекрывающих его ноды (z-порядок, zorder.rs).
        let group_count = frame.zplan.group_count();
        let final_group = frame.zplan.final_group();
        // Группы screen-space ПОЛОС (FR-052 U2): по одной группе на полосу,
        // индексы сразу за группами z-плана; stage-группа — после всех
        // полос. Раньше все screen-тексты шли одной группой после квадов
        // оверлея — тексты панели ложились поверх квадов модали, открытой
        // выше (полосное исполнение устраняет класс дефекта, §7.3).
        let band_count = frame.screen_bands.len();
        let stage_group = group_count + band_count;
        while self.renderers.len() <= stage_group {
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

        // Screen-space тексты ПОЛОС (FR-052 U2): константный физический
        // размер, позиции — логические px от угла окна, без камеры.
        // Буферы группируются по полосам — каждая полоса готовится в своей
        // текст-группе (см. хвост функции)
        let mut band_buffers: Vec<Vec<Buffer>> = Vec::with_capacity(frame.screen_bands.len());
        for band in frame.screen_bands {
            let mut buffers: Vec<Buffer> = Vec::with_capacity(band.texts.len());
            for st in band.texts {
                let font = st.font_size * scale_factor;
                let line_height = font * 1.3;
                let mut buffer =
                    Buffer::new(&mut self.font_system, Metrics::new(font, line_height));
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
                buffers.push(buffer);
            }
            band_buffers.push(buffers);
        }

        // FR-042/FR-044: тексты main stage — тот же screen-space конвейер,
        // что у панелей (константный физический размер); рисуются отдельной
        // группой после квадов stage (см. хвост функции)
        let mut stage_buffers: Vec<Buffer> = Vec::with_capacity(frame.stage_texts.len());
        for st in frame.stage_texts {
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
            stage_buffers.push(buffer);
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

        // FR-016 (CP5): бейджи узких мест — шейпинг покадрово (моно, как
        // строки результатов FR-013: метрика = код). Правое выравнивание по
        // якорю измеренной шириной (паттерн live-line буферов).
        let mut analysis_badge_buffers: Vec<(Buffer, [f32; 2], f32, Color)> =
            Vec::with_capacity(frame.analysis_badges.len());
        for badge in frame.analysis_badges {
            let font = RESULT_FONT_SIZE * zoom_px;
            let line_h = RESULT_LINE_HEIGHT * zoom_px;
            let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font, line_h));
            buffer.set_wrap(&mut self.font_system, Wrap::None);
            buffer.set_size(&mut self.font_system, Some(font * 24.0), Some(line_h));
            buffer.set_text(
                &mut self.font_system,
                badge.text.as_str(),
                mono_attrs(),
                Shaping::Advanced,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            let width_px = buffer
                .layout_runs()
                .next()
                .map(|run| run.line_w)
                .unwrap_or(0.0);
            analysis_badge_buffers.push((buffer, badge.anchor, width_px, badge.color));
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
                            // FR-050 Н9-2 (этап D): пролитая строка (авто-строка
                            // Р-4 / параметр с toParam) — зона наведения для
                            // тултипа источника («пролито: …»); логические px
                            // окна (физические / scale_factor, паттерн
                            // LineErrorHit). Только видимый ряд (не ниже клипа).
                            if let Some(kind) = &block.spill {
                                if bottom > top {
                                    spill_hits.push(SpillHit {
                                        rect: [
                                            left / scale_factor,
                                            top / scale_factor,
                                            (block.width * zoom_px) / scale_factor,
                                            (bottom - top) / scale_factor,
                                        ],
                                        kind: kind.clone(),
                                        // Н9-3: приёмник строки — для резолва
                                        // ребра контекст-меню параметра
                                        node: index,
                                    });
                                }
                            }
                            // FR-061 хвосты (D-7/D-8 runtime v1): кликабельные
                            // зоны тела — заголовок блока-ведомости (сворачи-
                            // вание/разворачивание) и экспандер описания
                            // («⋯ целиком ▾»/«▴ свернуть»); логические px окна
                            // (паттерн SpillHit). Только видимые (не ниже клипа).
                            let body_hit_kind = if block.header {
                                Some(BodyHitKind::BlockHeader)
                            } else if block.expander {
                                Some(BodyHitKind::DescExpander)
                            } else {
                                None
                            };
                            if let Some(kind) = body_hit_kind {
                                if bottom > top {
                                    body_hits.push(BodyHit {
                                        rect: [
                                            left / scale_factor,
                                            top / scale_factor,
                                            (block.width * zoom_px) / scale_factor,
                                            (bottom - top) / scale_factor,
                                        ],
                                        kind,
                                        node: index,
                                    });
                                }
                            }
                        }
                    }
                    // FR-013 (правка 2): результат каждой формульной строки —
                    // ПРАВЫЙ край ЕЁ строки (Numi-стиль). Привязка — блок тела
                    // с source_line этой строки (формульная строка — отдельный
                    // блок, см. body_items).
                    // FR-061 этап B (D-4/D-6): ячейки таблицы — значение на
                    // направляющей чисел, юнит на направляющей юнитов (текст
                    // прижат вправо: left = right − width), бейдж у правого
                    // края колонки. Вертикаль — единая формула рядов
                    // (Param/Calc — та же result_row_y, I-1/T5: порты и
                    // ячейки не разъезжаются; авто-строки — своя метрика).
                    if let (Some(guides), false) = (&entry.row_guides, entry.rows.is_empty()) {
                        let (origin, _, _) = body_area(node);
                        let body_left = node.x + BODY_PADDING;
                        let node_right = node.x + node.width - BODY_PADDING;
                        for row in &entry.rows {
                            let row_y = origin[1]
                                + row.row_top
                                + (row.row_line_h - RESULT_LINE_HEIGHT) / 2.0;
                            let top_phys = to_physical([origin[0], row_y])[1];
                            let bottom_phys = top_phys + RESULT_LINE_HEIGHT * zoom_px;
                            let bounds_left =
                                (to_physical([body_left, row_y])[0].floor() as i32) - 1;
                            // Значение: право на направляющую чисел
                            if let Some(cell) = &row.value {
                                let right_phys =
                                    to_physical([body_left + guides.value_right(), row_y])[0];
                                let left_phys = (right_phys - cell.width_px).round();
                                areas.push(TextArea {
                                    buffer: &cell.buffer,
                                    left: left_phys,
                                    top: top_phys,
                                    scale: 1.0,
                                    bounds: TextBounds {
                                        left: bounds_left,
                                        top: top_phys as i32,
                                        right: (right_phys.round() as i32) + 1,
                                        bottom: bottom_phys as i32,
                                    },
                                    default_color: dim_color(on_card(cell.color), text_factor),
                                    custom_glyphs: &[],
                                });
                            }
                            // Юнит: право на направляющую юнитов (O-4)
                            if let Some(cell) = &row.unit {
                                let right_phys =
                                    to_physical([body_left + guides.unit_right(), row_y])[0];
                                let left_phys = (right_phys - cell.width_px).round();
                                areas.push(TextArea {
                                    buffer: &cell.buffer,
                                    left: left_phys,
                                    top: top_phys,
                                    scale: 1.0,
                                    bounds: TextBounds {
                                        left: bounds_left,
                                        top: top_phys as i32,
                                        right: (right_phys.round() as i32) + 1,
                                        bottom: bottom_phys as i32,
                                    },
                                    default_color: dim_color(on_card(cell.color), text_factor),
                                    custom_glyphs: &[],
                                });
                            }
                            // Бейдж: правый край колонки (каскад Р-1)
                            if let Some(cell) = &row.badge {
                                let right_phys = to_physical([node_right, row_y])[0];
                                let left_phys = (right_phys - cell.width_px).round();
                                areas.push(TextArea {
                                    buffer: &cell.buffer,
                                    left: left_phys,
                                    top: top_phys,
                                    scale: 1.0,
                                    bounds: TextBounds {
                                        left: bounds_left,
                                        top: top_phys as i32,
                                        right: (right_phys.round() as i32) + 1,
                                        bottom: bottom_phys as i32,
                                    },
                                    default_color: dim_color(on_card(cell.color), text_factor),
                                    custom_glyphs: &[],
                                });
                                // FR-013 (правка 4): ошибка — расширенная зона
                                // наведения вокруг бейджа «!» для тултипа
                                if row.error_message.is_some() {
                                    error_hits.push(error_hit_rect(
                                        left_phys,
                                        top_phys,
                                        cell.width_px,
                                        zoom_px,
                                        scale_factor,
                                        row.error_message.clone().unwrap_or_default(),
                                    ));
                                }
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
                                            dim_color(on_card(self.theme.error), text_factor)
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
                                dim_color(on_card(self.theme.error), text_factor)
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
            // группах полос (band_group, FR-052; после квадов своей полосы)
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
                // FR-016 (CP5): бейджи узких мест — над правым верхним углом
                // карточки, правое выравнивание по якорю минус измеренная
                // ширина (зеркало строки результата FR-013, но над карточкой).
                for (buffer, anchor, width_px, color) in &analysis_badge_buffers {
                    let anchor_phys = to_physical(*anchor);
                    let line_h = RESULT_LINE_HEIGHT * zoom_px;
                    let left = anchor_phys[0] - width_px;
                    let top = anchor_phys[1] - line_h;
                    areas.push(TextArea {
                        buffer,
                        left,
                        top,
                        scale: 1.0,
                        bounds: TextBounds {
                            left: (left.floor() as i32) - 1,
                            top: top as i32,
                            right: (anchor_phys[0].round() as i32) + 1,
                            bottom: (top + line_h) as i32,
                        },
                        default_color: *color,
                        custom_glyphs: &[],
                    });
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
        // FR-050 Н9-2 (этап D): зоны пролитых строк кадра собраны — тултип
        // источника в оверлее следующего кадра
        self.spill_hits = spill_hits;
        self.body_hits = body_hits;
        // Screen-тексты ПОЛОС (FR-052 U2): отдельная группа на полосу —
        // рендерер рисует полосы по очереди (квады полосы → тексты полосы),
        // поэтому фон следующей полосы не закрывает строки предыдущей,
        // а тексты нижней полосы не ложатся поверх квадов верхней
        // FR-056 (F-5): scissor-бакеты полос заранее (та же конверсия
        // клипа, что у квадов) — TextBounds каждого текста пересекается с
        // бакетом своей полосы; тексты за клипом не готовятся.
        let band_scissors: Vec<Option<(i32, i32, i32, i32)>> = frame
            .screen_bands
            .iter()
            .map(|band| {
                crate::renderer::band_scissor_rect(
                    &band.clip,
                    scale_factor,
                    viewport_physical[0],
                    viewport_physical[1],
                )
                .map(|[x, y, w, h]| (x as i32, y as i32, (x + w) as i32, (y + h) as i32))
            })
            .collect();
        for (band_index, buffers) in band_buffers.iter().enumerate() {
            let mut band_areas: Vec<TextArea> = Vec::with_capacity(buffers.len());
            for (buffer, st) in buffers.iter().zip(frame.screen_bands[band_index].texts) {
                let mut area = screen_text_area(buffer, st, scale_factor);
                if let Some(clip) = band_scissors[band_index] {
                    if let Some(bounds) = clip_text_bounds(area.bounds, clip) {
                        area.bounds = bounds;
                        band_areas.push(area);
                    }
                }
            }
            if let Some(renderer) = self.renderers.get_mut(group_count + band_index) {
                prepare_group(
                    renderer,
                    device,
                    queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    &band_areas,
                    &mut self.swash_cache,
                )?;
            }
        }
        // FR-042/FR-044: тексты main stage — последняя группа кадра (после
        // модального прохода квадов stage в рендерере)
        let mut stage_areas: Vec<TextArea> = Vec::with_capacity(stage_buffers.len());
        for (buffer, st) in stage_buffers.iter().zip(frame.stage_texts) {
            stage_areas.push(screen_text_area(buffer, st, scale_factor));
        }
        if let Some(renderer) = self.renderers.get_mut(stage_group) {
            prepare_group(
                renderer,
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                &self.viewport,
                &stage_areas,
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

    /// Индекс текст-группы полосы `band_index` (FR-052 U2): группы полос
    /// идут сразу за группами z-плана, по одной на полосу; рендерер
    /// рисует полосы по очереди — квад-диапазон полосы, затем её группа.
    pub fn band_group(zplan: &crate::zorder::ZPlan, band_index: usize) -> usize {
        zplan.group_count() + band_index
    }

    /// Индекс группы текстов main stage (FR-042/FR-044): после ВСЕХ групп
    /// полос (`band_count` — число screen-полос кадра) — САМАЯ последняя
    /// группа кадра. Рендерер рисует её после модального прохода квадов
    /// stage (который идёт после всех полос и миникарты), поэтому тексты
    /// stage лежат поверх своих пилюль и карточек, но ничего живого
    /// канваса поверх stage нет.
    pub fn stage_group(zplan: &crate::zorder::ZPlan, band_count: usize) -> usize {
        zplan.group_count() + band_count
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

    // FR-056 (F-5): клип текстов полосы — TextBounds = пересечение
    // собственных границ текста со scissor-бакетом полосы.
    #[test]
    fn clip_text_bounds_identity_inside_band_clip() {
        // текст целиком внутри клипа — границы не меняются (инвариант
        // «0 визуальных изменений» при клипе-вьюпорте)
        let own = TextBounds {
            left: 100,
            top: 50,
            right: 220,
            bottom: 70,
        };
        assert_eq!(
            clip_text_bounds(own, (0, 0, 1280, 800)),
            Some(TextBounds {
                left: 100,
                top: 50,
                right: 220,
                bottom: 70
            })
        );
    }

    #[test]
    fn clip_text_bounds_shrinks_to_band_clip() {
        // текст высунулся за правый/нижний край полосы — обрезается
        let own = TextBounds {
            left: 1200,
            top: 760,
            right: 1400,
            bottom: 850,
        };
        assert_eq!(
            clip_text_bounds(own, (0, 0, 1280, 800)),
            Some(TextBounds {
                left: 1200,
                top: 760,
                right: 1280,
                bottom: 800
            })
        );
    }

    #[test]
    fn clip_text_bounds_disjoint_is_none() {
        // текст полностью за клипом полосы — не готовится вовсе
        let own = TextBounds {
            left: 1300,
            top: 50,
            right: 1400,
            bottom: 70,
        };
        assert_eq!(clip_text_bounds(own, (0, 0, 1280, 800)), None);
    }

    /// FR-023 (вилка владельца): кегль заголовка на 10–20 % больше кегля
    /// тела. Инвариант не даёт константам разъехаться при правках.
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn title_font_size_within_owner_range_over_body() {
        let ratio = TITLE_FONT_SIZE / BODY_FONT_SIZE;
        assert!(
            (1.10..=1.20).contains(&ratio),
            "заголовок {TITLE_FONT_SIZE} к телу {BODY_FONT_SIZE}: ratio={ratio}, вилка 1.10..1.20"
        );
        // Шапка вмещает строку заголовка с вертикальными полями
        assert!(HEADER_HEIGHT >= TITLE_LINE_HEIGHT + 8.0);
        assert!(TITLE_PADDING >= 10.0, "адекватный отступ заголовка");
    }

    /// CR-010: клип заголовка шаблонной ноды заканчивается ДО квад-иконки
    /// (длинное имя не рисуется под иконкой), а горизонтальные поля шапки
    /// симметричны: TITLE_PADDING слева = TEMPLATE_ICON_MARGIN_H справа.
    #[test]
    fn template_title_clip_clears_quad_icon() {
        use crate::cards::{template_icon_rect, TEMPLATE_ICON_MARGIN_H};
        let mut node = Node::text("tpl", "rps = 1000 rps", 100.0, 50.0);
        node.width = 260.0;
        node.set_template(Some(canvas_core::templates::TemplateRef {
            id: "mock.lb".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "mm1($rps, $service_rate)".to_owned(),
            params: std::collections::BTreeMap::new(),
            icon: "lb".to_owned(),
            color: "#4A90E2".to_owned(),
            name: Some("Балансировщик нагрузки".to_owned()),
            outputs: Vec::new(),
        }));
        // Ширина клипа — с резервом под иконку (как в фазе шейпинга)
        let reserves_icon = node.template().is_some();
        let title_width = title_clip_width(node.width, reserves_icon);
        let title_right = node.x + TITLE_PADDING + title_width;
        let icon = template_icon_rect(&node);
        assert!(
            title_right <= icon[0] + 0.01,
            "клип заголовка ({title_right}) залезает под иконку (левый край {})",
            icon[0]
        );
        assert!(
            (TEMPLATE_ICON_MARGIN_H - TITLE_PADDING).abs() < 0.01,
            "горизонтальные поля шапки асимметричны"
        );
        // У ноды без иконки резерва нет — клип по полям с двух сторон
        let plain = Node::text("n", "text", 0.0, 0.0);
        assert!(
            (title_clip_width(plain.width, false) - (plain.width - TITLE_PADDING * 2.0)).abs()
                < 0.01
        );
    }

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
            desc: "",
            mode: 0,
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
        // FR-061 D (D-8): изменение описания инвалидирует кэш
        assert!(
            !cache_fresh(
                entry,
                CacheKey {
                    desc: "описание",
                    ..same
                }
            ),
            "описание изменилось"
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
        shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            300.0,
            1.0,
            &[],
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        )
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
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
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

    /// FR-017 (CP6): whatif_lines — подменённые строки получают квад WhatIfBg
    /// на всю ширину и высоту блока формульной строки; без whatif_lines —
    /// квада нет.
    #[test]
    fn shape_body_whatif_line_quad() {
        let mut fs = FontSystem::new();
        let text = "Gateway\nrps = 1000";
        // Без подмен: только фон CodeBg формульной строки.
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            300.0,
            1.0,
            &[1],
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        );
        assert!(
            !layout
                .quads
                .iter()
                .any(|quad| quad.kind == BodyQuadKind::WhatIfBg),
            "без whatif_lines квада подмены нет: {:?}",
            layout.quads
        );
        // Подмена строки 1 → квад WhatIfBg поверх CodeBg.
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            300.0,
            1.0,
            &[1],
            &[1],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        );
        let whatif = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::WhatIfBg)
            .expect("квад what-if подмены есть");
        assert_eq!(whatif.rect[0], 0.0);
        assert_eq!(whatif.rect[2], 300.0, "квад на всю ширину тела");
        let code_bg = layout
            .quads
            .iter()
            .find(|quad| quad.kind == BodyQuadKind::CodeBg)
            .expect("фон формульной строки рядом");
        assert_eq!(
            whatif.rect[3], code_bg.rect[3],
            "квад подмены — на высоту блока формульной строки"
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

    /// FR-061 этап D (D-14/Q9): квады диагностики направляющих — вертикали
    /// на right-краях value/unit через зону строк данных; пустой список
    /// строк — квадов нет.
    #[test]
    fn guide_debug_quads_span_rows_at_right_edges() {
        let mut fs = FontSystem::new();
        let cell = |fs: &mut FontSystem| CachedCell {
            buffer: Buffer::new(fs, Metrics::new(12.0, 16.0)),
            width_px: 10.0,
            color: Color::rgb(0, 0, 0),
        };
        let row = |fs: &mut FontSystem, top: f32| CachedRow {
            kind: row_grid::RowKind::Calc,
            source_line: Some(0),
            name: String::new(),
            row_top: top,
            row_line_h: BODY_LINE_HEIGHT,
            left_end: 0.0,
            zebra: false,
            value: Some(cell(fs)),
            unit: None,
            badge: None,
            error_message: None,
        };
        let rows = vec![row(&mut fs, 0.0), row(&mut fs, 20.0)];
        let guides = canvas_ui::row_guides::RowGuides {
            value_w: 30.0,
            unit_w: 20.0,
            badge_w: 0.0,
            value_x: 200.0,
            unit_x: 240.0,
        };
        let quads = guide_debug_quads(&guides, &rows, 2.0);
        assert_eq!(quads.len(), 2, "линии value и unit");
        assert!(quads.iter().all(|q| q.kind == BodyQuadKind::GuideDebug));
        // Вертикаль через обе строки: y0 = 0, высота = (20+20)*зум
        assert_eq!(quads[0].rect[1], 0.0);
        assert_eq!(quads[0].rect[3], 80.0);
        // Линии на right-краях ячеек: (value_x + value_w) * зум
        assert_eq!(quads[0].rect[0], 460.0);
        assert_eq!(quads[1].rect[0], 520.0);
        // Пустой список строк — квадов нет
        assert!(guide_debug_quads(&guides, &[], 1.0).is_empty());
    }

    /// FR-061 этап D (O-5): раскраска лексем формулы — функция (ident + «(»)
    /// курсивом и formula_fn, операторы formula_op, переменные/числа — база;
    /// GFM-маркеры (`*` умножения) — литералы, italic-спанов нет.
    #[test]
    fn formula_rich_runs_tints_functions_and_operators() {
        let base = mono_attrs();
        let fn_c = Color::rgb(0xc7, 0x92, 0xea);
        let op_c = Color::rgb(0x66, 0x6a, 0x7c);
        let runs = formula_rich_runs("rps = max(a, 2) * 3", base, fn_c, op_c);
        let find = |needle: &str| {
            runs.iter()
                .find(|(s, _)| *s == needle)
                .copied()
                .unwrap_or_else(|| panic!("ран «{needle}» не найден: {runs:?}"))
        };
        let (rps, rps_attrs) = find("rps");
        assert_eq!(rps, "rps");
        assert_eq!(rps_attrs.color_opt, None, "переменная — без подкраски");
        assert_eq!(rps_attrs.style, Style::Normal);
        let (eq, eq_attrs) = find("=");
        assert_eq!(eq, "=");
        assert_eq!(eq_attrs.color_opt, Some(op_c));
        let (mx, mx_attrs) = find("max");
        assert_eq!(mx, "max");
        assert_eq!(mx_attrs.color_opt, Some(fn_c), "функция — formula_fn");
        assert_eq!(mx_attrs.style, Style::Italic, "функция — курсив (прототип)");
        let (star, star_attrs) = find("*");
        assert_eq!(star, "*");
        assert_eq!(star_attrs.color_opt, Some(op_c));
        // Умножение «a * 2 * 3» — оба «*» литеральные операторы, без italic
        let runs = formula_rich_runs("a * 2 * 3", base, fn_c, op_c);
        assert!(runs.iter().all(|(_, a)| a.style == Style::Normal));
    }

    /// FR-061 этап D (D-8): кламп описания — короткий текст целиком, длинный —
    /// обрезается с «…»; зона описания добавляет высоту стека (I-2: мера и
    /// рендер — один стек).
    #[test]
    fn clamp_desc_text_and_measure_parity() {
        let mut fs = FontSystem::new();
        let short = "Короткое описание.";
        assert_eq!(
            clamp_desc_text(
                &mut fs,
                short,
                280.0,
                canvas_core::tokens::TABLE_DESC_CLAMP_LINES
            )
            .0,
            short,
            "короткий текст не клампится"
        );
        let long = "Длинное описание расчётной модели веб-сервиса, которое заведомо не помещается в две строки узкого тела ноды и потому обязано обрезаться многоточием по словам.";
        let (clamped, truncated) = clamp_desc_text(
            &mut fs,
            long,
            280.0,
            canvas_core::tokens::TABLE_DESC_CLAMP_LINES,
        );
        assert!(clamped.ends_with("…"), "кламп завершается «…»: {clamped}");
        assert!(clamped.chars().count() < long.chars().count());
        // FR-061 хвосты: флаг усечения — true для длинного, false для короткого
        assert!(truncated, "длинный текст помечен усечённым");
        let (short_text, short_truncated) = clamp_desc_text(
            &mut fs,
            short,
            280.0,
            canvas_core::tokens::TABLE_DESC_CLAMP_LINES,
        );
        assert!(!short_truncated, "короткий текст не усечён");
        assert_eq!(short_text, short);
        // Пустой desc — пустая строка (зона не строится)
        assert_eq!(
            clamp_desc_text(&mut fs, "   ", 280.0, 2),
            (String::new(), false)
        );
        // Мера стека: desc-зона добавляет высоту
        let plain = measure_body_height("deploy = 40 $", 300.0, &[0], "");
        let with_desc = measure_body_height("deploy = 40 $", 300.0, &[0], "Описание схемы.");
        assert!(
            with_desc > plain,
            "desc-зона добавляет высоту: {plain} → {with_desc}"
        );
    }

    /// CR-009: формульная строка (source_line) — Numi-расчёт → mono; проза — sans.
    #[test]
    fn body_items_formula_line_is_mono() {
        let theme = ThemeColors::dark();
        let items = body_items(
            &theme,
            "Gateway\ndeploy = 40 $",
            &[1],
            &[],
            canvas_core::Language::Ru,
            true,
            None,
        );
        assert_eq!(items.len(), 2, "проза + формульная строка");
        assert!(!items[0].mono, "проза — sans");
        assert!(items[1].mono, "Numi-строка — моно");
        assert_eq!(items[1].source_line, Some(1));
    }

    /// FR-050 Р-2 (этап D): наклонное семейство CanvasDesk Mono Oblique
    /// зарегистрировано во встроенных шрифтах (oblique-производная
    /// Noto Sans Mono; генерация — scripts/gen_oblique_font.py).
    #[test]
    fn font_data_registers_mono_oblique_family() {
        let mut fs = FontSystem::new();
        for data in FONT_DATA {
            fs.db_mut().load_font_data((*data).to_vec());
        }
        assert!(
            fs.db().faces().any(|face| face
                .families
                .iter()
                .any(|(name, _)| name == MONO_OBLIQUE_FAMILY)),
            "нет вшитого лица {MONO_OBLIQUE_FAMILY}"
        );
        let oblique = mono_oblique_attrs();
        assert_eq!(oblique.family, Family::Name(MONO_OBLIQUE_FAMILY));
    }

    /// FR-050 Р-2/Н9-2 (этап D): строка параметра, запитанная toParam-ребром
    /// (подпись «param ← Источник · выход»), — моно + НАКЛОННОЕ начертание
    /// + данные тултипа источника; прочие строки — прямые, без payload.
    #[test]
    fn body_items_marks_spilled_param_oblique() {
        let theme = ThemeColors::dark();
        let spills = vec![crate::SpillView {
            param: "rps".to_owned(),
            line: Some(1),
            from_label: "Трафик".to_owned(),
            from_output: Some("peak_rps".to_owned()),
            value: Some("1389 rps".to_owned()),
            path: "Трафик.peak_rps".to_owned(),
            local: Some("500 rps".to_owned()),
        }];
        let items = body_items(
            &theme,
            "Gateway\nrps = 500 rps",
            &[1],
            &spills,
            canvas_core::Language::Ru,
            true,
            None,
        );
        assert_eq!(items.len(), 2);
        assert!(!items[0].oblique, "проза — прямое начертание");
        assert!(items[1].oblique, "пролитая строка — наклонное (Р-2)");
        assert!(items[1].mono);
        match &items[1].spill {
            Some(SpillHitKind::Param {
                param,
                path,
                value,
                local,
            }) => {
                assert_eq!(param, "rps");
                assert_eq!(path, "Трафик.peak_rps");
                assert_eq!(value.as_deref(), Some("1389 rps"));
                assert_eq!(local.as_deref(), Some("500 rps"));
            }
            other => panic!("нет данных тултипа Н9-2: {other:?}"),
        }
        assert!(items[0].spill.is_none(), "у прозы payload нет");
    }

    /// FR-067 (этап F): супрессия абзаца описания — строки диапазона не
    /// рендерятся, формульная строка жива с прежним source_line; без
    /// супрессии абзац в теле (регресс двойного показа).
    #[test]
    fn body_items_suppresses_desc_paragraph() {
        let theme = ThemeColors::dark();
        let text = "шлюз обрабатывает поток\n\nrps = 800 rps\n800 rps / 12 ms";
        let formula_lines = [2, 3];
        // Границы абзаца по общим функциям ядра (как в with_body_stack)
        assert_eq!(canvas_core::expr::first_prose_paragraph_span(text), Some((0, 1)));
        let suppress = Some((0, 1));
        let items = body_items(
            &theme,
            text,
            &formula_lines,
            &[],
            canvas_core::Language::Ru,
            true,
            suppress,
        );
        assert!(
            items.iter().all(|item| !item.text.contains("шлюз обрабатывает")),
            "абзац описания из тела убран"
        );
        // Формульные строки на месте, привязка к исходным строкам не сдвинулась
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].source_line, Some(2));
        assert_eq!(items[1].source_line, Some(3));
        // Без супрессии — абзац в теле (прежнее поведение с дубликатом)
        let items = body_items(
            &theme,
            text,
            &formula_lines,
            &[],
            canvas_core::Language::Ru,
            true,
            None,
        );
        assert!(items[0].text.contains("шлюз обрабатывает"));
    }

    /// FR-067: супрессия внутри смешанного сегмента — соседние строки
    /// сегмента (до/после абзаца) остаются, строки абзаца уходят.
    #[test]
    fn body_items_suppress_splits_mixed_segment() {
        let theme = ThemeColors::dark();
        // Один None-сегмент: проза-вступление, пустая строка, абзац, пустая, хвост
        let text = "вступление\n\nэто описание ноды\n\nхвост";
        let suppress = Some((2, 3));
        let items = body_items(
            &theme,
            text,
            &[],
            &[],
            canvas_core::Language::Ru,
            true,
            suppress,
        );
        assert!(
            items.iter().all(|item| !item.text.contains("это описание")),
            "строка абзаца не рендерится"
        );
        // Сегмент дробится: вступление и хвост живут отдельными блоками
        assert!(items.iter().any(|item| item.text == "вступление"));
        assert!(items.iter().any(|item| item.text == "хвост"));
    }

    /// FR-050 Р-4 (этап D): элементы авто-строк приёмника — префикс тела:
    /// наклонное моно, текст «Путь = значение», зазор первого 0 / прочих 2,
    /// payload AutoRow; unmapped — «Путь = —» и янтарный цвет (Р-3).
    #[test]
    fn spill_row_items_build_oblique_prefix() {
        let theme = ThemeColors::dark();
        let rows = vec![
            canvas_core::flow::AutoRow {
                node_id: "gateway".to_owned(),
                edge_id: "e1".to_owned(),
                slot: 0,
                path: "Трафик.peak_rps".to_owned(),
                field: "peak_rps".to_owned(),
                value: Some(canvas_core::Value::scalar(1389.0)),
            },
            canvas_core::flow::AutoRow {
                node_id: "gateway".to_owned(),
                edge_id: "e2".to_owned(),
                slot: 1,
                path: "Курсы.usd".to_owned(),
                field: "usd".to_owned(),
                value: None,
            },
        ];
        let items = spill_row_items(&theme, &rows, false);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].gap, 0.0, "первая авто-строка — без зазора");
        assert_eq!(items[1].gap, 2.0, "плотный список переменных");
        assert!(items.iter().all(|item| item.oblique && item.mono));
        // FR-061 этап B: левая часть авто-строки — ТОЛЬКО имя (путь);
        // значение/юнит — ячейки таблицы на направляющих (D-2/D-4).
        assert_eq!(items[0].text, "Трафик.peak_rps");
        assert_eq!(items[1].text, "Курсы.usd");
        assert_eq!(items[0].color, theme.code_text, "пролитое — цвет кода");
        assert_ne!(items[1].color, theme.code_text, "unmapped — янтарь Р-3");
        match &items[0].spill {
            Some(SpillHitKind::AutoRow {
                path,
                slot,
                value,
                template,
                edge_id: _,
            }) => {
                assert_eq!(path, "Трафик.peak_rps");
                assert_eq!(*slot, 0);
                assert_eq!(value.as_deref(), Some("1389"));
                assert!(!template, "приёмник — текстовая нода");
            }
            other => panic!("нет данных тултипа Н9-2: {other:?}"),
        }
    }

    /// FR-050 Р-4 (этап D): префикс авто-строк растит высоту стека тела
    /// (карточка обязана вместить строку-проекцию — рост в сцене, метрики
    /// здесь); пустой префикс — высота прежняя (байт-в-байт инвариант).
    #[test]
    fn with_body_stack_prefix_grows_height() {
        let mut fs = FontSystem::new();
        let theme = ThemeColors::dark();
        let text = "Gateway\ncache_hit = 0.6";
        let plain = with_body_stack(
            &mut fs,
            &theme,
            text,
            300.0,
            1.0,
            &[1],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            // хвосты FR-061: развёрнутый блок, кламп описания (дефолты)
            true,
            false,
            &mut Vec::new(),
            |_, _, _, _, _, _, _, _| {},
            |_, _| {},
        );
        let rows = vec![canvas_core::flow::AutoRow {
            node_id: "n".to_owned(),
            edge_id: "e".to_owned(),
            slot: 0,
            path: "Трафик.peak_rps".to_owned(),
            field: "peak_rps".to_owned(),
            value: Some(canvas_core::Value::scalar(1389.0)),
        }];
        let prefix = spill_row_items(&theme, &rows, false);
        let with_rows = with_body_stack(
            &mut fs,
            &theme,
            text,
            300.0,
            1.0,
            &[1],
            prefix,
            &[],
            canvas_core::Language::Ru,
            None,
            // хвосты FR-061: развёрнутый блок, кламп описания (дефолты)
            true,
            false,
            &mut Vec::new(),
            |_, _, _, _, _, _, _, _| {},
            |_, _| {},
        );
        assert!(
            with_rows > plain + 17.0,
            "авто-строка добавляет ряд (~18px): {plain} → {with_rows}"
        );
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
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
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

    // --- CR-012 (правка 2): measure_body_height — точное измерение тела ---

    /// Скриншотный Numi-лист из CR-012: три формульные строки при ширине
    /// тела 280 — каждая строка свой mono-блок (BODY_LINE_HEIGHT), между
    /// абзацами зазор 6 (body_gap). Измерение — зеркало shape_body при
    /// zoom=1.0: ровно 3·BODY_LINE_HEIGHT + 2·6.
    #[test]
    fn measure_body_height_numi_list_exact() {
        let text = "rps = 200 rps\ntoken_verify = 2 ms\ncache_ttl = 5 min";
        let height = measure_body_height(text, 280.0, &[0, 1, 2], "");
        assert_eq!(
            height,
            3.0 * BODY_LINE_HEIGHT + 2.0 * 6.0,
            "три mono-блока по {BODY_LINE_HEIGHT} с зазорами 6 между абзацами"
        );
    }

    /// Длинное mono-значение (~32 символа при ширине тела 240) переносится
    /// на 2 визуальных ряда — измеренная высота ровно 2·BODY_LINE_HEIGHT
    /// (один блок, внутренних зазоров нет). Оценка той же строки
    /// (`wrapped_body_rows`, canvas-app) согласована: тоже 2 ряда.
    #[test]
    fn measure_body_height_mono_wraps_to_two_rows() {
        let line = format!("{} = 5", "a".repeat(28)); // 32 символа
        let height = measure_body_height(&line, 240.0, &[0], "");
        assert_eq!(
            height,
            2.0 * BODY_LINE_HEIGHT,
            "mono-строка из 32 символов при ширине 240 — ровно 2 ряда"
        );
    }

    /// Проза без formula_lines шейпится sans-метрикой (Noto Sans Display):
    /// строка из 30 «a» при ширине 240 — один ряд (≈7 px/символ), а с
    /// mono-флагом (formula_lines=[0]) — два (≈8.6 px/символ). Регресс
    /// CR-012: рендер ставит mono-флаг по formula_lines.
    #[test]
    fn measure_body_height_prose_uses_sans_metrics() {
        let line = "a".repeat(30); // 30 символов — между двумя метриками
        let sans_height = measure_body_height(&line, 240.0, &[], "");
        let mono_height = measure_body_height(&line, 240.0, &[0], "");
        assert_eq!(
            sans_height, BODY_LINE_HEIGHT,
            "sans: 30 символов при ширине 240 — один ряд"
        );
        assert_eq!(
            mono_height,
            2.0 * BODY_LINE_HEIGHT,
            "mono: тот же текст — два ряда"
        );
    }

    /// Линия `---` — 12 px по тому же стеку: измерение её учитывает
    /// (зазоры вокруг линии — по 8, как у рендера).
    #[test]
    fn measure_body_height_counts_rule() {
        let height = measure_body_height("a\n\n---\n\nb", 300.0, &[], "");
        assert_eq!(
            height,
            BODY_LINE_HEIGHT + 8.0 + 12.0 + 8.0 + BODY_LINE_HEIGHT,
            "абзац + зазор 8 + линия 12 + зазор 8 + абзац"
        );
    }

    /// FR-061 этап C (D-7): заголовок блока-ведомости появляется при числе
    /// строк данных > T (Q2, дефолт 4) и наличии расчётных; текст —
    /// «▸ расчёт · N строк» с плюрализацией; позиция — перед первой
    /// расчётной строкой. Ниже порога заголовка нет.
    #[test]
    fn block_header_inserted_above_threshold_only() {
        let theme = ThemeColors::dark();
        let five = "a = 1\nb = 2\nc = 3\nd = 4\nd * 2";
        let items = body_items(
            &theme,
            five,
            &[0, 1, 2, 3, 4],
            &[],
            canvas_core::Language::Ru,
            true,
            None,
        );
        let header_pos = items
            .iter()
            .position(|item| item.header)
            .expect("заголовок блока вставлен");
        assert_eq!(items[header_pos].text, "▾ расчёт · 1 строка");
        // Следом — первая расчётная строка (source_line 4)
        assert_eq!(items[header_pos + 1].source_line, Some(4));
        // Ниже порога (4 строки данных) — заголовка нет
        let four = "a = 1\nb = 2\nc = 3\nd * 2";
        let items = body_items(
            &theme,
            four,
            &[0, 1, 2, 3],
            &[],
            canvas_core::Language::Ru,
            true,
            None,
        );
        assert!(
            items.iter().all(|item| !item.header),
            "порог T не достигнут"
        );
        // FR-061 хвосты (D-7 runtime v1): свёрнутый блок — шеврон «▸»,
        // следом превью-строка, расчётные строки СКРЫТЫ (блоков нет).
        let collapsed = body_items(
            &theme,
            five,
            &[0, 1, 2, 3, 4],
            &[],
            canvas_core::Language::Ru,
            false,
            None,
        );
        let header_pos = collapsed
            .iter()
            .position(|item| item.header)
            .expect("заголовок в свёрнутом виде есть");
        assert_eq!(collapsed[header_pos].text, "▸ расчёт · 1 строка");
        assert_eq!(
            collapsed[header_pos + 1].text,
            "параметры · 4 · формулы · 1 строка"
        );
        assert!(collapsed[header_pos + 1].preview, "второй элемент — превью");
        assert!(
            collapsed.iter().all(|item| item.source_line != Some(4)),
            "расчётная строка скрыта"
        );
    }

    /// FR-061 этап C (D-10/I-2): блок-режим меняет высоту тела, и измерение
    /// (`measure_body_height`) остаётся равным рендер-стеку — заголовок
    /// вставляется общим `body_items` (CR-012-инвариант).
    #[test]
    fn measure_matches_shape_with_block_header() {
        let text = "a = 1\nb = 2\nc = 3\nd = 4\nd * 2";
        let lines = &[0usize, 1, 2, 3, 4];
        let mut fs = FontSystem::new();
        for data in FONT_DATA {
            fs.db_mut().load_font_data((*data).to_vec());
        }
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            300.0,
            1.0,
            lines,
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        );
        let rendered = layout
            .blocks
            .iter()
            .map(|block| block.offset[1] + block.height)
            .fold(0.0f32, f32::max);
        let measured = measure_body_height(text, 300.0, lines, "");
        assert_eq!(
            measured, rendered,
            "блок-режим: measured {measured}, rendered {rendered}"
        );
        // Заголовок добавляет ещё ОДНУ строку тела (плюс 5-я строка
        // данных и зазоры стека) — рост через общий стек, не магию.
        let four = measure_body_height("a = 1\nb = 2\nc = 3\nd = 4", 300.0, &[0, 1, 2, 3], "");
        let five_h = measure_body_height(text, 300.0, lines, "");
        let diff = five_h - four;
        assert!(
            (2.0 * BODY_LINE_HEIGHT + 6.0..=2.0 * BODY_LINE_HEIGHT + 26.0).contains(&diff),
            "5-я строка + заголовок = две строки тела + зазоры (diff={diff})"
        );
    }

    /// Анти-дрейф: измерение и рендер делят один код стека (`with_body_stack`)
    /// — суммарная высота shape_body при zoom=1.0 (offset последнего блока +
    /// его высота, зазоры уже в offset) совпадает с измерением побайтово.
    #[test]
    fn measure_body_height_matches_shape_body_stack() {
        let text = "# Заголовок\nпроза строка\ndeploy = 40 $";
        let mut fs = FontSystem::new();
        for data in FONT_DATA {
            fs.db_mut().load_font_data((*data).to_vec());
        }
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            300.0,
            1.0,
            &[2],
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        );
        let rendered = layout
            .blocks
            .iter()
            .map(|block| block.offset[1] + block.height)
            .fold(0.0f32, f32::max);
        let measured = measure_body_height(text, 300.0, &[2], "");
        assert_eq!(
            measured, rendered,
            "измерение = рендер-стек: measured {measured}, rendered {rendered}"
        );
    }

    /// FR-067 (этап F): супрессия абзаца описания — измерение и рендер
    /// убирают абзац одинаково (I-2); desc == абзац добавляет МЕНЬШЕ
    /// высоты, чем постороннее описание той же длины (тело схлопнулось
    /// на высоту абзаца), и стект-паритет сохраняется.
    #[test]
    fn measure_and_shape_suppress_desc_paragraph_alike() {
        let text = "шлюз обрабатывает поток\nиз двух строк\n\ndeploy = 40 $";
        let para = canvas_core::expr::first_prose_paragraph(text).unwrap();
        let mut fs = FontSystem::new();
        for data in FONT_DATA {
            fs.db_mut().load_font_data((*data).to_vec());
        }
        // Ширина с запасом: абзац укладывается в ОДНУ строку зоны описания
        // (сравниваем именно супрессию тела, а не клампы описания)
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            text,
            420.0,
            1.0,
            &[3],
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            Some(para.as_str()),
            true,
            false,
        );
        let rendered = layout
            .blocks
            .iter()
            .map(|block| block.offset[1] + block.height)
            .fold(0.0f32, f32::max);
        let measured = measure_body_height(text, 420.0, &[3], &para);
        assert_eq!(
            measured, rendered,
            "паритет стека при супрессии абзаца"
        );
        // Абзац, показанный зоной описания, дешевле постороннего описания:
        // тело без абзаца против полного тела
        let other = measure_body_height(text, 420.0, &[3], "постороннее описание ноды");
        assert!(
            measured < other,
            "супрессия убрала абзац из тела: {measured} < {other}"
        );
    }

    /// Стек при зуме 2: квады в px виртуального буфера масштабируются.
    /// Буллит — в колонке-gutter: x = 4*zoom (без indent, он для текста).
    #[test]
    fn shape_body_quads_scale_with_zoom() {
        let mut fs = FontSystem::new();
        let layout = shape_body(
            &mut fs,
            &ThemeColors::dark(),
            "- a",
            300.0,
            2.0,
            &[],
            &[],
            Vec::new(),
            &[],
            canvas_core::Language::Ru,
            None,
            true,
            false,
        );
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
