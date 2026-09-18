//! Протокол поискового индекса + платформенно-нейтральный трейт
//! [`SearchBackend`] (T14, M8/W3).
//!
//! Протокол переехал из `canvas-shell` (wasm-port §3.1/§6 W3): нативная
//! реализация — FTS5 worker в shell (`SearchService`), web/тестовая —
//! [`MemSearch`] (индекс в памяти, §3.2: «индекс по нодам в памяти, ответы
//! через тот же [`SearchEvent]»). Приложение (`canvas-app`) общается с
//! любым backend только через трейт.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// FTS5-запись для переиндексации сцены (загрузка канваса).
#[derive(Debug)]
pub struct IndexEntry {
    /// Путь файла ноды (ключ записи).
    pub path: PathBuf,
    /// Имя файла для отображения.
    pub display_name: String,
}

/// Команда worker-потоку индекса.
#[derive(Debug)]
pub enum SearchCommand {
    /// Upsert записи файла (имя + извлечённый текст).
    IndexFile {
        /// Путь (ключ).
        path: PathBuf,
        /// Имя для отображения.
        display_name: String,
    },
    /// Удалить запись файла (нода удалена / файл пропал).
    RemoveFile {
        /// Путь (ключ).
        path: PathBuf,
    },
    /// Полная переиндексация набора: upsert всех записей, удалить лишние.
    /// Сценарий: загрузка канваса, открытие другого файла.
    ReplaceAll {
        /// Новый набор записей.
        entries: Vec<IndexEntry>,
    },
    /// Поисковый запрос (ответ — `SearchEvent::Ready`).
    Query {
        /// Текст запроса (сырой, экранируется внутри).
        query: String,
        /// Максимум строк в ответе.
        limit: usize,
    },
}

/// Хит поиска: путь + имя (матчинг с нодами — в приложении через
/// `canvas_core::fs_events::path_matches`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Путь файла (ключ записи).
    pub path: PathBuf,
    /// Имя для отображения.
    pub display_name: String,
}

/// Событие от worker-потока индекса (в приложение через responder).
#[derive(Debug)]
pub enum SearchEvent {
    /// Ответ на `SearchCommand::Query`: хиты в порядке bm25 (лучшие первыми).
    Ready(Vec<SearchHit>),
    /// Индексация `ReplaceAll` завершена: число актуальных записей (для лога).
    Indexed(usize),
}

/// Ответчик событий поиска (обёртка над EventLoopProxy в приложении).
pub type SearchResponder = Arc<dyn Fn(SearchEvent) + Send + Sync>;

/// Трейт поискового индекса (T14, M8/W3): команды уходят в backend,
/// ответы приходят приложению через [`SearchResponder`], переданный
/// при создании backend'а. Неблокирующая отправка (worker-поток/канал).
pub trait SearchBackend {
    /// Отправить команду backend'у (неблокирующе; ошибки доставки —
    /// внутри реализации: приложение живёт без поиска, SPEC §5.3).
    fn command(&self, cmd: SearchCommand);
}

/// Поиск в памяти (M8/W3): заглушка для тестов и web-бинаря (план §3.2 —
/// «MemSearch: индекс по нодам в памяти»). Без извлечения текста файлов:
/// матчинг — подстрока имени (регистронезависимо), порядок — по ключу
/// пути (BTreeMap, детерминированно). Ответы — через тот же протокол.
pub struct MemSearch {
    entries: Mutex<BTreeMap<PathBuf, String>>,
    responder: SearchResponder,
}

impl MemSearch {
    /// Создать backend с ответчиком (та же обёртка над EventLoopProxy,
    /// что у FTS5-worker'а).
    pub fn new(responder: SearchResponder) -> Self {
        Self {
            entries: Mutex::new(BTreeMap::new()),
            responder,
        }
    }

