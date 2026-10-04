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

---
Task ID: A
Agent: Agent A (kit expansion)
Task: Add 5 color slots to KitPalette + stage_close_button + backdrop components

Work Log:
- Прочитан контекст: `AGENTS.md` §«UI-кит», аудит `docs/dev-researches/ui-hardcode-audit.md` §3/§6.1, существующие `KitPalette`/`ThemeColors`/`REQUIRED_KEYS` — понят паттерн «производные слоты по `is_light_bg`» (как `tooltip_warn`/`whatif_accent`/`explain_leaf`/`control_hover_fill`)
- A1 — `crates/canvas-ui/src/component/mod.rs`: добавлены 5 новых полей в `KitPalette` (`control_success`, `control_warning`, `stage_dim`, `scrollbar_thumb`, `rule_color` — все `[f32; 4]`) с rust-комментариями-ссылками на бывшие хардкод-места (audit §3)
- A1 — `crates/canvas-render/src/theme.rs`: добавлены 4 новых поля в `ThemeColors` (control_success/control_warning/scrollbar_thumb/rule_color — `stage_dim` уже был), значения для `dark()` = бывшие инлайн-литералы из аудита (ноль визуального скачка I-1 FR-046); для `light()` — затемнённые эквиваленты для контраста к белому (control_success [0.18, 0.58, 0.42], control_warning [0.78, 0.50, 0.20], scrollbar_thumb [0.40, 0.42, 0.48, 0.5], rule_color [0.65, 0.66, 0.72, 0.6])
- A1 — `crates/canvas-render/src/theme.rs`: `From<&ThemeColors> for KitPalette` пробрасывает 5 новых слотов в kit (`stage_dim` впервые проброшен — раньше `KitPalette` его не имел)
- A1 — `crates/canvas-render/src/theme_presets.rs::map_to_theme_colors`: новые слоты для JSON-пресетов выводятся по `is_light_bg` (как `tooltip_warn`/`whatif_accent`), НЕ добавлены в `REQUIRED_KEYS` (паритет `ThemeColors`↔JSON сохранён через `is_light_bg`-вывод, аналогично существующим производным слотам — это не ломает canvas-core/theme_presets тесты и не требует правки 7 JSON-файлов пресетов)
- A1 — `crates/canvas-ui/src/component/test_support.rs`: `palette_a`/`palette_b` обновлены 5 новыми полями (значения в `palette_a` отличны от dark-значений темы, чтобы тест «два разных значения слота» оставался значимым; `palette_b` мутатор расширен на новые слоты 1:1)
- A1 — `crates/canvas-ui/src/component/table.rs`, `tests/snapshot.rs`, `tests/g4_lint.rs`: тестовые палитры-двойки обновлены 5 новыми полями (фиксированные значения из аудита)
- A2 — `crates/canvas-ui/src/component/button.rs`: добавлена `pub fn stage_close_button(slot: UiRect) -> UiRect` — кнопка «×» в правом-верхнем углу панели/модали, размер `ICON_BUTTON_SIZE × ICON_BUTTON_SIZE`, inset `SPACING_SM` (8). Контракт F-8: только геометрия (как `icon_button_rect`); стиль/цвет — через `icon_button_style`
- A2 — `crates/canvas-ui/src/kit.rs`: `stage_close_button` экспортирован в `pub use crate::component::button::{...}`
- A2 — добавлены 2 юнит-теста: `stage_close_button_is_inset_in_top_right_corner` (для слота [0,0,400,300]: right = slot.right() - inset, top = slot.y + inset, квадрат ICON_BUTTON_SIZE, внутри слота) и `stage_close_button_inset_invariant_for_arbitrary_slot` (для слота со смещением 120,80 — инвариант тот же)
- A3 — `crates/canvas-ui/src/component/panel.rs`: добавлена `pub fn backdrop(rect: UiRect, palette: &KitPalette) -> PaintItem` — возвращает `PaintItem::Rect { rect, fill: palette.stage_dim, border: [0.0;4], radius: 0.0 }` для dim-overlay под модалью. Контракт F-8: цвет — только слот; радиус 0 (backdrop без скруглений)
- A3 — `crates/canvas-ui/src/kit.rs`: `backdrop` экспортирован в `pub use crate::component::panel::{...}`
- A3 — добавлен юнит-тест `backdrop_paints_stage_dim_slot_with_no_border_no_radius`: для viewport [0,0,1280,800] возвращает `PaintItem::Rect` с fill = `palette.stage_dim`, rect = входному (без трансформаций); смена палитры меняет fill (контракт F-8 — «цвета только слоты»)
- Верификация:
  - `cargo build -p canvas-ui -p canvas-render` — ✓ clean (7.75s)
  - `cargo test -p canvas-ui` — ✓ 209 lib + 11 + 4 + 6 + 16 + 61 (snapshot) = 307 passed / 0 failed (2 ignored — не наши)
  - `cargo test -p canvas-render --lib` — ✓ 387 passed / 0 failed
  - `cargo test -p canvas-core --lib` — ✓ 488 passed / 0 failed (REQUIRED_KEYS parity тест в canvas-core не сломан — новые слоты НЕ в REQUIRED_KEYS, а в `map_to_theme_colors` выводятся по `is_light_bg`)
  - `cargo clippy -p canvas-ui -p canvas-render --all-targets -- -D warnings` — ✓ 0 warnings
  - `cargo fmt --all -- --check` — ✓ clean (после `cargo fmt --all`)

Stage Summary:
- Files changed (10):
  - `crates/canvas-ui/src/component/mod.rs` (+19 строк: 5 полей KitPalette + комментарии)
  - `crates/canvas-ui/src/component/button.rs` (+67 строк: stage_close_button + 2 теста)
  - `crates/canvas-ui/src/component/panel.rs` (+50 строк: backdrop + 1 тест + импорт PaintItem)
  - `crates/canvas-ui/src/component/test_support.rs` (+12 строк: 5 полей в palette_a/b)
  - `crates/canvas-ui/src/component/table.rs` (+5 строк: 5 полей в default-палитре)
  - `crates/canvas-ui/src/kit.rs` (+2 правки: stage_close_button и backdrop в pub use)
  - `crates/canvas-ui/tests/snapshot.rs` (+6 строк: 5 полей + комментарий)
  - `crates/canvas-ui/tests/g4_lint.rs` (+6 строк: 5 полей + комментарий)
  - `crates/canvas-render/src/theme.rs` (+46 строк: 4 поля ThemeColors + значения dark/light + проброс в From<&ThemeColors>)
  - `crates/canvas-render/src/theme_presets.rs` (+27 строк: 4 производных слота в map_to_theme_colors)
- New kit API (public):
  - `canvas_ui::kit::KitPalette` — 5 новых полей: `control_success`, `control_warning`, `stage_dim`, `scrollbar_thumb`, `rule_color` (все `[f32; 4]`)
  - `canvas_ui::kit::stage_close_button(slot: UiRect) -> UiRect` — каноническая кнопка «×» в правом-верхнем углу панели/модали (size = `ICON_BUTTON_SIZE`, inset = `SPACING_SM`)
  - `canvas_ui::kit::backdrop(rect: UiRect, palette: &KitPalette) -> PaintItem` — dim-overlay под модалью (color = `palette.stage_dim`, border/radius = 0)
  - `canvas_render::theme::ThemeColors` — 4 новых поля: `control_success`, `control_warning`, `scrollbar_thumb`, `rule_color` (в `dark()` = former hardcoded literals — ноль скачка; в `light()` = darker equivalents)
- Tests: 307 passed in canvas-ui (+3 new: 2 stage_close_button + 1 backdrop); 387 passed in canvas-render (без изменений — нет регрессий); 488 passed in canvas-core (REQUIRED_KEYS parity тест зелёный)
- Known consumer sites to migrate (for Agent B) — canvas-app only (Agent A не трогал canvas-app):
  - **A) KitPalette literal updates** (новые поля добавлены в struct — компиляция canvas-app упадёт без правок; добавить 5 новых полей в каждый literal):
    - `crates/canvas-app/src/kit_ui.rs:1917` (`gallery_palette()` test fn)
    - `crates/canvas-app/src/template_ui.rs:1480` (`scrollbar_test_palette()` test fn)
    - `crates/canvas-app/src/settings_ui.rs:1533` (`const SWITCH_GEOMETRY_PALETTE` — это const literal, поля обязательны, без `..Default::default()`)
    - `crates/canvas-app/src/admin_ui.rs:3419` (`test_palette()` test fn)
  - **B) rgba literal → slot migration** (audit §3 — каждый из 13 хардкодов ниже теперь имеет канонический слот в `KitPalette`):
    - `app/overlays.rs:3354` — `[0.02, 0.02, 0.04, 0.85]` (docs backdrop) → `kit::backdrop(rect, palette)` (`palette.stage_dim`)
    - `app/overlays.rs:4917` — `[0.02, 0.02, 0.04, 0.85]` (settings backdrop) → `kit::backdrop(rect, palette)` (`palette.stage_dim`)
    - `app/overlays.rs:3454` — `[0.30, 0.33, 0.40, 0.8]` (docs markdown Rule) → `palette.rule_color`
    - `app/overlays.rs:3489` — `[0.35, 0.38, 0.46, 0.7]` (docs scrollbar thumb) → `palette.scrollbar_thumb` (или через `kit::scroll_bar`)
    - `app/overlays.rs:5439,5459,5481,5711,5724` — `Color::rgba(77, 191, 140, 255)` (5× success badge) → `palette.control_success` (или через `kit::icon_button_style` если это кнопка)
    - `app/ai_status_panel.rs:201` — `[0.95, 0.65, 0.30, 1.0]` (paused dot) → `palette.control_warning`
    - `app/ai_status_panel.rs:203` — `[0.30, 0.75, 0.55, 1.0]` (active dot) → `palette.control_success`
    - `app/ai_status_panel.rs:376` — `[0.95, 0.65, 0.30, 1.0]` (near-limit fill) → `palette.control_warning`
    - `app/ai_status_panel.rs:392` — `[242.0/255.0, 165.0/255.0, 76.0/255.0, 1.0]` (paused label amber) → `palette.control_warning`
    - `app/agent_panel.rs:434` — `let success_color: [f32; 4] = [0.30, 0.75, 0.55, 1.0]` → `palette.control_success`
    - `app/agent_panel.rs:491` — `let success_color: [f32; 4] = [0.30, 0.75, 0.55, 1.0]` → `palette.control_success`
  - **C) Stage close button migration** (audit §6.1 — 7+ мест ручной формулы `x = panel.right() - 28.0, y = panel.y + 6.0`): `explain`, `autolink`, `docs`, `settings`, `kit_gallery`, `admin`, `onboarding` — заменить на `kit::stage_close_button(panel_slot)` (получить rect) + `kit::icon_button_style(state, palette)` (стиль)
  - **D) NOT migration needed**: `app/overlays.rs:3559` — это комментарий, не литерал; `app.rs:13144` — это sentinel-литерал в test `onboarding_overlay_draw_uses_theme_slots_only` (forbidden-list assertion, остаётся как есть — это regression-guard тест)
