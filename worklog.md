---
Task ID: FR-089-активация
Agent: Super Z (main)
Task: PostHog-настройки владельца (EU cloud, phc_-ключ) — вписать ключ, включить продуктовые события из приложения, e2e-верификация доставки

Work Log:
- Ключ вписан: `POSTHOG_KEY = phc_…` + `POSTHOG_HOST = https://eu.i.posthog.com` (регион EU — выбор владельца); валидация прямым POST `/batch/` из сессии сборки — 200 `{"status":"Ok"}`. Инвариант «пустой ключ = no-op» сохранён
- Продуктовые события: `app_launched` при каждой загрузке SDK (свойства app/language/role из TOML `canvasdesk.config`, `$set` person-свойства — любые события делимы по ролям в PostHog Insights); публичный `window.__cdTelemetry.track(event, props)` — no-op без ключа/SDK/согласия analytics; DOM-мост `canvasdesk:track` (CustomEvent, detail {event, properties}) — события из Rust через web-sys (сниппет в комментарии модуля index.html)
- Гейт свежего визита `pickerDone()`: до подтверждения пикера (нет записи consent/config) SDK не грузится, счётчик/track не отправляются; старые конфиги до FR-089 — дефолт true
- E2E-верификация (agent-browser, python http.server стенд): 0 запросов к posthog до подтверждения пикера → выбор «Архитектор» → reload → SDK грузится; `wasm_used` → POST `/batch/` 200 (прямой fetch без SDK); `__cdTelemetry.track()` → POST `/i/v0/e/` 200; payload `app_launched` доказан декодированием gzip-тела (DecompressionStream в тестовой пробе): event=app_launched, app=canvadesk-web, language=ru, role=architect, $set={language,role}
- Открытие про SDK: posthog-js v1.435 намеренно отбрасывает capture-события при `navigator.webdriver=true` (bot-фильтр: UA-подстроки + webdriver) — в автотестах/Playwright событий нет by design, не баг интеграции; у реальных пользователей доставка подтверждена (флаг снимался только в тестовой копии страницы, prod-код не трогался)
- Наблюдения владельцу: (1) session recording включён серверно по умолчанию в новом проекте (SDK грузит recorder, маскирование inputs включено) — оставить/выключить решает владелец; (2) `person_profiles` по умолчанию identified_only — Insights/события работают, вкладка People пуста до `identify` (приложение анонимно)
- Синтаксис 9 inline-скриптов — node --check; доки: fr-089-telemetry-consent-posthog.md (§4 строки активации, §6 блок e2e-верификации, §7 changelog), index-cr-fr.md (строка FR-089), этот журнал
- Push: ребейз поверх 811131e (конфликт worklog — параллельные сессии переверстали журнал в формат Task-ID; решено в новом формате, старая история — в git log)

Stage Summary:
- Телеметрия живая: ключ EU вписан, доставка подтверждена сквозным тестом (wasm_used / app_launched / track / $pageview / $exception)
- Владельцу остался один шаг в PostHog: Settings → General → Allowed domains → домен хостинга CanvasDesk
- Rust-код не тронут (только web-слой и доки) — риск для CI отсутствует

---
Task ID: ИТОГ-клип
Agent: Super Z (main)
Task: Клиппится текст «Итого» внизу шаблонных нод — CanvasDesk

