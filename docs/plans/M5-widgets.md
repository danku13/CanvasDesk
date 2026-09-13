# M5. Движок виджетов (JS/HTML-микрофронтенды) — план T20–T22

ЗАДАЧА: M5 целиком по SPEC §7.6 / TASKS T20–T22: WebView2-рантайм, bridge,
манифест, permissions, установка, встроенные виджеты, SDK и документация
разработчика. Порядок внутри milestone строгий: T20 → T21 → T22; каждая задача —
отдельный коммит (сессия = задача = коммит, AGENTS.md). После M5 — приёмка
владельцем по SPEC §10 и тег v1.1.

Источники истины (агент сверяется с ними, не с памятью):
- `docs/SPEC.md` §7.6 (движок виджетов), §5.1 (формат `canvasdesk`-ноды),
  §6.2 (LOD), §8 (ввод), §9 (риски), §10 (критерии релиза);
- `docs/TASKS.md` T20–T22 (промпты и критерии);
- `AGENTS.md` «Правила безопасности и виджетов (M5)»;
- `docs/RECIPES.md` — строки 29 и 149: WebView2 после репарентинга в
  WorkerW/Progman не получает корректный DPI и событие его смены (Lively/Seelen)
  → для виджетов обязательно пересчитывать rect по собственному DPI-поллингу
  (R10), что уже делает `App::scale_factor()` в desktop-режиме;
- `docs/interface-objects/node.md` — строка виджет-ноды;
- Web-справка WebView2 (SPEC §11): `ICoreWebView2` (postMessage,
  CapturePreview, WebResourceRequested), `ICoreWebView2Controller`
  (Bounds/ZoomFactor/TrySuspend/accelerator), `SetVirtualHostNameToFolderMapping`.

---

## 1. Цель и критерии приёмки

**M5 done (SPEC §10, дословно):** виджет из локальной папки ставится на канвас,
живой при zoom ≥ 0.25, snapshot на дальнем zoom, props переживают перезапуск,
bridge-вызовы без permission блокируются, 10 виджетов на сцене не роняют fps
ниже 60 за счёт лимита live-инстансов.

Частные критерии из TASKS:

- **T20:** демо-виджет (часы) ставится нодой на канвас, двигается, связывается
  ребром; живой при zoom ≥ 0.25, snapshot на дальнем; 10 виджетов — 60 fps за
  счёт лимита live; props-геометрия переживает перезапуск.
- **T21:** вызов без permission блокируется и логируется; remote URL как
  виджет отвергается; props переживают экспорт/импорт `.canvas`; встроенные
  виджеты (часы/календарь/стикер) работают.
- **T22:** по `docs/WIDGETS.md` новый виджет собирается и ставится на канвас
  за 30 минут без чтения исходников хоста.

Автоматическая часть: Linux-гейты (fmt/clippy/тесты) + локальный win-чек
`cargo check --target x86_64-pc-windows-msvc -p canvas-widgets -p canvas-core
-p canvas-render` (крейты без C-зависимостей) + CI windows-latest после каждого
пуша (авторитетный гейт, урок QA-1). Рантайм-поведение WebView2 (создание
контроллера, airspace, suspend/resume, фокус) — ручная приёмка владельца по
разделу 15 ACCEPTANCE.md.

---

## 2. Продуктовые решения (дефолты v1.1; подтвердить или переопределить на приёмке)

Вопросы владельцу были отправлены до старта (11 позиций); ответы не получены —
работаем по дефолтам, выровненным с SPEC. Каждая строка обратима без смены
архитектуры; при переопределении на приёмке — отдельный CR.

