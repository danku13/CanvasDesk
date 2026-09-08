//! Поисковый индекс (T14, SPEC §5.2): FTS5-таблица `search_index` в
//! `~/.canvasdesk/cache.db`, worker-поток-владелец `rusqlite::Connection`.
//!
//! Команды — через канал (`SearchCommand`), ответы — через `SearchResponder`
//! (в приложении — обёртка над `EventLoopProxy`, шлёт `AppEvent::Search`).
//! Извлечение текста (txt/md ≤ 256 КиБ) выполняется в worker-потоке —
//! рендер-поток не блокируется. PDF/прочие форматы — только имя файла
//! (T11 отложен до после v1 — план T14 §8).
//!
//! Индекс пересоздаваемый (SPEC §5.3): ошибка открытия БД — деградация
//! (warn, пустые результаты), не сбой приложения. Upsert записи —
//! DELETE+INSERT в транзакции: FTS5 — rowid-таблица без UNIQUE по path,
//! `INSERT OR REPLACE` не вычищает старую строку (зонд T14-A: при авто-rowid
//! конфликта нет, вставляется дубликат). `journal_mode` не меняем —
//! SQLite-default, как в `cache.rs` (записи редкие и короткие); файл
//! `cache.db` делят с ThumbCache, конфликты писателей гасим `busy_timeout`
//! соединения (см. `BUSY_TIMEOUT`).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// FTS5-запись для переиндексации сцены (загрузка канваса).
pub struct IndexEntry {
    /// Путь файла ноды (ключ записи).
    pub path: PathBuf,
    /// Имя файла для отображения.
    pub display_name: String,
}

/// Команда worker-потоку индекса.
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
pub enum SearchEvent {
    /// Ответ на `SearchCommand::Query`: хиты в порядке bm25 (лучшие первыми).
    Ready(Vec<SearchHit>),
    /// Индексация `ReplaceAll` завершена: число актуальных записей (для лога).
    Indexed(usize),
}

/// Ответчик событий поиска (обёртка над EventLoopProxy в приложении).
pub type SearchResponder = Arc<dyn Fn(SearchEvent) + Send + Sync>;

/// Схема FTS5 (идемпотентно): `path` — UNINDEXED-ключ записи (не участвует
/// в полнотекстовом матчинге), `display_name`/`text` — индексируемые колонки.
/// UNICODE61-токенизация по умолчанию: кириллица, «смета_2026.xlsx» →
/// смета/2026/xlsx, регистронезависимо (зонд T14, worklog T13T14-COORD-SEED).
const SCHEMA: &str = "CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
    path UNINDEXED,
    display_name,
    text
);";

/// Ожидание блокировки `cache.db`: файл на двоих с ThumbCache (записи
/// thumb_cache из пула тамбнейлов, service.rs). `journal_mode` не трогаем —
/// SQLite-default, как в `cache.rs` (пишем редко и помалу); конфликт
/// коротких параллельных писателей гасим ожиданием, а не потерей записи.
const BUSY_TIMEOUT: Duration = Duration::from_millis(5_000);

/// Лимит извлекаемого текста: 256 КиБ (план T14 §3).
const EXTRACT_LIMIT: u64 = 256 * 1024;

/// Сервис поиска: владеет worker-потоком с `rusqlite::Connection`.
/// Отправка команд — неблокирующая; ошибся канал (поток умер) — молча
/// (паттерн watcher.rs T10).
pub struct SearchService {
    sender: mpsc::Sender<SearchCommand>,
}

