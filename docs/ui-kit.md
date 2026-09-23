# UI kit — гайд каркаса экрана (PRD-0009)

> Как добавить поверхность за 3 шага, как верстать примитивами, как измерять
> текст и какие линты держат геометрию в порядке. Источник архитектуры —
> `docs/prd/prd-0009-ui-layering-uikit.md`; контракт поверхности —
> `docs/interface-objects/surface-registry.md`.

## 1. Что это

Экран CanvasDesk — не ad-hoc списки квадов, а **модель поверхностей**:

- `canvas-ui` — чистая геометрия экрана (без GPU/ОС): слои `UiLayer`,
  реестр `SurfaceRegistry`, capture-политики `CapturePolicy`, кадр `UiFrame`,
  `HitStack`, `KeyboardRouter`, layout-примитивы, `TextMeasurer`.
- `canvas-app::app::ui_registry` — декларация поверхностей приложения
  (что открыто, в каком слое, кто ловит клики/клавиатуру) + hit-rect'ы из тех
  же layout-функций, что рисуют.
- `canvas-render` — исполняет кадр полосами слоёв (`ScreenBand`), порядок =
  `UiLayer::DRAW_ORDER`; внутри полосы — порядок сборки.

Один источник геометрии → **ввод = тому, что видно**; добавление поверхности
не трогает цепочки ввода.

## 2. Поверхность за 3 шага

1. **Декларация** — `crates/canvas-app/src/app/ui_registry.rs`:
   константа `id`, запись в `build_registry` (слой, capture-политика,
   keyboard-scope при необходимости, деградация `HideBelow`).
2. **Hit-rect'ы** — arm в `fill_hit_rects`: rect'ы из тех же layout-функций,
   что использует отрисовка (`HitRect::interactive` / `HitRect::decoration`).
3. **Ввод** — клик: arm в `dispatch_surface_click` (тело вынести в метод
   `click_<surface>`); клавиатура: при необходимости arm в `owner_of` +
   `route_owner_key`; Esc: arm в `dispatch_esc`.

Отрисовка — своя overlay-функция в `app.rs`, квады/тексты кладутся в полосу
своего слоя (`ScreenBands::push`). Порядок pick и draw выводится из реестра —
ручные z-списки запрещены.

## 3. Слои и capture-политики

Слои снизу вверх: `World → WorldOverlay → Widgets → Panels → Popups → Modals
→ Drag → Toasts → Debug`.

| Политика | Клики по поверхности | Клики мимо (backdrop) | Примеры |
|---|---|---|---|
| `Block` | перехватывает всё | контракт поверхности (закрыть/глотнуть) | настройки, docs, галерея, онбординг, stage, диалог |
| `Capture` | по hit-rect'ам | уходит ниже | what-if бар, дока палитры, хоткеи, миникарта |
| `PassThrough` | не перехватывает | уходит ниже | мир |
| `Passive` | нет hit-rect'ов | уходит ниже | тосты |

Попапы (меню, flyout) — `Popups`, модали — `Modals`; панель/бар — `Panels`.
Esc-лестница выводится из порядка регистрации (`esc_stack`): регистрируй
поверхности от нижних к верхним.

## 4. Layout-примитивы (`canvas_ui::layout`)

Immediate-функции от слота родителя — возвращают rect'ы детей:

- `Row { gap, main, cross, policy }` / `Column { gap, main, cross }` —
  линейная вёрстка; дети — `Child::fixed(w, h)`, распорки — `Child::spacer`.
- `RowPolicy::Fit` — переполнение НЕ маскируется (ловит линт);
  `RowPolicy::SqueezeTail` — именованная деградация узкого слота (хвост
  сжимается до нулевой ширины, не пикается) — замена молчаливых `take`/`break`.
- `stack(slot, size, HAlign, VAlign)` — фиксированный блок в слоте
  (центрирование модалок, прижатие футера).
- `constrain(min, max, desired)` — кламп размера (модалки, панели).
- `pad(slot, EdgeInsets)` — внутренние отступы.
- `Custom(rect)` — escape-hatch экзотики (polar wheel, drop-сетка): только с
  комментарием-обоснованием, попадает в grep-аудит G8.

