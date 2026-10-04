//! FR-LLM-OAUTH — разбор и валидация `id_token` (OIDC) + JWKS-структуры.
//!
//! Token endpoint Sign-in-with-ChatGPT возвращает `id_token` — JWT
//! (RFC 7519) `header.payload.signature` в base64url (дизайн-док §4.2
//! шаг 6: «verify id_token — issuer, audience, nonce»). Здесь:
//!
//! 1. Минимальный разбор JWT без крейта `jsonwebtoken` (зависимостный
//!    бюджет zero-dep-крейта, ADR-0011): split по '.', base64url-декод
//!    ([`super::b64`]), JSON-claims через `serde_json` (за feature `serde`,
//!    транзитивной из `l1-llm`).
//! 2. Валидация claims: `iss` (issuer), `aud` (client_id приложения),
//!    `nonce` (привязка к login-сессии, replay-защита), `exp` (leeway 30 c).
//! 3. Проверка подписи RS256 — через трейт [`SignatureVerifier`].
//!
//! # SECURITY: signature verification stub, v1 preview limitation
//!
//! Дефолтная реализация [`NoopVerifier`] **НЕ проверяет подпись** id_token
//! — это осознанное ограничение v1 (design-док §4.7 «Preview limitations»:
//! API Sign-in-with-ChatGPT на момент cookbook помечен preview). Полная
//! RS256/JWKS-верификация требует крейта `rsa` (+num-bigint-dig) и
//! доверенного транспорта JWKS — подключается отдельной задачей, когда
//! OpenAI выведет flow из preview и зафиксирует JWKS-эндпоинт
//! (`{auth_host}/.well-known/jwks.json`). Продуктовый риск ограничен:
//! токен получен сервер-сервер HTTPS-обменом с token endpoint OpenAI
//! (не с пользовательского ввода), nonce-привязка и HTTPS-канал
//! закрывают replay/перехват; подмена `id_token` третьей стороной
//! детектируется по несовпадению `nonce`/`aud`/`exp`. Тем не менее
//! **целостность claims без проверки подписи не гарантирована** —
//! сигнатура-стаб помечен этим блоком и обязателен к замене до 1.0.

use crate::error::LlmError;

// FR-LLM-OAUTH: маркер для поиска (grep): файлы Stream D помечены
// `// FR-LLM-OAUTH:` в комментариях.

/// Ожидаемый issuer `id_token` (OpenAI auth-сервис). Публичная константа:
/// приложения и тесты сверяют claim `iss` с этим значением.
pub const EXPECTED_ISSUER: &str = "https://auth.openai.com";

/// Ожидаемый алгоритм подписи (OpenAI выдаёт RS256; design-док §4.2).
pub const EXPECTED_ALG: &str = "RS256";

/// Допуск на расхождение часов при проверке `exp` (сек).
/// Стандартная практика (например, `jsonwebtoken` default leeway 60);
/// взят 30 c — компромисс между false-positive refresh и окном replay.
pub const EXP_LEEWAY_SECS: u64 = 30;

/// Заголовок JWT (декодированный `header`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwtHeader {
    /// Алгоритм подписи (`alg`), например `RS256`.
    pub alg: String,
    /// Идентификатор ключа (`kid`) для выбора JWK из JWKS-набора.
    pub kid: Option<String>,
}

/// Claims из payload `id_token` (подмножество OIDC Standard Claims).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdClaims {
    /// `iss` — издатель токена; сверяется с [`EXPECTED_ISSUER`].
    pub issuer: String,
    /// `aud` — аудитория (client_id приложения).
    pub audience: String,
    /// `sub` — стабильный идентификатор пользователя (может не быть).
    pub subject: Option<String>,
    /// `nonce` — привязка к login-сессии (replay-защита, OIDC Core §3.1.2.1).
    pub nonce: Option<String>,
    /// `email` — отображается в Settings («Signed in as …»).
    pub email: Option<String>,
    /// `exp` — время истечения (unix, сек).
    pub expiry: u64,
    /// `iat` — время выпуска (unix, сек); информационное.
    pub issued_at: Option<u64>,
}

/// Верификатор подписи JWT (RS256). Трейт изолирует крипто-зависимость:
/// пока `rsa`-крейт не подключён (см. SECURITY-блок модуль-дока), живёт
/// no-op реализация; после — продуктовая с JWKS-кэшем.
pub trait SignatureVerifier: Send + Sync {
    /// Проверить подпись: `signing_input` — `header.payload` (те самые байты
    /// исходного токена), `signature` — декодированные base64url-байты
    /// третьего сегмента, `key` — JWK по `kid` (`None` — ключ не найден;
    /// настоящий верификатор обязан отказать, no-op — пропустить).
    fn verify(
        &self,
        signing_input: &str,
        signature: &[u8],
        key: Option<&JwksKey>,
    ) -> Result<(), LlmError>;
}

