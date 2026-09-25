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

pub mod button;
pub mod dropdown;
pub mod list;
pub mod modal;
pub mod panel;
pub mod row;
pub mod table;
#[cfg(test)]
pub mod test_support;
pub mod text_field;

use crate::geometry::{EdgeInsets, UiPoint, UiRect};

// --- Состояния и стили ------------------------------------------------------

/// Состояние интерактивного виджета кита (слоты состояний палитры — FR-053).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KitState {
    /// Обычное.
    Normal,
    /// Курсор над виджетом.
    Hovered,
    /// Кнопка зажата.
    Pressed,
    /// Недоступен (клик игнорируется потребителем).
    Disabled,
    /// Выбран (тоглы, строки списков, чипы фильтров).
    Selected,
}

/// Вариант кнопки (семантика действия, не цвет: цвет — из слотов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Главное действие модали/панели.
    Primary,
    /// Второстепенное действие.
    Secondary,
    /// Призрачная кнопка (иконка/текст без заливки) — хром, не акция.
    Ghost,
    /// Разрушающее действие (удаление/сброс).
    Danger,
}

/// Стиль интерактивного контрола — только слоты палитры + радиус шкалы.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlStyle {
    pub fill: [f32; 4],
    pub border: [f32; 4],
    pub text: [f32; 4],
    /// Радиус из radius-scale (chip/card/panel/pill).
    pub radius: f32,
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
}

// --- Метрики кита (spacing/radius-scale; значения — прежние константы) ------

/// Высота кнопки — прежняя высота кнопок диалога T21.
pub const BUTTON_HEIGHT: f32 = 30.0;
/// Горизонтальный пад кнопки: SPACING_LG (12) — прежний пад кнопок диалога.
pub const BUTTON_PAD_H: f32 = 12.0;
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
