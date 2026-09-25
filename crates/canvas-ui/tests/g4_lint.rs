//! FR-068 волна W0 (docs/change-requests/fr-068-ui-refactoring-long-term.md,
//! §«Волна W0 — UI hygiene», строка «Расширение G4-линта»): layout-линт на
//! чистой геометрии `canvas-ui` — headless, без GPU (паттерн PRD-0009 §9.1;
//! ADR-0015: «проверка — layout-линт + snapshot-тесты + perf-бюджет»).
//!
//! **Матрица: 5 canonical сцен × 3 окна (1280×800 / 1024×640 / 800×560) ×
//! 2 языка (RU/EN) = 30 прогонов.**
//!
//! Модель сцены: каждая сцена = [`SurfaceRegistry`] (декларации поверхностей:
//! слой/capture/деградация/scope) + [`UiFrame`] (hit-rect'ы поверхностей:
//! интерактивные + decoration) + дерево элементов
//! `Vec<(имя, rect, parent|None, layer)>`; rect'ы вычислены layout-примитивами
//! ([`Row`]/[`Column`]/`pad`) и kit-функциями (`panel_rect`, `modal`,
//! `dropdown_menu`, `text_field`, `switch`, `chip_size`, `button_size`,
//! `list_rows`, `scroll_bar`, `toast_area`, `card`, `icon_button_rect`,
//! `button_layout`) от вьюпорта окна. Язык влияет на лейблы и через
//! `TextMeasurer` (реальный шейпинг cosmic-text) — на ширины элементов:
//! все 30 прогонов имеют разную геометрию.
//!
//! Сцены — эталон правильной вёрстки: контент подобран так, что помещается
//! в самое малое окно 800×560; деградации выражены именованными политиками
//! (SqueezeTail what-if бара по смыслу FR-017/CR-015, hide-политика
//! реестра, число видимых строк списка), а не молчаливыми срезами.
//! Вырожденные rect'ы (сжатый SqueezeTail-хвост) невидимы и в сцену не
//! входят (PRD-0009 F-11 c: «hide-политики вместо вырожденных rect'ов»).
//!
//! Проверки на КАЖДОМ прогоне (ADR-0015 G4+; PRD-0009 F-11):
//! 1. каждый видимый элемент пересекает своего parent
//!    (`UiRect::intersection(rect, parent).is_some()`); корень — viewport;
//! 2. каждый элемент полосы L4+ (Popups и выше по `UiLayer::DRAW_ORDER`)
//!    пересекает viewport;
//! 3. `UiFrame::overlaps_within_layer()` пуст — интерактивные rect'ы разных
//!    поверхностей одной полосы не пересекаются;
//! 4. каждый интерактивный hit-rect целиком во вьюпорте (конвенция
//!    существующего линта `canvas-app/src/app/ui_layout_lint.rs`: 0 выходов,
//!    eps 0.01).
//!
//! Пункт (3) FR-068 — grep-аудит silent-clips (`take(`/`break`/`truncate`
//! в мигрированном коде) — выполняется вручную по kit.rs, не здесь
//! (см. worklog сессии FR-068 W0, Task 2-c).
//!
//! # Backend'ы вёрстки (FR-068 W1, Контракт-3 «G4-линты × N backend'ов»)
//!
//! Та же матрица прогоняется через ВЫБРАННЫЙ backend вёрстки
//! ([`SceneBackend`]): по умолчанию — `NativeBackend` (числа существующих
//! 6 тестов неизменны), под фичей `taffy` — дополнительные тесты строят
//! те же 5 сцен × 3 окна × 2 языка через `Row/Column::lay_out_with`
//! (&TaffyBackend) с ТЕМИ ЖЕ проверками G4. kit-функции (panel_rect,
//! dropdown_menu, list_rows…) идут мимо выбора — это rect-математика,
//! не flex; через backend идут только примитивы Row/Column (5 Row-мест
//! + 2 Column-места в сценах).

use std::collections::HashSet;

use canvas_ui::capture::CapturePolicy;
use canvas_ui::frame::{HitRect, UiFrame};
use canvas_ui::geometry::{EdgeInsets, UiRect, UiVec2};
use canvas_ui::kit::{
    button_layout, button_size, card, chip_size, dropdown_menu, icon_button_rect, list_rows, modal,
    modal_style, panel_content, panel_rect, panel_style, scroll_bar, switch, text_field,
    toast_area, KitPalette, KitState, ScrollState, TextFieldModel, LIST_ROW_GAP, LIST_ROW_H,
    SWITCH_H, SWITCH_W, TEXT_FIELD_HEIGHT, TEXT_FIELD_MIN_W,
};
use canvas_ui::layer::UiLayer;
use canvas_ui::layout::{
    pad, Child, Column, CrossAlign, HAlign, MainAlign, Row, RowPolicy, VAlign,
};
use canvas_ui::measure::TextMeasurer;
use canvas_ui::registry::{DegradationPolicy, SurfaceDecl, SurfaceRegistry};

/// Выбор backend'а вёрстки сцен (FR-068 W1, Контракт-3 «G4-линты × N
/// backend'ов»): один и тот же код сцен строится через любой backend.
/// `Native` — default (zero-dep инвариант G7; существующие тесты и их
/// числа неизменны); `Taffy` — только под фичей `taffy` (opt-in,
/// ADR-0014) — TaffyBackend в default-сборке не существует.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SceneBackend {
    Native,
    #[cfg(feature = "taffy")]
    Taffy,
}

/// Семейство UI-шрифта — паритет с unit-тестами кита (`kit.rs`).
const FAMILY: &str = "Noto Sans Display";
/// Кегль UI-текста (лейблы чипов/кнопок/строк).
const UI_FONT: f32 = 13.0;
/// Окна G4 (PRD-0009 AC-3.1; doc `DegradationPolicy`).
const VIEWPORTS: [[f32; 2]; 3] = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];
/// 5 canonical сцен FR-068 W0 (индекс = номер в постановке задачи).
const SCENES: [&str; 5] = [
    "main-canvas-whatif",
    "palette-dropdown",
    "explain-modal",
    "search-overlay",
    "settings-panel",
];
/// Прогонов на сцену: 3 окна × 2 языка.
const RUNS_PER_SCENE: usize = VIEWPORTS.len() * Lang::ALL.len();

