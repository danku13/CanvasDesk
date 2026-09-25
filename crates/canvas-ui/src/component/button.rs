//! FR-068 W3: кнопка/чип/свитч/иконка — перенос из kit.rs 1:1 (W3).
//!
//! Владелец волны (агент 3-a): дополнить `Props` + `impl Component` для
//! кнопки (см. worklog/FR-068 §W3); существующие функции — стабильный API.

use super::{
    ButtonVariant, ControlStyle, KitPalette, KitState, BUTTON_HEIGHT, BUTTON_PAD_H, CHIP_HEIGHT,
    CHIP_PAD_H, ICON_BUTTON_SIZE, SWITCH_H, SWITCH_KNOB_PAD, SWITCH_W,
};
use crate::geometry::{UiPoint, UiRect, UiVec2};
use crate::layout::{stack, HAlign, VAlign};
use crate::measure::TextMeasurer;

// --- Button -----------------------------------------------------------------

/// Замер кнопки по подписи: ширина = текст + 2·BUTTON_PAD_H, высота = const.
/// Пустая подпись → квадрат минимальной ширины BUTTON_HEIGHT.
pub fn button_size(
    label: &str,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> UiVec2 {
    let text_w = if label.is_empty() {
        0.0
    } else {
        m.width_of(fs, label, family, size)
    };
    let w = (text_w + BUTTON_PAD_H * 2.0).max(BUTTON_HEIGHT);
    UiVec2::new(w, BUTTON_HEIGHT)
}

/// Кнопка в слоте: измеренный размер, позиция — stack по выравниванию;
/// подпись усекается `ellipsis`, если слот уже кнопки (деградация видна
/// потребителю и линту, не молчаливый break).
pub struct ButtonLayout {
    /// Rect кнопки.
    pub rect: UiRect,
    /// Подпись после ellipsis (может отличаться от исходной).
    pub label: String,
}

pub fn button_layout(
    slot: UiRect,
    label: &str,
    align: (HAlign, VAlign),
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> ButtonLayout {
    let desired = button_size(label, m, fs, family, size);
    let max_w = slot.w;
    // Деградация узкого слота: усечение по фактической ширине (не срез);
    // кнопка не вылезает за слот — сжимается до его ширины.
    let width = desired.x.min(max_w);
    let shown = if desired.x > max_w {
        m.ellipsis(
            fs,
            label,
            family,
            size,
            (max_w - BUTTON_PAD_H * 2.0).max(0.0),
        )
    } else {
        label.to_owned()
    };
    let rect = stack(slot, UiVec2::new(width, BUTTON_HEIGHT), align.0, align.1);
    ButtonLayout { rect, label: shown }
}

/// Стиль кнопки: слот заливки по варианту, слот hover/pressed по состоянию,
/// текст — слот по состоянию (disabled — свой слот). Никакой арифметики
/// над цветами: только выбор слота.
pub fn button_style(variant: ButtonVariant, state: KitState, p: &KitPalette) -> ControlStyle {
    let base_fill = match variant {
        ButtonVariant::Primary => p.control_primary,
        ButtonVariant::Danger => p.control_danger,
        ButtonVariant::Secondary | ButtonVariant::Ghost => p.control_fill,
    };
    let fill = match state {
        KitState::Normal | KitState::Selected => base_fill,
        KitState::Hovered => match variant {
            ButtonVariant::Primary => p.primary_hover_fill,
            ButtonVariant::Secondary | ButtonVariant::Ghost | ButtonVariant::Danger => p.hover_fill,
        },
        KitState::Pressed => match variant {
            ButtonVariant::Primary => p.primary_hover_fill,
            _ => p.hover_fill,
        },
        KitState::Disabled => base_fill,
    };
    let border = match (variant, state) {
        (ButtonVariant::Ghost, KitState::Hovered | KitState::Pressed) => p.accent,
        (ButtonVariant::Ghost, _) => [0.0, 0.0, 0.0, 0.0],
        _ => p.control_border,
    };
    let text = match state {
        KitState::Disabled => p.disabled_text,
        _ => match variant {
            ButtonVariant::Ghost => p.text,
            _ => p.text_title,
        },
    };
    ControlStyle {
        fill,
        border,
        text,
        radius: canvas_core::tokens::RADIUS_CHIP,
    }
}

// --- IconButton -------------------------------------------------------------

/// Квадратная кнопка в слоте (угловые кнопки, ✕ модалей).
pub fn icon_button_rect(slot: UiRect, align: (HAlign, VAlign)) -> UiRect {
    stack(
        slot,
        UiVec2::new(ICON_BUTTON_SIZE, ICON_BUTTON_SIZE),
        align.0,
        align.1,
    )
}

/// Стиль icon-кнопки = Ghost-кнопка (тот же слот hover, без заливки в Normal).
pub fn icon_button_style(state: KitState, p: &KitPalette) -> ControlStyle {
    button_style(ButtonVariant::Ghost, state, p)
}

// --- Chip -------------------------------------------------------------------

/// Замер чипа: ширина = текст + 2·CHIP_PAD_H, высота = const.
pub fn chip_size(
    label: &str,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> UiVec2 {
    let text_w = m.width_of(fs, label, family, size);
    UiVec2::new(text_w + CHIP_PAD_H * 2.0, CHIP_HEIGHT)
}

/// Чип от левого края `origin_x` (ряд чипов компонует потребитель через
/// `Row`/SqueezeTail); усечение подписи — ellipsis по `max_width`.
pub struct ChipLayout {
    pub rect: UiRect,
    pub label: String,
}

pub fn chip_layout(
    origin: UiPoint,
    label: &str,
    max_width: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    size: f32,
) -> ChipLayout {
    let desired = chip_size(label, m, fs, family, size);
    let shown = if desired.x > max_width {
        m.ellipsis(
            fs,
            label,
            family,
            size,
            (max_width - CHIP_PAD_H * 2.0).max(0.0),
        )
    } else {
        label.to_owned()
    };
    let w = if shown == label { desired.x } else { max_width };
    ChipLayout {
        rect: UiRect::new(origin.x, origin.y, w, CHIP_HEIGHT),
        label: shown,
    }
}

/// Стиль чипа: selected → слот selected_fill, hovered → hover_fill,
/// disabled → текст disabled. Радиус RADIUS_CHIP.
pub fn chip_style(state: KitState, p: &KitPalette) -> ControlStyle {
    let fill = match state {
        KitState::Selected => p.selected_fill,
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => p.control_fill,
    };
    ControlStyle {
        fill,
        border: p.control_border,
        text: match state {
            KitState::Disabled => p.disabled_text,
            _ => p.text,
        },
        radius: canvas_core::tokens::RADIUS_CHIP,
    }
}

// --- Switch -----------------------------------------------------------------

/// Раскладка Switch (тогла): трек (pill) + бегунок + стиль трека + заливка
/// бегунка. `on` определяет позицию бегунка (вправо) и слот заливки трека.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchLayout {
    /// Rect трека (pill).
    pub track: UiRect,
    /// Rect бегунка (квадрат внутри трека).
    pub knob: UiRect,
    /// Стиль трека — слот заливки/рамки/радиуса (RADIUS_PILL).
    pub track_style: ControlStyle,
    /// Заливка бегунка (слот `text_title`/`disabled_text`).
    pub knob_fill: [f32; 4],
}

/// Switch в слоте: трек (pill) + бегунок. `on` — позиция бегунка и слот
/// заливки трека (`control_primary` on / `control_fill` off); `state` —
/// hover/pressed/disabled слоты; `knob_fill` — `text_title` (disabled —
/// `disabled_text`).
pub fn switch(slot: UiRect, on: bool, state: KitState, p: &KitPalette) -> SwitchLayout {
    let track = stack(
        slot,
        UiVec2::new(SWITCH_W, SWITCH_H),
        HAlign::Center,
        VAlign::Center,
    );
    // Бегунок: квадрат side = SWITCH_H - 2·pad; позиция — влево/вправо по `on`.
    let knob_side = (SWITCH_H - 2.0 * SWITCH_KNOB_PAD).max(0.0);
    let knob_x = if on {
        track.right() - SWITCH_KNOB_PAD - knob_side
    } else {
        track.x + SWITCH_KNOB_PAD
    };
    let knob_y = track.y + SWITCH_KNOB_PAD;
    let knob = UiRect::new(knob_x, knob_y, knob_side, knob_side);
    // Стиль трека: заливка — primary on / control_fill off; hover/pressed —
    // свои слоты; disabled — базовая. Радиус RADIUS_PILL (pill).
    let base_fill = if on {
        p.control_primary
    } else {
        p.control_fill
    };
    let fill = match state {
        KitState::Hovered | KitState::Pressed => {
            if on {
                p.primary_hover_fill
            } else {
                p.hover_fill
            }
        }
        _ => base_fill,
    };
    let track_style = ControlStyle {
        fill,
        border: p.control_border,
        text: p.text, // не используется у Switch (без подписи)
        radius: canvas_core::tokens::RADIUS_PILL,
    };
    // Бегунок: text_title (white/light); disabled — приглушён.
    let knob_fill = match state {
        KitState::Disabled => p.disabled_text,
        _ => p.text_title,
    };
    SwitchLayout {
        track,
        knob,
        track_style,
        knob_fill,
    }
}

// --- Icon -------------------------------------------------------------------

/// Семантическая иконка для IconButton. Глифы — существующим шрифтом
/// (NotoSansDisplay-Medium): 0 новых зависимостей (G7). Литералы потребителей
/// («✕»/«⚙»/«?»/«+») переносятся сюда; новые (Search/ArrowLeft/ArrowRight/
/// Refresh) — стандартные Unicode-символы, поддерживаемые NotoSansDisplay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Закрыть (✕).
    Close,
    /// Настройки/шестерёнка (⚙).
    Gear,
    /// Помощь/вопрос (?).
    Question,
    /// Поиск (⌕).
    Search,
    /// Добавить (+).
    Plus,
    /// Стрелка влево (←).
    ArrowLeft,
    /// Стрелка вправо (→).
    ArrowRight,
    /// Обновить (↻).
    Refresh,
}

/// Глиф иконки — `&'static str` для `TextMeasurer`/`Painter::label`. Маппинг
/// полон (все варианты `Icon` покрыты — тест `icon_glyph_mapping_is_complete`).
pub fn icon_glyph(i: Icon) -> &'static str {
    match i {
        Icon::Close => "×",
        Icon::Gear => "⚙",
        Icon::Question => "?",
        Icon::Search => "⌕",
        Icon::Plus => "+",
        Icon::ArrowLeft => "←",
        Icon::ArrowRight => "→",
        Icon::Refresh => "↻",
    }
}

