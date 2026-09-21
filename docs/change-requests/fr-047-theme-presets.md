# FR-047: Темы-пресеты как данные — 7 встроенных палитр в модалке настроек (этап D4 PRD-0006)

- **Статус:** выполнено
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (постановка и реализация по запросу владельца, сессия 2026-09-21)
- **Источник:** запрос владельца (сессия 2026-09-21): «Теперь хочу чтобы мы реализовали 5-7 тем-пресетов типа Встроенные пресеты: Nord, Dracula, Catppuccin (Mocha/Latte), Solarized (Dark/Light), Tokyo Night, Gruvbox, Monokai, GitHub Light/Dark; VSCode Dark Modern / Dark+» — решение открытого вопроса Q3 PRD-0006 (состав пресетов F-8); этап D4 роадмапа PRD-0006
- **Связанные задачи:** PRD-0006 §8 (F-8, G2, G3, US-3, AC-3.1/3.2, D4), FR-046 (design-токены v1 — фундамент), FR-039 (модалка настроек — поверхность выбора), FR-040 (i18n), CR-007 (урок контраста), FR-038 (паттерн цветовых слотов с тестами)
- **Создан:** 2026-09-21
- **Обновлён:** 2026-09-21

## Описание (What)

Конечный пользователь хочет работать в привычной палитре: выбрать одну из известных публичных тем (Nord, Dracula, Catppuccin, Solarized, Tokyo Night, Gruvbox) вместо двух классических (тёмная/светлая). Владелец зафиксировал состав: 5–7 пресетов из перечня-кандидатов. Запрос выявлен владельцем напрямую и закрывает открытый вопрос Q3 PRD-0006 («состав встроенных пресетов F-8 — решает владелец»): принято 7 пресетов — Nord, Dracula, Catppuccin Mocha, Catppuccin Latte, Solarized Light, Tokyo Night, Gruvbox Dark (5 тёмных + 2 светлых); Monokai, GitHub Light/Dark, VSCode Dark Modern/Dark+, Solarized Dark остаются в пуле кандидатов и добавляются позже тем же механизмом (JSON + одна строка реестра).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| Настройки (модалка FR-039, таб «Внешний вид») | Новая dropdown-строка «Тема-пресет» над строкой языка; карточки тёмной/светлой остаются, гасят пресет при клике | SPEC §6.6; docs/prd/README |
| config.toml | Новое поле `theme_preset: String` (serde default — пустая строка = классика); round-trip без потерь | SPEC §6.6, docs/SPEC.md (схема настроек) |
| Рендер канваса | Эффективная палитра = пресет или классика (`ThemeColors::from_settings`); все потребители `ThemeColors` работают без изменений | SPEC §6.2–6.3 |
| Виджеты/оверлеи | Флаг темности виджетам — от эффективной палитры (`is_dark()` по фону пресета) | SPEC §6.6 |
| Токены | Новый каталог данных `design/tokens/themes/*.json` — пресеты как данные (G2) | PRD-0006 §7 |

## Анализ (Root Cause — чего нет в коде)

- Тема — бинарный enum `Theme {Dark, Light}` (`canvas-core/src/settings.rs:47-53`), палитра выбирается `ThemeColors::from_theme` в ~15 точках `canvas-app/src/app.rs` — пресет не выразим: нет ни хранилища id, ни реестра палитр, ни UI-выбора.
- Механизм «пресет как данные» отсутствовал: `design/tokens/` содержит только примитивы текущего вида (FR-046), реестра тем нет; G2 PRD-0006 («пресет = JSON + регистрация, рендер-код не меняется») не реализован.
- UI: карточки темы в модалке FR-039 (settings_ui.rs, `theme_cards`) поддерживают ровно 2 карточки — 9 карточек не помещаются в адаптивную модалку с инвариантом 320×240 (MODAL_MIN_W/H, кламп viewport); dropdown-механика строк (`dropdown_options`/`apply_dropdown_value`) существует и масштабируется на любое число опций.
- Контраст-машина G3 (`theme.rs` тесты, `contrast.rs`) покрывала только классические темы — пресеты как новые данные требовали включения в машину (урок CR-007: нетекстовая графика ≥ 3:1, текст ≥ 4.5:1).

## Решения (зафиксированы)