| # | Развилка | Решение (дефолт) | Отвергнутые альтернативы |
|---|---|---|---|
| П1 | Объём сессии | Весь M5: T20 → T21 → T22, три коммита, тег v1.1 после приёмки | Только T20 / T20+T21 |
| П2 | Добавление виджета на канвас | ПКМ по пустому канвасу → «Виджеты ▸» (список установленных + встроенных, галочка у установленных); клик — нода в центре viewport. Drag-пакета из Explorer — отдельный путь установки (П10) | Палитра-панель; только drag (встроенным нет входа) |
| П3 | Контент при зуме | **ZoomFactor**: CSS-viewport виджета = мировому размеру области контента; контент масштабируется зумом канваса через `ICoreWebView2Controller::put_ZoomFactor` (кламп 0.25–5.0). Layout стабилен, текст чёткий, виджет ощущается «объектом канваса». При зуме > 5 контент клампится к 5× (виден целиком, относительно мельче) — задокументировано | Reflow (браузерный): контент «дышит» при зуме, на дальнем зуме обрезается; гибрид (два порога, гистерезисы ×2) |
| П4 | `fs:read`-allowlist | Директории файловых нод текущего канваса + папка `.canvas`-файла (те же, что слушает вотчер T10) — автоматически, без настроек | Настройки-редактор путей; весь `%USERPROFILE%` |
| П5 | Повторный drag того же widgetId (новая версия) | Диалог-панель «Обновить „Clock“ 1.0.0 → 1.2.0?»: Да — файлы пакета заменяются, ноды и props сохраняются; Нет — отмена. Та же версия — toast «уже установлен», действий нет | Тихая замена; жёсткий отказ |
| П6 | MCP-инструменты | Минимальный набор: `widget_list`, `widget_add {widgetId, x, y, width, height, props}`, `widget_set_props {nodeId, props}` | Полный набор (+ install/remove по пути); без MCP |
| П7 | Оверлей поверх live-виджета (airspace) | Временный snapshot: на время перекрытия live-виджет прячется (HWND hidden), рисуется последний снапшот. Перекрытие = пересечение с rect миникарты (постоянно видимой), панелей поиска/хоткеев/настроек, контекстного меню, а также на время рамки выделения и протягивания ребра. Пан/зум — НЕ перекрытие (виджеты живут) | Смещать оверлеи; оставить как есть (HWND поверх — некрасиво) |
| П8 | Тема для init/themeChanged | `{ dark: bool, accent: "#3B82F6" }`: dark из темы приложения (`Settings.theme`), accent — константа; смена темы в настройках → themeChanged всем live | Полные токены (bg/card/text/…) — после v1.1; заглушка dark:true |
| П9 | Встроенные виджеты | По SPEC: часы/дата (тикают, чистый JS), календарь-месяц (заметки на дни в props), стикер (текст в props). id: `com.canvasdesk.clock`, `com.canvasdesk.calendar`, `com.canvasdesk.sticker` | Замена стикера на TODO; + помодоро |
| П10 | Подтверждение установки при drag папки | In-canvas диалог-панель «Установить „Clock“ 1.0.0? Разрешения: fs:read, network»: Да/Нет (Enter/Esc, клик) — тот же компонент, что П5 | Нативный MessageBox; toast с кнопкой; автоустановка |
| П11 | Удаление пакета | «Виджеты ▸ <имя> ▸ Удалить пакет» → диалог-подтверждение: файлы из `~/.canvasdesk/widgets/<id>/` удаляются, ноды становятся «битыми» (серая рамка, как brokenLink) | Только ручное удаление папки; пакеты вечные |

Технические решения, принятые без вопроса (следствия SPEC/архитектуры):

- **Пути.** Пакеты ставятся в `~/.canvasdesk/widgets/<id>/` (единый корень
  данных приложения вместе с `cache.db`/`config.toml`, см. `canvas-shell::
  cache::default_cache_dir`). SPEC §7.6 упоминает `%APPDATA%/canvasdesk/widgets` —
  отклонение фиксируется здесь и в SPEC §7.6: фактический корень —
  `%USERPROFILE%\.canvasdesk`; рationale — не плодить второй корень данных.
  user-data-folder WebView2 — `~/.canvasdesk/webview2/` (один на приложение,
  общие browser-процессы рантайма).
- **Хром виджет-ноды.** Заголовок 28 world-px (общая константа `HEADER_HEIGHT`)
  + рамка-инсет 8 world-px слева/справа/снизу. WebView занимает внутреннюю
  область. Заголовок и рамка — канвасные (drag за них, SPEC §8), клик по
  контенту уходит виджету (порты связей и resize-углы — как у обычных нод).
- **Снапшоты** — отдельные текстуры (не атлас тамбнейлов: ячейка 256 мала для
  320×200×zoom), свой проход `widget_pass` поверх карточек (заместитель HWND,
  см. П7), LRU-кэп 16 текстур, захват с клампом 512×512.
- **`canvasdesk.label`.** При создании виджет-ноды `node.label = <имя пакета>` —
  заголовок карточки без обращения к registry из рендера; переименование
  вручную не предусматривается (имя — из манифеста).
- **canvas-widgets не зависит от rusqlite** (C-dep рвёт локальный win-чек):
  доступ к `widget_state` живёт в `canvas-shell` (там уже rusqlite и cache.db).

---

## 3. Инвентарь (что уже есть в коде)

- **Модель готова:** `Node.node_type: String` (Unknown переживает round-trip),
  `Node.canvasdesk: Option<CanvasdeskExt { widget_id, props }>` уже объявлено в
  `canvas-core/src/model.rs` и никем не читается; `Node.broken_link`,
  `Node.label`, `Node.preview_state` — тоже. Вида `NodeKind::Widget` НЕТ —
  добавляем (ветка `"widget"` в `kind()`; отдельный от Unknown, чтобы рендер и
  app различали виджеты без обращения к `canvasdesk`).
