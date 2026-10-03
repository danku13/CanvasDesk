# LLM-интеграция: план реализации в 4 параллельных потока

**Дата:** 2026-10-03
**Тип:** план реализации (parallel streams)
**Родительские документы:** `adr-0016-llm-layer-canvas-llm.md`, `prd-0010-llm-integration-byok-chatgpt.md`, `byok-chatgpt-oauth-design.md`
**Цель:** разделить реализацию LLM-интеграции на 4 независимых потока для параллельного исполнения разными сессиями

---

## Принцип разделения

4 потока разделены по **файлам и крейтам**, чтобы исключить merge-конфликты. Каждый поток:
- Имеет чёткий набор файлов (owned files)
- Не трогает файлы других потоков
- Зависимости между потоками — только через trait-определения (которые зафиксированы в ADR-0016)
- Может начинаться параллельно (этап 1 — общий trait, но каждый поток реализует свою часть)

---

## Поток A: canvas-llm crate (LLM-ядро)

**Владелец файлов:** `crates/canvas-llm/` (новый крейт, все файлы)
**Не трогает:** canvas-app, canvas-suggest, canvas-web, settings_ui.rs
**Зависимости:** ADR-0016 (trait определение зафиксировано)

### Что реализует

1. **`LlmProvider` trait** (все 4 операции: chat, choice, tool_calling, embed) + `ProviderCaps`, `ModelInfo`, `Message`, `ToolDef`, `ToolCall`, `ChoiceAnswer`, `ChatOpts`, `ToolCallingOpts`
2. **`OpenAiCompatibleProvider`** — один адаптер для 5 endpoint'ов (OpenAI, OpenRouter, z.ai, Moonshot, Ollama). `base_url` + `api_key` параметризуются.
3. **`AnthropicClaudeProvider`** — отдельный адаптер (`/v1/messages`, `x-api-key`, `tool_use` blocks)
4. **`Redact engine`** (Q1) — `redact_context(context: &str, mode: PrivacyMode) -> String`. Формат: `price=<redacted> руб`. PrivacyMode: `Off` (local/self-hosted) | `Redact` (cloud)
5. **`Cost-estimation engine`** (Q4) — `estimate_cost(model, input_tokens, max_output) -> CostEstimate`. `actual_cost(usage, pricing) -> f64`
6. **`Compliance flag`** (Q1+Q5) — `DataResidency: Local | Cloud | SelfHosted`. Влияет на redact + доступные провайдеры
7. **Self-hosted endpoint** (Q5) — любой URL + key, ответственность пользователя
8. **Health-check** — `health()` для каждого провайдера (валидация ключа)
9. **Settings struct** (новые поля): `llm_provider_suggest`, `llm_provider_graph`, `llm_provider_agent` (per-feature, Q2), `llm_api_key`, `llm_model`, `llm_endpoint`, `data_residency`, `llm_cost_limit_daily` (Q7, default $1.0), `llm_confidence_threshold` (Q7, default 0.5), `llm_telemetry_opt_in` (Q7, default false)

### Файлы

```
crates/canvas-llm/
├── Cargo.toml
├── src/
│   ├── lib.rs              # LlmProvider trait, публичные типы
│   ├── provider.rs         # trait + ProviderCaps + ModelInfo
│   ├── types.rs            # Message, ToolDef, ToolCall, ChoiceAnswer, ChatOpts
│   ├── error.rs            # LlmError
│   ├── openai_compat.rs    # OpenAiCompatibleProvider (5 endpoint'ов)
│   ├── anthropic.rs        # AnthropicClaudeProvider
│   ├── redact.rs           # Redact engine (Q1)
│   ├── cost.rs             # Cost-estimation engine (Q4)
│   ├── compliance.rs       # DataResidency, PrivacyMode (Q1+Q5)
│   ├── health.rs           # Health-check
│   └── settings.rs         # LlmSettings struct (новые поля Settings)
└── tests/
    ├── redact_tests.rs
    ├── cost_tests.rs
    └── provider_tests.rs
```

