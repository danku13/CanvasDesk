//! W3 (план `llm-waves-w1-w2-w3` §4, F-5.10): web-швы LLM — активация
//! ИИ-функционала браузерной сборки. Наполняет W2-симы `canvas-app`:
//!
//! - **wasm-executor** (п.2): инъекция [`App::set_llm_spawner`] —
//!   `spawn_local(thunk())`; результаты джоб кладутся future'ом в общий
//!   инбокс, дренаж — `llm_poll` в `about_to_wait` (паттерн `oauth_poll`);
//! - **fetch-транспорт** (п.3): [`App::set_llm_transport`] с
//!   `canvas_llm::WasmFetchTransport` (браузерный `window.fetch`);
//!   health-check/discovery и все LLM-вызовы уходят через единый шов W1;
//! - **OPFS token store** (п.1): [`OpfsTokenStore`] — реализация
//!   `canvas_llm::TokenStore` поверх OPFS (`oauth-tokens.json`,
//!   origin-scoped). Трейт синхронный, OPFS асинхронен — кэш в памяти +
//!   фоновая запись через `spawn_local`; свежесть на старте — [`preload`]
//!   (вызывается из `spawn_desk_web` до построения App);
//! - **OAuth web-флоу** (п.4): мост [`WebLlmBridge`] (popup логина на
//!   authorize-URL, выход) + [`recover_oauth_callback`] — детект
//!   `?oauth_callback=` (302-редирект воркера `cloud/llm-proxy`,
//!   `OAUTH_DEEP_LINK_BASE` = URL приложения с маркером), обмен code через
//!   pass-through прокси (`WasmFetchTransport`), сохранение токенов в OPFS
//!   и синхронизация флагов `chatgpt_connected/email` в localStorage-конфиге
//!   (бейдж 9-го таба оживает после перезагрузки страницы — inherent для
//!   redirect-флоу).
//!
//! Токены в `localStorage` НЕ пишутся (XSS-риск, дизайн-док §4.4) — только
//! OPFS; в localStorage живут несекретные PKCE-сессия, host id и базовый
//! URL прокси.
//!
//! Нативная компиляция (rlib-тесты каркаса): модуль целиком за
//! `target_arch = "wasm32"` — canvas-app без `l1-llm` не имеет швов.

use std::rc::Rc;
use std::sync::Arc;

use canvas_app::app::App;
use canvas_llm::chatgpt_oauth::auth::{OAuthClient, DEEP_LINK_REDIRECT};
use canvas_llm::chatgpt_oauth::tokens::TokenStore;
use canvas_llm::{LlmError, OAuthTokens};

/// Имя файла токенов в корне OPFS (origin-scoped хранилище браузера).
pub(crate) const TOKENS_FILE: &str = "oauth-tokens.json";
/// Ключ localStorage: несекретная PKCE-сессия логина (verifier/state/nonce)
/// между `start_login` и callback-перезагрузкой страницы.
const OAUTH_SESSION_KEY: &str = "canvasdesk.oauthSession";
/// Ключ localStorage: persistent device id (`ext_agent_host_id`, дизайн-док
/// §4.3) — один на установку, генерируется из CSPRNG при первом входе.
const HOST_ID_KEY: &str = "canvasdesk.agentHostId";
/// Ключ localStorage: базовый URL `cloud/llm-proxy` (деплой владельца).
/// Дефолт — стандартный subdomain воркера из `wrangler.toml`.
const PROXY_URL_KEY: &str = "canvasdesk.llmProxyUrl";
const DEFAULT_PROXY_URL: &str = "https://canvasdesk-llm-proxy.workers.dev";

// ============================================================================
// Токен-стор поверх OPFS (W3 п.1)
// ============================================================================

/// `TokenStore` поверх OPFS: кэш в памяти (синхронный трейт) + фоновая
/// запись файла [`TOKENS_FILE`] через `spawn_local`. Отказ OPFS —
/// `LlmError` из `save` (сессия несохраняема → честный Auth-класс), `load`
/// отдаёт кэш (сессия живёт до перезагрузки — graceful degradation F-5.9).
pub struct OpfsTokenStore {
    cache: std::sync::Mutex<Option<OAuthTokens>>,
}

