//! FR-051 U1 (PRD-0009 F-2/F-5/F-11): кадр экрана — снимок геометрии
//! поверхностей за кадр. Сборка кадра из реестра (адаптеры поверхностей —
//! U2); клип-политика поверхности обязательна (F-5 precursor); детектор
//! пересечений интерактивных rect'ов одного слоя — precursor layout-линта
//! F-11 (D1: «налезание из мистики — в два rect'а с именами»).

use crate::capture::CapturePolicy;
use crate::geometry::UiRect;
use crate::layer::UiLayer;
use crate::registry::{SurfaceId, SurfaceRegistry};

/// Хит-прямоугольник элемента поверхности. Имя элемента обязательно
/// (подписи debug-оверлея и сообщений линта).
#[derive(Debug, Clone, PartialEq)]
pub struct HitRect {
    pub rect: UiRect,
    pub element: String,
    /// Интерактивный rect участвует в pick; неинтерактивный — только в линтах
    /// и debug-оверлее (декор, подписи).
    pub interactive: bool,
}

impl HitRect {
    pub fn interactive(rect: UiRect, element: impl Into<String>) -> Self {
        Self {
            rect,
            element: element.into(),
            interactive: true,
        }
    }

    pub fn decoration(rect: UiRect, element: impl Into<String>) -> Self {
        Self {
            rect,
            element: element.into(),
            interactive: false,
        }
    }
}

/// Кадр одной поверхности. Клип обязателен: renderer отсекает по нему
/// scissor-бакетами (F-5, U2+); линты проверяют выход контента за клип.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceFrame {
    pub surface: SurfaceId,
    pub layer: UiLayer,
    pub capture: CapturePolicy,
    pub clip: UiRect,
    pub hit_rects: Vec<HitRect>,
}

impl SurfaceFrame {
    pub fn new(
        id: impl Into<String>,
        layer: UiLayer,
        capture: CapturePolicy,
        clip: UiRect,
    ) -> Self {
        Self {
            surface: SurfaceId::new(id),
            layer,
            capture,
            clip,
            hit_rects: Vec::new(),
        }
    }

    pub fn with_hit_rect(mut self, r: HitRect) -> Self {
        self.hit_rects.push(r);
        self
    }

    /// Верхний (последний зарегистрированный) интерактивный rect, содержащий
    /// точку. `Block`-backdrop обрабатывается `HitStack`, а не здесь.
    pub fn top_hit_at(&self, p: crate::geometry::UiPoint) -> Option<&HitRect> {
        self.hit_rects
            .iter()
            .rev()
            .find(|r| r.interactive && r.rect.contains(p))
    }
}

/// Кадр экрана: снимок видимых поверхностей за кадр (в порядке регистрации).
/// UiFrame собирается за кадр без аллокаций сверх практики (PRD-0009 §9.2;
/// переиспользование Vec — задача адаптеров U2).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiFrame {
    pub viewport: UiRect,
    pub surfaces: Vec<SurfaceFrame>,
}

/// Пересечение интерактивных rect'ов двух поверхностей одной полосы
/// (материал layout-линта F-11 a).
#[derive(Debug, Clone, PartialEq)]
pub struct Overlap<'a> {
    pub layer: UiLayer,
    pub a_surface: &'a SurfaceId,
    pub a_element: &'a str,
    pub b_surface: &'a SurfaceId,
    pub b_element: &'a str,
    pub at: UiRect,
}

impl UiFrame {
    /// Пустой кадр с вьюпортом.
    pub fn new(viewport: UiRect) -> Self {
        Self {
            viewport,
            surfaces: Vec::new(),
        }
    }

    /// Заготовка кадра из реестра: видимые при данном вьюпорте поверхности
    /// (hide-политика деградации — F-11 c), клип = вьюпорт, hit-rect'ы
    /// заполняются адаптерами поверхностей (U2).
    pub fn from_registry(registry: &SurfaceRegistry, viewport: UiRect) -> Self {
        let mut frame = Self::new(viewport);
        for decl in registry.visible_at(viewport.w, viewport.h) {
            frame.surfaces.push(SurfaceFrame {
                surface: decl.id.clone(),
                layer: decl.layer,
                capture: decl.capture,
                clip: viewport,
                hit_rects: Vec::new(),
            });
        }
        frame
    }

    /// Порядок pick-обхода: индексы поверхностей сверху вниз (реверс
    /// стабильной сортировки по слою; внутри слоя — реверс регистрации).
    pub fn pick_order(&self) -> Vec<usize> {
        let mut idx: Vec<usize> = (0..self.surfaces.len()).collect();
        idx.sort_by_key(|&i| self.surfaces[i].layer);
        idx.reverse();
        idx
    }

    /// Порядок draw-полос кадра: (слой, индексы в порядке регистрации) —
    /// зеркало `SurfaceRegistry::draw_bands` на кадре.
    pub fn draw_bands(&self) -> Vec<(UiLayer, Vec<usize>)> {
        let mut bands = Vec::new();
        for layer in UiLayer::DRAW_ORDER {
            let idx: Vec<usize> = self
                .surfaces
                .iter()
                .enumerate()
                .filter(|(_, s)| s.layer == layer)
                .map(|(i, _)| i)
                .collect();
            if !idx.is_empty() {
                bands.push((layer, idx));
            }
        }
        bands
    }

