//! FR-017 (CP6, волна B2): what-if нижний бар — чистая модель
//! (образец [`crate::settings_ui`]/[`crate::hints_ui`]): геометрия полосы
//! режима и пилюли входа, hit-тесты действий, раскрытый список подмен
//! со снятием отдельной подмены и таблица сравнения сценариев.
//!
//! Рендер и ввод — приложение (`main.rs`): бар собирается по кадру из
//! квадов + screen-текстов, клики перехватываются до канваса, побочные
//! эффекты (переключение сценария, Apply/Reset) — на стороне `App`.

use crate::ui::point_in_rect;

/// Отступ бара от нижнего края окна (логические px).
pub const BAR_MARGIN: f32 = 12.0;
/// Внутренние поля полосы.
pub const BAR_PADDING: f32 = 10.0;
/// Высота полосы режима.
pub const BAR_HEIGHT: f32 = 44.0;
/// Высота пилюли входа (вне режима).
pub const PILL_HEIGHT: f32 = 30.0;
/// Ширина пилюли входа.
pub const PILL_WIDTH: f32 = 104.0;
/// Высота чипа сценария.
pub const CHIP_HEIGHT: f32 = 26.0;
/// Горизонтальные поля чипа.
pub const CHIP_PAD_X: f32 = 12.0;
/// Запас ширины чипа поверх оценки текста (рендер-метрики Noto шире
/// эвристики `text_width` — без запаса подпись переливается на соседа,
/// см. CR-015).
pub const CHIP_SLACK: f32 = 4.0;
/// Зазор между элементами бара.
pub const BAR_GAP: f32 = 6.0;
/// Ширина индикатора «WHAT-IF».
pub const INDICATOR_WIDTH: f32 = 78.0;
/// Минимальная ширина кнопок Apply/Сброс/Сравнить (фактическая — по
/// подписи, `btn_width`).
pub const BTN_WIDTH: f32 = 74.0;
/// Горизонтальные поля кнопки.
pub const BTN_PAD_X: f32 = 12.0;
/// Максимальная длина подписи чипа сценария (символов) до «…».
pub const CHIP_LABEL_MAX: usize = 24;
/// Ширина кнопки «✕».
pub const CLOSE_WIDTH: f32 = 28.0;
/// Высота строки раскрытого списка подмен.
pub const LIST_ROW_H: f32 = 24.0;
/// Поля раскрытого списка.
pub const LIST_MARGIN: f32 = 6.0;
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
/// Поля таблицы сравнения.
pub const TABLE_MARGIN: f32 = 8.0;

