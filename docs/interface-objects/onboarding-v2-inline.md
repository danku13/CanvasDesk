# Онбординг v2 (inline-тур) — интерфейсный объект

Документ по FR-028 (см. `fr-028-onboarding-carousel.md`): расширение
существующего онбординга v1 (карусель карточек внутри canvas,
реализована в Rust/WASM) до **интерактивных inline-сценариев** в стиле
Intro.js / Guida.js / flows.sh. v2 — DOM-overlay поверх WebGPU-canvas:
пользователь не читает про канвас, а **выполняет реальные действия** в
живом канвасе под подсветкой и тултипом.

---

## 1. Определение

**Inline-тур** — DOM-overlay над canvas: dim-фон с SVG-cutout вокруг
текущего якоря, тултип с заголовком / телом / кнопками
(Back / Skip / Next), и state-machine, который ведёт пользователя по
сценарию из N шагов. Сценарий — plain-data JSON (Guida.js-стиль); каждый
шаг может сопровождаться `perform`-действиями (Intro.js click) и
`waitFor`-гейтом (flows.sh-контракт «выполнилось → продвигаемся»).

**Границы ответственности:**

- `sdk/web-onboarding/src/tour.ts` — движок: state-machine, lifecycle,
  rAF-refresh, keyboard (Esc / ← / → / Enter), signal-bus.
- `sdk/web-onboarding/src/positioning.ts` — 13 направлений tooltip +
  auto-flip + viewport clamp.
- `sdk/web-onboarding/src/highlight.ts` — dim overlay с SVG-cutout
  (even-odd mask, refresh через rAF).
- `sdk/web-onboarding/src/tooltip.ts` — DOM-tooltip с caret.
- `sdk/web-onboarding/src/actions.ts` — `performActions` (click / focus /
  signal / dispatch) + `awaitWaitFor` (selector / predicate / event /
  signal).
- `sdk/web-onboarding/src/scenarios/*.ts` — демо-сценарии (toolbar /
  first-run-inline / palette / calculations).
- `sdk/web-onboarding/canvasdesk-tour.js` — pre-built vanilla bundle
  (iife, ES2018, zero deps).
- `crates/canvas-web/index.html` — точка интеграции: `<script>`-тег
  грузит bundle, кнопка «Тур» в `#w6-toolbar` открывает picker.

**Не вторгается в Rust-код:** v2 работает alongside v1 карусели
(`onboarding_ui.rs`). v1 объясняет концепции — v2 даёт их попробовать.

---

## 2. Архитектурные влияния

| Библиотека   | Концепция                                | Где применяется                         |
|--------------|------------------------------------------|-----------------------------------------|
| **Intro.js** | overlay + tooltip + highlight cutout     | `highlight.ts`, `tooltip.ts`, `tour.ts` |
| **Guida.js** | сценарий как plain-data JSON              | `TourScenario` / `TourStep` типы       |
| **flows.sh** | perform+waitFor action runner с гейтом   | `actions.ts` (perform / waitFor)       |

---

## 3. API

### Tour

```ts
class Tour {
  constructor(opts?: TourOptions);
  run(scenario: TourScenario): TourHandle;
  onSignal(listener: (name: string, payload?: unknown) => void): () => void;
  signal(name: string, payload?: unknown): void;
  refresh(): void;
}
```

### TourStep

```ts
interface TourStep {
  id?: string;
  title: string;
  body?: string;
  anchor?: AnchorSpec;          // selector | element | rect
  side?: TooltipSide;           // top/bottom/left/right × start/end + center
  highlight?: HighlightOptions; // padding / radius / dim / dimColor / animate
  perform?: PerformSpec[];      // действия при активации шага
  waitFor?: WaitForSpec;        // гейт: Next disabled до resolve
  primaryLabel?: string;
  passive?: boolean;            // без Next (только waitFor/advanceOnClick)
  hideSkip?: boolean;
  advanceOnClick?: string | HTMLElement;
  meta?: Record<string, unknown>;
}
```

Полный справочник типов — `sdk/web-onboarding/src/types.ts` и
`sdk/web-onboarding/README.md`.

---

## 4. Сценарии (демо)

| ID                       | Имя                              | Скоープ                                          |
|--------------------------|----------------------------------|--------------------------------------------------|
| `cd-toolbar-tour`        | Тур по панели хранилища (W6)     | `#btn-open`, `#btn-recent`, `#btn-export`        |
| `cd-first-run-inline`    | Первый запуск: интерактив       | `waitFor(signal)`: `canvas:note-created`        |
| `cd-palette-tour`        | Палитра шаблонов (FR-018)        | Ctrl+P, колесо категорий, Shift+клик            |
| `cd-calculations-tour`   | Расчёты Numi (FR-013/014/015)    | Переменные / единицы / value-flow / what-if / MC |
| `cd-scheme-gallery-tour` | Галерея схем (FR-049)           | 8 шагов, 3 passive+waitFor (open/preview/apply) |

