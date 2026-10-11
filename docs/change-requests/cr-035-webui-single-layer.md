# CR-035: Единый UI-слой web — перенос author-bar/about/w6-toolbar/tour-menu в GPU

- **Статус:** выполнено (ожидает приёмки владельца)
- **Тип:** CR
- **Приоритет:** критично
- **Владелец:** danku13
- **Источник:** сообщение владельца 2026-10-11: «иконки улетели, чип действительно съехал, поясни что такое author-bar. Поясни зачем сейчас у интерфейса два слоя, часть кнопок на html слое, часть на wasm слое. Я вижу в этом большую проблему с вёрсткой и сложностью позиционирования элементов, если у нас нет проблем с переносом всех ui элементов в wasm слой, то давай так и сделаем» + issue #57 (чип перекрыт DOM author-bar)
- **Связанные задачи:** issue #60 (волна), issue #57 (закрывается), FR-031 (inline-тур), CR-033 (UR-003 — заменяется), FR-087 (About — эволюция), FR-107 (чип C4), FR-108 (C5)
- **Создан:** 2026-10-11
- **Обновлён:** 2026-10-11

## 1. Описание (What)

В web-сборке интерфейс жил в двух слоях: GPU-канвас (wasm, весь рабочий UI) и DOM-хром поверх него (`#author-bar` — ссылки автора в top-left, `#w6-toolbar` — кнопки «Тур/Открыть с диска…/Экспорт HTML» в top-right, `#about-overlay`, `#tour-menu`; z-index 10 — всегда выше канваса). Слои конфликтовали: чип активного канваса (C4, top-left) полностью скрыт под `#author-bar` — менеджер канвасов недостижим мышью (issue #57); UR-003/CR-033 требовал ручной синхронизации сдвигов DOM-хрома (`cd-panel-left/right` body-классы) из GPU-состояния панелей. Владелец подтвердил дефект визуально («иконки улетели, чип действительно съехал») и распорядился перенести все постоянные UI-элементы в слой wasm.

## 2. Влияние (Impact)

| Объект | Что было | Что стало |
|---|---|---|
| Чип активного канваса (FR-107) | перекрыт DOM `#author-bar`, клики уходят в GitHub/Telegram | виден и кликабелен — #57 закрыт |
| Author links (GitHub/TG/LI/Donate/ℹ) | DOM `#author-bar` top-left (166×30, z-index 10) | GPU author-strip «ℹ Об авторе» внизу-слева (web-only) + строки в GPU-модале About |
| «Об авторе» (FR-087) | DOM `#about-overlay` + JS-мост сигнала `ui:open-about` | GPU-модал `about_ui.rs` (panels/Block, Esc/✕/backdrop); пункт меню открывает напрямую |
| «Тур / Открыть с диска… / Экспорт HTML» | DOM `#w6-toolbar` (кнопки + листенеры `toolbar.rs`) | пункты меню канваса (web-хвост `canvas_menu_visible_items_web`) |
| Сценарии тура | DOM `#tour-menu` dropdown + btn-tour | GPU-подменю «Тур…» → сигнал `ui:run-tour:<id>` → JS-движок |
| UR-003 (CR-033) | body-классы `cd-panel-left/right`, CSS-сдвиги DOM-хрома | удалён — пересечений слоёв больше нет (`html_panel_overlap` удалён) |
| Тур-сценарии (sdk) | toolbar.ts целиком про `#w6-toolbar`; шаги locate-toolbar/save ссылались на DOM | удалён; шаги переехали на rect-якоря GPU-чипа (12,12,170,30) |

## 3. Анализ (Root Cause)

Двухслойность была исторической: DOM-кнопки обосновывались «жестами браузера» (пикер/экспорт требуют `window` + user gesture). К моменту CR волна C2 уже опровергла посылку — `pick_folder()` (fs_folder.rs:543) вызывает `window.show_directory_picker()` из GPU-оверлея менеджера, а `WebRequest::CanvasOpenDisk/CanvasImport/CanvasExportActive` доставляют файловые действия из canvas-app. Значит DOM-кнопки дублировали конвейер, существовавший в wasm-слое. Побочно: инлайн-бандл тура и SDK-источник разошлись (в бандле был фикс `document.head` для injectStyles, в источнике — нет; пересборка подняла дроп источника).

## 4. Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Модуль About | `canvas-app/src/about_ui.rs` (новый) | чистые `about_layout`/`about_hit` + `strip_layout`/`strip_hit` + `ABOUT_LINKS` (6 ссылок); тесты геометрии/hit |
| Пункты меню | `canvas-app/src/lib.rs` | `CanvasMenuItem::{OpenDisk, ExportHtml, Tour}` + `WEB_MENU_ITEMS` + `canvas_menu_visible_items_web(align, autowidth, web)` (натив — без web-хвоста, тест C5 «12 пунктов» в силе) + `SubmenuAction::RunTour(id)` |
| Состояние | `canvas-app/src/app.rs` | `about_open`/`author_strip` флаги, `WebRequest::{ExportHtml, OpenUrl}`, `is_web_build()`; `html_panel_overlap` удалён |
| Ввод | `canvas-app/src/app/input.rs` | клики `about-close/about-link-N/about-body`, `strip-about`; Esc-складка About (`KeyOwner::About`); Тур-подменю из 4 сценариев |
| Реестр | `canvas-app/src/app/ui_registry.rs` | поверхности `about` (Modals/Block) + `author_strip` (Panels/Capture), hit-зоны, VISUAL_ORDER |
| Отрисовка | `canvas-app/src/app/overlays.rs`, `handler.rs` | `about_overlay` (dim + панель + строки), `author_strip_overlay` (чип ℹ); полосы кадров |
| Web-слой | `canvas-web/src/*` | `toolbar.rs` удалён; `web_requests.rs`: ExportHtml → `export_html_active`, OpenUrl → `window.open(url, "_blank", "noopener")`; `tour_aware_app.rs`: UR-003-синхронизация удалена |
| index.html | `canvas-web/index.html` | −22 800 симв. DOM-хрома (разметка/CSS/JS); листенер `ui:run-tour:<id>` |
| Иконки | `assets/icons/*/info.svg` + `rasterize_icons.py` | имя `info` (44-е, атлас 1408×160); фолбэк-глиф ℹ (U+2139, NotoSansDisplay) |
| Тур-SDK | `sdk/web-onboarding/src/*` | toolbar.ts удалён; шаги на rect-якоря; `injectStyles` — фикс дрейфа (document.head); build-скрипту возвращён `--bundle` |
| i18n | `canvas-app/src/i18n.rs` | +27 ключей ×2 языка (меню, About-модал, strip-тултипы, сценарии) |
| Линты | `ui_layout_lint.rs` | `lint_author_strip_visible`, `lint_about_open` (+backdrop-ассерт), `lint_about_over_strip` |

## 5. Точки входа (Entry Points)

`docs/interface-objects/` (surface-registry: about/author_strip), `docs/user-docs/` (владелец решает отдельно — вопрос задан в сводке), onboarding: сценарии обновлены (rect-якоря), `docs/ACCEPTANCE.md` — приёмка ниже.

## 6. Проверка (Verification)

- [x] `cargo test --workspace` — 3175 passed / 0 failed (вкл. 3 новых линта G4: strip/About/combo)
- [x] `cargo clippy --workspace -- -D warnings` — чисто
- [x] `cargo fmt --check` — чисто
- [x] `scripts/wasm_gate.sh --check` — OK
- [x] E2E localhost-стенда (headless-Chromium, WASM-TESTING L2): DOM `#author-bar/#w6-toolbar/#about-overlay/#tour-menu` отсутствуют; чип top-left виден; author-strip кликабелен → модал About (6 строк-ссылок); меню канваса — 15 пунктов; подменю «Тур…» — 4 сценария; запуск сценария — `.cd-tour-tooltip` в DOM; 0 pageerror / 0 console.error

## 7. История изменений (Changelog)

- 2026-10-11 — Super Z — волна реализована единым коммитом (issue #60), статус «выполнено, ожидает приёмки».

## 8. Источники истины (References)

- `crates/canvas-app/src/about_ui.rs`; `canvas_chip_ui.rs` (паттерн чипа)
- `crates/canvas-app/src/app/ui_registry.rs` (id::ABOUT/AUTHOR_STRIP, VISUAL_ORDER)
- `crates/canvas-web/src/web_requests.rs` (ExportHtml/OpenUrl)
- `sdk/web-onboarding/src/tour.ts` (injectStyles — синхронизация с бандлом)
- Issue #60, #57; FR-104–108 (мультиканвас), CR-033 (UR-003 — отменён)
