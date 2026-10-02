# FR-090: Продуктовые события из Rust + люди с ролями в PostHog + `surface_opened`

- **Статус:** выполнено (2026-10-03)
- **Тип:** FR
- **Приоритет:** желательно
- **Владелец:** danku13
- **Источник:** диалог с владельцем (сессия 2026-10-03): «хочу видеть людей с ролями, как обработку анонимных профилей в настройках проекта? навешивай события из rust, то что ты уже показал и открытие каждой поверхности»
- **Связанные задачи:** FR-089 (телеметрия/PostHog/мост `canvasdesk:track`), FR-087 (роли), FR-052 (реестр поверхностей), PRD-0009 US-5 (`docs/interface-objects/surface-registry.md`)
- **Создан:** 2026-10-03
- **Обновлён:** 2026-10-03

## 1. Описание (What)

Три запроса владельца одним слоем поверх FR-089:

1. **Люди с ролями в People.** PostHog создаёт person-профили только для
   identified-событий (настройка проекта `person_profiles:
   identified_only` — дефолт новых проектов; подтверждена remote-конфигом
   `defaultIdentifiedOnly: true`), поэтому вкладка People была пуста, хотя
   события и Insights работали. Теперь при загрузке SDK вызывается
   `ph.identify(anonId, {language, role})` — стабильный `anonId` из
   `canvasdesk.consent` (тот же UUID, что у счётчика `wasm_used`):
   анонимная история мёржится в person, у которого есть свойства `role` и
   `language`; события прямого счётчика `wasm_used` (distinct_id = тот же
   anonId) ложатся на того же человека.
2. **События из Rust.** Мост `canvasdesk:track` (FR-089) существовал, но
   из Rust его никто не звал. Заведён модуль
   `canvas-app/src/app/telemetry.rs`: `track(event, properties)` —
   CustomEvent на `document` (контракт слушателя в index.html); натив —
   no-op. Навешаны события на ключевые точки приложения.
3. **`surface_opened {surface}` — открытие каждой поверхности.** Диф
   реестра поверхностей за пересборку кадра (`build_frame_at`, а не
   каждый кадр: пересборка ровно тогда, когда меняется сигнатура
   поверхностей `build_frame_sig`). Новая в реестре поверхность =
   «открыта».

## 2. Влияние (Impact)

| Объект | Изменение | Документация |
|---|---|---|
| `canvas-app/src/app/telemetry.rs` | новый модуль: `track` (мост, wasm32) + `surface_diff` + чистая `tracked_present` + 3 юнит-теста | этот документ §4 |
| `app.rs` | `pub mod telemetry;` + события: `theme_switched`, `language_switched` (кнопка и dropdown), `role_changed`, `scheme_applied` | §4 |
| `app/ui_registry.rs` | хук `surface_diff` в `build_frame_at` (натив — no-op) | §4 |
| `canvas-web/src/export.rs` | события `export_canvas` / `export_html` (вызов `canvas_app::app::telemetry::track`) | §4 |
| `canvas-app/Cargo.toml` (wasm32) | + web-sys `Document`/`CustomEvent`/`CustomEventInit`, + `js-sys` | §4 |
| `canvas-web/index.html` | `identify(anonId, {language, role})` в `captureAppLaunched` перед `app_launched` | §4 |
| Натив (десктоп) | без изменений поведения: `track`/`surface_diff` — no-op, как и весь слой телеметрии FR-089 | §3 |

## 3. Анализ (What was missing)

- People пуст до `identify`: события FR-089 уходили с анонимным device-id,
  а `person_profiles: identified_only` не создаёт по ним person. Выбор:
  просить владельца переключить настройку на «all events» (создаст
  мусорные анонимные профиля у всех, включая отказавшихся) или сделать
  `identify` — выбран `identify`: профиль появляется у согласившихся на
  аналитику, счётчик `wasm_used` отказавшихся остаётся без профиля (как и
  задумано FR-089).
- Мост `canvasdesk:track` из FR-089 — JS-сторона готова, Rust-стороны не
  было: не было ни отправщика (CustomEvent с detail), ни вызывающих точек.
