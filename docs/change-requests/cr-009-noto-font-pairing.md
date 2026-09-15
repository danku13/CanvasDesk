# CR-009: Шрифтовая пара Noto Sans Display + Noto Sans Mono (типографика канваса)

- **Статус:** выполнено
- **Тип:** CR (Change Request — замена механизма выбора шрифта)
- **Приоритет:** важно
- **Владелец:** агент (анализ + реализация)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «нужно заменить шрифт на Noto Sans Display для обычного текста medium 500, для bold 700 и Noto Sans Mono для строк определённых как Numi-like рассчётов и для результатов рассчёта»
- **Связанные задачи:** FR-013 (Numi-строки и результаты — моноширинное начертание), CR-007 (читаемость/контраст — шрифт второй фактор читаемости), SPEC.md §6.2 (LOD — метрики текста), docs/interface-objects/node.md (текст ноды)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-16 (аудит реализации)
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Требуется заменить шрифтовую базу канваса:

| Начертание | Шрифт | Вес |
|---|---|---|
| Обычный текст (заметки, заголовки нод, UI-панели, HUD) | **Noto Sans Display** | **Medium 500** |
| Жирный текст (GFM-заголовки `#`, `**bold**`, Ctrl+B) | **Noto Sans Display** | **Bold 700** |
| Numi-строки (строки, распознанные как расчёты — есть результат `eval_lines`) | **Noto Sans Mono** | Regular 400 |
| Результаты расчётов (построчные `42 $`, программный итог, живые в редакторе) | **Noto Sans Mono** | Regular 400 |

Выявил пользователь: визуально текст карточек набирается «случайным» системным
шрифтом вместо встроенного, моноширинного различия между расчётами и прозой нет.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| Текстовая нода | семейство/вес тела, заголовка, GFM-блоков; Numi-строки — моно | `docs/interface-objects/node.md` §2 (текст) |
| Результаты FR-013 | моноширинное начертание (карточка + живые в редакторе) | `docs/change-requests/fr-013-text-node-numi-expr.md` |
| Панели/HUD/тултипы/лейблы связей | Noto Sans Display Medium вместо системного fallback | `docs/interface-objects/search.md` и др. |
| Бинарник | `assets/fonts`: −Inter.ttf, +4 статических Noto (SIL OFL 1.1) | README (лицензии) |

## Анализ (Root Cause)

1. **Встроенный Inter фактически не используется.** `TextSystem::new`
   (`text.rs:1034-1035`) грузит Inter через `load_font_data`, но все буферы
   создаются с `Attrs::new()` — family `Family::Sans`. cosmic-text 0.12.1
   резолвит `Family::Sans` в `db.sans_serif_family()` = `"Fira Sans"`
   (`font/system.rs:142`), которого нет ни в системе, ни в бинаре.
   `Attrs::matches` (`attrs.rs:210-214`) вообще **не фильтрует по family**
   (только style/stretch), поэтому выбор уходит в fallback-цепочку
   (`font/fallback/mod.rs`) и на Linux заканчивается DejaVu Sans: текст
   рендерится «каким получится» — зависит от машины. `Family::Monospace`
   (фенсы, `text.rs:686`) аналогично резолвится в несуществующий
   `"Fira Mono"` → фенсы не гарантированно моноширинные.
2. **Вариативные шрифты не инстанцируются.** Текущий `Inter.ttf` — variable
   (wght 100–900). cosmic-text 0.12.1 не передаёт вариации в swash
   (`swash.rs: swash_image` — билдер без `Variation`), глифы берутся из
   дефолт-инстанса (wght 400): запрошенный `Weight::BOLD` не дал бы жирного
   начертания. Требуются **статические** инстансы (Medium 500, Bold 700).
3. **Numi-строки неразличимы.** `body_items` (`text.rs:400`) выделяет
   формульные строки в отдельные абзацы (`source_line: Some(i)` →
   `gfm::Block::Paragraph`, `text.rs:471-489`), но `BodyItem.mono` ставится
   только код-фенсам (`text.rs:524`) — расчёты рисуются тем же шрифтом, что
   проза. Результаты (`RESULT_FONT_SIZE`-буферы `text.rs:1334-1357`,
   `1381-1406`, `1607-1630`) тоже `Attrs::new()`.
4. **Редактор не согласован.** `EditingSession` (`edit.rs:338-343`, `464-467`)
   шейпит буфер с `Attrs::new()` — при вводе шрифт отличается от
   отрисованного тела (WYSIWYG-разрыв), Numi-строки в вводе не моно.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Ассеты | `assets/fonts/` | −`Inter.ttf`/`OFL.txt`; +`NotoSansDisplay-Medium.ttf`, `NotoSansDisplay-Bold.ttf`, `NotoSansMono-Regular.ttf`, `NotoSansMono-Bold.ttf` (статические, hinted, SIL OFL 1.1 — `OFL-NotoSans*.txt`) |
