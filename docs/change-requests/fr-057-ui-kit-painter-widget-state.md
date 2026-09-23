# FR-057: kit-core — Painter в canvas-ui (без wgpu) + WidgetState/фокус (перевод кита из «контрактов слотов» в виджеты)

- **Статус:** выявлено (постановка волны 2 кита; реализация по приказу владельца)
- **Тип:** FR
- **Приоритет:** критично
- **Владелец:** не назначен (любой агент — права на файлы см. отдельный раздел)
- **Источник:** гэп-анализ кита (сессия 2026-09-23); PRD-0009 §8 (F-8), §9.2; FR-055 (kit v1 — «кит НЕ рисует»)
- **Связанные задачи:** PRD-0009; FR-055 (kit.rs — стили/состояния), FR-053 (TextMeasurer/токены), FR-052 (KeyboardRouter); потребители — FR-058/FR-059/FR-060
- **Создан:** 2026-09-23
- **Обновлён:** 2026-09-23

## Описание
Кит v1 (FR-055) — чистые функции геометрии/стиля: рисование (`KitDraw`), подбор состояния по курсору (`cursor_state`) и фокус-логика лежат в потребителе (`canvas-app/src/kit_ui.rs:400–469, 472–479`; ~15 вызовов из app.rs). Каждая новая поверхность копирует этот адаптер. Требуется: (1) draw-слой в крейте `canvas-ui`, собирающий примитивы отрисовки **без wgpu** (крейт остаётся нуль-зависимым — G7); (2) машина состояний виджета (hover/pressed/focus/selected → `KitState` + ребро «клик»), чтобы миграции FR-059/060 не дублировали её. Выявлено агентом при гэп-анализе.

## Влияние
| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvas-ui` | новые модули `paint`, `widget` | `docs/ui-kit.md` §7 (новый подпункт) |
| `kit_ui.rs` (canvas-app) | `KitDraw` → тонкая обёртка над `Painter`; `app.rs` НЕ меняется (API `KitDraw` сохраняется дословно) | `docs/ui-kit.md` §2 |
| Потребители (FR-058/059/060) | кодируют против замороженных контрактов до слияния | этот FR |

## Анализ
- `kit_ui.rs:429–468` — `rect/control/label_center/label_left` дублируют то, что должно жить в крейте (конвертация в `CardInstance`/`OwnedText` — забота потребителя, сама модель — крейта).
- `kit_ui.rs:378–386` — `cursor_state(hovered, disabled)`: нет `pressed`/`focused`; зажатие и клик каждый потребитель отслеживает вручную.
- Фокуса виджетов нет нигде: `KeyboardRouter` (FR-051/052) маршрутизирует скоупы поверхностей, не контенты внутри них; Tab-навигации нет.
- `canvas-ui` не зависит от wgpu/winit (FR-051 G7) — Painter обязан остаться сборщиком данных.

## Требуемые изменения
| Что | Где | Как |
|---|---|---|
| Painter | `canvas-ui/src/paint.rs` (новый) | `PaintItem`/`Painter` по замороженному контракту; TDD (порядок items, take_items очищает) |
| WidgetState | `canvas-ui/src/widget.rs` (новый) | переходы указателя/селекции/фокуса → `KitState`; ребро клика; TDD-матрица переходов |
| FocusRing | `canvas-ui/src/keyboard.rs` | **только добавление** `FocusRing` (Tab next/prev по rect'ам скоупа); существующие сигнатуры не менять |
| Делегирование | `canvas-app/src/kit_ui.rs` | `KitDraw` делегирует `Painter` (методы и поведение 1:1); `cursor_state`/`dropdown_item_state` — deprecated-делегаты на `WidgetState` |
| Доки | `docs/ui-kit.md` | новый подпункт §7.1 «Painter и WidgetState» (FR-058 пишет §7.2 — секции не пересекаются) |

## Замороженные контракты (FR-058/059/060 кодируют против них до слияния)
```rust
// canvas_ui::paint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintAlign { Left, Center }

#[derive(Debug, Clone, PartialEq)]
pub enum PaintItem {
    Rect { rect: UiRect, fill: [f32; 4], border: [f32; 4], radius: f32 },
    Text { area: UiRect, text: String, color: [f32; 4], size: f32, align: PaintAlign },
}