### Оценка: 3-4 сессии

### Выходной артефакт для других потоков

- `LlmProvider` trait (зафиксирован в ADR-0016, не меняется)
- `LlmSettings` struct (поток B использует для UI)
- `CostEstimate` struct (поток B использует для статусной панели)
- `redact_context()` функция (поток C использует для suggest)

---

## Поток B: Settings "AI and Models" + Статусная панель + Onboarding

**Владелец файлов:**
- `crates/canvas-app/src/settings_ui.rs` (новый 9-й таб "AI and Models")
- `crates/canvas-app/src/app/ai_status_panel.rs` (новый модуль — статусная панель)
- `crates/canvas-app/src/onboarding_ui.rs` (новый экран AI-режима)
- `crates/canvas-app/src/app/overlays.rs` (только добавление вызова статусной панели, не трогать существующие overlay)
- `sdk/web-onboarding/src/scenarios/ai-mode.ts` (новый сценарий онбординга)
- `user-docs/ai-features.md` (новая user-документация)

**Не трогает:** canvas-llm crate, canvas-suggest, canvas-mcp, handler.rs (кроме точки вызова статусной панели)
**Зависимости:** `LlmSettings` от потока A (struct определён, можно stub до реализации)

### Что реализует

1. **Settings таб "AI and Models"** (9-й таб, Q2):
   - Per-feature выбор провайдера: Suggest / Graph Builder / Agent Panel — каждый со своим dropdown
   - Suggest: [BYOK ▼] / [Ollama] / [Laya] / [Off] — ChatGPT OAuth **недоступен** для suggest
   - Graph Builder: [ChatGPT OAuth ▼] / [BYOK] / [Ollama] / [Off]
   - Agent Panel: [ChatGPT OAuth ▼] / [BYOK] / [Ollama] / [Off]
   - API key input (сохраняется в keychain, НЕ в config.toml)
   - Model selection (dropdown из `provider.models()`)
   - Health-check button ("Проверить ключ")
   - **Rate limit counter** (Q2): "ChatGPT: 47/80 сообщений осталось (обновится через 2ч 15м)" — local counting
   - **Cost limit** (Q7): "Дневной лимит: $1.00" + slider
   - **Confidence threshold** (Q7): "Показывать подсказки если confidence ≥ [0.5]" + slider
   - **Telemetry opt-in** (Q7): "Отправлять предложенные ноды для улучшения каталога" (default OFF)
   - **Data residency** (Q5): "Режим данных: Local / Cloud / Self-hosted" + описание что значит каждый
   - **Self-hosted endpoint** (Q5): URL + API key field + описание "ответственность пользователя"

2. **Статусная панель** (Q4, справа над миникартой):
   ```
   ┌─────────────────────────┐
   │ AI: glm-5.3-flash (BYOK)│
   │ Suggest ✓  Graph ✓      │
   │ Agent ✓                 │
   │ Session: $0.04          │
   │ Day: $0.12 / $1.00      │
   │ [⚙] [⏸]                 │
   └─────────────────────────┘
   ```
   - Активная модель + провайдер (per-feature)
   - Включённые функции (toggle кликом)
   - Cost за сессию (накопительный)
   - Cost за день / дневной лимит (Q7)
   - ⚙ → открыть "AI and Models" таб
   - ⏸ → pause all AI (emergency stop)
   - При 80% дневного лимита (Q7): dialog "Достигнут 80% лимита ($0.80/$1.00). Расширить? Изменить лимит?"
   - **На mobile/узких окнах — прятать** (Q4)

3. **Onboarding экран AI-режима** (Q5, новый экран после language/role):
   ```
   Выберите режим AI:
   ○ Local only (Laya/Ollama) — работает offline, данные не уходят
   ○ Cloud (ChatGPT/BYOK) — лучше качество, данные уходят на {провайдер}
   ○ Self-hosted (ваш GPU сервер) — cloud-качество, данные в вашем контуре
   
   [Подробнее о privacy] [Продолжить]
   ```
   - Ссылка на user-docs/ai-features.md
   - Сохранение выбора в `data_residency` setting

