//! FR-068 W3: список/скролл (ScrollState/list_rows/scroll_bar) — перенос из kit.rs 1:1 (W3).
//!
//! W3 (агент 3-c): слой компонентной модели ДОБАВЛЕН: [`ListProps`],
//! [`List`] и `impl Component` (layout/paint/hit_test). Стабильный API
//! (FR-058) не менялся: [`ScrollState`], [`list_rows`], [`scroll_bar`].

use super::{Component, KitPalette, SCROLLBAR_KNOB_MIN, SCROLLBAR_WIDTH};
use crate::geometry::UiRect;
use crate::layout::LayoutBackend;
use crate::paint::Painter;

// --- Список + скролл ---------------------------------------------------------

/// Состояние скролла списка: `offset` — сдвиг контента вверх (px),
/// `content_h`/`viewport_h` — высота контента и окна видимости.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScrollState {
    /// Сдвиг контента вверх от начала (px); 0 — начало списка.
    pub offset: f32,
    /// Полная высота контента (сумма высот всех строк + зазоры).
    pub content_h: f32,
    /// Высота окна видимости (слота списка).
    pub viewport_h: f32,
}

impl ScrollState {
    /// Сдвинуть скролл на `dy` px (отрицательное — вверх). Без клампа (вызовите
    /// [`clamp`](Self::clamp) после, если нужно удержать в границах).
    pub fn scroll_by(&mut self, dy: f32) {
        self.offset = (self.offset + dy).max(0.0);
    }

    /// Зажать `offset` в `[0, max_offset]`.
    pub fn clamp(&mut self) {
        let max = self.max_offset();
        if self.offset > max {
            self.offset = max;
        }
        if self.offset < 0.0 {
            self.offset = 0.0;
        }
    }

    /// Нужен ли скролл (контент выше вьюпорта).
    pub fn needs_scroll(&self) -> bool {
        self.content_h > self.viewport_h
    }

    /// Максимальный сдвиг: `content_h - viewport_h`, не меньше 0.
    pub fn max_offset(&self) -> f32 {
        (self.content_h - self.viewport_h).max(0.0)
    }
}

/// Видимые строки списка: `(индекс, экранный rect)`. Чистая функция — без
/// мутаций `ScrollState`. Строка видима, если её низ ниже верха вьюпорта и
/// верх ниже низа вьюпорта (частичные строки на краях включаются).
pub fn list_rows(
    area: UiRect,
    s: &ScrollState,
    row_h: f32,
    gap: f32,
    count: usize,
) -> Vec<(usize, UiRect)> {
    if row_h <= 0.0 || count == 0 || s.viewport_h <= 0.0 {
        return Vec::new();
    }
    let stride = row_h + gap;
    // Первая видимая строка: наименьшее i, где низ (i*stride + row_h) > offset.
    // i > (offset - row_h) / stride. Учитываем частичную строку сверху.
    let first = (((s.offset - row_h) / stride).max(0.0).ceil() as usize).min(count);
    // Последняя видимая (включительно): наибольшее i, где верх (i*stride) <
    // offset + viewport_h. i < (offset + viewport_h) / stride.
    let last_inclusive = (((s.offset + s.viewport_h) / stride).ceil() as usize)
        .saturating_sub(1)
        .min(count.saturating_sub(1));
    let mut out = Vec::new();
    for i in first..=last_inclusive {
        let content_y = i as f32 * stride;
        let screen_y = area.y + content_y - s.offset;
        out.push((i, UiRect::new(area.x, screen_y, area.w, row_h)));
    }
    out
}

/// Бегунок скроллбара — только когда `needs_scroll`. Возвращает rect бегунка
/// (трек = правая полоса `area` шириной [`SCROLLBAR_WIDTH`]); `None` — скролл
/// не нужен. Цвет — на потребителе (слот `control_border`/`text_muted`).
pub fn scroll_bar(area: UiRect, s: &ScrollState, _p: &KitPalette) -> Option<UiRect> {
    if !s.needs_scroll() {
        return None;
    }
    let max = s.max_offset();
    if max <= 0.0 {
        return None;
    }
    let track_x = area.right() - SCROLLBAR_WIDTH;
    let track_h = area.h;
    // Бегунок: высота ∝ viewport/content, минимум SCROLLBAR_KNOB_MIN.
    let ratio = (s.viewport_h / s.content_h).clamp(0.0, 1.0);
    let knob_h = (track_h * ratio).max(SCROLLBAR_KNOB_MIN).min(track_h);
    // Позиция: 0 → верх, max → низ.
    let pos_ratio = (s.offset / max).clamp(0.0, 1.0);
    let knob_y = area.y + (track_h - knob_h) * pos_ratio;
    Some(UiRect::new(track_x, knob_y, SCROLLBAR_WIDTH, knob_h))
}

