//! FR-017 (CP6, волна B2): what-if нижний бар — чистая модель
//! (образец [`crate::settings_ui`]/[`crate::hints_ui`]): геометрия полосы
//! режима и пилюли входа, hit-тесты действий, раскрытый список подмен
//! со снятием отдельной подмены и таблица сравнения сценариев.
//!
//! Рендер и ввод — приложение (`app.rs`): бар собирается по кадру из
//! квадов + screen-текстов, клики перехватываются до канваса, побочные
//! эффекты (переключение сценария, Apply/Reset) — на стороне `App`.
//!
//! FR-053 (U3 PRD-0009, пилот G4/G5): ширины чипов/кнопок — ИЗМЕРЕННЫЕ
//! ([`TextMeasurer`] на реальном шейпинге cosmic-text, те же метрики, что
//! у рендера; эвристика `0.62·кегль` и запас `CHIP_SLACK` удалены —
//! урок CR-015 больше не нужен); подписи сценариев — Ellipsis-политика по
//! фактической ширине чипа (бывший посимвольный `truncate` удалён),
//! раскладка и отрисовка используют ОДНУ строку ([`BarLayout::scenario_labels`]);
//! хвост бара — примитив [`RowPolicy::SqueezeTail`] (бывшее замыкание
//! `take`), ширина бара — [`constrain`] к вьюпорту, поля — spacing-scale
//! `canvas_core::tokens::SPACING_*`.
//!
//! FR-068 W3.3: элементы бара — самоизмерение [`MeasuredItem::Text`]
//! (текст + пад, высота/мин-ширина — дизайн-константы); ручная проводка
//! «width_of → размер ребёнка» удалена, [`chip_width`]/[`btn_width`] —
//! только первый проход расчёта ширины бара.

use crate::ui::point_in_rect;
use canvas_ui::geometry::{EdgeInsets, UiRect, UiVec2};
use canvas_ui::kit;
use canvas_ui::layout::{
    constrain, pad, stack, CrossAlign, HAlign, MeasuredItem, Row, RowPolicy, VAlign,
};
use canvas_ui::measure::TextMeasurer;

/// Маржа бара от нижнего края окна (логические px; spacing-scale).
pub const BAR_MARGIN: f32 = canvas_core::tokens::SPACING_LG;
/// Внутренние поля полосы (spacing-scale).
pub const BAR_PADDING: f32 = canvas_core::tokens::SPACING_MD;
/// Высота полосы режима.
pub const BAR_HEIGHT: f32 = 44.0;
/// Высота пилюли входа (вне режима).
pub const PILL_HEIGHT: f32 = 30.0;
/// Ширина пилюли входа.
pub const PILL_WIDTH: f32 = 104.0;
/// Высота чипа сценария. LAY-W7 (аудит layouts-2026-10 §5): канонизация
/// на шкалу S3 — `kit::CHIP_HEIGHT` (24); ранее 26 (вне шкалы, +2px).
pub const CHIP_HEIGHT: f32 = kit::CHIP_HEIGHT;
/// Горизонтальные поля чипа (spacing-scale).
pub const CHIP_PAD_X: f32 = canvas_core::tokens::SPACING_LG;
/// Зазор между элементами бара (spacing-scale).
pub const BAR_GAP: f32 = canvas_core::tokens::SPACING_S;
/// Ширина индикатора «WHAT-IF».
pub const INDICATOR_WIDTH: f32 = 78.0;
/// Минимальная ширина кнопок Apply/Сброс/Сравнить (фактическая — по
/// измеренной подписи: [`MeasuredItem::Text`] c `min_w`; [`btn_width`] —
/// первый проход ширины бара).
pub const BTN_WIDTH: f32 = 74.0;
/// Горизонтальные поля кнопки (spacing-scale).
pub const BTN_PAD_X: f32 = canvas_core::tokens::SPACING_LG;
/// Кегль подписей чипов/кнопок бара (логические px; метка раскладки =
/// метка отрисовки — единый источник размера).
pub const CHIP_FONT: f32 = 13.0;
/// Ширина кнопки «✕».
pub const CLOSE_WIDTH: f32 = 28.0;
/// Высота строки раскрытого списка подмен. LAY-W7: канонизация на S3 —
/// `kit::LIST_ROW_H` (26); ранее 24 (на шкале только как CHIP_HEIGHT,
/// семантически — высота строки, не чипа).
pub const LIST_ROW_H: f32 = kit::LIST_ROW_H;
/// Поля раскрытого списка (spacing-scale).
pub const LIST_MARGIN: f32 = canvas_core::tokens::SPACING_S;
/// Ширина раскрытого списка.
pub const LIST_WIDTH: f32 = 480.0;
/// Ширина кнопки «✕» у строки подмены.
pub const REMOVE_WIDTH: f32 = 22.0;
/// Ширина слота «✕» удаления сценария в чипе бара (hit-зона ≥ визуала,
/// A3 design/rules/04; тот же размер, что у кнопки списка подмен —
/// один размер снятия в what-if).
pub const CHIP_CLOSE_W: f32 = 22.0;
/// Высота строки таблицы сравнения. LAY-W7: канонизация на S3 —
/// `kit::LIST_ROW_H` (26); ранее 24.
pub const TABLE_ROW_H: f32 = kit::LIST_ROW_H;
/// Высота шапки таблицы сравнения. LAY-W7: псевдоним `kit::LIST_ROW_H`
/// (значение уже совпадало — 26; выразим намерение через константу кита).
pub const TABLE_HEAD_H: f32 = kit::LIST_ROW_H;
/// Ширина колонки таблицы сравнения.
pub const TABLE_COL_W: f32 = 190.0;
/// Поля таблицы сравнения (spacing-scale).
pub const TABLE_MARGIN: f32 = canvas_core::tokens::SPACING_SM;

/// Семейство измерения = семейство screen-текстов рендера (parity метрик).
const FAMILY: &str = canvas_render::text::SANS_FAMILY;

/// Подписи бара what-if — строки приложения (i18n), по которым
/// СЧИТАЕТСЯ ШИРИНА и которые РИСУЮТСЯ. Фикс класса CR-015 (расширение
/// фикса FR-053 с counter/freeze на все элементы): раньше «База»/«Apply»/
/// «Сброс»/«Сравнить» были хардкодом RU внутри [`bar_layout`], а отрисовка
/// шла по i18n — в EN-локали чип «Compare» измерялся по «Сравнить» и
/// подпись переливалась на соседа (обрезалась). Один источник строк —
/// вызывающий (`App::whatif_bar_layout`), раскладка и отрисовка
/// используют ОДНИ И ТЕ ЖЕ подписи.
#[derive(Debug, Clone, Copy)]
pub struct BarLabels<'a> {
    /// Чип «База» (отключить подмены).
    pub base: &'a str,
    /// Кнопка Apply активного сценария.
    pub apply: &'a str,
    /// Кнопка «Сброс» (полный сброс сценариев — см. `BarAction::Reset`).
    pub reset: &'a str,
    /// FR-064 P2: кнопка «Заморозить»/«Разморозить» — лейбл по состоянию
    /// активного сценария (та же строка, что рисуется).
    pub freeze: &'a str,
    /// Кнопка «Сравнить» (таблица сравнения сценариев).
    pub compare: &'a str,
    /// Счётчик подмен активного сценария (i18n-строка с числом).
    pub counter: &'a str,
}

/// Действие клика по нижнему бару.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarAction {
    /// Пилюля «What-if» — вход в режим (свёрнутый вид).
    Enter,
    /// Чип «База» (отключить подмены).
    Base,
    /// Чип сценария `usize`.
    Scenario(usize),
    /// Чип «+» — новый сценарий.
    NewScenario,
    /// Счётчик подмен — раскрыть/скрыть список.
    ToggleOverrides,
    /// Apply активного сценария.
    Apply,
    /// Полный сброс what-if режима (все сценарии + подмены + заморозки,
    /// confirm-диалог — CJM-фикс).
    Reset,
    /// «✕» чипа сценария — удаление сценария `usize` (confirm-диалог,
    /// дизайн-док whatif-bar §5).
    DeleteScenario(usize),
    /// FR-064 P2: заморозка/разморозка активного сценария (снимок
    /// решений для сравнения сценариев).
    Freeze,
    /// Таблица сравнения сценариев.
    Compare,
    /// Кнопка «✕» — выход из режима.
    Close,
}

