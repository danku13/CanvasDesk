# FR-089: Телеметрия web-версии — двухуровневое согласие, PostHog и анонимный счётчик пользователей WASM

- **Статус:** выполнено
- **Тип:** FR
- **Приоритет:** желательно
- **Владелец:** danku13
- **Источник:** диалог с владельцем (сессия 2026-10-03): «есть ли смысл внедрять sentry… может есть более полезные для проекта альтернативы, например совмещённые с продуктовой аналитикой допустим PostHog. при первом входе на странице с выбором языка и роли можно было бы проставить отдельный пункт про согласие со сбором мониторинга ошибок и продуктовых метрик которые помогают проекту, по умолчанию она может быть checked, далее нужен какой-то счётчик где будет понятно какое количество уникальных пользователей взаимодействовало с WASM сервисом, даже если кто-то из них не согласился отдавать телеметрию» + решение: «давай делать в рамках твоего предложения, но чекбоксы оставляем предвыбранными»
- **Связанные задачи:** FR-028 (пикер первого запуска), FR-087 (единая страница язык+роль, таб «Профиль»), FR-039 (модалка настроек), FR-040 v2 (web-персист localStorage), `docs/WASM-TESTING.md`
- **Создан:** 2026-10-03
- **Обновлён:** 2026-10-03

## 1. Описание (What)

Web-версия не собирала ошибок и продуктовых метрик: падение обработчика (`wheel`, drag, WASM-пайплайн) у пользователя оставалось невидимым, а число реальных пользователей WASM-сервиса не измерялось. Требование владельца — платформа «всё-в-одном» (выбран **PostHog** против чистого Sentry: продуктовая аналитика + ошибки + session replay + feature flags, free tier ~1M событий/мес), согласие собирается на странице первого запуска (язык + роль, FR-087 v2), чекбоксы **предвыбраны**, и отдельная метрика «уникальные пользователи WASM-сервиса» должна работать **даже для отказавшихся от полной телеметрии**.

Дизайн согласия — **двухуровневый** (решение владельца):

| Уровень | Чекбокс/тумблер | Что отправляет | Куда |
|---|---|---|---|
| `counter` | «Анонимный счётчик использования» | один event `wasm_used` в UTC-сутки, `distinct_id` = анонимный UUID, свойства `{app}` | прямая POST `/batch/` — **без SDK** |
| `analytics` | «Метрики и отчёты об ошибках» | ошибки окна (`$exception`), автозахват/продуктовые события | PostHog SDK (jsDelivr `+esm`, динамический `import()`) |

Счётчик отделён от SDK намеренно: отказ от метрик не глушит метрику уникальных пользователей (событие отправляется напрямую, без загрузки SDK) — ровно требование «считать даже тех, кто не согласился отдавать телеметрию», при этом согласие на сам счётчик получено отдельной строкой с явным описанием (честный паттерн Home Assistant/VS Code, проходит GDPR-логику «granular consent»).

## 2. Влияние (Impact)

| Объект | Изменение | Документация |
|---|---|---|
| Пикер первого запуска (index.html) | + блок «Помочь проекту»: 2 предвыбранных чекбокса, RU/EN, стилистика акцентов FR-087 v2 | FR-028 v2, FR-087 v2 |
| `Settings` (canvas-core) | + `telemetry_counter: bool` / `telemetry_analytics: bool` (serde default true, TOML `canvasdesk.config`/config.toml) | `crates/canvas-core/src/settings.rs` |
| Модалка настроек, таб «Профиль» | + 2 тумблера (сразу за «Роль»): `SettingsRow::TelemetryCounter/Analytics` | FR-039, FR-087 |
| Web-персист (canvas-app) | `persist_settings_web` синхронизирует JSON `canvasdesk.consent` + эмит `canvasdesk:consent-changed` | FR-040 v2 |
| JS-модуль телеметрии (index.html) | новый `window.__cdTelemetry`: согласия, PostHog, счётчик | этот документ §4 |
| i18n | + 4 ключа RU/EN | `crates/canvas-app/src/i18n.rs` |

Натив (desktop): поля персистятся в config.toml, отправки нет (тумблеры видны, описание честно про web-контекст — поле-заготовка под будущий нативный коллектор).

