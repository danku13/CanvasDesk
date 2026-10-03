# BYOK + Sign-in-with-ChatGPT — дизайн интеграции LLM в CanvasDesk

**Дата:** 2026-10-03
**Тип:** дизайн-документ (proposal, не реализовано)
**Родительские документы:** `BYOK.md` (формат .byok, статус «предложение»), `llm-mm-source-benchmark.md` (benchmark LLM), `inline-ai-autocomplete-hypothesis.md` (FR-079), `adr-0009-mcp-transport-compatibility.md`, `adr-0010-mcp-stdio-purity.md`
**Связанные крейты:** `canvas-suggest/` (FR-079 suggest), `canvas-mcp/` (42 инструмента), `canvas-core/src/settings.rs`, `canvas-web/` (wasm), `canvas-app/` (desktop)
**Статус:** proposal для рецензии владельцем перед превращением в PRD/FR

---

## 1. Резюме для решения

**Что:** Спроектировать BYOK (Bring Your Own Key) для трёх AI-сценариев CanvasDesk: (1) suggest/автодополнение шаблонов нод, (2) генерация полноценных графов из текста/структуры, (3) любые операции доступные через MCP-инструменты (42 шт). Плюс — проанализировать интеграцию Sign-in-with-ChatGPT как альтернативный auth-путь (без ручного API-ключа).

**Вердикт дизайна:** BYOK реализуем через **единый LLM-слой абстракции** (`canvas-llm` крейт) с тремя провайдерами: (A) OpenAI-compatible API (OpenRouter, OpenAI direct, Anthropic, Ollama, z.ai), (B) Sign-in-with-ChatGPT (OAuth, без ключа пользователя), (C) legacy Laya sidecar (для offline/back-compat). Слой подключается к: suggest-движку (FR-079 mm-source), новому tool-graph-builder (генерация графов через MCP), и MCP-бриджу (агентные операции через LLM-tool-calling). Wasm-сборка требует proxy (desktop: sidecar, web: cloud function) — API-ключи нельзя в public bundle.

**Ключевое ограничение:** чистота MCP-канала (ADR-0010) — LLM-вызовы НЕ идут через stdio MCP-мост. LLM-вызоры идут через отдельный транспорт (HTTP proxy / sidecar), MCP-инструменты вызываются ЛОКАЛЬНО (как сейчас), LLM только решает «какой инструмент вызвать с какими аргументами».

---

## 2. Контекст и мотивация

### 2.1. Что умеет CanvasDesk сегодня

**MCP-сервер** (`canvas-mcp/src/lib.rs`, 42 инструмента):
- CRUD нод/рёбер: `node_create_note`, `node_create_file`, `node_update_text`, `node_move`, `node_resize`, `node_delete`, `edge_create`, `edge_delete`
- Flow: `flow_set_kind`, `flow_recalc`, `flow_cycle_check`, `edge_ports`
- Graph: `graph_validate`, `graph_apply` (батч до 256 операций)
- What-if: `whatif_set_override`, `whatif_set_param`
- Scheme: `scheme_instantiate` (шаблонные сцены)
- Expr: `expr_eval`, `expr_stats`
- Suggest: `suggest_template` (FR-079, lex+Laya fusion)
- Templates: `template_instantiate`, `template_list`

**Suggest-движок** (`canvas-suggest/`, FR-079):
- Гибрид lex (BM25) + mm (Laya sidecar, System One protocol)
- Fusion α=0.85, p@1=0.468 baseline
- `laya/client.rs` — готовый клиент `/v1/systemone` (Jev-совместимый)
- Benchmark (`llm-mm-source-benchmark.md`): LLM превосходит baseline на +57-114%, лучшая модель glm-5.3-flash p@1=1.000, jev-1.13 p@1=0.733 с latency 0.46с

**BYOK-документ** (`docs/BYOK.md`):
- Статус: «предложение, не реализовано»
- Описывает формат `.byok` (JSON для генерации mindmap/structured_notes/outline/summary)
- Команды MCP: `/canvasdesk.generate`, `/canvasdesk.insert`, `/canvasdesk.byok_read`
- Виджет `byok-generator` (sandbox WebView2, без network)

### 2.2. Чего не хватает

1. **Suggest с LLM** — `canvas-suggest/src/laya/` есть, но нет OpenRouter/OpenAI-клиента. Benchmark показал +114% к baseline, но интеграция не реализована.
2. **Генерация графов** — `.byok`-формат описан, но нет LLM-провайдера который превращает текст в граф (ноды + рёбра). Сейчас это делает внешний агент (Claude Desktop) через MCP.
3. **Agentные операции** — MCP-инструменты доступны внешнему агенту, но CanvasDesk не может сам звонить LLM для принятия решений («какой инструмент вызвать»).
4. **Auth-путь без ключа** — пользователи не хотят брать API-ключ OpenAI. Sign-in-with-ChatGPT решает это (OAuth, используется ChatGpt-подписка).

