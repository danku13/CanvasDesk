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

use crate::ui::point_in_rect;
use canvas_ui::geometry::{EdgeInsets, UiRect, UiVec2};
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
/// Высота чипа сценария.
pub const CHIP_HEIGHT: f32 = 26.0;
/// Горизонтальные поля чипа (spacing-scale).
pub const CHIP_PAD_X: f32 = canvas_core::tokens::SPACING_LG;
/// Зазор между элементами бара (spacing-scale).
pub const BAR_GAP: f32 = canvas_core::tokens::SPACING_S;
/// Ширина индикатора «WHAT-IF».
pub const INDICATOR_WIDTH: f32 = 78.0;
/// Минимальная ширина кнопок Apply/Сброс/Сравнить (фактическая — по
/// измеренной подписи, [`btn_width`]).
pub const BTN_WIDTH: f32 = 74.0;
/// Горизонтальные поля кнопки (spacing-scale).
pub const BTN_PAD_X: f32 = canvas_core::tokens::SPACING_LG;
/// Кегль подписей чипов/кнопок бара (логические px; метка раскладки =
/// метка отрисовки — единый источник размера).
pub const CHIP_FONT: f32 = 13.0;
/// Ширина кнопки «✕».
pub const CLOSE_WIDTH: f32 = 28.0;
/// Высота строки раскрытого списка подмен.
pub const LIST_ROW_H: f32 = 24.0;
/// Поля раскрытого списка (spacing-scale).
pub const LIST_MARGIN: f32 = canvas_core::tokens::SPACING_S;
/// Ширина раскрытого списка.
pub const LIST_WIDTH: f32 = 480.0;
/// Ширина кнопки «✕» у строки подмены.
pub const REMOVE_WIDTH: f32 = 22.0;
/// Высота строки таблицы сравнения.
pub const TABLE_ROW_H: f32 = 24.0;
/// Высота шапки таблицы сравнения.
pub const TABLE_HEAD_H: f32 = 26.0;
/// Ширина колонки таблицы сравнения.
pub const TABLE_COL_W: f32 = 190.0;
/// Поля таблицы сравнения (spacing-scale).
pub const TABLE_MARGIN: f32 = canvas_core::tokens::SPACING_SM;

/// Семейство измерения = семейство screen-текстов рендера (parity метрик).
const FAMILY: &str = canvas_render::text::SANS_FAMILY;

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
    /// Сброс подмен активного сценария.
    Reset,
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
    /// Счётчик подмен.
    pub overrides: [f32; 4],
    /// Кнопка «Apply».
    pub apply: [f32; 4],
    /// Кнопка «Сброс».
    pub reset: [f32; 4],
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
fn chip_width(label: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
    m.width_of(fs, label, FAMILY, CHIP_FONT) + CHIP_PAD_X * 2.0
}

/// Ширина кнопки по подписи: не уже `BTN_WIDTH`, поля `BTN_PAD_X`
/// (CR-015: «Сравнить» шире прежнего фикса 74 px и обрезалась).
fn btn_width(label: &str, m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem) -> f32 {
    BTN_WIDTH.max(m.width_of(fs, label, FAMILY, CHIP_FONT) + BTN_PAD_X * 2.0)
}

