# Suggest (Inline AI-подсказки) — UX/UI статус

**FR-079**: Inline AI-подсказки нод на канвасе (гибрид lex+Laya).
Эта страница — детальный UX/UI статус текущей реализации + план
редизайна по запросу владельца.

---

## 1. Текущая реализация (commit `20c5d2e` на момент анализа)

### 1.1 Компоненты

| Компонент | Где | Что делает |
|-----------|-----|------------|
| **C1 — попап подсказок FR-021** | `crates/canvas-app/src/hints_ui.rs` + `app/overlays.rs:1920` | При редактировании текста ноды (ввод формулы/описания) показывает попап с lex-подсказками (L0) + ИИ-строками (L1). ИИ-строки помечены `✦`. |
| **C3 — карточки «следующие ноды»** | `crates/canvas-app/src/suggest.rs:498-544` + `app/overlays.rs:1982` | После создания шаблонной ноды справа появляется стопка из до 3 карточек-предложений. |
| **Движок** | `crates/canvas-suggest/` (12 крейтов) | Гибрид lex v1/v2 + Laya (mm) с fusion α=0.85. На WASM — деградация до lex. |
| **Воркер** | `crates/canvas-app/src/suggest_worker.rs` | На нативе — отдельный поток с EventLoopProxy. На WASM — sync-путь в `about_to_wait`. |
| **Журнал S0** | `suggest::SuggestLog` | JSONL-лог `shown`/`accepted` событий (opt-out). |

### 1.2 Триггер карточек (C3)

**Сейчас**: карточки появляются **только** при создании новой шаблонной
ноды (`instantiate_template_at` → `suggest_request_cards(index)`).

```rust
// crates/canvas-app/src/app.rs:4987
self.suggest_request_cards(index);  // внутри instantiate_template_at
```

**Не появляется**: при выделении существующей ноды (Shift+клик, рамка).
Владелец хочет: при выборе ноды → карточки появляются справа.

### 1.3 Геометрия карточек

```rust
// crates/canvas-app/src/suggest.rs:491-496
pub const SUGGEST_CARD_W: f32 = 200.0;   // ширина
pub const SUGGEST_CARD_H: f32 = 30.0;   // высота
pub const SUGGEST_CARD_GAP: f32 = 6.0;  // зазор между
pub const SUGGEST_CARD_OFFSET_X: f32 = 12.0;  // отступ от правого края ноды
pub const SUGGEST_TOP_N: usize = 3;  // максимум карточек
```

Стопка позиционируется справа от якоря (последняя шаблонная нода).
При нехватке места справа — стопка уходит влево (`card_rects` кламп
по правому краю viewport).

### 1.4 Визуальный стиль карточек

```rust
// crates/canvas-app/src/app/overlays.rs:1996-2016
instances.push(CardInstance {
    pos: [rect.x, rect.y],
    size: [rect.w, rect.h],         // 200×30
    fill: palette.menu_fill,        // тёмный фон
    border: palette.palette_border,
    params: [6.0, 0.0, 0.0, 1.0],   // скругление 6px
    corners: [0.0; 4],
});
texts.push(OwnedScreenText {
    text: format!("✦ {}", cards.items[i].label),
    origin: [rect.x + 10.0, rect.y + 8.0],
    width: rect.w - 16.0,            // 184px под текст
    font_size: 12.0,
    color: palette.title,
    align: TextAlign::Left,
});
```

**Визуально**: маленький прямоугольник 200×30 с тёмным фоном, скруглённый
угол, текст `✦ ИмяШаблона`. Не похоже на ноду канваса — это чип-подсказка,
а не превью ноды.

### 1.5 Фильтрация и пороги

**`SuggestSettings`** (`crates/canvas-core/src/settings.rs:224`):

| Поле | Default | Что делает |
|------|---------|------------|
| `enabled` | `false` | Мастер-тумблер |
| `engine` | `Lex` | `Off`/`Lex`/`LexLaya` |
| `alpha` | `0.85` | Вес lex в fusion |
| `max_options` | `20` | Шортлист для L1 |
| `show_gate_min_ctx` | `120` | Мин. длина контекста (chars) |
| `log_suggest` | `true` | Журнал S0 |

**Поле `confidence_threshold` — ОТСУТСТВУЕТ.** Все 3 карточки (`SUGGEST_TOP_N`)
показываются всегда, если `options.is_empty()` — нет ни одной.