4. **User documentation** (Q5):
   - `user-docs/ai-features.md` — полный гайд: что делают AI-функции, какие данные отправляются, privacy-режимы, self-hosted настройка, cost
   - Онбординг-тур сценарий `sdk/web-onboarding/src/scenarios/ai-mode.ts`

### Файлы

```
crates/canvas-app/src/
├── settings_ui.rs              # +9-й таб "AI and Models"
├── app/
│   ├── ai_status_panel.rs      # НОВЫЙ — статусная панель
│   ├── overlays.rs             # +вызов ai_status_panel (минимальная правка)
│   └── handler.rs              # +точка вызова (1 строка)
├── onboarding_ui.rs            # +экран AI-режима
sdk/web-onboarding/src/scenarios/
└── ai-mode.ts                  # НОВЫЙ — онбординг-тур
user-docs/
└── ai-features.md              # НОВЫЙ — user documentation
```

### Оценка: 3-4 сессии

### Выходной артефакт для других потоков

- `ai_status_panel()` функция (поток C, D вызывают для отображения)
- `LlmSettings` UI (поток A читает настройки)
- Onboarding сохраняет `data_residency` (поток A читает)

---

## Поток C: Suggest с LLM mm-source + Custom-node suggest

**Владелец файлов:**
- `crates/canvas-suggest/src/llm/` (новый модуль, feature `l1-llm`)
- `crates/canvas-suggest/src/lib.rs` (feature gate, минимальная правка)
- `crates/canvas-suggest/Cargo.toml` (feature `l1-llm`)
- `crates/canvas-app/src/app/suggest.rs` (новый модуль — suggest UI с custom-нодами)
- `crates/canvas-app/src/app/overlays.rs` (только `suggest_cards_overlay` — расширение, не трогать другие)

**Не трогает:** canvas-llm crate, settings_ui.rs, canvas-mcp, handler.rs (кроме точки вызова suggest)
**Зависимости:** `LlmProvider` trait от потока A, `redact_context()` от потока A

### Что реализует

1. **`LlmMmSource`** (feature `l1-llm`, параллельно `l1-laya`):
   - Реализует `SuggestEngine` (тот же интерфейс что Laya)
   - `choice()` через `LlmProvider::choice()` — prompt template из benchmark
   - **Redact** (Q1): контекст ноды проходит через `redact_context()` перед отправкой
   - Fusion: `α·lex + (1−α)·llm`, α-пересчёт (LLM точнее → α ~0.6)
   - Fallback: ошибка/таймаут → `fusion(lex, ∅) = lex`
   - LLM-mm отключена в CI (golden-тесты только lex)

2. **Prefetch + cache** (Q2):
   - Запрос при фокусе ноды (до правки)
   - Кэш по хешу контекста (TTL 5 мин)
   - Cache hit → мгновенно; cache miss → запрос

3. **Loading-state skeleton** (Q7):
   - Skeleton ghost-node (полупрозрачный квад с spinner) пока LLM думает
   - → morph в реальный ghost-node

4. **Confidence-threshold** (Q7):
   - Settings: "Показывать подсказки если confidence ≥ [0.5]"
   - Если confidence < threshold → скрыть подсказку

5. **Custom-node suggest** (Q7, НОВОЕ):
   - LLM генерирует **3 варианта** кастомных нод (title + formula + params)
   - Промпт: передаём доступные формулы/операторы (из `expr.rs`) + право создать кастомную
   - **3 ghost-nodes рядом** (Q7)
   - **expr_eval валидация** (Q7): каждый вариант проверяется, битые — скрываются
   - Cost protection: custom-suggest только если (a) template confidence < threshold И (b) cost < limit
   - Cache по хешу контекста

6. **Cost limit per request** (Q7):
   - Перед запросом: `estimate_cost()` → если > user limit → skip, fallback на lex
   - После запроса: `actual_cost()` → в статусную панель (поток B)

