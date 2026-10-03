# FR-091: Session replay — видимость WASM-слоя (захват wgpu-канваса)

- **Статус:** выполнено (2026-10-03); **v2-доработка** (2026-10-03, тот же день): «по последним записям в posthog пока не видно, чтобы wasm часть была видна в replay» — конфигурация записи была верной, но сам канвас нечитаем для рекордера (см. §3а)
- **Тип:** FR
- **Приоритет:** желательно
- **Владелец:** danku13
- **Источник:** диалог с владельцем (сессия 2026-10-03): «сейчас при записи сеанса я вижу только то что происходит на web слое, но не вижу ничего на wasm слое, как это исправить?»; v2: «по последним записям в posthog пока не вижу чтобы wasm часть была видна в replay, так же отображаются только html части»
- **Связанные задачи:** FR-089 (телеметрия/PostHog/согласия), FR-090 (события из Rust, `surface_opened`), M8 (wasm-port, wgpu-рендер)
- **Создан:** 2026-10-03
- **Обновлён:** 2026-10-03 (v2)

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

## 3а. v2 — почему записи всё ещё показывали только HTML (2026-10-03)

После выката конфигурации §1 записи появились, но канвас в реплее остался невидимым. Живой эксперимент на проде (headless-браузер, инструментированный `createImageBitmap` + согласие `analytics=true`) дал точный диагноз:

