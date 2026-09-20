# FR-046: Design-токены — примитивы, `ThemeColors` v2 и миграция бродячих цветовых констант (PoC PRD-0006)

- **Статус:** выполнено (v1: F-1..F-7; F-8/F-9 — следующий этап D4)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (постановка по PRD-0006 и запросу владельца, сессия 2026-09-21)
- **Источник:** PRD-0006 (`docs/prd/prd-0006-design-system-tokens.md`, статус «в анализе», этап D0 дорожной карты) — запрос владельца: «Можем ли мы взять перечень из UI библиотек и на основании неё реализовать свою дизайн-систему?»
- **Связанные задачи:** PRD-0004 §F-1 (токены анатомии — первый потребитель метрик, стык §7.2 PRD-0006), CR-007 (контраст WCAG, урок), FR-038 (эталонный паттерн добавления слота: `guide_align`/`guide_grid` + тесты ≥3:1), FR-039 (модалка настроек — карточки тем), FR-040 (i18n), FR-042/FR-044/FR-045 (параллельные волны правки рендера — координация слияний), FR-014 (value-ребро), FR-016 (severity), FR-017 (what-if), FR-022 (wheel), CR-004 (хром виджета), ADR-0008 (allowlist — новых зависимостей нет), ADR-0011 (wasm-гейт), SPEC §6.2–6.3
- **Создан:** 2026-09-21
- **Обновлён:** 2026-09-21
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Реализация PoC PRD-0006 (этапы D1–D5): система design-токенов из трёх слоёв как
единая точка правды визуальных характеристик CanvasDesk.

1. **Слой примитивов (данные).** `design/tokens/*.json` — шкалы цветов (серый
   ×12, акцент ×12, функциональные error/warning/success/info), радиусы,
   типографика, длительности, альфа-ступени; значения стартуют равными текущим
   константам (ноль визуального скачка). Rust-зеркало — платформенно-нейтральный
   `canvas-core::tokens` с тестом паритета JSON↔Rust.
2. **Слой семантики.** `ThemeColors` v2 (`canvas-render/src/theme.rs`) — к
   текущим ~30 слотам добавляются: `accent`, `accent_soft`, `edge_default`,
   `edge_flow`, `edge_focus`, `edge_draft`, `selection`, `selection_fill`,
   `broken`, `error`, `hud`, `dialog_fill`, `dialog_border`, `dialog_text`,
   `toast_text`, `whatif_fill`, `whatif_badge`, `wheel_*`, `select_rect_fill`,
   `select_rect_border`; обе палитры (`dark()`/`light()`) собираются из
   примитивов.
3. **Миграция.** Все ~25 бродячих цветовых констант (перечень — PRD-0006 §2.1)
   переводятся на слоты палитры; билдеры, которым тема сегодня не передаётся,
   получают `&ThemeColors` параметром. Акцент становится единым источником —
   включая виджет-мост `ThemeInfo.accent` (сейчас `#3B82F6` захардкожен).
4. **Гарантии.** Контраст-тесты расширены на все новые слоты (обе темы,
   паттерн FR-038); токен-линт `scripts/token_lint.sh` защищает G1 от регресса.

Кто выявил: владелец (запрос о дизайн-системе) + агент (аудит визуального стека
2026-09-21, фактура — в PRD-0006 §2.1 и настоящем FR).

