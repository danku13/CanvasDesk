//! FR-068 W3: компонентная модель — [`Component`] trait + общие типы кита
//! ([`KitState`]/[`KitPalette`]/стили/константы-токены).
//!
//! Компоненты — [`button`], [`panel`], [`dropdown`], [`modal`],
//! [`text_field`], [`list`], [`row`] — владеют собственной логикой
//! вёрстки/стилей/отрисовки (перенос из `kit.rs` 1:1, W3); `kit.rs` — тонкий
//! фасад-реэкспорт: публичное API кита сохранено (§Контракт-1 PRD-0009 V-5,
//! потребители не переписываются).
//!
//! Retained-state: решение по плану FR-068 §«Retained-state (опционально)» —
//! НЕ вводится: профиль W2 — reflow 1000 узлов на `FlexLayoutEngine`
//! ~0.26 мс < 1 мс порога (KISS; порог §Гейты W2/§Контракт-8).

pub mod accordion;
pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod chat_bubble;
pub mod checkbox;
pub mod chip;
pub mod command_palette;
pub mod crumbs;
pub mod dropdown;
pub mod footer;
pub mod icon;
pub mod list;
pub mod modal;
pub mod nav_rail;
pub mod panel;
pub mod panel_header;
pub mod popover;
pub mod progress;
pub mod radio;
pub mod radio_card;
pub mod row;
pub mod segmented;
pub mod skeleton;
pub mod slider;
pub mod snackbar;
pub mod table;
pub mod tabs;
#[cfg(test)]
pub mod test_support;
pub mod text_field;
pub mod tree;
pub mod two_column;

use crate::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
// Wave A-фикс: nav_rail.rs импортирует бейдж-типы с корня компонента
// (`crate::component::{BadgeKind, BadgeTone}`) — реэкспорт submodule.
pub use self::badge::{BadgeKind, BadgeTone};

// --- Состояния и стили ------------------------------------------------------

/// Состояние интерактивного виджета кита (слоты состояний палитры — FR-053).
///
/// Wave T (2026-10, ui-kit-deep-review §5.1.1): расширено с 5 до 7 значений
/// (+ Focused, Dragged) по Material 3 interaction-states spec. Состояния
/// **additive** — могут стекаться (hover+focused+selected одновременно);
/// [`WidgetState::active_states`](crate::widget::WidgetState::active_states)
/// возвращает все активные. [`WidgetState::kit_state`] (deprecated alias)
/// возвращает «highest-priority»单一 состояние для обратной совместимости.
///
/// **State-layer** (M3): hover/focus/pressed/dragged выражаются через
/// opacity-overlay 8/10/10/16% поверх content/accent — см. [`state_layer_alpha`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KitState {
    /// Обычное.
    Normal,
    /// Курсор над виджетом.
    Hovered,
    /// Keyboard-фокус (Tab-навигация). Рисует focus-ring по слоту `accent`
    /// и state-layer 10%. `:focus-visible` семантика — только keyboard-origin:
    /// mouse-click не активирует Focused (см. [`WidgetState::set_focused`]).
    Focused,
    /// Кнопка зажата.
    Pressed,
    /// Элемент перетаскивается (drag-операция в процессе). State-layer 16%.
    Dragged,
    /// Недоступен (клик игнорируется потребителем).
    Disabled,
    /// Выбран (тоглы, строки списков, чипы фильтров).
    Selected,
    /// Ошибка валидации (text_field с неверным значением). Слот `control_danger`.
    Error,
}

/// State-layer opacity для Material 3 interaction-states spec (Wave T §5.1.1).
///
/// Возвращает alpha-overlay поверх content/accent цвета для каждого состояния:
/// - `Hovered` → 0.08 (8%)
/// - `Focused` → 0.10 (10%)
/// - `Pressed` → 0.10 (10%)
/// - `Dragged` → 0.16 (16%)
/// - остальные → 0.0 (нет overlay)
///
/// Механизм: consumer рисует base-fill, затем overlay-квад с этой alpha
/// поверх (цвет overlay = content или accent). Единая механика для всех
/// компонентов — не новые цвета, а opacity-ступени.
pub fn state_layer_alpha(state: KitState) -> f32 {
    match state {
        KitState::Hovered => 0.08,
        KitState::Focused => 0.10,
        KitState::Pressed => 0.10,
        KitState::Dragged => 0.16,
        KitState::Normal | KitState::Disabled | KitState::Selected | KitState::Error => 0.0,
    }
}

