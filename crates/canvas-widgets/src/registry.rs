//! Реестр пакетов виджетов (план M5 §4.8): скан `~/.canvasdesk/widgets`,
//! установка drag-пакета, обновление, удаление, встроенные пакеты
//! (встроены в бинарник, материализуются при старте, tombstone уважает
//! ручное удаление). ФС-операции через std::fs — тестируются на tempdir.
//!
//! M8/W11 (wasm-port §5): режим «память» (без ФС) — встроенные пакеты
//! обслуживаются прямо из include_dir-статики, установка из папки
//! недоступна, tombstone — в памяти (экспорт/импорт для сохранения — на
//! платформенном слое, web: localStorage). Решение W11 «OPFS или память» —
//! память: пакетные файлы (index.html и т.п.) в волне 1 никто не читает
//! (live-хост отсутствует), а манифесты нужны меню/LOD/permissions.

use crate::manifest::{ManifestError, WidgetManifest};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Встроенные пакеты: вшиваются в бинарник canvas-widgets
/// (assets/widgets в корне репо; для правки исходников — там же).
static EMBEDDED_WIDGETS: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/widgets");

/// Лимиты пакета при установке: защита от «пакета» в виде домашней папки.
pub const MAX_PACKAGE_FILES: usize = 1000;
pub const MAX_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;

/// Ошибка реестра.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("манифест пакета невалиден: {0}")]
    Manifest(#[from] ManifestError),
    #[error("папка пакета не читается: {0}")]
    Io(#[from] std::io::Error),
    #[error("источник не является папкой")]
    NotADirectory,
    #[error("пакет превышает лимиты: {files} файлов / {bytes} байт")]
    TooLarge { files: usize, bytes: u64 },
    #[error("пакет `{0}` не установлен")]
    NotInstalled(String),
    /// Операция требует файловой системы (установка из папки в режиме
    /// «память» — web W11: браузер не даёт папок; сценарий — вне волны 1).
    #[error("операция недоступна без файловой системы")]
    NoFileSystem,
}

/// Результат установки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallOutcome {
    /// Пакет новый: скопирован, ноду можно ставить.
    Installed,
    /// Версия обновлена: файлы заменены, ноды/props уцелели.
    Updated,
    /// Та же версия уже стоит: действий не требуется.
    SameVersion,
}

/// Установленный пакет.
#[derive(Debug, Clone, PartialEq)]
pub struct InstalledWidget {
    pub manifest: WidgetManifest,
    /// Папка пакета (`root/<id>`).
    pub dir: PathBuf,
    /// Пакет встроенный (материализован из бинарника).
    pub builtin: bool,
}

/// Хранилище реестра: файловая система (натив) или память (web W11 +
/// тесты без tempdir). Выбор — в конструкторе, без cfg: обе ветки
/// компилируются и тестируются на любой ОС.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Store {
    Fs,
    Memory,
}

/// Реестр над корнем `~/.canvasdesk/widgets` (или над памятью — web W11).
#[derive(Debug, Clone)]
pub struct WidgetRegistry {
    root: PathBuf,
    installed: BTreeMap<String, InstalledWidget>,
    store: Store,
    /// Tombstone-набор режима «память»: удалённые встроенные пакеты
    /// (id → версия встроенного на момент удаления). Персистентность —
    /// на вызывающем ([`Self::memory_tombstones`]/[`Self::set_memory_tombstones`]).
    memory_tombstones: BTreeMap<String, [u32; 3]>,
}