impl OpfsTokenStore {
    /// Пустой стор (кэш None); содержимое OPFS подтягивает [`preload`].
    pub fn new() -> Self {
        Self {
            cache: std::sync::Mutex::new(None),
        }
    }

    /// Прочитать файл токенов из OPFS в кэш (старт страницы). Ошибка OPFS —
    /// warn (ст store работает в памяти, refresh после перезагрузки
    /// недоступен — F-5.9).
    pub async fn preload(&self) {
        let file = match crate::opfs::opfs_root().await {
            Ok(root) => crate::opfs::read_opfs_text(&root, TOKENS_FILE).await,
            Err(err) => {
                tracing::warn!(target: "canvas_web", ?err, "llm_web: OPFS недоступен — токен-стор в памяти");
                return;
            }
        };
        match file {
            Ok(Some(text)) => match serde_json::from_str::<Option<OAuthTokens>>(&text) {
                Ok(tokens) => {
                    let connected = tokens.is_some();
                    if let Ok(mut slot) = self.cache.lock() {
                        *slot = tokens;
                    }
                    tracing::info!(target: "canvas_web", connected, "llm_web: токены загружены из OPFS");
                }
                Err(err) => {
                    tracing::warn!(target: "canvas_web", %err, "llm_web: битый JSON токенов — стор пуст");
                }
            },
            Ok(None) => {
                tracing::debug!(target: "canvas_web", "llm_web: файла токенов нет (не входил)")
            }
            Err(err) => {
                tracing::warn!(target: "canvas_web", ?err, "llm_web: чтение токенов из OPFS не удалось");
            }
        }
    }

    /// Снимок кэша + фоновая запись в OPFS (spawn_local; ошибки — warn,
    /// кэш остаётся источником правды до перезагрузки).
    fn persist(&self) {
        let snapshot = self.cache.lock().ok().and_then(|slot| slot.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(root) = crate::opfs::opfs_root().await else {
                tracing::warn!(target: "canvas_web", "llm_web: persist токенов — OPFS недоступен");
                return;
            };
            let text = serde_json::to_string(&snapshot).unwrap_or_else(|_| "null".into());
            if let Err(err) = crate::opfs::write_opfs_text(&root, TOKENS_FILE, &text).await {
                tracing::warn!(target: "canvas_web", ?err, "llm_web: запись токенов в OPFS не удалась");
            } else {
                tracing::debug!(target: "canvas_web", "llm_web: токены сохранены в OPFS");
            }
        });
    }
}

impl Default for OpfsTokenStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStore for OpfsTokenStore {
    fn save(&self, tokens: &OAuthTokens) -> Result<(), LlmError> {
        match self.cache.lock() {
            Ok(mut slot) => *slot = Some(tokens.clone()),
            Err(_) => return Err(LlmError::Auth("token store poisoned".into())),
        }
        self.persist();
        Ok(())
    }

    fn load(&self) -> Result<Option<OAuthTokens>, LlmError> {
        match self.cache.lock() {
            Ok(slot) => Ok(slot.clone()),
            Err(_) => Err(LlmError::Auth("token store poisoned".into())),
        }
    }

    fn clear(&self) -> Result<(), LlmError> {
        match self.cache.lock() {
            Ok(mut slot) => *slot = None,
            Err(_) => return Err(LlmError::Auth("token store poisoned".into())),
        }
        self.persist();
        Ok(())
    }
}

// ============================================================================
// OAuth-мост (W3 п.4): login popup / выход
// ============================================================================

/// Базовый URL cloud-прокси: localStorage-оверрайд [`PROXY_URL_KEY`] или
/// дефолт воркера из `wrangler.toml`. Битый оверрайд (без схемы) — дефолт.
fn proxy_base() -> String {
    let raw = window_storage_get(PROXY_URL_KEY).unwrap_or_default();
    let trimmed = raw.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        trimmed.trim_end_matches('/').to_owned()
    } else {
        DEFAULT_PROXY_URL.to_owned()
    }
}