7. **Benchmark redacted vs raw** (R10):
   - Прогон на eval-сете с redacted контекстом
   - Сравнение p@1 с raw (0.800-1.000)
   - Если падение >10% — fallback на consent-dialog

8. **"Сохранить как шаблон"** (Q7):
   - После применения custom-ноды → toast "Сохранить как шаблон? [Да] [Нет]"
   - "Да" → GitHub issue (тип enhancement) к canvasdesk repo (через `gh` CLI или web-link)
   - НЕ telemetry backend (отложено)

### Файлы

```
crates/canvas-suggest/src/
├── llm/                       # НОВЫЙ модуль
│   ├── mod.rs
│   ├── mm_source.rs           # LlmMmSource (choice через LlmProvider)
│   ├── custom_suggest.rs      # 3 custom-варианта
│   ├── cache.rs               # Prefetch + cache (Q2)
│   └── benchmark_redact.rs    # Benchmark redacted vs raw
├── lib.rs                     # +feature l1-llm
└── Cargo.toml                 # +feature l1-llm

crates/canvas-app/src/
├── app/
│   ├── suggest.rs             # НОВЫЙ — suggest UI (skeleton, 3 ghost-nodes, save-as-template)
│   └── overlays.rs            # +suggest_cards_overlay расширение
```

### Оценка: 3-4 сессии

### Выходной артефакт для других потоков

- `LlmMmSource` готов (поток D не зависит, но может использовать для agent tool-calling)
- Suggest UI с custom-нодами (поток B показывает в статусной панели что suggest активен)

---

## Поток D: Graph builder + Agent panel + ChatGPT OAuth + Web proxy

**Владелец файлов:**
- `crates/canvas-graph-builder/` (новый крейт)
- `crates/canvas-app/src/app/agent_panel.rs` (новый модуль)
- `crates/canvas-app/src/app/graph_builder_ui.rs` (новый модуль — dialog UI)
- `crates/canvas-llm/src/chatgpt_oauth/` (новый модуль внутри canvas-llm, НО отдельная директория — конфликт с потоком A только в Cargo.toml)
- `crates/canvas-web/src/llm_proxy.rs` (новый модуль — cloud function proxy)

**Не трогает:** canvas-suggest, settings_ui.rs, onboarding, overlays.rs (кроме точек вызова)
**Зависимости:** `LlmProvider` trait от потока A, `graph_apply`/`graph_validate` из canvas-mcp

### Что реализует

1. **Graph builder** (`canvas-graph-builder/`, 2-3 сессии):
   - `.byok` вход (текст + режим) → LLM `chat()` → `.canvas` JSON (nodes + edges)
   - Режимы: mindmap / outline / summary
   - Валидация через `graph_validate`
   - Вставка через `graph_apply` (батч)
   - **Cost estimate в dialog** (Q4): перед запросом
   - Preview UI: ghost-nodes показывают результат, accept/reject
   - Layout: FR-010 v2 Sugiyama-lite для расстановки

2. **Agent panel** (`agent_panel.rs`, 3-4 сессии):
   - Чат-UI (новая панель, `UiLayer::Modals` или отдельный dock)
   - LLM получает `tools/list` (42 MCP-инструмента) как `ToolDef[]`
   - **Selection-aware context** (Q3): `selected_nodes: Vec<usize>` (пусто = весь канвас)
   - Промпт: "Selected: [nodes]. Request: {query}. Modify ONLY selected or create new connected. Do NOT touch others."
   - **Validation whitelist** (Q3): `graph_apply` проверяет — каждая op либо создаёт новую, либо модифицирует selected. Непросимые → REJECT.
   - Разрешённые ops над selected (Q3): edit text/color, delete (confirm), move, create/delete edges
   - **Confirm всей связки** (Q3): preview (ghost-nodes), accept/reject
   - **FR-010 v2 layout** (Q3) для agent result
   - Multi-turn: tool_results → LLM продолжает
   - Undo на каждую agent-операцию
   - graph_validate перед apply