/// Composite state resolver (Wave T §5.1.1). Возвращает (base_fill,
/// overlay_alpha, overlay_color) для additive-состояний.
///
/// Если несколько состояний активны — берётся максимальная alpha
/// (M3 spec: state-layers stack, но визуально — max).
pub fn resolve_state(
    base_fill: [f32; 4],
    states: &[KitState],
    accent: [f32; 4],
) -> ([f32; 4], f32, [f32; 4]) {
    let mut max_alpha = 0.0;
    for &s in states {
        let a = state_layer_alpha(s);
        if a > max_alpha {
            max_alpha = a;
        }
    }
    (base_fill, max_alpha, accent)
}

/// Вариант кнопки (семантика действия, не цвет: цвет — из слотов).
///
/// Wave T (2026-10, ui-kit-deep-review §5.1.3): расширено с 4 до 7 вариантов
/// по union Carbon+M3+SLDS: + Tertiary, Text, Inverse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Главное действие модали/панели.
    #[default]
    Primary,
    /// Второстепенное действие.
    Secondary,
    /// Tertiary — lowest emphasis solid button (Carbon tertiary).
    /// Для tertiary-действий в плотных toolbar/формах.
    Tertiary,
    /// Призрачная кнопка (иконка/текст без заливки) — хром, не акция.
    Ghost,
    /// Text — text-only, no container (M3 text button). Lowest emphasis.
    Text,
    /// Разрушающее действие (удаление/сброс).
    Danger,
    /// Inverse — на цветном фоне (SLDS inverse). Для stage_dim backdrop,
    /// цветных хедеров, акцентных полос.
    Inverse,
}

/// Стиль интерактивного контрола — только слоты палитры + радиус шкалы.
///
/// Wave T (2026-10, ui-kit-deep-review §5.2.3): + `elevation: Elevation`
/// (shadow system). `radius: f32` сохранён для backwards-compat —
/// новые потребители могут использовать [`Shape`] enum для типобезопасности:
/// `style.radius = Shape::S.px()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub text: [f32; 4],
    /// Радиус из radius-scale (chip/card/panel/pill). Для типобезопасного
    /// доступа — [`Shape`] enum: `style.radius = Shape::S.px()`.
    pub radius: f32,
    /// Elevation (shadow) — Wave T §5.2.3. Default `Elevation::None` (flat).
    pub elevation: Elevation,
}

// --- Шкалы токенов (Wave T §5.2.2-5.2.6) ------------------------------------

/// Named size scale для контролов (Wave T §5.2.2, Fluent/Carbon pattern).
/// Visual size; hit-zone расширется до `MIN_TOUCH_TARGET` отдельно (LAY8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 24px — tight UI (toolbar, dense lists).
    Xs,
    #[default]
    /// 30px — standard (current BUTTON_HEIGHT/TEXT_FIELD_HEIGHT).
    Sm,
    /// 36px — comfortable (settings, modals).
    Md,
    /// 44px — touch-friendly (mobile, onboarding). Соответствует MIN_TOUCH_TARGET.
    Lg,
}

impl ControlSize {
    /// Высота кнопки для размера.
    pub const fn button_h(self) -> f32 {
        match self {
            Self::Xs => 24.0,
            Self::Sm => 30.0,
            Self::Md => 36.0,
            Self::Lg => 44.0,
        }
    }
    /// Высота чипа для размера.
    pub const fn chip_h(self) -> f32 {
        match self {
            Self::Xs => 20.0,
            Self::Sm => 24.0,
            Self::Md => 28.0,
            Self::Lg => 32.0,
        }
    }
    /// Высота текстового поля для размера.
    pub const fn field_h(self) -> f32 {
        match self {
            Self::Xs => 24.0,
            Self::Sm => 30.0,
            Self::Md => 36.0,
            Self::Lg => 44.0,
        }
    }
    /// Сторона квадратной icon-кнопки для размера.
    pub const fn icon_btn(self) -> f32 {
        match self {
            Self::Xs => 20.0,
            Self::Sm => 26.0,
            Self::Md => 32.0,
            Self::Lg => 44.0,
        }
    }
    /// Высота строки списка для размера.
    pub const fn list_row_h(self) -> f32 {
        match self {
            Self::Xs => 22.0,
            Self::Sm => 26.0,
            Self::Md => 30.0,
            Self::Lg => 36.0,
        }
    }
}

