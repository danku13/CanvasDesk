# FR-051: Слои, вёрстка и UI kit — реализация PRD-0009 (каркас canvas-ui: реестр, слои, capture-политики, HitStack, KeyboardRouter)

- **Статус:** в работе
- **Тип:** FR
- **Приоритет:** критично (инфраструктура для всех UI-фич; каждая новая поверхность сейчас повышает риск z-регрессий)
- **Владелец:** агент (по приказу владельца «оформляй и реализуй U0 и U1», сессия 2026-09-22)
- **Источник:** PRD-0009 (`docs/prd/prd-0009-ui-layering-uikit.md`, статус решения владельца: комбинация V-4+V-5+V-6+V-7, дорожная карта U0–U5 §13); запрос владельца 2026-09-22 о z-index/вёрстке/налезаниях; решение владельца по итогам сравнительного анализа egui/iced/Ribir — «свой Canvas UI» (S1) с точечными переносами из egui
- **Связанные задачи:** PRD-0006 (ThemeColors v2, `tokens.rs`), PRD-0007 (defense mode — будущий модальный потребитель U5+), PRD-0002/FR-042 (main stage, реестр взаимоисключимых модальностей Q6), FR-049 (галерея схем — пилот миграции U3), CR-006/CR-010/CR-011/CR-015 (прецеденты дефектов класса), ADR-0008 (зависимости), ADR-0011 (wasm-гейт)
- **Создан:** 2026-09-22
- **Обновлён:** 2026-09-22

## Описание (What)

PRD-0009 принят владельцем к реализации: системное решение проблем z-index, налезаний текстов/форм и рассинхронизации ввода через слоёную модель экрана + реестр поверхностей + layout-примитивы + UI kit + layout-линты (комбинация V-4+V-5+V-6+V-7). Данный FR — реализационная постановка PRD-0009 по его дорожной карте §13. Этап **U0** (этот документ): фиксация layer-стека (9 полос) и capture-политик (4 режима), карты переносов из egui/iced/Ribir, плана стадий. Этап **U1**: новый крейт `crates/canvas-ui` — чистые структуры каркаса (`UiLayer`, `CapturePolicy`, `SurfaceRegistry`, `UiFrame`, `HitStack`, `KeyboardRouter`) + модульные тесты (pick-матрица на модельных поверхностях) **без интеграции** — 0 правок рендера и вводных цепочек app.rs.

Выявлено владельцем (приказ «оформляй и реализуй U0 и U1») на основании PRD-0009 §13.

## Влияние (Impact)

| Объект/подсистема | Что меняется | Где в документации |
|---|---|---|
| Новый крейт `canvas-ui` | Создаётся: чистая геометрия экрана (слои, реестр, hit-test, клавиатурный роутер, фрейм) | `docs/SPEC.md` §3 (состав крейтов — обновить при U2+), `docs/ui-kit.md` (создаётся по PRD §14) |
| `canvas-app` | На U1 **не меняется** (каркас без интеграции); с U2 — RedrawRequested/on_left_button/on_key читают реестр | `docs/interface-objects/*` (по мере миграции поверхностей U5) |
| `canvas-render` | На U1 не меняется; с U2 — draw-полосы слоёв и scissor-бакеты | `docs/SPEC.md` §6 (бюджеты) |
| `design/tokens/dimensions.json` | На U1 не меняется; U3 — расширение spacing/radius/elevation-scale с паритетом | PRD-0006 §2.1 |
| Формат `.canvas` | Не затрагивается | `docs/SPEC.md` §5.1 |
| MCP | Не затрагивается | `docs/MCP.md` |

## Анализ (Root Cause) — что отсутствует

Каркас отсутствует целиком (новая функциональность):

1. **Нет слоёной модели экрана.** Порядок отрисовки UI задаётся в двух местах вручную: `canvas-app/src/app.rs:11905–12260` (сборка кадра) и `canvas-render/src/renderer.rs:1368–1442` (проходы); `canvas-render/src/zorder.rs` решает только z внутри мира (L0), не UI-полосы. История регрессий: CR-006 (select поверх виджетов), CR-010 (клип заголовка), stage-регрессия `app.rs:12115–12122` — числа/порядок вызовов рассинхронизируются.
2. **Нет реестра поверхностей.** 20+ экранных поверхностей (онбординг, галерея схем, поиск, настройки, help/docs, what-if бар, палитра шаблонов, wheel-сектора, тосты…) объявлены неявно — приоритет ввода задаётся if-цепочкой `on_left_button` (`app.rs:8923–9510`), клавиатурной лестницей `on_key` (`app.rs:7901–8226`), hover глушится списком (`app.rs:10519–10523`). Добавление поверхности = правки в 3+ местах.
3. **Нет capture-политик.** Политики «глотать всё» (модалки), «глотать клик в своих rect'ах» (доки), «не перехватывать» (empty-state) размыты по коду без формализации.
4. **Нет HitStack.** Реверс-обход слоёв с учётом скрытия за непрозрачными — реализуется заново в каждой цепочке.
5. **Нет KeyboardRouter.** Esc-стек и скоупы клавиш выводятся из лестницы if вручную; источники: egui `modal.rs` (стек модалей + Esc + any_popup_open), iced `overlay.rs` (оверлеи поверх дерева), Ribir `IgnorePointer` (аналог PassThrough) — паттерны подтверждены анализом (worklog Task 5).

