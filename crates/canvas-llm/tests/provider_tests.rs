//! FR-LLM-A — интеграционные тесты провайдеров (без сети).
//!
//! Проверяют конструирование preset'ов, метаданные (id/display_name/caps),
//! списки моделей и валидацию `set_active_model`. Без сети — только
//! структурная валидация (HTTP-запросы тестируются в Stream B/C/D через
//! mock-сервер или integration-тесты).
//!
//! Тесты требуют feature `l1-llm` (адаптеры `OpenAiCompatibleProvider` /
//! `AnthropicClaudeProvider` доступны только за флаг).

#![cfg(feature = "l1-llm")]

use canvas_llm::types::{JsonVal, Pricing, ProviderCaps};
use canvas_llm::{AnthropicClaudeProvider, LlmError, LlmProvider, OpenAiCompatibleProvider};

// ============================================================================
// OpenAiCompatibleProvider — 5 preset'ов
// ============================================================================

#[test]
fn openai_preset_metadata() {
    let p = OpenAiCompatibleProvider::openai("sk-test", "gpt-4o-mini");
    assert_eq!(p.id(), "openai");
    assert_eq!(p.display_name(), "OpenAI");
    assert_eq!(p.active_model(), "gpt-4o-mini");
    assert!(p.base_url().ends_with("api.openai.com/v1"));
    assert!(!p.is_openrouter());
}

#[test]
fn openai_preset_caps() {
    let p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
    let caps = p.caps();
    assert!(caps.chat);
    assert!(caps.choice);
    assert!(caps.tool_calling);
    assert!(caps.embed);
    assert!(!caps.streaming); // Stream A не реализует streaming
    assert!(caps.vision); // GPT-4o поддерживает vision
}

#[test]
fn openai_preset_models_with_pricing() {
    let p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
    assert!(p.models().len() >= 3);
    // Все OpenAI модели платные (pricing = Some).
    assert!(p.models().iter().all(|m| m.pricing.is_some()));
    // Контексты большие (128k для GPT-4o).
    assert!(p.models().iter().all(|m| m.context_length >= 100_000));
}

#[test]
fn openrouter_preset_metadata() {
    let p = OpenAiCompatibleProvider::openrouter("sk-or-test", "z-ai/glm-5.3-flash");
    assert_eq!(p.id(), "openrouter");
    assert_eq!(p.display_name(), "OpenRouter");
    assert!(p.is_openrouter()); // важно для choice через /api/alpha/decisions
    assert!(p.base_url().ends_with("openrouter.ai/api/v1"));
}

#[test]
fn openrouter_preset_has_free_nemotron() {
    let p = OpenAiCompatibleProvider::openrouter("sk", "nemotron/nemotron-super:free");
    let has_free = p.models().iter().any(|m| m.pricing.is_none());
    assert!(
        has_free,
        "OpenRouter должен включать free-tier (nemotron-super:free)"
    );
}

#[test]
fn openrouter_preset_models_diverse() {
    let p = OpenAiCompatibleProvider::openrouter("sk", "z-ai/glm-5.3-flash");
    // Должны быть модели от разных провайдеров (z.ai, Anthropic, Moonshot).
    let has_zai = p.models().iter().any(|m| m.id.starts_with("z-ai/"));
    let has_anthropic = p.models().iter().any(|m| m.id.starts_with("anthropic/"));
    let has_moonshot = p.models().iter().any(|m| m.id.starts_with("moonshotai/"));
    assert!(
        has_zai,
        "OpenRouter должен включать z.ai модели (p@1=1.000 на benchmark)"
    );
    assert!(has_anthropic);
    assert!(has_moonshot);
}

#[test]
fn zai_preset_metadata() {
    let p = OpenAiCompatibleProvider::zai("sk-zai", "glm-5.3-flash");
    assert_eq!(p.id(), "zai");
    assert_eq!(p.display_name(), "z.ai (GLM)");
    assert_eq!(p.active_model(), "glm-5.3-flash");
    assert!(p.base_url().ends_with("api.z.ai/v1"));
    assert!(!p.is_openrouter());
}

