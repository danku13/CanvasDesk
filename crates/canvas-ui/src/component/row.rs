//! FR-068 W3: строка табличного тела (FR-061: RowGuides/row_layout/paint_row) — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-d): дополнить `impl Component` для строки + каталог
//! миграции потребителей (см. worklog); функции — стабильный API.

use std::cell::RefCell;

use super::{Component, KitPalette, KitState};
use crate::geometry::UiRect;
use crate::layout::LayoutBackend;
use crate::measure::TextMeasurer;
use crate::paint::{PaintAlign, Painter};
use crate::row_guides::{RowCellWidths, RowGuides};
use crate::widget::WidgetState;

// --- Row (FR-061 D-15, этап E) ----------------------------------------------
//
// Кит-строка табличного тела: декларативные данные ([`RowParts`]) +
// колоночные направляющие ([`RowGuides`], D-3) → геометрия ([`row_layout`])
// + стиль из слотов ([`row_style`]) → отрисовка ([`paint_row`], Painter —
// FR-057). Потребители дают данные, каркас считает геометрию (правило
// FR-061): значение/юнит прижаты вправо на направляющих (D-4), лидер —
// пунктирная дорожка от конца левого текста до направляющей чисел (D-5;
// штрихи — [`leader_dash_rects`], единая геометрия с телом ноды).

/// Высота линейной коробки текста для вертикальной центровки:
/// `size * ROW_LINE_FRAC` — практика потребителей (панель FR-044:
/// коробка 12 px при кегле 11).
pub const ROW_LINE_FRAC: f32 = 1.1;
/// Отступ точки-маркера от левого края строки (панель FR-044, Р-4).
pub const ROW_DOT_PAD: f32 = 6.0;
/// Диаметр точки-маркера.
pub const ROW_DOT: f32 = 6.0;
/// Зазор «маркер → левый текст» / «левый текст → значение».
pub const ROW_TEXT_GAP: f32 = 6.0;
/// Минимальная ширина зоны глифа-маркера («ƒ»). Глиф уже зоны — зона
/// фиксируется, чтобы колонки строк не «дышали».
pub const ROW_GLYPH_MIN_W: f32 = 12.0;
/// Зазор «зона глифа → левый текст».
pub const ROW_GLYPH_GAP: f32 = 2.0;
/// Горизонтальный пад пилюли бейджа (этап E: бейдж — пилюля — примечание
/// D-14 «пилюля отложена в этап E (kit-Row)»). Ширина бейдж-колонки —
/// текст + 2·[`ROW_BADGE_PAD_H`].
pub const ROW_BADGE_PAD_H: f32 = 6.0;

/// Маркер левой колонки строки (прототип Р-4: value-точка / «ƒ»). Глиф —
/// существующим шрифтом (G7), ширина зоны — TextMeasurer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowMarker {
    /// Без маркера — левый текст от края строки.
    None,
    /// Круглая точка (value-маркер переменных).
    Dot,
    /// Глиф существующим шрифтом («ƒ» формул).
    Glyph(&'static str),
}

/// Данные строки таблицы (декларативно): потребители дают данные, каркас
/// ([`row_guides`]/[`row_layout`]) считает геометрию. Пустое значение —
/// ячейка не рисуется и лидера нет (строки-формулы панели FR-044).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowParts<'a> {
    pub marker: RowMarker,
    /// Левый текст (имя/формула/путь) — усекается ellipsis'ом (класс CR-015).
    pub label: &'a str,
    /// Значение (прижато вправо на направляющей чисел).
    pub value: &'a str,
    /// Юнит (прижат вправо на направляющей юнитов; пусто — скаляр).
    pub unit: &'a str,
    /// Бейдж (пилюля в бейдж-колонке; пусто — колонки нет).
    pub badge: &'a str,
}

/// Стиль строки — только слоты (контракт F-8). Поля — plain data:
/// потребитель переопределяет конкретные поля семантикой своей поверхности
/// (value-точка/ошибка/приглушение — как в панели FR-044); скрытой
/// арифметики над цветами в ките нет.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub marker: [f32; 4],
    pub label: [f32; 4],
    pub value: [f32; 4],
    pub unit: [f32; 4],
    /// Цвет текста бейджа (пилюля — [`RowStyle::badge_fill`]/[`RowStyle::badge_border`]).
    pub badge: [f32; 4],
    pub badge_fill: [f32; 4],
    pub badge_border: [f32; 4],
}