/// Elevation levels (Wave T §5.2.3, M3 + Fluent hybrid). Каждому уровню —
/// shadow-offset + shadow-blur + shadow-alpha для [`crate::paint::Painter::shadow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Elevation {
    /// 0 — flat (cards, panels in-place).
    #[default]
    None,
    /// 1 — subtle (hovered card, dropdown shadow).
    Xs,
    /// 2 — default modal/popover.
    Sm,
    /// 3 — raised modal over another modal.
    Md,
    /// 4 — top modal (settings over stage).
    Lg,
    /// 5 — max (dialog over everything).
    Xl,
}

impl Elevation {
    /// (offset_y, blur, alpha) для shadow-квада.
    pub const fn shadow(self) -> (f32, f32, f32) {
        match self {
            Self::None => (0.0, 0.0, 0.0),
            Self::Xs => (1.0, 2.0, 0.08),
            Self::Sm => (2.0, 4.0, 0.12),
            Self::Md => (4.0, 8.0, 0.16),
            Self::Lg => (8.0, 16.0, 0.20),
            Self::Xl => (16.0, 28.0, 0.24),
        }
    }
}

/// Shape scale (Wave T §5.2.5, M3 7 шагов). Заменяет bare `radius: f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    /// 0 — острый угол (divider, line).
    None,
    /// 4 — subtle (input inset, badge).
    Xs,
    /// 8 — chip, button, menu item (= RADIUS_CHIP).
    S,
    /// 10 — card, panel (= RADIUS_PANEL/CARD).
    M,
    /// 12 — pill, switch track (= RADIUS_PILL).
    L,
    /// 16 — large modal, sheet.
    Xl,
    /// 999 — circle/full-round (avatar, FAB).
    Full,
}

impl Shape {
    /// Радиус в px.
    pub const fn px(self) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Xs => 4.0,
            Self::S => 8.0,
            Self::M => 10.0,
            Self::L => 12.0,
            Self::Xl => 16.0,
            Self::Full => 999.0,
        }
    }
}

/// Spacing scale (Wave T §5.2.6, Carbon 13 шагов; our 5 — subset).
/// Используется в `Row.gap`/`Column.gap`/`EdgeInsets`/paddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spacing {
    /// 2 — hairline (divider inset).
    Xxs = 2,
    /// 4 — tight (icon-to-label).
    Xs = 4,
    #[default]
    /// 6 — chip gap, row gap (current SPACING_S).
    S = 6,
    /// 8 — control gap (current SPACING_SM).
    Sm = 8,
    /// 10 — bar padding (current SPACING_MD).
    Md = 10,
    /// 12 — panel pad (current SPACING_LG).
    Lg = 12,
    /// 16 — section gap.
    Xl = 16,
    /// 24 — viewport margin (current SPACING_XL).
    Xxl = 24,
    /// 32 — hero spacing.
    Xxxl = 32,
    /// 48 — modal air.
    Huge = 48,
    /// 64.
    Giant = 64,
    /// 96.
    Mega = 96,
    /// 160.
    Ultra = 160,
}

impl Spacing {
    /// Значение в px.
    pub const fn px(self) -> f32 {
        self as u32 as f32
    }
}

/// Стиль панели (контейнер поверхности) — только слоты + шкала.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub radius: f32,
    /// Внутренние отступы из spacing-scale.
    pub pad: EdgeInsets,
}

// --- Палитра-срез (контракт «цвета — только слоты») -------------------------

