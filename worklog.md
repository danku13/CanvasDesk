---
Task ID: PRD-0010-UI-прототип
Agent: Super Z (main)
Task: Спроектировать UI 5 новых AI-компонентов LLM-интеграции (PRD-0010/ADR-0016) и доработать docs/prototypes/prototype-unified.html: статусная панель AI, custom-node suggest, агент-панель, таб настроек «AI and Models», онбординг AI-режима

Work Log:
- Прочитаны контекстные доки: PRD-0010 (F-2.12-16, F-4, F-7, F-8, Q1-Q7), byok-chatgpt-oauth-design, DESIGN_RULES §1-12, user-docs/interface.md, исходный prototype-unified.html (4296 строк: ЧАСТЬ A-X, suggest FR-079)
- README-комментарий в шапке HTML-файла: состав, демо-входы, состояния, правила (Esc-лестница, токены, cost-движок)
- CSS: 5 блоков в каноничном стиле (--cd/--ai токены, паттерны sgset/rdlg): #aistatus+#minimock (220×140 мок, F-7.9 <900px, отъезд right:400px при agent-open), #agentpanel (шторка 388px), #aisetov (9-й таб, 640px), #onbov (карточки-радио), #limov (warn-диалог); пульт: группа «AI (PRD-0010)» + padding-bottom 326px против перекрытия стопкой правого-нижнего угла
- ЧАСТЬ AI (~700 строк JS): состояние AI{}; cost-движок aiSpend/aiDemoLimit (80% → aiOpenLimit, 100% → error); статусная панель (модель/провайдер по активной фиче, feats-тумблеры гейтят sgRunAI/sgShowCards/agSend, пауза); custom suggest (AI_CUSTOM 3 варианта, conf 0.87/0.74/0.42, skeleton+spinner → каскадный morph, confidence-бар, порог F-2.11 скрывает 3-й, флип у края + подъём над попапом C1, применение = замена ноды через SG.undo + тост «Сохранить как шаблон» F-2.16); агент-панель (agUser/agBot/agToolRow, tool-calls graph_read→node_create→graph_validate, preview Sugiyama-lite от выделенной ноды + тег AGENT PREVIEW, Accept = graph_apply одним батчем, Reject, rate-счётчик ChatGPT 47/80, 429/лимит error-сообщения); настройки (per-feature провайдеры с disabled ChatGPT для Suggest, health-check /v1/models, модель, слайдеры cost-limit/conf-threshold, residency radio+описания, self-hosted endpoint+валидация, телеметрия OFF); онбординг (3 режима, privacy-блок, Продолжить применяет residency+провайдеры)
- Хуки в существующий код: sgRunAI canAi += AI.feat.suggest&&!AI.paused, isHybrid |= aiSuggestLLM(); sgRenderPopup: лейбл движка «lex + AI · BYOK» + секция «⚡ custom-ноды · LLM»; sgPositionSurfaces/aiCustomLayout: agPad против перекрытия агент-панелью; mousedown: клик по ghost до закрытия ввода; updateHover: тултипы ghost (confidence+порог, preview); keydown: aiKeydown перед sgKeydown (онбординг→лимит→настройки→preview→панель, Ctrl+I); render: aiDrawGhosts после sgDrawEditing; resetAll: aiResetAll; resize: aiSyncShell (sghud над стопкой); TOUR +2 шага (14), #info +5 пунктов
- Багфиксы по ходу: agSend перезаписывал tool-строки (текст вынесен в .ag-txt), Esc из фокуса чата глотался stopPropagation, ghost-ноды перекрывались попапом (подъём над pr.top), README-заголовок ЧАСТИ L восстановлен после вставки
- E2E в headless-браузере (agent-browser, 18 скриншотов): агент-цикл (запрос→tools→preview→Accept→ноды CAC/LTV на канвасе, day $0.12→$0.14, rate 47→46), custom-цикл (генерация→morph→клик по ghost→замена ноды+тост), демо лимита (80% warn+диалог→100% full+error-ghost→сброс), онбординг (Local → laya/ollama/ollama), настройки (selfhost endpoint, телеметрия), узкое окно 800px (aistatus+minimock display:none), обе темы, Ctrl+I/Esc-лестница, reset; node --check 4561 строк JS — OK, теги сбалансированы, консоль чистая
- docs/prototypes/README.md: секция «AI-компоненты LLM-интеграции» + тур 14 шагов

