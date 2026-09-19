//! M8/W11 (wasm-port §5, §3.2): виджеты на web — снапшот/плейсхолдер,
//! реестр встроенных пакетов в памяти, widget_state в localStorage,
//! тик LOD/refresh из setInterval.
//!
//! Состав (карта замен §3.2):
//! - **реестр**: `WidgetRegistry::in_memory()` (выбор — в `App::new` по
//!   каталогу кэша), инициализация — общий `App::init_widgets()` (тот же
//!   путь, что в нативном main.rs). Осознанное отклонение от натива:
//!   tombstone удалённых встроенных пакетов живёт в памяти сессии (F5
//!   возвращает пакет) — экспорт/импорт API у реестра есть
//!   (`memory_tombstones`/`set_memory_tombstones`), сценарий сохранения —
//!   W12/волна 2 (в волне 1 узел-виджет и так рендерится плейсхолдером);
//! - **widget_state (T21-E)**: [`WebWidgetState`] — синхронный трейт
//!   `WidgetStateBackend` над localStorage (одна JSON-запись
//!   `canvasdesk.widget_state`, карта `{"node": {"key": "value"}}`);
//!   недоступный localStorage — деградация (пустые чтения, no-op запись,
//!   warn — как у конфига W6);
//! - **тик**: `setInterval` 1 с → `WidgetEvent::Tick` через
//!   `EventLoopProxy` (зеркало тик-потока «widget-tick» main.rs;
//!   `std::thread` на wasm недоступен, план §6.1 «gloo-interval — W11» —
//!   обошлись web-sys без новых зависимостей).
//!
//! LOD-деградация «как на Linux» — отдельного кода не требует: у
//! WidgetManager `runtime_ok() == false` вне Windows → все виджет-ноды
//! Placeholder (серая карточка-заглушка), общий код app/widgets.

// Чистая часть (StateMap) на нативе живёт только в тестах (web-обёртка —
// wasm-only) — паттерн web_state.rs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Ключ localStorage: одна JSON-запись на всё объёмное состояние виджетов
/// (записи мелкие — черновики/настройки инстансов; перезапись целиком
/// дешевле квоты и держит формат в одном месте).
pub(crate) const STATE_KEY: &str = "canvasdesk.widget_state";

// ============================================================================
// Карта состояния — чистая логика (нативные тесты внизу модуля)
// ============================================================================

/// Карта объёмного состояния виджетов: `(node_id, key) → value`.
/// Формат хранения — JSON-объект `{"node": {"key": "value"}}` (план §3.2
/// «localStorage (serde_json)»); изоляция нод — структурой вложенности
/// (T21: виджет не видит чужие ключи — API принимает node_id из контекста).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct StateMap {
    entries: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
}

impl StateMap {
    /// Разбор сохранённого JSON; битый текст — пустая карта (ошибка —
    /// у вызывающего: warn + старт с пустым состоянием, страница не падает).
    pub(crate) fn from_json(text: &str) -> (Self, Result<(), String>) {
        match serde_json::from_str(text) {
            Ok(entries) => (Self { entries }, Ok(())),
            Err(err) => (Self::default(), Err(err.to_string())),
        }
    }

    /// Сериализация (формат с сортировкой BTreeMap — стабильные строки,
    /// меньше перезаписей localStorage).
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(&self.entries).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Значение ключа ноды.
    pub(crate) fn get(&self, node_id: &str, key: &str) -> Option<String> {
        self.entries.get(node_id)?.get(key).cloned()
    }

    /// Записать значение (upsert).
    pub(crate) fn set(&mut self, node_id: &str, key: &str, value: &str) {
        self.entries
            .entry(node_id.to_owned())
            .or_default()
            .insert(key.to_owned(), value.to_owned());
    }

    /// Число пар (node, key) — диагностика/лог готовности.
    pub(crate) fn len(&self) -> usize {
        self.entries.values().map(|keys| keys.len()).sum()
    }
}

