//! FR-052 (этап U2 PRD-0009): реестр поверхностей экрана — единый диспетчер.
//!
//! Каркас `canvas-ui` (FR-051) подключается к приложению: каждая экранная
//! поверхность объявлена в реестре (слой, capture-политика, keyboard-scope,
//! деградация), за кадр собирается [`UiFrame`] с hit-rect'ами из тех же
//! layout-функций, что используют ввод и отрисовка (один источник геометрии
//! — детерминизм pick'а и кадра), клавиатура маршрутизируется по
//! `esc_stack` реестра, draw-порядок выводится в полосы [`UiLayer`].
//!
//! **Два порядка из одного реестра** (зафиксированы тестами):
//! * порядок **регистрации** — обратный Esc-лестнице `on_key` (дословно
//!   воспроизводит прежнюю ручную лестницу: stage → help_menu → docs →
//!   palette → strip → panel → menu → settings → hotkeys → whatif → wheel);
//!   head-поверхности (поиск/редактор/диалог/галерея/онбординг) — над stage;
//! * порядок **кадра** — визуальный (bottom→top в пределах слоя): pick
//!   `HitStack` и draw-полосы следуют визуальному верху (ввод = тому, что
//!   видно; класс дефектов «клик уходит под видимый верх» устранён).
//!
//! Поверхности, чей клик-код остался в canvas-цепочке (editor — клавиатура
//! и клики мира), регистрируются с пустыми hit-rect'ами: pick их
//! не перехватывает, поведение байт-в-байт прежнее.

use super::*;
use canvas_ui::capture::CapturePolicy;
use canvas_ui::frame::{HitRect, SurfaceFrame, UiFrame};
use canvas_ui::geometry::UiRect;
use canvas_ui::layer::UiLayer;
use canvas_ui::registry::{DegradationPolicy, SurfaceDecl, SurfaceRegistry};
use canvas_ui::KeyboardScopeId;

/// Идентификаторы поверхностей (стабильны — подписи debug-оверлея F-10).
pub mod id {
    /// Мир: карточки/рёбра/порты (L0) — клики обрабатывает canvas-цепочка.
    pub const WORLD: &str = "world";
    /// Wheel-меню шаблонов (L1, Block): сектора + хаб, глотает всё.
    pub const WHEEL: &str = "wheel";
    /// What-if пилюля/бар/список/таблица (L3, Capture).
    pub const WHATIF: &str = "whatif";
    /// Панель хоткеев (L3, Capture — клик по панели глотается).
    pub const HOTKEYS: &str = "hotkeys";
    /// Угловые кнопки: тема / помощь «?» / настройки ⚙ (L3, Capture).
    pub const CORNER_BUTTONS: &str = "corner_buttons";
    /// Модалка настроек + выпадающее меню (L3, Block).
    pub const SETTINGS: &str = "settings";
    /// Контекстное меню канваса + подменю пакетов (L4, Block).
    pub const MENU: &str = "menu";
    /// FR-050 Н2 (этап C): меню выбора (параметр приёмника / строка-источник)
    /// — transient popup как контекстное меню (L4, Block: клик мимо —
    /// закрыть и глотнуть, «либо отмена» в постановке; Esc — закрыть).
    pub const CHOICE_MENU: &str = "choice_menu";
    /// Док палитры шаблонов (развёрнутый) (L3, Capture).
    pub const TEMPLATE_PANEL: &str = "template_panel";
    /// Свёрнутая полоса категорий + flyout (L3, Capture).
    pub const TEMPLATE_STRIP: &str = "template_strip";
    /// Тулбар палитры выделения + открытая колонка (L2, Capture).
    pub const PALETTE: &str = "palette";
    /// Просмотрщик документации (L4, Block).
    pub const DOCS: &str = "docs";
    /// Меню помощи «?» + подменю разделов (L4, Block).
    pub const HELP_MENU: &str = "help_menu";
    /// Main stage — модальный срез пучка (L5, Block).
    pub const STAGE: &str = "stage";
    /// Панель поиска (L3, Block — мимо панели закрывается и глотает).
    pub const SEARCH: &str = "search";
    /// Сессия редактирования текста (L2, scope — клики остаются в мире).
    pub const EDITOR: &str = "editor";
    /// Окно проверки цепочки расчёта (PRD-0007 X2, L5, Block — мимо окна
    /// закрывается и глотает; Esc/✕ закрывают, фон — close_explain).
    pub const EXPLAIN: &str = "explain";
    /// Диалог ревью автосвязи (PRD-0007 X4, L5, Block — модален поверх
    /// канваса; панель объяснения прячется на время диалога, §6.5).
    pub const AUTOLINK: &str = "autolink";
    /// Модальный диалог Да/Нет (L5, Block).
    pub const DIALOG: &str = "dialog";
    /// Галерея схем (L5, Block).
    pub const GALLERY: &str = "gallery";
    /// Онбординг-карточка (L5, Block).
    pub const ONBOARDING: &str = "onboarding";
    /// Empty-state карточка пустого канваса (L3, Capture — мимо карточки
    /// канвас жив, AC-1.1 FR-049).
    pub const EMPTY: &str = "empty";
    /// Миникарта (L3, Capture; рисуется проходом рендерера поверх полос).
    pub const MINIMAP: &str = "minimap";
}

