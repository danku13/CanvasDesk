//! FR-057 (волна 2 кита, PRD-0009 §8 F-8/§9.2): [`Painter`] — draw-слой
//! крейта `canvas-ui`. Собирает примитивы отрисовки ([`PaintItem`]) как
//! ДАННЫЕ: крейт остаётся без wgpu/winit и внешних зависимостей (инвариант
//! G7 FR-051) — конвертацию в `CardInstance`/`OwnedText` выполняет
//! крейт-потребитель (как сегодня: `canvas-app/src/kit_ui.rs`).
//!
//! До FR-057 адаптер рисования жил в потребителе (`KitDraw` kit_ui.rs:400–469)
//! и копировался каждой новой поверхностью; теперь модель items — в крейте,
//! а `KitDraw` — тонкая обёртка над [`Painter`] (методы и поведение 1:1).
//!
//! Порядок items = порядок вызовов = draw-порядок (позже нарисованные
//! поверх ранних). `take_items` отдаёт накопленное и очищает журнал —
//! потребитель конвертирует items в инстансы рендера на своём кадре.

use crate::geometry::{UiPoint, UiRect};
use crate::kit::{ControlStyle, PanelStyle};

/// Выравнивание текста внутри области (зеркало `TextAlign` рендера —
/// крейт не знает о glyphon; конвертация на стороне потребителя).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintAlign {
    /// По левому краю области.
    Left,
    /// По центру области (контракт ScreenText: origin — левый край области
    /// выравнивания, Center центрирует в [origin, origin+width]).
    Center,
}

/// Примитив отрисовки — данные без GPU-типов (G7).
#[derive(Debug, Clone, PartialEq)]
pub enum PaintItem {
    /// Прямоугольник с заливкой/рамкой/радиусом.
    Rect {
        rect: UiRect,
        fill: [f32; 4],
        border: [f32; 4],
        radius: f32,
    },
    /// Текст в области (без шейпинга — ширину/усечение потребитель измеряет
    /// `TextMeasurer`'ом ДО вызова; здесь только место и стиль).
    Text {
        area: UiRect,
        text: String,
        color: [f32; 4],
        size: f32,
        align: PaintAlign,
    },
    /// FR-ICONS: SVG-иконка в области (квадратная, вписывается в `rect` по
    /// центру; tint — RGBA). `name` — строковый идентификатор (напр. "close",
    /// "gear"); набор (`IconStyle`) разрешает потребителем — Painter данных
    /// о наборе не хранит (G7: чистые данные, 0 зависимостей). Потребитель
    /// (KitDraw в canvas-app) решает: SVG-атлас или глиф-фолбэк (по
    /// `settings.icon_style`).
    Icon {
        rect: UiRect,
        name: String,
        tint: [f32; 4],
    },
    /// Клип-контейнер (FR-068 W1): содержимое рисуется только внутри `rect`
    /// (overflow:hidden-семантика в draw-слое). Вложенность — произвольная
    /// (клип в клипе — сужение области). Painter остаётся ДАННЫМИ (G7):
    /// отсечение исполняет потребитель — конвертация в scissor FR-056 на
    /// своём кадре (canvas-app kit_ui); UiLayer не затрагивается —
    /// слои/capture/draw-порядок без изменений (§Контракт-5 FR-068,
    /// D8 ADR-0013).
    ClipRect { rect: UiRect, items: Vec<PaintItem> },
    /// Transform-контейнер (FR-074): CSS `transform: rotate(deg)` —
    /// поворот вложенных items на `deg` градусов (по часовой —
    /// экранные координаты y-вниз) вокруг точки `origin` (ui px,
    /// обычно центр элемента — см. [`Painter::rotated_centered`]).
    /// CSS-семантика: layout НЕ меняет (геометрия rect'ов внутри —
    /// до-трансформационная; позиция/размер для хит-теста и клипа —
    /// забота потребителя). Аналог ClipRect — данные (G7): исполняет
    /// потребитель (конвертация в rotate-инстансы рендера — отдельная
    /// задача; до неё — прозрачный проход, как ClipRect в W1 до scissor).
    Transform {
        deg: f32,
        origin: UiPoint,
        items: Vec<PaintItem>,
    },
    /// z-index группа (FR-074): поддерево с явным z-порядком внутри
    /// stacking context'а Painter'а (CSS z-index). `take_items`
    /// стабильно сортирует журнал по z (больше z — позже = поверх;
    /// равные z — порядок вызовов); [`Painter::z_group`] вкладывает
    /// контекст в контекст (CSS stacking context). Данные (G7):
    /// потребитель рисует в уже отсортированном порядке.
    ZGroup { z: i32, items: Vec<PaintItem> },
}