/// Persistent device id: localStorage [`HOST_ID_KEY`], при отсутствии —
/// CSPRNG-генерация (PKCE-verifier как источник энтропии) и запись.
fn agent_host_id() -> String {
    if let Some(id) = window_storage_get(HOST_ID_KEY) {
        let id = id.trim().to_owned();
        if !id.is_empty() {
            return id;
        }
    }
    let generated = canvas_llm::chatgpt_oauth::pkce::generate_verifier()
        .unwrap_or_else(|_| format!("cd-{}", js_sys::Date::now() as u64));
    window_storage_set(HOST_ID_KEY, &generated);
    generated
}

/// OAuth-клиент web-пути: fetch-транспорт + pass-through прокси.
fn web_oauth_client() -> OAuthClient {
    OAuthClient::new(agent_host_id())
        .with_transport(Arc::new(canvas_llm::WasmFetchTransport))
        .with_proxy(Some(proxy_base()))
}

/// Мост OAuth для `App::set_web_oauth_bridge` (W2-сим, наполняется здесь).
struct WebLlmBridge {
    store: Arc<OpfsTokenStore>,
}

impl canvas_app::app::WebOAuthBridge for WebLlmBridge {
    /// Артефакты для фабрики провайдеров: OPFS-стор (кэш уже прогрет
    /// [`preload`], свежий после входа/выхода — фабрика читает store
    /// на каждый build). Host id фабрика берёт из настроек App.
    fn oauth_assets(&self) -> canvas_app::llm_factory::OAuthAssets {
        canvas_app::llm_factory::OAuthAssets {
            token_store: Some(self.store.clone()),
            ext_agent_host_id: String::new(),
        }
    }

    /// Web-флоу входа (PRD-0010 F-5.10): PKCE-сессия → popup на
    /// authorize-URL → OpenAI редиректит на `{proxy}/oauth/callback` →
    /// воркер 302 на URL приложения с `?oauth_callback=…` → страница
    /// перезагружается, обмен делает [`recover_oauth_callback`].
    fn start_login(&self) -> Result<(), String> {
        let redirect_uri = format!("{}/oauth/callback", proxy_base());
        let client = web_oauth_client();
        let session = client
            .build_login_session(&redirect_uri)
            .map_err(|err| err.to_string())?;
        // Несекретная часть сессии переживает перезагрузку страницы
        // (redirect-флоу); verifier — секрет, но localStorage в контексте
        // origin приложения — допустимый компромисс web-платформы
        // (дизайн-док §4.4 запрещает так хранить только API-токены).
        let stored = serde_json::json!({
            "verifier": session.verifier,
            "state": session.state,
            "nonce": session.nonce,
            "redirect_uri": session.redirect_uri,
        })
        .to_string();
        window_storage_set(OAUTH_SESSION_KEY, &stored);
        let window = web_sys::window().ok_or_else(|| "нет window".to_string())?;
        let popup = window.open_with_url_and_target(&session.login_url, "canvasdesk-oauth");
        if popup.is_err() {
            return Err(
                "всплывающее окно заблокировано браузером — разрешите popup для этого сайта"
                    .to_string(),
            );
        }
        tracing::info!(target: "canvas_web", "llm_web: oauth popup открыт");
        Ok(())
    }

    /// Выход: очистить OPFS-стор. Флаги бейджа в конфиге сбросит
    /// перезагрузка страницы (redirect-флоу симметричен входу).
    fn sign_out(&self) -> Result<(), String> {
        self.store.clear().map_err(|err| err.to_string())?;
        tracing::info!(target: "canvas_web", "llm_web: oauth выход — токены удалены");
        Ok(())
    }
}

// ============================================================================
// Callback-восстановление (W3 п.4): ?oauth_callback= → exchange → OPFS
// ============================================================================

/// Несекретная сохранённая сессия логина (JSON в localStorage).
#[derive(Debug, serde::Deserialize)]
struct StoredSession {
    verifier: String,
    state: String,
    #[serde(default)]
    #[allow(dead_code)]
    nonce: String,
    redirect_uri: String,
}

