# FR-108: Мультиканвас C5 — тонкий нативный слой (desktop) и сквозная приёмка

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца (ручной прогон Windows — за владельцем)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №1–54), агент (реализация)
- **Источник:** high-level issue #14 + sub-issue #9 (волна C5, после C0–C3; параллельна C4); план v2.1 §1 «Desktop — тонкий слой (№20)», §4 волна C5
- **Связанные задачи:** FR-103 (контракты C0), FR-104 (конвейер C1), FR-105 (хранилище C2), FR-106 (менеджер C3); C4 #8 (чип/title/онбординг — web-вход в менеджер)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация + финализация)

---

## Описание (What)

Тонкий нативный слой мультиканваса (решение №20): менеджер канвасов на
десктопе живёт поверх **недавних `.canvas`-файлов** (мини-список в
`config.toml`), «Открыть…»/«Создать» идут через **нативные файловые диалоги**
Windows (№34a, IFileOpenDialog/IFileSaveDialog), вход в менеджер — пункт
контекстного меню канваса. Web-режим не меняется (конвейер FR-104/105/106);
ренейм/шаблон/дубликат/удаление/миграция на нативе — сознательное ограничение
волны (файлами владеет ОС). Плюс сквозная приёмка: чек-лист 6 сценариев
issue #9 в `docs/ACCEPTANCE.md` (§13) с колонками [web | desktop].

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-core `settings` | `Settings.recent: Vec<String>` (serde default — старые конфиги совместимы), cap 10 в `normalize` | FR-108 |
| canvas-core `recent_files` (новый) | чистые `push_recent`/`decay_missing`/`display_name_of`/`file_name_of` (cap/dedup/свежайший первым; сравнение путей на Windows регистронезависимо); 7 тестов | FR-108 |
| canvas-shell `desktop/dialogs` (новый) | IFileOpenDialog/IFileSaveDialog (№34a): фильтр `*.canvas`, `SetDefaultExtension` + safety-net `ensure_canvas_extension`, FOS_OVERWRITEPROMPT; cfg(windows) + SAFETY, не-Windows — заглушка warn+None; 2 чистых теста | FR-108 |
| canvas-app `app` | native-режим менеджера (`native=cfg!(not(wasm32))`): entries из недавних (`refresh_native_recent`), «Открыть…» в слоте import и пустого состояния, «Создать» = save-диалог → файл+`.bak` → открытие; `record_recent_canvas` (absolutize/canonicalize со срезом `\\?\`) из `on_open_scene` и main.rs (CLI-путь/default.canvas; `--stress` не пишет); F2/дабл-клик-ренейм отключены; `on_open_scene` синхронизирует вотчер+поисковый индекс (паттерн старта main.rs); 3 нативных app-теста | FR-108 |
| canvas-app `canvas_manager_ui` | `CanvasManagerState.native` (платформа сборки), `StorageRowMode::Files` (без кнопки переезда), нативная раскладка (без «Из шаблона…» и ряда кнопок строки); 2 теста раскладки | FR-108 |
| canvas-app `lib`/`input`/`overlays`/`ui_registry` | пункт «Менеджер канвасов…» в голове контекстного меню канваса (12 пунктов), нативные ветки draw/hit/ui-sig | FR-108 |
| canvas-app `i18n` | +4 ключа RU/EN (`canvas.manager.open`, `canvas.storage.disk_files`, `menu.canvas_manager`, `canvas.native.create_failed_toast`) | FR-040 |
| Пользовательское поведение (desktop) | менеджер открывается из ПКМ-меню, показывает недавние файлы, открывает/создаёт файлы через системные диалоги; config.toml накапливает недавние | user-docs (см. Точки входа) |

## Анализ (Root Cause)

1. Менеджер C3 (FR-106) и весь конвейер — web-only: `WebRequest` дренажирует
   только `TourAwareApp` (нативной обёртки нет), источники entries — браузерные
   хранилища. На нативе недавние отсутствовали вовсе (`recent.rs` — IndexedDB,
   web-only), файловых диалогов не было («Открыть с диска…» — web-пикер).
2. Десктоп по решению №20 — тонкий слой: workspace-хранилище не строится,
   файлами владеет ОС; менеджеру нужен только честный источник строк
   (недавние) и два действия (открыть/создать).

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Недавние в конфиге | `settings.rs` + `recent_files.rs` (core) | `recent: Vec<String>` (абсолютные пути, serde default), `RECENT_CAP=10`; push — свежайший первым, dedup (Windows — регистронезависимо), cap; decay несуществующих — предикатом (без I/O в чистой функции); `display_name_of` = file_stem без `.canvas` (семантика `workspace::display_name`; ручной разбор `/`+`\\` — поведение одинаково на всех ОС, тесты на Linux гоняют Windows-пути); `normalize` клампит cap при загрузке |
| Обновление недавних | `App::record_recent_canvas` (app.rs) | absolutize → `push_recent` → `decay_missing` (только что открытый переживает decay — default.canvas первой сессии может ещё не существовать) → `Settings::save` сразу (деградация — warn) → `refresh_native_recent`. Вызовы: `on_open_scene` (все пути смены сцены: диалог, строка менеджера, drag-drop, CLI) + main.rs на старте (`--stress` исключён). На web — no-op |
| Нативные диалоги | `canvas-shell/desktop/dialogs.rs` | см. Влияние; дефолтное имя save-диалога — `auto_name` контрактов C0 (№39c, «Canvas N»), родитель — raw HWND окна (конвертация в типизированный HWND внутри canvas-shell, свою зависимость `windows` в canvas-app не тащим — идиома dragdrop::install); модальны в UI-потоке (паттерн TrackPopupMenu menu.rs — winit event loop заморожен на время диалога, осознанное решение) |
| Менеджер на недавних | `app.rs` + `canvas_manager_ui.rs` | `open_canvas_manager`: native → `StorageRowMode::Files` + `refresh_native_recent` (WebRequest-конвейер web-слоя НЕ дергается — на нативе запросы гнили бы в очереди); открытие строки/Enter/дабл-клик = чтение файла диска → `on_open_scene` (вотчер/поиск/недавние — общей точкой); слот import и «Открыть файл с диска…» пустого состояния = нативный open-диалог; «Создать» = save-диалог → `save_with_backup` (`.bak` прежней версии, SPEC §9) → открытие; пустое состояние №23a — CTA «Создать» + «Открыть файл с диска…» |
| Тонкий слой UI | `canvas_manager_ui.rs` + `overlays.rs` + `ui_registry.rs` | native-раскладка: «Из шаблона…» скрыт (rect 0×0), ряд кнопок выбранной строки скрыт (ренейм/удаление/дубликат/экспорт — зона ОС), «Открыть…» встаёт сразу за «Создать»; draw==hit (хиты тех же id); ui-sig включает `Files` |
| Ренейм на нативе | `app.rs` | F2 и двойной клик по имени НЕ входят в инлайн-ренейм (клавиша глотается; зона имени глотается как клик по строке) — файлами владеет ОС |
| Вход в менеджер | `lib.rs` + `input.rs` | пункт «Менеджер канвасов…» — голова контекстного меню пустого канваса (12 пунктов; до C4 web-вход оставался дублем кнопки «Недавние»). Выбор: минимум новых поверхностей, без хоткея, НЕ трогая web DOM-панель (её чистит C4) |
| Web-режим не сломан | `app.rs` (тесты) | web-тесты менеджера идут через `manager_app_with` с `native=false` (моделируют web-конвейер FR-104/105/106), нативные ветки — отдельные `manager_native_*` |

### События (нативный обратный канал — не потребовался)

Бриф предлагал `AppEvent`-варианты (`NativeOpenDialog`/`NativeCreateDialog`/
`RecentFilesChanged`) с обратным каналом по образцу ThumbService/McpPipe.
Реализация выбрала **синхронные модальные диалоги в UI-потоке** (паттерн
ShellExecute/TrackPopupMenu `menu.rs`): файловые диалоги локальны и быстры,
«обратный канал» вырождается в прямое возвращаемое значение, EventLoopProxy-
воркер не появляется. `RecentFilesChanged` тоже не нужен: недавние — поле
`Settings`, зеркала менеджера перестраивает `refresh_native_recent` в той же
точке мутации. Драг-дроп и CLI-путь не затронуты (остаются рабочими).

### Сознательные ограничения волны (тонкий слой, №20)

- **Ренейм** — на нативе отключён (F2/дабл-клик): имя файла меняет ОС
  (Explorer); инлайн-ренейм web-хранилища неприменим. TODO будущей волны:
  нативный rename-диалог с переносом ключа камеры (№12/№30b).
- **Шаблон/дубликат/импорт/удаление/миграция** — отсутствуют: файлами
  владеет ОС (№20). «Импорт» заменён «Открыть…»; строка хранилища — режим
  `Files` без кнопки переезда.
- **Диалоги — только Windows** (№34a): Linux/macOS натив — заглушка warn+None
  (CLI-путь и drag-drop остаются рабочими). Web — свои пикеры (WebRequest),
  сюда не доходит.
- **Модальность**: UI-поток заморожен на время диалога (задокументировано
  в шапке модуля).

## Точки входа (Entry Points)

- `docs/ACCEPTANCE.md` — новая секция «Мультиканвас (волны C0–C5)» (§13:
  6 сквозных сценариев [web|desktop] + матрица ручной приёмки) — этим коммитом.
- `docs/change-requests/index-cr-fr.md` — строка FR-108 — этим коммитом.
- `user-docs/interface.md` — секция «Менеджер канвасов (десктоп)» — этим
  коммитом (только desktop-поверхности; web-тексты про чип — зона C4 #8).
- `docs/interface-objects/` — новый объект не заводился (менеджер — оверлей
  C3 FR-106; desktop-ветка задокументирована здесь); если владелец попросит
  отдельный interface-object — следующая волна.

## Проверка (Verification)

- `cargo test --workspace` — **3072 passed / 0 failed** (15 новых: recent_files×7,
  settings×1, dialogs×2, canvas_manager_ui×2, app×3; web-тесты менеджера
  переведены в web-режим — `native=false` в хелпере);
- `cargo fmt --check`; `cargo clippy --workspace -- -D warnings` — зелёные;
- `scripts/wasm_gate.sh --check` — OK (web не сломан: native-ветки за
  `cfg!`, canvas-shell в wasm-сборке отсутствует);
- **cfg(windows)-диалоги**: проверены кросс-компиляцией — изолированный
  стенд с тем же `windows` 0.62.2 (features `Win32_UI_Shell` +
  `Win32_UI_Shell_Common` + `Win32_System_Com` + `Win32_Foundation`)
  включает `dialogs.rs` дословно и собирается под `x86_64-pc-windows-msvc`
  (полный `cargo check -p canvas-shell --target msvc` на Linux-машине
  невозможен: `libsqlite3-sys` требует MSVC `lib.exe` — окружение, не код);
  рантайм-поведение COM-диалогов (модальность, фильтр, overwrite-prompt)
  проверяется только живым прогоном на Windows;
- RED-верификация мутациями (сессия реализации): фильтр `*.canvas`, дефолтное
  расширение, dedup/cap/decay, натив-ветки раскладки — тесты ронялись
  мутациями, код восстановлен;
- **WASM L2 (браузерный стенд) не выполнялся** (нет wasm-bindgen CLI,
  2 ядра/4 ГБ, стенд занят параллельной волной) — web-часть покрыта
  юнит-тестами web-режима + L0-гейтом; ручные сценарии Chromium —
  `docs/ACCEPTANCE.md` §13 (владелец);
- **Ручной прогон натива — за владельцем**: Windows-диалоги, ПКМ-вход,
  недавние в config.toml, оба окна/темы/локали — `docs/ACCEPTANCE.md` §13.

## История изменений (Changelog)

- 2026-10-10 — агент (subagent MC-C5, коммит 039d471): ядро волны —
  Settings.recent + recent_files, dialogs (IFileOpen/IFileSave), native-режим
  менеджера, ПКМ-вход, 15 тестов; кросс-проверка msvc-компиляцией.
- 2026-10-10 — агент (subagent MC-C5-FIN, коммит 859ce24): догон внешнего
  интеграционного теста контекстного меню (12 пунктов после
  «Менеджер канвасов…»; юнит-тест lib.rs был обновлён, integration_groups —
  нет), повторные гейты (3072/0), изолированный msvc-стенд dialogs.rs,
  FR-108, ACCEPTANCE §13, user-docs, индексы, worklog-и.

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 (решения №20, №34a,
  №39c, №23a, №51a; §4 волна C5).
- FR-103/FR-104/FR-105/FR-106 — контракты/конвейер/хранилище/менеджер;
  `crates/canvas-shell/src/desktop/menu.rs` — паттерн Win32/COM-модуля;
  `crates/canvas-web/src/recent.rs` — образец семантики недавних (cap/порядок).
- GitHub: danku13/CanvasDesk issues #9 (эта волна), #14 (high-level).
- Приёмка: `docs/ACCEPTANCE.md` §13 «Мультиканвас (волны C0–C5)».
