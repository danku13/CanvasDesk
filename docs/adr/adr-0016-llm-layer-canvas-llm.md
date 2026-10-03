# ADR-0016: LLM-слой canvas-llm — унифицированный trait для 6 провайдеров (BYOK + Sign-in-with-ChatGPT)

- **Статус:** предложено
- **Дата:** 2026-10-03
- **Участники:** владелец (запрос BYOK + Sign-in-with-ChatGPT), агент (Super Z, дизайн-документ `docs/dev-researches/byok-chatgpt-oauth-design.md`)
- **Связанные:** ADR-0009 (MCP-транспорт), ADR-0010 (чистота MCP-stdio), ADR-0011 (wasm-гейт), FR-079 (suggest), `docs/BYOK.md` (формат .byok), `docs/dev-researches/llm-mm-source-benchmark.md` (benchmark 11 LLM), PRD-0010 (LLM-интеграция)

## Контекст

CanvasDesk имеет MCP-сервер с 42 инструментами (`crates/canvas-mcp/src/lib.rs`) и suggest-движок FR-079 (`crates/canvas-suggest/`, гибрид lex+Laya, p@1=0.468). Benchmark 11 LLM (`docs/dev-researches/llm-mm-source-benchmark.md`) показал: LLM превосходит baseline на +57–114% (p@1 0.733–1.000), но интеграция не реализована. Существующий `docs/BYOK.md` описывает формат `.byok` для генерации графов, но статус «предложение, не реализовано».

Владелец запросил BYOK для трёх AI-сценариев: (1) suggest/автодополнение, (2) генерация графов из текста, (3) агентные операции через MCP. Плюс — интеграцию Sign-in-with-ChatGPT (OAuth, без API-ключа пользователя).

Ограничения:
- **ADR-0010 (чистота MCP-stdio):** MCP-мост между CanvasDesk и внешним агентом содержит только JSON-RPC. LLM-вызоры туда не добавляются.
- **ADR-0011 (wasm-гейт):** ядро CanvasDesk (`canvas-core`, `canvas-scene`) компилируется под wasm32 без I/O и платформенного. LLM-клиент должен быть feature-gated, не в default build.
- **Wasm-сборка не может держать API-ключи:** public bundle = утечка. Нужен backend-proxy для web.
- **Существующий Laya-клиент** (`canvas-suggest/src/laya/client.rs`) реализует System One protocol (`/v1/systemone`) — готовая инфраструктура для Jev-совместимых моделей.

## Драйверы решения (требования)

1. **Единый trait** — все LLM-провайдеры за одним `LlmProvider` trait, потребители (suggest, graph-builder, agent-panel) не знают какой провайдер активен.
2. **6 провайдеров:** OpenAI, OpenRouter, Anthropic Claude, z.ai GLM, Moonshot Kimi, ChatGptOAuth (+ Laya legacy + Ollama как special-case).
3. **4 операции:** chat (генерация), choice (ранжирование для suggest), tool_calling (агентные операции через MCP), embed (векторные представления для catalog embeddings).
4. **ADR-0010 чистота MCP:** LLM-транспорт отделён от MCP-моста. LLM-вызоры через HTTP/fetch-proxy, MCP-инструменты вызываются локально через `graph_apply`.
5. **Wasm-совместимость:** `canvas-llm` feature-gated, default build не тянет network-deps. Web: cloud-function proxy для API-ключей.
6. **Backward-compat:** Laya sidecar остаётся как legacy offline fallback. Существующие golden-тесты suggest (`golden_fusion.rs`) не ломаются — LLM-mm отключена в CI.
7. **Безопасность:** API-ключи в OS keychain (desktop) / OPFS encrypted (web), НЕ в config.toml. OAuth tokens НЕ в localStorage (XSS).
8. **Cost-control:** fallback на lex при ошибке/таймауте LLM. Дебаунс 500мс для inline-UX. Health-check провайдера.

## Рассмотренные варианты

### Вариант A: Только Sign-in-with-ChatGPT (без BYOK)

**Описание:** Единственный auth-путь — OAuth через ChatGPT. Пользователь логинится, подписка покрывает запросы.

