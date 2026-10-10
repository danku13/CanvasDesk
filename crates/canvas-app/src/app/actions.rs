//! INT-2 (#44) P3: ActionRegistry — единый реестр действий приложения
//! (Wave A §5.5.3, паттерн Excalidraw `shapeActionPredicates`).
//!
//! Один реестр → context menu, command palette (Cmd+K), `?` cheatsheet,
//! keyboard shortcuts. Сейчас реестр хранится на `App` и доступен через
//! [`canvas_app::App::command_actions`]; функциональная палитра команд
//! (потребитель `ActionRegistry::command_actions()` + Wave C
//! `search_actions`) подключается отдельной волной.
//!
//! kbd-строки — канонические имена горячих клавиш. Текущий обработчик
//! клавиатуры (KeyboardRouter + `ui_registry`) реестр НЕ читает — он
//! остаётся источником правды для будущих поверхностей (палитра,
//! cheatsheet, контекстное меню). `predicate` не используется — все
//! действия видны всегда (gating — задача потребителя реестра).

use canvas_ui::kit::{Action, ActionRegistry, CommandCategory};

/// Построить реестр действий приложения (INT-2 P3).
///
/// 13 действий по 5 категориям: File (save/open_settings/new_canvas),
/// View (toggle_whatif/toggle_theme/close_all), Navigate
/// (open_search/open_gallery/open_command_palette), Edit
/// (undo/redo/duplicate), Help (open_help).
pub fn build_action_registry() -> ActionRegistry {
    let mut reg = ActionRegistry::new();
    // File
    reg.register(Action::new(
        "open_settings",
        "Открыть настройки",
        CommandCategory::File,
    ));
    reg.register(Action::new("save", "Сохранить канвас", CommandCategory::File).kbd("ctrl+s"));
    reg.register(Action::new("new_canvas", "Новый канвас", CommandCategory::File).kbd("ctrl+n"));
    // View
    reg.register(
        Action::new(
            "toggle_whatif",
            "Переключить what-if",
            CommandCategory::View,
        )
        .kbd("ctrl+w"),
    );
    reg.register(Action::new(
        "toggle_theme",
        "Сменить тему",
        CommandCategory::View,
    ));
    reg.register(
        Action::new("close_all", "Закрыть все панели", CommandCategory::View).kbd("escape"),
    );
    // Navigate
    reg.register(Action::new("open_search", "Поиск", CommandCategory::Navigate).kbd("ctrl+f"));
    reg.register(Action::new(
        "open_gallery",
        "Галерея схем",
        CommandCategory::Navigate,
    ));
    reg.register(
        Action::new(
            "open_command_palette",
            "Палитра команд",
            CommandCategory::Navigate,
        )
        .kbd("ctrl+k"),
    );
    // Edit
    reg.register(Action::new("undo", "Отменить", CommandCategory::Edit).kbd("ctrl+z"));
    reg.register(Action::new("redo", "Повторить", CommandCategory::Edit).kbd("ctrl+shift+z"));
    reg.register(Action::new("duplicate", "Дублировать", CommandCategory::Edit).kbd("ctrl+d"));
    // Help
    reg.register(Action::new("open_help", "Справка", CommandCategory::Help).kbd("?"));
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_at_least_10_actions() {
        let reg = build_action_registry();
        assert!(
            reg.len() >= 10,
            "реестр должен содержать не менее 10 действий, фактически {}",
            reg.len()
        );
    }

    #[test]
    fn registry_contains_expected_actions() {
        let reg = build_action_registry();
        for id in [
            "open_settings",
            "toggle_whatif",
            "save",
            "open_search",
            "open_gallery",
            "toggle_theme",
            "close_all",
            "open_help",
            "undo",
            "redo",
        ] {
            assert!(reg.get(id).is_some(), "ожидается действие {}", id);
        }
    }

    #[test]
    fn registry_shortcut_of_save() {
        let reg = build_action_registry();
        assert_eq!(reg.shortcut_of("save"), Some("ctrl+s"));
        assert_eq!(reg.shortcut_of("open_command_palette"), Some("ctrl+k"));
        assert_eq!(reg.shortcut_of("toggle_theme"), None);
    }

    #[test]
    fn registry_command_actions_for_palette() {
        let reg = build_action_registry();
        let cmds = reg.command_actions();
        assert!(cmds.len() >= 10);
        assert!(cmds.iter().any(|c| c.id == "save"));
        assert!(cmds.iter().any(|c| c.id == "open_command_palette"));
    }

    #[test]
    fn registry_search_by_label() {
        let reg = build_action_registry();
        let result = reg.search("сохран");
        assert_eq!(result.len(), 1);
        assert_eq!(reg.actions()[result[0]].id, "save");
    }

    #[test]
    fn registry_cheatsheet_groups_by_category() {
        let reg = build_action_registry();
        let groups = reg.cheatsheet();
        // File: open_settings, save, new_canvas → 3
        let file_count = groups
            .iter()
            .find(|(c, _)| *c == CommandCategory::File)
            .map(|(_, a)| a.len())
            .unwrap_or(0);
        assert_eq!(file_count, 3);
        // Edit: undo, redo, duplicate → 3
        let edit_count = groups
            .iter()
            .find(|(c, _)| *c == CommandCategory::Edit)
            .map(|(_, a)| a.len())
            .unwrap_or(0);
        assert_eq!(edit_count, 3);
        // Help: open_help → 1
        let help_count = groups
            .iter()
            .find(|(c, _)| *c == CommandCategory::Help)
            .map(|(_, a)| a.len())
            .unwrap_or(0);
        assert_eq!(help_count, 1);
    }

    #[test]
    fn registry_context_items_filters_available() {
        let reg = build_action_registry();
        // Все действия без predicate → все доступны
        let items = reg.context_items();
        assert_eq!(items.len(), reg.len());
    }
}