// --- Языки: лейблы сцен (RU/EN) ---------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang {
    Ru,
    En,
}

impl Lang {
    const ALL: [Lang; 2] = [Lang::Ru, Lang::En];

    fn tag(self) -> &'static str {
        match self {
            Lang::Ru => "RU",
            Lang::En => "EN",
        }
    }

    fn labels(self) -> &'static Labels {
        match self {
            Lang::Ru => &RU,
            Lang::En => &EN,
        }
    }
}

/// Лейблы canonical сцен. Язык влияет на геометрию через `TextMeasurer`:
/// RU-лейблы шире английских → вёрстка каждого прогона реально различается.
struct Labels {
    // сцена 1: main canvas + what-if бар
    whatif_title: &'static str,
    scenario_chips: [&'static str; 8],
    reset: &'static str,
    zoom_chip: &'static str,
    card_title: &'static str,
    card_field_text: &'static str,
    // сцена 2: palette dropdown
    palette_cats: [&'static str; 4],
    templates: [&'static str; 6],
    // сцена 3: explain modal
    explain_title: &'static str,
    explain_body: &'static str,
    explain_close: &'static str,
    explain_show: &'static str,
    // сцена 4: search overlay
    search_placeholder: &'static str,
    search_btn: &'static str,
    results: [&'static str; 10],
    // сцена 5: settings panel
    settings_title: &'static str,
    settings_rows: [&'static str; 5],
    settings_done: &'static str,
}

const RU: Labels = Labels {
    whatif_title: "Что-если",
    scenario_chips: [
        "База",
        "+20% спрос",
        "−10% цена",
        "×2 объём",
        "пессимист",
        "оптимист",
        "шок спроса",
        "дефолт",
    ],
    reset: "Сброс",
    zoom_chip: "100%",
    card_title: "Выручка узла",
    card_field_text: "выручка = 1200",
    palette_cats: ["Фигуры", "Стрелки", "Текст", "Шаблоны"],
    templates: [
        "Карточка процесса",
        "Сравнение",
        "Поток",
        "Диаграмма",
        "Матрица",
        "Отчёт",
    ],
    explain_title: "Объяснение линии",
    explain_body: "Показывает, откуда приходят значения в выбранную ячейку расчёта.",
    explain_close: "Закрыть",
    explain_show: "Показать цепочку",
    search_placeholder: "Поиск по заметкам…",
    search_btn: "Найти",
    results: [
        "Заметка о расчёте",
        "Модель мощности",
        "План запуска",
        "Ретро недели",
        "Идеи продуктов",
        "Смета ремонта",
        "Контакты поставщиков",
        "Договор поставки",
        "Отчёт за март",
        "Презентация Q2",
    ],
    settings_title: "Настройки",
    settings_rows: [
        "Показывать сетку",
        "Автосохранение",
        "Магниты направляющих",
        "Тени карточек",
        "Счётчик кадров",
    ],
    settings_done: "Готово",
};

const EN: Labels = Labels {
    whatif_title: "What-if",
    scenario_chips: [
        "Base",
        "+20% demand",
        "−10% price",
        "×2 volume",
        "pessimistic",
        "optimistic",
        "demand shock",
        "default",
    ],
    reset: "Reset",
    zoom_chip: "100%",
    card_title: "Node revenue",
    card_field_text: "revenue = 1200",
    palette_cats: ["Shapes", "Arrows", "Text", "Templates"],
    templates: [
        "Process card",
        "Compare",
        "Flow",
        "Diagram",
        "Matrix",
        "Report",
    ],
    explain_title: "Lineage",
    explain_body: "Shows where the selected cell value comes from.",
    explain_close: "Close",
    explain_show: "Show chain",
    search_placeholder: "Search notes…",
    search_btn: "Find",
    results: [
        "Calc note",
        "Capacity model",
        "Launch plan",
        "Week retro",
        "Product ideas",
        "Renovation budget",
        "Supplier contacts",
        "Supply contract",
        "March report",
        "Q2 deck",
    ],
    settings_title: "Settings",
    settings_rows: [
        "Show grid",
        "Autosave",
        "Guide snapping",
        "Card shadows",
        "Frame counter",
    ],
    settings_done: "Done",
};

// --- Модель сцены ------------------------------------------------------------

/// Элемент дерева сцены: имя, rect, parent (`None` — корень = viewport),
/// полоса слоя, интерактивность (участвует в pick).
struct Elem {
    surface: &'static str,
    name: String,
    rect: UiRect,
    parent: Option<String>,
    layer: UiLayer,
    interactive: bool,
}

/// Сцена под сборкой: реестр поверхностей + дерево элементов.
struct Scene {
    name: &'static str,
    registry: SurfaceRegistry,
    elements: Vec<Elem>,
}

impl Scene {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            registry: SurfaceRegistry::new(),
            elements: Vec::new(),
        }
    }

    fn surface(&mut self, id: &'static str, layer: UiLayer, capture: CapturePolicy) {
        self.registry.add(SurfaceDecl::new(id, layer, capture));
    }

    fn surface_scoped(
        &mut self,
        id: &'static str,
        layer: UiLayer,
        capture: CapturePolicy,
        scope: &'static str,
    ) {
        self.registry
            .add(SurfaceDecl::new(id, layer, capture).with_scope(scope));
    }

    fn surface_hidden_below(
        &mut self,
        id: &'static str,
        layer: UiLayer,
        capture: CapturePolicy,
        min_width: f32,
        min_height: f32,
    ) {
        self.registry
            .add(SurfaceDecl::new(id, layer, capture).with_degradation(
                DegradationPolicy::HideBelow {
                    min_width,
                    min_height,
                },
            ));
    }

    /// Добавить видимый элемент. Вырожденные rect'ы (нулевая ширина/высота —
    /// например SqueezeTail-хвост what-if бара) невидимы и не пикаются —
    /// в сцену не входят (PRD-0009 F-11 c).
    fn elem(
        &mut self,
        surface: &'static str,
        layer: UiLayer,
        name: String,
        rect: UiRect,
        parent: Option<String>,
        interactive: bool,
    ) {
        if rect.is_empty() {
            return;
        }
        self.elements.push(Elem {
            surface,
            name,
            rect,
            parent,
            layer,
            interactive,
        });
    }
}

/// Собранная сцена: кадр (поверхности + hit-rect'ы) и дерево элементов.
struct Built {
    scene: &'static str,
    frame: UiFrame,
    elements: Vec<Elem>,
}

fn finish(scene: Scene, viewport: UiRect) -> Built {
    let mut frame = UiFrame::from_registry(&scene.registry, viewport);
    for el in &scene.elements {
        let sf = frame
            .surfaces
            .iter_mut()
            .find(|s| s.surface.as_str() == el.surface)
            .unwrap_or_else(|| {
                panic!(
                    "{}: элемент «{}» ссылается на незарегистрированную поверхность «{}»",
                    scene.name, el.name, el.surface
                )
            });
        assert_eq!(
            sf.layer, el.layer,
            "{}: слой элемента «{}» не совпадает со слоем поверхности «{}»",
            scene.name, el.name, el.surface
        );
        sf.hit_rects.push(if el.interactive {
            HitRect::interactive(el.rect, el.name.clone())
        } else {
            HitRect::decoration(el.rect, el.name.clone())
        });
    }
    Built {
        scene: scene.name,
        frame,
        elements: scene.elements,
    }
}

// --- Матрица прогонов ---------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Combo {
    scene: usize,
    vp: [f32; 2],
    lang: Lang,
}

/// Полная матрица: 5 сцен × 3 окна × 2 языка = 30 прогонов (порядок
/// детерминирован: сцена → окно → язык).
fn matrix() -> Vec<Combo> {
    let mut combos = Vec::with_capacity(SCENES.len() * VIEWPORTS.len() * Lang::ALL.len());
    for scene in 0..SCENES.len() {
        for vp in VIEWPORTS {
            for &lang in &Lang::ALL {
                combos.push(Combo { scene, vp, lang });
            }
        }
    }
    combos
}

// --- Контекст сборки (замер текста + палитра-двойка) --------------------------

struct Ctx {
    m: TextMeasurer,
    fs: cosmic_text::FontSystem,
    palette: KitPalette,
    /// Backend примитивов Row/Column при сборке сцен (kit-функции —
    /// всегда native). Default — [`SceneBackend::Native`] (§Контракт-1:
    /// поведение существующих прогонов байт-в-байт прежнее).
    backend: SceneBackend,
}

impl Ctx {
    fn new() -> Self {
        Self {
            m: TextMeasurer::new(),
            fs: cosmic_text::FontSystem::new(),
            palette: palette_stub(),
            backend: SceneBackend::Native,
        }
    }

    /// `Row::lay_out` через backend сцены (Контракт-3 FR-068): все
    /// Row-места сцен идут через этот хелпер — смена backend'а
    /// локализована в одном match.
    fn lay_out_row(&self, row: Row, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        match self.backend {
            SceneBackend::Native => row.lay_out(slot, items),
            #[cfg(feature = "taffy")]
            SceneBackend::Taffy => row.lay_out_with(&canvas_ui::layout::TaffyBackend, slot, items),
        }
    }

    /// `Column::lay_out` через backend сцены (симметрично [`Ctx::lay_out_row`]).
    fn lay_out_column(&self, column: Column, slot: UiRect, items: &[Child]) -> Vec<UiRect> {
        match self.backend {
            SceneBackend::Native => column.lay_out(slot, items),
            #[cfg(feature = "taffy")]
            SceneBackend::Taffy => {
                column.lay_out_with(&canvas_ui::layout::TaffyBackend, slot, items)
            }
        }
    }
}

/// Палитра-двойка (геометрия кита от цветов не зависит — контракт F-8
/// «цвета — только слоты»; значения слотов произвольны).
fn palette_stub() -> KitPalette {
    KitPalette {
        panel_fill: [0.1, 0.1, 0.1, 1.0],
        panel_border: [0.2, 0.2, 0.2, 1.0],
        control_fill: [0.3, 0.3, 0.3, 1.0],
        control_border: [0.4, 0.4, 0.4, 1.0],
        control_primary: [0.5, 0.5, 0.5, 1.0],
        control_danger: [0.6, 0.6, 0.6, 1.0],
        hover_fill: [0.7, 0.7, 0.7, 1.0],
        primary_hover_fill: [0.75, 0.75, 0.75, 1.0],
        selected_fill: [0.8, 0.8, 0.8, 1.0],
        text: [0.9, 0.9, 0.9, 1.0],
        text_title: [0.91, 0.91, 0.91, 1.0],
        text_muted: [0.92, 0.92, 0.92, 1.0],
        disabled_text: [0.93, 0.93, 0.93, 1.0],
        accent: [0.94, 0.94, 0.94, 1.0],
    }
}

fn dbg_rect(r: UiRect) -> String {
    format!(
        "[x={} y={} w={} h={} right={} bottom={}]",
        r.x,
        r.y,
        r.w,
        r.h,
        r.right(),
        r.bottom()
    )
}

// --- Проверки линта (каждая — на каждом прогоне) ------------------------------

fn lint_combo(built: &Built, vp: [f32; 2], lang: Lang) {
    let tag = format!("{}/{}/{}×{}", built.scene, lang.tag(), vp[0], vp[1]);
    let viewport = UiRect::new(0.0, 0.0, vp[0], vp[1]);
    assert_eq!(
        (
            built.frame.viewport.x,
            built.frame.viewport.y,
            built.frame.viewport.w,
            built.frame.viewport.h
        ),
        (viewport.x, viewport.y, viewport.w, viewport.h),
        "{tag}: вьюпорт кадра не совпадает с вьюпортом прогона"
    );

    // Самопроверка дерева: имена уникальны (адресация parent по имени).
    let mut names: Vec<&str> = built.elements.iter().map(|e| e.name.as_str()).collect();
    let total = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), total, "{tag}: имена элементов неуникальны");

