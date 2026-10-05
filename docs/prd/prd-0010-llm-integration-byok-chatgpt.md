# PRD-0010: LLM-интеграция CanvasDesk — BYOK + Sign-in-with-ChatGPT для suggest, graph builder, agent panel

- **Статус:** черновик
- **Тип:** PRD (документ требований продукта; реализация оформляется отдельными `FR-NNN` по шаблону `docs/change-requests/cr-template.md`)
- **Приоритет:** важно
- **Владелец:** владелец продукта
- **Источник:** запрос владельца 2026-10-03: «нужно спроектировать функционал bring your own key для suggest нод, автодополнения нод, построения полноценных графов и т.д., всё что умеет mcp. Так же надо проанализировать как реализовать интеграцию Sign-in-with-ChatGPT»
- **Связанные документы:** ADR-0016 (LLM-слой canvas-llm), ADR-0009 (MCP-транспорт), ADR-0010 (чистота MCP-stdio), ADR-0011 (wasm-гейт), FR-079 (suggest), `docs/BYOK.md` (формат .byok), `docs/dev-researches/llm-mm-source-benchmark.md` (benchmark), `docs/dev-researches/byok-chatgpt-oauth-design.md` (дизайн-документ)
- **Создан:** 2026-10-03
- **Обновлён:** 2026-10-03

---

## 1. Резюме

CanvasDesk получает LLM-слой для трёх AI-сценариев: автодополнение шаблонов нод (suggest), генерация полноценных графов из текста (.byok), и агентные операции через MCP (42 инструмента). Пользователь выбирает провайдера в Settings: Sign-in-with-ChatGPT (OAuth, без ключа — для mass-market) или BYOK (OpenAI / Anthropic Claude / z.ai GLM / Moonshot Kimi / OpenRouter / Ollama — для power-users). Единый `LlmProvider` trait (chat / choice / tool_calling / embed) абстрагирует различия API. LLM-транспорт отделён от MCP-моста (ADR-0010 сохранён). Benchmark показал +114% к baseline (p@1 0.468 → 1.000 с glm-5.3-flash).

Граница PoC: этап 1–2 (canvas-llm crate + suggest с LLM mm-source) — MVP для валидации UX и cost. Этапы 3–6 (graph builder, agent panel, ChatGPT OAuth, web proxy) — после подтверждения.

---

## 2. Контекст и проблема

### 2.1 Как это выглядит сегодня

**MCP-сервер** (`crates/canvas-mcp/src/lib.rs`): 42 инструмента (CRUD нод/рёбер, flow, graph_validate/apply, what-if, scheme, expr, suggest, templates). Внешний агент (Claude Desktop) вызывает через stdio-мост.

**Suggest-движок** (`crates/canvas-suggest/`, FR-079): гибрид lex (BM25, p@1=0.404) + mm (Laya sidecar, p@1=0.383), fusion α=0.85 → p@1=0.468. Laya — fine-tuned classifier, `/v1/systemone` protocol (System One / Jev-совместимый), localhost sidecar.

**BYOK-документ** (`docs/BYOK.md`): формат `.byok` (JSON для генерации mindmap/outline/summary), статус «предложение, не реализовано». Команды MCP `/canvasdesk.generate`, `/canvasdesk.insert` описаны, но нет LLM-провайдера.

**Benchmark** (`docs/dev-researches/llm-mm-source-benchmark.md`): 11 LLM протестировано. Лучшие: glm-5.3-flash (p@1=1.000, $0.121/1000), nemotron-super:free (p@1=0.800, $0), jev-1.13 (p@1=0.733, 0.46с, System One protocol — тот же что Laya). Вывод: LLM превосходит baseline на +57–114%.

### 2.2 Чего не хватает

1. **Suggest с LLM** — `canvas-suggest/src/laya/` есть, но нет OpenRouter/OpenAI/Anthropic/z.ai/Moonshot-клиента. Benchmark доказал прирост, интеграция не реализована.
2. **Генерация графов** — `.byok`-формат описан, но нет LLM-провайдера который превращает текст в граф (ноды + рёбра). Сейчас это делает внешний агент через MCP.
3. **Agentные операции** — MCP-инструменты доступны внешнему агенту, но CanvasDesk не может сам звонить LLM для принятия решений («какой инструмент вызвать»).
4. **Auth-путь без ключа** — пользователи не хотят брать API-ключ. Sign-in-with-ChatGPT (OAuth, подписка покрывает запросы) решает это.
5. **Унификация** — 6 провайдеров с разными API (OpenAI-compat vs Anthropic vs System One), нужен единый trait.