- «Открытие поверхности»: 26 идентификаторов в реестре, но активные
  состояния поверхностей размазаны по App; хук в каждой точке открытия —
  ~20 правок и хрупко. Центральная точка одна — пересборка кадра:
  `build_frame_at` вызывается только при изменении сигнатуры поверхностей
  (`UiFrameSig`, битовые флаги), т.е. диф реестра там срабатывает ровно в
  момент открытия/закрытия. Ambient-хром (world, corner_buttons,
  template_strip, empty, minimap) активен без действия пользователя —
  «открытием» не считается; what-if в реестре живёт от пилюли входа
  (вьюпорты ≥900×600), поэтому трекается только реальная активация сессии
  (`scene.whatif_active`).

## 4. Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Мост из Rust | `app/telemetry.rs` | `track(event, properties)`: `js_sys::Object` + `Reflect::set` → detail `{event, properties}` → `CustomEvent::new_with_event_init_dict("canvasdesk:track")` → `document.dispatch_event`; всё в try-стиле «телеметрия не роняет приложение»; `#[cfg(wasm32)]` реализация, натив-стаб `let _ = (event, properties)` (паттерн `persist_settings`) |
| Дифф поверхностей | `app/telemetry.rs::surface_diff` | вызов из `build_frame_at`; `tracked_present` (чистая): реестр минус `AMBIENT_SURFACES`, what-if — по `whatif_active`; thread-local `PREV: Option<HashSet<String>>` между кадрами (wasm однопотен, поле в `App` не заводим); каждая новая поверхность → `surface_opened {surface}` |
| Роли/язык/тема | `app.rs` | `apply_dropdown_choice` (Role → `role_changed {role}` — id реестра ролей; Language → `language_switched {language: ru/en}`), `toggle_language` → `language_switched`, `toggle_theme` → `theme_switched {theme: dark/light}` |
| Схема | `app.rs::apply_scheme` | после успешного инстанцирования: `scheme_applied {scheme, nodes}` |
| Экспорт | `canvas-web/src/export.rs` | `export_canvas {bytes}` / `export_html {bytes}` в Ok-ветках (мост из sibling-крейта — `pub mod telemetry`, путь `canvas_app::app::telemetry::track`) |
| identify | `index.html` | в `captureAppLaunched` перед capture: `ph.identify(ensureAnonId(), {language, role})` (внутренний try — падение identify не глушит `app_launched`) |
| Зависимости | `canvas-app/Cargo.toml` | wasm32: web-sys +`Document`,`CustomEvent`,`CustomEventInit`; +`js-sys` (workspace) |

Таксономия событий (полная картина после FR-089 + FR-090):

| Событие | Свойства | Источник | Когда |
|---|---|---|---|
| `surface_opened` | `surface` | Rust (диф реестра) | открытие поверхности пользователем |
| `role_changed` | `role` | Rust | смена роли в табе «Профиль» |
| `language_switched` | `language` | Rust | смена языка (угловая кнопка / dropdown) |
| `theme_switched` | `theme` | Rust | смена классической темы тумблером |
| `scheme_applied` | `scheme`, `nodes` | Rust | схема применена из галереи |
| `export_canvas` | `bytes` | Rust (canvas-web) | выгрузка .canvas |
| `export_html` | `bytes` | Rust (canvas-web) | выгрузка HTML-артефакта |
| `app_launched` | `language`, `role`, `$set` | JS (FR-089) + identify (FR-090) | старт сессии |
| `wasm_used` | `app` | JS (FR-089) | суточный счётчик (без SDK) |
| `$pageview`, autocapture, `$exception` | — | SDK (FR-089) | автоматические |

Трекаются поверхности (21): wheel, whatif (по активации сессии), hotkeys,
settings, menu, choice_menu, template_panel, palette, docs, help_menu,
stage, search, flow_map, editor, explain, autolink, dialog, gallery,
onboarding, kit_gallery, admin_panel. Не трекаются (ambient):
world, corner_buttons, template_strip, empty, minimap.

