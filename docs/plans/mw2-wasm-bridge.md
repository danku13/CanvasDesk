# MW2. Мост canvas-mcp под wasm — план реализации (три сабагента)

- **Статус:** принят к исполнению (решение владельца 2026-09-19)
- **Дата:** 2026-09-19
- **Задача-источник:** FR-037 (ADR-0012), §«Задачи», строка MW2
- **Связанные:** FR-008 (подкоманда `mcp`), FR-034/ADR-0009 (транспорт),
  FR-035/ADR-0010 (чистота stdio), FR-036/ADR-0011 (прецедент wasm-гейта и
  wasip1-раннер), `docs/plans/wasm-port.md` (§6, примечание FR-037)
- **Решения владельца (2026-09-19):** MW2 выполняется без MW1 — файловые
  множества задач не пересекаются (`canvas-mcp` против
  `canvas-app`/`canvas-scene`); ветка `feature/fr-037-mw2-wasm-bridge` →
  один коммит → merge в main; полный локальный гейт приёмки (rustup stable +
  wasm32-unknown-unknown + wasm32-wasip1 + wasmtime ставятся в среде
  агента); план — этот файл; исполнение — тремя сабагентами
  MW2-a/MW2-b/MW2-c.

---

## 1. Объём MW2 (FR-037, дословно)

> **Мост под wasm.** `run_stdio` → `pub fn run_stdio_with_transport<T:
> AppTransport>` (чистое выделение; `run_stdio` — обёртка, поведение
> FR-008/034/035 не меняется); wasm-таргеты моста в гейтах; точечные
> `#[cfg(not(target_arch = "wasm32"))]` на тестах автоспавна (`std::process`
> в test-cfg — test-only cfg, прецедент FR-036). Приёмка: `cargo check
> --target wasm32-unknown-unknown -p canvas-mcp` ✓; `cargo test --target
> wasm32-wasip1 -p canvas-mcp` ✓ (тесты моста в wasmtime); регресс
> FR-008/034/035 — ноль.

Границы: продуктовое поведение не меняется; `main.rs` (bin
`canvasdesk-mcp`) не трогается; формат `.canvas` не затронут;
AGENTS/SPEC/ACCEPTANCE синхронизируются в MW4 (не здесь); новых внешних
зависимостей нет; новых тестов MW2 не требует (выделенный цикл — тонкая
обвязка над `handle_input`, который уже покрыт 13 тестами; число тестов
workspace не убывает).

## 2. Факты кода (main `1d419c2`)

1. **Крейт уже почти wasm-чист.** `crates/canvas-mcp/src/lib.rs` (1530
   строк): протокольный слой `parse_envelope`/`build_*`/`handle_line`/
   `handle_input` (`lib.rs:88–551`) — только serde_json; трейт
   `AppTransport` (`:60`). Зависимости (`Cargo.toml`): serde_json + anyhow;
   `windows` — под `[target.'cfg(windows)'.dependencies]` — правок
   Cargo.toml не требуется.
2. **Единственный владелец stdio-цикла — `run_stdio` (`lib.rs:594–637`).**
   Платформенное ветвление внутри: `cfg(windows)` — `connect_app` +
   `refresh_transport` (FR-034: reconnect перед каждым пакетом);
   `cfg(not(windows))` — `Option<OfflineTransport> = None`. Вызывается из
   `src/main.rs` (bin `canvasdesk-mcp`) и подкоманды `canvasdesk mcp`.
3. **Автоспавн-хелперы** `spawn_service_command`/`autosprawn_target`
   (`:659–689`) — `cfg(any(windows, test))`; под wasip1-test компилируются
   (std::process::Command есть в std wasi), но вызывающие тесты выполняют
   реальную ФС/процессную работу.
4. **15 тестов** (`:924–1529`): 13 чистых (framing, конверты, handshake,
   batch, offline, таймаут, −32601) + 2 автоспавн-теста:
   `spawn_service_command_isolates_stdio` (уже `#[cfg(unix)]` — wasm-таргеты
   не unix) и `autosprawn_target_prefers_sibling_gui_for_standalone_bridge`
   (без cfg — temp_dir/fs под wasip1 упадут в рантайме).
5. **Гейты**: `scripts/wasm_gate.sh` — `CRATES="-p canvas-core -p
   canvas-render -p canvas-widgets"`, ступень 3 жёстко `-p canvas-core`;
   CI `ci.yml`, джоба `wasm-check` — тот же список крейтов, только
   компиляция (исполнение — локально, ADR-0011); `.cargo/config.toml` —
   runner `wasmtime run -S inherit-env --dir .::/ --dir /tmp::/tmp` для
   wasip1.
