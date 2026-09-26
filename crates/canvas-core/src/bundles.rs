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

/// Лейн-раскладка подписей значений веера (FR-044 Р-1, чистая функция;
/// владелец 2026-09-22: пилюля тянется к своей линии — элементы несут
/// `preferred_x` = x середины своего ребра, клампится в коридор).
///
/// - `items` — `(индекс ребра, ширина пилюли, высота пилюли, желаемый x
///   центра)` в порядке рёбер веера (стабильный порядок `edge.id` на
///   вызывающей стороне); `None` — центр коридора (как раньше);
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
    items: Vec<(usize, f32, f32, Option<f32>)>,
    corridor: Rect,
    clamp_zone: Rect,
    axis_y: f32,
) -> FanLabelLayout {
    let heights: Vec<f32> = items.iter().map(|(_, _, h, _)| *h).collect();
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
    for (item, w, h, preferred_x) in items {
        // Горизонталь: пилюля тянется к середине СВОЕЙ линии (прототип:
        // `clamp(mid.x, zL+w/2+4, zR-w/2-4)`), при перепашке — центр
        // коридора; затем кламп внутрь (инвариант 2).
        let natural_center = preferred_x.unwrap_or(corridor.x + corridor.w / 2.0);
        let m = 4.0;
        let lo = corridor.x + w / 2.0 + m;
        let hi = corridor.right() - w / 2.0 - m;
        let mut clamped = false;
        let mut x = if lo <= hi {
            let center = natural_center.clamp(lo, hi);
            clamped = (center - natural_center).abs() > f32::EPSILON;
            center - w / 2.0
        } else {
            // Перепашка: пилюля шире коридора — центр с клампом
            clamped |= preferred_x.is_some();
            corridor.x + (corridor.w - w) / 2.0
        };
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
/// Максимальная доля стороны вьюпорта (PRD-0002 §7.2: «≤ 70%»).
pub const MAIN_STAGE_MAX_FRACTION: f32 = 0.7;
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

/// Прямоугольник main stage (инвариант 4 FR-042): центрирован, обе стороны
/// = 70% соответствующей стороны вьюпорта (владелец, 2026-09-22: stage
/// должен масштабироваться ДО 70% поля видимости — абсолютные капы
/// 1280×800 сняты: на экранах крупнее 1830×1140 они сжимали stage ниже
/// ожидаемых 70%), целиком внутри окна с полем [`STAGE_MARGIN`] при любых
/// пропорциях.
pub fn main_stage_rect(viewport: [f32; 2]) -> Rect {
    let vw = viewport[0].max(0.0);
    let vh = viewport[1].max(0.0);
    let w = (vw * MAIN_STAGE_MAX_FRACTION)
        .min(vw - STAGE_MARGIN * 2.0)
        .max(1.0);
    let h = (vh * MAIN_STAGE_MAX_FRACTION)
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
        // Страховка от рассинхрона кэша (багфикс паники «index out of
        // bounds: len 19, index 19»): индексы пучка валидны для канваса
        // НА МОМЕНТ ПОСТРОЕНИЯ. Все пути мутаций перестраивают индекс в
        // recompute_flow (включая воркер-ветку), но любой непредвиденный
        // путь не должен ронять рендер — устаревший индекс вне диапазона
        // пропускается, как ребро без ранга.
        let rank = |index: usize| -> Option<u8> {
            let edge = canvas.edges.get(index)?;
            if edge.color.as_deref().is_some_and(|c| !c.is_empty()) {
                Some(2)
            } else if edge.flow_kind() == FlowKind::Value {
                Some(1)
            } else {
                Some(0)
            }
        };
        // Первый максимум: порядок обхода — `edge.id`, равный ранг НЕ
        // вытесняет предыдущего кандидата (инвариант 2 — стабильность).
        let mut best: Option<(u8, usize)> = None;
        for &index in &bundle.edges {
            let Some(r) = rank(index) else { continue };
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

/// Шаг веера внутри группы рёбер с одинаковыми якорями (прототип R7,
/// drawStage: `off = (i-(g-1)/2)*12` — ±12 px по вертикали).
pub const STAGE_FAN_GROUP_STEP_PX: f32 = 12.0;

/// Метрики анатомии карточки (PRD-0004), нужные геометрии строк значений.
/// Заполняется вызывающим из констант рендера (HEADER_HEIGHT/BODY_*),
/// чтобы рендер и геометрия веера не разъезжались; Default — значения
/// дизайн-токенов (`canvas_core::tokens`: CARD_HEADER_HEIGHT/TYPE_*).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageMetrics {
    /// Высота шапки карточки (HEADER_HEIGHT).
    pub header_h: f32,
    /// Отступ тела от шапки (BODY_TOP_GAP).
    pub body_top_gap: f32,
    /// Шаг строк тела (BODY_LINE_HEIGHT).
    pub body_line: f32,
    /// Высота строки результата (RESULT_LINE_HEIGHT).
    pub result_line: f32,
    /// Внутренний отступ карточки (BODY_PADDING).
    pub body_padding: f32,
    /// Доп. зазор полосы результата (strip = result_line + extra).
    pub strip_extra: f32,
}

impl Default for StageMetrics {
    fn default() -> Self {
        Self {
            header_h: 34.0,
            body_top_gap: 4.0,
            body_line: 20.0,
            result_line: 16.0,
            body_padding: 10.0,
            strip_extra: 6.0,
        }
    }
}

/// Якоря веера по рёбрам среза: `[i] = (точка истока, точка приёмника)`
/// в stage-локальных px ([`stage_edge_anchor_points`]).
pub type StageAnchors = Vec<([f32; 2], [f32; 2])>;

/// Линия веера stage: якоря (с групповым смещением), полилиния кривой и
/// середина (порядок/привязка пилюль). Единая геометрия рендера и hit-test'а.
#[derive(Debug, Clone, PartialEq)]
pub struct StageEdgeLine {
    /// Полилиния Безье (segments+1 точка), stage-локальные px.
    pub points: Vec<[f32; 2]>,
    /// Точка истока (правый край ноды-истока на строке значения).
    pub from: [f32; 2],
    /// Точка приёмника (левый край ноды-приёмника на строке параметра).
    pub to: [f32; 2],
    /// Середина кривой (t = 0.5) — порядок и привязка пилюль.
    pub mid: [f32; 2],
}

/// Центр вертикали строки `li` тела ноды (stage-локальные px).
fn stage_row_center(node: &crate::model::Node, metrics: &StageMetrics, li: usize) -> f32 {
    node.y
        + metrics.header_h
        + metrics.body_top_gap
        + li as f32 * metrics.body_line
        + metrics.body_line / 2.0
}

/// Вертикаль строки с клампом в диапазон отображаемых строк; строк нет —
/// фолбэк (центр полосы результата или центр ноды).
fn stage_row_y(
    node: &crate::model::Node,
    metrics: &StageMetrics,
    rows: usize,
    li: usize,
    fallback: f32,
) -> f32 {
    if rows == 0 {
        fallback
    } else {
        stage_row_center(node, metrics, li.min(rows - 1))
    }
}

/// Число отображаемых строк тела (та же формула, что в отрисовке карточки:
/// строки сверх высоты тела не рисуются). Не-текст/пустой текст — 0.
fn stage_row_count(node: &crate::model::Node, metrics: &StageMetrics, has_footer: bool) -> usize {
    if node.kind() != crate::model::NodeKind::Text {
        return 0;
    }
    let text_lines = node.text.as_deref().map(|t| t.lines().count()).unwrap_or(0);
    if text_lines == 0 {
        return 0;
    }
    let footer_h = if has_footer {
        metrics.result_line + metrics.strip_extra
    } else {
        0.0
    };
    let avail_h =
        (node.height - metrics.header_h - metrics.body_top_gap - metrics.body_padding - footer_h)
            .max(0.0);
    let max_rows = ((avail_h / metrics.body_line).floor() as usize).max(1);
    text_lines.min(max_rows)
}

/// Центр полосы результата (футера), если он есть.
fn stage_strip_center(
    node: &crate::model::Node,
    metrics: &StageMetrics,
    has_footer: bool,
) -> Option<f32> {
    if !has_footer {
        return None;
    }
    let strip_h = metrics.result_line + metrics.strip_extra;
    Some(node.y + node.height - metrics.body_padding - strip_h / 2.0)
}

/// Строка присваивания `name = …` в Numi-листе (зеркало
/// `canvas_scene::assignment_line` — ядро не зависит от scene).
fn assignment_line_of(text: &str, name: &str) -> Option<usize> {
    text.split('\n').position(|line| {
        line.split_once('=')
            .map(|(n, _)| n.trim() == name)
            .unwrap_or(false)
    })
}

/// Точная привязка рёбер среза к строкам значений (владелец, 2026-09-22:
/// «точки выходов/входов — к строкам, на которых значения»; прототип
/// `portPos`: порты сидят на вертикали своих строк).
///
/// - исток: `from_line` → центр той строки; `from_output` → строка
///   присваивания переменной; иначе — центр полосы результата (есть футер)
///   либо равномерное распределение по строкам;
/// - приёмник: `to_param` → строка присваивания параметра; без параметра —
///   равномерное распределение по строкам (позиционные слоты `$N`);
/// - строк нет (не-текст/пусто) — центр ноды по вертикали.
///
/// Якорь истока — правый край ноды-истока, приёмника — левый край
/// ноды-приёмника. Детерминировано: порядок распределения — порядок рёбер
/// среза (стабилен по `edge.id` на вызывающей стороне).
pub fn stage_edge_anchor_points(
    slice: &Canvas,
    metrics: &StageMetrics,
    footers: [bool; 2],
) -> StageAnchors {
    let (Some(src), Some(dst)) = (slice.nodes.first(), slice.nodes.get(1)) else {
        return Vec::new();
    };
    let src_cy = src.y + src.height / 2.0;
    let dst_cy = dst.y + dst.height / 2.0;
    let src_rows = stage_row_count(src, metrics, footers[0]);
    let dst_rows = stage_row_count(dst, metrics, footers[1]);
    let src_strip = stage_strip_center(src, metrics, footers[0]);
    let src_text = src.text.as_deref().unwrap_or_default();
    let dst_text = dst.text.as_deref().unwrap_or_default();

    let row_y = |node: &crate::model::Node, rows: usize, li: usize, fallback: f32| {
        stage_row_y(node, metrics, rows, li, fallback)
    };
    // Распределение k рёбер без точной строки по полосе строк (прототип:
    // `rt + span*(i+0.5)/n`): i-е из k → строка floor((i+0.5)*rows/k).
    let distribute = |node: &crate::model::Node, k: usize, i: usize, rows: usize, fallback: f32| {
        if rows == 0 {
            return fallback;
        }
        let idx = (((i as f32 + 0.5) * rows as f32) / k.max(1) as f32).floor() as usize;
        stage_row_center(node, metrics, idx.min(rows - 1))
    };

    // 1) Точные якоря истока (from_line/from_output); остальное — в очередь
    let mut src_y: Vec<Option<f32>> = Vec::with_capacity(slice.edges.len());
    let mut src_queue: Vec<usize> = Vec::new();
    for (i, edge) in slice.edges.iter().enumerate() {
        let exact = if let Some(line) = edge.from_line {
            Some(row_y(src, src_rows, line, src_cy))
        } else if let Some(output) = edge.from_output.as_deref() {
            assignment_line_of(src_text, output)
                .map(|li| row_y(src, src_rows, li, src_cy))
                .or(src_strip)
        } else {
            src_strip
        };
        if exact.is_some() {
            src_y.push(exact);
        } else {
            src_y.push(None);
            src_queue.push(i);
        }
    }
    for (k, &i) in src_queue.iter().enumerate() {
        src_y[i] = Some(distribute(src, src_queue.len(), k, src_rows, src_cy));
    }

    // 2) Приёмник: to_param → строка присваивания; остальное — в очередь
    let mut dst_y: Vec<Option<f32>> = Vec::with_capacity(slice.edges.len());
    let mut dst_queue: Vec<usize> = Vec::new();
    for (i, edge) in slice.edges.iter().enumerate() {
        let exact = edge
            .to_param
            .as_deref()
            .and_then(|param| assignment_line_of(dst_text, param))
            .map(|li| row_y(dst, dst_rows, li, dst_cy));
        if exact.is_some() {
            dst_y.push(exact);
        } else {
            dst_y.push(None);
            dst_queue.push(i);
        }
    }
    for (k, &i) in dst_queue.iter().enumerate() {
        dst_y[i] = Some(distribute(dst, dst_queue.len(), k, dst_rows, dst_cy));
    }

    // 3) Якоря: исток — правый край, приёмник — левый край (прототип portPos)
    slice
        .edges
        .iter()
        .enumerate()
        .map(|(i, _)| {
            (
                [src.x + src.width, src_y[i].unwrap_or(src_cy)],
                [dst.x, dst_y[i].unwrap_or(dst_cy)],
            )
        })
        .collect()
}

/// Линии веера из якорей ([`stage_edge_anchor_points`]): рёбра с
/// ОДИНАКОВЫМИ якорями расходятся шагом [`STAGE_FAN_GROUP_STEP_PX`]
/// по вертикали (прототип: смещение в группе одного порта), кривая —
/// кубическая Безье с горизонтальными плечами
/// `dx = clamp(0.45·длины, 40, 130)` (прототип drawStage). Детерминировано.
pub fn stage_edge_lines(
    slice: &Canvas,
    anchors: &StageAnchors,
    segments: usize,
) -> Vec<StageEdgeLine> {
    if slice.edges.is_empty() {
        return Vec::new();
    }
    // Группы одинаковых якорей: ключ — округлённая пара вертикалей (×2 →
    // полупиксельная точность, целочисленный ключ без Float-хэша).
    let key = |a: &[f32; 2], b: &[f32; 2]| -> (i64, i64) {
        ((a[1] * 2.0).round() as i64, (b[1] * 2.0).round() as i64)
    };
    let mut groups: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, (a, b)) in anchors.iter().enumerate().take(slice.edges.len()) {
        groups.entry(key(a, b)).or_default().push(i);
    }
    let mut out = Vec::with_capacity(slice.edges.len());
    for (i, (a, b)) in anchors.iter().enumerate().take(slice.edges.len()) {
        let group = &groups[&key(a, b)];
        let pos = group.iter().position(|&g| g == i).unwrap_or(0);
        let dy = (pos as f32 - (group.len() as f32 - 1.0) / 2.0) * STAGE_FAN_GROUP_STEP_PX;
        let p0 = [a[0], a[1] + dy];
        let p3 = [b[0], b[1] + dy];
        let dx = ((p3[0] - p0[0]).abs() * 0.45).clamp(40.0, 130.0);
        let curve = crate::CubicBezier {
            p0,
            c0: [p0[0] + dx, p0[1]],
            c1: [p3[0] - dx, p3[1]],
            p1: p3,
        };
        let mid = crate::curve_point(&curve, 0.5);
        out.push(StageEdgeLine {
            points: crate::tessellate(&curve, segments.max(1)),
            from: p0,
            to: p3,
            mid,
        });
    }
    out
}

/// Полные линии веера среза: якоря + кривые одной причиной (удобство для
/// рендера и hit-test'а — одна и та же геометрия на кадре).
pub fn stage_edge_geometry(
    slice: &Canvas,
    metrics: &StageMetrics,
    footers: [bool; 2],
    segments: usize,
) -> Vec<StageEdgeLine> {
    let anchors = stage_edge_anchor_points(slice, metrics, footers);
    stage_edge_lines(slice, &anchors, segments)
}

/// Hit-test линий веера (FR-042 §Changes-4, ввод внутри stage): индекс
/// среза с минимальной дистанцией в допуске; None — промах.
pub fn stage_edge_at_lines(
    lines: &[StageEdgeLine],
    point: [f32; 2],
    tolerance: f32,
) -> Option<usize> {
    let mut best: Option<(f32, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let mut dist = f32::INFINITY;
        for w in line.points.windows(2) {
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

    fn demo_items(n: usize) -> Vec<(usize, f32, f32, Option<f32>)> {
        (0..n).map(|i| (i, PILL_W, PILL_H, None)).collect()
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
        let layout = stage_fan_label_layout(
            vec![(0, 500.0, PILL_H, None)],
            corridor,
            zone(0.0, 400.0),
            200.0,
        );
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

    /// preferred_x: пилюля тянется к x середины СВОЕЙ линии, кламп в
    /// коридор; без желаемого — центр коридора (прежнее поведение).
    #[test]
    fn fan_layout_preferred_x_follows_edge_mid() {
        let corridor = corridor_demo(); // x=100, w=500 → центр 350
                                        // Три пилюли с разными mid.x
        let items = vec![
            (0, PILL_W, PILL_H, Some(200.0)),
            (1, PILL_W, PILL_H, Some(350.0)),
            (2, PILL_W, PILL_H, Some(900.0)), // за пределами — кламп вправо
        ];
        let layout = stage_fan_label_layout(items, corridor, zone(0.0, 400.0), 200.0);
        let cx = |p: &FanPill| p.rect.x + p.rect.w / 2.0;
        assert!(
            (cx(&layout.pills[0]) - 200.0).abs() < 0.01,
            "по своей линии"
        );
        assert!((cx(&layout.pills[1]) - 350.0).abs() < 0.01, "центр совпал");
        assert!(
            cx(&layout.pills[2]) <= corridor.right() - 4.0 + 0.01,
            "кламп вправо"
        );
        assert!(layout.pills[2].clamped, "кламп зафиксирован");
        // Без preferred — центр коридора
        let plain = stage_fan_label_layout(demo_items(1), corridor, zone(0.0, 400.0), 200.0);
        assert!((cx(&plain.pills[0]) - 350.0).abs() < 0.01);
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

    /// Багфикс паники «index out of bounds: len 19, index 19»: устаревший
    /// индекс пучка (канвас укоротился после построения индекса) НЕ роняет
    /// dominant_edge — индекс вне диапазона пропускается, доминанта
    /// выбирается среди оставшихся валидных, пустой валидный набор — None.
    #[test]
    fn dominant_edge_stale_index_does_not_panic() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "A", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "B", 400.0, 0.0));
        let e = |id: &str| Edge::new(id, "a", None, "b", None);
        canvas.add_edge(e("e1"));
        canvas.add_edge(e("e2"));
        canvas.add_edge(e("e3"));
        let index = EdgeBundleIndex::build(&canvas);
        let bundle = index.bundle_of_pair("a", "b").expect("пучок");
        assert_eq!(index.dominant_edge(&canvas, bundle), Some(0));
        // Рассинхрон: пучок помнит индекс 3, канвас уже укоротили до 3 рёбер
        // (после удаления; len = 3 — индекс 3 вне диапазона). Именно этот
        // путь паниковал: `canvas.edges[index]` в воркер-окне FR-064.
        let stale = EdgeBundle {
            edges: vec![0, 3],
            weight: 2,
        };
        assert_eq!(
            index.dominant_edge(&canvas, &stale),
            Some(0),
            "устаревший индекс пропущен, доминанта — среди валидных"
        );
        let all_stale = EdgeBundle {
            edges: vec![7, 9],
            weight: 2,
        };
        assert_eq!(
            index.dominant_edge(&canvas, &all_stale),
            None,
            "нет валидных индексов — честный None, не паника"
        );
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

    /// Веер (новая геометрия, владелец 2026-09-22): якоря на строках
    /// значений, группы одинаковых якорей расходятся ±12 px, кривые
    /// детерминированы, hit-test ловит свою линию.
    #[test]
    fn stage_anchors_rows_groups_and_hit() {
        let mut slice = Canvas::default();
        // Исток: 2 строки значений («10 $» / «4 $»); приёмник: «$1 - $2»
        let mut a = Node::text("a", "10 $\n4 $", 0.0, 0.0);
        a.width = 220.0;
        a.height = 120.0;
        let mut b = Node::text("b", "$1 - $2", 500.0, 0.0);
        b.width = 250.0;
        b.height = 140.0;
        slice.nodes.push(a);
        slice.nodes.push(b);
        let mut e1 = Edge::new("f1", "a", None, "b", None);
        e1.from_line = Some(0);
        let mut e2 = Edge::new("f2", "a", None, "b", None);
        e2.from_line = Some(1);
        let e3 = Edge::new("f3", "a", None, "b", None); // без строки — распределение
        slice.add_edge(e1);
        slice.add_edge(e2);
        slice.add_edge(e3);
        let metrics = StageMetrics::default();
        let anchors = stage_edge_anchor_points(&slice, &metrics, [false, false]);
        assert_eq!(anchors.len(), 3);
        // Якорь истока — правый край; from_line=0/1 сидят на центрах строк
        let src_right = slice.nodes[0].x + slice.nodes[0].width;
        assert!((anchors[0].0[0] - src_right).abs() < 1e-4);
        let row0 = stage_row_center(&slice.nodes[0], &metrics, 0);
        let row1 = stage_row_center(&slice.nodes[0], &metrics, 1);
        assert!((anchors[0].0[1] - row0).abs() < 1e-4, "строка 0");
        assert!((anchors[1].0[1] - row1).abs() < 1e-4, "строка 1");
        // Ребро без from_line: футера нет → распределение по строкам
        let dist_y = anchors[2].0[1];
        assert!(
            (dist_y - row0).abs() < 1e-4 || (dist_y - row1).abs() < 1e-4,
            "распределение попало в строку значения"
        );
        // Приёмник: 3 ребра без to_param → 2 строки тела, 3 ребра → группы
        // по строкам: первое и третье совпадут (floor), якоря различны
        let dst_left = slice.nodes[1].x;
        for (a, b) in &anchors {
            assert!((b[0] - dst_left).abs() < 1e-4, "левый край приёмника");
            let _ = a;
        }
        // Линии: одинаковые якоря расходятся ±12 px (группы)
        let lines = stage_edge_lines(&slice, &anchors, 24);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert!(
                (line.points[0][0] - src_right).abs() < 1e-4,
                "старт у порта истока"
            );
            let last = line.points.last().expect("точки");
            assert!((last[0] - dst_left).abs() < 1e-4, "финиш у порта приёмника");
        }
        // Рёбра 0 и 2 попадают в одну строку приёмника при распределении —
        // их якоря различны по вертикали хотя бы с одной стороны (группы/строки)
        let distinct = lines
            .iter()
            .map(|l| (l.from[1], l.to[1]))
            .collect::<Vec<_>>();
        assert!(
            distinct[0] != distinct[1] || distinct[0] != distinct[2],
            "веер разводит линии по вертикалям"
        );
        // Hit-test: точка середины линии 1 ловит индекс 1
        let mid = lines[1].mid;
        assert_eq!(stage_edge_at_lines(&lines, mid, 4.0), Some(1));
        // Мимо всех линий
        assert_eq!(
            stage_edge_at_lines(&lines, [mid[0], mid[1] + 500.0], 6.0),
            None
        );
        // Детерминизм
        let again = stage_edge_lines(&slice, &anchors, 24);
        assert_eq!(lines, again);
    }

    /// Привязка приёмника к параметру: to_param сидит на строке присваивания
    /// параметра в Numi-листе приёмника (прототип: вход на строке значения).
    #[test]
    fn stage_anchor_target_param_row() {
        let mut slice = Canvas::default();
        let mut a = Node::text("a", "5", 0.0, 0.0);
        a.width = 200.0;
        a.height = 100.0;
        // Приёмник: параметр rps присваивается на второй строке (индекс 1)
        let mut b = Node::text("b", "цена = 9 $\nrps = $in\nитог = rps * 2", 400.0, 0.0);
        b.width = 300.0;
        b.height = 200.0;
        slice.nodes.push(a);
        slice.nodes.push(b);
        let mut e = Edge::new("f1", "a", None, "b", None);
        e.to_param = Some("rps".to_owned());
        slice.add_edge(e);
        let metrics = StageMetrics::default();
        let anchors = stage_edge_anchor_points(&slice, &metrics, [false, false]);
        // строка 1: header 34 + gap 4 + 1*20 + 10 = 68
        assert!(
            (anchors[0].1[1] - 68.0).abs() < 1e-4,
            "якорь на строке параметра"
        );
    }

    /// Без строк и футера якоря — центры сторон нод; одинаковые якоря
    /// расходятся группой ±12 px (пучок из 3 → -12/0/+12).
    #[test]
    fn stage_fan_group_offsets_on_identical_anchors() {
        let mut slice = Canvas::default();
        let mut a = Node::text("a", "A", 0.0, 0.0);
        a.width = 200.0;
        a.height = 100.0;
        let mut b = Node::text("b", "B", 400.0, 0.0);
        b.width = 200.0;
        b.height = 100.0;
        slice.nodes.push(a);
        slice.nodes.push(b);
        for id in ["f1", "f2", "f3"] {
            slice.add_edge(Edge::new(id, "a", None, "b", None));
        }
        let metrics = StageMetrics::default();
        let lines = stage_edge_geometry(&slice, &metrics, [false, false], 24);
        let ys: Vec<f32> = lines.iter().map(|l| l.from[1]).collect();
        // Одиночная строка «A»: центр строки = 34 (шапка) + 4 (gap) + 10 = 48
        assert!((ys[0] - (ys[1] - 12.0)).abs() < 1e-4, "-12");
        assert!(
            (ys[1] - 48.0).abs() < 1e-4,
            "якорь на центре единственной строки"
        );
        assert!((ys[2] - (ys[1] + 12.0)).abs() < 1e-4, "+12");
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
