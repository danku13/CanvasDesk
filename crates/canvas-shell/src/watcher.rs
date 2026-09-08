//! WatchService — файловый вотчер на `notify` (T10, SPEC §7.5, план §3).
//!
//! Один `RecommendedWatcher` держит NonRecursive-наблюдения за директориями
//! (watch/unwatch по diff желаемого набора — `sync_dirs`). Сырые события notify
//! уходят в поток-агрегатор: окно debounce 300 мс от первого события (шквал
//! Modify склеивается в один батч), дедуп, склейка rename-пар
//! (`RenameMode::Both` напрямую; `From`+`To` — так отдают ОБА бэкенда: inotify
//! IN_MOVED_FROM/TO и ReadDirectoryChangesW RENAMED_OLD/NEW_NAME; фолбэк
//! `Remove`+`Create` по порядку «remove раньше create»), затем батч
//! `Vec<FileEvent>` наружу через sender — `EventLoopProxy` приложения
//! (паттерн ThumbService: рендер-поток не блокируется, AGENTS.md).
//!
//! Ошибки вотчинга (сетевые диски, умерший хендл) — warn и деградация: события
//! не приходят, приложение продолжает работу (RECIPES R14). Поток-агрегатор
//! живёт до конца процесса (как worker'ы ThumbService). Кроссплатформенно:
//! inotify на Linux, ReadDirectoryChangesW на Windows — интеграционные тесты
//! гоняются на обеих ОС. `collapse` — чистая функция (notify::Event →
//! FileEvent), тестируется синтетикой без файловой системы (AGENTS.md).

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use canvas_core::{normalize_path, FileEvent};
use notify::event::{ModifyKind, RenameMode};
use notify::{EventKind, RecursiveMode, Watcher};

/// Окно debounce агрегатора: батч собирается не дольше (SPEC §7.5).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Получатель батчей событий: в приложении — замыкание с `EventLoopProxy`,
/// шлёт `AppEvent::FileEvents` (как drag_sender в T9).
pub type FileEventSender = Arc<dyn Fn(Vec<FileEvent>) + Send + Sync>;

/// Файловый вотчер: NonRecursive-наблюдения за набором директорий.
pub struct WatchService {
    /// None — вотчер не инициализировался (warn + деградация, sync_dirs — no-op).
    watcher: Option<notify::RecommendedWatcher>,
    /// Активные наблюдения (нормализованные пути) — diff-основа sync_dirs.
    watched: HashSet<PathBuf>,
}

impl WatchService {
    /// Запустить вотчер с агрегатором: события notify → mpsc → окно 300 мс →
    /// батч `FileEvent` в `sender`.
    pub fn new(sender: FileEventSender) -> Self {
        let (tx, rx) = mpsc::channel::<notify::Event>();
        // Callback notify выполняется в его внутреннем потоке: ошибки — warn
        // (деградация, RECIPES R14), события — в канал агрегатора. send-ошибка
        // означает, что приёмник умер, — молча выбрасываем (агрегатор уже вышел).
        let handler = move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                let _ = tx.send(event);
            }
            Err(err) => tracing::warn!(%err, "событие файлового вотчера потеряно"),
        };
        let watcher = match notify::recommended_watcher(handler) {
            Ok(watcher) => Some(watcher),
            Err(err) => {
                // Практически недостижимо (инициализация inotify/RDCW), но при
                // провале сервис просто не даёт событий — приложение живёт.
                tracing::warn!(%err, "файловый вотчер недоступен — слежение отключено");
                None
            }
        };
        // Агрегатор владеет sender'ом и приёмником канала, живёт до конца
        // процесса (паттерн ThumbService). tx остаётся у вотчера: пока
        // WatchService жив — канал жив; после drop — Disconnected и выход.
        std::thread::spawn(move || aggregator_loop(rx, sender));
        Self {
            watcher,
            watched: HashSet::new(),
        }
    }

    /// Синхронизировать набор наблюдаемых директорий с желаемым: добавить
    /// недостающие (`watch`, NonRecursive), снять отсутствующие (`unwatch`).
    /// Повторный вызов с тем же набором — no-op. Ошибки — warn, деградация:
    /// не установленное наблюдение не попадает в `watched` (следующий sync
    /// попробует снова), снятое покидает учёт даже при ошибке unwatch —
    /// пере-добавится, если директория снова понадобится.
    pub fn sync_dirs(&mut self, dirs: &[PathBuf]) {
        // Деградация: вотчера нет — нечего синхронизировать
        let Some(watcher) = self.watcher.as_mut() else {
            return;
        };
        // Желаемый набор: нормализация даже для уже нормализованных путей от
        // canvas_core::watched_dirs — защита от verbatim-префиксов и чужих
        // разделителей; уникальность — HashSet.
        let desired: HashSet<PathBuf> = dirs.iter().map(|dir| normalize_path(dir)).collect();
        // Diff с активными наблюдениями; одинаковый набор → списки пусты →
        // watch/unwatch не вызываются вовсе (не дёргаем бэкенд).
        let to_remove: Vec<PathBuf> = self
            .watched
            .iter()
            .filter(|dir| !desired.contains(*dir))
            .cloned()
            .collect();
        let to_add: Vec<PathBuf> = desired
            .iter()
            .filter(|dir| !self.watched.contains(*dir))
            .cloned()
            .collect();

        for dir in &to_remove {
            if let Err(err) = watcher.unwatch(dir) {
                tracing::warn!(%err, dir = %dir.display(), "снятие наблюдения не удалось");
            }
        }
        for dir in &to_add {
            if let Err(err) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                // Не вносим в watched — следующий sync_dirs попытается снова
                tracing::warn!(%err, dir = %dir.display(), "установка наблюдения не удалась");
                continue;
            }
            self.watched.insert(dir.clone());
        }
        // Снятые покидают учёт даже при ошибке unwatch (см. док-комментарий)
        self.watched.retain(|dir| desired.contains(dir));
    }
}