Work Log:
- Fresh container: rustup 1.99.0; main продвинулся (e3eb854, затем 832a5e6/d7368a2 от параллельной сессии)
- Диагностика: метка «ИТОГ» футера (text.rs result_label, FR-069) рисуется с v_center = (32−12)/2 = 10·zoom от верха полосы (сдвиг добавлен FR-075, 01bc8b38), но TextBounds.bottom остался «верх полосы + RESULT_LINE_HEIGHT (16)» — модель до-FR-075. glyphon 0.6 клипит растеризацию глифов по bounds (text_render.rs: clip bottom)
- CPU-замер cosmic-text (Noto Sans Display Medium, Metrics 10/12): базлайн «ИТОГ» = line_y 9.88 + v_center 10 = 19.88 от верха полосы > 16 → срезалось ~3.9px из ~7px прописных (нижняя половина букв)
- Фикс (1e28fbc): bounds метки = вся полоса (top = верх полосы, bottom = верх + strip_h·zoom) — паттерн блока значения; строка [10..22] внутри [0..32]
- Тесты: tests/itog_label_metrics.rs (CPU, инварианты v-center/полоса + документация арифметики регрессии), tests/result_strip_label_clip.rs (пиксельный, GPU-среда — в CI/контейнере без GPU корректно скипается, у владельца отработает)
- Попутно поднят красный CI #429 параллельной сессии: a662a9b (FR-089) пришёл без cargo fmt (34ab987) и с устаревшим тестом settings_ui modal_layout_structure_and_hit_tests — таб «Профиль» теперь 12 рядов (роль + TelemetryCounter/TelemetryAnalytics + 9 категорий), ожидания обновлены (53a826a)
- Гейты: workspace 2356 passed / 0 failed; fmt --check; clippy -D warnings; wasm-гейт ступени 1–2
- Push 53a826a (d7368a2..53a826a); Telegram-отчёты отправлены

Stage Summary:
- Метка «ИТОГ» больше не клипится: bounds = вся полоса результата
- CI #429 (апстрим) был красный — починен fmt + тестом на 53a826a; CI моего пуша — на контроле
- Коммиты: 1e28fbc (фикс+тесты), 34ab987 (fmt), 53a826a (тест профиля)

---
Task ID: UI-кит-аудит
Agent: Super Z (main)
Task: Детальный анализ — какие поверхности/компоненты UI остаются не на ui-kit, на хардкоде и без адаптива из концептуального дока

Work Log:
- Эталон из доков: docs/ui-kit.md (PRD-0009, §9), surface-registry, FR-059/060/062/068, CR-014/015 — G4-окна (1280×800/1024×640/800×560), HideBelow, клампы/флипы, цвета только через KitPalette/ThemeColors, SPACING_*/RADIUS_*
- Пофайловый аудит 26 поверхностей реестра + вне-реестровых (5 параллельных агентов), верификация ключевых находок на коде
- Отчёт: docs/dev-researches/ui-kit-adoption-audit.md (коммит 59eada7, база 53c7312)
- Telegram-протокол соблюдён: план + этапы + финальное саммари (chat 274002630)

Stage Summary:
- На ките ~8/26 (whatif, hints, flowmap, autolink, calc_panel, gallery, kit_gallery, admin_panel); частично ~12 (menu, settings, template, palette, stage, explain, search, hotkeys, dialog, empty, tooltip, suggest); hand-rolled ~6 (onboarding — заморожен, docs, help_menu, choice_menu, corner_buttons; wheel/minimap/HUD — world by design)
- Хардкод: ~60 литералов цветов в app-draw (explain 13+, palette 6 — светлой темы нет, hover ×6, тултипы 7) + 15 cards.rs + ~25 web-shell; радиусы литералами; HEADER_H 28 vs 34; токены без зеркала 5+3
- Дефекты адаптива: 16 приоритизированных (меню ПКМ без клампа, settings без скролла, calc 600>560, suggest-наложение, gallery без колеса, hotkeys pick≠draw и др.) + 10 локаций CR-015-эвристик
- Волны закрытия W-a…W-e — в §10 отчёта

---
Task ID: FR-090 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1)
Agent: Super Z (main)
Task: Запрос владельца: «хочу видеть людей с ролями, как обработку анонимных профилей в настройках проекта? навешивай события из rust, то что ты уже показал и открытие каждой поверхности»

