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
//! **Ограничение G4 (LAY-W17, решение владельца): coarse-rect'ы — вне скоупа.**
//! Реестр публикует hit-rect'ы из чистых layout-функций в ЛОГИЧЕСКИХ px
//! без coarse-расширения — те же зоны, что у точного указателя. Тач-44
//! (LAY8.3/FR-097: центрированное расширение до 44 лог. px на coarse,
//! `touch_targets::expand_xywh`) и резолюция coarse-пересечений
//! («ближайший центр», W15) — отдельная система ввода со своим гейтом;
//! включать их в кадр G4 значило бы дублировать состояние
//! `pointer_coarse()` в реестре и линтовать зоны, которых на десктопе
//! нет. Пересечения coarse-расширенных зон соседних целей (палитра-
//! строки, ⏸/⚙ — P2-1 ревью) отслеживает волна W15/touch_targets, этот
//! линт их не видит и не обязан.
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
            crate::search_ui::SearchRow {
                title: "Заметка о расчёте".into(),
                subtitle: "заметка".into(),
                kind: crate::search_ui::SearchRowKind::Node,
            },
            crate::search_ui::SearchRow {
                title: "capacity-model.canvas".into(),
                subtitle: "models/capacity".into(),
                kind: crate::search_ui::SearchRowKind::Node,
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
            scroll: {
                let mut scroll = crate::docs_ui::ScrollState::default();
                crate::docs_ui::sync_scroll(
                    &mut scroll,
                    crate::docs_ui::layout_page(page, content[2], &mut measurer, &mut fs)
                        .content_height,
                    content[3],
                );
                scroll
            },
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

/// FR-055 (этап U4): витрина кита — каноническое состояние G4-линта
/// («кит покрыт линтом» — результат этапа U4 по §13 PRD-0009): интерактивные
/// зоны шапки (кнопка темы + «✕») во вьюпортах и на обоих языках.
#[test]
fn lint_kit_gallery_open() {
    lint_state("kit_gallery", |app, _vp| {
        app.kit_gallery_open = true;
    });
    // Витрина — Block-модаль: угол экрана — её backdrop
    let mut app = lint_stub(Language::Ru);
    app.kit_gallery_open = true;
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::KIT_GALLERY);
}

/// FR-070 (этап 1): админпанель — каноническое состояние G4-линта:
/// шапка + сайдбар во вьюпортах и на обоих языках; Block-модаль.
#[test]
fn lint_admin_open() {
    lint_state("admin_panel", |app, _vp| {
        app.admin_open = true;
    });
    // Block-модаль: угол экрана — её backdrop
    let mut app = lint_stub(Language::Ru);
    app.admin_open = true;
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::ADMIN);
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

// --- LAY-W17 (ревью §3.4/§4): дозакрытие слепых зон G4 ---------------------
//
// Дешёвые канон-состояния поверхностей, чьи hit-rect'ы уже были в
// fill_hit_rects, но канон-состояния не было (wheel/choice_menu/dialog) +
// новая регистрация ai_onboarding (была вне реестра целиком).

/// LAY-W17: wheel-меню шаблонов — донат-меню (WorldOverlay/Block, extent+12
/// вокруг `menu.screen`). Каноническое состояние: центр вьюпорта, категория
/// не выбрана (только кольца категорий — геометрия wheel_geometry с клампом
/// центра в окно).
#[test]
fn lint_wheel_open() {
    lint_state("wheel", |app, vp| {
        app.wheel_menu = Some(crate::template_ui::WheelMenu {
            screen: [vp[0] / 2.0, vp[1] / 2.0],
            world: [0.0, 0.0],
            category: None,
        });
    });
    // Не вакуумно: hit-rect доната в кадре (extent категорий реестра + 12).
    let mut app = lint_stub(Language::Ru);
    app.wheel_menu = Some(crate::template_ui::WheelMenu {
        screen: [640.0, 400.0],
        world: [0.0, 0.0],
        category: None,
    });
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::WHEEL)
        .expect("wheel в кадре");
    assert!(
        surface.hit_rects.iter().any(|h| h.element == "wheel-donut"),
        "hit-rect доната wheel под линтом"
    );
}