`SuggestAnswer` (`suggest.rs:52`):
- `template_key: String` — ключ шаблона
- `score: f64` — fused-скор (для HUD/лога)
- `confidence: f64` — Platt-калиброванная уверенность (lex-режим: 0.0)
- `source: ScoreSource` — `Lex` или `Fusion { alpha }`

### 1.6 Домен-гейт (C4)

`Verdict` (`canvas_suggest::domain::Verdict`):
- `ShouldSuggest` — домен расчётный, предлагать
- `Skip` — домен framework/bespoke, не предлагать
- `Unknown` — недостаточно данных

`show_allowed_for_cards(document, settings)` — гейт: показывает, если
`document.len() >= show_gate_min_ctx` (120 chars).

### 1.7 Empty state

**Сейчас**: если `options.is_empty()` (после shortlist) или `items.is_empty()`
(после фильтрации каталога) — карточки **не показываются**.
Никакого туултипа «AI suggestions not available» нет.

```rust
// crates/canvas-app/src/app.rs:5556-5558
if options.is_empty() {
    return;  // тихо — ничего не показываем
}
// ...
// crates/canvas-app/src/app.rs:5616-5618
if items.is_empty() {
    self.suggest.cards = None;
    return;  // тихо — ничего не показываем
}
```

### 1.8 Закрытие карточек

`close_suggest_cards` (`app.rs:5765`):
- Esc
- Клик мимо карточек (в канвасе)
- Новая инстанциация шаблона (старая стопка закрывается, новая открывается)
- Смена выделения — НЕ закрывает (карточки привязаны к якорю по id, а
  не к текущему selection)

### 1.9 Клик по карточке

`suggest_card_click` (`app.rs:5704`):
- Находит карточку под курсором
- Закрывает стопку
- Логирует `accepted`
- Вставляет шаблон справа от якоря (`world = [n.x + n.width + 60.0, n.y]`)
- Новая нода → снова `suggest_request_cards` (цепочка предложений)

---

## 2. Что не нравится владельцу (запрос 2026-10-02)

> «Сейчас мне не нравится как работает эта функция. Нужно детальное
> описание UX/UI. Мы изначально обсуждали формат когда при выборе ноды
> справа от неё появляются примерно 3 "призрачные" ноды (в зависимости
> выставленного от threshold уверенности, может быть и меньше) которые
> являются предложениями по добавлению на canvas. Если предложений
> нет, то надо выводить в тултипе, что AI дополнений нет.»

### 2.1 Расхождение с изначальным дизайном

| Аспект | Изначальный дизайн | Текущая реализация |
|--------|---------------------|---------------------|
| **Триггер** | При выборе ноды | Только при создании шаблонной ноды |
| **Визуал** | Призрачные ноды (полные превью) | Чипы 200×30 с текстом |
| **Количество** | ~3, зависит от threshold | Фиксировано 3 (`SUGGEST_TOP_N`) |
| **Empty state** | Тултип «AI дополнений нет» | Тихо, ничего не показывается |

### 2.2 Требуемые изменения

1. **Триггер на выбор ноды** — `suggest_request_cards` вызывать не только
   из `instantiate_template_at`, но и из обработчика клика/выделения ноды.
2. **Confidence threshold** — добавить поле `confidence_threshold: f64`
   в `SuggestSettings`. Фильтровать `SuggestAnswer` по `confidence >= threshold`.
   Количество карточек = `min(SUGGEST_TOP_N, surviving_count)`.
3. **Призрачные ноды** — рендерить карточки как полноценные превью нод
   (с заголовком, телом, параметрами, цветом), а не как чипы с текстом.
   Прозрачность/стиль — призрак (dashed border, пониженная alpha).
4. **Empty state тултип** — при `items.is_empty()` показывать тултип
   «AI дополнений нет» рядом с якорем (справа от ноды).

---

## 3. План редизайна (этапы)

### 3.1 Этап A — confidence_threshold (Rust, ~30 мин)

**Файлы**: `crates/canvas-core/src/settings.rs`, `crates/canvas-app/src/suggest.rs`

- Добавить поле `confidence_threshold: f64` (default 0.0 — показываем всё).
- В `on_cards_ready` фильтровать `answers` по `confidence >= threshold`.
- Кламп [0.0, 1.0] при загрузке (`#[serde(default)]` — старые конфиги
  читаются как 0.0).
