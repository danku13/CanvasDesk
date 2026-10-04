//! FR-LLM-OAUTH-APP / PRD-0010 F-5.2+F-5.3+F-5.8 — OAuth-флоу приложения.
//!
//! Связывает готовый OAuth-клиент `canvas-llm::chatgpt_oauth` с UI
//! (9-й таб настроек «AI и модели», строка `SettingsRow::AiOAuth`):
//!
//! ```text
//! [UI] клик «Войти через ChatGPT»
//!   → App::oauth_start_login(): генерация ext_agent_host_id (один раз),
//!     bind localhost-листенера, build_login_session, открыть браузер
//!   → OAuthFlowState::Waiting { session }        (сразу, UI-тред)
//!   → worker-поток: wait_for_code → exchange_code → verify_id_token
//!     → сохранение токенов в TokenStore
//!   → [App] поллинг каждый кадр (about_to_wait, паттерн suggest):
//!     Connected { email } → settings.llm.chatgpt_connected/email + save
//! ```
//!
//! Отмена («Отменить» в Waiting) — bump generation: воркер после
//! разблокировки видит устаревший номер и отбрасывает результат. Воркер при
//! этом остаётся заблокирован в `accept()` до callback'а/выхода процесса —
//! известное ограничение v1 (листенер из canvas-llm блокирующий; TODO:
//! shutdown-механика в `chatgpt_oauth::auth::CallbackListener`).
//!
//! Токены — [`FileTokenStore`] поверх `oauth_tokens.json` рядом с
//! `config.toml` (`~/.canvasdesk/`), права 0600 на unix (дизайн-док §4.4:
//! токены НЕ в config.toml). TODO: OS keychain (Windows Credential Manager /
//! macOS Keychain / Secret Service) на desktop и OPFS encrypted на web —
//! по дизайн-доку §4.4 (файл 0600 — промежуточный шаг до keychain).
//!
//! Под wasm32 / без feature `l1-llm` модуль НЕ компилируется (ADR-0011:
//! web-путь — deep-link + cloud-proxy, F-5.10; UI показывает кнопку
//! недоступной).

// FR-LLM-OAUTH-APP: маркер для поиска (grep): файлы app-wiring OAuth помечены
// `// FR-LLM-OAUTH-APP:` в комментариях.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use canvas_llm::chatgpt_oauth::auth::{open_browser, CallbackListener, OAuthClient, CLIENT_ID};
use canvas_llm::chatgpt_oauth::jwt::{verify_id_token, IdClaims, NoopVerifier};
use canvas_llm::chatgpt_oauth::pkce::generate_verifier;
use canvas_llm::chatgpt_oauth::LoginSession;
use canvas_llm::{LlmError, OAuthTokens, TokenStore};

/// Состояние OAuth-флоу «Вход ChatGPT» (PRD-0010 F-5.8).
///
/// Живёт в `Arc<Mutex<…>>` внутри [`OAuthFlowHandle`] и опрашивается UI-тредом
/// каждый кадр (`about_to_wait`); переход Waiting→Connected также
/// синхронизирует персистентные флаги `LlmSettings.chatgpt_connected/email`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthFlowState {
    /// Вход не выполняется (кнопка «Войти через ChatGPT»).
    Idle,
    /// Ожидание подтверждения в браузере: листенер занял порт, login URL
    /// открыт/готов. `session` — PKCE/state/nonce до обмена кода.
    Waiting {
        /// Активная login-сессия (verifier нужен обмену кода в воркере;
        /// login_url — fallback «откройте ссылку вручную», F-5.9).
        session: LoginSession,
    },
    /// Вход выполнен: токены в store, `email` — claim `id_token` (пустая
    /// строка — claim не выдан, UI показывает «аккаунт»).
    Connected {
        /// E-mail аккаунта (или пустая строка).
        email: String,
    },
    /// Ошибка флоу (текст — для бейджа err в Settings + кнопка «Повторить»).
    Failed {
        /// Человекочитаемое описание ошибки (RU, из `LlmError`).
        error: String,
    },
}