/// Стиль строки из слотов состояний: Normal — прозрачная строка (фон/зебру
/// решает потребитель — слотами своей темы), Hovered — слот hover,
/// Selected — слот selected + рамка accent, Disabled — приглушённые тексты.
pub fn row_style(state: KitState, p: &KitPalette) -> RowStyle {
    let transparent = [0.0; 4];
    match state {
        KitState::Disabled => RowStyle {
            fill: transparent,
            border: transparent,
            marker: p.disabled_text,
            label: p.disabled_text,
            value: p.disabled_text,
            unit: p.disabled_text,
            badge: p.disabled_text,
            badge_fill: transparent,
            badge_border: transparent,
        },
        KitState::Selected => RowStyle {
            fill: p.selected_fill,
            border: p.accent,
            marker: p.text,
            label: p.text,
            value: p.text,
            unit: p.text_muted,
            badge: p.text,
            badge_fill: p.control_fill,
            badge_border: p.control_border,
        },
        KitState::Hovered => RowStyle {
            fill: p.hover_fill,
            ..row_style(KitState::Normal, p)
        },
        _ => RowStyle {
            fill: transparent,
            border: transparent,
            marker: p.text,
            label: p.text,
            value: p.text,
            unit: p.text_muted,
            badge: p.text,
            badge_fill: p.control_fill,
            badge_border: p.control_border,
        },
    }
}

/// Опции строки (не-дефолтное поведение).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowOpts {
    /// Строить дорожку лидера между левой частью и значением. Панель
    /// «Как считается» (прототип FR-044 без лидера) — выключает; тело ноды
    /// и витрина — дефолт.
    pub leader: bool,
    /// Зазор между соседними ячейками (параметр потребителя — контракт
    /// [`RowGuides::with_right_edge`]; дефолт — токен TABLE_GUIDE_GAP).
    pub gap: f32,
}

impl Default for RowOpts {
    fn default() -> Self {
        Self {
            leader: true,
            gap: canvas_core::tokens::TABLE_GUIDE_GAP,
        }
    }
}

/// Геометрия строки на направляющих ([`row_layout`]): rect'ы ячеек в
/// координатах потребителя. Тексты уже усечены ([`RowLayout::label_shown`],
/// политика Ellipsis); value/unit — area'ы под фактический текст (прижат
/// вправо: left = right − width, D-4).
#[derive(Debug, Clone, PartialEq)]
pub struct RowLayout {
    /// Слот строки (фон/зебра — на всю ширину).
    pub row: UiRect,
    /// Точка-маркер ([`RowMarker::Dot`]) — круг (радиус = w/2).
    pub dot: Option<UiRect>,
    /// Зона глифа-маркера ([`RowMarker::Glyph`]).
    pub glyph: Option<UiRect>,
    /// Зона левого текста (усечённого — [`RowLayout::label_shown`]).
    pub label: UiRect,
    /// Левый текст после ellipsis (последняя точка усечения — CR-015).
    pub label_shown: String,
    /// Дорожка лидера (x0, x1) на [`RowLayout::leader_y`]; `None` — значения
    /// нет / лидер выключен / дорожка короче TABLE_LEADER_MIN.
    pub leader: Option<(f32, f32)>,
    /// Вертикаль дорожки лидера (доля строки TABLE_LEADER_Y_FRAC — как в
    /// теле ноды).
    pub leader_y: f32,
    /// Ячейка значения (area уже под фактический текст).
    pub value: UiRect,
    /// Ячейка юнита.
    pub unit: UiRect,
    /// Пилюля бейджа.
    pub badge: Option<UiRect>,
}

/// Проход A кит-строк: замер ячеек по всем строкам ([`RowGuides::measure`])
/// и проход B от правого края ([`RowGuides::with_right_edge`]). Бейдж —
/// пилюля: текст + 2·[`ROW_BADGE_PAD_H`]. `None` — строк нет.
#[allow(clippy::too_many_arguments)]
pub fn row_guides(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
    rows: &[RowParts<'_>],
    right_edge: f32,
    gap: f32,
) -> Option<RowGuides> {
    let widths: Vec<RowCellWidths> = rows
        .iter()
        .map(|row| RowCellWidths {
            value_w: if row.value.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.value, family, size)
            },
            unit_w: if row.unit.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.unit, family, size)
            },
            badge_w: if row.badge.is_empty() {
                0.0
            } else {
                m.width_of(fs, row.badge, family, size) + 2.0 * ROW_BADGE_PAD_H
            },
        })
        .collect();
    RowGuides::measure(&widths).map(|g| g.with_right_edge(right_edge, gap))
}