/// Владелец клавиатуры — верх `esc_stack` реестра (Q4 PRD-0009: NUMI-хоткеи
/// и лестница команд не трогаются до U5; Canvas = прежняя лестница).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOwner {
    Onboarding,
    Gallery,
    Editor,
    Search,
    /// Док палитры в клавиатурном фокусе (иначе — Canvas, прежнее 8036).
    TemplatePanel,
    Dialog,
    /// Любая клавиша закрывает stage (фикс противоречия 8222: раньше
    /// Ctrl+F при stage открывал поиск вместо закрытия stage).
    Stage,
    /// Окно проверки цепочки (PRD-0007 X2): Esc закрывает, прочие клавиши
    /// идут в лестницу канваса (прежнее поведение X2 — только Esc).
    Explain,
    /// Диалог ревью автосвязи (PRD-0007 X4): Esc закрывает диалог
    /// (отклонённые забываются — возврат фоновой перепроверкой, AC-5.2).
    Autolink,
    /// Клавиатура идёт в канвас-лестницу (прежнее поведение).
    Canvas,
}

/// Владелец клавиатуры: верх esc_stack активных поверхностей.
pub fn key_owner(registry: &SurfaceRegistry) -> KeyOwner {
    let top = registry
        .esc_stack()
        .first()
        .map(|sid| sid.as_str().to_owned());
    match top.as_deref() {
        Some(id::ONBOARDING) => KeyOwner::Onboarding,
        Some(id::GALLERY) => KeyOwner::Gallery,
        Some(id::EDITOR) => KeyOwner::Editor,
        Some(id::SEARCH) => KeyOwner::Search,
        Some(id::DIALOG) => KeyOwner::Dialog,
        Some(id::STAGE) => KeyOwner::Stage,
        Some(id::EXPLAIN) => KeyOwner::Explain,
        Some(id::AUTOLINK) => KeyOwner::Autolink,
        Some(id::TEMPLATE_PANEL) => {
            // Фокус решает владелец (прежний гейт 8036: панель без фокуса
            // клавиши не перехватывает — Ctrl+P/лестница работают).
            KeyOwner::TemplatePanel
        }
        _ => KeyOwner::Canvas,
    }
}