## Требуемые изменения (Changes)

### U0 — фиксация решений (этот документ + PRD)

1. Layer-стек (9 полос, PRD-0009 §7.3) — **зафиксирован**:

| Слой | Полоса | Содержимое | Capture по умолчанию |
|---|---|---|---|
| L0 | `World` | Мир: карточки, рёбра, thumbs (существующий `zorder::plan_z_order` без изменений) | — (канвас) |
| L1 | `WorldOverlay` | Мир-оверлеи: wheel-сектора, guides, what-if пульс | `Capture` |
| L2 | `Widgets` | Инлайн-виджеты поверх карточек (текстовые поля карточек, порты) | `Capture` |
| L3 | `Panels` | Доки/бары: what-if бар, палитра шаблонов, поиск, настройки, help | `Capture` |
| L4 | `Popups` | Попапы: тултипы, dropdown-меню, hints, combobox-листы | `Capture` |
| L5 | `Modals` | Модали: онбординг, галерея схем, диалоги, main stage, defense (будущий) | `Block` |
| L6 | `Drag` | Drag&Drop: перетаскивание шаблонов/файлов | `Capture` |
| L7 | `Toasts` | Тосты | `Passive` |
| L8 | `Debug` | DebugOverlay (F9) | `Passive` |

2. Capture-политики (4 режима, PRD-0009 §7.3) — **зафиксированы**: `Block` (модальная: глотает всё под собой), `Capture` (глотает клик в своих rect'ах, мимо — пропускает ниже), `PassThrough` (не перехватывает ввод, рисуется), `Passive` (не интерактивна и не рисует hit-rect'ы).
3. Карта переносов из egui/iced/Ribir (уровни: L1 дословно / L2 адаптация / L3 паттерн) — **приложение А** ниже.
4. Q-дефолты PRD-0009 §11 (Q1: крейт `canvas-ui`; Q3: инкрементально; Q4: вся лестница on_key на router в U5; Q6: измерение cosmic-text + кэш) — **приняты**.

### U1 — крейт `crates/canvas-ui` (реализация в этом FR)

