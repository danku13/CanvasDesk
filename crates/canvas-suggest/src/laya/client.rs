//! HTTP-клиент `POST /v1/systemone` (порт `client.py` волны 1).

use crate::types::OptionDesc;
use serde_json::{json, Value};
use std::time::Duration;

/// Инструкции choice-вопроса.
pub const CHOICE_INSTRUCTIONS: &str = "Which template best fits the node being edited?";

/// Ошибка L1-запроса. Все варианты ведут к деградации на lex (не паника,
/// не retry-цикл — воркер решает).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayaError {
    /// Сеть/таймаут/HTTP-статус (sidecar не поднялся, ушёл в OOM, …).
    Transport(String),
    /// Протокол: битый JSON, нет `answers.main` (дрейф версии sidecar).
    Protocol(String),
}

impl std::fmt::Display for LayaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayaError::Transport(s) => write!(f, "laya transport: {s}"),
            LayaError::Protocol(s) => write!(f, "laya protocol: {s}"),
        }
    }
}

impl std::error::Error for LayaError {}

/// Ответ mm-модели на choice-вопрос: вероятности (убывание, при равенстве —
/// id по возрастанию) и уверенность ответа.
#[derive(Debug, Clone, PartialEq)]
pub struct MmAnswer {
    pub probs: Vec<(String, f64)>,
    /// `answer_confidence` (до Platt; калибровка — на стороне fusion).
    pub confidence: f64,
}

/// Клиент `/v1/systemone`. Потокобезопасён (ureq::Agent), один инстанс
/// на воркер.
#[derive(Debug, Clone)]
pub struct LayaClient {
    endpoint: String,
    /// Явная модель в каждом запросе (ловушка авто-роутинга, Э1).
    model: String,
    timeout: Duration,
    /// Число повторов при транспортной ошибке (план: 1).
    retries: u32,
}

impl LayaClient {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>, timeout_ms: u64) -> Self {
        Self {
            endpoint: endpoint.into(),
            model: model.into(),
            timeout: Duration::from_millis(timeout_ms),
            retries: 1,
        }
    }

    /// Дефолт из конфиг-схемы FR-079: 127.0.0.1:8000, 800 мс, 1 повтор.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Тело запроса (порт `build_payload`): state.document + questions.choice
    /// с criteria (пустой desc → null, как в harness) + ЯВНАЯ модель.
    pub fn build_payload(&self, document: &str, options: &[OptionDesc]) -> Value {
        let mut criteria = serde_json::Map::new();
        for o in options {
            let v = if o.desc.is_empty() {
                Value::Null
            } else {
                Value::String(o.desc.clone())
            };
            criteria.insert(o.id.clone(), v);
        }
        json!({
            "state": { "document": document },
            "questions": {
                "main": {
                    "type": "choice",
                    "instructions": CHOICE_INSTRUCTIONS,
                    "criteria": criteria,
                }
            },
            "model": self.model,
        })
    }

    /// Health-проба sidecar (`GET /health`, laya-serve): готов ли чекпойнт.
    /// Вызывается воркером приложения после spawn (cold start до 60 с —
    /// план FR-079 §5.1) — до готовности запросы бессмысленны.
    pub fn health(&self) -> Result<(), LayaError> {
        let url = format!("{}/health", self.endpoint.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();
        match agent.get(&url).call() {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(code, resp)) => {
                let _ = resp.into_string();
                Err(LayaError::Transport(format!("HTTP {code}")))
            }
            Err(ureq::Error::Transport(t)) => Err(LayaError::Transport(t.to_string())),
        }
    }

    /// Choice-запрос: ранжирование опций против документа.
    pub fn choice(&self, document: &str, options: &[OptionDesc]) -> Result<MmAnswer, LayaError> {
        let payload = self.build_payload(document, options);
        let url = format!("{}/v1/systemone", self.endpoint.trim_end_matches('/'));
        let agent = ureq::AgentBuilder::new().timeout(self.timeout).build();

        let mut last_err = LayaError::Transport("нет попыток".into());
        for _ in 0..=self.retries {
            match agent.post(&url).send_json(payload.clone()) {
                Ok(resp) => {
                    let body = resp
                        .into_string()
                        .map_err(|e| LayaError::Transport(e.to_string()))?;
                    return parse_choice_answer(&body);
                }
                Err(ureq::Error::Status(code, resp)) => {
                    last_err = LayaError::Transport(format!("HTTP {code}"));
                    let _ = resp.into_string(); // слить тело соединения
                }
                Err(ureq::Error::Transport(t)) => {
                    last_err = LayaError::Transport(t.to_string());
                }
            }
        }
        Err(last_err)
    }
}

/// Разбор ответа (порт `parse_answer`): `answers.main.probabilities` +
/// `answer_confidence`. Пробы сортируются по убыванию вероятности,
/// при равенстве — id по возрастанию (детерминизм fusion не зависит
/// от порядка, но HUD/логи должны быть стабильны).
pub fn parse_choice_answer(body: &str) -> Result<MmAnswer, LayaError> {
    let v: Value =
        serde_json::from_str(body).map_err(|e| LayaError::Protocol(format!("битый JSON: {e}")))?;
    let main = v
        .pointer("/answers/main")
        .ok_or_else(|| LayaError::Protocol("нет answers.main".into()))?;
    let probs_map = main
        .get("probabilities")
        .and_then(|p| p.as_object())
        .ok_or_else(|| LayaError::Protocol("нет answers.main.probabilities".into()))?;
    let mut probs: Vec<(String, f64)> = probs_map
        .iter()
        .map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0)))
        .collect();
    probs.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let confidence = main
        .get("answer_confidence")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);
    Ok(MmAnswer { probs, confidence })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_shape_matches_harness() {
        let c = LayaClient::new("http://127.0.0.1:8000", "multilingual", 800);
        let opts = vec![
            OptionDesc::new("ue-cac", "CAC: привлечение"),
            OptionDesc::new("ue-ltv", ""), // пустой desc → null
        ];
        let p = c.build_payload("[canvas] 2 nodes", &opts);
        assert_eq!(p["state"]["document"], "[canvas] 2 nodes");
        assert_eq!(p["model"], "multilingual");
        assert_eq!(p["questions"]["main"]["type"], "choice");
        assert_eq!(p["questions"]["main"]["instructions"], CHOICE_INSTRUCTIONS);
        assert_eq!(
            p["questions"]["main"]["criteria"]["ue-cac"],
            "CAC: привлечение"
        );
        assert_eq!(p["questions"]["main"]["criteria"]["ue-ltv"], Value::Null);
    }

    #[test]
    fn parse_answer_orders_and_confidence() {
        let body = r#"{"answers": {"main": {"probabilities": {"b": 0.3, "a": 0.5, "c": 0.5},
            "answer_confidence": 0.5}}, "usage": {"input_tokens": 42}}"#;
        let a = parse_choice_answer(body).unwrap();
        assert_eq!(a.confidence, 0.5);
        // равные вероятности a/c → id по возрастанию
        assert_eq!(a.probs[0].0, "a");
        assert_eq!(a.probs[1].0, "c");
        assert_eq!(a.probs[2].0, "b");
    }

    #[test]
    fn parse_answer_protocol_errors() {
        assert!(matches!(
            parse_choice_answer("not json"),
            Err(LayaError::Protocol(_))
        ));
        assert!(matches!(
            parse_choice_answer(r#"{"answers": {}}"#),
            Err(LayaError::Protocol(_))
        ));
    }
}
