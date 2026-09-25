//! FR-068 W3-продолжение (этап M1): Table — retained-компонент таблицы
//! поверх kit-функций Row v1 (дизайн: `docs/plans/fr-068-table-v2.md`).
//!
//! Кит v1 даёт строку ([`row_guides`] → [`row_layout`] → [`paint_row`],
//! FR-061), но оркестрация «замерить ВСЕ строки → общие направляющие →
//! окно видимости → per-row состояние/стиль → строки → бегунок»
//! дублируется потребителями (18 мест — каталог миграции §9.3.5).
//! [`Table`] владеет данными строк ([`TableRow`]), общими направляющими
//! (двухпроходность §3.1 FR-061: колонка значений стабильна при
//! прокрутке), окном видимости ([`ScrollState`] + [`list_rows`], FR-059)
//! и draw-порядком stage (строки → бегунок).
//!
//! Принципы (дизайн §3): I-1 — бит-в-бит геометрия существующих таблиц
//! (оракулы §7: `*_with` ≡ kit-функции, Component ≡ `*_with`); F-8 — цвета
//! только слотами ([`KitState`] → [`row_style`] + переопределения
//! [`TableRowStyle`], арифметики над цветами в ките нет); additive —
//! kit-функции Row v1 не меняются, Table их ПОТРЕБЛЯЕТ.
//!
//! Два слоя замера (дизайн §4.7): основной (потребительский) — методы
//! `_with` с ВНЕШНИМ замерщиком (продакшн-потребители с общим FontSystem
//! рендера — паритет замера с текущим кодом); компонентный — `impl
//! [`Component`]` с собственными `RefCell`-замерщиками (прецедент
//! [`Row`](crate::component::row::Row)). Направляющие строятся по ВСЕМ
//! строкам модели (не окна видимости) от ширины ВЬЮПОРТА
//! (`viewport_w − opts.right_pad`) — §4.2.

use std::cell::RefCell;

use super::list::{list_rows, scroll_bar, ScrollState};
use super::row::{
    paint_row, row_guides, row_layout, row_style, RowLayout, RowMarker, RowOpts, RowParts,
    RowStyle, ROW_DEFAULT_FAMILY, ROW_DEFAULT_SIZE,
};
use super::{Component, KitPalette, KitState, LIST_ROW_H};
use crate::geometry::{UiPoint, UiRect};
use crate::layout::LayoutBackend;
use crate::measure::TextMeasurer;
use crate::paint::Painter;
use crate::row_guides::RowGuides;

// --- Данные (дизайн §4.1) ----------------------------------------------------

/// Строка таблицы (retained-данные; владеет строками — прецедент
/// [`RowProps`](crate::component::row::RowProps)). Пересобирается только
/// при смене модели ([`Table::set_rows`]); per-frame анимации (приглушение,
/// рамка фокуса) — мутацией по месту `rows[i].style/state` (дизайн §4.6).
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    /// Маркер левой колонки (точка/глиф/нет — прототип Р-4 FR-044).
    pub marker: RowMarker,
    /// Левый текст (имя/формула/путь) — усечение ellipsis'ом (класс CR-015).
    pub label: String,
    /// Значение (прижато вправо на направляющей чисел, D-4); пустое —
    /// ячейки нет и лидера нет (строки-формулы панели).
    pub value: String,
    /// Юнит (прижат вправо на направляющей юнитов; пустой — скаляр).
    pub unit: String,
    /// Бейдж (пилюля в бейдж-колонке; `None`/пустой — колонки нет).
    pub badge: Option<String>,
    /// Состояние строки (Selected/Normal/Hovered/Disabled) — слоты
    /// [`row_style`]. Hover/press машина (`WidgetState`, FR-057) — у
    /// потребителя, если нужна: строки панели hover не имеют (фокус
    /// Р-5 — Selected).
    pub state: KitState,
    /// Переопределения слотов (plain data, F-8): `None` — слот базы
    /// [`row_style(state)`](row_style). Семантика потребителя (ошибка/
    /// unmapped/зебра/приглушение) — заполнением этих полей.
    pub style: TableRowStyle,
}

/// Переопределение слотов стиля строки ([`TableRowStyle::apply`]): каждое
/// поле — готовый цвет-слот (арифметика — на стороне потребителя до
/// присвоения, контракт F-8: кит не вычисляет цветов).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TableRowStyle {
    /// Фон строки (зебра/приглушение — потребитель).
    pub fill: Option<[f32; 4]>,
    /// Рамка строки (фокус Р-5/unmapped — потребитель).
    pub border: Option<[f32; 4]>,
    /// Цвет маркера (value-точка/глиф).
    pub marker: Option<[f32; 4]>,
    /// Цвет левого текста.
    pub label: Option<[f32; 4]>,
    /// Цвет значения (ошибка — слот danger потребителя).
    pub value: Option<[f32; 4]>,
    /// Цвет юнита.
    pub unit: Option<[f32; 4]>,
    /// Цвет текста бейджа.
    pub badge: Option<[f32; 4]>,
    /// Заливка пилюли бейджа.
    pub badge_fill: Option<[f32; 4]>,
    /// Рамка пилюли бейджа.
    pub badge_border: Option<[f32; 4]>,
}

impl TableRowStyle {
    /// Слияние с базой [`row_style(state)`](row_style): `Some` — замена
    /// поля, `None` — слот базы. Готовый стиль для [`paint_row`].
    pub fn apply(self, base: RowStyle) -> RowStyle {
        RowStyle {
            fill: self.fill.unwrap_or(base.fill),
            border: self.border.unwrap_or(base.border),
            marker: self.marker.unwrap_or(base.marker),
            label: self.label.unwrap_or(base.label),
            value: self.value.unwrap_or(base.value),
            unit: self.unit.unwrap_or(base.unit),
            badge: self.badge.unwrap_or(base.badge),
            badge_fill: self.badge_fill.unwrap_or(base.badge_fill),
            badge_border: self.badge_border.unwrap_or(base.badge_border),
        }
    }
}