/// Файловое хранилище OAuth-токенов (impl [`TokenStore`], PRD-0010 F-5.3).
///
/// Отдельный JSON-файл рядом с `config.toml` (`~/.canvasdesk/oauth_tokens.
/// json`) — токены НЕ попадают в config.toml (тот же принцип, что у BYOK-
/// ключа; дизайн-док §4.4). На unix файл создаётся с правами 0600 (чтение/
/// запись только владельцу); на Windows — обычный файл (ACL по умолчанию
/// профиля). TODO (дизайн-док §4.4): OS keychain на desktop / OPFS
/// encrypted на web.
///
/// Битый/чужой файл трактуется как «нет токенов» (graceful degradation
/// F-5.9: UI снова показывает «Войти»), ошибки записи — `LlmError::Auth`
/// (сессия несохраняема — тот же класс, что у `MemoryTokenStore`).
pub struct FileTokenStore {
    path: PathBuf,
}

impl FileTokenStore {
    /// Хранилище поверх файла `path` (создаётся при первом `save`).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl TokenStore for FileTokenStore {
    fn save(&self, tokens: &OAuthTokens) -> Result<(), LlmError> {
        let json = serde_json::to_string(tokens)
            .map_err(|e| LlmError::Auth(format!("oauth_tokens.json: сериализация: {e}")))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                LlmError::Auth(format!(
                    "oauth_tokens.json: каталог {} не создан: {e}",
                    parent.display()
                ))
            })?;
        }
        // FR-LLM-OAUTH-APP: 0600 на unix (дизайн-док §4.4 — файл читается
        // только владельцем); на Windows OpenOptions без mode.
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts
            .open(&self.path)
            .map_err(|e| LlmError::Auth(format!("oauth_tokens.json: открытие: {e}")))?;
        use std::io::Write;
        file.write_all(json.as_bytes())
            .and_then(|_| file.flush())
            .map_err(|e| LlmError::Auth(format!("oauth_tokens.json: запись: {e}")))
    }

    fn load(&self) -> Result<Option<OAuthTokens>, LlmError> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LlmError::Auth(format!("oauth_tokens.json: чтение: {e}"))),
        };
        if text.trim().is_empty() {
            // Пустой файл = нет сессии (идемпотентно с clear).
            return Ok(None);
        }
        match serde_json::from_str(&text) {
            Ok(tokens) => Ok(Some(tokens)),
            // Битый файл — не падаем: считаем сессии нет (F-5.9), warn выше.
            Err(_) => Ok(None),
        }
    }

    fn clear(&self) -> Result<(), LlmError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            // Нет файла — уже чисто (идемпотентность clear, как у memory).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(LlmError::Auth(format!("oauth_tokens.json: удаление: {e}"))),
        }
    }
}

/// Общее состояние флоу (шарится с воркер-потоком): state + номер сессии.
struct FlowShared {
    /// Текущее состояние (UI опрашивает каждый кадр).
    state: Mutex<OAuthFlowState>,
    /// Номер запуска флоу: воркер с устаревшим номером отбрасывает результат
    /// (механика отмены: «Отменить» = bump generation → результат discarded).
    generation: AtomicU64,
    /// Хранилище токенов (FileTokenStore / memory-фолбэк без config-пути).
    store: Arc<dyn TokenStore>,
}

impl FlowShared {
    /// Применить исход воркера: успех → токены в store + Connected,
    /// ошибка → Failed. Устаревший generation / не-Waiting state — discard
    /// (флоу был отменён или перезапущен).
    fn apply_outcome(&self, generation: u64, outcome: Result<(OAuthTokens, LoginSession), String>) {
        let mut state = match self.state.lock() {
            Ok(guard) => guard,
            Err(_) => return, // poisoned: флоу несостоятелен, UI покажет Idle
        };
        if self.generation.load(Ordering::Acquire) != generation {
            return; // отменено/перезапущено — результат устарел
        }
        if !matches!(*state, OAuthFlowState::Waiting { .. }) {
            return; // state уже сменили (cancel/sign_out) — discard
        }
        *state = match outcome {
            Ok((tokens, _session)) => {
                if let Err(err) = self.store.save(&tokens) {
                    OAuthFlowState::Failed {
                        error: format!("токены не сохранены: {err}"),
                    }
                } else {
                    OAuthFlowState::Connected {
                        email: tokens.account_email.unwrap_or_default(),
                    }
                }
            }
            Err(error) => OAuthFlowState::Failed { error },
        };
    }