/// Геометрия одной строки: маркер, левый текст (ellipsis до ячейки
/// значения), лидер (D-5), value/unit на направляющих (D-4), пилюля бейджа.
/// Ширины ячеек направляющих — вход [`RowGuides`] (проход A — [`row_guides`]
/// или собственный проход потребителя, как в теле ноды).
#[allow(clippy::too_many_arguments)]
pub fn row_layout(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
    slot: UiRect,
    guides: RowGuides,
    parts: &RowParts<'_>,
    opts: &RowOpts,
) -> RowLayout {
    let text_y = slot.y + (slot.h - size * ROW_LINE_FRAC) / 2.0;
    let (dot, glyph, label_x) = match parts.marker {
        RowMarker::Dot => (
            Some(UiRect::new(
                slot.x + ROW_DOT_PAD,
                slot.y + (slot.h - ROW_DOT) / 2.0,
                ROW_DOT,
                ROW_DOT,
            )),
            None,
            slot.x + ROW_DOT_PAD + ROW_DOT + ROW_TEXT_GAP,
        ),
        RowMarker::Glyph(glyph) => {
            let glyph_w = if glyph.is_empty() {
                0.0
            } else {
                m.width_of(fs, glyph, family, size)
            };
            let zone_w = glyph_w.max(ROW_GLYPH_MIN_W);
            (
                None,
                Some(UiRect::new(slot.x + ROW_DOT_PAD, text_y, zone_w, slot.h)),
                slot.x + ROW_DOT_PAD + zone_w + ROW_GLYPH_GAP,
            )
        }
        RowMarker::None => (None, None, slot.x),
    };
    let value_w = if parts.value.is_empty() {
        0.0
    } else {
        m.width_of(fs, parts.value, family, size)
    };
    let unit_w = if parts.unit.is_empty() {
        0.0
    } else {
        m.width_of(fs, parts.unit, family, size)
    };
    let value = if parts.value.is_empty() {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    } else {
        UiRect::new(guides.value_right() - value_w, text_y, value_w, slot.h)
    };
    let unit = if parts.unit.is_empty() {
        UiRect::new(0.0, 0.0, 0.0, 0.0)
    } else {
        UiRect::new(guides.unit_right() - unit_w, text_y, unit_w, slot.h)
    };
    let badge = if parts.badge.is_empty() {
        None
    } else {
        let pill_h = (size * ROW_LINE_FRAC + 6.0).min(slot.h);
        Some(UiRect::new(
            guides.unit_right() + opts.gap,
            slot.y + (slot.h - pill_h) / 2.0,
            guides.badge_w,
            pill_h,
        ))
    };
    // Левый текст: до ячейки значения / юнита / края строки (что первое) —
    // колонка левых текстов стабильна по ноде (табличная семантика §3.1).
    let label_right = if !parts.value.is_empty() {
        guides.value_x - ROW_TEXT_GAP
    } else if !parts.unit.is_empty() {
        guides.unit_x - ROW_TEXT_GAP
    } else {
        slot.right() - ROW_TEXT_GAP
    };
    let label_w = (label_right - label_x).max(0.0);
    let label_shown = m.ellipsis(fs, parts.label, family, size, label_w);
    let label = UiRect::new(label_x, text_y, label_w, slot.h);
    // Лидер: от конца фактического текста до направляющей чисел (D-5 —
    // как в теле ноды: left_end + PAD → value_right − PAD).
    let leader = if opts.leader && !parts.value.is_empty() && !label_shown.is_empty() {
        let x0 = label.x
            + m.width_of(fs, &label_shown, family, size)
            + canvas_core::tokens::TABLE_LEADER_PAD;
        let x1 = guides.value_right() - canvas_core::tokens::TABLE_LEADER_PAD;
        if x1 - x0 >= canvas_core::tokens::TABLE_LEADER_MIN {
            Some((x0, x1))
        } else {
            None
        }
    } else {
        None
    };
    RowLayout {
        row: slot,
        dot,
        glyph,
        label,
        label_shown,
        leader,
        leader_y: slot.y + slot.h * canvas_core::tokens::TABLE_LEADER_Y_FRAC,
        value,
        unit,
        badge,
    }
}