Скоуп: must-требования F-1..F-6 PRD-0006 целиком; should F-7 (линт) и F-8
(пресеты) — включены; F-9 (web CSS + persist) — включён последним этапом при
зелёных гейтах. V2 (кастомные палитры, импорт VSCode/Obsidian, color-picker) —
вне скоупа (PRD-0006 F-10).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `design/tokens/` | Новый каталог: JSON-файлы примитивов (цвет/размер/время) | PRD-0006 §7.1, `docs/prd/README.md` |
| `canvas-core::tokens` | Новый публичный модуль-зеркало примитивов + тест паритета | PRD-0006 §7.2, SPEC §6 |
| `canvas-render::theme` | `ThemeColors` v2: +~20 слотов; `dark()`/`light()` из токенов; новые контраст-тесты | PRD-0006 F-2, CR-007 |
| `canvas-render::cards` | Константы (`SELECTION_BORDER`, `BROKEN_BORDER`, `EDGE_COLOR`, `FLOW_EDGE_COLOR`, `DRAFT_COLOR`, `FOCUS_EDGE_COLOR`, `WIDGET_CHROME_HOVER_FILL`, `DROP_GHOST_*`, severity) → слоты/компонентные палитры; метрики `HEADER_HEIGHT`/`CORNER_RADIUS` — из `canvas-core::tokens` (алиасы с сохранением имён) | PRD-0006 F-3/F-6 |
| `canvas-render::text` | `RESULT_ERROR_COLOR` → `theme.error`; `HUD_COLOR` → `theme.hud`; размеры текста — из токенов (значения те же) | PRD-0006 F-3/F-6 |
| `canvas-render::renderer` | `TEXT_SELECTION_FILL`/`HIGHLIGHT_FILL`/`WHATIF_*` → слоты | PRD-0006 F-3 |
| `canvas-render::minimap` | Компонентная `MinimapPalette::from_theme` вместо 7 констант | PRD-0006 F-3 (Q4) |
| `canvas-app` (`lib.rs`, `app.rs`) | `SELECT_RECT_*`, wheel-палитра (закрытие открытого вопроса `app.rs:4949-4951`), диалоги/тосты, group-drop-подсвет, пульс результата → слоты; прокидывание темы | PRD-0006 F-3 |
| `canvas-app::widgets` | `ThemeInfo::new(dark, "#3B82F6")` ×2 → акцент из палитры | PRD-0006 F-4, CR-004 |
| `canvas-web` | `index.html` hex → CSS-переменные из токенов; persist конфига на web (F-9) | PRD-0006 F-9 |
| Темы-пресеты | 2–3 пресета как данные + карточки в модалке FR-039 | PRD-0006 F-8, Q3 |
| `scripts/token_lint.sh` | Новый локальный гейт (hex-литералы вне токенов/темы/тестов) | PRD-0006 F-7 |
| `.canvas` / MCP | Не затрагиваются (темы — свойство окружения, не контент) | SPEC §5.1 |
| `config.toml` | Схема не расширяется (кастомные палитры — V2) | FR-039 |
| Онбординг/user-docs | Решается вопросом владельцу при завершении (правило AGENTS.md) | AGENTS.md |

## Анализ (Root Cause — чего нет в коде)

Якоря — по аудиту 2026-09-21 (полная фактура: PRD-0006 §2.1/§2.3). Существующая
инфраструктура: `ThemeColors` (`theme.rs:11-206`) — Copy-структура, `dark()`/
`light()` с литеральными значениями; смена темы горячая (`app.rs:8206-8216`,
`renderer.rs:418`); контраст-машина `readable_on_card` (`theme.rs:170-176`,
`contrast.rs`) и тесты (`theme.rs:286-361`); WGSL color-free — цвета приходят
per-instance/юниформами (`cards.wgsl:15-22`, `grid.wgsl:7-19`).

1. **Модуля примитивов нет.** Шкал (серый/акцент/функциональные) не существует:
   значения захардкожены прямо в `dark()`/`light()` (`theme.rs:83-155`) и в
   бродячих константах. JSON-источника правды нет; PRD-0004 §F-1 планирует
   токены анатомии — модуль-получатель отсутствует.
2. **Слотов семантики не хватает.** Сегодняшние ~30 слотов не покрывают:
   выделение (`SELECTION_BORDER`, `cards.rs:24`), битуя рамку (`:26`), рёбра
   (`:643,646,666` — и `DRAFT_COLOR` `:649`), хром виджетов (`:592`),
   drop-ghost (`:1060-1064`), severity (`:42-102` — 3 уровня × 2 темы +
   тексты), ошибку (`text.rs:108`), HUD (`text.rs:140`), селекцию текста/
   подсветку/what-if (`renderer.rs:41-49`), wheel (`app.rs:4952-4957` — в коде
   оставлен комментарий «открытый вопрос FR-022»), диалоги/тосты
   (`app.rs:10726-10799`), select-рамку (`lib.rs:495-497`), минимапу
   (`minimap.rs:35-47`).
3. **Тема не везде доступна.** Билдеры `build_edge_instances`,
   `build_port_instances`, `build_line_port_instances`,
   `build_edge_handle_instances`, `build_draft_instances` (`cards.rs:891-1051`)
   не принимают `&ThemeColors` — потому и живут на константах; миграция
   требует прокидывания параметра (не глобала — инвариант I-4).
4. **Акцент размножен.** `[0.396,0.612,0.969,·]` в 9 местах + `#3B82F6`
   (`widgets.rs:109,138`) + `#8ab4ff` (`index.html:156`); единого слота нет —
   G4 недостижим без F-4.