**Плюсы:** ноль конфигурации, mass-market friendly, нет проблемы утёкших ключей.
**Минусы:** только OpenAI модели (GPT-4o/o1), нет Claude/GLM/Kimi; rate limits подписки (80 msg/3h GPT-4o); preview API; нет offline; нет embeddings (не входит в подписку).
**Вердикт:** недостаточно для power-users и enterprise (нужен выбор модели и offline).

### Вариант B: Только BYOK (без Sign-in-with-ChatGPT)

**Описание:** Пользователь сам берёт API-ключ у любого провайдера, вставляет в Settings.

**Плюсы:** полный контроль, любая модель, offline (Ollama/Laya).
**Минусы:** барьер входа (надо брать ключ), cost у пользователя, нет «магии» OAuth.
**Вердикт:** недостаточно для mass-market (ChatGPT-подписчиков больше, чем API-разработчиков).

### Вариант C: Прямые провайдеры без trait-абстракции

**Описание:** Каждый провайдер — отдельный модуль, потребитель сам выбирает и вызывает нужный.

**Плюсы:** простой старт, нет абстракции.
**Минусы:** consumer-side сложность (suggest/graph-builder/agent-panel каждый знает про все провайдеры); дублирование; невозможно переключать провайдера без перекомпиляции.
**Вердикт:** не масштабируется — 3 consumer × 6 провайдеров = 18 точек интеграции.

### Вариант D: Выбранный — canvas-llm crate с LlmProvider trait (гибрид A+B+абстракция)

**Описание:** Новый feature-gated крейт `canvas-llm/` с `LlmProvider` trait. 6 провайдеров в 3 protocol-группах. Потребители работают только с trait, не зная провайдера. LLM-транспорт отделён от MCP-моста (ADR-0010). ChatGPT OAuth + BYOK共存.

**Плюсы:** единая точка интеграции (3 consumer × 1 trait = 3 точки); switch провайдера в runtime (Settings); fallback-цепочка; ADR-0010 сохранён.
**Минусы:** сложность trait (4 операции × 6 провайдеров); два protocol-адаптера (OpenAI-compat + Anthropic); OAuth-инфра (PKCE, JWKS, refresh); cloud-proxy для wasm.
**Вердикт:** оптимально — покрывает оба сегмента (mass-market OAuth + power-user BYOK), масштабируется, ADR-0010 сохранён.

## Решение

**Принять Вариант D** — `canvas-llm` crate с `LlmProvider` trait, 6 провайдеров, гибрид BYOK + Sign-in-with-ChatGPT.