#[test]
fn zai_preset_models_glm_flash() {
    let p = OpenAiCompatibleProvider::zai("sk", "glm-5.3-flash");
    assert!(p.models().iter().any(|m| m.id == "glm-5.3-flash"));
    // Все модели z.ai с pricing (нет free-tier).
    assert!(p.models().iter().all(|m| m.pricing.is_some()));
}

#[test]
fn moonshot_preset_metadata() {
    let p = OpenAiCompatibleProvider::moonshot("sk-ms", "kimi-k3");
    assert_eq!(p.id(), "moonshot");
    assert_eq!(p.display_name(), "Moonshot (Kimi)");
    assert_eq!(p.active_model(), "kimi-k3");
    assert!(p.base_url().ends_with("api.moonshot.ai/v1"));
}

#[test]
fn moonshot_preset_vision_support() {
    let p = OpenAiCompatibleProvider::moonshot("sk", "kimi-k3");
    // Kimi поддерживает vision.
    assert!(p.caps().vision);
    assert!(p.models().iter().all(|m| m.supports_vision));
}

#[test]
fn ollama_preset_metadata() {
    let p = OpenAiCompatibleProvider::ollama("llama3");
    assert_eq!(p.id(), "ollama");
    assert_eq!(p.display_name(), "Ollama (local)");
    assert_eq!(p.active_model(), "llama3");
    // Ollama без auth (api_key пустой).
    assert_eq!(p.api_key(), "");
    assert!(p.base_url().starts_with("http://localhost"));
}

#[test]
fn ollama_preset_free_models() {
    let p = OpenAiCompatibleProvider::ollama("llama3");
    // Все Ollama модели free/local (pricing = None).
    assert!(p.models().iter().all(|m| m.pricing.is_none()));
}

#[test]
fn ollama_preset_caps_no_vision() {
    let p = OpenAiCompatibleProvider::ollama("llama3");
    let caps = p.caps();
    // Llama 3 не vision (по hardcoded списку). Cap vision = false.
    assert!(!caps.vision);
    assert!(caps.chat);
    assert!(caps.choice);
}

// ============================================================================
// set_active_model — валидация
// ============================================================================

#[test]
fn set_active_model_valid_openai() {
    let mut p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
    assert!(p.set_active_model("gpt-4o-mini").is_ok());
    assert_eq!(p.active_model(), "gpt-4o-mini");
}

#[test]
fn set_active_model_invalid_openai_rejected() {
    let mut p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
    let err = p.set_active_model("nonexistent-model").unwrap_err();
    assert!(matches!(err, LlmError::InvalidConfig(_)));
    // Активная модель не изменилась.
    assert_eq!(p.active_model(), "gpt-4o");
}

#[test]
fn set_active_model_valid_anthropic() {
    let mut p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    assert!(p.set_active_model("claude-3-5-haiku-20241022").is_ok());
    assert_eq!(p.active_model(), "claude-3-5-haiku-20241022");
}

#[test]
fn set_active_model_invalid_anthropic_rejected() {
    let mut p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    assert!(matches!(
        p.set_active_model("gpt-4o").unwrap_err(),
        LlmError::InvalidConfig(_)
    ));
}

// ============================================================================
// AnthropicClaudeProvider
// ============================================================================

#[test]
fn anthropic_preset_metadata() {
    let p = AnthropicClaudeProvider::new("sk-ant-test", "claude-3-5-sonnet-20241022");
    assert_eq!(p.id(), "anthropic");
    assert_eq!(p.display_name(), "Anthropic Claude");
    assert_eq!(p.active_model(), "claude-3-5-sonnet-20241022");
}

#[test]
fn anthropic_preset_caps_no_embed() {
    let p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    let caps = p.caps();
    assert!(caps.chat);
    assert!(caps.choice);
    assert!(caps.tool_calling);
    assert!(!caps.embed, "Anthropic не имеет embeddings API");
    assert!(!caps.streaming);
    assert!(caps.vision);
}