    /// Пересечения интерактивных rect'ов поверхностей одной полосы
    /// (линт F-11 a: 0 пересечений на canonical сценах; U5 — в CI).
    pub fn overlaps_within_layer(&self) -> Vec<Overlap<'_>> {
        let mut out = Vec::new();
        for (layer, idx) in self.draw_bands() {
            for (a_pos, &i) in idx.iter().enumerate() {
                for &j in idx.iter().skip(a_pos + 1) {
                    let sa = &self.surfaces[i];
                    let sb = &self.surfaces[j];
                    for ra in sa.hit_rects.iter().filter(|r| r.interactive) {
                        for rb in sb.hit_rects.iter().filter(|r| r.interactive) {
                            if let Some(at) = ra.rect.intersection(&rb.rect) {
                                out.push(Overlap {
                                    layer,
                                    a_surface: &sa.surface,
                                    a_element: &ra.element,
                                    b_surface: &sb.surface,
                                    b_element: &rb.element,
                                    at,
                                });
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturePolicy;
    use crate::geometry::{UiPoint, UiRect};
    use crate::registry::{DegradationPolicy, SurfaceDecl, SurfaceRegistry};

    fn vp() -> UiRect {
        UiRect::new(0.0, 0.0, 1280.0, 800.0)
    }

    #[test]
    fn from_registry_applies_hide_degradation() {
        let mut reg = SurfaceRegistry::new();
        reg.add(
            SurfaceDecl::new("whatif", UiLayer::Panels, CapturePolicy::Capture).with_degradation(
                DegradationPolicy::HideBelow {
                    min_width: 900.0,
                    min_height: 600.0,
                },
            ),
        );
        reg.add(SurfaceDecl::new(
            "toast",
            UiLayer::Toasts,
            CapturePolicy::Passive,
        ));
        assert_eq!(UiFrame::from_registry(&reg, vp()).surfaces.len(), 2);
        // 800×560 — whatif скрыт, кадр содержит только тосты
        let small = UiFrame::from_registry(&reg, UiRect::new(0.0, 0.0, 800.0, 560.0));
        assert_eq!(small.surfaces.len(), 1);
        assert_eq!(small.surfaces[0].surface.as_str(), "toast");
    }

    #[test]
    fn pick_order_is_top_layers_first_registration_reversed_within_layer() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(SurfaceFrame::new(
            "world",
            UiLayer::World,
            CapturePolicy::PassThrough,
            vp(),
        ));
        frame.surfaces.push(SurfaceFrame::new(
            "panel_a",
            UiLayer::Panels,
            CapturePolicy::Capture,
            vp(),
        ));
        frame.surfaces.push(SurfaceFrame::new(
            "panel_b",
            UiLayer::Panels,
            CapturePolicy::Capture,
            vp(),
        ));
        frame.surfaces.push(SurfaceFrame::new(
            "modal",
            UiLayer::Modals,
            CapturePolicy::Block,
            vp(),
        ));
        let order: Vec<&str> = frame
            .pick_order()
            .into_iter()
            .map(|i| frame.surfaces[i].surface.as_str())
            .collect();
        assert_eq!(order, vec!["modal", "panel_b", "panel_a", "world"]);
    }

    #[test]
    fn draw_bands_mirror_registry_order() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(SurfaceFrame::new(
            "a",
            UiLayer::Panels,
            CapturePolicy::Capture,
            vp(),
        ));
        frame.surfaces.push(SurfaceFrame::new(
            "m",
            UiLayer::Modals,
            CapturePolicy::Block,
            vp(),
        ));
        frame.surfaces.push(SurfaceFrame::new(
            "b",
            UiLayer::Panels,
            CapturePolicy::Capture,
            vp(),
        ));
        let bands = frame.draw_bands();
        assert_eq!(bands.len(), 2);
        assert_eq!(bands[0].0, UiLayer::Panels);
        assert_eq!(bands[0].1, vec![0, 2]);
        assert_eq!(bands[1].0, UiLayer::Modals);
    }

    #[test]
    fn overlap_detector_reports_same_layer_intersections_with_names() {
        let mut frame = UiFrame::new(vp());
        let bar = UiRect::new(0.0, 760.0, 1280.0, 40.0);
        let chip = UiRect::new(1200.0, 750.0, 60.0, 24.0);
        frame.surfaces.push(
            SurfaceFrame::new("whatif", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(bar, "whatif-bar")),
        );
        frame.surfaces.push(
            SurfaceFrame::new("wheel_hint", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(chip, "wheel-hint-chip")),
        );
        // верхняя полоса (тосты) с пересечением по координатам — не линтуется:
        // разные полосы, правило — только внутри одной
        frame.surfaces.push(
            SurfaceFrame::new("toast", UiLayer::Toasts, CapturePolicy::Passive, vp())
                .with_hit_rect(HitRect::decoration(chip, "toast-text")),
        );
        let overlaps = frame.overlaps_within_layer();
        assert_eq!(overlaps.len(), 1);
        let o = &overlaps[0];
        assert_eq!(o.layer, UiLayer::Panels);
        assert_eq!(
            (o.a_surface.as_str(), o.a_element),
            ("whatif", "whatif-bar")
        );
        assert_eq!(
            (o.b_surface.as_str(), o.b_element),
            ("wheel_hint", "wheel-hint-chip")
        );
        assert_eq!(o.at, UiRect::new(1200.0, 760.0, 60.0, 14.0));
    }

    #[test]
    fn passive_surfaces_declare_no_interactive_rects_by_contract() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(
            SurfaceFrame::new("toast", UiLayer::Toasts, CapturePolicy::Passive, vp())
                .with_hit_rect(HitRect::decoration(
                    UiRect::new(500.0, 20.0, 300.0, 40.0),
                    "toast",
                )),
        );
        assert!(frame.surfaces[0]
            .top_hit_at(UiPoint::new(650.0, 40.0))
            .is_none());
        // пассивная поверхность не даёт ни одного перехвата
        assert!(crate::hit::HitStack::pick(&frame, UiPoint::new(650.0, 40.0)).is_none());
    }
}