Зазоры/радиусы — из токенов `canvas_core::tokens` (`SPACING_S/SM/MD/LG/XL`,
`RADIUS_CHIP/PANEL/PILL`) — значения синхронизированы с
`design/tokens/dimensions.json`.

## 5. Измеренный текст (`canvas_ui::measure`)

Ширины для раскладки — ТОЛЬКО через `TextMeasurer` (реальный шейпинг
cosmic-text, те же метрики, что у screen-текстов рендера):

```rust
let mut measurer = canvas_ui::measure::TextMeasurer::new();
let mut fs = canvas_render::text::measure_font_system();
let w = measurer.width_of(&mut fs, label, canvas_render::text::SANS_FAMILY, 13.0);
let cut = measurer.ellipsis(&mut fs, label, FAMILY, 13.0, max_width);
```

- measurer создаётся на перекомпоновку/кадр (дешёвый; кэш внутри кадра);
- `FontSystem` — владение рендера (`measure_font_system`), аргументом;
- эвристики «символов × коэффициент» и `chars.truncate` запрещены (класс
  дефекта CR-015; усечение — только `ellipsis` по фактической ширине).

## 6. Линты (CI)

- **G4** (`app::ui_layout_lint`, исполняется `cargo test --workspace`):
  полный кадр реестра на канонических состояниях × 3 окна (1280×800,
  1024×640, 800×560) × RU/EN — 0 пересечений интерактивных rect'ов РАЗНЫХ
  поверхностей одной полосы, 0 выходов за вьюпорт; Block-модаль накрывает
  экран (backdrop-контракт).
- **G5** — grep-аудит мигрированных модулей: 0 `take(`-срезов, 0
  `break`-клампов раскладки, 0 символьных эвристик ширины.
- **G8** — 6 поверхностей на реестре+примитивах (поиск, настройки, docs,
  палитра шаблонов, what-if, галерея).
- Доброкачественные налезания (popup поверх панели с «верхний непрозрачный
  скрывает нижний») — не состояние линта: канонические состояния не комбинируют
  фичи; реальные коллизии (хоткеи × полоса палитры, модаль настроек × полоса)
  найдены и устранены на U5 — см. историю PRD-0009.
- **Scissor-политика рендера (FR-056, F-5)**: каждая полоса `ScreenBand`
  несёт клип `UiRect` (лог. px из `SurfaceFrame.clip` кадра реестра);
  рендер исполняет его scissor-бакетом полосы (один `set_scissor_rect`
  на полосу — R-1) и `TextBounds`-клипом текстов. Инвариант: scissor
  никогда не расширяет видимое (ceil/floor-конверсия + кламп к вьюпорту),
  пустой клип — полоса не рисуется. Мигрируемые модули (FR-059/060)
  удаляют ручные клампы — контент клипуется системой (G5).

## 7. Кит-виджеты (FR-055 U4, F-8)

`canvas_ui::kit` — модель виджетов поверх примитивов; кит НЕ рисует и ввод
не перехватывает (слой/модальность — только из реестра). Контракты:

- **цвета — только слоты**: [`KitPalette`] (срез `ThemeColors` v2 + слоты
  состояний; маппинг — `canvas-render::theme`) или явные слоты
  (`panel_style_of`/`control_style_of` — миграция каноники поверхностей,
  I-1 ноль скачка). Ни одной константы цвета в ките;
- **отступы/радиусы** — только spacing/radius-scale токенов;
- **текст** — только `TextMeasurer` + `ellipsis`; усечение посимвольно —
  запрещено (класс CR-015);
- компоненты: `Panel`/`Modal` (constrain+stack, затемнение), `Button`
  (Primary/Secondary/Ghost/Danger × Normal/Hover/Pressed/Disabled/Selected),
  `IconButton`, `Chip`, `Dropdown` (якорь + flip + зажим во вьюпорт),
  `Toast` (низ-центр, TTL, avoid-бар), `Tooltip` (якорь + flip + delay
  500 мс); анимации — `canvas_ui::anim` (`BoolAnim`, dt-детерминизм).