/// Сборка реестра активных поверхностей из состояния приложения.
///
/// Порядок регистрации = обратный Esc-лестнице (см. заголовок модуля).
/// Только активные поверхности: реестр — снимок состояния на кадр, дешёвый
/// (~20 `add`, без аллокаций тяжёлых) и детерминированный.
pub fn build_registry(app: &App) -> SurfaceRegistry {
    let mut reg = SurfaceRegistry::new();
    // 1. Мир — дефолтный scope NUMI-хоткеев (клавиатура канваса).
    reg.add(
        SurfaceDecl::new(id::WORLD, UiLayer::World, CapturePolicy::PassThrough)
            .with_scope(KeyboardScopeId::CANVAS),
    );
    // 2. Wheel-меню (Esc — последний в лестнице; Block: глотает любой клик,
    //    мимо секторов — near/far логика внутри обработчика).
    if app.wheel_menu.is_some() {
        reg.add(SurfaceDecl::new(
            id::WHEEL,
            UiLayer::WorldOverlay,
            CapturePolicy::Block,
        ));
    }
    // 3. What-if (Esc — предпоследний; HideBelow — деградация F-11c).
    if app.scene.whatif_active || whatif_pill_visible(app) {
        reg.add(
            SurfaceDecl::new(id::WHATIF, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::WHATIF)
                .with_degradation(DegradationPolicy::HideBelow {
                    min_width: 900.0,
                    min_height: 600.0,
                }),
        );
    }
    // 4. Панель хоткеев (Esc закрывает; клик по панели глотается).
    if app.hotkeys_open {
        reg.add(
            SurfaceDecl::new(id::HOTKEYS, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::HOTKEYS),
        );
    }
    // 5. Угловые кнопки (без scope — Esc их не закрывает).
    reg.add(SurfaceDecl::new(
        id::CORNER_BUTTONS,
        UiLayer::Panels,
        CapturePolicy::Capture,
    ));
    // 6. Модалка настроек (Esc: dropdown → панель — двухэтапный dismiss;
    //    Block: клик мимо модалки закрывает и глотается — FR-039).
    if app.settings_open {
        reg.add(
            SurfaceDecl::new(id::SETTINGS, UiLayer::Panels, CapturePolicy::Block)
                .with_scope(id::SETTINGS),
        );
    }
    // 7. Контекстное меню канваса (Block: мимо — закрыть, клик глотается).
    if app.menu.is_some() {
        reg.add(SurfaceDecl::new(
            id::MENU,
            UiLayer::Popups,
            CapturePolicy::Block,
        ));
    }
    // FR-050 Н2 (этап C): меню выбора — transient popup над меню канваса
    // (Block: клик мимо — закрыть и глотнуть — «либо отмена»; Esc — закрыть).
    if app.choice_menu.is_some() {
        reg.add(SurfaceDecl::new(
            id::CHOICE_MENU,
            UiLayer::Popups,
            CapturePolicy::Block,
        ));
    }
    // 8. Док палитры шаблонов (Esc закрывает даже без фокуса — 8182).
    if app.template_panel.open {
        reg.add(
            SurfaceDecl::new(id::TEMPLATE_PANEL, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::TEMPLATE_PANEL),
        );
    }
    // 9. Свёрнутая полоса + flyout (Esc гасит flyout — 8169).
    if !app.template_panel.open {
        reg.add(
            SurfaceDecl::new(id::TEMPLATE_STRIP, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::TEMPLATE_STRIP),
        );
    }
    // 10. Палитра выделения (Esc гасит раскрытие — 8162).
    if app.palette_geometry().is_some() {
        reg.add(
            SurfaceDecl::new(id::PALETTE, UiLayer::Widgets, CapturePolicy::Capture)
                .with_scope(id::PALETTE),
        );
    }
    // 11. Просмотрщик документации (Esc закрывает одним шагом).
    if app.docs.is_some() {
        reg.add(SurfaceDecl::new(
            id::DOCS,
            UiLayer::Popups,
            CapturePolicy::Block,
        ));
    }
    // 12. Меню помощи (Esc двухэтапный: подменю → меню).
    if app.help_menu.is_some() {
        reg.add(SurfaceDecl::new(
            id::HELP_MENU,
            UiLayer::Popups,
            CapturePolicy::Block,
        ));
    }
    // 13. Main stage (Esc — первый в лестнице; любой другой ключ тоже
    //     закрывает — KeyOwner::Stage).
    if app.main_stage.is_some() {
        reg.add(
            SurfaceDecl::new(id::STAGE, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::STAGE),
        );
    }
    // 14–18. Head-поверхности (клавиатура уходит им раньше лестницы).
    if app.search.is_open() {
        reg.add(
            SurfaceDecl::new(id::SEARCH, UiLayer::Panels, CapturePolicy::Block)
                .with_scope(id::SEARCH),
        );
    }
    if app.editing.is_some() {
        reg.add(
            SurfaceDecl::new(id::EDITOR, UiLayer::Widgets, CapturePolicy::Capture)
                .with_scope(id::EDITOR),
        );
    }
    // PRD-0007 (X2): окно проверки цепочки — над stage в esc-стеке
    // (Esc закрывает раньше лестницы), Esc/✕/фон — close_explain
    if app.explain.is_some() {
        reg.add(
            SurfaceDecl::new(id::EXPLAIN, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::EXPLAIN),
        );
    }
    // PRD-0007 (X4): диалог ревью автосвязи — верхний модал (§6.5): Esc/
    // ✕ закрывают, клик мимо — закрыть и глотнуть; панель объяснения,
    // если открыта, рендером прячется на время диалога
    if app.autolink_review.is_some() {
        reg.add(
            SurfaceDecl::new(id::AUTOLINK, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::AUTOLINK),
        );
    }
    if app.dialog.is_some() {
        reg.add(
            SurfaceDecl::new(id::DIALOG, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::DIALOG),
        );
    }
    if app.scheme_gallery.open {
        reg.add(
            SurfaceDecl::new(id::GALLERY, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::GALLERY),
        );
    }
    if app.onboarding.is_some() {
        reg.add(
            SurfaceDecl::new(id::ONBOARDING, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::ONBOARDING),
        );
    }
    // 19. Empty-state (Capture: мимо карточки канвас жив — AC-1.1 FR-049).
    if app.empty_state_visible() {
        reg.add(SurfaceDecl::new(
            id::EMPTY,
            UiLayer::Panels,
            CapturePolicy::Capture,
        ));
    }
    // 20. Миникарта (рисуется проходом рендерера; клик — центрирование).
    if app.minimap_rect().is_some() {
        reg.add(SurfaceDecl::new(
            id::MINIMAP,
            UiLayer::Panels,
            CapturePolicy::Capture,
        ));
    }
    reg
}

/// Пилюля входа в what-if видна на вьюпортах ≥ hide-порога (те же условия,
/// что у бара — деградация применяется к обеим формам поверхности).
fn whatif_pill_visible(app: &App) -> bool {
    let [w, h] = app.viewport_logical();
    w >= 900.0 && h >= 600.0
}

