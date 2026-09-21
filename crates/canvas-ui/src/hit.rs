//! FR-051 U1 (PRD-0009 F-3, §7.3): `HitStack` — реверс-обход слоёв кадра
//! с capture-политиками. Заменяет приоритетную if-цепочку `on_left_button`
//! и глушение hover списком (app.rs:10519) правилом.
//!
//! Алгоритм — адаптация семантики egui `hit_test.rs` (реверс-обход, верхний
//! непрозрачный скрывает нижнего; анализ 2026-09-22): порядок обхода —
//! [`UiFrame::pick_order`]; `Block` глотает клик в любой точке (backdrop),
//! `Capture` перехватывает только в своих rect'ах, `PassThrough`/`Passive`
//! пропускают.

use crate::capture::CapturePolicy;
use crate::frame::{HitRect, SurfaceFrame, UiFrame};
use crate::geometry::UiPoint;

/// Результат pick'а: кто получил клик.
#[derive(Debug, Clone, PartialEq)]
pub enum HitTarget<'a> {
    /// Интерактивный rect элемента поверхности.
    Element {
        surface: &'a SurfaceFrame,
        rect: &'a HitRect,
    },
    /// Backdrop модальной поверхности (клик мимо её rect'ов — глотается).
    Backdrop { surface: &'a SurfaceFrame },
}

/// Реверс-обход слоёв с capture-политиками (чистая функция — детерминизм,
/// PRD-0009 §9.6).
pub struct HitStack;

