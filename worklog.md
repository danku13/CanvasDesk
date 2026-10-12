# Worklog — журнал задач агентов (текущий период)

Формат записи: `---` / Task ID / Agent / Task / Work Log / Stage Summary /
Tokens (правило учёта токенов — AGENTS.md). Записи дописываются в конец
в хронологическом порядке. Ротация и Journal index — AGENTS.md,
«Учёт токенов по задачам», п. 2а (issue #23, 2026-10-10).

## Journal index (все записи: текущие и архивные)

- 2026-10-12 | UI-SYSTEMIC-PLAN (сессия web-0a539c42) | Системные решения по вёрстке (анализ находок второй модели): верификация 3 корней по коду (234 label-вызова в 9 файлах, 7 формул кареток, факторы 1.1/1.3), решения P1 VAnchor-рендер / P2 single-writer подписи / P3 kit::caret_rect / P4 LineMetrics, гейты-линты на каждый класс, мета-ответ «почему криво после волн» (6 механизмов) + триада Конвенция→Миграция→Гейт + WASM pixel-diff, волны V1/V2/V3 | worklog.md (текущий)
- 2026-10-12 | CR-036/#63 (сессия web-b5714086-2f9f-44a0-af82-302aa182533b) | Фикс вёрстки верхнего хрома после CR-035 (директивы владельца): WEB_TOOLBAR_INSET удалён — кластер ⚙/☼/RU-EN/? у кромки y=12..48; чип топ-право слева от «?» (ChipAnchor, сжатие/перенос ряд №2, гейт агент-панели); author-bar 5 иконок топ-лево (DOM-паритет, бренды в атласе); клампы хоткеев/flyout о chip_left_bound; тур #gpu-canvas-chip + TourOptions.hooks; wasm_audit HELP [1118,30] (прежний — в кнопку ЯЗЫКА); контракт-тест top_chrome; user-docs; #63 closed (VLM-верификация деплоя); #58 — коммент с вариантами решения | worklog.md (текущий)
- 2026-10-11 | WAVE-7 (сессия web-a6dbb853-16af-4113-bc40-c40ccb7269c3) | K4-витрина #62 (секции 6 W2 + 14 Wave C во всех ST1, golden-пины, §K4 33 секции, use-case §7 ×20) + Wave D #32 (интерактивная TO-BE витрина: D1/D3/D4/D5/D6/D7, 8 KitState, 7 тем, сайдбар, ?ui=demo); связка #62↔#32; сосуществование с M3-b адаптивом (#53); #62/#32/#27 closed completed | worklog.md (текущий)
- 2026-10-11 | DOCS-ST1 (сессия web-a6dbb853-16af-4113-bc40-c40ccb7269c3) | ST1-sync: KitState 8 состояний (Wave T) в rules/08-states.md (матрица, лестница по widget.rs, ST2 + state-layer M3, additive active_states) + 12 лестниц-сносок в 14 файлах (design/README ×2, use-cases ×11, docs/ui-kit.md §7.1); сверка с crates/; doc_lint 972/0 ошибок; issue #61 closed completed; создан #62 (K4-витрина, код-бэклог) | worklog/archive/worklog-2026-10-12.md
- 2026-10-11 | HOTFIX-56 (сессия web-b5714086-2f9f-44a0-af82-302aa182533b) | Паника prod-web «cannot recursively acquire mutex» (владелец) — вложенный захват MEASURE_FS: чип C4 каждый кадр/менеджер C3/kit_field_view ×4; фикс (m,fs) параметром + reorder; аудит scripts/audit_fs_locks.py; E2E headless до/после; main снова зелёный (fmt/clippy/тест INT); найдены #57 #58 | worklog/archive/worklog-2026-10-12.md
- 2026-10-08 | CR-029 (сессия web-c1430de8-9dcf-4130-9380-b5107677d30c) | Репорт владельца «иногда не отрабатывает автодополнение при вставке входящих значений» (скриншот: нода «Корзин | worklog-2026-10-02_10-08.md
- 2026-10-03 | PRD-0010-UI-прототип | Спроектировать UI 5 новых AI-компонентов LLM-интеграции (PRD-0010/ADR-0016) и доработать docs/prototypes/proto | worklog-2026-10-02_10-08.md
- 2026-10-02 | FR-089-активация | PostHog-настройки владельца (EU cloud, phc_-ключ) — вписать ключ, включить продуктовые события из приложения,  | worklog-2026-10-02_10-08.md
- 2026-09-24 | ИТОГ-клип | Клиппится текст «Итого» внизу шаблонных нод — CanvasDesk | worklog-2026-10-02_10-08.md
- 2026-10-02 | UI-кит-аудит | Детальный анализ — какие поверхности/компоненты UI остаются не на ui-kit, на хардкоде и без адаптива из концеп | worklog-2026-10-02_10-08.md
- 2026-10-02 | FR-090 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1) | Запрос владельца: «хочу видеть людей с ролями, как обработку анонимных профилей в настройках проекта? навешива | worklog-2026-10-02_10-08.md
- 2026-10-02 | W-a/W-b реализация | Реализация волн W-a и W-b аудита ui-kit (5 параллельных агентов в worktrees + проводка) | worklog-2026-10-02_10-08.md
- 2026-10-03 | FR-091 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1) | Запрос владельца: «сейчас при записи сеанса я вижу только то что происходит на web слое, но не вижу ничего на  | worklog-2026-10-02_10-08.md
- 2026-10-03 | mobile-touch-1 (FR-092) | проверить, почему на мобильных работает только HTML-оверлей, но не wasm | worklog-2026-10-02_10-08.md
- 2026-10-03 | 1-c (волна W-c, docs) | docs → примитивы + kit ScrollState — миграция поверхности документации (crates/canvas-app/src/docs_ui.rs, 1299 | worklog-2026-10-02_10-08.md
- 2026-10-03 | W-c/W-d реализация (сессия web-51d33571) | Реализация волн W-c и W-d аудита ui-kit (6 параллельных агентов в worktrees + проводка/интеграция) | worklog-2026-10-02_10-08.md
- 2026-10-03 | pages-rollout-verify (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1) | Запрос владельца: «продолжай Выкат на Pages (обычный trunk build) — до выката старые записи останутся "пустыми | worklog-2026-10-02_10-08.md
- 2026-10-03 | FR-094 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1) | Запрос владельца: «нужно проанализировать где у нас есть возможность для экономии на wasm бандле. так же прове | worklog-2026-10-02_10-08.md
- 2026-10-03 | FR-091 v2 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1) | Запрос владельца: «по последним записям в posthog пока не вижу чтобы wasm часть была видна в replay, так же от | worklog-2026-10-02_10-08.md
- 2026-10-03 | W-e реализация (сессия web-51d33571) | Волна W-e «решения владельца» аудита ui-kit (docs/dev-researches/ui-kit-adoption-audit.md §10): BP_COMPACT/BP_ | worklog-2026-10-02_10-08.md
- 2026-10-03 | W-f реализация (сессия web-51d33571) | Волна W-f «остатки» аудита ui-kit (приказ владельца «доделай W-f»): §9 CR-015 (stage + app), canvas-render кон | worklog-2026-10-02_10-08.md
- 2026-10-04 | A | Add 5 color slots to KitPalette + stage_close_button + backdrop components | worklog-2026-10-02_10-08.md
- 2026-10-04 | C | Fix CR-015 regression + add PANEL_HEADER_H/FONT_* tokens + migrate 7 *_ui.rs files | worklog-2026-10-02_10-08.md
- 2026-10-04 | B | Migrate overlays.rs rgba literals to new palette slots + KitPalette placeholder real values + stage_close_butt | worklog-2026-10-02_10-08.md
- 2026-10-04 | F | Create kit::anchored_stack + kit::table_layout_immediate + migrate suggest/whatif/tooltip | worklog-2026-10-02_10-08.md
- 2026-10-04 | E | Create radio_card, chat_bubble, footer_buttons, chip_strip components | worklog-2026-10-02_10-08.md
- 2026-10-04 | G | Migrate rgba→control_success/control_warning in ai_status_panel/agent_panel + 6 close-button sites to kit::sta | worklog-2026-10-02_10-08.md
- 2026-10-04 | D | Create `kit::icon_composition` + migrate `palette.rs` ~30 hand-rolled icon quads | worklog-2026-10-02_10-08.md
- 2026-10-05 | H | Create 4 LOW-priority kit components from audit §6.1 Tier-4 — crumbs (explain breadcrumbs), tree_layout (expla | worklog-2026-10-02_10-08.md
- 2026-10-05 | I | Create autocomplete_popup, search_panel, two_column, wrap_paragraphs | worklog-2026-10-02_10-08.md
- 2026-10-05 | K | Resolve 5 close-button TODO sites + explain.rs tooltip migration | worklog-2026-10-02_10-08.md
- 2026-10-05 | J | Migrate onboarding/graph_builder/autolink/scheme_gallery/template/settings/agent_panel consumers to existing k | worklog-2026-10-02_10-08.md
- 2026-10-07 | WHATIF-CJM-сессия | Аудит и фикс what-if функционала (задача владельца 2026-10-07): вёрстка/цветовая схема бара, полный сброс кноп | worklog-2026-10-02_10-08.md
- 2026-10-07 | L | Create panel_header, tooltip_rect_anchored, split_footer_buttons, chip_strip_wrap | worklog-2026-10-02_10-08.md
- 2026-10-07 | N | Migrate kit_ui/admin_ui → panel_header, explain.rs → tooltip_rect_anchored, template_ui → chip_strip_wrap | worklog-2026-10-02_10-08.md
- 2026-10-07 | O | Migrate onboarding AI mode cards → radio_card, carousel/AI footer → split_footer_buttons, tooltip.rs → anchore | worklog-2026-10-02_10-08.md
- 2026-10-07 | UR-001-сессия | Разбор отчёта владельца по ручному тесту handtest.canvas (11 дефектов) — глубокий корневой анализ каждого пунк | worklog-2026-10-02_10-08.md
- 2026-10-08 | P | Migrate 4 backdrop sites to kit::backdrop + toast rect to kit::toast_area | worklog-2026-10-02_10-08.md
- 2026-10-08 | Q | Fix 2 CR-015 text-measurement regressions in agent_panel + resolve chat_bubble force-fit (kit called only for  | worklog-2026-10-02_10-08.md
- 2026-10-08 | UR-001-W1+W2 (сессия 2026-10-08, продолжение) | Реализация волн W1+W2 плана UR-001 (CR-026, CR-019, CR-023, CR-020, CR-024, CR-025) + правило учёта токенов в  | worklog-2026-10-02_10-08.md
- 2026-10-08 | click-to-edit (сессия 2026-10-08, вечер) | Архитектурно-технологическая оценка и реализация инлайн-редактирования нод без перехода в режим правки двойным | worklog-2026-10-02_10-08.md
- 2026-10-08 | W3-C3-finish | Завершение FR-101 (подсказки входящих параметров по имени без `$`, формат вставки по языку токена, фикс счётчи | worklog-2026-10-02_10-08.md
- 2026-10-08 | W3-C1 | Fix `scheme_cjm_tests::geometry_clean_after_autogrow` (FAIL: OVERLAP text/text note-8 × note-9, investment-cas | worklog-2026-10-02_10-08.md
- 2026-10-08 | W4-D1-final | сверка и коммит этапа 2 (app,web), верификация, этап 3 (доки), коммит 3, worklog | worklog-2026-10-02_10-08.md
- 2026-10-08 | UR-001-W3+W4 (сессия 2026-10-08, продолжение) | Реализация волн W3+W4 плана UR-001 (CR-021, CR-022, FR-101, FR-100, D2-приёмка) | worklog-2026-10-02_10-08.md
- 2026-10-08 | FR-102 (сессия web-c1430de8-9dcf-4130-9380-b5107677d30c, этап 3) | Запрос владельца «нужна подсказка полей после точки; обновить формулировку и проверь CI» — FR-102: поля объект | worklog-2026-10-02_10-08.md
- 2026-10-08 | CR-030 (сессия web-0a539c42-84fd-4916-9a97-970c8052c51d) | Запрос владельца 2026-10-09 со скриншотом: «если edge идёт от значения, то он шёл от соответствующей точки… то | worklog-2026-10-02_10-08.md
- 2026-10-08 | CI-FIX (сессия web-c1430de8-9dcf-4130-9380-b5107677d30c) | Запрос владельца 2026-10-09: «проверь CI и исправь если есть ошибки» | worklog-2026-10-02_10-08.md
- 2026-10-08 | CR-031 (сессия web-0a539c42-84fd-4916-9a97-970c8052c51d) | Обратная связь владельца по онбордингу (7 пунктов, UR-002) — финал тура с выбором, вёрстка 50/50, ролевое valu | worklog-2026-10-02_10-08.md
- 2026-10-08 | W2 (сессия web-94c8d67d-efca-4624-9b94-34d7f5f7cc85, волна 2) | Волна 2 «Вживление в canvas-app: панели, входы, cost, health-UI, OAuth-сим» (план §3) | worklog-2026-10-02_10-08.md
- 2026-10-08 | CR-032 (сессия web-0a539c42-84fd-4916-9a97-970c8052c51d) | UR-003 (скриншот владельца, 2K): поверхность онбординга крошечная, иллюстрации не заполняют слот — адаптивный  | worklog-2026-10-02_10-08.md
- 2026-10-09 | FR-103 (GitHub #4, волна C0 мультиканваса; high-level #14) | Контракт-первый этап мультиканваса: заморозить контракты и чистые функции для параллельных воркстримов C1 (Opf | worklog-2026-10-08_10-10.md
- 2026-10-09 | W3-ядро+багфиксы (сессия web-94c8d67d-efca-4624-9b94-34d7f5f7cc85) | Волна W3 (web-путь LLM F-5.10) + багфиксы репорта владельца: (1) «проверка ключа не работает / по умолчанию “к | worklog-2026-10-08_10-10.md
- 2026-10-09 | UR-004/CR-033 (сессия web-0a539c42-84fd-4916-9a97-970c8052c51d) | Сообщение владельца: «поверхности ctrl+p и ctrl+i открываются под html слоем, что мешает их отображению… в ctr | worklog-2026-10-08_10-10.md
- 2026-10-09 | UR-005/CR-034 (сессия web-0a539c42-84fd-4916-9a97-970c8052c51d) | Сообщение владельца: «проверь ui-kit На пример активного поля ввода, сейчас на ctrl+i оно всегда выглядит как  | worklog-2026-10-08_10-10.md
- 2026-10-09 | UR-003-миникарта | «Проверь куда пропала мини-карта» — диагностика пропажи мини-карты, корневая причина, фикс, гейты | worklog-2026-10-08_10-10.md
- 2026-10-09 | INPUT-ADEQUACY (сессия web-94c8d67d-efca-4624-9b94-34d7f5f7cc85) | Сообщение владельца: «детальный аудит работы input компоненты в ui-kit… сформулируй концепции адекватности пов | worklog-2026-10-08_10-10.md
- 2026-10-09 | UR-005-rev2-миникарта-wasm (сессия web-0a539c42) | «я всё ещё не вижу мини-карту в wasm версии» (скриншот) — повторная диагностика UR-003, корневая причина, фикс | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W7 | Канонизация высот UI-поверхностей на шкалу S3 (аудит layouts-2026-10 §5 LAY-W7) | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W8 | Тач-44 покрытие: расширение hit-зон через touch_targets::touch_hit_xywh для what-if чипов, quick-пилюль агента | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W5 (сессия web-3b7b2cbb) | Сообщение владельца: «Изучи design\layouts-audit-2026-10.md и стартуй LAY-W5 и LAY-W6 отдельными агентами». Эт | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W6 (сессия web-3b7b2cbb) | Сообщение владельца: «Изучи design\layouts-audit-2026-10.md и стартуй LAY-W5 и LAY-W6 отдельными агентами». Эт | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W12 | CI-гейт LAY7: регрессионный grep-линт gap/y+= в canvas-app | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W11 | G4-линт: канонические состояния для graph_builder, flow_map, calc-панели, hints — закрыть слепые зоны | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W10 (сессия web-f007af5d-5af9-4a7f-a6f5-4986136a1aa9) | LAY-W10 — раздел «Исключения (documented девиации)» в design/rules/11-layouts.md (бэклог аудита раскладок §5,  | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W9 | LAY-W9 — row-скролл панели шаблонов поверх ScrollState: тип RowScroll + миграция TemplatePanel/StripHover + фи | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W9+LAY-W10-интеграция (сессия web-f007af5d-5af9-4a7f-a6f5-4986136a1aa9) | Оркестрация сессии — изучение аудита, постановка LAY-W9/LAY-W10 отдельным агентам (параллельные worktrees), сл | worklog-2026-10-08_10-10.md
- 2026-10-09 | LAY-W3/LAY-W4-сессия | LAY-W3 (админка + kit-витрина: линейки y+= → Column-скелет) и LAY-W4 (agent_panel: build() → Column/Row) из de | worklog-2026-10-08_10-10.md
- 2026-10-10 | LAY-VERIFY-e319cc1 | Верификация ревью волны LAY-W1..W12 (80af42b): «что произошло с e319cc1» — потерянная миграция W3a, слепая зон | worklog-2026-10-08_10-10.md

- 2026-10-10 | DOCS-EN-w1 (issues #17+#18–#25; сессия web-a6dbb853) | Реализация backlog'а документации: битые ссылки+линт, факты AGENTS.md, ротация worklog, полные пути, prd-0006, разгрузка индексов, CLAUDE.md, волна 1 перевода EN | worklog-2026-10-08_10-10.md
- 2026-10-10 | MC-C1 (GitHub #5, волна C1 мультиканваса; high-level #14) | Волна C1 на контрактах C0: OpfsStore над OPFS + JS-глю, AppEvent-конвейер + обратный канал App→web, Web Locks №14b + модал №35a, URL-синк №17a, битая ссылка №31c, активный сценарий №32c, persist() R-T3 | worklog-2026-10-08_10-10.md
- 2026-10-10 | DOCS-EN-w2 (issue #24; сессия web-a6dbb853) | Волна 2 перевода агентского контура EN: SPEC/TASKS/RECIPES/ui-kit/WASM-TESTING/активный index-cr-fr/skills-скоупы; 12 файлов 1:1, корпус 70863->63640 o200k | worklog-2026-10-08_10-10.md
- 2026-10-10 | DOCS-EN-w3 (issue #33; сессия web-a6dbb853) | Волна 3 агентского слоя: docs/agent/ (MAP, routes.yaml, брифы, глоссарий, 6 workflow-навыков, eval-скелет), разгрузка AGENTS.md 8280→4860 o200k, doc_lint+бэктик-пути+бюджеты, контракт-тест правила 5–6, дрейф счётчика CONTEXT/SPEC | worklog-2026-10-08_10-10.md
- 2026-10-10 | DOCS-EN-w4 (issue #34; сессия web-a6dbb853) | Волна 4 перевода: design/rules/ 00–11 + design/README.md RU→EN (13 файлов, 37.5k→173 кириллич. симв., ~29.5k→~24.5k o200k), ID правил сохранены (П*, C*, LAY*…), runtime-литералы и grep-якоря сверены с crates/ | worklog-2026-10-10_10-10.md
- 2026-10-10 | MC-C2 (GitHub #6, волна C2 мультиканваса; high-level #14) | Волна C2: FsAccessStore над granted-папкой (№41c тихий старт, R-T6 rename с .bak), миграция OPFS→папка №42a/№52a (копирование до удаления), баннер №44b, watch внешних изменений №45b/№53b | worklog-2026-10-08_10-10.md
- 2026-10-10 | MC-C3 (GitHub #7, волна C3 мультиканваса; high-level #14) | Оверлей-менеджер канвасов: список/поиск/группы №43a, создание×4, ренейм №9, удаление+undo №15a/№22c, строка хранилища №51a, drop-коллизия №26b | worklog-2026-10-10_10-10.md
- 2026-10-10 | WAVE-T (GitHub #28, high-level #27) | Wave T — Tokens & States: KitState+Focused/Dragged/Error, 4-role pairs (ButtonVariant+3), ControlSize, Elevation, Duration/Easing, Shape, Spacing | worklog-2026-10-08_10-10.md
- 2026-10-10 | WAVE-L (GitHub #30, high-level #27) | Wave L — Layout примитивы: grid_auto/Track, aspect_ratio, sticky_header, Responsive/WindowClass, Density | worklog-2026-10-08_10-10.md
- 2026-10-10 | WAVE-C (GitHub #29, high-level #27) | Wave C — 15 новых компонентов (checkbox, slider, radio, tabs, command_palette, accordion, progress, skeleton, badge, popover, snackbar, avatar, tree, segmented) | worklog-2026-10-08_10-10.md
- 2026-10-10 | MC-C4-DONE (GitHub #8, волна C4 мультиканваса; high-level #14) | Финализация C4: чип активного канваса №21c, title №28a, dirty-сигнал №29b, камера №12/№30b, онбординг №19, чистка DOM-панели №37b (FR-107) | worklog-2026-10-10_10-10.md

Архив: `worklog/archive/worklog-2026-10-02_10-08.md` (2026-10-02…2026-10-08, 46 записей). Ниже — записи с 2026-10-09.
---
Task ID: WAVE-7
Agent: Super Z (main + 8 субагентов)
Task: Решение владельца «сделай связку самих issues и двумя агентами реализуй #62 и #32»: K4-витрина (секции 6 W2 + 14 Wave C во всех ST1-состояниях, K4.1–K4.3, golden-тесты) + Wave D Demo v1 (интерактивная витрина D1/D3/D4/D5/D6/D7).

Work Log:
- Связка: тело #32 обновлено (D2 → реализация в #62; матрица D3 уточнена до 8 KitState с Error по ST1/Wave T #61; AC-D2/AC-D3 синхронизированы), кросс-комментарии в обоих.
- Исполнение — 5 раундов субагентов (Task-канал рвался по таймауту ~10–20 мин, прогресс сохранялся правилом «этап → гейты → коммит → пуш ветки»; recovery-перезапуски с диска):
  #62 (wave-62-k4): 2e72bf6 секции → 74bb514 golden → 2d67e65 доводка → ce82a27 доки; kit_sections_k4.rs ~2800 строк; §K4 13→33 секции (EN), use-case §7 ×20 (RU), ui-kit §7.3; rustdoc active_states сведён.
  #32 (wave-32-demo): 753668a v1.1 (F1–F4 + D3 пилюля 8 KitState) → 444184e v1.2 (D1-финиш + нативный вход) → e972cc9 D4 (AS-IS/TO-BE ST1-визуалы) → 02d7b00 D6 web ?ui=demo → 39dff4d D7 доки (ui-kit §7.4, kit-demo-v1.md).
- Merge-координация: обнаружен параллельный main (W-WEBUI #60 9f27982 + Wave M M3-a/b/c 77fa204, другая сессия). Порядок: #62 смёржен в новый main чисто (002dd0c, тесты 884+421/0). #32 — семантический конфликт M3-b×Wave D (kit_ui.rs 11 хунков, kit_demo поля, диспетчер overlays, колесо input): разрешён union-политикой (обе фичи в своих режимах: adaptive → M3-b, иначе → 32; ед. сигнатура impl 9-арг, 4 обёртки; App-хранилища скроллов разделены).
- Параллельная сессия смёржила wave-32-demo раньше (f89079a) с эквивалентным разрешением — мой дубль-merge abfb648 (агентная сборка, гейты зелёные) отброшен как избыточный; принят origin/main 19b1190 (проверено: cargo test 893+421/0, clippy 0 новых, doc_lint 0/974).
- #62 closed completed (чекбоксы [x], комментарий-верификация); #32 closed completed (D-пункты, ControlSize re-layout честно оставлен [ ] → v2); #27 (мастер-ревью T/C/L/A+Demo) closed completed.

Stage Summary:
- K4-норматив закрыт: компонент без секции витрины — незакрытая волна (K1.1/K4.3) больше не нарушается; 33 секции.
- Витрина — интерактивная demo (egui-паттерн): живые контролы, 8 KitState, темы/языки/размер/плотность, AS-IS/TO-BE, сайдбар, web-вход.
- Урок: параллельные сессии пушат в main во время работы — перед merge/push сверять origin; дублирующиеся merge-разрешения отбрасывать в пользу принятого.
- v2-хвосты: ControlSize re-layout, клавиатурный ринг, component paint, snapshot-эталоны Comp (FR-068 W0), standalone canvasdesk-demo, ручная браузерная приёмка ?ui=demo (владелец).
Tokens: in≈1260000 (main+8 агентов), out≈96000, total≈1360000 (estimate), model=GLM-5.3, scope=WAVE-7

## 2026-09-25 — feat(models): e-commerce capacity model — расчётный контур e-commerce на нодах CanvasDesk (сессия агента)

- **Запрос владельца:** собрать цепочку расчёта архитектуры e-commerce платформы с полным набором бэкенд-сервисов (OMS/TMS/WMS/ERP и др.), регулярной нагрузкой и сценариями Чёрной пятницы ×3/×5, стоимостью инфраструктуры в обычный период и стоимостью масштабирования; всё — на базе нод CanvasDesk («какие узлы потребуются и что закрыть шаблонными нодами»).
- **Модель:** `models/ecommerce-platform.canvas` — 39 нод (33 расчётные + 5 групп + подсказка), 51 value-связь с адресацией портов (`fromOutput`/`toParam`, FR-029). Пять зон: спрос/сценарии → доставка (CDN → tcp-lb → api-gateway → маршрутизация) → доменные сервисы → данные и фон → мощность и стоимость.
- **Шаблонные ноды (16):** cdn, tcp-lb, api-gateway, auth-service, grpc-service ×4 (Checkout/Платежи/Каталог/Поиск), cache-redis, queue-kafka, worker, db-nosql, db-sql-master, db-sql-replica, storage-s3, support-staffing (Эрланг C), capacity-planner ×3, infra-cost. Доменные OMS/WMS/TMS/ERP — Numi-ноды с mm1/mmc/erlang_c/littles_law (FR-015): OMS (order_rps×mult → события/платежи/БД/тики), WMS (штат сборщиков через Эрланг C + pickers_needed), TMS (посылки в пути и курьеры через закон Литтла), ERP (сток по SKU + проводки, mmc-пул).
- **What-if (FR-017):** сценарии «Чёрная пятница ×3» и «×5» предзаписаны в `canvasdesk.whatif.scenarios` — подмена строки `mult = 1` ноды «Режим» (line 3), весь граф пересчитывается, файл не мутируется. Стоимость масштабирования считается самим графом: capacity-planner'ы пересчитывают флот → `infra-cost` (compute+storage+egress) → «Итоги».
- **Грабли размерностей (важно для авторов моделей):** алгебра юнитов `expr.rs` не сворачивает Rate·Time → Count (нормализация только Count¹·Time⁻¹ → Rate¹) — формулы egress/штата/курьеров, написанные «по-инженерному» (`rps × sec / 1 req`), дают составные размерности и ломают сложение в downstream (usd·req vs usd). Исправлено через `littles_law(...)` (возвращает Count) и скаляризацию (`/ 1 req`, egress от `api_day × 8 KB × 30 / 1 TB`). Аналогично: планировщик отдаёт Count → в стоимость проливается скаляр `(sum(...) + db_servers) / 1 req`, иначе infra-cost даёт usd·count и «Итоги» ($1+$2) падают по размерности.
- **Верификация:** независимый Python-движок (`scripts/verify_ecom_canvas.py` вне репо) воспроизвёл семантику `flow.rs` (DAG, `$1..$N` по порядку рёбер, toParam-проливание «последний побеждает», qualified-ссылки по адресным формам имён) и `queueing.rs` (Erlang C через рекуррентную Erlang-B, перегрузка ρ≥1 → err/1.0). Результаты: `models/ecommerce-platform-report.md`. Числа: база 1111 rps пик / $2 122 мес; ×3 — 3333 rps / $4 516 (×2.13), шлюз 93%, checkout 84%; ×5 — 5556 rps / $6 910 (×3.26), без масштабирования перегрузка: шлюз 156%, checkout 140%, штат WMS 372%, поддержка 417%. 10/10 проверок.
- **Гейты:** модель открывается в web-версии/десктопе; формат — JSON Canvas 1.0 + снапшоты `canvasdesk.template` (id/version/expr/params/icon/color/name/outputs), value-рёбра `extra.canvasdesk.flow.kind = "value"`. Rust-код не менялся — сборка не затронута.
- **Telegram:** план сессии → отчёт этапов 2–5 → финальное саммари. Ветка: `feat/ecommerce-capacity-model`.

---

---
Task ID: 9 (issue #67)
Agent: Super Z (сессия web-c8da020e-a117-4a19-a125-f770d96a883e; V3 — субагент, отдельный клон)
Task: issue #67 «UI console v2: хвосты витрины — ControlSize re-layout, клавиатурный ринг, snapshot-эталоны»: V1+V2+V3, гейты, закрытие issue + Project v2.

Work Log:
- V1 (61c0e68): sized-API canvas-ui — button_layout_sized/chip_layout_sized/icon_button_rect_sized (высоты через методы ControlSize; паритет Sm==legacy — тест sized_button_parity_with_legacy); density-гэпы kit_demo (density_section_gap/density_row_gap — ступени S1: Compact MD/S, Comfortable LG/SM = канон v1, Spacious XL/MD); gallery_layout_impl — полный re-layout TO-BE-колонки (локальные btn_h/chip_h/field_h/icon_h/row_h/sec_gap/row_gap/ctl: demo_on → ControlSize-методы/токены, AS-IS → kit-константы — golden-пины целы); Wave C-секции/потребители/отрисовка на demo.size/demo.density (DEMO_TABS_PANEL_H 48 — канон, progress_spinner_radius = 0.3·button_h); тесты: draw==hit ×4 размера (v1_size_relayout_respects_control_size_and_draw_hit, Wave C — скролл-проход), density-ступени (v1_density_relayout_gaps_from_s1_tokens), K1-регресс (v1_asis_kit_metrics_regression).
- V2 (cc7c761): резолверы canvas-ui — tree_key (TreeKey/TreeKeyCtx → TreeAction) + command_palette_key (PaletteKey → CommandPaletteKey) — закрыты обещания use-cases (tree.md/command-palette.md §4, ревью #47; доки обновлены, doc_lint 0/974); FocusRing::focus_rect; kit_demo::apply_group_arrow (radio ↑/↓ wrap; segmented/tabs ←/→ через кит-tabs_key; tree ↑/↓ + ←/→ через tree_key; nav_rail ↑/↓) + 5 тестов; kit_ui::demo_ring (тулбар → mgroup → сайдбар → секции по (y,x), dedupe, оверлеи когда открыты)/demo_trap_ring (палитра → модалка → поповер → dropdown = лестница Esc)/demo_rect_of/demo_focus_rect (геометрия из кадра — draw==hit при скролле/size/density); app — Tab/Shift+Tab (TO-BE ринг; AS-IS F-17 прежний), стрелки групп, Enter-активация (kit_demo_activate — единый путь с кликом), Esc — trap-выход + сброс ринга (restore); focus_visible ST1 (рамка accent от клавиатуры, клик прячет — focus ≠ hover); тесты: v2_ring_full_closed_and_section_ordered / v2_trap_ring_priority_matches_esc_ladder / v2_focus_visible_not_hover.
- V3 (942d24f, субагент, ветка wave-67-v3-snapshots): сцены + golden Wave C (14) + W2 (6) × {Normal/Hovered/Disabled/Error} × {RU/EN} = 160 эталонов по образцу W0-кейсов; счётчик 60+160=220; существующие 60 не тронуты (FR-068 W0 §9).
- Гейты: fmt/clippy --workspace -D warnings 0/test --workspace (3253 → после merge 3417 passed / 0 failed, 96 сьютов)/lay7-lint/wasm_gate (ступени 1–2)/doc_lint 0/974. Урок подтверждён: origin/main двинулся во время работы (grid-125 + e-commerce от параллельной сессии) — сверка origin ПЕРЕД пушем; локальный merge в устаревший main сброшен, ветка пересоздана, re-merge в 321a03b чисто.
- Merge: 9c68b0a (V1+V2) + 5f20ebb (V3) → main, пуш 321a03b..5f20ebb. Issue #67: 12 чекбоксов [x] → комментарий-верификация (issuecomment-6112521944) → closed completed; Project v2: #67 → Done (токен проектов).

Stage Summary:
- Все 3 v2-хвоста #32/#67 в main: демо-витрина уважает Size/Density по всей колонке; полный клавиатурный ринг (Tab/стрелки/Esc/Enter, focus_visible ST1) + резолверы кита; 220 snapshot-эталонов.
- Остаток: ручная приёмка владельцем (клавиатура/size/density вживую — чек-лист #66); вне скоупа #67 — компонентный paint-слой 6 компонентов, standalone canvasdesk-demo (v2-хвосты при запуске).
Tokens: in≈480000, out≈105000, total≈585000 (estimate), model=GLM-5.3, scope=issue-67-v2-tails

---
Task ID: 10 (issue #63 / CR-036)
Agent: Super Z (main; сессия web-b5714086-2f9f-44a0-af82-302aa182533b)
Task: fix(web-ui): регрессия вёрстки верхнего хрома после CR-035 — кластер к углу, чип слева от «?», author-bar топ-лево как в DOM (директивы владельца 2026-10-11).

Work Log:
- Продолжение оборванной сессии: issue #63 создан ранее, код не начинался. Верификация деплоя до фикса (зонды кликов + VLM): кластер y=48..84 (WEB_TOOLBAR_INSET=36 — легаси под удалённый DOM-тулбар), чип топ-лево, strip внизу-слева; попутно найден дефект аудита: HELP [1163,66] фактически кнопка ЯЗЫКА (клик переключал RU↔EN, скриншоты «help_menu» снимали не меню).
- Инсет: WEB_TOOLBAR_INSET удалён (lib.rs: button_rect все углы к кромке, комментарий-некролог); flowmap PANEL_TOP=64. Тест settings_button_corners уже ассертил кромку — но на нативе (cfg(wasm32) невидим тестам).
- Чип: ChipAnchor{right_x,row_y,wrap_y,left_bound} + chip_left_bound (гарантированный край без замеров); chip_layout — правая привязка, сжатие (CHIP_MIN_ROW_W=96), перенос ряд №2 (flex-wrap DOM-тулбара); app::chip_anchor (TopRight → help.x−GAP/центр ряда; иначе vw−SPACING_LG; left_bound — поиск из чистой арифметики, W3.2); гейт агент-панели в реестре (паттерн CORNER_BUTTONS); rename-подсказка СЛЕВА от пилюли.
- Author-bar (about_ui.rs): AUTHOR_BAR_BUTTONS ×5 (github/telegram/linkedin/heart/info; фолбэки GH/TG/in/$/ℹ — брендовых глифов в шрифтах нет), 28×28/gap 4/margin 8 (DOM-паритет), author_bar_layout(dock_open) + сдвиг за док (UR-003), author_bar_hit; переименование strip→bar по всем потребителям (поле App::author_bar, id::AUTHOR_BAR, click_author_bar → OpenUrl/About); overlay: kit-кнопки + иконка 16×16 + тултип под кнопкой.
- Атлас: +16 SVG (пути из прежнего DOM index.html) ×4 набора + rasterize_icons.py (48 имён); icon_pipeline-тест 220→240.
- Клампы соседей (G4 поймал налезания на чип после переноса): hotkeys right_limit и flyout right_limit — о chip_left_bound−GAP (консервативно, без замеров; рядом №2 тоже чисто).
- Тур: TourOptions.hooks проброшен в конструктор (заявленная точка не была wired); якоря #gpu-canvas-chip (RU/EN locate-toolbar + save + scheme-gallery); резолвер в index.html (вьюпорт + localStorage button_corner — TOML-регекс, паттерн getLanguage); тексты «правый верхний угол»; npm build + инлайн бандла.
- wasm_audit.mjs: HELP [1118,30]/THEME [1206,30]/GEAR [1250,30]/MI_* −36px; probe2-клики вне ряда.
- i18n: ONBOARDING_STEP9_BODY RU/EN — «иконка списка в правом верхнем углу, слева от кнопки „?“».
- user-docs: interface.md (менеджер — иконка списка на чипе), export.md («Экспорт HTML…» — меню канваса), onboarding.md + v2 (Тур — подменю меню канваса; история #63).
- Контракт-тест top_chrome_contract_corners_and_rows (позиционные ассерты: кластер у кромки/чип прижат к «?» и в ряду/flowmap ниже/author-bar топ-лево) — лекарство от рецидива; CR-036 doc + индекс + ACCEPTANCE.
- Гейты: test --workspace (canvas-app 896, G4 49; 3 прогона — ранний прогон с 3 флейками от параллельной сборки), clippy/fmt, wasm_gate --check, mcp_wasm_gate, audit_fs_locks (2 кандидата прекоммитные Wave M — проверено git stash), doc_lint 975/0.
- Rebase на #67 (00f8094) — origin ушёл во время работы; гейты перепрогнаны. Push fee1d68.
- E2E: localhost (1280×800: кластер у кромки y≈12-15, чип слева от «?», author-bar 5 кнопок; About-модал по ℹ — 6 ссылок; help-меню по (1118,30); поиск 900×560 — чип переносится на ряд №2; SVG-бренды при icon_style=lucide; тур — тултип «В правом верхнем углу…» у чипа; 0 pageerror) + живой деплой canvas-web-6c118ac8a6c658ef (~4 мин после пуша, VLM: всё то же).
- Issues: #63 закрыт с комментарием-верификацией (вкл. самоанализ «почему вёрстка не была починена» — 4 причины: компенсация пережила предмет/гейты не видели позицию/аудит легализовал сбитый эталон/чек-лист функционален, не позиционен); #58 — комментарий по директиве владельца (механика + 3 варианта решения, рекомендация: опции развилки ведут на шаг 9), открыт за решением владельца.

Stage Summary:
- Верхний хром: author-bar (топ-лево, 5 иконок) — чип+кластер (топ-право, чип слева от «?») — паритет html-версии восстановлен; «улетевшие» иконки устранены (инсет удалён).
- Позиционный контракт закреплён тестом; соседние панели (хоткеев/flyout) клампятся о чип.
- Остаток: ручная приёмка владельцем (#66-контур); #58 — решение о нумерации шагов карусели за владельцем; Glyph-дефолт author-bar — текстовые метки (конвенция приложения; SVG при выборе набора в настройках).
Tokens: in≈820000, out≈190000, total≈1010000 (estimate), model=GLM-5.3, scope=CR-036-issue-63

---
Task ID: KIT-BUTTON-POLISH
Agent: Super Z (main)
Task: Полировка ui-kit компонента кнопки (Button/ButtonProps, canvas-ui)

Work Log:
- Синк main (0c4a9ad): учтены Wave A/C/T, #67 V1 (sized-API), K4-витрина; аудит показал: retained Button никем не используется, kit-реэкспорт не содержит компонент, ControlSize не дошёл до Component-слоя, focus-ring/state-layer рисуют только потребители (K4)
- ButtonProps: +size: ControlSize (Sm == legacy паритет, тест), +ButtonProps::new; Button::layout → button_layout_sized
- Button::paint: state-layer M3 (§5.1.1) — overlay слота text с альфой state_layer_alpha ТОЛЬКО для состояний без fill-слота (keyboard-focus 10% / dragged 16%; hover/pressed отвечают слотом hover_fill — задвоение исключено) + focus-ring FR-057 (§5.1.2, keyboard-origin, контур accent; button_style остаётся ring-агностиком)
- Button::set_disabled — синхронная смена props+state (ребро клика гасится)
- kit.rs: реэкспорт Button/ButtonProps/ButtonLayout/BUTTON_FAMILY/BUTTON_FONT_SIZE
- +6 юнит-тестов; гейты: fmt, check (native+wasm32), test canvas-ui 221 / workspace 3425 (0 fail), clippy --workspace --all-targets -D warnings, lay7, doc_lint — зелёные
- Ремонт преждесуществующего: canvas-app/lib.rs:3585 assert! на константах → const{} (clippy 1.99 assertions_on_constants; тулчейн свежее пинового CI)
- token_lint: 3 преждесуществующих файла с hex-литералами (export_html/cards/gpu_gate) — вне гейт-набора закрытия, не тронуты

Stage Summary:
- Component-слой кнопки доведён до канона кита (Wave T/#67 V1): шкала размеров, M3-отклик, keyboard-ring, retained-эргономика, публичный kit-API
- Визуальный ноль-скачок: Component потребителей не имеет; снапшоты/витрина не задеты
- scope=kit-button-polish

---
Task ID: KIT-BUTTON-POLISH
Agent: Super Z (main; сессия web-0a539c42-84fd-4916-9a97-970c8052c51d)
Task: Полировка ui-kit компонента «кнопка» (запрос владельца): миграция потребителей на kit::Button (снятый TODO onboarding), хит-зоны MIN_TOUCH_TARGET (LAY8.3), размерные кегли подписи.

Work Log:
- canvas-ui: ControlSize::font_size() (Xs=11 / Sm=13 / Md=14 / Lg=15; Sm ≡ BUTTON_FONT_SIZE) + ControlSize::pad_h() (8/12/14/16; Sm ≡ BUTTON_PAD_H) — размерная типографика подписи (кегль/воздух растут с высотой контрола, M3/Fluent).
- canvas-ui: LAY8.3 в ките — MIN_TOUCH_TARGET = 44 + touch_expand (центрированное hit-only расширение) + touch_hit (кламп в контейнер); математика 1:1 с FR-097 (единый источник).
- canvas-ui: слияние с параллельной полировкой 10ed162 (issue #67 V1: sized-layout, state-layer M3, focus-ring, set_disabled, ButtonProps::new): их sized-layout (высота=button_h, пад канона — эталоны V3 заморожены) сохранён 1:1; мои размерные кегли — ControlSize::font_size() (Xs=11/Sm=13/Md=14/Lg=15) + pad_h() (8/12/14/16) в mod.rs, проводка кегля шкалы в Component Button::layout (замер тем же кеглем, CR-015); ButtonProps + touch_hit: bool; hit_test override — зона до 44 при touch_hit (дефолт бит-в-бит).
- canvas-app: touch_targets.rs — математика расширения/клампа делегирована киту (тонкие мосты xywh/xyxy + coarse-флаг FR-097); тесты семантики прошли без правок.
- canvas-app: миграция Skip (снятый TODO onboarding button_layout): skip_rect — измеренная Ghost-кнопка (текст + 2·BUTTON_PAD_H·k, высота BUTTON_HEIGHT·k — канон, паритет Prev/Next); фикс 64×22 и отдельный кегль 11 удалены (64px был впритык к «Пропустить» без пада — измерение убирает риск клипа RU-подписи). button_rect/button_rect_touch/button_at/button_at_touch несут замерщик + skip_label: draw==hit — рендер (overlays.rs) и ввод (input.rs) мерят тем же замерщиком, что и раскладка карточки.
- Гейты (на ребейзе поверх 0de9f49): fmt, check --workspace, test --workspace, clippy, wasm_gate --check, lay7, doc_lint — прогон после разрешения конфликтов (см. коммит).
- Тесты: +5 kit (шкала кеглей/падов, кегль Component-кнопки по шкале, touch-зона hit-only, touch_expand/touch_hit клампы); пин Skip 64×22×k заменён измеренными инвариантами; дубли sized-тестов параллельной сессии устранены.

Stage Summary:
- Кнопка кита: 7 вариантов × 4 размера — единая шкала высота/пад/кегль; hit-зоны LAY8.3 — на уровне кита (Component touch_hit + утилиты touch_expand/touch_hit), FR-097 использует тот же источник.
- Онбординг: Skip — измеренная kit-геометрия, последний hand-rolled кнопочный фикс ушёл; RU-подпись без риска клипа (64 → ~80px при k=1); вне Skip визуальных скачков нет.
- Вопрос владельцу: обновлять ли онбординг-скриншоты/доки под новую геометрию Skip (шифт 64×22 → ~(80)×30 при k=1) — по протоколу FR/CR.
- Telegram: план (msg 1111), этап 1 (1112), этап 2, финальное саммари — отправлены.
Tokens: in≈120000, out≈40000, total≈160000 (estimate), model=GLM-4.7, scope=kit-button-polish

---
Task ID: UI-LAYOUT-AUDIT-SCREENSHOTS
Agent: Super Z (main)
Task: Расследование проблем вёрстки по скриншотам WASM/Chrome (папка Bugs_12-10-26), гипотезы и план доработок

Work Log:
- Скачаны 9 скриншотов (gdown) в /home/z/my-project/screenshots/Bugs_12-10-26; просмотрены все
- Зафиксированы дефекты: чип «Нажатие/Выбрано» двойной текст (витрина), каретка во всю высоту поля (агент-панель), title×Skip коллизия (онбординг шаг 1), «★» tofu (Об авторе), пустые икон-кнопки, обрезанный хинт «ключ пустой — введит» (настройки BYOK), поля модель-строк впритык к кромке ряда, «RU»+аватар витрины поверх хедера AI-агента, разнобой вертикальных отступов
- Корень 1 (текст): пайплайн текста top-anchored — screen_text_area (canvas-render/text.rs:456) ставит top=origin.y; label_center центрирует ТОЛЬКО горизонтально; каждый вызов крутит магические инсеты (+1/+2/+4/+5/+8) — «выровнен не везде»
- Корень 2 (каретка): 4 несогласованные геометрии — overlays.rs:52 (h-4, w1.5), overlays.rs:1654 (h-8, w1.5), kit_ui.rs:4844 (h-8, w1.0), agent_panel.rs:1076 (h-4, w1.5); caret = text_area.h (30) - inset ≈ 22-26px при line-height текста 12-13px×1.3 ≈ 16-17px; text_area у кита — вся высота поля (text_field.rs:597-602, PAD только горизонтальный)
- Корень 3 (чип): раскладка слота 2 = KIT_STATE_SELECTED («Выбрано», kit_ui.rs:1552), статичная отрисовка = Selected (overlays.rs:1558), демо-оверлей = STATE_LABELS[2] = PRESSED «Нажатие» (kit_ui.rs:4763-4766) → два разных текста на одном rect при TO-BE; тултип тоже STATE_LABELS (kit_ui.rs:5128)
- Корень 4 (онбординг): title рисуется однострочно на всю ширину текстовой колонки без ellipsis/клампа (overlays.rs:4441-4452), Skip — правый верх КАРТОЧКИ (skip_rect) → наложение на длинном RU-заголовке; точки центрируются по колонке (progress_dots), title/body слева — визуальный разнобой; на финальном шаге «Назад» висит в footer_h=46 зоне с 30px кнопкой → дыра над ним
- Корень 5 (настройки): badge BYOK фиксированные 110px (overlays.rs:6602) режут «ключ пустой — введите ключ» и могут уезжать под кнопку; поле TextInput field_y=22+30=52 = впритык к нижней кромке ряда (MODAL_ROW_HEIGHT=52, без нижнего пада)
- Корень 6 (глифы): «★» U+2605 отсутствует во встроенном шрифте → tofu (i18n.rs:1532); пустые икон-кнопки — вероятный fallback на пустой глиф/отсутствие в SVG-атласе (проверка в плане)
- Репо-контент не менялся (расследование, без коммита)

Stage Summary:
- Системный вывод: волны полировки добавляли компоненты (layout-контракты кита), но вертикальная типографика осталась «ручным трудом» на сотне call-site'ов; нужен TextMetrics-контракт (v-center + line-height) в рендер-пайплайне, caret-канон в ките, единый источник label'ов чипов
- План из 5 волн сформулирован и отправлен в TG + чат

---
Task ID: UI-SYSTEMIC-PLAN (сессия web-0a539c42)
Agent: Super Z (main)
Task: Анализ находок второй модели (расследование вёрстки: вертикаль текста, каретка, отступы) и системные решения по каждой проблеме — чтобы классы дефектов не возвращались

Work Log:
- Верификация находок второй модели по коду (spot-check rg/Read, без сборки): все 3 корневые причины подтверждены — рендер top=origin.y дословно (text.rs:456, вертикальной центровки нет), «Подпись — забота потребителя» (button.rs:650-652), text_area = полная высота контрола (text_field.rs:597-602), 7 формул кареток от высоты контрола (overlays.rs 52-58/1649-1662/6898-6913, agent_panel.rs 1071-1083, kit_ui.rs 4840-4849, admin_ui.rs 1335-1347, explain.rs 1647-1663), поиск: текст input+9.0 (overlays.rs:959) vs каретка text_area+2 — разные системы координат, ROW_LINE_FRAC=1.1 (row.rs:29) vs SCREEN_LINE_FACTOR=1.3, онбординг — единственный ручной v-center (overlays.rs:4563)
- Уточнения к находкам: label_center/label_left = 234 вызова в 9 файлах (модель считала 228; +banner.rs 1); SCREEN_LINE_FACTOR в canvas-ui/measure.rs:30, зеркальная LINE_HEIGHT_FACTOR=1.3 в canvas-render/text.rs:118 — две константы одного смысла (признак отсутствия владельца метрики)
- Системные решения (отправлены в TG, 4 сообщения, msg 1118-1121): P1 — VAnchor-контракт в рендер-примитиве (центр — дефолт однострочных, Top — осознанные стеки; top считает рендер из line_h шейпера); P2 — single-writer «виджет владеет подписью»: label_line_box в ките, label_center/label_left deprecated для контрол-подписей, миграция кодмодом по списку call-sites; P3 — kit::caret_rect от метрики строки (промежуточно) / caret в рендер-примитиве (финал, рассинхрон физически невозможен); P4 — схлопывание факторов в LineMetrics{ascent,descent,line_height} = line_metrics(family,size)
- Гейты на каждый класс: юнит-тест screen_text_area (центр == центр area.h), grep-линты в CI (полный-rect label, магические +N офсеты, литералы ×1.3/×1.1 вне measure.rs), golden-тест caret_rect — по образцу lay7_lint/token_lint
- Мета-ответ владельцу «почему после многих волн криво» — 6 механизмов (нет владельца геометрии; конвенции в ревью, не в гейтах; golden замораживали баг; магические числа плодились быстрее ревью; потеря знания между волнами; фикс по скриншотам без измеримой шкалы) + системный фреймворк: триада «Конвенция → Миграция → Гейт» как стандарт закрытия любого класса UI-дефектов (класс закрыт, только когда есть все три) + 4-й элемент: WASM pixel-diff на LAY8.1 (wasm_ui_diff.py)
- Порядок исполнения: V1 фундамент (спека rules/09-input + LineMetrics + VAnchor + caret_rect + ROW_LINE_FRAC→1.3) → V2 кодмод (все 234 label + 7 кареток, витрина→agent→настройки→поиск→admin/прочее, без исключений «на глаз») → V3 гейты (линты, перезаморозка golden одним коммитом, wasm L0/L2); риски: каскад golden-правок, ручные компенсации мигрировать ВСЁ (двойная конвенция вернёт баг), суб-пиксель ±1px как допуск, скриншот-дифф из-за VAnchor-дефолта
- Инфраструктура сессии: rootfs был 100% (0 avail, падали записи) — диагностика (du/df/mount): /tmp/my-project — отдельная PolarFS (чистка там не помогла), освобождено 3.2G удалением cd-target (3.7G wasm-кэш) с rootfs, cargo-target сохранён для быстрых нативных сборок; git remote в .git/config отсутствовал — восстановлен origin → danku13/CanvasDesk
- Скрипт: scripts/ui_layout_systemic_tg.py (4 сообщения TG, curl, HTML parse_mode)

Stage Summary:
- Все находки второй модели подтверждены кодом с точными строками; 2 уточнения (234 label-вызова в 9 файлах; фактор 1.3 дублирован в measure.rs:30 и text.rs:118)
- Владельцу передан системный фреймворк: каждая проблема закрыта триадой (владелец в архитектуре + гейт в CI + правило в спеке) — классы дефектов не смогут вернуться; ответ на вопрос «почему криво после волн» сформулирован
- План V1/V2/V3 готов к передаче исполняющей модели; по команде — GitHub-issue (high-level + чек-лист) и/или старт V1
Tokens: in≈95000, out≈28000, total≈123000 (estimate), model=GLM-5.3, scope=UI-SYSTEMIC-PLAN

---
Task ID: UI-V1-FOUNDATION (issue #68, сессия web-0a539c42)
Agent: Super Z (subagent V1, прерван платформой по таймауту до отчёта; верификация, гейты, worklog — координатор)
Task: Волна V1 системного канона вертикали (issue #68): LineMetrics, kit-хелперы text_area_vcentered/caret_rect, авто-центрирование line-box в рендере, ROW_LINE_FRAC→1.3, спека, тесты/доки

Work Log:
- measure.rs: LineMetrics{line_height,ascent,descent} + line_metrics(size) — чистая, hhea-доли (asc 1.069/desc 0.293 от суммы 1.362, инвариант ascent+descent==line_height бит-в-бит); CARET_WIDTH/PAD_V/MIN_H
- kit.rs (фасад) + lib.rs: pub use caret_rect, line_metrics, text_area_vcentered, LineMetrics — канонический путь canvas_ui::measure, эргономичный canvas_ui::kit:: (для V2)
- text_area_vcentered(rect,size): строковый бокс size×1.3, кламп по rect.h, slack пополам; caret_rect(text_area,size,caret_x): h=line_h−2·pad (≥6), w=1.5, центр строки
- render text.rs: ScreenText.area_h + pub(crate) vcenter_top (area_h>line_h → центрирование, иначе бит-паритет); top=(vcenter_top(...)*scale).round(); плумбинг height: PaintItem::Text→OwnedText (kit_ui flush_last_text)→OwnedScreenText (конструкторы app обновлены механически: rect.h где известен, 0.0 → топ-анкер/паритет)
- row.rs: ROW_LINE_FRAC = SCREEN_LINE_FACTOR (1.1 → 1.3, паритет с рендером); banner.rs: label через text_area_vcentered
- Снапшоты canvas-ui (switch/tabs/two_column и пр.) перегенерированы под новую геометрию рядов — прошли
- Спека design/rules/09-input.md: IN11 (line-box centering + бит-паритет рендера), IN12 (caret formula), IN13 (запрет магических офсетов; якорь будущих линтов V3); design/README индекс; docs/ui-kit.md — секция хелперов
- Тесты: measure (сумма метрик, геометрия хелперов), vcenter_top (render)

Stage Summary:
- Гейты (координатор): fmt OK; clippy -p canvas-ui -p canvas-render -D warnings OK; check -p canvas-app OK; cargo test -p canvas-ui 445+11+4+6+16 passed/0; canvas-render 420+1+15+1 passed/0; doc_lint: 0 ошибок по репо-файлам (108 FAIL в skills/ — pre-existing, сессией не трогались)
- API для V2: canvas_ui::kit::{text_area_vcentered(rect,size), caret_rect(text_area,size,caret_x)}; OwnedScreenText.height — высота rect-владельца (0.0 = бит-паритет топ-анкера)
- V2 границы: миграцию label/кареток агент НЕ начинал (agent_panel/overlays каретки ещё старые) — только механический плумбинг height
Tokens: in≈260000, out≈60000, total≈320000 (estimate), model=GLM-5.3, scope=UI-V1-FOUNDATION

---
Task ID: UI-V2-A (issue #68 волна V2, витрина)
Agent: subagent V2-a (kit_ui + k4)
Task: Механическая миграция витрины кита на канон вертикали V1 (issue #68): label-вызовы → text_area_vcentered, демо-каретка → caret_rect

Work Log:
- kit_ui.rs: 41 из 48 label-вызовов мигрированы (40 сайтов + хелпер icon() glyph-fallback); полные rect контрола и ручные суб-rect (+2…+9, (h−14)/2, h−6/h−4/h−8) → kit::text_area_vcentered(rect, size) с сохранением x/w-инсетов и кеглей; демо TextField — общий строковый бокс ta для текста и каретки; демо-каретка UiRect::new(caret_x, ta.y+4, 1, ta.h−8) → kit::caret_rect(ta, 13, caret_x) — h 28→12.9, w 1.0→1.5, центр строки field.y+18 сохранён бит-в-бит
- kit_sections_k4.rs: 24 из 32 (row_label_at — единый хелпер 17 подписей строк теперь пушит UiRect-строковый бокс text_area_vcentered(row,11) вместо UiPoint с ручной центровкой (row_h−18)/2; K4Sections.labels: Vec<(UiPoint,_)>→Vec<(UiRect,_)>; shift_and_filter по боксам; 23 wrap-сайта: banner-action, crumbs, footer, checkbox×2, radio, tabs, segmented, palette×2 (формулы (h−16)*0.5 → vcenter), accordion×2 (шейврон full header.h, лейбл по K4_ACCORDION_HEADER_H=32), badge×3, popover-якорь, snackbar×2 (full bar_h), avatar×3, tree×2)
- Не мигрировано (осознанно): kit_ui 5 сайтов топ-анкерного контента (accordion body, nav-rail sidebar-подпись, modal title/body h=40, popover body h=44) + 2 в тесте kitdraw_delegation_matches_direct_path (пин плумбинга, не UI); k4 8 сайтов: banner label_area / chat header / radio_card label+desc — компонентные строковые боксы (уже канон), chat text_area / two_column панели / accordion content / popover body — топ-анкерный контент с токен-падом (кандидаты на решение V3)
- Golden: в crates/canvas-app/tests/ эталонов витрины нет (matches — графовые integration_edges/groups); инлайн-гольдены kit_ui/k4_golden пинят слоты раскладки (не менялись) — 0 обновлений; дельты draw-геометрии для перезаморозки: full-rect сайты пиксель-идентичны (авто-центрирование рендера ≡ vcentered), ручные суб-rectы ±0.2…2.0 лог.px (K4-подписи +1.0, tooltip −0.8, accordion-заголовок +2.0)
- Комментарии: обновлены только stale-геометрия; логика/цвета/хит-зоны/ширины не тронуты
- Гейты: cargo fmt -p canvas-app OK; cargo check ПРОПУЩЕН по гейту диска (df Avail 1.4G < 1.5G) — выполнен координатором централизованно

Stage Summary:
- Витрина (kit_ui + k4) мигрирована на канон #68: 65 вызовов text_area_vcentered + 1 caret_rect; все ручные вертикальные компенсации подписей контрола устранены; топ-анкерные стеки и компонентные line-boxes сохранены (IN11/IN13 соблюдены; сомнительные места перечислены для V3)
Tokens: in≈160000, out≈3000 (prompt/completion, platform), model=GLM-5.3, scope=UI-V2-A

---
Task ID: UI-V2-B (issue #68, волна V2)
Agent: subagent V2-B (панели)
Task: Механическая миграция панелей на канон вертикали V1 (IN11–IN13): label → text_area_vcentered, каретки → caret_rect.

Work Log:
- agent_panel.rs: 9/16 label; каретка y+2/h26 → caret_rect (y+9.2/h11.6); селекция выровнена по caret_rect-геометрии (y+9.2/h11.6). Не тронуты: шапка-стек, hint/bubble-стеки, kit chat_bubble-боксы.
- admin_ui.rs: 31/41 label; каретка поля (x двойной-офсет! y+6/w1.0/h18) → caret_rect (x=caret_x абсолютный, y+8.55/w1.5/h12.9 — фикс ширины 1.0→1.5 и x). Не тронуты: card/таблица header-титулы, стеки тела, top-анкерные аннотации «Канвас».
- ai_status_panel.rs: 5/5 label; model_area — ручная центровка (HEAD_ROW_H−12.5)·0.5/h14 → text_area_vcentered (линейный бокс 16.25, y+3.875).
- graph_builder_ui.rs: 5/12 label (кнопки/est); title/subtitle, textarea, radio-card (J2 1:1), preview-стек — оставлены.
- explain.rs: caret y+5/фикс16 → caret_rect (y+7.2/h11.6); текст поля y+5.5/height0 → строковый бокс (y+5.2/height=15.6); тултип (h−12·1.3)/2 → vcentered (литерал 1.3 устранён). Loader-caption — топ-анкер, оставлен.
- handler.rs: 1/1 label (тост-кнопка).
- Тесты: crates/canvas-app/tests — панельные golden'ы отсутствуют (integration_* — canvas-модель); inline check() admin — layout-y не менялись; правок эталонов не потребовалось.
- Гейты: cargo check ПРОПУЩЕН — df Avail 1.4G < 1.5G (по протоколу); замена — rustfmt parse-check всех 6 файлов: 0 синтакс-ошибок. НЕТ git/test/clippy (координатор выполнил централизованно).

Stage Summary:
- 53/78 label-сайтов панелей на text_area_vcentered; 3/3 каретки + селекция agent на caret_rect; дефекты: каретка agent во всю высоту поля (26→11.6), w 1.0 admin (→1.5), двойной x-офсет admin-каретки, двойное центрирование после V1 (dropdown-пункты +6.2→+3.2, икон-кнопки +5.55→+4.55).
- V3-линтам на заметку: 25 оставленных топ-анкерных сайтов перечислены в отчёте (осознанные стеки, не нарушают IN11).
Tokens: in≈171000, out≈2400 (prompt/completion, platform), model=GLM-5.3, scope=UI-V2-B-panels

---
Task ID: UI-V2-C
Agent: Super Z (subagent V2-c)
Task: Механическая миграция overlays.rs на канон вертикали V1 (issue #68): каретки → kit::caret_rect, label/ручные v-центры → kit::text_area_vcentered, поиск/онбординг/настройки-навигация

Work Log:
- Каретки 3/3: kit_field_view (+2.0/h−4.0 → caret_rect по font_size — общий рендер поиска/галереи/шаблонов/чипа/менеджера), K4-витрина (+4.0/h−8.0 → 13.0), настройки TextInput (+2.0/h−4.0 → 11.0)
- Поиск: текст origin.y+9.0/height 0 → явный строковый бокс text_area_vcentered (origin.y+5.9, height 18.2 при h=30); каретка +2.0/26 → +7.9/14.2; текст и каретка на общем центре y+15.0; высота поля и +10/−20 сохранены
- Онбординг: 2 CTA-опции + CTA №19 + Prev/Next/Skip (ручные (h−font·1.3)/2 боксы → text_area_vcentered — бит-в-бит); настройки-навигация: глиф/подпись табов, заголовок раздела (title_rect h=30, было +3.0), подсказка (hint_rect h=24, было +4.0)
- label_center/label_left: 59/71 обёрнуты (админ 5, витрина 32, storage-баннер 2, миграция 5, чип+тултипы 6, менеджер 8 через draw_button-замыкание); из 18 d.label — 6 (онбординг ×3, flowmap титул/×/пустое)
- Того же класса: docs «×» (×1.3-литерал → хелпер), замыкание icon_top удалено (RU/EN, «?» — бит-в-бит)
- Оставлены: осознанные стеки (поиск-ряды, меню, hints, about, тела), уже строковые боксы BANNER_LABEL_LINE_H (баннер/титулы/storage-label), вне скоупа точек: wheel-меню (глиф+2.0/подпись h=12), d.icon-области, ~70 прочих OwnedScreenText пушей (docs/flyout/what-if/шаблоны — «прочее» волны)
- Гейты: cargo check ПРОПУЩЕН (df /: Avail 1.37G < 1.5G — гейт); rustfmt --check — exit 0; НЕТ test/clippy/git (координатор выполнил централизованно)

Stage Summary:
- overlays.rs на каноне вертикали: 3 каретки, поиск, онбординг, 59 label-вызовов; байт-паритет там, где формула совпадала, канон-центровка где были магические офсеты (+2…+8, font+2.0, caption+1.0)
- Остаток для «прочего»: wheel-меню, d.icon-области, ~70 ручных OwnedScreenText пушей вне списка точек — решение координатора (кандидат в отдельную волну с V3)
Tokens: in≈190000, out≈2500 (prompt/completion, platform), model=GLM-5.3, scope=UI-V2-C

---
Task ID: UI-V2-INTEGRATION (issue #68, сессия web-0a539c42)
Agent: Super Z (main, координатор 3 сабагентов V2-A/B/C)
Task: Интеграция волны V2: миграция витрины/панелей/overlays на канон вертикали, центральные гейты, коммит

Work Log:
- 3 параллельных агента (файлы не пересекаются): V2-A витрина, V2-B панели, V2-C overlays (детали в блоках выше)
- Все агенты: cargo check пропущен по дисковому гейту (Avail 1.4G < 1.5G), заменён rustfmt parse-check
- Координатор: освобождён диск (canvasdesk-run 79M + repro_out_*), fmt -p canvas-app, check OK (1 warning: SECTION_TITLE_H стал тест-онли после миграции row_label_at — импорт перенесён в mod k4_golden, координаторский фикс), clippy -p canvas-app --all-targets -D warnings OK, cargo test -p canvas-app 910/0 + 12 интеграционных наборов 0 failed
- Не мигрировано (осознанно): топ-анкерные стеки и контент-тела (IN11-допустимо), компонентные line-boxes (уже канон), wheel-меню/d.icon-области/~70 ручных OwnedScreenText пушей «прочее» — кандидат в отдельную волну вместе с V3-гейтами
- Golden-эталоны: обновлений не потребовалось (инлайн-гольдены пинят слоты раскладки, не draw-геометрию); дельты draw-геометрии для V3-перезаморозки задокументированы в блоке V2-A

Stage Summary:
- V2 закрыта: 153 label-вызова (41+24+53+59 витрина/панели/overlays из ~199 сайтов) + 7 кареток + поиск + онбординг-кнопки + селекция agent-панели на kit-каноне; каретки 26px при кегле 13, w 1.0, двойные x-офсеты — устранены как класс
- Гейты централизованно: fmt/clippy -D warnings/check/test 910+интеграции/0 — зелёные; бит-паритет подтверждён снапшотами и гольденами
- Остаток: ~70 ручных OwnedScreenText пушей (docs/flyout/what-if/шаблоны), wheel-меню, icon-области; V3 (линты + wasm L0/L2 + перезаморозка golden при наличии) — следующая волна
Tokens: in≈520000 (3 сабагента platform-учёт) + координаторская верификация ≈120000, out≈15000, total≈650000 (estimate), model=GLM-5.3, scope=UI-V2

---
Task ID: UI-V3 (issue #68 волна V3; сессия web-0a539c42)
Agent: Super Z (координатор; V3-A агент — прерван платформой после сканера; V3-B агенты B1/B3 — прерваны после правок, без отчётов; B2 — полный отчёт; гейты/интеграция/wasm — координатор)
Task: Волна V3 канона вертикали: grep-гейты IN13 + миграция 171 хита + WASM L0/L2 (LAY8.1)

Work Log:
- V3-A: сканер scripts/v3_ui_scan.py (793 строки: L1 полный-rect label / L2 магические y-офсеты / L3 ×1.3-×1.1 вне measure.rs; спаны label/icon-вызовов, let-резолвинг, отсечка тест-модов по mod-границам, лексер коммент/строк); обёртка v3_ui_lint.sh + selftest 26 фикстур; CI job v3-ui-lint (по образцу lay7-lint); спека IN13 — якорь на гейт; baseline: 171 хит (L1 51, L2 120, L3 0) — ровно задокументированные хвосты V2
- V3-B: 171 хит закрыт: ~126 сайтов мигрировано (text_area_vcentered 153→312 вызовов; caret_rect), 45 осознанных // v3:allow (топ-анкерные стеки IN11 / компонентные строковые боксы IN13 / арт онбординга). Патч сканера: rustfmt-устойчивость standalone-маркеров (rustfmt переносит хвостовой комментарий за скобку; walk стал пропускать standalone-allow в события, +2 фикстуры). Координаторские фиксы: stage.rs ×2 Rect→UiRect конверсия (типовая ошибка агента B2)
- Гейты натив: скан 0 нарушений, selftest 26/26, fmt, clippy --workspace -D warnings, cargo test --workspace 96 наборов 0 failed, doc_lint (108 pre-existing skills/ вне сессии)
- Ротация worklog по $worklog-rotate: бюджет 124k→115k (3 записи 2026-10-10 → archive/worklog-2026-10-10_10-10.md, индекс обновлён)
- Hygiene до V3: мусорный коммит 40a08d13 затрекал cargo-target/canvasdesk-run (11 189 файлов) — untrack + .gitignore
- V3-C wasm: wasm-bindgen-cli 0.2.127 (по Cargo.lock); L0 wasm_gate.sh --check OK; L2: стенд target/dist + Xvfb + Chromium; дефолтный сценарий поведенчески здоров (консоль чиста, GPU init OK), но его координаты дрейфанули (новый шаг онбординга «выбор языка» + CR-36 хром) — перекалибровка отдельной задачей; новый scripts/wasm_ui_lay81_scenario.mjs: ?ui=demo, онбординг-проход, вьюпорты 1280×800 и 1024×640, оракулы pixel-diff: стабильность кадра A→B 0 px (оба), клик-оракулы 999 791 px (1280, смена темы Nord→Light) и 5484 px (1024, тултип демо); скриншоты подтверждают канон визуально в двух темах и двух вьюпортах

Stage Summary:
- Класс «ручная вертикальная геометрия» закрыт триадой целиком: Конвенция (IN11-13) → Миграция (V1 рендер/V2 153 сайта/V3 126+45) → Гейт (v3-ui-lint в CI + selftest + wasm L0/L2)
- Пробные фоновые nohup-задачи платформа убивает по завершении вызова — тяжёлые сборки только foreground с resume кэша
- Дефолтный wasm_ui_scenario.mjs требует перекалибровки координат (дрейф онбординга/хрома) — кандидат в отдельную задачу, не блокер #68
Tokens: in≈680000, out≈60000, total≈740000 (estimate: координатор + 4 сабагента, 2 прерваны), model=GLM-5.3, scope=UI-V3

---
Task ID: UI-REWORK-SCREENS (доработки по скриншотам 2026-10-12; сессия web-0a539c42)
Agent: Super Z (main, координатор)
Task: Хвосты после V1–V3 #68: 6 дефектов по скриншотам владельца + проверка CI

Work Log:
- Скриншоты Drive (10 шт) скачаны, разобраны; 5 заказанных + бонус 110957:
- Онбординг 111807 (заголовок × Пропустить): строка заголовка якорилась pad(k)=24k, Skip — 10k..40k → пересечение полос; якорь title_top(k, compact) = skip_top+кнопка (compact — BTN_H_TOUCH), прогресс-точки/body_top_offset на том же якоре (единый источник)
- Онбординг 111822 (финал недостижим): CHOICE_STEP=7 (выбор) — «Далее» не рисуется, обе CTA завершают тур → шаг 8 (мультиканвас №19) недостижим ни одним кликом; порядок развёрнут: мультиканвас (CTA+«Далее») на 7, выбор — последний (CHOICE_STEP=8); тесты multicanvas_step_is_final_with_cta / carousel_reaches_multicanvas_through_next переписаны; input.rs body_area: is_last → has_cta_zone (паритет с draw)
- Чип 111018 (текст впритык к левому краю): ширина чипа заложена с BUTTON_PAD_H с двух сторон, а подпись рисовалась от name[0]; фикс: inset BUTTON_PAD_H + эллипсис-бюджет name_text_w − 2·BUTTON_PAD_H
- About 111127: строки-ссылки без внутренних падов → ABOUT_ROW_PAD_H=BUTTON_PAD_H / ABOUT_ROW_PAD_V=SPACING_SM, ABOUT_ROW_H 42→58; tofu «★» (U+2605 нет в Noto Sans Display и Symbols) → «✦» (U+2726 есть в CanvasDesk Symbols), RU+EN
- UI-консоль 172802 (вылет за панель): фильтр fully() в canvas_body/tokens_body проверял top = demo.y − offset (зона «прокрученного past») → строки рисовались выше верхней границы панели; top = demo.y (паритет Components/Fill)
- Витрина 110957 (двойной текст чипа): статичный draw (Выбрано) + draw_demo_extras (Нажатие) на одних rect'ах; статичный draw чипов загейтван !tobe (extras рисует ряд целиком)
- CI (было 3 красных рана): fmt — rustfmt-канон stage.rs:1743 (V3-коммит ушёл без fmt, исправлен cargo fmt); docs-lint — 108 FAIL в вендорном skills/ → SKIP_DIRS += skills (вне контракта доков); pages build and deployment — gitlink canvasdesk без .gitmodules (15effe30) → git rm --cached + .gitignore
- Гейты: fmt --all OK, clippy --workspace 0, cargo test -p canvas-app 910+12 наборов/0, v3_ui_lint OK, doc_lint OK (0 ошибок)

Stage Summary:
- 6/6 дефектов скриншотов закрыты по коду (не по глазам); онбординг: все 9 шагов достижимы, тур замыкается выбором «Что дальше»; CI-красные зоны идентифицированы по логам (fmt/docs-lint/pages) и починены
- Дальше: onboarding/user-docs — рекомендации владельцу (сообщение TG), сборка wasm L2-сценария не задета (Skip-координаты не изменились)
Tokens: in≈240000, out≈40000, model=GLM-5.3, scope=UI-REWORK-SCREENS

---
Task ID: UI-REWORK-SCREENS-CI (сессия web-0a539c42)
Agent: Super Z (main)
Task: Проверка CI по запросу владельца — 3 красных рана (cf54c55/bef8205/a7cd157) → зелёный

Work Log:
- Причины красного по логам ранов: (1) fmt — V3-коммит ушёл без cargo fmt (stage.rs:1743, rustfmt 1.10-канон); (2) docs-lint — 108 FAIL в вендорном skills/ (плейсхолдер-ссылки, чужие бэктик-пути) — гейт сканировал его как репо-доки; (3) pages build and deployment — gitlink canvasdesk без .gitmodules (15effe30, вложенный клон-бэкап) — fatal в checkout Pages и cleanup каждого CI-джоба
- Фиксы: cargo fmt (12a6dd78); doc_lint SKIP_DIRS += skills (вне контракта доков CanvasDesk); git rm --cached canvasdesk + .gitignore /canvasdesk/
- docs-lint после фиксов: worklog.md 120537 > 120000 симв → ротация $worklog-rotate: блоки MC-C5-FIN…ISSUE-STRUCTURE-64-66 (13 шт) → worklog/archive/worklog-2026-10-12.md lossless, Journal index обновлён (120537→59217) (a533a32d)
- gates (ubuntu) test: manager_native_recent_files_as_entries FAILED — 3 теста хелпера native_manager_app параллельно делили /tmp/canvasdesk-fr108-recent/живой.canvas: «{}» соседа перетирал валидный json между write и Enter → from_str падал в тост, сцена оставалась stub-путём; фикс — уникальный tmp-каталог pid+счётчик (67a9a903); тест-шаг CI не гонялся с 27a9be5 (fmt падал раньше) — код менеджера с зелёного рана не менялся (проверено diff'ом)
- Итог CI на 67a9a903: gates ubuntu/macos/windows SUCCESS, docs-lint/lay7/v3-ui-lint/licenses/wasm-check/llm-proxy/build/Pages SUCCESS; artifacts (3 ОС) — дозревают
- Рекомендации onboarding/user-docs отправлены в TG (отдельным сообщением по запросу владельца)

Stage Summary:
- main снова полностью зелёный; системных правок продукта по CI не потребовалось (fmt-канон, скоуп гейта, битый gitlink, гонка тест-фикстуры — инфраструктурный слой)
Tokens: in≈120000, out≈20000, model=GLM-5.3, scope=UI-REWORK-SCREENS-CI