### 2.3 Зацепки в коде и документации

- `crates/canvas-suggest/src/laya/client.rs:70` — `LayaClient::build_payload` (System One protocol, переиспользуется для jev-1.13)
- `crates/canvas-suggest/src/types.rs:62` — `SuggestContext`, `ScoredOption` (готовый интерфейс для mm-source)
- `crates/canvas-mcp/src/lib.rs:566` — `tools_list()` (42 инструмента, для agent panel tool-calling)
- `crates/canvas-core/src/settings.rs:372` — `Settings` struct (расширяется `llm_provider`, `llm_model`)
- `crates/canvas-suggest/tests/fixtures/evalset/source_a.jsonl` — замороженный eval-сет (175 фикстур)
- `docs/BYOK.md` — формат .byok (обновляется под graph-builder)
- `docs/dev-researches/llm-mm-source-benchmark.md` — benchmark (готовые prompt + метрики)
- `docs/dev-researches/byok-chatgpt-oauth-design.md` — полный дизайн-документ (10 разделов)

---

## 3. Цели и метрики

### 3.1 Цели (G1–G6)

| ID | Цель | Метрика |
|---|---|---|
| G1 | Suggest с LLM превосходит baseline | p@1 ≥ 0.800 на test-сплите (47 фикстур) |
| G2 | Graph builder генерирует валидные графы | ≥90% с первой попытки проходит `graph_validate` |
| G3 | Agent panel решает задачи без ручной правки | ≥70% задач через tool-calling |
| G4 | Setup time ≤ 30 сек для ChatGPT OAuth | от клика «Continue with ChatGPT» до первого запроса |
| G5 | Cost ≤ $0.50/мес на power-user | 3000 запросов/мес, гибрид free+paid |
| G6 | ADR-0010 сохранён | LLM-транспорт ≠ MCP-мост, CI golden-тесты lex-only зелёные |

### 3.2 Метрики качества (из benchmark)

| Метрика | Baseline | Цель |
|---|---|---|
| Suggest p@1 | 0.468 (lex+Laya) | ≥0.800 (LLM mm-source) |
| Suggest latency p95 | 800мс (Laya) | ≤2с (jev-1.13) или ≤10с (glm-5.3-flash с prefetch) |
| Graph builder success | N/A | ≥90% валидных с первой попытки |
| Agent task completion | N/A | ≥70% без правки |
| User setup time | 5 мин (Laya) | ≤30 сек (ChatGPT OAuth) |
| Cost/user/мес | $0 (Laya) | ≤$0.50 (гибрид) |

---

## 4. Пользовательские истории

### US-1: Mass-market пользователь (ChatGPT подписчик)

**Как** пользователь с ChatGPT Plus/Pro,
**я хочу** залогиниться через «Continue with ChatGPT»,
**чтобы** получить автодополнение шаблонов и генерацию графов без API-ключа.

**Acceptance criteria:**
- Клик «Continue with ChatGPT» → браузер открывает ChatGPT login
- После login → access_token сохранён, модель доступна
- Suggest работает (p@1 ≥ 0.800 на test-сплите)
- Graph builder генерирует граф из текста
- Cost: $0 (покрыто подпиской)
- Fallback на lex при rate limit подписки

### US-2: Power-user (BYOK)

**Как** пользователь с API-ключом OpenAI/Anthropic/z.ai,
**я хочу** выбрать провайдера и модель в Settings,
**чтобы** использовать лучшую модель для своих задач.

**Acceptance criteria:**
- Settings → LLM → выбор из 8 провайдеров
- Ввод API-ключа (сохраняется в OS keychain, НЕ в файле)
- Health-check: кнопка «Проверить ключ» → валидация
- Выбор модели из списка (`/v1/models` или hardcoded)
- Suggest + graph builder + agent panel работают через выбранную модель
- Cost: pay-per-token, виден в UI (оценка за сессию)

### US-3: Suggest при редактировании ноды