/// LAY-W17: меню выбора (Popups/Block, transient над контекстным меню) —
/// hit-rect панели (пункты + заголовок) под линтом; backdrop-контракт.
#[test]
fn lint_choice_menu_open() {
    lint_state("choice_menu", |app, _vp| {
        app.choice_menu = Some(crate::app::ChoiceMenu {
            origin: [300.0, 200.0],
            title_key: crate::i18n::keys::MENU_PICK_PARAM_TITLE,
            items: vec![crate::app::ChoiceItem {
                label: "rps".to_owned(),
                action: crate::app::ChoiceAction::Param {
                    from_node: "a".to_owned(),
                    from_side: canvas_core::Side::Right,
                    from_line: None,
                    to_node: "b".to_owned(),
                    param: "rps".to_owned(),
                },
            }],
            hovered: None,
        });
    });
    let mut app = lint_stub(Language::Ru);
    app.choice_menu = Some(crate::app::ChoiceMenu {
        origin: [300.0, 200.0],
        title_key: crate::i18n::keys::MENU_PICK_PARAM_TITLE,
        items: vec![crate::app::ChoiceItem {
            label: "rps".to_owned(),
            action: crate::app::ChoiceAction::Param {
                from_node: "a".to_owned(),
                from_side: canvas_core::Side::Right,
                from_line: None,
                to_node: "b".to_owned(),
                param: "rps".to_owned(),
            },
        }],
        hovered: None,
    });
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::CHOICE_MENU)
        .expect("choice_menu в кадре");
    assert!(
        surface.hit_rects.iter().any(|h| h.element == "choice-menu"),
        "hit-rect панели выбора под линтом"
    );
    assert_backdrop_is(&frame, ui_registry::id::CHOICE_MENU);
}

/// LAY-W17: модальный диалог Да/Нет (Modals/Block) — hit-rect окна диалога
/// (`app.dialog_rect`) под линтом на всех канонических вьюпортах/языках;
/// backdrop-контракт (угол экрана — Backdrop диалога). `test_viewport` —
/// `dialog_rect` читает `viewport_logical()` (единственный rect-источник
/// без явного вьюпорта; паттерн `lint_palette_selected`).
#[test]
fn lint_dialog_open() {
    lint_state("dialog", |app, vp| {
        app.test_viewport = Some(vp);
        app.dialog = Some(AppDialog::WhatIfReset {
            scenarios: 2,
            overrides: 3,
        });
    });
    let mut app = lint_stub(Language::Ru);
    app.test_viewport = Some([1280.0, 800.0]);
    app.dialog = Some(AppDialog::WhatIfReset {
        scenarios: 2,
        overrides: 3,
    });
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::DIALOG)
        .expect("dialog в кадре");
    assert!(
        surface.hit_rects.iter().any(|h| h.element == "dialog"),
        "hit-rect окна диалога под линтом"
    );
    assert_backdrop_is(&frame, ui_registry::id::DIALOG);
}

