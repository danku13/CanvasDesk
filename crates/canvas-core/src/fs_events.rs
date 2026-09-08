//! Модель файловых событий вотчера (T10, SPEC §7.5): платформенно-независимые
//! типы, нормализация путей и чистое применение событий к модели канваса.
//!
//! Shell (`canvas_shell::watcher`) доставляет батчи `FileEvent` с абсолютными
//! путями ОС; здесь они нормализуются (verbatim-префикс `\\?\`, разделители)
//! и сопоставляются с `Node.file` (относительные пути резолвятся от каталога
//! `.canvas` — конвенция JSON Canvas). Весь модуль — чистые функции без
//! блокировок и I/O: тестируется на любой ОС (AGENTS.md, ядро без GPU/ОС).

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use crate::model::Canvas;

/// Событие файловой системы, доставленное вотчером после debounce-агрегации.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    /// Путь появился (файл или каталог).
    Create(PathBuf),
    /// Содержимое или метаданные пути изменились.
    Modify(PathBuf),
    /// Переименование/перемещение: (старый путь, новый путь).
    Rename(PathBuf, PathBuf),
    /// Путь исчез (файл или каталог).
    Remove(PathBuf),
}

/// Реакция модели на применённое событие — что app-слой должен сделать
/// (инвалидировать тамбнейл-кэши, автосейв, перерисовка, пересборка вотчеров).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeChange {
    /// Содержимое файла ноды изменилось — тамбнейл устарел, перезапросить.
    ThumbStale(usize),
    /// Путь ноды обновлён (rename) — автосейв и пересинхронизация директорий.
    PathUpdated(usize),
    /// Файл ноды исчез — установлен brokenLink.
    Broken(usize),
    /// Файл ноды вернулся по прежнему пути — brokenLink снят.
    Restored(usize),
}

/// Каноническая форма пути для сравнения: без verbatim-префиксов `\\?\`/`\\.\`
/// (ReadDirectoryChangesW, план T10 §5 шаг 1), с унифицированными `/`.
/// Регистр НЕ меняется — сравнение регистронезависимое на Windows отдельно
/// (`path_eq`/`path_under`), чтобы пути в модели сохраняли исходный регистр.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut s = path.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_owned();
    } else if let Some(rest) = s.strip_prefix(r"\\.\") {
        s = rest.to_owned();
    }
    s = s.replace('\\', "/");
    PathBuf::from(s)
}

/// Абсолютный путь файловой ноды: `Node.file` абсолютный — как есть,
/// относительный — от каталога `.canvas`-файла (конвенция JSON Canvas),
/// с абсолютизацией через cwd (без canonicalize — он падает на битых ссылках).
pub fn resolve_node_path(node_file: &str, canvas_dir: &Path) -> PathBuf {
    let path = PathBuf::from(node_file);
    if path.is_absolute() {
        return path;
    }
    let joined = if canvas_dir.as_os_str().is_empty() {
        path
    } else {
        canvas_dir.join(&path)
    };
    if joined.is_absolute() {
        joined
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&joined))
            .unwrap_or(joined)
    }
}

/// Равенство путей после нормализации; на Windows — регистронезависимое
/// (NTFS case-insensitive), на Unix — точное (case-sensitive).
fn path_eq(a: &Path, b: &Path) -> bool {
    let (a, b) = (normalize_path(a), normalize_path(b));
    if cfg!(windows) {
        let lower = |p: &Path| -> Vec<String> {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
                .collect()
        };
        lower(&a) == lower(&b)
    } else {
        a == b
    }
}

/// `path` находится СТРОГО ПОД `dir` (component-wise префикс, без равенства);
/// на Windows — регистронезависимо.
fn path_under(path: &Path, dir: &Path) -> bool {
    let (path, dir) = (normalize_path(path), normalize_path(dir));
    if dir.components().next().is_none() {
        return false;
    }
    let mut path_components = path.components();
    // Префикс должен совпасть полностью, и у path обязаны остаться компоненты
    for dir_c in dir.components() {
        match path_components.next() {
            Some(path_c) if components_eq(&path_c, &dir_c) => continue,
            _ => return false,
        }
    }
    path_components.next().is_some()
}