    let find = |name: &str| {
        built
            .elements
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("{tag}: parent «{name}» не найден в дереве сцены"))
    };

    let mut visible = 0usize;
    let mut l4_plus = 0usize;
    for el in &built.elements {
        visible += 1;
        let parent_rect = match &el.parent {
            Some(p) => find(p).rect,
            None => viewport,
        };
        // (1) каждый видимый элемент пересекает своего parent (корень — viewport)
        assert!(
            el.rect.intersection(&parent_rect).is_some(),
            "{tag}: [{}] элемент «{}» {} не пересекает parent «{}» {}",
            el.layer.label(),
            el.name,
            dbg_rect(el.rect),
            el.parent.as_deref().unwrap_or("<viewport>"),
            dbg_rect(parent_rect)
        );
        // (2) элементы L4+ (Popups и выше по DRAW_ORDER) пересекают viewport
        if el.layer >= UiLayer::Popups {
            l4_plus += 1;
            assert!(
                el.rect.intersection(&viewport).is_some(),
                "{tag}: [{}] элемент L4+ «{}» {} не пересекает вьюпорт {:?}",
                el.layer.label(),
                el.name,
                dbg_rect(el.rect),
                (vp[0], vp[1])
            );
        }
    }

    // (3) 0 пересечений интерактивных rect'ов разных поверхностей одной полосы
    let overlaps = built.frame.overlaps_within_layer();
    assert!(
        overlaps.is_empty(),
        "{tag}: пересечения интерактивных rect'ов одной полосы: {}",
        overlaps
            .iter()
            .map(|o| format!(
                "[{}] {}:{} × {}:{} at {}",
                o.layer.label(),
                o.a_surface.as_str(),
                o.a_element,
                o.b_surface.as_str(),
                o.b_element,
                dbg_rect(o.at)
            ))
            .collect::<Vec<_>>()
            .join("; ")
    );

    // (4) каждый интерактивный hit-rect целиком во вьюпорте (конвенция
    // canvas-app ui_layout_lint: 0 выходов, eps 0.01)
    let mut interactive = 0usize;
    for surface in &built.frame.surfaces {
        for hit in &surface.hit_rects {
            if !hit.interactive {
                continue;
            }
            interactive += 1;
            let r = hit.rect;
            assert!(
                r.x >= -0.01
                    && r.y >= -0.01
                    && r.right() <= vp[0] + 0.01
                    && r.bottom() <= vp[1] + 0.01,
                "{tag}: интерактивный hit-rect {}:{} {} вне вьюпорта {:?}",
                surface.surface.as_str(),
                hit.element,
                dbg_rect(r),
                (vp[0], vp[1])
            );
        }
    }

    println!("{tag}: {visible} видимых элементов, {interactive} интерактивных, {l4_plus} на L4+ — 0 нарушений");
}

