//! FR-053 (U3 PRD-0009, F-7): layout-примитивы — immediate-mode функции
//! раскладки от слота родителя (PRD-0009 §7.4 V-5).
//!
//! Ничего не рисуют и не хранят состояния: каждый кадр потребитель
//! передаёт слот (родительский rect) и список детей — примитивы
//! возвращают вычисленные rect'ы. Интерфейс (слоты/constraints/align)
//! выбран совместимым со слотами taffy (PRD-0009 §7.4: эскалация на
//! taffy в v2 возможна без переписывания потребителей).
//!
//! # Политики переполнения
//!
//! `RowPolicy::Fit` — контент занимает слот; переполнение НЕ маскируется
//! (rect'ы выходят за слот) и ловится линтом/тестом G4 — молчаливый срез
//! (бывший `break`-кламп чипов галереи) становится видимым.
//!
//! `RowPolicy::SqueezeTail` — именованная деградация узкого слота: дети
//! получают `min(desired, остаток)`, хвост сжимается до нулевой ширины.
//! Вырожденные rect'ы невидимы и не пикаются (`UiRect::contains`
//! half-open) — дословная семантика бывшего замыкания `take` what-if
//! бара (FR-017/CR-015), вынесенная в именованную политику.

use crate::geometry::{EdgeInsets, UiRect, UiVec2};

/// Выравнивание по поперечной оси контейнера (вертикаль в `Row`,
/// горизонталь в `Column`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAlign {
    /// Прижать к началу поперечной оси.
    #[default]
    Start,
    /// Центрировать.
    Center,
    /// Прижать к концу поперечной оси.
    End,
}

/// Выравнивание по главной оси контейнера (распределение свободного места).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainAlign {
    /// Дети подряд от начала, зазор = `gap`.
    #[default]
    Start,
    /// Свободное место распределяется между детьми поровну
    /// (эффективный зазор = `gap` + доля свободного).
    SpaceBetween,
}

/// Политика переполнения главной оси [`Row`] (см. модульную доку).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowPolicy {
    /// Контент определяет занятость; переполнение слота не маскируется
    /// (ловится линтом G4).
    #[default]
    Fit,
    /// Деградация узкого слота: хвост сжимается до нулевой ширины.
    SqueezeTail,
}

/// Ребёнок линейного контейнера: фиксированный размер в ui px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Child {
    pub w: f32,
    pub h: f32,
}

impl Child {
    /// Ребёнок фиксированного размера.
    pub fn fixed(w: f32, h: f32) -> Self {
        Self {
            w: w.max(0.0),
            h: h.max(0.0),
        }
    }

    /// Распорка: занимает место по главной оси, нулевая высота
    /// (не является интерактивным/рисуемым элементом).
    pub fn spacer(len: f32) -> Self {
        Self {
            w: len.max(0.0),
            h: 0.0,
        }
    }
}

/// Горизонтальный контейнер (F-7 `Row{gap, align}` + политика переполнения).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Row {
    /// Базовый зазор между детьми (ui px; значения — spacing-scale
    /// `design/tokens/dimensions.json`, источник значений — у вызова).
    pub gap: f32,
    pub main: MainAlign,
    pub cross: CrossAlign,
    pub policy: RowPolicy,
}

impl Row {
    /// Раскладка детей в слоте: возвращает rect'ы (параллельно `items`).
    pub fn lay_out(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        match self.policy {
            RowPolicy::Fit => self.fit(slot, items),
            RowPolicy::SqueezeTail => self.squeeze_tail(slot, items),
        }
    }

    /// `Fit`: дети подряд с зазором; свободное место — по `main`.
    fn fit(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let n = items.len();
        let total: f32 = items.iter().map(|c| c.w).sum();
        let gaps_total = self.gap * n.saturating_sub(1) as f32;
        let extra = (slot.w - total - gaps_total).max(0.0);
        let gap = match self.main {
            MainAlign::Start => self.gap,
            // SpaceBetween: при n>1 свободное место добавляется к зазорам
            MainAlign::SpaceBetween if n > 1 => self.gap + extra / (n - 1) as f32,
            MainAlign::SpaceBetween => self.gap,
        };
        let mut x = slot.x;
        items
            .iter()
            .map(|c| {
                let rect = UiRect::new(x, self.cross_y(slot, c.h), c.w, c.h);
                x += c.w + gap;
                rect
            })
            .collect()
    }

    /// `SqueezeTail`: каждый ребёнок получает `min(desired, остаток)`;
    /// хвост сжимается до нуля, за правый край слота ничего не выходит.
    fn squeeze_tail(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let mut x = slot.x;
        let limit = slot.right();
        items
            .iter()
            .map(|c| {
                let remaining = (limit - x).max(0.0);
                let w = c.w.min(remaining);
                let rect = UiRect::new(x.min(limit), self.cross_y(slot, c.h), w, c.h);
                x += w + self.gap;
                rect
            })
            .collect()
    }