impl WidgetRegistry {
    /// Новый реестр над ФС; скан делается отдельно (`reload`).
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            installed: BTreeMap::new(),
            store: Store::Fs,
            memory_tombstones: BTreeMap::new(),
        }
    }

    /// Реестр без файловой системы (web W11): встроенные пакеты — из
    /// include_dir-статики, `root` — условная метка для логов/диагностики.
    pub fn in_memory() -> Self {
        Self {
            root: PathBuf::from("widgets:memory"),
            installed: BTreeMap::new(),
            store: Store::Memory,
            memory_tombstones: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Скан корня: каждая подпапка с валидным `widget.json` — пакет;
    /// невалидные — warn и пропуск (сбойные пакеты не ломают реестр).
    /// Память: источник истины — встроенная статика (не-tombstone).
    pub fn reload(&mut self) -> Result<(), RegistryError> {
        if self.store == Store::Memory {
            self.installed.clear();
            for pkg in EMBEDDED_WIDGETS.dirs() {
                let Some(manifest) = embedded_manifest(pkg) else {
                    continue; // warn — в materialize_builtins
                };
                if self
                    .memory_tombstones
                    .get(&manifest.id)
                    .is_some_and(|t| *t >= manifest.version_tuple())
                {
                    continue;
                }
                self.installed.insert(
                    manifest.id.clone(),
                    InstalledWidget {
                        manifest,
                        dir: builtin_memory_dir(pkg.path()),
                        builtin: true,
                    },
                );
            }
            return Ok(());
        }
        self.installed.clear();
        let entries = match std::fs::read_dir(&self.root) {
            Ok(entries) => entries,
            // Нет корня — нет пакетов (не ошибка: первый запуск)
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            match WidgetManifest::from_dir(&dir) {
                Ok(manifest) => {
                    let builtin = self.is_builtin(&manifest.id);
                    self.installed.insert(
                        manifest.id.clone(),
                        InstalledWidget {
                            manifest,
                            dir,
                            builtin,
                        },
                    );
                }
                Err(e) => {
                    tracing::warn!(?dir, error = %e, "widget-пакет пропущен: невалидный манифест");
                }
            }
        }
        Ok(())
    }

    /// Все установленные пакеты (по id).
    pub fn installed(&self) -> Vec<&InstalledWidget> {
        self.installed.values().collect()
    }

    pub fn get(&self, id: &str) -> Option<&InstalledWidget> {
        self.installed.get(id)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.installed.contains_key(id)
    }

    /// Установка пакета из папки-источника (drag из Explorer): валидация
    /// манифеста и лимитов, копирование в `root/<id>`. Та же версия —
    /// `SameVersion` (no-op); папка с битым манифестом перезаписывается.
    /// Память: папок нет — честная ошибка (NoFileSystem).
    pub fn install(&mut self, src: &Path) -> Result<InstallOutcome, RegistryError> {
        if self.store == Store::Memory {
            return Err(RegistryError::NoFileSystem);
        }
        if !src.is_dir() {
            return Err(RegistryError::NotADirectory);
        }
        let manifest = WidgetManifest::from_dir(src)?;
        std::fs::create_dir_all(&self.root)?;
        let dest = self.root.join(&manifest.id);

        let existing_version = WidgetManifest::from_dir(&dest)
            .ok()
            .map(|m| m.version_tuple());
        let outcome = match existing_version {
            Some(v) if v == manifest.version_tuple() => return Ok(InstallOutcome::SameVersion),
            Some(_) => InstallOutcome::Updated,
            None => InstallOutcome::Installed,
        };
        if dest.exists() {
            remove_package_dir(&dest)?;
        }
        copy_package(src, &dest)?;
        self.uptake(&dest, false);
        // Установка/обновление снимает tombstone (встроенный снова материализуем)
        self.clear_tombstone(&manifest.id);
        Ok(outcome)
    }

    /// Удаление пакета (ноды становятся «битыми» — это делает приложение).
    /// Ставит tombstone, чтобы встроенный пакет не воскрес на перезапуске.
    /// Память: из map + tombstone в памяти (файлов нет — persist на
    /// платформенном слое).
    pub fn remove(&mut self, id: &str) -> Result<(), RegistryError> {
        let dir = self
            .installed
            .get(id)
            .map(|w| w.dir.clone())
            .ok_or_else(|| RegistryError::NotInstalled(id.to_owned()))?;
        if self.store == Store::Memory {
            if let Some(version) = Self::builtin_manifests()
                .into_iter()
                .find(|m| m.id == id)
                .map(|m| m.version_tuple())
            {
                self.memory_tombstones.insert(id.to_owned(), version);
            }
            self.installed.remove(id);
            return Ok(());
        }
        remove_package_dir(&dir)?;
        if self.is_builtin(id) {
            let version = self
                .installed
                .get(id)
                .map(|w| w.manifest.version.clone())
                .unwrap_or_default();
            self.write_tombstone(id, &version);
        }
        self.installed.remove(id);
        Ok(())
    }

    /// Материализация встроенных пакетов при старте (план M5 §4.8):
    /// нет папки и нет tombstone → установить; версия встроенной выше
    /// установленной/tombstone → обновить; иначе не трогать. Память:
    /// «установить» = запись в map (файлов нет, dir — виртуальная метка).
    pub fn materialize_builtins(&mut self) -> Result<(), RegistryError> {
        if self.store == Store::Memory {
            for pkg in EMBEDDED_WIDGETS.dirs() {
                let Some(manifest) = embedded_manifest(pkg) else {
                    tracing::warn!(dir = ?pkg.path(), "встроенный пакет без валидного манифеста");
                    continue;
                };
                let builtin_v = manifest.version_tuple();
                let up_to_date = self
                    .installed
                    .get(&manifest.id)
                    .is_some_and(|w| w.manifest.version_tuple() >= builtin_v);
                let suppressed = self
                    .memory_tombstones
                    .get(&manifest.id)
                    .is_some_and(|t| *t >= builtin_v);
                if up_to_date || suppressed {
                    continue;
                }
                // Снятие tombstone до переезда manifest в InstalledWidget
                self.memory_tombstones.remove(&manifest.id);
                tracing::info!(id = %manifest.id, "встроенный виджет зарегистрирован в памяти");
                self.installed.insert(
                    manifest.id.clone(),
                    InstalledWidget {
                        manifest,
                        dir: builtin_memory_dir(pkg.path()),
                        builtin: true,
                    },
                );
            }
            return Ok(());
        }
        std::fs::create_dir_all(&self.root)?;
        for pkg in EMBEDDED_WIDGETS.dirs() {
            let Some(manifest) = embedded_manifest(pkg) else {
                tracing::warn!(dir = ?pkg.path(), "встроенный пакет без валидного манифеста");
                continue;
            };
            let dest = self.root.join(&manifest.id);
            let installed = WidgetManifest::from_dir(&dest).ok();
            let installed_version = installed.as_ref().map(|m| m.version_tuple());
            let tombstone_version = self.tombstone_version(&manifest.id);

            let builtin_v = manifest.version_tuple();
            let up_to_date = installed_version.is_some_and(|v| v >= builtin_v);
            let suppressed = tombstone_version.is_some_and(|t| t >= builtin_v);
            if up_to_date || suppressed {
                continue;
            }
            // Материализация: папки нет ИЛИ версия ниже — пишем файлы пакета
            let files_ok = write_embedded_package(pkg, &dest)?;
            debug_assert!(files_ok);
            tracing::info!(id = %manifest.id, version = %manifest.version, "встроенный виджет материализован");
            self.clear_tombstone(&manifest.id);
        }
        Ok(())
    }

    /// Список встроенных манифестов (для меню до материализации).
    pub fn builtin_manifests() -> Vec<WidgetManifest> {
        EMBEDDED_WIDGETS
            .dirs()
            .filter_map(embedded_manifest)
            .collect()
    }

    /// Tombstone-набор режима «память» (id, версия «1.2.0») — для
    /// сохранения платформенным слоем (web: localStorage, W11). ФС-режим —
    /// пустой список (tombstone живут файлами в root/.deleted).
    pub fn memory_tombstones(&self) -> Vec<(String, String)> {
        self.memory_tombstones
            .iter()
            .map(|(id, v)| (id.clone(), format_version(*v)))
            .collect()
    }

    /// Восстановить tombstone-набор режима «память» (до инициализации:
    /// `materialize_builtins`/`reload` уважают восстановленное). ФС-режим —
    /// no-op (tombstone читаются с диска сами).
    pub fn set_memory_tombstones(&mut self, entries: &[(String, String)]) {
        if self.store != Store::Memory {
            return;
        }
        self.memory_tombstones = entries
            .iter()
            .map(|(id, version)| (id.clone(), parse_version_line(version)))
            .collect();
    }

    fn is_builtin(&self, id: &str) -> bool {
        EMBEDDED_WIDGETS
            .dirs()
            .filter_map(embedded_manifest)
            .any(|m| m.id == id)
    }

    fn uptake(&mut self, dir: &Path, builtin: bool) {
        if let Ok(manifest) = WidgetManifest::from_dir(dir) {
            self.installed.insert(
                manifest.id.clone(),
                InstalledWidget {
                    manifest,
                    dir: dir.to_owned(),
                    builtin,
                },
            );
        }
    }

    fn tombstone_path(&self, id: &str) -> PathBuf {
        self.root.join(".deleted").join(id)
    }

    fn tombstone_version(&self, id: &str) -> Option<[u32; 3]> {
        let text = std::fs::read_to_string(self.tombstone_path(id)).ok()?;
        Some(parse_version_line(&text))
    }

    fn write_tombstone(&self, id: &str, version: &str) {
        let path = self.tombstone_path(id);
        if std::fs::create_dir_all(path.parent().unwrap_or(&self.root)).is_ok() {
            // Пишем текущую версию встроенного пакета, не установленного
            let builtin_v = Self::builtin_manifests()
                .into_iter()
                .find(|m| m.id == id)
                .map(|m| m.version)
                .unwrap_or_else(|| version.to_owned());
            let _ = std::fs::write(path, builtin_v);
        }
    }

    fn clear_tombstone(&self, id: &str) {
        let _ = std::fs::remove_file(self.tombstone_path(id));
    }
}

/// Манифест встроенного пакета: include_dir 0.7 хранит пути детей с префиксом
/// корня статики (`clock/widget.json`), поэтому `pkg.get_file("widget.json")`
/// не резолвится — ищем по file_name + родителю (корень пакета).
fn embedded_manifest(pkg: &include_dir::Dir<'_>) -> Option<WidgetManifest> {
    let file = pkg.files().find(|f| {
        f.path().file_name().is_some_and(|n| n == "widget.json")
            && f.path().parent() == Some(pkg.path())
    })?;
    WidgetManifest::parse(file.contents_utf8()?).ok()
}

/// tombstone-файл: одна строка — версия, «[1, 2, 0]» или «1.2.0».
fn parse_version_line(text: &str) -> [u32; 3] {
    let trimmed = text.trim().trim_matches(|c| c == '[' || c == ']');
    let mut out = [0, 0, 0];
    for (slot, part) in trimmed.split(&[',', '.'][..]).take(3).enumerate() {
        let digits: String = part
            .trim()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        out[slot] = digits.parse().unwrap_or(0);
    }
    out
}

/// Версия `[u32; 3]` → строка «1.2.0» (экспорт tombstone режима «память»).
fn format_version(v: [u32; 3]) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

/// Виртуальная папка встроенного пакета в режиме «память»: ясно помечена
/// как встроенная, файлов под ней нет (live-хост в волне 1 отсутствует —
/// dir читает только диагностика/логи).
fn builtin_memory_dir(pkg_path: &Path) -> PathBuf {
    let id = pkg_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    PathBuf::from(format!("/builtin/{id}"))
}

/// Копирование пакета с лимитами и пропуском симлинков (безопасность).
fn copy_package(src: &Path, dest: &Path) -> Result<(), RegistryError> {
    let (files, bytes) = package_stats(src)?;
    if files > MAX_PACKAGE_FILES || bytes > MAX_PACKAGE_BYTES {
        return Err(RegistryError::TooLarge { files, bytes });
    }
    copy_tree(src, dest)
}

/// Сбор статистики пакета (файлы/байты) — симлинки не считаем.
fn package_stats(dir: &Path) -> Result<(usize, u64), RegistryError> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut stack = vec![dir.to_owned()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                continue; // симлинки в пакете не копируем и не считаем
            }
            if meta.is_dir() {
                stack.push(path);
            } else {
                files += 1;
                bytes += meta.len();
            }
        }
    }
    Ok((files, bytes))
}