| Что → Где → Как |
|---|
| `Cargo.toml` → `crates/canvas-ui/` → новый крейт workspace (`crates/*` авточулен), зависимости: только `canvas-core` (токены/палитры; **0 новых внешних зависимостей** — G7); publish=false; головной комментарий-манифест (симметрия canvas-core=мир / canvas-ui=экран) |
| `src/geometry.rs` → чистые типы `UiPoint`, `UiVec2`, `UiRect` (f32, min/max-нормализация, contains, intersect, inset, перевод `[f32;2]`) — без внешних крейтов, headless-тестируемо |
| `src/layer.rs` → `UiLayer` (9 полос L0..L8, `as_u8`, `DRAW_ORDER` — производный порядок отрисовки по возрастанию; тест эквивалентности полосам PRD §7.3) |
| `src/capture.rs` → `CapturePolicy` (Block/Capture/PassThrough/Passive; `intercept_outside()` — семантика «глотает ли клик мимо своих rect'ов») |
| `src/registry.rs` → `SurfaceId` (копируемый newtype-строка), `SurfaceDecl` (id, layer, capture, keyboard_scope, degradation, label для debug-оверлея), `SurfaceRegistry` (add/ declarations в порядке регистрации; `draw_bands()` — группировка по слоям; `esc_stack()` — реверс-порядок модальных/скоуповых; тест: реестр — единственное место добавления поверхности = 1 вызов add на поверхность) |
| `src/frame.rs` → `HitRect` (rect + метка элемента + флаг interactive), `SurfaceFrame` (id + hit-rect'ы + clip-политика `ClipRect` обязательна — F-5 precursor), `UiFrame` (вьюпорт + фреймы поверхностей за кадр; `overlaps_within_layer()` — детектор пересечений для F-11 линтов) |
| `src/hit.rs` → `HitStack` (`pick(frame, point) -> Option<HitTarget>`: реверс-обход слоёв сверху вниз (L8→L0), внутри слоя — последняя зарегистрированная поверхность верхняя; `Block` глотает и останавливает обход даже без hit-rect (backdrop), `Capture` перехватывает только при попадании в hit-rect, `PassThrough`/`Passive` пропускают; элементы непрозрачного hit-rect выше — скрывают.pick ниже (egui hit_test semantics); тест-матрица G2) |
| `src/keyboard.rs` → `KeyboardScope` (именованный скоуп поверхности), `KeyboardRouter` (`push/pop` по активации поверхностей, `esc_target()` — верх активного стека, `route(key)` — верхний скоуп первым, нижние не получают при поглощении; автоматический Esc-стек из реестра — реверс-порядок регистраций модальных; тесты лестницы на модельных поверхностях) |
| `src/lib.rs` → реэкспорт + док-комментарий «как добавить поверхность за 3 шага» (U5 → docs/ui-kit.md) |
| Тесты `src/*` (TDD, детерминированные, без GPU): pick-матрица 4×(есть/нет hit) (G2 precursor); порядок draw-полос; Esc-стек из реестра; Block-backdrop глотает клик мимо rect'ов; Capture пропускает мимо rect'ов; верхний hit-rect скрывает нижний (per-layer); overlap-детектор; router-лестница; клавиатура не доходит до канваса при Block; ре-регистрация того же id — паника (контракт единственности) |

### U2+ — вне этого FR (следующие приказы)

- U2: интеграция (UiFrame-сборка из реестра в RedrawRequested, адаптеры поверхностей, renderer draw-полосы, чтение HitStack в on_left_button / Router в on_key) — отдельная ветка поверх этого FR.
- U3: TextMeasurer + токены слотов состояний + layout-примитивы + пилот (галерея схем, what-if бар).
- U4: UI kit v1 (F-8: Panel/Button/IconButton/Dropdown/Chip/Toast/Tooltip/Modal) + DebugOverlay + витрина.
- U5: миграция остальных поверхностей, вывод лестницы on_key, layout-линты в CI, docs/ui-kit.md.

## Точки входа (Entry Points)

- `docs/change-requests/index-cr-fr.md` — указатель следующего FR (FR-052) + строка FR-051.
- `docs/prd/README.md` — статус PRD-0009 «в анализе» → «в работе».
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 история изменений (U0/U1 выполнены), статус.
- `worklog.md` — запись о реализации.
- `docs/ui-kit.md` — создаётся на U5 (PRD §14); на U1 — док-комментарии в lib.rs.
- `docs/interface-objects/surface-registry.md` — контракт поверхности (U2+, PRD §14).
- `docs/ACCEPTANCE.md` — приёмочные US-1–US-5 (по мере гейтов G1–G8, U2+).
- `docs/SPEC.md` §3/§6 — состав крейтов и бюджеты (U2+, при изменении цифр).
- `Cargo.toml` workspace — без правок (членство через `crates/*`).

## Проверка (Verification)

U0:
- [x] Layer-стек 9 полос и capture-политики 4 режимов зафиксированы в документе (таблицы «Требуемые изменения»).
- [x] Карта переносов egui/iced/Ribir составлена (Приложение А).
- [x] Q-дефолты PRD-0009 приняты явно.

U1:
- [ ] Крейт `canvas-ui` собирается; зависимости — только `canvas-core` (0 новых внешних — G7).
- [ ] Pick-матрица автотестом: Block/Capture-поверхности перехватывают клик под собой; PassThrough/Passive — нет (G2 precursor, PRD US-1 AC-1.3).
- [ ] Draw-порядок выводится из реестра по возрастанию слоя (F-2 precursor); тест эквивалентен 9 полосам §7.3.
- [ ] Esc-стек выводится из реестра (реверс-порядок) — тест лестницы на модельных поверхностях.
- [ ] Clip-политика обязательна в `SurfaceFrame` (F-5 precursor) — контракт типа.
- [ ] 0 правок в `canvas-app`, `canvas-render`, `canvas-web` (каркас без интеграции — PRD U1 «0 правок рендера»); существующие z-тесты зелёные (G3).
- [ ] Гейты: `cargo fmt --all --check`; `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`; `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace`; `bash scripts/wasm_gate.sh`; `bash scripts/mcp_wasm_gate.sh` — зелёные.
- [ ] CI по merge SHA — зелёный.

## История изменений (Changelog)

- `2026-09-22` — агент: создан документ (FR-051), статус `в работе`; зафиксированы layer-стек (9 полос), capture-политики (4 режима), карта переносов (Приложение А), Q-дефолты; поставлен U1 (крейт canvas-ui без интеграции).

## Источники истины (References)

- `docs/prd/prd-0009-ui-layering-uikit.md` — §7.3 (точки интеграции, слои/модальности), §8 (F-1…F-12), §13 (U0–U5), §15 (DoD G1–G8).
- `crates/canvas-app/src/app.rs` — сборка кадра (:11905–12260), ввод (:8923–9510), лестница клавиатуры (:7901–8226), hover-глушение (:10519–10523).
- `crates/canvas-render/src/zorder.rs` — z-планировщик мира (не меняется).
- `crates/canvas-core/src/tokens.rs` — палитры/токены (единственная зависимость canvas-ui).
- Внешние референсы (анализ 2026-09-22, worklog Task 5): egui `layers.rs` (Order/LayerId), `hit_test.rs` (реверс-обход, скрытие за непрозрачными), `containers/modal.rs` (стек модалей + Esc), iced `widget/src/overlay.rs`, Ribir `IgnorePointer` — таксономия подходов, не зависимости.
- `docs/adr/adr-0008` (allowlist зависимостей), `docs/adr/adr-0011` (wasm-гейт).

---

## Приложение А: карта переносов egui → canvas-ui (итог глубокого анализа 2026-09-22)

Уровни: **L1** = дословный перенос алгоритма (минимальная адаптация типов), **L2** = адаптация (замена immediate-контекста на retained/фреймовую модель), **L3** = паттерн (идея подтверждена, реализация своя). Стадии — по §13 PRD-0009.

| egui-модуль (LOC) | Цель canvas-ui (PRD F-x) | Уровень | Стадия | Комментарий |
|---|---|---|---|---|
| `layers.rs` (263): `Order`/`LayerId`/`GraphicLayers` | `UiLayer` (F-2) | L2 | **U1** | Полосы egui → 9 полос PRD; отличие: egui — динамические LayerId с hash, у нас — фиксированная иерархия реестра; взята идея «порядок = данные, не вызовы» |
| `hit_test.rs` (553): `WidgetHits` реверс-обход, скрытие за непрозрачными, `disabled` | `HitStack` (F-3) | L2 | **U1** | Алгоритм реверс-обхода слоёв и семантика «непрозрачный верх скрывает низ» переносится; capture-политики — наша надстройка (у egui их роль играют Order+interact) |
| `containers/modal.rs` (164): стек модалей, backdrop, Esc, `any_popup_open` | `SurfaceRegistry::esc_stack` + `KeyboardRouter` (F-4) | L2 | **U1** | Модальный стек и Esc-семантика; backdrop-гла­тание = `Block` |
| `util/id_type_map.rs` (1101) | не переносится | L3 | — | У нас id — строки `SurfaceId`; типизированный map не нужен до U4 (кит) |
| `animation_manager.rs` (130): `animate_bool`/`animate_value` | kit-анимации | L3 | U4 | Линейная интерполяция с dt — тривиально воспроизводится; motion-токены PRD-0006 как источник таймингов |
| `epaint/text/text_layout.rs` (2606): галлеи, wrap/ellipsis-политики | `TextMeasurer` (F-6) | L3 | U3 | У нас cosmic-text уже владеет layout; из egui берутся **политики** (`Wrap(max_lines)`, `Ellipsis`, `Clip`) как контракт API, не код |
| `containers/popup.rs`/`tooltip.rs` | Popup/Tooltip кита (F-8) | L3 | U4 | Паттерн «якорь + flip при нехватке места + delay» |
| `containers/scroll_area.rs` | Panel/кит | L3 | U4+ | Retained-скролл — за пределами PoC (non-goal PRD §10: retained-дерево) |
| `text_selection.rs` + `widgets/text_edit/*` | TextInput (gap F-8) | L2 | U5+/v2 | Самый тяжёлый компонент; опора — существующее инлайн-редактирование `canvas-render/text.rs`; egui — референс поведений (курсор/selection/IME-преview) |
| `egui_kittest` (headless snapshot) | layout-линты CI (F-11) | L3 | U5 | Идея headless-прогонов без GPU у нас реализуется сильнее: чистая геометрия `UiFrame` уже headless by design |

**iced/Ribir идеи (L3):** iced `overlay.rs` — оверлеи как отдельная плоскость над деревом (у нас — полосы L4/L5 реестра, сильнее: слои глобальны, а не per-widget); iced `Catalog` — стилизация `fn(&Theme, Status) -> Style` как контракт кита U4 (вместо 20 параметров компонента); Ribir `IgnorePointer` — подтверждение capture-политики `PassThrough`; Ribir State→partial rebuild — аргумент против retained-дерева в PoC (частичный пересбор у нас уже есть: immediate-кадр).

**Отказ от egui-пилота поверхностного встраивания** (ранее обсуждавшийся S2): подтверждён — точечные L1/L2-переносы дают те же алгоритмы без двойного текст-движка, токен-дублирования и +1–2 МБ wasm (PRD-0009 §7.4 V-2).