// ============================================================================
// Web-часть: localStorage-бэкенд и тик — только под wasm (JS-рунтайм)
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub(crate) mod web {
    use super::StateMap;
    use canvas_core::WidgetStateBackend;
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;

    /// Хранилище widget_state в localStorage ([`WidgetStateBackend`],
    /// T21-E → web W11). Синхронный контракт трейта ложится на
    /// синхронный localStorage — без spawn_local/очередей.
    pub(crate) struct WebWidgetState;

    impl WebWidgetState {
        /// Создать бэкенд и рапортовать готовность (INFO-оракул дыма:
        /// число восстановленных пар — после F5 состояние читается).
        pub(crate) fn new() -> Self {
            let (map, parse_err) = Self::read();
            if let Err(err) = parse_err {
                tracing::warn!(target: "canvas_web", error = %err, "widget_state: битый JSON в localStorage — старт с пустым");
            }
            tracing::info!(target: "canvas_web", keys = map.len(), "widget_state: localStorage готов");
            Self
        }

        /// Прочитать карту из localStorage; недоступен — пустая (деградация).
        fn read() -> (StateMap, Result<(), String>) {
            let Some(text) = local_storage().and_then(|s| s.get_item(super::STATE_KEY).ok()) else {
                return (StateMap::default(), Ok(()));
            };
            let Some(text) = text else {
                return (StateMap::default(), Ok(()));
            };
            StateMap::from_json(&text)
        }

        /// Сохранить карту; недоступен/квота — warn, состояние канваса не
        /// затрагивается (данные виджета, не раскладка — как у натива).
        fn write(map: &StateMap) {
            let Some(storage) = local_storage() else {
                return;
            };
            if let Err(err) = storage.set_item(super::STATE_KEY, &map.to_json()) {
                tracing::warn!(target: "canvas_web", error = ?err, "widget_state: запись в localStorage не удалась");
            }
        }
    }

    impl WidgetStateBackend for WebWidgetState {
        fn get(&self, node_id: &str, key: &str) -> Option<String> {
            Self::read().0.get(node_id, key)
        }

        fn set(&mut self, node_id: &str, key: &str, value: &str) {
            let (mut map, parse_err) = Self::read();
            if let Err(err) = parse_err {
                tracing::warn!(target: "canvas_web", error = %err, "widget_state: битый JSON перезаписан");
            }
            map.set(node_id, key, value);
            Self::write(&map);
        }
    }

    /// localStorage страницы (None — приватный режим/отказ: деградация).
    fn local_storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    /// Тик виджетов (1 с): `WidgetEvent::Tick` через `EventLoopProxy` —
    /// побудка цикла для LOD/refresh-расписаний (зеркало тик-потока
    /// main.rs; на web — setInterval, новые зависимости не нужны).
    /// Провал — warn: тика нет, refresh-расписания двигают кадры ввода
    /// (деградация R14).
    pub(crate) fn install_tick(sender: canvas_widgets::WidgetEventSender) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let closure: Closure<dyn FnMut()> = Closure::new(move || {
            sender(canvas_widgets::WidgetEvent::Tick);
        });
        let callback = closure.as_ref().unchecked_ref::<js_sys::Function>();
        // 1000 мс — константа нативного тик-потока (main.rs, widget-tick);
        // web-sys 0.3: интервал задаёт вариант *_and_timeout_and_arguments
        if let Err(err) = window.set_interval_with_callback_and_timeout_and_arguments(
            callback,
            1000,
            &js_sys::Array::new(),
        ) {
            tracing::warn!(target: "canvas_web", error = ?err, "тик виджетов не установлен");
            return; // closure дропнется — тика нет (деградация)
        }
        closure.forget(); // живёт до выгрузки страницы (singleton)
    }
}

// ============================================================================
// Тесты чистой части (нативные, без JS-рунтайма)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::StateMap;

    #[test]
    fn state_map_roundtrip_and_upsert() {
        let mut map = StateMap::default();
        assert_eq!(map.len(), 0);
        assert_eq!(map.get("widget-1", "draft"), None);
        map.set("widget-1", "draft", "привет");
        assert_eq!(map.get("widget-1", "draft"), Some("привет".into()));
        // Upsert перезаписывает
        map.set("widget-1", "draft", "мир");
        assert_eq!(map.get("widget-1", "draft"), Some("мир".into()));
        // JSON-круг: сериализация → разбор → значения на месте
        let json = map.to_json();
        let (restored, err) = StateMap::from_json(&json);
        assert!(err.is_ok(), "{err:?}");
        assert_eq!(restored, map);
        assert_eq!(restored.len(), 1);
    }

    #[test]
    fn state_map_isolates_nodes() {
        // Изоляция (T21): ключи одной ноды не смешиваются с другой
        let mut map = StateMap::default();
        map.set("widget-1", "k", "v1");
        map.set("widget-2", "k", "v2");
        assert_eq!(map.get("widget-1", "k"), Some("v1".into()));
        assert_eq!(map.get("widget-2", "k"), Some("v2".into()));
        assert_eq!(map.get("widget-3", "k"), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn state_map_survives_unicode_and_special_keys() {
        // Ключи виджетов — произвольные строки (JSON-хранение обязан
        // переносить кириллицу/кавычки/переводы строк без потерь)
        let mut map = StateMap::default();
        let weird = "ключ\"с\\кавычками\nи\nпереводами";
        map.set("widget-1", weird, "значение ✓");
        let (restored, err) = StateMap::from_json(&map.to_json());
        assert!(err.is_ok(), "{err:?}");
        assert_eq!(
            restored.get("widget-1", weird).as_deref(),
            Some("значение ✓")
        );
    }

    #[test]
    fn state_map_broken_json_starts_empty() {
        let (map, err) = StateMap::from_json("{не json");
        assert_eq!(map.len(), 0, "битый текст — пустая карта");
        assert!(err.is_err(), "ошибка разбора возвращена вызывающему");
        // Пустой/отсутствующий — валидный старт
        let (map, err) = StateMap::from_json("{}");
        assert!(err.is_ok() && map.len() == 0);
    }
}
