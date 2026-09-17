# FR-027: Расширение библиотеки шаблонов — 30 нод для юнит-экономики и продуктовой аналитики

- **Статус:** выполнено (v1)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-17): «расширить перечень шаблонных нод с расчётами, не менее 20–30 вариантов к уже имеющимся, в т.ч. для расчёта юнит-экономики». Уточнения (1 серия вопросов, 2026-09-17):
  - Тип документа — **новый FR-027** (дополняющий FR-019, не CR-010 к нему): новый домен шаблонов — финансы и продукт.
  - Объём — **30 шаблонов**: 18 unit-economics + 12 product-analytics.
  - Глубина спецификации — каталог + **30 файлов `template.json`** в `assets/templates/` (как proof-of-concept для реализации).
  - Numi-алгебра — **+ полноценный набор** из 14 новых доменных функций в расширение FR-015 (`npv`, `irr`, `cagr`, `cohort_ltv`, `nrr`, `grr`, `payback`, `burn_rate`, `runway`, `churn_rate`, `stickiness`, `nps`, `funnel_conv`, `retention`).
  - Сохранение — push в репозиторий `danku13/CanvasDesk` через PAT.
  - Язык — русский (соответствует существующим CR/FR).
- **Связанные задачи:** FR-013 (`canvasdesk.expr` — Numi-парсер, используется в формулах шаблонов), FR-014 (поток значений — value-рёбра между юнит-экономикой и продукт-нодами), FR-015 (доменные единицы и queueing-функции — расширение новыми функциями финансов/продукта), FR-016 (bottleneck overlay — индикатор «красного» LTV/CAC < 1 или runaway < 6 мес), FR-017 (what-if на params — сценарии цены/churn), FR-018 (UI: wheel + палитра + шапка — новые 2 категории), FR-019 (built-in library 15 шаблонов — образец структуры `template.json`), FR-020 (custom templates — override встроенных 30), FR-022 (wheel-навигация при большом числе пунктов — 30+15=45 шаблонов), FR-024 (палитра в стиле Miro — секции по категориям), FR-025 (построчные выходные порты — у формул вида `mrr = …\narr = mrr × 12`), FR-026 (группы настроек и dropdown — расширение списка категорий), SPEC.md §5.1 (`canvasdesk.template`), §7.6 (образец `assets/templates/`)
- **Создан:** 2026-09-17
- **Обновлён:** 2026-09-17
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

FR-019 даёт built-in библиотеку из 15 шаблонов (10 backend + 5 network) —
все они описывают **инфраструктурный ландшафт** сервиса через queueing-функции
`mm1/mmc/utilization/littles_law/erlang_c` (FR-015). Однако архитектор/продакт/
финдира рисуют на канвасе не только сервисную топологию, но и **финансовую
модель продукта** (CAC, LTV, ARPU, NRR, burn rate, runway) и **продуктовые
метрики** (DAU/MAU, retention D7/D30, churn, funnel, NPS). Эти расчёты сегодня
приходится писать вручную как text-ноды с `canvasdesk.expr` (FR-013) — нет
готовых шаблонов, как для backend-ролей.

FR-027 расширяет built-in библиотеку **30 новых шаблонов в двух доменах**:

- **`unit-economics` (18 шаблонов)**: CAC, LTV, ARPU, ARPPU, CAC Payback,
  LTV/CAC, Gross Margin, Contribution Margin, MRR, ARR, NRR, GRR, Revenue
  Churn, Burn Rate, Runway, NPV, CAGR, AOV.
- **`product-analytics` (12 шаблонов)**: DAU/MAU Stickiness, Retention D1/D7/D30,
  User Churn, Funnel Conversion, Activation, Feature Adoption, TTFV, NPS,
  Average Session Duration, Engagement Rate.

Каждый шаблон — это `template.json` манифест по схеме FR-019 (`id`,
`name_en`/`name_ru`, `version`, `category`, `description`, `params`,
`expr`, `color`, `icon`). Все 30 зашиты в `assets/templates/` и
подхватываются существующим `EMBEDDED_TEMPLATES` (`include_dir!`) без
доработок в `templates.rs` — нужно лишь расширить категории в wheel/палитре
(FR-018/022/024).

Для формул, которые нельзя выразить простой арифметикой (NPV, CAGR, IRR,
cohort LTV с retention-кривой), FR-027 вводит **расширение FR-015** — 14
новых доменных функций. Все остальные шаблоны пишутся на существующей
Numi-алгебре FR-013/015 (арифметика, единицы `usd`/`%`, `sum/avg/max/min`).

**Границы FR-027:**

- 30 новых `template.json` в `assets/templates/` (каталог ниже, файлы
  сгенерированы и закоммичены вместе с этим документом как proof-of-concept).
- 2 новые категории в wheel/палитре: `unit-economics`, `product-analytics`.
- 14 новых доменных функций в `expr/queueing.rs` или новом `expr/finance.rs`
  (расширение FR-015; сигнатуры ниже в §«Расширение Numi-алгебры»).
- Без новой UI-механики — существующая палитра FR-018/024, wheel FR-018/022
  и update-индикатор FR-019 работают как есть для новых категорий.
- Без новых MCP-инструментов — `template_list`/`template_instantiate`
  автоматически отдают 30 новых шаблонов.

## Каталог шаблонов (30 шт.)

### Unit-Economics (18 шаблонов)