/// Суффикс `path` после снятия префикса `dir` (component-wise, на Windows
/// регистронезависимо) — в исходном регистре `path`; None, если префикса нет.
fn strip_prefix_ci(dir: &Path, path: &Path) -> Option<PathBuf> {
    let mut path_components = path.components();
    for dir_c in dir.components() {
        match path_components.next() {
            Some(path_c) if components_eq(&path_c, &dir_c) => continue,
            _ => return None,
        }
    }
    let rest: PathBuf = path_components.collect();
    Some(rest)
}

/// Нода-владелец события: `Node.file` резолвится от каталога канваса
/// и сравнивается с путём события (нормализация + регистр ОС).
pub fn path_matches(node_file: &str, canvas_dir: &Path, event_path: &Path) -> bool {
    path_eq(&resolve_node_path(node_file, canvas_dir), event_path)
}

/// Применить события к модели (чистая функция; canvas мутируется).
/// Rename обновляет `file` (точное совпадение + PATH-префикс при
/// переименовании каталога) и снимает brokenLink, Remove ставит brokenLink,
/// Create по пути битой ноды снимает флаг, Modify (и atomic-save
/// rename tmp→file) помечает тамбнейл устаревшим.
/// Возвращает изменения в порядке событий, без дублей.
pub fn apply_file_events(
    canvas: &mut Canvas,
    canvas_dir: &Path,
    events: &[FileEvent],
) -> Vec<NodeChange> {
    let mut changes: Vec<NodeChange> = Vec::new();
    for event in events {
        match event {
            FileEvent::Modify(path) => {
                for (index, node) in canvas.nodes.iter().enumerate() {
                    let Some(file) = node.file.as_deref() else {
                        continue; // ноды без file (текст/группы) событий не имеют
                    };
                    if path_matches(file, canvas_dir, path) {
                        push_unique(&mut changes, NodeChange::ThumbStale(index));
                    }
                }
            }
            FileEvent::Rename(from, to) => {
                apply_rename(canvas, canvas_dir, from, to, &mut changes);
            }
            FileEvent::Remove(path) => {
                for (index, node) in canvas.nodes.iter_mut().enumerate() {
                    let Some(file) = node.file.as_deref() else {
                        continue;
                    };
                    let resolved = resolve_node_path(file, canvas_dir);
                    // исчез сам файл ноды или каталог над ним
                    let gone = path_eq(&resolved, path) || path_under(&resolved, path);
                    if gone && node.broken_link != Some(true) {
                        node.broken_link = Some(true);
                        push_unique(&mut changes, NodeChange::Broken(index));
                    }
                }
            }
            FileEvent::Create(path) => {
                for (index, node) in canvas.nodes.iter_mut().enumerate() {
                    let Some(file) = node.file.as_deref() else {
                        continue;
                    };
                    // Create трактуется только как восстановление битой ноды
                    // (план T10 §7: новых нод из событий не создаём)
                    if node.broken_link == Some(true) && path_matches(file, canvas_dir, path) {
                        node.broken_link = None;
                        push_unique(&mut changes, NodeChange::Restored(index));
                    }
                }
            }
        }
    }
    changes
}

/// Добавить изменение в конец без дублей (порядок первых вхождений, T10 §5 шаг 2).
fn push_unique(changes: &mut Vec<NodeChange>, change: NodeChange) {
    if !changes.contains(&change) {
        changes.push(change);
    }
}

