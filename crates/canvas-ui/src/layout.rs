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
//!
//! `RowPolicy::Wrap { max_rows }` (FR-062 F-15) — жадная упаковка детей
//! по строкам слота: ребёнок не влез в строку — перенос; высота строки =
//! max высот детей строки; строк сверх `max_rows` НЕ маскируются —
//! выходят за нижний край слота (ловится линтом G4), как `Fit`.
//!
//! # Flex-факторы (FR-062 F-14)
//!
//! `Child.grow` — доля свободного места слота (после фиксированных
//! детей, базовых размеров flex-детей и зазоров), распределяемая
//! пропорционально `grow` (CSS flex-grow при flex-basis = базовый
//! размер). `MainAlign::End` — дети прижаты к концу главной оси.
//! Приоритеты: деградация `SqueezeTail` побеждает распределение (grow
//! игнорируется); grow поглощает свободное место — при Σgrow > 0 и
//! свободном месте `SpaceBetween`/`End` деградируют к `Start`/базовому
//! gap. `0.0` — фиксированный ребёнок (байт-в-байт прежнее поведение).
//!
//! # Measured-дети (FR-062 F-13)
//!
//! [`MeasuredItem`] — размер от контента внутри раскладки: ширина/высота
//! текстового ребёнка берутся из [`crate::measure::TextMeasurer`] (тот
//! же шейпинг, что у рендера) — ручная проводка `width_of →
//! Child::fixed` не нужна. Переполнение НЕ маскируется (наследует
//! политику ряда); молчаливого клампа в остаток слота нет — ограничение
//! ширины задаёт вызов явно (`max_w`), отрисовка усечения — ellipsis
//! потребителя (класс CR-015 защищён по построению: ширина не может
//! быть эвристикой — measurer в сигнатуре).

use crate::geometry::{EdgeInsets, UiRect, UiVec2};
use crate::measure::TextMeasurer;

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
    /// Дети подряд от конца главной оси (FR-062 F-14); зазор = `gap`.
    /// При детях с `grow` свободное место съедается распределением —
    /// `End` вырождается в `Start` (документировано в модуле).
    End,
}

/// Политика переполнения главной оси [`Row`] (см. модульную доку).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowPolicy {
    /// Контент определяет занятость; переполнение слота не маскируется
    /// (ловится линтом G4).
    #[default]
    Fit,
    /// Деградация узкого слота: хвост сжимается до нулевой ширины.
    /// Приоритетна над flex-факторами: `grow` детей игнорируется
    /// (FR-062 F-14).
    SqueezeTail,
    /// Жадная упаковка по строкам (FR-062 F-15, уточнение контракта:
    /// поле `max_rows` убрано — число видимых строк определяет высота
    /// слота; строки ниже слота не маскируются — естественно выходят за
    /// нижний край, что и ловит линт G4).
    Wrap,
}

/// Ребёнок линейного контейнера: размер в ui px + flex-фактор
/// (FR-062 F-14; все места создания — через конструкторы: grep-аудит
/// `Child {` литералов = 0 вне крейта — добавление поля безопасно).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Child {
    pub w: f32,
    pub h: f32,
    /// Flex-фактор роста по главной оси контейнера (ширина в `Row`,
    /// высота в `Column`): доля свободного места слота, пропорционально
    /// `grow`. `0.0` — фиксированный (текущее поведение). В `SqueezeTail`
    /// игнорируется (деградация приоритетна).
    pub grow: f32,
}

impl Child {
    /// Ребёнок фиксированного размера (grow = 0 — прежнее поведение).
    pub fn fixed(w: f32, h: f32) -> Self {
        Self {
            w: w.max(0.0),
            h: h.max(0.0),
            grow: 0.0,
        }
    }

    /// Распорка: занимает место по главной оси, нулевая высота
    /// (не является интерактивным/рисуемым элементом).
    pub fn spacer(len: f32) -> Self {
        Self {
            w: len.max(0.0),
            h: 0.0,
            grow: 0.0,
        }
    }