3. **ChatGPT OAuth** (`chatgpt_oauth/`, 2-3 сессии, этап 5):
   - OAuth-клиент: PKCE, localhost listener (desktop), deep-link (web)
   - Token storage: keychain (desktop), OPFS encrypted (web)
   - id_token verification (JWKS, JWT)
   - Model discovery: `GET /v1/models`
   - Inference: `POST /v1/responses` (streaming SSE)
   - Refresh flow
   - **Graceful degradation** (Q6): OAuth не работает → fallback на BYOK, toast
   - **Q6 ответ: бэкенд не нужен для хранения** — всё локально. Cloud function для wasm только для OAuth-callback redirect.

4. **Web proxy для wasm** (`llm_proxy.rs`, 1-2 сессии, этап 6):
   - Cloud function (Cloudflare Workers) — держит API-ключи в env
   - `canvas-llm` в wasm → fetch к proxy
   - CORS + rate-limit + audit log
   - OAuth-callback redirect для wasm (Q6)

### Файлы

```
crates/canvas-graph-builder/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── builder.rs             # .byok → LLM → .canvas JSON
│   ├── modes.rs               # mindmap / outline / summary
│   └── layout.rs              # FR-010 v2 для результата
└── tests/

crates/canvas-app/src/
├── app/
│   ├── agent_panel.rs         # НОВЫЙ — чат-UI + selection-aware + validation
│   └── graph_builder_ui.rs    # НОВЫЙ — dialog UI (cost estimate, preview)

crates/canvas-llm/src/
└── chatgpt_oauth/             # НОВЫЙ модуль (директория — не конфликт с потоком A)
    ├── mod.rs
    ├── client.rs              # PKCE, token exchange, refresh
    ├── jwks.rs                # id_token verification
    └── storage.rs             # keychain / OPFS

crates/canvas-web/src/
└── llm_proxy.rs               # НОВЫЙ — cloud function proxy для wasm
```

### Оценка: 8-11 сессий (граф 2-3 + агент 3-4 + OAuth 2-3 + proxy 1-2)

### Выходной артефакт для других потоков

- Graph builder готов (поток B показывает в статусной панели что graph active)
- Agent panel готов (поток B показывает в статусной панели что agent active)
- ChatGPT OAuth provider (поток A регистрирует в списке провайдеров)

---

## Сводная таблица 4 потоков

| Поток | Что | Сессий | Файлы (owned) | Зависимости |
|---|---|---|---|---|
| **A** | canvas-llm crate (ядро) | 3-4 | `crates/canvas-llm/` (всё кроме `chatgpt_oauth/`) | ADR-0016 (trait зафиксирован) |
| **B** | Settings + Статусная панель + Onboarding + User-docs | 3-4 | settings_ui.rs, ai_status_panel.rs, onboarding_ui.rs, ai-mode.ts, ai-features.md | `LlmSettings` от A (stub до реализации) |
| **C** | Suggest LLM + Custom-node + Cache + Skeleton | 3-4 | canvas-suggest/src/llm/, suggest.rs, overlays.rs (suggest часть) | `LlmProvider` от A, `redact_context()` от A |
| **D** | Graph builder + Agent panel + ChatGPT OAuth + Web proxy | 8-11 | canvas-graph-builder/, agent_panel.rs, graph_builder_ui.rs, chatgpt_oauth/, llm_proxy.rs | `LlmProvider` от A, `graph_apply`/`graph_validate` из canvas-mcp |

**Итого: 17-23 сессии** (параллельно — 4 потока, критический путь D = 8-11 сессий)

---

## Координация между потоками

### Trait-контракт (зафиксирован, не меняется)

```rust
// crates/canvas-llm/src/provider.rs — владение потока A, но интерфейс зафиксирован
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn caps(&self) -> ProviderCaps;
    fn models(&self) -> &[ModelInfo];
    fn active_model(&self) -> &str;
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError>;
    async fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<ChoiceAnswer, LlmError>;
    async fn tool_calling(&self, messages: &[Message], tools: &[ToolDef], opts: &ToolCallingOpts) -> Result<Vec<ToolCall>, LlmError>;
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError>;
    async fn health(&self) -> Result<(), LlmError>;
}
```