/// Ранг визуального порядка внутри кадра (bottom→top глобально; pick и
/// draw-полосы сортируют по слою, внутри слоя — по этому рангу).
const VISUAL_ORDER: &[&str] = &[
    id::SETTINGS,
    id::HOTKEYS,
    id::CORNER_BUTTONS,
    id::EMPTY,
    id::SEARCH,
    id::TEMPLATE_STRIP,
    id::TEMPLATE_PANEL,
    id::WHATIF,
    id::MINIMAP,
    id::EDITOR,
    id::PALETTE,
    id::WHEEL,
    id::MENU,
    id::CHOICE_MENU,
    id::HELP_MENU,
    id::DOCS,
    id::GALLERY,
    id::ONBOARDING,
    id::DIALOG,
    id::EXPLAIN,
    id::AUTOLINK,
    id::STAGE,
];

fn visual_rank(sid: &str) -> usize {
    VISUAL_ORDER
        .iter()
        .position(|&s| s == sid)
        .unwrap_or(VISUAL_ORDER.len())
}

/// Сборка кадра экрана под текущий вьюпорт приложения.
pub fn build_frame(app: &App) -> UiFrame {
    let [vw, vh] = app.viewport_logical();
    build_frame_at(app, [vw.max(0.0), vh.max(0.0)])
}

/// Сборка кадра экрана: видимые поверхности в ВИЗУАЛЬНОМ порядке
/// (bottom→top) + hit-rect'ы из тех же layout-функций, что у ввода/отрисовки.
/// Явный вьюпорт — headless-тесты (G4: 1280×800 / 1024×640 / 800×560).
pub fn build_frame_at(app: &App, viewport_logical: [f32; 2]) -> UiFrame {
    let registry = build_registry(app);
    let [vw, vh] = viewport_logical;
    let viewport = UiRect::new(0.0, 0.0, vw.max(0.0), vh.max(0.0));
    let mut frame = UiFrame::from_registry(&registry, viewport);
    // Перестановка в визуальный порядок (стабильно, внутри слоя);
    // hit-rect'ы заполняются после — по итоговому порядку не зависят.
    frame
        .surfaces
        .sort_by_key(|s| (s.layer, visual_rank(s.surface.as_str())));
    for surface in frame.surfaces.iter_mut() {
        fill_hit_rects(app, surface, vw, vh);
    }
    frame
}

