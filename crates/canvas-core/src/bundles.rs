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

// ============================================================================
// FR-042 (этап E1): пучки рёбер, толщина по весу и геометрия main stage.
// ============================================================================

use std::collections::HashMap;

use crate::model::Canvas;
use crate::FlowKind;

/// База формулы толщины пучка (FR-042 §Changes-1): d(1) = 1.8 world-px —
/// совпадает с `EDGE_DOT` рендера, одиночное ребро не меняет вид.
pub const BUNDLE_THICKNESS_BASE: f32 = 1.8;
/// Шаг толщины на каждое дополнительное ребро пучка.
pub const BUNDLE_THICKNESS_STEP: f32 = 0.9;
/// Кап толщины: очень крупные пучки не превращаются в трубы.
pub const BUNDLE_THICKNESS_MAX: f32 = 8.0;
/// Зазор между соседними линиями веера в stage поверх толщины (px).
pub const STAGE_FAN_GAP_PX: f32 = 6.0;
/// Максимальная доля стороны вьюпорта (PRD-0002 §7.2: «≤ 70%»).
pub const MAIN_STAGE_MAX_FRACTION: f32 = 0.7;
/// Абсолютный кап ширины stage, px.
pub const STAGE_MAX_W: f32 = 1280.0;
/// Абсолютный кап высоты stage, px.
pub const STAGE_MAX_H: f32 = 800.0;
/// Поле stage от краёв окна при клампе, px.
pub const STAGE_MARGIN: f32 = 24.0;
/// Горизонтальный отступ нод от краёв stage в stage-единицах, px.
pub const STAGE_NODE_PAD: f32 = 48.0;

/// Непрерывная толщина агрегированной линии пучка (инвариант 3 FR-042):
/// монотонная функция веса, `d(1) = 1.8`, кап 8.0.
pub fn bundle_thickness(weight: usize) -> f32 {
    (BUNDLE_THICKNESS_BASE + (weight.saturating_sub(1) as f32) * BUNDLE_THICKNESS_STEP)
        .min(BUNDLE_THICKNESS_MAX)
}

/// Шаг веера stage: не меньше толщины линии + зазор (инвариант 6).
pub fn stage_fan_spacing(weight: usize) -> f32 {
    bundle_thickness(weight) + STAGE_FAN_GAP_PX
}

/// Симметричные перпендикулярные смещения рёбер веера (FR-042 §4):
/// `[-(k-1)/2·s, …, +(k-1)/2·s]`, детерминировано; k = 0 → пусто.
pub fn stage_edge_fan(count: usize, spacing: f32) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    (0..count)
        .map(|i| (i as f32 - (count as f32 - 1.0) / 2.0) * spacing)
        .collect()
}

/// Прямоугольник main stage (инвариант 4 FR-042): центрирован, обе стороны
/// ≤ 70% соответствующей стороны вьюпорта (и ≤ абсолютных капов), целиком
/// внутри окна с полем [`STAGE_MARGIN`] при любых пропорциях.
pub fn main_stage_rect(viewport: [f32; 2]) -> Rect {
    let vw = viewport[0].max(0.0);
    let vh = viewport[1].max(0.0);
    let w = (vw * MAIN_STAGE_MAX_FRACTION)
        .min(STAGE_MAX_W)
        .min(vw - STAGE_MARGIN * 2.0)
        .max(1.0);
    let h = (vh * MAIN_STAGE_MAX_FRACTION)
        .min(STAGE_MAX_H)
        .min(vh - STAGE_MARGIN * 2.0)
        .max(1.0);
    Rect {
        x: (vw - w) / 2.0,
        y: (vh - h) / 2.0,
        w,
        h,
    }
}

/// Пучок рёбер: индексы в `canvas.edges` (стабильный порядок по `edge.id`)
/// и вес = число рёбер (решение PRD §11-Q1).
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeBundle {
    /// Индексы рёбер пучка, отсортированы по `edge.id` (инвариант 2).
    pub edges: Vec<usize>,
    /// Вес пучка = число рёбер.
    pub weight: usize,
}

/// Индекс пучков (FR-042 §Changes-1): группировка рёбер по упорядоченной
/// паре концов `(from_node, to_node)` — A→B и B→A разные пучки (Q2).
/// Висячие рёбра (одна из нод отсутствует) не агрегируются и построение
/// не ломают (инвариант 1). Runtime-кэш сцены: не сериализуется,
/// перестраивается в хвосте `recompute_flow` (O(edges) при мутациях).
#[derive(Debug, Clone, Default)]
pub struct EdgeBundleIndex {
    bundles: HashMap<(String, String), EdgeBundle>,
    of_edge: Vec<Option<(String, String)>>,
}