impl SearchService {
    /// Запустить worker-поток с индексом в `cache_dir` (файл cache.db,
    /// схема FTS5 создаётся идемпотентно). Ошибка открытия БД — деградация:
    /// warn + поток не стартует, команды уходят в пустоту (send-ошибки
    /// игнорируются), ответы не приходят. `cache_dir` — тот же, что у
    /// ThumbCache (`default_cache_dir`).
    pub fn spawn(cache_dir: PathBuf, responder: SearchResponder) -> Self {
        let (sender, receiver) = mpsc::channel::<SearchCommand>();
        // Поток-владелец Connection. Провал создания потока — та же
        // деградация, что и провал БД: замыкание (с receiver и responder)
        // дропается, send-ошибки в `command` игнорируются (SPEC §5.3).
        if let Err(err) = std::thread::Builder::new()
            .name("search-index".to_string())
            .spawn(move || index_loop(cache_dir, receiver, responder))
        {
            tracing::warn!(%err, "worker-поток поиска не создан — индекс отключён");
        }
        Self { sender }
    }

    /// Отправить команду worker-потоку (неблокирующе).
    pub fn command(&self, cmd: SearchCommand) {
        // Канал в умерший/нестартовавший поток (деградация) — ошибка
        // отправки игнорируется: приложение живёт без поиска
        let _ = self.sender.send(cmd);
    }
}

/// Тело worker-потока: открыть БД, создать схему, раздавать команды.
/// Любая ошибка инициализации — warn и выход (деградация, SPEC §5.3):
/// receiver дропается, команды теряются молча, паник нет. Поток завершается
/// сам, когда все `SearchService` дропнуты (канал команд разорван).
fn index_loop(
    cache_dir: PathBuf,
    receiver: mpsc::Receiver<SearchCommand>,
    responder: SearchResponder,
) {
    // Каталог кэша (тот же, что у ThumbCache) — идемпотентно
    if let Err(err) = fs::create_dir_all(&cache_dir) {
        tracing::warn!(
            %err,
            dir = %cache_dir.display(),
            "каталог кэша недоступен — индекс поиска отключён"
        );
        return;
    }
    let mut conn = match rusqlite::Connection::open(cache_dir.join("cache.db")) {
        Ok(conn) => conn,
        Err(err) => {
            tracing::warn!(%err, "SQLite open: БД индекса не открыта — поиск отключён");
            return;
        }
    };
    // Сначала busy_timeout: с созданием схемы может столкнуться пишущий
    // ThumbCache (общий cache.db); ошибка установки не фатальна
    if let Err(err) = conn.busy_timeout(BUSY_TIMEOUT) {
        tracing::warn!(%err, "busy_timeout не установлен — возможны потери записей индекса");
    }
    if let Err(err) = conn.execute_batch(SCHEMA) {
        tracing::warn!(%err, "SQLite schema: FTS5 не создана — поиск отключён");
        return;
    }
    for cmd in receiver {
        handle_command(&mut conn, &responder, cmd);
    }
}

/// Диспетчер одной команды. Ошибки БД — warn и деградация (команда
/// теряется, поток живёт); `responder` — просто вызов (замыкание приложения
/// без Result, паник не ожидается — паттерн watcher.rs).
fn handle_command(
    conn: &mut rusqlite::Connection,
    responder: &SearchResponder,
    cmd: SearchCommand,
) {
    match cmd {
        SearchCommand::IndexFile { path, display_name } => {
            if let Err(err) = index_file(conn, &path, &display_name) {
                tracing::warn!(%err, path = %path.display(), "SQLite index: файл не проиндексирован");
            }
        }
        SearchCommand::RemoveFile { path } => {
            if let Err(err) = remove_file(conn, &path) {
                tracing::warn!(%err, path = %path.display(), "SQLite delete: запись не удалена из индекса");
            }
        }
        SearchCommand::ReplaceAll { entries } => match replace_all(conn, entries) {
            Ok(count) => responder(SearchEvent::Indexed(count)),
            Err(err) => tracing::warn!(%err, "SQLite replace: переиндексация не выполнена"),
        },
        SearchCommand::Query { query, limit } => {
            responder(SearchEvent::Ready(run_query(conn, &query, limit)));
        }
    }
}

