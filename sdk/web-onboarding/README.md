# CanvasDesk Web Onboarding (`@canvasdesk/web-onboarding`)

Inline-тур по мотивам **Intro.js** (overlay + tooltip + highlight cutout),
**Guida.js** (сценарии как plain-data JSON) и **flows.sh** (поэтапные
действия с явным `waitFor`-гейтом).

Библиотека реализует v2 онбординга для CanvasDesk: интерактивные
сценарии поверх WebGPU-canvas. **Не заменяет** существующую карусель v1
(см. `docs/interface-objects/onboarding.md` и
`crates/canvas-app/src/onboarding_ui.rs`) — дополняет её DOM-overlay, в
котором пользователь реально выполняет действия в живом канвасе.

## Архитектура

```
sdk/web-onboarding/
├── src/
│   ├── types.ts          # публичные типы (AnchorSpec, TourStep, WaitForSpec…)
│   ├── positioning.ts    # 13 направлений + auto-flip + viewport clamp
│   ├── highlight.ts      # dim overlay с SVG-cutout (even-odd mask)
│   ├── tooltip.ts        # DOM-tooltip с caret
│   ├── actions.ts        # perform (click/focus/signal/dispatch)
│   │                     # + waitFor (selector/predicate/event/signal)
│   ├── tour.ts           # движок: state-machine, lifecycle, rAF-refresh
│   ├── styles.ts         # default CSS (CanvasDesk-палитра)
│   ├── index.ts          # публичный API
│   └── scenarios/        # демо-сценарии (toolbar, first-run, palette, calc)
├── canvasdesk-tour.js   # pre-built vanilla bundle (iife, ES2018)
├── package.json         # esbuild build pipeline
└── tsconfig.json
```

Zero runtime dependencies. Vanilla bundle (`canvasdesk-tour.js`)
загружается через `<script>`-тег без сборщика (используется в
`crates/canvas-web/index.html`). Источник — TypeScript в `src/`;
пересборка через `npm run build` (esbuild).

## API

### Quickstart (vanilla)

```html
<script src="canvasdesk-tour.js"></script>
<script>
  const tour = new CanvasDeskTour.Tour();
  // Демо-сценарии уже встроены:
  tour.run(CanvasDeskTour.scenarios.toolbarTourScenario);
</script>
```

### Quickstart (Vite/TypeScript)

```ts
import { Tour } from "@canvasdesk/web-onboarding";

const tour = new Tour();
tour.run(myScenario);
```

### Tour

```ts
class Tour {
  constructor(opts?: TourOptions);
  run(scenario: TourScenario, opts?: RunOptions): TourHandle | null;
  onSignal(listener: (name: string, payload?: unknown) => void): () => void;
  signal(name: string, payload?: unknown): void;
  refresh(): void;
  // persistence (localStorage-backed):
  isCompleted(id: string): boolean;
  getCompletedAt(id: string): number | null;
  markCompleted(id: string): void;
  resetCompleted(id: string): void;
  getResumeIndex(id: string): number | null;
  clearResume(id: string): void;
  static instance: Tour | null;
}
```

### RunOptions

```ts
interface RunOptions {
  skipIfCompleted?: boolean;       // don't run if isCompleted(id) — default false
  resume?: boolean;                 // start from saved step index — default false
  markCompletedOnSkip?: boolean;   // skip also marks completed — default false
}
```

### TourOptions

```ts
interface TourOptions {
  container?: HTMLElement;          // default document.body
  styles?: string;                 // inline CSS (DEFAULT_STYLES used if omitted)
  baseZIndex?: number;             // default 10000
  log?: (level, message) => void;  // default console
  storageKey?: string | null;      // localStorage prefix, default "cd-tour:"
                                   // set to null to disable persistence
}
```

### TourScenario

```ts
interface TourScenario {
  id: string;            // stable id; localStorage key
  name: string;
  steps: TourStep[];
  skippable?: boolean;          // default true
  primaryLabel?: string;        // default "Далее"
  skipLabel?: string;           // default "Пропустить"
  doneLabel?: string;           // default "Готово"
  backLabel?: string;           // default "Назад"
  autoAdvance?: boolean;        // default false
  onStart?(tour: TourHandle): void;
  onComplete?(tour: TourHandle): void;
  onSkip?(tour: TourHandle): void;
  onStep?(step: TourStep, index: number, tour: TourHandle): void;
}
```

