# План FR-079: Suggest-движок (гибрид lex+Laya) — технический концепт на ревью

**Дата:** 2026-09-28
**Статус:** черновик на ревью владельца — реализация не начата
**Родитель:** `docs/change-requests/fr-079-inline-ai-suggest-engine.md`
**Входные данные:** архив PoC волны 1 (harness `e6e666e`), отчёты волн 1–3
(`laya-poc-report.md`, `laya-poc-wave3-results.md`), гипотеза ред. 2.1 §8–14

Здесь зафиксирован **вариант реализации**: архитектура, контракты, конфиг,
пайплайн fine-tune, тест-стратегия, оценки. После утверждения план дробится
на задачи по стадиям S0–S5 (каждая — отдельная сессия/коммит по AGENTS.md).

---

## 1. Архитектура: трёхуровневый конвейер с одним клиентом

```
                    ┌─────────────────────────────────────────────────┐
                    │ canvas-app (UI-поток)                           │
                    │                                                 │
  ввод в ноде ──────┤ update_hints() [overlays.rs:1776]               │
  (debounce 300 мс) │   ├── L0: hint_items() — как сейчас, <1 мс      │
                    │   ├── домен-детектор (C4-гейт)                  │
                    │   ├── show-гейт (детерминированное правило)      │
                    │   └── SuggestRequest ──► mpsc ──┐               │
                    │                                  │              │
                    │ AppEvent::SuggestReady { gen } ◄─┼─ EventLoop   │
                    │   мердж ИИ-строк в попап (FR-021)│  Proxy       │
                    │   карточки С3 (после instantiate)│              │
                    └──────────────────────────────────┼──────────────┘
                                                       ▼
                    ┌─────────────────────────────────────────────────┐
                    │ SuggestWorker (поток; паттерн flow-worker FR-064)│
                    │  generation-счётчик — stale-ответы отбрасываются │
                    │                                                 │
                    │  1. сериализация контекста v0 (порт serialize.py)│
                    │  2. LexEngine v1: BM25+char3+синонимы → top-20   │
                    │  3. [engine=lex+laya] LayaSidecar: POST          │
                    │     /v1/systemone (явная модель, timeout 800 мс) │
                    │  4. fusion: α·lex + (1−α)·mm, α=0.85, min-max   │
                    │  5. Platt-калибровка confidence                  │
                    │  6. топ-3 → SuggestReady                         │
                    │  деградация: нет sidecar → fusion(lex, ∅)=lex    │
                    └───────────────────────┬─────────────────────────┘
                                            │ HTTP localhost
                                            ▼
                    ┌─────────────────────────────────────────────────┐
                    │ sidecar laya-serve (отдельный процесс, CPU)      │
                    │ laya[serve]==0.3.20, head_max_len=512,           │
                    │ один чекпойнт (mm zero-shot → ft после H8)       │
                    │ spawn/health/idle-shutdown — менеджер в S2        │
                    └─────────────────────────────────────────────────┘
```

Принципы (PoC-отчёт §12 «дёшево выпилить — дёшево дорабатывать»):

1. **Весь ИИ — за трейтом `SuggestEngine`** в отдельном крейте; приложение
   знает только `Vec<Suggestion>`. Удаление = minus крейт, feature-флаг и
   конфиг-ключ; ядро не затрагивается ни одной строкой.
2. **Feature-флаг `l1-laya`** (off by default): HTTP-клиент и менеджер
   sidecar не компилируются в дефолтной сборке — ML/сетевых зависимостей
   в дереве по-прежнему нет (правило AGENTS.md «без сети в хосте» соблюдено:
   sidecar — отдельный процесс, хост говорит только с localhost).
3. **Прогрессивный показ** (гипотеза §8.3): попап мгновенно показывает L0-строки,
   ИИ-строки догружаются и вмерживаются при ответе (позже 300–800 мс) — если
    попап ещё открыт и generation совпадает.
4. **Один протокол — три деплоя** (гипотеза §12.4): клиент `/v1/systemone`
    не различает локальный sidecar, будущий fine-tuned чекпойнт и облако
    (opt-in, вне v1) — меняется только адрес/модель в конфиге.

## 2. Крейт `canvas-suggest` (12-й в workspace)

