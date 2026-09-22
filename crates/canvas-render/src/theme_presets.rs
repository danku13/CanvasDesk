//! Отображение тем-пресетов в [`ThemeColors`] (FR-047, PRD-0006 D4/F-8).
//!
//! Данные — `design/tokens/themes/*.json` через `canvas_core::theme_presets`
//! (реестр, разбор, кэш). Этот модуль — универсальное отображение
//! «имя слота → поле структуры»: добавление пресета НЕ меняет этот файл
//! (G2 PRD-0006: пресет = JSON + строка реестра в canvas-core).
//!
//! Единицы — те же, что во всей палитре: sRGB 0..1 (RGBA), байтовые слоты
//! округляются до u8 (`Color::rgb`).

use crate::theme::ThemeColors;
use crate::Color;
use canvas_core::theme_presets::{self, ParsedPreset};

/// Разобранный пресет → палитра рендера. Ключи гарантированы валидатором
/// ядра (I-47.1: набор строго равен REQUIRED_KEYS), поэтому отсутствие
/// ключа здесь — паника-инвариант, а не сценарий.
fn map_to_theme_colors(parsed: &ParsedPreset) -> ThemeColors {
    let c = &parsed.colors;
    let bytes = |key: &str| {
        let v = &c[key];
        [
            (v[0] * 255.0).round() as u8,
            (v[1] * 255.0).round() as u8,
            (v[2] * 255.0).round() as u8,
        ]
    };
    let rgba = |key: &str| c[key];
    let rgb3 = |key: &str| {
        let v = &c[key];
        [v[0], v[1], v[2]]
    };
    let color = |key: &str| {
        let [r, g, b, _] = c[key];
        Color::rgb(
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    };
    // PRD-0007 (F-4/AC-3.4): слот explain_leaf в JSON-пресетах не хранится
    // (валидатор I-47.1 фиксирует набор ключей) — выводится по яркости фона:
    // светлый фон → затемнённый слот EXPLAIN_LEAF_LIGHT, тёмный → EXPLAIN_LEAF
    // (те же константы, что dark()/light() в theme.rs; контраст ≥ 3:1).
    let [br, bg, bb, _] = c["background"];
    let explain_leaf = if 0.2126 * br + 0.7152 * bg + 0.0722 * bb > 0.5 {
        canvas_core::tokens::EXPLAIN_LEAF_LIGHT
    } else {
        canvas_core::tokens::EXPLAIN_LEAF
    };
    // FR-053 (U3 F-9): слоты состояний контролов в JSON-пресетах не хранятся
    // (валидатор I-47.1 фиксирует набор ключей) — выводятся: hover/selected —
    // формула hover_fill (c·1.3+0.04; синий канал +0.06 — контракт бывшего
    // вычисления app.rs:1042-1049, ноль скачка) от menu_fill пресета;
    // primary hover и disabled-текст — тематически-независимые примитивы
    // tokens.rs (как dark()/light() в theme.rs).
    let menu = rgba("menu_fill");
    let control_hover = [
        (menu[0] * 1.3 + 0.04).min(1.0),
        (menu[1] * 1.3 + 0.04).min(1.0),
        (menu[2] * 1.3 + 0.06).min(1.0),
        menu[3],
    ];
    ThemeColors {
        background: bytes("background"),
        grid_minor: rgb3("grid_minor"),
        grid_major: rgb3("grid_major"),
        card_fill: rgba("card_fill"),
        edge_edit_fill: rgba("edge_edit_fill"),
        edge_label_fill: rgba("edge_label_fill"),
        menu_fill: rgba("menu_fill"),
        search_input_fill: rgba("search_input_fill"),
        search_row_fill: rgba("search_row_fill"),
        palette_row_fill: rgba("palette_row_fill"),
        palette_chip_fill: rgba("palette_chip_fill"),
        palette_tile_fill: rgba("palette_tile_fill"),
        palette_selected_fill: rgba("palette_selected_fill"),
        palette_hover_fill: rgba("palette_hover_fill"),
        palette_border: rgba("palette_border"),
        title: color("title"),
        icon: color("icon"),
        body: color("body"),
        edge_label: color("edge_label"),
        link: color("link"),
        quote: color("quote"),
        code_text: color("code_text"),
        gfm_code_fill: rgba("gfm_code_fill"),
        gfm_quote_fill: rgba("gfm_quote_fill"),
        gfm_muted_fill: rgba("gfm_muted_fill"),
        group_fill: rgba("group_fill"),
        group_border: rgba("group_border"),
        guide_align: rgba("guide_align"),
        guide_grid: rgba("guide_grid"),
        accent: rgba("accent"),
        selection_fill: rgba("selection_fill"),
        highlight: rgba("highlight"),
        whatif_fill: rgba("whatif_fill"),
        whatif_badge: color("whatif_badge"),
        error: color("error"),
        hud: color("hud"),
        stage_dim: rgba("stage_dim"),
        explain_leaf,
        control_hover_fill: control_hover,
        control_primary_hover_fill: canvas_core::tokens::CONTROL_PRIMARY_HOVER_FILL,
        control_selected_fill: control_hover,
        control_disabled_text: Color::rgb(
            canvas_core::tokens::CONTROL_DISABLED_TEXT[0],
            canvas_core::tokens::CONTROL_DISABLED_TEXT[1],
            canvas_core::tokens::CONTROL_DISABLED_TEXT[2],
        ),
    }
}

/// Палитра пресета по id: `None` — неизвестный id или невалидный документ
/// (невалидность ловится тестами; прод-путь деградирует к классике —
/// см. [`ThemeColors::from_settings`]).
pub fn preset_theme(id: &str) -> Option<ThemeColors> {
    theme_presets::parsed(id).map(map_to_theme_colors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contrast::{contrast_ratio, contrast_text_vs_fill};

    fn bg3(t: &ThemeColors) -> [f32; 3] {
        [
            t.background[0] as f32 / 255.0,
            t.background[1] as f32 / 255.0,
            t.background[2] as f32 / 255.0,
        ]
    }

    /// Все пресеты реестра отображаются в палитру; is_dark совпадает с
    /// заявленным в JSON флагом dark (фон-сумма — тот же механизм, что
    /// у классики). Список светлых — явный (состав Q3, тест обновляется
    /// вместе с реестром; паритет флага — тест ключей в canvas-core).
    #[test]
    fn presets_map_and_is_dark_matches_flag() {
        for preset in canvas_core::theme_presets::PRESETS {
            let want_dark = !matches!(preset.id, "catppuccin-latte" | "solarized-light");
            let theme = preset_theme(preset.id).unwrap_or_else(|| panic!("{}", preset.id));
            assert_eq!(
                theme.is_dark(),
                want_dark,
                "{}: is_dark(фон {:?}) != флаг dark",
                preset.id,
                theme.background
            );
        }
    }

    /// G3 (WCAG): контраст каждого пресета — графика к фону ≥ 3:1
    /// (акцент, направляющие, error, HUD, what-if бейдж).
    #[test]
    fn presets_graphics_contrast_vs_background() {
        for preset in canvas_core::theme_presets::PRESETS {
            let t = preset_theme(preset.id).unwrap();
            let bg = bg3(&t);
            let graphics: Vec<(&str, [f32; 3])> = vec![
                ("accent", [t.accent[0], t.accent[1], t.accent[2]]),
                (
                    "guide_align",
                    [t.guide_align[0], t.guide_align[1], t.guide_align[2]],
                ),
                (
                    "guide_grid",
                    [t.guide_grid[0], t.guide_grid[1], t.guide_grid[2]],
                ),
                (
                    "error",
                    [
                        t.error.r() as f32 / 255.0,
                        t.error.g() as f32 / 255.0,
                        t.error.b() as f32 / 255.0,
                    ],
                ),
                (
                    "hud",
                    [
                        t.hud.r() as f32 / 255.0,
                        t.hud.g() as f32 / 255.0,
                        t.hud.b() as f32 / 255.0,
                    ],
                ),
                (
                    "whatif_badge",
                    [
                        t.whatif_badge.r() as f32 / 255.0,
                        t.whatif_badge.g() as f32 / 255.0,
                        t.whatif_badge.b() as f32 / 255.0,
                    ],
                ),
            ];
            for (name, fg) in graphics {
                let r = contrast_ratio(fg, bg);
                assert!(r >= 3.0, "{}: {name} {r:.2} < 3:1 к фону {bg:?}", preset.id);
            }
        }
    }

    /// G3 (WCAG): тексты каждого пресета к карточке ≥ 4.5:1 (заголовок,
    /// тело, лейбл связи), code_text к подложке кода ≥ 4.5:1;
    /// приглушённые роли (icon/link/quote) к карточке ≥ 3:1.
    #[test]
    fn presets_text_contrast_vs_card() {
        for preset in canvas_core::theme_presets::PRESETS {
            let t = preset_theme(preset.id).unwrap();
            for (name, ink) in [
                ("title", t.title),
                ("body", t.body),
                ("edge_label", t.edge_label),
            ] {
                let r = contrast_text_vs_fill(ink, t.card_fill);
                assert!(r >= 4.5, "{}: {name} {r:.2} < 4.5 к карточке", preset.id);
            }
            let r = contrast_text_vs_fill(t.code_text, t.gfm_code_fill);
            assert!(
                r >= 4.5,
                "{}: code_text {r:.2} < 4.5 к подложке кода",
                preset.id
            );
            for (name, ink) in [("icon", t.icon), ("link", t.link), ("quote", t.quote)] {
                let r = contrast_text_vs_fill(ink, t.card_fill);
                assert!(r >= 3.0, "{}: {name} {r:.2} < 3.0 к карточке", preset.id);
            }
        }
    }

    /// Деградация: неизвестный/пустой id — None; from_settings с неизвестным
    /// пресетом возвращает классическую тему (мягкий fallback старых
    /// конфигов с устаревшим id).
    #[test]
    fn unknown_id_falls_back_to_classic() {
        assert!(preset_theme("").is_none());
        assert!(preset_theme("monokai").is_none());
        assert_eq!(
            ThemeColors::from_settings(canvas_core::Theme::Dark, "monokai"),
            ThemeColors::dark()
        );
        assert_eq!(
            ThemeColors::from_settings(canvas_core::Theme::Light, ""),
            ThemeColors::light()
        );
        // Известный пресет перекрывает классику независимо от `theme`
        assert_eq!(
            ThemeColors::from_settings(canvas_core::Theme::Dark, "nord"),
            preset_theme("nord").unwrap()
        );
    }
}
