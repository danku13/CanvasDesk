//! FR-108 (мультиканвас C5, issue #9): недавние канвасы нативного
//! приложения — мини-список в `config.toml` ([`crate::settings::Settings::recent`]).
//!
//! Desktop — тонкий слой (решение №20): у натива нет workspace-хранилища
//! (файлами владеет ОС), источником строк менеджера канвасов служит список
//! недавних `.canvas`-файлов. Семантика — зеркало web-недавних (IndexedDB,
//! FR-104): cap, свежайший первым, dedup; протухшие (файла больше нет)
//! записи чистятся при следующем обновлении.
//!
//! Модуль чистый (без I/O — существование файла проверяет колбэк), тестируется
//! нативно и под wasip1; на web не используется (недавние — IndexedDB).

/// Cap списка недавних (решение №20): «мини-список» в конфиге — 10 записей.
pub const RECENT_CAP: usize = 10;

/// Записать путь в недавние: свежайший первым, дубль (тот же путь)
/// переезжает наверх, длина клампится к [`RECENT_CAP`]. Чистая функция —
/// I/O и canonicalization на вызывающем (нативный App).
///
/// Идентичность путей — «как Windows-ФС»: на Windows регистр игнорируется
/// (канонические пути одного файла различаются регистром в зависимости от
/// того, как файл открывали); на Linux/macOS сравнение точное.
pub fn push_recent(list: &[String], path: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(list.len() + 1);
    if !path.is_empty() {
        out.push(path.to_owned());
    }
    for item in list {
        let keep = if out.len() < RECENT_CAP {
            !out.iter().any(|seen: &String| same_path(seen, item))
        } else {
            false
        };
        if keep {
            out.push(item.clone());
        }
    }
    out.truncate(RECENT_CAP);
    out
}

/// Убрать из списка записи, которых больше нет на диске (решение №20:
/// «файлы, которых нет, чистятся при следующем обновлении»). Существование
/// передаётся предикатом без I/O — тесты и вызывающий решают сами.
pub fn decay_missing(list: &[String], exists: impl Fn(&str) -> bool) -> Vec<String> {
    list.iter()
        .filter(|item| exists(item.as_str()))
        .cloned()
        .collect()
}

/// Отображаемое имя канваса из пути: имя файла без хвостового `.canvas`
/// (семантика [`crate::workspace::display_name`], но на входе путь).
/// «/home/x/Notes.canvas» → «Notes»; без расширения — имя файла как есть.
/// Разбор — вручную по последнему разделителю (`/` и `\\`, оба валидны на
/// Windows): `std::path::Path` платформозависим (на Linux бэкслэш — не
/// разделитель), а функция чистая и обязана вести себя одинаково на всех
/// ОС (тесты на Linux проверяют в том числе Windows-пути).
pub fn display_name_of(path: &str) -> String {
    let name = file_name_of(path);
    if name.is_empty() {
        return path.to_owned();
    }
    crate::workspace::display_name(&name).to_owned()
}

/// Имя файла (с расширением) из пути — канонический идентификатор записи
/// менеджера (контракт C0: имя = имя файла). Пустой результат (нет имени
/// файла — корень/«..») — пустая строка, вызывающий отбрасывает запись.
/// Разбор вручную по последнему `/`/`\\` — см. [`display_name_of`].
pub fn file_name_of(path: &str) -> String {
    match path.rsplit(['/', '\\']).next().unwrap_or_default() {
        // Компоненты без имени файла (корень/текущий/родительский каталог)
        // — пустой результат, вызывающий отбрасывает запись.
        "" | "." | ".." => String::new(),
        name => name.to_owned(),
    }
}

/// Сравнение путей на идентичность: на Windows — регистронезависимо
/// (семантика NTFS), иначе точное совпадение. Dedup [`push_recent`] не
/// должен плодить дубли «notes»/«Notes» одного файла.
fn same_path(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

// ============================================================================
// Чистые тесты (натив + wasip1)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Свежайший первым: push кладёт путь в голову списка.
    #[test]
    fn push_puts_newest_first() {
        let list = vec!["a.canvas".to_owned(), "b.canvas".to_owned()];
        let out = push_recent(&list, "c.canvas");
        assert_eq!(out, vec!["c.canvas", "a.canvas", "b.canvas"]);
    }

    /// Дубль переезжает наверх (порядок остальных сохраняется).
    #[test]
    fn push_moves_duplicate_to_front() {
        let list = vec!["a.canvas".to_owned(), "b.canvas".to_owned()];
        let out = push_recent(&list, "a.canvas");
        assert_eq!(out, vec!["a.canvas", "b.canvas"]);
    }

    /// Cap: список не длиннее RECENT_CAP, вытесняется хвост (самое старое).
    #[test]
    fn push_caps_to_recent_cap() {
        let list: Vec<String> = (0..RECENT_CAP).map(|i| format!("f{i}.canvas")).collect();
        let out = push_recent(&list, "new.canvas");
        assert_eq!(out.len(), RECENT_CAP);
        assert_eq!(out[0], "new.canvas");
        // хвост (f9 — самый старый) вытеснен, f0..f8 на месте
        assert!(!out.contains(&"f9.canvas".to_owned()));
        assert!(out.contains(&"f0.canvas".to_owned()));
    }

    /// Пустой список → единственный путь; пустой путь ничего не добавляет.
    #[test]
    fn push_empty_list_and_empty_path() {
        assert_eq!(push_recent(&[], "x.canvas"), vec!["x.canvas"]);
        let out = push_recent(&["a.canvas".to_owned()], "");
        assert_eq!(out, vec!["a.canvas"]);
    }

    /// Протухшие записи чистятся decay_missing (предикат без I/O).
    #[test]
    fn decay_drops_missing_files() {
        let list = vec![
            "есть.canvas".to_owned(),
            "нет.canvas".to_owned(),
            "тоже-есть.canvas".to_owned(),
        ];
        let out = decay_missing(&list, |p| p != "нет.canvas");
        assert_eq!(out, vec!["есть.canvas", "тоже-есть.canvas"]);
    }

    /// Отображаемое имя: file_stem без `.canvas`; без расширения — имя
    /// файла как есть (семантика workspace::display_name).
    #[test]
    fn display_name_of_strips_extension() {
        assert_eq!(display_name_of("/home/x/Notes.canvas"), "Notes");
        assert_eq!(display_name_of("C:\\Docs\\заметки.canvas"), "заметки");
        assert_eq!(display_name_of("/tmp/plain"), "plain");
        // Путь без компоненты имени — деградация в сам путь
        assert_eq!(display_name_of(""), "");
    }

    /// Имя файла с расширением — идентификатор записи менеджера (C0).
    #[test]
    fn file_name_of_keeps_extension() {
        assert_eq!(file_name_of("/a/b/Notes.canvas"), "Notes.canvas");
        assert_eq!(file_name_of("Notes.canvas"), "Notes.canvas");
        // корень/родителя без имени — пусто (запись отбрасывается)
        assert_eq!(file_name_of("/"), "");
        assert_eq!(file_name_of(".."), "");
    }
}