/// Переименование/перемещение: (a) точный владелец `from`; (b) ноды строго
/// под `from` — переименование каталога (суффикс после префикса сохраняет
/// регистр узла, `strip_prefix_ci`); (c) atomic-save — узел уже смотрит на
/// `to` (редактор сохранил через rename tmp→file): тамбнейл устарел, путь нет.
/// Узлы, обновлённые в (a)/(b), из (c) исключаются.
fn apply_rename(
    canvas: &mut Canvas,
    canvas_dir: &Path,
    from: &Path,
    to: &Path,
    changes: &mut Vec<NodeChange>,
) {
    let mut updated: Vec<usize> = Vec::new();
    for (index, node) in canvas.nodes.iter_mut().enumerate() {
        let Some(file) = node.file.as_deref() else {
            continue;
        };
        let resolved = resolve_node_path(file, canvas_dir);
        let new_abs = if path_eq(&resolved, from) {
            Some(to.to_path_buf())
        } else if path_under(&resolved, from) {
            // нормализация обеих сторон: смешанные разделители не должны
            // ломать снятие префикса (path_under уже сравнил компоненты)
            strip_prefix_ci(&normalize_path(from), &normalize_path(&resolved))
                .map(|suffix| to.join(suffix))
        } else {
            None
        };
        if let Some(new_abs) = new_abs {
            node.file = Some(relative_if_inside(canvas_dir, &new_abs));
            node.broken_link = None; // по новому пути файл существует
            updated.push(index);
            push_unique(changes, NodeChange::PathUpdated(index));
        }
    }
    for (index, node) in canvas.nodes.iter().enumerate() {
        if updated.contains(&index) {
            continue;
        }
        let Some(file) = node.file.as_deref() else {
            continue;
        };
        if path_matches(file, canvas_dir, to) {
            push_unique(changes, NodeChange::ThumbStale(index));
        }
    }
}

/// Относительное представление абсолютного пути для записи в `Node.file`:
/// внутри каталога канваса — относительный путь с `/` (конвенция JSON Canvas),
/// снаружи — абсолютный с `/`. Регистр исходного пути сохраняется.
pub fn relative_if_inside(canvas_dir: &Path, abs: &Path) -> String {
    // обе стороны нормализуются: каталог канваса и события могут прийти
    // с разными стилями разделителей (модель '/', watcher — ОС)
    let dir = normalize_path(canvas_dir);
    let abs_norm = normalize_path(abs);
    match strip_prefix_ci(&dir, &abs_norm) {
        // вырожденный случай: abs — сам каталог канваса
        Some(rel) if rel.as_os_str().is_empty() => ".".to_owned(),
        Some(rel) => rel
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        None => abs_norm.to_string_lossy().into_owned(),
    }
}

/// Директории для вотчинга: нормализованные родительские каталоги всех
/// файловых нод (dedup + сортировка — детерминированный diff в sync_dirs).
pub fn watched_dirs(canvas: &Canvas, canvas_dir: &Path) -> Vec<PathBuf> {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for node in &canvas.nodes {
        let Some(file) = node.file.as_deref() else {
            continue; // текстовые ноды директорий не дают
        };
        let resolved = resolve_node_path(file, canvas_dir);
        // вырожденный случай (относительный резолв без cwd): пустой родитель
        if let Some(parent) = resolved.parent().filter(|p| !p.as_os_str().is_empty()) {
            dirs.insert(normalize_path(parent));
        }
    }
    dirs.into_iter().collect()
}

