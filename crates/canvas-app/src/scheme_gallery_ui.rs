//! FR-049 (PRD-0008 T3): галерея шаблонов готовых схем + empty-state
//! пустого канваса.
//!
//! Модальная screen-space панель по паттернам репозитория (палитра
//! `template_ui`, модалка настроек FR-039): раскладка — чистые функции
//! от вьюпорта и списка строк; состояние — [`SchemeGalleryState`] без
//! I/O; отрисовка (квады/тексты из слотов `ThemeColors`, правило потока
//! PRD-0006 §7.1) — в `app.rs`. Клавиатура: ↑/↓ — выбор, Enter — открыть,
//! Esc — закрыть, печатаемый символ — фильтр. Инвариант 320×240: панель
//! клампится к вьюпорту, строки скроллятся окном видимости.
//!
//! Empty-state (US-1 PRD-0008): карточка по центру при пустом канвасе —
//! «Начните с шаблона» + «Пустой холст» (скрыть до следующего опустошения).

use canvas_core::schemes::{SchemeManifest, SchemeRegistry};

/// Константы раскладки галереи (логические px).
pub const PANEL_W: f32 = 560.0;
pub const HEADER_H: f32 = 40.0;
pub const INPUT_H: f32 = 34.0;
pub const CHIP_H: f32 = 28.0;
pub const ROW_H: f32 = 56.0;
pub const FOOTER_H: f32 = 26.0;
pub const PANEL_PAD: f32 = 12.0;

/// Состояние галереи схем (модальная; `None`-подобие — `open == false`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SchemeGalleryState {
    pub open: bool,
    /// Ординал выбранной строки среди отфильтрованных.
    pub selected: usize,
    /// Активная категория (`None` — «Все»).
    pub category: Option<String>,
    /// Фильтр по названию/описанию (без регистрозависимости).
    pub filter: String,
    /// Верх строки окна видимости (индекс в отфильтрованном списке).
    pub scroll_top: usize,
}

impl SchemeGalleryState {
    pub fn open(&mut self) {
        self.open = true;
        self.selected = 0;
        self.scroll_top = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.filter.clear();
        self.category = None;
        self.selected = 0;
        self.scroll_top = 0;
    }
}

/// Отфильтрованные строки: категория-чип + подстрока фильтра
/// (название RU/EN, описание RU/EN, категория).
pub fn rows<'a>(
    registry: &'a SchemeRegistry,
    state: &SchemeGalleryState,
) -> Vec<&'a SchemeManifest> {
    let filter = state.filter.to_lowercase();
    registry
        .list()
        .iter()
        .filter(|s| match &state.category {
            Some(cat) => s.category == *cat,
            None => true,
        })
        .filter(|s| {
            filter.is_empty()
                || s.name_ru.to_lowercase().contains(&filter)
                || s.name_en.to_lowercase().contains(&filter)
                || s.description_ru.to_lowercase().contains(&filter)
                || s.description_en.to_lowercase().contains(&filter)
                || s.category.to_lowercase().contains(&filter)
        })
        .collect()
}

/// Уникальные категории реестра в стабильном порядке (чипы «Все» + N).
pub fn categories(registry: &SchemeRegistry) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for scheme in registry.list() {
        if !out.iter().any(|(key, _, _)| *key == scheme.category) {
            out.push((
                scheme.category.clone(),
                scheme.category_ru.clone(),
                scheme.category_en.clone(),
            ));
        }
    }
    out
}

/// Сдвинуть окно видимости так, чтобы выбранная строка была видна.
pub fn clamp_scroll(state: &mut SchemeGalleryState, visible: usize) {
    if visible == 0 {
        state.scroll_top = 0;
        return;
    }
    if state.selected < state.scroll_top {
        state.scroll_top = state.selected;
    } else if state.selected >= state.scroll_top + visible {
        state.scroll_top = state.selected + 1 - visible;
    }
    // Кламп скролла к размеру списка — вызывающий передаёт rows.len().
}

/// Раскладка галереи: панель по центру, шапка, фильтр, чипы, строки.
#[derive(Debug, Clone, PartialEq)]
pub struct GalleryLayout {
    pub panel_rect: [f32; 4],
    pub header_rect: [f32; 4],
    pub close_rect: [f32; 4],
    pub input_rect: [f32; 4],
    /// Чипы: (rect, категория-ключ; `None` — «Все»).
    pub chip_rects: Vec<([f32; 4], Option<String>)>,
    /// Rect строк окна видимости (параллелен `visible_rows`).
    pub row_rects: Vec<[f32; 4]>,
    /// Индексы строк в общем отфильтрованном списке.
    pub visible_rows: Vec<usize>,
    pub footer_rect: [f32; 4],
}

