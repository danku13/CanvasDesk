//! FR-LLM-A / ADR-0016 §4.5 — типы LLM-слоя (FIXED API, не менять).
//!
//! Все типы, описанные в ADR-0016 и дизайн-документе
//! `docs/dev-researches/byok-chatgpt-oauth-design.md` §4.5, собираются
//! в этом модуле. Трейт `LlmProvider` живёт в `provider.rs`.
//!
//! **Wasm-гейт (ADR-0011):** в дефолтной сборке (без feature `l1-llm`)
//! типы компилируются без serde-деривов — это сохраняет zero-dep
//! дерево. Serde подключается через `cfg_attr(feature = "serde", ...)`,
//! где feature `serde` активируется транзитивно через `l1-llm` (см.
//! `Cargo.toml`).
//!
//! **Сериализуемость:** типы, которые уходят в config.toml (LlmSettings)
//! или сохраняются в JSON-логи (Message, ToolCall), имеют serde-деривы
//! за feature-флагом. Внутренние типы провайдеров (ProviderCaps) тоже
//! сериализуемы — для статуса/телеметрии.

// FR-LLM-A: feature `serde` транзитивно из `l1-llm` (Cargo.toml опциональный
// dep serde). Когда `l1-llm` off — derive не раскрывается, типы остаются
// pure-struct'ами (нужно для wasm-zero-dep).
//
// `LlmError` нужен только за `l1-llm` (для `not_supported()`-хелпера и
// `ChoiceAnswer::best()`-пустого fallback'а); без флага — не импортируем.
#[cfg(feature = "l1-llm")]
use crate::error::LlmError;

/// Минимальное JSON-значение для `ToolDef.input_schema` и
/// `ToolCall.arguments`. Локальный enum нужен, чтобы типы в `types.rs`
/// компилировались без `serde_json` (ADR-0011 wasm-гейт: default build
/// не тянет serde_json). Когда feature `serde` включена, доступна
/// конверсия в/из `serde_json::Value` (см. `JsonVal::from_serde` /
/// `JsonVal::to_serde`), что используют адаптеры `openai_compat.rs` /
/// `anthropic.rs` для сериализации запросов.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum JsonVal {
    Null,
    Bool(bool),
    /// Целое или дробное — единым f64 (как `serde_json::Number::as_f64`).
    Number(f64),
    String(String),
    Array(Vec<JsonVal>),
    /// Объект как `Vec<(String, JsonVal)>` (детерминированный порядок
    /// ключей — важно для golden-тестов и кэш-ключей).
    Object(Vec<(String, JsonVal)>),
}

impl JsonVal {
    /// Конструктор `null`.
    pub const fn null() -> Self {
        Self::Null
    }

    /// Конструктор строки.
    pub fn string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    /// Конструктор объекта из слайса пар (удобно для input_schema).
    pub fn object(pairs: impl IntoIterator<Item = (impl Into<String>, JsonVal)>) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    /// Конвертировать в `serde_json::Value` (только за feature `serde`).
    /// Адаптеры используют для построения тела HTTP-запроса.
    #[cfg(feature = "serde")]
    pub fn to_serde(&self) -> serde_json::Value {
        match self {
            JsonVal::Null => serde_json::Value::Null,
            JsonVal::Bool(b) => serde_json::Value::Bool(*b),
            JsonVal::Number(n) => {
                // serde_json::Number не имеет прямого f64-конструктора без
                // unsafe; используем `from_f64` через `serde_json::Number`
                // (через `json!`-эквивалент — сериализация+десериализация
                // гарантированно работает и не вводит unsafe).
                serde_json::from_str(&n.to_string()).unwrap_or(serde_json::Value::Null)
            }
            JsonVal::String(s) => serde_json::Value::String(s.clone()),
            JsonVal::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(Self::to_serde).collect())
            }
            JsonVal::Object(pairs) => {
                let mut m = serde_json::Map::new();
                for (k, v) in pairs {
                    m.insert(k.clone(), v.to_serde());
                }
                serde_json::Value::Object(m)
            }
        }
    }

    /// Конвертировать из `serde_json::Value` (только за feature `serde`).
    /// Используется при разборе ответов провайдеров.
    #[cfg(feature = "serde")]
    pub fn from_serde(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(*b),
            serde_json::Value::Number(n) => Self::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Self::String(s.clone()),
            serde_json::Value::Array(arr) => {
                Self::Array(arr.iter().map(Self::from_serde).collect())
            }
            serde_json::Value::Object(m) => Self::Object(
                m.iter()
                    .map(|(k, v)| (k.clone(), Self::from_serde(v)))
                    .collect(),
            ),
        }
    }
}

