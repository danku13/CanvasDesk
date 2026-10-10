# FR-107: Мультиканвас C4 — чип активного канваса, title, онбординг, чистка панели

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №1–54), агент (реализация)
- **Источник:** high-level issue #14 + sub-issue #8 (волна C4, после C3 #7, параллельно с C5 #9); план v2.1 §3.2
- **Связанные задачи:** FR-103 (контракты C0), FR-104 (конвейер C1), FR-105 (хранилище/миграция C2), FR-106 (менеджер C3 — чип его вход); C5 #9 (desktop-данные менеджера, приёмка)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация)

---

## Описание (What)

Волна C4 закрывает «повседневный» слой мультиканваса вокруг менеджера C3:
двухзонный чип активного канваса в верхней левой зоне GPU-слоя (имя —
инлайн-ренейм, иконка списка — вход в менеджер, №21c), `document.title`
вкладки «Имя — CanvasDesk» (№28a), dirty-сигнал без точки — стойкий значок
ошибки сохранения на чипе только при сбое (№29b), камера канваса
(зум+центр) в localStorage с переносом ключа при ренейме (№12/№30b),
финальная карточка онбординга «одна работа — один канвас» с CTA
«Создать канвас» (№19) и чистка DOM-панели `#w6-toolbar` — «Недавние» и
«Экспорт .canvas» уходят, остаются Тур / «Открыть с диска…» / «Экспорт
HTML» (№37b).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-app `canvas_chip_ui` (новый) | `CanvasChipState` (rename-буфер, save_failed), чистые `chip_layout`/`chip_hit` (`ChipHit::{Name, ListIcon, None}`, draw==hit), `ChipRenameBuffer` — 13 тестов | FR-107 |
| canvas-app `app`/`handler`/`input`/`overlays`/`ui_registry` | hit-зоны chip-name/chip-list в реестре поверхностей, клики/клавиатура ренейма (Esc/Enter/blur), disk-режим «только просмотр» (`CanvasActiveKind`), отрисовка KitPalette/tokens, сигнатура кадра | FR-107 |
| canvas-app `onboarding_ui` | `ONBOARDING_STEPS` — 9 карточек, `CtaAction::CreateCanvas` (CTA-механика минимально: флаг `cta` у шага), `CHOICE_STEP=7` — спец-ветки финала FR-028 переключены с `is_last` | FR-107 |
| canvas-app `i18n` | +9 ключей RU/EN (чип: rename/manager/disk/save_error hint+toast; онбординг: step9 title/body + multicanvas_cta) | FR-040 |
| canvas-core `workspace` | `CameraSnapshot` + `encode_camera`/`decode_camera` («x;y;zoom», округление, не-finite→0) — round-trip/битая строка/санитизация, 3 теста | FR-107 |
| canvas-scene `scene` | `autosave_if_due() -> Option<bool>` (результат сейва — сигнал №29b, `None` = сейв не был назначен) | FR-107 |
| canvas-web `title_sync` (новый) | `document_title` (чистая, 3 теста) + тонкая wasm-обёртка `Document::set_title`; хук — `web_state::set_active` (тот же, что `?canvas=`), ренейм активного переводит заголовок в `run_canvas_op` | FR-107 |
| canvas-web `camera_web` (новый) | localStorage по ключу `camera_key_for` (C0): `save`/`load` + `install_flush` (visibilitychange→hidden + pagehide → `CameraFlushRequested`); стартовое восстановление — `App::apply_startup_camera` в app_spawn | FR-107 |
| canvas-web `web_requests` | `CanvasCameraSave`/`CanvasCameraLoad`→`CanvasCameraRestored`, `CanvasActiveKind`; ренейм активного — переезд web-зеркала (set_active: лок+URL+title, недавние) | FR-107 |
| canvas-web `toolbar`/index.html + SDK-туры | №37b: кнопки `#btn-recent`/`#btn-export` и `set_recent_label` ушли; CSS-эллипсис удалён; туры (`toolbarTourScenario` RU/EN, `firstRunInlineScenario`, canvasdesk-tour.js, standalone) — «три кнопки»→«две», шаги recent/export удалены | FR-107 |
| Пользовательское поведение | смена канваса видна во вкладке и без URL; камера переживает перезагрузку; панель хранилища короче (менеджер — в чипе) | — |

## Анализ (Root Cause)