- **Порог LOD совпадает:** `THUMB_MIN_ZOOM = 0.25` (thumbs.rs) — виджеты живут
  при zoom ≥ 0.25 (+гистерезис), SPEC §6.2.
- **Паттерны интеграции** (по карте кода, сессия разведки от 2026-09-14):
  - сервисы: worker + `Arc<dyn Fn…>` + `EventLoopProxy<AppEvent>` + `user_event`
    + `request_redraw` (ThumbService/WatchService/McpWake — образец для
    WidgetHost-событий);
  - undo: снапшоты `Canvas` целиком, `push_undo` ДО мутации, лимит 50;
  - меню: плоские массивы пунктов в `canvas-app/src/lib.rs::ui`
    (`CanvasMenuItem`), рендер в `App::menu_overlay`, клики в `on_left_button`;
    вложенных подменю нет — расширяем `ContextMenu` полем подменю;
  - drop: `plan_drop`/`expand_drop_paths` различают файл/папку — папка с
    `widget.json` перехватывается ДО разворачивания в детей;
  - MCP: `TOOLS` в canvas-mcp + `mcp_dispatch` (чистая функция над SceneState —
    тестируется без окна);
  - RGBA→GPU: два образца — `set_thumbnail` (атлас) и `set_minimap`/
    `MinimapPipeline::upload` (своя текстура) — для снапшотов берём второй;
  - HWND: `raw_window_handle` → `RawWindowHandle::Win32` (main.rs, `resumed`);
    DPI: `App::scale_factor()` с поллингом в desktop-режиме (R10).
- **Веб-справка WebView2** — SPEC §11; webview2-com — обёртка над windows-rs
  (версия подбирается под windows 0.62 при добавлении зависимости, обоснование
  в коммите).

---

## 4. Архитектура

### 4.1. Крейт `canvas-widgets` — модули

| Файл | cfg | Ответственность | Юнит-тесты (Linux) |
|---|---|---|---|
| `lib.rs` | — | re-exports, типы `WidgetSnapshot`, `WidgetNodeView` | — |
| `manifest.rs` | — | `WidgetManifest` (serde, строгая схема, unknown-поля warn), валидация: id `[a-z0-9.\-]{3,64}` без `..`, entry — относительный путь без `..`/абсолютных/URL, defaultSize 160..2000, permissions — известные | parse/валид/невалид-манифесты, warn-лист |
| `permissions.rs` | — | `Permission` enum, `Permissions::check(method) -> Verdict`; enforcement-таблица (см. 4.6) | матрица вызов×permission |
| `bridge.rs` | — | типы `HostToWidget`/`WidgetToHost`, конверт JSON-RPC (`to_json`/`from_json`), невалидное = drop+warn (не паника) | round-trip, unknown-method, битый JSON |
| `lod.rs` | — | `LodPlanner`: live/snapshot/placeholder по (zoom, видим, перекрыт, есть ли пакет, лимит 6 LRU), гистерезис ±0.02, решение о refresh-снапшоте (5 с, только видимые за лимитом при zoom ≥ 0.25), кламп ZoomFactor 0.25–5 | таблицы решений, LRU-порядок, тайминги |
| `layout.rs` | — | мировая геометрия: `content_rect(node) -> (x, y, w, h)` (инсеты 28/8), `webview_rect_phys(content, camera, scale)` → пиксели, `zoom_factor(camera)` | инварианты round-физпкс, инсеты, клампы |
| `registry.rs` | — | `WidgetRegistry`: скан `~/.canvasdesk/widgets/*`, `install(src_dir)` (валидация манифеста, копирование, отказ при невалидном id), `update(...)`, `remove(id)`, встроенные пакеты: встраивание `include_dir!` → материализация при старте с tombstone `~/.canvasdesk/widgets/.deleted/<id>` (версия приложения в файле; новая версия виджета перебивает tombstone) | tempdir: install/update/remove/tombstone/скан |
| `snapshot.rs` | — | `decode_png_rgba(bytes) -> WidgetSnapshot` (image-крейт) | декод синтетического PNG |
| `host/mod.rs` | **windows** | `WidgetHost`: env/controller на UI-потоке, таблица live-инстансов, применение `FramePlan` (SetWindowPos/SetWindowRgn/ZoomFactor/show-hide/TrySuspend/Resume/CapturePreview), WebMessageReceived → события наружу, Esc через AcceleratorKeyPressed → фокус канвасу | — (win-чек msvc + CI) |
| `host/webview.rs` | **windows** | создание `CoreWebView2Environment`+`Controller` (async), `SetVirtualHostNameToFolderMapping` (host = санитизированный id), CSP-заголовок через `WebResourceRequested`-фильтр, блок внешних запросов без `network`, `NavigationStarting`/`NewWindowRequested` — отмена наружных, `PostWebMessageAsJson`, `CapturePreview`→PNG | — |