impl EdgeBundleIndex {
    /// Построить индекс из канваса. Детерминирован: порядок внутри пучка —
    /// по `edge.id`, повторное построение на той же модели идентично.
    pub fn build(canvas: &Canvas) -> Self {
        let mut exists: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(canvas.nodes.len());
        for node in &canvas.nodes {
            exists.insert(node.id.as_str());
        }
        let mut map: HashMap<(String, String), Vec<usize>> = HashMap::new();
        let mut of_edge: Vec<Option<(String, String)>> = Vec::with_capacity(canvas.edges.len());
        for (index, edge) in canvas.edges.iter().enumerate() {
            let key = if exists.contains(edge.from_node.as_str())
                && exists.contains(edge.to_node.as_str())
            {
                Some((edge.from_node.clone(), edge.to_node.clone()))
            } else {
                None // висячее ребро — ни с чем не агрегируется
            };
            if let Some(key) = &key {
                map.entry(key.clone()).or_default().push(index);
            }
            of_edge.push(key);
        }
        let mut bundles = HashMap::with_capacity(map.len());
        for (key, mut indices) in map {
            // Стабильный порядок внутри пучка — по id ребра (инвариант 2).
            indices.sort_by(|&a, &b| canvas.edges[a].id.cmp(&canvas.edges[b].id));
            let weight = indices.len();
            bundles.insert(
                key,
                EdgeBundle {
                    edges: indices,
                    weight,
                },
            );
        }
        Self { bundles, of_edge }
    }

    /// Пучок ребра (вес ≥ 1); одиночное ребро валидной пары — пучок
    /// веса 1 (агрегация на рендере включается при weight ≥ 2).
    pub fn bundle_of_edge(&self, edge_index: usize) -> Option<&EdgeBundle> {
        let key = self.of_edge.get(edge_index)?.as_ref()?;
        self.bundles.get(key)
    }

    /// Ключ пучка ребра — упорядоченная пара id концов.
    pub fn bundle_key_of_edge(&self, edge_index: usize) -> Option<(&str, &str)> {
        let key = self.of_edge.get(edge_index)?.as_ref()?;
        Some((key.0.as_str(), key.1.as_str()))
    }

    /// Пучок по паре концов (упорядоченная: from → to).
    pub fn bundle_of_pair(&self, from: &str, to: &str) -> Option<&EdgeBundle> {
        self.bundles.get(&(from.to_owned(), to.to_owned()))
    }

    /// Доминирующее ребро пучка (FR-042 §Changes-1): детерминированный
    /// выбор среди рёбер с наивысшим приоритетом класса: явный цвет
    /// пользователя → value-ребро (`FlowKind::Value`) → прочие. Порядок
    /// внутри класса — стабильный по `edge.id` (порядок `bundle.edges`).
    pub fn dominant_edge(&self, canvas: &Canvas, bundle: &EdgeBundle) -> Option<usize> {
        let rank = |index: usize| -> u8 {
            let edge = &canvas.edges[index];
            if edge.color.as_deref().is_some_and(|c| !c.is_empty()) {
                2
            } else if edge.flow_kind() == FlowKind::Value {
                1
            } else {
                0
            }
        };
        // Первый максимум: порядок обхода — `edge.id`, равный ранг НЕ
        // вытесняет предыдущего кандидата (инвариант 2 — стабильность).
        let mut best: Option<(u8, usize)> = None;
        for &index in &bundle.edges {
            let r = rank(index);
            if best.map_or(true, |(br, _)| r > br) {
                best = Some((r, index));
            }
        }
        best.map(|(_, index)| index)
    }

    /// Число пучков (диагностика/тесты).
    pub fn len(&self) -> usize {
        self.bundles.len()
    }

    /// Пуст ли индекс.
    pub fn is_empty(&self) -> bool {
        self.bundles.is_empty()
    }
}