/// Срез слотов палитры дизайн-системы для кита. Заполняется из
/// `canvas-render::theme::ThemeColors` (v2 + слоты состояний FR-053) —
/// impl в крейте рендера; кит остаётся без конкретных цветов.
///
/// Инвариант стиля: [`Button::style`] и др. выбирают СЛОТ по состоянию,
/// но не вычисляют новых цветов (без умножений/смешиваний).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KitPalette {
    /// Заливка панели/модали (`menu_fill`).
    pub panel_fill: [f32; 4],
    /// Рамка панели/модали (`palette_border`).
    pub panel_border: [f32; 4],
    /// Заливка secondary/ghost-контрола (`palette_chip_fill`).
    pub control_fill: [f32; 4],
    /// Рамка контрола (`DIALOG_BUTTON_BORDER` — слот `control_border`).
    pub control_border: [f32; 4],
    /// Заливка primary (`DIALOG_BUTTON_PRIMARY` — слот `control_primary`).
    pub control_primary: [f32; 4],
    /// Заливка danger (`error`, alpha 1.0 — слот `control_danger`).
    pub control_danger: [f32; 4],
    /// Hover вторичной строки/кнопки (слот состояний `control_hover_fill`).
    pub hover_fill: [f32; 4],
    /// Hover primary (слот состояний `control_primary_hover_fill`).
    pub primary_hover_fill: [f32; 4],
    /// Выбранная строка/чип (слот состояний `control_selected_fill`).
    pub selected_fill: [f32; 4],
    /// Основной текст (`body`).
    pub text: [f32; 4],
    /// Заголовки (`title`).
    pub text_title: [f32; 4],
    /// Приглушённый текст (`DIALOG_TEXT_MUTED` — слот `text_muted`).
    pub text_muted: [f32; 4],
    /// Текст disabled (слот состояний `control_disabled_text`).
    pub disabled_text: [f32; 4],
    /// Акцент (фокус-рамка, Ghost-hover).
    pub accent: [f32; 4],
    /// Зелёный статус «успех» (бейдж done/completed) — слот `control_success`
    /// (FR-070: был инлайн-литерал в `agent_panel.rs`/`ai_status_panel.rs`/
    /// `overlays.rs`; проброшен в kit как семантический слот, не константа).
    pub control_success: [f32; 4],
    /// Янтарный статус «предупреждение» (paused, low-quota) — слот
    /// `control_warning` (FR-070: два разных представления одного цвета —
    /// drift risk, теперь один слот).
    pub control_warning: [f32; 4],
    /// Затемнение фона main stage под модалью (backdrop) — слот `stage_dim`
    /// (FR-070: в `ThemeColors::stage_dim` уже был, но `KitPalette` не
    /// пробрасывал — проброшен здесь).
    pub stage_dim: [f32; 4],
    /// Заливка бегунка скроллбара — слот `scrollbar_thumb` (FR-070: был
    /// инлайн в `overlays.rs::docs_overlay`).
    pub scrollbar_thumb: [f32; 4],
    /// Заливка горизонтальной линии `---` markdown — слот `rule_color`
    /// (FR-070: был инлайн в `overlays.rs::docs_overlay`).
    pub rule_color: [f32; 4],
}

// --- Метрики кита (spacing/radius-scale; значения — прежние константы) ------

/// Высота кнопки — прежняя высота кнопок диалога T21.
pub const BUTTON_HEIGHT: f32 = 30.0;
/// Горизонтальный пад кнопки: SPACING_LG (12) — прежний пад кнопок диалога.
pub const BUTTON_PAD_H: f32 = 12.0;
/// Ширина кнопки футера по умолчанию (FR-UI-FOOTER): 100 px — значение
/// онбординга (`ONBOARDING_BUTTON_W`); `graph_builder_ui` использует 130
/// (передаёт свою `widths` через `footer_buttons_measured`). Ранее
/// использовался простым вариантом `footer_buttons` (фиксированная ширина,
/// удалён за отсутствием потребителей — FR-UI-FOOTER-R); оставлен как
/// reference-default для потребителей `footer_buttons_measured`, желающих
/// каноническую ширину кнопки 100 px (значение из спецификации T21).
pub const BUTTON_WIDTH: f32 = 100.0;
/// Сторона квадратной icon-кнопки (угловые кнопки ⚙/?).
pub const ICON_BUTTON_SIZE: f32 = 26.0;
/// Высота чипа (категории палитры — прежняя высота чипов).
pub const CHIP_HEIGHT: f32 = 24.0;
/// Горизонтальный пад чипа: SPACING_SM (8) — прежний пад чипов палитры.
pub const CHIP_PAD_H: f32 = 8.0;
/// Зазор между контролами в ряду: SPACING_SM.
pub const GAP_CONTROLS: f32 = 8.0;
/// TTL тоста (T21-A: 3 с).
pub const TOAST_TTL_MS: u64 = 3000;
/// Задержка показа тултипа (паттерн egui tooltip: delay 500 мс).
pub const TOOLTIP_DELAY_MS: u32 = 500;
/// Отступ тултипа от точки якоря (прежние +14/+18 тултипов канваса).
pub const TOOLTIP_OFFSET: UiPoint = UiPoint::new(14.0, 18.0);
/// Зазор dropdown от якоря.
pub const DROPDOWN_GAP: f32 = 4.0;