/// Сравнение компонентов пути с учётом платформы (Windows — без регистра).
fn components_eq(a: &Component, b: &Component) -> bool {
    if cfg!(windows) {
        a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;
    use std::str::FromStr;

    /// Абсолютный путь текущей платформы: на Windows — с диском (пути без
    /// префикса на Unix не абсолютны — main.rs resolve_file_path), на Unix —
    /// от корня. Windows-пути как чистые строки тестируются отдельно
    /// (normalize — строковая операция, работает на любой ОС).
    fn platform_abs(rest: &str) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(format!("C:/{rest}"))
        } else {
            PathBuf::from(format!("/{rest}"))
        }
    }

    /// normalize_path: verbatim-префиксы `\\?\` и `\\.\` (ReadDirectoryChangesW) срезаются.
    #[test]
    fn normalize_path_strips_verbatim_prefixes() {
        assert_eq!(
            normalize_path(Path::new(r"\\?\C:\a\b.png")),
            PathBuf::from("C:/a/b.png")
        );
        assert_eq!(
            normalize_path(Path::new(r"\\.\C:\dev\note.txt")),
            PathBuf::from("C:/dev/note.txt")
        );
    }

    /// normalize_path: `\` → `/`, регистр символов сохраняется.
    #[test]
    fn normalize_path_unifies_separators_keeps_case() {
        assert_eq!(
            normalize_path(Path::new(r"C:\Work\Sub\F.png")),
            PathBuf::from("C:/Work/Sub/F.png")
        );
        assert_eq!(
            normalize_path(Path::new("docs/SPEC.md")),
            PathBuf::from("docs/SPEC.md")
        );
    }

    /// resolve_node_path: относительный резолвится от canvas_dir, абсолютный — как есть.
    #[test]
    fn resolve_node_path_relative_and_absolute() {
        let dir = platform_abs("work");
        assert_eq!(resolve_node_path("sub/f.png", &dir), dir.join("sub/f.png"));
        let abs = platform_abs("elsewhere/g.png");
        assert_eq!(resolve_node_path(&abs.to_string_lossy(), &dir), abs);
    }

    /// resolve_node_path: пустой canvas_dir — cwd-фолбэк, не паникует,
    /// относительный file абсолютизируется.
    #[test]
    fn resolve_node_path_empty_dir_cwd_fallback() {
        let resolved = resolve_node_path("rel/f.png", Path::new(""));
        assert!(
            resolved.is_absolute(),
            "ожидался абсолютный путь: {resolved:?}"
        );
    }

    /// path_matches: точное совпадение после резолва; чужой путь не матчится.
    #[test]
    fn path_matches_exact_and_foreign() {
        let dir = platform_abs("work");
        assert!(path_matches("sub/f.png", &dir, &dir.join("sub/f.png")));
        assert!(!path_matches("sub/f.png", &dir, &dir.join("other/x.png")));
    }

    /// path_matches: смешанные разделители node_file (`\`) и события (`/`)
    /// унифицируются normalize на любой ОС.
    #[test]
    fn path_matches_mixed_separators() {
        let dir = platform_abs("work");
        assert!(path_matches(r"sub\f.png", &dir, &dir.join("sub/f.png")));
    }

    /// Windows: NTFS регистронезависим — пути в разном регистре матчатся
    /// (на Unix регистр значим — паттерн resolve_file_path_is_absolute из main.rs).
    #[cfg(windows)]
    #[test]
    fn path_matches_case_insensitive_on_windows() {
        let dir = platform_abs("work");
        assert!(path_matches("C:/A/B.png", &dir, Path::new("c:/a/b.png")));
    }

    /// Modify: file не меняется, ThumbStale(index); нода без file (текст) пропускается.
    #[test]
    fn apply_modify_marks_thumb_stale() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        canvas.nodes.push(Node::text("t1", "заметка", 0.0, 200.0));
        let before = canvas.clone();
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Modify(dir.join("sub/f.png"))],
        );
        assert_eq!(changes, vec![NodeChange::ThumbStale(0)]);
        assert_eq!(canvas.nodes[0].file.as_deref(), Some("sub/f.png"));
        assert_eq!(canvas, before);
    }

    /// Modify чужого пути — пустой список изменений.
    #[test]
    fn apply_modify_foreign_path_no_changes() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Modify(dir.join("other/x.png"))],
        );
        assert!(changes.is_empty());
    }

    /// Rename точный: file обновляется (внутри canvas_dir — относительный,
    /// снаружи — абсолютный с `/`), brokenLink снят; round-trip `.canvas`
    /// сохраняет новые пути (паттерн main.rs: to_json + from_str).
    #[test]
    fn apply_rename_exact_updates_file() {
        let dir = platform_abs("work");
        let outside = platform_abs("moved/g.png");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n2", "sub/out.png", 0.0, 150.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[
                FileEvent::Rename(dir.join("sub/f.png"), dir.join("sub/new.png")),
                FileEvent::Rename(dir.join("sub/out.png"), outside.clone()),
            ],
        );
        assert_eq!(
            changes,
            vec![NodeChange::PathUpdated(0), NodeChange::PathUpdated(1)]
        );
        assert_eq!(canvas.nodes[0].file.as_deref(), Some("sub/new.png"));
        assert_eq!(canvas.nodes[0].broken_link, None);
        let outside_str = normalize_path(&outside).to_string_lossy().into_owned();
        assert_eq!(canvas.nodes[1].file.as_deref(), Some(outside_str.as_str()));
        // round-trip: обновлённые пути доезжают через .canvas
        let json = canvas.to_json().expect("сериализация");
        let restored = Canvas::from_str(&json).expect("парсинг");
        assert_eq!(restored.nodes[0].file.as_deref(), Some("sub/new.png"));
        assert_eq!(
            restored.nodes[1].file.as_deref(),
            Some(outside_str.as_str())
        );
    }

    /// Rename каталога: все вложенные file обновляются по префиксу
    /// (регистр узла в суффиксе сохраняется), соседняя папка не тронута.
    #[test]
    fn apply_rename_directory_updates_children() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "a/f1.png", 0.0, 0.0, 100.0, 100.0));
        canvas.nodes.push(Node::file(
            "n2",
            "a/Nested/F2.png",
            0.0,
            150.0,
            100.0,
            100.0,
        ));
        canvas
            .nodes
            .push(Node::file("n3", "b/f3.png", 0.0, 300.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Rename(dir.join("a"), dir.join("c"))],
        );
        assert_eq!(
            changes,
            vec![NodeChange::PathUpdated(0), NodeChange::PathUpdated(1)]
        );
        assert_eq!(canvas.nodes[0].file.as_deref(), Some("c/f1.png"));
        assert_eq!(canvas.nodes[1].file.as_deref(), Some("c/Nested/F2.png"));
        assert_eq!(canvas.nodes[2].file.as_deref(), Some("b/f3.png"));
    }

    /// Rename tmp→file (atomic-save): узел смотрит на `to` — ThumbStale,
    /// путь не меняется.
    #[test]
    fn apply_rename_atomic_save_marks_stale() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "doc.txt", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Rename(
                dir.join("tmp-4f2a.tmp"),
                dir.join("doc.txt"),
            )],
        );
        assert_eq!(changes, vec![NodeChange::ThumbStale(0)]);
        assert_eq!(canvas.nodes[0].file.as_deref(), Some("doc.txt"));
    }

    /// Remove файла: brokenLink ставится, Broken(index) возвращается.
    #[test]
    fn apply_remove_breaks_node() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Remove(dir.join("sub/f.png"))],
        );
        assert_eq!(changes, vec![NodeChange::Broken(0)]);
        assert_eq!(canvas.nodes[0].broken_link, Some(true));
    }

    /// Remove чужого пути — пустой список изменений.
    #[test]
    fn apply_remove_foreign_path_no_changes() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Remove(dir.join("other/x.png"))],
        );
        assert!(changes.is_empty());
        assert_eq!(canvas.nodes[0].broken_link, None);
    }

    /// Повторный Remove того же пути — без второго Broken (флаг уже стоит).
    #[test]
    fn apply_remove_repeat_no_duplicate() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let remove = FileEvent::Remove(dir.join("sub/f.png"));
        let changes = apply_file_events(&mut canvas, &dir, &[remove.clone(), remove]);
        assert_eq!(changes, vec![NodeChange::Broken(0)]);
    }

    /// Remove каталога: вложенные узлы становятся битыми, соседняя папка — нет.
    #[test]
    fn apply_remove_directory_breaks_nested() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "a/x.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n2", "a/sub/y.png", 0.0, 150.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n3", "b/z.png", 0.0, 300.0, 100.0, 100.0));
        let changes = apply_file_events(&mut canvas, &dir, &[FileEvent::Remove(dir.join("a"))]);
        assert_eq!(changes, vec![NodeChange::Broken(0), NodeChange::Broken(1)]);
        assert_eq!(canvas.nodes[0].broken_link, Some(true));
        assert_eq!(canvas.nodes[1].broken_link, Some(true));
        assert_eq!(canvas.nodes[2].broken_link, None);
    }

    /// Remove + Create по тому же пути (восстановление): нода снова живая,
    /// изменения в порядке событий (Broken, затем Restored).
    #[test]
    fn apply_create_restores_broken_node() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[
                FileEvent::Remove(dir.join("sub/f.png")),
                FileEvent::Create(dir.join("sub/f.png")),
            ],
        );
        assert_eq!(
            changes,
            vec![NodeChange::Broken(0), NodeChange::Restored(0)]
        );
        assert_eq!(canvas.nodes[0].broken_link, None);
    }

    /// Create по пути НЕбитой ноды — изменений нет (Create лечит только битые).
    #[test]
    fn apply_create_on_intact_node_no_changes() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Create(dir.join("sub/f.png"))],
        );
        assert!(changes.is_empty());
        assert_eq!(canvas.nodes[0].broken_link, None);
    }

    /// Rename снимает brokenLink: битая нода получает новый путь и живёт.
    #[test]
    fn apply_rename_clears_broken_link() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        let mut node = Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0);
        node.broken_link = Some(true);
        canvas.nodes.push(node);
        let changes = apply_file_events(
            &mut canvas,
            &dir,
            &[FileEvent::Rename(
                dir.join("sub/f.png"),
                dir.join("sub/g.png"),
            )],
        );
        assert_eq!(changes, vec![NodeChange::PathUpdated(0)]);
        assert_eq!(canvas.nodes[0].file.as_deref(), Some("sub/g.png"));
        assert_eq!(canvas.nodes[0].broken_link, None);
    }

    /// Дедуп: два Modify одного пути в одном батче — один ThumbStale.
    #[test]
    fn apply_modify_deduplicated_in_batch() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/f.png", 0.0, 0.0, 100.0, 100.0));
        let modify = FileEvent::Modify(dir.join("sub/f.png"));
        let changes = apply_file_events(&mut canvas, &dir, &[modify.clone(), modify]);
        assert_eq!(changes, vec![NodeChange::ThumbStale(0)]);
    }

    /// relative_if_inside: внутри — относительный с `/` (разделители входа
    /// унифицируются); снаружи — абсолютный с `/`; abs == canvas_dir — `.`.
    #[test]
    fn relative_if_inside_variants() {
        let dir = platform_abs("work");
        assert_eq!(
            relative_if_inside(&dir, &dir.join("sub/f.png")),
            "sub/f.png"
        );
        assert_eq!(
            relative_if_inside(&dir, &dir.join(r"sub\f.png")),
            "sub/f.png"
        );
        let outside = platform_abs("elsewhere/g.png");
        let expected = normalize_path(&outside).to_string_lossy().into_owned();
        assert_eq!(relative_if_inside(&dir, &outside), expected);
        assert_eq!(relative_if_inside(&dir, &dir), ".");
    }

    /// watched_dirs: один каталог на несколько нод (dedup), сортировка
    /// (каталог канваса — префикс, сортируется раньше вложенных), text-ноды
    /// директорий не дают.
    #[test]
    fn watched_dirs_dedup_and_sort() {
        let dir = platform_abs("work");
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::file("n1", "sub/a.png", 0.0, 0.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n2", "sub/b.png", 0.0, 150.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n3", "other/c.png", 0.0, 300.0, 100.0, 100.0));
        canvas
            .nodes
            .push(Node::file("n4", "root.png", 0.0, 450.0, 100.0, 100.0));
        canvas.nodes.push(Node::text("t1", "заметка", 0.0, 600.0));
        assert_eq!(
            watched_dirs(&canvas, &dir),
            vec![
                normalize_path(&dir),
                normalize_path(&dir.join("other")),
                normalize_path(&dir.join("sub")),
            ]
        );
    }

    /// watched_dirs: пустой канвас — пустой список.
    #[test]
    fn watched_dirs_empty_canvas() {
        assert!(watched_dirs(&Canvas::default(), &platform_abs("work")).is_empty());
    }
}