/// No-op верификатор: всегда `Ok(())`. Дефолт для [`verify_id_token`].
///
/// FR-LLM-OAUTH / SECURITY: **signature verification stub** (v1 preview
/// limitation, дизайн-док §4.7). Подпись НЕ проверяется — см. SECURITY-блок
/// модуль-дока; замена на RS256-верификатор с JWKS-кэшем обязательна до 1.0.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopVerifier;

impl SignatureVerifier for NoopVerifier {
    fn verify(
        &self,
        _signing_input: &str,
        _signature: &[u8],
        _key: Option<&JwksKey>,
    ) -> Result<(), LlmError> {
        // SECURITY: заглушка — подпись НЕ проверяется (см. SECURITY-блок
        // модуль-дока и дизайн-док §4.7 «Preview limitations»). Настоящий
        // верификатор при key == None обязан вернуть LlmError::Auth.
        Ok(())
    }
}

/// Ключ JWKS-набора (`GET {auth_host}/.well-known/jwks.json`, дизайн-док
/// §4.4 п.1). RSA-параметры хранятся в base64url-кодировке как в JWK
/// (RFC 7517 §5): `n` — модуль, `e` — публичная экспонента.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwksKey {
    /// `kid` — совпадает с `JwtHeader.kid`.
    pub kid: String,
    /// `kty` — тип ключа; ожидается `RSA`.
    pub kty: String,
    /// `alg` — алгоритм; может отсутствовать в JWK (тогда из заголовка).
    pub alg: Option<String>,
    /// `n` — RSA-модуль (base64url, big-endian).
    pub n: String,
    /// `e` — RSA-экспонента (base64url, big-endian, обычно `AQAB` = 65537).
    pub e: String,
}

/// Разобрать JWKS-ответ (JSON со массивом `keys`) в список [`JwksKey`].
///
/// Неизвестные `kty` пропускаются (RS256-верификатор возьмёт только RSA).
pub fn parse_jwks(body: &serde_json::Value) -> Result<Vec<JwksKey>, LlmError> {
    let keys = body
        .get("keys")
        .and_then(|v| v.as_array())
        .ok_or_else(|| LlmError::Protocol("JWKS: нет массива keys".into()))?;
    Ok(keys
        .iter()
        .filter_map(|k| {
            Some(JwksKey {
                kid: k.get("kid")?.as_str()?.to_string(),
                kty: k.get("kty")?.as_str()?.to_string(),
                alg: k.get("alg").and_then(|a| a.as_str()).map(str::to_string),
                n: k.get("n")?.as_str()?.to_string(),
                e: k.get("e")?.as_str()?.to_string(),
            })
        })
        .collect())
}

/// Разбить JWT на 3 сегмента и декодировать заголовок.
fn parse_header(token: &str) -> Result<(JwtHeader, String, String), LlmError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return Err(LlmError::Auth(
            "id_token: ожидалось 3 сегмента header.payload.signature".into(),
        ));
    }
    let header_bytes = super::b64::decode(parts[0])
        .ok_or_else(|| LlmError::Auth("id_token: битый base64url в header".into()))?;
    let header_val: serde_json::Value = serde_json::from_slice(&header_bytes)
        .map_err(|e| LlmError::Auth(format!("id_token: битый JSON header: {e}")))?;
    let alg = header_val
        .get("alg")
        .and_then(|v| v.as_str())
        .ok_or_else(|| LlmError::Auth("id_token: header без alg".into()))?
        .to_string();
    let kid = header_val
        .get("kid")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Ok((
        JwtHeader { alg, kid },
        parts[0].to_string(),
        parts[2].to_string(),
    ))
}

/// Разобрать JWT и вернуть claims (без проверки подписи — см.
/// [`verify_id_token`] для полной валидации). Полезно в UI, чтобы
/// показать e-mail до завершения проверки JWKS.
pub fn parse_id_token(token: &str) -> Result<IdClaims, LlmError> {
    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| LlmError::Auth("id_token: нет payload-сегмента".into()))?;
    let bytes = super::b64::decode(payload)
        .ok_or_else(|| LlmError::Auth("id_token: битый base64url в payload".into()))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| LlmError::Auth(format!("id_token: битый JSON payload: {e}")))?;
    // `aud` может быть строкой или массивом (RFC 7519 §4.1.3) — берём
    // первый элемент массива.
    let audience = match v.get("aud") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(a)) => a
            .first()
            .and_then(|x| x.as_str())
            .ok_or_else(|| LlmError::Auth("id_token: aud-массив пуст".into()))?
            .to_string(),
        _ => return Err(LlmError::Auth("id_token: нет claim aud".into())),
    };
    Ok(IdClaims {
        issuer: v
            .get("iss")
            .and_then(|x| x.as_str())
            .ok_or_else(|| LlmError::Auth("id_token: нет claim iss".into()))?
            .to_string(),
        audience,
        subject: v.get("sub").and_then(|x| x.as_str()).map(str::to_string),
        nonce: v.get("nonce").and_then(|x| x.as_str()).map(str::to_string),
        email: v.get("email").and_then(|x| x.as_str()).map(str::to_string),
        expiry: v
            .get("exp")
            .and_then(|x| x.as_u64())
            .ok_or_else(|| LlmError::Auth("id_token: нет числового claim exp".into()))?,
        issued_at: v.get("iat").and_then(|x| x.as_u64()),
    })
}