/// Upsert одной записи: DELETE по path + INSERT в транзакции. FTS5 —
/// rowid-таблица без UNIQUE по path: `INSERT OR REPLACE` не даёт «одну
/// строку на путь» — при авто-rowid конфликта нет и вставляется дубликат
/// (зонд T14-A: rows=2 при distinct_path=1); транзакция держит атомарность
/// «удали + вставь» (между ними не видно промежуточного отсутствия).
fn index_file(
    conn: &mut rusqlite::Connection,
    path: &Path,
    display_name: &str,
) -> Result<(), rusqlite::Error> {
    // Текст читаем ДО транзакции: файловый I/O не должен держать блокировку
    // записи cache.db, которую делят с ThumbCache (см. BUSY_TIMEOUT)
    let text = extract_text(path).unwrap_or_default();
    let key = path_key(path);
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM search_index WHERE path = ?1",
        rusqlite::params![key],
    )?;
    tx.execute(
        "INSERT INTO search_index (path, display_name, text) VALUES (?1, ?2, ?3)",
        rusqlite::params![key, display_name, text],
    )?;
    tx.commit()
}

/// Удалить запись по path; 0 затронутых строк — норма (идемпотентно:
/// записи могло не быть).
fn remove_file(conn: &rusqlite::Connection, path: &Path) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM search_index WHERE path = ?1",
        rusqlite::params![path_key(path)],
    )?;
    Ok(())
}

