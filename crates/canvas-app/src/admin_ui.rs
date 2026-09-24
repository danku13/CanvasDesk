//! FR-070: UI-админпанель (консоль дизайн-системы) — модель раскладки +
//! адаптер отрисовки. Паттерн — витрина кита ([`crate::kit_ui`], FR-055):
//! чистая функция раскладки от вьюпорта + [`KitDraw`]-адаптер; сборка кадра —
//! метод `App::admin_panel_overlay` (app.rs), интеграция — реестр
//! поверхностей (Block-модаль, Esc/«✕»/backdrop закрывают).
//!
//! Состав (решения владельца, опрос 2026-09-25): сайдбар секций (паттерн
//! Storybook) + демо-зона справа; секции: «Компоненты» (полная матрица
//! состояний ST1 + Focused/Error), «Наполнение» (empty/medium/full
//! контейнеров), «Канвас» (состояния сущностей ST4), «Токены» (каталог
//! design-токенов + live-правка слотов палитры с кнопкой сброса). Темы —
//! кнопка цикла в шапке (как в витрине FR-055).
//!
//! Интерактив — реальные kit-контролы: пункты сайдбара (Button Secondary,
//! выбранная — слот Selected), кнопка темы (Primary), «Сброс» (Danger),
//! «✕» (IconButton). Демо-контент секций — декоративный (состояния
//! показываются статически, линт не считает его интерактивным).

use canvas_core::Language;
use canvas_ui::geometry::{EdgeInsets, UiPoint, UiRect, UiVec2};
use canvas_ui::kit::{self, KitPalette};
use canvas_ui::layout::{HAlign, VAlign};
use canvas_ui::measure::TextMeasurer;

/// Семейство шрифта подписей (тот же SANS, что у рендера и витрины).
pub const FONT_FAMILY: &str = crate::kit_ui::FONT_FAMILY;
/// Кегль подписей админпанели.
pub const LABEL_SIZE: f32 = crate::kit_ui::LABEL_SIZE;
/// Отступ панели админпанели от краёв вьюпорта (spacing-scale XL).
const VIEWPORT_MARGIN: f32 = 24.0;
/// Зазор между зонами панели.
pub const ZONE_GAP: f32 = 12.0;
/// Высота строки сайдбара (kit Button).
pub const SIDEBAR_ITEM_H: f32 = kit::BUTTON_HEIGHT;
/// Зазор между пунктами сайдбара.
pub const SIDEBAR_ITEM_GAP: f32 = 6.0;
/// Ширина колонки сайдбара.
pub const SIDEBAR_W: f32 = 190.0;
/// Ширина слота кнопки «Сброс».
pub const RESET_SLOT_W: f32 = 96.0;
/// Ширина слота кнопки темы (как в витрине).
pub const THEME_SLOT_W: f32 = crate::kit_ui::THEME_SLOT_W;
/// Высота шапки (ряд кнопок + заголовок).
pub const HEADER_H: f32 = 30.0;

/// Панель админпанели: минимум (влезает в 800×560 окна линта с запасом).
pub const PANEL_MIN: UiVec2 = UiVec2::new(760.0, 500.0);
/// Панель админпанели: максимум (на 1280×800 — 1120×720).
pub const PANEL_MAX: UiVec2 = UiVec2::new(1120.0, 720.0);

/// Секции админпанели (сайдбар; порядок = порядок пунктов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminSection {
    /// Все контролы × полная матрица состояний (ST1 + Focused/Error).
    Components,
    /// Контейнеры × наполнение (пустое / среднее / полное).
    Fill,
    /// Сущности канваса (ST4): карточка ноды, рёбра, порты.
    Canvas,
    /// Каталог design-токенов + live-правка слотов палитры.
    Tokens,
}

/// Все секции сайдбара (порядок пунктов).
pub const ADMIN_SECTIONS: [AdminSection; 4] = [
    AdminSection::Components,
    AdminSection::Fill,
    AdminSection::Canvas,
    AdminSection::Tokens,
];