/// Шаг плоского обхода дерева items ([`walk`]): контейнер клипа или лист.
/// Transform/ZGroup (FR-074) — прозрачные контейнеры: walk спускается
/// в них без шага (не клипы; порядок draw сохраняется).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClipStep<'a> {
    /// Rect клип-контейнера ([`PaintItem::ClipRect`]); следующие шаги —
    /// его поддерево в draw-порядке.
    Clip(UiRect),
    /// Листовой item (Rect/Text) в draw-порядке.
    Item(&'a PaintItem),
}

/// Плоский обход дерева items в draw-порядке (FR-068 W1):
/// [`PaintItem::ClipRect`] разворачивается — сначала сам rect-контейнер как
/// [`ClipStep::Clip`], затем дети (клип в клипе — DFS: вложенный `Clip`
/// идёт перед оставшимися детьми внешнего). Порядок детерминирован порядком
/// вызовов Painter'а; хелпер — потребителям (дампы/конвертация в scissor
/// FR-056) и тестам.
pub fn walk(items: &[PaintItem]) -> Vec<ClipStep<'_>> {
    fn push_steps<'a>(items: &'a [PaintItem], out: &mut Vec<ClipStep<'a>>) {
        for item in items {
            if let PaintItem::ClipRect { rect, items } = item {
                out.push(ClipStep::Clip(*rect));
                push_steps(items, out);
            } else if let PaintItem::Transform { items, .. } | PaintItem::ZGroup { items, .. } =
                item
            {
                // FR-074: прозрачный спуск — не клип, draw-порядок детей
                // в общем обходе сохраняется.
                push_steps(items, out);
            } else {
                out.push(ClipStep::Item(item));
            }
        }
    }
    let mut steps = Vec::new();
    push_steps(items, &mut steps);
    steps
}

/// Сборщик примитивов отрисовки (журнал items в порядке вызовов).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Painter {
    items: Vec<PaintItem>,
}