```
crates/canvas-suggest/
├── Cargo.toml            # [features] l1-laya = ["ureq"]; default = []
├── src/
│   ├── lib.rs           # реэкспорт контрактов
│   ├── types.rs         # SuggestRequest/Suggestion/SuggestContext/Verdict
│   ├── context.rs       # сериализация контекста v0 (порт serialize.py)
│   ├── catalog.rs       # каталог шаблонов → опции (key, ru/en имя, глосс, params)
│   ├── lex.rs           # LexEngine v1: BM25Okapi + char3 + словарь синонимов
│   ├── lex_v2.rs        # lex v2: names-синонимы каталога (0.55/0.15) — fallback
│   ├── fusion.rs        # score-fusion α + Platt (a, b из конфига)
│   ├── gate.rs          # show-гейт: правило длины/соседей (0.95 acc)
│   ├── domain.rs        # домен-детектор: Calculation | Framework | Bespoke
│   └── laya/            # #[cfg(feature = "l1-laya")]
│       ├── mod.rs       # LayaSidecarEngine: impl SuggestEngine
│       ├── client.rs    # POST /v1/systemone (ureq no-TLS, timeout, retry ×1)
│       ├── sidecar.rs   # spawn/health-wait/RSS/idle-shutdown (паттерн mcp)
│       └── manifest.rs  # sha256-пины, smoke-фикстура при старте
└── tests/
    ├── fixtures/        # JSONL из PoC: source_a (175), source_b (51),
    │                    # ext-test (75), laya-mm пробы (226, заморожены)
    ├── golden_lex.rs    # p@1 ≥ 0.38 pool / ≥ 0.319 test — CI-регресс
    ├── golden_fusion.rs # fusion ≥ 0.46 на замороженных mm-пробах
    ├── golden_gate.rs   # show-гейт acc ≥ 0.90 (noul-фикстуры)
    └── golden_domain.rs # 33 framework-фикстуры → Verdict::Framework
```

### 2.1. Контракты (эскиз)

```rust
// types.rs
pub struct SuggestRequest {
    pub trigger: SuggestTrigger,
    pub revision: u64,            // ревизия канваса: stale-запросы тонут в worker
}
pub enum SuggestTrigger {
    /// Редактирование текста ноды (C1): префикс строки каретки.
    Editing { prefix: String },
    /// Только что созданная шаблонная нода (C3).
    TemplateInstantiated { template_key: String },
}

pub struct Suggestion {
    pub template_key: Option<String>,  // None = «не предлагать» (none-зона)
    pub score: f32,                    // fused
    pub confidence: f32,               // после Platt (для HUD/логов, НЕ для гейта)
    pub source: SuggestSource,         // Lex | Fusion { alpha }
    pub reason: String,                // «объяснение источника» — доверие (гип. §15)
}

/// Всё, что движок знает о канвасе; собирается на UI-треде за O(N) видимых нод.
pub struct SuggestContext {
    pub canvas_summary: CanvasSummary, // ноды/категории/vars — секция [canvas]
    pub ego: EgoGraph,                 // [up]/[down] сосedi дырки — порт build_context
    pub hole: HoleInfo,                // title/editing — секция [node]
    pub catalog: CatalogSnapshot,      // 62 шаблона → опции (кэш, инвалидация реестра)
}

pub trait SuggestEngine: Send {
    fn name(&self) -> &'static str;
    fn rank(&self, ctx: &SuggestContext, options: &[OptionDesc])
        -> Vec<ScoredOption>;
}
```

`SuggestEngine::rank` — синхронный чистый вызов над контекстом; sidecar-версия
делает то же самое через HTTP (таймаут → пустой скор → fusion вырождается в lex).
Гибрид — не движок, а композиция в worker (формула в `fusion.rs`), поэтому
конфигурируется без кода.

### 2.2. Порты из Python-harness (волна 1, архив)