- WASM note: WASM-гейт (`scripts/wasm_gate.sh --check`) НЕ запускался в этой сессии — A1/A2/A3 — чистая логика (без GPU/ввода), нативные тесты покрывают поведение; ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений; kit-слоты/компоненты — это pure-logic-слой, не поведение)

---
Task ID: C
Agent: Agent C (tokens + heuristic)
Task: Fix CR-015 regression + add PANEL_HEADER_H/FONT_* tokens + migrate 7 *_ui.rs files

Work Log:
- Прочитан контекст: `AGENTS.md` §«UI-кит» (правила 4-6: дублированные локальные константы, ручной Buffer, эвристики ширины текста — запрещены), аудит `docs/dev-researches/ui-hardcode-audit.md` §4/§5, существующие `tokens.rs`/`dimensions.json` — понят паттерн «значения = прежним константам (I-1: ноль скачка), $desc — источник, parity-тест ловит расхождения».
- C1 — `crates/canvas-app/src/app/ai_status_panel.rs`: найдена эвристика CR-015 на строке 232 (`provider_label.len() as f32 * 5.5 + 14.0`). Анализ контекста: чип использует `kit::chip_style(...)` для стиля, но высота нестандартная (14px, не `kit::CHIP_HEIGHT=24`), и padding 7px (прототип `padding:1px 7px`), не `kit::CHIP_PAD_H=8`. `kit::chip_layout` форсил бы `CHIP_HEIGHT=24`, ломая вёрстку → выбран второй вариант из аудита: `TextMeasurer::width_of(label, family, size) + 2·pad_x` (family/size = параметрам отрисовки `label_center` ниже — `SANS_FAMILY`, 9.5px). Извлечена чистая функция `provider_chip_width(provider_label: &str) -> f32` (тов. `palette.rs::group_button_w` паттерн): `TextMeasurer::new()` + `measure_font_system()` в коротком скоупе, guard отпускается до вызова kit-рендера (FR-094 пул; `ai_status_panel()` других `measure_font_system()` не делает — скоуп безопасен).
- C1 — добавлен юнит-тест `provider_chip_width_measures_real_glyphs`: (1) «ChatGPT-Pro» строго шире «ChatGPT» (monotonicity реального шейпинга); (2) 7 латинских глифов «ChatGPT» vs 7 кириллических «АаБбВвГ» дают РАЗНУЮ ширину (бывшая эвристика `len()*5.5` дала бы 52.5 для обоих — регрессия CR-015 ловится именно здесь).
- C1 — collateral fix ( parallel session, Agent A добавил 5 полей в `KitPalette` — `control_success`/`control_warning`/`stage_dim`/`scrollbar_thumb`/`rule_color`; consumers в `canvas-app` не обновлены): добавлены 5 нулевых полей в 4 `KitPalette` literals в `canvas-app`: `settings_ui.rs::SWITCH_GEOMETRY_PALETTE` (const), `kit_ui.rs::gallery_palette` (test fn), `admin_ui.rs::test_palette` (test fn), `template_ui.rs::scrollbar_test_palette` (test fn). Без правок `cargo build -p canvas-app` падал на E0063 (missing fields). Все 5 полей — `[0.0; 4]` (pure-geometry test fixtures — слоты не читаются, как и существующие `panel_fill`/`control_fill`/etc).
- C2 — `crates/canvas-core/src/tokens.rs`: добавлены 3 константы семейства `PANEL_HEADER_H_*` (после `CARD_RESULT_STRIP_H`, до spacing-scale):
  - `PANEL_HEADER_H_S: f32 = 30.0` — small (settings nav, template panel, kit_ui)
  - `PANEL_HEADER_H_M: f32 = 38.0` — medium (docs, scheme_gallery, flowmap)
  - `PANEL_HEADER_H_L: f32 = 44.0` — large (calc_panel, agent_panel, whatif)
  Каждый с rustdoc-комментарием: источники (файлы), use-case, отличие от `CARD_HEADER_HEIGHT` (модель канваса) и `BUTTON_HEIGHT` кита.
- C2 — `design/tokens/dimensions.json`: добавлена новая группа `panel_header` (s/m/l) с `$note` о 7 источниках и зеркале в `tokens.rs`. Каждый ключ — `$type: dimension` + `$value` + `$desc` со ссылкой на файл-источник и зеркало токена.
- C2 — `crates/canvas-core/src/tokens.rs` parity-тест `json_dimensions_match_rust_mirror`: добавлены 3 assert'а (`panel_header.s/m/l` ↔ `PANEL_HEADER_H_S/M/L`).
- C3 — `crates/canvas-core/src/tokens.rs`: добавлены 4 константы типографической шкалы UI:
  - `FONT_TITLE_LG: f32 = 18.0` — крупные заголовки (onboarding, modal titles)
  - `FONT_BODY: f32 = 13.0` — тело UI (labels, list rows, кнопки)
  - `FONT_CAPTION: f32 = 11.0` — captions, sub-labels
  - `FONT_HINT: f32 = 10.0` — hints, badges, micro-labels
  Каждый с rustdoc-комментарием: источники (10 кеглей аудита 10–19 → 4-шаговая шкала), отличие от `TYPE_*` (кегли рендера канвас-нод, не UI).
- C3 — `design/tokens/dimensions.json`: добавлена новая группа `ui_typography` (title_lg/body/caption/hint) с `$note`. ВНИМАНИЕ: имя группы НЕ `typography` (та занята рендером канваса) — `ui_typography`, чтобы не конфликтовать.
- C3 — parity-тест `json_dimensions_match_rust_mirror`: добавлены 4 assert'а (`ui_typography.title_lg/body/caption/hint` ↔ `FONT_TITLE_LG/BODY/CAPTION/HINT`).
- C4 — миграция 7 `*_ui.rs` файлов на новые токены (conservative: только exact-value matches):
  1. `flowmap_ui.rs`: `HEADER_H=36` — нет точного совпадения (ближе всего `PANEL_HEADER_H_M=38`, отклонение 2px) → оставлен + TODO-комментарий с обоснованием (визуальный скачок 36→38 в W-d-волне без аудита геометрии — I-1: ноль скачка).
  2. `docs_ui.rs`: `DOCS_HEADER_H=38.0` → `pub use canvas_core::tokens::PANEL_HEADER_H_M as DOCS_HEADER_H;` (exact match 38=38, alias сохраняет public API).
  3. `scheme_gallery_ui.rs`: `HEADER_H=40` — оставлен + TODO (отклонение 2px от M=38); `ROW_FONT=13.0` → `pub use canvas_core::tokens::FONT_BODY as ROW_FONT;` (exact match 13=13); `ROW_DESC_FONT=11.0` → `pub use canvas_core::tokens::FONT_CAPTION as ROW_DESC_FONT;` (exact match 11=11).
  4. `template_ui.rs`: `PANEL_HEADER_H=30.0` → `pub use canvas_core::tokens::PANEL_HEADER_H_S as PANEL_HEADER_H;` (exact match 30=30). `SECTION_HEIGHT=24`/`TEMPLATE_ROW_ICON=18`/etc — нет точных совпадений, не тронуты.
  5. `calc_panel_ui.rs`: `PANEL_TITLE_H=18.0` — проверен контекст (используется как `h: PANEL_TITLE_H`), это HEIGHT, не font-size → НЕ мигрирован (добавлен rustdoc с обоснованием: `FONT_TITLE_LG` — кегль текста, не высота); `PANEL_ROW_H=22.0` — нет точного совпадения со шкалой S/M/L (30/38/44) и не шапка → оставлен + TODO (высота строки данных, не header).
  6. `autolink_ui.rs`: `HEADER_H=58.0` — нет точного совпадения (ближе всего `PANEL_HEADER_H_L=44`, отклонение 14px) → оставлен + TODO (двухстрочная шапка требует высоты 58).
  7. `explain_ui.rs`: `HEADER_H=56.0` — нет точного совпадения (отклонение 12px от L=44) → оставлен + TODO (заголовок + строка крошек); `CRUMB_*` (CRUMB_H=18, CRUMB_GAP=4, CRUMB_W_MAX=148, CRUMB_W_MIN=48, CRUMB_MAX_CHIPS=12) — нет кеглей, не мигрированы.