/// Раскладка галереи (чистая функция; кламп к вьюпорту — инвариант
/// 320×240, строки скроллятся окном видимости).
pub fn layout(
    viewport: [f32; 2],
    list: &[&SchemeManifest],
    state: &SchemeGalleryState,
) -> GalleryLayout {
    let max_w = (viewport[0] - 24.0).max(280.0);
    let panel_w = PANEL_W.min(max_w);
    // Сколько строк влезает: высота панели — от вьюпорта.
    let chrome = HEADER_H + INPUT_H + CHIP_H + FOOTER_H + PANEL_PAD * 3.0;
    let max_h = (viewport[1] - 24.0).max(160.0);
    // Панель растёт под список, но не выше вьюпорта (строки скроллятся).
    let panel_h = max_h.min(chrome + ROW_H * list.len().max(1) as f32);
    let avail_rows_h = (panel_h - chrome).max(0.0);
    let visible = ((avail_rows_h / ROW_H).floor() as usize).max(1);
    let shown = visible.min(list.len().saturating_sub(state.scroll_top));

    let x = (viewport[0] - panel_w) / 2.0;
    let y = (viewport[1] - panel_h) / 2.0;
    let inner_x = x + PANEL_PAD;
    let inner_w = panel_w - PANEL_PAD * 2.0;

    let header_y = y + PANEL_PAD;
    let input_y = header_y + HEADER_H;
    let chips_y = input_y + INPUT_H + 6.0;
    let rows_y = chips_y + CHIP_H + 6.0;
    let footer_y = y + panel_h - FOOTER_H - PANEL_PAD;

    // Чипы: «Все» + категории (укладываемся в ширину, остаток — срез).
    let mut chip_rects = Vec::new();
    let mut cx = inner_x;
    let chip_gap = 6.0;
    let all_w = 56.0f32.min(inner_w);
    chip_rects.push(([inner_x, chips_y, all_w, CHIP_H], None));
    cx += all_w + chip_gap;
    for (key, _, _) in categories(SchemeRegistry::embedded()) {
        let w = 120.0;
        if cx + w > inner_x + inner_w {
            break;
        }
        chip_rects.push(([cx, chips_y, w, CHIP_H], Some(key)));
        cx += w + chip_gap;
    }

    let mut row_rects = Vec::new();
    let mut visible_rows = Vec::new();
    for i in 0..shown {
        let index = state.scroll_top + i;
        if index >= list.len() {
            break;
        }
        row_rects.push([inner_x, rows_y + i as f32 * ROW_H, inner_w, ROW_H - 6.0]);
        visible_rows.push(index);
    }

    GalleryLayout {
        panel_rect: [x, y, panel_w, panel_h],
        header_rect: [inner_x, header_y, inner_w - 32.0, HEADER_H],
        close_rect: [x + panel_w - PANEL_PAD - 24.0, header_y + 4.0, 24.0, 24.0],
        input_rect: [inner_x, input_y, inner_w, INPUT_H],
        chip_rects,
        row_rects,
        visible_rows,
        footer_rect: [inner_x, footer_y, inner_w, FOOTER_H],
    }
}

/// Hit-test строки галереи (индекс в отфильтрованном списке).
pub fn row_at(lay: &GalleryLayout, point: [f32; 2]) -> Option<usize> {
    for (rect, index) in lay.row_rects.iter().zip(lay.visible_rows.iter()) {
        if point_in_rect(*rect, point) {
            return Some(*index);
        }
    }
    None
}

/// Hit-test чипа категории.
pub fn chip_at(lay: &GalleryLayout, point: [f32; 2]) -> Option<Option<String>> {
    for (rect, category) in &lay.chip_rects {
        if point_in_rect(*rect, point) {
            return Some(category.clone());
        }
    }
    None
}

/// Rect карточки empty-state `[x, y, w, h]` (по центру вьюпорта).
pub fn empty_card_rect(viewport: [f32; 2]) -> [f32; 4] {
    let w = 380.0f32.min((viewport[0] - 24.0).max(240.0));
    let h = 190.0f32.min((viewport[1] - 24.0).max(150.0));
    [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h]
}