pub struct Painter { /* items: Vec<PaintItem> */ }
impl Painter {
    pub fn new() -> Self;
    pub fn rect(&mut self, r: UiRect, fill: [f32; 4], border: [f32; 4], radius: f32);
    pub fn control(&mut self, r: UiRect, s: &ControlStyle);
    pub fn panel(&mut self, r: UiRect, s: &PanelStyle);
    pub fn label(&mut self, area: UiRect, text: &str, color: [f32; 4], size: f32, align: PaintAlign);
    pub fn items(&self) -> &[PaintItem];
    pub fn take_items(&mut self) -> Vec<PaintItem>;
}

// canvas_ui::widget
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WidgetState { /* поля приватные */ }
impl WidgetState {
    pub fn set_pointer(&mut self, inside: bool, pressed_now: bool);
    pub fn set_selected(&mut self, v: bool);
    pub fn set_disabled(&mut self, v: bool);
    pub fn set_focused(&mut self, v: bool);
    /// Приоритет: Disabled > Pressed > Hovered > Selected > Normal.
    pub fn kit_state(&self) -> KitState;
    pub fn is_focused(&self) -> bool;
    /// Ребро клика: press был внутри, release внутри. Вызывать на release.
    pub fn clicked(&mut self, released_now_inside: bool) -> bool;
}
// canvas_ui::keyboard — добавление
pub struct FocusRing { /* rect'ы в порядке Tab */ }
impl FocusRing {
    pub fn push(&mut self, r: UiRect);
    pub fn next(&mut self) -> Option<UiRect>;
    pub fn prev(&mut self) -> Option<UiRect>;
    pub fn current(&self) -> Option<&UiRect>;
    pub fn clear(&mut self);
}
```
Инвариант G7: `canvas-ui` не получает wgpu/winit и внешних зависимостей — `PaintItem` данные; конвертацию в `CardInstance`/`OwnedText` выполняет крейт-потребитель (как сегодня).

## Права на файлы (для параллельных агентов)
- **Вправе править:** `crates/canvas-ui/src/paint.rs` (новый), `crates/canvas-ui/src/widget.rs` (новый), `crates/canvas-ui/src/lib.rs` (только строки `pub mod paint; pub mod widget;`), `crates/canvas-ui/src/keyboard.rs` (только добавление `FocusRing`), `crates/canvas-app/src/kit_ui.rs`, `docs/ui-kit.md` (только подпункт §7.1).
- **Запрещено трогать:** `crates/canvas-app/src/app.rs`, `crates/canvas-render/**`, `crates/canvas-ui/src/kit.rs` (владелец FR-058), `hints_ui.rs`/`flowmap_ui.rs`/`calc_panel_ui.rs`/`autolink_ui.rs`/`palette.rs`/`explain_ui.rs` (владельцы FR-059/060).

## Зависимости
| Отношение | С кем |
|---|---|
| Кодировать | **сразу**, параллельно со всеми FR волны |
| Сливать | **до** FR-058/FR-059/FR-060 (они импортируют `paint`/`widget`) |
| Независим по файлам | FR-056 (canvas-render) |

## Точки входа
- `docs/ui-kit.md` — подпункт §7.1.
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 (история: волна 2).
- `worklog.md` — запись реализации.

## Проверка
- TDD: матрица переходов `WidgetState` (в т.ч. приоритет Disabled > Pressed > Hovered > Selected > Normal; ребро клика press→release внутри/снаружи); Painter: порядок/очистка items.
- Эквивалентность: тест «KitDraw до/после делегирования даёт те же quads/texts» на фиксированном примере (0 визуального скачка).
- `cargo test -p canvas-ui` + все гейты (fmt/clippy -D warnings/test/wasm/mcp-wasm) зелёные; **0 правок app.rs** (проверка диффом).

## История изменений
- `2026-09-23` — агент: создан документ (постановка волны 2 кита, FR-057), статус «выявлено»; контракты Painter/WidgetState/FocusRing заморожены.

## Источники истины
- `crates/canvas-app/src/kit_ui.rs` — `KitDraw` (400–469), `cursor_state`/`dropdown_item_state` (372–479), `theme_button_layout` (497–518).
- `crates/canvas-ui/src/kit.rs` — `KitState` (29–42), `ControlStyle`/`PanelStyle` (57–75).
- `crates/canvas-ui/src/keyboard.rs` — `KeyboardRouter` (13–22).
- `docs/prd/prd-0009-ui-layering-uikit.md` — §8 F-8, §9.2, Приложение А (карта переносов egui: text_edit → TextInput).
