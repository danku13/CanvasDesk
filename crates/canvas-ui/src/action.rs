//! Wave A §5.5.3: ActionRegistry — единый реестр действий.
//!
//! Один реестр → context menu, command palette, `?` cheatsheet, keyboard
//! shortcuts. Excalidraw `shapeActionPredicates` + `CommandPalette` pattern.
//!
//! Wave C `CommandPalette` принимает `&[CommandAction]` — `ActionRegistry`
//! поставляет actions через `search()` и `context_items()`.

use crate::component::command_palette::{CommandAction, CommandCategory};

/// Зарегистрированное действие (Wave A §5.5.3).
///
/// `predicate` — виден/доступен ли action в текущем состоянии (gating).
/// `handler` — вызывается при активации (consumer передаёт mutable state).
#[derive(Clone)]
pub struct Action {
    pub id: String,
    pub label: String,
    /// Shortcut (kbd-string: "cmd+k", "shift+0").
    pub kbd: Option<String>,
    pub category: CommandCategory,
    /// Predicate: виден/доступен ли action. None — всегда виден.
    pub predicate: Option<fn() -> bool>,
}

impl Action {
    pub fn new(id: impl Into<String>, label: impl Into<String>, category: CommandCategory) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kbd: None,
            category,
            predicate: None,
        }
    }

    pub fn kbd(mut self, kbd: impl Into<String>) -> Self {
        self.kbd = Some(kbd.into());
        self
    }

    pub fn predicate(mut self, p: fn() -> bool) -> Self {
        self.predicate = Some(p);
        self
    }

    /// Виден/доступен ли action (predicate true или None).
    pub fn is_available(&self) -> bool {
        self.predicate.map_or(true, |p| p())
    }

    /// Конвертировать в CommandAction для Wave C CommandPalette.
    pub fn to_command_action(&self) -> CommandAction {
        CommandAction {
            id: self.id.clone(),
            label: self.label.clone(),
            kbd: self.kbd.clone(),
            category: self.category,
        }
    }
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Action")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("kbd", &self.kbd)
            .field("category", &self.category)
            .field("has_predicate", &self.predicate.is_some())
            .finish()
    }
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Реестр действий (Wave A §5.5.3).
///
/// Один источник правды для: context menu, command palette (Cmd+K),
/// `?` cheatsheet, keyboard shortcuts. Идемпотентный register (по id).
#[derive(Debug, Clone, Default)]
pub struct ActionRegistry {
    actions: Vec<Action>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Зарегистрировать action (идемпотентно по id — обновляет если есть).
    pub fn register(&mut self, action: Action) {
        if let Some(existing) = self.actions.iter_mut().find(|a| a.id == action.id) {
            *existing = action;
        } else {
            self.actions.push(action);
        }
    }

    /// Все зарегистрированные actions.
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

