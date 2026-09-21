//! FR-051 U1: чистая геометрия экрана `canvas-ui` (PRD-0009 §7.3 — «модель
//! экрана», симметрично `canvas-core` = «модель мира»).
//!
//! Единицы — пиксели вьюпорта (ui px), начало координат — левый верхний угол
//! окна. Типы без внешних зависимостей: любые вычисления детерминированы и
//! headless-тестируемы (PRD-0009 §9.1/§9.6). Это precursor layout-примитивов
//! F-7 (U3): интерфейс выбран совместимым со слотами taffy (PRD-0009 §7.4).

/// Точка экрана в ui px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiPoint {
    pub x: f32,
    pub y: f32,
}

/// Вектор/размер в ui px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiVec2 {
    pub x: f32,
    pub y: f32,
}

/// Внутренние отступы (precursor `EdgeInsets` F-7; источник значений —
/// spacing-scale `design/tokens/dimensions.json`, подключается на U3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Прямоугольник экрана в ui px. Ширина/высота нормализуются: отрицательные
/// значения срезаются в 0 (вырожденный rect — валиден, детектируется
/// [`UiRect::is_empty`]; F-11 c — «hide-политики вместо вырожденных rect'ов»).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl UiPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl UiVec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl EdgeInsets {
    pub const fn uniform(px: f32) -> Self {
        Self {
            left: px,
            top: px,
            right: px,
            bottom: px,
        }
    }

    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl UiRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x,
            y,
            w: if w < 0.0 { 0.0 } else { w },
            h: if h < 0.0 { 0.0 } else { h },
        }
    }

    /// Из левого-верхнего и правого-нижнего углов (порядок нормализуется).
    pub fn from_min_max(min: UiPoint, max: UiPoint) -> Self {
        let (x0, x1) = if min.x <= max.x {
            (min.x, max.x)
        } else {
            (max.x, min.x)
        };
        let (y0, y1) = if min.y <= max.y {
            (min.y, max.y)
        } else {
            (max.y, min.y)
        };
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }

    pub const fn right(&self) -> f32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub const fn min(&self) -> UiPoint {
        UiPoint::new(self.x, self.y)
    }

    pub const fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    pub fn contains(&self, p: UiPoint) -> bool {
        !self.is_empty() && p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    pub fn intersects(&self, other: &UiRect) -> bool {
        !self.is_empty()
            && !other.is_empty()
            && self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Пересечение двух rect'ов; пустое множество — `None`.
    pub fn intersection(&self, other: &UiRect) -> Option<UiRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 > x0 && y1 > y0 {
            Some(UiRect::new(x0, y0, x1 - x0, y1 - y0))
        } else {
            None
        }
    }

    /// Сжатие rect'а на отступы (precursor `Padding` F-7).
    pub fn inset(&self, e: &EdgeInsets) -> UiRect {
        let w = self.w - e.horizontal();
        let h = self.h - e.vertical();
        UiRect::new(self.x + e.left, self.y + e.top, w.max(0.0), h.max(0.0))
    }

    /// Сдвиг rect'а (примитив `Stack`/позиционирования F-7).
    pub fn translated(&self, by: UiVec2) -> UiRect {
        UiRect::new(self.x + by.x, self.y + by.y, self.w, self.h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_normalizes_negative_size() {
        let r = UiRect::new(10.0, 10.0, -5.0, -5.0);
        assert_eq!((r.w, r.h), (0.0, 0.0));
        assert!(r.is_empty());
    }

    #[test]
    fn from_min_max_normalizes_order() {
        let r = UiRect::from_min_max(UiPoint::new(20.0, 30.0), UiPoint::new(10.0, 15.0));
        assert_eq!(r, UiRect::new(10.0, 15.0, 10.0, 15.0));
    }

    #[test]
    fn contains_is_half_open() {
        let r = UiRect::new(0.0, 0.0, 100.0, 50.0);
        assert!(r.contains(UiPoint::new(0.0, 0.0)));
        assert!(r.contains(UiPoint::new(99.9, 49.9)));
        assert!(!r.contains(UiPoint::new(100.0, 25.0)));
        assert!(!r.contains(UiPoint::new(50.0, 50.0)));
    }

    #[test]
    fn intersection_and_overlap() {
        let a = UiRect::new(0.0, 0.0, 100.0, 100.0);
        let b = UiRect::new(80.0, 80.0, 100.0, 100.0);
        let c = UiRect::new(200.0, 200.0, 10.0, 10.0);
        assert!(a.intersects(&b));
        assert_eq!(
            a.intersection(&b),
            Some(UiRect::new(80.0, 80.0, 20.0, 20.0))
        );
        assert!(!a.intersects(&c));
        assert_eq!(a.intersection(&c), None);
    }

    #[test]
    fn inset_and_translate() {
        let r = UiRect::new(10.0, 10.0, 120.0, 80.0);
        let inner = r.inset(&EdgeInsets::uniform(10.0));
        assert_eq!(inner, UiRect::new(20.0, 20.0, 100.0, 60.0));
        let moved = r.translated(UiVec2::new(-5.0, 2.5));
        assert_eq!(moved, UiRect::new(5.0, 12.5, 120.0, 80.0));
        // inset больше rect — срезается в пустой
        let empty = UiRect::new(0.0, 0.0, 4.0, 4.0).inset(&EdgeInsets::uniform(10.0));
        assert!(empty.is_empty());
    }
}