/// Опции таблицы (плоско; горизонтальный зазор направляющих — тот же
/// параметр, что [`RowOpts::gap`] / [`RowGuides::with_right_edge`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableOpts {
    /// Дорожка лидера D-5 (stage — off, тело ноды/витрина — on).
    pub leader: bool,
    /// Зазор между соседними ячейками направляющих (дефолт — токен
    /// `TABLE_GUIDE_GAP`).
    pub gap: f32,
    /// Высота строки (панель stage 22 / `LIST_ROW_H` 26 — параметр
    /// потребителя).
    pub row_h: f32,
    /// Вертикальный зазор строк [`list_rows`] (stage — 0).
    pub row_gap: f32,
    /// Отступ правого края направляющих от вьюпорта (stage «Переменные» —
    /// 6; формулы — 0: направляющие не строятся, деградация §4.2).
    pub right_pad: f32,
}

impl Default for TableOpts {
    fn default() -> Self {
        Self {
            leader: true,
            gap: canvas_core::tokens::TABLE_GUIDE_GAP,
            row_h: LIST_ROW_H,
            row_gap: 0.0,
            right_pad: 0.0,
        }
    }
}

/// Свойства [`Table`] (декларативный вход кадра). Кегль/семейство —
/// осознанное отличие от Row v1: stage рисует панель кеглем 11.0,
/// kit/admin — `LABEL_SIZE`; замер обязан идти тем же кеглем, что шейпинг
/// (T9 FR-061; дизайн §4.1 — «Table v2 и есть это расширение»).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableProps {
    /// Кегль строк (замер = шейпинг, T9 FR-061).
    pub size: f32,
    /// Семейство (то же строковое имя, что `Family::Name` рендера).
    pub family: &'static str,
    /// Опции таблицы (лидер/зазор/высота/пад права).
    pub opts: TableOpts,
    /// Палитра-срез (слоты состояний FR-053: база стилей строк, бегунок).
    pub palette: KitPalette,
}

impl Default for TableProps {
    // [`KitPalette`] — слоты из темы потребителя (F-8) и `Default` не
    // имеет: дефолт Props — нулевая (прозрачная) палитра-заглушка;
    // потребитель обязан подставить палитру своей темы до отрисовки
    // (как и для любого KitPalette). Нули — нейтральное «нет цвета»,
    // скрытой арифметики/конкретных цветов кит не содержит.
    fn default() -> Self {
        Self {
            size: ROW_DEFAULT_SIZE,
            family: ROW_DEFAULT_FAMILY,
            opts: TableOpts::default(),
            palette: KitPalette {
                panel_fill: [0.0; 4],
                panel_border: [0.0; 4],
                control_fill: [0.0; 4],
                control_border: [0.0; 4],
                control_primary: [0.0; 4],
                control_danger: [0.0; 4],
                hover_fill: [0.0; 4],
                primary_hover_fill: [0.0; 4],
                selected_fill: [0.0; 4],
                text: [0.0; 4],
                text_title: [0.0; 4],
                text_muted: [0.0; 4],
                disabled_text: [0.0; 4],
                accent: [0.0; 4],
            },
        }
    }
}

// --- Компонент (дизайн §4–§5) ------------------------------------------------

/// Таблица — retained-компонент (FR-068 Table v2, этап M1): владеет
/// свойствами ([`TableProps`]), данными строк ([`TableRow`]) и скроллом
/// ([`ScrollState`] — публичное поле, как у `List`: offset/clamp —
/// потребитель, `content_h` — [`Table::set_rows`], §4.3). Замерщик —
/// приватный (`RefCell<TextMeasurer>` + `RefCell<FontSystem>` — один
/// инстанс на компонент, практика measure.rs §14/Q6: `FontSystem::new()`
/// дорогой, на кадр не создаётся — §4.6).
///
/// Слои замера (§4.7): методы `_with` — внешний замерщик (продакшн-путь);
/// `impl Component` и owned-обёртки ([`Table::guides`],
/// [`Table::row_layout_at`]) — собственные `RefCell`-замерщики.
pub struct Table {
    /// Свойства кадра.
    pub props: TableProps,
    /// Строки модели (retained-данные; [`Table::set_rows`] при смене
    /// модели, мутация `rows[i]` — для per-frame анимаций, §4.6).
    pub rows: Vec<TableRow>,
    /// Состояние скролла (FR-059): offset/clamp — потребитель
    /// (scroll_by по колесу, как сейчас), `content_h` — [`Table::set_rows`].
    pub scroll: ScrollState,
    /// Замерщик текста (кэш ширин по ключу текст/семейство/кегль —
    /// переживает кадры, §4.6).
    measurer: RefCell<TextMeasurer>,
    /// Владелец `FontSystem` (реальный шейпинг cosmic-text — метрики
    /// рендера, CR-015); создаётся один раз на компонент (§4.6).
    font_system: RefCell<cosmic_text::FontSystem>,
}

impl Table {
    /// Компонент из свойств; строки/скролл — пустые (потребитель заполняет
    /// [`Table::set_rows`] и `scroll` своей моделью до layout).
    pub fn new(props: TableProps) -> Self {
        Self {
            props,
            rows: Vec::new(),
            scroll: ScrollState::default(),
            measurer: RefCell::new(TextMeasurer::new()),
            font_system: RefCell::new(cosmic_text::FontSystem::new()),
        }
    }

    /// Замена строк модели + синхронизация `scroll.content_h`
    /// (`n·row_h + (n−1)·row_gap`) — инвариант sync_scroll держит сам
    /// компонент: устранение класса рассинхронов на источнике (§4.3;
    /// у `List` content_h восстанавливается из высоты, Table ЗНАЕТ строки).
    /// offset/clamp — потребитель (`scroll.viewport_h` — из раскладки
    /// панели каждый кадр + `scroll.clamp()`, идемпотентно).
    pub fn set_rows(&mut self, rows: Vec<TableRow>) {
        let n = rows.len();
        self.rows = rows;
        self.scroll.content_h =
            n as f32 * self.props.opts.row_h + n.saturating_sub(1) as f32 * self.props.opts.row_gap;
    }