5. **Метрики вне токенов.** `HEADER_HEIGHT 34.0`, `CORNER_RADIUS 8.0`
   (`cards.rs:19-21`), типографика 16/22, 14/20, 12/16 (`text.rs:66-102`),
   длительности (`animate.rs`) — константы без общей точки (стык F-6).
6. **Защиты от регресса нет.** Ни линта, ни аудита: каждая волна (FR-016/017/
   022, what-if) добавляла новые литералы. `scripts/token_lint.sh` отсутствует.

## Решения (зафиксированы)

- **Р-1. Формат токенов — упрощённый W3C-совместимый JSON.** Файлы
  `design/tokens/colors.json`, `dimensions.json`, `motion.json`; группа —
  `{ "$type": "color|dimension|duration|opacity", "$value": …, "$desc": … }`.
  Полная DTCG-валидация и тулинг (Style Dictionary) — V2 (PRD-0006 §7.3, Q1).
- **Р-2. Размещение — `design/tokens/` в корне репо** (Q2 по предложению);
  `canvas-core::tokens` зеркалит значения константами; тест паритета читает
  JSON `include_str!("../../../design/tokens/colors.json")` + `serde_json`
  (уже в стеке — ADR-0008 не нарушается; парсинг только в тестах, рантайм
  ядра констант не читает — wasm-гейт не страдает).
- **Р-3. Зеркало в `canvas-core`, не в `canvas-render`.** Токены — данные без
  GPU/ОС (правило архитектуры №1); `canvas-render` уже зависит от `canvas-core`
  (проверяется на T-046.1; при отсутствии — добавляется workspace-зависимость,
  внешних крейтов нет). Метрики — тоже токены: `CARD_CORNER_RADIUS`,
  `CARD_HEADER_HEIGHT`, типографика, длительности — `canvas-render` берёт их
  через алиасы с сохранением текущих имён (`pub use`), чтобы дифф был
  минимальным.
- **Р-4. Слоты `ThemeColors` v2** (имена — публичные поля, значения — из
  примитивов): `accent` (rgba из `accent.solid`), `accent_soft` (α-ступень),
  `selection` (рамка = accent), `selection_fill`, `broken`, `edge_default`,
  `edge_flow`, `edge_focus`, `edge_draft`, `error`, `hud`, `dialog_fill`,
  `dialog_border`, `dialog_text`, `toast_text`, `whatif_fill`,
  `whatif_badge` (Color), `select_rect_fill`, `select_rect_border`,
  `wheel_dim`, `wheel_category`, `wheel_template`, `wheel_hover`,
  `wheel_hub_active`, `wheel_border`, `highlight` (`==текст==`).
  `group_fill`/`group_border` уже есть — переводятся на accent-примитив.
- **Р-5. Severity — компонентная таблица от функциональных примитивов.**
  `SeverityPalette` (3 уровня: warning/danger/critical — рамки + тексты
  бейджей) строится из `ThemeColors::severity(&self)`; значения = текущие
  `SEVERITY_DARK/LIGHT` (ноль скачка), но точка правды — палитра. FR-016
  контракт LOD (рамка ≥0.25, бейджи ≥0.6) не трогается.
- **Р-6. Минимапа — `MinimapPalette::from_theme(&ThemeColors)`** (Q4 по
  предложению): фон/карточка/текст/группа/broken/viewport/ребро выводятся из
  семантики (значения = текущие константы `minimap.rs:35-47`); CPU-растер
  получает палитру параметром.
- **Р-7. Акцент.** Единый примитив `accent` (тёмная тема: текущий
  `[0.396,0.612,0.969]`; светлая: свой). Все 9 акцентных литералов, групповые
  рамки, `ThemeInfo.accent` (hex-строка из слота — новая функция
  `Color::to_hex()`), `SELECT_RECT_*` — из слота. Web `#8ab4ff` — F-9.
  Авто-«чернила» на акцентных заливках — существующий `readable_on_card`.
- **Р-8. Прокидывание темы.** Билдеры `cards.rs` получают `&ThemeColors`
  параметром (не глобалом, не static); изменения сигнатур — внутренние
  (`pub(crate)`), внешние вызовы из `renderer.rs` передают существующую
  `self.theme`.
- **Р-9. Пресеты (F-8).** Пресет = JSON поверх примитивов (переопределяет
  акцентную/серую шкалу и/или отдельные слоты) + регистрация в перечне;
  в v1 — 2 пресета-кандидата (Nord dark, Catppuccin Mocha; состав Q3 —
  демо-гейт), выбор — карточки рядом с тёмной/светлой (модалка FR-039).
  Контраст-тесты гоняются по каждому пресету автоматически.