### TourStep

```ts
interface TourStep {
  id?: string;
  title: string;
  body?: string;
  anchor?: AnchorSpec;          // где подсветить
  side?: TooltipSide;           // куда положить tooltip
  highlight?: HighlightOptions; // null → без cutout
  perform?: PerformSpec[];      // действия при активации шага
  waitFor?: WaitForSpec;        // гейт: Next disabled до resolve
  primaryLabel?: string;
  passive?: boolean;            // без Next (только waitFor/advanceOnClick)
  hideSkip?: boolean;
  advanceOnClick?: string | HTMLElement;
  meta?: Record<string, unknown>;
}
```

### Anchor — к чему крепится tooltip

```ts
type AnchorSpec =
  | { kind: "selector"; selector: string; fallback?: Rect | null }
  | { kind: "element"; element: HTMLElement }
  | { kind: "rect"; rect: Rect };  // для canvas-internal регионов
```

### WaitFor — gate progression (flows.sh)

```ts
type WaitForSpec =
  | { kind: "selector"; selector: string; timeout?: number }
  | { kind: "predicate"; predicate: () => boolean; timeout?: number }
  | { kind: "event"; target: "document" | "window" | HTMLElement;
      eventType: string; timeout?: number }
  | { kind: "signal"; signals: string[]; timeout?: number };
```

### Perform — выполнить действие (Intro.js actions)

```ts
type PerformSpec =
  | { kind: "click"; target: "selector" | "element";
      selector?: string; element?: HTMLElement }
  | { kind: "focus"; selector?: string; element?: HTMLElement }
  | { kind: "signal"; signal: string; payload?: unknown }
  | { kind: "dispatch"; target: "selector" | "element";
      selector?: string; element?: HTMLElement;
      eventType: string; init?: EventInit };
```

## Сигнальный мост

Главный механизм интеграции с WASM-хостом — **сигналы**. Движок
выставляет `window.__canvasdeskTour = tour` (см.
`crates/canvas-web/index.html`), WASM-bridge вызывает:

```js
window.__canvasdeskTour.signal("canvas:note-created", { id: "n_1" });
```

Сценарий в `waitFor: { kind: "signal", signals: ["canvas:note-created"] }`
продвигается дальше.

См. `firstRunInlineScenario` (`src/scenarios/first-run-inline.ts`) — шаг
«Создайте первую заметку» ждёт `canvas:note-created`.

## Демо-сценарии

| Идентификатор          | Сценарий                         | Скоуп                                              |
|------------------------|----------------------------------|----------------------------------------------------|
| `cd-toolbar-tour`      | Тур по панели хранилища (W6)     | `#btn-open`, `#btn-recent`, `#btn-export`          |
| `cd-first-run-inline`  | Интерактивный первый запуск      | Канвас целиком, `waitFor(signal)` для note-created |
| `cd-palette-tour`      | Палитра шаблонов (FR-018)        | Ctrl+P, колесо категорий, Shift+клик               |
| `cd-calculations-tour` | Numi-формулы (FR-013/014/015)    | Переменные, единицы, value-flow, what-if, MC       |
| `cd-scheme-gallery-tour` | Галерея схем (FR-049)         | 8 шагов, 3 passive+waitFor (open/preview/apply)   |

## Запуск

### Standalone smoke test (без Rust/WASM)

