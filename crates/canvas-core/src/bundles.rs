//! FR-044 §Changes-1 — расширение контракта stage-подсистемы FR-042:
//! лейн-раскладка подписей значений веера и коридор между колонками портов.
//!
//! Модуль создаётся раньше исполнения FR-042 (сметчивание 2026-09-21),
//! чтобы stage не пришлось перепроектировать после E0: чистые функции
//! без рендера и состояния, wasm-гейт (ADR-0011). Правила — FR-044 §Решения
//! Р-1 (проверены владельцем на прототипе):
//! - подписи рисуются не у середин линий, а стопкой в «коридоре» между
//!   колонками подписей портов истока и приёмника;
//! - шаг стопки = фактическая высота пилюли + зазор 8 px;
//! - стопка центрируется на вертикальной оси веера и клампится в зону
//!   stage; при переполнении — уплотнение шага (§Q2, базово);
//! - пилюли зажаты в горизонтальный коридор;
//! - коллизии: подпись×подпись запрещены, подпись×текст нод запрещены,
//!   подпись над линией ребра — разрешено (подложка обеспечивает читаемость).
//!
//! `QualifiedRef`/`display_ref` (адресация «Объект.Поле») живут в
//! `dataref.rs` — FR-045 Р-5, приоритет владельца; этот модуль импортирует
//! их для отображения.

/// Зазор между пилюлями стопки (FR-044 Р-1).
pub const FAN_LABEL_GAP_PX: f32 = 8.0;

/// Прямоугольник в screen-space stage (локальная геометрия модуля).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn right(&self) -> f32 {
        self.x + self.w
    }

    fn bottom(&self) -> f32 {
        self.y + self.h
    }

    #[cfg(test)]
    fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    #[cfg(test)]
    fn contains_fully(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

/// Коридор между колонкой подписей портов истока и колонкой приёмника
/// (FR-044 §Changes-1 `fan_corridor`): горизонтальная полоса между
/// правым краем колонки истока + `pad` и левым краем колонки приёмника
/// − `pad`; вертикальный охват — объединение обеих колонок. Колонки
/// подписей в коридор не входят (инвариант 2). Перепашка (нет места) —
/// нулевая ширина.
pub fn fan_corridor(source_labels: Rect, target_labels: Rect, pad: f32) -> Rect {
    let left = source_labels.right() + pad;
    let right = target_labels.x - pad;
    let y = source_labels.y.min(target_labels.y);
    let bottom = source_labels.bottom().max(target_labels.bottom());
    let w = (right - left).max(0.0);
    Rect {
        x: if w > 0.0 { left } else { (left + right) / 2.0 },
        y,
        w,
        h: bottom - y,
    }
}

/// Пилюля подписи значения веера после раскладки.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FanPill {
    /// Элемент входа `items` (индекс ребра — по контракту вызова).
    pub item: usize,
    /// Итоговый прямоугольник пилюли.
    pub rect: Rect,
    /// Пилюля сдвинута/уплотнена относительно естественной стопки
    /// (кламп по вертикали или горизонтали).
    pub clamped: bool,
}

/// Результат лейн-раскладки (FR-044 §Changes-1 `FanLabelLayout`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FanLabelLayout {
    /// Пилюли в порядке входа `items` (сверху вниз по стопке).
    pub pills: Vec<FanPill>,
    /// Признак вертикального уплотнения: естественная стопка не влезла
    /// в `clamp_zone` и шаг был сжат (§Q2 — сократить текст/скролл: v2).
    pub compressed: bool,
}