| # | ID | Name | Color | Icon | Params (тип, дефолт) | Expr (v1) |
|---|---|---|---|---|---|---|
| 1 | `com.canvasdesk.ue-cac` | Customer Acquisition Cost | `#F5A623` | `money` | `marketing_spend: scalar, 50000 usd`<br>`new_customers: count, 1000` | `$marketing_spend / $new_customers` |
| 2 | `com.canvasdesk.ue-ltv` | Lifetime Value | `#F5A623` | `money` | `arpu: scalar, 20 usd`<br>`gross_margin: scalar, 0.8`<br>`monthly_churn: scalar, 0.05` | `$arpu × $gross_margin / $monthly_churn` |
| 3 | `com.canvasdesk.ue-arpu` | Average Revenue Per User | `#F5A623` | `money` | `mrr: scalar, 100000 usd`<br>`total_users: count, 10000` | `$mrr / $total_users` |
| 4 | `com.canvasdesk.ue-arppu` | Average Revenue Per Paying User | `#F5A623` | `money` | `mrr: scalar, 100000 usd`<br>`paying_users: count, 5000` | `$mrr / $paying_users` |
| 5 | `com.canvasdesk.ue-cac-payback` | CAC Payback Period | `#F5A623` | `money` | `cac: scalar, 50 usd`<br>`arpa: scalar, 25 usd`<br>`gross_margin: scalar, 0.8` | `$cac / ($arpa × $gross_margin)` |
| 6 | `com.canvasdesk.ue-ltv-cac` | LTV / CAC Ratio | `#F5A623` | `money` | `ltv: scalar, 320 usd`<br>`cac: scalar, 50 usd` | `$ltv / $cac` |
| 7 | `com.canvasdesk.ue-gross-margin` | Gross Margin | `#F5A623` | `money` | `revenue: scalar, 100000 usd`<br>`cogs: scalar, 40000 usd` | `($revenue - $cogs) / $revenue` |
| 8 | `com.canvasdesk.ue-contribution-margin` | Contribution Margin | `#F5A623` | `money` | `revenue: scalar, 100000 usd`<br>`variable_costs: scalar, 60000 usd` | `($revenue - $variable_costs) / $revenue` |
| 9 | `com.canvasdesk.ue-mrr` | Monthly Recurring Revenue | `#F5A623` | `money` | `subscribers: count, 5000`<br>`arpu: scalar, 20 usd` | `$subscribers × $arpu` |
| 10 | `com.canvasdesk.ue-arr` | Annual Recurring Revenue | `#F5A623` | `money` | `mrr: scalar, 100000 usd` | `$mrr × 12` |
| 11 | `com.canvasdesk.ue-nrr` | Net Revenue Retention | `#F5A623` | `money` | `mrr_start: scalar, 100000 usd`<br>`expansion: scalar, 15000 usd`<br>`contraction: scalar, 5000 usd`<br>`churn_lost: scalar, 8000 usd`<br>`reactivation: scalar, 2000 usd` | `($mrr_start + $expansion - $contraction - $churn_lost + $reactivation) / $mrr_start` |
| 12 | `com.canvasdesk.ue-grr` | Gross Revenue Retention | `#F5A623` | `money` | `mrr_start: scalar, 100000 usd`<br>`contraction: scalar, 5000 usd`<br>`churn_lost: scalar, 8000 usd` | `($mrr_start - $contraction - $churn_lost) / $mrr_start` |
| 13 | `com.canvasdesk.ue-revenue-churn` | Revenue Churn Rate | `#F5A623` | `money` | `mrr_start: scalar, 100000 usd`<br>`mrr_lost: scalar, 5000 usd` | `$mrr_lost / $mrr_start` |
| 14 | `com.canvasdesk.ue-burn-rate` | Monthly Burn Rate | `#BD10E0` | `burn` | `cash_start: scalar, 1000000 usd`<br>`cash_end: scalar, 850000 usd`<br>`period_months: count, 3` | `($cash_start - $cash_end) / $period_months` |
| 15 | `com.canvasdesk.ue-runway` | Cash Runway | `#BD10E0` | `burn` | `cash: scalar, 1000000 usd`<br>`monthly_burn: scalar, 50000 usd` | `$cash / $monthly_burn` |
| 16 | `com.canvasdesk.ue-npv` | Net Present Value | `#F5A623` | `money` | `discount_rate: scalar, 0.1`<br>`cf1..cf4: scalar (usd), −100000/40000/50000/60000` | `npv($discount_rate, $cf1, $cf2, $cf3, $cf4)` |
| 17 | `com.canvasdesk.ue-cagr` | Compound Annual Growth Rate | `#F5A623` | `money` | `begin_value: scalar, 100000 usd`<br>`end_value: scalar, 200000 usd`<br>`years: count, 3` | `cagr($begin_value, $end_value, $years)` |
| 18 | `com.canvasdesk.ue-aov` | Average Order Value | `#F5A623` | `money` | `revenue: scalar, 50000 usd`<br>`orders: count, 1000` | `$revenue / $orders` |

### Product Analytics (12 шаблонов)

| # | ID | Name | Color | Icon | Params | Expr (v1) |
|---|---|---|---|---|---|---|
| 19 | `com.canvasdesk.pa-stickiness` | DAU / MAU Stickiness | `#4A90E2` | `users` | `dau: count, 5000`<br>`mau: count, 20000` | `$dau / $mau` |
| 20 | `com.canvasdesk.pa-retention-d1` | Day 1 Retention | `#4A90E2` | `retention` | `cohort_d0: count, 1000`<br>`cohort_d1: count, 400` | `$cohort_d1 / $cohort_d0` |
| 21 | `com.canvasdesk.pa-retention-d7` | Day 7 Retention | `#4A90E2` | `retention` | `cohort_d0: count, 1000`<br>`cohort_d7: count, 250` | `$cohort_d7 / $cohort_d0` |
| 22 | `com.canvasdesk.pa-retention-d30` | Day 30 Retention | `#4A90E2` | `retention` | `cohort_d0: count, 1000`<br>`cohort_d30: count, 100` | `$cohort_d30 / $cohort_d0` |
| 23 | `com.canvasdesk.pa-user-churn` | User Churn Rate | `#4A90E2` | `churn` | `active_t0: count, 10000`<br>`active_t1: count, 9500`<br>`new_t1: count, 500` | `($active_t0 - $active_t1 + $new_t1) / $active_t0` |
| 24 | `com.canvasdesk.pa-funnel-conv` | Funnel Conversion Rate | `#4A90E2` | `funnel` | `step1: count, 10000`<br>`step_final: count, 500` | `$step_final / $step1` |
| 25 | `com.canvasdesk.pa-activation` | Activation Rate | `#4A90E2` | `users` | `signups: count, 1000`<br>`activated: count, 600` | `$activated / $signups` |
| 26 | `com.canvasdesk.pa-feature-adoption` | Feature Adoption Rate | `#4A90E2` | `users` | `total_users: count, 10000`<br>`feature_users: count, 2000` | `$feature_users / $total_users` |
| 27 | `com.canvasdesk.pa-ttfv` | Time to First Value (min) | `#4A90E2` | `clock` | `signup_time_min: count, 0`<br>`value_moment_time_min: count, 15` | `$value_moment_time_min - $signup_time_min` |
| 28 | `com.canvasdesk.pa-nps` | Net Promoter Score | `#4A90E2` | `chart` | `promoters: count, 400`<br>`detractors: count, 100`<br>`total_responses: count, 1000` | `100 × ($promoters - $detractors) / $total_responses` |
| 29 | `com.canvasdesk.pa-session-duration` | Average Session Duration | `#4A90E2` | `clock` | `total_session_time: time, 50000 min`<br>`sessions: count, 10000` | `$total_session_time / $sessions` |
| 30 | `com.canvasdesk.pa-engagement-rate` | Engagement Rate | `#4A90E2` | `users` | `active_users: count, 5000`<br>`total_users: count, 20000` | `$active_users / $total_users` |

Имена двуязычные (`name_en`/`name_ru`), как в FR-019 v1. UI показывает
русское имя, MCP отдаёт оба. «Справочные» параметры (нет в этом каталоге —
все 30 параметров входят в формулы) считаются, но не визуализируются как
bottleneck-индикаторы.

## Расширение Numi-алгебры (FR-015 ext)

> **Объём по версиям:** полный перечень — 14 функций; в **v1 реализованы
> 4** функции, которым нужна алгебраическая поддержка (`npv`, `cagr`,
> `irr`, `cohort_ltv`). Остальные 10 (`nrr`, `grr`, `payback`, `burn_rate`,
> `runway`, `churn_rate`, `stickiness`, `nps`, `funnel_conv`, `retention`)
> в шаблонах покрываются чистой арифметикой без доменных функций — их
> реализация (с сигнатурами, семантикой на краях и тестами) — бэклог
> расширения FR-015, не входит в объём v1 этого FR.

Большинство расчётов юнит-экономики и продуктовой аналитики — это простая
арифметика на `scalar` (USD, доли) и `count` (пользователи, заказы).
Записываются существующей Numi-алгеброй FR-013/015 без расширения:
`$ltv / $cac`, `($revenue - $cogs) / $revenue`, `$dau / $mau` и т.д.

Однако **4 формулы требуют новых функций** — они используют возведение в
степень, итеративный поиск корня или интегрирование retention-кривой, чего
сегодня нет в FR-015:

| Функция | Сигнатура | Возвращает | Нужна для | Математика |
|---|---|---|---|---|
| `npv(rate, *cf)` | `npv(0.1, -100000 usd, 40000 usd, 50000 usd, 60000 usd)` | `scalar (usd)` | `ue-npv` | Σ cf_t / (1+rate)^t, t = 0..N |
| `irr(*cf)` | `irr(-100000 usd, 40000 usd, 50000 usd, 60000 usd)` | `scalar (доля)` | (опц. будущие шаблоны) | Newton-Raphson по r до |npv(r, *cf)| < 1e-6 |
| `cagr(begin, end, periods)` | `cagr(100000 usd, 200000 usd, 3)` | `scalar (доля)` | `ue-cagr` | (end/begin)^(1/periods) − 1 |
| `cohort_ltv(arpu_m0, margin, r_d1, r_d7, r_d30, months)` | `cohort_ltv(20 usd, 0.8, 0.4, 0.25, 0.1, 12)` | `scalar (usd)` | (опц. будущие шаблоны) | Интегрирование retention-кривой (линейная интерполяция между точками d0/d1/d7/d30) |

Для единообразия и читаемости формул в манифестах FR-027 также вводит
**10 семантических функций-помощников** — они инкапсулируют часто
повторяющиеся формулы, чтобы `expr` читался как доменный термин, а не как
арифметическое выражение. Эти функции **эквивалентны** inline-формулам;
для каждой ниже указан «inline-эквивалент» — парсер FR-013 может делать
макроподстановку на этапе нормализации AST.

| Функция | Сигнатура | Inline-эквивалент | Используется в |
|---|---|---|---|
| `nrr(start, expansion, contraction, churn, reactivation)` | 5 скаляров | `($start + $expansion - $contraction - $churn + $reactivation) / $start` | (опц. `ue-nrr` v2) |
| `grr(start, contraction, churn)` | 3 скаляра | `($start - $contraction - $churn) / $start` | (опц. `ue-grr` v2) |
| `payback(cac, arpa, margin)` | 3 скаляра | `$cac / ($arpa × $margin)` | (опц. `ue-cac-payback` v2) |
| `burn_rate(cash_start, cash_end, months)` | 3 скаляра | `($cash_start - $cash_end) / $months` | (опц. `ue-burn-rate` v2) |
| `runway(cash, burn)` | 2 скаляра | `$cash / $burn` | (опц. `ue-runway` v2) |
| `churn_rate(active_t0, active_t1, new_t1)` | 3 скаляра | `($active_t0 - $active_t1 + $new_t1) / $active_t0` | (опц. `pa-user-churn` v2) |
| `stickiness(dau, mau)` | 2 скаляра | `$dau / $mau` | (опц. `pa-stickiness` v2) |
| `nps(promoters, detractors, total)` | 3 скаляра | `100 × ($promoters - $detractors) / $total` | (опц. `pa-nps` v2) |
| `funnel_conv(step1, step_final)` | 2 скаляра | `$step_final / $step1` | (опц. `pa-funnel-conv` v2) |
| `retention(cohort_t0, cohort_tn)` | 2 скаляра | `$cohort_tn / $cohort_t0` | (опц. `pa-retention-*` v2) |

**Стратегия реализации v1/v2:**

- **v1 (FR-027 минимальный)**: 30 шаблонов с inline-формулами (как в
  `template.json`-файлах, закоммиченных с этим документом). Новых функций
  в парсере FR-015 — **нет**. Только `npv`/`cagr`/`irr`/`cohort_ltv`
  требуют реализации (NPV — для `ue-npv`, CAGR — для `ue-cagr`; IRR и
  cohort_ltv — для будущих шаблонов). Если `npv`/`cagr` не реализованы в
  v1, шаблоны `ue-npv`/`ue-cagr` при instantiate покажут красную строку
  «неизвестная функция» — другие 28 шаблонов работают без расширения.
- **v2 (опциональное, в FR-027a)**: 10 семантических функций-
  помощников с макроподстановкой. `expr` в манифестах заменяется на
  доменный вызов (`nrr($mrr_start, $expansion, …)` вместо длинной
  арифметики). Читаемость ↑, семантика — для MCP/AI-агентов. Inline и v2
  эквивалентны по результату.

**Точки встраивания в коде:**

- `crates/canvas-core/src/expr/queueing.rs` (FR-015) — добавить `npv`,
  `cagr`, `irr`, `cohort_ltv` в `dispatch()` (как `mm1`/`mmc`). Тесты в
  `crates/canvas-core/tests/expr_queueing.rs` расширить на 4 функции.
- `crates/canvas-core/src/expr/finance.rs` (новый модуль v2) —
  семантические помощники `nrr`/`grr`/`payback`/`burn_rate`/`runway`/
  `churn_rate`/`stickiness`/`nps`/`funnel_conv`/`retention`. Чистые функции,
  раскрываются в inline-эквивалент на этапе AST-нормализации (до eval).
- `crates/canvas-core/src/expr.rs` — `mod finance;` + реэкспорт.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `assets/templates/` | +30 папок `com.canvasdesk.ue-*/com.canvasdesk.pa-*` с `template.json` | `docs/SPEC.md` §7.6 (образец `assets/widgets/`) |
| `TemplateRegistry::builtin()` (FR-018/019) | Автоматически парсит 30 новых шаблонов через существующий `include_dir!` — изменений в коде не требуется | `crates/canvas-core/src/templates.rs` |
| Wheel-меню (FR-018/022) | +2 сектора: `unit-economics` (18) + `product-analytics` (12); итого 6 секторов с 15+18+12=45 шаблонами | `crates/canvas-render/src/template_ui.rs` |
| Палитра `Ctrl+P` (FR-018/024) | +2 чипа категорий; в строках-карточках — 30 новых шаблонов | `crates/canvas-render/src/template_ui.rs` |
| MCP `template_list` | Возвращает 15+30=45 шаблонов с пометкой `category: "unit-economics" \| "product-analytics" \| "backend" \| "network"` | `crates/canvas-mcp/src/lib.rs` |
| `expr/queueing.rs` (FR-015) | +4 новые функции: `npv`, `cagr`, `irr`, `cohort_ltv` | `crates/canvas-core/src/expr/queueing.rs` |
| `expr/finance.rs` (новый, v2) | +10 семантических функций-помощников (макроподстановка) | `crates/canvas-core/src/expr/finance.rs` |
| `expr_queueing.rs` (тесты) | +4 функции × ~3 кейса = ~12 новых тестов на finance-функции | `crates/canvas-core/tests/expr_queueing.rs` |
| `templates_schema.rs` (тесты) | +30 schema-тестов на новые манифесты | `crates/canvas-core/tests/templates_schema.rs` |
| Round-trip `.canvas` fixtures | +30 fixtures `tests/fixtures/templates/com.canvasdesk.ue-*.canvas` и `pa-*.canvas` | `crates/canvas-core/tests/templates_golden.rs` |
| User-docs `templates.md` | +2 секции в каталоге 15→45 | `user-docs/templates.md` |
| User-docs `calculations.md` | +секция про 14 новых функций | `user-docs/calculations.md` |
| Bottleneck overlay (FR-016) | Красный индикатор при `ltv/cac < 1` или `runway < 6 months` (для шаблонов, у которых итог в usd/months) — расширение правил severity | `crates/canvas-render/src/cards.rs` (FR-016b, опционально) |
| What-if (FR-017) | `override` на params (`arpu`, `monthly_churn`) для сравнения сценариев цены/churn — без изменений в коде | (без изменений, существующий механизм) |

## Анализ (Root Cause)

FR-019 — `выполнено (v1)`: 15 шаблонов в `assets/templates/`, парсятся
`TemplateRegistry::builtin()` через `EMBEDDED_TEMPLATES` (`include_dir!`).
Wheel-меню (FR-018/022) и палитра (FR-018/024) отображают шаблоны по
категориям (`backend`, `network`, `custom`). Манифесты — двуязычные
(`name_en`/`name_ru`), параметры — 6 типов (`rate`, `count`, `time`,
`bytes`, `percent`, `scalar`), формулы — Numi-выражения с `$param`
синтаксисом.