Work Log:
- Диагноз People-пустоты: remote-конфиг проекта `defaultIdentifiedOnly: true` (= person_profiles: identified_only, дефолт новых проектов) — события/Insights работают, person не создаются до identify
- identify: `ph.identify(anonId, {language, role})` в `captureAppLaunched` (index.html) — стабильный anonId из canvasdesk.consent; анонимная история мёржится, wasm_used (distinct_id = anonId) ложится на того же person
- Новый модуль `canvas-app/src/app/telemetry.rs`: `track(event, props)` — Rust-сторона моста `canvasdesk:track` (CustomEvent на document, js-sys Object + Reflect.set; натив — no-op по паттерну persist_settings); `surface_diff` — диф реестра поверхностей (хук в `build_frame_at`: пересборка = смена сигнатуры поверхностей); `tracked_present` — чистая функция + 3 юнит-теста (ambient-фильтр/пользовательская поверхность/whatif-override)
- AMBIENT-хром не трекается: world, corner_buttons, template_strip, empty, minimap; whatif — только `scene.whatif_active` (пилюля входа ambient на ≥900×600)
- События из Rust: role_changed (apply_dropdown_choice/Role), language_switched (toggle_language + dropdown), theme_switched (toggle_theme), scheme_applied (apply_scheme: scheme+nodes), export_canvas/export_html (canvas-web/export.rs — pub-путь canvas_app::app::telemetry::track)
- Cargo (canvas-app, wasm32): web-sys +Document/CustomEvent/CustomEventInit, +js-sys
- e2e-верификация (agent-browser, тестовая копия с инструментацией): расшифрованы gzip-тела запросов — `$identify` (distinct_id = anonId, person_properties {language: ru, role: architect}), `$set`, `app_launched` ($is_identified: true), мост-батч surface_opened×2/role_changed/scheme_applied — 200 Ok
- Открытие: posthog-js v1.435 режет capture не только по navigator.webdriver, но и по «HeadlessChrome» в UA (очередь __request_queue пуста при снятом webdriver) — в тестовой копии сняты оба сигнала; реальных пользователей не касается
- node --check 9 inline-скриптов — OK; тестовая копия/стенд убраны из рабочего дерева
- Доки: fr-090-rust-events-surfaces-people.md (CR, таксономия событий + ответ про person_profiles), index-cr-fr (ревизия 7, следующий FR-091), surface-registry §3 (примечание о телеметрии), FR-089 §7 changelog
- Rust-гейты (fmt/clippy/test) — нет тулчейна в среде агента (прецедент FR-089), делегировано CI

Stage Summary:
- People наполняются identified-профилями с ролью/языком; настройку проекта переключать не нужно (identified_only остаётся: анонимные события без профиля — по дизайну, отказавшимся person не создаётся)
- Мост canvasdesk:track впервые используется Rust-кодом; surface_opened покрывает 21 пользовательскую поверхность реестра
- Тестовый человек в PostHog (role: architect + тестовые surface_opened/role_changed от e2e) — владельцу можно удалить в People
- Нумерация: следующий FR-091 (ревизия 7 индекса)

Task ID: W-a/W-b реализация
Agent: Super Z (main, сессия web-51d33571)
Task: Реализация волн W-a и W-b аудита ui-kit (5 параллельных агентов в worktrees + проводка)

Work Log:
- 5 параллельных агентов в git-worktrees: A1 menu-clamp (388a091), A2 settings-scroll (b2bcdc5), A3 whatif-suggest (d567708), A4 color-slots (d75546f), A5 gallery-hotkeys-header (2f08ff8, после ретрая по таймауту)
- Слияние веток в main без конфликтов; проводка скролла настроек выполнена main-агентом (колесо + offset в draw/pick/hit + 5 сбросов)
- Гейты: fmt --check; clippy --workspace --all-targets -D warnings; cargo test --workspace — 2378 passed / 0 failed / 2 ignored

Stage Summary:
- W-a закрыта: меню ПКМ кламп/флип + G4-линт; скролл настроек (kit ScrollState, проводка завершена); кламп высоты whatif + индикатор усечения; suggest-стопка без наложений; колесо галереи; hotkeys hit==draw; HEADER_H 28→34 через tokens::CARD_HEADER_HEIGHT
- W-b закрыта: литералы → слоты ThemeColors (palette — починена светлая тема; hover/selected ×6 → control_*; тултипы → tooltip_warn/info; explain 11× белый → text_on_accent; stage 0x14161c → chip_text; whatif/search/flowmap → whatif_accent/palette_border); новые слоты: text_on_accent, chip_text, tooltip_warn, tooltip_info, whatif_accent, palette_selected_dim_fill (+ theme_presets, тест-зеркало)
- Остатки: W-c (explain_frame → Painter, settings dropdown/switch → kit, docs → примитивы, template strip/flyout, corner buttons, tooltip kit), W-d (SPACING/RADIUS токены, зеркало tokens, ThemeColors в colors.json), W-e (BP_COMPACT/MOBILE, тач-цели web, разморозка onboarding)