    /// Отменить ожидание (кнопка «Отменить»): bump generation → результат
    /// воркера будет отброшен; state → Idle. Идемпотентно.
    fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut state) = self.state.lock() {
            if matches!(*state, OAuthFlowState::Waiting { .. }) {
                *state = OAuthFlowState::Idle;
            }
        }
    }

    /// Выйти (кнопка «Выйти» в Connected): очистить store + флаги, state →
    /// Idle. Bump generation — тоже (на случай Waiting из параллельного
    /// запуска).
    fn sign_out(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        let _ = self.store.clear();
        if let Ok(mut state) = self.state.lock() {
            *state = OAuthFlowState::Idle;
        }
    }
}

/// Хендл OAuth-флоу в `App` (натив + feature `l1-llm`).
///
/// Один инстанс на приложение; `&self`-методы — потокобезопасны. Не `Clone`:
/// владелец один (App), воркер держит только `Arc<FlowShared>`.
pub struct OAuthFlowHandle {
    shared: Arc<FlowShared>,
}

impl OAuthFlowHandle {
    /// Построить флоу: store — [`FileTokenStore`] в каталоге `config_dir`
    /// (рядом с `config.toml`, `~/.canvasdesk/oauth_tokens.json`) или
    /// memory-фолбэк (каталога нет — тесты/ограниченная среда; сессия живёт
    /// до перезапуска, F-5.9). Если store уже содержит токены — стартуем
    /// в `Connected`.
    pub fn new(config_dir: Option<&Path>) -> Self {
        let tokens_path = config_dir.map(|dir| dir.join("oauth_tokens.json"));
        let store: Arc<dyn TokenStore> = match &tokens_path {
            Some(path) => Arc::new(FileTokenStore::new(path.clone())),
            None => Arc::new(canvas_llm::MemoryTokenStore::new()),
        };
        let initial = match store.load() {
            Ok(Some(tokens)) => OAuthFlowState::Connected {
                email: tokens.account_email.unwrap_or_default(),
            },
            Ok(None) => OAuthFlowState::Idle,
            Err(err) => {
                tracing::warn!(%err, "oauth_tokens.json не прочитан — считаем вход не выполненным");
                OAuthFlowState::Idle
            }
        };
        Self {
            shared: Arc::new(FlowShared {
                state: Mutex::new(initial),
                generation: AtomicU64::new(0),
                store,
            }),
        }
    }