### Порядок запуска

1. **Поток A начинает первым** (создаёт trait + stub-реализации) — 1 сессия
2. **Потоки B, C, D начинают параллельно** после того как A зафиксировал trait (могут использовать stub/mock для разработки)
3. **Интеграция** когда все 4 потока готовы — 1-2 сессии (wire-up, end-to-end тесты)

### Конфликты файлов (исключены)

| Файл | Поток-владелец | Другие потоки |
|---|---|---|
| `crates/canvas-llm/src/*.rs` | A | B, C, D — только читают trait |
| `crates/canvas-llm/src/chatgpt_oauth/*.rs` | D | A — не трогает эту директорию |
| `crates/canvas-app/src/settings_ui.rs` | B | A, C, D — не трогают |
| `crates/canvas-app/src/app/ai_status_panel.rs` | B | A, C, D — не трогают |
| `crates/canvas-app/src/app/suggest.rs` | C | B, D — не трогают |
| `crates/canvas-app/src/app/agent_panel.rs` | D | B, C — не трогают |
| `crates/canvas-app/src/app/graph_builder_ui.rs` | D | B, C — не трогают |
| `crates/canvas-app/src/app/overlays.rs` | C (suggest часть) + B (status panel вызов) | Координация: B добавляет 1 строку вызова status_panel, C расширяет suggest_cards_overlay |
| `crates/canvas-app/src/app/handler.rs` | B (1 строка вызова) + D (1 строка вызова agent) | Координация: минимальные правки в разных местах |

### Shared файлы (координация)

- `crates/canvas-app/src/app/handler.rs` — B добавляет вызов `ai_status_panel`, D добавляет вызов `agent_panel`. Минимальные правки в разных местах, конфликт маловероятен.
- `crates/canvas-app/src/app/overlays.rs` — B добавляет вызов статусной панели, C расширяет suggest. Разные функции, конфликт маловероятен.
- `crates/canvas-core/src/settings.rs` — A добавляет новые поля Settings. B читает для UI. Координация через ADR-0016.

---

## Что нужно для запуска 4 сессий

### Каждой сессии передать:

1. **ADR-0016** (`docs/adr/adr-0016-llm-layer-canvas-llm.md`) — архитектурное решение, trait зафиксирован
2. **PRD-0010** (`docs/prd/prd-0010-llm-integration-byok-chatgpt.md`) — продуктовые требования
3. **Этот документ** (`docs/dev-researches/llm-implementation-plan-4-streams.md`) — какой поток, какие файлы, что реализовывать
4. **Дизайн-документ** (`docs/dev-researches/byok-chatgpt-oauth-design.md`) — детали trait + провайдеров
5. **Benchmark** (`docs/dev-researches/llm-mm-source-benchmark.md`) — обоснование LLM
6. **Конкретный поток** (A/B/C/D) — с указанием файлов и задач

### Рекомендуемый порядок запуска

1. **Сессия A** (1 сессия) — создать `canvas-llm` crate с trait + stub-реализации, зафиксировать API
2. **Параллельно 4 сессии** (B, C, D + A продолжает):
   - A: реализует провайдеры (OpenAI-compat, Anthropic, redact, cost, compliance)
   - B: Settings таб + статусная панель + onboarding + user-docs
   - C: Suggest LLM + custom-node + cache + skeleton
   - D: Graph builder + agent panel (OAuth и proxy — позже, после A)
3. **Интеграция** (1-2 сессии) — wire-up, end-to-end тесты, CI

---

## История

- `2026-10-03` — агент (Super Z): план 4 потоков создан. Финальные ответы владельца на Q1-Q7 (явный маркер redact, все agent ops разрешены, статусная панель справа, self-hosted любой, OAuth без бэкенда, 3 ghost-nodes, GitHub issue для шаблонов, $1/день лимит). 4 потока: A (canvas-llm ядро), B (Settings+Status+Onboarding), C (Suggest LLM), D (Graph+Agent+OAuth+Proxy). 17-23 сессии, критический путь D.