/// Цикл агрегатора: блокирующее ожидание первого события окна (поток-демон,
/// не рендер-поток — AGENTS.md), добор остатка окна `recv_timeout`'ом, затем
/// collapse и доставка батча. Disconnected (вотчер умер) — дослать собранное
/// и выйти.
fn aggregator_loop(rx: mpsc::Receiver<notify::Event>, sender: FileEventSender) {
    loop {
        let first = match rx.recv() {
            Ok(event) => event,
            // Все отправители умерли (WatchService dropped) — выходим без батча
            Err(mpsc::RecvError) => return,
        };
        let mut raws = vec![first];
        // Окно отсчитывается от ПЕРВОГО события: deadline фиксирован, а не
        // скользящий — иначе непрерывный шквал растягивал бы батч бесконечно.
        let deadline = Instant::now() + DEBOUNCE;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_default();
            if remaining.is_zero() {
                break; // окно закрыто — батч готов
            }
            match rx.recv_timeout(remaining) {
                Ok(event) => raws.push(event),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    deliver(&sender, collapse(raws));
                    return;
                }
            }
        }
        deliver(&sender, collapse(raws));
    }
}

/// Доставить батч (пустой collapse-результат не будит приложение).
fn deliver(sender: &FileEventSender, batch: Vec<FileEvent>) {
    if !batch.is_empty() {
        sender(batch);
    }
}

/// Промежуточный кандидат свёртки — значение одного raw-события notify
/// (Create/Modify/Remove/From/To/Rename) до склеек и дедупа. Приватный.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Cand {
    Create(PathBuf),
    Modify(PathBuf),
    Remove(PathBuf),
    /// `Modify(Name(From))`: ждёт пару `To` — иначе станет Remove.
    From(PathBuf),
    /// `Modify(Name(To))`: ждёт пару `From` — иначе станет Create.
    To(PathBuf),
    Rename(PathBuf, PathBuf),
}