    /// Flex-ребёнок (FR-062 F-14): `w`/`h` — базовый размер (flex-basis),
    /// `grow` — доля свободного места главной оси (пропорционально;
    /// отрицательный прижимается к 0).
    pub fn flexible(w: f32, h: f32, grow: f32) -> Self {
        Self {
            w: w.max(0.0),
            h: h.max(0.0),
            grow: grow.max(0.0),
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
            RowPolicy::Wrap => self.wrap(slot, items),
        }
    }

    /// `Fit`: дети подряд с зазором; свободное место — flex-детям по
    /// `grow` (FR-062 F-14), остаток — по `main`.
    fn fit(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let sizes = self.resolve_grow(slot.w, items);
        let n = items.len();
        let total: f32 = sizes.iter().sum();
        let gaps_total = self.gap * n.saturating_sub(1) as f32;
        let extra = (slot.w - total - gaps_total).max(0.0);
        let gap = match self.main {
            MainAlign::Start => self.gap,
            // SpaceBetween: при n>1 свободное место добавляется к зазорам
            MainAlign::SpaceBetween if n > 1 => self.gap + extra / (n - 1) as f32,
            MainAlign::SpaceBetween => self.gap,
            MainAlign::End => self.gap,
        };
        let mut x = match self.main {
            // End: дети прижаты к правому краю (свободное место — слева);
            // при grow свободное место уже съедено — x = slot.x
            MainAlign::End => (slot.right() - total - gaps_total).max(slot.x),
            _ => slot.x,
        };
        items
            .iter()
            .zip(&sizes)
            .map(|(c, &w)| {
                let rect = UiRect::new(x, self.cross_y(slot, c.h), w, c.h);
                x += w + gap;
                rect
            })
            .collect()
    }

    /// Распределение свободного места слота по `grow` (FR-062 F-14):
    /// свободное = slot.w − Σ(базовые ширины) − Σ(зазоры); flex-дети
    /// получают базу + долю свободного (пропорционально grow). Ширины
    /// фиксированных детей не меняются. Σgrow = 0 или свободного нет —
    /// базовые ширины (байт-в-байт прежнее поведение).
    fn resolve_grow(&self, slot_w: f32, items: &[Child]) -> Vec<f32> {
        let mut sizes: Vec<f32> = items.iter().map(|c| c.w).collect();
        let grow_total: f32 = items.iter().map(|c| c.grow).sum();
        if grow_total <= 0.0 {
            return sizes;
        }
        let basis: f32 = sizes.iter().sum();
        let gaps_total = self.gap * items.len().saturating_sub(1) as f32;
        let free = (slot_w - basis - gaps_total).max(0.0);
        if free <= 0.0 {
            return sizes;
        }
        for (size, child) in sizes.iter_mut().zip(items) {
            if child.grow > 0.0 {
                *size += free * child.grow / grow_total;
            }
        }
        sizes
    }

    /// `SqueezeTail`: каждый ребёнок получает `min(desired, остаток)`;
    /// хвост сжимается до нуля, за правый край слота ничего не выходит.
    /// Деградация приоритетна: `grow` игнорируется (FR-062 F-14).
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

    /// Жадная упаковка по строкам (FR-062 F-15, уточнение контракта:
    /// `max_rows` поле убрано — число видимых строк определяет ВЫСОТА
    /// слота; строки ниже слота не маскируются — естественно выходят за
    /// нижний край, что и ловит линт G4; без мёртвого параметра).
    /// Высота строки = max высот детей строки; зазоры: `gap` — по
    /// горизонтали, тот же `gap` — между строками; поперечное
    /// выравнивание — внутри строки (по её высоте).
    fn wrap(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let mut rects = vec![UiRect::default(); items.len()];
        let mut row: Vec<(usize, &Child)> = Vec::new();
        let mut row_w = 0.0;
        let mut row_h = 0.0;
        let mut row_y = slot.y;
        let mut flush =
            |row: &mut Vec<(usize, &Child)>, row_w: &mut f32, row_h: &mut f32, row_y: &mut f32| {
                let mut x = slot.x;
                let h = *row_h;
                for (i, c) in row.drain(..) {
                    let y = match self.cross {
                        CrossAlign::Start => *row_y,
                        // поперечное выравнивание внутри строки (по её высоте)
                        CrossAlign::Center => *row_y + (h - c.h).max(0.0) / 2.0,
                        CrossAlign::End => *row_y + (h - c.h).max(0.0),
                    };
                    rects[i] = UiRect::new(x, y, c.w, c.h);
                    x += c.w + self.gap;
                }
                *row_y += h + self.gap;
                *row_w = 0.0;
                *row_h = 0.0;
            };
        for (i, c) in items.iter().enumerate() {
            let next_w = if row.is_empty() {
                c.w
            } else {
                row_w + self.gap + c.w
            };
            // ребёнок шире слота — строка из одного (переполнение вправо
            // видно, как в Fit); непоместившийся в строку — перенос
            if !row.is_empty() && next_w > slot.w + f32::EPSILON {
                flush(&mut row, &mut row_w, &mut row_h, &mut row_y);
            }
            row.push((i, c));
            row_w = if row.len() == 1 {
                c.w
            } else {
                row_w + self.gap + c.w
            };
            row_h = row_h.max(c.h);
        }
        if !row.is_empty() {
            flush(&mut row, &mut row_w, &mut row_h, &mut row_y);
        }
        rects
    }