`unsafe` — только в `host/` (AGENTS: canvas-shell/canvas-widgets), каждый блок
с SAFETY-комментарием. Win32-приёмы со ссылками: SetWindowPos — стандартный
API; DPI-поллинг после репарентинга — R10 (виджеты пересчитывают rect каждый
кадр по `App::scale_factor()`, см. RECIPES 29/149).

### 4.2. События и потоки

- `AppEvent::Widget(canvas_widgets::WidgetEvent)` (cfg(windows), по образцу
  `Desktop`/`Shell`):
  `ControllerReady { node_id, ok }`, `SnapshotReady { node_id, w, h, rgba }`,
  `Message { node_id, msg: WidgetToHost }`, `Tick`.
- WebView2 живёт на UI-потоке (event loop winit): контроллер — ребёнок HWND
  канваса, события приходят в накачку сообщений цикла. Пока есть
  `pending_controllers > 0` — `about_to_wait` держит цикл красным (как
  `search_pending`), чтобы колбэк создания не ждал пробуждения.
- Таймер снапшотов: поток-тикер 1 с → `AppEvent::Widget(Tick)` → хост
  проверяет, каким виджетам пора освежить снапшот (stagger по хэшу node_id).
- `FramePlan` собирается на главной каждый `RedrawRequested` (см. 4.5) и
  применяется к host'у синхронно с кадром: сначала `SetWindowPos`/show/hide,
  потом `renderer.render(...)` — порядок фиксирует airspace (HWND не рисуется
  в момент кадра поверх снапшота-заместителя).

### 4.3. Нода-виджета в модели

- `node_type: "widget"`, `canvasdesk: { widgetId, props }`, `label` = имя
  пакета, `x/y/width/height` — как у всех нод; при отсутствии пакета —
  `broken_link: true` (серая рамка, «пакет удалён» в тултипе).
- Порты связей, группы, undo, автосейв, экспорт в Obsidian — работают как для
  обычных нод (CanvasdeskExt уже в serde round-trip).
- `preview_state` не используется виджетами (LOD-состояние вычисляется на лету,
  не персистится — SPEC хранит геометрию, не режим отображения).

### 4.4. Геометрия и зум (П3)

```
content = node.rect, инсет: сверху 28 (HEADER_HEIGHT), бока/низ 8
webview_phys = round(content × camera.zoom × scale_factor)  // пиксели экрана
zoom_factor  = clamp(camera.zoom, 0.25, 5.0)
```

CSS-viewport виджета = `webview_phys / (scale_factor × zoom_factor)` =
мировой размер контента (при зуме ≤ 5). Скругление углов HWND —
`SetWindowRgn` по 8 физ. px (как CORNER_RADIUS карточек). Каждый кадр:
`SetWindowPos(HWND, x, y, w, h, SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE |
SWP_NOZORDER)` — дёшево, SPEC §7.6. Resize ноды → CSS-viewport меняется →
контент перетекает (естественно); зум канваса → ZoomFactor (масштаб, без
reflow). Смена DPI (в т.ч. перенос окна) — пересчёт `webview_phys` тем же
формульным путём (поллинг R10 в desktop-режиме уже снят аппом).

### 4.5. LOD, лимит live, снапшоты (SPEC §7.6)

Решение на кадр для каждой виджет-ноды (`LodPlanner::plan`):

| Условие (по приоритету) | Режим |
|---|---|
| нет пакета (bitая) / рантайм WebView2 недоступен | placeholder: серая карточка + имя |
| zoom < 0.23 (выход) / вне viewport / перекрыт оверлеем/рамкой/ребром (П7) | **snapshot**: HWND hidden + `TrySuspend`; рисуем снапшот, если есть; иначе плейсхолдер |
| zoom ≥ 0.27 (вход, гистерезис ±0.02) и видим и не перекрыт | кандидат в live |
| кандидатов > LIVE_LIMIT (6, конфиг) | первые 6 по LRU-приоритету (недавно видимые/взаимодействовавшие) — live, остальные — snapshot + периодический refresh |

- Live → snapshot: финальный `CapturePreview` ДО hide (PNG → RGBA →
  `AppEvent::Widget(SnapshotReady)` → текстура). Захват с клампом 512×512.
- Snapshot-виджеты, видимые при zoom ≥ 0.25 и за лимитом (т.е. «ожидают
  освобождения слота»): refresh каждые 5 с (resume → capture → suspend,
  stagger). На zoom < 0.25 refresh не нужен (слишком мал на экране).
- Снапшот-квады: `widget_pass` — поверх карточек (заместитель HWND, П7),
  текстура per-widget (LRU-кэп 16), аплоад по образцу
  `MinimapPipeline::upload`. При отсутствии снапшота — заглушка темы.