impl From<bool> for JsonVal {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<f64> for JsonVal {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

impl From<i64> for JsonVal {
    fn from(n: i64) -> Self {
        Self::Number(n as f64)
    }
}

impl From<String> for JsonVal {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for JsonVal {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

/// Роль сообщения в диалоге. Нормализуется провайдером под свой API
/// (OpenAI: `system|user|assistant|tool`, Anthropic: `system` отдельно,
/// ChatGPT OAuth: `Responses API` roles).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Содержание сообщения. Multimodal-вариант — для провайдеров с
/// `caps().vision == true` (Moonshot Kimi, GPT-4o, Claude 3.5+).
/// `images` — base64 data URLs (`data:image/png;base64,...`),
/// формат одинаков для всех провайдеров; адаптер конвертирует в
/// нативный формат (OpenAI image_url, Anthropic source.base64).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MessageContent {
    /// Текстовое сообщение.
    Text(String),
    /// Multimodal: текст + до N картинок (data URL).
    Multimodal { text: String, images: Vec<String> },
}

impl MessageContent {
    /// Текстовая часть содержания (для промпт-билдинга в `choice`-fallback).
    pub fn text(&self) -> &str {
        match self {
            MessageContent::Text(t) => t,
            MessageContent::Multimodal { text, .. } => text,
        }
    }
}

/// Сообщение диалога. `tool_call_id` заполнен только для `Role::Tool`
/// (результат вызова инструмента) — парный к `ToolCall.id`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Конструктор пользовательского сообщения (текст).
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(text.into()),
            tool_call_id: None,
        }
    }

    /// Конструктор системного сообщения (текст).
    pub fn system(text: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(text.into()),
            tool_call_id: None,
        }
    }

    /// Конструктор ответа ассистента (текст).
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
            tool_call_id: None,
        }
    }

    /// Конструктор tool-результата (для multi-turn agent panel).
    pub fn tool(tool_call_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::Text(text.into()),
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Дескриптор инструмента (MCP-инструмент, выставляемый LLM).
/// Соответствует `tools/list` из `canvas-mcp` (42 инструмента).
/// `input_schema` — JSON Schema (как `inputSchema` MCP).
///
/// Не `Eq` — `JsonVal::Number(f64)` не реализует `Eq` (NaN).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub input_schema: crate::JsonVal,
}

impl ToolDef {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: crate::JsonVal,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

/// Результат `tool_calling` — LLM решил вызвать инструмент.
/// `id` — для пары с `Message::tool(id, ...)` в следующем ходе.
///
/// Не `Eq` — `arguments: JsonVal` (см. `JsonVal::Number`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: crate::JsonVal,
}

/// Результат `choice`-ранжирования (тот же интерфейс что Laya `MmAnswer`).
/// `probs` отсортированы по убыванию вероятности, при равенстве — по
/// `option_id` по возрастанию (детерминизм, см. `canvas-suggest`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChoiceAnswer {
    pub probs: Vec<(String, f64)>,
    pub confidence: f64,
}

impl ChoiceAnswer {
    /// Создать пустой ответ (zero-shot fallback). `confidence: 0.0`.
    pub fn empty() -> Self {
        Self {
            probs: Vec::new(),
            confidence: 0.0,
        }
    }

    /// Id лучшей опции (max prob), если есть. Используется suggest-воркером
    /// для top-1 метрики (p@1, golden-тесты).
    pub fn best(&self) -> Option<&str> {
        self.probs.first().map(|(id, _)| id.as_str())
    }
}

/// Тариф модели (USD за 1M токенов). `None` для free/local (Ollama, Laya).
/// Источники: открытые прайсы провайдеров (см. benchmark-документ).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pricing {
    /// Цена за 1M входных токенов (USD).
    pub input_per_mtok: f64,
    /// Цена за 1M выходных токенов (USD).
    pub output_per_mtok: f64,
}

