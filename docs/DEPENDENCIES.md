# Реестр зависимостей CanvasDesk

- **Статус:** принято (живой документ)
- **Дата создания:** 2026-09-18 (CP0, волна 0.2 продуктового роадмапа)
- **Связанные:** `docs/plans/product-roadmap.md` §4.1 (волна 0);
  `docs/architecture/math-computing-stack.md` §4 (критерии приёма, кандидаты),
  §7 (лицензионная политика); ADR-0008 (вычислительный стек); `deny.toml`
  (автоматический контроль), `about.toml` + `THIRD-PARTY-NOTICES.md` (notices)
- **Правило поддержания:** любая новая зависимость — это (1) ADR при
  неоднозначности, (2) строка в этом реестре до слияния, (3) зелёный
  `cargo deny check`, (4) перегенерация `THIRD-PARTY-NOTICES.md`
  (§Рецепты ниже), (5) запись в changelog PR. Версии всех прямых
  зависимостей централизованы в `[workspace.dependencies]` корневого
  `Cargo.toml` (SPEC §3).

## 1. First-party (workspace, закрытый код)

Все крейты помечены `publish = false`: закрытый B2B-продукт, публикация на
crates.io запрещена. Лицензирование продукта целиком регулируется
B2B-контрактом/EULA; в SBOM first-party — сам бинарник `canvasdesk`.

| Крейт | Назначение |
|---|---|
| `canvas-core` | модель `.canvas` + Numi-движок + поток значений (без GUI) |
| `canvas-render` | GPU-рендер (wgpu), текст, темы |
| `canvas-app` | приложение (окно, ввод, UI, MCP-диспетчер) |
| `canvas-shell` | десктоп-обвязка (Win32/COM, иконки, watcher) |
| `canvas-widgets` | движок HTML-виджетов (манифесты, bridge, LOD) |
| `canvas-mcp` | MCP-посредник: stdio ↔ named pipe, 22 инструмента |
| `canvas-preview-host` | headless-хост для скриншотов виджетов |
| `canvas-scene` | модель сцены + MCP-инструменты (wasm-верификация, FR-037) |
| `canvas-mcp-headless` | headless MCP-сервер для wasmtime/wasip1 (FR-037) |
| `canvas-web` | web-платформенный слой: bindgen-обвязка, web-сервисы (M8/W4) |

## 2. Прямые прод-зависимости (факт, `Cargo.lock` 2026-09-24)

Всего в графе сборки — 401 сторонний крейт (включая транзитивные и
dev-зависимости); полный состав с текстами лицензий —
`THIRD-PARTY-NOTICES.md`, машиночитаемый контроль — `deny.toml`.
Криптозависимостей нет; нативный C — только bundled SQLite внутри
`rusqlite` и toolchain-level `cc` (исключение зафиксировано архдоком §4.1).