### 2.3. Зачем Sign-in-with-ChatGPT

- **Без API-ключа** — пользователь логинится в ChatGPT, подписка покрывает запросы
- **Dynamic client registration** — не нужен client_secret, OpenAI регистрирует client_id в момент login
- **OAuth 2.0 + PKCE** — стандартный flow, безопасно для desktop/wasm
- **Resource = `https://api.openai.com/v1`** — access_token работает с OpenAI Responses API
- **Scope `chatgpt.tokens.use.direct`** — даёт доступ к модели через подписку пользователя
- **Jev-совместимость** — System One protocol (`typesafe/jev-1.13`) доступен через тот же OpenAI API

---

## 3. Архитектура

### 3.1. Слои

```
┌─────────────────────────────────────────────────────────────┐
│  UI (canvas-app / canvas-web)                               │
│  - Suggest dropdown (FR-079)                                │
│  - Graph generator dialog (.byok)                           │
│  - Agent panel (LLM-driven tool calling)                    │
└────────────────────────┬────────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  canvas-llm (НОВЫЙ крейт)                                   │
│  - LlmProvider trait (chat, choice, tool_calling, embed)   │
│  - Провайдеры (6):                                          │
│    A. OpenAiCompatible (OpenRouter, OpenAI, Ollama)         │
│    B. AnthropicClaude (claude-3.5/4, /v1/messages)          │
│    C. ZaiGlm (glm-5.3-flash, OpenAI-compat)                 │
│    D. MoonshotKimi (kimi-k3, OpenAI-compat)                 │
│    E. ChatGptOAuth (Sign-in-with-ChatGPT)                   │
│    F. LayaLegacy (sidecar, back-compat)                     │
│  - ProtocolAdapter: нормализует API-различия в единый trait │
│  - Settings: какой провайдер, модель, endpoint              │
│  - Валидация ключей / токенов                               │
└────────────────────────┬────────────────────────────────────┘
                         │
         ┌───────────────┼───────────────┐
         │               │               │
┌────────▼──────┐ ┌──────▼──────┐ ┌──────▼──────────────────┐
│ canvas-suggest│ │ canvas-mcp  │ canvas-graph-builder     │
│ (FR-079 mm)   │ │ (42 tools)  │ (НОВЫЙ: текст → граф)    │
│               │ │             │                          │
│ LLM как       │ │ LLM решает  │ LLM генерирует           │
│ mm-source     │ │ какой tool  │ .byok-JSON через         │
│ (fusion α)    │ │ вызвать     │ chat / tool_calling      │
└───────────────┘ └─────────────┘ └──────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────────┐
│  Транспорт (разделяет ADR-0010 чистоту MCP-stdio)           │
│                                                             │
│  Desktop: canvas-llm → напрямую HTTP (ureq) к провайдеру    │
│           ключ в OS keychain (Windows Credential Manager)   │
│                                                             │
│  Web (wasm): canvas-llm → fetch к cloud-function proxy      │
│              ключ НЕ в wasm-bundle; proxy держит ключ       │
│                                                             │
│  MCP-мост (canvas-mcp): НЕ ТРОГАТЬ — остаётся stdio,        │
│  чистый JSON-RPC, без LLM-вызовов (ADR-0010)                │
└─────────────────────────────────────────────────────────────┘
```

### 3.2. Ключевое разделение: LLM-транспорт ≠ MCP-транспорт

**ADR-0010 (чистота MCP-stdio):** MCP-мост между CanvasDesk и внешним агентом (Claude Desktop) содержит ТОЛЬКО JSON-RPC. LLM-вызовы туда не добавляются.

**LLM-вызовы идут отдельным транспортом:**
- Desktop: `canvas-llm` напрямую HTTP к OpenRouter/OpenAI/Ollama
- Web: `canvas-llm` через fetch к cloud-function proxy (CORS + key-hiding)

MCP-инструменты остаются локальными — LLM только решает «что вызвать», вызов идёт через `graph_apply` (батч) или отдельные tool-call'ы внутри приложения.

### 3.3. Три AI-сценария

#### Сценарий 1: Suggest (автодополнение шаблонов)

**Где:** `canvas-suggest/` (FR-079), mm-source в fusion