fn build(combo: Combo, ctx: &mut Ctx) -> Built {
    let vp = UiRect::new(0.0, 0.0, combo.vp[0], combo.vp[1]);
    let labels = combo.lang.labels();
    let scene = match combo.scene {
        0 => scene_main_canvas_whatif(ctx, vp, labels),
        1 => scene_palette_dropdown(ctx, vp, labels),
        2 => scene_explain_modal(ctx, vp, labels),
        3 => scene_search_overlay(ctx, vp, labels),
        4 => scene_settings_panel(ctx, vp, labels),
        other => panic!("неизвестный индекс сцены {other}"),
    };
    finish(scene, vp)
}

// --- Сцена 1: main canvas + what-if бар (L3) ----------------------------------

/// Высота what-if бара (FR-017): контент 30px + пад 8px сверху/снизу.
const WHATIF_BAR_H: f32 = 46.0;

fn scene_main_canvas_whatif(ctx: &mut Ctx, vp: UiRect, l: &Labels) -> Scene {
    let mut s = Scene::new(SCENES[0]);
    s.surface("world", UiLayer::World, CapturePolicy::PassThrough);
    s.surface("card_widgets", UiLayer::Widgets, CapturePolicy::Capture);
    s.surface("whatif", UiLayer::Panels, CapturePolicy::Capture);
    s.surface_hidden_below(
        "zoom_chip",
        UiLayer::Panels,
        CapturePolicy::Capture,
        720.0,
        520.0,
    );
    s.surface("toast", UiLayer::Toasts, CapturePolicy::Passive);

    // канвас-зона (L0): вьюпорт минус нижний бар
    let zone = UiRect::new(0.0, 0.0, vp.w, vp.h - WHATIF_BAR_H);
    s.elem(
        "world",
        UiLayer::World,
        "canvas-zone".into(),
        zone,
        None,
        false,
    );

    // карточка узла (L0) с inline-полем (L2): kit::card + kit::text_field
    let card_slot = UiRect::new(60.0, 80.0, 300.0, 130.0);
    let card = card(
        card_slot,
        UiVec2::new(240.0, 100.0),
        UiVec2::new(300.0, 130.0),
        24.0,
        &ctx.palette,
    );
    s.elem(
        "world",
        UiLayer::World,
        "node-card".into(),
        card.rect,
        Some("canvas-zone".into()),
        false,
    );
    s.elem(
        "world",
        UiLayer::World,
        format!("node-card-title[{}]", l.card_title),
        card.header,
        Some("node-card".into()),
        false,
    );
    let model = TextFieldModel {
        text: l.card_field_text.into(),
        ..TextFieldModel::default()
    };
    let field = text_field(
        card.body,
        UiVec2::new(140.0, 28.0),
        UiVec2::new(card.body.w - 16.0, 28.0),
        &model,
        "",
        false,
        KitState::Normal,
        &ctx.palette,
        &mut ctx.m,
        &mut ctx.fs,
        FAMILY,
        UI_FONT,
    );
    s.elem(
        "card_widgets",
        UiLayer::Widgets,
        "inline-field".into(),
        field.rect,
        Some("node-card".into()),
        true,
    );
    s.elem(
        "card_widgets",
        UiLayer::Widgets,
        "inline-field-text".into(),
        field.text_area,
        Some("inline-field".into()),
        false,
    );

    // what-if бар (L3): Row/SqueezeTail — именованная деградация узкого слота
    // (FR-017/CR-015: хвост сжимается до нуля, вырожденные чипы невидимы)
    let bar = UiRect::new(0.0, vp.h - WHATIF_BAR_H, vp.w, WHATIF_BAR_H);
    let content = pad(bar, EdgeInsets::uniform(8.0));
    s.elem(
        "whatif",
        UiLayer::Panels,
        "whatif-bar".into(),
        bar,
        None,
        false,
    );
    s.elem(
        "whatif",
        UiLayer::Panels,
        "whatif-content".into(),
        content,
        Some("whatif-bar".into()),
        false,
    );

    let title = chip_size(l.whatif_title, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
    let mut kids: Vec<(String, Child, bool)> = vec![(
        format!("whatif-title[{}]", l.whatif_title),
        Child::fixed(title.x, title.y),
        false,
    )];
    for chip in l.scenario_chips {
        let size = chip_size(chip, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
        kids.push((
            format!("scenario-chip[{chip}]"),
            Child::fixed(size.x, size.y),
            true,
        ));
    }
    let reset = button_size(l.reset, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
    kids.push((
        format!("reset-btn[{}]", l.reset),
        Child::fixed(reset.x, reset.y),
        true,
    ));
    let items: Vec<Child> = kids.iter().map(|(_, c, _)| *c).collect();
    let rects = ctx.lay_out_row(
        Row {
            gap: 8.0,
            cross: CrossAlign::Center,
            policy: RowPolicy::SqueezeTail,
            ..Row::default()
        },
        content,
        &items,
    );
    for ((name, _, interactive), rect) in kids.into_iter().zip(rects) {
        s.elem(
            "whatif",
            UiLayer::Panels,
            name,
            rect,
            Some("whatif-content".into()),
            interactive,
        );
    }

    // zoom-чип — вторая поверхность полосы Panels (линт F-11 a: две разные
    // поверхности одной полосы не пересекаются интерактивными rect'ами)
    let zoom = UiRect::new(vp.w - 64.0 - 12.0, bar.y - 8.0 - 24.0, 64.0, 24.0);
    s.elem(
        "zoom_chip",
        UiLayer::Panels,
        format!("zoom-chip[{}]", l.zoom_chip),
        zoom,
        None,
        true,
    );

    // тост (L7, Passive): toast_area(avoid=bar) поднимается над баром (CR-016)
    let toast = toast_area(vp, Some(bar));
    s.elem(
        "toast",
        UiLayer::Toasts,
        "toast-area".into(),
        toast,
        None,
        false,
    );
    s
}

// --- Сцена 2: palette dropdown open (L4) --------------------------------------

fn scene_palette_dropdown(ctx: &mut Ctx, vp: UiRect, l: &Labels) -> Scene {
    let mut s = Scene::new(SCENES[1]);
    s.surface("world", UiLayer::World, CapturePolicy::PassThrough);
    s.surface("palette_bar", UiLayer::Panels, CapturePolicy::Capture);
    s.surface_scoped(
        "palette_dropdown",
        UiLayer::Popups,
        CapturePolicy::Capture,
        "palette_dropdown",
    );

    s.elem(
        "world",
        UiLayer::World,
        "canvas-zone".into(),
        vp,
        None,
        false,
    );

    // док палитры (L3): строка чипов категорий
    let bar = UiRect::new(16.0, 12.0, vp.w - 32.0, 44.0);
    let content = pad(bar, EdgeInsets::uniform(8.0));
    s.elem(
        "palette_bar",
        UiLayer::Panels,
        "palette-bar".into(),
        bar,
        None,
        false,
    );

    let mut kids: Vec<(String, Child)> = Vec::new();
    for cat in l.palette_cats {
        let size = chip_size(cat, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
        kids.push((format!("palette-chip[{cat}]"), Child::fixed(size.x, size.y)));
    }
    let items: Vec<Child> = kids.iter().map(|(_, c)| *c).collect();
    let rects = ctx.lay_out_row(
        Row {
            gap: 8.0,
            cross: CrossAlign::Center,
            ..Row::default()
        },
        content,
        &items,
    );
    for ((name, _), &rect) in kids.iter().zip(rects.iter()) {
        s.elem(
            "palette_bar",
            UiLayer::Panels,
            name.clone(),
            rect,
            Some("palette-bar".into()),
            true,
        );
    }

    // dropdown под якорной категорией (kit::dropdown_menu: зажим во вьюпорт
    // по горизонтали + flip при нехватке места снизу)
    let anchor = rects[1];
    let dd = dropdown_menu(anchor, vp, UiVec2::new(240.0, 180.0));
    s.elem(
        "palette_dropdown",
        UiLayer::Popups,
        "palette-dropdown".into(),
        dd.menu,
        None,
        false,
    );
    let menu_content = pad(dd.menu, EdgeInsets::uniform(8.0));
    s.elem(
        "palette_dropdown",
        UiLayer::Popups,
        "palette-dropdown-content".into(),
        menu_content,
        Some("palette-dropdown".into()),
        false,
    );

    // список шаблонов: видимые строки = сколько помещается (list_rows +
    // ScrollState), хвост — скроллбаром, не молчаливым срезом
    let total = l.templates.len();
    let stride = LIST_ROW_H + LIST_ROW_GAP;
    let visible = (((menu_content.h + LIST_ROW_GAP) / stride).floor() as usize).min(total);
    let scroll = ScrollState {
        offset: 0.0,
        content_h: total as f32 * stride - LIST_ROW_GAP,
        viewport_h: menu_content.h,
    };
    for (i, rect) in list_rows(menu_content, &scroll, LIST_ROW_H, LIST_ROW_GAP, visible) {
        s.elem(
            "palette_dropdown",
            UiLayer::Popups,
            format!("tpl-row[{}]", l.templates[i]),
            rect,
            Some("palette-dropdown-content".into()),
            true,
        );
    }
    if let Some(knob) = scroll_bar(menu_content, &scroll, &ctx.palette) {
        s.elem(
            "palette_dropdown",
            UiLayer::Popups,
            "palette-dropdown-scrollbar".into(),
            knob,
            Some("palette-dropdown-content".into()),
            true,
        );
    }
    s
}

// --- Сцена 3: explain modal (L5) ----------------------------------------------

fn scene_explain_modal(ctx: &mut Ctx, vp: UiRect, l: &Labels) -> Scene {
    let mut s = Scene::new(SCENES[2]);
    s.surface("world", UiLayer::World, CapturePolicy::PassThrough);
    s.surface_scoped("explain", UiLayer::Modals, CapturePolicy::Block, "explain");

    s.elem(
        "world",
        UiLayer::World,
        "canvas-zone".into(),
        vp,
        None,
        false,
    );

    // затемнение (decoration, весь вьюпорт) + панель по центру (kit::modal)
    let layout = modal(
        vp,
        UiVec2::new(320.0, 180.0),
        UiVec2::new(vp.w - 48.0, vp.h - 48.0),
        UiVec2::new(480.0, 240.0),
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        "explain-dim".into(),
        layout.dim,
        None,
        false,
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        "explain-panel".into(),
        layout.panel,
        Some("explain-dim".into()),
        false,
    );
    let content = panel_content(layout.panel, &modal_style(&ctx.palette));
    s.elem(
        "explain",
        UiLayer::Modals,
        "explain-panel-content".into(),
        content,
        Some("explain-panel".into()),
        false,
    );

    // Column: заголовок, текст (2 строки), [spacer grow], Row кнопок
    let title_w = ctx
        .m
        .width_of(&mut ctx.fs, l.explain_title, FAMILY, UI_FONT);
    let col_rects = ctx.lay_out_column(
        Column {
            gap: 8.0,
            ..Column::default()
        },
        content,
        &[
            Child::fixed(title_w, 20.0),
            Child::fixed(content.w, 36.0),
            Child::flexible(0.0, 0.0, 1.0),
            Child::fixed(content.w, 30.0),
        ],
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        format!("explain-title[{}]", l.explain_title),
        col_rects[0],
        Some("explain-panel-content".into()),
        false,
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        format!("explain-body[«{}»]", l.explain_body),
        col_rects[1],
        Some("explain-panel-content".into()),
        false,
    );
    // col_rects[2] — spacer (w=0): невидим, в сцену не входит
    let buttons_rect = col_rects[3];
    s.elem(
        "explain",
        UiLayer::Modals,
        "explain-buttons".into(),
        buttons_rect,
        Some("explain-panel-content".into()),
        false,
    );

    let w_close = button_size(l.explain_close, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
    let w_show = button_size(l.explain_show, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
    let btn_rects = ctx.lay_out_row(
        Row {
            gap: 8.0,
            main: MainAlign::End,
            cross: CrossAlign::Center,
            ..Row::default()
        },
        buttons_rect,
        &[
            Child::fixed(w_close.x, w_close.y),
            Child::fixed(w_show.x, w_show.y),
        ],
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        format!("explain-btn-secondary[{}]", l.explain_close),
        btn_rects[0],
        Some("explain-buttons".into()),
        true,
    );
    s.elem(
        "explain",
        UiLayer::Modals,
        format!("explain-btn-primary[{}]", l.explain_show),
        btn_rects[1],
        Some("explain-buttons".into()),
        true,
    );

    let close_icon = icon_button_rect(content, (HAlign::End, VAlign::Start));
    s.elem(
        "explain",
        UiLayer::Modals,
        "explain-close-icon".into(),
        close_icon,
        Some("explain-panel-content".into()),
        true,
    );
    s
}

// --- Сцена 4: search overlay (L4) ---------------------------------------------

fn scene_search_overlay(ctx: &mut Ctx, vp: UiRect, l: &Labels) -> Scene {
    let mut s = Scene::new(SCENES[3]);
    s.surface("world", UiLayer::World, CapturePolicy::PassThrough);
    s.surface_scoped("search", UiLayer::Popups, CapturePolicy::Capture, "search");

    s.elem(
        "world",
        UiLayer::World,
        "canvas-zone".into(),
        vp,
        None,
        false,
    );

    // панель поиска сверху по центру: слот = верхняя полоса 360px вьюпорта,
    // panel_rect центрирует панель в полосе
    let band = UiRect::new(0.0, 0.0, vp.w, 360.0);
    let panel = panel_rect(
        band,
        UiVec2::new(420.0, 260.0),
        UiVec2::new(vp.w - 48.0, 352.0),
        UiVec2::new(560.0, 300.0),
    );
    s.elem(
        "search",
        UiLayer::Popups,
        "search-panel".into(),
        panel,
        None,
        false,
    );
    let content = panel_content(panel, &panel_style(&ctx.palette));
    s.elem(
        "search",
        UiLayer::Popups,
        "search-panel-content".into(),
        content,
        Some("search-panel".into()),
        false,
    );

    // Row: TextField (flex) + кнопка поиска
    let btn_size = button_size(l.search_btn, &mut ctx.m, &mut ctx.fs, FAMILY, UI_FONT);
    let row_rect = UiRect::new(content.x, content.y, content.w, 30.0);
    s.elem(
        "search",
        UiLayer::Popups,
        "search-field-row".into(),
        row_rect,
        Some("search-panel-content".into()),
        false,
    );
    let cell_rects = ctx.lay_out_row(
        Row {
            gap: 8.0,
            cross: CrossAlign::Center,
            ..Row::default()
        },
        row_rect,
        &[
            Child::flexible(200.0, TEXT_FIELD_HEIGHT, 1.0),
            Child::fixed(btn_size.x, btn_size.y),
        ],
    );
    let field = text_field(
        cell_rects[0],
        UiVec2::new(TEXT_FIELD_MIN_W, TEXT_FIELD_HEIGHT),
        UiVec2::new(cell_rects[0].w, TEXT_FIELD_HEIGHT),
        &TextFieldModel::default(),
        l.search_placeholder,
        true,
        KitState::Normal,
        &ctx.palette,
        &mut ctx.m,
        &mut ctx.fs,
        FAMILY,
        UI_FONT,
    );
    s.elem(
        "search",
        UiLayer::Popups,
        "search-field".into(),
        field.rect,
        Some("search-field-row".into()),
        true,
    );
    s.elem(
        "search",
        UiLayer::Popups,
        "search-field-text".into(),
        field.text_area,
        Some("search-field".into()),
        false,
    );
    s.elem(
        "search",
        UiLayer::Popups,
        format!("search-btn[{}]", l.search_btn),
        cell_rects[1],
        Some("search-field-row".into()),
        true,
    );

    // результаты: видимые строки = сколько помещается, хвост — скроллбаром
    let list = UiRect::new(
        content.x,
        content.y + 30.0 + 8.0,
        content.w,
        content.h - 38.0,
    );
    s.elem(
        "search",
        UiLayer::Popups,
        "search-results".into(),
        list,
        Some("search-panel-content".into()),
        false,
    );
    let total = l.results.len();
    let stride = LIST_ROW_H + LIST_ROW_GAP;
    let visible = (((list.h + LIST_ROW_GAP) / stride).floor() as usize).min(total);
    let scroll = ScrollState {
        offset: 0.0,
        content_h: total as f32 * stride - LIST_ROW_GAP,
        viewport_h: list.h,
    };
    for (i, rect) in list_rows(list, &scroll, LIST_ROW_H, LIST_ROW_GAP, visible) {
        s.elem(
            "search",
            UiLayer::Popups,
            format!("result-row[{}]", l.results[i]),
            rect,
            Some("search-results".into()),
            true,
        );
    }
    if let Some(knob) = scroll_bar(list, &scroll, &ctx.palette) {
        s.elem(
            "search",
            UiLayer::Popups,
            "search-scrollbar".into(),
            knob,
            Some("search-results".into()),
            true,
        );
    }
    s
}

// --- Сцена 5: settings panel (L3) ---------------------------------------------

fn scene_settings_panel(ctx: &mut Ctx, vp: UiRect, l: &Labels) -> Scene {
    let mut s = Scene::new(SCENES[4]);
    s.surface("world", UiLayer::World, CapturePolicy::PassThrough);
    s.surface_scoped(
        "settings",
        UiLayer::Panels,
        CapturePolicy::Capture,
        "settings",
    );

    s.elem(
        "world",
        UiLayer::World,
        "canvas-zone".into(),
        vp,
        None,
        false,
    );

    // панель настроек по центру вьюпорта (panel_rect)
    let panel = panel_rect(
        vp,
        UiVec2::new(380.0, 300.0),
        UiVec2::new(vp.w - 48.0, vp.h - 48.0),
        UiVec2::new(460.0, 330.0),
    );
    s.elem(
        "settings",
        UiLayer::Panels,
        "settings-panel".into(),
        panel,
        None,
        false,
    );
    let content = panel_content(panel, &panel_style(&ctx.palette));
    s.elem(
        "settings",
        UiLayer::Panels,
        "settings-panel-content".into(),
        content,
        Some("settings-panel".into()),
        false,
    );

    let title_w = ctx
        .m
        .width_of(&mut ctx.fs, l.settings_title, FAMILY, UI_FONT);
    s.elem(
        "settings",
        UiLayer::Panels,
        format!("settings-title[{}]", l.settings_title),
        UiRect::new(content.x, content.y, title_w, 20.0),
        Some("settings-panel-content".into()),
        false,
    );
    let close_icon = icon_button_rect(content, (HAlign::End, VAlign::Start));
    s.elem(
        "settings",
        UiLayer::Panels,
        "settings-close-icon".into(),
        close_icon,
        Some("settings-panel-content".into()),
        true,
    );

    // строки настроек: Column из Row (лейбл слева, switch справа)
    let rows_area = UiRect::new(
        content.x,
        content.y + 28.0,
        content.w,
        5.0 * 24.0 + 4.0 * 8.0,
    );
    s.elem(
        "settings",
        UiLayer::Panels,
        "settings-rows".into(),
        rows_area,
        Some("settings-panel-content".into()),
        false,
    );
    let row_rects = ctx.lay_out_column(
        Column {
            gap: 8.0,
            ..Column::default()
        },
        rows_area,
        &[Child::fixed(content.w, 24.0); 5],
    );
    let on_flags = [true, false, true, false, false];
    for (i, row_rect) in row_rects.iter().enumerate() {
        let row_name = format!("settings-row[{}]", l.settings_rows[i]);
        s.elem(
            "settings",
            UiLayer::Panels,
            row_name.clone(),
            *row_rect,
            Some("settings-rows".into()),
            false,
        );
        let label_w = ctx
            .m
            .width_of(&mut ctx.fs, l.settings_rows[i], FAMILY, UI_FONT);
        let cells = ctx.lay_out_row(
            Row {
                gap: 8.0,
                main: MainAlign::SpaceBetween,
                cross: CrossAlign::Center,
                ..Row::default()
            },
            *row_rect,
            &[
                Child::fixed(label_w, 16.0),
                Child::fixed(SWITCH_W, SWITCH_H),
            ],
        );
        s.elem(
            "settings",
            UiLayer::Panels,
            format!("settings-label[{}]", l.settings_rows[i]),
            cells[0],
            Some(row_name.clone()),
            false,
        );
        let sw = switch(cells[1], on_flags[i], KitState::Normal, &ctx.palette);
        let switch_name = format!("settings-switch[{}]", l.settings_rows[i]);
        s.elem(
            "settings",
            UiLayer::Panels,
            switch_name.clone(),
            sw.track,
            Some(row_name),
            true,
        );
        s.elem(
            "settings",
            UiLayer::Panels,
            format!("settings-switch-knob[{}]", l.settings_rows[i]),
            sw.knob,
            Some(switch_name),
            false,
        );
    }

    // кнопка «Готово» внизу справа (kit::button_layout: измеренная ширина)
    let done_slot = UiRect::new(content.x, content.bottom() - 30.0, content.w, 30.0);
    let done = button_layout(
        done_slot,
        l.settings_done,
        (HAlign::End, VAlign::Center),
        &mut ctx.m,
        &mut ctx.fs,
        FAMILY,
        UI_FONT,
    );
    s.elem(
        "settings",
        UiLayer::Panels,
        format!("settings-btn-done[{}]", done.label),
        done.rect,
        Some("settings-panel-content".into()),
        true,
    );
    s
}

// --- Тесты: 5 сцен + счётчик матрицы -------------------------------------------

#[test]
fn scene_1_main_canvas_whatif() {
    let mut ctx = Ctx::new();
    let mut runs = 0;
    for combo in matrix().into_iter().filter(|c| c.scene == 0) {
        let built = build(combo, &mut ctx);
        lint_combo(&built, combo.vp, combo.lang);
        runs += 1;
    }
    assert_eq!(
        runs, RUNS_PER_SCENE,
        "сцена должна покрывать все комбинации окно×язык"
    );
}

#[test]
fn scene_2_palette_dropdown() {
    let mut ctx = Ctx::new();
    let mut runs = 0;
    for combo in matrix().into_iter().filter(|c| c.scene == 1) {
        let built = build(combo, &mut ctx);
        lint_combo(&built, combo.vp, combo.lang);
        runs += 1;
    }
    assert_eq!(runs, RUNS_PER_SCENE);
}

#[test]
fn scene_3_explain_modal() {
    let mut ctx = Ctx::new();
    let mut runs = 0;
    for combo in matrix().into_iter().filter(|c| c.scene == 2) {
        let built = build(combo, &mut ctx);
        lint_combo(&built, combo.vp, combo.lang);
        runs += 1;
    }
    assert_eq!(runs, RUNS_PER_SCENE);
}

#[test]
fn scene_4_search_overlay() {
    let mut ctx = Ctx::new();
    let mut runs = 0;
    for combo in matrix().into_iter().filter(|c| c.scene == 3) {
        let built = build(combo, &mut ctx);
        lint_combo(&built, combo.vp, combo.lang);
        runs += 1;
    }
    assert_eq!(runs, RUNS_PER_SCENE);
}

#[test]
fn scene_5_settings_panel() {
    let mut ctx = Ctx::new();
    let mut runs = 0;
    for combo in matrix().into_iter().filter(|c| c.scene == 4) {
        let built = build(combo, &mut ctx);
        lint_combo(&built, combo.vp, combo.lang);
        runs += 1;
    }
    assert_eq!(runs, RUNS_PER_SCENE);
}

/// Ассерт полноты матрицы: ровно 30 прогонов (5 сцен × 3 окна × 2 языка),
/// каждая сцена покрыта всеми окнами и языками, комбинации уникальны.
#[test]
fn lint_covers_30_scenarios() {
    assert_eq!(
        SCENES.len() * VIEWPORTS.len() * Lang::ALL.len(),
        30,
        "постановка FR-068 W0: ровно 30 прогонов"
    );
    let combos = matrix();
    assert_eq!(
        combos.len(),
        30,
        "счётчик сгенерированных комбинаций должен быть 30"
    );

    let mut seen = HashSet::new();
    for combo in &combos {
        assert!(
            seen.insert(format!(
                "{}|{:?}|{}",
                combo.scene,
                combo.vp,
                combo.lang.tag()
            )),
            "дубликат комбинации: сцена {} {:?} {}",
            combo.scene,
            combo.vp,
            combo.lang.tag()
        );
    }

    for (idx, name) in SCENES.iter().enumerate() {
        let mine: Vec<&Combo> = combos.iter().filter(|c| c.scene == idx).collect();
        assert_eq!(
            mine.len(),
            RUNS_PER_SCENE,
            "сцена «{name}»: ожидается {RUNS_PER_SCENE} прогонов"
        );
        for vp in VIEWPORTS {
            assert!(
                mine.iter().any(|c| c.vp == vp),
                "сцена «{name}»: окно {vp:?} не покрыто"
            );
        }
        for lang in Lang::ALL {
            assert!(
                mine.iter().any(|c| c.lang == lang),
                "сцена «{name}»: язык {} не покрыт",
                lang.tag()
            );
        }
    }
}

// --- Taffy-прогоны (FR-068 W1, Контракт-3 «G4-линты × N backend'ов») ----------

/// Те же 5 canonical сцен × 3 окна × 2 языка, построенные через
/// `TaffyBackend` (Row/Column — `lay_out_with`; kit-функции остаются
/// native — они не flex), с ТЕМИ ЖЕ G4-проверками (parent-пересечение,
/// viewport L4+, overlaps пуст, hit-rect'ы во вьюпорте). Компилируется
/// только под фичей `taffy` (default-сборка не тянет taffy — G7).
///
/// Сцены подобраны так, что помещаются в самое малое окно 800×560;
/// SqueezeTail-деградация what-if бара под taffy сжимает детей без
/// выхода за пределы слота (flex_shrink распределяет сжатие
/// пропорционально — C3) — G4-свойства сохраняются. Числа могут
/// отличаться от native-прогонов (rounding px-сетки taffy на дробных
/// measured-размерах, ≤ 0.5 ui px) — побитовое равенство с native
/// пинится отдельно в `backend_parity.rs`; здесь проверяются СВОЙСТВА
/// вёрстки, а не совпадение значений.
#[cfg(feature = "taffy")]
mod taffy_backend_runs {
    use super::*;

    /// Ctx с taffy-путём примитивов Row/Column.
    fn taffy_ctx() -> Ctx {
        let mut ctx = Ctx::new();
        ctx.backend = SceneBackend::Taffy;
        ctx
    }

    #[test]
    fn taffy_scene_1_main_canvas_whatif() {
        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in matrix().into_iter().filter(|c| c.scene == 0) {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(
            runs, RUNS_PER_SCENE,
            "taffy: сцена должна покрывать все комбинации окно×язык"
        );
    }

    #[test]
    fn taffy_scene_2_palette_dropdown() {
        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in matrix().into_iter().filter(|c| c.scene == 1) {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(runs, RUNS_PER_SCENE);
    }

    #[test]
    fn taffy_scene_3_explain_modal() {
        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in matrix().into_iter().filter(|c| c.scene == 2) {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(runs, RUNS_PER_SCENE);
    }

    #[test]
    fn taffy_scene_4_search_overlay() {
        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in matrix().into_iter().filter(|c| c.scene == 3) {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(runs, RUNS_PER_SCENE);
    }

    #[test]
    fn taffy_scene_5_settings_panel() {
        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in matrix().into_iter().filter(|c| c.scene == 4) {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(runs, RUNS_PER_SCENE);
    }

    /// Счётчик матрицы taffy-прогонов (паттерн `lint_covers_30_scenarios`):
    /// полнота 5×3×2 = 30, уникальность, покрытие окон/языков — плюс
    /// сквозной прогон ВСЕХ 30 комбинаций через taffy-путь с линтом.
    #[test]
    fn taffy_lint_covers_30_scenarios() {
        assert_eq!(
            SCENES.len() * VIEWPORTS.len() * Lang::ALL.len(),
            30,
            "постановка FR-068 W0: ровно 30 прогонов (та же матрица под taffy)"
        );
        let combos = matrix();
        assert_eq!(combos.len(), 30, "счётчик комбинаций должен быть 30");

        let mut seen = HashSet::new();
        for combo in &combos {
            assert!(
                seen.insert(format!(
                    "{}|{:?}|{}",
                    combo.scene,
                    combo.vp,
                    combo.lang.tag()
                )),
                "дубликат комбинации: сцена {} {:?} {}",
                combo.scene,
                combo.vp,
                combo.lang.tag()
            );
        }

        let mut ctx = taffy_ctx();
        let mut runs = 0;
        for combo in combos {
            let built = build(combo, &mut ctx);
            lint_combo(&built, combo.vp, combo.lang);
            runs += 1;
        }
        assert_eq!(
            runs, 30,
            "taffy-прогон должен покрыть все 30 комбинаций (5 сцен × 3 окна × 2 языка)"
        );
    }
}