**Как** пользователь редактирующий ноду,
**я хочу** видеть подсказку подходящего шаблона,
**чтобы** не искать вручную в каталоге из 19+ шаблонов.

**Acceptance criteria:**
- При вводе текста в ноду → debounce 500мс → LLM choice-запрос
- Подсказка появляется через ≤2с (jev-1.13) или ≤10с (glm-5.3-flash с prefetch)
- Fallback на lex при ошибке/таймауте (мгновенно)
- Undo на применение подсказки (FR-006)
- Отключаемо в Settings («LLM-подсказки» тумблер)

### US-4: Генерация графа из текста

**Как** пользователь с текстом (например, meeting notes),
**я хочу** сгенерировать граф нод + рёбер,
**чтобы** быстро структурировать идеи на канвасе.

**Acceptance criteria:**
- UI: dialog с режимами (mindmap / outline / summary), полем текста, preview
- LLM генерирует JSON {nodes, edges} → валидация `graph_validate`
- Preview показывает граф до вставки
- Accept → вставка через `graph_apply` (батч, один undo-шаг)
- Reject → отмена
- Cost: виден в dialog (оценка токенов)

### US-5: Agent panel для операций

**Как** пользователь,
**я хочу** написать «создай ноду CAC и свяжи с LTV» в чат,
**чтобы** CanvasDesk сам выполнил операции через MCP.

**Acceptance criteria:**
- Чат-панель: пользователь пишет запрос на естественном языке
- LLM получает `tools/list` (42 MCP-инструмента)
- LLM возвращает tool_calls → приложение исполняет через `graph_apply`
- Multi-turn: LLM видит результаты, продолжает
- Undo на каждую agent-операцию
- Подтверждение для деструктивных ops (`node_delete`)
- Safety: `graph_validate` перед apply

---

## 5. Функциональные требования

### F-1: canvas-llm crate (этап 1)

- **F-1.1:** Новый feature-gated крейт `canvas-llm/` (default build не тянет network-deps)
- **F-1.2:** `LlmProvider` trait: `chat()`, `choice()`, `tool_calling()`, `embed()`, `health()`
- **F-1.3:** `ProviderCaps`: {chat, choice, tool_calling, embed, streaming, vision}
- **F-1.4:** `ModelInfo`: id, display_name, context_length, supports_tools, supports_vision, pricing
- **F-1.5:** `OpenAiCompatibleProvider` — один адаптер для OpenAI, OpenRouter, z.ai, Moonshot, Ollama
- **F-1.6:** `AnthropicClaudeProvider` — отдельный адаптер (`/v1/messages`, `x-api-key`, `tool_use`)
- **F-1.7:** Settings: `llm_provider` (enum), `llm_api_key`, `llm_model`, `llm_endpoint`
- **F-1.8:** Storage: OS keychain (desktop), OPFS encrypted (web)
- **F-1.9:** Health-check для каждого провайдера (валидация ключа)

### F-2: Suggest с LLM mm-source (этап 2)

- **F-2.1:** `canvas-suggest/src/llm/` модуль (feature `l1-llm`, параллельно `l1-laya`)
- **F-2.2:** `LlmMmSource` реализует `SuggestEngine` (тот же интерфейс что Laya)
- **F-2.3:** Prompt template из `llm-mm-source-benchmark.md` (готов, проверен)
- **F-2.4:** Fusion: `α·lex + (1−α)·llm`, α-пересчёт (LLM точнее → α ~0.6)
- **F-2.5:** Fallback: ошибка/таймаут → `fusion(lex, ∅) = lex` (как Laya)
- **F-2.6:** LLM-mm отключена в CI (golden-тесты только lex)
- **F-2.7:** Debounce 500мс для inline-UX
- **F-2.8:** Prefetch: запрос при фокусе ноды (до начала правки), кэш по хешу контекста (TTL 5 мин)
- **F-2.9:** Redact context перед отправкой (Q1): `price=<redacted> руб` — явный маркер, сохраняем unit
- **F-2.10:** Loading-state skeleton (Q7): полупрозрачный квад с spinner → morph в ghost-node
- **F-2.11:** Confidence-threshold в settings (Q7, default 0.5): если confidence < threshold → скрыть подсказку
- **F-2.12:** Custom-node suggest (Q7, НОВОЕ): LLM генерирует 3 варианта кастомных нод (title + formula + params), 3 ghost-nodes рядом. Промпт передаёт доступные формулы/операторы (из expr.rs) + право создать кастомную.
- **F-2.13:** Custom-нода валидация через expr_eval (Q7): битые варианты скрываются
- **F-2.14:** Cost limit per request (Q7): перед запросом estimate_cost, если > limit → skip + fallback lex
- **F-2.15:** Benchmark redacted vs raw (R10): на этапе 2, если падение >10% p@1 → fallback на consent-dialog
- **F-2.16:** "Сохранить как шаблон" (Q7): после применения custom-ноды → GitHub issue (тип enhancement) к canvasdesk repo