// --- Метрики v2 (FR-058) ----------------------------------------------------

/// Высота текстового поля — прежняя высота полей поиска/фильтра пилотов.
pub const TEXT_FIELD_HEIGHT: f32 = 30.0;
/// Горизонтальный пад текстового поля: SPACING_SM (8).
pub const TEXT_FIELD_PAD_H: f32 = 8.0;
/// Минимальная ширина текстового поля.
pub const TEXT_FIELD_MIN_W: f32 = 80.0;

/// Высота строки списка (подсказки/flowmap/calc_panel) — без зазора.
pub const LIST_ROW_H: f32 = 26.0;
/// Зазор между строками списка: SPACING_S (6).
pub const LIST_ROW_GAP: f32 = 6.0;

/// Ширина трека скроллбара (узкая полоса у правого края).
pub const SCROLLBAR_WIDTH: f32 = 4.0;
/// Минимальная высота бегунка скроллбара (чтобы оставался захватимым).
pub const SCROLLBAR_KNOB_MIN: f32 = 20.0;

/// Ширина трека Switch (pill). Соразмерна высоте кнопки (BUTTON_HEIGHT).
pub const SWITCH_W: f32 = 36.0;
/// Высота трека Switch.
pub const SWITCH_H: f32 = 20.0;
/// Внутренний пад Switch: отступ бегунка от краёв трека.
pub const SWITCH_KNOB_PAD: f32 = 2.0;

/// Результат компонентного hit-test: индекс дочернего rect'а из переданных
/// в [`Component::hit_test`] (компонентный уровень; surface-уровень —
/// [`crate::hit::HitTarget`], границы владений FR-068: hit.rs не трогаем).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentHit {
    /// Индекс rect'а в срезе `rects`, переданном в `hit_test`.
    pub index: usize,
}

impl ComponentHit {
    /// Первый rect, содержащий точку `p` (по [`UiRect::contains`]).
    pub fn pick(rects: &[UiRect], p: UiPoint) -> Option<Self> {
        rects
            .iter()
            .position(|r| r.contains(p))
            .map(|index| Self { index })
    }
}

/// FR-068 W3: `Component` — компонентная модель UI (свойства/вёрстка/отрисовка/hit-test).
///
/// Компонент — retained-объект: владеет `Props` (а интерактивные — и
/// [`WidgetState`](crate::widget::WidgetState), интеграция — задача агентов
/// W3 в своих файлах); layout — immediate поверх
/// [`LayoutBackend`](crate::layout::LayoutBackend)
/// (default — [`crate::layout::default_backend`]).
pub trait Component {
    /// Свойства компонента (декларативный вход кадра).
    type Props;

    /// Доступ к свойствам (для отладки/диффа; W3 — без обязательного диффа).
    fn props(&self) -> &Self::Props;

    /// Вёрстка: слот родителя -> rects (детей/самого компонента).
    /// Реализации делегируют в kit-функции поверх `backend` — паритет
    /// Native/Flex/Taffy сохраняется (§Контракт-3 FR-068).
    fn layout(&self, backend: &dyn crate::layout::LayoutBackend, slot: UiRect) -> Vec<UiRect>;

    /// Отрисовка rects через [`Painter`](crate::paint::Painter) (данные,
    /// без GPU — G7).
    fn paint(&self, painter: &mut crate::paint::Painter, rects: &[UiRect]);

    /// Компонентный hit-test: какой из `rects` содержит точку.
    /// Дефолт — первый содержащий rect ([`UiRect::contains`]).
    fn hit_test(&self, rects: &[UiRect], point: UiPoint) -> Option<ComponentHit> {
        ComponentHit::pick(rects, point)
    }
}