| Python (архив) | Rust (порт) | Инвариант |
|---|---|---|
| `serialize.py: build_context` | `context.rs` | формат А побайтово (golden-фикстуры контекстов уже в source_a.jsonl — сверка строкой) |
| `serialize.py: parse_node` | `context.rs` | формулы/defines/refs (регекс уже совместим с Numi-движком; у нас есть `expr::line_kind` — используем его вместо регекса) |
| `baseline_lex.py` | `lex.rs` | p@1 ≥ 0.38 pool на фикстурах — CI-гейт |
| `shortlist.py` | `lex.rs` (топ-N) | ≤ 20 опций, golden/none не нужны в продукте |
| `client.py: build_payload/parse_answer` | `laya/client.rs` | протокол: state.document + questions.choice + ЯВНАЯ модель |
| fusion (волна 2, утерян) | `fusion.rs` | α=0.85, min-max; проверка на замороженных mm-пробах ≥ 0.46 |
| lex v2 (волна 3, утерян) | `lex_v2.rs` | names-синонимы 0.55/0.15; автонаб 0.404 test |

BM25 (Okapi) реализуется руками (~80 строк, k1=1.5, b=0.75 — как в
rank_bm25): детерминизм, ноль зависимостей, golden-сверка с фикстурами.

### 2.3. Домен-детектор (C4) — правило, не ML

Волна 3: все три подхода проваливают framework-домены (0.036–0.071) —
сигнала «канвас вне каталога» нет в признаках, и его не создаёт ни промпт,
ни описание none (H4, побитово). Правило до L1 (рекомендация §9.2):

```
Verdict::Calculation   — есть шаблонные ноды ≥1 ИЛИ Numi-присваивания ≥2
                          ИЛИ value-рёбра ≥1  → предлагаем (C1/C3)
Verdict::Framework     — ≥5 info-нод, 0 формул, 0 шаблонов, 0 value-рёбер
                          (CJM/JTBD/Service Blueprint профиль) → НЕ предлагаем
Verdict::Bespoke       — ноды с прозаическим текстом без формул
                          (renovation/onboarding профиль) → НЕ предлагаем
Verdict::Unknown       — пустой/малый канвас (<5 нод) → предлагаем
                          (новый пользователь строит модель — cold start не душим)
```

Точная настройка порогов — на фикстурах framework-схем (33) + 11 эталонных
расчётных схем из `assets/canvas-schemes/`: цель — 0 ложных подавлений на
расчётных, ≥ 90% подавлений на framework. Гейт проверяется в
`golden_domain.rs` на BOTH наборах.

### 2.4. Show-гейт — детерминированное правило

Волна 3 §7: уверенность mm zero-shot не несёт информации о правильности
(Platt a≈0), нейро-гейтинг недопустим до fine-tune. Правило (порт
`baseline_lex.py: noul`, 0.95 acc + noul v0.2 0.90): показывать, если
`len(context) > 120` И есть соседи ([up] или [down] непусты). Порог —
конфиг (`show_gate_min_ctx`). После H8 (fine-tune) — пересмотр по H5-методике
(Platt + margin-гейт, precision ≥ 0.9 при покрытии ≥ 0.5).

### 2.5. Калибровка

`confidence' = sigmoid(a · confidence + b)`, стартовые a=−0.018, b=−0.814
(волна 3, ECE 0.099). Коэффициенты — конфиг; после H8 — re-fit на dev-пробах
нового чекпойнта (скрипт в harness, числа руками в конфиг). Confidence
используется только для HUD/логов/сортировки, НЕ для show-гейта.

## 3. Конфигурация и настройки

```toml
# config.toml (desktop) / localStorage (web — только enabled/engine=lex)
[suggest]
enabled = false               # master-switch; ON не раньше гейта S5
engine = "lex"                # off | lex | lex+laya
alpha = 0.85                  # вес lex в fusion (dev-fit волны 2–3)
max_options = 20              # shortlist для L1 (комфорт Laya ~20)
show_gate_min_ctx = 120       # show-гейт: мин. длина контекста, символов
log_suggest = true            # ~/.canvasdesk/suggest-log.jsonl (opt-out)

[suggest.laya]                # читается только с feature l1-laya
model = "multilingual"        # именованный чекпойнт ИЛИ путь к fine-tuned
endpoint = "http://127.0.0.1:8000"
timeout_ms = 800              # бюджет L1 (прогрессивный показ терпит)
idle_shutdown_s = 600         # sidecar выгружается после простоя
platt_a = -0.018
platt_b = -0.814
# ожидаемые sha256 весов + smoke-фикстура — в laya-manifest.json рядом с весами
```

Деградации (продукт-гейт «не показывай, если не уверен»):
- sidecar не поднялся/таймаут/smoke не прошёл → `engine` фактически = lex,
  `tracing::warn!` один раз, в настройках бейдж «L1 недоступен»;