    /// FR-062 F-13: раскладка measured-детей (размер от контента —
    /// через [`TextMeasurer`]); та же политика, что [`Row::lay_out`]
    /// (в т.ч. `Wrap`); flex-факторы у measured-детей не задаются
    /// (сигнатура F-13 минимальна — фиксированные размеры + текст).
    pub fn lay_out_measured(
        &self,
        slot: UiRect,
        items: &[MeasuredItem],
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        family: &str,
        size: f32,
    ) -> Vec<UiRect> {
        let children: Vec<Child> = items
            .iter()
            .map(|item| item.resolve(m, fs, family, size))
            .collect();
        self.lay_out(slot, &children)
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
/// видимости строк, а не сжатием хвоста). FR-062 F-14: flex-факторы
/// (`Child.grow` — по высоте) и `MainAlign::End` поддержаны.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Column {
    pub gap: f32,
    pub main: MainAlign,
    pub cross: CrossAlign,
}

impl Column {
    /// Раскладка детей в слоте: возвращает rect'ы (параллельно `items`).
    pub fn lay_out(&self, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        let sizes = self.resolve_grow(slot.h, items);
        let n = items.len();
        let total: f32 = sizes.iter().sum();
        let gaps_total = self.gap * n.saturating_sub(1) as f32;
        let extra = (slot.h - total - gaps_total).max(0.0);
        let gap = match self.main {
            MainAlign::Start => self.gap,
            MainAlign::SpaceBetween if n > 1 => self.gap + extra / (n - 1) as f32,
            MainAlign::SpaceBetween => self.gap,
            MainAlign::End => self.gap,
        };
        let mut y = match self.main {
            MainAlign::End => (slot.bottom() - total - gaps_total).max(slot.y),
            _ => slot.y,
        };
        items
            .iter()
            .zip(&sizes)
            .map(|(c, &h)| {
                let rect = UiRect::new(self.cross_x(slot, c.w), y, c.w, h);
                y += h + gap;
                rect
            })
            .collect()
    }

    /// Распределение свободного места слота по `grow` по высоте
    /// (FR-062 F-14; симметрично [`Row::resolve_grow`]).
    fn resolve_grow(&self, slot_h: f32, items: &[Child]) -> Vec<f32> {
        let mut sizes: Vec<f32> = items.iter().map(|c| c.h).collect();
        let grow_total: f32 = items.iter().map(|c| c.grow).sum();
        if grow_total <= 0.0 {
            return sizes;
        }
        let basis: f32 = sizes.iter().sum();
        let gaps_total = self.gap * items.len().saturating_sub(1) as f32;
        let free = (slot_h - basis - gaps_total).max(0.0);
        if free <= 0.0 {
            return sizes;
        }
        for (size, child) in sizes.iter_mut().zip(items) {
            if child.grow > 0.0 {
                *size += free * child.grow / grow_total;
            }
        }
        sizes
    }

