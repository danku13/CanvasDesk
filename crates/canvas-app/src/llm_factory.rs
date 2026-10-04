//! FR-LLM-OAUTH-APP / PRD-0010 F-5.6+F-5.9 — фабрика `LlmProvider` по
//! [`canvas_llm::LlmProviderId`] из `Settings`.
//!
//! Единственная точка, где панели приложения (agent panel, graph builder)
//! превращают пользовательский выбор провайдера в конкретный адаптер
//! `canvas-llm`. Раньше выбор существовал только как UI-состояние (панели
//! работали на моках, `// FR-LLM-D-TODO:`); теперь фабрика строит реальный
//! провайдер и заодно валидирует конфиг (пустой BYOK-ключ → `None`),
//! а панели через [`provider_unavailable_message`] получают сообщение
//! graceful degradation F-5.9 (fallback на BYOK).
//!
//! **Маппинг** (модели-«glm-5.3-*» в дефолтах — бенчмарк-пресеты прототипа;
//! по дизайн-доку F-2/бенчмарку `docs/dev-researches/llm-mm-source-benchmark.md`
//! модели хостятся на OpenRouter, поэтому дефолт BYOK-пресета —
//! `OpenAiCompatibleProvider::openrouter`):
//! - [`LlmProviderId::Byok`] → `OpenAiCompatibleProvider::openrouter`
//!   (дефолт); непустой `settings.endpoint` → selfhost-пресет
//!   (`OpenAiCompatibleProvider::new` с `base_url = endpoint`, ключ —
//!   `settings.selfhost_key`, F-5.9/Q5 «в контуре»);
//! - [`LlmProviderId::ChatGptOAuth`] → `ChatGptOAuthProvider` (токены — из
//!   [`OAuthAssets`], модель — из `model_graph`/`model_agent` по фиче);
//! - [`LlmProviderId::Ollama`] → `OpenAiCompatibleProvider::ollama`;
//! - [`LlmProviderId::Laya`] / [`LlmProviderId::Off`] → `None`: Laya —
//!   sidecar-транспорт внутри canvas-suggest (FR-079 S3, `laya/client.rs`),
//!   а не `LlmProvider`; `Off` — AI выключен.
//!
//! Дефолтная сборка (без feature `l1-llm`, wasm-гейт ADR-0011) — stub:
//! фабрика всегда возвращает `None`, чтобы canvas-app компилировался без
//! сетевых зависимостей (панели получают сообщение о недоступности).

// FR-LLM-OAUTH-APP: маркер для поиска (grep): файлы app-wiring OAuth помечены
// `// FR-LLM-OAUTH-APP:` в комментариях.

use canvas_llm::{LlmProvider, LlmProviderId, LlmSettings};

/// FR-LLM-OAUTH-APP: OAuth-артефакты, нужные фабрике для
/// `ChatGptOAuthProvider` (дизайн-док §4.3/§4.4). Собирается `App` из
/// состояния OAuth-флоу (`app/oauth_flow.rs`) на каждый запрос провайдера.
///
/// Без feature `l1-llm` структура пуста (stub) — OAuth-провайдер не строится.
#[cfg(feature = "l1-llm")]
pub struct OAuthAssets {
    /// Хранилище OAuth-токенов (canvas-app: `FileTokenStore` поверх
    /// oauth_tokens.json / memory-фолбэк; дизайн-док §4.4 — токены НЕ в
    /// config.toml). `None` — вход не выполнялся / store недоступен:
    /// провайдер строится с пустой memory-записью и вернёт `Auth` при
    /// первом запросе (UI покажет «войти снова», F-5.9).
    pub token_store: Option<std::sync::Arc<dyn canvas_llm::TokenStore>>,
    /// Persistent device id (один на установку; из
    /// `LlmSettings.ext_agent_host_id`, дизайн-док §4.3). Пустая строка —
    /// вход ещё не выполнялся (refresh/login из провайдера не потребуются).
    pub ext_agent_host_id: String,
}

/// Stub-версия [`OAuthAssets`] для сборок без feature `l1-llm` (wasm-гейт).
#[cfg(not(feature = "l1-llm"))]
#[derive(Default)]
pub struct OAuthAssets {
    /// Приватное поле — структура не конструируется литералом снаружи
    /// (единая точка создания — [`OAuthAssets::stub`]).
    _priv: (),
}