| Крейт | Версия | Лицензия | Назначение / выбор |
|---|---|---|---|
| `serde` + `serde_json` | 1.x | MIT OR Apache-2.0 | сериализация `.canvas`/MCP |
| `thiserror` | 2.x | MIT OR Apache-2.0 | типизированные ошибки core |
| `anyhow` | 1.x | MIT OR Apache-2.0 | ошибки приложения |
| `toml` | 0.8 | MIT OR Apache-2.0 | `config.toml` (T19) |
| `rstar` | 0.12 | MIT OR Apache-2.0 | R-tree spatial index (SPEC §4) |
| `rusqlite` (bundled) | 0.32 | MIT | тамбнейл-кэш; bundled SQLite — public domain, без системного SQLite |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | MIT | логирование |
| `notify` | 8.2 | CC0-1.0 | файловый вотчер (T10) |
| `winit` | 0.30 | Apache-2.0 OR MIT | окно, ввод, цикл событий |
| `wgpu` | 22 | MIT OR Apache-2.0 | GPU-рендер |
| `glyphon` | 0.6 | MIT OR Apache-2.0 OR Zlib | текст на GPU |
| `cosmic-text` | 0.12 | MIT OR Apache-2.0 | шейпинг/редактирование текста |
| `arboard` | 3.x | MIT OR Apache-2.0 | буфер обмена (T7) |
| `pollster` | 0.3 | Apache-2.0/MIT | блокирующий запуск async GPU |
| `wasm-bindgen` | 0.2.127 | MIT OR Apache-2.0 | JS-глю браузерной сборки `canvas-web` (M8/W4); семейство уже было в дереве транзитивно (winit, wasm-цели) — с W4 прямая зависимость, компилируется и нативно (заглушки макросов) |
| `windows` / `windows-core` | 0.62 | MIT OR Apache-2.0 | Win32/COM (только Windows-таргеты) |
| `statrs` | 0.17 | MIT | L2-статистика: распределения/квантили/ДИ (FR-063, за фичей `stats` в canvas-core — в сборку по умолчанию не входит) |
| `rand` | 0.8 | MIT OR Apache-2.0 | Rng-трейты, SeedableRng (FR-063, за фичей `stats`; default-features = false — без getrandom) |
| `rand_chacha` | 0.3 | MIT OR Apache-2.0 | ChaCha8Rng — единственный источник случайности (FR-063, за фичей `stats`) |
| `rand_distr` | 0.4 | MIT OR Apache-2.0 | сэмплирование Normal/LogNormal (FR-063, за фичей `stats`) |

**Выбор опции дуальных лицензий.** Для крейтов `MIT OR Apache-2.0`
продукт следует обязательствам обеих сторон консервативно: сохранение
notices (MIT) и NOTICE-механики (Apache-2.0) обеспечены генерируемым
`THIRD-PARTY-NOTICES.md` (архдок §7.3). Явного отказа от одной из опций
не требуется — ссылки на оба текста присутствуют в notices.

## 3. Кандидаты на будущее (archdoc §4.2, только по продуктовому триггеру)

Волна S роадмапа: подключение — только через cargo-фичи `stats` /
`parallel` в `canvas-core` (см. `crates/canvas-core/Cargo.toml`), чтобы
B2B-сборка могла отключить неиспользуемые слои. S0 (Foundation) и S1
(FR-063, M2) выполнены 2026-09-24: `statrs`/`rand`/`rand_chacha`/
`rand_distr` перенесены в §2.

| Слой | Крейт | Назначение | Лицензия | Триггер (роадмап §4.5) |
|---|---|---|---|---|
| L2 | `sobol_burley` | QMC (Соболь, Owen-scrambled) | MIT OR Apache-2.0 | S3 (FR-066, за фичей `qmc`) |
| L2 | `argmin` | численная оптимизация | MIT OR Apache-2.0 | первый домен с оптимизацией |
| L2 | `gauss-quad` / `quadrature` | квадратуры | MIT OR Apache-2.0 / BSD-2 | интегралы SLA |
| L2 | `puruspe` / `special` | спецфункции | MIT OR Apache-2.0 | при выходе за `statrs` |
| L3 | `ndarray` / `faer` / `nalgebra` | линейная алгебра | MIT / MIT / Apache-2.0 | M6: домен с матричной математикой |
| L4 | `rayon` | поярусный параллелизм | MIT OR Apache-2.0 | S3 (уже транзитивно в дереве через `cosmic-text`) |
| L4 | `crossbeam-channel`, `parking_lot` | примитивы синхронизации | MIT OR Apache-2.0 | S2/S3 при недостатке std |
| L5 | `chrono` / `jiff` | календарные сетки финдоменов | MIT OR Apache-2.0 | первый домен с датами |
| L5 | `cargo-deny` / `cargo-about` / `cargo-auditable` | CI-контроль | MIT OR Apache-2.0 | уже внедрены (CP0) — вне бинарника |