6. **Блокеры wasm**: компиляция под `wasm32-unknown-unknown` — блокеров не
   видно (Windows-ветки отсекаются cfg; std::io компилируется); исполнение
   под wasip1 — только 2 автоспавн-теста (гварды MW2-a).

## 3. Ключевое решение: сигнатура выделения

`run_stdio_with_transport` получает транспорт **и хук переподключения**:
reconnect (FR-034) платформенный, спрятать его в транспорт нельзя — трейт
`AppTransport` не умеет строить новые соединения.

```rust
/// Платформенно-нейтральный stdio-цикл моста (FR-037/MW2).
pub fn run_stdio_with_transport<T, R>(
    mut transport: Option<T>,
    mut reconnect: R,
) -> anyhow::Result<()>
where
    T: AppTransport,
    R: FnMut(&mut Option<T>),
```

- `run_stdio` — тонкая обёртка в стиле текущего кода (cfg-let'ы): сборка
  транспорта + хук (`refresh_transport` на Windows, no-op на прочих
  платформах) + вызов.
- MW3 (headless, будущая задача) вызовет
  `run_stdio_with_transport(Some(session), |_| {})` — литеральная запись
  FR-037 «`run_stdio_with_transport(Some(session))`» уточняется вторым
  аргументом; правка строки MW3 — в момент исполнения MW3.
- Альтернативы отклонены: цикл без хука — потеря FR-034 на Windows либо
  дублирование цикла; хук внутри `AppTransport` — платформенная утечка в
  протокольный трейт.

## 4. Декомпозиция (3 сабагента)

| ID | Объём | Файлы | Зависит от |
|----|-------|-------|------------|
| MW2-a | Рефакторинг моста + гварды тестов | `crates/canvas-mcp/src/lib.rs` | — |
| MW2-b | Гейты: скрипт + CI | `scripts/wasm_gate.sh`, `.github/workflows/ci.yml` | — |
| MW2-c | Верификация, документация, коммит | FR-037, worklog.md, git | MW2-a + MW2-b + окружение |

Параллелизм: MW2-a ∥ MW2-b (множества файлов не пересекаются) → MW2-c.
Тулчейн (rustup stable + два wasm-таргета + wasmtime) ставит оркестратор
параллельно с MW2-a/MW2-b. Каждому сабагенту — Task ID, чтение
`AGENTS.md`, этого плана и `/home/z/my-project/worklog.md`; после работы —
запись в `/home/z/my-project/worklog.md`.

### MW2-a — мост (детальное ТЗ)

1. Тело `run_stdio` (цикл stdin → `split_frames` → `handle_input` →
   stdout, `lib.rs:594–637`) переносится в
   `run_stdio_with_transport<T, R>` (§3); вместо
   `#[cfg(windows)] refresh_transport(&mut transport)` — вызов
   `reconnect(&mut transport)` перед разбором каждого пакета.
2. `run_stdio` — обёртка (cfg-let стиль существующего кода):
   - `cfg(windows)`: транспорт = `connect_app(no_spawn)`, хук =
     `refresh_transport` (fn item реализует FnMut);
   - `cfg(not(windows))`: `let _ = no_spawn;`, транспорт
     `None::<OfflineTransport>`, хук — no-op замыкание;
   - финальный вызов `run_stdio_with_transport(transport, reconnect)`.
   Докстринг `run_stdio` актуализировать (нативная обёртка; цикл выделен
   по FR-037/MW2).
3. Гварды тестов: `spawn_service_command_isolates_stdio` —
   `#[cfg(all(unix, not(target_arch = "wasm32")))]`;
   `autosprawn_target_prefers_sibling_gui_for_standalone_bridge` —
   `#[cfg(not(target_arch = "wasm32"))]`; комментарий-ссылка на прецедент
   FR-036 (test-only cfg).
4. Контингенция: если автоспавн-хелперы не компилируются под wasip1-test —
   расширить их cfg до `#[cfg(any(windows, all(test, not(target_arch =
   "wasm32"))))]`.
5. Запреты: не трогать `main.rs`, имена/ассерты 15 тестов, поведение
   FR-008/034/035, Cargo.toml; комментарии на русском; стиль/fmt файла
   соблюсти.
   Приёмка (статическая): pub-сигнатура `run_stdio(&[String]) ->
   anyhow::Result<()>` неизменна; `run_stdio_with_transport` — pub; diff —
   только `lib.rs`.

### MW2-b — гейты (детальное ТЗ)

1. `scripts/wasm_gate.sh`: `CRATES` += `-p canvas-mcp` (ступени 1–2);
   ступень 3 — явный список `-p canvas-core -p canvas-mcp` (**не**
   `$CRATES`: render/widgets под wasip1 не тестируются — wgpu-тесты требуют
   GPU-адаптер; прецедент — текущая жёсткая строка `-p canvas-core`);
   шапка-комментарий и echo-строки синхронизировать.
2. `ci.yml`, джоба `wasm-check`: `run:` += `-p canvas-mcp`; имя шага и
   комментарий (FR-036 → «+ FR-037/MW2 мост») актуализировать; `targets:`
   джобы не меняются (wasip1 — локально, ADR-0011); прочие джобы не
   трогать.
3. Приёмка (статическая): `bash -n scripts/wasm_gate.sh` ✓; YAML-отступы
   точны; diff — ровно два файла.

### MW2-c — верификация, документация, коммит

Гейты приёмки (тулчейн ставит оркестратор; PATH: `~/.cargo/bin` +
`~/.wasmtime/bin`; порядок — от дешёвых к тяжёлым):

1. `cargo fmt --check` (дрейф — `cargo fmt` с пометкой в отчёте);
2. `cargo clippy -p canvas-mcp --all-targets -- -D warnings`;
3. `cargo test -p canvas-mcp` — нативно: 15 passed (регресс
   FR-008/034/035 — ноль);
4. `cargo check --target wasm32-unknown-unknown -p canvas-mcp` ✓ (R1);
5. `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-mcp`
   ✓ — 13 passed в wasmtime (R2);
6. `scripts/wasm_gate.sh` — полный зелёный прогон (ступени 1–3; тяжёлые
   первые сборки — nohup + лог + поллинг, cargo инкрементален);
7. Документация: FR-037 — строка MW2 «**Выполнено 2026-09-19:** …»
   (прецедент — строка W0 в wasm-port.md) + Changelog; `worklog.md`
   (корень репо) — запись сверху по формату файла;
   `/home/z/my-project/worklog.md` — секция Task ID mw2-c;
8. Коммит (один, conventional): `feat(mcp): FR-037 MW2 — мост canvas-mcp
   под wasm` (тело: изменения + результаты приёмки; файлы: `lib.rs`,
   `wasm_gate.sh`, `ci.yml`, `docs/plans/mw2-wasm-bridge.md`, FR-037,
   `worklog.md`). Push ветки; merge в main — оркестратор.

## 5. Риски и митигации

| Риск | Вероятность | Митигация |
|---|---|---|
| 10-мин таймаут команд в среде агента (первые сборки wgpu под wasm) | средняя | cargo инкрементален — повторный запуск продолжает с кэша; тяжёлые шаги — nohup + лог + поллинг |
| wasmtime новее отлаженного (флаги `-S inherit-env`, `--dir`) | низкая | при конфликте флагов — pinned-версия install-скриптом (`--version`); строка runner в `.cargo/config.toml` не меняется |
| Сеть crates.io | низкая | Cargo.lock закоммичен (`--locked`); сбой — повтор |
| wasip1-поведение `std::io::stdin` в тест-харнессе | низкая | харнесс stdin не читает; 13 чистых тестов не трогают io; гварды MW2-a снимают ФС/процессы |
| Регресс FR-008/034/035 | низкая | `run_stdio` — обёртка без изменения семантики; 15 нативных тестов — гейт MW2-c |

## 6. Критерии приёмки MW2 (сводка = строка MW2 FR-037)

1. fmt/clippy/нативные тесты + R1/R2 — зелёные (MW2-c, пп. 1–5).
2. `scripts/wasm_gate.sh` — полный зелёный прогон.
3. Число тестов workspace не убывает (1034 — нижняя граница).
4. Поведение Windows-продукта не изменилось (`main.rs` без правок,
   pub-сигнатуры стабильны).
5. Документация синхронна: FR-037 (строка MW2 + Changelog), worklog.
6. Один коммит в `feature/fr-037-mw2-wasm-bridge`, merge в main, CI
   `wasm-check` зелёный на пушe main.

## История изменений

- 2026-09-19 — создан по приказу владельца «распланируй реализацию MW2
  с 3 сабагентами»: декомпозиция MW2-a/b/c, решение о сигнатуре с хуком
  reconnect (§3), факты кода, риски, критерии приёмки; решения владельца
  зафиксированы в шапке.