- web (wasm) → только lex: коротких `cfg` в app нет, выбирает реализация
  трейта (правило архитектуры п.2);
- каталог пуст (нет шаблонов) → движок отвечает пусто, попап без ИИ-строк.

## 4. Интеграция в canvas-app

### 4.1. SuggestWorker (`suggest_worker.rs`, новый)

Паттерн flow-worker FR-064 (`canvas-scene/src/worker.rs`): поток +
`mpsc::Receiver<SuggestRequest>` + `EventLoopProxy<AppEvent>`; в запросе
`revision`, в ответе `generation` — ответ с отставшей ревизией отбрасывается
в app. Паника/зависание воркера не роняет UI: канал просто перестаёт
отвечать, попап остаётся с L0 (правило «фолбэк + warn»).

`AppEvent::SuggestReady { generation, suggestions }` — новое поле enum
(`app.rs:293`), обрабатывается в `handler.rs` рядом с `Search`.

### 4.2. Триггеры

| Триггер | Точка | Поведение |
|---|---|---|
| C1: ввод в ноде | `update_hints()` — уже единый фаннел | debounce 300 мс (между паттернами поиска 200 мс и автосвязи 700 мс); попап L0 не ждёт |
| C3: instantiate | `instantiate_template_at()` (`app.rs:4695`) | асинхронно; карточки у ноды, клик = вставка шаблона рядом (схема-раскладка FR-071 подберёт место) |

### 4.3. UI

> **UX-канон зафиксирован 2026-09-30:** интерактивный флоу C1/C3/C4+
> настроек и телеметрии спроектирован и визуализирован в
> `docs/prototypes/prototype-unified.html` (см. README прототипов и
> DESIGN_RULES §12 — правила выведены из библиотеки книг владельца:
> Doherty 400 мс, закон Хика, Zeigarnik, Podmajersky, Schüll/Cababa,
> Duke). Реализация S3 следует прототипу как источнику истины по
> поведению и состояниям.

- `HintKind::Ai` в `hints_ui.rs`: те же `HintItem { insert, label, detail }`,
  иконка-источник (молния/иконка каталога) + `reason` в detail. Мердж:
  L0-строки сверху, ИИ-строки ниже (источник виден), общий `HINT_LIMIT`.
- Принятие ИИ-строки шаблона = `instantiate_template_at` c заменой
  редактируемой ноды (существующий прецедент палитры) — undo одним шагом.
- Карточки С3: паттерн whatif-бара/тултипа в UiLayer; максимум 3, у ноды,
  закрываются по Esc/клику мимо; пишут события accepted/dismissed.
- Настройки: вкладка «Подсказки» в `settings_ui.rs` — enabled/engine/«загрузить
  L1» (для lex+laya: согласие на догрузку/подключение sidecar ~650 МБ весов).
- i18n: все строки через `i18n::tr` (RU/EN) — как у FR-021.

### 4.4. Метрический слой (S0 — первым)

`~/.canvasdesk/suggest-log.jsonl`, одна строка на событие:

```json
{"ts":"...","rev":42,"event":"shown","source":"fusion","top":"ue-ltv",
 "score":0.71,"conf":0.34,"latency_ms":512,"engine":"lex+laya","domain":"calc"}
```

События: `shown`, `accepted`, `dismissed`, `edited` (текст ноды изменился
после принятия — контроль «атрибуции авторства», гипотеза §15). HUD-панель
латентностей конвейера в dev-режиме (по образцу F3). Журнал локальный,
opt-out в настройках; выгрузка — вне v1. Это же — датасет будущего
fine-tune (data flywheel): accept/dismiss → новые позитивы/негативы.

## 5. Sidecar: жизненный цикл и дистрибуция

### 5.1. Lifecycle (паттерн `canvasdesk mcp`)

- Ленивый spawn при первом L1-запросе (или префетч при старте, если
  `enabled && engine=lex+laya` — конфиг-ключ `prefetch`).
- Health-wait: `GET /health` до готовности чекпойнта (cold start до 60 с —
  тост «загружаю подсказки…», попап работает на lex).
- **Явная модель в каждом запросе** (ловушка авто-роутинга: второй чекпойнт
  в RAM); один чекпойнт на инстанс.