impl OAuthAssets {
    /// Stub-конструктор для дефолтной сборки (без сети).
    #[cfg(not(feature = "l1-llm"))]
    pub fn stub() -> Self {
        Self { _priv: () }
    }
}

/// FR-LLM-OAUTH-APP: собрать провайдер для AI-фичи по выбору пользователя.
///
/// `feature` — per-feature значение `LlmSettings.provider_{suggest,graph,
/// agent}`; `model` — соответствующая `model_*` (для ChatGptOAuth — модель
/// подписки из `model_graph`/`model_agent`); `settings` — LLM-настройки;
/// `oauth` — OAuth-артефакты (см. [`OAuthAssets`]).
///
/// Возвращает `None`, когда провайдер недоступен по конфигу (пустой BYOK-
/// ключ, `Off`, Laya) — вызывающая панель показывает сообщение F-5.9.
#[cfg(feature = "l1-llm")]
pub fn build_feature_provider(
    feature: LlmProviderId,
    model: &str,
    settings: &LlmSettings,
    oauth: &OAuthAssets,
) -> Option<Box<dyn LlmProvider>> {
    match feature {
        // FR-LLM-OAUTH-APP: BYOK — дефолт OpenRouter (бенчмарк-модели
        // «glm-5.3-*» хостятся там, см. модуль-док). Непустой endpoint →
        // selfhost-пресет: base_url = endpoint, ключ — selfhost_key
        // (F-5.9/Q5 «Self-hosted в вашем контуре»). Пустой ключ — None
        // (InvalidConfig-лог классом выше: `LlmSettings::validate` уже
        // помечает конфиг невалидным, здесь — мягкий отказ панели).
        LlmProviderId::Byok => {
            let key = settings.api_key.trim();
            if key.is_empty() {
                tracing::debug!("llm_factory: BYOK без API-ключа — провайдер недоступен");
                return None;
            }
            let endpoint = settings.endpoint.trim();
            let provider = if endpoint.is_empty() {
                canvas_llm::OpenAiCompatibleProvider::openrouter(key, model)
            } else {
                // Self-hosted OpenAI-compatible endpoint (data_residency =
                // SelfHosted). Пресет «selfhost»: кастомный base_url +
                // отдельный selfhost_key (может быть пуст — локальный
                // endpoint без авторизации).
                canvas_llm::OpenAiCompatibleProvider::new(
                    "selfhost",
                    "Self-hosted",
                    endpoint,
                    settings.selfhost_key.trim(),
                    model,
                    Vec::new(),
                )
            };
            Some(Box::new(provider))
        }
        // FR-LLM-OAUTH-APP: Sign-in-with-ChatGPT — токены из store (дизайн-
        // док §4.4), proxy не используется на desktop (прямой api.openai.com;
        // web-путь через cloud-proxy F-5.10 — вне этой фабрики, wasm).
        LlmProviderId::ChatGptOAuth => {
            let store: Box<dyn canvas_llm::TokenStore> = match &oauth.token_store {
                // Arc<dyn TokenStore> → Box: тонкая обёртка (trait blanket
                // для Arc в canvas-llm нет — store единственный владелец).
                Some(arc) => Box::new(SharedTokenStore(arc.clone())),
                None => Box::new(canvas_llm::MemoryTokenStore::new()),
            };
            Some(Box::new(
                canvas_llm::ChatGptOAuthProvider::new(store, oauth.ext_agent_host_id.clone(), None)
                    .with_model(model),
            ))
        }
        // FR-LLM-OAUTH-APP: Ollama — localhost, без ключа.
        LlmProviderId::Ollama => Some(Box::new(canvas_llm::OpenAiCompatibleProvider::ollama(
            model,
        ))),
        // Laya — sidecar внутри canvas-suggest (FR-079), не LlmProvider:
        // choice-ранжирование идёт мимо этого трейта. Off — AI выключен.
        LlmProviderId::Laya | LlmProviderId::Off => None,
    }
}

/// Stub-версия фабрики (дефолтная сборка без `l1-llm`): провайдеров нет —
/// всегда `None` (wasm-гейт ADR-0011; панели показывают сообщение F-5.9).
#[cfg(not(feature = "l1-llm"))]
pub fn build_feature_provider(
    _feature: LlmProviderId,
    _model: &str,
    _settings: &LlmSettings,
    _oauth: &OAuthAssets,
) -> Option<Box<dyn LlmProvider>> {
    None
}