### F-3: Graph builder (этап 3)

- **F-3.1:** Новый крейт `canvas-graph-builder/`
- **F-3.2:** `.byok` вход (текст + режим) → LLM `chat()` → `.canvas` JSON (nodes + edges)
- **F-3.3:** Валидация через `graph_validate` (MCP-инструмент)
- **F-3.4:** Вставка через `graph_apply` (батч до 256 операций)
- **F-3.5:** UI: dialog с режимами (mindmap/outline/summary), preview, accept/reject
- **F-3.6:** Обновление `docs/BYOK.md` — спецификация graph-builder input
- **F-3.7:** Cost estimate в dialog (оценка токенов до запроса)

### F-4: Agent panel (этап 4)

- **F-4.1:** `canvas-app/src/agent_panel.rs` — чат-UI
- **F-4.2:** LLM получает `tools/list` (42 MCP-инструмента) как `ToolDef[]`
- **F-4.3:** `tool_calling()` → tool_calls → исполнение через `graph_apply`
- **F-4.4:** Multi-turn: tool_results отправляются обратно, LLM продолжает
- **F-4.5:** Undo на каждую agent-операцию (FR-006)
- **F-4.6:** Подтверждение для деструктивных ops (`node_delete`)
- **F-4.7:** `graph_validate` перед apply
- **F-4.8:** Selection-aware context (Q3): `selected_nodes: Vec<usize>` (пусто = весь канвас). Промпт: "Modify ONLY selected or create new connected. Do NOT touch others."
- **F-4.9:** Validation whitelist (Q3): graph_apply проверяет — каждая op либо создаёт новую ноду, либо модифицирует selected. Непросимые → REJECT
- **F-4.10:** Разрешённые ops над selected (Q3): edit text/color, delete (confirm), move, create/delete edges
- **F-4.11:** Confirm всей связки (Q3): preview (ghost-nodes), accept/reject
- **F-4.12:** FR-010 v2 layout (Sugiyama-lite) для agent result — ноды расставлены по графу, не кластер в углу

### F-5: Sign-in-with-ChatGPT (этап 5)

- **F-5.1:** `canvas-llm/src/chatgpt_oauth/` — OAuth-клиент (PKCE, JWKS, refresh)
- **F-5.2:** Localhost HTTP listener (desktop), deep-link `canvasdesk://oauth/callback` (web)
- **F-5.3:** Token storage: keychain (desktop), OPFS encrypted (web) — **бэкенд не нужен для хранения** (Q6 ответ: всё локально)
- **F-5.4:** id_token verification (JWT, JWKS, issuer/audience/nonce)
- **F-5.5:** Model discovery: `GET /v1/models`
- **F-5.6:** Inference: `POST /v1/responses` (streaming SSE)
- **F-5.7:** Refresh flow: access_token истёк → refresh_token → новый токен
- **F-5.8:** UI: «Continue with ChatGPT» button в Settings
- **F-5.9:** Graceful degradation (Q6): OAuth не работает → fallback на BYOK, toast "ChatGPT недоступен, переключились на {fallback}"
- **F-5.10:** Cloud function для wasm ТОЛЬКО для OAuth-callback redirect (Q6, не хранит токены)

### F-6: Web proxy для wasm (этап 6)

- **F-6.1:** Cloud function (Cloudflare Workers / Vercel) — держит API-ключи
- **F-6.2:** `canvas-llm` в wasm → fetch к proxy
- **F-6.3:** Proxy: валидация CORS, rate-limit per-user, audit log
- **F-6.4:** OAuth redirect_uri для wasm: deep-link + cloud function для token exchange

### F-7: Settings "AI and Models" таб + Статусная панель (Q2+Q4)