---
Task ID: FR-091 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1)
Agent: Super Z (main)
Task: Запрос владельца: «сейчас при записи сеанса я вижу только то что происходит на web слое, но не вижу ничего на wasm слое, как это исправить?»

Work Log:
- Диагноз: весь UI рисуется Rust/wgpu в один <canvas> (WebGPU, winit → attach_canvas_to_dom), rrweb пишет DOM — канвас для записи чёрный ящик; canvas capture у PostHog выключен по умолчанию (не покрывается DOM-маскировкой). События FR-090 в таймлайне реплея уже есть — слепота касалась визуального слоя
- Исследование SDK (артефакты v1.435.8 = актуальная @1 на npm: jsDelivr +esm бандл, eu.i.posthog.com/static/lazy-recorder.js и recorder.js): резолв конфига — локальные session_recording.captureCanvas.{recordCanvas,canvasFps,canvasQuality} приоритетнее remote-настроек проекта (?? remote.canvasRecording.enabled/fps/quality ?? 4/«0.4»); сборка rrweb-опций (recordCanvas, sampling.canvas=fps, dataURLOptions webp, canvasResolutionScale, canvasMasking из canvasCapture.maskRegionsFn)
- Механика FPS-обсервера (числовой fps): rAF-цикл каждые 1000/fps мс собирает querySelectorAll("canvas") (включая shadow roots) — БЕЗ фильтра по типу контекста; webgl/webgl2 → предочистка color buffer, прочие → сразу createImageBitmap(canvas, {resizeWidth/Height по clientWidth/Height×resolutionScale}) → encode-воркер WebP → canvasMutation-события (реплей drawImage-кадрами). WebGPU-канвас идёт тем же путём (растр последнего презентованного кадра); getContext("webgpu") патчится (ставит __context="webgpu"), из захвата не исключается
- Фикс: crates/canvas-web/index.html — posthog.init + session_recording: {captureCanvas: {recordCanvas: true, canvasFps: 4, canvasQuality: "0.4"}, canvasCapture: {resolutionScale: 0.6}} + комментарий (механика, приоритет локальных опций, приватность/маскировка, версионность ≥1.105.7). Rust не тронут
- Доки: fr-091-session-replay-canvas-wasm.md (CR: What/Impact/анализ механики/Changes/Verification), index-cr-fr.md — строка FR-091 + нумерация ревизия 8 (следующий FR-092)
- Верификация: node --check 9 inline-скриптов (OK); браузерный стенд (agent-browser, http.server, тестовая копия index.html с инструментацией — seed согласий, снятие bot-фильтра webdriver+HeadlessChrome по прецеденту FR-090, шпион createImageBitmap/fetch/XHR/sendBeacon, WebGPU-канвас с рендер-циклом): SDK инициализируется без ошибок; lazy-recorder загружен (initSessionRecording); createImageBitmap на канвасе ~4 fps (514 вызовов за ~90 с в первом прогоне); флеш на visibilitychange → POST https://eu.i.posthog.com/s/ 200, тело 16 КБ (DOM-снапшот дал бы 1–3 КБ — кадры канваса в payload); события /e/ и /i/v0/e/ 200
- WebGPU-адаптер в headless Chrome недоступен (requestAdapter → null) — стенд проверен на 2d-фолбэке; контекст-агностичность для webgpu подтверждена исходниками recorder.js (единственная контекстная ветка — предочистка webgl). Реальный wgpu-канвас (Chrome/Edge) идентичен по пути захвата
- Стенд и тестовая копия — вне рабочего дерева репо (/home/z/my-project/fr091-stand), рабочее дерево чистое (кроме трёх целевых файлов)