// =============================================================================
// Wave A (2026-10, ui-kit-deep-review §5.5): архитектурные паттерны.
//
// Response (egui pattern) + WidgetExt (druid pattern) — interaction-эргономика
// для immediate-mode кита. Response возвращает каждый вызов виджета: геометрия
// + interaction-state в одном объекте. WidgetExt — chainable модификаторы.
// =============================================================================

/// Результат вызова виджета: геометрия + interaction-state (Wave A §5.5.1).
///
/// egui `Response` pattern: каждый вызов `kit.button(...)` возвращает `Response`
/// с `hovered`, `clicked`, `dragged`, `has_focus` и т.д. — consumer проверяет
/// `if resp.clicked { ... }` сразу после отрисовки, без отдельных state-машин.
///
/// Chain methods ( [`WidgetExt`] ): `resp.on_hover_text("Help").on_click(|| save())`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Response {
    /// Rect виджета (для hit-test, tooltip-anchor).
    pub rect: UiRect,
    /// Active KitState (из WidgetState).
    pub state: KitState,
    /// Hovered в этом кадре.
    pub hovered: bool,
    /// Clicked (press+release внутри) — true ровно один кадр.
    pub clicked: bool,
    /// Dragged (press inside + move > threshold).
    pub dragged: bool,
    /// Drag-delta (если dragged).
    pub drag_delta: UiVec2,
    /// Has keyboard focus.
    pub has_focus: bool,
    /// Focus-visible (keyboard-originated).
    pub focus_visible: bool,
    /// Changed (для input — значение изменилось).
    pub changed: bool,
}

impl Default for Response {
    fn default() -> Self {
        Self {
            rect: UiRect::default(),
            state: KitState::Normal,
            hovered: false,
            clicked: false,
            dragged: false,
            drag_delta: UiVec2::new(0.0, 0.0),
            has_focus: false,
            focus_visible: false,
            changed: false,
        }
    }
}

impl Response {
    /// Создать Response из WidgetState + rect (удобный конструктор).
    pub fn from_widget(rect: UiRect, ws: &crate::widget::WidgetState, clicked: bool) -> Self {
        #[allow(deprecated)]
        let state = ws.kit_state();
        Self {
            rect,
            state,
            hovered: matches!(state, KitState::Hovered),
            clicked,
            dragged: ws.is_dragged(),
            drag_delta: UiVec2::new(0.0, 0.0),
            has_focus: ws.is_focused(),
            focus_visible: ws.focus_ring_visible(),
            changed: false,
        }
    }

    /// Chain: регистрирует tooltip для показа после TOOLTIP_DELAY_MS.
    /// Consumer-side: tooltip-state хранит (rect, text), рисуется в overlay.
    pub fn on_hover_text(self, _text: &str) -> Self {
        // TODO Wave D: интеграция с tooltip-state (consumer хранит очередь).
        // Пока — no-op, consumer рисует tooltip отдельно.
        self
    }

    /// Chain: устанавливает disabled (no-op для уже отрисованного виджета —
    /// для pre-draw используйте WidgetState::set_disabled).
    pub fn disabled_if(self, _cond: bool) -> Self {
        self
    }

    /// Chain: вызывает `f` если `clicked == true`.
    pub fn on_click<F: FnOnce()>(self, f: F) -> Self {
        if self.clicked {
            f();
        }
        self
    }
}

/// Chainable модификаторы для [`Response`] (Wave A §5.5.5, druid `WidgetExt`).
///
/// Позволяет `.on_hover_text().disabled_if().on_click()` цепочкой после
/// вызова виджета. Аналог druid `WidgetExt` для immediate-mode.
pub trait WidgetExt: Sized {
    /// Регистрирует tooltip.
    fn on_hover_text(self, text: &str) -> Self;
    /// Устанавливает disabled (post-draw — no-op, для информации).
    fn disabled_if(self, cond: bool) -> Self;
    /// Вызывает `f` при click.
    fn on_click<F: FnOnce()>(self, f: F) -> Self;
    /// Устанавливает KitState (post-draw — для информации).
    fn with_state(self, state: KitState) -> Self;
    /// Устанавливает slot override (post-draw — для информации).
    fn with_slot(self, _key: &str, _val: &str) -> Self;
}