**Как:**
- `canvas-suggest/src/llm/` (новый модуль, feature `l1-llm`) — LLM-клиент как mm-source
- Реализует `SuggestEngine` trait (уже есть в `types.rs`)
- Возвращает `MmAnswer { probs, confidence }` (тот же интерфейс что Laya)
- Fusion `α·lex + (1−α)·llm` без изменений
- Prompt template из `llm-mm-source-benchmark.md` (готов, проверен на 15 фикстурах)

**Провайдеры:**
- OpenRouter: `z-ai/glm-5.3-flash` (p@1=1.0, $0.121/1000) или `nemotron-3-super:free` (p@1=0.8, $0)
- Sign-in-with-ChatGPT: GPT-4o-mini через подписку (без ключа)
- Laya legacy: sidecar (offline fallback)

**Fallback:** ошибка/таймаут LLM → `fusion(lex, ∅) = lex` (как у Laya)

#### Сценарий 2: Генерация графов (.byok)

**Где:** `canvas-graph-builder/` (новый крейт), виджет `byok-generator`

**Как:**
- LLM получает `.byok`-вход (текст + режим: mindmap/outline/summary)
- Возвращает JSON с нодами + рёбрами (формат `.canvas`)
- Валидация: `graph_validate` (MCP-инструмент) перед вставкой
- Вставка: `graph_apply` (батч до 256 операций)

**Промпт:**
```
SYSTEM: Ты — генератор графов для CanvasDesk. Верни JSON:
        {"nodes":[{"id","label","type","content"}],
         "edges":[{"from","to","label"}]}
USER:   Режим: {mindmap|outline|summary}
        Текст: {source.text}
        Параметры: {params}
```

**Tool-calling альтернатива:** LLM может вызвать `node_create_note` / `edge_create` напрямую через MCP-tool-calling (вместо генерации JSON). Это безопаснее (валидация на лету), но медленнее (N round-trips).

#### Сценарий 3: Agentные операции (LLM → MCP tool-calling)

**Где:** `canvas-app/src/agent_panel.rs` (новый модуль)

**Как:**
- UI: чат-панель, пользователь пишет «создай ноду CAC и свяжи с LTV»
- LLM получает список MCP-инструментов (42 шт, из `tools/list`)
- LLM решает: вызвать `node_create_note(label="CAC")` → `node_create_note(label="LTV")` → `edge_create(from, to)`
- Вызовы идут через `graph_apply` (батч) — один round-trip к MCP
- Результат: применён к канвасу, показан в UI

**Safety:**
- `graph_validate` перед apply
- Undo-шаг (FR-006) на каждую agent-операцию
- Подтверждение пользователя для деструктивных ops (`node_delete`)

---

## 4. Sign-in-with-ChatGPT — анализ интеграции

### 4.1. Что это

OAuth 2.0 + PKCE flow от OpenAI. Пользователь логинится в ChatGPT → приложение получает access_token → использует OpenAI Responses API через подписку пользователя (без API-ключа).

### 4.2. Flow (из cookbook)

```
1. App → browser: open https://chatgpt.com/auth/login
   params: client_id=dynamic_agent_client (динамическая регистрация!)
           agent_name_hint=CanvasDesk
           ext_agent_host_id=<persistent device id>
           PKCE code_verifier + state + nonce
           scope: openid profile email offline_access
           resource: https://api.openai.com/v1
           (resource.invoke: chatgpt.tokens.use.direct)

2. User → ChatGPT: login + select workspace + approve permissions

3. OpenAI → app (redirect to localhost listener):
   callback: code, state

4. App → OpenAI token endpoint:
   POST /oauth/token
   grant_type=authorization_code
   client_id=<issued by OpenAI in step 2>
   code=<from callback>
   code_verifier=<PKCE>
   redirect_uri=http://127.0.0.1:PORT/callback
   resource=https://api.openai.com/v1

5. OpenAI → app:
   {access_token, refresh_token, id_token}

6. App: verify id_token (JWKS, issuer, audience, nonce)
        save profile (client_id, tokens, scopes)

7. App → OpenAI: use access_token with Responses API
   GET /v1/models  (model discovery)
   POST /v1/responses (streaming inference)
```

### 4.3. Ключевые особенности

| Особенность | Значение для CanvasDesk |
|---|---|
| Dynamic client registration | Не нужен client_secret в коде — OpenAI регистрирует client при первом login |
| `ext_agent_host_id` | Persistent device id (один на установку) — reuse across profiles |
| `resource=https://api.openai.com/v1` | access_token работает с Responses API |
| `chatgpt.tokens.use.direct` scope | Даёт доступ через подписку ChatGPT (Plus/Pro/Team) |
| `offline_access` | refresh_token — долгоживущие сессии |
| id_token verification | JWT: issuer, audience, nonce — нужен JWKS-клиент |
| Model discovery | `GET /v1/models` — список доступных моделей для токена |

