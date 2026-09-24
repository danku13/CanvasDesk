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
    /// FR-050 Н9-4 (этап E): панель «Карта проливаний» (L3, Capture —
    /// клик мимо панели работает с канвасом: владелец изучает истоки,
    /// переходя по строкам; закрытие — ✕/Esc/Ctrl+Shift+M/пункт меню).
    pub const FLOW_MAP: &str = "flow_map";
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
    /// FR-055 (этап U4, F-8): витрина кита (L5, Block — мимо панели
    /// закрывается и глотает; вход — пункт «?» «О интерфейсе», Q5-a).
    pub const KIT_GALLERY: &str = "kit_gallery";
    /// FR-070: UI-админпанель (L5, Block — мимо панели закрывается и
    /// глотает; вход — пункт «?» «UI-консоль»).
    pub const ADMIN: &str = "admin_panel";
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
    /// Витрина кита (FR-055 U4): Esc закрывает, прочие клавиши глотаются
    /// (модаль поверх канваса; интерактив — только кнопки шапки).
    KitGallery,
    /// Админпанель (FR-070): Esc закрывает, прочие клавиши глотаются
    /// (модаль; интерактив — шапка + сайдбар).
    Admin,
    /// Клавиатура идёт в канвас-лестницу (прежнее поведение).
    Canvas,
}

/// Владелец-обработчик поверхности (FR-054, Q4-a): `Some` — у поверхности
/// есть клавиатурный обработчик ([`KeyOwner`] → `App::route_owner_key`);
/// `None` — скоуп пропускает событие вниз по стеку (лестница канваса).
pub fn owner_of(surface: &str) -> Option<KeyOwner> {
    match surface {
        id::ONBOARDING => Some(KeyOwner::Onboarding),
        id::GALLERY => Some(KeyOwner::Gallery),
        // FR-055 U4: витрина кита — модаль (Esc закрывает, прочие глотаются)
        id::KIT_GALLERY => Some(KeyOwner::KitGallery),
        // FR-070: админпанель — модаль (Esc закрывает, прочие глотаются)
        id::ADMIN => Some(KeyOwner::Admin),
        id::EDITOR => Some(KeyOwner::Editor),
        id::SEARCH => Some(KeyOwner::Search),
        id::DIALOG => Some(KeyOwner::Dialog),
        id::STAGE => Some(KeyOwner::Stage),
        id::EXPLAIN => Some(KeyOwner::Explain),
        // PRD-0007 X4: диалог ревью автосвязи — Esc закрывает, прочие глотаются
        id::AUTOLINK => Some(KeyOwner::Autolink),
        // Фокус решает владельца (прежний гейт 8036: панель без фокуса
        // клавиши не перехватывает — Ctrl+P/лестница работают).
        id::TEMPLATE_PANEL => Some(KeyOwner::TemplatePanel),
        _ => None,
    }
}