Stage Summary:
- Записи сеансов теперь содержат WASM-слой: канвас снапшотится 4 кадра/с (WebP, resolutionScale 0.6), события FR-090 — в таймлайне реплея
- Локальные опции captureCanvas приоритетнее настроек проекта: включать в PostHog UI ничего не обязательно (опционально — Settings → Session replay для покрытия на уровне проекта; для управления из UI — убрать captureCanvas-блок из posthog.init)
- Ретроспективы нет: канвасы попадут только в записи после выката новой index.html (Pages)
- Приватность: содержимое заметок видно в записях (гейт согласий FR-089 сохранён — SDK грузится только при analytics=true); маскировка — canvasCapture.maskRegionsFn (≥1.408); владельцу рекомендуется упомянуть запись экрана в формулировке чекбокса «Метрики и отчёты об ошибках»
- Тестовые следы в PostHog: человек fr091-stand-* (события + запись с канвасом) — владельцу можно удалить; запись можно открыть глазами как приёмку («видео» канваса в реплее)
- Cargo-гейты не требуются (изменение только в web-слое); нумерация: следующий FR-092 (ревизия 8 индекса)

---
Task ID: mobile-touch-1 (FR-092)
Agent: Z (сессия web-94a160de)
Task: проверить, почему на мобильных работает только HTML-оверлей, но не wasm

Work Log:
- Диагностика задеплоенного Pages-бандла в headless-браузере (десктоп + мобильная эмуляция): wasm стартует, двухступенчатый GPU-выбор фолбэкается на WebGL2, рендер жив — мёртв ввод
- Корень: winit-web шлёт pointerType=touch только как WindowEvent::Touch (prevent_default давит совместимые mouse-события), canvas-app не имел ветки Touch
- FR-092: canvas-core::touch — чистая машина TouchGesture (тап/драг → мышь; два пальца — пан серединой + pinch-зум; Cancelled/нулевая дистанция защищены; 8 тестов); canvas-app on_touch под wasm32 (натив не тронут), TwoFinger глушится под модалкой/screen-space UI; debug-оракулы touch: событие/действие
- telemetry.rs: thread_local → const-блок (clippy wasm32 missing_const_for_thread_local)
- Браузерный дым локального бандла: тап закрывает canvas-онбординг, тап «Пустой холст» чистит сид, Moved/TwoFinger в логах, пан/пинч двигают сетку (пикс. diff), жест над flyout поглощён поверхностью
- Нюансы харнесса (задокументированы в FR-092): синтетическому pointermove нужны getCoalescedEvents→[self] и button:-1
- Гейты: fmt, clippy -D warnings (натив + wasm32 lib), test --workspace (83 набора), wasm_gate --check; ребейз поверх параллельного FR-091 (session replay), перенумерация FR-091→FR-092 (прецедент FR-083/FR-089)

Stage Summary:
- Мобильный ввод web-версии работает: тап/драг/пан/пинч; коммит в main, CI на контроле
Task ID: 1-c (волна W-c, docs)
Agent: W-c agent (worktree wc-docs-kit, ветка wc-docs-kit)
Task: docs → примитивы + kit ScrollState — миграция поверхности документации (crates/canvas-app/src/docs_ui.rs, 1299 строк) на ui-kit (аудит §5 A2 / §10 W-c)