### 4.4. Что нужно реализовать

1. **OAuth-клиент** (новый модуль `canvas-llm/src/chatgpt_oauth/`):
   - PKCE generation (code_verifier + code_challenge)
   - Localhost HTTP listener (127.0.0.1:PORT/callback)
   - Browser open (`webbrowser` crate / `window.open` в wasm)
   - Token exchange (ureq / fetch)
   - id_token verification (JWKS, JWT-decode, `jsonwebtoken` crate)

2. **Token storage:**
   - Desktop: OS keychain (Windows Credential Manager, macOS Keychain, Linux Secret Service)
   - Web: OPFS / IndexedDB (encrypted) — НЕ localStorage (XSS-риск)

3. **Refresh flow:**
   - access_token истекает → refresh_token → новый access_token
   - Если refresh_token истёк → re-login (пользователь видит «Continue with ChatGPT» снова)

4. **Model discovery + inference:**
   - `GET /v1/models` — список моделей для подписки пользователя
   - `POST /v1/responses` — streaming inference (Server-Sent Events)

### 4.5. Интеграция с canvas-llm — LlmProvider trait

Единый trait для всех провайдеров. Расширен под 4 класса операций: chat (генерация), choice (ранжирование, для suggest), tool_calling (агентные операции через MCP), embed (векторные представления для catalog embeddings).

```rust
/// Сообщение в едином формате (нормализуется провайдером под свой API).
pub struct Message {
    pub role: Role,         // System | User | Assistant | Tool
    pub content: MessageContent,  // Text | Multimodal { text, images }
    pub tool_call_id: Option<String>,  // для Tool-сообщений (результат вызова)
}

/// Tool-дескриптор (MCP-инструмент, выставляемый LLM).
pub struct ToolDef {
    pub name: String,           // "node_create_note"
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema (как MCP inputSchema)
}

/// Результат tool_calling — LLM решил вызвать инструмент.
pub struct ToolCall {
    pub id: String,             // id вызова (для tool_result в следующем ходе)
    pub name: String,           // имя MCP-инструмента
    pub arguments: serde_json::Value,  // аргументы (JSON)
}

/// Результат choice-ранжирования (тот же интерфейс что Laya MmAnswer).
pub struct ChoiceAnswer {
    pub probs: Vec<(String, f64)>,  // (option_id, probability)
    pub confidence: f64,
}

/// Метаданные модели (для UI выбора).
pub struct ModelInfo {
    pub id: String,             // "z-ai/glm-5.3-flash"
    pub display_name: String,   // "GLM 5.3 Flash"
    pub context_length: usize,
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub pricing: Option<Pricing>,  // None для free/local
}

/// Возможности провайдера (для UI и feature-gating).
pub struct ProviderCaps {
    pub chat: bool,
    pub choice: bool,           // нативный choice-протокол (System One) или chat-fallback
    pub tool_calling: bool,
    pub embed: bool,
    pub streaming: bool,
    pub vision: bool,
}

/// Единый trait для всех LLM-провайдеров.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Идентификатор провайдера (для Settings).
    fn id(&self) -> &str;
    
    /// Человекочитаемое имя (для UI).
    fn display_name(&self) -> &str;
    
    /// Возможности провайдера.
    fn caps(&self) -> ProviderCaps;
    
    /// Список доступных моделей (для UI выбора).
    fn models(&self) -> &[ModelInfo];
    
    /// Активная модель (выбранная пользователем в Settings).
    fn active_model(&self) -> &str;
    
    /// Генерация текста (одно- или multi-turn).
    /// Возвращает текст ответа (без tool_calls — для этого есть tool_calling).
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError>;
    
    /// Choice-ранжирование: документ + опции → вероятности.
    /// Реализация зависит от провайдера:
    /// - System One protocol (Jev, Laya) → /api/alpha/decisions, нативные probs
    /// - OpenAI-compatible → chat с response_format=json_object, parse из текста
    /// - Anthropic → chat с tool_use (forced choice), probs из tool_result
    async fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<ChoiceAnswer, LlmError>;
    
    /// Tool-calling: LLM получает список MCP-инструментов + запрос пользователя,
    /// возвращает список tool_calls (имя + аргументы).
    /// Для agent panel: приложение исполняет tool_calls через graph_apply,
    /// отправляет tool_results обратно, LLM продолжает (multi-turn).
    async fn tool_calling(
        &self,
        messages: &[Message],
        tools: &[ToolDef],
        opts: &ToolCallingOpts,
    ) -> Result<Vec<ToolCall>, LlmError>;
    
    /// Векторное представление текста (для catalog embeddings, semantic search).
    /// Не все провайдеры поддерживают — caps().embed.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError>;
    
    /// Проверка доступности (валидация ключа/токена, ping endpoint).
    async fn health(&self) -> Result<(), LlmError>;
}

/// Опции генерации.
pub struct ChatOpts {
    pub temperature: f32,       // 0.0 для детерминированных (suggest)
    pub max_tokens: Option<usize>,
    pub response_format: ResponseFormat,  // Text | JsonObject
    pub stream: bool,           // streaming (для UI)
}

/// Опции tool-calling.
pub struct ToolCallingOpts {
    pub temperature: f32,
    pub max_tokens: Option<usize>,
    pub tool_choice: ToolChoice,  // Auto | Specific(name) | None
}
```

