//! FR-LLM-A / Q4 (cost visibility) — Cost-estimation engine.
//!
//! Функции оценки стоимости запроса: до (`estimate_cost`) — для dialog
//! preview (graph builder, agent panel); после (`actual_cost`) — в
//! статусную панель (Stream B `ai_status_panel.rs`).
//!
//! **Тарифы** хранятся в `ModelInfo.pricing` (USD за 1M токенов, см.
//! `types.rs`). Источник: открытые прайсы провайдеров (см. benchmark
//! `docs/dev-researches/llm-mm-source-benchmark.md`). Для free/local
//! моделей (Ollama, Laya sidecar) `pricing: None` → cost = 0.0.
//!
//! **Токен-эстиматор** (`estimate_tokens`): грубый `len/4` — работает
//! для смешанного русско-английского текста (Numi-формат). Для точного
//! подсчёта нужен tiktoken/cl100k_base, но это потянуло бы зависимость
//! и нелинейный код — грубой оценки достаточно для cost-preview.

use crate::types::{CostEstimate, ModelInfo, Pricing};

/// Оценить стоимость запроса до его выполнения (Q4 cost preview).
///
/// Используется в dialog'ах (graph builder, agent panel) перед кнопкой
/// «Generate» — пользователь видит: «~$0.0042 (in: 1200 tok, out: 500 tok)».
///
/// Если `model.pricing` — `None` (free/local), `estimated_cost_usd = 0.0`.
pub fn estimate_cost(
    model: &ModelInfo,
    input_tokens: usize,
    max_output_tokens: usize,
) -> CostEstimate {
    let estimated_cost_usd = match model.pricing {
        None => 0.0,
        Some(p) => actual_cost(&p, input_tokens, max_output_tokens),
    };
    CostEstimate {
        estimated_input_tokens: input_tokens,
        estimated_output_tokens: max_output_tokens,
        estimated_cost_usd,
    }
}

/// Посчитать фактическую стоимость запроса по тарифу и usage (Q4).
///
/// Формула: `(input_tokens / 1_000_000) * pricing.input_per_mtok +
/// (output_tokens / 1_000_000) * pricing.output_per_mtok`.
///
/// Используется после запроса (Stream B `ai_status_panel.rs` суммирует
/// за сессию и за день).
pub fn actual_cost(pricing: &Pricing, input_tokens: usize, output_tokens: usize) -> f64 {
    let in_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok;
    let out_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok;
    in_cost + out_cost
}

/// Грубая оценка числа токенов в тексте (RU+EN mixed, Numi-формат).
///
/// ~4 символа на токен — стандартная эвристика для тикенайзеров BPE
/// (cl100k_base, llama tokenizer). Для RU-текста оценка слегка
/// консервативная (кириллица = 1.5-2 токена/символ), но для cost-preview
/// точность ±20% приемлема (пользователь видит «~$0.004», не «$0.0042»).
pub fn estimate_tokens(text: &str) -> usize {
    // `len()` возвращает байты; для ASCII это ~4 байт/токен, для UTF-8
    // кириллицы — 2 байта/символ, но 1.5-2 токена/символ. Итог:
    // байты/4 ≈ токены для типичных смешанных текстов CanvasDesk.
    text.len() / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    fn free_model() -> ModelInfo {
        ModelInfo::free_local("ollama/llama3", "Llama 3", 8192, true, false)
    }

    fn priced_model() -> ModelInfo {
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

    #[test]
    fn estimate_cost_free_model() {
        let m = free_model();
        let c = estimate_cost(&m, 1000, 500);
        assert_eq!(c.estimated_input_tokens, 1000);
        assert_eq!(c.estimated_output_tokens, 500);
        assert_eq!(c.estimated_cost_usd, 0.0);
    }

    #[test]
    fn estimate_cost_priced_model() {
        let m = priced_model();
        // $0.121/1M tok * (1000 + 500) tok = 0.121 * 1500 / 1_000_000 = 0.0001815
        let c = estimate_cost(&m, 1000, 500);
        let expected = (1000.0 / 1_000_000.0) * 0.121 + (500.0 / 1_000_000.0) * 0.121;
        assert!((c.estimated_cost_usd - expected).abs() < 1e-12);
    }

    #[test]
    fn actual_cost_formula() {
        let p = Pricing {
            input_per_mtok: 5.0,
            output_per_mtok: 15.0,
        };
        // 1M входных = $5, 1M выходных = $15
        assert!((actual_cost(&p, 1_000_000, 0) - 5.0).abs() < 1e-12);
        assert!((actual_cost(&p, 0, 1_000_000) - 15.0).abs() < 1e-12);
        // 1000 in + 500 out = 5/1000 + 15*0.5/1000 = 0.005 + 0.0075 = 0.0125
        assert!((actual_cost(&p, 1000, 500) - 0.0125).abs() < 1e-12);
    }

    #[test]
    fn actual_cost_free_pricing() {
        let p = Pricing::free();
        assert_eq!(actual_cost(&p, 1_000_000, 1_000_000), 0.0);
    }

    #[test]
    fn estimate_tokens_ascii() {
        // 16 символов ASCII → ~4 токена.
        assert_eq!(estimate_tokens("hello world test"), 4);
    }

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_russian() {
        // "Привет, мир!" — 12 символов в UTF-8 это больше байт (кириллица
        // = 2 байта/символ). Грубая оценка ~6 байт/4 = ~6 токенов.
        let t = estimate_tokens("Привет, мир!");
        // Точное значение зависит от encoding, но > 0.
        assert!(t > 0);
    }

    #[test]
    fn estimate_cost_zero_tokens() {
        let m = priced_model();
        let c = estimate_cost(&m, 0, 0);
        assert_eq!(c.estimated_cost_usd, 0.0);
    }

    #[test]
    fn estimate_cost_rounds_to_displayable() {
        // Реальный пример из PRD: 3000 запросов/мес * (~1200 in + 500 out tok)
        // = 3.6M in + 1.5M out токенов. С glm-5.3-flash ($0.121/1M):
        // 3.6 * 0.121 + 1.5 * 0.121 = 0.4356 + 0.1815 = $0.6171/мес.
        let m = priced_model();
        let per_request = estimate_cost(&m, 1200, 500).estimated_cost_usd;
        let monthly = per_request * 3000.0;
        // Цель PRD: ≤ $0.50/мес. Реальная цифра $0.62 — близко, на грани.
        // Это обосновывает гибрид free+paid (nemotron-super:free для части
        // запросов). Тест фиксирует формулу, не требование ≤ $0.50.
        assert!(monthly > 0.0);
        assert!((monthly - 0.6171).abs() < 0.001);
    }
}