/// Отрисовка строки в Painter (FR-057): фон/рамка → маркер → левый текст →
/// лидер → значение → юнит → бейдж (порядок = draw-порядок). Ширины и
/// усечение уже посчитаны в [`row_layout`] — здесь только место и стиль
/// (контракт Painter::Text).
pub fn paint_row(p: &mut Painter, lay: &RowLayout, parts: &RowParts<'_>, s: &RowStyle, size: f32) {
    p.rect(lay.row, s.fill, s.border, canvas_core::tokens::RADIUS_CHIP);
    if let Some(dot) = lay.dot {
        p.rect(dot, s.marker, [0.0; 4], dot.w / 2.0);
    }
    if let (Some(zone), RowMarker::Glyph(glyph)) = (lay.glyph, parts.marker) {
        if !glyph.is_empty() {
            p.label(zone, glyph, s.marker, size, PaintAlign::Left);
        }
    }
    p.label(lay.label, &lay.label_shown, s.label, size, PaintAlign::Left);
    if let Some((x0, x1)) = lay.leader {
        for dash in leader_dash_rects(
            x0,
            x1,
            lay.leader_y,
            1.0,
            canvas_core::tokens::TABLE_LEADER_MIN,
        ) {
            p.rect(
                UiRect::new(dash[0], dash[1], dash[2], dash[3]),
                s.marker,
                [0.0; 4],
                0.0,
            );
        }
    }
    if lay.value.w > 0.0 {
        p.label(lay.value, parts.value, s.value, size, PaintAlign::Left);
    }
    if lay.unit.w > 0.0 {
        p.label(lay.unit, parts.unit, s.unit, size, PaintAlign::Left);
    }
    if let Some(pill) = lay.badge {
        p.rect(pill, s.badge_fill, s.badge_border, pill.h / 2.0);
        let text = UiRect::new(
            pill.x + ROW_BADGE_PAD_H,
            pill.y + (pill.h - size * ROW_LINE_FRAC) / 2.0,
            (pill.w - 2.0 * ROW_BADGE_PAD_H).max(0.0),
            pill.h,
        );
        p.label(text, parts.badge, s.badge, size, PaintAlign::Left);
    }
}

/// Штрихи лидера (D-5) — ЕДИНАЯ геометрия с телом ноды
/// (`canvas-render/text.rs`): токены TABLE_LEADER_{DASH,GAP,H}, штрихи
/// высотой `TABLE_LEADER_H·scale` на ординате `y` от `x0` до `x1`;
/// дорожка короче `min_track·scale` — штрихов нет (тело ноды — 6 px
/// исторически; кит — TABLE_LEADER_MIN).
pub fn leader_dash_rects(x0: f32, x1: f32, y: f32, scale: f32, min_track: f32) -> Vec<[f32; 4]> {
    let dash = canvas_core::tokens::TABLE_LEADER_DASH * scale;
    let step =
        (canvas_core::tokens::TABLE_LEADER_DASH + canvas_core::tokens::TABLE_LEADER_GAP) * scale;
    let h = canvas_core::tokens::TABLE_LEADER_H * scale;
    let mut out = Vec::new();
    if x1 - x0 >= min_track * scale {
        let mut x = x0;
        while x + dash <= x1 {
            out.push([x, y, dash, h]);
            x += step;
        }
    }
    out
}

// --- FR-068 W3: компонент Row (Component-слой поверх kit-функций) ------------
//
// Компонент — retained-объект: владеет свойствами ([`RowProps`]), машиной
// состояний ([`WidgetState`]) и замерщиком текста ([`TextMeasurer`] +
// `cosmic_text::FontSystem` — владелец инстанса компонент, практика
// measure.rs §14/Q6: общий пул потребителя не churn'ится). Вёрстка/отрисовка
// — immediate-делегирование в kit-функции ([`row_guides`]/[`row_layout`]/
// [`paint_row`]) — побитовый паритет с прямым вызовом (тест-оракул в
// `mod tests`).

/// Кегль кит-строки компонента (захардкожен как в тестах `component/*` —
/// `ROW_SIZE` 12.0; не-дефолтный кегль — расширение `RowProps` в v2).
pub const ROW_DEFAULT_SIZE: f32 = 12.0;

/// Семейство кит-строки компонента (паритет `FAMILY` тестов `component/*`;
/// то же строковое имя, что `Family::Name` рендера).
pub const ROW_DEFAULT_FAMILY: &str = "Noto Sans Display";

/// Свойства компонента [`Row`] (декларативный вход кадра). Строки владеет
/// компонент (`String`) — Props переживают кадр (retained-контракт W3).
/// Юнит-колонка вне Props v1 (пустая ячейка: потребители юнитов — тело ноды
/// FR-061 — остаются на kit-функциях); кегль/семейство — константы
/// компонента ([`ROW_DEFAULT_SIZE`]/[`ROW_DEFAULT_FAMILY`], паритет тестам
/// 12.0 / "Noto Sans Display").
#[derive(Debug, Clone, PartialEq)]
pub struct RowProps {
    /// Маркер левой колонки (точка/глиф/нет — прототип Р-4).
    pub marker: RowMarker,
    /// Левый текст (имя/формула/путь) — усекается ellipsis'ом (класс CR-015).
    pub label: String,
    /// Значение (прижато вправо на направляющей чисел, D-4); пустое —
    /// ячейки нет и лидера нет.
    pub value: String,
    /// Бейдж (пилюля в бейдж-колонке; `None` — колонки нет).
    pub badge: Option<String>,
    /// Опции строки (лидер/зазор направляющих).
    pub opts: RowOpts,
    /// Палитра-срез (слоты состояний FR-053; контракт F-8 — без арифметики
    /// над цветами, стиль выбирает [`row_style`]).
    pub palette: KitPalette,
}