### 4.6. Провайдеры — реализация trait

Шесть провайдеров, разбитых по 3 группам (по protocol-совместимости):

#### Группа 1: OpenAI-compatible (один адаптер, 4 провайдера)

OpenAI chat/completions API — де-факто стандарт. Реализуется один `OpenAiCompatibleAdapter`, провайдеры отличаются только `base_url` + auth-header + список моделей.

| Провайдер | base_url | Auth | Модели | Особенности |
|---|---|---|---|---|
| **OpenAI** | `https://api.openai.com/v1` | `Authorization: Bearer <key>` | gpt-4o, gpt-4o-mini, o1 | Прямой API, pay-per-token |
| **OpenRouter** | `https://openrouter.ai/api/v1` | `Authorization: Bearer <key>` | 100+ моделей (роутер) | Единый ключ к Claude/Gemini/Qwen/etc; `/api/alpha/decisions` для System One |
| **z.ai (GLM)** | `https://api.z.ai/v1` (или через OpenRouter) | `Authorization: Bearer <key>` | glm-5.3-flash, glm-5.3-flashx | OpenAI-compat; p@1=1.000 на benchmark |
| **Moonshot (Kimi)** | `https://api.moonshot.ai/v1` | `Authorization: Bearer <key>` | kimi-k3, kimi-k2 | OpenAI-compat; `tools` через `function` type; vision input |

Реализация:
```rust
pub struct OpenAiCompatibleProvider {
    base_url: String,
    api_key: String,        // из Settings/keychain
    model: String,          // активная модель
    models: Vec<ModelInfo>, // список (fetch /v1/models или hardcoded)
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError> {
        // POST {base_url}/chat/completions
        // body: {model, messages, temperature, max_tokens, response_format, stream}
        // header: Authorization: Bearer {api_key}
    }
    
    async fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<ChoiceAnswer, LlmError> {
        // Два пути:
        // 1. Если base_url = openrouter → POST /api/alpha/decisions (System One, нативный)
        // 2. Иначе → chat с prompt template из llm-mm-source-benchmark.md, parse JSON
    }
    
    async fn tool_calling(&self, messages: &[Message], tools: &[ToolDef], opts) -> Result<Vec<ToolCall>> {
        // POST {base_url}/chat/completions с tools=[{type:"function",function:{name,description,parameters}}]
        // response.choices[0].message.tool_calls → Vec<ToolCall>
    }
    
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // POST {base_url}/embeddings (не все провайдеры; OpenAI — да, z.ai — да, Moonshot — check)
    }
}
```

#### Группа 2: Anthropic Claude (отдельный адаптер)

Claude использует **отличный от OpenAI** API: `/v1/messages`, `system` отдельно от `messages`, `x-api-key` вместо Bearer, `tool_use` блоки вместо `tool_calls`.

| Аспект | OpenAI | Anthropic |
|---|---|---|
| Endpoint | `/v1/chat/completions` | `/v1/messages` |
| Auth | `Authorization: Bearer <key>` | `x-api-key: <key>` + `anthropic-version: 2023-06-01` |
| System prompt | В `messages` (role=system) | Отдельное поле `system` |
| Tool-calling | `tools` + `tool_calls` в response | `tools` + `tool_use` block в content |
| Choice-ранжирование | `response_format: json_object` + parse | `tool_use` (forced choice) или text + parse |
| Streaming | SSE, `data:` chunks | SSE, event types (`content_block_delta`, etc.) |