impl AdminSection {
    /// Ключ i18n подписи пункта сайдбара.
    pub fn label_key(self) -> &'static str {
        match self {
            AdminSection::Components => crate::i18n::keys::ADMIN_SECTION_COMPONENTS,
            AdminSection::Fill => crate::i18n::keys::ADMIN_SECTION_FILL,
            AdminSection::Canvas => crate::i18n::keys::ADMIN_SECTION_CANVAS,
            AdminSection::Tokens => crate::i18n::keys::ADMIN_SECTION_TOKENS,
        }
    }

    /// Индекс секции (элемент сайдбара / суффикс hit-элемента).
    pub fn index(self) -> usize {
        ADMIN_SECTIONS.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// Секция по индексу пункта сайдбара (вне диапазона — None).
    pub fn at(index: usize) -> Option<AdminSection> {
        ADMIN_SECTIONS.get(index).copied()
    }
}

/// Раскладка админпанели (чистая функция от вьюпорта и секции). Этап 1
/// (каркас): шапка + сайдбар + демо-зона с заголовком секции и подсказкой;
/// тела секций наполняются последующими этапами FR-070 (контент демо-зоны
/// сдвигается `scroll.offset` — контракт скролла кита).
#[derive(Debug, Clone)]
pub struct AdminLayout {
    /// Панель админпанели (kit Modal).
    pub panel: UiRect,
    /// Контент внутри панели (минус пад SPACING_LG).
    pub content: UiRect,
    /// Заголовок «UI-консоль» (шапка, слева).
    pub title: UiRect,
    /// Кнопка «Сброс» (kit Button Danger, интерактив).
    pub reset: UiRect,
    /// Кнопка темы (kit Button Primary, интерактив).
    pub theme: UiRect,
    /// Кнопка «✕» (kit IconButton, интерактив).
    pub close: UiRect,
    /// Колонка сайдбара (контейнер пунктов).
    pub sidebar: UiRect,
    /// Пункты сайдбара — по [`ADMIN_SECTIONS`] (kit Button, интерактив).
    pub sidebar_items: Vec<UiRect>,
    /// Демо-зона (окно скролла тела секции).
    pub demo: UiRect,
    /// Полная высота тела секции (для скролла — контент).
    pub demo_content_h: f32,
    /// Заголовок секции в демо-зоне (origin + ключ i18n).
    pub section_title: (UiPoint, &'static str),
    /// Подсказка тела: wrapped-строки (описание состава секции; перенос —
    /// [`wrap_text`], размер [`LABEL_SIZE`]).
    pub hint_lines: Vec<String>,
}

/// Панель админпанели, зажатая во вьюпорт (тот же контракт, что у витрины:
/// constrain(min, max, desired) + кламп к вьюпорту — G4-линт).
pub fn admin_panel(vp: UiRect) -> UiRect {
    let avail = UiVec2::new(
        (vp.w - VIEWPORT_MARGIN).max(0.0),
        (vp.h - VIEWPORT_MARGIN).max(0.0),
    );
    let max = UiVec2::new(PANEL_MAX.x.min(avail.x), PANEL_MAX.y.min(avail.y));
    let min = UiVec2::new(PANEL_MIN.x.min(avail.x), PANEL_MIN.y.min(avail.y));
    kit::panel_rect(vp, min, max, PANEL_MAX)
}

/// Геометрия демо-зоны без полной раскладки (колесо скролла — hit-тест
/// зоны без перестроения тел секций; та же формула, что у [`admin_layout`]).
pub fn admin_demo_viewport(viewport: [f32; 2]) -> UiRect {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let body_top = content.y + HEADER_H + ZONE_GAP;
    let demo_x = content.x + SIDEBAR_W + ZONE_GAP;
    UiRect::new(
        demo_x,
        body_top,
        (content.right() - demo_x).max(0.0),
        (content.bottom() - body_top).max(0.0),
    )
}

/// Слоты шапки (кнопки темы/сброса/«✕») без полной раскладки — hit-rect'ы
/// реестра (те же формулы, что у [`admin_layout`]; шапка фиксирована —
/// скролл демо-зоны слоты не сдвигает).
pub fn admin_hit_slots(viewport: [f32; 2]) -> (UiRect, UiRect, UiRect) {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));
    let close = kit::icon_button_rect(
        UiRect::new(
            content.right() - kit::ICON_BUTTON_SIZE,
            content.y,
            kit::ICON_BUTTON_SIZE,
            HEADER_H,
        ),
        (HAlign::Center, VAlign::Center),
    );
    let theme = UiRect::new(
        close.x - 8.0 - THEME_SLOT_W,
        content.y,
        THEME_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    let reset = UiRect::new(
        theme.x - 8.0 - RESET_SLOT_W,
        content.y,
        RESET_SLOT_W,
        kit::BUTTON_HEIGHT,
    );
    (theme, reset, close)
}

