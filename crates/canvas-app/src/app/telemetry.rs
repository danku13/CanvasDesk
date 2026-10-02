//! FR-090: продуктовые события из Rust → PostHog — DOM-мост
//! `canvasdesk:track` (CustomEvent на `document`; слушатель — модуль
//! `__cdTelemetry` в `crates/canvas-web/index.html`, FR-089).
//!
//! Контракт (index.html, блок «Продуктовые события»): detail
//! `{event: String, properties: Object}` → `__cdTelemetry.track`.
//! Без ключа/согласия analytics вызовы no-op на стороне JS — телеметрия
//! не грузится и не отправляется (двухуровневое согласие FR-089).
//! Натив (десктоп): коллектора нет — те же вызовы безопасны и ничего
//! не делают (поля согласий персистятся, FR-089 §2).
//!
//! [`surface_diff`] — событие `surface_opened {surface}` по дифу реестра
//! поверхностей: новая в реестре поверхность = «открыта». Ambient-хром
//! (world/corner_buttons/template_strip/empty/minimap) открытием не
//! считается; what-if — только реальная активация сессии
//! (`scene.whatif_active`; пилюля входа видна на больших вьюпортах).

#[cfg(any(target_arch = "wasm32", test))]
use super::ui_registry;
use super::App;
use canvas_ui::registry::SurfaceRegistry;
#[cfg(any(target_arch = "wasm32", test))]
use std::collections::HashSet;

/// Ambient-хром сцены: активен без действия пользователя — «открытием»
/// не считается, событие `surface_opened` по нему не шлётся.
/// Продакшн-потребитель — только wasm-срез `surface_diff_web`; на нативе
/// без `cfg(test)` было бы dead_code (нативный clippy -D warnings,
/// красит CI) — потому гейт как у потребителя.
#[cfg(any(target_arch = "wasm32", test))]
const AMBIENT_SURFACES: &[&str] = &[
    ui_registry::id::WORLD,
    ui_registry::id::CORNER_BUTTONS,
    ui_registry::id::TEMPLATE_STRIP,
    ui_registry::id::EMPTY,
    ui_registry::id::MINIMAP,
];

/// Отправить продуктовое событие (web — мост в `__cdTelemetry.track`,
/// натив — no-op: отправки нет, FR-089 §2). Свойства — плоские строки:
/// `track("role_changed", &[("role", "architect")])`.
pub fn track(event: &str, properties: &[(&str, &str)]) {
    #[cfg(target_arch = "wasm32")]
    {
        track_web(event, properties);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (event, properties);
    }
}

/// web-реализация [`track`]: CustomEvent `canvasdesk:track` на `document`
/// (контракт слушателя в index.html — detail `{event, properties}`).
#[cfg(target_arch = "wasm32")]
fn track_web(event: &str, properties: &[(&str, &str)]) {
    use wasm_bindgen::JsValue;
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(document) = window.document() else {
        return;
    };
    // detail {event, properties} — контракт слушателя index.html
    // (document.addEventListener("canvasdesk:track", …)).
    let detail = js_sys::Object::new();
    let event_key = JsValue::from_str("event");
    if js_sys::Reflect::set(&detail, &event_key, &JsValue::from_str(event)).is_err() {
        return;
    }
    let props = js_sys::Object::new();
    for &(key, value) in properties {
        let prop_key = JsValue::from_str(key);
        let prop_value = JsValue::from_str(value);
        let _ = js_sys::Reflect::set(&props, &prop_key, &prop_value);
    }
    let props_key = JsValue::from_str("properties");
    if js_sys::Reflect::set(&detail, &props_key, &props).is_err() {
        return;
    }
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&detail);
    let bridge = "canvasdesk:track";
    if let Ok(custom) = web_sys::CustomEvent::new_with_event_init_dict(bridge, &init) {
        // Телеметрия не должна ронять приложение: неуспех тихо проглатываем.
        let _ = document.dispatch_event(&custom);
    }
}

/// FR-090: `surface_opened {surface}` — диф активных поверхностей
/// реестра. Вызывается из `ui_registry::build_frame_at` при пересборке
/// кадра — ровно тогда меняется сигнатура поверхностей
/// (`build_frame_sig`: битовые флаги открытых поверхностей), т.е. дифф
/// срабатывает в момент открытия/закрытия, а не каждый кадр.
/// Натив — no-op.
pub fn surface_diff(app: &App, registry: &SurfaceRegistry) {
    #[cfg(target_arch = "wasm32")]
    {
        surface_diff_web(app, registry);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (app, registry);
    }
}