Реализация:
```rust
pub struct AnthropicClaudeProvider {
    api_key: String,        // из Settings/keychain
    model: String,          // "claude-3-5-sonnet-20241022" / "claude-4-..."
    models: Vec<ModelInfo>,
}

#[async_trait]
impl LlmProvider for AnthropicClaudeProvider {
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError> {
        // POST https://api.anthropic.com/v1/messages
        // header: x-api-key, anthropic-version: 2023-06-01
        // body: {model, system: <из messages role=system>, messages: <остальные>,
        //        max_tokens, temperature}
        // response.content[0].text (type="text")
    }
    
    async fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<ChoiceAnswer, LlmError> {
        // Два пути:
        // 1. Forced tool_use: создаём tool "rank" с enum = option_ids,
        //    tool_choice = {type:"tool", name:"rank"} → LLM вынужден вызвать
        //    → parse tool_call.arguments.ranking → probs (равномерно от порядка)
        // 2. Text + parse: chat с prompt template, parse JSON из text
    }
    
    async fn tool_calling(&self, messages: &[Message], tools: &[ToolDef], opts) -> Result<Vec<ToolCall>> {
        // POST /v1/messages с tools=[{name, description, input_schema}]
        // response.content → filter type=="tool_use" → Vec<ToolCall>
    }
    
    async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Anthropic не имеет embeddings API → Err(NotSupported)
        // Для embeddings использовать OpenAI/z.ai fallback
    }
}
```

#### Группа 3: ChatGptOAuth + LayaLegacy (специфичные адаптеры)

| Провайдер | Auth | Endpoint | Особенности |
|---|---|---|---|
| **ChatGptOAuth** | OAuth access_token (refresh) | `https://api.openai.com/v1/responses` | Sign-in-with-ChatGPT; model discovery `/v1/models`; streaming через SSE |
| **LayaLegacy** | нет (localhost) | `http://127.0.0.1:8000/v1/systemone` | System One protocol (тот же что jev-1.13); sidecar-процесс; offline |

`ChatGptOAuthProvider` — почти OpenAI-compatible, но:
- Auth через `Authorization: Bearer <oauth_access_token>` (не статичный ключ)
- Endpoint `/v1/responses` (не `/v1/chat/completions`) — Responses API
- Model discovery через `/v1/models` (список зависит от подписки)
- Refresh flow: access_token истёк → refresh_token → новый токен

`LayaLegacyProvider` — переиспользует существующий `canvas-suggest/src/laya/client.rs`:
- Только `choice()` реализован (mm-source для suggest)
- `chat()`, `tool_calling()`, `embed()` → `Err(NotSupported)` (Laya — classifier, не general LLM)
- Offline, без ключа, sidecar

### 4.7. Ограничения Sign-in-with-ChatGPT

1. **Только OpenAI модели** — GPT-4o, GPT-4o-mini, o1. Нет Claude/Gemini/Qwen. Но через BYOK пользователь может добавить любой.
2. **Rate limits подписки** — ChatGPT Plus имеет лимиты (например, 80 msg/3h для GPT-4o). Для suggest (частые запросы) может упереться. Fallback на BYOK при 429.
3. **Preview limitations** — на момент написания cookbook: «Preview limitations», API может меняться.
4. **Desktop-first** — wasm требует cloud-function proxy (redirect_uri должен быть HTTPS, не localhost). Решение: deep-link `canvasdesk://oauth/callback` + cloud function для token exchange.
5. **Embeddings не входят в подписку** — `embed()` через ChatGptOAuth вернёт `Err(NotSupported)`. Для catalog embeddings нужен BYOK с OpenAI/z.ai.

### 4.8. Гибрид с BYOK

Пользователь выбирает в Settings (приоритет сверху вниз — что показывать первым):
- **«Sign in with ChatGPT»** — OAuth, без ключа (Plus/Pro подписка). Только chat/tool_calling, без embed.
- **«OpenAI API key»** — ручной ключ (pay-per-token). Полный caps (chat/choice/tool/embed).
- **«Anthropic API key»** — Claude 3.5/4. chat/tool_calling, без embed.
- **«z.ai API key»** — GLM 5.3 Flash. p@1=1.000 на benchmark. Полный caps.
- **«Moonshot (Kimi) API key»** — Kimi K3. OpenAI-compat, vision input.
- **«OpenRouter API key»** — роутер к 100+ моделям (включая free). Полный caps + System One (jev-1.13).
- **«Ollama (local)»** — localhost:11434, без ключа (desktop only). OpenAI-compat.
- **«Laya sidecar»** — legacy offline (для back-compat). Только choice.