- **Р-10. Web (F-9).** `index.html`: 6 hex → CSS-переменные (`--cd-bg`,
  `--cd-btn-bg`, `--cd-btn-text`, `--cd-btn-border`, `--cd-btn-hover`,
  `--cd-focus`), значения — комментарии-блок «сгенерировано из токенов» +
  скрипт `scripts/web_css_tokens.sh` (проверка паритета). Persist: подключить
  сохранение `localStorage["canvasdesk.config"]` на смене настроек web
  (`app_spawn.rs:170` — `config_path: None`) — тем же ключом, `normalize()`
  web выравнивается с десктопом.

## Требуемые изменения (Changes)

| Что → Где → Как | Этап |
|---|---|
| `design/tokens/{colors,dimensions,motion}.json` — создать; значения примитивов = текущие константы (соответствие фиксируется комментариями `$desc` с координатами источника) | T-046.1 |
| `crates/canvas-core/src/tokens.rs` — создать: `pub mod tokens` (шкалы, радиусы, типографика, длительности, альфа) + `#[cfg(test)]` паритет-тест JSON↔Rust | T-046.1 |
| `crates/canvas-render/Cargo.toml` — зависимость `canvas-core` (если отсутствует) | T-046.1 |
| `crates/canvas-render/src/cards.rs` — `pub use canvas_core::tokens::{…}` для `HEADER_HEIGHT`/`CORNER_RADIUS`; удалить цветовые константы, билдеры принимают `&ThemeColors` (`card_instance`, `build_edge_instances`, `build_port_instances`, `build_line_port_instances`, `build_edge_handle_instances`, `build_draft_instances`, drop-ghost хелперы, severity) | T-046.2 |
| `crates/canvas-render/src/theme.rs` — `ThemeColors` v2 (+слооты Р-4, `SeverityPalette`, пресеты); `dark()`/`light()` из `canvas_core::tokens`; контраст-тесты на новые слоты (паттерн `:286-313`); тест «легаси-значения == прежним константам» | T-046.2 |
| `crates/canvas-render/src/text.rs` — `RESULT_ERROR_COLOR`/`HUD_COLOR` → `theme.error`/`theme.hud`; размеры — из токенов | T-046.2 |
| `crates/canvas-render/src/renderer.rs` — `TEXT_SELECTION_FILL`/`HIGHLIGHT_FILL`/`WHATIF_*` → слоты; передать тему в билдеры | T-046.2 |
| `crates/canvas-render/src/minimap.rs` — `MinimapPalette::from_theme`; вызов в точке построения кадра (`app.rs:3959-3961`) | T-046.2 |
| `crates/canvas-app/src/lib.rs` — `SELECT_RECT_FILL/BORDER` → слоты (оверлей `app.rs:10870-10878`) | T-046.3 |
| `crates/canvas-app/src/app.rs` — wheel-палитра из темы (`wheel_overlay`), диалоги/тосты (`:10726-10799`), group-drop (`:10886-10887`), пульс результата (`:10853-10867`) → слоты; `widgets.set_theme(dark, accent_hex)` | T-046.3 |
| `crates/canvas-app/src/widgets.rs` — `ThemeInfo::new(dark, "#3B82F6")` ×2 → акцент из палитры (`set_theme(dark, accent)`) | T-046.3 |
| `scripts/token_lint.sh` — hex-литералы (`0x[0-9a-fA-F]{6}`, `#RRGGBB`) вне белого списка (tokens.rs/theme.rs/тесты/шейдеры/Win32-коды shell-крейта) = ошибка | T-046.3 |
| Пресеты: `design/tokens/presets/{nord,catppuccin-mocha}.json` + загрузка в `ThemeColors` + карточки в `settings_ui.rs` (модалка FR-039) + i18n-ключи (FR-040) | T-046.4 |
| `crates/canvas-web/index.html` — CSS-переменные из токенов; `app_spawn.rs` — persist конфига; `scripts/web_css_tokens.sh` — паритет | T-046.4 |
| Документы: SPEC §6 (подраздел «Палитра и токены»), ACCEPTANCE.md (§ волны FR-046), index-cr-fr.md (статус), PRD-0006 (статус/история), interface-objects (сверка упоминаний цветов) | T-046.5 |