/// LAY-W17: AI-онбординг (FR-LLM-B, F-8) — каноническое состояние G4.
/// Поверхность зарегистрирована в реестре (Modals/Block/Always — ревью §3.4:
/// была главной слепой зоной); hit-rect'ы карточки и 3 радио-карточек +
/// кнопок из [`onboarding_ui::ai_onboarding_layout`] во всех вьюпортах/
/// языках; backdrop-контракт (Block: угол экрана — Backdrop, модаль не
/// закрывается кликом мимо — контракт тур-онбординга).
#[test]
fn lint_ai_onboarding_open() {
    lint_state("ai_onboarding", |app, _vp| {
        app.ai_onboarding = Some(crate::onboarding_ui::AiOnboardingState::default());
    });
    // Не вакуумно: карточка + 3 радио-карточки + 2 кнопки = 6 rect'ов.
    let mut app = lint_stub(Language::Ru);
    app.ai_onboarding = Some(crate::onboarding_ui::AiOnboardingState::default());
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::AI_ONBOARDING)
        .expect("ai_onboarding в кадре 1280×800");
    assert_eq!(
        surface.hit_rects.len(),
        6,
        "карточка + режимы ×3 + privacy + continue"
    );
    let mut app = lint_stub(Language::Ru);
    // Раскрытый блок privacy — карточка выше (AI_ONB_PRIV_H 180): геометрия
    // кнопок/режимов не меняется (блок добавляется ниже кнопок) — линт-пин
    // второй формы поверхности.
    app.ai_onboarding = Some(crate::onboarding_ui::AiOnboardingState {
        selected: Some(crate::onboarding_ui::AiOnboardingChoice::Cloud),
        privacy_open: true,
    });
    let frame = build_frame_at(&app, [800.0, 560.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::AI_ONBOARDING)
        .expect("ai_onboarding в кадре 800×560 (privacy_open)");
    assert!(surface
        .hit_rects
        .iter()
        .any(|h| h.element == "ai-onb-continue"));
    // Block-модаль: угол экрана — её backdrop.
    assert_backdrop_is(&frame, ui_registry::id::AI_ONBOARDING);
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
    lint_state("menu", |app, vp| {
        // W-a (дефект адаптива №1 из аудита ui-kit): линт видит РЕАЛЬНЫЙ
        // кламп меню. Прежний фикс-ориджин (400, 140) был слепой зоной —
        // любая регрессия клампа мимо него. Теперь курсор в правом-нижнем
        // углу, origin — через ту же чистую функцию `clamped_menu_origin`,
        // что открывает меню в input.rs; регрессия клампа = hit-rect
        // menu-base вне вьюпорта → assert в lint_frame падает. Подменю
        // «Виджеты ▸» раскрыто — флип подменю у правого края тоже под
        // линтом; пункт один — вертикаль заведомо влезает.
        let items = crate::ui::canvas_menu_visible_items_ext(false, false).len();
        let origin = crate::ui::clamped_menu_origin([vp[0] - 1.0, vp[1] - 1.0], items, vp);
        app.menu = Some(crate::ui::ContextMenu {
            origin,
            submenu: Some(crate::ui::Submenu {
                origin: crate::ui::submenu_origin_next_to(origin, vp),
                entries: vec![crate::ui::SubmenuEntry {
                    action: crate::ui::SubmenuAction::Insert("com.canvasdesk.clock".to_owned()),
                    label: "Clock".to_owned(),
                }],
            }),
        });
    });
}

#[test]
fn lint_hotkeys_open() {
    lint_state("hotkeys", |app, _vp| {
        app.hotkeys_open = true;
    });
}

// --- FR-060: канонические состояния перенесённых поверхностей волны 2 -----

/// Предложение автосвязи A→B для линт-состояний волны 2.
fn lint_autolink_proposal() -> canvas_core::AutolinkProposal {
    canvas_core::AutolinkProposal {
        from_node: "A".into(),
        from_line: 1,
        to_node: "B".into(),
        param: "rate".into(),
        percent: 100,
        unit_match: None,
    }
}

/// Канвас «исток → приёмник» для линт-состояний волны 2.
fn lint_ab_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("A", "Исток", 0.0, 0.0));
    canvas.nodes.push(Node::text("B", "Приёмник", 300.0, 0.0));
    canvas
}

/// FR-060: диалог ревью автосвязи (kit::modal + list_rows) — интерактивный
/// rect (диалог) во всех окнах/языках; модальность — backdrop-контракт.
#[test]
fn lint_autolink_review_open() {
    lint_state("autolink_review", |app, _vp| {
        app.autolink_review = Some(crate::autolink_ui::Review::build(
            &lint_ab_canvas(),
            vec![lint_autolink_proposal()],
        ));
    });
    let mut app = lint_stub(Language::Ru);
    app.autolink_review = Some(crate::autolink_ui::Review::build(
        &lint_ab_canvas(),
        vec![lint_autolink_proposal()],
    ));
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::AUTOLINK);
}

/// FR-060: окно проверки цепочки (kit::modal) — Loading (дерево из фоновой
/// сборки ещё не пришло); линт проверяет геометрию окна и модальность.
#[test]
fn lint_explain_open() {
    lint_state("explain", |app, _vp| {
        let (_tx, rx) = std::sync::mpsc::channel();
        app.explain = Some(crate::explain_ui::ExplainState::loading(
            canvas_core::LineageNodeId::total("A"),
            1,
            crate::explain_ui::ExplainBuild::Native(rx),
        ));
    });
    let mut app = lint_stub(Language::Ru);
    let (_tx, rx) = std::sync::mpsc::channel();
    app.explain = Some(crate::explain_ui::ExplainState::loading(
        canvas_core::LineageNodeId::total("A"),
        1,
        crate::explain_ui::ExplainBuild::Native(rx),
    ));
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert_backdrop_is(&frame, ui_registry::id::EXPLAIN);
}