- **F-7.1:** Новый 9-й таб "AI and Models" в Settings (после "Внешний вид")
- **F-7.2:** Per-feature выбор провайдера (Q2): Suggest / Graph Builder / Agent Panel — каждый со своим dropdown. ChatGPT OAuth недоступен для Suggest
- **F-7.3:** Rate limit counter (Q2): local counting, "ChatGPT: 47/80 сообщений осталось"
- **F-7.4:** Cost limit (Q7): $1/день по умолчанию, slider. При 80% — dialog расширения
- **F-7.5:** Confidence threshold (Q7): default 0.5, slider
- **F-7.6:** Telemetry opt-in (Q7): default OFF, возможно через PostHug
- **F-7.7:** Data residency (Q5): Local / Cloud / Self-hosted + описание
- **F-7.8:** Self-hosted endpoint (Q5): URL + API key, ответственность пользователя
- **F-7.9:** Статусная панель (Q4): справа над миникартой. Активная модель, включённые функции, cost за сессию, cost за день/лимит. ⚙ → Settings, ⏸ → pause AI. На mobile/узких — прятать
- **F-7.10:** Cost estimation engine (Q4): before/after request, tariff calculator

### F-8: Onboarding AI-режима + User documentation (Q5)

- **F-8.1:** Новый экран онбординга (после language/role): "Выберите режим AI: Local / Cloud / Self-hosted"
- **F-8.2:** Сохранение выбора в `data_residency` setting
- **F-8.3:** Ссылка на user-docs/ai-features.md
- **F-8.4:** `user-docs/ai-features.md` — полный гайд: AI-функции, какие данные отправляются, privacy-режимы, self-hosted, cost
- **F-8.5:** Онбординг-тур сценарий `sdk/web-onboarding/src/scenarios/ai-mode.ts`
- **F-8.6:** Информация о self-hosted в Settings с однозначным описанием что включает пользователь

---

## 6. Нефункциональные требования

### NF-1: Безопасность

- API-ключи в OS keychain (desktop), НЕ в config.toml
- OAuth tokens в OPFS encrypted (web), НЕ в localStorage (XSS-риск)
- API-ключи НЕ в wasm-bundle (cloud proxy)
- Prompt injection: `graph_validate` перед apply, sandbox tool-calling
- Privacy: опциональный local-only режим (Ollama / Laya), явный consent для cloud

### NF-2: Производительность

- Suggest latency p95 ≤ 2с (jev-1.13) или ≤ 10с (glm-5.3-flash с prefetch)
- Debounce 500мс для inline-UX
- LLM-mm не блокирует lex (async, fallback)
- Graph builder: streaming preview (partial результат до completion)

### NF-3: Совместимость

- ADR-0010 (MCP-stdio чистота): LLM-транспорт отдельный от MCP-моста
- ADR-0011 (wasm-гейт): `canvas-llm` feature-gated, default build без network-deps
- Laya sidecar back-compat: `l1-laya` feature остаётся
- Golden-тесты suggest: LLM-mm отключена в CI, проверяется только lex
- `docs/BYOK.md` обновляется (не ломается)

### NF-4: Cost

- Power-user (3000 запросов/мес) ≤ $0.50/мес
- Free-tier (nemotron-super:free / Ollama / Laya) = $0
- Cost estimate в UI перед запросом (graph builder, agent panel)
- Rate limit fallback: 429 → lex (suggest) / retry с backoff (agent)

### NF-5: Отказоустойчивость

- Fallback-цепочка: ChatGPT OAuth → BYOK → lex (suggest) / ошибка (graph/agent)
- Health-check провайдера при старте
- Retry с backoff при транспортной ошибке (1 повтор, как Laya)
- Timeout: 10с для suggest, 30с для graph builder, 60с для agent

---

## 7. Дизайн (решение)

Принято ADR-0016: `canvas-llm` crate с `LlmProvider` trait, 6 провайдеров в 3 protocol-группах, гибрид BYOK + Sign-in-with-ChatGPT. LLM-транспорт отделён от MCP-моста (ADR-0010 сохранён).

Полный дизайн: `docs/dev-researches/byok-chatgpt-oauth-design.md` (10 разделов, trait определение, все провайдеры, transport, риски, этапы).