    /// Части i-й строки (вид [`RowParts`] kit-функций): заимствование
    /// полей, бейдж `None`/пустой — пустая ячейка (правило заимствования
    /// §4.7; zero-copy на время замера/отрисовки).
    fn parts_of(&self, i: usize) -> RowParts<'_> {
        let row = &self.rows[i];
        RowParts {
            marker: row.marker,
            label: &row.label,
            value: &row.value,
            unit: &row.unit,
            badge: row.badge.as_deref().unwrap_or(""),
        }
    }

    /// Полный стиль i-й строки: база [`row_style(state)`](row_style) +
    /// переопределения потребителя ([`TableRowStyle::apply`], F-8).
    fn merged_style(&self, i: usize) -> RowStyle {
        self.rows[i]
            .style
            .apply(row_style(self.rows[i].state, &self.props.palette))
    }

    // --- Основной (потребительский) слой замера (§4.7.1) ---------------------

    /// Проход A+B: направляющие по ВСЕМ строкам модели (не окна видимости!)
    /// — колонка значений стабильна при прокрутке (§3.1 FR-061).
    /// Делегирование: [`row_guides`] (семейство/кегль — Props), правый край
    /// `viewport_w − opts.right_pad`, зазор `opts.gap`.
    ///
    /// Правило деградации (§4.2, обязательное — иначе бит-в-бит не
    /// сходится): строк нет ИЛИ правые колонки пусты у всех строк
    /// (`value_w+unit_w+badge_w == 0`) → `None`. `with_right_edge`
    /// вычитает структурный зазор безусловно — «наивные» нулевые
    /// направляющие сдвинули бы `label_right` на `gap` влево относительно
    /// текущего рендера формул (паритет stage.rs:1386–1392 даёт
    /// [`Table::row_layout_with`] деградированным входом).
    pub fn guides_with(
        &self,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        viewport_w: f32,
    ) -> Option<RowGuides> {
        let parts: Vec<RowParts<'_>> = self
            .rows
            .iter()
            .map(|row| RowParts {
                marker: row.marker,
                label: &row.label,
                value: &row.value,
                unit: &row.unit,
                badge: row.badge.as_deref().unwrap_or(""),
            })
            .collect();
        let right_edge = viewport_w - self.props.opts.right_pad;
        let guides = row_guides(
            m,
            fs,
            self.props.family,
            self.props.size,
            &parts,
            right_edge,
            self.props.opts.gap,
        )?;
        if guides.value_w + guides.unit_w + guides.badge_w == 0.0 {
            return None;
        }
        Some(guides)
    }

    /// Видимые строки: `(МОДЕЛЬНЫЙ индекс, усечённый rect)` —
    /// [`list_rows`] + ПЕРЕСЕЧЕНИЕ каждого rect с
    /// вьюпортом (клип-семантика stage: частичная строка усекается, текст
    /// центрируется в усечённом слоте — calc_panel_ui::visible_rows).
    /// Отличие от `List::layout` (полные rect'ы частичных строк) —
    /// осознанное, бит-в-бит требование I-1 (дизайн §4.3).
    ///
    /// Замера не требует; чистая функция — `scroll` не мутируется
    /// (инвариант 2 FR-050).
    pub fn visible_rows(&self, viewport: UiRect) -> Vec<(usize, UiRect)> {
        list_rows(
            viewport,
            &self.scroll,
            self.props.opts.row_h,
            self.props.opts.row_gap,
            self.rows.len(),
        )
        .into_iter()
        .filter_map(|(index, rect)| rect.intersection(&viewport).map(|r| (index, r)))
        .collect()
    }

    /// Геометрия одной строки ([`row_layout`]) в слоте `slot`.
    /// Направляющие — ОБЩИЕ, от ширины ВЬЮПОРТА (`viewport_w`), а не от
    /// слота: слот строки full-width (возможно усечён по вертикали —
    /// [`Table::visible_rows`]), поэтому `right_edge = viewport_w −
    /// opts.right_pad` (§4.2). Деградация (§4.2): направляющие не
    /// построены ([`Table::guides_with`] = `None`) — вход от правого края
    /// строки `RowGuides { value_w:0, unit_w:0, badge_w:0, value_x:
    /// unit_x: slot.right() }` — точный паритет ручной деградации
    /// stage.rs:1386–1392 (`label_right = slot.right() − ROW_TEXT_GAP`).
    ///
    /// `index` вне строк модели → `None`.
    pub fn row_layout_with(
        &self,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        viewport_w: f32,
        slot: UiRect,
        index: usize,
    ) -> Option<RowLayout> {
        if index >= self.rows.len() {
            return None;
        }
        let parts = self.parts_of(index);
        let guides = self.guides_with(m, fs, viewport_w).unwrap_or_else(|| {
            // Деградация §4.2 — паритет stage.rs:1386–1392: правый край
            // строки, без структурного зазора (иначе label_right ушёл бы
            // на gap влево от текущего рендера формул).
            RowGuides {
                value_w: 0.0,
                unit_w: 0.0,
                badge_w: 0.0,
                value_x: slot.right(),
                unit_x: slot.right(),
            }
        });
        let opts = RowOpts {
            leader: self.props.opts.leader,
            gap: self.props.opts.gap,
        };
        Some(row_layout(
            m,
            fs,
            self.props.family,
            self.props.size,
            slot,
            guides,
            &parts,
            &opts,
        ))
    }

    /// Отрисовка в Painter (§4.4): для каждой видимой строки — полный стиль
    /// ([`Table::merged_style`]), геометрия ([`Table::row_layout_with`]
    /// от ширины вьюпорта), kit [`paint_row`]; ПОСЛЕ строк — бегунок
    /// [`scroll_bar`] (draw-порядок stage:1313–1436; rect слотом
    /// `control_border`, радиус w/2 — паритет `List::paint` и
    /// stage:1429–1436; `None` — 0 items).
    ///
    /// Основной слой замера: замерщик — ВНЕШНИЙ (продакшн-потребители с
    /// общим FontSystem рендера — §4.7).
    pub fn paint_with(
        &self,
        p: &mut Painter,
        m: &mut TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
        viewport: UiRect,
    ) {
        for (index, slot) in self.visible_rows(viewport) {
            let style = self.merged_style(index);
            if let Some(lay) = self.row_layout_with(m, fs, viewport.w, slot, index) {
                paint_row(p, &lay, &self.parts_of(index), &style, self.props.size);
            }
        }
        if let Some(knob) = scroll_bar(viewport, &self.scroll, &self.props.palette) {
            p.rect(
                knob,
                self.props.palette.control_border,
                [0.0; 4],
                knob.w / 2.0,
            );
        }
    }

    /// Модельный индекс строки под точкой (§4.5): по [`Table::visible_rows`]
    /// — первый слот, содержащий точку ([`UiRect::contains`]). Замена
    /// `CalcPanelLayout::var_row_at/formula_row_at` при миграции input.rs
    /// (геометрия идентична — оракул T2). Вне вьюпорта/в зазоре — `None`.
    pub fn model_index_at(&self, viewport: UiRect, point: UiPoint) -> Option<usize> {
        self.visible_rows(viewport)
            .into_iter()
            .find(|(_, slot)| slot.contains(point))
            .map(|(index, _)| index)
    }

    // --- Компонентный слой: owned-обёртки (собственные замерщики) ------------

    /// [`Table::guides_with`] на СОБСТВЕННЫХ замерщиках компонента
    /// (потребители без общего замерщика; §4.7.2).
    pub fn guides(&self, viewport_w: f32) -> Option<RowGuides> {
        let mut m = self.measurer.borrow_mut();
        let mut fs = self.font_system.borrow_mut();
        self.guides_with(&mut m, &mut fs, viewport_w)
    }

    /// [`Table::row_layout_with`] на СОБСТВЕННЫХ замерщиках компонента.
    pub fn row_layout_at(&self, viewport_w: f32, slot: UiRect, index: usize) -> Option<RowLayout> {
        let mut m = self.measurer.borrow_mut();
        let mut fs = self.font_system.borrow_mut();
        self.row_layout_with(&mut m, &mut fs, viewport_w, slot, index)
    }
}

