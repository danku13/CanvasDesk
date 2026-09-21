//! FR-051 U1 (PRD-0009 F-1, §7.3): реестр поверхностей — единственное место
//! добавления экранной поверхности. Каждая поверхность объявлена записью
//! (id, слой, capture-политика, keyboard-scope, политика деградации).
//!
//! Миграция (PRD-0009 §7.3): с U2 реестр — единственный диспетчер ввода и
//! источника draw-порядка; добавление поверхности = 1 вызов `add` + 1 функция
//! контента (G1).

use crate::capture::CapturePolicy;
use crate::layer::UiLayer;

/// Идентификатор поверхности (уникален в реестре; контракт единственности
/// enforced `SurfaceRegistry::add`). Стабилен в рамках сессии — годится для
/// подписей debug-оверлея (F-10).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SurfaceId(String);

impl SurfaceId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Именованный keyboard-scope поверхности (F-4). Одна поверхность — один
/// scope; NUMI-хоткеи канваса — дефолтный scope (`"canvas"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyboardScopeId(String);

impl KeyboardScopeId {
    pub const CANVAS: &'static str = "canvas";

    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KeyboardScopeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Политика деградации на малом окне (PRD-0009 F-11 c: «hide-политики вместо
/// вырожденных rect'ов»; вьюпорты G4: 1280×800, 1024×640, 800×560).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DegradationPolicy {
    /// Рисуется всегда.
    Always,
    /// Скрывается, если вьюпорт меньше минимума по любой из осей.
    HideBelow { min_width: f32, min_height: f32 },
}

impl DegradationPolicy {
    pub fn hidden_at(self, viewport_w: f32, viewport_h: f32) -> bool {
        match self {
            DegradationPolicy::Always => false,
            DegradationPolicy::HideBelow {
                min_width,
                min_height,
            } => viewport_w < min_width || viewport_h < min_height,
        }
    }
}

/// Декларация поверхности в реестре (PRD-0009 F-1).
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceDecl {
    pub id: SurfaceId,
    pub layer: UiLayer,
    pub capture: CapturePolicy,
    pub keyboard_scope: Option<KeyboardScopeId>,
    pub degradation: DegradationPolicy,
}

impl SurfaceDecl {
    pub fn new(id: impl Into<String>, layer: UiLayer, capture: CapturePolicy) -> Self {
        Self {
            id: SurfaceId::new(id),
            layer,
            capture,
            keyboard_scope: None,
            degradation: DegradationPolicy::Always,
        }
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.keyboard_scope = Some(KeyboardScopeId::new(scope));
        self
    }

    pub fn with_degradation(mut self, d: DegradationPolicy) -> Self {
        self.degradation = d;
        self
    }
}

/// Ошибка регистрации (дубликат id).
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryError(pub SurfaceId);

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "поверхность «{}» уже зарегистрирована", self.0)
    }
}

impl std::error::Error for RegistryError {}

/// Реестр поверхностей. Порядок регистрации — порядок внутри полосы
/// (draw: стабильная сортировка по [`UiLayer`]; hit: реверс). Дубликат id —
/// контрактная ошибка (паника в `add`, `Result` в `try_add`).
#[derive(Debug, Clone, Default)]
pub struct SurfaceRegistry {
    decls: Vec<SurfaceDecl>,
}

