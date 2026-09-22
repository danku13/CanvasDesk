//! FR-055 (этап U4 PRD-0009, F-8): витрина кита — модель раскладки +
//! адаптер «модель кита → квад/текст кадра». Сборка кадра — метод
//! `App::kit_gallery_overlay` (app.rs, паттерн прочих оверлеев).
//!
//! Витрина = живой образец кита (Q5 PRD-0009: вариант (a) — пункт «О
//! интерфейсе» меню «?», доступна всегда; полезна для баг-репортов и
//! приёмки): компоненты × состояния × RU/EN × темы. Интерактив — только
//! кнопка темы (реальный kit::Button Primary) и «✕»; остальные контролы
//! показывают состояния статически (декоративные rect'ы — линт не считает
//! их интерактивными). Тема переключается на ходу — слоты палитры меняются,
//! виджеты перерисовываются теми же функциями (контракт F-8 «цвета — только
//! слоты» проверяется наглядно).
//!
//! Контракт поверхности (реестр U2): Modals/Block — клик мимо панели
//! (backdrop) закрывает и глотает; Esc — закрыть; «✕» — закрыть.
//! Отрисовка — полоса `UiLayer::Modals`.

use canvas_core::Language;
use canvas_render::camera::Vec2;
use canvas_render::cards::CardInstance;
use canvas_render::text::{measure_font_system, TextAlign, SANS_FAMILY};
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{self, ButtonVariant, ControlStyle, KitState};
use canvas_ui::measure::TextMeasurer;

/// Семейство шрифта подписей витрины (тот же SANS, что у рендера).
pub const FONT_FAMILY: &str = SANS_FAMILY;
/// Панель витрины: минимум (влезает в 800×560 окна линта с запасом).
pub const PANEL_MIN: UiVec2 = UiVec2::new(560.0, 420.0);
/// Панель витрины: максимум (на 1280×800 — 900×640).
pub const PANEL_MAX: UiVec2 = UiVec2::new(900.0, 640.0);
/// Отступ секций по вертикали (spacing-scale).
pub const SECTION_GAP: f32 = 12.0;
/// Ширина колонки подписи состояния.
pub const STATE_LABEL_W: f32 = 84.0;
/// Кегль подписей контролов.
pub const LABEL_SIZE: f32 = 13.0;
/// Ширина слота кнопки темы.
pub const THEME_SLOT_W: f32 = 170.0;

/// Подписи состояний (ключи i18n витрины).
pub const STATE_LABELS: [&str; 4] = [
    "kit.state.normal",
    "kit.state.hover",
    "kit.state.pressed",
    "kit.state.disabled",
];

/// Секции витрины (имена компонентов кита — классы, не переводятся).
pub const SECTION_BUTTONS: &str = "kit.section.buttons";
pub const SECTION_ICON: &str = "kit.section.icon_buttons";
pub const SECTION_CHIPS: &str = "kit.section.chips";
pub const SECTION_DROPDOWN: &str = "kit.section.dropdown";
pub const SECTION_TOAST: &str = "kit.section.toast";
pub const SECTION_TOOLTIP: &str = "kit.section.tooltip";

/// Ряд кнопок одного варианта.
#[derive(Debug, Clone)]
pub struct ButtonRow {
    /// Вариант (заголовок ряда).
    pub variant: ButtonVariant,
    /// Слот ряда (полная ширина контента).
    pub slot: UiRect,
    /// Rect'ы 4 кнопок состояний (Normal/Hovered/Pressed/Disabled).
    pub buttons: Vec<UiRect>,
}