### 4.6. Bridge (JSON-RPC поверх postMessage)

Конверт: `{"jsonrpc":"2.0","method":…,"params":{…}}` + `id` для запросов.
Хост→виджет — `PostWebMessageAsJson`, виджет→хост —
`window.chrome.webview.postMessage` → `WebMessageReceived` (JSON).

**host → widget (уведомления):**

| method | params | когда |
|---|---|---|
| `init` | `{ nodeId, props, theme, zoom }` | после готовности контроллера |
| `propsChanged` | `{ props }` | setProps/undo/MCP изменили props |
| `visibility` | `{ visible, reason }` | live↔snapshot переход |
| `themeChanged` | `{ theme }` | смена темы приложения |

**widget → host:**

| method | params | permission | семантика |
|---|---|---|---|
| `ready` | — | — | виджет загружен, готов к init-данным |
| `resize` | `{ w, h }` | — | запрос размера ноды (кламп 160..2000) |
| `setProps` | `{ props }` | — | persist в `.canvas` (undo-шаг!) |
| `openFile` | `{ path }` | `shell:open` | открыть в ассоц. приложении |
| `readDir` | `{ path }` (запрос, с `id`) | `fs:read` | листинг allowlist-директории (П4); ответ `{ entries }` / ошибка |
| `toast` | `{ text }` | — | уведомление (в v1.1 — лог+HUD-строка) |
| `stateGet`/`stateSet` | `{ key }` / `{ key, value }` (запросы) | — | объёмное состояние в SQLite `widget_state` — добавлено к списку SPEC, т.к. §7.6 ссылается на widget_state, но bridge-пути не определял |

`canvas:read` — в v1.1 зарезервировано (будущие запросы структуры канваса);
манифестом принимается, ни один метод не требует. `network` — см. 4.7.
Каждое сообщение десериализуется serde-схемой; невалидное — drop + warn
(AGENTS). Enforcement — на КАЖДЫЙ вызов (permissions.rs), отказ логируется
(tracing warn) и возвращается виджету ошибкой JSON-RPC для запросов с `id`.

### 4.7. Безопасность (AGENTS «Правила безопасности и виджетов»)

- Только локальные пакеты; remote URL как виджет — отвергается на установке
  (валидация манифеста: entry не URL) и на навигации.
- Origin пакета: `SetVirtualHostNameToFolderMapping(«<sanitized-id>», папка
  пакета)` → навигация только `https://<sanitized-id>/entry`;
  `NavigationStarting` вне origin и `NewWindowRequested` — отменяются.
- `WebResourceRequested`-фильтр (все запросы): разрешены `https://<host>/*`;
  внешние http(s) — только при permission `network`, иначе Cancel; CSP
  `default-src 'self'` инжектится в заголовок ответа (с `connect-src 'self'`
  плюс `https:` при network — иначе фетчи виджетов бесполезны).
- Suspend/скрытие — не доверяет виджету: при window hidden (минимизация окна)
  все live переводятся в snapshot.
- Паника виджета/крэш рендер-процесса WebView2 (ProcessFailed) — warn,
  пересоздание контроллера; канвас не падает.

### 4.8. Установка, обновление, удаление, встроенные

- Drag папки с валидным `widget.json` (корень) → призрак-план «Установить
  виджет …» → Drop → диалог-панель (П10) → `registry.install`: копирование в
  `~/.canvasdesk/widgets/<id>/`, нода НЕ ставится автоматически? — ставится:
  после успешной установки в точке дропа создаётся нода (это и есть «ставится
  на канвас», SPEC §10). Отказ диалога → ничего не копируется.
- Обновление (П5): замена файлов, props/ноды сохраняются; live-инстансы
  пересоздаются (reload).
- Удаление (П11): `registry.remove(id)`, ноды пакета → `broken_link: true`.
- Встроенные (П9): вшиты в бинарник canvas-widgets (`include_dir!`),
  материализуются при старте: нет папки и нет tombstone → установить; версия
  ниже встроенной → обновить; tombstone с версией ≥ версии виджета → не
  ставить (уважаем удаление пользователем). Встраивание — обоснование
  коммита (include_dir — новая зависимость).
- Watcher на `~/.canvasdesk/widgets` не ставим (изменения пакетов — только
  через наши сценарии; ручное удаление папки подхватится на следующем старте
  и в `registry.reload()` по меню).

### 4.9. Данные

- `props` + геометрия — в `.canvas` (истина раскладки, SPEC §5.3); переживают
  экспорт/импорт, автосейв 2 c, undo-снапшоты.