impl From<StoredSession> for canvas_llm::chatgpt_oauth::auth::LoginSession {
    fn from(s: StoredSession) -> Self {
        canvas_llm::chatgpt_oauth::auth::LoginSession {
            verifier: s.verifier,
            challenge: String::new(), // challenge нужен только в login URL
            state: s.state,
            nonce: s.nonce,
            redirect_uri: s.redirect_uri,
            login_url: String::new(),
        }
    }
}

/// Если страница открыта с `?oauth_callback=…` (302 воркера после логина) —
/// обменять code на токены, сохранить в OPFS и вписать флаги
/// `chatgpt_connected/chatgpt_email` в localStorage-конфиг (App прочитает
/// их при `load_settings` — бейдж 9-го таба «вход выполнен»). Любая ошибка
/// — warn и продолжение обычного старта (F-5.9: вход просто не выполнен).
/// URL чистится (`history.replaceState`) в любом исходе.
pub async fn recover_oauth_callback() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(search) = window.location().search() else {
        return;
    };
    if !search.contains("oauth_callback") {
        return;
    }
    let query = search.trim_start_matches('?');
    let outcome: Result<OAuthTokens, String> = (|| async {
        use canvas_llm::chatgpt_oauth::auth::{
            parse_callback_error, parse_callback_query, states_equal,
        };
        if let Some(error) = parse_callback_error(query) {
            return Err(format!("провайдер отказал во входе: {error}"));
        }
        let params = parse_callback_query(query).map_err(|err| err.to_string())?;
        let stored = window_storage_get(OAUTH_SESSION_KEY)
            .ok_or("нет сохранённой сессии логина (страница открыта вне флоу входа)")?;
        let session: StoredSession =
            serde_json::from_str(&stored).map_err(|err| format!("битая сессия логина: {err}"))?;
        if !states_equal(&session.state, &params.state) {
            return Err("state не совпал — callback отклонён (CSRF-защита)".to_string());
        }
        let client = web_oauth_client();
        client
            .exchange_code_async(&session.into(), &params.code)
            .await
            .map_err(|err| err.to_string())
    })()
    .await;
    // Стор колбэка независим от стора моста (App ещё не создан) — токены
    // уходят в OPFS, мост прогреется из файла в inject() (preload).
    let store = OpfsTokenStore::new();
    match outcome {
        Ok(tokens) => {
            let email = email_from_id_token(&tokens);
            let saved = store.save(&tokens).is_ok();
            sync_config_flags(true, email.as_deref());
            tracing::info!(
                target: "canvas_web",
                saved,
                "llm_web: oauth callback обработан — токены в OPFS"
            );
        }
        Err(error) => {
            tracing::warn!(target: "canvas_web", %error, "llm_web: oauth callback не прошёл");
            sync_config_flags(false, None);
        }
    }
    window_storage_remove(OAUTH_SESSION_KEY);
    clean_url_after_callback(&window, &search);
    // Чтобы флаги из синхронизированного конфига точно попали в App —
    // перезагрузка без oauth-параметров (redirect-флоу и так её делает).
    let _ = window.location().reload();
}

/// Обмен вернул токены только внутри замыкания — переигрывать обмен
/// нельзя (code одноразовый), поэтому email извлекается из `id_token`
/// здесь; токены пишет [`recover_oauth_callback`] через exchange-стор.
fn email_from_id_token(tokens: &OAuthTokens) -> Option<String> {
    canvas_llm::chatgpt_oauth::jwt::parse_id_token(&tokens.id_token)
        .ok()
        .and_then(|claims| claims.email)
}

// ============================================================================
// Инъекция швов в App (W3 п.2–3)
// ============================================================================

