# Настройка окружения разработки CanvasDesk

Пошаговая инструкция: что установить на чистой машине, чтобы начать
разработку (задача T0 из `docs/TASKS.md` и далее). Проект
кроссплатформенный (M7): Windows — полная функциональность, Linux/macOS —
оконный канвас с платформенными фичами по плану M7.

Проверено на: Windows 11 25H2 (build 26200), Ubuntu 24.04 (GitHub Actions
ubuntu-latest), macOS 14 (GitHub Actions macos-latest).

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

### Шаг 4. Проверка установки (критерий готовности к T0)

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

## 4. Linux (Ubuntu/Debian)

Rust ставится rustup'ом, MSVC не нужен; системные пакеты — только
для линковки (linker) и тестов SQLite (rusqlite bundled собирает С
компилятором из системы):

```bash
sudo apt install build-essential pkg-config curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
# в новом шелле:
rustup component add clippy rustfmt
cargo build --workspace && cargo test --workspace
```

Особенности: тамбнейлы файлов — заглушки до T27 (план M7); drag-drop из
файлового менеджера — до T28; MCP-транспорт — до T29; `--desktop` —
Windows-only. Всё остальное (канвас, заметки, связи, вотчер inotify,
поиск FTS5, минимапа, undo) работает.

## 5. macOS

```bash
xcode-select --install   # clang для C-зависимостей (rusqlite bundled)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
rustup component add clippy rustfmt
cargo build --workspace && cargo test --workspace
```

Особенности: как на Linux, но вотчер — FSEvents (совместимость уже в
коде, 70783d1); wgpu рендерит через Metal.

## 6. Кросс-чек Windows-кода с Linux (для агента/CI)

`cargo check/clippy --target x86_64-pc-windows-msvc -p canvas-widgets -p
canvas-core -p canvas-render` работает без линкера (крейты без
C-зависимостей); canvas-app не покрыт (rusqlite требует lib.exe) — его
Windows-компиляцию валидирует только CI (гейты на ubuntu/windows/macos).
Правки `.github/workflows/` агент пушит сам — PAT со scope `workflow`.

## 7. Замечания

- После установки Build Tools и rustup терминал нужно перезапустить —
  иначе `link.exe`/`cargo` не подхватятся из PATH.
- winget из Git Bash может падать из-за интерактивного прогресса —
  при сбое выполняйте команды из PowerShell.
- Что **не нужно** ставить сейчас (потребуется позже в конкретных задачах):
  - `cargo install cargo-wix` — сборка MSI, задача T19;
  - бинарник pdfium — PDF-превью, задача T11;
  - WebView2 Evergreen bootstrapper — дистрибуция, задача T19.

## 8. Устранение неполадок

### Windows: LNK1104 «не удается открыть файл …canvasdesk.exe»

Симптом: `cargo build/run --release` падает на линковке
(`link.exe failed with exit code: 1104`) — линкер не может открыть на
запись `target\release\deps\canvasdesk.exe`. Это **не ошибка кода**:
CI (windows-latest) собирает тот же коммит — выходной файл на машине
разработчика занят другим процессом. Windows блокирует exe работающего
процесса, а `deps\canvasdesk.exe` — жёсткая ссылка на файл
`target\release\canvasdesk.exe`, который запускает `cargo run`
(в `deps/` cargo использует имя с подчёркиваниями, это тот же файл).

Причины по частоте и лечение:

1. **CanvasDesk запущен.** Закройте окно приложения. Если окна нет —
   процесс может жить в фоне: режим `--desktop`, включённый автозапуск
   (HKCU Run), либо осиротевший после закрытия терминала `cargo run`
   экземпляр (Ctrl+C в консоли не убивает GUI-процесс — он остаётся в
   Диспетчере задач). Лечение:
   ```powershell
   taskkill /f /im canvasdesk.exe
   ```
2. **Антивирус (Windows Defender).** Real-time сканирование свежего
   exe кратко блокирует файл — повторная сборка проходит. Для комфорта
   добавьте исключение на папку проекта (или хотя бы `target\`):
   «Параметры → Конфиденциальность и защита → Безопасность Windows →
   Защита от вирусов и угроз → Управление настройками → Исключения».
3. **Параллельные сборки.** IDE (rust-analyzer выполняет `cargo check`)
   и ручная сборка в терминале могут конфликтовать за один выходной
   файл — не запускайте две сборки одновременно.

Linux/macOS разрешают перезапись запущенных бинарников — проблема
специфична для Windows. Обходной путь без локальной сборки: свежие
бинари каждого пуша в main — артефакты `build-<os>` в GitHub Actions
(workflow CI, job artifacts).
