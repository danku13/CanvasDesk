//! FR-LLM-A / Q1+Q5 (privacy + data residency) — Compliance flags.
//!
//! `DataResidency` определяет, какие провайдеры доступны и применяется
//! ли `redact_context()` перед отправкой контекста в LLM. Выбор
//! пользователь делает на онбординге (Stream B `onboarding_ui.rs`)
//! или в Settings (F-7.7).
//!
//! **Три режима** (Q5 ответ владельца 2026-10-03: hybrid + self-hosted
//! любой + документация + онбординг):
//! - `Local`: только Ollama / Laya sidecar. Данные не покидают машину.
//!   Никакого redact — нет облачного транспорта.
//! - `Cloud`: ChatGPT OAuth / BYOK (OpenAI/Anthropic/z.ai/Moonshot/OpenRouter).
//!   Redact включён, т.к. контекст уходит к провайдеру.
//! - `SelfHosted`: любой URL + key (ответственность пользователя).
//!   Redact выключен — данные в контуре пользователя, но провайдер
//!   не входит в whitelist ChatGPT OAuth/BYOK-cloud.

/// Режим пребывания данных (Q5). Определяет доступные провайдеры и
/// применение `redact_context()` (Q1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataResidency {
    /// Только local: Ollama / Laya. Данные не покидают машину.
    #[default]
    Local,
    /// Cloud: ChatGPT OAuth / BYOK-cloud. Redact включён (Q1).
    Cloud,
    /// Self-hosted: любой URL + key (ответственность пользователя).
    /// Redact выключен (данные в контуре пользователя).
    SelfHosted,
}

impl DataResidency {
    /// Privacy-режим для `redact_context()` (Q1).
    ///
    /// - `Local` / `SelfHosted` → `PrivacyMode::Off` (контекст как есть).
    /// - `Cloud` → `PrivacyMode::Redact` (числа заменяются на `<redacted>`).
    pub fn privacy_mode(&self) -> PrivacyMode {
        match self {
            DataResidency::Local | DataResidency::SelfHosted => PrivacyMode::Off,
            DataResidency::Cloud => PrivacyMode::Redact,
        }
    }

    /// Доступен ли Sign-in-with-ChatGPT (OAuth) в этом режиме.
    ///
    /// OAuth отправляет данные к OpenAI → только в `Cloud`.
    /// В `Local`/`SelfHosted` кнопка «Continue with ChatGPT» недоступна
    /// (Stream B Settings UI).
    pub fn allows_chatgpt_oauth(&self) -> bool {
        matches!(self, DataResidency::Cloud)
    }

    /// Доступны ли BYOK-cloud провайдеры (OpenAI/Anthropic/z.ai/Moonshot/
    /// OpenRouter) в этом режиме.
    ///
    /// Только в `Cloud` (данные уходят к провайдеру).
    pub fn allows_byok_cloud(&self) -> bool {
        matches!(self, DataResidency::Cloud)
    }

    /// Доступны ли self-hosted провайдеры (Ollama / Laya sidecar /
    /// произвольный URL) в этом режиме.
    ///
    /// `SelfHosted` (основной сценарий) и `Local` (Ollama/Laya как
    /// частный случай self-hosted на localhost).
    pub fn allows_self_hosted(&self) -> bool {
        matches!(self, DataResidency::SelfHosted | DataResidency::Local)
    }
}

impl std::fmt::Display for DataResidency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataResidency::Local => write!(f, "Local"),
            DataResidency::Cloud => write!(f, "Cloud"),
            DataResidency::SelfHosted => write!(f, "SelfHosted"),
        }
    }
}

/// Privacy-режим для `redact_context()` (Q1). Производный от
/// `DataResidency` (см. `DataResidency::privacy_mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PrivacyMode {
    /// Контекст уходит как есть (Local / SelfHosted).
    #[default]
    Off,
    /// Числа в шаблонах `key=value[unit]` заменяются на `<redacted>`
    /// (Cloud — данные уходят к провайдеру).
    Redact,
}

impl std::fmt::Display for PrivacyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrivacyMode::Off => write!(f, "Off"),
            PrivacyMode::Redact => write!(f, "Redact"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_mode_mapping() {
        assert_eq!(DataResidency::Local.privacy_mode(), PrivacyMode::Off);
        assert_eq!(DataResidency::SelfHosted.privacy_mode(), PrivacyMode::Off);
        assert_eq!(DataResidency::Cloud.privacy_mode(), PrivacyMode::Redact);
    }

    #[test]
    fn allows_chatgpt_oauth_only_cloud() {
        assert!(!DataResidency::Local.allows_chatgpt_oauth());
        assert!(DataResidency::Cloud.allows_chatgpt_oauth());
        assert!(!DataResidency::SelfHosted.allows_chatgpt_oauth());
    }

    #[test]
    fn allows_byok_cloud_only_cloud() {
        assert!(!DataResidency::Local.allows_byok_cloud());
        assert!(DataResidency::Cloud.allows_byok_cloud());
        assert!(!DataResidency::SelfHosted.allows_byok_cloud());
    }

    #[test]
    fn allows_self_hosted_local_and_self_hosted() {
        assert!(DataResidency::Local.allows_self_hosted());
        assert!(!DataResidency::Cloud.allows_self_hosted());
        assert!(DataResidency::SelfHosted.allows_self_hosted());
    }

    #[test]
    fn default_is_local() {
        // Дефолт — самый приватный режим (Local). Онбординг переопределяет
        // при выборе пользователя.
        assert_eq!(DataResidency::default(), DataResidency::Local);
    }

    #[test]
    fn display_strings() {
        assert_eq!(DataResidency::Local.to_string(), "Local");
        assert_eq!(DataResidency::Cloud.to_string(), "Cloud");
        assert_eq!(DataResidency::SelfHosted.to_string(), "SelfHosted");
        assert_eq!(PrivacyMode::Off.to_string(), "Off");
        assert_eq!(PrivacyMode::Redact.to_string(), "Redact");
    }
}