- Smoke при старте: 1 фикстура с известным ответом (A-unit-economics-margin-ru-C3,
  19 опций) — дрейф версии/весов → деградация на lex + warn (блокер по плану
  волны 1 §10).
- Idle-shutdown 600 с; shutdown при выходе приложения (child-process kill).
- RSS-контроль: > 2.5 ГБ (red-порог гейта PoC) → warn в HUD.

### 5.2. Дистрибуция (решение на ревью — варианты)

| Вариант | Что в установщике | Для кого | Когда |
|---|---|---|---|
| **v1 (рекомендую): detect** | ничего; продукт ищет `laya-serve` (PATH/конфиг-путь) и предлагает включить L1, если найден | dev/владелец/ранние пользователи | S2 |
| v1.1: bundle | опциональный компонент MSI: embeddable-Python + `laya[serve]` wheel (~250 МБ) + веса по требованию (~650 МБ) | все desktop | после S5-гейта |
| v2: in-process | ONNX-порт (`ort`) или Rust-порт (laya-candle) — минус sidecar, плюс ML-зависимость в дереве | все | когда экосистема стабилизируется (риск-лист §15) |
| v3: cloud | opt-in облако (Jev/laya.studio), тот же клиент | web/слабое железо | отдельное решение |

v1 оставляет дистрибутив нетронутым и снимает вопрос размера с критического
пути; продуктовое решение о bundle — после приёмки S5.

## 6. Пайплайн fine-tune H8 (S4, офлайн, harness — вне репо)

```
1. Восстановление harness волны 3 (код w2/w3 утерян при откате контейнера,
   только волна 1 в архиве): поверх e6e666e восстановить fusion/lex v2/
   расширенный evalset по специциям отчётов.
   Контроль воспроизводимости: lex 0.319/0.413/0.381, mm бит-в-бит
   (187/187 choice + 39/39 show), fusion 0.468/α=0.85.      [агент, ~0.5 дня]
2. Экспорт train/dev: A-источник (175 golden) + 33 framework-none + авто-B (51);
   test (75) ЗАМОРОЖЕН и не треникуется. Формат JSONL: context, options,
   golden, hard_negatives = топ-10 lex-дистракторов (shortlist их уже
   поставляет).                                              [агент, ~0.25 дня]
3. Ноутбук Kaggle 2×T4: адаптация авторского fine-tune-ноутбука Laya
   (decision-head на замороженном энкодере mm) под наш JSONL-экспорт.
   Ранний стоп по dev; логирование p@1 по эпохам.            [агент, ~0.5 дня]
4. Запуск: Kaggle (владелец или агент), 4–8 ч GPU, бесплатно.
5. Возврат весов: локальный чекпойнт + manifest (sha256, laya-версия,
   head_max_len) + Platt re-fit на dev-пробах нового чекпойнта
   (числа → конфиг suggest.laya.platt_*).                     [агент, ~0.25 дня]
6. Перегейт по H1-протоколу на замороженном test (75):
   цель p@1 ≥ 0.60 (green) / 0.50–0.60 (yellow: гибрид ft-mm+lex).
   Отчёт в dev-researches; при green — модель по умолчанию
   suggest.laya.model = путь к ft-чекпойнту.                  [агент, ~0.5 дня]
```

Референс авторов: 0.362 → 0.766 (их домен); наш вход ниже (0.309 пул
zero-shot), поэтому green-цель 0.60 консервативна. Риск переобучения на 259
строках лечится: замороженный энкодер (только головка), hard negatives,
ранний стоп, контроль на замороженном test. При недостижении — остаётся
zero-shot-конфиг волны 3 (≈0.47 на расчётных доменах), решение владельца.

## 7. Тест-стратегия (CI без sidecar, без GPU, без сети)

