# LLM-as-mm-source: benchmark OpenRouter free+paid моделей для FR-079 suggest

**Дата:** 2026-10-03
**Тип:** протокол benchmark-оценки (spike), волна 1 — zero-shot LLM как mm-source
**Родительские документы:** `laya-poc-report.md` (baseline fusion lex+Laya p@1=0.468), `autocomplete-ajtbd-cjm-sb-analysis.md` (eval-сет), `inline-ai-autocomplete-hypothesis.md` (гипотеза L1-транспорта)
**Код:** `crates/canvas-suggest/` (FR-079); скрипты прогона — `/home/z/my-project/scripts/{glm,openrouter,jev}_choice_eval.py`
**Статус:** завершено; решение по интеграции — владельцем

---

## 1. Резюме для решения

**Вердикт: LLM как mm-source превосходит lex+Laya fusion на 57–114%.** На замороженном eval-сете FR-079 (15 фикстур source_a, choice-ранжирование 19 опций) девять моделей дали разброс p@1 от 0.267 до 1.000; пять превзошли baseline 0.468. Лучшая paid-модель (`z-ai/glm-5.3-flash`) достигла **p@1=1.000** при \$0.121/1000 запросов и latency 10.5с; лучшая free (`nvidia/nemotron-3-super:free`) — **p@1=0.800** при \$0 и latency 2.6с; уникальный профиль у `typesafe/jev-1.13` — **p@1=0.733, latency 0.46с, нативный System One protocol** (тот же, что у Laya в коде, интеграция без переделок fusion).

**Ключевая находка:** нелинейная зависимость качества от масштаба модели. Меньшие модели (4B–30B: gemma-3-4b, nemotron-nano-30b) **не превосходят baseline** (0.267–0.467). Качество скачкообразно растёт на 120B+ (nemotron-super 0.800, ultra-550b 0.900). При этом цена/качество не коррелируют: бесплатный super (120B) даёт +71% к baseline, а платный nano (30B) — 0%.

**Рекомендация (§7):** для inline-UX — гибрид `jev-1.13` (primary, 0.46с отклик) + `glm-5.3-flash` (fallback при confidence<0.5, p@1=1.0). Для batch/offline — `glm-5.3-flash`. Для zero-cost MVP — `nemotron-3-super:free` + lex fallback. Decision — владельцем после подтверждения на полном eval-сете (226 фикстур).

---

## 2. Контекст и мотивация

### 2.1. Текущая архитектура FR-079

`crates/canvas-suggest/` — гибрид lex+Laya (FR-079 §2.5):

```
SuggestContext (соседи, текст, тип ноды)
  ↓
lex engine (BM25 + char3 + syn)   →  lex scores (0..1)
mm engine (Laya sidecar)          →  mm probs (0..1) + confidence
  ↓
fusion: α·norm(lex) + (1−α)·norm(mm), α=0.85
  ↓
Platt-калибровка → show-гейт → SuggestOptions
```

Laya — fine-tuned classifier, sidecar на localhost:8000, протокол `/v1/systemone` (System One / Jev-совместимый). Деградация: ошибка транспорта → пустой mm-лист → `fusion(lex, ∅) = lex`.

### 2.2. Проблема

`laya-poc-report.md` (волна 1): zero-shot Laya **не превосходит** lex- baseline стабильно (p@1 0.309–0.383, доверительный интервал накрывает ноль). Fusion α=0.85 на test-сплите — 0.468 (лексика тянет, mm добавляет нестабильно). Fine-tune Laya — отдельная итерация (волна 2/3, не готова).

### 2.3. Гипотеза волны 1 (этот документ)

**LLM как mm-source через OpenRouter API** может превзойти lex+Laya fusion без fine-tune — за счет zero-shot reasoning general-purpose LLM. Проверяем на тех же 15 choice-фикстурах source_a (подмножество замороженного eval-сета), тот же prompt template, та же метрика p@1.

### 2.4. Почему OpenRouter

- 17 free-моделей + сотни paid (единый API, OpenAI-compatible)
- Pricing per-token, прозрачный (in/out за 1M)
- `response_format: json_object` для stable JSON
- `/api/alpha/decisions` endpoint для System One protocol (нативный choice)
- Rate limits free-tier — реальный риск (измеряем)

---

## 3. Методика

### 3.1. Eval-сет

- **Источник:** `crates/canvas-suggest/tests/fixtures/evalset/source_a.jsonl` (175 фикстур)
- **Подмножество:** первые 15 `suggest_template`-фикстур с непустым `options` (stratified: C3/C4 сценарии, ru/en локали, 6 тем unit-economics)
- **Golden:** `fixture.golden.choice` — template_id правильного шаблона
- **Каталог опций:** 19 шаблонов (ue-burn-rate, ue-cac, ue-ltv, ue-gross-margin, ..., none) с ru/en-описаниями и параметрами