Порядок: T-046.1 → T-046.2 → T-046.3 → T-046.4 → T-046.5; каждый этап —
отдельная ветка `feature/fr-046-*`, полный гейт, merge --no-ff
(паттерн волны FR-038). TDD: паритет-тест и контраст-тесты пишутся ДО
миграции значений.

## Инварианты

1. **I-1. Ноль визуального скачка:** значения всех мигрируемых констант не
   меняются; отличие допускается только через протокол F-5 PRD-0006
   (документированное исключение с решением владельца).
2. **I-2. Нейтральность ядра:** `canvas-core::tokens` — только данные
   (`const`), без GPU/ОС/serde-рантайма; `scripts/wasm_gate.sh` зелёный.
3. **I-3. Шейдеры не меняются:** WGSL вне скоупа (цвета — инстансы/юниформы);
   единственная шейдерная цветовая константа (тень, `cards.wgsl:105`) — вне
   скоупа PRD-0006.
4. **I-4. Тема — параметр, не глобал:** прокидывание `&ThemeColors`
   параметрами; никаких `static`/`OnceLock`-палитр.
5. **I-5. Паритет обязателен:** расхождение JSON↔Rust = красный тест.
6. **I-6. Контраст автотестом:** каждый новый слот покрыт тестом ≥3:1
   (графика) / ≥4.5:1 (текст) на обеих темах и пресетах; исключения —
   документированы, не замолчаны.
7. **I-7. Совместимость настроек:** схема `config.toml` не расширяется;
   старые конфиги читаются без миграций.
8. **I-8. Round-trip `.canvas` не затрагивается** (темы — не контент).
9. **I-9. `ThemeColors` остаётся `Copy`;** построение — только
   `dark()`/`light()`/пресеты; горячая смена темы (`set_theme`) не деградирует.
10. **I-10. Этап = ветка = полный гейт:** fmt/clippy/test/wasm_gate/
    mcp_wasm_gate перед каждым merge; push только после локальных гейтов.

## Точки входа

- `docs/prd/prd-0006-design-system-tokens.md` — статус → «в работе (FR-046 создан)» на D0; запись в §16 на каждом этапе; DoD §15 — на D5.
- `docs/prd/README.md` — статус строки PRD-0006.
- `docs/change-requests/index-cr-fr.md` — строка FR-046 + заметка о нумерации (следующий — FR-047).
- `docs/SPEC.md` §6 — подраздел «Палитра и design-токены» (после D2–D3).
- `docs/ACCEPTANCE.md` — приёмка волны FR-046 (G1–G5 замеры) на T-046.5.
- `docs/interface-objects/*.md` — сверка упоминаний конкретных цветов на T-046.5.
- `user-docs/` и онбординг — явный вопрос владельцу при завершении (AGENTS.md; темы/акцент касаются страницы настроек).
- `docs/DEPENDENCIES.md` — без изменений (новых зависимостей нет).

## Отклонения реализации (фиксируются по протоколу PRD-0006)

1. **Р-4 сокращён до фактических слотов темы.** В `ThemeColors` добавлены
   только слоты, потребляемые через тему в v1: `accent`, `selection_fill`,
   `highlight`, `whatif_fill`, `whatif_badge`, `error`, `hud`. Акцентное
   семейство с одинаковыми значениями в обеих темах (рёбра, draft, хром
   виджетов, drop-ghost, select-рамка, группы) мигрировано на **алиасы
   примитива** `canvas_core::tokens::ACCENT` — единый источник на этапе
   компиляции (G4 достигнут), прокидывание `&ThemeColors` в билдеры cards.rs
   (полная F-2/F-3) — v2 (D4+, вместе с пресетами, перекрашивающими акцент).
2. **Санкционированное визуальное изменение одно** (F-4/G4): акцент
   виджет-моста `#3B82F6` → `#659CF7` (hex примитива ACCENT). Остальные
   значения — бит-в-бит прежним (I-1).
3. **Контраст-исключения (F-5)** — существующие пары, не проходящие WCAG,
   значения НЕ менялись; регрессионные границы зафиксированы тестом
   `v2_slots_contrast_documented`: accent к светлому фону 2.52:1, hud к
   светлому фону 2.51:1, error к card_fill 4.32/3.38 (против AA 4.5).
   Подстройка значений — отдельное решение владельца.
4. **F-8 (пресеты) и F-9 (web CSS/persist) — не реализованы в v1**, переносятся
   на этап D4 (§13 PRD-0006): пресеты требуют решения владельца по составу (Q3)
   и прокидывания темы в акцентное семейство (отклонение 1).