impl Pricing {
    /// Free/local модель (Ollama, Laya sidecar) — нулевая цена.
    pub const fn free() -> Self {
        Self {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
        }
    }
}

/// Метаданные модели для UI выбора. Список задаётся в провайдере
/// (hardcoded — см. `openai_compat.rs`, `anthropic.rs`) или
/// загружается через `GET /v1/models` (для ChatGPT OAuth — Stream D).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ModelInfo {
    /// Идентификатор модели (передаётся в `model` поле API).
    /// Пример: `"z-ai/glm-5.3-flash"`, `"claude-3-5-sonnet-20241022"`.
    pub id: String,
    /// Человекочитаемое имя (для dropdown в Settings).
    pub display_name: String,
    /// Размер контекста (токенов).
    pub context_length: usize,
    /// Поддержка tool-calling (`tool_calling()` доступен).
    pub supports_tools: bool,
    /// Поддержка vision-входа (`MessageContent::Multimodal`).
    pub supports_vision: bool,
    /// Тариф (USD/1M токенов). `None` — free/local.
    pub pricing: Option<Pricing>,
}

impl ModelInfo {
    /// Создать free/local модель (Ollama, Laya) без тарифа.
    pub fn free_local(
        id: impl Into<String>,
        display_name: impl Into<String>,
        context_length: usize,
        supports_tools: bool,
        supports_vision: bool,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            context_length,
            supports_tools,
            supports_vision,
            pricing: None,
        }
    }
}

/// Возможности провайдера (для UI и feature-gating).
/// Влияет на то, какие операции доступны в Settings
/// (например, `embed: false` у Anthropic — кнопка «Embeddings»
/// недоступна, нужно переключиться на OpenAI/z.ai).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProviderCaps {
    /// `chat()` — генерация текста.
    pub chat: bool,
    /// `choice()` — нативный choice-протокол (System One) или chat-fallback.
    pub choice: bool,
    /// `tool_calling()` — agent panel.
    pub tool_calling: bool,
    /// `embed()` — векторные представления (catalog embeddings).
    pub embed: bool,
    /// Streaming (SSE). В Stream A не реализовано (заглушки), Stream D
    /// добавит для ChatGPT OAuth (`/v1/responses`).
    pub streaming: bool,
    /// Multimodal-вход (`MessageContent::Multimodal`).
    pub vision: bool,
}

/// Формат ответа для `chat()`. `JsonObject` эквивалентен
/// `response_format: { type: "json_object" }` (OpenAI-compat) или
/// forced-tool (Anthropic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ResponseFormat {
    /// Свободный текст (по умолчанию).
    #[default]
    Text,
    /// JSON-объект (для `choice`-fallback через chat).
    JsonObject,
}

/// Выбор инструмента для `tool_calling`. `Auto` — LLM сам решает,
/// `Specific(name)` — forced (используется для `choice` у Anthropic),
/// `None` — отключить tool-calling в этом вызове.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ToolChoice {
    /// LLM сама решает, вызывать ли инструмент (default).
    #[default]
    Auto,
    /// Forced: LLM обязана вызвать конкретный инструмент.
    /// Используется для `choice` у Anthropic (tool "rank").
    Specific(String),
    /// Без tool-calling в этом вызове.
    None,
}

/// Опции генерации `chat()`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ChatOpts {
    /// 0.0 — детерминированный (suggest choice), 0.7–1.0 — креативный.
    pub temperature: f32,
    /// Лимит выходных токенов. `None` — default провайдера.
    pub max_tokens: Option<usize>,
    /// Формат ответа (текст / JSON).
    pub response_format: ResponseFormat,
    /// Streaming (SSE). В Stream A игнорируется адаптерами (stream:false),
    /// резервируется под Stream D (ChatGPT OAuth Responses API).
    pub stream: bool,
}

impl Default for ChatOpts {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: None,
            response_format: ResponseFormat::Text,
            stream: false,
        }
    }
}