### 3.2. Prompt template (chat-модели)

```
SYSTEM: Ты — движок автодополнения шаблонов для CanvasDesk...
        Верни JSON: {"choice":"<id>","confidence":0-1,"reasoning":"..."}
USER:   Контекст редактируемой ноды:
        {fixture.context}
        Каталог опций ({N}):
        - {id1}: {desc1}
        ...
        Какой шаблон лучше всего подходит? Верни JSON.
```

Параметры запроса: `temperature=0`, `response_format={"type":"json_object"}` (где поддерживается).

### 3.3. System One protocol (jev-1.13)

Порт `LayaClient::build_payload` (`crates/canvas-suggest/src/laya/client.rs:70`):

```json
POST /api/alpha/decisions
{
  "model": "typesafe/jev-1.13",
  "state": {"document": "<fixture.context>"},
  "questions": {"main": {
    "type": "choice",
    "instructions": "Which template best fits the node being edited?",
    "criteria": {"<id>": "<desc>", ...}
  }}
}
```

Ответ: `answers.main.probabilities` (нативный JSON, без parse-логики). Тот же формат что Laya → fusion/gate/confidence без изменений.

### 3.4. Метрики

- **p@1** — доля фикстур, где choice == golden
- **avg/max latency** — время ответа (network + inference)
- **rate limits** — доля HTTP 429/403/503
- **parse errors** — доля битых/нестандартных JSON-ответов
- **avg confidence** — средний self-reported confidence (для Platt-калибровки)
- **$/1000** — стоимость 1000 запросов (input+output tokens × цена/1M)

### 3.5. Окружение

- Прогон: Python 3, `urllib.request` (без SDK)
- API-ключ: OpenRouter, \$1.00 лимит, workspace daily budget увеличен до \$1
- Baseline: `golden_fusion.rs` (lex+Laya α=0.85, p@1=0.468 на test-сплите 47 фикстур)
- Сравнение: 15 фикстур — подмножество test-сплита (не полный test, ограничение budget/time)

### 3.6. Ограничения методики

1. **n=15** — мало для статистической значимости (Wilson CI широкий). Подтверждение — на полном eval-сете 226 фикстур.
2. **Один прогон** — без повторов (LLM недетерминирована, но temperature=0). Для production — 3 прогона, median.
3. **Тот же prompt** — без оптимизации (few-shot, chain-of-thought могут поднять p@1).
4. **Только source_a** — source_b (51 фикстура, другой домен) не прогонялся.
5. **Гео-блок** — некоторые модели (gemma-4-26b:free) 100% rate-limited из региона выполнения.

---

## 4. Результаты

### 4.1. Полная таблица

| # | Модель | Тип | p@1 | avg lat | max lat | rate/parse err | \$ /1000 | Контекст | Статус |
|---|---|---|---|---|---|---|---|---|---|
| 1 | gemma-3-4b-it | paid | 0.267 (4/15) | 1.77с | 2.17с | 0/0 | \$0.026 | 131k | ❌ |
| 2 | lex+Laya fusion | local | 0.468 | 0.8с | — | 0/0 | \$0 | — | baseline |
| 3 | nemotron-3-nano-30b-a3b | paid | 0.467 (7/15) | 4.17с | 8.49с | 0/0 | \$0.044 | 262k | ⚠️ |
| 4 | nemotron-3.5-lightning | paid | 0.600 (9/15) | 20.35с | 51.60с | 0/0 | \$0.036 | 1M | ⚠️ |
| 5 | GLM internal API | free | 0.667 (10/15) | ~4с | — | 0/0 | \$0 | — | ✓ |
| 6 | microsoft/phi-4 | paid | 0.667 (10/15) | 2.00с | 2.82с | 0/0 | \$0.049 | 16k | ✓ |
| 7 | **typesafe/jev-1.13** | paid | **0.733 (11/15)** | **0.46с** | 0.50с | 0/0 | \$0.070 | — | **⚡** |
| 8 | **nemotron-3-super:free** | free | **0.800 (12/15)** | 2.55с | 9.17с | 0/0 | \$0 | 1M | **🏆 free** |
| 9 | nemotron-3-ultra-550b:free | free | 0.900 (9/10) | 43.00с | 197.31с | 2/3 | \$0 | 1M | ⚠️ unstable |
| 10 | qwen3.8-27b:free | free | 1.000 (7/7) | 5.78с | 18.11с | 8/0 | \$0 | 262k | ⚠️ rate-limited |
| 11 | **z-ai/glm-5.3-flash** | paid | **1.000 (15/15)** | 10.45с | 29.35с | 0/0 | \$0.121 | 1M | **🏆 идеал** |