Benchmark: `docs/dev-researches/llm-mm-source-benchmark.md` (11 моделей, p@1 до 1.000, cost $0.036–0.121/1000).

---

## 8. Дорожная карта — 4 параллельных потока

Полный план: `docs/dev-researches/llm-implementation-plan-4-streams.md`

| Поток | Что | Сессий | Файлы (owned) | Зависимости |
|---|---|---|---|---|
| **A** | canvas-llm crate (ядро): LlmProvider trait, 6 провайдеров, redact engine, cost engine, compliance | 3-4 | `crates/canvas-llm/` (всё кроме `chatgpt_oauth/`) | ADR-0016 |
| **B** | Settings "AI and Models" + Статусная панель + Onboarding + User-docs | 3-4 | settings_ui.rs, ai_status_panel.rs, onboarding_ui.rs, ai-mode.ts, ai-features.md | LlmSettings от A |
| **C** | Suggest LLM mm-source + Custom-node suggest + Cache + Skeleton | 3-4 | canvas-suggest/src/llm/, suggest.rs, overlays.rs (suggest) | LlmProvider от A, redact от A |
| **D** | Graph builder + Agent panel + ChatGPT OAuth + Web proxy | 8-11 | canvas-graph-builder/, agent_panel.rs, graph_builder_ui.rs, chatgpt_oauth/, llm_proxy.rs | LlmProvider от A, graph_apply/validate из canvas-mcp |
| **Итого** | | **17-23** | | Критический путь: D (8-11) |

**Порядок запуска:**
1. Поток A (1 сессия) — создать crate с trait + stub
2. Потоки B, C, D параллельно (+ A продолжает)
3. Интеграция (1-2 сессии) — wire-up, end-to-end тесты

MVP-граница: потоки A+C (suggest с LLM) — валидация UX + cost. Потоки B+D — после подтверждения.

---

## 9. Открытые вопросы

Все критические вопросы (Q1-Q7) решены владельцем (2026-10-03). Остаются технические:

| ID | Вопрос | Решение | Статус |
|---|---|---|---|
| Q1 | Privacy consent | Redact с явным маркером `price=<redacted> руб` (Q1 ответ) | ✓ решено |
| Q2 | ChatGPT OAuth для suggest | Per-feature, ChatGPT только graph/agent (Q2 ответ) | ✓ решено |
| Q3 | Agent auto-apply vs confirm | Confirm всей связки, selection-aware, все ops над selected разрешены (Q3 ответ) | ✓ решено |
| Q4 | Cost visibility | Статусная панель справа над миникартой, прятать на mobile (Q4 ответ) | ✓ решено |
| Q5 | Offline-first vs cloud-first | Hybrid + self-hosted любой + документация + онбординг (Q5 ответ) | ✓ решено |
| Q6 | ChatGPT OAuth хранение | Всё локально, бэкенд не нужен, cloud function только для wasm redirect (Q6 ответ) | ✓ решено |
| Q7 | Suggest UX | Loading skeleton + confidence threshold + 3 custom ghost-nodes + expr_eval + GitHub issue для шаблонов + $1/день лимит (Q7 ответ) | ✓ решено |
| T1 | α-пересчёт для fusion с LLM | Замер на test-сплите в потоке C | отложено |
| T2 | Platt-перекалибровка под LLM confidence | После потока C, ECE-замер | отложено |
| T3 | Telemetry: PostHug vs свой backend | Опционально, возможно PostHug custom event | отложено |
| T4 | Benchmark redacted vs raw | В потоке C, если падение >10% → consent-dialog fallback | отложено |

---

## 10. Риски и митигации

| Риск | Вероятность | Влияние | Митигация |
|---|---|---|---|
| API-ключ в wasm-bundle (утечка) | Средняя | Высокое | Cloud-proxy (этап 6), ключ в env proxy |
| LLM latency убивает inline-UX | Высокая | Среднее | Prefetch (этап 2), jev-1.13 (0.46с), lex fallback |
| Недетерминизм ломает golden-тесты | Высокая | Низкое | Feature-gating в CI, lex-only тесты |
| Prompt injection из канвас-контента | Средняя | Высокое | `graph_validate`, sandbox tool-calling |
| ChatGPT Plus rate limits | Средняя | Среднее | Fallback на BYOK при 429 |
| Cost превышает бюджет | Низкая | Среднее | Cost estimate в UI, free-tier fallback |
| OAuth token в localStorage (XSS) | Низкая | Высокое | OPFS encrypted, CSP strict |
| ADR-0010 нарушение | Низкая | Высокое | LLM-транспорт отдельный, ревью ADR-0016 |