/// Раскладка main stage (FR-042 §Changes-1 + §7.2): позиции нод в
/// rect-относительных экранных px (уже с масштабом), масштаб сжатия
/// (≤ 1; гарантирует умещение при любом viewport — инвариант 4/7) и
/// коридор между колонками нод (стык FR-044 `fan_corridor`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageLayout {
    /// Левый верхний угол ноды-истока (rect-относительные screen px).
    pub source_pos: [f32; 2],
    /// Левый верхний угол ноды-приёмника.
    pub target_pos: [f32; 2],
    /// Масштаб сжатия содержимого (1.0 — натуральный размер).
    pub scale: f32,
    /// Коридор для подписей веера между колонками нод (screen px,
    /// rect-относительные; ширина 0 — перепашка, подписи в FR-044 клампятся).
    pub corridor: Rect,
}

/// Раскладка stage: исток слева, приёмник справа, вертикально центрированы;
/// при нехватке места содержимое сжимается единым масштабом (≤ 1), поэтому
/// инвариант «всё умещается в rect» геометрически гарантирован. Чистая
/// функция — детерминирована (инвариант 3 FR-044 для стыка раскладок).
pub fn stage_layout(source_size: [f32; 2], target_size: [f32; 2], rect: &Rect) -> StageLayout {
    let pad = STAGE_NODE_PAD;
    // Натуральная ширина контента: две ноды + два отступа + минимальный
    // коридор подписей (половина ширины самой широкой колонки, минимум 160).
    let min_corridor = 160.0f32.max(source_size[0].min(target_size[0]) * 0.5);
    let natural_w = source_size[0] + target_size[0] + pad * 2.0 + min_corridor;
    let natural_h = source_size[1].max(target_size[1]) + pad * 2.0;
    let scale = if natural_w <= rect.w && natural_h <= rect.h {
        1.0
    } else {
        let sx = rect.w / natural_w;
        let sy = rect.h / natural_h;
        (sx.min(sy)).max(0.05)
    };
    let scaled = |size: f32| size * scale;
    let corridor_w =
        ((rect.w - pad * scale * 2.0 - scaled(source_size[0]) - scaled(target_size[0])).max(0.0))
            .max(if scale < 1.0 {
                scaled(min_corridor)
            } else {
                0.0
            });
    let source_x = pad * scale;
    let target_x = source_x + scaled(source_size[0]) + corridor_w;
    let source_pos = [source_x, (rect.h - scaled(source_size[1])) / 2.0];
    let target_pos = [target_x, (rect.h - scaled(target_size[1])) / 2.0];
    let corridor = Rect {
        x: source_x + scaled(source_size[0]),
        y: source_pos[1],
        w: corridor_w,
        h: scaled(source_size[1]).max(scaled(target_size[1])),
    };
    StageLayout {
        source_pos,
        target_pos,
        scale,
        corridor,
    }
}

/// Единичная нормаль веера: перпендикуляр к оси «центр истока → центр
/// приёмника» среза (слайс stage содержит ровно 2 ноды). None — нод не две
/// или ось вырождена.
pub fn stage_fan_normal(slice: &Canvas) -> Option<[f32; 2]> {
    let a = slice.nodes.first()?;
    let b = slice.nodes.get(1)?;
    let ax = b.x + b.width / 2.0 - (a.x + a.width / 2.0);
    let ay = b.y + b.height / 2.0 - (a.y + a.height / 2.0);
    let len = ax.hypot(ay);
    if len < f32::EPSILON {
        return None;
    }
    Some([-ay / len, ax / len])
}

/// Полилиния ребра среза со смещением веера: polyline ребра слайса,
/// каждая точка сдвинута на `fan_offset * нормаль` (FR-042 §4 —
/// перпендикулярно оси A→B). Единая формула для рендера и hit-test'а.
pub fn stage_edge_points(
    slice: &Canvas,
    edge_index: usize,
    fan_offset: f32,
    segments: usize,
) -> Option<Vec<[f32; 2]>> {
    let edge = slice.edges.get(edge_index)?;
    let normal = stage_fan_normal(slice)?;
    let points = crate::edge_polyline(slice, edge, false, segments)?;
    if fan_offset.abs() < f32::EPSILON {
        return Some(points);
    }
    Some(
        points
            .into_iter()
            .map(|p| [p[0] + normal[0] * fan_offset, p[1] + normal[1] * fan_offset])
            .collect(),
    )
}