/// Свёртка raw-событий notify одного debounce-окна в `Vec<FileEvent>`
/// (чистая функция — тестируется синтетикой без файловой системы).
///
/// Шаги: (1) классификация по `EventKind` → кандидаты в порядке прихода;
/// (2) склейка `From`+`To` → Rename — каждый `From` берёт первый
/// неиспользованный `To` ПОСЛЕ него; одинокий `From` → Remove (файл ушёл в
/// неизвестном направлении), одинокий `To` → Create (файл пришёл извне);
/// (3) фолбэк междиректорных move, когда notify пары не дал: `Remove(a)` +
/// первый ПОСЛЕДУЮЩИЙ неиспользованный `Create(b)` → Rename(a,b) — Create ДО
/// Remove («файл родился и умер») никогда не склеивается; (4) после
/// Rename(a,b) подавляются Create(b) и Remove(a) этого окна — путь уже учтён
/// переименованием; (5) дедуп одинаковых (kind+path) кандидатов, финальный
/// порядок — порядок первых вхождений. Сравнение путей точное: события одного
/// окна приходят из одного watcher-корня (одинаковый вид и регистр путей).
fn collapse(raws: Vec<notify::Event>) -> Vec<FileEvent> {
    let cands = candidates(raws);

    // (2) From+To → Rename
    let mut paired: Vec<Cand> = Vec::with_capacity(cands.len());
    let mut to_consumed = vec![false; cands.len()];
    for (i, cand) in cands.iter().enumerate() {
        if to_consumed[i] {
            continue;
        }
        match cand {
            Cand::From(from) => {
                // первый неиспользованный To ПОСЛЕ этого From
                let to = (i + 1..cands.len())
                    .find(|&j| !to_consumed[j] && matches!(cands[j], Cand::To(_)));
                match to {
                    Some(j) => {
                        to_consumed[j] = true;
                        if let Cand::To(path) = &cands[j] {
                            paired.push(Cand::Rename(from.clone(), path.clone()));
                        }
                    }
                    None => paired.push(Cand::Remove(from.clone())),
                }
            }
            // To, не подобранный ни одним более ранним From
            Cand::To(to) => paired.push(Cand::Create(to.clone())),
            other => paired.push(other.clone()),
        }
    }

    // (3) Remove+Create → Rename (фолбэк междиректорных move)
    let mut merged: Vec<Cand> = Vec::with_capacity(paired.len());
    let mut create_consumed = vec![false; paired.len()];
    for (i, cand) in paired.iter().enumerate() {
        if create_consumed[i] {
            continue;
        }
        match cand {
            Cand::Remove(from) => {
                // первый неиспользованный Create ПОСЛЕ этого Remove; Create,
                // встретившийся раньше, остаётся самостоятельным
                let create = (i + 1..paired.len())
                    .find(|&j| !create_consumed[j] && matches!(paired[j], Cand::Create(_)));
                match create {
                    Some(j) => {
                        create_consumed[j] = true;
                        if let Cand::Create(path) = &paired[j] {
                            merged.push(Cand::Rename(from.clone(), path.clone()));
                        }
                    }
                    None => merged.push(Cand::Remove(from.clone())),
                }
            }
            other => merged.push(other.clone()),
        }
    }

    // (4) Подавление после Rename(a, b): Create(b) и Remove(a) уже учтены
    let renames: Vec<(PathBuf, PathBuf)> = merged
        .iter()
        .filter_map(|cand| match cand {
            Cand::Rename(from, to) => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect();
    if !renames.is_empty() {
        merged.retain(|cand| match cand {
            Cand::Create(path) => !renames.iter().any(|(_, to)| path == to),
            Cand::Remove(path) => !renames.iter().any(|(from, _)| path == from),
            _ => true,
        });
    }

    // (5) Дедуп (kind+path), порядок первых вхождений
    let mut seen: HashSet<Cand> = HashSet::with_capacity(merged.len());
    let mut unique: Vec<Cand> = Vec::with_capacity(merged.len());
    for cand in merged {
        if seen.insert(cand.clone()) {
            unique.push(cand);
        }
    }
    // From/To разрешены шагом (2) — здесь их быть не может
    unique
        .into_iter()
        .filter_map(|cand| match cand {
            Cand::Create(path) => Some(FileEvent::Create(path)),
            Cand::Modify(path) => Some(FileEvent::Modify(path)),
            Cand::Remove(path) => Some(FileEvent::Remove(path)),
            Cand::Rename(from, to) => Some(FileEvent::Rename(from, to)),
            Cand::From(_) | Cand::To(_) => None,
        })
        .collect()
}

/// Шаг (1) collapse: один raw-event → 0..1 кандидат по `EventKind`.
/// События без путей и неоднозначные `Name(Any|Other)` игнорируются.
fn candidates(raws: Vec<notify::Event>) -> Vec<Cand> {
    let mut cands = Vec::with_capacity(raws.len());
    for event in raws {
        let paths = event.paths;
        let kind = event.kind;
        match kind {
            // Create: File/Folder/Any/Other — путь появился
            EventKind::Create(_) => {
                if let Some(path) = paths.first() {
                    cands.push(Cand::Create(path.clone()));
                }
            }
            // Remove: File/Folder/Any/Other — путь исчез
            EventKind::Remove(_) => {
                if let Some(path) = paths.first() {
                    cands.push(Cand::Remove(path.clone()));
                }
            }
            // Rename одним событием: (from, to) по порядку в paths
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                if paths.len() >= 2 {
                    cands.push(Cand::Rename(paths[0].clone(), paths[1].clone()));
                } // меньше двух путей — пара неполна, игнор
            }
            // Разъятые половины rename (inotify и RDCW): склеит шаг (2)
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                if let Some(path) = paths.first() {
                    cands.push(Cand::From(path.clone()));
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                if let Some(path) = paths.first() {
                    cands.push(Cand::To(path.clone()));
                }
            }
            // Data/Metadata/Any/Other — содержимое или метаданные изменились
            EventKind::Modify(
                ModifyKind::Data(_) | ModifyKind::Metadata(_) | ModifyKind::Any | ModifyKind::Other,
            ) => {
                if let Some(path) = paths.first() {
                    cands.push(Cand::Modify(path.clone()));
                }
            }
            // Modify(Name(Any | Other)) — неоднозначные половинки rename;
            // Access(..) и прочее — не мутирующие/нераспознанные: игнор
            _ => {}
        }
    }
    cands
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, RemoveKind};

    /// Ожидание первого батча после действия (CI под нагрузкой, план §5).
    const FIRST: Duration = Duration::from_secs(5);
    /// Тишина «ничего не пришло»: окно 300 мс + запас.
    const QUIET: Duration = Duration::from_millis(700);
    /// Пауза опроса канала в ожидании.
    const POLL: Duration = Duration::from_millis(20);

    // ---------- Юнит-тесты collapse (синтетика, без файловой системы) ----------

    fn pb(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    /// Синтетическое событие notify: `Event::new(kind)` + пути через builder
    /// (реальный конструктор notify 8.2, см. notify-types/src/event.rs).
    fn raw(kind: EventKind, paths: &[&str]) -> notify::Event {
        let mut event = notify::Event::new(kind);
        for path in paths {
            event = event.add_path(PathBuf::from(*path));
        }
        event
    }

    /// 1. RenameMode::Both с парой путей → Rename(a, b).
    #[test]
    fn collapse_both_rename_two_paths() {
        let out = collapse(vec![raw(
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            &["/t/a", "/t/b"],
        )]);
        assert_eq!(out, vec![FileEvent::Rename(pb("/t/a"), pb("/t/b"))]);
    }

    /// 2. From(a) + To(b) → Rename(a, b) — так отдают inotify и RDCW.
    #[test]
    fn collapse_from_to_glue() {
        let out = collapse(vec![
            raw(
                EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                &["/t/a"],
            ),
            raw(
                EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                &["/t/b"],
            ),
        ]);
        assert_eq!(out, vec![FileEvent::Rename(pb("/t/a"), pb("/t/b"))]);
    }

    /// 3. From без To → Remove; To без From → Create.
    #[test]
    fn collapse_orphan_from_and_to() {
        let out = collapse(vec![raw(
            EventKind::Modify(ModifyKind::Name(RenameMode::From)),
            &["/t/a"],
        )]);
        assert_eq!(out, vec![FileEvent::Remove(pb("/t/a"))]);

        let out = collapse(vec![raw(
            EventKind::Modify(ModifyKind::Name(RenameMode::To)),
            &["/t/b"],
        )]);
        assert_eq!(out, vec![FileEvent::Create(pb("/t/b"))]);
    }

    /// 4. Remove(a) + Create(b) в этом порядке → Rename(a, b) (фолбэк).
    #[test]
    fn collapse_remove_create_glue() {
        let out = collapse(vec![
            raw(EventKind::Remove(RemoveKind::File), &["/t/a"]),
            raw(EventKind::Create(CreateKind::File), &["/t/b"]),
        ]);
        assert_eq!(out, vec![FileEvent::Rename(pb("/t/a"), pb("/t/b"))]);
    }

    /// 5. Create раньше Remove («файл родился и умер») — без склейки.
    #[test]
    fn collapse_create_before_remove_not_glued() {
        let out = collapse(vec![
            raw(EventKind::Create(CreateKind::File), &["/t/p"]),
            raw(EventKind::Remove(RemoveKind::File), &["/t/p"]),
        ]);
        assert_eq!(
            out,
            vec![FileEvent::Create(pb("/t/p")), FileEvent::Remove(pb("/t/p"))]
        );
    }

    /// 6. Rename(a, b) + Create(b) — дубликат целевого пути подавлен.
    #[test]
    fn collapse_rename_suppresses_followup() {
        let out = collapse(vec![
            raw(
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                &["/t/a", "/t/b"],
            ),
            raw(EventKind::Create(CreateKind::File), &["/t/b"]),
        ]);
        assert_eq!(out, vec![FileEvent::Rename(pb("/t/a"), pb("/t/b"))]);
    }

    /// 7. Шквал Modify(Data) одного пути → один Modify (дедуп).
    #[test]
    fn collapse_modify_burst_dedup() {
        let out = collapse(vec![
            raw(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/t/f"],
            ),
            raw(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/t/f"],
            ),
            raw(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/t/f"],
            ),
        ]);
        assert_eq!(out, vec![FileEvent::Modify(pb("/t/f"))]);
    }

    /// 8. Modify(Metadata) + Modify(Data) одного пути → один Modify.
    #[test]
    fn collapse_modify_metadata_and_data_dedup() {
        let out = collapse(vec![
            raw(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                &["/t/f"],
            ),
            raw(
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                &["/t/f"],
            ),
        ]);
        assert_eq!(out, vec![FileEvent::Modify(pb("/t/f"))]);
    }

    /// 9. Access-события и события без путей игнорируются.
    #[test]
    fn collapse_ignores_access_and_pathless() {
        let out = collapse(vec![
            raw(EventKind::Access(AccessKind::Any), &["/t/f"]),
            raw(EventKind::Access(AccessKind::Read), &["/t/f"]),
            raw(EventKind::Create(CreateKind::File), &[]),
            raw(EventKind::Remove(RemoveKind::Any), &[]),
            raw(EventKind::Modify(ModifyKind::Data(DataChange::Any)), &[]),
        ]);
        assert!(out.is_empty());
    }

    /// 10. Create(File) + Create(Folder) одного пути → один Create.
    #[test]
    fn collapse_create_file_folder_dedup() {
        let out = collapse(vec![
            raw(EventKind::Create(CreateKind::File), &["/t/d"]),
            raw(EventKind::Create(CreateKind::Folder), &["/t/d"]),
        ]);
        assert_eq!(out, vec![FileEvent::Create(pb("/t/d"))]);
    }

    /// 11. Пустой вход → пустой выход.
    #[test]
    fn collapse_empty_input() {
        assert!(collapse(Vec::new()).is_empty());
    }

    // ---------- Интеграционные тесты: реальный notify + tempdir ----------

    /// Уникальный temp-каталог теста (паттерн thumbs.rs).
    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("canvasdesk-watch-{}-{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("создать tempdir");
        dir
    }

    /// Sender для тестов: батчи в mpsc-канал теста.
    fn test_sender() -> (FileEventSender, mpsc::Receiver<Vec<FileEvent>>) {
        let (tx, rx) = mpsc::channel();
        (
            Arc::new(move |batch: Vec<FileEvent>| {
                let _ = tx.send(batch);
            }) as FileEventSender,
            rx,
        )
    }

    /// Нормализованная копия батча для сравнения: RDCW строит пути от корня
    /// вотча (наш нормализованный, с `/`), а std::env::temp_dir — с `\`;
    /// нормализация уравнивает формы (в приложении это делает path_matches).
    fn normalized(batch: Vec<FileEvent>) -> Vec<FileEvent> {
        batch
            .into_iter()
            .map(|event| match event {
                FileEvent::Create(path) => FileEvent::Create(normalize_path(&path)),
                FileEvent::Modify(path) => FileEvent::Modify(normalize_path(&path)),
                FileEvent::Remove(path) => FileEvent::Remove(normalize_path(&path)),
                FileEvent::Rename(from, to) => {
                    FileEvent::Rename(normalize_path(&from), normalize_path(&to))
                }
            })
            .collect()
    }

    fn norm(path: &Path) -> PathBuf {
        normalize_path(path)
    }

    /// Дождаться первого батча (до `timeout`), опрос каждые POLL.
    fn wait_batch(
        rx: &mpsc::Receiver<Vec<FileEvent>>,
        timeout: Duration,
    ) -> Option<Vec<FileEvent>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Ok(batch) = rx.try_recv() {
                return Some(batch);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_default();
            if remaining.is_zero() {
                return None;
            }
            std::thread::sleep(remaining.min(POLL));
        }
    }

    /// Убедиться, что в течение `quiet` новых батчей нет.
    fn assert_quiet(rx: &mpsc::Receiver<Vec<FileEvent>>, quiet: Duration) {
        let deadline = Instant::now() + quiet;
        loop {
            if let Ok(batch) = rx.try_recv() {
                panic!("неожидался батч: {batch:?}");
            }
            if deadline.checked_duration_since(Instant::now()).is_none() {
                return;
            }
            std::thread::sleep(POLL);
        }
    }

    /// Собрать все батчи в течение `quiet` (для мягких ассертов).
    fn collect_quiet(rx: &mpsc::Receiver<Vec<FileEvent>>, quiet: Duration) -> Vec<Vec<FileEvent>> {
        let mut batches = Vec::new();
        let deadline = Instant::now() + quiet;
        loop {
            if let Ok(batch) = rx.try_recv() {
                batches.push(batch);
            }
            if deadline.checked_duration_since(Instant::now()).is_none() {
                return batches;
            }
            std::thread::sleep(POLL);
        }
    }

    /// Тест 12: создание файла в наблюдаемой директории → Create.
    #[test]
    fn watcher_create_file() {
        let dir = temp_dir("create");
        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        let file = dir.join("a.txt");
        std::fs::write(&file, b"data").expect("запись файла");

        let batch = normalized(wait_batch(&rx, FIRST).expect("батч создания не пришёл"));
        assert!(batch.contains(&FileEvent::Create(norm(&file))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 13: изменение существующего файла (создан до sync) → один
    /// Modify (шквал write-событий склеен дедупом).
    #[test]
    fn watcher_modify_file() {
        let dir = temp_dir("modify");
        let file = dir.join("m.txt");
        std::fs::write(&file, b"v1").expect("создание до sync");

        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        std::fs::write(&file, b"v2").expect("повторная запись");

        let batch = normalized(wait_batch(&rx, FIRST).expect("батч модификации не пришёл"));
        assert!(batch.contains(&FileEvent::Modify(norm(&file))));
        assert_eq!(
            batch
                .iter()
                .filter(|e| matches!(e, FileEvent::Modify(_)))
                .count(),
            1
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 14: переименование файла → Rename(f, g) с сохранением порядка
    /// from→to (inotify и RDCW отдают парой From+To; collapse склеивает).
    #[test]
    fn watcher_rename_file() {
        let dir = temp_dir("rename");
        let from = dir.join("f.txt");
        let to = dir.join("g.txt");
        std::fs::write(&from, b"v1").expect("создание до sync");

        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        std::fs::rename(&from, &to).expect("переименование");

        let batch = normalized(wait_batch(&rx, FIRST).expect("батч переименования не пришёл"));
        assert!(batch.contains(&FileEvent::Rename(norm(&from), norm(&to))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 15: удаление файла (создан до sync) → Remove.
    #[test]
    fn watcher_remove_file() {
        let dir = temp_dir("remove");
        let file = dir.join("r.txt");
        std::fs::write(&file, b"v1").expect("создание до sync");

        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        std::fs::remove_file(&file).expect("удаление");

        let batch = normalized(wait_batch(&rx, FIRST).expect("батч удаления не пришёл"));
        assert!(batch.contains(&FileEvent::Remove(norm(&file))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 16: восстановление — remove (ждём батч Remove) → запись заново
    /// → Create (окна разделены ожиданием: Remove уже доставлен, склейки в
    /// Rename нет).
    #[test]
    fn watcher_restore_flow() {
        let dir = temp_dir("restore");
        let file = dir.join("s.txt");
        std::fs::write(&file, b"v1").expect("создание до sync");

        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        std::fs::remove_file(&file).expect("удаление");
        let removed = normalized(wait_batch(&rx, FIRST).expect("батч удаления не пришёл"));
        assert!(removed.contains(&FileEvent::Remove(norm(&file))));

        // Дождавшись батч Remove, мы знаем: окно закрылось ДО новой записи —
        // события записи откроют новое окно, склейки Remove+Create не будет
        std::fs::write(&file, b"back").expect("запись заново");
        let restored = normalized(wait_batch(&rx, FIRST).expect("батч восстановления не пришёл"));
        assert!(restored.contains(&FileEvent::Create(norm(&file))));
        assert!(!restored.contains(&FileEvent::Rename(norm(&file), norm(&file))));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 17: debounce-шквал — 10 быстрых записей → один батч с одним
    /// Modify (мягко: если ОС разорвала окно — допустимо 2 батча, но суммарно
    /// один Modify-набор без дублей).
    #[test]
    fn watcher_debounce_burst() {
        let dir = temp_dir("burst");
        let file = dir.join("burst.txt");
        std::fs::write(&file, b"v0").expect("создание до sync");

        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        for i in 0..10 {
            std::fs::write(&file, format!("v{i}")).expect("запись в цикле");
        }

        let first = normalized(wait_batch(&rx, FIRST).expect("первый батч не пришёл"));
        let rest: Vec<Vec<FileEvent>> = collect_quiet(&rx, Duration::from_secs(1))
            .into_iter()
            .map(normalized)
            .collect();
        let mut batches = vec![first];
        batches.extend(rest);

        assert!(
            batches.len() <= 2,
            "шквал должен схлопнуться в ≤2 батча, пришло {}",
            batches.len()
        );
        assert!(
            batches
                .iter()
                .flatten()
                .all(|e| matches!(e, FileEvent::Modify(_))),
            "посторонние события в шквале: {batches:?}"
        );
        let modify_paths: HashSet<PathBuf> = batches
            .iter()
            .flatten()
            .filter_map(|e| match e {
                FileEvent::Modify(path) => Some(path.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(modify_paths, HashSet::from([norm(&file)]));
        if batches.len() == 1 {
            assert_eq!(batches[0], vec![FileEvent::Modify(norm(&file))]);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 18: NonRecursive — событие о вложенной папке приходит (Create(sub)
    /// от родителя), но изменения ВНУТРИ подпапки не приходят.
    #[test]
    fn watcher_non_recursive() {
        let dir = temp_dir("nonrec");
        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));

        let sub = dir.join("sub");
        std::fs::create_dir(&sub).expect("подпапка после sync");
        let batch = normalized(wait_batch(&rx, FIRST).expect("батч Create(sub) не пришёл"));
        assert!(batch.contains(&FileEvent::Create(norm(&sub))));

        std::fs::write(sub.join("x.png"), b"png").expect("запись в подпапке");
        assert_quiet(&rx, QUIET);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 19: sync_dirs по diff — снятая директория перестаёт давать
    /// события, оставшаяся продолжает.
    #[test]
    fn watcher_sync_dirs_diff() {
        let dir_a = temp_dir("diff-a");
        let dir_b = temp_dir("diff-b");
        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(&[dir_a.clone(), dir_b.clone()]);

        let file_b = dir_b.join("b.txt");
        std::fs::write(&file_b, b"b").expect("запись в b");
        let batch = normalized(wait_batch(&rx, FIRST).expect("батч по dir_b не пришёл"));
        assert!(batch.contains(&FileEvent::Create(norm(&file_b))));

        // Снимаем dir_b; пауза: RDCW unwatch не ждёт подтверждения
        svc.sync_dirs(std::slice::from_ref(&dir_a));
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(dir_b.join("b2.txt"), b"b2").expect("запись в b после снятия");
        assert_quiet(&rx, QUIET);

        let file_a = dir_a.join("a.txt");
        std::fs::write(&file_a, b"a").expect("запись в a");
        let batch = normalized(wait_batch(&rx, FIRST).expect("батч по dir_a не пришёл"));
        assert!(batch.contains(&FileEvent::Create(norm(&file_a))));

        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    /// Тест 20: идемпотентность sync_dirs — тот же набор дважды, watch не
    /// задваивается, событие приходит один раз.
    #[test]
    fn watcher_sync_dirs_idempotent() {
        let dir = temp_dir("idem");
        let (sender, rx) = test_sender();
        let mut svc = WatchService::new(sender);
        svc.sync_dirs(std::slice::from_ref(&dir));
        svc.sync_dirs(std::slice::from_ref(&dir));
        assert_eq!(svc.watched.len(), 1); // учёт не задвоился (приватное поле)

        let file = dir.join("i.txt");
        std::fs::write(&file, b"x").expect("запись файла");

        let batch = normalized(wait_batch(&rx, FIRST).expect("батч не пришёл"));
        assert!(batch.contains(&FileEvent::Create(norm(&file))));
        assert_eq!(
            batch
                .iter()
                .filter(|e| matches!(e, FileEvent::Create(_)))
                .count(),
            1
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