| Регистрация | `text.rs` `FONT_DATA`, `TextSystem::new` | `FONT_DATA: &[&[u8]]` из 4 шрифтов, загрузка циклом |
| Базовые атрибуты | `text.rs` | `SANS_FAMILY`/`MONO_FAMILY` + `sans_attrs()` = `Family::Name("Noto Sans Display").weight(MEDIUM)`, `mono_attrs()` = `Family::Name("Noto Sans Mono")`; все `Attrs::new()` на call-sites заменены (заголовок, иконка, тело, HUD, оверлеи, screen-тексты, лейблы связей, бейдж, результаты — моно) |
| Numi-строки | `text.rs` `push_gfm_blocks` | `Block::Paragraph` при `source_line.is_some()` → `mono: true` (строка с результатом `eval_lines` = Numi-строка) |
| Редактор | `edit.rs` | базис rich-спанов посемейственно: строка из `formula_line_indices(plain)` (через `canvas_core::expr::eval_lines`) → `mono_attrs()`, прочие → `sans_attrs()`; жирные спаны внутри строк сохраняют family строки; для лейблов связей mono не применяется |
| Тесты | `text.rs`, `edit.rs` | регистрация 4 лиц в fontdb (веса 400/500/700); `body_items`: формульная строка → `mono`; `shape_body`: атрибуты строки буфера = Noto Sans Mono / Noto Sans Display Medium; редактор: Numi-строка → mono, проза → sans |

Вне рамок: egui/веб-виджеты (FR-001) используют собственные шрифты WebView —
не затрагиваются; метрики `BODY_FONT_SIZE`/`LINE_HEIGHT` не меняются.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` — §2 (текст ноды: шрифтовая пара).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — результаты Numi-стиля: моно.
- `README.md` — лицензии шрифтов (SIL OFL 1.1).
- `AGENTS.md` — без изменений (путь `assets/fonts` сохранён).

## Проверка (Verification)

- [x] fontdb содержит ровно 4 вшитых лица: Noto Sans Display 500/700, Noto Sans Mono 400/700 (юнит-тест `font_data_registers_noto_faces`).
- [x] `body_items("deploy = 40 $", [0])` → элемент `mono == true` (тест).
- [x] Атрибуты буфера тела: Numi-строка — `Family::Name("Noto Sans Mono")`, проза — `Family::Name("Noto Sans Display")` + `Weight::MEDIUM` (тест).
- [x] Редактор: Numi-строка в буфере — mono, соседняя проза — sans medium (тест `editor_formula_lines_use_mono`).
- [x] Bold-спан: `Weight::BOLD` (700) при сохранении family (тест rich_spans).
- [x] `cargo test --workspace` зелёный; `cargo clippy --workspace` — 0 warnings; `cargo fmt` чист.
- [x] Живой прогон (Xvfb + lavapipe): скриншоты сцены с Numi-листом — расчёты и результаты моноширинные, проза/заголовки Noto Sans Display.
  - `docs/assets/cr-009-canvas.png` — карточки сцены demo-numi: Numi-строки и результаты Noto Sans Mono, заголовки/проза Noto Sans Display (Medium/Bold), кириллица «итого» в моно.
  - `docs/assets/cr-009-editor.png` — live-редактор: набранные `vm = 40 $`, `db = 25 $`, `sum = vm + db` моноширинные уже при вводе, живые результаты справа (WYSIWYG совпадает с карточкой).

## История изменений
- `2026-09-16` — агент (аудит реализации всех CR/FR, main `984ca6b`): реализация подтверждена — 4 ttf в `assets/fonts/` (Display Medium/Bold, Mono Regular/Bold, OFL), бандл `FONT_DATA` = 4× `include_bytes!` (`text.rs:34-39`), базовые атрибуты `Noto Sans Display` Weight::MEDIUM (500) и `Noto Sans Mono` (`text.rs:49-60`), Bold 700 без смены family (`:724-729`), mono для формульных строк и результатов (`:520,560,1389,1439`); тесты `font_data_registers_noto_faces`, `base_attrs_pin_noto_families`, `body_items_formula_line_is_mono`, `shape_body_fonts_by_line_kind`, `editor_formula_lines_use_mono` — зелёные. Статус `выполнено` подтверждён.


- `2026-09-15` — агент: создан документ (`CR-009`), статус `выявлено`, анализ (cosmic-text 0.12.1: family не участвует в `Attrs::matches`, Sans→«Fira Sans», variable-шрифты не инстанцируются), требования.
- `2026-09-15` — агент: реализация (assets, text.rs, edit.rs, тесты), статус `выполнено` после гейтов и живой верификации.
- `2026-09-15` — агент: перенумерация `CR-008` → `CR-009` — номер `CR-008` занят владельцем репо (`cr-008-smart-edge-ports.md`, коммит `9f00f3f`) пока этот документ был в работе.

## Источники истины

- `crates/canvas-render/src/text.rs` — `FONT_DATA` (25-26), `TextSystem::new` (1034-1035), `rich_spans` (193), `shape_body` (654), `push_gfm_blocks` (440-587), результаты (1330-1416, 1589-1633), бейдж (1564-1587), HUD (1467-1494), screen-тексты (1540-1559).
- `crates/canvas-render/src/edit.rs` — `EditingSession::new` (322-366), `refresh_styles` (464-474).
- cosmic-text 0.12.1: `src/attrs.rs:210` (`matches` — без family), `src/font/system.rs:139-143` (`Fira Sans`/`Fira Mono`), `src/swash.rs:29-52` (нет вариаций), `src/font/fallback/mod.rs` (цепочка fallback).
- fontdb 0.16.2: `src/lib.rs:976-990` (family из name ID16, fallback ID1; вес из OS/2).
- `assets/fonts/OFL-NotoSansDisplay.txt`, `assets/fonts/OFL-NotoSansMono.txt` — лицензии.
- Google Fonts CSS2 API — источник статических TTF (v30/v37, полный кириллический контур).