impl WidgetExt for Response {
    fn on_hover_text(self, text: &str) -> Self {
        Response::on_hover_text(self, text)
    }
    fn disabled_if(self, cond: bool) -> Self {
        Response::disabled_if(self, cond)
    }
    fn on_click<F: FnOnce()>(self, f: F) -> Self {
        Response::on_click(self, f)
    }
    fn with_state(mut self, state: KitState) -> Self {
        self.state = state;
        self
    }
    fn with_slot(self, _key: &str, _val: &str) -> Self {
        self
    }
}

#[cfg(test)]
mod wave_t_tests {
    use super::*;

    // --- AC-T1: KitState 7 values + state-layer ---

    /// Stable-эквивалент `std::mem::variant_count::<KitState>()` (unstable,
    /// issue #73662 — Wave T написал под ночным rustc; fix main): исчерпывающий
    /// match — компилятор заставляет обновить при добавлении варианта.
    fn kit_state_variant_count() -> usize {
        match KitState::Normal {
            KitState::Normal
            | KitState::Hovered
            | KitState::Focused
            | KitState::Pressed
            | KitState::Dragged
            | KitState::Disabled
            | KitState::Selected
            | KitState::Error => 8,
        }
    }

    #[test]
    fn kit_state_has_8_variants_with_error() {
        // Wave T §5.1.1: 8 значений (Normal, Hovered, Focused, Pressed,
        // Dragged, Disabled, Selected, Error)
        assert_eq!(
            kit_state_variant_count(),
            8,
            "KitState должен иметь 8 вариантов"
        );
    }

    #[test]
    fn state_layer_alpha_matches_m3_spec() {
        assert!((state_layer_alpha(KitState::Hovered) - 0.08).abs() < 1e-6);
        assert!((state_layer_alpha(KitState::Focused) - 0.10).abs() < 1e-6);
        assert!((state_layer_alpha(KitState::Pressed) - 0.10).abs() < 1e-6);
        assert!((state_layer_alpha(KitState::Dragged) - 0.16).abs() < 1e-6);
        // Нет overlay для этих состояний
        assert_eq!(state_layer_alpha(KitState::Normal), 0.0);
        assert_eq!(state_layer_alpha(KitState::Disabled), 0.0);
        assert_eq!(state_layer_alpha(KitState::Selected), 0.0);
        assert_eq!(state_layer_alpha(KitState::Error), 0.0);
    }

    #[test]
    fn resolve_state_returns_max_alpha() {
        let base = [0.5, 0.5, 0.5, 1.0];
        let accent = [0.0, 0.5, 1.0, 1.0];
        // hover + focused → max(0.08, 0.10) = 0.10
        let (_, alpha, _) = resolve_state(base, &[KitState::Hovered, KitState::Focused], accent);
        assert!((alpha - 0.10).abs() < 1e-6);
        // dragged alone → 0.16
        let (_, alpha, _) = resolve_state(base, &[KitState::Dragged], accent);
        assert!((alpha - 0.16).abs() < 1e-6);
        // Normal → 0.0
        let (_, alpha, _) = resolve_state(base, &[KitState::Normal], accent);
        assert_eq!(alpha, 0.0);
    }

    // --- AC-T3: ButtonVariant 7 variants ---

    /// Stable-эквивалент `std::mem::variant_count::<ButtonVariant>()`
    /// (unstable — fix main; исчерпывающий match охраняет расширение).
    fn button_variant_count() -> usize {
        match ButtonVariant::Primary {
            ButtonVariant::Primary
            | ButtonVariant::Secondary
            | ButtonVariant::Tertiary
            | ButtonVariant::Ghost
            | ButtonVariant::Text
            | ButtonVariant::Danger
            | ButtonVariant::Inverse => 7,
        }
    }

    #[test]
    fn button_variant_has_7_values() {
        assert_eq!(
            button_variant_count(),
            7,
            "ButtonVariant должен иметь 7 вариантов"
        );
    }

    #[test]
    fn button_variant_default_is_primary() {
        assert_eq!(ButtonVariant::default(), ButtonVariant::Primary);
    }

    // --- AC-T5: ControlSize ---