/// Раскладка админпанели. Шапка фиксирована (hit-слоты реестра —
/// [`admin_hit_slots`]); сайдбар и демо-зона — ниже шапки; тело секции
/// (этап 1) — заголовок + подсказка.
pub fn admin_layout(
    viewport: [f32; 2],
    section: AdminSection,
    _p: &KitPalette,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> AdminLayout {
    let vp = UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0));
    let panel = admin_panel(vp);
    let content = panel.inset(&EdgeInsets::uniform(canvas_core::tokens::SPACING_LG));

    // Шапка: заголовок слева; справа — «Сброс», тема, «✕» (край).
    let (theme, reset, close) = admin_hit_slots(viewport);
    let title_w = (reset.x - 8.0 - content.x).max(0.0);
    let title = UiRect::new(content.x, content.y + 6.0, title_w, 20.0);

    // Тело: сайдбар слева, демо-зона в остатке ширины.
    let body_top = content.y + HEADER_H + ZONE_GAP;
    let sidebar = UiRect::new(
        content.x,
        body_top,
        SIDEBAR_W,
        (content.bottom() - body_top).max(0.0),
    );
    let mut sidebar_items = Vec::with_capacity(ADMIN_SECTIONS.len());
    for (i, sec) in ADMIN_SECTIONS.iter().enumerate() {
        let item = kit::button_layout(
            UiRect::new(
                sidebar.x,
                sidebar.y + i as f32 * (SIDEBAR_ITEM_H + SIDEBAR_ITEM_GAP),
                SIDEBAR_W,
                SIDEBAR_ITEM_H,
            ),
            crate::i18n::tr(Language::Ru, sec.label_key()),
            (HAlign::Start, VAlign::Center),
            m,
            fs,
            FONT_FAMILY,
            LABEL_SIZE,
        );
        sidebar_items.push(item.rect);
    }
    let demo = admin_demo_viewport(viewport);

    // Тело секции (этап 1): заголовок секции + подсказка с переносом.
    let section_title = (UiPoint::new(demo.x, demo.y), section.label_key());
    let hint_key = match section {
        AdminSection::Components => crate::i18n::keys::ADMIN_HINT_COMPONENTS,
        AdminSection::Fill => crate::i18n::keys::ADMIN_HINT_FILL,
        AdminSection::Canvas => crate::i18n::keys::ADMIN_HINT_CANVAS,
        AdminSection::Tokens => crate::i18n::keys::ADMIN_HINT_TOKENS,
    };
    let hint_text = crate::i18n::tr(Language::Ru, hint_key);
    let hint_w = (demo.w - 2.0 * 10.0).max(0.0);
    let hint_lines = wrap_text(m, fs, hint_text, hint_w, LABEL_SIZE);
    let demo_content_h = 24.0 + hint_lines.len() as f32 * 18.0 + ZONE_GAP;

    AdminLayout {
        panel,
        content,
        title,
        theme,
        reset,
        close,
        sidebar,
        sidebar_items,
        demo,
        demo_content_h,
        section_title,
        hint_lines,
    }
}

/// Перенос текста по словам под заданную ширину (чистая функция —
/// измерение TextMeasurer'ом; длинное слово не рвётся — строка шире слота).
pub fn wrap_text(
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    text: &str,
    max_w: f32,
    size: f32,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        let candidate = format!("{current} {word}");
        if m.width_of(fs, &candidate, FONT_FAMILY, size) <= max_w {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit_ui::{color4, cursor_in, new_measurer, KitDraw};

    /// Раскладка детерминирована на канонических вьюпортах (G4-линт:
    /// 1280×800 / 1024×640 / 800×560), панель зажата во вьюпорт.
    #[test]
    fn layout_is_deterministic_and_clamped() {
        let palette = test_palette();
        for vp in [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]] {
            let mut m = new_measurer();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = admin_layout(vp, AdminSection::Components, &palette, &mut m, &mut fs);
            let vp_rect = UiRect::new(0.0, 0.0, vp[0], vp[1]);
            assert!(
                vp_rect.contains(UiPoint::new(lay.panel.x, lay.panel.y)),
                "панель выходит за вьюпорт {vp:?}: {:?}",
                lay.panel
            );
            assert!(
                lay.panel.right() <= vp_rect.right() + 0.5
                    && lay.panel.bottom() <= vp_rect.bottom() + 0.5,
                "панель выходит за вьюпорт {vp:?}: {:?}",
                lay.panel
            );
            assert_eq!(lay.sidebar_items.len(), ADMIN_SECTIONS.len());
            // Демо-зона справа от сайдбара, внутри панели.
            assert!(lay.demo.x >= lay.sidebar.right());
            assert!(lay.panel.contains(UiPoint::new(lay.demo.x, lay.demo.y)));
            // Слоты шапки не пересекаются.
            assert!(lay.reset.right() <= lay.theme.x);
            assert!(lay.theme.right() <= lay.close.x);
        }
    }

    /// Секции: индексы и обратное восстановление — полный цикл.
    #[test]
    fn sections_roundtrip() {
        for (i, sec) in ADMIN_SECTIONS.iter().enumerate() {
            assert_eq!(sec.index(), i);
            assert_eq!(AdminSection::at(i), Some(*sec));
        }
        assert_eq!(AdminSection::at(ADMIN_SECTIONS.len()), None);
        // Ключи подписей различны.
        let keys: Vec<&'static str> = ADMIN_SECTIONS.iter().map(|s| s.label_key()).collect();
        for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
                assert_ne!(keys[i], keys[j]);
            }
        }
    }

    /// Hit-слоты шапки совпадают с полной раскладкой (один источник
    /// геометрии — контракт реестра FR-052 U2).
    #[test]
    fn hit_slots_match_full_layout() {
        let palette = test_palette();
        for vp in [[1280.0, 800.0], [800.0, 560.0]] {
            let mut m = new_measurer();
            let mut fs = canvas_render::text::measure_font_system();
            let lay = admin_layout(vp, AdminSection::Tokens, &palette, &mut m, &mut fs);
            let (theme, reset, close) = admin_hit_slots(vp);
            assert_eq!(theme, lay.theme);
            assert_eq!(reset, lay.reset);
            assert_eq!(close, lay.close);
            let demo = admin_demo_viewport(vp);
            assert_eq!(demo, lay.demo);
        }
    }

    /// KitDraw-адаптер рисует каркас: панель, шапку, пункты сайдбара,
    /// рамку демо-зоны — quads/texts непусты и в screen-координатах.
    #[test]
    fn frame_assembly_smoke() {
        let camera = canvas_render::Camera::default();
        let viewport: canvas_render::camera::Vec2 = [1280.0, 800.0];
        let palette = test_palette();
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let lay = admin_layout(
            [1280.0, 800.0],
            AdminSection::Fill,
            &palette,
            &mut m,
            &mut fs,
        );
        let mut d = KitDraw::new(&camera, viewport);
        // Панель + рамка демо-зоны + пункт сайдбара + подписи.
        d.rect(lay.panel, palette.panel_fill, palette.panel_border, 10.0);
        d.rect(
            lay.demo,
            [0.0; 4],
            palette.control_border,
            canvas_core::tokens::RADIUS_PANEL,
        );
        for (i, item) in lay.sidebar_items.iter().enumerate() {
            let state = if i == 1 {
                kit::KitState::Selected
            } else {
                kit::KitState::Normal
            };
            let style = kit::button_style(kit::ButtonVariant::Secondary, state, &palette);
            d.control(*item, &style);
            d.label_center(*item, "X", style.text, LABEL_SIZE);
        }
        d.label_left(
            UiRect::new(
                lay.section_title.0.x,
                lay.section_title.0.y,
                lay.demo.w,
                18.0,
            ),
            "Секция",
            palette.text_title,
            LABEL_SIZE,
        );
        assert!(!d.quads.is_empty(), "кадр без квадов");
        assert!(!d.texts.is_empty(), "кадр без текстов");
        // Все квады — валидные размеры (screen-пространство вьюпорта).
        for q in &d.quads {
            assert!(q.size[0] >= 0.0 && q.size[1] >= 0.0);
        }
        let _ = color4([0.0, 0.0, 0.0, 1.0]);
        let _ = cursor_in(&lay.demo, [lay.demo.x + 1.0, lay.demo.y + 1.0]);
    }

    /// Перенос по словам: строки не шире слота (кроме одиночного длинного
    /// слова), порядок слов сохранён, пустой ввод — одна пустая строка.
    #[test]
    fn wrap_text_breaks_by_words() {
        let mut m = new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let text = "короткий текст переносится по словам при превышении ширины слота";
        let lines = wrap_text(&mut m, &mut fs, text, 140.0, LABEL_SIZE);
        assert!(lines.len() >= 2, "ожидается перенос: {lines:?}");
        // Слова не теряются (сумма слов строк = число слов исходного текста).
        let words_in_lines: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
        assert_eq!(words_in_lines, text.split_whitespace().count());
        for line in &lines {
            let w = m.width_of(&mut fs, line, FONT_FAMILY, LABEL_SIZE);
            assert!(
                w <= 140.0 + 0.5 || line.split_whitespace().count() == 1,
                "строка шире слота и не одиночное слово: {line:?} ({w})"
            );
        }
        assert_eq!(
            wrap_text(&mut m, &mut fs, "", 140.0, LABEL_SIZE),
            vec![String::new()]
        );
        assert_eq!(
            wrap_text(&mut m, &mut fs, "одно", 140.0, LABEL_SIZE).len(),
            1
        );
    }

    /// Тестовая палитра (значения не важны — важны различные слоты).
    fn test_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.1, 0.1, 0.12, 0.97],
            panel_border: [0.3, 0.3, 0.35, 1.0],
            control_fill: [0.2, 0.2, 0.24, 1.0],
            control_border: [0.35, 0.4, 0.5, 1.0],
            control_primary: [0.16, 0.32, 0.6, 1.0],
            control_danger: [0.8, 0.2, 0.2, 1.0],
            hover_fill: [0.25, 0.25, 0.3, 1.0],
            primary_hover_fill: [0.25, 0.46, 0.84, 1.0],
            selected_fill: [0.3, 0.3, 0.4, 1.0],
            text: [0.9, 0.9, 0.95, 1.0],
            text_title: [0.95, 0.95, 1.0, 1.0],
            text_muted: [0.7, 0.7, 0.75, 1.0],
            disabled_text: [0.5, 0.5, 0.55, 1.0],
            accent: [0.396, 0.612, 0.969, 1.0],
        }
    }
}