/// Квадратная кнопка с иконкой в слоте — делегирует [`icon_button_rect`]
/// (та же геометрия; иконка — отдельным вызовом [`icon_glyph`] для замера/
/// отрисовки потребителем). `icon` зарезервирован для будущей текстовой
/// раскладки (ширина глифа может варьироваться — v2 с TextMeasurer).
pub fn icon_button(slot: UiRect, _icon: Icon, align: (HAlign, VAlign)) -> UiRect {
    icon_button_rect(slot, align)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::panel::panel_style;
    use crate::component::test_support::{palette_a, palette_b, FAMILY};
    #[test]
    fn button_measure_is_text_plus_padding() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let w = button_size("ОО", &mut m, &mut fs, FAMILY, 13.0).x;
        let text_w = m.width_of(&mut fs, "ОО", FAMILY, 13.0);
        assert!((w - (text_w + BUTTON_PAD_H * 2.0)).abs() < 0.01);
        // Пустая подпись — минимальная квадратная кнопка
        assert_eq!(
            button_size("", &mut m, &mut fs, FAMILY, 13.0).x,
            BUTTON_HEIGHT
        );
        assert_eq!(
            button_size("", &mut m, &mut fs, FAMILY, 13.0).y,
            BUTTON_HEIGHT
        );
    }
    #[test]
    fn button_in_narrow_slot_ellipsizes() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 40.0, BUTTON_HEIGHT);
        let layout = button_layout(
            slot,
            "Длинная подпись кнопки",
            (HAlign::Center, VAlign::Center),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(layout.label != "Длинная подпись кнопки");
        assert!(layout.rect.w <= slot.w + 0.01);
        // Широкий слот — подпись целиком
        let wide = UiRect::new(0.0, 0.0, 400.0, BUTTON_HEIGHT);
        let layout = button_layout(
            wide,
            "Длинная подпись кнопки",
            (HAlign::Center, VAlign::Center),
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert_eq!(layout.label, "Длинная подпись кнопки");
    }
    /// Контракт F-8: стиль собирается ТОЛЬКО выбором слота палитры — смена
    /// палитры меняет стиль; состояния берут РАЗНЫЕ слоты.
    #[test]
    fn styles_use_palette_slots_only() {
        let a = palette_a();
        let b = palette_b();
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Ghost,
            ButtonVariant::Danger,
        ] {
            let sa = button_style(variant, KitState::Normal, &a);
            let sb = button_style(variant, KitState::Normal, &b);
            assert_ne!(sa.fill, sb.fill, "{variant:?}: fill не из слота");
            assert_ne!(sa.text, sb.text, "{variant:?}: text не из слота");
        }
        // Состояния: hover/pressed/disabled — свои слоты
        let normal = button_style(ButtonVariant::Secondary, KitState::Normal, &a);
        let hovered = button_style(ButtonVariant::Secondary, KitState::Hovered, &a);
        assert_eq!(hovered.fill, a.hover_fill);
        assert_ne!(normal.fill, hovered.fill);
        let disabled = button_style(ButtonVariant::Secondary, KitState::Disabled, &a);
        assert_eq!(disabled.text, a.disabled_text);
        // Ghost в Normal — без рамки
        let ghost = button_style(ButtonVariant::Ghost, KitState::Normal, &a);
        assert_eq!(ghost.border[3], 0.0);
        // Чип: selected — слот selected
        assert_eq!(chip_style(KitState::Selected, &a).fill, a.selected_fill);
        // Радиусы — из radius-scale
        assert_eq!(normal.radius, canvas_core::tokens::RADIUS_CHIP);
        assert_eq!(panel_style(&a).radius, canvas_core::tokens::RADIUS_PANEL);
    }
    #[test]
    fn chip_measures_and_ellipsizes() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let c = chip_layout(
            UiPoint::new(10.0, 20.0),
            "Категория",
            f32::INFINITY,
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        let text_w = m.width_of(&mut fs, "Категория", FAMILY, 13.0);
        assert!((c.rect.w - (text_w + CHIP_PAD_H * 2.0)).abs() < 0.01);
        assert_eq!(c.rect.h, CHIP_HEIGHT);
        let c = chip_layout(
            UiPoint::new(10.0, 20.0),
            "Очень длинная категория чипа",
            60.0,
            &mut m,
            &mut fs,
            FAMILY,
            13.0,
        );
        assert!(c.label != "Очень длинная категория чипа");
        assert!(c.rect.w <= 60.0 + 0.01);
    }
    #[test]
    fn icon_button_is_square_in_slot() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let r = icon_button_rect(slot, (HAlign::End, VAlign::Start));
        assert_eq!(r.w, ICON_BUTTON_SIZE);
        assert_eq!(r.h, ICON_BUTTON_SIZE);
        assert!((r.right() - slot.right()).abs() < 0.01);
        assert!((r.y - slot.y).abs() < 0.01);
    }

    // === FR-058: тесты компонентов v2 =======================================

    // --- TextFieldModel: insert/backspace/delete/move/select/set_text -------
    #[test]
    fn switch_geometry_in_slot_and_knob_position_by_on() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 30.0);
        let p = palette_a();
        // ON
        let on = switch(slot, true, KitState::Normal, &p);
        assert!((on.track.w - SWITCH_W).abs() < 0.01);
        assert!((on.track.h - SWITCH_H).abs() < 0.01);
        assert!(
            (on.track.x - (slot.x + (slot.w - SWITCH_W) / 2.0)).abs() < 0.01,
            "трек по центру слота"
        );
        let knob_side = SWITCH_H - 2.0 * SWITCH_KNOB_PAD;
        assert!((on.knob.w - knob_side).abs() < 0.01);
        assert!((on.knob.h - knob_side).abs() < 0.01);
        // ON: knob у правого края трека
        assert!((on.knob.right() - (on.track.right() - SWITCH_KNOB_PAD)).abs() < 0.01);
        // OFF: knob у левого края трека
        let off = switch(slot, false, KitState::Normal, &p);
        assert!((off.knob.x - (off.track.x + SWITCH_KNOB_PAD)).abs() < 0.01);
    }
    #[test]
    fn switch_uses_palette_slots_for_fill() {
        let p = palette_a();
        // ON normal → control_primary
        let on = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Normal,
            &p,
        );
        assert_eq!(on.track_style.fill, p.control_primary);
        // OFF normal → control_fill
        let off = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            false,
            KitState::Normal,
            &p,
        );
        assert_eq!(off.track_style.fill, p.control_fill);
        // ON hovered → primary_hover_fill
        let on_h = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Hovered,
            &p,
        );
        assert_eq!(on_h.track_style.fill, p.primary_hover_fill);
        // OFF hovered → hover_fill
        let off_h = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            false,
            KitState::Hovered,
            &p,
        );
        assert_eq!(off_h.track_style.fill, p.hover_fill);
        // Knob fill: text_title (normal), disabled_text (disabled)
        assert_eq!(on.knob_fill, p.text_title);
        let dis = switch(
            UiRect::new(0.0, 0.0, 100.0, 30.0),
            true,
            KitState::Disabled,
            &p,
        );
        assert_eq!(dis.knob_fill, p.disabled_text);
        // Радиус — RADIUS_PILL
        assert_eq!(on.track_style.radius, canvas_core::tokens::RADIUS_PILL);
    }

    // --- card: геометрия в слоте, min/max, header/body ----------------------
    #[test]
    fn icon_glyph_mapping_is_complete() {
        // Все варианты Icon возвращают непустой &str
        for icon in [
            Icon::Close,
            Icon::Gear,
            Icon::Question,
            Icon::Search,
            Icon::Plus,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::Refresh,
        ] {
            let g = icon_glyph(icon);
            assert!(!g.is_empty(), "{icon:?} — пустой глиф");
        }
        // Существующие литералы потребителей сохранены (I-1 ноль скачка)
        assert_eq!(icon_glyph(Icon::Close), "×");
        assert_eq!(icon_glyph(Icon::Gear), "⚙");
        assert_eq!(icon_glyph(Icon::Question), "?");
        assert_eq!(icon_glyph(Icon::Plus), "+");
    }
    #[test]
    fn icon_button_delegates_to_icon_button_rect() {
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let align = (HAlign::End, VAlign::Start);
        // icon_button возвращает тот же rect, что icon_button_rect
        for icon in [
            Icon::Close,
            Icon::Gear,
            Icon::Question,
            Icon::Search,
            Icon::Plus,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::Refresh,
        ] {
            let r = icon_button(slot, icon, align);
            let expected = icon_button_rect(slot, align);
            assert_eq!(
                r, expected,
                "{icon:?}: icon_button делегирует icon_button_rect"
            );
        }
    }

    // === FR-062 F-17: focus_order ===
}
