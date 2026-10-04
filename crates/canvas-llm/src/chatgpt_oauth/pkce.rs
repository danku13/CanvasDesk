//! FR-LLM-OAUTH — PKCE (Proof Key for Code Exchange, RFC 7636).
//!
//! Sign-in-with-ChatGPT (дизайн-док
//! `docs/dev-researches/byok-chatgpt-oauth-design.md` §4.2) требует PKCE
//! с методом **S256**:
//!
//! ```text
//! code_verifier  = 43..128 символов из unreserved-алфавита (RFC 7636 §4.1)
//! code_challenge = BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))   (§4.2)
//! ```
//!
//! `code_verifier` генерируется из 32 случайных байт (`getrandom` — ОС-энтропия;
//! time/address-хаки запрещены): base64url(32 байта) = ровно 43 символа,
//! все — из unreserved-алфавита. Тем же генератором делаются `state`
//! (CSRF-защита, RFC 6749 §10.12) и `nonce` (replay-защита id_token, OIDC Core §3.1.2.1).
//!
//! Случайность: feature `l1-llm` подключает optional dep `getrandom = "0.3"`.
//! Ошибка энтропии → `LlmError::Protocol` (не паникуем — правило репо).

use crate::error::LlmError;
use sha2::{Digest, Sha256};

// FR-LLM-OAUTH: константы PKCE — RFC 7636 §4.1/§4.2.
/// Минимальная длина `code_verifier` (символов) — RFC 7636 §4.1.
pub const VERIFIER_MIN_LEN: usize = 43;
/// Максимальная длина `code_verifier` (символов) — RFC 7636 §4.1.
pub const VERIFIER_MAX_LEN: usize = 128;
/// Метод преобразования challenge — только S256 (plain запрещён RFC 7636 §4.2
/// и не принимается ChatGPT OAuth; дизайн-док §4.2).
pub const CODE_CHALLENGE_METHOD: &str = "S256";

/// Символы `unreserved` из RFC 7636 §4.1: ALPHA / DIGIT / "-" / "." / "_" / "~".
fn is_unreserved(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~')
}

/// Валиден ли `code_verifier` по RFC 7636 §4.1 (длина 43..=128, unreserved).
///
/// Публично: приложение может проверить верификатор перед обменом кода
/// (ранний отказ вместо 400 от token endpoint).
pub fn is_valid_verifier(verifier: &str) -> bool {
    let len = verifier.len();
    (VERIFIER_MIN_LEN..=VERIFIER_MAX_LEN).contains(&len) && verifier.bytes().all(is_unreserved)
}

/// Вычислить `code_challenge` для `verifier` (метод S256):
/// `BASE64URL(SHA256(ASCII(verifier)))` без паддинга — RFC 7636 §4.2.
///
/// Чистая функция от входа (тестовый вектор — RFC 7636 Appendix B).
pub fn challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    super::b64::encode(&digest)
}

/// Сгенерировать `code_verifier`: 32 случайных байта → base64url (43 символа).
///
/// Источник энтропии — `getrandom` (ОС CSPRNG: getentropy/BCryptGenRandom/
/// /dev/urandom; на wasm — за cloud-proxy см. модуль-док). Все 43 символа
/// гарантированно из unreserved-алфавита → `is_valid_verifier` истинно.
pub fn generate_verifier() -> Result<String, LlmError> {
    random_b64url(32)
}

/// Сгенерировать `state`/`nonce`: 24 случайных байта → base64url (32 символа).
///
/// `state` проверяется в callback на равенство (см.
/// [`crate::chatgpt_oauth::auth::states_equal`]), `nonce` — в claims
/// `id_token` (см. [`crate::chatgpt_oauth::jwt`]).
pub fn generate_state() -> Result<String, LlmError> {
    random_b64url(24)
}

/// Сгенерировать `nonce` (OIDC replay-защита) — тот же генератор, что `state`.
pub fn generate_nonce() -> Result<String, LlmError> {
    random_b64url(24)
}

/// N случайных байт → base64url без паддинга. Общая точка входа энтропии.
fn random_b64url(n: usize) -> Result<String, LlmError> {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).map_err(|e| {
        // Ошибка CSPRNG — не сетевая и не auth-ошибка; классифицируем как
        // протокольную (OAuth-сессию построить нельзя) без паники.
        LlmError::Protocol(format!("криптогенератор недоступен (getrandom): {e}"))
    })?;
    Ok(super::b64::encode(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тестовый вектор RFC 7636 Appendix B — обязательная проверка S256.
    #[test]
    fn rfc7636_appendix_b_vector() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert!(is_valid_verifier(verifier));
        assert_eq!(
            challenge_from_verifier(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_validation_boundaries() {
        // 42 символа — короче минимума; 129 — длиннее максимума; 43/128 — ок.
        assert!(!is_valid_verifier(&"a".repeat(42)));
        assert!(is_valid_verifier(&"a".repeat(43)));
        assert!(is_valid_verifier(&"a".repeat(128)));
        assert!(!is_valid_verifier(&"a".repeat(129)));
        // Недопустимые символы (не unreserved): '+', '/', '='.
        assert!(!is_valid_verifier(&(format!("{}+/=", "a".repeat(42)))));
        assert!(!is_valid_verifier("")); // пустой
                                         // Кириллица — не unreserved.
        assert!(!is_valid_verifier(&"а".repeat(43)));
    }

    #[test]
    fn challenge_is_base64url_nopad_sha256() {
        // Challenge всегда 43 символа для любого verifier (SHA256 → 32 байта).
        let ch = challenge_from_verifier(&"x".repeat(43));
        assert_eq!(ch.len(), 43);
        assert!(!ch.contains('='));
        assert!(ch
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'));
    }

    #[test]
    fn generated_verifier_is_valid_and_unique() {
        // Требует CSPRNG (getrandom за feature l1-llm). Сеть не нужна.
        let v1 = generate_verifier().expect("CSPRNG доступен");
        let v2 = generate_verifier().expect("CSPRNG доступен");
        assert!(is_valid_verifier(&v1), "{v1}");
        assert!(is_valid_verifier(&v2), "{v2}");
        assert_ne!(v1, v2, "два вызова CSPRNG не должны совпадать");
    }

    #[test]
    fn generated_state_nonce_shape() {
        let s = generate_state().expect("CSPRNG доступен");
        let n = generate_nonce().expect("CSPRNG доступен");
        assert_eq!(s.len(), 32);
        assert_eq!(n.len(), 32);
        assert_ne!(s, n);
    }
}
