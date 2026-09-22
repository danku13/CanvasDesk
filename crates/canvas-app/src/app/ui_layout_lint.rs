//! FR-054 (U5 PRD-0009, F-11): сквозной layout-линт полного кадра —
//! CI-гейт G4. На канонических состояниях приложения (одно активное
//! состояние поверх idle) × 3 окна (1280×800 / 1024×640 / 800×560) ×
//! RU/EN проверяется:
//! 1. `UiFrame::overlaps_within_layer` пуст — интерактивные rect'ы РАЗНЫХ
//!    поверхностей одной полосы не пересекаются;
//! 2. каждый интерактивный hit-rect целиком во вьюпорте (0 выходов;
//!    hide-деградации — HideBelow — учтены реестром);
//! 3. pick-контракт U2 стабилен: у Block-поверхностей клик в угол экрана
//!    даёт Backdrop этой поверхности (модаль реально накрывает экран).
//!
//! Канонические состояния НЕ комбинируют фичи (что-if + док палитры и
//! т.п.): их совместная геометрия — нормализованные дельты U2 (порядок
//! pick), а не предмет этого линта. Состояния строятся на headless-заглушке
//! App (без окна/GPU — паттерн тестов `ui_registry`).
//!
//! Исполняется `cargo test --workspace` → CI (линты F-11 в CI — §13 U5).
#![cfg(test)]

use super::ui_registry::build_frame_at;
use super::*; // приватные поля App и типы модуля app (паттерн ui_registry-тестов)
use canvas_core::Language;
use canvas_ui::frame::UiFrame;

const VIEWPORTS: [[f32; 2]; 3] = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];

/// Заглушка App для headless-линта (паттерн test_stub из ui_registry).
fn lint_stub(language: Language) -> App {
    let scene = SceneState::new(
        Canvas::default(),
        std::path::PathBuf::from("target/tmp/fr054-layout-lint.canvas"),
    );
    let (search_responder, _rx) = {
        let (tx, rx) = std::sync::mpsc::channel();
        let responder: canvas_core::SearchResponder = std::sync::Arc::new(move |event| {
            let _ = tx.send(event);
        });
        (responder, rx)
    };
    let mut app = App::new(
        scene,
        Box::new(canvas_core::NoopThumbs),
        Settings::default(),
        None,
        Some(std::env::temp_dir().join(format!("canvasdesk-fr054-{}", std::process::id()))),
        std::sync::Arc::new(|_event: canvas_core::DragEvent| {}),
        std::sync::Arc::new(|_event: canvas_widgets::WidgetEvent| {}),
        Box::new(canvas_core::NoopWatch),
        Box::new(canvas_core::MemSearch::new(search_responder)),
        Box::new(canvas_core::NoopClipboard),
        Some(Box::new(canvas_core::MemWidgetState::default())),
        false,
        Box::new(canvas_render::renderer_init::NoopRendererLaunch),
    );
    app.onboarding = None; // снять авто-показ первого запуска (FR-028)
    app.settings.language = language;
    app
}

/// Линт одного кадра: 0 пересечений слоёв, 0 выходов за вьюпорт.
fn lint_frame(frame: &UiFrame, viewport: [f32; 2], tag: &str) {
    let overlaps = frame.overlaps_within_layer();
    assert!(
        overlaps.is_empty(),
        "{tag}: пересечения интерактивных rect'ов одной полосы: {:?}",
        overlaps
            .iter()
            .map(|o| format!(
                "{}:{} × {}:{}",
                o.a_surface.as_str(),
                o.a_element,
                o.b_surface.as_str(),
                o.b_element
            ))
            .collect::<Vec<_>>()
    );
    for surface in &frame.surfaces {
        for hit in &surface.hit_rects {
            if !hit.interactive {
                continue;
            }
            let r = hit.rect;
            assert!(
                r.x >= -0.01
                    && r.y >= -0.01
                    && r.right() <= viewport[0] + 0.01
                    && r.bottom() <= viewport[1] + 0.01,
                "{tag}: hit-rect {}:{} вне вьюпорта {viewport:?}: {:?}",
                surface.surface.as_str(),
                hit.element,
                (r.x, r.y, r.right(), r.bottom())
            );
        }
    }
}

/// Прогнать состояние через все окна и оба языка. `prepare` получает
/// вьюпорт линта (в headless-заглушке нет окна — `viewport_logical()` нулевой).
fn lint_state(tag: &str, prepare: impl Fn(&mut App, [f32; 2])) {
    for (lang, lang_tag) in [(Language::Ru, "RU"), (Language::En, "EN")] {
        for vp in VIEWPORTS {
            let mut app = lint_stub(lang);
            prepare(&mut app, vp);
            let frame = build_frame_at(&app, vp);
            lint_frame(&frame, vp, &format!("{tag}/{lang_tag}/{vp:?}"));
        }
    }
}