impl HitStack {
    /// Кто получает клик в точке `p`. `None` — клик уходит канвасу (мир L0
    /// обрабатывается существующим конвейером, PRD-0009 §10).
    pub fn pick<'a>(frame: &'a UiFrame, p: UiPoint) -> Option<HitTarget<'a>> {
        for i in frame.pick_order() {
            let surface = &frame.surfaces[i];
            match surface.capture {
                CapturePolicy::Block => {
                    if let Some(rect) = surface.top_hit_at(p) {
                        return Some(HitTarget::Element { surface, rect });
                    }
                    return Some(HitTarget::Backdrop { surface });
                }
                CapturePolicy::Capture => {
                    if let Some(rect) = surface.top_hit_at(p) {
                        return Some(HitTarget::Element { surface, rect });
                    }
                }
                CapturePolicy::PassThrough | CapturePolicy::Passive => {}
            }
        }
        None
    }

    /// Клик поглощён экранной поверхностью (не доходит до канваса)?
    /// Pick-матрица G2: Block/Capture — да (в своих rect'ах/везде),
    /// PassThrough/Passive — нет.
    pub fn absorbs(frame: &UiFrame, p: UiPoint) -> bool {
        Self::pick(frame, p).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::HitRect;
    use crate::geometry::UiRect;
    use crate::layer::UiLayer;

    fn vp() -> UiRect {
        UiRect::new(0.0, 0.0, 1280.0, 800.0)
    }

    fn bar(y: f32) -> UiRect {
        UiRect::new(0.0, y, 1280.0, 40.0)
    }

    /// ПИК-МАТРИЦА G2 (PRD-0009 US-1 AC-1.3): все 4 политики × (попадание /
    /// мимо hit-rect'ов) — точный владелец клика.
    #[test]
    fn pick_matrix_all_policies() {
        let mut frame = UiFrame::new(vp());
        // панели: Capture с rect'ом
        frame.surfaces.push(
            SurfaceFrame::new("whatif", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(bar(760.0), "whatif-bar")),
        );
        // пустой PassThrough
        frame.surfaces.push(SurfaceFrame::new(
            "hint",
            UiLayer::Popups,
            CapturePolicy::PassThrough,
            vp(),
        ));
        // модаль: Block с rect'ом кнопки
        frame.surfaces.push(
            SurfaceFrame::new("gallery", UiLayer::Modals, CapturePolicy::Block, vp())
                .with_hit_rect(HitRect::interactive(
                    UiRect::new(600.0, 400.0, 80.0, 32.0),
                    "gallery-apply",
                )),
        );

        // 1) Block + попадание в rect → элемент модали
        let hit = HitStack::pick(&frame, UiPoint::new(620.0, 410.0));
        match hit {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), "gallery");
                assert_eq!(rect.element, "gallery-apply");
            }
            other => panic!("ожидался Element модали, получено {other:?}"),
        }

        // 2) Block + мимо rect'ов → backdrop модали (глотает всё под собой)
        let hit = HitStack::pick(&frame, UiPoint::new(100.0, 100.0));
        match hit {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), "gallery");
            }
            other => panic!("ожидался Backdrop модали, получено {other:?}"),
        }

        // 3) без модали: Capture + попадание → элемент панели
        let mut frame2 = UiFrame::new(vp());
        frame2.surfaces.push(
            SurfaceFrame::new("whatif", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(bar(760.0), "whatif-bar")),
        );
        let hit = HitStack::pick(&frame2, UiPoint::new(640.0, 770.0));
        match hit {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), "whatif");
                assert_eq!(rect.element, "whatif-bar");
            }
            other => panic!("ожидался Element панели, получено {other:?}"),
        }

        // 4) Capture + мимо rect'ов; PassThrough/Passive пропускают → None
        //    (клик уходит канвасу)
        assert!(HitStack::pick(&frame2, UiPoint::new(640.0, 300.0)).is_none());
        assert!(!HitStack::absorbs(&frame2, UiPoint::new(640.0, 300.0)));
    }

    #[test]
    fn block_backdrop_hides_everything_below_even_at_point_of_lower_surface() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(
            SurfaceFrame::new("whatif", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(bar(760.0), "whatif-bar")),
        );
        frame.surfaces.push(SurfaceFrame::new(
            "onboarding",
            UiLayer::Modals,
            CapturePolicy::Block,
            vp(),
        ));
        // клик прямо по whatif-бару — но модаль Block выше: бар не получает
        let hit = HitStack::pick(&frame, UiPoint::new(640.0, 770.0));
        match hit {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), "onboarding");
            }
            other => panic!("модаль обязана заглотить клик, получено {other:?}"),
        }
    }

    #[test]
    fn capture_misses_pass_through_to_lower_surface() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(
            SurfaceFrame::new("whatif", UiLayer::Panels, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(bar(760.0), "whatif-bar")),
        );
        // dropdown (Popups, Capture) с rect'ом над баром, клик мимо него
        frame.surfaces.push(
            SurfaceFrame::new("dropdown", UiLayer::Popups, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(UiRect::new(100.0, 700.0, 200.0, 50.0), "dd-list")),
        );
        // клик в бар: dropdown не перехватывает (мимо), бар получает
        let hit = HitStack::pick(&frame, UiPoint::new(640.0, 770.0));
        match hit {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), "whatif");
                assert_eq!(rect.element, "whatif-bar");
            }
            other => panic!("бар должен получить клик сквозь промах dropdown, получено {other:?}"),
        }
        // клик в dropdown: перехватывается им, бар ниже недоступен
        let hit = HitStack::pick(&frame, UiPoint::new(150.0, 720.0));
        match hit {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), "dropdown");
                assert_eq!(rect.element, "dd-list");
            }
            other => panic!("dropdown должен перехватить, получено {other:?}"),
        }
    }

    #[test]
    fn topmost_registration_wins_within_surface() {
        let mut frame = UiFrame::new(vp());
        let r1 = UiRect::new(0.0, 0.0, 200.0, 200.0);
        let r2 = UiRect::new(100.0, 0.0, 200.0, 200.0);
        frame.surfaces.push(
            SurfaceFrame::new("wheel", UiLayer::WorldOverlay, CapturePolicy::Capture, vp())
                .with_hit_rect(HitRect::interactive(r1, "sector-a"))
                .with_hit_rect(HitRect::interactive(r2, "sector-b")),
        );
        let hit = HitStack::pick(&frame, UiPoint::new(150.0, 100.0));
        match hit {
            Some(HitTarget::Element { rect, .. }) => assert_eq!(rect.element, "sector-b"),
            other => panic!("верхний (последний) rect должен выиграть, получено {other:?}"),
        }
    }

    #[test]
    fn world_layer_pass_through_leaves_click_to_canvas() {
        let mut frame = UiFrame::new(vp());
        frame.surfaces.push(SurfaceFrame::new(
            "world",
            UiLayer::World,
            CapturePolicy::PassThrough,
            vp(),
        ));
        assert!(HitStack::pick(&frame, UiPoint::new(1.0, 1.0)).is_none());
        assert!(!HitStack::absorbs(&frame, UiPoint::new(1.0, 1.0)));
    }

    #[test]
    fn hidden_surface_is_not_pickable() {
        let mut reg = crate::registry::SurfaceRegistry::new();
        reg.add(
            crate::registry::SurfaceDecl::new("whatif", UiLayer::Panels, CapturePolicy::Capture)
                .with_degradation(crate::registry::DegradationPolicy::HideBelow {
                    min_width: 900.0,
                    min_height: 600.0,
                }),
        );
        let small = UiFrame::from_registry(&reg, UiRect::new(0.0, 0.0, 800.0, 560.0));
        // поверхность скрыта — в кадре нет, клик не перехватывается
        assert_eq!(small.surfaces.len(), 0);
        assert!(!HitStack::absorbs(&small, UiPoint::new(1.0, 1.0)));
    }
}
