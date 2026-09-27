# FR-076: Экспорт самодостаточного HTML («артефакт защиты»)

- **Статус:** в работе
- **Тип:** FR
- **Приоритет:** критично (закрывает GAP-01, P1)
- **Владелец:** агент-сессия (Super Z), по запросу владельца
- **Источник:** `docs/dev-researches/archify-transfer-analysis.md` (карта переноса, T1) + `docs/market-researches/04-cjm-audit-plan.md` §J6/§реестр (GAP-01: «пришлите это» → нечего слать)
- **Связанные задачи:** GAP-01 (P1), PRD-0007 §10 (отказ от PNG-экспорта lineage — не конфликтует: HTML, не PNG), FR-017/FR-064 (what-if таблица сравнения), FR-048 (режим защиты — живое окно, асинхронный случай закрывает этот FR)
- **Создан:** 2026-09-28
- **Обновлён:** 2026-09-28

## Описание

Нет артефакта для асинхронного показа модели: защита расчёта возможна только
живым окном приложения (FR-048), стейкхолдер, попросивший «пришлите это»,
ничего не получает (GAP-01). Требуется экспорт текущего канваса в
самодостаточный HTML-файл: офлайн-артефакт с SVG-снимком канваса, вычисленными
значениями нод, подписями значений на value-рёбрах и таблицей сравнения
what-if сценариев. Идея перенесена из Archify (практика «portable by
default»): результат — один файл, который можно открыть в любом браузере и
переслать без установки приложения. Выявлено владельцем (запрос на анализ
переноса из Archify, сессия 2026-09-28).

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| экспорт | Новый артефакт: HTML рядом с `.canvas` (натив), download-blob (веб) | `docs/market-researches/04-cjm-audit-plan.md` §J6 (GAP-01 закрывается частично: HTML; PNG — отдельный кандидат T4) |
| ядро | Новый чистый модуль `canvas-core/src/export_html.rs` (без I/O, без GPU, wasm-совместимый) | AGENTS.md §архитектура (canvas-core без ОС/GPU — соблюдено) |
| веб-панель | Кнопка «Экспорт HTML» рядом с «Экспорт .canvas» | `crates/canvas-web/index.html`, `toolbar.rs` |
| нативный ввод | Хоткей Ctrl+Shift+E (кириллица «у») | `app/input.rs` (лестница Ctrl+Shift-хоткеев), FR-004 (оверлей клавиш — обновить список) |

## Анализ

- `crates/canvas-core/src/export_html.rs` — отсутствует (нет модуля, нет API).
- `crates/canvas-web/src/export.rs:15` — `export_active()` экспортирует только
  `.canvas` (JSON), артефакта с расчётом нет; `download_blob` (`export.rs:67`)
  хардкодит MIME `application/json`.
- `crates/canvas-web/src/toolbar.rs:38` — единственная кнопка экспорта
  `btn-export`; DOM-разметка — `crates/canvas-web/index.html:316`.
- `crates/canvas-app/src/app/input.rs:828–858` — лестница Ctrl+Shift-хоткеев
  (I — what-if, M — карта проливаний); Ctrl+Shift+E свободен.
- Данные для артефакта уже есть в ядре: `flow::FlowSolutions`
  (`outputs`/`lines`/`named`, `flow.rs:426`), what-if сравнение
  `whatif::compare_scenarios` (`whatif.rs:330`) + `scenarios_from_canvas`
  (`whatif.rs:50`); конвенция заголовка карточки — `cards.rs:213 title_for`
  (в render-крейте, для ядра — упрощённый аналог в самом модуле).
- Подпись значения на value-ребре — конвенция `stage_edge_value_text`
  (`app/stage.rs:1706`): `fromLine` → построчный выход, `fromOutput` →
  именованный, иначе узловой итог; control-ребро значения не несёт.
- Почему HTML, а не PNG: SVG-текст рендерит браузер (не нужен CPU-растер
  глифов — миникарта `minimap.rs` тому пруф, что без GPU текст не рисуется);
  what-if таблица — нативный HTML; генератор — чистая функция в canvas-core
  (TDD без GUI-стенда); будущий PNG (T4) строится растеризацией этого же
  артефакта, а не с нуля. Полное обоснование —
  `docs/dev-researches/archify-transfer-analysis.md` §3.1.

## Требуемые изменения