/// Hit-test рёбер среза с веером (FR-042 §Changes-4, ввод внутри stage):
/// индекс среза с минимальной дистанцией в допуске; None — промах.
pub fn stage_edge_at(
    slice: &Canvas,
    fan: &[f32],
    point: [f32; 2],
    tolerance: f32,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for index in 0..slice.edges.len() {
        let offset = fan.get(index).copied().unwrap_or(0.0);
        let Some(points) = stage_edge_points(slice, index, offset, 24) else {
            continue;
        };
        let mut dist = f32::INFINITY;
        for w in points.windows(2) {
            let d = point_segment_distance(point, w[0], w[1]);
            if d < dist {
                dist = d;
            }
        }
        if dist <= tolerance && best.map_or(true, |(bd, _)| dist < bd) {
            best = Some((dist, index));
        }
    }
    best.map(|(_, index)| index)
}

/// Расстояние от точки до отрезка (чистая геометрия).
fn point_segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let abx = b[0] - a[0];
    let aby = b[1] - a[1];
    let apx = p[0] - a[0];
    let apy = p[1] - a[1];
    let len2 = abx * abx + aby * aby;
    let t = if len2 < f32::EPSILON {
        0.0
    } else {
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    };
    let cx = a[0] + abx * t;
    let cy = a[1] + aby * t;
    (p[0] - cx).hypot(p[1] - cy)
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

#[cfg(test)]
mod fr042_tests {
    use super::*;
    use crate::model::{Edge, Node};

