//! FR-103 (мультиканвас C0): модель записей и чистые функции
//! рабочего стола канвасов — контракты, на которых параллельно работают
//! воркстримы C1 (OpfsStore) / C2 (FsAccessStore + миграция) и C3
//! (менеджер-оверлей). Модуль платформенно-нейтрален: только `std`,
//! нативные тесты исполняются и под wasip1 (гейт wasm).
//!
//! Соглашения имён (решения владельца, план мультиканваса v2.1):
//! - **Имя записи = имя файла с расширением** `.canvas` («default.canvas»)
//!   — тот же идентификатор, что в recent/?canvas=/Web Locks; для UI есть
//!   [`display_name`] (без расширения) и [`to_file_name`] (обратный ход).
//! - Автоимя — всегда латиницей «Canvas», «Canvas 2», … (№39c), первый
//!   свободный; коллизии создания — суффикс « (N)» (№13a/№26b); дубликат —
//!   «{имя} {суффикс}», суффикс из i18n (RU «(копия)»/EN «(copy)», №27a).
//! - Сравнение занятости регистронезависимо: granted-папки на Windows
//!   не различают регистр — «notes»/«Notes» не должны становиться
//!   двойниками (миграция/переезд это сохраняет).
//! - Лимит числа канвасов — [`MAX_CANVASES`] (№16: пользовательского
//!   лимита нет, константа — единая точка санити-гварда для хранилищ).
//!
//! Активный what-if сценарий канваса — см. [`crate::whatif`]
//! (`canvasdesk.whatif.active`, №32c).

use std::collections::BTreeMap;

/// Санити-гвард числа канвасов в хранилище (№16): пользовательского лимита
/// нет, но хранилища проверяют объём в одной точке — защита от runaway
/// (зацикленный импорт/дублирование) и переполнения листинга менеджера.
pub const MAX_CANVASES: usize = 10_000;

/// Максимальная длина имени канваса (символы, после trim).
pub const MAX_NAME_LEN: usize = 120;

/// Источник записи рабочего стола (план §3.1): OPFS-библиотека,
/// granted-папка FS Access (workspace после переезда, №51a) или
/// локальный файл, открытый с диска (`«Открыть с диска…»`, №37b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Браузерное OPFS-хранилище (тихий старт, №41c).
    Opfs,
    /// Папка на диске через FS Access (после «Переехать на диск…»).
    Folder,
    /// Индивидуальный файл с диска (вне workspace-хранилища).
    Disk,
}

/// Запись списка канвасов (контракт листинга `WorkspaceStore`).
/// `repo: Some(имя)` — канвас лежит в группе `repos/<имя>` (№43a, стык с
/// PRD-0011: зеркала подключённых репозиториев показываются группами).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasEntry {
    /// Имя файла с расширением `.canvas` (канонический идентификатор).
    pub name: String,
    /// Время последнего изменения (unix ms) — сортировка «по дате».
    pub ts: u64,
    /// Источник записи.
    pub kind: EntryKind,
    /// Группа-репозиторий (`repos/<имя>`), если запись из зеркала репо.
    pub repo: Option<String>,
}

impl CanvasEntry {
    /// Отображаемое имя (без `.canvas`) — для строк менеджера и чипа.
    pub fn display(&self) -> &str {
        display_name(&self.name)
    }
}

/// Ошибка валидации имени канваса (№9: inline-ренейм; №6/№26b: создание).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameError {
    /// Пустое имя (после trim).
    Empty,
    /// Длиннее [`MAX_NAME_LEN`].
    TooLong,
    /// Символ, небезопасный для файловых систем (`/ \ : * ? " < > |`,
    /// управляющие) — поле `ch` для диагностики.
    ForbiddenChar(char),
    /// Зарезервированное имя (`.` / `..` / ведущая точка — скрытые/относительные).
    Reserved,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "имя пустое"),
            Self::TooLong => {
                write!(f, "имя длиннее {MAX_NAME_LEN} символов")
            }
            Self::ForbiddenChar(ch) => write!(f, "недопустимый символ {ch:?}"),
            Self::Reserved => write!(f, "зарезервированное имя"),
        }
    }
}

impl std::error::Error for NameError {}

// ============================================================================
// Имена: отображение/файл, валидация, занятость
// ============================================================================

