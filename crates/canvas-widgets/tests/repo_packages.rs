//! Интеграционные тесты репозитория (T22): синхронизация копий SDK,
//! валидность всех widget.json в дереве репо, бюджет строк SDK и защита
//! от регрессии CSP встроенных виджетов (inline-<script> блокировался
//! собственной CSP — починено миграцией на внешние скрипты в T22).
//!
//! Читают файлы репозитория относительно CARGO_MANIFEST_DIR (крейт лежит
//! в crates/canvas-widgets, корень — на два уровня выше); в CI-матрице
//! тесты выполняются на всех трёх ОС, поэтому сравнения устойчивы к
//! переводам строк (CRLF нормализуется).

use canvas_widgets::manifest::WidgetManifest;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("корень репо")
}

fn read_normalized(path: &Path) -> String {
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("не читается {}: {e}", path.display()));
    // Windows-checkout может дать CRLF — сравниваем содержательно
    text.replace("\r\n", "\n")
}

/// Копии SDK в шаблоне и примерах обязаны быть байт-в-байт текущими
/// (после правки sdk/canvasdesk.ts — скопировать во все места).
#[test]
fn vendored_sdk_ts_copies_match_source() {
    let root = repo_root();
    let source = read_normalized(&root.join("sdk/canvasdesk.ts"));
    let copies = [
        "sdk/widget-template/src/canvasdesk.ts",
        "examples/todo-panel/src/canvasdesk.ts",
        "examples/dashboard/src/canvasdesk.ts",
    ];
    for copy in copies {
        assert_eq!(
            read_normalized(&root.join(copy)),
            source,
            "копия SDK разошлась с sdk/canvasdesk.ts — обновите {copy} (см. sdk/README.md)"
        );
    }
}

/// Встроенные виджеты несут iife-сборку SDK: сверяется с sdk/canvasdesk.js.
#[test]
fn embedded_widgets_ship_current_sdk_build() {
    let root = repo_root();
    let build = read_normalized(&root.join("sdk/canvasdesk.js"));
    let widgets = root.join("assets/widgets");
    let mut checked = 0;
    for entry in fs::read_dir(&widgets).expect("assets/widgets") {
        let dir = entry.expect("entry").path();
        let sdk_copy = dir.join("canvasdesk.js");
        if sdk_copy.is_file() {
            assert_eq!(
                read_normalized(&sdk_copy),
                build,
                "{}: копия canvasdesk.js разошлась с sdk/canvasdesk.js — пересоберите и скопируйте",
                dir.display()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 3,
        "ожидались SDK-копии во встроенных виджетах, найдено {checked}"
    );
}

/// Контракт плана M5 §7 (T22-A): SDK — меньше 150 строк.
#[test]
fn sdk_line_budget() {
    let source = read_normalized(&repo_root().join("sdk/canvasdesk.ts"));
    let lines = source.lines().count();
    assert!(
        lines <= 150,
        "sdk/canvasdesk.ts разросся до {lines} строк (бюджет T22-A — 150)"
    );
}

/// CSP-регрессия: во встроенных виджетах не должно быть inline-<script>
/// (CSP default-src 'self' их блокирует — баг T21, исправлен в T22).
/// Разрешены только внешние <script src="...">.
#[test]
fn builtin_widgets_have_no_inline_scripts() {
    let root = repo_root();
    let widgets = root.join("assets/widgets");
    for entry in fs::read_dir(&widgets).expect("assets/widgets") {
        let dir = entry.expect("entry").path();
        let html_path = dir.join("index.html");
        if !html_path.is_file() {
            continue;
        }
        let html = read_normalized(&html_path);
        assert!(
            html.contains("Content-Security-Policy"),
            "{}: нет CSP-меты",
            html_path.display()
        );
        for line in html.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<script") && !trimmed.contains("src=") {
                panic!(
                    "{}: inline-<script> заблокируется собственной CSP — вынесите в файл ({trimmed})",
                    html_path.display()
                );
            }
        }
        assert!(
            html.contains("canvasdesk.js"),
            "{}: виджет не подключает SDK",
            html_path.display()
        );
    }
}

/// Все манифесты в дереве репо валидны, их entry существует в пакете.
/// Полный список источников пакетов: встроенные + шаблон + примеры.
#[test]
fn repo_widget_manifests_parse_and_entries_exist() {
    let root = repo_root();
    // (путь widget.json, директория, относительно которой живёт entry)
    let mut manifests: Vec<(String, PathBuf)> = Vec::new();

    let widgets = root.join("assets/widgets");
    for entry in fs::read_dir(&widgets).expect("assets/widgets") {
        let dir = entry.expect("entry").path();
        let json_path = dir.join("widget.json");
        if json_path.is_file() {
            manifests.push((fs::read_to_string(&json_path).expect("widget.json"), dir));
        }
    }
    for project in [
        "sdk/widget-template",
        "examples/todo-panel",
        "examples/dashboard",
    ] {
        // Vite-проекты: манифест лежит в public/ (копируется в корень
        // вывода pack), а entry (index.html) — в корне проекта
        let json_path = root.join(project).join("public/widget.json");
        assert!(json_path.is_file(), "нет {project}/public/widget.json");
        manifests.push((
            fs::read_to_string(&json_path).expect("widget.json"),
            root.join(project),
        ));
    }
    assert!(
        manifests.len() >= 6,
        "ожидались 3 встроенных + 3 проекта, есть {}",
        manifests.len()
    );

    for (json, dir) in &manifests {
        let manifest = WidgetManifest::parse(json)
            .unwrap_or_else(|e| panic!("{}: невалидный манифест: {e}", dir.display()));
        let entry = dir.join(manifest.entry.replace('\\', "/"));
        assert!(
            entry.is_file(),
            "{}: entry {} не найден",
            dir.display(),
            manifest.entry
        );
    }
}

/// Vite-проекты шаблона и примеров собираются из index.html в корне
/// проекта (vite build), а base: "./" обязателен для пакета.
#[test]
fn vite_projects_use_relative_base() {
    let root = repo_root();
    for project in [
        "sdk/widget-template",
        "examples/todo-panel",
        "examples/dashboard",
    ] {
        let config = read_normalized(&root.join(project).join("vite.config.ts"));
        assert!(
            config.contains("base: \"./\""),
            "{project}: vite.config.ts без base \"./\" — ассеты соберутся на абсолютные пути и сломаются в виртуальном origin"
        );
        assert!(
            root.join(project).join("index.html").is_file(),
            "{project}: нет index.html — не из чего собирать"
        );
    }
}
