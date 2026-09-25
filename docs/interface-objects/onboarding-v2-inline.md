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
| `tour:<id>:complete`     | Эмитит сам движок по завершении сценария  | хост: телеметрия / `onboarding_done`     |

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

- `2026-09-26` — агент: реализация v2 inline-тура в сессии:
  TS-библиотека `sdk/web-onboarding/` (7 файлов: types, positioning,
  highlight, tooltip, actions, tour, styles + index + 4 сценария),
  pre-built vanilla bundle `canvasdesk-tour.js` (zero deps, ES2018),
  интеграция в `crates/canvas-web/index.html` (кнопка «Тур» в
  `#w6-toolbar`, URL-hash `#tour=<id>` для авто-запуска, signal-мост
  `window.__canvasdeskTour`). Документ — этот файл; README —
  `sdk/web-onboarding/README.md`. v1 (`onboarding_ui.rs`) не
  тронут — v2 alongside, не замещает.
