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
    /// FR-LLM-B / PRD-0010 F-8: экран выбора AI-режима (Local / Cloud /
    /// Self-hosted) — полноэкранная модаль (L5, Block). LAY-W17 (ревью
    /// §3.4/§4): зарегистрирован в реестре (был главным «слепым пятном»
    /// G4 — модаль вне реестра без hit-rect'ов/линта/telemetry). Открытие —
    /// действие пользователя (пункт «?» «Онбординг AI» / триггер продукта).
    pub const AI_ONBOARDING: &str = "ai_onboarding";
    /// FR-055 (этап U4, F-8): витрина кита (L5, Block — мимо панели
    /// закрывается и глотает; вход — пункт «?» «О интерфейсе», Q5-a).
    pub const KIT_GALLERY: &str = "kit_gallery";
    /// FR-070: UI-админпанель (L5, Block — мимо панели закрывается и
    /// глотает; вход — пункт «?» «UI-консоль»).
    pub const ADMIN: &str = "admin_panel";
    /// Empty-state карточка пустого канваса (L3, Capture — мимо карточки
    /// канвас жив, AC-1.1 FR-049).
    pub const EMPTY: &str = "empty";
    /// FR-LLM-D / PRD-0010 F-4: агент-панель (L3, Capture — клики внутри
    /// глотаются ранней ветвью ввода; HideBelow { 600, 240 } — LAY-W1).
    pub const AGENT_PANEL: &str = "agent_panel";
    /// FR-LLM / PRD-0010 F-7.9: AI-статус-панель (L3, Capture; ambient-хром —
    /// видима, пока AI не выключен; HideBelow { 900, 131 } — LAY-W1, порог —
    /// derive из констант панели, стыковка LAY-W2).
    pub const AI_STATUS: &str = "ai_status";
    /// Миникарта (L3, Capture; рисуется проходом рендерера поверх полос).
    pub const MINIMAP: &str = "minimap";
    /// FR-105 (мультиканвас C2): баннер потери доступа к granted-папке
    /// (L3, Capture — клики по кнопкам баннера глотаются; мимо баннера
    /// канвас жив: баннер не блокирует работу, №44b).
    pub const STORAGE_BANNER: &str = "storage_banner";
    /// FR-105: диалог миграции OPFS→папка (L5, Block — Esc/«Отмена»/«✕»
    /// закрывают, клик мимо панели — тоже; №42a/№52a).
    pub const MIGRATE: &str = "migrate";
    /// FR-106 (C3, issue #7): менеджер канвасов — оверлей (список/поиск/
    /// сортировка/группы, создание, ренейм №9, удаление №15a, строка
    /// хранилища №51a). Модаль Modals/Block; вход — кнопка «Недавние»
    /// DOM-панели web (до чипа №21c волны C4).
    pub const CANVAS_MANAGER: &str = "canvas_manager";
    /// FR-105: тост с действием «Перезагрузить» (L7, Capture — интерактивна
    /// только кнопка; клик мимо проваливается в канвас, №45b).
    pub const TOAST: &str = "toast";
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
    /// FR-105 (мультиканвас C2): диалог миграции — модаль; Esc/Enter
    /// (кнопка «Переехать»)/стрелки/Space (галочка) — в диалоге, прочие
    /// глотаются (паттерн галереи схем).
    Migrate,
    /// FR-106 (мультиканвас C3): менеджер канвасов — модаль; Esc/↑/↓/Enter/
    /// F2 (ренейм №9) + ввод в фильтр/буфер ренейма, прочие глотаются
    /// (паттерн галереи схем).
    CanvasManager,
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
        // FR-105 (C2): диалог миграции — модаль (Esc/Enter/стрелки/Space)
        id::MIGRATE => Some(KeyOwner::Migrate),
        // FR-106 (C3): менеджер канвасов — модаль (Esc/стрелки/Enter/F2/фильтр)
        id::CANVAS_MANAGER => Some(KeyOwner::CanvasManager),
        _ => None,
    }
}

/// LAY8 (п.3, design/rules/11-layouts.md): политики деградации панелей —
/// ЕДИНСТВЕННЫЙ источник брейкпоинтов показа (LAY8.2: брейкпоинт обязан
/// жить в `SurfaceRegistry`, а не в теле отрисовки). Draw/hit-тела панелей
/// консультируются с этими же политиками ([`agent_panel_visible`] /
/// [`ai_status_visible`]), линт G4 видит те же условия (HideBelow
/// применяется `UiFrame::from_registry`).
///
/// **Агент-панель** (PRD-0010 F-4): min_w 600 — прежний брейкпоинт
/// `agent_panel::AGENT_PANEL_MIN_VIEWPORT_W`; min_h 240 — фактический
/// минимум контента: фиксированный вертикальный стек 186 px (44 шапка +
/// 32 контекст-чипы + 22 cost-строка + 44 полоса ввода + 36 quick + 8
/// зазор GAP) + 54 px минимума журнала (остаток min_h 240 − 186; при
/// line_h 15 лога влезает три строки сообщения — 45 px + 8 px верхнего
/// пада — с запасом 1 px) — ниже журнал
/// схлопывается (`log_h ≤ 0`), панель нечитаема (аудит предлагал { 600, 0
/// } — осуждено: full-height панель с нулевой высотой — вырожденный rect,
/// запрещён LAY8 п.4).
pub(crate) const AGENT_PANEL_DEGRADATION: DegradationPolicy = DegradationPolicy::HideBelow {
    min_width: agent_panel::AGENT_PANEL_MIN_VIEWPORT_W,
    min_height: 240.0,
};

/// LAY8 (п.3): **AI-статус-панель** (PRD-0010 F-7.9): min_w 900 — прежний
/// брейкпоинт `ai_status_panel::AI_STATUS_MIN_VIEWPORT_W`; min_h — DERIVE
/// из констант панели (стыковка LAY-W2: единственный источник цифр — сама
/// панель), worst-case контент с paused-лейблом
/// `AI_STATUS_H + AI_STATUS_PAUSED_EXTRA` = 100 + 19 = 119 плюс нижний
/// отступ `AI_STATUS_MARGIN` 12 → 131 (до миграции W2 было 95 + 18 + 12 =
/// 125): ниже панель вылезает за верх вьюпорта (y < 0) — LAY8 требует
/// скрыть ЦЕЛИКОМ (клип вместо скрытия запрещён). Порог выведен для
/// размещения в правом нижнем углу (вне зоны миникарты); над зоной
/// миникарты (окно ≥ 252×172 — minimap_pass) панель требует большей
/// высоты, геометрия той ветки не менялась (вне стыковки W2).
pub(crate) const AI_STATUS_DEGRADATION: DegradationPolicy = DegradationPolicy::HideBelow {
    min_width: ai_status_panel::AI_STATUS_MIN_VIEWPORT_W,
    min_height: ai_status_panel::AI_STATUS_H
        + ai_status_panel::AI_STATUS_PAUSED_EXTRA
        + ai_status_panel::AI_STATUS_MARGIN,
};

/// **What-if** (PRD-0010 F-11c): порог 900×600 — ЕДИНСТВЕННОЕ место
/// объявления (LAY-W21: раньше литералы жили инлайном в декларации ниже и
/// ДУБЛИРОВАЛИСЬ в [`whatif_pill_visible`] — ревью §3.3). Деградация
/// применяется к обеим формам поверхности: бар скрывается HideBelow-политикой
/// кадра (`UiFrame::from_registry`), пилюля входа — гейтом
/// [`whatif_pill_visible`] над ЭТОЙ же константой (паттерн
/// [`AGENT_PANEL_DEGRADATION`] / [`AI_STATUS_DEGRADATION`]; именованного
/// источника в `whatif_ui` нет — брейкпоинт всегда жил в реестре).
pub(crate) const WHATIF_DEGRADATION: DegradationPolicy = DegradationPolicy::HideBelow {
    min_width: 900.0,
    min_height: 600.0,
};

/// LAY8.2: гейт показа агент-панели для draw/hit-тел — та же политика
/// HideBelow, что в декларации реестра (единственный источник решения;
/// inline-сравнения 600px из тел отрисовки удалены — LAY-W1).
pub(crate) fn agent_panel_visible(viewport: [f32; 2]) -> bool {
    !AGENT_PANEL_DEGRADATION.hidden_at(viewport[0], viewport[1])
}

