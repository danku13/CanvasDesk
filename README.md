# CanvasDesk

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![CLA](https://img.shields.io/badge/CLA-required-orange.svg)](CLA.md)
[![cla-check](https://img.shields.io/badge/PR-CLA%20auto--accept-green.svg)](.github/workflows/cla-check.yml)

**Визуальная система математического моделирования** (ADR-0007): бесконечный
зумируемый канвас, на котором исполняемые математические модели строятся из
расчётных нод (Numi-листы и шаблоны), значения **проливаются** по
value-связям (DAG-движок, live-пересчёт), доменная математика (единицы
измерения, теория очередей, финансы) встроена в ядро, а ИИ-агент собирает и
проверяет модели через MCP.

Носитель модели — файловый канвас (карточки — настоящие файлы), JS/HTML-виджеты
и режим «вместо рабочего стола»: исходная инфраструктура первой волны
(M1–M5) никуда не ушла — на ней строятся модели.

Формат хранения — [JSON Canvas](https://jsoncanvas.org) (`.canvas`): раскладку
можно открыть в Obsidian и наоборот, неизвестные поля переживают round-trip
(совместимость формата — часть носителя, не позиционирование).

## Сценарии и целевые аудитории

CanvasDesk закрывает четыре класса задач — по эталонам
[ADR-0006](docs/adr/adr-0006-reference-scenario-catalog.md); портреты ролей
и боли аудиторий — в [исследовании рынка](docs/market-researches/README.md).

| ЦА | Сценарий (типовой вопрос) | Чем закрывается | Где посмотреть |
|---|---|---|---|
| **Solution/Technical архитектор** | Capacity: «выдержит ли сервис 10k rps? сколько реплик и воркеров нужно?» | queueing-математика `mm1`/`mmc`/`erlang_c`/`littles_law`, инфраструктурные шаблоны (LB, шлюз, кэш, БД, очередь, CDN…), индикаторы узких мест по ρ | эталон №1 system design; шаблоны `cdn`, `cache-redis`, `queue-kafka`, `websocket` |
| **Продакт-аналитик / финдир** | Юнит-экономика: «что с runway, если churn −1 пп? какой LTV при таком CAC?» | UE-шаблоны (CAC, LTV, LTV:CAC, MRR, ARR, NRR, GRR, ARPU, burn rate, runway, NPV/IRR/CAGR), what-if с таблицей сравнения сценариев | эталон №5 unit economics; шаблоны `ue-*` |
| **Архитектор / менеджер** | Защита бюджета и решений: «что если нагрузка ×2? где сломается P&L?» | именованные what-if сценарии (до 3), дельты «было → стало», Apply одним undo — базовая модель не портится | эталоны №2–№4 (защита бюджета, P&L, value chain); Ctrl+Shift+I |
| **ИИ-агент (MCP)** | «Собери модель по описанию и проверь числа» — агент строит граф за минуты, человек исследует | 39 MCP-инструментов: `graph_apply` (атомарный батч), `graph_validate` (коды E-*/W-*), `analyze_bottlenecks`, `whatif_*` | [рецепт агента](user-docs/agent-recipe.md), [пакет скиллов](skills/README.md) |

## Что стоит попробовать

- **Веб-версия без установки** — [danku13.github.io/CanvasDesk/app/](https://danku13.github.io/CanvasDesk/app/): тот же движок в браузере (WebGPU, Chromium), загрузка ~1 с; ввод, кириллица, поиск, OPFS — работают.
- **Готовый бинарь** — GitHub Actions → CI → артефакты `build-<os>` (Windows x64, Linux, macOS universal2 `.app`).
- **Эталонные схемы** — галерея схем в приложении и `assets/canvas-schemes/` (unit-economics, capacity-service, intro-whatif, project-budget…): открыть и покрутить параметры.
- **45 встроенных шаблонов** — палитра слева или wheel-меню: инфраструктура, юнит-экономика, продуктовая аналитика.
- **What-if** — Ctrl+Shift+I: подмените строку расчёта и увидите дельты по всему downstream; сравните сценарии таблицей.
- **ИИ-агент** — `canvasdesk.exe mcp` + [рецепт агента](user-docs/agent-recipe.md): агент собирает модель, `graph_validate` проверяет корректность, `analyze_bottlenecks` показывает узкие места.
- **Виджеты и кастомизация** — SDK-шаблон [sdk/widget-template/](sdk/widget-template/README.md) (примеры todo-panel/dashboard); 7 тем оформления (tokyo-night, dracula, nord…), английский интерфейс в настройках.

## Математическое моделирование

- **Numi-листы** — формулы «человеческим языком» в любой заметке:
  `rps = 1000`, `latency = 50 ms`, единицы конвертируются, переменные
  протекают сверху вниз (FR-013, FR-015)
- **Поток значений** — value-связи между нодами: значение источника
  пересчитывает весь downstream мгновенно; граф — DAG, циклы блокируются
  (FR-014); построчные точки выхода — за фича-флагом (FR-025)
- **45 встроенных шаблонов** — инфраструктура (балансировщик, шлюз, кэш,
  БД, очередь, CDN…), юнит-экономика (CAC, LTV, MRR, runway…), продуктовая
  аналитика (retention, funnel, NPS…) — палитра, wheel-меню, MCP
  (FR-018/FR-019/FR-027)
- **Доменная математика** — `mm1`/`mmc`/`erlang_c`/`littles_law` (Erlang-C,
  детект перегрузки ρ ≥ 1), `npv`/`irr`/`cagr`/`cohort_ltv`
- **Эталонные модели** — Instagram MVP (ADR-0005) и каталог №1–№5: system
  design, защита бюджета, P&L, value chain, unit economics (ADR-0006) —
  агент собирает их через MCP «проливанием» значений, без ручного копирования
  чисел (CR-013, FR-029)

Архитектурные решения — [docs/adr/](docs/adr/README.md); расчётная волна
FR — [docs/change-requests/index-cr-fr.md](docs/change-requests/index-cr-fr.md).

> Статус: активная разработка. **Кроссплатформенный проект (M7): Windows 10/11
> x64 — полная функциональность; Linux и macOS — оконный канвас, платформенные
> фичи закрываются по плану
> [M7](docs/plans/M7-crossplatform.md).** Сборки артефактов CI — на всех трёх ОС.

## Статус задач (T0–T25)

| Задача | Статус | Примечание |
|---|---|---|
| T0–T6 (M1) | ✅ | Workspace, камера, модель, карточки, culling, тамбнейлы |
| T7–T10 (M2) | ✅ | Заметки, форматирование, связи, drag-drop, вотчер |
| T11–T12 (M3) | ⏸ | Отложены после v1.0 (ACCEPTANCE.md) |
| T13–T14 (M3) | ✅ | Миникарта, поиск FTS5 |
| T15–T17 (M4) | ✅ | Desktop-режим (`--desktop`), шина событий, иконки/меню |
| T18–T19 (M4) | ❌ | Энергосбережение, MSI — не влиты |
| T20–T22 (M5) | ✅ | T20 ✅ (рантайм, WebView2-хост, LOD/снапшоты); T21 ✅ (bridge/permissions, установка drag-ом, widget_state); T22 ✅ (SDK, шаблон, примеры, [docs/WIDGETS.md](docs/WIDGETS.md)) |
| T23 | ❌ | Динамическая подсветка связей — в плане |
| T24 (M6) | ✅ | MCP / BYOK (`canvas-mcp`) |
| T25 (M6) | ❌ | SDK BYOK-виджетов — опционально, не выполнено |
| T26–T30 (M7) | ❌ | Кроссплатформенность Win/Linux/macOS — [план](docs/plans/M7-crossplatform.md) |

Расчётная волна (моделирование) — статус FR в
[docs/change-requests/index-cr-fr.md](docs/change-requests/index-cr-fr.md):
FR-013 (Numi), FR-014 (поток значений), FR-015 (единицы/queueing),
FR-018/019/027 (шаблоны, 45 шт.), FR-021 (подсказки), FR-025 (построчные
выходы), FR-016 (bottleneck-индикаторы), FR-017 (what-if v1),
FR-020 (custom templates v1), FR-029/FR-032/FR-033 (порты значений,
валидация графа, `graph_apply` — волна A роадмапа) — выполнены;
в работе: FR-044/FR-045 (анатомия ноды), FR-058–FR-060 (ui-kit волна 2);
состав и приёмка — CR-013 + ADR-0005/0006.

## Что уже работает

- Бесконечный канвас: панорамирование (средняя кнопка / Space+drag / тачпад),
  зум Ctrl+колесо к курсору (0.05–4.0), pinch, бесконечная сетка
- Файловые карточки с системными тамбнейлами (асинхронный пул потоков +
  SQLite-кэш в `~/.canvasdesk`)
- Текстовые заметки: создание двойным кликом, инлайн-редактирование
  (многострочность, выделение, Ctrl+C/X/V/A, кириллица), палитра цветов по ПКМ,
  авторост под контент + ручной resize за правый нижний угол
- Форматирование текста (markdown-подмножество, совместимо с Obsidian):
  `**жирный**`, `*курсив*`, `==подсветка==`; хоткеи Ctrl+B / Ctrl+I / Ctrl+H
- Панель настроек: летающая кнопка ⚙ (угол настраивается) или Ctrl+, —
  угол кнопки, сетка, HUD; конфиг `~/.canvasdesk/config.toml`
- Автосейв с debounce 2 с + `.bak` предыдущей версии
- Производительность: spatial index (R-tree) + culling — 5000 нод при 60 fps;
  HUD с fps/p95 по F3; нагрузочный режим `--stress N`

## Платформенная матрица (M7)

| Возможность | Windows | Linux | macOS |
|---|---|---|---|
| Канвас, заметки, связи, миникарта, поиск, undo | ✅ | ✅ | ✅ |
| Файловый вотчер | RDCW | inotify | FSEvents |
| Тамбнейлы файлов | системные | M7 (T27) | M7 (T27) |
| Drag-drop из файлового менеджера | ✅ | M7 (T28) | M7 (T28) |
| MCP (AI-клиенты) | named pipe | M7 (T29, UDS) | M7 (T29, UDS) |
| Режим «вместо рабочего стола» (`--desktop`) | ✅ | — отложено | — отложено |
| Живые виджеты (M5) | WebView2 | снапшот/плейсхолдер до T22+ | снапшот/плейсхолдер до T22+ |

## Сборка и запуск

Требования: Rust stable 1.80+; на Windows — ещё Windows SDK (MSVC).

```powershell
cargo build --workspace --release
cargo run -p canvas-app --release -- path\to\file.canvas   # без аргумента — default.canvas
cargo run -p canvas-app --release -- --stress 5000         # нагрузочная сцена
canvasdesk.exe mcp                                        # MCP-посредник (stdio; автостарт сервиса)
cargo test --workspace                                     # тесты
```

На Linux/macOS те же команды (бинарь без суффикса, пути POSIX);
инструкции по платформам — [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md).

Готовые бинари каждой сборки main — GitHub Actions → CI → артефакты
`build-<os>` (можно скачать без локальной сборки).

macOS: артефакт `build-macos-universal` — universal2 (Apple Silicon +
Intel) в виде `CanvasDesk.app` + инструкция. Без подписи Apple Developer ID
macOS ставит карантин на скачанное: распакуйте `.tar.gz` и выполните
`xattr -cr CanvasDesk.app`, затем откройте приложение (двойной клик или
`./canvasdesk` из терминала). Каждый пуш в main прогоняет смок-тест
запуска приложения прямо на macOS-раннере CI (шаг «Smoke» в логе джобы
`artifacts (macos-latest)`).

**LNK1104** (`не удается открыть файл …canvasdesk.exe`) — не ошибка кода,
а занятый бинарник: закройте запущенный CanvasDesk (включая фоновый
`--desktop`/автозапуск и осиротевшие после Ctrl+C экземпляры) и
`taskkill /f /im canvasdesk.exe`; разбор причин —
[docs/ENVIRONMENT.md §8](docs/ENVIRONMENT.md).

Один exe — весь стек (FR-008): `canvasdesk.exe` — GUI-сервис; `canvasdesk.exe mcp` —
MCP-посредник для AI-клиентов (конфиг клиента: command = `canvasdesk.exe`,
args = `["mcp"]`); при недоступном pipe посредник сам поднимает сервис — весь
стек одной командой (`--no-spawn` — отключить). Отдельный `canvasdesk-mcp.exe`
сохраняется для совместимости.

Для внешних ИИ-агентов — [пакет скиллов `skills/`](skills/): подключение
и разведка, сборка моделей, проверка чисел, what-if сценарии; полный
каталог 39 инструментов + контракт синхронности с реестром (тесты
`skills_*` в canvas-mcp, протокол — `skills/UPDATE-PROTOCOL.md`).

### Веб-версия (wasm, M8)

Тот же движок (canvas-core/render/scene, WebGPU) собирается в браузерное
wasm-приложение — крейт `canvas-web` (план
[docs/plans/wasm-port.md](docs/plans/wasm-port.md)): ввод/редактирование,
поиск, браузерное хранение (OPFS-дефолт, «Открыть с диска…», экспорт
`.canvas`). Браузер — Chromium (Chrome/Edge: нужен WebGPU); Firefox —
экспериментально (§8.3 — бонус, не таргет).

Локальный запуск (dev-сервер с hot-rebuild; детали по платформам —
[docs/ENVIRONMENT.md](docs/ENVIRONMENT.md)):

```powershell
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
cd crates\canvas-web
trunk serve --open          # http://127.0.0.1:8080
```

`wasm-bindgen-cli` trunk скачает сам; его версия обязана совпадать с
Cargo.lock (0.2.127) — при ошибке авто-скачивания (404):
`cargo install wasm-bindgen-cli --version 0.2.127 --locked`.

URL-параметры: `?canvas=имя` (канвас из OPFS), `?stress=5000`
(нагрузочная сцена в памяти), `?log=debug` (debug-лог в консоль).
Файлы из `target/dist` открывайте только через HTTP (`trunk serve`),
не `file://` — wasm-модуль не загрузится.

Релизный бандл + оптимизация (`wasm-opt -Oz`) + отчёт о размере:
`scripts/web_bundle.sh` (Linux/WSL; цель §8.8 — ≤8 МБ raw / ≤4 МБ brotli).

Публикация на GitHub Pages — workflow
[.github/workflows/pages-web.yml](.github/workflows/pages-web.yml):
веб-версия по пути `/app` + Jekyll-сборка документации из `docs/` (та же,
что у прежнего branch-деплоя) в том же артефакте. URL приложения:
`https://danku13.github.io/CanvasDesk/app/`. Необязательная чистка:
Settings → Pages → Source: «GitHub Actions» — остановит параллельные
legacy-сборки `docs/` (деплой workflow'ом работает и без переключения).
Нюанс: в частном репозитории Pages доступен на планах Pro/Team/Enterprise —
иначе используйте локальную сборку. Ограничения v1 —
[wasm-port §9](docs/plans/wasm-port.md): CJK-IME нет, WebGL2-фолбэка нет.

## Горячие клавиши

| Ввод | Действие |
|---|---|
| Средняя кнопка / Space+ЛКМ / тачпад | Панорамирование |
| Ctrl+колесо / pinch | Зум к курсору |
| Двойной клик по пустому месту | Новая заметка |
| Двойной клик по заметке | Редактирование (Enter — готово, Shift+Enter — новая строка, Esc — откат) |
| Ctrl+B / Ctrl+I / Ctrl+H | Жирный / курсив / подсветка выделенного |
| ПКМ по карточке | Палитра цветов |
| Shift + drag от порта | Связь-поток значений (value-связь для расчётов) |
| Shift + клик по пустому месту | Wheel-меню шаблонов (кольцо категорий) |
| Ctrl+P | Палитра шаблонов (фокус в поиск; Esc — свернуть) |
| F | Режим фокуса (подсветка связей выбранной ноды) |
| Ctrl+G | Сгруппировать выделенные ноды |
| Tab / Enter (в заметке mindmap) | Дочерняя / соседняя нода |
| ЛКМ за правый нижний угол | Ручной resize карточки |
| F3 | HUD (fps, p95, счётчики) / следующий результат поиска |
| Ctrl+, | Панель настроек |
| «?» (кластер ⚙/тема) | Меню помощи: документация и онбординг |
| F1 / ПКМ по канвасу → «Горячие клавиши» | Оверлей списка хоткеев (тогл) |
| Ctrl+Z / Ctrl+Y | Отмена / возврат (глубина 50) |
| Ctrl+C / Ctrl+X / Ctrl+V / Ctrl+D | Копировать / вырезать / вставить / дублировать ноды |

Полный список — [user-docs/hotkeys.md](user-docs/hotkeys.md) (F1 в приложении).

## Стек

Rust (edition 2021) · winit 0.30 · wgpu 22 · glyphon/cosmic-text · rstar ·
serde_json · rusqlite (bundled) · notify · windows-rs · tracing

## Шрифты

Встроены в бинарь (SIL Open Font License 1.1, `assets/fonts/OFL-NotoSans*.txt`):
**Noto Sans Display** — Medium 500 (базовый текст) и Bold 700 (акценты),
**Noto Sans Mono** — Numi-расчёты, результаты и код-фенсы (CR-009,
[docs/change-requests/cr-009-noto-font-pairing.md](docs/change-requests/cr-009-noto-font-pairing.md)).

## Дорожная карта

Движение — по [продуктовому роадмапу](docs/plans/product-roadmap.md)
(принят 2026-09-18 вместе с [ADR-0008](docs/adr/adr-0008-math-computing-stack.md)):
лестница ступеней, каждая — самостоятельно демонстрируемая; углубление —
только после подтверждения спроса.

| Этап | Состав | Статус |
|---|---|---|
| **M1–M6** (T0–T24) | канвас и рендер (wgpu), заметки, миникарта/поиск, desktop-режим, движок виджетов + SDK, MCP | ✅ |
| **Волна моделирования** | Numi-движок, поток значений (DAG), единицы/queueing, 45 шаблонов, онбординг (FR-013…FR-031) | ✅ |
| **Волна 0** — гигиена | cargo-deny в CI, реестр зависимостей, SBOM, триаж CR/FR (CP0) | ✅ |
| **Волна A** — композиция | FR-029 порты значений → FR-032 валидация графа → FR-033 `graph_apply` → рецепт агента (CP1–CP4) | ✅ |
| **Волна B** — аналитика | FR-016 узкие места → FR-017 v1 what-if: сценарии, дельты, сравнение (CP5–CP6) | ✅ |
| **M8** — wasm-порт | `canvas-web`: веб-версия на GitHub Pages, бандл ≤ 4 МБ brotli, wasm/mcp-wasm гейты | ✅ |
| **UI layering** (PRD-0009) | canvas-ui, ui-kit v2, scissor-клиппинг (FR-051–FR-057) | ✅ |
| **Лицензионный каркас** | AGPLv3 + CLA с автопринятием в PR (M0 архдока) | ✅ |

**Сейчас — волна V: проверка востребованности.** Догфудинг владельца и живые
демо архитекторам/аналитикам; гейт Go — **≥ 5 внешних пользователей сами
построили модель и вернулись к ней второй раз**. Параллельно в работе:
FR-044/FR-045 (анатомия ноды — источники данных, calc-trace), миграция
ui-kit волна 2 (FR-058–FR-060), FR-037 (MCP-wasm верификация), FR-043
(универсальный импорт архитектур).

**Волна S — после Go, по продуктовым триггерам:** статистика и доверительные
интервалы (M2), сценарный worker + freeze/сравнение сценариев (M3, FR-017 v2),
параллелизм и Monte Carlo (M4/M5), композитные шаблоны (R4/FR-020 v2),
платформенные фичи Linux/macOS (M7), BYOK (T25).

Организационный трек — подготовка к реестру РФ (ПП № 1236): лицензионный
каркас закрыт; чек-лист архдока §8.4 (правообладатель, товарный знак,
описание функциональных характеристик, SBOM в дистрибутиве) — по готовности
к заявке, волны разработки не блокирует.

История проектирования волн — [docs/devlog/](docs/devlog/) (M5: как
строился движок виджетов).

Подробности: [docs/SPEC.md](docs/SPEC.md) — спецификация,
[docs/TASKS.md](docs/TASKS.md) — план инфраструктурной волны (T0–T22, M1–M5),
[docs/RECIPES.md](docs/RECIPES.md) — рецепты shell-интеграции Windows,
[docs/ACCEPTANCE.md](docs/ACCEPTANCE.md) — программа ручной приёмки.

Пользовательская документация и FAQ — [user-docs/](user-docs/)
(публикуется на GitHub Pages; инструкция публикации —
[user-docs/README.md](user-docs/README.md)). Внутренние доки по
интерфейсным объектам — [docs/interface-objects/](docs/interface-objects/)
(нода, связь, миникарта, поиск).

---

## Лицензия и правила игры (License & Rules)

**Copyright © 2026 danku13.** Проект распространяется под **[GNU AGPLv3](LICENSE)**. Модель простая: код бесплатный и открытый — навсегда; судьба проекта (включая лицензию) — в руках владельца.

| | |
|---|---|
| 💚 **Код бесплатен** | Весь код — под [GNU AGPLv3](LICENSE): свободно используйте, изучайте, изменяйте и распространяйте. Никаких «open core» хитростей — сообщество получает весь проект целиком. |
| 🔒 **Коммерческие права у владельца** | Исключительные права на проект и **право изменять его лицензию в будущем** сохраняет владелец (**danku13**): можно выпустить коммерческую или проприетарную редакцию, применить dual-licensing, закрыть проект. Код, уже выпущенный под AGPLv3, для сообщества от этого не закроется. |
| ✍️ **PR = подписание CLA** | Отправляя Pull Request, вы **автоматически принимаете [CLA](CLA.md)**: авторство вашего кода остаётся за вами, но право лицензировать вклад в будущем переходит владельцу. Это защищает проект от юридических рисков при смене лицензии. |

### Мини-FAQ (License FAQ)

**Можно ли использовать CanvasDesk в коммерческих целях? / Can I use it commercially?**

Да — на условиях AGPLv3: коммерческое использование законно, пока соблюдаются условия лицензии (исходники изменений открываются под AGPLv3; для сетевых сервисов действует §13 AGPL — исходники версии, доступной пользователям, должны быть открыты). Разрешение владельца для этого не требуется.

*Yes — under AGPLv3 terms: commercial use is legal as long as you comply with the license (your modifications stay under AGPLv3; for network services §13 applies — the source of the version you serve must be available). No separate permission from the owner is needed.*

**Условия AGPLv3 мне не подходят (закрытая интеграция, OEM, white-label, SaaS без открытия исходников) / AGPLv3 doesn't fit my case**

Напишите владельцу — обсудим **коммерческую лицензию** под ваш сценарий: Telegram [@danku13](https://t.me/danku13) или [danku13@yandex.ru](mailto:danku13@yandex.ru).

*Contact the owner to discuss a **commercial license** for your scenario: Telegram [@danku13](https://t.me/danku13) or [danku13@yandex.ru](mailto:danku13@yandex.ru).*

**Инвестиции и партнёрство / Investment & partnership**

Компании и инвесторы: если хотите обсудить коммерческое использование проекта, инвестиции или партнёрство — свяжитесь любым способом: Telegram [@danku13](https://t.me/danku13), [danku13@yandex.ru](mailto:danku13@yandex.ru).

*Companies and investors: to discuss commercial use, investment or partnership — reach out via Telegram [@danku13](https://t.me/danku13) or [danku13@yandex.ru](mailto:danku13@yandex.ru).*

**Что происходит с моим вкладом? / What happens to my contribution?**

Авторство — ваше (git-история сохранит), но по [CLA](CLA.md) владелец получает право лицензировать вклад на любых условиях в будущем. Правила участия — в [CONTRIBUTING.md](CONTRIBUTING.md); механика автоподписания — чекбокс в [шаблоне PR](.github/PULL_REQUEST_TEMPLATE.md) + [cla-check](.github/workflows/cla-check.yml).

*Authorship stays with you (git history preserves it), but per the [CLA](CLA.md) the owner may license your contribution on any terms in the future. Contribution rules — [CONTRIBUTING.md](CONTRIBUTING.md).*