#[test]
fn anthropic_preset_models_with_pricing() {
    let p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    assert!(p.models().len() >= 3);
    // Все Anthropic модели платные.
    assert!(p.models().iter().all(|m| m.pricing.is_some()));
    // Все поддерживают vision + tools.
    assert!(p.models().iter().all(|m| m.supports_vision));
    assert!(p.models().iter().all(|m| m.supports_tools));
}

#[test]
fn anthropic_preset_models_includes_sonnet_haiku_opus() {
    let p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    let ids: Vec<&str> = p.models().iter().map(|m| m.id.as_str()).collect();
    assert!(ids.iter().any(|id| id.contains("sonnet")));
    assert!(ids.iter().any(|id| id.contains("haiku")));
    assert!(ids.iter().any(|id| id.contains("opus")));
}

// ============================================================================
// ProviderCaps default + behaviour
// ============================================================================

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

// ============================================================================
// Pricing sanity — реальные тарифы из открытых прайсов
// ============================================================================

#[test]
fn openai_gpt4o_pricing_realistic() {
    let p = OpenAiCompatibleProvider::openai("sk", "gpt-4o");
    let gpt4o = p.models().iter().find(|m| m.id == "gpt-4o").unwrap();
    let pricing = gpt4o.pricing.unwrap();
    // Реальные тарифы OpenAI на 2026: $2.50/1M in, $10.00/1M out.
    assert!((pricing.input_per_mtok - 2.50).abs() < 0.01);
    assert!((pricing.output_per_mtok - 10.00).abs() < 0.01);
}

#[test]
fn openrouter_glm_flash_pricing_realistic() {
    let p = OpenAiCompatibleProvider::openrouter("sk", "z-ai/glm-5.3-flash");
    let glm = p
        .models()
        .iter()
        .find(|m| m.id == "z-ai/glm-5.3-flash")
        .unwrap();
    let pricing = glm.pricing.unwrap();
    // Benchmark-документ: $0.121/1M (in и out одинаково для GLM).
    assert!((pricing.input_per_mtok - 0.121).abs() < 0.001);
    assert!((pricing.output_per_mtok - 0.121).abs() < 0.001);
}

#[test]
fn anthropic_claude_sonnet_pricing_realistic() {
    let p = AnthropicClaudeProvider::new("sk-ant", "claude-3-5-sonnet-20241022");
    let sonnet = p.models().iter().find(|m| m.id.contains("sonnet")).unwrap();
    let pricing = sonnet.pricing.unwrap();
    // Реальные тарифы Anthropic: $3/1M in, $15/1M out.
    assert!((pricing.input_per_mtok - 3.0).abs() < 0.01);
    assert!((pricing.output_per_mtok - 15.0).abs() < 0.01);
}

#[test]
fn pricing_free_helper() {
    let p = Pricing::free();
    assert_eq!(p.input_per_mtok, 0.0);
    assert_eq!(p.output_per_mtok, 0.0);
}

// ============================================================================
// JsonVal — serde-конверсии (feature l1-llm → serde)
// ============================================================================

#[test]
fn json_val_to_serde_roundtrip() {
    let v = JsonVal::Object(vec![
        ("name".into(), JsonVal::String("rank".into())),
        ("limit".into(), JsonVal::Number(5.0)),
        ("flag".into(), JsonVal::Bool(true)),
        (
            "opts".into(),
            JsonVal::Array(vec![JsonVal::Null, JsonVal::Number(1.0)]),
        ),
    ]);
    let s = v.to_serde();
    assert_eq!(s["name"], "rank");
    assert_eq!(s["limit"], 5);
    assert_eq!(s["flag"], true);
    assert!(s["opts"][0].is_null());
    assert_eq!(s["opts"][1], 1);
    // Обратная конверсия: ключи serde_json::Map по умолчанию сортируются
    // (BTreeMap). Сравниваем поле-за-полем.
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
    assert_eq!(find("flag"), JsonVal::Bool(true));
    assert_eq!(
        find("opts"),
        JsonVal::Array(vec![JsonVal::Null, JsonVal::Number(1.0)])
    );
}