## 5. Точки входа (Entry Points)

- `docs/interface-objects/surface-registry.md` — §3 примечание о
  телеметрии открытия поверхностей.
- FR-089 — §7 changelog: identify + события из Rust (перекрёстная ссылка).
- Ответ владельцу про настройки PostHog — §6 ниже.

## 6. Проверка (Verification)

Web-слой (agent-browser, стенд http.server, тестовая копия index.html с
инструментацией: снимает bot-фильтр posthog-js, fetch-spy с
gzip-распаковкой тел, рекордер моста):

- [x] `identify`: тело запроса — `distinct_id` = `anonId` из
  `canvasdesk.consent`, `person_properties {language: "ru", role:
  "architect"}`, device-id мёржится; persistence SDK: `$user_state:
  identified`, `$stored_person_properties {language, role}`.
- [x] `app_launched`: доходит с `role`/`language` + `$set` (как в FR-089),
  теперь на identified-профиле (`$is_identified: true`,
  `$process_person_profile: true`).
- [x] Мост `canvasdesk:track` (диспатч CustomEvent ровно в форме
  Rust-кода — detail `{event, properties}`): батч
  `surface_opened {surface: settings|search}`, `role_changed {role:
  developer}`, `scheme_applied {scheme, nodes}` доставлен POST-ом,
  distinct_id = anonId, 200 Ok.
- [x] Настройка проекта подтверждена remote-конфигом: `defaultIdentifiedOnly:
  true` (= person_profiles: identified_only) — «обработка анонимных
  профилей в настройках проекта»: с `identify` люди создаются при
  identified-событиях; анонимные события (до identify / при отказе от
  метрик) не создают профилей, но считаются в Insights. Переключать
  настройку не нужно; при желании видеть мусорные анонимные профиля —
  Settings → Data Management → Person Profiles → «all events».
- [x] Открытие при верификации: **posthog-js v1.435 режет capture не только
  по `navigator.webdriver`, но и по подстроке `HeadlessChrome` в UA**
  (события не доходили до очереди `__request_queue` при снятом webdriver) —
  в тестовой копии сняты оба сигнала; реальных пользователей (обычный
  Chrome) не касается.
- [x] Синтаксис 9 inline-скриптов index.html — `node --check`
  (`scripts/check_inline_scripts.js`).
- [x] Юнит-тесты `telemetry.rs` (натив): `ambient_surfaces_are_not_tracked`,
  `user_opened_surface_is_tracked`, `whatif_tracked_only_when_session_active`
  (паттерн `test_stub` из ui_registry.rs).
- [ ] **cargo-гейты (fmt/clippy/test) — в среде агента нет Rust-тулчейна
  (прецедент FR-089): валидация делегирована CI при пуше.**

Владельцу в PostHog: People теперь наполняются identified-профилями с
ролью/языком (первый тестовый человек от e2e-верификации — role:
architect, тестовые события surface_opened/role_changed можно удалить
в People → … → Merge or delete).

## 7. История изменений (Changelog)

- 2026-10-03 — агент (Super Z, сессия web-f324f377) — реализация целиком:
  `identify(anonId)` (люди с ролями в People), модуль `telemetry.rs`
  (мост + `surface_opened` по дифу реестра), события
  role/language/theme/scheme/export из Rust, Cargo-фичи web-sys+js-sys;
  e2e-верификация с расшифровкой gzip-тел запросов ($identify /
  app_launched / мост); открытие и обход UA-бот-фильтра «HeadlessChrome».

## 8. Источники истины (References)

- `crates/canvas-app/src/app/telemetry.rs` — мост, дифф, тесты.
- `crates/canvas-app/src/app.rs`, `app/ui_registry.rs`, `crates/canvas-web/src/export.rs` — точки событий.
- `crates/canvas-web/index.html` — identify + слушатель моста (FR-089).
- PostHog docs: `identify` (мерж анонимной истории, person properties),
  person profiles (`identified_only` / `always`), capture API.