1. До C4 вход в менеджер — только кнопка «Недавние» DOM-панели (временная,
   FR-106 §Точки входа); постоянный вход — решение №21c: двухзонный чип
   активного канваса (топ-лево свободен: топ-центр — поиск 460px, топ-право
   — DOM-панель).
2. Идентичность «одна работа = один канвас» не доводилась до пользователя:
   онбординг кончался финалом FR-028, вкладка браузера не показывала имя
   канваса, камера терялась при перезагрузке (№28a/№12/№19).
3. Решение №37b: «Недавние»/«Экспорт .canvas» дублируют менеджер — панель
   сокращается до трёх кнопок (Тур / Открыть с диска… / Экспорт HTML);
   recent.rs/IndexedDB остаются (фолбэк битой ссылки №31c).

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Чип (№21c) | `canvas_chip_ui.rs` + app | верхняя ЛЕВАЯ зона; зона (а) имя — одиночный клик = инлайн-ренейм (буфер, Esc/Enter/потеря фокуса; валидация `validate_canvas_name` + коллизия по entries), применение — через конвейер `CanvasOp::Rename` менеджера (перенос `.bak` и ключа камеры уже там); зона (б) иконка списка — `CanvasManagerOpen`; hover/focus KitState/Focused, цвета KitPalette, геометрия tokens; `CHIP_MAX_W 156` — 2px зазор до поиска в минимальном G4-вьюпорте; сдвиг за развёрнутый док палитры (UR-003) |
| Disk-режим чипа | app + `CanvasActiveKind` | `ActiveKind::Disk` — «только просмотр» с тултипом `canvas.chip.disk_hint`, ренейм-буфер гасится (менеджер не ренеймит дисковые файлы) |
| document.title (№28a) | `title_sync.rs` + `web_state::set_active` | чистая `document_title(Option<&str>)`: `Some`→«Имя — CanvasDesk» (display_name без `.canvas`), `None`/пустое→«CanvasDesk»; применяется в едином крючке смены активного (`set_active`, рядом с url_sync), в т.ч. при открытии/создании/ренейме активного; статический `<title>` — «CanvasDesk» |
| Dirty-сигнал (№29b) | `scene.rs` + handler + чип | `autosave_if_due -> Option<bool>`; `Some(false)` (ошибка) → `chip.save_failed=true` + стойкий значок ⚠ на чипе + разовый тост `canvas.chip.save_error_toast`; `Some(true)` снимает; `None` — ничего. Постоянных dot-индикаторов сохранённости в UI нет |
| Камера (№12/№30b) | core + `camera_web.rs` + app | сериализация «x;y;zoom» (`{:.2};{:.2};{:.3}`, не-finite→0) — round-trip + битая строка→`None` (тихий дефолт); сохранение: уход с канваса (`on_open_scene` → `CanvasCameraSave`) и выгрузка/скрытие страницы (`install_flush`: visibilitychange→hidden + pagehide → `CameraFlushRequested` → App немедленно отвечает снимком; запись синхронная — успевает до ухода). Загрузка: `OpenScene` → `CanvasCameraLoad` → `CanvasCameraRestored`; web-старт (сцена напрямую) → `App::apply_startup_camera` в app_spawn (`?stress` — сцена синтетическая, камера дефолт). Ренейм НЕ сохраняет — ключ переносится готовым `CanvasMoveCameraKey` (C3) тем же конвейером |
| Онбординг-шаг (№19) | `onboarding_ui.rs` | финальная (9-я) карточка «одна работа — один канвас» + CTA `CtaAction::CreateCanvas` («Создать канвас» → `CanvasManagerOpen`, завершает тур; «Готово» остаётся); спец-ветки финала FR-028 переведены с `is_last` на `CHOICE_STEP=7` |
| Чистка панели (№37b) | index.html + toolbar.rs + SDK | удалены `#btn-recent`/`#btn-export` (разметка, CSS, глю `set_recent_label` и все вызовы); остаются `#btn-tour`/`#btn-open`/`#btn-export-html`; туры RU/EN: «три кнопки»→«две», шаги recent/export удалены, финал — «список канвасов — чип в левом верхнем углу»; `firstRunInlineScenario` locate-toolbar — «открыть с диска, экспорт HTML»; standalone-test.html и регресс-тест тура (3 шага, 1/3) обновлены; аудиты (`web_layout_audit.py`, `web_touch_targets_audit.py`) — эллипсис-кейсы удалены; recent.rs/IndexedDB НЕ тронуты (фолбэк №31c) |