/// Геометрия полосы режима: rect целиком + rect'ы элементов.
#[derive(Debug, Clone, PartialEq)]
pub struct BarLayout {
    /// Rect полосы `[x, y, w, h]` (логические px).
    pub rect: [f32; 4],
    /// Индикатор «WHAT-IF».
    pub indicator: [f32; 4],
    /// Чип «База».
    pub base: [f32; 4],
    /// Чипы сценариев (порядок = порядок списка).
    pub scenarios: Vec<[f32; 4]>,
    /// Чип «+» (новый сценарий).
    pub new_scenario: [f32; 4],
    /// Слоты «✕» удаления сценария внутри чипов (параллелен `scenarios`;
    /// правый край чипа, полная высота — hit-зона ≥ визуала, A3).
    pub scenario_closes: Vec<[f32; 4]>,
    /// Счётчик подмен.
    pub overrides: [f32; 4],
    /// Кнопка «Apply».
    pub apply: [f32; 4],
    /// Кнопка «Сброс».
    pub reset: [f32; 4],
    /// Кнопка «Заморозить»/«Разморозить».
    /// FR-064 P2: кнопка «Заморозить»/«Разморозить» (лейбл — состояние
    /// активного сценария, измеряется та же строка, что рисуется).
    pub freeze: [f32; 4],
    /// Кнопка «Сравнить».
    pub compare: [f32; 4],
    /// Кнопка «✕».
    pub close: [f32; 4],
    /// Подписи чипов сценариев — Ellipsis-политика по фактической ширине
    /// чипа (параллелен `scenarios`); раскладка и отрисовка используют
    /// ОДНУ строку (урок CR-015).
    pub scenario_labels: Vec<String>,
}

/// Rect пилюли входа «What-if» (вне режима): низ-центр окна.
pub fn enter_pill_rect(viewport: [f32; 2]) -> [f32; 4] {
    [
        (viewport[0] - PILL_WIDTH) / 2.0,
        viewport[1] - BAR_MARGIN - PILL_HEIGHT,
        PILL_WIDTH,
        PILL_HEIGHT,
    ]
}

/// Измеренная ширина чипа по подписи: ширина текста + поля `CHIP_PAD_X`
/// (запас не нужен: ширина реальная, CR-015-эвристика удалена).
/// FR-068 W3.3: элементы бара сами себя измеряют ([`MeasuredItem::Text`]) —
/// функция осталась ТОЛЬКО для первого прохода (расчёт желаемой ширины
/// бара `desired` ниже, аналог двухпроходных направляющих); замер тот же
/// ключ кэша [`TextMeasurer`], что у резолва items — значения совпадают
/// бит-в-бит (одна точка измерения на подпись).
fn chip_width(label: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
    m.width_of(fs, label, FAMILY, CHIP_FONT) + CHIP_PAD_X * 2.0
}

/// Ширина кнопки по подписи: не уже `BTN_WIDTH`, поля `BTN_PAD_X`
/// (CR-015: «Сравнить» шире прежнего фикса 74 px и обрезалась).
/// FR-068 W3.3: первый проход (см. [`chip_width`]) — сами items кнопок
/// измеряются [`MeasuredItem::Text`] c `min_w: BTN_WIDTH`.
fn btn_width(label: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
    BTN_WIDTH.max(m.width_of(fs, label, FAMILY, CHIP_FONT) + BTN_PAD_X * 2.0)
}