/// Отображаемое имя: без хвостового `.canvas` (само имя не меняется,
/// если расширения нет). «x.canvas» → «x», «заметки» → «заметки».
pub fn display_name(file_name: &str) -> &str {
    file_name.strip_suffix(".canvas").unwrap_or(file_name)
}

/// Имя файла из отображаемого: добавляет `.canvas`, если его ещё нет
/// (двойное расширение не плодим: «x.canvas» остаётся как есть).
pub fn to_file_name(display: &str) -> String {
    if display.ends_with(".canvas") {
        display.to_owned()
    } else {
        format!("{display}.canvas")
    }
}

/// Занято ли имя (регистронезависимо — семантика Windows-папок, см. модуль).
pub fn name_taken(file_name: &str, existing: &[CanvasEntry]) -> bool {
    let wanted = file_name.to_lowercase();
    existing
        .iter()
        .any(|entry| entry.name.to_lowercase() == wanted)
}

/// Валидация отображаемого имени (№9): trim → непусто, ≤[`MAX_NAME_LEN`],
/// без `/ \ : * ? " < > |` и управляющих, не `.`/`..`/без ведущей точки.
/// Возвращает нормализованное (обрезанное по пробелам) имя.
pub fn validate_canvas_name(raw: &str) -> Result<String, NameError> {
    let name = raw.trim();
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > MAX_NAME_LEN {
        return Err(NameError::TooLong);
    }
    if name == "." || name == ".." || name.starts_with('.') {
        return Err(NameError::Reserved);
    }
    for ch in name.chars() {
        if matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || ch.is_control() {
            return Err(NameError::ForbiddenChar(ch));
        }
    }
    Ok(name.to_owned())
}

// ============================================================================
// Автоимена и коллизии (№39c / №13a / №26b / №27a)
// ============================================================================

/// Автоимя нового канваса (№6/№39c): «Canvas», «Canvas 2», «Canvas 3», … —
/// первый свободный, всегда латиницей (обе локали). Возвращает имя файла.
pub fn auto_name(existing: &[CanvasEntry]) -> String {
    for n in 1u64.. {
        let candidate = if n == 1 {
            "Canvas".to_owned()
        } else {
            format!("Canvas {n}")
        };
        let file = to_file_name(&candidate);
        if !name_taken(&file, existing) {
            return file;
        }
    }
    unreachable!("регистронезависимых имён меньше, чем u64")
}

/// Суффикс коллизии (№13a/№26b): «Canvas 2» занят → «Canvas 2 (1)»,
/// затем «(2)», … Свободное имя возвращается как есть. Суффикс встаёт
/// ПЕРЕД расширением: «x.canvas» → «x (1).canvas».
pub fn collision_suffix(file_name: &str, existing: &[CanvasEntry]) -> String {
    if !name_taken(file_name, existing) {
        return file_name.to_owned();
    }
    let display = display_name(file_name);
    for n in 1u64.. {
        let candidate = to_file_name(&format!("{display} ({n})"));
        if !name_taken(&candidate, existing) {
            return candidate;
        }
    }
    unreachable!()
}

/// Имя дубликата (№27a): «{имя} {суффикс}» (суффикс из i18n: RU «(копия)» /
/// EN «(copy)»); занято — поверх авто-суффикс « (1)», « (2)», …
pub fn copy_name(file_name: &str, existing: &[CanvasEntry], suffix: &str) -> String {
    let candidate = to_file_name(&format!("{} {suffix}", display_name(file_name)));
    if !name_taken(&candidate, existing) {
        return candidate;
    }
    collision_suffix(&candidate, existing)
}

// ============================================================================
// Сортировка и группировка листинга (№43a)
// ============================================================================

/// Режим сортировки списка канвасов в менеджере.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// По имени (регистронезависимо, стабильный порядок).
    Name,
    /// По дате изменения (свежие сверху; равные ts — по имени).
    ModifiedDesc,
}

/// Чистая сортировка записей (стабильная; вход не изменяется).
pub fn sorted_entries(entries: &[CanvasEntry], mode: SortMode) -> Vec<CanvasEntry> {
    let mut sorted = entries.to_vec();
    match mode {
        SortMode::Name => sorted.sort_by_key(|entry| entry.display().to_lowercase()),
        SortMode::ModifiedDesc => sorted.sort_by(|a, b| {
            b.ts.cmp(&a.ts)
                .then_with(|| a.display().to_lowercase().cmp(&b.display().to_lowercase()))
        }),
    }
    sorted
}

