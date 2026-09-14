//! Permissions манифеста (SPEC §7.6, AGENTS «Правила безопасности и виджетов»):
//! enforcement на КАЖДЫЙ bridge-вызов; `network` — opt-in для внешних
//! WebResourceRequested-запросов.
//!
//! Десериализация строгая: неизвестное строковое значение permissions —
//! ошибка манифеста (пакет не устанавливается, опечатки видны сразу).
//! Forward-compat на уровне ПОЛЕЙ манифеста (unknown_fields — warn).

use crate::bridge::WidgetToHost;
use serde::Deserialize;
use std::collections::BTreeSet;

/// Разрешение из `widget.json` (строковые значения — SPEC §7.6).
/// `CanvasRead` в v1.1 зарезервировано: ни один bridge-метод его не требует
/// (будущие запросы структуры канваса), манифестом принимается.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub enum Permission {
    #[serde(rename = "canvas:read")]
    CanvasRead,
    #[serde(rename = "shell:open")]
    ShellOpen,
    #[serde(rename = "fs:read")]
    FsRead,
    #[serde(rename = "network")]
    Network,
}

impl Permission {
    /// Строковое представление (столбец permissions манифеста, логи, диалоги).
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::CanvasRead => "canvas:read",
            Permission::ShellOpen => "shell:open",
            Permission::FsRead => "fs:read",
            Permission::Network => "network",
        }
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Набор выданных разрешений пакета.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Permissions {
    granted: BTreeSet<Permission>,
}

