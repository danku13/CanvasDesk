# FR-091: Session replay — видимость WASM-слоя (захват wgpu-канваса)

- **Статус:** выполнено (2026-10-03)
- **Тип:** FR
- **Приоритет:** желательно
- **Владелец:** danku13
- **Источник:** диалог с владельцем (сессия 2026-10-03): «сейчас при записи сеанса я вижу только то что происходит на web слое, но не вижу ничего на wasm слое, как это исправить?»
- **Связанные задачи:** FR-089 (телеметрия/PostHog/согласия), FR-090 (события из Rust, `surface_opened`), M8 (wasm-port, wgpu-рендер)
- **Создан:** 2026-10-03
- **Обновлён:** 2026-10-03

## 1. Описание (What)

В записях сеансов PostHog (session replay) был виден только web-слой —
DOM-пикер первого запуска; после входа в приложение экран оставался
пустым. Причина архитектурная: весь UI CanvasDesk рисуется Rust/wgpu в
**один `<canvas>`** (WebGPU; winit создаёт, canvas-web вставляет в body
— `attach_canvas_to_dom`), а запись сеанса — это rrweb-снапшоты **DOM**
(мутации, инпуты, курсор). Внутрь канваса rrweb не смотрит; захват
содержимого канвасов у PostHog **выключен по умолчанию** (канвас не
покрывается DOM-маскировкой — осознанное приватность-решение платформы).
Продуктовые события FR-090 (`surface_opened`, `role_changed`, …) при
этом в таймлайне реплея уже есть — «слепота» касалась именно
визуального слоя.

Фикс — включить canvas capture в `posthog.init` (локальный конфиг):

```js
session_recording: {
  captureCanvas: {        // приоритет над настройками проекта
    recordCanvas: true,   // включить захват канвасов
    canvasFps: 4,         // 0–12 (дефолт PostHog 4)
    canvasQuality: "0.4"  // WebP-качество "0"–"1"
  },
  canvasCapture: {
    resolutionScale: 0.6  // 0.1–1 (рекомендация PostHog, defaults 2026-05-30)
  }
}
```

## 2. Влияние (Impact)

| Объект | Изменение | Документация |
|---|---|---|
| `canvas-web/index.html` | `posthog.init` + `session_recording` (captureCanvas/canvasCapture) с комментарием | этот документ §4 |
| Rust | без изменений (события поверхностей уже FR-090) | — |
| Натив (десктоп) | без изменений: слой телеметрии — web-only (FR-089) | — |
| PostHog (UI проекта) | опционально: Settings → Session replay → включить canvas recording — тот же эффект на уровне проекта; локальные опции `captureCanvas` имеют приоритет | §6 |

## 3. Анализ (Why) — механика, проверенная по исходникам SDK

Версия SDK: `posthog-js@1` → **1.435.8** (актуальная на npm), чанки
`lazy-recorder.js`/`recorder.js` (EU cloud, `eu.i.posthog.com/static`).
Ключевые факты из кода (минифицированные артефакты разобраны вручную):

1. **Разрешение конфига** (геттер в `lazy-recorder.js`):
   `enabled = local.captureCanvas.recordCanvas ?? remote.canvasRecording.enabled`
   (локальный `posthog.init` **выигрывает** у настроек проекта; fps:
   `local.canvasFps ?? remote.fps ?? 4`; quality аналогично `?? "0.4"`,
   clamp fps 0–12, resolutionScale 0.1–1).
2. **Сборка опций rrweb**: при `enabled` → `recordCanvas: true`,
   `sampling.canvas = fps`, `dataURLOptions = {type: image/webp,
   quality}`, `canvasResolutionScale`, `canvasMasking` (из
   `canvasCapture.maskRegionsFn`).
3. **Режим FPS-обсервера** (числовой fps, а не `"all"`): каждые
   `1000/fps` мс собирает **ВСЕ `<canvas>` в DOM**
   (`querySelectorAll("canvas")` + shadow roots) — фильтрации по типу
   контекста нет; для `webgl/webgl2` дополнительно очищает color buffer
   (если нет `preserveDrawingBuffer`), для прочих — сразу
   `createImageBitmap(canvas, {resizeWidth, resizeHeight,
   resizeQuality: "medium"})` → encode-воркер → WebP → событие
   `canvasMutation` (реплей = `drawImage`). **WebGPU-канвас
   читается как растр последнего презентованного кадра** — тот же путь,
   что у безконтекстного канваса; `getContext("webgpu")` патчится
   (ставит `__context="webgpu"`), но из захвата канвас не исключается.
4. **Требования**: canvas capture в SDK с **v1.105.7** (маскировка
   `maskRegionsFn` — с v1.408); полноценно работает в Chromium (нужен
   `OffscreenCanvas` + encode worker — Chrome/Edge; веб-версия
   CanvasDesk и так требует Chrome/Edge).
5. **Гейт согласия сохраняется**: SDK грузится только при
   `analytics=true` (FR-089) — записи с канвасом есть только у
   согласившихся; отказавшимся не отправляется ничего.

Риски/границы:

- **4 fps** — панорамы и медленные правки видны отлично; между снапшотами
  промежуточные кадры теряются (осознанный компромисс платформы; для
  «видео-качества» fps поднимается до 12 ценой аплоада).