/// FR-LLM-OAUTH-APP: обёртка `Arc<dyn TokenStore>` под `Box<dyn TokenStore>`
/// (конструктор `ChatGptOAuthProvider::new` принимает Box; store живёт в
/// `App::oauth_flow` и переиспользуется между запросами провайдера).
#[cfg(feature = "l1-llm")]
struct SharedTokenStore(std::sync::Arc<dyn canvas_llm::TokenStore>);

#[cfg(feature = "l1-llm")]
impl canvas_llm::TokenStore for SharedTokenStore {
    fn save(&self, tokens: &canvas_llm::OAuthTokens) -> Result<(), canvas_llm::LlmError> {
        self.0.save(tokens)
    }
    fn load(&self) -> Result<Option<canvas_llm::OAuthTokens>, canvas_llm::LlmError> {
        self.0.load()
    }
    fn clear(&self) -> Result<(), canvas_llm::LlmError> {
        self.0.clear()
    }
}

/// FR-LLM-OAUTH-APP / PRD-0010 F-5.9: сообщение для панели, если провайдер
/// по текущему конфигу недоступен (`build_feature_provider == None`).
///
/// Панели (agent/graph) вызывают перед отправкой запроса: `None` — провайдер
/// готов (реальный вызов — точка `// FR-LLM-D-TODO:` в панели); `Some(text)`
/// — показать сообщение пользователем-механикой панели (AgentMessage::Error /
/// `show_toast`) и НЕ отправлять запрос.
///
/// F-5.9 (fallback на BYOK): при недоступном ChatGPT-входе, если
/// `settings.api_key` непустой, сообщение обещает fallback на BYOK — реальный
/// вызов строит провайдер повторно с `LlmProviderId::Byok` (точка
/// `// FR-LLM-D-TODO:`). Тексты — RU в стиле мок-сообщений панелей
/// (`// FR-LLM-D-TODO:` i18n при вживлении реальных вызовов).
#[cfg(feature = "l1-llm")]
pub fn provider_unavailable_message(
    feature: LlmProviderId,
    model: &str,
    settings: &LlmSettings,
    oauth: &OAuthAssets,
) -> Option<String> {
    if build_feature_provider(feature, model, settings, oauth).is_some() {
        return None;
    }
    let byok_fallback_ready = !settings.api_key.trim().is_empty();
    match feature {
        LlmProviderId::ChatGptOAuth => Some(if byok_fallback_ready {
            "ChatGPT: вход не выполнен (нет токенов) — запрос уйдёт через BYOK-ключ. \
             Выполните «Войти через ChatGPT» в Настройках AI (9-й таб)."
                .to_owned()
        } else {
            "ChatGPT: вход не выполнен (нет токенов), BYOK-ключ не задан. \
             Выполните «Войти через ChatGPT» или введите BYOK-ключ в Настройках AI (9-й таб)."
                .to_owned()
        }),
        LlmProviderId::Byok => {
            Some("BYOK: API-ключ не задан — введите ключ в Настройках AI (9-й таб).".to_owned())
        }
        LlmProviderId::Ollama => Some(
            "Ollama: провайдер недоступен для сборки (нет feature l1-llm?) — \
             проверьте настройку endpoint в Настройках AI."
                .to_owned(),
        ),
        // Laya — не LlmProvider (sidecar canvas-suggest); Off — AI выключен
        // (панели сами проверяют Off раньше фабрики).
        LlmProviderId::Laya | LlmProviderId::Off => None,
    }
}

#[cfg(all(test, feature = "l1-llm"))]
mod tests {
    use super::*;
    use canvas_llm::TokenStore;

    /// Дефолтные настройки с непустым BYOK-ключом (как `app_with_llm_settings`).
    fn byok_settings() -> LlmSettings {
        LlmSettings {
            api_key: "sk-test-123".to_owned(),
            ..LlmSettings::default()
        }
    }

    fn no_oauth() -> OAuthAssets {
        OAuthAssets {
            token_store: None,
            ext_agent_host_id: String::new(),
        }
    }