impl Permissions {
    pub fn new<I: IntoIterator<Item = Permission>>(perms: I) -> Self {
        Self {
            granted: perms.into_iter().collect(),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn allows(&self, perm: Permission) -> bool {
        self.granted.contains(&perm)
    }

    /// Внешние сетевые запросы виджета (WebResourceRequested-фильтр).
    pub fn grants_network(&self) -> bool {
        self.allows(Permission::Network)
    }

    pub fn list(&self) -> Vec<Permission> {
        self.granted.iter().copied().collect()
    }

    /// Проверка bridge-вызова (план M5 §4.6): `Ok` — разрешение не нужно или
    /// выдано; `Err(p)` — вызов блокируется, логируется, виджету возвращается
    /// JSON-RPC-ошибка (для запросов с `id`).
    pub fn check_call(&self, call: &WidgetToHost) -> Result<(), Permission> {
        match required_permission(call) {
            None => Ok(()),
            Some(p) if self.allows(p) => Ok(()),
            Some(p) => Err(p),
        }
    }
}

/// Какое permission требуется методу (таблица плана M5 §4.6).
pub fn required_permission(call: &WidgetToHost) -> Option<Permission> {
    match call {
        WidgetToHost::OpenFile { .. } => Some(Permission::ShellOpen),
        WidgetToHost::ReadDir { .. } => Some(Permission::FsRead),
        // ready/resize/setProps/toast/stateGet/stateSet — своих данных виджета,
        // permission не требуют; network проверяется WebResourceRequested-фильтром
        WidgetToHost::Ready
        | WidgetToHost::Resize { .. }
        | WidgetToHost::SetProps { .. }
        | WidgetToHost::Toast { .. }
        | WidgetToHost::StateGet { .. }
        | WidgetToHost::StateSet { .. } => None,
    }
}

/// Путь запрошенной директории в контексте allowlist (T21-A, П4): `""`/`"."`
/// → корень канваса; относительный — от корня канваса (виджет не знает
/// абсолютных путей — их даёт readDir родителя). Возвращает канонический
/// абсолютный путь, ЕСЛИ он лежит внутри одного из разрешённых корней
/// (папки файловых нод + корень канваса). Чистая функция с fs-доступом
/// только на канонизацию — тестируется на tempdir.
pub fn resolve_fs_request(
    requested: &str,
    canvas_dir: &std::path::Path,
    allowed_roots: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    let requested_path = std::path::Path::new(requested.trim());
    let joined = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        canvas_dir.join(requested_path)
    };
    // Канонизация защищает от «..», симлинков и смешанных разделителей;
    // несуществующий путь — ошибка до сравнения с корнями
    let canonical = joined
        .canonicalize()
        .map_err(|e| format!("путь недоступен: {e}"))?;
    let roots: Vec<std::path::PathBuf> = allowed_roots
        .iter()
        .chain(std::iter::once(&canvas_dir.to_path_buf()))
        .filter_map(|r| r.canonicalize().ok())
        .collect();
    if roots.iter().any(|root| canonical.starts_with(root)) {
        Ok(canonical)
    } else {
        Err("путь вне allowlist (доступны только папки канваса и файловых нод)".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetProps;

    #[test]
    fn enforcement_matrix() {
        let none = Permissions::empty();
        let shell = Permissions::new([Permission::ShellOpen]);
        let fs = Permissions::new([Permission::FsRead]);
        let all = Permissions::new([
            Permission::ShellOpen,
            Permission::FsRead,
            Permission::Network,
            Permission::CanvasRead,
        ]);

        let open = WidgetToHost::OpenFile {
            path: "C:/x.txt".to_owned(),
        };
        let read = WidgetToHost::ReadDir {
            path: "docs".to_owned(),
        };
        let props = WidgetToHost::SetProps {
            props: WidgetProps::new(),
        };
        let state = WidgetToHost::StateSet {
            key: "k".into(),
            value: "v".into(),
        };

        // Без разрешений — openFile/readDir блокируются, остальное проходит
        assert_eq!(none.check_call(&open), Err(Permission::ShellOpen));
        assert_eq!(none.check_call(&read), Err(Permission::FsRead));
        assert_eq!(none.check_call(&props), Ok(()));
        assert_eq!(none.check_call(&state), Ok(()));
        // Выданное разрешение открывает свой метод, не соседний
        assert_eq!(shell.check_call(&open), Ok(()));
        assert_eq!(shell.check_call(&read), Err(Permission::FsRead));
        assert_eq!(fs.check_call(&read), Ok(()));
        assert_eq!(fs.check_call(&open), Err(Permission::ShellOpen));
        assert!(all.check_call(&open).is_ok());
        assert!(all.check_call(&read).is_ok());
    }

    #[test]
    fn network_flag() {
        assert!(!Permissions::empty().grants_network());
        assert!(Permissions::new([Permission::Network]).grants_network());
    }

    #[test]
    fn strict_deserialization_of_values() {
        // Известные значения парсятся (манифест)
        let perms: Vec<Permission> =
            serde_json::from_str(r#"["fs:read","network"]"#).expect("известные значения");
        assert_eq!(perms, vec![Permission::FsRead, Permission::Network]);
        // Опечатка/неизвестное значение — ошибка манифеста, не молчаливый skip
        assert!(serde_json::from_str::<Vec<Permission>>(r#"["netwrok"]"#).is_err());
        assert!(serde_json::from_str::<Vec<Permission>>(r#"["fs:write"]"#).is_err());
    }

    #[test]
    fn required_permission_table() {
        assert_eq!(
            required_permission(&WidgetToHost::OpenFile { path: "x".into() }),
            Some(Permission::ShellOpen)
        );
        assert_eq!(
            required_permission(&WidgetToHost::ReadDir { path: "d".into() }),
            Some(Permission::FsRead)
        );
        assert_eq!(required_permission(&WidgetToHost::Ready), None);
        assert_eq!(
            required_permission(&WidgetToHost::Toast { text: "t".into() }),
            None
        );
    }

    #[test]
    fn fs_allowlist_resolution() {
        let tmp = std::env::temp_dir().join(format!("cd_perms_fs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let canvas = tmp.join("canvas");
        let files = tmp.join("files");
        let outside = tmp.join("outside");
        std::fs::create_dir_all(&canvas).unwrap();
        std::fs::create_dir_all(files.join("sub")).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let roots = vec![files.clone()];

        // "" / "." → корень канваса
        let got = resolve_fs_request("", &canvas, &roots).expect("корень канваса");
        assert_eq!(got, canvas.canonicalize().unwrap());
        assert!(resolve_fs_request(".", &canvas, &roots).is_ok());
        // Относительный путь внутри файлового корня — но резолвится от
        // канваса: папка файловых нод как абсолютный — ок
        let got = resolve_fs_request(&files.join("sub").to_string_lossy(), &canvas, &roots)
            .expect("файловый корень");
        assert!(got.ends_with("sub"));
        // Вне корней — отказ
        let err = resolve_fs_request(&outside.to_string_lossy(), &canvas, &roots)
            .expect_err("вне allowlist");
        assert!(err.contains("allowlist"), "{err}");
        // Traversal «..» наружу — отказ (канонизация уводит за корни)
        let err = resolve_fs_request(
            &canvas.join("../outside").to_string_lossy(),
            &canvas,
            &roots,
        )
        .expect_err("traversal");
        assert!(err.contains("allowlist"), "{err}");
        // Несуществующий путь — отказ с причиной
        assert!(resolve_fs_request("no/such/dir", &canvas, &roots).is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