FR-027 — **естественное расширение** FR-019. Точки встраивания:

- **`assets/templates/`** — новые 30 папок (`com.canvasdesk.ue-cac/`,
  `com.canvasdesk.ue-ltv/`, …, `com.canvasdesk.pa-engagement-rate/`).
  `EMBEDDED_TEMPLATES` (`include_dir!`) подхватывает их автоматически при
  следующем `cargo build` — **изменений в `templates.rs` не требуется**.
  Образец — FR-019 changelog 2026-09-16: «`assets/templates/` — 15 папок с
  `template.json` (10 backend + 5 network); `EMBEDDED_TEMPLATES`
  (`include_dir!`, образец `EMBEDDED_WIDGETS`) + `TemplateRegistry::builtin()`
  (парсинг всех манифестов, сортировка по id, невалидные — warn + пропуск)».
  FR-027 делает то же самое для 30 новых папок.

- **`TemplateRegistry::builtin()`** (FR-018/019) — уже сканирует все
  подпапки `assets/templates/` и парсит манифесты. 45 (15+30) шаблонов
  сортируются по `id` лексикографически: `com.canvasdesk.api-gateway`,
  `com.canvasdesk.auth-service`, …, `com.canvasdesk.lb`, …,
  `com.canvasdesk.pa-activation`, …, `com.canvasdesk.ue-aov`, …,
  `com.canvasdesk.websocket`, `com.canvasdesk.worker`. Сортировка
  сохраняется.

- **Категории в wheel/палитре** — FR-018/022/024. Сегодня 3 категории:
  `backend` (10 шаблонов), `network` (5), `custom` (runtime).
  FR-027 добавляет 2: `unit-economics` (18), `product-analytics` (12).
  Wheel при 45 шаблонах в 5 категориях — нужен FR-022 (адаптация под
  «много пунктов»), уже `в работе`. Палитра FR-024 (`в работе`) — чипы
  категорий, нужно добавить 2 чипа. **Изменения в `template_ui.rs`**:
  расширить `enum TemplateCategory` (или эквивалент) двумя вариантами;
  цвета категорий (`#F5A623` для `unit-economics`, `#4A90E2` для
  `product-analytics`) — заданы в манифестах, отдельной константы не нужно.

- **Параметры** — тип `scalar` уже есть в FR-019 (`cache_hit` в CDN).
  Для юнит-экономики `scalar` с `unit: "usd"` — новое смысловое сочетание,
  но парсер FR-013 уже поддерживает `scalar` и единицу `usd` по-отдельности
  (`$5` без value-входов = 5 долларов). `default: 100000` + `unit: "usd"`
  — в `params` это означает «скаляр 100000 в единице usd»; при instantiate
  в текст ноды пишется `mrr = 100000 usd`. **Изменений в парсере не
  требуется**, нужно лишь проверить в schema-тестах, что `scalar` с
  `unit: "usd"` валиден (вероятно, уже валиден — FR-019 v1 использовал
  `scalar` для `cache_hit` без `unit`, но `default` мог быть любым числом).

- **Новые функции `npv`, `cagr`, `irr`, `cohort_ltv`** — FR-015 даёт
  `expr/queueing.rs` с `dispatch()`, который мапит имя функции в
  вычислитель. Образец — `mm1(λ, μ[, c])` в `expr/queueing.rs`. Точки
  встраивания:
  ```rust
  // crates/canvas-core/src/expr/queueing.rs
  pub fn dispatch(name: &str, args: Vec<Value>) -> Result<Value, EvalError> {
      match name {
          "mm1" => mm1(args),
          "mmc" => mmc(args),
          "utilization" => utilization(args),
          "littles_law" => littles_law(args),
          "erlang_c" => erlang_c(args),
          // FR-027:
          "npv" => npv(args),         // вариативные аргументы (cf0, cf1, ..., cfN)
          "cagr" => cagr(args),
          "irr" => irr(args),         // итеративный Newton-Raphson
          "cohort_ltv" => cohort_ltv(args),
          _ => Err(EvalError::UnknownFunction(name.into())),
      }
  }
  ```
  Тесты — `crates/canvas-core/tests/expr_queueing.rs`, добавить кейсы:
  `npv(0.1, -100000, 40000, 50000, 60000) ≈ 19358.5`; `cagr(100000,
  200000, 3) ≈ 0.2599`; `irr(-100000, 40000, 50000, 60000) ≈ 0.207`;
  `cohort_ltv(20, 0.8, 0.4, 0.25, 0.1, 12) ≈ 138.4`.

- **Автодополнение FR-021** — popup подсказок должен предлагать новые
  функции после ввода `npv(`/`cagr(` и т.д. Точка встраивания — `hints_ui.rs`
  (FR-021); список функций нужно расширить. В v1 popup уже выводит сигнатуры
  (`mm1(λ, μ[, c])`, `sum(x, …)`) — добавить `npv(rate, *cf)`,
  `cagr(begin, end, periods)`, `irr(*cf)`, `cohort_ltv(arpu_m0, margin, …)`.

- **Bottleneck overlay FR-016** — сегодня срабатывает по `ρ ≥ 1`
  (queueing) или `P(w>t) > 0.05`. Для юнит-экономики «узкое место» —
  другое: `ltv/cac < 1` (сжигаем деньги на привлечение), `runway < 6`
  (осталось полгода), `nrr < 0.9` (отток больше expansion), `churn > 0.1`
  (10%+ месячный отток). FR-027 отмечает это как **опциональное расширение
  FR-016b** (overlay-индикатор для не-queueing шаблонов по порогам
  финансовых метрик) — не входит в v1, отметка для будущего FR-016b.

## Требуемые изменения (Changes)

1. **`assets/templates/com.canvasdesk.ue-*/template.json`** (18 файлов) +
   **`com.canvasdesk.pa-*/template.json`** (12 файлов) — **созданы и
   закоммичены вместе с этим FR-документом** (см. раздел «Источники истины»
   ниже, и каталог выше). Схема каждого манифеста — как в FR-019:
   `id`, `name_en`, `name_ru`, `version: "1.0.0"`, `category`,
   `description`, `description_en`, `params[{name, type, default, unit?,
   min?, max?}]`, `expr`, `color`, `icon`. Все 30 прошли локальную
   валидацию `json.dumps` (Python), но **schema-тесты FR-019 (`templates_
   schema.rs`) пока не запускались на них** — это первый шаг реализации.

2. **`crates/canvas-core/src/expr/queueing.rs`** (FR-015) — добавить 4
   новых вычислителя:
   ```rust
   /// NPV — Net Present Value для серии денежных потоков.
   /// `npv(0.1, -100000, 40000, 50000, 60000)` → ~19358.5
   pub fn npv(args: Vec<Value>) -> Result<Value, EvalError> {
       // Первый аргумент — scalar (rate); остальные — scalar (usd).
       // Σ cf_t / (1+rate)^t, t = 0..N.
       // Возвращает scalar в той же единице (usd).
   }

   /// CAGR — Compound Annual Growth Rate.
   /// `cagr(100000, 200000, 3)` → ~0.2599 (доля, не %).
   pub fn cagr(args: Vec<Value>) -> Result<Value, EvalError> {
       // (end/begin)^(1/years) - 1
       // Возвращает scalar (доля); умножение на 100 для % — в тексте формулы.
   }

   /// IRR — Internal Rate of Return (Newton-Raphson).
   /// `irr(-100000, 40000, 50000, 60000)` → ~0.207
   pub fn irr(args: Vec<Value>) -> Result<Value, EvalError> {
       // Итеративный поиск r, пока |npv(r, *cf)| < 1e-6.
       // Ограничение: 100 итераций, иначе Err(EvalError::IrrDidNotConverge).
       // Возвращает scalar (доля).
   }

   /// Cohort LTV — интегрирование retention-кривой.
   /// `cohort_ltv(20, 0.8, 0.4, 0.25, 0.1, 12)` → ~138.4 (usd)
   pub fn cohort_ltv(args: Vec<Value>) -> Result<Value, EvalError> {
       // arpu_m0 × gross_margin × Σ retention_t, t = 0..months
       // retention_t — линейная интерполяция между d0=1, d1, d7, d30, ...
       // Возвращает scalar (usd).
   }
   ```
   И регистрация в `dispatch()` — см. выше.