/// Компонент строки табличного тела (FR-061 D-15): retained-объект с
/// [`Component`]-интерфейсом поверх kit-функций. `props`/`state` — публичные
/// (потребитель читает Props/ведёт состояние через `set_*`); замерщик —
/// приватный (RefCell: `Component::layout`/`paint` принимают `&self` —
/// immediate-вёрстка поверх retained-компонента, §`component/mod.rs`).
pub struct Row {
    /// Свойства кадра.
    pub props: RowProps,
    /// Машина состояний виджета (FR-057): hover/pressed/selected/disabled
    /// → [`KitState`] → слоты [`row_style`].
    pub state: WidgetState,
    /// Замерщик текста (кэш ширин по ключу текст/семейство/кегль).
    measurer: RefCell<TextMeasurer>,
    /// Владелец `FontSystem` (реальный шейпинг cosmic-text — метрики рендера,
    /// CR-015); `FontSystem::new()` дорог — создаётся один раз на компонент,
    /// не на кадр.
    font_system: RefCell<cosmic_text::FontSystem>,
}

impl Row {
    /// Компонент из свойств; состояние — [`WidgetState::default`] (Normal).
    pub fn new(props: RowProps) -> Self {
        Self {
            props,
            state: WidgetState::default(),
            measurer: RefCell::new(TextMeasurer::new()),
            font_system: RefCell::new(cosmic_text::FontSystem::new()),
        }
    }

    /// Части строки из props (вид [`RowParts`] kit-функций). Юнит — вне
    /// Props v1: пустая ячейка.
    fn parts(&self) -> RowParts<'_> {
        RowParts {
            marker: self.props.marker,
            label: &self.props.label,
            value: &self.props.value,
            unit: "",
            badge: self.props.badge.as_deref().unwrap_or(""),
        }
    }

    /// Полная геометрия строки в слоте (вид [`RowLayout`] kit-функций):
    /// направляющие одной строки ([`row_guides`]; правый край — край слота,
    /// зазор — `props.opts.gap`) + [`row_layout`]. `Component::layout`
    /// возвращает плоский срез ([`row_rects`]); этот метод оставляет
    /// потребителю именованные ячейки (лидер, `label_shown`).
    pub fn layout_row(&self, slot: UiRect) -> RowLayout {
        let parts = self.parts();
        let rows = [parts];
        let mut m = self.measurer.borrow_mut();
        let mut fs = self.font_system.borrow_mut();
        // Одна строка — направляющие всегда построены (RowGuides::measure
        // возвращает None только на пустом списке строк).
        let guides = row_guides(
            &mut m,
            &mut fs,
            ROW_DEFAULT_FAMILY,
            ROW_DEFAULT_SIZE,
            &rows,
            slot.right(),
            self.props.opts.gap,
        )
        .expect("одна строка — направляющие всегда построены");
        row_layout(
            &mut m,
            &mut fs,
            ROW_DEFAULT_FAMILY,
            ROW_DEFAULT_SIZE,
            slot,
            guides,
            &rows[0],
            &self.props.opts,
        )
    }
}

/// Плоский порядок rect'ов компонента [`Row`] (выдача `Component::layout`):
/// слот строки (индекс 0 — hit-цель строки), затем точка/глиф-зона (если
/// есть), левый текст, значение, юнит, бейдж (если есть). Лидер и
/// `label_shown` в плоской выдаче не участвуют (штрихи/тексты — данные
/// [`RowLayout`], не hit-цели).
pub fn row_rects(lay: &RowLayout) -> Vec<UiRect> {
    let mut out = Vec::with_capacity(6);
    out.push(lay.row);
    if let Some(dot) = lay.dot {
        out.push(dot);
    }
    if let Some(glyph) = lay.glyph {
        out.push(glyph);
    }
    out.push(lay.label);
    out.push(lay.value);
    out.push(lay.unit);
    if let Some(badge) = lay.badge {
        out.push(badge);
    }
    out
}

impl Component for Row {
    type Props = RowProps;

    fn props(&self) -> &RowProps {
        &self.props
    }

    /// Вёрстка строки в слоте — делегирование в [`Row::layout_row`] (kit-
    /// функции [`row_guides`]/[`row_layout`]). Backend в геометрии Row не
    /// участвует: раскладка — направляющие от замера текста, а замер един у
    /// всех backend'ов (§F-13 FR-062 — TextMeasurer ДО адаптера), паритет
    /// Native/Flex/Taffy тривиален; параметр — контракт `Component` (W3).
    fn layout(&self, _backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        row_rects(&self.layout_row(slot))
    }