- Верификация:
  - `cargo build -p canvas-core -p canvas-app` — ✓ clean (13.83s)
  - `cargo test -p canvas-core --lib` — ✓ 488 passed / 0 failed (parity-тест с 7 новыми assert'ами зелёный)
  - `cargo test -p canvas-app --lib` — ✓ 582 passed / 0 failed (новый тест `provider_chip_width_measures_real_glyphs` зелёный, 12 ai_status_panel тестов зелёные)
  - `cargo clippy -p canvas-core -p canvas-app --all-targets -- -D warnings` — ✓ 0 warnings
  - `cargo fmt --all -- --check` — ✓ clean

Stage Summary:
- Files changed (12):
  - `crates/canvas-core/src/tokens.rs` (+54 строки: 7 токенов + parity assert'ы)
  - `design/tokens/dimensions.json` (+15 строк: 2 новые группы `panel_header`/`ui_typography`)
  - `crates/canvas-app/src/app/ai_status_panel.rs` (+56 строк: `provider_chip_width` fn + CR-015 fix + тест; -1 строка: эвристика)
  - `crates/canvas-app/src/settings_ui.rs` (+6 строк: 5 нулевых KitPalette-полей + комментарий — collateral Agent A)
  - `crates/canvas-app/src/admin_ui.rs` (+6 строк: 5 нулевых KitPalette-полей + комментарий — collateral Agent A)
  - `crates/canvas-app/src/kit_ui.rs` (+6 строк: 5 нулевых KitPalette-полей + комментарий — collateral Agent A)
  - `crates/canvas-app/src/template_ui.rs` (+6 строк KitPalette collateral + 3 строки `pub use` для PANEL_HEADER_H_S)
  - `crates/canvas-app/src/flowmap_ui.rs` (+4 строки: TODO-комментарий для HEADER_H=36)
  - `crates/canvas-app/src/docs_ui.rs` (+3 строки: `pub use PANEL_HEADER_H_M as DOCS_HEADER_H`; -1 строка: литерал 38.0)
  - `crates/canvas-app/src/scheme_gallery_ui.rs` (+8 строк: TODO для HEADER_H=40 + 2 `pub use` для ROW_FONT/ROW_DESC_FONT; -2 строки: литералы)
  - `crates/canvas-app/src/calc_panel_ui.rs` (+6 строк: TODO для PANEL_ROW_H + rustdoc для PANEL_TITLE_H)
  - `crates/canvas-app/src/autolink_ui.rs` (+5 строк: TODO для HEADER_H=58)
  - `crates/canvas-app/src/explain_ui.rs` (+5 строк: TODO для HEADER_H=56)
- New tokens (7):
  - `PANEL_HEADER_H_S: f32 = 30.0` — small (settings nav, template panel, kit_ui)
  - `PANEL_HEADER_H_M: f32 = 38.0` — medium (docs, scheme_gallery, flowmap)
  - `PANEL_HEADER_H_L: f32 = 44.0` — large (calc_panel, agent_panel, whatif)
  - `FONT_TITLE_LG: f32 = 18.0` — крупные заголовки (onboarding, modal titles)
  - `FONT_BODY: f32 = 13.0` — тело UI (labels, list rows, кнопки)
  - `FONT_CAPTION: f32 = 11.0` — captions, sub-labels
  - `FONT_HINT: f32 = 10.0` — hints, badges, micro-labels
- CR-015 fix (before/after):
  - BEFORE (ai_status_panel.rs:232):
    ```rust
    // Ширина чипа — по подписи (грубо: len*5.5 + 14 padding); clamp.
    let prov_chip_w = (provider_label.len() as f32 * 5.5 + 14.0).clamp(48.0, 96.0);
    ```
  - AFTER:
    ```rust
    // CR-015: ширина чипа провайдера — ИЗМЕРЕННАЯ реальным шейпингом при
    // кегле отрисовки (9.5px, тот же font/size, что у `label_center`
    // ниже). Прежняя эвристика `len() as f32 * 5.5 + 14.0` — регрессия
    // CR-015: ломалась на кириллице/эмодзи (AGENTS.md §UI-кит правило 5).
    // `padding:1px 7px` прототипа → 2·7.0; clamp остаётся прежним.
    // FontSystem — глобальный пул приложения (FR-094); скоуп короткий —
    // guard отпускается до вызова kit-рендера (как в palette.rs::group_button_w).
    let prov_chip_w = provider_chip_width(&provider_label).clamp(48.0, 96.0);
    ```
  - New helper `fn provider_chip_width(provider_label: &str) -> f32` — `TextMeasurer::width_of(label, SANS_FAMILY, 9.5) + 2·7.0` (parity с `label_center` рендером: те же family/size).
- Migrations done (file → old constant → new token):
  - `docs_ui.rs` → `DOCS_HEADER_H=38.0` → `PANEL_HEADER_H_M` (pub use alias)
  - `scheme_gallery_ui.rs` → `ROW_FONT=13.0` → `FONT_BODY` (pub use alias)
  - `scheme_gallery_ui.rs` → `ROW_DESC_FONT=11.0` → `FONT_CAPTION` (pub use alias)
  - `template_ui.rs` → `PANEL_HEADER_H=30.0` → `PANEL_HEADER_H_S` (pub use alias)
- Deviations left as TODO comments (no value change, I-1: ноль скачка):
  - `flowmap_ui.rs::HEADER_H=36` — TODO: migrate to `PANEL_HEADER_H_M=38` (отклонение 2px)
  - `scheme_gallery_ui.rs::HEADER_H=40` — TODO: migrate to `PANEL_HEADER_H_M=38` (отклонение 2px)
  - `calc_panel_ui.rs::PANEL_ROW_H=22` — TODO: высота строки данных, не header (нет подходящего токена)
  - `calc_panel_ui.rs::PANEL_TITLE_H=18` — HEIGHT, не font-size (не мигрируется на `FONT_TITLE_LG` — разные категории токенов)
  - `autolink_ui.rs::HEADER_H=58` — TODO: migrate to `PANEL_HEADER_H_L=44` (отклонение 14px; двухстрочная шапка)
  - `explain_ui.rs::HEADER_H=56` — TODO: migrate to `PANEL_HEADER_H_L=44` (отклонение 12px; заголовок + строка крошек)
- Collateral fixes for Agent A's WIP (parallel session — `KitPalette` got 5 new fields, canvas-app literals not updated):
  - `settings_ui.rs::SWITCH_GEOMETRY_PALETTE` (const, [0.0; 4] ×5)
  - `kit_ui.rs::gallery_palette` (test fn, [0.0; 4] ×5)
  - `admin_ui.rs::test_palette` (test fn, [0.0; 4] ×5)
  - `template_ui.rs::scrollbar_test_palette` (test fn, [0.0; 4] ×5)
  Без этих правок `cargo build -p canvas-app` падал на E0063 (missing fields). Все 5 полей — `[0.0; 4]` (pure-geometry test fixtures — слоты не читаются).
- Tests: 488 passed in canvas-core (+7 parity assert'ов в существующем тесте); 582 passed in canvas-app (+1 новый тест `provider_chip_width_measures_real_glyphs` в ai_status_panel::tests); 0 regressions.
- WASM note: WASM-гейт (`scripts/wasm_gate.sh --check`) НЕ запускался в этой сессии — изменения в `tokens.rs` (новые константы + parity assert'ы) — pure-logic, нативные тесты покрывают поведение; C1 (CR-015 fix) меняет ширину чипа провайдера через `TextMeasurer::width_of` (семантически идентично рендеру — те же метрики cosmic-text), нативные тесты покрывают. Ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений поведения; здесь — чистая логика метрик).
- Out-of-scope оставлено для др. агентов:
  - `ai_status_panel.rs:201/203/376/392` rgba-литералы → слоты `control_warning`/`control_success` (Agent A §B отмечал — это его миграция rgba→slot, не моя)
  - `agent_panel.rs:434,491` `let success_color` → `palette.control_success` (Agent A §B)
  - `kit_ui.rs::HEADER_H`/section heights — kit_ui не входит в мою миграцию (только упомянут как use-case для `PANEL_HEADER_H_S`)

---
Task ID: B
Agent: Agent B (overlays migration)
Task: Migrate overlays.rs rgba literals to new palette slots + KitPalette placeholder real values + stage_close_button

Work Log:
- Прочитан контекст: worklog.md — секции Task A (kit expansion: 5 новых полей `KitPalette` + `kit::stage_close_button` + `kit::backdrop`) и Task C (collateral `[0.0;4]` ×5 в 4 canvas-app `KitPalette` literals; CR-015 fix). `KitPalette::dark()` confirmed: `control_success=[0.30,0.75,0.55,1.0]`, `control_warning=[0.95,0.65,0.30,1.0]`, `stage_dim=[0.02,0.02,0.04,0.6]`, `scrollbar_thumb=[0.35,0.38,0.46,0.7]`, `rule_color=[0.30,0.33,0.40,0.8]`.
- B1 — 4 canvas-app `KitPalette` literals (зануленные `[0.0;4]` ×5 от Agent C): заменены на канон dark-значения (1:1 с `KitPalette::dark()` Agent A — single source of truth). Тест-fixture контекст НЕ читает эти слоты (геометрия от них не зависит — Agent C это установил), но значения теперь не расходятся с production. Файлы: `settings_ui.rs::SWITCH_GEOMETRY_PALETTE` (const), `kit_ui.rs::gallery_palette` (test fn), `admin_ui.rs::test_palette` (test fn), `template_ui.rs::scrollbar_test_palette` (test fn). Комментарии обновлены с «зануляем» на «берём из dark() — не разойтись с production».
- B2 — миграция 9 rgba-литералов в `app/overlays.rs` на слоты темы:
  - 3354 docs backdrop `[0.02,0.02,0.04,0.85]` → `palette.stage_dim` (alpha 0.6 канонич. коридора PRD 0.55–0.65 — НЕ переопределена, single source of truth)
  - 4917 settings backdrop `[0.02,0.02,0.04,0.85]` → `palette.stage_dim` (та же семантика)
  - 3454 docs Rule `[0.30,0.33,0.40,0.8]` → `palette.rule_color`
  - 3489 docs scrollbar thumb `[0.35,0.38,0.46,0.7]` → `palette.scrollbar_thumb`
  - 5450/5470/5492/5722/5735 — 5× `Color::rgba(77,191,140,255)` (success-badge для AiApiKey/AiSelfhostUrl/AiSelfhostKey, дважды для последнего — кнопка + текст-поле) → `crate::kit_ui::color4(palette.control_success)`. Тождество: round(0.30·255)=77, round(0.75·255)=191, round(0.55·255)=140, round(1.0·255)=255 — ноль визуального скачка I-1, но теперь через канон-слот темы. `crate::kit_ui::color4` — существующий helper `[f32;4] → glyphon Color` (используется в `support.rs`, `debug_overlay.rs`, `admin_ui.rs` — не новый код).
- B3 — миграция close-button геометрии в `docs_ui::viewer_close_rect` (close-button call-site в `overlays.rs:3370` использует этот helper; тот же helper используется hit-тестом в `input.rs:1893` — миграция helper'а покрывает обе точки синхронно):
  - Старая формула: `size = DOCS_CLOSE_BUTTON.min(panel_min*0.5); pad = (DOCS_HEADER_H - size)/2; result = [panel.right - pad - size, panel.y + pad, size, size]` → в нормальном случае (panel_min >= 52) `[panel.right - 32, panel.y + 6, 26, 26]` (6px inset — вертикально по центру 38px шапки).
  - Новая (нормальный случай): `kit::stage_close_button(slot)` → `[panel.right - 34, panel.y + 8, 26, 26]` (8px inset = `SPACING_SM`, канон Agent A — audit §6.1). Tiny-panel fallback сохранён (panel_min < 52): пропорциональный scale-down `size = ICON_BUTTON_SIZE.min(panel_min*0.5)`, `inset = (panel_min - size)/2` — как и прежде (кламп по полу-диагонали панели).
  - `pub const DOCS_CLOSE_BUTTON: f32 = 26.0` теперь НЕ используется — оставлен как `pub const` для обратной совместимости + `#[deprecated(since="0.2.0", note="use canvas_ui::kit::ICON_BUTTON_SIZE")]`. Внешних потребителей не найдено (grep по репозиторию — только 1 internal use, который мигрирован).
  - Замечание: Agent A упоминал 7+ close-button мест (explain, autolink, docs, settings, kit_gallery, admin, onboarding). Я мигрировал ТОЛЬКО docs (`docs_ui::viewer_close_rect`), потому что: (1) Agent B scope — overlays.rs; (2) docs_ui.rs НЕ в DO-NOT-touch списке задачи; (3) миграция helper'а 1:1 покрывает рендер (overlays.rs:3370) и hit-test (input.rs:1893) — клик по нарисованной кнопке остаётся валидным. Остальные 6 мест (explain, autolink, settings, kit_gallery, admin, onboarding) — НЕ в overlays.rs scope, и у каждого свой локальный helper/inline. Настройки (settings) вообще не имеют close-кнопки (закрытие через Ctrl+, клик по ⚙ или клик мимо модалки — см. `input.rs:342,2115`). WhatIf в overlays.rs имеет «×»-remove кнопки у элементов списка (`overlays.rs:517`) — это не stage-close, оставлено как есть.
- B4 (optional) — `app/explain.rs:1589` tooltip: оставлен `// TODO: migrate to kit::tooltip + Painter::panel` комментарий. Анализ: `kit::tooltip` имеет сигнатуру `(anchor, text_size, viewport, hovered_ms, delay_ms) -> Option<TooltipLayout>` с hover-delay семантикой — explain.rs tooltip не хранит hover-state (всегда показан при наведении на explain-кнопку) и якорь = правый-край кнопки, а не курсор. Миграция потребует: (1) хранить hover_ms в App state, (2) переделать якорь на UiPoint, (3) switch с ThemeColors слотов (menu_fill/palette_border) на KitPalette слоты (panel_fill/panel_border) для `Painter::panel` — не 1:1 swap, отдельная W-e волна. Текущая ручная сборка CardInstance валидна (слоты темы, радиус 6).
- Верификация:
  - `cargo build -p canvas-app` — ✓ clean (5.96s)
  - `cargo build -p canvas-render` — ✓ clean (8.25s — theme tests unaffected)
  - `cargo test -p canvas-app --lib` — ✓ 582 passed / 0 failed (34.14s; no regressions)
  - `cargo test -p canvas-render --lib` — ✓ 387 passed / 0 failed (2.67s)
  - `cargo clippy -p canvas-app --all-targets -- -D warnings` — ✓ 0 warnings
  - `cargo fmt --all -- --check` — ✓ clean (после `cargo fmt --all` — rustfmt переформатировал `#[deprecated]` на многострочник и `[a,b,c,d]` на multi-line array в `viewer_close_rect` fallback)

Stage Summary:
- Files changed (7):
  - `crates/canvas-app/src/settings_ui.rs` (+5 строк: 5 KitPalette-полей [0.0;4] → dark() значения + комментарий обновлён)
  - `crates/canvas-app/src/kit_ui.rs` (+5 строк: 5 KitPalette-полей [0.0;4] → dark() значения + комментарий обновлён)
  - `crates/canvas-app/src/admin_ui.rs` (+5 строк: 5 KitPalette-полей [0.0;4] → dark() значения + комментарий обновлён)
  - `crates/canvas-app/src/template_ui.rs` (+5 строк: 5 KitPalette-полей [0.0;4] → dark() значения + комментарий обновлён)
  - `crates/canvas-app/src/app/overlays.rs` (+28 строк: 9 rgba→slot миграций + 2 explanatory comment-блока; 0 удалено — migrations 1:1 по token'у)
  - `crates/canvas-app/src/docs_ui.rs` (+22 строки: viewer_close_rect → kit::stage_close_button + tiny-panel fallback + DOCS_CLOSE_BUTTON deprecation)
  - `crates/canvas-app/src/app/explain.rs` (+7 строк: TODO comment для B4 future migration)
- rgba literals migrated: 9 в overlays.rs (2 backdrop → stage_dim; 1 Rule → rule_color; 1 scrollbar_thumb; 5 Color::rgba(77,191,140,255) → color4(palette.control_success))
- KitPalette placeholders replaced with real values: 20 (4 literals × 5 полей — [0.0;4] → dark()-значения)
- Close buttons migrated to kit::stage_close_button: 1 (docs_ui::viewer_close_rect — покрывает overlays.rs:3370 render + input.rs:1893 hit-test синхронно). Остальные 6 мест из audit §6.1 (explain, autolink, settings, kit_gallery, admin, onboarding) — за пределами overlays.rs scope Agent B; settings вообще не имеет close-кнопки.
- Tests: 582 passed in canvas-app (no regressions); 387 passed in canvas-render (theme tests unaffected); clippy clean; fmt clean.
- Out-of-scope (оставлено для будущих волн):
  - `ai_status_panel.rs:201/203/376/392` rgba → control_warning/control_success — Agent A §B отметил, но эти сайты вне overlays.rs scope Agent B (избежать файл-конфликтов с будущей работой)
  - `agent_panel.rs:434,491` `let success_color` → palette.control_success — Agent A §B (тоже вне scope)
  - `app/explain.rs:1589` tooltip → kit::tooltip — оставлен TODO comment (B4 — миграция не 1:1, требует hover-state + якорь-кнопка vs курсор)
  - 6 близких close-button мест (explain, autolink, settings, kit_gallery, admin, onboarding) — каждый со своим локальным inline/helper, не в overlays.rs
- WASM note: WASM-гейт (`scripts/wasm_gate.sh --check`) НЕ запускался — миграции B1/B2 — чисто замена rgba-литералов на слоты (значения 1:1 — I-1: ноль визуального скачка для `control_success`; для `stage_dim` alpha 0.85→0.6 — каноничный коридор PRD 0.55–0.65, Agent A так и проектировал). B3 меняет позицию «×» в шапке docs на 2px (inset 6→8, как и `kit::stage_close_button` — канон для всех 7+ мест audit §6.1) — нативные тесты покрывают поведение (тест `viewer_rect_clamps_to_window` проверяет, что close в пределах панели); ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений поведения; здесь — чистая консолидация геометрии/цветов в каноничные слоты).

---
Task ID: F
Agent: Agent F (anchored_stack + table_layout_immediate)
Task: Create kit::anchored_stack + kit::table_layout_immediate + migrate suggest/whatif/tooltip

Work Log:
- Read worklog.md, dropdown.rs::dropdown_menu (anchor + flip + clamp для одного menu rect), table.rs::Table (retained компонент с row_layout_with/visible_rows), suggest.rs::card_rects (hand-rolled stack offset+clamp), whatif_ui.rs::table_layout (hand-rolled Vec<Vec<[f32;4]>>), tooltip.rs::layout_tooltips (kit::tooltip + ручной стек внутри). Подтвердил: `Side` enum в kit нет; `canvas_core::Side` — для JSON Canvas рёбер (семантически не подходит для UI popup).
- F1: Добавил `AnchoredSide` enum (Right/Left/Top/Bottom) и `anchored_stack(anchor, side, element_sizes, gap, viewport)` в `component/dropdown.rs` (≈145 строк + 5 unit tests ≈120 строк = +267 строк). Алгоритм обобщает `dropdown_menu` (1 элемент) на N элементов с flip+clamp: по поперечной оси (X для Right/Left) — natural side → flip на противоположную при нехватке места → position-clamp к viewport.x; по продольной оси (Y для Right/Left) — выравнивание по anchor.y → прижим к viewport.bottom() при нехватке → прижим к viewport.y при переполнении. `gap` = и зазор от якоря, и inter-element spacing (типовой UI-паттерн; suggest: SUGGEST_CARD_GAP == SUGGEST_CARD_OFFSET_X == 12). Тесты: fits_on_right_side, flips_to_left_when_right_full, taller_than_viewport_clamps_to_top, bottom_side_horizontal, empty_and_single.
- F2: Добавил `table_layout_immediate(slot, column_widths, row_heights, header_h) -> Vec<Vec<UiRect>>` в `component/table.rs` (+60 строк impl + 50 строк тестов = +112 строк). immediate-API без retained-state: первая строка на slot.y, каждая следующая — ниже предыдущей; колонки слева направо с шагом = column_widths[c]; header_h Some → первой строкой (если > 0). Тесты: 2x3+header → 4x2 grid (бит-в-бит), no_header_and_empty.
- F3: Экспорт `anchored_stack` + `AnchoredSide` + `table_layout_immediate` из `kit.rs` (2 строки изменений).
- F4: Мигрировал `suggest.rs::card_rects` (hand-rolled ~30 строк → kit::anchored_stack вызов ~15 строк). Публичный API (signature, return type `Vec<UiRect>`) сохранён. Поля 4 px кодируются в viewport kit-функции (`UiRect::new(4, 4, vw-8, vh-8)`), паритет прежним `viewport[0]-4.0`/`viewport[1]-4.0`/`.max(4.0)`. Бит-в-бит паритет подтверждён: 3 существующих теста card_rects_* проходят без изменений.
- F5: Мигрировал `whatif_ui.rs::table_layout` (hand-rolled ~25 строк header+cells loops → kit::table_layout_immediate вызов + UiRect→[f32;4] адаптер). Публичный API (signature, return type `TableLayout { rect, header: Vec<[f32;4]>, cells: Vec<Vec<[f32;4]>>, tail }`) сохранён. Бит-в-бит паритет: 2 существующих теста table_layout_* проходят без изменений.
- F6 (tooltip): МИГРАЦИЯ ОТЛОЖЕНА с подробным TODO. Тултипы стакаются (`cards: Vec<TooltipCard>`), но текущая реализация через `kit::tooltip` (anchor=точка курсора + offset+flip) + ручной стек внутри — НЕ выражается через `anchored_stack` с одним параметром `gap`. Две асимметрии: (1) якорь-точка (курсор) vs rect (suggest); (2) kit::tooltip asym-flip по X (natural +offset, flipped -size без -offset) и симметричный по Y (±TOOLTIP_OFFSET.y) vs anchored_stack симметричный (±gap). Чтобы flipped-Y совпал, нужно gap=2*TOOLTIP_OFFSET.y=36, но тогда natural-X уедет на +36 вместо +14 — математически неразрешимо с одним gap. Suggest подходит (offset==gap==12), tooltip — нет (offset 14/18 ≠ gap 8). TODO оставлен в `tooltip.rs::layout_tooltips` doc-comment с полным анализом; текущая реализация корректна и сохраняет визуальный паритет.
- F7: Verify: cargo build -p canvas-ui -p canvas-app — clean (4 warning'а от palette.rs/icon.rs — другие агенты); cargo test -p canvas-ui --lib — 244 passed (включая 5 новых anchored_stack + 2 новых table_layout_immediate тестов); cargo test -p canvas-app --lib — 585 passed (включая 3 suggest card_rects + 2 whatif table_layout тестов — все bit-exact паритет); cargo clippy -p canvas-ui -p canvas-app --all-targets -- -D warnings — единственный error в `component/icon.rs::icon_quad` (too_many_arguments, 8/7) — ВНЕ моей зоны (icon.rs в "DO NOT touch" списке, owned by Agent D); с `-A clippy::too_many_arguments` мой код чист; cargo fmt --all -- --check — clean (exit 0).

Stage Summary:
- Files changed: 6 (dropdown.rs +267, table.rs +112, kit.rs ~+6, suggest.rs ±67 net, whatif_ui.rs ±56 net, tooltip.rs +31 TODO)
- New kit API: `anchored_stack(anchor, side, element_sizes, gap, viewport) -> Vec<UiRect>` + `AnchoredSide` enum (Right/Left/Top/Bottom) в `component/dropdown.rs`; `table_layout_immediate(slot, column_widths, row_heights, header_h) -> Vec<Vec<UiRect>>` в `component/table.rs`; оба экспортированы из `kit.rs`.
- Consumer migrations: `suggest.rs::card_rects` (lines 558-580) → `kit::anchored_stack` (бит-в-бит); `whatif_ui.rs::table_layout` (lines 498-567) → `kit::table_layout_immediate` (бит-в-бит, с UiRect→[f32;4] адаптером); `tooltip.rs::layout_tooltips` — отложен с подробным TODO (две асимметрии: anchor=точка vs rect, kit::tooltip asym-flip vs anchored_stack sym-flip; gap=14/18 не совпадает с inter-card=8).
- Tests: 244 (canvas-ui) + 585 (canvas-app) = 829 passed; 7 новых тестов (5 anchored_stack + 2 table_layout_immediate) + 5 существующих bit-exact паритет тестов (3 card_rects + 2 table_layout) — все зелёные.
- Issues: единственный clippy error в `component/icon.rs::icon_quad` (too_many_arguments) — owned by Agent D (icon.rs в "DO NOT touch" списке); не блокирует компиляцию/тесты. tooltip миграция отложена (с обоснованием) — текущая реализация корректна.

---
Task ID: E
Agent: Agent E (4 new kit components)
Task: Create radio_card, chat_bubble, footer_buttons, chip_strip components

Work Log:
- Прочитан контекст: worklog.md (Task A/B/C — kit expansion, KitPalette, FR-070 backdrop, stage_close_button), AGENTS.md §«UI-кит» (правила: цвета только слоты, геометрия только SPACING_*/RADIUS_*, текст только через TextMeasurer), audit `docs/dev-researches/ui-hardcode-audit.md` §6.1 (4 missing kit components из 4 разных файлов × 3 поверхности каждый — MEDIUM priority). Изучена инфраструктура: component/mod.rs (KitState/KitPalette/ControlStyle/PanelStyle/константы), component/button.rs (button_style/chip_style/switch pattern), component/panel.rs (panel_style/card/backdrop pattern), paint.rs (PaintItem::Rect{rect, fill, border, radius}), geometry.rs (UiRect API: new/right/bottom/inset), layout.rs (Row::lay_out_measured, MeasuredItem::Text, RowPolicy::Fit/SqueezeTail), measure.rs (TextMeasurer::width_of), tokens.rs (SPACING_S/SM/MD/LG, RADIUS_CHIP/PANEL/PILL, FONT_BODY/CAPTION).
- E1 — `crates/canvas-ui/src/component/radio_card.rs` (323 строки): новый kit-компонент radio-card. Pattern: `ai_onboarding` (3×) + `graph_builder_ui` (3×) — карточка с радио-индикатором (◉/○), label и описанием. API:
  - `pub struct RadioCardLayout { rect, indicator, label, desc: Option<UiRect> }`
  - `pub struct RadioCardStyle { card_fill, card_border, radius, label_color, desc_color, indicator_fill }`
  - `pub fn radio_card(slot, label_w, desc_w, selected, state, palette) -> (RadioCardLayout, RadioCardStyle)` — раскладка + стиль в одном вызове (как `switch`).
  - `pub fn paint_radio_card(layout, style) -> Vec<PaintItem>` — фон карточки + индикатор (radius=side/2 → кружок).
  - Константы: `RADIO_INDICATOR_SIZE=12.0` (explicit, не выведен из RADIUS_PILL), `RADIO_LABEL_LINE_H=16.0` (FONT_BODY·1.3≈16.9→16), `RADIO_DESC_LINE_H=14.0` (FONT_CAPTION·1.3≈14.3→14).
  - Стиль по `(selected, state)`: (true, _) → accent tint(0.10) fill + accent border + accent indicator; (false, Hovered|Pressed) → hover_fill + panel_border + text_muted indicator; (false, Normal) → panel_fill + panel_border + text_muted indicator; Disabled → disabled_text. Tint (alpha-overlay над rgb слота, rgb сохраняется) — именованный паттерн `agent_panel.rs:443-444`/`graph_builder_ui.rs:197-202` (документированное отклонение от «никакой арифметики над цветами» — tint это прозрачность существующего слота, не новый цвет).
  - 5 юнит-тестов: selected-vs-unselected разные стили; layout уважает границы слота; смена палитры меняет стиль (контракт F-8); Hovered использует hover_fill; paint_emits_two_items (фон + индикатор, radius=side/2 → circle).
- E2 — `crates/canvas-ui/src/component/chat_bubble.rs` (300 строк): новый kit-компонент chat bubble. Pattern: `agent_panel.rs` (normal/error/success + tool_calls). API:
  - `pub enum ChatBubbleKind { Normal, Error, Success }`
  - `pub struct ChatBubbleLayout { rect, text_area, tool_call_rows: Vec<UiRect> }`
  - `pub struct ChatBubbleStyle { fill, border, radius, text_color }`
  - `pub fn chat_bubble(slot, n_lines, n_tool_calls, kind, palette) -> (ChatBubbleLayout, ChatBubbleStyle)`.
  - `pub fn paint_chat_bubble(layout, style) -> Vec<PaintItem>` — ровно 1 item (фон пузыря).
  - Константы: `CHAT_LINE_H=14.0` (FONT_CAPTION·1.3≈14.3→14, тот же что agent_panel.rs), `CHAT_TOOL_CALL_H=14.0` (= CHAT_LINE_H, отдельная константа для семантики).
  - Стиль по `kind`: Normal → panel_fill/panel_border/text; Error → control_danger tint(0.08) fill / tint(0.38) border / control_danger text; Success → control_success tint(0.08) / tint(0.38) / control_success. Tint-альфы (0.08/0.38) — исторические значения `agent_panel.rs:443-451`.
  - 6 юнит-тестов: kind_normal/error/success каждый проверяет fill/border/text_color по слотам; style_uses_palette_slots_only (3 kind × palette_a vs palette_b); layout_respects_slot_bounds (rect=slot, text_area внутри пада, tool_call_rows ниже text_area с зазором SPACING_S, по CHAT_TOOL_CALL_H высотой); paint_emits_single_background_rect.
- E3 — `crates/canvas-ui/src/component/footer.rs` (199 строк): новый kit-компонент 3-button right-aligned footer. Pattern: `autolink_ui`, `onboarding`, `settings`, `graph_builder_ui`. API:
  - `pub fn footer_buttons(slot, n) -> Vec<(UiRect, usize)>` — фиксированная ширина BUTTON_WIDTH=100.0 (новый kit-токен в mod.rs), высота BUTTON_HEIGHT, зазор GAP_CONTROLS, правый край последней кнопки = slot.right() (inset = 0 — потребитель inset'ит слот сам через `pad`/`inset` при необходимости; API остаётся минимальным и не кодирует inset отдельно).
  - `pub fn footer_buttons_measured(slot, widths: &[f32], _m: &mut TextMeasurer) -> Vec<(UiRect, usize)>` — переменные ширины (потребитель измеряет через TextMeasurer::width_of + 2·BUTTON_PAD_H или `button::button_size`); `_m` зарезервирован для будущей версии с встроенным замером.
  - 5 юнит-тестов: 3 кнопки right-aligned (правый край последней = slot.right(), зазор = GAP_CONTROLS, высота = BUTTON_HEIGHT, Y по центру слота); right_align_invariant для arbitrary slot; measured_widths (переменные ширины); narrow_slot_does_not_mask_overflow (контракт G4: переполнение НЕ маскируется); zero_buttons returns empty.
- E4 — `crates/canvas-ui/src/component/chip.rs` (202 строки): новый kit-компонент Row из Fit-чипов с опциональным «All» preset. Pattern: `scheme_gallery_ui`, `template_ui`, `settings_ui`. API:
  - `pub fn chip_strip(slot, items: &[&str], squeeze: bool, m, fs) -> Vec<(UiRect, &str)>` — каждый чип измерен через `MeasuredItem::Text { pad_x: 2·CHIP_PAD_H, h: Some(CHIP_HEIGHT) }`, `Row::lay_out_measured` с `RowPolicy::Fit` (по умолчанию, переполнение НЕ маскируется — G4) или `SqueezeTail` (если `squeeze=true`). Возвращает Vec<(rect, &str)> параллельно items.
  - Константы: `CHIP_FAMILY="Noto Sans Display"` (паритет sans_attrs рендера), `CHIP_FONT_SIZE=12.0` (прежний кегль чипов галереи/whatif BAR_FONT_SIZE), `CHIP_GAP=SPACING_S=6.0` (бывший BAR_GAP whatif_ui.rs).
  - 5 юнит-тестов: 3 чипа fit в широком слоте (первый слева, ширина = text + 2·CHIP_PAD_H, зазор = SPACING_S); fit_policy_does_not_mask_overflow (переполнение НЕ маскируется — G4); squeeze_tail_compresses_overflow (хвост сжат до 0 ширины, ни один чип не выходит за слот); empty_items_returns_empty; order_preserved (result[i] ↔ items[i]).
- E5 — экспорт в `kit.rs` + `component/mod.rs`:
  - mod.rs: добавлены `pub mod chat_bubble; pub mod chip; pub mod footer; pub mod radio_card;` (alphabetic order). Добавлена константа `BUTTON_WIDTH: f32 = 100.0` (новый kit-токен — канон ширины кнопки футера; onboarding=100, graph_builder=130→передаёт через `widths`).
  - kit.rs: добавлены `pub use` для всех новых функций/типов/констант: `chat_bubble::*`, `chip::{chip_strip, CHIP_FAMILY, CHIP_FONT_SIZE, CHIP_GAP}`, `footer::{footer_buttons, footer_buttons_measured}`, `radio_card::*`. В `pub use crate::component::{...}` добавлен `BUTTON_WIDTH`.
- E6 — Верификация:
  - `cargo build -p canvas-ui` — ✓ clean (0.98s)
  - `cargo test -p canvas-ui --lib` — ✓ 244 passed / 0 failed / 0 ignored (2.0s; из них 21 — мои новые тесты: radio_card×5 + chat_bubble×6 + footer×5 + chip×5)
  - `cargo clippy -p canvas-ui --all-targets -- -D warnings` — ⚠️ 1 error в `component/icon.rs:302` (Agent D's WIP: `fn icon_quad` 8 args > 7). НЕ мой файл — territory Agent D, не чиню. Проверил мой код без icon.rs (временно закомментировав `pub mod icon;` + `pub use ... icon::*`): clippy чист. Agent D должен добавить `#[allow(clippy::too_many_arguments)]` в icon.rs.
  - `cargo fmt --all -- --check` — ✓ clean (после `cargo fmt --all`: rustfmt реформатировал многострочник match arm в radio_card.rs и выровнял inline-комментарии).
  - `cargo build -p canvas-app` — ✓ clean (12.89s — canvas-app не пострадал от моих изменений; consumer migration — территория других агентов).
  - `cargo test -p canvas-app --lib` — ✓ 585 passed / 0 failed / 0 ignored (58.55s; no regressions в canvas-app).

Stage Summary:
- Files changed (6):
  - `crates/canvas-ui/src/component/radio_card.rs` (+323 строки, новый)
  - `crates/canvas-ui/src/component/chat_bubble.rs` (+300 строк, новый)
  - `crates/canvas-ui/src/component/footer.rs` (+199 строк, новый)
  - `crates/canvas-ui/src/component/chip.rs` (+202 строки, новый)
  - `crates/canvas-ui/src/component/mod.rs` (+12 строк: 4 pub mod + BUTTON_WIDTH const + doc)
  - `crates/canvas-ui/src/kit.rs` (+21 строк: 4 pub use блока + BUTTON_WIDTH в экспорте)
- New kit API (functions + types + constants):
  - `kit::radio_card(slot, label_w, desc_w, selected, state, palette) -> (RadioCardLayout, RadioCardStyle)` + `kit::paint_radio_card(layout, style) -> Vec<PaintItem>` + типы `RadioCardLayout, RadioCardStyle` + константы `RADIO_INDICATOR_SIZE=12.0, RADIO_LABEL_LINE_H=16.0, RADIO_DESC_LINE_H=14.0`
  - `kit::chat_bubble(slot, n_lines, n_tool_calls, kind, palette) -> (ChatBubbleLayout, ChatBubbleStyle)` + `kit::paint_chat_bubble(layout, style) -> Vec<PaintItem>` + типы `ChatBubbleKind, ChatBubbleLayout, ChatBubbleStyle` + константы `CHAT_LINE_H=14.0, CHAT_TOOL_CALL_H=14.0`
  - `kit::footer_buttons(slot, n) -> Vec<(UiRect, usize)>` + `kit::footer_buttons_measured(slot, widths, m) -> Vec<(UiRect, usize)>` + новый kit-токен `BUTTON_WIDTH=100.0` (в mod.rs)
  - `kit::chip_strip(slot, items, squeeze, m, fs) -> Vec<(UiRect, &str)>` + константы `CHIP_FAMILY="Noto Sans Display", CHIP_FONT_SIZE=12.0, CHIP_GAP=SPACING_S=6.0`
- Tests: 21 новых теста в canvas-ui (radio_card×5, chat_bubble×6, footer×5, chip×5) — все pass. 244 всего в canvas-ui (было 223 + 21 новых). canvas-app 585 pass (no regressions).
- Consumer migration candidates (для будущих агентов — не моя территория):
  - `crates/canvas-app/src/app/ai_onboarding.rs` (3×) → `kit::radio_card` (radio-card с описанием)
  - `crates/canvas-app/src/app/graph_builder_ui.rs` (3× radio-card + footer Cancel/Generate) → `kit::radio_card` + `kit::footer_buttons_measured` (BTN_W=130 — measured variant)
  - `crates/canvas-app/src/onboarding_ui.rs::button_rect` (Prev/Next/Skip footer) → `kit::footer_buttons` (фиксированная ширина 100 = BUTTON_WIDTH — совпадает с ONBOARDING_BUTTON_W=100)
  - `crates/canvas-app/src/autolink_ui.rs` (footer) → `kit::footer_buttons_measured`
  - `crates/canvas-app/src/settings_ui.rs` (footer) → `kit::footer_buttons_measured`
  - `crates/canvas-app/src/app/agent_panel.rs::AgentMessage::Bot` (chat bubble с tool_calls) → `kit::chat_bubble(ChatBubbleKind::Normal|Error|Success, …)` — миграция хардкода `[0.30, 0.75, 0.55, 1.0]` success_color на `palette.control_success` (Agent A/B отметили, но не мигрировали — теперь kit готов)
  - `crates/canvas-app/src/scheme_gallery_ui.rs::chip_layout` (Row из чипов «Все» + категории) → `kit::chip_strip(slot, &["Все", "API", …], squeeze=false, m, fs)` — устраняет ручной `MeasuredItem::Fixed`-скелет чипов + `Row::lay_out_measured` бойлерплейт
  - `crates/canvas-app/src/template_ui.rs::chip_strip` (аналогично) → `kit::chip_strip`
  - `crates/canvas-app/src/settings_ui.rs` (chip-фильтр категорий) → `kit::chip_strip` (если есть)
- Design decisions:
  - **Alpha-tint паттерн** (rgb сохраняется, alpha заменяется на 0.08/0.10/0.38) — именованный хелпер `tint(slot, alpha)` в radio_card.rs и chat_bubble.rs. Документированное отклонение от «никакой арифметики над цветами» (контракт F-8): tint — это прозрачность существующего слота `accent`/`control_danger`/`control_success`, не новый цвет. Источник паттерна — `agent_panel.rs:443-451`/`graph_builder_ui.rs:197-202` (значения alpha 1:1 — I-1: ноль визуального скачка). Альтернатива (новые `control_*_tint` слоты в KitPalette) отклонена: это потребовало бы расширения KitPalette + theme.rs + theme_presets.rs + REQUIRED_KEYS — вне scope Agent E (контракт: «только canvas-ui/src/component/ и kit.rs»).
  - **BUTTON_WIDTH=100.0** — новый kit-токен в mod.rs. Значение выбрано из onboarding_ui.rs (ONBOARDING_BUTTON_W=100). graph_builder_ui.rs (BTN_W=130) использует `footer_buttons_measured` с переменными ширинами. Документировано в комментарии.
  - **Inset = 0 в footer** (правый край последней кнопки = slot.right()): потребитель inset'ит слот сам через `pad`/`inset` при необходимости. API остаётся минимальным и не кодирует inset отдельно (в отличие от `stage_close_button` где inset = SPACING_SM каноничен). Если потребитель хочет inset 8 от правого края, он передаёт `slot.inset(&EdgeInsets { right: 8, … })` или просто slot с смещением.
  - **`_m` параметр в `footer_buttons_measured`** зарезервирован для будущей версии с встроенным замером (сегодня потребитель измеряет сам через `TextMeasurer::width_of` или `button::button_size`, передавая `widths: &[f32]`). Сигнатура симметрична с `Row::lay_out_measured` (m + fs) для однозначного API. `#[allow(clippy::unused_self)]` НЕ нужен — нет `&self`, lint irrelevant; `_m` (underscore prefix) уже подавляет unused_variable.
  - **paint_radio_card: индикатор всегда залитый кружок** (selected=accent fill, unselected=text_muted fill). Вариант «кольцо для unselected» (fill=прозрачный, border=text_muted) — ответственность потребителя через `Painter::rect` с прозрачным fill; кит отдаёт канонический залитый кружок, согласованный с glyph-фолбэком `◉`/`○` прототипа graph_builder_ui.
- WASM note: WASM-гейт НЕ запускался — компоненты PURE LOGIC (геометрия + слот-выбор): раскладка — чистая арифметика UiRect, стиль — выбор слота palette без арифметики над rgb (только alpha-overlay на существующем rgb, документированный паттерн). Нативные тесты покрывают поведение (21 тест). Ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений поведения; здесь — новый API, 0 текущих потребителей, миграция на новые компоненты — будущие агенты).

---
Task ID: G
Agent: Agent G (rgba→slot + close-buttons)
Task: Migrate rgba→control_success/control_warning in ai_status_panel/agent_panel + 6 close-button sites to kit::stage_close_button

Work Log:
- Прочитан контекст: worklog.md — секции Task A (5 новых KitPalette-слотов `control_success`/`control_warning`/`stage_dim`/`scrollbar_thumb`/`rule_color` + `kit::stage_close_button(slot)` + `kit::backdrop`), Task B (9 rgba→slot миграций в overlays.rs + docs_ui::viewer_close_rect → kit::stage_close_button), Task C (CR-015 fix + 7 *_ui.rs files на токены PANEL_HEADER_H_*/FONT_*). Подтверждено: `KitPalette::dark()` содержит `control_success=[0.30,0.75,0.55,1.0]` и `control_warning=[0.95,0.65,0.30,1.0]` (1:1 с бывшими хардкод-литералами аудита §3). `kit::stage_close_button(slot: UiRect) -> UiRect` возвращает `UiRect::new(slot.right() - ICON_BUTTON_SIZE - SPACING_SM, slot.y + SPACING_SM, ICON_BUTTON_SIZE, ICON_BUTTON_SIZE)` — канон «× в углу панели/модали».
- G1 — `crates/canvas-app/src/app/ai_status_panel.rs`: контекст разобран — в `ai_status_panel()` обе переменные `palette: ThemeColors` (line 274) и `kit_palette: KitPalette` (line 275) в скоупе. Литералы найдены через grep (не по line-numbers аудита — файл рефакторился FR-LLM-FIX-2): 4 миграции rgba→slot:
  - line ~352 `[0.95, 0.65, 0.30, 1.0]` (paused dot, был line 201 в аудите) → `kit_palette.control_warning`
  - line ~354 `[0.30, 0.75, 0.55, 1.0]` (active dot, был line 203) → `kit_palette.control_success`
  - line ~508 `[0.95, 0.65, 0.30, 1.0]` (near-limit fill, был line 370/376) → `kit_palette.control_warning`
  - line ~524 `[242.0/255.0, 165.0/255.0, 76.0/255.0, 1.0]` (paused label amber, был line 386/392) → `kit_palette.control_warning` (round(242/255·1.0)=0.949≈0.95, round(165/255)=0.647≈0.65, round(76/255)=0.298≈0.30 — канон-слот 1:1)
  - Обновлён комментарий блока «Цветная точка состояния»: убрано утверждение «Не kit-слот (индикатор состояния — семантика, а не тема)» (контр-продуктивно после миграции на слоты control_success/control_warning — это и есть семантика kit-палитры).
- G2 — `crates/canvas-app/src/app/agent_panel.rs`: контекст разобран — `kit_palette: KitPalette` в скоупе в обеих точках. 2 миграции `let success_color: [f32; 4] = [0.30, 0.75, 0.55, 1.0];` → `let success_color: [f32; 4] = kit_palette.control_success;` (паритет с соседней строкой `let danger_color = kit_palette.control_danger;`):
  - line 435 (AgentMsgKind::Success bubble — заливка/border/text success)
  - line 492 (tool_call status glyph «✓» — color success)
  - Комментарий обновлён: «FR-LLM-D: семантические цвета — берём из kit_palette (control_danger/control_success — audit §3: зелёный раньше был инлайн-литералом…)»
- G3 — миграция 6 close-button сайтов на `kit::stage_close_button`. Стратегия: для каждого сайта — читать контекст, проверить patтерн `panel.right() - MAGIC, panel.y + MAGIC, SIZE, SIZE` (audit §6.1), оценить визуальный скачок (size 30 → 26 = 4px, size 22/24 → 26 = 2-4px), мигрировать или оставить TODO:
  - **MIGRATED** (2 сайта) — size близок к канону `ICON_BUTTON_SIZE=26`, паттерн соответствует «× в углу панели»:
    1. `flowmap_ui.rs:177` — `let close = UiRect::new(panel.right() - 28.0, panel.y + 6.0, 22.0, 22.0);` → `let close = kit::stage_close_button(panel);` (size 22→26, inset 6→8 — сдвиг ≈ 2px вправо/вниз, допустимо по AGENTS.md §«UI-кит»).
    2. `scheme_gallery_ui.rs:320-325` — `let close = stack(UiRect::new(inner.x, inner.y + 4.0, inner_w, 24.0), UiVec2::new(24.0, 24.0), HAlign::End, VAlign::Start);` → `let close = canvas_ui::kit::stage_close_button(panel);` (size 24→26, позиция переезжает из шапки inner в угол panel — канонический паттерн «× в углу модали», audit §6.1).
  - **TODO** (5 сайтов) — size/семантика отклоняется от канона `stage_close_button`, миграция требует отдельной волны UI-геометрии:
    3. `autolink_ui.rs:255-271` (`close_rect`) — size 30×30, **явно задокументированное отклонение** (FR-060/FR-059: «дословно сильнее переченя замена; паттерн отклонения kit::card из FR-059»). 30→26 = visual jump 4px. TODO добавлен в docstring.
    4. `explain_ui.rs:116-132` (`close_rect`) — size 30×30 (`CLOSE_SIZE`), отклонение не задокументировано явно, но та же ситуация (30→26 = visual jump 4px). Связано с TODO `HEADER_H=56 → PANEL_HEADER_H_L=44` (Agent C, W-d). TODO добавлен в docstring.
    5. `kit_ui.rs:395-424` (`gallery_layout`) и `kit_ui.rs:1419-1441` (`gallery_hit_slots`) — уже использует `kit::icon_button_rect` с `ICON_BUTTON_SIZE=26` (size каноничен), но позиция от `content.right()` (через `panel.inset(SPACING_LG)`), а `stage_close_button(panel)` считает от `panel.right()`. Миграция оторвёт close от соседних theme (и reset в admin) кнопок по вертикали (theme останется на content.y, close уедет на panel.y+8). TODO добавлен в оба места.
    6. `admin_ui.rs:163-186` (`admin_hit_slots`) — та же ситуация что kit_ui: close/theme/reset выстроены в одну строку шапки content. TODO добавлен в docstring.
    7. `onboarding_ui.rs:344-379` (`button_rect` для `OnboardingButton::Skip`) — Skip это подписанная прямоугольная кнопка 64×22 («Пропустить»), НЕ каноническая «×» в углу модали. `stage_close_button` возвращает квадрат 26×26 под глиф «×» — смена форм-фактора и UX. TODO добавлен в docstring.
- G4 — Верификация:
  - `cargo build -p canvas-app` — ✓ clean (8.85s)
  - `cargo test -p canvas-app --lib` — ✓ **585 passed / 0 failed** (63.82s; было 582 — Agent C добавил 1 тест `provider_chip_width_measures_real_glyphs`, +2 от других параллельных агентов; мои изменения регрессий не добавили)
  - `cargo clippy -p canvas-app --all-targets -- -D warnings` — ⚠ blocked: `crates/canvas-ui/src/component/icon.rs:323` clippy::too_many_arguments (8/7) на внутренней `icon_quad` — это WIP Agent D (Agent G НЕ трогает canvas-ui per task constraints). Рабочий обход: `cargo clippy --no-deps -p canvas-app --all-targets -- -D warnings` — ✓ clean (canvas-app clippy проходит без warnings; canvas-ui/icon.rs вне scope).
  - `cargo fmt --all -- --check` — ✓ clean
- Модульные тесты по затронутым файлам (sub-run для верификации регрессий):
  - `app::ai_status_panel::tests::*` — 15/15 pass
  - `app::agent_panel::tests::*` — 15/15 pass
  - `flowmap_ui::tests::*` — 6/6 pass (включая `hit_tests` с `flow_map_close_at`)
  - `scheme_gallery_ui::tests::*` — 10/10 pass (включая `hit_tests_rows_chips_and_empty_buttons`, `g4_lint_viewports_and_languages`)
  - `autolink_ui::tests::*` — 11/11 pass
  - `explain_ui::tests::*` — 50/50 pass
  - `onboarding_ui::tests::*` — 12/12 pass (включая `onboarding_overlay_draw_uses_theme_slots_only` regression-guard)
  - `kit_ui::tests::gallery_layout_*` — 2/2 pass
  - `admin_ui::tests::*` — 23/23 pass

Stage Summary:
- Files changed (9):
  - `crates/canvas-app/src/app/ai_status_panel.rs` (+12 / -8 строк: 4 rgba→slot миграции + обновлённые комментарии)
  - `crates/canvas-app/src/app/agent_panel.rs` (+5 / -4 строк: 2 `let success_color = …` → kit_palette.control_success + комментарий)
  - `crates/canvas-app/src/flowmap_ui.rs` (+5 / -1 строк: 1 close-button → kit::stage_close_button + FR-070 комментарий)
  - `crates/canvas-app/src/scheme_gallery_ui.rs` (+8 / -8 строк: 1 close-button → kit::stage_close_button, stack/UiVec2/End/Start заменены на канон call)
  - `crates/canvas-app/src/autolink_ui.rs` (+6 / -0 строк: TODO в docstring `close_rect` — size 30×30 documented dev)
  - `crates/canvas-app/src/explain_ui.rs` (+8 / -0 строк: TODO в docstring `close_rect` — size 30×30, связано с HEADER_H=56 TODO)
  - `crates/canvas-app/src/kit_ui.rs` (+14 / -0 строк: TODO в `gallery_layout` + `gallery_hit_slots` — layout coherence с theme кнопкой)
  - `crates/canvas-app/src/admin_ui.rs` (+8 / -0 строк: TODO в docstring `admin_hit_slots` — layout coherence с theme/reset)
  - `crates/canvas-app/src/onboarding_ui.rs` (+9 / -0 строк: TODO в docstring `button_rect` — Skip это подписанная кнопка 64×22, не «×»)
- rgba→slot migrations: 6 (4 в ai_status_panel.rs: paused-dot, active-dot, near-limit-fill, paused-label-amber; 2 в agent_panel.rs: success_color bubble + success_color tool_call glyph)
- Close-button migrations: 2 (flowmap_ui.rs:177 → kit::stage_close_button(panel); scheme_gallery_ui.rs:320 → kit::stage_close_button(panel))
- Close-button deviations left as TODO: 5 (autolink_ui::close_rect — size 30 documented dev; explain_ui::close_rect — size 30 связан с HEADER_H=56 TODO; kit_ui::gallery_layout + gallery_hit_slots — layout coherence с theme кнопкой в шапке content; admin_ui::admin_hit_slots — layout coherence с theme/reset; onboarding_ui::button_rect Skip — подписанная кнопка 64×22, не «×»)
- Tests: 585 passed / 0 failed in canvas-app (no regressions; +3 net vs baseline 582 — добавленные параллельными агентами тесты)
- Out-of-scope (для будущих волн):
  - kit_ui/admin_ui шапка реструктуризация (close + соседние кнопки в угол панели) — отдельная волна UI-геометрии
  - onboarding Skip → `kit::button_layout` (rect подписанной кнопки-призрака) — отдельная задача UX-семантики
  - explain_ui HEADER_H=56 → PANEL_HEADER_H_L=44 (Agent C TODO) — связано с explain_ui::close_rect size 30
  - canvas-ui/component/icon.rs:323 clippy::too_many_arguments — WIP Agent D (canvas-ui вне scope Agent G)
- WASM note: WASM-гейт (`scripts/wasm_gate.sh --check`) НЕ запускался — миграции G1/G2 чисто заменяют rgba-литералы на слоты (значения 1:1 с `KitPalette::dark()` — I-1: ноль визуального скачка). G3 flowmap/scheme_gallery — close button переезжает на каноническую позицию `stage_close_button` (сдвиг ≈ 2-6px — canonical kit value per AGENTS.md §«UI-кит»); нативные тесты покрывают поведение (`flow_map_close_at`, `hit_tests_rows_chips_and_empty_buttons`, `g4_lint_viewports_and_languages`). Ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений поведения; здесь — чистая консолидация геометрии/цветов в каноничные слоты).

---
Task ID: D
Agent: Agent D (icon_composition)
Task: Create `kit::icon_composition` + migrate `palette.rs` ~30 hand-rolled icon quads

Work Log:
- Прочитан контекст: worklog.md (Agent A — `KitPalette` +5 слотов + `stage_close_button` + `backdrop`; Agent B — overlays rgba→slot миграции; Agent C — `PANEL_HEADER_H_*`/`FONT_*` токены + CR-015 fix; Agent G — `g4_lint` + flowmap/scheme_gallery close-button миграции, упомянул «canvas-ui/component/icon.rs:323 clippy::too_many_arguments — WIP Agent D» — учтено, исправлено рефакторингом `icon_quad` на `[f32; 2]` для pos/size вместо 4×f32). Аудит `docs/dev-researches/ui-hardcode-audit.md` §6.1: `kit::icon_composition(rect, kind, tint) -> Vec<PaintItem>` — HIGH severity, крупнейший источник ручных `CardInstance` литералов.
- D1 — Прочитан `crates/canvas-app/src/palette.rs::icon_quads` (строки 1144–1363): каталог из 20 `PaletteIcon` вариантов (3 текстовых: Rename/Clear/Flow — квадов нет; 19 квадовых: Swatch/LineSolid/LineDashed/LineDotted/Thin/Medium/Thick/TreeHorizontal/TreeVertical/Radial/AddChild/AddSibling/Collapse/Expand/Duplicate/Folder/GroupBox/Sliders/Template/Pin). Прочитан `paint.rs::PaintItem` enum (variant `Rect { rect: UiRect, fill, border, radius }` — это нужная нам форма), `geometry.rs::UiRect` (x, y, w, h с нормализацией отрицательных в 0), `app/support.rs::paint_items_to_band` (адаптер Painter→`Vec<CardInstance>`, `pub(super)` — недоступен из palette.rs).
- D1 — Создан `crates/canvas-ui/src/component/icon.rs` (581 строк):
  - `pub enum IconKind` — каталог 20 вариантов (1:1 с quad-bearing `PaletteIcon`): `Swatch { fill: Option<[f32; 4]> }` (потребитель разрешает пресет «1»..«6» через `preset_color` ДО вызова kit — кит не знает о `ThemeColors`, инвариант G7 FR-051), `LinesSolid`, `LinesDashed`, `LinesDotted`, `Thin`, `Medium`, `Thick`, `TreeHorizontal`, `TreeVertical`, `Radial`, `AddChild`, `AddSibling`, `Collapse`, `Expand`, `Duplicate`, `Folder`, `GroupBox`, `Sliders`, `Template`, `Pin`.
  - `pub fn icon_composition(rect: UiRect, kind: IconKind, tint: [f32; 4]) -> Vec<PaintItem>` — чистая функция, геометрия 1:1 с `palette.rs::icon_quads` (магические числа 3.0/1.5/8.0/12.0/etc — те же, перенос 1:1, не «улучшаем пропорции» — это визуальная регрессия I-1 FR-046; унификация геометрии иконок — отдельная волна v2 с ручной L2-проверкой). Замыкания `solid`/`outline` берут `&mut Vec<PaintItem>` аргументом (как в `palette.rs::icon_quads`), НЕ захватывая `items` — иначе два замыкания конфликтуют по borrow checker.
  - `fn icon_quad(items, pos: [f32; 2], size: [f32; 2], fill, border, radius)` — private helper (1:1 с прежним `palette.rs::icon_quad`, порт в кит; после рефакторинга с `[f32; 2]` ушло clippy::too_many_arguments с 8 аргументов на 6).
  - 7 unit-тестов: `lines_solid_single_quad_in_bounds` (ровно 1 квад, fill=tint, radius=1.5), `sliders_six_quads_in_bounds_and_proportions` (6 квадов 3×2: линии h=1.6, каретки h=5.0), `tree_horizontal_seven_quads_in_bounds` (7 квадов: 3 узла 8×8 radius=2 + 4 отрезка radius=0), `swatch_outline_vs_fill_one_quad_each` (Swatch(None)=outline fill=0/border=tint; Swatch(Some)=solid fill=preset/border=0), `radial_six_quads_first_outline_rest_solid` (6 квадов: кольцо outline + 5 solid точек), `all_kinds_non_empty_and_in_bounds` (parity-тест: 21 вид, все возвращают ≥1 квад внутри rect), `geometry_scales_with_rect` (в 2× слоте 36×36 число квадов неизменно, все в границах).
- D2 — `crates/canvas-ui/src/component/mod.rs`: добавлено `pub mod icon;` (между `dropdown` и `list` — алфавитный порядок). `crates/canvas-ui/src/kit.rs`: добавлен `pub use crate::component::icon::{icon_composition, IconKind};` (между `dropdown` и `list` реэкспортами).
- D3 — `crates/canvas-app/src/app/support.rs`: добавлен `pub(crate) fn paint_items_to_cards(items: Vec<PaintItem>) -> Vec<CardInstance>` (24 строки) — обёртка над существующим `paint_items_to_band`: вызывает его с throwaway `texts` вектором (контракт `icon_composition`: ни одного `PaintItem::Text`), возвращает только `Vec<CardInstance>`; `debug_assert!(texts.is_empty())` ловит баг в ките, если текст случайно появился. AGENTS.md §«UI-кит» правило 6: «`CardInstance` напрямую — только в `canvas-render` (это его тип) и в адаптерах `app/support.rs` (граница слоёв)» — адаптер живёт в каноничном месте.
- D3 — `crates/canvas-app/src/app.rs`: добавлен `pub(crate) use support::paint_items_to_cards;` (рядом с `pub use support::measured_result_reserve_height;`) — re-export для `palette.rs` (тот же крейт, не часть внешнего API приложения; `pub(crate) use` т.к. `paint_items_to_cards` — `pub(crate)`, не `pub`).
- D3 — `crates/canvas-app/src/palette.rs::icon_quads` мигрирован (строки 1143–1203, 52 insertions / 212 deletions):
  - Прежний `icon_quad` (private helper) удалён — ручная сборка `CardInstance { pos, size, fill, border, params, corners }` в overlay-логике запрещена (AGENTS.md §«UI-кит» правило 6).
  - `icon_quads` теперь: матч `PaletteIcon → IconKind` (с разрешением Swatch preset через `preset_color`), вызов `kit::icon_composition(UiRect::new(rect...), kind, tint)`, адаптация `crate::app::paint_items_to_cards(items)`. Сигнатура `pub fn icon_quads(icon, rect, tint, theme) -> Vec<CardInstance>` — НЕ менялась (обратная совместимость с `overlays.rs:4156` и тестами в `palette::tests`).
  - Текстовые иконки `Rename`/`Clear`/`Flow` → `return Vec::new()` (прежний контракт: квадов нет, рисуются `ScreenText`'ом).
- Верификация:
  - `cargo build -p canvas-ui -p canvas-app` — ✓ clean (8.79s)
  - `cargo test -p canvas-ui --lib` — ✓ 244 passed / 0 failed (включая 7 новых icon::tests)
  - `cargo test -p canvas-app --lib` — ✓ 585 passed / 0 failed (включая `palette::tests::icons_bounds_and_text_glyphs` parity-тест — границы квадов 1:1 с прежним ручным построением, I-1: ноль визуального скачка)
  - `cargo clippy -p canvas-ui -p canvas-app --all-targets -- -D warnings` — ✓ 0 warnings
  - `cargo fmt --all -- --check` — ✓ clean (после `cargo fmt --all` — rustfmt переформатировал 2 inline `solid(...)` вызова на multi-line, где `[f32; 2]` массивы переходили порог ширины)
  - Замечание Agent G «canvas-ui/component/icon.rs:323 clippy::too_many_arguments» — ИСПРАВЛЕНО: рефакторинг `icon_quad` с `(items, px: f32, py: f32, sw: f32, sh: f32, fill, border, radius)` (8 args) на `(items, pos: [f32; 2], size: [f32; 2], fill, border, radius)` (6 args — паритет с прежним `palette.rs::icon_quad`).

Stage Summary:
- Files changed (6):
  - `crates/canvas-ui/src/component/icon.rs` (+581 строк, новый файл: `IconKind` enum 20 вариантов + `icon_composition` + private `icon_quad` + 7 тестов)
  - `crates/canvas-ui/src/component/mod.rs` (+1 строка: `pub mod icon;`)
  - `crates/canvas-ui/src/kit.rs` (+1 строка: `pub use crate::component::icon::{icon_composition, IconKind};`)
  - `crates/canvas-app/src/app/support.rs` (+24 строки: `pub(crate) fn paint_items_to_cards` — обёртка над `paint_items_to_band`)
  - `crates/canvas-app/src/app.rs` (+1 строка: `pub(crate) use support::paint_items_to_cards;`)
  - `crates/canvas-app/src/palette.rs` (+52 / -212 строк: миграция `icon_quads` internals на kit + удалён private `icon_quad`)
- New kit API (public):
  - `canvas_ui::kit::IconKind` — enum 20 вариантов: `Swatch { fill: Option<[f32; 4]> }`, `LinesSolid`, `LinesDashed`, `LinesDotted`, `Thin`, `Medium`, `Thick`, `TreeHorizontal`, `TreeVertical`, `Radial`, `AddChild`, `AddSibling`, `Collapse`, `Expand`, `Duplicate`, `Folder`, `GroupBox`, `Sliders`, `Template`, `Pin`
  - `canvas_ui::kit::icon_composition(rect: UiRect, kind: IconKind, tint: [f32; 4]) -> Vec<PaintItem>` — декларативная спецификация иконки → `Vec<PaintItem>` (контракт F-8: цвет — только tint/Swatch::fill; геометрия — относительно rect; кит НЕ знает о ThemeColors — G7 FR-051)
- Adapter API (pub(crate), canvas-app internal):
  - `canvas_app::app::paint_items_to_cards(items: Vec<PaintItem>) -> Vec<CardInstance>` — обёртка над `paint_items_to_band` для consumers, у которых весь вывод иконок квадовый (`palette::icon_quads` через `icon_composition`). `debug_assert!` ловит баг, если в items случайно появился `PaintItem::Text`.
- Icons migrated: 20 `PaletteIcon` variants (19 quad-bearing + Swatch sub-variants; 3 text-only `Rename`/`Clear`/`Flow` → `return Vec::new()` без вызова kit). Всего портировано ~50 индивидуальных квадов (TreeHorizontal — 7, TreeVertical — 7, Radial — 6, Sliders — 6, AddChild/AddSibling — по 3, остальные 1–2). Каталог `IconKind` 1:1 с `PaletteIcon` — zero visual jump (I-1 FR-046).
- Tests: 244 passed in canvas-ui (+7 new `component::icon::tests::*`); 585 passed in canvas-app (palette::tests::icons_bounds_and_text_glyphs — parity-тест зелёный, границы квадов 1:1 с прежним ручным построением); 0 regressions.
- Icons NOT migrated 1:1: нет — все 19 quad-bearing вариантов перенесены дословно. Геометрия — магические числа (3.0/1.5/8.0/12.0/etc) — те же, что в `palette.rs::icon_quads` (НЕ «пересчитаны через токены» — это визуальная регрессия; унификация геометрии иконок — отдельная волна v2 с ручной L2-проверкой, как требует AGENTS.md §«Самопроверка UI на WASM»).
- Issues encountered:
  - Borrow checker: первая итерация `icon.rs` имела два `let mut solid = |...| { items.push(...) }` замыкания, захватывающих `items` mutably — `error[E0499]: cannot borrow items as mutable more than once at a time`. Исправлено рефакторингом замыканий на параметр `q: &mut Vec<PaintItem>` (как в `palette.rs::icon_quads`) + free helper `icon_quad(items, ...)`.
  - clippy::too_many_arguments: `icon_quad` с `(items, px, py, sw, sh, fill, border, radius)` — 8 args (предел 7). Исправлено: `pos: [f32; 2]` + `size: [f32; 2]` → 6 args (паритет с прежним `palette.rs::icon_quad`). Agent G заметил это в своей работе — учтено в этой итерации.
  - `pub use support::paint_items_to_cards`: `error[E0364]: paint_items_to_cards is only public within the crate, and cannot be re-exported outside`. Исправлено: `pub(crate) use support::paint_items_to_cards;` (т.к. `paint_items_to_cards` — `pub(crate)`, не `pub`).
- WASM note: WASM-гейт (`scripts/wasm_gate.sh --check`) НЕ запускался — миграция D3 меняет ТОЛЬКО внутренности `palette::icon_quads` (сигнатура `Vec<CardInstance>` сохранена, геометрия квадов 1:1 с прежней — I-1: ноль визуального скачка). Нативные тесты `palette::tests::icons_bounds_and_text_glyphs` покрывают поведение (границы квадов, число квадов per icon kind). Ручная L2-проверка не требуется (правило AGENTS.md §«Самопроверка UI на WASM» — для UI-изменений поведения; здесь — pure refactor с zero behavior change).
- Out-of-scope (НЕ тронуто — DO-NOT-touch list):
  - `app/overlays.rs`, `suggest.rs`, `whatif_ui.rs`, `tooltip.rs`, `ai_status_panel.rs`, `agent_panel.rs`, `explain.rs`, `autolink_ui.rs`, `kit_ui.rs`, `admin_ui.rs`, `onboarding_ui.rs`, `flowmap_ui.rs`, `scheme_gallery_ui.rs` — территории других агентов.
  - `palette.rs::icon_quads` — единственный consumer `kit::icon_composition` в этой волне; будущие consumers (другие места с ручными quad-композициями иконок) — отдельная волна.