    fn cross_y(&self, slot: UiRect, h: f32) -> f32 {
        match self.cross {
            CrossAlign::Start => slot.y,
            CrossAlign::Center => slot.y + (slot.h - h).max(0.0) / 2.0,
            CrossAlign::End => slot.y + (slot.h - h).max(0.0),
        }
    }
}

/// Вертикальный контейнер (F-7 `Column{gap, align}`); политика — только
/// `Fit` (вертикальная деградация пилотов выражается размером окна
/// видимости строк, а не сжатием хвоста).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Column {
    pub gap: f32,
    pub main: MainAlign,
    pub cross: CrossAlign,
}

impl Column {
    /// Раскладка детей в слоте: возвращает rect'ы (параллельно `items`).
    pub fn lay_out(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let n = items.len();
        let total: f32 = items.iter().map(|c| c.h).sum();
        let gaps_total = self.gap * n.saturating_sub(1) as f32;
        let extra = (slot.h - total - gaps_total).max(0.0);
        let gap = match self.main {
            MainAlign::Start => self.gap,
            MainAlign::SpaceBetween if n > 1 => self.gap + extra / (n - 1) as f32,
            MainAlign::SpaceBetween => self.gap,
        };
        let mut y = slot.y;
        items
            .iter()
            .map(|c| {
                let rect = UiRect::new(self.cross_x(slot, c.w), y, c.w, c.h);
                y += c.h + gap;
                rect
            })
            .collect()
    }

    fn cross_x(&self, slot: UiRect, w: f32) -> f32 {
        match self.cross {
            CrossAlign::Start => slot.x,
            CrossAlign::Center => slot.x + (slot.w - w).max(0.0) / 2.0,
            CrossAlign::End => slot.x + (slot.w - w).max(0.0),
        }
    }
}

/// Выравнивание фиксированного блока в слоте по горизонтали/вертикали
/// (F-7 `Stack{align}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Вертикальное выравнивание фиксированного блока в слоте.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    #[default]
    Start,
    Center,
    End,
}

/// Разместить блок фиксированного размера в слоте (F-7 `Stack{align}`):
/// панель галереи по центру вьюпорта, empty-карточка и т.п.
pub fn stack(slot: UiRect, size: UiVec2, h: HAlign, v: VAlign) -> UiRect {
    let x = match h {
        HAlign::Start => slot.x,
        HAlign::Center => slot.x + (slot.w - size.x).max(0.0) / 2.0,
        HAlign::End => slot.x + (slot.w - size.x).max(0.0),
    };
    let y = match v {
        VAlign::Start => slot.y,
        VAlign::Center => slot.y + (slot.h - size.y).max(0.0) / 2.0,
        VAlign::End => slot.y + (slot.h - size.y).max(0.0),
    };
    UiRect::new(x, y, size.x.max(0.0), size.y.max(0.0))
}

/// Кламп размера в min/max границы (F-7 `Constrain(min/max)`); нижняя
/// граница приоритетна над верхней (`max` срезается до `min` —
/// противоречивые границы дают `min`).
pub fn constrain(min: UiVec2, max: UiVec2, desired: UiVec2) -> UiVec2 {
    let lo_x = min.x.max(0.0);
    let lo_y = min.y.max(0.0);
    let hi_x = max.x.max(lo_x);
    let hi_y = max.y.max(lo_y);
    UiVec2::new(desired.x.clamp(lo_x, hi_x), desired.y.clamp(lo_y, hi_y))
}

/// Сжать слот на отступы (F-7 `Padding`); обёртка [`UiRect::inset`]
/// (отрицательный остаток — пустой rect, не вырожденный сдвиг).
pub fn pad(slot: UiRect, e: EdgeInsets) -> UiRect {
    slot.inset(&e)
}

/// Escape-hatch экзотики (R-3 PRD-0009): прямая геометрия вне примитивов.
/// Каждое использование обязано нести комментарий-обоснование (почему
/// примитивы не выражают раскладку) — попадает в grep-аудит G8.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Custom(pub UiRect);

