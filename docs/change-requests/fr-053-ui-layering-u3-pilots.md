# FR-053: Слои, вёрстка и UI kit — этап U3 PRD-0009: TextMeasurer, токены состояний/spacing, layout-примитивы, пилотная миграция

- **Статус:** выполнено (U3)
- **Тип:** FR
- **Приоритет:** критично (инфраструктура вёрстки для всех UI-фич; пилоты проверяют ядро на живых поверхностях)
- **Владелец:** агент (по приказу владельца «Продолжай u3»)
- **Источник:** PRD-0009 §13 этап U3; FR-051 (каркас U1), FR-052 (интеграция U2)
- **Связанные задачи:** PRD-0009 §7.4 (V-5/V-6), AC-4.1, G4, G5, G8-precursor; PRD-0006 F-6 (spacing), FR-046 (слоты v2); CR-015 (урок: эвристика ширины чипов)
- **Создан:** 2026-09-22
- **Обновлён:** 2026-09-22

---

## 1. Описание (What)

Этап U3 дорожной карты PRD-0009: добавить в `canvas-ui` **измерение текста** (F-6) и **layout-примитивы** (F-7), расширить токены **spacing/radius-scale** и **слотами состояний** (F-9, в пределах пилотов) и перевести на них две пилотные поверхности: **галерею схем** (модалка, L4 `Modals`) и **what-if бар** (Capture, L3 `Panels`). Результат этапа — гейты G4 (layout-линт на 3 окна × RU/EN на пилотах) и G5 (0 ручных клампов/срезов в мигрированных модулях).

Выявлено агентом в ходе реализации PRD-0009 (этап назначен владельцем приказом «Продолжай u3»).

## 2. Влияние (Impact)

| Объект | Что меняется | Где |
|---|---|---|
| `canvas-ui` | +2 модуля: `layout.rs` (примитивы), `measure.rs` (TextMeasurer); +зависимость cosmic-text (уже в workspace — не новая) | `crates/canvas-ui/` |
| Токены геометрии | `dimensions.json` + секция `spacing` (s/sm/md/lg/xl) и `radius` (chip/pill/panel); паритет в `tokens.rs` | `design/tokens/dimensions.json`, `canvas-core/src/tokens.rs` |
| Слоты состояний | `colors.json` + группа `control` (hover/selected/disabled); `ThemeColors` +4 слота; паритет-тест продолжается | `design/tokens/colors.json`, `canvas-core/src/tokens.rs`, `canvas-render/src/theme.rs` |
| What-if бар | ширины чипов/кнопок = измеренные (вместо эвристики `0.62·кегль`), подписи чипов — Ellipsis-политика (вместо `chars.truncate`), хвост бара — примитив `SqueezeTail` (вместо замыкания `take`), счётчик подмен — i18n-строка из приложения | `canvas-app/src/whatif_ui.rs`, `app.rs` |
| Галерея схем | раскладка собрана примитивами (Stack/Column/Row), `break`-кламп чипов удалён (переполнение стало тестируемым), тексты строк — измеренный Ellipsis, hover/selected — слоты | `canvas-app/src/scheme_gallery_ui.rs`, `app.rs` |
| Отрисовка пилотов | fills состояний — из слотов (самодельное посветление `hover_fill` в пилотах удалено), метки строк/чипов — из раскладки | `canvas-app/src/app.rs` |

## 3. Анализ (Root Cause — что отсутствует)

1. **Нет измерения текста**: ширины для раскладки берутся из эвристик — `whatif_ui::text_width` (`whatif_ui.rs:64-66`, фактор `0.62·кегль`, урок CR-015: эвристика занижала ширину, потребовался `CHIP_SLACK`); `docs_ui::text_width` (вне пилотов — U5). Рендер при этом шейпит реальным cosmic-text — раскладка и отрисовка живут в разных метриках (расхождение = класс дефекта «текст шире чипа»).
2. **Нет политики усечения**: `whatif_ui::chip_label` (`whatif_ui.rs:128-135`) режет по числу символов (`chars.truncate`), не по фактической ширине; тексты строк галереи рисуются в screen-текстах с `Wrap::None` без какого-либо усечения — длинное описание переливается на соседнюю строку.
3. **Нет примитивов**: раскладка — ручная арифметика rect'ов с рассыпанными клампами: замыкание `take` (`whatif_ui.rs:188-193`, хвост бара зжимается до нуля молча), `break` в цикле чипов галереи (`scheme_gallery_ui.rs:168-170`, молчаливый срез категорий — тот класс дефекта, что ловил D2 CJM), клампы панели к вьюпорту (`scheme_gallery_ui.rs:133-141`).
4. **Нет слотов состояний**: hover собирается самодельным посветлением `hover_fill` (`app.rs:1042-1049`; галерея-строки `app.rs:4844`, empty-кнопки `app.rs:4944,4949`); disabled-текст — локальная константа `dim` (`app.rs:2808`); значение hover зависит от формулы, а не от токена (PRD-0009 §7.1.7).
5. **spacing не потребляется UI-оверлеями**: константы 6/8/10/12/24 живут литералами в модулях пилотов (`BAR_GAP`, `PANEL_PAD`, …), в `dimensions.json` их нет (§7.1.7 PRD-0009).
6. **Ширина счётчика подмен считается по RU-строке**, а рисуется i18n-строка (`bar_layout` форматирует «подмен: N», draw — `trf(WHATIF_OVERRIDES)`) — в EN ширина раскладки не соответствует подписи.