fn copy_tree(src: &Path, dest: &Path) -> Result<(), RegistryError> {
    std::fs::create_dir_all(dest)?;
    let mut stack = vec![src.to_owned()];
    let mut dest_stack = vec![dest.to_owned()];
    while let Some(current) = stack.pop() {
        let current_dest = dest_stack.pop().expect("стеки синхронны");
        for entry in std::fs::read_dir(&current)? {
            let entry = entry?;
            let path = entry.path();
            let target = current_dest.join(entry.file_name());
            let meta = std::fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                std::fs::create_dir_all(&target)?;
                stack.push(path);
                dest_stack.push(target);
            } else {
                std::fs::copy(&path, &target)?;
            }
        }
    }
    Ok(())
}

/// Запись встроенного пакета на диск (материализация). Пути файлов в
/// include_dir 0.7 — от корня СТАТИКИ («clock/widget.json» для пакета
/// «clock»), поэтому отрезаем префикс папки пакета и пишем от его корня.
fn write_embedded_package(pkg: &include_dir::Dir<'_>, dest: &Path) -> Result<bool, RegistryError> {
    if dest.exists() {
        remove_package_dir(dest)?;
    }
    std::fs::create_dir_all(dest)?;
    let mut count = 0usize;
    write_dir_entries(pkg, pkg.path(), dest, &mut count)?;
    Ok(count > 0)
}