/// Лейн-раскладка подписей значений веера (FR-044 Р-1, чистая функция).
///
/// - `items` — `(индекс ребра, ширина пилюли, высота пилюли)` в порядке
///   рёбер веера (стабильный порядок `edge.id` на вызывающей стороне);
/// - `corridor` — горизонтальный коридор ([`fan_corridor`]);
/// - `clamp_zone` — зона stage между заголовком и панелью «Как считается»;
/// - `axis_y` — вертикальная ось веера (центрирование стопки).
///
/// Детерминирована: одинаковый вход → одинаковый выход (инвариант 3).
/// Гарантии: пилюли внутри `clamp_zone` (инвариант 2) и внутри `corridor`;
/// пересечения пилюль исключены, пока суммарная высота помещается в зону
/// (инвариант 1), при переполнении — уплотнение шага до нуля и прижатие
/// к нижней границе (пересечения возможны только в этом деградационном
/// режиме — сигнал для сокращения текста, §Q2).
pub fn stage_fan_label_layout(
    items: Vec<(usize, f32, f32)>,
    corridor: Rect,
    clamp_zone: Rect,
    axis_y: f32,
) -> FanLabelLayout {
    let heights: Vec<f32> = items.iter().map(|(_, _, h)| *h).collect();
    let sum_h: f32 = heights.iter().sum();
    let n = items.len();
    let total = if n > 1 {
        sum_h + FAN_LABEL_GAP_PX * (n - 1) as f32
    } else {
        sum_h
    };
    let zone_h = clamp_zone.h.max(0.0);
    // Естественная стопка: центрирована на оси веера.
    let mut top = axis_y - total / 2.0;
    let mut compressed = false;
    let mut gap = FAN_LABEL_GAP_PX;
    if zone_h <= 0.0 || n == 0 {
        return FanLabelLayout::default();
    }
    if total > zone_h {
        // Переполнение: сначала уплотняем шаг до нуля (§Q2, базово),
        // затем прижимаем стопку к верхней границе зоны; каждая пилюля
        // клампится внутрь зоны (деградационный режим, компромисс §Q2).
        compressed = true;
        gap = if sum_h >= zone_h {
            0.0
        } else {
            (zone_h - sum_h) / (n - 1).max(1) as f32
        };
        top = clamp_zone.y;
    } else {
        // Кламп всей стопки внутрь зоны без изменения шага.
        top = top.max(clamp_zone.y).min(clamp_zone.y + zone_h - total);
    }
    let mut pills = Vec::with_capacity(n);
    let mut y = top;
    let mut natural_y = if total > zone_h {
        axis_y - total / 2.0
    } else {
        top
    };
    for (item, w, h) in items {
        // Горизонталь: центр коридора, кламп внутрь (инвариант 2).
        let mut x = corridor.x + (corridor.w - w) / 2.0;
        let mut clamped = false;
        if corridor.w > 0.0 && x < corridor.x {
            x = corridor.x;
            clamped = true;
        }
        if corridor.w > 0.0 && x + w > corridor.right() {
            x = if w >= corridor.w {
                corridor.x
            } else {
                corridor.right() - w
            };
            clamped = true;
        }
        let rect = if compressed {
            // Прижатие внутрь зоны: перекрытие возможно только здесь.
            let py = if y + h > clamp_zone.bottom() {
                clamped = true;
                clamp_zone.bottom() - h
            } else {
                y
            };
            y = py + h; // уплотнение: без зазора
            Rect { x, y: py, w, h }
        } else {
            if (y - natural_y).abs() > f32::EPSILON {
                clamped = true;
            }
            let py = y;
            y = py + h + gap;
            natural_y = natural_y + h + gap;
            Rect { x, y: py, w, h }
        };
        pills.push(FanPill {
            item,
            rect,
            clamped,
        });
    }
    FanLabelLayout { pills, compressed }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PILL_W: f32 = 170.0;
    const PILL_H: f32 = 24.0;

    fn demo_items(n: usize) -> Vec<(usize, f32, f32)> {
        (0..n).map(|i| (i, PILL_W, PILL_H)).collect()
    }

    fn zone(y: f32, h: f32) -> Rect {
        Rect {
            x: 0.0,
            y,
            w: 800.0,
            h,
        }
    }

    fn corridor_demo() -> Rect {
        Rect {
            x: 100.0,
            y: 0.0,
            w: 500.0,
            h: 400.0,
        }
    }

    /// Инвариант 1: пучок ×6 (демо-размеры) — 0 попарных пересечений.
    #[test]
    fn fan_layout_x6_no_overlaps() {
        let layout =
            stage_fan_label_layout(demo_items(6), corridor_demo(), zone(50.0, 300.0), 200.0);
        assert_eq!(layout.pills.len(), 6);
        assert!(!layout.compressed);
        for i in 0..layout.pills.len() {
            for j in i + 1..layout.pills.len() {
                assert!(
                    !layout.pills[i].rect.intersects(&layout.pills[j].rect),
                    "пилюли {i} и {j} пересекаются: {:?} × {:?}",
                    layout.pills[i].rect,
                    layout.pills[j].rect
                );
            }
        }
        // Центрирование на оси: верх+низ симметричны.
        let top = layout.pills[0].rect.y;
        let bottom = layout.pills[5].rect.bottom();
        assert!((top - (400.0 - bottom)).abs() < 0.01, "стопка центрирована");
    }

    /// Инвариант 2: ×8 при минимальной высоте — все пилюли внутри зоны
    /// (уплотнение шага, деградационный режим §Q2).
    #[test]
    fn fan_layout_x8_min_height_clamped_inside() {
        let zone = zone(50.0, 100.0);
        let layout = stage_fan_label_layout(demo_items(8), corridor_demo(), zone, 100.0);
        assert!(layout.compressed, "переполнение должно быть распознано");
        for pill in &layout.pills {
            assert!(
                zone.contains_fully(&pill.rect),
                "пилюля {:?} вне зоны {:?}",
                pill.rect,
                zone
            );
        }
    }

    /// Инвариант 3: детерминизм — двойной вызов даёт идентичные rect.
    #[test]
    fn fan_layout_deterministic() {
        let a = stage_fan_label_layout(demo_items(6), corridor_demo(), zone(50.0, 300.0), 200.0);
        let b = stage_fan_label_layout(demo_items(6), corridor_demo(), zone(50.0, 300.0), 200.0);
        assert_eq!(a, b);
        // Смена порядка рёбер меняет раскладку предсказуемо: item смещается.
        let mut reordered = demo_items(6);
        reordered.swap(0, 5);
        let c = stage_fan_label_layout(reordered, corridor_demo(), zone(50.0, 300.0), 200.0);
        assert_eq!(c.pills[0].item, 5, "порядок входа сохранён в выходе");
    }

    /// Инвариант 2 (горизонталь): слишком широкая пилюля клампится в
    /// коридор; колонки подписей портов не перекрываются.
    #[test]
    fn fan_layout_horizontal_clamp() {
        let corridor = Rect {
            x: 100.0,
            y: 0.0,
            w: 300.0,
            h: 400.0,
        };
        let layout =
            stage_fan_label_layout(vec![(0, 500.0, PILL_H)], corridor, zone(0.0, 400.0), 200.0);
        let pill = &layout.pills[0];
        assert!(pill.clamped);
        assert_eq!(
            pill.rect.x, corridor.x,
            "широкая пилюля прижата к левому краю"
        );
    }

    /// `fan_corridor`: коридор не включает колонки подписей портов.
    #[test]
    fn corridor_excludes_label_columns() {
        let source = Rect {
            x: 10.0,
            y: 100.0,
            w: 120.0,
            h: 200.0,
        };
        let target = Rect {
            x: 500.0,
            y: 120.0,
            w: 140.0,
            h: 180.0,
        };
        let pad = 16.0;
        let c = fan_corridor(source, target, pad);
        assert!(
            c.x >= source.right() + pad - f32::EPSILON,
            "левее колонки истока"
        );
        assert!(
            c.right() <= target.x - pad + f32::EPSILON,
            "правее колонки приёмника"
        );
        assert!(c.y <= source.y.min(target.y));
        assert!(c.bottom() >= source.bottom().max(target.bottom()));
        // Перепашка: колонки наезжают — нулевая ширина.
        let tight = fan_corridor(
            source,
            Rect {
                x: 20.0,
                y: 100.0,
                w: 50.0,
                h: 100.0,
            },
            pad,
        );
        assert_eq!(tight.w, 0.0);
    }

    /// Стопка клампится в зону без уплотнения, когда помещается впритык.
    #[test]
    fn fan_layout_stack_shift_clamp() {
        // Ось вне зоны: стопка сдвигается целиком, шаг сохранён.
        let layout = stage_fan_label_layout(
            demo_items(4),
            corridor_demo(),
            zone(300.0, 200.0),
            100.0, // ось выше зоны
        );
        assert!(!layout.compressed);
        let top = layout.pills[0].rect.y;
        assert!((top - 300.0).abs() < 0.01, "стопка прижата к верху зоны");
        for pill in &layout.pills {
            assert!(zone(300.0, 200.0).contains_fully(&pill.rect));
        }
    }
}