/// Срез «какие поверхности сейчас открыты пользователем»: реестр без
/// ambient-хрома; what-if — только активная сессия (в реестре поверхность
/// живёт, пока видна пилюля входа на вьюпортах ≥900×600 — почти всегда
/// на десктопе). Чистая функция — инварианты покрыты тестами на нативе.
/// Гейт `wasm32 | test` — см. комментарий к `AMBIENT_SURFACES`.
#[cfg(any(target_arch = "wasm32", test))]
fn tracked_present(app: &App, registry: &SurfaceRegistry) -> HashSet<String> {
    let mut present: HashSet<String> = registry
        .declarations()
        .iter()
        .map(|decl| decl.id.as_str().to_owned())
        .filter(|sid| !AMBIENT_SURFACES.contains(&sid.as_str()))
        .collect();
    if app.scene.whatif_active {
        present.insert(ui_registry::id::WHATIF.to_owned());
    } else {
        present.remove(ui_registry::id::WHATIF);
    }
    present
}

/// web-реализация [`surface_diff`]: предыдущий срез живёт между кадрами
/// в thread-local (wasm — один поток; поле в `App` не заводим — реестр
/// и так снимок иммутабельного состояния, дифф — побочный эффект кадра).
#[cfg(target_arch = "wasm32")]
fn surface_diff_web(app: &App, registry: &SurfaceRegistry) {
    use std::cell::RefCell;
    let present = tracked_present(app, registry);
    thread_local! {
        static PREV: RefCell<Option<HashSet<String>>> = RefCell::new(None);
    }
    PREV.with(|prev_cell| {
        let mut prev = prev_cell.borrow_mut();
        if prev.as_ref() == Some(&present) {
            return;
        }
        for sid in &present {
            let opened = match prev.as_ref() {
                Some(before) => !before.contains(sid),
                None => true,
            };
            if opened {
                track("surface_opened", &[("surface", sid.as_str())]);
            }
        }
        *prev = Some(present);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Минимальный App без окна (паттерн test_stub из ui_registry.rs).
    fn test_stub() -> App {
        let scene = crate::app::SceneState::new(
            crate::Canvas::default(),
            std::path::PathBuf::from("target/tmp/fr090-telemetry.canvas"),
        );
        let (search_responder, _rx) = {
            let (tx, rx) = std::sync::mpsc::channel();
            let responder: canvas_core::SearchResponder = std::sync::Arc::new(move |event| {
                let _ = tx.send(event);
            });
            (responder, rx)
        };
        App::new(
            scene,
            Box::new(canvas_core::NoopThumbs),
            crate::Settings::default(),
            None,
            Some(std::env::temp_dir().join(format!("canvasdesk-fr090-{}", std::process::id()))),
            std::sync::Arc::new(|_event: canvas_core::DragEvent| {}),
            std::sync::Arc::new(|_event: canvas_widgets::WidgetEvent| {}),
            Box::new(canvas_core::NoopWatch),
            Box::new(canvas_core::MemSearch::new(search_responder)),
            Box::new(canvas_core::NoopClipboard),
            Some(Box::new(canvas_core::MemWidgetState::default())),
            false,
            Box::new(canvas_render::renderer_init::NoopRendererLaunch),
        )
    }

    /// Ambient-хром не считается открытием: world/corner_buttons/
    /// template_strip/empty/minimap отфильтрованы из среза (empty на
    /// пустом канвасе и свёрнутая полоса присутствуют в реестре).
    #[test]
    fn ambient_surfaces_are_not_tracked() {
        let app = test_stub();
        let registry = ui_registry::build_registry(&app);
        let present = tracked_present(&app, &registry);
        for ambient in AMBIENT_SURFACES {
            assert!(!present.contains(*ambient), "ambient {ambient} в срезе");
        }
    }

    /// Поверхность, открытая действием пользователя, попадает в срез.
    #[test]
    fn user_opened_surface_is_tracked() {
        let mut app = test_stub();
        app.settings_open = true;
        let registry = ui_registry::build_registry(&app);
        let present = tracked_present(&app, &registry);
        assert!(present.contains(ui_registry::id::SETTINGS));
    }

    /// What-if: пилюля входа (вьюпорты ≥900×600) не считается открытием —
    /// только активная сессия `scene.whatif_active`.
    #[test]
    fn whatif_tracked_only_when_session_active() {
        let mut app = test_stub();
        let registry = ui_registry::build_registry(&app);
        assert!(!tracked_present(&app, &registry).contains(ui_registry::id::WHATIF));
        app.scene.whatif_active = true;
        let registry = ui_registry::build_registry(&app);
        assert!(tracked_present(&app, &registry).contains(ui_registry::id::WHATIF));
    }
}
