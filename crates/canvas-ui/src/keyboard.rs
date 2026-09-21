//! FR-051 U1 (PRD-0009 F-4, §7.3): `KeyboardRouter` — key-scope стек и
//! автоматический Esc-стек из реестра. Текущая лестница `on_key`
//! (app.rs:7901–8226) с U2/U5 выводится из реестра; NUMI-хоткеи канваса —
//! дефолтный scope и не затрагиваются.
//!
//! Референс семантики — egui `containers/modal.rs` (стек модалей + Esc +
//! any_popup_open; анализ 2026-09-22).

use crate::registry::{KeyboardScopeId, SurfaceId, SurfaceRegistry};

/// Активация поверхности с keyboard-scope.
#[derive(Debug, Clone, PartialEq)]
pub struct Activation {
    pub surface: SurfaceId,
    pub scope: KeyboardScopeId,
}

/// Роутер клавиатуры: стек активаций поверхностей. Верхний скоуп первым
/// получает событие; поглотившее — гасит доставку вниз (детерминированная
/// лестница вместо if-цепочки).
#[derive(Debug, Clone, Default)]
pub struct KeyboardRouter {
    stack: Vec<Activation>,
}

impl KeyboardRouter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Автоматический Esc-стек из реестра (F-4): все поверхности с
    /// keyboard-scope в порядке регистрации снизу вверх. Модальные `Block`
    /// без scope в стек скоупов не входят — их Esc обрабатывается по
    /// [`SurfaceRegistry::esc_stack`].
    pub fn from_registry(registry: &SurfaceRegistry) -> Self {
        let mut router = Self::new();
        for decl in registry.declarations() {
            if let Some(scope) = &decl.keyboard_scope {
                router.push(decl.id.clone(), scope.clone());
            }
        }
        router
    }

    /// Поверхность активировалась (открыта) — верх стека.
    pub fn push(&mut self, surface: SurfaceId, scope: KeyboardScopeId) {
        self.stack.push(Activation { surface, scope });
    }

    /// Поверхность закрыта — снимается со стека вместе со всем, что выше
    /// неё (закрытие нижней поверхности закрывает и «детей» над ней);
    /// возвращает снятые активации в порядке снизу вверх.
    pub fn pop_surface(&mut self, surface: &SurfaceId) -> Option<Vec<Activation>> {
        let pos = self.stack.iter().rposition(|a| &a.surface == surface)?;
        Some(self.stack.split_off(pos))
    }

    /// Снять верх стека.
    pub fn pop_top(&mut self) -> Option<Activation> {
        self.stack.pop()
    }

    /// Верх стека (первый получатель событий).
    pub fn top(&self) -> Option<&Activation> {
        self.stack.last()
    }

    /// Стек снизу вверх как срез активаций (верх — конец; для debug-оверлея).
    pub fn activations(&self) -> &[Activation] {
        &self.stack
    }

    /// Доставка события: верхний скоуп первым; `consume` возвращает true,
    /// если скоуп поглотил событие. Возвращает Some(поверхность) поглотителя.
    pub fn deliver(&self, mut consume: impl FnMut(&Activation) -> bool) -> Option<&Activation> {
        self.stack
            .iter()
            .rev()
            .find(|activation| consume(activation))
    }

    /// Автоматический Esc: цель — верх стека; при пустом стеке — None
    /// (Esc уходит канвасу/дефолтному scope).
    pub fn esc_target(&self) -> Option<&Activation> {
        self.top()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturePolicy;
    use crate::layer::UiLayer;
    use crate::registry::{SurfaceDecl, SurfaceRegistry};

    fn modal(id: &str) -> SurfaceDecl {
        SurfaceDecl::new(id, UiLayer::Modals, CapturePolicy::Block).with_scope(id)
    }

    fn seed_router() -> (SurfaceRegistry, KeyboardRouter) {
        let mut reg = SurfaceRegistry::new();
        reg.add(modal("gallery"));
        reg.add(modal("dialog"));
        let router = KeyboardRouter::from_registry(&reg);
        (reg, router)
    }

    #[test]
    fn router_seeds_from_registry_in_registration_order() {
        let (_reg, router) = seed_router();
        let tops: Vec<&str> = router
            .activations()
            .iter()
            .map(|a| a.surface.as_str())
            .collect();
        assert_eq!(tops, vec!["gallery", "dialog"]);
        assert_eq!(router.top().map(|a| a.surface.as_str()), Some("dialog"));
    }

    #[test]
    fn delivery_is_top_first_until_consumed() {
        let (_reg, router) = seed_router();
        let mut handled_by: Vec<String> = Vec::new();
        let absorbed = router.deliver(|a| {
            // нижний (gallery) не поглощает, верхний (dialog) поглощает
            if a.surface.as_str() == "dialog" {
                handled_by.push(a.surface.to_string());
                true
            } else {
                handled_by.push(a.surface.to_string());
                false
            }
        });
        assert_eq!(absorbed.map(|a| a.surface.as_str()), Some("dialog"));
        assert_eq!(handled_by, vec!["dialog"]);
    }

    #[test]
    fn unconsumed_event_falls_through_every_scope() {
        let (_reg, router) = seed_router();
        let mut visited = 0;
        let absorbed = router.deliver(|_a| {
            visited += 1;
            false
        });
        assert!(absorbed.is_none());
        assert_eq!(visited, 2);
    }

    #[test]
    fn pop_surface_removes_it_and_everything_above() {
        let mut reg = SurfaceRegistry::new();
        reg.add(modal("gallery"));
        reg.add(modal("dialog"));
        reg.add(
            SurfaceDecl::new("whatif", UiLayer::Panels, CapturePolicy::Capture)
                .with_scope("whatif"),
        );
        let mut router = KeyboardRouter::from_registry(&reg);
        // закрываем gallery — dialog и whatif (выше неё) снимаются вместе
        let popped = router.pop_surface(&SurfaceId::new("gallery"));
        assert!(popped.is_some());
        assert!(router.activations().is_empty());
        assert!(router.esc_target().is_none());
    }

    #[test]
    fn esc_target_is_top_of_stack() {
        let (_reg, mut router) = seed_router();
        assert_eq!(
            router.esc_target().map(|a| a.surface.as_str()),
            Some("dialog")
        );
        router.pop_top();
        assert_eq!(
            router.esc_target().map(|a| a.surface.as_str()),
            Some("gallery")
        );
        router.pop_top();
        assert!(router.esc_target().is_none());
    }

    #[test]
    fn canvas_hotkeys_survive_modal_close_via_registry_esc_stack() {
        // реестр: панель без scope, две модали со scope, тост пассивен
        let mut reg = SurfaceRegistry::new();
        reg.add(SurfaceDecl::new(
            "whatif",
            UiLayer::Panels,
            CapturePolicy::Capture,
        ));
        reg.add(modal("gallery"));
        reg.add(modal("dialog"));
        reg.add(SurfaceDecl::new(
            "toast",
            UiLayer::Toasts,
            CapturePolicy::Passive,
        ));
        // Esc-стек из реестра: реверс-порядок активных
        let stack = reg.esc_stack();
        let names: Vec<&str> = stack.iter().map(|id| id.as_str()).collect();
        assert_eq!(names, vec!["dialog", "gallery"]);
        // клавиатура при открытой модали не доходит до канваса: pick-матрица
        // (hit.rs) глотает клики Block'ом, а скоуп-доставка кончается на
        // верхнем поглотителе — канвас получает события только после
        // закрытия всех модалей (стек пуст)
        let mut router = KeyboardRouter::from_registry(&reg);
        assert!(router.esc_target().is_some());
        let _ = router.pop_surface(&SurfaceId::new("dialog"));
        let _ = router.pop_surface(&SurfaceId::new("gallery"));
        assert!(router.esc_target().is_none());
    }
}