    /// Снимок состояния для UI (settings_ui.rs: бейдж/кнопка строки AiOAuth).
    pub fn state(&self) -> OAuthFlowState {
        match self.shared.state.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => OAuthFlowState::Idle, // poisoned — UI-безопасный фолбэк
        }
    }

    /// Хранилище токенов (для `OAuthAssets` фабрики провайдеров).
    pub fn token_store(&self) -> Arc<dyn TokenStore> {
        self.shared.store.clone()
    }

    /// FR-LLM-OAUTH-APP: сгенерировать persistent device id (CSPRNG через
    /// `pkce::generate_verifier` — ОС-энтропия; 43 символа unreserved).
    /// Один раз на установку: вызывающий (App) сохраняет в
    /// `LlmSettings.ext_agent_host_id` (config.toml).
    pub fn generate_ext_agent_host_id() -> Result<String, LlmError> {
        generate_verifier()
    }

    /// Пометить флоу ошибкой (setup-шаги на UI-треде): bump generation
    /// (воркеров этого запуска нет/устаревшие) + state → Failed.
    fn fail(&self, error: String) {
        let _ = self.shared.generation.fetch_add(1, Ordering::AcqRel);
        if let Ok(mut state) = self.shared.state.lock() {
            *state = OAuthFlowState::Failed { error };
        }
    }

    /// Запустить вход (клик «Войти через ChatGPT» / «Повторить»).
    ///
    /// Быстрые шаги (bind листенера, PKCE-сессия, спавн браузера) —
    /// синхронно на UI-треде (мгновенный переход `Waiting { session }`);
    /// блокирующее ожидание callback + обмен кода + валидация claims +
    /// сохранение токенов — в worker-потоке (правило «не блокировать
    /// рендер-поток», AGENTS.md). Ошибка любого шага → `Failed { error }`.
    ///
    /// `ext_agent_host_id` — persistent device id из `LlmSettings`
    /// (пустым быть не должно: App генерирует перед вызовом).
    pub fn start_login(&self, ext_agent_host_id: &str) {
        // Повторный клик в Waiting — игнор (кнопка в этот момент «Отменить»).
        if matches!(self.state(), OAuthFlowState::Waiting { .. }) {
            return;
        }
        // Быстрые шаги — синхронно на UI-треде (bind занимает порт мгновенно,
        // PKCE — чистый crypto, спавн браузера не ждёт); блокирующее ожидание
        // callback — только в воркере ниже. Ошибка любого шага → Failed
        // (кнопка станет «Повторить», F-5.9).
        let listener = match CallbackListener::bind() {
            Ok(listener) => listener,
            Err(e) => {
                self.fail(format!("листенер callback: {e}"));
                return;
            }
        };
        let setup_client = OAuthClient::new(ext_agent_host_id);
        let session = match setup_client.build_login_session(listener.redirect_uri()) {
            Ok(session) => session,
            Err(e) => {
                self.fail(format!("login-сессия: {e}"));
                return;
            }
        };
        // F-5.9: ошибка спавна браузера (нет xdg-open/open/start) — Failed
        // с текстом; login_url остаётся в логах (tracing).
        if let Err(e) = open_browser(&session.login_url) {
            self.fail(format!("не удалось открыть браузер: {e}"));
            return;
        }
        tracing::info!(
            port = listener.port(),
            "OAuth: браузер открыт, ожидание callback на localhost"
        );
        // Новый запуск = новый номер (воркеры прежних сессий — discard).
        let generation = {
            let previous = self.shared.generation.fetch_add(1, Ordering::AcqRel);
            previous + 1
        };
        if let Ok(mut state) = self.shared.state.lock() {
            *state = OAuthFlowState::Waiting {
                session: session.clone(),
            };
        }
        // Воркер: ждать callback (блокирующе) → обмен кода → валидация
        // claims id_token → результат в shared. Листенер/сессия переезжают
        // в поток (Send: TcpListener + строки).
        let shared = self.shared.clone();
        let client = OAuthClient::new(ext_agent_host_id);
        let spawn = std::thread::Builder::new()
            .name("oauth-login".to_owned())
            .spawn(move || {
                let outcome = complete_desktop_login(&client, &listener, &session);
                shared.apply_outcome(generation, outcome);
            });
        if let Err(err) = spawn {
            // Поток не стартовал: флоу не завершится сам — Failed сразу.
            self.fail(format!("воркер OAuth не запущен: {err}"));
        }
    }

    /// Отменить ожидание (кнопка «Отменить» в Waiting).
    ///
    /// Воркер остаётся заблокированным в accept до callback/выхода процесса
    /// (см. модуль-док) — результат будет отброшен по generation.
    pub fn cancel(&self) {
        self.shared.cancel();
    }

    /// Выйти (кнопка «Выйти» в Connected): очистить store; state → Idle.
    pub fn sign_out(&self) {
        self.shared.sign_out();
    }
}