- `widget_state(node_id, key, value)` — таблица в существующем `cache.db`
  (модуль `canvas-shell/src/widget_state.rs`, rusqlite уже там);
  пересоздаваемая, как весь кэш. API: get/set/keys, ошибки = warn и пустой
  результат (SPEC §5.3 — удаление кэша ничего не ломает).

### 4.10. Ввод и фокус (SPEC §8)

- Клик по контенту — ввод уходит WebView2 (HWND). Клик по заголовку/рамке —
  канвас: выделение + drag (как у обычных нод). Порты/углы resize — работают.
- Esc: `ICoreWebView2Controller::AcceleratorKeyPressed` (VK_ESCAPE) →
  `SetFocus(hwnd канваса)` — фокус и клавиатура возвращаются канвасу; это
  надёжнее, чем ждать сотрудничества виджета. Двойной клик по контенту —
  виджету (не открываем файл, не редактируем).
- Выделение рамкой (CR-001) выделяет и виджет-ноды (обычный AABB).

### 4.11. Airspace (П7)

HWND поверх wgpu — архитектурное ограничение (SPEC §7.6). Политика: live
скрывается (→ snapshot) при перекрытии с: миникартой (всегда видимой),
панелью поиска (Ctrl+F), панелью хоткеев (F1), настройками, контекстным
меню, диалогом установки, на время `select_rect` и `edge_drag`. Пересечение
считается в логических координатах оверлей-rect'ов (миникарта уже умеет
отдавать rect из рендера; для панелей — их экранные rect'ы из lib.rs::ui).
В desktop-режиме поверх канваса нет чужих оверлеев — виджеты живут свободно.

### 4.12. MCP (П6)

- `widget_list` → установленные пакеты (id, name, version, permissions,
  число нод);
- `widget_add { widgetId, x, y, width?, height?, props? }` → валидация по
  списку установленных, нода в (x, y) или центр viewport при пропуске;
  undo-шаг; ответ — id ноды;
- `widget_set_props { nodeId, props }` — полная замена map (undo-шаг).
- Диспетчер: расширение `mcp_dispatch` (передать `installed: &[(id, name,
  version)]` из registry; геометрия/undo — как в существующих инструментах).
  Схемы инструментов — в `TOOLS` canvas-mcp. Тесты — в main.rs tests по
  образцу `mcp_node_*`.

### 4.13. Структура репозитория после M5

```
crates/canvas-widgets/        # 4.1
crates/canvas-shell/src/widget_state.rs   # 4.9
crates/canvas-app/src/widgets.rs          # WidgetManager (координатор M5)
crates/canvas-render/src/widget_pass.rs   # снапшот-квады
assets/widgets/{clock,calendar,sticker}/  # исходники встроенных (копии для правки)
sdk/widget-template/          # Vite+TS шаблон (T22)
sdk/canvasdesk.ts             # клиентский SDK (<150 строк)
examples/todo-panel/          # пример 1: офлайн, props
examples/dashboard/           # пример 2: network-fetch
docs/WIDGETS.md               # документация разработчика виджетов (T22)
```

---

## 5. T20. Widget runtime (WebView2-хост)

**Цель:** виджет-нода живёт на канвасе: WebView2-инстанс над областью контента,
синхронизация с камерой, LOD live/snapshot, лимит live-инстансов, снапшоты,
ввод/фокус. Демо: встроенные часы ставятся из меню, двигаются, связываются.

Подзадачи (зоны файлов; внутри T20 последовательные, одна сессия):

- **T20-A — core (`canvas-core/src/model.rs`)**: `NodeKind::Widget` (ветка
  `"widget"` в `kind()`), конструктор `Node::widget(id, widget_id, label, x, y,
  w, h, props)`. Тесты: round-trip виджет-ноды (тип + canvasdesk + unknown-поля
  рядом), kind().
- **T20-B — чистое ядро canvas-widgets**: `manifest.rs`, `permissions.rs`,
  `bridge.rs` (типы + serde), `lod.rs`, `layout.rs`, `snapshot.rs` (декод PNG).
  Тесты по таблице 4.1 (все на Linux).
- **T20-C — registry + встроенные (`registry.rs`, `assets/widgets/clock/`)**:
  скан/установка/обновление/удаление/tombstone; встраивание `include_dir!`;
  материализация при старте. Часы: vanilla JS (цифровые + дата, тикают),
  `widget.json` без permissions. Тесты: tempdir-сценарии 4.8.
- **T20-D — рендер (`canvas-render/src/widget_pass.rs` + `renderer.rs`)**:
  API `set_widget_snapshot(node_key, rgba, w, h)`, `clear_widget_snapshot(key)`,
  `widget_quad(index)`; квады поверх карточек (мир-хвост перед screen-оверлеями);
  инстанс как ThumbInstance (uv по текстуре); LRU-кэп 16 текстур; в
  `card_instance` виджет = обычная карточка (заголовок `label`, при
  `broken_link` — серая). Placeholder-контент (когда снапшота нет и режим не
  live): заливка `theme.card_fill` + имя пакета уже в заголовке. Тесты:
  LOD-видимость текстур (чистые решения), кэп LRU.
