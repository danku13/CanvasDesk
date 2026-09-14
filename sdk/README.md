# CanvasDesk Widget SDK

Типизированная обёртка над widget-bridge (SPEC §7.6) — единственная точка
интеграции виджета с хостом. Без зависимостей, 131 строка.

| Файл | Назначение |
|---|---|
| `canvasdesk.ts` | **источник** — ES-модуль с типами. Импортируется в Vite/TS-проектах: `import { CanvasDesk } from "./canvasdesk"` |
| `canvasdesk.js` | сборка для script-тега (формат iife, ES2018): устанавливает глобал `window.CanvasDesk`. Для виджетов без сборщика (vanilla HTML/JS) |

Полный справочник API, протокол моста и permissions —
[`docs/WIDGETS.md`](../docs/WIDGETS.md).

## Подключение

**В проекте со сборщиком (Vite/TypeScript):** скопируйте `canvasdesk.ts` в
`src/` (шаблон `sdk/widget-template/` уже так делает) и импортируйте:

```ts
import { CanvasDesk } from "./canvasdesk";

CanvasDesk.init((data) => render(data.props, data.theme));
CanvasDesk.onProps((props) => render(props));
CanvasDesk.setProps({ count: 42 });
```

**Без сборщика (vanilla):** положите `canvasdesk.js` рядом с `index.html`:

```html
<script src="canvasdesk.js"></script>
<script src="app.js"></script>
```

```js
CanvasDesk.init(function (data) { render(data.props, data.theme); });
```

## Сборка `canvasdesk.js`

Файл генерируется из `canvasdesk.ts` (не редактируется руками):

```
npx esbuild canvasdesk.ts --format=iife --target=es2018 --outfile=canvasdesk.js \
  '--banner:js=// Сгенерировано из canvasdesk.ts: npx esbuild canvasdesk.ts --format=iife --target=es2018 --outfile=canvasdesk.js (см. sdk/README.md). Не редактировать руками.'
```

Копии SDK в репозитории (встроенные виджеты `assets/widgets/*/canvasdesk.js`,
шаблон и примеры `src/canvasdesk.ts`) сверяются с источником побайтово
тестом `crates/canvas-widgets/tests/repo_packages.rs` — после правки SDK
пересоберите `canvasdesk.js` и обновите все копии, иначе CI покраснеет.

## Standalone-режим

Если `window.chrome.webview` отсутствует (страница открыта как обычный сайт),
SDK переключается на no-op-адаптер: уведомления (`setProps`, `toast`, …)
молча теряются, запросы (`readDir`, `stateGet`, `stateSet`) отвергаются с
ошибкой `CanvasDesk: мост недоступен (standalone-режим)`. Тот же bundle
работает и как виджет, и как страница — «микрофронтенд» (пример:
`examples/todo-panel`).
