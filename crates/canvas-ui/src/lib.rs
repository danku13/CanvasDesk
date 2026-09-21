//! canvas-ui — модель экрана: слои, реестр поверхностей, capture-политики,
//! HitStack и KeyboardRouter (PRD-0009, реализация FR-051, этап U1).
//!
//! Симметрия крейтов: `canvas-core` — модель мира (данные канваса),
//! `canvas-ui` — модель экрана (кто кого перекрывает, кто получит ввод).
//! Чистая геометрия без GPU/ОС — вся логика headless-тестируема
//! (PRD-0009 §9.1); интеграция в `canvas-app`/`canvas-render` — U2+
//! (этот крейт на U1 ничего не рендерит и ввод не перехватывает).
//!
//! # Как добавить поверхность (3 шага, полный гайд — U5 `docs/ui-kit.md`)
//!
//! 1. **Декларация** — один вызов [`registry::SurfaceRegistry::add`]:
//!    id, слой [`layer::UiLayer`], capture-политика [`capture::CapturePolicy`],
//!    keyboard-scope, политика деградации [`registry::DegradationPolicy`].
//! 2. **Контент кадра** — адаптер поверхности за кадр заполняет
//!    [`frame::SurfaceFrame`]: hit-rect'ы (с именами элементов) и клип.
//! 3. **Ввод и отрисовка — автоматически**: клики раздаёт [`hit::HitStack`],
//!    клавиатура — [`keyboard::KeyboardRouter`], draw-порядок выводится из
//!    реестра ([`frame::UiFrame::draw_bands`]); renderer рисует в полосе
//!    своего слоя (U2+).
//!
//! # Карты переносов
//!
//! egui `layers.rs`/`hit_test.rs`/`modal.rs` — адаптация алгоритмов
//! (FR-051, Приложение А); iced `overlay.rs`/Catalog и Ribir
//! `IgnorePointer` — паттерны. Таксономия, не зависимости: крейт не имеет
//! внешних зависимостей (G7).

pub mod capture;
pub mod frame;
pub mod geometry;
pub mod hit;
pub mod keyboard;
pub mod layer;
pub mod registry;

pub use capture::CapturePolicy;
pub use frame::{HitRect, Overlap, SurfaceFrame, UiFrame};
pub use geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
pub use hit::{HitStack, HitTarget};
pub use keyboard::{Activation, KeyboardRouter};
pub use layer::UiLayer;
pub use registry::{
    DegradationPolicy, KeyboardScopeId, RegistryError, SurfaceDecl, SurfaceId, SurfaceRegistry,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Интеграционный сценарий каркаса (PRD-0009 US-1 AC-1.1/AC-1.2 на
    /// модельных поверхностях): реестр → кадр → pick → клавиатура. Полная
    /// интеграция — U2.
    #[test]
    fn skeleton_end_to_end_on_model_surfaces() {
        let mut reg = SurfaceRegistry::new();
        // канвас (дефолтный scope NUMI-хоткеев) — мир ниже всех
        reg.add(
            SurfaceDecl::new("world", UiLayer::World, CapturePolicy::PassThrough)
                .with_scope(KeyboardScopeId::CANVAS),
        );
        // what-if бар (Capture, hide на малом окне)
        reg.add(
            SurfaceDecl::new("whatif", UiLayer::Panels, CapturePolicy::Capture)
                .with_degradation(DegradationPolicy::HideBelow {
                    min_width: 900.0,
                    min_height: 600.0,
                }),
        );
        // галерея схем (модаль Block)
        reg.add(SurfaceDecl::new("gallery", UiLayer::Modals, CapturePolicy::Block).with_scope("gallery"));
        // тост (Passive)
        reg.add(SurfaceDecl::new("toast", UiLayer::Toasts, CapturePolicy::Passive));

        let vp = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let mut frame = UiFrame::from_registry(&reg, vp);
        assert_eq!(frame.surfaces.len(), 4);

        // адаптеры заполняют hit-rect'ы
        for s in frame.surfaces.iter_mut() {
            if s.surface.as_str() == "whatif" {
                s.hit_rects.push(HitRect::interactive(
                    UiRect::new(0.0, 760.0, 1280.0, 40.0),
                    "whatif-bar",
                ));
            }
            if s.surface.as_str() == "gallery" {
                s.hit_rects.push(HitRect::interactive(
                    UiRect::new(600.0, 400.0, 80.0, 32.0),
                    "gallery-apply",
                ));
            }
        }

        // draw: 4 полосы по возрастанию (Panels → Modals → Toasts; World в кадре PassThrough — полоса тоже выводится)
        let bands = frame.draw_bands();
        let layers: Vec<UiLayer> = bands.iter().map(|(l, _)| *l).collect();
        assert_eq!(
            layers,
            vec![UiLayer::World, UiLayer::Panels, UiLayer::Modals, UiLayer::Toasts]
        );

        // ввод: клик по кнопке модали — модаль; клик по бару — backdrop модали;
        // без модали бар получил бы клик
        let click_apply = HitStack::pick(&frame, UiPoint::new(620.0, 410.0));
        assert!(matches!(
            click_apply,
            Some(HitTarget::Element { surface, .. }) if surface.surface.as_str() == "gallery"
        ));
        let click_bar = HitStack::pick(&frame, UiPoint::new(640.0, 770.0));
        assert!(matches!(
            click_bar,
            Some(HitTarget::Backdrop { surface }) if surface.surface.as_str() == "gallery"
        ));

        // клавиатура: Esc-цель — верх стека (галерея), канвас — дно
        let router = KeyboardRouter::from_registry(&reg);
        assert_eq!(router.esc_target().map(|a| a.surface.as_str()), Some("gallery"));
        assert_eq!(
            router.activations().first().map(|a| a.scope.as_str()),
            Some(KeyboardScopeId::CANVAS)
        );

        // пересечений интерактивных rect'ов одной полосы нет (линт F-11 a precursor)
        assert!(frame.overlaps_within_layer().is_empty());

        // малое окно: whatif скрыт hide-политикой, кадр короче
        let small = UiFrame::from_registry(&reg, UiRect::new(0.0, 0.0, 800.0, 560.0));
        assert_eq!(small.surfaces.len(), 3);
    }
}