impl From<Custom> for UiRect {
    fn from(c: Custom) -> UiRect {
        c.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::UiPoint;

    fn slot() -> UiRect {
        UiRect::new(100.0, 50.0, 300.0, 200.0)
    }

    #[test]
    fn row_fit_places_items_with_gap() {
        let r = Row {
            gap: 6.0,
            ..Row::default()
        };
        let rects = r.lay_out(
            slot(),
            &[Child::fixed(80.0, 30.0), Child::fixed(40.0, 30.0)],
        );
        assert_eq!(rects[0], UiRect::new(100.0, 50.0, 80.0, 30.0));
        assert_eq!(rects[1], UiRect::new(186.0, 50.0, 40.0, 30.0));
    }

    #[test]
    fn row_cross_aligns() {
        let items = [Child::fixed(50.0, 20.0)];
        let start = Row {
            gap: 0.0,
            cross: CrossAlign::Start,
            ..Row::default()
        }
        .lay_out(slot(), &items)[0];
        assert_eq!(start.y, 50.0);
        let center = Row {
            gap: 0.0,
            cross: CrossAlign::Center,
            ..Row::default()
        }
        .lay_out(slot(), &items)[0];
        assert_eq!(center.y, 50.0 + (200.0 - 20.0) / 2.0);
        let end = Row {
            gap: 0.0,
            cross: CrossAlign::End,
            ..Row::default()
        }
        .lay_out(slot(), &items)[0];
        assert_eq!(end.y, 50.0 + 200.0 - 20.0);
    }

    #[test]
    fn row_space_between_distributes_extra() {
        let r = Row {
            gap: 0.0,
            main: MainAlign::SpaceBetween,
            ..Row::default()
        };
        // Слот 300, дети 80+40: свободных 180 → зазор 180
        let rects = r.lay_out(
            slot(),
            &[Child::fixed(80.0, 20.0), Child::fixed(40.0, 20.0)],
        );
        assert_eq!(rects[0].x, 100.0);
        assert_eq!(rects[1].x, 100.0 + 80.0 + 180.0);
    }

    /// SqueezeTail — дословная семантика бывшего `take`: каждый элемент
    /// получает min(desired, остаток); хвост — нулевой ширины; за слот
    /// ничего не выходит.
    #[test]
    fn row_squeeze_tail_matches_take_semantics() {
        let r = Row {
            gap: 6.0,
            policy: RowPolicy::SqueezeTail,
            ..Row::default()
        };
        let small = UiRect::new(0.0, 0.0, 100.0, 30.0);
        let rects = r.lay_out(
            small,
            &[
                Child::fixed(60.0, 30.0),
                Child::fixed(60.0, 30.0),
                Child::fixed(60.0, 30.0),
            ],
        );
        // Первый: 60 (0..60), x → 66; второй: min(60, 34) = 34 (66..100),
        // x → 106 (за слот); третий: остаток 0.
        assert_eq!(rects[0].w, 60.0);
        assert_eq!(rects[1].w, 34.0);
        assert_eq!(rects[2].w, 0.0);
        for rect in &rects {
            assert!(rect.right() <= small.right() + f32::EPSILON);
            assert!(rect.x <= small.right());
        }
        // Вырожденный (нулевой) rect — пустой: не пикается (контракт
        // невидимости сжатого хвоста).
        assert!(rects[2].is_empty());
        assert!(!rects[2].contains(UiPoint::new(rects[2].x, rects[2].y)));
    }

    #[test]
    fn column_places_items_with_gap_and_cross() {
        let c = Column {
            gap: 6.0,
            cross: CrossAlign::Center,
            ..Column::default()
        };
        let rects = c.lay_out(
            slot(),
            &[Child::fixed(100.0, 40.0), Child::fixed(100.0, 30.0)],
        );
        assert_eq!(rects[0], UiRect::new(200.0, 50.0, 100.0, 40.0));
        assert_eq!(rects[1], UiRect::new(200.0, 96.0, 100.0, 30.0));
    }

    #[test]
    fn stack_centers_fixed_block() {
        let panel = stack(
            slot(),
            UiVec2::new(100.0, 60.0),
            HAlign::Center,
            VAlign::Center,
        );
        assert_eq!(panel, UiRect::new(200.0, 120.0, 100.0, 60.0));
        let bottom_right = stack(slot(), UiVec2::new(100.0, 60.0), HAlign::End, VAlign::End);
        assert_eq!(bottom_right, UiRect::new(300.0, 190.0, 100.0, 60.0));
    }

    #[test]
    fn constrain_clamps_both_bounds() {
        let min = UiVec2::new(40.0, 20.0);
        let max = UiVec2::new(120.0, 80.0);
        assert_eq!(
            constrain(min, max, UiVec2::new(500.0, 5.0)),
            UiVec2::new(120.0, 20.0),
            "верхний кламп и приоритет min"
        );
        // Противоречивые границы: max < min → срезается до min.
        assert_eq!(
            constrain(min, UiVec2::new(10.0, 10.0), UiVec2::new(5.0, 5.0)),
            UiVec2::new(40.0, 20.0)
        );
    }

    #[test]
    fn pad_shrinks_slot() {
        let inner = pad(
            slot(),
            EdgeInsets {
                left: 12.0,
                top: 10.0,
                right: 12.0,
                bottom: 10.0,
            },
        );
        assert_eq!(inner, UiRect::new(112.0, 60.0, 276.0, 180.0));
        // Отступы больше слота — пустой rect (не отрицательный).
        assert!(pad(UiRect::new(0.0, 0.0, 4.0, 4.0), EdgeInsets::uniform(10.0)).is_empty());
    }

    #[test]
    fn spacer_takes_room_without_height() {
        let r = Row {
            gap: 0.0,
            ..Row::default()
        };
        let rects = r.lay_out(
            slot(),
            &[
                Child::fixed(50.0, 30.0),
                Child::spacer(40.0),
                Child::fixed(50.0, 30.0),
            ],
        );
        assert_eq!(rects[1], UiRect::new(150.0, 50.0, 40.0, 0.0));
        assert_eq!(rects[2].x, 190.0);
    }
}