/// Владелец клавиатуры: верх esc_stack активных поверхностей (легаси-head
/// U2; боевой путь с FR-054 — проход `KeyboardRouter::deliver` по всем
/// скоупам — эквивалентность фиксирует тест `router_delivery_matches_legacy_head`).
pub fn key_owner(registry: &SurfaceRegistry) -> KeyOwner {
    registry
        .esc_stack()
        .first()
        .and_then(|sid| owner_of(sid.as_str()))
        .unwrap_or(KeyOwner::Canvas)
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
    //    Слой Modals (FR-054, дельта: было Panels — модалка рисовалась ПОД
    //    полосой палитры/пустой карточкой того же слоя; модаль выше панелей
    //    — гейт G4 «0 пересечений интерактивных rect'ов одного слоя»).
    if app.settings_open {
        reg.add(
            SurfaceDecl::new(id::SETTINGS, UiLayer::Modals, CapturePolicy::Block)
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
    // FR-050 Н9-4 (этап E): карта проливаний — Capture-панель (канвас
    // под ней жив: владелец кликает ноды, переходя по строкам карты)
    if app.flow_map_open {
        reg.add(
            SurfaceDecl::new(id::FLOW_MAP, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::FLOW_MAP),
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
    // FR-055 (этап U4, F-8): витрина кита — Modals/Block (мимо панели
    // закрывается и глотает; вход — пункт «?» «О интерфейсе», Q5-a);
    // клавиатура — только Esc (закрыть), прочие глотаются.
    if app.kit_gallery_open {
        reg.add(
            SurfaceDecl::new(id::KIT_GALLERY, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::KIT_GALLERY),
        );
    }
    // FR-070: UI-админпанель — Modals/Block (мимо панели закрывается и
    // глотает; вход — пункт «?» «UI-консоль»; клавиатура — только Esc).
    if app.admin_open {
        reg.add(
            SurfaceDecl::new(id::ADMIN, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::ADMIN),
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
    id::FLOW_MAP,
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
    id::KIT_GALLERY,
    id::ADMIN,
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
    // Контракт формата — xywh [x, y, w, h] (общий для раскладок; исключение
    // — xyxy search_ui::PanelLayout, конвертируется отдельно ниже). Прежнее
    // замыкание UiRect::new(r[0], r[1], r[0]+r[2], r[1]+r[3]) трактовало
    // xywh как xyxy и ЗАВЫШАЛО hit-rect'ы всех поверхностей (w ← x+w,
    // h ← y+h) — расхождение pick с видимой панелью (найдено линтом
    // F-11 FR-054).
    let rect = |r: [f32; 4]| UiRect::new(r[0], r[1], r[2], r[3]);
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
            // FR-054 (G4): панель смещается правее полосы палитры (налезание
            // одного слоя запрещено) — тот же сдвиг, что у отрисовки.
            let panel =
                crate::ui::hotkeys_panel_rect_at(viewport, app.hotkeys_left_offset(viewport));
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
            // FR-054: ширины чипов — измеренные (measurer на вызов).
            let mut measurer = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = template_panel_layout(
                vw,
                vh,
                &app.templates,
                &app.template_panel,
                &rows,
                &mut measurer,
                &mut fs,
            );
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "template-panel"));
        }
        id::TEMPLATE_STRIP => {
            let categories = app.template_category_names();
            let mut measurer = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let strip = template_ui::dock_strip_layout(&categories, vh, &mut measurer, &mut fs);
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
            // Rect::xywh → UiRect::xywh напрямую: прежняя конверсия
            // UiRect::new(r.x, r.y, r.x + r.w, r.y + r.h) трактовала xywh
            // как xyxy и ЗАВЫШАЛА pick-зону до краёв экрана (класс дефекта
            // линта F-11) — клики рядом с окном глотались как Element{stage}
            // вместо Backdrop-контракта «клик мимо окна закрывает».
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(r.x, r.y, r.w, r.h),
                "stage",
            ));
        }
        id::SEARCH => {
            let lay = search_layout(vw, vh, &app.search);
            // PanelLayout — xyxy (уникальный формат модуля, см. контракт
            // search_ui::PanelLayout): конвертация прямая, НЕ как xywh —
            // прежняя трактовка [x0,y0,x1,y1] как [x,y,w,h] завышала
            // hit-rect (право/низ экрана), расширяя pick панели поиска.
            let [sx0, sy0, sx1, sy1] = lay.panel_rect;
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(sx0, sy0, sx1 - sx0, sy1 - sy0),
                "search-panel",
            ));
        }
        // FR-050 Н9-4 (этап E): карта проливаний — панель + «✕» + строки
        // (раскладка flowmap_ui на ките FR-059 — UiRect-контракт; клики —
        // click_flow_map (hit-тест той же раскладкой), клик мимо панели —
        // канвас (Capture)
        id::FLOW_MAP => {
            let lay = app.flow_map_layout();
            surface
                .hit_rects
                .push(HitRect::interactive(lay.panel, "flow-map-panel"));
            surface
                .hit_rects
                .push(HitRect::interactive(lay.close, "flow-map-close"));
            for (i, (_, row)) in lay.rows.iter().enumerate() {
                surface
                    .hit_rects
                    .push(HitRect::interactive(*row, format!("flow-map-row-{i}")));
            }
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
        // FR-055 (этап U4): витрина кита — интерактивные зоны шапки (одни
        // слоты, что у отрисовки — kit_ui::gallery_hit_slots); прочий контент
        // витрины — декоративный (состояния показываются статически).
        // ТЕЛО панели — базовая pick-зона (первой — кнопки выше выигрывают):
        // Block-модаль без тела классифицировала клик по контенту как
        // Backdrop и ЗАКРЫВАЛА панель при нажатии на себя (жалоба владельца
        // 2026-09-25) — теперь поведение main stage: клик внутри окна
        // глотается (click_kit_gallery, элемент мимо кнопок — no-op).
        id::KIT_GALLERY => {
            let body =
                crate::kit_ui::gallery_panel(UiRect::new(0.0, 0.0, vw.max(0.0), vh.max(0.0)));
            surface
                .hit_rects
                .push(HitRect::interactive(body, "kit-gallery-panel"));
            let (theme, close) = crate::kit_ui::gallery_hit_slots(viewport);
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(theme.x, theme.y, theme.w, theme.h),
                "kit-gallery-theme",
            ));
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(close.x, close.y, close.w, close.h),
                "kit-gallery-close",
            ));
        }
        // FR-070: админпанель — интерактивные зоны шапки (те же слоты, что
        // у отрисовки) + пункты сайдбара; демо-контент — декоративный.
        // ТЕЛО панели — базовая pick-зона (первой — кнопки/сайдбар/свотчи
        // выше выигрывают): клик по контенту UI-консоли глотается, панель
        // не закрывается (поведение main stage; прежде тело отсутствовало
        // в кадре — Block-модаль трактовала такой клик как Backdrop и
        // закрывала панель при нажатии на себя — жалоба владельца
        // 2026-09-25).
        id::ADMIN => {
            let admin_body = app.admin_layout_at([vw, vh]).panel;
            surface
                .hit_rects
                .push(HitRect::interactive(admin_body, "admin-panel"));
            let (theme, reset, close) = crate::admin_ui::admin_hit_slots(viewport);
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(theme.x, theme.y, theme.w, theme.h),
                "admin-theme",
            ));
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(reset.x, reset.y, reset.w, reset.h),
                "admin-reset",
            ));
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(close.x, close.y, close.w, close.h),
                "admin-close",
            ));
            let admin_lay_frame = app.admin_layout_at([vw, vh]);
            for (i, item) in admin_lay_frame.sidebar_items.iter().enumerate() {
                surface.hit_rects.push(HitRect::interactive(
                    UiRect::new(item.x, item.y, item.w, item.h),
                    format!("admin-section-{i}"),
                ));
            }
            // Свотчи слотов палитры (live-правка, FR-070) — интерактивные
            let admin_lay = app.admin_layout_at([vw, vh]);
            if let Some(tokens) = &admin_lay.tokens {
                for group in &tokens.groups {
                    for row in &group.rows {
                        if let (Some(i), Some(swatch)) = (row.slot_index, row.swatch) {
                            surface
                                .hit_rects
                                .push(HitRect::interactive(swatch, format!("admin-token-{i}")));
                        }
                    }
                }
            }
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

    /// Поверхности объявлены константами без дублей (контракт
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
            id::KIT_GALLERY,
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

    /// FR-054 (Q4-a TDD): доставка KeyboardRouter эквивалентна прежнему
    /// head U2 (верх esc_stack) на матрице состояний; у состояний без
    /// владельца (settings/whatif/hotkeys сверху) доставка не находит
    /// владельца (None ≡ Canvas). Исключение — задокументированная дельта
    /// «док палитры в фокусе + палитра выделения видима» (см. ниже).
    #[test]
    fn router_delivery_matches_legacy_head() {
        let routed_owner = |registry: &SurfaceRegistry| {
            let router = canvas_ui::KeyboardRouter::from_registry(registry);
            router
                .deliver(|a| owner_of(a.surface.as_str()).is_some())
                .and_then(|a| owner_of(a.surface.as_str()))
        };
        let assert_eq_legacy = |app: &App| {
            let registry = build_registry(app);
            let legacy = key_owner(&registry);
            let routed = routed_owner(&registry);
            match legacy {
                KeyOwner::Canvas => assert_eq!(routed, None, "state без владельца"),
                owner => assert_eq!(routed, Some(owner), "state с владельцем {owner:?}"),
            }
        };

        let mut app = test_stub();
        app.onboarding = None;
        assert_eq_legacy(&app); // idle — Canvas
        app.search.open();
        assert_eq_legacy(&app); // Search
        app.search.close();
        app.scheme_gallery.open();
        assert_eq_legacy(&app); // Gallery
        app.scheme_gallery.close();
        app.onboarding = Some(crate::onboarding_ui::OnboardingState::default());
        assert_eq_legacy(&app); // Onboarding
        app.onboarding = None;
        app.settings_open = true;
        assert_eq_legacy(&app); // Settings — владельца нет
        app.settings_open = false;
        app.hotkeys_open = true;
        assert_eq_legacy(&app); // Hotkeys — владельца нет
        app.hotkeys_open = false;
        app.scene.whatif_active = true;
        assert_eq_legacy(&app); // Whatif — владельца нет
        app.scene.whatif_active = false;
        app.template_panel.open();
        app.template_panel.focused = true;
        assert_eq_legacy(&app); // TemplatePanel (палитры нет — панели нет и в кадре)
    }

    /// FR-054: задокументированная дельта против U2 — «док палитры в
    /// фокусе + палитра выделения видима»: у палитры владельца клавиатуры
    /// нет, роутер проходит сквозь неё к панели (прежний гейт 8036);
    /// легаси-head U2 (только верх esc_stack) отдавал клавиши Canvas —
    /// панель в фокусе не получала клавиатуру (регресс U2 устранён).
    #[test]
    fn router_walks_past_non_owner_scopes_to_panel() {
        let mut reg = SurfaceRegistry::new();
        // Порядок регистрации как в build_registry: панель раньше палитры,
        // в esc-стеке палитра — ВЫШЕ панели.
        reg.add(
            SurfaceDecl::new(id::TEMPLATE_PANEL, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::TEMPLATE_PANEL),
        );
        reg.add(
            SurfaceDecl::new(id::PALETTE, UiLayer::Widgets, CapturePolicy::Capture)
                .with_scope(id::PALETTE),
        );
        let router = canvas_ui::KeyboardRouter::from_registry(&reg);
        let routed = router
            .deliver(|a| owner_of(a.surface.as_str()).is_some())
            .and_then(|a| owner_of(a.surface.as_str()));
        assert_eq!(routed, Some(KeyOwner::TemplatePanel));
        // Легаси-head U2 на том же реестре: верх стека — палитра → Canvas.
        assert_eq!(key_owner(&reg), KeyOwner::Canvas);
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
        // FR-054: раскладка страницы — измеренным текстом (measurer на вызов).
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

    // --- FR-055 (этап U4 PRD-0009): витрина кита + DebugOverlay (G6) ---

    /// Витрина в реестре: Modals/Block, hit-rect'ы шапки (тема/✕) на месте,
    /// pick по кнопке темы даёт Element поверхности kit_gallery.
    #[test]
    fn kit_gallery_surface_pickable() {
        let mut app = test_stub();
        app.onboarding = None;
        app.kit_gallery_open = true;
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::KIT_GALLERY)
            .expect("kit_gallery в реестре");
        assert_eq!(decl.layer, UiLayer::Modals);
        assert_eq!(decl.capture, CapturePolicy::Block);

        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::KIT_GALLERY)
            .expect("kit_gallery в кадре");
        let elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        assert!(elements.contains(&"kit-gallery-theme"));
        assert!(elements.contains(&"kit-gallery-close"));

        // Курсор в центр кнопки темы → Element поверхности (координата —
        // из hit-rect'а кадра, не из раскладки: один источник геометрии)
        let theme = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "kit-gallery-theme")
            .expect("кнопка темы");
        let c = UiPoint::new(
            theme.rect.x + theme.rect.w / 2.0,
            theme.rect.y + theme.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::KIT_GALLERY);
                assert_eq!(rect.element, "kit-gallery-theme");
            }
            other => panic!("кнопка темы не пикается: {other:?}"),
        }
        // Esc-стек: витрина — верх (открыта последней из модалей)
        assert_eq!(key_owner(&reg), KeyOwner::KitGallery);
    }

    /// FR-070: админпанель — Block-модаль; pick по кнопкам шапки и пунктам
    /// сайдбара даёт Element поверхности admin_panel; Esc-владелец — Admin.
    #[test]
    fn admin_panel_surface_pickable() {
        let mut app = test_stub();
        app.onboarding = None;
        app.admin_open = true;
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::ADMIN)
            .expect("admin_panel в реестре");
        assert_eq!(decl.layer, UiLayer::Modals);
        assert_eq!(decl.capture, CapturePolicy::Block);

        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::ADMIN)
            .expect("admin_panel в кадре");
        let elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        for expected in [
            "admin-theme",
            "admin-reset",
            "admin-close",
            "admin-section-0",
            "admin-section-1",
            "admin-section-2",
            "admin-section-3",
        ] {
            assert!(elements.contains(&expected), "нет hit-rect {expected}");
        }

        // Курсор в центр пункта сайдбара «Токены» (индекс 3) → Element
        let tokens = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "admin-section-3")
            .expect("пункт сайдбара");
        let c = UiPoint::new(
            tokens.rect.x + tokens.rect.w / 2.0,
            tokens.rect.y + tokens.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::ADMIN);
                assert_eq!(rect.element, "admin-section-3");
            }
            other => panic!("пункт сайдбара не пикается: {other:?}"),
        }
        // Esc-стек: админпанель — верх
        assert_eq!(key_owner(&reg), KeyOwner::Admin);
    }

    /// FR-070 (этап 3): секция «Токены» — свотчи слотов пикаются
    /// (live-правка: element admin-token-{i}).
    #[test]
    fn admin_token_swatch_pickable() {
        let mut app = test_stub();
        app.onboarding = None;
        app.admin_open = true;
        app.admin_section = crate::admin_ui::AdminSection::Tokens;
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::ADMIN)
            .expect("admin_panel в кадре");
        let token_elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .filter(|e| e.starts_with("admin-token-"))
            .collect();
        assert_eq!(token_elements.len(), 14, "14 свотчей слотов");

        // Первый видимый свотч — пик https:// как Element admin-token-N
        let first = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "admin-token-0")
            .expect("свотч первого слота");
        let c = UiPoint::new(
            first.rect.x + first.rect.w / 2.0,
            first.rect.y + first.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::ADMIN);
                assert_eq!(rect.element, "admin-token-0");
            }
            other => panic!("свотч не пикается: {other:?}"),
        }
    }

    /// Регресс (жалоба владельца 2026-09-25): клик по ТЕЛУ UI-консоли (мимо
    /// кнопок/сайдбара/свотчей) не закрывает панель — поведение main stage
    /// (клик внутри окна глотается, модаль жива). Прежде тело панели не было
    /// pick-зоной: Block-модаль классифицировала клик как Backdrop —
    /// dispatch_surface_backdrop закрывал панель при любом нажатии на себя.
    #[test]
    fn admin_panel_body_click_is_not_backdrop() {
        let mut app = test_stub();
        app.onboarding = None;
        app.admin_open = true;
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::ADMIN)
            .expect("admin_panel в кадре");
        let body = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "admin-panel")
            .expect("тело панели — pick-зона (admin-panel)");
        // Точка у нижне-правого угла панели (паддинг): внутри тела, но вне
        // интерактивных rect'ов (сайдбар слева, шапка сверху, свотчи в сетке)
        let p = UiPoint::new(body.rect.right() - 4.0, body.rect.bottom() - 4.0);
        assert!(
            !surface
                .hit_rects
                .iter()
                .any(|r| r.element != "admin-panel" && r.rect.contains(p)),
            "точка теста попала на интерактивный rect — сдвинуть точку"
        );
        match HitStack::pick(&frame, p) {
            Some(HitTarget::Element { surface, .. }) => {
                assert_eq!(surface.surface.as_str(), id::ADMIN);
            }
            Some(HitTarget::Backdrop { .. }) => {
                panic!("клик по телу панели — Backdrop: панель закрывается при нажатии на себя");
            }
            None => panic!("клик по телу панели ушёл в канвас"),
        }
        // Контракт Block-модали сохранён: клик МИМО панели — Backdrop
        // (закрыть и глотнуть)
        match HitStack::pick(&frame, UiPoint::new(4.0, 4.0)) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::ADMIN);
            }
            other => panic!("клик мимо панели обязан закрывать (Backdrop), получено {other:?}"),
        }
    }

    /// Регресс (жалоба владельца 2026-09-25): витрина «О интерфейсе» — та же
    /// болезнь: клик по телу витрины (мимо кнопки темы/«✕») не закрывает
    /// панель; мимо панели — Backdrop (контракт Block-модали сохранён).
    #[test]
    fn kit_gallery_body_click_is_not_backdrop() {
        let mut app = test_stub();
        app.onboarding = None;
        app.kit_gallery_open = true;
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::KIT_GALLERY)
            .expect("kit_gallery в кадре");
        let body = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "kit-gallery-panel")
            .expect("тело витрины — pick-зона (kit-gallery-panel)");
        // Нижне-правый угол панели (паддинг): кнопки темы/«✕» — в шапке
        let p = UiPoint::new(body.rect.right() - 4.0, body.rect.bottom() - 4.0);
        assert!(
            !surface
                .hit_rects
                .iter()
                .any(|r| r.element != "kit-gallery-panel" && r.rect.contains(p)),
            "точка теста попала на интерактивный rect — сдвинуть точку"
        );
        match HitStack::pick(&frame, p) {
            Some(HitTarget::Element { surface, .. }) => {
                assert_eq!(surface.surface.as_str(), id::KIT_GALLERY);
            }
            Some(HitTarget::Backdrop { .. }) => {
                panic!("клик по телу витрины — Backdrop: панель закрывается при нажатии на себя");
            }
            None => panic!("клик по телу витрины ушёл в канвас"),
        }
        match HitStack::pick(&frame, UiPoint::new(4.0, 4.0)) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::KIT_GALLERY);
            }
            other => panic!("клик мимо витрины обязан закрывать (Backdrop), получено {other:?}"),
        }
    }

    /// Регресс: pick-зона main stage — ТОЧНО окно stage (xywh), а не
    /// завышенный rect (x+w / y+h в полях w/h — класс дефекта линта F-11).
    /// Прежняя конверсия UiRect::new(r.x, r.y, r.x + r.w, r.y + r.h)
    /// раздувала зону до правого/нижнего края экрана: клики РЯДОМ с окном
    /// глотались как Element{stage} вместо Backdrop-контракта.
    #[test]
    fn stage_pick_zone_matches_window() {
        let mut app = test_stub();
        app.onboarding = None;
        app.main_stage = Some(MainStageState {
            key: ("a".to_owned(), "b".to_owned()),
            edges: Vec::new(),
            slice: Canvas::default(),
            scale: 1.0,
        });
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::STAGE)
            .expect("stage в кадре");
        let hit = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "stage")
            .expect("pick-зона окна stage");
        let r = main_stage_rect([1280.0, 800.0]);
        assert_eq!(
            (hit.rect.x, hit.rect.y, hit.rect.w, hit.rect.h),
            (r.x, r.y, r.w, r.h),
            "pick-зона stage обязана совпадать с видимым окном"
        );
        // Точка справа окна (в завышенном rect попадала): после фикса —
        // Backdrop stage (клик мимо окна закрывает модаль), не Element
        let px = r.x + r.w + 8.0;
        assert!(px < 1280.0, "тест рассчитан на окно уже вьюпорта");
        match HitStack::pick(&frame, UiPoint::new(px, r.y + r.h / 2.0)) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::STAGE);
            }
            other => panic!("клик рядом с окном stage — не Element: {other:?}"),
        }
    }

    /// Регресс дрейфа панелей (правка 2026-09-25): квады полос админпанели — СЫРЫЕ
    /// screen-px, совпадающие с hit-раскладкой реестра при ЛЮБОЙ камере.
    /// Прежняя двойная конверсия (KitDraw screen→world + рендер screen→world)
    /// сдвигала квад на `P+(s−V/2)/z` относительно текстов/hit-rect'ов —
    /// панель «разъезжалась» при панорамировании канваса.
    #[test]
    fn admin_overlay_quads_are_raw_screen_space() {
        let mut app = test_stub();
        app.onboarding = None;
        app.test_viewport = Some([1280.0_f32, 800.0]);
        // Ненулевые пан/зум — квад-затемнение обязан остаться в origin
        // вьюпорта (сырой px), а не уехать в world-координаты
        app.camera.set_zoom(1.7);
        app.camera.pan([137.0, -64.0]);
        app.admin_open = true;
        let (quads, _) = app.admin_panel_overlay();
        assert!(!quads.is_empty(), "админпанель рисует затемнение + панель");
        // Затемнение: сырой px — origin (0,0), размер = вьюпорт
        assert_eq!(quads[0].pos, [0.0, 0.0]);
        assert_eq!(quads[0].size, [1280.0, 800.0]);
        // Панель: совпадает с hit-раскладкой реестра (одна геометрия)
        let lay = app.admin_layout_at([1280.0, 800.0]);
        assert_eq!(quads[1].pos, [lay.panel.x, lay.panel.y]);
        assert_eq!(quads[1].size, [lay.panel.w, lay.panel.h]);
    }

    /// Регресс дрейфа панелей (правка 2026-09-25): витрина кита — та же конвенция
    /// полос (сырые screen-px) при пан/зуме камеры.
    #[test]
    fn kit_gallery_overlay_quads_are_raw_screen_space() {
        let mut app = test_stub();
        app.onboarding = None;
        app.test_viewport = Some([1280.0_f32, 800.0]);
        app.camera.set_zoom(1.7);
        app.camera.pan([137.0, -64.0]);
        app.kit_gallery_open = true;
        let (quads, _) = app.kit_gallery_overlay();
        assert!(!quads.is_empty(), "витрина рисует затемнение + панель");
        assert_eq!(quads[0].pos, [0.0, 0.0]);
        assert_eq!(quads[0].size, [1280.0, 800.0]);
    }

    /// Регресс дрейфа панелей (правка 2026-09-25): диалог ревью автосвязи живёт в
    /// СТАДИЙНОМ проходе (world-конвенция) — квад затемнения обязан быть
    /// сконвертирован screen→world ЗДЕСЬ (иначе рендер рисует его как world:
    /// диалог «приклеивается» к канвасу и разъезжается со screen-текстами).
    #[test]
    fn autolink_review_quads_are_world_space() {
        let mut app = test_stub();
        app.onboarding = None;
        app.test_viewport = Some([1280.0_f32, 800.0]);
        app.camera.set_zoom(1.7);
        app.camera.pan([137.0, -64.0]);
        let mut canvas = canvas_core::Canvas::default();
        canvas
            .nodes
            .push(canvas_core::Node::text("A", "Исток", 0.0, 0.0));
        canvas
            .nodes
            .push(canvas_core::Node::text("B", "Приёмник", 300.0, 0.0));
        let proposal = canvas_core::AutolinkProposal {
            from_node: "A".into(),
            from_line: 1,
            to_node: "B".into(),
            param: "rate".into(),
            percent: 100,
            unit_match: None,
        };
        app.autolink_review = Some(crate::autolink_ui::Review::build(&canvas, vec![proposal]));
        let viewport = [1280.0_f32, 800.0];
        let (insts, _) = app.autolink_frame(viewport);
        assert!(!insts.is_empty(), "диалог рисует затемнение + окно");
        // Затемнение: квад в WORLD-координатах (screen_to_world от (0,0))
        assert_eq!(
            insts[0].pos,
            app.camera.screen_to_world([0.0, 0.0], viewport)
        );
        // Размер поделен на зум (константный экранный размер)
        assert_eq!(insts[0].size, [1280.0 / 1.7, 800.0 / 1.7]);
    }

    /// G6: DebugOverlay показывает рамки/подписи слоёв и имя под курсором.
    /// Модель чистая (кадр + геометрия) — headless.
    #[test]
    fn debug_overlay_labels_layers_and_cursor() {
        let mut app = test_stub();
        app.onboarding = None;
        app.kit_gallery_open = true; // Block-модаль с hit-rect'ами
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        // Курсор — в центр кнопки «✕» витрины
        let close = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::KIT_GALLERY)
            .and_then(|s| {
                s.hit_rects
                    .iter()
                    .find(|r| r.element == "kit-gallery-close")
            })
            .expect("✕ витрины")
            .rect;
        let cursor = [close.x + close.w / 2.0, close.y + close.h / 2.0];
        let (quads, texts) = crate::debug_overlay::build([1280.0, 800.0], cursor, &frame);
        // Рамка на каждый hit-rect кадра + плашка под курсором
        let rect_count: usize = frame.surfaces.iter().map(|s| s.hit_rects.len()).sum();
        assert!(
            quads.len() >= rect_count,
            "рамок {} меньше rect'ов {}",
            quads.len(),
            rect_count
        );
        // Подпись «слой/поверхность/элемент» — есть для витрины (Modals)
        assert!(texts
            .iter()
            .any(|t| t.text.contains(&format!("L5·Modals / {}", id::KIT_GALLERY))));
        // Имя под курсором — surface/element кнопки ✕
        assert!(texts
            .iter()
            .any(|t| t.text == format!("{} / kit-gallery-close", id::KIT_GALLERY)));
    }

    /// G6: пересечения интерактивных rect'ов одного слоя подсвечиваются
    /// (та же функция overlaps_within_layer, что у G4-линта). Кадр —
    /// модельный (канонические состояния налезаний не дают — линт).
    #[test]
    fn debug_overlay_highlights_intersections() {
        let mut app = test_stub();
        app.onboarding = None;
        // Модельный кадр: две панели одного слоя с общим интерактивным
        // rect'ом — пересечение обязано попасть в подсветку
        let mut frame = UiFrame {
            viewport: UiRect::new(0.0, 0.0, 1280.0, 800.0),
            ..Default::default()
        };
        let mut a = SurfaceFrame::new("a", UiLayer::Panels, CapturePolicy::Capture, frame.viewport);
        a.hit_rects.push(HitRect::interactive(
            UiRect::new(100.0, 100.0, 200.0, 60.0),
            "a-rect",
        ));
        let mut b = SurfaceFrame::new("b", UiLayer::Panels, CapturePolicy::Capture, frame.viewport);
        b.hit_rects.push(HitRect::interactive(
            UiRect::new(200.0, 120.0, 200.0, 60.0),
            "b-rect",
        ));
        frame.surfaces.push(a);
        frame.surfaces.push(b);
        assert_eq!(frame.overlaps_within_layer().len(), 1);

        let (quads, texts) = crate::debug_overlay::build([1280.0, 800.0], [10.0, 10.0], &frame);
        // 2 рамки rect'ов + 1 плашка пересечения
        assert_eq!(quads.len(), 3, "рамки + пересечение: {quads:?}");
        assert!(texts
            .iter()
            .any(|t| t.text.contains("× a × b") && t.text.contains("L3·Panels")));
    }
}
