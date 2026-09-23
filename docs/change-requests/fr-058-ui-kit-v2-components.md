# FR-058: Компоненты кита v2 — TextField, список+скролл, Switch, Card, Icon-глифы (canvas-ui, чистые модели)

- **Статус:** выявлено (постановка волны 2 кита; реализация по приказу владельца)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** не назначен (любой агент — права на файлы см. отдельный раздел)
- **Источник:** гэп-анализ кита (сессия 2026-09-23): потребители FR-059/060 требуют компонентов, которых нет в kit v1
- **Связанные задачи:** PRD-0009 §8 (F-8); FR-055 (kit v1 — стиль/паттерны), FR-057 (Painter/WidgetState — контракты), FR-053 (TextMeasurer)
- **Создан:** 2026-09-23
- **Обновлён:** 2026-09-23

## Описание
Для полного переноса кастомного UI на кит не хватает компонентов: текстовое поле с кареткой/селекцией (поиск, фильтр подсказок), строка списка со скролл-состоянием (hints, flowmap, calc_panel, autolink, explain, галереи), переключатель (настройки), контентная карточка с хедером (flowmap/explain/карточки-оверлеи), иконки для IconButton (сейчас глифы «✕»/«⚙» — текстовые литералы потребителей). Компоненты — чистые модели/функции в стиле kit v1: геометрия + стиль + модель состояния; рисование — через Painter (FR-057); ввод не перехватывают, событий не владеют. Выявлено агентом.

## Влияние
| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvas-ui::kit` | расширение компонентами v2 (только добавление) | `docs/ui-kit.md` §7 (подпункт §7.2) |
| Потребители | FR-059/060 мигрируют модули на эти компоненты | этот FR, FR-059/060 |
| Витрина | обновляется НЕ здесь (kit_gallery в app.rs — владелец FR-059) | FR-055 (контракт витрины) |

## Анализ
- `crates/canvas-ui/src/kit.rs` (748 строк) — только 8 компонентов v1; `rg "^pub"` не находит ни TextField, ни списка/скролла, ни Switch/Card/Icon.
- Скролла нет нигде: `layout.rs` — Row/Column без оффсета; переполнение сегодня = линт-ошибка, а не прокрутка.
- `kit.rs:267–282` — IconButton без глифов (только rect/style); литералы «✕» разбросаны по потребителям.
- Поле ввода: карта переносов egui (Приложение А FR-051) отводит `text_edit → TextInput`; модель поля — prerequisite миграции поиска/подсказок.

## Требуемые изменения
| Что | Где | Как |
|---|---|---|
| TextField | `kit.rs` (добавление) | модель `TextFieldModel` + раскладка `text_field(...)` — см. контракты |
| Список/скролл | `kit.rs` | `ScrollState` + `list_rows(...)` + `scroll_bar(...)` |
| Switch | `kit.rs` | `switch(...)` — трек/курок из слотов |
| Card | `kit.rs` | `card(...)` — хедер/тело от слота |
| Icon | `kit.rs` | `Icon` + `icon_glyph` + `icon_button` (глифы существующим шрифтом, 0 новых зависимостей) |
| Доки | `docs/ui-kit.md` §7.2 | таблица компонентов v2 + инвариант каретки |

## Замороженные контракты (FR-059/060 кодируют против них)
```rust
// --- TextField: модель (символы, не байты) + раскладка ---
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextFieldModel {
    pub text: String,
    pub caret: usize,                 // позиция в СИМВОЛАХ (chars().count())
    pub sel: Option<(usize, usize)>,  // (anchor, head) в символах
}
impl TextFieldModel {
    pub fn insert(&mut self, s: &str);
    pub fn backspace(&mut self);
    pub fn delete(&mut self);
    pub fn move_caret(&mut self, chars: isize, extend: bool);
    pub fn select_all(&mut self);
    pub fn clear_selection(&mut self);
    pub fn set_text(&mut self, s: String);
}

pub struct TextFieldLayout {
    pub rect: UiRect,
    pub text_area: UiRect,
    pub caret_x: f32,        // по замеру текста до каретки
    pub text_shown: String,  // с учётом placeholder/ellipsis
}
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    slot: UiRect, min: UiVec2, max: UiVec2, model: &TextFieldModel,
    placeholder: &str, focused: bool, state: KitState, p: &KitPalette,
    m: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem, family: &str, size: f32,
) -> TextFieldLayout;

