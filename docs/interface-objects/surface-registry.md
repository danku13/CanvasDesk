# Surface Registry — контракт поверхности экрана (PRD-0009, US-5)

> Дополнение к `node.md` / `edge.md` / …: общий контракт «кто на экране, в
> каком слое, кто получает ввод». Гайд потребителя — `docs/ui-kit.md`;
> источник архитектуры — PRD-0009 §7.3/§7.4.

## 1. Модель

Экран — упорядоченный набор **поверхностей**. Каждая поверхность объявлена в
реестре (`crates/canvas-app/src/app/ui_registry.rs`, `build_registry`) тремя
решениями:

| Поле | Значение | Кому что даёт |
|---|---|---|
| `id` | стабильная строка (`SURFACE_IDS`) | подписи debug-оверлея, pick, тесты |
| `layer` | `UiLayer` (9 полос: World…Debug) | draw-полоса рендера, порядок pick |
| `capture` | `Block / Capture / PassThrough / Passive` | кто получает клик, судьба backdrop-клика |
| `keyboard_scope` | `KeyboardScopeId` | доставка клавиатуры `KeyboardRouter` (верх стека первым) |
| `degradation` | `HideBelow {min_width, min_height}` | исчезновение вместо вырожденной геометрии (what-if бар: 900×600) |

Реестр собирается **на каждый кадр** из состояния приложения (только активные
поверхности; порядок регистрации = от нижних к верхним = обратный Esc-лестнице).

## 2. Контракт поверхности

1. **Геометрия — из layout-функций**: hit-rect'ы кадра (`fill_hit_rects`) и
   отрисовка используют одни и те же чистые функции → pick ≡ тому, что видно.
2. **Клики**: `HitStack::pick` возвращает `Element{surface, element}` (клик по
   hit-rect'у поверхности) или `Backdrop{surface}` (клик мимо — ближайшая
   Block-поверхность). Тела обработки — методы `click_<surface>`.
3. **Backdrop-контракт** (Block-поверхности): галерея/поиск/настройки/docs/
   help/stage/меню — закрыть и глотнуть; онбординг/диалог — глотнуть.
4. **Клавиатура**: `KeyboardRouter::from_registry(...).deliver(...)` — верхний
   скоуп первым, поглотивший гасит доставку; скоупы без обработчика пропускают
   вниз к Canvas-лестнице (команды и NUMI-хоткеи). Esc — `esc_stack` +
   `dispatch_esc` (2-фазные: help-подменю, settings-dropdown).
5. **Draw-порядок**: `UiFrame::draw_bands` → полосы `ScreenBand` по
   `UiLayer::DRAW_ORDER`; внутри полосы — порядок сборки кадра.
   Клип полосы — `SurfaceFrame.clip` поверхности (обязателен с U1):
   рендер исполняет его scissor-бакетом полосы и `TextBounds`-клипом
   текстов (FR-056, F-5); сужение per-surface (compact/scroll/clip/hide)
   исполнено волнами миграции FR-059/060 (аудит G5 закрыт).
   Внутри полосы кит может поднимать элементы точечно: `PaintItem::ZGroup`
   (z-index per-element, стабильная сортировка `take_items`) и
   `PaintItem::Transform` (rotate) — FR-074.

## 3. Поверхности (U4, 22 идентификатора)

| id | Слой | Capture | Scope | Примечание |
|---|---|---|---|---|
| `world` | World | PassThrough | CANVAS | карточки/рёбра; NUMI-хоткеи |
| `wheel` | WorldOverlay | Block | — | donut-меню, polar-геометрия |
| `whatif` | Panels | Capture | whatif | бар/список/таблица/пилюля; HideBelow 900×600 |
| `hotkeys` | Panels | Capture | hotkeys | смещается правее полосы палитры (FR-054) |
| `corner_buttons` | Panels | Capture | — | ⚙/тема/«?» |
| `settings` | **Modals** | Block | settings | FR-054: Panels → Modals (модаль выше панелей) |
| `menu` | Popups | Block | — | контекстное меню + подменю |
| `choice_menu` | Popups | Block | — | FR-050 Н2 |
| `template_panel` | Panels | Capture | template_panel | развёрнутый док |
| `template_strip` | Panels | Capture | template_strip | свёрнутая полоса |
| `palette` | Widgets | Capture | palette | тулбар выделения |
| `docs` | Popups | Block | — | просмотрщик документации |
| `help_menu` | Popups | Block | — | меню «?» + подменю |
| `stage` | Modals | Block | stage | любой ключ закрывает |
| `search` | Panels | Block | search | backdrop закрывает |
| `editor` | Widgets | Capture | editor | клики остаются в мире |
| `explain` | Modals | Block | explain | PRD-0007 X2 |
| `dialog` | Modals | Block | dialog | T21 |
| `gallery` | Modals | Block | gallery | FR-049 |
| `onboarding` | Modals | Block | onboarding | FR-028 |
| `kit_gallery` | Modals | Block | kit_gallery | FR-055 U4: витрина кита («?» → «О интерфейсе», Q5-a); Esc/backdrop/✕ закрывают |
| `admin_panel` | Modals | Block | admin_panel | FR-070: UI-админпанель («?» → «UI-консоль»); Esc/backdrop/✕ закрывают; сайдбар секций; свотчи токенов — live-правка |
| `empty` | Panels | Capture | — | empty-state (AC-1.1 FR-049) |
| `minimap` | Panels | Capture | — | рисуется проходом рендерера |

## 4. Точки входа

- `crates/canvas-app/src/app/ui_registry.rs` — декларации, кадр, маршрутизация.
- `crates/canvas-ui/src/` — каркас (layers/registry/capture/frame/hit/keyboard/layout/measure); вёрстка поверхностей — `layout/flex.rs` (`FlexLayoutEngine`, сцена — `layout/scene.rs`: grid-треки Auto/MinMax, sticky{top,left} — FR-074).
- `crates/canvas-render/src/renderer.rs` — исполнение `ScreenBand`.
- `docs/ui-kit.md` — «поверхность за 3 шага».
- `docs/prd/prd-0009-ui-layering-uikit.md` — архитектура и гейты G1–G8.

## 5. Актуальность стека (2026-09-25)

После FR-068 W4 движок вёрстки поверхностей один — `FlexLayoutEngine`
(ADR-0015; taffy/`TaffyBackend` вырезаны из workspace целиком);
`default_backend()` → Flex, `pilot_backend()` → `NativeBackend` (API пилотов
сохранён). CSS-паритет вёрстки расширен FR-074: Auto/minmax-треки Grid,
горизонтальный sticky (`ScenePosition::Sticky{top,left}`), rotate и
z-index per-element в paint-слое. Контракт разделов 1–4 не меняется:
слои/реестр/capture/keyboard ортогональны выбору движка вёрстки.