- Тесты: round-trip default, фильтрация.

### 3.2 Этап B — Empty state тултип (~45 мин)

**Файлы**: `crates/canvas-app/src/suggest.rs`, `app/overlays.rs`

- Добавить поле `empty_tooltip: Option<String>` в `SuggestCards` (или
  отдельное поле в `SuggestState`).
- В `on_cards_ready` при `items.is_empty()` — ставить `empty_tooltip`
  вместо `cards = None`.
- В `suggest_cards_overlay` — рисовать тултип «✦ AI дополнений нет»
  (i18n ключи `SUGGEST_EMPTY_RU`/`SUGGEST_EMPTY_EN`).
- Закрытие тултипа — те же триггеры (Esc, клик мимо, новая инстанциация).

### 3.3 Этап C — Триггер на выбор ноды (~30 мин)

**Файлы**: `crates/canvas-app/src/app/input.rs`

- В `click_context_menu` (или где обрабатывается одиночный клик по ноде)
  добавить вызов `suggest_request_cards(index)` после установки selection.
- Условие: только если нода — шаблонная (`template.is_some()`); для
  пустых заметок suggest не запускается (нет контекста для домен-гейта).
- Закрывать старую стопку при смене selection (если якорь сменился).

### 3.4 Этап D — Призрачные ноды (дизайн, ~1 час)

**Файлы**: `crates/canvas-app/src/suggest.rs`, `app/overlays.rs`,
`crates/canvas-render/src/text.rs`

Это самый крупный этап — меняет визуальный стиль карточек с чипов на
полные превью нод:

- Размер: `SUGGEST_CARD_W × SUGGEST_CARD_H` → `node_width × node_height`
  (стандартные размеры ноды, например 240×120).
- Рендер: `CardInstance` с полным телом (header + body + params preview).
- Стиль: призрак — `border: dashed`, `fill.alpha = 0.6`, `params.w = 0`
  (без скругления углов как у реальных нод).
- Hover: подсветка рамки.
- Клик: вставляет шаблон (как сейчас).

**Не в скоупе этой сессии** — требует переделки геометрии стопки
(вертикальный стек 200×30 → горизонтальный/вертикальный стек
240×120) и адаптации клампа по viewport. Оставляем как design-only
для следующего коммита.

### 3.5 Этап E — Верификация

- `cargo check --workspace` + `clippy -- -D warnings` + `fmt --check`
- Юнит-тесты на confidence_threshold + empty tooltip
- Manual: trunk serve → создать шаблонную ноду → проверить карточки;
  выбрать ноду → проверить появление карточек;
  настроить `confidence_threshold = 1.0` → проверить тултип «нет».

---

## 4. Открытые вопросы

1. **Триггер на любой клик или только на шаблонные ноды?** — для пустых
   заметок нет контекста (домен-гейт отработает как Skip). Предлагаем:
   триггерить на любой клик, но карточки покажутся только если домен
   проходит (для пустых заметок — тихо, без тултипа «нет»).

2. **Confidence threshold по умолчанию** — 0.0 (показываем всё) или
   0.5 (умеренный фильтр)? Предлагаем 0.0 — пусть пользователь сам
   настроит через settings.

3. **Где тултип «AI дополнений нет» — у якоря или у курсора?**
   Предлагаем у якоря (правее ноды) — консистентно с положением карточек.

4. **Призрачные ноды — горизонтальный или вертикальный стек?**
   Вертикальный (как сейчас) экономит место, но требует высоты.
   Горизонтальный — уходит вправо, может выйти за viewport.
   Предлагаем: вертикальный, но если 3+ ноды не влезают по высоте —
   переключаться на 2 колонки.

---

## 5. Связанные документы

- `docs/change-requests/fr-079-inline-ai-suggest-engine.md` — изначальный FR.
- `docs/plans/fr-079-suggest-engine.md` — техплан (S1-S5 стадии).
- `crates/canvas-app/src/suggest.rs` — модель + геометрия карточек.
- `crates/canvas-app/src/suggest_worker.rs` — воркер.
- `crates/canvas-app/src/hints_ui.rs` — попап FR-021 (C1).
- `crates/canvas-app/src/app/overlays.rs:1982` — рендер карточек.
- `crates/canvas-suggest/` — движок (lex + fusion + domain + L1).