Сценарии живут в `sdk/web-onboarding/src/scenarios/`. Расширение —
новый `.ts`-файл + export в `scenarios/index.ts`. Хост-приложение
может также инлайнить сценарии как JSON-объекты (Guida.js-стиль).

---

## 5. Сигнальный мост (WASM ↔ JS)

Главный механизм интеграции движка с Rust/WASM-хостом —
**сигнальный bus**. Движок выставляет `window.__canvasdeskTour = tour`
(см. `crates/canvas-web/index.html`); WASM-bridge (через
`canvas_web::js_glue`) вызывает:

```js
window.__canvasdeskTour.signal("canvas:note-created", { id: "n_1" });
```

Сценарий в `waitFor.kind: "signal", signals: ["canvas:note-created"]`
продвигается дальше. См. шаг «Создайте первую заметку» в
`firstRunInlineScenario`.

**Регистрируемые сигналы (контракт):**

| Сигнал                   | Когда эмитить                              | Шаг-потребитель                          |
|--------------------------|--------------------------------------------|------------------------------------------|
| `canvas:note-created`    | После создания заметки двойным кликом      | `firstRunInline` / `create-note`         |
| `canvas:note-activated`  | После начала редактирования заметки       | `firstRunInline` / `create-note` (alt)   |
| `canvas:palette-opened`  | После открытия палитры Ctrl+P             | `paletteTour` / `open`                   |
| `canvas:palette-closed`  | После закрытия палитры                    | future: `paletteTour` cleanup steps      |
| `canvas:scheme-gallery-opened` | После открытия галереи схем        | `schemeGalleryTour` / `open`             |
| `canvas:scheme-preview-shown`  | После открытия превью схемы        | `schemeGalleryTour` / `preview`          |
| `canvas:scheme-applied`        | После применения схемы в активный канвас | `schemeGalleryTour` / `apply`    |
| `tour:<id>:complete`     | Эмитит сам движок по завершении сценария  | хост: телеметрия / `onboarding_done`     |

### 5.1 Текущая реализация (production)

Сигналы `canvas:palette-opened` и `canvas:note-created` эмитятся
**JS-side шимом** в `crates/canvas-web/index.html` (FR-028 v2):

- **Ctrl+P keydown** (существующий W9 шим preventDefault): после
  preventDefault, через 120ms вызывает
  `window.__canvasdeskTour.signal("canvas:palette-opened")`. WASM-side
  успевает открыть палитру (`template_ui.rs`) → tour продвигается.
- **dblclick на `body > canvas`**: листенер (ставится через
  `requestAnimationFrame` retry, т.к. winit-web вставляет canvas в DOM
  асинхронно) → `window.__canvasdeskTour.signal("canvas:note-created")`.
  False positive: dblclick на существующей ноде = edit mode; в контексте
  тура «создайте заметку» семантика корректна.

Эти сигналы **не требуют Rust-side wiring** и работают в production
сегодня (фикс блокера из предыдущей итерации).

См. `sdk/web-onboarding/tests/signal-shim-test.html` (HTML-страница с
mock canvas + встроенным шимом) и `tests/test_signal_shim.py` (Python
Playwright-раннер).

### 5.2 Rust-side signal emission (TODO — design, не реализовано)

Сигналы `canvas:scheme-gallery-opened`, `canvas:scheme-preview-shown`,
`canvas:scheme-applied` **не покрыты JS-side шимом** — нет hotkey для
scheme-gallery, нет DOM-событий для preview/apply. Требуется Rust-side
эмиссия.

**Design (контракт для будущего PR с Rust dev env):**

1. **`AppEvent::TourSignal(String)` variant** в
   `crates/canvas-app/src/app.rs::AppEvent`. Pure addition — не
   инфицировать существующие match arms; canvas-app просто шлёт
   событие наружу через `EventLoopProxy::send_event`. Это событие
   получает **внешний consumer** (canvas-web wrapper), не сам App.

2. **`pending_tour_signals: Vec<String>` field в `App`** +
   `pub fn push_tour_signal(&mut self, name: &str)` (мутатор) +
   `pub fn drain_tour_signals(&mut self) -> Vec<String>` (drain).
   Поле — `#[serde(skip)]` (не часть config.toml); для нативных
   rlib-тестов остаётся пустым (callback None → drain возвращает
   пустой Vec).