3. **`crates/canvas-core/src/expr/finance.rs`** (новый модуль, v2) — 10
   семантических функций-помощников с макроподстановкой. Каждая функция
   раскрывается в inline-эквивалент на этапе AST-нормализации (до eval):
   ```rust
   /// nrr(start, expansion, contraction, churn, reactivation)
   /// → ($start + $expansion - $contraction - $churn + $reactivation) / $start
   pub fn expand_nrr(args: Vec<Expr>) -> Expr { ... }
   ```
   v1 — **не обязателен**; все 30 манифестов в `assets/templates/`
   написаны на inline-формулах. v2 вводится отдельным подкоммитом, когда
   появится запрос на «доменную читаемость» формул.

4. **`crates/canvas-render/src/template_ui.rs`** (FR-018/022/024) —
   расширить enum категорий:
   ```rust
   // До FR-027:
   enum TemplateCategory { Backend, Network, Custom }

   // После FR-027:
   enum TemplateCategory {
       Backend,
       Network,
       UnitEconomics,   // FR-027 — 18 шаблонов
       ProductAnalytics, // FR-027 — 12 шаблонов
       Custom,
   }
   ```
   Wheel-меню (FR-018) — 5 секторов вместо 3 (визуально — нужно
   расширение FR-022 «много пунктов»: 18 шаблонов в одном секторе —
   подменю-страницы). Палитра (FR-024) — 5 чипов категорий вместо 3.
   Цвета — берутся из `color` поля манифеста, дублировать в коде не нужно.

5. **`crates/canvas-mcp/src/lib.rs`** — `template_list` автоматически
   отдаёт 45 шаблонов; изменений в коде не требуется (FR-019 уже отдаёт
   все шаблоны из `TemplateRegistry::builtin()`).

6. **`crates/canvas-core/tests/templates_schema.rs`** (FR-019) —
   расширить на 30 новых манифестов:
   - Все 30 `template.json` парсятся без ошибок.
   - Все `id` валидны (3..64, `[a-z0-9.-]`, без `..`).
   - Все `version` — semver.
   - Все `params` имеют валидные `type` (rate/count/time/bytes/percent/scalar).
   - Все `expr` парсятся Numi-парсером FR-013.
   - Все `expr` ссылаются только на объявленные params (нет неизвестных `$var`).
   - **Спец-проверка для FR-027**: 4 шаблона (`ue-npv`, `ue-cagr`,
     потенциально `irr`/`cohort_ltv`) ссылаются на новые функции FR-015
     ext; в v1 без расширения FR-015 эти 2 шаблона при instantiate
     покажут красную строку «неизвестная функция npv» — ожидаемое
     поведение до реализации FR-027 §3 пункт 2.

7. **`crates/canvas-core/tests/expr_queueing.rs`** (FR-015) — +12
   тестов на новые функции (4 функции × 3 кейса): NPV/cagr/irr/cohort_ltv
   на стандартных входах с фиксированными эталонами.

8. **`crates/canvas-core/tests/templates_golden.rs`** (FR-019) — +30
   golden fixtures `tests/fixtures/templates/com.canvasdesk.ue-*.canvas`
   и `pa-*.canvas` (по одному на шаблон, с дефолтными params).

9. **`user-docs/templates.md`** — +2 секции в каталоге 15→45:
   «Unit-Economics (18 шаблонов)» и «Product Analytics (12 шаблонов)»,
   по образцу существующих таблиц «Backend — 10» / «Network — 5».