5. **F-7 линт** — по фактическим исключениям (риск R5): Win32-домены,
   тест-модули, комментарии, дата-контракт манифестов шаблонов
   (`templates::DEFAULT_TEMPLATE_COLOR`, FR-018).

## Проверка (Verification)

Автоматические гейты (каждый этап):
- `cargo fmt --all --check`; `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`; `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace`; `bash scripts/wasm_gate.sh`; `bash scripts/mcp_wasm_gate.sh` (последний — на этапах, трогающих scene/mcp-зависимые крейты).

Функциональная проверка:
- [x] Тест паритета `design/tokens/*.json` ↔ `canvas_core::tokens` зелёный (4 теста).
- [x] Контраст-тесты покрывают новые слоты (обе темы); известные исключения задокументированы (см. Отклонения п.3).
- [x] G1: `scripts/token_lint.sh` зелёный; цветовых литералов вне токенов/исключений — 0 (было ~25).
- [x] G4: `ThemeInfo.accent` приходит из палитры (`tokens::ACCENT`); select-рамка/порты/каретка/хром/drop-ghost — от единого примитива.
- [x] G5: `HEADER_HEIGHT`/`CORNER_RADIUS`/типографика — алиасы токенов; значения == прежним (паритет-тест).
- [ ] G2: пресет-файл без правок рендера — этап D4 (не реализовано в v1).
- [x] Гейты: fmt/clippy -D warnings/test --workspace (app lib 229, core 253, render 288)/wasm_gate --check/mcp_wasm_gate --check/token_lint — зелёные.
- [x] Round-trip существующих `.canvas` не изменён (тесты canvas-core зелёные).
- [ ] Демо-сценарий PRD-0006 §6 (ручной осмотр владельцем) — по завершении D4.
- [x] Онбординг/user-docs: вопрос владельцу задан в итоговом ответе (AGENTS.md).

## История изменений

- `2026-09-21` — агент: создан документ (FR-046), статус `в анализе`; реализационная постановка PRD-0006 (этап D0): анализ бродячих констант с координатами, решения Р-1..Р-10, план T-046.1..5, 10 инвариантов, точки входа, проверка.
- `2026-09-21` — агент: статус → `в работе` (T-046.1: примитивы design/tokens + canvas-core::tokens + паритет-тесты).
- `2026-09-21` — агент: статус → **`выполнено (v1)`**; T-046.2 (ThemeColors v2 + миграция рендера + контраст-тесты F-5) и T-046.3 (миграция canvas-app + единый акцент G4 + токен-линт F-7) выполнены; отклонения зафиксированы (Р-4 сокращён до фактических слотов; акцент-семейство — алиасы примитива; F-8/F-9 → D4; санкционированное изменение виджет-акцента #3B82F6 → #659CF7); гейты: fmt/clippy/test --workspace/wasm_gate/mcp_wasm_gate/token_lint — зелёные.

## Источники истины

- PRD-0006 — `docs/prd/prd-0006-design-system-tokens.md` (§2.1 аудит, §7 решение, §8 F-1..F-10, §13 D0–D5, §15 DoD) — постановка настоящего FR.
- PRD-0004 — `docs/prd/prd-0004-node-anatomy-restructure.md` §F-1 (токены анатомии), §7.3 — стык метрик.
- CR-007 — урок контраста (WCAG 1.4.11): `theme.rs:286-361` (тесты), `contrast.rs` (машина).
- FR-038 — `docs/change-requests/fr-038-snap-grid-alignment.md`: паттерн добавления слота `guide_align`/`guide_grid` с тестами.
- Код: `crates/canvas-render/src/theme.rs:11-206,286-361`; `cards.rs:14-26,42-102,136-156,585-600,640-666,1060-1064`; `text.rs:35-142`; `renderer.rs:39-52,418`; `minimap.rs:35-47`; `guides.rs:177-187`; `grid.rs:52-90`; `crates/canvas-app/src/lib.rs:495-497`; `app.rs:4948-4960,8206-8216,10726-10799,10853-10887`; `widgets.rs:105-140`; `crates/canvas-web/index.html:101-156`; `app_spawn.rs:170`; `crates/canvas-core/src/settings.rs:44-53,196-283`.
- ADR-0008 (allowlist — serde_json уже в стеке), ADR-0011 (wasm-гейт), SPEC §6.2–6.3.
- Внешние (таксономия, не реализации): W3C Design Tokens CG, Material 3 tokens, Radix Colors, shadcn/ui theming, IBM Carbon governance.