/// Кнопки empty-state: (rect «Открыть галерею», rect «Пустой холст»).
pub fn empty_buttons(card: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let btn_w = (card[2] - 3.0 * 10.0) / 2.0;
    let y = card[1] + card[3] - 44.0;
    (
        [card[0] + 10.0, y, btn_w, 34.0],
        [card[0] + 20.0 + btn_w, y, btn_w, 34.0],
    )
}

/// Точка в прямоугольнике `[x, y, w, h]` (общий хелпер модуля).
pub fn point_in_rect(rect: [f32; 4], point: [f32; 2]) -> bool {
    point[0] >= rect[0]
        && point[0] <= rect[0] + rect[2]
        && point[1] >= rect[1]
        && point[1] <= rect[1] + rect[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> SchemeGalleryState {
        SchemeGalleryState {
            open: true,
            ..Default::default()
        }
    }

    #[test]
    fn rows_list_all_and_filter() {
        let registry = SchemeRegistry::embedded();
        let st = state();
        assert_eq!(rows(registry, &st).len(), registry.list().len());
        let mut filtered = st.clone();
        filtered.filter = "смета".into();
        let r = rows(registry, &filtered);
        assert!(!r.is_empty(), "фильтр по русскому названию находит");
        assert!(r.iter().all(|s| s.category == "planning"));
        let mut cat = st.clone();
        cat.category = Some("onboarding".into());
        assert_eq!(rows(registry, &cat).len(), 2, "две онбординг-схемы");
    }

    #[test]
    fn categories_unique() {
        let registry = SchemeRegistry::embedded();
        let cats = categories(registry);
        assert!(cats.len() >= 3, "G2: ≥ 3 категории");
        let mut keys: Vec<&String> = cats.iter().map(|(k, _, _)| k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), cats.len());
    }

    #[test]
    fn layout_clamps_to_small_viewport() {
        let registry = SchemeRegistry::embedded();
        let st = state();
        let list = rows(registry, &st);
        // Инвариант 320×240: панель помещается, хотя бы одна строка видна.
        let lay = layout([320.0, 240.0], &list, &st);
        assert!(lay.panel_rect[2] <= 320.0);
        assert!(lay.panel_rect[3] <= 240.0);
        assert_eq!(lay.visible_rows.len(), 1, "одна строка в окне");
        // Стандартный вьюпорт — все схемы видимы без скролла.
        let lay = layout([1280.0, 800.0], &list, &st);
        assert_eq!(lay.visible_rows.len(), list.len());
    }

    #[test]
    fn clamp_scroll_follows_selection() {
        let mut st = state();
        st.selected = 5;
        clamp_scroll(&mut st, 3);
        assert_eq!(st.scroll_top, 3, "выбранная строка видна снизу");
        st.selected = 1;
        clamp_scroll(&mut st, 3);
        assert_eq!(st.scroll_top, 1, "выбранная строка видна сверху");
    }

    #[test]
    fn hit_tests_rows_chips_and_empty_buttons() {
        let registry = SchemeRegistry::embedded();
        let st = state();
        let list = rows(registry, &st);
        let lay = layout([1280.0, 800.0], &list, &st);
        let rect = lay.row_rects[0];
        assert_eq!(
            row_at(&lay, [rect[0] + 4.0, rect[1] + 4.0]),
            Some(0),
            "клик по первой строке"
        );
        assert_eq!(row_at(&lay, [0.0, 0.0]), None, "мимо строк");
        let (chip_rect, chip_cat) = lay.chip_rects[0].clone();
        assert_eq!(
            chip_at(&lay, [chip_rect[0] + 2.0, chip_rect[1] + 2.0]),
            Some(None),
            "чип «Все»"
        );
        assert!(chip_cat.is_none());
        // Empty-state: кнопки внутри карточки, не пересекаются.
        let card = empty_card_rect([1280.0, 800.0]);
        let (open_btn, dismiss_btn) = empty_buttons(card);
        assert!(point_in_rect(card, [open_btn[0] + 2.0, open_btn[1] + 2.0]));
        assert!(point_in_rect(
            card,
            [dismiss_btn[0] + 2.0, dismiss_btn[1] + 2.0]
        ));
        assert!(
            open_btn[0] + open_btn[2] <= dismiss_btn[0],
            "кнопки не пересекаются"
        );
    }
}