`sdk/web-onboarding/standalone-test.html` — отдельная HTML-страница для
ручной проверки движка. Грузит `canvasdesk-tour.js`, эмулирует
mock-панель хранилища (#w6-toolbar с кнопками) и предлагает кнопки
запуска каждого сценария + ручную отправку сигналов в tour-bus.

Открыть можно двумя способами:

```sh
# Вариант 1: file:// (browser открывает напрямую)
$BROWSER sdk/web-onboarding/standalone-test.html

# Вариант 2: через http-сервер (для тестирования fetch и т.п.)
cd sdk/web-onboarding
python3 -m http.server 8090
# → http://localhost:8090/standalone-test.html
```

### Vanilla (canvas-web `index.html`)

Кнопка «Тур» в `#w6-toolbar` открывает **dropdown-меню** со списком
сценариев (раньше был `prompt()` — заменили на стилизуемый DOM-menu
с aria-haspopup / role=menu / Esc для закрытия). URL-hash
`#tour=cd-toolbar-tour` запускает автоматически — точка входа для
deeplink-онбординга из маркетинговых писем или подсказок.

### Vite/TS (виджет)

Импорт типа:

```ts
import { Tour, scenarios } from "@canvasdesk/web-onboarding";
import type { TourScenario, TourStep } from "@canvasdesk/web-onboarding";
```

## Расширение

### Кастомный tooltip

```ts
const tour = new Tour({
  hooks: {
    renderTooltip: (ctx) => {
      const el = document.createElement("div");
      // ... своя разметка
      return el;
    },
  },
});
```

### Кастомный anchor (canvas-internal)

Для canvas-internal элементов (вне DOM) хост может зарегистрировать
резолвер:

```ts
const tour = new Tour({
  hooks: {
    resolveAnchor: (spec) => {
      if (spec.kind === "selector" && spec.selector.startsWith("canvas:")) {
        // Спросить Rust-side через bridge: вернуть rect из spatial.rs
        const rect = window.__canvasdesk_queryRect(spec.selector);
        return rect ? { kind: "rect", rect } : null;
      }
      return null;
    },
  },
});
```

## Сборка

```sh
cd sdk/web-onboarding
npm install
npm run build       # → canvasdesk-tour.js
npm run minify      # → canvasdesk-tour.min.js
npm run typecheck   # tsc --noEmit
```

## Тестирование

### Smoke (ручной)

Открыть `crates/canvas-web/index.html` после `trunk serve` (см.
`scripts/web_bundle.sh`), нажать «Тур», выбрать сценарий, пройти
шаги. Esc — выход. URL `#tour=cd-toolbar-tour` — авто-запуск.

### Standalone HTML (без Rust/WASM)

`sdk/web-onboarding/standalone-test.html` — отдельная страница с mock
toolbar + кнопками запуска сценариев + manual signal-emit buttons.
Открывается напрямую через `file://` или `python3 -m http.server 8090`.

### Регрессионные тесты (Playwright)

`sdk/web-onboarding/tests/test_tour_regression.py` — 11 автоматических
тестов через headless Chromium, покрывают:

| Тест                                | Что проверяет                                       |
|-------------------------------------|-----------------------------------------------------|
| `toolbar_tour_full_flow`            | 5 шагов + Done → tour:complete signal              |
| `passive_step_auto_advances_on_signal` | passive+waitFor auto-advance + dim disabled      |
| `palette_tour_passive_advance`      | canvas:palette-opened → step 3                     |
| `calculations_tour_navigation`      | 7 шагов tooltip сверху на rect-anchor              |
| `deeplink_url_hash_auto_launches`   | #tour=cd-toolbar-tour → auto-start                 |
| `deeplink_unknown_id_warns`         | неизвестный id → console.warn, no tooltip         |
| `esc_skip_closes_tour`              | Esc → skip → unmount                               |
| `back_button_navigation`            | Back → step N-1                                    |
| `arrow_keys_navigation`             | ArrowRight/Left → next/back                       |
| `persistence_completion`            | isCompleted после Done; skipIfCompleted refuses    |
| `persistence_resume`                | Esc на шаге 2 → resumeIndex=1; run(resume) → шаг 2 |

Запуск:

```sh
python3 sdk/web-onboarding/tests/test_tour_regression.py
```

CI: тест можно подключить в `.github/workflows/` через
`pip install playwright && playwright install chromium` + запуск
скрипта.

См. `docs/ACCEPTANCE.md` §19 для v2 чек-листа.

## Связанные документы

- `docs/interface-objects/onboarding.md` — v1 карусель (Rust-side).
- `docs/interface-objects/onboarding-v2-inline.md` — v2 (эта
  библиотека, см. ниже).
- `docs/change-requests/fr-028-onboarding-carousel.md` — изначальный FR.
- `crates/canvas-app/src/onboarding_ui.rs` — Rust-реализация v1.
- `crates/canvas-web/index.html` — точка интеграции в web-shell.

## Лицензия

MIT (см. `LICENSE` в корне репозитория).