    /// Отрисовка строки — делегирование в [`paint_row`] со стилем из слотов
    /// состояний ([`row_style`] по [`WidgetState::kit_state`]). Геометрия
    /// пересчитывается от rect'а строки (immediate-контракт: `rects` —
    /// выдача `Component::layout`, индекс 0 — слот строки).
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&slot) = rects.first() else {
            return;
        };
        let lay = self.layout_row(slot);
        let parts = self.parts();
        let style = row_style(self.state.kit_state(), &self.props.palette);
        paint_row(painter, &lay, &parts, &style, ROW_DEFAULT_SIZE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{font_system, load_display_font, palette_a, FAMILY};
    use crate::paint::PaintItem;
    const ROW_SIZE: f32 = 12.0;
    fn row_parts<'a>(
        marker: RowMarker,
        label: &'a str,
        value: &'a str,
        unit: &'a str,
        badge: &'a str,
    ) -> RowParts<'a> {
        RowParts {
            marker,
            label,
            value,
            unit,
            badge,
        }
    }
    /// D-15: `row_guides` — направляющие по max-ширинам ячеек кит-строк,
    /// правый край — параметр потребителя; бейдж-колонка = текст + 2·пад.
    #[test]
    fn row_guides_takes_max_and_positions_from_right_edge() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let rows = [
            row_parts(RowMarker::Dot, "a", "800", "rps", ""),
            row_parts(RowMarker::None, "b", "20", "", "← источник"),
        ];
        let g = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            &rows,
            300.0,
            canvas_core::tokens::TABLE_GUIDE_GAP,
        )
        .unwrap();
        let badge_w = m.width_of(&mut fs, "← источник", FAMILY, ROW_SIZE) + 2.0 * ROW_BADGE_PAD_H;
        assert_eq!(g.badge_w, badge_w, "бейдж — текст + 2 пада пилюли");
        assert_eq!(g.value_w, m.width_of(&mut fs, "800", FAMILY, ROW_SIZE));
        assert_eq!(g.unit_w, m.width_of(&mut fs, "rps", FAMILY, ROW_SIZE));
        // Правый край: бейдж [300−badge..300], юнит левее, значение ещё левее
        assert_eq!(
            g.unit_right(),
            300.0 - badge_w - canvas_core::tokens::TABLE_GUIDE_GAP
        );
        assert_eq!(
            g.value_right(),
            g.unit_right() - canvas_core::tokens::TABLE_GUIDE_GAP - g.unit_w
        );
        // Пустой список — таблицы нет
        assert!(row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &[], 300.0, 6.0).is_none());
    }
    /// D-15: `row_layout` — значение/юнит прижаты вправо на направляющих
    /// (D-4), маркеры занимают прежние зоны панели (точка x+6, текст x+18),
    /// пустое значение — ячейки нет.
    #[test]
    fn row_layout_aligns_cells_to_guides() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(10.0, 20.0, 300.0, 22.0);
        let rows = [row_parts(RowMarker::Dot, "путь", "1389", "rps", "")];
        let g = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            &rows,
            slot.right(),
            canvas_core::tokens::TABLE_GUIDE_GAP,
        )
        .unwrap();
        let lay = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        // Точка: x+6, диаметр 6, центр по вертикали
        let dot = lay.dot.unwrap();
        assert_eq!(dot.x, slot.x + ROW_DOT_PAD);
        assert_eq!(dot.w, ROW_DOT);
        // Левый текст начинается в x+18 (6+6+6) — зона панели FR-044
        assert_eq!(lay.label.x, slot.x + 18.0);
        // Значение: право на направляющую чисел, left = right − width (D-4)
        let vw = m.width_of(&mut fs, "1389", FAMILY, ROW_SIZE);
        assert!((lay.value.right() - g.value_right()).abs() < 0.01);
        assert!((lay.value.w - vw).abs() < 0.01);
        // Юнит: право на направляющую юнитов (O-4)
        let uw = m.width_of(&mut fs, "rps", FAMILY, ROW_SIZE);
        assert!((lay.unit.right() - g.unit_right()).abs() < 0.01);
        assert!((lay.unit.w - uw).abs() < 0.01);
        assert!(
            (lay.leader_y - slot.y - slot.h * canvas_core::tokens::TABLE_LEADER_Y_FRAC).abs()
                < 0.01
        );
        // Формульная строка (глиф, без значения): ячейки нет, лидера нет
        let formula = row_parts(RowMarker::Glyph("ƒ"), "итого = a * b", "", "", "");
        let lf = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &formula,
            &RowOpts::default(),
        );
        assert!(lf.glyph.is_some());
        assert_eq!(lf.value.w, 0.0);
        assert!(lf.leader.is_none());
    }
    /// D-15: RowOpts::leader=false (панель FR-044 — прототип без лидера) —
    /// дорожки нет, прочая геометрия без изменений.
    #[test]
    fn row_opts_disable_leader() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let rows = [row_parts(RowMarker::Dot, "путь", "800", "", "")];
        let g = row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &rows, 300.0, 6.0).unwrap();
        let on = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        let off = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts {
                leader: false,
                ..RowOpts::default()
            },
        );
        assert!(on.leader.is_some());
        assert!(off.leader.is_none());
        assert_eq!(on.value, off.value);
        assert_eq!(on.label, off.label);
    }
    /// D-15: `paint_row` — порядок items = фон → маркер → текст → лидер →
    /// значение → бейдж (draw-порядок), цвета — из RowStyle дословно.
    #[test]
    fn paint_row_emits_items_in_draw_order() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let rows = [
            row_parts(RowMarker::Dot, "a", "800", "rps", "← ист"),
            row_parts(RowMarker::Dot, "a", "800", "rps", "← ист"),
        ];
        let g = row_guides(&mut m, &mut fs, FAMILY, ROW_SIZE, &rows, 300.0, 6.0).unwrap();
        let lay = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            ROW_SIZE,
            slot,
            g,
            &rows[0],
            &RowOpts::default(),
        );
        let style = row_style(KitState::Normal, &palette_a());
        let mut p = Painter::new();
        paint_row(&mut p, &lay, &rows[0], &style, ROW_SIZE);
        let items = p.items();
        // фон, точка, label, ≥1 штрих, value, unit, пилюля, текст бейджа
        assert!(
            items.len() >= 7,
            "ожидались все слои строки: {}",
            items.len()
        );
        assert!(matches!(items[0], PaintItem::Rect { .. }), "фон — rect");
        assert!(matches!(items[1], PaintItem::Rect { .. }), "точка — rect");
        assert!(matches!(items[2], PaintItem::Text { .. }), "label — text");
        // штрихи между label и value — rect с цветом маркера
        let dashes = items[3..]
            .iter()
            .take_while(|i| matches!(i, PaintItem::Rect { .. }))
            .count();
        assert!(dashes >= 1, "лидер — хотя бы один штрих");
        assert!(
            matches!(items[3 + dashes], PaintItem::Text { .. }),
            "value — text"
        );
        let last = items.last().unwrap();
        match last {
            PaintItem::Text { text, .. } => assert_eq!(text, "← ист"),
            other => panic!("последний — текст бейджа, получено {:?}", other),
        }
    }
    /// D-5/этап E: `leader_dash_rects` — та же арифметика, что в теле ноды
    /// (штрих TABLE_LEADER_DASH, шаг DASH+GAP, min_track); детерминизм.
    #[test]
    fn leader_dashes_match_node_arithmetic() {
        // Прежний цикл text.rs: x от x0 с шагом (DASH+GAP)·z, пока x+DASH·z <= x1
        let z = 1.25_f32;
        let (x0, x1, y) = (10.0_f32, 200.0_f32, 13.75_f32);
        let expected = {
            let mut v = Vec::new();
            let step = (canvas_core::tokens::TABLE_LEADER_DASH
                + canvas_core::tokens::TABLE_LEADER_GAP)
                * z;
            let mut x = x0;
            while x + canvas_core::tokens::TABLE_LEADER_DASH * z <= x1 {
                v.push([
                    x,
                    y,
                    canvas_core::tokens::TABLE_LEADER_DASH * z,
                    canvas_core::tokens::TABLE_LEADER_H * z,
                ]);
                x += step;
            }
            v
        };
        let got = leader_dash_rects(x0, x1, y, z, 6.0);
        assert_eq!(got.len(), expected.len());
        for (a, b) in got.iter().zip(&expected) {
            assert_eq!(a, b);
        }
        // Дорожка короче минимума — штрихов нет
        assert!(leader_dash_rects(0.0, 5.0, y, 1.0, 6.0).is_empty());
    }
    /// D-15: `row_style` — состояния берут слоты состояний (Selected —
    /// selected_fill + accent, Disabled — disabled_text), Normal —
    /// прозрачный фон (зебра — решение потребителя).
    #[test]
    fn row_style_uses_state_slots() {
        let p = palette_a();
        let normal = row_style(KitState::Normal, &p);
        assert_eq!(normal.fill, [0.0; 4], "Normal — прозрачная строка");
        assert_eq!(normal.label, p.text);
        assert_eq!(normal.unit, p.text_muted);
        let selected = row_style(KitState::Selected, &p);
        assert_eq!(selected.fill, p.selected_fill);
        assert_eq!(selected.border, p.accent);
        let disabled = row_style(KitState::Disabled, &p);
        assert_eq!(disabled.label, p.disabled_text);
        assert_eq!(disabled.badge_fill, [0.0; 4]);
    }

    // === FR-068 W3: компонент Row (Component-слой) ==========================

    /// W3: паритет `Component::layout` ↔ прямой `row_layout` (те же части из
    /// тех же props, тот же инстанс FontSystem — метрики идентичны):
    /// плоская выдача совпадает дословно, индекс 0 — слот строки. Backend —
    /// [`crate::layout::default_backend`] (исторически: на `--features taffy` —
    /// TaffyBackend): в геометрию Row backend не входит — паритет обязателен
    /// на обеих сборках (§Контракт-3 FR-068).
    #[test]
    fn component_layout_matches_row_layout_oracle() {
        let slot = UiRect::new(10.0, 20.0, 300.0, 22.0);
        let row = Row::new(RowProps {
            marker: RowMarker::Dot,
            label: "путь /api/rps".to_owned(),
            value: "1389".to_owned(),
            badge: Some("← источник".to_owned()),
            opts: RowOpts::default(),
            palette: palette_a(),
        });
        load_display_font(&mut row.font_system.borrow_mut());

        let rects = row.layout(crate::layout::default_backend(), slot);

        // Оракул: прямые row_guides/row_layout с теми же частями (build из
        // тех же props) и тем же инстансом FontSystem.
        let parts = row.parts();
        let lay = {
            let rows = [parts];
            let mut m = row.measurer.borrow_mut();
            let mut fs = row.font_system.borrow_mut();
            let g = row_guides(
                &mut m,
                &mut fs,
                ROW_DEFAULT_FAMILY,
                ROW_DEFAULT_SIZE,
                &rows,
                slot.right(),
                row.props.opts.gap,
            )
            .unwrap();
            row_layout(
                &mut m,
                &mut fs,
                ROW_DEFAULT_FAMILY,
                ROW_DEFAULT_SIZE,
                slot,
                g,
                &rows[0],
                &row.props.opts,
            )
        };
        assert_eq!(rects, row_rects(&lay), "плоская выдача = oracle row_layout");
        assert_eq!(rects[0], slot, "индекс 0 — слот строки (hit-цель)");
        assert_eq!(rects.len(), 6, "row, dot, label, value, unit, badge");
    }

    /// W3: `Component::paint` эмитит элементы строки (draw-порядок
    /// [`paint_row`]: фон → маркер → тексты), и состояние виджета ведёт
    /// стиль — Normal даёт прозрачный фон строки (зебру решает потребитель),
    /// hover — слот `hover_fill` палитры (интеграция `WidgetState` FR-057).
    #[test]
    fn component_paint_emits_items_and_uses_state() {
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let mut row = Row::new(RowProps {
            marker: RowMarker::Dot,
            label: "задержка p99".to_owned(),
            value: "42".to_owned(),
            badge: None,
            opts: RowOpts::default(),
            palette: palette_a(),
        });
        load_display_font(&mut row.font_system.borrow_mut());

        let rects = row.layout(crate::layout::default_backend(), slot);
        let mut p = Painter::new();
        row.paint(&mut p, &rects);
        let items = p.items();
        assert!(!items.is_empty(), "paint эмитит элементы строки");
        let fill_of = |item: &PaintItem| match item {
            PaintItem::Rect { fill, .. } => Some(*fill),
            _ => None,
        };
        assert!(
            matches!(items[0], PaintItem::Rect { .. }),
            "фон строки — rect"
        );
        assert_eq!(
            fill_of(&items[0]),
            Some([0.0; 4]),
            "Normal — прозрачный фон строки"
        );
        // hover → слот hover_fill (WidgetState → KitState → row_style)
        row.state.set_pointer(true, false);
        let mut p = Painter::new();
        row.paint(&mut p, &rects);
        assert_eq!(
            fill_of(&p.items()[0]),
            Some(palette_a().hover_fill),
            "hover — слот hover_fill"
        );
    }

    /// W3: hit-test компонента (дефолтный — [`ComponentHit::pick`]): точка
    /// внутри слота — строка (индекс 0 — первый rect плоской выдачи), вне
    /// слота — None.
    #[test]
    fn component_hit_test_finds_row_at_index_0() {
        use crate::component::ComponentHit;
        use crate::geometry::UiPoint;
        let slot = UiRect::new(0.0, 0.0, 300.0, 22.0);
        let row = Row::new(RowProps {
            marker: RowMarker::None,
            label: "строка".to_owned(),
            value: "20".to_owned(),
            badge: None,
            opts: RowOpts::default(),
            palette: palette_a(),
        });
        let rects = row.layout(crate::layout::default_backend(), slot);
        assert_eq!(
            row.hit_test(&rects, UiPoint::new(150.0, 11.0)),
            Some(ComponentHit { index: 0 }),
            "внутри слота — строка (index 0)"
        );
        assert!(
            row.hit_test(&rects, UiPoint::new(500.0, 11.0)).is_none(),
            "вне слота — мимо"
        );
    }
}