// --- List: компонентная модель (FR-068 W3) ----------------------------------

/// Свойства [`List`] (декларативный вход кадра; FR-068 W3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListProps {
    /// Высота строки (ui px) — параметр потребителя (кит-константы
    /// `LIST_ROW_H`/`LIST_ROW_GAP` — лишь дефолты кита).
    pub row_h: f32,
    /// Зазор между строками (ui px).
    pub gap: f32,
    /// Срез слотов палитры (бегунок — слот `control_border`).
    pub palette: KitPalette,
}

/// Список — retained-компонент (FR-068 W3): `Props` + стабильный
/// [`ScrollState`] (FR-058; поле `scroll` публично — потребитель ведёт
/// offset/content_h/viewport_h через `scroll_by`/`clamp`).
///
/// Разделение труда (контракт кита «list_rows — чистая геометрия»):
/// [`Component::layout`] — ТОЛЬКО rect'ы видимых строк ([`list_rows`], в
/// порядке модельных индексов); фон/зебру/выделение/тексты строк рисует
/// ПОТРЕБИТЕЛЬ слотами своей темы (hover/selected — его WidgetState'ы —
/// W3 без hover-логики в List, образец `canvas-app/overlays.rs`:
/// `chip_style(row_widget.kit_state(), …)` по rect'ам list_rows).
/// [`Component::paint`] — только хром скроллбара: бегунок [`scroll_bar`]
/// слотом `control_border`. Строки НЕ рисуются: прозрачный rect строки —
/// мусорный item в draw-журнале Painter (G7-данные).
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    /// Свойства кадра.
    pub props: ListProps,
    /// Состояние скролла (стабильное поле; FR-058).
    pub scroll: ScrollState,
}

impl List {
    /// Новый список: скролл — дефолтный (offset 0; content/viewport — 0:
    /// потребитель заполняет `scroll` своей моделью до layout).
    pub fn new(props: ListProps) -> Self {
        Self {
            props,
            scroll: ScrollState::default(),
        }
    }

    /// Число строк модели из инварианта `scroll.content_h` («полная высота
    /// контента — сумма высот всех строк + зазоры»):
    /// `content_h = n·row_h + (n−1)·gap ⇒ n = (content_h + gap)/(row_h+gap)`;
    /// округление гасит ошибку f32 авторского `content_h`. `row_h+gap ≤ 0`
    /// (и NaN) — 0. Сигнатура [`Component::layout`] числа строк не принимает,
    /// а `ListProps` по постановке W3 его не содержит — восстанавливаем из
    /// `content_h`, который потребитель и так ведёт рядом с моделью строк.
    fn row_count(&self) -> usize {
        let stride = self.props.row_h + self.props.gap;
        if stride <= 0.0 || stride.is_nan() {
            return 0;
        }
        ((self.scroll.content_h + self.props.gap) / stride)
            .round()
            .max(0.0) as usize
    }
}

impl Component for List {
    type Props = ListProps;

    fn props(&self) -> &Self::Props {
        &self.props
    }

    /// Видимые строки: [`list_rows`] в слоте (offset/viewport — из
    /// `self.scroll`; row_h/gap — из `self.props`; контракт: потребитель
    /// держит `scroll.viewport_h == slot.h`). Backend не используется:
    /// [`list_rows`] — чистая функция слота (паритет движков тривиален,
    /// §Контракт-3 FR-068).
    fn layout(&self, _backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        let count = self.row_count();
        list_rows(slot, &self.scroll, self.props.row_h, self.props.gap, count)
            .into_iter()
            .map(|(_, rect)| rect)
            .collect()
    }