## 3. Анализ (What was missing)

- Не было ни согласий, ни слоя телеметрии: web-слой (index.html) не содержал SDK/отправки; `Settings` не имел полей согласий.
- Счётчик уникальных пользователей WASM отсутствовал как класс: единственный «сигнал жизни» — логи `tracing` в консоли браузера, не агрегируемые.
- Ключевой архитектурный вопрос — источник правды согласий при двух слоях (JS рантайм до WASM init и Rust-настройки после): решено двойной записью с одинаковой семантикой — TOML `canvasdesk.config` (Rust, тумблеры) + JSON `canvasdesk.consent` (JS-рантайм: те же флаги + `anonId` + `lastWasmDay`), синхронизация в одну сторону из Rust при персисте (мерж, anonId/отметка суток не трогаются) + событие `canvasdesk:consent-changed` для живого применения.

## 4. Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Поля согласий | `canvas-core/src/settings.rs` | `telemetry_counter`/`telemetry_analytics` (container serde default → `Default::default()` = true: старые конфиги грузятся включёнными, как предвыбранные чекбоксы); round-trip-литерал и новый тест `telemetry_consent_defaults_and_parse` |
| i18n | `canvas-app/src/i18n.rs` | ключи `ROW/DESC_TELEMETRY_COUNTER/ANALYTICS`, RU+EN |
| Строки настроек | `canvas-app/src/settings_ui.rs` | варианты `TelemetryCounter/TelemetryAnalytics` (Toggle), `SETTINGS_ROWS` 41→43, таб «Профиль» (после Role), все 6 match-функций + тесты инвариантов |
| Применение тумблеров | `canvas-app/src/app/overlays.rs` | arm инверсии bool-поля (персист общим хвостом `save_settings`), arm pill-рендера из bool-поля |
| Синхронизация согласий | `canvas-app/src/app.rs` | `sync_consent_web`: мерж JSON `canvasdesk.consent` (сохраняет `anonId`/`lastWasmDay`), эмит `canvasdesk:consent-changed` только при реальной смене; Cargo.toml web-sys +Event/EventTarget |
| Чекбоксы пикера | `canvas-web/index.html` | блок `.consent` (2 чекбокса, hint, RU/EN живая перерисовка), `pick()` дописывает `telemetry_*` в TOML и создаёт JSON-consent с `anonId` (crypto.randomUUID с фолбэком) |
| Модуль телеметрии | `canvas-web/index.html` | `window.__cdTelemetry`: `readConsent/writeConsent`, `trackWasmUsed` (суточный дедуп + `/batch/`), `loadPostHog` (`import()` только при analytics; `capture_exceptions`), `$exception` по window error/unhandledrejection, живой opt-in/out по событию + `storage` (кросс-вкладка), once-слушатели первого жеста (pointerdown/keydown/wheel) → счётчик «взаимодействовал», всё в try/catch — телеметрия не роняет приложение |
| Конфиг ключа | `canvas-web/index.html` | `POSTHOG_KEY = ""` (placeholder) + `POSTHOG_HOST` (us/eu) с инструкцией в комментарии; **пустой ключ = полный no-op** (SDK не грузится, счётчик не отправляется) |
| CSS | `canvas-web/index.html` | компакт-раскладка пикера (gap 24→16, step 16→12, padding 24→16 — весь контент с consent-блоком влезает в 1280×720 без скролла); чекбоксы `accent-color: #D68A32`, label hover — тинт `#5BAEC4`, focus-within outline `#5BAEC4` |

## 5. Точки входа (Entry Points)

- `docs/interface-objects/onboarding-v2-inline.md` — упомянуть consent-блок пикера (сделано: см. §7 changelog).
- FR-087 (таб «Профиль») — расширен двумя тумблерами; index-cr-fr обновлён.
- PostHog-инструкция — комментарий «КОНФИГ ВЛАДЕЛЬЦА» в index.html (шаги регистрации, phc_-ключ, регион us/eu, allowed domains для публичного репо).

## 6. Проверка (Verification)

Web-слой (браузер, agent-browser + VLM, python http.server стенда):