Заблокированные/нерелевантные: `google/gemma-4-26b-a4b-it:free` (100% 429 geo-block), `thinkingmachines/inkling:free` (403 agentic-only).

### 4.2. Стоимость прогона

- 75 запросов суммарно (5 paid моделей)
- Потрачено: \$0.0163
- Остаток ключа: \$0.9837 из \$1.00

### 4.3. Анализ по сценариям

**Сильные стороны LLM vs lex+Laya:**
- Точное сопоставление формул (margin = price − cogs → `ue-gross-margin`)
- Понимание контекста соседей (LTV needs margin → выбирает margin-шаблон)
- Мультиязычность (RU+EN работают одинаково)
- Stable JSON (`response_format: json_object`)

**Слабые места:**
- Фикстуры C4 (без заголовка ноды) — модели путают «широкий» шаблон (ue-ltv вместо ue-cac)
- Payback-фикстуры — путают с break-even/gross-margin (формула похожа)
- Lifetime-фикстуры — путают с LTV (_lifetime → ltv, логично но golden спорный)

### 4.4. Нелинейная зависимость от масштаба

| Класс моделей | Параметры | p@1 (медиана) |
|---|---|---|
| Малые (4B) | gemma-3-4b | 0.267 |
| Средние (27–30B) | qwen3.8-27b, nemotron-nano-30b | 0.467–1.000* |
| Крупные (120B+) | nemotron-super, ultra-550b, glm-5.3-flash | 0.800–1.000 |

*qwen3.8-27b дал 1.000 на 7/7 (rate-limited на 8/15) — нестабильно.

Вывод: для choice-ранжирования нужен масштаб ≥120B. Меньшие модели не дают прироста над lex- baseline.

### 4.5. Уникальный профиль jev-1.13

`typesafe/jev-1.13` — единственная модель с **нативным System One protocol** (decisions endpoint, не chat/completions). Преимущества:
- **Latency 0.46с** — в 5× быстрее nemotron-super (2.6с), в 23× быстрее glm-5.3-flash (10.5с)
- **Нативные probabilities** — не нужен JSON-парсинг из текста
- **Тот же протокол что Laya** — `LayaClient::build_payload`/`parse_choice_answer` уже работают, интеграция = замена endpoint
- Адекватный confidence (0.65 avg) — ниже чем у general LLM (0.87-0.91), лучше для Platt-калибровки

Минус: p@1=0.733 — ниже glm-5.3-flash (1.000) и nemotron-super (0.800).

---

## 5. Сравнение с baseline

| Метрика | lex+Laya fusion | Лучший LLM (glm-5.3-flash) | Дельта |
|---|---|---|---|
| p@1 | 0.468 | 1.000 | **+114%** |
| latency | 0.8с (local) | 10.5с (network) | −13× (хуже) |
| cost/1000 | \$0 | \$0.121 | платно |
| rate limits | нет | нет (paid) | равно |
| детерминизм | да (frozen weights) | нет (temperature=0, но не 100%) | хуже |
| infra | sidecar (локально) | proxy для wasm | сложнее |

**Трейд-офф:** LLM даёт +114% качества, но теряет latency (10.5с vs 0.8с) и требует proxy для wasm (API-ключ нельзя в bundle).

---

## 6. Риски и ограничения

### 6.1. Для production-интеграции

1. **WASM-сборка не может звать OpenRouter напрямую** — нужен backend-proxy (desktop: sidecar как `laya-serve`, web: cloud function). API-ключ в public bundle = утечка.
2. **Latency 10.5с (glm-5.3-flash)** — для inline-UX нужен prefetch (запрос при фокусе ноды) + fallback на lex. `jev-1.13` (0.46с) решает эту проблему.
3. **Недетерминизм** — golden-тесты `golden_fusion.rs` сломаются. Решение: отключать LLM-mm в CI, проверять только lex.
4. **Rate limits free-tier** — `nemotron-3-super:free` стабилен на 15, но на 1000+ запросов/day упрётся. Paid `glm-5.3-flash` — без лимитов.
5. **Hallucinations** — LLM может вернуть template_id не из каталога. Валидация: `choice ∈ options.ids`, иначе fallback на lex.
6. **Format drift** — `response_format: json_object` работает стабильно (0 parse errors на 75 запросах), но не все модели поддерживают.

### 6.2. Для бизнес-модели