- **T20-E — host cfg(windows) (`host/`)**: среда+контроллер, виртуальный
  origin, сообщение-цикл, `FramePlan`-применение (4.4/4.5), CapturePreview,
  suspend/resume, Esc-фокус, `AppEvent::Widget`-события. SAFETY-комментарии.
  Win-валидация: локальный msvc-чек + CI.
- **T20-F — интеграция в app (`canvas-app/src/widgets.rs` + `main.rs`)**:
  `WidgetManager` (registry + host + planner + снапшоты); `AppEvent::Widget`;
  обновление плана в `RedrawRequested` (до render), дрен событий в
  `user_event`; меню «Виджеты ▸» (П2, подменю в lib.rs::ui + тест состава
  меню); `--stress-widgets N` (часовые ноды для нагрузочной приёмки);
  недоступность WebView2-рантайма → placeholder + понятная ошибка в HUD/лог
  (SPEC §9). Тесты: план кадра на синтетической сцене (без окна), состав меню,
  stress-генератор, hit-тест подменю.

Критерии T20 (из TASKS): часы из меню — на канвасе, живые при zoom ≥ 0.25,
снапшот на дальнем, двигаются, связываются ребром; 10 виджетов — 60 fps
(локально проверяем только архитектурно: 6 live + 4 snapshot; финально —
приёмка владельцем); геометрия и props переживают перезапуск (тест
round-trip + автосейв).

## 6. T21. Bridge, манифест, permissions

**Цель:** полный протокол, enforcement на каждый вызов, установка drag-ом с
диалогом, обновление/удаление, `widget_state`, календарь и стикер.

- **T21-A — enforcement моста**: обработка `WidgetToHost` в WidgetManager:
  setProps (undo-шаг! push_undo до мутации, `propsChanged` обратно, автосейв),
  openFile (perm shell:open → существующий ShellExecute-путь app), readDir
  (perm fs:read + allowlist П4: `watched_dirs` сцены, чистая функция), toast
  (HUD-строка 3 с + лог), resize (кламп, `scene.mark_dirty`), stateGet/stateSet
  (через canvas-shell). Отказы: warn + JSON-RPC error (для запросов с id).
  Тесты: матрица enforcement на мок-хосте (без WebView2), undo-шаги setProps.
- **T21-B — установка drag-ом**: перехват в `on_drag_event` до
  `expand_drop_paths`: папка с валидным `widget.json` → призрак
  «Установить виджет» (одна карточка в точке курсора) → Drop →
  диалог-панель (П10: Да/Нет, Enter/Esc) → install → нода. Невалидный
  манифест → призрак-ошибка + toast. Повтор с новой версией → диалог
  обновления (П5). Тесты: чистые функции классификации дропа
  (папка-виджет/папка-файлы/файл), план вставки, диалог-хит-тест.
- **T21-C — подменю управления**: «Виджеты ▸ <имя> ▸ Удалить пакет» (П11) с
  подтверждением; «Обновить из папки…» не делаем (обновление — только drag,
  П5). Тесты: пункты, галочки установленных, состояния.
- **T21-D — виджеты календарь и стикер**: vanilla JS, `props`-персистентность
  через SDK-обёртку (v0: прямой postMessage — T22 вынесет в canvasdesk.ts,
  встроенные мигрируют на SDK). Календарь: месяц × дни, клик дня — заметка
  (props.notes). Стикер: contenteditable-текст → props.text (debounce).
- **T21-E — `canvas-shell/src/widget_state.rs`**: таблица в cache.db, API
  get/set/keys; тесты на tempdir.

Критерии T21 (из TASKS): вызов без permission блокируется и логируется;
remote URL отвергается (тест манифеста + CI); props переживают
экспорт/импорт (тест round-trip + ручная); встроенные работают (ручная
приёмка).

## 7. T22. Widget SDK и примеры

- **T22-A — `sdk/canvasdesk.ts`** (<150 строк, без зависимостей):
  типизированная обёртка: `CanvasDesk.init(cb)`, `onProps/onTheme/…`,
  `setProps/resize/toast/openFile/readDir/stateGet/stateSet` (Promise),
  детект standalone (`window.chrome.webview` отсутствует → no-op-адаптер —
  тот же bundle работает как обычная страница).
- **T22-B — `sdk/widget-template/`**: Vite + TypeScript, `npm run pack` →
  `dist-widget/` с манифестом (готовая к drag-установка). README в шаблоне.