impl Painter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Прямоугольник с заливкой/рамкой/радиусом.
    pub fn rect(&mut self, r: UiRect, fill: [f32; 4], border: [f32; 4], radius: f32) {
        self.items.push(PaintItem::Rect {
            rect: r,
            fill,
            border,
            radius,
        });
    }

    /// Стиль контрола (заливка + рамка из [`ControlStyle`]).
    pub fn control(&mut self, r: UiRect, s: &ControlStyle) {
        self.rect(r, s.fill, s.border, s.radius);
    }

    /// Стиль панели (заливка/рамка/радиус из [`PanelStyle`]); пад панели —
    /// забота раскладки потребителя (`panel_content`), не отрисовки.
    pub fn panel(&mut self, r: UiRect, s: &PanelStyle) {
        self.rect(r, s.fill, s.border, s.radius);
    }

    /// Текст в области (цвет/кегль/выравнивание — слоты потребителя).
    pub fn label(
        &mut self,
        area: UiRect,
        text: &str,
        color: [f32; 4],
        size: f32,
        align: PaintAlign,
    ) {
        self.items.push(PaintItem::Text {
            area,
            text: text.to_owned(),
            color,
            size,
            align,
        });
    }

    /// FR-ICONS: SVG-иконка в области (квадратная, по центру; tint — RGBA).
    /// `name` — строковый идентификатор (напр. "close", "gear"). Набор
    /// (`IconStyle`) разрешает потребитель — Painter только копит данные (G7).
    pub fn icon(&mut self, rect: UiRect, name: &str, tint: [f32; 4]) {
        self.items.push(PaintItem::Icon {
            rect,
            name: name.to_owned(),
            tint,
        });
    }

    /// Клип-контейнер: items, нарисованные в `paint`, оборачиваются в
    /// [`PaintItem::ClipRect`] с rect `r` (порядок вложенных = порядок
    /// вызовов; см. [`walk`] для плоского обхода). Отсечение — забота
    /// потребителя (scissor FR-056), Painter копит данные (G7).
    pub fn clip_rect(&mut self, r: UiRect, paint: impl FnOnce(&mut Painter)) {
        let mut inner = Painter::new();
        paint(&mut inner);
        self.items.push(PaintItem::ClipRect {
            rect: r,
            items: inner.take_items(),
        });
    }

    /// Transform-контейнер (FR-074): CSS `transform: rotate(deg)` вокруг
    /// точки `origin` (ui px; положительный угол — по часовой в экранной
    /// системе y-вниз). Layout не меняет; данные — G7 (исполняет
    /// потребитель). Вложенность — произвольная (композиция поворотов —
    /// забота потребителя).
    pub fn rotated(&mut self, deg: f32, origin: UiPoint, paint: impl FnOnce(&mut Painter)) {
        let mut inner = Painter::new();
        paint(&mut inner);
        self.items.push(PaintItem::Transform {
            deg,
            origin,
            items: inner.take_items(),
        });
    }

    /// Поворот вокруг ЦЕНТРА области (частый случай: иконка/бейдж на
    /// карточке) — `origin` = центр `area`.
    pub fn rotated_centered(&mut self, area: UiRect, deg: f32, paint: impl FnOnce(&mut Painter)) {
        let origin = UiPoint::new(area.x + area.w / 2.0, area.y + area.h / 2.0);
        self.rotated(deg, origin, paint);
    }

    /// z-index группа (FR-074): CSS z-index — поддерево с явным z-порядком
    /// внутри текущего stacking context'а (журнал Painter'а). `take_items`
    /// стабильно сортирует по z: больше — поверх; равные — порядок вызовов.
    /// Вложенный z_group — свой контекст (как CSS stacking context).
    pub fn z_group(&mut self, z: i32, paint: impl FnOnce(&mut Painter)) {
        let mut inner = Painter::new();
        paint(&mut inner);
        self.items.push(PaintItem::ZGroup {
            z,
            items: inner.take_items(),
        });
    }

    /// Журнал items (порядок = draw-порядок).
    pub fn items(&self) -> &[PaintItem] {
        &self.items
    }

    /// Отдать накопленное и очистить журнал (одна конвертация на кадр).
    /// FR-074: перед отдачей журнал СТАБИЛЬНО сортируется по z (CSS
    /// z-index): ZGroup-ы — по их z, остальные items (z = 0) сохраняют
    /// относительный порядок между собой и относительно ZGroup с z = 0.
    pub fn take_items(&mut self) -> Vec<PaintItem> {
        self.items.sort_by_key(|item| match item {
            PaintItem::ZGroup { z, .. } => *z,
            _ => 0,
        });
        std::mem::take(&mut self.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::EdgeInsets;

    const FILL: [f32; 4] = [0.2, 0.4, 0.6, 1.0];
    const BORDER: [f32; 4] = [0.1, 0.2, 0.3, 0.9];
    const TEXT: [f32; 4] = [0.9, 0.9, 0.9, 1.0];

    fn control_style() -> ControlStyle {
        ControlStyle {
            fill: FILL,
            border: BORDER,
            text: TEXT,
            radius: 6.0,
        }
    }

    fn panel_style() -> PanelStyle {
        PanelStyle {
            fill: FILL,
            border: BORDER,
            radius: 8.0,
            pad: EdgeInsets::uniform(12.0),
        }
    }

    /// Порядок items = порядок вызовов (draw-порядок), payload дословный.
    #[test]
    fn items_keep_insertion_order_and_payload() {
        let mut p = Painter::new();
        p.rect(UiRect::new(1.0, 2.0, 3.0, 4.0), FILL, BORDER, 4.0);
        p.control(UiRect::new(5.0, 6.0, 7.0, 8.0), &control_style());
        p.panel(UiRect::new(9.0, 10.0, 11.0, 12.0), &panel_style());
        p.label(
            UiRect::new(13.0, 14.0, 40.0, 16.0),
            "Подпись",
            TEXT,
            13.0,
            PaintAlign::Center,
        );

        let items = p.items();
        assert_eq!(items.len(), 4);
        assert_eq!(
            items[0],
            PaintItem::Rect {
                rect: UiRect::new(1.0, 2.0, 3.0, 4.0),
                fill: FILL,
                border: BORDER,
                radius: 4.0,
            }
        );
        // control/panel — rect со слотами стиля (пад панели не рисуется)
        assert_eq!(
            items[1],
            PaintItem::Rect {
                rect: UiRect::new(5.0, 6.0, 7.0, 8.0),
                fill: FILL,
                border: BORDER,
                radius: 6.0,
            }
        );
        assert_eq!(
            items[2],
            PaintItem::Rect {
                rect: UiRect::new(9.0, 10.0, 11.0, 12.0),
                fill: FILL,
                border: BORDER,
                radius: 8.0,
            }
        );
        assert_eq!(
            items[3],
            PaintItem::Text {
                area: UiRect::new(13.0, 14.0, 40.0, 16.0),
                text: "Подпись".to_owned(),
                color: TEXT,
                size: 13.0,
                align: PaintAlign::Center,
            }
        );
    }

    /// take_items отдаёт всё накопленное и ОЧИЩАЕТ журнал (повторный вызов —
    /// пустой вектор; новый контент копится с нуля).
    #[test]
    fn take_items_drains_and_clears() {
        let mut p = Painter::new();
        p.rect(UiRect::new(0.0, 0.0, 10.0, 10.0), FILL, BORDER, 0.0);
        p.label(
            UiRect::new(0.0, 0.0, 10.0, 10.0),
            "Т",
            TEXT,
            12.0,
            PaintAlign::Left,
        );
        assert_eq!(p.items().len(), 2);

        let taken = p.take_items();
        assert_eq!(taken.len(), 2);
        assert!(matches!(taken[0], PaintItem::Rect { .. }));
        assert!(matches!(taken[1], PaintItem::Text { .. }));
        // Журнал пуст после take
        assert!(p.items().is_empty());
        assert!(p.take_items().is_empty());

        // Новый контент копится с чистого листа
        p.rect(UiRect::new(1.0, 1.0, 2.0, 2.0), FILL, BORDER, 1.0);
        assert_eq!(p.items().len(), 1);
    }

    /// Выравнивание текста сохраняется как есть (Left/Center — данные).
    #[test]
    fn label_keeps_align_variant() {
        let mut p = Painter::new();
        let area = UiRect::new(0.0, 0.0, 100.0, 20.0);
        p.label(area, "Л", TEXT, 11.0, PaintAlign::Left);
        p.label(area, "Ц", TEXT, 11.0, PaintAlign::Center);
        match (&p.items()[0], &p.items()[1]) {
            (PaintItem::Text { align: a, .. }, PaintItem::Text { align: b, .. }) => {
                assert_eq!(a, &PaintAlign::Left);
                assert_eq!(b, &PaintAlign::Center);
            }
            _ => panic!("ожидались Text-items"),
        }
    }

    /// Контракт G7: Painter — данные; конвертацию в инстансы рендера делает
    /// потребитель. Фиксируем, что items клонируемы и сравнимы (потребитель
    /// строит по ним CardInstance/OwnedText вне крейта).
    #[test]
    fn items_are_plain_data() {
        let mut p = Painter::new();
        p.rect(UiRect::new(0.0, 0.0, 10.0, 10.0), FILL, BORDER, 2.0);
        let snapshot = p.clone();
        assert_eq!(p, snapshot);
        let moved = p.items()[0].clone();
        assert_eq!(moved, snapshot.items()[0]);
    }

    /// clip_rect оборачивает вложенные items в ClipRect: rect — дословно,
    /// порядок детей = порядок вызовов внутри замыкания; соседние items
    /// идут в общем draw-порядке журнала (клип — один item).
    #[test]
    fn clip_rect_wraps_inner_items_in_call_order() {
        let mut p = Painter::new();
        p.rect(UiRect::new(0.0, 0.0, 100.0, 100.0), FILL, BORDER, 0.0);
        p.clip_rect(UiRect::new(10.0, 20.0, 50.0, 60.0), |inner| {
            inner.label(
                UiRect::new(12.0, 22.0, 40.0, 16.0),
                "В клипе",
                TEXT,
                12.0,
                PaintAlign::Left,
            );
            inner.rect(UiRect::new(14.0, 24.0, 8.0, 8.0), FILL, BORDER, 2.0);
        });
        p.label(
            UiRect::new(0.0, 0.0, 10.0, 10.0),
            "После",
            TEXT,
            12.0,
            PaintAlign::Left,
        );

        let items = p.items();
        assert_eq!(
            items.len(),
            3,
            "rect + ClipRect + label — в порядке вызовов"
        );
        assert!(matches!(items[0], PaintItem::Rect { .. }));
        assert!(matches!(items[2], PaintItem::Text { .. }));
        assert_eq!(
            items[1],
            PaintItem::ClipRect {
                rect: UiRect::new(10.0, 20.0, 50.0, 60.0),
                items: vec![
                    PaintItem::Text {
                        area: UiRect::new(12.0, 22.0, 40.0, 16.0),
                        text: "В клипе".to_owned(),
                        color: TEXT,
                        size: 12.0,
                        align: PaintAlign::Left,
                    },
                    PaintItem::Rect {
                        rect: UiRect::new(14.0, 24.0, 8.0, 8.0),
                        fill: FILL,
                        border: BORDER,
                        radius: 2.0,
                    },
                ],
            }
        );
    }

    /// Вложенные клипы (клип в клипе): структура дерева сохраняется дословно,
    /// `walk` даёт детерминированный DFS-порядок — Clip внешнего, Clip
    /// внутреннего, его дети, затем оставшиеся дети внешнего.
    #[test]
    fn nested_clips_keep_structure_and_walk_is_dfs() {
        let mut p = Painter::new();
        p.clip_rect(UiRect::new(0.0, 0.0, 100.0, 100.0), |outer| {
            outer.clip_rect(UiRect::new(10.0, 10.0, 40.0, 40.0), |inner| {
                inner.label(
                    UiRect::new(12.0, 12.0, 20.0, 10.0),
                    "Глубоко",
                    TEXT,
                    11.0,
                    PaintAlign::Left,
                );
            });
            outer.label(
                UiRect::new(50.0, 50.0, 30.0, 10.0),
                "Рядом",
                TEXT,
                11.0,
                PaintAlign::Left,
            );
        });

        // Структура: ClipRect(outer) → [ClipRect(inner) → [Text], Text]
        let items = p.items();
        assert_eq!(items.len(), 1, "вложенный клип — один корневой item");
        let PaintItem::ClipRect {
            rect: outer,
            items: outer_items,
        } = &items[0]
        else {
            panic!("ожидался ClipRect снаружи");
        };
        assert_eq!(*outer, UiRect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(outer_items.len(), 2);
        let PaintItem::ClipRect {
            rect: inner,
            items: inner_items,
        } = &outer_items[0]
        else {
            panic!("ожидался вложенный ClipRect");
        };
        assert_eq!(*inner, UiRect::new(10.0, 10.0, 40.0, 40.0));
        assert_eq!(inner_items.len(), 1);
        assert!(matches!(inner_items[0], PaintItem::Text { .. }));
        assert!(matches!(outer_items[1], PaintItem::Text { .. }));

        // walk: DFS draw-порядок
        let steps = walk(items);
        assert_eq!(steps.len(), 4, "2 клипа + 2 листа");
        assert_eq!(
            steps[0],
            ClipStep::Clip(UiRect::new(0.0, 0.0, 100.0, 100.0))
        );
        assert_eq!(
            steps[1],
            ClipStep::Clip(UiRect::new(10.0, 10.0, 40.0, 40.0))
        );
        assert!(matches!(
            steps[2],
            ClipStep::Item(PaintItem::Text { text, .. }) if text == "Глубоко"
        ));
        assert!(matches!(
            steps[3],
            ClipStep::Item(PaintItem::Text { text, .. }) if text == "Рядом"
        ));
    }

    /// take_items отдаёт ClipRect как ЕДИНЫЙ item (дети — внутри, не
    /// всплывают в журнал) и очищает журнал — потребитель конвертирует
    /// дерево клипов за один кадр.
    #[test]
    fn take_items_hands_clip_as_single_item_and_clears() {
        let mut p = Painter::new();
        p.clip_rect(UiRect::new(0.0, 0.0, 80.0, 40.0), |inner| {
            inner.rect(UiRect::new(2.0, 2.0, 10.0, 10.0), FILL, BORDER, 1.0);
        });
        p.rect(UiRect::new(90.0, 0.0, 10.0, 10.0), FILL, BORDER, 0.0);
        assert_eq!(p.items().len(), 2);

        let taken = p.take_items();
        assert_eq!(taken.len(), 2, "клип — один item, дети не всплывают");
        let PaintItem::ClipRect { rect, items } = &taken[0] else {
            panic!("ожидался ClipRect");
        };
        assert_eq!(*rect, UiRect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], PaintItem::Rect { .. }));
        // Журнал пуст после take
        assert!(p.items().is_empty());
        assert!(p.take_items().is_empty());
    }

    /// Контракт G7 (items_are_plain_data) распространяется на ClipRect:
    /// Clone/PartialEq выведены — потребитель клонирует/сравнивает деревья
    /// клипов как данные (дамп-тесты, конвертация в scissor FR-056).
    #[test]
    fn cliprect_is_plain_data() {
        let mut p = Painter::new();
        p.clip_rect(UiRect::new(1.0, 2.0, 30.0, 40.0), |inner| {
            inner.label(
                UiRect::new(3.0, 4.0, 20.0, 10.0),
                "К",
                TEXT,
                11.0,
                PaintAlign::Center,
            );
        });
        let snapshot = p.clone();
        assert_eq!(p, snapshot);
        let moved = p.items()[0].clone();
        assert_eq!(moved, snapshot.items()[0]);
        // PartialEq различает детей клипа (данные, а не синглтон)
        assert_ne!(
            moved,
            PaintItem::ClipRect {
                rect: UiRect::new(1.0, 2.0, 30.0, 40.0),
                items: Vec::new(),
            }
        );
    }

    /// FR-074: rotated оборачивает items в Transform с дословными
    /// deg/origin; rotated_centered берёт origin = центр области;
    /// вложенные порядок и payload сохранены.
    #[test]
    fn rotated_wraps_items_and_centered_takes_area_center() {
        let mut p = Painter::new();
        let area = UiRect::new(10.0, 20.0, 40.0, 20.0);
        p.rotated(45.0, UiPoint::new(5.0, 6.0), |inner| {
            inner.rect(UiRect::new(0.0, 0.0, 8.0, 8.0), FILL, BORDER, 1.0);
        });
        p.rotated_centered(area, -30.0, |inner| {
            inner.label(
                UiRect::new(12.0, 22.0, 20.0, 10.0),
                "Стрелка",
                TEXT,
                11.0,
                PaintAlign::Center,
            );
        });
        let items = p.items();
        assert_eq!(items.len(), 2, "два Transform-контейнера в порядке вызовов");
        assert_eq!(
            items[0],
            PaintItem::Transform {
                deg: 45.0,
                origin: UiPoint::new(5.0, 6.0),
                items: vec![PaintItem::Rect {
                    rect: UiRect::new(0.0, 0.0, 8.0, 8.0),
                    fill: FILL,
                    border: BORDER,
                    radius: 1.0,
                }],
            }
        );
        // origin центра: (10+40/2, 20+20/2) = (30, 30).
        assert_eq!(
            items[1],
            PaintItem::Transform {
                deg: -30.0,
                origin: UiPoint::new(30.0, 30.0),
                items: vec![PaintItem::Text {
                    area: UiRect::new(12.0, 22.0, 20.0, 10.0),
                    text: "Стрелка".to_owned(),
                    color: TEXT,
                    size: 11.0,
                    align: PaintAlign::Center,
                }],
            }
        );
    }

    /// FR-074: walk прозрачно спускается в Transform/ZGroup (не клипы) —
    /// дети в общем draw-порядке, без Clip-шага.
    #[test]
    fn walk_descends_into_transform_and_zgroup() {
        let mut p = Painter::new();
        p.clip_rect(UiRect::new(0.0, 0.0, 100.0, 100.0), |inner| {
            inner.rotated(90.0, UiPoint::new(50.0, 50.0), |rot| {
                rot.rect(UiRect::new(1.0, 1.0, 4.0, 4.0), FILL, BORDER, 0.0);
            });
            inner.z_group(2, |zg| {
                zg.label(
                    UiRect::new(2.0, 2.0, 10.0, 8.0),
                    "Z",
                    TEXT,
                    10.0,
                    PaintAlign::Left,
                );
            });
        });
        let steps = walk(p.items());
        assert_eq!(
            steps.len(),
            3,
            "Clip + (rect сквозь Transform) + (label сквозь ZGroup)"
        );
        assert_eq!(
            steps[0],
            ClipStep::Clip(UiRect::new(0.0, 0.0, 100.0, 100.0))
        );
        assert!(matches!(steps[1], ClipStep::Item(PaintItem::Rect { .. })));
        assert!(matches!(steps[2], ClipStep::Item(PaintItem::Text { .. })));
    }

    /// FR-074: z_group + take_items — стабильная сортировка по z:
    /// A(z=0), B(z=2), C(z=1) → draw-порядок A, C, B (CSS z-index).
    #[test]
    fn z_group_reorders_take_items_stably() {
        let mut p = Painter::new();
        let rect = |x| UiRect::new(x, 0.0, 10.0, 10.0);
        p.rect(rect(0.0), FILL, BORDER, 0.0); // A (z = 0)
        p.z_group(2, |g| {
            g.rect(rect(10.0), FILL, BORDER, 0.0); // B (z = 2)
        });
        p.z_group(1, |g| {
            g.rect(rect(20.0), FILL, BORDER, 0.0); // C (z = 1)
        });
        p.rect(rect(30.0), FILL, BORDER, 0.0); // D (z = 0, после групп)
        let items = p.take_items();
        // Стабильный порядок равных z: A, D остаются вокруг групп.
        let xs: Vec<f32> = items
            .iter()
            .map(|it| match it {
                PaintItem::Rect { rect, .. } => rect.x,
                PaintItem::ZGroup { items, .. } => match items[0] {
                    PaintItem::Rect { rect, .. } => rect.x,
                    _ => f32::NAN,
                },
                _ => f32::NAN,
            })
            .collect();
        assert_eq!(
            xs,
            vec![0.0, 30.0, 20.0, 10.0],
            "A, D (z=0), C (z=1), B (z=2)"
        );
        // Журнал очищен.
        assert!(p.items().is_empty());
    }

    /// FR-074: вложенные z_group — внутренний take сортирует свой
    /// контекст; внешний — свой (семантика CSS stacking context).
    #[test]
    fn nested_z_groups_sort_within_context() {
        let mut p = Painter::new();
        p.z_group(5, |outer| {
            outer.rect(UiRect::new(0.0, 0.0, 4.0, 4.0), FILL, BORDER, 0.0); // a (z0 внутри)
            outer.z_group(-1, |inner| {
                inner.rect(UiRect::new(8.0, 0.0, 4.0, 4.0), FILL, BORDER, 0.0); // b
            });
            outer.rect(UiRect::new(12.0, 0.0, 4.0, 4.0), FILL, BORDER, 0.0); // c (z0)
        });
        let items = p.take_items();
        assert_eq!(items.len(), 1);
        let PaintItem::ZGroup {
            z,
            items: inner_items,
        } = &items[0]
        else {
            panic!("ожидался ZGroup");
        };
        assert_eq!(*z, 5);
        // Внутри: a (0), ZGroup(-1) → b, c (0)? НЕТ: сортировка по z
        // ставит ZGroup(-1) ПЕРЕД z=0-элементами: b, a, c.
        let first = &inner_items[0];
        assert!(
            matches!(first, PaintItem::ZGroup { z: -1, .. }),
            "отрицательный z — глубже (раньше) в порядке отрисовки"
        );
        assert_eq!(inner_items.len(), 3);
    }
}