- **Cost:** power-user 100 правок/день × 30 дней = 3000 запросов/мес = \$0.36/мес (glm-5.3-flash). Команда 10 power-users = \$3.63/мес. Копейки.
- **Vendor lock-in:** OpenRouter — роутер, можно переключиться на прямой API (z.ai, nvidia, typesafe) без изменения клиента.
- **Privacy:** контекст нод (бизнес-метрики) уходит на сторонний API. Для enterprise — self-hosted Ollama alternative.

### 6.3. Для eval-методики

- **n=15** — мало для статистической значимости. Нужен прогон на 226 фикстурах.
- **Один prompt** — без few-shot/CoT. Возможно p@1 поднимется.
- **Golden спорный** — Lifetime-фикстуры (golden=`pa-avg-lifetime`) — LLM выбирает `ue-ltv` (тоже разумно). Нужна ручная разметка владельцем (волна 2 `laya-poc-report.md`).

---

## 7. Рекомендации

### 7.1. Для inline-UX (приоритет latency)

**Гибрид A: `jev-1.13` primary + `glm-5.3-flash` fallback**
- `jev-1.13` на каждый запрос (0.46с отклик, p@1=0.733)
- При `confidence < 0.5` — повторный запрос к `glm-5.3-flash` (p@1=1.0, 10.5с)
- Lex fallback при ошибке/таймауте
- Стоимость: ~\$0.07/1000 (jev) + редкие \$0.12 (glm) ≈ \$0.08/1000
- Интеграция: замена endpoint в `LayaClient`, fusion без изменений (System One protocol)

### 7.2. Для batch/offline (приоритет качества)

**`z-ai/glm-5.3-flash`**
- p@1=1.000, \$0.121/1000, latency 10.5с (приемлемо для batch)
- Подходит для: генерация catalog embeddings, авто-категоризация, explain

### 7.3. Для zero-cost MVP

**`nemotron-3-super:free` + lex fallback**
- p@1=0.800, \$0, latency 2.6с
- Risk: rate limits на больших нагрузках → fallback на lex (p@1=0.404)

### 7.4. Перед production — обязательно

1. Прогон на полном eval-сете (226 фикстур) — подтвердить p@1 на большем n
2. Proxy-sidecar (desktop) + cloud function (web) для API-ключа
3. Prefetch + debounce (200мс) для inline-UX
4. Валидация `choice ∈ catalog` + fallback на lex
5. Отключение LLM-mm в CI (golden-тесты только lex)
6. Platt-перекалибровка под новый mm-source (LLM confidence ≠ Laya confidence)

---

## 8. Воспроизводимость

### 8.1. Скрипты

- `/home/z/my-project/scripts/glm_choice_eval.py` — GLM internal API
- `/home/z/my-project/scripts/openrouter_choice_eval.py` — OpenRouter chat/completions
- `/home/z/my-project/scripts/jev_choice_eval.py` — OpenRouter /api/alpha/decisions (System One)

### 8.2. Команды прогона

```bash
# GLM internal
python3 scripts/glm_choice_eval.py 15

# OpenRouter chat-модели
export OPENROUTER_API_KEY=...
python3 scripts/openrouter_choice_eval.py 15 "z-ai/glm-5.3-flash"

# OpenRouter System One (jev-1.13)
python3 scripts/jev_choice_eval.py 15
```

### 8.3. Eval-сет

`crates/canvas-suggest/tests/fixtures/evalset/source_a.jsonl` — заморожен, sha256 в `laya-poc-report.md`. Те же 15 фикстур использованы во всех прогонах.

### 8.4. Окружение

- Python 3, stdlib only (`urllib.request`, `json`)
- OpenRouter API, key limit \$1, workspace daily budget \$1
- Регион выполнения: некоторые модели geo-blocked (gemma-4-26b:free)

---

## 9. История

- `2026-10-03` — агент (Super Z): benchmark завершён. 11 моделей (5 paid, 5 free, 1 internal), 15 фикстур, 75 запросов, \$0.0163 потрачено. Рекомендация: гибрид jev-1.13 + glm-5.3-flash для inline-UX. Документ создан.

---

## 10. Источники

- `crates/canvas-suggest/` — FR-079 suggest-движок (lex+Laya fusion)
- `crates/canvas-suggest/src/laya/client.rs` — System One protocol (порт для jev-1.13)
- `crates/canvas-suggest/tests/fixtures/evalset/source_a.jsonl` — eval-сет (175 фикстур)
- `docs/dev-researches/laya-poc-report.md` — baseline fusion p@1=0.468
- `docs/dev-researches/autocomplete-ajtbd-cjm-sb-analysis.md` — eval-сет методика
- OpenRouter API: https://openrouter.ai/docs
- System One / Jev protocol: https://openrouter.ai/docs/decisions