Отвергнутые варианты и rationale — `docs/architecture/math-computing-stack.md`
§4.3 (внешние embed-движки, C/C++ BLAS, Python-ядро, GPU-compute, polars,
copyleft-изоляция).

## 4. Долг сопровождения (unmaintained, осознанные исключения)

`deny.toml [advisories].ignore` фиксирует три информационных RUSTSEC ID
(не уязвимости — уведомления о прекращении сопровождения крейтов
рендер-стека; решение: миграция wgpu/cosmic-text/glyphon — после гейта
Go, см. роадмап §4.5):

| RUSTSEC | Крейт | Путь в дереве | План |
|---|---|---|---|
| RUSTSEC-2024-0436 | `paste` 1.0.15 | wgpu 22 → naga | миграция wgpu 23+ |
| RUSTSEC-2026-0206 | `rustybuzz` 0.14.1 | cosmic-text 0.12 → glyphon 0.6 | миграция cosmic-text/glyphon |
| RUSTSEC-2026-0192 | `ttf-parser` 0.20/0.21/0.25 | cosmic-text/glyphon/rustybuzz (3 копии) | миграция рендер-стека; дубли — warn в [bans] |

Новые unmaintained-уведомления валят CI (`licenses` job) и требуют
сознательного триажа: добавить сюда с планом или обновить крейт.

## 5. Рецепты

### Перегенерация THIRD-PARTY-NOTICES (после изменения состава зависимостей)

```bash
cargo about generate -c about.toml docs/templates/third-party-notices.hbs \
  > THIRD-PARTY-NOTICES.md
git add THIRD-PARTY-NOTICES.md  # файл живёт в репо; релиз проверяет дрифт
```

Релизный пайплайн (`.github/workflows/build-all.yml`, job `notices-sbom`)
валидирует отсутствие дрифта и прикладывает notices к релизу.

### Контроль лицензий локально (то же, что CI-джоба `licenses`)

```bash
cargo deny check          # licenses + bans + sources + advisories
cargo deny check licenses # точечно
```

### SBOM из релизного бинарника (cargo-auditable)

Релизные бинари собираются `cargo auditable build` (CI `artifacts`) —
полный список зависимостей вшит в бинарник (`.dep-vectors` секция).
Извлечение для заявки/аудита:

```bash
cargo install auditable-extract --locked
auditable-extract target/release/canvasdesk > canvasdesk-sbom.json
```

### Добавление новой зависимости (чек-лист)

1. Критерии архдока §4.1 (пермиссивная лицензия из allowlist §7.2, pure
   Rust, без криптографии, живой репозиторий, транзитивная чистота —
   `cargo deny check` покажет).
2. Версия — в `[workspace.dependencies]` корневого `Cargo.toml`; в крейте —
   `cargo.toml = { workspace = true }`-паттерн.
3. Строка в §2/§3 этого реестра (назначение, выбор опции дуальной лицензии).
4. Перегенерация notices (рецепт выше) в том же PR.
5. Дуальные лицензии: при `X OR Y` с непермиссивной опцией — пермиссивная
   фиксируется в реестре явно (архдок §7.2).

## История изменений

- `2026-09-18` — создан при выполнении CP0 волны 0 (роадмап §4.1): факт
  прямых зависимостей (23, включая 7 workspace), кандидаты волны S из
  архдока §4.2, долг сопровождения рендер-стека, рецепты notices/SBOM.
- `2026-09-24` — S0/S1 (FR-063): `statrs` 0.17 (MIT), `rand` 0.8 /
  `rand_chacha` 0.3 / `rand_distr` 0.4 (MIT OR Apache-2.0) перенесены
  из §3 в §2 — optional за фичей `stats` в canvas-core (default-сборка
  их не резолвит); отмечена находка: транзитивный getrandom 0.2 от
  statrs→rand(std) не компилируется под wasm32-unknown-unknown — не
  влияет на гейты (они идут с default-фичами), решение по web-сборке
  с `stats` — точка решения владельца.
