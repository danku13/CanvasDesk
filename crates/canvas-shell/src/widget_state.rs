//! Объёмное состояние виджетов (T21-E, план M5 §4.9): таблица
//! `widget_state(node_id, key, value)` в существующем `cache.db`.
//! Пересоздаваемая, как весь кэш (SPEC §5.3): ошибки = warn и пустой
//! результат у вызывающего, паники запрещены (AGENTS).
//!
//! Изоляция temp-директорий (T21): связка node_id+key принадлежит
//! конкретной ноде канваса — виджет не может читать чужие строки,
//! API принимает node_id только из контекста своего инстанса
//! (менеджер подставляет его сам, виджету он недоступен).

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// Хранилище widget_state (одна таблица в cache.db).
pub struct WidgetStateStore {
    conn: Connection,
}

impl WidgetStateStore {
    /// Открыть cache.db в `dir` (тот же файл, что у тамбнейл-кэша);
    /// создать таблицу при необходимости.
    pub fn open(dir: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(dir.join("cache.db"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS widget_state (
                 node_id TEXT NOT NULL,
                 key     TEXT NOT NULL,
                 value   TEXT NOT NULL,
                 PRIMARY KEY (node_id, key)
             );",
        )?;
        Ok(Self { conn })
    }

    /// Значение ключа ноды (нет строки/сбой — None: кэш пересоздаваем).
    pub fn get(&self, node_id: &str, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM widget_state WHERE node_id = ?1 AND key = ?2",
                params![node_id, key],
                |row| row.get(0),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Записать значение (upsert). Ошибка — warn, состояние канваса
    /// не затрагивается (данные виджета, не раскладка).
    pub fn set(&mut self, node_id: &str, key: &str, value: &str) {
        let res = self.conn.execute(
            "INSERT INTO widget_state (node_id, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(node_id, key) DO UPDATE SET value = excluded.value",
            params![node_id, key, value],
        );
        if let Err(e) = res {
            tracing::warn!(node_id, key, error = %e, "widget_state: запись не удалась");
        }
    }

    /// Все ключи ноды (для отладки/полного восстановления виджета).
    pub fn keys(&self, node_id: &str) -> Vec<String> {
        let Ok(mut stmt) = self
            .conn
            .prepare("SELECT key FROM widget_state WHERE node_id = ?1 ORDER BY key")
        else {
            return Vec::new();
        };
        stmt.query_map(params![node_id], |row| row.get::<_, String>(0))
            .map(|rows| rows.flatten().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cd_widget_state_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn set_get_roundtrip_and_upsert() {
        let dir = temp_dir("roundtrip");
        let mut store = WidgetStateStore::open(&dir).unwrap();
        assert_eq!(store.get("widget-1", "draft"), None);
        store.set("widget-1", "draft", "привет");
        assert_eq!(store.get("widget-1", "draft"), Some("привет".into()));
        // Upsert перезаписывает
        store.set("widget-1", "draft", "мир");
        assert_eq!(store.get("widget-1", "draft"), Some("мир".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn isolation_between_nodes() {
        // Изоляция (T21): строки одной ноды не видны другой
        let dir = temp_dir("isolation");
        let mut store = WidgetStateStore::open(&dir).unwrap();
        store.set("widget-1", "k", "v1");
        store.set("widget-2", "k", "v2");
        assert_eq!(store.get("widget-1", "k"), Some("v1".into()));
        assert_eq!(store.get("widget-2", "k"), Some("v2".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_sorted_and_scoped() {
        let dir = temp_dir("keys");
        let mut store = WidgetStateStore::open(&dir).unwrap();
        store.set("widget-1", "b", "1");
        store.set("widget-1", "a", "2");
        store.set("widget-2", "c", "3");
        assert_eq!(store.keys("widget-1"), vec!["a", "b"]);
        assert_eq!(store.keys("widget-2"), vec!["c"]);
        assert!(store.keys("widget-3").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reopen_survives() {
        // Переживает переоткрытие (тот же файл cache.db)
        let dir = temp_dir("reopen");
        {
            let mut store = WidgetStateStore::open(&dir).unwrap();
            store.set("widget-1", "note", "текст");
        }
        let store = WidgetStateStore::open(&dir).unwrap();
        assert_eq!(store.get("widget-1", "note"), Some("текст".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
