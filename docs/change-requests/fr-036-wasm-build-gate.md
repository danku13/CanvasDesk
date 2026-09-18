# FR-036: WASM-сборка ядра — гейт wasm32-unknown-unknown + исполнение тестов в wasmtime (wasm32-wasip1)

**Статус:** реализовано (v1) · **Дата:** 2026-09-18 · **ADR:** 0011 · **Связанные:** `docs/plans/wasm-port.md` (M8, W0), ADR-0008, AGENTS «Сборка и тесты», SPEC §3

## Проблема

Приказ владельца: «распланировать сборку wasm и реализовать сборку для
повышения твоей автономности в тестировании». Среда агента — Linux-контейнер
без GUI и Windows: нативные гейты (fmt/clippy/test) зелёные, но приложение
в окне и named-pipe-сессии не запускаются. При этом план веб-порта M8
(`docs/plans/wasm-port.md`, §2) зафиксировал экспериментом от 2026-09-16,
что ядро компилируется под `wasm32-unknown-unknown` без изменений — а
закрепляющая это задача W0 (CI-гейт) оставалась открытой.

| # | Разрыв | Следствие |
|---|--------|-----------|
| 1 | Компиляция ≠ исполнение | `cargo check` не доказывает корректность ядра под wasm; паники `std::env::temp_dir`/`std::process::id` на wasm-таргетах остались бы не найденными |
| 2 | Гейт не закреплён | wasm-совместимость ядра защищена только разовым экспериментом — любой коммит может её сломать незаметно |
| 3 | Нет wasm-тест-раннера | Агент не может прогонять тесты расчётного ядра в wasm-среде: GUI-зависимые слои недоступны, а чистое ядро — доступно, но не исполняется |

## Требование (решение ADR-0011)

1. **Продуктовый таргет `wasm32-unknown-unknown`**: check ядра
   (`canvas-core`, `canvas-render`, `canvas-widgets`) + артефакт rlib
   ядра — локальный скрипт `scripts/wasm_gate.sh` (ступени 1–2) и
   CI-джоба `wasm-check` (ubuntu, каждый пуш — приёмка W0).
2. **Тестовый таргет `wasm32-wasip1`**: runner wasmtime в
   `.cargo/config.toml` (`-S inherit-env --dir .::/ --dir /tmp::/tmp`,
   активен только при явном `--target`); ступень 3 гейта —
   `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core`.
3. **Тестовая песочница** `test_scratch_root()` (`#[cfg(test)]` в core):
   нативно — `temp_dir` (поведение не меняется), под wasm —
   `.wasi-scratch` в предоткрытом CWD; в интеграционном тесте
   `scene_ops` — локальная копия + замена `process::id()` на
   `SystemTime`-суффикс. Единственное `cfg(target_arch)` в core —
   только в тестовой компиляции (исключение зафиксировано в ADR-0011).
4. Правило «ядро обязано собираться и исполняться под wasm» — в AGENTS
   («Сборка и тесты»); строка wasm-таргетов — SPEC §3; статус W0 — в
   плане M8.

## Изменения по файлам

| Файл | Изменение |
|---|---|
| `scripts/wasm_gate.sh` | NEW: трёхступенчатый гейт (check / build rlib / test wasip1), режим `--check` |
| `.cargo/config.toml` | NEW: runner wasmtime для `wasm32-wasip1` (предоткрытые каталоги) |
| `.github/workflows/ci.yml` | NEW: джоба `wasm-check` — ступень 1 на каждый пуш (приёмка W0) |
| `crates/canvas-core/src/lib.rs` | NEW: `#[cfg(test)] test_scratch_root()` — wasm-совместимая песочница |
| `crates/canvas-core/src/settings.rs` | 2 теста: `temp_dir` → `test_scratch_root()` |
| `crates/canvas-core/src/templates.rs` | `temp_root()` + тест absent-корня: → `test_scratch_root()` |
| `crates/canvas-core/tests/scene_ops.rs` | NEW: локальный `scratch_root()`; `process::id()` → `SystemTime` |
| `.gitignore` | `.wasi-scratch/` (побочные артефакты wasm-прогонов) |
| `AGENTS.md` · `docs/SPEC.md` §3 · `docs/plans/wasm-port.md` · `docs/adr/README.md` · `docs/change-requests/index-cr-fr.md` · `docs/ACCEPTANCE.md` §25 | контракты и индексы |

## Критерии приёмки

- `cargo check --target wasm32-unknown-unknown -p canvas-core -p
  canvas-render -p canvas-widgets` — зелёный.
- `cargo test --target wasm32-wasip1 -p canvas-core` (wasmtime) —
  **301/301** (lib 189 + 112 интеграционных).
- Нативный регресс — ноль: `cargo test -p canvas-core` — 301/301,
  поведение тестов не изменено.
- `scripts/wasm_gate.sh` — OK (все ступени); `--check` — OK.
- CI: `wasm-check` зелёная на пуш в main.

## Не-цели (границы волны)

- Браузерный MVP (W1–W12: web_time, вынос App в lib, крейт `canvas-web`,
  OPFS/FS Access, wasm-bindgen-фасады) — последующие задачи плана M8.
- Исполнение wasm-тестов в CI (wasmtime в раннере) — локальная ступень
  гейта; перенос в CI при появлении потребности.
- `canvas-render`/`canvas-widgets` — только компиляция (GPU-тесты
  требуют адаптера WebGPU — вне wasip1).

## Changelog

- 2026-09-18 — v1: гейт (скрипт + CI), runner-конфиг, wasm-совместимость
  тестов ядра; 301/301 под wasip1 и нативно; ADR-0011.