    fn cross_x(&self, slot: UiRect, w: f32) -> f32 {
        match self.cross {
            CrossAlign::Start => slot.x,
            CrossAlign::Center => slot.x + (slot.w - w).max(0.0) / 2.0,
            CrossAlign::End => slot.x + (slot.w - w).max(0.0),
        }
    }
}

/// Ребёнок measured-раскладки (FR-062 F-13): размер от контента.
///
/// Ширина/высота текстового ребёнка берутся из [`TextMeasurer`] (тот же
/// шейпинг, что у screen-текстов рендера); ограничение ширины — только
/// явное (`max_w`) — молчаливого клампа в остаток слота нет (G5):
/// усечение подписи — ellipsis потребителя по той же измеренной ширине.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeasuredItem<'a> {
    /// Фиксированный размер (эквивалент [`Child::fixed`]) — оракул
    /// эквивалентности с ручной проводкой `width_of → Child::fixed`.
    Fixed { w: f32, h: f32 },
    /// Размер от текста: ширина = `width_of` (кламп в `max_w`, если
    /// задан, и подъём до `min_w`), высота = измеренная (строки · кегль ·
    /// `SCREEN_LINE_FACTOR`).
    Text {
        text: &'a str,
        /// Явный потолок ширины (ellipsis — решение потребителя).
        max_w: Option<f32>,
        /// Пол ширины (например, пад чипа) — шире текста не будет уже.
        min_w: f32,
    },
    /// Распорка (эквивалент [`Child::spacer`]).
    Spacer(f32),
}

impl MeasuredItem<'_> {
    /// Разрешить в фиксированного [`Child`] измерениями (точка интеграции
    /// TextMeasurer; ширины/высоты — реальные, не эвристика).
    pub fn resolve(
        &self,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        family: &str,
        size: f32,
    ) -> Child {
        match *self {
            MeasuredItem::Fixed { w, h } => Child::fixed(w, h),
            MeasuredItem::Spacer(len) => Child::spacer(len),
            MeasuredItem::Text { text, max_w, min_w } => {
                let spec = crate::measure::TextSpec {
                    text,
                    family,
                    size,
                    max_width: f32::INFINITY,
                };
                let measured = m.measure(fs, &spec);
                let mut w = measured.width.max(min_w);
                if let Some(max) = max_w {
                    w = w.min(max.max(min_w));
                }
                Child::fixed(w, measured.height)
            }
        }
    }
}