3. **Эмиссия из `App`** — вызовы `push_tour_signal` в:
   - `crates/canvas-app/src/app.rs::create_note_at` →
     `push_tour_signal("canvas:note-created")`
   - `crates/canvas-app/src/template_ui.rs` (5 мест `panel.open = true`)
     → `push_tour_signal("canvas:palette-opened")`
   - `crates/canvas-app/src/app.rs::scheme_gallery.open()` (line ~6183)
     → `push_tour_signal("canvas:scheme-gallery-opened")`
   - `crates/canvas-app/src/app.rs::apply_scheme` (line ~4787) →
     `push_tour_signal("canvas:scheme-applied")`
   - scheme preview open → `push_tour_signal("canvas:scheme-preview-shown")`

4. **Wrapper ApplicationHandler в `crates/canvas-web/src/tour_aware_app.rs`**
   (новый модуль) — оборачивает `App`, делегирует все методы
   `ApplicationHandler<AppEvent>` во внутренний App, после каждого
   `window_event` / `user_event` / `about_to_wait` вызывает
   `inner.drain_tour_signals()` и для каждого сигнала —
   `crate::tour_signal::emit(name)`.

5. **Изменение в `crates/canvas-web/src/app_spawn.rs`** — заменить
   `event_loop.spawn_app(app)` на `event_loop.spawn_app(TourAwareApp::new(app))`.

6. **Реализация `tour_signal::emit` уже готова**
   (`crates/canvas-web/src/tour_signal.rs`, commit bed667e): вызывает
   `window.__canvasdeskTour.signal(name)` через `web_sys::window()` +
   `js_sys::Reflect::get`. Gated `#[cfg(target_arch = "wasm32")]`.

**Инварианты реализации (для приёмки):**

- Native rlib-тесты остаются зелёными (callback None → drain возвращает
  пустой Vec, `tour_signal::emit` — no-op stub).
- wasm32 build не падает ( TourAwareApp реализует ApplicationHandler
  корректно, делегирование не ломает существующие обработчики).