    /// ТОЛЬКО бегунок скроллбара ([`scroll_bar`]; `None` — 0 items):
    /// слот `control_border`, прозрачная рамка, радиус = w/2 (пилюля —
    /// как у потребителя-пилота: `d.rect(knob, control_border, [0;4], 2.0)`).
    /// Область бегунка восстанавливается из `rects` (строки [`list_rows`]
    /// full-width в вьюпорте): x/w — от первой видимой строки; y — её верх
    /// (точен при offset = 0 и выровненном по stride offset); h —
    /// `scroll.viewport_h` (окно видимости — точно по контракту). Точный
    /// якорь по слоту — потребитель вызывает [`scroll_bar`] сам.
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&first) = rects.first() else {
            return;
        };
        let area = UiRect::new(first.x, first.y, first.w, self.scroll.viewport_h);
        if let Some(knob) = scroll_bar(area, &self.scroll, &self.props.palette) {
            painter.rect(
                knob,
                self.props.palette.control_border,
                [0.0; 4],
                knob.w / 2.0,
            );
        }
    }

    // hit_test — дефолтный: ComponentHit::pick по rects — индекс строки,
    // содержащей точку (зазор между строками — None: строка не перекрывает
    // свой stride).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{palette_a, palette_b};
    use crate::component::{ComponentHit, LIST_ROW_GAP, LIST_ROW_H};
    use crate::geometry::UiPoint;
    use crate::layout::default_backend;
    use crate::paint::{PaintItem, Painter};
    #[test]
    fn scroll_state_scroll_by_positive_and_negative() {
        let mut s = ScrollState {
            offset: 10.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        s.scroll_by(50.0);
        assert_eq!(s.offset, 60.0);
        s.scroll_by(-30.0);
        assert_eq!(s.offset, 30.0);
        // Отрицательный результат клампится к 0 (не уходит в минус)
        s.scroll_by(-100.0);
        assert_eq!(s.offset, 0.0);
    }
    #[test]
    fn scroll_state_clamp_at_edges() {
        let mut s = ScrollState {
            offset: 200.0, // больше max_offset
            content_h: 200.0,
            viewport_h: 100.0,
        };
        s.clamp();
        assert_eq!(s.offset, 100.0, "clamp к max_offset");
        // offset < 0 — кламп к 0
        s.offset = -10.0;
        s.clamp();
        assert_eq!(s.offset, 0.0);
        // offset в пределах — без изменений
        s.offset = 50.0;
        s.clamp();
        assert_eq!(s.offset, 50.0);
    }
    #[test]
    fn scroll_state_needs_scroll_and_max_offset() {
        // Контент выше вьюпорта — нужен скролл
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        assert!(s.needs_scroll());
        assert_eq!(s.max_offset(), 100.0);
        // Контент равен вьюпорту — скролл не нужен, max_offset = 0
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(!s.needs_scroll());
        assert_eq!(s.max_offset(), 0.0);
        // Контент меньше вьюпорта — скролл не нужен, max_offset = 0 (не отрицательный)
        let s = ScrollState {
            offset: 0.0,
            content_h: 50.0,
            viewport_h: 100.0,
        };
        assert!(!s.needs_scroll());
        assert_eq!(s.max_offset(), 0.0);
    }

    // --- list_rows: оффсет → индексы/rect'ы, частичные строки ----------------
    #[test]
    fn list_rows_empty_when_no_rows_or_zero_height() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 0.0,
            content_h: 0.0,
            viewport_h: 100.0,
        };
        assert!(list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 0).is_empty());
        // row_h = 0 — вырожденный, пустой результат
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(list_rows(area, &s, 0.0, LIST_ROW_GAP, 5).is_empty());
    }
    #[test]
    fn list_rows_all_visible_when_no_scroll() {
        let area = UiRect::new(10.0, 20.0, 200.0, 100.0);
        // 3 строки по 26px + 2 зазора по 6 = 90px — помещаются в 100px
        let s = ScrollState {
            offset: 0.0,
            content_h: 90.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].0, 0);
        assert_eq!(rows[1].0, 1);
        assert_eq!(rows[2].0, 2);
        // Геометрия: первая строка у верха area
        assert!((rows[0].1.y - area.y).abs() < 0.01);
        assert!((rows[0].1.x - area.x).abs() < 0.01);
        assert_eq!(rows[0].1.w, area.w);
        assert_eq!(rows[0].1.h, LIST_ROW_H);
        // Вторая — на stride ниже
        let stride = LIST_ROW_H + LIST_ROW_GAP;
        assert!((rows[1].1.y - (area.y + stride)).abs() < 0.01);
    }
    #[test]
    fn list_rows_offset_skips_hidden_rows() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // 5 строк по 26 + 4 зазора по 6 = 154px content; viewport 100
        // stride = 32. offset = 32 (1 stride) — скрываем строку 0.
        let s = ScrollState {
            offset: 32.0,
            content_h: 154.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 5);
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(!indices.contains(&0), "строка 0 скрыта offset'ом");
        assert!(indices.contains(&1), "строка 1 видна");
        assert!(indices.contains(&2), "строка 2 видна");
        assert!(indices.contains(&3), "строка 3 видна");
        assert!(indices.contains(&4), "строка 4 видна");
        // Строка 1 — у верха area (screen_y = 0 + 32 - 32 = 0)
        let row1 = rows.iter().find(|(i, _)| *i == 1).unwrap().1;
        assert!(
            (row1.y - 0.0).abs() < 0.01,
            "строка 1 у верха viewport: y={}",
            row1.y
        );
    }
    #[test]
    fn list_rows_includes_partial_row_at_top() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // offset=40 — строка 1 (top=32, bottom=58) видна частично сверху (8px).
        let s = ScrollState {
            offset: 40.0,
            content_h: 154.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 5);
        // Строка 0 скрыта (bottom=26 < offset=40)
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(!indices.contains(&0), "строка 0 скрыта");
        // Строка 1 видна частично сверху: screen_y = 0 + 32 - 40 = -8
        let row1 = rows.iter().find(|(i, _)| *i == 1).unwrap().1;
        assert!(
            (row1.y - (-8.0)).abs() < 0.01,
            "частичная строка сверху: y={}",
            row1.y
        );
    }
    #[test]
    fn list_rows_includes_partial_rows_at_bottom() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // offset=0, viewport=100, row_h=26, gap=6 → stride=32
        // Строки 0..3 полностью (0..96), строка 3 (96..122) — частично снизу (96..100)
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let rows = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 10);
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert!(indices.contains(&3), "частичная строка 3 включена снизу");
        assert!(!indices.contains(&4), "строка 4 за пределами viewport");
    }
    #[test]
    fn list_rows_does_not_mutate_scroll_state() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 30.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let s_before = s.clone();
        let _ = list_rows(area, &s, LIST_ROW_H, LIST_ROW_GAP, 10);
        assert_eq!(s, s_before, "чистая функция — без мутаций");
    }

    // --- scroll_bar: None когда не нужен, knob при needs_scroll --------------
    #[test]
    fn scroll_bar_none_when_no_scroll_needed() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let s = ScrollState {
            offset: 0.0,
            content_h: 100.0,
            viewport_h: 100.0,
        };
        assert!(scroll_bar(area, &s, &palette_a()).is_none());
        // content_h < viewport_h — тоже нет
        let s = ScrollState {
            offset: 0.0,
            content_h: 50.0,
            viewport_h: 100.0,
        };
        assert!(scroll_bar(area, &s, &palette_a()).is_none());
    }
    #[test]
    fn scroll_bar_knob_geometry_proportional_and_positioned() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // content=200, viewport=100 → ratio=0.5, knob_h=50, max_offset=100
        let s = ScrollState {
            offset: 0.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert_eq!(knob.x, area.right() - SCROLLBAR_WIDTH);
        assert_eq!(knob.w, SCROLLBAR_WIDTH);
        assert!((knob.h - 50.0).abs() < 0.01, "knob_h = 0.5 * 100 = 50");
        // offset=0 → knob у верха
        assert!((knob.y - area.y).abs() < 0.01);
        // offset=max → knob у низа
        let s = ScrollState {
            offset: 100.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert!(
            (knob.bottom() - area.bottom()).abs() < 0.01,
            "knob у низа при max offset"
        );
        // offset=50 → knob в середине
        let s = ScrollState {
            offset: 50.0,
            content_h: 200.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        let expected_y = area.y + (100.0 - 50.0) * 0.5; // 25
        assert!((knob.y - expected_y).abs() < 0.01);
    }
    #[test]
    fn scroll_bar_knob_min_height_enforced() {
        let area = UiRect::new(0.0, 0.0, 200.0, 100.0);
        // content=1000, viewport=100 → ratio=0.1 → knob_h=10 → clamped to MIN=20
        let s = ScrollState {
            offset: 0.0,
            content_h: 1000.0,
            viewport_h: 100.0,
        };
        let knob = scroll_bar(area, &s, &palette_a()).unwrap();
        assert!(knob.h >= SCROLLBAR_KNOB_MIN, "минимальная высота бегунка");
    }

    // --- List: компонентная модель (FR-068 W3) -------------------------------
    #[test]
    fn list_component_layout_matches_list_rows_directly() {
        let slot = UiRect::new(10.0, 20.0, 200.0, 100.0);
        // 5 строк: content_h = 5·26 + 4·6 = 154 > viewport 100 — нужен скролл.
        let scroll = ScrollState {
            offset: 0.0,
            content_h: 154.0,
            viewport_h: 100.0,
        };
        let mut list = List {
            props: ListProps {
                row_h: LIST_ROW_H,
                gap: LIST_ROW_GAP,
                palette: palette_a(),
            },
            scroll,
        };
        let rects = list.layout(default_backend(), slot);
        let direct = list_rows(slot, &list.scroll, LIST_ROW_H, LIST_ROW_GAP, 5);
        assert!(!rects.is_empty(), "видимые строки есть");
        assert_eq!(
            rects.len(),
            direct.len(),
            "количество согласовано с list_rows напрямую"
        );
        for (got, (_, expected)) in rects.iter().zip(&direct) {
            assert_eq!(got, expected, "rect строки = list_rows");
        }
        // Смещённый offset (частичные строки краёв включены) — тоже согласован.
        list.scroll.offset = 40.0;
        let rects = list.layout(default_backend(), slot);
        let direct = list_rows(slot, &list.scroll, LIST_ROW_H, LIST_ROW_GAP, 5);
        assert_eq!(rects.len(), direct.len());
        for (got, (_, expected)) in rects.iter().zip(&direct) {
            assert_eq!(got, expected);
        }
    }
    #[test]
    fn list_component_paint_emits_scrollbar_knob_when_needed() {
        let palette = palette_a();
        let slot = UiRect::new(10.0, 20.0, 200.0, 100.0);
        let mut list = List {
            props: ListProps {
                row_h: LIST_ROW_H,
                gap: LIST_ROW_GAP,
                palette,
            },
            scroll: ScrollState {
                offset: 0.0,
                content_h: 154.0,
                viewport_h: 100.0,
            },
        };
        let rects = list.layout(default_backend(), slot);
        let mut p = Painter::new();
        list.paint(&mut p, &rects);
        assert_eq!(
            p.items().len(),
            1,
            "needs_scroll — ровно один item: бегунок"
        );
        let expected = scroll_bar(slot, &list.scroll, &palette).unwrap();
        assert_eq!(
            p.items()[0],
            PaintItem::Rect {
                rect: expected,
                fill: palette.control_border,
                border: [0.0; 4],
                radius: expected.w / 2.0,
            },
            "бегунок: слот control_border, прозрачная рамка, радиус w/2"
        );
        // Контент меньше вьюпорта — скроллбара нет (0 items; строки рисует
        // потребитель — paint строк не эмитит никогда).
        list.scroll = ScrollState {
            offset: 0.0,
            content_h: 90.0,
            viewport_h: 100.0,
        };
        let rects = list.layout(default_backend(), slot);
        let mut p = Painter::new();
        list.paint(&mut p, &rects);
        assert!(p.items().is_empty(), "без переполнения — 0 items");
    }
    #[test]
    fn list_component_hit_test_finds_visible_row() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let list = List {
            props: ListProps {
                row_h: LIST_ROW_H,
                gap: LIST_ROW_GAP,
                palette: palette_b(),
            },
            scroll: ScrollState {
                offset: 0.0,
                content_h: 154.0,
                viewport_h: 100.0,
            },
        };
        let rects = list.layout(default_backend(), slot);
        // Центр второй строки (stride 32: y=32..58) — индекс 1.
        let row1 = rects[1];
        let inside = UiPoint::new(row1.x + row1.w / 2.0, row1.y + row1.h / 2.0);
        assert_eq!(
            list.hit_test(&rects, inside),
            Some(ComponentHit { index: 1 }),
            "hit_test дефолтный: индекс строки-rect'а"
        );
        // Точка в зазоре между строками (y = 26..32) — ни одна не содержит.
        let gap_point = UiPoint::new(100.0, 29.0);
        assert_eq!(list.hit_test(&rects, gap_point), None);
    }

    // --- switch: on/off позиция, геометрия в слоте, стили --------------------
}
