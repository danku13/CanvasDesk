//! FR-LLM-OAUTH — хранилище OAuth-токенов (`TokenStore`).
//!
//! Трейт абстрагирует место хранения access/refresh/id-токенов Sign-in-with-
//! ChatGPT (PRD-0010 F-5.3): **в продукте `canvas-app` реализует store поверх
//! OS keychain** (Windows Credential Manager / macOS Keychain / Linux Secret
//! Service), **web — OPFS encrypted**; хранить токены в `localStorage`
//! ЗАПРЕЩЕНО (XSS-риск, дизайн-док §4.4). API-токены НЕ попадают в
//! `config.toml` (тот же принцип, что у BYOK-ключа — `settings.rs`).
//!
//! `MemoryTokenStore` — референс-реализация для тестов и фолбэка (токены
//! живут до перезапуска процесса; refresh тогда невозможен → UI снова
//! показывает «Войти» — graceful degradation PRD-0010 F-5.9).

use crate::error::LlmError;

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

/// Набор OAuth-токенов Sign-in-with-ChatGPT (ответ token endpoint +
/// вычисленное время истечения). Сериализуем за feature `serde` — так
/// продукт-хранилище (keychain/OPFS) может класть структуру как JSON-блоб.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OAuthTokens {
    /// Bearer-токен для Responses API (`Authorization: Bearer <access_token>`).
    pub access_token: String,
    /// Долгоживущий токен обновления (`offline_access`, дизайн-док §4.3).
    /// Истёк/отозван → `LlmError::Auth` → UI «войти снова» (F-5.9).
    pub refresh_token: String,
    /// ID-токен (JWT) с профилем пользователя; claims валидируются
    /// (`chatgpt_oauth::jwt`) — issuer/audience/nonce/exp.
    pub id_token: String,
    /// Абсолютное время истечения `access_token` (unix, сек) =
    /// `now + expires_in` из ответа token endpoint.
    pub expires_at: u64,
    /// E-mail аккаунта (claim `email` из id_token) — для отображения в
    /// Settings («Signed in as …»). `None` — claim не выдан.
    pub account_email: Option<String>,
}

/// Хранилище OAuth-токенов (PRD-0010 F-5.3).
///
/// Реализации:
/// - [`MemoryTokenStore`] — в памяти (тесты/фолбэк);
/// - продуктовые (вне крейта): OS keychain (desktop) / OPFS encrypted (web)
///   — см. док-комментарий модуля.
///
/// Все методы принимают `&self` и не паникуют: store используется из
/// `Box<dyn TokenStore>` внутри [`crate::ChatGptOAuthProvider`] (Send + Sync).
pub trait TokenStore: Send + Sync {
    /// Сохранить/заменить набор токенов.
    fn save(&self, tokens: &OAuthTokens) -> Result<(), LlmError>;

    /// Загрузить токены (`None` — пользователь ещё не входил).
    fn load(&self) -> Result<Option<OAuthTokens>, LlmError>;

    /// Удалить токены (logout / невосстановимый refresh-failure, F-5.9).
    fn clear(&self) -> Result<(), LlmError>;
}

/// In-memory реализация [`TokenStore`] (`Mutex<Option<OAuthTokens>>`).
///
/// Для unit-тестов и как фолбэк, если keychain недоступен (тогда сессия
/// живёт до перезапуска — задокументировано в модуль-доке).
#[derive(Default)]
pub struct MemoryTokenStore {
    inner: std::sync::Mutex<Option<OAuthTokens>>,
}

impl MemoryTokenStore {
    /// Пустое хранилище (нет залогиненного пользователя).
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MemoryTokenStore {
    fn save(&self, tokens: &OAuthTokens) -> Result<(), LlmError> {
        // Mutex::lock → PoisonError только при панике в другом потоке;
        // не паникуем сами — ошибка сохранения как Auth-класс (сессия
        // несохраняема → безопаснее считать её невалидной).
        match self.inner.lock() {
            Ok(mut slot) => {
                *slot = Some(tokens.clone());
                Ok(())
            }
            Err(_) => Err(LlmError::Auth("token store poisoned".into())),
        }
    }

    fn load(&self) -> Result<Option<OAuthTokens>, LlmError> {
        match self.inner.lock() {
            Ok(slot) => Ok(slot.clone()),
            Err(_) => Err(LlmError::Auth("token store poisoned".into())),
        }
    }

    fn clear(&self) -> Result<(), LlmError> {
        match self.inner.lock() {
            Ok(mut slot) => {
                *slot = None;
                Ok(())
            }
            Err(_) => Err(LlmError::Auth("token store poisoned".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tokens() -> OAuthTokens {
        OAuthTokens {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            id_token: "hdr.pld.sig".into(),
            expires_at: 1_700_000_000,
            account_email: Some("user@example.com".into()),
        }
    }

    #[test]
    fn memory_store_roundtrip() {
        let store = MemoryTokenStore::new();
        assert_eq!(store.load().unwrap(), None);
        store.save(&sample_tokens()).unwrap();
        assert_eq!(store.load().unwrap(), Some(sample_tokens()));
    }

    #[test]
    fn memory_store_overwrite_and_clear() {
        let store = MemoryTokenStore::new();
        store.save(&sample_tokens()).unwrap();
        let mut t2 = sample_tokens();
        t2.access_token = "at2".into();
        store.save(&t2).unwrap();
        assert_eq!(store.load().unwrap().unwrap().access_token, "at2");
        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
        // Повторный clear идемпотентен.
        assert!(store.clear().is_ok());
    }
}