- Существующие regression-тесты (11/11) проходят без изменений
  (сигналы эмитятся, JS-side шим остаётся как fallback для dev-сервера
  без wrapper'а).

**Альтернатива (отвергнута):** `AppEvent::TourSignal(String)` без
pending_signals — App RECEIVES события, не эмитит наружу. Не подходит,
т.к. App::create_note_at — это внутренний метод, не имеет
EventLoopProxy под рукой.

---

## 6. Инварианты

1. **Инвариант изоляции:** inline-тур не модифицирует Rust-side v1
   (`onboarding_ui.rs`, `crates/canvas-app/src/main.rs`). v1 и v2 —
   независимы; пользователь может пройти v2 без прохождения v1.
2. **Инвариант zero-deps:** vanilla bundle (`canvasdesk-tour.js`)
   работает без npm-установки и без сборщика. Источник — TS, сборка
   только при рефакторинге движка.
3. **Инвариант выхода:** Esc всегда закрывает тур (если `skippable`
   сценария не `false`); кнопка Skip видна на каждом шагу (NN/g).
4. **Инвариант `waitFor` soft-fail:** таймаут `waitFor` не ломает тур
   — Next остаётся кликабельным, движок пишет warning в log. Это
   паттерн из flows.sh: контракт проверки не должен блокировать юзера.
5. **Инвариант якорей:** selector-miss → fallback rect → null →
   tooltip в center без cutout. Деградация graceful.
6. **Инвариант refresh:** rAF-цикл пересчитывает позицию tooltip и
   cutout каждый кадр — canvas-анимация не «уезжает» от тултипа.

---

## 7. Точки расширения

### Кастомный tooltip-рендерер

`TourHooks.renderTooltip(ctx)` позволяет заменить стандартный
тултип. Engine всё равно делает positioning — нужно только вернуть
`HTMLElement`.

### Кастомный anchor-резолвер

`TourHooks.resolveAnchor(spec)` — для canvas-internal регионов без DOM.
Хост может спросить Rust-side через bridge (`canvas_web::js_glue`) и
вернуть `Rect`.

### Кастомный dim-рендерер

`TourHooks.renderDim(ctx)` — заменить SVG-cutout (например, на
WebGPU-renderer, рисующий dim прямо в канвас).

### Сторонние сценарии

Любой TS/JS-объект, удовлетворяющий `TourScenario`, можно передать в
`Tour.run()`. Это позволяет командам(CanvasDesk widgets) поставлять
свои мини-туры.

---

## 8. Тестирование

### Smoke-тест (браузер)

1. `trunk serve` (см. `scripts/web_bundle.sh`) — поднять web-shell.
2. Открыть `http://localhost:8080/`.
3. Нажать «Тур» в `#w6-toolbar` → выбрать сценарий.
4. Пройти шаги; проверить:
   - tooltip виден и не уезжает за viewport;
   - cutout подсвечивает якорь;
   - Esc закрывает тур;
   - URL `#tour=cd-toolbar-tour` запускает тур при загрузке.

### Интеграционный тест (future)

Puppeteer/Playwright:

1. Открыть `index.html`.
2. `page.click("#btn-tour")`.
3. `page.on("dialog", d => d.accept("1"))`.
4. Дождаться `.cd-tour-tooltip` с текстом «Панель хранилища».
5. `page.click(".cd-tour-btn_primary")` × N.
6. Проверить: тур закрылся, `tour:<id>:complete` signal эмитился.

См. `docs/ACCEPTANCE.md` §19 для чек-листа ручной приёмки.

---

## 9. Связанные документы

- `docs/interface-objects/onboarding.md` — v1 карусель.
- `docs/change-requests/fr-028-onboarding-carousel.md` — FR-028.
- `sdk/web-onboarding/README.md` — справочник API библиотеки.
- `crates/canvas-web/index.html` — точка интеграции (`<script>`-тег,
  кнопка «Тур»).
- `crates/canvas-app/src/onboarding_ui.rs` — Rust-реализация v1.
- `crates/canvas-web/src/toolbar.rs` — листенеры W6-тулбара
  (Rust-side кнопок `#btn-open` / `#btn-recent` / `#btn-export`).

---

## 10. История изменений

- `2026-10-03` — агент (FR-089): пикер первого запуска (та же точка
  интеграции `crates/canvas-web/index.html`) дополнен блоком согласий
  телеметрии «Помочь проекту» — два предвыбранных чекбокса (анонимный
  счётчик `wasm_used` + метрики/ошибки PostHog), RU/EN, акценты
  FR-087 v2; рядом с пикером появился модуль `window.__cdTelemetry`
  (согласия `canvasdesk.consent`, счётчик, живой opt-in/out по
  событию `canvasdesk:consent-changed`). На сам inline-тур влияния
  нет (DOM-слой пикера отделён). Детали:
  `docs/change-requests/fr-088-telemetry-consent-posthog.md`.
- `2026-09-29` — агент: добивка v2 — фикс блокера + JS-side signal
  detection + CI + документация:
  - **Inline bundle**: `canvasdesk-tour.js` (\~48 КБ) теперь inline
    внутри `crates/canvas-web/index.html` через
    `<script data-cd-tour-bundle="inline">...</script>` блок. Trunk
    больше не копирует отдельный JS-файл (хук `[[hooks]]` удалён —
    был ненадёжным, env var path / timing). Скрипт
    `scripts/inline_tour_bundle.py` регенерирует inline-блок при
    пересборке bundle из TS-источника.
  - **JS-side signal shim** (\S 5.1): Ctrl+P keydown \+ dblclick на
    `body > canvas` эмитят `canvas:palette-opened` / `canvas:note-created`
    в tour-bus. Делает `paletteTour` и `firstRunInline` рабочими в
    проде без Rust-side wiring.
  - **CI workflow** `.github/workflows/tour-tests.yml`: 11 Playwright
    regression тестов + signal-shim тест как гейт на PR.
  - **Документация** (\S 5.2): design doc для Rust-side signal emission
    (AppEvent::TourSignal + pending_signals + TourAwareApp wrapper) —
    для будущего PR с Rust dev env.
  - Tests: `sdk/web-onboarding/tests/test_signal_shim.py` +
    `signal-shim-test.html` (HTML + Python runner).
- `2026-09-26` — агент: реализация v2 inline-тура в сессии:
  TS-библиотека `sdk/web-onboarding/` (7 файлов: types, positioning,
  highlight, tooltip, actions, tour, styles + index + 4 сценария),
  pre-built vanilla bundle `canvasdesk-tour.js` (zero deps, ES2018),
  интеграция в `crates/canvas-web/index.html` (кнопка «Тур» в
  `#w6-toolbar`, URL-hash `#tour=<id>` для авто-запуска, signal-мост
  `window.__canvasdeskTour`). Документ — этот файл; README —
  `sdk/web-onboarding/README.md`. v1 (`onboarding_ui.rs`) не
  тронут — v2 alongside, не замещает.