/// Группировка листинга (№43a): корень плоско + группы `repos/<имя>`
/// (алфавит групп — BTreeMap). Порядок внутри групп — как во входе
/// (сортируйте до группировки: [`sorted_entries`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListingGroups {
    /// Записи вне репозиториев (корень workspace).
    pub root: Vec<CanvasEntry>,
    /// Группы-зеркала репо: имя → записи (`repos/<имя>`).
    pub repos: BTreeMap<String, Vec<CanvasEntry>>,
}

/// Разбить листинг на корень и группы репозиториев.
pub fn group_entries(entries: &[CanvasEntry]) -> ListingGroups {
    let mut groups = ListingGroups::default();
    for entry in entries {
        match &entry.repo {
            None => groups.root.push(entry.clone()),
            Some(repo) => groups
                .repos
                .entry(repo.clone())
                .or_default()
                .push(entry.clone()),
        }
    }
    groups
}

// ============================================================================
// FR-104 (C1): payload операции хранилища для события CanvasOpDone
// ============================================================================

/// FR-104 (мультиканвас C1): операция `WorkspaceStore` для события
/// `AppEvent::CanvasOpDone` — payload живёт в ядре, потому что canvas-app
/// (владелец `AppEvent`) не зависит от canvas-web, где определён
/// `WorkspaceError`: результат переносится как `Option<String>`
/// (человекочитаемый текст ошибки, `WorkspaceError::to_string`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanvasOp {
    /// Создание пустого канваса.
    Create { name: String },
    /// Переименование (файл + `.bak`-близнец).
    Rename { old: String, new: String },
    /// Мягкое удаление (№15a: файл → `<name>.bak`).
    Delete { name: String },
}

// ============================================================================
// Ключ камеры и план миграции (№12/№30b, №42a/№52a)
// ============================================================================

/// Ключ localStorage камеры канваса (№12/№30b): формат заморожен в C0,
/// потребитель — C4 (перенос ключа при ренейме — сторона платформенного
/// слоя, файловый ренейм — `WorkspaceStore::rename`).
pub fn camera_key_for(file_name: &str) -> String {
    format!("canvasdesk.camera.{file_name}")
}

// ============================================================================
// FR-107 (мультиканвас C4): сериализация камеры канваса (№12/№30b)
// ============================================================================

/// Снимок камеры канваса (№12/№30b): центр viewport в world-координатах
/// + зум. Живёт в ядре (canvas-render не виден веб-слою напрямую, а
/// зависимость направлена core ← render — снимок собирает потребитель,
/// владеющий камерой). Хранение — localStorage по ключу
/// [`camera_key_for`]; перенос при ренейме активного — `CanvasMoveCameraKey`
/// (C3, значение переносится как есть).
///
/// Формат строки — «x;y;zoom» (разделитель `;`, координаты и зум —
/// f32 с округлением: центр до 2 знаков, зум до 3): компактность
/// важнее точности — визуально неотличимо, а round-trip стабилен.
/// Битые строки (не 3 части, не-числа, NaN/±inf) — `decode_camera`
/// возвращает `None` → потребитель молча берёт дефолтную камеру.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSnapshot {
    /// Мировая точка в центре viewport.
    pub center: [f32; 2],
    /// Зум (валидация диапазона — сторона камеры: `Camera::set_zoom`
    /// клампит при применении).
    pub zoom: f32,
}

/// Закодировать снимок камеры: «x;y;zoom» с округлением (центр — 2
/// знака, зум — 3; половинное округление не критично — восстановление
/// визуально неотличимо). Не-числа кодируются как «0» — битые снимки не
/// появляются в хранилище.
pub fn encode_camera(snapshot: &CameraSnapshot) -> String {
    let f = |v: f32| if v.is_finite() { v } else { 0.0 };
    format!(
        "{:.2};{:.2};{:.3}",
        f(snapshot.center[0]),
        f(snapshot.center[1]),
        f(snapshot.zoom)
    )
}