    /// Снимок индекса (тесты).
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Пуст ли индекс (тесты).
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Отравленный Mutex не ломает backend: данные восстанавливаем
    /// (паттерн «паника в responder не убивает индекс»).
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PathBuf, String>> {
        self.entries.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl SearchBackend for MemSearch {
    fn command(&self, cmd: SearchCommand) {
        match cmd {
            SearchCommand::IndexFile { path, display_name } => {
                self.lock().insert(path, display_name);
            }
            SearchCommand::RemoveFile { path } => {
                self.lock().remove(&path);
            }
            SearchCommand::ReplaceAll { entries } => {
                let count = entries.len();
                *self.lock() = entries
                    .into_iter()
                    .map(|entry| (entry.path, entry.display_name))
                    .collect();
                (self.responder)(SearchEvent::Indexed(count));
            }
            SearchCommand::Query { query, limit } => {
                let needle = query.to_lowercase();
                let hits = if needle.is_empty() {
                    Vec::new()
                } else {
                    self.lock()
                        .iter()
                        .filter(|(_, name)| name.to_lowercase().contains(&needle))
                        .take(limit)
                        .map(|(path, display_name)| SearchHit {
                            path: path.clone(),
                            display_name: display_name.clone(),
                        })
                        .collect()
                };
                (self.responder)(SearchEvent::Ready(hits));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Responder, собирающий события в канал (тесты).
    fn channel_responder() -> (SearchResponder, mpsc::Receiver<SearchEvent>) {
        let (tx, rx) = mpsc::channel();
        let responder: SearchResponder = Arc::new(move |event| {
            let _ = tx.send(event);
        });
        (responder, rx)
    }

    /// ReplaceAll: индекс заменяется целиком, приходит Indexed(count).
    #[test]
    fn replace_all_replaces_and_reports() {
        let (responder, rx) = channel_responder();
        let search = MemSearch::new(responder);
        search.command(SearchCommand::ReplaceAll {
            entries: vec![
                IndexEntry {
                    path: PathBuf::from("/c/смета.xlsx"),
                    display_name: "смета.xlsx".into(),
                },
                IndexEntry {
                    path: PathBuf::from("/c/отчёт.md"),
                    display_name: "отчёт.md".into(),
                },
            ],
        });
        assert_eq!(search.len(), 2);
        assert!(matches!(
            rx.recv().expect("Indexed после ReplaceAll"),
            SearchEvent::Indexed(2)
        ));
        // Повторный ReplaceAll с меньшим набором — лишние записи удалены
        search.command(SearchCommand::ReplaceAll {
            entries: vec![IndexEntry {
                path: PathBuf::from("/c/отчёт.md"),
                display_name: "отчёт.md".into(),
            }],
        });
        assert_eq!(search.len(), 1);
    }

    /// Query: подстрока имени, регистронезависимо, лимит; пустой запрос —
    /// пустой ответ (как в FTS5-версии: пустая строка не матчится).
    #[test]
    fn query_matches_display_name_case_insensitive() {
        let (responder, rx) = channel_responder();
        let search = MemSearch::new(responder);
        search.command(SearchCommand::ReplaceAll {
            entries: vec![
                IndexEntry {
                    path: PathBuf::from("/c/Смета_2026.xlsx"),
                    display_name: "Смета_2026.xlsx".into(),
                },
                IndexEntry {
                    path: PathBuf::from("/c/readme.md"),
                    display_name: "readme.md".into(),
                },
            ],
        });
        let _ = rx.recv();
        search.command(SearchCommand::Query {
            query: "смета".into(),
            limit: 16,
        });
        match rx.recv().expect("Ready после Query") {
            SearchEvent::Ready(hits) => {
                assert_eq!(hits.len(), 1);
                assert_eq!(hits[0].display_name, "Смета_2026.xlsx");
                assert_eq!(hits[0].path, PathBuf::from("/c/Смета_2026.xlsx"));
            }
            other => panic!("ожидался Ready, получен {other:?}"),
        }
        search.command(SearchCommand::Query {
            query: String::new(),
            limit: 16,
        });
        match rx.recv().expect("Ready после пустого Query") {
            SearchEvent::Ready(hits) => assert!(hits.is_empty()),
            other => panic!("ожидался Ready, получен {other:?}"),
        }
    }

    /// IndexFile/RemoveFile: upsert по ключу-пути и удаление.
    #[test]
    fn index_and_remove_file_upserts_by_path() {
        let (responder, _rx) = channel_responder();
        let search = MemSearch::new(responder);
        search.command(SearchCommand::IndexFile {
            path: PathBuf::from("/c/a.txt"),
            display_name: "a.txt".into(),
        });
        search.command(SearchCommand::IndexFile {
            path: PathBuf::from("/c/a.txt"),
            display_name: "a-переименованный.txt".into(),
        });
        assert_eq!(search.len(), 1);
        search.command(SearchCommand::RemoveFile {
            path: PathBuf::from("/c/a.txt"),
        });
        assert!(search.is_empty());
    }
}
