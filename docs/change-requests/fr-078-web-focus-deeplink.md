# FR-078: Deep-link ?focus= на ноду в веб-версии

- **Статус:** в работе
- **Тип:** FR
- **Приоритет:** желательно (J6 «пришлите это»: ссылка ведёт глаз читателя)
- **Владелец:** агент-сессия (Super Z), по запросу владельца
- **Источник:** `docs/dev-researches/archify-transfer-analysis.md` (карта переноса, T3) — практика Archify «стабильные ссылки» (`#focus=<id>`, `#route=`, `#view=`)
- **Связанные задачи:** J6/GAP-01 (асинхронный показ), FR-076 (HTML-артефакт — альтернативный способ «пришлите это»), W6 `?canvas=` (url_params), FR-049 `?template=` (паттерн отложенного применения)
- **Создан:** 2026-09-28
- **Обновлён:** 2026-09-28

## Описание

На конкретную ноду канваса нельзя сослаться: `?canvas=`/`?template=` открывают
канвас/схему целиком, но получатель ссылки сам ищет нужную ноду. В сценарии
защиты («посмотри вот на этот узкий места/итог») ссылка должна вести глаз:
открывать канвас с камерой, центрированной на ноде, с выделением. Перенос
практики Archify — viewer восстанавливает состояние из URL (`#focus=<id>`).

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| веб-URL | Новый параметр `?focus=<node-id>` (санитизация id) | `crates/canvas-web/src/url_params.rs` |
| приложение | `App::pending_focus` + применение на первом кадре (паттерн `?template=`) | `crates/canvas-app/src/app/handler.rs` |
| i18n | Тост «нода не найдена» (RU/EN) | `crates/canvas-app/src/i18n.rs` |
| user-docs | Раздел о ссылке на ноду в доке обмена | `user-docs/export.md` |

## Анализ

- `url_params.rs:223 parse_query` — расширяемый парсер (W5→W6→FR-049→FR-055);
  строковые параметры идут через `string_param` + санитизация; у `?focus=`
  значения — id нод (`n_product`, `mm1`, …): буквы/цифры/`_`/`-`, ≤ 64.
- Паттерн отложенного применения: `App::pending_scheme` (app.rs:1106) +
  первый кадр `handler.rs:74` (RedrawRequested — вьюпорт/камера готовы).
  Deep-link повторяет его: `camera.set_center` (camera.rs:38, тот же вызов,
  что у минимапы T13) + `Selection::Node(index)`.
- Неизвестный id — мягкий отказ тостом (как `GALLERY_UNKNOWN` у `?template=`):
  страница обязана открыться при любом URL (правило обёртки url_params).

## Требуемые изменения

1. `url_params.rs`: поле `focus: Option<String>` + `sanitize_node_id`
   (non-empty, ≤ 64, alnum/`_`/`-`; Unicode-буквы допустимы — id бывают
   кириллическими) + парсинг в `parse_query` + тесты (валидный/битый/
   проценты/комбинация с `?canvas=`).
2. `canvas-app/src/app.rs`: поле `pending_focus` + `set_pending_focus`
   (сеттер для canvas-web, зеркалит `set_pending_scheme`).
3. `canvas-app/src/app/handler.rs`: применение на первом кадре — поиск по id,
   `camera.set_center(центр ноды)`, `Selection::Node`, лог; неизвестный id —
   warn + тост `TOAST_FOCUS_NOT_FOUND`.
4. `canvas-app/src/i18n.rs`: ключ + RU/EN строки (таблицы полны — тест
   `tables_are_complete_and_consistent`).
5. `canvas-web/src/app_spawn.rs`: `app.set_pending_focus(params.focus)` после
   `set_pending_scheme`.
6. `user-docs/export.md`: раздел «Ссылка на ноду в веб-версии».
7. `docs/change-requests/index-cr-fr.md`: строка FR-078.

## Точки входа

- `user-docs/export.md` — дополнен (п. 6).
- `crates/canvas-web/src/app_spawn.rs` докмодуль «Аргументы запуска» —
  упоминание `?focus=` (комментарий при проводке).

## Проверка

- [x] `cargo test -p canvas-web` (нативные rlib-тесты url_params: 2 новых
  теста + существующие 15 с полем focus).
- [x] `cargo check -p canvas-web --target wasm32-unknown-unknown`,
  `cargo check -p canvas-app`.
- [x] `cargo test -p canvas-app` — 405 тестов (кроме двух прекоммитных
  падений scheme_gallery_ui, воспроизводятся на чистом дереве).
- [x] `cargo clippy -p canvas-app -p canvas-web --target wasm32-unknown-unknown -- -D warnings`, `cargo fmt --check`.
- [ ] L2 браузерный стенд — вне сессии; ручная проверка: открыть
  `…/app/?canvas=<имя>&focus=<id ноды>` → камера на ноде, нода выделена;
  `?focus=нет-такой` → тост «Нода не найдена», канвас открыт.

## История изменений

- `2026-09-28` — агент: создан документ (анализ, изменения, проверка), статус `в работе`.
- `2026-09-28` — агент: реализовано (url_params + pending_focus + первый кадр + i18n + user-docs); статус `выполнено`.

## Источники истины

- `crates/canvas-web/src/url_params.rs` (`focus`, `sanitize_node_id`, тесты).
- `crates/canvas-app/src/app/handler.rs:90` (применение на первом кадре).
- `crates/canvas-render/src/camera.rs:38` (`set_center` — прецедент минимапы T13).
- Archify: `viewer/focus.js` — прецедент восстановления фокуса из URL-фрагмента.
