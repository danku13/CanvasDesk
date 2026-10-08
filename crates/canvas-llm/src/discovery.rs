//! W1 (wave-1, `docs/plans/llm-waves-w1-w2-w3.md` §W1 п.4) — discovery
//! моделей `GET /v1/models` с TTL-кэшем.
//!
//! Для dropdown'а выбора модели в Settings (волна W2 подключит UI): вместо
//! статического hardcoded-списка — реальный список моделей провайдера.
//! Запрос идёт через [`crate::transport::HttpTransport`] (натив —
//! UreqTransport; web — WasmFetchTransport к cloud-proxy `cloud/llm-proxy`,
//! у воркера есть CORS-прокси `/v1/models`).
//!
//! Кэш — in-memory с TTL (дефолт 10 мин, ключ = provider_id + base_url);
//! persist между сессиями — сознательный бэклог (§6 плана волн).
//!
//! Ошибки: 401/403 → `Auth`, 429 → `RateLimit` (с `Retry-After`), прочий
//! не-2xx → `Transport` — общий [`HttpResponse::map_status`]. Секреты
//! (API-ключ) уходят только в заголовках и никогда не попадают в тексты
//! ошибок (ошибки транспорта содержат URL/статус/тело ответа, не заголовки).

use crate::error::LlmError;
use crate::transport::{HttpRequest, HttpResponse, HttpTransport};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// TTL кэша discovery по умолчанию — 10 минут.
const DEFAULT_TTL: Duration = Duration::from_secs(600);

/// Модель, возвращённая discovery (`GET /v1/models`).
///
/// Сознательно НЕ [`crate::types::ModelInfo`]: у discovery-ответа нет
/// context_length/pricing/supports_* — заполнять их фиктивными значениями
/// значило бы вводить W2-UI в заблуждение. Если UI-дропдауну нужен именно
/// `ModelInfo` — собирается из `id` с честными дефолтами на стороне W2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredModel {
    /// Идентификатор модели (передаётся в поле `model` API).
    pub id: String,
    /// Владелец модели (`owned_by` из ответа, если провайдер отдаёт).
    pub owned_by: Option<String>,
}

/// Разобрать JSON-ответ `GET /v1/models` (чистая функция — тесты без сети).
/// Формат OpenAI-compatible: `{"data": [{"id": "…", "owned_by": "…"}, …]}`.
/// Список сортируется по `id` (стабильный порядок для UI).
pub fn parse_models_json(text: &str) -> Result<Vec<DiscoveredModel>, LlmError> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| LlmError::Protocol(format!("models: битый JSON: {e}")))?;
    let data = value
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LlmError::Protocol("models: нет data[]".into()))?;
    let mut models: Vec<DiscoveredModel> = data
        .iter()
        .filter_map(|m| {
            let id = m.get("id")?.as_str()?.to_string();
            let owned_by = m
                .get("owned_by")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Some(DiscoveredModel { id, owned_by })
        })
        .collect();
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

/// Запрос `GET {base_url}/models` с опциональным Bearer-ключом (общее для
/// sync/async вариантов).
fn models_request(base_url: &str, auth: Option<&str>, timeout: Duration) -> HttpRequest {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = HttpRequest::get(url, timeout);
    if let Some(key) = auth.filter(|k| !k.trim().is_empty()) {
        req = req.with_header("Authorization", format!("Bearer {}", key.trim()));
    }
    req
}

/// Discovery: реальный список моделей провайдера (async — web/wasm путь
/// и async-контексты; на нативе футура доводится в worker-потоке).
pub async fn list_models(
    transport: &dyn HttpTransport,
    base_url: &str,
    auth: Option<&str>,
    timeout: Duration,
) -> Result<Vec<DiscoveredModel>, LlmError> {
    let resp = transport
        .execute(models_request(base_url, auth, timeout))
        .await?;
    models_from_response(resp)
}

/// Синхронный двойник [`list_models`] (desktop: worker-поток; WasmFetch
/// вернёт `NotSupported` — web использует async-вариант).
pub fn list_models_blocking(
    transport: &dyn HttpTransport,
    base_url: &str,
    auth: Option<&str>,
    timeout: Duration,
) -> Result<Vec<DiscoveredModel>, LlmError> {
    let resp = transport.execute_blocking(&models_request(base_url, auth, timeout))?;
    models_from_response(resp)
}

/// Маппинг статуса + парсинг тела (общее для sync/async).
fn models_from_response(resp: HttpResponse) -> Result<Vec<DiscoveredModel>, LlmError> {
    if let Some(err) = resp.map_status("models") {
        return Err(err);
    }
    parse_models_json(&resp.body_str())
}

/// Ключ кэша: (идентификатор провайдера, base_url). У одного провайдера
/// может быть несколько endpoint'ов (selfhost) — кэш раздельный.
pub type CacheKey = (String, String);

/// Запись кэша: модели + момент загрузки (`web_time::Instant` — wasm32-
/// безопасный таймер, см. wasm-гейт времени FR-079 S3-fix).
struct CacheEntry {
    models: Vec<DiscoveredModel>,
    fetched_at: web_time::Instant,
}

/// In-memory TTL-кэш discovery (потокобезопасный; дефолтный TTL 10 мин).
///
/// Использование (W2): перед сетевым запросом — [`Self::get`]; после
/// успешного — [`Self::insert`]; при ошибке 401 (ключ сменили) —
/// [`Self::invalidate`].
#[derive(Default)]
pub struct ModelCache {
    ttl: Duration,
    entries: Mutex<HashMap<CacheKey, CacheEntry>>,
}