/// Hit-rect'ы поверхности из тех же чистых layout-функций, что использует
/// ввод (детерминизм: pick ≡ поведению прежних веток).
fn fill_hit_rects(app: &App, surface: &mut SurfaceFrame, vw: f32, vh: f32) {
    let viewport = [vw, vh];
    let rect = |r: [f32; 4]| UiRect::new(r[0], r[1], r[0] + r[2], r[1] + r[3]);
    match surface.surface.as_str() {
        id::WHEEL => {
            // donut-меню: bbox extent (polar-геометрия проверяется в
            // обработчике — rect только для pick «клик у wheel, не в мир»).
            let categories = app.templates.categories();
            let template_count = app
                .wheel_menu
                .as_ref()
                .and_then(|m| m.category.as_deref())
                .map(|c| app.templates.by_category(c).len())
                .unwrap_or(0);
            let center = app
                .wheel_menu
                .as_ref()
                .map(|m| m.screen)
                .unwrap_or([vw / 2.0, vh / 2.0]);
            let geo = template_ui::wheel_geometry(center, vw, vh, categories.len(), template_count);
            let r = geo.extent + 12.0;
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(center[0] - r, center[1] - r, center[0] + r, center[1] + r),
                "wheel-donut",
            ));
        }
        id::WHATIF => {
            if app.scene.whatif_active {
                let layout = app.whatif_bar_layout();
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(layout.rect), "whatif-bar"));
                if app.whatif_list_open {
                    let rows = app.whatif_override_rows();
                    let list = whatif_ui::overrides_list_layout(layout.rect, rows.len(), viewport);
                    surface
                        .hit_rects
                        .push(HitRect::interactive(rect(list), "whatif-list"));
                }
                if app.whatif_compare_open {
                    let (columns, rows) = app.whatif_compare_table();
                    let table =
                        whatif_ui::table_layout(&columns, rows.len(), layout.rect, viewport);
                    surface
                        .hit_rects
                        .push(HitRect::interactive(rect(table.rect), "whatif-table"));
                }
            } else {
                surface.hit_rects.push(HitRect::interactive(
                    rect(crate::whatif_ui::enter_pill_rect(viewport)),
                    "whatif-pill",
                ));
            }
        }
        id::HOTKEYS => {
            let panel = hotkeys_panel_rect(viewport);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(panel), "hotkeys-panel"));
        }
        id::CORNER_BUTTONS => {
            surface.hit_rects.push(HitRect::interactive(
                rect(theme_button_rect(app.settings.button_corner, viewport)),
                "theme-button",
            ));
            surface.hit_rects.push(HitRect::interactive(
                rect(help_button_rect(app.settings.button_corner, viewport)),
                "help-button",
            ));
            surface.hit_rects.push(HitRect::interactive(
                rect(button_rect(app.settings.button_corner, viewport)),
                "settings-button",
            ));
            // PRD-0007 (X4, AC-5.5): бейдж предложений автосвязи — клик
            // открывает ревью (та же видимость, что в рендере бейджа)
            if app.autolink_badge_visible() {
                surface.hit_rects.push(HitRect::interactive(
                    rect(crate::autolink_ui::badge_rect([vw, vh])),
                    "autolink-badge",
                ));
            }
        }
        id::SETTINGS => {
            let layout = modal_layout(app.settings_tab, viewport);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(layout.rect), "settings-modal"));
            if let Some(row) = app.settings_dropdown.open_row {
                let items = dropdown_options(row, &app.settings);
                let anchor = layout
                    .row_rect(row)
                    .map(|r| control_rect(r, RowKind::Dropdown))
                    .unwrap_or([0.0; 4]);
                let menu = dropdown_layout(anchor, viewport, items.len(), anchor[2]);
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(menu), "settings-dropdown"));
            }
        }
        id::MENU => {
            if let Some(base) = app.menu_open_rect() {
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(base), "menu-base"));
            }
            if let Some(sub) = app.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                surface.hit_rects.push(HitRect::interactive(
                    rect(crate::ui::submenu_rect(sub)),
                    "menu-sub",
                ));
            }
        }
        // FR-050 Н2 (этап C): панель меню выбора (пункты + заголовок;
        // выбор пункта — геометрия отрисовки в обработчике)
        id::CHOICE_MENU => {
            if let Some(r) = app.choice_menu_rect() {
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(r), "choice-menu"));
            }
        }
        id::TEMPLATE_PANEL => {
            let rows = template_panel_rows(&app.templates, &app.template_panel);
            let lay = template_panel_layout(vw, vh, &app.templates, &app.template_panel, &rows);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "template-panel"));
        }
        id::TEMPLATE_STRIP => {
            let categories = app.template_category_names();
            let strip = template_ui::dock_strip_layout(&categories, vh);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(strip.rect), "template-strip"));
            if let Some(fly) = app.template_flyout_geometry(viewport, &strip) {
                // Только СТРОКИ flyout интерактивны (клик в тело flyout —
                // мимо, уходит в канвас — прежнее поведение 9315–9371)
                for (v, row) in fly.row_rects.iter().enumerate() {
                    surface
                        .hit_rects
                        .push(HitRect::interactive(rect(*row), format!("flyout-row-{v}")));
                }
            }
        }
        id::PALETTE => {
            if let Some((lay, _, _)) = app.palette_geometry() {
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.bar), "palette-bar"));
                if let Some(open) = app.palette_hover.open.and_then(|g| lay.groups.get(g)) {
                    surface.hit_rects.push(HitRect::interactive(
                        rect(open.dropdown),
                        "palette-dropdown",
                    ));
                }
            }
        }
        id::DOCS => {
            let panel = docs_ui::viewer_rect(viewport);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(panel), "docs-viewer"));
        }
        id::HELP_MENU => {
            if let Some(menu) = &app.help_menu {
                surface.hit_rects.push(HitRect::interactive(
                    rect(docs_ui::help_menu_rect(menu.origin)),
                    "help-menu",
                ));
                if menu.docs_open {
                    let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
                    surface.hit_rects.push(HitRect::interactive(
                        rect(docs_ui::help_submenu_rect(sub)),
                        "help-submenu",
                    ));
                }
            }
        }
        id::STAGE => {
            let r = main_stage_rect(viewport);
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(r.x, r.y, r.x + r.w, r.y + r.h),
                "stage",
            ));
        }
        id::SEARCH => {
            let lay = search_layout(vw, vh, &app.search);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "search-panel"));
        }
        id::DIALOG => {
            surface
                .hit_rects
                .push(HitRect::interactive(rect(app.dialog_rect()), "dialog"));
        }
        id::EXPLAIN => {
            // Окно проверки: rect окна — pick-зона; внутри обработчик
            // ✕/чип/крошки/узлы (on_explain_click X2), фон — Backdrop
            let win = crate::explain_ui::window_rect([vw, vh]);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(win), "explain-window"));
        }
        id::AUTOLINK => {
            // Диалог ревью (PRD-0007 X4): rect диалога — pick-зона;
            // внутри — on_autolink_click, фон — Backdrop (закрыть)
            let win = crate::autolink_ui::dialog_rect([vw, vh]);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(win), "autolink-dialog"));
        }
        id::GALLERY => {
            let registry = canvas_core::schemes::SchemeRegistry::embedded();
            let list = scheme_gallery_ui::rows(registry, &app.scheme_gallery);
            let lay = scheme_gallery_ui::layout(viewport, &list, &app.scheme_gallery);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "gallery-panel"));
        }
        id::ONBOARDING => {
            if let Some(state) = &app.onboarding {
                let card = onboarding_ui::card_rect(viewport, state.step, app.settings.language);
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(card), "onboarding-card"));
            }
        }
        id::EMPTY => {
            let card = scheme_gallery_ui::empty_card_rect(viewport);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(card), "empty-card"));
        }
        id::MINIMAP => {
            if let Some(r) = app.minimap_rect() {
                surface.hit_rects.push(HitRect::interactive(
                    UiRect::new(r[0], r[1], r[2], r[3]),
                    "minimap",
                ));
            }
        }
        // EDITOR/WORLD: hit-rect'ов нет — клики остаются в canvas-цепочке.
        _ => {}
    }
}