/// Разобрать строку камеры (обратный ход [`encode_camera`]): строго 3
/// части по `;`, каждая — конечное f32; иначе `None` (битая/чужая строка
/// → дефолтная камера, без паники). Пробелы вокруг частей допустимы
/// (ручная правка localStorage не ломает загрузку).
pub fn decode_camera(raw: &str) -> Option<CameraSnapshot> {
    let mut parts = raw.split(';');
    let mut next = || -> Option<f32> {
        let value = parts.next()?.trim().parse::<f32>().ok()?;
        value.is_finite().then_some(value)
    };
    let x = next()?;
    let y = next()?;
    let zoom = next()?;
    if parts.next().is_some() {
        return None; // ровно 3 части — хвост лишний
    }
    Some(CameraSnapshot {
        center: [x, y],
        zoom,
    })
}

/// План миграции OPFS → папка (№42a): чистая функция над двумя листингами;
/// исполнение (копирование + удаление оригиналов, №52a) — волна C2.
///
/// Коллизии в ЦЕЛИ решаются авто-суффиксом « (N)» (семантика №26b) —
/// внутри плана имена тоже уникальны. Отсутствующие в источнике имена
/// собираются в `missing` (чекбокс-лист валидирует выбор, но план честно
/// сообщает о расхождении с листингом).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationPlan {
    /// Копии: (имя в OPFS, итоговое имя в папке).
    pub copies: Vec<(String, String)>,
    /// Выбранные имена, которых нет в источнике (не блокируют остальные).
    pub missing: Vec<String>,
}

/// Построить план миграции: `source` — листинг OPFS, `selected` — выбранные
/// имена файлов, `target_existing` — уже лежащие в папке назначения.
pub fn migration_plan(
    source: &[CanvasEntry],
    selected: &[String],
    target_existing: &[CanvasEntry],
) -> MigrationPlan {
    let mut plan = MigrationPlan::default();
    // Занятость в цели растёт по мере планирования (суффиксы не слипаются).
    let mut target_names: Vec<CanvasEntry> = target_existing.to_vec();
    for name in selected {
        let Some(entry) = source
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
        else {
            plan.missing.push(name.clone());
            continue;
        };
        let dst = if name_taken(&entry.name, &target_names) {
            collision_suffix(&entry.name, &target_names)
        } else {
            entry.name.clone()
        };
        target_names.push(CanvasEntry {
            name: dst.clone(),
            ts: entry.ts,
            kind: EntryKind::Folder,
            repo: None,
        });
        plan.copies.push((entry.name.clone(), dst));
    }
    plan
}

// ============================================================================
// Watch внешних изменений (№45b/№53b) — чистая часть
// ============================================================================

/// Снимок папки для детекта внешних изменений (№45b): «имя → время
/// последнего изменения (unix ms)». Строит платформенный слой (poll
/// `lastModified` на focus/visibilitychange), сравнивает — чистая
/// функция [`snapshot_changed`] ниже. Ключи — имена файлов (белый список
/// `.canvas` — обязанность построителя снимка; `.bak`-близнецы в Prompt
/// не участвуют).
pub type WatchSnapshot = BTreeMap<String, u64>;

/// Имена файлов, изменившихся снаружи между двумя снимками: `ts` вырос
/// или файл появился. Удалённые/переименованные наружу НЕ считаются
/// изменением активного файла (перезагружать нечего — их обрабатывает
/// листинг менеджера, волна C3). Порядок — алфавит (BTreeMap-итерация,
/// детерминизм для тестов).
pub fn snapshot_changed(before: &WatchSnapshot, after: &WatchSnapshot) -> Vec<String> {
    after
        .iter()
        .filter(|(name, ts)| before.get(*name) != Some(ts))
        .map(|(name, _)| name.clone())
        .collect()
}