impl ModelCache {
    /// Кэш с TTL по умолчанию (10 минут).
    pub fn new() -> Self {
        Self::with_ttl(DEFAULT_TTL)
    }

    /// Кэш с кастомным TTL (тесты: `Duration::ZERO` → всегда истёк).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Свежий (не истёкший) список моделей, если есть в кэше.
    pub fn get(&self, provider_id: &str, base_url: &str) -> Option<Vec<DiscoveredModel>> {
        let entries = self.entries.lock().expect("model cache");
        let entry = entries.get(&(provider_id.to_string(), base_url.to_string()))?;
        if entry.fetched_at.elapsed() > self.ttl {
            return None; // истёк — вызывающий перезапросит
        }
        Some(entry.models.clone())
    }

    /// Положить список в кэш (после успешного discovery).
    pub fn insert(&self, provider_id: &str, base_url: &str, models: Vec<DiscoveredModel>) {
        let mut entries = self.entries.lock().expect("model cache");
        entries.insert(
            (provider_id.to_string(), base_url.to_string()),
            CacheEntry {
                models,
                fetched_at: web_time::Instant::now(),
            },
        );
    }

    /// Сбросить запись (смена ключа, ошибка авторизации).
    pub fn invalidate(&self, provider_id: &str, base_url: &str) {
        let mut entries = self.entries.lock().expect("model cache");
        entries.remove(&(provider_id.to_string(), base_url.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;

    const FIXTURE: &str = r#"{
        "object": "list",
        "data": [
            { "id": "gpt-4o-mini", "owned_by": "system" },
            { "id": "z-ai/glm-5.3-flash", "owned_by": "z.ai" },
            { "id": "gpt-4o", "owned_by": "openai" }
        ]
    }"#;

    #[test]
    fn parse_sorts_by_id_and_keeps_owned_by() {
        let models = parse_models_json(FIXTURE).unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        // Сортировка по id (стабильный порядок для UI-дропдауна).
        assert_eq!(ids, vec!["gpt-4o", "gpt-4o-mini", "z-ai/glm-5.3-flash"]);
        assert_eq!(models[2].owned_by.as_deref(), Some("z.ai"));
    }

    #[test]
    fn parse_rejects_broken_json_and_missing_data() {
        assert!(matches!(
            parse_models_json("not json"),
            Err(LlmError::Protocol(_))
        ));
        assert!(matches!(
            parse_models_json(r#"{"object": "list"}"#),
            Err(LlmError::Protocol(_))
        ));
    }

    #[test]
    fn list_models_via_mock_sends_bearer_and_parses() {
        pollster::block_on(async {
            let mock = MockTransport::new(vec![MockTransport::json_ok(200, FIXTURE)]);
            let models = list_models(
                &mock,
                "https://api.example.com/v1",
                Some("sk-test"),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            assert_eq!(models.len(), 3);

            let req = mock.last_request().unwrap();
            assert_eq!(req.url, "https://api.example.com/v1/models");
            assert_eq!(req.method, crate::transport::HttpMethod::Get);
            assert_eq!(req.header("authorization"), Some("Bearer sk-test"));
        });
    }

    #[test]
    fn list_models_maps_401_to_auth() {
        pollster::block_on(async {
            let mock = MockTransport::new(vec![MockTransport::json_ok(401, "no key")]);
            let err = list_models(
                &mock,
                "https://api.example.com/v1",
                Some("bad"),
                Duration::from_secs(10),
            )
            .await
            .unwrap_err();
            assert!(matches!(err, LlmError::Auth(_)));
        });
    }

    #[test]
    fn list_models_empty_key_sends_no_auth_header() {
        pollster::block_on(async {
            let mock = MockTransport::new(vec![MockTransport::json_ok(200, FIXTURE)]);
            list_models(
                &mock,
                "https://api.example.com/v1",
                Some(""),
                Duration::from_secs(10),
            )
            .await
            .unwrap();
            let req = mock.last_request().unwrap();
            assert_eq!(
                req.header("authorization"),
                None,
                "Ollama без ключа — заголовка нет"
            );
        });
    }

    #[test]
    fn cache_hit_expiry_and_invalidate() {
        let cache = ModelCache::with_ttl(Duration::from_secs(600));
        let models = parse_models_json(FIXTURE).unwrap();

        // Пусто → вставка → попадание → инвалидация → пусто.
        assert!(cache.get("zai", "https://api.z.ai/v1").is_none());
        cache.insert("zai", "https://api.z.ai/v1", models.clone());
        assert_eq!(cache.get("zai", "https://api.z.ai/v1").unwrap().len(), 3);
        cache.invalidate("zai", "https://api.z.ai/v1");
        assert!(cache.get("zai", "https://api.z.ai/v1").is_none());

        // Ключи изолированы: base_url входит в ключ (selfhost-эндпоинты).
        cache.insert("zai", "https://api.z.ai/v1", models);
        assert!(cache.get("zai", "https://other.example.com/v1").is_none());
    }

    #[test]
    fn cache_zero_ttl_is_always_expired() {
        let cache = ModelCache::with_ttl(Duration::ZERO);
        cache.insert("p", "https://base", vec![]);
        assert!(
            cache.get("p", "https://base").is_none(),
            "TTL=0 → запись мгновенно истекла"
        );
    }
}