/// Раскладка витрины (чистая функция от вьюпорта; ширины подписей —
/// измеренные TextMeasurer'ом).
#[derive(Debug, Clone)]
pub struct GalleryLayout {
    /// Панель витрины (kit Modal).
    pub panel: UiRect,
    /// Контент внутри панели (минус пад).
    pub content: UiRect,
    /// Кнопка «✕» (kit IconButton, интерактив).
    pub close: UiRect,
    /// Кнопка темы (kit Button Primary, интерактив).
    pub theme: UiRect,
    /// Заголовок витрины.
    pub title: UiRect,
    /// Ряды кнопок: 4 варианта × 4 состояния.
    pub button_rows: Vec<ButtonRow>,
    /// Икон-кнопки: 4 состояния.
    pub icon_buttons: Vec<UiRect>,
    /// Чипы: 4 состояния.
    pub chips: Vec<UiRect>,
    /// Закрытый dropdown-якорь.
    pub dropdown_anchor: UiRect,
    /// Открытое dropdown-меню.
    pub dropdown_menu: UiRect,
    /// Строки dropdown-меню.
    pub dropdown_items: Vec<UiRect>,
    /// Область тоста (kit Toast).
    pub toast: UiRect,
    /// Чип-якорь тултипа.
    pub tooltip_anchor: UiRect,
    /// Пузырь тултипа (delay пройден — показан).
    pub tooltip: UiRect,
    /// Подписи секций (origin + текст).
    pub section_titles: Vec<(UiPoint, &'static str)>,
}

/// Перевод ключа витрины (ключи — 'static константы модуля).
fn tr(lang: Language, key: &'static str) -> String {
    crate::i18n::tr(lang, key).to_owned()
}

/// Отступ панели витрины от краёв вьюпорта (spacing-scale XL).
const VIEWPORT_MARGIN: f32 = 24.0;

/// Панель витрины, зажатая во вьюпорт: constrain(min, max, desired) + кламп
/// к вьюпорту (модаль не вылезает на малых окнах — G4-линт: hit-rect'ы
/// интерактивных зон целиком во вьюпорте; поймал 800×560 на U4).
pub fn gallery_panel(vp: UiRect) -> UiRect {
    let avail = UiVec2::new(
        (vp.w - VIEWPORT_MARGIN).max(0.0),
        (vp.h - VIEWPORT_MARGIN).max(0.0),
    );
    let max = UiVec2::new(PANEL_MAX.x.min(avail.x), PANEL_MAX.y.min(avail.y));
    let min = UiVec2::new(PANEL_MIN.x.min(avail.x), PANEL_MIN.y.min(avail.y));
    kit::panel_rect(vp, min, max, PANEL_MAX)
}

/// Раскладка витрины. Секции — Column-поток от контента; высоты — метрики
/// кита, ширины подписей — измеренные.
pub fn gallery_layout(
    viewport: [f32; 2],
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> GalleryLayout {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    // Панель: kit Modal (constrain + stack по центру, зажата во вьюпорт)
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));

    let mut y = content.y;
    let full_w = content.w;