Приоритет при выборе: ChatGPT OAuth > OpenAI direct > Anthropic > z.ai > Moonshot > OpenRouter > Ollama > Laya.

---

## 5. Реализация по этапам

### Этап 1: canvas-llm crate + все BYOK-провайдеры (2-3 сессии)

- Новый крейт `canvas-llm/` (feature-gated, не в default build)
- `LlmProvider` trait (chat, choice, tool_calling, embed) + `ProviderCaps`
- `OpenAiCompatibleProvider` — один адаптер для 4 провайдеров:
  - OpenAI (`api.openai.com/v1`)
  - OpenRouter (`openrouter.ai/api/v1` + `/api/alpha/decisions` для System One)
  - z.ai (`api.z.ai/v1` — GLM 5.3 Flash)
  - Moonshot/Kimi (`api.moonshot.ai/v1` — Kimi K3)
  - Ollama (`localhost:11434/v1` — local)
- `AnthropicClaudeProvider` — отдельный адаптер (`/v1/messages`, `x-api-key`, `tool_use`)
- Settings: `llm_provider` (enum), `llm_api_key`, `llm_model`, `llm_endpoint`
- Storage: OS keychain (desktop), OPFS encrypted (web)
- Health-check для каждого провайдера (валидация ключа)

### Этап 2: Suggest с LLM mm-source (1 сессия)

- `canvas-suggest/src/llm/` — `LlmMmSource` реализует `SuggestEngine`
- Prompt template из benchmark (готов)
- Fusion: `α·lex + (1−α)·llm`, α-пересчёт (LLM точнее → α ниже, ~0.6)
- Fallback: ошибка → lex-only (как Laya)
- Feature `l1-llm` в canvas-suggest (параллельно `l1-laya`)

### Этап 3: Graph builder (2-3 сессии)

- `canvas-graph-builder/` (новый крейт)
- `.byok` → LLM → `.canvas` JSON
- Валидация через `graph_validate`
- Вставка через `graph_apply`
- UI: dialog с режимами (mindmap/outline/summary), preview, accept/reject

### Этап 4: Agent panel (3-4 сессии)

- `canvas-app/src/agent_panel.rs` — чат-UI
- LLM получает `tools/list` (42 MCP-инструмента)
- Tool-calling: LLM → `graph_apply` (батч)
- Undo-шаг на каждую agent-операцию
- Safety: подтверждение деструктивных ops

### Этап 5: Sign-in-with-ChatGPT (2-3 сессии)

- `canvas-llm/src/chatgpt_oauth/` — OAuth-клиент
- PKCE, localhost listener (desktop), deep-link (web)
- Token storage: keychain / OPFS
- id_token verification (JWKS)
- Model discovery + streaming inference
- UI: «Continue with ChatGPT» button в Settings

### Этап 6: Web proxy для wasm (1-2 сессии)

- Cloud function (Cloudflare Workers / Vercel) — держит API-ключи
- `canvas-llm` в wasm → fetch к proxy
- Proxy: валидация CORS, rate-limit per-user, audit log

---

## 6. Риски и митигации

### 6.1. Безопасность

| Риск | Митигация |
|---|---|
| API-ключ в wasm-bundle (утечка) | Web: cloud-function proxy (ключ в env proxy, не в wasm) |
| API-ключ в config.toml (утечка в git) | Desktop: OS keychain, НЕ файл. Settings только хранит provider/model, не ключ |
| OAuth token в localStorage (XSS) | OPFS encrypted + CSP strict |
| LLM-инъекция (prompt injection из канвас-контента) | Валидация output, sandbox tool-calling (graph_validate перед apply) |
| Приватность: контекст нод уходит к LLM | Опциональный local-only режим (Ollama / Laya), явный consent для cloud |

### 6.2. Совместимость

| Риск | Митигация |
|---|---|
| ADR-0010 (MCP-stdio чистота) нарушена | LLM-транспорт отдельный от MCP-моста (§3.2) |
| Golden-тесты suggest ломаются (LLM недетерминирована) | Feature `l1-llm` отключена в CI, проверяется только lex |
| Laya sidecar back-compat | `l1-laya` feature остаётся, выбор провайдера в Settings |
| WASM-сборка без network-deps | `canvas-llm` feature-gated, default build не тянет ureq/jsonwebtoken |

### 6.3. Cost / rate limits

| Риск | Митигация |
|---|---|
| LLM cost при power-use | По benchmark: $0.12/1000 (glm-5.3-flash), $0.07/1000 (jev-1.13). Power-user 3000/мес = $0.36 |
| Rate limits free-tier (OpenRouter) | Fallback на lex при 429; приоритет paid > free |
| ChatGPT Plus rate limits | Model discovery: показывать только доступные; debounce 500мс |
| Cloud-function proxy cost | Cloudflare Workers free-tier 100k requests/день — достаточно для MVP |

