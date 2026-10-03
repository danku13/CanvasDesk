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
- **F-2.8:** Prefetch: запрос при фокусе ноды (до начала правки)

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

### F-5: Sign-in-with-ChatGPT (этап 5)

- **F-5.1:** `canvas-llm/src/chatgpt_oauth/` — OAuth-клиент (PKCE, JWKS, refresh)
- **F-5.2:** Localhost HTTP listener (desktop), deep-link `canvasdesk://oauth/callback` (web)
- **F-5.3:** Token storage: keychain (desktop), OPFS encrypted (web)
- **F-5.4:** id_token verification (JWT, JWKS, issuer/audience/nonce)
- **F-5.5:** Model discovery: `GET /v1/models`
- **F-5.6:** Inference: `POST /v1/responses` (streaming SSE)
- **F-5.7:** Refresh flow: access_token истёк → refresh_token → новый токен
- **F-5.8:** UI: «Continue with ChatGPT» button в Settings

### F-6: Web proxy для wasm (этап 6)

- **F-6.1:** Cloud function (Cloudflare Workers / Vercel) — держит API-ключи
- **F-6.2:** `canvas-llm` в wasm → fetch к proxy
- **F-6.3:** Proxy: валидация CORS, rate-limit per-user, audit log
- **F-6.4:** OAuth redirect_uri для wasm: deep-link + cloud function для token exchange

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

## 8. Дорожная карта

| Этап | Что | Сессий | Зависимости |
|---|---|---|---|
| 1 | canvas-llm crate + BYOK-провайдеры (OpenAI-compat + Anthropic) | 2-3 | ADR-0016 |
| 2 | Suggest с LLM mm-source | 1 | Этап 1 |
| 3 | Graph builder (.byok → граф) | 2-3 | Этап 1 |
| 4 | Agent panel (tool-calling через MCP) | 3-4 | Этап 1 |
| 5 | Sign-in-with-ChatGPT (OAuth) | 2-3 | Этап 1 |
| 6 | Web proxy для wasm | 1-2 | Этап 1, 5 |
| **Итого** | | **12-16** | |

MVP-граница: этапы 1–2 (валидация UX + cost). Этапы 3–6 — после подтверждения.

---

## 9. Открытые вопросы

| ID | Вопрос | Решение |
|---|---|---|
| Q1 | α-пересчёт для fusion с LLM (0.85 для Laya → ? для LLM) | Замер на test-сплите после этапа 2 |
| Q2 | Platt-перекалибровка под LLM confidence (отличается от Laya) | После этапа 2, ECE-замер |
| Q3 | Cloud-proxy: Cloudflare Workers vs Vercel vs self-hosted | Решение на этапе 6 |
| Q4 | Deep-link `canvasdesk://` для OAuth на web — регистрация схемы | Решение на этапе 5 |
| Q5 | Agent panel: auto-apply vs подтверждение каждой операции | Решение на этапе 4 (UX-тест) |
| Q6 | Privacy: consent-диалог для cloud LLM (контекст нод уходит) | Решение на этапе 1 |

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