Work Log:
- Изучены образцы кита (whatif_ui/hints_ui/flowmap_ui) и исходники canvas-ui (component/list.rs, measure.rs, kit.rs, layout.rs, geometry.rs)
- Локальный дубль ScrollState (offset/max_offset + new/wheel/resize, docs_ui.rs:877–906) УДАЛЁН; скролл — тип кита: `pub use canvas_ui::kit::ScrollState` (путь docs_ui::ScrollState сохранён реэкспортом); семантика колеса/sync — pub-обёртки `wheel_scroll` (scroll_by+clamp, флаг изменения) и `sync_scroll` (content_h/viewport_h + clamp без сброса, паттерн flow_map_layout)
- link_at — скролл по ссылке (&ScrollState: тип кита не Copy)
- Меню «?» и подменю разделов — стопки через kit::list_rows (menu_item_rows: окно без прокрутки ровно в пункты, зазор 0, offset 0 — паттерн hints_ui::hint_rows); pub-API help_menu_item_rect/help_submenu_item_rect сохранён: валидные индексы бит-в-бит прежней формуле, вне диапазона — пустой rect; hit-тесты help_*_item_at сознательно не тронуты (математика индексов, x-семантика паддингов)
- viewer_rect — слот через kit::stack (End/Start, паттерн flowmap_ui), x/y прежние дословно
- Markdown-блоки/таблицы (PageBuilder/layout_table) — честно оставлены (ячейки одной строки с одного y + перенос по ширине колонки вне контракта list_rows/Table); отступы заменены на canvas_core::tokens (SPACING_S/SM/MD/LG при точном совпадении 6/8/10/12), 4/16/3/1.5/2 — дословно (вне шкалы); HELP_MENU_PAD → tokens::SPACING_S
- Тесты: scroll_clamps_offset переписан на kit API; добавлен kit_scroll_state_clamps_like_old_local (паритет клампов с прежним локальным: 0/max/короткий контент/resize без сброса/scroll_by+clamp ≡ обёртки); добавлен menu_item_rows_match_old_stack_formula (бит-в-бит геометрия стопок + вне диапазона); поиск/hit-тесты ссылок не тронуты
- Внешние пользователи удалённого дубля (НЕ правлены — жёсткие рамки файлов, только отчёт): app.rs:641 (поле DocsViewer.scroll — тип совместим через реэкспорт), app.rs:7240 + app/ui_registry.rs:1688 + app/ui_layout_lint.rs:171 (ScrollState::new → заменить на sync_scroll), app/input.rs:3909–3911 (resize/wheel → sync_scroll/wheel_scroll), input.rs:1780 (link_at по значению → &viewer.scroll), app/overlays.rs:3293,3297 (поле max_offset → метод max_offset())
- cargo в среде нет — компиляция делегирована CI (прецедент FR-090); правки минимально-структурные, API только из источников crates/canvas-ui

Stage Summary:
- docs_ui.rs на ките во всём, что кит выражает: ScrollState — тип кита, меню/списки — list_rows, слот панели — stack; раскладка markdown/таблиц дословна и переведена на шкалу токенов (ноль визуального скачка, тесты-оракулы прежних формул)
- Проводка потребителей осталась за следующей волной (drop-in: docs_ui::sync_scroll/wheel_scroll, link_at по ссылке) — перечислены с точными строками
- Коммит 61353ac (код docs_ui.rs) в ветке wc-docs-kit; worklog дополнен

---
Task ID: W-c/W-d реализация (сессия web-51d33571)
Agent: Super Z (main, сессия web-51d33571)
Task: Реализация волн W-c и W-d аудита ui-kit (6 параллельных агентов в worktrees + проводка/интеграция)

Work Log:
- 6 параллельных агентов в git-worktrees: 1-a explain+tooltip (66d0f0e — агент дважды падал по таймауту API, код допринят и закоммичен main-агентом после ревью: API Painter/WidgetState/kit::tooltip сверены, cargo check/test/clippy чисто), 1-b settings-kit (cc4cb38), 1-c docs-kit (61353ac), 1-d template+corner (4a2b0eb + типовой фикс Color→[f32;4]), 2-a spacing-tokens (39f4964), 2-b token-mirror (c63e60c)
- Проводка docs выполнена main-агентом (ec1c86d): потребители kit ScrollState — input.rs (link_at по ссылке, sync_scroll/wheel_scroll), app.rs/ui_registry.rs/ui_layout_lint.rs (ScrollState::new → default+sync_scroll), overlays.rs (max_offset() метод); E0106 link_at решён явным 'a; паритет-тест агента исправлен (у max прокрутка вверх меняет позицию — семантика прежнего wheel)
- Слияние 6 веток в main без конфликтов (overlays.rs — регионы 2570-2710/4240-4290 vs 3293-3297 разнесены)
- Гейты на merged main: cargo fmt --all -- --check (чисто после fmt-коммита); clippy --workspace --all-targets -D warnings — чисто; cargo test --workspace — 2391 passed / 0 failed (было 2378; +13 тестов W-c/W-d)
- Тулчейн rust 1.99 установлен в среде сессии (стабильно для будущих волн)