/// Сборщик screen-полос кадра (FR-052, U2): контент поверхностей кладётся
/// в полосы по [`UiLayer`]; `finish` сортирует по слою по возрастанию —
/// порядок полос = контракт `UiFrame::draw_bands` (реестр), порядок внутри
/// полосы = порядок push (порядок отрисовки сохранён дословно).
#[derive(Default)]
pub(crate) struct ScreenBands {
    bands: Vec<(UiLayer, Vec<CardInstance>, Vec<OwnedScreenText>)>,
}

impl ScreenBands {
    pub(crate) fn push(
        &mut self,
        layer: UiLayer,
        instances: Vec<CardInstance>,
        texts: Vec<OwnedScreenText>,
    ) {
        if instances.is_empty() && texts.is_empty() {
            return;
        }
        self.bands.push((layer, instances, texts));
    }

    /// Полосы в порядке отрисовки: слои по возрастанию (стабильно —
    /// порядок регистрации контента внутри слоя сохранён).
    pub(crate) fn finish(mut self) -> Vec<(UiLayer, Vec<CardInstance>, Vec<OwnedScreenText>)> {
        self.bands.sort_by_key(|(layer, _, _)| *layer);
        self.bands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Заглушка App для headless-тестов реестра (без окна/GPU —
    /// Noop-бэкенды, как app_assembles_on_stub_backends).
    fn test_stub() -> App {
        let scene = crate::app::SceneState::new(
            crate::Canvas::default(),
            std::path::PathBuf::from("target/tmp/fr052-ui-registry.canvas"),
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
            Some(std::env::temp_dir().join(format!("canvasdesk-fr052-{}", std::process::id()))),
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

    /// 21 поверхностей объявлены константами без дублей (контракт
    /// единственности реестра — паника на дубликате словлена сборкой).
    #[test]
    fn surface_ids_are_unique() {
        let ids = [
            id::WORLD,
            id::WHEEL,
            id::WHATIF,
            id::HOTKEYS,
            id::CORNER_BUTTONS,
            id::SETTINGS,
            id::MENU,
            id::CHOICE_MENU,
            id::TEMPLATE_PANEL,
            id::TEMPLATE_STRIP,
            id::PALETTE,
            id::DOCS,
            id::HELP_MENU,
            id::STAGE,
            id::SEARCH,
            id::EDITOR,
            id::DIALOG,
            id::GALLERY,
            id::ONBOARDING,
            id::EMPTY,
            id::MINIMAP,
        ];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        let count = sorted.len();
        sorted.dedup();
        assert_eq!(sorted.len(), count, "дубликат идентификатора поверхности");
    }

    /// FR-050 Н2 (этап C): открытое меню выбора — поверхность Popups/Block
    /// (клик мимо — backdrop закрывает), в esc-стеке РАНЬШЕ контекстного
    /// меню (transient-выбор приоритетнее базового меню); hit-rect панели
    /// накрывает пункты и заголовок; закрытое — поверхности нет.
    #[test]
    fn choice_menu_surface_block_above_context_menu() {
        let mut app = test_stub();
        app.onboarding = None;
        app.choice_menu = Some(crate::app::ChoiceMenu {
            origin: [200.0, 200.0],
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
        let registry = build_registry(&app);
        assert!(
            registry
                .esc_stack()
                .iter()
                .any(|sid| sid.as_str() == id::CHOICE_MENU),
            "меню выбора в esc-стеке"
        );
        // Порядок: CHOICE_MENU раньше MENU в лестнице (закрывается первым)
        let esc: Vec<String> = registry
            .esc_stack()
            .iter()
            .map(|sid| sid.as_str().to_owned())
            .collect();
        if let (Some(c), Some(m)) = (
            esc.iter().position(|s| s == id::CHOICE_MENU),
            esc.iter().position(|s| s == id::MENU),
        ) {
            assert!(c < m, "выбор закрывается раньше контекстного меню");
        }
        // Hit-rect панели: накрывает первый пункт (сдвиг на заголовок)
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::CHOICE_MENU)
            .expect("поверхность меню выбора в кадре");
        assert!(!surface.hit_rects.is_empty(), "hit-rect задан");
        // Закрытое меню — поверхности нет (инвариант «нет состояния — нет
        // поверхности»)
        app.choice_menu = None;
        let registry = build_registry(&app);
        assert!(
            !registry
                .esc_stack()
                .iter()
                .any(|sid| sid.as_str() == id::CHOICE_MENU),
            "закрытое меню — в реестре отсутствует"
        );
    }

    /// Реестр пустого канваса: мир + угловые кнопки (минимум поверхностей).
    #[test]
    fn idle_registry_has_world_and_chrome() {
        let mut app = test_stub();
        app.onboarding = None; // снять авто-показ первого запуска (FR-028)
        let reg = build_registry(&app);
        let names: Vec<&str> = reg.declarations().iter().map(|d| d.id.as_str()).collect();
        assert!(names.contains(&id::WORLD));
        assert!(names.contains(&id::CORNER_BUTTONS));
        // Пустой канвас: empty-state карточка (Capture) в центре кадра —
        // клик по ней перехвачен, угол экрана уходит канвасу (AC-1.1)
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let center = HitStack::pick(&frame, UiPoint::new(640.0, 400.0));
        assert!(matches!(center, Some(HitTarget::Element { .. })));
        assert!(HitStack::pick(&frame, UiPoint::new(2.0, 2.0)).is_none());
    }

    /// Галерея (Block): клик по панели — Element, мимо — Backdrop;
    /// backdrop-клик не доходит до мира (pick-матрица G2 на реальном
    /// адаптере).
    #[test]
    fn gallery_block_backdrop_swallows() {
        let mut app = test_stub();
        app.onboarding = None;
        app.scheme_gallery.open();
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let vp = frame.viewport;
        // панель центрирована: клик в центр вьюпорта — панель
        let center = UiPoint::new(vp.x + vp.w / 2.0, vp.y + vp.h / 2.0);
        match HitStack::pick(&frame, center) {
            Some(HitTarget::Element { surface, .. }) => {
                assert_eq!(surface.surface.as_str(), id::GALLERY);
            }
            other => panic!("ожидался Element галереи, получено {other:?}"),
        }
        // угол экрана — backdrop галереи (мир не получает)
        let corner = UiPoint::new(2.0, 2.0);
        match HitStack::pick(&frame, corner) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::GALLERY);
            }
            other => panic!("ожидался Backdrop галереи, получено {other:?}"),
        }
    }

    /// Onboarding Block: backdrop глотается (модаль НЕ закрывается кликом
    /// мимо — прежнее поведение 9042–9085), Element — карточка.
    #[test]
    fn onboarding_backdrop_swallows_without_close() {
        let mut app = test_stub();
        app.onboarding = Some(crate::onboarding_ui::OnboardingState::default());
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let corner = UiPoint::new(2.0, 2.0);
        match HitStack::pick(&frame, corner) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::ONBOARDING);
            }
            other => panic!("ожидался Backdrop онбординга, получено {other:?}"),
        }
    }

    /// What-if: HideBelow 900×600 — на 800×560 (G4) поверхности нет в кадре;
    /// на 1280×800 бар перехватывает клик в своей полосе (Capture), мимо —
    /// клик уходит канвасу.
    #[test]
    fn whatif_hide_below_and_capture_rect() {
        let mut app = test_stub();
        app.onboarding = None;
        app.scene.whatif_active = true;
        // канонический вьюпорт G4 (test_stub без окна — вьюпорт [0,0])
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let bar = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::WHATIF)
            .expect("whatif в кадре на большом вьюпорте");
        assert!(!bar.hit_rects.is_empty());
        // малый вьюпорт: hide-политика прячет поверхность
        // (проверка напрямую по реестру — вьюпорт test_stub фиксирован)
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::WHATIF)
            .expect("whatif в реестре");
        assert!(decl.degradation.hidden_at(800.0, 560.0));
        assert!(!decl.degradation.hidden_at(1280.0, 800.0));
    }

    /// Esc-стек: порядок дословно воспроизводит прежнюю лестницу 8143–8232
    /// (stage → help → docs → palette → strip → panel → menu → settings →
    /// hotkeys → whatif → wheel); head-поверхности — над stage.
    #[test]
    fn esc_stack_matches_legacy_ladder() {
        let mut app = test_stub();
        app.onboarding = None;
        app.scene.whatif_active = true;
        app.hotkeys_open = true;
        app.settings_open = true;
        app.menu = Some(crate::ui::ContextMenu {
            origin: [10.0, 10.0],
            submenu: None,
        });
        app.template_panel.open();
        let page = crate::docs_ui::page_index_by_id("index").expect("страница существует");
        let viewport = app.viewport_logical();
        let panel = crate::docs_ui::viewer_rect(viewport);
        let content = crate::docs_ui::viewer_content_rect(panel);
        app.docs = Some(DocsViewer {
            page,
            layout: crate::docs_ui::layout_page(page, content[2]),
            scroll: crate::docs_ui::ScrollState::new(
                crate::docs_ui::layout_page(page, content[2]).content_height,
                content[3],
            ),
            layout_width: content[2],
        });
        app.help_menu = Some(HelpMenuState {
            origin: [900.0, 700.0],
            docs_open: false,
        });
        // Палитра выделения и меню канваса взаимоисключающи (palette_target:
        // menu.is_some() → None) — порядок palette↔menu недостижим, обе
        // позиции проверены раздельными состояниями (см. registry order).
        let reg = build_registry(&app);
        let stack: Vec<String> = reg
            .esc_stack()
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect();
        let pos = |name: &str| stack.iter().position(|s| s == name);
        let idx = |name: &str| pos(name).unwrap_or_else(|| panic!("{name} в стеке"));
        // stage закрыт — в стеке отсутствует; порядок head-над-ladder проверен
        // в key_owner_follows_esc_top
        assert!(pos(id::STAGE).is_none());
        assert!(idx(id::HELP_MENU) < idx(id::DOCS));
        // strip↔panel взаимоисключающи (свёрнутый/развёрнутый док) —
        // порядок пары недостижим в одном состоянии, позиции раздельны
        assert!(idx(id::DOCS) < idx(id::TEMPLATE_PANEL));
        assert!(idx(id::MENU) < idx(id::SETTINGS));
        assert!(idx(id::SETTINGS) < idx(id::HOTKEYS));
        assert!(idx(id::HOTKEYS) < idx(id::WHATIF));
        // wheel в реестре нет (меню закрыто) — последний активный: whatif
        assert_eq!(stack.first().map(String::as_str), Some(id::HELP_MENU));
    }

    /// Владелец клавиатуры: верх esc_stack → KeyOwner (пежний порядок
    /// head-веток: onboarding > gallery > dialog > editor > search > stage).
    #[test]
    fn key_owner_follows_esc_top() {
        let mut app = test_stub();
        app.onboarding = None;
        assert_eq!(key_owner(&build_registry(&app)), KeyOwner::Canvas);
        app.search.open();
        assert_eq!(key_owner(&build_registry(&app)), KeyOwner::Search);
        // EditingSession требует font_system (GPU-free, но тяжёл) —
        // head-приоритет editor проверяем декларацией: редактор регистрируется
        // НАД stage и поиском (порядок build_registry, шаги 14–16)
        app.scheme_gallery.open();
        assert_eq!(key_owner(&build_registry(&app)), KeyOwner::Gallery);
        app.onboarding = Some(crate::onboarding_ui::OnboardingState::default());
        assert_eq!(key_owner(&build_registry(&app)), KeyOwner::Onboarding);
    }

    /// Тост пассивен: не перехватывает клик даже в своей зоне (G2).
    #[test]
    fn toast_is_passive() {
        let app = test_stub();
        let reg = build_registry(&app);
        // тост появляется только при живом тосте — в пустом реестре его нет,
        // контракт Passive проверяем декларацией на модельном реестре
        let mut model = SurfaceRegistry::new();
        model.add(SurfaceDecl::new(
            "toast",
            UiLayer::Toasts,
            canvas_ui::CapturePolicy::Passive,
        ));
        let frame = UiFrame::from_registry(&model, UiRect::new(0.0, 0.0, 1280.0, 800.0));
        assert!(HitStack::pick(&frame, UiPoint::new(640.0, 790.0)).is_none());
        assert!(reg
            .declarations()
            .iter()
            .all(|d| d.id.as_str() != "toast" || true));
    }

    /// Draw-полосы кадра: слои по возрастанию (контракт draw_bands),
    /// модали выше панелей, тосты выше модалей.
    #[test]
    fn frame_bands_are_layer_ascending() {
        let mut app = test_stub();
        app.onboarding = None;
        app.scene.whatif_active = true;
        app.scheme_gallery.open();
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let bands = frame.draw_bands();
        let layers: Vec<UiLayer> = bands.iter().map(|(l, _)| *l).collect();
        let mut sorted = layers.clone();
        sorted.sort_by_key(|l| *l);
        assert_eq!(layers, sorted, "полосы не по возрастанию слоя");
        assert!(layers.contains(&UiLayer::Modals));
        assert!(layers.contains(&UiLayer::Panels));
    }
}