/// LAY8.2: гейт показа AI-статус-панели для draw/hit-тел — HideBelow
/// декларации реестра (900×131, derive из констант панели; LAY-W1+W2).
pub(crate) fn ai_status_visible(viewport: [f32; 2]) -> bool {
    !AI_STATUS_DEGRADATION.hidden_at(viewport[0], viewport[1])
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

/// Брейкпоинты ширины модалки настроек ([`id::SETTINGS`], LAY8.2 норматива
/// `design/rules/11-layouts.md`): брейкпоинт регистрируется РЯДОМ С
/// декларацией поверхности в реестре, а не в теле отрисовки. Единственное
/// объявление — здесь; `settings_ui` потребляет их ре-экспортом под
/// прежними именами (`MODAL_BP_*`) — [`crate::settings_ui::modal_mode`]
/// ветвится Desktop/Compact/Mobile (W-e, вариант «A», решение владельца
/// 03.10.2026): ≥ 1280 — двухколоночная раскладка, 768..1280 — одноколоночная
/// с горизонтальным таб-баром, < 768 — полноэкранный лист. Порог «компакт»
/// 1280 совпадает с каноническим вьюпортом гейта LAY8.1 (1280×800).
pub const SETTINGS_BP_COMPACT: f32 = 1280.0;
/// Брейкпоинт «мобильный» модалки настроек ([`id::SETTINGS`], ширина окна,
/// лог. px): [`SETTINGS_BP_MOBILE`] ≤ viewport < [`SETTINGS_BP_COMPACT`] —
/// компакт-режим ([`crate::settings_ui::modal_mode`]), ниже — полноэкранный
/// лист вьюпорт минус внешние поля. См. [`SETTINGS_BP_COMPACT`] —
/// единственное объявление брейкпоинтов настроек (LAY8.2).
pub const SETTINGS_BP_MOBILE: f32 = 768.0;

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
    //    LAY-W21: политика — WHATIF_DEGRADATION (единственный источник
    //    900×600; раньше инлайн-литералы, дублировавшиеся гейтом пилюли).
    if app.scene.whatif_active || whatif_pill_visible(app) {
        reg.add(
            SurfaceDecl::new(id::WHATIF, UiLayer::Panels, CapturePolicy::Capture)
                .with_scope(id::WHATIF)
                .with_degradation(WHATIF_DEGRADATION),
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
    //    W-e (дефект №13 аудита): политика деградации объявлена ЯВНО —
    //    Always: настройки не прячутся ни на одном вьюпорте, они
    //    АДАПТИРУЮТСЯ брейкпоинтами ширины (settings_ui::modal_mode,
    //    вариант «A» от 03.10.2026) — HideBelow противоречил бы смыслу
    //    волны (модалка целиком в окне вплоть до 320×240).
    //    Брейкпоинты адаптации — SETTINGS_BP_COMPACT/SETTINGS_BP_MOBILE
    //    (выше, перед build_registry; LAY8.2: объявлены здесь же, рядом
    //    с декларацией поверхности, а не в теле отрисовки settings_ui).
    if app.settings_open {
        reg.add(
            SurfaceDecl::new(id::SETTINGS, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::SETTINGS)
                .with_degradation(DegradationPolicy::Always),
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
    // FR-105 (мультиканвас C2, №44b): баннер потери доступа к granted-папке
    // — Panels/Capture (клик мимо баннера работает с канвасом: баннер не
    // блокирует работу; без scope — ввод вне кнопок не глотается).
    if app.storage_banner.is_some() {
        reg.add(SurfaceDecl::new(
            id::STORAGE_BANNER,
            UiLayer::Panels,
            CapturePolicy::Capture,
        ));
    }
    // FR-105 (C2, №42a/№52a): диалог миграции OPFS→папка — Modals/Block
    // (Esc/«Отмена»/«✕» закрывают, клик мимо панели — backdrop-закрытие).
    if app.migrate.open {
        reg.add(
            SurfaceDecl::new(id::MIGRATE, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::MIGRATE),
        );
    }
    // FR-106 (C3, issue #7): менеджер канвасов — Modals/Block (Esc/
    // backdrop закрывают, клик по телу панели глотается — модаль жива).
    // Деградация — Always (как у диалога миграции: панель клампится к
    // вьюпорту, список скроллится — прятать целиком нет причин).
    if app.canvas_manager.open {
        reg.add(
            SurfaceDecl::new(id::CANVAS_MANAGER, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::CANVAS_MANAGER)
                .with_degradation(DegradationPolicy::Always),
        );
    }
    // FR-105 (C2, №45b): тост с действием «Перезагрузить» — Toasts/Capture:
    // интерактивна ТОЛЬКО кнопка (hit-rect ниже); клик мимо — в канвас,
    // прежняя пассивность строки тоста сохранена.
    if app.toast_action.is_some() {
        reg.add(SurfaceDecl::new(
            id::TOAST,
            UiLayer::Toasts,
            CapturePolicy::Capture,
        ));
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
    // FR-LLM-B / PRD-0010 F-8: AI-онбординг — Modals/Block (полноэкранная
    // модаль выбора AI-режима; backdrop глотается без закрытия — явный
    // выбор, контракт онбординга). LAY-W17: регистрация в реестре (ревью
    // §3.4 — ai_onboarding был вне реестра: без id/hit-rect'ов/линта).
    // Деградация — Always ЯВНО (семантика как у DIALOG/SETTINGS): карточка
    // АДАПТИРУЕТСЯ клампом к вьюпорту с полями AI_ONB_VIEWPORT_MARGIN
    // (onboarding_ui::ai_onboarding_layout, kit::modal) вплоть до узких
    // окон — HideBelow противоречил бы смыслу модали выбора.
    if app.ai_onboarding.is_some() {
        reg.add(
            SurfaceDecl::new(id::AI_ONBOARDING, UiLayer::Modals, CapturePolicy::Block)
                .with_scope(id::AI_ONBOARDING)
                .with_degradation(DegradationPolicy::Always),
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
    // 21. FR-LLM-D / PRD-0010 F-4: агент-панель (Ctrl+I; чат-UI
    //     tool-calling). LAY-W1 (аудит §3.8, P1 LAY8.2): брейкпоинт показа
    //     перенесён из тела отрисовки СЮДА — HideBelow в декларации (LAY8
    //     п.3: ниже минимума поверхность скрыта ЦЕЛИКОМ). Capture без
    //     scope: клики по-прежнему обрабатывает agent_panel_click (ранняя
    //     ветвь ввода до pick), esc-стек и клавиатура не меняются.
    if app.agent_panel.open {
        reg.add(
            SurfaceDecl::new(id::AGENT_PANEL, UiLayer::Panels, CapturePolicy::Capture)
                .with_degradation(AGENT_PANEL_DEGRADATION),
        );
    }
    // 22. FR-LLM / PRD-0010 F-7.9: AI-статус-панель — ambient-хром (видима,
    //     пока AI не выключен целиком). Брейкпоинт 900px из тела отрисовки —
    //     в HideBelow декларации (LAY-W1, LAY8.2); в телеметрии — ambient
    //     (telemetry::AMBIENT_SURFACES), открытием не считается.
    if !app.settings.llm.all_off() {
        reg.add(
            SurfaceDecl::new(id::AI_STATUS, UiLayer::Panels, CapturePolicy::Capture)
                .with_degradation(AI_STATUS_DEGRADATION),
        );
    }
    reg
}

/// Пилюля входа в what-if видна на вьюпортах ≥ hide-порога (те же условия,
/// что у бара — деградация применяется к обеим формам поверхности).
/// LAY-W21: условие — из политики [`WHATIF_DEGRADATION`] декларации
/// (единственный источник 900×600; раньше литералы дублировались здесь —
/// ревью §3.3); `hidden_at` — строгий `<`, поведение бит-в-бит прежнему
/// `w >= 900 && h >= 600`.
fn whatif_pill_visible(app: &App) -> bool {
    let [w, h] = app.viewport_logical();
    !WHATIF_DEGRADATION.hidden_at(w, h)
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
    // LAY-W1: панели AI — поверх дока/миникарты, под меню/модалями
    // (панели Panels-полосы; миникарта при открытой агент-панели скрыта).
    id::AGENT_PANEL,
    id::AI_STATUS,
    id::MINIMAP,
    // FR-105 (C2): баннер потери доступа — Panels-полоса над панелями AI
    id::STORAGE_BANNER,
    id::EDITOR,
    id::PALETTE,
    id::WHEEL,
    id::MENU,
    id::CHOICE_MENU,
    id::HELP_MENU,
    id::DOCS,
    // FR-106 (C3): менеджер канвасов — ПОД галереей-пикером (№38a:
    // «Из шаблона…» открывает галерею поверх менеджера; список — снизу
    // вверх, поэтому менеджер раньше галереи)
    id::CANVAS_MANAGER,
    id::GALLERY,
    id::KIT_GALLERY,
    id::ADMIN,
    // FR-105 (C2): диалог миграции — модаль уровня галереи/витрины
    id::MIGRATE,
    id::ONBOARDING,
    // LAY-W17: AI-онбординг — над туром онбординга (порядок полосы Modals
    // в handler.rs: ai_onboarding-полоса пушится после onboarding-полосы),
    // под DIALOG (Да/Нет — верх модального стека).
    id::AI_ONBOARDING,
    id::DIALOG,
    id::EXPLAIN,
    id::AUTOLINK,
    id::STAGE,
    // FR-105 (C2): тост с действием — верхняя полоса (Toasts-слой и так
    // выше модалей; ранг — детерминизм отладочного оверлея)
    id::TOAST,
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
    // FR-090: surface_opened — диф активных поверхностей (web; натив —
    // no-op). Здесь, а не в App::ui_frame: пересборка кадра происходит
    // ровно тогда, когда меняется сигнатура поверхностей (build_frame_sig).
    super::telemetry::surface_diff(app, &registry);
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

// ============================================================================
// FR-PERF-A: кэш UI-кадра с инвалидацией по сигнатуре
// ============================================================================
//
// `build_frame(app)` вызывался 2× за кадр (hover hit-test в `on_cursor_moved`
// + DebugOverlay в `RedrawRequested`) с полной пересборкой `build_registry` +
// `UiFrame::from_registry` + sort + `fill_hit_rects` для каждой поверхности
// (~сотни мкс–мс на кадр). Кэш + сигнатура инвалидации — пересборка только
// при реальном изменении состояния; типичный случай (движение курсора без
// прочих взаимодействий) — cache hit во втором вызове и между кадрами.
//
// Сигнатура `UiFrameSig` собирает всё, что влияет на выход `build_frame_at`:
// вьюпорт, ревизия сцены, позиция/зум камеры (через `palette_anchor_screen`),
// битовые флаги открытых поверхностей, хэш выделения (для `palette_target`),
// скроллы/раскрытия отдельных панелей и т.д. Сравнение дешёвое (~80 байт
// primitive compares), без аллокаций.

/// FR-PERF-A: Битовые флаги открытых/активных поверхностей для сигнатуры
/// кэша UI-кадра. Любое изменение → пересборка (hit-rect'ы зависят от
/// состава кадра). Простые `pub const` вместо крейта `bitflags` — G7
/// (0 внешних зависимостей у canvas-ui/canvas-app).
pub mod ui_frame_flags {
    /// Wheel-меню шаблонов открыто (`app.wheel_menu.is_some()`).
    pub const WHEEL_OPEN: u64 = 1 << 0;
    /// What-if активен (`app.scene.whatif_active || whatif_pill_visible`).
    pub const WHATIF_ACTIVE: u64 = 1 << 1;
    /// Список подмен whatif раскрыт (`app.whatif_list_open`).
    pub const WHATIF_LIST_OPEN: u64 = 1 << 2;
    /// Таблица сравнения whatif открыта (`app.whatif_compare_open`).
    pub const WHATIF_COMPARE_OPEN: u64 = 1 << 3;
    /// Панель хоткеев открыта (`app.hotkeys_open`).
    pub const HOTKEYS_OPEN: u64 = 1 << 4;
    /// Модалка настроек открыта (`app.settings_open`).
    pub const SETTINGS_OPEN: u64 = 1 << 5;
    /// Выпадающее меню строки настроек открыто (`app.settings_dropdown.open_row.is_some()`).
    pub const SETTINGS_DROPDOWN_OPEN: u64 = 1 << 6;
    /// Контекстное меню канваса открыто (`app.menu.is_some()`).
    pub const MENU_OPEN: u64 = 1 << 7;
    /// Подменю канваса раскрыто (`app.menu.as_ref().and_then(|m| m.submenu.as_ref()).is_some()`).
    pub const MENU_SUBMENU_OPEN: u64 = 1 << 8;
    /// Меню выбора (choice_menu) открыто (`app.choice_menu.is_some()`).
    pub const CHOICE_MENU_OPEN: u64 = 1 << 9;
    /// Док палитры шаблонов развёрнут (`app.template_panel.open`).
    pub const TEMPLATE_PANEL_OPEN: u64 = 1 << 10;
    /// Палитра выделения видима (`app.palette_geometry().is_some()`).
    pub const PALETTE_VISIBLE: u64 = 1 << 11;
    /// Hover-раскрытие группы палитры активно (`app.palette_hover.open.is_some()`).
    pub const PALETTE_HOVER_OPEN: u64 = 1 << 12;
    /// Просмотрщик документации открыт (`app.docs.is_some()`).
    pub const DOCS_OPEN: u64 = 1 << 13;
    /// Меню помощи «?» открыто (`app.help_menu.is_some()`).
    pub const HELP_MENU_OPEN: u64 = 1 << 14;
    /// Подменю документации в меню помощи раскрыто (`help_menu.docs_open`).
    pub const HELP_MENU_DOCS_OPEN: u64 = 1 << 15;
    /// Main stage открыт (`app.main_stage.is_some()`).
    pub const STAGE_OPEN: u64 = 1 << 16;
    /// Панель поиска открыта (`app.search.is_open()`).
    pub const SEARCH_OPEN: u64 = 1 << 17;
    /// Карта проливаний открыта (`app.flow_map_open`).
    pub const FLOW_MAP_OPEN: u64 = 1 << 18;
    /// Сессия инлайн-редактирования активна (`app.editing.is_some()`).
    pub const EDITOR_OPEN: u64 = 1 << 19;
    /// Окно проверки цепочки расчёта открыто (`app.explain.is_some()`).
    pub const EXPLAIN_OPEN: u64 = 1 << 20;
    /// Диалог ревью автосвязи открыт (`app.autolink_review.is_some()`).
    pub const AUTOLINK_REVIEW_OPEN: u64 = 1 << 21;
    /// Модальный диалог Да/Нет открыт (`app.dialog.is_some()`).
    pub const DIALOG_OPEN: u64 = 1 << 22;
    /// Галерея схем открыта (`app.scheme_gallery.open`).
    pub const GALLERY_OPEN: u64 = 1 << 23;
    /// Витрина кита открыта (`app.kit_gallery_open`).
    pub const KIT_GALLERY_OPEN: u64 = 1 << 24;
    /// UI-админпанель открыта (`app.admin_open`).
    pub const ADMIN_OPEN: u64 = 1 << 25;
    /// Онбординг-тур активен (`app.onboarding.is_some()`).
    pub const ONBOARDING_OPEN: u64 = 1 << 26;
    /// Empty-state карточка видна (`app.empty_state_visible()`).
    pub const EMPTY_VISIBLE: u64 = 1 << 27;
    /// Миникарта видна (`app.minimap_rect().is_some()`).
    pub const MINIMAP_VISIBLE: u64 = 1 << 28;
    /// Бейдж автосвязи виден (`app.autolink_badge_visible()`).
    pub const AUTOLINK_BADGE_VISIBLE: u64 = 1 << 29;
    /// Drag ноды активен — глушит `palette_target` (см. `palette_target()`).
    pub const DRAGGING: u64 = 1 << 30;
    /// Drag резиновой линии связи или рамки выделения активен — глушит
    /// `palette_target` (см. `palette_target()`).
    pub const EDGE_OR_SELECT_DRAG: u64 = 1 << 31;
    /// FR-LLM-D / LAY-W1: агент-панель открыта (`app.agent_panel.open`).
    /// u32 был исчерпан (32 бита) — флаги расширены до u64.
    pub const AGENT_PANEL_OPEN: u64 = 1 << 32;
    /// FR-LLM / LAY-W1: AI-статус-панель в реестре (`!llm.all_off()`,
    /// ambient); брейкпоинт вьюпорта учитывается сигнатурным `viewport`.
    pub const AI_STATUS_VISIBLE: u64 = 1 << 33;
    /// LAY-W17: AI-онбординг открыт (`app.ai_onboarding.is_some()`).
    /// Регистрация в реестре — состав кадра меняется открытием/закрытием
    /// модали; без флага кэш отдал бы кадр без поверхности (pick мимо).
    pub const AI_ONBOARDING_OPEN: u64 = 1 << 34;
    /// FR-105 (C2, №44b): баннер потери доступа показан
    /// (`app.storage_banner.is_some()`).
    pub const STORAGE_BANNER_VISIBLE: u64 = 1 << 35;
    /// FR-105 (C2, №42a): диалог миграции открыт (`app.migrate.open`).
    pub const MIGRATE_OPEN: u64 = 1 << 36;
    /// FR-105 (C2, №45b): тост с действием активен (`app.toast_action`).
    pub const TOAST_ACTION_ACTIVE: u64 = 1 << 37;
    /// FR-106 (C3): менеджер канвасов открыт (`app.canvas_manager.open`).
    pub const CANVAS_MANAGER_OPEN: u64 = 1 << 38;
}

/// FR-PERF-A: Сигнатура инвалидации кэша UI-кадра — всё, что влияет на
/// `build_frame_at`. Любое изменение → пересборка. Сравнение дешёвое
/// (primitive compares, без аллокаций). Хэш через `PartialEq` (derive).
///
/// Поля:
/// * `viewport` — логический (w, h); влияет на все layout-функции.
/// * `scene_revision` — `SceneState::revision` (правки нод/рёбер/групп,
///   смена активного whatif-сценария — см. `recompute_flow`).
/// * `camera` — (center_x, center_y, zoom); позиционирует PALETTE через
///   `palette_anchor_screen` (`world_to_screen` от bbox выделения).
/// * `flags` — битовые флаги открытых поверхностей (см. [`ui_frame_flags`]).
/// * `selection_hash` — хэш `selected` + `selected_nodes` (PALETTE зависит
///   от выделения; выделение НЕ меняет `scene_revision`).
/// * `whatif_active` / `whatif_scenario_count` / `whatif_frozen_count` /
///   `whatif_override_count` — состояние what-if (сценарии/заморозки/подмены
///   НЕ меняют `scene_revision`, но влияют на `whatif_bar_layout`).
/// * `flow_map_scroll_offset` — скролл списка карты проливаний.
/// * `template_hover_open` / `template_hover_scroll` — hover-раскрытие
///   категории template-strip (flyout влияет на TEMPLATE_STRIP hit-rect'ы).
/// * `palette_hover_open` — раскрытая hover'ом группа палитры (PALETTE
///   dropdown hit-rect).
/// * `onboarding_step` — шаг онбординг-тура (ONBOARDING card_rect).
/// * `language` — язык интерфейса (влияет на замеряемые ширины подписей
///   палитры/меню/диалогов/онбординга).
/// * `button_corner` — позиция угловых кнопок (CORNER_BUTTONS hit-rect'ы).
/// * `settings_tab` — активный таб модалки настроек (SETTINGS modal_layout).
/// * `search_scroll_top` / `search_rows_count` — состояние панели поиска
///   (SEARCH layout: число видимых строк = clamp(rows − scroll, MAX)).
/// * `admin_section` — активная секция админпанели (ADMIN tokens layout).
/// * `wheel_template_count` — число шаблонов выбранной категории wheel-меню
///   (WHEEL extent зависит от count; `wheel_menu.category` меняется кликом
///   без смены флага `WHEEL_OPEN`).
///
/// FR-106 (C3): `Copy` снят — `manager_filter: String` (фильтр меняет
/// состав hit-строк менеджера; строка не Copy, сравнение — PartialEq).
#[derive(Debug, Clone, PartialEq)]
pub struct UiFrameSig {
    /// Логический viewport (w, h) — влияет на все layout-функции.
    pub viewport: [f32; 2],
    /// `SceneState::revision` — ревизия модели (правки нод/рёбер/групп).
    pub scene_revision: u64,
    /// Камера (center_x, center_y, zoom) — PALETTE через world_to_screen.
    pub camera: [f32; 3],
    /// Битовые флаги открытых поверхностей (см. [`ui_frame_flags`]).
    /// u64 — биты 0..31 исчерпаны (LAY-W1: +AGENT_PANEL_OPEN/AI_STATUS_VISIBLE).
    pub flags: u64,
    /// Хэш выделения (selected + selected_nodes) — для PALETTE.
    pub selection_hash: u64,
    /// Активный whatif-сценарий (-1 = None/«База»).
    pub whatif_active: i32,
    /// Число whatif-сценариев (`scene.scenarios.len()`).
    pub whatif_scenario_count: u32,
    /// Число замороженных whatif-сценариев (`scene.frozen.len()`).
    pub whatif_frozen_count: u32,
    /// Число построчных подмен активного сценария (`whatif_override_count`).
    pub whatif_override_count: u32,
    /// Скролл списка карты проливаний (`flow_map_scroll.offset`).
    pub flow_map_scroll_offset: f32,
    /// Раскрытая категория template-strip (-1 = None, иначе индекс).
    pub template_hover_open: i32,
    /// Скролл flyout'а template-strip (`template_hover.scroll.first`).
    pub template_hover_scroll: u32,
    /// Раскрытая hover'ом группа палитры (-1 = None, иначе индекс).
    pub palette_hover_open: i32,
    /// Шаг онбординг-тура (0 если `ONBOARDING_OPEN` сброшен).
    pub onboarding_step: u32,
    /// Язык интерфейса (`settings.language`).
    pub language: canvas_core::Language,
    /// Позиция угловых кнопок (`settings.button_corner`).
    pub button_corner: canvas_core::Corner,
    /// Активный таб настроек (`settings_tab`).
    pub settings_tab: u32,
    /// Скролл списка результатов поиска (`search.scroll_top`).
    pub search_scroll_top: u32,
    /// Число строк поиска (`search.rows.len()`).
    pub search_rows_count: u32,
    /// Активная секция админпанели.
    pub admin_section: crate::admin_ui::AdminSection,
    /// Число шаблонов выбранной категории wheel-меню (0 — категория не
    /// выбрана; влияет на `wheel_geometry.extent` → WHEEL hit-rect).
    pub wheel_template_count: u32,
    /// LAY-W1: снапшот AI-фич (бит 0 — suggest, 1 — graph, 2 — agent,
    /// 3 — пауза): ширины чипов AI-статус-панели (`feat_chips_measured`)
    /// зависят от них — hit-rect'ы поверхности меняются без смены
    /// `scene_revision`.
    pub ai_feats: u8,
    /// LAY-W17: скролл колонки «Переменные» панели «Как считается»
    /// (offset px, `stage_calc_vars_scroll.offset`). Видимые строки
    /// колонки — hit-rect'ы поверхности STAGE — скролл-зависимы; без поля
    /// кэш кадра отдал бы устаревшие rect'ы после прокрутки колесом.
    pub stage_calc_vars_offset: f32,
    /// LAY-W17: скролл колонки «Расчёт» панели «Как считается» (offset px,
    /// `stage_calc_formulas_scroll.offset`) — см. [`Self::stage_calc_vars_offset`].
    pub stage_calc_formulas_offset: f32,
    /// FR-105 (C2): скролл окна чекбокс-листа миграции (`migrate.scroll_top`
    /// — MIGRATE hit-строки двигаются без смены состава кадра).
    pub migrate_scroll_top: u32,
    /// FR-105 (C2): выбранный ординал миграции + длина листинга + счётчик
    /// галочек (упаковка: selected<<40 | checked<<24 | len; hit-строки и
    /// стили зависят от них).
    pub migrate_pack: u64,
    /// FR-106 (C3): фильтр менеджера канвасов — состав hit-строк зависит
    /// от него (какие канвасы в списке), сравнение строк — дёшево.
    pub manager_filter: String,
    /// FR-106 (C3): упаковка состояния менеджера: selected<<48 |
    /// scroll_top<<32 | entries.len()<<16 | editing<<3 | sort<<2 |
    /// storage (0 — браузерное+FS, 1 — браузерное без FS, 2 — папка).
    pub manager_pack: u64,
}

/// FR-PERF-A: Простой нечётный миксер хэша (FNV-1a вариант) для примитивов.
/// Без аллокаций, без ветвлений (кроме цикла по selected_nodes). Достаточно
/// для обнаружения любого изменения состава выделения (порядок важен —
/// `selected_nodes` сохраняет порядок добавления).
fn mix_u64(mut state: u64, value: u64) -> u64 {
    state ^= value;
    state = state.wrapping_mul(0x9E3779B97F4A7C15);
    state.rotate_left(13)
}

/// FR-PERF-A: Хэш состояния выделения (`selected` + `selected_nodes`).
/// Определяет `palette_target()` (без блокировок модалей — те в `flags`):
/// None/Node(primary)/Edge(index) + состав `selected_nodes` (для bbox
/// `palette_anchor_screen`). Любое изменение → другая сигнатура → пересборка.
fn selection_hash(app: &App) -> u64 {
    let mut h = 0u64;
    match &app.selected {
        Some(Selection::Node(i)) => {
            h = mix_u64(h, 1);
            h = mix_u64(h, *i as u64);
        }
        Some(Selection::Edge(i)) => {
            h = mix_u64(h, 2);
            h = mix_u64(h, *i as u64);
        }
        None => h = mix_u64(h, 0),
    }
    h = mix_u64(h, app.selected_nodes.len() as u64);
    for &i in &app.selected_nodes {
        h = mix_u64(h, i as u64);
    }
    h
}

/// FR-PERF-A: Сигнатура инвалидации кэша UI-кадра под текущее состояние
/// приложения. Все вызовы `build_frame` в коде заменяются на `App::ui_frame`,
/// который сравнивает сигнатуру с кэшем: совпала — возвращает кэш, иначе —
/// пересборка и запись в кэш.
///
/// Полнота: поля покрывают все входы `build_registry` + `fill_hit_rects`.
/// Известные неучтённые факторы (редкие, без visual-регрессии):
/// * `app.templates` (TemplateRegistry) — меняется при импорте custom-шаблонов
///   (явная инвалидация `cached_ui_frame = None` в точке изменения).
/// * `app.dialog` содержимое (title/body инстанса) — стабильно на жизнь
///   инстанса; закрытие+открытие другого диалога меняет флаг `DIALOG_OPEN`.
/// * `app.scheme_gallery` (filter/category/scroll) — GALLERY hit-rect =
///   panel_rect, зависит только от viewport.
/// * `app.docs` (page/scroll) — DOCS hit-rect = viewer_rect(viewport).
/// * `app.help_menu.origin` — стабильно на жизнь инстанса.
/// * `app.menu.origin` / `app.choice_menu.origin` / `app.wheel_menu.screen`
///   — стабильно на жизнь инстанса.
pub fn build_frame_sig(app: &App) -> UiFrameSig {
    let viewport = app.viewport_logical();
    let camera_pos = app.camera.position();
    let camera_zoom = app.camera.zoom();
    // Битовые флаги открытых поверхностей — единственный источник
    // «какие поверхности в кадре». Порядок проверок — произвольный (OR).
    let mut flags: u64 = 0;
    if app.wheel_menu.is_some() {
        flags |= ui_frame_flags::WHEEL_OPEN;
    }
    if app.scene.whatif_active || whatif_pill_visible(app) {
        flags |= ui_frame_flags::WHATIF_ACTIVE;
    }
    if app.whatif_list_open {
        flags |= ui_frame_flags::WHATIF_LIST_OPEN;
    }
    if app.whatif_compare_open {
        flags |= ui_frame_flags::WHATIF_COMPARE_OPEN;
    }
    if app.hotkeys_open {
        flags |= ui_frame_flags::HOTKEYS_OPEN;
    }
    if app.settings_open {
        flags |= ui_frame_flags::SETTINGS_OPEN;
    }
    if app.settings_dropdown.open_row.is_some() {
        flags |= ui_frame_flags::SETTINGS_DROPDOWN_OPEN;
    }
    if app.menu.is_some() {
        flags |= ui_frame_flags::MENU_OPEN;
    }
    if app.menu.as_ref().and_then(|m| m.submenu.as_ref()).is_some() {
        flags |= ui_frame_flags::MENU_SUBMENU_OPEN;
    }
    if app.choice_menu.is_some() {
        flags |= ui_frame_flags::CHOICE_MENU_OPEN;
    }
    if app.template_panel.open {
        flags |= ui_frame_flags::TEMPLATE_PANEL_OPEN;
    }
    if app.palette_geometry().is_some() {
        flags |= ui_frame_flags::PALETTE_VISIBLE;
    }
    if app.palette_hover.open.is_some() {
        flags |= ui_frame_flags::PALETTE_HOVER_OPEN;
    }
    if app.docs.is_some() {
        flags |= ui_frame_flags::DOCS_OPEN;
    }
    if let Some(help) = &app.help_menu {
        flags |= ui_frame_flags::HELP_MENU_OPEN;
        if help.docs_open {
            flags |= ui_frame_flags::HELP_MENU_DOCS_OPEN;
        }
    }
    if app.main_stage.is_some() {
        flags |= ui_frame_flags::STAGE_OPEN;
    }
    if app.search.is_open() {
        flags |= ui_frame_flags::SEARCH_OPEN;
    }
    if app.flow_map_open {
        flags |= ui_frame_flags::FLOW_MAP_OPEN;
    }
    if app.editing.is_some() {
        flags |= ui_frame_flags::EDITOR_OPEN;
    }
    if app.explain.is_some() {
        flags |= ui_frame_flags::EXPLAIN_OPEN;
    }
    if app.autolink_review.is_some() {
        flags |= ui_frame_flags::AUTOLINK_REVIEW_OPEN;
    }
    if app.dialog.is_some() {
        flags |= ui_frame_flags::DIALOG_OPEN;
    }
    if app.scheme_gallery.open {
        flags |= ui_frame_flags::GALLERY_OPEN;
    }
    if app.kit_gallery_open {
        flags |= ui_frame_flags::KIT_GALLERY_OPEN;
    }
    if app.admin_open {
        flags |= ui_frame_flags::ADMIN_OPEN;
    }
    if app.onboarding.is_some() {
        flags |= ui_frame_flags::ONBOARDING_OPEN;
    }
    // LAY-W17: AI-онбординг в составе кадра.
    if app.ai_onboarding.is_some() {
        flags |= ui_frame_flags::AI_ONBOARDING_OPEN;
    }
    if app.empty_state_visible() {
        flags |= ui_frame_flags::EMPTY_VISIBLE;
    }
    if app.minimap_rect().is_some() {
        flags |= ui_frame_flags::MINIMAP_VISIBLE;
    }
    if app.autolink_badge_visible() {
        flags |= ui_frame_flags::AUTOLINK_BADGE_VISIBLE;
    }
    if app.dragging.is_some() {
        flags |= ui_frame_flags::DRAGGING;
    }
    if app.edge_drag.is_some() || app.select_rect.is_some() {
        flags |= ui_frame_flags::EDGE_OR_SELECT_DRAG;
    }
    // LAY-W1: панели AI в составе кадра (AGENT_PANEL/AI_STATUS).
    if app.agent_panel.open {
        flags |= ui_frame_flags::AGENT_PANEL_OPEN;
    }
    if !app.settings.llm.all_off() {
        flags |= ui_frame_flags::AI_STATUS_VISIBLE;
    }
    // FR-105 (C2): хранилище рабочего пространства (баннер/миграция/тост).
    if app.storage_banner.is_some() {
        flags |= ui_frame_flags::STORAGE_BANNER_VISIBLE;
    }
    if app.migrate.open {
        flags |= ui_frame_flags::MIGRATE_OPEN;
    }
    if app.toast_action.is_some() {
        flags |= ui_frame_flags::TOAST_ACTION_ACTIVE;
    }
    // FR-106 (C3): менеджер канвасов открыт — состав кадра меняется.
    if app.canvas_manager.open {
        flags |= ui_frame_flags::CANVAS_MANAGER_OPEN;
    }
    // Hover-раскрытия и скроллы отдельных панелей — влияют на hit-rect'ы
    // внутри поверхности (не на состав кадра).
    let (template_hover_open, template_hover_scroll) = match &app.template_hover {
        Some(h) => (h.open.map_or(-1, |i| i as i32), h.scroll.first as u32),
        None => (-1, 0),
    };
    let palette_hover_open = app.palette_hover.open.map_or(-1, |i| i as i32);
    let flow_map_scroll_offset = app.flow_map_scroll.offset;
    let onboarding_step = app.onboarding.as_ref().map(|s| s.step as u32).unwrap_or(0);
    // FR-087: счётчик секторов шаблонов — по видимым категориям (роль).
    let wheel_template_count = app
        .wheel_menu
        .as_ref()
        .and_then(|m| m.category.as_deref())
        .map(|c| app.visible_templates_by_category(c).len() as u32)
        .unwrap_or(0);
    // LAY-W1: снапшот AI-фич — ширины чипов AI-статус-панели.
    let ai_feats = u8::from(app.ai_suggest_enabled)
        | (u8::from(app.ai_graph_enabled) << 1)
        | (u8::from(app.ai_agent_enabled) << 2)
        | (u8::from(app.ai_paused) << 3);
    UiFrameSig {
        viewport,
        scene_revision: app.scene.revision,
        camera: [camera_pos[0], camera_pos[1], camera_zoom],
        flags,
        selection_hash: selection_hash(app),
        whatif_active: app.scene.active_scenario.map_or(-1, |i| i as i32),
        whatif_scenario_count: app.scene.scenarios.len() as u32,
        whatif_frozen_count: app.scene.frozen.len() as u32,
        whatif_override_count: app.scene.whatif_override_count() as u32,
        flow_map_scroll_offset,
        template_hover_open,
        template_hover_scroll,
        palette_hover_open,
        onboarding_step,
        language: app.settings.language,
        button_corner: app.settings.button_corner,
        settings_tab: app.settings_tab as u32,
        search_scroll_top: app.search.scroll_top as u32,
        search_rows_count: app.search.rows.len() as u32,
        admin_section: app.admin_section,
        wheel_template_count,
        ai_feats,
        // LAY-W17: скроллы колонок calc-панели — видимые строки STAGE.
        stage_calc_vars_offset: app.stage_calc_vars_scroll.offset,
        stage_calc_formulas_offset: app.stage_calc_formulas_scroll.offset,
        // FR-105 (C2): состояние диалога миграции (скролл/выбор/галочки)
        migrate_scroll_top: app.migrate.scroll_top as u32,
        migrate_pack: ((app.migrate.selected as u64) << 40)
            | ((app.migrate.checked.iter().filter(|c| **c).count() as u64) << 24)
            | (app.migrate.entries.len() as u64),
        // FR-106 (C3): состояние менеджера (фильтр меняет состав hit-строк;
        // скролл/выбор/редактирование/сортировка/режим — геометрию и стили)
        manager_filter: app.canvas_manager.filter.clone(),
        manager_pack: {
            let storage = match app.canvas_manager.storage {
                crate::canvas_manager_ui::StorageRowMode::Browser { fs_available: true } => 0u64,
                crate::canvas_manager_ui::StorageRowMode::Browser {
                    fs_available: false,
                } => 1,
                crate::canvas_manager_ui::StorageRowMode::Folder => 2,
            };
            ((app.canvas_manager.selected as u64) << 48)
                | ((app.canvas_manager.scroll_top as u64) << 32)
                | ((app.canvas_manager.entries.len() as u64) << 16)
                | (u64::from(app.canvas_manager.editing.is_some()) << 3)
                | (u64::from(app.canvas_manager.sort == canvas_core::workspace::SortMode::Name)
                    << 2)
                | storage
        },
    }
}

/// W-a (аудит §8 п.6): панель хоткеев с ИЗМЕРЕННЫМ расширением — единый
/// источник геометрии для отрисовки ([`super::overlays`]) и hit-rect
/// реестра (контракт «hit-rect'ы из тех же layout-функций, что
/// отрисовка»). Фикс среза 2026-09-25 (wasm-аудит, скриншот 11_hotkeys):
/// константная ширина 340 рвала самое длинное описание («…режим защиты:
/// раскрыть следующий уровень») у кромки — панель ДОТЯГИВАЕТСЯ до самого
/// длинного описания (измерение тем же лицом, что рисует строки). Раньше
/// измерение жило только в draw, реестр считал панель константной —
/// правая часть видимой панели не пикалась (нарушение «ввод = тому,
/// что видно»).
pub(crate) fn hotkeys_panel_rect(
    viewport: [f32; 2],
    left_offset: f32,
    corner: canvas_core::Corner,
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    tr: impl Fn(&'static str) -> &'static str,
) -> [f32; 4] {
    let mut panel = crate::ui::hotkeys_panel_rect_at(viewport, left_offset);
    let longest = crate::ui::HOTKEYS
        .iter()
        .map(|(_, d)| m.width_of(fs, tr(d), crate::admin_ui::FONT_FAMILY, 12.0))
        .fold(0.0_f32, f32::max);
    let desired =
        (crate::ui::HOTKEYS_PADDING * 2.0 + crate::ui::HOTKEYS_KEY_COLUMN + longest + 2.0)
            .min(viewport[0]);
    panel[2] = panel[2].max(desired);
    // FR-054 (гейт G4 «0 налезаний одной полосы»): дотянутая панель на
    // узких окнах (800×560, RU) наезжала на угловой кластер кнопок
    // ⚙/тема/язык/«?» — та же полоса Panels. Налезание было только
    // ВИЗУАЛЬНЫМ с фикса среза 2026-09-25: hit-rect реестра оставался
    // константным, линт слепого кадра его не видел; синхронизация pick с
    // draw (эта функция) сделала налезание видимым линту. Расширение
    // клампится левой кромкой кластера (зазор SETTINGS_GAP); у левых
    // углов кластер не мешает правому краю панели — просто отступ от
    // кромки вьюпорта. Цена компромисса: на самом узком окне G4 самое
    // длинное RU-описание снова частично срезается — перекрытие
    // интерактивных кнопок хуже срезанного текста.
    let right_limit = match corner {
        canvas_core::Corner::TopRight | canvas_core::Corner::BottomRight => {
            (crate::ui::help_button_rect(corner, viewport)[0] - crate::ui::SETTINGS_GAP).max(0.0)
        }
        canvas_core::Corner::TopLeft | canvas_core::Corner::BottomLeft => {
            (viewport[0] - crate::ui::SETTINGS_MARGIN).max(0.0)
        }
    };
    panel[2] = panel[2].min((right_limit - panel[0]).max(0.0));
    panel
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
            // FR-087: геометрия wheel — по видимым категориям (роль),
            // паритет с рендером wheel_overlay и кликами click_wheel_menu.
            let categories = app.template_category_names();
            let template_count = app
                .wheel_menu
                .as_ref()
                .and_then(|m| m.category.as_deref())
                .map(|c| app.visible_templates_by_category(c).len())
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
            // W-a: ширина — ИЗМЕРЕННАЯ, общая функция с отрисовкой
            // (pick = видимой панели).
            // ПОРЯДОК ВАЖЕН: hotkeys_left_offset сам захватывает общий
            // FontSystem (dock_strip_layout меряет полосу доков) — вызванный
            // под уже захваченным guard'ом measure_font_system он
            // самозаблокировал бы неповторно-входящий Mutex.
            let left = app.hotkeys_left_offset(viewport);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let panel = hotkeys_panel_rect(
                viewport,
                left,
                app.settings.button_corner,
                &mut m,
                &mut fs,
                |k| app.tr(k),
            );
            surface
                .hit_rects
                .push(HitRect::interactive(rect(panel), "hotkeys-panel"));
        }
        id::CORNER_BUTTONS => {
            // LAY-W1: кластер ⚙/тема/язык/«?» в ПРАВЫХ углах накрывается
            // непрозрачной агент-панелью во всю высоту (draw-порядок полосы:
            // settings_overlay раньше agent_panel_overlay) и не кликабелен
            // (agent_panel_click глотает клики в rect панели раньше pick) —
            // «ввод = тому, что видно» (П5/LAY1.2): hit-rect'ы не
            // публикуются, иначе G4-линт видел бы налезание ⚙ на ✕ панели.
            // Продуктовый вопрос «хром недоступен при открытой
            // агент-панели» — вне волны (на web DOM-бары разводит
            // html_panel_overlap — app.rs).
            let agent_covers = app.agent_panel_rect().is_some()
                && matches!(
                    app.settings.button_corner,
                    canvas_core::Corner::TopRight | canvas_core::Corner::BottomRight
                );
            if !agent_covers {
                surface.hit_rects.push(HitRect::interactive(
                    rect(theme_button_rect(app.settings.button_corner, viewport)),
                    "theme-button",
                ));
                // FR-040 v2: кнопка переключения языка (RU/EN) — между темой и «?»
                surface.hit_rects.push(HitRect::interactive(
                    rect(language_button_rect(app.settings.button_corner, viewport)),
                    "language-button",
                ));
                surface.hit_rects.push(HitRect::interactive(
                    rect(help_button_rect(app.settings.button_corner, viewport)),
                    "help-button",
                ));
                surface.hit_rects.push(HitRect::interactive(
                    rect(button_rect(app.settings.button_corner, viewport)),
                    "settings-button",
                ));
            }
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
            // W-a: hit-rect'ы по той же scrolled-раскладке, что рисование
            // (overlays.rs) и pick (input.rs) — один offset, «ввод = тому,
            // что видно»; dropdown-якорь уезжает со строкой.
            // FR-LLM-FIX: настройки для фильтрации AI-таба (BYOK-модель).
            let layout = modal_layout_scrolled_with_settings(
                app.settings_tab,
                viewport,
                app.settings_scroll_top,
                &app.settings,
            );
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
            // FR-087: строки/чипы — видимые категории (роль/ручной фильтр).
            let rows = template_panel_rows(
                &app.templates,
                &app.template_panel,
                app.settings.language,
                &app.template_category_names(),
            );
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
                &app.template_category_names(),
                &app.template_category_display_names(),
            );
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "template-panel"));
        }
        id::TEMPLATE_STRIP => {
            // FR-040 v2: геометрия по локализованным подписям (паритет
            // с рендером/hit-test дока).
            let categories = app.template_category_display_names();
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
            // LAY-W17 (ревью §3.4, «calc — строки мимо кадра»): строки
            // панели «Как считается» — в кадре реестра. Регресс-аудит
            // click_main_stage: pick внутри окна stage и прежде отдавал
            // Element{STAGE,"stage"} (окно = pick-зона), click_main_stage
            // игнорирует имя элемента и пере-хит-тестит cursor той же
            // stage_frame_ctx — строки не меняют ни адресата, ни
            // обработчик клика. Колесо — ранний return до pick (input.rs,
            // stage_calc_wheel_scroll), hover-глушение absorbs не меняется
            // (строки — подмножество окна). Скролл-зависимость видимых
            // строк покрыта сигнатурой (stage_calc_vars_offset /
            // stage_calc_formulas_offset). Строки пушатся ПОСЛЕ окна —
            // top_hit_at отдаёт строке клик внутри неё; элемент несёт
            // МОДЕЛЬНЫЙ индекс строки (как StageCalcFocus —
            // скролл-независимый).
            if let Some(stage) = &app.main_stage {
                // Защита от вырожденных срезов (hand-constructed состояния
                // тестов: slice без нод) — stage_frame_ctx читает nodes[0..2]:
                // прод-путь MainStageState::open гарантирует ≥ 2 ноды.
                if stage.slice.nodes.len() >= 2 {
                    let ctx = app.stage_frame_ctx(stage, &r, stage.scale.max(f32::EPSILON));
                    if let Some(panel) = &ctx.panel {
                        for (i, (_, row)) in panel.var_rows.iter().enumerate() {
                            surface.hit_rects.push(HitRect::interactive(
                                UiRect::new(r.x + row[0], r.y + row[1], row[2], row[3]),
                                format!("calc-var-row-{i}"),
                            ));
                        }
                        for (i, (_, row)) in panel.formula_rows.iter().enumerate() {
                            surface.hit_rects.push(HitRect::interactive(
                                UiRect::new(r.x + row[0], r.y + row[1], row[2], row[3]),
                                format!("calc-formula-row-{i}"),
                            ));
                        }
                    }
                }
            }
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
            // FR-087: строки и чипы галереи — видимые категории (роль/фильтр).
            let list = scheme_gallery_ui::rows(
                registry,
                &app.scheme_gallery,
                &app.visible_scheme_categories(),
            );
            let lay = scheme_gallery_ui::layout(
                viewport,
                &list,
                &app.scheme_gallery,
                app.settings.language == canvas_core::Language::Ru,
                &app.visible_scheme_categories(),
            );
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel_rect), "gallery-panel"));
        }
        // FR-105 (C2, №44b): баннер потери доступа — интерактивны кнопки
        // «Переподключить»/«Переключиться в браузерное» (геометрия —
        // storage_ui + измеренные ширины, как у отрисовки; клик мимо
        // кнопок проваливается — баннер не блокирует канвас).
        id::STORAGE_BANNER => {
            let lang = app.settings.language;
            let label = crate::i18n::tr(lang, keys::CANVAS_STORAGE_LOST_BANNER);
            let reconnect = crate::i18n::tr(lang, keys::CANVAS_STORAGE_RECONNECT);
            let switch = crate::i18n::tr(lang, keys::CANVAS_STORAGE_SWITCH_BROWSER);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let family = canvas_render::text::SANS_FAMILY;
            let font = canvas_core::tokens::FONT_BODY;
            let label_w = m.width_of(&mut fs, label, family, font);
            let rec_w = canvas_ui::kit::button_size(reconnect, &mut m, &mut fs, family, font).x;
            let sw_w = canvas_ui::kit::button_size(switch, &mut m, &mut fs, family, font).x;
            let lay = crate::storage_ui::storage_banner_layout(viewport, label_w, rec_w, sw_w);
            surface.hit_rects.push(HitRect::interactive(
                rect(lay.reconnect),
                "storage-reconnect",
            ));
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.switch), "storage-switch"));
        }
        // FR-105 (C2, №42a/№52a): диалог миграции — панели-кнопка (тело,
        // глотает клик), строки чекбокс-листа, «✕», «Переехать…»/«Отмена»
        // (геометрия — storage_ui::migrate_layout + измеренные кнопки).
        id::MIGRATE => {
            let lang = app.settings.language;
            let go = crate::i18n::tr(lang, keys::CANVAS_STORAGE_MOVE_TO_DISK);
            let cancel = crate::i18n::tr(lang, keys::DIALOG_CANCEL);
            let mut m = canvas_ui::measure::TextMeasurer::new();
            let mut fs = canvas_render::text::measure_font_system();
            let family = canvas_render::text::SANS_FAMILY;
            let font = canvas_core::tokens::FONT_BODY;
            let go_w = canvas_ui::kit::button_size(go, &mut m, &mut fs, family, font).x;
            let cancel_w = canvas_ui::kit::button_size(cancel, &mut m, &mut fs, family, font).x;
            let lay = crate::storage_ui::migrate_layout(viewport, &app.migrate, go_w, cancel_w);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel), "migrate-panel"));
            for (index, row) in lay.rows.iter().enumerate() {
                surface.hit_rects.push(HitRect::interactive(
                    rect(row.row),
                    format!("migrate-row-{}", lay.first_row + index),
                ));
            }
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.close), "migrate-close"));
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.go), "migrate-go"));
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.cancel), "migrate-cancel"));
        }
        // FR-106 (C3): менеджер канвасов — тело панели (базовая pick-зона,
        // глотает клик — модаль жива), строки/зоны имён, кнопки, строка
        // хранилища (№51a). Порядок: тело первой, затем строки, зоны имён
        // ПОСЛЕ строк (реверс-обход pick'а — имя выигрывает у строки) и
        // кнопки последними.
        id::CANVAS_MANAGER => {
            let lay = app.manager_layout_current(viewport);
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.panel), "manager-panel"));
            for row in &lay.rows {
                if let Some(entry) = row.entry {
                    surface.hit_rects.push(HitRect::interactive(
                        rect(row.row),
                        format!("manager-row-{entry}"),
                    ));
                }
            }
            for row in &lay.rows {
                if let Some(entry) = row.entry {
                    surface.hit_rects.push(HitRect::interactive(
                        rect(row.name),
                        format!("manager-row-{entry}-name"),
                    ));
                }
            }
            if lay.has_entries {
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.create), "manager-create"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.template), "manager-template"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.import), "manager-import"));
                surface.hit_rects.push(HitRect::interactive(
                    rect(lay.duplicate),
                    "manager-duplicate",
                ));
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.rename), "manager-rename"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.delete), "manager-delete"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.export), "manager-export"));
            }
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.sort_chip), "manager-sort"));
            surface
                .hit_rects
                .push(HitRect::interactive(rect(lay.close), "manager-close"));
            if lay.storage_move[2] > 0.0 {
                surface.hit_rects.push(HitRect::interactive(
                    rect(lay.storage_move),
                    "manager-storage-move",
                ));
            }
            if let Some(empty) = &lay.empty {
                surface.hit_rects.push(HitRect::interactive(
                    rect(empty.create),
                    "manager-empty-create",
                ));
                surface.hit_rects.push(HitRect::interactive(
                    rect(empty.open_disk),
                    "manager-empty-disk",
                ));
            }
        }
        // FR-105 (C2, №45b): тост с действием — ТОЛЬКО кнопка
        // «Перезагрузить» (геометрия — storage_ui::toast_action_rect от
        // toast-области с whatif-avoid, как у отрисовки).
        id::TOAST => {
            if let Some((text, _)) = &app.toast {
                let lang = app.settings.language;
                // FR-106 (C3, №15a): подпись кнопки зависит от действия —
                // «Перезагрузить» (№45b) или «Отменить» (мягкое удаление).
                let label = match &app.toast_action {
                    Some(crate::app::ToastAction::UndoDelete(_)) => {
                        crate::i18n::tr(lang, keys::CANVAS_MANAGER_UNDO)
                    }
                    _ => crate::i18n::tr(lang, keys::CANVAS_EXT_RELOAD_ACTION),
                };
                let mut m = canvas_ui::measure::TextMeasurer::new();
                let mut fs = canvas_render::text::measure_font_system();
                let family = canvas_render::text::SANS_FAMILY;
                let font = crate::storage_ui::TOAST_ACTION_FONT;
                let action_w = canvas_ui::kit::button_size(label, &mut m, &mut fs, family, font).x;
                let text_w = m.width_of(&mut fs, text, family, font);
                let avoid = if app.scene.whatif_active {
                    Some(UiRect::new(
                        0.0,
                        viewport[1] - crate::whatif_ui::BAR_MARGIN - crate::whatif_ui::BAR_HEIGHT,
                        viewport[0],
                        crate::whatif_ui::BAR_HEIGHT,
                    ))
                } else {
                    None
                };
                let vp_rect = UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
                let mut toast = canvas_ui::kit::toast_area(vp_rect, avoid);
                // полоса с действием — высота кнопки (синхронно с рендером:
                // там клип растёт до TOAST_ACTION_STRIP_H, клип не режет низ)
                toast.h = crate::storage_ui::TOAST_ACTION_STRIP_H;
                let r = crate::storage_ui::toast_action_rect(
                    [toast.x, toast.y, toast.w, toast.h],
                    text_w,
                    action_w,
                );
                // FR-106 (C3, №15a): элемент кнопки зависит от действия
                // («Перезагрузить» №45b / «Отменить» мягкого удаления).
                let element = match &app.toast_action {
                    Some(crate::app::ToastAction::UndoDelete(_)) => "toast-undo",
                    _ => "toast-reload",
                };
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(r), element));
            }
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
            let (theme, close) =
                crate::kit_ui::gallery_hit_slots(viewport, &app.effective_palette().kit_palette());
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(theme.x, theme.y, theme.w, theme.h),
                "kit-gallery-theme",
            ));
            surface.hit_rects.push(HitRect::interactive(
                UiRect::new(close.x, close.y, close.w, close.h),
                "kit-gallery-close",
            ));
            // Wave D v1 (issue #32): TO-BE — демо-хиты из кэша раскладки кадра
            // (тулбар/сайдбар/секции; draw==hit — та же GalleryLayout). Кэш
            // пуст (первый кадр до отрисовки) — хитов нет, next кадр добавит.
            if app.kit_demo.tobe {
                if let Some(demo) = app
                    .kit_demo_layout
                    .as_ref()
                    .and_then(|lay| lay.demo.as_ref())
                {
                    for (r, id) in &demo.demo_hits {
                        if r.w > 0.0 && r.h > 0.0 {
                            surface.hit_rects.push(HitRect::interactive(
                                *r,
                                format!("{}{}", crate::kit_demo::HIT_PREFIX, id),
                            ));
                        }
                    }
                }
            }
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
            let (theme, reset, close) =
                crate::admin_ui::admin_hit_slots(viewport, &app.admin_effective_palette());
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
                // W-e: раскладка измеренная (общая с отрисовкой/вводом —
                // «ввод = тому, что видно»); скролл не нужен реестру
                // (кнопки не двигаются — футер прибит к низу карточки).
                let mut m = canvas_ui::measure::TextMeasurer::new();
                let mut fs = canvas_render::text::measure_font_system();
                let lay = onboarding_ui::card_layout(
                    viewport,
                    state.step,
                    app.settings.language,
                    &app.settings.role,
                    &mut canvas_ui::kit::ScrollState::default(),
                    &mut m,
                    &mut fs,
                );
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.card), "onboarding-card"));
            }
        }
        // LAY-W17 (ревью §3.4): AI-онбординг — карточка + интерактивные
        // зоны из [`onboarding_ui::ai_onboarding_layout`] (та же чистая
        // функция, что draw `ai_onboarding_overlay` — «ввод = тому, что
        // видно»). Карточка — базовая pick-зона первой (клик по телу
        // карточки глотается Block-модалью, контракт ONBOARDING); карточки-
        // радио и кнопки пушатся после — top_hit_at отдаёт им клик внутри
        // их rect'ов. Блок privacy — декоративный текст (не интерактивен,
        // hit-тест `ai_onboarding_button_at` его не возвращает) — не
        // публикуется.
        id::AI_ONBOARDING => {
            if let Some(state) = &app.ai_onboarding {
                let lay = onboarding_ui::ai_onboarding_layout(viewport, state);
                surface
                    .hit_rects
                    .push(HitRect::interactive(rect(lay.card), "ai-onb-card"));
                for (i, mode) in lay.mode_cards.iter().enumerate() {
                    surface.hit_rects.push(HitRect::interactive(
                        rect(*mode),
                        format!("ai-onb-mode-{i}"),
                    ));
                }
                surface.hit_rects.push(HitRect::interactive(
                    rect(lay.btn_privacy),
                    "ai-onb-privacy",
                ));
                // «Продолжить» — disabled до выбора режима (прототип F-8):
                // rect публикуется как интерактивный — зона кнопки видна
                // линту G4 при любой геометрии карточки; гейт доступности
                // (state.selected) — обязанность обработчика, не реестра.
                surface.hit_rects.push(HitRect::interactive(
                    rect(lay.btn_continue),
                    "ai-onb-continue",
                ));
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
        // FR-LLM-D / LAY-W1 (аудит §3.8): агент-панель — интерактивные зоны
        // из ЕДИНОЙ раскладки AgentPanelLayout (та же, что draw
        // `agent_panel_overlay` и hit `agent_panel_hit`; UR-005). Preview
        // Accept/Reject в реестр не выносятся: их геометрия в hit-тесте
        // пока приблизительна (FR-LLM-D-TODO), канон линта — стабильный
        // скелет панели (тело панели — не pick-зона: клики внутри глотает
        // ранняя ветвь agent_panel_click, как прежде).
        id::AGENT_PANEL => {
            if let Some(panel) = app.agent_panel_rect() {
                let lay = agent_panel::AgentPanelLayout::build(UiRect::new(
                    panel[0], panel[1], panel[2], panel[3],
                ));
                surface
                    .hit_rects
                    .push(HitRect::interactive(lay.close, "agent-close"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(lay.input, "agent-input"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(lay.send, "agent-send"));
                for (i, q) in lay.quick.iter().enumerate() {
                    surface
                        .hit_rects
                        .push(HitRect::interactive(*q, format!("agent-quick-{i}")));
                }
            }
        }
        // FR-LLM / LAY-W1: AI-статус-панель — ⏸/⚙ (head_button_rects) +
        // чипы Suggest/Graph/Agent (единый замер feat_chip_rects) — те же
        // функции геометрии, что draw/`ai_status_panel_hit` (UR-005).
        id::AI_STATUS => {
            if let Some(panel) = app.ai_status_panel_rect() {
                let (pause, gear) = ai_status_panel::head_button_rects(panel);
                surface
                    .hit_rects
                    .push(HitRect::interactive(pause, "ai-status-pause"));
                surface
                    .hit_rects
                    .push(HitRect::interactive(gear, "ai-status-gear"));
                let mut m = canvas_ui::measure::TextMeasurer::new();
                let mut fs = canvas_render::text::measure_font_system();
                let chips = ai_status_panel::feat_chip_rects(
                    &mut m,
                    &mut fs,
                    panel,
                    app.ai_suggest_enabled,
                    app.ai_graph_enabled,
                    app.ai_agent_enabled,
                    app.ai_paused,
                );
                for (i, chip) in chips.iter().enumerate() {
                    surface
                        .hit_rects
                        .push(HitRect::interactive(*chip, format!("ai-status-chip-{i}")));
                }
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
///
/// FR-CLIP: каждая полоса несёт собственный `clip: UiRect` — рендерер
/// превращает его в scissor-бакет (`band_scissor_rect`), контент полосы
/// не выходит за границы поверхности. Пустой клип (`is_empty`) = полоса
/// рендерится в полном вьюпорте (историческое поведение); пустые
/// instances/texts + пустой клип — `push` пропускает.
#[derive(Default)]
pub(crate) struct ScreenBands {
    bands: Vec<(UiLayer, UiRect, Vec<CardInstance>, Vec<OwnedScreenText>)>,
}

impl ScreenBands {
    pub(crate) fn push(
        &mut self,
        layer: UiLayer,
        clip: UiRect,
        instances: Vec<CardInstance>,
        texts: Vec<OwnedScreenText>,
    ) {
        if instances.is_empty() && texts.is_empty() {
            return;
        }
        self.bands.push((layer, clip, instances, texts));
    }

    /// Полосы в порядке отрисовки: слои по возрастанию (стабильно —
    /// порядок регистрации контента внутри слоя сохранён).
    pub(crate) fn finish(
        mut self,
    ) -> Vec<(UiLayer, UiRect, Vec<CardInstance>, Vec<OwnedScreenText>)> {
        self.bands.sort_by_key(|(layer, _, _, _)| *layer);
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
            id::AI_ONBOARDING,
            id::EMPTY,
            id::MINIMAP,
            id::AGENT_PANEL,
            id::AI_STATUS,
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

    /// LAY-W17 (ревью §3.4): AI-онбординг в реестре — Modals/Block/Always
    /// (полноэкранная модаль адаптируется клампом, не прячется), hit-rect'ы
    /// карточки/режимов/кнопок из ai_onboarding_layout, backdrop-контракт
    /// как у онбординга (глотается без закрытия), флаг сигнатуры.
    #[test]
    fn ai_onboarding_surface_block_with_card_hit_rects() {
        let mut app = test_stub();
        app.onboarding = None;
        app.ai_onboarding = Some(crate::onboarding_ui::AiOnboardingState::default());
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::AI_ONBOARDING)
            .expect("ai_onboarding в реестре");
        assert_eq!(decl.layer, UiLayer::Modals);
        assert_eq!(decl.capture, CapturePolicy::Block);
        assert_eq!(decl.degradation, DegradationPolicy::Always);
        // Esc-стек: модаль присутствует (Block), закрывается раньше тура
        // онбординга (регистрация после ONBOARDING)
        let esc: Vec<String> = reg
            .esc_stack()
            .iter()
            .map(|sid| sid.as_str().to_owned())
            .collect();
        let pos = |name: &str| esc.iter().position(|s| s == name);
        if let (Some(a), Some(b)) = (pos(id::AI_ONBOARDING), pos(id::ONBOARDING)) {
            assert!(a < b, "AI-онбординг в esc-стеке выше тура онбординга");
        }

        let viewport = [1280.0_f32, 800.0];
        let frame = build_frame_at(&app, viewport);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::AI_ONBOARDING)
            .expect("ai_onboarding в кадре");
        let elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        for expected in [
            "ai-onb-card",
            "ai-onb-mode-0",
            "ai-onb-mode-1",
            "ai-onb-mode-2",
            "ai-onb-privacy",
            "ai-onb-continue",
        ] {
            assert!(elements.contains(&expected), "нет hit-rect {expected}");
        }
        // Pick-паритет: центр карточки-радио Cloud (индекс 1) → Element
        // ai-onb-mode-1 (верхний rect выигрывает у карточки-тела).
        let lay = crate::onboarding_ui::ai_onboarding_layout(
            viewport,
            app.ai_onboarding.as_ref().unwrap(),
        );
        let mode = lay.mode_cards[1];
        let c = UiPoint::new(mode[0] + mode[2] / 2.0, mode[1] + mode[3] / 2.0);
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::AI_ONBOARDING);
                assert_eq!(rect.element, "ai-onb-mode-1");
            }
            other => panic!("карточка-радио не пикается: {other:?}"),
        }
        // Backdrop-контракт Block-модали: угол экрана — Backdrop ai_onboarding.
        match HitStack::pick(&frame, UiPoint::new(2.0, 2.0)) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::AI_ONBOARDING);
            }
            other => panic!("ожидался Backdrop AI-онбординга, получено {other:?}"),
        }
        // Флаг сигнатуры: открытие/закрытие меняет состав кадра.
        assert_ne!(
            build_frame_sig(&app).flags & ui_frame_flags::AI_ONBOARDING_OPEN,
            0
        );
        app.ai_onboarding = None;
        assert_eq!(
            build_frame_sig(&app).flags & ui_frame_flags::AI_ONBOARDING_OPEN,
            0
        );
        assert!(build_registry(&app)
            .declarations()
            .iter()
            .all(|d| d.id.as_str() != id::AI_ONBOARDING));
    }

    /// LAY-W17 (ревью §3.4, «calc — строки мимо кадра»): строки панели
    /// «Как считается» — hit-rect'ы поверхности STAGE, внутри окна stage
    /// (пик = видимой строке); элемент — модельный индекс строки.
    /// Фикстура — как lint_calc_panel_open (пучок веса 2 + формула).
    #[test]
    fn stage_calc_rows_published_in_frame() {
        let mut app = test_stub();
        app.onboarding = None;
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
        assert!(app.main_stage.is_some(), "stage открыт на пучке веса 2");

        let viewport = [1280.0_f32, 800.0];
        let frame = build_frame_at(&app, viewport);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::STAGE)
            .expect("stage в кадре");
        let vars: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .filter(|e| e.starts_with("calc-var-row-"))
            .collect();
        let formulas: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .filter(|e| e.starts_with("calc-formula-row-"))
            .collect();
        assert_eq!(vars.len(), 2, "2 строки переменных (users, conv)");
        assert_eq!(formulas.len(), 1, "1 строка формулы (x = …)");
        // Строки внутри окна stage (регресс-аудит: pick-зона не шире окна).
        let r = main_stage_rect(viewport);
        for hit in surface.hit_rects.iter() {
            if hit.element.starts_with("calc-") {
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
        // Пик строки — Element{STAGE, calc-var-row-N} (адресат клика тот же
        // click_main_stage; элемент информативен для debug-оверлея).
        let var0 = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "calc-var-row-0")
            .expect("первая строка переменных");
        let c = UiPoint::new(
            var0.rect.x + var0.rect.w / 2.0,
            var0.rect.y + var0.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::STAGE);
                assert_eq!(rect.element, "calc-var-row-0");
            }
            other => panic!("строка calc-панели не пикается: {other:?}"),
        }
        // Клик МИМО окна — по-прежнему Backdrop (закрытие stage).
        match HitStack::pick(&frame, UiPoint::new(2.0, 2.0)) {
            Some(HitTarget::Backdrop { surface }) => {
                assert_eq!(surface.surface.as_str(), id::STAGE);
            }
            other => panic!("клик мимо окна stage обязан закрывать: {other:?}"),
        }
    }

    /// LAY-W17: сигнатура кадра отслеживает скроллы колонок calc-панели —
    /// видимые строки (hit-rect'ы STAGE) скролл-зависимы; без полей кэш
    /// отдал бы устаревшие rect'ы после прокрутки колесом.
    #[test]
    fn ui_frame_sig_tracks_stage_calc_scroll() {
        let mut app = test_stub();
        app.onboarding = None;
        let sig = build_frame_sig(&app);
        app.stage_calc_vars_scroll.offset = 26.0;
        assert_ne!(
            build_frame_sig(&app).stage_calc_vars_offset,
            sig.stage_calc_vars_offset,
            "скролл переменных = другая сигнатура"
        );
        app.stage_calc_formulas_scroll.offset = 52.0;
        assert_ne!(
            build_frame_sig(&app).stage_calc_formulas_offset,
            sig.stage_calc_formulas_offset,
            "скролл формул = другая сигнатура"
        );
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
        // LAY-W21: границы политики — строгий `<` (паттерн пинов 599/239/
        // 130-131 панелей): ровно на пороге видима, на 1 меньше — скрыта
        // (эта же политика — гейт whatif_pill_visible).
        assert!(decl.degradation.hidden_at(899.0, 600.0), "< 900 — скрыта");
        assert!(decl.degradation.hidden_at(900.0, 599.0), "< 600 — скрыта");
        assert!(
            !decl.degradation.hidden_at(900.0, 600.0),
            "ровно порог — видима"
        );
    }

    /// LAY8.2 (аудит LAY-W5): брейкпоинты модалки настроек объявлены В
    /// РЕЕСТРЕ ([`SETTINGS_BP_COMPACT`]/[`SETTINGS_BP_MOBILE`], рядом с
    /// декларацией SETTINGS), а не в теле отрисовки; значения канонические
    /// (1280 = вьюпорт гейта LAY8.1; 768 — порог мобайл-листа, W-e «A»);
    /// деградация SETTINGS — Always (настройки адаптируются, не прячутся).
    #[test]
    fn settings_breakpoints_registered_with_surface() {
        assert_eq!(SETTINGS_BP_COMPACT, 1280.0);
        assert_eq!(SETTINGS_BP_MOBILE, 768.0);
        let mut app = test_stub();
        app.onboarding = None;
        app.settings_open = true;
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::SETTINGS)
            .expect("открытая модалка настроек в реестре");
        assert_eq!(decl.degradation, DegradationPolicy::Always);
    }

    // --- LAY-W1 (аудит 2026-10 §3.8): панели AI в реестре -------------------

    /// LAY-W1: агент-панель зарегистрирована (Panels/Capture) с HideBelow
    /// { 600, 240 }: на 800×560 — в кадре с интерактивными зонами (✕/input/
    /// send/quick), ниже минимума — скрыта ЦЕЛИКОМ (0 rect'ов, LAY8 п.3–4);
    /// pick по ✕ — Element поверхности (пик = видимой панели).
    #[test]
    fn agent_panel_registry_hide_below_and_pick() {
        let mut app = test_stub();
        app.onboarding = None;
        app.agent_panel.open = true;
        // test_viewport — rect-функции панели читают viewport_logical()
        // (в headless-заглушке без окна он нулевой).
        app.test_viewport = Some([800.0, 560.0]);
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::AGENT_PANEL)
            .expect("агент-панель в реестре");
        assert_eq!(decl.layer, UiLayer::Panels);
        assert_eq!(decl.capture, CapturePolicy::Capture);
        assert!(!decl.degradation.hidden_at(800.0, 560.0));
        assert!(decl.degradation.hidden_at(599.0, 800.0), "< 600 — скрыта");
        assert!(decl.degradation.hidden_at(1280.0, 239.0), "< 240 — скрыта");

        let frame = build_frame_at(&app, [800.0, 560.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::AGENT_PANEL)
            .expect("панель в кадре на 800×560");
        let elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        for expected in [
            "agent-close",
            "agent-input",
            "agent-send",
            "agent-quick-0",
            "agent-quick-2",
        ] {
            assert!(elements.contains(&expected), "нет hit-rect {expected}");
        }
        // Курсор в центр ✕ → Element панели (пик = видимой панели).
        let close = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "agent-close")
            .expect("✕ панели");
        let c = UiPoint::new(
            close.rect.x + close.rect.w / 2.0,
            close.rect.y + close.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::AGENT_PANEL);
                assert_eq!(rect.element, "agent-close");
            }
            other => panic!("✕ агент-панели не пикается: {other:?}"),
        }
        // Ниже минимума — поверхности нет в кадре (0 rect'ов).
        let frame_small = build_frame_at(&app, [599.0, 560.0]);
        assert!(
            frame_small
                .surfaces
                .iter()
                .all(|s| s.surface.as_str() != id::AGENT_PANEL),
            "HideBelow < 600 — панель ЦЕЛИКОМ вне кадра"
        );
    }

    /// LAY-W1: AI-статус-панель (ambient: !all_off) с HideBelow { 900, 131 }:
    /// на 1024×640 — в кадре (⏸/⚙ + чипы), на 800×560 и ниже 131 — скрыта
    /// ЦЕЛИКОМ; all_off — поверхности нет (ambient-условие, не брейкпоинт).
    #[test]
    fn ai_status_registry_hide_below_and_pick() {
        let mut app = test_stub();
        app.onboarding = None;
        app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
        app.test_viewport = Some([1024.0, 640.0]);
        let reg = build_registry(&app);
        let decl = reg
            .declarations()
            .iter()
            .find(|d| d.id.as_str() == id::AI_STATUS)
            .expect("AI-статус в реестре (AI включён)");
        assert_eq!(decl.capture, CapturePolicy::Capture);
        assert!(!decl.degradation.hidden_at(1024.0, 640.0));
        assert!(decl.degradation.hidden_at(800.0, 560.0), "< 900 — скрыта");
        // Порог — derive из констант панели (стыковка LAY-W2):
        // AI_STATUS_H 100 + AI_STATUS_PAUSED_EXTRA 19 + AI_STATUS_MARGIN 12 = 131.
        assert!(
            !decl.degradation.hidden_at(1280.0, 131.0),
            "граница 131 — видима"
        );
        assert!(decl.degradation.hidden_at(1280.0, 130.0), "< 131 — скрыта");

        let frame = build_frame_at(&app, [1024.0, 640.0]);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::AI_STATUS)
            .expect("панель в кадре на 1024×640");
        let elements: Vec<&str> = surface
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        for expected in [
            "ai-status-pause",
            "ai-status-gear",
            "ai-status-chip-0",
            "ai-status-chip-2",
        ] {
            assert!(elements.contains(&expected), "нет hit-rect {expected}");
        }
        // Курсор в центр ⚙ → Element панели.
        let gear = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "ai-status-gear")
            .expect("⚙ панели");
        let c = UiPoint::new(
            gear.rect.x + gear.rect.w / 2.0,
            gear.rect.y + gear.rect.h / 2.0,
        );
        match HitStack::pick(&frame, c) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::AI_STATUS);
                assert_eq!(rect.element, "ai-status-gear");
            }
            other => panic!("⚙ AI-статуса не пикается: {other:?}"),
        }
        // 800×560 — скрыта ЦЕЛИКОМ.
        let small = build_frame_at(&app, [800.0, 560.0]);
        assert!(
            small
                .surfaces
                .iter()
                .all(|s| s.surface.as_str() != id::AI_STATUS),
            "HideBelow < 900 — панель ЦЕЛИКОМ вне кадра"
        );
        // AI выключен — поверхности нет (ambient-условие регистрации).
        let mut off = test_stub();
        off.onboarding = None;
        assert!(off.settings.llm.all_off());
        assert!(build_registry(&off)
            .declarations()
            .iter()
            .all(|d| d.id.as_str() != id::AI_STATUS));
    }

    /// LAY-W1: draw-гейт обеих панелей — политика HideBelow реестра
    /// (`agent_panel_visible` / `ai_status_visible`): ≥ минимума — рисуется,
    /// ниже — скрыта ЦЕЛИКОМ (0 квадов/текстов/иконок — LAY8 п.4).
    #[test]
    fn panels_draw_gated_by_hide_below() {
        let mut app = test_stub();
        app.onboarding = None;
        app.agent_panel.open = true;
        app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;

        // Агент-панель: ≥ 600×240 — рисуется.
        app.test_viewport = Some([1280.0, 800.0]);
        let (quads, _, _) = app.agent_panel_overlay();
        assert!(!quads.is_empty(), "агент-панель ≥ минимума — рисуется");
        // < 600 по ширине — 0 rect'ов.
        app.test_viewport = Some([599.0, 800.0]);
        let (quads, texts, icons) = app.agent_panel_overlay();
        assert!(
            quads.is_empty() && texts.is_empty() && icons.is_empty(),
            "агент-панель < 600 — скрыта ЦЕЛИКОМ"
        );
        // < 240 по высоте — 0 rect'ов.
        app.test_viewport = Some([1280.0, 239.0]);
        let (quads, texts, icons) = app.agent_panel_overlay();
        assert!(
            quads.is_empty() && texts.is_empty() && icons.is_empty(),
            "агент-панель < 240 — скрыта ЦЕЛИКОМ"
        );
        // Закрытая панель не рисуется и на большом вьюпорте.
        app.agent_panel.open = false;
        app.test_viewport = Some([1280.0, 800.0]);
        let (quads, _, _) = app.agent_panel_overlay();
        assert!(quads.is_empty(), "закрытая панель не рисуется");

        // AI-статус: ≥ 900×131 — рисуется.
        app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
        app.test_viewport = Some([1280.0, 800.0]);
        let (quads, _, _) = app.ai_status_panel();
        assert!(!quads.is_empty(), "AI-статус ≥ минимума — рисуется");
        // < 900 — 0 rect'ов.
        app.test_viewport = Some([899.0, 800.0]);
        let (quads, texts, icons) = app.ai_status_panel();
        assert!(
            quads.is_empty() && texts.is_empty() && icons.is_empty(),
            "AI-статус < 900 — скрыта ЦЕЛИКОМ"
        );
        // < 131 — 0 rect'ов.
        app.test_viewport = Some([1280.0, 130.0]);
        let (quads, texts, icons) = app.ai_status_panel();
        assert!(
            quads.is_empty() && texts.is_empty() && icons.is_empty(),
            "AI-статус < 131 — скрыта ЦЕЛИКОМ"
        );
        // AI выключен — не рисуется (ambient-условие, не брейкпоинт).
        app.settings.llm = canvas_llm::LlmSettings::default();
        app.test_viewport = Some([1280.0, 800.0]);
        let (quads, _, _) = app.ai_status_panel();
        assert!(quads.is_empty(), "all_off — панель не рисуется");
    }

    /// LAY-W1: кластер угловых кнопок в ПРАВЫХ углах под агент-панелью не
    /// публикует hit-rect'ы — «ввод = тому, что видно» (панель непрозрачна и
    /// глотает клики раньше pick); для левых углов и при закрытой панели
    /// кластер кликабелен (4 rect'а).
    #[test]
    fn corner_buttons_suppressed_under_agent_panel() {
        let mut app = test_stub();
        app.onboarding = None;
        app.agent_panel.open = true;
        app.test_viewport = Some([1280.0, 800.0]);
        let corners = |app: &App| {
            build_frame_at(app, [1280.0, 800.0])
                .surfaces
                .iter()
                .find(|s| s.surface.as_str() == id::CORNER_BUTTONS)
                .expect("кластер в кадре")
                .hit_rects
                .len()
        };
        assert_eq!(corners(&app), 0, "кластер под панелью — 0 hit-rect'ов");
        // Левый угол: панель кластер не накрывает — rect'ы на месте.
        app.settings.button_corner = canvas_core::Corner::TopLeft;
        assert_eq!(corners(&app), 4, "левый угол — кластер кликабелен");
        // Панель закрыта — правый угол снова в кадре.
        app.settings.button_corner = canvas_core::Corner::TopRight;
        app.agent_panel.open = false;
        assert_eq!(corners(&app), 4, "панель закрыта — кластер кликабелен");
    }

    /// LAY-W1: сигнатура кадра реагирует на открытие агент-панели, состав
    /// AI-статуса (all_off ↔ включён) и тумблеры/паузу AI-фич — ширины чипов
    /// панели = hit-rect'ы, без инвалидации кэш отдал бы устаревший кадр.
    #[test]
    fn ui_frame_sig_tracks_agent_and_ai_status() {
        let mut app = test_stub();
        app.onboarding = None;
        app.agent_panel.open = true;
        assert_ne!(
            build_frame_sig(&app).flags & ui_frame_flags::AGENT_PANEL_OPEN,
            0
        );
        app.agent_panel.open = false;
        assert_eq!(
            build_frame_sig(&app).flags & ui_frame_flags::AGENT_PANEL_OPEN,
            0
        );
        // AI включён — AI_STATUS_VISIBLE; тумблер фичи меняет сигнатуру.
        app.settings.llm.provider_suggest = canvas_llm::LlmProviderId::Laya;
        let sig_ai = build_frame_sig(&app);
        assert_ne!(sig_ai.flags & ui_frame_flags::AI_STATUS_VISIBLE, 0);
        assert_ne!(sig_ai.ai_feats & 0b0001, 0, "suggest включён");
        app.ai_suggest_enabled = false;
        assert_ne!(
            build_frame_sig(&app).ai_feats,
            sig_ai.ai_feats,
            "тумблер чипа = другая геометрия hit-rect'ов"
        );
        app.ai_paused = true;
        assert_ne!(build_frame_sig(&app).ai_feats & 0b1000, 0, "пауза");
    }

    /// W-a (аудит §8 п.6): pick-rect реестра панели хоткеев == draw-rect —
    /// общая функция [`hotkeys_panel_rect`] с измеренным расширением.
    /// Регресс: реестр считал панель константной ширины
    /// (hotkeys_panel_rect_at) — правая часть дотянутой панели не
    /// пикалась (нарушение «ввод = тому, что видно»).
    #[test]
    fn hotkeys_hit_rect_matches_draw_rect() {
        let mut app = test_stub();
        app.onboarding = None;
        app.hotkeys_open = true;
        // канонический вьюпорт G4 (test_stub без окна — вьюпорт [0,0])
        let viewport = [1280.0_f32, 800.0];
        let frame = build_frame_at(&app, viewport);
        let surface = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::HOTKEYS)
            .expect("панель хоткеев в кадре");
        let hit = surface
            .hit_rects
            .iter()
            .find(|r| r.element == "hotkeys-panel")
            .expect("hit-rect панели");
        // left_offset ДО захвата FontSystem (см. порядок в fill_hit_rects:
        // hotkeys_left_offset сам берёт этот же Mutex — вложенный захват =
        // самозаблокировка неповторно-входящего Mutex). Константная база
        // тоже считается до захвата — guard fs живёт до конца теста.
        let left = app.hotkeys_left_offset(viewport);
        let base = crate::ui::hotkeys_panel_rect_at(viewport, left);
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let draw = hotkeys_panel_rect(
            viewport,
            left,
            app.settings.button_corner,
            &mut m,
            &mut fs,
            |k| app.tr(k),
        );
        assert_eq!(
            (hit.rect.x, hit.rect.y, hit.rect.w, hit.rect.h),
            (draw[0], draw[1], draw[2], draw[3]),
            "pick = видимой панели"
        );
        // Расширение реально работает: измеренная ширина строго больше
        // константной базы (длинные RU-описания не влезают в 340).
        assert!(
            draw[2] > base[2],
            "измеренное расширение шире константной панели"
        );
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
        let (quads, _, _) = app.admin_panel_overlay();
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
        let (quads, _, _) = app.kit_gallery_overlay();
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

    /// FR-105 (мультиканвас C2): поверхности хранилища — баннер потери
    /// доступа (Panels/Capture: hit-rect'ы ТОЛЬКО кнопки — канвас под
    /// баннером жив, №44b), диалог миграции (Modals/Block, №42a) и тост
    /// с действием (Toasts/Capture: интерактивна только кнопка, №45b);
    /// без действия строка тоста в реестре не появляется (пассивна).
    #[test]
    fn storage_surfaces_hit_rects() {
        let mut app = test_stub();
        // авто-показ онбординга первого запуска не участвует (FR-028);
        // его Block-модаль выше MIGRATE в VISUAL_ORDER — снят, чтобы
        // пик строки миграции шёл в сам диалог
        app.onboarding = None;
        // №44b: баннер потери доступа — Capture, кнопки пикаются
        app.storage_banner = Some("permission".to_owned());
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let banner = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::STORAGE_BANNER)
            .expect("баннер в кадре");
        assert_eq!(banner.capture, CapturePolicy::Capture);
        let elements: Vec<&str> = banner
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        assert!(elements.contains(&"storage-reconnect"), "кнопка есть");
        assert!(elements.contains(&"storage-switch"), "кнопка есть");
        assert!(
            !elements.iter().any(|e| !e.starts_with("storage-")),
            "интерактивны только кнопки (канвас под баннером жив)"
        );
        app.storage_banner = None;

        // №42a: диалог миграции — Block, тело/строки/кнопки пикаются
        app.migrate.open_with("рабочий.canvas");
        let entries: Vec<_> = (0..3)
            .map(|i| canvas_core::workspace::CanvasEntry {
                name: format!("c{i}.canvas"),
                ts: i as u64,
                kind: canvas_core::workspace::EntryKind::Opfs,
                repo: None,
            })
            .collect();
        app.migrate.set_entries(entries);
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let migrate = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::MIGRATE)
            .expect("диалог миграции в кадре");
        assert_eq!(migrate.capture, CapturePolicy::Block);
        let elements: Vec<&str> = migrate
            .hit_rects
            .iter()
            .map(|r| r.element.as_str())
            .collect();
        for expected in [
            "migrate-panel",
            "migrate-row-0",
            "migrate-go",
            "migrate-cancel",
            "migrate-close",
        ] {
            assert!(elements.contains(&expected), "нет hit-rect {expected}");
        }
        // клик по строке пикается в диалог (ввод = тому, что видно)
        let row = migrate
            .hit_rects
            .iter()
            .find(|r| r.element == "migrate-row-0")
            .expect("строка списка");
        let center = UiPoint::new(row.rect.x + 2.0, row.rect.y + row.rect.h / 2.0);
        match HitStack::pick(&frame, center) {
            Some(HitTarget::Element { surface, rect }) => {
                assert_eq!(surface.surface.as_str(), id::MIGRATE);
                assert_eq!(rect.element, "migrate-row-0");
            }
            other => panic!("строка миграции не пикается: {other:?}"),
        }
        app.migrate.close();

        // №45b: тост с действием — Capture, интерактивна только кнопка
        app.toast = Some(("файл изменился".to_owned(), std::time::Instant::now()));
        app.toast_action = Some(ToastAction::ReloadExternal);
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        let toast = frame
            .surfaces
            .iter()
            .find(|s| s.surface.as_str() == id::TOAST)
            .expect("тост с действием в кадре");
        assert_eq!(toast.capture, CapturePolicy::Capture);
        assert_eq!(toast.hit_rects.len(), 1, "интерактивна только кнопка");
        assert_eq!(toast.hit_rects[0].element, "toast-reload");
        // hit-зона — вся кнопка (высота полосы = кнопке кита: рендер
        // растит клип до TOAST_ACTION_STRIP_H, scissor не режет низ)
        assert!(
            (toast.hit_rects[0].rect.h - crate::storage_ui::TOAST_ACTION_STRIP_H).abs()
                < f32::EPSILON
        );
        // без действия — поверхности TOAST нет (строка пассивна)
        app.toast_action = None;
        let frame = build_frame_at(&app, [1280.0, 800.0]);
        assert!(
            frame
                .surfaces
                .iter()
                .all(|s| s.surface.as_str() != id::TOAST),
            "обычный тост не интерактивен"
        );
    }
}