- **Privатность**: содержимое канваса (= заметки пользователя) попадает
  в записи. Митигации: согласие FR-089 (гейт), `maskRegionsFn`
  (регионы пикселей закрашиваются до кодирования, не покидают браузер),
  отключение тумблером в табе «Профиль» → SDK opt-out. Владельцу стоит
  обновить формулировку чекбокса «Метрики и отчёты об ошибках» (упомянуть
  запись экрана) — продуктовое решение, в код не вносилось (см. §6).
- **Payload**: полноэкранный канвас в device-пикселях без даунскейла —
  тяжёлые кадры; `resolutionScale: 0.6` снапшотит по CSS-размеру
  (`clientWidth/Height` × scale) — рекомендованный PostHog дефолт.

## 4. Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Конфиг реплея | `crates/canvas-web/index.html` (`loadPostHog` → `posthog.init`) | `session_recording: {captureCanvas: {recordCanvas: true, canvasFps: 4, canvasQuality: "0.4"}, canvasCapture: {resolutionScale: 0.6}}` + комментарий (приоритет локальных опций, механика FPS-обсервера, приватность, маскировка) |
| Доки | этот документ; индекс CR (ревизия 8); worklog | — |

## 5. Точки входа (Entry Points)

- FR-089 §7 changelog — перекрёстная ссылка (канвас-захват поверх того же SDK-гейта).
- FR-090 §4 таксономия — события уже в таймлайне реплея (правая панель
  плеера, синхронизация по времени).

## 6. Проверка (Verification)

- [x] Конфиг-механика подтверждена разбором артефактов SDK v1.435.8:
      main-бандл (jsDelivr `+esm`), `lazy-recorder.js`,
      `recorder.js` (EU cloud): приоритет `captureCanvas` над remote,
      сборка rrweb-опций, FPS-обсервер без фильтра по типу контекста,
      `createImageBitmap`-снапшоты → WebP-воркер.
- [x] Документация PostHog: canvas recording (поддержка с v1.105.7, 2D
      и WebGL; включение из replay-настроек проекта; 4 fps по умолчанию;
      `maskRegionsFn` с v1.408) — docs/session-replay/canvas-recording.
- [x] Стенд-проверка (agent-browser, локальный http-сервер, тестовая
      копия index.html с инструментацией: seed согласий FR-089, снятие
      bot-фильтра webdriver/HeadlessChrome по прецеденту FR-090 §6,
      шпион `createImageBitmap`/fetch/XHR/sendBeacon, полноэкранный
      канвас с рендер-циклом): SDK инициализируется с новым конфигом
      без ошибок консоли; lazy-recorder загружен
      (`__PosthogExtensions__.initSessionRecording`); FPS-обсервер
      вызывает `createImageBitmap` на канвасе ровно с ~4 fps (514
      вызовов за ~90 с); флеш на visibilitychange → `POST
      https://eu.i.posthog.com/s/` **200**, тело **16 КБ** (DOM-снапшот
      тестовой страницы дал бы 1–3 КБ — кадры канваса в payload);
      события `/e/` + beacon `/i/v0/e/` — 200. WebGPU-адаптер в
      headless-браузере агента недоступен (`requestAdapter` → null) —
      стенд прогнан на 2d-фолбэке; контекст-агностичность для webgpu
      подтверждена исходниками recorder.js (§3) — путь захвата
      идентичен. Запись тестовой сессии (distinct_id `fr091-stand-*`)
      можно открыть в PostHog как ручную приёмку: реплей показывает
      «видео» канваса.
- [x] Синтаксис 9 inline-скриптов index.html — `node --check`.
- [ ] **cargo-гейты — не требуются** (изменение только в web-слое,
      Rust не тронут). CI проходит по обычному расписанию.

Владельцу в PostHog (опционально):

- Settings → Session replay → canvas recording — включить на уровне
  проекта (тот же эффект + покрытия ретро-активности; локальные опции
  имеют приоритет — для управления из UI убрать `captureCanvas`-блок из
  `posthog.init`).
- Ретроспективы не будет: канвасы начнут попадать в записи только
  после выката новой index.html (старые записи остаются «пустыми» — это
  ограничение записи, не плеера).

## 7. История изменений (Changelog)

- 2026-10-03 — агент (Super Z, сессия web-f324f377) — реализация:
  `session_recording` (captureCanvas/canvasCapture) в `posthog.init`,
  комментарий с механикой и приватностью; CR-документ, индекс ревизия 8;
  верификация по исходникам SDK v1.435.8 + браузерный стенд.

## 8. Источники истины (References)

- `crates/canvas-web/index.html` — блок `session_recording` в `posthog.init`.
- posthog-js v1.435.8: `lazy-recorder.js` (резолв конфига, сборка
  rrweb-опций), `recorder.js` (`initCanvasFPSObserver`,
  `createImageBitmap`-снапшоты, encode-воркер).
- PostHog docs: Session replay → Canvas recording (v1.105.7+, 4 fps,
  replay settings, `maskRegionsFn` v1.408+).
- FR-089 (согласия/гейт SDK), FR-090 (события поверхностей в таймлайне).
