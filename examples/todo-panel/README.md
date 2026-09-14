# Todo Panel — пример виджета-микрофронта (T22-C)

Панель задач, работающая в двух режимах из одного bundle:

- **Виджет CanvasDesk** — задачи лежат в `props` ноды (файл `.canvas`):
  переживают перезапуск приложения, undo/redo и экспорт/импорт сцены.
  Permissions не нужны — виджет полностью офлайновый.
- **Обычная страница** (`npm run dev` или `dist-widget/index.html` в
  браузере) — SDK не находит моста (`window.chrome.webview`), переключается
  на no-op-адаптер, задачи уезжают в `localStorage`.

Разница между режимами — только адаптер персистентности
(`CanvasDesk.setProps` ↔ `localStorage`), см. `src/main.ts`.

## Сборка и установка

```
npm install
npm run typecheck
npm run pack      # → dist-widget/
```

Перетащите `dist-widget/` на канвас CanvasDesk → «Установить» →
виджет в меню «Виджеты ▸ Todo Panel». Дальше — как обычная нода:
двигается, связывается рёбрами, попадает в мини-карту.

## Что демонстрирует

| Приём | Где в коде |
|---|---|
| props-персистентность | `persist()` → `CanvasDesk.setProps({ items })` |
| разбор чужих props без падения | `fromProps()` — фильтрация мусорных элементов |
| undo-дружелюбие | одно `setProps` на действие пользователя (не на кадр) |
| standalone-фолбэк | `CanvasDesk.standalone` → `localStorage` |
| тема канваса | `onTheme` → CSS-переменные |

Справочник по мосту и permissions — `docs/WIDGETS.md`.