/// Геометрия полосы режима. `scenario_names` — имена пользовательских
/// сценариев; `counter_label` — подпись счётчика подмен (i18n-строка
/// приложения — ширина считается по ТОЙ ЖЕ строке, что рисуется: фикс
/// FR-053, раньше ширина считалась по RU при EN-подписи). Полоса
/// центрируется по низу окна; ширина — сумма измеренных элементов,
/// сжатая [`constrain`] к вьюпорту.
pub fn bar_layout(
    scenario_names: &[String],
    counter_label: &str,
    freeze_label: &str,
    viewport: [f32; 2],
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> BarLayout {
    let counter_w = chip_width(counter_label, measurer, fs);
    let chips_w: f32 = scenario_names
        .iter()
        .map(|name| chip_width(name, measurer, fs))
        .sum::<f32>()
        + chip_width("База", measurer, fs)
        + chip_width("+", measurer, fs);
    let controls_w = INDICATOR_WIDTH
        + counter_w
        + CLOSE_WIDTH
        + btn_width("Apply", measurer, fs)
        + btn_width("Сброс", measurer, fs)
        + btn_width(freeze_label, measurer, fs)
        + btn_width("Сравнить", measurer, fs);
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
    // FR-068 W3.1 (staged-миграция потребителей, каталог
    // docs/plans/fr-068-w3-consumer-migration.md, топ-1): элементы бара
    // переведены с ручной проводки `Child::fixed(width_of…)` на семейство
    // measured-API ([`MeasuredItem`] через [`Row::lay_out_measured`]).
    // Чип/кнопка = текст + пад (`CHIP_PAD_X`/`BTN_PAD_X`) — точную ширину
    // даёт [`MeasuredItem::Fixed`] (замер ОДИН раз выше, строки те же);
    // авто-размер [`MeasuredItem::Text`] — после появления пад-семантики
    // в F-13 (без изменения ширины чипов — отдельное решение владельца).
    // Геометрия бит-в-бит с прежней: `MeasuredItem::Fixed` резолвится в
    // тот же `Child::fixed` и тот же движок SqueezeTail (оракул F-13).
    let mut items: Vec<MeasuredItem> = Vec::with_capacity(scenario_names.len() + 8);
    items.push(MeasuredItem::Fixed {
        w: INDICATOR_WIDTH,
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: chip_width("База", measurer, fs),
        h: CHIP_HEIGHT,
    });
    for name in scenario_names {
        items.push(MeasuredItem::Fixed {
            w: chip_width(name, measurer, fs),
            h: CHIP_HEIGHT,
        });
    }
    items.push(MeasuredItem::Fixed {
        w: chip_width("+", measurer, fs),
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: counter_w,
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: btn_width("Apply", measurer, fs),
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: btn_width("Сброс", measurer, fs),
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: btn_width(freeze_label, measurer, fs),
        h: CHIP_HEIGHT,
    });
    items.push(MeasuredItem::Fixed {
        w: btn_width("Сравнить", measurer, fs),
        h: CHIP_HEIGHT,
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
    // Подписи сценариев — Ellipsis по фактической (возможно сжатой)
    // ширине чипа минус поля: раскладка и отрисовка — одна строка.
    let scenario_labels = scenario_names
        .iter()
        .zip(rects[2..2 + n].iter())
        .map(|(name, rect)| {
            let inner = (rect.w - CHIP_PAD_X * 2.0).max(0.0);
            measurer.ellipsis(fs, name, FAMILY, CHIP_FONT, inner)
        })
        .collect();
    BarLayout {
        rect: [bar.x, bar.y, bar.w, bar.h],
        indicator: as_rect(&rects[0]),
        base: as_rect(&rects[1]),
        scenarios: rects[2..2 + n].iter().map(as_rect).collect(),
        new_scenario: as_rect(&rects[2 + n]),
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
pub fn bar_action_at(layout: &BarLayout, point: [f32; 2]) -> Option<BarAction> {
    if point_in_rect(layout.close, point) {
        return Some(BarAction::Close);
    }
    if point_in_rect(layout.apply, point) {
        return Some(BarAction::Apply);
    }
    if point_in_rect(layout.reset, point) {
        return Some(BarAction::Reset);
    }
    if point_in_rect(layout.compare, point) {
        return Some(BarAction::Compare);
    }
    if point_in_rect(layout.freeze, point) {
        return Some(BarAction::Freeze);
    }
    if point_in_rect(layout.overrides, point) {
        return Some(BarAction::ToggleOverrides);
    }
    if point_in_rect(layout.new_scenario, point) {
        return Some(BarAction::NewScenario);
    }
    for (i, rect) in layout.scenarios.iter().enumerate() {
        if point_in_rect(*rect, point) {
            return Some(BarAction::Scenario(i));
        }
    }
    if point_in_rect(layout.base, point) {
        return Some(BarAction::Base);
    }
    None
}

/// Rect раскрытого списка подмен: над полосой, по её центру, кламп к окну.
pub fn overrides_list_layout(bar_rect: [f32; 4], count: usize, viewport: [f32; 2]) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    let height = count as f32 * LIST_ROW_H + LIST_MARGIN * 2.0;
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
pub fn override_row_at(list: [f32; 4], count: usize, point: [f32; 2]) -> Option<usize> {
    if !point_in_rect(list, point) {
        return None;
    }
    let rel = point[1] - list[1] - LIST_MARGIN;
    if rel < 0.0 {
        return None;
    }
    let row = (rel / LIST_ROW_H) as usize;
    (row < count).then_some(row)
}

/// Геометрия таблицы сравнения: rect + rect'ы шапки и ячеек.
#[derive(Debug, Clone, PartialEq)]
pub struct TableLayout {
    /// Rect таблицы `[x, y, w, h]`.
    pub rect: [f32; 4],
    /// Rect'ы колонок шапки (подписи «переменная | База | С1 | С2»).
    pub header: Vec<[f32; 4]>,
    /// Ячейки `[row][col]`.
    pub cells: Vec<Vec<[f32; 4]>>,
}

/// Геометрия таблицы сравнения: над баром по центру, кламп к окну.
/// `columns` — подписи колонок (первая — «переменная»), `rows` — число
/// строк сравнения. Таблица в v1 read-only — hit-тесты не нужны.
pub fn table_layout(
    columns: &[String],
    rows: usize,
    bar_rect: [f32; 4],
    viewport: [f32; 2],
) -> TableLayout {
    let cols = columns.len().max(1);
    let width = (TABLE_COL_W * cols as f32 + TABLE_MARGIN * 2.0)
        .min((viewport[0] - BAR_MARGIN * 2.0).max(0.0));
    let height = TABLE_HEAD_H + rows as f32 * TABLE_ROW_H + TABLE_MARGIN * 2.0;
    let x = (bar_rect[0] + bar_rect[2] / 2.0 - width / 2.0).clamp(
        BAR_MARGIN,
        (viewport[0] - width - BAR_MARGIN).max(BAR_MARGIN),
    );
    let y = (bar_rect[1] - height - 4.0).max(BAR_MARGIN);
    let header: Vec<[f32; 4]> = (0..cols)
        .map(|c| {
            [
                x + TABLE_MARGIN + c as f32 * TABLE_COL_W,
                y + TABLE_MARGIN,
                TABLE_COL_W,
                TABLE_HEAD_H,
            ]
        })
        .collect();
    let cells = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| {
                    [
                        x + TABLE_MARGIN + c as f32 * TABLE_COL_W,
                        y + TABLE_MARGIN + TABLE_HEAD_H + r as f32 * TABLE_ROW_H,
                        TABLE_COL_W,
                        TABLE_ROW_H,
                    ]
                })
                .collect()
        })
        .collect();
    TableLayout {
        rect: [x, y, width, height],
        header,
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_ui::geometry::UiRect;

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
        bar_layout(names, counter, "❄ Заморозить", viewport, &mut m, &mut fs)
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
        // Метка сценария из раскладки помещается в свой чип.
        for (rect, label) in lay.scenarios.iter().zip(lay.scenario_labels.iter()) {
            let label_w = m.width_of(&mut fs, label, FAMILY, CHIP_FONT);
            assert!(
                label_w <= rect[2] - CHIP_PAD_X * 2.0 + 0.01,
                "метка «{label}» ({label_w}) шире чипа"
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
            label_w <= narrow.scenarios[0][2] - CHIP_PAD_X * 2.0 + 0.01,
            "усечённая подпись укладывается в сжатый чип"
        );
        // Короткое имя не усекается.
        let short_names = vec!["С1".to_owned()];
        let short = layout(&short_names, "подмен: 0", [1600.0, 900.0]);
        assert_eq!(short.scenario_labels[0], "С1");
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
}