- [x] Пикер: блок «Помочь проекту (необязательно)» с 2 предвыбранными чекбоксами; RU дефолт, EN переключение перерисовывает тексты живо.
- [x] Полный цикл: снят чекбокс метрик → клик роли «Architect» → localStorage `canvasdesk.config` = `language="en"`/`role="architect"`/`telemetry_counter=true`/`telemetry_analytics=false`; `canvasdesk.consent` = `{counter:true, analytics:false, anonId:"<uuid>", lastWasmDay:null}`; после reload пикер скрыт, `window.__cdTelemetry.getConsent()` = тот же JSON.
- [x] Живой ресинк: `canvasdesk:consent-changed` (window и document) и `storage`-событие → перечитывание согласий без перезагрузки.
- [x] No-op: без `POSTHOG_KEY` — `trackWasmUsed()` безопасен, страница без ошибок (`agent-browser errors` пуст).
- [x] Стили: computed — label hover `rgba(91,174,196,.1)` (акцент-2), чекбокс accent `rgb(214,138,50)` (акцент-1).
- [x] Вёрстка: VLM-аудит 1280×720 — блок целиком виден включая hint (первый прогон нашёл обрезку hint на 720p — исправлено компакт-раскладкой, повторный VLM-прогон чист).
- [x] Синтаксис всех 8 inline-скриптов — `node --check` (скрипт `scripts/check_inline_scripts.js` рабочей директории агента).

Rust-слой:

- [x] Полнота match: все 7 match-выражений `settings_ui.rs`, 2 в `overlays.rs` (apply + pill-рендер), wildcard в `sync_settings_row` — новые варианты закрыты; `SETTINGS_ROWS` = 43 = фактическое число элементов; таб «Профиль» = 12 строк.
- [x] Полевые литералы: все `Settings { … }` со `..Default` (7 мест) + полный литерал `toml_round_trip` расширен новыми полями.
- [x] Тесты: `telemetry_consent_defaults_and_parse` (canvas-core: дефолты/parse/отказ), инварианты настроек обновлены (в т.ч. `tabs_cover_all_rows`, `every_row_has_label_and_description` — ключи RU/EN симметричны).
- [ ] **cargo-гейты (fmt/clippy/test) — не прогонялись в среде агента (нет Rust-тулчейна); изменения механические по образцу FR-087. Проверка delegируется CI (`.github/workflows/ci.yml`: fmt --check → clippy -D warnings → cargo test --workspace) при пуше.** Владельцу: после вставки `phc_`-ключа прогнать ручную приёмку пунктов выше.

## 7. История изменений (Changelog)

- 2026-10-03 — агент (Super Z, сессия web-f324f377) — реализация целиком (web-слой + core + настройки + i18n + доки), статус «выполнено», web-верификация браузером/VLM, Rust-валидация через CI.
- 2026-10-03 — фикс вёрстки: компакт-раскладка пикера (hint consent-блока обрезался на 720p — найден VLM-аудитом, исправлен и перепроверен).
- 2026-10-03 — перенумерация FR-088 → FR-089: номер FR-088 параллельно заняла другая сессия агента (`fr-088-search-rows-explain-trigger.md`, адаптивная вёрстка поиска) — по правилу индекса (прецедент FR-083) перенумерация механическая, все код-комментарии переименованы.

## 8. Источники истины (References)

- `crates/canvas-web/index.html` — чекбоксы пикера, модуль `__cdTelemetry`, конфиг `POSTHOG_KEY`/`POSTHOG_HOST`.
- `crates/canvas-core/src/settings.rs` — поля согласий, дефолты, тест.
- `crates/canvas-app/src/settings_ui.rs`, `app/overlays.rs`, `app.rs`, `i18n.rs`, `Cargo.toml` — строки/применение/синхронизация.
- PostHog docs: JS SDK (dynamic import), Capture API `/batch/`, `opt_in/out_capturing`, error tracking (`$exception`, `capture_exceptions`), allowed domains.
- Приватность: в `wasm_used` нет браузера/URL/действий — только анонимный UUID и факт «пользовался сегодня»; полный отказ = нулевой трафик.
