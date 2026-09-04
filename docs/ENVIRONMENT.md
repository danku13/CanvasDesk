# Настройка окружения разработки CanvasDesk

Пошаговая инструкция: что установить на чистую Windows-машину, чтобы начать
разработку (задача T0 из `docs/TASKS.md` и далее).

Проверено на: Windows 11 25H2 (build 26200) — целевая платформа проекта.

---

## 1. Состояние текущей машины (проверено)

| Компонент | Статус |
|---|---|
| Windows 11 25H2 (build 26200) | есть — целевая ОС |
| Git / Git Bash | установлен |
| VS Code | установлен |
| winget | доступен |
| Rust (rustup/cargo/rustc) | **отсутствует** |
| MSVC Build Tools / Visual Studio | **отсутствует** |

Особенность: `link.exe`, который находится в Git Bash — это GNU-утилита из
поставки Git, а не MSVC linker. Для сборки Rust на таргете
`x86_64-pc-windows-msvc` нужен настоящий MSVC linker из Build Tools.

WebView2 Runtime предустановлен в Windows 11 — отдельно ставить не нужно
(потребуется в M5, задачи T20–T22).

## 2. Требования по документации проекта

- `docs/SPEC.md` §3: Rust **stable 1.80+**, edition 2021, реализация только
  Windows 10/11 x64 → тулчейн `x86_64-pc-windows-msvc`.
- `docs/AGENTS.md` «Сборка и тесты»: после T0 обязаны работать
  `cargo build --workspace`, `cargo test --workspace`,
  `cargo clippy --workspace -- -D warnings`, `cargo fmt --check` → нужны
  clippy и rustfmt (входят в default-профиль rustup).
- `docs/TASKS.md` «Порядок старта» п.2: `rustup default stable` +
  Windows SDK (через Visual Studio Build Tools).

## 3. Установка

Команды выполняются в PowerShell (обычном, не обязательно от администратора —
winget сам запросит UAC).

### Шаг 1. MSVC Build Tools (обязательно)

Rust на MSVC-таргете линкуется через `link.exe` и использует Windows SDK
(нужны windows-rs, wgpu/DX12, COM в `canvas-shell`):

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Это workload «Desktop development with C++» (~3–6 ГБ): MSVC v143,
Windows 11 SDK. Полная Visual Studio Community **не нужна** — разработка
ведётся в VS Code.

### Шаг 2. Rust toolchain (обязательно)

```powershell
winget install --id Rustlang.Rustup
```

Затем **в новом терминале** (обновление PATH):

```powershell
rustup default stable
rustup target add x86_64-pc-windows-msvc
```

Альтернатива: скачать `rustup-init.exe` с https://rustup.rs — эквивалентно.

### Шаг 3. rust-analyzer в VS Code (рекомендуется)

```powershell
code --install-extension rust-lang.rust-analyzer
```

## 4. Проверка установки (критерий готовности к T0)

В новом терминале:

```powershell
rustc --version     # stable >= 1.80
cargo --version
cargo clippy --version
cargo fmt --version
```

Смоук-тест компиляции (проверяет MSVC linker и Windows SDK):

```powershell
cargo new hello
cd hello
cargo run           # бинарь собрался и вывел "Hello, world!"
```

После выполнения T0 дополнительно: `cargo build --workspace`,
`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
`cargo fmt --check` — все зелёные.

## 5. Замечания

- После установки Build Tools и rustup терминал нужно перезапустить —
  иначе `link.exe`/`cargo` не подхватятся из PATH.
- winget из Git Bash может падать из-за интерактивного прогресса —
  при сбое выполняйте команды из PowerShell.
- Что **не нужно** ставить сейчас (потребуется позже в конкретных задачах):
  - `cargo install cargo-wix` — сборка MSI, задача T19;
  - бинарник pdfium — PDF-превью, задача T11;
  - WebView2 Evergreen bootstrapper — дистрибуция, задача T19.
