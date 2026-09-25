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
//!
//! FR-053 (U3 PRD-0009, пилот G4/G5): раскладка собрана примитивами
//! `canvas-ui::layout` (панель — [`stack`]/[`constrain`], вертикальный
//! ритм — [`Column`], чипы — [`Row`] с политикой `Fit` — бывший
//! `break`-кламп удалён: переполнение стало тестируемым, молчаливый срез
//! невозможен), поля — spacing-scale `canvas_core::tokens::SPACING_*`;
//! подписи строк — измеренный Ellipsis ([`row_labels`]: заголовок/описание
//! усекаются по фактической ширине строки — текст больше не переливается
//! на соседнюю строку, screen-тексты рендера не переносятся).

use canvas_core::schemes::{SchemeManifest, SchemeRegistry};
use canvas_ui::geometry::{EdgeInsets, UiRect, UiVec2};
use canvas_ui::layout::{
    constrain, pad, stack, Column, CrossAlign, HAlign, MeasuredItem, Row, VAlign,
};
use canvas_ui::measure::TextMeasurer;

/// Семейство измерения = семейство screen-текстов рендера (parity метрик).
const FAMILY: &str = canvas_render::text::SANS_FAMILY;

/// Ширина панели галереи (логические px).
pub const PANEL_W: f32 = 560.0;
/// Высота шапки.
pub const HEADER_H: f32 = 40.0;
/// Высота поля фильтра.
pub const INPUT_H: f32 = 34.0;
/// Высота чипа категории.
pub const CHIP_H: f32 = 28.0;
/// Полный шаг строки списка (строка + зазор).
pub const ROW_H: f32 = 62.0;
/// Высота видимой части строки (шаг минус зазор `SPACING_S`).
pub const ROW_INNER_H: f32 = ROW_H - canvas_core::tokens::SPACING_S;
/// Высота футера.
pub const FOOTER_H: f32 = 26.0;
/// Внутренние поля панели (spacing-scale).
pub const PANEL_PAD: f32 = canvas_core::tokens::SPACING_LG;
/// Ширина чипа категории (фикс — D2 CJM: полный ряд «Все» + 4 категории
/// при PANEL_W 560; design-константа, не эвристика).
pub const CHIP_W: f32 = 108.0;
/// Ширина чипа «Все».
pub const CHIP_ALL_W: f32 = 56.0;
/// Кегль заголовка строки.
pub const ROW_FONT: f32 = 13.0;
/// Кегль описания строки.
pub const ROW_DESC_FONT: f32 = 11.0;
/// Поле текста внутри строки (spacing-scale).
pub const ROW_TEXT_PAD: f32 = canvas_core::tokens::SPACING_MD;
/// Высота кнопки empty-state.
pub const EMPTY_BTN_H: f32 = 34.0;

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
/// 320×240, строки скроллятся окном видимости). FR-053: собрана
/// примитивами `canvas-ui::layout` — вертикальный ритм дословно прежний
/// (зазоры header→input 0, input→chips `SPACING_S`, chips→rows
/// `SPACING_S`, строки примыкают к футеру), позиция панели/размер —
/// Stack/Constrain. W3.2 (каталог `docs/plans/fr-068-w3-consumer-migration.md`):
/// дети скелета/чипов/строк выражаются [`MeasuredItem`] —
/// `Row/Column::lay_out_measured` без ручных фиксированных детей.
pub fn layout(
    viewport: [f32; 2],
    list: &[&SchemeManifest],
    state: &SchemeGalleryState,
) -> GalleryLayout {
    // W3.2: замерщик — канонические shared-точки на вызов (Text-детей
    // нет — замерщик геометрию не читает).
    let mut m = TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    layout_with(viewport, list, state, &mut m, &mut fs)
}