Поверхность за 3 шага (§2) + стиль из кита: виджет возвращает rect и
[`ControlStyle`]/[`PanelStyle`] — потребитель кладёт квад/текст в полосу
своего слоя. Живой образец — витрина `kit_gallery` (меню «?» → «О
интерфейсе»): компоненты × состояния × RU/EN × темы; кнопка темы в шапке —
реальный kit-контрол (переключение темы меняет слоты — виджеты
перерисовываются теми же функциями).

### 7.1 Painter и WidgetState (FR-057, волна 2)

Draw-слой и машина состояний виджета переехали из потребителя (`kit_ui.rs`)
в крейт `canvas-ui` — миграции FR-058/059/060 кодируют против них, а не
копируют адаптер:

- **`canvas_ui::paint`** — `Painter` собирает `PaintItem::Rect`/`PaintItem::Text`
  как ДАННЫЕ (инвариант G7: без wgpu/winit, 0 внешних зависимостей;
  конвертацию в `CardInstance`/`OwnedText` выполняет крейт-потребитель).
  Порядок items = draw-порядок; `take_items()` отдаёт накопленное и очищает.
  `KitDraw` в `canvas-app` — тонкая обёртка над Painter (методы/поведение 1:1,
  эквивалентность quads/texts — тест `kitdraw_delegation_matches_direct_path`);
- **`canvas_ui::widget`** — `WidgetState`: переходы указателя/селекции/фокуса
  → `KitState` по детерминированной матрице приоритетов
  **Disabled > Pressed > Hovered > Selected > Normal** + ребро клика
  `clicked()` («press был внутри, release внутри»; press по disabled и press
  вне виджета клик не дают). `cursor_state`/`dropdown_item_state` в
  `kit_ui.rs` — deprecated-делегаты на `WidgetState` (потребители мигрируют
  в FR-059/060);
- **`canvas_ui::keyboard::FocusRing`** — Tab-порядок focus-rect'ов скоупа
  (`next`/`prev` по кольцу, `current`, `clear`): `KeyboardRouter` ведёт
  скоупы ПОВЕРХНОСТЕЙ, `FocusRing` — фокус контента внутри поверхности
  (рамка по слоту `accent` — решение потребителя). Существующие сигнатуры
  `keyboard.rs` не менялись (только добавление).

```rust
let mut p = Painter::new();
p.control(btn_rect, &button_style(variant, state, &palette));
p.label(btn_rect, &label, style.text, 13.0, PaintAlign::Center);
for item in p.take_items() { /* конвертация в инстансы рендера */ }

let mut w = WidgetState::default();
w.set_pointer(hovered, pressed_now);   // каждый кадр
w.set_selected(is_on); w.set_focused(ring.current() == Some(&rect));
let style = button_style(variant, w.kit_state(), &palette); // Disabled>Pressed>Hovered>Selected>Normal
if w.clicked(released_inside) { /* действие один раз на press→release */ }
```

### DebugOverlay (F-10, G6)

Тогл — **F9** (натив) / `?ui=debug` (web). По кадру реестра показывает:
рамки hit-rect'ов с подписью «L3·Panels / settings / элемент» (цвет по
слою), имя поверхности/элемента под курсором, подсветку пересечений
интерактивных rect'ов одного слоя (`overlaps_within_layer` — механика
G4-линта в рантайме). Оверлей не участвует в pick (поверхности в реестре нет —
диагностика не меняет ввод); рисуется полосой `UiLayer::Debug` (L8).
Модель чистая — headless-тесты.

### 7.2 Компоненты v2 (FR-058)

`canvas_ui::kit` (волна 2) — чистые модели/функции в стиле v1: геометрия +
стиль + модель состояния; рисование — через Painter (FR-057), ввод не
перехватывают, событий не владеют. Компоненты — только **добавление** к v1
(существующие сигнатуры/константы не меняются).

