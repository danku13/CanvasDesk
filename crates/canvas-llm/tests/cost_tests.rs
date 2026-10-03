//! FR-LLM-A / Q4 — интеграционные тесты cost-движка.
//!
//! Проверяют формулы `estimate_cost()`, `actual_cost()`, `estimate_tokens()`
//! и реалистичный сценарий из PRD-0010 NF-4 (3000 запросов/мес ≤ $0.50).
//! Тесты не зависят от feature `l1-llm` — `cost` модуль pure-std.

use canvas_llm::cost::{actual_cost, estimate_cost, estimate_tokens};
use canvas_llm::types::{ModelInfo, Pricing};

fn free_model() -> ModelInfo {
    ModelInfo::free_local("ollama/llama3", "Llama 3 (Ollama)", 8192, true, false)
}

fn glm_flash() -> ModelInfo {
    ModelInfo {
        id: "z-ai/glm-5.3-flash".into(),
        display_name: "GLM 5.3 Flash".into(),
        context_length: 128_000,
        supports_tools: true,
        supports_vision: false,
        pricing: Some(Pricing {
            input_per_mtok: 0.121,
            output_per_mtok: 0.121,
        }),
    }
}

fn gpt4o() -> ModelInfo {
    ModelInfo {
        id: "gpt-4o".into(),
        display_name: "GPT-4o".into(),
        context_length: 128_000,
        supports_tools: true,
        supports_vision: true,
        pricing: Some(Pricing {
            input_per_mtok: 2.50,
            output_per_mtok: 10.00,
        }),
    }
}

fn claude_sonnet() -> ModelInfo {
    ModelInfo {
        id: "claude-3-5-sonnet".into(),
        display_name: "Claude 3.5 Sonnet".into(),
        context_length: 200_000,
        supports_tools: true,
        supports_vision: true,
        pricing: Some(Pricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }),
    }
}

#[test]
fn estimate_cost_free_model_zero() {
    let m = free_model();
    let c = estimate_cost(&m, 1_000_000, 1_000_000);
    assert_eq!(c.estimated_cost_usd, 0.0);
    assert_eq!(c.estimated_input_tokens, 1_000_000);
    assert_eq!(c.estimated_output_tokens, 1_000_000);
}

#[test]
fn estimate_cost_glm_flash_priced() {
    let m = glm_flash();
    // 1000 in + 500 out * $0.121/1M = 0.000121 + 0.0000605 = 0.0001815
    let c = estimate_cost(&m, 1000, 500);
    let expected = (1000.0 / 1_000_000.0) * 0.121 + (500.0 / 1_000_000.0) * 0.121;
    assert!((c.estimated_cost_usd - expected).abs() < 1e-12);
    assert!(c.estimated_cost_usd > 0.0);
}

#[test]
fn actual_cost_formula_simple() {
    let p = Pricing {
        input_per_mtok: 5.0,
        output_per_mtok: 15.0,
    };
    // 1M in = $5, 1M out = $15
    assert!((actual_cost(&p, 1_000_000, 0) - 5.0).abs() < 1e-12);
    assert!((actual_cost(&p, 0, 1_000_000) - 15.0).abs() < 1e-12);
}

#[test]
fn actual_cost_zero_tokens() {
    let p = Pricing {
        input_per_mtok: 5.0,
        output_per_mtok: 15.0,
    };
    assert_eq!(actual_cost(&p, 0, 0), 0.0);
}

#[test]
fn actual_cost_free_pricing_zero() {
    let p = Pricing::free();
    assert_eq!(actual_cost(&p, 100_000, 50_000), 0.0);
}

#[test]
fn estimate_tokens_ascii_text() {
    // 16 ASCII chars → ~4 tokens.
    assert_eq!(estimate_tokens("hello world test"), 4);
}

#[test]
fn estimate_tokens_empty_zero() {
    assert_eq!(estimate_tokens(""), 0);
}

#[test]
fn estimate_tokens_russian_nonzero() {
    let t = estimate_tokens("Привет, мир!");
    assert!(t > 0);
}

#[test]
fn estimate_tokens_long_text() {
    let text = "a".repeat(4000);
    assert_eq!(estimate_tokens(&text), 1000);
}

#[test]
fn prd_nf4_power_user_monthly_budget() {
    // PRD-0010 NF-4: power-user 3000 запросов/мес ≤ $0.50/мес (гибрид free+paid).
    // Реалистичный запрос: 1200 in + 500 out токенов.
    let per_request = estimate_cost(&glm_flash(), 1200, 500).estimated_cost_usd;
    let monthly_glm = per_request * 3000.0;
    // GLM 5.3 Flash: $0.121/1M * 3000 * (1200 + 500) / 1M = 0.6171
    assert!((monthly_glm - 0.6171).abs() < 0.001);

    // Гибрид: 70% nemotron-super:free (pricing=None) + 30% glm-flash.
    let free_requests = 3000.0 * 0.7;
    let paid_requests = 3000.0 * 0.3;
    let free_cost = estimate_cost(&free_model(), 1200, 500).estimated_cost_usd * free_requests;
    let paid_cost = per_request * paid_requests;
    let hybrid_monthly = free_cost + paid_cost;
    assert!(
        hybrid_monthly < 0.50,
        "hybrid should fit $0.50: {hybrid_monthly}"
    );
}

#[test]
fn estimate_cost_gpt4o_priced() {
    let m = gpt4o();
    // 1000 in * $2.50/1M + 500 out * $10.00/1M = $0.0025 + $0.005 = $0.0075
    let c = estimate_cost(&m, 1000, 500);
    let expected = (1000.0 / 1_000_000.0) * 2.50 + (500.0 / 1_000_000.0) * 10.00;
    assert!((c.estimated_cost_usd - expected).abs() < 1e-12);
    assert!(c.estimated_cost_usd > 0.0);
}

#[test]
fn estimate_cost_claude_sonnet_priced() {
    let m = claude_sonnet();
    // Claude дороже GPT-4o: $3/1M in + $15/1M out
    let c = estimate_cost(&m, 1000, 500);
    let expected = (1000.0 / 1_000_000.0) * 3.0 + (500.0 / 1_000_000.0) * 15.0;
    assert!((c.estimated_cost_usd - expected).abs() < 1e-12);
}

#[test]
fn cost_estimate_metadata_correct() {
    let m = gpt4o();
    let c = estimate_cost(&m, 1500, 750);
    assert_eq!(c.estimated_input_tokens, 1500);
    assert_eq!(c.estimated_output_tokens, 750);
}

#[test]
fn actual_cost_large_request_within_budget() {
    // 1000 запросов по 2000 in + 1000 out tok на glm-flash.
    let per_req = actual_cost(
        &Pricing {
            input_per_mtok: 0.121,
            output_per_mtok: 0.121,
        },
        2000,
        1000,
    );
    let daily = per_req * 1000.0;
    // Дневной лимит $1.00 (default LlmSettings.cost_limit_daily).
    assert!(daily < 1.0, "1000 requests/day fits $1.00 limit: {daily}");
}