### Нумерация онбординга

`ONBOARDING_STEPS` — массив карточек карусели (индексы 0–8); welcome-экран
выбора роли — отдельная поверхность ДО карусели. Сквозная нумерация issue
(«шаг 10» = welcome + 8 карточек + шаг финальный) в коде = 9-й элемент
массива (`step9.*` в i18n), `CHOICE_STEP=7` — финал FR-028.

## Точки входа (Entry Points)

Чип (иконка списка) — постоянный вход в менеджер канвасов (кнопка «Недавние»
ушла, №37b); CTA онбординга → тот же `CanvasManagerOpen`; recent-фолбэк
битой ссылки №31c — без изменений. index-cr-fr — этим коммитом;
interface-objects/user-docs — C5 (приёмка).

## Проверка (Verification)

- `cargo test --workspace` — **3081 passed / 0 failed** (26+ новых:
  canvas_chip_ui×13, workspace-камера×3, title_sync×3, app-чип×3,
  ui_registry×2, onboarding×2 + расширен button_hit_tests CTA-веткой;
  тест scene адаптирован к `Option<bool>`);
- `cargo clippy --workspace -- -D warnings` — зелёный;
- `cargo fmt --check` — зелёный;
- `scripts/wasm_gate.sh --check` — OK (компиляция canvas-web под
  wasm32 чистая);
- i18n-инвариант FR-040 (RU/EN полнота, +9 ключей C4);
- WASM L2 не выполнялся (окружение: нет wasm-bindgen CLI, 2 ядра) — ручные
  дым-сценарии владельцем (Chromium, `?log=debug`):
  1. **Чип-ренейм**: клик по имени чипа (топ-лево) → поле; Esc — отмена,
     Enter — применяется (менеджер и вкладка показывают новое имя);
     коллизия — запрещается с тостом; дисковый файл (Открыть с диска… —
     затем снять доступ) — чип «только просмотр» с тултипом;
     клик по иконке списка → менеджер.
  2. **title**: открыть канвас → вкладка «Имя — CanvasDesk»; `?canvas=`
     после перезагрузки — то же; ренейм активного — заголовок переезжает;
     без канваса — «CanvasDesk».
  3. **Камера**: зум/панорама → переключиться на другой канвас и обратно
     (или перезагрузить страницу) — камера восстановлена; ренейм — камера
     следует за новым именем (ключ перенесён).
  4. **Онбординг-шаг**: чистый профиль (или меню «?» → повтор онбординга) →
     последняя карточка «одна работа — один канвас», CTA «Создать канвас»
     открывает менеджер; «Готово» завершает без него.
  5. **Панель из 3 кнопок**: топ-право — Тур / «Открыть с диска…» /
     «Экспорт HTML»; тур панели — 3 шага (без «Недавние»/«Экспорт
     .canvas»); RU и EN; тёмная/светлая тема.
  6. **Dirty-сигнал**: дисковый канвас → в DevTools Application снять
     permission хэндлу (или отвлечь вкладку) → правка + автосейв падает →
     на чипе стойкий ⚠ + тост; после успешного сохранения ⚠ исчезает.

## История изменений (Changelog)

- 2026-10-10 — агент (subagent MC-C4, сессия прерывалась): чистые части
  (8fe51ae: сериализация камеры core + canvas_chip_ui, 13 тестов),
  интеграция (3795172: hit-зоны/ренейм/онбординг-шаг 10/dirty-сигнал/хуки
  камеры, AppEvent-блок `// FR-107 (C4)`), web-часть (0879e2a: title_sync +
  camera_web + чистка DOM-панели №37b + SDK-туры). Гейты зелёные, FR-107,
  worklog.

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 (решения №12/№19/№21c/
  №28a/№29b/№30b/№37b).
- FR-103/FR-104/FR-105/FR-106 — контракты/конвейер/хранилище/менеджер;
  `canvas_manager_ui.rs` — паттерн чистого UI-модуля и конвейер ренейма;
  `url_sync.rs` — паттерн тонкого web-хука; `fs_folder::install_watch` —
  паттерн DOM-листенеров.
- Коммиты: 8fe51ae, 3795172, 0879e2a (ветка `wave/mc-c4`).
- GitHub: danku13/CanvasDesk issues #8 (эта волна), #14 (high-level).