| Уровень | Что | Гейт |
|---|---|---|
| golden_lex | 226 фикстур (source_a/b), фиксированный каталог | p@1 ≥ 0.38 pool, ≥ 0.319 test |
| golden_fusion | замороженные mm-пробы из архива (`report/runs/laya-mm.json`, probs по 226 фикс.) + lex-скоринг → fusion | ≥ 0.46 test-old; α-чувствительность ±0.05 не ниже 0.44 |
| golden_gate | noul-фикстуры (show/don't-show) + правило | acc ≥ 0.90 |
| golden_domain | 33 framework-фикстуры + 11 расчётных схем | framework ≥ 90% подавлено; расчётные 0 ложных подавлений |
| контракты | сериализация контекста: строки source_a.jsonl побайтово | 0 расхождений |
| интеграция | worker: stale-generation, таймаут sidecar → деградация lex, паника воркера → попап жив | юнит-тесты с mock-движком |
| UI L2 | `wasm_ui_test.sh`: попап с ИИ-строками (lex-режим), карточки С3 | пиксельный дифф по рецепту WASM-TESTING §3 |
| wasm-гейт | `cargo check -p canvas-suggest --target wasm32-unknown-unknown` (без feature) | чисто |

Ключевая идея: **весь гибрид детерминированно тестируем офлайн** по
замороженным пробам волны 1 — sidecar в CI не нужен. Фикстуры (~2 МБ JSONL)
кладутся в `tests/fixtures/` с указанием происхождения (архив, коммит,
sha256) — как golden-паттерн `scheme_cjm_tests.rs`.

## 8. Порядок работ и оценки (агент-дни)

| Стадия | Содержание | Оценка | Коммит(ы) |
|---|---|---|---|
| S0 | метрический слой + HUD | 0.5–1 | feat(suggest): S0 |
| S1 | canvas-suggest чистое ядро + golden-гейты | 2–3 | feat(suggest): S1 |
| S2 | l1-laya: клиент + lifecycle + smoke | 2–3 | feat(suggest): S2 |
| S3 | worker + UI + настройки + i18n + L2 | 3–5 | feat(suggest): S3 |
| S4 | H8: harness → экспорт → ноутбук → перегейт | 2 + 4–8 ч Kaggle wall-clock | docs + harness (вне репо) |
| S5 | дефолты + user-docs + онбординг + приёмка | 1 | feat(suggest): S5 + docs |
| **итого** | | **~9–14 агент-дней** | |

S4 стартует после S1 (нужен canvas-suggest для перегейта на Rust-реализации —
опционально; перегейт можно вести и Python-harness, Rust-гейты подключим
как cross-check). Параллелизм: S4 не блокирует S2–S3.

## 9. Риски и открытые вопросы

| Риск | Митигация |
|---|---|
| fine-tune не даёт 0.60 (мало данных: 259 строк) | zero-shot-конфиг ≈ 0.47 остаётся продуктом; data flywheel (S0-лог) копит реальные пары; повторный H8 на большем сете |
| ru-качество mm (0.269 vs en 0.524) не чинится fine-tune | H2 закрыт отрицательно (промпт не лечит); в train-экспорт ru-фикстуры входят как есть — код-свитч-данные по литературе лечатся обучением; контроль ru-слайса в перегейте |
| латентность CPU p95 > 1 с на слабом железе | прогрессивный показ (попап не ждёт); T3.json one-shot с ноутбука владельца (пакет готов в архиве) закроет матрицу |
| дрейф laya 0.3.x (проекту 10 дней) | пин версии + sha256 + smoke-фикстура; свап модели (H10) — вне критического пути |
| вес sidecar 650 МБ / RAM 1.8 ГБ | detect-режим v1 (дистрибутив не растёт); idle-shutdown; осознанный opt-in |
| навязчивость (продуктовый риск №1 гипотезы) | on-demand по гейту show, не чаще N/мин (конфиг), полная отключаемость, reason у каждой подсказки |

**Вопросы владельцу** (помимо FR §«решения»): (1) порядок S4 vs S2–S3 —
параллельно или fine-tune первым? (2) запуск Kaggle-ноутбука — сам или дать
инструкцию? (3) включать ли T3-one-shot в S2-гейт как обязательное условие?

## 10. Чек-лист ревью этого плана

- [ ] Скоуп v1 (C1/C3/C4-гейт; C2/C5 вне) — верно?
- [ ] Крейт `canvas-suggest` + feature `l1-laya` + конфиг-ключи — приемлемо?
- [ ] Дефолт OFF до S5, opt-in после S3 — верно?
- [ ] Дистрибуция v1 detect (без bundle) — приемлемо?
- [ ] Оценки и порядок стадий — реалистичны?
- [ ] Гейты (golden-пороги, acceptance ≥ 25%, p50 < 500 мс CPU) — верные?