---

## 11. Definition of Done

- [ ] ADR-0016 принят (статус «принято»)
- [ ] `canvas-llm` crate создан, feature-gated
- [ ] `LlmProvider` trait реализован для 6 провайдеров
- [ ] Suggest с LLM mm-source: p@1 ≥ 0.800 на test-сплите
- [ ] Graph builder: ≥90% валидных графов
- [ ] Agent panel: ≥70% задач без правки
- [ ] ChatGPT OAuth: setup ≤ 30 сек
- [ ] Web proxy для wasm: API-ключи не в bundle
- [ ] CI: LLM-mm отключена, golden-тесты lex-only зелёные
- [ ] ADR-0010 (MCP-stdio чистота) не нарушена
- [ ] `docs/BYOK.md` обновлён
- [ ] Cost ≤ $0.50/мес на power-user

---

## 12. История

- `2026-10-03` — агент (Super Z): PRD создан. 3 AI-сценария (suggest, graph builder, agent panel), 6 провайдеров, гибрид BYOK + Sign-in-with-ChatGPT. 6 этапов (12-16 сессий). Статус: черновик. Связан с ADR-0016.
- `2026-10-03` — агент (Super Z): правка — финальные ответы владельца на Q1-Q7. Добавлены: F-2.9..F-2.16 (redact, custom-node, skeleton, confidence, cost-limit, save-as-template), F-4.8..F-4.12 (selection-aware, whitelist, confirm, FR-010 layout), F-5.9..F-5.10 (graceful degradation, cloud function для wasm OAuth). План 4 параллельных потоков в `docs/dev-researches/llm-implementation-plan-4-streams.md`. 17-23 сессии.
- `2026-10-05` — агент (Super Z + суб-агенты): **Stream D завершён** — реализованы оставшиеся пункты. (1) `canvas-llm/src/chatgpt_oauth/` (F-5.1–F-5.7): PKCE RFC 7636 S256, OAuthClient (login URL, localhost-listener desktop, deep-link web, exchange/refresh, proxy_url для wasm), JWT claims + SignatureVerifier (v1 SECURITY-стаб, §4.7), TokenStore, ChatGptOAuthProvider (Responses API, авто-refresh, choice через forced tool `rank`, embed→NotSupported). (2) `cloud/llm-proxy/` (F-5.10): stateless Cloudflare Worker — 302 deep-link redirect, pass-through token exchange, CORS-прокси /v1/*, BYOK-through-proxy; 45 node-тестов + CI-job `llm-proxy`. (3) Интеграция canvas-app: llm_factory (Byok→OpenRouter/selfhost/Ollama/ChatGptOAuth), OAuth-флоу Idle/Waiting/Connected/Failed, FileTokenStore (0600), Settings-блок «Вход ChatGPT», fallback F-5.9, i18n ru+en. (4) PoC-валидация (E2E-смоук oauth_e2e.rs против mock-proxy, без сети): полный desktop-цикл + метрика G4, CSRF state-mismatch → 403/Auth, авто-refresh 401→retry, проактивный refresh по expires_at. Найдены и исправлены 2 бага: парсинг request-line в `wait_for_code` (метод вместо пути — вечный 404-цикл листенера) и отсутствие email в `account_email` при exchange (бейдж Settings пуст до первого refresh). Гейты: fmt/clippy/workspace ~2600 тестов/wasm32 — ok.

---

## 13. Источники

- ADR-0016: `docs/adr/adr-0016-llm-layer-canvas-llm.md`
- Дизайн-документ: `docs/dev-researches/byok-chatgpt-oauth-design.md`
- Benchmark: `docs/dev-researches/llm-mm-source-benchmark.md`
- FR-079: `docs/change-requests/fr-079-*.md` (suggest)
- BYOK формат: `docs/BYOK.md`
- Laya-клиент: `crates/canvas-suggest/src/laya/client.rs`
- MCP-инструменты: `crates/canvas-mcp/src/lib.rs`
- Sign-in-with-ChatGPT: https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt
