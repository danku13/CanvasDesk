//! FR-068 W3: панель/карточка/конструкторы стилей — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-a): [`PanelProps`]/[`Panel`] + `impl Component`
//! добавлены (см. секцию «Component» внизу файла); существующие функции —
//! стабильный API кита (§Контракт-1 PRD-0009 V-5).

use super::{Component, ControlStyle, KitPalette, PanelStyle};
use crate::geometry::{EdgeInsets, UiRect, UiVec2};
use crate::layout::{constrain, stack, HAlign, LayoutBackend, VAlign};
use crate::paint::Painter;

// --- Panel ------------------------------------------------------------------

/// Стиль панели: заливка/рамка `panel_*`, радиус RADIUS_PANEL, пад SPACING_LG.
pub fn panel_style(p: &KitPalette) -> PanelStyle {
    PanelStyle {
        fill: p.panel_fill,
        border: p.panel_border,
        radius: canvas_core::tokens::RADIUS_PANEL,
        pad: EdgeInsets::uniform(canvas_core::tokens::SPACING_LG),
    }
}

/// Панель-контейнер: размер = constrain(min,max,desired) в слоте, позиция =
/// stack (выравнивание задаёт потребитель). Возвращает rect панели.
pub fn panel_rect(slot: UiRect, min: UiVec2, max: UiVec2, desired: UiVec2) -> UiRect {
    let size = constrain(min, max, desired);
    stack(slot, size, HAlign::Center, VAlign::Center)
}

/// Внутренняя область контента панели (минус пад стиля).
pub fn panel_content(panel: UiRect, style: &PanelStyle) -> UiRect {
    panel.inset(&style.pad)
}

// --- Явные слоты (миграция пилотов: I-1 ноль скачка) ------------------------

/// Стиль панели из ЯВНЫХ слотов темы поверхности. Контракт F-8 сохраняется:
/// аргументы — значения слотов `ThemeColors` (потребитель передаёт слот, кит
/// не изобретает цветов); радиус — из radius-scale/каноническая константа
/// поверхности (миграция I-1: ноль визуального скачка — приоритет над
/// унификацией радиусов, унификация — v2 с токен-паритетом).
pub fn panel_style_of(fill: [f32; 4], border: [f32; 4], radius: f32, pad: f32) -> PanelStyle {
    PanelStyle {
        fill,
        border,
        radius,
        pad: EdgeInsets::uniform(pad),
    }
}

/// Стиль контрола из ЯВНЫХ слотов (см. [`panel_style_of`]).
pub fn control_style_of(
    fill: [f32; 4],
    border: [f32; 4],
    text: [f32; 4],
    radius: f32,
) -> ControlStyle {
    ControlStyle {
        fill,
        border,
        text,
        radius,
    }
}

// --- Card -------------------------------------------------------------------

/// Раскладка контентной карточки с хедером: внешний rect + хедер + body.
/// Хедер и body — внутри пад панели (`panel_style(p).pad` = SPACING_LG):
/// заголовок не впритык к краю, контент — ниже хедера с тем же падом.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CardLayout {
    /// Внешний rect карточки (constrain+stack в слоте).
    pub rect: UiRect,
    /// Rect хедера (внутри пада; высота = `header_h` клампнутая к остатку).
    pub header: UiRect,
    /// Rect body (внутри пада; ниже хедера).
    pub body: UiRect,
}

/// Карточка в слоте: внешний rect = constrain+stack; хедер и body — внутри
/// пада панели (`panel_style(p).pad`). Палитра — слот фона/рамки (потребитель
/// рисует через `panel_style(p)` отдельно; контракт: цвет отдельно от геометрии).
pub fn card(slot: UiRect, min: UiVec2, max: UiVec2, header_h: f32, p: &KitPalette) -> CardLayout {
    let pad = panel_style(p).pad;
    let rect = panel_rect(slot, min, max, max);
    let inner = rect.inset(&pad);
    let hh = header_h.min(inner.h);
    let header = UiRect::new(inner.x, inner.y, inner.w, hh);
    let body = UiRect::new(inner.x, inner.y + hh, inner.w, (inner.h - hh).max(0.0));
    CardLayout { rect, header, body }
}

// === Component (FR-068 W3, агент 3-a) =======================================

/// Свойства панели — декларативный вход кадра (FR-068 W3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelProps {
    /// Нижняя граница размера ([`constrain`]; клампится к ≥ 0).
    pub min: UiVec2,
    /// Верхняя граница размера ([`constrain`]; нижняя граница приоритетна).
    pub max: UiVec2,
    /// Желаемый размер (зажимается в [min, max]).
    pub desired: UiVec2,
    /// Палитра-срез: заливка/рамка — слоты `panel_*` (контракт F-8).
    pub palette: KitPalette,
}

/// Панель как [`Component`] — НЕинтерактивный контейнер поверхности:
/// [`WidgetState`](crate::widget::WidgetState) не нужен — стиль панели
/// (`panel_style`) не зависит от hover/pressed/disabled, хранить состояние
/// нечего (FR-068 §W3: `state` — только интерактивным компонентам).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    /// Свойства кадра.
    pub props: PanelProps,
}

impl Panel {
    /// Новая панель с заданными свойствами.
    pub fn new(props: PanelProps) -> Self {
        Self { props }
    }
}

impl Component for Panel {
    type Props = PanelProps;

    fn props(&self) -> &PanelProps {
        &self.props
    }

    /// Два rect'а в ФИКСИРОВАННОМ порядке (контракт среза rects):
    ///
    /// - `rects[0]` — сама панель: [`panel_rect`] =
    ///   constrain(min, max, desired) + stack по центру слота;
    /// - `rects[1]` — контент: `rects[0]` минус пад [`panel_style`]
    ///   ([`panel_content`], SPACING_LG) — готовый слот для детей
    ///   потребителя.
    ///
    /// `backend` не участвует: раскладка одной панели — constrain+stack,
    /// контейнерной вёрстки нет (сигнатура kit-функций НЕ менялась —
    /// §Контракт-1 FR-068). Усечение до одного rect'а допустимо только
    /// у ПОТРЕБИТЕЛЯ (`paint` терпит срез из одного элемента).
    fn layout(&self, _backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect> {
        let panel = panel_rect(slot, self.props.min, self.props.max, self.props.desired);
        let content = panel_content(panel, &panel_style(&self.props.palette));
        vec![panel, content]
    }

    /// Рисует `rects[0]` панелью: заливка/рамка — слоты `panel_*`,
    /// радиус RADIUS_PANEL ([`panel_style`]). `rects[1]` (контент, если
    /// в срезе больше одного rect'а) отрисовке НЕ подлежит — это
    /// геометрия детей потребителя (пад — забота раскладки, см.
    /// [`Painter::panel`]); потребитель сам рисует детей в этот слот.
    /// Пустой `rects` — no-op (деградация, не паника).
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]) {
        let Some(&rect) = rects.first() else {
            return;
        };
        painter.panel(rect, &panel_style(&self.props.palette));
    }

    // hit_test — дефолтный из [`Component`]: панель неинтерактивна, но
    // pick по `rects[0]` (панель раньше контента в срезе) полезен
    // потребителю поверх детей; переопределение не требуется (план
    // FR-068 §W3).
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::button::switch;
    use crate::component::modal::modal;
    use crate::component::test_support::palette_a;
    use crate::component::KitState;
    #[test]
    fn panel_and_modal_are_centered_and_clamped() {
        let vp = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let panel = panel_rect(
            vp,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(600.0, 400.0),
            UiVec2::new(900.0, 500.0),
        );
        // desired зажат max → 600×400, центр вьюпорта
        assert!((panel.w - 600.0).abs() < 0.01);
        assert!((panel.x - (1280.0 - 600.0) / 2.0).abs() < 0.01);
        let modal = modal(
            vp,
            UiVec2::new(280.0, 150.0),
            UiVec2::new(440.0, 150.0),
            UiVec2::new(440.0, 150.0),
        );
        assert!((modal.panel.x - (1280.0 - 440.0) / 2.0).abs() < 0.01);
        assert_eq!(modal.dim, vp);
        // Контент панели минус пад
        let style = panel_style(&palette_a());
        let content = panel_content(panel, &style);
        assert!((content.x - (panel.x + style.pad.left)).abs() < 0.01);
    }
    #[test]
    fn card_geometry_in_slot_with_header_and_body() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let p = palette_a();
        let lay = card(
            slot,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(400.0, 300.0),
            30.0,
            &p,
        );
        // rect = 400×300 (по max)
        assert!((lay.rect.w - 400.0).abs() < 0.01);
        assert!((lay.rect.h - 300.0).abs() < 0.01);
        assert!((lay.rect.x - slot.x).abs() < 0.01);
        // header и body — внутри пада SPACING_LG (12)
        let pad = canvas_core::tokens::SPACING_LG;
        assert!((lay.header.x - (lay.rect.x + pad)).abs() < 0.01);
        assert!((lay.header.y - (lay.rect.y + pad)).abs() < 0.01);
        assert!((lay.header.w - (lay.rect.w - 2.0 * pad)).abs() < 0.01);
        assert!((lay.header.h - 30.0).abs() < 0.01, "header_h как передано");
        // body — ниже header, внутри пада
        assert!((lay.body.x - lay.header.x).abs() < 0.01);
        assert!((lay.body.y - (lay.header.y + lay.header.h)).abs() < 0.01);
        assert!((lay.body.w - lay.header.w).abs() < 0.01);
        assert!((lay.body.bottom() - (lay.rect.bottom() - pad)).abs() < 0.01);
    }
    #[test]
    fn card_clamps_to_min_max() {
        let slot = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let p = palette_a();
        // desired = max = 400×300 → clamp к max
        let lay = card(
            slot,
            UiVec2::new(200.0, 100.0),
            UiVec2::new(400.0, 300.0),
            30.0,
            &p,
        );
        assert!((lay.rect.w - 400.0).abs() < 0.01);
        assert!((lay.rect.h - 300.0).abs() < 0.01);
    }
    #[test]
    fn card_header_h_clamped_when_too_tall() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let p = palette_a();
        // header_h = 200, но inner.h < 200 → header сжимается до inner.h
        let lay = card(
            slot,
            UiVec2::new(100.0, 50.0),
            UiVec2::new(200.0, 100.0),
            200.0,
            &p,
        );
        let pad = canvas_core::tokens::SPACING_LG;
        let inner_h = (lay.rect.h - 2.0 * pad).max(0.0);
        assert!(
            (lay.header.h - inner_h).abs() < 0.01,
            "header_h сжат до inner.h"
        );
        // body — нулевой (всё съел header)
        assert!(lay.body.h.abs() < 0.01);
    }

    // --- icon_glyph: маппинг полон ------------------------------------------
    /// Золотая геометрия Switch (фикс-слоты: трек/курок) и Card
    /// (хедер/тело) — изменение раскладки ловится эталоном.
    #[test]
    fn snapshot_switch_and_card_geometry_golden() {
        use crate::testing::{assert_snapshot, snap};
        let p = palette_a();
        // Switch: трек 36×20 по центру слота; бегунок 16×16 (on — справа)
        let slot = UiRect::new(10.0, 20.0, 200.0, 40.0);
        let sw = switch(slot, true, KitState::Normal, &p);
        assert_snapshot(
            format!("{}\n{}", snap("track", sw.track), snap("knob", sw.knob)),
            "track x=92 y=30 w=36 h=20\nknob x=110 y=32 w=16 h=16",
        );
        // Card: пад панели SPACING_LG=12; хедер 24, тело — остаток
        let card = card(
            UiRect::new(0.0, 0.0, 400.0, 120.0),
            UiVec2::new(0.0, 120.0),
            UiVec2::new(400.0, 120.0),
            24.0,
            &p,
        );
        assert_snapshot(
            format!(
                "{}\n{}",
                snap("card_header", card.header),
                snap("card_body", card.body)
            ),
            "card_header x=12 y=12 w=376 h=24\ncard_body x=12 y=36 w=376 h=72",
        );
    }

    // --- Row (FR-061 D-15, этап E) ------------------------------------------

    // === FR-068 W3: Component Panel (агент 3-a) =============================
    use crate::component::ComponentHit;
    use crate::geometry::UiPoint;
    use crate::layout::default_backend;
    use crate::paint::PaintItem;

    fn props() -> PanelProps {
        PanelProps {
            min: UiVec2::new(200.0, 100.0),
            max: UiVec2::new(600.0, 400.0),
            desired: UiVec2::new(900.0, 500.0),
            palette: palette_a(),
        }
    }

    /// Component::layout: [панель, контент] — фиксированный порядок;
    /// оба rect'а внутри слота, контент внутри панели (минус пад
    /// SPACING_LG), desired зажат max.
    #[test]
    fn component_panel_layout_panel_then_content() {
        let slot = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let panel = Panel::new(props());
        let rects = panel.layout(default_backend(), slot);
        assert_eq!(rects.len(), 2, "rects[0] — панель, rects[1] — контент");
        let p = rects[0];
        // desired 900×500 зажат max → 600×400; панель внутри слота.
        assert!((p.w - 600.0).abs() < 0.01);
        assert!((p.h - 400.0).abs() < 0.01);
        assert!(
            p.x >= slot.x && p.right() <= slot.right() + 0.01,
            "в слоте по X"
        );
        assert!(
            p.y >= slot.y && p.bottom() <= slot.bottom() + 0.01,
            "в слоте по Y"
        );
        // Контент = панель минус пад стиля (SPACING_LG) — внутри панели.
        let pad = canvas_core::tokens::SPACING_LG;
        let c = rects[1];
        assert!((c.x - (p.x + pad)).abs() < 0.01);
        assert!((c.y - (p.y + pad)).abs() < 0.01);
        assert!((c.w - (p.w - 2.0 * pad)).abs() < 0.01);
        assert!((c.h - (p.h - 2.0 * pad)).abs() < 0.01);
        assert!(c.x >= p.x && c.right() <= p.right() + 0.01);
        assert!(c.y >= p.y && c.bottom() <= p.bottom() + 0.01);
    }

    /// Component::paint: ровно один Rect со слотами [`panel_style`]
    /// (panel_fill/panel_border/RADIUS_PANEL); контент-rect отрисовке
    /// не подлежит; пустой rects — no-op.
    #[test]
    fn component_panel_paint_emits_panel_rect() {
        let panel = Panel::new(props());
        let pal = panel.props.palette;
        let rects = vec![
            UiRect::new(5.0, 5.0, 300.0, 200.0),
            UiRect::new(17.0, 17.0, 276.0, 176.0),
        ];
        let mut painter = Painter::new();
        panel.paint(&mut painter, &rects);
        assert!(!painter.items().is_empty());
        assert_eq!(painter.items().len(), 1, "контент (rects[1]) не рисуется");
        assert_eq!(
            painter.items()[0],
            PaintItem::Rect {
                rect: rects[0],
                fill: pal.panel_fill,
                border: pal.panel_border,
                radius: canvas_core::tokens::RADIUS_PANEL,
            }
        );
        // Пустой rects — no-op, не паника.
        let mut painter = Painter::new();
        panel.paint(&mut painter, &[]);
        assert!(painter.items().is_empty());
    }

    /// Component::hit_test (дефолт из трейта): точка внутри панели →
    /// `ComponentHit { index: 0 }` (панель раньше контента в срезе),
    /// вне — None.
    #[test]
    fn component_panel_hit_test_picks_panel_rect() {
        let panel = Panel::new(props());
        let rects = panel.layout(default_backend(), UiRect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(rects.len(), 2);
        let inside = UiPoint::new(rects[0].x + 1.0, rects[0].y + 1.0);
        assert_eq!(
            panel.hit_test(&rects, inside),
            Some(ComponentHit { index: 0 })
        );
        let outside = UiPoint::new(rects[0].right() + 10.0, rects[0].y + 1.0);
        assert_eq!(panel.hit_test(&rects, outside), None);
    }
}