impl ChatOpts {
    /// Детерминированный текстовый ответ (suggest choice, validation).
    pub fn deterministic() -> Self {
        Self {
            temperature: 0.0,
            max_tokens: None,
            response_format: ResponseFormat::Text,
            stream: false,
        }
    }

    /// JSON-ответ (для `choice`-fallback через chat).
    pub fn json() -> Self {
        Self {
            temperature: 0.0,
            max_tokens: None,
            response_format: ResponseFormat::JsonObject,
            stream: false,
        }
    }
}

/// Опции `tool_calling()`. Не `Copy` — `ToolChoice::Specific(String)`
/// владеет строкой (форсированное имя инструмента).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ToolCallingOpts {
    pub temperature: f32,
    pub max_tokens: Option<usize>,
    pub tool_choice: ToolChoice,
}

impl Default for ToolCallingOpts {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            max_tokens: None,
            tool_choice: ToolChoice::Auto,
        }
    }
}

/// Оценка стоимости запроса (Q4 — cost visibility). Считается до
/// (`estimate_cost`) и после (`actual_cost`) запроса. До — для dialog
/// preview (graph builder, agent panel), после — в статусную панель
/// (Stream B `ai_status_panel.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CostEstimate {
    /// Оценка входных токенов (по `estimate_tokens`).
    pub estimated_input_tokens: usize,
    /// Оценка выходных токенов (по `max_output_tokens`).
    pub estimated_output_tokens: usize,
    /// Оценка стоимости в USD. 0.0 для free/local.
    pub estimated_cost_usd: f64,
}

impl CostEstimate {
    /// Free/local запрос (Ollama, Laya).
    pub const fn free(input_tokens: usize, output_tokens: usize) -> Self {
        Self {
            estimated_input_tokens: input_tokens,
            estimated_output_tokens: output_tokens,
            estimated_cost_usd: 0.0,
        }
    }
}

/// Описание опции для `choice()`. Re-export из `canvas-suggest::OptionDesc`
/// невозможен (cyclic-dep: `canvas-suggest` будет зависеть от `canvas-llm`
/// для LlmMmSource, см. Stream C). Поэтому тип дублируется локально;
/// структура идентична `canvas_suggest::types::OptionDesc` (id + desc).
///
/// Конверсия: приложение/воркер делает `OptionDesc { id, desc }` →
/// `canvas_llm::OptionDesc { id, desc }` тривиально (одинаковые поля).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OptionDesc {
    pub id: String,
    pub desc: String,
}

impl OptionDesc {
    pub fn new(id: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            desc: desc.into(),
        }
    }
}