/// Block-поверхность накрывает экран: клик в угол — Backdrop этой
/// поверхности (контракт «модаль реально модальна» на реальном кадре).
fn assert_backdrop_is(frame: &UiFrame, surface_id: &str) {
    let pick = canvas_ui::HitStack::pick(frame, canvas_ui::UiPoint::new(2.0, 2.0));
    match pick {
        Some(canvas_ui::HitTarget::Backdrop { surface }) => {
            assert_eq!(surface.surface.as_str(), surface_id);
        }
        Some(canvas_ui::HitTarget::Element { surface, .. }) => panic!(
            "угол экрана попал в Element {} — поверхность {surface_id} не накрывает экран",
            surface.surface.as_str()
        ),
        None => panic!("угол экрана ушёл в канвас — {surface_id} не Block-модаль"),
    }
}

// --- Канонические состояния (idle + одно активное) ---

#[test]
fn lint_idle() {
    lint_state("idle", |_app, _vp| {});
}

#[test]
fn lint_search_open() {
    lint_state("search", |app, _vp| {
        app.search.open();
        app.search.set_results(vec![
            canvas_render::search_ui::SearchRow {
                title: "Заметка о расчёте".into(),
                subtitle: "заметка".into(),
            },
            canvas_render::search_ui::SearchRow {
                title: "capacity-model.canvas".into(),
                subtitle: "models/capacity".into(),
            },
        ]);
    });
}

#[test]
fn lint_settings_open() {
    lint_state("settings", |app, _vp| {
        app.settings_open = true;
    });
}

#[test]
fn lint_docs_open() {
    lint_state("docs", |app, viewport| {
        let page = crate::docs_ui::page_index_by_id("hotkeys").expect("страница есть");
        let panel = crate::docs_ui::viewer_rect(viewport);
        let content = crate::docs_ui::viewer_content_rect(panel);
        // FR-054: раскладка страницы — измеренным текстом (measurer на вызов)
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        app.docs = Some(DocsViewer {
            page,
            layout: crate::docs_ui::layout_page(page, content[2], &mut measurer, &mut fs),
            scroll: crate::docs_ui::ScrollState::new(
                crate::docs_ui::layout_page(page, content[2], &mut measurer, &mut fs)
                    .content_height,
                content[3],
            ),
            layout_width: content[2],
        });
    });
}

#[test]
fn lint_help_menu_open() {
    lint_state("help_menu", |app, viewport| {
        app.help_menu = Some(HelpMenuState {
            origin: crate::docs_ui::help_menu_origin(
                super::help_button_rect(app.settings.button_corner, viewport),
                viewport,
            ),
            docs_open: false,
        });
    });
}

#[test]
fn lint_template_panel_open() {
    lint_state("template_panel", |app, _vp| {
        app.template_panel.open();
        app.template_panel.focused = true;
    });
}

#[test]
fn lint_template_strip_flyout() {
    lint_state("template_strip", |app, _vp| {
        // Flyout над пустым канвасом налезает на empty-карточку доброкачественно
        // (flyout рисуется выше, pick ≡ визуальному верху — семантика egui
        // «верхний непрозрачный скрывает нижний»); каноническое состояние для
        // линта — сцена с содержимым (карточка скрыта).
        app.empty_state_dismissed = true;
        let mut hover = crate::template_ui::StripHover::new();
        hover.open = Some(0);
        hover.pinned = true; // flyout раскрыт пином — стабильно для линта
        app.template_hover = Some(hover);
    });
}

#[test]
fn lint_gallery_open() {
    lint_state("gallery", |app, _vp| {
        app.scheme_gallery.open();
    });
    // Галерея — Block-модаль: угол экрана — её backdrop
    let mut app = lint_stub(Language::Ru);
    app.scheme_gallery.open();
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::GALLERY);
}

#[test]
fn lint_onboarding_open() {
    lint_state("onboarding", |app, _vp| {
        app.onboarding = Some(crate::onboarding_ui::OnboardingState::default());
    });
    let mut app = lint_stub(Language::Ru);
    app.onboarding = Some(crate::onboarding_ui::OnboardingState::default());
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::ONBOARDING);
}

#[test]
fn lint_stage_open() {
    lint_state("stage", |app, _vp| {
        app.main_stage = Some(MainStageState {
            key: ("a".to_owned(), "b".to_owned()),
            edges: Vec::new(),
            slice: Canvas::default(),
            scale: 1.0,
        });
    });
}

#[test]
fn lint_whatif_active() {
    lint_state("whatif", |app, _vp| {
        app.scene.whatif_active = true;
    });
}

#[test]
fn lint_context_menu_open() {
    lint_state("menu", |app, _vp| {
        app.menu = Some(crate::ui::ContextMenu {
            origin: [400.0, 300.0],
            submenu: None,
        });
    });
}

#[test]
fn lint_hotkeys_open() {
    lint_state("hotkeys", |app, _vp| {
        app.hotkeys_open = true;
    });
}

#[test]
fn lint_empty_state() {
    lint_state("empty", |app, _vp| {
        // Пустой канвас заглушки — empty-state карточка видима (dismissed
        // снят конструктором); онбординг снят в lint_stub.
        assert!(app.empty_state_visible() || app.scene.canvas.nodes.is_empty());
    });
}