    /// Поиск по query (fuzzy substring, case-insensitive) + predicate filter.
    /// Возвращает индексы matching actions (для CommandPalette).
    pub fn search(&self, query: &str) -> Vec<usize> {
        let q = query.to_lowercase();
        self.actions
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_available())
            .filter(|(_, a)| query.is_empty() || a.label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    /// Actions для command palette (как CommandAction, отфильтрованные predicate).
    pub fn command_actions(&self) -> Vec<CommandAction> {
        self.actions
            .iter()
            .filter(|a| a.is_available())
            .map(|a| a.to_command_action())
            .collect()
    }

    /// Shortcut для action по id (для KeyboardRouter binding).
    pub fn shortcut_of(&self, id: &str) -> Option<&str> {
        self.actions
            .iter()
            .find(|a| a.id == id)
            .and_then(|a| a.kbd.as_deref())
    }

    /// Cheatsheet: actions сгруппированы по категориям (для `?` dialog).
    pub fn cheatsheet(&self) -> Vec<(CommandCategory, Vec<&Action>)> {
        let mut groups: Vec<(CommandCategory, Vec<&Action>)> = Vec::new();
        for action in &self.actions {
            if !action.is_available() {
                continue;
            }
            if let Some(group) = groups.iter_mut().find(|(cat, _)| *cat == action.category) {
                group.1.push(action);
            } else {
                groups.push((action.category, vec![action]));
            }
        }
        groups
    }

    /// Context-menu items: actions, видимые в текущем состоянии
    /// (predicate=true), для контекстного меню канваса/selection.
    pub fn context_items(&self) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.is_available()).collect()
    }

    /// Action по id.
    pub fn get(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    /// Количество зарегистрированных actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_true() -> bool { true }
    fn always_false() -> bool { false }

    fn seed_registry() -> ActionRegistry {
        let mut reg = ActionRegistry::new();
        reg.register(Action::new("open_settings", "Открыть настройки", CommandCategory::File).kbd("ctrl+,").predicate(always_true));
        reg.register(Action::new("toggle_whatif", "Переключить what-if", CommandCategory::View).kbd("ctrl+w").predicate(always_true));
        reg.register(Action::new("save", "Сохранить канвас", CommandCategory::File).kbd("ctrl+s").predicate(always_true));
        reg.register(Action::new("delete_node", "Удалить ноду", CommandCategory::Edit).predicate(always_false)); // disabled — no selection
        reg
    }

    #[test]
    fn register_and_get() {
        let reg = seed_registry();
        assert_eq!(reg.len(), 4);
        let a = reg.get("save").unwrap();
        assert_eq!(a.label, "Сохранить канвас");
        assert_eq!(a.kbd.as_deref(), Some("ctrl+s"));
    }

    #[test]
    fn register_idempotent_by_id() {
        let mut reg = ActionRegistry::new();
        reg.register(Action::new("save", "Сохранить", CommandCategory::File));
        reg.register(Action::new("save", "Сохранить канвас", CommandCategory::File)); // update
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("save").unwrap().label, "Сохранить канвас");
    }

    #[test]
    fn search_empty_query_returns_all_available() {
        let reg = seed_registry();
        let result = reg.search("");
        // 3 available (delete_node predicate=false)
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn search_substring_case_insensitive() {
        let reg = seed_registry();
        let result = reg.search("настрой");
        assert_eq!(result.len(), 1);
        assert_eq!(reg.actions()[result[0]].id, "open_settings");
    }

    #[test]
    fn search_respects_predicate() {
        let reg = seed_registry();
        // delete_node has predicate=false → not in search
        let result = reg.search("удалить");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn shortcut_of() {
        let reg = seed_registry();
        assert_eq!(reg.shortcut_of("save"), Some("ctrl+s"));
        assert_eq!(reg.shortcut_of("delete_node"), None); // no kbd
        assert_eq!(reg.shortcut_of("nonexistent"), None);
    }

    #[test]
    fn cheatsheet_groups_by_category() {
        let reg = seed_registry();
        let groups = reg.cheatsheet();
        // File: open_settings, save; View: toggle_whatif; Edit: delete_node (predicate=false → hidden)
        let file_count = groups.iter().find(|(c, _)| *c == CommandCategory::File).map(|(_, a)| a.len()).unwrap_or(0);
        assert_eq!(file_count, 2);
        let view_count = groups.iter().find(|(c, _)| *c == CommandCategory::View).map(|(_, a)| a.len()).unwrap_or(0);
        assert_eq!(view_count, 1);
        // Edit: delete_node hidden (predicate=false)
        let edit_count = groups.iter().find(|(c, _)| *c == CommandCategory::Edit).map(|(_, a)| a.len()).unwrap_or(0);
        assert_eq!(edit_count, 0);
    }

    #[test]
    fn context_items_filters_predicate() {
        let reg = seed_registry();
        let items = reg.context_items();
        // 3 available (delete_node predicate=false)
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn command_actions_for_palette() {
        let reg = seed_registry();
        let cmds = reg.command_actions();
        assert_eq!(cmds.len(), 3); // predicate filtered
        assert!(cmds.iter().any(|c| c.id == "save"));
        assert!(!cmds.iter().any(|c| c.id == "delete_node")); // hidden
    }
}