/// Convenience-конструктор ошибки «операция не поддерживается» —
/// используется адаптерами (Anthropic `embed()`, Laya legacy `chat()`).
///
/// В текущей реализации адаптеры вызывают `LlmError::NotSupported("...")`
/// напрямую. Хелпер оставлен для будущих расширений (например, Laya legacy
/// в Stream C). Помечен `#[allow(dead_code)]` чтобы не warn'ил.
#[cfg(feature = "l1-llm")]
#[allow(dead_code)]
pub(crate) fn not_supported(op: &'static str) -> LlmError {
    LlmError::NotSupported(op)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_constructors() {
        let u = Message::user("hi");
        assert_eq!(u.role, Role::User);
        assert_eq!(u.content.text(), "hi");
        assert!(u.tool_call_id.is_none());

        let s = Message::system("rules");
        assert_eq!(s.role, Role::System);

        let a = Message::assistant("ok");
        assert_eq!(a.role, Role::Assistant);

        let t = Message::tool("call_1", "result");
        assert_eq!(t.role, Role::Tool);
        assert_eq!(t.tool_call_id.as_deref(), Some("call_1"));
    }

    #[test]
    fn message_content_text_helper() {
        let t = MessageContent::Text("hello".into());
        assert_eq!(t.text(), "hello");

        let m = MessageContent::Multimodal {
            text: "see image".into(),
            images: vec!["data:image/png;base64,...".into()],
        };
        assert_eq!(m.text(), "see image");
    }

    #[test]
    fn choice_answer_best() {
        let a = ChoiceAnswer {
            probs: vec![("a".into(), 0.5), ("b".into(), 0.3)],
            confidence: 0.5,
        };
        assert_eq!(a.best(), Some("a"));

        let empty = ChoiceAnswer::empty();
        assert!(empty.best().is_none());
        assert_eq!(empty.confidence, 0.0);
    }

    #[test]
    fn chat_opts_defaults() {
        let d = ChatOpts::default();
        assert_eq!(d.temperature, 0.7);
        assert!(d.max_tokens.is_none());
        assert_eq!(d.response_format, ResponseFormat::Text);
        assert!(!d.stream);

        let det = ChatOpts::deterministic();
        assert_eq!(det.temperature, 0.0);

        let js = ChatOpts::json();
        assert_eq!(js.response_format, ResponseFormat::JsonObject);
    }

    #[test]
    fn tool_choice_default() {
        assert_eq!(ToolChoice::default(), ToolChoice::Auto);
    }

    #[test]
    fn pricing_free() {
        let p = Pricing::free();
        assert_eq!(p.input_per_mtok, 0.0);
        assert_eq!(p.output_per_mtok, 0.0);
    }

    #[test]
    fn model_info_free_local() {
        let m = ModelInfo::free_local("ollama/llama3", "Llama 3 (Ollama)", 8192, true, false);
        assert!(m.pricing.is_none());
        assert!(m.supports_tools);
        assert!(!m.supports_vision);
    }

    #[test]
    fn cost_estimate_free() {
        let c = CostEstimate::free(100, 50);
        assert_eq!(c.estimated_input_tokens, 100);
        assert_eq!(c.estimated_output_tokens, 50);
        assert_eq!(c.estimated_cost_usd, 0.0);
    }

    #[test]
    fn provider_caps_default_all_false() {
        let c = ProviderCaps::default();
        assert!(!c.chat);
        assert!(!c.choice);
        assert!(!c.tool_calling);
        assert!(!c.embed);
        assert!(!c.streaming);
        assert!(!c.vision);
    }

    #[test]
    fn json_val_constructors() {
        assert_eq!(JsonVal::null(), JsonVal::Null);
        assert_eq!(JsonVal::string("x"), JsonVal::String("x".into()));
        assert_eq!(
            JsonVal::object([("a", JsonVal::Bool(true)), ("b", JsonVal::Null)]),
            JsonVal::Object(vec![
                ("a".into(), JsonVal::Bool(true)),
                ("b".into(), JsonVal::Null)
            ])
        );
    }

    #[test]
    fn json_val_from_conversions() {
        let b: JsonVal = true.into();
        assert_eq!(b, JsonVal::Bool(true));
        let n: JsonVal = 3.5f64.into();
        assert_eq!(n, JsonVal::Number(3.5));
        let i: JsonVal = 42i64.into();
        assert_eq!(i, JsonVal::Number(42.0));
        let s: JsonVal = "hi".into();
        assert_eq!(s, JsonVal::String("hi".into()));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_val_serde_roundtrip() {
        let v = JsonVal::Object(vec![
            ("name".into(), JsonVal::String("rank".into())),
            ("limit".into(), JsonVal::Number(5.0)),
            (
                "opts".into(),
                JsonVal::Array(vec![JsonVal::Bool(true), JsonVal::Null]),
            ),
        ]);
        let s = v.to_serde();
        assert_eq!(s["name"], "rank");
        assert_eq!(s["limit"], 5);
        assert_eq!(s["opts"][0], true);
        assert!(s["opts"][1].is_null());
        // Обратная конверсия: ключи serde_json::Map по умолчанию
        // сортируются (BTreeMap). Сравниваем поле-за-полем, а не
        // побитово (порядок ключей не детерминирован без `preserve_order`).
        let back = JsonVal::from_serde(&s);
        let back_obj = match back {
            JsonVal::Object(pairs) => pairs,
            _ => panic!("expected Object"),
        };
        let find = |key: &str| -> JsonVal {
            back_obj
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| panic!("missing key {key}"))
        };
        assert_eq!(find("name"), JsonVal::String("rank".into()));
        assert_eq!(find("limit"), JsonVal::Number(5.0));
        assert_eq!(
            find("opts"),
            JsonVal::Array(vec![JsonVal::Bool(true), JsonVal::Null])
        );
    }
}