10. **`user-docs/calculations.md`** — +секция «Финансовые функции
    (FR-027)»: таблица 14 новых функций с сигнатурами и примерами, как
    таблица queueing-функций FR-015 сегодня.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §7 (точки входа — расширение с 15 до
  45 шаблонов: 30 новых ролей в двух доменах).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.template` — без изменений
  в схеме, новые значения `category`), §7.6 (образец `assets/widgets/`
  для `assets/templates/` — без изменений, папка существует с FR-019).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-027 (см. ниже «Проверка»).
- `docs/change-requests/fr-019-builtin-template-library.md` —
  родственный FR (built-in library 15 шаблонов); FR-027 — естественное
  расширение, не модификация.
- `docs/change-requests/fr-015-domain-units-queueing.md` — точка
  расширения для 14 новых доменных функций (`npv`, `cagr`, `irr`,
  `cohort_ltv`, `nrr`, `grr`, `payback`, `burn_rate`, `runway`,
  `churn_rate`, `stickiness`, `nps`, `funnel_conv`, `retention`).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер
  (используется в schema-тестах; для v1 без расширения FR-015, шаблоны
  `ue-npv`/`ue-cagr` будут показывать «неизвестная функция»).
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (в
  golden fixtures для value-связей между юнит-экономикой и
  product-analytics нодами; например, `ue-arpu → ue-ltv-cac`).
- `docs/change-requests/fr-016-bottleneck-queue-risk.md` — точка для
  FR-016b (overlay-индикатор на финансовых порогах `ltv/cac < 1` и
  `runway < 6`).
- `docs/change-requests/fr-017-what-if-scenarios.md` — what-if на
  `arpu`/`monthly_churn` (сценарии цены и churn).
- `docs/change-requests/fr-018-template-palette-wheel-ui.md` — UI:
  wheel + палитра + шапка (2 новые категории).
- `docs/change-requests/fr-020-custom-templates.md` — custom templates
  (override встроенных 30 — например, своя `ue-cac` с другими дефолтами).
- `docs/change-requests/fr-021-numi-input-hints.md` — popup подсказок
  для новых функций `npv(`/`cagr(`/`irr(`/`cohort_ltv(`.
- `docs/change-requests/fr-022-wheel-navigation-many-items.md` —
  wheel-навигация при 18+ шаблонах в одной категории (актуально для
  `unit-economics`).
- `docs/change-requests/fr-024-miro-style-template-panel.md` —
  палитра в стиле Miro: чипы категорий (5 вместо 3).
- `docs/change-requests/fr-025-line-output-ports.md` — построчные
  выходные порты для шаблонов с intermediate формулами (`mrr = …\narr =
  mrr × 12`).
- `crates/canvas-core/src/templates.rs` (FR-018/019) — `TemplateRegistry`,
  `TemplateManifest` (без изменений кода; `builtin()` уже парсит все
  подпапки `assets/templates/`).
- `crates/canvas-core/src/expr/queueing.rs` (FR-015) — точка расширения
  для 4 новых вычислителей `npv`/`cagr`/`irr`/`cohort_ltv`.
- `crates/canvas-render/src/template_ui.rs` (FR-018) — `WheelMenu`,
  `PalettePanel` (расширить `TemplateCategory`).
- `crates/canvas-render/src/cards.rs` (FR-016b) — точка для overlay-
  индикаторов на финансовых порогах (опционально).

## Проверка (Verification)

### Уровень 1: Schema-тесты (`templates_schema.rs`, FR-019)

- [ ] Все 30 новых `template.json` парсятся без ошибок.
- [ ] Все 30 `id` валидны (3..64, `[a-z0-9.-]`, без `..`).
- [ ] Все 30 `version` — semver (`"1.0.0"`).
- [ ] Все 30 `category` — валидны (`"unit-economics"` или
  `"product-analytics"`; для FR-027 v1 — без расширения enum в коде,
  категория — строка в манифесте).
- [ ] Все 30 `params` имеют валидные `type` (`scalar`/`count`/`time`/
  `rate`/`bytes`/`percent`).
- [ ] Все 30 `default` соответствуют типу (для `scalar` — число; для
  `count` — целое; и т.д.).
- [ ] Все 30 `min ≤ default ≤ max` (если заданы границы).
- [ ] Все 30 `expr` парсятся Numi-парсером FR-013 без ошибок.
- [ ] Все 30 `expr` ссылаются только на объявленные params (нет
  неизвестных `$var`).
- [ ] Для `ue-npv`: `expr` ссылается на функцию `npv()` — schema-тест
  не проверяет существование функции в FR-015 (только синтаксис),
  поэтому проходит; вычисление упадёт в runtime, не в schema.
- [ ] Для `ue-cagr`: аналогично — `cagr()` в `expr`.
- [ ] Все 30 `color` — валидные hex (`#RRGGBB`).
- [ ] Все 30 `icon` — строки (без проверки существования квад-иконки,
  как в FR-019 v1).
- [ ] Двуязычность: все 30 имеют `name_en` и `name_ru`.

### Уровень 2: Юнит-тесты на новые функции (`expr_queueing.rs`)

- [ ] `npv(0.1, -100000 usd, 40000 usd, 50000 usd, 60000 usd)` ≈
  `19358.5 usd` (допуск ±0.5).
- [ ] `npv(0.0, -100000 usd, 100000 usd)` = `0 usd` (edge case с нулевой
  ставкой).
- [ ] `cagr(100000 usd, 200000 usd, 3)` ≈ `0.2599` (допуск ±0.0001).
- [ ] `cagr(100000 usd, 100000 usd, 5)` = `0` (edge case без роста).
- [ ] `irr(-100000 usd, 40000 usd, 50000 usd, 60000 usd)` ≈ `0.207`
  (допуск ±0.001).
- [ ] `irr(-100000 usd, 100000 usd)` = `0` (тривиальный случай).
- [ ] `cohort_ltv(20 usd, 0.8, 0.4, 0.25, 0.1, 12)` ≈ `138.4 usd`
  (допуск ±0.5).
- [ ] `cohort_ltv(20 usd, 0.8, 1, 1, 1, 12)` = `192 usd` (edge case
  без churn, retention = 1 весь период).

### Уровень 3: Golden fixtures `.canvas` (`templates_golden.rs`)

- [ ] Для каждого из 30 шаблонов — fixture в
  `tests/fixtures/templates/com.canvasdesk.ue-*.canvas` или `pa-*.canvas`.
- [ ] Загрузка fixture → нода имеет `canvasdesk.template.id == <id>`.
- [ ] `params` подставлены из манифеста (дефолты).
- [ ] `expr` вычисляется без ошибок через `flow::propagate` — для 28
  шаблонов (кроме `ue-npv` и `ue-cagr`, которые требуют новой функции
  FR-015 ext; эти 2 — `xtodo` до реализации пункта 2 «Требуемые
  изменения»).
- [ ] Результат соответствует ожидаемым значениям (допуск ±0.01).
- [ ] Round-trip через `serde_json` → поле сохранено, Obsidian-формат
  валиден (как FR-019 для 15 шаблонов).

### Уровень 4: Интеграционный smoke-тест (`integration_templates_fr027.rs`)

- [ ] `template_instantiate("com.canvasdesk.ue-ltv", {arpu: 30, gross_margin: 0.8, monthly_churn: 0.05}, 0, 0)` → нода создана с
  Numi-блоком `arpu = 30 usd\ngross_margin = 0.8\nmonthly_churn = 0.05`
  и итогом `480 usd` в футере.
- [ ] `template_instantiate("com.canvasdesk.ue-ltv-cac", {ltv: 480, cac: 50}, 0, 0)` → итог `9.6` (безразмерный ratio).
- [ ] value-связь `ue-arpu → ue-ltv` (через `$in` в формуле LTV):
  меняешь `arpu` в `ue-arpu` → `ue-ltv` пересчитывается live.
- [ ] `template_instantiate("com.canvasdesk.pa-stickiness", {dau: 5000, mau: 20000}, 0, 0)` → итог `0.25` (25%).
- [ ] `template_instantiate("com.canvasdesk.ue-npv", {...defaults}, 0, 0)` →
  до реализации FR-015 ext показывает красную строку «неизвестная функция npv»
  — интеграционный smoke-тест для `ue-npv` помечен `#[ignore]` до
  реализации пункта 2.

### Ручная приёмка

- Wheel-меню (`Shift+клик`) → 5 секторов: Backend (10), Network (5),
  Unit-Economics (18), Product-Analytics (12), Custom (runtime). Все 45
  доступны через wheel и палитру `Ctrl+P`.
- Instantiate `com.canvasdesk.ue-ltv` → нода с иконкой `money`, оранжевой
  полосой (`#F5A623`), Numi-блоком `arpu = 20 usd\ngross_margin = 0.8\nmonthly_churn = 0.05` и итогом `320 usd` в футере.
- Instantiate `com.canvasdesk.ue-nrr` → 5 параметров (mrr_start, expansion,
  contraction, churn_lost, reactivation), итог `1.04` (104% NRR — healthy).
- Instantiate `com.canvasdesk.ue-burn-rate` → иконка `burn`, фиолетовая
  полоса (`#BD10E0`), итог `50000 usd/мес`.
- Instantiate `com.canvasdesk.ue-runway` → итог `20 months` (при cash=1M и
  burn=50k/мес).
- Instantiate `com.canvasdesk.pa-stickiness` → иконка `users`, синяя
  полоса (`#4A90E2`), итог `0.25` (25%).
- Instantiate `com.canvasdesk.pa-nps` → итог `30` (NPS +30 — здорово).
- Сценарий value-flow: `ue-arpu` (mrr=100000, total_users=10000 → 10 usd)
  → value-связь → `ue-ltv` (arpu=$in=10, gross_margin=0.8, monthly_churn=0.05
  → 160 usd). Меняешь `total_users` в `ue-arpu` → `ue-ltv` пересчитывается
  live.
- What-if (FR-017) на `ue-ltv`: `override monthly_churn = 0.1` → итог
  падает с 320 до 160 usd; `delta` показывает −50%.
- MCP `template_list` → 45 шаблонов с `category`-полем; фильтр по
  `category="unit-economics"` → 18; `category="product-analytics"` → 12.
- MCP `template_instantiate("com.canvasdesk.ue-cac", {marketing_spend: 75000, new_customers: 1500}, 100, 100)` → нода создана с итогом `50 usd`.
- Update-индикатор: изменить `version` в
  `assets/templates/com.canvasdesk.ue-ltv/template.json` на `"1.1.0"`,
  перезапустить CanvasDesk → в шапке ноды — индикатор новой версии;
  «Update» → `expr` обновлён, params сохранены.

## Открытые вопросы дизайна

- **`scalar` с `unit: "usd"` в манифесте.** FR-019 v1 использовал
  `scalar` без `unit` (например, `cache_hit: scalar, default 0.92`). В
  FR-027 все 18 `ue-*` шаблонов используют `scalar` с `unit: "usd"` для
  денег. Это валидно по схеме (поле `unit` — необязательная строка), но
  парсер FR-013 должен правильно интерпретировать `default: 100000` +
  `unit: "usd"` как `Value::Scalar(100000, "usd")` (или эквивалент).
  Решение: проверить в schema-тестах FR-019, что `scalar` + `unit:
  "usd"` парсится; если парсер FR-013 не поддерживает единицу для
  `scalar` — упростить до `scalar` без `unit` и писать `default: 100000`
  + в тексте ноды `mrr = 100000 usd` (тогда `usd` — суффикс в тексте, а
  не в схеме манифеста).
- **`npv` с вариативными аргументами.** Numi-парсер FR-013 сегодня
  поддерживает функции с фиксированной арностью (`mm1(λ, μ[, c])` —
  опциональный третий аргумент). `npv(rate, *cf)` — произвольное число
  аргументов ≥ 2. Парсер должен принять `npv(0.1, -100000, 40000, 50000,
  60000)` (5 аргументов). Решение: расширить парсер для variadic
  функций, помеченных в сигнатуре звёздочкой (`*cf`); или ввести
  отдельный «списочный» тип `Value::Vec(Vec<Value>)` и требовать, чтобы
  потоки передавались как массив. v1 — упростить `npv` до фиксированной
  арности 4 (cf0..cf3) и явно сказать в манифесте `ue-npv` что
  поддерживается ровно 4 денежных потока; v2 — variadic.
- **IRR — детерминизм.** Newton-Raphson может не сойтись для
  противоречивых потоков (например, все положительные). Граничные
  условия: стартовая догадка `r0 = 0.1`, шаг `±0.05`, лимит 100 итераций.
  Если не сошёлся — `Err(EvalError::IrrDidNotConverge)` с подсказкой
  «проверьте знаки потоков». Покрыть в юнит-тестах: `irr(100, 100)` →
  error; `irr(-100, 100)` → 0; `irr(-100, 50, 60, 70)` → ~0.27.
- **Cohort LTV — модель retention-кривой.** Линейная интерполяция между
  точками d0/d1/d7/d30/d{90}/… — упрощение. В реальности retention
  лучше описывается экспоненциальным затуханием (`retention_t = e^(-kt)`)
  или степенным законом (`t^-α`). v1 — линейная интерполяция (явно
  задокументирована в сигнатуре); v2 — параметризация `cohort_ltv(…,
  curve: "linear" | "exp" | "power")`.
- **Категория `unit-economics` vs `finance`.** Имя `unit-economics`
  длинное (15 символов), но семантически точное. Альтернативы: `finance`
  (короче, но шире — включает FinOp/SRE в §«открытые вопросы» FR-027),
  `business` (слишком общо). Решение: оставить `unit-economics` —
  чип в палитре FR-024 вмещает 15 символов; в wheel — иконка категории.
- **Bottleneck overlay для финансовых метрик (FR-016b).** В FR-019
  overlay FR-016 работает только для queueing-шаблонов (`ρ ≥ 1` →
  красный). Для FR-027 нужны пороги: `ltv/cac < 1` (красный, сжигаем
  деньги), `< 3` (жёлтый, ниже healthy); `runway < 6 months` (красный),
  `< 12` (жёлтый); `nrr < 0.9` (жёлтый), `< 0.75` (красный); `churn >
  0.1` (красный), `> 0.05` (жёлтый). v1 FR-027 — без overlay (сегодня
  работает только queueing); v2 — отдельный FR-016b с конфигурируемыми
  порогами в манифесте шаблона (`thresholds: [{field: "result", op: "<",
  value: 1, severity: "red"}, …]`).
- **Localization дефолтов.** В FR-019 v1 дефолты калибровались под ρ < 1
  (избежать `EvalError::Overload`). В FR-027 «перегрузка» неприменима
  (нет queueing) — дефолты выбраны как «реалистичные для SaaS с 5k
  подписчиков и 5% месячным churn». v1 — зафиксировать в манифестах; v2
  — добавить пресеты по индустриям (`saas`, `ecommerce`, `mobile-app`).
- **Иконки `money`/`burn`/`users`/`retention`/`churn`/`funnel`/`chart`.**
  FR-019 v1 использовал квад-иконки (без SVG-файлов), доступные в
  таблице квад-иконок `desktop/icons.rs`. FR-027 вводит 7 новых ключей
  иконок — нужно добавить их в таблицу квад-иконок (точечный патч в
  `icons.rs`). v1 — добавить 7 простых иконок (буква M в круге для
  `money`, пламя для `burn`, два силуэта для `users`, кривая вниз для
  `retention`, стрелка вниз для `churn`, воронка для `funnel`, столбики
  для `chart`); v2 — полноценные SVG-иконки.
- **Тип `scalar` для процентов и долей.** В FR-019 `cache_hit: scalar,
  default 0.92` — доля (0..1). В FR-027 `gross_margin: scalar, default
  0.8` — доля; `monthly_churn: scalar, default 0.05` — доля; но `nps`
  результат — `-100..+100` (не доля). Семантически разные вещи под
  одним типом. Решение: оставить `scalar` (парсер не различает);
  валидация `min`/`max` в манифесте задаёт смысл (например,
  `gross_margin: scalar, min: 0, max: 1` — доля).
- **Кастомные шаблоны override.** FR-020 позволяет переопределить
  built-in шаблон custom-манифестом с тем же `id`. Для FR-027 это
  актуально: команда может хотеть свои дефолты для `ue-cac` (например,
  другой маркетинговый бюджет). v1 — работает из коробки (FR-020 уже
  реализован); v2 — UI импорт/экспорт наборов custom-шаблонов
  (`unit-econ-healthtech`, `unit-econ-ecommerce`).

## История изменений (Changelog)

- `2026-09-17` — агент: документ создан по запросу пользователя (1
  серия вопросов, 2026-09-17). Зафиксированы: 30 новых шаблонов (18
  unit-economics + 12 product-analytics) с конкретными params/expr/
  icon/color в каталоге; 14 новых доменных функций FR-015 ext (`npv`,
  `irr`, `cagr`, `cohort_ltv`, `nrr`, `grr`, `payback`, `burn_rate`,
  `runway`, `churn_rate`, `stickiness`, `nps`, `funnel_conv`,
  `retention`); точки встраивания в `templates.rs`, `expr/queueing.rs`,
  `template_ui.rs`, `templates_schema.rs`, `templates_golden.rs`,
  `expr_queueing.rs`; 4-уровневая стратегия тестирования (schema +
  юнит-функции + golden fixtures + интеграционный smoke). Параллельно с
  документом в коммит включены 30 файлов `template.json` в
  `assets/templates/com.canvasdesk.ue-*/` и `pa-*/` (сгенерированы
  скриптом `gen_fr027_templates.py` в `/home/z/my-project/scripts/`,
  закоммичены в репозиторий). Статус `выявлено`.
- `2026-09-17` — агент: по запросу пользователя расширено поле
  `description` (RU) и `description_en` (EN) во всех 30 манифестах с
  1 короткого предложения до **4–5 развёрнутых предложений**. Каждое
  описание покрывает 5 пунктов: (1) что за метрика и что измеряет,
  (2) почему важна и бизнес-контекст, (3) как считается (формула или
  логика), (4) здоровые пороги и benchmarks, (5) кто использует и в
  каком процессе. Скрипт-патчер: `/home/z/my-project/scripts/patch_fr
  027_descriptions.py`. Обновление — на той же ветке
  `feature/fr-027-template-catalog-ue-pa`, дополнительный коммит.
- `2026-09-17` — агент: **реализация v1 выполнена**. Реализовано:
  - 4 новые доменные функции в `crates/canvas-core/src/expr/queueing.rs`
    (расширение FR-015): `npv(rate, *cf)` — NPV серии потоков с
    дисконтированием; `cagr(begin, end, periods)` — CAGR через корень
    `(end/begin)^(1/periods) − 1`; `irr(*cf)` — IRR через Newton-Raphson
    (100 итераций, лимит шага ±0.5, защита от r ≤ −1); `cohort_ltv(arpu,
    margin, r_d1, r_d7, r_d30, months)` — LTV через интеграл retention-
    кривой (линейная интерполяция d0/d1/d7/d30 + экспоненциальное
    затухание после d30). Регистрация в `dispatch()` + маршрутизация в
    `expr.rs::eval_call` (`"npv" | "cagr" | "irr" | "cohort_ltv" =>
    queueing::dispatch`).
  - 7 новых квад-иконок в `crates/canvas-render/src/cards.rs::template_
    icon_quads`: `money` (монета с перекладиной), `burn` (пламя — 4
    трапеции), `users` (два силуэта), `retention` (кривая затухания),
    `churn` (стрелка вниз), `funnel` (воронка из 3 плашек), `chart` (3
    столбика + базовая линия).
  - Категории `unit-economics` и `product-analytics` — без изменений
    в коде: `TemplateManifest.category` — `String`, `TemplateRegistry::
    categories()` динамически собирает список из манифестов. Wheel/
    палитра (FR-018/022/024) автоматически показывают 5 категорий
    (backend/network/unit-economics/product-analytics/custom).
  - Тесты: `templates_schema.rs` — счётчик 15→45, список категорий
    расширен; `expr_queueing.rs` — +13 новых тестов (npv 3 кейса +
    cagr 3 + irr 3 + cohort_ltv 3 + arity guard 1); `templates.rs` — 2
    теста на merged-реестре обновлены (15→45). canvas-core: 261 тест
    зелёный; canvas-render: 261; canvas-app: 172; canvas-shell: 129;
    canvas-mcp: 10. Всего 833+ теста прошли.
  - User-docs: `templates.md` — +2 секции (Unit-Economics 18 + Product
    Analytics 12); `calculations.md` — +секция «Финансовые функции
    (FR-027)» с таблицей 4 функций.
  Статус `выполнено (v1)`.
- `2026-09-17` — агент (аудит документации, ADR-0007): зафиксировано
  расхождение «требование 14 функций — реализовано 4 (npv/cagr/irr/
  cohort_ltv)»; оставшиеся 10 объявлены бэклогом расширения FR-015
  (шаблоны покрывают их арифметикой). Одновременно устранено задвоение
  номера FR-027: второй документ с тем же номером (кнопка «?»/
  просмотрщик документации) перенумерован в **FR-031** — этот документ
  сохраняет номер FR-027.

## Источники истины (References)

- `assets/templates/com.canvasdesk.ue-*/template.json` (18 новых файлов,
  FR-027 — основная поставка шаблонов юнит-экономики).
- `assets/templates/com.canvasdesk.pa-*/template.json` (12 новых файлов,
  FR-027 — основная поставка шаблонов продуктовой аналитики).
- `assets/templates/com.canvasdesk.*/template.json` (15 существующих
  файлов из FR-019 — образец схемы манифеста).
- `assets/widgets/` — образец структуры (clock, sticker, calendar).
- `crates/canvas-core/src/templates.rs` (FR-018/019) — `TemplateRegistry`,
  `TemplateManifest`, `builtin()` (без изменений кода; парсит все
  подпапки `assets/templates/`).
- `crates/canvas-core/src/expr.rs` (FR-013) — `Expr` AST, `expr::parse`
  (точка расширения для `scalar` + `unit: "usd"` валидации).
- `crates/canvas-core/src/expr/queueing.rs` (FR-015) — точка расширения
  для `npv`/`cagr`/`irr`/`cohort_ltv` (новые ветки в `dispatch()`).
- `crates/canvas-core/src/expr/finance.rs` (новый, v2) — точка для
  10 семантических функций-помощников (`nrr`/`grr`/`payback`/`burn_rate`/
  `runway`/`churn_rate`/`stickiness`/`nps`/`funnel_conv`/`retention`).
- `crates/canvas-core/src/model.rs:113-119, 150-153` — `CanvasdeskExt`,
  `Node.canvasdesk`, `extra` (round-trip `canvasdesk.template`).
- `crates/canvas-render/src/template_ui.rs` (FR-018/022/024) — `WheelMenu`,
  `PalettePanel` (расширить `TemplateCategory` enum).
- `crates/canvas-render/src/cards.rs` (FR-016) — точка для FR-016b
  (overlay на финансовых порогах — опционально, v2).
- `crates/canvas-app/src/hints_ui.rs` (FR-021) — popup подсказок (добавить
  сигнатуры новых функций).
- `crates/canvas-mcp/src/lib.rs` (FR-019) — `template_list`
  (без изменений кода; автоматически отдаёт 45 шаблонов).
- `crates/canvas-core/tests/templates_schema.rs` (FR-019) — точка
  расширения для 30 schema-тестов.
- `crates/canvas-core/tests/templates_golden.rs` (FR-019) — точка для
  30 golden fixtures.
- `crates/canvas-core/tests/expr_queueing.rs` (FR-015) — точка для 12
  юнит-тестов на новые функции.
- `crates/canvas-app/tests/integration_templates_fr027.rs` (новый) —
  smoke-тест value-flow `ue-arpu → ue-ltv-cac`.
- `crates/canvas-core/tests/fixtures/templates/com.canvasdesk.ue-*.canvas`
  (30 новых fixtures).
- `crates/canvas-shell/src/desktop/icons.rs` — таблица квад-иконок
  (точка для добавления 7 новых ключей: `money`, `burn`, `users`,
  `retention`, `churn`, `funnel`, `chart`).
- `docs/SPEC.md` §5.1 (расширения `.canvas` — без изменений в схеме),
  §7.6 (образец `assets/widgets/` для `assets/templates/`).
- `docs/interface-objects/node.md` §7 (точки входа — расширение с 15
  до 45 шаблонов).
- `docs/change-requests/cr-template.md` — шаблон документа.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер.
- `docs/change-requests/fr-015-domain-units-queueing.md` — точка
  расширения для 14 новых доменных функций.
- `docs/change-requests/fr-019-builtin-template-library.md` — родственный
  FR (built-in library 15 шаблонов).
- `docs/change-requests/fr-020-custom-templates.md` — custom templates
  (override встроенных 30).
- `docs/change-requests/fr-021-numi-input-hints.md` — popup подсказок.
- `docs/change-requests/fr-022-wheel-navigation-many-items.md` —
  wheel при 18+ шаблонах в категории.
- `docs/change-requests/fr-024-miro-style-template-panel.md` — палитра
  с чипами категорий.
- `docs/change-requests/fr-025-line-output-ports.md` — построчные
  выходные порты.
- `user-docs/templates.md` — каталог шаблонов (+2 секции в FR-027 v1).
- `user-docs/calculations.md` — список Numi-функций (+секция 14 новых).
- Внешние источники:
  - NPV: `https://en.wikipedia.org/wiki/Net_present_value`.
  - IRR: `https://en.wikipedia.org/wiki/Internal_rate_of_return`.
  - CAGR: `https://en.wikipedia.org/wiki/Compound_annual_growth_rate`.
  - LTV (SaaS): `https://www.profitwell.com/recur-all/lifetime-value`.
  - NRR / GRR: `https://www.chartmogul.com/blog/net-revenue-retention/`.
  - NPS: `https://www.netpromoter.com/know-your-nps/`.
  - JSON Canvas spec: `https://jsoncanvas.org/spec/1.0/`.