| Компонент | Функция | Контракт |
|---|---|---|
| `TextField` | `text_field(slot, min, max, model, placeholder, focused, state, p, m, fs, family, size)` | Модель `TextFieldModel { text, caret, sel }` + раскладка `TextFieldLayout { rect, text_area, caret_x, text_shown }`. `caret_x = -1.0` — каретка не рисуется (не в фокусе). |
| Список + скролл | `list_rows(area, s, row_h, gap, count) -> Vec<(usize, UiRect)>` + `scroll_bar(area, s, p) -> Option<UiRect>` | `ScrollState { offset, content_h, viewport_h }` — `scroll_by`/`clamp`/`needs_scroll`/`max_offset`. `list_rows` — чистая функция (без мутаций); частичные строки на краях включаются. |
| `Switch` | `switch(slot, on, state, p)` | `SwitchLayout { track, knob, track_style, knob_fill }`. `on` — позиция бегунка (вправо) и слот заливки трека (`control_primary` on / `control_fill` off); радиус `RADIUS_PILL`. |
| `Card` | `card(slot, min, max, header_h, p)` | `CardLayout { rect, header, body }`. Хедер и body — внутри пада панели (`panel_style(p).pad` = `SPACING_LG`). |
| `Icon` | `icon_glyph(i) -> &'static str` + `icon_button(slot, icon, align)` | `enum Icon { Close, Gear, Question, Search, Plus, ArrowLeft, ArrowRight, Refresh }`. Глифы — существующим шрифтом (NotoSansDisplay-Medium): 0 новых зависимостей (G7). `icon_button` делегирует `icon_button_rect` (квадрат `ICON_BUTTON_SIZE`). |

**Инвариант каретки** (зафиксирован в контракте FR-058): позиции `caret`/`sel`
в `TextFieldModel` — в **СИМВОЛАХ** (`chars().count()`), не байтах.
Вставка/удаление/движение корректны на юникоде (emoji 4-байтные, кириллица
2-байтная). IME/UTF-16-конвертация — на стороне ввода потребителя (тестируется
`text_field_unicode_emoji_and_cyrillic_positions`).

**Non-goals** (выведены в постановку при появлении потребителя): `Slider`
(спекулятивный компонент без экрана со слайдером). Витрина `kit_gallery`
обновляется во FR-059 (владелец волны 1 миграции).

### 7.3 Витрина: состав после FR-059 (волна 1 миграции)

Витрина `kit_gallery` показывает v1-секции (Buttons ×4 варианта ×4 состояния,
IconButtons, Chips, Dropdown, Toast, Tooltip) + секции компонентов v2:
**TextField** ×3 (Normal / Focused — каретка по `caret_x` / Disabled с
placeholder), **Switch** ×4 (Off/On × Normal/Hovered/Disabled), **Card**
(хедер + тело), **список+скролл** (8 строк в окне 3, выделенная строка,
демо-сдвиг — бегунок в треке), **Icon-глифы** ×4 (Search/ArrowLeft/
ArrowRight/Refresh). Контент витрины выше максимальной панели — колонка
секций прокручивается (`ScrollState`, колесо над контентом, шапка фиксирована;
при offset 0 — прежняя раскладка дословно). Поверхности волны 1
(`hints_ui`, `flowmap_ui`, `calc_panel_ui`) мигрированы на
`dropdown_menu`/`stack`/`list_rows`+`ScrollState`/`scroll_bar`, состояния
строк — `WidgetState`, отрисовка — Painter (§7.1).

## 8. Статус кита

- Готово (U1–U3, U5): каркас слоёв/реестра, примитивы, TextMeasurer, линты,
  миграция 6 поверхностей, KeyboardRouter на всей лестнице `on_key`.
- Готово (U4, FR-055): кит-виджеты F-8 (`canvas_ui::kit` + `anim`),
  DebugOverlay F-10 (G6 закрыт), витрина `kit_gallery`, хром пилотов
  (контейнер/пилюля what-if, «✕» галереи) — через kit-стили (явные слоты).
  Scissor-бакеты (F-5) и замер wasm-прироста — при следующей web-сборке
  (G7-остаток); TextInput кита — v2/за пределами PoC.