/// Грубая оценка ширины текста (средний глиф ≈ 0.62 кегля — синк с
/// `docs_ui::CHAR_W_FACTOR` для Noto Sans Display) — для раскладки чипов
/// сценариев; точность не критична (сверху есть `CHIP_SLACK`/поля кнопок,
/// см. CR-015).
pub fn text_width(text: &str, font_size: f32) -> f32 {
    text.chars().count() as f32 * font_size * 0.62
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
    /// Сброс подмен активного сценария.
    Reset,
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
    /// Кнопка «Сравнить».
    pub compare: [f32; 4],
    /// Кнопка «✕».
    pub close: [f32; 4],
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

/// Подпись чипа сценария с капом длины: длинные имена схлопываются «…»
/// (единообразно в раскладке и отрисовке — `app.rs` рисует эту же строку).
pub fn chip_label(name: &str) -> String {
    let mut chars: Vec<char> = name.chars().collect();
    if chars.len() > CHIP_LABEL_MAX {
        chars.truncate(CHIP_LABEL_MAX);
        chars.push('…');
    }
    chars.into_iter().collect()
}

/// Ширина чипа по подписи (с капом `chip_label`).
fn chip_width(label: &str) -> f32 {
    text_width(&chip_label(label), 13.0) + CHIP_PAD_X * 2.0 + CHIP_SLACK
}

/// Ширина кнопки по подписи: не уже `BTN_WIDTH`, поля `BTN_PAD_X`
/// (CR-015: «Сравнить» шире прежнего фикса 74 px и обрезалась).
fn btn_width(label: &str) -> f32 {
    BTN_WIDTH.max(text_width(label, 13.0) + BTN_PAD_X * 2.0)
}

/// Геометрия полосы режима. `scenario_names` — имена пользовательских
/// сценариев; `override_count` — число подмен активного сценария (для
/// подписи счётчика). Полоса центрируется по низу окна; ширина — сумма
/// элементов, клампится к окну.
pub fn bar_layout(
    scenario_names: &[String],
    override_count: usize,
    viewport: [f32; 2],
) -> BarLayout {
    let counter_label = format!("подмен: {override_count}");
    let counter_w = chip_width(&counter_label);
    // Ширина: паддинги + индикатор + База + чипы + «+» + счётчик + 3
    // кнопки + ✕ + зазоры (элементов scenarios.len() + 7 — зазоров на 1
    // меньше, но запас не вредит; считаем точно)
    let chips_w: f32 = scenario_names
        .iter()
        .map(|name| chip_width(name))
        .sum::<f32>()
        + chip_width("База")
        + chip_width("+");
    let controls_w = INDICATOR_WIDTH
        + counter_w
        + CLOSE_WIDTH
        + btn_width("Apply")
        + btn_width("Сброс")
        + btn_width("Сравнить");
    let elements = scenario_names.len() as f32 + 7.0; // чипы+База+«+»+инд+счёт+3кн+✕
    let width = (BAR_PADDING * 2.0 + chips_w + controls_w + BAR_GAP * (elements - 1.0))
        .min((viewport[0] - BAR_MARGIN * 2.0).max(0.0));
    let rect = [
        (viewport[0] - width) / 2.0,
        viewport[1] - BAR_MARGIN - BAR_HEIGHT,
        width,
        BAR_HEIGHT,
    ];
    let cy = rect[1] + (BAR_HEIGHT - CHIP_HEIGHT) / 2.0;
    // CR-015: элементы не уходят за правый край полосы (узкое окно) —
    // при нехватке ширины хвост ужимается до нуля, а не рисуется мимо бара.
    let right_limit = rect[0] + rect[2] - BAR_PADDING;
    let mut x = rect[0] + BAR_PADDING;
    let mut take = |w: f32| {
        let w = w.min((right_limit - x).max(0.0));
        let rect = [x.min(right_limit), cy, w, CHIP_HEIGHT];
        x += w + BAR_GAP;
        rect
    };
    let indicator = take(INDICATOR_WIDTH);
    let base = take(chip_width("База"));
    let scenarios = scenario_names
        .iter()
        .map(|name| take(chip_width(name)))
        .collect();
    let new_scenario = take(chip_width("+"));
    let overrides = take(counter_w);
    let apply = take(btn_width("Apply"));
    let reset = take(btn_width("Сброс"));
    let compare = take(btn_width("Сравнить"));
    let close = take(CLOSE_WIDTH);
    BarLayout {
        rect,
        indicator,
        base,
        scenarios,
        new_scenario,
        overrides,
        apply,
        reset,
        compare,
        close,
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
        let layout = bar_layout(&names(), 3, viewport);
        assert!(layout.rect[1] + layout.rect[3] <= viewport[1] - BAR_MARGIN + 0.01);
        // Порядок слева направо: индикатор < База < С1 < С2 < «+» < счётчик
        // < Apply < Сброс < Сравнить < ✕
        let xs = |r: [f32; 4]| r[0];
        assert!(xs(layout.indicator) < xs(layout.base));
        assert!(xs(layout.base) < xs(layout.scenarios[0]));
        assert!(xs(layout.scenarios[0]) < xs(layout.scenarios[1]));
        assert!(xs(layout.scenarios[1]) < xs(layout.new_scenario));
        assert!(xs(layout.new_scenario) < xs(layout.overrides));
        assert!(xs(layout.overrides) < xs(layout.apply));
        assert!(xs(layout.apply) < xs(layout.reset));
        assert!(xs(layout.reset) < xs(layout.compare));
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
        let layout = bar_layout(&names(), 1, [360.0, 240.0]);
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

    /// Оценка ширины текста монотонна и положительна.
    #[test]
    fn text_width_monotonic() {
        assert!(text_width("Рост", 13.0) > 0.0);
        assert!(text_width("Рост ×2", 13.0) > text_width("Рост", 13.0));
        assert!(text_width("", 13.0) == 0.0);
    }

    /// CR-015: типичный набор бара (скриншот пользователя) — соседние
    /// элементы не пересекаются, каждый чип/кнопка не уже своей подписи
    /// (текст не переливается на соседа).
    #[test]
    fn bar_layout_no_overlap_and_covers_labels() {
        let names: Vec<String> = ["Сценарий 3", "Сценарий 2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let layout = bar_layout(&names, 0, [1600.0, 900.0]);
        let rects = [
            layout.indicator,
            layout.base,
            layout.scenarios[0],
            layout.scenarios[1],
            layout.new_scenario,
            layout.overrides,
            layout.apply,
            layout.reset,
            layout.compare,
            layout.close,
        ];
        for w in rects.windows(2) {
            assert!(
                w[0][0] + w[0][2] <= w[1][0] + 0.01,
                "соседние элементы пересекаются"
            );
        }
        let chip_cover = |rect: [f32; 4], label: &str| {
            assert!(
                rect[2] >= text_width(&chip_label(label), 13.0) + CHIP_PAD_X * 2.0 - 0.01,
                "чип «{label}» уже подписи"
            );
        };
        chip_cover(layout.base, "База");
        chip_cover(layout.scenarios[0], "Сценарий 3");
        chip_cover(layout.scenarios[1], "Сценарий 2");
        chip_cover(layout.overrides, "подмен: 0");
        let btn_cover = |rect: [f32; 4], label: &str| {
            assert!(
                rect[2] >= text_width(label, 13.0) + BTN_PAD_X * 2.0 - 0.01,
                "кнопка «{label}» уже подписи"
            );
        };
        btn_cover(layout.apply, "Apply");
        btn_cover(layout.reset, "Сброс");
        btn_cover(layout.compare, "Сравнить");
    }

    /// CR-015: ширина кнопки — не уже минимума и покрывает подпись.
    #[test]
    fn btn_width_covers_labels() {
        assert!(btn_width("Apply") >= BTN_WIDTH);
        assert!(btn_width("Сравнить") >= BTN_WIDTH);
        assert!(btn_width("Сравнить") >= text_width("Сравнить", 13.0) + BTN_PAD_X * 2.0 - 0.01);
    }

    /// CR-015: длинные имена сценариев капаются «…» (раскладка и отрисовка
    /// используют одну строку — `chip_label`).
    #[test]
    fn chip_label_caps_long_names() {
        let long = "Очень длинное имя сценария с деталями эксперимента";
        let capped = chip_label(long);
        assert!(capped.chars().count() <= CHIP_LABEL_MAX + 1);
        assert!(capped.ends_with('…'));
        assert_eq!(chip_label("Сценарий 2"), "Сценарий 2");
    }

    /// CR-015: узкое окно — все элементы внутри rect бара, правый край
    /// ничего не уходит за полосу (хвост ужимается, а не рисуется мимо).
    #[test]
    fn bar_layout_narrow_window_keeps_elements_inside() {
        let layout = bar_layout(&names(), 1, [400.0, 240.0]);
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
}