// --- Список + скролл ---
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScrollState { pub offset: f32, pub content_h: f32, pub viewport_h: f32 }
impl ScrollState {
    pub fn scroll_by(&mut self, dy: f32);
    pub fn clamp(&mut self);
    pub fn needs_scroll(&self) -> bool;
    pub fn max_offset(&self) -> f32;
}
/// Видимые строки: (индекс, экранный rect). Чистая функция — без мутаций.
pub fn list_rows(area: UiRect, s: &ScrollState, row_h: f32, gap: f32, count: usize) -> Vec<(usize, UiRect)>;
/// Трек скроллбара — только когда needs_scroll.
pub fn scroll_bar(area: UiRect, s: &ScrollState, p: &KitPalette) -> Option<UiRect>;

// --- Switch ---
pub struct SwitchLayout { pub track: UiRect, pub knob: UiRect, pub track_style: ControlStyle, pub knob_fill: [f32; 4] }
pub fn switch(slot: UiRect, on: bool, state: KitState, p: &KitPalette) -> SwitchLayout;

// --- Card ---
pub struct CardLayout { pub rect: UiRect, pub header: UiRect, pub body: UiRect }
pub fn card(slot: UiRect, min: UiVec2, max: UiVec2, header_h: f32, p: &KitPalette) -> CardLayout;

// --- Icon (глифы существующего шрифта; литералы потребителей переносятся сюда) ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon { Close, Gear, Question, Search, Plus, ArrowLeft, ArrowRight, Refresh }
pub fn icon_glyph(i: Icon) -> &'static str;
pub fn icon_button(slot: UiRect, icon: Icon, align: (HAlign, VAlign)) -> UiRect;
```
Инварианты: каретка/селекция — в символах (IME/UTF-16-конвертация — на стороне ввода потребителя, зафиксировать в доке); список чистый (без мутаций); Switch/Card — только геометрия и слоты; никаких новых внешних зависимостей (G7); Slider — НЕ включён (спекулятивный компонент без потребителя — вернуть в постановку при появлении экрана со слайдером).

## Права на файлы (для параллельных агентов)
- **Вправе править:** `crates/canvas-ui/src/kit.rs` (**только добавление** — существующие сигнатуры/константы не менять), `docs/ui-kit.md` (только подпункт §7.2), `#[cfg(test)]`-секция kit.rs.
- **Запрещено трогать:** `paint.rs`/`widget.rs`/`keyboard.rs`/`lib.rs` (владелец FR-057), `canvas-app/**`, `canvas-render/**`.

## Зависимости
| Отношение | С кем |
|---|---|
| Кодировать | **сразу** — против контрактов этого FR + контрактов FR-057 (Painter/WidgetState) |
| Сливать | **после** FR-057 (импортирует `paint`), **до** FR-059/060 |
| Независим по файлам | FR-056 (canvas-render) |

## Точки входа
- `docs/ui-kit.md` — подпункт §7.2 (компоненты v2).
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 (история: волна 2).
- `worklog.md` — запись реализации.

## Проверка
- Юнит-тесты: TextField (вставка/удаление/селекция/движение каретки на юникоде — emoji/multi-byte); ScrollState (кламп краёв, max_offset, needs_scroll); list_rows (оффсет → корректные индексы/rect'ы, частичные строки); switch/card (геометрия в слоте, мин/макс); icon_glyph (маппинг полон).
- Гейты (fmt/clippy -D warnings/test/wasm/mcp-wasm) зелёные; **0 правок canvas-app/canvas-render** (проверка диффом).

## История изменений
- `2026-09-23` — агент: создан документ (постановка волны 2 кита, FR-058), статус «выявлено»; контракты компонентов v2 заморожены; Slider выведен в non-goal до появления потребителя.

## Источники истины
- `crates/canvas-ui/src/kit.rs` — стиль v1, метрики (117–138), IconButton (267–282).
- `crates/canvas-ui/src/measure.rs` — `TextMeasurer` (32–70) — текст только через него.
- `crates/canvas-ui/src/layout.rs` — Row/Column/stack/constrain (27–252) — база для list_rows/card.
- `docs/ui-kit.md` §7 — состав кита v1 (для §7.2).