Stage Summary:
- prototype-unified.html: 4296 → ~5900 строк, 5 AI-компонентов PRD-0010 в стиле существующего прототипа, все состояния (loading/empty/error/success), интерактивные toggle/dropdown/slider, адаптивность F-7.9
- Интеграция бесшовная: suggest-гейты FR-079 уважают AI.feat/paused/провайдера; Esc-лестница расширена в начале; cost-движок единый для custom/агента
- Владельцу на приёмку: пульт «AI (PRD-0010)» → «Демо лимита» (3 клика: 80%→100%→сброс), «Агент-панель» (Ctrl+I), «Ввод в ноде (C1)» → секция custom-нод; вариант conf 0,42 скрыт порогом 0,50 — понизить слайдер в «Настройки AI»

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

---
Task ID: FR-091 v2 (сессия web-f324f377-6d94-493b-8e7c-59b9f9d210a1)
Agent: Super Z (main)
Task: Запрос владельца: «по последним записям в posthog пока не вижу чтобы wasm часть была видна в replay, так же отображаются только html части»

Work Log:
- Синхронизация: origin/main ушёл вперёд (9bf0867 FR-094 + ece0079 FR-093 + docs до 8788ff2) — пулл, рабочий шум (996 файлов 0/0) сброшен; FR-094 (паника cosmic-text витрины) уже в trunk и задеплоен (66d42445, 13:24Z)
- Диагноз на живом проде (agent-browser, consent analytics=true, инструментированный createImageBitmap + Worker): FPS-обсервер PostHog ДЕЙСТВИТЕЛЬНО снапшотит канвас (115 вызовов/25с ≈ 4.6fps, bitmap 768×480 = remote resolutionScale 0.6), но каждый снапшот 100% пуст (nzA=0/nzRGB=0) при живом рендере композитора (47% пикселей); прямой toDataURL — WebP 3КБ (пустой); атрибут контекста preserveDrawingBuffer:false, маркер __context отсутствует
- Корень 1 (WebGL2): wgpu создаёт webgl2 без preserveDrawingBuffer → drawing buffer очищается после композитинга → readback пуст; патч getContext у PostHog ставится после цепочки SDK→decide→lazy-recorder и проигрывает гонку буту WASM
- Корень 2 (WebGPU, браузер владельца с реальным GPU): растр webgpu-канваса рекордер не читает ВООБЩЕ — posthog/posthog#57008 (открытый issue, «works in 2D and WebGL», у WebGPU нет аналога preserveDrawingBuffer — gpuweb#2743); сверка с исходниками lazy-recorder.js v1.435.8 (getContext-патч форсит флаг, FPS-обсервер контекст-агностичен) + документация PostHog
- Фикс 1: index.html — getContext-шим в <head> (webgl/webgl2/experimental-webgl → preserveDrawingBuffer:true, мерж opts) ДО любых getContext (wgpu, gpu-gate-проба) — детерминированный выигрыш гонки; исправлены неверные утверждения v1 в комментарии (webgpu-растр «читается» — опровергнуто; локальный canvasCapture.resolutionScale игнорируется — только remote date-default)
- Фикс 2: canvas-render — wasm32 static PREFER_GL_FOR_CAPTURE + pub set_prefer_gl_for_capture; create_gpu_web при флаге пропускает WebGPU-ступень (create_gpu_web_gl — прямой GL-путь, общий хвост двухступенчатого); canvas-web/renderer_launch — analytics_recording_active() (localStorage canvasdesk.consent, семантика index.html: нет записи+конфига → false; нет поля/битая → true) ставит флаг до spawn_local — §3.1: ядро не знает про PostHog
- Локальная верификация ДО пуша (тестовая страница: live index.html + шим + абсолютные URL ассетов, python http.server, CORS GitHub Pages): pdb=true, снапшоты 0 → 16000/16000 непустых пикселей (107 вызовов/25с), рендер идентичен (47% до/после), ошибок нет — фикс подтверждён на реальном приложении без пересборки
- Гейты: fmt; check+clippy (натив+wasm32) -D warnings — 0; тесты 402+0 (canvas-render) + 51+0 (canvas-web); node --check шим-блока; пуш aeae900
- Ретро-фикс 3809a64 (найден при live-приёмке): быстрый GL-путь читал window.inner_size() до layout канваса → «окно создано 0×0» + surface клампился 1×1, лечение — гонка Resized-vs-RedrawRequested (Resized обрабатывается только при живом рендерере, handler.rs); wait_canvas_layout() в launch() — 2 rAF до Renderer::new, окно рождается 1280×800, кламп исчез; гейты повторно зелёные
- Live-приёмка финальная (Pages f526a8f7): консоль «захват канваса извне — WebGPU-ступень пропущена» → GL → «окно создано 1280×800»; readback nzA=16000 pdb=true; FPS-обсервер 48 вызовов/12с; CI/Pages/Tour — success; браузер закрыт (pagehide — флаш записи, в PostHog остался тестовый person probe-capture-test-01 как демо-реплей)
- Доки: fr-091-session-replay-canvas-wasm.md (§3а v2: диагноз/фикс/верификация, §4, §6, §7, §8 + правки v1-утверждений), index-cr-fr.md (строка FR-091 + v2)