1. `canvas-core/src/export_html.rs` (новый): `ExportHtmlOptions { title, dark }`,
   `export_html(canvas, solutions, comparison: Option<&ScenarioComparison>, options) -> String`,
   `scenario_comparison_for_export(canvas, base) -> Option<ScenarioComparison>`
   (композиция `scenarios_from_canvas` + `active_line_exprs` + `propagate_with_lines`
   + `compare_scenarios`). Инварианты: HTML-экранирование всего пользовательского
   текста; детерминизм (тот же канвас → побитово тот же HTML, без метки времени);
   офлайн (инлайн CSS/JS, без внешних ресурсов); группы — подложками, value-рёбра —
   акцентным цветом с подписью значения, узловые итоги — футером карточки.
2. `canvas-core/src/lib.rs`: регистрация модуля.
3. `canvas-app/src/app/export_ui.rs` (новый): `App::export_html_artifact()` —
   снимок `flow_baseline` (double buffer `read_flow`), генерация, запись
   `<путь канваса>.html` через `std::fs::write`, toast результата.
4. `canvas-app/src/app.rs`: `mod export_ui;`; `app/input.rs`: Ctrl+Shift+E
   («e»/«у») → `export_html_artifact()`.
5. `canvas-web/src/export.rs`: `export_html_active()` — чтение последней
   сохранённой версии (как `export_active`), `Canvas::from_str`,
   `propagate_with_lines` (база) + `scenario_comparison_for_export` →
   `export_html`; `download_blob` получает параметр MIME.
6. `canvas-web/src/toolbar.rs` + `index.html`: кнопка `btn-export-html`
   («Экспорт HTML», title с пояснением).
7. `docs/change-requests/index-cr-fr.md`: строка FR-076.
8. `user-docs/`: раздел об HTML-экспорте (артефакт защиты, что внутри, как
   отправить стейкхолдеру).

## Точки входа

- `docs/market-researches/04-cjm-audit-plan.md` §J6 — GAP-01: пометка «закрыт частично (HTML-артефакт, FR-076)».
- `user-docs/` — инструкция пользователя (см. Требуемые изменения п. 8).
- `crates/canvas-app/src/app/input.rs` — комментарий лестницы хоткеев.
- Горячие клавиши (FR-004 оверлей `?`/F1): добавить Ctrl+Shift+E в список — отдельной правкой UI-реестра клавиш (вне MVP-коммита ядра, отмечено в Верификации).

## Проверка

- [x] `cargo test -p canvas-core`: модульные тесты `export_html` — пустой
  канвас, формула со значением, экранирование (`<script>`), подпись значения
  на value-ребре, таблица what-if с дельтами, детерминизм (два вызова
  побитово равны), группы, пресеты цвета vs `#RRGGBB`.
- [x] `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`.
- [x] `cargo check -p canvas-web --target wasm32-unknown-unknown` — ядро
  платформенно-нейтрально (сборка под wasm не ломается).
- [ ] L2 браузерный стенд (`scripts/wasm_ui_test.sh`) — в сессии недоступен
  (нет trunk/wasm-bindgen-cli/Chromium-стенда). Ручная проверка: открыть
  веб-версию → построить схему с what-if → «Экспорт HTML» → открыть
  скачанный файл офлайн (flight mode) → проверить SVG-снимок, значения,
  таблицу сравнения, переключение темы, зум.
- [ ] Натив: Ctrl+Shift+E рядом с сохранённым канвасом создаёт `.html`,
  toast сообщает путь; ручная проверка на Windows (в сессии Linux-сборка
  без GUI-рантайма).

## История изменений

- `2026-09-28` — агент: создан документ по шаблону cr-template, статус `в работе`; анализ (код-ссылки), требуемые изменения, точки входа, проверка.
- `2026-09-28` — агент: реализован модуль ядра + тесты (canvas-core), проводка натив (Ctrl+Shift+E) и веб (кнопка), индекс FR обновлён; статус `выполнено (v1)` — L2/натив-ручная проверка остаются на владельце (см. Проверка).

## Источники истины

- `docs/dev-researches/archify-transfer-analysis.md` §3 (карта T1), §3.1 (почему HTML), §3.2 (границы MVP).
- `docs/market-researches/04-cjm-audit-plan.md:308` — GAP-01.
- `crates/canvas-core/src/flow.rs:426` (FlowSolutions), `crates/canvas-core/src/whatif.rs:50/330` (сценарии/сравнение), `crates/canvas-core/src/model.rs:213` (Node), `crates/canvas-app/src/app/stage.rs:1706` (конвенция значения ребра).
- Archify: `archify/assets/template.html` (прецедент самодостаточного viewer), `viewer/export.js` (канонический экспорт без transient-состояний).