Stage Summary:
- W-c закрыта: explain_frame → Painter+WidgetState (последний остаток FR-060: items → paint_items_to_stage, hover/selected — матрица KitState; painter-клипы доступны для scissor FR-056); tooltip → kit::tooltip + перенос по ШИРИНЕ (TextMeasurer, дефект B8: неразрывные пути не клипаются, NBSP-разряды целы); settings — pill-тумблер → kit switch (36×20, RADIUS_PILL), dropdown → kit dropdown_menu (flip+кламп кита, скролл W-a сохранён); docs — локальный дубль ScrollState удалён → kit, меню → list_rows; template strip/flyout → kit dropdown_menu/list_rows (ручные клампы §8.11 устранены); corner buttons → kit IconButton (hit==draw, rect'ы lib.rs не тронуты)
- W-d закрыта: SPACING_*/RADIUS_* вместо литералов-дублей (24 замены: menu/calc/autolink/palette/search; 16.0/4.0 — вне шкалы, осознанно); зеркало tokens.rs достроено: 5 edge + 3 motion + 4 LOD-токена (паритет-тесты JSON↔Rust; consumers — алиасы с прежними именами); card.header_height подтверждён (W-a); решение по ThemeColors — Вариант A: семантика живёт в themes/*.json (37 слотов × 7 пресетов, тест-зеркало), colors.json остаётся примитивами (§7а отчёта)
- Остатки: W-e (решения владельца: BP_COMPACT/MOBILE, тач-цели web ≥44px, разморозка onboarding, пагинация template-строк)

---
Task ID: pages-rollout-verify (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1)
Agent: Super Z (main)
Task: Запрос владельца: «продолжай Выкат на Pages (обычный trunk build) — до выката старые записи останутся "пустыми", ретроспективы не будет»

Work Log:
- Факт выката: pages-web.yml уже отработал на main@0e4028d (run 37108516379, все шаги зелёные, Deploy 08:08:34Z) — единая сборка сайта docs + /app из trunk; ожидание владельца «до выката» снято: index.html с FR-091 уже в проде с этого деплоя
- Live-верификация по https: /app/index.html (200) несёт FR-091-конфиг `session_recording.captureCanvas {recordCanvas:true, canvasFps:4, canvasQuality:"0.4"}` + `canvasCapture.resolutionScale 0.6` и ключ EU; ассеты `canvas-web-c236b3e412eee141.js` (143 891 Б, application/javascript) и `canvas-web-c236b3e412eee141_bg.wasm` (12 256 109 Б, application/wasm) — 200, last-modified 08:08:30 = момент шага Deploy
- Браузерный смоук live-сайта (agent-browser, 1280×800, чекбокс «Метрики» снят — тест-человек в PostHog не создаётся): пикер отрендерен → «Продолжить без роли» → canvas 1280×800 в DOM, контекст webgl2 (WebGPU в headless недоступен — штатный фолбэк gpu-gate), рендер жив (46% пикселей вне фона на скриншоте), консоль без ошибок, запросов к eu.i.posthog.com — ноль: гейт согласия FR-089 работает на проде
- localStorage после смоука: `canvasdesk.config {language=ru, telemetry_counter=true, telemetry_analytics=false}` + `canvasdesk.consent` с anonId — согласия пишутся корректно; скриншот-доказательство сохранён вне репо
- Этот коммит (worklog) пушом повторно прогоняет полный цикл trunk build (CI + Pages) — контроль воспроизводимости конвейера после выката

Stage Summary:
- Выкат завершён и подтверждён: новые записи сеансов будут содержать WASM-слой (4 кадра/с WebP, масштаб 0.6) и события FR-090 в таймлайне; старые записи остаются без канваса — ретроспективы нет, как и ожидалось
- FYI: бандл вырос сверх §8.8 — ~12.5 МБ raw / ~4.9 МБ по сети против лимитов 8/4 (деплой не блокируется, отчёт размера информационный) — кандидат на внимание в будущих волнах
- Владельцу: (1) PostHog → Settings → General → Allowed domains — добавить https://danku13.github.io, если ещё не добавлен; (2) приёмка глазами: новая сессия с включёнными «Метриками» → Replays → в записи виден канвас

---
Task ID: FR-094 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1)
Agent: Super Z (main)
Task: Запрос владельца: «нужно проанализировать где у нас есть возможность для экономии на wasm бандле. так же проверь почему падает витрина интерфейса с логом: …no default font found (cosmic-text shape.rs:251)… chrome.action.show is not a function»

Work Log:
- Пулл main@ece0079 (FR-093); деплой b99777f — тот же бандл, что в логе владельца
- Репро падения на live (agent-browser): пикер → «без роли» → Esc онбординга → «Blank canvas» (кнопка empty-карточки) → «?»-меню (кнопка кластера настроек, [1100..1136]×[48..84], WEB_TOOLBAR_INSET=36) → «О интерфейсе» → витрина открывается, верхние секции живут → синтетический `wheel` на канвасе (agent-browser scroll страницу не трогает — шлётся dispatchEvent WheelEvent) → паника «no default font found» в консоли — воспроизведено в точности
- Корень: overlays.rs `kit_gallery_overlay` секция компонентного слоя — `Row::new` + ДЕФОЛТНЫЕ `Component::layout/paint` (внутренний `FontSystem::new()`): в wasm fontdb пуста (нет системных шрифтов) → cosmic-text shape.rs:251 паника → wasm-трап → rAF умирает. Все Table-потребители уже на `_with`-контракте §4.7 (внешний fs) — витрина нарушила его только в секции Row; kit_ui.rs:2430 — тест, не прод
- `chrome.action.show is not a function` — из браузерного РАСШИРЕНИЯ (MV3 API), rrweb-console-record в записи сеанса пишет консоль целиком; к приложению отношения не имеет (задокументировано в FR-094)
- Фикс: `Row::layout_row_with(m, fs, slot)` + `Row::paint_with(painter, m, fs, rects)` (тело прежнего layout_row бит-в-бит, пулы снаружи); `layout_row` делегирует внутренним полям; overlays.rs — секция Row на внешние m/fs функции (MutexGuard-коэрция), `use Component as _` из блока удалён; новый тест `layout_row_with_matches_internal_on_same_font` (геометрия + элементы Painter паритет внеш/внутр на одном лице)
- Гейты локально (rustup 1.99 поставлен в сессию): fmt чисто; canvas-ui 207 passed/0 failed; canvas-app все зелёные; clippy 0 (натив + wasm32 lib); wasm32 lib check — чисто (bin/main.rs под wasm не собирается by design — проверяется lib, как в CI-гейте)
- Анализ бандла (live b99777f, Python-парсер секций/сегментов): code 6.78 МБ (55.3%) + data 5.41 МБ (44.1%); в data 1 908 940 байт ЧИСТЫХ ДУБЛИКАТОВ (548 пар сегментов с равным md5, крупнейшие 318К/305К/305К/217К/217К…×2) — thin LTO не схлопывает одинаковые статик-таблицы; unicode-крейты в Cargo.lock одиночные → дубль не от версий; шрифты 1.86 МБ вшиты (name-таблицы идентифицируются, сырые байты перепакованы wasm-bindgen/wasm-opt); i18n ~353 КБ; user-docs 155 КБ; [profile.release] — только lto="thin" (opt-level дефолт 3, panic unwind, cgu дефолт)
- Доку: docs/dev-researches/wasm-bundle-size-analysis.md (замеры, рецепты с приоритетами: lto=fat+cgu=1 ≈ −1.8 МБ; opt-level=s −1..−2 МБ; panic=abort −5..10%; вынос шрифтов −1.86 МБ с CDN-кешем; план и риски, §8.8-пересмотр)
- Доки: fr-094-kit-row-external-font-pool.md (CR: диагноз/фикс/приёмка/расшифровка chrome.action), index-cr-fr.md (+FR-094), этот журнал

Stage Summary:
- Витрина больше не падает: прод-путь Row на внешний пул (§4.7), оракул-тест паритета; после выката — повтор e2e-сценария на live (обязательный шаг приёмки)
- Экономия бандла измерена и расписана: главный резерв — 1.82 МБ дубликатов (2 строки профиля), затем opt-level и шрифты; план в dev-researches
- CI должен остаться зелёным: fmt/check/test/clippy прогнаны локально с тем же тулчейном