impl Component for Table {
    type Props = TableProps;

    fn props(&self) -> &Self::Props {
        &self.props
    }

    /// Слоты видимых строк — ПЛОСКИЙ срез усечённых rect'ов в порядке
    /// модельных индексов ([`Table::visible_rows`]; контракт `Component`
    /// mod.rs). Backend в геометрии Table не участвует: раскладка — окно
    /// видимости от `self.scroll` (чистая функция слота), паритет
    /// движков тривиален (§Контракт-3 FR-068).
    fn layout(&self, _backend: &dyn LayoutBackend, viewport: UiRect) -> Vec<UiRect> {
        self.visible_rows(viewport)
            .into_iter()
            .map(|(_, rect)| rect)
            .collect()
    }

    /// Строки → бегунок (§4.4). Геометрия пересчитывается из `self.scroll`
    /// (чистая функция — детерминизм, инвариант 2 FR-050; контракт:
    /// `rects == layout(viewport)` в том же порядке — модельный индекс
    /// i-го rect'а — `visible_rows[i].0`). Viewport восстанавливается из
    /// rect'ов (прецедент `List::paint`): x/y/w — первой строки, h —
    /// `scroll.viewport_h` (окно видимости — по контракту). Замер —
    /// собственные `RefCell`-замерщики компонента (`&self`, §4.7.2).
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&first) = rects.first() else {
            return;
        };
        let viewport = UiRect::new(first.x, first.y, first.w, self.scroll.viewport_h);
        let mut m = self.measurer.borrow_mut();
        let mut fs = self.font_system.borrow_mut();
        self.paint_with(painter, &mut m, &mut fs, viewport);
    }

    // hit_test — дефолтный ComponentHit::pick (§4.5): индекс усечённого
    // rect'а видимой строки; зазоры row_gap>0 — None (как у List).
    // Потребительский (модельный) hit — Table::model_index_at.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::test_support::{
        font_system, load_display_font, palette_a, palette_b, FAMILY,
    };
    use crate::layout::default_backend;
    use crate::paint::PaintItem;
    // Константы кита, нужные только тестам (ROW_TEXT_GAP — паритет
    // label_right формул; LIST_ROW_GAP — зазор окна видимости T2/T6)
    use crate::component::row::ROW_TEXT_GAP;
    use crate::component::LIST_ROW_GAP;

    const SIZE: f32 = ROW_DEFAULT_SIZE; // 12.0 — база кит-строки (Props::default)

    /// Строка-фабрика: все ячейки строками (модель теста — compact).
    fn tr(
        marker: RowMarker,
        label: &str,
        value: &str,
        unit: &str,
        badge: Option<&str>,
        state: KitState,
        style: TableRowStyle,
    ) -> TableRow {
        TableRow {
            marker,
            label: label.to_owned(),
            value: value.to_owned(),
            unit: unit.to_owned(),
            badge: badge.map(str::to_owned),
            state,
            style,
        }
    }

    /// Данные kit-демо (4 строки: Dot/юнит/бейдж/Selected — витрина примитива).
    fn demo_rows() -> Vec<TableRow> {
        vec![
            tr(
                RowMarker::Dot,
                "a",
                "800",
                "rps",
                Some("← источник"),
                KitState::Normal,
                TableRowStyle::default(),
            ),
            tr(
                RowMarker::None,
                "b",
                "20",
                "",
                None,
                KitState::Selected,
                TableRowStyle::default(),
            ),
            tr(
                RowMarker::Glyph("ƒ"),
                "c = a + b",
                "1389",
                "",
                Some("x"),
                KitState::Normal,
                TableRowStyle::default(),
            ),
            tr(
                RowMarker::Dot,
                "d",
                "50",
                "req/s",
                None,
                KitState::Normal,
                TableRowStyle::default(),
            ),
        ]
    }

    /// Строки-формулы stage («Расчёт»: ƒ-маркер, правые ячейки пусты).
    fn formula_rows() -> Vec<TableRow> {
        ["итого = a + b", "итого = a * b", "итого = c / d"]
            .iter()
            .map(|label| {
                tr(
                    RowMarker::Glyph("ƒ"),
                    label,
                    "",
                    "",
                    None,
                    KitState::Normal,
                    TableRowStyle::default(),
                )
            })
            .collect()
    }

    /// Оракул-сборка RowParts из модели строк (правило заимствования §4.7:
    /// badge `None`/пустой → "").
    fn parts_of_rows(rows: &[TableRow]) -> Vec<RowParts<'_>> {
        rows.iter()
            .map(|row| RowParts {
                marker: row.marker,
                label: &row.label,
                value: &row.value,
                unit: &row.unit,
                badge: row.badge.as_deref().unwrap_or(""),
            })
            .collect()
    }

    // === T1 (дизайн §7): направляющие ≡ kit row_guides =======================

    /// T1: `guides_with` ≡ `row_guides` на тех же частях (право =
    /// `viewport_w − right_pad`, зазор `opts.gap`) — побитово на целочисленных
    /// ширинах (прецедент тестов `cells_with` row_guides.rs); owned `guides`
    /// ≡ `guides_with`. Два датасета: kit-демо (4 строки с бейджем) и
    /// stage-подобный (только label+value, кегль 11.0, right_pad 6).
    #[test]
    fn t1_guides_with_matches_kit_row_guides() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        const W: f32 = 300.0;

        // kit-демо: right_pad = 0, gap — дефолт (токен)
        let mut table = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts::default(),
            palette: palette_a(),
        });
        table.rows = demo_rows();
        let expected = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            SIZE,
            &parts_of_rows(&table.rows),
            W - table.props.opts.right_pad,
            table.props.opts.gap,
        );
        assert!(expected.is_some(), "демо-данные имеют живые правые колонки");
        assert_eq!(
            table.guides_with(&mut m, &mut fs, W),
            expected,
            "guides_with ≡ kit row_guides (побитово, целые входы)"
        );
        let demo_expected = expected;

        // stage-подобные (только label+value): кегль 11.0, right_pad = 6,
        // лидер off (панель без лидера)
        let mut stage = Table::new(TableProps {
            size: 11.0,
            family: FAMILY,
            opts: TableOpts {
                leader: false,
                right_pad: 6.0,
                ..TableOpts::default()
            },
            palette: palette_b(),
        });
        stage.rows = vec![
            tr(
                RowMarker::Dot,
                "ставка",
                "13.5",
                "",
                None,
                KitState::Normal,
                TableRowStyle::default(),
            ),
            tr(
                RowMarker::Dot,
                "объём",
                "1200",
                "",
                None,
                KitState::Normal,
                TableRowStyle::default(),
            ),
        ];
        let expected = row_guides(
            &mut m,
            &mut fs,
            FAMILY,
            11.0,
            &parts_of_rows(&stage.rows),
            W - stage.props.opts.right_pad,
            stage.props.opts.gap,
        );
        assert_eq!(
            stage.guides_with(&mut m, &mut fs, W),
            expected,
            "stage-подобные: right_edge = viewport_w − right_pad"
        );

        // owned-слой ≡ kit-оракул: собственные замерщики компонента
        // (детерминированный шрифт тестов — тот же файл, метрики равны)
        let mut owned = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts::default(),
            palette: palette_a(),
        });
        owned.rows = demo_rows();
        load_display_font(&mut owned.font_system.borrow_mut());
        assert_eq!(
            owned.guides(W),
            demo_expected,
            "owned guides ≡ kit row_guides (тот же детерминированный шрифт)"
        );
        // owned ≡ *_with на ТЕХ ЖЕ замерщиках (два слоя, один инстанс)
        let via_with = {
            let mut m2 = owned.measurer.borrow_mut();
            let mut fs2 = owned.font_system.borrow_mut();
            owned.guides_with(&mut m2, &mut fs2, W)
        };
        assert_eq!(owned.guides(W), via_with, "owned guides ≡ guides_with");
    }

    // === T2: окно видимости ≡ list_rows + пересечение ========================

    /// T2: `visible_rows` ≡ `list_rows` + пересечение с вьюпортом (референс
    /// в тесте): offset 0 / mid-scroll / max; частичные строки сверху и
    /// снизу усечены, модельные индексы в порядке следования; чистота
    /// (scroll не мутируется).
    #[test]
    fn t2_visible_rows_matches_list_rows_plus_intersection() {
        let mut table = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts {
                row_h: 22.0,
                row_gap: LIST_ROW_GAP,
                ..TableOpts::default()
            },
            palette: palette_a(),
        });
        table.rows = (0..10)
            .map(|i| {
                tr(
                    RowMarker::Dot,
                    &format!("row {i}"),
                    "1",
                    "",
                    None,
                    KitState::Normal,
                    TableRowStyle::default(),
                )
            })
            .collect();
        // content_h = 10·22 + 9·6 = 274 (инвариант sync_scroll §4.3)
        table.scroll = ScrollState {
            offset: 0.0,
            content_h: 10.0 * 22.0 + 9.0 * LIST_ROW_GAP,
            viewport_h: 100.0,
        };
        let viewport = UiRect::new(10.0, 20.0, 200.0, 100.0);
        let content_h = table.scroll.content_h;
        let row_h = table.props.opts.row_h;
        let row_gap = table.props.opts.row_gap;
        let count = table.rows.len();

        // Референс: kit list_rows + пересечение (клип-семантика stage)
        let reference = |offset: f32| -> Vec<(usize, UiRect)> {
            let s = ScrollState {
                offset,
                content_h,
                viewport_h: 100.0,
            };
            list_rows(viewport, &s, row_h, row_gap, count)
                .into_iter()
                .filter_map(|(i, r)| r.intersection(&viewport).map(|r| (i, r)))
                .collect()
        };

        for &offset in &[0.0, 45.0, 80.0, content_h - 100.0] {
            table.scroll.offset = offset;
            let scroll_before = table.scroll.clone();
            let got = table.visible_rows(viewport);
            assert_eq!(got, reference(offset), "offset {offset}: ≡ list_rows+∩");
            assert_eq!(
                table.scroll, scroll_before,
                "offset {offset}: чистая функция — scroll не мутируется"
            );
            assert!(!got.is_empty(), "offset {offset}: видимые строки есть");
        }

        // Частичные строки: offset 45 → сверху усечены (rect.y == viewport.y,
        // h < row_h), снизу усечены (rect.bottom() == viewport.bottom()).
        table.scroll.offset = 45.0;
        let rows = table.visible_rows(viewport);
        let indices: Vec<usize> = rows.iter().map(|(i, _)| *i).collect();
        assert_eq!(indices, vec![1, 2, 3, 4, 5], "модельные индексы по порядку");
        let (first_i, first_rect) = rows[0];
        assert_eq!(first_i, 1);
        assert_eq!(
            first_rect.y, viewport.y,
            "частичная строка сверху — усечена"
        );
        assert!(first_rect.h < row_h, "h усечён до пересечения");
        let (_, last_rect) = rows[rows.len() - 1];
        assert_eq!(
            last_rect.bottom(),
            viewport.bottom(),
            "частичная строка снизу — усечена до низа вьюпорта"
        );
        assert!(last_rect.h < row_h);
        // Полная строка посередине — h == row_h (пересечение не режет)
        let (_, mid_rect) = rows[2];
        assert_eq!(mid_rect.h, row_h, "полная строка не усечена");
        assert_eq!(mid_rect.x, viewport.x, "слот full-width по горизонтали");
        assert_eq!(mid_rect.w, viewport.w);
    }

    // === T3: paint ≡ ручной цикл =============================================

    /// T3: `paint_with` ≡ ручной цикл (visible_rows → row_layout_with →
    /// merged_style → paint_row, затем бегунок) — равенство `Vec<PaintItem>`
    /// (Painter::items); данные с override (зебра-fill, Selected,
    /// border-override) и без; + `Component::paint` ≡ `paint_with` (те же
    /// items при одинаковом состоянии замерщиков).
    #[test]
    fn t3_paint_with_matches_manual_loop() {
        let pa = palette_a();
        let pb = palette_b();
        let viewport = UiRect::new(0.0, 10.0, 240.0, 60.0);
        let mut m = TextMeasurer::new();
        let mut fs = font_system();

        // Датасет 1: с переопределениями (зебра-fill, Selected+border,
        // unit-override) и overflow → бегунок
        let rows = vec![
            tr(
                RowMarker::Dot,
                "a",
                "800",
                "rps",
                Some("← ист"),
                KitState::Normal,
                TableRowStyle {
                    fill: Some(pb.hover_fill),
                    ..TableRowStyle::default()
                },
            ),
            tr(
                RowMarker::None,
                "b",
                "20",
                "",
                None,
                KitState::Selected,
                TableRowStyle {
                    border: Some(pa.accent),
                    ..TableRowStyle::default()
                },
            ),
            tr(
                RowMarker::Glyph("ƒ"),
                "c = a + b",
                "",
                "",
                None,
                KitState::Normal,
                TableRowStyle::default(),
            ),
            tr(
                RowMarker::Dot,
                "d",
                "50",
                "req/s",
                None,
                KitState::Normal,
                TableRowStyle {
                    unit: Some(pb.text_muted),
                    ..TableRowStyle::default()
                },
            ),
        ];
        let mut table = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts {
                row_h: 22.0,
                leader: false,
                ..TableOpts::default()
            },
            palette: pa,
        });
        table.rows = rows;
        table.scroll = ScrollState {
            offset: 30.0,
            content_h: 4.0 * 22.0,
            viewport_h: viewport.h,
        };

        // Эталон: ручной цикл + бегунок последним (draw-порядок stage)
        let mut ref_p = Painter::new();
        for (index, slot) in table.visible_rows(viewport) {
            let style = table.rows[index]
                .style
                .apply(row_style(table.rows[index].state, &table.props.palette));
            let lay = table
                .row_layout_with(&mut m, &mut fs, viewport.w, slot, index)
                .unwrap();
            paint_row(
                &mut ref_p,
                &lay,
                &table.parts_of(index),
                &style,
                table.props.size,
            );
        }
        if let Some(knob) = scroll_bar(viewport, &table.scroll, &table.props.palette) {
            ref_p.rect(
                knob,
                table.props.palette.control_border,
                [0.0; 4],
                knob.w / 2.0,
            );
        }

        let mut p = Painter::new();
        table.paint_with(&mut p, &mut m, &mut fs, viewport);
        assert_eq!(
            p.items(),
            ref_p.items(),
            "paint_with ≡ ручной цикл (items дословно, порядок = draw-порядок)"
        );
        // последний item — бегунок (строки → бегунок)
        let knob = scroll_bar(viewport, &table.scroll, &table.props.palette).unwrap();
        assert_eq!(
            p.items().last(),
            Some(&PaintItem::Rect {
                rect: knob,
                fill: pa.control_border,
                border: [0.0; 4],
                radius: knob.w / 2.0,
            }),
            "бегунок: слот control_border, прозрачная рамка, радиус w/2"
        );

        // Датасет 2: без переопределений (все Normal, default style)
        let mut plain = Table::new(table.props);
        plain.rows = demo_rows();
        plain.scroll = ScrollState {
            offset: 0.0,
            content_h: 4.0 * 22.0,
            viewport_h: viewport.h,
        };
        let mut ref_p = Painter::new();
        for (index, slot) in plain.visible_rows(viewport) {
            let style = plain.rows[index]
                .style
                .apply(row_style(plain.rows[index].state, &plain.props.palette));
            let lay = plain
                .row_layout_with(&mut m, &mut fs, viewport.w, slot, index)
                .unwrap();
            paint_row(
                &mut ref_p,
                &lay,
                &plain.parts_of(index),
                &style,
                plain.props.size,
            );
        }
        if let Some(knob) = scroll_bar(viewport, &plain.scroll, &plain.props.palette) {
            ref_p.rect(
                knob,
                plain.props.palette.control_border,
                [0.0; 4],
                knob.w / 2.0,
            );
        }
        let mut p = Painter::new();
        plain.paint_with(&mut p, &mut m, &mut fs, viewport);
        assert_eq!(p.items(), ref_p.items(), "без override: paint_with ≡ цикл");

        // Component::paint ≡ paint_with: owned-слой, одинаковые замерщики
        let mut owned = Table::new(table.props);
        owned.rows = table.rows.clone();
        owned.scroll = table.scroll;
        load_display_font(&mut owned.font_system.borrow_mut());
        let rects = owned.layout(default_backend(), viewport);
        assert!(!rects.is_empty(), "layout: видимые строки есть");
        let mut p1 = Painter::new();
        owned.paint(&mut p1, &rects);
        let mut p2 = Painter::new();
        {
            let mut m2 = owned.measurer.borrow_mut();
            let mut fs2 = owned.font_system.borrow_mut();
            owned.paint_with(&mut p2, &mut m2, &mut fs2, viewport);
        }
        assert_eq!(p1.items(), p2.items(), "Component::paint ≡ paint_with");
        // последний item — бегунок (draw-порядок: строки → бегунок)
        let knob = scroll_bar(viewport, &owned.scroll, &owned.props.palette).unwrap();
        assert_eq!(
            p1.items().last(),
            Some(&PaintItem::Rect {
                rect: knob,
                fill: owned.props.palette.control_border,
                border: [0.0; 4],
                radius: knob.w / 2.0,
            }),
            "Component::paint: строки → бегунок последним"
        );
        assert!(!p1.items().is_empty());
    }

    // === T4: деградация ≡ формулы stage ======================================

    /// T4: все правые ячейки пусты (только label, как формулы stage) →
    /// `guides_with` = `None`; `row_layout_with`: `label_right = slot.right()
    /// − ROW_TEXT_GAP`, value/unit rect'ы нулевые (паритет stage.rs:1386–1402).
    /// Смешанный случай: у одной строки из нескольких непустой value →
    /// guides Some, пустые unit/badge не съедают ширину живой колонки (§6).
    #[test]
    fn t4_degradation_matches_stage_manual_guides() {
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        const W: f32 = 300.0;
        let slot = UiRect::new(0.0, 0.0, W, 22.0);

        // Все правые ячейки пусты → направляющих нет
        let mut table = Table::new(TableProps {
            size: 11.0,
            family: FAMILY,
            opts: TableOpts {
                leader: false,
                right_pad: 0.0,
                ..TableOpts::default()
            },
            palette: palette_a(),
        });
        table.rows = formula_rows();
        assert!(
            table.guides_with(&mut m, &mut fs, W).is_none(),
            "правые колонки пусты у всех строк → None (§4.2)"
        );

        // row_layout_with: деградированный вход от правого края строки
        let lay = table
            .row_layout_with(&mut m, &mut fs, W, slot, 0)
            .expect("index 0 валиден");
        assert!(
            (lay.label.right() - (slot.right() - ROW_TEXT_GAP)).abs() < 1e-3,
            "label_right = slot.right() − ROW_TEXT_GAP (паритет stage.rs:1386–1402)"
        );
        assert_eq!(
            lay.value,
            UiRect::new(0.0, 0.0, 0.0, 0.0),
            "пустое значение — нулевой rect"
        );
        assert_eq!(
            lay.unit,
            UiRect::new(0.0, 0.0, 0.0, 0.0),
            "пустой юнит — нулевой rect"
        );
        assert!(lay.badge.is_none(), "пустой бейдж — пилюли нет");
        assert!(lay.leader.is_none(), "лидер off (opts) и значения нет");

        // Бит-в-бит с ручной деградацией stage (1386–1392)
        let manual_guides = RowGuides {
            value_w: 0.0,
            unit_w: 0.0,
            badge_w: 0.0,
            value_x: slot.right(),
            unit_x: slot.right(),
        };
        let manual_lay = row_layout(
            &mut m,
            &mut fs,
            FAMILY,
            11.0,
            slot,
            manual_guides,
            &table.parts_of(0),
            &RowOpts {
                leader: false,
                gap: table.props.opts.gap,
            },
        );
        assert_eq!(
            lay, manual_lay,
            "row_layout_with ≡ ручная деградация stage (RowLayout дословно)"
        );

        // Смешанный случай: одна строка из нескольких с непустым value → Some
        let mut formulas = formula_rows();
        let first_formula = formulas.remove(0);
        let second_formula = formulas.remove(0); // бывший №1
        let mut mixed = Table::new(table.props);
        mixed.rows = vec![
            first_formula,
            tr(
                RowMarker::Dot,
                "ставка",
                "13.5",
                "",
                None,
                KitState::Normal,
                TableRowStyle::default(),
            ),
            second_formula,
        ];
        let g = mixed
            .guides_with(&mut m, &mut fs, W)
            .expect("живая колонка значения есть");
        let right_edge = W - mixed.props.opts.right_pad;
        assert_eq!(g.badge_w, 0.0, "пустой бейдж не ест ширину колонки (§6)");
        assert_eq!(g.unit_w, 0.0, "пустой юнит не ест ширину колонки (§6)");
        assert_eq!(
            g.value_w,
            m.width_of(&mut fs, "13.5", FAMILY, 11.0),
            "value_w — max по строкам модели"
        );
        // §6: пустые unit/badge не съедают ШИРИНУ живой колонки значения —
        // структурные зазоры цепочки value|unit|badge сохраняются
        // (with_right_edge вычитает gap на каждой границе — их две), т.е.
        // value_right = right_edge − 2·gap. Мастер-оракул бит-в-бит — T1
        // (≡ kit row_guides): арифметика with_right_edge дословна.
        assert_eq!(
            g.value_right(),
            right_edge - 2.0 * canvas_core::tokens::TABLE_GUIDE_GAP,
            "value_right: right_edge минус структурные зазоры цепочки"
        );
        // Формульная строка в смешанной таблице: value/unit пусты — текст до
        // края строки (label_right без направляющих, §4.2 замечание)
        let lay = mixed.row_layout_with(&mut m, &mut fs, W, slot, 0).unwrap();
        assert!(
            (lay.label.right() - (slot.right() - ROW_TEXT_GAP)).abs() < 1e-3,
            "формула в смешанной таблице — до края строки"
        );
    }

    // === T5: детерминизм/порядок =============================================

    /// T5: перестановка строк не меняет направляющие (max коммутативен);
    /// повторный вызов идентичен (инвариант 2 FR-050 — повторный пересчёт
    /// не «дышит»); свежие замерщики — тот же результат.
    #[test]
    fn t5_guides_are_deterministic_and_order_free() {
        let mut table = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts::default(),
            palette: palette_a(),
        });
        table.rows = demo_rows();
        let mut m = TextMeasurer::new();
        let mut fs = font_system();
        const W: f32 = 280.0;

        let g1 = table.guides_with(&mut m, &mut fs, W);
        let g2 = table.guides_with(&mut m, &mut fs, W);
        assert_eq!(g1, g2, "повторный вызов идентичен (кэш не «дышит»)");

        table.rows.reverse();
        let g3 = table.guides_with(&mut m, &mut fs, W);
        assert_eq!(g1, g3, "перестановка строк не меняет направляющие");

        // Свежие замерщики (новый кэш/FontSystem) — тот же результат
        let mut m2 = TextMeasurer::new();
        let mut fs2 = font_system();
        assert_eq!(
            table.guides_with(&mut m2, &mut fs2, W),
            g1,
            "замер детерминирован относительно состояния кэша"
        );
    }

    // === T6: content_h / model_index_at / clamp ==============================

    /// T6: `set_rows` синхронизирует `content_h = n·row_h + (n−1)·row_gap`
    /// (пустая модель — 0, без отрицательного хвоста); `model_index_at`
    /// внутри/вне вьюпорта и в зазоре; смещённый offset — модельный индекс
    /// скролл-независим; `scroll.clamp` идемпотентен.
    #[test]
    fn t6_set_rows_syncs_content_h_and_model_index_at() {
        let mut table = Table::new(TableProps {
            size: SIZE,
            family: FAMILY,
            opts: TableOpts {
                row_h: LIST_ROW_H,
                row_gap: LIST_ROW_GAP,
                ..TableOpts::default()
            },
            palette: palette_a(),
        });
        let model = |n: usize| -> Vec<TableRow> {
            (0..n)
                .map(|i| {
                    tr(
                        RowMarker::Dot,
                        &format!("row {i}"),
                        "1",
                        "",
                        None,
                        KitState::Normal,
                        TableRowStyle::default(),
                    )
                })
                .collect()
        };
        table.set_rows(model(5));
        assert_eq!(
            table.scroll.content_h,
            5.0 * LIST_ROW_H + 4.0 * LIST_ROW_GAP,
            "content_h = n·row_h + (n−1)·row_gap (§4.3)"
        );
        table.set_rows(model(1));
        assert_eq!(
            table.scroll.content_h, LIST_ROW_H,
            "одна строка — без хвоста"
        );
        table.set_rows(Vec::new());
        assert_eq!(
            table.scroll.content_h, 0.0,
            "пустая модель — content_h = 0 (n−1 клампится)"
        );

        // model_index_at: модельные индексы видимых строк (stride 32)
        table.set_rows(model(5));
        let viewport = UiRect::new(0.0, 0.0, 200.0, 100.0);
        table.scroll.viewport_h = viewport.h;
        table.scroll.offset = 0.0;
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 10.0)),
            Some(0),
            "внутри строки 0"
        );
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 40.0)),
            Some(1),
            "внутри строки 1"
        );
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 98.0)),
            Some(3),
            "частичная строка снизу (96..100) — модельный индекс 3"
        );
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 29.0)),
            None,
            "зазор между строками — ни одна (row_gap > 0)"
        );
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 150.0)),
            None,
            "вне вьюпорта — None"
        );
        // Смещённый offset: частичная строка сверху — модельный индекс
        // скролл-независим (инвариант 3 FR-059)
        table.scroll.offset = 40.0;
        assert_eq!(
            table.model_index_at(viewport, UiPoint::new(100.0, 5.0)),
            Some(1),
            "частичная строка сверху (модельный индекс 1)"
        );

        // clamp идемпотентен
        table.scroll.offset = 1000.0;
        table.scroll.clamp();
        let once = table.scroll.offset;
        table.scroll.clamp();
        assert_eq!(table.scroll.offset, once, "clamp идемпотентен");
        assert_eq!(table.scroll.offset, table.scroll.max_offset());
    }
}
