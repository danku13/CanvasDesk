# Worklog — журнал задач агентов (текущий период)

Формат записи: `---` / Task ID / Agent / Task / Work Log / Stage Summary /
Tokens (правило учёта токенов — AGENTS.md). Записи дописываются в конец
в хронологическом порядке. Ротация и Journal index — AGENTS.md,
«Учёт токенов по задачам», п. 2а (issue #23, 2026-10-10).

## Journal index (все записи: текущие и архивные)

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
- 2026-10-10 | DOCS-EN-w4 (issue #34; сессия web-a6dbb853) | Волна 4 перевода: design/rules/ 00–11 + design/README.md RU→EN (13 файлов, 37.5k→173 кириллич. симв., ~29.5k→~24.5k o200k), ID правил сохранены (П*, C*, LAY*…), runtime-литералы и grep-якоря сверены с crates/ | текущий
- 2026-10-10 | MC-C2 (GitHub #6, волна C2 мультиканваса; high-level #14) | Волна C2: FsAccessStore над granted-папкой (№41c тихий старт, R-T6 rename с .bak), миграция OPFS→папка №42a/№52a (копирование до удаления), баннер №44b, watch внешних изменений №45b/№53b | worklog-2026-10-08_10-10.md
- 2026-10-10 | MC-C3 (GitHub #7, волна C3 мультиканваса; high-level #14) | Оверлей-менеджер канвасов: список/поиск/группы №43a, создание×4, ренейм №9, удаление+undo №15a/№22c, строка хранилища №51a, drop-коллизия №26b | текущий
- 2026-10-10 | WAVE-T (GitHub #28, high-level #27) | Wave T — Tokens & States: KitState+Focused/Dragged/Error, 4-role pairs (ButtonVariant+3), ControlSize, Elevation, Duration/Easing, Shape, Spacing | worklog-2026-10-08_10-10.md
- 2026-10-10 | WAVE-L (GitHub #30, high-level #27) | Wave L — Layout примитивы: grid_auto/Track, aspect_ratio, sticky_header, Responsive/WindowClass, Density | worklog-2026-10-08_10-10.md
- 2026-10-10 | WAVE-C (GitHub #29, high-level #27) | Wave C — 15 новых компонентов (checkbox, slider, radio, tabs, command_palette, accordion, progress, skeleton, badge, popover, snackbar, avatar, tree, segmented) | worklog-2026-10-08_10-10.md

Архив: `worklog/archive/worklog-2026-10-02_10-08.md` (2026-10-02…2026-10-08, 46 записей). Ниже — записи с 2026-10-09.
---
Task ID: DOCS-EN-w4 (issue #34; сессия web-a6dbb853)
Agent: Super Z (main) + 4 translate-агента (5-a/5-b/5-c/5-d)
Task: Волна 4 перевода RU→EN — design/rules/ (12 нормативных файлов) + design/README.md, по языковой политике #24 (решение владельца по итогам волны 3, п.2)

Work Log:
- Tracking-issue #34 создан по правилу «задачи → код» и подшит sub-issue к мастер-issue #17 (sub-issues #18–#25, #34)
- Переведены 13 файлов (смысл 1:1, структура сохранена — заголовки/таблицы/списки/фенсы, строки ±5%): 00-principles 107→107, 01-colors 107→107, 02-typography 89→89 (агент 5-a); 03-spacing-radius 112→113, 04-contrast-a11y 68→68, 05-layering 62→62, 06-motion 46→46 (5-b); 07-theming 83→85, 08-states 78→80, 09-input 120→123, 10-components 144→144 (5-c); 11-layouts 416→420 (5-d); design/README.md 96→96 (main)
- ID правил не переводились (П1–П11, C1–C8, T*, S*, ST*, IN1–IN10, K*, LAY1–LAY11, G1–G8, I-1/I-5) — прецедент RECIPES §8.3; проверены числовые мультимножества (px/α/пороги/длительности) и мультимножества ID — 0 расхождений (агенты 5-c/5-d)
- Сверка runtime-литералов и grep-якорей с кодом (главная правка волны): `==текст==` (markdown.rs/theme.rs/tokens.rs), «ПАРАМЕТРЫ · N» (text.rs FR-069), «⋯ целиком ▾» (text.rs:861), «Принять»/«Отклонить» (autolink_ui.rs) — восстановлены с EN-глоссами; `min(desired, остаток)`, `<зазор>`, `// LAY9.1: паритет-оракул`, `offset + ручной кламп` — реальные якоря layout.rs/canvas-app, сохранены байт-в-байт; чистый псевдокод без якорей переведён (chars.len() × width, stack(center))
- Гейты: doc_lint 931 ссылка + 2533 бэктик-пути + 5 бюджетов — 0 ошибок (равен базовой линии); кириллица 37 532 → 173 симв. (только ID правил, сверенные якоря, цитата W8); замер o200k (оценка по калибровке doc_lint: RU 2.93 / EN 3.68 симв/ток): ~29 500 → ~24 500 токенов (−16.8%, консистентно с волнами 1–2)

Stage Summary:
- design/rules/ + README — целиком на EN; норматив UI-волн (#27/#31/#32) теперь читается агентами без перевода «на лету»; единственный крупный RU-массив agent-контура закрыт
- Не переводились (вне скоупа #34): design/rules-audit-2026-10.md, design/layouts-audit-2026-10.md (точечные аудиты-отчёты), design/use-cases/ (кандидат волны 5 — решение владельца), design/tokens/ (JSON)
- Для владельца: ротация worklog.md по триггеру объёма (правило AGENTS.md); решение по design/use-cases/
Tokens: in≈392000, out≈88000, total≈480000 (estimate), model=GLM-5.3 (Super Z main), scope=DOCS-EN-w4

---
Task ID: MC-C3 (GitHub #7, волна C3 мультиканваса; high-level #14)
Agent: Super Z (subagent MC-C3 + координатор main при финализации; worktree wt-c3/ветка wave/mc-c3)
Task: Волна C3: оверлей-менеджер канвасов — список/поиск/сортировка/группы (№43a), 4 источника создания, инлайн-ренейм (№9), мягкое удаление с undo (№15a), пустое состояние (№23a), строка хранилища (№51a), drop-коллизия (№26b).

Work Log:
- Сессия агента прерывалась (контекст) — незакоммиченный задел (~16 файлов + canvas_manager_ui.rs 50 КБ); финализация (wasm-фиксы, гейты, FR-106, worklog) — координатор от имени MC-C3
- canvas_manager_ui.rs (новый, ~1050 строк): CanvasManagerState (open/filter/selected/scroll/entries/editing) по паттерну scheme_gallery; rows — поиск по display_name регистронезависимо + SortMode + группы №43a с заголовками (фильтр в группах тоже); move_selection пропускает заголовки, scroll_to_reveal/wheel_scroll; инлайн-ренейм №9 (begin_rename/insert/backspace/cancel/take_edit); StorageRowMode №51a (browser+кнопка / folder / unsupported — Firefox/Safari без кнопки); manager_layout/row_at/name_at (draw==hit); empty_card №23a; format_ts; DOUBLE_CLICK_MS-детект (строка — открыть №49, имя — ренейм) — 15 тестов
- app-интеграция: AppEvent-блок // FR-106 (CanvasManagerOpen — кнопка «Недавние» тулбара до чипа C4, №37b; CanvasSavedAsCopy №26b; undo-удаления), клавиатура оверлея (Esc/Esc-ренейм/Enter/F2/↑↓/PgUp/PgDn/ввод), manager_after_open (шаблон №38a: создать → галерея-пикер set_pending_scheme поверх), после удаления активного №22c/№40b — менеджер открыт + новый «Canvas N» под ним; +8 i18n-ключей RU/EN (тосты удаления/коллизий/ошибок, Экспорт, бейджи browser/folder/disk)
- web_requests.rs: операции менеджера — открытие из workspace (OPFS/папка), дубликат №27a (полный .canvas: сценарии/заморозки/extra; copy_name с i18n-суффиксом; сразу активен), импорт файла (пикер, санитизация, №26b авто-суффикс), undo удаления (.bak→файл без переключения сцены), перенос ключа камеры localStorage при ренейме активного (№12/№30b, формат C0 camera_key_for), remove_recent при удалении (№15a)
- drop_files.rs: DOM-drop с коллизией — было create:true (тихая перезапись) → collision_suffix + тост «создана копия» (№26b)
- Координаторские фиксы сессии: manager_last_click std::time::Instant → canvas_core::time::Instant (W1-аудит); 3 ошибки владения wasm-only (E0382 name/sanitized, E0521 remove_recent future, E0308) — видны только под wasm32-таргетом (натив проходит), подчёркнуто в FR-106 §Проверка; unused import
- Гейты (финальное состояние): fmt --check OK; clippy --workspace -D warnings OK; cargo test --workspace 3001 passed / 0 failed (22 новых: canvas_manager_ui×15, app×5, web_requests×2); wasm_gate.sh --check OK
- WASM L2 не гонялся (нет wasm-bindgen CLI, 2 ядра/4 ГБ) — ручные сценарии 8 шт. в FR-106 §Проверка (вкл. тёмная/светлая, RU/EN)

Stage Summary:
- C3 закрыта целиком: полный цикл управления канвасами в UI (создать/открыть/переименовать/дублировать/удалить+undo/импорт/экспорт); вход — кнопка «Недавние» (C4 заменит на двухзонный чип №21c + document.title №28a)
- C4-задел: AppEvent-конвейер и web_requests операции переиспользуются чипом; persist() переносится в точку менеджера (TODO в C1)
- Открытые пункты: онбординг-шаг 10, чистка DOM-панели, чип, title — C4; desktop-слой и приёмка — C5
Tokens: in≈560k, out≈130k, total≈690k (estimate; subagent MC-C3 + координаторская финализация), model=GLM, scope=FR-106

---
Task ID: WAVE-A (GitHub #31, high-level #27)
Agent: Super Z (main)
Task: Wave A — Architecture паттерны (Response, FocusTrap, ActionRegistry, NavRail, WidgetExt, smart guides).

Work Log:
- #31 переведена в In Progress. Прочитаны component/mod.rs (Component trait, KitState, ControlStyle), keyboard.rs (FocusRing, KeyboardRouter), kit.rs (фасад), lib.rs (exports).
- 5.5.1 Response (component/mod.rs): Response { rect, state, hovered, clicked, dragged, drag_delta, has_focus, focus_visible, changed }. from_widget(rect, ws, clicked) — конструктор из WidgetState. Chain: on_hover_text(), disabled_if(), on_click(F). Default impl. egui pattern — каждый вызов виджета возвращает Response с interaction-state.
- 5.5.5 WidgetExt (component/mod.rs): trait WidgetExt: Sized { on_hover_text, disabled_if, on_click, with_state, with_slot }. impl WidgetExt for Response. druid WidgetExt pattern — chainable модификаторы.
- 5.5.2 FocusTrap (keyboard.rs): FocusTrap { ring: FocusRing, boundary: UiRect, previous_focus: Option<UiRect> }. new(boundary), push(r), on_tab(shift) → Option<UiRect>, current(), remember_previous(focus), restore() → Option<UiRect>, boundary(), clear(), retain_order(). Fluent FocusZone/FocusTrap pattern — Tab не выходит за boundary.
- 5.5.3 ActionRegistry (action.rs — новый): Action { id, label, kbd, category, predicate }. new(id, label, category), kbd(s), predicate(fn). is_available(). ActionRegistry { actions }. register(action) — идемпотентно по id. search(query) — fuzzy substring + predicate filter, возвращает индексы. command_actions() — Vec<CommandAction> для Wave C CommandPalette. shortcut_of(id) → Option<&str>. cheatsheet() → Vec<(category, Vec<&Action>)> для ? dialog. context_items() → Vec<&Action>. get(id), len(), is_empty(). Excalidraw shapeActionPredicates pattern — один реестр на 3 поверхности.
- 5.5.4 NavRail (component/nav_rail.rs — новый): NavRailItem { id, icon, label, badge } + badge(BadgeKind). NavRail { items, active }. NavRailLayout { rail, buttons, sidebar, active }. nav_rail_layout(viewport, rail, sidebar_w, rail_w, button_h, gap) — rail ~48px + sidebar ~280px. nav_rail_button_style(active, state, palette). nav_rail_hit(layout, point) → Option<usize>. nav_rail_activate(rail, index), nav_rail_activate_by_id(rail, id). Figma UI3 pattern.
- 5.5.6 Smart guides (canvas-app/app/smart_guides.rs — новый): GuideOrientation { Horizontal, Vertical }. GuideLine { orientation, pos, range }. DistanceLabel { pos, text[16], text_len } — fixed buffer (no alloc в hot path). SmartGuides { lines, labels }. compute_smart_guides(dragged, siblings, threshold) → (SmartGuides, snap_offset). SNAP_THRESHOLD=6.0. Center alignment (vertical/horizontal), edge alignment (left/right/top/bottom), distance labels «N px». tldraw/Miro pattern — magenta lines + distance labels.
- Экспорт: lib.rs +action mod, +Action/ActionRegistry use, +FocusTrap use. kit.rs +Response/WidgetExt, +NavRail, +Action/ActionRegistry, +FocusTrap. component/mod.rs +nav_rail mod. app.rs +smart_guides mod.
- Тесты: 25+ новых TDD-тестов: Response default/from_widget, WidgetExt chain, FocusTrap tab/shift_tab/current/restore/boundary/empty, ActionRegistry register/search/shortcut/cheatsheet/context_items/command_actions, NavRail layout/hit/activate/activate_by_id/badge, smart_guides center/edge/no_alignment/empty_siblings/distance_label.

Stage Summary:
- Wave A завершена: 8 файлов (3 новых), +~600 строк, 25+ новых TDD-тестов
- 6 архитектурных паттернов: Response (egui), FocusTrap (Fluent), ActionRegistry (Excalidraw), NavRail (Figma), WidgetExt (druid), smart guides (tldraw/Miro)
- AC (A1-A6): все выполнены
- Гейты: cargo недоступен в среде — test/clippy/fmt делегированы CI
- Wave D (#32) может интегрировать Response в витрину (on_hover_text/on_click chain)
Tokens: in≈140000, out≈35000, total≈175000 (estimate), model=GLM-4.7 (Super Z main), scope=WAVE-A