    #[test]
    fn control_size_button_heights() {
        assert_eq!(ControlSize::Xs.button_h(), 24.0);
        assert_eq!(ControlSize::Sm.button_h(), 30.0);
        assert_eq!(ControlSize::Md.button_h(), 36.0);
        assert_eq!(ControlSize::Lg.button_h(), 44.0);
    }

    #[test]
    fn control_size_chip_heights() {
        assert_eq!(ControlSize::Xs.chip_h(), 20.0);
        assert_eq!(ControlSize::Sm.chip_h(), 24.0);
        assert_eq!(ControlSize::Md.chip_h(), 28.0);
        assert_eq!(ControlSize::Lg.chip_h(), 32.0);
    }

    #[test]
    fn control_size_field_heights() {
        assert_eq!(ControlSize::Xs.field_h(), 24.0);
        assert_eq!(ControlSize::Sm.field_h(), 30.0);
        assert_eq!(ControlSize::Md.field_h(), 36.0);
        assert_eq!(ControlSize::Lg.field_h(), 44.0);
    }

    #[test]
    fn control_size_icon_button() {
        assert_eq!(ControlSize::Xs.icon_btn(), 20.0);
        assert_eq!(ControlSize::Sm.icon_btn(), 26.0);
        assert_eq!(ControlSize::Md.icon_btn(), 32.0);
        assert_eq!(ControlSize::Lg.icon_btn(), 44.0);
    }

    #[test]
    fn control_size_list_row_heights() {
        assert_eq!(ControlSize::Xs.list_row_h(), 22.0);
        assert_eq!(ControlSize::Sm.list_row_h(), 26.0);
        assert_eq!(ControlSize::Md.list_row_h(), 30.0);
        assert_eq!(ControlSize::Lg.list_row_h(), 36.0);
    }

    #[test]
    fn control_size_default_is_sm() {
        assert_eq!(ControlSize::default(), ControlSize::Sm);
    }

    // --- AC-T6: Elevation ---

    #[test]
    fn elevation_shadow_values() {
        assert_eq!(Elevation::None.shadow(), (0.0, 0.0, 0.0));
        assert_eq!(Elevation::Xs.shadow(), (1.0, 2.0, 0.08));
        assert_eq!(Elevation::Sm.shadow(), (2.0, 4.0, 0.12));
        assert_eq!(Elevation::Md.shadow(), (4.0, 8.0, 0.16));
        assert_eq!(Elevation::Lg.shadow(), (8.0, 16.0, 0.20));
        assert_eq!(Elevation::Xl.shadow(), (16.0, 28.0, 0.24));
    }

    #[test]
    fn elevation_default_is_none() {
        assert_eq!(Elevation::default(), Elevation::None);
    }

    // --- AC-T8: Shape ---

    #[test]
    fn shape_px_values() {
        assert_eq!(Shape::None.px(), 0.0);
        assert_eq!(Shape::Xs.px(), 4.0);
        assert_eq!(Shape::S.px(), 8.0);
        assert_eq!(Shape::M.px(), 10.0);
        assert_eq!(Shape::L.px(), 12.0);
        assert_eq!(Shape::Xl.px(), 16.0);
        assert_eq!(Shape::Full.px(), 999.0);
    }

    #[test]
    fn shape_default_is_none() {
        assert_eq!(Shape::default(), Shape::None);
    }

    // --- AC-T9: Spacing ---

    #[test]
    fn spacing_px_values() {
        assert_eq!(Spacing::Xxs.px(), 2.0);
        assert_eq!(Spacing::Xs.px(), 4.0);
        assert_eq!(Spacing::S.px(), 6.0);
        assert_eq!(Spacing::Sm.px(), 8.0);
        assert_eq!(Spacing::Md.px(), 10.0);
        assert_eq!(Spacing::Lg.px(), 12.0);
        assert_eq!(Spacing::Xl.px(), 16.0);
        assert_eq!(Spacing::Xxl.px(), 24.0);
        assert_eq!(Spacing::Xxxl.px(), 32.0);
        assert_eq!(Spacing::Huge.px(), 48.0);
        assert_eq!(Spacing::Giant.px(), 64.0);
        assert_eq!(Spacing::Mega.px(), 96.0);
        assert_eq!(Spacing::Ultra.px(), 160.0);
    }

    #[test]
    fn spacing_default_is_s() {
        assert_eq!(Spacing::default(), Spacing::S);
    }
}