impl SurfaceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Регистрация поверхности; паника при дубликате id (контракт
    /// единственности — реестр единственное место добавления поверхности).
    pub fn add(&mut self, decl: SurfaceDecl) {
        if let Err(e) = self.try_add(decl) {
            panic!("SurfaceRegistry: {e}");
        }
    }

    pub fn try_add(&mut self, decl: SurfaceDecl) -> Result<(), RegistryError> {
        if self.decls.iter().any(|d| d.id == decl.id) {
            return Err(RegistryError(decl.id));
        }
        self.decls.push(decl);
        Ok(())
    }

    /// Декларации в порядке регистрации.
    pub fn declarations(&self) -> &[SurfaceDecl] {
        &self.decls
    }

    /// Поверхности полосы в порядке регистрации.
    pub fn surfaces_in_layer(&self, layer: UiLayer) -> impl Iterator<Item = &SurfaceDecl> {
        self.decls.iter().filter(move |d| d.layer == layer)
    }

    /// Draw-полосы: группировка по слоям по возрастанию, внутри — порядок
    /// регистрации (стабильная сортировка; F-2).
    pub fn draw_bands(&self) -> Vec<(UiLayer, Vec<&SurfaceDecl>)> {
        let mut bands = Vec::new();
        for layer in UiLayer::DRAW_ORDER {
            let surfaces: Vec<&SurfaceDecl> = self.surfaces_in_layer(layer).collect();
            if !surfaces.is_empty() {
                bands.push((layer, surfaces));
            }
        }
        bands
    }

    /// Автоматический Esc-стек из реестра (F-4): реверс-порядок регистрации,
    /// фильтр — модальные (`Block`) либо объявившие keyboard-scope.
    /// Верх стека — последняя зарегистрированная активная поверхность.
    pub fn esc_stack(&self) -> Vec<SurfaceId> {
        self.decls
            .iter()
            .rev()
            .filter(|d| d.capture == CapturePolicy::Block || d.keyboard_scope.is_some())
            .map(|d| d.id.clone())
            .collect()
    }

    /// Видимые при данном вьюпорте поверхности (hide-политика деградации;
    /// F-11 c precursor).
    pub fn visible_at(&self, viewport_w: f32, viewport_h: f32) -> impl Iterator<Item = &SurfaceDecl> {
        self.decls
            .iter()
            .filter(move |d| !d.degradation.hidden_at(viewport_w, viewport_h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modal(id: &str) -> SurfaceDecl {
        SurfaceDecl::new(id, UiLayer::Modals, CapturePolicy::Block).with_scope(id)
    }

    fn panel(id: &str) -> SurfaceDecl {
        SurfaceDecl::new(id, UiLayer::Panels, CapturePolicy::Capture)
    }

    #[test]
    fn duplicate_id_panics_in_add_and_errs_in_try_add() {
        let mut reg = SurfaceRegistry::new();
        reg.add(panel("whatif"));
        assert_eq!(
            reg.try_add(panel("whatif")),
            Err(RegistryError(SurfaceId::new("whatif")))
        );
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut reg = SurfaceRegistry::new();
            reg.add(modal("onboarding"));
            reg.add(modal("onboarding"));
        }));
        assert!(result.is_err(), "дубликат id в add должен паниковать");
    }

    #[test]
    fn draw_bands_group_by_layer_ascending() {
        let mut reg = SurfaceRegistry::new();
        reg.add(panel("whatif"));
        reg.add(modal("gallery"));
        reg.add(SurfaceDecl::new("toast", UiLayer::Toasts, CapturePolicy::Passive));
        reg.add(panel("search"));
        let bands = reg.draw_bands();
        let layers: Vec<UiLayer> = bands.iter().map(|(l, _)| *l).collect();
        assert_eq!(layers, vec![UiLayer::Panels, UiLayer::Modals, UiLayer::Toasts]);
        // порядок регистрации внутри полосы сохранён (стабильность)
        let panels: Vec<&str> = bands[0]
            .1
            .iter()
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(panels, vec!["whatif", "search"]);
    }

    #[test]
    fn esc_stack_is_reverse_registration_of_active_surfaces() {
        let mut reg = SurfaceRegistry::new();
        reg.add(panel("whatif")); // Capture без scope — в стек не попадает
        reg.add(modal("gallery"));
        reg.add(modal("dialog"));
        reg.add(SurfaceDecl::new("toast", UiLayer::Toasts, CapturePolicy::Passive));
        let stack = reg.esc_stack();
        let names: Vec<&str> = stack.iter().map(|id| id.as_str()).collect();
        assert_eq!(names, vec!["dialog", "gallery"]);
    }

    #[test]
    fn hide_below_degradation_filters_by_viewport() {
        let mut reg = SurfaceRegistry::new();
        reg.add(
            panel("whatif")
                .with_degradation(DegradationPolicy::HideBelow {
                    min_width: 900.0,
                    min_height: 600.0,
                }),
        );
        reg.add(panel("search"));
        // 1280×800 (G4) — обе видимы
        assert_eq!(reg.visible_at(1280.0, 800.0).count(), 2);
        // 800×560 (G4) — whatif скрыт hide-политикой
        let names: Vec<&str> = reg
            .visible_at(800.0, 560.0)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(names, vec!["search"]);
    }

    #[test]
    fn surfaces_in_layer_keeps_registration_order() {
        let mut reg = SurfaceRegistry::new();
        reg.add(panel("a"));
        reg.add(modal("m"));
        reg.add(panel("b"));
        let names: Vec<&str> = reg
            .surfaces_in_layer(UiLayer::Panels)
            .map(|d| d.id.as_str())
            .collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