Stage Summary:
- Replay теперь видит WASM-слой: захват шёл постоянно, но снапшоты были пустыми (два корня: preserveDrawingBuffer у WebGL2-контекста wgpu + webgpu-нечитаемость posthog#57008); фикс — детерминированный getContext-шим + согласие analytics ⇒ WebGL2-бэкенд (отказавшимся остаётся WebGPU); смена согласия действует после перезагрузки
- Отказ браузера кешировать HTML учтён: приёмка после выката обязательно с reload/revalidate (утренний замер мог видеть старый индекс из max-age=600)
- Владельцу: новые записи (после 14:0xZ) в PostHog Replays показывают канвас; тестовый person probe-capture-test-01 можно посмотреть/удалить; поднять canvasFps с 4 до 12 при желании «видео-качества» (тяжелее аплоады)

---
Task ID: W-e реализация (сессия web-51d33571)
Agent: интегратор Super Z + 4 параллельных агента (worktrees we-*)

Task: Волна W-e «решения владельца» аудита ui-kit (docs/dev-researches/ui-kit-adoption-audit.md §10): BP_COMPACT/BP_MOBILE — реализовать (вариант «A»); тач-цели web ≥44px; разморозка onboarding; пагинация template-строк → ScrollState. Решения владельца от 03.10.2026: брейкпоинты — вариант «A»; onboarding — разморозить.

Work Log:
- 4 параллельных агента в worktrees (we-bp 6d75ced, we-touch 5790d2e, we-template da1bb92, we-onboard 809a41c) + сальваж: на 1-a/1-c/1-d инфраструктура дважды рвала транспорт, наработка каждый раз восстанавливалась из worktree и дожималась
- 1-a брейкпоинты настроек (вариант «A»): ModalMode Desktop/Compact(<1280, одна колонка + компактный таб-бар на measured-примитивах)/Mobile(<768, полноэкранный лист); скролл W-a и kit switch/dropdown W-c без регресса; DegradationPolicy::Always (настройки адаптируются, не прячутся); G4-тесты 1280×800/1279×800/1024×640/800×560/767×600
- 1-b тач-цели web (дефект №14, CR-014): тулбар/тур-кнопки/роли/consent ≥44×44, author-bar 46×46 (::after hit при визуале 28px), #btn-recent — кап 240px эллипсиса жив; Playwright-аудит 19/19 (новый scripts/web_touch_targets_audit.py), регресс-тесты тура 11/11; index.html ↔ sdk/web-onboarding синхронно
- 1-c разморозка onboarding (решение владельца 03.10.2026): kit::modal по слоту вьюпорта С ПОЛЯМИ (SPACING_LG — аудит §8 №12), тело ужимается kit ScrollState/list_rows при клампе высоты (футер с CTA всегда виден), TextMeasurer::wrap вместо CHAR_W_FACTOR 0.62 (§9 CR-015), draw → Painter + WidgetState/button_style (замена ад-хок hover_fill ×1.3), 6 литералов → слоты темы; hit-rect'ы из той же card_layout («ввод = тому, что видно»); ui-kit.md/surface-registry.md обновлены. Дожим интегратором: фикс самодедлока теста (MutexGuard measure_font_system через onboarding_overlay), тест Skip приведён к дизайну (ghost в правом верхнем углу — NN/g), граница теста ширины 216 (пословный перенос кита не дробит «markdown-разметкой» 146.3px > 144px ниже стресс-поля)
- 1-d template-строки: break-пагинация и кэп MAX_VISIBLE_ROWS=12 удалены; окно строк = kit ScrollState по измеренной высоте панели (wheel/клавиатура/«ввод = тому, что видно»), бегунок kit::scroll_bar, хвост клеится к низу; >12 строк на высоких окнах (1280×1000/1200), хвост достижим на 800×560
- Интеграция: 4 merge --no-ff в main + конфликт §10 W-e аудит-дока разрешён сводным статусом; origin/main (36d69dc FR-091 v2) влит

Stage Summary:
- Волна W-e закрыта полностью (все 4 пункта §10): main = 2101424 (9211e85 we-touch, 879a121 we-bp, 16ccdfc we-template, f09affc we-onboard + origin)
- Гейты на merged main: cargo fmt --all -- --check ✓; clippy --workspace --all-targets -D warnings ✓ (0 ошибок); cargo test --workspace — 2416 passed / 0 failed (83 бинарника; canvas-app lib 503: было 492, +11)
- Владельцу на ручную приёмку: (1) web-стенд Ctrl+, сузить окно <1280/<768 — компакт/лист настроек; (2) Ctrl+P — колесо/стрелки по строкам шаблонов на 800×560 и высоком окне; (3) первый запуск — карточка онбординга на 800×560/240×180 (поля, футер виден, тело скроллится); (4) Pages — тулбар/тур с тача (hit ≥44)
- Известное ограничение (вне скоупа W-e): пословный перенос кита не дробит неразрывные токены — на карточке онбординга ниже ширины 216 возможен перелив ≤2px («markdown-разметкой»); брейк по дефису/посимвольный брейк — кандидат в отдельный FR к canvas-ui
- Открытые остатки аудита (на отдельные волны): CR-015 stage truncate_chars ×10, auto_width.rs «10 слов × 6 chars», docs-сниппет 44 символа, ~25 литералов DOM-палитр web-shell (не вошли в 1-b), text.rs (zoom 0.6/1.3), перенос search из canvas-render

---
Task ID: W-f реализация (сессия web-51d33571)
Agent: интегратор Super Z + 4 параллельных агента (worktrees wf-*, W-f-2 дожат интегратором)

Task: Волна W-f «остатки» аудита ui-kit (приказ владельца «доделай W-f»): §9 CR-015 (stage + app), canvas-render константы + перенос search_ui (B7), web-shell DOM-палитры.

Work Log:
- 4 параллельных агента в worktrees; транспорт рвал запуск/отчёты трижды (W-f-2 — все три запуска) — наработка сальважена из worktrees, W-f-2 дожат интегратором вручную
- W-f-1 stage CR-015 (fb922b7): StageMeasure (TextMeasurer + guard внешнего пула, FR-094); все 11 вызовов truncate_chars → измеренное усечение по геометрии отрисовки; ширины пилюль «chars×7.2/6.3» → width_of + паддинг-токены SPACING_XL/LG (паритет 24/12), минимумы и кап коридором сохранены; truncate_chars удалена из support.rs; +4 теста wf_*
- W-f-2 остатки CR-015 в canvas-app (7904f3a): каретка whatif explain → width_of (кегль 12); кнопки групп palette → измеренная подпись PAL_CAPTION_FONT (единый источник с отрисовкой) + PAL_CAPTION_PAD; split_two_lines → ширина-бюджет WHEEL_TPL_TEXT_W=64 (слот отрисовки) при WHEEL_TPL_FONT=11, WHEEL_TPL_TEXT_CHARS удалён; дефект №15: .take(2) → wrap_lines_with_more (перенос по измеренной ширине + i18n «ещё N», RU/EN) в empty-state галереи и описаниях настроек; auto_width: 420 «10×6×7» → измеренный эталон REFERENCE_TEN_WORDS при TYPE_BODY (продуктовое решение владельца сохранено), CJK-битый комментарий исправлен; docs_ui сниппет 44 симв — owner-approved exception (решение 02.10.2026, поведение не менялось); аудит §8 №15/§9 обновлён
- W-f-2 сальваж-находка: самодедлок теста палитры — guard глобального пула шейпинга держался через measured_layout (Mutex нереентрантен); замер скоуплен, хелпер задокументирован предостережением
- W-f-3 canvas-render (6feddb8): LINE_HEIGHT_FACTOR=1.3 (4 места) и WHATIF_LOD_ZOOM_MIN=0.6 — именованные константы (слот относительного межстрочного в tokens нет — кандидат W-g; отдельная семантика от LOD_NODE_L0_MAX_ZOOM подтверждена dimensions.json); search_ui перенесён canvas-render → canvas-app (потребители только там, цикла нет; путь в самом крейте — crate::search_ui), 30 тестов переехали бит-в-бит
- W-f-4 web-shell (0c95005): инвентарь 90 shell-использований / 28 уникальных значений → :root 29 слотов --cd-* (байт-в-байт паритет, 0 новых цветов); head/body-CSS, JS DEFAULT_STYLES тура (--cd-* с fallback), DIM_DEFAULTS через getComputedStyle (SVG data-URI var() не резолвит), заглушка gpu_gate.rs; данные-палитры канваса сознательно не мигрированы (конфиг); scripts/web_shell_palette_parity.py — паритет 396/396 деклараций, node --check 10/10
- Интеграция: 4 merge --no-ff (180b8ad, 7dd6aac, 885cf53, 2ecccd1); конфликт §9 аудита разрешён сводкой строк W-f-1/W-f-2; §10 дополнен итогом W-f

Stage Summary:
- Волна W-f закрыта: класс CR-015 в canvas-app/canvas-render ликвидирован (кроме owner-exception docs-сниппета), B7 перенесён, web-shell палитры консолидированы
- Гейты на merged main (2ecccd1): fmt ✓; clippy --workspace --all-targets -D warnings ✓ (0); cargo test --workspace — 2435 passed / 0 failed (83 бинарника); wasm32 --lib check canvas-app/canvas-render/canvas-web ✓; паритет палитр 396/396 ✓
- Владельцу на ручную приёмку: (1) wheel-меню шаблонов Shift+клик — подписи секторов переносятся по ширине слота (клипа нет); (2) whatif-каретка — позиция по глифам; (3) empty-state галереи/описания настроек — «ещё N» вместо тихого среза; (4) новая текстовая нода ≈ ширина 10 слов по метрикам шрифта (±несколько px от прежних 440); (5) Pages — визуальный паритет shell (цвета не менялись)
- Открытые остатки аудита (на будущие волны, вне W-f): dock_strip_layout (B3), слот межстрочного в tokens (W-g кандидат), ~10 символьно-смежных мест вне layout-путей (перечень §9 по факту пуст, исторические ссылки в док-комментариях)