### 6.4. UX

| Риск | Митигация |
|---|---|
| LLM latency 10с убивает inline-UX | Prefetch при фокусе ноды; jev-1.13 (0.46с) для suggest; lex мгновенно |
| LLM возвращает невалидный JSON | `response_format: json_object` + валидация + fallback на lex |
| LLM hallucinates template_id | `choice ∈ catalog` проверка, иначе fallback |
| Agent делает нежелательные ops | Undo на каждую операцию, подтверждение деструктивных |

---

## 7. Альтернативы

### 7.1. Только Sign-in-with-ChatGPT (без BYOK)

**Плюс:** ноль конфигурации для пользователя (login → работает).
**Минус:** только OpenAI модели, rate limits подписки, preview API, нет fallback при отсутствии интернета.
**Вердикт:** недостаточно для power-users и enterprise (нужен выбор модели).

### 7.2. Только BYOK (без Sign-in-with-ChatGPT)

**Плюс:** полный контроль, любая модель (OpenRouter 100+).
**Минус:** барьер входа (пользователь берёт API-ключ), cost у пользователя.
**Вердикт:** недостаточно для mass-market (ChatGPT-подписчиков больше, чем API-разработчиков).

### 7.3. Выбранный гибрид (BYOK + Sign-in-with-ChatGPT)

**Плюс:** покрывает оба сегмента, fallback-цепочка, выбор модели (8 провайдеров).
**Минус:** сложность реализации (6 этапов, ~12-16 сессий), два auth-механизма, два protocol-адаптера (OpenAI-compat + Anthropic).
**Вердикт:** оптимально — масс-market через ChatGPT OAuth, power-users через BYOK (любая из 8 моделей: OpenAI/Claude/GLM/Kimi/OpenRouter/Ollama + Laya legacy).

---

## 8. Метрики успеха

| Метрика | Baseline | Цель |
|---|---|---|
| Suggest p@1 | 0.468 (lex+Laya) | ≥0.800 (LLM mm-source) |
| Suggest latency p95 | 800мс (Laya) | ≤2с (jev-1.13) или ≤10с (glm-5.3-flash с prefetch) |
| Graph builder success rate | N/A (не реализовано) | ≥90% валидных графов с первой попытки |
| Agent task completion | N/A | ≥70% задач без ручной правки |
| User setup time | 5 мин (Laya sidecar) | ≤30 сек (ChatGPT OAuth) или ≤2 мин (BYOK ключ) |
| Cost per user/мес | $0 (Laya local) | ≤$0.50 (LLM, гибрид free+paid) |

---

## 9. История

- `2026-10-03` — агент (Super Z): дизайн-документ создан. Анализ Sign-in-with-ChatGPT (cookbook), интеграция с canvas-llm. Три AI-сценария: suggest, graph builder, agent panel. 6 этапов реализации. Гибрид BYOK + OAuth. Статус: proposal для рецензии владельцем.
- `2026-10-03` — агент (Super Z): правка — LlmProvider расширен до 6 провайдеров (3 группы protocol-совместимости). Добавлены Anthropic Claude (`/v1/messages`, `x-api-key`, `tool_use`), z.ai GLM (OpenAI-compat, p@1=1.000 на benchmark), Moonshot Kimi (OpenAI-compat, vision). Trait расширен: `ProviderCaps`, `embed()`, `health()`, `ModelInfo`, `ToolCallingOpts`. Этап 1 — 2-3 сессии (вместо 1-2).

---

## 10. Источники

- `docs/BYOK.md` — существующий формат .byok (статус «предложение»)
- `docs/dev-researches/llm-mm-source-benchmark.md` — benchmark 11 LLM
- `docs/dev-researches/inline-ai-autocomplete-hypothesis.md` — FR-079 гипотеза
- `docs/adr/adr-0009-mcp-transport-compatibility.md` — MCP транспорт
- `docs/adr/adr-0010-mcp-stdio-purity.md` — чистота MCP-stdio
- `crates/canvas-suggest/src/laya/client.rs` — System One protocol (переиспользуется)
- `crates/canvas-mcp/src/lib.rs` — 42 MCP-инструмента
- OpenAI Sign-in-with-ChatGPT cookbook: https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt
- OpenAI OAuth docs: https://platform.openai.com/docs/plugins/authentication
- OpenRouter API: https://openrouter.ai/docs