    // Шапка: заголовок слева, кнопка темы справа (перед ✕), ✕ — край
    let title = UiRect::new(
        content.x,
        y,
        (full_w - 2.0 * (kit::ICON_BUTTON_SIZE + 8.0)).max(0.0),
        30.0,
    );
    let close = kit::icon_button_rect(
        UiRect::new(
            content.right() - kit::ICON_BUTTON_SIZE,
            y,
            kit::ICON_BUTTON_SIZE,
            30.0,
        ),
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
    );
    let theme = UiRect::new(
        close.x - 8.0 - THEME_SLOT_W,
        y + (30.0 - kit::BUTTON_HEIGHT) / 2.0,
        THEME_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    y += 30.0 + SECTION_GAP;

    let mut section_titles: Vec<(UiPoint, &'static str)> = Vec::new();
    let mut button_rows: Vec<ButtonRow> = Vec::new();
    let mut icon_buttons: Vec<UiRect> = Vec::new();
    let mut chips: Vec<UiRect> = Vec::new();
    let mut dropdown_items: Vec<UiRect> = Vec::new();

    // Подпись состояния слева + контрол в остатке ширины
    let control_x = content.x + STATE_LABEL_W + canvas_core::tokens::SPACING_SM;
    let control_w = (content.right() - control_x).max(0.0);

    // --- Buttons: 4 варианта × 4 состояния ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_BUTTONS));
    y += 18.0;
    let variants = [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ];
    for variant in variants {
        let slot = UiRect::new(content.x, y, full_w, kit::BUTTON_HEIGHT);
        let per = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        let mut buttons = Vec::with_capacity(4);
        for (i, state_label) in STATE_LABELS.iter().enumerate() {
            let cell = UiRect::new(
                control_x + i as f32 * (per + kit::GAP_CONTROLS),
                y,
                per,
                kit::BUTTON_HEIGHT,
            );
            let label = tr(lang, state_label);
            let bl = kit::button_layout(
                cell,
                &label,
                (
                    canvas_ui::layout::HAlign::Start,
                    canvas_ui::layout::VAlign::Center,
                ),
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            buttons.push(bl.rect);
        }
        button_rows.push(ButtonRow {
            variant,
            slot,
            buttons,
        });
        y += kit::BUTTON_HEIGHT + 8.0;
    }
    y += SECTION_GAP - 4.0;

    // --- IconButtons: 4 состояния ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_ICON));
    y += 18.0;
    for i in 0..4 {
        let cell = UiRect::new(
            control_x + i as f32 * (kit::ICON_BUTTON_SIZE + kit::GAP_CONTROLS),
            y,
            kit::ICON_BUTTON_SIZE,
            kit::ICON_BUTTON_SIZE,
        );
        icon_buttons.push(kit::icon_button_rect(
            cell,
            (
                canvas_ui::layout::HAlign::Start,
                canvas_ui::layout::VAlign::Center,
            ),
        ));
    }
    y += kit::ICON_BUTTON_SIZE + SECTION_GAP;

    // --- Chips: 4 состояния (измеренные ширины) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_CHIPS));
    y += 18.0;
    {
        let mut x = control_x;
        let max_w = ((control_w - 3.0 * kit::GAP_CONTROLS) / 4.0).max(0.0);
        for state_label in STATE_LABELS.iter() {
            let label = tr(lang, state_label);
            let cl = kit::chip_layout(
                UiPoint::new(x, y),
                &label,
                max_w,
                m,
                fs,
                FONT_FAMILY,
                LABEL_SIZE,
            );
            chips.push(cl.rect);
            x += cl.rect.w + kit::GAP_CONTROLS;
        }
    }
    y += kit::CHIP_HEIGHT + SECTION_GAP;

    // --- Dropdown: якорь-кнопка + открытое меню (3 строки) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_DROPDOWN));
    y += 18.0;
    let anchor_label = tr(lang, "kit.dropdown.anchor");
    let anchor_btn = kit::button_layout(
        UiRect::new(control_x, y, 190.0, kit::BUTTON_HEIGHT),
        &anchor_label,
        (
            canvas_ui::layout::HAlign::Start,
            canvas_ui::layout::VAlign::Center,
        ),
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let dropdown_anchor = anchor_btn.rect;
    y += kit::BUTTON_HEIGHT + 4.0;
    let item_h = 26.0;
    let menu_h = 3.0 * item_h + 2.0 * 4.0;
    let dd = kit::dropdown_menu(dropdown_anchor, vp, UiVec2::new(190.0, menu_h));
    for i in 0..3 {
        dropdown_items.push(UiRect::new(
            dd.menu.x + 4.0,
            dd.menu.y + 4.0 + i as f32 * (item_h + 4.0),
            dd.menu.w - 8.0,
            item_h,
        ));
    }
    let dropdown_menu = dd.menu;
    y += menu_h + SECTION_GAP;

    // --- Toast: строка внизу контента (kit Toast) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_TOAST));
    y += 18.0;
    let toast = UiRect::new(content.x, y, content.w, 20.0);
    y += 20.0 + SECTION_GAP;

    // --- Tooltip: якорь-чип + пузырь (delay пройден) ---
    section_titles.push((UiPoint::new(content.x, y), SECTION_TOOLTIP));
    y += 18.0;
    let anchor_label = tr(lang, "kit.tooltip.anchor");
    let cl = kit::chip_layout(
        UiPoint::new(control_x, y),
        &anchor_label,
        140.0,
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    let tooltip_anchor = cl.rect;
    let tip_text = tr(lang, "kit.tooltip.body");
    let tip_w = (m.width_of(fs, &tip_text, FONT_FAMILY, 12.0) + 16.0).min(240.0);
    let tip = kit::tooltip(
        UiPoint::new(tooltip_anchor.right(), tooltip_anchor.y + 4.0),
        UiVec2::new(tip_w, 18.0),
        vp,
        kit::TOOLTIP_DELAY_MS,
        kit::TOOLTIP_DELAY_MS,
    );
    let tooltip = tip
        .map(|t| t.rect)
        .unwrap_or(UiRect::new(0.0, 0.0, 0.0, 0.0));

    GalleryLayout {
        panel,
        content,
        close,
        theme,
        title,
        button_rows,
        icon_buttons,
        chips,
        dropdown_anchor,
        dropdown_menu,
        dropdown_items,
        toast,
        tooltip_anchor,
        tooltip,
        section_titles,
    }
}

/// Слот-раскладка интерактивных зон для hit-rect'ов реестра (без
/// измерителя — фиксированные слоты шапки; совпадает с полной раскладкой —
/// одна геометрия для ввода и отрисовки). Возврат: (кнопка темы, «✕»).
pub fn gallery_hit_slots(viewport: [f32; 2]) -> (UiRect, UiRect) {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = gallery_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let close = kit::icon_button_rect(
        UiRect::new(
            content.right() - kit::ICON_BUTTON_SIZE,
            content.y,
            kit::ICON_BUTTON_SIZE,
            30.0,
        ),
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
    );
    let theme = UiRect::new(
        close.x - 8.0 - THEME_SLOT_W,
        content.y + (30.0 - kit::BUTTON_HEIGHT) / 2.0,
        THEME_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    (theme, close)
}

/// Состояние интерактивного контрола по курсору (hover).
pub fn cursor_state(hovered: bool, disabled: bool) -> KitState {
    if disabled {
        KitState::Disabled
    } else if hovered {
        KitState::Hovered
    } else {
        KitState::Normal
    }
}

/// [f32;4] sRGB → glyphon Color (представление, не арифметика цвета).
pub fn color4(c: [f32; 4]) -> canvas_render::Color {
    canvas_render::Color::rgba(
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
        (c[3] * 255.0).round() as u8,
    )
}

/// Адаптер: модель кита → квад/текст кадра (screen-rect → world-инстанс —
/// тот же паттерн `screen_rect_quad` app.rs; camera/viewport даёт App).
pub(crate) struct KitDraw<'a> {
    camera: &'a canvas_render::Camera,
    viewport: Vec2,
    pub quads: Vec<CardInstance>,
    pub texts: Vec<OwnedText>,
}

/// Владеемый screen-текст кадра (зеркало `app::OwnedScreenText` — модуль
/// без доступа к приватным полям app.rs; конвертация на кадре).
pub struct OwnedText {
    pub text: String,
    pub origin: [f32; 2],
    pub width: f32,
    pub font_size: f32,
    pub color: canvas_render::Color,
    pub align: TextAlign,
}

impl<'a> KitDraw<'a> {
    pub fn new(camera: &'a canvas_render::Camera, viewport: Vec2) -> Self {
        Self {
            camera,
            viewport,
            quads: Vec::new(),
            texts: Vec::new(),
        }
    }

    /// Прямоугольник с заливкой/рамкой/радиусом.
    pub fn rect(&mut self, r: UiRect, fill: [f32; 4], border: [f32; 4], radius: f32) {
        self.quads.push(crate::app::screen_rect_quad_pub(
            self.camera,
            self.viewport,
            [r.x, r.y, r.w, r.h],
            fill,
            border,
            radius,
        ));
    }

    /// Стиль контрола (заливка + рамка).
    pub fn control(&mut self, r: UiRect, s: &ControlStyle) {
        self.rect(r, s.fill, s.border, s.radius);
    }

    /// Подпись по центру области (контракт ScreenText: origin — левый край
    /// области выравнивания, Center центрирует в [origin, origin+width]).
    pub fn label_center(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32) {
        self.texts.push(OwnedText {
            text: text.to_owned(),
            origin: [area.x, area.y],
            width: area.w,
            font_size: size,
            color: color4(color),
            align: TextAlign::Center,
        });
    }

    /// Подпись слева.
    pub fn label_left(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32) {
        self.texts.push(OwnedText {
            text: text.to_owned(),
            origin: [area.x, area.y],
            width: area.w,
            font_size: size,
            color: color4(color),
            align: TextAlign::Left,
        });
    }
}

/// Подбор стиля строки dropdown по курсору (hover — реальный слот).
pub fn dropdown_item_state(hovered: bool) -> KitState {
    cursor_state(hovered, false)
}

/// Проверка «курсор внутри rect'а» (xywh UiRect).
pub fn cursor_in(r: &UiRect, cursor: [f32; 2]) -> bool {
    r.contains(UiPoint::new(cursor[0], cursor[1]))
}

/// Публичный доступ к измерителю для сборки в app.rs (единый кадр —
/// один measurer).
pub fn new_measurer() -> TextMeasurer {
    TextMeasurer::new()
}

/// Обёртка измерения ширины (app.rs не тянет cosmic-text типы).
pub fn measured_width(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    text: &str,
    size: f32,
) -> f32 {
    m.width_of(fs, text, FONT_FAMILY, size)
}

/// Замер кнопки темы в шапке (та же функция кита, что и у отрисовки).
pub fn theme_button_layout(
    slot: UiRect,
    lang: Language,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> (UiRect, String) {
    let label = tr(lang, "kit.gallery.theme");
    let bl = kit::button_layout(
        slot,
        &label,
        (
            canvas_ui::layout::HAlign::Center,
            canvas_ui::layout::VAlign::Center,
        ),
        m,
        fs,
        FONT_FAMILY,
        LABEL_SIZE,
    );
    (bl.rect, bl.label)
}

/// Замер, что нужен FontSystem — инициализация guard'а в app.rs (владение
/// рендера); модуль даёт тонкую обёртку.
pub fn with_font_system<R>(f: impl FnOnce(&mut cosmic_text::FontSystem) -> R) -> R {
    let mut guard = measure_font_system();
    f(&mut guard)
}