/// Полная валидация `id_token`: структура, alg, подпись (через
/// [`SignatureVerifier`] + JWKS-ключ по `kid`), claims:
///
/// - `iss` == [`EXPECTED_ISSUER`];
/// - `aud` == `expected_audience` (client_id, с которым делался login);
/// - `nonce` == `expected_nonce`, если сессия его задавала (replay);
/// - `exp` > `now` - [`EXP_LEEWAY_SECS`] (просроченный → `LlmError::Auth`:
///   UI повторяет login, F-5.9).
///
/// `keys` — JWKS-набор; при пустом наборе и [`NoopVerifier`] валидация
/// ограничивается claims (см. SECURITY-блок модуль-дока).
pub fn verify_id_token(
    token: &str,
    expected_nonce: Option<&str>,
    expected_audience: &str,
    now: u64,
    verifier: &dyn SignatureVerifier,
    keys: &[JwksKey],
) -> Result<IdClaims, LlmError> {
    let (header, signing_input, sig_b64) = parse_header(token)?;
    if header.alg != EXPECTED_ALG {
        return Err(LlmError::Auth(format!(
            "id_token: неожиданный alg '{}' (ожидался {EXPECTED_ALG})",
            header.alg
        )));
    }
    // Подпись: выбираем JWK по kid (при отсутствии kid — единственный ключ).
    let key = match header.kid.as_deref() {
        Some(kid) => keys.iter().find(|k| k.kid == kid),
        None => keys.first(),
    };
    let sig_bytes = super::b64::decode(&sig_b64)
        .ok_or_else(|| LlmError::Auth("id_token: битая base64url подпись".into()))?;
    // Настоящий верификатор при отсутствующем ключе (ротация JWKS) сам
    // откажет; NoopVerifier (preview-стаб) продолжает claims-валидацию.
    verifier.verify(&signing_input, &sig_bytes, key)?;

    let claims = parse_id_token(token)?;
    validate_claims(&claims, expected_nonce, expected_audience, now)?;
    Ok(claims)
}

