# Wave M · M0 — Аудит 12 модалей и дизайн-контракт «80% + mobile-style»

Эпик: issue #55 (Wave M). Задача: #49 (M0, Сессия A). Дата: 2026-10-11.
Источники: `crates/canvas-app/src/app/ui_registry.rs:303–607` (build_registry), пофайловый скан рендера, skill [ui-for-agents](https://github.com/danku13/ui-for-agents) (M-O `20-components/modals-and-overlays.md`, T-M `30-ux-patterns/touch-and-mobile.md`, R-A `40-quality/responsive-adaptive.md`).

## 1. Цель и границы

Ремастер всех 12 истинных модалей (слой `Modals` + `CapturePolicy::Block`) под фича-флаг `modals_adaptive` (M2): размер окна **80% слота** по формуле-контракту (§3), внутренний скелет по трём тирам раскладки, для компактных разрешений — mobile-style одноколоночная вёрстка. Старые ветки рендера сохраняются (флаг OFF — путь не меняется ни символом). Демонтаж старых версий — отдельная волна M5 (вне этой серии).

В границах Wave M: геометрия окна, обвязка (header/footer/backdrop), тир-скелеты, скроллы, усечение текста по TextMeasurer. Вне границ: арт онбординга (`onboarding_art.rs`, задача «онбординг ≥2x»), глубокий рефактор admin_ui (см. §6), смена контрактов взаимодействия (§5), смена esc-лестницы/VISUAL_ORDER/CapturePolicy.

## 2. Инвентарь — 12 истинных модалей

| # | id | Регистрация | Рендер (paint) | Rect-расчёт |
|---|---|---|---|---|
| 1 | `settings` | ui_registry.rs:372 | app/overlays.rs:5396 (модальная часть :5656–6707) | settings_ui.rs:1897 `modal_layout_with`, размер :1750–1770 |
| 2 | `stage` | :435 | app/stage.rs:1029 `stage_frame` | canvas-core/bundles.rs:256 `main_stage_rect` (MAX_FRACTION 0.7, MARGIN 24) |
| 3 | `explain` | :464 | app/explain.rs:554 `explain_frame` | explain_ui.rs:106 `window_rect` (kit::modal) |
| 4 | `migrate` | :482 | app/overlays.rs:7229 | storage_ui.rs:249 `migrate_layout` |
| 5 | `canvas_manager` | :492 | app/overlays.rs:7638 | canvas_manager_ui.rs:464 `manager_layout` |
| 6 | `autolink` | :512 | app/explain.rs:120 `autolink_frame` | autolink_ui.rs:240 `dialog_rect` (kit::modal) |
| 7 | `dialog` | :518 | app/handler.rs:754–813 (band Modals :812) | app.rs:11635 `dialog_rect` → :11684 измеренный |
| 8 | `gallery` | :524 | app/overlays.rs:2020 | scheme_gallery_ui.rs:259 `layout_with` |
| 9 | `kit_gallery` | :533 | app/overlays.rs:1274 | kit_ui.rs:617 `gallery_panel` / :649 TO-BE layout |
| 10 | `admin_panel` | :541 | app/overlays.rs:1045 | admin_ui.rs:145 `admin_panel`, layout :220–330 |
| 11 | `onboarding` | :547 | app/overlays.rs:4137 | onboarding_ui.rs:545 `card_layout` (kit::modal) |
| 12 | `ai_onboarding` | :561 | app/overlays.rs:4406 | onboarding_ui.rs:950 `ai_onboarding_layout` (kit::modal) |

Полиморфизм окон уже сейчас — три семейства:
- **`kit::modal(slot, min, max, desired)`** — explain, autolink, onboarding, ai_onboarding, dialog (семантика «min-инвариант приоритетен», без финального viewport-клампа — `canvas-ui/src/component/modal.rs:19–28`);
- **`kit::panel_rect(slot, min, max, desired)`** = `stack(slot, constrain(...), Center, Center)` — kit_gallery (min 560×420 / max 900×640, kit_ui.rs:55,57), admin_panel (min 760×500 / max 1120×720, admin_ui.rs:55,57);
- **собственные формулы** — settings (двухъярусный constrain 560×400…880×640 + Mobile fullscreen-sheet <768), stage (0.7 от вьюпорта − 48), migrate (480 × контент), canvas_manager (640 × контент, max_h = vh−48), gallery (660 × контент, ≤ vh−48), dialog (измеренный clamp 280…440).

## 3. Дизайн-контракт 80% + mobile-style (M1-примитив)

### 3.1 Формула

```
eff   = slot − margin (каждая сторона)          // слот = вьюпорт поверхности
tier  = layout_tier(slot.w)                      // Compact ≤767 | Medium 768–1279 | Expanded ≥1280
size  = constrain(min, max, ratio × eff)          // ratio = 0.8
rect  = if tier == Compact { eff (full-bleed) }   // mobile-style: окно занимает весь слот
        else { stack(eff, size, Center, Center) }
```

Примитив M1: `kit::modal_rect_flex(slot: UiRect, margin: f32, ratio: f32, min: UiVec2, max: UiVec2) -> UiRect` + `LayoutTier { Compact, Medium, Expanded }` + `layout_tier(width: f32) -> LayoutTier` (`canvas-ui/src/component/modal.rs`, реэкспорт `kit.rs`). В Compact примитив возвращает full-bleed `eff` — ветвление тиров инкапсулировано, модали не дублируют его. Тиры — R-A R2 «именнованные диапазоны, не устройства»; min/max остаются у каждой модали (M-O R7 «size follows content» — размер контента сидит в min, потолок в max).

### 3.2 Тиры

| Тир | Диапазон ширины | Поведение |
|---|---|---|
| Compact | ≤767 (вкл. 375) | mobile-style: одноколоночная вёрстка, full-bleed лист (safe-margin), действия в нижней зоне (thumb zone T-M R2), таргеты ≥44×44 (T-M R1), без hover-only (T-M R3) |
| Medium | 768–1279 | 80% слота, min-габариты контракта модали, при нехватке высоты — скролл внутри (M-O R7) |
| Expanded | ≥1280 | 80% слота, max-ограничения, канонические пропорции |

Канонические вьюпорты приёмки: **1280×800 / 1024×640 / 800×560** (LAY8.1) + **375 / 768 / 1024 / 1280 / 1440 / 1920** (CR-014). Крайние — R-A R6 «тестируй края»: 375×667 и 1920×1080.

### 3.3 Совместимость с существующими брейкпоинтами

`SETTINGS_BP_MOBILE = 768` / `SETTINGS_BP_COMPACT = 1280` (ui_registry.rs:290,296) **совпадают** с границами тиров: Mobile-ветка settings (<768 fullscreen-sheet `viewport − 2·SETTINGS_MARGIN`) — уже существующий Compact-тир контракта. Settings — эталон, остальные модали подтягиваются к той же лестнице. Fullscreen-sheet сегодня есть только у settings; у остальных — кламп к вьюпорту с margin (SPACING_XL=24 у admin/kit_gallery/canvas_manager/gallery; SPACING_LG=12 у settings/onboarding/ai_onboarding; 20 у explain).

## 4. Аудит по модалям — состояние и план перевода

«Скелет» = живые `y +=`-курсоры в layout/paint (lay7-семантика). «Усечение» — как обрабатывается текст.

| Модаль | Окно сейчас (flag OFF) | Скелет | Скролл/панели | Усечение | План при flag ON (тиры) |
|---|---|---|---|---|---|
| **settings** | 0.45vw×0.60vh → constrain 560×400…880×640 → кламп vp−24; <768 — fullscreen-sheet | **y+= 0**, fully measured (Row/Column::lay_out_measured, MeasuredItem) | собственный скролл контента; Desktop: nav-сайдбар 180 + контент; Compact: таб-бар 34 | wrap_lines_with_more («ещё N»), маска ключа | эталон: desired = 0.8 (сейчас 0.45/0.60), min/max те же; тиры уже совпадают; M3-b только перегоняет на modal_rect_flex + мобильный скелет |
| **stage** | min(0.7·vp, vp−48), центр; world-адаптация | y+= 1 (`ext_y += 32.0`, stage.rs:1878, `lay7:allow`) | ScrollState ×4; calc-панель, 2 скролл-колонки | ellipsis ×17, TextMeasurer ×7 | desired 0.8 вместо 0.7 (margin 24); **golden-пины геометрии обновлять согласованно**; compact — сцена full-bleed, действия вниз |
| **explain** | kit::modal(vp−20, min 320×240, max=слот, desired 0.80) | y+= 0; 2D-tidy-дерево + pan (документ. отклонение LAY2/LAY10) | pan, без скролл-баров; шапка 44, футер 40 | ширины `(win−240).max(120)` | уже 80% — перевод на modal_rect_flex бесшовный; max=слот сохранить (FR-083) |
| **migrate** | 480 × контент-высота, ≤vp, центр вручную | y+= 0 | чекбокс-список, окно 10 строк, скролл-кламп | ellipsis, wrap_text ×2 | desired = 0.8·eff с min = контент (480), max = прежний; compact — full-bleed список |
| **canvas_manager** | 640 × (top + 9 строк + bottom), max_h = vh−48 | y+= 0 | окно 9 строк + kit::scroll_bar; поиск + чип; empty-state | ellipsis ×2 | как migrate; compact — поиск во всю ширину, строки full-bleed |
| **autolink** | kit::modal(vp, min 320×240, max 760×760, desired 0.94vw×0.88vh) | y+= 3 в rows_layout (группы, константы) | ScrollState.clamp; баннер отклонённых; сворачивание групп | ширины `(win−120).max(120)` | desired → 0.8; **parity-тесты :634–667 и ListRow :673 обновить синхронно**; y+= ×3 в rows_layout заменить на Column (M3-c) |
| **dialog** | измеренный: w clamp 280…440 ≤ vp−40; h по контенту | y+= 0 | нет | m.ellipsis ×2 | formula с min/max 280…440 даёт desktop 440 (не разрастается — M-O R7), compact — full-bleed sheet; tight clip = rect |
| **gallery** | 660 × контент, ≤vh−48, центр | y+= 0 (Column + Spacer) | поиск + чипы категорий + список (scroll_top) | измеренные Ellipsis (FR-053 U3) | desired 0.8, min = контент 660; compact — 1 колонка |
| **kit_gallery** | panel_rect(vp, 560×420…900×640) | y+= 0, sy+= ×2 (сайдбар TO-BE, константы) | scroll.offset (кит-контракт); сайдбар без скролла (v1 D7) | измеренные чипы F-13; **F2: chars>36 (kit_ui.rs:3287)** | desired 0.8, max 900; **F1: PANEL_MAX 640 → окно 542px < 610px потребности — сайдбар получает скролл / в compact группы в табы-аккордеон** (M3-b) |
| **admin_panel** | panel_rect(vp, 760×500…1120×720) | y+= 0 (5 хитов — все в комментариях) | admin_scroll; сайдбар 190 формулой `y + i·(H+GAP)` | wrap_text hint_lines, срез по видимости | desired 0.8, max 1120×720; **риск: min 760×500 при 800×560 → eff 752 < 760 — min-инвариант выйдет за margin; кламп к вьюпорту обязателен**; глубокая чистка формулы сайдбара — по объёму диффа (§6) |
| **onboarding** | kit::modal(vp−12, card 720×scale, h по контенту) | y+= 0 | list_rows + ScrollState тела; футер CTA всегда виден | `(col−pad·2).max(0)` | desired = 0.8·eff с card-клампом; **арт не трогать**; compact — full-bleed с нижними CTA (T-M R7) |
| **ai_onboarding** | kit::modal(vp−12, w 600, h сумма блоков, +180 privacy) | **y+= 2 + py+= ×2 в privacy + layout y+= ×1** (шаги из констант) | нет скролла; 3 radio-card | `(mode−28−tag).max(0)` | desired 0.8; на 800×560 h≈440…631 ок; privacy-лист на compact — скролл внутри; y+= ×4 заменить на Column (M3-c); **backdrop НЕ закрывает — сохранить** |

Сводка: 9 из 12 уже без живых курсорных `y +=`; исключения — ai_onboarding (4), autolink rows_layout (3), kit_gallery sy+= (2, константы). Усечение символьным клипом — kit_gallery F2 (M3-b); остальные — ellipsis/wrap по ширине.

## 5. Осознанные отклонения от ui-for-agents (сохраняем, документируем)

1. **M-O R5 «одна модаль за раз»** — продукт сознательно строит модальные последовательности: canvas_manager→gallery→kit_gallery, dialog поверх любого окна, explain поверх stage, autolink прячет explain. Это рабочие потоки подтверждения/детализации, а не случайный стек. Wave M стек не ломает; реорганизация — отдельное решение владельца.
2. **M-O R4 «scrim click — явное решение»**: ai_onboarding backdrop НЕ закрывает модаль (нужен явный выбор режима) — решение уже принято, сохраняем.
3. **M-O R10 «multi-step — drawer/отдельный view»**: onboarding остаётся модалью с шагами (продуктовое решение FR-028), compact-тир даёт ему bottom-sheet-поведение (T-M R7) без смены поверхности.
4. **T-M R1 таргеты ≥44×44**: компактные контролы kit (кнопки 22–30px) в Compact-тире поднимаются до touch-минимума только в новых ветках флага; desktop-тиры не трогаем (T-M R3 — hover остаётся desktop-расширением).

## 6. Карта конфликтов мержа (сессии B/C/D)

A (M0–M2): `canvas-ui/component/modal.rs` + `kit.rs`, `canvas-core/settings.rs`, `canvas-app/settings_ui.rs` (тумблер), `overlays.rs` (одна match-ветка тумблера), `i18n.rs` (пара ключей), `app/ui_layout_lint.rs` (новый lint).

| Сессия | Файлы | Пересечения |
|---|---|---|
| B (M3-a) | overlays.rs (:7229–7429, :7638–8044, :2020–2289), storage_ui.rs, canvas_manager_ui.rs, scheme_gallery_ui.rs, handler.rs/app.rs (dialog) | overlays.rs — разные диапазоны строк |
| C (M3-b) | overlays.rs (:1274–2019, :5656–6707), kit_ui.rs, admin_ui.rs, **settings_ui.rs** | settings_ui.rs после A (тумблер) — старт только после мержа A |
| D (M3-c) | overlays.rs (:4137–4712), stage.rs, explain.rs/explain_ui.rs, autolink_ui.rs, onboarding_ui.rs | overlays.rs — разные диапазоны |

Все четыре сессии правят `overlays.rs` в непересекающихся диапазонах; общие хелперы не менять без синхронизации в issues. Мержи — rebase поверх main по мере мержа A (B/C/D стартуют строго после).

## 7. Риски и правила приёмки

- **Golden-пины**: stage (геометрия), parity-тесты autolink (:634–667, :673), golden kit_ui `tobe_demo_layout_structure` (:5560+) — при переводах обновлять в том же коммите, пины «случайно подогнанными» не считать.
- **Min-инвариант vs margin**: `kit::modal` приоритизирует min даже за пределами слота (documented деградация) — для admin 760 min при 800×560 обязательна финальная проверка «окно в вьюпорте» (G4).
- **G4**: M2 добавляет `lint_modals_adaptive` — флаг ON × модали × вьюпорты {1280×800, 800×560, 375×667} × RU/EN: хиты реестра, отсутствие паник, интерактивные rect-ы в пределах вьюпорта. После M3-партий lint автоматически покрывает новые ветки.
- **WASM**: изменения UI проверяются `scripts/wasm_gate.sh --check` (+L2 браузер-стенд по возможности; если нет — инструкции ручной проверки в отчёте).
- **Приёмка M4** (владелец): 12 модалей × флаг on/off × {1280×800, 1024×640, 800×560, 375, 768, 1440} × RU/EN.