fn write_dir_entries(
    dir: &include_dir::Dir<'_>,
    pkg_prefix: &Path,
    dest: &Path,
    count: &mut usize,
) -> Result<(), RegistryError> {
    for entry in dir.entries() {
        match entry {
            include_dir::DirEntry::File(file) => {
                let rel = file.path().strip_prefix(pkg_prefix).unwrap_or(file.path());
                let target = dest.join(rel);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, file.contents())?;
                *count += 1;
            }
            include_dir::DirEntry::Dir(sub) => {
                write_dir_entries(sub, pkg_prefix, dest, count)?;
            }
        }
    }
    Ok(())
}

/// Удаление папки пакета с защитой от выхода за корень (служебные имена
/// `.deleted` и т.п. удалять через этот путь нельзя).
fn remove_package_dir(dir: &Path) -> Result<(), RegistryError> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let unsafe_name = name.is_empty()
        || name == "."
        || name == ".."
        || (name.starts_with('.') && name != ".deleted");
    if unsafe_name {
        return Err(RegistryError::NotADirectory);
    }
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("canvasdesk_registry_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("tempdir");
        dir
    }

    /// Пакет-источник для установки (drag).
    fn make_package(dir: &Path, id: &str, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("widget.json"),
            format!(
                r#"{{"id": "{id}", "name": "T", "version": "{version}", "entry": "index.html"}}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.join("index.html"), "<p>ok</p>").unwrap();
    }

    #[test]
    fn install_scan_remove_lifecycle() {
        let root = temp_root("lifecycle");
        let src = root.join("_src").join("clock");
        make_package(&src, "com.test.clock", "1.0.0");

        let mut reg = WidgetRegistry::new(root.join("widgets"));
        reg.reload().expect("пустой скан");
        assert!(reg.installed().is_empty(), "пакетов ещё нет");

        let outcome = reg.install(&src).expect("установка");
        assert_eq!(outcome, InstallOutcome::Installed);
        assert!(reg.contains("com.test.clock"));
        let installed = reg.get("com.test.clock").expect("пакет");
        assert_eq!(installed.manifest.version, "1.0.0");
        assert!(
            installed.dir.join("index.html").exists(),
            "файлы скопированы"
        );

        // Повторная установка той же версии — no-op
        assert_eq!(
            reg.install(&src).expect("та же версия"),
            InstallOutcome::SameVersion
        );

        // Обновление до 1.1.0 — файлы заменены
        let src2 = root.join("_src2").join("clock");
        make_package(&src2, "com.test.clock", "1.1.0");
        std::fs::write(src2.join("index.html"), "<p>new</p>").unwrap();
        assert_eq!(
            reg.install(&src2).expect("обновление"),
            InstallOutcome::Updated
        );
        assert_eq!(reg.get("com.test.clock").unwrap().manifest.version, "1.1.0");
        assert!(
            std::fs::read_to_string(reg.get("com.test.clock").unwrap().dir.join("index.html"))
                .unwrap()
                .contains("new")
        );

        // Скан подхватывает пакеты с диска (перезапуск приложения)
        let mut reg2 = WidgetRegistry::new(root.join("widgets"));
        reg2.reload().expect("скан после установки");
        assert!(reg2.contains("com.test.clock"));

        // Удаление: папки нет, реестр чист
        reg.remove("com.test.clock").expect("удаление");
        assert!(!reg.contains("com.test.clock"));
        assert!(!reg
            .get("com.test.clock")
            .map(|w| w.dir.exists())
            .unwrap_or(false));
        assert!(
            reg.remove("com.test.clock").is_err(),
            "второе удаление — ошибка"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_manifest_rejected_nothing_copied() {
        let root = temp_root("invalid");
        let src = root.join("_src").join("bad");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("widget.json"), r#"{"id": "BAD!", "name": ""}"#).unwrap();

        let mut reg = WidgetRegistry::new(root.join("widgets"));
        assert!(reg.install(&src).is_err(), "невалидный манифест отвергнут");
        assert!(reg.installed().is_empty());
        assert!(
            !root.join("widgets").join("BAD!").exists(),
            "ничего не скопировано"
        );

        // Папка-источник без widget.json — ошибка манифеста
        let noscript = root.join("_src").join("empty");
        std::fs::create_dir_all(&noscript).unwrap();
        assert!(reg.install(&noscript).is_err());
        assert!(
            reg.install(&root.join("_no_such_dir")).is_err(),
            "нет папки"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn symlinks_skipped_and_limits_enforced() {
        let root = temp_root("limits");
        let src = root.join("_src").join("pkg");
        make_package(&src, "com.test.big", "1.0.0");
        // Симлинк-файл — не копируется
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hostname", src.join("link.txt")).unwrap();

        let mut reg = WidgetRegistry::new(root.join("widgets"));
        reg.install(&src).expect("пакет с симлинком ставится");
        assert!(
            !reg.get("com.test.big")
                .unwrap()
                .dir
                .join("link.txt")
                .exists(),
            "симлинк не скопирован"
        );
        assert!(reg
            .get("com.test.big")
            .unwrap()
            .dir
            .join("index.html")
            .exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn builtin_materialize_respects_tombstone_and_versions() {
        let root = temp_root("builtin");
        let mut reg = WidgetRegistry::new(root.join("widgets"));

        // Материализация: встроенные пакеты появились
        reg.materialize_builtins().expect("материализация");
        reg.reload().expect("скан");
        let builtins: Vec<String> = WidgetRegistry::builtin_manifests()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(!builtins.is_empty(), "в репо есть встроенные пакеты");
        for id in &builtins {
            assert!(reg.contains(id), "встроенный {id} материализован");
            assert!(reg.get(id).unwrap().builtin, "помечен как встроенный");
        }
        // Идемпотентность: повторный вызов не ломает
        reg.materialize_builtins()
            .expect("повторная материализация");

        // Удаление встроенного → tombstone: повторная материализация не воскрешает
        let first = builtins[0].clone();
        reg.remove(&first).expect("удаление встроенного");
        assert!(!reg.contains(&first));
        reg.materialize_builtins()
            .expect("материализация после удаления");
        reg.reload().expect("скан");
        assert!(!reg.contains(&first), "tombstone удерживает удаление");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn builtin_list_has_clock() {
        // T20-C: часы — обязательный встроенный пакет (демо)
        let ids: Vec<String> = WidgetRegistry::builtin_manifests()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(
            ids.iter().any(|id| id.contains("clock")),
            "встроенные: {ids:?}"
        );
    }

    #[test]
    fn version_line_parsing() {
        assert_eq!(parse_version_line("1.2.0"), [1, 2, 0]);
        assert_eq!(parse_version_line("[1, 2, 0]"), [1, 2, 0]);
        assert_eq!(parse_version_line(" 2.5.1-beta\n "), [2, 5, 1]);
        assert_eq!(parse_version_line("мусор"), [0, 0, 0]);
    }

    // ==========================================================================
    // M8/W11: режим «память» (web) — без ФС, встроенные пакеты из статики
    // ==========================================================================

    #[test]
    fn memory_mode_materializes_and_lists_builtins() {
        let mut reg = WidgetRegistry::in_memory();
        reg.materialize_builtins().expect("материализация в памяти");
        reg.reload().expect("reload в памяти");

        let builtins: Vec<String> = WidgetRegistry::builtin_manifests()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(!builtins.is_empty(), "в репо есть встроенные пакеты");
        for id in &builtins {
            let pkg = reg.get(id).expect("встроенный зарегистрирован");
            assert!(pkg.builtin, "{id} помечен встроенным");
            assert!(
                pkg.dir.starts_with("/builtin/"),
                "виртуальная папка: {}",
                pkg.dir.display()
            );
        }
        // Идемпотентность: повторный вызов не дублирует и не ломает
        reg.materialize_builtins()
            .expect("повторная материализация");
        assert_eq!(reg.installed().len(), builtins.len());
    }

    #[test]
    fn memory_mode_remove_tombstones_and_restore() {
        let mut reg = WidgetRegistry::in_memory();
        reg.materialize_builtins().expect("материализация");
        let first = WidgetRegistry::builtin_manifests()[0].id.clone();

        reg.remove(&first).expect("удаление в памяти");
        assert!(!reg.contains(&first));

        // Tombstone в памяти удерживает повторную материализацию/reload
        reg.materialize_builtins()
            .expect("материализация после удаления");
        reg.reload().expect("reload после удаления");
        assert!(!reg.contains(&first), "tombstone удерживает удаление");

        // Экспорт tombstone: (id, «1.2.0»)-формат, валидный для импорта
        let exported = reg.memory_tombstones();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].0, first);
        assert!(
            exported[0]
                .1
                .split('.')
                .all(|part| part.parse::<u32>().is_ok()),
            "версия вида 1.2.0: {}",
            exported[0].1
        );

        // Перезапуск страницы: новый реестр + импорт tombstone до init —
        // удалённый встроенный пакет не возвращается
        let mut reg2 = WidgetRegistry::in_memory();
        reg2.set_memory_tombstones(&exported);
        reg2.materialize_builtins()
            .expect("материализация с tombstone");
        assert!(!reg2.contains(&first));
        // Остальные встроенные — на месте
        for m in WidgetRegistry::builtin_manifests() {
            if m.id != first {
                assert!(reg2.contains(&m.id), "{} на месте", m.id);
            }
        }

        // ФС-режим: импорт tombstone — no-op (tombstone живут файлами)
        let mut fs_reg = WidgetRegistry::new(std::env::temp_dir().join("canvasdesk_w11_fs_reg"));
        fs_reg.set_memory_tombstones(&exported);
        assert!(fs_reg.memory_tombstones().is_empty());
    }

    #[test]
    fn memory_mode_install_from_folder_is_rejected() {
        let mut reg = WidgetRegistry::in_memory();
        let src = std::env::temp_dir().join(format!("canvasdesk_w11_src_{}", std::process::id()));
        std::fs::create_dir_all(&src).unwrap();
        let err = reg.install(&src).expect_err("папок на web нет");
        assert!(matches!(err, RegistryError::NoFileSystem), "{err}");
        // Несуществующий id — как на ФС: NotInstalled
        assert!(matches!(
            reg.remove("no.such.package"),
            Err(RegistryError::NotInstalled(_))
        ));
        let _ = std::fs::remove_dir_all(&src);
    }
}