1. **Состав** (решение владельца, ответ Q3): 7 пресетов — Nord, Dracula, Catppuccin Mocha, Catppuccin Latte, Solarized Light, Tokyo Night, Gruvbox Dark. Пары Mocha/Latte покрывают светлый режим; Monokai/GitHub/VSCode/Solarized Dark — пул «добавить позже» (одна строка + JSON, ≤ 30 мин — метрика G2).
2. **Пресет = данные**: полный набор из 36 семантических слотов `ThemeColors` в `design/tokens/themes/<id>.json` (hex `#RRGGBB`/`#RRGGBBAA`); реестр `canvas_core::theme_presets::PRESETS` — одна строка `ThemePreset { id, label, json: include_str!(...) }` на пресет. Разбор+валидация (набор ключей строго равен `REQUIRED_KEYS`, I-47.1) с кэшем `OnceLock` — разбор один раз на процесс, выборка O(1) на кадр (wasm-безопасно: `include_str!` без ФС, ADR-0011).
3. **Хранение выбора**: `Settings.theme_preset: String` (пустая строка — классика; `#[serde(default)]` — старые конфиги совместимы); неизвестный id (переименование/ручная правка) деградирует мягко к классике (`active_preset()` → `find()` → `None`), ноль паник.
4. **UI**: dropdown-строка «Тема-пресет» в табе «Внешний вид» (опции: «Классическая» + 7 меток реестра; имена собственные — без i18n, конвенция FR-040 для native-лейблов); карточки тёмной/светлой остаются и при клике сбрасывают пресет; тумблер ☀/🌙 тоже сбрасывает пресет (явный выбор классики). Карты пресетов (AC-3.2) — отклонение, см. §Отклонения.
5. **Контраст (G3)**: значения слотов сверены pre-commit скриптом-генератором и закреплены тестами `canvas-render/src/theme_presets.rs`: графика к фону (accent, guide_align, guide_grid, error, hud, whatif_badge) ≥ 3:1; тексты к карточке (title, body, edge_label) ≥ 4.5:1; code_text к подложке кода ≥ 4.5:1; приглушённые роли (icon, link, quote) к карточке ≥ 3:1. Каждая пара на каждом пресете — красный CI при деградации.
6. **Направляющие FR-038** сохраняют семантику «отличима от акцента»: для каждого пресета выбран контрастный к акценту тон (например, Nord — оранжевый aurora #D08770 при акценте frost #81A1C1; Gruvbox — purple #D3869B при акценте orange #FE8019).

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| 7 JSON-пресетов | `design/tokens/themes/{nord,dracula,catppuccin-mocha,catppuccin-latte,solarized-light,tokyo-night,gruvbox-dark}.json` | 36 слотов + `$note`/`source`/`license`/`dark`; сгенерированы скриптом с WCAG-пре-валидацией |
| Реестр+парсер+кэш | `crates/canvas-core/src/theme_presets.rs` (новый) | `PRESETS`, `REQUIRED_KEYS`, `parse` (валидация I-47.1), `find`, `parsed` (кэш `OnceLock`); тесты: все пресеты разбираются, паритет label (I-47.2), отказ на недостающем/лишнем/нечитаемом ключе, hex-формы |
| Поле настроек | `crates/canvas-core/src/settings.rs` | `theme_preset: String` (serde default), `active_preset()`; round-trip тест расширен пресетом |
| Отображение в палитру | `crates/canvas-render/src/theme_presets.rs` (новый) | `preset_theme(id) -> Option<ThemeColors>` — универсальный маппинг «имя слота → поле»; `ThemeColors::from_settings(theme, preset_id)` в `theme.rs`; тесты G3 на все пресеты |
| Единая точка выбора палитры | `crates/canvas-app/src/app.rs` | `effective_palette()`; 13 точек `ThemeColors::from_theme(self.settings.theme)` → `self.effective_palette()`; `apply_effective_theme()` (рендер + виджеты + redraw); тумблер/карточки сбрасывают пресет; флаг виджетов при старте — от эффективной палитры |
| Dropdown-строка | `crates/canvas-app/src/settings_ui.rs` | `SettingsRow::ThemePreset` (SETTINGS_ROWS 18→19, таб «Внешний вид»), `row_kind` → Dropdown, `dropdown_value`/`dropdown_options`/`apply_dropdown_value` (индекс 0 = «Классическая»), инвариант-тесты |
| Локализация | `crates/canvas-app/src/i18n.rs` | Ключи `ROW_THEME_PRESET`/`DESC_THEME_PRESET`/`THEME_PRESET_CLASSIC` (RU+EN) |

## Инварианты

- **I-47.1**: набор ключей `colors` пресета строго равен `REQUIRED_KEYS` (36) — недостающий/лишний/нечитаемый ключ = пресет не грузится, fallback на классику; ловится тестом разбора.
- **I-47.2**: `label` реестра == `label` JSON (паритет-тест, как tokens FR-046 I-5).
- **I-47.3**: контраст-машина G3 покрывает каждый пресет (те же пороги, что классика: графика ≥ 3:1, текст ≥ 4.5:1) — красный CI на непроходимой паре.
- **I-47.4**: добавление пресета не трогает `.rs` рендера — JSON-файл + строка `PRESETS` + тест-список светлых в render-тесте (G2; время ≤ 30 мин).
- **I-47.5**: обратная совместимость конфига: пустое поле/старый config.toml = классическая тема; неизвестный id = мягкая деградация без паник.
- **I-47.6**: шейдеры не меняются (I-3 FR-046) — цвета приходят рантаймом через инстансы/юниформы.

## Отклонения реализации (протокол PRD-0006)

1. **AC-3.2 «выбор карточкой»** → dropdown-строка. 9 карточек не влезают в адаптивную модалку при клампе 320×240 (инвариант FR-039: модалка целиком в окне); карточки классики сохранены (визуальный паттерн Obsidian «Base theme» — сам Obsidian использует dropdown для темы). Карточки пресетов с превью-свотчами — кандидат v2.
2. **Состав F-8 расширен** с «2–3 пресета» до 7 — прямое решение владельца по Q3 (PRD-0006 §13 Q3 закрыт).
3. **Слоты, подстроенные от официальных палитр** под пороги WCAG (значения существующих тем не менялись — в отличие от FR-046 I-1, пресеты — новые данные, порог важнее буквального соответствия): Nord `error` #BF616A→#CE6B76, `gfm_code_fill` #434C5E→#39414E; Catppuccin Latte `code_text` #179299→#0E757C, `whatif_badge` #DF8E1D→#9A6A00, `quote` #8D93A8→#7D839C; Solarized Light `code_text` #2AA198→#19727A, `whatif_badge` #B58900→#8F6C00; поверхности (card/menu/grid) производные официальных ролей bg/surface (документированы в `$note` каждого JSON).
4. **Severity/минимапа/wheel/диалоги/тосты** остаются на общих примитивах FR-046 (не входят в 36 слотов пресета) — дифференциация по темам, как и для классики, — v2 (PRD-0006 §7).

## Точки входа

- `docs/change-requests/index-cr-fr.md` — строка FR-047 (этот файл).
- `docs/prd/prd-0006-design-system-tokens.md` — changelog D4 (F-8), закрытие Q3.
- `docs/prd/README.md` — статус PRD-0006.
- `docs/SPEC.md` §6.6 — пресеты как данные и эффективная палитра.
- `docs/ACCEPTANCE.md` — секция FR-047 (замер гейтов).
- `worklog.md` — запись волны.

## Проверка (Verification)

- [x] `cargo fmt --all --check` — чисто.
- [x] `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings` — 0 ошибок.
- [x] `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` — 1301 тест зелёный (46 бинарей), включая новые: разбор/валидация/паритет пресетов (canvas-core), is_dark + контраст G3 всех 7 пресетов + fallback (canvas-render), dropdown-инварианты ThemePreset + layout таба «Внешний вид» (canvas-app).
- [x] `bash scripts/token_lint.sh` — 0 hex-литералов вне токенов (G1; hex пресетов — данные в `design/tokens/`, allowlist FR-046).
- [x] `bash scripts/wasm_gate.sh` — ядро и мост собираются под wasm, тесты в wasmtime зелёные.
- [x] `bash scripts/mcp_wasm_gate.sh` — полная MCP-сессия под wasm зелёная.
- [ ] Ручная приёмка владельцем: модалка → «Внешний вид» → «Тема-пресет» → поочерёдный выбор 7 пресетов (перерисовка на лету: канвас, карточки, меню, поиск, палитра, wheel, диалоги, тосты, минимапа), перезапуск — выбор сохранён в config.toml; клик по карточке тёмной/светлой и тумблеру ☀/🌙 — возврат к классике.

## История изменений

- `2026-09-21` — агент: создан документ (FR-047) по решению владельца (Q3 → состав 7 пресетов); реализация T-047.1 (данные: 7 JSON + реестр/парсер/кэш canvas-core), T-047.2 (отображение + контраст-тесты G3 canvas-render), T-047.3 (настройки/UI/i18n canvas-app), документация; статус `выполнено`; все гейты зелёные (fmt/clippy/1301 тест/token_lint/wasm/mcp).

## Источники истины

- `docs/prd/prd-0006-design-system-tokens.md` — §8 F-8/F-9, G2/G3, §13 (Q3, этапы D4/V2).
- `docs/change-requests/fr-046-design-tokens.md` — фундамент: примитивы, `ThemeColors` v2, токен-линт, протокол отклонений.
- `design/tokens/themes/*.json` — данные пресетов (36 слотов, `source`/`license` каждой палитры).
- `crates/canvas-core/src/theme_presets.rs` — реестр, валидация, кэш.
- `crates/canvas-render/src/theme_presets.rs` — отображение в `ThemeColors`, тесты G3.
- `crates/canvas-app/src/settings_ui.rs`, `crates/canvas-app/src/app.rs`, `crates/canvas-app/src/i18n.rs` — UI, эффективная палитра, локализация.
- Официальные палитры: nordtheme.com, draculatheme.com, github.com/catppuccin/catppuccin, ethanschoonover.com/solarized, github.com/tokyo-night/tokyo-night-vscode, github.com/morhetz/gruvbox.