    /// Сцена FR-042: A→B ×3, B→A ×2, висячее ребро.
    fn scene() -> Canvas {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "A", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "B", 400.0, 0.0));
        let e = |id: &str, from: &str, to: &str| Edge::new(id, from, None, to, None);
        canvas.add_edge(e("e1", "a", "b"));
        canvas.add_edge(e("e2", "a", "b"));
        canvas.add_edge(e("e3", "b", "a"));
        canvas.add_edge(e("e4", "b", "a"));
        canvas.add_edge(e("e5", "a", "b"));
        // Висячее: ghost-ноды нет
        canvas.add_edge(e("e6", "a", "ghost"));
        canvas
    }

    /// Инвариант 1: группировка по упорядоченной паре; A→B и B→A —
    /// разные пучки (Q2); висячее ребро не агрегируется.
    #[test]
    fn grouping_bidirectional_and_hanging() {
        let canvas = scene();
        let index = EdgeBundleIndex::build(&canvas);
        assert_eq!(index.len(), 2, "A→B и B→A — два пучка");
        let ab = index.bundle_of_pair("a", "b").expect("пучок A→B");
        assert_eq!(ab.weight, 3);
        assert_eq!(ab.edges, vec![0, 1, 4], "стабильный порядок по edge.id");
        let ba = index.bundle_of_pair("b", "a").expect("пучок B→A");
        assert_eq!(ba.weight, 2);
        assert_eq!(ba.edges, vec![2, 3]);
        // Висячее ребро (индекс 5) — без пучка
        assert!(index.bundle_of_edge(5).is_none());
        assert!(index.bundle_key_of_edge(5).is_none());
        // Доступ через bundle_of_edge согласован
        assert_eq!(index.bundle_of_edge(0).map(|b| b.weight), Some(3));
        assert_eq!(index.bundle_key_of_edge(2), Some(("b", "a")));
    }

    /// Инвариант 1: пустой канвас — пустой индекс без паники.
    #[test]
    fn empty_canvas() {
        let index = EdgeBundleIndex::build(&Canvas::default());
        assert!(index.is_empty());
        assert!(index.bundle_of_edge(0).is_none());
    }

    /// Инвариант 2: детерминизм — повторное построение идентично.
    #[test]
    fn deterministic_rebuild() {
        let canvas = scene();
        let a = EdgeBundleIndex::build(&canvas);
        let b = EdgeBundleIndex::build(&canvas);
        for (index, _) in canvas.edges.iter().enumerate() {
            assert_eq!(
                a.bundle_of_edge(index).map(|x| x.edges.clone()),
                b.bundle_of_edge(index).map(|x| x.edges.clone())
            );
            if let Some(bundle) = a.bundle_of_edge(index) {
                assert_eq!(
                    a.dominant_edge(&canvas, bundle),
                    b.dominant_edge(&canvas, b.bundle_of_edge(index).unwrap())
                );
            }
        }
    }

    /// Доминирующее ребро: user-color > value > прочее; при равном ранге —
    /// первое по edge.id (стабильность).
    #[test]
    fn dominant_edge_priority_and_stability() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "A", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "B", 400.0, 0.0));
        let e1 = Edge::new("e1", "a", None, "b", None); // прочее (control)
        let mut e2 = Edge::new("e2", "a", None, "b", None); // user-color
        e2.color = Some("#ff0000".to_owned());
        let mut e3 = Edge::new("e3", "a", None, "b", None); // value
        e3.set_flow_kind(crate::FlowKind::Value);
        canvas.add_edge(e1);
        canvas.add_edge(e2);
        canvas.add_edge(e3);
        let index = EdgeBundleIndex::build(&canvas);
        let bundle = index.bundle_of_pair("a", "b").expect("пучок");
        assert_eq!(bundle.weight, 3);
        let dominant = index.dominant_edge(&canvas, bundle).expect("доминант");
        assert_eq!(canvas.edges[dominant].id, "e2", "user-color выигрывает");
        // Убираем цвет: доминанта — value-ребро e3
        canvas.edges[1].color = None;
        let index2 = EdgeBundleIndex::build(&canvas);
        let bundle2 = index2.bundle_of_pair("a", "b").unwrap();
        let dominant2 = index2.dominant_edge(&canvas, bundle2).unwrap();
        assert_eq!(canvas.edges[dominant2].id, "e3", "value > прочее");
        // Все прочие (ни цвета, ни value): первое по edge.id
        canvas.edges[2].set_flow_kind(crate::FlowKind::Control);
        let index3 = EdgeBundleIndex::build(&canvas);
        let bundle3 = index3.bundle_of_pair("a", "b").unwrap();
        let dominant3 = index3.dominant_edge(&canvas, bundle3).unwrap();
        assert_eq!(canvas.edges[dominant3].id, "e1");
    }

    /// Инвариант 3: толщина — d(1)=1.8, d(2)=2.7, d(4)=4.5, d(8)=8.0 (кап),
    /// монотонность.
    #[test]
    fn thickness_values_and_monotonic() {
        assert!((bundle_thickness(1) - 1.8).abs() < 1e-4);
        assert!((bundle_thickness(2) - 2.7).abs() < 1e-4);
        assert!((bundle_thickness(4) - 4.5).abs() < 1e-4);
        assert!((bundle_thickness(8) - 8.0).abs() < 1e-4, "кап");
        assert!((bundle_thickness(50) - 8.0).abs() < 1e-4, "кап держится");
        let mut prev = bundle_thickness(1);
        for w in 2..=12 {
            let d = bundle_thickness(w);
            assert!(d >= prev, "монотонность при weight={w}");
            prev = d;
        }
    }

    /// Инвариант 4: main_stage_rect на пропорциях 320×240, 1920×1080,
    /// 3440×1440 (ультраширокий), 1080×2400 (высокий) — внутри окна,
    /// ≤ 70% сторон, центрирован.
    #[test]
    fn stage_rect_viewports() {
        for viewport in [
            [320.0, 240.0],
            [1920.0, 1080.0],
            [3440.0, 1440.0],
            [1080.0, 2400.0],
        ] {
            let [vw, vh] = viewport;
            let r = main_stage_rect(viewport);
            assert!(r.x >= STAGE_MARGIN - 1e-3, "{viewport:?}: поле слева");
            assert!(r.y >= STAGE_MARGIN - 1e-3, "{viewport:?}: поле сверху");
            assert!(
                r.right() <= vw - STAGE_MARGIN + 1e-3,
                "{viewport:?}: поле справа"
            );
            assert!(
                r.bottom() <= vh - STAGE_MARGIN + 1e-3,
                "{viewport:?}: поле снизу"
            );
            assert!(
                r.w <= vw * MAIN_STAGE_MAX_FRACTION + 1e-3,
                "{viewport:?}: ≤70% ширины"
            );
            assert!(
                r.h <= vh * MAIN_STAGE_MAX_FRACTION + 1e-3,
                "{viewport:?}: ≤70% высоты"
            );
            assert!(
                (r.x + r.w / 2.0 - vw / 2.0).abs() < 1e-3,
                "{viewport:?}: центр по X"
            );
            assert!(
                (r.y + r.h / 2.0 - vh / 2.0).abs() < 1e-3,
                "{viewport:?}: центр по Y"
            );
        }
    }

    /// Инвариант 6: веер симметричен, шаг равен spacing, детерминирован;
    /// чёт/нечёт.
    #[test]
    fn fan_symmetry_and_spacing() {
        let s = stage_fan_spacing(4);
        let even = stage_edge_fan(4, s);
        assert_eq!(even.len(), 4);
        assert!((even[0] + even[3]).abs() < 1e-5 && (even[1] + even[2]).abs() < 1e-5);
        assert!((even[1] - even[0] - s).abs() < 1e-5, "шаг = spacing");
        let odd = stage_edge_fan(5, s);
        assert!(odd.iter().any(|o| o.abs() < 1e-6), "нечёт: центральная — 0");
        assert_eq!(stage_edge_fan(0, s).len(), 0);
        assert_eq!(even, stage_edge_fan(4, s), "детерминизм");
        assert!(s > bundle_thickness(4), "шаг больше толщины");
    }

    /// Инвариант 5/6: hit-test веера — попадание в свою линию (точки
    /// берутся с самих полилиний веера), промах мимо веера, допуск от
    /// толщины.
    #[test]
    fn stage_hit_test() {
        let mut slice = Canvas::default();
        slice.nodes.push(Node::text("a", "A", 0.0, 0.0));
        slice.nodes.push(Node::text("b", "B", 400.0, 0.0));
        let e = |id: &str| Edge::new(id, "a", None, "b", None);
        slice.add_edge(e("f1"));
        slice.add_edge(e("f2"));
        slice.add_edge(e("f3"));
        let spacing = stage_fan_spacing(3);
        let fan = stage_edge_fan(3, spacing);
        let normal = stage_fan_normal(&slice).expect("нормаль");
        assert!((normal[0].abs() - 0.0).abs() < 1e-4 && (normal[1] - 1.0).abs() < 1e-4);
        // Точка середины средней линии веера — попадание в индекс 1;
        // середина верхней — в индекс 2.
        let mid_of = |index: usize| {
            let points = stage_edge_points(&slice, index, fan[index], 24).expect("полилиния");
            let mid = points[points.len() / 2];
            (mid[0], mid[1])
        };
        let (cx, cy) = mid_of(1);
        assert_eq!(
            stage_edge_at(&slice, &fan, [cx, cy], 4.0),
            Some(1),
            "центральная"
        );
        let (ux, uy) = mid_of(2);
        assert_eq!(
            stage_edge_at(&slice, &fan, [ux, uy], 3.0),
            Some(2),
            "верхняя"
        );
        // Допуск от толщины: точка на d/2 от средней линии ловится при
        // допуске max(EDGE_HIT_TOLERANCE, d/2 + 2.0)
        let half = bundle_thickness(3) / 2.0 + 1.0;
        let (nx, ny) = mid_of(0);
        let _ = (nx, ny);
        let near = [cx + normal[0] * half, cy + normal[1] * half];
        assert!(
            stage_edge_at(&slice, &fan, near, half + 2.0).is_some(),
            "допуск от толщины"
        );
        // Мимо всех линий — далеко перпендикулярно оси
        assert_eq!(stage_edge_at(&slice, &fan, [cx, cy + 500.0], 6.0), None);
    }

    /// Раскладка stage: натуральный случай — масштаб 1, коридор между
    /// колонками; тесный viewport — содержимое сжато и умещается в rect
    /// (инвариант 7).
    #[test]
    fn stage_layout_fits() {
        let rect = main_stage_rect([1920.0, 1080.0]);
        let layout = stage_layout([320.0, 220.0], [320.0, 220.0], &rect);
        assert!((layout.scale - 1.0).abs() < 1e-4, "натуральный размер");
        assert!(layout.source_pos[0] > 0.0 && layout.target_pos[0] > layout.source_pos[0]);
        assert!(
            layout.target_pos[0] + 320.0 <= rect.w + 1e-3,
            "приёмник внутри rect"
        );
        assert!(layout.corridor.w > 0.0, "коридор есть");
        // Тесное окно: сжатие гарантирует умещение
        let tight = main_stage_rect([400.0, 300.0]);
        let tight_layout = stage_layout([320.0, 220.0], [320.0, 220.0], &tight);
        assert!(tight_layout.scale < 1.0, "сжатие потребовалось");
        let full_w = tight_layout.target_pos[0] + 320.0 * tight_layout.scale;
        assert!(full_w <= tight.w + 1e-3, "умещается по ширине");
        let max_h = 220.0 * tight_layout.scale;
        assert!(max_h <= tight.h + 1e-3, "умещается по высоте");
        // Детерминизм
        assert_eq!(
            stage_layout([320.0, 220.0], [320.0, 220.0], &tight),
            tight_layout
        );
    }
}