/// 2D-сетка равных явных колонок (FR-062 F-16): row-major ячейки
/// `cols.len() · rows` от слота; ширины колонок заданы вызовом
/// (например, из [`crate::row_guides::RowGuides`] — направляющие
/// табличного тела), высота строки общая. Спаны/авто-треки — вне
/// контракта (территория taffy по триггерам ADR-0013 T2).
pub fn grid_cells(slot: UiRect, cols: &[f32], rows: usize, row_h: f32, gap: UiVec2) -> Vec<UiRect> {
    let mut rects = Vec::with_capacity(cols.len().saturating_mul(rows));
    let row_h = row_h.max(0.0);
    for r in 0..rows {
        let y = slot.y + r as f32 * (row_h + gap.y.max(0.0));
        let mut x = slot.x;
        for &w in cols {
            let w = w.max(0.0);
            rects.push(UiRect::new(x, y, w, row_h));
            x += w + gap.x.max(0.0);
        }
    }
    rects
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

    // === FR-062 F-14: flex-факторы ===

    /// Свободное место распределяется пропорционально grow (2:1);
    /// сумма ширин + зазоры = слот ровно.
    #[test]
    fn row_grow_distributes_free_space_proportionally() {
        let r = Row {
            gap: 6.0,
            ..Row::default()
        };
        let slot = slot(); // 300
        let rects = r.lay_out(
            slot,
            &[
                Child::fixed(80.0, 30.0),
                Child::flexible(40.0, 30.0, 2.0),
                Child::flexible(40.0, 30.0, 1.0),
            ],
        );
        // basis 160 + зазоры 12 → свободных 128: 2/3 и 1/3
        approx(rects[0].w, 80.0);
        approx(rects[1].w, 40.0 + 128.0 * 2.0 / 3.0);
        approx(rects[2].w, 40.0 + 128.0 / 3.0);
        approx(rects[0].x, 100.0);
        approx(rects[1].x, 186.0);
        approx(rects[2].x, 186.0 + rects[1].w + 6.0);
        approx(rects[2].right(), slot.right());
    }

    /// grow = 0 — байт-в-байт прежнее поведение (инвариант «только
    /// добавление» волны 2).
    #[test]
    fn row_zero_grow_is_byte_identical_to_fixed() {
        let r = Row {
            gap: 6.0,
            cross: CrossAlign::Center,
            ..Row::default()
        };
        let items = [Child::fixed(80.0, 30.0), Child::fixed(40.0, 30.0)];
        let old = r.lay_out(slot(), &items);
        let new = r.lay_out(
            slot(),
            &[
                Child::flexible(80.0, 30.0, 0.0),
                Child::flexible(40.0, 30.0, 0.0),
            ],
        );
        assert_eq!(old, new);
    }

    /// grow поглощает свободное место — SpaceBetween деградирует к базовому
    /// зазору (документированный приоритет).
    #[test]
    fn row_space_between_degenerates_with_grow() {
        let r = Row {
            gap: 6.0,
            main: MainAlign::SpaceBetween,
            ..Row::default()
        };
        let slot = slot(); // 300
        let rects = r.lay_out(
            slot,
            &[
                Child::flexible(40.0, 30.0, 1.0),
                Child::flexible(40.0, 30.0, 1.0),
            ],
        );
        // свободных 214 → каждый 147; зазор — базовый 6
        approx(rects[0].w, 147.0);
        approx(rects[1].w, 147.0);
        approx(rects[1].x - rects[0].right(), 6.0);
        approx(rects[1].right(), slot.right());
    }

    /// MainAlign::End — дети прижаты к правому краю слота.
    #[test]
    fn row_end_aligns_children_to_slot_end() {
        let r = Row {
            gap: 6.0,
            main: MainAlign::End,
            ..Row::default()
        };
        let slot = slot(); // 300, right 400
        let rects = r.lay_out(slot, &[Child::fixed(80.0, 30.0), Child::fixed(40.0, 30.0)]);
        approx(rects[0].x, 274.0);
        approx(rects[1].x, 360.0);
        approx(rects[1].right(), 400.0);
    }

    /// Column: grow распределяет вертикальное свободное место.
    #[test]
    fn column_grow_distributes_vertical_free_space() {
        let c = Column {
            gap: 6.0,
            ..Column::default()
        };
        let slot = slot(); // h 200, bottom 250
        let rects = c.lay_out(
            slot,
            &[
                Child::flexible(50.0, 40.0, 1.0),
                Child::flexible(50.0, 40.0, 1.0),
            ],
        );
        // свободных 200−80−6 = 114 → каждый 97
        approx(rects[0].h, 97.0);
        approx(rects[1].h, 97.0);
        approx(rects[1].bottom(), 250.0);
    }

    /// Деградация приоритетна: SqueezeTail игнорирует grow.
    #[test]
    fn squeeze_tail_ignores_grow() {
        let fixed = Row {
            gap: 6.0,
            policy: RowPolicy::SqueezeTail,
            ..Row::default()
        }
        .lay_out(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            &[Child::fixed(60.0, 30.0), Child::fixed(60.0, 30.0)],
        );
        let flex = Row {
            gap: 6.0,
            policy: RowPolicy::SqueezeTail,
            ..Row::default()
        }
        .lay_out(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            &[
                Child::flexible(60.0, 30.0, 10.0),
                Child::flexible(60.0, 30.0, 5.0),
            ],
        );
        assert_eq!(fixed, flex);
    }

    // === FR-062 F-15: Wrap ===

    /// Не влез в строку — перенос.
    #[test]
    fn wrap_moves_overflow_to_next_row() {
        let r = Row {
            gap: 10.0,
            policy: RowPolicy::Wrap,
            ..Row::default()
        };
        let slot = UiRect::new(0.0, 0.0, 100.0, 80.0);
        let rects = r.lay_out(
            slot,
            &[
                Child::fixed(40.0, 30.0),
                Child::fixed(40.0, 30.0),
                Child::fixed(40.0, 30.0),
            ],
        );
        assert_eq!(rects[0], UiRect::new(0.0, 0.0, 40.0, 30.0));
        assert_eq!(rects[1], UiRect::new(50.0, 0.0, 40.0, 30.0));
        // вторая строка: y = 30 + 10 (gap и по вертикали)
        assert_eq!(rects[2], UiRect::new(0.0, 40.0, 40.0, 30.0));
    }

    /// Высота строки = max высот детей строки; cross — внутри строки.
    #[test]
    fn wrap_row_height_is_max_of_children() {
        let r = Row {
            gap: 10.0,
            policy: RowPolicy::Wrap,
            ..Row::default()
        };
        let slot = UiRect::new(0.0, 0.0, 100.0, 200.0);
        let rects = r.lay_out(
            slot,
            &[
                Child::fixed(40.0, 30.0),
                Child::fixed(40.0, 20.0),
                Child::fixed(40.0, 50.0),
            ],
        );
        assert_eq!(rects[0], UiRect::new(0.0, 0.0, 40.0, 30.0));
        assert_eq!(rects[1], UiRect::new(50.0, 0.0, 40.0, 20.0));
        assert_eq!(rects[2], UiRect::new(0.0, 40.0, 40.0, 50.0));
    }

    /// Строки ниже слота НЕ маскируются — выходят за нижний край
    /// (ловится линтом G4, как Fit).
    #[test]
    fn wrap_overflow_below_slot_is_visible() {
        let r = Row {
            gap: 10.0,
            policy: RowPolicy::Wrap,
            ..Row::default()
        };
        let slot = UiRect::new(0.0, 0.0, 100.0, 40.0);
        let rects = r.lay_out(
            slot,
            &[
                Child::fixed(40.0, 30.0),
                Child::fixed(40.0, 30.0),
                Child::fixed(40.0, 30.0),
            ],
        );
        assert!(rects[2].y >= slot.bottom(), "строка за слотом видима");
    }

    /// Ребёнок шире слота — строка из одного (переполнение вправо видно).
    #[test]
    fn wrap_single_wider_than_slot_stays_in_own_row() {
        let r = Row {
            gap: 10.0,
            policy: RowPolicy::Wrap,
            ..Row::default()
        };
        let slot = UiRect::new(0.0, 0.0, 100.0, 200.0);
        let rects = r.lay_out(slot, &[Child::fixed(200.0, 30.0), Child::fixed(40.0, 30.0)]);
        assert_eq!(rects[0], UiRect::new(0.0, 0.0, 200.0, 30.0));
        assert_eq!(rects[1], UiRect::new(0.0, 40.0, 40.0, 30.0));
    }

    // === FR-062 F-13: measured-дети ===

    /// Оракул эквивалентности: lay_out_measured == ручная проводка
    /// width_of → Child::fixed → lay_out (те же rect'ы дословно).
    #[test]
    fn measured_row_matches_manual_fixed_oracle() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = TextMeasurer::new();
        let family = "Noto Sans Display";
        let labels = ["50 rps", "1000 запр/с", "Σ"];
        // Оракул: ручная проводка (как до F-13)
        let manual_widths: Vec<f32> = labels
            .iter()
            .map(|l| m.width_of(&mut fs, l, family, 13.0))
            .collect();
        let manual = Row {
            gap: 8.0,
            ..Row::default()
        }
        .lay_out(
            slot(),
            &[
                Child::fixed(manual_widths[0], 18.0),
                Child::fixed(manual_widths[1], 18.0),
                Child::fixed(manual_widths[2], 18.0),
            ],
        );
        // F-13: тот же ряд через measured-детей
        let measured = Row {
            gap: 8.0,
            ..Row::default()
        }
        .lay_out_measured(
            slot(),
            &[
                MeasuredItem::Text {
                    text: labels[0],
                    max_w: None,
                    min_w: 0.0,
                },
                MeasuredItem::Text {
                    text: labels[1],
                    max_w: None,
                    min_w: 0.0,
                },
                MeasuredItem::Text {
                    text: labels[2],
                    max_w: None,
                    min_w: 0.0,
                },
            ],
            &mut m,
            &mut fs,
            family,
            13.0,
        );
        // ширины дословно; высоты — измеренные (18 = кегль·1.3 ≈ 16.9 —
        // фиксируем измеренные, не ручные)
        for (i, (mr, mn)) in measured.iter().zip(manual.iter()).enumerate() {
            approx(mr.w, mn.w);
            assert!(mr.x >= mn.x - 0.01 && mr.x <= mn.x + 0.01, "x[{i}]");
        }
        // высота текст-ребёнка — из измерения (kегль·фактор строки)
        let spec = crate::measure::TextSpec {
            text: labels[0],
            family,
            size: 13.0,
            max_width: f32::INFINITY,
        };
        let measured_h = m.measure(&mut fs, &spec).height;
        approx(measured[0].h, measured_h);
    }

    /// Кламп ширины — только явный max_w; min_w поднимает короткие.
    #[test]
    fn measured_text_clamps_only_explicit_max_w() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = TextMeasurer::new();
        let family = "Noto Sans Display";
        let rects = Row::default().lay_out_measured(
            UiRect::new(0.0, 0.0, 1000.0, 30.0),
            &[
                MeasuredItem::Text {
                    text: "длинная подпись которая заведомо шире потолка",
                    max_w: Some(50.0),
                    min_w: 0.0,
                },
                MeasuredItem::Text {
                    text: "Σ",
                    max_w: None,
                    min_w: 24.0,
                },
            ],
            &mut m,
            &mut fs,
            family,
            13.0,
        );
        approx(rects[0].w, 50.0);
        assert!(rects[1].w >= 24.0);
    }

    // === FR-062 F-16: grid_cells ===

    /// Row-major ячейки: cols × rows от слота с зазорами.
    #[test]
    fn grid_cells_row_major_positions() {
        let cells = grid_cells(
            UiRect::new(0.0, 0.0, 220.0, 100.0),
            &[100.0, 100.0],
            2,
            40.0,
            UiVec2::new(10.0, 10.0),
        );
        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0], UiRect::new(0.0, 0.0, 100.0, 40.0));
        assert_eq!(cells[1], UiRect::new(110.0, 0.0, 100.0, 40.0));
        assert_eq!(cells[2], UiRect::new(0.0, 50.0, 100.0, 40.0));
        assert_eq!(cells[3], UiRect::new(110.0, 50.0, 100.0, 40.0));
    }

    /// Отрицательные ширины/высоты срезаются в 0 (нормализация слота).
    #[test]
    fn grid_cells_negative_clamped() {
        let cells = grid_cells(
            UiRect::new(0.0, 0.0, 100.0, 40.0),
            &[-5.0],
            1,
            -3.0,
            UiVec2::new(0.0, 0.0),
        );
        assert_eq!(cells[0], UiRect::new(0.0, 0.0, 0.0, 0.0));
    }

    // === FR-062 F-18: геометрические снапшоты (детерминированные) ===

    /// Золотые строки геометрии фикс-компонентов (без шрифтов): изменение
    /// раскладки ловится сравнением с эталоном (перегенерация — осознанно).
    #[test]
    fn snapshot_fixed_geometry_golden() {
        use crate::testing::{assert_snapshot, snap};
        let grid = grid_cells(
            UiRect::new(10.0, 20.0, 300.0, 100.0),
            &[70.0, 70.0, 70.0],
            2,
            24.0,
            UiVec2::new(8.0, 6.0),
        );
        let got = grid
            .iter()
            .map(|r| snap("grid", *r))
            .collect::<Vec<_>>()
            .join("\n");
        assert_snapshot(
            got,
            "grid x=10 y=20 w=70 h=24\n\
             grid x=88 y=20 w=70 h=24\n\
             grid x=166 y=20 w=70 h=24\n\
             grid x=10 y=50 w=70 h=24\n\
             grid x=88 y=50 w=70 h=24\n\
             grid x=166 y=50 w=70 h=24",
        );
        let stacked = stack(
            UiRect::new(0.0, 0.0, 400.0, 300.0),
            UiVec2::new(120.0, 60.0),
            HAlign::End,
            VAlign::End,
        );
        assert_snapshot(snap("stack", stacked), "stack x=280 y=240 w=120 h=60");
    }

    fn approx(a: f32, b: f32) {
        assert!(
            (a - b).abs() < 0.01,
            "approx failed: {a} vs {b} (diff {})",
            (a - b).abs()
        );
    }
}