- **T22-C — примеры**: `examples/todo-panel` (офлайн: items в props —
  «микрофронтенд»: тот же bundle открывается standalone-страницей);
  `examples/dashboard` (fetch наружу, permission `network` — демонстрация
  отказа без него: fetch падает, виджет показывает понятную ошибку).
- **T22-D — `docs/WIDGETS.md`**: формат пакета, манифест (таблица полей),
  bridge API (таблицы 4.6), permissions (таблица+примеры), LOD-жизненный цикл
  (когда ваш виджет спит), ограничения (airspace П7, лимит 6, кламп зума 5×,
  512-снапшот), туториал «от нуля до канваса за 30 минут», отладка
  (`--stress-widgets`, лог-маркеры host'а).
- Критерий (TASKS): по WIDGETS.md новый виджет собирается и ставится за 30
  минут без чтения исходников хоста — проверяется владельцем на приёмке.

## 8. Риски и митигации

| Риск | Митигация |
|---|---|
| webview2-com несовместим с windows 0.62 | подбор версии на T20-E; при конфликте — пин windows под webview2-com с обоснованием в коммите (база уже 0.62) |
| Создание контроллера асинхронно, цикл winit в Wait | pending-флаг держит `about_to_wait` красным (как search_pending) до ControllerReady |
| WebView2 в WorkerW-ребёнке не получает DPI (RECIPES 29/149) | rect пересчитывается каждый кадр из `App::scale_factor()` (R10-поллинг уже в app); ZoomFactor от DPI не зависит |
| Крэш процесса виджета | ProcessFailed → warn + пересоздание контроллера; канвас жив |
| Падение fps от SetWindowPos каждый кадр | SWP_ASYNCWINDOWPOS + только live (≤6); адресность изменений — двигаем только при delta |
| Снапшот-текстуры едят память | LRU-кэп 16, кламп 512×512, высвобождение текстуры при удалении ноды |
| Пакет-атака (path traversal, злой entry) | валидация манифеста (4.1), виртуальный origin, запрет навигации, CSP, network opt-in |
| Undo разрастается от setProps-спама | коалесценция: подряд идущие setProps одной ноды в один шаг (сравнение snapshot), как в редактировании текста |
| Linux CI не видит cfg(windows)-код | локальный msvc-чек перед пушем + windows-latest CI после каждого пуша (урок QA-1) |

## 9. Задокументированные ограничения v1.1

1. Airspace: HWND поверх канваса — live скрывается при перекрытии оверлеем
   (П7); оверлеи НЕ смещаются. 2. Зум контента клампится 5× (П3). 3. Лимит
   live-инстансов 6 (LRU); 7-й и далее — снапшоты с refresh 5 с. 4. Снапшот
   «замораживается» при zoom < 0.25 (без refresh). 5. Клик по контенту не
   выделяет ноду (выделение — заголовок/рамка/рамка-выбор). 6. `canvas:read`
   зарезервировано. 7. Обновление встроенных после ручного удаления — только
   с новой версией виджета (tombstone). 8. Миникарта видна всегда → её угол —
   постоянная snapshot-зона. 9. Remote URL как виджет — запрещён
   архитектурно. 10. Фокус виджета глушит хоткеи канваса до Esc/клика по
   канвасу (SPEC §8).

## 10. Производительность (цели приёмки)

- 10 виджетов (6 live + 4 snapshot): пан/зум ≥ 60 fps (GTX 1050/Iris Xe);
  SetWindowPos ≤6 окон/кадр, CapturePreview ≤1 одновременно (stagger).
- Холодный старт с 10 виджет-нодами: +<1 с к базовому (контроллеры создаются
  лениво по LOD, максимум 6).
- Память: live-виджеты считаются «живыми превью» (SPEC §6.3 — вне квоты 500
  МБ базовой сцены); снапшот-текстуры ≤16 × 512² × 4 ≈ 16 МБ.
- `--stress-widgets 10` — сценарий для приёмки.

## 11. Порядок работ и коммиты

1. Коммит «docs(m5): план T20–T22…» — этот документ + SPEC §7.6-уточнения +
   TASKS-ссылки + ACCEPTANCE §15. CI.
2. T20 (A→F) — гейты, коммит `feat(widgets): T20 — рантайм…`, CI.
3. T21 (A→E) — гейты, коммит `feat(widgets): T21 — bridge/permissions…`, CI.
4. T22 (A→D) — гейты, коммит `feat(sdk): T22 — canvasdesk.ts…`, CI.
5. Владелец: рантайм-приёмка по ACCEPTANCE §15 → тег v1.1.

Протокол работы — AGENTS.md; журнал — `/home/z/my-project/worklog.md`;
после каждой задачи — запись в worklog (Task ID: m5-docs / m5-t20 / m5-t21 /
m5-t22).