    /// Byok → OpenRouter-пресет (дефолт бенчмарк-моделей), модель — как
    /// задана пользователем (с текстовым вводом валидация по списку не
    /// делается — модели подписки обновляются раньше hardcoded-списка).
    #[test]
    fn byok_maps_to_openrouter_preset() {
        let settings = byok_settings();
        let p =
            build_feature_provider(LlmProviderId::Byok, "glm-5.3-flash", &settings, &no_oauth())
                .expect("провайдер построен");
        assert_eq!(p.id(), "openrouter");
    }

    /// Непустой endpoint → selfhost-пресет: base_url = endpoint, ключ —
    /// selfhost_key (Q5 «в контуре»).
    #[test]
    fn byok_with_endpoint_overrides_to_selfhost() {
        let settings = LlmSettings {
            api_key: "sk-test-123".to_owned(),
            endpoint: "https://llm.corp.local/v1".to_owned(),
            selfhost_key: "shkey-abc".to_owned(),
            ..LlmSettings::default()
        };
        let p = build_feature_provider(LlmProviderId::Byok, "m1", &settings, &no_oauth())
            .expect("провайдер построен");
        assert_eq!(p.id(), "selfhost");
    }

    /// Пустой BYOK-ключ → None (мягкий отказ панели, F-5.9).
    #[test]
    fn byok_without_key_is_none() {
        let settings = LlmSettings {
            api_key: String::new(),
            endpoint: String::new(),
            ..LlmSettings::default()
        };
        assert!(build_feature_provider(LlmProviderId::Byok, "m", &settings, &no_oauth()).is_none());
        // Сообщение пользователю — про ключ.
        let msg = provider_unavailable_message(LlmProviderId::Byok, "m", &settings, &no_oauth());
        assert!(msg.unwrap().contains("ключ"));
    }

    /// ChatGptOAuth строится всегда (валидность решает store при запросе):
    /// id провайдера — chatgpt_oauth, модель — переданная.
    #[test]
    fn chatgpt_oauth_maps_to_provider() {
        let p = build_feature_provider(
            LlmProviderId::ChatGptOAuth,
            "gpt-5.2",
            &LlmSettings::default(),
            &no_oauth(),
        )
        .expect("провайдер построен");
        assert_eq!(p.id(), "chatgpt_oauth");
        assert_eq!(p.active_model(), "gpt-5.2");
    }

    /// FR-LLM-OAUTH-APP: plumbing store из `OAuthAssets` — `SharedTokenStore`
    /// проксирует save/load/clear (провайдер читает токены именно через эту
    /// обёртку; TraitObject-Arc → Box без blanket impl в canvas-llm).
    #[test]
    fn shared_token_store_proxies_operations() {
        let store = std::sync::Arc::new(canvas_llm::MemoryTokenStore::new());
        let wrapper = SharedTokenStore(store.clone());
        let tokens = canvas_llm::OAuthTokens {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            id_token: "hdr.pld.sig".into(),
            expires_at: u64::MAX,
            account_email: Some("user@example.com".into()),
        };
        assert_eq!(wrapper.load().expect("load"), None);
        wrapper.save(&tokens).expect("save");
        assert_eq!(wrapper.load().expect("load"), Some(tokens));
        // Прокси двусторонний: запись в memory-store видна через wrapper.
        store.clear().expect("clear");
        assert_eq!(wrapper.load().expect("load"), None);
    }

    /// Ollama → ollama-пресет (localhost).
    #[test]
    fn ollama_maps_to_ollama_preset() {
        let p = build_feature_provider(
            LlmProviderId::Ollama,
            "llama3.1:8b",
            &LlmSettings::default(),
            &no_oauth(),
        )
        .expect("провайдер построен");
        assert_eq!(p.id(), "ollama");
    }

    /// Laya/Off → None (Laya — sidecar canvas-suggest, Off — AI выключен).
    #[test]
    fn laya_and_off_are_none() {
        let s = LlmSettings::default();
        assert!(build_feature_provider(LlmProviderId::Laya, "m", &s, &no_oauth()).is_none());
        assert!(build_feature_provider(LlmProviderId::Off, "m", &s, &no_oauth()).is_none());
        // И сообщение для них не генерируется (панель проверяет Off раньше).
        assert!(provider_unavailable_message(LlmProviderId::Off, "m", &s, &no_oauth()).is_none());
    }
}