// ============================================================================
// Тесты (нативные + wasip1: модуль чистый, std-only)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, ts: u64) -> CanvasEntry {
        CanvasEntry {
            name: name.to_owned(),
            ts,
            kind: EntryKind::Opfs,
            repo: None,
        }
    }

    fn names(entries: &[CanvasEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    // --- имена: отображение/файл -------------------------------------------

    #[test]
    fn display_and_file_roundtrip() {
        assert_eq!(display_name("проект.canvas"), "проект");
        assert_eq!(
            display_name("заметки"),
            "заметки",
            "без расширения — как есть"
        );
        assert_eq!(to_file_name("проект"), "проект.canvas");
        assert_eq!(
            to_file_name("x.canvas"),
            "x.canvas",
            "двойное расширение не плодим"
        );
        assert_eq!(display_name(&to_file_name("a b")), "a b");
    }

    #[test]
    fn name_taken_is_case_insensitive() {
        let existing = [entry("Notes.canvas", 1)];
        assert!(name_taken("notes.canvas", &existing));
        assert!(name_taken("NOTES.CANVAS", &existing));
        assert!(!name_taken("notes2.canvas", &existing));
    }

    // --- валидация (№9) -----------------------------------------------------

    #[test]
    fn validate_accepts_normal_names() {
        assert_eq!(
            validate_canvas_name("  Мой проект  "),
            Ok("Мой проект".into())
        );
        assert_eq!(
            validate_canvas_name("Отчёт Q3 — v2 (черновик)"),
            Ok("Отчёт Q3 — v2 (черновик)".into()),
            "тире/скобки/цифры разрешены"
        );
        assert_eq!(validate_canvas_name(&"a".repeat(120)), Ok("a".repeat(120)));
    }

    #[test]
    fn validate_rejects_bad_names() {
        assert_eq!(validate_canvas_name("   "), Err(NameError::Empty));
        assert_eq!(
            validate_canvas_name(&"a".repeat(121)),
            Err(NameError::TooLong)
        );
        for bad in [".", "..", ".hidden"] {
            assert_eq!(validate_canvas_name(bad), Err(NameError::Reserved), "{bad}");
        }
        for bad in [
            "a/b", "a\\b", "a:b", "a*b", "a?b", "a\"b", "a<b", "a>b", "a|b", "a\u{0}b",
        ] {
            assert!(
                matches!(validate_canvas_name(bad), Err(NameError::ForbiddenChar(_))),
                "{bad}"
            );
        }
    }

    // --- автоимена/коллизии (№39c/№13a/№27a) --------------------------------

    #[test]
    fn auto_name_first_free_latin() {
        assert_eq!(auto_name(&[]), "Canvas.canvas");
        let existing = [entry("default.canvas", 1), entry("Canvas.canvas", 2)];
        assert_eq!(auto_name(&existing), "Canvas 2.canvas");
        let existing = [
            entry("Canvas.canvas", 1),
            entry("Canvas 2.canvas", 2),
            entry("Canvas 3.canvas", 3),
        ];
        assert_eq!(auto_name(&existing), "Canvas 4.canvas");
        // регистронезависимость: CANVAS.canvas занимает первый слот
        let existing = [entry("CANVAS.canvas", 1)];
        assert_eq!(auto_name(&existing), "Canvas 2.canvas");
    }

    #[test]
    fn collision_suffix_numbers() {
        let existing = [entry("Canvas 2.canvas", 1)];
        assert_eq!(
            collision_suffix("Canvas 2.canvas", &existing),
            "Canvas 2 (1).canvas",
            "суффикс перед расширением (№13a)"
        );
        let existing = [
            entry("x.canvas", 1),
            entry("x (1).canvas", 2),
            entry("x (2).canvas", 3),
        ];
        assert_eq!(collision_suffix("x.canvas", &existing), "x (3).canvas");
        assert_eq!(
            collision_suffix("free.canvas", &existing),
            "free.canvas",
            "свободное не трогаем"
        );
    }

    #[test]
    fn copy_name_with_i18n_suffix() {
        let existing = [entry("Идея.canvas", 1)];
        assert_eq!(
            copy_name("Идея.canvas", &existing, "(копия)"),
            "Идея (копия).canvas",
            "№27a"
        );
        assert_eq!(
            copy_name("Идея.canvas", &existing, "(copy)"),
            "Идея (copy).canvas",
            "суффикс приходит из i18n"
        );
        let existing = [entry("Идея.canvas", 1), entry("Идея (копия).canvas", 2)];
        assert_eq!(
            copy_name("Идея.canvas", &existing, "(копия)"),
            "Идея (копия) (1).canvas",
            "коллизия поверх копии — авто-суффикс"
        );
    }

    // --- сортировка/группы (№43a) -------------------------------------------

    #[test]
    fn sort_by_name_and_modified() {
        let entries = vec![
            entry("b.canvas", 10),
            entry("A.canvas", 30),
            entry("a2.canvas", 30),
            entry("c.canvas", 5),
        ];
        assert_eq!(
            names(&sorted_entries(&entries, SortMode::Name)),
            ["A.canvas", "a2.canvas", "b.canvas", "c.canvas"],
            "регистронезависимо, A < a2 (короткое — меньше)"
        );
        assert_eq!(
            names(&sorted_entries(&entries, SortMode::ModifiedDesc)),
            ["A.canvas", "a2.canvas", "b.canvas", "c.canvas"],
            "свежие сверху, равные ts — по имени (a2 после A)"
        );
    }

    #[test]
    fn groups_root_and_repos() {
        let entries = vec![
            entry("мой.canvas", 1),
            CanvasEntry {
                name: "svc.canvas".into(),
                ts: 2,
                kind: EntryKind::Folder,
                repo: Some("infra".into()),
            },
            entry("default.canvas", 3),
            CanvasEntry {
                name: "billing.canvas".into(),
                ts: 4,
                kind: EntryKind::Folder,
                repo: Some("mono".into()),
            },
            CanvasEntry {
                name: "core.canvas".into(),
                ts: 5,
                kind: EntryKind::Folder,
                repo: Some("infra".into()),
            },
        ];
        let groups = group_entries(&entries);
        assert_eq!(names(&groups.root), ["мой.canvas", "default.canvas"]);
        let repo_names: Vec<_> = groups.repos.keys().collect();
        assert_eq!(repo_names, ["infra", "mono"], "алфавит групп");
        assert_eq!(
            names(&groups.repos["infra"]),
            ["svc.canvas", "core.canvas"],
            "порядок внутри группы — как во входе"
        );
        assert_eq!(names(&groups.repos["mono"]), ["billing.canvas"]);
    }

    // --- ключ камеры (№12/№30b) ---------------------------------------------

    #[test]
    fn camera_key_format() {
        assert_eq!(
            camera_key_for("default.canvas"),
            "canvasdesk.camera.default.canvas"
        );
        assert_eq!(camera_key_for("a b.canvas"), "canvasdesk.camera.a b.canvas");
        assert_ne!(camera_key_for("x.canvas"), camera_key_for("y.canvas"));
    }

    // --- FR-107 (C4): сериализация камеры (№12/№30b) ------------------------

    #[test]
    fn camera_snapshot_roundtrip() {
        let snapshot = CameraSnapshot {
            center: [-1234.5678, 42.0],
            zoom: 0.75,
        };
        let encoded = encode_camera(&snapshot);
        assert_eq!(encoded, "-1234.57;42.00;0.750", "формат x;y;zoom с округлением");
        assert_eq!(
            decode_camera(&encoded),
            Some(CameraSnapshot {
                center: [-1234.57, 42.0],
                zoom: 0.75,
            }),
            "round-trip восстанавливает округлённые значения"
        );
        // Дефолтная камера (0;0;1) — валидный снимок
        assert_eq!(
            decode_camera(&encode_camera(&CameraSnapshot {
                center: [0.0, 0.0],
                zoom: 1.0
            })),
            Some(CameraSnapshot {
                center: [0.0, 0.0],
                zoom: 1.0
            })
        );
    }

    #[test]
    fn camera_decode_broken_strings_fall_to_none() {
        // Не 3 части
        assert_eq!(decode_camera(""), None);
        assert_eq!(decode_camera("1;2"), None);
        assert_eq!(decode_camera("1;2;3;4"), None);
        // Не-числа
        assert_eq!(decode_camera("a;b;c"), None);
        assert_eq!(decode_camera("1;b;3"), None);
        // NaN/inf — не конечные
        assert_eq!(decode_camera("NaN;0;1"), None);
        assert_eq!(decode_camera("0;inf;1"), None);
        assert_eq!(decode_camera("0;0;-inf"), None);
        // Пробелы вокруг частей — допустимы (ручная правка хранилища)
        assert_eq!(
            decode_camera(" 10.5 ; -20.25 ; 1.0 "),
            Some(CameraSnapshot {
                center: [10.5, -20.25],
                zoom: 1.0
            })
        );
    }

    #[test]
    fn camera_encode_sanitizes_non_finite() {
        // Битые значения не попадают в хранилище: NaN/inf → 0
        assert_eq!(
            encode_camera(&CameraSnapshot {
                center: [f32::NAN, f32::INFINITY],
                zoom: f32::NEG_INFINITY
            }),
            "0.00;0.00;0.000"
        );
        // Отрицательный зум кодируется как есть — кламп на применении
        // (`Camera::set_zoom` — сторона камеры)
        assert_eq!(
            encode_camera(&CameraSnapshot {
                center: [0.0, 0.0],
                zoom: -2.5
            }),
            "0.00;0.00;-2.500"
        );
    }

    // --- план миграции (№42a) -------------------------------------------------

    #[test]
    fn migration_plan_free_names_copy_as_is() {
        let source = [entry("a.canvas", 1), entry("b.canvas", 2)];
        let plan = migration_plan(&source, &["a.canvas".into(), "b.canvas".into()], &[]);
        assert_eq!(
            plan.copies,
            [
                ("a.canvas".into(), "a.canvas".into()),
                ("b.canvas".into(), "b.canvas".into())
            ]
        );
        assert!(plan.missing.is_empty());
    }

    #[test]
    fn migration_plan_collisions_suffixed_per_plan() {
        let source = [entry("x.canvas", 1), entry("y.canvas", 2)];
        let target = [entry("x.canvas", 9)];
        let plan = migration_plan(&source, &["x.canvas".into()], &target);
        assert_eq!(
            plan.copies,
            [("x.canvas".into(), "x (1).canvas".into())],
            "коллизия в цели — авто-суффикс (№26b)"
        );
        // два одинаковых выбора: суффиксы не слипаются
        let plan = migration_plan(&source, &["x.canvas".into(), "x.canvas".into()], &[]);
        assert_eq!(
            plan.copies,
            [
                ("x.canvas".into(), "x.canvas".into()),
                ("x.canvas".into(), "x (1).canvas".into())
            ]
        );
    }

    #[test]
    fn migration_plan_reports_missing() {
        let source = [entry("a.canvas", 1)];
        let plan = migration_plan(&source, &["a.canvas".into(), "ghost.canvas".into()], &[]);
        assert_eq!(plan.missing, ["ghost.canvas".to_owned()]);
        assert_eq!(plan.copies.len(), 1, "остальные копии не блокируются");
    }

    // --- FR-104 (C1): payload операций ----------------------------------------

    #[test]
    fn canvas_op_payloads() {
        let create = CanvasOp::Create {
            name: "новый.canvas".into(),
        };
        let rename = CanvasOp::Rename {
            old: "a.canvas".into(),
            new: "b.canvas".into(),
        };
        let delete = CanvasOp::Delete {
            name: "gone.canvas".into(),
        };
        // Разные операции — разные payload'ы (равенство различает варианты)
        assert_ne!(create, rename);
        assert_ne!(rename, delete);
        assert_ne!(create, delete);
        // Клон идентичен (событие уходит в AppEvent как есть)
        assert_eq!(create.clone(), create);
        assert_eq!(
            rename,
            CanvasOp::Rename {
                old: "a.canvas".into(),
                new: "b.canvas".into()
            }
        );
        assert!(format!("{create:?}").contains("новый.canvas"));
    }

    // --- FR-105 (C2): watch внешних изменений (№45b/№53b) -------------------------------

    fn snap(pairs: &[(&str, u64)]) -> WatchSnapshot {
        pairs
            .iter()
            .map(|(name, ts)| (name.to_string(), *ts))
            .collect()
    }

    #[test]
    fn snapshot_changed_detects_modified_and_new() {
        let before = snap(&[("a.canvas", 10), ("b.canvas", 20)]);
        let after = snap(&[("a.canvas", 11), ("b.canvas", 20), ("c.canvas", 1)]);
        assert_eq!(
            snapshot_changed(&before, &after),
            ["a.canvas", "c.canvas"],
            "изменившийся ts + новый файл; нетронутый b молчит"
        );
    }

    #[test]
    fn snapshot_changed_ignores_deleted_and_equal() {
        let before = snap(&[("a.canvas", 10), ("gone.canvas", 5)]);
        // gone.canvas удалён снаружи, a.canvas не менялся
        let after = snap(&[("a.canvas", 10)]);
        assert!(
            snapshot_changed(&before, &after).is_empty(),
            "удаление и равный ts — не «файл изменился» (активный перезагружать нечем)"
        );
        // снимки без пересечений
        assert!(snapshot_changed(&snap(&[]), &snap(&[])).is_empty());
    }
}