/// Хвост desktop-login в воркере: обмен кода → валидация claims id_token
/// (iss/aud/nonce/exp; подпись — `NoopVerifier`, v1 SECURITY-стаб canvas-llm)
/// → готово к сохранению. Ошибки — человекочитаемая строка для бейджа err.
fn complete_desktop_login(
    client: &OAuthClient,
    listener: &CallbackListener,
    session: &LoginSession,
) -> Result<(OAuthTokens, LoginSession), String> {
    // Шаги 3–5 flow (дизайн-док §4.2): callback (state-проверка/CSRF внутри
    // wait_for_code) → обмен кода на токены.
    let callback = listener
        .wait_for_code(&session.state)
        .map_err(|e| e.to_string())?;
    let tokens = client
        .exchange_code(session, &callback.code)
        .map_err(|e| e.to_string())?;
    // Шаг 6 — claims-валидация обязательна (replay-защита nonce, exp).
    // Ключей JWKS нет (v1): пустой набор + NoopVerifier → только claims
    // (см. SECURITY-блок chatgpt_oauth/jwt.rs).
    let now = canvas_llm::chatgpt_oauth::auth::now_unix().map_err(|e| e.to_string())?;
    let _claims: IdClaims = verify_id_token(
        &tokens.id_token,
        Some(&session.nonce),
        CLIENT_ID,
        now,
        &NoopVerifier,
        &[],
    )
    .map_err(|e| e.to_string())?;
    Ok((tokens, session.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_llm::MemoryTokenStore;

    /// Уникальный tmp-путь для теста (без сети/без GPU; удаляется в конце).
    fn scratch_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "canvasdesk-oauth-test-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ))
    }

    fn sample_tokens() -> OAuthTokens {
        OAuthTokens {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            id_token: "hdr.pld.sig".into(),
            expires_at: 1_700_000_000,
            account_email: Some("user@example.com".into()),
        }
    }

    /// FileTokenStore round-trip: save → load возвращает те же токены,
    /// clear → load возвращает None (повторный clear идемпотентен).
    #[test]
    fn file_token_store_roundtrip() {
        let path = scratch_path("roundtrip");
        let store = FileTokenStore::new(path.clone());
        assert_eq!(store.load().expect("load пустой"), None);
        store.save(&sample_tokens()).expect("save");
        assert_eq!(store.load().expect("load"), Some(sample_tokens()));
        // Перезапись второй сессией.
        let mut second = sample_tokens();
        second.access_token = "at2".into();
        store.save(&second).expect("save2");
        assert_eq!(store.load().expect("load2").unwrap().access_token, "at2");
        store.clear().expect("clear");
        assert_eq!(store.load().expect("load после clear"), None);
        assert!(store.clear().is_ok(), "повторный clear идемпотентен");
        let _ = std::fs::remove_file(&path);
    }

    /// Права файла токенов — 0600 на unix (дизайн-док §4.4).
    #[cfg(unix)]
    #[test]
    fn file_token_store_mode_0600_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        let path = scratch_path("mode");
        let store = FileTokenStore::new(path.clone());
        store.save(&sample_tokens()).expect("save");
        let mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "файл токенов доступен только владельцу");
        let _ = std::fs::remove_file(&path);
    }

    /// Битый JSON в файле трактуется как «нет сессии» (F-5.9), а не ошибка.
    #[test]
    fn file_token_store_corrupt_file_loads_none() {
        let path = scratch_path("corrupt");
        std::fs::write(&path, "{не json").expect("запись мусора");
        let store = FileTokenStore::new(path.clone());
        assert_eq!(store.load().expect("load битого файла"), None);
        let _ = std::fs::remove_file(&path);
    }

    /// Хендл без каталога конфига: memory-фолбэк, старт — Idle.
    #[test]
    fn handle_without_config_path_starts_idle() {
        let handle = OAuthFlowHandle::new(None);
        assert_eq!(handle.state(), OAuthFlowState::Idle);
    }

    /// Хендл с каталогом конфига: token store пишет файл oauth_tokens.json
    /// именно в него (поведенческая проверка пути).
    #[test]
    fn handle_uses_config_dir_for_token_file() {
        let dir = scratch_path("dir");
        let handle = OAuthFlowHandle::new(Some(&dir));
        handle
            .token_store()
            .save(&sample_tokens())
            .expect("save через handle");
        assert!(
            dir.join("oauth_tokens.json").exists(),
            "файл в каталоге конфига"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Переходы OAuthFlowState (без сети/потоков): apply_outcome с текущим
    /// generation → Connected (email из токенов); с устаревшим — discard.
    #[test]
    fn flow_state_transitions_connect_and_discard() {
        let shared = Arc::new(FlowShared {
            state: Mutex::new(OAuthFlowState::Idle),
            generation: AtomicU64::new(0),
            store: Arc::new(MemoryTokenStore::new()),
        });
        // "start": state → Waiting, generation 1.
        let generation = shared.generation.fetch_add(1, Ordering::AcqRel) + 1;
        *shared.state.lock().expect("lock") = OAuthFlowState::Waiting {
            session: login_session_stub(),
        };
        // Устаревший воркер (generation 0) — результат отброшен.
        shared.apply_outcome(0, Err("поздний отказ".into()));
        assert!(matches!(
            *shared.state.lock().expect("lock"),
            OAuthFlowState::Waiting { .. }
        ));
        // Успех с текущим generation → Connected + токены в store.
        let tokens = sample_tokens();
        shared.apply_outcome(generation, Ok((tokens, login_session_stub())));
        assert_eq!(
            *shared.state.lock().expect("lock"),
            OAuthFlowState::Connected {
                email: "user@example.com".into()
            }
        );
        assert!(shared.store.load().expect("load").is_some());
    }

    /// Отмена: cancel() возвращает Idle, а поздний успех воркера отбрасывается
    /// (токены НЕ попадают в store).
    #[test]
    fn flow_state_cancel_discards_late_success() {
        let shared = Arc::new(FlowShared {
            state: Mutex::new(OAuthFlowState::Waiting {
                session: login_session_stub(),
            }),
            generation: AtomicU64::new(1),
            store: Arc::new(MemoryTokenStore::new()),
        });
        shared.cancel();
        assert_eq!(*shared.state.lock().expect("lock"), OAuthFlowState::Idle);
        // Поздний успех (старый generation) — discard, store пуст.
        shared.apply_outcome(1, Ok((sample_tokens(), login_session_stub())));
        assert_eq!(*shared.state.lock().expect("lock"), OAuthFlowState::Idle);
        assert!(shared.store.load().expect("load").is_none());
    }

    /// Ошибка воркера с текущим generation → Failed (текст для бейджа err).
    #[test]
    fn flow_state_failure_sets_failed() {
        let shared = Arc::new(FlowShared {
            state: Mutex::new(OAuthFlowState::Waiting {
                session: login_session_stub(),
            }),
            generation: AtomicU64::new(3),
            store: Arc::new(MemoryTokenStore::new()),
        });
        shared.apply_outcome(3, Err("state из callback не совпал (CSRF)".into()));
        assert_eq!(
            *shared.state.lock().expect("lock"),
            OAuthFlowState::Failed {
                error: "state из callback не совпал (CSRF)".into()
            }
        );
    }

    /// Sign out: store очищен, state → Idle.
    #[test]
    fn flow_state_sign_out_clears_store() {
        let store = Arc::new(MemoryTokenStore::new());
        store.save(&sample_tokens()).expect("save");
        let shared = Arc::new(FlowShared {
            state: Mutex::new(OAuthFlowState::Connected {
                email: "user@example.com".into(),
            }),
            generation: AtomicU64::new(0),
            store,
        });
        shared.sign_out();
        assert_eq!(*shared.state.lock().expect("lock"), OAuthFlowState::Idle);
        assert!(shared.shared_store_is_empty());
    }

    impl FlowShared {
        /// Хелпер теста: store пуст?
        fn shared_store_is_empty(&self) -> bool {
            self.store.load().expect("load").is_none()
        }
    }

    /// Стаб login-сессии (PKCE-значения не проверяются в этих тестах).
    fn login_session_stub() -> LoginSession {
        LoginSession {
            verifier: "v".repeat(43),
            challenge: "c".repeat(43),
            state: "st".into(),
            nonce: "nn".into(),
            redirect_uri: "http://127.0.0.1:8765/callback".into(),
            login_url: "https://chatgpt.com/auth/login?…".into(),
        }
    }

    /// Генератор device id: 43 символа unreserved, два вызова различны
    /// (тот же CSPRNG, что PKCE-verifier).
    #[test]
    fn generated_ext_agent_host_id_shape() {
        let id1 = OAuthFlowHandle::generate_ext_agent_host_id().expect("CSPRNG");
        let id2 = OAuthFlowHandle::generate_ext_agent_host_id().expect("CSPRNG");
        assert_eq!(id1.len(), 43);
        assert_ne!(id1, id2);
        assert!(id1.bytes().all(|c| c.is_ascii_alphanumeric()
            || c == b'-'
            || c == b'_'
            || c == b'.'
            || c == b'~'));
    }
}