### LlmProvider trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn caps(&self) -> ProviderCaps;  // {chat, choice, tool_calling, embed, streaming, vision}
    fn models(&self) -> &[ModelInfo];
    fn active_model(&self) -> &str;
    
    async fn chat(&self, messages: &[Message], opts: &ChatOpts) -> Result<String, LlmError>;
    async fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<ChoiceAnswer, LlmError>;
    async fn tool_calling(&self, messages: &[Message], tools: &[ToolDef], opts: &ToolCallingOpts) -> Result<Vec<ToolCall>, LlmError>;
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, LlmError>;
    async fn health(&self) -> Result<(), LlmError>;
}
```

### 3 protocol-группы провайдеров

**Группа 1: OpenAI-compatible** (один `OpenAiCompatibleProvider`, 5 endpoint'ов):
- OpenAI (`api.openai.com/v1`), OpenRouter (`openrouter.ai/api/v1` + `/api/alpha/decisions`), z.ai GLM (`api.z.ai/v1`), Moonshot Kimi (`api.moonshot.ai/v1`), Ollama (`localhost:11434/v1`)

**Группа 2: Anthropic Claude** (отдельный `AnthropicClaudeProvider`):
- `/v1/messages`, `x-api-key` + `anthropic-version`, `system` отдельно, `tool_use` blocks, forced tool_choice для choice

**Группа 3: Специфичные:**
- `ChatGptOAuthProvider` — `/v1/responses` (Responses API), OAuth access_token + refresh, model discovery
- `LayaLegacyProvider` — `/v1/systemone` (System One, переиспользует `canvas-suggest/src/laya/client.rs`), только `choice()`, offline

### Транспорт (ADR-0010 сохранён)

- **Desktop:** `canvas-llm` → напрямую HTTP (ureq) к провайдеру. Ключ в OS keychain.
- **Web (wasm):** `canvas-llm` → fetch к cloud-function proxy. Ключ в env proxy, НЕ в wasm-bundle.
- **MCP-мост:** НЕ ТРОГАТЬ — остаётся stdio, чистый JSON-RPC. LLM только решает «какой tool вызвать», вызов идёт через `graph_apply` локально.

### 3 AI-сценария (consumer'ы trait)

1. **Suggest** (`canvas-suggest/src/llm/`) — LLM как mm-source в fusion `α·lex + (1−α)·llm`. `choice()` операция. Fallback на lex при ошибке.
2. **Graph builder** (`canvas-graph-builder/`, новый) — `.byok` текст → LLM → `.canvas` JSON (nodes + edges). `chat()` или `tool_calling()`. Валидация через `graph_validate`, вставка через `graph_apply`.
3. **Agent panel** (`canvas-app/src/agent_panel.rs`, новый) — LLM получает `tools/list` (42 MCP-инструмента), `tool_calling()` возвращает tool_calls, приложение исполняет через `graph_apply`, multi-turn.

### Feature-gating

- `canvas-llm` — feature-gated, default build не тянет network-deps (ureq, jsonwebtoken)
- `l1-llm` в canvas-suggest (параллельно `l1-laya`)
- Wasm-сборка: `canvas-llm` использует `web-sys::fetch` (не ureq), cloud-proxy обязателен
- CI: LLM-mm отключена, golden-тесты проверяют только lex

## Последствия

### Положительные

- Единая точка интеграции для 3 consumer'ов (suggest, graph-builder, agent-panel)
- 8 провайдеров в Settings (ChatGPT OAuth + OpenAI + Anthropic + z.ai + Moonshot + OpenRouter + Ollama + Laya)
- Switch провайдера в runtime без перекомпиляции
- Fallback-цепочка: ChatGPT OAuth → BYOK → lex
- ADR-0010 (MCP-stdio чистота) сохранён — LLM-транспорт отдельный
- Benchmark показал +114% к baseline (p@1 0.468 → 1.000 с glm-5.3-flash)

### Отрицательные

- Сложность trait (4 операции × 6 провайдеров = 24 метода-реализации)
- Два protocol-адаптера (OpenAI-compat + Anthropic) — разные endpoint/auth/tool-format
- OAuth-инфра: PKCE, JWKS, refresh, localhost listener (desktop) / deep-link (web)
- Cloud-function proxy для wasm (отдельная инфра, cost)
- ~12-16 сессий реализации (6 этапов)
- Недетерминизм LLM ломает golden-тесты — нужен feature-gating в CI

### Нейтральные

- Laya sidecar остаётся как legacy (не удаляется, back-compat)
- `docs/BYOK.md` (формат .byok) обновляется — становится спецификацией graph-builder input

## Валидация

- Дизайн-документ: `docs/dev-researches/byok-chatgpt-oauth-design.md` (10 разделов, полный trait + провайдеры)
- Benchmark: `docs/dev-researches/llm-mm-source-benchmark.md` (11 моделей, p@1 до 1.000)
- Sign-in-with-ChatGPT: https://developers.openai.com/cookbook/articles/sign-in-with-chatgpt (OAuth 2.0 + PKCE, dynamic client registration)
- Существующий Laya-клиент: `crates/canvas-suggest/src/laya/client.rs` (System One protocol готов)
- MCP-инструменты: `crates/canvas-mcp/src/lib.rs` (42 инструмента, `tools/list`)

## История

- `2026-10-03` — агент (Super Z): ADR создан. Вариант D (canvas-llm crate с LlmProvider trait). 6 провайдеров, 3 protocol-группы, гибрид BYOK + Sign-in-with-ChatGPT. Статус: предложено.