/// Геометрия полосы режима. `scenario_names` — имена пользовательских
/// сценариев; `labels` — подписи бара ([`BarLabels`]: i18n-строки
/// приложения — ширина считается по ТЕМ ЖЕ строкам, что рисуются: фикс
/// класса CR-015, раньше «База»/«Сброс»/«Сравнить» были хардкодом RU).
/// Полоса центрируется по низу окна; ширина — сумма измеренных элементов,
/// сжатая [`constrain`] к вьюпорту.
pub fn bar_layout(
    scenario_names: &[String],
    labels: &BarLabels,
    viewport: [f32; 2],
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> BarLayout {
    let counter_w = chip_width(labels.counter, measurer, fs);
    // Чип сценария шире имени на слот «✕» удаления (right slot).
    let chips_w: f32 = scenario_names
        .iter()
        .map(|name| chip_width(name, measurer, fs) + CHIP_CLOSE_W)
        .sum::<f32>()
        + chip_width(labels.base, measurer, fs)
        + chip_width("+", measurer, fs);
    let controls_w = INDICATOR_WIDTH
        + counter_w
        + CLOSE_WIDTH
        + btn_width(labels.apply, measurer, fs)
        + btn_width(labels.reset, measurer, fs)
        + btn_width(labels.freeze, measurer, fs)
        + btn_width(labels.compare, measurer, fs);
    let elements = scenario_names.len() as f32 + 8.0; // чипы+База+«+»+инд+счёт+4кн+✕
    let desired = BAR_PADDING * 2.0 + chips_w + controls_w + BAR_GAP * (elements - 1.0).max(0.0);
    // Ширина бара: желаемая, сжатая к вьюпорту (примитив Constrain).
    let avail = (viewport[0] - BAR_MARGIN * 2.0).max(0.0);
    let width = constrain(
        UiVec2::new(0.0, 0.0),
        UiVec2::new(avail, f32::INFINITY),
        UiVec2::new(desired, BAR_HEIGHT),
    )
    .x;
    // Позиция: низ-центр вьюпорта с нижней маржёй (примитив Stack).
    let viewport_slot = pad(
        UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
        EdgeInsets {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: BAR_MARGIN,
        },
    );
    let bar = stack(
        viewport_slot,
        UiVec2::new(width, BAR_HEIGHT),
        HAlign::Center,
        VAlign::End,
    );
    // Элементы: слот с горизонтальными полями, высота полосы; крестовое
    // центрирование даёт cy = bar.y + (BAR_HEIGHT - CHIP_HEIGHT)/2.
    let items_slot = pad(
        bar,
        EdgeInsets {
            left: BAR_PADDING,
            top: 0.0,
            right: BAR_PADDING,
            bottom: 0.0,
        },
    );
    // Деградация узкого окна — именованная политика SqueezeTail: каждый
    // элемент получает min(желаемое, остаток), хвост сжимается до нуля
    // (вырожденные rect'ы невидимы и не пикаются); дословная семантика
    // прежнего замыкания `take` (CR-015).
    // FR-068 W3.3 (каталог docs/plans/fr-068-w3-consumer-migration.md
    // §9.3.1/§9.5): класс проводок «width_of → размер ребёнка» закрыт —
    // чип/кнопка сами себя измеряют ([`MeasuredItem::Text`]: текст + пад
    // `CHIP_PAD_X`/`BTN_PAD_X`, высота — дизайн-константа [`CHIP_HEIGHT`];
    // у кнопок пол ширины `BTN_WIDTH`). Пад-семантика появилась в F-13,
    // ручные замеры в items ушли. Геометрия бит-в-бит с прежней: `Text`
    // резолвится в `Fixed { w: width_of + pad_x }` (оракулы canvas-ui
    // `measured_text_pad_x_*`) и тот же движок SqueezeTail; подпись — та
    // же строка, что рисуется (counter_label/freeze_label — фикс FR-053).
    // Ширина бара (`desired` выше) считается по тому же замеру в первом
    // проходе ([`chip_width`]/[`btn_width`] — одна точка измерения).
    // Индикатор/«✕» — константные ширины (не текст) — остаются `Fixed`.
    let mut items: Vec<MeasuredItem> = Vec::with_capacity(scenario_names.len() + 8);
    items.push(MeasuredItem::Fixed {
        w: INDICATOR_WIDTH,
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Text {
        text: labels.base,
        max_w: None,
        min_w: 0.0,
        pad_x: CHIP_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    for name in scenario_names {
        items.push(MeasuredItem::Text {
            text: name.as_str(),
            max_w: None,
            min_w: 0.0,
            // pad_x симметричен; справа к нему добавляется слот «✕»
            // удаления сценария (правый край чипа — BarLayout::
            // scenario_closes).
            pad_x: CHIP_PAD_X * 2.0 + CHIP_CLOSE_W,
            h: Some(CHIP_HEIGHT),
        });
    }
    items.push(MeasuredItem::Text {
        text: "+",
        max_w: None,
        min_w: 0.0,
        pad_x: CHIP_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Text {
        text: labels.counter,
        max_w: None,
        min_w: 0.0,
        pad_x: CHIP_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Text {
        text: labels.apply,
        max_w: None,
        min_w: BTN_WIDTH,
        pad_x: BTN_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Text {
        text: labels.reset,
        max_w: None,
        min_w: BTN_WIDTH,
        pad_x: BTN_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Text {
        text: labels.freeze,
        max_w: None,
        min_w: BTN_WIDTH,
        pad_x: BTN_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Text {
        text: labels.compare,
        max_w: None,
        min_w: BTN_WIDTH,
        pad_x: BTN_PAD_X * 2.0,
        h: Some(CHIP_HEIGHT),
    });
    items.push(MeasuredItem::Fixed {
        w: CLOSE_WIDTH,
        h: CHIP_HEIGHT,
    });
    let rects = Row {
        gap: BAR_GAP,
        cross: CrossAlign::Center,
        policy: RowPolicy::SqueezeTail,
        ..Row::default()
    }
    .lay_out_measured(items_slot, &items, measurer, fs, FAMILY, CHIP_FONT);
    let n = scenario_names.len();
    let as_rect = |r: &UiRect| [r.x, r.y, r.w, r.h];
    // Слоты «✕» удаления: правый край чипа, полная высота (hit-зона A3).
    let scenario_closes: Vec<[f32; 4]> = rects[2..2 + n]
        .iter()
        .map(|r| [r.x + r.w - CHIP_CLOSE_W, r.y, CHIP_CLOSE_W, r.h])
        .collect();
    // Подписи сценариев — Ellipsis по фактической (возможно сжатой)
    // ширине чипа минус поля и слот «✕»: раскладка и отрисовка — одна строка.
    let scenario_labels = scenario_names
        .iter()
        .zip(rects[2..2 + n].iter())
        .map(|(name, rect)| {
            let inner = (rect.w - CHIP_PAD_X * 2.0 - CHIP_CLOSE_W).max(0.0);
            measurer.ellipsis(fs, name, FAMILY, CHIP_FONT, inner)
        })
        .collect();
    BarLayout {
        rect: [bar.x, bar.y, bar.w, bar.h],
        indicator: as_rect(&rects[0]),
        base: as_rect(&rects[1]),
        scenarios: rects[2..2 + n].iter().map(as_rect).collect(),
        new_scenario: as_rect(&rects[2 + n]),
        scenario_closes,
        overrides: as_rect(&rects[3 + n]),
        apply: as_rect(&rects[4 + n]),
        reset: as_rect(&rects[5 + n]),
        freeze: as_rect(&rects[6 + n]),
        compare: as_rect(&rects[7 + n]),
        close: as_rect(&rects[8 + n]),
        scenario_labels,
    }
}

/// Hit-test бара: какое действие под точкой. Счётчик кликабелен всегда;
/// Apply/Сброс — только при активном сценарии с подменами (гейт выше,
/// по `override_count == 0` кнопки рисуются приглушёнными, но rect есть).
///
/// LAY-W8 (FR-097): на coarse-указателе hit-зоны чипов/кнопок/слотов «✕»
/// дотягиваются до [`crate::touch_targets::MIN_TOUCH_TARGET`] (44 лог. px)
/// центрированно, с клампом в бар ([`BarLayout::rect`]) — расширенная зона
/// не выходит за полосу и не перекрывает соседние поверхности. Слот «✕»
/// удаления сценария клампится в свой чип ([`BarLayout::scenarios`]) —
/// расширенная зона не «крадёт» клики у соседнего чипа. На точном
/// указателе — rect без изменений (десктоп бит-в-бит прежний).
pub fn bar_action_at(layout: &BarLayout, point: [f32; 2]) -> Option<BarAction> {
    use crate::touch_targets::touch_hit_xywh;
    let bar = layout.rect;
    // FR-097: тач-цель ≥ 44 лог. px (кламп в бар)
    if point_in_rect(touch_hit_xywh(layout.close, bar), point) {
        return Some(BarAction::Close);
    }
    if point_in_rect(touch_hit_xywh(layout.apply, bar), point) {
        return Some(BarAction::Apply);
    }
    if point_in_rect(touch_hit_xywh(layout.reset, bar), point) {
        return Some(BarAction::Reset);
    }
    if point_in_rect(touch_hit_xywh(layout.compare, bar), point) {
        return Some(BarAction::Compare);
    }
    if point_in_rect(touch_hit_xywh(layout.freeze, bar), point) {
        return Some(BarAction::Freeze);
    }
    if point_in_rect(touch_hit_xywh(layout.overrides, bar), point) {
        return Some(BarAction::ToggleOverrides);
    }
    if point_in_rect(touch_hit_xywh(layout.new_scenario, bar), point) {
        return Some(BarAction::NewScenario);
    }
    // «✕» удаления сценария — ПЕРЕД телом чипа (слот — правый край чипа).
    // FR-097: тач-цель ≥ 44 (кламп в чип — не крадёт клики у соседа)
    for (i, rect) in layout.scenario_closes.iter().enumerate() {
        // Контейнер — чип сценария (не бар): расширенная зона «✕» не
        // выходит за пределы своего чипа и не налезает на следующий.
        let chip = layout.scenarios.get(i).copied().unwrap_or(bar);
        if point_in_rect(touch_hit_xywh(*rect, chip), point) {
            return Some(BarAction::DeleteScenario(i));
        }
    }
    for (i, rect) in layout.scenarios.iter().enumerate() {
        if point_in_rect(touch_hit_xywh(*rect, bar), point) {
            return Some(BarAction::Scenario(i));
        }
    }
    if point_in_rect(touch_hit_xywh(layout.base, bar), point) {
        return Some(BarAction::Base);
    }
    None
}

/// Вместимость рядов по внутренней высоте (без полей): если всё вмещается —
/// `count`; иначе — сколько рядов влезает с резервом слота под индикатор
/// усечения (хвост не пропадает молча — урок CR-015 про молчаливый take).
fn row_capacity(inner_h: f32, count: usize, row_h: f32) -> usize {
    if inner_h >= count as f32 * row_h {
        return count;
    }
    (((inner_h - row_h) / row_h).floor().max(0.0)) as usize
}

/// Сколько строк подмен фактически видно в rect списка (кламп высоты W-a).
/// Раскладка и отрисовка считают вместимость этой функцией — расхождений
/// нет (тот же источник геометрии у hit-теста, реестра и draw).
pub fn list_visible_rows(count: usize, list: [f32; 4]) -> usize {
    row_capacity((list[3] - LIST_MARGIN * 2.0).max(0.0), count, LIST_ROW_H)
}

/// Rect раскрытого списка подмен: над полосой, по её центру, кламп к окну.
/// W-a (дефект аудита §8 п.7): высота клампится к доступному месту от бара
/// до верха вьюпорта (`bar.y - 2*BAR_MARGIN`) — раньше десятки подмен
/// уезжали за верх. Не вмещается — видны вмещающиеся ряды + слот индикатора
/// усечения ([`list_visible_rows`] — сколько рядов рисовать).
pub fn overrides_list_layout(bar_rect: [f32; 4], count: usize, viewport: [f32; 2]) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    // Доступная высота: от верха вьюпорта до бара с маржой с обеих сторон.
    let available = (bar_rect[1] - 2.0 * BAR_MARGIN).max(0.0);
    let visible = row_capacity((available - LIST_MARGIN * 2.0).max(0.0), count, LIST_ROW_H);
    // Высота: все ряды — как раньше; при усечении — видимые ряды + один
    // слот под индикатор «… ещё N» (кэп `available` — на всякий случай).
    let height = if visible >= count {
        count as f32 * LIST_ROW_H + LIST_MARGIN * 2.0
    } else {
        (LIST_MARGIN * 2.0 + (visible + 1) as f32 * LIST_ROW_H)
            .min(available)
            .max(0.0)
    };
    let width = LIST_WIDTH.min((viewport[0] - BAR_MARGIN * 2.0).max(0.0));
    let x = (bar_rect[0] + bar_rect[2] / 2.0 - width / 2.0).clamp(
        BAR_MARGIN,
        (viewport[0] - width - BAR_MARGIN).max(BAR_MARGIN),
    );
    let y = (bar_rect[1] - height - 4.0).max(BAR_MARGIN);
    [x, y, width, height]
}

/// Rect строки подмены в раскрытом списке.
pub fn override_row_rect(list: [f32; 4], row: usize) -> [f32; 4] {
    [
        list[0] + LIST_MARGIN,
        list[1] + LIST_MARGIN + row as f32 * LIST_ROW_H,
        list[2] - LIST_MARGIN * 2.0,
        LIST_ROW_H,
    ]
}

/// Rect кнопки «✕» у строки подмены (правый край строки).
pub fn remove_button_rect(list: [f32; 4], row: usize) -> [f32; 4] {
    let row = override_row_rect(list, row);
    [
        row[0] + row[2] - REMOVE_WIDTH,
        row[1] + (LIST_ROW_H - REMOVE_WIDTH) / 2.0,
        REMOVE_WIDTH,
        REMOVE_WIDTH,
    ]
}

/// Hit-test строки подмены: индекс строки под точкой (`None` — поля,
/// мимо списка). Кнопка «✕» — часть строки (снятие отдельным хит-тестом).
///
/// LAY-W8 (FR-097): на coarse-указателе hit-зона строки (24 px) дотягивается
/// до 44 лог. px центрированно, с клампом в `list` — расширенная зона не
/// выходит за список и не «крадёт» клики у соседних строк за пределами
/// списка. На точном указателе — арифметика по строкам (24 px) без
/// расширения (десктоп бит-в-бит прежний).
pub fn override_row_at(list: [f32; 4], count: usize, point: [f32; 2]) -> Option<usize> {
    if !point_in_rect(list, point) {
        return None;
    }
    let visible = list_visible_rows(count, list);
    // FR-097: тач-цель строки ≥ 44 лог. px (кламп в список). Раньше —
    // арифметика rel/LIST_ROW_H (точный hit); теперь — итерация с
    // расширением (центрированно, кламп в list). На точном указателе
    // touch_hit_xywh — no-op, поведение прежнее.
    use crate::touch_targets::touch_hit_xywh;
    for row in 0..visible {
        let row_rect = override_row_rect(list, row);
        if point_in_rect(touch_hit_xywh(row_rect, list), point) {
            return Some(row);
        }
    }
    None
}

/// Геометрия таблицы сравнения: rect + rect'ы шапки и ячеек.
#[derive(Debug, Clone, PartialEq)]
pub struct TableLayout {
    /// Rect таблицы `[x, y, w, h]`.
    pub rect: [f32; 4],
    /// Rect'ы колонок шапки (подписи «переменная | База | С1 | С2»).
    pub header: Vec<[f32; 4]>,
    /// Ячейки `[row][col]` — только вмещающиеся по высоте строки
    /// (кламп W-a; хвост строк рисуется индикатором усечения).
    pub cells: Vec<Vec<[f32; 4]>>,
    /// W-a: слот полосы индикатора «… ещё N» при усечении (`None` —
    /// таблица вместила все строки).
    pub tail: Option<[f32; 4]>,
}

/// Геометрия таблицы сравнения: над баром по центру, кламп к окну.
/// `columns` — подписи колонок (первая — «переменная»), `rows` — число
/// строк сравнения. Таблица в v1 read-only — hit-тесты не нужны.
///
/// FR-UI-TABLE-IMMEDIATE (audit §6.2): сетка ячеек делегирована в
/// [`canvas_ui::kit::table_layout_immediate`] — immediate-API для таблиц
/// (без retained-state). Внешний rect (с клампом к бару/окну), расчёт
/// `visible` (усечение строк по высоте) и слот индикатора «… ещё N»
/// остаются у потребителя (whatif-специфика); сетка header+cells —
/// kit-функция с бит-в-бит паритетом (проверяется существующими тестами
/// `table_layout_*`).
pub fn table_layout(
    columns: &[String],
    rows: usize,
    bar_rect: [f32; 4],
    viewport: [f32; 2],
) -> TableLayout {
    use canvas_ui::geometry::UiRect;
    use canvas_ui::kit::table_layout_immediate;
    let cols = columns.len().max(1);
    let width = (TABLE_COL_W * cols as f32 + TABLE_MARGIN * 2.0)
        .min((viewport[0] - BAR_MARGIN * 2.0).max(0.0));
    // W-a (дефект аудита §8 п.7): высота клампится к доступному месту от
    // бара до верха вьюпорта (`bar.y - 2*BAR_MARGIN`) — раньше десятки строк
    // сравнения уезжали за верх.
    let available = (bar_rect[1] - 2.0 * BAR_MARGIN).max(0.0);
    let desired = TABLE_HEAD_H + rows as f32 * TABLE_ROW_H + TABLE_MARGIN * 2.0;
    let height = desired.min(available).max(0.0);
    let x = (bar_rect[0] + bar_rect[2] / 2.0 - width / 2.0).clamp(
        BAR_MARGIN,
        (viewport[0] - width - BAR_MARGIN).max(BAR_MARGIN),
    );
    let y = (bar_rect[1] - height - 4.0).max(BAR_MARGIN);
    // Видны только вмещающиеся строки; усечённый хвост — честный индикатор
    // «… ещё N» в зарезервированном слоте (никаких молчаливых take, CR-015).
    let visible = row_capacity(
        (height - TABLE_MARGIN * 2.0 - TABLE_HEAD_H).max(0.0),
        rows,
        TABLE_ROW_H,
    );
    let tail = if visible < rows {
        Some([
            x + TABLE_MARGIN,
            y + TABLE_MARGIN + TABLE_HEAD_H + visible as f32 * TABLE_ROW_H,
            width - TABLE_MARGIN * 2.0,
            TABLE_ROW_H,
        ])
    } else {
        None
    };
    // FR-UI-TABLE-IMMEDIATE: delegating cell grid to kit-function.
    // Slot — внутренний прямоугольник таблицы (после TABLE_MARGIN со всех
    // сторон); header — первой строкой, далее visible строк данных.
    let slot = UiRect::new(
        x + TABLE_MARGIN,
        y + TABLE_MARGIN,
        (width - TABLE_MARGIN * 2.0).max(0.0),
        (height - TABLE_MARGIN * 2.0).max(0.0),
    );
    let col_widths: Vec<f32> = vec![TABLE_COL_W; cols];
    let row_heights: Vec<f32> = vec![TABLE_ROW_H; visible];
    let grid: Vec<Vec<UiRect>> =
        table_layout_immediate(slot, &col_widths, &row_heights, Some(TABLE_HEAD_H));
    // Адаптер UiRect → [f32;4]: первая строка — header, остальное — cells.
    let to_arr = |r: &UiRect| [r.x, r.y, r.w, r.h];
    let header: Vec<[f32; 4]> = grid
        .first()
        .map(|row| row.iter().map(to_arr).collect())
        .unwrap_or_default();
    let cells: Vec<Vec<[f32; 4]>> = grid
        .iter()
        .skip(if header.is_empty() { 0 } else { 1 })
        .map(|row| row.iter().map(to_arr).collect())
        .collect();
    TableLayout {
        rect: [x, y, width, height],
        header,
        cells,
        tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_ui::geometry::UiRect;

    /// LAY-W7 (аудит layouts-2026-10 §5): высоты what-if на шкале S3 —
    /// `CHIP_HEIGHT`/`LIST_ROW_H`/`TABLE_ROW_H`/`TABLE_HEAD_H` суть
    /// псевдонимы канонических констант кита. Численная проверка
    /// фиксирует инвариант: смена значения в ките подхватывается here
    /// автоматически (без локальных литералов).
    #[test]
    fn lay_w7_heights_are_canonical_s3() {
        assert_eq!(CHIP_HEIGHT, kit::CHIP_HEIGHT);
        assert_eq!(LIST_ROW_H, kit::LIST_ROW_H);
        assert_eq!(TABLE_ROW_H, kit::LIST_ROW_H);
        assert_eq!(TABLE_HEAD_H, kit::LIST_ROW_H);
    }

    /// Детерминированный FontSystem тестов: вшитый рендером шрифт (тот же
    /// файл, что FONT_DATA canvas-render) — метрики одинаковы на всех CI.
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
    }

    fn layout(names: &[String], counter: &str, viewport: [f32; 2]) -> BarLayout {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let labels = BarLabels {
            base: "База",
            apply: "Apply",
            reset: "Сброс",
            freeze: "❄ Заморозить",
            compare: "Сравнить",
            counter,
        };
        bar_layout(names, &labels, viewport, &mut m, &mut fs)
    }

    /// EN-подписи бара (регресс фикса класса CR-015): раскладка измеряет
    /// ПЕРЕДАННЫЕ строки — кнопка не уже своей фактической EN-подписи
    /// (раньше «Compare» измерялся по хардкоду «Сравнить» и переливался).
    #[test]
    fn bar_layout_covers_en_labels() {
        let names: Vec<String> = vec!["Growth ×2".to_owned()];
        let labels = BarLabels {
            base: "Base",
            apply: "Apply",
            reset: "Reset",
            freeze: "❄ Freeze",
            compare: "Compare",
            counter: "overrides: 12",
        };
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let lay = bar_layout(&names, &labels, [1600.0, 900.0], &mut m, &mut fs);
        let mut cover = |rect: [f32; 4], label: &str, tag: &str| {
            assert!(
                rect[2] >= m.width_of(&mut fs, label, FAMILY, CHIP_FONT) + BTN_PAD_X * 2.0 - 0.01,
                "кнопка «{tag}» уже своей EN-подписи"
            );
        };
        cover(lay.apply, labels.apply, "Apply");
        cover(lay.reset, labels.reset, "Reset");
        cover(lay.freeze, labels.freeze, "Freeze");
        cover(lay.compare, labels.compare, "Compare");
        assert!(
            lay.base[2]
                >= m.width_of(&mut fs, labels.base, FAMILY, CHIP_FONT) + CHIP_PAD_X * 2.0 - 0.01,
            "чип «Base» уже своей EN-подписи"
        );
    }

    fn names() -> Vec<String> {
        vec!["Рост ×2".to_owned(), "Отказ реплики".to_owned()]
    }

    /// Пилюля входа — низ-центр, в пределах окна.
    #[test]
    fn enter_pill_bottom_center() {
        let rect = enter_pill_rect([800.0, 600.0]);
        assert_eq!(rect[1] + rect[3], 600.0 - BAR_MARGIN);
        assert_eq!(rect[0] + rect[2] / 2.0, 400.0);
        assert_eq!(rect[2], PILL_WIDTH);
    }

    /// Элементы бара идут слева направо без наложений; полоса в пределах
    /// окна; hit-тесты находят каждый элемент по своему rect.
    #[test]
    fn bar_layout_elements_and_hits() {
        let viewport = [1600.0, 900.0];
        let layout = layout(&names(), "подмен: 3", viewport);
        assert!(layout.rect[1] + layout.rect[3] <= viewport[1] - BAR_MARGIN + 0.01);
        // Порядок слева направо: индикатор < База < С1 < С2 < «+» < счётчик
        // < Apply < Сброс < Заморозить < Сравнить < ✕
        let xs = |r: [f32; 4]| r[0];
        assert!(xs(layout.indicator) < xs(layout.base));
        assert!(xs(layout.base) < xs(layout.scenarios[0]));
        assert!(xs(layout.scenarios[0]) < xs(layout.scenarios[1]));
        assert!(xs(layout.scenarios[1]) < xs(layout.new_scenario));
        assert!(xs(layout.new_scenario) < xs(layout.overrides));
        assert!(xs(layout.overrides) < xs(layout.apply));
        assert!(xs(layout.apply) < xs(layout.reset));
        assert!(xs(layout.reset) < xs(layout.freeze));
        assert!(xs(layout.freeze) < xs(layout.compare));
        assert!(xs(layout.compare) < xs(layout.close));
        // Hit-тесты
        let hit = |r: [f32; 4]| [r[0] + 3.0, r[1] + 3.0];
        assert_eq!(
            bar_action_at(&layout, hit(layout.close)),
            Some(BarAction::Close)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.apply)),
            Some(BarAction::Apply)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.reset)),
            Some(BarAction::Reset)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.freeze)),
            Some(BarAction::Freeze)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.compare)),
            Some(BarAction::Compare)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.overrides)),
            Some(BarAction::ToggleOverrides)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.new_scenario)),
            Some(BarAction::NewScenario)
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.scenarios[1])),
            Some(BarAction::Scenario(1))
        );
        assert_eq!(
            bar_action_at(&layout, hit(layout.base)),
            Some(BarAction::Base)
        );
        // Мимо бара — None
        assert_eq!(bar_action_at(&layout, [5.0, 5.0]), None);
    }

    /// LAY-W8 (FR-097): на coarse-указателе hit-зона чипа/кнопки бара
    /// дотягивается до 44 лог. px центрированно, с клампом в бар — клик
    /// 5 px выше чипа «База» (но внутри 44-px расширенной зоны и внутри
    /// бара) попадает в `BarAction::Base`. На точном указателе тот же
    /// клик проходит мимо. Симметричный сценарий «coarse vs precise».
    #[test]
    fn lay_w8_bar_action_coarse_expands_chip_hit_zone() {
        let layout = layout(&names(), "подмен: 3", [1600.0, 900.0]);
        let chip = layout.base;
        // 5 px ниже чипа, но внутри бара (BAR_HEIGHT=44, CHIP_HEIGHT=24 →
        // под чипом есть место для расширения центрированно по Y).
        let below = [chip[0] + chip[2] / 2.0, chip[1] + chip[3] + 5.0];
        // Точка внутри бара (проходит внешний гейт point_in_rect(bar)).
        assert!(
            below[1] >= layout.rect[1] && below[1] <= layout.rect[1] + layout.rect[3],
            "below={below:?} bar={:?}",
            layout.rect
        );

        let was = canvas_core::web_bridge::pointer_coarse();
        // Coarse: расширение — клик попадает в Base.
        canvas_core::web_bridge::set_pointer_coarse(true);
        assert_eq!(
            bar_action_at(&layout, below),
            Some(BarAction::Base),
            "coarse: клик 5 px ниже чипа «База» должен попасть в Base"
        );
        // Precise: без расширения — клик мимо (None или другой элемент).
        canvas_core::web_bridge::set_pointer_coarse(false);
        assert_ne!(
            bar_action_at(&layout, below),
            Some(BarAction::Base),
            "precise: клик 5 px ниже чипа «База» не должен попасть в Base"
        );
        canvas_core::web_bridge::set_pointer_coarse(was);
    }

    /// LAY-W8: на coarse hit-зона слота «✕» удаления сценария (22 px)
    /// дотягивается до 44 центрированно, с клампом в чип сценария —
    /// расширенная зона не крадёт клики у соседнего чипа. Клик 5 px левее
    /// слота «✕» (но внутри чипа) попадает в `DeleteScenario(i)` на coarse,
    /// в `Scenario(i)` на precise (тело чипа). На точном — прежнее поведение.
    #[test]
    fn lay_w8_bar_action_close_slot_clamps_to_chip() {
        let layout = layout(&names(), "подмен: 3", [1600.0, 900.0]);
        let i = 0;
        let close = layout.scenario_closes[i];
        let chip = layout.scenarios[i];
        // 5 px левее слота «✕», в теле чипа (между подписью и «✕»).
        let left_of_close = [close[0] - 5.0, close[1] + close[3] / 2.0];
        // Точка внутри чипа — иначе выйдет за рамки теста.
        assert!(left_of_close[0] >= chip[0]);

        let was = canvas_core::web_bridge::pointer_coarse();
        // Coarse: расширенный «✕» (кламп в чип) ловит клик левее слота.
        canvas_core::web_bridge::set_pointer_coarse(true);
        assert_eq!(
            bar_action_at(&layout, left_of_close),
            Some(BarAction::DeleteScenario(i)),
            "coarse: клик 5 px левее «✕» в пределах чипа — DeleteScenario"
        );
        // Precise: слот 22 px — клик левее слота, в теле чипа → Scenario(i).
        canvas_core::web_bridge::set_pointer_coarse(false);
        assert_eq!(
            bar_action_at(&layout, left_of_close),
            Some(BarAction::Scenario(i)),
            "precise: клик 5 px левее «✕» — тело чипа, Scenario(i)"
        );
        canvas_core::web_bridge::set_pointer_coarse(was);
    }

    /// LAY-W8: `override_row_at` на coarse расширяет hit-зону строки (24 px)
    /// до 44 центрированно, с клампом в список. Клик в верхней марже списка
    /// (выше строки 0 на 3 px, в пределах 44-px расширенной зоны) попадает
    /// в строку 0 на coarse; на precise — None (поведение прежнее).
    #[test]
    fn lay_w8_override_row_at_coarse_expands_row() {
        let bar = [500.0, 800.0, 600.0, BAR_HEIGHT];
        let list = overrides_list_layout(bar, 3, [1600.0, 900.0]);
        // LIST_MARGIN = SPACING_S = 6. Точка в верхней марже списка —
        // 3 px от верха списка, выше строки 0 на 3 px (6-3=3).
        let in_top_margin = [list[0] + list[2] / 2.0, list[1] + 3.0];
        // Точка внутри list (проходит внешний гейт point_in_rect(list)).
        assert!(in_top_margin[1] >= list[1] && in_top_margin[1] <= list[1] + list[3]);

        let was = canvas_core::web_bridge::pointer_coarse();
        // Precise: верхняя маржа — None (поведение прежнее, rel < 0).
        canvas_core::web_bridge::set_pointer_coarse(false);
        assert_eq!(
            override_row_at(list, 3, in_top_margin),
            None,
            "precise: клик в верхней марже списка — None"
        );
        // Coarse: расширенная строка 0 ловит клик в марже.
        canvas_core::web_bridge::set_pointer_coarse(true);
        assert_eq!(
            override_row_at(list, 3, in_top_margin),
            Some(0),
            "coarse: клик в верхней марже — строка 0 (расширенная hit-зона)"
        );
        canvas_core::web_bridge::set_pointer_coarse(was);
    }

    /// Узкое окно: полоса клампится внутрь (ширина <= окно - 2×margin).
    #[test]
    fn bar_layout_clamps_narrow_window() {
        let layout = layout(&names(), "подмен: 1", [360.0, 240.0]);
        assert!(layout.rect[0] >= BAR_MARGIN - 0.01);
        assert!(layout.rect[0] + layout.rect[2] <= 360.0 - BAR_MARGIN + 0.01);
    }

    /// Список подмен: кламп к окну, строки по порядку, «✕» — в правой
    /// части своей строки; hit-тесты строк и кнопки снятия.
    #[test]
    fn overrides_list_layout_and_hits() {
        let bar = [500.0, 800.0, 600.0, BAR_HEIGHT];
        assert_eq!(overrides_list_layout(bar, 0, [1600.0, 900.0]), [0.0; 4]);
        let list = overrides_list_layout(bar, 3, [1600.0, 900.0]);
        assert!(list[1] + list[3] <= bar[1] - 4.0 + 0.01, "список над баром");
        assert_eq!(
            override_row_at(list, 3, [list[0] + 5.0, list[1] + 2.0]),
            None,
            "поле"
        );
        let row1 = override_row_rect(list, 1);
        assert_eq!(
            override_row_at(list, 3, [row1[0] + 10.0, row1[1] + 3.0]),
            Some(1)
        );
        assert_eq!(
            override_row_at(list, 3, [row1[0] + 10.0, list[1] - 2.0]),
            None
        );
        let remove = remove_button_rect(list, 1);
        assert!(
            point_in_rect(row1, [remove[0] + 2.0, remove[1] + 2.0]),
            "✕ внутри строки"
        );
        assert!(remove[0] + remove[2] <= row1[0] + row1[2] + 0.01);
        // У нижнего края окна список клампится внутрь
        let low = overrides_list_layout([500.0, 40.0, 600.0, BAR_HEIGHT], 5, [1600.0, 900.0]);
        assert!(low[1] >= BAR_MARGIN - 0.01);
    }

    /// Таблица сравнения: шапка + ячейки сеткой, кламп к окну.
    #[test]
    fn table_layout_grid_and_clamp() {
        let columns: Vec<String> = ["переменная", "База", "С1", "С2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let bar = [500.0, 800.0, 600.0, BAR_HEIGHT];
        let layout = table_layout(&columns, 4, bar, [1600.0, 900.0]);
        assert_eq!(layout.header.len(), 4);
        assert_eq!(layout.cells.len(), 4);
        assert_eq!(layout.cells[0].len(), 4);
        // Шапка выше ячеек, колонки — слева направо
        assert!(layout.header[0][1] < layout.cells[0][0][1]);
        assert!(layout.header[0][0] < layout.header[1][0]);
        assert!(layout.cells[0][0][1] < layout.cells[1][0][1]);
        assert!(layout.rect[1] + layout.rect[3] <= bar[1] - 4.0 + 0.01);
        // Узкое окно — таблица клампится, ширина <= окно
        let narrow = table_layout(&columns, 2, bar, [400.0, 300.0]);
        assert!(narrow.rect[0] >= BAR_MARGIN - 0.01);
        assert!(narrow.rect[0] + narrow.rect[2] <= 400.0 - BAR_MARGIN + 0.01);
    }

    /// CR-015: типичный набор бара (скриншот пользователя) — соседние
    /// элементы не пересекаются, каждый чип/кнопка не уже своей
    /// ИЗМЕРЕННОЙ подписи (текст не переливается на соседа).
    #[test]
    fn bar_layout_no_overlap_and_covers_labels() {
        let names: Vec<String> = ["Сценарий 3", "Сценарий 2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let counter = "подмен: 0";
        let lay = layout(&names, counter, [1600.0, 900.0]);
        let rects = [
            lay.indicator,
            lay.base,
            lay.scenarios[0],
            lay.scenarios[1],
            lay.new_scenario,
            lay.overrides,
            lay.apply,
            lay.reset,
            lay.compare,
            lay.close,
        ];
        for w in rects.windows(2) {
            assert!(
                w[0][0] + w[0][2] <= w[1][0] + 0.01,
                "соседние элементы пересекаются"
            );
        }
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        // Чип: ширина = измеренная подпись + 2×CHIP_PAD_X (точно).
        let wanted_base = test_chip_width("База", &mut m, &mut fs);
        assert!(lay.base[2] >= wanted_base - 0.01, "чип «База» уже подписи");
        let wanted_counter = test_chip_width(counter, &mut m, &mut fs);
        assert!(
            lay.overrides[2] >= wanted_counter - 0.01,
            "чип счётчика уже подписи"
        );
        // Метка сценария из раскладки помещается в свой чип (минус поля
        // и слот «✕» удаления).
        for (rect, label) in lay.scenarios.iter().zip(lay.scenario_labels.iter()) {
            let label_w = m.width_of(&mut fs, label, FAMILY, CHIP_FONT);
            assert!(
                label_w <= rect[2] - CHIP_PAD_X * 2.0 - CHIP_CLOSE_W + 0.01,
                "метка «{label}» ({label_w}) шире чипа без слота «✕»"
            );
        }
        let mut btn_cover = |rect: [f32; 4], label: &str| {
            assert!(
                rect[2] >= m.width_of(&mut fs, label, FAMILY, CHIP_FONT) + BTN_PAD_X * 2.0 - 0.01,
                "кнопка «{label}» уже подписи"
            );
        };
        btn_cover(lay.apply, "Apply");
        btn_cover(lay.reset, "Сброс");
        btn_cover(lay.compare, "Сравнить");
    }

    /// Измеренная ширина чипа по подписи (тестовый контракт внутренней
    /// `chip_width`).
    fn test_chip_width(label: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
        m.width_of(fs, label, FAMILY, CHIP_FONT) + CHIP_PAD_X * 2.0
    }

    /// FR-053: длинные имена сценариев усекаются Ellipsis-политикой по
    /// фактической ширине чипа: на широком окне чип по измеренной ширине
    /// имени (усечения нет), в узком окне SqueezeTail сжимает чип —
    /// подпись усекается «…» (подпись раскладки = подпись отрисовки).
    #[test]
    fn scenario_labels_ellipsis_by_measured_width() {
        let long = "Очень длинное имя сценария с деталями эксперимента".to_owned();
        let names = vec![long.clone()];
        // Широкое окно: чип sized по измеренной подписи — имя целиком.
        let wide = layout(&names, "подмен: 0", [1600.0, 900.0]);
        assert_eq!(wide.scenario_labels.len(), 1);
        assert_eq!(wide.scenario_labels[0], long, "помещается — не усекается");
        // Узкое окно: чип сжат политикой — подпись усечена и укладывается.
        let narrow = layout(&names, "подмен: 0", [420.0, 400.0]);
        let label = &narrow.scenario_labels[0];
        assert_ne!(label.as_str(), long, "сжатый чип — имя усечено");
        assert!(label.ends_with('\u{2026}'), "усечение — многоточием");
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        let label_w = m.width_of(&mut fs, label, FAMILY, CHIP_FONT);
        assert!(
            label_w <= narrow.scenarios[0][2] - CHIP_PAD_X * 2.0 - CHIP_CLOSE_W + 0.01,
            "усечённая подпись укладывается в сжатый чип (без слота «✕»)"
        );
        // Короткое имя не усекается.
        let short_names = vec!["С1".to_owned()];
        let short = layout(&short_names, "подмен: 0", [1600.0, 900.0]);
        assert_eq!(short.scenario_labels[0], "С1");
    }

    /// «✕» удаления сценария: слот — правый край чипа (полная высота,
    /// hit-зона A3), hit-тест различает удаление и активацию, подпись
    /// не наезжает на слот.
    #[test]
    fn scenario_chip_close_slot_hit_and_layout() {
        let layout = layout(&names(), "подмен: 2", [1600.0, 900.0]);
        assert_eq!(layout.scenario_closes.len(), layout.scenarios.len());
        for (chip, close) in layout.scenarios.iter().zip(layout.scenario_closes.iter()) {
            // Слот — у правого края чипа, той же высоты.
            assert!((close[0] + close[2] - (chip[0] + chip[2])).abs() <= 0.01);
            assert_eq!(close[1], chip[1]);
            assert_eq!(close[3], chip[3]);
        }
        // Прямой hit-тест: точка в слоте → DeleteScenario, точка в теле
        // (левая часть чипа) → Scenario(i).
        for i in 0..layout.scenarios.len() {
            let close = layout.scenario_closes[i];
            let point = [close[0] + close[2] / 2.0, close[1] + close[3] / 2.0];
            assert_eq!(
                bar_action_at(&layout, point),
                Some(BarAction::DeleteScenario(i)),
                "«✕» чипа {i} — удаление сценария"
            );
            let chip = layout.scenarios[i];
            let body = [chip[0] + 4.0, chip[1] + chip[3] / 2.0];
            assert_eq!(
                bar_action_at(&layout, body),
                Some(BarAction::Scenario(i)),
                "тело чипа {i} — активация сценария"
            );
            // Подпись не наезжает на слот «✕».
            let label = &layout.scenario_labels[i];
            let mut fs = font_system();
            let mut m = TextMeasurer::new();
            let label_w = m.width_of(&mut fs, label, FAMILY, CHIP_FONT);
            assert!(
                CHIP_PAD_X + label_w <= close[0] - chip[0] + 0.01,
                "подпись «{label}» наезжает на слот «✕»"
            );
        }
    }

    /// CR-015: ширина кнопки — не уже минимума и покрывает подпись.
    #[test]
    fn btn_width_covers_labels() {
        let mut fs = font_system();
        let mut m = TextMeasurer::new();
        assert!(btn_width("Apply", &mut m, &mut fs) >= BTN_WIDTH);
        assert!(btn_width("Сравнить", &mut m, &mut fs) >= BTN_WIDTH);
        assert!(
            btn_width("Сравнить", &mut m, &mut fs)
                >= m.width_of(&mut fs, "Сравнить", FAMILY, CHIP_FONT) + BTN_PAD_X * 2.0 - 0.01
        );
    }

    /// CR-015: узкое окно — все элементы внутри rect бара, правый край
    /// ничего не уходит за полосу (хвост ужимается SqueezeTail-политикой).
    #[test]
    fn bar_layout_narrow_window_keeps_elements_inside() {
        let layout = layout(&names(), "подмен: 1", [400.0, 240.0]);
        let right = layout.rect[0] + layout.rect[2] - BAR_PADDING + 0.01;
        let left = layout.rect[0] + BAR_PADDING - 0.01;
        for r in [
            layout.indicator,
            layout.base,
            layout.new_scenario,
            layout.overrides,
            layout.apply,
            layout.reset,
            layout.compare,
            layout.close,
        ]
        .into_iter()
        .chain(layout.scenarios.iter().copied())
        {
            assert!(r[0] >= left, "элемент левее бара");
            assert!(r[0] + r[2] <= right, "элемент за правым краем бара");
        }
    }

    /// FR-053 (G4-линт пилота): вьюпорты 1280×800 / 1024×640 / 800×560 ×
    /// RU/EN × короткие/длинные/много сценариев — 0 пересечений
    /// интерактивных rect'ов бара, 0 выходов за вьюпорт; повтор кадровым
    /// путём U2 (UiFrame.overlaps_within_layer — поверхности пилотов на
    /// своих слоях не пересекаются).
    #[test]
    fn g4_lint_viewports_and_languages() {
        let short: Vec<String> = vec!["С1".into(), "С2".into()];
        let long: Vec<String> = vec![
            "Сценарий с очень длинным описанием эксперимента".into(),
            "Короткий".into(),
            "Ещё один длинный сценарий отказоустойчивости кластера".into(),
        ];
        let many: Vec<String> = (0..8).map(|i| format!("Сценарий {i}")).collect();
        let counters = ["подмен: 3", "overrides: 3"];
        let viewports = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];
        for names in [&short, &long, &many] {
            for counter in counters {
                for vp in viewports {
                    let lay = layout(names, counter, vp);
                    // Бар внутри вьюпорта (маржа lg).
                    assert!(lay.rect[0] >= BAR_MARGIN - 0.01, "bar left {vp:?}");
                    assert!(
                        lay.rect[0] + lay.rect[2] <= vp[0] - BAR_MARGIN + 0.01,
                        "bar right {vp:?}"
                    );
                    assert!(lay.rect[1] >= BAR_MARGIN - 0.01, "bar top {vp:?}");
                    // Элементы попарно не пересекаются (полуоткрытые rect'ы,
                    // вырожденные — пустые: intersects = false).
                    let all: Vec<[f32; 4]> = [
                        lay.indicator,
                        lay.base,
                        lay.new_scenario,
                        lay.overrides,
                        lay.apply,
                        lay.reset,
                        lay.compare,
                        lay.close,
                    ]
                    .into_iter()
                    .chain(lay.scenarios.iter().copied())
                    .collect();
                    for i in 0..all.len() {
                        for j in i + 1..all.len() {
                            let a = UiRect::new(all[i][0], all[i][1], all[i][2], all[i][3]);
                            let b = UiRect::new(all[j][0], all[j][1], all[j][2], all[j][3]);
                            assert!(
                                !a.intersects(&b),
                                "пересечение {i}×{j} при {vp:?}/{counter}"
                            );
                        }
                    }
                    // Кадровый путь U2: whatif (Panels) и gallery (Modals)
                    // с элементами бара/панели не пересекаются в своих слоях.
                    let mut reg = canvas_ui::SurfaceRegistry::new();
                    reg.add(canvas_ui::SurfaceDecl::new(
                        "whatif",
                        canvas_ui::UiLayer::Panels,
                        canvas_ui::CapturePolicy::Capture,
                    ));
                    reg.add(canvas_ui::SurfaceDecl::new(
                        "gallery",
                        canvas_ui::UiLayer::Modals,
                        canvas_ui::CapturePolicy::Block,
                    ));
                    let mut frame = canvas_ui::UiFrame::from_registry(
                        &reg,
                        UiRect::new(0.0, 0.0, vp[0], vp[1]),
                    );
                    for s in frame.surfaces.iter_mut() {
                        match s.surface.as_str() {
                            "whatif" => {
                                s.hit_rects.push(canvas_ui::HitRect::interactive(
                                    UiRect::new(lay.rect[0], lay.rect[1], lay.rect[2], lay.rect[3]),
                                    "whatif-bar",
                                ));
                                s.hit_rects.push(canvas_ui::HitRect::interactive(
                                    UiRect::new(
                                        lay.apply[0],
                                        lay.apply[1],
                                        lay.apply[2],
                                        lay.apply[3],
                                    ),
                                    "whatif-apply",
                                ));
                            }
                            "gallery" => {
                                s.hit_rects.push(canvas_ui::HitRect::interactive(
                                    UiRect::new(360.0, 120.0, 560.0, 400.0),
                                    "gallery-panel",
                                ));
                            }
                            _ => {}
                        }
                    }
                    assert!(
                        frame.overlaps_within_layer().is_empty(),
                        "пересечения слоёв при {vp:?}/{counter}"
                    );
                }
            }
        }
    }

    /// W-a (дефект аудита §8 п.7): кламп высоты списка подмен — при
    /// count=50 на 900×600 и 800×560 rect целиком в вьюпорте и над баром;
    /// видны только вмещающиеся ряды + слот индикатора усечения; хвост
    /// не пикается (hit-тест согласован с раскладкой).
    #[test]
    fn overrides_list_layout_clamps_height() {
        let count = 50usize;
        for viewport in [[900.0, 600.0], [800.0, 560.0]] {
            let bar = [
                viewport[0] / 2.0 - 300.0,
                viewport[1] - BAR_MARGIN - BAR_HEIGHT,
                600.0,
                BAR_HEIGHT,
            ];
            let list = overrides_list_layout(bar, count, viewport);
            // Целиком в вьюпорте: не выше маржи верха и не наезжает на бар.
            assert!(list[1] >= BAR_MARGIN - 0.01, "{viewport:?}");
            assert!(
                list[1] + list[3] <= bar[1] - 4.0 + 0.01,
                "список наехал на бар: {viewport:?}"
            );
            // Кэп высоты = доступное место от бара до верха.
            assert!(list[3] <= bar[1] - 2.0 * BAR_MARGIN + 0.01, "{viewport:?}");
            // Усечение честное: видимых рядов меньше count, но добрый десяток.
            let visible = list_visible_rows(count, list);
            assert!(visible < count, "хвост усечён: {viewport:?}");
            assert!(visible >= 10, "влезает добрый десяток: {viewport:?}");
            // Последний видимый ряд целиком внутри списка (поле снизу).
            let last = override_row_rect(list, visible - 1);
            assert!(last[1] + last[3] <= list[1] + list[3] - LIST_MARGIN + 0.01);
            // Слот индикатора — сразу под последним рядом, внутри списка.
            let slot_y = list[1] + LIST_MARGIN + visible as f32 * LIST_ROW_H;
            assert!(slot_y + LIST_ROW_H <= list[1] + list[3] - LIST_MARGIN + 0.01);
            // Hit-тест: последний видимый ряд находится, хвост/слот — нет.
            assert_eq!(
                override_row_at(list, count, [list[0] + 10.0, last[1] + 3.0]),
                Some(visible - 1)
            );
            assert_eq!(
                override_row_at(list, count, [list[0] + 10.0, slot_y + 5.0]),
                None,
                "слот индикатора не пикается как ряд"
            );
            // Соседние видимые ряды не налагаются (шаг LIST_ROW_H).
            for r in 1..visible {
                let prev = override_row_rect(list, r - 1);
                let cur = override_row_rect(list, r);
                assert!(prev[1] + prev[3] <= cur[1] + 0.01);
            }
        }
    }

    /// W-a: кламп высоты таблицы сравнения — при 50 строках на 900×600 и
    /// 800×560 rect целиком в вьюпорте; ячеек — только вмещающие, слот
    /// индикатора хвоста внутри таблицы; вмещающаяся таблица — прежняя
    /// геометрия (tail = None).
    #[test]
    fn table_layout_clamps_height() {
        let columns: Vec<String> = ["переменная", "База", "С1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let rows = 50usize;
        for viewport in [[900.0, 600.0], [800.0, 560.0]] {
            let bar = [
                viewport[0] / 2.0 - 300.0,
                viewport[1] - BAR_MARGIN - BAR_HEIGHT,
                600.0,
                BAR_HEIGHT,
            ];
            let table = table_layout(&columns, rows, bar, viewport);
            // Целиком в вьюпорте и над баром; кэп = доступное место.
            assert!(table.rect[1] >= BAR_MARGIN - 0.01, "{viewport:?}");
            assert!(
                table.rect[1] + table.rect[3] <= bar[1] - 4.0 + 0.01,
                "{viewport:?}"
            );
            assert!(
                table.rect[3] <= bar[1] - 2.0 * BAR_MARGIN + 0.01,
                "{viewport:?}"
            );
            // Шапка внутри таблицы.
            let head = table.header[0];
            assert!(head[1] + head[3] <= table.rect[1] + table.rect[3] + 0.01);
            // Усечение честное: ячеек меньше строк, но добрый десяток.
            assert!(table.cells.len() < rows, "{viewport:?}");
            assert!(table.cells.len() >= 10, "{viewport:?}");
            // Последняя видимая ячейка целиком внутри таблицы (поле снизу).
            let last = table.cells[table.cells.len() - 1][0];
            assert!(last[1] + last[3] <= table.rect[1] + table.rect[3] - TABLE_MARGIN + 0.01);
            // Слот индикатора — под последней ячейкой, внутри таблицы.
            let tail = table.tail.expect("усечение даёт слот индикатора");
            assert!(tail[1] >= last[1] + last[3] - 0.01);
            assert!(tail[1] + tail[3] <= table.rect[1] + table.rect[3] - TABLE_MARGIN + 0.01);
            // Соседние видимые ячейки не налагаются (шаг TABLE_ROW_H).
            for pair in table.cells.windows(2) {
                assert!(pair[0][0][1] + pair[0][0][3] <= pair[1][0][1] + 0.01);
            }
        }
        // Всё вмещается — прежняя геометрия без изменений, слота нет.
        let bar = [500.0, 800.0, 600.0, BAR_HEIGHT];
        let roomy = table_layout(&columns, 4, bar, [1600.0, 900.0]);
        assert_eq!(roomy.cells.len(), 4);
        assert!(roomy.tail.is_none());
    }
}