/// То же с ЯВНЫМ замерщиком (для потребителей, уже держащих
/// `measure_font_system` — двойной лок глобального FontSystem невозможен).
pub fn layout_with(
    viewport: [f32; 2],
    list: &[&SchemeManifest],
    state: &SchemeGalleryState,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> GalleryLayout {
    let max_w = (viewport[0] - canvas_core::tokens::SPACING_XL).max(280.0);
    let panel_w = PANEL_W.min(max_w);
    // Сколько строк влезает: высота панели — от вьюпорта. Маржа XL
    // учитывается С ОБЕИХ сторон (панель центрируется по вертикали:
    // иначе при ROW_H 62 панель 536 в 560-окне давала y=12 < маржи —
    // поймано G4-линтом после wasm-аудита 2026-09-25).
    let chrome = HEADER_H + INPUT_H + CHIP_H + FOOTER_H + PANEL_PAD * 3.0;
    let max_h = (viewport[1] - canvas_core::tokens::SPACING_XL * 2.0).max(160.0);
    // Панель растёт под список, но не выше вьюпорта (строки скроллятся).
    let panel_h = max_h.min(chrome + ROW_H * list.len().max(1) as f32);
    let avail_rows_h = (panel_h - chrome).max(0.0);
    let visible = ((avail_rows_h / ROW_H).floor() as usize).max(1);
    let shown = visible.min(list.len().saturating_sub(state.scroll_top));

    // Панель — Stack по центру вьюпорта (позиция = (vw−w)/2, (vh−h)/2).
    let panel = stack(
        UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
        UiVec2::new(panel_w, panel_h),
        HAlign::Center,
        VAlign::Center,
    );
    let inner = pad(panel, EdgeInsets::uniform(PANEL_PAD));
    let inner_w = panel_w - PANEL_PAD * 2.0;

    // Вертикальный ритм панели: Column без базового зазора + явные
    // распорки `SPACING_S` там, где прежняя геометрия имела зазор
    // (header→input 0, input→chips 6, chips→rows 6, rows→footer 0 —
    // ноль визуального скачка). W3.2: дети — MeasuredItem (Fixed/Spacer);
    // Семантика Spacer в колонке — нулевая высота (main-ось — высота;
    // см. оракул measured_column_matches_manual_fixed_oracle) — бит-в-бит
    // с прежней проводкой.
    let items = vec![
        // Шапка: место под заголовок (кнопка «×» — в правом крае панели).
        MeasuredItem::Fixed {
            w: inner_w - 32.0,
            h: HEADER_H,
        },
        MeasuredItem::Fixed {
            w: inner_w,
            h: INPUT_H,
        },
        MeasuredItem::Spacer(canvas_core::tokens::SPACING_S),
        MeasuredItem::Fixed {
            w: inner_w,
            h: CHIP_H,
        },
        MeasuredItem::Spacer(canvas_core::tokens::SPACING_S),
        MeasuredItem::Fixed {
            w: inner_w,
            h: avail_rows_h,
        },
        MeasuredItem::Fixed {
            w: inner_w,
            h: FOOTER_H,
        },
    ];
    let col = Column {
        gap: 0.0,
        cross: CrossAlign::Start,
        ..Column::default()
    }
    .lay_out_measured(inner, &items, m, fs, FAMILY, 12.0);
    let as_rect = |r: &UiRect| [r.x, r.y, r.w, r.h];
    let header_rect = as_rect(&col[0]);
    let input_rect = as_rect(&col[1]);
    let chips_slot = col[3];
    let rows_slot = col[5];
    let footer_rect = as_rect(&col[6]);

    // Кнопка «×»: правый край шапки, офсет +4 (дизайн-центровка в 40 px
    // шапке; Stack на под-слоте шапки).
    let close = stack(
        UiRect::new(inner.x, inner.y + 4.0, inner_w, 24.0),
        UiVec2::new(24.0, 24.0),
        HAlign::End,
        VAlign::Start,
    );

    // Чипы: «Все» + категории — Row с политикой Fit (все элементы
    // раскладываются; переполнение слота НЕ маскируется — ловится
    // тестом `chips_all_categories_fit`/G4-линтом; прежний молчаливый
    // `break`-кламп удалён). Ширина «Все» всегда влезает: панель ≥ 280,
    // слот чипов ≥ 256.
    let mut chip_children = vec![MeasuredItem::Fixed {
        w: CHIP_ALL_W,
        h: CHIP_H,
    }];
    let mut chip_keys: Vec<Option<String>> = vec![None];
    for (key, _, _) in categories(SchemeRegistry::embedded()) {
        chip_children.push(MeasuredItem::Fixed {
            w: CHIP_W,
            h: CHIP_H,
        });
        chip_keys.push(Some(key));
    }
    let chip_layout = Row {
        gap: canvas_core::tokens::SPACING_S,
        cross: CrossAlign::Start,
        ..Row::default()
    }
    .lay_out_measured(chips_slot, &chip_children, m, fs, FAMILY, 12.0);
    let chip_rects: Vec<([f32; 4], Option<String>)> = chip_layout
        .iter()
        .zip(chip_keys)
        .map(|(r, key)| (as_rect(r), key))
        .collect();

    // Строки окна видимости: Column с зазором `SPACING_S`, видимая часть
    // строки `ROW_INNER_H` (полный шаг ROW_H — дословно прежний ритм).
    let row_children: Vec<MeasuredItem> = (0..shown)
        .map(|_| MeasuredItem::Fixed {
            w: inner_w,
            h: ROW_INNER_H,
        })
        .collect();
    let row_layout = Column {
        gap: canvas_core::tokens::SPACING_S,
        cross: CrossAlign::Start,
        ..Column::default()
    }
    .lay_out_measured(rows_slot, &row_children, m, fs, FAMILY, 12.0);
    let mut row_rects = Vec::with_capacity(shown);
    let mut visible_rows = Vec::with_capacity(shown);
    // Инвариант: `shown <= list.len() - scroll_top` (кламп выше), поэтому
    // scroll_top + i < list.len() для всех i < shown — клампов в цикле нет.
    for (i, r) in row_layout.iter().enumerate() {
        let index = state.scroll_top + i;
        row_rects.push(as_rect(r));
        visible_rows.push(index);
    }

    GalleryLayout {
        panel_rect: [panel.x, panel.y, panel.w, panel.h],
        header_rect,
        close_rect: [close.x, close.y, close.w, close.h],
        input_rect,
        chip_rects,
        row_rects,
        visible_rows,
        footer_rect,
    }
}

/// Подписи строк окна видимости (FR-053): заголовок и описание,
/// усечённые Ellipsis-политикой по фактической ширине строки минус
/// поля `ROW_TEXT_PAD` (screen-тексты рендера не переносятся — без
/// усечения длинное описание переливалось на соседнюю строку).
#[derive(Debug, Clone, PartialEq)]
pub struct RowLabel {
    pub title: String,
    pub desc: String,
}

/// Измеренные подписи строк (параллелен `GalleryLayout.row_rects`).
///
/// Контракт W3.2: вызывающий держит guard `measure_font_system` (передаёт
/// `fs` явно) — внутренняя раскладка обязана брать `layout_with`, а НЕ
/// лочащую `layout`. Прецедент бага 2026-09-26: вызов `layout` здесь давал
/// второй лок того же глобального FontSystem — wasm падал паникой
/// «cannot recursively acquire mutex» (std no_threads Mutex), натив —
/// дедлоком кадра RedrawRequested (галерея схем открыта → кадр стоит
/// навсегда). `layout_with` даёт бит-в-бит ту же геометрию (то же тело
/// функции), замерщик переиспользуется.
pub fn row_labels(
    viewport: [f32; 2],
    list: &[&SchemeManifest],
    state: &SchemeGalleryState,
    ru: bool,
    measurer: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<RowLabel> {
    let lay = layout_with(viewport, list, state, measurer, fs);
    let max_w = (lay.row_rects.first().map(|r| r[2]).unwrap_or(0.0) - ROW_TEXT_PAD * 2.0).max(0.0);
    lay.visible_rows
        .iter()
        .map(|&index| {
            let Some(scheme) = list.get(index) else {
                return RowLabel {
                    title: String::new(),
                    desc: String::new(),
                };
            };
            RowLabel {
                title: measurer.ellipsis(fs, scheme.display_name(ru), FAMILY, ROW_FONT, max_w),
                desc: measurer.ellipsis(
                    fs,
                    if ru {
                        &scheme.description_ru
                    } else {
                        &scheme.description_en
                    },
                    FAMILY,
                    ROW_DESC_FONT,
                    max_w,
                ),
            }
        })
        .collect()
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
/// FR-053: размер — Constrain (desired 380×190, min 240×150, max —
/// вьюпорт минус маржа `SPACING_XL`), позиция — Stack по центру.
pub fn empty_card_rect(viewport: [f32; 2]) -> [f32; 4] {
    let size = constrain(
        UiVec2::new(240.0, 150.0),
        UiVec2::new(
            (viewport[0] - canvas_core::tokens::SPACING_XL).max(240.0),
            (viewport[1] - canvas_core::tokens::SPACING_XL).max(150.0),
        ),
        UiVec2::new(380.0, 190.0),
    );
    let card = stack(
        UiRect::new(0.0, 0.0, viewport[0], viewport[1]),
        size,
        HAlign::Center,
        VAlign::Center,
    );
    [card.x, card.y, card.w, card.h]
}

/// Кнопки empty-state: (rect «Открыть галерею», rect «Пустой холст»).
/// FR-053: слот кнопок — поля `SPACING_MD` к бокам и низу карточки,
/// прижим к низу (cross End), ширина каждой — половина слота минус
/// зазор `SPACING_MD` (дословно прежняя геометрия).
pub fn empty_buttons(card: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    // W3.2: замерщик — канонические shared-точки на вызов.
    let mut m = TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    empty_buttons_with(card, &mut m, &mut fs)
}

/// То же с ЯВНЫМ замерщиком (см. [`empty_buttons`]).
pub fn empty_buttons_with(
    card: [f32; 4],
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> ([f32; 4], [f32; 4]) {
    let slot = UiRect::new(card[0], card[1], card[2], card[3]).inset(&EdgeInsets {
        left: canvas_core::tokens::SPACING_MD,
        top: 0.0,
        right: canvas_core::tokens::SPACING_MD,
        bottom: canvas_core::tokens::SPACING_MD,
    });
    let btn_w = (slot.w - canvas_core::tokens::SPACING_MD) / 2.0;
    let rects = Row {
        gap: canvas_core::tokens::SPACING_MD,
        cross: CrossAlign::End,
        ..Row::default()
    }
    .lay_out_measured(
        slot,
        &[
            MeasuredItem::Fixed {
                w: btn_w,
                h: EMPTY_BTN_H,
            },
            MeasuredItem::Fixed {
                w: btn_w,
                h: EMPTY_BTN_H,
            },
        ],
        m,
        fs,
        FAMILY,
        12.0,
    );
    let as_rect = |r: &UiRect| [r.x, r.y, r.w, r.h];
    (as_rect(&rects[0]), as_rect(&rects[1]))
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

    /// Детерминированный FontSystem тестов: вшитый рендером шрифт.
    fn font_system() -> cosmic_text::FontSystem {
        let mut fs = cosmic_text::FontSystem::new();
        const FONT: &[u8] = include_bytes!("../../../assets/fonts/NotoSansDisplay-Medium.ttf");
        fs.db_mut().load_font_data(FONT.to_vec());
        fs
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
        // Стандартный вьюпорт: панель помещается целиком; каталог из 10
        // схем (аудит 2026-09: 6 → 10) выше окна — окно видимости
        // показывает вместившиеся строки, остальное добирает скролл
        // (clamp_scroll приводит последнюю схему в видимость).
        let lay = layout([1280.0, 800.0], &list, &st);
        assert!(
            !lay.visible_rows.is_empty() && lay.visible_rows.len() <= list.len(),
            "окно видимости непустое и не больше списка"
        );
        assert!(
            lay.visible_rows.len() < list.len(),
            "10 строк по {}px выше вьюпорта 800 — скролл обязан существовать",
            ROW_H
        );
        let mut scrolled = st.clone();
        scrolled.selected = list.len() - 1;
        clamp_scroll(&mut scrolled, lay.visible_rows.len());
        let lay2 = layout([1280.0, 800.0], &list, &scrolled);
        assert!(
            lay2.visible_rows.contains(&(list.len() - 1)),
            "последняя схема доступна прокруткой"
        );
    }

    /// D2 CJM: полный ряд «Все» + N категорий обязан раскладываться на
    /// стандартном и компактном десктопе; FR-053: `break`-кламп удалён —
    /// Row(Fit) раскладывает ВСЕ чипы, и тест фиксирует переполнение,
    /// если оно появится (молчаливый срез невозможен).
    #[test]
    fn chips_all_categories_fit() {
        let registry = SchemeRegistry::embedded();
        let st = state();
        let list = rows(registry, &st);
        for viewport in [[1280.0, 800.0], [1024.0, 768.0], [800.0, 600.0]] {
            let lay = layout(viewport, &list, &st);
            assert_eq!(
                lay.chip_rects.len(),
                1 + categories(registry).len(),
                "все категории в ряду при {:?}",
                viewport
            );
            // Полный ряд реально помещается в слот чипов (без переполнения).
            let chips_w = CHIP_ALL_W
                + canvas_core::tokens::SPACING_S
                + categories(registry).len() as f32 * (CHIP_W + canvas_core::tokens::SPACING_S)
                - canvas_core::tokens::SPACING_S;
            let inner_w = lay.panel_rect[2] - PANEL_PAD * 2.0;
            assert!(
                chips_w <= inner_w + 0.01,
                "ряд чипов {chips_w} шире слота {inner_w} при {:?}",
                viewport
            );
        }
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

    /// FR-053 (G4-линт пилота): вьюпорты 1280×800 / 1024×640 / 800×560 ×
    /// RU/EN — панель в вьюпорте (маржа xl), все элементы внутри панели,
    /// чипы/строки попарно не пересекаются; измеренные подписи строк
    /// укладываются в ширину строки.
    #[test]
    fn g4_lint_viewports_and_languages() {
        let registry = SchemeRegistry::embedded();
        let st = state();
        let list = rows(registry, &st);
        let viewports = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];
        for ru in [true, false] {
            for vp in viewports {
                let lay = layout(vp, &list, &st);
                // Панель внутри вьюпорта (маржа xl).
                assert!(
                    lay.panel_rect[0] >= canvas_core::tokens::SPACING_XL - 0.01,
                    "{vp:?}"
                );
                assert!(
                    lay.panel_rect[0] + lay.panel_rect[2]
                        <= vp[0] - canvas_core::tokens::SPACING_XL + 0.01,
                    "{vp:?}"
                );
                assert!(
                    lay.panel_rect[1] >= canvas_core::tokens::SPACING_XL - 0.01,
                    "{vp:?}"
                );
                // Элементы внутри панели.
                for r in lay
                    .row_rects
                    .iter()
                    .chain(lay.chip_rects.iter().map(|(r, _)| r))
                    .chain([&lay.input_rect, &lay.footer_rect])
                {
                    assert!(r[0] >= lay.panel_rect[0] - 0.01, "левее панели {vp:?}");
                    assert!(
                        r[0] + r[2] <= lay.panel_rect[0] + lay.panel_rect[2] + 0.01,
                        "за правым краем панели {vp:?}"
                    );
                    assert!(r[1] >= lay.panel_rect[1] - 0.01, "выше панели {vp:?}");
                    assert!(
                        r[1] + r[3] <= lay.panel_rect[1] + lay.panel_rect[3] + 0.01,
                        "ниже панели {vp:?}"
                    );
                }
                // Чипы и строки попарно не пересекаются.
                let chips: Vec<[f32; 4]> = lay.chip_rects.iter().map(|(r, _)| *r).collect();
                for group in [chips, lay.row_rects.clone()] {
                    for i in 0..group.len() {
                        for j in i + 1..group.len() {
                            let a = UiRect::new(group[i][0], group[i][1], group[i][2], group[i][3]);
                            let b = UiRect::new(group[j][0], group[j][1], group[j][2], group[j][3]);
                            assert!(!a.intersects(&b), "пересечение {i}×{j} при {vp:?}");
                        }
                    }
                }
                // Измеренные подписи строк укладываются в ширину строки.
                let mut fs = font_system();
                let mut m = TextMeasurer::new();
                let labels = row_labels(vp, &list, &st, ru, &mut m, &mut fs);
                assert_eq!(labels.len(), lay.row_rects.len());
                for (label, rect) in labels.iter().zip(lay.row_rects.iter()) {
                    let inner = rect[2] - ROW_TEXT_PAD * 2.0;
                    assert!(
                        m.width_of(&mut fs, &label.title, FAMILY, ROW_FONT) <= inner + 0.05,
                        "заголовок шире строки при {vp:?}"
                    );
                    assert!(
                        m.width_of(&mut fs, &label.desc, FAMILY, ROW_DESC_FONT) <= inner + 0.05,
                        "описание шире строки при {vp:?}"
                    );
                }
            }
        }
    }
}