## 4. Требуемые изменения (Changes)

### 4.1 canvas-ui: `layout.rs` — примитивы (F-7)

Immediate-mode чистые функции от слота родителя (интерфейс совместим со слотами taffy — PRD-0009 §7.4):

- `Row { gap, main: MainAlign, cross: CrossAlign, policy: RowPolicy }` — дети слева направо; `MainAlign::{Start, SpaceBetween}`, `CrossAlign::{Start, Center, End}`; `RowPolicy::{Fit, SqueezeTail}`.
  - `Fit` — контент определяет занятость; переполнение слота не маскируется (ловится линтом/тестом G4);
  - `SqueezeTail` — именованная деградация: элементы получают `min(desired, остаток)`, хвост сжимается до нулевой ширины (дословная семантика бывшего `take` what-if бара — вырожденные rect'ы невидимы и не пикаются, `UiRect::contains` half-open).
- `Column { gap, main, cross }` — вертикальный аналог.
- `Child { w, h }` + `Child::fixed/spacer` (spacer — зазор без «пустого» квадрата).
- `stack(slot, size, h: HAlign, v: VAlign)` — размещение фиксированного блока в слоте (центрирование панели галереи, empty-карточка).
- `constrain(min, max, desired)` — кламп размера (Clamp-семантика `Constrain(min/max)` F-7).
- `pad(slot, EdgeInsets)` — обёртка `UiRect::inset`.
- `Custom(pub UiRect)` — escape-hatch экзотики (R-3): прямая геометрия с комментарием-обоснованием; в линт попадает явно.
- Тесты: суммы/gaps, cross-align, SpaceBetween, SqueezeTail (хвост нулевой, префикс точный), stack-центрирование, constrain-клампы.

### 4.2 canvas-ui: `measure.rs` — TextMeasurer (F-6)

- `TextMeasurer::measure(&mut self, fs: &mut FontSystem, spec: &TextSpec) -> Measured {width, height, lines}` — реальный шейпинг cosmic-text (`Wrap::None`, семейство/вес как у screen-текстов рендера — parity метрик), `Metrics::new(size, size·1.3)` — тот же фактор строки, что в screen-конвейере `text.rs`.
- Кэш по (текст, семейство, кегль, макс-ширина) — Q6-a «измерение + кэш»; ограничение ёмкости (переполнение → очистка), `FontSystem` — аргументом (владелец инстанса — `canvas-render::text`, Q6/§14 PRD).
- `width_of(...)` — однострочная ширина (замена `text_width`-эвристик).
- `ellipsis(...)` — политика усечения по ФАКТИЧЕСКОЙ ширине: бинарный поиск по границам символов префикса + `…`, кэш переиспользует ширины префиксов (замена `chars.truncate`).
- Зависимость `cosmic-text` — уже в workspace (рендер), не новая (G7: 0 новых runtime-зависимостей); wasm-совместима (canvas-ui транзитивно в wasm-гейте через canvas-render).
- Тесты: детерминированный `FontSystem` со вшитым `NotoSansDisplay-Medium.ttf` (тот же файл, что рендер); монотонность ширины, пустая строка, ellipsis ≤ max_w и монотонность, кэш-попадание (повтор — тот же результат).

### 4.3 Токены (F-9 в пределах пилотов)

- `design/tokens/dimensions.json`: секция `spacing` — `s=6` (BAR_GAP/зазор чипов/LIST_MARGIN), `sm=8` (TABLE_MARGIN), `md=10` (BAR_PADDING/поля empty-кнопок), `lg=12` (BAR_MARGIN/CHIP_PAD_X/BTN_PAD_X/PANEL_PAD), `xl=24` (поля панели к вьюпорту); секция `radius` — `chip=6` (квад what-if чипа), `card=8` (уже есть), `panel=10` (панель галереи), `pill=12` (чипы категорий галереи). Значения = текущим константам (I-1: ноль скачка), `$desc` — источник.
- `canvas-core/src/tokens.rs`: константы `SPACING_S/SM/MD/LG/XL`, `RADIUS_CHIP/PANEL/PILL` + паритет-тест против JSON.
- Константы пилотов переопределяются из scale: `BAR_GAP = SPACING_S`, `BAR_MARGIN = SPACING_LG`, `PANEL_PAD = SPACING_LG` и т.д. (значения прежние, источник один).
- `design/tokens/colors.json` + группа `control`: `control.hover_fill` = hover строки/вторичной кнопки (бывший `hover_fill(menu_fill)` — значения обеих тем зафиксированы константами), `control.primary_hover_fill` = `hover_fill(DIALOG_BUTTON_PRIMARY)`, `control.selected_fill` = значение hover (сегодня selected и hover неразличимы — ноль скачка; семантика разделена слотами), `control.disabled_text` = `#8a909c` (бывшая локальная `dim`). Паритет в `tokens.rs` + `ThemeColors` (+слоты `control_hover_fill`, `control_primary_hover_fill`, `control_selected_fill`, `control_disabled_text`) + строки в `v2_slots_match_tokens_in_both_themes`/контраст-протоколе FR-046.
- В пилотах `hover_fill(...)`/`dim` заменяются слотами; сама функция `hover_fill` остаётся (не-пилотные поверхности — U5).

### 4.4 Пилот what-if бар (Capture)

- `bar_layout(scenario_names, counter_label, viewport, measurer, fs)`: ширины чипов/кнопок — измеренные (`width_of` + поля из spacing), `counter_label` — параметр (i18n-строка приложения — фикс п.6 анализа), хвост — `Row::SqueezeTail` на слоте бара (вместо `take`), ширина бара — `constrain` к вьюпорту.
- `BarLayout.scenario_labels: Vec<String>` — подписи чипов, отэллипсенные тем же `ellipsis(max_w = ширина чипа − 2·CHIP_PAD_X)`, что и ширина (раскладка и отрисовка — одна строка, урок CR-015); draw-сайт `app.rs:2892` читает метку из раскладки.
- Удалены: `text_width`, `chip_label`, `CHIP_LABEL_MAX`, `CHIP_SLACK`, замыкание `take` (G5: grep-аудит `take(`/truncate-срезы в модуле — 0). `CHIP_SLACK` больше не нужен: ширина измеряется реально.
- Нормализованная дельта (допустимое видимое отличие, фиксируется сознательно): ширины чипов/кнопок меняются на несколько px (эвристика 0.62·кегль ≠ измеренная ширина Noto; CR-015 компенсировал запасом `CHIP_SLACK` — теперь ширина точная); в EN счётчик «overrides: N» шире RU — кнопки сдвигаются (раньше ширина считалась по RU при EN-подписи — дефект).

### 4.5 Пилот галерея схем (модалка)

- `layout()`: панель — `stack(центр вьюпорта)` + `constrain` к вьюпорту; вертикальный ритм — `Column` со спейсерами (6/0/6/6 — дословно сегодняшний ритм, ноль скачка); чипы — `Row{Fit}` без `break` (все категории обязаны влезать — тест `chips_all_categories_fit` продолжает ловить молчаливый срез, теперь переполнение было бы видно линту); строки — `Column{gap 6}` в окне видимости; empty-карточка и кнопки — `stack` + `Row`.
- `row_labels(list, state, viewport, measurer, fs) -> Vec<RowLabel{title, desc}>` — измеренный Ellipsis заголовка (13 px) и описания (11 px) по ширине строки − 20; draw-сайт `app.rs:4848-4870` берёт метки из раскладки (текст больше не переливается на соседнюю строку — класс дефекта §3.2).
- hover/selected строки и empty-кнопки — слоты `control_*` (п. 4.3).
- Нормализованная дельта: длинные описания строк галереи усекаются «…» по фактической ширине (сегодня переливались на соседнюю строку без усечения — дефект, устраняется).

### 4.6 Lint-гейты на пилотах (G4/G5-precursor)

- Тесты в модулях пилотов: вьюпорты **1280×800 / 1024×640 / 800×560** × **RU/EN**:
  - галерея: панель ⊆ вьюпорт (маржа xl), все элементы ⊆ панели, чипы/строки попарно не пересекаются (`UiRect::intersects`), все категории присутствуют;
  - what-if: элементы бара попарно не пересекаются, ⊆ бара, бар ⊆ вьюпорта (маржа lg); список/таблица — над баром, ⊆ вьюпорта; длинные имена сценариев — ellipsis укладывается в чип;
  - повтор через `SurfaceFrame`/`overlaps_within_layer` (кадровый путь U2).
- G5: в `whatif_ui.rs`/`scheme_gallery_ui.rs` нет `take(`/`chars.truncate`/`truncate_chars`/break-клампов циклов раскладки (проверяется grep-аудитом в FR-верификации).

## 5. Точки входа (Entry Points)

- `docs/change-requests/index-cr-fr.md` — строка FR-053, указатель → FR-054.
- `docs/prd/prd-0009-ui-layering-uikit.md` — статус (U3), changelog.
- `docs/prd/README.md` — статус PRD-0009.
- `docs/SPEC.md` §6 — не меняется (бюджеты рендера не задеты; scissor — U4/U5).
- `worklog.md` — двойная запись по завершении.

## 6. Проверка (Verification)

- [ ] TDD-тесты `canvas-ui::layout` (примитивы) и `canvas-ui::measure` (измерение/кэш/ellipsis) зелёные.
- [ ] Тесты пилотов: G4-матрица 3 окна × RU/EN (0 пересечений, 0 выходов, ellipsis в границах) зелёные; `chips_all_categories_fit`, CR-015-инварианты (без наложений, подпись ⊆ чип) — на измеренных ширинах.
- [ ] Паритет: `dimensions.json` (spacing/radius) ↔ `tokens.rs`; `colors.json` (control) ↔ `tokens.rs` ↔ `ThemeColors` (обе темы).
- [ ] G5-аудит: grep `take\(|chars\.truncate|truncate_chars` в `whatif_ui.rs`/`scheme_gallery_ui.rs` — 0 совпадений.
- [ ] Существующие z-тесты и вводные цепочки не затронуты (реестр U2 не менялся — раскладки/отрисовка пилотов).
- [ ] Гейты: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`; `scripts/wasm_gate.sh`; `scripts/mcp_wasm_gate.sh` — зелёные; CI по merge SHA.

## 7. История изменений (Changelog)

- 2026-09-22 — агент — U3 реализован: canvas-ui layout.rs/measure.rs (+12 тестов примитивов/измерения), tokens.rs SPACING/RADIUS/CONTROL + паритет, ThemeColors +4 слота (пресеты — вывод от menu_fill), пилоты what-if/галерея (измеренные ширины, Ellipsis-подписи из раскладки, SqueezeTail/Constrain/Stack/Column/Row, hover/selected/disabled — слоты), G4-линт 3 окна × RU/EN, G5-аудит 0 клампов; гейты ветки (на объединённом с origin/main коде) зелёные: fmt, clippy -D warnings, test --workspace, wasm_gate, mcp_wasm_gate; CI по merge SHA 599bca8 — ПОЛНОСТЬЮ ЗЕЛЁНЫЙ (12/12 check-runs). Статус «выполнено (U3)».
- 2026-09-22 — агент — документ создан (постановка этапа U3 PRD-0009: F-6/F-7/F-9-частично + пилоты галерея/what-if), статус «в работе».

## 8. Источники истины (References)

- `docs/prd/prd-0009-ui-layering-uikit.md` §7.4 (V-5/V-6), §8 (F-6/F-7/F-9), §11 (Q6), §13 (U3), §15 (G4/G5/G7).
- `docs/change-requests/fr-051-ui-layering-uikit.md`, `fr-052-ui-layering-u2-integration.md` — каркас и интеграция.
- `crates/canvas-ui/src/{geometry,frame}.rs` — UiRect/EdgeInsets/SurfaceFrame (база примитивов).
- `crates/canvas-render/src/text.rs:35-60,690-740,951-985` — FONT_DATA, `sans_attrs`, screen-конвейер (Metrics ×1.3, Wrap::None), общий FontSystem измерения.
- `crates/canvas-app/src/whatif_ui.rs` (эвристика `text_width`, `chip_label`, `take`), `scheme_gallery_ui.rs` (break-кламп чипов), `app.rs:1042-1049,2808,2892,4844,4944-4949` (hover_fill/dim/draw-сайты).
- `design/tokens/{dimensions,colors}.json`, `crates/canvas-core/src/tokens.rs:203-479` — паритет-машина PRD-0006.
- `docs/change-requests/cr-015-whatif-bar-layout.md` — урок: эвристика ширины чипов.