/// Активация LLM в web-сборке: wasm-executor (`spawn_local`), fetch-
/// транспорт, OAuth-мост с OPFS-стором (кэш прогрет из OPFS). Вызывается
/// из `spawn_desk_web` после построения App, до `spawn_app`.
pub async fn inject(app: &mut App) {
    // 1. Токен-стор (OPFS) + мост.
    let store = Arc::new(OpfsTokenStore::new());
    store.preload().await;
    app.set_web_oauth_bridge(Arc::new(WebLlmBridge { store }));
    // 2. Fetch-транспорт (единый шов W1: health/discovery/LlmProvider).
    app.set_llm_transport(Arc::new(canvas_llm::WasmFetchTransport));
    // 3. Wasm-шов executor'а: thunk → spawn_local (UI-тред, !Send-футуры
    // легальны — однонитевой рантайм wasm). Результат джобы кладётся в
    // инбокс future'ом; дренаж — llm_poll в about_to_wait.
    let spawner: canvas_app::llm_executor::LlmSpawnFn =
        Rc::new(|thunk: canvas_app::llm_executor::LlmJobThunk| {
            wasm_bindgen_futures::spawn_local(thunk());
        });
    app.set_llm_spawner(spawner);
    tracing::info!(
        target: "canvas_web",
        "llm_web: LLM-швы активированы (spawner/transport/opfs-store/oauth-bridge)"
    );
}

// ============================================================================
// localStorage / URL утилиты
// ============================================================================

fn window_storage_get(key: &str) -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()
        .flatten()?
        .get_item(key)
        .ok()
        .flatten()
}

fn window_storage_set(key: &str, value: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        if let Err(err) = storage.set_item(key, value) {
            tracing::warn!(target: "canvas_web", ?err, key, "llm_web: localStorage.set не удался");
        }
    }
}

fn window_storage_remove(key: &str) {
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.remove_item(key);
    }
}

/// Вписать флаги `chatgpt_connected/chatgpt_email` в localStorage-конфиг
/// (TOML `canvasdesk.config` — тот же путь, что `persist_settings_web`).
/// Битый конфиг — только warn (App поднимет дефолты, вход виден не будет).
fn sync_config_flags(connected: bool, email: Option<&str>) {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return;
    };
    let Ok(Some(text)) = storage.get_item("canvasdesk.config") else {
        tracing::warn!(target: "canvas_web", "llm_web: конфига нет — флаги входа не вписаны");
        return;
    };
    match toml::from_str::<canvas_core::Settings>(&text) {
        Ok(mut settings) => {
            settings.llm.chatgpt_connected = connected;
            settings.llm.chatgpt_email = email.unwrap_or_default().to_owned();
            match toml::to_string(&settings) {
                Ok(serialized) => {
                    if let Err(err) = storage.set_item("canvasdesk.config", &serialized) {
                        tracing::warn!(target: "canvas_web", ?err, "llm_web: конфиг не записан");
                    }
                }
                Err(err) => {
                    tracing::warn!(target: "canvas_web", %err, "llm_web: сериализация конфига")
                }
            }
        }
        Err(err) => tracing::warn!(target: "canvas_web", %err, "llm_web: разбор конфига"),
    }
}

/// `history.replaceState` без oauth-параметров (code/state/oauth_callback
/// не должны попасть в закладки/рефреш-повторы — code одноразовый).
fn clean_url_after_callback(window: &web_sys::Window, search: &str) {
    let params = web_sys::UrlSearchParams::new_with_str(search.trim_start_matches('?')).ok();
    if let Some(params) = params {
        for name in ["oauth_callback", "code", "state", "scope"] {
            let _ = params.delete(&name);
        }
        let rest = params.to_string().as_string().unwrap_or_default();
        let search_clean = if rest.is_empty() {
            String::new()
        } else {
            format!("?{rest}")
        };
        let path = window.location().pathname().unwrap_or_default();
        let url = format!("{path}{search_clean}");
        if let Err(err) = window.history().and_then(|history| {
            history
                .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url))
                .map_err(|err| err)
        }) {
            tracing::warn!(target: "canvas_web", ?err, "llm_web: чистка URL не удалась");
        }
    }
    let _ = DEEP_LINK_REDIRECT; // константа задокументирована в доке модуля
}