/// Валидация claims отдельно от транспорта/подписи (чистая функция —
/// покрывается тестами без сети).
pub fn validate_claims(
    claims: &IdClaims,
    expected_nonce: Option<&str>,
    expected_audience: &str,
    now: u64,
) -> Result<(), LlmError> {
    if claims.issuer != EXPECTED_ISSUER {
        return Err(LlmError::Auth(format!(
            "id_token: iss '{}' != '{}'",
            claims.issuer, EXPECTED_ISSUER
        )));
    }
    if claims.audience != expected_audience {
        return Err(LlmError::Auth(format!(
            "id_token: aud '{}' != client_id '{}'",
            claims.audience, expected_audience
        )));
    }
    if let Some(expected) = expected_nonce {
        if claims.nonce.as_deref() != Some(expected) {
            return Err(LlmError::Auth(
                "id_token: nonce не совпал с login-сессией (возможен replay)".into(),
            ));
        }
    }
    if claims.expiry <= now.saturating_sub(EXP_LEEWAY_SECS) {
        return Err(LlmError::Auth(format!(
            "id_token: истёк (exp {} <= now - {}s leeway)",
            claims.expiry, EXP_LEEWAY_SECS
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Собрать тестовый JWT: header/payload — JSON → base64url, подпись —
    /// фейковая (валидация подписи — NoopVerifier).
    fn make_token(header: &serde_json::Value, payload: &serde_json::Value) -> String {
        format!(
            "{}.{}.c2ln",
            super::super::b64::encode(serde_json::to_string(header).unwrap().as_bytes()),
            super::super::b64::encode(serde_json::to_string(payload).unwrap().as_bytes()),
        )
    }

    fn valid_payload(exp: u64) -> serde_json::Value {
        serde_json::json!({
            "iss": EXPECTED_ISSUER,
            "aud": "dynamic_agent_client",
            "sub": "user-1",
            "nonce": "n0nce",
            "email": "user@example.com",
            "exp": exp,
            "iat": exp - 3600,
        })
    }

    const NOW: u64 = 1_700_000_000;

    #[test]
    fn parse_id_token_extracts_claims() {
        let token = make_token(
            &serde_json::json!({"alg": "RS256", "kid": "k1"}),
            &valid_payload(NOW + 3600),
        );
        let claims = parse_id_token(&token).unwrap();
        assert_eq!(claims.issuer, EXPECTED_ISSUER);
        assert_eq!(claims.audience, "dynamic_agent_client");
        assert_eq!(claims.subject.as_deref(), Some("user-1"));
        assert_eq!(claims.nonce.as_deref(), Some("n0nce"));
        assert_eq!(claims.email.as_deref(), Some("user@example.com"));
        assert_eq!(claims.expiry, NOW + 3600);
        assert_eq!(claims.issued_at, Some(NOW));
    }

    #[test]
    fn verify_id_token_happy_path() {
        let token = make_token(
            &serde_json::json!({"alg": "RS256", "kid": "k1"}),
            &valid_payload(NOW + 3600),
        );
        let keys = vec![JwksKey {
            kid: "k1".into(),
            kty: "RSA".into(),
            alg: Some("RS256".into()),
            n: "mod".into(),
            e: "AQAB".into(),
        }];
        let claims = verify_id_token(
            &token,
            Some("n0nce"),
            "dynamic_agent_client",
            NOW,
            &NoopVerifier,
            &keys,
        )
        .expect("валидный токен должен пройти claims-валидацию");
        assert_eq!(claims.email.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn verify_id_token_expired_rejected() {
        // exp = NOW - 60 (за пределами leeway 30 c) → Auth «войти снова».
        let token = make_token(
            &serde_json::json!({"alg": "RS256", "kid": "k1"}),
            &valid_payload(NOW - 60),
        );
        let err = verify_id_token(
            &token,
            Some("n0nce"),
            "dynamic_agent_client",
            NOW,
            &NoopVerifier,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, LlmError::Auth(_)), "{err}");
        assert!(err.to_string().contains("истёк"));
    }

    #[test]
    fn verify_id_token_bad_issuer_aud_nonce_rejected() {
        let mut payload = valid_payload(NOW + 3600);
        payload["iss"] = serde_json::json!("https://evil.example.com");
        let token = make_token(&serde_json::json!({"alg": "RS256"}), &payload);
        assert!(matches!(
            verify_id_token(
                &token,
                None,
                "dynamic_agent_client",
                NOW,
                &NoopVerifier,
                &[]
            ),
            Err(LlmError::Auth(_))
        ));

        let mut payload = valid_payload(NOW + 3600);
        payload["aud"] = serde_json::json!("other-client");
        let token = make_token(&serde_json::json!({"alg": "RS256"}), &payload);
        assert!(matches!(
            verify_id_token(
                &token,
                None,
                "dynamic_agent_client",
                NOW,
                &NoopVerifier,
                &[]
            ),
            Err(LlmError::Auth(_))
        ));

        let token = make_token(
            &serde_json::json!({"alg": "RS256"}),
            &valid_payload(NOW + 3600),
        );
        // nonce не совпал → replay-отказ.
        assert!(matches!(
            verify_id_token(
                &token,
                Some("другой"),
                "dynamic_agent_client",
                NOW,
                &NoopVerifier,
                &[]
            ),
            Err(LlmError::Auth(_))
        ));
    }

    #[test]
    fn verify_id_token_wrong_alg_rejected() {
        let token = make_token(
            &serde_json::json!({"alg": "none"}),
            &valid_payload(NOW + 3600),
        );
        let err = verify_id_token(
            &token,
            None,
            "dynamic_agent_client",
            NOW,
            &NoopVerifier,
            &[],
        )
        .unwrap_err();
        assert!(err.to_string().contains("alg"));
    }

    #[test]
    fn parse_id_token_garbage_rejected() {
        assert!(parse_id_token("не-jwt").is_err());
        assert!(parse_id_token("a.b").is_err());
        assert!(parse_id_token("!!!.###.$$$").is_err());
    }

    #[test]
    fn parse_jwks_skips_non_rsa() {
        let body = serde_json::json!({
            "keys": [
                {"kty": "EC", "crv": "P-256", "kid": "ec1", "x": "x", "y": "y"},
                {"kty": "RSA", "kid": "rsa1", "n": "bm9kYXRh", "e": "AQAB", "alg": "RS256"}
            ]
        });
        let keys = parse_jwks(&body).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].kid, "rsa1");
        assert_eq!(keys[0].e, "AQAB");
    }
}