/// Полная переиндексация: транзакция «DELETE всех + INSERT всех записей».
/// Стратегия полного пересбора вместо вычитания «чужих» путей выбрана
/// сознательно: объём набора = числу file-нод канваса (сотни строк), код
/// проще и не зависит от согласованности старого состояния индекса;
/// транзакция гарантирует атомарную замену. Ответ — число записей (для
/// лога приложения). Файловый I/O (извлечение текстов) вынесен ДО
/// транзакции, чтобы не держать блокировку записи на время чтений.
fn replace_all(
    conn: &mut rusqlite::Connection,
    entries: Vec<IndexEntry>,
) -> Result<usize, rusqlite::Error> {
    let mut rows: Vec<(String, &str, String)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let text = extract_text(&entry.path).unwrap_or_default();
        rows.push((path_key(&entry.path), entry.display_name.as_str(), text));
    }
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM search_index", [])?;
    {
        let mut stmt =
            tx.prepare("INSERT INTO search_index (path, display_name, text) VALUES (?1, ?2, ?3)")?;
        for (path, display_name, text) in &rows {
            stmt.execute(rusqlite::params![path, display_name, text])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Выполнить запрос: экранирование → `MATCH` + ранжирование bm25 + LIMIT.
/// Пустой после экранирования запрос — пустой ответ БЕЗ выполнения MATCH
/// (пустой/битый MATCH — потенциальная ошибка SQLite). Ошибка SQLite или
/// чтения строки (битый MATCH недостижим после экранирования, но обработан)
/// — warn и деградация: пустой/частичный список, не паника.
fn run_query(conn: &rusqlite::Connection, query: &str, limit: usize) -> Vec<SearchHit> {
    let match_str = escape_query(query);
    if match_str.is_empty() {
        return Vec::new();
    }
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = match conn.prepare(
        "SELECT path, display_name FROM search_index
         WHERE search_index MATCH ?1
         ORDER BY bm25(search_index)
         LIMIT ?2",
    ) {
        Ok(stmt) => stmt,
        Err(err) => {
            tracing::warn!(%err, "SQLite query: запрос не подготовлен — пустой результат");
            return Vec::new();
        }
    };
    let rows = match stmt.query_map(rusqlite::params![match_str, limit], |row| {
        Ok(SearchHit {
            path: PathBuf::from(row.get::<_, String>(0)?),
            display_name: row.get::<_, String>(1)?,
        })
    }) {
        Ok(rows) => rows,
        Err(err) => {
            tracing::warn!(%err, "SQLite query: MATCH не выполнен — пустой результат");
            return Vec::new();
        }
    };
    let mut hits = Vec::new();
    for row in rows {
        match row {
            Ok(hit) => hits.push(hit),
            Err(err) => tracing::warn!(%err, "SQLite query: строка индекса пропущена"),
        }
    }
    hits
}

/// Ключ записи — строка пути как есть (`to_string_lossy`), БЕЗ нормализации:
/// нормализация путей — на стороне модели (как в T10: индекс хранит путь
/// ноды в исходном виде, матчинг с нодами — `path_matches` в приложении).
fn path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Извлечь текст для индекса: `.txt`/`.md` — чтение ≤ 256 КиБ (UTF-8 lossy);
/// прочее — пустая строка (только имя; PDF — после T11/v1).
/// Недоступный файл — None (запись не обновляется текстом, только именем).
fn extract_text(path: &Path) -> Option<String> {
    // Расширение регистронезависимо (кириллических расширений не бывает)
    let is_text = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "txt" | "md"));
    if !is_text {
        // Прочие форматы — только имя: Some(""), вызывающая сторона уже
        // передала display_name (план T14 §8 — PDF-текст после T11/v1)
        return Some(String::new());
    }
    // Читаем только префикс ≤ 256 КиБ: File + take + read_to_end
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(EXTRACT_LIMIT)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Экранировать запрос пользователя в FTS5-MATCH: термы без пробелов
/// обёртываются кавычками + префикс (`"смет" *` — ловит склонения и хвосты
/// составных токенов), термы склеиваются пробелом (AND). Пустой результат —
/// пустой список термов (вызов MATCH пропускается). Внутренние кавычки
/// вырезаются. Проверено зондом (план T14 §3).
fn escape_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter_map(|term| {
            // Внутренние двойные кавычки вырезаются — единственный
            // спецсимвол внутри FTS5-фразы; пустые термы отбрасываются
            let cleaned = term.replace('"', "");
            if cleaned.is_empty() {
                None
            } else {
                Some(format!("\"{cleaned}\" *"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU32, Ordering};

    /// Ожидание события от worker-потока, чтобы тест не вис вечно (бриф T14-A).
    const RECV: Duration = Duration::from_secs(2);

    /// Временный каталог без внешних зависимостей (pid + атомарный счётчик,
    /// паттерн cache.rs): реальный ~/.canvasdesk не трогаем, параллельные
    /// тесты не конфликтуют.
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "canvasdesk-search-{}-{}-{tag}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("создать tempdir");
        dir
    }

    /// Responder-заглушка: события поиска — в mpsc-канал теста.
    fn test_responder() -> (SearchResponder, mpsc::Receiver<SearchEvent>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(move |event: SearchEvent| {
                let _ = tx.send(event);
            }) as SearchResponder,
            rx,
        )
    }

    /// Сервис на каталоге-кэше + канал событий.
    fn spawn_service(cache_dir: &Path) -> (SearchService, mpsc::Receiver<SearchEvent>) {
        let (responder, rx) = test_responder();
        (SearchService::spawn(cache_dir.to_path_buf(), responder), rx)
    }

    /// Дождаться события (таймаут — паника теста, не висим).
    fn wait_event(rx: &mpsc::Receiver<SearchEvent>) -> SearchEvent {
        rx.recv_timeout(RECV).expect("событие поиска не пришло")
    }

    /// Дождаться `Ready` с хитами.
    fn wait_ready(rx: &mpsc::Receiver<SearchEvent>) -> Vec<SearchHit> {
        match wait_event(rx) {
            SearchEvent::Ready(hits) => hits,
            SearchEvent::Indexed(count) => panic!("ожидался Ready, пришёл Indexed({count})"),
        }
    }

    /// Дождаться `Indexed` с числом записей.
    fn wait_indexed(rx: &mpsc::Receiver<SearchEvent>) -> usize {
        match wait_event(rx) {
            SearchEvent::Indexed(count) => count,
            SearchEvent::Ready(hits) => {
                panic!("ожидался Indexed, пришёл Ready({} хитов)", hits.len())
            }
        }
    }

    /// Отправить IndexFile.
    fn index(svc: &SearchService, path: PathBuf, display_name: &str) {
        svc.command(SearchCommand::IndexFile {
            path,
            display_name: display_name.to_string(),
        });
    }

    /// Отправить Query.
    fn query(svc: &SearchService, query: &str, limit: usize) {
        svc.command(SearchCommand::Query {
            query: query.to_string(),
            limit,
        });
    }

    // ---------- escape_query ----------

    /// Пример из плана: «смета 2026» → два префиксных терма (AND).
    #[test]
    fn escape_query_wraps_terms_with_prefix() {
        assert_eq!(escape_query("смета 2026"), "\"смета\" * \"2026\" *");
        // Табы/переносы — тоже разделители
        assert_eq!(escape_query("смета\t2026"), "\"смета\" * \"2026\" *");
    }

    /// Пустой ввод, одни пробелы, термы из одних кавычек → пустая строка
    /// (MATCH пропускается).
    #[test]
    fn escape_query_blank_is_empty() {
        assert_eq!(escape_query(""), "");
        assert_eq!(escape_query("   "), "");
        assert_eq!(escape_query(" \t \n "), "");
        assert_eq!(escape_query("\"\""), "");
        assert_eq!(escape_query(" \" "), "");
    }

    /// Внутренние кавычки вырезаются; скобки и прочий мусор остаются
    /// ВНУТРИ кавычек — для FTS5 это литеральная фраза (безопасно; зонд:
    /// пустые фразы вроде "(" — не ошибка, просто совпадений нет).
    #[test]
    fn escape_query_strips_quotes_keeps_junk() {
        assert_eq!(escape_query("\"смета\""), "\"смета\" *");
        assert_eq!(escape_query("(\"OR"), "\"(OR\" *");
    }

    // ---------- extract_text ----------

    /// txt/md в любом регистре читаются (кириллица); прочие расширения —
    /// пустой текст (только имя); несуществующий файл — None.
    #[test]
    fn extract_text_txt_md_cyrillic() {
        let dir = temp_dir("extract");

        let md = dir.join("заметка.md");
        fs::write(&md, "смета на 2026 год").expect("запись .md");
        assert_eq!(extract_text(&md).as_deref(), Some("смета на 2026 год"));

        let txt = dir.join("REPORT.TXT");
        fs::write(&txt, b"plain ascii").expect("запись .TXT");
        assert_eq!(extract_text(&txt).as_deref(), Some("plain ascii"));

        let upper = dir.join("PLAN.MD");
        fs::write(&upper, "ЗАМЕТКА").expect("запись .MD");
        assert_eq!(extract_text(&upper).as_deref(), Some("ЗАМЕТКА"));

        // Прочие форматы — только имя (PDF после T11/v1, план T14 §8)
        let xlsx = dir.join("табель.xlsx");
        fs::write(&xlsx, b"PK\x03\x04not-really").expect("запись .xlsx");
        assert_eq!(extract_text(&xlsx).as_deref(), Some(""));

        assert_eq!(extract_text(&dir.join("нет_файла.md")), None);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Файл 300 КиБ усекается до 256 КиБ (читается только префикс).
    #[test]
    fn extract_text_truncates_to_256k() {
        let dir = temp_dir("trunc");
        let file = dir.join("big.md");
        fs::write(&file, "a".repeat(300 * 1024)).expect("запись 300 KiB");
        let text = extract_text(&file).expect("файл доступен");
        assert_eq!(text.len(), 256 * 1024);
        assert!(text.chars().all(|c| c == 'a'));
        let _ = fs::remove_dir_all(&dir);
    }

    // ---------- сервис: индексация и запросы (worker-поток, mpsc) ----------

    /// Основной сценарий: реальный .md с кириллицей + запись без файла →
    /// префикс «смет» находит обе (смета/смету в тексте; смета_… в имени
    /// составного токена), чужая запись не находится.
    #[test]
    fn index_and_query_prefix_cyrillic() {
        let dir = temp_dir("main");

        let md = dir.join("бюджет.md");
        fs::write(&md, "смета на 2026 год, смету перепроверить").expect("запись .md");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, md.clone(), "бюджет.md");
        // Запись без реального файла (битая ссылка): индексируется только имя
        index(&svc, dir.join("смета_декабрь.xlsx"), "смета_декабрь.xlsx");
        // Контрольная запись без терма
        index(&svc, dir.join("отчёт.doc"), "отчёт.doc");

        query(&svc, "смет", 10);
        let mut hits = wait_ready(&rx);
        assert_eq!(
            hits.len(),
            2,
            "префикс смет: смета/смету в тексте + смета_… в имени"
        );
        hits.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(hits[0].path, md);
        assert_eq!(hits[0].display_name, "бюджет.md");
        assert_eq!(hits[1].path, dir.join("смета_декабрь.xlsx"));
        assert_eq!(hits[1].display_name, "смета_декабрь.xlsx");

        let _ = fs::remove_dir_all(&dir);
    }

    /// Склонения ловятся префиксом; точный терм «смету» — только запись с
    /// токеном «смету» (имя смета_декабрь ≠ смету без префикса).
    #[test]
    fn query_exact_term_vs_inflection() {
        let dir = temp_dir("exact");
        let md = dir.join("бюджет.md");
        fs::write(&md, "смета и смету сверить").expect("запись .md");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, md.clone(), "бюджет.md");
        index(&svc, dir.join("смета_декабрь.xlsx"), "смета_декабрь.xlsx");

        // Точный терм без префикса: только смету в тексте .md
        query(&svc, "смету", 10);
        let hits = wait_ready(&rx);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, md);

        // Префикс «смет» ловит оба склонения и составное имя
        query(&svc, "смет", 10);
        assert_eq!(wait_ready(&rx).len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Несколько термов — AND-семантика: «смета 2026» находит только записи
    /// с обоими токенами.
    #[test]
    fn query_multiple_terms_and() {
        let dir = temp_dir("and");
        let md = dir.join("док.md");
        fs::write(&md, "смета на 2026 год").expect("запись .md");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, md.clone(), "док.md");
        // смета есть, 2026 нет
        index(&svc, dir.join("смета_план.xlsx"), "смета_план.xlsx");
        // 2026 есть, сметы нет
        index(&svc, dir.join("итоги_2026.pdf"), "итоги_2026.pdf");

        query(&svc, "смета 2026", 10);
        let hits = wait_ready(&rx);
        assert_eq!(hits.len(), 1, "AND: оба токена только в .md");
        assert_eq!(hits[0].path, md);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Пустой/пробельный запрос — Ready с пустым списком БЕЗ MATCH (не
    /// ошибка, не паника).
    #[test]
    fn query_blank_is_empty_ready() {
        let dir = temp_dir("blank");
        let md = dir.join("док.md");
        fs::write(&md, "смета").expect("запись .md");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, md, "док.md");

        query(&svc, "", 10);
        assert!(wait_ready(&rx).is_empty());
        query(&svc, "   ", 10);
        assert!(wait_ready(&rx).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Мусорные термы не паникуют и не рушат запрос: терм из скобок —
    /// пустая фраза (зонд: не ошибка, совпадений нет), в смеси с нормальным
    /// термом остальные термы продолжают работать.
    #[test]
    fn query_garbage_terms_do_not_break() {
        let dir = temp_dir("garbage");
        let md = dir.join("док.md");
        fs::write(&md, "смета").expect("запись .md");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, md, "док.md");

        // Запрос из одного мусорного терма: ноль строк, без ошибки
        query(&svc, "( ", 10);
        assert!(wait_ready(&rx).is_empty());

        // Мусорный терм в смеси: полезный терм работает
        query(&svc, "смета (", 10);
        assert_eq!(wait_ready(&rx).len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Лимит запроса: LIMIT ?2 = 1 — одна строка из двух совпадений.
    #[test]
    fn query_limit_one() {
        let dir = temp_dir("limit");
        let a = dir.join("смета_a.md");
        fs::write(&a, "смета").expect("запись a");
        let b = dir.join("смета_b.md");
        fs::write(&b, "смета").expect("запись b");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, a, "смета_a.md");
        index(&svc, b, "смета_b.md");

        query(&svc, "смет", 10);
        assert_eq!(wait_ready(&rx).len(), 2);

        query(&svc, "смет", 1);
        assert_eq!(wait_ready(&rx).len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    /// RemoveFile удаляет запись из индекса (проверка повторным запросом).
    #[test]
    fn remove_file_deletes_entry() {
        let dir = temp_dir("remove");
        let a = dir.join("смета_a.md");
        fs::write(&a, "смета").expect("запись a");
        let b = dir.join("смета_b.md");
        fs::write(&b, "смета").expect("запись b");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, a.clone(), "смета_a.md");
        index(&svc, b.clone(), "смета_b.md");

        query(&svc, "смет", 10);
        assert_eq!(wait_ready(&rx).len(), 2);

        svc.command(SearchCommand::RemoveFile { path: a });
        query(&svc, "смет", 10);
        let hits = wait_ready(&rx);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, b);

        let _ = fs::remove_dir_all(&dir);
    }

    /// ReplaceAll пересобирает индекс с нуля: старые записи (и имя, и текст)
    /// пропадают, новые появляются, ответ Indexed = числу записей набора.
    #[test]
    fn replace_all_rebuilds_index() {
        let dir = temp_dir("replaceall");
        let old = dir.join("старый.md");
        fs::write(&old, "смета").expect("запись старого");

        let (svc, rx) = spawn_service(&dir);
        index(&svc, old, "старый.md");
        query(&svc, "смет", 10);
        assert_eq!(wait_ready(&rx).len(), 1);

        svc.command(SearchCommand::ReplaceAll {
            entries: vec![
                IndexEntry {
                    path: dir.join("новая_смета.xlsx"),
                    display_name: "новая_смета.xlsx".to_string(),
                },
                IndexEntry {
                    path: dir.join("отчёт_v2.doc"),
                    display_name: "отчёт_v2.doc".to_string(),
                },
            ],
        });
        assert_eq!(wait_indexed(&rx), 2);

        // Старая запись исчезла целиком: её текст больше не находится…
        query(&svc, "смет", 10);
        let hits = wait_ready(&rx);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].display_name, "новая_смета.xlsx");

        // …и по имени тоже
        query(&svc, "стар", 10);
        assert!(wait_ready(&rx).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    /// Повторный IndexFile — upsert: текст заменяется, дублей записей не
    /// остаётся (проверка DELETE+INSERT, зонд: INSERT OR REPLACE давал бы
    /// дубликат).
    #[test]
    fn index_file_upsert_replaces_text() {
        let dir = temp_dir("upsert");
        let doc = dir.join("док.md");

        let (svc, rx) = spawn_service(&dir);
        fs::write(&doc, "смета бюджет").expect("запись 1");
        index(&svc, doc.clone(), "док.md");
        query(&svc, "смет", 10);
        assert_eq!(wait_ready(&rx).len(), 1);

        // Содержимое изменилось: повторная индексация заменяет текст
        fs::write(&doc, "ревизия отчёта").expect("запись 2");
        index(&svc, doc.clone(), "док.md");

        query(&svc, "смет", 10);
        assert!(
            wait_ready(&rx).is_empty(),
            "старый текст заменён, а не задублирован"
        );

        query(&svc, "реви", 10);
        let hits = wait_ready(&rx);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, doc);

        // Имя ищется ровно одной строкой (нет дублей от повторного upsert)
        query(&svc, "док", 10);
        assert_eq!(wait_ready(&rx).len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    /// Деградация (SPEC §5.3): cache_dir — существующий файл, create_dir_all
    /// падает → поток не стартует: команды теряются молча, событий нет,
    /// паники нет (таймаут или обрыв канала — оба варианта без результата).
    #[test]
    fn degraded_cache_dir_swallows_commands() {
        let dir = temp_dir("degraded");
        let blocker = dir.join("blocker.db");
        fs::write(&blocker, b"not a dir").expect("запись блокирующего файла");

        let (svc, rx) = spawn_service(&blocker);
        query(&svc, "смет", 10);
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