/// FR-060: палитра выделения (kit::dropdown_menu) — бар + открытая колонка
/// во всех окнах/языках. Screen-якорь требует вьюпорта — в headless-заглушке
/// выставляется тестовый оверрайд `test_viewport` (тот же механизм, что у
/// клик-тестов screen-space UI); клампы палитры дополнительно покрыты
/// модельными тестами palette.rs (bar_size_and_origin и др.).
#[test]
fn lint_palette_selected() {
    lint_state("palette", |app, vp| {
        app.scene
            .canvas
            .nodes
            .push(Node::text("A", "Риск", 100.0, 100.0));
        app.selected = Some(Selection::Node(0));
        app.palette_hover.open = Some(0);
        app.test_viewport = Some(vp);
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

// --- LAY-W1 (аудит 2026-10 §3.8, P1 LAY8.2): панели AI в реестре ----------

/// Агент-панель (Ctrl+I) — каноническое состояние G4: интерактивные зоны
/// (✕ / поле ввода / send / quick-пилюли) во всех вьюпортах и на обоих
/// языках. HideBelow { 600, 240 } на канонических вьюпортах не срабатывает
/// (минимум 800×560 > 600×240) — геометрия панели под линтом полностью.
/// `test_viewport` — rect-функции панели читают вьюпорт линта (в
/// headless-заглушке `viewport_logical()` нулевой).
#[test]
fn lint_agent_panel_open() {
    lint_state("agent_panel", |app, vp| {
        app.agent_panel.open = true;
        app.test_viewport = Some(vp);
    });
    // Не вакуумно: на 800×560 панель в кадре с полным скелетом (6 rect'ов:
    // ✕ + input + send + 3 quick); ниже 600 — ЦЕЛИКОМ вне кадра.
    let mut app = lint_stub(Language::Ru);
    app.agent_panel.open = true;
    app.test_viewport = Some([800.0, 560.0]);
    let frame = build_frame_at(&app, [800.0, 560.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::AGENT_PANEL)
        .expect("агент-панель в кадре 800×560");
    assert_eq!(
        surface.hit_rects.len(),
        6,
        "скелет панели под линтом: ✕/input/send/quick×3"
    );
    let mut app = lint_stub(Language::Ru);
    app.agent_panel.open = true;
    app.test_viewport = Some([599.0, 560.0]);
    let frame = build_frame_at(&app, [599.0, 560.0]);
    assert!(frame
        .surfaces
        .iter()
        .all(|s| s.surface.as_str() != ui_registry::id::AGENT_PANEL));
}

/// AI-статус-панель — каноническое состояние G4: ⏸/⚙ + чипы
/// Suggest/Graph/Agent во вьюпортах ≥ 900×131. На 800×560 панель скрыта
/// HideBelow { 900, 131 } (derive из констант панели — стыковка LAY-W2) —
/// поверхности в кадре нет, 0 rect'ов (LAY8 п.3 «скрыта ЦЕЛИКОМ»); линт
/// проходит тривиально (как whatif на 800×560).
/// AI включён — дефолт `LlmSettings` all_off, панель не регистрируется.
#[test]
fn lint_ai_status_open() {
    lint_state("ai_status", |app, vp| {
        app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
        app.test_viewport = Some(vp);
    });
    // Не вакуумно: на 1024×640 — 5 rect'ов (⏸/⚙ + чипы ×3); на 800×560 —
    // HideBelow, поверхности в кадре нет (0 rect'ов).
    let mut app = lint_stub(Language::Ru);
    app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
    app.test_viewport = Some([1024.0, 640.0]);
    let frame = build_frame_at(&app, [1024.0, 640.0]);
    let surface = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::AI_STATUS)
        .expect("AI-статус в кадре 1024×640");
    assert_eq!(surface.hit_rects.len(), 5, "⏸/⚙ + чипы Suggest/Graph/Agent");
    let mut app = lint_stub(Language::Ru);
    app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
    app.test_viewport = Some([800.0, 560.0]);
    let frame = build_frame_at(&app, [800.0, 560.0]);
    assert!(frame
        .surfaces
        .iter()
        .all(|s| s.surface.as_str() != ui_registry::id::AI_STATUS));
}

// --- LAY-W11/W17: канонические состояния слепых зон линта ------------------
//
// Аудит ui-kit (§5 LAY-W11) выявил 4 поверхности без канонических состояний
// G4-линта: graph_builder, flow_map, calc-панель, hints. Класс покрытия
// (см. комментарии у каждого теста):
// * flow_map — полный lint: поверхность в реестре (id::FLOW_MAP),
//   hit-rect'ы панели/«✕»/строк в кадре;
// * calc_panel — LAY-W17 закрыл слепую зону: строки панели в кадре
//   (поверхность STAGE публикует calc-var-row-N/calc-formula-row-N после
//   регресс-аудита click_main_stage — окно и строки под линтом);
// * graph_builder, hints — LAY-W17, документированный статус «вне
//   реестра» (решение по фактическому коду): оверлеи рисуются через
//   `screen_bands` в handler.rs (Modals/Popups), клики graph_builder —
//   ранней ветвью `graph_builder_click` ДО pick (input.rs, паттерн
//   транзиентных AI-поверхностей: suggest-карточки/status/agent-панель),
//   hints — keyboard-only (мышь проваливается в канвас намеренно).
//   Регистрация в реестре создала бы вторую, всегда мёртвую pick-ветку
//   поверх ранней и меняла бы hover-глушение absorbs — без выгоды для
//   ввода; канон-состояния ниже — маркеры (нулевые пересечения других
//   поверхностей при открытом оверлее). Assert'ы «поверхности НЕТ в
//   кадре» западают при регистрации и напомнят переработать линт на
//   полный кадр + hit-rect'ы.

/// LAY-W11: карта проливаний — полный G4-lint. Поверхность в реестре
/// (`id::FLOW_MAP`, `build_registry` при `flow_map_open`), hit-rect'ы
/// панели/«✕»/строк в кадре (`fill_hit_rects` → `flow_map_layout`).
/// `test_viewport` — `flow_map_layout` читает `viewport_logical()` (не
/// явный `vp` из `build_frame_at`); без оверрайда панель вырождается в 0×0
/// (паттерн `lint_palette_selected`). Сцена без проливаний — панель с
/// футер-подсказкой (rows_count = 0 → list_h = FOOTER_H).
#[test]
fn lint_flow_map_open() {
    lint_state("flow_map", |app, vp| {
        app.flow_map_open = true;
        app.test_viewport = Some(vp);
    });
    // Верификация покрытия: frame содержит FLOW_MAP поверхность с hit-rect'ами
    // (panel + close; rows_count = 0 — без строк). Слепая зона закрыта.
    let mut app = lint_stub(Language::Ru);
    let vp = [1280.0, 800.0];
    app.flow_map_open = true;
    app.test_viewport = Some(vp);
    let frame = build_frame_at(&app, vp);
    let flow_map = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::FLOW_MAP)
        .expect("FLOW_MAP поверхность в реестре при flow_map_open");
    assert!(
        flow_map.hit_rects.len() >= 2,
        "FLOW_MAP: panel + close hit-rect'ы (got {})",
        flow_map.hit_rects.len()
    );
}

/// LAY-W17: main stage с реальным пучком ≥ 2 рёбер и формулой в приёмнике —
/// панель «Как считается» строится (`calc_panel_ui::layout` → `Some`).
///
/// LAY-W17 закрыл слепую зону: hit-rect'ы СТРОК панели (`var_rows`,
/// `formula_rows`) публикуются поверхностью STAGE (`calc-var-row-N` /
/// `calc-formula-row-N`, модельные индексы). Регресс-аудит click_main_stage
/// (см. комментарий в fill_hit_rects): pick внутри окна stage и прежде был
/// Element{STAGE} → click_main_stage игнорирует имя элемента и пере-хит-
/// тестит той же stage_frame_ctx; колесо — ранний return до pick. Скролл-
/// зависимость видимых строк покрыта сигнатурой кэша кадра
/// (`stage_calc_vars_offset`/`stage_calc_formulas_offset`).
///
/// В кадре STAGE: rect ОКНА + строки панели. Lint детерминирует инварианты
/// окна и строк на нетривиальном срезе (2 ноды + 2 ребра + формула):
/// регрессия `main_stage_rect`/`build_frame_at`/`calc_panel_ui::layout`
/// будет поймана.
///
/// Фикстура: 2 ноды (Исток с выходами users/conv, Отчёт с формулой
/// `x = Исток.users * Исток.conv`), 2 value-ребра src→dst (пучок веса 2 —
/// `MainStageState::open` требует `bundle.weight ≥ 2`). `recompute_flow` —
/// значения выходов истока в потоке (модель панели — `RowValue::Ok`, не
/// `Unknown`).
#[test]
fn lint_calc_panel_open() {
    lint_state("calc_panel", |app, _vp| {
        let mut src = Node::text("src", "Исток\nusers = 10\nconv = 0.2", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = Исток.users * Исток.conv", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 220.0;
        app.scene.canvas.nodes.push(src);
        app.scene.canvas.nodes.push(dst);
        let mut e1 = Edge::new("e1", "src", None, "dst", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("users".to_owned());
        let mut e2 = Edge::new("e2", "src", None, "dst", None);
        e2.set_flow_kind(FlowKind::Value);
        e2.from_output = Some("conv".to_owned());
        app.scene.canvas.edges.push(e1);
        app.scene.canvas.edges.push(e2);
        // Без пересчёта потока модель панели строится, но значения выходов
        // — Unknown; `layout` всё равно вернёт Some (переменные/формулы есть).
        // recompute_flow — каноничность состояния (значения как у FR-044).
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        // Каноничность: stage открыт (пучок веса 2 ≥ 2 — `open` не вырожден).
        assert!(app.main_stage.is_some(), "stage открыт на пучке веса 2");
    });
    // Верификация покрытия: frame содержит STAGE поверхность с hit-rect'ом
    // окна. Слепая зона — строки calc-панели НЕ в кадре (клики через
    // `click_main_stage` → `stage_frame_ctx`, не через реестр); этот lint
    // детерминирует только rect ОКНА stage на нетривиальном пучке.
    let mut app = lint_stub(Language::Ru);
    let vp = [1280.0, 800.0];
    let mut src = Node::text("src", "Исток\nusers = 10\nconv = 0.2", 0.0, 0.0);
    src.width = 420.0;
    src.height = 200.0;
    let mut dst = Node::text("dst", "Отчёт\nx = Исток.users * Исток.conv", 700.0, 0.0);
    dst.width = 420.0;
    dst.height = 220.0;
    app.scene.canvas.nodes.push(src);
    app.scene.canvas.nodes.push(dst);
    let mut e1 = Edge::new("e1", "src", None, "dst", None);
    e1.set_flow_kind(FlowKind::Value);
    e1.from_output = Some("users".to_owned());
    let mut e2 = Edge::new("e2", "src", None, "dst", None);
    e2.set_flow_kind(FlowKind::Value);
    e2.from_output = Some("conv".to_owned());
    app.scene.canvas.edges.push(e1);
    app.scene.canvas.edges.push(e2);
    app.scene.recompute_flow();
    let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
    app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
    let frame = build_frame_at(&app, vp);
    let stage = frame
        .surfaces
        .iter()
        .find(|s| s.surface.as_str() == ui_registry::id::STAGE)
        .expect("STAGE поверхность в реестре при main_stage.open");
    assert!(
        stage.hit_rects.iter().any(|h| h.element == "stage"),
        "STAGE: hit-rect окна stage есть"
    );
    // LAY-W17: слепая зона «строки мимо кадра» закрыта — строки панели в
    // кадре (2 переменных + 1 формула фикстуры), внутри окна stage.
    let var_rows = stage
        .hit_rects
        .iter()
        .filter(|h| h.element.starts_with("calc-var-row-"))
        .count();
    let formula_rows = stage
        .hit_rects
        .iter()
        .filter(|h| h.element.starts_with("calc-formula-row-"))
        .count();
    assert_eq!(var_rows, 2, "строки переменных (users, conv) в кадре");
    assert_eq!(formula_rows, 1, "строка формулы (x = …) в кадре");
    let r = main_stage_rect(vp);
    for hit in stage
        .hit_rects
        .iter()
        .filter(|h| h.element.starts_with("calc-"))
    {
        assert!(
            hit.rect.x >= r.x
                && hit.rect.y >= r.y
                && hit.rect.right() <= r.x + r.w
                && hit.rect.bottom() <= r.y + r.h,
            "строка {} вне окна stage",
            hit.element
        );
    }
}

/// LAY-W11: graph_builder dialog — документированный статус «вне реестра»
/// (LAY-W17). Оверлей рисуется через `screen_bands` (`handler.rs:347-350`,
/// слой Modals — затемнение + карточка диалога `graph_builder_overlay`), НЕ
/// в реестре `build_registry`. Hit-тест — отдельный путь
/// `graph_builder_hit`/`graph_builder_click` (`input.rs:3152`, РАННЯЯ ветвь
/// до pick, паттерн транзиентных AI-поверхностей). Решение LAY-W17:
/// регистрация в реестре не делается — ранняя ветвь перехватывает клики до
/// pick (регистрация дала бы всегда мёртвую pick-ветку и поменяла бы
/// hover-глушение absorbs); assert ниже западает при регистрации и
/// напомнит переработать lint на полный кадр + backdrop-контракт.
#[test]
fn lint_graph_builder_open() {
    lint_state("graph_builder", |app, _vp| {
        app.graph_builder.open = true;
        // Каноничность состояния: текст + режим (по умолчанию Mindmap) —
        // оверлей `graph_builder_overlay` строит ненулевые инстансы (хотя
        // они и не в кадре реестра, рендер-путь детерминирован).
        app.graph_builder.text = "Сводка по продукту: CAC, LTV, отток".into();
        // Слепая зона: frame не содержит graph_builder surface —
        // `lint_frame` проходит тривиально. Маркер документирует состояние.
        assert!(app.graph_builder.open);
    });
    // Верификация слепой зоны: frame НЕ содержит graph_builder поверхности
    // (оверлей рисуется через `screen_bands` в handler.rs:347-350, минуя
    // реестр). Когда поверхность добавят в реестр — этот assert ЗАПАДАЁТ
    // (напоминание переработать lint на полный кадр + backdrop-контракт).
    let mut app = lint_stub(Language::Ru);
    app.graph_builder.open = true;
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert!(
        frame
            .surfaces
            .iter()
            .all(|s| s.surface.as_str() != "graph_builder"),
        "graph_builder НЕ в реестре (слепая зона LAY-W11); добавлен — переработать lint"
    );
}

/// LAY-W11: hints popup — документированный статус «вне реестра» (LAY-W17).
/// Popup рисуется через `screen_bands` (`handler.rs:453-454`, слой Popups —
/// `hints_overlay`), НЕ в реестре `build_registry`. Hit-тест — только
/// клавиатурная навигация (`input.rs:524`: ArrowUp/Down/Enter/Tab/Escape);
/// мышь через popup НЕ перехватывается (клики проваливаются в
/// canvas-цепочку — намеренно, транзиент у каретки). Регистрация в реестре
/// не делается (LAY-W17): Capture-поверхность у каретки перехватила бы
/// клики мира, которых popup сегодня не глотает; assert ниже западает при
/// регистрации и напомнит переработать lint на полный кадр + hit-rect'ы
/// строк.
#[test]
fn lint_hints_open() {
    lint_state("hints", |app, vp| {
        app.hints.open = true;
        app.hints.items = vec![
            hints_ui::HintItem::text(
                hints_ui::HintKind::Var,
                "users".into(),
                "users".into(),
                "переменная".into(),
            ),
            hints_ui::HintItem::text(
                hints_ui::HintKind::Var,
                "conv".into(),
                "conv".into(),
                "переменная".into(),
            ),
        ];
        // Якорь — центр вьюпорта (низ каретки в реальном UI; в headless нет
        // EditingSession — центр каноничен и детерминирован).
        app.hints.anchor = [vp[0] * 0.5, vp[1] * 0.5];
        app.hints.selected = 0;
        // Слепая зона: frame не содержит hints surface — `lint_frame`
        // проходит тривиально. Маркер документирует каноническое состояние.
        assert!(app.hints.open);
        assert!(!app.hints.items.is_empty());
    });
    // Верификация слепой зоны: frame НЕ содержит hints поверхности (popup
    // рисуется через `screen_bands` в handler.rs:453-454, минуя реестр).
    // Когда поверхность добавят в реестр — этот assert ЗАПАДАЁТ (напоминание
    // переработать lint на полный кадр + hit-rect'ы строк).
    let mut app = lint_stub(Language::Ru);
    app.hints.open = true;
    app.hints.items = vec![hints_ui::HintItem::text(
        hints_ui::HintKind::Var,
        "users".into(),
        "users".into(),
        "переменная".into(),
    )];
    app.hints.anchor = [640.0, 400.0];
    let frame = build_frame_at(&app, [1280.0, 800.0]);
    assert!(
        frame.surfaces.iter().all(|s| s.surface.as_str() != "hints"),
        "hints НЕ в реестре (слепая зона LAY-W11); добавлен — переработать lint"
    );
}