1. **Захват был активен и снапшотил канвас**: FPS-обсервер вызывал `createImageBitmap` на канвасе 1280×800 с частотой ≈4.6 кадра/с (115 вызовов за ~25 с), bitmap 768×480 (remote date-default `resolutionScale 0.6` действует, локальное значение игнорируется).
2. **Каждый снапшот был 100% прозрачным** (`nzA=0/nzRGB=0` из 16 000 сэмплов) — при том что композитор показывал отрендеренный кадр (47% пикселей вне фона на скриншоте того же момента). Прямой `toDataURL` — тот же результат (WebP 3 КБ — пустой).
3. **Корень (WebGL2-путь)**: контекст создан wgpu с дефолтным `preserveDrawingBuffer:false` → после композитинга браузер очищает drawing buffer → любой readback (`createImageBitmap`/`toDataURL`) возвращает пустой растр. Сам PostHog патчит `HTMLCanvasElement.prototype.getContext` и форсит флаг — но патч ставится после цепочки «SDK-импорт → /decide → lazy-recorder.js → record()» и проигрывает гонку буту WASM (wgpu создаёт контекст раньше): маркер `__context` на канвасе отсутствовал, атрибут — `preserveDrawingBuffer:false`.
4. **Корень (WebGPU-путь — браузер владельца с реальным GPU)**: растр webgpu-канваса PostHog не читает ВООБЩЕ — у WebGPU нет аналога `preserveDrawingBuffer` (posthog/posthog#57008, открытый issue; их документация заявляет «works in 2D and WebGL» без WebGPU).

### Фикс v2 — два независимых механизма

| # | Где | Что | Закрывает |
|---|---|---|---|
| 1 | `canvas-web/index.html` (getContext-шим, §4) | `preserveDrawingBuffer:true` для `webgl/webgl2/experimental-webgl` — ставится синхронно в `<head>` ДО любых `getContext` (wgpu, gpu-gate-проба), перманентно выигрывая гонку у патча рекордера | WebGL2-путь (в т.ч. фолбэк headless/старых GPU) |
| 2 | `canvas-render` (`set_prefer_gl_for_capture`) + `canvas-web/renderer_launch` | при согласии `analytics=true` (JSON `canvasdesk.consent`, семантика index.html) `create_gpu_web` пропускает WebGPU-ступень — сразу GL (WebGL2), читаемый для рекордера | WebGPU-путь (браузеры с реальным GPU) |

Правило §3.1 соблюдено: согласия читает web-слой (canvas-web), ядро (canvas-render) получает абстрактный флаг «захват канваса извне» и не знает про PostHog. Отказавшимся от аналитики WebGPU остаётся (запись не идёт — компромиссов нет); смена согласия действует после перезагрузки (выбор бэкенда разовый, при старте). Цена WebGL2+preserveDrawingBuffer — сохранённый drawing buffer; рендер не деградировал (§6: 47% пикселей до/после — идентично).

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
| Конфиг реплея (v1) | `crates/canvas-web/index.html` (`loadPostHog` → `posthog.init`) | `session_recording: {captureCanvas: {recordCanvas: true, canvasFps: 4, canvasQuality: "0.4"}, canvasCapture: {resolutionScale: 0.6}}` + комментарий (приоритет локальных опций, механика FPS-обсервера, приватность, маскировка; v2 — исправлены неверные утверждения: локальный resolutionScale игнорируется, webgpu-канвас НЕ читается) |
| v2: getContext-шим | `crates/canvas-web/index.html` (новый `<script>` после WebGPU-limits-шима) | патч `HTMLCanvasElement.prototype.getContext`: `webgl/webgl2/experimental-webgl` → `preserveDrawingBuffer:true` (мерж в opts), ставится синхронно в `<head>` до любых getContext |
| v2: capture-режим рендера | `crates/canvas-render/src/renderer.rs` | wasm32-only: `static PREFER_GL_FOR_CAPTURE` + `pub fn set_prefer_gl_for_capture(bool)`; `create_gpu_web` при флаге пропускает WebGPU-ступень (новый прямой GL-путь `create_gpu_web_gl`, общий хвост двухступенчатого пути) |
| v2: чтение согласия | `crates/canvas-web/src/renderer_launch.rs` | `analytics_recording_active()` (localStorage `canvasdesk.consent`, семантика index.html: нет записи+конфига → false — свежий визит; нет поля/битая → true — дефолты чекбоксов) → `set_prefer_gl_for_capture` до `spawn_local` |
| Доки | этот документ (§3а/§4/§6/§7/§8); worklog | — |

## 5. Точки входа (Entry Points)

- FR-089 §7 changelog — перекрёстная ссылка (канвас-захват поверх того же SDK-гейта).
- FR-090 §4 таксономия — события уже в таймлайне реплея (правая панель
  плеера, синхронизация по времени).

## 6. Проверка (Verification)

### v1 (2026-10-03, утром)

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
      без ошибок консоли; lazy-recorder загружен; FPS-обсервер вызывает
      `createImageBitmap` ~4 fps (514 вызовов за ~90 с); флеш на
      visibilitychange → `POST https://eu.i.posthog.com/s/` **200**;
      синтаксис 9 inline-скриптов — `node --check`.
- [x] cargo-гейты v1 — не требовались (изменение только в web-слое).

### v2 (2026-10-03, после обеда)

- [x] **Диагноз на живом прод-сайте** (b99777f→66d42445, headless,
      consent `analytics=true`): проба `createImageBitmap` — 115
      снапшотов/25 с (захват активен), каждый 100% пуст
      (`nzA=0/nzRGB=0`), атрибут контекста `preserveDrawingBuffer:false`,
      композитор при этом показывает рендер (47% пикселей) —
      воспроизведён баг владельца «только html части».
- [x] **Фикс-гипотеза проверена на реальном приложении без пересборки**
      (тестовая страница: live index.html + шим в `<head>` + абсолютные
      URL ассетов 66d42445, локальный http-сервер, GitHub Pages CORS):
      атрибут `preserveDrawingBuffer:true`, 107 снапшотов/25 с,
      снапшоты **полностью непустые** (`nzA=16000/nzRGB=16000` из
      16 000 сэмплов, 768×480) — контент канваса пошёл в пайплайн
      записи; рендер не деградировал (47% до/после — идентично),
      ошибок страницы нет.
- [x] **Гейты локально** (тулчейн 1.99, как в CI): `cargo fmt` — чисто;
      `cargo check` canvas-render + canvas-web — натив и wasm32 — чисто;
      `cargo clippy -- -D warnings` — оба таргета — 0; `cargo test` —
      402 passed/0 failed (canvas-render) + canvas-web зелёные.
- [x] Синтаксис inline-скриптов index.html — `node --check` (включая
      новый шим-блок).
- [ ] **Live-приёмка после выката v2** (этот пуш): повтор пробы на
      https://danku13.github.io/CanvasDesk/app/ — снапшоты непустые,
      консоль показывает пропуск WebGPU-ступени при analytics=true;
      владельцу: новая сессия с «Метриками» → Replays → канвас виден
      (в т.ч. в браузере с реальным GPU — через WebGL2-бэкенд).

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
- 2026-10-03 (v2) — агент (Super Z, та же сессия) — доработка по
  фидбэку владельца «в replay только html части»: диагноз live-экспериментом
  (снапшоты пустые из-за preserveDrawingBuffer:false у wgpu-контекста
  + webgpu нечитаем вовсе, posthog#57008); фикс — getContext-шим
  index.html + `set_prefer_gl_for_capture` (согласие → WebGL2-бэкенд);
  фиксированы неверные утверждения v1 в комментариях и этом документе
  (webgpu-растр, локальный resolutionScale); эмпирическая приёмка до/после
  на реальном приложении; гейты fmt/check/clippy/test натив+wasm32.

## 8. Источники истины (References)

- `crates/canvas-web/index.html` — блок `session_recording` в `posthog.init`
  + getContext-шим (FR-091 v2).
- `crates/canvas-web/src/renderer_launch.rs` — `analytics_recording_active`,
  вызов `set_prefer_gl_for_capture` (FR-091 v2).
- `crates/canvas-render/src/renderer.rs` — `PREFER_GL_FOR_CAPTURE`/
  `set_prefer_gl_for_capture`, `create_gpu_web`/`create_gpu_web_gl`/
  `create_gpu_web_two_step` (FR-091 v2).
- posthog-js v1.435.8: `lazy-recorder.js` (резолв конфига, сборка
  rrweb-опций), `recorder.js` (`initCanvasFPSObserver`,
  `createImageBitmap`-снапшоты, encode-воркер, патч getContext с
  форсированием preserveDrawingBuffer).
- PostHog docs: Session replay → Canvas recording (v1.105.7+, 2D и
  WebGL; WebGPU отсутствует) — docs/session-replay/canvas-recording.
- PostHog issue #57008 «Session recording support for webgpu-backed
  canvas» — webgpu-канвасы не попадают в replay (нет аналога
  preserveDrawingBuffer, gpuweb#2743).
- FR-089 (согласия/гейт SDK), FR-090 (события поверхностей в таймлайне).
