//! Wave C §5.3.6: CommandPalette — поиск действий (Cmd+K).
//!
//! Анатомия: `input` (поиск) + `results` (N actions) + `selected`.
//! Keyboard: ↑/↓ navigate, Enter activate, Esc close, Tab autocomplete.
//!
//! Note: Wave A добавит ActionRegistry — CommandPalette примет `&ActionRegistry`
//! как источник действий. Сейчас — generic по slice `&[CommandAction]`.

use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Категория действия (Wave C §5.3.6, для группировки в palette).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    File,
    Edit,
    View,
    Navigate,
    Tools,
    Help,
    Ai,
}

/// Действие для command palette (Wave C §5.3.6).
/// Wave A заменит на Action из ActionRegistry.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandAction {
    pub id: String,
    pub label: String,
    /// Shortcut (kbd-string: "cmd+k", "shift+0").
    pub kbd: Option<String>,
    pub category: CommandCategory,
}

/// Раскладка command palette (Wave C §5.3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandPaletteLayout {
    /// Rect поля ввода (search).
    pub input: UiRect,
    /// Rect списка результатов.
    pub results: Vec<(UiRect, usize)>, // (rect, action_index)
    /// Индекс выбранного результата.
    pub selected: usize,
}

/// Вёрстка command palette: центрировано-top, 560px wide, 8 results visible.
#[allow(clippy::too_many_arguments)] // прецедент kit: layout-функции
pub fn command_palette_layout(
    viewport: UiRect,
    results: &[usize], // индексы отфильтрованных действий
    selected: usize,
    input_h: f32,
    row_h: f32,
    max_visible: usize,
    _m: &mut TextMeasurer,
    _fs: &mut cosmic_text::FontSystem,
) -> CommandPaletteLayout {
    let width = 560.0_f32.min(viewport.w - 80.0);
    let x = viewport.x + (viewport.w - width).max(0.0) / 2.0;
    let y = viewport.y + 60.0; // top-center, отступ 60px
    let input = UiRect::new(x, y, width, input_h);
    let visible = results.len().min(max_visible);
    let results_layout = (0..visible)
        .map(|i| {
            let row_y = y + input_h + i as f32 * row_h;
            (UiRect::new(x, row_y, width, row_h), results[i])
        })
        .collect();
    CommandPaletteLayout {
        input,
        results: results_layout,
        selected,
    }
}

/// Fuzzy search по действиям: возвращает индексы matching actions.
/// Простая подстрока (case-insensitive); Wave A может заменить на fuzzy-matcher.
pub fn search_actions(actions: &[CommandAction], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..actions.len()).collect();
    }
    let q = query.to_lowercase();
    actions
        .iter()
        .enumerate()
        .filter(|(_, a)| a.label.to_lowercase().contains(&q))
        .map(|(i, _)| i)
        .collect()
}

/// Keyboard: ↑/↓ navigate, Enter activate, Esc close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPaletteKey {
    Prev,   // ↑
    Next,   // ↓
    Enter,  // activate
    Escape, // close
}

/// Hit-test: возвращает индекс результата по точке.
pub fn command_palette_hit(layout: &CommandPaletteLayout, p: UiPoint) -> Option<usize> {
    layout
        .results
        .iter()
        .find(|(r, _)| r.contains(p))
        .map(|(_, i)| *i)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_actions() -> Vec<CommandAction> {
        vec![
            CommandAction {
                id: "open_settings".into(),
                label: "Открыть настройки".into(),
                kbd: Some("ctrl+,".into()),
                category: CommandCategory::File,
            },
            CommandAction {
                id: "toggle_whatif".into(),
                label: "Переключить what-if".into(),
                kbd: Some("ctrl+w".into()),
                category: CommandCategory::View,
            },
            CommandAction {
                id: "save".into(),
                label: "Сохранить канвас".into(),
                kbd: Some("ctrl+s".into()),
                category: CommandCategory::File,
            },
        ]
    }

    #[test]
    fn search_empty_query_returns_all() {
        let actions = test_actions();
        let result = search_actions(&actions, "");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn search_substring_case_insensitive() {
        let actions = test_actions();
        let result = search_actions(&actions, "настрой");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 0); // "Открыть настройки"
                                  // "сохран" matches "Сохранить"
        let result = search_actions(&actions, "сохран");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 2);
    }

    #[test]
    fn command_palette_layout_centered_top() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let viewport = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let results = vec![0, 1, 2];
        let lay = command_palette_layout(viewport, &results, 0, 40.0, 32.0, 8, &mut m, &mut fs);
        // width = 560, centered: x = (1280-560)/2 = 360
        assert!((lay.input.w - 560.0).abs() < 0.01);
        assert!((lay.input.x - 360.0).abs() < 0.01);
        // y = 60 (top-center)
        assert!((lay.input.y - 60.0).abs() < 0.01);
        // 3 results
        assert_eq!(lay.results.len(), 3);
    }

    #[test]
    fn command_palette_layout_max_visible() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let viewport = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let results = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]; // 10
        let lay = command_palette_layout(viewport, &results, 0, 40.0, 32.0, 8, &mut m, &mut fs);
        // max_visible=8 → only 8 shown
        assert_eq!(lay.results.len(), 8);
    }

    #[test]
    fn hit_test_resolves_result_rows() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let viewport = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let results = vec![0, 1, 2];
        let lay = command_palette_layout(viewport, &results, 0, 40.0, 32.0, 8, &mut m, &mut fs);
        // Click on input
        assert_eq!(command_palette_hit(&lay, UiPoint::new(400.0, 70.0)), None);
        // Click on first result (y = 60 + 40 = 100..132)
        assert_eq!(
            command_palette_hit(&lay, UiPoint::new(400.0, 110.0)),
            Some(0)
        );
    }
}
