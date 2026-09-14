# Шаблон виджета CanvasDesk

Минимальный Vite + TypeScript-проект, собирающийся в пакет виджета.
Полный справочник (манифест, bridge, permissions, LOD) — `docs/WIDGETS.md`.

## Быстрый старт

```
npm install
npm run dev        # разработка в браузере (standalone-режим SDK)
npm run typecheck  # проверка типов (vite build её НЕ делает)
npm run pack       # сборка пакета → dist-widget/
```

`dist-widget/` содержит `widget.json`, `index.html` и ассеты — перетащите
эту папку на канвас CanvasDesk, подтвердите установку, виджет появится
в точке дропа и в меню «Виджеты ▸».

## Что где

| Путь | Что это |
|---|---|
| `public/widget.json` | манифест пакета — id, name, version, defaultSize, permissions |
| `index.html` | разметка + CSP. `base: "./"` в vite.config.ts обязателен |
| `src/main.ts` | демо-виджет «счётчик»: init/onProps/onTheme, setProps, toast |
| `src/canvasdesk.ts` | вендоренная копия SDK (`sdk/canvasdesk.ts`), обновляется из корня |

## Чек-лист перед pack

1. `widget.json`: уникальный `id` (`[a-z0-9.-]`, 3–64 символа), читаемый `name`.
2. `defaultSize` в пределах 160..2000 (иначе манифест отвергнется).
3. Перечислены только нужные `permissions` — лишние не запрашивайте.
4. `npm run typecheck` зелёный.
5. В standalone (`npm run dev`) виджет не падает без моста.

## Идея «микрофронта»

SDK сам детектит отсутствие `window.chrome.webview`: в браузере уведомления
становятся no-op, запросы отвергаются внятной ошибкой. Тот же bundle
работает и виджетом, и обычной страницей — см. `examples/todo-panel`.
